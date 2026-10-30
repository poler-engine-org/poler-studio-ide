#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Qutrit-лаборатория: троичная квантовая физика в Калькуляторе Всего.
//!
//! Открытия полевой сессии v0.63.0 (см. docs/UNDOCUMENTED.md, раздел
//! «Qutrit-лаборатория») переведены в регрессию: ω = e^(2πi/3), алгебра
//! Вейля clock/shift, Гелл-Манн su(3), QFT кутрита F3 и F3⊗F3, спин-1
//! (бозон) против кубита (фермион), фазовый вихрь, трит-мост и
//! квантовое блуждание по треугольнику.
//!
//! Сессия 3 добавляет: SUM-гейт и состояние Белла |Φ₃⁺⟩ (блок 9),
//! частичный след селекторными матрицами со смешанностью 1/3,
//! стабилизаторы и 3-цикл Беллов под X⊗X† (блок 10), QPE-трит-считывание
//! и 81-мерный трит-конвейер «19 = 1T01 → фазы → QFT-декод» (блок 11).
//!
//! Сессия 7 добавляет: CGLMP d=4 на двух квквартах (блоки 16–17) —
//! локальный предел 2 (брутфорс 256 стратегий), max-ent аналитику статьи
//! (2/3)(√2+√(10−√2)) и некомпактный оптимум I₄* = 2.972698267102 на
//! палиндроме (1,γ₄,γ₄,1), где γ₄ — корень квадратики над √2-башней
//! (DE+eig переоткрыли его до вывода алгебры): квкварт бьёт кутрит.
//!
//! Все проверки сведены к вещественным скалярам через abs/trace/det:
//! принтер Калькулятора печатает комплексные числа строкой, поэтому
//! тест оперирует только модулями, следами и нуль-нормами
//! trace(M†M) = Σ|m_ij|² (равна нулю тогда и только тогда, когда M = 0).

use poler_engine::calc::CalcState;

const SETUP: &[&str] = &[
    "let om = exp(2*pi/3*i)",
    "let Z3 = [1,0,0; 0,om,0; 0,0,om^2]",
    "let X3 = [0,0,1; 1,0,0; 0,1,0]",
    "let F3 = (1/sqrt(3)) * [1,1,1; 1,om,om^2; 1,om^2,om]",
    "let l1 = [0,1,0; 1,0,0; 0,0,0]",
    "let l2 = [0,-i,0; i,0,0; 0,0,0]",
    "let l3 = [1,0,0; 0,-1,0; 0,0,0]",
    "let l4 = [0,0,1; 0,0,0; 1,0,0]",
    "let l5 = [0,0,-i; 0,0,0; i,0,0]",
    "let l6 = [0,0,0; 0,0,1; 0,1,0]",
    "let l7 = [0,0,0; 0,0,-i; 0,i,0]",
    "let l8 = (1/sqrt(3)) * [1,0,0; 0,1,0; 0,0,-2]",
    "let Sx = (1/sqrt(2)) * [0,1,0; 1,0,1; 0,1,0]",
    "let Sy = (1/sqrt(2)) * [0,-i,0; i,0,-i; 0,i,0]",
    "let Sz = [1,0,0; 0,0,0; 0,0,-1]",
    "let N3 = [0,0,0; 0,1,0; 0,0,2]",
    "let psi0 = (1/sqrt(3)) * [1; om; om^2]",
    // Блоки 9–11 (сессия 3): SUM-гейт, частичный след, QPE-конвейер
    "let e0 = [1; 0; 0]",
    "let e1 = [0; 1; 0]",
    "let e2 = [0; 0; 1]",
    "let P0 = [1,0,0; 0,0,0; 0,0,0]",
    "let P1 = [0,0,0; 0,1,0; 0,0,0]",
    "let P2 = [0,0,0; 0,0,0; 0,0,1]",
    "let SUM = kron(P0, eye(3)) + kron(P1, X3) + kron(P2, X3*X3)",
];

fn lab() -> CalcState {
    let mut st = CalcState::new();
    for line in SETUP {
        st.eval_line(line)
            .unwrap_or_else(|e| panic!("{line}: {e}"));
    }
    st
}

/// Значение выражения как вещественный скаляр (принтер даёт «x.y»).
fn re(st: &mut CalcState, expr: &str) -> f64 {
    let out = st
        .eval_line(expr)
        .unwrap_or_else(|e| panic!("{expr}: {e}"));
    out.trim()
        .parse::<f64>()
        .unwrap_or_else(|_| panic!("{expr}: вывод не вещественный скаляр: '{out}'"))
}

/// Модуль комплексного результата: abs() Калькулятора возвращает f64.
fn modulus(st: &mut CalcState, expr: &str) -> f64 {
    re(st, &format!("abs({expr})"))
}

/// Нуль-тест матрицы/вектора: trace(M†M) = Σ|m_ij|² = 0 ⟺ M = 0.
fn zero_norm(st: &mut CalcState, expr: &str) -> f64 {
    re(st, &format!("trace(dagger({expr}) * ({expr}))"))
}


/// Вещественная часть комплексного вывода принтера («x + yi» → x).
fn re_part(st: &mut CalcState, expr: &str) -> f64 {
    let out = st
        .eval_line(expr)
        .unwrap_or_else(|e| panic!("{expr}: {e}"));
    let s = out.trim();
    let real = s.split(" + ").next().unwrap().split(" - ").next().unwrap();
    real.parse::<f64>()
        .unwrap_or_else(|_| panic!("{expr}: не число: '{out}'"))
}

fn near(x: f64, target: f64, tol: f64, what: &str) {
    assert!(
        (x - target).abs() < tol,
        "{what}: получилось {x}, ожидалось {target} (допуск {tol})"
    );
}

// ---------------------------------------------------------------------------
// Блок 1. Фундамент: ω = e^(2πi/3)
// ---------------------------------------------------------------------------

#[test]
fn qutrit_omega_fundament() {
    let mut st = lab();
    near(modulus(&mut st, "om^3 - 1"), 0.0, 1e-12, "ω³ = 1");
    near(modulus(&mut st, "1 + om + om^2"), 0.0, 1e-12, "1 + ω + ω² = 0");
    near(modulus(&mut st, "om + om^2 + 1"), 0.0, 1e-12, "ω + ω² = −1");
    near(modulus(&mut st, "om"), 1.0, 1e-12, "|ω| = 1");
}

// ---------------------------------------------------------------------------
// Блок 2. Алгебра Вейля: Z·X = ω·X·Z и спектр сдвига
// ---------------------------------------------------------------------------

#[test]
fn qutrit_weyl_clock_shift() {
    let mut st = lab();
    // Соотношение Вейля (верная конвенция): нулевая матрица
    near(
        zero_norm(&mut st, "Z3*X3 - om*X3*Z3"),
        0.0,
        1e-26,
        "Weyl: Z·X = ω·X·Z",
    );
    // Негативный контроль: неверная конвенция (ω²) даёт норму O(1)
    let bad = zero_norm(&mut st, "Z3*X3 - om^2*X3*Z3");
    assert!(bad > 1.0, "негативный контроль: {bad} должно быть O(1)");
    // Фазовый обмен X·Z·X⁻¹·Z⁻¹ = ω²·I → trace = 3ω²
    near(
        modulus(&mut st, "trace(X3*Z3*inv(X3)*inv(Z3)) - 3*om^2"),
        0.0,
        1e-12,
        "X·Z·X⁻¹·Z⁻¹ = ω²·I",
    );
}

#[test]
fn qutrit_shift_spectrum_is_cube_roots() {
    let mut st = lab();
    // Спектр циклического сдвига = {1, ω, ω²}: det(X − λI) = 0 для корней
    near(
        modulus(&mut st, "det(X3 - eye(3))"),
        0.0,
        1e-10,
        "1 ∈ spec(X3)",
    );
    near(
        modulus(&mut st, "det(X3 - om*eye(3))"),
        0.0,
        1e-10,
        "ω ∈ spec(X3)",
    );
    near(
        modulus(&mut st, "det(X3 - om^2*eye(3))"),
        0.0,
        1e-10,
        "ω² ∈ spec(X3)",
    );
    // λ = 2 — не собственное значение: det(X−2I) = 7 точно
    let not_eig = modulus(&mut st, "det(X3 - 2*eye(3))");
    assert!(
        (not_eig - 7.0).abs() < 1.0,
        "det(X3 − 2·I) = {not_eig}, ожидалось 7"
    );
}

// ---------------------------------------------------------------------------
// Блок 3. QFT кутрита: F3 и двухкутритное F3⊗F3
// ---------------------------------------------------------------------------

#[test]
fn qutrit_qft_unitary_and_duality() {
    let mut st = lab();
    // Унитарность F3†F3 = I → trace = 3
    near(
        modulus(&mut st, "trace(dagger(F3)*F3) - 3"),
        0.0,
        1e-12,
        "F3†·F3 = I",
    );
    // Двойственность Вейля: QFT переводит сдвиг в часы
    near(
        zero_norm(&mut st, "F3*X3*dagger(F3) - Z3"),
        0.0,
        1e-24,
        "F3·X3·F3† = Z3",
    );
    // Двухкутритное QFT: F3⊗F3 унитарно в 9 измерениях
    st.eval_line("let F9 = kron(F3, F3)").unwrap();
    near(
        modulus(&mut st, "trace(dagger(F9)*F9) - 9"),
        0.0,
        1e-11,
        "(F3⊗F3)†·(F3⊗F3) = I₉",
    );
}

// ---------------------------------------------------------------------------
// Блок 4. Гелл-Манн su(3): структурные константы и Казимир
// ---------------------------------------------------------------------------

#[test]
fn qutrit_gell_mann_su3() {
    let mut st = lab();
    // Коммутаторы с точными структурными константами f_abc
    near(
        zero_norm(&mut st, "l1*l2 - l2*l1 - 2*i*l3"),
        0.0,
        1e-26,
        "[λ1,λ2] = 2i·λ3 (f123 = 1)",
    );
    near(
        zero_norm(&mut st, "l4*l5 - l5*l4 - i*(l3 + sqrt(3)*l8)"),
        0.0,
        1e-26,
        "[λ4,λ5] = i·(λ3 + √3·λ8)",
    );
    near(
        zero_norm(&mut st, "l6*l7 - l7*l6 - i*(-l3 + sqrt(3)*l8)"),
        0.0,
        1e-26,
        "[λ6,λ7] = i·(−λ3 + √3·λ8)",
    );
    // Антикоммутатор {λ1,λ2} = 0
    near(
        zero_norm(&mut st, "l1*l2 + l2*l1"),
        0.0,
        1e-26,
        "{λ1,λ2} = 0",
    );
    // Оператор Казимира: Σλa² = (16/3)·I → trace = 16
    near(
        modulus(
            &mut st,
            "trace(l1*l1 + l2*l2 + l3*l3 + l4*l4 + l5*l5 + l6*l6 + l7*l7 + l8*l8) - 16",
        ),
        0.0,
        1e-12,
        "Казимир Σλ² = (16/3)·I",
    );
    // expm(iθλ2) ∈ SU(3): унитарность и det = 1
    st.eval_line("let U = expm(i * 0.7 * l2)").unwrap();
    near(
        modulus(&mut st, "trace(dagger(U)*U) - 3"),
        0.0,
        1e-12,
        "U†·U = I",
    );
    near(modulus(&mut st, "det(U) - 1"), 0.0, 1e-9, "det U = 1");
}

// ---------------------------------------------------------------------------
// Блок 5. Спин-1: целый спин — бозон; кубит — фермион
// ---------------------------------------------------------------------------

#[test]
fn qutrit_spin1_boson_vs_qubit_fermion() {
    let mut st = lab();
    // Алгебра углового момента [Sx,Sy] = i·Sz
    near(
        zero_norm(&mut st, "Sx*Sy - Sy*Sx - i*Sz"),
        0.0,
        1e-26,
        "[Sx,Sy] = i·Sz",
    );
    // Полный оборот 2π: кутрит возвращается (trace = 3, бозон),
    // кубит набирает −1 (trace = −2, фермион)
    near(
        modulus(&mut st, "trace(expm(-2*pi*i*Sz)) - 3"),
        0.0,
        1e-9,
        "спин-1: R(2π) = +I",
    );
    near(
        modulus(&mut st, "trace(expm(-2*pi*i*0.5*pauli_z())) + 2"),
        0.0,
        1e-9,
        "спин-1/2: R(2π) = −I",
    );
    // На состояниях: e^{-i2πSz}|0⟩ = +|0⟩; e^{-i2π(σz/2)}|0⟩ = −|0⟩
    near(
        zero_norm(&mut st, "schrodinger(Sz, [1; 0; 0], 2*pi) - [1; 0; 0]"),
        0.0,
        1e-20,
        "кутрит: 2π → +ψ",
    );
    near(
        zero_norm(&mut st, "schrodinger(0.5*pauli_z(), [1; 0], 2*pi) + [1; 0]"),
        0.0,
        1e-20,
        "кубит: 2π → −ψ",
    );
}

// ---------------------------------------------------------------------------
// Блок 6. Фазовый вихрь ψ = (1, ω, ω²)/√3
// ---------------------------------------------------------------------------

#[test]
fn qutrit_phase_vortex() {
    let mut st = lab();
    st.eval_line("let flat = (1/sqrt(3)) * [1; 1; 1]").unwrap();
    // Вихрь — собственный вектор сдвига: X3·ψ = ω²·ψ («импульс»)
    near(
        zero_norm(&mut st, "X3*psi0 - om^2*psi0"),
        0.0,
        1e-26,
        "X3·ψ = ω²·ψ",
    );
    // Эволюция под оператором числа: t = 2π/3 раскручивает вихрь в плоскую волну
    near(
        zero_norm(&mut st, "schrodinger(N3, psi0, 2*pi/3) - flat"),
        0.0,
        1e-24,
        "ψ(2π/3) = (1,1,1)/√3",
    );
    // t = 2π: полный возврат вихря (период)
    near(
        zero_norm(&mut st, "schrodinger(N3, psi0, 2*pi) - psi0"),
        0.0,
        1e-24,
        "ψ(2π) = ψ0",
    );
    // QFT вихря локализует импульс: F3·ψ = |2⟩
    near(
        zero_norm(&mut st, "F3*psi0 - [0; 0; 1]"),
        0.0,
        1e-24,
        "F3·ψ = |2⟩",
    );
}

// ---------------------------------------------------------------------------
// Блок 7. Трит-мост: сбалансированная троичность × кубические фазы
// ---------------------------------------------------------------------------

#[test]
fn qutrit_trit_bridge() {
    let mut st = lab();
    // Сбалансированная троичная запись: 19 = 27 − 9 + 1 = «1T01»
    let s19 = st.eval_line("trits(19)").unwrap();
    assert!(
        s19.contains("1T01"),
        "trits(19) = '{s19}', ожидалась запись 1T01"
    );
    near(
        re(&mut st, "trit_val(trits(19))"),
        19.0,
        1e-12,
        "trit_val∘trits = id",
    );
    near(
        re(&mut st, "trit_val(trits(-19))"),
        -19.0,
        1e-12,
        "отрицательные числа в тритах",
    );
    // Троичная логика Клини: отрицание «1TT» (5) — это «T11»
    let not5 = st.eval_line("trit_not(trits(5))").unwrap();
    assert!(
        not5.contains("T11"),
        "trit_not(trits(5)) = '{not5}', ожидалось T11"
    );
    // Фаза несёт трит по модулю 3: ω^19 = ω (19 mod 3 = 1)
    near(modulus(&mut st, "om^19 - om"), 0.0, 1e-12, "ω^19 = ω");
    near(
        modulus(&mut st, "om^trit_val(trits(19)) - om"),
        0.0,
        1e-12,
        "ω^trit_val(trits(19)) = ω",
    );
    // Фазовое «сложение» тритов: ω^a·ω^b = ω^(a+b)
    near(
        modulus(&mut st, "om^7 * om^11 - om^(7+11)"),
        0.0,
        1e-12,
        "ω^a·ω^b = ω^(a+b)",
    );
}

// ---------------------------------------------------------------------------
// Блок 8. Квантовое блуждание по треугольнику: H = X + X† + Z + Z†
// ---------------------------------------------------------------------------

#[test]
fn qutrit_triangle_walk() {
    let mut st = lab();
    st.eval_line("let Hqw = X3 + dagger(X3) + Z3 + dagger(Z3)")
        .unwrap();
    // Эрмитовость гамильтониана — эволюция унитарна
    near(
        zero_norm(&mut st, "dagger(Hqw) - Hqw"),
        0.0,
        1e-26,
        "H = X+X†+Z+Z† эрмитов",
    );
    // Норма состояния сохраняется при блуждании
    near(
        modulus(
            &mut st,
            "trace(dagger(schrodinger(Hqw, [1; 0; 0], 1)) * schrodinger(Hqw, [1; 0; 0], 1)) - 1",
        ),
        0.0,
        1e-12,
        "|ψ(t)|² = 1 при блуждании",
    );
    // Спектр {−2, 1−√3, 1+√3}: след = 0, det = 4 — точно
    near(
        modulus(&mut st, "trace(Hqw)"),
        0.0,
        1e-12,
        "trace H = 0 (сумма спектра)",
    );
    near(
        modulus(&mut st, "det(Hqw) - 4"),
        0.0,
        1e-9,
        "det H = 4 (произведение спектра)",
    );
}

// ---------------------------------------------------------------------------
// Блок 9. SUM-гейт (кутритный CNOT) и состояние Белла |Φ₃⁺⟩
// ---------------------------------------------------------------------------

#[test]
fn qutrit_sum_gate_and_bell_pair() {
    let mut st = lab();
    // SUM = Σ_j P_j ⊗ X3^j — перестановочный гейт: SUM†·SUM = I₉
    near(
        modulus(&mut st, "trace(dagger(SUM)*SUM) - 9"),
        0.0,
        1e-12,
        "SUM†·SUM = I₉ (унитарность)",
    );
    // Квантовый сумматор по модулю 3: |j⟩|k⟩ ↦ |j⟩|(j+k) mod 3⟩
    near(
        zero_norm(&mut st, "SUM*kron(e1,e1) - kron(e1,e2)"),
        0.0,
        1e-26,
        "|1⟩|1⟩ → |1⟩|2⟩",
    );
    near(
        zero_norm(&mut st, "SUM*kron(e2,e2) - kron(e2,e1)"),
        0.0,
        1e-26,
        "|2⟩|2⟩ → |2⟩|1⟩",
    );
    near(
        zero_norm(&mut st, "SUM*kron(e2,e1) - kron(e2,e0)"),
        0.0,
        1e-26,
        "|2⟩|1⟩ → |2⟩|0⟩ (перенос по модулю)",
    );
    // Запутывание: SUM·((F3|0⟩)⊗|0⟩) = |Φ₃⁺⟩ = (|00⟩+|11⟩+|22⟩)/√3
    near(
        zero_norm(
            &mut st,
            "SUM*kron(F3*e0, e0) - (1/sqrt(3))*(kron(e0,e0) + kron(e1,e1) + kron(e2,e2))",
        ),
        0.0,
        1e-24,
        "SUM·(F3|0⟩⊗|0⟩) = |Φ₃⁺⟩",
    );
    // Плот-кодирование: (I⊗X^a)|Φ⟩ → SUM† → flat⊗|a⟩ — трит одной частицей
    near(
        zero_norm(
            &mut st,
            "dagger(SUM)*kron(eye(3), X3)*(SUM*kron(F3*e0,e0)) - kron((1/sqrt(3))*[1;1;1], e1)",
        ),
        0.0,
        1e-24,
        "dense coding: a=1 декодирован",
    );
    near(
        zero_norm(
            &mut st,
            "dagger(SUM)*kron(eye(3), X3*X3)*(SUM*kron(F3*e0,e0)) - kron((1/sqrt(3))*[1;1;1], e2)",
        ),
        0.0,
        1e-24,
        "dense coding: a=2 декодирован",
    );
}

// ---------------------------------------------------------------------------
// Блок 10. Частичный след, энтропия и стабилизаторы пары Белла
// ---------------------------------------------------------------------------

#[test]
fn qutrit_bell_partial_trace_and_stabilizers() {
    let mut st = lab();
    st.eval_line("let bell = SUM*kron(F3*e0, e0)").unwrap();
    st.eval_line("let rho = bell*dagger(bell)").unwrap();
    // Селекторные матрицы: S_m = I₃ ⊗ ⟨m| — частичный след без reshape
    st.eval_line("let S0 = kron(eye(3), [1,0,0])").unwrap();
    st.eval_line("let S1 = kron(eye(3), [0,1,0])").unwrap();
    st.eval_line("let S2 = kron(eye(3), [0,0,1])").unwrap();
    st.eval_line("let rhoA = S0*rho*dagger(S0) + S1*rho*dagger(S1) + S2*rho*dagger(S2)")
        .unwrap();
    // Tr_B(ρ) = I/3 — максимально смешанное состояние подсистемы
    near(
        zero_norm(&mut st, "rhoA - (1/3)*eye(3)"),
        0.0,
        1e-24,
        "Tr_B(ρ) = I/3",
    );
    // Чистоты: глобально чистое, локально максимально смешанное
    near(
        modulus(&mut st, "trace(rho*rho) - 1"),
        0.0,
        1e-12,
        "Tr(ρ²) = 1 (глобальная чистота)",
    );
    near(
        modulus(&mut st, "trace(rhoA*rhoA) - 1/3"),
        0.0,
        1e-12,
        "Tr(ρA²) = 1/3 (смешанность)",
    );
    // Спектр ρA = {1/3, 1/3, 1/3} ⟹ энтропия фон Неймана S = ln 3
    near(
        re(&mut st, "ln(3) + 3*(1/3)*ln(1/3)"),
        0.0,
        1e-12,
        "S = −Σλ·lnλ = ln 3 (λ = 1/3)",
    );
    // Стабилизаторы пары Белла: (Z⊗Z†) и (X⊗X) — у кутритов X† ≠ X!
    near(
        zero_norm(&mut st, "kron(Z3, dagger(Z3))*bell - bell"),
        0.0,
        1e-24,
        "(Z⊗Z†)|Φ₃⁺⟩ = |Φ₃⁺⟩",
    );
    near(
        zero_norm(&mut st, "kron(X3, X3)*bell - bell"),
        0.0,
        1e-24,
        "(X⊗X)|Φ₃⁺⟩ = |Φ₃⁺⟩",
    );
    // Негативный контроль: X⊗X† НЕ стабилизатор (3 нечётно, X† ≠ X)
    let bad = zero_norm(&mut st, "kron(X3, dagger(X3))*bell - bell");
    assert!(
        bad > 1.0,
        "негативный контроль X⊗X†: {bad} должно быть O(1)"
    );
    // Открытие сессии 3: X⊗X† вращает семейство Белла 3-циклом
    near(
        zero_norm(
            &mut st,
            "kron(X3,dagger(X3))*bell - (1/sqrt(3))*(kron(e0,e1) + kron(e1,e2) + kron(e2,e0))",
        ),
        0.0,
        1e-24,
        "W: |Φ₃⁺⟩ → |Ψ₀⟩ = (|01⟩+|12⟩+|20⟩)/√3",
    );
    near(
        zero_norm(
            &mut st,
            "kron(X3,dagger(X3))*(kron(X3,dagger(X3))*bell) - (1/sqrt(3))*(kron(e0,e2) + kron(e1,e0) + kron(e2,e1))",
        ),
        0.0,
        1e-24,
        "W: |Ψ₀⟩ → |Ψ₁⟩ = (|02⟩+|10⟩+|21⟩)/√3",
    );
    near(
        zero_norm(
            &mut st,
            "kron(X3,dagger(X3))*(kron(X3,dagger(X3))*(kron(X3,dagger(X3))*bell)) - bell",
        ),
        0.0,
        1e-24,
        "W³ = I: 3-цикл Беллов замкнут",
    );
    // Отрицательный контроль редукции: product-состояние остаётся чистым
    st.eval_line("let prod = kron((1/sqrt(3))*[1;1;1], (1/sqrt(3))*[1;1;1])")
        .unwrap();
    st.eval_line("let rhoP = prod*dagger(prod)").unwrap();
    st.eval_line(
        "let rhoPA = S0*rhoP*dagger(S0) + S1*rhoP*dagger(S1) + S2*rhoP*dagger(S2)",
    )
    .unwrap();
    near(
        modulus(&mut st, "trace(rhoPA*rhoPA) - 1"),
        0.0,
        1e-12,
        "product-состояние: редукция чистая (нет запутанности)",
    );
    near(
        zero_norm(&mut st, "rhoPA - (1/3)*[1;1;1]*dagger([1;1;1])"),
        0.0,
        1e-24,
        "Tr_B(ρP) = |flat⟩⟨flat|",
    );
}

// ---------------------------------------------------------------------------
// Блок 11. QPE-трит-считывание и 81-мерный трит-конвейер (19 = 1T01)
// ---------------------------------------------------------------------------

#[test]
fn qutrit_qpe_and_trit_lattice() {
    let mut st = lab();
    // Одно-кутритное QPE: |0⟩ → F3 → фаза ω^a → F3† ⇒ |a⟩ — точное считывание
    near(
        zero_norm(&mut st, "dagger(F3)*Z3*((1/sqrt(3))*[1;1;1]) - e1"),
        0.0,
        1e-26,
        "QPE: фаза ω¹ → трит |1⟩",
    );
    near(
        zero_norm(&mut st, "dagger(F3)*(Z3*Z3)*((1/sqrt(3))*[1;1;1]) - e2"),
        0.0,
        1e-26,
        "QPE: фаза ω² → трит |2⟩",
    );
    // Сбалансированный трит −1: ω^(−1) = ω² читается как |2⟩ ≡ −1 (mod 3)
    near(
        zero_norm(&mut st, "dagger(F3)*dagger(Z3)*((1/sqrt(3))*[1;1;1]) - e2"),
        0.0,
        1e-26,
        "QPE: трит −1 → |2⟩ (баланс без потерь)",
    );
    // 81-мерный конвейер: 19 = «1T01» → D4 = Z3⊗Z3†⊗I⊗Z3 → F81†·D4·flat81 = e46
    st.eval_line("let D4 = kron(Z3, kron(dagger(Z3), kron(eye(3), Z3)))")
        .unwrap();
    st.eval_line("let F81 = kron(kron(F3,F3), kron(F3,F3))").unwrap();
    st.eval_line(
        "let flat81 = kron(kron((1/sqrt(3))*[1;1;1],(1/sqrt(3))*[1;1;1]), kron((1/sqrt(3))*[1;1;1],(1/sqrt(3))*[1;1;1]))",
    )
    .unwrap();
    // e46: цифры (1,2,0,1)₃ = 27+18+0+1 = 46 — «1T01» с заменой 2↔T
    near(
        zero_norm(
            &mut st,
            "dagger(F81)*D4*flat81 - kron(e1, kron(e2, kron(e0, e1)))",
        ),
        0.0,
        1e-20,
        "19 = 1T01 → фазы → QFT-декод → e46 (детерминированно)",
    );
    // Born-вероятность правильного декода: |⟨e46|декод⟩|² = 1
    near(
        modulus(
            &mut st,
            "trace(dagger(kron(e1, kron(e2, kron(e0, e1))))*(dagger(F81)*D4*flat81))",
        ),
        1.0,
        1e-10,
        "|⟨e46|декод⟩| = 1 — вероятность 1",
    );
    // Круг замкнулся: трит-запись движка и квантовый декод согласованы
    near(
        re(&mut st, "trit_val(trits(19))"),
        19.0,
        1e-12,
        "round-trip: число → триты → фазы → QFT → число",
    );
}

// ---------------------------------------------------------------------------
// Блоки 12–15 (сессия 4): CGLMP-нарушение локального реализма, кутритный QAOA
// на живом коннектоме (Max-3-Cut, 3^6=729), t5c-мост «кристалл→квант»,
// квантовые прогулки с Loschmidt-эхом. Живые прогоны — download/experiments/.
// ---------------------------------------------------------------------------

/// Настройка CGLMP: |Φ₃⟩ + фазы (α₁=0, α₂=1/2, β₁=1/4, β₂=−1/4) + QFT с
/// взаимно сопряжённых сторон (F⊗F†) + проекторы классов (A−B) mod 3.
fn cglmp_setup(st: &mut CalcState) {
    for line in [
        "let psiB = (1/sqrt(3)) * [1;0;0;0;1;0;0;0;1]",
        // фазы измерений статьи Collins et al. 2002 (d=3)
        "let A2p = [1,0,0; 0,exp(pi/3*i),0; 0,0,exp(2*pi/3*i)]",
        "let B1p = [1,0,0; 0,exp(pi/6*i),0; 0,0,exp(pi/3*i)]",
        "let B2p = [1,0,0; 0,exp(-pi/6*i),0; 0,0,exp(-pi/3*i)]",
        // ВАЖНО (грабли сессии 4): F⊗F†, НЕ F⊗F — иначе разностные корреляции
        // состояния Белла размываются в плоские 1/3 и I3 = 0
        "let FF = kron(F3, dagger(F3))",
        "let s11 = FF * kron(eye(3), B1p) * psiB",
        "let s12 = FF * kron(eye(3), B2p) * psiB",
        "let s21 = FF * kron(A2p, B1p) * psiB",
        "let s22 = FF * kron(A2p, B2p) * psiB",
        // проекторы классов c = (A−B) mod 3
        "let Pc0 = kron(e0,e0)*transpose(kron(e0,e0)) + kron(e1,e1)*transpose(kron(e1,e1)) + kron(e2,e2)*transpose(kron(e2,e2))",
        "let Pcp = kron(e1,e0)*transpose(kron(e1,e0)) + kron(e2,e1)*transpose(kron(e2,e1)) + kron(e0,e2)*transpose(kron(e0,e2))",
        "let Pcm = kron(e2,e0)*transpose(kron(e2,e0)) + kron(e0,e1)*transpose(kron(e0,e1)) + kron(e1,e2)*transpose(kron(e1,e2))",
    ] {
        st.eval_line(line).unwrap_or_else(|e| panic!("{line}: {e}"));
    }
}

#[test]
fn cglmp_violation_of_local_realism() {
    let mut st = lab();
    cglmp_setup(&mut st);
    // q0 = P(A1=B1) = (4+2√3)/9 — вероятность «согласия» оптимальной пары
    near(
        re(&mut st, "trace(dagger(s11)*Pc0*s11)"),
        (4.0 + 2.0 * 3.0_f64.sqrt()) / 9.0,
        1e-12,
        "CGLMP q0 = (4+2√3)/9",
    );
    // q-1 = P(A1=B1−1) = 1/9
    near(
        re(&mut st, "trace(dagger(s11)*Pcm*s11)"),
        1.0 / 9.0,
        1e-12,
        "CGLMP q-1 = 1/9",
    );
    // СИММЕТРИЯ correlP статьи: 4 равенства вероятностей
    near(
        re(&mut st, "trace(dagger(s21)*Pcm*s21) - trace(dagger(s11)*Pc0*s11)"),
        0.0,
        1e-12,
        "P(B1=A2+1) = q0",
    );
    near(
        re(&mut st, "trace(dagger(s12)*Pcp*s12) - trace(dagger(s11)*Pcm*s11)"),
        0.0,
        1e-12,
        "P(B2=A1-1) = q-1",
    );
    // ГЛАВНОЕ: I3 = 4(q0 − q-1) = (4/9)(3+2√3) ≈ 2.872935 > 2 (локальный предел)
    let i3 = re(
        &mut st,
        "trace(dagger(s11)*Pc0*s11) + trace(dagger(s21)*Pcm*s21) + trace(dagger(s22)*Pc0*s22) + trace(dagger(s12)*Pc0*s12) - trace(dagger(s11)*Pcm*s11) - trace(dagger(s21)*Pc0*s21) - trace(dagger(s22)*Pcm*s22) - trace(dagger(s12)*Pcp*s12)",
    );
    near(i3, (4.0 / 9.0) * (3.0 + 2.0 * 3.0_f64.sqrt()), 1e-12, "I3(QM)");
    assert!(i3 > 2.0, "нарушение локального реализма: I3 = {i3} > 2");
    // Шумовой порог: rho(eta*) = eta|Фи><Фи| + (1-eta)I/9, I3(eta*) = 2 ровно.
    // Сторона сопряжения (грабли): trace(Pc*FF*r*FF†), НЕ trace(FF†*r*FF*Pc)
    st.eval_line("let rho = psiB*transpose(psiB)").unwrap();
    st.eval_line("let etas = 2/((4/9)*(3 + 2*sqrt(3)))").unwrap();
    st.eval_line("let rho1 = etas*rho + (1-etas)*(1/9)*eye(9)").unwrap();
    st.eval_line("let r11 = kron(eye(3),B1p)*rho1*dagger(kron(eye(3),B1p))").unwrap();
    st.eval_line("let r21 = kron(A2p,B1p)*rho1*dagger(kron(A2p,B1p))").unwrap();
    st.eval_line("let r22 = kron(A2p,B2p)*rho1*dagger(kron(A2p,B2p))").unwrap();
    st.eval_line("let r12 = kron(eye(3),B2p)*rho1*dagger(kron(eye(3),B2p))").unwrap();
    st.eval_line("let a11 = FF*r11*dagger(FF)").unwrap();
    st.eval_line("let a21 = FF*r21*dagger(FF)").unwrap();
    st.eval_line("let a22 = FF*r22*dagger(FF)").unwrap();
    st.eval_line("let a12 = FF*r12*dagger(FF)").unwrap();
    near(
        re_part(
            &mut st,
            "trace(Pc0*a11) + trace(Pcm*a21) + trace(Pc0*a22) + trace(Pc0*a12) - trace(Pcm*a11) - trace(Pc0*a21) - trace(Pcm*a22) - trace(Pcp*a12)",
        ),
        2.0,
        1e-10,
        "I3(eta*) = 2 — критическая видимость шума",
    );
}

/// Оптимум Ацина (сессия 6): НЕкомпактное состояние
/// |Ψ_mv⟩ = (|00⟩+γ|11⟩+|22⟩)/√(2+γ²), γ=(√11−√3)/2 ≈ 0.792287,
/// СО СТАНДАРТНЫМИ фазовыми лестницами CGLMP даёт точный максимум
/// I₃ = 1+√(11/3) ≈ 2.914854 — Acín-Durt-Gisin-Latorre (quant-ph/0111143).
/// DE-поиск сессии 6 переоткрыл это состояние независимо (γ = 0.792287
/// с 4 знаками) до сверки со статьёй. Четыре пути проверки: контур CGLMP,
/// оператор Белла == статье (позлементно), собственное уравнение, шум.
#[test]
fn acin_optimum_nonmaximally_entangled() {
    let mut st = lab();
    cglmp_setup(&mut st);
    for line in [
        "let gam = (sqrt(11)-sqrt(3))/2",
        "let nn = 2 + gam^2",
        "let mv = (1/sqrt(nn)) * [1;0;0;0;gam;0;0;0;1]",
        "let s11m = FF * kron(eye(3), B1p) * mv",
        "let s12m = FF * kron(eye(3), B2p) * mv",
        "let s21m = FF * kron(A2p, B1p) * mv",
        "let s22m = FF * kron(A2p, B2p) * mv",
        "let M11 = kron(eye(3), B1p)",
        "let M12 = kron(eye(3), B2p)",
        "let M21 = kron(A2p, B1p)",
        "let M22 = kron(A2p, B2p)",
        // оператор Белла из примитивов движка: B = Σ±(A⊗B)†·F F†·Pc·F F·(A⊗B)
        "let Bop = dagger(M11)*dagger(FF)*Pc0*FF*M11 + dagger(M21)*dagger(FF)*Pcm*FF*M21 + dagger(M22)*dagger(FF)*Pc0*FF*M22 + dagger(M12)*dagger(FF)*Pc0*FF*M12 - dagger(M11)*dagger(FF)*Pcm*FF*M11 - dagger(M21)*dagger(FF)*Pc0*FF*M21 - dagger(M22)*dagger(FF)*Pcm*FF*M22 - dagger(M12)*dagger(FF)*Pcp*FF*M12",
        // оператор Белла из статьи (quant-ph/0111143, Eq. bellop) — литерал
        "let Bpaper = [0,0,0,0,2/sqrt(3),0,0,0,2; 0,0,0,0,0,2/sqrt(3),0,0,0; 0,0,0,0,0,0,0,0,0; 0,0,0,0,0,0,0,2/sqrt(3),0; 2/sqrt(3),0,0,0,0,0,0,0,2/sqrt(3); 0,2/sqrt(3),0,0,0,0,0,0,0; 0,0,0,0,0,0,0,0,0; 0,0,0,2/sqrt(3),0,0,0,0,0; 2,0,0,0,2/sqrt(3),0,0,0,0]",
    ] {
        st.eval_line(line).unwrap_or_else(|e| panic!("{line}: {e}"));
    }
    // 1) контур CGLMP: I₃ = 1+√(11/3) — точный литературный максимум d=3
    let i3 = re(
        &mut st,
        "trace(dagger(s11m)*Pc0*s11m) + trace(dagger(s21m)*Pcm*s21m) + trace(dagger(s22m)*Pc0*s22m) + trace(dagger(s12m)*Pc0*s12m) - trace(dagger(s11m)*Pcm*s11m) - trace(dagger(s21m)*Pc0*s21m) - trace(dagger(s22m)*Pcm*s22m) - trace(dagger(s12m)*Pcp*s12m)",
    );
    near(i3, 1.0 + (11.0_f64 / 3.0).sqrt(), 1e-12, "I₃(Ацин) = 1+√(11/3)");
    // превышает и локальный предел (2), и компактный максимум Белла
    assert!(i3 > 2.0, "нарушение локального реализма: {i3} > 2");
    assert!(
        i3 > (4.0 / 9.0) * (3.0 + 2.0 * 3.0_f64.sqrt()),
        "некомпактное состояние бьёт компактный максимум 2.8729"
    );
    // 2) оператор Белла движка == оператору статьи позлементно
    near(
        zero_norm(&mut st, "Bop-Bpaper"),
        0.0,
        1e-24,
        "Bop == Bpaper (статья, Eq. bellop)",
    );
    // 3) Rayleigh и собственное уравнение: |Ψ_mv⟩ — собственный вектор B
    near(
        re_part(&mut st, "trace(dagger(mv)*Bop*mv)"),
        1.0 + (11.0_f64 / 3.0).sqrt(),
        1e-12,
        "⟨Ψ|B|Ψ⟩ = λ_max (Rayleigh)",
    );
    near(
        modulus(&mut st, "Bop*mv - (1+sqrt(11/3))*mv"),
        0.0,
        1e-12,
        "B·Ψ = λ·Ψ — собственный вектор (норма невязки)",
    );
    // 4) шумовой порог: η* = 2/(1+√(11/3)) → I₃(η*) = 2 ровно;
    //    η*(Ацин) < η*(Белл): некомпактное состояние ТОЛЕРАНТНЕЕ к шуму
    st.eval_line("let etam = 2/(1+sqrt(11/3))").unwrap();
    st.eval_line("let rhom = mv*transpose(mv)").unwrap();
    st.eval_line("let rhom1 = etam*rhom + (1-etam)*(1/9)*eye(9)").unwrap();
    for (nm, m) in [("r11m", "M11"), ("r12m", "M12"), ("r21m", "M21"), ("r22m", "M22")] {
        st.eval_line(&format!("let {nm} = {m}*rhom1*dagger({m})")).unwrap();
        st.eval_line(&format!("let a{nm} = FF*{nm}*dagger(FF)")).unwrap();
    }
    near(
        re_part(
            &mut st,
            "trace(Pc0*ar11m) + trace(Pcm*ar21m) + trace(Pc0*ar22m) + trace(Pc0*ar12m) - trace(Pcm*ar11m) - trace(Pc0*ar21m) - trace(Pcm*ar22m) - trace(Pcp*ar12m)",
        ),
        2.0,
        1e-10,
        "I₃(η*_Ацин) = 2 — критическая видимость",
    );
    near(
        re(&mut st, "2/(1+sqrt(11/3))"),
        2.0 / (1.0 + (11.0_f64 / 3.0).sqrt()),
        1e-12,
        "η* = 2/(1+√(11/3)) ≈ 0.686141 < η*(Белл) = 0.696152",
    );
}

// ---------------------------------------------------------------------------
// Блоки 16–17 (сессия 7): CGLMP d=4 — КВКВАРТЫ. Локальный предел 2 (брутфорс
// 256 стратегий), max-ent = (2/3)(√2+√(10−√2)) (статья Collins et al. 2001),
// некомпактный оптимум I₄* = 2.972698267102 на палиндроме (1,γ,γ,1)/√(2+2γ²),
// где γ₄ — корень квадратики √2(1+√(2−√2))γ² + 2√(2−√2)γ − √2(1+√(2−√2)) = 0
// над √2-башней. DE (12 DOF) + собственный вектор B₄ нашли его независимо
// до вывода алгебры. I₄* > I₃*(Ацина): квкварт бьёт кутрит; η* меньше —
// терпеливее к шуму. Живой прогон: download/experiments/d4_ququart_v063.txt.
// ---------------------------------------------------------------------------

/// Настройка CGLMP d=4: |Φ₄⟩ + линейные лестницы статьи (α₂=1/2, β₁=1/4,
/// β₂=−1/4; DE сессии 7 подтвердил их оптимальность и для некомпакта)
/// + QFT с сопряжённых сторон F₄⊗F₄† + проекторы 4 классов (A−B) mod 4.
fn cglmp4_setup(st: &mut CalcState) {
    for line in [
        "let F4 = (1/2)*[1,1,1,1; 1,i,-1,-i; 1,-1,1,-1; 1,-i,-1,i]",
        "let u0 = [1;0;0;0]",
        "let u1 = [0;1;0;0]",
        "let u2 = [0;0;1;0]",
        "let u3 = [0;0;0;1]",
        // ВАЖНО (грабли сессии 4, d=4 издание): F₄⊗F₄†, НЕ F₄⊗F₄
        "let FF4 = kron(F4, dagger(F4))",
        "let A2p = [1,0,0,0; 0,exp(pi/4*i),0,0; 0,0,exp(pi/2*i),0; 0,0,0,exp(3*pi/4*i)]",
        "let B1p = [1,0,0,0; 0,exp(pi/8*i),0,0; 0,0,exp(pi/4*i),0; 0,0,0,exp(3*pi/8*i)]",
        "let B2p = [1,0,0,0; 0,exp(-pi/8*i),0,0; 0,0,exp(-pi/4*i),0; 0,0,0,exp(-3*pi/8*i)]",
        // проекторы классов c = (A−B) mod 4 (ранг 4 в 16-мерии)
        "let Qc0 = kron(u0,u0)*transpose(kron(u0,u0)) + kron(u1,u1)*transpose(kron(u1,u1)) + kron(u2,u2)*transpose(kron(u2,u2)) + kron(u3,u3)*transpose(kron(u3,u3))",
        "let Qc1 = kron(u1,u0)*transpose(kron(u1,u0)) + kron(u2,u1)*transpose(kron(u2,u1)) + kron(u3,u2)*transpose(kron(u3,u2)) + kron(u0,u3)*transpose(kron(u0,u3))",
        "let Qc2 = kron(u2,u0)*transpose(kron(u2,u0)) + kron(u3,u1)*transpose(kron(u3,u1)) + kron(u0,u2)*transpose(kron(u0,u2)) + kron(u1,u3)*transpose(kron(u1,u3))",
        "let Qc3 = kron(u3,u0)*transpose(kron(u3,u0)) + kron(u0,u1)*transpose(kron(u0,u1)) + kron(u1,u2)*transpose(kron(u1,u2)) + kron(u2,u3)*transpose(kron(u2,u3))",
        "let M11 = kron(eye(4), B1p)",
        "let M12 = kron(eye(4), B2p)",
        "let M21 = kron(A2p, B1p)",
        "let M22 = kron(A2p, B2p)",
        "let psiB4 = (1/2)*[1;0;0;0;0;1;0;0;0;0;1;0;0;0;0;1]",
    ] {
        st.eval_line(line).unwrap_or_else(|e| panic!("{line}: {e}"));
    }
}

#[test]
fn cglmp_ququart_d4_violation() {
    let mut st = lab();
    cglmp4_setup(&mut st);
    // (1) ЛОКАЛЬНЫЙ ПРЕДЕЛ: брутфорс всех 256 детерминированных стратегий
    // (a1,a2,b1,b2 ∈ Z4) по формуле Id статьи — максимум РОВНО 2.
    let mut best = f64::NEG_INFINITY;
    for a1 in 0..4i32 {
        for a2 in 0..4 {
            for b1 in 0..4 {
                for b2 in 0..4 {
                    let p = |x: i32, y: i32, c: i32| -> f64 {
                        if (x - y).rem_euclid(4) == c {
                            1.0
                        } else {
                            0.0
                        }
                    };
                    let (p11, p12, p21, p22) = (
                        |c: i32| p(a1, b1, c),
                        |c: i32| p(a1, b2, c),
                        |c: i32| p(a2, b1, c),
                        |c: i32| p(a2, b2, c),
                    );
                    let v = (p11(0) + p21(3) + p22(0) + p12(0) - p11(3) - p21(0) - p22(3)
                        - p12(1))
                        + (1.0 / 3.0)
                            * (p11(1) + p21(2) + p22(1) + p12(3) - p11(2) - p21(1) - p22(2)
                                - p12(2));
                    best = best.max(v);
                }
            }
        }
    }
    near(best, 2.0, 1e-12, "локальный предел I4 = 2 (брутфорс 256 стратегий)");
    // (2) измеренные состояния max-ent
    for line in [
        "let me11 = FF4*M11*psiB4",
        "let me12 = FF4*M12*psiB4",
        "let me21 = FF4*M21*psiB4",
        "let me22 = FF4*M22*psiB4",
    ] {
        st.eval_line(line).unwrap_or_else(|e| panic!("{line}: {e}"));
    }
    // (3) q0 = P(A1=B1) = 1/(8(2−√(2+√2))) = 1/(2d²sin²(π/16)) — аналитика статьи
    near(
        re(&mut st, "trace(dagger(me11)*Qc0*me11)"),
        1.0 / (8.0 * (2.0 - (2.0 + 2.0_f64.sqrt()).sqrt())),
        1e-12,
        "CGLMP d=4 q0 = 1/(8(2−√(2+√2)))",
    );
    // (4) ГЛАВНОЕ: I4(max-ent) = (2/3)(√2+√(10−√2)) ≈ 2.896243 > 2 — статья, точно
    let i4me = re(
        &mut st,
        "trace(dagger(me11)*Qc0*me11) + trace(dagger(me21)*Qc3*me21) + trace(dagger(me22)*Qc0*me22) + trace(dagger(me12)*Qc0*me12) - trace(dagger(me11)*Qc3*me11) - trace(dagger(me21)*Qc0*me21) - trace(dagger(me22)*Qc3*me22) - trace(dagger(me12)*Qc1*me12) + (1/3)*(trace(dagger(me11)*Qc1*me11) + trace(dagger(me21)*Qc2*me21) + trace(dagger(me22)*Qc1*me22) + trace(dagger(me12)*Qc3*me12) - trace(dagger(me11)*Qc2*me11) - trace(dagger(me21)*Qc1*me21) - trace(dagger(me22)*Qc2*me22) - trace(dagger(me12)*Qc2*me12))",
    );
    near(
        i4me,
        (2.0 / 3.0) * (2.0_f64.sqrt() + (10.0 - 2.0_f64.sqrt()).sqrt()),
        1e-11,
        "I4(max-ent) = (2/3)(√2+√(10−√2)) — Collins et al. 2001",
    );
    assert!(i4me > 2.0, "нарушение локального реализма: I4 = {i4me} > 2");
    // (5) d=4 max-ent УСТУПАЕТ оптимуму Ацина d=3: интрига некомпакта
    assert!(
        i4me < 1.0 + (11.0_f64 / 3.0).sqrt(),
        "max-ent d=4 ({i4me}) < Ацин d=3 (2.914854) — некомпакт обязан побить"
    );
}

/// Оптимум «Ацина для квквартов» (сессия 7): палиндромное состояние
/// |Ψ₄⟩ = (|00⟩+γ|11⟩+γ|22⟩+|33⟩)/√(2+2γ²) с линейными лестницами статьи.
/// γ₄ = [√(8−3√2+4√(2−√2)) − √(2−√2)]/[√2(1+√(2−√2))] — корень квадратики
/// √2(1+t)γ² + 2tγ − √2(1+t) = 0, t = √(2−√2). DE+eig переоткрыли 0.7393724
/// до вывода алгебры. Четыре пути: контур, Rayleigh, собственное уравнение,
/// шумовой порог. I₄* = 2.972698267102 > I₃*(Ацина) = 2.914854216.
#[test]
fn ququart_acin_optimum_d4() {
    let mut st = lab();
    cglmp4_setup(&mut st);
    for line in [
        "let gam4 = (sqrt(8 - 3*sqrt(2) + 4*sqrt(2-sqrt(2))) - sqrt(2-sqrt(2))) / (sqrt(2)*(1+sqrt(2-sqrt(2))))",
        "let nn4 = 2 + 2*gam4^2",
        "let mv4 = (1/sqrt(nn4))*[1;0;0;0;0;gam4;0;0;0;0;gam4;0;0;0;0;1]",
        "let s11 = FF4*M11*mv4",
        "let s12 = FF4*M12*mv4",
        "let s21 = FF4*M21*mv4",
        "let s22 = FF4*M22*mv4",
        // оператор Белла B₄ из примитивов движка (16 членов, CSE кеширует)
        "let Bop4 = dagger(M11)*dagger(FF4)*Qc0*FF4*M11 + dagger(M21)*dagger(FF4)*Qc3*FF4*M21 + dagger(M22)*dagger(FF4)*Qc0*FF4*M22 + dagger(M12)*dagger(FF4)*Qc0*FF4*M12 - dagger(M11)*dagger(FF4)*Qc3*FF4*M11 - dagger(M21)*dagger(FF4)*Qc0*FF4*M21 - dagger(M22)*dagger(FF4)*Qc3*FF4*M22 - dagger(M12)*dagger(FF4)*Qc1*FF4*M12 + (1/3)*(dagger(M11)*dagger(FF4)*Qc1*FF4*M11 + dagger(M21)*dagger(FF4)*Qc2*FF4*M21 + dagger(M22)*dagger(FF4)*Qc1*FF4*M22 + dagger(M12)*dagger(FF4)*Qc3*FF4*M12 - dagger(M11)*dagger(FF4)*Qc2*FF4*M11 - dagger(M21)*dagger(FF4)*Qc1*FF4*M21 - dagger(M22)*dagger(FF4)*Qc2*FF4*M22 - dagger(M12)*dagger(FF4)*Qc2*FF4*M12)",
    ] {
        st.eval_line(line).unwrap_or_else(|e| panic!("{line}: {e}"));
    }
    // 1) γ₄ — точный корень квадратики над √2-башней
    near(
        re(&mut st, "gam4"),
        0.7393724305634157,
        1e-13,
        "γ₄ = [√(8−3√2+4√(2−√2)) − √(2−√2)]/[√2(1+√(2−√2))]",
    );
    // 2) контур CGLMP: I₄* = 2.972698267102 — точная алгебра сессии 7
    let i4 = re(
        &mut st,
        "trace(dagger(s11)*Qc0*s11) + trace(dagger(s21)*Qc3*s21) + trace(dagger(s22)*Qc0*s22) + trace(dagger(s12)*Qc0*s12) - trace(dagger(s11)*Qc3*s11) - trace(dagger(s21)*Qc0*s21) - trace(dagger(s22)*Qc3*s22) - trace(dagger(s12)*Qc1*s12) + (1/3)*(trace(dagger(s11)*Qc1*s11) + trace(dagger(s21)*Qc2*s21) + trace(dagger(s22)*Qc1*s22) + trace(dagger(s12)*Qc3*s12) - trace(dagger(s11)*Qc2*s11) - trace(dagger(s21)*Qc1*s21) - trace(dagger(s22)*Qc2*s22) - trace(dagger(s12)*Qc2*s12))",
    );
    near(i4, 2.9726982671022437, 1e-11, "I₄*(квкварт) — точная алгебра");
    // превышает локальный предел, max-ent И оптимум Ацина d=3
    assert!(i4 > 2.0, "нарушение локального реализма: {i4} > 2");
    assert!(
        i4 > (2.0 / 3.0) * (2.0_f64.sqrt() + (10.0 - 2.0_f64.sqrt()).sqrt()),
        "некомпактное состояние бьёт max-ent d=4 (2.896243)"
    );
    assert!(
        i4 > 1.0 + (11.0_f64 / 3.0).sqrt(),
        "КВКВАРТ БЬЁТ КУТРИТ: I₄* ({i4}) > I₃*(Ацина) = 2.914854"
    );
    // 3) Rayleigh и собственное уравнение: |Ψ₄⟩ — собственный вектор B₄
    near(
        re_part(&mut st, "trace(dagger(mv4)*Bop4*mv4)"),
        2.9726982671022437,
        1e-11,
        "⟨Ψ|B₄|Ψ⟩ = λ_max (Rayleigh)",
    );
    near(
        modulus(&mut st, "abs(Bop4*mv4 - 2.9726982671022437*mv4)"),
        0.0,
        1e-10,
        "B₄·Ψ = λ·Ψ — собственный вектор (норма невязки)",
    );
    // 4) шумовой порог: η* = 2/I₄* → I₄(η*) = 2 ровно;
    //    η*(d=4) < η*(Ацин d=3): квквартовый некомпакт ТОЛЕРАНТНЕЕ к шуму
    st.eval_line("let eta4 = 2/2.9726982671022437").unwrap();
    st.eval_line("let rho4 = mv4*transpose(mv4)").unwrap();
    st.eval_line("let rho41 = eta4*rho4 + (1-eta4)*(1/16)*eye(16)").unwrap();
    for (nm, m) in [("r11", "M11"), ("r12", "M12"), ("r21", "M21"), ("r22", "M22")] {
        st.eval_line(&format!("let {nm} = {m}*rho41*dagger({m})")).unwrap();
        st.eval_line(&format!("let a{nm} = FF4*{nm}*dagger(FF4)")).unwrap();
    }
    near(
        re_part(
            &mut st,
            "trace(Qc0*ar11) + trace(Qc3*ar21) + trace(Qc0*ar22) + trace(Qc0*ar12) - trace(Qc3*ar11) - trace(Qc0*ar21) - trace(Qc3*ar22) - trace(Qc1*ar12) + (1/3)*(trace(Qc1*ar11) + trace(Qc2*ar21) + trace(Qc1*ar22) + trace(Qc3*ar12) - trace(Qc2*ar11) - trace(Qc1*ar21) - trace(Qc2*ar22) - trace(Qc2*ar12))",
        ),
        2.0,
        1e-9,
        "I₄(η*_квкварт) = 2 — критическая видимость",
    );
    near(
        re(&mut st, "eta4"),
        2.0 / 2.9726982671022437,
        1e-12,
        "η*(d=4) = 0.672789 < η*(Ацин d=3) = 0.686141 — терпеливее к шуму",
    );
    assert!(
        2.0 / 2.9726982671022437 < 2.0 / (1.0 + (11.0_f64 / 3.0).sqrt()),
        "η*(d=4) < η*(Ацин d=3): квкварт терпеливее к шуму"
    );
}

/// Настройка QAOA: 6 кутритов K4-ядра коннектома мухи (реальные веса).
fn qaoa_setup(st: &mut CalcState) {
    for line in [
        "let I81 = kron(eye(9), eye(9))",
        "let flat = kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1],(1/sqrt(3))*[1;1;1])))))",
        // проекторы равенства EQU_uv (след 243 = ранг 3 в 729-мерии)
        "let EQU01 = kron(P0,kron(P0,I81)) + kron(P1,kron(P1,I81)) + kron(P2,kron(P2,I81))",
        "let EQU02 = kron(P0,kron(eye(3),kron(P0,eye(27)))) + kron(P1,kron(eye(3),kron(P1,eye(27)))) + kron(P2,kron(eye(3),kron(P2,eye(27))))",
        "let EQU04 = kron(P0,kron(eye(9),kron(P0,eye(9)))) + kron(P1,kron(eye(9),kron(P1,eye(9)))) + kron(P2,kron(eye(9),kron(P2,eye(9))))",
        "let EQU05 = kron(P0,kron(eye(27),kron(P0,eye(3)))) + kron(P1,kron(eye(27),kron(P1,eye(3)))) + kron(P2,kron(eye(27),kron(P2,eye(3))))",
        "let EQU12 = kron(eye(3),kron(P0,kron(P0,eye(27)))) + kron(eye(3),kron(P1,kron(P1,eye(27)))) + kron(eye(3),kron(P2,kron(P2,eye(27))))",
        "let EQU13 = kron(eye(3),kron(P0,kron(eye(3),kron(P0,eye(9))))) + kron(eye(3),kron(P1,kron(eye(3),kron(P1,eye(9))))) + kron(eye(3),kron(P2,kron(eye(3),kron(P2,eye(9)))))",
        "let EQU14 = kron(eye(3),kron(P0,kron(eye(9),kron(P0,eye(3))))) + kron(eye(3),kron(P1,kron(eye(9),kron(P1,eye(3))))) + kron(eye(3),kron(P2,kron(eye(9),kron(P2,eye(3)))))",
        "let EQU15 = kron(eye(3),kron(P0,kron(eye(27),P0))) + kron(eye(3),kron(P1,kron(eye(27),P1))) + kron(eye(3),kron(P2,kron(eye(27),P2)))",
        "let EQU24 = kron(eye(9),kron(P0,kron(eye(3),kron(P0,eye(3))))) + kron(eye(9),kron(P1,kron(eye(3),kron(P1,eye(3))))) + kron(eye(9),kron(P2,kron(eye(3),kron(P2,eye(3)))))",
        "let EQU25 = kron(eye(9),kron(P0,kron(eye(9),P0))) + kron(eye(9),kron(P1,kron(eye(9),P1))) + kron(eye(9),kron(P2,kron(eye(9),P2)))",
        "let EQU45 = kron(I81,kron(P0,P0)) + kron(I81,kron(P1,P1)) + kron(I81,kron(P2,P2))",
        // эрмитов миксер: X3 + X3† (грабли сессии 3: X† ≠ X)
        "let G3 = X3 + dagger(X3)",
    ] {
        st.eval_line(line).unwrap_or_else(|e| panic!("{line}: {e}"));
    }
}

#[test]
fn qutrit_qaoa_max3cut_on_fly_connectome() {
    let mut st = lab();
    qaoa_setup(&mut st);
    // (1) ранги проекторов: каждое EQU_uv — ранг 3 в 729-мерии (след 243)
    for e in ["EQU01", "EQU24", "EQU45"] {
        near(re(&mut st, &format!("trace({e})")), 243.0, 1e-9, "след EQU");
    }
    // (2) миксер унитарен: trace(M†M) = 3, det(M) = 1 (G3 бесследов)
    st.eval_line("let M1 = expm(-0.9*i*G3)").unwrap();
    near(re(&mut st, "trace(dagger(M1)*M1)"), 3.0, 1e-9, "миксер унитарен");
    near(re_part(&mut st, "det(M1)"), 1.0, 1e-9, "det миксера");
    // (3) оптимальное назначение x=16 (гаngи {3,4,5}|{0,2}|{1}) режет 4403 из 4435 —
    // индикатор [c_u≠c_v] = ceil(|c_u−c_v|/2); sign(0)=1 в Калькуляторе (грабли!)
    near(
        re(&mut st, "735*ceil(abs(1-0)/2) + 24*ceil(abs(1-1)/2) + 1496*ceil(abs(1-0)/2) + 1222*ceil(abs(1-0)/2) + 251*ceil(abs(2-1)/2) + 231*ceil(abs(2-0)/2) + 206*ceil(abs(2-0)/2) + 194*ceil(abs(2-0)/2) + 40*ceil(abs(1-0)/2) + 28*ceil(abs(1-0)/2) + 8*ceil(abs(0-0)/2)"),
        4403.0,
        1e-9,
        "Max-3-Cut оптимум 4403/4435 (x=16)",
    );
    // (4) QAOA p=1 при (γ,β)=(0.9,0.9): E[cut'] = 2.363073 (норм.), ~80% оптимума.
    // Линейная цепочка переприсвоек s = ... — НЕ вложенные выражения (иначе 2^11)
    st.eval_line("let s = flat").unwrap();
    for (w, n) in [
        (735.0 / 1496.0, "EQU01"), (24.0 / 1496.0, "EQU02"), (1.0, "EQU04"),
        (1222.0 / 1496.0, "EQU05"), (251.0 / 1496.0, "EQU12"), (231.0 / 1496.0, "EQU13"),
        (206.0 / 1496.0, "EQU14"), (194.0 / 1496.0, "EQU15"), (40.0 / 1496.0, "EQU24"),
        (28.0 / 1496.0, "EQU25"), (8.0 / 1496.0, "EQU45"),
    ] {
        st.eval_line(&format!("let s = exp(0.9*{w}*i)*s + (1 - exp(0.9*{w}*i))*({n}*s)")).unwrap();
    }
    st.eval_line("let s = kron(expm(-0.9*i*G3), kron(expm(-0.9*i*G3), kron(expm(-0.9*i*G3), kron(expm(-0.9*i*G3), kron(expm(-0.9*i*G3), expm(-0.9*i*G3))))))*s").unwrap();
    near(re(&mut st, "trace(dagger(s)*s)"), 1.0, 1e-9, "норма QAOA-состояния");
    let ec = [
        (735.0 / 1496.0, "EQU01"), (24.0 / 1496.0, "EQU02"), (1.0, "EQU04"),
        (1222.0 / 1496.0, "EQU05"), (251.0 / 1496.0, "EQU12"), (231.0 / 1496.0, "EQU13"),
        (206.0 / 1496.0, "EQU14"), (194.0 / 1496.0, "EQU15"), (40.0 / 1496.0, "EQU24"),
        (28.0 / 1496.0, "EQU25"), (8.0 / 1496.0, "EQU45"),
    ]
    .iter()
    .map(|(w, n)| format!("{w}*(1 - trace(dagger(s)*{n}*s))"))
    .collect::<Vec<_>>()
    .join(" + ");
    near(re(&mut st, &ec), 2.363073, 1e-4, "E[cut] QAOA p=1 (γ=β=0.9)");
}

#[test]
fn t5c_crystal_to_quantum_bridge() {
    let mut st = lab();
    // Живые триты строки «мысль» кристалла permanent_memory.t5c: [-1,1,-1,-1,-1,-1]
    // (захардкожены для герметичности теста; парсер — scripts/t5c_bridge_session.sh)
    for line in [
        "let F729 = kron(F3, kron(F3, kron(F3, kron(F3, kron(F3, F3)))))",
        "let flat6 = kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1],(1/sqrt(3))*[1;1;1])))))",
        // t=−1 → diag(1, ω², ω⁴); t=+1 → diag(1, ω, ω²) (грабли: t=1 — фаза om, не 1!)
        "let D1 = kron([1,0,0;0,om^2,0;0,0,om^4], kron([1,0,0;0,om,0;0,0,om^2], kron([1,0,0;0,om^2,0;0,0,om^4], kron([1,0,0;0,om^2,0;0,0,om^4], kron([1,0,0;0,om^2,0;0,0,om^4],[1,0,0;0,om^2,0;0,0,om^4])))))",
        "let D2 = kron([1,0,0;0,om,0;0,0,om^2], kron([1,0,0;0,om,0;0,0,om^2], kron([1,0,0;0,om^2,0;0,0,om^4], kron([1,0,0;0,om^2,0;0,0,om^4], kron([1,0,0;0,om^2,0;0,0,om^4],[1,0,0;0,om^2,0;0,0,om^4])))))",
        // декод: |−t mod 3⟩: «мысль» → |1,2,1,1,1,1⟩; «кристалл» → |2,2,1,1,1,1⟩
        "let dec1 = kron(e1, kron(e2, kron(e1, kron(e1, kron(e1, e1)))))",
        "let dec2 = kron(e2, kron(e2, kron(e1, kron(e1, kron(e1, e1)))))",
    ] {
        st.eval_line(line).unwrap_or_else(|e| panic!("{line}: {e}"));
    }
    // Мост без потерь: F·D·flat = |−t⟩ с машинной точностью
    near(
        zero_norm(&mut st, "F729*D1*flat6 - dec1"),
        0.0,
        1e-24,
        "t5c-мост «мысль»: F·D·flat = |−t⟩",
    );
    near(
        zero_norm(&mut st, "F729*D2*flat6 - dec2"),
        0.0,
        1e-24,
        "t5c-мост «кристалл»: F·D·flat = |−t⟩",
    );
    // Born-вероятность декода = 1
    near(
        modulus(&mut st, "trace(dagger(dec1)*(F729*D1*flat6))"),
        1.0,
        1e-10,
        "P(декод «мысль») = 1",
    );
    // Ортогональность двух мыслей
    near(
        modulus(&mut st, "trace(dagger(F729*D1*flat6)*(F729*D2*flat6))"),
        0.0,
        1e-10,
        "мысли ортогональны",
    );
    // Суперпозиция двух мыслей → Born-коллапс ровно 50/50
    st.eval_line("let sup = F729 * (D1*flat6 + D2*flat6)/sqrt(2)").unwrap();
    near(
        modulus(&mut st, "trace(dagger(dec1)*sup)"),
        1.0 / 2.0_f64.sqrt(),
        1e-10,
        "P(коллапс → «мысль») = 1/2",
    );
    near(
        modulus(&mut st, "trace(dagger(dec2)*sup)"),
        1.0 / 2.0_f64.sqrt(),
        1e-10,
        "P(коллапс → «кристалл») = 1/2",
    );
}

/// «ПАМЯТЬ ВЫБИРАЕТ НЕКОМПАКТНОСТЬ» (сессия 7): кристалл
/// permanent_memory.t5c (8198×8198 тритов) сканирован на 67 182 610
/// 6-тритовых окон; argmax I₃ по собственным окнам памяти:
/// окно [1,0,−1,0,−1,1] (13 вхождений; строки 'and','to','in','system'…)
/// → γ_mem = 578/729 = 0.7928669410 → I₃ = 99.999987% оптимума Ацина
/// (точный γ* = (√11−√3)/2 = 0.7922870 недостижим из 6 тритов).
/// Аналитика семейства Ацина: I₃(γ) = 4(2√3γ+3)/(3(2+γ²)) (сессия 7).
/// Живой прогон: download/experiments/crystal_gamma_v063.txt.
#[test]
fn crystal_chooses_acin_gamma() {
    let mut st = lab();
    cglmp_setup(&mut st);
    // выбор памяти: γ = 578/729 (цифры окна 210102₃ как база-3 дробь)
    st.eval_line("let gmem = 578/729").unwrap();
    st.eval_line("let nnm = 2 + gmem^2").unwrap();
    st.eval_line("let mvm = (1/sqrt(nnm)) * [1;0;0;0;gmem;0;0;0;1]").unwrap();
    for (nm, m) in [
        ("s11", "kron(eye(3), B1p)"),
        ("s12", "kron(eye(3), B2p)"),
        ("s21", "kron(A2p, B1p)"),
        ("s22", "kron(A2p, B2p)"),
    ] {
        st.eval_line(&format!("let {nm} = FF * {m} * mvm")).unwrap();
    }
    // (1) контур CGLMP для выбранного памятью γ — и сверка с аналитикой
    //     семейства Ацина I₃(γ) = 4(2√3γ+3)/(3(2+γ²))
    let i3m = re(
        &mut st,
        "trace(dagger(s11)*Pc0*s11) + trace(dagger(s21)*Pcm*s21) + trace(dagger(s22)*Pc0*s22) + trace(dagger(s12)*Pc0*s12) - trace(dagger(s11)*Pcm*s11) - trace(dagger(s21)*Pc0*s21) - trace(dagger(s22)*Pcm*s22) - trace(dagger(s12)*Pcp*s12)",
    );
    let i3a = re(&mut st, "4*(2*sqrt(3)*gmem + 3)/(3*(2 + gmem^2))");
    near(i3m, 2.9148538425489474, 1e-11, "I₃(γ_mem) — выбор памяти 578/729");
    near(
        i3m - i3a,
        0.0,
        1e-12,
        "контур == аналитика 4(2√3γ+3)/(3(2+γ²))",
    );
    // (2) память почти на пределе: выше компактного Белла, чуть ниже Ацина
    assert!(
        i3m > (4.0 / 9.0) * (3.0 + 2.0 * 3.0_f64.sqrt()),
        "память выше компактного Белла 2.872934 (+1.46%)"
    );
    assert!(
        i3m < 1.0 + (11.0_f64 / 3.0).sqrt(),
        "зазор до точного оптимума 3.7e-7 — алгебра точнее 6 тритов"
    );
    // (3) МОСТ: те же триты окна [1,0,−1,0,−1,1] → фазовая решётка → QFT
    //     читает их как |−t mod 3⟩ = |2,0,1,0,1,2⟩ с P = 1 («фазы накормлены
    //     тритами кристалла» буквально); грабли сессии 4: t=1 → фаза om
    st.eval_line("let F729 = kron(F3, kron(F3, kron(F3, kron(F3, kron(F3, F3)))))").unwrap();
    st.eval_line("let flat6 = kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1],(1/sqrt(3))*[1;1;1])))))").unwrap();
    st.eval_line("let DMEM = kron([1,0,0;0,om,0;0,0,om^2], kron([1,0,0;0,1,0;0,0,1], kron([1,0,0;0,om^2,0;0,0,om^4], kron([1,0,0;0,1,0;0,0,1], kron([1,0,0;0,om^2,0;0,0,om^4],[1,0,0;0,om,0;0,0,om^2])))))").unwrap();
    st.eval_line("let dec_mem = kron(e2, kron(e0, kron(e1, kron(e0, kron(e1, e2)))))").unwrap();
    near(
        zero_norm(&mut st, "F729*DMEM*flat6 - dec_mem"),
        0.0,
        1e-24,
        "мост: окно памяти [1,0,−1,0,−1,1] → |−t mod 3⟩",
    );
    near(
        modulus(&mut st, "trace(dagger(dec_mem)*(F729*DMEM*flat6))"),
        1.0,
        1e-10,
        "P(декод окна памяти) = 1 — фазы накормлены тритами",
    );
    // (4) γ окна в цифрах: 2/3 + 1/9 + 0/27 + 1/81 + 0/243 + 2/729 = 578/729
    near(
        re(&mut st, "2/3 + 1/9 + 1/81 + 2/729"),
        578.0 / 729.0,
        1e-12,
        "цифры окна 210102₃ = γ_mem",
    );
}

#[test]
fn quantum_walk_loschmidt_revival() {
    let mut st = lab();
    // K4-ядро коннектома мухи, взвешенная смежность /1496, ψ₀ = |хаб⟩
    st.eval_line("let Hf = (1/1496)*[-0,735,24,0,1496,1222; 735,-0,251,231,206,194; 24,251,-0,0,40,28; 0,231,0,-0,0,0; 1496,206,40,0,-0,8; 1222,194,28,0,8,-0]").unwrap();
    st.eval_line("let hub = [1;0;0;0;0;0]").unwrap();
    // унитарность эволюции: норма сохраняется
    for t in [0.5, 1.0, 2.5, 4.0] {
        near(
            re(&mut st, &format!("trace(dagger(schrodinger(Hf, hub, {t}))*schrodinger(Hf, hub, {t}))")),
            1.0,
            1e-9,
            "унитарность прогулки",
        );
    }
    // Сигнал ПОКИДАЕТ хаб (L(1.0) ≈ 0.038 < 0.1)…
    let l1 = re(&mut st, "abs(trace(dagger(hub)*schrodinger(Hf, hub, 1.0)))^2");
    assert!(l1 < 0.1, "L(1.0) = {l1} — сигнал должен почти уйти из хаба");
    // …и РЕВАЙВИТ: L(2.5) ≈ 0.830 > 0.8 — квантовая память сигнала
    let l25 = re(&mut st, "abs(trace(dagger(hub)*schrodinger(Hf, hub, 2.5)))^2");
    assert!(l25 > 0.8, "L(2.5) = {l25} — квантовый ревайвал в хабе");
    near(l25, 0.830, 0.01, "высота ревайвала");
}

// ---------------------------------------------------------------------------
// Сессия-9: суверенный рендер — коннектом и интерференция В ЯЗЫКЕ
// ---------------------------------------------------------------------------

/// Строковый вывод Калькулятора как SVG (Value::Str печатается в
/// кавычках — срезаем первый/последний байт, как в viz_session.sh).
fn svg_of(st: &mut CalcState, expr: &str) -> String {
    let out = st
        .eval_line(expr)
        .unwrap_or_else(|e| panic!("{expr}: {e}"));
    let t = out.trim();
    let t = t.strip_prefix('"').unwrap_or(t);
    let t = t.strip_suffix('"').unwrap_or(t);
    t.to_string()
}

#[test]
fn sovereign_render_fly_connectome_graph() {
    let mut st = lab();
    // K4-ядро коннектома мухи (реальные веса сессии-4, симметричные):
    // 11 рёбер из 15 возможных — силовая укладка прямо в языке.
    st.eval_line(
        "let A = [0,735,24,0,1496,1222; \
                  735,0,251,231,206,194; \
                  24,251,0,0,40,28; \
                  0,231,0,0,0,0; \
                  1496,206,40,0,0,8; \
                  1222,194,28,0,8,0]",
    )
    .unwrap();
    let svg = svg_of(&mut st, "viz_graph(A)");
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("N = 6"));
    assert!(svg.contains("рёбер = 11"));
    assert!(svg.contains("компонент = 1"));
    assert!(svg.contains("Fruchterman"));
    // Дважды — бит-в-бит (золотая спираль, без RNG).
    let svg2 = svg_of(&mut st, "viz_graph(A)");
    assert_eq!(svg, svg2, "укладка детерминирована");
    // Ошибка через язык: не квадратная «смежность».
    assert!(st.eval_line("viz_graph([1,2,3])").is_err());
}

#[test]
fn sovereign_render_double_slit_field_and_surf() {
    let mut st = lab();
    // Двущелевая картина Born В ЯЗЫКЕ: z = cos²(πx/2)·exp(−y²·3),
    // литерал с ФОРМУЛАМИ — значения считает сам движок (x ∈ [−3, 3],
    // y ∈ [−1, 1], сетка 9×13).
    let rows: Vec<String> = (0..9)
        .map(|i| {
            let y = i as f64 / 8.0 * 2.0 - 1.0;
            (0..13)
                .map(|j| {
                    let x = j as f64 / 12.0 * 6.0 - 3.0;
                    // скобки обязательны: (-3)^2 ≠ -3^2 (грабля 22)
                    format!("cos({x:.6}*pi/2)^2*exp(-({y:.6})^2*3)")
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect();
    st.eval_line(&format!("let Z = [{}]", rows.join(";")))
        .unwrap();
    // Одна матрица — два рендера («от перестановки слагаемых сумма
    // не меняется»): изолинии и 3D-поверхность.
    let field = svg_of(&mut st, "viz_field(Z)");
    assert!(field.starts_with("<svg"));
    assert!(field.contains("сетка 9×13"));
    assert!(field.contains("изолинии: 7 уровней"));
    assert!(field.contains("<polyline"));
    let surf = svg_of(&mut st, "viz_surf(Z)");
    assert!(surf.contains("квадов 96")); // (9−1)·(13−1)
    assert!(surf.contains("painter"));
    // Физика прямо в языке: строка y=0 (селектор строки 4) — яркие
    // полосы x = −2, 0, 2 дают |Z| = 1, тёмная x = −1 даёт 0.
    let fringe = |j: usize| {
        let mut col = vec!["0".to_string(); 13];
        col[j] = "1".into();
        format!("abs(trace([0,0,0,0,1,0,0,0,0]*Z*[{}]))", col.join(";"))
    };
    near(re(&mut st, &fringe(6)), 1.0, 1e-9, "яркая полоса x=0");
    near(re(&mut st, &fringe(2)), 1.0, 1e-9, "яркая полоса x=-2");
    near(re(&mut st, &fringe(10)), 1.0, 1e-9, "яркая полоса x=+2");
    near(re(&mut st, &fringe(4)), 0.0, 1e-9, "тёмная полоса x=-1");
}
