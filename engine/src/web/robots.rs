//! robots.txt по RFC 9309 / google/robotstxt (Apache-2.0, Google).
//!
//! Украдено и адаптировано под zero-deps философию poler-engine:
//! * группы `User-agent` (совпадение по токену или `*`);
//! * `Allow`/`Disallow` — побеждает **самое длинное** совпадение
//!   (при равной длине Allow выигрывает, как в референсной C++-библиотеке);
//! * wildcards: `*` — любая последовательность, `$` — якорь конца;
//! * `Crawl-delay` (нестандарт, но чтим) и `Sitemap`;
//! * 404/пустой robots → полное разрешение (RFC 9309 §2.3.1.3).

/// Имя нашего краулера для сопоставления в robots.txt.
pub const USER_AGENT: &str = "poler-engine";

/// Разобранный robots.txt одного хоста.
#[derive(Debug, Clone, Default)]
pub struct Robots {
    /// Правила группы, применимой к нам: (allow, паттерн).
    rules: Vec<(bool, String)>,
    /// Crawl-delay из нашей группы, секунды (0 = нет).
    pub crawl_delay_s: f64,
    /// Sitemap-URL'ы (глобальные, вне групп).
    pub sitemaps: Vec<String>,
    /// robots.txt не получен (404/пустой) — всё разрешено.
    pub absent: bool,
}

/// Группа правил robots.txt: (user-агенты, правила, crawl-delay).
type Group = (Vec<String>, Vec<(bool, String)>, f64);

impl Robots {
    /// Разобранный robots.txt одного хоста.
    /// Полный запрет (401/403 по RFC 9309).
    pub fn disallow_all() -> Self {
        Self {
            rules: vec![(false, "/".to_string())],
            crawl_delay_s: 0.0,
            sitemaps: Vec::new(),
            absent: false,
        }
    }

    /// Разбор тела robots.txt.
    pub fn parse(body: &str) -> Self {
        let mut all_sitemaps = Vec::new();
        let mut groups: Vec<Group> = Vec::new();
        let mut cur: Option<Group> = None;

        for raw in body.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (key, val) = match line.split_once(':') {
                Some((k, v)) => (k.trim().to_ascii_lowercase(), v.trim().to_string()),
                None => continue,
            };
            match key.as_str() {
                "user-agent" => {
                    // новая группа начинается, только если у текущей уже есть правила
                    let started = cur
                        .as_ref()
                        .map(|(_, r, _)| !r.is_empty())
                        .unwrap_or(false);
                    if started {
                        groups.push(cur.take().unwrap());
                    }
                    let ua = val.to_ascii_lowercase();
                    cur.get_or_insert_with(|| (Vec::new(), Vec::new(), 0.0)).0.push(ua);
                }
                "allow" => {
                    if let Some((_, r, _)) = cur.as_mut() {
                        r.push((true, val));
                    }
                }
                "disallow" => {
                    if let Some((_, r, _)) = cur.as_mut() {
                        r.push((false, val));
                    }
                }
                "crawl-delay" => {
                    if let Some((_, _, d)) = cur.as_mut() {
                        if let Ok(v) = val.trim().parse::<f64>() {
                            *d = v;
                        }
                    }
                }
                "sitemap" if val.starts_with("http://") || val.starts_with("https://") => {
                    all_sitemaps.push(val);
                }
                _ => {}
            }
        }
        if let Some(g) = cur.take() {
            groups.push(g);
        }

        // RFC 9309 §2.3: применяется ОДНА самая специфичная группа —
        // точное имя агента приоритетнее `*`.
        let mine = groups
            .iter()
            .find(|(uas, _, _)| uas.iter().any(|ua| ua == USER_AGENT))
            .or_else(|| {
                groups
                    .iter()
                    .find(|(uas, _, _)| uas.iter().any(|ua| ua == "*"))
            });
        let (rules, crawl_delay_s) = match mine {
            Some((_, r, d)) => (r.clone(), *d),
            None => (Vec::new(), 0.0),
        };

        Robots {
            rules,
            crawl_delay_s,
            sitemaps: all_sitemaps,
            absent: body.trim().is_empty(),
        }
    }

    /// Разрешён ли URL (path с query: `/a/b?x=1`) для нашего агента.
    pub fn allowed(&self, path: &str) -> bool {
        if self.absent || self.rules.is_empty() {
            return true;
        }
        // RFC 9309 §2.2.2: побеждает самое длинное ПРАВИЛО (длина записи),
        // при равной длине Allow выигрывает.
        let mut best: Option<(usize, bool)> = None; // (entry_len, allow)
        for (allow, pat) in &self.rules {
            if pat.is_empty() {
                // "Disallow:" (пустой) = разрешить всё
                if !*allow {
                    best = Some((0, true));
                }
                continue;
            }
            if rule_matches(pat, path) {
                let len = pat.chars().count();
                let better = match best {
                    None => true,
                    Some((bl, ba)) => len > bl || (len == bl && *allow && !ba),
                };
                if better {
                    best = Some((len, *allow));
                }
            }
        }
        best.map(|(_, a)| a).unwrap_or(true)
    }
}

/// Совпадает ли паттерн robots с путём (префиксная семантика + wildcards).
///
/// Поддержка `*` (любая последовательность) и `$` (конец строки).
fn rule_matches(pattern: &str, path: &str) -> bool {
    rule_match_rec(&pattern.chars().collect::<Vec<_>>(), 0,
                   &path.chars().collect::<Vec<_>>(), 0).is_some()
}

fn rule_match_rec(p: &[char], pi: usize, s: &[char], si: usize) -> Option<usize> {
    // конец паттерна
    if pi == p.len() {
        return Some(si);
    }
    // '$' — только конец строки
    if p[pi] == '$' && pi + 1 == p.len() {
        return if si == s.len() { Some(si) } else { None };
    }
    if p[pi] == '*' {
        // '*' может съесть 0..остаток; пробуем все длины (backtracking)
        for take in (0..=(s.len() - si)).rev() {
            if let Some(end) = rule_match_rec(p, pi + 1, s, si + take) {
                return Some(end);
            }
        }
        return None;
    }
    // обычный символ
    if si < s.len() && p[pi] == s[si] {
        rule_match_rec(p, pi + 1, s, si + 1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: &str = "User-agent: *\nDisallow: /search\nDisallow: /admin/*\nAllow: /admin/public$\nCrawl-delay: 2\n\nUser-agent: poler-engine\nDisallow: /private\n";

    #[test]
    fn longest_entry_wins() {
        // Allow-правило длиннее → побеждает вопреки Disallow
        let r = Robots::parse("User-agent: *\nDisallow: /a\nAllow: /abc$");
        assert!(r.allowed("/abc"));
        assert!(!r.allowed("/ab"));
    }

    #[test]
    fn prefix_semantics() {
        // robots: Disallow-паттерн — префиксное совпадение
        let r = Robots::parse("User-agent: *\nDisallow: /search");
        assert!(!r.allowed("/search"));
        assert!(!r.allowed("/search?q=1"));
        assert!(!r.allowed("/searching"));
        assert!(r.allowed("/about"));
    }

    #[test]
    fn wildcard_and_dollar() {
        let r = Robots::parse("User-agent: *\nDisallow: /admin/*\nAllow: /admin/public$");
        assert!(!r.allowed("/admin/panel"));
        assert!(r.allowed("/admin/public"));
        assert!(!r.allowed("/admin/public/x"));
    }

    #[test]
    fn specific_agent_group_wins() {
        let r = Robots::parse(R);
        // группа poler-engine (точное совпадение) важнее группы *
        assert!(!r.allowed("/private/data"));
        assert!(r.allowed("/search"));
        assert!((r.crawl_delay_s - 0.0).abs() < 1e-9); // crawl-delay был в группе *
    }

    #[test]
    fn crawl_delay_and_sitemap() {
        let r = Robots::parse(
            "User-agent: poler-engine\nDisallow: /tmp\nCrawl-delay: 2.5\nSitemap: https://x/s.xml",
        );
        assert!((r.crawl_delay_s - 2.5).abs() < 1e-9);
        assert_eq!(r.sitemaps, vec!["https://x/s.xml".to_string()]);
    }

    #[test]
    fn empty_disallow_allows() {
        let r = Robots::parse("User-agent: *\nDisallow:\n");
        assert!(r.allowed("/anything"));
    }

    #[test]
    fn absent_robots_allows_everything() {
        let r = Robots::parse("");
        assert!(r.absent);
        assert!(r.allowed("/secret"));
    }

    #[test]
    fn comments_and_case() {
        let r = Robots::parse("USER-AGENT: *\n# comment\nDisallow: /x  # inline\n");
        assert!(!r.allowed("/xyz"));
        assert!(r.allowed("/y"));
    }

    #[test]
    fn real_robots_shape() {
        let body = "User-agent: Googlebot\nDisallow: /private\n\nUser-agent: *\nDisallow: /\nAllow: /public\nSitemap: https://site.com/sitemap.xml\n";
        let r = Robots::parse(body);
        // группа Googlebot не наша; группа * запрещает всё кроме /public
        assert!(!r.allowed("/whatever"));
        assert!(r.allowed("/public/files"));
    }
}
