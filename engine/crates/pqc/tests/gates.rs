#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Гейты и statevector: паритет с аналитическими формулами на 1e-12.

use pqc::{complex::Cx, Gate, Statevector};

const TOL: f64 = 1e-12;

fn assert_amps(sv: &Statevector, expected: &[(f64, f64)]) {
    assert_eq!(sv.dim(), expected.len(), "dimension mismatch");
    for (i, (a, e)) in sv.amplitudes().iter().zip(expected).enumerate() {
        assert!(
            (a.re - e.0).abs() < TOL && (a.im - e.1).abs() < TOL,
            "amp[{i}] = {:?}, expected {:?}",
            a,
            e
        );
    }
}

#[test]
fn ry_zero_is_identity() {
    let mut sv = Statevector::new(2).unwrap();
    sv.apply_ry(0, 0.0).unwrap();
    sv.apply_ry(1, 0.0).unwrap();
    assert_amps(&sv, &[(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)]);
}

#[test]
fn ry_pi_flips_to_one() {
    let mut sv = Statevector::new(1).unwrap();
    sv.apply_ry(0, std::f64::consts::PI).unwrap();
    assert_amps(&sv, &[(0.0, 0.0), (1.0, 0.0)]);
}

#[test]
fn ry_pi_half_superposition() {
    let mut sv = Statevector::new(1).unwrap();
    sv.apply_ry(0, std::f64::consts::FRAC_PI_2).unwrap();
    let k = std::f64::consts::FRAC_1_SQRT_2;
    assert_amps(&sv, &[(k, 0.0), (k, 0.0)]);
}

#[test]
fn ry_two_qubits_tensor_structure() {
    // Ry(a) на q0, Ry(b) на q1: amp[b1·2 + b0] = s(b0|a)·s(b1|b).
    let (a, b) = (0.7, 1.3);
    let mut sv = Statevector::new(2).unwrap();
    sv.apply_ry(0, a).unwrap();
    sv.apply_ry(1, b).unwrap();
    let (ca, sa) = ((a / 2.0).cos(), (a / 2.0).sin());
    let (cb, sb) = ((b / 2.0).cos(), (b / 2.0).sin());
    assert_amps(
        &sv,
        &[
            (ca * cb, 0.0),
            (sa * cb, 0.0),
            (ca * sb, 0.0),
            (sa * sb, 0.0),
        ],
    );
}

#[test]
fn rx_quarter_rotation() {
    // Rx(π/2)|0⟩ = (cos(π/4), −i·sin(π/4)).
    let mut sv = Statevector::new(1).unwrap();
    sv.apply_rx(0, std::f64::consts::FRAC_PI_2).unwrap();
    let k = std::f64::consts::FRAC_1_SQRT_2;
    assert_amps(&sv, &[(k, 0.0), (0.0, -k)]);
}

#[test]
fn rx_pi_is_minus_i_x() {
    let mut sv = Statevector::new(1).unwrap();
    sv.apply_rx(0, std::f64::consts::PI).unwrap();
    assert_amps(&sv, &[(0.0, 0.0), (0.0, -1.0)]);
}

#[test]
fn rz_adds_phases_on_plus() {
    // H затем Rz(φ): |+⟩ → (e^{−iφ/2}|0⟩ + e^{+iφ/2}|1⟩)/√2.
    let phi = std::f64::consts::FRAC_PI_3;
    let mut sv = Statevector::new(1).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_rz(0, phi).unwrap();
    let k = std::f64::consts::FRAC_1_SQRT_2;
    let (c, s) = ((phi / 2.0).cos(), (phi / 2.0).sin());
    assert_amps(&sv, &[(k * c, -k * s), (k * c, k * s)]);
}

#[test]
fn hadamard_three_qubits_uniform() {
    let mut sv = Statevector::new(3).unwrap();
    for q in 0..3 {
        sv.apply_h(q).unwrap();
    }
    let e = 1.0 / (8.0_f64).sqrt();
    let expected: Vec<(f64, f64)> = (0..8).map(|_| (e, 0.0)).collect();
    assert_amps(&sv, &expected);
}

#[test]
fn x_swaps_amplitudes() {
    let mut sv = Statevector::new(2).unwrap();
    sv.apply_x(1).unwrap();
    assert_amps(&sv, &[(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)]);

    let mut sv = Statevector::from_phases(&[0.6]).unwrap();
    sv.apply_x(0).unwrap();
    // X меняет местами различные амплитуды: [cos, sin] → [sin, cos].
    let t = 0.6_f64.acos();
    assert_amps(&sv, &[((t / 2.0).sin(), 0.0), ((t / 2.0).cos(), 0.0)]);
}

#[test]
fn y_on_ground_is_i_one() {
    let mut sv = Statevector::new(1).unwrap();
    sv.apply_y(0).unwrap();
    assert_amps(&sv, &[(0.0, 0.0), (0.0, 1.0)]);
}

#[test]
fn z_flips_one() {
    let mut sv = Statevector::new(1).unwrap();
    sv.apply_x(0).unwrap();
    sv.apply_z(0).unwrap();
    assert_amps(&sv, &[(0.0, 0.0), (-1.0, 0.0)]);
}

#[test]
fn cnot_builds_bell_state() {
    let mut sv = Statevector::new(2).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_cnot(0, 1).unwrap();
    let k = std::f64::consts::FRAC_1_SQRT_2;
    assert_amps(&sv, &[(k, 0.0), (0.0, 0.0), (0.0, 0.0), (k, 0.0)]);
}

#[test]
fn cnot_control_zero_is_noop() {
    let mut sv = Statevector::new(2).unwrap();
    sv.apply_x(1).unwrap(); // |10⟩: контрол q0 = 0.
    sv.apply_cnot(0, 1).unwrap();
    assert_amps(&sv, &[(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)]);
}

#[test]
fn cnot_control_one_flips_target() {
    let mut sv = Statevector::new(3).unwrap();
    sv.apply_x(2).unwrap(); // контрол q2 = 1.
    sv.apply_cnot(2, 0).unwrap();
    // |100⟩ → |101⟩ = индекс 5.
    assert_amps(
        &sv,
        &[
            (0.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
            (1.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
        ],
    );
}

#[test]
fn cz_signs_coincident_ones() {
    let mut sv = Statevector::new(2).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_h(1).unwrap();
    sv.apply_cz(0, 1).unwrap();
    assert_amps(&sv, &[(0.5, 0.0), (0.5, 0.0), (0.5, 0.0), (-0.5, 0.0)]);
}

#[test]
fn cnot_rejects_same_qubit() {
    let mut sv = Statevector::new(2).unwrap();
    assert!(sv.apply_cnot(1, 1).is_err());
    assert!(sv.apply_cz(0, 0).is_err());
}

#[test]
fn gates_reject_bad_qubits() {
    let mut sv = Statevector::new(2).unwrap();
    for g in [
        Gate::ry(2, 1.0),
        Gate::rx(5, 1.0),
        Gate::rz(9, 1.0),
        Gate::h(3),
        Gate::x(7),
        Gate::y(11),
        Gate::z(13),
        Gate::cx(0, 9),
        Gate::cz(9, 0),
        Gate::U2 {
            q: 42,
            m: [[Cx::ONE, Cx::ZERO], [Cx::ZERO, Cx::ONE]],
        },
    ] {
        assert!(sv.apply(g).is_err(), "gate {g} must be rejected");
    }
}

#[test]
fn norm_preserved_under_gate_sequence() {
    let mut sv = Statevector::new(5).unwrap();
    let seq = [
        Gate::h(0),
        Gate::h(2),
        Gate::cx(0, 1),
        Gate::ry(3, 0.777),
        Gate::rx(4, -1.234),
        Gate::rz(1, 2.5),
        Gate::cz(2, 3),
        Gate::y(0),
        Gate::x(4),
        Gate::cx(3, 0),
    ];
    sv.apply_seq(seq).unwrap();
    assert!((sv.norm() - 1.0).abs() < TOL);
}

#[test]
fn gate_enum_dispatch_matches_direct_calls() {
    let mut a = Statevector::new(3).unwrap();
    let mut b = Statevector::new(3).unwrap();
    a.apply(Gate::ry(1, 0.9)).unwrap();
    b.apply_ry(1, 0.9).unwrap();
    a.apply(Gate::cx(0, 2)).unwrap();
    b.apply_cnot(0, 2).unwrap();
    for (x, y) in a.amplitudes().iter().zip(b.amplitudes()) {
        assert_eq!(x, y);
    }
}

// --- Анзац из фаз ---

#[test]
fn from_phases_poles() {
    // p0=+1 → q0=|0⟩; p1=−1 → q1=|1⟩: единственный исход 0b10.
    let sv = Statevector::from_phases(&[1.0, -1.0]).unwrap();
    assert_amps(&sv, &[(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)]);
}

#[test]
fn from_phases_all_zero_is_uniform() {
    let sv = Statevector::from_phases(&[0.0; 4]).unwrap();
    let expected: Vec<(f64, f64)> = (0..16).map(|_| (0.25, 0.0)).collect();
    assert_amps(&sv, &expected);
}

#[test]
fn from_phases_matches_analytic_product() {
    let ps = [0.3, -0.7, 0.5];
    let sv = Statevector::from_phases(&ps).unwrap();
    for x in 0..8usize {
        let mut amp = 1.0;
        for (q, &p) in ps.iter().enumerate() {
            let theta = p.acos();
            amp *= if x & (1 << q) == 0 {
                (theta / 2.0).cos()
            } else {
                (theta / 2.0).sin()
            };
        }
        assert!(
            (sv.amplitudes()[x].re - amp).abs() < TOL && sv.amplitudes()[x].im.abs() < TOL,
            "amp[{x}] mismatch"
        );
    }
}

#[test]
fn marginals_follow_phase_formula() {
    let ps = [0.25, -0.6, 0.9, -1.0, 0.0];
    let sv = Statevector::from_phases(&ps).unwrap();
    let m = sv.marginals();
    for (q, &p) in ps.iter().enumerate() {
        assert!((m[q] - (1.0 - p) / 2.0).abs() < TOL, "q = {q}");
    }
}

#[test]
fn from_phases_rejects_bad_values() {
    assert!(Statevector::from_phases(&[1.5]).is_err());
    assert!(Statevector::from_phases(&[-1.0001]).is_err());
    assert!(Statevector::from_phases(&[f64::NAN]).is_err());
    assert!(Statevector::from_phases(&[f64::INFINITY]).is_err());
    assert!(Statevector::from_phases(&[]).is_err());
    assert!(matches!(
        Statevector::from_phases(&vec![0.5; 27]),
        Err(pqc::PqcError::TooManyQubits { .. })
    ));
}

#[test]
fn expect_z_returns_phase_for_every_qubit() {
    let ps = [0.11, -0.22, 0.33, -0.44, 0.55, 0.66];
    let sv = Statevector::from_phases(&ps).unwrap();
    for (q, &p) in ps.iter().enumerate() {
        assert!((sv.expect_z(q).unwrap() - p).abs() < TOL);
    }
}

#[test]
fn probabilities_are_born_weights() {
    let ps = [0.4, -0.1];
    let sv = Statevector::from_phases(&ps).unwrap();
    let probs = sv.probabilities();
    let (t0, t1) = (ps[0].acos(), ps[1].acos());
    let expected = [
        (t0 / 2.0).cos() * (t1 / 2.0).cos(),
        (t0 / 2.0).sin() * (t1 / 2.0).cos(),
        (t0 / 2.0).cos() * (t1 / 2.0).sin(),
        (t0 / 2.0).sin() * (t1 / 2.0).sin(),
    ];
    for (p, e) in probs.iter().zip(expected) {
        // Born: вероятность = квадрат амплитуды.
        assert!((p - e * e).abs() < TOL);
    }
}
