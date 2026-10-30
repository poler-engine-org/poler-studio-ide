#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Интеграционная верификация Flat Crystallization: сгенерированное ядро
//! компилируется настоящим rustc и исполняется — результат сверяется
//! побитово с runtime-конвейером `MetaPipeline`.

use poler_engine::quantum::crystallizer::WeightCircuitBuilder;
use poler_engine::quantum::meta_compiler::{crystallize_to_flat_simd_rust, MetaPipeline};

struct XorShift32(u32);

impl XorShift32 {
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}

#[test]
fn test_flat_kernel_compiles_and_matches_runtime() {
    // rustc и AVX2 обязательны для исполнения сгенерированного ядра.
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    if std::process::Command::new(&rustc)
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("rustc недоступен — пропуск компиляции плоского ядра");
        return;
    }
    #[cfg(target_arch = "x86_64")]
    {
        if !is_x86_feature_detected!("avx2") {
            eprintln!("AVX2 недоступен — пропуск исполнения плоского ядра");
            return;
        }
    }

    // Детерминированная схема: тернарный слой + дубли строк (CSE) + скаляр.
    let mut rng = XorShift32(0xBEEF);
    let (in_d, out_d) = (48usize, 32usize);
    let mut ternary = vec![0i8; in_d * out_d];
    for i in 0..in_d * out_d {
        // Строки 3 и 4 идентичны строке 0 → CSE-склейка.
        ternary[i] = if i / in_d == 3 || i / in_d == 4 {
            ternary[i % in_d]
        } else {
            (rng.next() % 3) as i8 - 1
        };
    }
    let mk = || {
        let mut b = WeightCircuitBuilder::new(in_d);
        let oi = b.compile_linear_layer(in_d, out_d, &ternary);
        let extra = b.add_gate(
            oi[0],
            poler_engine::quantum::crystallizer::GateCoeff::Scalar(-1.75),
            oi[out_d - 1],
            poler_engine::quantum::crystallizer::GateCoeff::One,
            "scalar_mix",
        );
        (b, oi, extra)
    };
    let (b1, out_idx, extra) = mk();
    // Заявленные выходы: строки слоя + скалярный микс (иначе DCE корректно
    // вычистит его как недостижимый).
    let mut declared = out_idx.clone();
    declared.push(extra);
    let pipe = MetaPipeline::run_with_outputs(b1, &declared).expect("pipeline");

    // Входы и эталон (runtime-конвейер).
    let mut operands = vec![0.0f32; pipe.n_operands()];
    for v in operands[..in_d].iter_mut() {
        *v = (rng.next() % 199) as f32 / 11.0 - 9.0;
    }
    pipe.execute(&mut operands);

    let inputs: Vec<String> = (0..in_d).map(|i| format!("{:?}", operands[i])).collect();
    let mut checks = String::new();
    for &o in out_idx.iter() {
        let slot = pipe.slot_of(o).expect("выход жив");
        checks.push_str(&format!(
            "    check(&ops, {slot}, {:?});\n",
            operands[slot]
        ));
    }
    let extra_slot = pipe.slot_of(extra).expect("скаляр жив");
    checks.push_str(&format!(
        "    check(&ops, {extra_slot}, {:?});\n",
        operands[extra_slot]
    ));

    let kernel = crystallize_to_flat_simd_rust("poler_flat_kernel", &mk().0);
    let inputs_str = inputs.join(", ");

    // Автономная программа: ядро + проверщик.
    let program = format!(
        "{kernel}\n\
         fn check(ops: &[f32], slot: usize, want: f32) {{\n\
         \x20   if ops[slot].to_bits() != want.to_bits() {{\n\
         \x20       eprintln!(\"слот {{slot}}: {{}} против {{}}\", ops[slot], want);\n\
         \x20       std::process::exit(1);\n\
         \x20   }}\n\
         }}\n\
         fn main() {{\n\
         \x20   let inputs: [f32; {in_d}] = [{inputs_str}];\n\
         \x20   let mut ops = vec![0.0f32; N_OPERANDS];\n\
         \x20   ops[..{in_d}].copy_from_slice(&inputs);\n\
         \x20   unsafe {{ poler_flat_kernel(ops.as_mut_slice()) }};\n\
         {checks}\
         \x20   println!(\"OK: плоское ядро совпало с runtime-конвейером побитово\");\n\
         }}\n"
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("flat_kernel.rs");
    std::fs::write(&src, &program).expect("запись исходника ядра");
    let bin = dir.path().join("flat_kernel_bin");

    let t0 = std::time::Instant::now();
    let status = std::process::Command::new(&rustc)
        .arg("-O")
        .arg("--edition")
        .arg("2021")
        .arg(&src)
        .arg("-o")
        .arg(&bin)
        .status()
        .expect("запуск rustc");
    assert!(status.success(), "rustc не скомпилировал плоское ядро");
    eprintln!("rustc скомпилировал плоское ядро за {:?}", t0.elapsed());

    let out = std::process::Command::new(&bin)
        .output()
        .expect("запуск плоского ядра");
    assert!(
        out.status.success(),
        "плоское ядро разошлось с runtime: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("{}", String::from_utf8_lossy(&out.stdout).trim());
}
