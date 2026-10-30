#![allow(unused, clippy::all, clippy::incompatible_msrv)]
use std::fs::File;
use std::time::Instant;
use memmap2::MmapOptions;
use poler_engine::quantum::crystallizer::{SafetensorsHeader, WeightCircuitBuilder, execute_circuit_direct};

#[test]
fn test_real_safetensors_crystallization_bench() {
    let model_path = "/home/vitalij/Стільниця/Нова тека (4)/model.safetensors";
    if !std::path::Path::new(model_path).exists() {
        eprintln!("model.safetensors not found, skipping real model bench");
        return;
    }

    let file = File::open(model_path).expect("failed to open safetensors");
    let mmap = unsafe { MmapOptions::new().map(&file).expect("failed to mmap") };

    let t0 = Instant::now();
    let header = SafetensorsHeader::parse(&mmap).expect("failed to parse header");
    let parse_time = t0.elapsed();

    println!("\n=== SAFETENSORS CRYSTALLIZATION REPORT ===");
    println!("Safetensors total size: {} MB", mmap.len() / (1024 * 1024));
    println!("Header size: {} bytes", header.header_size);
    println!("Tensor count: {}", header.tensors.len());
    println!("Header parse time: {:?}", parse_time);

    // Находим первый слой внимания или проекции
    let mut chosen_tensor = None;
    for (name, info) in &header.tensors {
        if info.shape.len() == 2 && info.shape[0] >= 64 && info.shape[1] >= 64 {
            chosen_tensor = Some((name.clone(), info.clone()));
            break;
        }
    }

    if let Some((name, info)) = chosen_tensor {
        println!("\nTarget Attention Layer: {}", name);
        println!("Shape: {:?}, Dtype: {}", info.shape, info.dtype);
        
        let in_dim = info.shape[1].min(256);
        let out_dim = info.shape[0].min(256);

        // Строим R1CS конвейер для блока 256x256
        let mut builder = WeightCircuitBuilder::new(in_dim);
        
        // Превращаем срез весов в тритернарные фазовые коэффициенты {-1, 0, 1}
        let data_start = header.header_size + info.data_offsets[0];
        let mut ternary_matrix = Vec::with_capacity(in_dim * out_dim);
        for i in 0..(in_dim * out_dim) {
            let byte_idx = data_start + (i * 2) % (info.data_offsets[1] - info.data_offsets[0]);
            let byte_val = mmap[byte_idx] as i8;
            let ternary = if byte_val > 40 { 1 } else if byte_val < -40 { -1 } else { 0 };
            ternary_matrix.push(ternary);
        }

        let t_build = Instant::now();
        let out_indices = builder.compile_linear_layer(in_dim, out_dim, &ternary_matrix);
        let build_time = t_build.elapsed();

        println!("R1CS Circuit Gates: {}", builder.gates.len());
        println!("Total Operands in L1 Cache: {}", builder.n_operands);
        println!("Circuit compilation time: {:?}", build_time);

        // Бенчмарк прямого исполнения (Direct L1/L3 Execution)
        let mut operands = vec![1.0f32; builder.n_operands];
        let n_iters = 10_000;
        let t_exec = Instant::now();
        for _ in 0..n_iters {
            execute_circuit_direct(&builder, &mut operands);
        }
        let exec_time = t_exec.elapsed();
        let time_per_pass = exec_time.as_secs_f64() / n_iters as f64;
        let tok_per_sec = 1.0 / time_per_pass;

        println!("Execution speed (Direct R1CS): {:.2} µs/pass -> {:.1} passes/sec", time_per_pass * 1e6, tok_per_sec);
        assert!(!out_indices.is_empty());
    }
}
