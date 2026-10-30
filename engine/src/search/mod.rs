//! Поисковые подсистемы: детект намерения запроса (v0.82.0), глубокий
//! сбор диска (harvester).

pub mod disk_harvester;
pub mod intent;

pub use disk_harvester::{run_harvest, HarvestConfig, HarvestFormat, HarvestMode, HarvestStats};
pub use intent::{
    classify_path, detect_query_intent, hit_tier, FileClass, IntentMode, QueryIntent,
    SignatureQuery,
};
