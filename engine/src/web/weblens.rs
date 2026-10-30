//! WebLens — браузерная поверхность POLER Engine (Manifest V3).
//!
//! Расширение встроено в бинарник (`include_bytes!`) и материализуется
//! движком одной командой `--web-lens`: бинарь самодостаточен, отдельного
//! дистрибутива расширения нет. Архитектура:
//!
//! ```text
//! poler-engine --web-lens
//!   ├── материализация weblens/ в ~/.local/share/poler-engine/weblens
//!   │     (+ config.json с endpoint и токеном MCP-сервера)
//!   ├── запуск ОКОННОГО Chromium с --load-extension=<weblens>
//!   │     (автоустановка: движок управляет браузером — расширение уже в нём)
//!   └── MCP over HTTP на 127.0.0.1:8765 (тот же --mcp-http)
//!         ↑
//!   side panel / content script → JSON-RPC tools/call (Bearer)
//! ```
//!
//! Для ЕЖЕДНЕВНОГО браузера пользователя (не управляемого движком):
//! `--web-lens-install` — материализация + пошаговая инструкция
//! «Load unpacked» (chrome://-страницы автоматизировать нельзя — это
//! защита самого браузера, честно документируем).

use std::io::Write;
use std::path::{Path, PathBuf};

/// Файлы расширения, вшитые в бинарник: (относительный путь, байты).
const FILES: &[(&str, &[u8])] = &[
    ("manifest.json", include_bytes!("../../weblens/manifest.json")),
    ("background.js", include_bytes!("../../weblens/background.js")),
    ("panel.html", include_bytes!("../../weblens/panel.html")),
    ("panel.css", include_bytes!("../../weblens/panel.css")),
    ("panel.js", include_bytes!("../../weblens/panel.js")),
    ("content.js", include_bytes!("../../weblens/content.js")),
    ("icons/icon16.png", include_bytes!("../../weblens/icons/icon16.png")),
    ("icons/icon32.png", include_bytes!("../../weblens/icons/icon32.png")),
    ("icons/icon48.png", include_bytes!("../../weblens/icons/icon48.png")),
    ("icons/icon128.png", include_bytes!("../../weblens/icons/icon128.png")),
];

/// Директория материализации расширения:
/// `$POLER_WEBLENS_DIR` → `~/.local/share/poler-engine/weblens`.
pub fn weblens_dir() -> PathBuf {
    if let Ok(p) = std::env::var("POLER_WEBLENS_DIR") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/poler-engine/weblens")
}

/// Файл токена WebLens (0600, генерируется при первом запуске).
fn token_file() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config/poler-engine/weblens-token")
}

/// Токен WebLens: читаем сохранённый или генерируем новый (hex 32 байта).
/// Живёт отдельно от POLER_MCP_TOKEN: расширение и туннель не смешиваются.
pub fn weblens_token() -> Result<String, String> {
    let f = token_file();
    if let Ok(t) = std::fs::read_to_string(&f) {
        let t = t.trim().to_string();
        if t.len() >= 32 {
            return Ok(t);
        }
    }
    let token = crate::mcp_http::generate_token();
    if let Some(dir) = f.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    }
    let mut fh = std::fs::File::create(&f).map_err(|e| format!("{}: {e}", f.display()))?;
    // 0600: токен даёт доступ к MCP-инструментам движка с localhost
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fh.set_permissions(std::fs::Permissions::from_mode(0o600));
    }
    writeln!(fh, "{token}").map_err(|e| format!("запись токена: {e}"))?;
    Ok(token)
}

/// Материализация расширения в `dir` + config.json (endpoint, token).
/// Идемпотентно: повторный вызов обновляет файлы (новая версия движка
/// приносит новую версию WebLens).
pub fn materialize_into(dir: &Path, endpoint: &str, token: &str) -> Result<PathBuf, String> {
    for (rel, bytes) in FILES {
        let dest = dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        std::fs::write(&dest, bytes).map_err(|e| format!("{}: {e}", dest.display()))?;
    }
    // config.json — единственный файл, НЕ вшитый в бинарник: endpoint+токен.
    // Аудит-фикс №H1: файл содержит MCP-токен — права 0600 с момента
    // создания (раньше fs::write давал 0644: на multi-user-хостах с домашней
    // директорией 0755 токен читали другие локальные пользователи).
    let cfg = serde_json::json!({
        "endpoint": endpoint,
        "token": token,
    });
    let cfg_path = dir.join("config.json");
    let cfg_body = serde_json::to_string_pretty(&cfg).unwrap_or_default();
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&cfg_path)
            .map_err(|e| format!("{}: {e}", cfg_path.display()))?;
        f.write_all(cfg_body.as_bytes())
            .map_err(|e| format!("{}: {e}", cfg_path.display()))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&cfg_path, &cfg_body)
            .map_err(|e| format!("{}: {e}", cfg_path.display()))?;
    }
    validate_mv3(dir)?;
    Ok(dir.to_path_buf())
}

/// Материализация в стандартную директорию (`weblens_dir()`).
pub fn materialize(endpoint: &str, token: &str) -> Result<PathBuf, String> {
    materialize_into(&weblens_dir(), endpoint, token)
}

/// Санити-проверка материализованного расширения: manifest.json парсится,
/// manifest_version == 3, все заявленные файлы существуют.
/// Вызывается и из unit-тестов, и после каждой материализации.
pub fn validate_mv3(dir: &Path) -> Result<(), String> {
    let manifest_path = dir.join("manifest.json");
    let raw = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("manifest.json: {e}"))?;
    let m: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("manifest.json не JSON: {e}"))?;
    if m.get("manifest_version").and_then(|v| v.as_u64()) != Some(3) {
        return Err("manifest_version != 3 — Chrome отклонит расширение".into());
    }
    for key in ["background", "side_panel"] {
        if m.get(key).is_none() {
            return Err(format!("в manifest нет ключа {key} (MV3 side panel)"));
        }
    }
    let sw = m
        .pointer("/background/service_worker")
        .and_then(|v| v.as_str())
        .ok_or("background.service_worker не задан")?;
    if !dir.join(sw).is_file() {
        return Err(format!("service_worker {sw} не материализован"));
    }
    let panel = m
        .pointer("/side_panel/default_path")
        .and_then(|v| v.as_str())
        .ok_or("side_panel.default_path не задан")?;
    if !dir.join(panel).is_file() {
        return Err(format!("side panel {panel} не материализован"));
    }
    Ok(())
}

/// Поиск ОКОННОГО браузера (не headless-shell: у того нет окна).
/// Порядок: chromium-сборки → google-chrome → playwright-кеш (полный chromium).
pub fn find_windowed_browser() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("POLER_CHROME_BIN") {
        let p = PathBuf::from(p);
        if p.is_file() && !p.to_string_lossy().contains("headless-shell") {
            return Some(p);
        }
    }
    // полный chromium из playwright-кеша предпочтительнее branded chrome:
    // google-chrome (v137+) блокирует --load-extension
    let home = std::env::var("HOME").ok()?;
    let pw = PathBuf::from(&home).join(".cache/ms-playwright");
    let full = crate::web::playwright_candidates(&pw)
        .into_iter()
        .find(|p| !p.to_string_lossy().contains("headless-shell"));
    if let Some(p) = full {
        return Some(p);
    }
    for name in [
        "chromium",
        "chromium-browser",
        "brave-browser",
        "microsoft-edge",
        "google-chrome",
        "google-chrome-stable",
        "chrome",
    ] {
        if let Some(path) = std::env::var("PATH")
            .ok()?
            .split(':')
            .map(|dir| Path::new(dir).join(name))
            .find(|p| p.is_file())
        {
            return Some(path);
        }
    }
    ["/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome", "/snap/bin/chromium"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

/// Отдельный профиль для браузера WebLens (не смешиваем с google-профилем:
/// SingletonLock не даст двум окнам жить в одном профиле).
pub fn weblens_profile_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".cache/poler-engine/weblens-profile")
}

/// Запуск оконного браузера с WebLens (автоустановка: расширение загружено
/// флагом --load-extension — ноль кликов в chrome://extensions).
/// Браузер отсоединён от родителя: живёт и после Ctrl+C демона.
pub fn spawn_windowed_browser(ext_dir: &Path, start_url: &str) -> Result<std::process::Child, String> {
    let bin = find_windowed_browser().ok_or_else(|| {
        "Оконный браузер не найден (headless-shell не умеет окна).\n\
         Установите chromium или укажите POLER_CHROME_BIN к полному браузеру,\n\
         либо используйте --web-lens-install для своего браузера."
            .to_string()
    })?;
    let profile = weblens_profile_dir();
    std::fs::create_dir_all(&profile).map_err(|e| format!("профиль: {e}"))?;
    let log = std::env::temp_dir().join("poler-weblens-browser.log");
    let log_f = std::fs::File::options()
        .create(true)
        .append(true)
        .open(&log)
        .map_err(|e| format!("лог {log:?}: {e}"))?;
    let branded = bin.to_string_lossy().contains("google-chrome")
        || (bin.file_name().map(|n| n == "chrome").unwrap_or(false)
            && !bin.to_string_lossy().contains("chromium"));
    let child = std::process::Command::new(&bin)
        .args([
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-blink-features=AutomationControlled",
            &format!("--user-data-dir={}", profile.display()),
            // CDP того же движка — для будущих сценариев (скриншоты, deep-reading)
            "--remote-debugging-port=9223",
            &format!("--load-extension={}", ext_dir.display()),
            start_url,
        ])
        .stdout(log_f.try_clone().map_err(|e| e.to_string())?)
        .stderr(log_f)
        .spawn()
        .map_err(|e| format!("запуск {:?}: {e}", bin))?;
    if branded {
        eprintln!(
            "poler-weblens: найден google-chrome — с v137 он блокирует --load-extension.\n\
             Если панель не появилась: поставьте chromium, либо загрузите расширение\n\
             вручную (--web-lens-install распечатает шаги)."
        );
    }
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("weblens-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn materialize_creates_mv3_extension_with_config() {
        let dir = temp_dir("materialize");
        let out = materialize_into(&dir, "http://127.0.0.1:8765/", "test-token-1234567890")
            .expect("материализация");
        assert_eq!(out, dir);
        // манифест на месте и это строгий MV3
        validate_mv3(&dir).expect("MV3-валидность");
        // все вшитые файлы материализованы байт-в-байт
        for (rel, bytes) in FILES {
            let got = std::fs::read(dir.join(rel)).unwrap_or_default();
            assert_eq!(got.as_slice(), *bytes, "файл {rel} испорчен при материализации");
        }
        // config.json: endpoint + токен для service worker
        let cfg: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap())
                .unwrap();
        assert_eq!(cfg["endpoint"], "http://127.0.0.1:8765/");
        assert_eq!(cfg["token"], "test-token-1234567890");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_mv3_rejects_broken_dir() {
        let dir = temp_dir("broken");
        std::fs::create_dir_all(&dir).unwrap();
        // пусто — манифеста нет
        assert!(validate_mv3(&dir).is_err());
        // не-MV3 манифест
        std::fs::write(dir.join("manifest.json"), r#"{"manifest_version": 2}"#).unwrap();
        assert!(validate_mv3(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn embedded_manifest_is_strict_mv3() {
        // манифест в репо — валиден до компиляции, а не только после записи
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("weblens");
        validate_mv3(&root).expect("встроенный манифест — строгий MV3");
    }

    #[test]
    fn embedded_manifest_no_remote_code() {
        // MV3 запрещает удалённый код: все скрипты — локальные файлы,
        // запрещённых ключей быть не должно
        let raw = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("weblens/manifest.json"),
        )
        .unwrap();
        let m: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(m.get("content_security_policy").is_none(), "CSP не переопределяем");
        let perms = m["permissions"].as_array().unwrap();
        for p in perms {
            let p = p.as_str().unwrap();
            assert!(
                ["sidePanel", "activeTab", "scripting"].contains(&p),
                "минимальные permissions, лишнее: {p}"
            );
        }
    }
}
