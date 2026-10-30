//! Коннектом FLYCSR1 — мозг мухи FlyWire v783 как исполняемая матрица A.
//!
//! Формат (docs/flywire-connectome/README.md §3): zstd-19 поверх сырого
//! little-endian буфера
//! `FLYCSR1 | core_flag u8 | n_nodes u32 | n_edges u32 |
//!  offsets u32[n_nodes+1] | targets u32[n_edges] | weights u16[n_edges] |
//!  nt_class u8[n_edges]`.
//!
//! Артефакты в git (~43 МБ): `flywire_v783_{full,core}.csr.zst` +
//! `flywire_v783_nodes.bin` (138 639 отсортированных u64 root_id).
//! Сырьё (852 МБ feather) не нужно — ридер самодостаточен.
//!
//! Операции канонического уравнения `dp/dt = −η·Π_Λ[D·p + γJ·p + ∇F]`:
//! - [`Connectome::edge`] / [`Connectome::signed_weight`] — A(u,v) со
//!   знаком: +1 ach/glut (возбуждающий), −1 gaba (тормозной),
//!   0 октопамин/серотонин/допамин (модуляторный);
//! - [`Connectome::rotor`] — J = A − Aᵀ: ротор циркуляции влияния пары
//!   нейронов (канон R[n]);
//! - [`Connectome::k_hop`] — BFS потока сигнала по исходящим рёбрам
//!   (среза строки CSR — сплошная память);
//! - [`Connectome::build_in_edges`] — CSC-транспонирование: impact-слой
//!   AIDDE («кто управляет нейроном»).
//!
//! Декомпрессия — один проход `zstd::bulk` с гардом [`MAX_RAW_BYTES`];
//! валидация (магия/размеры/монотонность/сортировка/диапазоны) полная,
//! до первого запроса.

use std::fs;
use std::path::Path;

/// Магия формата (7 байт).
pub const MAGIC: &[u8; 7] = b"FLYCSR1";

/// Гард декомпрессии: сырой CSR-буфер не бывает больше. Полный
/// коннектом мухи ≈ 106 МиБ; запас — на два порядка больше мозг.
pub const MAX_RAW_BYTES: usize = 512 * 1024 * 1024;

/// Порог ядра: рёбра с весом ≥ 5 синапсов (core_flag = 1).
pub const CORE_MIN_SYNAPSES: u16 = 5;

/// Медиаторы nt_class: 0=gaba, 1=ach, 2=glut, 3=oct, 4=ser, 5=da
/// (Eckstein et al. 2024, argmax_c Σ score_c·syn_count).
pub const NT_NAMES: [&str; 6] = ["gaba", "ach", "glut", "oct", "ser", "da"];

/// Знак класса медиатора: +1 возбуждающий, −1 тормозной, 0 модуляторный.
#[inline]
pub fn nt_sign(class: u8) -> i32 {
    match class {
        1 | 2 => 1, // ach, glut
        0 => -1,    // gaba
        _ => 0,     // oct, ser, da
    }
}

/// Ребро CSR в позиции idx. Для in-edge (CSC) `target` — источник,
/// т.е. «другой конец» ребра относительно узла запроса.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    /// Позиция ребра в CSR-массивах (targets/weights/nt).
    pub idx: usize,
    /// Другой конец: цель для out-edge, источник для in-edge.
    pub target: u32,
    /// Сумма syn_count пары (агрегация по нейропилям).
    pub weight: u16,
    /// Доминантный медиатор связи (0..5, см. [`NT_NAMES`]).
    pub nt: u8,
}

impl Edge {
    /// Знак связи: +1 / −1 / 0.
    #[inline]
    pub fn sign(&self) -> i32 {
        nt_sign(self.nt)
    }

    /// Знаковый вес: sign · weight (модуляторы дают 0).
    #[inline]
    pub fn signed_weight(&self) -> i32 {
        self.sign() * self.weight as i32
    }

    /// Имя медиатора.
    pub fn nt_name(&self) -> &'static str {
        NT_NAMES[self.nt as usize]
    }
}

/// Фильтр рёбер K-hop обхода по знаку (поток сигнала).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignFilter {
    /// Все связи (и возбуждающие, и тормозные, и модуляторные).
    All,
    /// Только возбуждающие (ach/glut).
    Excitatory,
    /// Только тормозные (gaba).
    Inhibitory,
}

impl SignFilter {
    /// Парсинг CLI-значения.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "all" | "все" => Ok(Self::All),
            "exc" | "возб" => Ok(Self::Excitatory),
            "inh" | "торм" => Ok(Self::Inhibitory),
            _ => Err(format!(
                "неизвестный фильтр знака «{s}» (ожидалось all | exc | inh)"
            )),
        }
    }

    fn wanted_sign(self) -> Option<i32> {
        match self {
            Self::All => None,
            Self::Excitatory => Some(1),
            Self::Inhibitory => Some(-1),
        }
    }
}

/// Результат K-hop BFS: размер фронта на каждом хопе + достигнуто узлов.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KHopReport {
    /// Число впервые достигнутых нейронов на каждом хопе (1..=k).
    /// Пустой фронт прерывает обход и НЕ пушится.
    pub frontier_sizes: Vec<usize>,
    /// Всего достигнуто узлов, включая стартовый.
    pub visited: usize,
}

/// Коннектом FLYCSR1 в RAM: CSR-матрица смежности со знаками.
#[derive(Clone, Debug)]
pub struct Connectome {
    core: bool,
    n_nodes: usize,
    n_edges: usize,
    offsets: Vec<u32>,
    targets: Vec<u32>,
    weights: Vec<u16>,
    nt: Vec<u8>,
}

impl Connectome {
    /// Чтение `.csr.zst`-артефакта: файл → zstd (гард 512 МиБ) →
    /// парсинг + полная валидация. Сырой feather не нужен.
    pub fn load(path: &Path) -> Result<Self, String> {
        let blob = fs::read(path)
            .map_err(|e| format!("не удалось прочитать {}: {e}", path.display()))?;
        let raw = zstd::bulk::Decompressor::new()
            .map_err(|e| format!("zstd-декодер: {e}"))?
            .decompress(&blob, MAX_RAW_BYTES)
            .map_err(|e| {
                format!(
                    "zstd-декомпрессия {} (гард {} байт): {e}",
                    path.display(),
                    MAX_RAW_BYTES
                )
            })?;
        Self::from_raw(&raw).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Разбор сырого (уже распакованного) FLYCSR1-буфера с валидацией.
    pub fn from_raw(raw: &[u8]) -> Result<Self, String> {
        if raw.len() < 16 {
            return Err(format!(
                "буфер {} байт короче заголовка FLYCSR1 (16 байт)",
                raw.len()
            ));
        }
        if &raw[..7] != MAGIC {
            return Err("не FLYCSR1: неверная магия".to_string());
        }
        let core_flag = raw[7];
        if core_flag > 1 {
            return Err(format!("core_flag = {core_flag} (ожидалось 0 | 1)"));
        }
        let n_nodes = u32::from_le_bytes(raw[8..12].try_into().unwrap()) as usize;
        let n_edges = u32::from_le_bytes(raw[12..16].try_into().unwrap()) as usize;

        // Размеры секций с защитой от переполнения.
        let offs_bytes = n_nodes
            .checked_add(1)
            .and_then(|v| v.checked_mul(4))
            .ok_or("n_nodes переполняет размеры секций")?;
        let expected = 16usize
            .checked_add(offs_bytes)
            .and_then(|v| v.checked_add(n_edges.checked_mul(4)?))
            .and_then(|v| v.checked_add(n_edges.checked_mul(2)?))
            .and_then(|v| v.checked_add(n_edges))
            .ok_or("n_edges переполняет размеры секций")?;
        if raw.len() != expected {
            return Err(format!(
                "размер буфера {} != ожидаемым {} (n_nodes={n_nodes}, n_edges={n_edges})",
                raw.len(),
                expected
            ));
        }

        // Копирование секций (LE → нативный порядок).
        let mut offsets = Vec::with_capacity(n_nodes + 1);
        for c in raw[16..16 + offs_bytes].chunks_exact(4) {
            offsets.push(u32::from_le_bytes(c.try_into().unwrap()));
        }
        let mut pos = 16 + offs_bytes;
        let mut targets = Vec::with_capacity(n_edges);
        for c in raw[pos..pos + n_edges * 4].chunks_exact(4) {
            targets.push(u32::from_le_bytes(c.try_into().unwrap()));
        }
        pos += n_edges * 4;
        let mut weights = Vec::with_capacity(n_edges);
        for c in raw[pos..pos + n_edges * 2].chunks_exact(2) {
            weights.push(u16::from_le_bytes(c.try_into().unwrap()));
        }
        pos += n_edges * 2;
        let nt = raw[pos..pos + n_edges].to_vec();

        // Валидация структуры.
        if offsets[0] != 0 {
            return Err(format!("offsets[0] = {} (ожидалось 0)", offsets[0]));
        }
        if offsets[n_nodes] as usize != n_edges {
            return Err(format!(
                "offsets[n] = {} != n_edges = {n_edges}",
                offsets[n_nodes]
            ));
        }
        for w in offsets.windows(2) {
            if w[0] > w[1] {
                return Err("offsets не монотонны".to_string());
            }
        }
        for u in 0..n_nodes {
            let (a, b) = (offsets[u] as usize, offsets[u + 1] as usize);
            let row = &targets[a..b];
            for t in row.windows(2) {
                if t[0] >= t[1] {
                    return Err(format!(
                        "targets строки {u} не строго возрастают (дубликат?)"
                    ));
                }
            }
        }
        for (i, &t) in targets.iter().enumerate() {
            if t as usize >= n_nodes {
                return Err(format!("targets[{i}] = {t} вне диапазона узлов (0..{n_nodes})"));
            }
        }
        for (i, &c) in nt.iter().enumerate() {
            if c > 5 {
                return Err(format!("nt_class[{i}] = {c} вне диапазона 0..5"));
            }
        }
        if core_flag == 1 {
            for (i, &w) in weights.iter().enumerate() {
                if w < CORE_MIN_SYNAPSES {
                    return Err(format!(
                        "core-артефакт: weights[{i}] = {w} < {} (порог ядра)",
                        CORE_MIN_SYNAPSES
                    ));
                }
            }
        }

        Ok(Self {
            core: core_flag == 1,
            n_nodes,
            n_edges,
            offsets,
            targets,
            weights,
            nt,
        })
    }

    /// Ядро (рёбра ≥ 5 синапсов) или полный граф.
    pub fn is_core(&self) -> bool {
        self.core
    }

    pub fn n_nodes(&self) -> usize {
        self.n_nodes
    }

    pub fn n_edges(&self) -> usize {
        self.n_edges
    }

    /// Исходящая степень нейрона (None — индекс вне диапазона).
    pub fn out_degree(&self, u: usize) -> Option<u32> {
        if u >= self.n_nodes {
            return None;
        }
        Some(self.offsets[u + 1] - self.offsets[u])
    }

    /// Цели исходящих рёбер нейрона (срез CSR-строки, отсортированы).
    pub fn row_targets(&self, u: usize) -> Option<&[u32]> {
        if u >= self.n_nodes {
            return None;
        }
        Some(&self.targets[self.offsets[u] as usize..self.offsets[u + 1] as usize])
    }

    /// Исходящие рёбра нейрона (CSR-строка: цель, вес, медиатор).
    pub fn out_edges(&self, u: usize) -> Option<impl Iterator<Item = Edge> + '_> {
        if u >= self.n_nodes {
            return None;
        }
        let (a, b) = (self.offsets[u] as usize, self.offsets[u + 1] as usize);
        Some((a..b).map(move |idx| Edge {
            idx,
            target: self.targets[idx],
            weight: self.weights[idx],
            nt: self.nt[idx],
        }))
    }

    /// Вес ребра по позиции в CSR-массивах (без границ — вызывается
    /// только с idx из срезов строк; см. [`Connectome::out_edges`]).
    /// C2/v0.33.0: восстановление цепочки кратчайшего пути.
    pub fn row_edge_weight(&self, idx: usize) -> u16 {
        self.weights[idx]
    }

    /// Медиатор ребра по позиции в CSR-массивах.
    pub fn row_edge_nt(&self, idx: usize) -> u8 {
        self.nt[idx]
    }

    /// Ребро u→v бинарным поиском по отсортированной CSR-строке.
    pub fn edge(&self, u: usize, v: usize) -> Option<Edge> {
        if u >= self.n_nodes || v >= self.n_nodes {
            return None;
        }
        let (a, b) = (self.offsets[u] as usize, self.offsets[u + 1] as usize);
        let row = &self.targets[a..b];
        match row.binary_search(&(v as u32)) {
            Ok(j) => {
                let idx = a + j;
                Some(Edge {
                    idx,
                    target: v as u32,
                    weight: self.weights[idx],
                    nt: self.nt[idx],
                })
            }
            Err(_) => None,
        }
    }

    /// Знаковый вес A(u,v) = sign(nt)·weight; 0, если связи нет
    /// (модуляторные рёбра также дают 0 — они не входят в J).
    pub fn signed_weight(&self, u: usize, v: usize) -> i32 {
        self.edge(u, v)
            .map(|e| e.signed_weight())
            .unwrap_or(0)
    }

    /// Ротор пары J = A − Aᵀ: циркуляция влияния. Антисимметричен
    /// по построению: J(u,v) = −J(v,u). Однонаправленное ребро даёт
    /// чистый поток ±w, реципрокная симметричная пара — 0.
    pub fn rotor(&self, u: usize, v: usize) -> i32 {
        self.signed_weight(u, v) - self.signed_weight(v, u)
    }

    /// Суммарная синаптическая масса графа (Σ weights).
    pub fn total_mass(&self) -> u64 {
        self.weights.iter().map(|&w| w as u64).sum()
    }

    /// Масса и число рёбер по каждому из 6 медиаторов.
    pub fn mass_by_nt(&self) -> [(u64, u64); 6] {
        let mut out = [(0u64, 0u64); 6];
        for i in 0..self.n_edges {
            let c = self.nt[i] as usize;
            out[c].0 += self.weights[i] as u64;
            out[c].1 += 1;
        }
        out
    }

    /// K-hop BFS потока сигнала от нейрона: размеры фронтов по хопам
    /// и всего достигнуто. Пустой фронт прерывает обход и не пушится.
    pub fn k_hop(&self, start: usize, k: usize, filter: SignFilter) -> KHopReport {
        if start >= self.n_nodes {
            return KHopReport::default();
        }
        let want = filter.wanted_sign();
        let mut visited = vec![false; self.n_nodes];
        visited[start] = true;
        let mut frontier: Vec<u32> = vec![start as u32];
        let mut sizes = Vec::new();
        for _ in 0..k {
            let mut next: Vec<u32> = Vec::with_capacity(frontier.len() * 4);
            for &u in &frontier {
                let (a, b) =
                    (self.offsets[u as usize] as usize, self.offsets[u as usize + 1] as usize);
                for i in a..b {
                    if let Some(w) = want {
                        if nt_sign(self.nt[i]) != w {
                            continue;
                        }
                    }
                    let v = self.targets[i] as usize;
                    if !visited[v] {
                        visited[v] = true;
                        next.push(v as u32);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            sizes.push(next.len());
            frontier = next;
        }
        let visited = visited.iter().filter(|&&b| b).count();
        KHopReport {
            frontier_sizes: sizes,
            visited,
        }
    }

    /// CSC-транспонирование (входящие рёбра): O(n + m), один проход
    /// counting-sort. Источники внутри цели отсортированы по индексу.
    /// Impact-слой AIDDE: «кто управляет нейроном».
    pub fn build_in_edges(&self) -> InEdges {
        let n = self.n_nodes;
        let m = self.n_edges;
        let mut offsets = vec![0u32; n + 1];
        for &t in &self.targets {
            offsets[t as usize + 1] += 1;
        }
        for i in 1..=n {
            offsets[i] += offsets[i - 1];
        }
        let mut cursor = offsets.clone();
        let mut sources = vec![0u32; m];
        let mut edge_idx = vec![0u32; m];
        for u in 0..n {
            let (a, b) = (self.offsets[u] as usize, self.offsets[u + 1] as usize);
            for i in a..b {
                let v = self.targets[i] as usize;
                let pos = cursor[v] as usize;
                sources[pos] = u as u32;
                edge_idx[pos] = i as u32;
                cursor[v] += 1;
            }
        }
        InEdges {
            offsets,
            sources,
            edge_idx,
        }
    }
}

/// CSC-представление: входящие рёбра каждого нейрона сплошным срезом.
#[derive(Clone, Debug)]
pub struct InEdges {
    offsets: Vec<u32>,
    sources: Vec<u32>,
    edge_idx: Vec<u32>,
}

impl InEdges {
    /// Входящая степень нейрона (None — индекс вне диапазона).
    pub fn in_degree(&self, v: usize) -> Option<u32> {
        if v + 1 >= self.offsets.len() {
            return None;
        }
        Some(self.offsets[v + 1] - self.offsets[v])
    }

    /// Входящие рёбра нейрона v (источники, отсортированы по индексу).
    /// `Edge.target` здесь — источник, `Edge.idx` — позиция в CSR-массивах.
    pub fn in_edges<'a>(
        &'a self,
        con: &'a Connectome,
        v: usize,
    ) -> Option<impl Iterator<Item = Edge> + 'a> {
        if v + 1 >= self.offsets.len() {
            return None;
        }
        let (a, b) = (self.offsets[v] as usize, self.offsets[v + 1] as usize);
        Some(
            self.sources[a..b]
                .iter()
                .zip(&self.edge_idx[a..b])
                .map(move |(&s, &i)| Edge {
                    idx: i as usize,
                    target: s,
                    weight: con.weights[i as usize],
                    nt: con.nt[i as usize],
                }),
        )
    }
}

/// Таблица нейронов: 138 639 отсортированных u64 root_id (LE).
/// Индексы CSR — позиции в этом массиве.
#[derive(Clone, Debug)]
pub struct ConnectomeNodes {
    ids: Vec<u64>,
}

impl ConnectomeNodes {
    /// Чтение `nodes.bin`.
    pub fn load(path: &Path) -> Result<Self, String> {
        let raw = fs::read(path)
            .map_err(|e| format!("не удалось прочитать {}: {e}", path.display()))?;
        Self::from_raw(&raw).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Разбор сырого буфера u64 LE.
    pub fn from_raw(raw: &[u8]) -> Result<Self, String> {
        if raw.len() % 8 != 0 {
            return Err(format!(
                "размер {} не кратен 8 (u64 LE)",
                raw.len()
            ));
        }
        let ids = raw
            .chunks_exact(8)
            .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
            .collect::<Vec<_>>();
        for w in ids.windows(2) {
            if w[0] >= w[1] {
                return Err("root_id не строго возрастают".to_string());
            }
        }
        Ok(Self { ids })
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// root_id по индексу CSR.
    pub fn root_id(&self, idx: usize) -> Option<u64> {
        self.ids.get(idx).copied()
    }

    /// Индекс CSR по root_id (бинарный поиск по сортировке).
    pub fn idx_of(&self, root_id: u64) -> Option<usize> {
        self.ids.binary_search(&root_id).ok()
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    /// Синтетический FLYCSR1-буфер: 4 узла, edges: (u, v, w, nt).
    /// Общий для тестов connectome и flyops.
    pub(crate) fn synth(core: bool, edges: &[(u32, u32, u16, u8)]) -> Vec<u8> {
        let n_nodes = 4u32;
        let mut rows: Vec<Vec<(u32, u16, u8)>> = vec![vec![]; n_nodes as usize];
        for &(u, v, w, nt) in edges {
            rows[u as usize].push((v, w, nt));
        }
        for row in &mut rows {
            row.sort_by_key(|e| e.0);
        }
        let n_edges: u32 = edges.len() as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(super::MAGIC);
        buf.push(core as u8);
        buf.extend_from_slice(&n_nodes.to_le_bytes());
        buf.extend_from_slice(&n_edges.to_le_bytes());
        let mut acc = 0u32;
        buf.extend_from_slice(&0u32.to_le_bytes());
        for row in &rows {
            acc += row.len() as u32;
            buf.extend_from_slice(&acc.to_le_bytes());
        }
        for row in &rows {
            for e in row {
                buf.extend_from_slice(&e.0.to_le_bytes());
            }
        }
        for row in &rows {
            for e in row {
                buf.extend_from_slice(&e.1.to_le_bytes());
            }
        }
        for row in &rows {
            for e in row {
                buf.push(e.2);
            }
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_support::synth;

    // 1. Синтетика: roundtrip + базовые запросы.
    #[test]
    fn synthetic_roundtrip_and_queries() {
        // 0→1 w10 ach(+1); 0→2 w5 gaba(−1); 1→2 w3 da(0); 2→0 w7 glut(+1); 2→3 w9 gaba(−1)
        let buf = synth(
            false,
            &[(0, 1, 10, 1), (0, 2, 5, 0), (1, 2, 3, 5), (2, 0, 7, 2), (2, 3, 9, 0)],
        );
        let con = Connectome::from_raw(&buf).unwrap();
        assert_eq!(con.n_nodes(), 4);
        assert_eq!(con.n_edges(), 5);
        assert!(!con.is_core());
        assert_eq!(con.out_degree(0), Some(2));
        assert_eq!(con.out_degree(3), Some(0));
        assert_eq!(con.row_targets(0), Some(&[1u32, 2][..]));
        let e = con.edge(0, 2).unwrap();
        assert_eq!((e.weight, e.nt, e.sign()), (5, 0, -1));
        assert_eq!(e.nt_name(), "gaba");
        assert!(con.edge(0, 3).is_none());
        assert!(con.edge(3, 0).is_none());
        assert_eq!(con.signed_weight(0, 1), 10);
        assert_eq!(con.signed_weight(0, 2), -5);
        assert_eq!(con.signed_weight(1, 2), 0); // модулятор da
        assert_eq!(con.signed_weight(0, 3), 0); // связи нет
        assert_eq!(con.total_mass(), 10 + 5 + 3 + 7 + 9);
        // за диапазоном — None/0, не паника
        assert_eq!(con.out_degree(99), None);
        assert_eq!(con.signed_weight(0, 99), 0);
    }

    // 2. Валидация отвергает плохую магию.
    #[test]
    fn rejects_bad_magic() {
        let mut buf = synth(false, &[(0, 1, 5, 1)]);
        buf[3] ^= 0xFF;
        let err = Connectome::from_raw(&buf).unwrap_err();
        assert!(err.contains("магия"), "{err}");
    }

    // 3. Валидация отвергает обрыв и лишние байты.
    #[test]
    fn rejects_truncated_and_oversized() {
        let buf = synth(false, &[(0, 1, 5, 1), (1, 2, 6, 2)]);
        let err = Connectome::from_raw(&buf[..buf.len() - 1]).unwrap_err();
        assert!(err.contains("размер буфера"), "{err}");
        let mut big = buf.clone();
        big.push(0);
        assert!(Connectome::from_raw(&big).unwrap_err().contains("размер буфера"));
        assert!(Connectome::from_raw(&buf[..10]).unwrap_err().contains("короче заголовка"));
    }

    // 4. Валидация отвергает немонотонные offsets.
    #[test]
    fn rejects_nonmonotonic_offsets() {
        let mut buf = synth(false, &[(0, 1, 5, 1), (1, 2, 6, 2)]);
        // offsets[1] = 1 → поднять до 2 (rows: 0→[1], 1→[2]) — разрыв монотонности не создать,
        // вместо этого ломаем хвост: offsets[n] != n_edges.
        let n = 4usize;
        let off_pos = 16 + 4 * (n + 1) - 4;
        buf[off_pos..off_pos + 4].copy_from_slice(&3u32.to_le_bytes());
        let err = Connectome::from_raw(&buf).unwrap_err();
        assert!(err.contains("offsets[n]"), "{err}");
    }

    // 5. Валидация отвергает нестрого отсортированные/вне диапазона targets.
    #[test]
    fn rejects_unsorted_or_out_of_range_targets() {
        // дубликат цели в строке: 0→1 w5, 0→1 w6 (в synth сортируем сами — обходим)
        let mut buf = synth(false, &[(0, 1, 10, 1), (0, 2, 6, 2)]);
        // переставить targets строки 0: [1,2] → [2,1]
        let n = 4usize;
        let tgt_pos = 16 + 4 * (n + 1);
        buf[tgt_pos..tgt_pos + 4].copy_from_slice(&2u32.to_le_bytes());
        buf[tgt_pos + 4..tgt_pos + 8].copy_from_slice(&1u32.to_le_bytes());
        let err = Connectome::from_raw(&buf).unwrap_err();
        assert!(err.contains("не строго возрастают"), "{err}");
        // цель вне диапазона узлов
        let mut buf2 = synth(false, &[(0, 1, 5, 1)]);
        buf2[tgt_pos..tgt_pos + 4].copy_from_slice(&99u32.to_le_bytes());
        let err2 = Connectome::from_raw(&buf2).unwrap_err();
        assert!(err2.contains("вне диапазона"), "{err2}");
    }

    // 6. Валидация отвергает плохой nt_class и core-вес ниже порога.
    #[test]
    fn rejects_bad_nt_or_core_weight() {
        let n = 4usize;
        let mut buf = synth(false, &[(0, 1, 5, 1)]);
        let nt_pos = 16 + 4 * (n + 1) + 4 + 2;
        buf[nt_pos] = 6;
        assert!(Connectome::from_raw(&buf)
            .unwrap_err()
            .contains("nt_class"));
        let buf2 = synth(true, &[(0, 1, 2, 1)]); // core с w=2 < 5
        assert!(Connectome::from_raw(&buf2)
            .unwrap_err()
            .contains("порог ядра"));
        // валидный core проходит
        assert!(Connectome::from_raw(&synth(true, &[(0, 1, 5, 1)])).is_ok());
    }

    // 7. Ротор J = A − Aᵀ: антисимметрия и знаки.
    #[test]
    fn rotor_antisymmetry_and_signs() {
        // 0⇄2 реципрокная НЕсимметричная: 0→2 ach w5, 2→0 gaba w7
        // 1→3 односторонняя glut w4
        let buf = synth(false, &[(0, 2, 5, 1), (2, 0, 7, 0), (1, 3, 4, 2)]);
        let con = Connectome::from_raw(&buf).unwrap();
        // J(0,2) = +5 − (−7) = 12; антисимметрия: J(2,0) = −12
        assert_eq!(con.rotor(0, 2), 12);
        assert_eq!(con.rotor(2, 0), -12);
        // односторонний поток: J(1,3) = +4, J(3,1) = −4
        assert_eq!(con.rotor(1, 3), 4);
        assert_eq!(con.rotor(3, 1), -4);
        // модуляторная связь не участвует в J
        let buf2 = synth(false, &[(0, 1, 100, 5)]);
        let con2 = Connectome::from_raw(&buf2).unwrap();
        assert_eq!(con2.rotor(0, 1), 0);
        // реципрокная симметричная пара гасится
        let buf3 = synth(false, &[(0, 1, 6, 1), (1, 0, 6, 1)]);
        let con3 = Connectome::from_raw(&buf3).unwrap();
        assert_eq!(con3.rotor(0, 1), 0);
    }

    // 8. K-hop: фронты, остановка на пустом фронте, фильтр знака.
    #[test]
    fn k_hop_frontiers_and_filter() {
        // 0→{1,2} ach; 1→3 gaba; 2→3 gaba; 3 без исходящих
        let buf = synth(false, &[(0, 1, 5, 1), (0, 2, 6, 1), (1, 3, 7, 0), (2, 3, 8, 0)]);
        let con = Connectome::from_raw(&buf).unwrap();
        let r = con.k_hop(0, 5, SignFilter::All);
        assert_eq!(r.frontier_sizes, vec![2, 1]); // пустой фронт на хопе 3 не пушится
        assert_eq!(r.visited, 4);
        // только возбуждающие: оба ребра в 3 — gaba, 3 не достигнут
        let r_exc = con.k_hop(0, 5, SignFilter::Excitatory);
        assert_eq!(r_exc.frontier_sizes, vec![2]);
        assert_eq!(r_exc.visited, 3);
        // только тормозные: от 0 нет gaba-рёбер — пусто сразу
        let r_inh = con.k_hop(0, 5, SignFilter::Inhibitory);
        assert!(r_inh.frontier_sizes.is_empty());
        assert_eq!(r_inh.visited, 1);
        // старт вне диапазона
        assert_eq!(con.k_hop(99, 2, SignFilter::All), KHopReport::default());
    }

    // 9. CSC: инварианты входящих рёбер.
    #[test]
    fn csc_invariants() {
        let edges = [(0, 1, 10, 1), (0, 2, 5, 0), (1, 2, 3, 5), (2, 0, 7, 2), (2, 3, 9, 0), (3, 2, 4, 1)];
        let buf = synth(false, &edges);
        let con = Connectome::from_raw(&buf).unwrap();
        let csc = con.build_in_edges();
        // Σ in_degree == n_edges
        let total: u32 = (0..4).map(|v| csc.in_degree(v).unwrap()).sum();
        assert_eq!(total as usize, con.n_edges());
        // каждое ребро u→v видно как in-edge цели v с теми же w/nt
        for &(u, v, w, nt) in &edges {
            let hit = csc
                .in_edges(&con, v as usize)
                .unwrap()
                .find(|e| e.target == u)
                .expect("in-edge не найден");
            assert_eq!((hit.weight, hit.nt), (w, nt));
        }
        // источники отсортированы; изолированный по входу узел — 0
        for v in 0..4 {
            let list: Vec<Edge> = csc.in_edges(&con, v).unwrap().collect();
            let srcs: Vec<u32> = list.iter().map(|e| e.target).collect();
            let mut sorted = srcs.clone();
            sorted.sort_unstable();
            assert_eq!(srcs, sorted);
            assert_eq!(list.len(), csc.in_degree(v).unwrap() as usize);
        }
        assert_eq!(csc.in_degree(3), Some(1));
        assert_eq!(csc.in_degree(99), None);
    }

    // 10. nodes.bin: roundtrip + бинарный поиск.
    #[test]
    fn nodes_roundtrip_and_lookup() {
        let mut raw = Vec::new();
        for id in [10u64, 20, 30, 720575940596125868] {
            raw.extend_from_slice(&id.to_le_bytes());
        }
        let ns = ConnectomeNodes::from_raw(&raw).unwrap();
        assert_eq!(ns.len(), 4);
        assert_eq!(ns.root_id(3), Some(720575940596125868));
        assert_eq!(ns.idx_of(20), Some(1));
        assert_eq!(ns.idx_of(25), None);
        assert_eq!(ns.root_id(99), None);
        // не кратен 8 / не отсортирован
        assert!(ConnectomeNodes::from_raw(&raw[..5]).unwrap_err().contains("кратен"));
        let mut bad = Vec::new();
        for id in [30u64, 10] {
            bad.extend_from_slice(&id.to_le_bytes());
        }
        assert!(ConnectomeNodes::from_raw(&bad)
            .unwrap_err()
            .contains("возрастают"));
    }

    // ---------- Золотые числа v783 (артефакты из git) ----------

    fn artifact(name: &str) -> std::path::PathBuf {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/flywire-connectome")
            .join(name);
        assert!(
            p.exists(),
            "артефакт {} не найден — клонируйте репозиторий целиком",
            p.display()
        );
        p
    }

    // 11. Ядро (7 МБ): структура, массы по медиаторам, K-hop, CSC узла 0.
    #[test]
    fn golden_core_artifact() {
        let con = Connectome::load(&artifact("flywire_v783_core.csr.zst")).unwrap();
        assert!(con.is_core());
        assert_eq!(con.n_nodes(), 138_639);
        assert_eq!(con.n_edges(), 2_700_513);
        assert_eq!(con.total_mass(), 34_153_566);
        // (масса, рёбра) по 6 медиаторам — золотые числа конвертера
        let m = con.mass_by_nt();
        assert_eq!(m[0], (8_354_305, 623_512)); // gaba
        assert_eq!(m[1], (19_563_948, 1_582_068)); // ach
        assert_eq!(m[2], (5_636_506, 448_097)); // glut
        assert_eq!(m[3], (88_646, 9_459)); // oct
        assert_eq!(m[4], (321_025, 21_839)); // ser
        assert_eq!(m[5], (189_136, 15_538)); // da
        // спот-рёбра узла 0
        assert_eq!(con.out_degree(0), Some(13));
        assert_eq!(con.row_targets(0).unwrap()[0], 6135);
        let e = con.edge(0, 6135).unwrap();
        assert_eq!((e.weight, e.nt, e.sign()), (5, 1, 1)); // ach, возбуждающий
        // K-hop от 0: фронты [13, 443], всего 457 (эталон BFS)
        let r = con.k_hop(0, 2, SignFilter::All);
        assert_eq!(r.frontier_sizes, vec![13, 443]);
        assert_eq!(r.visited, 457);
        // CSC узла 0: топ-источник — GABAергический w17
        let csc = con.build_in_edges();
        assert_eq!(csc.in_degree(0).unwrap(), 13);
        let mut ins: Vec<Edge> = csc.in_edges(&con, 0).unwrap().collect();
        ins.sort_by_key(|e| std::cmp::Reverse(e.weight));
        assert_eq!(ins[0].target, 79_529);
        assert_eq!((ins[0].weight, ins[0].nt, ins[0].sign()), (17, 0, -1));
        // rotor: 0→6135 есть, 6135→0 нет — чистый поток
        assert_eq!(con.rotor(0, 6135), 5);
        assert_eq!(con.rotor(6135, 0), -5);
    }

    // 12. Полный граф (35 МБ): масса, E/I/мод баланс, узлы.
    #[test]
    fn golden_full_artifact() {
        let con = Connectome::load(&artifact("flywire_v783_full.csr.zst")).unwrap();
        assert!(!con.is_core());
        assert_eq!(con.n_nodes(), 138_639);
        assert_eq!(con.n_edges(), 15_091_983);
        assert_eq!(con.total_mass(), 54_492_922);
        // баланс синаптической массы: возб/торм/мод
        let m = con.mass_by_nt();
        let exc = m[1].0 + m[2].0;
        let inh = m[0].0;
        let mod_ = m[3].0 + m[4].0 + m[5].0;
        assert_eq!((exc, inh, mod_), (40_087_037, 12_766_424, 1_639_461));
        // E/I 73.6 / 23.4 / 3.0 (в процентах, с допуском округления)
        let tot = con.total_mass() as f64;
        assert!((exc as f64 / tot * 100.0 - 73.6).abs() < 0.05);
        assert!((inh as f64 / tot * 100.0 - 23.4).abs() < 0.05);
        assert!((mod_ as f64 / tot * 100.0 - 3.0).abs() < 0.05);
        // таблица нейронов: первый root_id и обратный поиск
        let ns = ConnectomeNodes::load(&artifact("flywire_v783_nodes.bin")).unwrap();
        assert_eq!(ns.len(), 138_639);
        assert_eq!(ns.root_id(0), Some(720575940596125868));
        assert_eq!(ns.idx_of(720575940596125868), Some(0));
    }
}
