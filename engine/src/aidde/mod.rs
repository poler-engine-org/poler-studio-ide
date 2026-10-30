//! AIDDE — AI-Interpreted Dependency & Impact Engine.
//!
//! Ответ на слепоту grep/RAG и «близорукость» линейного интерпретатора:
//! движок строит **сквозной граф зависимостей** между файлами проекта и
//! выдаёт Impact Passport — что сломается, если изменить или удалить
//! символ (upstream), и от чего неявно зависит сам символ (downstream).
//!
//! * **Уровень 1** — глобальная таблица символов: определения (`fn`,
//!   `struct`, `class`, `def`, …) с файлами и строками + вызовы + импорты;
//! * **Уровень 2** — call graph, разрешённый через таблицу символов
//!   (межфайловые связи: `pipeline.rs` —вызывает→ `allocator.rs`);
//! * **Уровень 3** — двунаправленный impact-анализ (BFS по прямым и
//!   обратным рёбрам) + Triage Layer (v0.21: эвристические сигналы
//!   внимания, формально отделённые от доказанных графом отношений)
//!   + danger level.

pub mod impact;
pub mod sqlite_store;
pub mod symbols;

pub use impact::{
    impact_analysis, triage_scan, Dependency, Dependent, ImpactReport, StructuralRelations,
    TriageAlert, TriageCategory,
};
pub use sqlite_store::{impact_analysis_sqlite, last_seg, SymbolStore};
pub use symbols::{CallSite, Definition, ImportStmt, SymbolTable};
