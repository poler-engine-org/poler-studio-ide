use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("morpheme_crystals.rs");
    let mut f = File::create(&dest_path).unwrap();

    // 256 crystalline morpheme slots (K=4, 4 trits per crystal = 2^8 combinations)
    // Structured across rust/math keywords, operators, and phonetic syllables
    let mut table = [0u32; 256];

    // Seed essential atomic tokens (ASCII packed into u32 LE)
    let core_tokens: &[(&str, usize)] = &[
        ("fn ", 1),
        ("let ", 2),
        ("mut ", 3),
        ("impl", 4),
        ("for ", 5),
        ("loop", 6),
        ("true", 7),
        ("fals", 8),
        ("self", 9),
        ("type", 10),
        ("pub ", 11),
        ("mod ", 12),
        ("use ", 13),
        ("enum", 14),
        ("str ", 15),
        ("i64 ", 16),
        ("f64 ", 17),
        ("u32 ", 18),
        ("vec ", 19),
        ("ret ", 20),
        ("psi ", 21),
        ("phi ", 22),
        ("lens", 23),
        ("fock", 24),
        ("born", 25),
        ("step", 26),
        ("loss", 27),
        ("gate", 28),
        ("p_st", 29),
        ("zero", 30),
        ("one ", 31),
    ];

    for &(tok, idx) in core_tokens {
        let bytes = tok.as_bytes();
        let mut u = 0u32;
        for (i, &b) in bytes.iter().enumerate().take(4) {
            u |= (b as u32) << (i * 8);
        }
        table[idx] = u;
    }

    writeln!(
        f,
        "/// Предвычисленная AOT-таблица морфемных кристаллов (256 x 4 байта = 1024 B, L1 cache)"
    )
    .unwrap();
    writeln!(f, "pub static MORPHEME_CRYSTALS: [u32; 256] = [").unwrap();
    for val in table.iter() {
        writeln!(f, "    0x{:08X},", val).unwrap();
    }
    writeln!(f, "];").unwrap();

    println!("cargo:rerun-if-changed=build.rs");
}
