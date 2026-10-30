//! Гироскоп памяти (RQ10, уровень 4): кососимметричный резонансный
//! оператор `J = A − Aᵀ` и фазовые векторы смысла Im(P) — без O(N²).
//!
//! ## Потоковая физика аккумулятора
//!
//! Токены потока — это события «дуга i появилась раньше дуги j».
//! Для каждого события в скользящем окне `W` каждый предшественник `i`
//! получает направленный знаковый вклад `A[i→j] += s_i·s_j`
//! (полярность s — из хеша токена, как в TF-IDF-кодировщике).
//! Стоимость — O(W) на токен при окне W: полный тензор пар N×N
//! **не строится никогда**.
//!
//! Антисимметричная часть отделяет циркуляцию смысла от ассоциации:
//!
//! ```text
//! J = A − Aᵀ,   Jᵀ = −J,   J[i][j] > 0 ⟺ смысл течёт i → j
//! ```
//!
//! ε-ворота — относительные, как в LENS-фильтре фаз: хранится только
//! `|J[i][j]| ≥ ε·max|J|` — масштаб циркуляции задаёт самая сильная
//! пара, шум отмирает вместе с ростом доминирующих русел. Выжившие
//! пары квантуются в i8 (масштаб `max|J| ↔ 127`) и занимают верхний
//! треугольник топологической секции контейнера v3 (`POLER_Q3`):
//! **5 байт на пару** — бинарник весит копейки.
//!
//! ## Резонансные моды: Im(P) — фазовые векторы смысла
//!
//! Кососимметричная `J` порождает чисто мнимые собственные пары `±iλ`:
//! генератор `iJ` эрмитов, поток `e^(−iηJ)` унитарен — гироскоп
//! **вращает** фазы памяти, не разрушая амплитуды. Каждая мода —
//! плоскость вращения `(u_k, v_k)`:
//!
//! ```text
//! J·u_k = +λ_k·v_k,    J·v_k = −λ_k·u_k
//! ```
//!
//! Моды извлекаются степенной итерацией на симметричной `−J²`
//! (собственные значения `λ_k²`) с дефляцией плоскостей — O(nnz(J))
//! на итерацию, без диагонализации и без материализации матрицы.
//! Дуги, входящие в `u_k`/`v_k`, получают фазовые углы Блоха
//! `θ_i = arccos(p_i)` из фазовой секции контейнера — это и есть
//! фазовые векторы смысла Im(P): смысл дуги `i` прецессирует в
//! плоскости моды `k` с угловой скоростью `λ_k`.
//!
//! ## Прецессия памяти R[n]
//!
//! Транспорт фазы (эйлеров расклад унитарного потока, первый порядок):
//!
//! ```text
//! θ_i ← θ_i − η·Σ_j J[i][j]·sin(θ_j − θ_i)      (направленный Курамото)
//! ```
//!
//! `|ψ_i| = |e^{iθ_i}| = 1` сохраняется точно — гироскоп не забывает.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::{BuildHasher, Hasher};

use pqw::gyro::GyroData;

/// Быстрый hasher u64-ключей (multiply-xor): аккумулятор делает
/// O(W) вставок на токен — SipHash здесь неоправданно дорог.
#[derive(Default, Clone, Copy)]
struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn mix(&mut self, v: u64) {
        self.hash = (self.hash.rotate_left(5) ^ v).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
    fn write(&mut self, bytes: &[u8]) {
        // Медленный путь для составных ключей (не используется в горячем коде).
        for &b in bytes {
            self.mix(b as u64);
        }
    }
    #[inline]
    fn write_u8(&mut self, v: u8) {
        self.mix(v as u64);
    }
    #[inline]
    fn write_u32(&mut self, v: u32) {
        self.mix(v as u64);
    }
    #[inline]
    fn write_u64(&mut self, v: u64) {
        self.mix(v);
    }
    #[inline]
    fn write_usize(&mut self, v: usize) {
        self.mix(v as u64);
    }
}

#[derive(Default, Clone, Copy)]
struct FxBuild;

impl BuildHasher for FxBuild {
    type Hasher = FxHasher;
    fn build_hasher(&self) -> FxHasher {
        FxHasher { hash: 0 }
    }
}

type FxMap<K, V> = HashMap<K, V, FxBuild>;

#[inline]
fn pack_pair(a: u32, b: u32) -> u64 {
    (a as u64) << 32 | b as u64
}

#[inline]
fn unpack_pair(k: u64) -> (u32, u32) {
    ((k >> 32) as u32, k as u32)
}

/// Одна резонансная мода гироскопа: плоскость вращения `(u, v)`
/// с угловой скоростью `λ`.
///
/// RQ11: мода — это **архетип** в смысле уравнения `a ⊗_ε a = a`:
/// спектральный проектор `A_k = u_k u_kᵀ + v_k v_kᵀ` идемпотентен
/// `A_k² = A_k` тогда и только тогда, когда `(u, v)` ортонормированы —
/// невязка ортонормальности и есть невязка идемпотентности.
#[derive(Clone, Debug, PartialEq)]
pub struct GyroMode {
    /// Угловая скорость моды: собственная пара `±iλ` оператора J.
    pub lambda: f64,
    /// Первый вектор плоскости: `J·u = +λ·v`. Компоненты ≥ 10% максимума.
    pub u: Vec<(u32, f64)>,
    /// Второй вектор плоскости: `J·v = −λ·u`.
    pub v: Vec<(u32, f64)>,
    /// Ritz-невязка `max(‖Ju − λv‖₂, ‖Jv + λu‖₂)/λ` — насколько
    /// плоскость действительно инвариантна относительно `J`.
    pub ritz_residual: f64,
    /// Невязка ортонормальности `|u·v| + |‖u‖−1| + |‖v‖−1|` —
    /// тождественно невязка идемпотентности `A² = A` проектора
    /// `A = uuᵀ + vvᵀ` (уравнение архетипа).
    pub ortho_residual: f64,
    /// Доля массы плоскости в sparse-представлении:
    /// `(‖u_kept‖² + ‖v_kept‖²)/2` — «захват» архетипа доминирующими
    /// компонентами (≤ 12 на вектор).
    pub capture: f64,
}

/// Максимальное число итераций степенной процедуры на моду.
const MAX_POWER_ITER: usize = 128;
/// Порог отсечения компонент моды (доля от максимума |c|).
const MODE_COMPONENT_FLOOR: f64 = 0.1;
/// Потолок компонент моды в sparse-представлении.
const MODE_COMPONENT_CAP: usize = 12;

/// Потоковый гироскоп памяти: окно направленного контекста + бюджет пар.
///
/// Builder-контракт: [`Gyroscope::new`] → [`Gyroscope::observe`]\* →
/// [`Gyroscope::skew_pairs`]/[`Gyroscope::gyro_data`]/
/// [`Gyroscope::resonant_modes`]. Детерминирован: одинаковая
/// последовательность событий → одинаковое состояние.
pub struct Gyroscope {
    window: usize,
    budget: usize,
    ring: VecDeque<(u32, f64)>,
    flow: FxMap<u64, f64>,
    ticks: u64,
    prune_ctr: usize,
    prune_every: usize,
}

impl Gyroscope {
    /// Новый гироскоп: окно `W ≥ 1` (дальность направленного контекста),
    /// бюджет `budget ≥ 16` пар (потолок HashMap после прореживания).
    pub fn new(window: usize, budget: usize) -> Gyroscope {
        Gyroscope {
            window: window.max(1),
            budget: budget.max(16),
            ring: VecDeque::with_capacity(window.max(1).min(4096)),
            flow: FxMap::default(),
            ticks: 0,
            prune_ctr: 0,
            prune_every: 4096,
        }
    }

    /// Одно событие потока: координата + фазовая полярность `±1`.
    ///
    /// Каждый предшественник окна получает вклад в направленный вес;
    /// сама координата отправляется в окно, самое старое событие
    /// (за пределами W) выпадает.
    pub fn observe(&mut self, coord: u32, sign: f64) {
        self.ticks += 1;
        let (head, tail) = self.ring.as_slices();
        for slice in [head, tail] {
            for &(c, s) in slice.iter() {
                if c != coord {
                    let key = pack_pair(c, coord);
                    // Порядок «предшественник → новичок»: направление потока смысла.
                    *self.flow.entry(key).or_insert(0.0) += s * sign;
                }
            }
        }
        self.ring.push_back((coord, sign));
        if self.ring.len() > self.window {
            self.ring.pop_front();
        }
        self.prune_ctr += 1;
        if self.prune_ctr >= self.prune_every {
            self.prune_ctr = 0;
            self.prune();
        }
    }

    /// Прогон последовательности событий `(координата, полярность)`.
    pub fn observe_seq(&mut self, events: &[(u32, f64)]) {
        for &(c, s) in events {
            self.observe(c, s);
        }
    }

    /// Resume: впитать пары `J` из контейнера v3 (консолидированная
    /// память) + счётчик тактов. Возвращает число впитанных пар.
    ///
    /// Деквантованный вес входит как готовое мнение: `w > 0 → A[i→j] += w`,
    /// `w < 0 → A[j→i] += |w|` — гироскоп продолжает накапливать поверх.
    pub fn absorb(&mut self, pairs: &[(u32, u32, f64)], ticks: u64) -> usize {
        let n = pairs.len();
        for &(i, j, w) in pairs {
            if w >= 0.0 {
                *self.flow.entry(pack_pair(i, j)).or_insert(0.0) += w;
            } else {
                *self.flow.entry(pack_pair(j, i)).or_insert(0.0) += -w;
            }
        }
        self.ticks = self.ticks.max(ticks);
        n
    }

    /// Потолок HashMap: остаются `budget` пар с наибольшими |весами|.
    fn prune(&mut self) {
        if self.flow.len() <= self.budget {
            return;
        }
        let mut order: Vec<u64> = self.flow.keys().copied().collect();
        let keep_from = order.len() - self.budget;
        order.select_nth_unstable_by(keep_from, |&a, &b| {
            self.flow[&a]
                .abs()
                .total_cmp(&self.flow[&b].abs())
        });
        let mut kept: FxMap<u64, f64> = FxMap::default();
        kept.reserve(self.budget);
        for &k in &order[keep_from..] {
            kept.insert(k, self.flow[&k]);
        }
        self.flow = kept;
    }

    /// Счётчик тактов — всего наблюдённых событий потока.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Окно направленного контекста W.
    pub fn window(&self) -> usize {
        self.window
    }

    /// Число сырых направленных пар в памяти (до ε-ворот).
    pub fn raw_pairs(&self) -> usize {
        self.flow.len()
    }

    /// `J = A − Aᵀ` с ε-воротами: возвращает верхний треугольник
    /// `(i < j, J[i][j])`, отсортированный по `(i, j)`.
    ///
    /// Ворота — относительные, как в LENS-фильтре фаз: после вычисления
    /// всех ненулевых `J` остаётся только `|J| ≥ ε·max|J|` (масштаб
    /// циркуляции задаёт самая сильная пара). Идеально симметричные
    /// пары (`J = 0` — ассоциация без циркуляции) не хранятся вовсе;
    /// слабые пары ниже порога умирают вместе с ростом доминирующего
    /// потока — гироскоп помнит главные русла смысла.
    pub fn skew_pairs(&self, epsilon: f64) -> Vec<(u32, u32, f64)> {
        let mut all: BTreeMap<(u32, u32), f64> = BTreeMap::new();
        let mut max_j = 0.0_f64;
        for (&k, &w_ab) in self.flow.iter() {
            let (a, b) = unpack_pair(k);
            let w_ba = self.flow.get(&pack_pair(b, a)).copied().unwrap_or(0.0);
            let j = w_ab - w_ba;
            if j == 0.0 {
                continue;
            }
            max_j = max_j.max(j.abs());
            if a < b {
                all.insert((a, b), j);
            } else {
                all.insert((b, a), -j);
            }
        }
        if max_j == 0.0 {
            return Vec::new();
        }
        let floor = max_j * epsilon.clamp(0.0, 1.0);
        all.into_iter()
            .filter(|&(_, w)| w.abs() >= floor)
            .map(|((i, j), w)| (i, j, w))
            .collect()
    }

    /// Данные для топологической секции контейнера v3 (или `None`,
    /// если после ε-ворот не осталось ни одной пары).
    pub fn gyro_data(&self, epsilon: f64, d_pol: u32) -> Option<GyroData> {
        let pairs = self.skew_pairs(epsilon);
        if pairs.is_empty() {
            return None;
        }
        GyroData::new(self.window as u32, self.ticks, pairs, d_pol).ok()
    }

    /// Резонансные моды ε-выжившего гироскопа: топ-K плоскостей
    /// вращения по убыванию λ (см. схему модуля).
    pub fn resonant_modes(&self, epsilon: f64, k: usize) -> Vec<GyroMode> {
        resonant_modes_from_pairs(&self.skew_pairs(epsilon), k)
    }
}

/// Резонансные моды по явному верхнему треугольнику `J` (например,
/// деквантованному из контейнера v3): степенная итерация на `−J²`
/// с дефляцией найденных плоскостей.
///
/// Подпространство — только дуги, входящие в пары: размер задачи
/// `O(число пар)`, а не `O(d_pol)`; полная матрица не материализуется.
pub fn resonant_modes_from_pairs(pairs: &[(u32, u32, f64)], k: usize) -> Vec<GyroMode> {
    let (nodes, modes) = dense_modes_local(pairs, k);
    modes
        .into_iter()
        .map(|(lambda, u, v, ritz_residual, ortho_residual)| {
            let su = sparsify(&nodes, &u);
            let sv = sparsify(&nodes, &v);
            // Захват: доля массы плоскости в ≤12 компонентах на вектор.
            let capture = (su.iter().map(|&(_, c)| c * c).sum::<f64>()
                + sv.iter().map(|&(_, c)| c * c).sum::<f64>())
                / 2.0;
            GyroMode {
                lambda,
                u: su,
                v: sv,
                ritz_residual,
                ortho_residual,
                capture,
            }
        })
        .collect()
}

/// Плотная мода гироскопа в **полном** пространстве `d_pol` (RQ12:
/// крипто-проектор алгебры архетипа). Вне дуг русел компоненты 0.
#[derive(Clone, Debug)]
pub struct DenseMode {
    /// Угловая скорость моды (собственная пара ±iλ J).
    pub lambda: f64,
    /// Полный вектор u плоскости (d_pol компонент).
    pub u: Vec<f64>,
    /// Полный вектор v плоскости (d_pol компонент).
    pub v: Vec<f64>,
    /// Ritz-невязка инвариантности плоскости.
    pub ritz_residual: f64,
    /// Невязка ортонормальности ≡ невязка идемпотентности A² = A.
    pub ortho_residual: f64,
}

/// Плотные моды в полном пространстве: для крипто-схемы RQ12, где
/// проектор `a = Σ (u uᵀ + v vᵀ)` обязан быть точным идемпотентом
/// (прореженные моды теряют массу — захват 56–75% — и идемпотентность
/// разрушается). Детерминизм тот же, что у `resonant_modes_from_pairs`.
pub fn resonant_modes_dense_from_pairs(
    pairs: &[(u32, u32, f64)],
    k: usize,
    d_pol: usize,
) -> Vec<DenseMode> {
    let (nodes, modes) = dense_modes_local(pairs, k);
    modes
        .into_iter()
        .map(|(lambda, u, v, ritz_residual, ortho_residual)| {
            let embed = |local: &[f64]| -> Vec<f64> {
                let mut full = vec![0.0_f64; d_pol];
                for (li, &node) in nodes.iter().enumerate() {
                    if (node as usize) < d_pol {
                        full[node as usize] = local[li];
                    }
                }
                full
            };
            DenseMode {
                lambda,
                u: embed(&u),
                v: embed(&v),
                ritz_residual,
                ortho_residual,
            }
        })
        .collect()
}

/// Ядро степенной итерации на −J² с дефляцией: локальное подпространство
/// дуг пар + плотные (u, v) в локальных индексах.
fn dense_modes_local(
    pairs: &[(u32, u32, f64)],
    k: usize,
) -> (Vec<u32>, Vec<(f64, Vec<f64>, Vec<f64>, f64, f64)>) {
    if pairs.is_empty() || k == 0 {
        return (Vec::new(), Vec::new());
    }

    // Локальное подпространство.
    let mut nodes: Vec<u32> = pairs.iter().flat_map(|&(i, j, _)| [i, j]).collect();
    nodes.sort_unstable();
    nodes.dedup();
    let n = nodes.len();
    let mut index: FxMap<u32, usize> = FxMap::default();
    index.reserve(n);
    for (li, &node) in nodes.iter().enumerate() {
        index.insert(node, li);
    }

    // Списки смежности J (локальные индексы, оба направления).
    let mut nbr: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    for &(i, j, w) in pairs {
        let (li, lj) = (index[&i], index[&j]);
        nbr[li].push((lj, w));
        nbr[lj].push((li, -w));
    }

    let matvec = |x: &[f64], y: &mut Vec<f64>| {
        y.clear();
        y.resize(n, 0.0);
        for (i, row) in nbr.iter().enumerate() {
            let xi = x[i];
            if xi == 0.0 && row.is_empty() {
                continue;
            }
            let mut acc = 0.0;
            for &(j, w) in row {
                acc += w * x[j];
            }
            y[i] = acc;
        }
    };

    let l2 = |x: &[f64]| x.iter().map(|c| c * c).sum::<f64>().sqrt();
    let normalize = |x: &mut [f64]| {
        let norm = l2(x);
        if norm > 0.0 {
            for c in x.iter_mut() {
                *c /= norm;
            }
        }
        norm
    };

    let mut modes: Vec<(f64, Vec<f64>, Vec<f64>, f64, f64)> = Vec::new();
    let mut found_u: Vec<Vec<f64>> = Vec::new();
    let mut found_v: Vec<Vec<f64>> = Vec::new();

    // Проекция из найденных плоскостей (Грам-Шмидт по обеим осям).
    let project_out = |x: &mut [f64], us: &[Vec<f64>], vs: &[Vec<f64>]| {
        for _ in 0..2 {
            for u in us.iter().chain(vs.iter()) {
                let dot: f64 = x.iter().zip(u.iter()).map(|(a, b)| a * b).sum();
                if dot != 0.0 {
                    for (a, b) in x.iter_mut().zip(u.iter()) {
                        *a -= dot * b;
                    }
                }
            }
        }
    };

    let mut y = Vec::with_capacity(n);
    let mut z = Vec::with_capacity(n);

    for mode_no in 0..k {
        // Детерминированный старт: узор на основе индексов и номера моды.
        let mut x: Vec<f64> = (0..n)
            .map(|t| {
                1.0 + (((t.wrapping_mul(26_544_357_61))
                    .wrapping_add(mode_no.wrapping_mul(40_503)))
                    % 97) as f64
                    / 97.0
            })
            .collect();
        project_out(&mut x, &found_u, &found_v);
        if normalize(&mut x) < 1e-12 {
            break;
        }

        // Степенная итерация x ← (I−P)(−J²)(I−P) x.
        for _ in 0..MAX_POWER_ITER {
            matvec(&x, &mut y);
            project_out(&mut y, &found_u, &found_v);
            matvec(&y, &mut z);
            for c in z.iter_mut() {
                *c = -*c;
            }
            project_out(&mut z, &found_u, &found_v);
            if normalize(&mut z) < 1e-14 {
                break;
            }
            x.copy_from_slice(&z);
        }
        // Рэлеевское значение λ² = xᵀ(−J²)x на сошедшемся векторе.
        matvec(&x, &mut y);
        project_out(&mut y, &found_u, &found_v);
        matvec(&y, &mut z);
        let lambda2 = -x.iter().zip(z.iter()).map(|(a, b)| a * b).sum::<f64>();
        if !lambda2.is_finite() || lambda2 <= 1e-24 {
            break;
        }
        let lambda = lambda2.sqrt();

        // Плоскость моды: u = x, v = J·u / λ.
        let mut u = x;
        project_out(&mut u, &found_u, &found_v);
        if normalize(&mut u) < 1e-12 {
            break;
        }
        let mut v: Vec<f64> = Vec::new();
        matvec(&u, &mut v);
        for c in v.iter_mut() {
            *c /= lambda;
        }
        project_out(&mut v, &found_u, &found_v);
        let dot: f64 = u.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
        for (a, b) in v.iter_mut().zip(u.iter()) {
            *a -= dot * b;
        }
        if normalize(&mut v) < 1e-12 {
            break;
        }

        // Фиксация знака: максимальная по модулю компонента u положительна.
        let mut max_i = 0usize;
        let mut max_c = 0.0_f64;
        for (i, &c) in u.iter().enumerate() {
            if c.abs() > max_c {
                max_c = c.abs();
                max_i = i;
            }
        }
        if u[max_i] < 0.0 {
            for c in u.iter_mut() {
                *c = -*c;
            }
            for c in v.iter_mut() {
                *c = -*c;
            }
        }

        // RQ11: невязки плоскости — численная верификация уравнения
        // архетипа `a ⊗_ε a = a` для спектрального проектора A = uuᵀ + vvᵀ.
        //
        // Ritz: плоскость обязана удовлетворять Ju = +λv, Jv = −λu
        // (инвариантность относительно J) — иначе это не плоскость вращения.
        let mut ju = Vec::with_capacity(n);
        matvec(&u, &mut ju);
        let mut jv = Vec::with_capacity(n);
        matvec(&v, &mut jv);
        let l_safe = lambda.max(1e-12);
        let ritz_u = ju
            .iter()
            .zip(v.iter())
            .map(|(&a, &b)| {
                let d = a - lambda * b;
                d * d
            })
            .sum::<f64>()
            .sqrt()
            / l_safe;
        let ritz_v = jv
            .iter()
            .zip(u.iter())
            .map(|(&a, &b)| {
                let d = a + lambda * b;
                d * d
            })
            .sum::<f64>()
            .sqrt()
            / l_safe;
        let ritz = ritz_u.max(ritz_v);
        // Ортонормальность (u, v) ⟺ идемпотентность A = uuᵀ + vvᵀ:
        // A² = A распадается на ‖u‖=1, ‖v‖=1, u·v=0.
        let dot_uv: f64 = u.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
        let ortho = dot_uv.abs() + (l2(&u) - 1.0).abs() + (l2(&v) - 1.0).abs();

        found_u.push(u.clone());
        found_v.push(v.clone());
        modes.push((lambda, u, v, ritz, ortho));
    }

    (nodes, modes)
}

/// Sparse-представление вектора моды: доминирующие компоненты
/// (≥ 10% максимума, не более 12), по убыванию |компоненты|.
fn sparsify(nodes: &[u32], x: &[f64]) -> Vec<(u32, f64)> {
    let max = x.iter().map(|c| c.abs()).fold(0.0_f64, f64::max);
    if max <= 0.0 {
        return Vec::new();
    }
    let mut comps: Vec<(u32, f64)> = x
        .iter()
        .enumerate()
        .filter(|&(_, &c)| c.abs() >= MODE_COMPONENT_FLOOR * max)
        .map(|(i, &c)| (nodes[i], c))
        .collect();
    comps.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
    comps.truncate(MODE_COMPONENT_CAP);
    comps
}

/// Один шаг прецессии памяти R[n]: направленный Курамото-транспорт
/// фаз, порождаемый гироскопом J.
///
/// `thetas` — углы Блоха всех `d_pol` дуг (радианы); пары задают
/// верхний треугольник J. Стоимость — O(nnz(J)): никакого O(N²).
/// Модуль фазы `|e^{iθ}| = 1` сохраняется точно.
pub fn precess_step(pairs: &[(u32, u32, f64)], thetas: &mut [f64], eta: f64) {
    let mut delta = vec![0.0_f64; thetas.len()];
    for &(i, j, w) in pairs {
        let si = thetas[i as usize];
        let sj = thetas[j as usize];
        // θ̇_i = −η·J_ij·sin(θ_j − θ_i); для j — тот же вклад (J_ji = −w).
        let torque = eta * w * (sj - si).sin();
        delta[i as usize] -= torque;
        delta[j as usize] -= torque;
    }
    for (t, d) in thetas.iter_mut().zip(delta.iter()) {
        *t += d;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 0.05;

    /// MVR-v3 цикл D (docs/MVR_PROTOCOL.md): побитовая сверка формулы
    /// прецессии с кодом. НАПРАВЛЕННЫЙ Курамото-транспорт: оба конца ребра
    /// получают ОДИН И ТОТ ЖЕ вклад `−torque` (комментарий L637: «для j —
    /// тот же вклад, J_ji = −w»). Следствия, проверяемые точно:
    ///
    /// 1. Инвариант ребра: для одиночной пары (i,j) разность θ_j − θ_i
    ///    не меняется её собственным моментом (lockstep) — в вещественной
    ///    арифметике точно, в FP — с допуском накопленного округления.
    /// 2. Глобального инварианта Σθ у направленного транспорта НЕТ
    ///    (в отличие от симметричного Курамото ±torque) — задокументировано
    ///    в паспорте цикла D; норма сохраняется ЛИНЕЙНЫМ ротором J = A − Aᵀ
    ///    (теорема I.1, sympy/numpy-верификатор), а не фазовым транспортом.
    #[test]
    fn precess_step_edge_lockstep_preserves_pair_difference() {
        // 1) Одиночная пара: разность фаз инвариантна в вещественной
        // арифметике (lockstep); в FP — с допуском накопленного округления
        // (против грубого нарушения, если бы транспорт был не lockstep).
        let mut thetas = vec![0.3, -1.2, 2.4];
        let d0 = thetas[1] - thetas[0];
        for _ in 0..1000 {
            precess_step(&[(0, 1, 0.7)], &mut thetas, 0.37);
        }
        let scale = thetas.iter().fold(0.0f64, |m, t| m.max(t.abs())).max(1.0);
        assert!(
            (thetas[1] - thetas[0] - d0).abs() <= 1e-6 * scale,
            "lockstep нарушен для одиночной пары"
        );

        // 2) Две непересекающиеся пары: каждая разность инвариантна точно.
        let mut th = vec![0.3, -1.2, 2.4, 0.9, -2.8];
        let (d02, d34) = (th[2] - th[0], th[4] - th[3]);
        let pairs = vec![(0u32, 2u32, -1.3f64), (3u32, 4u32, 2.1f64)];
        for _ in 0..1000 {
            precess_step(&pairs, &mut th, 0.9);
        }
        let scale2 = th.iter().fold(0.0f64, |m, t| m.max(t.abs())).max(1.0);
        assert!((th[2] - th[0] - d02).abs() <= 1e-6 * scale2);
        assert!((th[4] - th[3] - d34).abs() <= 1e-6 * scale2);

        // 3) Формула против независимой реплики: θ̇_k = −η·Σ J_km·sin(θ_m−θ_k),
        //    J_ij = w, J_ji = −w — ОБА конца получают −torque (строки L639-640).
        let pairs3: Vec<(u32, u32, f64)> =
            vec![(0, 1, 0.7), (1, 2, -1.3), (0, 3, 2.1), (2, 3, 0.4)];
        let mut a = vec![0.3, -1.2, 2.4, 0.9];
        let mut b = a.clone();
        let eta = 0.37f64;
        for _ in 0..1000 {
            precess_step(&pairs3, &mut a, eta);
            let mut d = vec![0.0f64; b.len()];
            for &(i, j, w) in &pairs3 {
                let t = eta * w * (b[j as usize] - b[i as usize]).sin();
                d[i as usize] -= t;
                d[j as usize] -= t; // направленный транспорт: тот же вклад
            }
            for (t, dd) in b.iter_mut().zip(d.iter()) {
                *t += dd;
            }
        }
        assert_eq!(a, b, "расхождение формулы и независимой реплики");
    }

    #[test]
    fn window_binds_pairing_range() {
        // W = 2: новое событие спаривается с 2 предыдущими, не дальше.
        let mut g = Gyroscope::new(2, 1024);
        g.observe_seq(&[(10, 1.0), (20, 1.0), (30, 1.0)]);
        assert!(g.flow.contains_key(&pack_pair(10, 20)));
        assert!(g.flow.contains_key(&pack_pair(10, 30))); // 10 ещё в окне
        assert!(g.flow.contains_key(&pack_pair(20, 30)));
        // 40-е событие: 10 уже вытеснен из окна — пары 10→40 нет.
        g.observe_seq(&[(40, 1.0)]);
        assert!(!g.flow.contains_key(&pack_pair(10, 40)));
        assert!(g.flow.contains_key(&pack_pair(20, 40)));
        assert!(g.flow.contains_key(&pack_pair(30, 40)));
        // W = 1: спаривание только с непосредственным предшественником.
        let mut g1 = Gyroscope::new(1, 1024);
        g1.observe_seq(&[(10, 1.0), (20, 1.0), (30, 1.0)]);
        assert!(g1.flow.contains_key(&pack_pair(10, 20)));
        assert!(g1.flow.contains_key(&pack_pair(20, 30)));
        assert!(!g1.flow.contains_key(&pack_pair(10, 30)));
        assert_eq!(g.ticks(), 4);
    }

    #[test]
    fn contributions_are_signed() {
        // W = 1: спаривание только с непосредственным предшественником —
        // вклады считаются точно.
        let mut g = Gyroscope::new(1, 1024);
        g.observe_seq(&[(1, 1.0), (2, -1.0)]);
        assert_eq!(g.flow[&pack_pair(1, 2)], -1.0);
        // Повтор с теми же знаками удваивает вклад.
        g.observe_seq(&[(1, 1.0), (2, -1.0)]);
        assert_eq!(g.flow[&pack_pair(1, 2)], -2.0);
        // Обратное направление — свежий гироскоп, чистый след.
        let mut g2 = Gyroscope::new(1, 1024);
        g2.observe_seq(&[(2, 1.0), (1, 1.0)]);
        assert_eq!(g2.flow[&pack_pair(2, 1)], 1.0);
    }

    #[test]
    fn self_pairing_skipped() {
        let mut g = Gyroscope::new(8, 1024);
        g.observe_seq(&[(5, 1.0), (5, 1.0), (5, -1.0)]);
        assert!(g.flow.is_empty());
        assert_eq!(g.ticks(), 3);
    }

    #[test]
    fn skew_gate_keeps_directed_flow() {
        // Разделители-события (каждый раз новый номер) не дают обратному
        // потоку 20→10 накапливаться: чистая циркуляция 10→20.
        let mut g = Gyroscope::new(1, 1024);
        for round in 0..10u32 {
            g.observe_seq(&[(10, 1.0), (20, 1.0), (100 + round, 1.0)]);
        }
        // A[10→20] = 10, A[20→10] = 0: J = 10 — доминирующее русло.
        // Пар с разделителями — по 1 наблюдению (J = ±1): ε = 0.15
        // отсекает их как шум ниже 15% от max|J| = 10.
        let pairs = g.skew_pairs(0.15);
        assert_eq!(pairs, vec![(10, 20, 10.0)]);
        // При ε = 0.05 (порог 0.5) слабые пары тоже выживают — но русло 10→20
        // остаётся самой сильной циркуляцией.
        let loose = g.skew_pairs(0.05);
        assert!(loose.len() > 1);
        let strongest = loose
            .iter()
            .max_by(|a, b| a.2.abs().total_cmp(&b.2.abs()))
            .unwrap();
        assert_eq!(*strongest, (10, 20, 10.0));
    }

    #[test]
    fn symmetric_pairing_cancels() {
        // Вперёд-назад поровну: J = 0, ассоциация без циркуляции —
        // гироскоп такие пары не хранит вовсе (даже при ε = 0).
        let mut g = Gyroscope::new(1, 1024);
        for _ in 0..5 {
            g.observe_seq(&[(1, 1.0), (2, 1.0)]);
            g.observe_seq(&[(2, 1.0), (1, 1.0)]);
        }
        assert!(g.skew_pairs(0.0).is_empty());
    }

    #[test]
    fn relative_gate_scales_with_max() {
        // Слабая пара умирает, когда появляется доминирующее русло.
        let mut g = Gyroscope::new(1, 1024);
        // Слабая циркуляция 1→2 (J = 1).
        for round in 0..2u32 {
            g.observe_seq(&[(1, 1.0), (2, 1.0), (200 + round, 1.0)]);
        }
        assert_eq!(g.skew_pairs(0.05).len() >= 1, true);
        // Появляется сильное русло 7→8 (J = 20): порог 15% = 3 — слабая
        // пара (J = 2) и пары с разделителями (J = 1) отмирают.
        for round in 0..20u32 {
            g.observe_seq(&[(7, 1.0), (8, 1.0), (300 + round, 1.0)]);
        }
        let pairs = g.skew_pairs(0.15);
        assert_eq!(pairs, vec![(7, 8, 20.0)]);
    }

    #[test]
    fn upper_triangle_orientation() {
        // Направление 30→20 (индексы в убывании) → верхний треугольник (20, 30, −w).
        let mut g = Gyroscope::new(1, 1024);
        for round in 0..4u32 {
            g.observe_seq(&[(30, 1.0), (20, 1.0), (500 + round, 1.0)]);
        }
        let pairs = g.skew_pairs(0.3);
        assert_eq!(pairs, vec![(20, 30, -4.0)]);
    }

    #[test]
    fn budget_prunes_to_top_pairs() {
        let mut g = Gyroscope::new(64, 16);
        g.prune_every = 8;
        // 40 разных координат попарно: до прореживания сотни пар.
        for round in 0..40u32 {
            g.observe_seq(&[(round, 1.0), (round + 100, 1.0), (round + 200, 1.0)]);
        }
        assert!(g.raw_pairs() <= 16, "raw={} budget=16", g.raw_pairs());
        // Такты не теряются при прореживании.
        assert_eq!(g.ticks(), 120);
    }

    #[test]
    fn gyro_data_and_absorb_roundtrip() {
        // Чистая циркуляция через разделители + слабое обратное течение.
        let mut g = Gyroscope::new(1, 1024);
        for round in 0..7u32 {
            g.observe_seq(&[(3, 1.0), (8, 1.0), (900 + round, 1.0)]);
        }
        for round in 0..2u32 {
            g.observe_seq(&[(8, 1.0), (3, 1.0), (950 + round, 1.0)]);
        }
        // J[3→8] = 7−2 = 5; пары с разделителями (J=±1) — ниже порога 0.25·5.
        let pairs = g.skew_pairs(0.25);
        assert_eq!(pairs, vec![(3, 8, 5.0)]);
        let data = g.gyro_data(0.25, 4096).expect("циркуляция должна выжить");
        assert_eq!(data.window(), 1);
        assert_eq!(data.ticks(), g.ticks());
        assert_eq!(data.pairs().len(), 1);

        // Resume: свежий гироскоп впитывает консолидированную память.
        let mut g2 = Gyroscope::new(1, 1024);
        let absorbed = g2.absorb(data.pairs(), data.ticks());
        assert_eq!(absorbed, 1);
        assert_eq!(g2.ticks(), g.ticks());
        assert_eq!(g2.skew_pairs(0.25), g.skew_pairs(0.25));
    }

    #[test]
    fn mode_of_single_pair() {
        // Одиночная пара (i,j,w): λ = |w|; плоскость (u,v) — в точности
        // подпространство {e_i, e_j} с инвариантами J·u = λ·v, J·v = −λ·u.
        // (Для одной пары −J² = w²I вырождена: u — произвольный вектор
        // плоскости, поэтому проверяем инварианты, а не e_i/e_j.)
        let comp = |v: &[(u32, f64)], n: u32| {
            v.iter()
                .find(|&&(m, _)| m == n)
                .map(|&(_, c)| c)
                .unwrap_or(0.0)
        };
        for w in [2.5_f64, -2.5] {
            let modes = resonant_modes_from_pairs(&[(4, 9, w)], 1);
            assert_eq!(modes.len(), 1);
            let m = &modes[0];
            assert!((m.lambda - w.abs()).abs() < 1e-9, "lambda {}", m.lambda);
            // Энергия плоскости целиком на дугах 4 и 9.
            let (u4, u9) = (comp(&m.u, 4), comp(&m.u, 9));
            let (v4, v9) = (comp(&m.v, 4), comp(&m.v, 9));
            assert!((u4 * u4 + v4 * v4 - 1.0).abs() < 1e-9, "энергия дуги 4");
            assert!((u9 * u9 + v9 * v9 - 1.0).abs() < 1e-9, "энергия дуги 9");
            // Точная пара инвариантов кососимметричной моды.
            assert!((w * u9 - m.lambda * v4).abs() < 1e-9, "(Ju)_4 ≠ λv_4");
            assert!((-w * u4 - m.lambda * v9).abs() < 1e-9, "(Ju)_9 ≠ λv_9");
            assert!((u4 * v4 + u9 * v9).abs() < 1e-9, "u·v ≠ 0");
        }
    }

    #[test]
    fn mode_residuals_on_exact_plane() {
        // RQ11: одиночная пара — точная плоскость вращения. Ritz- и
        // орто-невязки ~ машинной точности, идемпотентность A² = A
        // проектора A = uuᵀ + vvᵀ выполняется точно; захват = 1.
        for w in [5.0_f64, -5.0, 0.5] {
            let modes = resonant_modes_from_pairs(&[(0u32, 1u32, w)], 1);
            assert_eq!(modes.len(), 1);
            let m = &modes[0];
            assert!((m.lambda - w.abs()).abs() < 1e-9, "lambda {}", m.lambda);
            assert!(m.ritz_residual < 1e-9, "ritz = {}", m.ritz_residual);
            assert!(m.ortho_residual < 1e-12, "ortho = {}", m.ortho_residual);
            assert!(m.capture > 0.999, "capture = {}", m.capture);
        }
    }

    #[test]
    fn mode_residuals_triangle_circulation() {
        // RQ11: циркуляция 0→1→2→0. Спектр J: {0, ±i√3·w} — одна
        // плоскость вращения с λ = √3. Нулевая ось модой не является
        // (после дефляции λ² → 0, процедура останавливается).
        let pairs = vec![(0u32, 1u32, 1.0), (1u32, 2u32, 1.0), (0u32, 2u32, -1.0)];
        let modes = resonant_modes_from_pairs(&pairs, 2);
        assert_eq!(modes.len(), 1);
        let m = &modes[0];
        assert!((m.lambda - 3.0_f64.sqrt()).abs() < 1e-9, "lambda {}", m.lambda);
        assert!(m.ritz_residual < 1e-9, "ritz = {}", m.ritz_residual);
        assert!(m.ortho_residual < 1e-12, "ortho = {}", m.ortho_residual);
        // Все три дуги в моде равного веса — захват полный.
        assert!(m.capture > 0.999, "capture = {}", m.capture);
    }

    #[test]
    fn modes_deflate_in_lambda_order() {
        // Две непересекающиеся пары: моды разделяются и упорядочены по λ.
        let pairs = vec![(1u32, 2u32, 5.0), (7u32, 8u32, 1.0)];
        let modes = resonant_modes_from_pairs(&pairs, 2);
        assert_eq!(modes.len(), 2);
        assert!((modes[0].lambda - 5.0).abs() < 1e-9);
        assert!((modes[1].lambda - 1.0).abs() < 1e-9);
        // Первая мода живёт на дугах {1,2}, вторая — на {7,8}.
        let nodes0: Vec<u32> = modes[0]
            .u
            .iter()
            .chain(modes[0].v.iter())
            .map(|&(n, _)| n)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(nodes0, vec![1, 2]);
        let nodes1: Vec<u32> = modes[1]
            .u
            .iter()
            .chain(modes[1].v.iter())
            .map(|&(n, _)| n)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(nodes1, vec![7, 8]);
    }

    #[test]
    fn modes_deterministic() {
        // Разделители не дают обратной циркуляции: моды чистые и стабильные.
        let mut g = Gyroscope::new(1, 1024);
        for k in 0..6u32 {
            for round in 0..(k + 2) {
                g.observe_seq(&[(k, 1.0), (k + 10, 1.0), (500 + k * 50 + round, 1.0)]);
            }
        }
        let a = g.resonant_modes(0.1, 4);
        let b = g.resonant_modes(0.1, 4);
        assert_eq!(a, b);
        assert!(!a.is_empty());
        // λ убывают.
        for w in a.windows(2) {
            assert!(w[0].lambda >= w[1].lambda - 1e-12);
        }
        assert!(a[0].lambda > a[a.len() - 1].lambda);
    }

    #[test]
    fn cycle_pair_modes() {
        // 3-цикл 1→2→3→1 с равными весами: все узлы в одной моде.
        let pairs = vec![(1, 2, 3.0), (2, 3, 3.0), (1, 3, -3.0)];
        let modes = resonant_modes_from_pairs(&pairs, 2);
        assert!(!modes.is_empty());
        // Кососимметричная 3×3 имеет ранг 2: две моды с λ = 3·(√3/2)?
        // Точность не проверяем — только разделение узлов и убывание λ.
        assert!(modes[0].lambda > 0.0);
        if modes.len() > 1 {
            assert!(modes[0].lambda >= modes[1].lambda - 1e-9);
        }
    }

    #[test]
    fn precess_single_pair_exact() {
        // Пара (1,2,w), θ_1 = 0, θ_2 = π/2: обе дуги шагают на −η·w.
        let pairs = vec![(1u32, 2u32, 1.0_f64)];
        let mut thetas = vec![0.777, 0.0, std::f64::consts::FRAC_PI_2];
        precess_step(&pairs, &mut thetas, 0.1);
        assert_eq!(thetas[0], 0.777); // посторонняя дуга не тронута
        assert!((thetas[1] + 0.1).abs() < 1e-12);
        assert!((thetas[2] - (std::f64::consts::FRAC_PI_2 - 0.1)).abs() < 1e-12);
        // Относительная фаза пары сохраняется — совместная прецессия.
        assert!(((thetas[2] - thetas[1]) - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn precess_aligned_and_empty_do_not_move() {
        // Δθ = 0 → sin 0 = 0 → прецессии нет.
        let pairs = vec![(0, 1, 5.0)];
        let mut thetas = vec![0.3, 0.3];
        precess_step(&pairs, &mut thetas, 0.5);
        assert_eq!(thetas, vec![0.3, 0.3]);
        // Пустой гироскоп — покой.
        let mut thetas = vec![0.3, 0.3];
        precess_step(&[], &mut thetas, 0.5);
        assert_eq!(thetas, vec![0.3, 0.3]);
    }

    #[test]
    fn precess_chain_mixes_torques() {
        // Узел в двух парах получает разные моменты: относительные фазы едут.
        let pairs = vec![(0, 1, 1.0), (1, 2, -1.0)];
        let mut thetas = vec![0.0, 0.0, std::f64::consts::FRAC_PI_2];
        precess_step(&pairs, &mut thetas, 0.2);
        // Пара (0,1): Δ=0 → нет вклада. Пара (1,2): sin(π/2)·(−1) → +0.2 обеим.
        assert!((thetas[0]).abs() < 1e-12);
        assert!((thetas[1] - 0.2).abs() < 1e-12);
        assert!((thetas[2] - (std::f64::consts::FRAC_PI_2 + 0.2)).abs() < 1e-12);
    }

    #[test]
    fn observe_is_deterministic() {
        let seq: Vec<(u32, f64)> = (0..100)
            .map(|i| (i % 13, if i % 2 == 0 { 1.0 } else { -1.0 }))
            .collect();
        let mut a = Gyroscope::new(16, 1024);
        let mut b = Gyroscope::new(16, 1024);
        a.observe_seq(&seq);
        b.observe_seq(&seq);
        assert_eq!(a.skew_pairs(EPS), b.skew_pairs(EPS));
        assert_eq!(a.ticks(), b.ticks());
    }
}
