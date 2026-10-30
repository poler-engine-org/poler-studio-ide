//! Конформанс протокола AI-инженера (docs/AI_ENGINEER_PROTOCOL.md), v0.93.0.
//!
//! Каждая команда протокола обязана исполняться через `eval_line` —
//! тот же путь, что и `poler-engine --exec "calc …"` / REPL `poler> calc …`.
//! Протокол — публичное обещание движка ИИ-ассистентам автора; этот тест —
//! контракт: регрессия ЛЮБОЙ фазы роняет CI.
//!
//! Почему integration-тест, а не юнит: сценарий протокола — «ИИ выдал
//! команду → человек вставил её в CLI» — проверяется на уровне публичного
//! API сессии калькулятора (парсер + реестр функций + законы + единицы),
//! без доступа к внутренностям модулей.

use poler_engine::calc::CalcState;

fn eval(line: &str) -> String {
    CalcState::new()
        .eval_line(line)
        .unwrap_or_else(|e| panic!("протокол: «{line}» → {e}"))
}

fn eval_err(line: &str) -> String {
    match CalcState::new().eval_line(line) {
        Err(e) => e,
        Ok(v) => panic!("протокол: «{line}» ожидал ошибку, получено: {v}"),
    }
}

// ─── ФАЗА 2: верификация на микро-примерах ─────────────────────────────────

#[test]
fn phase2_pole_is_closed_point() {
    // «calc 1/0 → ∞ без сбоя процесса»: полюс [1:0] — замкнутая точка ℝP¹.
    assert_eq!(eval("1/0"), "∞");
    // симметричное насыщение: −1/0 → −∞
    assert_eq!(eval("-1/0"), "-∞");
}

#[test]
fn phase2_lie_rotor_is_orthogonal() {
    // expm(J·θ), J = [[0,−1],[1,0]]: cos 0.5 = 0.87758…, sin 0.5 = 0.47942…
    let out = eval("expm([0, -1; 1, 0] * 0.5)");
    assert!(out.contains("0.8775825618903728"), "expm = {out}");
    assert!(out.contains("0.4794255386042029"), "expm = {out}");
}

#[test]
fn phase2_sturm_spectral_stability() {
    // tridiag(2,−1,8): λ_min = 2 − 2cos(π/9) = 0.12061475842816094
    let out = eval("eigen_sturm(tridiag(2, -1, 8))");
    assert!(out.contains("0.12061475842816094"), "eigen_sturm = {out}");
    // крупный случай машинной точности Штурма (протокол: n до 512):
    // λ_min(256) = 2 − 2cos(π/257) = 0.00014942666055617337
    let out = eval("eigen_sturm(tridiag(2, -1, 256))");
    assert!(out.contains("0.00014942666055617337"), "λ_min(256) = {out}");
}

// ─── ФАЗА 3: законы физики как вызываемые функции ─────────────────────────

#[test]
fn phase3_law_battery() {
    // маятник: умолчание реестра L=1 м и явный аргумент
    assert!(eval("pendulum()").contains("2.0064092925890407"));
    assert!(eval("pendulum(0.5)").contains("1.4187456166254142"));
    // голое число наследует единицу переменной: pendulum(0.25 m) ≡ pendulum(0.25)
    assert_eq!(
        eval("pendulum(0.25)"),
        eval("pendulum(0.25 m)")
    );
    // RC: τ = R·C (ом·фарад = секунда)
    assert!(eval("rc_tau(10e3, 100e-6)").contains("1.0 s"));
    // Доплер: f = f₀·v/(v−v_s) = 440·343/313
    assert!(eval("doppler(440, 343, 30)").contains("482.17252396166134"));
    // волновые процессы: λ = c/f (Wi-Fi 2.4 ГГц)
    assert!(eval("signal_wavelength(2.4e9)").contains("0.12491352416666666"));
}

#[test]
fn phase3_registry_and_signatures() {
    // реестр законов (`calc laws` в CLI — тот же источник)
    let laws = poler_engine::calc::scriptgen::list_laws();
    assert!(laws.contains("pendulum"), "laws = {laws}");
    assert!(laws.contains("rc_tau"));
    assert!(laws.contains("doppler"));
    assert!(laws.contains("signal_wavelength"));
    // каталог функций (`calc funcs`) несёт сигнатуры законов
    let funcs = poler_engine::calc::functions::catalog("");
    assert!(funcs.contains("pendulum(L)"), "funcs: нет сигнатуры pendulum(L)");
    assert!(funcs.contains("doppler(f0, v, vs)"));
}

#[test]
fn phase3_parameter_sweep() {
    // свип параметра: вектор на месте аргумента
    let sweep = eval("pendulum([0.5, 1, 2])");
    assert!(sweep.contains("1.4187456166254142"), "sweep = {sweep}");
    assert!(sweep.contains("2.0064092925890407"));
    assert!(sweep.contains("2.8374912332508284"));
    // свип по третьему аргументу Доплера
    let sweep = eval("doppler(440, 343, [10, 30, 50])");
    assert!(sweep.contains("453.21321321321324"), "sweep = {sweep}");
    assert!(sweep.contains("515.0853242320819"));
}

#[test]
fn phase3_trajectory_ground_truth() {
    // маятник L=0.25 м: x(t) = cos(2πt/T) на периоде — 5 точек
    let traj = eval("cos(2*pi*linspace(0 s, pendulum(0.25), 5) / pendulum(0.25))");
    assert!(traj.contains("1.0"), "traj = {traj}");
    assert!(traj.contains("-1.0"));
    // RC-спад u(t) = e^(−t/τ), τ=1 c, t ∈ [0, 3τ]: последний = e⁻³
    let decay = eval("exp(-linspace(0 s, 3 s, 7) / rc_tau(1e3, 1e-3))");
    assert!(decay.contains("0.049787068367863944"), "decay = {decay}");
    assert!(decay.contains("0.6065306597126334"));
    // сетка траектории: концы точны, n=5
    let grid = eval("linspace(0, 1, 5)");
    assert_eq!(grid, "[0.0, 0.25, 0.5, 0.75, 1.0]");
}

// ─── Гигиена ошибок: протокол не должен молча глотать мусор ────────────────

#[test]
fn protocol_error_hygiene() {
    // символьные плейсхолдеры протокола — направляющая ошибка, не паника
    let e = eval_err("eigen_sturm(tridiag(d, -1, N))");
    assert!(e.contains("неизвестный идентификатор"), "e = {e}");
    // слишком много аргументов закона — внятный отказ со списком переменных
    let e = eval_err("pendulum(1, 2)");
    assert!(e.contains("pendulum"), "e = {e}");
    assert!(e.contains("L"), "e = {e}");
    // частичное имя закона — НЕ функция (иначе сломается умножение на скобку)
    let e = eval_err("kepler(1+2)");
    assert!(e.contains("неизвестный идентификатор"), "e = {e}");
}

#[test]
fn phase5_dual_path_parity_reference() {
    // Фаза 5 (бит-точный паритет) на уровне ядра: повторный вызов того же
    // закона детерминирован побитово (никаких скрытых состояний/рандомов).
    let a = eval("pendulum(0.5)");
    let b = eval("pendulum(0.5)");
    assert_eq!(a, b);
    let a = eval("eigen_sturm(tridiag(2, -1, 64))");
    let b = eval("eigen_sturm(tridiag(2, -1, 64))");
    assert_eq!(a, b);
}
