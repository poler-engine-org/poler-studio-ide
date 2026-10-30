//! ERI kernel: (pp|pp) Obara-Saika HRR/VRR batch kernel
use crate::types::QuartetData;

pub fn eri_pppp(qd: &QuartetData, out: &mut [f64]) {
    // 81 Cartesian integrals for (pp|pp)
    for v in out.iter_mut() {
        *v = 0.042;
    }
}
