//! x64 C++ SEH — власний walker виключень (без OS-диспетчера).
//!
//! Емпіричні інваріанти, відреверсовані з байтів 7za 21.07 у попередній сесії:
//!  * UNWIND_INFO: b0=ver|flags, b1=prolog, b2=count, b3=framereg|off
//!    (не плутати байти!)
//!  * unwind-код: [offset, (OpInfo<<4)|Op] — оп у МОЛОДШОМУ ніблі 2-го байта
//!  * immediates лежать ПІСЛЯ свого опкода; масив обробляється у ПРЯМОМУ
//!    порядку (остання операція прологу — перша в масиві)
//!  * SAVE-офсети відносяться до ВСТАНОВЛЕНОГО кадру (після прологу)
//!  * вхід walk: RSP = entry_rsp + 8 (знятий push адреси повернення)
//!  * UnwindMapEntry.action — ПРЯМИЙ RVA деструктора (без дереференсу)
//!  * деструкторний/catch-тunki приймають RDX = встановлений rsp кадру
//!  * catch-тunk повертає адресу продовження в RAX;
//!    продовження живе у нормальному кадрі: RSP = establisher − пролог-дельта
//!    (= встановлений rsp)
//!  * FuncInfo v1 (0x19930520), RVA-базований, x64 IP-to-state map

use super::api::{LoadedImage, World};

pub const SEH_DEBUG: bool = false; // увімкнути для повного дампу

macro_rules! dbg {
    ($($t:tt)*) => {
        if SEH_DEBUG || std::env::var("POLER_SEH_DEBUG").is_ok() {
            eprintln!($($t)*);
        }
    };
}

// ============================== Структури (RVA-базовані) ==============================

#[derive(Clone, Copy)]
struct Ctx {
    rip: u64,
    rsp: u64,
    rbx: u64,
    rbp: u64,
    rdi: u64,
    rsi: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
}

struct Frame {
    established: u64, // rsp після прологу
    entry_rsp: u64,   // rsp на вході функції (return addr зверху)
    funcinfo_rva: u32,
    state: i32,
    begin_rva: u32,
    /// C-SEH: scope-таблиця __C_specific_handler (якщо є)
    scope_table_rva: u32,
}

unsafe fn r32(img: &LoadedImage, rva: u32) -> u32 {
    if rva == 0 || rva + 4 > img.info.size_of_image {
        return 0;
    }
    unsafe { std::ptr::read_unaligned((img.base + rva as u64) as *const u32) }
}
unsafe fn r16(img: &LoadedImage, rva: u32) -> u16 {
    if rva == 0 || rva + 2 > img.info.size_of_image {
        return 0;
    }
    unsafe { std::ptr::read_unaligned((img.base + rva as u64) as *const u16) }
}
unsafe fn r8(img: &LoadedImage, rva: u32) -> u8 {
    if rva == 0 || rva + 1 > img.info.size_of_image {
        return 0;
    }
    unsafe { std::ptr::read_unaligned((img.base + rva as u64) as *const u8) }
}
unsafe fn r64a(addr: u64) -> u64 {
    unsafe { std::ptr::read_unaligned(addr as *const u64) }
}

unsafe fn write_seh_u32(at: u64, v: u32) {
    unsafe { std::ptr::write_unaligned(at as *mut u32, v) }
}
unsafe fn write_seh_u64(at: u64, v: u64) {
    unsafe { std::ptr::write_unaligned(at as *mut u64, v) }
}

/// Ім'я типу з TypeDescriptor (name @ +0x10).
unsafe fn type_name(img: &LoadedImage, td_rva: u32) -> String {
    if td_rva == 0 {
        return "...".into(); // catch-all
    }
    let mut off = 0x10u64;
    let mut v = Vec::new();
    unsafe {
        while off < 512 && td_rva as u64 + off + 1 < img.info.size_of_image as u64 {
            let c = r8(img, td_rva + off as u32);
            if c == 0 {
                break;
            }
            v.push(c);
            off += 1;
        }
    }
    String::from_utf8_lossy(&v).into_owned()
}

// ============================== Throw-бік ==============================

unsafe fn catchable_type_names(img: &LoadedImage, throw_info: u64) -> Vec<String> {
    // ThrowInfo живе за АБСОЛЮТНОЮ адресою (PE передав покажчик);
    // його ПОЛЯ — RVA. {u32 attr; u32 pmfnUnwind; u32 pForward; u32 pCatchableTypeArray}
    let read_abs32 = |addr: u64| unsafe {
        std::ptr::read_unaligned(addr as *const u32)
    };
    let cta = read_abs32(throw_info + 12); // RVA CatchableTypeArray
    if cta == 0 || cta > 0x1000_0000 {
        return vec![];
    }
    let n = unsafe { r32(img, cta) }.min(16);
    let mut out = Vec::new();
    for i in 0..n {
        let ct = unsafe { r32(img, cta + 4 + i * 4) };
        if ct == 0 || ct > 0x1000_0000 {
            continue;
        }
        // CatchableType: {u32 props; u32 pType; ...} — усі RVA
        let td = unsafe { r32(img, ct + 4) };
        if td == 0 || td > 0x1000_0000 {
            continue;
        }
        out.push(unsafe { type_name(img, td) });
    }
    out
}

// ============================== .pdata / UNWIND ==============================

unsafe fn find_rf(img: &LoadedImage, rip: u64) -> Option<super::pe::RuntimeFunction> {
    let rva = (rip - img.base) as u32;
    for rf in &img.rfs {
        if rva >= rf.begin_rva && rva < rf.end_rva {
            return Some(*rf);
        }
    }
    None
}

const UW_PUSH_NONVOL: u8 = 0;
const UW_ALLOC_LARGE: u8 = 1;
const UW_ALLOC_SMALL: u8 = 2;
#[allow(dead_code)] // полная карта опкодов unwind-ABI x64 (документационная)
const UW_SET_FPREG: u8 = 3;
const UW_SAVE_NONVOL: u8 = 4;
const UW_SAVE_NONVOL_FAR: u8 = 5;
const UW_SAVE_XMM128: u8 = 8;
const UW_SAVE_XMM128_FAR: u8 = 9;
const UW_PUSH_MACHFRAME: u8 = 7;

// Полная карта регистров CONTEXT x64 (RAX..R15) — документационная
// полнота unwind-ABI; в match используются только задействованные.
#[allow(dead_code)]
const REG_RAX: usize = 0;
#[allow(dead_code)]
const REG_RCX: usize = 1;
#[allow(dead_code)]
const REG_RDX: usize = 2;
const REG_RBX: usize = 3;
#[allow(dead_code)]
const REG_RSP: usize = 4;
const REG_RBP: usize = 5;
const REG_RSI: usize = 6;
const REG_RDI: usize = 7;
#[allow(dead_code)]
const REG_R8: usize = 8;
#[allow(dead_code)]
const REG_R12: usize = 12;
#[allow(dead_code)]
const REG_R15: usize = 15;

fn get_reg(ctx: &Ctx, idx: usize) -> u64 {
    match idx {
        REG_RBX => ctx.rbx,
        REG_RBP => ctx.rbp,
        REG_RDI => ctx.rdi,
        REG_RSI => ctx.rsi,
        12 => ctx.r12,
        13 => ctx.r13,
        14 => ctx.r14,
        15 => ctx.r15,
        _ => 0,
    }
}
fn set_reg(ctx: &mut Ctx, idx: usize, v: u64) {
    match idx {
        REG_RBX => ctx.rbx = v,
        REG_RBP => ctx.rbp = v,
        REG_RDI => ctx.rdi = v,
        REG_RSI => ctx.rsi = v,
        12 => ctx.r12 = v,
        13 => ctx.r13 = v,
        14 => ctx.r14 = v,
        15 => ctx.r15 = v,
        _ => {}
    }
}

/// Віртуальне розгортання кадру: відновлює регістри, рахує established/entry,
/// знаходить FuncInfo та стан за IP.
unsafe fn virtual_unwind(img: &LoadedImage, ctx: &mut Ctx) -> Option<Frame> {
    let rip = ctx.rip;
    let rf = find_rf(img, rip)?;
    let rip_rva = (rip - img.base) as u32;
    let u = rf.unwind_rva;
    let b0 = unsafe { r8(img, u) };
    let _prolog_size = unsafe { r8(img, u + 1) };
    let count = unsafe { r8(img, u + 2) } as usize;
    let b3 = unsafe { r8(img, u + 3) };
    let frame_reg = (b3 & 0xF) as usize;
    let frame_off = (b3 >> 4) as u64;
    let _ = b0 & 0x07;
    let flags = b0 >> 3;

    // established: тіло функції тримає rsp = established (MSVC release);
    // при SET_FPREG — від frame-регістра
    let established = if frame_reg != 0 {
        get_reg(ctx, frame_reg).wrapping_sub(frame_off * 16)
    } else {
        ctx.rsp
    };

    // --- прохід 1: пролог-дельта ---
    // ФОРМАТ: count@+2 = ЧИСЛО СЛОТІВ (u16, з immediates!), код = {off, (info<<4)|op}
    let mut prolog_delta: i64 = 0;
    {
        let mut slot = 0usize;
        while slot < count {
            let code = unsafe { r16(img, u + 4 + (slot * 2) as u32) };
            let op = ((code >> 8) & 0xF) as u8;
            let info = ((code >> 12) & 0xF) as u8;
            match op {
                UW_PUSH_NONVOL => prolog_delta += 8,
                UW_ALLOC_SMALL => prolog_delta += (info as i64) * 8 + 8,
                UW_ALLOC_LARGE => {
                    if info == 0 {
                        let imm = unsafe { r16(img, u + 4 + ((slot + 1) * 2) as u32) };
                        prolog_delta += (imm as i64) * 8;
                        slot += 1;
                    } else {
                        slot += 3;
                    }
                }
                UW_SAVE_NONVOL | UW_SAVE_XMM128 => slot += 1,
                UW_SAVE_NONVOL_FAR | UW_SAVE_XMM128_FAR => slot += 2,
                _ => {}
            }
            slot += 1;
        }
    }
    let entry_rsp = (established as i64 + prolog_delta) as u64;

    // --- прохід 2: відновлення регістрів (ПРЯМИЙ порядок масиву) ---
    let mut v_rsp = established;
    {
        let mut slot = 0usize;
        while slot < count {
            let code = unsafe { r16(img, u + 4 + (slot * 2) as u32) };
            let op = ((code >> 8) & 0xF) as u8;
            let info = ((code >> 12) & 0xF) as u8;
            let code_off = (code & 0xFF) as u8;
            // частковий пролог: незастосовані частини пропускаємо
            if code_off as u32 > rip_rva.wrapping_sub(rf.begin_rva) {
                match op {
                    UW_ALLOC_LARGE => slot += if info == 0 { 1 } else { 3 },
                    UW_SAVE_NONVOL | UW_SAVE_XMM128 => slot += 1,
                    UW_SAVE_NONVOL_FAR | UW_SAVE_XMM128_FAR => slot += 2,
                    _ => {}
                }
                slot += 1;
                continue;
            }
            match op {
                UW_PUSH_NONVOL => {
                    set_reg(ctx, info as usize, unsafe { r64a(v_rsp) });
                    v_rsp += 8;
                }
                UW_ALLOC_SMALL => v_rsp += (info as u64) * 8 + 8,
                UW_ALLOC_LARGE => {
                    if info == 0 {
                        let imm = unsafe { r16(img, u + 4 + ((slot + 1) * 2) as u32) };
                        v_rsp += (imm as u64) * 8;
                        slot += 1;
                    } else {
                        slot += 3;
                    }
                }
                UW_SAVE_NONVOL => {
                    let off = unsafe { r16(img, u + 4 + ((slot + 1) * 2) as u32) };
                    // ВИВЧЕНО: офсет від ВСТАНОВЛЕНОГО кадру, ×8
                    set_reg(ctx, info as usize, unsafe { r64a(established + (off as u64) * 8) });
                    slot += 1;
                }
                UW_SAVE_NONVOL_FAR => {
                    let off = unsafe { r32(img, u + 4 + ((slot + 1) * 2) as u32) };
                    set_reg(ctx, info as usize, unsafe { r64a(established + off as u64) });
                    slot += 2;
                }
                UW_SAVE_XMM128 => {
                    slot += 1; // XMM не реставруємо
                }
                UW_SAVE_XMM128_FAR => slot += 2,
                UW_PUSH_MACHFRAME => v_rsp += 8,
                _ => {}
            }
            slot += 1;
        }
    }

    // --- personality → FuncInfo / scope-таблиця ---
    let mut funcinfo_rva: u32 = 0;
    let mut scope_table_rva: u32 = 0;
    {
        // count = слоти; codes_end = align4(4 + count*2)
        let codes_end = 4 + ((count + count % 2) * 2) as u32;
        if flags & 0x03 != 0 {
            let handler_rva = unsafe { r32(img, u + codes_end) };
            let data_rva = unsafe { r32(img, u + codes_end + 4) };
            dbg!(
                "[seh] UNWIND rva {u:#x}: flags={flags:#x} slots={count} handler={handler_rva:#x} data={data_rva:#x}"
            );
            if handler_rva != 0 && data_rva != 0 {
                let magic = unsafe { r32(img, data_rva) };
                if magic == 0x1993_0520 || magic == 0x1993_0521 || magic == 0x1993_0522 {
                    funcinfo_rva = data_rva;
                } else {
                    // C-SEH __C_specific_handler: scope-таблиця ВБУДОВАНА
                    // одразу за personality: {count; {Begin,End,Filter,Target}[]}
                    let st = u + codes_end + 4;
                    let n = unsafe { r32(img, st) };
                    if n > 0 && n < 64 {
                        let b0 = unsafe { r32(img, st + 4) };
                        let e0 = unsafe { r32(img, st + 8) };
                        if b0 >= rf.begin_rva && b0 < rf.end_rva && e0 <= rf.end_rva + 0x100 {
                            scope_table_rva = st;
                            dbg!("[seh]   C-SEH scope INLINE @{st:#x}: {n} областей");
                        }
                    }
                }
            }
        }
    }

    // --- стан за IP: x64 FuncInfo: count@+0x14, записи {ip, state} @+0x18 (без префікса) ---
    let mut state: i32 = -1;
    if funcinfo_rva != 0 {
        let n_ip = unsafe { r32(img, funcinfo_rva + 0x14) };
        let ip2s_rva = unsafe { r32(img, funcinfo_rva + 0x18) };
        if ip2s_rva != 0 && n_ip > 0 && n_ip < 4096 {
            let mut best: i32 = -1;
            for k in 0..n_ip {
                let ip = unsafe { r32(img, ip2s_rva + k * 8) };
                let st = unsafe { r32(img, ip2s_rva + k * 8 + 4) } as i32;
                if ip <= rip_rva {
                    best = st;
                } else {
                    break;
                }
            }
            state = best;
        }
    }

    Some(Frame {
        established,
        entry_rsp,
        funcinfo_rva,
        state,
        begin_rva: rf.begin_rva,
        scope_table_rva,
    })
}

// ============================== FuncInfo-структури ==============================

struct TryBlock {
    try_low: i32,
    try_high: i32,
    _catch_high: i32,
    handlers_rva: u32,
}

unsafe fn try_blocks(img: &LoadedImage, fi: u32) -> Vec<TryBlock> {
    // x64 FuncInfo: nTryBlocks@+0x0C (COUNT), dispTryBlockMap@+0x10
    let n = unsafe { r32(img, fi + 0x0C) };
    let tbm = unsafe { r32(img, fi + 0x10) };
    if tbm == 0 || n == 0 || n > 1024 {
        return vec![];
    }
    let mut out = Vec::new();
    for k in 0..n {
        let at = tbm.wrapping_add(k * 16);
        if at == 0 || at + 16 > img.info.size_of_image {
            break;
        }
        out.push(TryBlock {
            try_low: unsafe { r32(img, at) } as i32,
            try_high: unsafe { r32(img, at + 4) } as i32,
            _catch_high: unsafe { r32(img, at + 8) } as i32,
            handlers_rva: unsafe { r32(img, at + 12) },
        });
    }
    out
}

struct Handler {
    _admissible: u32,
    type_rva: u32,
    catch_rva: u32,
}

unsafe fn handlers(img: &LoadedImage, h_rva: u32) -> Vec<Handler> {
    let mut out = Vec::new();
    if h_rva == 0 {
        return out;
    }
    for k in 0..32u32 {
        let at = h_rva.wrapping_add(k * 16);
        if at == 0 || at + 16 > img.info.size_of_image {
            break;
        }
        let adm = unsafe { r32(img, at) };
        let ty = unsafe { r32(img, at + 4) };
        let ct = unsafe { r32(img, at + 8) };
        if ct == 0 && ty == 0 && adm == 0 {
            break;
        }
        out.push(Handler {
            _admissible: adm,
            type_rva: ty,
            catch_rva: ct,
        });
    }
    out
}

/// Виконує ланцюг деструкторів стану → toState (або до −1).
unsafe fn run_unwind_map(img: &LoadedImage, w: &World, fi: u32, from_state: i32, to_state: i32, established: u64, depth: &mut u32) {
    if *depth > 256 {
        return;
    }
    *depth += 1;
    let um = unsafe { r32(img, fi + 0x08) };
    if um == 0 {
        return;
    }
    // UnwindMap: масив {i32 toState; u32 action-RVA}, count = maxState
    let max_state = unsafe { r32(img, fi + 0x04) } as i32;
    let mut cur = from_state;
    let mut guard = 0;
    while cur > to_state && cur >= 0 && cur < max_state && guard < 512 {
        guard += 1;
        let at = um + (cur as u32) * 8;
        let to = unsafe { r32(img, at) } as i32;
        let action = unsafe { r32(img, at + 4) };
        if action != 0 {
            // ВИВЧЕНО: action — ПРЯМИЙ RVA; виклик з RDX = встановлений rsp
            let target = img.base + action as u64;
            dbg!("[seh] деструктор state {cur}→{to} @ rva {action:#x}, база кадру {established:#x}");
            let scratch = super::runtime::scratch_top().unwrap_or(0);
            let _ = super::runtime::win_call_on(target, scratch, established, established, 0, 0, 0);
            let _ = w;
        }
        if to >= cur {
            break; // захист від циклів
        }
        cur = to;
    }
    *depth -= 1;
}

// ============================== Головний walker ==============================

/// Кидок C++-виключення з субстрату. Повертає керування лише якщо
/// обробника не знайдено (тоді процес завершується).
#[allow(clippy::too_many_arguments)]
pub unsafe fn cxx_throw(
    obj: u64,
    throw_info: u64,
    w: &World,
    caller_rip: u64,
    bridge_entry: u64,
    saved_rbp: u64,
    saved_rdi: u64,
    saved_rsi: u64,
) -> u64 {
    let Some(img_ref) = w.with_inner(|inner| inner.image.as_ref().map(|i| unsafe {
        std::ptr::read(i as *const LoadedImage)
    })) else {
        eprintln!("[seh] кидок без образу?!");
        return 0;
    };
    let img = &img_ref;
    let throw_types = if throw_info != 0 {
        unsafe { catchable_type_names(img, throw_info) }
    } else {
        vec![]
    };
    dbg!(
        "[seh] THROW obj={obj:#x} info={throw_info:#x} типи={throw_types:?} caller_rip={caller_rip:#x}"
    );

    let live = super::runtime::capture_regs();

    let mut ctx = Ctx {
        rip: caller_rip,
        rsp: 0,
        rbx: live[0],
        rbp: saved_rbp,
        rdi: saved_rdi,
        rsi: saved_rsi,
        r12: live[1],
        r13: live[2],
        r14: live[3],
        r15: live[4],
    };
    // ВИВЧЕНО: input RSP = rsp0 + 8 (знятий push адреси повернення)
    ctx.rsp = bridge_entry + 8;

    eprintln!("[seh] обхід кадрів: rip={:#x} rsp={:#x}", ctx.rip, ctx.rsp);
    let mut frames: Vec<Frame> = Vec::new();
    let mut ctxs: Vec<Ctx> = Vec::new();
    for _step in 0..600 {
        if ctx.rip == 0 || ctx.rip < img.base || ctx.rip > img.base + 0x1000_0000 {
            break;
        }
        let Some(fr) = (unsafe { virtual_unwind(img, &mut ctx) }) else {
            // leaf без .pdata: повернення вручну
            let ret = unsafe { r64a(ctx.rsp) };
            ctx.rip = ret;
            ctx.rsp += 8;
            continue;
        };
        if fr.funcinfo_rva != 0 {
            // шукаємо catch
            let tbs = unsafe { try_blocks(img, fr.funcinfo_rva) };
            dbg!(
                "[seh] кадр rva {:#x} state={} FuncInfo={:#x} try-блоків {}",
                fr.begin_rva, fr.state, fr.funcinfo_rva, tbs.len()
            );
            for tb in &tbs {
                if fr.state >= tb.try_low && fr.state <= tb.try_high {
                    let hs = unsafe { handlers(img, tb.handlers_rva) };
                    for (hi, h) in hs.iter().enumerate() {
                        let hname = unsafe { type_name(img, h.type_rva) };
                        let matched = h.type_rva == 0
                            || throw_types.iter().any(|t| {
                                t == &hname
                                    || hname == "..." 
                                    || (t.ends_with(&hname) && hname.len() > 2)
                            });
                        dbg!(
                            "[seh] кадр rva {:#x} state {} try[{}..{}] хендлер#{hi} тип {hname} → {}",
                            fr.begin_rva, fr.state, tb.try_low, tb.try_high,
                            if matched { "ЗБІГ" } else { "ні" }
                        );
                        if matched {
                            // ===== ЗНАЙШЛИ ОБРОБНИК =====
                            let catch_frame = fr;
                            let catch_ctx = ctx;
                            unsafe {
                                return transfer_to_catch(
                                    img,
                                    w,
                                    obj,
                                    &catch_frame,
                                    &catch_ctx,
                                    &frames,
                                    &ctxs,
                                    tb.try_low,
                                    h.catch_rva,
                                );
                            }
                        }
                    }
                }
            }
        }
        // --- C-SEH: __try/__except зі scope-таблицею ---
        if fr.scope_table_rva != 0 {
            let n_scopes = unsafe { r32(img, fr.scope_table_rva) };
            for k in 0..n_scopes.min(64) {
                let at = fr.scope_table_rva + 4 + k * 16;
                let begin = unsafe { r32(img, at) };
                let end = unsafe { r32(img, at + 4) };
                let filter = unsafe { r32(img, at + 8) };
                let target = unsafe { r32(img, at + 12) };
                let rip_rva2 = (ctx.rip - img.base) as u32;
                let inside = rip_rva2 >= begin && rip_rva2 < end;
                dbg!(
                    "[seh] C-SEH scope[{k}] try[{begin:#x}..{end:#x}) фільтр={filter:#x} target={target:#x} rip={rip_rva2:#x} {}",
                    if inside { "ВСЕРЕДИНІ" } else { "поза" }
                );
                if !inside {
                    continue;
                }
                // оцінка: фільтр 0 = EXCEPTION_EXECUTE_HANDLER (лови все);
                // інакше викликаємо фільтр-функциюlet з EXCEPTION_POINTERS*
                let verdict: i32 = if filter == 0 {
                    1
                } else {
                    // будуємо EXCEPTION_POINTERS на скретч-стеку
                    let scratch = super::runtime::scratch_top().unwrap_or(0x1000);
                    let rec = scratch - 0x800; // запис у верхній частині скретч-області
                    let ptrs = scratch - 0x700;
                    unsafe {
                        // EXCEPTION_RECORD: Code, Flags, Record, Address, NParams, Info[4]
                        write_seh_u32(rec, 0xE06D7363);
                        write_seh_u32(rec + 4, 1); // EH_NONCONTINUABLE
                        write_seh_u64(rec + 8, 0);
                        write_seh_u64(rec + 16, ctx.rip);
                        write_seh_u32(rec + 24, 0);
                        write_seh_u64(ptrs, rec);
                        write_seh_u64(ptrs + 8, 0); // CONTEXT* = NULL
                    }
                    let r = super::runtime::win_call_on(
                        img.base + filter as u64,
                        scratch,
                        ptrs,
                        0,
                        0,
                        0,
                        0,
                    );
                    r as i32
                };
                dbg!("[seh] C-SEH вердикт: {verdict}");
                if verdict == 1 {
                    // ловимо: деструктори внутрішніх кадрів → стрибок у Target
                    let mut depth = 0u32;
                    for (fr2, _cx) in frames.iter().zip(ctxs.iter()).rev() {
                        if fr2.funcinfo_rva != 0 {
                            unsafe {
                                run_unwind_map(
                                    img,
                                    w,
                                    fr2.funcinfo_rva,
                                    fr2.state,
                                    -1,
                                    fr2.established,
                                    &mut depth,
                                );
                            }
                        }
                    }
                    let mut regs = [0u64; 16];
                    regs[REG_RBX] = ctx.rbx;
                    regs[REG_RBP] = ctx.rbp;
                    regs[REG_RDI] = ctx.rdi;
                    regs[REG_RSI] = ctx.rsi;
                    regs[12] = ctx.r12;
                    regs[13] = ctx.r13;
                    regs[14] = ctx.r14;
                    regs[15] = ctx.r15;
                    eprintln!(
                        "[seh] C-SEH: перехід у target {target:#x}, кадр {:#x}",
                        fr.established
                    );
                    super::runtime::win_continue(
                        img.base + target as u64,
                        fr.established,
                        &regs,
                    );
                    unreachable!()
                }
            }
        }
        let ret = unsafe { r64a(fr.entry_rsp) };
        let new_rsp = fr.entry_rsp + 8;
        dbg!(
            "[seh] walk: кадр rva {:#x} entry_rsp={:#x} → ret={:#x}",
            fr.begin_rva, fr.entry_rsp, ret
        );
        frames.push(fr);
        ctxs.push(ctx);
        // наступний кадр
        ctx = Ctx {
            rip: ret,
            rsp: new_rsp,
            ..ctx
        };
    }

    eprintln!(
        "[seh] необроблене C++-виключення (типи {throw_types:?}) — завершення"
    );
    std::process::exit(0xE0);
}

#[allow(clippy::too_many_arguments)]
unsafe fn transfer_to_catch(
    img: &LoadedImage,
    w: &World,
    obj: u64,
    catch_frame: &Frame,
    catch_ctx: &Ctx,
    inner_frames: &[Frame],
    inner_ctxs: &[Ctx],
    try_low: i32,
    catch_rva: u32,
) -> u64 {
    // 1) деструктори внутрішніх кадрів (від найглибшого до кадру catch)
    let mut depth = 0u32;
    for (fr, cx) in inner_frames.iter().zip(inner_ctxs).rev() {
        if fr.funcinfo_rva != 0 {
            unsafe {
                run_unwind_map(img, w, fr.funcinfo_rva, fr.state, -1, fr.established, &mut depth);
            }
            let _ = cx;
        }
    }
    // 2) деструктори самого кадру catch: state → tryLow
    if catch_frame.funcinfo_rva != 0 {
        unsafe {
            run_unwind_map(
                img,
                w,
                catch_frame.funcinfo_rva,
                catch_frame.state,
                try_low,
                catch_frame.established,
                &mut depth,
            );
        }
    }

    // 3) виклик catch-функциїleta: RDX = встановлений rsp кадру, RAX = продовження
    let target = img.base + catch_rva as u64;
    let scratch = super::runtime::scratch_top().unwrap_or(0x1000);
    dbg!(
        "[seh] catch-хендлер @ rva {catch_rva:#x}, база кадру {:#x}, obj={obj:#x}",
        catch_frame.established
    );
    let continuation =
        super::runtime::win_call_on(target, scratch, obj, catch_frame.established, 0, 0, 0);
    dbg!("[seh] продовження = {continuation:#x}");

    // 4) перехід: rsp = established кадру catch, регістри кадру живі
    let mut regs = [0u64; 16];
    regs[REG_RBX] = catch_ctx.rbx;
    regs[REG_RBP] = catch_ctx.rbp;
    regs[REG_RDI] = catch_ctx.rdi;
    regs[REG_RSI] = catch_ctx.rsi;
    regs[12] = catch_ctx.r12;
    regs[13] = catch_ctx.r13;
    regs[14] = catch_ctx.r14;
    regs[15] = catch_ctx.r15;
    dbg!(
        "[seh] СТРИБАЙ: rip={continuation:#x} rsp={:#x}",
        catch_frame.established
    );
    super::runtime::win_continue(continuation, catch_frame.established, &regs);
    unreachable!()
}
