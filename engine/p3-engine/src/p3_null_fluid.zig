// =============================================================================
// P³ DISPERSIVE & PHASE BOUNDARY OPTICS v1.0 — ZIG
// =============================================================================
//
// Математическая модель дисперсионных сред, фазовых границ и комплексной
// проективной оптики на P³ (модель Друде-Лоренца, эванесцентные волны).
//
// Все физические параметры конфигурируются пользователем.
//
// Ключевые физические соотношения:
//   ρ(θ) = sin²(θ − θ_crit) · H(θ − θ_crit) — плотность фазовой границы
//   n_eff = i · n₁ · √(ε₂/ε₁) — комплексный/мнимый показатель преломления
//   δ_opt = λ / (2π · |Im(n)|) — глубина скин-слоя / затухания
//   T_op/T_norm = (1 + κ·ρ) / dΣ — спектр операционного времени переноса
//   d_n = d_0 · (n + ½) — квантование уровней проникновения
//   n² = ε(ω) = 1 − ω_p² / [ω(ω + i·γ_coll)] — модель плазменного отражателя Друде
// =============================================================================

const std = @import("std");
const math = std.math;

/// Degrees to radians conversion constant
const deg_to_rad: f64 = math.pi / 180.0;

// =============================================================================
// 1. PHASE BOUNDARY CONFIGURATION
// =============================================================================

/// Конфигурация параметров фазовой границы и оптической среды.
pub const PhaseBoundaryConfig = struct {
    /// Критический угол (градусы) — порог фазового перехода
    theta_crit_deg: f64,
    /// Ширина переходной зоны (градусы)
    delta_deg: f64,
    /// Показатель преломления первой среды
    n_omega: f64,
    /// Диэлектрическая проницаемость первой среды
    eps_omega: f64,
    /// Диэлектрическая проницаемость второй среды
    eps_chi: f64,
    /// Скорость света в вакууме (м/с)
    c_light: f64,
    /// Характеристическая частота волнового поля (Гц)
    f_phi: f64,
    /// Базовый шаг дискретизации глубины (м)
    d_k: f64,
    /// Коэффициент связи масштабирования времени
    kappa: f64,

    /// Универсальная конфигурация по умолчанию (воздух/вакуум)
    pub fn initDefault() PhaseBoundaryConfig {
        return .{
            .theta_crit_deg = 45.0,
            .delta_deg = 5.0,
            .n_omega = 1.0,
            .eps_omega = 1.0,
            .eps_chi = 1.0,
            .c_light = 2.99792458e8,
            .f_phi = 1.0e9,
            .d_k = 0.01,
            .kappa = 1.0,
        };
    }

    /// Анизотропный коэффициент диэлектрической связи: κ = |ε₂| / ε₁
    pub inline fn kappaPhi(self: PhaseBoundaryConfig) f64 {
        return self.eps_chi / self.eps_omega;
    }

    /// Фундаментальный шаг квантования: d_0 = d_K / 1.5
    pub inline fn d0(self: PhaseBoundaryConfig) f64 {
        return self.d_k / 1.5;
    }

    /// Скорость распространения в среде: c_medium = c / √n₁ (м/с)
    pub inline fn cPhi(self: PhaseBoundaryConfig) f64 {
        return self.c_light / @sqrt(self.n_omega);
    }

    /// Характеристическое время релаксации: τ = 1/f (с)
    pub inline fn tauRelax(self: PhaseBoundaryConfig) f64 {
        return 1.0 / self.f_phi;
    }
};

pub const NullFluidConfig = PhaseBoundaryConfig;

// =============================================================================
// 2. ПРОЕКТИВНАЯ ПЛОТНОСТЬ ПЕРЕХОДА
// =============================================================================

/// Проективная плотность нуль-жидкости.
///
/// ρ_∅(θ) = sin²(θ − θ_crit) · H(θ − θ_crit)
///
/// Exact формула (без малых углов): sin² вместо x².
pub fn nullFluidDensityExact(theta_deg: f64, theta_crit_deg: f64) f64 {
    if (theta_deg <= theta_crit_deg) return 0.0;
    const diff_rad = deg_to_rad * (theta_deg - theta_crit_deg);
    const s = math.sin(diff_rad);
    return s * s;
}

/// Проективная плотность — малые углы (для GPU).
///
/// Для θ − θ_crit ≪ 1: sin²(x) ≈ x²
pub fn nullFluidDensitySmallAngle(theta_deg: f64, theta_crit_deg: f64) f64 {
    if (theta_deg <= theta_crit_deg) return 0.0;
    const diff_rad = deg_to_rad * (theta_deg - theta_crit_deg);
    return diff_rad * diff_rad;
}

pub const criticalDensityExact = nullFluidDensityExact;
pub const criticalDensitySmallAngle = nullFluidDensitySmallAngle;

// =============================================================================
// 3. ПРОЕКТИВНАЯ ОПТИКА
// =============================================================================

/// Результат проективной оптики.
///
/// n_∅ = i · n_Ω · √κ_φ — мнимый показатель преломления.
// =============================================================================
// 3. КОМПЛЕКСНАЯ ПРОЕКТИВНАЯ ОПТИКА
// =============================================================================

/// Параметры комплексного показателя преломления и эванесцентного затухания.
pub const ProjectiveOptics = struct {
    /// Отношение диэлектрических проницаемостей: κ = ε₂ / ε₁
    kappa_phi: f64,
    /// Модуль мнимой части показателя преломления: |Im(n)| = n₁ · √κ
    n_null_abs: f64,
    /// Длина волны в вакууме λ = c / f (м)
    lambda_phi: f64,
    /// Глубина проникновения эванесцентного поля: δ = λ / (2π · |Im(n)|) (м)
    delta_opt: f64,

    /// Расчет комплексной оптики из конфигурации
    pub fn compute(cfg: NullFluidConfig) ProjectiveOptics {
        const kp = cfg.kappaPhi();
        const sqrt_kp = @sqrt(kp);
        const n_abs = cfg.n_omega * sqrt_kp;
        const lambda = cfg.c_light / cfg.f_phi;
        const delta = lambda / (2.0 * math.pi * n_abs);
        return .{
            .kappa_phi = kp,
            .n_null_abs = n_abs,
            .lambda_phi = lambda,
            .delta_opt = delta,
        };
    }
};

// =============================================================================
// 4. СПЕКТР ВРЕМЕНИ ПЕРЕНОСА
// =============================================================================

/// Спектр масштабирования времени переноса через фазовую границу.
///
/// T_op / T_norm = (1 + κ · ρ) / (sin²(δ · r / r₀) / sin²(δ))
pub fn operationalTimeSpectrum(r_ratio: f64, cfg: NullFluidConfig) f64 {
    const theta_r = 90.0 - cfg.delta_deg * r_ratio;
    const rho_r = nullFluidDensityExact(theta_r, cfg.theta_crit_deg);

    const delta_rad = deg_to_rad * cfg.delta_deg;
    const sin2_delta = math.sin(delta_rad) * math.sin(delta_rad);
    const arg = delta_rad * r_ratio;
    const sin2_arg = math.sin(arg) * math.sin(arg);
    const d_sigma = sin2_arg / sin2_delta;

    if (d_sigma < 1e-10) return std.math.floatMax(f64);
    return (1.0 + cfg.kappa * rho_r) / d_sigma;
}

// =============================================================================
// 5. КВАНТОВАНИЕ УРОВНЕЙ ПОГРУЖЕНИЯ
// =============================================================================

/// Дискретные уровни глубины проникновения: d_n = d_0 · (n + ½)
pub fn immersionDepth(n: u32, cfg: NullFluidConfig) f64 {
    return cfg.d0() * (@as(f64, @floatFromInt(n)) + 0.5);
}

// =============================================================================
// 6. СПЕКТРАЛЬНЫЕ КЛАССЫ РЕЗОНАНСНЫХ СОСТОЯНИЙ
// =============================================================================

/// Спектральный класс для неэрмитова гамильтониана.
pub const SpectralClass = enum(u2) {
    /// E < 0, Γ ≈ 0, τ → ∞ — связанное метастабильное состояние
    alpha = 0,
    /// E < 0, Γ > 0, τ > threshold — квазистационарное состояние
    beta = 1,
    /// E > 0, τ ≈ 0 — непрерывный спектр / распад
    gamma = 2,
};

/// Конфигурация порогов спектральной классификации.
pub const SpectralClassConfig = struct {
    /// Порог ширины резонанса (Γ)
    gamma_threshold: f64,
    /// Порог времени жизни (τ, секунды)
    tau_threshold: f64,
    /// Порог энергии: E < 0 → bound, E > 0 → continuum
    energy_threshold: f64,

    pub fn initDefault() SpectralClassConfig {
        return .{
            .gamma_threshold = 1e-3,
            .tau_threshold = 4.0,
            .energy_threshold = 0.0,
        };
    }

    /// Классифицировать состояние по энергии, ширине и времени жизни
    pub fn classify(self: SpectralClassConfig, energy: f64, gamma_width: f64, tau: f64) SpectralClass {
        if (energy > self.energy_threshold) return .gamma;
        if (gamma_width < self.gamma_threshold) return .alpha;
        if (tau > self.tau_threshold) return .beta;
        return .gamma;
    }
};

// =============================================================================
// 7. МОДЕЛЬ ПЛАЗМЕННОГО ОТРАЖАТЕЛЯ (DRUDE-LORENZ)
// =============================================================================

/// Модель плазменного/металлического отражателя Друде:
///
/// n² = ε(ω) = 1 − ω_p² / [ω · (ω + i · γ_coll)]
///
/// При γ_coll → 0: ε(ω) < 0 (для ω < ω_p) → полное отражение (плазмонное зеркало).
pub const DrudeReflectorOptics = struct {
    /// Плазменная частота (рад/с)
    omega_p: f64,
    /// Частота столкновений/релаксации (рад/с)
    gamma_coll: f64,
    /// Частота падающего излучения (рад/с)
    omega: f64,

    /// Вычислить комплексную диэлектрическую проницаемость ε(ω) = ε_real + i·ε_imag
    pub fn dielectric(self: DrudeReflectorOptics) struct { re: f64, im: f64 } {
        const w = self.omega;
        const wp2 = self.omega_p * self.omega_p;
        const gc = self.gamma_coll;

        const denom = w * (w * w + gc * gc);
        if (@abs(denom) < 1e-30) return .{ .re = 1.0, .im = 0.0 };

        const re = 1.0 - wp2 * w / denom;
        const im = wp2 * gc / denom;
        return .{ .re = re, .im = im };
    }

    /// Коэффициент отражения Френеля при нормальном падении: R = ((|n| - 1) / (|n| + 1))²
    pub fn reflectivity(self: DrudeReflectorOptics) f64 {
        const eps = self.dielectric();
        const eps_abs = @sqrt(eps.re * eps.re + eps.im * eps.im);
        const n_abs = @sqrt(eps_abs);
        const ratio = (n_abs - 1.0) / (n_abs + 1.0);
        return ratio * ratio;
    }
};

// =============================================================================
// 8. ПАРАМЕТРИЧЕСКИЕ СВОЙСТВА МАТЕРИАЛОВ
// =============================================================================

/// Параметры проводимости и стабильности материала.
pub const ChemicalSpecies = struct {
    /// Название материала/элемента
    name: []const u8,
    /// Удельная проводимость σ_e
    sigma_e: f64,
    /// Порог критической проводимости
    stability_threshold: f64,

    /// Проверка стабильности проводимости
    pub inline fn isBelowThreshold(self: ChemicalSpecies) bool {
        return self.sigma_e < self.stability_threshold;
    }
};

/// Пространственная зона среды с заданными физическими свойствами.
pub const MediumZone = struct {
    cfg: PhaseBoundaryConfig,
    /// Максимальный радиус зоны (м)
    r_0: f64,
    /// Проводимость среды
    sigma_e: f64,
    /// Порог стабильности
    stability_threshold: f64,

    /// Время распространения сигнала на расстояние L
    pub fn signalTime(self: MediumZone, L: f64) f64 {
        return L / self.cfg.cPhi();
    }

    /// Проверка стабильности зоны
    pub inline fn isBelowThreshold(self: MediumZone) bool {
        return self.sigma_e < self.stability_threshold;
    }
};

// =============================================================================
// 9. ГЕОДЕЗИЧЕСКИЕ ТОЧКИ НА СФЕРЕ S³
// =============================================================================

/// N точек, равномерно распределенных по долготе на заданной широте θ на сфере S³:
/// Point_k = [sin(θ)·cos(φ_k), sin(θ)·sin(φ_k), 0, cos(θ)], где φ_k = 2π·k / N
pub fn circlePoint(k: u32, n: u32, theta_deg: f64) [4]f64 {
    const theta_rad = theta_deg * deg_to_rad;
    const phi_rad = 2.0 * math.pi * @as(f64, @floatFromInt(k)) / @as(f64, @floatFromInt(n));
    const st = math.sin(theta_rad);
    const ct = math.cos(theta_rad);
    return .{
        st * math.cos(phi_rad),
        st * math.sin(phi_rad),
        0.0,
        ct,
    };
}

// =============================================================================
// 10. ТЕСТЫ
// =============================================================================

test "PhaseBoundary: zero below theta_crit" {
    const cfg = PhaseBoundaryConfig.initDefault();
    try std.testing.expectApproxEqAbs(criticalDensityExact(cfg.theta_crit_deg - 5.0, cfg.theta_crit_deg), 0.0, 1e-10);
    try std.testing.expectApproxEqAbs(criticalDensityExact(cfg.theta_crit_deg, cfg.theta_crit_deg), 0.0, 1e-10);
}

test "PhaseBoundary: sin² at theta = theta_crit + 5°" {
    const cfg = PhaseBoundaryConfig.initDefault();
    const rho = criticalDensityExact(cfg.theta_crit_deg + 5.0, cfg.theta_crit_deg);
    const expected = math.sin(5.0 * deg_to_rad) * math.sin(5.0 * deg_to_rad);
    try std.testing.expectApproxEqAbs(rho, expected, 1e-10);
}

test "PhaseBoundary: increases monotonically" {
    const cfg = PhaseBoundaryConfig.initDefault();
    const rho_1 = criticalDensityExact(cfg.theta_crit_deg + 1.0, cfg.theta_crit_deg);
    const rho_2 = criticalDensityExact(cfg.theta_crit_deg + 2.0, cfg.theta_crit_deg);
    const rho_3 = criticalDensityExact(cfg.theta_crit_deg + 3.0, cfg.theta_crit_deg);
    try std.testing.expect(rho_1 < rho_2);
    try std.testing.expect(rho_2 < rho_3);
}

test "PhaseBoundary vs small-angle: agree near theta_crit" {
    const cfg = PhaseBoundaryConfig.initDefault();
    const rho_exact = criticalDensityExact(cfg.theta_crit_deg + 0.5, cfg.theta_crit_deg);
    const rho_small = criticalDensitySmallAngle(cfg.theta_crit_deg + 0.5, cfg.theta_crit_deg);
    try std.testing.expectApproxEqAbs(rho_exact, rho_small, rho_exact * 0.02);
}

test "PhaseBoundary: non-zero at theta = 90 deg" {
    const cfg = PhaseBoundaryConfig.initDefault();
    const rho = nullFluidDensityExact(90.0, cfg.theta_crit_deg);
    try std.testing.expect(rho > 0.0);
}

test "PhaseBoundaryConfig: custom parameters" {
    const cfg = PhaseBoundaryConfig{
        .theta_crit_deg = 80.0,
        .delta_deg = 10.0,
        .n_omega = 1.5,
        .eps_omega = 3.0,
        .eps_chi = 1.2,
        .c_light = 3e8,
        .f_phi = 2e12,
        .d_k = 0.01,
        .kappa = 5.0,
    };
    try std.testing.expectApproxEqAbs(cfg.kappaPhi(), 0.4, 1e-10);
    try std.testing.expectApproxEqAbs(cfg.d0(), 0.01 / 1.5, 1e-15);
}

test "ProjectiveOptics: compute values" {
    const cfg = PhaseBoundaryConfig{
        .theta_crit_deg = 45.0,
        .delta_deg = 5.0,
        .n_omega = 2.05,
        .eps_omega = 4.2,
        .eps_chi = 1.8,
        .c_light = 2.998e8,
        .f_phi = 1.4e12,
        .d_k = 0.02,
        .kappa = 7.2,
    };
    const optics = ProjectiveOptics.compute(cfg);
    try std.testing.expectApproxEqAbs(optics.kappa_phi, 1.8 / 4.2, 1e-10);
    try std.testing.expect(optics.n_null_abs > 1.0);
    try std.testing.expect(optics.n_null_abs < 2.0);
    try std.testing.expect(optics.delta_opt > 1e-6);
    try std.testing.expect(optics.delta_opt < 1e-3);
}

test "operationalTimeSpectrum: diverges at r=0" {
    const cfg = NullFluidConfig.initDefault();
    const T = operationalTimeSpectrum(0.0, cfg);
    try std.testing.expect(T > 1e10);
}

test "operationalTimeSpectrum: finite at r=r_0" {
    const cfg = NullFluidConfig.initDefault();
    const T = operationalTimeSpectrum(1.0, cfg);
    try std.testing.expect(T < 1e10);
    try std.testing.expect(T > 0);
}

test "immersionDepth: d_0*(n+0.5)" {
    const cfg = NullFluidConfig.initDefault();
    try std.testing.expectApproxEqAbs(immersionDepth(0, cfg), cfg.d0() * 0.5, 1e-15);
    try std.testing.expectApproxEqAbs(immersionDepth(1, cfg), cfg.d_k, 1e-15);
}

test "SpectralClassConfig: classify" {
    const sc = SpectralClassConfig.initDefault();
    // Gamma class: E > 0
    try std.testing.expect(sc.classify(1.0, 0.1, 1.0) == .gamma);
    // Alpha class: E < 0, small gamma
    try std.testing.expect(sc.classify(-1.0, 1e-6, 1e10) == .alpha);
    // Beta class: E < 0, large gamma, long tau
    try std.testing.expect(sc.classify(-1.0, 0.1, 10.0) == .beta);
}

test "DrudeReflectorOptics: high reflectivity at low damping" {
    const bm = DrudeReflectorOptics{
        .omega_p = 1e15,
        .gamma_coll = 1e6,
        .omega = 1e15,
    };
    const R = bm.reflectivity();
    try std.testing.expect(R > 0.9);
}

test "DrudeReflectorOptics: lower reflectivity at high damping" {
    const bm = DrudeReflectorOptics{
        .omega_p = 1e15,
        .gamma_coll = 1e15,
        .omega = 1e15,
    };
    const R = bm.reflectivity();
    try std.testing.expect(R < 1.0);
}

test "ChemicalSpecies: stability threshold" {
    const species = ChemicalSpecies{
        .name = "element_A",
        .sigma_e = 1.5,
        .stability_threshold = 5.0,
    };
    try std.testing.expect(species.isBelowThreshold());

    const species2 = ChemicalSpecies{
        .name = "element_B",
        .sigma_e = 7.2,
        .stability_threshold = 5.0,
    };
    try std.testing.expect(!species2.isBelowThreshold());
}

test "MediumZone: signal propagation time" {
    const zone = MediumZone{
        .cfg = PhaseBoundaryConfig.initDefault(),
        .r_0 = 0.5,
        .sigma_e = 4.0,
        .stability_threshold = 5.0,
    };
    const tau = zone.signalTime(100.0);
    try std.testing.expect(tau > 1e-9);
    try std.testing.expect(tau < 1e-5);
    try std.testing.expect(zone.isBelowThreshold());
}

test "circlePoint: all on S³" {
    const theta = 80.0;
    const n: u32 = 12;
    for (0..n) |k| {
        const p = circlePoint(@intCast(k), n, theta);
        const norm_sq = p[0] * p[0] + p[1] * p[1] + p[2] * p[2] + p[3] * p[3];
        try std.testing.expectApproxEqAbs(norm_sq, 1.0, 1e-10);
    }
}

test "circlePoint: custom N and θ" {
    const p = circlePoint(0, 8, 70.0);
    try std.testing.expectApproxEqAbs(p[3], math.cos(70.0 * deg_to_rad), 1e-10);
}
