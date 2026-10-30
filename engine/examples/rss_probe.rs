#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! MVR #7: пиковый RSS докинга с JIT-ядром (getrusage).
fn main() {
    let text = std::fs::read_to_string("/home/z/my-project/download/pdb/3ptb.pdb").unwrap();
    let mm = poler_engine::chem::pdb::parse_pdb(&text).unwrap();
    let params = poler_engine::chem::dock::DockParams::default();
    let res = poler_engine::chem::dock::dock("NC(=N)c1ccccc1", &mm, &params, &poler_engine::chem::dock::PocketSpec::Auto).unwrap();
    let ru = unsafe {
        let mut r = std::mem::MaybeUninit::<libc::rusage>::uninit();
        libc::getrusage(libc::RUSAGE_SELF, r.as_mut_ptr());
        r.assume_init()
    };
    println!("RMSD={:?} n_evals={}", res.rmsd, res.n_evals);
    println!("пиковый RSS: {} КиБ ({} МБ)", ru.ru_maxrss, ru.ru_maxrss / 1024);
}
