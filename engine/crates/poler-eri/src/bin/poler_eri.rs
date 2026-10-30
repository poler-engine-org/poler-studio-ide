#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! POLER-ERI v3.1.0 CLI — Quantum-Inspired ERI Meta-Compiler
//!
//! Usage:
//!   poler-eri                  — Run test suite
//!   poler-eri compile          — Batch-compile all quartets up to MAX_AM
//!   poler-eri stats            — Print statistics without writing files
//!   poler-eri generate         — Generate code for a single quartet
//!   poler-eri transpile        — Parse a C source file and extract patterns
//!   poler-eri validate         — Run built-in validation tests
//!   poler-eri archetype        — Show archetype info for a quartet
//!   poler-eri verify           — Crypto-verify all archetypes
//!   poler-eri reverse          — Reverse-compile source file → archetypes
//!   poler-eri one-key          — Full one-keystroke pipeline
//!   poler-eri dashboard        — Launch HTTP dashboard

use poler_eri::*;
use std::env;
use std::fs;


fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        run_test_suite();
        return;
    }

    match args[1].as_str() {
        "compile" => cmd_compile(&args[2..]),
        "stats" => cmd_stats(&args[2..]),
        "generate" => cmd_generate(&args[2..]),
        "transpile" => cmd_transpile(&args[2..]),
        "validate" => cmd_validate(),
        "archetype" => cmd_archetype(&args[2..]),
        "verify" => cmd_verify(&args[2..]),
        "reverse" => cmd_reverse(&args[2..]),
        "one-key" => cmd_one_key(&args[2..]),
        "dashboard" => cmd_dashboard(&args[2..]),
        "help" | "--help" | "-h" => cmd_help(),
        other => {
            eprintln!("Unknown command: '{}'", other);
            eprintln!("Run 'poler-eri help' for usage information.");
            std::process::exit(1);
        }
    }
}

// ── Subcommand implementations ────────────────────────────────────────────

/// `poler-eri compile --max-am N --output DIR`
fn cmd_compile(args: &[String]) {
    let max_am = parse_flag(args, "--max-am", 3);
    let output = parse_flag_str(args, "--output", "src/autogen");

    println!("╔══════════════════════════════════════════════════╗");
    println!("║  POLER-ERI v3.1.0 BatchCompiler                  ║");
    println!("╠══════════════════════════════════════════════════╣");
    println!("║  MAX_AM: {} ({})                             ", max_am, shell_name(max_am));
    println!("║  Output: {}                              ", output);
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    let batch = BatchCompiler::new(max_am);
    println!("Compiling all shell quartets...");
    let modules = batch.compile_all();

    BatchCompiler::print_summary(&modules);

    // Detailed module list
    println!("\n┌──────────┬────────────┬─────────┬────────┐");
    println!("│ Quartet  │ Module     │  Gates  │ Bytes  │");
    println!("├──────────┼────────────┼─────────┼────────┤");
    for m in &modules {
        let (la, lb, lc, ld) = m.quartet;
        println!("│ ({0}{1}|{2}{3})  │ {4:<10} │ {5:>5}   │ {6:>6} │",
            shell_name(la), shell_name(lb), shell_name(lc), shell_name(ld),
            m.name, m.n_gates, m.code.len());
    }
    println!("└──────────┴────────────┴─────────┴────────┘");

    // Write to files
    match BatchCompiler::write_to_dir(&modules, &output) {
        Ok(_) => {
            println!("\n  Written {} modules to {}", modules.len(), output);
        }
        Err(e) => {
            eprintln!("\n  Error writing modules: {}", e);
            std::process::exit(1);
        }
    }

    // Final statistics
    let total_gates: usize = modules.iter().map(|m| m.n_gates).sum();
    let total_bytes: usize = modules.iter().map(|m| m.code.len()).sum();
    println!("\n=== Summary ===");
    println!("  Modules:    {}", modules.len());
    println!("  R1CS gates: {}", total_gates);
    println!("  Code size:  {} bytes ({:.1} KB)", total_bytes, total_bytes as f64 / 1024.0);
}

/// `poler-eri stats --max-am N`
fn cmd_stats(args: &[String]) {
    let max_am = parse_flag(args, "--max-am", 5);

    println!("╔══════════════════════════════════════════════════╗");
    println!("║  POLER-ERI v3.1.0 Statistics                     ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    for am in 0..=max_am {
        let batch = BatchCompiler::new(am);
        let modules = batch.compile_all();
        let total_gates: usize = modules.iter().map(|m| m.n_gates).sum();
        let total_bytes: usize = modules.iter().map(|m| m.code.len()).sum();
        println!("  MAX_AM={} ({}): {:>3} modules, {:>6} gates, {:>8} bytes ({:.1} KB)",
            am, shell_name(am), modules.len(), total_gates, total_bytes, total_bytes as f64 / 1024.0);
    }
}

/// `poler-eri generate --quartet la,lb,lc,ld`
fn cmd_generate(args: &[String]) {
    let quartet_str = parse_flag_str(args, "--quartet", "1,1,1,1");
    let parts: Vec<usize> = quartet_str.split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    if parts.len() != 4 {
        eprintln!("Invalid quartet format: '{}'. Use la,lb,lc,ld (e.g. 2,0,1,1)", quartet_str);
        std::process::exit(1);
    }

    let (la, lb, lc, ld) = (parts[0], parts[1], parts[2], parts[3]);
    println!("Generating code for ({0}{1}|{2}{3})...",
        shell_name(la), shell_name(lb), shell_name(lc), shell_name(ld));

    let module = BatchCompiler::compile_one(la, lb, lc, ld);

    println!("\n=== Generated Code ===\n");
    println!("{}", module.code);
    println!("\n=== Circuit Info ===");
    println!("  Name:    {}", module.name);
    println!("  Quartet: ({0}{1}|{2}{3})", shell_name(la), shell_name(lb), shell_name(lc), shell_name(ld));
    println!("  Gates:   {}", module.n_gates);
    println!("  Size:    {} bytes", module.code.len());
}

/// `poler-eri transpile --input FILE`
fn cmd_transpile(args: &[String]) {
    let input = parse_flag_str(args, "--input", "");

    if input.is_empty() {
        eprintln!("No input file specified. Use --input <path>");
        std::process::exit(1);
    }

    println!("╔══════════════════════════════════════════════════╗");
    println!("║  POLER-ERI v3.1.0 C Transpiler                  ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    let content = match fs::read_to_string(&input) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading '{}': {}", input, e);
            std::process::exit(1);
        }
    };

    let parser = CParser::new();
    let patterns = parser.parse_file(&content);
    println!("Found {} recurrence patterns", patterns.len());

    for p in &patterns {
        println!("\n  {} -> ({},{},{},{}): {} gates, {} variables",
            p.fn_name,
            p.angular_momenta.0, p.angular_momenta.1,
            p.angular_momenta.2, p.angular_momenta.3,
            p.gates.len(), p.variables.len());
    }

    // Transpile each pattern
    let transpiler = Transpiler::new(crate::transpiler::TranspileConfig::default());
    let result = transpiler.transpile_file(&content);

    println!("\n=== Transpilation Result ===");
    println!("  Transpiled: {}", result.n_transpiled);
    println!("  Failed:     {}", result.n_failed);

    for warning in &result.warnings {
        println!("  Warning: {}", warning);
    }

    for (name, code) in &result.generated_code {
        println!("\n--- {} ---", name);
        let preview_len = code.len().min(500);
        println!("{}", &code[..preview_len]);
        if code.len() > 500 {
            println!("  ... ({} more bytes)", code.len() - 500);
        }
    }
}

/// `poler-eri archetype --quartet la,lb,lc,ld`
fn cmd_archetype(args: &[String]) {
    let quartet_str = parse_flag_str(args, "--quartet", "1,1,1,1");
    let parts: Vec<usize> = quartet_str.split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    if parts.len() != 4 {
        eprintln!("Invalid quartet format: '{}'. Use la,lb,lc,ld", quartet_str);
        std::process::exit(1);
    }

    let (la, lb, lc, ld) = (parts[0], parts[1], parts[2], parts[3]);
    let eps = archetype::Epsilon::one();

    println!("╔══════════════════════════════════════════════════╗");
    println!("║  POLER-ERI v3.1.0 Archetype Inspector            ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    let mut a = archetype::Archetype::new(la, lb, lc, ld, eps);

    println!("  Quartet:     ({0}{1}|{2}{3})",
        shell_name(la), shell_name(lb), shell_name(lc), shell_name(ld));
    println!("  Gates:       {}", a.circuit.gates.len());
    println!("  Operands:    {}", a.circuit.n_operands);
    println!("  Prefactors:  {}", a.circuit.n_prefactors);
    println!("  Fingerprint: {:016x}", a.fingerprint);
    println!();

    // Verify idempotency
    let idem = a.verify_idempotency();
    println!("  Idempotency (a x a = a): {}", if idem { "PASS" } else { "FAIL" });

    // Crystallize
    let code = a.crystallize();
    println!("  Crystallized: {} bytes", code.len());

    // Verify fixed point
    let fp = a.verify_fixed_point();
    println!("  Fixed-point (p* = a x p*): {}", if fp { "PASS" } else { "FAIL" });
    println!();

    // Show the archetype equation
    println!("  Archetype Equation:");
    println!("    a x_e a = a  (idempotency)");
    println!("    p* = a x_e p*  (fixed point)");
    println!();

    // Show crystallized code preview
    let preview_len = code.len().min(500);
    println!("  Code preview:");
    println!("{}", &code[..preview_len]);
    if code.len() > 500 {
        println!("  ... ({} more bytes)", code.len() - 500);
    }
}

/// `poler-eri verify --max-am N`
fn cmd_verify(args: &[String]) {
    let max_am = parse_flag(args, "--max-am", 2);

    println!("╔══════════════════════════════════════════════════╗");
    println!("║  POLER-ERI v3.1.0 Crypto Verification            ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    let eps = archetype::Epsilon::one();
    let verifier = crypto::CryptoVerifier::new(max_am, eps);

    println!("  MAX_AM: {}", max_am);
    println!("  Archetypes: {}", verifier.registry.len());
    println!("  Merkle Root: {:016x}", verifier.root_hash());
    println!();

    let results = verifier.verify_all();
    let n_ok = results.iter().filter(|r| r.all_ok()).count();
    let n_fail = results.len() - n_ok;

    println!("  Verification Results:");
    println!("  ┌──────────┬─────────┬────────────┬────────────┬────────────┐");
    println!("  │ Quartet  │ Idemp.  │ Fixed-pt   │ Fingerprint│  Status    │");
    println!("  ├──────────┼─────────┼────────────┼────────────┼────────────┤");
    for r in &results {
        let status = if r.all_ok() { "OK" } else { "FAIL" };
        println!("  │ {:<8} │ {:>5}   │ {:>5}      │ {:>5}      │ {:>6}     │",
            r.label,
            if r.idempotency_ok { "yes" } else { "NO" },
            if r.fixed_point_ok { "yes" } else { "NO" },
            if r.fingerprint_ok { "yes" } else { "NO" },
            status);
    }
    println!("  └──────────┴─────────┴────────────┴────────────┴────────────┘");
    println!();
    println!("  Total: {}/{} passed, {} failed", n_ok, results.len(), n_fail);

    if n_fail > 0 {
        std::process::exit(1);
    }
}

/// `poler-eri reverse --input FILE`
fn cmd_reverse(args: &[String]) {
    let input = parse_flag_str(args, "--input", "");

    if input.is_empty() {
        eprintln!("No input file specified. Use --input <path>");
        std::process::exit(1);
    }

    println!("╔══════════════════════════════════════════════════╗");
    println!("║  POLER-ERI v3.1.0 Reverse Meta-Compiler          ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    let content = match fs::read_to_string(&input) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading '{}': {}", input, e);
            std::process::exit(1);
        }
    };

    let rc = reverse::ReverseCompiler::new(archetype::Epsilon::one());
    let result = rc.process_file(&content, &input);

    println!("  Patterns found:   {}", result.patterns.len());
    println!("  Archetypes built: {}", result.archetypes.len());
    println!("  Signed modules:   {}", result.signed_modules.len());
    println!("  Unmappable:       {}", result.n_unmappable);
    println!("  Merkle Root:      {:016x}", result.merkle_root);
    println!();

    for signed in &result.signed_modules {
        let (la, lb, lc, ld) = signed.module.quartet;
        println!("  [{}] ({}{}|{}{}) gates={} fp={:016x} verified={}",
            signed.module.name,
            shell_name(la), shell_name(lb), shell_name(lc), shell_name(ld),
            signed.module.n_gates,
            signed.fingerprint,
            signed.verified);
    }
}

/// `poler-eri one-key --max-am N`
fn cmd_one_key(args: &[String]) {
    let max_am = parse_flag(args, "--max-am", 2);

    println!("╔══════════════════════════════════════════════════╗");
    println!("║  POLER-ERI v3.1.0 One-Keystroke Pipeline         ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();
    println!("  MAX_AM: {}", max_am);
    println!();

    // Step 1: Build archetypes
    print!("  [1/4] Building archetypes... ");
    let (modules, results, root) = crypto::CryptoVerifier::one_keystroke(max_am);
    println!("{} archetypes", modules.len());

    // Step 2: Crystallize
    print!("  [2/4] Crystallizing... ");
    let total_gates: usize = modules.iter().map(|m| m.n_gates).sum();
    let total_bytes: usize = modules.iter().map(|m| m.code.len()).sum();
    println!("{} gates, {} bytes", total_gates, total_bytes);

    // Step 3: Verify
    print!("  [3/4] Verifying... ");
    let n_ok = results.iter().filter(|r| r.all_ok()).count();
    println!("{}/{} passed", n_ok, results.len());

    // Step 4: Sign
    println!("  [4/4] Merkle Root: {:016x}", root);

    // Summary
    println!();
    println!("  === One-Keystroke Complete ===");
    println!("  Modules:   {}", modules.len());
    println!("  Gates:     {}", total_gates);
    println!("  Code:      {:.1} KB", total_bytes as f64 / 1024.0);
    println!("  Verified:  {}/{}", n_ok, results.len());
    println!("  Root Hash: {:016x}", root);

    if n_ok < results.len() {
        println!();
        println!("  WARNING: Some archetypes failed verification!");
        std::process::exit(1);
    }
}

/// `poler-eri validate`
fn cmd_validate() {
    println!("╔══════════════════════════════════════════════════╗");
    println!("║  POLER-ERI v3.1.0 Validation                    ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    let mut passed = 0;
    let mut failed = 0;

    // Test 1: Boys function
    print!("  [1/10] Boys function... ");
    let f0_zero = boys_f(0, 0.0);
    if (f0_zero - 1.0).abs() < 1e-10 {
        println!("OK (F_0(0) = {})", f0_zero);
        passed += 1;
    } else {
        println!("FAIL (F_0(0) = {}, expected 1.0)", f0_zero);
        failed += 1;
    }

    // Test 2: (ss|ss) ERI
    print!("  [2/10] ERI (ss|ss)... ");
    let ra = Point([0.0, 0.0, 0.0]);
    let qd = QuartetData::new(ra, ra, ra, ra, 1.0, 1.0, 1.0, 1.0);
    let eri_val = vrr::eri_ss_ss(&qd);
    if eri_val > 0.0 {
        println!("OK (value = {:.6})", eri_val);
        passed += 1;
    } else {
        println!("FAIL (value = {:.6}, expected > 0)", eri_val);
        failed += 1;
    }

    // Test 3: Circuit (ss|ss)
    print!("  [3/10] Circuit (ss|ss)... ");
    let c_ss = CircuitBuilder::new(0, 0, 0, 0).build_circuit();
    if c_ss.gates.is_empty() && c_ss.n_operands == 1 {
        println!("OK (0 gates, 1 operand)");
        passed += 1;
    } else {
        println!("FAIL ({} gates, {} operands)", c_ss.gates.len(), c_ss.n_operands);
        failed += 1;
    }

    // Test 4: Circuit (pp|pp) has all 5 stages
    print!("  [4/10] Circuit (pp|pp)... ");
    let c_pp = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
    let stages: std::collections::HashSet<usize> = c_pp.gates.iter().map(|g| g.stage).collect();
    if stages.len() == 5 {
        println!("OK ({} gates, 5 stages)", c_pp.gates.len());
        passed += 1;
    } else {
        println!("FAIL ({} stages, expected 5)", stages.len());
        failed += 1;
    }

    // Test 5: Crystallizer
    print!("  [5/10] Crystallizer... ");
    let code = VrrCrystallizer::crystallize(&c_pp, "eri_pppp");
    if code.contains("eri_pppp") && code.contains("VRR-Bra") {
        println!("OK ({} bytes)", code.len());
        passed += 1;
    } else {
        println!("FAIL");
        failed += 1;
    }

    // Test 6: BatchCompiler
    print!("  [6/10] BatchCompiler... ");
    let batch = BatchCompiler::new(1);
    let modules = batch.compile_all();
    if modules.len() >= 9 {
        println!("OK ({} modules for MAX_AM=1)", modules.len());
        passed += 1;
    } else {
        println!("FAIL ({} modules, expected >= 9)", modules.len());
        failed += 1;
    }

    // Test 7: Molecule
    print!("  [7/10] Molecule Li40... ");
    let li40 = Molecule::li40();
    if li40.atoms.len() == 40 {
        println!("OK ({} atoms, E_nuc = {:.4})", li40.atoms.len(), li40.nuclear_repulsion());
        passed += 1;
    } else {
        println!("FAIL ({} atoms, expected 40)", li40.atoms.len());
        failed += 1;
    }

    // Test 8: C Parser
    print!("  [8/10] C Parser... ");
    let parser = CParser::new();
    let test_c = "static inline void _g0_2d4d_0111(double * g, Rys2eT *bc, CINTEnvVars *envs) { g[0] = 1; }\n";
    let patterns = parser.parse_file(test_c);
    if patterns.len() == 1 {
        println!("OK (found 1 pattern)");
        passed += 1;
    } else {
        println!("FAIL (found {} patterns, expected 1)", patterns.len());
        failed += 1;
    }

    // Test 9: Archetype idempotency
    print!("  [9/10] Archetype idempotency (a x a = a)... ");
    let mut arch = archetype::Archetype::new(1, 1, 1, 1, archetype::Epsilon::one());
    arch.crystallize();
    if arch.verify_idempotency() && arch.verify_fixed_point() {
        println!("OK");
        passed += 1;
    } else {
        println!("FAIL");
        failed += 1;
    }

    // Test 10: Crypto verification
    print!("  [10/10] Crypto Merkle tree... ");
    let verifier = crypto::CryptoVerifier::new(1, archetype::Epsilon::one());
    let results = verifier.verify_all();
    let all_ok = results.iter().all(|r| r.all_ok());
    if all_ok {
        println!("OK ({}/{} archetypes verified, root={:016x})",
            results.len(), results.len(), verifier.root_hash());
        passed += 1;
    } else {
        println!("FAIL");
        failed += 1;
    }

    println!("\n=== Results: {}/{} passed ===", passed, passed + failed);
    if failed > 0 {
        std::process::exit(1);
    }
}

/// `poler-eri dashboard --port N`
fn cmd_dashboard(args: &[String]) {
    let port = parse_flag(args, "--port", 8080);
    println!("╔══════════════════════════════════════════════════╗");
    println!("║  POLER-ERI v3.1.0 Dashboard                     ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();
    println!("Dashboard would start on http://localhost:{}/", port);
    println!("(HTTP server not yet implemented - use CLI commands instead)");
    println!();
    println!("Available commands:");
    println!("  poler-eri compile --max-am 3 --output src/autogen");
    println!("  poler-eri stats --max-am 5");
    println!("  poler-eri generate --quartet 2,1,2,1");
    println!("  poler-eri transpile --input ../libcint/src/g2e.c");
    println!("  poler-eri archetype --quartet 2,1,2,1");
    println!("  poler-eri verify --max-am 3");
    println!("  poler-eri reverse --input source.rs");
    println!("  poler-eri one-key --max-am 3");
    println!("  poler-eri validate");
}

/// Print help message.
fn cmd_help() {
    println!("POLER-ERI v3.1.0 - Quantum-Inspired ERI Meta-Compiler + Archetype Algebra");
    println!();
    println!("USAGE:");
    println!("  poler-eri                    Run test suite");
    println!("  poler-eri <COMMAND> [FLAGS]  Run a subcommand");
    println!();
    println!("COMMANDS:");
    println!("  compile    Batch-compile all quartets up to MAX_AM");
    println!("  stats      Print statistics without writing files");
    println!("  generate   Generate code for a single quartet");
    println!("  transpile  Parse a C source file and extract patterns");
    println!("  validate   Run built-in validation tests (10 checks)");
    println!("  archetype  Show archetype info and verify idempotency");
    println!("  verify     Crypto-verify all archetypes (Merkle tree)");
    println!("  reverse    Reverse-compile source file to archetypes");
    println!("  one-key    Full one-keystroke pipeline (compile+verify+sign)");
    println!("  dashboard  Launch HTTP dashboard (not yet implemented)");
    println!("  help       Show this help message");
    println!();
    println!("FLAGS:");
    println!("  --max-am N       Maximum angular momentum (default: 3)");
    println!("  --output DIR     Output directory (default: src/autogen)");
    println!("  --quartet L,L,L,L  Shell quartet (e.g. 2,1,2,1)");
    println!("  --input FILE     Input source file (C, Python, or Rust)");
    println!("  --port N         HTTP port (default: 8080)");
    println!();
    println!("ARCHETYPE EQUATION:");
    println!("  a x_e a = a    (idempotency: re-compilation produces same code)");
    println!("  p* = a x_e p*  (fixed point: SCF convergence guaranteed)");
    println!();
    println!("  where A = (O, +, x_e) is the Algebra of Senses and");
    println!("  a is a non-trivial idempotent element (archetype)");
}

// ── Built-in test suite (run when no command is given) ─────────────────────

fn run_test_suite() {
    println!("╔══════════════════════════════════════════════════╗");
    println!("║  POLER-ERI v3.1.0 Meta-Compiler Test Suite       ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    // Boys function
    println!("--- Boys Function ---");
    let f0 = boys_f(0, 0.0);
    println!("  F_0(0) = {} (expected 1.0)", f0);
    let f0_1 = boys_f(0, 1.0);
    println!("  F_0(1) = {:.6} (expected ~0.7468)", f0_1);

    // Overlap
    println!("\n--- Overlap ---");
    let s_ss = overlap::overlap_ss(1.0, 1.0, 0.0);
    println!("  S(ss, same center) = {:.6}", s_ss);

    // (ss|ss) ERI
    println!("\n--- ERI (ss|ss) ---");
    let ra = Point([0.0, 0.0, 0.0]);
    let qd = QuartetData::new(ra, ra, ra, ra, 1.0, 1.0, 1.0, 1.0);
    let eri_val = vrr::eri_ss_ss(&qd);
    println!("  (ss|ss) same center = {:.6}", eri_val);

    // Circuit + Crystallizer
    println!("\n--- Circuit & Crystallizer ---");
    let c_ss = CircuitBuilder::new(0, 0, 0, 0).build_circuit();
    println!("  (ss|ss) circuit: {} gates, {} operands", c_ss.gates.len(), c_ss.n_operands);

    let c_pp = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
    println!("  (pp|pp) circuit: {} gates, {} operands", c_pp.gates.len(), c_pp.n_operands);

    let code_pp = VrrCrystallizer::crystallize(&c_pp, "eri_pppp");
    println!("  Crystallized (pp|pp): {} bytes", code_pp.len());
    println!("  Preview: {}", &code_pp[..code_pp.len().min(200)]);

    // BatchCompiler
    println!("\n--- BatchCompiler ---");
    let batch = BatchCompiler::new(2);
    let modules = batch.compile_all();
    BatchCompiler::print_summary(&modules);

    // Molecule
    println!("\n--- Molecule ---");
    let h2 = Molecule::h2(1.4);
    println!("  H2: {} atoms, {} electrons, E_nuc = {:.6}",
        h2.atoms.len(), h2.n_electrons(), h2.nuclear_repulsion());

    let li40 = Molecule::li40();
    println!("  Li40: {} atoms, {} electrons, E_nuc = {:.4}",
        li40.atoms.len(), li40.n_electrons(), li40.nuclear_repulsion());

    // Archetype (v3.1.0)
    println!("\n--- Archetype Algebra ---");
    let mut arch = archetype::Archetype::new(1, 1, 1, 1, archetype::Epsilon::one());
    let _code = arch.crystallize();
    println!("  (pp|pp) archetype: {} gates, fingerprint={:016x}", arch.circuit.gates.len(), arch.fingerprint);
    println!("  Idempotency (a x a = a): {}", if arch.verify_idempotency() { "PASS" } else { "FAIL" });
    println!("  Fixed-point (p* = a x p*): {}", if arch.verify_fixed_point() { "PASS" } else { "FAIL" });

    // Crypto (v3.1.0)
    println!("\n--- Crypto Verification ---");
    let verifier = crypto::CryptoVerifier::new(1, archetype::Epsilon::one());
    println!("  Merkle Root: {:016x}", verifier.root_hash());
    let results = verifier.verify_all();
    let n_ok = results.iter().filter(|r| r.all_ok()).count();
    println!("  Verified: {}/{} archetypes", n_ok, results.len());

    // C Parser
    println!("\n--- C Parser ---");
    let _parser = CParser::new();
    println!("  CParser ready. Use 'poler-eri transpile --input <file>' to parse C source.");

    println!("\n=== All tests complete ===");
}

// ── CLI helpers ────────────────────────────────────────────────────────────

fn parse_flag(args: &[String], flag: &str, default: usize) -> usize {
    for i in 0..args.len() {
        if args[i] == flag {
            if i + 1 < args.len() {
                return args[i + 1].parse().unwrap_or(default);
            }
        }
    }
    default
}

fn parse_flag_str(args: &[String], flag: &str, default: &str) -> String {
    for i in 0..args.len() {
        if args[i] == flag {
            if i + 1 < args.len() {
                return args[i + 1].clone();
            }
        }
    }
    default.to_string()
}
