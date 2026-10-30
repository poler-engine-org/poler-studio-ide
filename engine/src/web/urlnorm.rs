//! Нормализация и разрешение URL (RFC 3986 + google-канонизация).
//!
//! Украдено из практики Googlebot: нормализованный URL — ключ дедупликации
//! в frontier'е и индексе. Один и тот же документ, на который ссылаются
//! как `example.com/a?utm_source=x&b=1#frag` и `EXAMPLE.com/a?b=1`,
//! обязан получить один идентификатор.

/// Разобранный URL (минимальный, без decode/encode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub scheme: String,
    /// host[:port] (port опущен, если дефолтный)
    pub host: String,
    /// путь с ведущим `/` (может быть `/`)
    pub path: String,
    /// нормализованная query-строка (без ведущего `?`, может быть пуста)
    pub query: String,
}

impl Url {
    /// Парсинг абсолютного URL. `None` — если не http/https или кривой.
    pub fn parse(url: &str) -> Option<Url> {
        let url = url.trim().split('#').next().unwrap_or("").trim();
        let (scheme, rest) = url.split_once("://")?;
        let scheme = scheme.to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return None;
        }
        // host[:port]/path?query
        let (authority, tail) = match rest.find(['/', '?']) {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        let authority = authority.trim();
        if authority.is_empty() || authority.contains([' ', '\t', '<', '>']) {
            return None;
        }
        // ipv6 в скобках не трогаем, обычный host — lowercase + дефолтный порт
        let (host_raw, port) = if let Some(rest6) = authority.strip_prefix('[') {
            let (h, p) = rest6.split_once(']')?;
            let p = p.strip_prefix(':').map(|s| s.to_string());
            (format!("[{h}]"), p)
        } else {
            match authority.rsplit_once(':') {
                Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty() => {
                    (h.to_string(), Some(p.to_string()))
                }
                _ => (authority.to_string(), None),
            }
        };
        let host = match port.as_deref() {
            Some("80") if scheme == "http" => host_raw.to_ascii_lowercase(),
            Some("443") if scheme == "https" => host_raw.to_ascii_lowercase(),
            Some(p) => format!("{}:{}", host_raw.to_ascii_lowercase(), p),
            None => host_raw.to_ascii_lowercase(),
        };

        let (path, query) = match tail.split_once('?') {
            Some((p, q)) => (p, q),
            None => (tail, ""),
        };
        let path = if path.is_empty() { "/".to_string() } else { path.to_string() };
        let query = normalize_query(query);
        Some(Url { scheme, host, path, query })
    }

    /// Полный нормализованный URL.
    pub fn as_str(&self) -> String {
        if self.query.is_empty() {
            format!("{}://{}{}", self.scheme, self.host, self.path)
        } else {
            format!("{}://{}{}?{}", self.scheme, self.host, self.path, self.query)
        }
    }

    /// path + ?query — для сопоставления с robots.txt.
    pub fn robots_path(&self) -> String {
        if self.query.is_empty() {
            self.path.clone()
        } else {
            format!("{}?{}", self.path, self.query)
        }
    }

    /// Разрешение ссылки относительно текущего URL (как `a.href`).
    pub fn join(&self, href: &str) -> Option<Url> {
        let href = href.trim();
        if href.is_empty() || href.starts_with('#') {
            return None;
        }
        // абсолютный URL с чужой схемой (mailto:, javascript:, tel:, …) — мусор
        let scheme_like = href
            .split_once(':')
            .map(|(s, _)| {
                !s.is_empty()
                    && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                    && s.chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
                    && !s.contains('/')
            })
            .unwrap_or(false);
        if scheme_like && !href.starts_with("http://") && !href.starts_with("https://") {
            return None;
        }
        if let Some(abs) = href.strip_prefix("//") {
            return Url::parse(&format!("{}://{}", self.scheme, abs));
        }
        if href.starts_with("http://") || href.starts_with("https://") {
            return Url::parse(href);
        }
        let (raw_path, raw_query) = match href.split_once('?') {
            Some((p, q)) => (p, q),
            None => (href, ""),
        };
        let resolved = if raw_path.starts_with('/') {
            resolve_dots(raw_path)
        } else {
            // относительно «директории» текущего пути
            let base_dir = match self.path.rfind('/') {
                Some(i) => &self.path[..=i],
                None => "/",
            };
            resolve_dots(&format!("{base_dir}{raw_path}"))
        }?;
        let path = if resolved.is_empty() { "/".to_string() } else { resolved };
        Some(Url {
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            path,
            query: normalize_query(raw_query),
        })
    }

    /// Хост для per-host politeness frontier'а (без порта).
    pub fn host_key(&self) -> &str {
        // [ipv6]:port → «[ipv6]»; host:port → host
        if let Some(rest) = self.host.strip_prefix('[') {
            // всё до закрывающей скобки включительно
            let end = rest.find(']').map(|i| i + 2).unwrap_or(self.host.len());
            return &self.host[..end];
        }
        match self.host.rsplit_once(':') {
            Some((h, _)) => h,
            None => &self.host,
        }
    }
}

/// Схлопывание `.`/`..` в пути. Возвращает None при выходе за корень.
fn resolve_dots(path: &str) -> Option<String> {
    let mut segs: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segs.pop()?; // выше корня — битая ссылка
            }
            s => segs.push(s),
        }
    }
    let mut out = format!("/{}", segs.join("/"));
    // сохранить trailing slash (директория), кроме случаев ".."/"."
    if path.ends_with('/') && !path.ends_with("/..") && !path.ends_with("/.") && out != "/" {
        out.push('/');
    }
    Some(out)
}

/// Нормализация query: выкидываем трекеры, сортируем параметры —
/// `?b=1&utm_source=x&a=2` → `a=2&b=1`.
fn normalize_query(query: &str) -> String {
    if query.is_empty() {
        return String::new();
    }
    let mut params: Vec<&str> = query.split('&').filter(|p| !p.is_empty()).collect();
    params.retain(|p| {
        let name = p.split('=').next().unwrap_or("");
        let name = name.to_ascii_lowercase();
        !(name.starts_with("utm_")
            || name == "gclid"
            || name == "fbclid"
            || name == "yclid"
            || name == "ref"
            || name == "referrer")
    });
    params.sort_unstable();
    params.join("&")
}

/// Точка входа: нормализовать абсолютный URL целиком (строкой).
pub fn normalize(url: &str) -> Option<String> {
    Url::parse(url).map(|u| u.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_parse() {
        let u = Url::parse("HTTPS://Example.COM:443/a/b?z=1&a=2#frag").unwrap();
        assert_eq!(u.scheme, "https");
        assert_eq!(u.host, "example.com"); // :443 дефолтный — снят
        assert_eq!(u.path, "/a/b");
        assert_eq!(u.query, "a=2&z=1"); // отсортирована
        assert_eq!(u.as_str(), "https://example.com/a/b?a=2&z=1");
    }

    #[test]
    fn tracker_stripping() {
        let a = Url::parse("https://x.io/p?utm_source=tw&id=7&fbclid=abc").unwrap();
        assert_eq!(a.query, "id=7");
        let b = Url::parse("https://x.io/p?id=7").unwrap();
        assert_eq!(a.as_str(), b.as_str()); // дедупликация frontier'а
    }

    #[test]
    fn default_ports_and_root() {
        let u = Url::parse("http://a.com:80").unwrap();
        assert_eq!(u.as_str(), "http://a.com/");
        let u = Url::parse("https://a.com:8443/x").unwrap();
        assert_eq!(u.host, "a.com:8443");
        assert_eq!(u.host_key(), "a.com");
    }

    #[test]
    fn rejects_non_http() {
        assert!(Url::parse("mailto:a@b.c").is_none());
        assert!(Url::parse("javascript:void(0)").is_none());
        assert!(Url::parse("ftp://x.io").is_none());
        assert!(Url::parse("не url").is_none());
    }

    #[test]
    fn join_relative() {
        let base = Url::parse("https://ex.com/docs/page.html?x=1").unwrap();
        assert_eq!(
            base.join("other.html").unwrap().as_str(),
            "https://ex.com/docs/other.html"
        );
        assert_eq!(
            base.join("/root").unwrap().as_str(),
            "https://ex.com/root"
        );
        assert_eq!(
            base.join("../up").unwrap().as_str(),
            "https://ex.com/up"
        );
        assert_eq!(
            base.join("//cdn.io/lib.js").unwrap().as_str(),
            "https://cdn.io/lib.js"
        );
        assert_eq!(
            base.join("https://other.com/z").unwrap().as_str(),
            "https://other.com/z"
        );
        assert!(base.join("#anchor").is_none());
        assert!(base.join("").is_none());
    }

    #[test]
    fn robots_path_includes_query() {
        let u = Url::parse("https://ex.com/search?q=1").unwrap();
        assert_eq!(u.robots_path(), "/search?q=1");
    }

    #[test]
    fn ipv6_host() {
        let u = Url::parse("http://[::1]:8080/x").unwrap();
        assert_eq!(u.host, "[::1]:8080");
        assert_eq!(u.host_key(), "[::1]");
    }

    #[test]
    fn case_insensitive_host() {
        assert_eq!(
            normalize("HTTPS://EXAMPLE.COM/Page").unwrap(),
            "https://example.com/Page" // path чувствителен к регистру
        );
    }
}
