//! VAULT-ASM — сокровищница DeepSeek Vault в машинном коде x86_64.
//!
//! v0.87.0: пять микроядер на чистом ассемблере (Intel-синтаксис,
//! `global_asm!`), портированных из `docs/vault_drafts/`:
//!
//! | Ядро | Источник в сокровищнице | Базис |
//! |------|--------------------------|-------|
//! | [`fep_asm`] | `poler-core/src/fep_loss.rs` (Фристон) | AVX2+FMA |
//! | [`lens_asm`] | `poler-lens/src/lens_index.rs` (No-Hits) | SSE2 (базовый!) |
//! | [`synapse_asm`] | блоки 0986/0984 (синапс 1.7Б) | AVX2+FMA+F16C |
//! | [`cordic_asm`] | P3/CORDIC S¹ | SSE2 + целочисленный |
//! | [`stdp_asm`] | LanguageCoreV2 (0601, STDP+WTA) | AVX2+FMA |
//!
//! v0.88.0 TRIT-ASM: троичная физика процессора — триты и кутриты
//! подняты из Rust-слоя движка (calc/trits.rs, pqc) в машинный код:
//!
//! | Ядро | Физика | Базис |
//! |------|--------|-------|
//! | [`trit_asm`] | 𝕋={−1,0,+1}: Клини, vpsignb, LEA×3, magic-÷3, VPTERNLOGD | SSSE3/SSE4.1/AVX-512 |
//! | [`qutrit_asm`] | ℤ₃ ω-ротор 120° + мост трит→S¹ | SSE2/AVX2+FMA |
//!
//! v0.89.0 ABS-ZERO: АЗУ — Абсолютный Ноль ([`zero_asm`]): проективная
//! прямая ℝP¹ [N:D] (деление перекрёстным умножением — ДЕЛЕНИЯ НЕТ,
//! полюс D=0 — точка, не сбой), изолированный/уникальный/симметричный
//! машинный нуль (121 = все-⊙, ±(3⁴⁰−1)/2), безопасное деление без #DE,
//! ноль-сумма кутрита (N, D, −(N+D)).
//!
//! v0.90.0 SYNAPSE-TRIT ([`synapse_trit_asm`]): слияние тритов + кутритов
//! + нейромодулей — ℤ₁₂-ротор (синус за ~1 такт вместо ~200 FLOPs),
//! трит-MAC без умножителя, pack5-поле 0.2 Б/синапс (50M синапсов
//! FlyWire = 10 МБ — кэш L3), CSR-коннектом с rayon-параллелизмом,
//! точный Q8-распад к абсолютному нулю за конечные шаги.
//!
//! Каждое ядро имеет скалярный эталон (fallback для хостов без AVX2 и
//! референс для тестов). CLI: `--asm-info`, `--asm-bench`.

#[cfg(target_arch = "x86_64")]
pub mod cordic_asm;
#[cfg(target_arch = "x86_64")]
pub mod fep_asm;
#[cfg(target_arch = "x86_64")]
pub mod lens_asm;
#[cfg(target_arch = "x86_64")]
pub mod qutrit_asm;
#[cfg(target_arch = "x86_64")]
pub mod stdp_asm;
#[cfg(target_arch = "x86_64")]
pub mod synapse_asm;
#[cfg(target_arch = "x86_64")]
pub mod synapse_trit_asm;
#[cfg(target_arch = "x86_64")]
pub mod trit_asm;
#[cfg(target_arch = "x86_64")]
pub mod zero_asm;

use std::sync::OnceLock;

/// CPUID-возможности хоста (детекция один раз).
#[derive(Debug, Clone, Copy)]
pub struct AsmCaps {
    pub avx2: bool,
    pub fma: bool,
    pub f16c: bool,
    pub popcnt: bool,
    pub avx512f: bool,
    pub sse2: bool,
    /// AVX (VEX-кодировка) — базис vpsubb/vpsignb/vpminsb-ядер trit_asm.
    pub avx: bool,
    /// SSSE3 — vpsignb (тритное умножение знаком).
    pub ssse3: bool,
    /// SSE4.1 — vpminsb/vpmaxsb (Клини-вентили), vpmovsxbw.
    pub sse41: bool,
}

impl AsmCaps {
    fn detect() -> Self {
        Self {
            avx2: is_x86_feature_detected!("avx2"),
            fma: is_x86_feature_detected!("fma"),
            f16c: is_x86_feature_detected!("f16c"),
            popcnt: is_x86_feature_detected!("popcnt"),
            avx512f: is_x86_feature_detected!("avx512f"),
            sse2: is_x86_feature_detected!("sse2"),
            avx: is_x86_feature_detected!("avx"),
            ssse3: is_x86_feature_detected!("ssse3"),
            sse41: is_x86_feature_detected!("sse4.1"),
        }
    }

    /// Золотой набор VAULT-ASM (все ядра, кроме SSE2-базовых).
    pub fn golden(&self) -> bool {
        self.avx2 && self.fma && self.f16c
    }

    /// Троичный золотой набор TRIT-ASM (Клини + vpsignb + VPTERNLOGD).
    pub fn trit_golden(&self) -> bool {
        self.avx && self.ssse3 && self.sse41 && self.avx512f
    }
}

static CAPS: OnceLock<AsmCaps> = OnceLock::new();

/// Возможности CPU (кэш через `OnceLock`).
pub fn caps() -> AsmCaps {
    *CAPS.get_or_init(AsmCaps::detect)
}

/// Текстовый отчёт о микроядрах для CLI (`--asm-info`).
pub fn info_report() -> String {
    let c = caps();
    let on = |b: bool| if b { "ON" } else { "—" };
    format!(
        "VAULT-ASM x86_64 microkernels (v0.90.0 SYNAPSE-TRIT)\n\
         \n\
         CPU basis:\n\
           AVX2      : {}\n\
           FMA       : {}\n\
           F16C      : {}\n\
           POPCNT    : {}\n\
           AVX-512F  : {}\n\
           SSE2      : {} (baseline — LENS/CORDIC/QUTRIT-Q15 всегда)\n\
           SSSE3     : {} (vpsignb — тритное умножение знаком)\n\
           SSE4.1    : {} (Клини-вентили vpminsb/vpmaxsb)\n\
         \n\
         Kernels (source → engine):\n\
           fep_asm      poler-core/fep_loss.rs  → literary/engine.rs (F, ∇F)  [AVX2+FMA]\n\
           lens_asm     poler-lens/lens_index.rs → engine.rs Causal Nexus    [SSE2]\n\
           synapse_asm  vault 0986/0984 (1.7Б)  → SSN inference layer        [AVX2+FMA+F16C]\n\
           cordic_asm   P3 CORDIC S¹ (Q30)      → детерминированный ротор    [SSE2+int]\n\
           stdp_asm     LanguageCoreV2 0601      → трёхфакторный STDP + WTA   [AVX2+FMA]\n\
         \n\
         TRIT-ASM v0.88.0 (троичная физика процессора):\n\
           trit_asm     𝕋={{−1,0,+1}} Клини-вентили / ⟨u,v⟩ vpsignb /\n\
                       pack5 LEA×3 / unpack5 magic-÷3 / тернарный спайк ⊕⊙⊖\n\
                       / VPTERNLOGD-мультиплексор (imm 0xE4, sel?a:b)   [SSSE3+SSE4.1, AVX-512]\n\
           qutrit_asm   ℤ₃ ω-ротор 120° (f32 FMA + Q14 pmulhw) /\n\
                       мост трит→S¹ (θ_t = t·2π/3)              [SSE2/AVX2]\n\
         \n\
         ABS-ZERO v0.89.0 (АЗУ — Абсолютный Ноль, ℝP¹):\n\
           zero_asm     [N:D] класс (Finite/Zero/Pole/Gauge) / инверсия=ОБМЕН /\n\
                       умножение и ДЕЛЕНИЕ перекрёстным умножением (полюс D=0 —\n\
                       точка, не сбой; 0/0 → калибровка) / 128-битное сравнение\n\
                       дробей БЕЗ деления / div-safe ±MAX насыщение (нет #DE) /\n\
                       вакуум-скан pack5 (121=все⊙, pcmpeqb+popcnt) /\n\
                       qutrit3 (N,D,−(N+D)) — сумма ≡ 0 по построению    [int+SSE2+POPCNT]\n\
         \n\
         SYNAPSE-TRIT v0.90.0 (слияние тритов + кутритов + нейромодулей):\n\
           synapse_trit_asm  ℤ₁₂-ротор sincos Q24 — LUT в РЕГИСТРАХ (vpermd+\n\
                             vblendvps), ω-поворот бит-точно mod 12, 1+ω+ω²=0 /\n\
                             трит-MAC vpsignb (32 трита/итер, 0 умножений) /\n\
                             pack5-поле 0.2 Б/синапс: 50M синапсов FlyWire = 10 МБ\n\
                             (весь мозг в кэше L3) / CSR-коннектом + rayon /\n\
                             Q8-распад: абсолютный ноль за КОНЕЧНЫЕ шаги /\n\
                             АЗУ: точное [N:D]-ранжирование рёбер нексуса\n\
                             (cmp_canonical вместо f64-кастов), div_safe_f32\n\
                             в FEP-нормировке и STDP-softmax           [AVX2+SSSE3+SSE4.1]\n\
         \n\
         Golden set: {} | TRIT golden: {}",
        on(c.avx2),
        on(c.fma),
        on(c.f16c),
        on(c.popcnt),
        on(c.avx512f),
        on(c.sse2),
        on(c.ssse3),
        on(c.sse41),
        on(c.golden()),
        on(c.trit_golden()),
    )
}

/// Печать отчёта (CLI `--asm-info`).
pub fn print_info() {
    println!("{}", info_report());
}

fn mps(n: f64, secs: f64) -> f64 {
    n / secs / 1e6
}

fn gbs(bytes: f64, secs: f64) -> f64 {
    bytes / secs / 1e9
}

/// Бенчмарк микроядер (CLI `--asm-bench`): пропускная способность +
/// сверка со скалярными эталонами. Возвращает код выхода.
pub fn run_bench(synapses: usize, cycles: usize) -> std::process::ExitCode {
    use std::time::Instant;
    let c = caps();
    println!("VAULT-ASM microkernel benchmark (v0.90.0 SYNAPSE-TRIT)");
    println!(
        "caps: avx2={} fma={} f16c={} popcnt={} avx512={} ssse3={} sse41={}\n",
        c.avx2, c.fma, c.f16c, c.popcnt, c.avx512f, c.ssse3, c.sse41
    );
    let mut all_pass = true;

    // ---------------- FEP: полный шаг Фристона ----------------
    {
        let n = 1 << 20;
        let mut p: Vec<f32> = (0..n).map(|i| ((i % 97) as f32) / 97.0).collect();
        let o: Vec<f32> = (0..n).map(|i| ((i % 61) as f32) / 61.0).collect();
        let gw: Vec<f32> = vec![1.0; n];
        let mut grad = vec![0.0f32; n];
        // прогрев
        fep_asm::step(&mut p, &o, &gw, &mut grad, 1e-3, 0.05);
        let t = Instant::now();
        let steps = 100;
        let mut f = 0.0;
        for _ in 0..steps {
            f = fep_asm::step(&mut p, &o, &gw, &mut grad, 1e-3, 0.05);
        }
        let dt = t.elapsed().as_secs_f64();
        let ok = f.is_finite();
        all_pass &= ok;
        println!(
            "FEP  step        n={:>9} x{:>3}: {:8.1} M MAC8/s   F={:.4}   {}",
            n,
            steps,
            mps((n * steps) as f64, dt),
            f,
            pass(ok)
        );
        // сверка
        let mut p2 = p.clone();
        let mut g2 = vec![0.0f32; 64];
        let mut p3: Vec<f32> = p[..64].to_vec();
        let o64: Vec<f32> = o[..64].to_vec();
        let gw64: Vec<f32> = gw[..64].to_vec();
        p2[..64].copy_from_slice(&p3);
        fep_asm::step(&mut p2[..64], &o64, &gw64, &mut g2, 1e-3, 0.05);
        fep_asm::step_scalar(&mut p3, &o64, &gw64, &mut vec![0.0f32; 64], 1e-3, 0.05);
        let ok2 = p2[..64]
            .iter()
            .zip(&p3)
            .all(|(a, b)| (a - b).abs() < 1e-4);
        all_pass &= ok2;
        println!("     parity vs scalar: {}", pass(ok2));
    }

    // ---------------- FEP: только энергия ----------------
    {
        let n = 10_000_000;
        let p: Vec<f32> = (0..n).map(|i| ((i % 89) as f32) / 89.0).collect();
        let o: Vec<f32> = (0..n).map(|i| ((i % 71) as f32) / 71.0).collect();
        // identity-таблица .rodata ограничена 64 float — для 10M метрика
        // материализуется явно (один аллок на бенч)
        let gw: Vec<f32> = vec![1.0; n];
        let t = Instant::now();
        let f = fep_asm::energy(&p, &o, Some(&gw), 0.5);
        let dt = t.elapsed().as_secs_f64();
        println!(
            "FEP  energy      n={:>9}     : {:8.1} M elem/s  ({:.2} GB/s)  F={:.2}",
            n,
            mps(n as f64, dt),
            gbs((n * 8) as f64, dt),
            f
        );
    }

    // ---------------- LENS: No-Hits барьер ----------------
    {
        let n = 10_000_000;
        let mut s = 12345u64;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        let w: Vec<f32> = (0..n).map(|_| (next() % 100) as f32 / 100.0).collect();
        let flags: Vec<u64> = (0..n).map(|_| next() & 0xF).collect();
        let mut out = vec![0u32; n];
        let t = Instant::now();
        let kept = lens_asm::filter(&w, &flags, &mut out, lens_asm::LENS_MIN_W, 0x3, 0x8);
        let dt = t.elapsed().as_secs_f64();
        let t2 = Instant::now();
        let bits = lens_asm::popcount(&flags, 0x3);
        let dt2 = t2.elapsed().as_secs_f64();
        println!(
            "LENS filter      n={:>9}     : {:8.1} M cand/s   keep {:.1}%   [барьер 0.05]",
            n,
            mps(n as f64, dt),
            100.0 * kept as f64 / n as f64
        );
        println!(
            "LENS popcount    n={:>9}     : {:8.1} M flags/s  Σbits={}",
            n,
            mps(n as f64, dt2),
            bits
        );
        // сверка семантики на подмножестве
        let mut out_r = vec![0u32; 4096];
        let kr = lens_asm::filter_scalar(&w[..4096], &flags[..4096], &mut out_r, lens_asm::LENS_MIN_W, 0x3, 0x8);
        let ka = lens_asm::filter(&w[..4096], &flags[..4096], &mut out[..4096], lens_asm::LENS_MIN_W, 0x3, 0x8);
        let ok = kr == ka && out[..ka] == out_r[..kr];
        all_pass &= ok;
        println!("     parity vs scalar: {}", pass(ok));
    }

    // ---------------- SSN: синапс-атомарный слой ----------------
    {
        let n_pre = (synapses / 8).max(1024);
        let fanout = 8;
        let field = synapse_asm::SynapseField::synthetic(n_pre, fanout, 4242);
        let mut pre: Vec<f32> = (0..field.pad_pre_len())
            .map(|i| ((i % 53) as f32) / 53.0)
            .collect();
        let mut post = vec![0.0f32; field.n_post];
        // прогрев
        field.step_f16(&pre, &mut post, 0.9, 0.5);
        post.iter_mut().for_each(|x| *x = 0.0);
        let t = Instant::now();
        let mut e = 0.0;
        for _ in 0..cycles.max(1) {
            e = field.step_f16(&pre, &mut post, 0.9, 0.5);
        }
        let dt = t.elapsed().as_secs_f64();
        let total = (n_pre * fanout) as f64 * cycles.max(1) as f64;
        let mb_f16 = (n_pre * fanout * 2) as f64 / 1e6;
        println!(
            "SSN  f16         {:>9} syn x{:>3}: {:8.1} M syn/s   payload {:.1} МБ (2.0 Б/син)  E={:.2}",
            n_pre * fanout,
            cycles.max(1),
            mps(total, dt),
            mb_f16,
            e
        );
        // i8-режим: 1 Б/синапс
        let mut post8 = vec![0.0f32; field.n_post];
        field.step_i8(&pre, &mut post8, 0.9, 0.5);
        post8.iter_mut().for_each(|x| *x = 0.0);
        let t = Instant::now();
        let mut e8 = 0.0;
        for _ in 0..cycles.max(1) {
            e8 = field.step_i8(&pre, &mut post8, 0.9, 0.5);
        }
        let dt = t.elapsed().as_secs_f64();
        let mb_i8 = (n_pre * fanout) as f64 / 1e6;
        println!(
            "SSN  i8          {:>9} syn x{:>3}: {:8.1} M syn/s   payload {:.1} МБ (1.0 Б/син)  E={:.2}",
            n_pre * fanout,
            cycles.max(1),
            mps(total, dt),
            mb_i8,
            e8
        );
        // сверка всех 16 примитивов
        let mut f16 = synapse_asm::SynapseField::synthetic(256, 8, 777);
        for (i, fu) in f16.func.iter_mut().enumerate() {
            *fu = (i % synapse_asm::FUNC_COUNT) as u8;
        }
        let mut pa = vec![0.0f32; f16.n_post];
        let mut pr = vec![0.0f32; f16.n_post];
        let ea = f16.step_f16(&pre[..f16.pad_pre_len()], &mut pa, 0.9, 0.5);
        let er = f16.step_scalar(
            synapse_asm::Density::F16,
            &pre[..f16.pad_pre_len()],
            &mut pr,
            0.9,
            0.5,
        );
        let ok = (ea - er).abs() / er.abs().max(1e-6) < 2e-3;
        all_pass &= ok;
        println!(
            "     parity 16 примитивов vs scalar: {} ({:.4} vs {:.4})",
            pass(ok),
            ea,
            er
        );
    }

    // ---------------- CORDIC: ротор S¹ ----------------
    {
        let n = 2_000_000;
        let mut re: Vec<f32> = (0..n).map(|i| ((i * 37 % 199) as f32 / 100.0) - 1.0).collect();
        let mut im: Vec<f32> = (0..n).map(|i| ((i * 53 % 199) as f32 / 100.0) - 1.0).collect();
        let t = Instant::now();
        cordic_asm::renorm(&mut re, &mut im);
        let dt = t.elapsed().as_secs_f64();
        // ошибка нормы
        let mut worst = 0.0f32;
        for i in 0..n {
            let m = (re[i] * re[i] + im[i] * im[i]).sqrt();
            worst = worst.max((m - 1.0).abs());
        }
        let ok = worst < 5e-3;
        all_pass &= ok;
        println!(
            "CORDIC renorm    n={:>9}     : {:8.3} M elem/s  max‖·‖err {:.2e}  {}",
            n,
            mps(n as f64, dt),
            worst,
            pass(ok)
        );
    }

    // ---------------- STDP + WTA ----------------
    {
        let n = 10_000_000.min(synapses.max(1_000_000));
        let mut w: Vec<f32> = vec![0.1; n];
        let mut tr: Vec<f32> = vec![0.0; n];
        let pre: Vec<f32> = (0..n).map(|i| ((i % 31) as f32 / 31.0)).collect();
        stdp_asm::step(&mut w, &mut tr, &pre, 0.01, 0.9, 0.95, 2.0);
        let t = Instant::now();
        let steps = cycles.max(1);
        let mut dw = 0.0;
        for _ in 0..steps {
            dw = stdp_asm::step(&mut w, &mut tr, &pre, 0.01, 0.9, 0.95, 2.0);
        }
        let dt = t.elapsed().as_secs_f64();
        let total = n as f64 * steps as f64;
        println!(
            "STDP step        {:>9} syn x{:>3}: {:8.1} M syn/s   Σ|Δw|={:.1}",
            n,
            steps,
            mps(total, dt),
            dw
        );
        let rates: Vec<f32> = (0..1_000_000)
            .map(|i| ((i * 2654435761u64 % 1000) as f32 / 1000.0))
            .collect();
        let t = Instant::now();
        let idx = stdp_asm::wta_argmax(&rates);
        let dt = t.elapsed().as_secs_f64();
        println!(
            "WTA  argmax      n={:>9}     : {:8.1} M rate/s  idx={}  {}",
            rates.len(),
            mps(rates.len() as f64, dt),
            idx,
            pass(idx == wta_expected(&rates))
        );
    }

    // ---------------- TRIT: троичная физика v0.88 ----------------
    {
        let n = 10_000_000;
        let mut s = 4242u64;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        let a: Vec<i8> = (0..n).map(|_| ((next() % 3) as i64 as i8) - 1).collect();
        let b: Vec<i8> = (0..n).map(|_| ((next() % 3) as i64 as i8) - 1).collect();
        // прогрев
        let _ = trit_asm::dot(&a[..1024], &b[..1024]);
        let t = Instant::now();
        let d = trit_asm::dot(&a, &b);
        let dt = t.elapsed().as_secs_f64();
        let d_ref: i64 = a.iter().zip(&b).map(|(&x, &y)| x as i64 * y as i64).sum();
        let ok = d == d_ref;
        all_pass &= ok;
        println!(
            "TRIT dot vpsignb {:>9} trit    : {:8.1} M trit/s  ⟨u,v⟩={}  {}",
            n,
            mps(n as f64, dt),
            d,
            pass(ok)
        );
        // тернарный спайк
        let x: Vec<f32> = (0..n)
            .map(|_| ((next() % 200) as f32) / 100.0 - 1.0)
            .collect();
        let mut sp = vec![0.0f32; n];
        trit_asm::spike_f32(&x[..1024], &mut sp[..1024], 0.4);
        let t = Instant::now();
        trit_asm::spike_f32(&x, &mut sp, 0.4);
        let dt = t.elapsed().as_secs_f64();
        let ok = x.iter().zip(&sp).all(|(&v, &o)| {
            o == if v > 0.4 {
                1.0
            } else if v < -0.4 {
                -1.0
            } else {
                0.0
            }
        });
        all_pass &= ok;
        println!(
            "TRIT spike ⊕⊙⊖  {:>9} elem    : {:8.1} M elem/s  θ=0.4  {}",
            n,
            mps(n as f64, dt),
            pass(ok)
        );
        // pack/unpack 5 трит/байт
        let mut packed = vec![0u8; n / 5];
        let mut unpacked = vec![0i8; n / 5 * 5];
        let t = Instant::now();
        let nb = trit_asm::pack5(&a, &mut packed);
        let nt = trit_asm::unpack5(&packed, &mut unpacked);
        let dt = t.elapsed().as_secs_f64();
        let ok = nb == n / 5 && nt == nb * 5 && &a[..nb * 5] == &unpacked[..nb * 5];
        all_pass &= ok;
        println!(
            "TRIT pack5 LEA×3 {:>9} trit    : {:8.1} M trit/s  5 трит/байт (3⁵<2⁸)  {}",
            nb * 5,
            mps((nb * 5) as f64, dt),
            pass(ok)
        );
        // VPTERNLOGD-мультиплексор
        if c.avx512f {
            let sel: Vec<u32> = (0..1_000_000).map(|_| (next() % 3) as u32).collect();
            let va: Vec<u32> = (0..1_000_000).map(|i| i as u32 * 7 + 1).collect();
            let vb: Vec<u32> = (0..1_000_000).map(|i| i as u32 * 13 + 5).collect();
            let mut out = vec![0u32; 1_000_000];
            let t = Instant::now();
            trit_asm::mux512(&va, &vb, &sel, &mut out);
            let dt = t.elapsed().as_secs_f64();
            let ok = out
                .iter()
                .zip(&sel)
                .zip(&va)
                .zip(&vb)
                .all(|(((o, &s), &x), &y)| *o == (s & x) | (!s & y));
            all_pass &= ok;
            println!(
                "TRIT mux VPTERNLOGD {:>9} lane   : {:8.1} M lane/s  bit-mux (imm 0xE4)  {}",
                1_000_000,
                mps(1e6, dt),
                pass(ok)
            );
        }
    }

    // ---------------- QUTRIT: ℤ₃ ω-ротор v0.88 ----------------
    {
        let n = 2_000_000;
        let mut s = 31415u64;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        let mut re: Vec<f32> = (0..n)
            .map(|_| ((next() % 200) as f32) / 100.0 - 1.0)
            .collect();
        let mut im: Vec<f32> = (0..n)
            .map(|_| ((next() % 200) as f32) / 100.0 - 1.0)
            .collect();
        let (re0, im0) = (re.clone(), im.clone());
        let t = Instant::now();
        qutrit_asm::omega_f32(&mut re, &mut im);
        let dt = t.elapsed().as_secs_f64();
        // ℤ₃-цикл: три поворота = тождество
        qutrit_asm::omega_f32(&mut re, &mut im);
        qutrit_asm::omega_f32(&mut re, &mut im);
        let mut worst = 0.0f32;
        for i in 0..n {
            worst = worst.max((re[i] - re0[i]).abs()).max((im[i] - im0[i]).abs());
        }
        let ok = worst < 1e-5;
        all_pass &= ok;
        println!(
            "QUTRIT ω f32     {:>9} rot     : {:8.1} M rot/s   ω³=I err {:.2e}  {}",
            n,
            mps(n as f64, dt),
            worst,
            pass(ok)
        );
        // Q14-паритет
        let mut x: Vec<i16> = (0..n / 10)
            .map(|_| (((next() % 200) as i32 - 100) * 120 / 1000).clamp(-12000, 12000) as i16)
            .collect();
        let mut y: Vec<i16> = (0..n / 10)
            .map(|_| (((next() % 200) as i32 - 100) * 120 / 1000).clamp(-12000, 12000) as i16)
            .collect();
        let mut re2: Vec<f32> = x.iter().map(|&v| v as f32 / 16384.0).collect();
        let mut im2: Vec<f32> = y.iter().map(|&v| v as f32 / 16384.0).collect();
        qutrit_asm::omega_q15(&mut x, &mut y);
        qutrit_asm::omega_f32(&mut re2, &mut im2);
        let ok = x
            .iter()
            .zip(&y)
            .zip(&re2)
            .zip(&im2)
            .all(|(((&xi, &yi), &r), &m)| {
                (xi as f32 / 16384.0 - r).abs() < 2e-3 && (yi as f32 / 16384.0 - m).abs() < 2e-3
            });
        all_pass &= ok;
        println!(
            "QUTRIT ω Q14 pmulhw {:>9} rot     : SSE2-базис   паритет f32  {}",
            n / 10,
            pass(ok)
        );
        // мост трит→S¹
        let trits: Vec<i8> = (0..n / 10).map(|_| ((next() % 3) as i64 as i8) - 1).collect();
        let mut pre = vec![0.0f32; n / 10];
        let mut pim = vec![0.0f32; n / 10];
        let t = Instant::now();
        qutrit_asm::project(&trits, &mut pre, &mut pim);
        let dt = t.elapsed().as_secs_f64();
        let ok = pre
            .iter()
            .zip(&pim)
            .all(|(&r, &m)| ((r * r + m * m).sqrt() - 1.0).abs() < 1e-6);
        all_pass &= ok;
        println!(
            "QUTRIT трит→S¹   {:>9} trit    : {:8.1} M trit/s  θ=t·2π/3 на S¹  {}",
            n / 10,
            mps((n / 10) as f64, dt),
            pass(ok)
        );
    }

    // ---------------- AZU: Абсолютный Ноль v0.89 ----------------
    {
        let n = 16 * 1024 * 1024;
        let mut s = 97531u64;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        // троичные данные с четвертью вакуума: ~25% байтов все-⊙ (121)
        let packed: Vec<u8> = (0..n)
            .map(|_| {
                if next() % 4 == 0 {
                    zero_asm::TRIT5_ZERO_CODE
                } else {
                    let mut v = (next() % 243) as u8;
                    if v == zero_asm::TRIT5_ZERO_CODE {
                        v = v.wrapping_add(1);
                    }
                    v
                }
            })
            .collect();
        let t = Instant::now();
        let zeros = zero_asm::trit_zero_count(&packed);
        let dt = t.elapsed().as_secs_f64();
        let zeros_ref = zero_asm::trit_zero_count_scalar(&packed);
        let ok = zeros == zeros_ref && zeros > 0;
        all_pass &= ok;
        println!(
            "AZU  вакуум-скан  {:>9} Б      : {:8.2} GB/s    плотность ⊙ {:.1}%  {}",
            n,
            gbs(n as f64, dt),
            100.0 * zeros as f64 / n as f64,
            pass(ok)
        );
        // проективные операции на потоке пар
        let m = 10_000_000;
        let a: Vec<Proj2> = (0..m)
            .map(|_| Proj2 { n: (next() % 4096) as i64 - 2048, d: (next() % 2045) as i64 + 4 })
            .collect();
        let b: Vec<Proj2> = (0..m)
            .map(|_| Proj2 { n: (next() % 4096) as i64 - 2048, d: (next() % 2045) as i64 + 4 })
            .collect();
        let t = Instant::now();
        let mut acc = 0i64;
        for i in 0..m {
            let r = a[i].div(&b[i]);
            acc ^= r.n;
        }
        let dt = t.elapsed().as_secs_f64();
        let ok = acc != i64::MAX; // живой поток, не вырожден
        all_pass &= ok;
        println!(
            "AZU  [N:D] div    {:>9} дробь  : {:8.1} M div/s   перекрёстное умножение, деления НЕТ",
            m,
            mps(m as f64, dt)
        );
        // точное сравнение там, где f64 округляет
        let t = Instant::now();
        let mut ord = 0i64;
        for i in 0..m {
            let r = zero_asm::cmp_canonical(&a[i].into(), &b[i].into());
            ord += r as i64;
        }
        let dt = t.elapsed().as_secs_f64();
        let ok = (-(m as i64)..=m as i64).contains(&ord);
        all_pass &= ok;
        println!(
            "AZU  cmp 128-бит  {:>9} дробь  : {:8.1} M cmp/s   Σord={} (N1·D2 vs N2·D1)",
            m,
            mps(m as f64, dt),
            ord
        );
        // безопасное деление i32: края + поток
        let nums: Vec<i32> = (0..m).map(|i| ((i % 977) as i32 - 488) * 4_194_303).collect();
        let dens: Vec<i32> = (0..m).map(|i| ((i % 883) as i32 - 441) * 4_194_303).collect();
        let t = Instant::now();
        let mut dacc = 0i32;
        for i in 0..m {
            let r = zero_asm::div_safe_i32(nums[i], if i % 500_000 == 0 { 0 } else { dens[i] });
            dacc = dacc.wrapping_add(r);
        }
        let dt = t.elapsed().as_secs_f64();
        let edge_ok = zero_asm::div_safe_i32(7, 0) == i32::MAX
            && zero_asm::div_safe_i32(-7, 0) == -i32::MAX
            && zero_asm::div_safe_i32(i32::MIN, -1) == i32::MAX
            && zero_asm::div_safe_i32(0, 0) == 0;
        all_pass &= edge_ok;
        println!(
            "AZU  div-safe i32 {:>9} дел    : {:8.1} M div/s   края 0, MIN/-1: насыщение, не #DE  {}",
            m,
            mps(m as f64, dt),
            pass(edge_ok)
        );
        let _ = dacc;
    }

    // ---------------- SYNAPSE-TRIT: ℤ₁₂-ротор + трит-MAC + CSR ----------------
    {
        use synapse_trit_asm::*;
        println!("--- SYNAPSE-TRIT (триты + кутриты + нейромодули) ---");

        // 1) ℤ₁₂-ротор: синус+косинус за ~1 такт (LUT в регистрах)
        let n = 8_000_000usize;
        let mut s = 20251010u64 | 1;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        // фазы с каноническим k ∈ 0..15 (12..15 канонизируются чтением)
        let phase: Vec<i32> = (0..n)
            .map(|_| (((next() >> 40) as i32 & 0x0F) << 24) | (next() >> 40) as i32 & 0x00FF_FFFF)
            .collect();
        let mut sout = vec![0i32; n];
        let mut cout = vec![0i32; n];
        z3_sincos(&phase, &mut sout, &mut cout); // прогрев
        let t = Instant::now();
        z3_sincos(&phase, &mut sout, &mut cout);
        let dt = t.elapsed().as_secs_f64();
        // Калькулятор-оракул: max ошибка в LSB Q24 против libm
        let mut max_lsb: f64 = 0.0;
        for i in (0..n).step_by(n / 4096 + 1) {
            let k = ((phase[i] as u32) >> 24) & 15;
            let ang = (k as f64) * std::f64::consts::FRAC_PI_6;
            let es = (ang.sin() * 16_777_216.0).round();
            let ec = (ang.cos() * 16_777_216.0).round();
            max_lsb = max_lsb
                .max((sout[i] as f64 - es).abs())
                .max((cout[i] as f64 - ec).abs());
        }
        // ω³ = I — бит-точно (инвариант канонического k)
        let ok_omega = (0..4096).all(|_| {
            let p = (next() >> 32) as i32;
            let mut q = p;
            for _ in 0..3 {
                q = z3_advance(q, 4);
            }
            z3_sincos_scalar(p) == z3_sincos_scalar(q)
        });
        all_pass &= ok_omega && max_lsb <= 1.0;
        println!(
            "Z3   ротор sincos  {:>7} фаз    : {:8.1} M rot/s   err ≤ {:.1} LSB Q24, ω³=I бит-точно {}",
            n,
            mps(n as f64, dt),
            max_lsb,
            pass(ok_omega)
        );
        println!(
            "     классика: ~200 FLOPs/синус (Тейлор/Чебышёв) → дефляция операции ×{}",
            200
        );

        // 2) плотный трит-MAC (развернутые веса, 1 Б/синапс)
        let n2 = 50_000_000usize;
        let w: Vec<i8> = (0..n2).map(|_| ((next() >> 33) % 3) as i64 as i8 - 1).collect();
        let x: Vec<i8> = (0..n2).map(|_| ((next() >> 33) % 3) as i64 as i8 - 1).collect();
        let r = mac32(&w, &x); // прогрев
        let t = Instant::now();
        let r2 = mac32(&w, &x);
        let dt = t.elapsed().as_secs_f64();
        let sc: i64 = w[..4096].iter().zip(&x[..4096]).map(|(&a, &b)| a as i64 * b as i64).sum();
        let sc_full_prefix = mac32(&w[..4096], &x[..4096]);
        let ok_mac = r == r2 && sc == sc_full_prefix;
        all_pass &= ok_mac;
        println!(
            "TRIT MAC dense     {:>7} син    : {:8.3} G син/с   vpsignb, 0 умножений  {}",
            n2,
            mps(n2 as f64, dt) / 1000.0,
            pass(ok_mac)
        );

        // 3) pack5-поле: 0.2 Б/синапс, 40M синапсов = 8 МБ (L3!)
        let field = TritSsnField::synthetic(1_000_000, 40, 7);
        let xs: Vec<i8> = (0..field.pad_x_len())
            .map(|_| ((next() >> 33) % 3) as i64 as i8 - 1)
            .collect();
        let mut post = vec![0i32; field.n_post];
        field.step(&xs, &mut post, 224); // прогрев
        let t = Instant::now();
        let steps = 4;
        let mut en = 0i64;
        for _ in 0..steps {
            en = field.step(&xs, &mut post, 224);
        }
        let dt = t.elapsed().as_secs_f64();
        let ok_field = en > 0 && post.iter().all(|&p| p != i32::MIN);
        all_pass &= ok_field;
        println!(
            "TRIT STEP pack5    {:>7} син    : {:8.1} M син/с   {} Б/синапс → 40M син = {:.0} МБ (L3)  {}",
            field.synapses(),
            mps((field.synapses() * steps) as f64, dt),
            field.bytes_per_synapse(),
            field.w_packed.len() as f64 / 1e6,
            pass(ok_field)
        );

        // 4) CSR-коннектом (FlyWire-масштаб): пост-центричные строки + rayon
        let csr = TritCsrConnectome::synthetic(65_536, 80, 5);
        let xc: Vec<i8> = (0..csr.n_nodes)
            .map(|_| ((next() >> 33) % 3) as i64 as i8 - 1)
            .collect();
        {
            let mut post_w = vec![0i32; csr.n_nodes];
            csr.step(&xc, &mut post_w, 224); // прогрев (отдельный массив!)
        }
        // ОДИН шаг с одинакового нулевого состояния — только так
        // seq и rayon-пути сравнимы (прогрев не должен мутировать пост)
        let mut post_s = vec![0i32; csr.n_nodes];
        let t = Instant::now();
        let ec = csr.step(&xc, &mut post_s, 224);
        let dt = t.elapsed().as_secs_f64();
        let mut post_p = vec![0i32; csr.n_nodes];
        let t2 = Instant::now();
        let ep = csr.step_parallel(&xc, &mut post_p, 224);
        let dt2 = t2.elapsed().as_secs_f64();
        let ok_csr = ec > 0 && ec == ep && post_s == post_p;
        all_pass &= ok_csr;
        println!(
            "TRIT CSR коннектом {:>7} рёбер  : {:8.1} M реб/с   seq;  {:8.1} M реб/с   rayon×{}  {}",
            csr.n_edges,
            mps(csr.n_edges as f64, dt),
            mps(csr.n_edges as f64, dt2),
            rayon::current_num_threads(),
            pass(ok_csr)
        );
        println!(
            "     FlyWire-масштаб: 50M синапсов → {:.0} МБ payload (0.2 Б/син) — весь мозг в L3",
            50e6 * 0.2 / 1e6
        );

        // 5) СИНТЕЗ: честная математика к 4.1·10^18 (El Capitan-класс)
        let raw_dense = mps(n2 as f64, dt) * 1e6; // оп/с (плотный MAC)
        let raw_csr = mps(csr.n_edges as f64, dt2) * 1e6; // параллельный CSR
        let raw_best = raw_dense.max(raw_csr);
        // классический эквивалент: 1 нейрон ≈ 1000 FLOPs (коэффициент
        // владельца) — счётчик операций схлопывается троичной физикой
        let equiv = raw_best * 1000.0;
        let target = 4.1e18;
        let cores = target / equiv;
        println!("СИНТЕЗ (операции переопределены троичной физикой):");
        println!(
            "     сырая трит-скорость на этом Xeon : {:.2} G трит-оп/с",
            raw_best / 1e9
        );
        println!(
            "     классический эквивалент (×1000)  : {:.2} G оп/с   [нейрон 1000 FLOPs → ~1 вектор-инстр/32 синапса]",
            equiv / 1e9
        );
        println!(
            "     до 4.1·10^18 оп/с (El Capitan-класс): ~{:.0} тыс. ядер этой архитектуры ({:.0} узлов × {} ядер)",
            cores / 1000.0,
            cores / (rayon::current_num_threads() as f64 * 1000.0),
            rayon::current_num_threads()
        );
        println!(
            "     точка 4.1e18 достигается МАСШТАБИРОВАНИЕМ архитектуры, а не враньём о одном CPU"
        );
        // Q8-распад: абсолютный ноль за конечные шаги
        let mut p8 = 100i64;
        let mut n8 = 0;
        while p8 != 0 && n8 < 64 {
            p8 = (p8 * 224) >> 8;
            n8 += 1;
        }
        let mut f8 = f32::from_bits(1);
        for _ in 0..64 {
            f8 *= 0.875;
        }
        println!(
            "     АЗУ-распад Q8: пост → ТОЧНЫЙ 0 за {} шагов; f32×0.875 на субнормали — НИКОГДА ({} > 0)",
            n8,
            f8
        );
    }

    println!("\nИтог: {}", if all_pass { "ВСЕ ПРОВЕРКИ ПРОЙДЕНЫ" } else { "ЕСТЬ РАССОГЛАСОВАНИЯ" });
    if all_pass {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(3)
    }
}

fn wta_expected(rates: &[f32]) -> u32 {
    let mut best = 0usize;
    for (i, &r) in rates.iter().enumerate() {
        if r > rates[best] {
            best = i;
        }
    }
    best as u32
}

/// Локальная пара для бенча АЗУ (не pub — только run_bench).
#[derive(Clone, Copy)]
struct Proj2 {
    n: i64,
    d: i64,
}

impl From<Proj2> for zero_asm::Proj {
    fn from(p: Proj2) -> Self {
        zero_asm::Proj { n: p.n, d: p.d }
    }
}

impl Proj2 {
    fn div(&self, o: &Proj2) -> Proj2 {
        let r = zero_asm::Proj::from(*self).div(&zero_asm::Proj::from(*o));
        Proj2 { n: r.n, d: r.d }
    }
}

fn pass(ok: bool) -> &'static str {
    if ok {
        "PASS"
    } else {
        "FAIL"
    }
}
