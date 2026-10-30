#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Измерение MVR #5: JIT-ядро скоринга vs интерпретатор на 3PTB (контур A3).

use poler_engine::chem::dock::{
    apply_state, build_field, dock, prepare_ligand, score_pose, DockParams, PocketSpec, PoseState,
};
use poler_engine::chem::pdb::{parse_pdb, ProteinIndex};
use poler_engine::graph::chem_kernel::{compile_scoring_kernel, jit_scoring_reference};

fn main() {
    let text = std::fs::read_to_string("/home/z/my-project/download/pdb/3ptb.pdb").expect("3ptb.pdb");
    let mm = parse_pdb(&text).expect("PDB");
    let receptor = mm.receptor_atoms();
    let charges = poler_engine::chem::pdb::receptor_charges(&mm);
    let donors = poler_engine::chem::pdb::reconstruct_donors(&mm);
    let index = ProteinIndex::build(&mm, &receptor);
    let lig = prepare_ligand("NC(=N)c1ccccc1").expect("бензамидин");
    let crystal_lig = mm.ligand_residues().first().copied().expect("лиганд 3PTB");
    let pocket = poler_engine::chem::pdb::pocket_from_ligand(&mm, crystal_lig);
    let field = build_field(&mm, &index, &charges, &donors, pocket.center, pocket.radius + 2.0);
    println!("поле: {} атомов; лиганд: {} тяжёлых, {} узлов", field.len, lig.conf.heavy_map.len(), lig.base.len());

    // 1) полный dock() с JIT (после LTO-сборки? — дебаг-сборка без LTO)
    let params = DockParams::default();
    let t0 = std::time::Instant::now();
    let res = dock("NC(=N)c1ccccc1", &mm, &params, &PocketSpec::Auto).expect("докинг");
    let jit_ms = t0.elapsed().as_millis();
    println!("dock() JIT: {} мс, n_evals={}, RMSD={:?}", jit_ms, res.n_evals,
        res.rmsd.map(|v| format!("{:.2} Å", v)));

    // 2) полный dock() без JIT (интерпретатор)
    let params_i = DockParams { jit: false, ..Default::default() };
    let t0 = std::time::Instant::now();
    let res_i = dock("NC(=N)c1ccccc1", &mm, &params_i, &PocketSpec::Auto).expect("докинг");
    let interp_ms = t0.elapsed().as_millis();
    println!("dock() интерпретатор: {} мс, n_evals={}, RMSD={:?}", interp_ms, res_i.n_evals,
        res_i.rmsd.map(|v| format!("{:.2} Å", v)));
    println!("УСКОРЕНИЕ: {:.2}×", interp_ms as f64 / jit_ms.max(1) as f64);

    // 3) изолированный скоринг: JIT vs score_pose на одинаковых позах
    let kernel = compile_scoring_kernel(&lig, &field).expect("ядро");
    println!("ядро: {} инстр, код {} Б + блоб {} Б (таблицы в исполняемой странице)",
        kernel.inst, kernel.code_bytes, kernel.blob_bytes);
    let n = 26_416usize;
    let mut poses: Vec<PoseState> = Vec::with_capacity(64);
    let mut rs: u64 = 0x9E3779B97F4A7C15;
    for _ in 0..64 {
        let mut r = |lo: f64, hi: f64| {
            rs = rs.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let u = (rs >> 11) as f64 / (1u64 << 53) as f64;
            lo + u * (hi - lo)
        };
        poses.push(PoseState {
            center: [pocket.center[0] + r(-1.0, 1.0), pocket.center[1] + r(-1.0, 1.0), pocket.center[2] + r(-1.0, 1.0)],
            rot: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            torsions: vec![r(-3.14, 3.14); lig.torsions.len()],
        });
    }
    let all_pos: Vec<Vec<[f64; 3]>> = poses.iter().map(|st| apply_state(&lig, st)).collect();
    let t0 = std::time::Instant::now();
    let mut acc = 0.0f64;
    for i in 0..n {
        let t = kernel.score(&lig, &all_pos[i % all_pos.len()]);
        acc += t.e_vdw;
    }
    let k_ms = t0.elapsed().as_millis();
    let t0 = std::time::Instant::now();
    for i in 0..n {
        let t = score_pose(&lig, &field, &all_pos[i % all_pos.len()]);
        acc += t.e_hb;
    }
    let i_ms = t0.elapsed().as_millis();
    println!("изолированный скоринг ×{}: JIT {} мс vs интерпретатор {} мс → {:.2}× (acc {:.3})",
        n, k_ms, i_ms, i_ms as f64 / k_ms.max(1) as f64, acc);

    // 4) точность на реальном поле (JIT vs score_pose, 64 позы)
    let mut worst = 0.0f64;
    for pos in &all_pos {
        let j = kernel.score(&lig, pos);
        let o = score_pose(&lig, &field, pos);
        for (a, b) in [(j.e_vdw, o.e_vdw), (j.e_elec, o.e_elec), (j.e_lipo, o.e_lipo), (j.e_hb, o.e_hb), (j.e_desolv, o.e_desolv)] {
            let d = (a - b).abs();
            if d > worst { worst = d; }
        }
    }
    println!("макс. отклонение на компонент vs score_pose: {:.2e} (допуск 1e-9)", worst);
}
