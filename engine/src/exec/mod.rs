//! POLER Exec — идеальный исполнитель команд (E2/v0.32.0, фича `pnd-ffi`).
//!
//! Рождён диагностикой исходников GNU bash 5.2 самим POLER-Engine
//! (docs/EXEC_AUDIT.md): 337 небезопасных strcpy/sprintf/strcat в 71 C-файле,
//! `free()` внутри обработчика сигнала (trap.c:839), гонка REINSTALL_SIGCHLD
//! (потеря SIGCHLD → зомби), неограниченный захват вывода `$(...)` (subst.c),
//! ноль таймаутов на дочерние процессы.
//!
//! Ядро — `os/core/poler_exec.zig` (Zig, raw-syscall слой на ассемблере,
//! ноль libc в ребёнке между fork и exec). Этот модуль — безопасная обвязка:
//! CString-мост, типизированные ошибки, реестр фоновых задач.
//!
//! E2/v0.32.0 — узкие места E1 устранены на уровне ABI:
//! - голова вывода больше не теряется: `capture = HeadTail` держит
//!   первые B/2 + маркер «dropped N bytes» + последние B/2;
//! - PTY: программы, требующие терминал (sudo/fzf/htop, isatty), работают;
//! - PATH-разрешение переехало В РЕБЁНКА (execvp-семантика): родитель
//!   не делает ни одного stat перед fork;
//! - cwd: chdir в бутстрапе ребёнка (родитель многопоточен — трогать
//!   процесс-глобальный cwd ему запрещено);
//! - отмена: атомарный cancel_flag → TERM→KILL за ≤25 мс;
//! - `TaskRegistry`: фоновые задачи для MCP (poler_exec_async/kill).
//!
//! Контракт (все классы bash-ошибок исключены конструктивно):
//! - таймаут: timerfd(MONOTONIC) + SIGTERM → grace → SIGKILL группе;
//! - вывод: O(1) памяти при любом объёме (кольцо / голова+хвост);
//! - зомби: pidfd-пробуждение + wait4(WNOHANG) в ppoll-цикле;
//! - инъекции: argv передаётся массивом, БЕЗ шелл-парсинга;
//! - fds: CLOEXEC-гигиена, чужие дескрипторы ребёнку не протекают.

use std::ffi::{c_char, CString};
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ── C-ABI слоя Zig (os/core/poler_exec.zig) ─────────────────────────────────
//
// ПОРЯДОК ПОЛЕЙ ЗЕРКАЛИТ Zig-структуры 1:1 (repr(C) обеих сторон).

#[repr(C)]
struct ExecOptions {
    timeout_ms: u64,
    grace_ms: u64,
    max_out_bytes: u64,
    stdin_data: *const u8,
    stdin_len: u64,
    cwd: *const c_char,
    pty: u32,
    /// 0 = tail, 1 = head_tail.
    capture: u32,
    cancel_flag: *const u32,
}

#[repr(C)]
struct ExecResult {
    exit_code: i32,
    signal: i32,
    timed_out: u32,
    truncated: u32,
    duration_us: u64,
    stdout_len: u64,
    stderr_len: u64,
    pid: i32,
    cancelled: u32,
}

extern "C" {
    fn poler_exec_run(
        path: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
        opts: *const ExecOptions,
        stdout_buf: *mut u8,
        stdout_cap: usize,
        stderr_buf: *mut u8,
        stderr_cap: usize,
        res: *mut ExecResult,
    ) -> i32;
}

/// Запас под маркер «dropped N bytes» в режиме head_tail: буферы
/// выделяются размером cap + MARKER_SLACK, ядру передаётся бюджет cap.
pub const MARKER_SLACK: usize = 64;

// ── Публичный контракт ──────────────────────────────────────────────────────

/// Режим захвата вывода при переполнении лимита.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptureMode {
    /// Хвост N байт (классика E1): только последние max_out_bytes.
    #[default]
    Tail,
    /// Голова B/2 + маркер «dropped N bytes» + хвост B/2 (E2): стек-трейс
    /// в начале огромного лога больше не теряется.
    HeadTail,
}

impl CaptureMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tail => "tail",
            Self::HeadTail => "head_tail",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "tail" => Some(Self::Tail),
            "head_tail" => Some(Self::HeadTail),
            _ => None,
        }
    }
}

/// Спецификация запуска. `program` — имя из PATH (разрешается В РЕБЁНКЕ,
/// execvp-семантика: ноль stat в родителе) или путь с '/'.
#[derive(Debug, Clone)]
pub struct ExecSpec {
    pub program: String,
    pub args: Vec<String>,
    /// `None` — наследовать окружение текущего процесса.
    pub env: Option<Vec<(String, String)>>,
    /// Жёсткий таймаут, мс. 0 — без таймаута (осторожно!).
    pub timeout_ms: u64,
    /// Grace между SIGTERM и SIGKILL, мс [умолчание в обвязке: 100].
    pub grace_ms: u64,
    /// Лимит захвата НА ПОТОК, байт (хвост либо голова+хвост — см. capture).
    pub max_out_bytes: usize,
    /// Данные в stdin ребёнка.
    pub stdin_data: Option<Vec<u8>>,
    /// Рабочий каталог ребёнка (chdir в бутстрапе ребёнка). `None` — наследовать.
    pub cwd: Option<String>,
    /// Запустить под псевдотерминалом (200x50, stdout/stderr слиты).
    pub pty: bool,
    /// Режим захвата при переполнении лимита вывода.
    pub capture: CaptureMode,
    /// Атомарный флаг отмены (устанавливается извне, например poler_exec_kill).
    pub cancel: Option<Arc<AtomicU32>>,
}

impl Default for ExecSpec {
    fn default() -> Self {
        Self {
            program: String::new(),
            args: Vec::new(),
            env: None,
            timeout_ms: 30_000,
            grace_ms: 100,
            max_out_bytes: 64 * 1024,
            stdin_data: None,
            cwd: None,
            pty: false,
            capture: CaptureMode::Tail,
            cancel: None,
        }
    }
}

/// Итог исполнения.
#[derive(Debug, Clone)]
pub struct ExecOutcome {
    /// Код выхода; `None` — процесс убит сигналом.
    pub exit_code: Option<i32>,
    /// Сигнал, убивший процесс (обычно TERM/KILL после таймаута/отмены).
    pub signal: Option<i32>,
    pub timed_out: bool,
    /// Запуск отменён через cancel_flag (TERM→KILL, как таймаут, но не он).
    pub cancelled: bool,
    /// Вывод превысил лимит — удержан хвост либо голова+маркер+хвост.
    pub truncated: bool,
    /// Захваченный stdout (до `max_out_bytes` + маркер в head_tail).
    pub stdout: Vec<u8>,
    /// Захваченный stderr (ПУСТ при pty — потоки слиты в stdout).
    pub stderr: Vec<u8>,
    /// Полная длительность, мкс (CLOCK_MONOTONIC).
    pub duration_us: u64,
    /// Пид ребёнка (диагностика).
    pub pid: i32,
    /// Запуск был под псевдотерминалом (stdout содержит и stderr).
    pub pty: bool,
}

/// Ошибка запуска (исполнение, начавшееся хотя бы на мгновение,
/// ошибкой не считается — см. `ExecOutcome`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecError {
    /// Программа не найдена (PATH/путь) — ENOENT. Код 127.
    NotFound,
    /// Нет права исполнения — EACCES. Код 126.
    PermissionDenied,
    /// Прочий errno этапа запуска (EAGAIN/ENOMEM/ENOTDIR/EISDIR…).
    SpawnFailed(i32),
    /// Некорректная спецификация (NUL в строках, недоступный cwd и т.п.).
    Invalid(String),
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "программа не найдена"),
            Self::PermissionDenied => write!(f, "нет права исполнения"),
            Self::SpawnFailed(e) => write!(f, "запуск не удался (errno {e})"),
            Self::Invalid(m) => write!(f, "некорректная спецификация: {m}"),
        }
    }
}

impl std::error::Error for ExecError {}

impl ExecError {
    /// Код выхода по конвенции шелла (для CLI/JSON-отчётов).
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotFound => 127,
            Self::PermissionDenied => 126,
            _ => 125,
        }
    }
}

// ── Запуск ──────────────────────────────────────────────────────────────────

/// Выполнить команду по спецификации. Блокирует до смерти ребёнка,
/// таймаута, отмены или исчерпания дренажа — но никогда не зависает
/// навсегда (при `timeout_ms > 0` или `cancel`).
///
/// PATH-разрешение имени выполняет САМ ребёнок (execvp-семантика ядра):
/// родитель не делает ни одного stat — узкое место E1 устранено.
pub fn run(spec: &ExecSpec) -> Result<ExecOutcome, ExecError> {
    if spec.program.is_empty() {
        return Err(ExecError::Invalid("пустое имя программы".into()));
    }
    if spec.program.contains('\0') {
        return Err(ExecError::Invalid("NUL-байт в имени программы".into()));
    }
    // argv[0] — как задан (имя или путь): так делает и шелл.
    let prog_c = CString::new(spec.program.as_str())
        .map_err(|_| ExecError::Invalid("NUL-байт в пути программы".into()))?;

    let mut argv_owned: Vec<CString> = Vec::with_capacity(spec.args.len() + 1);
    argv_owned.push(prog_c.clone());
    for a in &spec.args {
        argv_owned.push(
            CString::new(a.as_str())
                .map_err(|_| ExecError::Invalid("NUL-байт в аргументе".into()))?,
        );
    }
    let argv_ptrs: Vec<*const c_char> = argv_owned
        .iter()
        .map(|s| s.as_ptr())
        .chain(std::iter::once(std::ptr::null()))
        .collect();

    // envp: явное окружение или наследование текущего.
    let env_owned: Vec<CString> = match &spec.env {
        Some(pairs) => pairs
            .iter()
            .map(|(k, v)| {
                CString::new(format!("{k}={v}"))
                    .map_err(|_| ExecError::Invalid("NUL-байт в переменной окружения".into()))
            })
            .collect::<Result<_, _>>()?,
        None => std::env::vars_os()
            .map(|(k, v)| {
                let mut b = k.as_bytes().to_vec();
                b.push(b'=');
                b.extend_from_slice(v.as_bytes());
                CString::new(b)
                    .map_err(|_| ExecError::Invalid("NUL-байт в окружении процесса".into()))
            })
            .collect::<Result<_, _>>()?,
    };
    let envp_ptrs: Vec<*const c_char> = env_owned
        .iter()
        .map(|s| s.as_ptr())
        .chain(std::iter::once(std::ptr::null()))
        .collect();

    let cwd_c: Option<CString> = match &spec.cwd {
        Some(c) => Some(
            CString::new(c.as_str())
                .map_err(|_| ExecError::Invalid("NUL-байт в рабочем каталоге".into()))?,
        ),
        None => None,
    };

    // Буферы захвата: cap == 0 → discard; head_tail → запас под маркер.
    let cap = spec.max_out_bytes;
    let slack = if spec.capture == CaptureMode::HeadTail { MARKER_SLACK } else { 0 };
    let mut stdout_buf = vec![0u8; cap.saturating_add(slack).max(1)];
    let mut stderr_buf = vec![0u8; if spec.pty { 1 } else { cap.saturating_add(slack).max(1) }];
    let (so_ptr, so_cap): (*mut u8, usize) =
        if cap == 0 { (std::ptr::null_mut(), 0) } else { (stdout_buf.as_mut_ptr(), cap) };
    // PTY: stderr слит в stdout терминала — буфер не нужен.
    let (se_ptr, se_cap): (*mut u8, usize) = if cap == 0 || spec.pty {
        (std::ptr::null_mut(), 0)
    } else {
        (stderr_buf.as_mut_ptr(), cap)
    };

    let stdin_ref = spec.stdin_data.as_deref().unwrap_or(&[]);
    // AtomicU32 repr-прозрачен над u32: приведение указателя легально
    // (гарантия std), Zig читает его @atomicLoad(u32, .acquire).
    let cancel_ptr = spec
        .cancel
        .as_ref()
        .map(|a| Arc::as_ptr(a) as *const u32)
        .unwrap_or(std::ptr::null());
    let opts = ExecOptions {
        timeout_ms: spec.timeout_ms,
        grace_ms: spec.grace_ms,
        max_out_bytes: cap as u64,
        stdin_data: if stdin_ref.is_empty() {
            std::ptr::null()
        } else {
            stdin_ref.as_ptr()
        },
        stdin_len: stdin_ref.len() as u64,
        cwd: cwd_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        pty: if spec.pty { 1 } else { 0 },
        capture: match spec.capture {
            CaptureMode::Tail => 0,
            CaptureMode::HeadTail => 1,
        },
        cancel_flag: cancel_ptr,
    };

    let mut res = ExecResult {
        exit_code: -1,
        signal: 0,
        timed_out: 0,
        truncated: 0,
        duration_us: 0,
        stdout_len: 0,
        stderr_len: 0,
        pid: -1,
        cancelled: 0,
    };

    // Единственный unsafe: вызов C-ABI ядра. Все указатели валидны
    // до конца вызова (argv_owned/env_owned/cwd/stdin живут на стеке).
    let rc = unsafe {
        poler_exec_run(
            prog_c.as_ptr(),
            argv_ptrs.as_ptr(),
            envp_ptrs.as_ptr(),
            &opts,
            so_ptr,
            so_cap,
            se_ptr,
            se_cap,
            &mut res,
        )
    };

    if rc != 0 {
        // exit 125 из ядра = провал chdir (cwd), не путать с exec-провалом.
        if res.exit_code == 125 {
            return Err(ExecError::Invalid(format!(
                "рабочий каталог недоступен (errno {})",
                -rc
            )));
        }
        return Err(match -rc {
            2 => ExecError::NotFound,          // ENOENT
            13 => ExecError::PermissionDenied,  // EACCES
            e => ExecError::SpawnFailed(e),
        });
    }

    let so_len = (res.stdout_len as usize).min(cap.saturating_add(slack));
    let se_len = if spec.pty { 0 } else { (res.stderr_len as usize).min(cap.saturating_add(slack)) };

    Ok(ExecOutcome {
        exit_code: if res.exit_code >= 0 { Some(res.exit_code) } else { None },
        signal: if res.signal != 0 { Some(res.signal) } else { None },
        timed_out: res.timed_out != 0,
        cancelled: res.cancelled != 0,
        truncated: res.truncated != 0,
        stdout: if cap == 0 { Vec::new() } else { stdout_buf[..so_len].to_vec() },
        stderr: if cap == 0 || spec.pty { Vec::new() } else { stderr_buf[..se_len].to_vec() },
        duration_us: res.duration_us,
        pid: res.pid,
        pty: spec.pty,
    })
}

// ── Реестр фоновых задач (MCP: poler_exec_async / task / kill / list) ────────

/// Снимок состояния задачи для отчёта (клонируется под локом).
#[derive(Debug, Clone)]
pub enum TaskSnapshot {
    Running { command: String, elapsed_us: u64 },
    Done { command: String, elapsed_us: u64, result: Result<ExecOutcome, ExecError> },
}

/// Фоновая задача исполнителя: живёт в собственном потоке, отменяется
/// атомарным флагом (TERM→KILL за ≤25 мс), результат удерживается для
/// последующих опросов poler_exec_task.
pub struct TaskRegistry {
    inner: Mutex<std::collections::HashMap<u64, TaskEntry>>,
    next_id: AtomicU64,
}

struct TaskEntry {
    command: String,
    started: Instant,
    cancel: Arc<AtomicU32>,
    status: TaskStatus,
}

enum TaskStatus {
    Running,
    Done(Box<Result<ExecOutcome, ExecError>>),
}

/// Максимум задач в реестре: старые завершённые вытесняются.
const TASK_LIMIT: usize = 128;

impl Default for TaskRegistry {
    fn default() -> Self {
        Self { inner: Mutex::new(std::collections::HashMap::new()), next_id: AtomicU64::new(0) }
    }
}

impl TaskRegistry {
    /// Поставить задачу в фон: выделить id, запустить поток, вернуть id.
    /// Поток живёт до смерти ребёнка; реестр удерживает результат.
    /// Ассоциированная функция (не метод): потоку нужна Arc-копия реестра,
    /// а `self: &Arc<Self>` нестабилен как self-тип.
    pub fn spawn(reg: &Arc<Self>, command: String, mut spec: ExecSpec) -> u64 {
        let id = reg.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let cancel = Arc::new(AtomicU32::new(0));
        reg.inner.lock().expect("poler-exec: отравленный лок реестра").insert(
            id,
            TaskEntry {
                command,
                started: Instant::now(),
                cancel: cancel.clone(),
                status: TaskStatus::Running,
            },
        );
        reg.evict();
        let reg = reg.clone();
        std::thread::spawn(move || {
            spec.cancel = Some(cancel);
            let out = run(&spec);
            let mut m = reg.inner.lock().expect("poler-exec: отравленный лок реестра");
            if let Some(e) = m.get_mut(&id) {
                e.status = TaskStatus::Done(Box::new(out));
            }
        });
        id
    }

    /// Снять снимок задачи (клонирование под локом, без блокировки рендера).
    pub fn snapshot(&self, id: u64) -> Option<TaskSnapshot> {
        let m = self.inner.lock().expect("poler-exec: отравленный лок реестра");
        let e = m.get(&id)?;
        let elapsed_us = e.started.elapsed().as_micros() as u64;
        Some(match &e.status {
            TaskStatus::Running => TaskSnapshot::Running { command: e.command.clone(), elapsed_us },
            TaskStatus::Done(r) => TaskSnapshot::Done {
                command: e.command.clone(),
                elapsed_us,
                result: (**r).clone(),
            },
        })
    }

    /// Запросить отмену задачи. `Some(true)` — была жива и отменена,
    /// `Some(false)` — уже завершена, `None` — не найдена.
    pub fn kill(&self, id: u64) -> Option<bool> {
        let mut m = self.inner.lock().expect("poler-exec: отравленный лок реестра");
        let e = m.get_mut(&id)?;
        match &mut e.status {
            TaskStatus::Running => {
                e.cancel.store(1, Ordering::Release);
                Some(true)
            }
            TaskStatus::Done(_) => Some(false),
        }
    }

    /// Все задачи (id, снимок) — для poler_exec_list.
    pub fn list(&self) -> Vec<(u64, TaskSnapshot)> {
        let m = self.inner.lock().expect("poler-exec: отравленный лок реестра");
        let mut ids: Vec<u64> = m.keys().copied().collect();
        ids.sort_unstable();
        ids.into_iter()
            .map(|id| {
                let e = &m[&id];
                let elapsed_us = e.started.elapsed().as_micros() as u64;
                let snap = match &e.status {
                    TaskStatus::Running => {
                        TaskSnapshot::Running { command: e.command.clone(), elapsed_us }
                    }
                    TaskStatus::Done(r) => TaskSnapshot::Done {
                        command: e.command.clone(),
                        elapsed_us,
                        result: (**r).clone(),
                    },
                };
                (id, snap)
            })
            .collect()
    }

    /// Вытеснение завершённых задач за пределами TASK_LIMIT.
    fn evict(&self) {
        let mut m = self.inner.lock().expect("poler-exec: отравленный лок реестра");
        if m.len() <= TASK_LIMIT {
            return;
        }
        let done: Vec<u64> = m
            .iter()
            .filter(|(_, e)| matches!(e.status, TaskStatus::Done(_)))
            .map(|(id, _)| *id)
            .collect();
        for id in done {
            m.remove(&id);
            if m.len() <= TASK_LIMIT {
                break;
            }
        }
    }
}

// ── Тесты (живой fork/exec — как в os/core/poler_exec.zig) ─────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(program: &str, args: &[&str]) -> ExecSpec {
        ExecSpec {
            program: program.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            ..ExecSpec::default()
        }
    }

    #[test]
    fn echo_roundtrip() {
        let out = run(&spec("echo", &["полер-exec", "живой"])).unwrap();
        assert_eq!(out.exit_code, Some(0));
        assert!(!out.timed_out);
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "полер-exec живой");
        assert!(out.pid > 0);
        assert!(!out.cancelled);
    }

    #[test]
    fn exit_code_42() {
        let out = run(&spec("sh", &["-c", "exit 42"])).unwrap();
        assert_eq!(out.exit_code, Some(42));
        assert_eq!(out.signal, None);
    }

    #[test]
    fn stderr_capture() {
        let out = run(&spec("sh", &["-c", "echo err-полер >&2"])).unwrap();
        assert_eq!(out.exit_code, Some(0));
        assert!(out.stdout.is_empty());
        assert_eq!(String::from_utf8_lossy(&out.stderr).trim(), "err-полер");
    }

    #[test]
    fn timeout_kills() {
        let s = ExecSpec { timeout_ms: 80, grace_ms: 40, ..spec("sleep", &["30"]) };
        let out = run(&s).unwrap();
        assert!(out.timed_out);
        assert!(!out.cancelled);
        assert!(out.duration_us < 2_000_000, "длительность {} мкс", out.duration_us);
        assert!(out.signal == Some(15) || out.signal == Some(9));
    }

    #[test]
    fn not_found() {
        let err = run(&spec("/несуществующий/полер", &[])).unwrap_err();
        assert_eq!(err, ExecError::NotFound);
    }

    #[test]
    fn name_from_path_not_found() {
        // E2: PATH ищет ребёнок — NotFound приходит его errno через fail-пайп.
        let err = run(&spec("полер-несуществует-12345", &[])).unwrap_err();
        assert_eq!(err, ExecError::NotFound);
    }

    #[test]
    fn big_output_tail() {
        // 256 КиБ нулей при лимите 4 КиБ: хвост + truncated (режим по умолчанию).
        let s = ExecSpec {
            max_out_bytes: 4096,
            timeout_ms: 5000,
            ..spec("dd", &["if=/dev/zero", "bs=1024", "count=256"])
        };
        let out = run(&s).unwrap();
        assert_eq!(out.exit_code, Some(0));
        assert!(out.truncated);
        assert_eq!(out.stdout.len(), 4096);
        assert!(out.stdout.iter().all(|&b| b == 0));
    }

    #[test]
    fn stdin_roundtrip() {
        let payload = "stdin-полер-42";
        let s = ExecSpec { stdin_data: Some(payload.as_bytes().to_vec()), ..spec("cat", &[]) };
        let out = run(&s).unwrap();
        assert_eq!(out.exit_code, Some(0));
        assert_eq!(out.stdout, payload.as_bytes());
    }

    #[test]
    fn env_override() {
        let s = ExecSpec {
            env: Some(vec![("POLER_TEST_X".into(), "значение-1".into())]),
            ..spec("sh", &["-c", "printf %s \"$POLER_TEST_X\""])
        };
        let out = run(&s).unwrap();
        assert_eq!(out.exit_code, Some(0));
        assert_eq!(out.stdout, "значение-1".as_bytes());
    }

    // ── E2/v0.32.0 ─────────────────────────────────────────────────────

    #[test]
    fn head_tail_keeps_head_and_tail() {
        // Голова (HEAD7) и хвост (TAIL7) при 100x переполнении — оба живы.
        let s = ExecSpec {
            capture: CaptureMode::HeadTail,
            max_out_bytes: 2048,
            timeout_ms: 10_000,
            ..spec(
                "sh",
                &["-c", "printf HEAD7; dd if=/dev/zero bs=1024 count=100 2>/dev/null; printf TAIL7"],
            )
        };
        let out = run(&s).unwrap();
        assert_eq!(out.exit_code, Some(0));
        assert!(out.truncated);
        assert!(out.stdout.len() > 2048, "нет маркера: {}", out.stdout.len());
        assert!(out.stdout.len() <= 2048 + MARKER_SLACK);
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.starts_with("HEAD7"), "голова потеряна");
        assert!(text.ends_with("TAIL7"), "хвост потеряна");
        assert!(text.contains("dropped"), "нет маркера опущенных байт");
    }

    #[test]
    fn pty_makes_isatty_true() {
        let s = ExecSpec {
            pty: true,
            timeout_ms: 5000,
            ..spec("sh", &["-c", "test -t 1 && echo TTY-ПОЛЕР"])
        };
        let out = run(&s).unwrap();
        assert_eq!(out.exit_code, Some(0));
        assert!(out.pty);
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("TTY-ПОЛЕР"), "вывод PTY: {text:?}");
        assert!(out.stderr.is_empty(), "при pty stderr слит в stdout");
    }

    #[test]
    fn cwd_runs_child_in_directory() {
        let s = ExecSpec {
            cwd: Some("/tmp".into()),
            env: Some(vec![("PATH".into(), "/bin:/usr/bin".into())]),
            ..spec("sh", &["-c", "pwd"])
        };
        let out = run(&s).unwrap();
        assert_eq!(out.exit_code, Some(0));
        assert!(String::from_utf8_lossy(&out.stdout).starts_with("/tmp"));
    }

    #[test]
    fn missing_cwd_is_invalid() {
        let s = ExecSpec {
            cwd: Some("/полер/не/существует".into()),
            ..spec("true", &[])
        };
        let err = run(&s).unwrap_err();
        assert!(matches!(err, ExecError::Invalid(_)), "ожидалась Invalid, got {err:?}");
    }

    #[test]
    fn cancel_flag_kills_child() {
        let flag = Arc::new(AtomicU32::new(0));
        let setter = {
            let flag = flag.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(150));
                flag.store(1, Ordering::Release);
            })
        };
        let s = ExecSpec {
            cancel: Some(flag),
            timeout_ms: 30_000, // таймаут не сработает — только отмена
            ..spec("sleep", &["30"])
        };
        let out = run(&s).unwrap();
        setter.join().unwrap();
        assert!(out.cancelled);
        assert!(!out.timed_out, "отмена ≠ таймаут");
        assert!(out.duration_us < 3_000_000, "длительность {} мкс", out.duration_us);
    }

    #[test]
    fn registry_spawn_kill_and_result() {
        let reg = Arc::new(TaskRegistry::default());
        let id = TaskRegistry::spawn(
            &reg,
            "sleep 30".into(),
            ExecSpec { timeout_ms: 60_000, ..spec("sleep", &["30"]) },
        );
        // Жива → отмена → завершена с cancelled.
        match reg.snapshot(id) {
            Some(TaskSnapshot::Running { .. }) => {}
            other => panic!("ожидалась Running, got {other:?}"),
        }
        assert_eq!(reg.kill(id), Some(true));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match reg.snapshot(id) {
                Some(TaskSnapshot::Done { result: Ok(out), .. }) => {
                    assert!(out.cancelled);
                    assert!(!out.timed_out);
                    break;
                }
                Some(TaskSnapshot::Running { .. }) => {
                    if std::time::Instant::now() > deadline {
                        panic!("задача не отменилась за 5 с");
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                other => panic!("неожиданный снимок: {other:?}"),
            }
        }
        // Повторный kill по завершённой задаче: Some(false).
        assert_eq!(reg.kill(id), Some(false));
        // Несуществующая задача: None.
        assert_eq!(reg.kill(999_999), None);
    }

    #[test]
    fn registry_spawn_completes() {
        let reg = Arc::new(TaskRegistry::default());
        let id = TaskRegistry::spawn(&reg, "echo poler-task".into(), spec("echo", &["poler-task"]));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match reg.snapshot(id) {
                Some(TaskSnapshot::Done { result: Ok(out), .. }) => {
                    assert_eq!(out.exit_code, Some(0));
                    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "poler-task");
                    break;
                }
                Some(TaskSnapshot::Running { .. }) => {
                    if std::time::Instant::now() > deadline {
                        panic!("echo не завершился за 5 с");
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                other => panic!("неожиданный снимок: {other:?}"),
            }
        }
        let listed = reg.list();
        assert!(listed.iter().any(|(lid, _)| *lid == id));
    }
}
