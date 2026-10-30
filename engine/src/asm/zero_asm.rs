//! ZERO-ASM — АЗУ: Абсолютный Ноль (v0.89.0).
//!
//! «Нам нужен ноль для одного — для вычислений»: представление нуля и
//! точность в контексте цифровой архитектуры и машинной арифметики
//! (фиксированная точка, знаменатели, предотвращение деления на ноль).
//!
//! ## Вердикт верификации — Калькулятором Всего движка (ноль Python)
//!
//! ```text
//! poler-engine --exec 'calc 2^-30'                  → 9.313225746154785e-10
//! poler-engine --exec 'calc 2^-1074'                → 5e-324   (последний субнормаль f64)
//! poler-engine --exec 'calc 2^-1075'                → 0.0      (цепочка ε/2 УМЕРЛА)
//! poler-engine --exec 'calc (3^5-1)/2'              → 121      (код все-⊙ байта pack5)
//! poler-engine --exec 'calc (3^40-1)/2'             → 6078832729528464400 < 2^63
//! poler-engine --exec 'calc 2^63 - (2^63-1)'        → 1        (асимметрия дополнения до двух)
//! poler-engine --exec 'calc 81*log2(3)'             → 128.38   (81 трит ≈ 128 бит)
//! poler-engine --exec 'calc 1 + e^(i*2*pi/3) + e^(i*4*pi/3)'
//!                                                    → −2.2e−16 + 3.3e−16i  (1+ω+ω²=0)
//! poler-engine --exec 'calc 1/0'   → ∞      poler-engine --exec 'calc 1/-0' → -∞
//! ```
//!
//! **Истинный абсолютный ноль машины** — не вещественный предел (там
//! ∀ε>0 ∃ε/2 — это для аэродинамики и физики), а точка решётки, у которой
//! ЧЕТЫРЕ строгих свойства:
//!
//! 1. **Изоляция**: ∃δ>0 (δ = 1 ULP), шар B(0,δ)∩M = {0} — вокруг нуля
//!    ПУСТО. В ℝ это ложь (ноль — точка накопления), в fixed-point и
//!    троичной решётке — истина. Следствие: тест `x == 0` ТОЧЕН, никакого
//!    эпсилон-фаззинга. Цепочка ε, ε/2, ε/4, … в машине КОНЕЧНА и умирает
//!    в ⊙ (в f64 — после 1074 делений, в Q30 — после 30).
//! 2. **Уникальность**: одна кодировка. Троичный нуль — все триты ⊙,
//!    в pack5-байте это ровно 121 = (3⁵−1)/2 и НИЧЕГО больше (тест
//!    `az_packed_zero_is_unique` перебирает все 243 кода). IEEE-754
//!    проваливает: +0.0 и −0.0 — ДВЕ кодировки одного числа, и калькулятор
//!    это показывает: 1/0 → ∞, 1/−0 → −∞ — «один» ноль даёт два полюса.
//! 3. **Центр симметрии**: негация тотальна. 40 тритов: диапазон
//!    ±(3⁴⁰−1)/2 = ±6078832729528464400 — |MIN| = |MAX| ТОЧНО. Двоичное
//!    дополнение до двух асимметрично ровно на 1 (2⁶³ − (2⁶³−1) = 1):
//!    −(−2⁶³) переполняется, `abs(INT_MIN)` знаменитый баг, а
//!    `INT_MIN / −1` поднимает #DE на железе — деление на «ноль-класс»
//!    внутри КАЖДОГО двоичного ALU. АЗУ лечит это насыщением.
//! 4. **Аннигиляция**: R + (−R) = 0 для ВСЕХ R решётки — та «находимость
//!    абсолютного нуля», вокруг которой строился исходный диалог. Это
//!    групповая аксиома ℤ: она верна для каждого целого (не только для
//!    избранного остатка R), троичная решётка дарит её аппаратно, а
//!    двоичная — на 2ⁿ−1 из 2ⁿ точек (ломается ровно в MIN).
//!
//! Проективный слой ℝP¹ [N:D] доводит до конца то, что начал диалог:
//! полюс D=0 — это ТОЧКА (север), а не сбой; [1:0] ≡ [−1:0] (λ-инвариант
//! однородных координат) — +∞ и −∞ замкнуты в одну точку; [0:0] —
//! единственная калибровочная неопределённость, детектируется ОДНИМ
//! тестом; инверсия — ОБМЕН регистров (нуль ↔ полюс зеркальны);
//! деление заменено перекрёстным умножением — ДЕЛЕНИЯ НЕТ ВООБЩЕ.
//! Фазы кутрита живут на той же плоскости: 1+ω+ω²=0 (верифицировано
//! калькулятором), а вложение (N, D, −(N+D)) имеет сумму проекций ≡ 0
//! ПО ПОСТРОЕНИЮ — плоскость A₂, дом тритов {-1,0,+1}.
//!
//! | Ядро | Смысл | Базис |
//! |------|-------|-------|
//! | `poler_az_class` | класс [N:D]: Finite/Zero/Pole/Gauge | целочисленный |
//! | `poler_az_inv` | инверсия = ОБМЕН (xchg-семантика) | целочисленный |
//! | `poler_az_proj_mul` | [N1·N2 : D1·D2] + насыщение в полюс | целочисленный |
//! | `poler_az_proj_div` | a/b ПЕРЕКРЁСТНЫМ умножением [N1·D2 : D1·N2] —
//! |                  | деления НЕТ ВООБЩЕ, b=0 → полюс, не сбой | целочисленный |
//! | `poler_az_cmp` | ТОЧНОЕ сравнение дробей (128-бит, БЕЗ деления) | целочисленный |
//! | `poler_az_div_safe_i32` | деление Q-домена без #DE (±MAX насыщение) | целочисленный |
//! | `poler_az_trit_zero_count` | плотность вакуума: байты все-⊙ (121) | SSE2+POPCNT |
//! | `poler_az_qutrit3` | (N, D, −(N+D)): сумма ≡ 0 по построению | целочисленный |

use std::arch::global_asm;

/// Код все-⊙ байта pack5: (3⁵−1)/2 = 121 — единственная кодировка
/// троичного нуля в 5-тритном байте (Калькулятор Всего).
pub const TRIT5_ZERO_CODE: u8 = 121;

/// Троичный радиус u64: 40 тритов = 8 pack5-байтов.
pub const TRITS_PER_U64: usize = 40;

/// (3⁴⁰−1)/2 = 6078832729528464400 < 2⁶³ — сбалансированный диапазон
/// 40 тритов влезает в i64 БЕЗ переполнения; |MIN| = |MAX| ТОЧНО
/// (Калькулятор Всего; контраст: 2⁶³ − (2⁶³−1) = 1 у дополнения до двух).
pub const TRIT_U64_RADIUS: i64 = 6078832729528464400;

/// Симметричный положительный полюс насыщения i64.
pub const POLE_I64_POS: i64 = i64::MAX;

/// Симметричный отрицательный полюс насыщения i64: −(2⁶³−1) — НЕ MIN,
/// чтобы полюс сам не был точкой-изгоем негации.
pub const POLE_I64_NEG: i64 = -i64::MAX;

/// ULP Q30 — шаг решётки CORDIC-ротора (Калькулятор Всего:
/// 2⁻³⁰ = 9.313225746154785e-10). Радиус изоляции машинного нуля.
pub const Q30_ULP_F64: f64 = 9.313225746154785e-10;

global_asm! {
    r#"
    .text
    .p2align 4
    # 16×код все-⊙ (121 = (3^5-1)/2) для вакуум-скана pack5-байтов
.Laz_z121:
    .byte 121, 121, 121, 121, 121, 121, 121, 121
    .byte 121, 121, 121, 121, 121, 121, 121, 121

    .text
    # ================================================================
    # u64 poler_az_class(i64 n, i64 d)   rdi=n, rsi=d -> rax
    #   Класс точки [N:D] на проективной прямой RP1:
    #   0=Finite  1=Zero([0:D!=0])  2=Pole([N!=0:0])  3=Gauge([0:0])
    #   код = (n==0) + 2*(d==0) — без веток: sete + lea.
    # ================================================================
    .globl poler_az_class
    .p2align 4
poler_az_class:
    xor eax, eax
    test rdi, rdi
    sete al
    xor ecx, ecx
    test rsi, rsi
    sete cl
    lea rax, [rax + rcx*2]
    ret

    # ================================================================
    # void poler_az_inv(i64 n, i64 d, i64* out)   rdi=n, rsi=d, rdx=out
    #   inv([N:D]) = [D:N] — обратное число это ОБМЕН:
    #   нуль [0:1] и полюс [1:0] зеркальны на RP1. Ни деления, ни умножения.
    # ================================================================
    .globl poler_az_inv
    .p2align 4
poler_az_inv:
    mov rax, rsi
    mov [rdx], rax          # N' = D
    mov rax, rdi
    mov [rdx + 8], rax      # D' = N
    ret

    # ================================================================
    # void poler_az_proj_mul(i64 n1, i64 d1, i64 n2, i64 d2, i64* out)
    #   rdi=n1, rsi=d1, rdx=n2, rcx=d2, r8=out (2 слота: N, D)
    #   Умножение дробей: [N1:D1]*[N2:D2] = [N1*N2 : D1*D2].
    #   Переполнение произведения -> насыщение в ПОЛЮС [+-MAX : 0];
    #   знак значения = XOR знаков четырёх аргументов — без умножения.
    # ================================================================
    .globl poler_az_proj_mul
    .p2align 4
poler_az_proj_mul:
    mov r9, rdx             # r9 = n2
    mov rax, rdi
    imul rax, r9            # N = n1*n2 (ДВУХоперандный: rax = rax*r9 —
    jo .Lpm_ovf             # трёхрегистровый imul кодируется как EVEX/APX
    mov rdx, rsi            # и убивает pre-APX Xeon сигалом SIGILL!)
    imul rdx, rcx           # D = d1*d2
    jo .Lpm_ovf
    mov [r8], rax
    mov [r8 + 8], rdx
    ret
.Lpm_ovf:
    # знак значения: старшие биты n1^d1^n2^d2
    mov rax, rdi
    xor rax, rsi
    xor rax, r9
    xor rax, rcx
    mov rdx, 9223372036854775807        # +полюс (mov r64, imm64)
    mov rcx, -9223372036854775807       # -полюс
    test rax, rax
    cmovs rdx, rcx                       # знак < 0 -> -MAX
    mov [r8], rdx
    mov qword ptr [r8 + 8], 0            # знаменатель полюса = 0
    ret

    # ================================================================
    # void poler_az_proj_div(i64 n1, i64 d1, i64 n2, i64 d2, i64* out)
    #   rdi=n1, rsi=d1, rdx=n2, rcx=d2, r8=out (2 слота: N, D)
    #   ДЕЛЕНИЕ БЕЗ ДЕЛЕНИЯ: a/b = [N1*D2 : D1*N2] — перекрёстное
    #   умножение. b=0 -> знаменатель результата 0 -> ПОЛЮС (∞),
    #   а не исключение; 0/0 -> [0:0] — калибровка. Насыщение то же.
    # ================================================================
    .globl poler_az_proj_div
    .p2align 4
poler_az_proj_div:
    mov r9, rdx             # r9 = n2
    mov rax, rdi
    imul rax, rcx            # N = n1*d2 (двухоперандный imul — без APX)
    jo .Lpd_ovf
    mov rdx, rsi
    imul rdx, r9             # D = d1*n2
    jo .Lpd_ovf
    mov [r8], rax
    mov [r8 + 8], rdx
    ret
.Lpd_ovf:
    mov rax, rdi
    xor rax, rsi
    xor rax, r9
    xor rax, rcx
    mov rdx, 9223372036854775807
    mov rcx, -9223372036854775807
    test rax, rax
    cmovs rdx, rcx
    mov [r8], rdx
    mov qword ptr [r8 + 8], 0
    ret

    # ================================================================
    # i64 poler_az_cmp(i64 n1, i64 d1, i64 n2, i64 d2)
    #   rdi=n1, rsi=d1, rdx=n2, rcx=d2 -> rax in -1/0/+1
    #   КОНТРАКТ: d1 > 0 и d2 > 0 (каноническая форма).
    #   N1/D1 ? N2/D2  <=>  N1*D2 ? N2*D1 (128-битные произведения,
    #   однооперандный imul) — сравнение дробей БЕЗ деления и БЕЗ потерь:
    #   там, где f64 округляет 3e18+1 к 3e18 и врёт «равно», здесь точно.
    #   Старшие слова сравниваются ЗНАКОВО, младшие — БЕЗЗНАКОВО.
    # ================================================================
    .globl poler_az_cmp
    .p2align 4
poler_az_cmp:
    mov r8, rdx             # r8 = n2
    mov rax, rdi
    imul rcx                # rdx:rax = n1*d2   (P1)
    mov r10, rax
    mov r11, rdx
    mov rax, r8
    imul rsi                # rdx:rax = n2*d1   (P2)
    cmp r11, rdx            # старшие — знаково
    jl .Lpc_lt
    jg .Lpc_gt
    cmp r10, rax            # младшие — беззнаково
    jb .Lpc_lt
    ja .Lpc_gt
    xor eax, eax
    ret
.Lpc_lt:
    mov rax, -1
    ret
.Lpc_gt:
    mov rax, 1
    ret

    # ================================================================
    # i32 poler_az_div_safe_i32(i32 num, i32 den)   edi=num, esi=den -> eax
    #   Деление fixed-point БЕЗ исключений:
    #   den==0            -> симметричный полюс +MAX/-MAX по знаку num
    #                        (0/0 -> 0: калибровка схлопывается в вакуум)
    #   num==MIN, den==-1 -> +MAX — единственная асимметричная точка
    #                        дополнения до двух (на железе idiv = #DE)
    #   иначе             -> точный idiv
    # ================================================================
    .globl poler_az_div_safe_i32
    .p2align 4
poler_az_div_safe_i32:
    test esi, esi
    jz .Lds_sat
    cmp esi, -1
    jne .Lds_idiv
    cmp edi, 0x80000000
    je .Lds_max
.Lds_idiv:
    mov eax, edi
    cdq
    idiv esi
    ret
.Lds_max:
    mov eax, 0x7FFFFFFF
    ret
.Lds_sat:
    test edi, edi
    jz .Lds_zero
    js .Lds_neg
    mov eax, 0x7FFFFFFF
    ret
.Lds_neg:
    mov eax, 0x80000001     # -(2^31-1) — симметричный минус-полюс
    ret
.Lds_zero:
    xor eax, eax
    ret

    # ================================================================
    # usize poler_az_trit_zero_count(const u8* packed, usize n)
    #   rdi=packed, rsi=n -> rax
    #   «Плотность вакуума»: число байт == 121 (все 5 тритов в состоянии
    #   отдыха ⊙ — ТОЧНЫЙ абсолютный ноль, тест без эпсилон). 16 байт за
    #   проход: pcmpeqb + pmovmskb + popcnt (SSE2 + POPCNT).
    # ================================================================
    .globl poler_az_trit_zero_count
    .p2align 4
poler_az_trit_zero_count:
    xor eax, eax
    xor rcx, rcx
    test rsi, rsi
    jz .Ltz_fin
    lea r9, [rip + .Laz_z121]
    movdqa xmm1, [r9]
.Ltz_lp:
    mov rdx, rsi
    sub rdx, rcx
    cmp rdx, 16
    jb .Ltz_fin
    movdqu xmm0, [rdi + rcx]
    pcmpeqb xmm0, xmm1
    pmovmskb r8d, xmm0
    popcnt r8d, r8d
    add eax, r8d
    add rcx, 16
    jmp .Ltz_lp
.Ltz_fin:
    ret

    # ================================================================
    # void poler_az_qutrit3(i64 n, i64 d, i64* out)
    #   rdi=n, rsi=d, rdx=out (3 слота)
    #   out = [N, D, -(N+D)] — кутритная сумма проекций Тождественно ноль
    #   ПО ПОСТРОЕНИЮ (lea + neg), плоскость A2: дом тритов и фаз 1+ω+ω²=0.
    # ================================================================
    .globl poler_az_qutrit3
    .p2align 4
poler_az_qutrit3:
    lea rax, [rdi + rsi]
    neg rax
    mov [rdx], rdi
    mov [rdx + 8], rsi
    mov [rdx + 16], rax
    ret
    "#
}

extern "sysv64" {
    fn poler_az_class(n: i64, d: i64) -> u64;
    fn poler_az_inv(n: i64, d: i64, out: *mut i64);
    fn poler_az_proj_mul(n1: i64, d1: i64, n2: i64, d2: i64, out: *mut i64);
    fn poler_az_proj_div(n1: i64, d1: i64, n2: i64, d2: i64, out: *mut i64);
    fn poler_az_cmp(n1: i64, d1: i64, n2: i64, d2: i64) -> i64;
    fn poler_az_div_safe_i32(num: i32, den: i32) -> i32;
    fn poler_az_trit_zero_count(packed: *const u8, n: usize) -> usize;
    fn poler_az_qutrit3(n: i64, d: i64, out: *mut i64);
}

/// Класс точки [N:D] на проективной прямой ℝP¹.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjClass {
    /// Обыкновенная дробь N/D, D≠0, N≠0.
    Finite = 0,
    /// [0:D≠0] — южный полюс: истинный абсолютный ноль (изолирован,
    /// уникален, центр симметрии решётки).
    Zero = 1,
    /// [N≠0:0] — северный полюс: ∞. Проективно [1:0]≡[−1:0] — ОДНА точка
    /// (λ-инвариант однородных координат замыкает +∞ и −∞).
    Pole = 2,
    /// [0:0] — калибровочная неопределённость: единственный запрещённый
    /// класс, детектируется ОДНИМ тестом вместо NaN-отравления.
    Gauge = 3,
}

impl ProjClass {
    fn from_code(code: u64) -> Self {
        match code {
            1 => ProjClass::Zero,
            2 => ProjClass::Pole,
            3 => ProjClass::Gauge,
            _ => ProjClass::Finite,
        }
    }
}

/// Проективная дробь [N:D]: значение N/D; D=0 — полюс (∞), не сбой;
/// [0:0] — калибровка. Деления НЕТ: умножения и обмены.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Proj {
    pub n: i64,
    pub d: i64,
}

impl Proj {
    /// Абсолютный ноль [0:1] — южный полюс ℝP¹.
    pub const ZERO: Proj = Proj { n: 0, d: 1 };
    /// Единица [1:1].
    pub const ONE: Proj = Proj { n: 1, d: 1 };
    /// Полюс [1:0] = ∞ (≡ [−1:0] — одна проективная точка).
    pub const INF: Proj = Proj { n: 1, d: 0 };
    /// Калибровочная неопределённость [0:0] — единственный запрещённый класс.
    pub const GAUGE: Proj = Proj { n: 0, d: 0 };

    /// Целое как дробь [v:1].
    pub fn from_int(v: i64) -> Self {
        Proj { n: v, d: 1 }
    }

    /// Класс точки: asm-ядро (sete+lea, без веток).
    pub fn classify(&self) -> ProjClass {
        ProjClass::from_code(unsafe { poler_az_class(self.n, self.d) })
    }

    /// Инверсия = ОБМЕН [D:N]: нуль ↔ полюс зеркальны; 1/x без деления.
    pub fn inv(&self) -> Self {
        let mut out = [0i64; 2];
        unsafe { poler_az_inv(self.n, self.d, out.as_mut_ptr()) };
        Proj { n: out[0], d: out[1] }
    }

    /// Умножение [N1·N2 : D1·D2]; переполнение — насыщение в полюс
    /// со знаком значения (asm-ядро).
    pub fn mul(&self, o: &Proj) -> Proj {
        let mut out = [0i64; 2];
        unsafe { poler_az_proj_mul(self.n, self.d, o.n, o.d, out.as_mut_ptr()) };
        Proj { n: out[0], d: out[1] }
    }

    /// ДЕЛЕНИЕ БЕЗ ДЕЛЕНИЯ: a/b = [N1·D2 : D1·N2] перекрёстным
    /// умножением (asm-ядро). b=0 → полюс (∞), 0/0 → калибровка [0:0],
    /// x/∞ → нуль, ∞/x → полюс — «деление на ноль» перестаёт быть
    /// событием: это координата на ℝP¹, а не исключение.
    pub fn div(&self, o: &Proj) -> Proj {
        let mut out = [0i64; 2];
        unsafe { poler_az_proj_div(self.n, self.d, o.n, o.d, out.as_mut_ptr()) };
        Proj { n: out[0], d: out[1] }
    }

    /// Аддитивная негация [−N:D] — тотальна на троично-симметричной
    /// решётке (контраст: i64::MIN ломает её в дополнении до двух).
    pub fn neg(&self) -> Self {
        Proj { n: self.n.wrapping_neg(), d: self.d }
    }

    /// Сложение [N1·D2+N2·D1 : D1·D2] в i128-промежуточных — ТОЧНО
    /// (1/2+1/3 = [5:6] без всяких 0.833…4). Переполнение i64 — насыщение:
    /// |числитель|>|знаменатель| → полюс знака, иначе нуль.
    pub fn add(&self, o: &Proj) -> Proj {
        let n = self.n as i128 * o.d as i128 + o.n as i128 * self.d as i128;
        let d = self.d as i128 * o.d as i128;
        let fits = |v: i128| v >= i64::MIN as i128 && v <= i64::MAX as i128;
        if fits(n) && fits(d) {
            return Proj { n: n as i64, d: d as i64 };
        }
        // насыщение по величине: полюс знака либо нуль
        let sign = (n < 0) != (d < 0);
        if n.unsigned_abs() > d.unsigned_abs() {
            Proj { n: if sign { POLE_I64_NEG } else { POLE_I64_POS }, d: 0 }
        } else {
            Proj::ZERO
        }
    }

    /// Каноническая форма: НОД-редукция (бинарный gcd) + знаменатель > 0.
    /// [−1:0] → [1:0]: плюс- и минус-полюс — ОДНА проективная точка.
    pub fn canonicalize(&self) -> Proj {
        match self.classify() {
            ProjClass::Gauge => Proj::GAUGE,
            ProjClass::Zero => Proj::ZERO,
            ProjClass::Pole => Proj { n: 1, d: 0 },
            ProjClass::Finite => {
                let mut n = self.n;
                let mut d = self.d;
                if d < 0 {
                    n = -n;
                    d = -d;
                }
                let g = gcd64(n.unsigned_abs(), d.unsigned_abs()) as i64;
                if g > 1 {
                    n /= g;
                    d /= g;
                }
                Proj { n, d }
            }
        }
    }

    /// Приближённое значение (только для отображения — не для сравнений!).
    pub fn value_f64(&self) -> f64 {
        self.n as f64 / self.d as f64
    }
}

/// Бинарный НОД (без деления — сдвиги и вычитания; «ноль Python»-стиль
/// железа). gcd(12,18)=6 — сверено с Калькулятором Всего.
fn gcd64(mut a: u64, mut b: u64) -> u64 {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    let za = a.trailing_zeros();
    let zb = b.trailing_zeros();
    a >>= za;
    b >>= zb;
    // общая двойная степень: минимум валиций, фиксируется ОДИН раз;
    // нули, стрипаемые в цикле из b, НЕ входят в НОД (a нечётно)
    let shift = za.min(zb);
    loop {
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        b -= a;
        if b == 0 {
            return a << shift;
        }
        b >>= b.trailing_zeros();
    }
}

/// ТОЧНОЕ сравнение канонических дробей (d1>0, d2>0): 128-битные
/// произведения N1·D2 vs N2·D1 — БЕЗ деления. asm-ядро.
pub fn cmp_canonical(a: &Proj, b: &Proj) -> std::cmp::Ordering {
    debug_assert!(a.d > 0 && b.d > 0, "контракт канонической формы: знаменатель > 0");
    let r = unsafe { poler_az_cmp(a.n, a.d, b.n, b.d) };
    r.cmp(&0)
}

/// Безопасное деление i32 (Q-домен): НИКОГДА не поднимает #DE.
/// den==0 → симметричный полюс ±(2³¹−1) по знаку числителя; 0/0 → 0;
/// MIN/−1 → +MAX (асимметрия дополнения до двух излечена насыщением).
pub fn div_safe_i32(num: i32, den: i32) -> i32 {
    unsafe { poler_az_div_safe_i32(num, den) }
}

/// «Плотность вакуума»: число байтов pack5, где все 5 тритов = ⊙ (код 121).
/// Тест точного нуля БЕЗ эпсилона — изоляция делает равенство строгим.
/// Хвост <16 байт — скалярно (паттерн trit_asm).
pub fn trit_zero_count(packed: &[u8]) -> usize {
    let n = packed.len();
    if n == 0 {
        return 0;
    }
    if crate::asm::caps().popcnt {
        let bulk = n / 16 * 16;
        let head = if bulk > 0 {
            unsafe { poler_az_trit_zero_count(packed.as_ptr(), bulk) }
        } else {
            0
        };
        let tail = packed[bulk..].iter().filter(|&&b| b == TRIT5_ZERO_CODE).count();
        head + tail
    } else {
        trit_zero_count_scalar(packed)
    }
}

/// Доля байтов абсолютного нуля в буфере (0..1) — диагностика покоя
/// троичных данных (.t5q/.t5c: сколько синапсов в рефрактерном стазисе).
pub fn trit_zero_density(packed: &[u8]) -> f64 {
    if packed.is_empty() {
        return 0.0;
    }
    trit_zero_count(packed) as f64 / packed.len() as f64
}

/// Кутритная ноль-сумма: [N, D, −(N+D)] — сумма проекций ≡ 0 ПО
/// ПОСТРОЕНИЮ (A₂-плоскость, фазы 1+ω+ω²=0). asm-ядро: lea+neg.
pub fn qutrit3(n: i64, d: i64) -> [i64; 3] {
    let mut out = [0i64; 3];
    unsafe { poler_az_qutrit3(n, d, out.as_mut_ptr()) };
    out
}

// ------------------------- скалярные зеркала -------------------------

/// Скалярное зеркало `poler_az_class` (референс тестов, не-x86_64).
pub fn classify_scalar(n: i64, d: i64) -> ProjClass {
    ProjClass::from_code((n == 0) as u64 + 2 * (d == 0) as u64)
}

/// Скалярное зеркало `poler_az_div_safe_i32`.
pub fn div_safe_i32_scalar(num: i32, den: i32) -> i32 {
    if den == 0 {
        return if num > 0 {
            i32::MAX
        } else if num < 0 {
            -i32::MAX
        } else {
            0
        };
    }
    if den == -1 && num == i32::MIN {
        return i32::MAX;
    }
    num / den
}

/// Скалярное зеркало `poler_az_trit_zero_count`.
pub fn trit_zero_count_scalar(packed: &[u8]) -> usize {
    packed.iter().filter(|&&b| b == TRIT5_ZERO_CODE).count()
}

/// Скалярное зеркало `poler_az_proj_mul` (checked-умножения).
pub fn proj_mul_scalar(a: &Proj, b: &Proj) -> Proj {
    match (a.n.checked_mul(b.n), a.d.checked_mul(b.d)) {
        (Some(n), Some(d)) => Proj { n, d },
        _ => {
            // знак произведения (n1/d1)*(n2/d2) — XOR четырёх знаков
            let negative = (a.n < 0) ^ (a.d < 0) ^ (b.n < 0) ^ (b.d < 0);
            Proj { n: if negative { POLE_I64_NEG } else { POLE_I64_POS }, d: 0 }
        }
    }
}

/// Скалярное зеркало `poler_az_proj_div` (checked, перекрёстное).
pub fn proj_div_scalar(a: &Proj, b: &Proj) -> Proj {
    match (a.n.checked_mul(b.d), a.d.checked_mul(b.n)) {
        (Some(n), Some(d)) => Proj { n, d },
        _ => {
            let negative = (a.n < 0) ^ (a.d < 0) ^ (b.n < 0) ^ (b.d < 0);
            Proj { n: if negative { POLE_I64_NEG } else { POLE_I64_POS }, d: 0 }
        }
    }
}

/// Скалярное зеркало `poler_az_cmp` (i128-произведения).
pub fn cmp_canonical_scalar(a: &Proj, b: &Proj) -> std::cmp::Ordering {
    debug_assert!(a.d > 0 && b.d > 0);
    let p1 = a.n as i128 * b.d as i128;
    let p2 = b.n as i128 * a.d as i128;
    p1.cmp(&p2)
}

/// Скалярное зеркало `poler_az_qutrit3`.
pub fn qutrit3_scalar(n: i64, d: i64) -> [i64; 3] {
    [n, d, -(n + d)]
}

// ------------------------------ тесты ------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn xs64(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        *state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn proj_vec(n: usize, seed: u64) -> Vec<Proj> {
        let mut s = seed | 1;
        (0..n)
            .map(|_| {
                let a = (xs64(&mut s) >> 33) as i64 % 1000;
                let b = (xs64(&mut s) >> 33) as i64 % 997;
                Proj { n: a, d: b }
            })
            .collect()
    }

    #[test]
    fn az_class_four_projective_classes() {
        let cases = [
            (Proj { n: 5, d: 2 }, ProjClass::Finite),
            (Proj { n: -9, d: 4 }, ProjClass::Finite),
            (Proj::ZERO, ProjClass::Zero),
            (Proj { n: 0, d: -3 }, ProjClass::Zero),
            (Proj::INF, ProjClass::Pole),
            (Proj { n: -7, d: 0 }, ProjClass::Pole),
            (Proj::GAUGE, ProjClass::Gauge),
        ];
        for (p, want) in cases {
            assert_eq!(p.classify(), want, "classify [{:?}]", (p.n, p.d));
            assert_eq!(classify_scalar(p.n, p.d), want, "scalar [{:?}]", (p.n, p.d));
        }
    }

    #[test]
    fn az_inv_is_register_swap() {
        // нуль и полюс зеркальны
        assert_eq!(Proj::ZERO.inv(), Proj::INF);
        assert_eq!(Proj::INF.inv(), Proj::ZERO);
        assert_eq!(Proj::ONE.inv(), Proj::ONE);
        // инволюция: inv(inv(x)) = x
        for p in proj_vec(500, 77) {
            assert_eq!(p.inv().inv(), p, "inv-inv [{:?}]", (p.n, p.d));
            assert_eq!(p.inv(), Proj { n: p.d, d: p.n });
        }
    }

    #[test]
    fn az_pole_is_single_projective_point() {
        // +бесконечность и -бесконечность — ОДНА точка (lambda-инвариант)
        assert_eq!(Proj { n: -1, d: 0 }.canonicalize(), Proj::INF.canonicalize());
        assert_eq!(Proj { n: -999, d: 0 }.canonicalize(), Proj { n: 7, d: 0 }.canonicalize());
        // и обратное к «бесконечной» дроби — нуль, и наоборот
        assert_eq!(Proj { n: 5, d: 0 }.inv().canonicalize(), Proj::ZERO);
        assert_eq!(Proj { n: 0, d: 9 }.inv().canonicalize(), Proj::INF);
    }

    #[test]
    fn az_mul_algebra() {
        let ones = proj_vec(300, 123);
        for p in &ones {
            assert_eq!(Proj::ONE.mul(p), *p, "ONE*x = x (структурно)");
            if p.classify() == ProjClass::Finite {
                assert_eq!(Proj::ZERO.mul(p).canonicalize(), Proj::ZERO, "ZERO*x");
            } else if p.classify() == ProjClass::Pole {
                // 0 * бесконечность = калибровка — единственный запрещённый класс
                assert_eq!(Proj::ZERO.mul(p), Proj::GAUGE, "ZERO*полюс = калибровка");
            }
            if p.classify() == ProjClass::Finite {
                assert_eq!(Proj::INF.mul(p).classify(), ProjClass::Pole, "INF*x");
                assert_eq!(
                    Proj::INF.mul(p).canonicalize(),
                    Proj::INF.canonicalize(),
                    "INF*x канонически = полюс"
                );
            }
        }
        // калибровка: нуль * полюс = [0:0] — единственный запрещённый класс
        assert_eq!(Proj::INF.mul(&Proj::ZERO), Proj::GAUGE);
        assert_eq!(Proj::ZERO.mul(&Proj::INF), Proj::GAUGE);
        // коммутативность + паритет со скаляром
        let a = proj_vec(300, 456);
        let b = proj_vec(300, 789);
        for i in 0..a.len() {
            let asm = a[i].mul(&b[i]);
            let sc = proj_mul_scalar(&a[i], &b[i]);
            assert_eq!(asm, sc, "mul asm=scalar [{:?}]", (a[i].n, a[i].d));
            assert_eq!(asm, b[i].mul(&a[i]), "коммутативность");
        }
        // точная рациональность: (1/2)*(2/3) = [2:6] = 1/3
        let half = Proj { n: 1, d: 2 };
        let two_thirds = Proj { n: 2, d: 3 };
        assert_eq!(half.mul(&two_thirds), Proj { n: 2, d: 6 });
        assert_eq!(half.mul(&two_thirds).canonicalize(), Proj { n: 1, d: 3 });
    }

    #[test]
    fn az_div_without_division() {
        // ГЛАВНЫЙ АНТИ-#DE МЕХАНИЗМ: деление на ноль = ПОЛЮС, не сбой
        let half = Proj { n: 1, d: 2 };
        let third = Proj { n: 1, d: 3 };
        // (1/2)/(2/3) = 3/4 — перекрёстно: [1*3 : 2*2]
        let two_thirds = Proj { n: 2, d: 3 };
        assert_eq!(half.div(&two_thirds), Proj { n: 3, d: 4 });
        // a / 0 = полюс со знаком a ([1:2]/[0:1] -> [-1:0]; насыщение
        // ±MAX включается ТОЛЬКО при переполнении произведения)
        assert_eq!(half.div(&Proj::ZERO).classify(), ProjClass::Pole);
        assert_eq!(half.div(&Proj::ZERO).canonicalize(), Proj::INF);
        assert_eq!((&half.neg()).div(&Proj::ZERO), Proj { n: -1, d: 0 });
        // 0 / a = нуль; a / inf = нуль; inf / a = полюс
        assert_eq!(Proj::ZERO.div(&third).canonicalize(), Proj::ZERO);
        assert_eq!(half.div(&Proj::INF).canonicalize(), Proj::ZERO);
        assert_eq!(Proj::INF.div(&half).canonicalize(), Proj::INF);
        // неопределённости — калибровка [0:0], детектируется одним тестом
        assert_eq!(Proj::ZERO.div(&Proj::ZERO), Proj::GAUGE);
        assert_eq!(Proj::INF.div(&Proj::INF), Proj::GAUGE);
        // inv = div(ONE, x): согласованность двух ядер
        for p in proj_vec(300, 2024) {
            if p.classify() == ProjClass::Finite {
                assert_eq!(
                    Proj::ONE.div(&p).canonicalize(),
                    p.inv().canonicalize(),
                    "inv = ONE/x"
                );
            }
        }
        // случайный паритет asm/скаляр
        let a = proj_vec(300, 11);
        let b = proj_vec(300, 22);
        for i in 0..a.len() {
            assert_eq!(a[i].div(&b[i]), proj_div_scalar(&a[i], &b[i]), "div asm=scalar");
        }
        // переполнение деления: огромное/ничтожное -> полюс
        let big = Proj { n: i64::MAX - 1, d: 1 };
        let tiny = Proj { n: 1, d: i64::MAX - 1 };
        assert_eq!(big.div(&tiny).classify(), ProjClass::Pole);
    }

    #[test]
    fn az_mul_overflow_saturates_to_pole() {
        let big = Proj { n: i64::MAX - 1, d: 1 };
        let r = big.mul(&big);
        assert_eq!((r.n, r.d), (POLE_I64_POS, 0), "+полюс насыщения");
        let neg = Proj { n: -(i64::MAX - 1), d: 1 };
        let r2 = neg.mul(&big);
        assert_eq!((r2.n, r2.d), (POLE_I64_NEG, 0), "-полюс насыщения");
        assert_eq!(r.classify(), ProjClass::Pole);
        // скалярное зеркало согласно
        assert_eq!(proj_mul_scalar(&big, &big), r);
        assert_eq!(proj_mul_scalar(&neg, &big), r2);
    }

    #[test]
    fn az_add_exact_rationals() {
        // 1/2 + 1/3 = 5/6 — ТОЧНО, без 0.8333333333333334
        let half = Proj { n: 1, d: 2 };
        let third = Proj { n: 1, d: 3 };
        assert_eq!(half.add(&third), Proj { n: 5, d: 6 });
        assert_eq!(half.add(&third).value_f64(), 5.0 / 6.0);
        // 1/3 + 1/3 = 6/9 сырое (ТОЧНАЯ пара без сокращения), = 2/3 канонич.
        assert_eq!(third.add(&third), Proj { n: 6, d: 9 });
        assert_eq!(third.add(&third).canonicalize(), Proj { n: 2, d: 3 });
        // x + (-x) = ZERO — аннигиляция для ВСЕХ конечных x
        for p in proj_vec(400, 321) {
            if p.classify() == ProjClass::Finite {
                let neg = p.neg();
                assert_eq!(p.add(&neg).canonicalize(), Proj::ZERO, "x+(-x)");
            }
        }
        // ZERO + x = x
        for p in proj_vec(100, 654) {
            assert_eq!(Proj::ZERO.add(&p).canonicalize(), p.canonicalize());
        }
        // большое сложение: 2^62/1 + 2^62/1 -> переполнение -> полюс
        let huge = Proj::from_int(1 << 62);
        assert_eq!(huge.add(&huge).classify(), ProjClass::Pole);
    }

    #[test]
    fn az_div_safe_table() {
        let table: &[(i32, i32, i32)] = &[
            (7, 2, 3),
            (-7, 2, -3),
            (7, -2, -3),
            (-7, -2, 3),
            (0, 5, 0),
            (7, 0, i32::MAX),
            (-7, 0, -i32::MAX),
            (0, 0, 0),
            (i32::MIN, -1, i32::MAX),
            (i32::MIN, 1, i32::MIN),
            (i32::MAX, 1, i32::MAX),
            (i32::MAX, -1, -i32::MAX),
        ];
        for &(num, den, want) in table {
            let got = div_safe_i32(num, den);
            assert_eq!(got, want, "div_safe({num},{den})");
            let got_s = div_safe_i32_scalar(num, den);
            assert_eq!(got_s, want, "scalar({num},{den})");
        }
    }

    #[test]
    fn az_div_safe_never_faults() {
        // «предотвращение деления на ноль» как КЛАСС: ни одна комбинация
        // краёв не поднимает исключения (на железе idiv дал бы #DE)
        let nums = [i32::MIN, -7, -1, 0, 1, 7, i32::MAX];
        let dens = [-3, -2, -1, 0, 1, 2, 3];
        for &n in &nums {
            for &d in &dens {
                let r = div_safe_i32(n, d);
                let rs = div_safe_i32_scalar(n, d);
                assert_eq!(r, rs, "({n},{d})");
                // симметрия насыщения: полюс +/-MAX симметричен
                if d == 0 && n != 0 {
                    assert_eq!(r.unsigned_abs(), i32::MAX as u32, "полюс симметричен");
                }
            }
        }
    }

    #[test]
    fn az_cmp_exact_where_float_fails() {
        // f64 округляет 3000000000000000001 к 3000000000000000000:
        // float-сравнение говорит «равно», АЗУ говорит «больше» — точно
        let a = Proj { n: 3000000000000000001, d: 2 };
        let b = Proj { n: 3000000000000000000, d: 2 };
        assert_eq!(cmp_canonical(&a, &b), std::cmp::Ordering::Greater);
        assert_eq!(
            cmp_canonical_scalar(&a, &b),
            std::cmp::Ordering::Greater
        );
        // f64 здесь врёт равенством:
        assert_eq!(a.value_f64(), b.value_f64(), "f64 не различает — АЗУ различает");
        // и симметрично: меньше
        assert_eq!(cmp_canonical(&b, &a), std::cmp::Ordering::Less);
    }

    #[test]
    fn az_cmp_canonical_fractions() {
        let f = |n: i64, d: i64| Proj { n, d };
        assert_eq!(cmp_canonical(&f(1, 2), &f(1, 3)), std::cmp::Ordering::Greater);
        // равные дроби с РАЗНЫМИ представлениями: 2/6 == 1/3
        assert_eq!(cmp_canonical(&f(2, 6), &f(1, 3)), std::cmp::Ordering::Equal);
        assert_eq!(cmp_canonical(&f(-5, 3), &f(-7, 3)), std::cmp::Ordering::Greater);
        assert_eq!(cmp_canonical(&f(-7, 3), &f(-5, 3)), std::cmp::Ordering::Less);
        // случайный паритет со скаляром
        let a = proj_vec(500, 111);
        let b = proj_vec(500, 222);
        for i in 0..a.len() {
            let ca = a[i].canonicalize();
            let cb = b[i].canonicalize();
            if ca.d > 0 && cb.d > 0 {
                assert_eq!(
                    cmp_canonical(&ca, &cb),
                    cmp_canonical_scalar(&ca, &cb),
                    "cmp [{:?}] vs [{:?}]",
                    (ca.n, ca.d),
                    (cb.n, cb.d)
                );
            }
        }
    }

    #[test]
    fn az_trit_zero_count_density() {
        let mut s = 4242u64;
        let mut packed = vec![0u8; 1000];
        let mut expect = 0;
        for b in packed.iter_mut() {
            let v = (xs64(&mut s) >> 33) % 4;
            if v == 0 {
                *b = TRIT5_ZERO_CODE;
                expect += 1;
            } else {
                // любое значение, КРОМЕ 121 (иначе тест слеп)
                let mut cand = (xs64(&mut s) % 243) as u8;
                if cand == TRIT5_ZERO_CODE {
                    cand = cand.wrapping_add(1);
                }
                *b = cand;
            }
        }
        let got = trit_zero_count(&packed);
        assert_eq!(got, expect, "вакуум-скан asm");
        assert_eq!(trit_zero_count_scalar(&packed), expect, "вакуум-скан scalar");
        let density = trit_zero_density(&packed);
        assert!((density - expect as f64 / 1000.0).abs() < 1e-12);
        // пустой буфер — нуль и ноль плотности
        assert_eq!(trit_zero_count(&[]), 0);
        assert_eq!(trit_zero_density(&[]), 0.0);
    }

    #[test]
    fn az_qutrit3_zero_sum_by_construction() {
        let mut s = 97531u64;
        for _ in 0..500 {
            let n = (xs64(&mut s) >> 33) as i64 % 1_000_000;
            let d = (xs64(&mut s) >> 33) as i64 % 1_000_000;
            let q = qutrit3(n, d);
            assert_eq!(q, qutrit3_scalar(n, d), "asm=scalar");
            // сумма проекций — ТОЧНЫЙ ноль: A2-плоскость
            let sum = (q[0] as i128) + (q[1] as i128) + (q[2] as i128);
            assert_eq!(sum, 0, "сумма кутрита [{n},{d}]");
            // норма — i128, без переполнения
            let norm2 = (q[0] as i128).pow(2) + (q[1] as i128).pow(2) + (q[2] as i128).pow(2);
            let expect = (n as i128).pow(2) + (d as i128).pow(2) + ((n + d) as i128).pow(2);
            assert_eq!(norm2, expect);
        }
        // кутрит из нуля и полюса: [0,0,0] и [1,0,-1]
        assert_eq!(qutrit3(0, 0), [0, 0, 0]);
        assert_eq!(qutrit3(1, 0), [1, 0, -1]);
    }

    #[test]
    fn az_trit_negation_total_vs_int_min() {
        // троичная решётка: негация ТОТАЛЬНА — x + (-x) = 0 для всех x
        let mut s = 13579u64;
        for _ in 0..300 {
            let t = ((xs64(&mut s) >> 33) % 3) as i64 - 1; // -1..=1
            assert_eq!(-(-t), t, "негация — инволюция");
            assert_eq!(t + (-t), 0, "аннигиляция");
        }
        // а вот двоичное дополнение до двух ломается ровно в ОДНОЙ точке:
        assert_eq!(i32::MIN.checked_neg(), None, "MIN — точка-изгой негации");
        assert_eq!(i64::MIN.checked_neg(), None);
        // на решётке 40 тритов радиус симметричен ТОЧНО: негация MAX валидна
        assert_eq!(TRIT_U64_RADIUS.checked_neg(), Some(-TRIT_U64_RADIUS));
        // (у i64: MAX-негация валидна, MIN — нет: асимметрия ровно 1)
        assert_eq!(i64::MAX.checked_neg(), Some(-i64::MAX));
    }

    #[test]
    fn az_packed_zero_is_unique() {
        // перебор ВСЕХ 243 кодов 5-тритного байта: все-триты-ноль
        // кодируется РОВНО ОДНИМ значением — 121 (уникальность нуля)
        let mut out = vec![0i8; 5];
        for v in 0u8..243 {
            unpack5_ref(&[v], &mut out);
            let all_zero = out.iter().all(|&t| t == 0);
            assert_eq!(
                all_zero,
                v == TRIT5_ZERO_CODE,
                "код {v}: все-ноль = {}",
                all_zero
            );
        }
    }

    #[test]
    fn az_q30_isolation_ulp() {
        // ИЗОЛЯЦИЯ машинного нуля Q30: минимальный ненулевой |x| = 1 ULP;
        // шар радиуса 1 ULP вокруг нуля содержит ТОЛЬКО сам ноль
        let min_abs = (-1i32..=1).filter(|&x| x != 0).map(|x| x.abs()).min();
        assert_eq!(min_abs, Some(1), "в открытом шаре радиуса 1 ULP — только сам ноль");
        // ULP Q30 из Калькулятора Всего: 2^-30 = 9.313225746154785e-10
        assert!((Q30_ULP_F64 - 2f64.powi(-30)).abs() < 1e-26);
        // halving умирает: последний субнормаль f64 = 2^-1074 (5e-324,
        // битовая запись 0x1), его половина — УЖЕ ТОЧНЫЙ НОЛЬ
        let sub = f64::from_bits(1);
        assert!(sub > 0.0, "последний субнормаль больше нуля");
        assert_eq!(sub / 2.0, 0.0, "цепочка эпсилон/2 конечна: halving умирает в ⊙");
    }

    #[test]
    fn az_ternary_radius_constants() {
        // 2*R + 1 = 3^40: полный симметричный диапазон 40 тритов (в u64:
        // сам радиус R влезает в i64, а ПОЛНЫЙ ПРОЛЁТ 3^40 — только в u64)
        assert_eq!(2 * TRIT_U64_RADIUS as u64 + 1, 3u64.pow(40));
        // 40 тритов = 8 pack5-байтов — ровно один u64
        assert_eq!(TRITS_PER_U64, 40);
        assert_eq!(TRITS_PER_U64 / 5, 8);
        // код нуля: (3^5-1)/2 = 121 — Калькулятор Всего
        assert_eq!(TRIT5_ZERO_CODE as u32, (3u32.pow(5) - 1) / 2);
        // полюса насыщения симметричны
        assert_eq!(POLE_I64_NEG, -POLE_I64_POS);
    }

    #[test]
    fn az_engine_trits_zero_parity() {
        // мост к каноническому троичному слою движка: ноль Trits — все
        // триты в покое, pack5 даёт байты 121, вакуум-скан их видит
        use crate::calc::trits::Trits;
        let z = Trits::zero();
        assert!(z.is_zero());
        let mut digits = z.digits.clone();
        while digits.len() % 5 != 0 {
            digits.insert(0, 0);
        }
        let mut packed = vec![0u8; digits.len() / 5];
        crate::asm::trit_asm::pack5(&digits, &mut packed);
        assert!(packed.iter().all(|&b| b == TRIT5_ZERO_CODE));
        assert_eq!(trit_zero_count(&packed), packed.len());
        assert!((trit_zero_density(&packed) - 1.0).abs() < 1e-12);
        // и число ноль: Trits::from_i64(0) — тоже все-ноль
        let z0 = Trits::from_i64(0).unwrap();
        assert!(z0.digits.iter().all(|&t| t == 0));
    }

    /// локальная ссылка-unpack (5 трит из байта) для теста уникальности
    fn unpack5_ref(bytes: &[u8], out: &mut [i8]) {
        for &v in bytes {
            let mut x = v;
            for k in (0..5).rev() {
                let r = x % 3;
                out[k] = r as i8 - 1;
                x /= 3;
            }
        }
    }
}

/// Безопасное деление f32 — семантика АЗУ в плавающем домене:
/// * `den ≠ 0` — обычное деление;
/// * `den = 0, num ≠ 0` — ПОЛЮС: насыщение ±f32::MAX по знаку числителя
///   (∞ как предельная точка ℝP¹, без NaN-отравления конвейера);
/// * `0/0` — КАЛИБРОВКА [0:0] → 0.0 (принятое значение неопределённости).
///
/// Применение: нормировки FEP `x/(1+x)` (полюс x=−1), STDP-softmax
/// (Σ=0 → равномерные награды), любая статистика с делением на сумму.
pub fn div_safe_f32(num: f32, den: f32) -> f32 {
    if den != 0.0 {
        num / den
    } else if num == 0.0 {
        0.0
    } else if num > 0.0 {
        f32::MAX
    } else {
        f32::MIN
    }
}

#[cfg(test)]
mod div_safe_f32_tests {
    use super::div_safe_f32;

    #[test]
    fn div_safe_f32_pole_semantics() {
        assert_eq!(div_safe_f32(1.0, 2.0), 0.5);
        // полюс: насыщение по знаку числителя (НЕ NaN!)
        assert_eq!(div_safe_f32(1.0, 0.0), f32::MAX);
        assert_eq!(div_safe_f32(-2.0, 0.0), f32::MIN);
        // калибровка 0/0 → 0
        assert_eq!(div_safe_f32(0.0, 0.0), 0.0);
        // FEP-нормировка x/(1+x): полюс x = −1 больше не отравляет
        let norm = |x: f32| div_safe_f32(x, 1.0 + x);
        assert!(norm(-1.0).is_finite());
        assert_eq!(norm(1.0), 0.5);
        assert_eq!(norm(0.0), 0.0);
    }
}
