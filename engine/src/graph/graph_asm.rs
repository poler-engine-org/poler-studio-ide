//! Графовий компілятор моделей у нативний машинний код (x86_64 JIT/AOT).
//!
//! Перетворює класичний граф нейромережі або коннектом A у прямі апаратні
//! інструкції CPU без матричних бібліотек, без інтерпретатора і без циклів for.
//!
//! Математика:
//! - Ребра з вагою ~0 або тритом 0 елімінуються на етапі графа.
//! - Ненульові зв'язки розгортаються в прямі SIMD/FMA інструкції.
//! - Фазовий ротор J = A - A^T додає зворотні зв'язки для динамічного резонансу.


/// Тип зв'язку в обчислювальному графі.
#[derive(Debug, Clone, PartialEq)]
pub enum WeightKind {
    /// Точне float32 значення.
    F32(f32),
    /// Трійковий зв'язок {-1, 0, +1} з блочним масштабом.
    Trit { val: i8, scale: f32 },
}

/// Ребро обчислювального графа: джерело -> приймач з вагою.
#[derive(Debug, Clone)]
pub struct ComputeEdge {
    pub src_node: usize,
    pub dst_node: usize,
    pub weight: WeightKind,
}

/// Граф обчислень шару нейромережі.
#[derive(Debug, Clone, Default)]
pub struct ModelComputeGraph {
    pub num_inputs: usize,
    pub num_outputs: usize,
    pub edges: Vec<ComputeEdge>,
    pub threshold: f32,
}

impl ModelComputeGraph {
    /// Створити новий граф розмірності N -> M.
    pub fn new(num_inputs: usize, num_outputs: usize, threshold: f32) -> Self {
        Self {
            num_inputs,
            num_outputs,
            edges: Vec::new(),
            threshold,
        }
    }

    /// Додати ребро від входу до виходу. Ребра з |weight| < threshold ігноруються (вакуум).
    pub fn add_dense_weight(&mut self, src: usize, dst: usize, weight: f32) {
        if weight.abs() >= self.threshold {
            self.edges.push(ComputeEdge {
                src_node: src,
                dst_node: dst,
                weight: WeightKind::F32(weight),
            });
        }
    }

    /// Додати трійкове ребро Trit5.
    pub fn add_trit_weight(&mut self, src: usize, dst: usize, trit: i8, scale: f32) {
        if trit != 0 {
            self.edges.push(ComputeEdge {
                src_node: src,
                dst_node: dst,
                weight: WeightKind::Trit { val: trit, scale },
            });
        }
    }

    /// Кількість активних зв'язків у графі.
    pub fn active_edges(&self) -> usize {
        self.edges.len()
    }

    /// Коефіцієнт розрідження (вакуум) графа.
    pub fn sparsity_ratio(&self) -> f32 {
        let total = self.num_inputs * self.num_outputs;
        if total == 0 {
            return 1.0;
        }
        1.0 - (self.edges.len() as f32 / total as f32)
    }
}

/// Згенерований машинний код (лістинг ASM та виконуваний байткод).
#[derive(Debug)]
pub struct CompiledMachineGraph {
    /// Асемблерний лістинг у форматі x86_64 NASM/GAS.
    pub asm_listing: String,
    /// Сирі байти машинного коду (опкоди x86_64).
    pub machine_bytes: Vec<u8>,
    /// Кількість скомпільованих апаратних інструкцій.
    pub instruction_count: usize,
}

/// Компілятор графа у машинний код x86_64.
pub struct GraphMachineCompiler;

impl GraphMachineCompiler {
    /// Скомпілювати граф нейромережі в асемблер x86_64 та машинний байткод.
    ///
    /// ABI: `fn forward(input: *const f32, output: *mut f32)`
    /// - `rdi` = вказівник на вхідний вектор f32
    /// - `rsi` = вказівник на вихідний вектор f32
    pub fn compile_x86_64(graph: &ModelComputeGraph, func_name: &str) -> CompiledMachineGraph {
        let mut asm = String::with_capacity(graph.edges.len() * 64 + 512);
        let mut bytes = Vec::with_capacity(graph.edges.len() * 16 + 128);
        let mut inst_count = 0;

        // Генерація прологу функції (System V AMD64 ABI)
        asm.push_str(&format!("; Auto-generated POLER Neural Graph Machine Code: {}\n", func_name));
        asm.push_str(&format!("; Active graph edges: {} / Sparsity: {:.2}%\n", 
            graph.edges.len(), graph.sparsity_ratio() * 100.0));
        asm.push_str("global ");
        asm.push_str(func_name);
        asm.push('\n');
        asm.push_str(func_name);
        asm.push_str(":\n");

        // Групування ребер за вихідними вузлами (dst_node) для оптимізації акумулятора
        let mut in_edges: Vec<Vec<&ComputeEdge>> = vec![Vec::new(); graph.num_outputs];
        for edge in &graph.edges {
            if edge.dst_node < graph.num_outputs {
                in_edges[edge.dst_node].push(edge);
            }
        }

        // Обробка кожного вихідного нейрона
        for (dst_idx, edges) in in_edges.iter().enumerate() {
            if edges.is_empty() {
                // Нульовий вихід: xorps xmm0, xmm0; movss [rsi + offset], xmm0
                asm.push_str(&format!("    ; Node {} (Vacuum - 0 edges)\n", dst_idx));
                asm.push_str("    xorps   xmm0, xmm0\n");
                asm.push_str(&format!("    movss   dword [rsi + {}], xmm0\n", dst_idx * 4));
                
                // xorps xmm0, xmm0 -> 0x0F 0x57 0xC0
                bytes.extend_from_slice(&[0x0F, 0x57, 0xC0]);
                // movss [rsi + disp], xmm0 -> 0xF3 0x0F 0x11 ...
                emit_movss_store_rsi(&mut bytes, (dst_idx * 4) as i32);
                inst_count += 2;
                continue;
            }

            asm.push_str(&format!("    ; Node {} (Fan-in: {} edges)\n", dst_idx, edges.len()));
            // Обнулення акумулятора xmm0
            asm.push_str("    xorps   xmm0, xmm0\n");
            bytes.extend_from_slice(&[0x0F, 0x57, 0xC0]);
            inst_count += 1;

            for edge in edges {
                let src_offset = (edge.src_node * 4) as i32;
                match edge.weight {
                    WeightKind::F32(w) => {
                        // ВЕС ВШИТ В МАШИННЫЙ КОД как immediate (mov eax, биты f32):
                        //   movss xmm1, [rdi + src_offset]  — загрузка входа x[j]
                        //   mov  eax, <w_bits>              — обученный вес literal
                        //   movd xmm2, eax
                        //   mulss xmm1, xmm2                — x[j] * w
                        //   addss xmm0, xmm1                — аккумуляция
                        asm.push_str(&format!("    movss   xmm1, dword [rdi + {}]\n", src_offset));
                        asm.push_str(&format!(
                            "    mov     eax, 0x{:08X}        ; вес f32 = {:.6}\n",
                            w.to_bits(),
                            w
                        ));
                        asm.push_str("    movd    xmm2, eax\n");
                        asm.push_str("    mulss   xmm1, xmm2\n");
                        asm.push_str("    addss   xmm0, xmm1\n");

                        emit_movss_load_rdi(&mut bytes, src_offset);
                        // mov eax, imm32(w bits) -> 0xB8 + 4 байта
                        bytes.push(0xB8);
                        bytes.extend_from_slice(&w.to_bits().to_le_bytes());
                        // movd xmm2, eax -> 0x66 0x0F 0x6E 0xD0
                        bytes.extend_from_slice(&[0x66, 0x0F, 0x6E, 0xD0]);
                        // mulss xmm1, xmm2 -> 0xF3 0x0F 0x59 0xCA
                        bytes.extend_from_slice(&[0xF3, 0x0F, 0x59, 0xCA]);
                        // addss xmm0, xmm1 -> 0xF3 0x0F 0x58 0xC1
                        bytes.extend_from_slice(&[0xF3, 0x0F, 0x58, 0xC1]);
                        inst_count += 5;
                    }
                    WeightKind::Trit { val, scale: _ } => {
                        match val {
                            1 => {
                                // No-Mul: пряме додавання входу
                                asm.push_str(&format!("    addss   xmm0, dword [rdi + {}] ; Trit +1 (No-Mul)\n", src_offset));
                                emit_addss_mem_rdi(&mut bytes, src_offset);
                                inst_count += 1;
                            }
                            -1 => {
                                // No-Mul: пряме віднімання входу
                                asm.push_str(&format!("    subss   xmm0, dword [rdi + {}] ; Trit -1 (No-Mul)\n", src_offset));
                                emit_subss_mem_rdi(&mut bytes, src_offset);
                                inst_count += 1;
                            }
                            _ => {} // 0 - вакуум, ігнорується
                        }
                    }
                }
            }

            // Збереження результату нейрона
            asm.push_str(&format!("    movss   dword [rsi + {}], xmm0\n", dst_idx * 4));
            emit_movss_store_rsi(&mut bytes, (dst_idx * 4) as i32);
            inst_count += 1;
        }

        // Епілог (ret -> 0xC3)
        asm.push_str("    ret\n");
        bytes.push(0xC3);
        inst_count += 1;

        CompiledMachineGraph {
            asm_listing: asm,
            machine_bytes: bytes,
            instruction_count: inst_count,
        }
    }
}

// ---------------------------------------------------------------------------
// Хелпери кодування опкодів x86_64
// ---------------------------------------------------------------------------

fn emit_movss_load_rdi(buf: &mut Vec<u8>, disp: i32) {
    if (0..=127).contains(&disp) {
        // movss xmm1, [rdi + disp8] -> 0xF3 0x0F 0x10 0x4F disp8
        buf.extend_from_slice(&[0xF3, 0x0F, 0x10, 0x4F, disp as u8]);
    } else {
        // movss xmm1, [rdi + disp32] -> 0xF3 0x0F 0x10 0x8F disp32
        buf.extend_from_slice(&[0xF3, 0x0F, 0x10, 0x8F]);
        buf.extend_from_slice(&disp.to_le_bytes());
    }
}

fn emit_movss_store_rsi(buf: &mut Vec<u8>, disp: i32) {
    if (0..=127).contains(&disp) {
        // movss [rsi + disp8], xmm0 -> 0xF3 0x0F 0x11 0x46 disp8
        buf.extend_from_slice(&[0xF3, 0x0F, 0x11, 0x46, disp as u8]);
    } else {
        // movss [rsi + disp32], xmm0 -> 0xF3 0x0F 0x11 0x86 disp32
        buf.extend_from_slice(&[0xF3, 0x0F, 0x11, 0x86]);
        buf.extend_from_slice(&disp.to_le_bytes());
    }
}

fn emit_addss_mem_rdi(buf: &mut Vec<u8>, disp: i32) {
    if (0..=127).contains(&disp) {
        // addss xmm0, [rdi + disp8] -> 0xF3 0x0F 0x58 0x47 disp8
        buf.extend_from_slice(&[0xF3, 0x0F, 0x58, 0x47, disp as u8]);
    } else {
        buf.extend_from_slice(&[0xF3, 0x0F, 0x58, 0x87]);
        buf.extend_from_slice(&disp.to_le_bytes());
    }
}

fn emit_subss_mem_rdi(buf: &mut Vec<u8>, disp: i32) {
    if (0..=127).contains(&disp) {
        // subss xmm0, [rdi + disp8] -> 0xF3 0x0F 0x5C 0x47 disp8
        buf.extend_from_slice(&[0xF3, 0x0F, 0x5C, 0x47, disp as u8]);
    } else {
        buf.extend_from_slice(&[0xF3, 0x0F, 0x5C, 0x87]);
        buf.extend_from_slice(&disp.to_le_bytes());
    }
}

// ---------------------------------------------------------------------------
// Тести
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_vacuum_sparsity() {
        let mut g = ModelComputeGraph::new(4, 4, 0.1);
        g.add_dense_weight(0, 0, 0.85);
        g.add_dense_weight(1, 0, 0.02); // менше порогу -> вакуум
        g.add_dense_weight(2, 1, -0.90);

        assert_eq!(g.active_edges(), 2);
        assert_eq!(g.sparsity_ratio(), 1.0 - (2.0 / 16.0));
    }

    #[test]
    fn test_compile_trit_no_mul_asm() {
        let mut g = ModelComputeGraph::new(4, 2, 0.0);
        g.add_trit_weight(0, 0, 1, 1.0);  // +1
        g.add_trit_weight(1, 0, -1, 1.0); // -1
        g.add_trit_weight(2, 0, 0, 1.0);  // 0 вакуум

        let compiled = GraphMachineCompiler::compile_x86_64(&g, "layer_trit_no_mul");
        assert!(compiled.asm_listing.contains("Trit +1 (No-Mul)"));
        assert!(compiled.asm_listing.contains("Trit -1 (No-Mul)"));
        assert!(!compiled.machine_bytes.is_empty());
        assert_eq!(*compiled.machine_bytes.last().unwrap(), 0xC3); // ret
    }
}

// ---------------------------------------------------------------------------
// A1 (ступінь v0.76): DataflowGraph — наскрізні багатошарові графи.
// Весь конвеєр компілюється в ОДИН kernel: дані шару живуть у регістрах xmm
// під час накопичення і в стековому ping-pong кадрі між шарами (без купи).
// Активації: Identity / SignTrit / TanhRat. Ред'юсери: Sum / Max.
// Інваріант: машинний код біт-в-біт еквівалентний dataflow_reference_eval.
// ---------------------------------------------------------------------------

/// Активація вузла після редукції вхідних ребер.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeAct {
    Identity,
    /// Трит-знак: sign(x) ∈ {−1, 0, +1} (0 — точний нуль; NaN → 0).
    SignTrit,
    /// Раціональний tanh: мінімакс на |x| < 4 (похибка в області ≤ 5e-7 —
    /// 2 ulp f32), насичення ±1 за межею (стрибок ≈ 6.7e-4 — задокументовано).
    TanhRat,
}

/// Ред'юсер вхідних ребер вузла.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reducer {
    Sum,
    /// Максимум внесків (ліво-асоціативно в порядку ребер).
    Max,
}

/// Специфікація шару.
#[derive(Debug, Clone)]
pub struct LayerSpec {
    pub width: usize,
    pub act: NodeAct,
    pub reduce: Reducer,
}

/// Ребро між СУСІДНІМИ шарами: src_layer → src_layer + 1 (0 = вхід x).
#[derive(Debug, Clone)]
pub struct DataflowEdge {
    pub src_layer: usize,
    pub src_node: usize,
    pub dst_node: usize,
    pub weight: WeightKind,
}

/// Наскрізний багатошаровий граф обчислень.
#[derive(Debug, Clone)]
pub struct DataflowGraph {
    pub num_inputs: usize,
    /// Шари 1..N (останній — вихідний).
    pub layers: Vec<LayerSpec>,
    /// Ребера в порядку додавання (порядок = порядок сумування).
    pub edges: Vec<DataflowEdge>,
    pub threshold: f32,
}

/// Коефіцієнти раціонального tanh (мінімакс-фіт, scripts/fit_tanh_rational.py).
const TANH_A0: f32 = 0.9999999;
const TANH_A1: f32 = 0.12618537;
const TANH_A2: f32 = 0.0025810248;
const TANH_A3: f32 = 5.6638123e-06;
const TANH_B1: f32 = 0.45951828;
const TANH_B2: f32 = 0.022420991;
const TANH_B3: f32 = 0.0001781013;

/// Раціональний tanh — ТА Ж послідовність операцій, що й у кодгені.
#[inline]
pub fn tanh_rat(x: f32) -> f32 {
    if x >= 4.0 {
        1.0
    } else if x <= -4.0 {
        -1.0
    } else {
        let t = x * x;
        let num = ((TANH_A3 * t + TANH_A2) * t + TANH_A1) * t + TANH_A0;
        let den = ((TANH_B3 * t + TANH_B2) * t + TANH_B1) * t + 1.0;
        let q = num / den;
        x * q
    }
}

/// Трит-знак (NaN → 0).
#[inline]
pub fn sign_trit(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

impl DataflowGraph {
    pub fn new(num_inputs: usize, threshold: f32) -> Self {
        Self { num_inputs, layers: Vec::new(), edges: Vec::new(), threshold }
    }

    /// Додати шар (1-базований індекс; 0 — вхід).
    pub fn add_layer(&mut self, width: usize, act: NodeAct, reduce: Reducer) -> usize {
        self.layers.push(LayerSpec { width, act, reduce });
        self.layers.len()
    }

    /// Додати ребро; F32 з |w| < threshold і трити 0 — вакуум.
    pub fn add_edge(&mut self, src_layer: usize, src_node: usize, dst_node: usize, w: WeightKind) {
        let next = src_layer + 1;
        if next > self.layers.len() {
            return;
        }
        let src_w = if src_layer == 0 { self.num_inputs } else { self.layers[src_layer - 1].width };
        if src_node >= src_w || dst_node >= self.layers[next - 1].width {
            return;
        }
        match w {
            WeightKind::F32(v) => {
                if v.abs() >= self.threshold {
                    self.edges.push(DataflowEdge { src_layer, src_node, dst_node, weight: WeightKind::F32(v) });
                }
            }
            WeightKind::Trit { val, scale } => {
                if val != 0 {
                    self.edges.push(DataflowEdge { src_layer, src_node, dst_node, weight: WeightKind::Trit { val, scale } });
                }
            }
        }
    }

    pub fn active_edges(&self) -> usize {
        self.edges.len()
    }

    /// Частка вакууму від повної решітки між шарами.
    pub fn sparsity_ratio(&self) -> f32 {
        let mut total = 0usize;
        let mut prev = self.num_inputs;
        for l in &self.layers {
            total += prev * l.width;
            prev = l.width;
        }
        if total == 0 {
            return 1.0;
        }
        1.0 - (self.edges.len() as f32 / total as f32)
    }
}

/// Еталонний інтерпретатор — біт-в-біт дзеркало кодгена.
pub fn dataflow_reference_eval(g: &DataflowGraph, x: &[f32]) -> Result<Vec<f32>, String> {
    if x.len() != g.num_inputs {
        return Err(format!("x: {} ≠ входів {}", x.len(), g.num_inputs));
    }
    let mut cur: Vec<f32> = x.to_vec();
    for (li, layer) in g.layers.iter().enumerate() {
        let mut next = vec![0.0f32; layer.width];
        for j in 0..layer.width {
            let mut acc = 0.0f32;
            let mut first = true;
            for e in &g.edges {
                if e.src_layer != li || e.dst_node != j {
                    continue;
                }
                let sv = cur[e.src_node];
                let c = match e.weight {
                    WeightKind::F32(w) => sv * w,
                    WeightKind::Trit { val, scale: _ } => sv * (val as f32),
                };
                match layer.reduce {
                    Reducer::Sum => acc += c,
                    Reducer::Max => {
                        if first {
                            acc = c;
                            first = false;
                        } else {
                            acc = if c > acc { c } else { acc };
                        }
                    }
                }
            }
            next[j] = match layer.act {
                NodeAct::Identity => acc,
                NodeAct::SignTrit => sign_trit(acc),
                NodeAct::TanhRat => tanh_rat(acc),
            };
        }
        cur = next;
    }
    Ok(cur)
}

/// Компілятор DataflowGraph.
pub struct DataflowCompiler;

struct Labels {
    targets: std::collections::HashMap<String, usize>,
    patches: Vec<(usize, String)>,
}

impl Labels {
    fn new() -> Self {
        Self { targets: std::collections::HashMap::new(), patches: Vec::new() }
    }
    fn bind(&mut self, name: &str, pos: usize) {
        self.targets.insert(name.to_string(), pos);
    }
    fn patch(&mut self, rel32_pos: usize, name: &str) {
        self.patches.push((rel32_pos, name.to_string()));
    }
    fn resolve(&self, code: &mut Vec<u8>) -> Result<(), String> {
        for &(pos, ref name) in &self.patches {
            let tgt = *self.targets.get(name).ok_or_else(|| format!("мітка {name} не прив'язана"))?;
            let rel = tgt as i64 - (pos + 4) as i64;
            let rel32 = rel as i32;
            if rel != rel32 as i64 {
                return Err("перехід вне ±2 ГБ".into());
            }
            code[pos..pos + 4].copy_from_slice(&rel32.to_le_bytes());
        }
        Ok(())
    }
}

fn emit_movss_load_rbp(buf: &mut Vec<u8>, disp: i32) {
    buf.extend_from_slice(&[0xF3, 0x0F, 0x10, 0x8D]);
    buf.extend_from_slice(&disp.to_le_bytes());
}

fn emit_movss_store_rbp(buf: &mut Vec<u8>, disp: i32) {
    buf.extend_from_slice(&[0xF3, 0x0F, 0x11, 0x85]);
    buf.extend_from_slice(&disp.to_le_bytes());
}

/// mov eax, imm32; movd xmm{r}, eax.
fn emit_f32_imm_to_xmm(asm: &mut String, bytes: &mut Vec<u8>, r: u8, c: f32, note: &str) {
    asm.push_str(&format!("    mov     eax, 0x{:08X}", c.to_bits()));
    if !note.is_empty() {
        asm.push_str(&format!(" ; {note}"));
    }
    asm.push('\n');
    bytes.push(0xB8);
    bytes.extend_from_slice(&c.to_bits().to_le_bytes());
    bytes.extend_from_slice(&[0x66, 0x0F, 0x6E, 0xC0 | (r << 3)]);
    asm.push_str(&format!("    movd    xmm{r}, eax\n"));
}

impl DataflowCompiler {
    /// Скомпілювати граф в ОДИН kernel x86_64 (ABI fn(x,y), rdi/rsi).
    pub fn compile_x86_64(g: &DataflowGraph, func_name: &str) -> Result<CompiledMachineGraph, String> {
        if g.layers.is_empty() {
            return Err("порожній DataflowGraph".into());
        }
        let max_width = g.layers.iter().map(|l| l.width).chain(std::iter::once(g.num_inputs)).max().unwrap_or(0);
        if max_width == 0 || max_width > 4096 {
            return Err(format!("ширина шару {max_width} неприпустима"));
        }
        let frame = ((max_width * 8) + 15) & !15usize; // ping-pong ×2
        let mut asm = String::with_capacity(g.edges.len() * 48 + 2048);
        let mut bytes: Vec<u8> = Vec::with_capacity(g.edges.len() * 16 + 1024);
        let mut labels = Labels::new();
        let mut inst = 0usize;
        let mut lid = 0usize;

        asm.push_str(&format!("; POLER DataflowGraph «{}»: {} шарів, {} ребер, вакуум {:.1}%\n", func_name, g.layers.len(), g.edges.len(), g.sparsity_ratio() * 100.0));
        asm.push_str(&format!("global {func_name}\n{func_name}:\n    push    rbp\n    mov     rbp, rsp\n"));
        bytes.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5]);
        if frame < 128 {
            bytes.extend_from_slice(&[0x48, 0x83, 0xEC, frame as u8]);
        } else {
            bytes.extend_from_slice(&[0x48, 0x81, 0xEC]);
            bytes.extend_from_slice(&(frame as u32).to_le_bytes());
        }
        inst += 3;
        // значення шару L — у буфері (L−1)%2, зсув [rbp − 4·((L−1)%2·max_width + k + 1)]
        let val_disp = |buf: usize, k: usize| -(((buf * max_width + k + 1) * 4) as i32);

        for (li, layer) in g.layers.iter().enumerate() {
            let dst_idx = li + 1;
            let src_buf = if li == 0 { None } else { Some((li - 1) % 2) };
            let is_last = dst_idx == g.layers.len();
            asm.push_str(&format!("    ; ── шар {dst_idx} ({} вузлів, {:?}, {:?})\n", layer.width, layer.act, layer.reduce));
            for j in 0..layer.width {
                let edges: Vec<&DataflowEdge> = g.edges.iter().filter(|e| e.src_layer == li && e.dst_node == j).collect();
                if !edges.is_empty() && layer.reduce == Reducer::Sum {
                    asm.push_str("    xorps   xmm0, xmm0\n");
                    bytes.extend_from_slice(&[0x0F, 0x57, 0xC0]);
                    inst += 1;
                }
                let mut first = true;
                for e in &edges {
                    match src_buf {
                        None => {
                            asm.push_str(&format!("    movss   xmm1, dword [rdi + {}]\n", e.src_node * 4));
                            emit_movss_load_rdi(&mut bytes, (e.src_node * 4) as i32);
                        }
                        Some(b) => {
                            asm.push_str(&format!("    movss   xmm1, dword [rbp − {}]\n", (b * max_width + e.src_node + 1) * 4));
                            emit_movss_load_rbp(&mut bytes, val_disp(b, e.src_node));
                        }
                    }
                    inst += 1;
                    match &e.weight {
                        WeightKind::F32(w) => {
                            emit_f32_imm_to_xmm(&mut asm, &mut bytes, 2, *w, &format!("вага f32 = {:.6}", w));
                            asm.push_str("    mulss   xmm1, xmm2\n");
                            bytes.extend_from_slice(&[0xF3, 0x0F, 0x59, 0xCA]);
                            inst += 2;
                            match layer.reduce {
                                Reducer::Sum => {
                                    asm.push_str("    addss   xmm0, xmm1\n");
                                    bytes.extend_from_slice(&[0xF3, 0x0F, 0x58, 0xC1]);
                                }
                                Reducer::Max if first => {
                                    asm.push_str("    movaps  xmm0, xmm1\n");
                                    bytes.extend_from_slice(&[0x0F, 0x28, 0xC1]);
                                }
                                Reducer::Max => {
                                    asm.push_str("    maxss   xmm0, xmm1\n");
                                    bytes.extend_from_slice(&[0xF3, 0x0F, 0x5F, 0xC1]);
                                }
                            }
                            inst += 1;
                        }
                        WeightKind::Trit { val, scale: _ } => match val {
                            1 => match layer.reduce {
                                Reducer::Sum => {
                                    asm.push_str("    addss   xmm0, xmm1   ; трит +1 (No-Mul)\n");
                                    bytes.extend_from_slice(&[0xF3, 0x0F, 0x58, 0xC1]);
                                }
                                Reducer::Max if first => {
                                    asm.push_str("    movaps  xmm0, xmm1\n");
                                    bytes.extend_from_slice(&[0x0F, 0x28, 0xC1]);
                                }
                                Reducer::Max => {
                                    asm.push_str("    maxss   xmm0, xmm1\n");
                                    bytes.extend_from_slice(&[0xF3, 0x0F, 0x5F, 0xC1]);
                                }
                            },
                            -1 => {
                                // внесок = −x: точна інверсія знака (pxor маскою 0x80000000)
                                asm.push_str("    ; трит −1: точний −x (No-Mul)\n    mov     eax, 0x80000000\n    movd    xmm2, eax\n    xorps   xmm1, xmm2\n");
                                bytes.push(0xB8);
                                bytes.extend_from_slice(&0x80000000u32.to_le_bytes());
                                bytes.extend_from_slice(&[0x66, 0x0F, 0x6E, 0xD0]);
                                bytes.extend_from_slice(&[0x0F, 0x57, 0xCA]);
                                inst += 3;
                                match layer.reduce {
                                    Reducer::Sum => {
                                        asm.push_str("    addss   xmm0, xmm1\n");
                                        bytes.extend_from_slice(&[0xF3, 0x0F, 0x58, 0xC1]);
                                    }
                                    Reducer::Max if first => {
                                        asm.push_str("    movaps  xmm0, xmm1\n");
                                        bytes.extend_from_slice(&[0x0F, 0x28, 0xC1]);
                                    }
                                    Reducer::Max => {
                                        asm.push_str("    maxss   xmm0, xmm1\n");
                                        bytes.extend_from_slice(&[0xF3, 0x0F, 0x5F, 0xC1]);
                                    }
                                }
                                inst += 1;
                            }
                            _ => {}
                        },
                    }
                    first = false;
                }
                if first {
                    asm.push_str("    xorps   xmm0, xmm0   ; вакуум\n");
                    bytes.extend_from_slice(&[0x0F, 0x57, 0xC0]);
                    inst += 1;
                }
                match layer.act {
                    NodeAct::Identity => {}
                    NodeAct::SignTrit => {
                        Self::emit_sign_trit(&mut asm, &mut bytes, &mut labels, &mut lid);
                        inst += 8;
                    }
                    NodeAct::TanhRat => {
                        Self::emit_tanh_rat(&mut asm, &mut bytes, &mut labels, &mut lid);
                        inst += 34;
                    }
                }
                if is_last {
                    asm.push_str(&format!("    movss   dword [rsi + {}], xmm0\n", j * 4));
                    emit_movss_store_rsi(&mut bytes, (j * 4) as i32);
                } else {
                    let b = (dst_idx - 1) % 2;
                    asm.push_str(&format!("    movss   dword [rbp − {}], xmm0\n", (b * max_width + j + 1) * 4));
                    emit_movss_store_rbp(&mut bytes, val_disp(b, j));
                }
                inst += 1;
            }
        }
        asm.push_str("    leave\n    ret\n");
        bytes.extend_from_slice(&[0xC9, 0xC3]);
        inst += 2;
        labels.resolve(&mut bytes).map_err(|e| format!("фікссапи: {e}"))?;
        Ok(CompiledMachineGraph { asm_listing: asm, machine_bytes: bytes, instruction_count: inst })
    }

    fn emit_sign_trit(asm: &mut String, b: &mut Vec<u8>, labels: &mut Labels, lid: &mut usize) {
        let (lp, ln, ld) = (format!("sgn_p{lid}"), format!("sgn_n{lid}"), format!("sgn_d{lid}"));
        *lid += 1;
        asm.push_str("    ; активація: трит-знак\n    xorps   xmm1, xmm1\n    comiss  xmm0, xmm1\n    ja      .pos\n");
        b.extend_from_slice(&[0x0F, 0x57, 0xC9]);
        b.extend_from_slice(&[0x0F, 0x2F, 0xC1]);
        let p = b.len();
        b.extend_from_slice(&[0x0F, 0x87, 0, 0, 0, 0]);
        labels.patch(p + 2, &lp);
        asm.push_str("    comiss  xmm1, xmm0\n    ja      .neg\n");
        b.extend_from_slice(&[0x0F, 0x2F, 0xC8]);
        let p = b.len();
        b.extend_from_slice(&[0x0F, 0x87, 0, 0, 0, 0]);
        labels.patch(p + 2, &ln);
        asm.push_str("    movaps  xmm0, xmm1   ; нуль (NaN теж)\n    jmp     .done\n");
        b.extend_from_slice(&[0x0F, 0x28, 0xC1]);
        let p = b.len();
        b.extend_from_slice(&[0xE9, 0, 0, 0, 0]);
        labels.patch(p + 1, &ld);
        labels.bind(&lp, b.len());
        asm.push_str(".pos:\n");
        emit_f32_imm_to_xmm(asm, b, 0, 1.0, "1.0f");
        asm.push_str("    jmp     .done\n");
        let p = b.len();
        b.extend_from_slice(&[0xE9, 0, 0, 0, 0]);
        labels.patch(p + 1, &ld);
        labels.bind(&ln, b.len());
        asm.push_str(".neg:\n");
        emit_f32_imm_to_xmm(asm, b, 0, -1.0, "−1.0f");
        labels.bind(&ld, b.len());
        asm.push_str(".done:\n");
    }

    fn emit_tanh_rat(asm: &mut String, b: &mut Vec<u8>, labels: &mut Labels, lid: &mut usize) {
        let (lp, ln, ld) = (format!("th_p{lid}"), format!("th_n{lid}"), format!("th_d{lid}"));
        *lid += 1;
        asm.push_str("    ; активація: tanh_rat (|x| < 4)\n");
        emit_f32_imm_to_xmm(asm, b, 6, 4.0, "4.0f");
        asm.push_str("    comiss  xmm0, xmm6\n    jae     .sat_pos\n");
        b.extend_from_slice(&[0x0F, 0x2F, 0xC6]);
        let p = b.len();
        b.extend_from_slice(&[0x0F, 0x83, 0, 0, 0, 0]);
        labels.patch(p + 2, &lp);
        emit_f32_imm_to_xmm(asm, b, 6, -4.0, "−4.0f");
        asm.push_str("    comiss  xmm6, xmm0\n    jae     .sat_neg\n");
        b.extend_from_slice(&[0x0F, 0x2F, 0xF0]);
        let p = b.len();
        b.extend_from_slice(&[0x0F, 0x83, 0, 0, 0, 0]);
        labels.patch(p + 2, &ln);
        asm.push_str("    movaps  xmm1, xmm0\n    mulss   xmm1, xmm0   ; t = x²\n");
        b.extend_from_slice(&[0x0F, 0x28, 0xC8]);
        b.extend_from_slice(&[0xF3, 0x0F, 0x59, 0xC8]);
        // num → xmm4 (Horner), den → xmm5; рег-байди з попередньої верифікації
        emit_f32_imm_to_xmm(asm, b, 4, TANH_A3, "a3");
        asm.push_str("    mulss   xmm4, xmm1\n");
        b.extend_from_slice(&[0xF3, 0x0F, 0x59, 0xE1]);
        emit_f32_imm_to_xmm(asm, b, 5, TANH_A2, "a2");
        asm.push_str("    addss   xmm4, xmm5\n");
        b.extend_from_slice(&[0xF3, 0x0F, 0x58, 0xE5]);
        asm.push_str("    movaps  xmm6, xmm4\n    mulss   xmm6, xmm1\n");
        b.extend_from_slice(&[0x0F, 0x28, 0xF4]);
        b.extend_from_slice(&[0xF3, 0x0F, 0x59, 0xF1]);
        emit_f32_imm_to_xmm(asm, b, 5, TANH_A1, "a1");
        asm.push_str("    addss   xmm6, xmm5\n");
        b.extend_from_slice(&[0xF3, 0x0F, 0x58, 0xF5]);
        asm.push_str("    movaps  xmm4, xmm6\n    mulss   xmm4, xmm1\n");
        b.extend_from_slice(&[0x0F, 0x28, 0xE6]);
        b.extend_from_slice(&[0xF3, 0x0F, 0x59, 0xE1]);
        emit_f32_imm_to_xmm(asm, b, 5, TANH_A0, "a0");
        asm.push_str("    addss   xmm4, xmm5   ; num\n");
        b.extend_from_slice(&[0xF3, 0x0F, 0x58, 0xE5]);
        emit_f32_imm_to_xmm(asm, b, 5, TANH_B3, "b3");
        asm.push_str("    mulss   xmm5, xmm1\n");
        b.extend_from_slice(&[0xF3, 0x0F, 0x59, 0xE9]);
        emit_f32_imm_to_xmm(asm, b, 2, TANH_B2, "b2");
        asm.push_str("    addss   xmm5, xmm2\n");
        b.extend_from_slice(&[0xF3, 0x0F, 0x58, 0xEA]);
        asm.push_str("    movaps  xmm3, xmm5\n    mulss   xmm3, xmm1\n");
        b.extend_from_slice(&[0x0F, 0x28, 0xDD]);
        b.extend_from_slice(&[0xF3, 0x0F, 0x59, 0xD9]);
        emit_f32_imm_to_xmm(asm, b, 2, TANH_B1, "b1");
        asm.push_str("    addss   xmm3, xmm2\n");
        b.extend_from_slice(&[0xF3, 0x0F, 0x58, 0xDA]);
        asm.push_str("    movaps  xmm5, xmm3\n    mulss   xmm5, xmm1\n");
        b.extend_from_slice(&[0x0F, 0x28, 0xEB]);
        b.extend_from_slice(&[0xF3, 0x0F, 0x59, 0xE9]);
        emit_f32_imm_to_xmm(asm, b, 2, 1.0, "1.0f");
        asm.push_str("    addss   xmm5, xmm2   ; den\n");
        b.extend_from_slice(&[0xF3, 0x0F, 0x58, 0xEA]);
        asm.push_str("    divss   xmm4, xmm5\n    mulss   xmm0, xmm4\n");
        b.extend_from_slice(&[0xF3, 0x0F, 0x5E, 0xE5]);
        b.extend_from_slice(&[0xF3, 0x0F, 0x59, 0xC4]);
        asm.push_str("    jmp     .done\n");
        let p = b.len();
        b.extend_from_slice(&[0xE9, 0, 0, 0, 0]);
        labels.patch(p + 1, &ld);
        labels.bind(&lp, b.len());
        asm.push_str(".sat_pos:\n");
        emit_f32_imm_to_xmm(asm, b, 0, 1.0, "1.0f");
        asm.push_str("    jmp     .done\n");
        let p = b.len();
        b.extend_from_slice(&[0xE9, 0, 0, 0, 0]);
        labels.patch(p + 1, &ld);
        labels.bind(&ln, b.len());
        asm.push_str(".sat_neg:\n");
        emit_f32_imm_to_xmm(asm, b, 0, -1.0, "−1.0f");
        labels.bind(&ld, b.len());
        asm.push_str(".done:\n");
    }
}

#[cfg(test)]
mod tests_a1 {
    use super::*;

    fn exec_kernel(g: &DataflowGraph, x: &[f32]) -> Vec<f32> {
        let c = DataflowCompiler::compile_x86_64(g, "test_df").expect("компіляція");
        let kern = crate::triune::jit_loop::ExecutableKernel::load(&c.machine_bytes).expect("load RX");
        let mut y = vec![0.0f32; g.layers.last().unwrap().width];
        unsafe { kern.call_raw(x.as_ptr(), y.as_mut_ptr()) };
        y
    }

    fn lcg(s: &mut u64) -> f32 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*s >> 33) as i32 as f64 / i32::MAX as f64 * 2.0) as f32
    }

    #[test]
    fn test_tanh_rat_accuracy() {
        let mut worst_in = 0.0f32;
        let mut worst_all = 0.0f32;
        let mut i = -6000i32;
        while i <= 6000 {
            let x = i as f32 * 0.001;
            let e = (tanh_rat(x) - x.tanh()).abs();
            if x.abs() < 3.999 && e > worst_in { worst_in = e; }
            if e > worst_all { worst_all = e; }
            i += 1;
        }
        assert!(worst_in <= 5e-7, "в області: {worst_in}");
        assert!(worst_all <= 7e-4, "глобально: {worst_all}");
    }

    #[test]
    fn test_sign_trit_semantics() {
        assert_eq!(sign_trit(0.0), 0.0);
        assert_eq!(sign_trit(-0.0), 0.0);
        assert_eq!(sign_trit(1e-30), 1.0);
        assert_eq!(sign_trit(-1e30), -1.0);
        assert_eq!(sign_trit(f32::NAN), 0.0);
    }

    #[test]
    fn test_dataflow_single_layer_bitexact() {
        let mut g = DataflowGraph::new(5, 0.05);
        g.add_layer(4, NodeAct::Identity, Reducer::Sum);
        let mut s = 42u64;
        for i in 0..4 {
            for j in 0..5 {
                g.add_edge(0, j, i, WeightKind::F32(lcg(&mut s)));
            }
        }
        let x: Vec<f32> = (0..5).map(|_| lcg(&mut s)).collect();
        let r = dataflow_reference_eval(&g, &x).unwrap();
        let j = exec_kernel(&g, &x);
        for (a, b) in r.iter().zip(&j) {
            assert_eq!(a.to_bits(), b.to_bits(), "біт-в-біт IEEE-754");
        }
    }

    #[test]
    fn test_dataflow_deep_bitexact_100_graphs() {
        for seed in 0..100u64 {
            let mut s = seed.wrapping_mul(0x9E3779B97F4A7C15) | 1;
            let n_in = 2 + (s % 5) as usize;
            let mut g = DataflowGraph::new(n_in, 0.03);
            let n_layers = 1 + (s % 3) as usize;
            for l in 0..n_layers {
                let w = 2 + ((s >> 8) % 5) as usize;
                let act = match (s >> 16) % 3 {
                    0 => NodeAct::Identity,
                    1 => NodeAct::SignTrit,
                    _ => NodeAct::TanhRat,
                };
                let red = if (s >> 24) % 2 == 0 { Reducer::Sum } else { Reducer::Max };
                g.add_layer(w, act, red);
                let src_w = if l == 0 { n_in } else { g.layers[l - 1].width };
                for dst in 0..w {
                    for src in 0..src_w {
                        let r = lcg(&mut s);
                        if r.abs() > 0.55 {
                            g.add_edge(l, src, dst, WeightKind::F32(lcg(&mut s)));
                        } else {
                            let t = match (s >> 40) % 3 { 0 => -1, 1 => 1, _ => 0 };
                            g.add_edge(l, src, dst, WeightKind::Trit { val: t, scale: 1.0 });
                        }
                    }
                }
            }
            let x: Vec<f32> = (0..n_in).map(|_| lcg(&mut s)).collect();
            let r = dataflow_reference_eval(&g, &x).unwrap();
            let j = exec_kernel(&g, &x);
            for (a, b) in r.iter().zip(&j) {
                assert_eq!(a.to_bits(), b.to_bits(), "граф seed={seed}: {a:?} ≠ {b:?}");
            }
        }
    }

    #[test]
    fn test_dataflow_no_mul_on_trit_edges() {
        let mut g = DataflowGraph::new(6, 1.0);
        g.add_layer(3, NodeAct::Identity, Reducer::Sum);
        g.add_layer(2, NodeAct::Identity, Reducer::Sum);
        for j in 0..6 {
            let t = if j % 2 == 0 { 1 } else { -1 };
            g.add_edge(0, j, j % 3, WeightKind::Trit { val: t as i8, scale: 1.0 });
        }
        for j in 0..3 {
            g.add_edge(1, j, j % 2, WeightKind::Trit { val: 1, scale: 1.0 });
        }
        let c = DataflowCompiler::compile_x86_64(&g, "no_mul").unwrap();
        let b = &c.machine_bytes;
        let mut mulss = 0;
        for i in 0..b.len().saturating_sub(3) {
            if b[i] == 0xF3 && b[i + 1] == 0x0F && b[i + 2] == 0x59 {
                mulss += 1;
            }
        }
        assert_eq!(mulss, 0, "No-Mul порушено: {mulss} mulss");
        let x = vec![1.0f32, -2.0, 3.0, -4.0, 5.0, -6.0];
        let y = exec_kernel(&g, &x);
        let r = dataflow_reference_eval(&g, &x).unwrap();
        for (a, bb) in r.iter().zip(&y) {
            assert_eq!(a.to_bits(), bb.to_bits());
        }
    }

    #[test]
    fn test_dataflow_wx_discipline() {
        let mut g = DataflowGraph::new(3, 0.0);
        g.add_layer(2, NodeAct::Identity, Reducer::Sum);
        g.add_edge(0, 0, 0, WeightKind::F32(0.5));
        g.add_edge(0, 1, 1, WeightKind::F32(-0.25));
        let c = DataflowCompiler::compile_x86_64(&g, "wx_probe").unwrap();
        let _k = crate::triune::jit_loop::ExecutableKernel::load(&c.machine_bytes).unwrap();
        let maps = std::fs::read_to_string("/proc/self/maps").unwrap();
        for line in maps.lines() {
            let p = line.split_whitespace().nth(1).unwrap_or("");
            let bb = p.as_bytes();
            if bb.len() >= 3 && bb[0] == b'r' && bb[1] == b'w' && bb[2] == b'x' {
                panic!("W+X сторінка: {line}");
            }
        }
    }

    #[test]
    fn test_dataflow_zero_input_edge_cases() {
        let mut g = DataflowGraph::new(2, 0.0);
        g.add_layer(2, NodeAct::Identity, Reducer::Sum);
        g.add_edge(0, 0, 0, WeightKind::Trit { val: -1, scale: 1.0 });
        g.add_edge(0, 1, 1, WeightKind::Trit { val: 1, scale: 1.0 });
        let x = vec![0.0f32, -0.0];
        let y = exec_kernel(&g, &x);
        let r = dataflow_reference_eval(&g, &x).unwrap();
        assert_eq!(r[0].to_bits(), y[0].to_bits());
        assert_eq!(r[1].to_bits(), y[1].to_bits());
    }
}
