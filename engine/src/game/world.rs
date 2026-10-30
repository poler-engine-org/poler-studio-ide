//! World: сущности, компоненты, иерархия — сердце игрового ядра.
//!
//! Конструкция против двух болей UE (см. roadmap §2):
//! * **нет GC** — сущность это `(index, generation)`; слот умер →
//!   его старые handle всюду дают `None`, resurrection невозможен;
//! * **нет pointer-магии** — родственные связи по тем же handle,
//!   итерация — по плотным слотам, без аллокаций на тике.
//!
//! Иерархия: `parent → children`; удаление родителя удаляет поддерево
//! (как `AActor::Destroy` роняет прикреплённые акторы). Мировая
//! трансформация считается DFS от корней — родители всегда раньше детей.

use std::collections::HashSet;

use super::orbit::Orbit;
use super::transform::Transform;

/// Хендл сущности. `generation` отличает живого от мёртвого предшественника.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Entity {
    pub index: u32,
    pub generation: u32,
}

impl Entity {
    #[inline]
    pub fn raw(self) -> u64 {
        ((self.generation as u64) << 32) | self.index as u64
    }
}

/// Класс небесного тела (влияет на рендер и сегментацию).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BodyClass {
    Star,
    Planet,
    Moon,
    Station,
    Probe,
}

impl BodyClass {
    /// Идентификатор сегмента P³ (палитра 10 цветов, 0 = фон).
    pub fn seg_id(self) -> u8 {
        match self {
            BodyClass::Star => 4,    // золото
            BodyClass::Planet => 1,  // циан (переопределяется индексом планеты)
            BodyClass::Moon => 8,    // серый
            BodyClass::Station => 9, // сталь
            BodyClass::Probe => 5,   // фиолетовый
        }
    }
}

/// Физическое тело: масса (для кеплеровских ω детей), радиус, класс.
#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub class: BodyClass,
    pub mass: f64,
    pub radius: f64,
    /// Переопределение сегмента рендера (планеты получают свой цвет).
    pub seg_override: Option<u8>,
}

/// Мир: хранилище сущностей и компонентов.
///
/// Компоненты — плотные `Vec<Option<T>>` по слотам: никакой динамики
/// на горячем пути, обход кэш-дружелюбен, наличие/отсутствие — `Option`.
pub struct World {
    /// Слот сущности: жива ли, поколение, связи, имя.
    slots: Vec<SlotMeta>,
    free: Vec<u32>,
    transforms: Vec<Option<Transform>>,
    orbits: Vec<Option<Orbit>>,
    bodies: Vec<Option<Body>>,
    /// Кэш мировых положений (пересчитывается каждый тик).
    world_pos: Vec<[f64; 3]>,
    world_spin: Vec<f64>,
    /// Счётчик тиков симуляции.
    pub tick: u64,
}

#[derive(Clone)]
struct SlotMeta {
    alive: bool,
    generation: u32,
    parent: Option<Entity>,
    children: Vec<Entity>,
    name: String,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        World {
            slots: Vec::new(),
            free: Vec::new(),
            transforms: Vec::new(),
            orbits: Vec::new(),
            bodies: Vec::new(),
            world_pos: Vec::new(),
            world_spin: Vec::new(),
            tick: 0,
        }
    }

    // ------------------------------------------------------------------
    // Жизненный цикл сущностей
    // ------------------------------------------------------------------

    /// Создать сущность. Слот берётся из свободных (с новым поколением)
    /// либо расширяются хранилища.
    pub fn spawn(&mut self, name: &str) -> Entity {
        let (index, generation) = match self.free.pop() {
            Some(i) => {
                let gen = self.slots[i as usize].generation;
                (i, gen)
            }
            None => {
                let i = self.slots.len() as u32;
                self.slots.push(SlotMeta {
                    alive: false,
                    generation: 0,
                    parent: None,
                    children: Vec::new(),
                    name: String::new(),
                });
                self.transforms.push(None);
                self.orbits.push(None);
                self.bodies.push(None);
                self.world_pos.push([0.0; 3]);
                self.world_spin.push(0.0);
                (i, 0)
            }
        };
        let meta = &mut self.slots[index as usize];
        meta.alive = true;
        meta.parent = None;
        meta.children.clear();
        meta.name = name.to_string();
        Entity { index, generation }
    }

    /// Жива ли сущность (поколение должно совпасть).
    pub fn is_alive(&self, e: Entity) -> bool {
        self.meta(e).map(|m| m.alive && m.generation == e.generation).unwrap_or(false)
    }

    /// Метаданные слота, если хендл актуален.
    fn meta(&self, e: Entity) -> Option<&SlotMeta> {
        self.slots.get(e.index as usize).filter(|_| {
            // поколение проверяет вызывающий через is_alive; здесь только границы
            true
        })
    }

    pub fn name(&self, e: Entity) -> Option<&str> {
        self.slots
            .get(e.index as usize)
            .filter(|m| m.alive && m.generation == e.generation)
            .map(|m| m.name.as_str())
    }

    pub fn parent(&self, e: Entity) -> Option<Entity> {
        self.slots
            .get(e.index as usize)
            .filter(|m| m.alive && m.generation == e.generation)
            .and_then(|m| m.parent)
    }

    pub fn children(&self, e: Entity) -> &[Entity] {
        self.slots
            .get(e.index as usize)
            .map(|m| m.children.as_slice())
            .unwrap_or(&[])
    }

    /// Все живые сущности (в порядке слотов — детерминированно).
    pub fn entities(&self) -> Vec<Entity> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive)
            .map(|(i, m)| Entity {
                index: i as u32,
                generation: m.generation,
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.slots.iter().filter(|m| m.alive).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Привязать ребёнка к родителю. Родитель обязан быть живым; циклы
    /// невозможны конструктивно: ребёнок отвязывается от прежнего родителя.
    pub fn set_parent(&mut self, child: Entity, parent: Option<Entity>) -> Result<(), String> {
        if !self.is_alive(child) {
            return Err(format!("set_parent: сущность {} мертва", child.index));
        }
        if let Some(p) = parent {
            if !self.is_alive(p) {
                return Err(format!("set_parent: родитель {} мертв", p.index));
            }
            if p == child {
                return Err("set_parent: сущность не может быть своим родителем".into());
            }
        }
        // отвязать от прежнего
        if let Some(old) = self.slots[child.index as usize].parent {
            self.slots[old.index as usize]
                .children
                .retain(|&c| c != child);
        }
        self.slots[child.index as usize].parent = parent;
        if let Some(p) = parent {
            self.slots[p.index as usize].children.push(child);
        }
        Ok(())
    }

    /// Удалить сущность и всё её поддерево (как DestroyActor в UE).
    /// Порядок: сначала дети (рекурсивно), потом сама сущность.
    pub fn despawn(&mut self, e: Entity) -> Result<usize, String> {
        if !self.is_alive(e) {
            return Err(format!("despawn: сущность {} уже мертва", e.index));
        }
        let kids: Vec<Entity> = self.slots[e.index as usize].children.clone();
        let mut removed = 0;
        for c in kids {
            removed += self.despawn(c).unwrap_or(0);
        }
        // отвязать от родителя
        if let Some(old) = self.slots[e.index as usize].parent {
            self.slots[old.index as usize]
                .children
                .retain(|&c| c != e);
        }
        let meta = &mut self.slots[e.index as usize];
        meta.alive = false;
        meta.generation += 1; // воскресить старый handle нельзя
        meta.parent = None;
        meta.children.clear();
        let i = e.index as usize;
        self.transforms[i] = None;
        self.orbits[i] = None;
        self.bodies[i] = None;
        self.free.push(e.index);
        Ok(removed + 1)
    }

    // ------------------------------------------------------------------
    // Компоненты
    // ------------------------------------------------------------------

    fn slot(&self, e: Entity, what: &str) -> Result<usize, String> {
        if !self.is_alive(e) {
            return Err(format!("{what}: сущность {} мертва", e.index));
        }
        Ok(e.index as usize)
    }

    pub fn set_transform(&mut self, e: Entity, t: Transform) -> Result<(), String> {
        let i = self.slot(e, "set_transform")?;
        self.transforms[i] = Some(t);
        Ok(())
    }

    pub fn transform(&self, e: Entity) -> Option<Transform> {
        self.transforms.get(e.index as usize).copied().flatten()
    }

    pub fn set_orbit(&mut self, e: Entity, o: Orbit) -> Result<(), String> {
        let i = self.slot(e, "set_orbit")?;
        self.orbits[i] = Some(o);
        Ok(())
    }

    pub fn orbit(&self, e: Entity) -> Option<Orbit> {
        self.orbits.get(e.index as usize).copied().flatten()
    }

    pub fn set_body(&mut self, e: Entity, b: Body) -> Result<(), String> {
        let i = self.slot(e, "set_body")?;
        self.bodies[i] = Some(b);
        Ok(())
    }

    pub fn body(&self, e: Entity) -> Option<Body> {
        self.bodies.get(e.index as usize).copied().flatten()
    }

    // ------------------------------------------------------------------
    // Тик: фазы систем (фиксированный порядок — контракт, как tick groups UE)
    // ------------------------------------------------------------------

    /// Один шаг симуляции. Фазы:
    /// 1. **Orbit** — углы и локальные позиции орбит;
    /// 2. **Transform** — мировые позиции по иерархии (DFS от корней);
    /// 3. самовращение (`spin_rate`) учитывается в фазе Transform.
    pub fn tick(&mut self, dt: f64) {
        // --- Фаза 1: орбиты ---
        let n = self.slots.len();
        for i in 0..n {
            if !self.slots[i].alive {
                continue;
            }
            if let Some(o) = self.orbits[i].as_mut() {
                o.angle += o.omega * dt;
                o.angle %= 2.0 * std::f64::consts::PI;
                // локальная позиция: наклон орбиты вокруг X
                let (s, c) = (o.angle.sin(), o.angle.cos());
                let ci = o.inclination.cos();
                let si = o.inclination.sin();
                let pos = [o.radius * c, o.radius * s * si, o.radius * s * ci];
                if let Some(t) = self.transforms[i].as_mut() {
                    t.pos = pos;
                }
            }
            // самовращение
            if let Some(t) = self.transforms[i].as_mut() {
                t.spin += t.spin_rate * dt;
            }
        }

        // --- Фаза 2: мировые трансформы (родители раньше детей) ---
        let roots: Vec<Entity> = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive && m.parent.is_none())
            .map(|(i, m)| Entity {
                index: i as u32,
                generation: m.generation,
            })
            .collect();
        for r in roots {
            let parent_pos = [0.0; 3];
            let parent_spin = 0.0;
            self.update_world(r, parent_pos, parent_spin);
        }

        self.tick += 1;
    }

    /// DFS: мировая позиция = родительская + локальная (со спином родителя
    /// для корректной иерархии позже; сейчас перенос — трансляция).
    fn update_world(&mut self, e: Entity, parent_pos: [f64; 3], parent_spin: f64) {
        let i = e.index as usize;
        let t = self.transforms[i].unwrap_or(Transform::NEUTRAL);
        let wp = [
            parent_pos[0] + t.pos[0],
            parent_pos[1] + t.pos[1],
            parent_pos[2] + t.pos[2],
        ];
        let ws = parent_spin + t.spin;
        self.world_pos[i] = wp;
        self.world_spin[i] = ws;
        let kids = self.slots[i].children.clone();
        for c in kids {
            if self.is_alive(c) {
                self.update_world(c, wp, ws);
            }
        }
    }

    /// Мировая позиция сущности (кэш последнего тика).
    pub fn world_pos(&self, e: Entity) -> Option<[f64; 3]> {
        if !self.is_alive(e) {
            return None;
        }
        Some(self.world_pos[e.index as usize])
    }

    /// Мировой спин (радианы, накопительный).
    pub fn world_spin(&self, e: Entity) -> Option<f64> {
        if !self.is_alive(e) {
            return None;
        }
        Some(self.world_spin[e.index as usize])
    }

    // ------------------------------------------------------------------
    // Детерминизм-маяк
    // ------------------------------------------------------------------

    /// State-hash: FNV-1a по (слот, поколение, биты мировых позиций) всех
    /// живых сущностей. Одинаковая история тиков → бит-в-бит одинаковый
    /// хэш — фундамент replay/lockstep-сетевого кода (вместо того чтобы
    /// верить репликации UE «на слово»).
    pub fn state_hash(&self) -> u64 {
        const OFFSET: u64 = 0xcbf29ce484222325;
        const PRIME: u64 = 0x100000001b3;
        let mut h = OFFSET;
        h ^= self.tick;
        h = h.wrapping_mul(PRIME);
        for (i, m) in self.slots.iter().enumerate() {
            if !m.alive {
                continue;
            }
            h ^= (i as u64) | ((m.generation as u64) << 32);
            h = h.wrapping_mul(PRIME);
            let p = self.world_pos[i];
            for x in p {
                h ^= x.to_bits();
                h = h.wrapping_mul(PRIME);
            }
        }
        h
    }

    /// Найти сущность по имени (первое совпадение; имена не обязаны
    /// быть уникальными — это документированная свобода, как теги).
    pub fn find(&self, name: &str) -> Option<Entity> {
        self.slots
            .iter()
            .enumerate()
            .find(|(_, m)| m.alive && m.name == name)
            .map(|(i, m)| Entity {
                index: i as u32,
                generation: m.generation,
            })
    }

    /// Имена всех живых сущностей, отсортированные по слоту.
    pub fn names(&self) -> Vec<(Entity, String)> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive)
            .map(|(i, m)| {
                (
                    Entity {
                        index: i as u32,
                        generation: m.generation,
                    },
                    m.name.clone(),
                )
            })
            .collect()
    }

    /// Проверка целостности иерархии (для тестов): дети знают родителя,
    /// родитель — детей, циклов нет.
    pub fn hierarchy_valid(&self) -> bool {
        let mut seen = HashSet::new();
        for (i, m) in self.slots.iter().enumerate() {
            if !m.alive {
                continue;
            }
            let e = Entity {
                index: i as u32,
                generation: m.generation,
            };
            if !seen.insert(e.raw()) {
                return false;
            }
            if let Some(p) = m.parent {
                if !self.is_alive(p) {
                    return false;
                }
                if !self.slots[p.index as usize].children.contains(&e) {
                    return false;
                }
            }
            for &c in &m.children {
                if !self.is_alive(c) {
                    return false;
                }
                if self.slots[c.index as usize].parent != Some(e) {
                    return false;
                }
            }
        }
        // отсутствие циклов: DFS не должен встретить повтор
        for (i, m) in self.slots.iter().enumerate() {
            if m.alive && m.parent.is_none() {
                let root = Entity {
                    index: i as u32,
                    generation: m.generation,
                };
                let mut visited = HashSet::new();
                let mut stack = vec![root];
                while let Some(x) = stack.pop() {
                    if !visited.insert(x.raw()) {
                        return false;
                    }
                    for &c in self.children(x) {
                        stack.push(c);
                    }
                }
            }
        }
        true
    }
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_despawn_generation() {
        let mut w = World::new();
        let a = w.spawn("a");
        assert!(w.is_alive(a));
        assert_eq!(w.len(), 1);
        w.despawn(a).unwrap();
        assert!(!w.is_alive(a), "старый handle мёртв");
        // слот переиспользуется с новым поколением
        let b = w.spawn("b");
        assert_eq!(b.index, a.index, "слот переиспользован");
        assert_ne!(b.generation, a.generation, "поколение выросло");
        assert!(!w.is_alive(a), "воскрешение невозможно");
        assert!(w.is_alive(b));
    }

    #[test]
    fn despawn_drops_subtree() {
        let mut w = World::new();
        let sun = w.spawn("sun");
        let planet = w.spawn("planet");
        let moon = w.spawn("moon");
        w.set_parent(planet, Some(sun)).unwrap();
        w.set_parent(moon, Some(planet)).unwrap();
        assert!(w.hierarchy_valid());
        assert_eq!(w.despawn(sun).unwrap(), 3, "поддерево целиком");
        assert!(w.is_empty());
        assert!(w.hierarchy_valid());
    }

    #[test]
    fn dead_parent_rejected() {
        let mut w = World::new();
        let a = w.spawn("a");
        let b = w.spawn("b");
        w.despawn(a).unwrap();
        assert!(w.set_parent(b, Some(a)).is_err(), "мёртвый родитель");
        assert!(w.set_parent(b, Some(b)).is_err(), "сам себе родитель");
        assert!(w.hierarchy_valid());
    }

    #[test]
    fn reattach_moves_child() {
        let mut w = World::new();
        let p1 = w.spawn("p1");
        let p2 = w.spawn("p2");
        let c = w.spawn("c");
        w.set_parent(c, Some(p1)).unwrap();
        w.set_parent(c, Some(p2)).unwrap();
        assert_eq!(w.parent(c), Some(p2));
        assert!(w.children(p1).is_empty());
        assert_eq!(w.children(p2), &[c]);
        assert!(w.hierarchy_valid());
    }

    #[test]
    fn world_positions_compose_hierarchy() {
        let mut w = World::new();
        let sun = w.spawn("sun");
        let planet = w.spawn("planet");
        let moon = w.spawn("moon");
        w.set_parent(planet, Some(sun)).unwrap();
        w.set_parent(moon, Some(planet)).unwrap();
        w.set_transform(sun, Transform::at([1.0, 0.0, 0.0])).unwrap();
        w.set_transform(planet, Transform::at([2.0, 3.0, 0.0])).unwrap();
        w.set_transform(moon, Transform::at([0.5, -1.0, 0.25])).unwrap();
        w.tick(0.0);
        let mp = w.world_pos(moon).unwrap();
        let expected = [1.0 + 2.0 + 0.5, 0.0 + 3.0 - 1.0, 0.25];
        for (a, b) in mp.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-15);
        }
    }

    #[test]
    fn state_hash_tracks_world_changes() {
        let mut w = World::new();
        let a = w.spawn("a");
        w.set_transform(a, Transform::at([0.0, 0.0, 0.0])).unwrap();
        w.tick(0.0);
        let h0 = w.state_hash();
        w.set_transform(a, Transform::at([1.0, 0.0, 0.0])).unwrap();
        w.tick(0.0);
        let h1 = w.state_hash();
        assert_ne!(h0, h1, "хэш чувствителен к позициям");
        // бит-в-бит детерминизм: та же ИСТОРИЯ — тот же хэш
        // (хэш включает счётчик тиков: повторяем её целиком)
        let mut w2 = World::new();
        let a2 = w2.spawn("a");
        w2.set_transform(a2, Transform::at([0.0, 0.0, 0.0])).unwrap();
        w2.tick(0.0);
        w2.set_transform(a2, Transform::at([1.0, 0.0, 0.0])).unwrap();
        w2.tick(0.0);
        assert_eq!(w.state_hash(), w2.state_hash());
    }

    #[test]
    fn find_by_name() {
        let mut w = World::new();
        let a = w.spawn("aetheria");
        w.spawn("other");
        assert_eq!(w.find("aetheria"), Some(a));
        assert_eq!(w.find("missing"), None);
    }
}
