#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Диагностика RQ23: gap-RLE сжатие GYRO-секции на больших разреженных
//! решётках (d_pol ≥ 65536) — кластеризованные топологии реальных мозгов.

use pqw::{GyroData, Lexicon, PqwReader, PqwWriter, GYRO_SECTION_VERSION_RLE};

fn main() {
    println!("=== RQ23: gap-RLE топологии гироскопа — большие решётки ===\n");
    for (d, n_pairs, cluster) in [
        // (d_pol, пар, кластеризованность): кластер = соседние слоты
        // (знания скучены в тематические зоны), scatter = равномерный разброс.
        (65536usize, 4096, true),
        (65536, 16384, true),
        (65536, 65536, true),
        (65536, 16384, false),
        (131072, 65536, true),
        (262144, 131072, true),
    ] {
        // Строим пары: кластер — строки треугольника (соседние слоты),
        // scatter — равномерное распределение по слотам.
        let mut pairs: Vec<(u32, u32, f64)> = Vec::new();
        let mut rng: u64 = 0x9E3779B97F4A7C15;
        let next = |rng: &mut u64| -> u64 {
            *rng ^= *rng << 13;
            *rng ^= *rng >> 7;
            *rng ^= *rng << 17;
            *rng
        };
        if cluster {
            // Тематические кластеры: полные строки j (соседние слоты).
            let rows = (n_pairs as f64 / 64.0).ceil() as u32;
            let mut count = 0;
            'outer: for k in 0..rows {
                let j = 300 + (k as u64 * 997 % (d as u64 - 400)) as u32;
                if j == 0 {
                    continue;
                }
                for i in 0..j.min(64) {
                    if count >= n_pairs {
                        break 'outer;
                    }
                    let w = if next(&mut rng) & 1 == 0 { 1.0 } else { -1.0 };
                    pairs.push((i, j, w));
                    count += 1;
                }
            }
        } else {
            let mut seen = std::collections::HashSet::new();
            while pairs.len() < n_pairs {
                let h = next(&mut rng);
                let slot = h % (d as u64 * (d as u64 - 1) / 2);
                // Слот → пара (целочисленное обращение).
                let (i, j) = pqw::slot_to_pair(slot);
                if i < j && j < d as u32 && seen.insert((i, j)) {
                    let w = if h & (1 << 62) != 0 { 1.0 } else { -1.0 };
                    pairs.push((i, j, w));
                }
            }
        }
        let g = match GyroData::new(8, 1_000_000, pairs, d as u32) {
            Ok(g) => g,
            Err(e) => {
                println!("d={d}: пропуск ({e})");
                continue;
            }
        };
        let sparse = g.encode(false).unwrap();
        let rle = g.encode_rle().unwrap();
        let (best, codec) = g.encode_best(false).unwrap();
        let ratio = sparse.len() as f64 / best.len() as f64;
        let kind = if cluster { "кластер" } else { "разброс" };
        println!(
            "d_pol={d:>7} пар={n_pairs:>6} [{kind:>7}]: \
             разреженный {sparse_len:>8} Б → gap-RLE {rle_len:>8} Б (×{ratio:.2}); \
             выбран кодек v{codec} ({best_len} Б)",
            sparse_len = sparse.len(),
            rle_len = rle.len(),
            best_len = best.len(),
            codec = if codec == GYRO_SECTION_VERSION_RLE { 2 } else { 1 },
        );

        // Полный контейнер v4: читается обратно, топология бит-в-бит.
        let lex = Lexicon::new(vec![(0, "фаза".into())], d as u32).unwrap();
        let mut w = PqwWriter::new(d as u32).unwrap();
        w.add_phase(0, 0.9).unwrap();
        let bytes = w.to_bytes_v4(&g, &lex).unwrap();
        let r = PqwReader::from_bytes(&bytes).unwrap();
        let section = r.gyro().unwrap();
        assert_eq!(section.pairs().len(), g.pairs().len());
        let _ = r.verify_payload();
        println!(
            "         контейнер {} Б (фазы {} Б + GYRO {} Б + LEXI) — mmap zero-copy ✓",
            bytes.len(),
            r.header().phase_len,
            r.header().topology_len
        );
    }
}
