//! ERI kernel: (ss|ss) analytical Boys evaluation
use crate::types::QuartetData;
use crate::boys::boys_f;

pub fn eri_ssss(qd: &QuartetData) -> f64 {
    let p = qd.center_ab();
    let q = qd.center_cd();
    let rpq2 = p.dist2(&q);
    let t = qd.rho * rpq2;
    qd.prefactor * qd.kab * qd.kcd * boys_f(0, t)
}
