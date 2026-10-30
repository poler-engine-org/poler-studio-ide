//! SDF/MOL-парсер v0.77.0 (контур B): библиотеки лигандов для скрининга.
//!
//! * V2000-молблоки: counts-line (фиксированные колонки I3), блоки атомов
//!   (F10.4 × 3, элемент, заряд-код) и связей (порядок), `M  CHG`,
//!   много-записность через `$$$$`.
//! * Водороды SDF поглощаются в `h_count` тяжёлых атомов, НО их
//!   координаты сохраняются (`explicit_h`) — конформер SDF приоритетнее
//!   укладчика (канон ступени).
//! * `.smi`-файлы: `SMILES имя` на строку.
//! * Писатель V2000 — для round-trip и генерации фикстур.
//! * V3000 — честно не поддерживается (задокументировано как следующий шаг).
//!
//! Дисциплина как в `pdb.rs`: разбор по байтовым срезам фиксированной
//! ширины, без String на атом до необходимости (символ элемента — String
//! молекулярного графа, как в `smiles.rs`).

use super::smiles::{
    infer_h_counts, parse_smiles, BondOrder, MoleculeGraph, SmilesAtom, SmilesBond,
};

// ─── Запись библиотеки ──────────────────────────────────────────────────

/// Одна молекула из SDF: граф (тяжёлые атомы) + конформер файла.
#[derive(Debug, Clone)]
pub struct SdfMol {
    /// Имя (строка 1 молблока; для .smi — имя из строки).
    pub name: String,
    /// Молекулярный граф: только тяжёлые атомы, H — в `h_count`.
    pub graph: MoleculeGraph,
    /// Координаты тяжёлых атомов, Å (порядок `graph.atoms`).
    pub heavy_pos: Vec<[f64; 3]>,
    /// Явные H из файла: `explicit_h[i]` — координаты H атома i
    /// (пусто → H будут построены конусом по гибридизации).
    pub explicit_h: Vec<Vec<[f64; 3]>>,
    /// Число явных H, поглощённых в h_count (для отчётов).
    pub n_h_absorbed: usize,
}

/// Запись библиотеки унифицированная: SDF-молекула или SMILES-строка.
#[derive(Debug, Clone)]
pub enum LibraryEntry {
    /// Из SDF: конформер файла приоритетнее укладчика.
    Sdf(SdfMol),
    /// Из .smi или списка: имя + SMILES.
    Smiles { name: String, smiles: String },
}

// ─── Вспомогательные: колонки фиксированной ширины ──────────────────────

/// Срез колонок [start, start+len) с падением до короткой строки.
fn col(line: &str, start: usize, len: usize) -> &str {
    let b = line.as_bytes();
    if start >= b.len() {
        return "";
    }
    let end = (start + len).min(b.len());
    let s = &line[start..end];
    s.trim_matches(|c: char| c == ' ' || c == '\t')
}

/// Число из колонки (int или float).
fn col_num<T: std::str::FromStr>(line: &str, start: usize, len: usize) -> Option<T> {
    col(line, start, len).parse::<T>().ok()
}

/// Заряд V2000-кода атома (колонки 36..39): 0 — нейтральный,
/// 1..3 = +3..+1, 4..6 = −3..−1, 7 — радикал (игнорируем, честно).
fn charge_code(c: i32) -> i8 {
    match c {
        1 => 3,
        2 => 2,
        3 => 1,
        4 => -3,
        5 => -2,
        6 => -1,
        _ => 0,
    }
}

/// Порядок связи V2000 → BondOrder (4 — ароматическая).
fn bond_order(t: i32) -> Option<BondOrder> {
    Some(match t {
        1 => BondOrder::Single,
        2 => BondOrder::Double,
        3 => BondOrder::Triple,
        4 => BondOrder::Aromatic,
        _ => return None,
    })
}

/// Нормализация символа элемента: «CL» → «Cl», «c» → «C».
fn norm_symbol(sym: &str) -> String {
    if sym.is_empty() {
        return String::new();
    }
    let mut c = sym.chars();
    let first = c.next().unwrap_or('C');
    let rest: String = c.collect();
    format!("{}{}", first.to_uppercase(), rest.to_lowercase())
}

// ─── Парсер одного молблока ─────────────────────────────────────────────

/// Разобрать один V2000-молблок (до и включая `M  END`).
pub fn parse_mol_block(block: &str) -> Result<SdfMol, String> {
    let lines: Vec<&str> = block.lines().map(|l| l.trim_end_matches('\r')).collect();
    if lines.len() < 4 {
        return Err("молблок короче заголовка (нужно 4 строки)".into());
    }
    let name = lines[0].trim().to_string();
    let counts = lines[3];
    if counts.len() < 6 {
        return Err(format!("counts-line слишком короткая: «{counts}»"));
    }
    // V3000 — честный отказ (по маркировке counts-line или M V30-блоку)
    if counts.contains("V3000")
        || (lines.len() > 4 && lines[4].trim_start().starts_with("M  V30"))
    {
        return Err("V3000 не поддерживается (следующий шаг ступени)".into());
    }
    let n_atoms: usize = col_num(counts, 0, 3)
        .ok_or_else(|| format!("число атомов не читается: «{}»", col(counts, 0, 3)))?;
    let n_bonds: usize = col_num(counts, 3, 3)
        .ok_or_else(|| format!("число связей не читается: «{}»", col(counts, 3, 3)))?;
    if n_atoms == 0 {
        return Err("молекула без атомов".into());
    }
    if n_atoms > 512 || n_bonds > 1024 {
        return Err(format!("слишком большая запись: {n_atoms} атомов / {n_bonds} связей"));
    }
    if lines.len() < 4 + n_atoms + n_bonds {
        return Err(format!(
            "молблок обрезан: заявлено {n_atoms}+{n_bonds}, есть {} строк",
            lines.len() - 4
        ));
    }

    // Сырые атомы: (символ, координаты, заряд-код)
    struct RawAtom {
        sym: String,
        pos: [f64; 3],
        charge: i8,
    }
    let mut raw: Vec<RawAtom> = Vec::with_capacity(n_atoms);
    for li in lines.iter().take(4 + n_atoms).skip(4) {
        if li.trim_start().starts_with("M  ") || li.trim() == "M  END" {
            return Err("молблок обрезан: блок атомов не начался".into());
        }
        let x: f64 = col_num(li, 0, 10).ok_or_else(|| format!("x не читается: «{li}»"))?;
        let y: f64 = col_num(li, 10, 10).ok_or_else(|| format!("y не читается: «{li}»"))?;
        let z: f64 = col_num(li, 20, 10).ok_or_else(|| format!("z не читается: «{li}»"))?;
        let sym = norm_symbol(col(li, 31, 3));
        if sym.is_empty() {
            return Err(format!("пустой символ элемента: «{li}»"));
        }
        let cc: i32 = col_num(li, 36, 3).unwrap_or(0);
        raw.push(RawAtom {
            sym,
            pos: [x, y, z],
            charge: charge_code(cc),
        });
    }

    // Сырые связи: (a, b, порядок) — индексы 1-based
    let mut raw_bonds: Vec<(usize, usize, i32)> = Vec::with_capacity(n_bonds);
    for li in lines
        .iter()
        .take(4 + n_atoms + n_bonds)
        .skip(4 + n_atoms)
    {
        let a: usize = col_num(li, 0, 3).ok_or_else(|| format!("связь a не читается: «{li}»"))?;
        let b: usize = col_num(li, 3, 3).ok_or_else(|| format!("связь b не читается: «{li}»"))?;
        let t: i32 = col_num(li, 6, 3).unwrap_or(1);
        if a == 0 || b == 0 || a > n_atoms || b > n_atoms {
            return Err(format!("связь вне диапазона атомов: {a}–{b}"));
        }
        if bond_order(t).is_none() {
            return Err(format!("неизвестный порядок связи: {t}"));
        }
        raw_bonds.push((a - 1, b - 1, t));
    }

    // M  CHG — формальные заряды (приоритетнее заряд-кодов).
    // Реальные писатели (PubChem/OpenBabel/RDKit) пишут тройки через пробелы,
    // строго-спековые фиксированные колонки на них расходятся — разбор по токенам.
    let mut charges: Vec<i8> = raw.iter().map(|a| a.charge).collect();
    for li in lines.iter().skip(4 + n_atoms + n_bonds) {
        let l = li.trim_end();
        if l == "M  END" {
            break;
        }
        if l.starts_with("M  CHG") {
            let toks: Vec<&str> = l[6..].split_whitespace().collect();
            if let Some((&n, rest)) = toks.split_first() {
                if let Ok(n) = n.parse::<usize>() {
                    let pairs = rest.len() / 2;
                    for k in 0..n.min(pairs) {
                        let ai: usize = rest[2 * k].parse().unwrap_or(0);
                        let ch: i32 = rest[2 * k + 1].parse().unwrap_or(0);
                        if ai >= 1 && ai <= n_atoms && (-9..=9).contains(&ch) {
                            charges[ai - 1] = ch as i8;
                        }
                    }
                }
            }
        }
    }

    // Поглощение H: тяжёлые — в граф, H — в h_count родителя с координатами
    let is_h = |s: &str| s == "H" || s == "D";
    let mut heavy_of: Vec<Option<usize>> = vec![None; n_atoms];
    let mut atoms: Vec<SmilesAtom> = Vec::with_capacity(n_atoms);
    let mut heavy_pos: Vec<[f64; 3]> = Vec::with_capacity(n_atoms);
    let mut explicit_h: Vec<Vec<[f64; 3]>> = Vec::new();
    let mut n_h_absorbed = 0usize;
    for (i, a) in raw.iter().enumerate() {
        if is_h(&a.sym) {
            continue;
        }
        heavy_of[i] = Some(atoms.len());
        atoms.push(SmilesAtom {
            symbol: a.sym.clone(),
            z: 0, // заполним ниже по символу
            aromatic: false,
            charge: charges[i],
            h_count: 0,
            // h_explicit поднимется при поглощении первого явного H;
            // атомы без явных H в файле получат неявные H по валентности
            h_explicit: false,
            isotope: None,
            chiral: None,
        });
        heavy_pos.push(a.pos);
        explicit_h.push(Vec::new());
    }
    // z по символу: из таблицы geom3d нет общего z — берём из smiles-парсера
    // (z нужен только для ConfNode; заполним через symbol_z)
    for a in atoms.iter_mut() {
        a.z = symbol_z(&a.symbol);
    }

    // Связи: тяжёлые⇄тяжёлые → граф; тяжёлый⇄H → поглощение
    let mut bonds: Vec<SmilesBond> = Vec::with_capacity(n_bonds);
    for &(a, b, t) in &raw_bonds {
        let ord = bond_order(t).unwrap();
        let (ha, hb) = (heavy_of[a], heavy_of[b]);
        match (ha, hb) {
            (Some(x), Some(y)) => {
                bonds.push(SmilesBond {
                    a: x,
                    b: y,
                    order: ord,
                });
            }
            (Some(x), None) | (None, Some(x)) => {
                // H-связь: поглощаем в родителя (координату H сохраним);
                // явные H — авторитет, дальше infer не трогает
                let h_idx = if ha.is_none() { a } else { b };
                explicit_h[x].push(raw[h_idx].pos);
                atoms[x].h_count = atoms[x].h_count.saturating_add(1);
                atoms[x].h_explicit = true;
                n_h_absorbed += 1;
            }
            (None, None) => {
                return Err("связь H–H без тяжёлого атома (водородная молекула?)".into());
            }
        }
    }
    // Ароматические атомы (type 4): оба конца связи
    for b in bonds.iter() {
        if b.order == BondOrder::Aromatic {
            atoms[b.a].aromatic = true;
            atoms[b.b].aromatic = true;
        }
    }

    let mut graph = MoleculeGraph {
        atoms,
        bonds,
        components: 0,
        rings: Vec::new(),
    };
    // H-вывод для атомов БЕЗ явных H из файла (неявный стиль SDF);
    // атомы с поглощёнными H помечены h_explicit — их h_count авторитетен
    infer_h_counts(&mut graph);
    super::smiles::compute_rings(&mut graph);
    graph.components = count_components(&graph);

    if graph.atoms.is_empty() {
        return Err(format!("«{name}»: только водороды, тяжёлых атомов нет"));
    }

    Ok(SdfMol {
        name,
        graph,
        heavy_pos,
        explicit_h,
        n_h_absorbed,
    })
}

/// Атомный номер по символу (органический субсет + основные).
fn symbol_z(sym: &str) -> u8 {
    match sym {
        "H" => 1,
        "B" => 5,
        "C" => 6,
        "N" => 7,
        "O" => 8,
        "F" => 9,
        "Na" => 11,
        "Mg" => 12,
        "Si" => 14,
        "P" => 15,
        "S" => 16,
        "Cl" => 17,
        "K" => 19,
        "Ca" => 20,
        "Fe" => 26,
        "Zn" => 30,
        "Se" => 34,
        "Br" => 35,
        "I" => 53,
        _ => 0,
    }
}

/// Число связных компонент графа (BFS).
fn count_components(g: &MoleculeGraph) -> usize {
    let n = g.atoms.len();
    let mut seen = vec![false; n];
    let mut comps = 0usize;
    for s in 0..n {
        if seen[s] {
            continue;
        }
        comps += 1;
        seen[s] = true;
        let mut stack = vec![s];
        while let Some(u) = stack.pop() {
            for (w, _) in g.neighbors(u) {
                if !seen[w] {
                    seen[w] = true;
                    stack.push(w);
                }
            }
        }
    }
    comps
}

// ─── Много-записный SDF и .smi ──────────────────────────────────────────

/// Разобрать SDF-файл (много записей, `$$$$`-разделители).
/// Ошибочные записи пропускаются с диагностикой в `skipped`.
pub fn parse_sdf(text: &str) -> Result<(Vec<SdfMol>, Vec<String>), String> {
    let mut mols = Vec::new();
    let mut skipped = Vec::new();
    let mut block = String::new();
    let mut n_record = 0usize;
    for line in text.lines() {
        let l = line.trim_end_matches('\r');
        if l.trim() == "$$$$" {
            n_record += 1;
            if block.trim().is_empty() {
                block.clear();
                continue;
            }
            match parse_mol_block(&block) {
                Ok(m) => mols.push(m),
                Err(e) => skipped.push(format!("запись #{n_record}: {e}")),
            }
            block.clear();
        } else {
            block.push_str(l);
            block.push('\n');
        }
    }
    // Хвост без $$$$ — тоже запись
    if !block.trim().is_empty() {
        n_record += 1;
        match parse_mol_block(&block) {
            Ok(m) => mols.push(m),
            Err(e) => skipped.push(format!("запись #{n_record}: {e}")),
        }
    }
    if n_record == 0 {
        return Err("SDF пуст: ни одной записи".into());
    }
    Ok((mols, skipped))
}

/// Прочитать SDF-файл с диска.
pub fn read_sdf(path: &str) -> Result<(Vec<SdfMol>, Vec<String>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("не прочитать {path}: {e}"))?;
    parse_sdf(&text)
}

/// Разобрать .smi-файл: строки `SMILES имя`.
/// Пустые строки и `#`-комментарии пропускаются.
pub fn parse_smi_file(text: &str) -> Result<Vec<LibraryEntry>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let mut it = l.split_whitespace();
        let smiles = match it.next() {
            Some(s) => s,
            None => continue,
        };
        let name: String = it.collect::<Vec<&str>>().join(" ");
        if name.is_empty() {
            return Err(format!("строка {}: нет имени после SMILES «{smiles}»", i + 1));
        }
        // валидация SMILES сразу — отказ на этапе разбора библиотеки
        match parse_smiles(smiles) {
            Ok(_) => out.push(LibraryEntry::Smiles {
                name,
                smiles: smiles.to_string(),
            }),
            Err(e) => {
                return Err(format!(
                    "строка {}: SMILES «{smiles}» не разбирается: {e}",
                    i + 1
                ))
            }
        }
    }
    if out.is_empty() {
        return Err(".smi-файл пуст: ни одной строки «SMILES имя»".into());
    }
    Ok(out)
}

/// Прочитать .smi-файл с диска.
pub fn read_smi(path: &str) -> Result<Vec<LibraryEntry>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("не прочитать {path}: {e}"))?;
    parse_smi_file(&text)
}

/// Универсальный вход: .sdf/.mol или .smi по расширению.
pub fn read_library(path: &str) -> Result<(Vec<LibraryEntry>, Vec<String>), String> {
    let lower = path.to_lowercase();
    if lower.ends_with(".smi") {
        Ok((read_smi(path)?, Vec::new()))
    } else if lower.ends_with(".sdf") || lower.ends_with(".mol") {
        let (mols, skipped) = read_sdf(path)?;
        Ok((
            mols.into_iter().map(LibraryEntry::Sdf).collect(),
            skipped,
        ))
    } else {
        Err(format!(
            "библиотека «{path}»: ожидается .sdf/.mol или .smi"
        ))
    }
}

// ─── Писатель V2000 ─────────────────────────────────────────────────────

/// Записать молблок V2000: тяжёлые атомы графа + H-узлы конформера
/// (координаты конформера пишутся как есть; заряды — `M  CHG`).
pub fn mol_block(name: &str, g: &MoleculeGraph, conf: &super::geom3d::Conformer) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(256 + 64 * conf.nodes.len());
    // Заголовок: титул / программа / комментарий / counts
    let title: String = name.chars().take(80).collect();
    let _ = writeln!(s, "{title}");
    let _ = writeln!(s, "  poler-engine v0.77.0");
    let _ = writeln!(s);
    let n_atoms = conf.nodes.len();
    let n_bonds = conf.bonds.len();
    let _ = writeln!(s, "{n_atoms:0>3}{n_bonds:0>3}  0  0  0  0  0  0  0  0999 V2000");
    // Атомы: узлы конформера (тяжёлые + H) — карта узел→строка
    for (ni, node) in conf.nodes.iter().enumerate() {
        let p = conf.positions[ni];
        let _ = writeln!(
            s,
            "{:>10.4}{:>10.4}{:>10.4} {:<3} 0  0  0  0  0  0  0  0  0  0  0  0",
            p[0],
            p[1],
            p[2],
            node.symbol
        );
    }
    // Связи конформера (тяжёлые + H-узлы) — порядок 1/2/3/4
    for b in conf.bonds.iter() {
        let t = match b.order {
            BondOrder::Single => 1,
            BondOrder::Double => 2,
            BondOrder::Triple => 3,
            BondOrder::Quadruple => 0,
            BondOrder::Aromatic => 4,
        };
        let a1 = b.a + 1;
        let b1 = b.b + 1;
        let _ = writeln!(s, "{a1:0>3}{b1:0>3}{t:0>3}  0  0  0  0");
    }
    // Формальные заряды тяжёлых атомов: строка узла = heavy_map[ai]+1.
    // Формат как в реальных файлах (PubChem/OpenBabel): поля через пробел,
    // правое выравнивание — «M  CHG  1   5   1». ГРАБЛЯ (B1): склейка
    // полей {:0>3}{:0>3} давала «005001» одной лексемой, а отрицательный
    // заряд паддингился нулём («0-1») — парсер по токенам терял заряд,
    // катионный актив становился нейтральным. Закрыто round-trip-тестом
    // с зарядом (round_trip_charged).
    let charged: Vec<(usize, i32)> = g
        .atoms
        .iter()
        .enumerate()
        .filter(|(_, a)| a.charge != 0)
        .map(|(ai, a)| (conf.heavy_map[ai] + 1, a.charge as i32))
        .collect();
    if !charged.is_empty() {
        let mut line = format!("M  CHG{:0>3}", charged.len());
        for (a, c) in charged {
            let _ = write!(line, " {a:>3} {c:>3}");
        }
        let _ = writeln!(s, "{}", line);
    }
    let _ = writeln!(s, "M  END");
    s
}

/// SDF-текст из набора молекул (разделители `$$$$`).
pub fn sdf_text(records: &[(String, MoleculeGraph, super::geom3d::Conformer)]) -> String {
    let mut out = String::new();
    for (name, g, conf) in records {
        out.push_str(&mol_block(name, g, conf));
        out.push_str("$$$$\n");
    }
    out
}

// ─── Тесты ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Молблок этанола с явными H (ручная запись, фиксированные колонки).
    fn ethanol_block() -> String {
        let mut b = String::new();
        b.push_str("ethanol\n  poler-engine\n\n  9  8  0  0  0  0  0  0  0  0999 V2000\n");
        // атомы: C, C, O + 6 H
        b.push_str("    0.0000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0\n");
        b.push_str("    1.5000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0\n");
        b.push_str("    2.1000    1.2000    0.0000 O   0  0  0  0  0  0  0  0  0  0  0  0\n");
        for (x, y, z) in [
            (-0.5, 0.8, 0.5),
            (-0.5, -0.8, 0.5),
            (-0.5, 0.0, -1.0),
            (2.0, -0.5, 0.9),
            (2.0, -0.5, -0.9),
            (3.0, 1.2, 0.0),
        ] {
            b.push_str(&format!(
                "{x:10.4}{y:10.4}{z:10.4} H   0  0  0  0  0  0  0  0  0  0  0  0\n"
            ));
        }
        // связи: C-C, C-O, 3×C-H(1), 2×C-H(2), O-H
        for (a, bb, t) in [(1, 2, 1), (2, 3, 1), (1, 4, 1), (1, 5, 1), (1, 6, 1), (2, 7, 1), (2, 8, 1), (3, 9, 1)] {
            b.push_str(&format!("{a:0>3}{bb:0>3}{t:0>3}  0  0  0  0\n"));
        }
        b.push_str("M  END\n");
        b
    }

    #[test]
    fn sdf_counts_and_atoms() {
        let m = parse_mol_block(&ethanol_block()).unwrap();
        assert_eq!(m.name, "ethanol");
        assert_eq!(m.graph.atoms.len(), 3); // тяжёлые: C, C, O
        assert_eq!(m.graph.bonds.len(), 2); // C-C, C-O
        assert_eq!(m.heavy_pos[0], [0.0, 0.0, 0.0]);
        assert_eq!(m.heavy_pos[2], [2.1, 1.2, 0.0]);
        assert_eq!(m.n_h_absorbed, 6);
        assert_eq!(m.graph.atoms[0].h_count, 3); // CH3
        assert_eq!(m.graph.atoms[1].h_count, 2); // CH2
        assert_eq!(m.graph.atoms[2].h_count, 1); // OH
        assert_eq!(m.explicit_h[2].len(), 1);
        assert_eq!(m.explicit_h[2][0], [3.0, 1.2, 0.0]);
    }

    #[test]
    fn sdf_hill_formula() {
        let m = parse_mol_block(&ethanol_block()).unwrap();
        assert_eq!(m.graph.hill_formula(), "C2H6O");
    }

    #[test]
    fn sdf_charge_code_and_m_chg() {
        // ацетат: заряд-код 6 = −1 на атоме O
        let mut b = String::new();
        b.push_str("acetate\n  x\n\n  3  2  0  0  0  0  0  0  0  0999 V2000\n");
        b.push_str("    0.0000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0\n");
        b.push_str("    1.3000    0.0000    0.0000 O   0  0  0  0  0  0  0  0  0  0  0  0\n");
        b.push_str("   -0.7000    0.9000    0.0000 O   0  0  0  0  0  0  0  0  0  0  0  0\n");
        b.push_str("  1  2  2  0  0  0  0\n");
        b.push_str("  1  3  1  0  0  0  0\n");
        b.push_str("M  CHG  1   3  -1\n");
        b.push_str("M  END\n");
        let m = parse_mol_block(&b).unwrap();
        assert_eq!(m.graph.atoms[2].charge, -1); // M CHG приоритетнее кода
        // теперь заряд-код без M CHG: код 6 = −1, колонки 36..39
        let mut b2 = b.replace("M  CHG  1   3  -1\n", "");
        b2 = b2.replace(
            "   -0.7000    0.9000    0.0000 O   0  0  0  0  0  0  0  0  0  0  0  0",
            "   -0.7000    0.9000    0.0000 O   0  6  0  0  0  0  0  0  0  0  0  0",
        );
        let m2 = parse_mol_block(&b2).unwrap();
        assert_eq!(m2.graph.atoms[2].charge, -1); // код 6 = −1
        // фрагмент — формиат HCOO⁻: C получает 1 H по валентности (4 − 3)
        assert_eq!(m.graph.atoms[0].h_count, 1);
    }

    #[test]
    fn sdf_implicit_h_style() {
        // SDF без явных H (2D-стиль): толуол
        let mut b = String::new();
        b.push_str("toluene\n  x\n\n  7  7  0  0  0  0  0  0  0  0999 V2000\n");
        let coords = [
            (0.0000, 1.4000, 0.0),
            (1.2124, 0.7000, 0.0),
            (1.2124, -0.7000, 0.0),
            (0.0000, -1.4000, 0.0),
            (-1.2124, -0.7000, 0.0),
            (-1.2124, 0.7000, 0.0),
            (2.5000, 1.5000, 0.0),
        ];
        for (x, y, z) in coords {
            b.push_str(&format!(
                "{x:10.4}{y:10.4}{z:10.4} C   0  0  0  0  0  0  0  0  0  0  0  0\n"
            ));
        }
        for (a, bb, t) in [(1, 2, 2), (2, 3, 1), (3, 4, 2), (4, 5, 1), (5, 6, 2), (6, 1, 1), (2, 7, 1)] {
            b.push_str(&format!("{a:0>3}{bb:0>3}{t:0>3}  0  0  0  0\n"));
        }
        b.push_str("M  END\n");
        let m = parse_mol_block(&b).unwrap();
        assert_eq!(m.graph.atoms[0].h_count, 1); // CH ароматический
        assert_eq!(m.graph.atoms[6].h_count, 3); // CH3
        assert_eq!(m.graph.hill_formula(), "C7H8");
        assert_eq!(m.graph.rings.len(), 1); // SSSR: бензольное кольцо
        // C–CH₃ — терминальная связь (у CH₃ степень 1): по правилам движка
        // НЕ вращаемая (вырожденное кручение) — как и в SMILES-пути
        assert_eq!(m.graph.rotatable_bonds().len(), 0);
    }

    #[test]
    fn sdf_multi_record() {
        let mut text = String::new();
        text.push_str(&ethanol_block());
        text.push_str("$$$$\n");
        // вторая запись: метан
        text.push_str("methane\n  x\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n");
        text.push_str("    0.0000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0\n");
        text.push_str("M  END\n");
        text.push_str("$$$$\n");
        let (mols, skipped) = parse_sdf(&text).unwrap();
        assert_eq!(mols.len(), 2);
        assert!(skipped.is_empty());
        assert_eq!(mols[0].name, "ethanol");
        assert_eq!(mols[1].name, "methane");
        assert_eq!(mols[1].graph.atoms[0].h_count, 4); // infer: 4 H
    }

    #[test]
    fn sdf_bad_record_isolated_but_reported() {
        let mut text = String::new();
        text.push_str(&ethanol_block());
        text.push_str("$$$$\n");
        text.push_str("broken\n  x\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n");
        // нет атомных строк → обрезан
        text.push_str("M  END\n");
        text.push_str("$$$$\n");
        let (mols, skipped) = parse_sdf(&text).unwrap();
        assert_eq!(mols.len(), 1);
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].contains("обрезан"));
    }

    #[test]
    fn sdf_v3000_honest_reject() {
        let b = "name\n  x\n\n  0  0  0  0  0  0  0  0  0  0999 V3000\nM  V30 BEGIN CTAB\n";
        let e = parse_mol_block(b).unwrap_err();
        assert!(e.contains("V3000"), "ожидался честный отказ V3000: {e}");
    }

    #[test]
    fn sdf_truncated_counts_error() {
        let e = parse_mol_block("x\ny\n\nonly").unwrap_err();
        assert!(e.contains("counts-line"));
        let e2 = parse_mol_block("x\ny\n\n  5  0  0  0  0  0  0  0  0  0999 V2000\n").unwrap_err();
        assert!(e2.contains("обрезан"));
    }

    #[test]
    fn sdf_aromatic_bond_type4() {
        // бензол type-4
        let mut b = String::new();
        b.push_str("benzene\n  x\n\n  6  6  0  0  0  0  0  0  0  0999 V2000\n");
        for i in 0..6 {
            let ang = i as f64 * std::f64::consts::TAU / 6.0;
            b.push_str(&format!(
                "{:10.4}{:10.4}    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0\n",
                1.4 * ang.cos(),
                1.4 * ang.sin()
            ));
        }
        for i in 0..6 {
            let a = i + 1;
            let bb = if i == 5 { 1 } else { i + 2 };
            b.push_str(&format!("{a:0>3}{bb:0>3}  4  0  0  0  0\n"));
        }
        b.push_str("M  END\n");
        let m = parse_mol_block(&b).unwrap();
        assert!(m.graph.atoms.iter().all(|a| a.aromatic));
        assert!(m.graph.bonds.iter().all(|b| b.order == BondOrder::Aromatic));
        // ароматические связи не вращаемые
        assert_eq!(m.graph.rotatable_bonds().len(), 0);
    }

    #[test]
    fn sdf_h_h_bond_rejected() {
        let mut b = String::new();
        b.push_str("H2\n  x\n\n  2  1  0  0  0  0  0  0  0  0999 V2000\n");
        b.push_str("    0.0000    0.0000    0.0000 H   0  0  0  0  0  0  0  0  0  0  0  0\n");
        b.push_str("    0.7000    0.0000    0.0000 H   0  0  0  0  0  0  0  0  0  0  0  0\n");
        b.push_str("  1  2  1  0  0  0  0\n");
        b.push_str("M  END\n");
        let e = parse_mol_block(&b).unwrap_err();
        assert!(e.contains("H–H"));
    }

    #[test]
    fn smi_file_parsing() {
        let text = "# комментарий\nNC(=N)c1ccccc1 бензамидин\nCCO этанол\n";
        let entries = parse_smi_file(text).unwrap();
        assert_eq!(entries.len(), 2);
        match &entries[0] {
            LibraryEntry::Smiles { name, smiles } => {
                assert_eq!(smiles, "NC(=N)c1ccccc1");
                assert_eq!(name, "бензамидин");
            }
            _ => panic!("ожидался Smiles"),
        }
        let e = parse_smi_file("толькоИмяБезПробела").unwrap_err();
        assert!(e.contains("нет имени"));
    }

    #[test]
    fn round_trip_write_parse() {
        // этанол: SMILES → граф → конформер → SDF → парс — позиции совпадают
        let g = parse_smiles("CCO").unwrap();
        let conf = super::super::geom3d::embed(&g).unwrap();
        let text = mol_block("ethanol-rt", &g, &conf);
        let m = parse_mol_block(&text).unwrap();
        assert_eq!(m.graph.hill_formula(), g.hill_formula());
        assert_eq!(m.graph.bonds.len(), g.bonds.len());
        // тяжёлые позиции — как записаны (формат %.4f)
        for (ai, ni) in g.atoms.iter().enumerate().map(|(i, _)| (i, conf.heavy_map[i])) {
            let written = conf.positions[ni];
            let reread = m.heavy_pos[ai];
            for k in 0..3 {
                assert!(
                    (written[k] - reread[k]).abs() < 5e-4,
                    "позиция {ai}[{k}]: {written:?} vs {reread:?}"
                );
            }
        }
        // явные H: поглощены ровно в h_count
        let n_h: usize = m.explicit_h.iter().map(|v| v.len()).sum();
        assert_eq!(n_h, 6);
        assert_eq!(m.graph.hill_formula(), "C2H6O");
        // и много-записный SDF целиком
        let text2 = sdf_text(&[("a".into(), g.clone(), conf.clone())]);
        let (mols, skipped) = parse_sdf(&text2).unwrap();
        assert_eq!(mols.len(), 1);
        assert!(skipped.is_empty());
    }

    #[test]
    fn norm_symbol_case() {
        assert_eq!(norm_symbol("CL"), "Cl");
        assert_eq!(norm_symbol("c"), "C");
        assert_eq!(norm_symbol("Br"), "Br");
    }

    /// ГРАБЛЯ B1 (2026-10-06): писатель M CHG склеивал поля без пробелов
    /// («M  CHG001 005001») и паддингил нулём отрицательные заряды («0-1»)
    /// — парсер по токенам терял заряд, протонированный актив читался
    /// нейтральным и проваливал enrichment. Заряд обязан переживать
    /// полный round-trip: граф → конформер → SDF → парс.
    #[test]
    fn round_trip_charged() {
        for (smi, want_formula, want_q) in [
            ("NC(=[NH2+])c1ccccc1", "C7H9N2+", 1), // амидиний-катион
            ("CC(=O)[O-]", "C2H3O2-", -1),          // ацетат-анион
            ("[O-][N+](=O)c1ccccc1", "C6H5NO2", 0), // нитробензол: сумма 0
        ] {
            let g = parse_smiles(smi).unwrap();
            let conf = super::super::geom3d::embed(&g).unwrap();
            let text = mol_block("rt", &g, &conf);
            let m = parse_mol_block(&text).unwrap();
            assert_eq!(
                m.graph.hill_formula(),
                want_formula,
                "формула заряда не пережила round-trip для {smi}"
            );
            assert_eq!(
                m.graph.total_charge(),
                want_q,
                "заряд не пережил round-trip для {smi}"
            );
            assert_eq!(m.graph.bonds.len(), g.bonds.len());
            // формулы H тоже совпадают (H катиона не потерялись)
            assert_eq!(
                m.n_h_absorbed,
                g.atoms.iter().map(|a| a.h_count as usize).sum::<usize>()
            );
        }
    }

    #[test]
    fn benzamidine_block_from_smiles() {
        // канон структуры активного 3PTB: бензамидин с явными H из укладчика
        let g = parse_smiles("NC(=N)c1ccccc1").unwrap();
        let conf = super::super::geom3d::embed(&g).unwrap();
        let text = mol_block("BAM", &g, &conf);
        let m = parse_mol_block(&text).unwrap();
        assert_eq!(m.graph.hill_formula(), "C7H8N2");
        assert_eq!(m.graph.atoms.len(), 9); // 7 C + 2 N
        assert_eq!(m.n_h_absorbed, 8);
        // связи: 7 (граф) — ароматическое кольцо Kekulé
        assert_eq!(m.graph.bonds.len(), g.bonds.len());
    }
}

