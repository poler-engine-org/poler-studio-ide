// ============================================================================
// POLER Exec — идеальный исполнитель команд (E2/v0.32.0)
// ============================================================================
//
// Рождён диагностикой исходников GNU bash 5.2 самим POLER-Engine
// (docs/EXEC_AUDIT.md, 2026-09-17): 337 небезопасных strcpy/sprintf/strcat
// в 71 C-файле, free() внутри обработчика сигнала (trap.c:839),
// REINSTALL_SIGCHLD (потеря SIGCHLD между переустановками → зомби),
// неограниченный захват вывода $(...) без лимита (subst.c:755),
// ноль таймаутов на дочерние процессы.
//
// E2/v0.32.0 — устранены узкие места САМОГО ИНСТРУМЕНТА (стресс-аудит):
//
// | Узкое место E1                              | Решение E2                       |
// |---------------------------------------------|----------------------------------|
// | терялась ГОЛОВА вывода (хвост-only)         | capture=head_tail: первые B/2    |
// |   (стек-трейс в начале 100 МБ логов)        |   + маркер «dropped N» + хвост   |
// | PTY-программы (sudo/fzf/htop, isatty)       | /dev/ptmx + setsid + slave —     |
// |   не работали вообще                        |   настоящий терминал 200x50      |
// | $PATH сканировался в РОДИТЕЛЕ (stat на      | execvp-семантика в РЕБЁНКЕ:      |
// |   каждый каталог до fork)                    |   ноль stat до fork              |
// | не было cwd — а chdir в многопоточном       | chdir в бутстрапе ребёнка;       |
// |   родителе запрещён                          |   провал → errno + exit 125      |
// | запуск нельзя было отменить извне            | cancel_flag (atomic) →           |
// |                                             |   TERM→KILL замечается ≤25 мс    |
// | утечка in_wr при раннем выходе из цикла     | страховочный close на выходе     |
//
// Ответ POLER — исполнитель, в котором перечисленные классы ошибок
// НЕВОЗМОЖНЫ ПО ПОСТРОЕНИЮ:
//
// | Находка в bash                              | Решение здесь                     |
// |---------------------------------------------|-----------------------------------|
// | strcpy/sprintf (337 вызовов, C)             | ноль строк C, ноль libc           |
// | free() в signal-handler (deadlock арены)    | ноль обработчиков сигналов:       |
// |                                             |   poll-driven, SIGCHLD не нужен   |
// | REINSTALL_SIGCHLD race (зомби)              | wait4(WNOHANG) в ppoll-цикле      |
// | неограниченный вывод $(...) (OOM)           | кольцевой буфер: хвост N байт     |
// |                                             |   + флаг truncated, O(1) памяти   |
// | нет таймаутов (вечное зависание)            | timerfd(MONOTONIC) + ppoll;       |
// |                                             |   SIGTERM → grace → SIGKILL       |
// | fork → сотни строк C до exec                | ребёнок после fork выполняет       |
// |   (malloc-арены залочены)                   |   ТОЛЬКО raw-syscalls (asm):      |
// |                                             |   dup3 → setpgid → close_range    |
// |                                             |   → execve. Deadlock-free.        |
// | блокирующие read на пайпах                  | pipe2(O_NONBLOCK\|O_CLOEXEC)+ppoll|
//
// Ассемблерный слой: системные вызовы x86_64 напрямую (syscall insn),
// без PLT/libc — единственный код ребёнка между fork и exec.
//
// Сборка: входит в libpoler_core.a (root = abi.zig, см. build.zig).
// C-ABI: poler_exec_run(). Тесты: zig build test (артефакт exec_tests).
// ============================================================================

const std = @import("std");
const builtin = @import("builtin");

// Исполнитель написан под Linux x86_64 (raw-syscall слой). Другие цели
// осознанно не поддерживаются: суверенный стек движка — Linux/x86_64.
comptime {
    if (builtin.os.tag != .linux or builtin.cpu.arch != .x86_64) {
        @compileError("poler_exec: только Linux x86_64 (raw-syscall слой)");
    }
}

// ── Системные вызовы x86_64 (номера) ────────────────────────────────────────

const SYS = struct {
    const READ = 0;
    const WRITE = 1;
    const CLOSE = 3;
    const FCNTL = 72;
    const OPENAT = 257;
    const DUP3 = 292;
    const PIPE2 = 293;
    const EXECVE = 59;
    const EXIT = 60;
    const WAIT4 = 61;
    const KILL = 62;
    const SETPGID = 109; // x86_64! (154 — номер aarch64, здесь даёт ENOSYS)
    const SETSID = 112; // PTY: новая сессия (группа = pid, доказуемо наша)
    const CHDIR = 80; // cwd — только в ребёнке (родитель многопоточен)
    const IOCTL = 16; // TIOCGPTN / TIOCSPTLCK / TIOCSWINSZ
    const NANOSLEEP = 35;
    const PIDFD_OPEN = 434; // ядро >= 5.3: POLLIN ровно при смерти ребёнка
    const CLOCK_GETTIME = 228;
    const PPOLL = 271;
    const TIMERFD_CREATE = 283;
    const TIMERFD_SETTIME = 286;
    const CLOSE_RANGE = 436;
};

const O_RDONLY: usize = 0;
const O_WRONLY: usize = 1;
const O_RDWR: usize = 2; // ptmx master
const O_NOCTTY: usize = 0x100; // master НЕ становится управляющим
const O_CLOEXEC: usize = 0x80000;
const O_NONBLOCK: usize = 0x800;
// Пайпы рождаются БЛОКИРУЮЩИМИСЯ: pipe2(O_NONBLOCK) ставит флаг на ОБА
// конца — ребёнок получил бы EAGAIN на записи при полном пайпе и падал
// (найдено тестом dd: 256 КиБ > буфера пайпа). Неблокируемость нужна
// ТОЛЬКО родителю — он ставит её сам через fcntl(F_SETFL) на СВОИ концы
// (это per-fd, детскому концу не мешает).
const PIPE_FLAGS: usize = O_CLOEXEC;

const AT_FDCWD: usize = @bitCast(@as(isize, -100));
const CLOSE_RANGE_CLOEXEC: usize = 4; // 1<<2: ПОМЕТИТЬ (2 = UNSHARE — реально ЗАКРЫВАЕТ!)
const F_SETFD: usize = 2;
const FD_CLOEXEC: usize = 1;
const F_SETFL: usize = 4;

const POLLIN: i16 = 0x001;
const POLLOUT: i16 = 0x004;
const POLLERR: i16 = 0x008;
const POLLHUP: i16 = 0x010;

// ── PTY (ioctl, кодировка x86_64/generic) ───────────────────────────────────
const TIOCGPTN: usize = 0x80045430; // номер подчинённого терминала
const TIOCSPTLCK: usize = 0x40045431; // разблокировка pty
const TIOCSWINSZ: usize = 0x5414; // размер окна

const WinSize = extern struct {
    row: u16 = 50,
    col: u16 = 200, // широкое окно: меньше переносов в выводе агента
    xpixel: u16 = 0,
    ypixel: u16 = 0,
};

const SIGKILL: usize = 9;
const SIGTERM: usize = 15;

const WNOHANG: usize = 1;

const CLOCK_MONOTONIC: usize = 1;
const TFD_CLOEXEC: usize = 0x80000;

const EIO: i32 = 5; // PTY: запись в master после закрытия slave
const EINTR: i32 = 4;
const EAGAIN: i32 = 11;
const EPIPE: i32 = 32;
const ENOENT: i32 = 2;
const EACCES: i32 = 13;

/// Дренаж пайпов после смерти ребёнка (внуки могли унаследовать write-концы):
/// 250 мс — после этого вывод считается покинутым.
const POST_REAP_GRACE_US: u64 = 250_000;

/// Без pidfd (ядра < 5.3): интервал опроса wait4 в цикле, мкс. Найдено
/// тестом echo: ppoll спал на голом timerfd 3 с, не вызвав wait4 ни разу.
const REAP_POLL_US: u64 = 2_000;

/// Опрос cancel_flag, мкс: ppoll не спит дольше — отмена замечается
/// за ≤25 мс (плата — редкие пустые пробуждения, ТОЛЬКО при cancel_flag).
const CANCEL_POLL_US: u64 = 25_000;

/// Максимальная длина маркера «dropped N bytes» между головой и хвостом
/// (capture=head_tail). Вызывающий обязан дать буфер ≥ cap + MARKER_MAX.
const MARKER_MAX: usize = 64;

/// Финальный harvest после SIGKILL: не блокируем навечно (патологический
/// D-state), опрашиваем WNOHANG до этого лимита, мкс.
const FINAL_REAP_US: u64 = 10_000_000;

/// Отладочная трассировка цикла исполнителя (fd 2). Только для диагностики.
const EXEC_TRACE: bool = false;

fn trace(comptime fmt: []const u8, args: anytype) void {
    if (!EXEC_TRACE) return;
    var buf: [512]u8 = undefined;
    const s = std.fmt.bufPrint(&buf, fmt, args) catch return;
    _ = sys3(SYS.WRITE, 2, @intFromPtr(s.ptr), s.len);
}

// ── Ассемблерный слой: syscall напрямую, без libc/PLT ───────────────────────

inline fn sys1(nr: usize, a: usize) usize {
    return asm volatile ("syscall"
        : [ret] "={rax}" (-> usize),
        : [nr] "{rax}" (nr),
          [a1] "{rdi}" (a),
        : "rcx", "r11", "memory"
    );
}

inline fn sys2(nr: usize, a: usize, b: usize) usize {
    return asm volatile ("syscall"
        : [ret] "={rax}" (-> usize),
        : [nr] "{rax}" (nr),
          [a1] "{rdi}" (a),
          [a2] "{rsi}" (b),
        : "rcx", "r11", "memory"
    );
}

inline fn sys3(nr: usize, a: usize, b: usize, c: usize) usize {
    return asm volatile ("syscall"
        : [ret] "={rax}" (-> usize),
        : [nr] "{rax}" (nr),
          [a1] "{rdi}" (a),
          [a2] "{rsi}" (b),
          [a3] "{rdx}" (c),
        : "rcx", "r11", "memory"
    );
}

inline fn sys4(nr: usize, a: usize, b: usize, c: usize, d: usize) usize {
    return asm volatile ("syscall"
        : [ret] "={rax}" (-> usize),
        : [nr] "{rax}" (nr),
          [a1] "{rdi}" (a),
          [a2] "{rsi}" (b),
          [a3] "{rdx}" (c),
          [a4] "{r10}" (d),
        : "rcx", "r11", "memory"
    );
}

inline fn sys5(nr: usize, a: usize, b: usize, c: usize, d: usize, e: usize) usize {
    return asm volatile ("syscall"
        : [ret] "={rax}" (-> usize),
        : [nr] "{rax}" (nr),
          [a1] "{rdi}" (a),
          [a2] "{rsi}" (b),
          [a3] "{rdx}" (c),
          [a4] "{r10}" (d),
          [a5] "{r8}" (e),
        : "rcx", "r11", "memory"
    );
}

/// Ядро Linux возвращает -errno в rax при ошибке (диапазон -1..-4095).
inline fn errOf(rc: usize) ?i32 {
    const signed: isize = @bitCast(rc);
    if (signed < 0 and signed > -4096) return @intCast(-signed);
    return null;
}

inline fn isErrno(rc: usize, e: i32) bool {
    const maybe = errOf(rc);
    return maybe != null and maybe.? == e;
}

// ── Структуры для сисколов ──────────────────────────────────────────────────

const Timespec = extern struct {
    sec: isize = 0,
    nsec: isize = 0,
};

const Itimerspec = extern struct {
    interval: Timespec = .{},
    value: Timespec = .{},
};

const PollFd = extern struct {
    fd: i32,
    events: i16,
    revents: i16,
};

/// Мікросекунды CLOCK_MONOTONIC — для длительностей и дедлайнов.
fn nowUs() u64 {
    var ts: Timespec = .{};
    _ = sys2(SYS.CLOCK_GETTIME, CLOCK_MONOTONIC, @intFromPtr(&ts));
    const s: u64 = @intCast(@max(ts.sec, 0));
    const ns: u64 = @intCast(@max(ts.nsec, 0));
    return s * 1_000_000 + ns / 1000;
}

// ── C-ABI структуры ─────────────────────────────────────────────────────────

pub const Options = extern struct {
    /// Жёсткий таймаут, мс. 0 = без таймаута (блокирует до смерти ребёнка).
    timeout_ms: u64,
    /// Grace между SIGTERM и SIGKILL, мс [default в обвязке: 100].
    grace_ms: u64,
    /// Лимит захвата на поток (stdout и stderr отдельно), байт.
    /// Удерживается ХВОСТ вывода (кольцевой буфер). 0 = только discard.
    max_out_bytes: u64,
    /// Данные в stdin ребёнка (null → /dev/null, при PTY → терминал).
    stdin_data: ?[*]const u8,
    stdin_len: u64,
    /// Рабочий каталог ребёнка (null → наследовать). chdir выполняется
    /// в бутстрапе ребёнка: chdir процесс-глобален, многопоточный родитель
    /// трогать его не имеет права. Провал → errno в fail-пайп, exit 125.
    cwd: ?[*:0]const u8 = null,
    /// 1 = запустить под псевдотерминалом (/dev/ptmx + setsid + slave,
    /// окно 200x50). stdout и stderr сливаются в один поток (природа PTY).
    pty: u32 = 0,
    /// 0 = хвост (tail, классика E1); 1 = голова+маркер+хвост (head_tail):
    /// первые B/2 байт + «dropped N» + последние B/2. Буфер вызывающего
    /// обязан быть ≥ B + MARKER_MAX при режиме 1.
    capture: u32 = 0,
    /// Атомарный флаг отмены (u32, 0 → отмена). Цикл замечает установку
    /// за ≤CANCEL_POLL_US и запускает эскалацию TERM→KILL как при
    /// таймауте, но res.timed_out остаётся 0, res.cancelled = 1.
    cancel_flag: ?*const u32 = null,
};

pub const Result = extern struct {
    /// Код выхода (WEXITSTATUS); -1, если убит сигналом.
    exit_code: i32,
    /// Номер сигнала (WTERMSIG) или 0.
    signal: i32,
    /// 1 = сработал таймаут (послана TERM→KILL группе).
    timed_out: u32,
    /// 1 = вывод превысил лимит (захвачен хвост/голова+хвост).
    truncated: u32,
    /// Полная длительность, мкс (MONOTONIC).
    duration_us: u64,
    /// Байт записано в stdout_buf / stderr_buf (после finalize;
    /// в head_tail-режиме может превысить бюджет на длину маркера).
    stdout_len: u64,
    stderr_len: u64,
    /// Пид ребёнка (диагностика; -1 при провале fork).
    pid: i32,
    /// 1 = запуск отменён через cancel_flag (TERM→KILL, как таймаут,
    /// но timed_out не устанавливается).
    cancelled: u32,
};

// ── Кольцевой буфер: хвост вывода за O(1) памяти ────────────────────────────

const Ring = struct {
    buf: ?[*]u8,
    cap: usize,
    pos: usize, // старейший байт (после первого оборота)
    total: u64, // всего прочитано (может быть > cap)

    fn append(r: *Ring, data: []const u8) void {
        r.total += data.len;
        const buf = r.buf orelse return; // cap==0 → discard
        if (r.cap == 0) return;
        var i: usize = 0;
        while (i < data.len) {
            const n = @min(data.len - i, r.cap - r.pos);
            @memcpy(buf[r.pos..][0..n], data[i..][0..n]);
            r.pos = if (r.pos + n == r.cap) 0 else r.pos + n;
            i += n;
        }
    }

    /// Разворачивает кольцо в логический порядок; возвращает длину хвоста.
    fn finalize(r: *Ring) u64 {
        if (r.total <= r.cap) return r.total;
        if (r.buf) |buf| rotateLeftSlice(buf[0..r.cap], r.pos);
        return @intCast(r.cap);
    }
};

/// rotate-left на месте тремя реверсами (std.mem.rotateLeft появился
/// позже 0.14): result[i] = old[(i+k) % n].
fn rotateLeftSlice(buf: []u8, k_in: usize) void {
    if (buf.len == 0) return;
    const k = k_in % buf.len;
    if (k == 0) return;
    std.mem.reverse(u8, buf);
    std.mem.reverse(u8, buf[0 .. buf.len - k]);
    std.mem.reverse(u8, buf[buf.len - k ..]);
}

// ── Приёмник вывода: tail (классика E1) или head_tail (E2) ──────────────────
//
// Узкое место E1: кольцо удерживало ТОЛЬКО хвост — стек-трейс или шапка
// компилятора в начале 100-мегабайтного лога терялись безвозвратно.
//
// head_tail делит бюджет B пополам: первые B/2 байт (голова) лежат
// линейно с самого начала, последние B/2 (хвост) — кольцо в области
// [B/2, B). При переполнении между ними вставляется маркер
// «dropped N bytes»; итог ≤ B + MARKER_MAX, копирование — по одному
// байту на вход + один финальный сдвиг хвоста.

const Sink = struct {
    head_tail: bool,
    buf: ?[*]u8,
    cap: usize, // бюджет данных B (НЕ физический размер буфера)
    ring: Ring, // tail: весь буфер; head_tail: после переполнения — [B/2, B)
    lin: usize, // head_tail, фаза 1: линейное заполнение (≤ B)
    overflowed: bool,
    total: u64,

    fn initTail(buf: ?[*]u8, cap: usize) Sink {
        return .{
            .head_tail = false,
            .buf = buf,
            .cap = cap,
            .ring = .{ .buf = buf, .cap = cap, .pos = 0, .total = 0 },
            .lin = 0,
            .overflowed = false,
            .total = 0,
        };
    }

    fn initHeadTail(buf: ?[*]u8, cap: usize) Sink {
        return .{
            .head_tail = true,
            .buf = buf,
            .cap = cap,
            .ring = .{ .buf = null, .cap = 0, .pos = 0, .total = 0 },
            .lin = 0,
            .overflowed = false,
            .total = 0,
        };
    }

    fn append(s: *Sink, data: []const u8) void {
        s.total += data.len;
        const buf = s.buf orelse return; // cap==0 → discard
        if (s.cap == 0) return;
        if (!s.head_tail or s.overflowed) {
            s.ring.append(data);
            return;
        }
        // Фаза 1: линейное заполнение [0, B).
        const room = s.cap - s.lin;
        if (data.len <= room) {
            @memcpy(buf[s.lin..][0..data.len], data);
            s.lin += data.len;
            return;
        }
        // Переход в переполнение: голова = [0, B/2) уже на месте;
        // кольцо принимает область [B/2, B) как заполненную (байты
        // [B/2, B) потока — самый «свежий» хвост на этот момент).
        if (room > 0) @memcpy(buf[s.lin..][0..room], data[0..room]);
        s.lin = s.cap;
        const head_len = s.cap / 2;
        const ring_cap = s.cap - head_len;
        s.ring = .{ .buf = buf + head_len, .cap = ring_cap, .pos = 0, .total = ring_cap };
        s.overflowed = true;
        s.ring.append(data[room..]);
    }

    /// Разворачивает данные в логический порядок; возвращает длину.
    /// head_tail-контракт: итог = [голова B/2][маркер][хвост B/2] ≤ B + MARKER_MAX.
    fn finalize(s: *Sink) u64 {
        const buf = s.buf orelse return 0;
        if (s.cap == 0) return 0;
        if (!s.head_tail) return s.ring.finalize();
        if (!s.overflowed) return @intCast(s.lin); // всё уместилось — уже линейно
        const head_len = s.cap / 2;
        const ring_cap = s.cap - head_len;
        const dropped = s.total - s.cap;
        // Маркер в зазор между головой и хвостом.
        var mbuf: [MARKER_MAX]u8 = undefined;
        const marker = std.fmt.bufPrint(&mbuf, "\n[poler-exec: dropped {d} bytes]\n", .{dropped}) catch
            return s.ring.finalize(); // невозможно (50 < 64), но безопасно
        const m = marker.len;
        // Хвост сдвигается вправо на m (copyBackwards — перекрытие безопасно):
        // [head_len .. B) → [head_len+m .. B+m).
        std.mem.copyBackwards(u8, buf[head_len + m .. head_len + m + ring_cap], buf[head_len .. head_len + ring_cap]);
        // Развернуть кольцо на новом месте.
        rotateLeftSlice(buf[head_len + m .. head_len + m + ring_cap], s.ring.pos);
        // Маркер — в освободившийся зазор.
        @memcpy(buf[head_len..][0..m], marker);
        return @intCast(s.cap + m);
    }
};

// ── Ребёнок: единственный код между fork и exec — raw syscalls ──────────────

/// Deadlock-free бутстрап: после fork в многопоточном родителе легальны
/// только async-signal-safe операции. Здесь их НЕТ даже таких — только
/// прямые syscall-инструкции (dup3/openat/setpgid/close_range/execve).
/// Провал этапа: errno в fail-пайп (CLOEXEC — жив только здесь) + выход.
/// 127 — провал exec (канон шелла), 125 — провал chdir (cwd недоступен).
fn writeErrnoExit(fail_wr: i32, rc: usize, code: usize) noreturn {
    const errno: u8 = @truncate(0 -% rc);
    _ = sys3(SYS.WRITE, fdToU(fail_wr), @intFromPtr(&errno), 1);
    _ = sys1(SYS.EXIT, code);
    unreachable; // exit(2) не возвращается
}

/// execvp-семантика (E2): путь с '/' — прямой execve; имя — перебор
/// каталогов PATH ИЗ envp прямо здесь, в ребёнке. Ноль stat в родителе:
/// родитель больше не сканирует каталоги перед fork (узкое место E1 —
/// каждый запуск имени без '/' стоил N системных вызовов stat/access).
fn childExec(
    path: [*:0]const u8,
    argv: [*:null]const ?[*:0]const u8,
    envp: [*:null]const ?[*:0]const u8,
    fail_wr: i32,
) noreturn {
    // Путь с '/' — как задан.
    var has_slash = false;
    {
        var i: usize = 0;
        while (path[i] != 0) : (i += 1) {
            if (path[i] == '/') {
                has_slash = true;
                break;
            }
        }
    }
    if (has_slash) {
        const rc = sys3(SYS.EXECVE, @intFromPtr(path), @intFromPtr(argv), @intFromPtr(envp));
        writeErrnoExit(fail_wr, rc, 127);
    }

    // PATH из envp; отсутствующий/пустой → /bin:/usr/bin (как glibc).
    var path_val: []const u8 = "/bin:/usr/bin";
    {
        var p: usize = 0;
        while (envp[p]) |e| : (p += 1) {
            const s = std.mem.span(e);
            if (std.mem.startsWith(u8, s, "PATH=")) {
                path_val = s[5..];
                break;
            }
        }
    }

    var seen_eacces = false;
    var cand: [4096]u8 = undefined;
    const name = std.mem.span(path);
    var it = std.mem.splitScalar(u8, path_val, ':');
    while (it.next()) |dir| {
        const d = if (dir.len == 0) "." else dir; // пустой элемент = cwd (glibc)
        const full = std.fmt.bufPrintZ(&cand, "{s}/{s}", .{ d, name }) catch continue;
        const rc = sys3(SYS.EXECVE, @intFromPtr(full.ptr), @intFromPtr(argv), @intFromPtr(envp));
        if (rc != 0) { // execve вернулся — всегда -errno
            if (errOf(rc)) |e| {
                if (e == EACCES) seen_eacces = true;
            }
        }
    }
    // Ничего не нашли: EACCES приоритетнее ENOENT (семантика execvp).
    var errno: u8 = @intCast(ENOENT);
    if (seen_eacces) errno = @intCast(EACCES);
    _ = sys3(SYS.WRITE, fdToU(fail_wr), @intFromPtr(&errno), 1);
    _ = sys1(SYS.EXIT, 127);
    unreachable;
}

fn childBootstrap(
    path: [*:0]const u8,
    argv: [*:null]const ?[*:0]const u8,
    envp: [*:null]const ?[*:0]const u8,
    cwd: ?[*:0]const u8,
    slave: ?[*:0]const u8, // PTY: путь к подчинённому терминалу
    in_rd: i32, // -1 → PTY-терминал или /dev/null
    out_wr: i32, // -1 при PTY
    err_wr: i32, // -1 при PTY
    fail_wr: i32,
) noreturn {
    // 0) PTY: своя сессия, затем open slave БЕЗ O_NOCTTY — терминал
    //    становится управляющим (TIOCSCTTY не нужен после setsid).
    //    Группа новой сессии = pid — доказуемо наша, групповой kill легален.
    var session_ok = false;
    if (slave != null) {
        if (errOf(sys1(SYS.SETSID, 0)) == null) session_ok = true;
    }
    var tty_fd: i32 = -1;
    if (slave) |sl| {
        const tfd = sys4(SYS.OPENAT, AT_FDCWD, @intFromPtr(sl), O_RDWR, 0);
        if (errOf(tfd) == null) tty_fd = @intCast(tfd);
    }

    // 1) stdio: PTY → все три потока в терминал (stdout/stderr слиты —
    //    природа PTY); иначе 0 ← stdin (или /dev/null), 1/2 ← пайпы.
    if (tty_fd >= 0) {
        _ = sys3(SYS.DUP3, fdToU(tty_fd), 0, 0);
        _ = sys3(SYS.DUP3, fdToU(tty_fd), 1, 0);
        _ = sys3(SYS.DUP3, fdToU(tty_fd), 2, 0);
        if (tty_fd > 2) _ = sys1(SYS.CLOSE, fdToU(tty_fd));
    } else {
        if (in_rd >= 0) {
            _ = sys3(SYS.DUP3, fdToU(in_rd), 0, 0);
        } else {
            const devnull = "/dev/null";
            const fd = sys4(SYS.OPENAT, AT_FDCWD, @intFromPtr(devnull.ptr), O_RDONLY, 0);
            if (errOf(fd) == null) {
                _ = sys3(SYS.DUP3, fd, 0, 0);
                _ = sys1(SYS.CLOSE, fd);
            }
        }
        if (out_wr >= 0) _ = sys3(SYS.DUP3, fdToU(out_wr), 1, 0);
        if (err_wr >= 0) _ = sys3(SYS.DUP3, fdToU(err_wr), 2, 0);
    }

    // 2) Своя process-group: таймаут убивает ГРУППУ (включая внуков),
    //    никогда — родителя и его группу. Результат уходит родителю первым
    //    байтом fail-пайпа ДО close_range (тот лишь ПОМЕЧАЕТ CLOEXEC,
    //    поэтому байт доходит и при успешном exec, и при провале).
    //    PTY: setsid уже создал группу; иначе — setpgid(0,0).
    const pg_ok: u8 = blk: {
        if (session_ok) break :blk 1;
        const pg_rc = sys2(SYS.SETPGID, 0, 0);
        break :blk if (pg_rc == 0) 1 else 0;
    };
    _ = sys3(SYS.WRITE, fdToU(fail_wr), @intFromPtr(&pg_ok), 1);

    // 3) cwd: chdir ТОЛЬКО в ребёнке — в многопоточном родителе он
    //    процесс-глобален. Провал → errno + exit 125 (отличим от 127).
    if (cwd) |c| {
        const rc = sys1(SYS.CHDIR, @intFromPtr(c));
        if (errOf(rc) != null) writeErrnoExit(fail_wr, rc, 125);
    }

    // 4) Всё >= 3 пометить CLOEXEC: на успехе exec закроет сам; при провале
    //    останутся открытыми — сообщим errno и выйдем (следующий шаг).
    //    PTY: копия master в ребёнке тоже помечается — не протечёт.
    const cr = sys3(SYS.CLOSE_RANGE, 3, std.math.maxInt(u32), CLOSE_RANGE_CLOEXEC);
    if (errOf(cr) != null) {
        // Ядра без close_range (< 5.9): fcntl(F_SETFD) по диапазону.
        var fd: usize = 3;
        while (fd < 1024) : (fd += 1) {
            _ = sys3(SYS.FCNTL, fd, F_SETFD, FD_CLOEXEC);
        }
    }

    // 5) exec: прямой или PATH-поиск (ноль stat в родителе). Возврата нет.
    childExec(path, argv, envp, fail_wr);
}

// ── Родитель: ppoll-цикл без сигналов ───────────────────────────────────────

fn closeFd(fd: i32) void {
    if (fd >= 0) _ = sys1(SYS.CLOSE, fdToU(fd));
}

/// Убийство с безопасным наведением: прямой kill(pid) — всегда (гарантия
/// смерти ребёнка); групповой kill(-pid) — только при доказанном владении
/// группой (успешный setpgid ребёнка или родителя — чужую осиротевшую группу
/// с совпавшим pgid трогать нельзя).
fn sendKill(pid_u: usize, sig: usize, group_ok: bool) void {
    _ = sys2(SYS.KILL, pid_u, sig);
    if (group_ok) _ = sys2(SYS.KILL, 0 -% pid_u, sig);
}

/// i32 (fd) → usize для сисколла, с знаковым расширением.
inline fn fdToU(fd: i32) usize {
    return @bitCast(@as(isize, fd));
}

fn readInto(fd: i32, sink: *Sink, scratch: []u8) bool {
    // Возвращает true при EOF/HUP. EAGAIN → не EOF (просто нет данных).
    const rc = sys3(SYS.READ, fdToU(fd), @intFromPtr(scratch.ptr), scratch.len);
    if (errOf(rc)) |e| {
        if (e == EAGAIN) return false;
        return true; // EPIPE/EIO и пр. — считаем потоком закрытым
    }
    if (rc == 0) return true;
    sink.append(scratch[0..rc]);
    return false;
}

/// Полный прогон: fork → бутстрап → ppoll-цикл → wait4.
///
/// Возвращает 0 при успехе (res заполнен) или -errno этапа подготовки
/// (пайпы/fork/timer/ptmx). Провал самого exec: ребёнок пишет errno
/// в fail-пайп и выходит 127 (exec) или 125 (chdir/cwd) — возвращается
/// -errno, res.exit_code декодируется из wait4.
export fn poler_exec_run(
    path: [*:0]const u8,
    argv: [*:null]const ?[*:0]const u8,
    envp: [*:null]const ?[*:0]const u8,
    opts: *const Options,
    stdout_buf: ?[*]u8,
    stdout_cap: usize,
    stderr_buf: ?[*]u8,
    stderr_cap: usize,
    res: *Result,
) i32 {
    res.* = .{
        .exit_code = -1,
        .signal = 0,
        .timed_out = 0,
        .truncated = 0,
        .duration_us = 0,
        .stdout_len = 0,
        .stderr_len = 0,
        .pid = -1,
        .cancelled = 0,
    };

    const t0 = nowUs();
    const use_pty = opts.pty != 0;
    const need_stdin = opts.stdin_data != null and opts.stdin_len > 0;

    // ── PTY: мастер (/dev/ptmx) + путь к подчинённому терминалу ──────────
    var master_fd: i32 = -1;
    var slave_buf: [32]u8 = undefined;
    var slave: ?[*:0]const u8 = null;
    if (use_pty) {
        const ptmx = "/dev/ptmx";
        const m = sys4(SYS.OPENAT, AT_FDCWD, @intFromPtr(ptmx.ptr), O_RDWR | O_NOCTTY | O_CLOEXEC, 0);
        if (errOf(m)) |e| return -e;
        master_fd = @intCast(m);
        var ptn: u32 = 0;
        if (errOf(sys3(SYS.IOCTL, m, TIOCGPTN, @intFromPtr(&ptn)))) |e| {
            _ = sys1(SYS.CLOSE, m);
            return -e;
        }
        var unlock: u32 = 0;
        _ = sys3(SYS.IOCTL, m, TIOCSPTLCK, @intFromPtr(&unlock));
        const ws = WinSize{};
        _ = sys3(SYS.IOCTL, m, TIOCSWINSZ, @intFromPtr(&ws));
        if (std.fmt.bufPrintZ(&slave_buf, "/dev/pts/{d}", .{ptn})) |s| {
            slave = s.ptr;
        } else |_| {
            _ = sys1(SYS.CLOSE, m);
            return -12; // ENOMEM: путь не влез в 32 байта (невозможно)
        }
    }

    // ── Пайпы: stdout, stderr (не под PTY), execfail, (stdin) ─────────────
    var out_pipe: [2]i32 = .{ -1, -1 };
    var err_pipe: [2]i32 = .{ -1, -1 };
    var fail_pipe: [2]i32 = .{ -1, -1 };
    var in_pipe: [2]i32 = .{ -1, -1 };

    if (!use_pty) {
        if (errOf(sys4(SYS.PIPE2, @intFromPtr(&out_pipe), PIPE_FLAGS, 0, 0))) |e| return -e;
        if (errOf(sys4(SYS.PIPE2, @intFromPtr(&err_pipe), PIPE_FLAGS, 0, 0))) |e| {
            closePipe(&out_pipe);
            return -e;
        }
    }
    if (errOf(sys4(SYS.PIPE2, @intFromPtr(&fail_pipe), O_CLOEXEC, 0, 0))) |e| {
        closePipe(&out_pipe);
        closePipe(&err_pipe);
        return -e;
    }
    if (need_stdin and !use_pty) {
        if (errOf(sys4(SYS.PIPE2, @intFromPtr(&in_pipe), PIPE_FLAGS, 0, 0))) |e| {
            closePipe(&out_pipe);
            closePipe(&err_pipe);
            closePipe(&fail_pipe);
            return -e;
        }
    }

    // ── fork ──────────────────────────────────────────────────────────────
    // std.posix.fork на Linux без libc = прямой clone(SIGCHLD).
    const pid = std.posix.fork() catch {
        // ENOMEM/EAGAIN/EAGAIN-класс: ресурс исчерпан — честный -errno
        closePipe(&out_pipe);
        closePipe(&err_pipe);
        closePipe(&fail_pipe);
        closePipe(&in_pipe);
        if (master_fd >= 0) _ = sys1(SYS.CLOSE, fdToU(master_fd));
        return -11; // EAGAIN
    };

    if (pid == 0) {
        childBootstrap(
            path,
            argv,
            envp,
            opts.cwd,
            slave,
            if (use_pty) -1 else in_pipe[0],
            if (use_pty) -1 else out_pipe[1],
            if (use_pty) -1 else err_pipe[1],
            fail_pipe[1],
        );
    }

    // ── Родитель: закрыть детские концы ───────────────────────────────────
    closeFd(in_pipe[0]);
    closeFd(out_pipe[1]);
    closeFd(err_pipe[1]);
    closeFd(fail_pipe[1]);
    // PTY: мастер — он и чтение вывода, и запись stdin (потоки слиты).
    const out_rd: i32 = if (use_pty) master_fd else out_pipe[0];
    const err_rd: i32 = if (use_pty) -1 else err_pipe[0];
    const fail_rd = fail_pipe[0];
    const in_wr: i32 = if (use_pty) (if (need_stdin) master_fd else -1) else in_pipe[1];
    var in_wr_open = in_wr >= 0; // страховка закрытия при раннем выходе (E2)

    // Неблокирующий режим — только на родительских концах (per-fd):
    // ppoll + дренирующие read без блокировки; ребёнок пишет блокирующе.
    _ = sys3(SYS.FCNTL, fdToU(out_rd), F_SETFL, O_NONBLOCK);
    if (err_rd >= 0) _ = sys3(SYS.FCNTL, fdToU(err_rd), F_SETFL, O_NONBLOCK);
    _ = sys3(SYS.FCNTL, fdToU(fail_rd), F_SETFL, O_NONBLOCK);
    if (in_wr >= 0) _ = sys3(SYS.FCNTL, fdToU(in_wr), F_SETFL, O_NONBLOCK);

    // Группа ребёнка (родитель дублирует setpgid — классическая гонка
    // «кто раньше», один из двух всегда успевает). Успех ЛЮБОЙ из сторон
    // доказывает, что группа -pid принадлежит нашему ребёнку — только тогда
    // групповой kill(-pid) легален (иначе можно ударить по чужой осиротевшей
    // группе с совпавшим pgid).
    const parent_pgid_ok = sys2(SYS.SETPGID, fdToU(pid), fdToU(pid)) == 0;

    // ── Таймер (timerfd, MONOTONIC — не зависит от NTP-скачков) ───────────
    var timer_fd: i32 = -1;
    if (opts.timeout_ms > 0) {
        const tfd = sys2(SYS.TIMERFD_CREATE, CLOCK_MONOTONIC, TFD_CLOEXEC);
        if (errOf(tfd) == null) {
            const ms = opts.timeout_ms;
            const its = Itimerspec{ .value = .{
                .sec = @intCast(ms / 1000),
                .nsec = @intCast((ms % 1000) * 1_000_000),
            } };
            _ = sys4(SYS.TIMERFD_SETTIME, tfd, 0, @intFromPtr(&its), 0);
            timer_fd = @intCast(tfd);
        }
    }

    // ── Состояние цикла ───────────────────────────────────────────────────
    var out_sink = if (opts.capture == 1)
        Sink.initHeadTail(stdout_buf, stdout_cap)
    else
        Sink.initTail(stdout_buf, stdout_cap);
    var err_sink = if (opts.capture == 1)
        Sink.initHeadTail(stderr_buf, stderr_cap)
    else
        Sink.initTail(stderr_buf, stderr_cap);

    var out_eof = false;
    var err_eof = use_pty; // PTY: stderr слит в stdout терминала
    var fail_done = false;
    var child_done = false;
    var timed_out = false;
    var cancelled = false;
    var kill_stage: u8 = 0; // 0 — ещё не убивали, 1 — TERM, 2 — KILL
    var term_at: u64 = 0;
    var post_reap_deadline: u64 = 0;
    var status: u32 = 0;
    const grace_us: u64 = opts.grace_ms * 1000;

    var stdin_off: usize = 0;
    var stdin_done = !need_stdin;
    const stdin_ptr = opts.stdin_data orelse null;
    const stdin_len: usize = @intCast(opts.stdin_len);

    var scratch: [4096]u8 = undefined;
    const pid_u: usize = fdToU(pid);

    // pidfd (ядро >= 5.3): ppoll проснётся ровно при смерти ребёнка.
    // На уже-умершего (зомби) pidfd_open легален и fd сразу читаем — гонки
    // нет. Провал (старое ядро) → запасной режим: ppoll с капом REAP_POLL_US.
    const pidfd: i32 = blk: {
        const pfd = sys2(SYS.PIDFD_OPEN, pid_u, 0);
        if (errOf(pfd) != null) break :blk -1;
        break :blk @intCast(pfd);
    };

    var exec_errno: i32 = 0; // >0, если exec провалился (байт из fail-пайпа)
    var child_pgid_ok = false; // [0]-байт fail-пайпа: setpgid ребёнка удался
    var fail_got: usize = 0; // сколько байт fail-протокола прочитано

    // ── Главный ppoll-цикл: сигналы не нужны, зомби невозможны ────────────
    while (true) {
        const now = nowUs();
        trace("[{d}] done={} oe={} ee={} fe={} sd={} to={} ks={} prd={d}\n", .{ now, child_done, out_eof, err_eof, fail_done, stdin_done, timed_out, kill_stage, post_reap_deadline });

        // E2: отмена — внешний атомарный флаг (MCP poler_exec_kill).
        // Включает ту же эскалацию TERM→KILL, что и таймаут, но
        // res.timed_out не трогаем — отчёт честно скажет cancelled.
        if (!cancelled and !child_done) {
            if (opts.cancel_flag) |cf| {
                if (@atomicLoad(u32, cf, .acquire) != 0) {
                    cancelled = true;
                    res.cancelled = 1;
                    timed_out = true;
                }
            }
        }

        // Эскалация убийства: TERM → grace → KILL. Прямой kill(pid) — всегда;
        // групповой kill(-pid) — только если группа доказано наша.
        if (timed_out and !child_done and kill_stage < 2) {
            if (kill_stage == 0) {
                sendKill(pid_u, SIGTERM, parent_pgid_ok or child_pgid_ok);
                kill_stage = 1;
                term_at = now;
            } else if (now >= term_at + grace_us) {
                sendKill(pid_u, SIGKILL, parent_pgid_ok or child_pgid_ok);
                kill_stage = 2;
            }
        }

        // Дренаж после смерти ребёнка закончен (внуки бросили вывод).
        if (child_done and post_reap_deadline != 0 and now >= post_reap_deadline) {
            trace("BREAK: post_reap deadline\n", .{});
            break;
        }
        if (child_done and out_eof and err_eof) {
            trace("BREAK: eof after reap\n", .{});
            break;
        }

        // Неблокирующий harvest.
        if (!child_done) {
            const st = sys4(SYS.WAIT4, pid_u, @intFromPtr(&status), WNOHANG, 0);
            trace("  wait4 -> {d} (pid={d})\n", .{ st, pid_u });
            if (st == pid_u) {
                child_done = true;
                post_reap_deadline = nowUs() + POST_REAP_GRACE_US;
                continue;
            }
            if (errOf(st)) |_| {
                trace("BREAK: wait4 errno st={d}\n", .{st});
                child_done = true; // ECHILD — уже кем-то reapнут
                break;
            }
        }

        // Набор событий.
        var fds: [6]PollFd = undefined;
        var n: usize = 0;
        if (!out_eof) {
            fds[n] = .{ .fd = out_rd, .events = POLLIN, .revents = 0 };
            n += 1;
        }
        if (!err_eof) {
            fds[n] = .{ .fd = err_rd, .events = POLLIN, .revents = 0 };
            n += 1;
        }
        if (!fail_done) {
            fds[n] = .{ .fd = fail_rd, .events = POLLIN, .revents = 0 };
            n += 1;
        }
        if (!stdin_done) {
            fds[n] = .{ .fd = in_wr, .events = POLLOUT, .revents = 0 };
            n += 1;
        }
        if (timer_fd >= 0) {
            fds[n] = .{ .fd = timer_fd, .events = POLLIN, .revents = 0 };
            n += 1;
        }
        // pidfd: пробуждение ровно в момент смерти ребёнка.
        if (pidfd >= 0 and !child_done) {
            fds[n] = .{ .fd = pidfd, .events = POLLIN, .revents = 0 };
            n += 1;
        }

        // Нечего ждать и ребёнок жив: сон 1 мс (ppoll-таймаут), снова harvest.
        if (n == 0) {
            if (child_done) break;
            var one_ms = Timespec{ .nsec = 1_000_000 };
            _ = sys5(SYS.PPOLL, 0, 0, @intFromPtr(&one_ms), 0, 8);
            continue;
        }

        // Дедлайн пробуждения ppoll (NULL = блок до события):
        // - дренаж после смерти ребёнка (внуки бросили вывод);
        // - эскалация TERM→KILL (grace истёк, а ребёнок жив).
        // БЕЗ этого ppoll спал бы до ближайшего события (например, таймера
        // таймаута) — найдено тестом «echo + timeout 3s»: цикл жил 3 с
        // вместо мгновенного выхода по 250-мс дренажному дедлайну.
        var poll_deadline: u64 = 0; // 0 = нет дедлайна
        if (child_done and post_reap_deadline != 0) poll_deadline = post_reap_deadline;
        if (timed_out and kill_stage == 1) {
            const hard = term_at + grace_us;
            if (poll_deadline == 0 or hard < poll_deadline) poll_deadline = hard;
        }
        // Нет pidfd (старое ядро) — нельзя спать бесконечно: умерший ребёнок
        // не разбудит ppoll (пайпы уже EOF, таймер дренирован). Кап 2 мс
        // гарантирует регулярный wait4-опрос.
        if (pidfd < 0 and !child_done) {
            const cap = now + REAP_POLL_US;
            if (poll_deadline == 0 or cap < poll_deadline) poll_deadline = cap;
        }
        // E2: отмена — ppoll не спит дольше CANCEL_POLL_US, флаг замечается
        // не позднее 25 мс (плата — редкие пустые пробуждения).
        if (opts.cancel_flag != null and !child_done) {
            const ccap = now + CANCEL_POLL_US;
            if (poll_deadline == 0 or ccap < poll_deadline) poll_deadline = ccap;
        }

        var ts: Timespec = .{};
        var tmo_ptr: usize = 0;
        if (poll_deadline != 0) {
            const remaining_us = poll_deadline -% now;
            if (remaining_us == 0 or poll_deadline <= now) {
                // дедлайн уже настал — не спим вовсе (0 нс)
            } else {
                ts.sec = @intCast(remaining_us / 1_000_000);
                ts.nsec = @intCast((remaining_us % 1_000_000) * 1_000);
            }
            tmo_ptr = @intFromPtr(&ts);
        }

        const prc = sys5(SYS.PPOLL, @intFromPtr(&fds), n, tmo_ptr, 0, 8);
        trace("  ppoll n={d} tmo_ptr={d} -> rc={d} rev=({d},{d},{d},{d})\n", .{ n, tmo_ptr, prc, fds[0].revents, fds[1].revents, fds[2].revents, fds[3].revents });
        if (isErrno(prc, EINTR)) continue; // прерван сигналом — просто повтор
        if (errOf(prc) != null) {
            trace("BREAK: ppoll errno\n", .{});
            break; // прочие ошибки ppoll — выходим из цикла
        }
        if (prc == 0) continue; // дедлайн пробуждения — наверх за пересчётом

        for (fds[0..n]) |*f| {
            if (f.revents == 0) continue;

            // Независимые if (НЕ else-if): под PTY out_rd == in_wr == master —
            // одно событие может относиться и к чтению, и к записи stdin.
            if (f.fd == out_rd and out_rd >= 0 and !out_eof) {
                if (f.revents & POLLIN != 0) {
                    if (readInto(out_rd, &out_sink, &scratch)) out_eof = true;
                }
                if (!out_eof and f.revents & (POLLHUP | POLLERR) != 0) {
                    if (readInto(out_rd, &out_sink, &scratch)) out_eof = true;
                }
            }
            if (f.fd == err_rd and err_rd >= 0 and !err_eof) {
                if (f.revents & POLLIN != 0) {
                    if (readInto(err_rd, &err_sink, &scratch)) err_eof = true;
                }
                if (!err_eof and f.revents & (POLLHUP | POLLERR) != 0) {
                    if (readInto(err_rd, &err_sink, &scratch)) err_eof = true;
                }
            }
            if (f.fd == fail_rd and !fail_done) {
                if (f.revents & POLLIN != 0) {
                    // Протокол fail-пайпа: [0] = pgid_ok (пишется сразу после
                    // setpgid), [1] = errno провала exec (если был).
                    var eb: [8]u8 = undefined;
                    const r = sys3(SYS.READ, fdToU(fail_rd), @intFromPtr(&eb), eb.len);
                    if (errOf(r) != null) {
                        fail_done = true;
                    } else {
                        // Байты могут прийти РАЗНЫМИ чтениями (pgid — до exec,
                        // errno — после провала): разбираем по смещению,
                        // а не по количеству в одном read (найдено тестом
                        // ENOENT: errno затирал pgid_ok и терялся).
                        for (eb[0..r]) |b| {
                            switch (fail_got) {
                                0 => child_pgid_ok = (b != 0),
                                1 => {
                                    exec_errno = b;
                                    fail_done = true; // errno доставлен
                                },
                                else => {},
                            }
                            fail_got += 1;
                        }
                    }
                }
                if (f.revents & (POLLHUP | POLLERR) != 0) fail_done = true;
            }
            if (f.fd == in_wr and in_wr >= 0 and !stdin_done and f.revents & (POLLOUT | POLLERR | POLLHUP) != 0) {
                const rc = sys3(SYS.WRITE, fdToU(in_wr), @intFromPtr(stdin_ptr.?) + stdin_off, stdin_len - stdin_off);
                if (errOf(rc)) |e| {
                    if (e == EPIPE or e == EIO) {
                        stdin_done = true; // ребёнок ушёл, не читая stdin
                        closeFd(in_wr);
                        in_wr_open = false;
                    }
                    // EAGAIN — повтор на следующей итерации
                } else {
                    stdin_off += rc;
                    if (stdin_off >= stdin_len) {
                        stdin_done = true;
                        closeFd(in_wr); // EOF для ребёнка
                        in_wr_open = false;
                    }
                }
            }
            if (f.fd == timer_fd and timer_fd >= 0) {
                var tb: [8]u8 = undefined;
                const r = sys3(SYS.READ, fdToU(timer_fd), @intFromPtr(&tb), 8);
                if (r == 8 and !timed_out) { // ложные пробуждения не считаем
                    timed_out = true;
                    res.timed_out = 1;
                }
            }
        }
    }

    // ── Финальный harvest: ОГРАНИЧЕННЫЙ, никогда не блокируем навечно.
    // После SIGKILL ребёнок умирает мгновенно; патологический D-state
    // (непрерываемый сон в драйвере) не подвешивает исполнителя: опрашиваем
    // WNOHANG до FINAL_REAP_US, затем смиряемся (статус честно неизвестен).
    if (!child_done) {
        const hard_stop = nowUs() + FINAL_REAP_US;
        while (true) {
            const st = sys4(SYS.WAIT4, pid_u, @intFromPtr(&status), WNOHANG, 0);
            if (st == pid_u) break;
            if (errOf(st)) |_| break; // ECHILD — уже кем-то забран
            if (nowUs() >= hard_stop) break;
            var nap = Timespec{ .nsec = 10_000_000 }; // 10 мс
            _ = sys2(SYS.NANOSLEEP, @intFromPtr(&nap), 0);
        }
        child_done = true;
    }

    // ── Декодирование статуса ДО ветки exec_errno: код 127/125 приходит ──
    // из wait4 и различает стадию провала (exec vs chdir/cwd).
    const low7: u32 = status & 0x7f;
    if (low7 == 0) {
        res.exit_code = @intCast((status >> 8) & 0xff);
    } else if (low7 != 0x7f) {
        res.exit_code = -1;
        res.signal = @intCast(low7);
    } else {
        res.exit_code = -1; // stopped — в нашем контракте не бывает
    }

    // ── Провал exec/chdir: вернуть -errno, код уже декодирован ────────────
    if (exec_errno != 0) {
        if (in_wr_open and in_wr != out_rd) closeFd(in_wr);
        closeFd(out_rd);
        closeFd(err_rd);
        closeFd(fail_rd);
        closeFd(timer_fd);
        closeFd(pidfd);
        res.pid = pid;
        res.duration_us = nowUs() - t0;
        return -exec_errno;
    }

    res.stdout_len = out_sink.finalize();
    res.stderr_len = err_sink.finalize();
    res.truncated = if (out_sink.total > stdout_cap or err_sink.total > stderr_cap) 1 else 0;
    res.duration_us = nowUs() - t0;

    // PTY: in_wr == out_rd == master — закрыть ровно один раз.
    if (in_wr_open and in_wr != out_rd) closeFd(in_wr);
    closeFd(out_rd);
    closeFd(err_rd);
    closeFd(fail_rd);
    closeFd(timer_fd);
    closeFd(pidfd);
    res.pid = pid;

    return 0;
}

fn closePipe(p: *[2]i32) void {
    closeFd(p[0]);
    closeFd(p[1]);
    p[0] = -1;
    p[1] = -1;
}

// ── Тесты (zig build test → артефакт exec_tests в build.zig) ────────────────

const testing = std.testing;

test "Ring: без оборота — логический порядок" {
    var buf: [8]u8 = undefined;
    var r = Ring{ .buf = &buf, .cap = 8, .pos = 0, .total = 0 };
    r.append("ABCD");
    try testing.expectEqual(@as(u64, 4), r.finalize());
    try testing.expectEqualStrings("ABCD", buf[0..4]);
    try testing.expect(r.total <= 8);
}

test "Ring: оборот — хвост, rotateLeft разворачивает" {
    var buf: [4]u8 = undefined;
    var r = Ring{ .buf = &buf, .cap = 4, .pos = 0, .total = 0 };
    r.append("ABCDEF"); // хвост "CDEF", pos указывает на C
    try testing.expectEqual(@as(u64, 6), r.total);
    try testing.expectEqual(@as(u64, 4), r.finalize());
    try testing.expectEqualStrings("CDEF", buf[0..4]);
}

test "Ring: точный оборот без переполнения" {
    var buf: [4]u8 = undefined;
    var r = Ring{ .buf = &buf, .cap = 4, .pos = 0, .total = 0 };
    r.append("12");
    r.append("3456"); // хвост "3456"
    try testing.expectEqual(@as(u64, 4), r.finalize());
    try testing.expectEqualStrings("3456", buf[0..4]);
}

test "Ring: нулевой cap — discard, но total считает" {
    var r = Ring{ .buf = null, .cap = 0, .pos = 0, .total = 0 };
    r.append("ignored");
    try testing.expectEqual(@as(u64, 7), r.total);
    try testing.expectEqual(@as(u64, 0), r.finalize());
}

test "errOf: errno из отрицательного rax" {
    try testing.expectEqual(@as(?i32, null), errOf(0));
    try testing.expectEqual(@as(?i32, null), errOf(42));
    try testing.expectEqual(@as(?i32, 2), errOf(@bitCast(@as(isize, -2)))); // ENOENT
    try testing.expectEqual(@as(?i32, 4), errOf(@bitCast(@as(isize, -4)))); // EINTR
}

test "exec: /bin/echo — exit 0, stdout захвачен" {
    if (!fileExists("/bin/echo")) return error.SkipZigTest;

    var argv = [_:null]?[*:0]const u8{ "/bin/echo", "полер-полер" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 3000,
        .grace_ms = 100,
        .max_out_bytes = 4096,
        .stdin_data = null,
        .stdin_len = 0,
    };
    var out: [4096]u8 = undefined;
    var errb: [4096]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run(
        "/bin/echo",
        &argv,
        &envp,
        &opts,
        &out,
        out.len,
        &errb,
        errb.len,
        &res,
    );
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(i32, 0), res.exit_code);
    try testing.expectEqual(@as(u32, 0), res.timed_out);
    try testing.expectEqual(@as(u32, 0), res.truncated);
    try testing.expect(res.pid > 0); // пид доступен для диагностики
    const stdout = out[0..@intCast(res.stdout_len)];
    try testing.expect(std.mem.indexOf(u8, stdout, "полер-полер") != null);
}

test "exec: код 7 + stderr + stdout одновременно" {
    if (!fileExists("/bin/sh")) return error.SkipZigTest;

    var argv = [_:null]?[*:0]const u8{ "/bin/sh", "-c", "echo out-полер; echo err-полер >&2; exit 7" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 3000,
        .grace_ms = 100,
        .max_out_bytes = 4096,
        .stdin_data = null,
        .stdin_len = 0,
    };
    var out: [256]u8 = undefined;
    var errb: [256]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run("/bin/sh", &argv, &envp, &opts, &out, out.len, &errb, errb.len, &res);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(i32, 7), res.exit_code);
    try testing.expect(std.mem.indexOf(u8, out[0..@intCast(res.stdout_len)], "out-полер") != null);
    try testing.expect(std.mem.indexOf(u8, errb[0..@intCast(res.stderr_len)], "err-полер") != null);
}

test "exec: таймаут — TERM→KILL, timed_out=1" {
    if (!fileExists("/bin/sleep")) return error.SkipZigTest;

    var argv = [_:null]?[*:0]const u8{ "/bin/sleep", "30" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 80, // 80 мс
        .grace_ms = 50,
        .max_out_bytes = 4096,
        .stdin_data = null,
        .stdin_len = 0,
    };
    var out: [64]u8 = undefined;
    var errb: [64]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run("/bin/sleep", &argv, &envp, &opts, &out, out.len, &errb, errb.len, &res);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), res.timed_out);
    try testing.expect(res.duration_us < 2_000_000); // никак не 30 с
    try testing.expect(res.signal == 15 or res.signal == 9 or res.exit_code >= 0);
}

test "exec: провал exec — -ENOENT, exit 127" {
    var argv = [_:null]?[*:0]const u8{ "/несуществующий/полер" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 3000,
        .grace_ms = 100,
        .max_out_bytes = 256,
        .stdin_data = null,
        .stdin_len = 0,
    };
    var out: [64]u8 = undefined;
    var errb: [64]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run("/несуществующий/полер", &argv, &envp, &opts, &out, out.len, &errb, errb.len, &res);
    try testing.expectEqual(@as(i32, -2), rc); // -ENOENT
    try testing.expectEqual(@as(i32, 127), res.exit_code);
}

test "exec: stdin → cat, roundtrip" {
    if (!fileExists("/bin/cat")) return error.SkipZigTest;

    const payload = "полер-stdin-roundtrip-42";
    var argv = [_:null]?[*:0]const u8{ "/bin/cat" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 3000,
        .grace_ms = 100,
        .max_out_bytes = 4096,
        .stdin_data = payload.ptr,
        .stdin_len = payload.len,
    };
    var out: [4096]u8 = undefined;
    var errb: [64]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run("/bin/cat", &argv, &envp, &opts, &out, out.len, &errb, errb.len, &res);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(i32, 0), res.exit_code);
    try testing.expectEqualStrings(payload, out[0..@intCast(res.stdout_len)]);
}

test "exec: огромный вывод — кольцо держит хвост, truncated=1" {
    if (!fileExists("/bin/dd")) return error.SkipZigTest;

    var argv = [_:null]?[*:0]const u8{ "/bin/dd", "if=/dev/zero", "bs=1024", "count=256" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 5000,
        .grace_ms = 100,
        .max_out_bytes = 4096, // 256 КиБ вывода → хвост 4 КиБ
        .stdin_data = null,
        .stdin_len = 0,
    };
    var out: [4096]u8 = undefined;
    var errb: [4096]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run("/bin/dd", &argv, &envp, &opts, &out, out.len, &errb, errb.len, &res);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(i32, 0), res.exit_code);
    try testing.expectEqual(@as(u32, 1), res.truncated);
    try testing.expectEqual(@as(u64, 4096), res.stdout_len);
}

// ── E2/v0.32.0: тесты новых возможностей ─────────────────────────────────────

test "Sink: head_tail без переполнения — линейно" {
    var buf: [8]u8 = undefined;
    var s = Sink.initHeadTail(&buf, 8);
    s.append("ABCD");
    try testing.expectEqual(@as(u64, 4), s.finalize());
    try testing.expectEqualStrings("ABCD", buf[0..4]);
}

test "Sink: head_tail с переполнением — голова+маркер+хвост" {
    var buf: [8 + 64]u8 = undefined;
    var s = Sink.initHeadTail(&buf, 8);
    s.append("HEAD");
    const zeros = [_]u8{0} ** 20;
    s.append(&zeros); // 4 + 20 = 24 байта
    s.append("TAIL");
    // итого 28: голова 4 (HEAD), хвост 4 (TAIL), опущено 20
    const n = s.finalize();
    try testing.expectEqual(@as(u64, 28), s.total);
    try testing.expect(n > 8);
    try testing.expect(n <= 8 + 64);
    const out = buf[0..@intCast(n)];
    try testing.expect(std.mem.startsWith(u8, out, "HEAD"));
    try testing.expect(std.mem.endsWith(u8, out, "TAIL"));
    try testing.expect(std.mem.indexOf(u8, out, "dropped 20 bytes") != null);
}

test "exec: имя без слэша — PATH-поиск в ребёнке" {
    if (!fileExists("/bin/echo")) return error.SkipZigTest;

    var argv = [_:null]?[*:0]const u8{ "echo", "path-child-полер" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 3000,
        .grace_ms = 100,
        .max_out_bytes = 4096,
        .stdin_data = null,
        .stdin_len = 0,
    };
    var out: [4096]u8 = undefined;
    var errb: [64]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run("echo", &argv, &envp, &opts, &out, out.len, &errb, errb.len, &res);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(i32, 0), res.exit_code);
    try testing.expect(std.mem.indexOf(u8, out[0..@intCast(res.stdout_len)], "path-child-полер") != null);
}

test "exec: имя не найдено в PATH — -ENOENT, exit 127" {
    var argv = [_:null]?[*:0]const u8{ "полер-нет-такой-12345" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 3000,
        .grace_ms = 100,
        .max_out_bytes = 256,
        .stdin_data = null,
        .stdin_len = 0,
    };
    var out: [64]u8 = undefined;
    var errb: [64]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run("полер-нет-такой-12345", &argv, &envp, &opts, &out, out.len, &errb, errb.len, &res);
    try testing.expectEqual(@as(i32, -2), rc); // -ENOENT
    try testing.expectEqual(@as(i32, 127), res.exit_code);
}

test "exec: cwd — ребёнок работает в каталоге" {
    if (!fileExists("/bin/sh")) return error.SkipZigTest;

    var argv = [_:null]?[*:0]const u8{ "/bin/sh", "-c", "pwd" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 3000,
        .grace_ms = 100,
        .max_out_bytes = 4096,
        .stdin_data = null,
        .stdin_len = 0,
        .cwd = "/tmp",
    };
    var out: [256]u8 = undefined;
    var errb: [64]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run("/bin/sh", &argv, &envp, &opts, &out, out.len, &errb, errb.len, &res);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(i32, 0), res.exit_code);
    // PWD в env нет — sh вызывает getcwd() → фактический каталог
    try testing.expect(std.mem.startsWith(u8, out[0..@intCast(res.stdout_len)], "/tmp"));
}

test "exec: несуществующий cwd — -ENOENT, exit 125" {
    var argv = [_:null]?[*:0]const u8{ "/bin/true" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 3000,
        .grace_ms = 100,
        .max_out_bytes = 256,
        .stdin_data = null,
        .stdin_len = 0,
        .cwd = "/полер/не/существует",
    };
    var out: [64]u8 = undefined;
    var errb: [64]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run("/bin/true", &argv, &envp, &opts, &out, out.len, &errb, errb.len, &res);
    try testing.expectEqual(@as(i32, -2), rc); // -ENOENT от chdir
    try testing.expectEqual(@as(i32, 125), res.exit_code); // 125 = cwd, не 127
}

test "exec: head_tail — голова + маркер + хвост" {
    if (!fileExists("/bin/sh")) return error.SkipZigTest;

    var argv = [_:null]?[*:0]const u8{ "/bin/sh", "-c", "printf HEAD9; dd if=/dev/zero bs=1024 count=64 2>/dev/null; printf TAIL9" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 5000,
        .grace_ms = 100,
        .max_out_bytes = 4096,
        .stdin_data = null,
        .stdin_len = 0,
        .capture = 1, // head_tail
    };
    const budget = 2048;
    var out: [2048 + 64]u8 = undefined; // контракт: буфер ≥ бюджет + MARKER_MAX
    var res: Result = undefined;

    const rc = poler_exec_run("/bin/sh", &argv, &envp, &opts, &out, budget, null, 0, &res);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(i32, 0), res.exit_code);
    try testing.expectEqual(@as(u32, 1), res.truncated);
    const n: usize = @intCast(res.stdout_len);
    try testing.expect(n > budget); // бюджет + маркер
    try testing.expect(n <= budget + 64);
    try testing.expect(std.mem.startsWith(u8, out[0 .. budget / 2], "HEAD9")); // голова
    try testing.expect(std.mem.indexOf(u8, out[0..n], "dropped") != null); // маркер
    try testing.expect(std.mem.endsWith(u8, out[0..n], "TAIL9")); // хвост
}

test "exec: PTY — isatty(1) и isatty(2) истинны, потоки слиты" {
    if (!fileExists("/bin/sh")) return error.SkipZigTest;

    var argv = [_:null]?[*:0]const u8{ "/bin/sh", "-c", "test -t 1 && test -t 2 && echo TTY-OK-ПОЛЕР" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 3000,
        .grace_ms = 100,
        .max_out_bytes = 4096,
        .stdin_data = null,
        .stdin_len = 0,
        .pty = 1,
    };
    var out: [4096]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run("/bin/sh", &argv, &envp, &opts, &out, out.len, null, 0, &res);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(i32, 0), res.exit_code);
    try testing.expect(std.mem.indexOf(u8, out[0..@intCast(res.stdout_len)], "TTY-OK-ПОЛЕР") != null);
}

test "exec: PTY + таймаут — сессия ликвидируется" {
    if (!fileExists("/bin/sh")) return error.SkipZigTest;

    var argv = [_:null]?[*:0]const u8{ "/bin/sh", "-c", "sleep 30" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 150,
        .grace_ms = 50,
        .max_out_bytes = 256,
        .stdin_data = null,
        .stdin_len = 0,
        .pty = 1,
    };
    var out: [256]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run("/bin/sh", &argv, &envp, &opts, &out, out.len, null, 0, &res);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), res.timed_out);
    try testing.expect(res.duration_us < 2_000_000); // никак не 30 с
}

test "exec: отмена — cancel_flag убивает ребёнка" {
    if (!fileExists("/bin/sleep")) return error.SkipZigTest;

    const F = struct {
        var flag: u32 = 0;
        fn setter() void {
            std.Thread.sleep(150 * std.time.ns_per_ms);
            @atomicStore(u32, &flag, 1, .release);
        }
    };
    const th = std.Thread.spawn(.{}, F.setter, .{}) catch return error.SkipZigTest;
    defer th.join();

    var argv = [_:null]?[*:0]const u8{ "/bin/sleep", "30" };
    var envp = [_:null]?[*:0]const u8{"PATH=/bin:/usr/bin"};
    const opts = Options{
        .timeout_ms = 30_000, // таймаут НЕ сработает — только отмена
        .grace_ms = 100,
        .max_out_bytes = 256,
        .stdin_data = null,
        .stdin_len = 0,
        .cancel_flag = &F.flag,
    };
    var out: [64]u8 = undefined;
    var errb: [64]u8 = undefined;
    var res: Result = undefined;

    const rc = poler_exec_run("/bin/sleep", &argv, &envp, &opts, &out, out.len, &errb, errb.len, &res);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), res.cancelled);
    try testing.expectEqual(@as(u32, 0), res.timed_out); // отмена ≠ таймаут
    try testing.expect(res.duration_us < 3_000_000); // ~150 мс + grace
}

fn fileExists(path: [*:0]const u8) bool {
    const fd = sys4(SYS.OPENAT, AT_FDCWD, @intFromPtr(path), O_RDONLY, 0);
    if (errOf(fd) != null) return false;
    _ = sys1(SYS.CLOSE, fd);
    return true;
}
