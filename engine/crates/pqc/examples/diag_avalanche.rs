#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Диагностика RQ23: честное сравнение лавины v1 (линейный транспорт)
//! против v2 (транспорт + спин) на одинаковых ключах/сообщениях/IV.

use pqc::trite::{self, TritKey};
use pqc::Rng;
use pqw::{GyroData, Lexicon, PqwWriter};

fn build_key(d: u32) -> TritKey {
    // Синтетический мозг: кластеризованные русла + лексикон (для v4).
    let mut pairs = Vec::new();
    let mut k = 1u64;
    let next = |k: &mut u64| -> u64 {
        *k = k.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *k
    };
    while pairs.len() < 600 {
        let h = next(&mut k);
        let i = (h % (d as u64 / 2)) as u32;
        let j = d as u32 - 1 - ((h >> 32) % 64) as u32;
        if i < j {
            let w = if h & 1 == 0 { 1.0 } else { -1.0 };
            if !pairs.iter().any(|&(pi, pj, _)| pi == i && pj == j) {
                pairs.push((i, j, w));
            }
        }
    }
    let g = GyroData::new(8, 12345, pairs, d).unwrap();
    let lex = Lexicon::new(vec![(0, "фаза".into()), (1, "трит".into())], d).unwrap();
    let mut w = PqwWriter::new(d).unwrap();
    w.add_phase(0, 0.9).unwrap();
    let bytes = w.to_bytes_v4(&g, &lex).unwrap();
    let reader = pqw::PqwReader::from_bytes(&bytes).unwrap();
    TritKey::from_reader(&reader, 0).unwrap()
}

fn avalanche_of(key: &TritKey, msg: &[u8], version: u16, flip: usize) -> f64 {
    let iv_seed = 777;
    let mut rng = Rng::seed_from_u64(iv_seed);
    let (_, data, _) = key.encrypt_body_version(msg, &mut rng, version);
    let mut flipped = msg.to_vec();
    flipped[flip] ^= 1;
    let mut rng2 = Rng::seed_from_u64(iv_seed);
    let (_, data2, _) = key.encrypt_body_version(&flipped, &mut rng2, version);
    let n = data.len() * 4;
    let t1 = trite::unpack_trites(&data, n);
    let t2 = trite::unpack_trites(&data2, n);
    t1.iter().zip(t2.iter()).filter(|(a, b)| a != b).count() as f64 / n as f64
}

fn main() {
    for &d in &[4096usize, 2048] {
        let key = build_key(d as u32);
        println!("=== d_pol={d}, ticks_v1={}, ticks_v2={} ===", key.ticks, key.ticks_v2());
        for &len in &[600usize, 3000, 16384] {
            let msg: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            let blocks = (len * 8 / 30 + key.capacity_trites - 1) / key.capacity_trites.max(1);
            for &flip in &[0, len / 2, len - 1] {
                let av1 = avalanche_of(&key, &msg, 1, flip);
                let av2 = avalanche_of(&key, &msg, 2, flip);
                println!(
                    "len={len:6} blocks≈{blocks:3} flip={flip:6}: v1={av1:.3} v2={av2:.3} Δ={:+.3}",
                    av2 - av1
                );
            }
        }
    }
}
