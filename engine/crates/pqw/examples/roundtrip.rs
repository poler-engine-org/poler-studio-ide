#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Минимальный пример: запись и чтение `.poler`-файла.

use pqw::{PqwReader, PqwWriter};

fn main() -> Result<(), pqw::PqwError> {
    let dir = std::path::Path::new("target");
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join("example.poler");

    // Запись: явные дуги (экспертный режим, LENS не фильтрует).
    let mut w = PqwWriter::new(512)?.hyperparams(0.01, 0.1, 0.99, 0.05);
    w.add_phase(7, 0.5)?;
    w.add_phase(11, -0.9)?;
    w.add_phase(300, 0.0)?; // явный нуль
    w.write_to(&path)?;

    // Чтение из буфера.
    let bytes = std::fs::read(&path)?;
    let r = PqwReader::from_bytes(&bytes)?;
    println!("d_pol = {}, nnz = {}", r.d_pol(), r.nnz());
    for (index, p) in r.decoded() {
        println!("  arc[{index}] p = {p:.6}, theta = {:.6}", p.acos());
    }

    // Zero-copy чтение через mmap (unix).
    #[cfg(unix)]
    {
        let map = pqw::Mmap::open(&path)?;
        let r2 = PqwReader::from_bytes(map.as_slice())?;
        r2.verify_payload()?;
        println!("payload digest verified (mmap, {} bytes)", map.len());
    }

    Ok(())
}
