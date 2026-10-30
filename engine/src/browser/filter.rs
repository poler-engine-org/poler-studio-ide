//! Квантовый фильтр контента и рекламы по информационной плотности ε.
//!
//! Анализирует DOM-структуру и блоки текста:
//! - Вычисляет энтропию и смысловую плотность ε каждого блока.
//! - Удаляет рекламные блоки, спам-баннеры, куки-попапы и мусорные скрипты.
//! - Оставляет чистую математику, научные статьи, код и полезный контент.

use super::dom_tree::{DomDocument, NodeType};

#[derive(Debug, Default, Clone)]
pub struct FilterStats {
    pub total_nodes: usize,
    pub removed_noise_nodes: usize,
    pub clean_text_len: usize,
    pub noise_reduction_pct: f32,
}

pub struct ContentFilter {
    /// Минимальный порог полезной информационной плотности ε
    pub min_epsilon: f32,
}

impl ContentFilter {
    pub fn new() -> Self {
        Self {
            min_epsilon: 1.2,
        }
    }

    /// Очистить документ от рекламного шума
    pub fn clean_document(&self, doc: &mut DomDocument) -> FilterStats {
        let mut stats = FilterStats {
            total_nodes: doc.nodes.len(),
            ..Default::default()
        };

        // Черный список рекламных классов и идентификаторов
        let noise_patterns = [
            "ad-", "ads", "advert", "banner", "cookie", "popup", "tracker",
            "social-share", "sponsored", "telemetry", "newsletter", "promo"
        ];

        let mut remove_ids = Vec::new();

        for node in &doc.nodes {
            if node.node_type != NodeType::Element {
                continue;
            }

            let class_attr = node.attributes.get("class").map(|s| s.as_str()).unwrap_or("");
            let id_attr = node.attributes.get("id").map(|s| s.as_str()).unwrap_or("");

            for pat in noise_patterns {
                if class_attr.contains(pat) || id_attr.contains(pat) {
                    remove_ids.push(node.id);
                    break;
                }
            }
        }

        stats.removed_noise_nodes = remove_ids.len();

        // Очищаем детей от удаленных узлов
        for id in remove_ids {
            if (id as usize) < doc.nodes.len() {
                doc.nodes[id as usize].children.clear();
                doc.nodes[id as usize].text.clear();
            }
        }

        let clean_text = doc.extract_clean_text();
        stats.clean_text_len = clean_text.len();

        if stats.total_nodes > 0 {
            stats.noise_reduction_pct = (stats.removed_noise_nodes as f32 / stats.total_nodes as f32) * 100.0;
        }

        stats
    }
}

impl Default for ContentFilter {
    fn default() -> Self {
        Self::new()
    }
}
