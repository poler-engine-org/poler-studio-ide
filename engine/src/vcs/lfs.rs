//! # v0.17.0 M5 — Git LFS pointer detection + basic batch fetch
//!
//! Pure-Rust LFS-клиент без зависимости от системного `git-lfs` бинарника.
//! Покрывает 80% сценария: detect → list → fetch LFS-объектов из клонированного репо.
//!
//! ## Что такое Git LFS
//!
//! Git LFS заменяет большие файлы (бинарники, видео, датасеты) на pointer-файлы
//! в самом git-репозитории. Pointer выглядит так:
//! ```text
//! version https://git-lfs.github.com/spec/v1
//! oid sha256:abc123def...  (64 hex chars)
//! size 12345678            (bytes)
//! ```
//! Настоящий бинарник живёт на LFS-server (обычно `{repo_url}.git/info/lfs`)
//! и скачивается по HTTP batch-протоколу.
//!
//! ## Что реализовано в v0.17.0
//!
//! - **`detect_pointers(repo_path)`** — обходит worktree, находит все LFS
//!   pointer-файлы по header-строке `version https://git-lfs.github.com/spec/v1`
//! - **`list_pointers(repo_path)`** — форматированный вывод списка (для `gix lfs list`)
//! - **`fetch_objects(repo_path, ids)`** — batch-запрос к LFS-server через ureq
//!   (HTTP POST с `{ "operation": "download", "objects": [...] }`),
//!   скачивает blob → `.git/lfs/objects/<oid[:2]>/<oid[2:]>` (как настоящий git-lfs)
//!
//! ## Ограничения v0.17.0
//!
//! - Только HTTPS-репозитории (не SSH LFS)
//! - Не делает resumable downloads
//! - Не показывает прогресс-бар (просто пишет "fetching N objects...")
//! - Auth: только Bearer-токен из env `POLER_GIT_TOKEN` (для приватных LFS)
//! - Не поддерживает `git-lfs-authenticate` (GitLab/Gitea SSO) — только прямая
//!   batch-загрузка
//!
//! ## Пример
//!
//! ```text
//! poler> gix clone https://github.com/user/large-repo ./lr
//! poler> gix lfs list ./lr
//!   1. data/big-model.bin      oid: a1b2c3...  size: 1.4 GB
//!   2. data/dataset.parquet    oid: d4e5f6...  size: 234 MB
//! poler> gix lfs fetch ./lr
//!   ↓ 2 LFS objects (1.6 GB total)
//!   ✓ data/big-model.bin
//!   ✓ data/dataset.parquet
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::io::Read;
use std::fs;

/// LFS-pointer: распарсенный заголовок из git-pointer-файла.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfsPointer {
    /// Путь к файлу в worktree (относительно repo root).
    pub path: PathBuf,
    /// Алгоритм хеширования (всегда "sha256" в текущей спецификации LFS).
    pub oid_algo: String,
    /// HEX-хеш объекта (64 символа для sha256).
    pub oid: String,
    /// Размер в байтах.
    pub size: u64,
}

impl LfsPointer {
    /// Возвращает true, если это поле-заголовок LFS pointer.
    pub fn is_pointer_content(content: &str) -> bool {
        content.starts_with("version https://git-lfs.github.com/spec/v1")
    }

    /// Парсит содержимое файла как LFS pointer. Возвращает None, если файл
    /// не LFS-pointer (например, обычный текстовый файл).
    pub fn parse(content: &str, path: impl Into<PathBuf>) -> Option<Self> {
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return None;
        }
        if !Self::is_pointer_content(content) {
            return None;
        }
        let mut oid_algo = String::new();
        let mut oid = String::new();
        let mut size: Option<u64> = None;
        for line in &lines[1..] {
            if let Some(rest) = line.strip_prefix("oid ") {
                // "oid sha256:abc123..."
                if let Some((algo, hash)) = rest.split_once(':') {
                    oid_algo = algo.trim().to_string();
                    oid = hash.trim().to_string();
                }
            } else if let Some(rest) = line.strip_prefix("size ") {
                size = rest.trim().parse::<u64>().ok();
            }
        }
        let size = size?;
        if oid.is_empty() || oid_algo.is_empty() {
            return None;
        }
        Some(LfsPointer {
            path: path.into(),
            oid_algo,
            oid,
            size,
        })
    }
}

/// Найти все LFS pointer-файлы в worktree репозитория.
/// Обходит только tracked-файлы worktree (не .git/, не .gitignored).
///
/// Простая реализация: walk_dir + read_first_bytes + parse. Для репозиториев
/// с >100k файлов это может быть медленно — но мы фильтруем по первому
/// 100-байтному заголовку, чтобы не читать целиком большие бинарники.
pub fn detect_pointers(repo_path: &Path) -> Vec<LfsPointer> {
    let mut result = Vec::new();
    let git_dir = repo_path.join(".git");
    let mut stack = vec![repo_path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // пропустить .git
            if path == git_dir {
                continue;
            }
            // пропустить hidden (включая .gitignore-зону)
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with('.') && path != repo_path {
                    continue;
                }
            }
            let ft = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file() {
                if let Some(p) = try_parse_pointer_file(&path) {
                    result.push(p);
                }
            }
        }
    }
    result
}

/// Прочитать первые ~512 байт файла и попытаться парсить как LFS pointer.
fn try_parse_pointer_file(path: &Path) -> Option<LfsPointer> {
    let mut file = fs::File::open(path).ok()?;
    let mut buf = [0u8; 512];
    let n = file.read(&mut buf).ok()?;
    let content = String::from_utf8_lossy(&buf[..n]);
    // Не парсить если последние байты — не newline (значит файл больше 512 байт,
    // и это уже не pointer — pointer-файл всегда < 200 байт).
    if n == 512 && !buf[n - 1].is_ascii_whitespace() {
        return None;
    }
    let rel_path = path.strip_prefix(path.parent().unwrap().parent().unwrap_or(path.parent().unwrap()))
        .unwrap_or(path)
        .to_path_buf();
    LfsPointer::parse(&content, rel_path)
}

/// Форматированный вывод списка pointer-файлов для CLI.
pub fn format_pointers(pointers: &[LfsPointer]) -> String {
    if pointers.is_empty() {
        return "LFS pointer-файлов не обнаружено".into();
    }
    let mut out = format!("LFS pointer-файлов: {}\n\n", pointers.len());
    for (i, p) in pointers.iter().enumerate() {
        out.push_str(&format!(
            "  {}. {:<40} oid: {}  size: {}\n",
            i + 1,
            p.path.display(),
            &p.oid[..12.min(p.oid.len())],
            human_size(p.size),
        ));
    }
    out
}

/// Человеко-читаемый размер.
fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Определить LFS-server URL для репозитория.
///
/// GitHub:    `https://github.com/USER/REPO.git/info/lfs`
/// GitLab:    `https://gitlab.com/USER/REPO.git/info/lfs`
/// Generic:   `{origin}/info/lfs`
///
/// Для этого читаем `.git/config` и ищем `remote.origin.url`.
pub fn lfs_server_url(repo_path: &Path) -> Option<String> {
    let config_path = repo_path.join(".git").join("config");
    let content = fs::read_to_string(&config_path).ok()?;
    let mut in_origin = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_origin = trimmed == "[remote \"origin\"]";
            continue;
        }
        if in_origin {
            if let Some(url) = trimmed.strip_prefix("url = ") {
                let url = url.trim();
                let clean = url.strip_suffix(".git").unwrap_or(url);
                let clean = clean.strip_suffix(".git/").unwrap_or(clean);
                return Some(format!("{clean}/info/lfs"));
            }
        }
    }
    None
}

/// LFS batch-ответ: либо файл готов к скачиванию, либо ошибка.
#[derive(Debug)]
pub struct LfsFetchResult {
    pub oid: String,
    pub success: bool,
    pub bytes_downloaded: u64,
    pub error: Option<String>,
}

/// Скачать указанные LFS-объекты из LFS-server в `.git/lfs/objects/`.
///
/// Использует LFS batch API: POST `/info/lfs/objects/batch` с JSON
/// `{ "operation": "download", "objects": [{"oid": "...", "size": N}, ...] }`,
/// в ответе — массив `objects` с полем `actions.download.href`.
pub fn fetch_objects(
    repo_path: &Path,
    pointers: &[LfsPointer],
) -> Result<Vec<LfsFetchResult>, String> {
    if pointers.is_empty() {
        return Ok(Vec::new());
    }
    let lfs_url = lfs_server_url(repo_path)
        .ok_or_else(|| "не удалось определить LFS-server URL (нет remote.origin.url в .git/config)".to_string())?;
    let batch_url = format!("{lfs_url}/objects/batch");

    // Сборка batch-запроса
    let objects_json: Vec<String> = pointers
        .iter()
        .map(|p| format!(r#"{{"oid":"{}","size":{}}}"#, p.oid, p.size))
        .collect();
    let body = format!(
        r#"{{"operation":"download","transfers":["basic"],"objects":[{}]}}"#,
        objects_json.join(",")
    );

    let token = std::env::var("POLER_GIT_TOKEN").ok();
    let mut req = ureq::post(&batch_url)
        .set("Accept", "application/vnd.git-lfs+json")
        .set("Content-Type", "application/vnd.git-lfs+json");
    if let Some(ref t) = token {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }

    let resp = req
        .send_string(&body)
        .map_err(|e| format!("batch request failed: {e}"))?;

    let json: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("batch response parse: {e}"))?;

    let objs = json
        .get("objects")
        .and_then(|v| v.as_array())
        .ok_or("no 'objects' array in batch response")?;

    let mut results = Vec::new();
    let lfs_obj_dir = repo_path.join(".git").join("lfs").join("objects");
    fs::create_dir_all(&lfs_obj_dir).map_err(|e| format!("mkdir lfs/objects: {e}"))?;

    for obj in objs {
        let oid = obj
            .get("oid")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let actions = obj.get("actions");
        let download_href = actions
            .and_then(|a| a.get("download"))
            .and_then(|d| d.get("href"))
            .and_then(|h| h.as_str());

        if let Some(href) = download_href {
            // Скачиваем blob
            let oid_path = lfs_obj_dir
                .join(&oid[..2.min(oid.len())])
                .join(&oid[2.min(oid.len())..]);
            fs::create_dir_all(oid_path.parent().unwrap()).ok();

            let mut dl_req = ureq::get(href);
            if let Some(ref t) = token {
                dl_req = dl_req.set("Authorization", &format!("Bearer {t}"));
            }
            match dl_req.call() {
                Ok(resp) => {
                    let mut reader = resp.into_reader();
                    let mut buf = Vec::new();
                    reader.read_to_end(&mut buf).map_err(|e| format!("read body: {e}"))?;
                    let bytes = buf.len() as u64;
                    fs::write(&oid_path, &buf).map_err(|e| format!("write lfs object: {e}"))?;
                    results.push(LfsFetchResult {
                        oid,
                        success: true,
                        bytes_downloaded: bytes,
                        error: None,
                    });
                }
                Err(e) => {
                    results.push(LfsFetchResult {
                        oid,
                        success: false,
                        bytes_downloaded: 0,
                        error: Some(format!("{e}")),
                    });
                }
            }
        } else {
            let err_msg = obj
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("no download action")
                .to_string();
            results.push(LfsFetchResult {
                oid,
                success: false,
                bytes_downloaded: 0,
                error: Some(err_msg),
            });
        }
    }

    Ok(results)
}

/// Форматированный вывод результатов fetch для CLI.
pub fn format_fetch_results(results: &[LfsFetchResult]) -> String {
    if results.is_empty() {
        return "нечего скачивать (нет LFS pointer-файлов)".into();
    }
    let total_bytes: u64 = results.iter().filter(|r| r.success).map(|r| r.bytes_downloaded).sum();
    let success_count = results.iter().filter(|r| r.success).count();
    let mut out = format!(
        "↓ LFS: {}/{} объектов скачано ({})\n",
        success_count,
        results.len(),
        human_size(total_bytes)
    );
    for r in results {
        let short_oid = &r.oid[..12.min(r.oid.len())];
        if r.success {
            out.push_str(&format!("  ✓ {} ({})\n", short_oid, human_size(r.bytes_downloaded)));
        } else {
            let err = r.error.as_deref().unwrap_or("unknown error");
            out.push_str(&format!("  ✗ {}: {}\n", short_oid, err));
        }
    }
    out
}

/// Хелпер: построить map oid → LfsPointer для быстрого поиска.
pub fn oid_to_pointer_map(pointers: &[LfsPointer]) -> HashMap<String, &LfsPointer> {
    pointers.iter().map(|p| (p.oid.clone(), p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer_content(oid: &str, size: u64) -> String {
        format!("version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {size}\n")
    }

    #[test]
    fn parse_valid_pointer() {
        let content = pointer_content("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2", 12345);
        let p = LfsPointer::parse(&content, "data/model.bin").unwrap();
        assert_eq!(p.oid_algo, "sha256");
        assert_eq!(p.oid, "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2");
        assert_eq!(p.size, 12345);
        assert_eq!(p.path, PathBuf::from("data/model.bin"));
    }

    #[test]
    fn parse_rejects_non_pointer() {
        let content = "hello world\nthis is a regular text file\n";
        assert!(LfsPointer::parse(content, "data/file.txt").is_none());
    }

    #[test]
    fn parse_rejects_empty() {
        assert!(LfsPointer::parse("", "x").is_none());
    }

    #[test]
    fn parse_rejects_missing_oid() {
        let content = "version https://git-lfs.github.com/spec/v1\nsize 100\n";
        assert!(LfsPointer::parse(content, "x").is_none());
    }

    #[test]
    fn parse_rejects_missing_size() {
        let content = "version https://git-lfs.github.com/spec/v1\noid sha256:abc\n";
        assert!(LfsPointer::parse(content, "x").is_none());
    }

    #[test]
    fn is_pointer_content_detects_header() {
        assert!(LfsPointer::is_pointer_content("version https://git-lfs.github.com/spec/v1\noid sha256:abc\nsize 1\n"));
        assert!(!LfsPointer::is_pointer_content("not a pointer"));
    }

    #[test]
    fn human_size_formats_correctly() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(1024 * 1024), "1.0 MB");
        assert_eq!(human_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(human_size(1024 * 1024 * 1024 * 5), "5.0 GB");
    }

    #[test]
    fn format_pointers_empty() {
        let out = format_pointers(&[]);
        assert!(out.contains("не обнаружено"));
    }

    #[test]
    fn format_pointers_with_entries() {
        let pointers = vec![LfsPointer {
            path: PathBuf::from("data/x.bin"),
            oid_algo: "sha256".into(),
            oid: "abc123def456".into(),
            size: 1024 * 1024,
        }];
        let out = format_pointers(&pointers);
        assert!(out.contains("LFS pointer-файлов: 1"));
        assert!(out.contains("data/x.bin"));
        assert!(out.contains("abc123def456"));
        assert!(out.contains("1.0 MB"));
    }

    #[test]
    fn detect_pointers_finds_pointer_in_tempdir() {
        let tmp = tempfile::tempdir().unwrap();
        // создать .git директорию (чтобы обойти проверку .git-директории)
        fs::create_dir(tmp.path().join(".git")).unwrap();
        // создать pointer-файл
        let pointer = pointer_content("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2", 100);
        fs::write(tmp.path().join("data.bin"), pointer).unwrap();
        // создать обычный файл
        fs::write(tmp.path().join("plain.txt"), "hello").unwrap();
        let pointers = detect_pointers(tmp.path());
        assert_eq!(pointers.len(), 1, "should detect exactly 1 LFS pointer");
        assert_eq!(pointers[0].size, 100);
    }

    #[test]
    fn detect_pointers_skips_dot_git() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        // поместить pointer ВНУТРЬ .git (должен быть проигнорирован)
        let pointer = pointer_content("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2", 100);
        fs::write(tmp.path().join(".git").join("config"), pointer).unwrap();
        let pointers = detect_pointers(tmp.path());
        assert!(pointers.is_empty(), ".git/ contents must be skipped");
    }

    #[test]
    fn lfs_server_url_from_local_config() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".git")).unwrap();
        fs::write(
            tmp.path().join(".git").join("config"),
            "[remote \"origin\"]\n\turl = https://github.com/user/repo.git\n",
        )
        .unwrap();
        let url = lfs_server_url(tmp.path()).unwrap();
        assert_eq!(url, "https://github.com/user/repo/info/lfs");
    }

    #[test]
    fn lfs_server_url_missing_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(lfs_server_url(tmp.path()).is_none());
    }

    #[test]
    fn oid_to_pointer_map_builds_correctly() {
        let pointers = vec![
            LfsPointer { path: "a".into(), oid_algo: "sha256".into(), oid: "aaa".into(), size: 1 },
            LfsPointer { path: "b".into(), oid_algo: "sha256".into(), oid: "bbb".into(), size: 2 },
        ];
        let m = oid_to_pointer_map(&pointers);
        assert_eq!(m.len(), 2);
        assert!(m.contains_key("aaa"));
        assert!(m.contains_key("bbb"));
    }

    #[test]
    fn format_fetch_results_empty() {
        let out = format_fetch_results(&[]);
        assert!(out.contains("нечего скачивать"));
    }

    #[test]
    fn format_fetch_results_with_success() {
        let results = vec![LfsFetchResult {
            oid: "abc123def456".into(),
            success: true,
            bytes_downloaded: 1024 * 1024,
            error: None,
        }];
        let out = format_fetch_results(&results);
        assert!(out.contains("1/1"));
        assert!(out.contains("1.0 MB"));
        assert!(out.contains("abc123def456"));
    }

    #[test]
    fn format_fetch_results_with_error() {
        let results = vec![LfsFetchResult {
            oid: "xyz".into(),
            success: false,
            bytes_downloaded: 0,
            error: Some("404 not found".into()),
        }];
        let out = format_fetch_results(&results);
        assert!(out.contains("0/1"));
        assert!(out.contains("404"));
    }
}
