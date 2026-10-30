//! Парсеры структуры документа: AST-скоупы кода, markdown-сцены,
//! извлечение троек «сущность — предикат — сущность».

pub mod ast_code;
pub mod markdown_scenes;
pub mod triples;

pub use ast_code::{detect_lang, extract_enclosing_scope, CodeLang, CodeScope};
pub use markdown_scenes::SceneContext;
pub use triples::{extract_code_triples, extract_triples, Triple};
