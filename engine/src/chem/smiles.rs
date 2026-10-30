//! OpenSMILES-парсер: молекулярный граф из строки вида "CC(=O)Oc1ccccc1C(=O)O".
//!
//! # Что поддерживается (Stage 1 лестницы «атом → молекула → белок → клетка»)
//!
//! * Органический субсет: B C N O P S F Cl Br I + ароматические b c n o p s
//! * Bracket-атомы: `[nH]`, `[NH4+]`, `[O-]`, `[13C]`, `[C@@H]` (хиральность
//!   парсится и сохраняется, в укладке v1 не используется)
//! * Связи: `-`, `=`, `#`, `$`, `:`, `/`, `\`, `~`; неявные связи
//!   (аром-аром → ароматическая 1.5, прочие → одинарная)
//! * Ветви `( )`, кольцевые замыкания `1…9` и `%10…%99`
//! * Дисконтинуация `.` — несколько компонентов
//! * Неявные водороды по валентностям органического субсета;
//!   ароматические связи считаются σ-порядком 1 (бензол → C6H6),
//!   ароматические n/o/s/p НЕ получают неявных H (пиридин — без H;
//!   пиррол пишется `[nH]`)
//! * SSSR: кратчайший цикл через каждую связь (BFS) + жадное покрытие
//!   связей — нафталин → два 6-цикла, индол → 6+5

use std::collections::BTreeMap;

/// Порядок связи.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BondOrder {
    Single,
    Double,
    Triple,
    Quadruple,
    Aromatic,
}

impl BondOrder {
    /// Численный порядок (ароматическая = 1.5).
    pub fn value(self) -> f64 {
        match self {
            BondOrder::Single => 1.0,
            BondOrder::Double => 2.0,
            BondOrder::Triple => 3.0,
            BondOrder::Quadruple => 4.0,
            BondOrder::Aromatic => 1.5,
        }
    }

    /// σ-порядок для подсчёта неявных водородов (ароматическая = 1).
    pub fn sigma(self) -> f64 {
        match self {
            BondOrder::Aromatic => 1.0,
            other => other.value(),
        }
    }

    pub fn name_ru(self) -> &'static str {
        match self {
            BondOrder::Single => "одинарная",
            BondOrder::Double => "двойная",
            BondOrder::Triple => "тройная",
            BondOrder::Quadruple => "четверная",
            BondOrder::Aromatic => "ароматическая",
        }
    }
}

/// Атом молекулярного графа.
#[derive(Debug, Clone)]
pub struct SmilesAtom {
    /// Символ элемента («C», «Cl», …), нормализован с заглавной.
    pub symbol: String,
    pub z: u8,
    pub aromatic: bool,
    pub charge: i8,
    /// Число H (явное из [nH] или неявное по валентности).
    pub h_count: u8,
    /// h_count задан явно bracket-синтаксисом?
    pub h_explicit: bool,
    pub isotope: Option<u16>,
    /// Хиральная метка @ / @@ (сохраняется, v1 не используется).
    pub chiral: Option<String>,
}

/// Связь молекулярного графа.
#[derive(Debug, Clone)]
pub struct SmilesBond {
    pub a: usize,
    pub b: usize,
    pub order: BondOrder,
}

/// Молекулярный граф.
#[derive(Debug, Clone)]
pub struct MoleculeGraph {
    pub atoms: Vec<SmilesAtom>,
    pub bonds: Vec<SmilesBond>,
    /// Число дисконтинуированных компонентов.
    pub components: usize,
    /// SSSR: списки индексов атомов циклов (порядок обхода по связям).
    pub rings: Vec<Vec<usize>>,
}

impl MoleculeGraph {
    /// Соседи атома: (индекс соседа, порядок связи).
    pub fn neighbors(&self, idx: usize) -> Vec<(usize, BondOrder)> {
        let mut v = Vec::new();
        for b in &self.bonds {
            if b.a == idx {
                v.push((b.b, b.order));
            } else if b.b == idx {
                v.push((b.a, b.order));
            }
        }
        v
    }

    pub fn degree(&self, idx: usize) -> usize {
        self.neighbors(idx).len()
    }

    /// Валентность атома (неявные H + связи, ароматика = 1.5).
    pub fn valence_used(&self, idx: usize) -> f64 {
        self.neighbors(idx).iter().map(|(_, o)| o.value()).sum::<f64>()
            + self.atoms[idx].h_count as f64
    }

    /// Молекулярная формула в нотации Хилла (C, H, затем алфавит) + заряд.
    pub fn hill_formula(&self) -> String {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for a in &self.atoms {
            *counts.entry(a.symbol.clone()).or_insert(0) += 1;
        }
        for a in &self.atoms {
            *counts.entry("H".to_string()).or_insert(0) += a.h_count as usize;
        }
        let mut parts = Vec::new();
        for sym in ["C", "H"] {
            if let Some(&n) = counts.get(sym) {
                // Единицу не пишем: H2O, а не H2O1 (нотация Хилла)
                parts.push(if n > 1 {
                    format!("{sym}{n}")
                } else {
                    sym.to_string()
                });
                counts.remove(sym);
            }
        }
        for (sym, &n) in &counts {
            parts.push(if n > 1 {
                format!("{sym}{n}")
            } else {
                sym.clone()
            });
        }
        let mut f = if parts.is_empty() {
            "пусто".to_string()
        } else {
            parts.join("")
        };
        match self.total_charge() {
            0 => {}
            1 => f.push('+'),
            -1 => f.push('-'),
            q => f.push_str(&format!("{q:+}")),
        }
        f
    }

    /// Суммарный формальный заряд.
    pub fn total_charge(&self) -> i32 {
        self.atoms.iter().map(|a| a.charge as i32).sum()
    }

    /// Атомы в ароматических кольцах (SSSR-цикл ≤ 6, все атомы ароматические).
    pub fn aromatic_ring_atoms(&self) -> Vec<usize> {
        let mut v = Vec::new();
        for ring in &self.rings {
            if ring.len() <= 6 && ring.iter().all(|&i| self.atoms[i].aromatic) {
                v.extend(ring.iter().copied());
            }
        }
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Принадлежит ли атом какому-либо кольцу.
    pub fn in_ring(&self, idx: usize) -> bool {
        self.rings.iter().any(|r| r.contains(&idx))
    }

    /// Является ли связь кольцевой (соседи в каком-то SSSR-цикле).
    pub fn bond_in_ring(&self, a: usize, b: usize) -> bool {
        self.rings
            .iter()
            .any(|r| r.contains(&a) && r.contains(&b) && adjacent_in(r, a, b))
    }

    /// Вращаемые связи: одинарная, не в кольце, оба атома не терминальные.
    pub fn rotatable_bonds(&self) -> Vec<usize> {
        let mut v = Vec::new();
        for (bi, b) in self.bonds.iter().enumerate() {
            if b.order != BondOrder::Single {
                continue;
            }
            if self.bond_in_ring(b.a, b.b) {
                continue;
            }
            if self.degree(b.a) >= 2 && self.degree(b.b) >= 2 {
                v.push(bi);
            }
        }
        v
    }
}

fn adjacent_in(ring: &[usize], a: usize, b: usize) -> bool {
    for (i, &x) in ring.iter().enumerate() {
        let y = ring[(i + 1) % ring.len()];
        if (x == a && y == b) || (x == b && y == a) {
            return true;
        }
    }
    false
}

// ─── Валентности органического субсета ─────────────────────────────────

/// Целевая валентность для неявных H.
fn default_valence(symbol: &str, charge: i8, sigma_sum: f64) -> f64 {
    let v: f64 = match symbol {
        "B" => 3.0,
        "C" => 4.0,
        "N" => 3.0,
        "O" => 2.0,
        "P" => {
            if sigma_sum > 3.0 { 5.0 } else { 3.0 }
        }
        "S" => {
            if sigma_sum > 2.0 { 6.0 } else { 2.0 }
        }
        "F" | "Cl" | "Br" | "I" => 1.0,
        _ => 0.0,
    };
    // Зарядовые поправки (v1, основные случаи)
    let adj = match (symbol, charge) {
        ("N", 1) => 1.0,
        ("N", -1) => -1.0,
        ("O", 1) => 1.0,
        ("O", -1) => -1.0,
        ("C", 1) | ("C", -1) => -1.0,
        ("S", 1) => 1.0,
        ("S", -1) => -1.0,
        _ => 0.0,
    };
    (v + adj).max(0.0)
}

// ─── Парсер ─────────────────────────────────────────────────────────────

struct Parser {
    chars: Vec<char>,
    pos: usize,
    atoms: Vec<SmilesAtom>,
    bonds: Vec<SmilesBond>,
    pending_order: Option<BondOrder>,
    ring_open: BTreeMap<u32, (usize, Option<BondOrder>)>,
    prev: Option<usize>,
    branch_stack: Vec<Option<usize>>,
    components: usize,
}

/// Разобрать SMILES в молекулярный граф.
pub fn parse_smiles(smiles: &str) -> Result<MoleculeGraph, String> {
    let mut p = Parser {
        chars: smiles.chars().collect(),
        pos: 0,
        atoms: Vec::new(),
        bonds: Vec::new(),
        pending_order: None,
        ring_open: BTreeMap::new(),
        prev: None,
        branch_stack: Vec::new(),
        components: 0,
    };
    p.run()?;

    // Неявные водороды по валентностям
    for i in 0..p.atoms.len() {
        if p.atoms[i].h_explicit {
            continue;
        }
        let sym = p.atoms[i].symbol.clone();
        // Ароматические гетероатомы не получают неявных H (пиридин);
        // пиррол обязан писать [nH]. Ароматические C/B — получают.
        if p.atoms[i].aromatic && matches!(sym.as_str(), "N" | "O" | "S" | "P" | "Se" | "As") {
            p.atoms[i].h_count = 0;
            continue;
        }
        if !matches!(
            sym.as_str(),
            "B" | "C" | "N" | "O" | "P" | "S" | "F" | "Cl" | "Br" | "I"
        ) {
            p.atoms[i].h_count = 0;
            continue;
        }
        // Порядок для H-подсчёта: ароматическая связь = 1.5
        // (бензольный C: 2·1.5 + H = валентность 4)
        let bond_sum: f64 = p
            .bonds
            .iter()
            .filter(|b| b.a == i || b.b == i)
            .map(|b| b.order.value())
            .sum();
        let sigma_sum = bond_sum;
        let target = default_valence(&sym, p.atoms[i].charge, sigma_sum);
        let h = (target - sigma_sum).round();
        if h > 0.0 && h <= 9.0 {
            p.atoms[i].h_count = h as u8;
        }
    }

    let mut graph = MoleculeGraph {
        atoms: p.atoms,
        bonds: p.bonds,
        components: p.components + 1, // «.»-разделителей на один меньше
        rings: Vec::new(),
    };
    graph.rings = sssr(&graph);
    Ok(graph)
}

/// Пересчитать неявные водороды для графа НЕ из SMILES (SDF и т.п.).
///
/// Семантика — как в `parse_smiles`: атомы с `h_explicit` не трогаются
/// (их H авторитетен — например, поглощенные явные H из SDF); для
/// остальных — по валентности, с тем же правилом ароматических
/// гетероатомов (пиридин: 0 H; пиррол обязан нести явный H).
/// Текущий `h_count` — нижняя граница (max, не замена).
pub fn infer_h_counts(g: &mut MoleculeGraph) {
    for i in 0..g.atoms.len() {
        if g.atoms[i].h_explicit {
            continue;
        }
        let sym = g.atoms[i].symbol.clone();
        if g.atoms[i].aromatic && matches!(sym.as_str(), "N" | "O" | "S" | "P" | "Se" | "As") {
            g.atoms[i].h_count = 0;
            continue;
        }
        if !matches!(
            sym.as_str(),
            "B" | "C" | "N" | "O" | "P" | "S" | "F" | "Cl" | "Br" | "I"
        ) {
            g.atoms[i].h_count = 0;
            continue;
        }
        let sigma_sum: f64 = g
            .bonds
            .iter()
            .filter(|b| b.a == i || b.b == i)
            .map(|b| b.order.value())
            .sum();
        let target = default_valence(&sym, g.atoms[i].charge, sigma_sum);
        let h = (target - sigma_sum).round();
        if h > g.atoms[i].h_count as f64 && h <= 9.0 {
            g.atoms[i].h_count = h as u8;
        }
    }
}

/// Пересчитать SSSR-кольца графа (для графов не из SMILES).
pub fn compute_rings(g: &mut MoleculeGraph) {
    g.rings = sssr(g);
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn run(&mut self) -> Result<(), String> {
        loop {
            match self.peek() {
                None => break,
                Some(' ') | Some('\t') | Some('\n') | Some('\r') => {
                    self.pos += 1;
                }
                Some('.') => {
                    self.pos += 1;
                    self.components += 1;
                    self.prev = None;
                    self.pending_order = None;
                }
                Some('(') => {
                    self.pos += 1;
                    self.branch_stack.push(self.prev);
                }
                Some(')') => {
                    self.pos += 1;
                    self.prev = self
                        .branch_stack
                        .pop()
                        .ok_or_else(|| format!("{}: лишняя ')'", self.pos))?;
                }
                Some(c) if "=-#$:~/\\".contains(c) => {
                    self.pos += 1;
                    self.pending_order = Some(match c {
                        '=' => BondOrder::Double,
                        '#' => BondOrder::Triple,
                        '$' => BondOrder::Quadruple,
                        ':' => BondOrder::Aromatic,
                        // ~ «любая», / и \ — стерео-одинарные
                        _ => BondOrder::Single,
                    });
                }
                Some('%') => {
                    self.pos += 1;
                    let d1 = self.next().and_then(|c| c.to_digit(10));
                    let d2 = self.next().and_then(|c| c.to_digit(10));
                    let label = d1
                        .zip(d2)
                        .map(|(a, b)| a * 10 + b)
                        .ok_or_else(|| format!("{}: %nn требует двух цифр", self.pos))?;
                    self.ring_closure(label)?;
                }
                Some(c) if c.is_ascii_digit() => {
                    self.pos += 1;
                    self.ring_closure(c.to_digit(10).unwrap())?;
                }
                Some('[') => {
                    let atom = self.bracket_atom()?;
                    self.push_atom(atom)?;
                }
                Some(c) if c.is_ascii_alphabetic() || c == '*' => {
                    self.pos += 1; // первый символ потребляем здесь
                    let atom = self.organic_atom(c)?;
                    self.push_atom(atom)?;
                }
                Some(c) => {
                    return Err(format!("{}: неожиданный символ '{c}'", self.pos));
                }
            }
        }
        if !self.branch_stack.is_empty() {
            return Err("незакрытая ветка '('".into());
        }
        if !self.ring_open.is_empty() {
            let labels: Vec<_> = self.ring_open.keys().collect();
            return Err(format!("незамкнутые кольца: {labels:?}"));
        }
        Ok(())
    }

    /// Атом органического субсета (одна-две буквы).
    fn organic_atom(&mut self, first: char) -> Result<SmilesAtom, String> {
        let aromatic = first.is_lowercase();
        let upper = first.to_ascii_uppercase();
        let mut symbol = String::new();
        symbol.push(upper);
        // Двухбуквенные органического субсета: Cl, Br (в т.ч. cl/br).
        // ВАЖНО: вторая буква в валидном SMILES — строчная, кандидат
        // собирается с сохранением регистра («Cl»≠«CL»: старый баг
        // давал «CL» и хлор/бром никогда не разбирались — ловится
        // тестом halogen_smiles_regression).
        if matches!(upper, 'C' | 'B') {
            if let Some(second) = self.peek() {
                let candidate = format!("{upper}{second}");
                if candidate == "Cl" || candidate == "Br" {
                    symbol = candidate;
                    self.pos += 1;
                }
            }
        }
        let z = crate::universal_chem::get_element_by_symbol(&symbol)
            .map(|e| e.z)
            .ok_or_else(|| format!("{}: неизвестный элемент «{symbol}»", self.pos))?;
        Ok(SmilesAtom {
            symbol,
            z,
            aromatic,
            charge: 0,
            h_count: 0,
            h_explicit: false,
            isotope: None,
            chiral: None,
        })
    }

    /// Bracket-атом: [nH], [NH4+], [13C@@H2-], [*].
    fn bracket_atom(&mut self) -> Result<SmilesAtom, String> {
        self.pos += 1; // '['
        let mut isotope = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                isotope.push(c);
                self.pos += 1;
            } else {
                break;
            }
        }
        let first = self
            .next()
            .filter(|c| c.is_ascii_alphabetic())
            .ok_or_else(|| format!("{}: пустой bracket-атом", self.pos))?;
        let aromatic = first.is_lowercase();
        let mut symbol = String::new();
        symbol.push(first.to_ascii_uppercase());
        if let Some(second) = self.peek() {
            if second.is_ascii_lowercase() && second != 'h' {
                let cand = format!("{symbol}{second}");
                if crate::universal_chem::get_element_by_symbol(&cand).is_some() {
                    symbol = cand;
                    self.pos += 1;
                }
            }
        }
        // Хиральность @ / @@
        let mut chiral = None;
        if self.peek() == Some('@') {
            self.pos += 1;
            if self.peek() == Some('@') {
                self.pos += 1;
                chiral = Some("@@".into());
            } else {
                chiral = Some("@".into());
            }
        }
        // H-счётчик
        let mut h_count = 0u8;
        let mut h_explicit = false;
        if self.peek() == Some('H') {
            self.pos += 1;
            h_explicit = true;
            let mut digits = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    digits.push(c);
                    self.pos += 1;
                } else {
                    break;
                }
            }
            h_count = if digits.is_empty() { 1 } else { digits.parse().unwrap_or(1) };
        }
        // Заряд: +, ++, +2, -, --, -2
        let mut charge = 0i8;
        match self.peek() {
            Some('+') => {
                self.pos += 1;
                charge = 1;
                while self.peek() == Some('+') {
                    charge += 1;
                    self.pos += 1;
                }
                if charge == 1 {
                    if let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            let d = (c as u8 - b'0') as i8;
                            if (1..=9).contains(&d) {
                                charge = d;
                                self.pos += 1;
                            }
                        }
                    }
                }
            }
            Some('-') => {
                self.pos += 1;
                charge = -1;
                while self.peek() == Some('-') {
                    charge -= 1;
                    self.pos += 1;
                }
                if charge == -1 {
                    if let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            let d = (c as u8 - b'0') as i8;
                            if (1..=9).contains(&d) {
                                charge = -d;
                                self.pos += 1;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        // :class — пропускаем
        if self.peek() == Some(':') {
            self.pos += 1;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
        if self.next() != Some(']') {
            return Err(format!("{}: bracket-атом не закрыт ']'", self.pos));
        }
        let z = if symbol == "*" {
            0
        } else {
            crate::universal_chem::get_element_by_symbol(&symbol)
                .map(|e| e.z)
                .ok_or_else(|| format!("{}: неизвестный элемент «{symbol}»", self.pos))?
        };
        Ok(SmilesAtom {
            symbol,
            z,
            aromatic,
            charge,
            h_count,
            h_explicit,
            isotope: if isotope.is_empty() { None } else { isotope.parse().ok() },
            chiral,
        })
    }

    fn push_atom(&mut self, atom: SmilesAtom) -> Result<(), String> {
        self.atoms.push(atom);
        let cur = self.atoms.len() - 1;
        if let Some(prev) = self.prev {
            let both_arom = self.atoms[prev].aromatic && self.atoms[cur].aromatic;
            let order = self.pending_order.unwrap_or(if both_arom {
                BondOrder::Aromatic
            } else {
                BondOrder::Single
            });
            self.bonds.push(SmilesBond {
                a: prev,
                b: cur,
                order,
            });
        }
        self.pending_order = None;
        self.prev = Some(cur);
        Ok(())
    }

    fn ring_closure(&mut self, label: u32) -> Result<(), String> {
        let cur = self
            .prev
            .ok_or_else(|| format!("{}: замыкание кольца {label} без атома", self.pos))?;
        if let Some((start, order)) = self.ring_open.remove(&label) {
            if start == cur {
                return Err(format!("кольцо {label} замыкается на том же атоме"));
            }
            let both_arom = self.atoms[start].aromatic && self.atoms[cur].aromatic;
            let order = order.or(self.pending_order).unwrap_or(if both_arom {
                BondOrder::Aromatic
            } else {
                BondOrder::Single
            });
            self.bonds.push(SmilesBond {
                a: start,
                b: cur,
                order,
            });
        } else {
            self.ring_open.insert(label, (cur, self.pending_order));
        }
        self.pending_order = None;
        Ok(())
    }
}

// ─── SSSR: кратчайший цикл через каждую связь ───────────────────────────

/// Кольцевое восприятие: для каждой связи ищем кратчайший путь между её
/// концами в графе БЕЗ этой связи (BFS); цикл = путь + связь.
/// Затем жадно выбираем циклы от коротких к длинным, пока не покроем все
/// кольцевые связи. Для нафталина даёт два 6-цикла (периметр отбрасывается),
/// для индола — 6+5.
fn sssr(g: &MoleculeGraph) -> Vec<Vec<usize>> {
    let n = g.atoms.len();
    if n < 3 {
        return Vec::new();
    }
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); n];
    for b in &g.bonds {
        adjacency[b.a].push(b.b);
        adjacency[b.b].push(b.a);
    }

    let mut candidates: Vec<Vec<usize>> = Vec::new();
    for b in &g.bonds {
        // BFS b.a → b.b без этой связи
        let path = shortest_path_excluding(b.a, b.b, &adjacency, b.a, b.b);
        if let Some(path) = path {
            if path.len() >= 2 {
                // цикл: [a, ..., b] (замыкание — сама связь b.a–b.b)
                candidates.push(path);
            }
        }
    }

    // Уникализация по отсортированному множеству атомов
    let mut unique: Vec<Vec<usize>> = Vec::new();
    for c in candidates {
        let mut sorted = c.clone();
        sorted.sort_unstable();
        if !unique.iter().any(|u| {
            let mut s = u.clone();
            s.sort_unstable();
            s == sorted
        }) {
            unique.push(c);
        }
    }
    unique.sort_by_key(|c| c.len());

    // Жадное покрытие кольцевых связей
    let mut covered: Vec<(usize, usize)> = Vec::new();
    let mut result = Vec::new();
    for cycle in &unique {
        let mut adds = false;
        for i in 0..cycle.len() {
            let x = cycle[i];
            let y = cycle[(i + 1) % cycle.len()];
            let (lo, hi) = if x < y { (x, y) } else { (y, x) };
            if !covered.contains(&(lo, hi)) {
                adds = true;
                covered.push((lo, hi));
            }
        }
        if adds {
            result.push(cycle.clone());
        }
    }
    result.retain(|c| c.len() >= 3 && c.len() <= 14);
    result
}

fn shortest_path_excluding(
    from: usize,
    to: usize,
    adjacency: &[Vec<usize>],
    excl_a: usize,
    excl_b: usize,
) -> Option<Vec<usize>> {
    let n = adjacency.len();
    let mut parent = vec![usize::MAX; n];
    let mut visited = vec![false; n];
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(from);
    visited[from] = true;
    while let Some(u) = queue.pop_front() {
        if u == to {
            break;
        }
        for &w in &adjacency[u] {
            // исключаем удалённую связь
            if (u == excl_a && w == excl_b) || (u == excl_b && w == excl_a) {
                continue;
            }
            if !visited[w] {
                visited[w] = true;
                parent[w] = u;
                queue.push_back(w);
            }
        }
    }
    if !visited[to] {
        return None;
    }
    let mut path = Vec::new();
    let mut cur = to;
    while cur != usize::MAX {
        path.push(cur);
        if cur == from {
            break;
        }
        cur = parent[cur];
    }
    path.reverse();
    Some(path)
}

// ─── Дескрипторы ────────────────────────────────────────────────────────

/// Фармакофорные дескрипторы уровня Липински.
#[derive(Debug, Clone)]
pub struct Descriptors {
    pub h_bond_donors: usize,
    pub h_bond_acceptors: usize,
    pub rotatable: usize,
    pub aromatic_rings: usize,
    pub rings: usize,
    pub heavy_atoms: usize,
    pub total_atoms: usize,
}

pub fn descriptors(g: &MoleculeGraph) -> Descriptors {
    let mut hbd = 0usize;
    let mut hba = 0usize;
    for (i, a) in g.atoms.iter().enumerate() {
        match a.symbol.as_str() {
            "N" | "O" => {
                // Конвенция Липински/RDKit: донор = АТОМ с ≥1 H
                // (NH2 — один донор, не два)
                if a.h_count > 0 && a.charge >= 0 {
                    hbd += 1;
                }
                // Акцептор: O всегда (2 неподелённые пары), N — без H
                let acceptor = match a.symbol.as_str() {
                    "O" => a.charge <= 0,
                    "N" => a.h_count == 0 && a.charge <= 0,
                    _ => false,
                };
                if acceptor && g.degree(i) < 4 {
                    hba += 1;
                }
            }
            _ => {}
        }
    }
    let aromatic_rings = g
        .rings
        .iter()
        .filter(|r| r.len() <= 6 && r.iter().all(|&i| g.atoms[i].aromatic))
        .count();
    Descriptors {
        h_bond_donors: hbd,
        h_bond_acceptors: hba,
        rotatable: g.rotatable_bonds().len(),
        aromatic_rings,
        rings: g.rings.len(),
        heavy_atoms: g.atoms.len(),
        total_atoms: g.atoms.iter().map(|a| 1 + a.h_count as usize).sum(),
    }
}

/// Оценка logP атомной схемой (калибровка по опорным молекулам, ±1 logP).
///
/// Типы: ароматический/алифатический C, H, гидроксильный/эфирный/карбонильный O,
/// ароматический/алифатический N, S, галогены.
/// Опорные точки (допуск ±1): метанол −0.77, этанол −0.31, ацетон −0.24,
/// бензол 2.13, толуол 2.73, фенол 1.46, пиридин 0.65. Схема грубая —
/// её роль: липофильный флаг Липински, не QSPR-прецизионность.
pub fn logp_estimate(g: &MoleculeGraph) -> f64 {
    // Вклады: грубая атомная схема ±1 logP (для липофильного флага Липински,
    // не для QSPR-прецизионности; амидные/эфирные контексты не разделены)
    const C_AROM: f64 = 0.26;
    const C_ALIPH: f64 = 0.30; // без гетеро-соседей
    const C_HET: f64 = -0.10; // алифатический C при O/N
    const C_CARBONYL: f64 = -0.30; // карбонильный/тиокарбонильный C
    const H_ANY: f64 = 0.10;
    const O_HYDROXYL: f64 = -0.60;
    const O_ETHER: f64 = -0.80;
    const O_CARBONYL: f64 = -0.55;
    const N_AROM: f64 = -1.10;
    const N_ALIPH: f64 = -0.80;
    const N_AMIDE: f64 = -0.30; // амидный/мочевинный N при C=O
    const N_IMINE: f64 = -0.60; // иминный =N
    const S_ANY: f64 = 0.30;
    const HALOGEN: f64 = 0.40;

    let mut sum = 0.0f64;
    for (i, a) in g.atoms.iter().enumerate() {
        let nbrs = g.neighbors(i);
        let is_carbonyl_c = a.symbol == "C"
            && nbrs.iter().any(|(j, o)| {
                *o == BondOrder::Double
                    && matches!(g.atoms[*j].symbol.as_str(), "O" | "S")
            });
        let has_het_neighbor = !a.aromatic
            && nbrs
                .iter()
                .any(|(j, _)| matches!(g.atoms[*j].symbol.as_str(), "O" | "N" | "S"));
        let is_carbonyl_o = a.symbol == "O" && nbrs.iter().any(|(_, o)| *o == BondOrder::Double);
        let is_hydroxyl_o = a.symbol == "O" && a.h_count > 0;
        match a.symbol.as_str() {
            "C" => {
                sum += if a.aromatic {
                    C_AROM
                } else if is_carbonyl_c {
                    C_CARBONYL
                } else if has_het_neighbor {
                    C_HET
                } else {
                    C_ALIPH
                };
            }
            "O" => {
                sum += if is_carbonyl_o {
                    O_CARBONYL
                } else if is_hydroxyl_o {
                    O_HYDROXYL
                } else {
                    O_ETHER
                };
            }
            "N" => {
                // Амидный N (при карбонильном C) и иминный (=N) мягче алифатического
                let adjacent_carbonyl = nbrs.iter().any(|(j, _)| {
                    let jj = *j;
                    if g.atoms[jj].symbol != "C" {
                        return false;
                    }
                    g.neighbors(jj).iter().any(|(k, o)| {
                        *o == BondOrder::Double && matches!(g.atoms[*k].symbol.as_str(), "O" | "S")
                    })
                });
                let is_imine = nbrs.iter().any(|(_, o)| *o == BondOrder::Double);
                sum += if a.aromatic {
                    N_AROM
                } else if adjacent_carbonyl {
                    N_AMIDE
                } else if is_imine {
                    N_IMINE
                } else {
                    N_ALIPH
                };
            }
            "S" => sum += S_ANY,
            "F" | "Cl" | "Br" | "I" => sum += HALOGEN,
            _ => {}
        }
        if a.symbol != "H" {
            sum += a.h_count as f64 * H_ANY;
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halogen_smiles_regression() {
        // Регрессия контура B (2026-10-06): кандидат двухбуквенного
        // элемента собирался капслоком («CL»≠«Cl») — хлор и бром в
        // SMILES никогда не разбирались. Проверяем разбор + формулы.
        let cl = parse_smiles("Clc1ccccc1").unwrap();
        assert_eq!(cl.hill_formula(), "C6H5Cl");
        assert_eq!(cl.atoms.iter().filter(|a| a.symbol == "Cl").count(), 1);
        let br = parse_smiles("Brc1ccccc1").unwrap();
        assert_eq!(br.hill_formula(), "C6H5Br");
        // смешанный случай: C-Cl связь и «cl»-ароматика не путаются
        let two = parse_smiles("ClCCBr").unwrap();
        assert_eq!(two.hill_formula(), "C2H4BrCl");
        assert_eq!(two.bonds.len(), 3);
    }

    #[test]
    fn parse_water() {
        let g = parse_smiles("O").unwrap();
        assert_eq!(g.atoms.len(), 1);
        assert_eq!(g.atoms[0].h_count, 2);
        assert_eq!(g.hill_formula(), "H2O");
    }

    #[test]
    fn parse_ethanol() {
        let g = parse_smiles("CCO").unwrap();
        assert_eq!(g.atoms.len(), 3);
        assert_eq!(g.bonds.len(), 2);
        assert_eq!(g.hill_formula(), "C2H6O");
        assert_eq!(g.total_charge(), 0);
    }

    #[test]
    fn parse_benzene_ring() {
        let g = parse_smiles("c1ccccc1").unwrap();
        assert_eq!(g.atoms.len(), 6);
        assert_eq!(g.bonds.len(), 6);
        assert_eq!(g.rings.len(), 1, "SSSR: один 6-цикл");
        assert_eq!(g.rings[0].len(), 6);
        assert!(g.atoms.iter().all(|a| a.aromatic));
        // Ароматические связи = σ-порядок 1 → у каждого c остаётся 1 H
        assert_eq!(g.hill_formula(), "C6H6");
        assert_eq!(descriptors(&g).aromatic_rings, 1);
    }

    #[test]
    fn parse_pyridine_no_implicit_h_on_arom_n() {
        let g = parse_smiles("c1ccncc1").unwrap();
        assert_eq!(g.hill_formula(), "C5H5N", "пиридин: n без H");
        let n_idx = g.atoms.iter().position(|a| a.symbol == "N").unwrap();
        assert_eq!(g.atoms[n_idx].h_count, 0);
    }

    #[test]
    fn parse_pyrrole_explicit_nh() {
        let g = parse_smiles("c1cc[nH]c1").unwrap();
        assert_eq!(g.hill_formula(), "C4H5N");
        let n_idx = g.atoms.iter().position(|a| a.symbol == "N").unwrap();
        assert_eq!(g.atoms[n_idx].h_count, 1);
    }

    #[test]
    fn parse_aspirin() {
        let g = parse_smiles("CC(=O)Oc1ccccc1C(=O)O").unwrap();
        assert_eq!(g.atoms.len(), 13, "9 C + 4 O тяжёлых атомов");
        assert_eq!(g.hill_formula(), "C9H8O4");
        assert_eq!(g.rings.len(), 1);
        let d = descriptors(&g);
        assert_eq!(d.h_bond_donors, 1, "COOH-гидроксил");
        assert!(d.rotatable >= 2, "ацетильная и кислотная вращаемые");
        assert_eq!(d.aromatic_rings, 1);
    }

    #[test]
    fn parse_caffeine() {
        // PubChem (кекulé-форма): 14 тяжёлых атомов, 2 кольца (6+5)
        let g = parse_smiles("CN1C=NC2=C1C(=O)N(C)C(=O)N2C").unwrap();
        assert_eq!(g.atoms.len(), 14);
        assert_eq!(g.hill_formula(), "C8H10N4O2");
        assert_eq!(g.rings.len(), 2, "пуриновый скелет: 6+5");
        assert_eq!(descriptors(&g).h_bond_acceptors, 6, "4N + 2 карбонильных O");
    }

    #[test]
    fn parse_bracket_atoms() {
        let g = parse_smiles("[NH4+]").unwrap();
        assert_eq!(g.atoms[0].charge, 1);
        assert_eq!(g.atoms[0].h_count, 4);
        assert_eq!(g.total_charge(), 1);

        let g = parse_smiles("C[NH3+]").unwrap();
        assert_eq!(g.hill_formula(), "CH6N+");
        assert_eq!(g.total_charge(), 1);

        let g = parse_smiles("[13C]").unwrap();
        assert_eq!(g.atoms[0].isotope, Some(13));

        let g = parse_smiles("C[C@@H](N)C(=O)O").unwrap(); // L-аланин
        assert_eq!(g.atoms[1].chiral.as_deref(), Some("@@"));
        assert_eq!(g.hill_formula(), "C3H7NO2");
    }

    #[test]
    fn parse_charge_and_anion() {
        let g = parse_smiles("CC(=O)[O-]").unwrap(); // ацетат
        assert_eq!(g.total_charge(), -1);
        assert_eq!(g.hill_formula(), "C2H3O2-");
    }

    #[test]
    fn parse_naphthalene_fused() {
        let g = parse_smiles("c1ccc2ccccc2c1").unwrap();
        assert_eq!(g.rings.len(), 2, "нафталин: два сплавленных 6-цикла");
        assert!(g.rings.iter().all(|r| r.len() == 6));
        assert_eq!(g.hill_formula(), "C10H8");
    }

    #[test]
    fn parse_indole_fused65() {
        let g = parse_smiles("c1ccc2c(c1)[nH]cc2").unwrap();
        assert_eq!(g.rings.len(), 2, "индол: 6+5");
        assert_eq!(g.hill_formula(), "C8H7N");
    }

    #[test]
    fn parse_disconnected() {
        let g = parse_smiles("O.CCO").unwrap();
        assert_eq!(g.components, 2);
        assert_eq!(g.atoms.len(), 4);
    }

    #[test]
    fn parse_errors() {
        assert!(parse_smiles("c1ccccc").is_err(), "незамкнутое кольцо");
        assert!(parse_smiles("C((").is_err(), "лишняя скобка");
        assert!(parse_smiles("[Xx]").is_err(), "неизвестный элемент");
        assert!(parse_smiles("C%1").is_err(), "%nn без цифр");
    }

    #[test]
    fn logp_calibration_targets() {
        let cases = [
            ("CCO", -0.31),     // этанол
            ("CO", -0.77),      // метанол
            ("CC(=O)C", -0.24), // ацетон
            ("c1ccccc1", 2.13), // бензол
            ("Cc1ccccc1", 2.73), // толуол
            ("Oc1ccccc1", 1.46), // фенол
            ("c1ccncc1", 0.65), // пиридин
        ];
        for (smi, target) in cases {
            let g = parse_smiles(smi).unwrap();
            let est = logp_estimate(&g);
            assert!(
                (est - target).abs() < 1.0,
                "logP({smi}) = {est:.2}, цель {target}"
            );
        }
    }

    #[test]
    fn lipinski_caffeine() {
        let g = parse_smiles("CN1C=NC2=C1C(=O)N(C)C(=O)N2C").unwrap();
        let d = descriptors(&g);
        assert_eq!(d.h_bond_donors, 0);
        assert_eq!(d.h_bond_acceptors, 6);
        assert_eq!(d.rotatable, 0, "N-CH3 терминальные — не вращаемые");
    }

    #[test]
    fn parse_dopamine_and_nicotine() {
        // Дофамин — нейромедиатор
        let g = parse_smiles("NCCc1ccc(O)c(O)c1").unwrap();
        assert_eq!(g.hill_formula(), "C8H11NO2");
        assert_eq!(g.rings.len(), 1);
        assert_eq!(descriptors(&g).h_bond_donors, 3, "NH2 + 2×OH = 3 атома-донора");
        // Никотин — пирролидин + пиридин
        let g = parse_smiles("CN1CCCC1c2cccnc2").unwrap();
        assert_eq!(g.hill_formula(), "C10H14N2");
        assert_eq!(g.rings.len(), 2);
    }
}
