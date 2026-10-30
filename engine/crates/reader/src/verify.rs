//! Верификация POLER Reader — «запуск > чтение > доверие».
//!
//! V1 детерминизм · V2 ротор+дисипация · V3 форманты в спектре ·
//! V4 F0 кепстром · V5 микротремор · V6 Π_Λ Мак-Віні · V7 реальное
//! время · V8 сжатие книги.

use crate::book::parse_text;
use crate::fft::{cepstral_f0, spectral_peaks};
use crate::linalg;
use crate::polerbook::{PhrasePassport, PolerBook};
use crate::rng::Xorshift64;
use crate::stream::{pack_book, render_phrases, RenderResult};
use crate::voice::{Archetype, LivingVoice};
use crate::FS;

/// Результат одного аксиом-чека.
#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

/// Прогнать полный suite на короткой книге. Возвращает (чеки, все-ок).
pub fn run_suite(voice_seed: u64) -> (Vec<Check>, bool) {
    let mut checks = Vec::new();
    let mut ok = |name: &'static str, cond: bool, detail: String| {
        checks.push(Check { name, ok: cond, detail });
        cond
    };

    let text = "Живой голос звучит и дышит. Ротор крутит форманты!";
    let phrases = parse_text(text);
    let mk_passports = |seed_off: u64| -> Vec<PhrasePassport> {
        phrases
            .iter()
            .enumerate()
            .map(|(i, p)| PhrasePassport {
                seed: seed_off + i as u64,
                arch: Archetype::ACalm,
                pause_ms: (p.pause_s * 1000.0) as u16,
            })
            .collect()
    };

    // ── V1: детерминизм ─────────────────────────────────────────────
    let a = render_phrases(&phrases, &mk_passports(100), voice_seed, Archetype::ACalm)
        .expect("render");
    let b = render_phrases(&phrases, &mk_passports(100), voice_seed, Archetype::ACalm)
        .expect("render");
    let c = render_phrases(&phrases, &mk_passports(100), voice_seed ^ 1, Archetype::ACalm)
        .expect("render");
    let same = a.samples.len() == b.samples.len()
        && a.samples
            .iter()
            .zip(b.samples.iter())
            .all(|(x, y)| x.to_bits() == y.to_bits());
    let v1a = ok(
        "V1a",
        same,
        format!("{} сэмплов бит-в-бит", a.samples.len()),
    );
    let differ = a
        .samples
        .iter()
        .zip(c.samples.iter())
        .filter(|(x, y)| x != y)
        .count();
    let v1b = ok("V1b", differ > 100, format!("{differ} отличий у другой личности"));
    let _ = (v1a, v1b);

    // ── V2: ротор сохраняет норму, дисипация убивает энергию ────────
    let mut g = Xorshift64::new(voice_seed);
    let couplings = crate::resonator::couplings_from_seed(&mut g);
    let tract = crate::resonator::Tract {
        formants: Archetype::ACalm.formants(),
        bandwidths: Archetype::ACalm.bandwidths(),
    };
    let ad_rot = crate::resonator::zoh_rotor(&tract, &couplings, FS as f64);
    let mut x = [0.3, -0.2, 0.5, 0.1, -0.4, 0.2];
    let e0 = linalg::vec_norm(&x);
    for _ in 0..20_000 {
        x = linalg::mat_vec(&ad_rot, &x);
    }
    let drift = (linalg::vec_norm(&x) - e0).abs();
    ok("V2a", drift < 1e-9, format!("дрейф нормы {drift:.2e}/20000 шагов"));

    let (ad_dis, _) = crate::resonator::zoh(&tract, &couplings, FS as f64);
    let mut x2 = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    for _ in 0..10_000 {
        x2 = linalg::mat_vec(&ad_dis, &x2);
    }
    let e_fin = linalg::vec_norm(&x2);
    ok("V2b", e_fin < 1e-3, format!("энергия без входа → {e_fin:.2e}"));

    // ── V3: форманты в спектре выдержанного «а» ────────────────────
    // (как в цикле K: прямой зонд LivingVoice на опорном архетипе —
// коартикуляционный поток проверяется отдельными тестами stream)
    let mut probe_voice = LivingVoice::new(voice_seed, Archetype::ACalm);
    let n_probe = (1.2 * FS as f64) as usize;
    let mut probe_out: Vec<f64> = Vec::with_capacity(n_probe);
    let f0p = Archetype::ACalm.f0();
    for _ in 0..n_probe {
        probe_out.push(probe_voice.sample(
            1.0,
            f0p,
            Archetype::ACalm.vibrato_hz(),
            Archetype::ACalm.jitter(),
            Archetype::ACalm.shimmer_db(),
        ));
    }
    // пропустить атаку (0.2 с) и взять 0.8 с стационара
    let steady = &probe_out[(0.2 * FS as f64) as usize..];
    let peaks = spectral_peaks(steady, FS, 6, 200.0, 90.0);
    let target = Archetype::ACalm.formants();
    let mut hits = 0;
    for &t in target.iter() {
        if peaks.iter().any(|&(f, rel)| rel > 0.20 && (f - t).abs() < 100.0) {
            hits += 1;
        }
    }
    ok(
        "V3",
        hits >= 2,
        format!("{hits}/3 формант среди пиков {peaks:?}"),
    );

    // ── V4: F0 кепстром (тот же выдержанный зонд) ───────────────────
    let f0_meas = cepstral_f0(steady, FS).unwrap_or(0.0);
    let f0_base = Archetype::ACalm.f0();
    ok(
        "V4",
        (f0_meas - f0_base).abs() / f0_base < 0.15,
        format!("F0 {f0_meas:.1} Гц против {f0_base:.1}"),
    );

    // ── V5: микротремор в физиологичных пределах ────────────────────
    let p = &a.passport;
    ok(
        "V5",
        p.jitter_std_pct > 0.1 && p.jitter_std_pct < 2.0 && p.shimmer_std_pct > 0.2 && p.shimmer_std_pct < 6.0,
        format!(
            "jitter {:.2}% shimmer {:.2}% (периодов {})",
            p.jitter_std_pct, p.shimmer_std_pct, p.n_periods
        ),
    );

    // ── V6: Π_Λ Мак-Віні на когерентности ───────────────────────────
    // Собираем подвыборку состояний ψ из свежего короткого рендера
    let mut voice = LivingVoice::new(voice_seed, Archetype::ACalm);
    let mut states: Vec<linalg::Vec6> = Vec::new();
    let mut stride = 0usize;
    for _i in 0..(FS as usize) {
        let y = voice.sample(1.0, f0_base, 5.2, 0.008, 1.0);
        let _ = y;
        stride += 1;
        if stride >= 21 && states.len() < 1024 {
            states.push(*voice.psi());
            stride = 0;
        }
    }
    // нормировка строк и матрица когерентности
    let m = states.len() as f64;
    let mut coh = linalg::zeros();
    for s in &states {
        let n = linalg::vec_norm(s).max(1e-12);
        for i in 0..6 {
            for j in 0..6 {
                coh[i][j] += (s[i] / n) * (s[j] / n) / m;
            }
        }
    }
    let eig = linalg::eig_sym(&coh);
    // масштаб λmax → 0.9 (0.5 — неподвижная точка Q, избегаем)
    let mut cs = coh;
    let lmax = eig[0].max(1e-12);
    for i in 0..6 {
        for j in 0..6 {
            cs[i][j] *= 0.9 / lmax;
        }
    }
    // Q(P) = 3P² − 2P³ до сходимости
    let mut pm = cs;
    let mut res = f64::INFINITY;
    let mut iters = 0usize;
    for it in 0..60 {
        let pp = linalg::mat_mul(&pm, &pm);
        let ppp = linalg::mat_mul(&pp, &pm);
        let mut q = linalg::zeros();
        for i in 0..6 {
            for j in 0..6 {
                q[i][j] = 3.0 * pp[i][j] - 2.0 * ppp[i][j];
            }
        }
        let pp2 = linalg::mat_mul(&q, &q);
        res = 0.0;
        for i in 0..6 {
            for j in 0..6 {
                res = res.max((pp2[i][j] - q[i][j]).abs());
            }
        }
        pm = q;
        iters = it + 1;
        if res < 1e-13 {
            break;
        }
    }
    ok(
        "V6",
        res < 1e-10,
        format!("Мак-Віни residual {res:.2e} за {iters} итераций"),
    );

    // ── V7: быстрее реального времени ───────────────────────────────
    let t0 = std::time::Instant::now();
    let _r7 = render_phrases(&phrases, &mk_passports(300), voice_seed, Archetype::ACalm)
        .expect("render");
    let dt = t0.elapsed().as_secs_f64();
    let rt = a.duration_s / dt;
    ok(
        "V7",
        rt > 1.0,
        format!(
            "RT ×{rt:.1} ({:.2} с звука за {dt:.2} с)",
            a.duration_s
        ),
    );

    // ── V8: сжатие книги против PCM ─────────────────────────────────
    let book = pack_book(text, voice_seed, Archetype::ACalm).expect("pack");
    let book_sz = book.size_bytes() as f64;
    let wav_sz = a.samples.len() as f64 * 2.0;
    let ratio = wav_sz / book_sz;
    ok(
        "V8",
        ratio > 100.0,
        format!("книга {book_sz:.0} Б против WAV {wav_sz:.0} Б = ×{ratio:.0}"),
    );

    let all = checks.iter().all(|c| c.ok);
    (checks, all)
}

/// Выдержанный кусок гласной из середины рендера (для diagnostics).
#[allow(dead_code)]
fn middle_vowel_slice(r: &RenderResult, frac: f64) -> Vec<f64> {
    let n = r.samples.len();
    let len = (0.35 * FS as f64) as usize; // 350 мс
    let start = ((n.saturating_sub(len)) as f64 * frac) as usize;
    r.samples[start..start + len.min(n - start)].to_vec()
}

/// Сформировать текстовый отчёт.
pub fn report(checks: &[Check], all: bool) -> String {
    let mut s = String::new();
    s.push_str("═ POLER READER VERIFICATION ═\n");
    for c in checks {
        s.push_str(&format!(
            "  [{}] {} — {}\n",
            if c.ok { "OK" } else { "FAIL" },
            c.name,
            c.detail
        ));
    }
    s.push_str(&format!(
        "\nИТОГ: {} ({} аксиом)",
        if all { "ВСЕ ПОДТВЕРЖДЕНЫ" } else { "ЕСТЬ НАРУШЕНИЯ" },
        checks.len()
    ));
    s
}

/// Паспорт книги (для CLI info).
pub fn book_info(book: &PolerBook) -> String {
    let phrases = parse_text(&book.text);
    let words = phrases.iter().map(|p| p.words.len()).sum::<usize>();
    let est_s: f64 = phrases
        .iter()
        .map(|p| {
            p.words.len() as f64 * 0.20 // ~2.5 звука/слово × ~80 мс средн.
                + p.pause_s
        })
        .sum();
    format!(
        ".poler-book: {} Б | фраз: {} | слов: {} | оценка звучания: {:.1} мин\nличности: {} | сидов: {}",
        book.size_bytes(),
        phrases.len(),
        words,
        est_s / 60.0,
        book.passports
            .first()
            .map(|p| p.arch.name())
            .unwrap_or("?"),
        book.passports.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_suite_passes() {
        let (checks, all) = run_suite(0xC0FFEE_1234ABCD);
        for c in &checks {
            if !c.ok {
                eprintln!("FAILED: {} — {}", c.name, c.detail);
            }
        }
        assert!(all, "suite должен проходить полностью");
        assert!(checks.len() >= 10);
    }

    #[test]
    fn suite_report_is_informative() {
        let (checks, _) = run_suite(7);
        let r = report(&checks, true);
        assert!(r.contains("V1a"));
        assert!(r.contains("ИТОГ"));
    }
}
