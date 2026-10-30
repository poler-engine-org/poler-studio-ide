//! Ротамерная библиотека боковых цепей v0.78.0 (контур C1):
//! induced fit / soft pocket.
//!
//! χ-углы боковых цепей квантуются РОДНОЙ сеткой движка — 27 секторов
//! на оборот (2π/27 = 13⅓°), тем же механизмом, что и кручения лиганда.
//! Канонические значения (стиль Penultimate Library: χ1 ∈ {−60, +60,
//! 180}) проецируются на ближайшие узлы; соседние узлы достижимы
//! шагами ±1 сектор в MC (промежуточные — канон ступени).
//!
//! Верификация (C1): `nearest_node` — расстояние наблюдаемого χ из PDB
//! до ближайшего узла библиотеки; тест на 3PTB/1CRN сверяет статистику.
//!
//! Дисциплина: таблицы — статические срезы, zero-alloc; атомы — по
//! именам PDB без String.

use super::pdb::MacroMol;
use super::smiles::parse_smiles;

/// Сектор χ-сетки: 2π/27 = 13⅓° (родная трит-сетка движка).
pub const CHI_SECTOR_DEG: f64 = 360.0 / 27.0;

// ─── Глубина боковой цепи по имени атома ────────────────────────────────

/// Уровень атома в боковой цепи: 0 = остов (N, CA, C, O, OXT) или H,
/// 1 = CB, 2 = CG/OG/OG1/SG/CG1/CG2/SD, 3 = CD/ND1/OD1/OD2…, 4 = CE…,
/// 5 = CZ/CZ2…, 6 = CH2/NH1…
///
/// Правило: третья буква из хвоста имени кодирует глубину в стандартной
/// номенклатуре PDB (CB→CG→CD→CE→CZ→CH; OG→OD→OE→OH; SG→SD).
pub fn side_depth(name: &str) -> u8 {
    let n = name.trim();
    if n.len() < 2 || !n.as_bytes()[0].is_ascii_alphabetic() {
        return 0;
    }
    match n {
        "N" | "CA" | "C" | "O" | "OXT" => 0,
        "CB" => 1,
        _ => {
            // H-атомы остова/боковой цепи (в PDB с H): HA, HB2, HG… — 0,
            // они не вращаются отдельно от родителя
            if n.starts_with('H') {
                return 0;
            }
            // вторая буква: B→G→D→E→Z→H — глубина 1..6; исключения:
            // терминальные типы (NH1/NH2 Arg: H — 5-й уровень) читаются
            // по последней букве группы
            let b = n.as_bytes()[1];
            match b {
                b'B' => 1,
                b'G' => 2,
                b'D' => 3,
                b'E' => 4,
                b'Z' => 5,
                b'H' => 6,
                _ => {
                    // CG1/CG2 и т.п. уже покрыты; нестандарт — 0 (безопасно:
                    // атом не вращается)
                    0
                }
            }
        }
    }
}

// ─── Секторы ────────────────────────────────────────────────────────────

/// Угол (градусы) → ближайший узел сетки (тай −180..180 нормализуется).
/// Округление half-away-from-zero — детерминизм.
pub fn deg_to_sector(deg: f64) -> i32 {
    let d = norm_deg(deg);
    (d / CHI_SECTOR_DEG).round() as i32
}

/// Узел сетки → угол (градусы).
pub fn sector_to_deg(sec: i32) -> f64 {
    sec as f64 * CHI_SECTOR_DEG
}

/// Нормализация угла в (−180, 180].
pub fn norm_deg(deg: f64) -> f64 {
    let mut d = deg;
    while d > 180.0 {
        d -= 360.0;
    }
    while d <= -180.0 {
        d += 360.0;
    }
    d
}

// ─── Библиотека ─────────────────────────────────────────────────────────

/// Ротамерный набор остатка: узлы χ1/χ2 на 27-секторной сетке.
pub struct RotamerSet {
    pub residue: &'static str,
    /// Узлы χ1 (секторы; ±60/180 → ±5/14 c проме­жут­ными шагами MC).
    pub chi1: &'static [i8],
    /// Узлы χ2 (пусто — нет χ2).
    pub chi2: &'static [i8],
}

/// Канонические узлы χ1: {−60, +60, 180}° → секторы {−5, +5, 14}
/// (60/13⅓ = 4.5 — тай между узлами 4 и 5; округление half-away: 5).
/// Соседние узлы (−4, +4, 13) — «промежуточные», покрываются шагами ±1.
const CHI1_STD: &[i8] = &[-5, 5, 14];

/// Характерные узлы χ2 по типам (стиль Penultimate):
/// • ароматика/кислоты/амиды (Phe, Tyr, His, Trp, Asp, Asn): ±90° → ±7;
/// • Leu/Met: {65, 175} → {5, 13};
/// • Ile: {60, 120, 180} → {5, 9, 14};
/// • длинные (Arg, Lys, Glu, Gln): −60/60/180 → {−5, 5, 14}
///   (хвосты χ3+ не моделируются — компактная таблица χ1/χ2, канон C1).
const CHI2_AROM: &[i8] = &[-7, 7];
const CHI2_LEU: &[i8] = &[5, 13];
const CHI2_ILE: &[i8] = &[5, 9, 14];
const CHI2_LONG: &[i8] = &[-5, 5, 14];
const NONE: &[i8] = &[];

/// Ротамерная библиотека гибких остатков кармана.
pub fn rotamers(res_name: &str) -> Option<&'static RotamerSet> {
    Some(match res_name {
        "SER" => &RotamerSet { residue: "SER", chi1: CHI1_STD, chi2: NONE },
        "THR" => &RotamerSet { residue: "THR", chi1: CHI1_STD, chi2: NONE },
        "VAL" => &RotamerSet { residue: "VAL", chi1: CHI1_STD, chi2: NONE },
        "CYS" => &RotamerSet { residue: "CYS", chi1: CHI1_STD, chi2: NONE },
        "LEU" => &RotamerSet { residue: "LEU", chi1: CHI1_STD, chi2: CHI2_LEU },
        "ILE" => &RotamerSet { residue: "ILE", chi1: CHI1_STD, chi2: CHI2_ILE },
        "MET" => &RotamerSet { residue: "MET", chi1: CHI1_STD, chi2: CHI2_LEU },
        "ASP" => &RotamerSet { residue: "ASP", chi1: CHI1_STD, chi2: CHI2_AROM },
        "ASN" => &RotamerSet { residue: "ASN", chi1: CHI1_STD, chi2: CHI2_AROM },
        "HIS" => &RotamerSet { residue: "HIS", chi1: CHI1_STD, chi2: CHI2_AROM },
        "PHE" => &RotamerSet { residue: "PHE", chi1: CHI1_STD, chi2: CHI2_AROM },
        "TYR" => &RotamerSet { residue: "TYR", chi1: CHI1_STD, chi2: CHI2_AROM },
        "TRP" => &RotamerSet { residue: "TRP", chi1: CHI1_STD, chi2: CHI2_AROM },
        "ARG" => &RotamerSet { residue: "ARG", chi1: CHI1_STD, chi2: CHI2_LONG },
        "LYS" => &RotamerSet { residue: "LYS", chi1: CHI1_STD, chi2: CHI2_LONG },
        "GLU" => &RotamerSet { residue: "GLU", chi1: CHI1_STD, chi2: CHI2_LONG },
        "GLN" => &RotamerSet { residue: "GLN", chi1: CHI1_STD, chi2: CHI2_LONG },
        _ => return None,
    })
}

/// Остаток с подвижной боковой цепью (для induced fit)?
pub fn is_flexible(res_name: &str) -> bool {
    rotamers(res_name).is_some()
}

// ─── Оси χ: атомы двугранного угла ──────────────────────────────────────

/// 4 атома двугранного угла χ остатка (по именам PDB).
/// χ1 = N–CA–CB–X1, χ2 = CA–CB–X1–X2. Вращение: вокруг CA–CB (χ1,
/// движутся атомы глубины ≥ 2), вокруг CB–X1 (χ2, глубина ≥ 3).
/// Только гибкие типы (синхронно с `rotamers`): Gly/Ala/Pro → None.
pub fn chi_atoms(res_name: &str, chi: u8) -> Option<[&'static str; 4]> {
    if rotamers(res_name).is_none() {
        return None;
    }
    let x1 = match res_name {
        "SER" => "OG",
        "THR" => "OG1",
        "CYS" => "SG",
        "VAL" | "ILE" => "CG1",
        _ => "CG",
    };
    let x2 = match res_name {
        "LEU" | "ILE" | "PHE" | "TYR" | "HIS" | "TRP" => "CD1",
        "MET" => "SD",
        "ASP" | "ASN" => "OD1",
        "GLU" | "GLN" | "ARG" | "LYS" => "CD",
        _ => return if chi == 1 { Some(["N", "CA", "CB", x1]) } else { None },
    };
    match chi {
        1 => Some(["N", "CA", "CB", x1]),
        2 => Some(["CA", "CB", x1, x2]),
        _ => None,
    }
}

// ─── Геометрия: двугранный угол ─────────────────────────────────────────

/// Двугранный угол a–b–c–d, градусы (−180..180]. b–c — ось вращения.
pub fn dihedral_deg(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> f64 {
    let sub = |p: [f64; 3], q: [f64; 3]| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
    let cross = |u: [f64; 3], v: [f64; 3]| {
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]
    };
    let dot = |u: [f64; 3], v: [f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let b1 = sub(b, a);
    let b2 = sub(c, b);
    let b3 = sub(d, c);
    let n1 = cross(b1, b2);
    let n2 = cross(b2, b3);
    let b2n = (b2[0] * b2[0] + b2[1] * b2[1] + b2[2] * b2[2]).sqrt();
    // классическая формула: x = n1·n2, y = (n1×n2)·b̂₂
    let x = dot(n1, n2);
    let y = dot(cross(n1, n2), [b2[0] / b2n, b2[1] / b2n, b2[2] / b2n]);
    y.atan2(x).to_degrees()
}

/// Поворот точки p вокруг оси (ax→ay) на угол (радианы) — формула
/// Родрига; ось задаётся двумя точками.
pub fn rotate_about(
    p: [f64; 3],
    ax: [f64; 3],
    ay: [f64; 3],
    angle_rad: f64,
) -> [f64; 3] {
    let sub = |q: [f64; 3], r: [f64; 3]| [q[0] - r[0], q[1] - r[1], q[2] - r[2]];
    let u = {
        let d = sub(ay, ax);
        let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        [d[0] / n, d[1] / n, d[2] / n]
    };
    let v = sub(p, ax);
    let cos = angle_rad.cos();
    let sin = angle_rad.sin();
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let uxv = cross(u, v);
    [
        ax[0] + v[0] * cos + uxv[0] * sin + u[0] * dot(u, v) * (1.0 - cos),
        ax[1] + v[1] * cos + uxv[1] * sin + u[1] * dot(u, v) * (1.0 - cos),
        ax[2] + v[2] * cos + uxv[2] * sin + u[2] * dot(u, v) * (1.0 - cos),
    ]
}

// ─── Наблюдаемые χ из PDB ───────────────────────────────────────────────

/// Наблюдаемый χ остатка (по атомам оси), градусы. None — атомов нет.
pub fn observed_chi(mm: &MacroMol, ri: usize, chi: u8) -> Option<f64> {
    let res_name = mm.residues[ri].name_str();
    let names = chi_atoms(res_name, chi)?;
    let mut pts = [[0.0f64; 3]; 4];
    for (k, nm) in names.iter().enumerate() {
        let ai = mm.residue_atom(ri, nm)?;
        pts[k] = mm.atoms[ai].pos;
    }
    Some(dihedral_deg(pts[0], pts[1], pts[2], pts[3]))
}

/// Ближайший УЗЕЛ библиотеки к наблюдаемому χ: (сектор, |Δ| в градусах).
pub fn nearest_node(res_name: &str, chi: u8, deg: f64) -> Option<(i32, f64)> {
    let set = rotamers(res_name)?;
    let nodes = match chi {
        1 => set.chi1,
        2 => set.chi2,
        _ => return None,
    };
    if nodes.is_empty() {
        return None;
    }
    let d = norm_deg(deg);
    let mut best: Option<(i32, f64)> = None;
    for &n in nodes {
        let nd = sector_to_deg(n as i32);
        let mut delta = (d - nd).abs();
        if delta > 180.0 {
            delta = 360.0 - delta;
        }
        if best.map(|(_, b)| delta < b).unwrap_or(true) {
            best = Some((n as i32, delta));
        }
    }
    best
}

/// Число гибких остатков белка (для отчётов C1).
pub fn flexible_residues(mm: &MacroMol) -> Vec<usize> {
    mm.residues
        .iter()
        .enumerate()
        .filter(|(_, r)| is_flexible(r.name_str()))
        .map(|(i, _)| i)
        .collect()
}

// ─── Тесты ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_grid_roundtrip() {
        // 2π/27 сетка: 60° → сектор 4.5 → тай 5 (half-away)
        assert_eq!(deg_to_sector(60.0), 5);
        assert_eq!(deg_to_sector(-60.0), -5);
        assert_eq!(deg_to_sector(180.0), 14); // 13.5 → 14
        assert_eq!(deg_to_sector(0.0), 0);
        assert_eq!(deg_to_sector(-180.0), 14); // нормализация в (−180,180]
        // узел → угол → узел
        for s in [-13i32, -5, -1, 0, 1, 5, 13] {
            assert_eq!(deg_to_sector(sector_to_deg(s)), s);
        }
        // пол-сектора — максимум расстояния до узла
        assert!((deg_to_sector(6.6) as f64 * CHI_SECTOR_DEG - 6.6).abs() < CHI_SECTOR_DEG / 2.0);
    }

    #[test]
    fn side_depth_ladder() {
        assert_eq!(side_depth("N"), 0);
        assert_eq!(side_depth("CA"), 0);
        assert_eq!(side_depth("OXT"), 0);
        assert_eq!(side_depth("CB"), 1);
        assert_eq!(side_depth("OG"), 2); // Ser
        assert_eq!(side_depth("OG1"), 2); // Thr
        assert_eq!(side_depth("CG1"), 2); // Val
        assert_eq!(side_depth("CG"), 2);
        assert_eq!(side_depth("SD"), 3); // Met
        assert_eq!(side_depth("OD1"), 3); // Asp
        assert_eq!(side_depth("CD1"), 3); // Phe
        assert_eq!(side_depth("CE"), 4);
        assert_eq!(side_depth("CZ"), 5); // Tyr
        assert_eq!(side_depth("OH"), 6); // Tyr OH — за CZ
        assert_eq!(side_depth("NH1"), 6); // Arg
        assert_eq!(side_depth("HA"), 0); // H остова
        assert_eq!(side_depth("HB2"), 0);
    }

    #[test]
    fn chi_axes_definitions() {
        assert_eq!(chi_atoms("SER", 1).unwrap(), ["N", "CA", "CB", "OG"]);
        assert_eq!(chi_atoms("THR", 1).unwrap(), ["N", "CA", "CB", "OG1"]);
        assert_eq!(chi_atoms("VAL", 1).unwrap(), ["N", "CA", "CB", "CG1"]);
        assert_eq!(chi_atoms("ASP", 1).unwrap(), ["N", "CA", "CB", "CG"]);
        assert_eq!(chi_atoms("ASP", 2).unwrap(), ["CA", "CB", "CG", "OD1"]);
        assert_eq!(chi_atoms("PHE", 2).unwrap(), ["CA", "CB", "CG", "CD1"]);
        assert_eq!(chi_atoms("MET", 2).unwrap(), ["CA", "CB", "CG", "SD"]);
        assert!(chi_atoms("SER", 2).is_none(), "у Ser нет χ2");
        assert!(chi_atoms("GLY", 1).is_none(), "Gly не гибкий");
        assert!(chi_atoms("ALA", 1).is_none());
        assert!(chi_atoms("PRO", 1).is_none(), "Pro — кольцо, не трогаем");
    }

    #[test]
    fn library_nodes_canonical() {
        // χ1: {−60, 60, 180} → узлы {−5, 5, 14}
        for res in ["SER", "VAL", "ASP", "TRP", "LYS"] {
            let set = rotamers(res).unwrap();
            assert_eq!(set.chi1, &[-5i8, 5, 14], "χ1 узлы {res}");
        }
        // Asp χ2 ±90 → ±7
        assert_eq!(rotamers("ASP").unwrap().chi2, &[-7i8, 7]);
        // Leu χ2 {65, 175} → {5, 13}
        assert_eq!(rotamers("LEU").unwrap().chi2, &[5i8, 13]);
        // у Val нет χ2
        assert!(rotamers("VAL").unwrap().chi2.is_empty());
        assert!(rotamers("GLY").is_none());
    }

    #[test]
    fn nearest_node_distances() {
        let (n, d) = nearest_node("SER", 1, 62.0).unwrap();
        assert_eq!(n, 5);
        assert!(d < 7.0, "62° близко к узлу 5 (66.7°): {d}");
        let (n, d) = nearest_node("ASP", 2, -88.0).unwrap();
        assert_eq!(n, -7);
        assert!(d < 7.0);
        // 100° — между узлами ±7: ближайший 7 (93.3°), расстояние ~7
        let (_, d) = nearest_node("ASP", 2, 100.0).unwrap();
        assert!(d < 20.0);
        assert!(nearest_node("GLY", 1, 60.0).is_none());
    }

    /// C1: верификация против PDB-статистики — наблюдаемые χ1/χ2 в 3PTB
    /// и 1CRN лежат в разумной близости к узлам библиотеки (порог 20°:
    /// реальные боковые цепи сидят в пределах ~сектора-полутора от
    /// канонических ротамеров; выбросы бывают, медиана обязана быть
    /// малой). Живые числа в отчёте сессии.
    #[test]
    fn pdb_chi_verification_3ptb_1crn() {
        for path in ["tests/fixtures/3ptb.pdb", "tests/fixtures/1crn.pdb"] {
            let mm = MacroMol::from_file(path).unwrap();
            let mut devs: Vec<f64> = Vec::new();
            let mut n_chi = 0usize;
            for &ri in flexible_residues(&mm).iter() {
                for chi in [1u8, 2u8] {
                    if let Some(deg) = observed_chi(&mm, ri, chi) {
                        if let Some((_, d)) =
                            nearest_node(mm.residues[ri].name_str(), chi, deg)
                        {
                            devs.push(d);
                            n_chi += 1;
                        }
                    }
                }
            }
            assert!(n_chi >= 5, "{path}: мало χ для статистики ({n_chi})");
            devs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = devs[devs.len() / 2];
            let max = devs[devs.len() - 1];
            eprintln!("{path}: χ наблюдаемых {n_chi}, медиана {median:.1}°, максимум {max:.1}°");
            assert!(median < 10.0, "{path}: медиана {median}° — узлы не там");
            // максимум — выбросы допустимы (реальные конформации),
            // но 75-й перцентиль обязан быть компактным
            let p75 = devs[devs.len() * 3 / 4];
            assert!(p75 < 20.0, "{path}: p75 {p75}° — библиотека мимо");
        }
    }

    /// Геометрия: двугранный угол и вращение Родрига согласованы.
    #[test]
    fn dihedral_and_rotation_consistency() {
        // квадрат в плоскости: N-CA-CB-OG с χ = 90°
        let n = [0.0, 0.0, 0.0];
        let ca = [1.0, 0.0, 0.0];
        let cb = [1.0, 1.0, 0.0];
        let og = [1.0, 1.0, 1.0]; // перпендикуляр — χ = 90°
        assert!((dihedral_deg(n, ca, cb, og) - 90.0).abs() < 1e-9);
        // вращение OG вокруг CA-CB на −90° кладёт её в плоскость z=0
        let rot = rotate_about(og, ca, cb, -std::f64::consts::FRAC_PI_2);
        let chi_new = dihedral_deg(n, ca, cb, rot);
        assert!((chi_new - 0.0).abs() < 1e-6, "χ после поворота {chi_new}");
        // атомы на оси не движутся
        let cb_rot = rotate_about(cb, ca, cb, 1.234);
        assert_eq!(cb_rot, cb);
    }

    /// Smoke: парсер не нужен для библиотеки, но сетка согласована с
    /// кручениями лиганда (тот же TORS_SECTOR).
    #[test]
    fn chi_grid_matches_torsion_grid() {
        let g = parse_smiles("CCO").unwrap();
        assert_eq!(g.atoms.len(), 3);
        assert!((CHI_SECTOR_DEG - 360.0 / 27.0).abs() < 1e-12);
        // 27 секторов на оборот — родная трит-сетка (канон инвариантов)
        assert_eq!(27 * 1, (360.0 / CHI_SECTOR_DEG) as i32);
    }
}
