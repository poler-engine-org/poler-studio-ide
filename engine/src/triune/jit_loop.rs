//! Замкнутий JIT-контур: навчані ваги → машинний код x86_64 → виконання →
//! пластичність → перекомпіляція.
//!
//! Об'єднує два попередні кроки в одну петлю (відповідь на директиву
//! «веса обученые в машинный код превратить»):
//!
//! ```text
//! [ .t5q — стиснені тритові ваги (mmap O_RDWR) ]
//!        │  зчитування блоку: трит × масштаб → f32
//!        ▼
//! [1] WeightsInCode (через GraphMachineCompiler):
//!        кожен вага вшивається в машинний код x86_64 як immediate
//!        (mov eax, <біти f32> → movd → mulss) — ваги СТАЮТЬ кодом;
//!        трити ±1 — No-Mul (addss/subss), вакуум елімінується.
//!        │
//!        ▼
//! [2] ExecutableKernel: anon-mmap → копія коду → mprotect(R|X) →
//!        transmute у fn(*const f32, *mut f32) — ПРЯМЕ виконання CPU.
//!        │  y = W·x (активациї)
//!        ▼
//! [3] PlasticityCompiler::hebbian_impulses(x, y) + apply():
//!        градієнтно-вільна переписка тритів прямо в стисненому .t5q;
//!        commit() → sha256 + msync.
//!        │
//!        ▼
//! [4] JitLoop::recompile(): нові ваги → новий машинний код → крок [2].
//! ```
//!
//! ## Гарантії
//!
//! - **Біт-в-біт детермінізм**: кодген сумує в порядку зростання j
//!   (movss→mulss→addss), тому результат виконання машинного коду
//!   тотожний наївному циклу `acc += x[j] * w[i][j]` аж до бітів IEEE-754.
//! - Код виконується з окремої R|X сторінки (W^X: запис → потім лише читання
//!   і виконання); на x86_64 кеш інструкцій когерентний — flush не потрібен.
//! - Порожній шар → нульовий вихід (xorps + movss), як у reference.
//!
//! ## Обмеження (чесно)
//!
//! - Скалярний SSE-кодген без AVX/FMA: ~16 байт машинного коду на вагу.
//!   Для демо- і середніх шарів; SIMD-розгортка — наступний крок.
//! - Один шар = один kernel; багатошаровий конвеєр збирається з циклів
//!   по `JitLoop` на кожен блок ваг.

use std::fs::OpenOptions;

use memmap2::{MmapMut, MmapOptions};

use crate::graph::graph_asm::{CompiledMachineGraph, GraphMachineCompiler, ModelComputeGraph};
use crate::triune::compiler::{PlasticityCompiler, PlasticityConfig, T5qMmapView};

/// Тип виконуваної функції: System V AMD64, `fn(x: *const f32, y: *mut f32)`.
/// v0.93.0 (fix macOS/aarch64): ABI sysv64 існує лише на x86_64; на інших
/// архітектурах — еквівалентний "C" (JIT-кодген все одно x86_64-only).
#[cfg(target_arch = "x86_64")]
pub type MatvecFn = unsafe extern "sysv64" fn(*const f32, *mut f32);
#[cfg(not(target_arch = "x86_64"))]
pub type MatvecFn = unsafe extern "C" fn(*const f32, *mut f32);

/// Розмір сторінки пам'яті (для вирівнювання RWX-регіону).
fn page_size() -> usize {
    #[cfg(unix)]
    unsafe {
        let v = libc::sysconf(libc::_SC_PAGESIZE);
        if v <= 0 {
            4096
        } else {
            v as usize
        }
    }
    #[cfg(not(unix))]
    {
        4096
    }
}

// ---------------------------------------------------------------------------
// 1. Виконавець машинного коду (RWX-сторінка + transmute)
// ---------------------------------------------------------------------------

/// Виконуваний машинний код: anon-mmap → копія → `mprotect(PROT_READ|PROT_EXEC)`.
///
/// Небезпечний контракт: код мусить дотримуватись ABI
/// `extern "C" fn(*const f32, *mut f32)` (rdi = x, rsi = y),
/// не чіпати callee-saved регістри і завершуватись `ret`.
#[derive(Debug)]
pub struct ExecutableKernel {
    mem: MmapMut,
    code_len: usize,
}

impl ExecutableKernel {
    /// Завантажити машинні байти на виконувану сторінку.
    pub fn load(code: &[u8]) -> Result<Self, String> {
        Self::load_with_tail(code, 0)
    }

    /// Код + хвостовий блоб даних (WeightsInCode: таблиці вшиті в
    /// виконувану сторінку після `ret`); хвіст — читабельні дані.
    pub fn load_with_tail(code: &[u8], code_len: usize) -> Result<Self, String> {
        if code.is_empty() {
            return Err("пустий машинный код".into());
        }
        let eff = if code_len == 0 { code.len() } else { code_len };
        if eff > code.len() {
            return Err(format!("code_len {eff} поза межами {}", code.len()));
        }
        if code[eff - 1] != 0xC3 {
            return Err("машинный код не заканчивается ret (0xC3)".into());
        }
        let page = page_size();
        let len = code.len().div_ceil(page) * page;
        let mut mem = MmapOptions::new().len(len).map_anon()
            .map_err(|e| format!("anon mmap {len} Б: {e}"))?;
        mem[..code.len()].copy_from_slice(code);
        // W^X: запис зроблено — залишаємо лише читання + виконання.
        #[cfg(unix)]
        {
            let rc = unsafe {
                libc::mprotect(
                    mem.as_ptr() as *mut libc::c_void,
                    len,
                    libc::PROT_READ | libc::PROT_EXEC,
                )
            };
            if rc != 0 {
                return Err(format!("mprotect RX: {}", std::io::Error::last_os_error()));
            }
        }
        #[cfg(windows)]
        {
            extern "system" {
                fn VirtualProtect(addr: *const core::ffi::c_void, size: usize, new_prot: u32, old_prot: *mut u32) -> i32;
            }
            let mut old = 0u32;
            let ok = unsafe {
                VirtualProtect(
                    mem.as_ptr() as *const core::ffi::c_void,
                    len,
                    0x20, // PAGE_EXECUTE_READ
                    &mut old,
                )
            };
            if ok == 0 {
                return Err(format!("VirtualProtect RX: {}", std::io::Error::last_os_error()));
            }
        }
        Ok(Self { mem, code_len: eff })
    }

    /// Довжина машинного коду в байтах (без паддінгу сторінки).
    pub fn code_len(&self) -> usize {
        self.code_len
    }

    /// Повна довжина виділеної сторінки.
    pub fn page_len(&self) -> usize {
        self.mem.len()
    }

    /// Прямий виклик машинного коду (небезпечно: довжини не перевіряються).
    ///
    /// ## Safety
    /// Виклик має право читати `cols` f32 з `x` і писати `rows` f32 у `y`
    /// (залежить від того, як згенеровано код).
    pub unsafe fn call_raw(&self, x: *const f32, y: *mut f32) {
        let f: MatvecFn = std::mem::transmute(self.mem.as_ptr());
        f(x, y);
    }

    /// Прямий виклик для f64-ядер (хімія: A3): ABI `fn(*const f64, *mut f64)`.
    ///
    /// ## Safety
    /// Викликаний код має дотримуватись ABI і не зберігати callee-saved
    /// регістри поза власною рамкою.
    pub unsafe fn call_raw_f64(&self, x: *const f64, y: *mut f64) {
        #[cfg(target_arch = "x86_64")]
        let f: unsafe extern "sysv64" fn(*const f64, *mut f64) = std::mem::transmute(self.mem.as_ptr());
        #[cfg(not(target_arch = "x86_64"))]
        let f: unsafe extern "C" fn(*const f64, *mut f64) = std::mem::transmute(self.mem.as_ptr());
        f(x, y);
    }

    /// Матвекторний виклик з перевіркою довжин: `y[rows] = W·x[cols]`.
    pub fn execute_matvec(
        &self,
        x: &[f32],
        y: &mut [f32],
        rows: usize,
        cols: usize,
    ) -> Result<(), String> {
        if x.len() != cols {
            return Err(format!("x: {} значений ≠ cols {cols}", x.len()));
        }
        if y.len() != rows {
            return Err(format!("y: {} значений ≠ rows {rows}", y.len()));
        }
        unsafe { self.call_raw(x.as_ptr(), y.as_mut_ptr()) };
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2. Ваги в машинному коді (dense matvec через графовий компілятор)
// ---------------------------------------------------------------------------

/// Результат компіляції матриці ваг у машинний код x86_64.
///
/// Кожен вага стає literal-ом у потоці інструкцій
/// (`mov eax, <біти f32>`), слой виконується без звернень до масиву ваг.
#[derive(Debug)]
pub struct WeightsInCode {
    compiled: CompiledMachineGraph,
    rows: usize,
    cols: usize,
}

impl WeightsInCode {
    /// Скомпілювати dense-матрицю `w[rows×cols]` (row-major) у машинний код.
    ///
    /// Порядок сумування — за зростанням j у кожному рядку, тому
    /// виконання біт-в-біт збігається з наївним циклом.
    pub fn compile_matvec(rows: usize, cols: usize, w: &[f32]) -> Result<Self, String> {
        if rows == 0 || cols == 0 {
            return Err(format!("вырожденный слой {rows}×{cols}"));
        }
        if w.len() != rows * cols {
            return Err(format!("веса: {} значений ≠ {rows}×{cols}", w.len()));
        }
        let mut g = ModelComputeGraph::new(cols, rows, 0.0);
        for i in 0..rows {
            for j in 0..cols {
                g.add_dense_weight(j, i, w[i * cols + j]);
            }
        }
        let compiled = GraphMachineCompiler::compile_x86_64(&g, "weights_in_code_matvec");
        Ok(Self { compiled, rows, cols })
    }

    /// Машинні байти (без виконання).
    pub fn machine_bytes(&self) -> &[u8] {
        &self.compiled.machine_bytes
    }

    /// Асемблерний лістинг (для інспекції/демо).
    pub fn asm_listing(&self) -> &str {
        &self.compiled.asm_listing
    }

    /// Кількість апаратних інструкцій.
    pub fn instruction_count(&self) -> usize {
        self.compiled.instruction_count
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Щільність коду: байт машинного коду на одну вагу.
    pub fn bytes_per_weight(&self) -> f32 {
        if self.rows * self.cols == 0 {
            0.0
        } else {
            self.compiled.machine_bytes.len() as f32 / (self.rows * self.cols) as f32
        }
    }

    /// Завантажити на виконувану сторінку.
    pub fn load(&self) -> Result<ExecutableKernel, String> {
        ExecutableKernel::load(&self.compiled.machine_bytes)
    }
}

/// Еталонний інтерпретатор графа (дзеркало семантики кодгену):
/// F32-ребро → `x[src] * w`, Trit ±1 → No-Mul `±x[src]`, 0 → пропуск.
///
/// Порядок накопичення — порядок ребер (у `WeightsInCode` — зростання j),
/// тому збігається з машинним кодом біт-в-біт.
pub fn graph_reference_eval(graph: &ModelComputeGraph, x: &[f32]) -> Result<Vec<f32>, String> {
    if x.len() != graph.num_inputs {
        return Err(format!("x: {} ≠ входов {}", x.len(), graph.num_inputs));
    }
    let mut y = vec![0.0f32; graph.num_outputs];
    for e in &graph.edges {
        if e.dst_node >= graph.num_outputs || e.src_node >= graph.num_inputs {
            continue;
        }
        let v = match e.weight {
            crate::graph::graph_asm::WeightKind::F32(w) => x[e.src_node] * w,
            crate::graph::graph_asm::WeightKind::Trit { val, scale: _ } => {
                x[e.src_node] * (val as f32)
            }
        };
        y[e.dst_node] += v;
    }
    Ok(y)
}

// ---------------------------------------------------------------------------
// 3. Замкнутий контур: машинний код ⇄ пластичність .t5q
// ---------------------------------------------------------------------------

/// Звіт одного циклу контуру: forward → Hebb → in-place → recompile → forward.
#[derive(Debug, Clone)]
pub struct CycleReport {
    /// Вихід ДО мутації (старий машинний код).
    pub y_before: Vec<f32>,
    /// Вихід ПІСЛЯ мутації (новий машинний код, той самий вхід).
    pub y_after: Vec<f32>,
    /// Знайдено кореляцій (імпульсів пластичності).
    pub impulses: usize,
    /// Реально переписаних тритів у .t5q.
    pub flips: u64,
    /// Вакуум (нульові трити) до/після commit.
    pub zeros_before: u64,
    pub zeros_after: u64,
    /// Розмір нового машинного коду, байт.
    pub code_bytes: usize,
    /// Час commit (sha256 + msync), мс.
    pub commit_ms: u128,
}

impl CycleReport {
    /// Норма зміни виходу — сигнал навчання одного циклу.
    pub fn y_delta_norm(&self) -> f32 {
        self.y_before
            .iter()
            .zip(&self.y_after)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f32>()
            .sqrt()
    }
}

/// Замкнутий контур «ваги → машинний код → виконання → пластичність».
///
/// Один `JitLoop` відповідає одному слою `rows×cols`, що живе
/// в .t5q починаючи з блоку `block_of` (як у `hebbian_impulses`).
pub struct JitLoop {
    plasticity: PlasticityCompiler,
    block_of: u64,
    rows: usize,
    cols: usize,
    kernel: ExecutableKernel,
    asm: String,
    inst: usize,
}

impl JitLoop {
    /// Відкрити контур: зчитати ваги блоку з .t5q, скомпілювати машинний код.
    pub fn open(
        path: &std::path::Path,
        cfg: PlasticityConfig,
        block_of: u64,
        rows: usize,
        cols: usize,
    ) -> Result<Self, String> {
        if rows == 0 || cols == 0 {
            return Err(format!("вырожденный слой {rows}×{cols}"));
        }
        let plasticity = PlasticityCompiler::open(path, cfg)?;
        let bsz = plasticity.view().block_size() as u64;
        let needed = (rows as u64 * cols as u64).div_ceil(bsz);
        if block_of + needed > plasticity.view().block_count() {
            return Err(format!(
                "слой {rows}×{cols} требует {needed} блоков от {block_of}, в файле {}",
                plasticity.view().block_count()
            ));
        }
        let w = snapshot_weights(plasticity.view(), block_of, rows, cols)?;
        let wie = WeightsInCode::compile_matvec(rows, cols, &w)?;
        let kernel = wie.load()?;
        Ok(Self {
            asm: wie.asm_listing().to_string(),
            inst: wie.instruction_count(),
            plasticity,
            block_of,
            rows,
            cols,
            kernel,
        })
    }

    /// Поточні ваги шару (деквант: трит × масштаб блока).
    pub fn weights(&self) -> Result<Vec<f32>, String> {
        snapshot_weights(self.plasticity.view(), self.block_of, self.rows, self.cols)
    }

    /// Виконати forward машинним кодом: `y[rows] = W·x[cols]`.
    pub fn execute(&self, x: &[f32], y: &mut [f32]) -> Result<(), String> {
        self.kernel.execute_matvec(x, y, self.rows, self.cols)
    }

    /// Перекомпілювати машинний код з ПОТОЧНИХ ваг .t5q (після мутацій).
    /// Повертає розмір нового коду в байтах.
    pub fn recompile(&mut self) -> Result<usize, String> {
        let w = self.weights()?;
        let wie = WeightsInCode::compile_matvec(self.rows, self.cols, &w)?;
        self.kernel = wie.load()?;
        self.asm = wie.asm_listing().to_string();
        self.inst = wie.instruction_count();
        Ok(self.kernel.code_len())
    }

    /// Повний цикл контуру:
    /// 1. forward машинним кодом (старі ваги);
    /// 2. хеббовські імпульси з пари (x, y);
    /// 3. in-place переписка тритів + commit (sha256, msync);
    /// 4. перекомпіляція машинного коду з мутованих ваг;
    /// 5. forward новим кодом — наступний крок вже «навчений».
    pub fn cycle(&mut self, x: &[f32]) -> Result<CycleReport, String> {
        let mut y_before = vec![0.0f32; self.rows];
        self.execute(x, &mut y_before)?;

        let impulses =
            self.plasticity
                .hebbian_impulses(self.block_of, self.rows, self.cols, x, &y_before)?;
        let flips = self.plasticity.apply(&impulses)?;
        let stats = self.plasticity.commit()?;

        let code_bytes = self.recompile()?;

        let mut y_after = vec![0.0f32; self.rows];
        self.execute(x, &mut y_after)?;

        Ok(CycleReport {
            y_before,
            y_after,
            impulses: impulses.len(),
            flips,
            zeros_before: stats.zeros_before,
            zeros_after: stats.zeros_after,
            code_bytes,
            commit_ms: stats.ms,
        })
    }

    /// Компілятор пластичності (для ручних мутацій/фазового ротора).
    pub fn plasticity_mut(&mut self) -> &mut PlasticityCompiler {
        &mut self.plasticity
    }

    /// Тільки-читання вид на .t5q.
    pub fn view(&self) -> &T5qMmapView {
        self.plasticity.view()
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn block_of(&self) -> u64 {
        self.block_of
    }

    /// Розмір активного машинного коду, байт.
    pub fn code_len(&self) -> usize {
        self.kernel.code_len()
    }

    /// Кількість апаратних інструкцій поточного коду.
    pub fn instruction_count(&self) -> usize {
        self.inst
    }

    /// Останній асемблерний лістинг.
    pub fn asm_listing(&self) -> &str {
        &self.asm
    }
}

/// Деквант шару з .t5q: плоский індекс → (блок, індекс), вага = трит × масштаб.
fn snapshot_weights(
    view: &T5qMmapView,
    block_of: u64,
    rows: usize,
    cols: usize,
) -> Result<Vec<f32>, String> {
    let bsz = view.block_size() as u64;
    let mut w = Vec::with_capacity(rows * cols);
    for f in 0..(rows as u64 * cols as u64) {
        w.push(view.weight(block_of + f / bsz, (f % bsz) as usize)?);
    }
    Ok(w)
}

/// Відкрити .t5q на читання+запис без компілятора пластичності
/// (допоміжна функція для зовнішніх інструментів).
#[allow(dead_code)]
pub fn open_rw_hint(path: &std::path::Path) -> Result<T5qMmapView, String> {
    let _file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("открыть {}: {e}", path.display()))?;
    T5qMmapView::open_rw(path)
}

// ---------------------------------------------------------------------------
// Тести
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::graph_asm::WeightKind;
    use crate::triune::stream_quant::{stream_quantize, StreamQuantConfig};
    use std::io::Write;

    /// Детермінований LCG → f32 в (-1, 1).
    fn lcg(s: &mut u64) -> f32 {
        *s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((*s >> 33) as i32 as f64 / i32::MAX as f64) as f32
    }

    /// Наївний матвектор-еталон (порядок j — зростання, як у кодгені).
    fn naive_matvec(rows: usize, cols: usize, w: &[f32], x: &[f32]) -> Vec<f32> {
        let mut y = vec![0.0f32; rows];
        for (i, yi) in y.iter_mut().enumerate() {
            let mut acc = 0.0f32;
            for j in 0..cols {
                acc += x[j] * w[i * cols + j];
            }
            *yi = acc;
        }
        y
    }

    fn tmp_t5q(values: usize, seed: u64, tag: &str) -> std::path::PathBuf {
        let mut s = seed;
        let mut data = Vec::with_capacity(values * 4);
        for _ in 0..values {
            data.extend_from_slice(&lcg(&mut s).to_le_bytes());
        }
        let mut out = Vec::new();
        stream_quantize(&data[..], &mut out, &StreamQuantConfig::default()).unwrap();
        let dir = std::env::temp_dir().join(format!("t5q_jit_loop_{seed}_{tag}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("model.t5q");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&out).unwrap();
        drop(f);
        path
    }

    // ---- 1. Ваги в коді: біт-в-біт збіг з еталоном -----------------------

    #[test]
    fn dense_matvec_bit_exact_small() {
        // Нестандартні розміри: 7×13, 1×1.
        for &(rows, cols) in &[(7usize, 13usize), (1, 1), (3, 5)] {
            let mut s = 42u64;
            let w: Vec<f32> = (0..rows * cols).map(|_| lcg(&mut s)).collect();
            let x: Vec<f32> = (0..cols).map(|_| lcg(&mut s)).collect();
            let wie = WeightsInCode::compile_matvec(rows, cols, &w).unwrap();
            let kern = wie.load().unwrap();
            let mut y = vec![0.0f32; rows];
            kern.execute_matvec(&x, &mut y, rows, cols).unwrap();
            let r = naive_matvec(rows, cols, &w, &x);
            for i in 0..rows {
                assert_eq!(y[i].to_bits(), r[i].to_bits(), "слой {rows}×{cols}, рядок {i}");
            }
        }
    }

    #[test]
    fn dense_matvec_bit_exact_32x32() {
        let (rows, cols) = (32usize, 32usize);
        let mut s = 4242u64;
        let w: Vec<f32> = (0..rows * cols).map(|_| lcg(&mut s)).collect();
        let x: Vec<f32> = (0..cols).map(|_| lcg(&mut s)).collect();
        let wie = WeightsInCode::compile_matvec(rows, cols, &w).unwrap();
        let kern = wie.load().unwrap();
        let mut y = vec![0.0f32; rows];
        kern.execute_matvec(&x, &mut y, rows, cols).unwrap();
        let r = naive_matvec(rows, cols, &w, &x);
        for i in 0..rows {
            assert_eq!(y[i].to_bits(), r[i].to_bits(), "рядок {i}");
        }
        // Ваги дійсно вшиті в код: 32×32=1024 ваги × 4 біти… байти immediate.
        assert!(wie.machine_bytes().len() > rows * cols * 4);
        assert!(wie.bytes_per_weight() > 4.0);
    }

    // ---- 2. Виконавець: контракт довжин -----------------------------------

    #[test]
    fn execute_len_mismatch_rejected() {
        let wie = WeightsInCode::compile_matvec(2, 3, &[0.1; 6]).unwrap();
        let kern = wie.load().unwrap();
        let x = vec![0.5f32; 3];
        let mut y = vec![0.0f32; 2];
        assert!(kern.execute_matvec(&x, &mut y, 2, 3).is_ok());
        let x_bad = vec![0.5f32; 4];
        assert!(kern.execute_matvec(&x_bad, &mut y, 2, 3).is_err());
        let mut y_bad = vec![0.0f32; 3];
        assert!(kern.execute_matvec(&x, &mut y_bad, 2, 3).is_err());
    }

    #[test]
    fn executor_rejects_bad_code() {
        assert!(ExecutableKernel::load(&[]).is_err());
        assert!(ExecutableKernel::load(&[0x90]).is_err()); // без ret
    }

    // ---- 3. Графовий шлях: F32 тепер ДІЙСНО множить -----------------------

    #[test]
    fn graph_f32_executes_with_weights() {
        let mut g = ModelComputeGraph::new(3, 2, 0.0);
        g.add_dense_weight(0, 0, 0.5);
        g.add_dense_weight(1, 0, -1.25);
        g.add_dense_weight(2, 0, 2.0);
        g.add_dense_weight(0, 1, 0.125);
        g.add_dense_weight(2, 1, -0.75);
        let c = GraphMachineCompiler::compile_x86_64(&g, "f32_mul_test");
        let kern = ExecutableKernel::load(&c.machine_bytes).unwrap();
        let x = [1.5f32, -0.8, 0.4];
        let mut y = [0.0f32; 2];
        kern.execute_matvec(&x, &mut y, 2, 3).unwrap();
        let r = graph_reference_eval(&g, &x).unwrap();
        assert_eq!(y[0].to_bits(), r[0].to_bits());
        assert_eq!(y[1].to_bits(), r[1].to_bits());
        // Контрольне значення руками: 1.5·0.5 + (−0.8)·(−1.25) + 0.4·2 = 2.55.
        assert!((y[0] - 2.55f32).abs() < 1e-6, "y[0] = {}", y[0]);
    }

    #[test]
    fn graph_trit_no_mul_executes() {
        let mut g = ModelComputeGraph::new(4, 2, 0.0);
        g.add_trit_weight(0, 0, 1, 1.0);
        g.add_trit_weight(1, 0, -1, 1.0);
        g.add_trit_weight(2, 0, 0, 1.0); // вакуум
        g.add_trit_weight(3, 0, 1, 1.0);
        g.add_trit_weight(1, 1, -1, 1.0);
        g.add_trit_weight(2, 1, 1, 1.0);
        let c = GraphMachineCompiler::compile_x86_64(&g, "trit_no_mul_test");
        let kern = ExecutableKernel::load(&c.machine_bytes).unwrap();
        let x = [0.7f32, -0.3, 0.55, 1.2];
        let mut y = [0.0f32; 2];
        kern.execute_matvec(&x, &mut y, 2, 4).unwrap();
        // y0 = +x0 − x1 + x3; y1 = −x1 + x2 (трит 0 — пропущений).
        let y0 = x[0] - x[1] + x[3];
        let y1 = -x[1] + x[2];
        assert_eq!(y[0].to_bits(), y0.to_bits());
        assert_eq!(y[1].to_bits(), y1.to_bits());
    }

    #[test]
    fn graph_vacuum_node_zero_output() {
        let mut g = ModelComputeGraph::new(2, 3, 0.0);
        g.add_dense_weight(0, 0, 0.9);
        // Виходи 1 і 2 — вакуумні (без ребер).
        let c = GraphMachineCompiler::compile_x86_64(&g, "vacuum_test");
        let kern = ExecutableKernel::load(&c.machine_bytes).unwrap();
        let x = [2.0f32, -1.0];
        let mut y = [1.111f32; 3];
        kern.execute_matvec(&x, &mut y, 3, 2).unwrap();
        assert_eq!(y[0].to_bits(), (x[0] * 0.9f32).to_bits());
        assert_eq!(y[1].to_bits(), 0.0f32.to_bits());
        assert_eq!(y[2].to_bits(), 0.0f32.to_bits());
    }

    // ---- 4. Замкнутий контур з .t5q ---------------------------------------

    #[test]
    fn jit_loop_closed_cycle() {
        let path = tmp_t5q(4096, 777, "cycle");
        let mut lp = JitLoop::open(&path, PlasticityConfig::default(), 0, 32, 32).unwrap();

        // До циклу: машинний код біт-в-біт збігається з деквантом .t5q.
        let x: Vec<f32> = (0..32)
            .map(|i| if i % 2 == 0 { 0.9f32 } else { -0.7f32 })
            .collect();
        let mut y = vec![0.0f32; 32];
        lp.execute(&x, &mut y).unwrap();
        let w0 = lp.weights().unwrap();
        let r0 = naive_matvec(32, 32, &w0, &x);
        for i in 0..32 {
            assert_eq!(y[i].to_bits(), r0[i].to_bits(), "до цикла, рядок {i}");
        }

        // Цикл: Hebb → in-place → commit → перекомпіляція → новий forward.
        let rep = lp.cycle(&x).unwrap();
        assert!(
            rep.impulses > 0,
            "хебб должен найти корреляции при сильных активациях"
        );
        assert!(rep.flips > 0, "хотя бы один трит должен быть переписан");

        // Новий машинний код біт-в-біт збігається з новими вагами.
        let w1 = lp.weights().unwrap();
        assert_ne!(w0, w1, "вага шару должна измениться");
        for i in 0..32 {
            let mut acc = 0.0f32;
            for j in 0..32 {
                acc += x[j] * w1[i * 32 + j];
            }
            assert_eq!(rep.y_after[i].to_bits(), acc.to_bits(), "после цикла, рядок {i}");
        }
        // y_before — це старий код, теж еталонний.
        for i in 0..32 {
            assert_eq!(rep.y_before[i].to_bits(), r0[i].to_bits());
        }
        // Навчальний сигнал ненульовий і цілісність файлу збережена.
        assert!(rep.y_delta_norm() > 0.0);
        assert!(rep.code_bytes > 0);
        lp.view().verify().unwrap();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn jit_loop_recompile_reflects_manual_mutation() {
        let path = tmp_t5q(512, 1234, "manual"); // рівно один блок
        let mut lp = JitLoop::open(&path, PlasticityConfig::default(), 0, 4, 4).unwrap();
        let x = vec![1.0f32; 4];
        let mut y0 = vec![0.0f32; 4];
        lp.execute(&x, &mut y0).unwrap();

        // Ручна in-place мутація: трит 0 блоку 0 → протилежний (вага (0,0) шару).
        {
            let v = lp.plasticity_mut().view_mut();
            let t0 = v.trit(0, 0).unwrap();
            let t1 = if t0 == 1 { -1i8 } else { 1 };
            let changed = v.write_trit(0, 0, t1).unwrap();
            assert!(changed, "трит должен смениться (был {t0}, стал {t1})");
            assert_eq!(v.trit(0, 0).unwrap(), t1);
        }
        // Перекомпіляція підхоплює мутацію без перечитування файлу з диска.
        lp.recompile().unwrap();
        let mut y1 = vec![0.0f32; 4];
        lp.execute(&x, &mut y1).unwrap();
        assert_ne!(
            y0[0].to_bits(),
            y1[0].to_bits(),
            "рядок 0 должен измениться после мутации веса (0,0)"
        );

        // Фіксація: після commit файл знову проходить verify.
        lp.plasticity_mut().commit().unwrap();
        lp.view().verify().unwrap();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn jit_loop_deterministic() {
        // Два ІДЕНТИЧНИХ файли (один seed, різні шляхи) — контури незалежні.
        let p1 = tmp_t5q(2048, 99, "det_a");
        let p2 = tmp_t5q(2048, 99, "det_b");
        let mut a = JitLoop::open(&p1, PlasticityConfig::default(), 0, 16, 16).unwrap();
        let mut b = JitLoop::open(&p2, PlasticityConfig::default(), 0, 16, 16).unwrap();
        let x: Vec<f32> = (0..16).map(|i| 0.8 - 0.1 * i as f32).collect();

        let ra = a.cycle(&x).unwrap();
        let rb = b.cycle(&x).unwrap();
        assert_eq!(ra.impulses, rb.impulses);
        assert_eq!(ra.flips, rb.flips);
        for (ya, yb) in ra.y_after.iter().zip(rb.y_after.iter()) {
            assert_eq!(ya.to_bits(), yb.to_bits());
        }
        let _ = std::fs::remove_file(&p1);
        let _ = std::fs::remove_file(&p2);
    }

    // ---- 5. Дрібні перевірки API ------------------------------------------

    #[test]
    fn weights_in_code_rejects_bad_shapes() {
        assert!(WeightsInCode::compile_matvec(0, 4, &[]).is_err());
        assert!(WeightsInCode::compile_matvec(4, 0, &[]).is_err());
        assert!(WeightsInCode::compile_matvec(2, 3, &[0.1; 5]).is_err());
    }

    #[test]
    fn weight_kind_reference_eval_guard() {
        let mut g = ModelComputeGraph::new(2, 1, 0.0);
        g.add_dense_weight(0, 0, 0.5);
        g.edges.push(crate::graph::graph_asm::ComputeEdge {
            src_node: 0,
            dst_node: 0,
            weight: WeightKind::Trit { val: 1, scale: 1.0 },
        });
        assert!(graph_reference_eval(&g, &[0.5f32]).is_err()); // x != входов
        let y = graph_reference_eval(&g, &[0.5f32, 9.0]).unwrap();
        // F32: 0.5·0.5, Trit+1 No-Mul: +0.5 → сума в порядку ребер.
        assert_eq!(y[0].to_bits(), (0.25f32 + 0.5f32).to_bits());
    }
}
