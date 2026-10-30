#![allow(unused, clippy::all, clippy::incompatible_msrv)]
use poler_eri::BatchCompiler;

fn main() {
    let batch = BatchCompiler::new(3);
    let modules = batch.compile_all();
    
    let dir = "/home/z/my-project/download/libcint-rust/src/autogen";
    BatchCompiler::write_to_dir(&modules, dir).expect("Failed to write autogen files");
    println!("Generated {} modules in {}", modules.len(), dir);
    
    let poler_dir = "/home/z/my-project/download/POLER-ERI-v3.0.1/generated";
    BatchCompiler::write_to_dir(&modules, poler_dir).expect("Failed to write generated files");
    println!("Also wrote {} modules to {}", modules.len(), poler_dir);
}
