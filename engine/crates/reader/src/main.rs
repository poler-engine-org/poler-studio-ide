#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! CLI POLER Reader — приложение живого голоса для книг.
//!
//!   poler-reader pack    book.txt -o book.poler-book --voice a_calm --seed 42
//!   poler-reader render  book.poler-book -o demo.wav --seed 42
//!   poler-reader render  book.txt --voice u_calm -o demo.wav
//!   poler-reader info    book.poler-book
//!   poler-reader verify  --seed 42
//!   poler-reader voices

use std::path::PathBuf;
use std::process::ExitCode;

use poler_reader::book::{detect_format, load_book, Format};
use poler_reader::polerbook::PolerBook;
use poler_reader::stream::{pack_book, render_book_file};
use poler_reader::verify;
use poler_reader::voice::Archetype;
use poler_reader::{FS, Result};

#[derive(Debug)]
struct Args {
    cmd: String,
    input: Option<PathBuf>,
    out: Option<PathBuf>,
    voice: String,
    seed: u64,
}

fn parse_args() -> Result<Args> {
    let mut it = std::env::args().skip(1);
    let cmd = it
        .next()
        .ok_or_else(|| poler_reader::ReaderError::BadInput(usage().into()))?;
    let mut a = Args {
        cmd,
        input: None,
        out: None,
        voice: "a_calm".into(),
        seed: 42,
    };
    let mut take_out = false;
    let mut take_voice = false;
    let mut take_seed = false;
    for tok in it {
        if take_out {
            a.out = Some(PathBuf::from(tok));
            take_out = false;
        } else if take_voice {
            a.voice = tok;
            take_voice = false;
        } else if take_seed {
            a.seed = tok
                .parse()
                .map_err(|_| poler_reader::ReaderError::BadInput("seed должен быть числом".into()))?;
            take_seed = false;
        } else {
            match tok.as_str() {
                "-o" | "--out" => take_out = true,
                "--voice" | "-v" => take_voice = true,
                "--seed" | "-s" => take_seed = true,
                "-h" | "--help" => {
                    return Err(poler_reader::ReaderError::BadInput(usage().into()))
                }
                other => a.input = Some(PathBuf::from(other)),
            }
        }
    }
    Ok(a)
}

fn usage() -> &'static str {
    "POLER Reader — живой голос для книг (роторный резонатор + тритная щель)\n\
     \n\
     ИСПОЛЬЗОВАНИЕ:\n\
       poler-reader pack <текст.txt|md|fb2> -o <книга.poler-book> [--voice a_calm] [--seed N]\n\
       poler-reader render <книга.poler-book|текст.txt> -o <звук.wav> [--voice a_calm] [--seed N]\n\
       poler-reader info <книга.poler-book>\n\
       poler-reader verify [--seed N]\n\
       poler-reader voices\n\
     \n\
       --voice: a_calm (мужской спокойный), a_bright (яркий), i_dark (тёмный), u_calm (мягкий)\n\
       --seed : 64-битная личность диктора (один seed = один голос навсегда)"
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n");
            eprintln!("{}", usage());
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(msg) => {
            if !msg.is_empty() {
                println!("{msg}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("❌ {e}");
            ExitCode::from(1)
        }
    }
}

fn run(a: &Args) -> Result<String> {
    match a.cmd.as_str() {
        "voices" => Ok(Archetype::ALL
            .iter()
            .map(|v| {
                let f = v.formants();
                format!(
                    "{:<10} F1/F2/F3 = {:.0}/{:.0}/{:.0} Гц, F0 = {:.0} Гц",
                    v.name(),
                    f[0],
                    f[1],
                    f[2],
                    v.f0()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")),
        "verify" => {
            let (checks, all) = verify::run_suite(a.seed);
            println!("{}", verify::report(&checks, all));
            if all {
                Ok(String::new())
            } else {
                Err(poler_reader::ReaderError::BadInput("верификация провалена".into()))
            }
        }
        "info" => {
            let path = a
                .input
                .as_ref()
                .ok_or_else(|| poler_reader::ReaderError::BadInput("info <книга>".into()))?;
            if detect_format(path) != Format::PolerBook {
                return Err(poler_reader::ReaderError::BadInput(
                    "info работает с .poler-book (pack сначала)".into(),
                ));
            }
            let book = PolerBook::load(path)?;
            Ok(verify::book_info(&book))
        }
        "pack" => {
            let src = a
                .input
                .clone()
                .ok_or_else(|| poler_reader::ReaderError::BadInput("pack <текст>".into()))?;
            let arch = Archetype::from_name(&a.voice)
                .ok_or_else(|| poler_reader::ReaderError::Unknown(format!("голос {}", a.voice)))?;
            let text = std::fs::read_to_string(&src)?;
            let book = pack_book(&text, a.seed, arch)?;
            let out = a.out.clone().unwrap_or_else(|| {
                let mut p = src.clone();
                p.set_extension("poler-book");
                p
            });
            book.save(&out)?;
            Ok(format!(
                "✓ упаковано: {} ({} Б) — {}",
                out.display(),
                book.size_bytes(),
                verify::book_info(&book)
            ))
        }
        "render" => {
            let src = a
                .input
                .clone()
                .ok_or_else(|| poler_reader::ReaderError::BadInput("render <книга|текст>".into()))?;
            let arch = Archetype::from_name(&a.voice)
                .ok_or_else(|| poler_reader::ReaderError::Unknown(format!("голос {}", a.voice)))?;
            let out = a.out.clone().unwrap_or_else(|| "living_book.wav".into());
            let book = if detect_format(&src) == Format::PolerBook {
                PolerBook::load(&src)?
            } else {
                let phrases = load_book(&src)?;
                // простой путь: текст → книга → рендер
                let text = phrases
                    .iter()
                    .map(|p| p.words.join(" "))
                    .collect::<Vec<_>>()
                    .join(". ");
                pack_book(&format!("{text}. "), a.seed, arch)?
            };
            let r = render_book_file(&book, a.seed, &out)?;
            Ok(format!(
                "✓ {} — {:.1} с звука, {} фраз, {} периодов щели, jitter {:.2}%, RT-норм OK (fs {} Гц)",
                out.display(),
                r.duration_s,
                r.n_phrases,
                r.passport.n_periods,
                r.passport.jitter_std_pct,
                FS
            ))
        }
        other => Err(poler_reader::ReaderError::Unknown(format!(
            "команда {other}\n{}",
            usage()
        ))),
    }
}
