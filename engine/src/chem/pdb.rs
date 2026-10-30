//! PDB-макромолекулы: парсер и структурный анализ (лестница «атом → молекула → белок»).
//!
//! # Ступень 4: белки как мишени докинга
//!
//! * **Парсер PDB** (fixed-width колонки): ATOM/HETATM с выводом элемента
//!   из колонок 77–78 или инференсом из имени атома (привило PDB: имя с
//!   пробела в 13-й колонке — однобуквенный элемент; 4-символьные имена
//!   с 13-й — элемент = первая буква). MODEL → берётся первая модель,
//!   altLoc ≠ A отбрасывается. HELIX/SHEET → вторичная структура,
//!   SSBOND → дисульфидные мостики, TITLE/REMARK 2 → паспорт.
//! * **mmCIF** (`_atom_site` loop): минимальный читатель STAR-токенов
//!   с кавычками (`'…''`), первым pdbx_PDB_model_num и auth_*-координатами.
//! * **Нулевые аллокации на атом**: имя/остаток/элемент — inline-массивы
//!   байтов, одна строка файла на вход.
//! * **Классификация**: 20 аминокислот + MSE, воды, ионы, лиганды;
//!   классы остатков (гидрофобный/полярный/заряженный/ароматический).
//! * **Заряды**: самосогласованная объединённая (united) схема —
//!   NET-заряды остатков точны (Arg/Lys +1, Asp/Glu −1, концы ±1,
//!   ионы по таблице), лёгкая полярность боковых цепей.
//! * **Реконструкция H-доноров**: амидный N−H по биссектрисе CA/C′,
//!   Lys-NZ тригональная пирамида, гуанидиний Arg в плоскости,
//!   кольцевые N-H (His/Trp) от центроида кольца, OH/SH от соседа.
//!
//! Все координаты — ангстремы, как в PDB.

use crate::geo::tree27::{BBox3, Tree27};

// ─── Структуры данных ───────────────────────────────────────────────────

/// Атом PDB: нулевые аллокации, поля фиксированной ширины.
#[derive(Debug, Clone, Copy)]
pub struct PdbAtom {
    pub serial: u32,
    /// Имя атома («CA», «OD1», «C1»…) — 4 байта, правые пробелы значимые.
    pub name: [u8; 4],
    /// Имя остатка («ALA», «BEN»…) — 3 байта.
    pub res_name: [u8; 3],
    /// Идентификатор цепи (ASCII-байт; 0 = пустой).
    pub chain: u8,
    /// Номер остатка.
    pub res_seq: i32,
    /// Код вставки (ASCII-байт; 0 = пустой).
    pub icode: u8,
    /// Координаты, Å.
    pub pos: [f64; 3],
    /// B-фактор (температурный фактор).
    pub b_factor: f32,
    /// Элемент («C», «N», «Fe»…) — 2 байта.
    pub element: [u8; 2],
    /// Формальный заряд из колонок 79–80.
    pub formal_charge: i8,
    /// HETATM (true) или ATOM (false).
    pub het: bool,
}

impl PdbAtom {
    /// Имя атома как &str (обрезано с обеих сторон — PDB выравнивает
    /// имена вправо от 13-й колонки, ведущий пробел значим только
    /// для вывода).
    pub fn name_str(&self) -> &str {
        let start = self
            .name
            .iter()
            .position(|&b| b != b' ')
            .unwrap_or(self.name.len());
        let end = self
            .name
            .iter()
            .rposition(|&b| b != b' ')
            .map_or(start, |p| p + 1);
        std::str::from_utf8(&self.name[start..end]).unwrap_or("?")
    }

    /// Имя остатка как &str.
    pub fn res_str(&self) -> &str {
        let n = self.res_name.iter().rposition(|&b| b != b' ').map_or(0, |p| p + 1);
        std::str::from_utf8(&self.res_name[..n]).unwrap_or("?")
    }

    /// Элемент как &str.
    pub fn elem_str(&self) -> &str {
        let n = self.element.iter().rposition(|&b| b != b' ').map_or(0, |p| p + 1);
        std::str::from_utf8(&self.element[..n]).unwrap_or("?")
    }

    /// Символ цепи для печати.
    pub fn chain_char(&self) -> char {
        if self.chain == 0 || self.chain == b' ' { '_' } else { self.chain as char }
    }
}

/// Тип остатка.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResKind {
    /// Стандартная аминокислота (вкл. MSE → метионин-подобная).
    Protein,
    /// Вода (HOH/WAT/DOD).
    Water,
    /// Ион металла/анион (CA, MG, ZN, CL, SO4…).
    Ion,
    /// Всё остальное — лиганд/кофактор.
    Ligand,
}

/// Вторичная структура остатка.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsKind {
    Helix,
    Sheet,
    Coil,
}

/// Остаток: непрерывный срез атомов.
#[derive(Debug, Clone, Copy)]
pub struct ResidueInfo {
    pub chain: u8,
    pub seq: i32,
    pub icode: u8,
    pub name: [u8; 3],
    pub kind: ResKind,
    pub ss: SsKind,
    /// Диапазон атомов [start, end) в `MacroMol::atoms`.
    pub atoms: (u32, u32),
}

impl ResidueInfo {
    /// Имя остатка как &str.
    pub fn name_str(&self) -> &str {
        let n = self.name.iter().rposition(|&b| b != b' ').map_or(0, |p| p + 1);
        std::str::from_utf8(&self.name[..n]).unwrap_or("?")
    }
}

/// Класс остатка для раскраски/карты кармана.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResClass {
    Hydrophobic,
    Polar,
    ChargedPos,
    ChargedNeg,
    Aromatic,
    Cysteine,
    Glycine,
    Other,
}

impl ResClass {
    /// Цвет класса (RGB) — как в фармакофорных картах: жёлтый гидрофобный,
    /// голубой полярный, красный/синий заряженные, циан ароматический.
    pub fn color(self) -> (u8, u8, u8) {
        match self {
            ResClass::Hydrophobic => (212, 181, 60),
            ResClass::Polar => (96, 155, 232),
            ResClass::ChargedPos => (235, 101, 84),
            ResClass::ChargedNeg => (84, 137, 235),
            ResClass::Aromatic => (140, 214, 198),
            ResClass::Cysteine => (240, 165, 45),
            ResClass::Glycine => (190, 190, 190),
            ResClass::Other => (200, 120, 210),
        }
    }

    pub fn name_ru(self) -> &'static str {
        match self {
            ResClass::Hydrophobic => "гидрофобный",
            ResClass::Polar => "полярный",
            ResClass::ChargedPos => "заряженный +",
            ResClass::ChargedNeg => "заряженный −",
            ResClass::Aromatic => "ароматический",
            ResClass::Cysteine => "цистеин",
            ResClass::Glycine => "глицин",
            ResClass::Other => "прочий",
        }
    }
}

/// Макромолекула из PDB/mmCIF.
#[derive(Debug, Clone, Default)]
pub struct MacroMol {
    pub atoms: Vec<PdbAtom>,
    pub residues: Vec<ResidueInfo>,
    /// Заголовок (TITLE, склеенный).
    pub title: String,
    /// Разрешение (REMARK 2), Å.
    pub resolution: Option<f32>,
    /// Число моделей в файле.
    pub n_models: usize,
    /// HELIX-диапазоны: (chain, seq от, seq до).
    pub helix_ranges: Vec<(u8, i32, i32)>,
    /// SHEET-диапазоны.
    pub sheet_ranges: Vec<(u8, i32, i32)>,
    /// Дисульфиды: пары индексов атомов (SG, SG).
    pub ssbonds: Vec<(usize, usize)>,
    /// Неразобранные SSBOND-записи: (chain1, seq1, chain2, seq2).
    pub ssbond_pairs: Vec<(u8, i32, u8, i32)>,
}

// ─── Таблицы ────────────────────────────────────────────────────────────

/// 20 стандартных аминокислот + MSE (селено-метионин считается белковым).
pub fn is_standard_aa(name: &[u8; 3]) -> bool {
    matches!(
        name,
        b"ALA" | b"ARG" | b"ASN" | b"ASP" | b"CYS" | b"GLN" | b"GLU" | b"GLY" | b"HIS"
            | b"ILE" | b"LEU" | b"LYS" | b"MET" | b"PHE" | b"PRO" | b"SER" | b"THR"
            | b"TRP" | b"TYR" | b"VAL" | b"MSE"
    )
}

fn is_water(name: &[u8; 3]) -> bool {
    matches!(name, b"HOH" | b"WAT" | b"DOD" | b"TIP" | b"SOL" | b"H2O")
}

/// Ионы: (имя, заряд) — металлы, галогениды, сульфат/фосфат.
/// Имена — 3 байта с пробелами-заполнителями, как в PDB.
fn ion_charge(name: &[u8; 3]) -> Option<f64> {
    Some(match name {
        b"CA " | b"MG " | b"ZN " | b"NI " | b"CO " | b"CU " | b"CD " | b"HG " | b"PT " | b"PD " => 2.0,
        b"MN " => 2.0,
        b"FE " => 2.0, //Fe2+/Fe3+ неоднозначность — берём 2+
        b"NA " | b"K  " | b"LI " | b"CS " | b"RB " | b"AG " | b"NH4" => 1.0,
        b"CL " | b"BR " | b"I  " | b"F  " => -1.0,
        b"SO4" | b"PO4" => -2.0,
        _ => return None,
    })
}

/// Двухбуквенные элементы для инференса из HETATM-имён.
const TWO_LETTER: [&[u8]; 24] = [
    b"FE", b"ZN", b"MG", b"MN", b"CA", b"NA", b"CL", b"BR", b"SE", b"CU", b"NI", b"CO",
    b"CD", b"HG", b"PT", b"AU", b"AG", b"CR", b"LI", b"SI", b"AL", b"SB", b"SN", b"PB",
];

/// Однобуквенный код аминокислоты (для последовательности).
pub fn aa_one_letter(name: &[u8; 3]) -> Option<u8> {
    Some(match name {
        b"ALA" => b'A',
        b"ARG" => b'R',
        b"ASN" => b'N',
        b"ASP" => b'D',
        b"CYS" => b'C',
        b"GLN" => b'Q',
        b"GLU" => b'E',
        b"GLY" => b'G',
        b"HIS" => b'H',
        b"ILE" => b'I',
        b"LEU" => b'L',
        b"LYS" => b'K',
        b"MET" => b'M',
        b"MSE" => b'M',
        b"PHE" => b'F',
        b"PRO" => b'P',
        b"SER" => b'S',
        b"THR" => b'T',
        b"TRP" => b'W',
        b"TYR" => b'Y',
        b"VAL" => b'V',
        _ => return None,
    })
}

/// Класс остатка.
pub fn residue_class(name: &[u8; 3], kind: ResKind) -> ResClass {
    if kind != ResKind::Protein {
        return ResClass::Other;
    }
    match name {
        b"ALA" | b"VAL" | b"LEU" | b"ILE" | b"MET" | b"MSE" | b"PRO" => ResClass::Hydrophobic,
        b"PHE" | b"TRP" | b"TYR" => ResClass::Aromatic,
        b"ASP" | b"GLU" => ResClass::ChargedNeg,
        b"LYS" | b"ARG" | b"HIS" => ResClass::ChargedPos,
        b"SER" | b"THR" | b"ASN" | b"GLN" => ResClass::Polar,
        b"CYS" => ResClass::Cysteine,
        b"GLY" => ResClass::Glycine,
        _ => ResClass::Other,
    }
}

// ─── Парсер PDB (fixed-width) ───────────────────────────────────────────

/// Прочитать число из колонок [from, to) (байты).
fn col_f64(b: &[u8], from: usize, to: usize) -> Option<f64> {
    if b.len() <= from {
        return None;
    }
    let s = std::str::from_utf8(&b[from..to.min(b.len())]).ok()?;
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        t.parse::<f64>().ok()
    }
}

fn col_i32(b: &[u8], from: usize, to: usize) -> Option<i32> {
    if b.len() <= from {
        return None;
    }
    let s = std::str::from_utf8(&b[from..to.min(b.len())]).ok()?;
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        t.parse::<i32>().ok()
    }
}

/// Заряд из колонок 79–80: «2+», «1-», « +»…
fn col_charge(b: &[u8]) -> i8 {
    if b.len() < 79 {
        return 0;
    }
    let mut mag: i8 = 1;
    let mut sign: i8 = 1;
    for &c in &b[78..b.len().min(80)] {
        if c.is_ascii_digit() {
            mag = (c - b'0') as i8;
        } else if c == b'-' {
            sign = -1;
        }
    }
    mag * sign
}

/// Инференс элемента из имени атома (когда колонки 77–78 пусты).
///
/// PDB-конвенция: имена ≤3 символов начинаются с 14-й колонки (байт 13),
/// 4-символьные — с 13-й (байт 12, элемент = первая буква). Для атомов
/// стандартных остатков «CA» — это углерод (альфа), не кальций.
fn infer_element(het: bool, res_is_aa: bool, name: &[u8; 4]) -> [u8; 2] {
    // обрезаем пробелы; ищем первую букву
    let start = name.iter().position(|&b| b != b' ').unwrap_or(4);
    let mut first_alpha = None;
    for &c in &name[start..] {
        if c.is_ascii_alphabetic() {
            first_alpha = Some(c);
            break;
        }
    }
    let first = match first_alpha {
        Some(c) => c,
        None => return *b"C ",
    };
    // двухбуквенный кандидат (только для гетеро/не-белковых контекстов)
    if (het || !res_is_aa) && name.len() >= start + 2 {
        let c2 = name[start + 1];
        if c2.is_ascii_alphabetic() && TWO_LETTER.iter().any(|e| e[0] == first && e[1] == c2) {
            return [first, c2 + 32]; // «FE» → «Fe»
        }
    }
    [first.to_ascii_uppercase(), b' ']
}

/// Нормализация имени остатка: PDB выравнивает вправо (« CA» → «CA »).
fn norm_res_name(raw: [u8; 3]) -> [u8; 3] {
    let start = raw.iter().position(|&b| b != b' ').unwrap_or(3);
    let mut out = [b' '; 3];
    for i in 0..(3 - start) {
        out[i] = raw[start + i];
    }
    out
}

/// Нормализация элемента из колонок 77–78 («FE» → «Fe», « C» → «C»).
fn norm_element(raw: &[u8]) -> [u8; 2] {
    let start = raw.iter().position(|&b| b != b' ').unwrap_or(raw.len());
    if start >= raw.len() {
        return *b"C ";
    }
    let first = raw[start].to_ascii_uppercase();
    let second = raw
        .get(start + 1)
        .copied()
        .filter(|&c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .unwrap_or(b' ');
    [first, second]
}

/// Разобрать PDB-текст (одна или много моделей — берётся первая).
pub fn parse_pdb(text: &str) -> Result<MacroMol, String> {
    let mut mm = MacroMol::default();
    let mut in_first_model = true;
    let mut seen_model = false;

    for line in text.lines() {
        let b = line.as_bytes();
        if b.len() < 6 {
            continue;
        }
        let rec = &b[..6];
        match rec {
            b"ATOM  " | b"HETATM" => {
                if !in_first_model || b.len() < 54 {
                    continue;
                }
                // altLoc: берём только пусто/A
                if let Some(&alt) = b.get(16) {
                    if alt != b' ' && alt != b'A' {
                        continue;
                    }
                }
                let serial = col_i32(b, 6, 11).unwrap_or(0).max(0) as u32;
                let mut name = [b' '; 4];
                for i in 0..4 {
                    if let Some(&c) = b.get(12 + i) {
                        name[i] = c;
                    }
                }
                let mut res_name = [b' '; 3];
                for i in 0..3 {
                    if let Some(&c) = b.get(17 + i) {
                        res_name[i] = c;
                    }
                }
                let res_name = norm_res_name(res_name);
                let chain = b.get(21).copied().filter(|&c| c != b' ').unwrap_or(0);
                let res_seq = col_i32(b, 22, 26).unwrap_or(0);
                let icode = b.get(26).copied().filter(|&c| c != b' ').unwrap_or(0);
                let x = col_f64(b, 30, 38).ok_or("ATOM: нет X")?;
                let y = col_f64(b, 38, 46).ok_or("ATOM: нет Y")?;
                let z = col_f64(b, 46, 54).ok_or("ATOM: нет Z")?;
                let b_factor = col_f64(b, 60, 66).unwrap_or(0.0) as f32;
                let het = rec == b"HETATM";
                let element = if b.len() >= 78 {
                    let raw = &b[76..78];
                    if raw.iter().any(|&c| c != b' ') {
                        norm_element(raw)
                    } else {
                        infer_element(het, is_standard_aa(&res_name), &name)
                    }
                } else {
                    infer_element(het, is_standard_aa(&res_name), &name)
                };
                let formal_charge = if b.len() >= 79 { col_charge(b) } else { 0 };
                mm.atoms.push(PdbAtom {
                    serial,
                    name,
                    res_name,
                    chain,
                    res_seq,
                    icode,
                    pos: [x, y, z],
                    b_factor,
                    element,
                    formal_charge,
                    het,
                });
            }
            b"MODEL " => {
                mm.n_models += 1;
                if seen_model {
                    in_first_model = false; // только первая модель
                }
                seen_model = true;
            }
            b"ENDMDL" => {
                in_first_model = false;
            }
            b"HELIX " => {
                // initChainID col 20 (байт 19), initSeqNum 22–25, endSeqNum 34–37
                let ch = b.get(19).copied().filter(|&c| c != b' ').unwrap_or(0);
                if let (Some(a), Some(z)) = (col_i32(b, 21, 25), col_i32(b, 33, 37)) {
                    mm.helix_ranges.push((ch, a, z));
                }
            }
            b"SHEET " => {
                let ch = b.get(21).copied().filter(|&c| c != b' ').unwrap_or(0);
                if let (Some(a), Some(z)) = (col_i32(b, 22, 26), col_i32(b, 33, 37)) {
                    mm.sheet_ranges.push((ch, a, z));
                }
            }
            b"SSBOND" => {
                // chain1 байт 15, seq1 17–21; chain2 байт 29, seq2 31–35
                let c1 = b.get(15).copied().filter(|&c| c != b' ').unwrap_or(0);
                let c2 = b.get(29).copied().filter(|&c| c != b' ').unwrap_or(0);
                if let (Some(s1), Some(s2)) = (col_i32(b, 17, 21), col_i32(b, 31, 35)) {
                    mm.ssbond_pairs.push((c1, s1, c2, s2));
                }
            }
            b"TITLE " => {
                let t = std::str::from_utf8(&b[10.min(b.len())..]).unwrap_or("").trim();
                if !t.is_empty() {
                    if !mm.title.is_empty() {
                        mm.title.push(' ');
                    }
                    mm.title.push_str(t);
                }
            }
            b"REMARK" => {
                if mm.resolution.is_none() && line.contains("RESOLUTION.") {
                    if let Some(v) = line
                        .split("RESOLUTION.")
                        .nth(1)
                        .and_then(|s| s.trim().split_whitespace().next())
                        .and_then(|s| s.parse::<f32>().ok())
                    {
                        if v > 0.0 && v < 20.0 {
                            mm.resolution = Some(v);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if mm.atoms.is_empty() {
        return Err("PDB: ни одного атома ATOM/HETATM".into());
    }
    if mm.n_models == 0 {
        mm.n_models = 1;
    }
    group_residues(&mut mm);
    resolve_ssbonds(&mut mm);
    Ok(mm)
}

// ─── mmCIF (минимальный _atom_site) ─────────────────────────────────────

/// STAR-токен: слово, 'кавычка' (внутри '' — экранированная кавычка) или "двойная".
fn cif_tokens(line: &str) -> Vec<String> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        if b[i] == b'#' {
            break; // комментарий до конца строки
        }
        if b[i] == b'\'' || b[i] == b'"' {
            let quote = b[i];
            let mut s = String::new();
            i += 1;
            while i < b.len() {
                if b[i] == quote {
                    // удвоенная кавычка = экранированная
                    if b.get(i + 1) == Some(&quote) {
                        s.push(quote as char);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                s.push(b[i] as char);
                i += 1;
            }
            out.push(s);
        } else {
            let start = i;
            while i < b.len() && b[i] != b' ' && b[i] != b'\t' {
                i += 1;
            }
            out.push(String::from_utf8_lossy(&b[start..i]).into_owned());
        }
    }
    out
}

fn blank3(s: &str) -> [u8; 3] {
    let b = s.as_bytes();
    let mut out = [b' '; 3];
    for i in 0..b.len().min(3) {
        out[i] = b[i];
    }
    out
}

fn blank4(s: &str) -> [u8; 4] {
    let b = s.as_bytes();
    let mut out = [b' '; 4];
    for i in 0..b.len().min(4) {
        out[i] = b[i];
    }
    out
}

/// Разобрать mmCIF: loop_ _atom_site.… Требуемые колонки: Cartn_x/y/z,
/// comp_id; остальные — по мере наличия. Первая модель pdbx_PDB_model_num.
pub fn parse_mmcif(text: &str) -> Result<MacroMol, String> {
    let mut mm = MacroMol::default();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    let mut found_loop = false;

    while i < lines.len() {
        let t = lines[i].trim();
        if t == "loop_" {
            // заголовки колонок
            let mut cols: Vec<String> = Vec::new();
            let mut j = i + 1;
            while j < lines.len() {
                let tj = lines[j].trim();
                if tj.starts_with('_') {
                    // _atom_site.field (возможно с суффиксом #)
                    let name = tj.split_whitespace().next().unwrap_or("");
                    cols.push(name.to_ascii_lowercase());
                    j += 1;
                } else {
                    break;
                }
            }
            let is_atom_site = cols.iter().any(|c| c == "_atom_site.cartn_x");
            if is_atom_site {
                found_loop = true;
                let idx = |name: &str| cols.iter().position(|c| c.as_str() == name);
                let i_group = idx("_atom_site.group_pdb");
                let i_id = idx("_atom_site.id");
                let i_elem = idx("_atom_site.type_symbol");
                let i_atom = idx("_atom_site.label_atom_id");
                let i_alt = idx("_atom_site.label_alt_id");
                let i_comp = idx("_atom_site.label_comp_id")
                    .ok_or("mmCIF: нет label_comp_id")?;
                let i_asym = idx("_atom_site.auth_asym_id")
                    .or_else(|| idx("_atom_site.label_asym_id"));
                let i_seq = idx("_atom_site.auth_seq_id")
                    .or_else(|| idx("_atom_site.label_seq_id"));
                let i_ins = idx("_atom_site.pdbx_pdb_ins_code");
                let i_x = idx("_atom_site.cartn_x").ok_or("mmCIF: нет Cartn_x")?;
                let i_y = idx("_atom_site.cartn_y").ok_or("mmCIF: нет Cartn_y")?;
                let i_z = idx("_atom_site.cartn_z").ok_or("mmCIF: нет Cartn_z")?;
                let i_b = idx("_atom_site.b_iso_or_equiv");
                let i_model = idx("_atom_site.pdbx_pdb_model_num");
                let i_charge = idx("_atom_site.pdbx_formal_charge");

                // строки данных до следующего loop_/_/#
                let mut k = j;
                let mut first_model_done = false;
                while k < lines.len() {
                    let tk = lines[k].trim();
                    if tk.is_empty() || tk.starts_with('#') {
                        if tk.starts_with('#') {
                            k += 1;
                            continue;
                        }
                        k += 1;
                        continue;
                    }
                    if tk == "loop_" || tk.starts_with('_') || tk.starts_with("data_") {
                        break;
                    }
                    let toks = cif_tokens(lines[k]);
                    let get = |ix: Option<usize>| -> String {
                        ix.and_then(|x| toks.get(x)).map(|s| s.as_str()).unwrap_or("").to_string()
                    };
                    let model: u32 = get(i_model).parse().unwrap_or(1);
                    if first_model_done && model > 1 {
                        k += 1;
                        continue;
                    }
                    if model == 1 {
                        first_model_done = true;
                    }
                    let alt = get(i_alt);
                    if !alt.is_empty() && alt != "." && alt != "?" && alt != "A" {
                        k += 1;
                        continue;
                    }
                    let group = get(i_group);
                    let het = group == "HETATM";
                    let comp = get(Some(i_comp));
                    let res_name = norm_res_name(blank3(&comp));
                    let name_raw = get(i_atom);
                    let name = blank4(&name_raw);
                    let elem_raw = get(i_elem);
                    let element = if elem_raw.is_empty() || elem_raw == "." || elem_raw == "?" {
                        infer_element(het, is_standard_aa(&res_name), &name)
                    } else {
                        norm_element(elem_raw.as_bytes())
                    };
                    let chain = {
                        let c = get(i_asym);
                        let cb = c.as_bytes();
                        if cb.is_empty() || c == "." || c == "?" {
                            0
                        } else {
                            cb[0]
                        }
                    };
                    let seq: i32 = {
                        let s = get(i_seq);
                        if s.is_empty() || s == "." || s == "?" { 0 } else { s.parse().unwrap_or(0) }
                    };
                    let icode = {
                        let c = get(i_ins);
                        let cb = c.as_bytes();
                        if c.is_empty() || c == "." || c == "?" || cb.is_empty() {
                            0
                        } else {
                            cb[0]
                        }
                    };
                    let x: f64 = get(Some(i_x)).parse().map_err(|_| "mmCIF: bad X")?;
                    let y: f64 = get(Some(i_y)).parse().map_err(|_| "mmCIF: bad Y")?;
                    let z: f64 = get(Some(i_z)).parse().map_err(|_| "mmCIF: bad Z")?;
                    let b_fac: f32 = get(i_b).parse().unwrap_or(0.0);
                    let formal = {
                        let c = get(i_charge);
                        parse_cif_charge(&c)
                    };
                    mm.atoms.push(PdbAtom {
                        serial: get(i_id).parse().unwrap_or(0),
                        name,
                        res_name,
                        chain,
                        res_seq: seq,
                        icode,
                        pos: [x, y, z],
                        b_factor: b_fac,
                        element,
                        formal_charge: formal,
                        het,
                    });
                    k += 1;
                }
                i = k;
                continue;
            }
        }
        i += 1;
    }

    if !found_loop {
        return Err("mmCIF: блок _atom_site не найден".into());
    }
    if mm.atoms.is_empty() {
        return Err("mmCIF: ни одного атома в _atom_site".into());
    }
    mm.n_models = 1;
    group_residues(&mut mm);
    Ok(mm)
}

fn parse_cif_charge(s: &str) -> i8 {
    // форматы: «2+», «-1», «+»
    let b = s.as_bytes();
    let mut mag: i8 = 1;
    let mut sign: i8 = 1;
    for &c in b {
        if c.is_ascii_digit() {
            mag = (c - b'0') as i8;
        } else if c == b'-' {
            sign = -1;
        }
    }
    mag * sign
}

// ─── Группировка остатков ───────────────────────────────────────────────

fn group_residues(mm: &mut MacroMol) {
    mm.residues.clear();
    let mut start = 0usize;
    for i in 1..=mm.atoms.len() {
        let new_group = i == mm.atoms.len() || {
            let a = &mm.atoms[i];
            let p = &mm.atoms[i - 1];
            a.het != p.het
                || a.chain != p.chain
                || a.res_seq != p.res_seq
                || a.icode != p.icode
                || a.res_name != p.res_name
        };
        if new_group {
            let p = &mm.atoms[start];
            let kind = if is_standard_aa(&p.res_name) {
                ResKind::Protein
            } else if is_water(&p.res_name) {
                ResKind::Water
            } else if ion_charge(&p.res_name).is_some() {
                ResKind::Ion
            } else {
                ResKind::Ligand
            };
            let ss = {
                let mut ss = SsKind::Coil;
                for &(ch, a0, a1) in &mm.helix_ranges {
                    if ch == p.chain && p.res_seq >= a0 && p.res_seq <= a1 {
                        ss = SsKind::Helix;
                    }
                }
                for &(ch, a0, a1) in &mm.sheet_ranges {
                    if ch == p.chain && p.res_seq >= a0 && p.res_seq <= a1 {
                        ss = SsKind::Sheet;
                    }
                }
                ss
            };
            mm.residues.push(ResidueInfo {
                chain: p.chain,
                seq: p.res_seq,
                icode: p.icode,
                name: p.res_name,
                kind,
                ss,
                atoms: (start as u32, i as u32),
            });
            start = i;
        }
    }
}

fn resolve_ssbonds(mm: &mut MacroMol) {
    let pairs = std::mem::take(&mut mm.ssbond_pairs);
    let mut out = Vec::new();
    for &(c1, s1, c2, s2) in &pairs {
        let mut sg1 = None;
        let mut sg2 = None;
        for (ri, r) in mm.residues.iter().enumerate() {
            if r.kind != ResKind::Protein || r.name != *b"CYS" {
                continue;
            }
            if r.chain == c1 && r.seq == s1 {
                for &ai in &mm.residue_atom_ids(ri) {
                    if mm.atoms[ai].name_str() == "SG" {
                        sg1 = Some(ai);
                    }
                }
            }
            if r.chain == c2 && r.seq == s2 {
                for &ai in &mm.residue_atom_ids(ri) {
                    if mm.atoms[ai].name_str() == "SG" {
                        sg2 = Some(ai);
                    }
                }
            }
        }
        if let (Some(a), Some(b)) = (sg1, sg2) {
            out.push((a, b));
        }
    }
    mm.ssbonds = out;
}

// NOTE: dbg-хелперы удалены

// ─── Методы MacroMol ────────────────────────────────────────────────────

impl MacroMol {
    /// Прочитать файл: PDB или mmCIF (по префиксу data_).
    pub fn from_file(path: &str) -> Result<MacroMol, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("не прочитать {path}: {e}"))?;
        let first = text.lines().map(str::trim).find(|l| !l.is_empty());
        if let Some(f) = first {
            if f.starts_with("data_") {
                return parse_mmcif(&text);
            }
        }
        parse_pdb(&text)
    }

    /// Индексы атомов остатка.
    pub fn residue_atom_ids(&self, ri: usize) -> Vec<usize> {
        let (a, b) = self.residues[ri].atoms;
        (a as usize..b as usize).collect()
    }

    /// Атом остатка по имени.
    pub fn residue_atom(&self, ri: usize, name: &str) -> Option<usize> {
        let (a, b) = self.residues[ri].atoms;
        for i in a as usize..b as usize {
            if self.atoms[i].name_str() == name {
                return Some(i);
            }
        }
        None
    }

    /// Цепочки белковых остатков (в порядке появления).
    pub fn chains(&self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        for r in &self.residues {
            if r.kind == ResKind::Protein && !out.contains(&r.chain) {
                out.push(r.chain);
            }
        }
        out
    }

    /// Рецептор: белковые остатки + ионы (без вод и лигандов).
    pub fn receptor_atoms(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for (ri, r) in self.residues.iter().enumerate() {
            if matches!(r.kind, ResKind::Protein | ResKind::Ion) {
                out.extend(self.residue_atom_ids(ri));
            }
        }
        out
    }

    /// Лиганды (HETATM без вод/ионов) — по имени остатка.
    pub fn ligand_residues(&self) -> Vec<usize> {
        self.residues
            .iter()
            .enumerate()
            .filter(|(_, r)| r.kind == ResKind::Ligand)
            .map(|(i, _)| i)
            .collect()
    }

    /// Число вод.
    pub fn water_count(&self) -> usize {
        self.residues
            .iter()
            .filter(|r| r.kind == ResKind::Water)
            .map(|r| (r.atoms.1 - r.atoms.0) as usize)
            .sum()
    }

    /// Последовательность цепи однобуквенным кодом.
    pub fn sequence(&self, chain: u8) -> String {
        let mut s = String::new();
        for r in &self.residues {
            if r.kind == ResKind::Protein && r.chain == chain {
                s.push(aa_one_letter(&r.name).map(|c| c as char).unwrap_or('x'));
            }
        }
        s
    }

    /// Суммарный формальный заряд рецептора (Arg/Lys/Asp/Glu + концы + ионы).
    pub fn net_charge(&self) -> f64 {
        let mut q = 0.0;
        let mut prev_chain: Option<u8> = None;
        let protein: Vec<usize> = self
            .residues
            .iter()
            .enumerate()
            .filter(|(_, r)| r.kind == ResKind::Protein)
            .map(|(i, _)| i)
            .collect();
        for &ri in &protein {
            let r = &self.residues[ri];
            if prev_chain != Some(r.chain) {
                q += 1.0; // N-конец каждой цепи
            }
            q += match &r.name {
                b"ARG" | b"LYS" => 1.0,
                b"ASP" | b"GLU" => -1.0,
                _ => 0.0,
            };
            prev_chain = Some(r.chain);
        }
        // C-концы: по одному на цепь
        let mut seen: Vec<u8> = Vec::new();
        for &ri in protein.iter().rev() {
            let r = &self.residues[ri];
            if !seen.contains(&r.chain) {
                seen.push(r.chain);
                q -= 1.0;
            }
        }
        // ионы
        for r in &self.residues {
            if r.kind == ResKind::Ion {
                if let Some(iq) = ion_charge(&r.name) {
                    q += iq;
                }
            }
        }
        q
    }

    /// Остаток, владеющий атомом (бинарный поиск по диапазонам).
    pub fn residue_of_atom(&self, atom_idx: usize) -> Option<usize> {
        self.residues
            .binary_search_by(|r| {
                if atom_idx < r.atoms.0 as usize {
                    std::cmp::Ordering::Greater
                } else if atom_idx >= r.atoms.1 as usize {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .ok()
    }

    /// Класс остатка.
    pub fn res_class(&self, ri: usize) -> ResClass {
        residue_class(&self.residues[ri].name, self.residues[ri].kind)
    }
}

// ─── Заряды рецептора (united-схема) ────────────────────────────────────

/// Частичные заряды атомов рецептора (e). Самосогласованная объединённая
/// схема: NET-заряды остатков точны, полярность боковых цепей лёгкая.
///
/// * хребет (нейтральный остаток = 0): N −0.38, CA +0.38, C +0.50, O −0.50
/// * концы: N-конец N +0.80 + CA +0.20 (+1); C-конец C +0.80, O −0.80,
///   OXT −1.00 (−1)
/// * ARG: NE +0.30, NH1/NH2 +0.35 (итого +1); LYS: NZ +1.0
/// * ASP: OD1/OD2 −0.50; GLU: OE1/OE2 −0.50 (итого −1)
/// * ASN: OD1 −0.40, ND2 +0.40; GLN: OE1 −0.40, NE2 +0.40
/// * HIS (HID): ND1 +0.05, NE2 −0.05; TRP: NE1 −0.15, CD1 +0.15
/// * SER/THR/TYR: O −0.25, соседний C +0.25; CYS: SG −0.20, CB +0.20
/// * MET: SD −0.10, CG/CE +0.05
/// * ионы: заряд из таблицы (SO4: O −0.50 ×4)
pub fn receptor_charges(mm: &MacroMol) -> Vec<f64> {
    let mut q = vec![0.0f64; mm.atoms.len()];
    // первые/последние белковые остатки каждой цепи (индексы остатков)
    let mut first_idx: Vec<(u8, usize)> = Vec::new();
    let mut last_idx: Vec<(u8, usize)> = Vec::new();
    for (ri, r) in mm.residues.iter().enumerate() {
        if r.kind != ResKind::Protein {
            continue;
        }
        if !first_idx.iter().any(|&(c, _)| c == r.chain) {
            first_idx.push((r.chain, ri));
        }
        match last_idx.iter().position(|&(c, _)| c == r.chain) {
            Some(i) => last_idx[i].1 = ri,
            None => last_idx.push((r.chain, ri)),
        }
    }

    for (ri, r) in mm.residues.iter().enumerate() {
        let (a, b) = r.atoms;
        let ai = |name: &str| -> Option<usize> {
            (a as usize..b as usize).find(|&i| mm.atoms[i].name_str() == name)
        };
        match r.kind {
            ResKind::Protein => {
                let first = first_idx.iter().any(|&(_, i)| i == ri);
                let last = last_idx.iter().any(|&(_, i)| i == ri);
                // хребет
                if let Some(i) = ai("N") {
                    q[i] = if first { 0.80 } else { -0.38 };
                }
                if let Some(i) = ai("CA") {
                    q[i] = if first { 0.20 } else { 0.38 };
                }
                if let Some(i) = ai("C") {
                    q[i] = if last { 0.80 } else { 0.50 };
                }
                if let Some(i) = ai("O") {
                    q[i] = if last { -0.80 } else { -0.50 };
                }
                if last {
                    if let Some(i) = ai("OXT") {
                        q[i] = -1.00;
                    }
                }
                // боковые цепи
                match &r.name {
                    b"ARG" => {
                        if let Some(i) = ai("NE") {
                            q[i] = 0.30;
                        }
                        for n in ["NH1", "NH2"] {
                            if let Some(i) = ai(n) {
                                q[i] = 0.35;
                            }
                        }
                    }
                    b"LYS" => {
                        if let Some(i) = ai("NZ") {
                            q[i] = 1.0;
                        }
                    }
                    b"ASP" => {
                        for n in ["OD1", "OD2"] {
                            if let Some(i) = ai(n) {
                                q[i] = -0.50;
                            }
                        }
                    }
                    b"GLU" => {
                        for n in ["OE1", "OE2"] {
                            if let Some(i) = ai(n) {
                                q[i] = -0.50;
                            }
                        }
                    }
                    b"ASN" => {
                        if let Some(i) = ai("OD1") {
                            q[i] = -0.40;
                        }
                        if let Some(i) = ai("ND2") {
                            q[i] = 0.40;
                        }
                    }
                    b"GLN" => {
                        if let Some(i) = ai("OE1") {
                            q[i] = -0.40;
                        }
                        if let Some(i) = ai("NE2") {
                            q[i] = 0.40;
                        }
                    }
                    b"HIS" => {
                        if let Some(i) = ai("ND1") {
                            q[i] = 0.05;
                        }
                        if let Some(i) = ai("NE2") {
                            q[i] = -0.05;
                        }
                    }
                    b"TRP" => {
                        if let Some(i) = ai("NE1") {
                            q[i] = -0.15;
                        }
                        if let Some(i) = ai("CD1") {
                            q[i] = 0.15;
                        }
                    }
                    b"SER" => {
                        if let Some(i) = ai("OG") {
                            q[i] = -0.25;
                        }
                        if let Some(i) = ai("CB") {
                            q[i] = 0.25;
                        }
                    }
                    b"THR" => {
                        if let Some(i) = ai("OG1") {
                            q[i] = -0.25;
                        }
                        if let Some(i) = ai("CB") {
                            q[i] = 0.15;
                        }
                        if let Some(i) = ai("CG2") {
                            q[i] = 0.10;
                        }
                    }
                    b"TYR" => {
                        if let Some(i) = ai("OH") {
                            q[i] = -0.25;
                        }
                        if let Some(i) = ai("CZ") {
                            q[i] = 0.25;
                        }
                    }
                    b"CYS" => {
                        if let Some(i) = ai("SG") {
                            q[i] = -0.20;
                        }
                        if let Some(i) = ai("CB") {
                            q[i] = 0.20;
                        }
                    }
                    b"MET" | b"MSE" => {
                        let sd = if r.name == *b"MSE" { "SE" } else { "SD" };
                        if let Some(i) = ai(sd) {
                            q[i] = -0.10;
                        }
                        for n in ["CG", "CE"] {
                            if let Some(i) = ai(n) {
                                q[i] = 0.05;
                            }
                        }
                    }
                    _ => {}
                }
            }
            ResKind::Ion => {
                let net = ion_charge(&r.name).unwrap_or(0.0);
                if r.name == *b"SO4" || r.name == *b"PO4" {
                    // заряд на кислородах
                    for i in a as usize..b as usize {
                        if mm.atoms[i].elem_str() == "O" {
                            q[i] = -0.50;
                        }
                    }
                } else if (b - a) as usize == 1 {
                    q[a as usize] = net;
                } else {
                    q[a as usize] = net;
                }
            }
            _ => {
                // лиганды/воды: формальный заряд из файла, если был
                for i in a as usize..b as usize {
                    q[i] = mm.atoms[i].formal_charge as f64;
                }
            }
        }
    }
    q
}

// ─── Реконструкция донорных H ───────────────────────────────────────────

/// Реконструированный донорный водород.
#[derive(Debug, Clone, Copy)]
pub struct DonorH {
    /// Индекс атома-донора (N/O/S) в mm.atoms.
    pub donor: usize,
    /// Позиция H, Å.
    pub h: [f64; 3],
}

fn v_sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn v_add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn v_scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn v_norm(a: [f64; 3]) -> [f64; 3] {
    let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if n < 1e-9 {
        [0.0, 0.0, 1.0]
    } else {
        v_scale(a, 1.0 / n)
    }
}
fn v_cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Длина связи D–H по элементу донора.
fn h_bond_len(elem: &str) -> f64 {
    match elem {
        "N" => 1.03,
        "O" => 0.97,
        "S" => 1.33,
        _ => 1.09,
    }
}

/// Ортонормированный базис с осью v.
fn basis_with_axis(v: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let up = if v[2].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 0.0, 1.0] };
    let p = v_norm(v_cross(up, v));
    let q = v_cross(v, p);
    (p, q)
}

/// Реконструировать H-доноры белка (X-ray обычно без H).
///
/// Геометрия: амидный N−H по биссектрисе противоположно CA/C′; NZ (Lys) —
/// тригональная пирамида; гуанидиний Arg — в плоскости; His/Trp — от
/// центроида кольца; Ser/Thr/Tyr/Cys — от единственного соседа.
pub fn reconstruct_donors(mm: &MacroMol) -> Vec<DonorH> {
    let mut out = Vec::new();
    let protein: Vec<usize> = mm
        .residues
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == ResKind::Protein)
        .map(|(i, _)| i)
        .collect();

    let mut prev_res: Option<usize> = None;
    for &ri in &protein {
        let r = &mm.residues[ri];
        let name = r.name_str();
        let prev_in_chain = if prev_res.map(|p| mm.residues[p].chain == r.chain).unwrap_or(false) {
            prev_res
        } else {
            None
        };

        let pos = |nm: &str| mm.residue_atom(ri, nm).map(|i| mm.atoms[i].pos);

        // Амидный N–H (все кроме PRO)
        if name != "PRO" {
            if let Some(n) = pos("N") {
                let ca = pos("CA");
                let cprev = prev_in_chain.and_then(|p| mm.residue_atom(p, "C")).map(|i| mm.atoms[i].pos);
                let dir = match (ca, cprev) {
                    (Some(ca), Some(cp)) => {
                        let u1 = v_norm(v_sub(ca, n));
                        let u2 = v_norm(v_sub(cp, n));
                        v_norm(v_scale(v_add(u1, u2), -1.0))
                    }
                    (Some(ca), None) => v_norm(v_sub(n, ca)), // N-конец: H от CA
                    _ => [0.0, 0.0, 1.0],
                };
                let l = h_bond_len(mm.atoms[mm.residue_atom(ri, "N").unwrap()].elem_str());
                out.push(DonorH {
                    donor: mm.residue_atom(ri, "N").unwrap(),
                    h: v_add(n, v_scale(dir, l)),
                });
            }
        }
        // N-конец: NH3+ — пирамида от CA
        let is_first = prev_in_chain.is_none();
        if is_first && name != "PRO" {
            if let (Some(n), Some(ca)) = (pos("N"), pos("CA")) {
                let v = v_norm(v_sub(n, ca));
                let (p, q) = basis_with_axis(v);
                let alpha = 70.5f64.to_radians();
                for k in 0..3 {
                    let phi = (90.0 + 120.0 * k as f64).to_radians();
                    let d = v_add(v_scale(v, alpha.cos()), {
                        let w = v_add(v_scale(p, phi.cos()), v_scale(q, phi.sin()));
                        v_scale(w, alpha.sin())
                    });
                    out.push(DonorH {
                        donor: mm.residue_atom(ri, "N").unwrap(),
                        h: v_add(n, v_scale(v_norm(d), 1.03)),
                    });
                }
            }
        }

        match name {
            "LYS" => {
                if let (Some(nz), Some(ce)) = (pos("NZ"), pos("CE")) {
                    let v = v_norm(v_sub(nz, ce));
                    let (p, q) = basis_with_axis(v);
                    let alpha = 70.5f64.to_radians();
                    for k in 0..3 {
                        let phi = (90.0 + 120.0 * k as f64).to_radians();
                        let d = v_add(v_scale(v, alpha.cos()), {
                            let w = v_add(v_scale(p, phi.cos()), v_scale(q, phi.sin()));
                            v_scale(w, alpha.sin())
                        });
                        out.push(DonorH {
                            donor: mm.residue_atom(ri, "NZ").unwrap(),
                            h: v_add(nz, v_scale(v_norm(d), 1.03)),
                        });
                    }
                }
            }
            "ARG" => {
                // NE: один H от CZ; NH1/NH2: по два H в плоскости гуанидиния
                let cz = pos("CZ");
                let ne = pos("NE");
                if let (Some(cz), Some(ne)) = (cz, ne) {
                    let d = v_norm(v_sub(ne, cz));
                    out.push(DonorH {
                        donor: mm.residue_atom(ri, "NE").unwrap(),
                        h: v_add(ne, v_scale(d, 1.03)),
                    });
                    // плоскость
                    let nh1 = pos("NH1");
                    if let Some(nh1) = nh1 {
                        let nrm = v_norm(v_cross(v_sub(ne, cz), v_sub(nh1, cz)));
                        for nn in ["NH1", "NH2"] {
                            if let Some(npos) = pos(nn) {
                                let v = v_norm(v_sub(npos, cz));
                                let w = v_norm(v_cross(nrm, v));
                                for sgn in [1.0f64, -1.0f64] {
                                    let d = v_norm(v_add(v_scale(v, 0.5), v_scale(w, 0.866 * sgn)));
                                    out.push(DonorH {
                                        donor: mm.residue_atom(ri, nn).unwrap(),
                                        h: v_add(npos, v_scale(d, 1.03)),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            "HIS" => {
                // HID-таутомер: H на ND1, от центроида кольца
                let ring = ["CG", "ND1", "CE1", "NE2", "CD2"];
                let pts: Vec<[f64; 3]> = ring.iter().filter_map(|n| pos(n)).collect();
                if pts.len() >= 4 {
                    let c = v_scale(
                        pts.iter().fold([0.0; 3], |acc, p| v_add(acc, *p)),
                        1.0 / pts.len() as f64,
                    );
                    if let Some(nd1) = pos("ND1") {
                        let d = v_norm(v_sub(nd1, c));
                        out.push(DonorH {
                            donor: mm.residue_atom(ri, "ND1").unwrap(),
                            h: v_add(nd1, v_scale(d, 1.03)),
                        });
                    }
                }
            }
            "TRP" => {
                let ring = ["CD1", "CG", "CD2", "CE2", "CZ2", "NE1"];
                let pts: Vec<[f64; 3]> = ring.iter().filter_map(|n| pos(n)).collect();
                if pts.len() >= 5 {
                    let c = v_scale(
                        pts.iter().fold([0.0; 3], |acc, p| v_add(acc, *p)),
                        1.0 / pts.len() as f64,
                    );
                    if let Some(ne1) = pos("NE1") {
                        let d = v_norm(v_sub(ne1, c));
                        out.push(DonorH {
                            donor: mm.residue_atom(ri, "NE1").unwrap(),
                            h: v_add(ne1, v_scale(d, 1.03)),
                        });
                    }
                }
            }
            "ASN" | "GLN" => {
                // амидная NH2 в плоскости C(O)–N
                let (c_name, o_name, n_name) = if name == "ASN" {
                    ("CG", "OD1", "ND2")
                } else {
                    ("CD", "OE1", "NE2")
                };
                if let (Some(c), Some(o), Some(n)) = (pos(c_name), pos(o_name), pos(n_name)) {
                    let v = v_norm(v_sub(n, c));
                    let nrm = v_norm(v_cross(v_sub(o, c), v_sub(n, c)));
                    let w = v_norm(v_cross(nrm, v));
                    for sgn in [1.0f64, -1.0f64] {
                        let d = v_norm(v_add(v_scale(v, 0.5), v_scale(w, 0.866 * sgn)));
                        out.push(DonorH {
                            donor: mm.residue_atom(ri, n_name).unwrap(),
                            h: v_add(n, v_scale(d, 1.03)),
                        });
                    }
                }
            }
            "SER" | "THR" | "TYR" | "CYS" => {
                let (o_name, c_name) = match name {
                    "SER" => ("OG", "CB"),
                    "THR" => ("OG1", "CB"),
                    "TYR" => ("OH", "CZ"),
                    _ => ("SG", "CB"),
                };
                if let (Some(o), Some(c)) = (pos(o_name), pos(c_name)) {
                    let d = v_norm(v_sub(o, c));
                    let l = h_bond_len(mm.atoms[mm.residue_atom(ri, o_name).unwrap()].elem_str());
                    out.push(DonorH {
                        donor: mm.residue_atom(ri, o_name).unwrap(),
                        h: v_add(o, v_scale(d, l)),
                    });
                }
            }
            _ => {}
        }
        prev_res = Some(ri);
    }
    out
}

// ─── Карман ─────────────────────────────────────────────────────────────

/// Карман связывания.
#[derive(Debug, Clone)]
pub struct Pocket {
    /// Центр, Å.
    pub center: [f64; 3],
    /// Радиус сферы кармана (атомы рецептора в ней), Å.
    pub radius: f64,
    /// Индексы остатков рецептора в кармане.
    pub residues: Vec<usize>,
    /// Индексы атомов рецептора в кармане.
    pub atoms: Vec<usize>,
    /// Метод: «лиганд» | «слепой» | «точка».
    pub method: &'static str,
}

fn centroid_of(atoms: &[usize], mm: &MacroMol) -> [f64; 3] {
    let mut c = [0.0f64; 3];
    for &i in atoms {
        c[0] += mm.atoms[i].pos[0];
        c[1] += mm.atoms[i].pos[1];
        c[2] += mm.atoms[i].pos[2];
    }
    if !atoms.is_empty() {
        for k in 0..3 {
            c[k] /= atoms.len() as f64;
        }
    }
    c
}

/// Карман по кристаллическому лиганду: 8 Å вокруг его атомов.
pub fn pocket_from_ligand(mm: &MacroMol, lig_res: usize) -> Pocket {
    let lig_atoms: Vec<usize> = mm.residue_atom_ids(lig_res)
        .into_iter()
        .filter(|&i| mm.atoms[i].elem_str() != "H")
        .collect();
    let lig_name = mm.residues[lig_res].name;
    let c = if lig_atoms.is_empty() {
        mm.residue_atom_ids(lig_res)
            .first()
            .map(|&i| mm.atoms[i].pos)
            .unwrap_or([0.0; 3])
    } else {
        centroid_of(&lig_atoms, mm)
    };
    let mut radius = 0.0f64;
    for &i in &lig_atoms {
        radius = radius.max(dist(c, mm.atoms[i].pos));
    }
    radius += 4.0;
    let mut atoms = Vec::new();
    for (i, a) in mm.atoms.iter().enumerate() {
        if a.het && a.res_name == lig_name {
            continue; // сам лиганд исключаем
        }
        if matches!(a.elem_str(), "H" | "D") {
            continue; // H не считаем контактами
        }
        if dist(c, a.pos) <= radius + 2.0 {
            atoms.push(i);
        }
    }
    let residues = residues_of_atoms(mm, &atoms);
    Pocket {
        center: c,
        radius,
        residues,
        atoms,
        method: "лиганд",
    }
}

/// Слепой поиск кармана: решётка 1.5 Å, критерий вогнутости —
/// много соседей в сфере 8 Å И точка вне ВдВ-контакта (2.4–5.5 Å от
/// ближайшего атома). Внутренность белка отсекается порогом d_min.
pub fn pocket_blind(mm: &MacroMol) -> Result<Pocket, String> {
    let receptor = mm.receptor_atoms();
    if receptor.is_empty() {
        return Err("нет атомов рецептора".into());
    }
    let mut min = [f64::MAX; 3];
    let mut max = [f64::MIN; 3];
    for &i in &receptor {
        let p = mm.atoms[i].pos;
        for k in 0..3 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    // решётка 1.5 Å
    let step = 1.5f64;
    let count_near = |p: [f64; 3], r: f64| -> (usize, f64) {
        let mut n = 0usize;
        let mut dmin = f64::MAX;
        let r2 = r * r;
        for &i in &receptor {
            let q = mm.atoms[i].pos;
            let d2 = {
                let dx = p[0] - q[0];
                let dy = p[1] - q[1];
                let dz = p[2] - q[2];
                dx * dx + dy * dy + dz * dz
            };
            if d2 < r2 {
                n += 1;
            }
            dmin = dmin.min(d2.sqrt());
        }
        (n, dmin)
    };

    let mut candidates: Vec<([f64; 3], usize)> = Vec::new();
    let mut max_burial = 0usize;
    let mut x = min[0] + step;
    while x < max[0] {
        let mut y = min[1] + step;
        while y < max[1] {
            let mut z = min[2] + step;
            while z < max[2] {
                let p = [x, y, z];
                let (burial, dmin) = count_near(p, 8.0);
                if dmin >= 2.4 && dmin <= 5.5 && burial >= 40 {
                    candidates.push((p, burial));
                    max_burial = max_burial.max(burial);
                }
                z += step;
            }
            y += step;
        }
        x += step;
    }
    if candidates.is_empty() {
        return Err("слепой поиск: кандидатов нет (малый/плоский рецептор?)".into());
    }
    // порог: верхние 30% захоронения
    let thr = ((max_burial as f64) * 0.7).ceil() as usize;
    let mut best: Option<([f64; 3], usize)> = None;
    for &(p, b) in &candidates {
        if b >= thr {
            match best {
                Some((_, bb)) if bb >= b => {}
                _ => best = Some((p, b)),
            }
        }
    }
    let (seed, _bbest) = best.ok_or("слепой поиск: seed не найден")?;
    // кластер: кандидаты в 6 Å от seed → взвешенный центроид
    let cluster: Vec<([f64; 3], usize)> = candidates
        .iter()
        .copied()
        .filter(|&(p, _)| dist(p, seed) <= 6.0)
        .collect();
    let mut wsum = 0.0f64;
    let mut c = [0.0f64; 3];
    for &(p, b) in &cluster {
        let w = b as f64;
        wsum += w;
        for k in 0..3 {
            c[k] += p[k] * w;
        }
    }
    for k in 0..3 {
        c[k] /= wsum.max(1.0);
    }
    let radius = 6.5f64;
    let mut atoms = Vec::new();
    for &i in &receptor {
        if dist(c, mm.atoms[i].pos) <= radius + 2.5 {
            atoms.push(i);
        }
    }
    let residues = residues_of_atoms(mm, &atoms);
    Ok(Pocket {
        center: c,
        radius,
        residues,
        atoms,
        method: "слепой",
    })
}

/// Карман по точке (x,y,z) — ручное указание центра.
pub fn pocket_at_point(mm: &MacroMol, center: [f64; 3], radius: f64) -> Pocket {
    let mut atoms = Vec::new();
    for &i in &mm.receptor_atoms() {
        if dist(center, mm.atoms[i].pos) <= radius {
            atoms.push(i);
        }
    }
    let residues = residues_of_atoms(mm, &atoms);
    Pocket {
        center,
        radius,
        residues,
        atoms,
        method: "точка",
    }
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Остатки, владеющие данными атомами (уникальные, по порядку).
fn residues_of_atoms(mm: &MacroMol, atoms: &[usize]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    'outer: for (ri, r) in mm.residues.iter().enumerate() {
        let (a, b) = r.atoms;
        for &i in atoms {
            if (i as u32) >= a && (i as u32) < b {
                out.push(ri);
                continue 'outer;
            }
        }
    }
    out
}

// ─── tree27-индекс белка ────────────────────────────────────────────────

/// Пространственный индекс рецептора: 27-дерево на решётке Å×100.
pub struct ProteinIndex {
    tree: Tree27,
    /// Атомы-рецепторы (индексы в mm.atoms) — payload дерева.
    pub ids: Vec<usize>,
    pub stats: (usize, usize, u8),
}

impl ProteinIndex {
    /// Построить индекс по атомам рецептора.
    pub fn build(mm: &MacroMol, receptor: &[usize]) -> ProteinIndex {
        const PAD: i64 = 250; // запас под конверты ±2 Å + окно запроса
        let mut min = [i64::MAX; 3];
        let mut max = [i64::MIN; 3];
        for &i in receptor {
            let p = mm.atoms[i].pos;
            for k in 0..3 {
                let v = (p[k] * 100.0).round() as i64;
                min[k] = min[k].min(v - PAD);
                max[k] = max[k].max(v + PAD);
            }
        }
        let bounds = BBox3::new(min[0], min[1], min[2], max[0], max[1], max[2]);
        let mut tree = Tree27::new(bounds, 8);
        for (k, &i) in receptor.iter().enumerate() {
            let p = mm.atoms[i].pos;
            let x = (p[0] * 100.0).round() as i64;
            let y = (p[1] * 100.0).round() as i64;
            let z = (p[2] * 100.0).round() as i64;
            // небольшой конверт вокруг атома (ВдВ ~ 2 Å)
            let _ = tree.insert(BBox3::new(x - 200, y - 200, z - 200, x + 200, y + 200, z + 200), k);
        }
        let stats = tree.stats();
        ProteinIndex {
            tree,
            ids: receptor.to_vec(),
            stats,
        }
    }

    /// Атомы рецептора в сфере радиуса r вокруг точки (Å).
    pub fn within(&self, mm: &MacroMol, p: [f64; 3], r: f64) -> Vec<usize> {
        let x = (p[0] * 100.0).round() as i64;
        let y = (p[1] * 100.0).round() as i64;
        let z = (p[2] * 100.0).round() as i64;
        let rr = (r * 100.0).round() as i64 + 200; // + конверт атома
        let q = BBox3::new(x - rr, y - rr, z - rr, x + rr, y + rr, z + rr);
        let hits = self.tree.query(&q);
        // точная фильтрация по евклиду
        let r2 = r * r;
        hits.into_iter()
            .filter(|&k| {
                let ap = mm.atoms[self.ids[k]].pos;
                let dx = ap[0] - p[0];
                let dy = ap[1] - p[1];
                let dz = ap[2] - p[2];
                dx * dx + dy * dy + dz * dz <= r2
            })
            .map(|k| self.ids[k])
            .collect()
    }
}

// ─── Паспорт макромолекулы ──────────────────────────────────────────────

/// Человекочитаемый паспорт (стиль «голоса учёного»).
pub fn macro_passport(mm: &MacroMol, path: &str) -> String {
    let protein_res: usize = mm.residues.iter().filter(|r| r.kind == ResKind::Protein).count();
    let n_helix = mm.residues.iter().filter(|r| r.ss == SsKind::Helix).count();
    let n_sheet = mm.residues.iter().filter(|r| r.ss == SsKind::Sheet).count();
    let chains = mm.chains();
    let het = mm.atoms.iter().filter(|a| a.het).count();
    let lig = mm.ligand_residues();
    let q = mm.net_charge();
    let mut s = String::new();
    s.push_str(&format!(
        "═══ Макромолекула: {path} ═══\n"
    ));
    if !mm.title.is_empty() {
        s.push_str(&format!("[0] {}\n", truncate(&mm.title, 100)));
    }
    s.push_str(&format!(
        "[1] Атомы: {} всего ({} тяжёлых PDB-строк ATOM, {} HETATM), {} остатков белка, {} вод, {} ионов, {} лигандов; моделей {}{}.\n",
        mm.atoms.len(),
        mm.atoms.iter().filter(|a| !a.het).count(),
        het,
        protein_res,
        mm.water_count(),
        mm.residues.iter().filter(|r| r.kind == ResKind::Ion).count(),
        lig.len(),
        mm.n_models,
        mm.resolution
            .map(|r| format!(", разрешение {r:.2} Å"))
            .unwrap_or_default()
    ));
    s.push_str(&format!(
        "[2] Цепочки: {}. ",
        chains
            .iter()
            .map(|&c| {
                let ch = if c == 0 { '_' } else { c as char };
                format!("{ch}: {} остатков", mm.sequence(c).len())
            })
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if let Some(&c0) = chains.first() {
        let seq = mm.sequence(c0);
        if seq.len() > 60 {
            s.push_str(&format!("N-конец: {}… C-конец: …{}\n", &seq[..30], &seq[seq.len() - 30..]));
        } else if !seq.is_empty() {
            s.push_str(&format!("Последовательность: {seq}\n"));
        } else {
            s.push('\n');
        }
    } else {
        s.push_str("белковых цепочек нет\n");
    }
    s.push_str(&format!(
        "[3] Вторичная структура: {} α-спиральных остатков, {} β-листовых (HELIX/SHEET записи: {} + {}), дисульфидов {}.\n",
        n_helix,
        n_sheet,
        mm.helix_ranges.len(),
        mm.sheet_ranges.len(),
        mm.ssbonds.len()
    ));
    let composition = residue_composition(mm);
    s.push_str(&format!(
        "[4] Состав: гидрофобные {}, полярные {}, заряженные {}, ароматические {}, всего NET-заряд рецептора {:+.0} e.\n",
        composition.0,
        composition.1,
        composition.2,
        composition.3,
        q
    ));
    if !lig.is_empty() {
        let names: Vec<String> = lig
            .iter()
            .map(|&ri| {
                let r = &mm.residues[ri];
                format!(
                    "{}{} ({} атомов)",
                    r.name_str(),
                    if r.chain == 0 { '_' } else { r.chain as char },
                    (r.atoms.1 - r.atoms.0)
                )
            })
            .collect();
        s.push_str(&format!("[5] Лиганды/кофакторы: {}.\n", names.join(", ")));
    }
    s
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        let mut end = n;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

fn residue_composition(mm: &MacroMol) -> (usize, usize, usize, usize) {
    let mut h = 0;
    let mut p = 0;
    let mut c = 0;
    let mut a = 0;
    for (ri, r) in mm.residues.iter().enumerate() {
        if r.kind != ResKind::Protein {
            continue;
        }
        match mm.res_class(ri) {
            ResClass::Hydrophobic | ResClass::Cysteine | ResClass::Glycine => h += 1,
            ResClass::Polar => p += 1,
            ResClass::ChargedPos | ResClass::ChargedNeg => c += 1,
            ResClass::Aromatic => a += 1,
            ResClass::Other => {}
        }
    }
    (h, p, c, a)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Мини-белок из 3 остатков: ALA-LYS-ASP + C-конец OXT.
    const MINI_PDB: &str = "\
HEADER    TEST
TITLE     MINI PROTEIN FOR TESTS
REMARK   2 RESOLUTION.    1.50 ANGSTROMS.
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.458   0.000   0.000  1.00 10.00           C
ATOM      3  C   ALA A   1       2.010   1.418   0.000  1.00 10.00           C
ATOM      4  O   ALA A   1       1.234   2.404   0.000  1.00 10.00           O
ATOM      5  N   LYS A   2       3.320   1.658   0.000  1.00 10.00           N
ATOM      6  CA  LYS A   2       3.872   3.076   0.000  1.00 10.00           C
ATOM      7  C   LYS A   2       4.424   4.494   0.000  1.00 10.00           C
ATOM      8  O   LYS A   2       3.648   5.480   0.000  1.00 10.00           O
ATOM      9  CE  LYS A   2       5.300   4.000   0.000  1.00 10.00           C
ATOM     10  NZ  LYS A   2       6.100   5.200   0.000  1.00 10.00           N
ATOM     11  N   ASP A   3       5.734   4.734   0.000  1.00 10.00           N
ATOM     12  CA  ASP A   3       6.286   6.152   0.000  1.00 10.00           C
ATOM     13  C   ASP A   3       6.838   7.570   0.000  1.00 10.00           C
ATOM     14  O   ASP A   3       6.062   8.556   0.000  1.00 10.00           O
ATOM     15  OXT ASP A   3       8.168   7.810   0.000  1.00 10.00           O
ATOM     16  OD1 ASP A   3       7.600   5.600   1.200  1.00 10.00           O
ATOM     17  OD2 ASP A   3       7.600   5.600  -1.200  1.00 10.00           O
HETATM   18  O   HOH A  20       9.000   0.000   0.000  1.00 20.00           O
HETATM   19  CA  CA  A  30       9.000   9.000   0.000  1.00 15.00          CA
HETATM   20  C1  BEN A  40       5.000   0.000   5.000  1.00 12.00           C
HETATM   21  N1  BEN A  40       6.300   0.000   5.000  1.00 12.00           N
TER
END
";

    #[test]
    fn parse_mini_counts() {
        let mm = parse_pdb(MINI_PDB).unwrap();
        assert_eq!(mm.atoms.len(), 21);
        assert_eq!(mm.residues.len(), 6, "ALA,LYS,ASP,HOH,CA,BEN");
        assert_eq!(mm.residues.iter().filter(|r| r.kind == ResKind::Protein).count(), 3);
        assert_eq!(mm.water_count(), 1);
        assert_eq!(mm.residues.iter().filter(|r| r.kind == ResKind::Ion).count(), 1);
        assert_eq!(mm.ligand_residues().len(), 1);
        assert_eq!(mm.title, "MINI PROTEIN FOR TESTS");
        assert_eq!(mm.resolution, Some(1.50));
    }

    #[test]
    fn element_inference_rules() {
        let mm = parse_pdb(MINI_PDB).unwrap();
        // ATOM «CA» в белковом остатке — углерод, НЕ кальций
        assert_eq!(mm.atoms[1].elem_str(), "C", "CA альфа-углерод");
        // HETATM «CA» — ион кальция
        assert_eq!(mm.atoms[18].elem_str(), "Ca", "HETATM CA = кальций");
        // HETATM «C1»/«N1» — одно-буквенные из имени
        assert_eq!(mm.atoms[19].elem_str(), "C");
        assert_eq!(mm.atoms[20].elem_str(), "N");
        // Колонки 77–78 уважаются
        assert_eq!(mm.atoms[0].elem_str(), "N");
    }

    #[test]
    fn altloc_and_models() {
        // altLoc B (байт 16) отбрасывается; вторая модель игнорируется
        let mut alt: Vec<u8> =
            b"ATOM      2  N   ALA A   1       9.999   9.999   9.999  1.00 10.00           N".to_vec();
        alt[16] = b'B';
        let txt = "MODEL        1\nATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N\n".to_string()
            + &String::from_utf8(alt).unwrap()
            + "\nENDMDL\nMODEL        2\nATOM      3  N   GLY A   2       1.000   0.000   0.000  1.00 10.00           N\nENDMDL\n";
        let mm = parse_pdb(&txt).unwrap();
        assert_eq!(mm.atoms.len(), 1, "altLoc B выкинут, вторая модель тоже");
        assert_eq!(mm.n_models, 2);
        assert!((mm.atoms[0].pos[0] - 0.0).abs() < 1e-9, "остался валидный атом модели 1");
    }

    #[test]
    fn first_model_kept() {
        let txt = "MODEL        1\nATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N\nENDMDL\nMODEL        2\nATOM      2  N   GLY A   2       1.000   0.000   0.000  1.00 10.00           N\nENDMDL\n";
        let mm = parse_pdb(txt).unwrap();
        assert_eq!(mm.atoms.len(), 1, "только первая модель");
        assert_eq!(mm.n_models, 2);
        assert!((mm.atoms[0].pos[0] - 0.0).abs() < 1e-9);
    }

    #[test]
    fn charges_net_balance() {
        let mm = parse_pdb(MINI_PDB).unwrap();
        let q = receptor_charges(&mm);
        // Белок: атомы 0..17 (ALA 0-3, LYS 4-9, ASP 10-16, HOH 17)
        let sum: f64 = (0..17).map(|i| q[i]).sum();
        assert!((sum - 0.0).abs() < 1e-9, "NET белка = 0, получено {sum}");
        // Хребет нейтрального остатка (LYS): N+CA+C+O = 0
        let lys_bb = q[4] + q[5] + q[6] + q[7];
        assert!(lys_bb.abs() < 1e-9, "LYS backbone = {lys_bb}");
        // Кальций +2
        assert!((q[18] - 2.0).abs() < 1e-9, "CA ион +2");
        // Asp карбоксил: OD1+OD2 = −1
        let od = q[15] + q[16];
        assert!((od + 1.0).abs() < 1e-9);
        // Lys NZ +1
        assert!((q[9] - 1.0).abs() < 1e-9);
        // формальный NET: белок 0 + ион +2
        assert!((mm.net_charge() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn donor_h_reconstruction() {
        let mm = parse_pdb(MINI_PDB).unwrap();
        let donors = reconstruct_donors(&mm);
        // 3 амидных N-H (N-конец даёт +3, но первый N уже имеет 1 обычный H →
        // N-конец: 1 биссекторный + 3 пирамидальных; Lys NZ: 3; Asp: 1)
        let n_donor_atoms = donors.iter().map(|d| d.donor).filter(|&d| mm.atoms[d].name_str() == "N").count();
        assert!(n_donor_atoms >= 4, "N-доноров {n_donor_atoms}");
        let nz = donors.iter().filter(|d| mm.atoms[d.donor].name_str() == "NZ").count();
        assert_eq!(nz, 3, "Lys NZ: три H");
        // Длина N-H ≈ 1.03 Å
        for d in &donors {
            let dn = mm.atoms[d.donor].pos;
            let l = {
                let dx = d.h[0] - dn[0];
                let dy = d.h[1] - dn[1];
                let dz = d.h[2] - dn[2];
                (dx * dx + dy * dy + dz * dz).sqrt()
            };
            assert!((l - 1.03).abs() < 1e-6, "N-H длина {l}");
        }
    }

    #[test]
    fn pocket_from_ligand_excludes_ligand() {
        let mm = parse_pdb(MINI_PDB).unwrap();
        let lig = mm.ligand_residues()[0];
        let p = pocket_from_ligand(&mm, lig);
        assert_eq!(p.method, "лиганд");
        // атомы кармана не содержат сам BEN
        assert!(p.atoms.iter().all(|&i| mm.atoms[i].res_name != *b"BEN"));
        assert!(!p.residues.is_empty());
    }

    #[test]
    fn pocket_at_point_circle() {
        let mm = parse_pdb(MINI_PDB).unwrap();
        let p = pocket_at_point(&mm, [3.5, 2.0, 0.0], 3.0);
        assert_eq!(p.method, "точка");
        assert!(!p.atoms.is_empty());
    }

    #[test]
    fn tree27_index_neighbors() {
        let mm = parse_pdb(MINI_PDB).unwrap();
        let rec = mm.receptor_atoms();
        let idx = ProteinIndex::build(&mm, &rec);
        // атомы в 3 Å от NZ Lys (6.1, 5.2, 0)
        let near = idx.within(&mm, [6.1, 5.2, 0.0], 3.0);
        assert!(near.contains(&9), "NZ (атом 9) в собственной сфере");
        assert!(near.contains(&10), "N Asp (атом 10) в 3 Å от NZ");
        assert_eq!(idx.stats.0, rec.len(), "27-дерево: все атомы рецептора внутри");
    }

    #[test]
    fn mmcif_minimal_loop() {
        let txt = "\
data_test
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_alt_id
_atom_site.label_comp_id
_atom_site.auth_asym_id
_atom_site.auth_seq_id
_atom_site.pdbx_PDB_ins_code
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
_atom_site.pdbx_PDB_model_num
ATOM 1 N N . ALA A 1 ? 0.000 0.000 0.000 1.0 10.0 1
ATOM 2 C CA . ALA A 1 ? 1.458 0.000 0.000 1.0 10.0 1
HETATM 3 C C1 . BEN A 40 ? 5.000 0.000 5.000 1.0 12.0 1
HETATM 4 N 'C1''' . BEN A 40 ? 6.300 0.000 5.000 1.0 12.0 1
HETATM 5 O O . HOH A 50 ? 9.000 9.000 9.000 1.0 20.0 2
#
";
        let mm = parse_mmcif(txt).unwrap();
        // модель 2 (HOH) выкинута
        assert_eq!(mm.atoms.len(), 4);
        assert_eq!(mm.residues.len(), 2, "ALA, BEN (HOH был в модели 2)");
        assert_eq!(mm.atoms[1].elem_str(), "C");
        assert_eq!(mm.atoms[0].pos[0], 0.0);
    }

    #[test]
    fn right_justified_ion_name() {
        // PDB пишет короткие имена остатков вправо: " CA" — ион кальция
        let txt = "HETATM 1631 CA    CA A 480    -10.300   4.358  36.895  1.00 15.00          CA\n";
        let mm = parse_pdb(txt).unwrap();
        assert_eq!(mm.atoms[0].res_str(), "CA");
        assert_eq!(mm.residues[0].kind, ResKind::Ion);
        assert!((mm.net_charge() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn sequence_one_letter() {
        let mm = parse_pdb(MINI_PDB).unwrap();
        assert_eq!(mm.sequence(b'A'), "AKD");
    }

    #[test]
    fn secondary_structure_records() {
        let txt = "\
HELIX    1   1 GLY A    2 GLY A    4
ATOM      1  N   GLY A   2       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  N   GLY A   3       1.000   0.000   0.000  1.00 10.00           N
ATOM      3  N   GLY A   5       2.000   0.000   0.000  1.00 10.00           N
";
        let mm = parse_pdb(txt).unwrap();
        assert_eq!(mm.helix_ranges, vec![(b'A', 2, 4)]);
        assert_eq!(mm.residues[0].ss, SsKind::Helix);
        assert_eq!(mm.residues[1].ss, SsKind::Helix);
        assert_eq!(mm.residues[2].ss, SsKind::Coil);
    }

    #[test]
    fn ssbond_resolution() {
        let txt = "\
SSBOND   1 CYS A    2    CYS A    5
ATOM      1  N   CYS A   2       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  SG  CYS A   2       1.000   0.000   0.000  1.00 10.00           S
ATOM      3  N   CYS A   5       5.000   0.000   0.000  1.00 10.00           N
ATOM      4  SG  CYS A   5       6.000   0.000   0.000  1.00 10.00           S
";
        let mm = parse_pdb(txt).unwrap();
        assert_eq!(mm.ssbonds, vec![(1, 3)], "SG-SG по SSBOND-записи");
    }
    #[test]
    fn dbg_donors() {
        let mm = parse_pdb(MINI_PDB).unwrap();
        let donors = reconstruct_donors(&mm);
        eprintln!("donors total = {}", donors.len());
        for d in donors.iter().take(10) {
            eprintln!("  donor atom {} name {:?} elem {:?}", d.donor, mm.atoms[d.donor].name_str(), mm.atoms[d.donor].elem_str());
        }
        eprintln!("residues: {:?}", mm.residues.iter().map(|r| (r.name_str(), r.kind)).collect::<Vec<_>>());
        eprintln!("ALA atoms range {:?}", mm.residues[0].atoms);
        eprintln!("residue_atom(0,N) = {:?}", mm.residue_atom(0, "N"));
        eprintln!("atom0 name {:?} atoms0..4 {:?}", mm.atoms[0].name_str(), (0..4).map(|i| mm.atoms[i].name_str()).collect::<Vec<_>>());
    }
}
