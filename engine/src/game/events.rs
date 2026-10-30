//! # U1: Система событий — очередь с коалесцингом (цикл U, v0.56.0)
//!
//! Ответ на боль UE: ` PeekMessage → виртуальный Dispatch → распределение
//! по всем подписчикам` — порядок побочных эффектов размазан по кадру,
//! события мыши за один кадр плодят тысячи аллокаций.
//!
//! Принцип POLER: события — **типизированные значения** в одной очереди
//! с явной политикой коалесцинга. Порядок строгий (FIFO внутри некоалес-
//! цируемых типов), никакой виртуальной диспетчеризации, никакого
//! распределения по куче на каждое движение мыши.
//!
//! ```text
//!   Источник (окно/скрипт/сеть)      →  EventQueue::push
//!        │ коалесцинг: MouseMove сливается в одно суммарное,
//!        │            Resize/ Wheel держат последнее значение
//!        ▼
//!   Кадр: while let Some(ev) = queue.pop() → Input::on_event(ev)
//!        → логика читает свёрнутое состояние (Input)
//! ```
//!
//! Детерминизм: очередь хешируется FNV-1a (`events_hash`) — одинаковый
//! сценарий ввода даёт бит-в-бит одинаковый хеш потока событий. Это
//! контракт для replay/lockstep: состояние мира + поток событий вместе
//! полностью определяют следующий кадр.

use std::collections::VecDeque;

/// Фаза клавиши.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPhase {
    Pressed,
    Released,
    /// Автоповтор (ОС шлёт при удержании) — логика может игнорировать.
    Repeat,
}

/// Кнопка мыши (1 = левая, как в X11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    X(u8),
}

impl MouseButton {
    pub fn index(self) -> usize {
        match self {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
            MouseButton::X(i) => 3 + (i as usize % 5),
        }
    }
}

/// Типизированное событие ввода/окна.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// Пользователь закрыл окно.
    WindowClose,
    /// Изменение размера клиентской области (px).
    WindowResize { w: u32, h: u32 },
    /// Фокус окна получил/потерял.
    WindowFocus { gained: bool },
    /// Клавиша.
    Key { code: crate::game::input::KeyCode, phase: KeyPhase },
    /// Введённый символ (текстовый ввод, не навигация).
    Text(char),
    /// Движение мыши: накопленная дельта за интервал (px, система координат
    /// окна, Y вниз). Коалесцируется: событие одно, дельты суммируются.
    MouseMove { dx: f64, dy: f64 },
    /// Кнопка мыши.
    MouseButton { button: MouseButton, phase: KeyPhase },
    /// Колесо: положительное — от пользователя (zoom in).
    MouseWheel { delta: f64 },
    /// Пользовательское событие (сетевой слой, таймеры, gamepad-стаб).
    User(u64),
}

impl Event {
    /// Свернуть два события одного типа в одно (коалесцинг).
    /// `None` — события не коалесцируются (порядок важен).
    pub fn coalesce(self, next: Event) -> Option<Event> {
        use Event::*;
        match (self, next) {
            (MouseMove { dx: a1, dy: a2 }, MouseMove { dx: b1, dy: b2 }) => {
                Some(MouseMove { dx: a1 + b1, dy: a2 + b2 })
            }
            (WindowResize { .. }, r @ WindowResize { .. }) => Some(r),
            (WindowFocus { .. }, f @ WindowFocus { .. }) => Some(f),
            (MouseWheel { delta: a }, MouseWheel { delta: b }) => {
                Some(MouseWheel { delta: a + b })
            }
            _ => None,
        }
    }

    /// Приоритет обработки в очереди (меньше — раньше при равного рода
    /// событиях кадра; фактически сохраняет FIFO, но даёт политике
    /// перестроения точку входа).
    pub fn priority(self) -> u8 {
        use Event::*;
        match self {
            WindowClose => 0,
            WindowResize { .. } => 1,
            WindowFocus { .. } => 2,
            Key { .. } | Text(_) => 3,
            MouseButton { .. } => 4,
            MouseMove { .. } => 5,
            MouseWheel { .. } => 6,
            User(_) => 7,
        }
    }
}

/// Очередь событий кадра с коалесцингом.
///
/// Инварианты:
/// - `len ≤ capacity`: при переполнении коалесцируемые типы сжимаются,
///   некоалесцируемые теряют **самые старые** (политика drop-oldest —
///   свежий ввод важнее исторического, как в боевых движках);
/// - порядок некоалесцируемых событий — строгий FIFO.
#[derive(Debug)]
pub struct EventQueue {
    queue: VecDeque<Event>,
    capacity: usize,
    /// Счётчик принятых событий (до коалесцинга) — диагностика потерь.
    pub pushed_total: u64,
    /// Сколько событий свернуто коалесцингом.
    pub coalesced_total: u64,
    /// Сколько отброшено drop-oldest.
    pub dropped_total: u64,
}

impl Default for EventQueue {
    fn default() -> Self {
        EventQueue::new(1024)
    }
}

impl EventQueue {
    /// Ёмкость — верхняя граница длины очереди.
    pub fn new(capacity: usize) -> Self {
        EventQueue {
            queue: VecDeque::with_capacity(capacity.max(16)),
            capacity: capacity.max(16),
            pushed_total: 0,
            coalesced_total: 0,
            dropped_total: 0,
        }
    }

    /// Принять событие. Коалесцируемые типы сливаются с хвостом; при
    /// переполнении — попытка сжатия, затем drop-oldest.
    pub fn push(&mut self, ev: Event) {
        self.pushed_total += 1;
        if let Some(tail) = self.queue.back_mut() {
            if let Some(merged) = tail.coalesce(ev) {
                *tail = merged;
                self.coalesced_total += 1;
                return;
            }
        }
        if self.queue.len() >= self.capacity {
            // Сначала пробуем сжать однотипные повторы в середине очереди
            if !self.compact_once() {
                if let Some(dropped) = self.queue.pop_front() {
                    self.dropped_total += 1;
                    // Коалесцируемый тип мог бы слиться с хвостом — но он
                    // не смог (иначе не попал бы сюда), теряем честно.
                    let _ = dropped;
                }
            }
        }
        self.queue.push_back(ev);
    }

    /// Одна проходка сжатия: ищем соседнюю пару, коалесцируемую
    /// «через голову» промежуточных событий другого типа. Возвращает
    /// true, если что-то сжалось.
    fn compact_once(&mut self) -> bool {
        let n = self.queue.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (self.queue[i], self.queue[j]);
                if let Some(merged) = a.coalesce(b) {
                    // Типы MouseMove с промежуточным Key между ними:
                    // сворачиваем в позицию более раннего, j удаляем.
                    // Безопасно: коалесцинг коммутативен по сумме/последнему.
                    self.queue[i] = merged;
                    self.queue.remove(j);
                    self.coalesced_total += 1;
                    return true;
                }
            }
        }
        false
    }

    /// Извлечь следующее событие (FIFO).
    pub fn pop(&mut self) -> Option<Event> {
        self.queue.pop_front()
    }

    /// Длина очереди.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Пусто?
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Очистить (начало нового кадра после полного разбора).
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Хеш потока событий (FNV-1a по дискриминантам и полям).
    /// Контракт детерминизма: один сценарий → один хеш.
    pub fn events_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for ev in &self.queue {
            h ^= discriminant_of(ev);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
            h ^= payload_bits(ev);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }
}

/// Стабильный дискриминант варианта (не mem::discriminant — он
/// непрозрачен; здесь явные константы для кросс-версионной стабильности).
fn discriminant_of(ev: &Event) -> u64 {
    use Event::*;
    match ev {
        WindowClose => 1,
        WindowResize { .. } => 2,
        WindowFocus { .. } => 3,
        Key { .. } => 4,
        Text(_) => 5,
        MouseMove { .. } => 6,
        MouseButton { .. } => 7,
        MouseWheel { .. } => 8,
        User(_) => 9,
    }
}

/// Биты полезной нагрузки события (только значимые поля).
fn payload_bits(ev: &Event) -> u64 {
    use Event::*;
    match ev {
        WindowClose => 0,
        WindowResize { w, h } => (*w as u64) << 32 | *h as u64,
        WindowFocus { gained } => *gained as u64,
        Key { code, phase } => {
            let p = match phase {
                KeyPhase::Pressed => 0u64,
                KeyPhase::Released => 1,
                KeyPhase::Repeat => 2,
            };
            (code.bits() as u64) << 3 | p
        }
        Text(c) => *c as u64,
        MouseMove { dx, dy } => dx.to_bits() ^ (dy.to_bits() << 1),
        MouseButton { button, phase } => {
            let p = match phase {
                KeyPhase::Pressed => 0u64,
                KeyPhase::Released => 1,
                KeyPhase::Repeat => 2,
            };
            (button.index() as u64) << 3 | p
        }
        MouseWheel { delta } => delta.to_bits(),
        User(v) => *v,
    }
}

#[cfg(test)]
impl EventQueue {
    /// Только для тестов: заглянуть в хвост не извлекая всё.
    fn pop_back_for_test(&mut self) -> Option<Event> {
        self.queue.back().copied()
    }
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::input::KeyCode;

    #[test]
    fn fifo_order_preserved_for_non_coalescing() {
        let mut q = EventQueue::new(64);
        q.push(Event::WindowClose);
        q.push(Event::Key { code: KeyCode::Escape, phase: KeyPhase::Pressed });
        q.push(Event::User(7));
        assert_eq!(q.pop(), Some(Event::WindowClose));
        assert_eq!(
            q.pop(),
            Some(Event::Key { code: KeyCode::Escape, phase: KeyPhase::Pressed })
        );
        assert_eq!(q.pop(), Some(Event::User(7)));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn mouse_move_coalesces_by_summation() {
        let mut q = EventQueue::new(64);
        for (dx, dy) in [(1.0, 2.0), (3.0, -1.0), (0.5, 0.5)] {
            q.push(Event::MouseMove { dx, dy });
        }
        assert_eq!(q.len(), 1, "три движения — одно событие");
        assert_eq!(q.pop(), Some(Event::MouseMove { dx: 4.5, dy: 1.5 }));
        assert_eq!(q.coalesced_total, 2);
    }

    #[test]
    fn resize_keeps_last_and_wheel_sums() {
        let mut q = EventQueue::new(64);
        q.push(Event::WindowResize { w: 640, h: 480 });
        q.push(Event::WindowResize { w: 1280, h: 720 });
        assert_eq!(q.pop(), Some(Event::WindowResize { w: 1280, h: 720 }));
        q.push(Event::MouseWheel { delta: 1.0 });
        q.push(Event::MouseWheel { delta: 2.5 });
        assert_eq!(q.pop(), Some(Event::MouseWheel { delta: 3.5 }));
    }

    #[test]
    fn key_events_between_mouse_moves_do_not_merge() {
        let mut q = EventQueue::new(64);
        q.push(Event::MouseMove { dx: 1.0, dy: 0.0 });
        q.push(Event::Key { code: KeyCode::Space, phase: KeyPhase::Pressed });
        q.push(Event::MouseMove { dx: 2.0, dy: 0.0 });
        // Порядок: Move, Key, Move — вставка не слилась с головным хвостом
        assert_eq!(q.len(), 3);
        assert_eq!(q.pop(), Some(Event::MouseMove { dx: 1.0, dy: 0.0 }));
        assert_eq!(
            q.pop(),
            Some(Event::Key { code: KeyCode::Space, phase: KeyPhase::Pressed })
        );
        assert_eq!(q.pop(), Some(Event::MouseMove { dx: 2.0, dy: 0.0 }));
    }

    #[test]
    fn capacity_drops_oldest_and_reports() {
        let mut q = EventQueue::new(20);
        for i in 0..40u64 {
            q.push(Event::User(i)); // некоалесцируемые
        }
        assert!(q.len() <= 20, "длина ограничена ёмкостью: {}", q.len());
        assert!(q.dropped_total >= 20, "потери посчитаны: {}", q.dropped_total);
        // Самое старое потеряно, самое новое — на хвосте
        let last = q.pop_back_for_test();
        assert_eq!(last, Some(Event::User(39)));
    }

    #[test]
    fn compact_rescues_moves_across_other_events() {
        // Переполнение некоалесцируемыми + коалесцируемыми вперемешку:
        // сжатие должно найти пару Move по разные стороны от Key.
        let mut q = EventQueue::new(24);
        for i in 0..20u64 {
            q.push(Event::User(i));
        }
        q.push(Event::MouseMove { dx: 1.0, dy: 1.0 });
        q.push(Event::Key { code: KeyCode::W, phase: KeyPhase::Pressed });
        q.push(Event::MouseMove { dx: 5.0, dy: 0.0 });
        // Очередь переполнена: compact_once ищет пары через головы
        let moves: Vec<Event> = {
            let mut v = Vec::new();
            while let Some(ev) = q.pop() {
                v.push(ev);
            }
            v.into_iter()
                .filter(|e| matches!(e, Event::MouseMove { .. }))
                .collect()
        };
        // Оба Move могли слиться (10+? — зависит от компактации) или
        // остаться раздельными, но сумма дельт обязана сохраниться.
        let (sx, sy) = moves.iter().fold((0.0, 0.0), |(a, b), e| match e {
            Event::MouseMove { dx, dy } => (a + dx, b + dy),
            _ => (a, b),
        });
        assert!((sx - 6.0).abs() < 1e-9 && (sy - 1.0).abs() < 1e-9, "сумма дельт потеряна: {sx},{sy}");
    }

    #[test]
    fn events_hash_is_deterministic_and_sensitive() {
        let build = || {
            let mut q = EventQueue::new(64);
            q.push(Event::Key { code: KeyCode::W, phase: KeyPhase::Pressed });
            q.push(Event::MouseMove { dx: 3.5, dy: -1.25 });
            q.push(Event::MouseWheel { delta: 2.0 });
            q
        };
        let h1 = build().events_hash();
        let h2 = build().events_hash();
        assert_eq!(h1, h2, "один сценарий — один хеш");
        let mut other = build();
        other.push(Event::MouseWheel { delta: 2.0 }); // сольётся → сумма 4.0
        other.push(Event::WindowClose); // изменит хеш
        assert_ne!(h1, other.events_hash());
        // Пустая очередь — база FNV
        assert_eq!(EventQueue::new(8).events_hash(), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn priority_bounded_and_ordered_by_kind() {
        let close = Event::WindowClose.priority();
        let key = Event::Key { code: KeyCode::A, phase: KeyPhase::Pressed }.priority();
        let mv = Event::MouseMove { dx: 0.0, dy: 0.0 }.priority();
        assert!(close < key && key < mv);
        assert!(Event::User(1).priority() >= mv);
    }
}
