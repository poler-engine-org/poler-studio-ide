#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! # POLER Reader — живой голос для книг
//!
//! Приложение из директивы владельца («нужно вначале приложение»):
//! читалка/плеер, где книга (.poler-book) синтезируется роторным
//! резонатором живого голоса (цикл K) **с коартикуляцией** — плавными
//! глайдами формант между звуками по каноническому уравнению.
//!
//! Каноническое уравнение `ṗ = −η·Π_Λ[D·p + γ·J·p + ∇F]` отображается:
//! - **J = A − Aᵀ** — ротор: фазы формант + вихревые перекачки энергии;
//! - **D** — дисипация (смуги BW): Ляпуновское затухание между импульсами;
//! - **∇F / вход** — тритная щель {-1,0,+1} (No-Mul слой) + дифференцированный
//!   импульс Розенберга;
//! - **Π_Λ** — Мак-Віні 3P²−2P³ на матрице когерентности состояния.
//!
//! Ключевое отличие от цикла K (одиночные гласные): резонатор
//! **один на всю книгу** — состояние ψ непрерывно, а формантные цели
//! F1/F2/F3 плывут от звука к звуку (τ ≈ 40 мс), как настоящий
//! речевой тракт. ZOH-матрицы пересчитываются блоками по 32 сэмпла.
//!
//! Ноль внешних зависимостей: expm 6×6 (скейлинг-возведение в квадрат +
//! ряд Тейлора), гауссово решение, Якоби-собственные значения, radix-2 FFT —
//! всё своё.

pub mod book;
pub mod fft;
pub mod linalg;
pub mod phonemes;
pub mod polerbook;
pub mod resonator;
pub mod rng;
pub mod stream;
pub mod trit;
pub mod verify;
pub mod voice;
pub mod wav;

/// Ошибка приложения.
#[derive(Debug)]
pub enum ReaderError {
    /// Некорректный вход (IO/формат).
    BadInput(String),
    /// Неизвестный архетип/язык.
    Unknown(String),
    /// Ошибка ввода-вывода.
    Io(std::io::Error),
}

impl std::fmt::Display for ReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReaderError::BadInput(s) => write!(f, "некорректный вход: {s}"),
            ReaderError::Unknown(s) => write!(f, "неизвестно: {s}"),
            ReaderError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for ReaderError {}

impl From<std::io::Error> for ReaderError {
    fn from(e: std::io::Error) -> Self {
        ReaderError::Io(e)
    }
}

/// Частота дискретизации всего конвейера (как в цикле K).
pub const FS: u32 = 22_050;

pub type Result<T> = std::result::Result<T, ReaderError>;
