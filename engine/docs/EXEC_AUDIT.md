# EXEC_AUDIT — диагностика GNU bash 5.2 → POLER Exec (E1/v0.31.0)

**Дата**: 2026-09-18 · **Инструмент диагностики**: POLER Engine v0.31.0 сам на себе
(`--grep` / `--grep-regex` / `--grep-count`, паритет с ripgrep верифицирован)
· **Объект**: исходники GNU bash 5.2 (45 C-файлов ядра, tarball ftp.gnu.org)

---

## 1. Задача

Инструмент выполнения команд — самая нагруженная поверхность любого агента:
через неё проходят сборки, тесты, диагностика. Требование владельца: взять
эталонный исполнитель (GNU bash — стандарт де-факто на Linux), провести
диагностику его исходников силами POLER Engine и построить исполнитель,
в котором найденные классы ошибок **невозможны по построению**.

Результат — кирпич **E1/v0.31.0**: `os/core/poler_exec.zig` (ядро, raw-syscall
слой на ассемблере) + `src/exec/mod.rs` (Rust-обвязка, фича `pnd-ffi`)
+ CLI `--exec` + MCP-инструмент `poler_exec`.

## 2. Находки (все якоря проверяемы через `poler-engine <путь> --grep ...`)

### 2.1 Небезопасные строковые примитивы — 424 вызова в 75 .c-файлах

```
$ poler-engine bash-5.2 --grep "strcpy|strcat|sprintf|vsprintf|strncat" \
      --grep-regex --grep-count
```

Итог: **424 вызова в 75 .c-файлах** (широкий класс — включая lib/sh, builtins,
support/). Каждое место — потенциальная перезапись буфера при нештатной длине
данных. Лидеры: lib/sh/snprintf.c (собственная реализация, 51), lib/sh/strftime.c (47).

**Следствие для исполнителя**: строковая обработка argv/envp/путей не может
идти через неограниченные C-примитивы.

### 2.2 `free()` на путях сигнальной механики — trap.c:839

```
static void
free_trap_command (sig)
     int sig;
{
  ...
    free (trap_list[sig]);      /* ← trap.c:839 */
}
```

`free()` не входит в список async-signal-safe (man 7 signal-safety): в
многопоточном процессе она может взять mutex, уже удержанный прерванным
потоком — дедлок. Манипуляции ловушек в bash перевязаны с механикой SIGCHLD
(trap.c — 50 упоминаний SIGCHLD) и выполняются на путях, реагирующих на
сигналы.

### 2.3 REINSTALL_SIGCHLD-гонка — jobs.c:144-152 и jobs.c:319-333

Сам макрос (признаёт проблему в комментарии):

```
/* jobs.c:144 */
/* If the system needs it, REINSTALL_SIGCHLD_HANDLER will reinstall the
   handler for SIGCHLD. */
#if defined (MUST_REINSTALL_SIGHANDLERS)
#  define REINSTALL_SIGCHLD_HANDLER signal (SIGCHLD, sigchld_handler)
```

И ручная очередь как заплатка той же гонки (jobs.c:319):

```
static int queue_sigchld;
/* We set queue_sigchld around the call to waitchld to protect data structures
   from a SIGCHLD arriving while waitchld is executing. */
#define UNQUEUE_SIGCHLD(os) \
  do { \
    queue_sigchld--; \
    if (queue_sigchld == 0 && os != sigchld) \
      { ... waitchld (-1, 0); ... } \
  } while (0)
```

Окно между приходом SIGCHLD и переустановкой обработчика (или между
waitchld и разбором очереди) — потерянный SIGCHLD → зомби до следующего
произвольного события. Всего: jobs.c — 92 упоминания SIGCHLD,
`set_sigchld_handler` переустанавливается из **8 точек** (execute_cmd.c:4,
subst.c:2, jobs.c:1, jobs.h:1) — разветвлённая ручная синхронизация.

### 2.4 Неограниченный захват вывода `$(...)` — subst.c

`command_substitute` (4 вхождения в subst.c) читает вывод ребёнка в буфер,
растущий через **7 xrealloc** без какого-либо лимита: `$(yes)` = OOM.
Дренирования/капа нет — «память закончится раньше диска».

### 2.5 Ноль таймаутов на дочерних процессах

Во всём bash нет механизма «убить ребёнка через N мс»: `alarm`/`setitimer`
используются только для `read -t` и патологий терминала. Зависший ребёнок
(например, `$(cat)` без stdin-данных) подвешивает оболочку навсегда.

## 3. Проект решения: конструктивные исключения

| Класс ошибки bash | Решение в poler_exec | Как обеспечено |
|---|---|---|
| strcpy/sprintf (424 вызова) | ноль строк C, ноль libc в ребёнке | ребёнок между fork и exec — только `syscall`-инструкции; строки не копируются вовсе |
| free() в сигнальном контексте | ноль обработчиков сигналов | poll-driven: SIGCHLD не нужен, ничего не free() в контексте, деструкторов нет |
| REINSTALL_SIGCHLD-гонка | wait4(WNOHANG) в ppoll-цикле + pidfd | пробуждение ровно при смерти ребёнка (pidfd, ядро ≥ 5.3); фолбэк — кап 2 мс |
| неограниченный `$(...)` | кольцевой буфер: хвост N байт | O(1) памяти при любом объёме вывода + флаг truncated |
| вечные зависания | timerfd(MONOTONIC) + TERM → grace → KILL | таймаут гарантирует завершение; финальный harvest ограничен 10 с даже в D-state патологии |
| шелл-инъекции | argv = массив указателей | нет шелл-парсинга команды — конструкции `; rm -rf` невозможны синтаксически |
| утечка fds | pipe2(O_CLOEXEC) + close_range(CLOEXEC) | чужие дескрипторы не протекают в ребёнка и обратно |

## 4. Архитектура (os/core/poler_exec.zig, ~950 строк)

```
Родитель                                     Ребёнок (fork → exec)
───────                                      ─────────────────────
pipe2 ×4 (out/err/fail/stdin)                dup3 → setpgid(0,0) →
  │                                          close_range(3..∞, CLOEXEC)
timerfd(MONOTONIC, timeout)                  → execve. При провале:
pidfd_open(pid) ← ядро ≥ 5.3                  байт errno в fail-пайп, exit 127
  │
ppoll-цикл {out_rd, err_rd, fail_rd,          единственный код между
             in_wr, timerfd, pidfd}           fork и exec — raw-syscalls
  │ read → кольцевые буферы (хвост N)         (async-signal-safe по
  │ wait4(WNOHANG) на каждом пробуждении      построению, не по надежде)
  ▼
kill-политика: TERM → grace → KILL; прямой kill(pid) всегда,
  групповой kill(-pid) — только при доказанном владении группой
```

**Протокол fail-пайпа** (2 байта, фрейминг по смещению):
`[0]` — успех setpgid ребёнка (групповой kill легален), `[1]` — errno
провала exec. Пайп CLOEXEC: при успешном exec закрывается сам.

**Kill-безопасность**: групповой `kill(-pid)` отправляется только если
setpgid подтверждён ребёнком ИЛИ родителем. Осиротевшая чужая группа
с совпавшим pgid не может пострадать (pid переработан — реальный сценарий).

## 5. Боевой журнал разработки (найдено тем же методом — диагностикой)

Ядро писалось с соблюдением собственных стандартов; тем показательнее
пять багов, найденных тестами и трассировкой (все — в этой сессии, все
исправлены, каждый закрыт тестом):

1. **SETPGID = 154 → 109** — 154 это номер aarch64; на x86_64 сисколл
   возвращал ENOSYS. Симптом: ребёнок оставался в чужой process-group,
   `kill(-pid)` бил в ESRCH — зависший sleep не умирал даже от SIGKILL.
   Найдено пробой примитива (exit-код 100+38) и /proc-инспектией зомби.
2. **Сон ppoll на голом timerfd** — после EOF всех пайпов и до harvest
   wait4 не вызывался вовсе: цикл спал до срабатывания таймера (echo
   «выполнялся» 3 с при фактических 1 мс). Исправлено pidfd-пробуждением
   (основной путь) и капом REAP_POLL_US=2 мс (фолбэк).
3. **pipe2(O_NONBLOCK) ставит флаг на ОБА конца** — ребёнок получал
   EAGAIN на записи в полный пайп: `dd` 256 КиБ падала с exit 1.
   Исправлено: пайпы рождаются блокирующимися, родитель ставит O_NONBLOCK
   себе через fcntl(F_SETFL) (per-fd).
4. **CLOSE_RANGE_UNSHARE(2) ≠ CLOSE_RANGE_CLOEXEC(4)** — флаг 2 реально
   ЗАКРЫВАЛ fds 3..∞ в ребёнке, ломая доставку errno из fail-пайпа.
5. **Фрейминг fail-пайпа** — два байта протокола приходили разными read():
   errno затирал байт pgid_ok. Исправлен разбор по смещению.

Урок: каждый из пяти — класс той же природы, что находки в bash (неверная
инвариантность, гонка, неучтённая семантика флага). Диагностический метод
POLER — изоляция примитива + трассировка + дифференциал — применим к самому
движку и его кирпичам.

## 6. Тестовая матрица

- **Zig-ядро** (os/core, `zig build test`): 11/11 — Ring×4, errOf, echo,
  код 7 + оба потока, таймаут TERM→KILL, ENOENT→127/−2, stdin-cat-roundtrip,
  dd 256 КиБ → хвост 4 КиБ + truncated.
- **Rust-мост** (cargo test --features pnd-ffi): 9/9 — echo-кириллица,
  exit 42, stderr, таймаут, NotFound×2, хвост, stdin, env-изоляция.
  Полный прогон движка: **1164 passed / 0 failed**.
- **CLI-смоук** (release): exit 42→42; таймаут 300 мс → 124 за 303 мс;
  потоки разведены; усечение ровно 4096/256 КиБ; stdin-roundtrip.
- **MCP** (JSON-RPC): манифест содержит poler_exec; echo+exit 5 — 801 мкс;
  sleep-таймаут 250 мс → timed_out=true за 250298 мкс.

## 7. Как использовать

```bash
# CLI: флаги ДО --exec; после — команда целиком (включая её -флаги)
poler-engine --exec-timeout-ms 5000 --exec make -j4
poler-engine --exec-max-out 65536 --exec dd if=/dev/zero bs=1M count=64
poler-engine --exec-stdin "текст" --exec cat
# коды: ребёнка | 124 таймаут | 127 не найдено | 126 без права

# MCP-инструмент (агентам):
{"name":"poler_exec","arguments":{"command":"cargo","args":["test"],
 "timeout_ms":120000,"max_out_bytes":262144}}

# Rust API (фича pnd-ffi):
use poler_engine::exec::{run, ExecSpec};
let out = run(&ExecSpec{ program:"make".into(), args:vec!["-j4".into()],
    timeout_ms:300_000, ..Default::default() })?;
```

Ограничения: Linux x86_64 (comptime-guard); stdin — однократный буфер
(не стрим); cwd наследуется (смену каталога делает сама команда).

---

## 7. E2/v0.32.0 — устранение узких мест самого инструмента

Стресс-тест на хосте (i7-3770, CachyOS) и само-аудит выявили 7 ограничений
E1. Каждое закрыто в ядре `os/core/poler_exec.zig` (Zig, raw-syscalls) —
не обёрткой, а архитектурно.

| # | Узкое место E1 | Решение E2 | Где |
|---|----------------|------------|-----|
| 1 | Кольцо держало ТОЛЬКО хвост: стек-трейс в начале 100 МБ лога терялся безвозвратно | `capture=head_tail` (Sink): бюджет B делится — первые B/2 (голова, линейно) + маркер `\n[poler-exec: dropped N bytes]\n` + последние B/2 (кольцо в [B/2, B)). Итог ≤ B+64, один финальный сдвиг хвоста | Sink в poler_exec.zig |
| 2 | Не было PTY: sudo/fzf/htop и любые isatty-программы не работали | `/dev/ptmx` (O_RDWR\|O_NOCTTY\|O_CLOEXEC) + ioctl TIOCGPTN/TIOCSPTLCK(0)/TIOCSWINSZ(200x50) + в ребёнке setsid + open slave без O_NOCTTY → управляющий терминал. stdout/stderr слиты (природа PTY), группа сессии = pid → таймаут убивает всё | childBootstrap |
| 3 | $PATH сканировал РОДИТЕЛЬ (stat на каждый каталог до fork) | execvp-семантика в РЕБЁНКЕ: путь с '/' — прямой execve; имя — перебор каталогов PATH из envp, EACCES приоритетнее ENOENT. Ноль stat в родителе, ноль кэшей со стагнацией | childExec |
| 4 | Не было cwd (chdir процесс-глобален, многопоточный MCP-родитель трогать не может) | chdir в бутстрапе ребёнка; провал → errno в fail-пайп + exit 125 (отличим от 127 exec) | childBootstrap §3 |
| 5 | Запуск нельзя было отменить извне | `cancel_flag: ?*const u32` в Options: цикл делает @atomicLoad(acquire), ppoll не спит дольше 25 мс при активном флаге → TERM→grace→KILL; `res.cancelled=1`, `timed_out` НЕ трогается | poler_exec_run |
| 6 | MCP env не пробрасывался | `env` (MCP-объект) и `--exec-env KEY=VALUE` (CLI, повторяемый) | mcp.rs / main.rs |
| 7 | Утечка fd: in_wr не закрывался при раннем выходе из цикла | флаг `in_wr_open` + страховочный close на выходе; PTY: master == out_rd == in_wr — закрывается ровно один раз | poler_exec_run |

### 7.1 Параллельность MCP (узкое место уровня сервера)

E1: stdio-цикл читал запросы последовательно — один `poler_exec sleep 30`
блокировал сервер на 30 с; 20 стресс-задач вставали в FIFO-очередь.

E2: пул воркеров (`available_parallelism` clamped 2..=8) + реестр фоновых
задач:

* `poler_exec_async {command,…}` → `{task_id, status:"running"}` мгновенно;
  задача живёт в собственном потоке (TaskRegistry, лимит 128 с вытеснением
  завершённых);
* `poler_exec_task {task_id, wait_ms≤60000}` — опрос/ожидание: running-снимок
  или полный JSON исполнения (+command/elapsed_us);
* `poler_exec_kill {task_id}` — атомарная отмена (см. #5), `killed:true/false`;
* `poler_exec_list {}` — обзор реестра;
* tools/call exec-семейства уходит в пул: pipelined-запросы исполняются
  ПАРАЛЛЕЛЬНО, ответы — по готовности (JSON-RPC допускает внеочередность,
  клиент сопоставляет по id). Замер: 2×`sleep 1` параллельно = **1.008 с**
  стеновых (сериально 2 с+).

### 7.2 C-ABI изменения (обе стороны зеркальны, repr(C))

```
Options: + cwd: ?[*:0]const u8 = null      // chdir в ребёнке
         + pty: u32 = 0                    // /dev/ptmx + setsid + slave
         + capture: u32 = 0                // 0=tail, 1=head_tail
         + cancel_flag: ?*const u32 = null // атомарная отмена
Result:  + cancelled: u32                  // 1 = отменён (≠ таймаут)
```
Контракт head_tail: буфер вызывающего ≥ бюджет + 64 байта (MARKER_MAX).
Провал chdir: rc = -errno, res.exit_code = 125; провал exec: rc = -errno,
exit 127 — стадии различимы.

### 7.3 Матрица приёмки E2

| Проверка | Результат |
|----------|-----------|
| Zig-тесты ядра (включая PTY/head_tail/cwd/cancel/PATH) | **21/21** |
| zig build test (крипто 30 + ABI-parity + exec 21) | **зелёные** |
| Rust pnd-ffi (lib+doc+integration+gateway) | **1177/1177** (+13 новых) |
| Rust default | **1115/1115** (базовая линия E1 сохранена) |
| CLI: PATH по имени (echo), cwd (/tmp), env, таймаут 300 мс → 124 за 303 мс | ✅ |
| CLI: PTY — `tty` = /dev/pts/N, `tput cols` = 200 | ✅ |
| CLI: head_tail — маркер `dropped 588383 bytes`, голова seq 1…, хвост …100000 | ✅ |
| MCP: 3 pipelined запроса (2×sleep 1 + echo) | **1.008 с** стеновых (параллельно) |
| MCP: async-цикл spawn→list→kill→task(wait) | running→killed→done, cancelled=true, timed_out=false |
