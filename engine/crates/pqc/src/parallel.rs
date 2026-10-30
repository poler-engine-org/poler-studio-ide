//! Минимальный fork-join поверх `std::thread::scope` — без rayon.
//!
//! Все разбиения детерминированы: потоки работают над непересекающимися
//! диапазонами, поэтому результат побитово воспроизводим независимо от
//! числа потоков. Порог `PAR_MIN_AMPS` отсекает случаи, где накладные
//! расходы на создание потоков не окупаются.

use std::thread::available_parallelism;

/// Минимальный объём работы для распараллеливания (32 768 амплитуд = 512 KiB).
pub(crate) const PAR_MIN_AMPS: usize = 1 << 15;

/// Потоков не больше, чем доступно ядрам, и не больше восьми.
pub fn worker_count() -> usize {
    available_parallelism().map(|n| n.get()).unwrap_or(1).min(8)
}

/// Применяет `f` к непересекающимся подсрезам `v`, нарезанным кратно `block`
/// (контракт: `v.len() % block == 0`; последний подсрез может быть короче,
/// но остаётся кратным `block`). При малом объёме — последовательный вызов.
pub(crate) fn par_blocks_mut<T, F>(v: &mut [T], block: usize, f: F)
where
    T: Send,
    F: Fn(&mut [T]) + Sync,
{
    let len = v.len();
    if block == 0 || !len.is_multiple_of(block) {
        f(v);
        return;
    }
    let threads = worker_count();
    let n_blocks = len / block;
    if threads <= 1 || len < PAR_MIN_AMPS || n_blocks < threads {
        f(v);
        return;
    }
    let per = (n_blocks / threads).max(1);
    let step = per * block;
    // Каждый подсрез — кратное block (включая последний: len и step кратны block).
    let parts: Vec<&mut [T]> = v.chunks_mut(step).collect();
    std::thread::scope(|s| {
        let f = &f;
        let mut handles = Vec::new();
        let mut first = None;
        for (i, part) in parts.into_iter().enumerate() {
            if i == 0 {
                first = Some(part);
            } else {
                // Замыкание владеет &F (F: Sync) и собственным &mut [T].
                handles.push(s.spawn(move || f(part)));
            }
        }
        if let Some(part) = first {
            f(part);
        }
        for h in handles {
            h.join().expect("pqc worker thread panicked");
        }
    });
}

/// Параллельно отображает `inp` в `out` (равные длины): `f` получает
/// непересекающиеся пары подсрезов. Последовательный путь при малом объёме.
pub(crate) fn par_zip_map<T, R, F>(inp: &[T], out: &mut [R], f: F)
where
    T: Sync,
    R: Send,
    F: Fn(&[T], &mut [R]) + Sync,
{
    debug_assert_eq!(inp.len(), out.len());
    let len = inp.len();
    let threads = worker_count();
    if threads <= 1 || len < PAR_MIN_AMPS {
        f(inp, out);
        return;
    }
    let step = (len / threads).max(1024);
    let parts: Vec<&mut [R]> = out.chunks_mut(step).collect();
    std::thread::scope(|s| {
        let f = &f;
        let mut handles = Vec::new();
        for (i, part) in parts.into_iter().enumerate() {
            let lo = i * step;
            let hi = lo + part.len();
            let inp_part = &inp[lo..hi];
            handles.push(s.spawn(move || f(inp_part, part)));
        }
        for h in handles {
            h.join().expect("pqc worker thread panicked");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn par_blocks_covers_all_elements() {
        let mut v = vec![0u64; 100_000];
        par_blocks_mut(&mut v, 4, |slice| {
            for x in slice.iter_mut() {
                *x += 1;
            }
        });
        assert!(v.iter().all(|&x| x == 1));
    }

    #[test]
    fn par_blocks_sequential_for_small() {
        let mut v = vec![7u64; 16];
        par_blocks_mut(&mut v, 4, |slice| {
            assert_eq!(slice.len(), 16); // без разбиения
        });
    }

    #[test]
    fn par_zip_map_writes_all() {
        let inp: Vec<u64> = (0..100_000u64).collect();
        let mut out = vec![0u64; inp.len()];
        par_zip_map(&inp, &mut out, |i, o| {
            for (a, b) in i.iter().zip(o.iter_mut()) {
                *b = a * 2;
            }
        });
        assert_eq!(out[0], 0);
        assert_eq!(out[99_999], 199_998);
        assert!(out.iter().enumerate().all(|(i, &x)| x == i as u64 * 2));
    }

    #[test]
    fn par_blocks_deterministic_result() {
        let mut a = vec![1u64; 1 << 20];
        let mut b = vec![1u64; 1 << 20];
        let op = |slice: &mut [u64]| {
            for x in slice.iter_mut() {
                *x = x.wrapping_mul(3).wrapping_add(7);
            }
        };
        par_blocks_mut(&mut a, 64, op);
        for x in b.iter_mut() {
            *x = x.wrapping_mul(3).wrapping_add(7);
        }
        assert_eq!(a, b);
    }
}
