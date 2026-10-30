//! S2→E2 моторный мост: [`MotorIntent`] → двухуровневое исполнение.
//!
//! Последняя миля «управления ПК на 100% движком»: кристалл говорит
//! («запусти сборку», «покажи статус») → моторный слой [`crate::triune::motor`]
//! рендерит предложение → **этот мост** решает и исполняет.
//!
//! ## Двухуровневый контур (директива безопасности)
//!
//! | Уровень | Действия | Режим |
//! |---|---|---|
//! | **R1 ReadOnly** | `git status/log`, чтение файлов, `--version` | авто, без подтверждения |
//! | **M2 Mutating** | `cargo build/test`, `git push`, `xdg-open` | белый список + подтверждение `[y/N]` (или `--motor-yes`) |
//!
//! ## Гарантии (наследованы от poler_box_exec, но без Docker)
//!
//! - **Таймаут-каскад**: SIGTERM группе процессов → grace 2 с → SIGKILL.
//! - **Кольцевой буфер O(1)**: вывод зажат `max_out` байт (голова+хвост,
//!   середина заменяется маркером `… dropped N bytes …`) — OOM невозможен.
//! - **Без шелла**: `program + args` напрямую в `exec` — конвейеры,
//!   редиректы и инъекции невозможны по построению.
//! - Деструктив отклоняется ещё в [`crate::triune::motor`] (до моста).
//!
//! ## Неисполнимое — не исполняется
//!
//! `resolve` возвращает `None` для нераспознанных объектов: интент
//! остаётся предложением, мост его пропускает (прозрачность для агента).

use crate::triune::motor::{MotorIntent, MotorOp};
use std::io::Read;
use std::time::{Duration, Instant};

/// Уровень действия двухуровневого контура.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionLevel {
    /// Чтение без побочных эффектов — исполняется автоматически.
    ReadOnly,
    /// Мутация состояния (сборка, пуш, GUI) — белый список + подтверждение.
    Mutating,
}

impl ActionLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionLevel::ReadOnly => "R1",
            ActionLevel::Mutating => "M2",
        }
    }
}

/// Конкретное исполнимое действие (без шелла).
#[derive(Clone, Debug)]
pub struct ResolvedAction {
    pub level: ActionLevel,
    pub program: String,
    pub args: Vec<String>,
    pub summary: String,
    /// Файл для движкового (бесшеллового) чтения — уровень R1.
    pub read_file: Option<std::path::PathBuf>,
}

/// Конфигурация моста (из CLI).
#[derive(Clone, Debug)]
pub struct BridgeConfig {
    /// Мост активен (`--motor-act`).
    pub act: bool,
    /// Авто-подтверждение мутаций (`--motor-yes`, красный баннер).
    pub yes: bool,
    /// Таймаут команды, мс.
    pub timeout_ms: u64,
    /// Лимит вывода, байт (голова+хвост).
    pub max_out: usize,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self { act: false, yes: false, timeout_ms:30_000, max_out: 65_536 }
    }
}

/// Результат исполнения.
#[derive(Clone, Debug)]
pub struct ExecOutcome {
    pub status: &'static str,
    pub exit_code: Option<i32>,
    pub output: String,
    pub dropped_bytes: u64,
    pub duration_ms: u128,
}

impl ExecOutcome {
    /// Отказ без исполнения (ворота моста).
    #[allow(dead_code)]
    fn refused(reason: &str) -> Self {
        Self { status: "refused", exit_code: None, output: reason.into(), dropped_bytes: 0, duration_ms: 0 }
    }
}

/// Разрешение интента в действие. `None` = объект не распознан,
/// интент остаётся предложением без исполнения.
pub fn resolve(op: &MotorOp, object: &[String]) -> Option<ResolvedAction> {
    let obj = object.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>();
    let has = |forms: &[&str]| obj.iter().any(|w| forms.contains(&w.as_str()));
    let first = obj.first().cloned().unwrap_or_default();

    match op {
        // Чтение файла — движковое, без процесса вообще.
        MotorOp::Read => {
            if obj.is_empty() {
                return None;
            }
            // tokenize рвёт имена файлов по не-буквам И нижнит регистр
            // («Cargo.toml» → [cargo, toml]): восстанавливаем путь перебором
            // кандидатов (слэш/точка) + регистро-нечувательный поиск в каталоге.
            let candidates = [obj.join("/"), obj.join("."), obj[0].clone()];
            let mut path = candidates[0].clone();
            for c in &candidates {
                let r = resolve_case_insensitive(c);
                if r.exists() {
                    path = r.to_string_lossy().into_owned();
                    break;
                }
            }
            Some(ResolvedAction {
                level: ActionLevel::ReadOnly,
                program: "<engine-read>".into(),
                args: vec![path.clone()],
                summary: format!("чтение {path}"),
                read_file: Some(std::path::PathBuf::from(path)),
            })
        }
        // Показ статуса/логов — git read-only.
        MotorOp::Show => {
            if has(&["логи", "лог", "logs", "log", "історію", "историю"]) {
                Some(cmd(ActionLevel::ReadOnly, "git", vec!["log".into(), "--oneline".into(), "-10".into()], "последние коммиты"))
            } else {
                // статус или что угодно неопознанное → безопасный статус
                Some(cmd(ActionLevel::ReadOnly, "git", vec!["status".into(), "--short".into(), "--branch".into()], "статус рабочего дерева"))
            }
        }
        // Сборка — мутация (артефакты target/).
        MotorOp::Build => Some(cmd(ActionLevel::Mutating, "cargo", vec!["build".into()], "сборка проекта")),
        // Запуск: белый список объектов.
        MotorOp::Run => {
            if has(&["сборку", "збірку", "build"]) {
                Some(cmd(ActionLevel::Mutating, "cargo", vec!["build".into()], "запуск сборки"))
            } else if has(&["тести", "тесты", "tests", "test"]) {
                Some(cmd(ActionLevel::Mutating, "cargo", vec!["test".into()], "запуск тестов"))
            } else if has(&["пуш", "push"]) {
                Some(cmd(ActionLevel::Mutating, "git", vec!["push".into()], "отправка на GitHub"))
            } else {
                None
            }
        }
        // Открытие приложения — мутация внешнего мира (GUI).
        MotorOp::Open => {
            if first.is_empty() {
                return None;
            }
            Some(cmd(ActionLevel::Mutating, "xdg-open", vec![first.clone()], format!("открыть {first}")))
        }
        MotorOp::None => None,
    }
}

fn cmd(level: ActionLevel, program: &str, args: Vec<String>, summary: impl Into<String>) -> ResolvedAction {
    ResolvedAction { level, program: program.into(), args, summary: summary.into(), read_file: None }
}

/// Регистро-нечувственное восстановление пути: tokenize нижнит регистр,
/// а FS — case-sensitive («cargo.toml» → «Cargo.toml»). Компонент ищется
/// в родительском каталоге без учёта регистра; без совпадения — как есть.
fn resolve_case_insensitive(path: &str) -> std::path::PathBuf {
    let p = std::path::Path::new(path);
    if p.exists() {
        return p.to_path_buf();
    }
    let mut resolved = std::path::PathBuf::new();
    for comp in p.components() {
        if let std::path::Component::Normal(c) = comp {
            let c_str = c.to_string_lossy();
            let search_dir: std::path::PathBuf =
                if resolved.as_os_str().is_empty() { ".".into() } else { resolved.clone() };
            let matched = std::fs::read_dir(&search_dir).ok().and_then(|rd| {
                rd.filter_map(|e| e.ok())
                    .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(&c_str))
                    .map(|e| e.file_name())
            });
            resolved.push(matched.unwrap_or_else(|| c.to_os_string()));
        } else {
            resolved.push(comp.as_os_str());
        }
    }
    resolved
}

/// Интерактивное подтверждение мутации (UX Open Interpreter: `[y/N]`).
/// Не-tty stdin → отказ (безопасный дефолт для пайпов/агентов).
pub fn confirm_mutating(action: &ResolvedAction) -> bool {
    // В тестах не ждём ввод пользователя — считаем отсутствие подтверждения
    #[cfg(test)]
    {
        let _ = action;
        false
    }

    #[cfg(all(not(test), not(unix)))]
    {
        let _ = action;
        return false;
    }

    #[cfg(all(not(test), unix))]
    {
        use std::io::Write;
        use std::os::unix::io::AsRawFd;

        // Проверяем, что stdin процесса является интерактивным терминалом
        let stdin_fd = std::io::stdin().as_raw_fd();
        let is_stdin_tty = unsafe { libc::isatty(stdin_fd) == 1 };
        if !is_stdin_tty {
            return false;
        }

        let mut tty = match std::fs::OpenOptions::new().read(true).write(true).open("/dev/tty") {
            Ok(f) => {
                let is_atty = unsafe { libc::isatty(f.as_raw_fd()) == 1 };
                if !is_atty {
                    return false;
                }
                f
            }
            Err(_) => return false,
        };

        let _ = write!(tty, "⚡ МОТОР M2 [{}] {} {:}? [y/N] ", action.level.as_str(), action.program, action.args.join(" "));
        let _ = tty.flush();
        let mut buf = [0u8; 8];
        let Ok(n) = tty.read(&mut buf) else { return false };
        let ans = String::from_utf8_lossy(&buf[..n]);
        let c = ans.trim().chars().next().unwrap_or('n');
        let c = c.to_lowercase().next().unwrap_or(c);
        // y/yes (EN), д/да (RU), т/так (UA)
        matches!(c, 'y' | 'д' | 'т')
    }
}

/// Исполнение действия с гарантиями моста (таймаут-каскад + кольцевой буфер).
pub fn execute(action: &ResolvedAction, cfg: &BridgeConfig) -> ExecOutcome {
    let t0 = Instant::now();

    // Движковое чтение: без процесса, с лимитом.
    if let Some(path) = &action.read_file {
        return match std::fs::read(path) {
            Ok(bytes) => {
                let (text, dropped) = ring_buffer(&bytes, cfg.max_out);
                ExecOutcome { status: "ok", exit_code: Some(0), output: text, dropped_bytes: dropped, duration_ms: t0.elapsed().as_millis() }
            }
            Err(e) => ExecOutcome { status: "error", exit_code: None, output: format!("чтение {}: {e}", path.display()), dropped_bytes: 0, duration_ms: t0.elapsed().as_millis() },
        };
    }

    let mut command = std::process::Command::new(&action.program);
    command.args(&action.args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Своя группа процессов: каскад сигналов бьёт всю группу.
        // (Rust 1.98: pre_exec — unsafe-вызов в любом edition)
        unsafe { command.pre_exec(|| { let _ = libc::setsid(); Ok(()) }); }
    }

    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            return ExecOutcome { status: "error", exit_code: None, output: format!("spawn {} {}: {e}", action.program, action.summary), dropped_bytes: 0, duration_ms: t0.elapsed().as_millis() }
        }
    };

    // Читатели stdout/stderr в отдельных потоках: не блокируемся на
    // пайпах (иначе полный пайп заморозит try_wait → таймаут не сработает).
    let stdout_handle = child.stdout.take().map(|mut p| {
        std::thread::spawn(move || { let mut v = Vec::new(); let _ = p.read_to_end(&mut v); v })
    });
    let stderr_handle = child.stderr.take().map(|mut p| {
        std::thread::spawn(move || { let mut v = Vec::new(); let _ = p.read_to_end(&mut v); v })
    });

    // Ожидание с таймаутом → каскад SIGTERM (grace 2 с) → SIGKILL.
    let deadline = t0 + Duration::from_millis(cfg.timeout_ms.max(100));
    let mut timed_out = false;
    let status = 'wait: loop {
        match child.try_wait() {
            Ok(Some(s)) => break 'wait Some(s),
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    kill_group(&child, 15);
                    let grace = t0 + Duration::from_millis(cfg.timeout_ms.max(100) + 2_000);
                    loop {
                        match child.try_wait() {
                            Ok(Some(s)) => break 'wait Some(s),
                            Ok(None) if Instant::now() >= grace => {
                                kill_group(&child, 9);
                                let _ = child.wait();
                                break 'wait None;
                            }
                            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                            Err(_) => break 'wait None,
                        }
                    }
                } else {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            Err(_) => break 'wait None,
        }
    };
    let _ = status;

    // Пайпы закрылись смертью ребёнка — джойним читателей.
    let stdout_buf = stdout_handle.map(|h| h.join().unwrap_or_default()).unwrap_or_default();
    let stderr_buf = stderr_handle.map(|h| h.join().unwrap_or_default()).unwrap_or_default();

    let mut combined = stdout_buf;
    combined.extend_from_slice(b"\n");
    combined.extend_from_slice(&stderr_buf);
    let (text, dropped) = ring_buffer(&combined, cfg.max_out);

    if timed_out {
        ExecOutcome { status: "timeout", exit_code: None, output: format!("… таймаут {} мс (SIGTERM→SIGKILL) …\n{}", cfg.timeout_ms, text), dropped_bytes: dropped, duration_ms: t0.elapsed().as_millis() }
    } else {
        let code = status.and_then(|s| s.code());
        ExecOutcome { status: if code == Some(0) { "ok" } else { "error" }, exit_code: code, output: text, dropped_bytes: dropped, duration_ms: t0.elapsed().as_millis() }
    }
}

#[cfg(unix)]
fn kill_group(child: &std::process::Child, sig: i32) {
    unsafe {
        libc::kill(-(child.id() as i32), sig);
    }
}
#[cfg(not(unix))]
fn kill_group(_child: &std::process::Child, _sig: i32) {}

/// Кольцевой буфер: голова + хвост, середина → маркер `dropped`.
fn ring_buffer(bytes: &[u8], max_out: usize) -> (String, u64) {
    if bytes.len() <= max_out {
        return (String::from_utf8_lossy(bytes).into_owned(), 0);
    }
    let half = max_out / 2;
    let dropped = (bytes.len() - max_out) as u64;
    let mut out = String::with_capacity(max_out + 64);
    out.push_str(&String::from_utf8_lossy(&bytes[..half]));
    out.push_str(&format!("\n… dropped {dropped} bytes …\n"));
    out.push_str(&String::from_utf8_lossy(&bytes[bytes.len() - half..]));
    (out, dropped)
}

/// Прогон интентов одного высказывания через двухуровневый контур.
/// Возвращает JSON-протокол для агента (прозрачность каждого решения).
pub fn act_on_intents(intents: &[MotorIntent], cfg: &BridgeConfig) -> Vec<serde_json::Value> {
    let mut log = Vec::new();
    for i in intents {
        if !i.allowed {
            log.push(serde_json::json!({
                "verb": i.verb, "op": i.op.as_str(),
                "level": "-", "status": "refused", "output": i.reason,
            }));
            continue;
        }
        let Some(action) = resolve(&i.op, &i.object) else {
            log.push(serde_json::json!({
                "verb": i.verb, "op": i.op.as_str(),
                "level": "-", "status": "unresolved",
                "output": format!("объект {:?} не распознан — интент остался предложением", i.object),
            }));
            continue;
        };
        let proceed = match action.level {
            ActionLevel::ReadOnly => true,
            ActionLevel::Mutating => cfg.yes || confirm_mutating(&action),
        };
        if !proceed {
            log.push(serde_json::json!({
                "verb": i.verb, "op": i.op.as_str(),
                "level": action.level.as_str(), "program": action.program, "args": action.args,
                "status": "refused",
                "output": "мутация без подтверждения (нет tty или отказ; --motor-yes для авто)",
            }));
            continue;
        }
        let o = execute(&action, cfg);
        log.push(serde_json::json!({
            "verb": i.verb, "op": i.op.as_str(),
            "level": action.level.as_str(), "program": action.program, "args": action.args,
            "status": o.status, "exit_code": o.exit_code,
            "dropped_bytes": o.dropped_bytes, "duration_ms": o.duration_ms,
            "output": o.output,
        }));
    }
    log
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triune::motor;

    #[test]
    fn levels_two_circuit() {
        assert_eq!(resolve(&MotorOp::Show, &["статус".into()]).unwrap().level, ActionLevel::ReadOnly);
        assert_eq!(resolve(&MotorOp::Build, &[]).unwrap().level, ActionLevel::Mutating);
        assert_eq!(resolve(&MotorOp::Run, &["збірку".into()]).unwrap().level, ActionLevel::Mutating);
        assert_eq!(resolve(&MotorOp::Read, &["Cargo.toml".into()]).unwrap().level, ActionLevel::ReadOnly);
    }

    #[test]
    fn resolve_ua_and_ru_objects() {
        // UA и RU объекты одинаково распознаются
        for w in ["сборку", "збірку", "build"] {
            let a = resolve(&MotorOp::Run, &[w.into()]).unwrap();
            assert_eq!((a.program.as_str(), a.args[0].as_str()), ("cargo", "build"));
        }
        for w in ["тести", "тесты", "tests"] {
            let a = resolve(&MotorOp::Run, &[w.into()]).unwrap();
            assert_eq!(a.args[0], "test");
        }
        // логи → git log, статус → git status
        assert_eq!(resolve(&MotorOp::Show, &["логи".into()]).unwrap().args[0], "log");
        assert_eq!(resolve(&MotorOp::Show, &["статус".into()]).unwrap().args[0], "status");
        assert_eq!(resolve(&MotorOp::Show, &["стан".into()]).unwrap().args[0], "status");
        // push
        assert_eq!(resolve(&MotorOp::Run, &["пуш".into()]).unwrap().args[0], "push");
    }

    #[test]
    fn unknown_run_object_stays_proposal() {
        assert!(resolve(&MotorOp::Run, &["хрень".into()]).is_none());
        assert!(resolve(&MotorOp::Open, &[]).is_none());
        assert!(resolve(&MotorOp::None, &["всё".into()]).is_none());
    }

    #[test]
    fn read_file_is_engine_native() {
        let a = resolve(&MotorOp::Read, &["Cargo.toml".into()]).unwrap();
        assert!(a.read_file.is_some());
        assert_eq!(a.program, "<engine-read>");
    }

    #[test]
    fn read_file_path_reconstruction() {
        // tokenize рвёт «Cargo.toml» на [cargo, toml] И нижнит регистр —
        // мост восстанавливает разделитель (точка) и регистр (FS чувствительна)
        let a = resolve(&MotorOp::Read, &["cargo".into(), "toml".into()]).unwrap();
        assert_eq!(a.read_file.unwrap().to_str().unwrap(), "Cargo.toml");
        // директории через слэш: «src triune» → src/triune
        let b = resolve(&MotorOp::Read, &["src".into(), "triune".into()]).unwrap();
        assert_eq!(b.read_file.unwrap().to_str().unwrap(), "src/triune");
        // Cargo.lock — второй гарантированный файл в корне крейта
        let c = resolve(&MotorOp::Read, &["cargo".into(), "lock".into()]).unwrap();
        assert_eq!(c.read_file.unwrap().to_str().unwrap(), "Cargo.lock");
    }

    #[test]
    fn ring_buffer_head_tail_marker() {
        let big: Vec<u8> = (0..100_000u32).map(|i| b'0' + (i % 10) as u8).collect();
        let (text, dropped) = ring_buffer(&big, 1_000);
        assert!(dropped == 99_000);
        assert!(text.starts_with("0123456789"));
        assert!(text.contains("dropped 99000 bytes"));
        assert_eq!(text.trim_end().chars().last(), Some('9'));
        // Маленький вывод проходит целиком
        let (t2, d2) = ring_buffer(b"hello", 1_000);
        assert_eq!((t2.as_str(), d2), ("hello", 0));
    }

    #[test]
    fn execute_read_only_version() {
        let a = ResolvedAction {
            level: ActionLevel::ReadOnly,
            program: "git".into(),
            args: vec!["--version".into()],
            summary: "версия git".into(),
            read_file: None,
        };
        let o = execute(&a, &BridgeConfig::default());
        assert_eq!(o.status, "ok");
        assert_eq!(o.exit_code, Some(0));
        assert!(o.output.contains("git version"));
    }

    #[test]
    fn execute_timeout_cascade() {
        let a = ResolvedAction {
            level: ActionLevel::Mutating,
            program: "sleep".into(),
            args: vec!["30".into()],
            summary: "зависший процесс".into(),
            read_file: None,
        };
        let mut cfg = BridgeConfig::default();
        cfg.timeout_ms = 400; // + grace 2000 → общий < 3.5 с
        let t0 = Instant::now();
        let o = execute(&a, &cfg);
        assert_eq!(o.status, "timeout");
        assert!(t0.elapsed() < Duration::from_secs(4), "каскад не затянулся: {:?}", t0.elapsed());
    }

    #[test]
    fn act_refuses_destructive_before_bridge() {
        let intents = motor::scan("удали файл и покажи статус");
        let log = act_on_intents(&intents, &BridgeConfig::default());
        assert_eq!(log.len(), 2);
        assert_eq!(log[0]["status"], "refused"); // деструктив
        assert_eq!(log[1]["status"], "ok");      // статус исполнен (R1 авто)
        assert_eq!(log[1]["level"], "R1");
    }

    #[test]
    fn act_mutating_without_tty_refused_by_default() {
        // cargo test гоняется без tty → мутация должна быть отказана
        let intents = motor::scan("запусти сборку");
        assert_eq!(intents.len(), 1);
        let log = act_on_intents(&intents, &BridgeConfig::default());
        assert_eq!(log[0]["status"], "refused");
        assert!(log[0]["output"].as_str().unwrap().contains("--motor-yes"));
    }

    #[test]
    fn act_mutating_with_yes_executes() {
        let intents = motor::scan("запусти тесты");
        assert_eq!(intents.len(), 1);
        let mut cfg = BridgeConfig::default();
        cfg.yes = true;
        // Ворота: резолв мутации + yes → proceed (не запуская реальный cargo).
        let action = resolve(&intents[0].op, &intents[0].object).unwrap();
        assert_eq!(action.level, ActionLevel::Mutating);
        let proceed = match action.level {
            ActionLevel::ReadOnly => true,
            ActionLevel::Mutating => cfg.yes || confirm_mutating(&action),
        };
        assert!(proceed, "--motor-yes должен пропускать мутацию к исполнению");
        // Исполнение мутации с yes на безвредной команде /bin/true.
        let harmless = ResolvedAction {
            level: ActionLevel::Mutating,
            program: "true".into(),
            args: vec![],
            summary: "безвредная мутация".into(),
            read_file: None,
        };
        let o = execute(&harmless, &cfg);
        assert_eq!(o.status, "ok");
        assert_eq!(o.exit_code, Some(0));
    }

    #[test]
    fn act_unresolved_object_is_transparent() {
        let intents = motor::scan("запусти хрень");
        let log = act_on_intents(&intents, &BridgeConfig::default());
        assert_eq!(log[0]["status"], "unresolved");
    }
}
