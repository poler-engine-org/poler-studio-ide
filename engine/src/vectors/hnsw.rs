//! Poler-нативный HNSW поверх RaBitQ-кодов (v2.0, Приоритет 4, кирпич 1).
//!
//! Все существующие HNSW-крейты хранят fp32-векторы — это убивает всю
//! плотность. Здесь граф живёт НАД квантованным хранилищем: дистанция
//! обхода — симметричная popcount-оценка (XOR + POPCNT, десятки ГБ/с),
//! финальное ранжирование — асимметричный ADC (запрос в полной точности).
//!
//! Алгоритм — классический HNSW (Malkov & Yashunin, TPAMI 2018),
//! чистая реализация: слоёв `L = floor(-ln(U)·mL)` (геометрия),
//! вставка — жадный спуск + search-layer с efConstruction, выбор
//! соседей — эвристика разнообразия из статьи (Algorithm 4) с
//! добором отброшенных (keepPruned). Детерминизм: уровни из
//! SplitMix64, сравнения — total_cmp с tie-break по id узла —
//! две сборки одного корпуса дают побитово одинаковый граф.
//!
//! Память графа: слой 0 — `Vec<u32>` на узел (cap M0), верхние слои —
//! разреженная карта (узлов с уровнем ≥ 1 всего ~1/M). На 1B векторов
//! граф — верхний IVF-слой поверх mmap-кодов (см. research §5.6).

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use super::rabitq::{adc_cos, sym_ip, QueryPrep, SplitMix64, SymSide};
use super::store::CodeSource;

/// Магия формата сериализации графа.
const MAGIC: [u8; 8] = *b"PHNSW\x01\0\0";

// ---------------------------------------------------------------------------
// Конфигурация
// ---------------------------------------------------------------------------

/// Параметры HNSW. Умолчания — статья: M=16, M0=2M=32, efC=200.
#[derive(Debug, Clone)]
pub struct HnswConfig {
    /// Соседей на слой ≥ 1.
    pub m: usize,
    /// Соседей на слое 0 (2M).
    pub m0: usize,
    /// Ширина поиска при вставке.
    pub ef_construction: usize,
    /// Сид уровней (детерминизм сборки).
    pub seed: u64,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            m0: 32,
            ef_construction: 200,
            seed: 0x484E_5357_0000_0001,
        }
    }
}

// ---------------------------------------------------------------------------
// Куча: total-порядок с tie-break по узлу (детерминизм)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct HeapItem(f64, u32);

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}
impl Eq for HeapItem {}
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0).then(self.1.cmp(&other.1))
    }
}

// ---------------------------------------------------------------------------
// Индекс
// ---------------------------------------------------------------------------

/// HNSW-граф над `CodeSource`-хранилищем.
///
/// Узлы вставляются строго по порядку слотов (`insert(store, node)`
/// с `node == len()`): вектор уже закодирован в хранилище, граф
/// читает только коды и скаляры — fp32 в индексе нет вообще.
pub struct HnswIndex {
    cfg: HnswConfig,
    ml: f64,
    entry: Option<u32>,
    max_level: u8,
    /// Число слоёв минус 1 у каждого узла.
    levels: Vec<u8>,
    /// Слой 0: соседи каждого узла (cap m0).
    links0: Vec<Vec<u32>>,
    /// Слои ≥ 1: только у узлов с уровнем ≥ 1 (разреженно).
    links_up: HashMap<u32, Vec<Vec<u32>>>,
    rng: SplitMix64,
}

impl HnswIndex {
    /// Новый пустой индекс.
    pub fn new(cfg: HnswConfig) -> Self {
        let ml = 1.0 / (cfg.m.max(2) as f64).ln();
        Self {
            rng: SplitMix64::new(cfg.seed),
            ml,
            entry: None,
            max_level: 0,
            levels: Vec::new(),
            links0: Vec::new(),
            links_up: HashMap::new(),
            cfg,
        }
    }

    /// Число узлов.
    pub fn len(&self) -> usize {
        self.levels.len()
    }

    /// Пуст ли индекс.
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    /// Точка входа (для диагностики).
    pub fn entry(&self) -> Option<u32> {
        self.entry
    }

    /// Соседи узла на слое `layer` (0 = нижний).
    fn neighbors(&self, u: u32, layer: u8) -> &[u32] {
        if layer == 0 {
            return &self.links0[u as usize];
        }
        match self.links_up.get(&u) {
            Some(v) => &v[(layer - 1) as usize],
            None => &[],
        }
    }

    /// Уровень нового узла: `floor(-ln(U)·mL)`, cap 31.
    fn draw_level(&mut self) -> u8 {
        let u = self.rng.next_u01().max(1e-12);
        let l = (-u.ln() * self.ml).floor();
        l.min(31.0) as u8
    }

    // -- вставка -----------------------------------------------------------

    /// Вставляет узел `node` (слот хранилища; строго `node == len()`).
    ///
    /// Точки входа и соседство считаются ТОЛЬКО по квантованным
    /// представлениям — fp32-оригиналы индексу не нужны.
    pub fn insert<C: CodeSource>(&mut self, store: &C, node: u32) {
        assert!(
            node as usize == self.levels.len(),
            "HNSW: вставка строго по порядку (node == len)"
        );
        let level = self.draw_level();
        self.levels.push(level);
        self.links0.push(Vec::with_capacity(self.cfg.m0.min(8)));
        if level > 0 {
            self.links_up
                .insert(node, vec![Vec::new(); level as usize]);
        }
        let Some(entry) = self.entry else {
            self.entry = Some(node);
            self.max_level = level;
            return;
        };

        // Собственное квантованное представление вставляемого (запрос).
        let qs = QuantSide::from_store(store, node);
        let q = qs.side();
        let dp = store.d_pad();

        // Жадный спуск по верхним слоям до level+1
        let mut cur = entry;
        let mut cur_d = -sym_ip(store.side(cur), q, dp);
        if level < self.max_level {
            for layer in ((level + 1)..=self.max_level).rev() {
                loop {
                    let mut best = cur;
                    let mut best_d = cur_d;
                    for &nb in self.neighbors(cur, layer) {
                        let d = -sym_ip(store.side(nb), q, dp);
                        if d < best_d {
                            best = nb;
                            best_d = d;
                        }
                    }
                    if best == cur {
                        break;
                    }
                    cur = best;
                    cur_d = best_d;
                }
            }
        }

        // Послойное подключение (сверху вниз до 0)
        let mut visited: Vec<u64> = Vec::new();
        let mut cands: Vec<(f64, u32)> = Vec::new();
        let mut cur_ep = (cur_d, cur);
        for layer in (0..=level.min(self.max_level)).rev() {
            self.search_layer(
                store,
                &q,
                cur_ep.0,
                cur_ep.1,
                self.cfg.ef_construction.max(1),
                layer,
                &mut visited,
                &mut cands,
            );
            let selected = self.select_heuristic(store, &cands, self.cfg.m);
            if layer == 0 {
                self.links0[node as usize] = selected.clone();
            } else {
                self.links_up
                    .get_mut(&node)
                    .expect("верхние слои узла созданы")
                    [(layer - 1) as usize] = selected.clone();
            }
            for &v in &selected {
                let cap = if layer == 0 { self.cfg.m0 } else { self.cfg.m };
                self.connect_reverse(store, v, node, layer, cap);
            }
            if let Some(&(d, n)) = cands.first() {
                cur_ep = (d, n);
            }
        }
        if level > self.max_level {
            self.max_level = level;
            self.entry = Some(node);
        }
    }

    /// Обратная ссылка `node → v` на слое с прунингом по эвристике.
    fn connect_reverse<C: CodeSource>(
        &mut self,
        store: &C,
        v: u32,
        node: u32,
        layer: u8,
        cap: usize,
    ) {
        let dp = store.d_pad();
        let existing: Vec<u32> = if layer == 0 {
            self.links0[v as usize].clone()
        } else {
            self.links_up
                .get(&v)
                .map(|ls| ls[(layer - 1) as usize].clone())
                .unwrap_or_default()
        };
        if existing.contains(&node) {
            return;
        }
        let needs_prune = existing.len() + 1 > cap;
        if !needs_prune {
            if layer == 0 {
                self.links0[v as usize].push(node);
            } else if let Some(ls) = self.links_up.get_mut(&v) {
                ls[(layer - 1) as usize].push(node);
            }
            return;
        }
        // Прунинг: пересобрать список v по эвристике относительно v
        let vq = QuantSide::from_store(store, v);
        let vq_side = vq.side();
        let mut arr: Vec<(f64, u32)> = existing
            .iter()
            .map(|&u| (-sym_ip(store.side(u), vq_side, dp), u))
            .collect();
        arr.push((-sym_ip(store.side(node), vq_side, dp), node));
        arr.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let keep = self.select_heuristic(store, &arr, cap);
        if layer == 0 {
            self.links0[v as usize] = keep;
        } else if let Some(ls) = self.links_up.get_mut(&v) {
            ls[(layer - 1) as usize] = keep;
        }
    }

    // -- поиск -------------------------------------------------------------

    /// Поиск топ-k: обход по sym (popcount), переранжирование по ADC
    /// (запрос в полной точности), возврат (косинус-оценка, узел),
    /// по убыванию оценки. `ef ≥ k` — ширина луча на слое 0.
    pub fn search<C: CodeSource>(
        &self,
        store: &C,
        prep: &QueryPrep,
        k: usize,
        ef: usize,
    ) -> Vec<(f64, u32)> {
        if self.entry.is_none() || k == 0 {
            return Vec::new();
        }
        let ef = ef.max(k);
        let q = prep.sym();
        let dp = store.d_pad();

        // Жадный спуск по верхним слоям
        let mut cur = self.entry.unwrap();
        let mut cur_d = -sym_ip(store.side(cur), q, dp);
        for layer in (1..=self.max_level).rev() {
            loop {
                let mut best = cur;
                let mut best_d = cur_d;
                for &nb in self.neighbors(cur, layer) {
                    let d = -sym_ip(store.side(nb), q, dp);
                    if d < best_d {
                        best = nb;
                        best_d = d;
                    }
                }
                if best == cur {
                    break;
                }
                cur = best;
                cur_d = best_d;
            }
        }

        // Слой 0: широкий поиск
        let mut visited: Vec<u64> = Vec::new();
        let mut cands: Vec<(f64, u32)> = Vec::new();
        self.search_layer(store, &q, cur_d, cur, ef, 0, &mut visited, &mut cands);

        // ADC-переранжирование (полная точность запроса)
        let mut ranked: Vec<(f64, u32)> = cands
            .iter()
            .map(|&(_, n)| {
                let sc = store.scalars(n);
                (adc_cos(store.codes(n), sc, prep, dp), n)
            })
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        ranked.truncate(k);
        ranked
    }

    /// Классический search-layer (Algorithm 2): два зеркальных heap.
    #[allow(clippy::too_many_arguments)]
    fn search_layer<C: CodeSource>(
        &self,
        store: &C,
        q: &SymSide<'_>,
        ep_d: f64,
        ep: u32,
        ef: usize,
        layer: u8,
        visited: &mut Vec<u64>,
        out: &mut Vec<(f64, u32)>,
    ) {
        let words = store.len().div_ceil(64);
        visited.clear();
        visited.resize(words, 0);
        let dp = store.d_pad();
        let q = *q; // SymSide — Copy

        let mut cand: std::collections::BinaryHeap<std::cmp::Reverse<HeapItem>> =
            std::collections::BinaryHeap::new();
        let mut res: std::collections::BinaryHeap<HeapItem> = std::collections::BinaryHeap::new();
        bit_set(visited, ep);
        cand.push(std::cmp::Reverse(HeapItem(ep_d, ep)));
        res.push(HeapItem(ep_d, ep));

        while let Some(std::cmp::Reverse(HeapItem(d, u))) = cand.pop() {
            if res.len() >= ef {
                if let Some(worst) = res.peek() {
                    if d > worst.0 {
                        break;
                    }
                }
            }
            for &nb in self.neighbors(u, layer) {
                if bit_test(visited, nb) {
                    continue;
                }
                bit_set(visited, nb);
                let d2 = -sym_ip(store.side(nb), q, dp);
                if res.len() < ef {
                    cand.push(std::cmp::Reverse(HeapItem(d2, nb)));
                    res.push(HeapItem(d2, nb));
                } else if let Some(worst) = res.peek() {
                    if d2 < worst.0 {
                        res.pop();
                        res.push(HeapItem(d2, nb));
                        cand.push(std::cmp::Reverse(HeapItem(d2, nb)));
                    }
                }
            }
        }
        out.clear();
        out.extend(res.into_iter().map(|h| (h.0, h.1)));
        out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    }

    /// Эвристика выбора соседей (Algorithm 4): кандидат добавляется,
    /// если он ближе к запросу, чем к каждому уже выбранному; остаток
    /// добирается отброшенными (keepPrunedConnections).
    /// Дистанции до запроса уже лежат в `cands` — сам запрос не нужен.
    fn select_heuristic<C: CodeSource>(
        &self,
        store: &C,
        cands: &[(f64, u32)],
        m: usize,
    ) -> Vec<u32> {
        if cands.len() <= m {
            return cands.iter().map(|c| c.1).collect();
        }
        let dp = store.d_pad();
        let mut sel: Vec<u32> = Vec::with_capacity(m);
        let mut disc: Vec<u32> = Vec::new();
        for &(d, c) in cands {
            if sel.len() >= m {
                break;
            }
            if sel.is_empty() {
                sel.push(c);
                continue;
            }
            let cs = store.side(c);
            let ok = sel
                .iter()
                .all(|&s| d < -sym_ip(store.side(s), cs, dp));
            if ok {
                sel.push(c);
            } else {
                disc.push(c);
            }
        }
        for &c in disc.iter() {
            if sel.len() >= m {
                break;
            }
            sel.push(c);
        }
        sel
    }

    // -- сериализация --------------------------------------------------------

    /// Сохраняет граф (плоский формат; коды остаются в хранилище).
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let count = self.levels.len();
        let mut buf: Vec<u8> = Vec::with_capacity(32 + count * 40);
        buf.extend_from_slice(&MAGIC);
        buf.extend_from_slice(&(count as u32).to_le_bytes());
        buf.extend_from_slice(&(self.entry.unwrap_or(u32::MAX)).to_le_bytes());
        buf.push(self.max_level);
        buf.extend_from_slice(&[0u8; 3]);
        buf.extend_from_slice(&self.levels);
        for u in 0..count {
            for layer in 0..=self.levels[u] {
                let links = self.neighbors(u as u32, layer);
                buf.extend_from_slice(&(links.len() as u32).to_le_bytes());
                for &l in links {
                    buf.extend_from_slice(&l.to_le_bytes());
                }
            }
        }
        let mut f = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        f.write_all(&buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(())
    }

    /// Открывает граф, сохранённый `save` (конфиг — для будущих вставок).
    pub fn open(path: &Path, cfg: HnswConfig) -> Result<Self, String> {
        let mut f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if buf.len() < 20 || buf[0..8] != MAGIC {
            return Err("чужая магия: не PHNSW-граф".into());
        }
        let u32_at = |off: usize| u32::from_le_bytes(buf[off..off + 4].try_into().unwrap());
        let count = u32_at(8) as usize;
        let entry_raw = u32_at(12);
        let max_level = buf[16];
        if buf.len() < 20 + count {
            return Err("граф обрезан: блок уровней".into());
        }
        let mut idx = Self::new(cfg);
        idx.max_level = max_level;
        idx.entry = (entry_raw != u32::MAX).then_some(entry_raw);
        idx.levels = buf[20..20 + count].to_vec();
        idx.links0 = vec![Vec::new(); count];
        let mut off = 20 + count;
        for u in 0..count {
            for layer in 0..=idx.levels[u] {
                if off + 4 > buf.len() {
                    return Err("граф обрезан: список соседей".into());
                }
                let deg = u32_at(off) as usize;
                off += 4;
                if off + deg * 4 > buf.len() {
                    return Err("граф обрезан: соседи".into());
                }
                let links: Vec<u32> = (0..deg)
                    .map(|i| u32_at(off + i * 4))
                    .collect();
                off += deg * 4;
                if layer == 0 {
                    idx.links0[u] = links;
                } else {
                    let nlevels = idx.levels[u] as usize;
                    let slots = idx
                        .links_up
                        .entry(u as u32)
                        .or_insert_with(|| vec![Vec::new(); nlevels]);
                    slots[(layer - 1) as usize] = links;
                }
            }
        }
        Ok(idx)
    }
}

// -- вспомогательное --------------------------------------------------------

/// Владеющее квантованное представление узла (для вставок и прунинга).
struct QuantSide {
    codes: Vec<u64>,
    mu: f32,
    delta: f32,
}

impl QuantSide {
    fn from_store<C: CodeSource>(store: &C, u: u32) -> Self {
        let sc = store.scalars(u);
        Self {
            codes: store.codes(u).to_vec(),
            mu: sc.mu,
            delta: sc.delta,
        }
    }
    fn side(&self) -> SymSide<'_> {
        SymSide {
            codes: &self.codes,
            mu: self.mu,
            delta: self.delta,
        }
    }
}

#[inline]
fn bit_test(v: &[u64], i: u32) -> bool {
    (v[(i / 64) as usize] >> (i % 64)) & 1 == 1
}

#[inline]
fn bit_set(v: &mut [u64], i: u32) {
    v[(i / 64) as usize] |= 1u64 << (i % 64);
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::rabitq::SplitMix64;
    use super::super::store::QuantizedStore;
    use super::*;

    /// Смесь гауссиан (реалистичная кластерность эмбеддингов).
    pub(super) fn mixture(n: usize, d: usize, centers: usize, seed: u64, noise: f64) -> Vec<Vec<f32>> {
        let mut rng = SplitMix64::new(seed);
        let gauss = |rng: &mut SplitMix64| {
            let u1 = rng.next_u01().max(1e-12);
            let u2 = rng.next_u01();
            (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
        };
        let cs: Vec<Vec<f64>> = (0..centers)
            .map(|_| (0..d).map(|_| gauss(&mut rng)).collect())
            .collect();
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let c = &cs[i % centers];
            let mut v: Vec<f32> = (0..d)
                .map(|j| (c[j] + gauss(&mut rng) * noise) as f32)
                .collect();
            let norm = (v.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>()).sqrt();
            if norm > 0.0 {
                for x in v.iter_mut() {
                    *x /= norm as f32;
                }
            }
            out.push(v);
        }
        out
    }

    fn build(n: usize, d: usize, seed: u64, cfg: HnswConfig) -> (QuantizedStore, HnswIndex) {
        let data = mixture(n, d, (n / 25).max(4), seed, 0.35);
        let mut store = QuantizedStore::new(d, 0xBEEF);
        for (i, x) in data.iter().enumerate() {
            store.push(i as u32, x);
        }
        let mut idx = HnswIndex::new(cfg);
        for i in 0..n as u32 {
            idx.insert(&store, i);
        }
        (store, idx)
    }

    pub(super) fn gt_topk(data: &[Vec<f32>], q: &[f32], k: usize) -> Vec<usize> {
        let mut ips: Vec<(f64, usize)> = data
            .iter()
            .enumerate()
            .map(|(i, x)| {
                (
                    x.iter().zip(q).map(|(a, b)| (*a as f64) * (*b as f64)).sum(),
                    i,
                )
            })
            .collect();
        ips.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        ips.truncate(k);
        ips.into_iter().map(|p| p.1).collect()
    }

    #[test]
    fn empty_and_single() {
        let store = QuantizedStore::new(64, 1);
        let idx = HnswIndex::new(HnswConfig::default());
        let q = vec![0.5f32; 64];
        let prep = store.prepare_query(&q);
        assert!(idx.search(&store, &prep, 10, 64).is_empty());

        let mut store = QuantizedStore::new(64, 1);
        let x = vec![1.0f32; 64];
        store.push(7, &x);
        let mut idx = HnswIndex::new(HnswConfig::default());
        idx.insert(&store, 0);
        let prep = store.prepare_query(&x);
        let res = idx.search(&store, &prep, 10, 64);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].1, 0);
    }

    #[test]
    fn insert_rejects_out_of_order() {
        let mut store = QuantizedStore::new(64, 1);
        store.push(0, &vec![1.0; 64]);
        let mut idx = HnswIndex::new(HnswConfig::default());
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            idx.insert(&store, 5);
        }));
        assert!(res.is_err(), "вставка вне порядка обязана паниковать");
    }

    #[test]
    fn recall_gate_mixture() {
        // Инвариант графа: HNSW обязан находить ≥ 90% кандидатов,
        // которые САМ оценщик способен отранжировать (потолок brute-ADC).
        // Напоминание о физике: recall brute-ADC против fp32 на вырожденных
        // кластерах ограничен шумом 1 бита — это свойство квантования,
        // не графа; реалистичные числа даёт бенчмарк на 768-d.
        use crate::vectors::rabitq::adc_cos;
        let n = 8000;
        let (store, idx) = build(
            n,
            128,
            12345,
            HnswConfig {
                m: 12,
                m0: 24,
                ef_construction: 100,
                seed: 77,
            },
        );
        let data = mixture(n, 128, (n / 25).max(4), 12345, 0.35);
        let mut rng = SplitMix64::new(999);
        let nq = 30;
        let mut vs_ceiling = 0.0f64;
        let mut ceiling_vs_fp32 = 0.0f64;
        for qi in 0..nq {
            // запросы из тех же кластеров (реалистичный сценарий)
            let mut q = data[(qi * 331 + 7) % n].clone();
            for v in q.iter_mut() {
                *v += (rng.next_u01() - 0.5) as f32 * 0.02;
            }
            let prep = store.prepare_query(&q);
            // потолок: полный скан с тем же ADC-ранжированием
            let mut ceiling: Vec<(f64, u32)> = (0..n as u32)
                .map(|i| {
                    (
                        adc_cos(store.codes(i), store.scalars(i), &prep, store.d_pad()),
                        i,
                    )
                })
                .collect();
            ceiling.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            let want_adc: Vec<usize> =
                ceiling.iter().take(10).map(|p| p.1 as usize).collect();
            let got: Vec<usize> =
                idx.search(&store, &prep, 10, 48).iter().map(|p| p.1 as usize).collect();
            let want_gt = gt_topk(&data, &q, 10);
            vs_ceiling += want_adc.iter().filter(|w| got.contains(w)).count() as f64 / 10.0;
            ceiling_vs_fp32 +=
                want_gt.iter().filter(|w| want_adc.contains(w)).count() as f64 / 10.0;
        }
        vs_ceiling /= nq as f64;
        ceiling_vs_fp32 /= nq as f64;
        assert!(
            vs_ceiling >= 0.90,
            "HNSW теряет кандидатов оценщика: {vs_ceiling:.3} < 0.90 — граф деградировал"
        );
        assert!(
            ceiling_vs_fp32 >= 0.45,
            "потолок оценщика подозрительно низ: {ceiling_vs_fp32:.3}"
        );
    }

    #[test]
    fn build_is_deterministic() {
        let cfg = HnswConfig {
            m: 8,
            m0: 16,
            ef_construction: 50,
            seed: 4242,
        };
        let (s1, i1) = build(1500, 64, 31337, cfg.clone());
        let (s2, i2) = build(1500, 64, 31337, cfg);
        let t1 = std::env::temp_dir().join(format!("hns-det-1-{}.bin", std::process::id()));
        let t2 = std::env::temp_dir().join(format!("hns-det-2-{}.bin", std::process::id()));
        i1.save(&t1).unwrap();
        i2.save(&t2).unwrap();
        let b1 = std::fs::read(&t1).unwrap();
        let b2 = std::fs::read(&t2).unwrap();
        assert_eq!(b1, b2, "две сборки дали разные графы");
        // и хранилища тоже
        let q = vec![0.25f32; 64];
        let p1 = s1.prepare_query(&q);
        let p2 = s2.prepare_query(&q);
        assert_eq!(p1.yq, p2.yq);
        let _ = std::fs::remove_file(&t1);
        let _ = std::fs::remove_file(&t2);
    }

    #[test]
    fn save_open_search_identical() {
        let (store, idx) = build(
            2000,
            64,
            555,
            HnswConfig {
                m: 8,
                m0: 16,
                ef_construction: 60,
                seed: 8,
            },
        );
        let data = mixture(2000, 64, 80, 555, 0.35);
        let tmp = std::env::temp_dir().join(format!("hns-io-{}.bin", std::process::id()));
        idx.save(&tmp).unwrap();
        let loaded = HnswIndex::open(
            &tmp,
            HnswConfig {
                m: 8,
                m0: 16,
                ef_construction: 60,
                seed: 8,
            },
        )
        .unwrap();
        assert_eq!(loaded.len(), 2000);
        let mut agree = true;
        for qi in 0..10 {
            let q = data[(qi * 197 + 3) % 2000].clone();
            let prep = store.prepare_query(&q);
            let a = idx.search(&store, &prep, 10, 48);
            let b = loaded.search(&store, &prep, 10, 48);
            if a != b {
                agree = false;
                break;
            }
        }
        assert!(agree, "поиск до/после сериализации разошёлся");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn open_rejects_garbage() {
        let tmp = std::env::temp_dir().join(format!("hns-bad-{}.bin", std::process::id()));
        std::fs::write(&tmp, b"XXXXXXXXXXXXXXXXXXXX").unwrap();
        assert!(HnswIndex::open(&tmp, HnswConfig::default()).is_err());
        let _ = std::fs::remove_file(&tmp);
    }
}

