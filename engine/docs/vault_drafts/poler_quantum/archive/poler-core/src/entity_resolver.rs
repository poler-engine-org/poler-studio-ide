//! entity_resolver.rs — Модуль разрешения кореферентности (Coreference & Entity Aliases)
//! Учитывает местоимения и синонимы для точного вычисления epsilon_climax.

use std::collections::HashMap;

pub struct EntityResolver {
    pub canonical_map: HashMap<String, String>,
}

impl EntityResolver {
    pub fn new() -> Self {
        Self {
            canonical_map: HashMap::new(),
        }
    }

    pub fn register_alias(&mut self, alias: &str, canonical: &str) {
        self.canonical_map.insert(alias.to_lowercase(), canonical.to_string());
    }

    pub fn resolve<'a>(&'a self, token: &'a str) -> &'a str {
        let key = token.to_lowercase();
        if let Some(canon) = self.canonical_map.get(&key) {
            canon.as_str()
        } else {
            token
        }
    }
}
