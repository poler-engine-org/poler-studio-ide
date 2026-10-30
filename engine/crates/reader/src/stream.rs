//! Потоковый синтез книги: фразы → сэмплы через ОДИН живой голос.
//!
//! Конвейер фразы:
//!   слова → сегменты (фонемы) → посэмпольный рендер LivingVoice
//!   с контуром F0 (Tune) и целями формант на гласных;
//!   паузы — u=0 (ψ естественно затухает: Ляпунов/дисипация).
//!
//! Результат нормируется к пику 0.82 и пишется в WAV.

use crate::book::Phrase;
use crate::phonemes::segments_for_word;
use crate::polerbook::{PhrasePassport, PolerBook};
use crate::rng::Xorshift64;
use crate::voice::{Archetype, LivingVoice};
use crate::wav;
use crate::{FS, Result};
use std::path::Path;

/// Межсловная микропауза (с) — щель прикрыта, энергия стравливается.
const WORD_GAP_S: f64 = 0.028;

/// Итог рендера.
#[derive(Debug, Clone)]
pub struct RenderResult {
    /// Сэмплы [-1, 1].
    pub samples: Vec<f64>,
    /// Количество фраз.
    pub n_phrases: usize,
    /// Длительность (с).
    pub duration_s: f64,
    /// Паспорт живости голоса.
    pub passport: crate::voice::RenderPassport,
    /// Нормировочный множитель.
    pub peak_norm: f64,
}

/// Синтезировать вектор фраз одним голосом (seed = личность).
pub fn render_phrases(
    phrases: &[Phrase],
    passports: &[PhrasePassport],
    voice_seed: u64,
    base: Archetype,
) -> Result<RenderResult> {
    let mut voice = LivingVoice::new(voice_seed, base);
    let mut out: Vec<f64> = Vec::new();

    for (pi, phrase) in phrases.iter().enumerate() {
        let pp = passports.get(pi);
        let tempo = phrase.tune.tempo();
        let tune = phrase.tune;
        // фразовое семя: детерминированная вариативность (если паспорт есть)
        let phrase_rng = match pp {
            Some(p) => Xorshift64::new(p.seed),
            None => Xorshift64::new(voice_seed ^ (0x9E37_79B9_7F4A_7C15u64.wrapping_add(pi as u64))),
        };
        let mut prng = phrase_rng;

        // сегменты всех слов фразы
        let mut prev_arch = base;
        let mut seg_stream: Vec<(crate::phonemes::Segment, f64)> = Vec::new(); // (сегмент, старт в фразе)
        let mut t_cursor = 0.0f64;
        for (wi, word) in phrase.words.iter().enumerate() {
            let segs = segments_for_word(word, &mut prev_arch, tempo, &mut prng);
            for s in segs {
                seg_stream.push((s, t_cursor));
                t_cursor += s.dur_s;
            }
            if wi + 1 < phrase.words.len() {
                t_cursor += WORD_GAP_S;
            }
        }
        let phrase_dur = t_cursor + phrase.pause_s;
        let n_samples = (phrase_dur * FS as f64).ceil() as usize;

        // длительность фразы без паузы (для контура F0)
        let voiced_dur = t_cursor.max(1e-9);

        // посэмпольный проход
        let mut si = 0usize;
        let mut next_switch = seg_stream
            .first()
            .map(|(_s, t)| (*t * FS as f64) as usize)
            .unwrap_or(usize::MAX);
        for i in 0..n_samples {
            let t = i as f64 / FS as f64;
            // продвижение по сегментам
            while si < seg_stream.len() && i >= next_switch {
                let (seg, _) = &seg_stream[si];
                if seg.drive > 0.0 {
                    let f = seg.arch.formants();
                    let bw = seg.arch.bandwidths();
                    voice.set_target(f, bw);
                }
                // на паузе цель формант остаётся прежней (тракт не прыгает)
                si += 1;
                next_switch = if si < seg_stream.len() {
                    (seg_stream[si].1 * FS as f64) as usize
                } else {
                    usize::MAX
                };
            }
            let (drive, f0, vib, jit, shim) = if si == 0 {
                // первая миллисекунда — мягкая атака (щель открывается)
                let seg = seg_stream.first().map(|(s, _)| s).unwrap();
                let ramp = (t / 0.012).clamp(0.0, 1.0);
                (
                    seg.drive * ramp,
                    base.f0() * tune.f0_scale(0.0),
                    base.vibrato_hz(),
                    base.jitter(),
                    base.shimmer_db(),
                )
            } else if si <= seg_stream.len() {
                let (seg, seg_t) = &seg_stream[si - 1];
                let arch = seg.arch;
                (
                    seg.drive,
                    arch.f0() * tune.f0_scale((t - seg_t).max(0.0) / voiced_dur),
                    arch.vibrato_hz(),
                    arch.jitter(),
                    arch.shimmer_db(),
                )
            } else {
                // пауза после фразы: щель закрыта
                (0.0, base.f0(), base.vibrato_hz(), base.jitter(), base.shimmer_db())
            };
            out.push(voice.sample(drive, f0, vib, jit, shim));
        }
    }

    // нормировка к пику 0.82
    let peak = out.iter().fold(0.0f64, |a, &v| a.max(v.abs()));
    let norm = if peak > 1e-12 { 0.82 / peak } else { 1.0 };
    for s in out.iter_mut() {
        *s *= norm;
    }

    let duration_s = out.len() as f64 / FS as f64;
    Ok(RenderResult {
        samples: out,
        n_phrases: phrases.len(),
        duration_s,
        passport: voice.passport(),
        peak_norm: norm,
    })
}

/// Упаковать книгу (текст → паспорта по фразам) — детерминированно из
/// (voice_seed, базовый архетип).
pub fn pack_book(text: &str, voice_seed: u64, base: Archetype) -> Result<PolerBook> {
    let phrases = crate::book::parse_text(text);
    if phrases.is_empty() {
        return Err(crate::ReaderError::BadInput("пустой текст".into()));
    }
    let mut rng = Xorshift64::new(voice_seed ^ 0xD1B5_4A32_D3F0_8E11);
    let passports = phrases
        .iter()
        .map(|p| {
            let seed = rng.next_u64();
            PhrasePassport {
                seed,
                arch: base,
                pause_ms: (p.pause_s * 1000.0).round().clamp(0.0, 65535.0) as u16,
            }
        })
        .collect();
    Ok(PolerBook::new(text.to_string(), passports))
}

/// Полный конвейер: .poler-book → WAV-файл. Возвращает результат.
pub fn render_book_file(book: &PolerBook, voice_seed: u64, out_wav: &Path) -> Result<RenderResult> {
    let phrases = crate::book::parse_text(&book.text);
    if phrases.is_empty() {
        return Err(crate::ReaderError::BadInput("в книге нет фраз".into()));
    }
    let base = book
        .passports
        .first()
        .map(|p| p.arch)
        .unwrap_or(Archetype::ACalm);
    let result = render_phrases(&phrases, &book.passports, voice_seed, base)?;
    wav::save_wav(out_wav, &result.samples, FS)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::parse_text;

    #[test]
    fn render_short_book_produces_audio() {
        let phrases = parse_text("Да. Нет?");
        let passports: Vec<PhrasePassport> = phrases
            .iter()
            .enumerate()
            .map(|(i, p)| PhrasePassport {
                seed: 100 + i as u64,
                arch: Archetype::ACalm,
                pause_ms: (p.pause_s * 1000.0) as u16,
            })
            .collect();
        let r = render_phrases(&phrases, &passports, 42, Archetype::ACalm).unwrap();
        // 2 фразы ≈ (2 звука + паузы) ≈ 0.6-1.2 с
        assert!(r.duration_s > 0.5, "длительность: {}", r.duration_s);
        assert!(r.duration_s < 3.0);
        // сигнал живой: не тишина и не клип
        let peak = r.samples.iter().fold(0.0f64, |a, &v| a.max(v.abs()));
        assert!(peak > 0.5, "пик после нормировки: {peak}");
        assert!(peak <= 0.82 + 1e-9);
        // есть периоды (щели работали)
        assert!(r.passport.n_periods > 10);
    }

    #[test]
    fn render_is_deterministic() {
        let phrases = parse_text("Живой голос звучит.");
        let mk = || {
            let passports: Vec<PhrasePassport> = phrases
                .iter()
                .enumerate()
                .map(|(i, p)| PhrasePassport {
                    seed: 7 + i as u64,
                    arch: Archetype::ACalm,
                    pause_ms: (p.pause_s * 1000.0) as u16,
                })
                .collect();
            render_phrases(&phrases, &passports, 0xABCD, Archetype::ACalm).unwrap()
        };
        let a = mk();
        let b = mk();
        assert_eq!(a.samples.len(), b.samples.len());
        for (x, y) in a.samples.iter().zip(b.samples.iter()) {
            assert_eq!(x.to_bits(), y.to_bits(), "бит-в-бит детерминизм");
        }
    }

    #[test]
    fn different_voice_seed_different_wave() {
        let phrases = parse_text("Тот же текст.");
        let passports: Vec<PhrasePassport> = phrases
            .iter()
            .enumerate()
            .map(|(i, p)| PhrasePassport {
                seed: 5 + i as u64,
                arch: Archetype::ACalm,
                pause_ms: (p.pause_s * 1000.0) as u16,
            })
            .collect();
        let a = render_phrases(&phrases, &passports, 1, Archetype::ACalm).unwrap();
        let b = render_phrases(&phrases, &passports, 2, Archetype::ACalm).unwrap();
        let differ = a
            .samples
            .iter()
            .zip(b.samples.iter())
            .filter(|(x, y)| x != y)
            .count();
        assert!(differ > 100, "другая личность → другая волна ({differ} отличий)");
    }

    #[test]
    fn pack_and_render_book_roundtrip() {
        let dir = std::env::temp_dir().join("poler_reader_stream");
        let _ = std::fs::create_dir_all(&dir);
        let text = "Первая фраза книги. Вторая фраза!";
        let book = pack_book(text, 777, Archetype::ACalm).unwrap();
        assert_eq!(book.text, text);
        assert_eq!(book.passports.len(), 2);
        let p = dir.join("demo.poler-book");
        book.save(&p).unwrap();
        let loaded = PolerBook::load(&p).unwrap();
        assert_eq!(loaded, book);
        let wav_path = dir.join("demo.wav");
        let r = render_book_file(&loaded, 777, &wav_path).unwrap();
        assert!(wav_path.exists());
        assert!(r.duration_s > 0.8);
        // размер книги крошечный против WAV
        let book_sz = std::fs::metadata(&p).unwrap().len() as f64;
        let wav_sz = std::fs::metadata(&wav_path).unwrap().len() as f64;
        assert!(wav_sz / book_sz > 50.0, "сжатие: {wav_sz}/{book_sz}");
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_file(&wav_path);
    }

    #[test]
    fn lyapunov_holds_in_full_render() {
        // между импульсами энергия не растёт — инвариант D-дисипации
        let phrases = parse_text("Ротор диссипирует энергию между импульсами всегда.");
        let passports: Vec<PhrasePassport> = phrases
            .iter()
            .enumerate()
            .map(|(i, p)| PhrasePassport {
                seed: 9 + i as u64,
                arch: Archetype::ACalm,
                pause_ms: (p.pause_s * 1000.0) as u16,
            })
            .collect();
        let r = render_phrases(&phrases, &passports, 33, Archetype::ACalm).unwrap();
        // допустимы единичные транзиты на границах блоков ZOH (смена
        // геометрии) — но не систематические нарушения
        assert!(
            u64::from(r.passport.lyapunov_violations) < r.passport.n_periods / 10,
            "нарушений Ляпунова: {} на {} периодов",
            r.passport.lyapunov_violations,
            r.passport.n_periods
        );
    }
}
