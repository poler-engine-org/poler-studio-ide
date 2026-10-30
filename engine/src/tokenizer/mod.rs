//! Токенизация, инвертированный индекс и zero-copy PII-санитайзер.

pub mod inverted_index;
pub mod pii;

pub use inverted_index::InvertedIndex;
pub use pii::{PiiCleaner, PiiMode};
