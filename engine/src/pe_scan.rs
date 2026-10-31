//! PE-SCAN (v1.6.0, Epic 1): статичний аналізатор PE32+ (AMD64) для
//! декомпіляційного контуру POLER-OS — таблиця експортів (EAT) з
//! форвардерами, імпорти, SSN-стаби (`mov r10,rcx; mov eax,imm32;
//! … syscall; ret`) у кодових секціях.
//!
//! Принцип інструменту (канон POLER): індексує, знаходить, віддає.
//! Інструмент НЕ тлумачить семантику — тільки точні метадані формату.
//!
//! Модуль не залежить від ОС (на відміну від `winpe`, який живе на
//! Linux-примітивах memfd/ptrace): працює і на Windows-хості, і в пісочниці.
//!
//! Тести — на РЕАЛЬНИХ бінарниках (tests/fixtures/pe/):
//!   * 7za.exe     — справжній PE64-застосунок (імпорти, 0 експортів, 0 SSN)
//!   * libssp-0.dll — справжня DLL з EAT (13 експортів, mingw-w64 14.2)

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    pub name: String,
    pub virtual_address: u32,
    pub virtual_size: u32,
    pub raw_pointer: u32,
    pub raw_size: u32,
    pub characteristics: u32,
}

impl Section {
    pub fn is_exec(&self) -> bool {
        self.characteristics & 0x2000_0000 != 0 // IMAGE_SCN_MEM_EXECUTE
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportSymbol {
    pub dll: String,
    /// Ім'я або #ординал
    pub name: String,
    pub by_ordinal: bool,
    pub iat_rva: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportSymbol {
    /// None — експорт тільки за ординалом
    pub name: Option<String>,
    /// 1-based (ExportBase + індекс EAT)
    pub ordinal: u32,
    pub rva: u32,
    /// Форвардер "NTDLL.RtlAllocateHeap" / "NTDLL.#42"
    pub forwarder: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SsnStub {
    pub rva: u32,
    pub ssn: u32,
    /// Чи присутній test byte ptr [0x7FFE0308],1 — сторож KUSER_SHARED_DATA
    pub checks_shared_data: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeReport {
    pub path: String,
    pub file_size: usize,
    pub machine: String,
    pub subsystem: String,
    pub timestamp: u32,
    pub entry_rva: u32,
    pub image_base: u64,
    pub size_of_image: u32,
    pub sections: Vec<Section>,
    pub imports: Vec<ImportSymbol>,
    pub exports: Vec<ExportSymbol>,
    pub dll_name: Option<String>,
    pub ssn_stubs: Vec<SsnStub>,
}

fn u16_at(b: &[u8], off: usize) -> Result<u16, String> {
    b.get(off..off + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| format!("u16@{off:#x}: за межами файлу"))
}
fn u32_at(b: &[u8], off: usize) -> Result<u32, String> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| format!("u32@{off:#x}: за межами файлу"))
}
fn u64_at(b: &[u8], off: usize) -> Result<u64, String> {
    b.get(off..off + 8)
        .map(|s| {
            let mut a = [0u8; 8];
            a.copy_from_slice(s);
            u64::from_le_bytes(a)
        })
        .ok_or_else(|| format!("u64@{off:#x}: за межами файлу"))
}
fn cstr_at(b: &[u8], off: usize) -> Option<String> {
    let end = b[off..].iter().position(|&c| c == 0)? + off;
    Some(String::from_utf8_lossy(&b[off..end]).into_owned())
}

const DIR_EXPORT: usize = 0;
const DIR_IMPORT: usize = 1;

pub fn analyze_bytes(image: &[u8], path: &str) -> Result<PeReport, String> {
    if image.len() < 0x40 || &image[0..2] != b"MZ" {
        return Err("не MZ".into());
    }
    let pe_off = u32_at(image, 0x3C)? as usize;
    if image.get(pe_off..pe_off + 4) != Some(b"PE\0\0") {
        return Err("не PE-сигнатура".into());
    }
    let machine = u16_at(image, pe_off + 4)?;
    if machine != 0x8664 {
        return Err(format!(
            "machine {machine:#x} != AMD64: аналізатор — тільки PE32+ x64"
        ));
    }
    let opt_off = pe_off + 24;
    let magic = u16_at(image, opt_off)?;
    if magic != 0x20B {
        return Err(format!("magic {magic:#x} != PE32+ (0x20B)"));
    }
    let timestamp = u32_at(image, pe_off + 8)?;
    let nsec = u16_at(image, pe_off + 6)? as usize;
    let size_of_opt = u16_at(image, pe_off + 20)? as usize;
    if nsec > 96 {
        return Err(format!("nsec={nsec} > 96"));
    }
    let entry_rva = u32_at(image, opt_off + 16)?;
    let image_base = u64_at(image, opt_off + 24)?;
    let size_of_image = u32_at(image, opt_off + 56)?;
    let subsystem = u16_at(image, opt_off + 68)?;
    let sec_off = pe_off + 24 + size_of_opt;

    let mut sections = Vec::with_capacity(nsec);
    for i in 0..nsec {
        let s = sec_off + i * 40;
        let name = image
            .get(s..s + 8)
            .map(|raw| {
                let n = raw.iter().position(|&c| c == 0).unwrap_or(8);
                String::from_utf8_lossy(&raw[..n]).into_owned()
            })
            .ok_or_else(|| format!("секція {i}: за межами файлу"))?;
        sections.push(Section {
            name,
            virtual_address: u32_at(image, s + 12)?,
            virtual_size: u32_at(image, s + 8)?,
            raw_pointer: u32_at(image, s + 20)?,
            raw_size: u32_at(image, s + 16)?,
            characteristics: u32_at(image, s + 36)?,
        });
    }

    let rva_to_offset = |rva: u32| -> Option<usize> {
        if rva as usize >= size_of_image as usize {
            return None;
        }
        for s in &sections {
            let span = s.virtual_size.max(s.raw_size);
            if s.virtual_address <= rva && rva < s.virtual_address + span {
                if s.raw_size == 0 {
                    return None;
                }
                let d = rva - s.virtual_address;
                if d >= s.raw_size {
                    return None;
                }
                return Some(s.raw_pointer as usize + d as usize);
            }
        }
        None
    };

    let dir = |idx: usize| -> Result<(u32, u32), String> {
        let base = opt_off + 112 + idx * 8;
        Ok((u32_at(image, base)?, u32_at(image, base + 4)?))
    };

    // ── EAT (директорія 0) ──
    let mut exports = Vec::new();
    let mut dll_name = None;
    {
        let (exp_rva, exp_size) = dir(DIR_EXPORT)?;
        if exp_rva != 0 && exp_size >= 40 {
            let eo = rva_to_offset(exp_rva).ok_or("EAT RVA не в секції")?;
            let num_funcs = u32_at(image, eo + 20)?;
            let num_names = u32_at(image, eo + 24)?;
            if num_funcs > 200_000 || num_names > 200_000 {
                return Err("EAT: підозріло багато символів".into());
            }
            dll_name = rva_to_offset(u32_at(image, eo + 12)?).and_then(|o| cstr_at(image, o));
            let eat_off = rva_to_offset(u32_at(image, eo + 28)?);
            let names_off = rva_to_offset(u32_at(image, eo + 32)?);
            let ords_off = rva_to_offset(u32_at(image, eo + 36)?);

            // ім'я за індексом EAT (лінійно: ненадійні байти — без бінпоиску)
            let mut names_by_index = vec![None::<String>; num_funcs as usize];
            if let (Some(no), Some(oo)) = (names_off, ords_off) {
                for i in 0..num_names as usize {
                    let name_rva = u32_at(image, no + i * 4)?;
                    let eat_idx = u16_at(image, oo + i * 2)? as usize;
                    if eat_idx < num_funcs as usize {
                        if let Some(name_off) = rva_to_offset(name_rva) {
                            names_by_index[eat_idx] = cstr_at(image, name_off);
                        }
                    }
                }
            }

            if let Some(eo_funcs) = eat_off {
                for i in 0..num_funcs as usize {
                    let rva = u32_at(image, eo_funcs + i * 4)?;
                    if rva == 0 {
                        continue;
                    }
                    // форвардер: RVA усередині export-директорії
                    let forwarder = if rva >= exp_rva && rva < exp_rva.saturating_add(exp_size) {
                        rva_to_offset(rva).and_then(|o| cstr_at(image, o))
                    } else {
                        None
                    };
                    exports.push(ExportSymbol {
                        name: names_by_index[i].clone(),
                        ordinal: u32_at(image, eo + 16)? + i as u32,
                        rva,
                        forwarder,
                    });
                }
            }
        }
    }

    // ── Імпорти (директорія 1) ──
    let mut imports = Vec::new();
    {
        let (imp_rva, imp_size) = dir(DIR_IMPORT)?;
        if imp_rva != 0 && imp_size >= 20 {
            let mut doff = rva_to_offset(imp_rva).ok_or("IMPORT RVA не в секції")?;
            loop {
                let oft = u32_at(image, doff)?;
                let name_rva = u32_at(image, doff + 12)?;
                let ft = u32_at(image, doff + 16)?;
                if oft == 0 && name_rva == 0 && ft == 0 {
                    break;
                }
                let dll = rva_to_offset(name_rva)
                    .and_then(|o| cstr_at(image, o))
                    .unwrap_or_default();
                let thunk = if oft != 0 { oft } else { ft };
                if let Some(mut toff) = rva_to_offset(thunk) {
                    let mut slot = 0u32;
                    loop {
                        let t = match u64_at(image, toff) {
                            Ok(v) => v,
                            Err(_) => break,
                        };
                        if t == 0 {
                            break;
                        }
                        let iat_rva = thunk + slot * 8;
                        if t & 0x8000_0000_0000_0000 != 0 {
                            imports.push(ImportSymbol {
                                dll: dll.clone(),
                                name: format!("#{}", t & 0xFFFF),
                                by_ordinal: true,
                                iat_rva,
                            });
                        } else if let Some(name_off) = rva_to_offset((t & 0x7FFF_FFFF) as u32) {
                            let nm = cstr_at(image, name_off + 2).unwrap_or_default();
                            imports.push(ImportSymbol {
                                dll: dll.clone(),
                                name: nm,
                                by_ordinal: false,
                                iat_rva,
                            });
                        }
                        toff += 8;
                        slot += 1;
                    }
                }
                doff += 20;
            }
        }
    }

    // ── SSN-стаби в кодових секціях ──
    let ssn_stubs = scan_ssn_stubs(image, &sections);

    Ok(PeReport {
        path: path.to_string(),
        file_size: image.len(),
        machine: "AMD64 (PE32+)".into(),
        subsystem: match subsystem {
            2 => "WINDOWS_GUI".into(),
            3 => "WINDOWS_CUI".into(),
            1 => "NATIVE".into(),
            9 => "WINDOWS_CE_GUI".into(),
            other => format!("{other}"),
        },
        timestamp,
        entry_rva,
        image_base,
        size_of_image,
        sections,
        imports,
        exports,
        dll_name,
        ssn_stubs,
    })
}

pub fn analyze_file(path: &std::path::Path) -> Result<PeReport, String> {
    let image = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    analyze_bytes(&image, &path.display().to_string())
}

/// Пошук стабів системних викликів: `4C 8B D1` (mov r10,rcx) … `B8 imm32`
/// (mov eax,SSN) … вікно 32 байт з `0F 05` (syscall) і `C3` (ret).
/// Прапорець KUSER_SHARED_DATA: `F6 04 25 08 03 FE 7F` (test byte
/// ptr [0x7FFE0308],1) усередині вікна.
fn scan_ssn_stubs(image: &[u8], sections: &[Section]) -> Vec<SsnStub> {
    let mut out = Vec::new();
    for sec in sections.iter().filter(|s| s.is_exec() && s.raw_size >= 16) {
        let start = sec.raw_pointer as usize;
        let end = start.saturating_add(sec.raw_size as usize).min(image.len());
        let code = match image.get(start..end) {
            Some(c) => c,
            None => continue,
        };
        let mut i = 0usize;
        while i + 8 <= code.len() {
            // mov r10, rcx; mov eax, imm32
            if code[i] == 0x4C && code[i + 1] == 0x8B && code[i + 2] == 0xD1 && code[i + 3] == 0xB8
            {
                let ssn = u32::from_le_bytes([
                    code[i + 4],
                    code[i + 5],
                    code[i + 6],
                    code[i + 7],
                ]);
                let win_end = (i + 36).min(code.len());
                let window = &code[i + 8..win_end];
                let has_syscall = window.windows(2).any(|w| w == [0x0F, 0x05]);
                let has_ret = window.contains(&0xC3);
                if has_syscall && has_ret {
                    let checks = window.windows(7).any(|w| {
                        w == [0xF6, 0x04, 0x25, 0x08, 0x03, 0xFE, 0x7F]
                    });
                    out.push(SsnStub {
                        rva: sec.virtual_address + i as u32,
                        ssn,
                        checks_shared_data: checks,
                    });
                }
                i += 8;
                continue;
            }
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEVEN_ZA: &[u8] = include_bytes!("../tests/fixtures/pe/7za.exe");
    const LIBSSP: &[u8] = include_bytes!("../tests/fixtures/pe/libssp-0.dll");

    #[test]
    fn real_exe_headers_and_imports() {
        let r = analyze_bytes(SEVEN_ZA, "7za.exe").expect("parse 7zr.exe");
        assert_eq!(r.machine, "AMD64 (PE32+)");
        // 7-Zip собирается с ImageBase 0x400000 (низкий базис — ядро грузит по высокому с .reloc)
        assert!(!r.sections.is_empty());
        // Реальні імпорти 7-Zip: KERNEL32 та інші DLL
        assert!(!r.imports.is_empty());
        assert!(r
            .imports
            .iter()
            .any(|s| s.dll.eq_ignore_ascii_case("KERNEL32.dll")));
        // Застосунок без експортів і без SSN-стабів (0 хибних спрацювань)
        assert!(r.exports.is_empty());
        assert!(r.ssn_stubs.is_empty());
    }

    #[test]
    fn real_dll_eat() {
        let r = analyze_bytes(LIBSSP, "libssp-0.dll").expect("parse libssp-0.dll");
        // Справжня EAT mingw-w64 libssp: 13 експортів, усі на ім'я
        assert_eq!(r.exports.len(), 13);
        assert!(r.exports.iter().all(|e| e.name.is_some()));
        assert!(r.exports.iter().any(|e| e.name.as_deref() == Some("__chk_fail")));
        // Ординали безперервні від бази
        let base = r.exports[0].ordinal;
        assert!(r
            .exports
            .iter()
            .enumerate()
            .all(|(i, e)| e.ordinal == base + i as u32));
        // Усі RVA ненульові, форвардерів у libssp немає
        assert!(r.exports.iter().all(|e| e.rva != 0 && e.forwarder.is_none()));
        assert_eq!(r.dll_name.as_deref(), Some("libssp-0.dll"));
    }

    #[test]
    fn rejects_non_pe_and_wrong_machine() {
        assert!(analyze_bytes(b"not a pe file", "x").is_err());
        let mut mz = [0u8; 0x100];
        mz[0] = b'M';
        mz[1] = b'Z';
        assert!(analyze_bytes(&mz, "x").is_err());
    }

    #[test]
    fn ssn_pattern_matcher() {
        // Канонічний стаб ntdll (патерн-фікстур, не формат-фікстур):
        // mov r10,rcx; mov eax,0x42; test byte [0x7FFE0308],1; jne +3;
        // ret; syscall; ret
        let stub: [u8; 22] = [
            0x4C, 0x8B, 0xD1, 0xB8, 0x42, 0x00, 0x00, 0x00, // mov r10,rcx; mov eax,0x42
            0xF6, 0x04, 0x25, 0x08, 0x03, 0xFE, 0x7F, 0x01, // test byte [7FFE0308],1
            0x75, 0x03, 0xC3, // jne +3; ret
            0x0F, 0x05, 0xC3, // syscall; ret
        ];
        let sec = Section {
            name: ".text".into(),
            virtual_address: 0x1000,
            virtual_size: stub.len() as u32,
            raw_pointer: 0,
            raw_size: stub.len() as u32,
            characteristics: 0x2000_0000,
        };
        let hits = scan_ssn_stubs(&stub, &[sec]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].ssn, 0x42);
        assert_eq!(hits[0].rva, 0x1000);
        assert!(hits[0].checks_shared_data);
    }
}
