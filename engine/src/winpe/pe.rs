//! PE32+ (AMD64) парсер — порт poler-os zig-kernel/src64/pe.zig на Rust.
//! Читає образ з пам'яті (без fs): заголовки, секції, імпорти, .pdata.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    /// RVA віртуальної адреси секції
    pub virtual_address: u32,
    /// Розмір у пам'яті (VirtualSize)
    pub virtual_size: u32,
    /// Зсув у файлі (PointerToRawData)
    pub raw_pointer: u32,
    /// Розмір у файлі (SizeOfRawData)
    pub raw_size: u32,
    /// Characteristics
    pub characteristics: u32,
}

impl Section {
    pub fn is_exec(&self) -> bool {
        self.characteristics & 0x20000000 != 0 // IMAGE_SCN_MEM_EXECUTE
    }
    pub fn is_write(&self) -> bool {
        self.characteristics & 0x80000000 != 0 // IMAGE_SCN_MEM_WRITE
    }
}

#[derive(Debug, Clone)]
pub struct ImportSymbol {
    pub dll: String,
    pub name: String,
    /// RVA IAT-слота, який треба заповнити
    pub iat_rva: u32,
    /// Це дані (не функція)? Для таких IAT має містити адресу ЗПИСУВАНОЇ комірки.
    pub is_data: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeFunction {
    pub begin_rva: u32,
    pub end_rva: u32,
    pub unwind_rva: u32,
}

#[derive(Clone)]
pub struct PeInfo {
    pub entry_rva: u32,
    pub image_base: u64,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub section_alignment: u32,
    pub sections: Vec<Section>,
    /// RVA директорії виключень (.pdata)
    pub exception_dir_rva: u32,
    pub exception_dir_size: u32,
    /// RVA директорії релокацій (.reloc)
    pub reloc_dir_rva: u32,
    pub reloc_dir_size: u32,
}

fn u16_at(b: &[u8], off: usize) -> Result<u16, String> {
    if off + 2 > b.len() {
        return Err(format!("u16@{off:#x}: за межами образу ({})", b.len()));
    }
    Ok(u16::from_le_bytes([b[off], b[off + 1]]))
}
fn u32_at(b: &[u8], off: usize) -> Result<u32, String> {
    if off + 4 > b.len() {
        return Err(format!("u32@{off:#x}: за межами образу ({})", b.len()));
    }
    Ok(u32::from_le_bytes([
        b[off],
        b[off + 1],
        b[off + 2],
        b[off + 3],
    ]))
}
fn u64_at(b: &[u8], off: usize) -> Result<u64, String> {
    if off + 8 > b.len() {
        return Err(format!("u64@{off:#x}: за межами образу ({})", b.len()));
    }
    Ok(u64::from_le_bytes([
        b[off],
        b[off + 1],
        b[off + 2],
        b[off + 3],
        b[off + 4],
        b[off + 5],
        b[off + 6],
        b[off + 7],
    ]))
}

/// Швидка перевилка: MZ + PE + AMD64 (PE32+).
pub fn is_pe32_plus(image: &[u8]) -> bool {
    if image.len() < 0x40 || &image[0..2] != b"MZ" {
        return false;
    }
    let pe_off = match u32_at(image, 0x3C) {
        Ok(v) => v as usize,
        Err(_) => return false,
    };
    if pe_off + 6 > image.len() || &image[pe_off..pe_off + 4] != b"PE\0\0" {
        return false;
    }
    matches!(u16_at(image, pe_off + 4), Ok(0x8664))
}

pub fn parse(image: &[u8]) -> Result<PeInfo, String> {
    if image.len() < 0x40 || &image[0..2] != b"MZ" {
        return Err("не MZ".into());
    }
    let pe_off = u32_at(image, 0x3C)? as usize;
    if pe_off + 24 > image.len() || &image[pe_off..pe_off + 4] != b"PE\0\0" {
        return Err("не PE-сигнатура".into());
    }
    let machine = u16_at(image, pe_off + 4)?;
    if machine != 0x8664 {
        return Err(format!(
            "machine {machine:#x} != AMD64: лише PE32+ x64 підтримується"
        ));
    }
    let num_sections = u16_at(image, pe_off + 6)? as usize;
    let opt_size = u16_at(image, pe_off + 20)? as usize;
    let opt_off = pe_off + 24;

    let magic = u16_at(image, opt_off)?;
    if magic != 0x20b {
        return Err(format!("optional magic {magic:#x} != PE32+ (0x20b)"));
    }
    let entry_rva = u32_at(image, opt_off + 16)?;
    let image_base = u64_at(image, opt_off + 24)?;
    let section_alignment = u32_at(image, opt_off + 32)?;
    let size_of_image = u32_at(image, opt_off + 56)?;
    let size_of_headers = u32_at(image, opt_off + 60)?;

    // Data directories: після 112 байтів фіксованої частини PE32+ → 16 директорій по 8 байт
    let dd_off = opt_off + 112;
    if dd_off + 16 * 8 > opt_off + opt_size {
        return Err("data directories не влізли в optional header".into());
    }
    let exception_dir_rva = u32_at(image, dd_off + 3 * 8)?;
    let exception_dir_size = u32_at(image, dd_off + 3 * 8 + 4)?;
    let reloc_dir_rva = u32_at(image, dd_off + 5 * 8)?;
    let reloc_dir_size = u32_at(image, dd_off + 5 * 8 + 4)?;

    let sec_tbl = opt_off + opt_size;
    let mut sections = Vec::with_capacity(num_sections);
    for i in 0..num_sections {
        let s = sec_tbl + i * 40;
        if s + 40 > image.len() {
            return Err(format!("секція {i} за межами файлу"));
        }
        sections.push(Section {
            virtual_address: u32_at(image, s + 12)?,
            virtual_size: u32_at(image, s + 8)?,
            raw_pointer: u32_at(image, s + 20)?,
            raw_size: u32_at(image, s + 16)?,
            characteristics: u32_at(image, s + 36)?,
        });
    }
    Ok(PeInfo {
        entry_rva,
        image_base,
        size_of_image,
        size_of_headers,
        section_alignment,
        sections,
        exception_dir_rva,
        exception_dir_size,
        reloc_dir_rva,
        reloc_dir_size,
    })
}

/// Застосовує BASE_RELOCATION (тип 10 = DIR64) до відображеного образу.
/// # Safety
/// `mapped` — валидный указатель на загруженный образ размером не меньше
/// `info.size_of_image`; вызывающий отвечает за exclusive-доступ к памяти
/// (релокации пишут по `mapped + rva`).
pub unsafe fn apply_relocs(image: &[u8], info: &PeInfo, mapped: *mut u8, delta: i64) -> Result<u64, String> {
    if info.reloc_dir_rva == 0 || info.reloc_dir_size == 0 || delta == 0 {
        return Ok(0);
    }
    let mut off = info
        .rva_to_offset(info.reloc_dir_rva)
        .ok_or("reloc RVA не в секції")?;
    let end = off + info.reloc_dir_size as usize;
    let mut n = 0u64;
    while off + 8 <= end.min(image.len()) {
        let page_rva = u32_at(image, off)?;
        let block_size = u32_at(image, off + 4)? as usize;
        if block_size < 8 || page_rva == 0 {
            break;
        }
        let entries = (block_size - 8) / 2;
        for i in 0..entries {
            let e = u16_at(image, off + 8 + i * 2)?;
            let etype = (e >> 12) as u8;
            let eoff = (e & 0xFFF) as u32;
            if etype == 0 {
                continue; // padding
            }
            if etype != 10 {
                continue; // лише DIR64 на x64
            }
            let target_rva = page_rva + eoff;
            if let Some(section) = info.sections.iter().find(|s| {
                let sz = s.virtual_size.max(s.raw_size);
                target_rva >= s.virtual_address && target_rva < s.virtual_address + sz
            }) {
                let _ = section;
                unsafe {
                    let at = mapped.add(target_rva as usize) as *mut u64;
                    let v = std::ptr::read_unaligned(at);
                    std::ptr::write_unaligned(at, (v as i64 + delta) as u64);
                }
                n += 1;
            }
        }
        off += block_size;
    }
    Ok(n)
}

impl PeInfo {
    /// RVA → файлового зсуву (через таблицю секцій).
    pub fn rva_to_offset(&self, rva: u32) -> Option<usize> {
        for s in &self.sections {
            let sz = s.virtual_size.max(s.raw_size);
            if rva >= s.virtual_address && rva < s.virtual_address + sz {
                let delta = rva - s.virtual_address;
                if delta < s.raw_size {
                    return Some((s.raw_pointer + delta) as usize);
                }
                return None; // у віртуальному заповненні (BSS-подібне)
            }
        }
        None
    }

    /// Список усіх імпортів (dll, символ, iat_rva).
    /// Дані-символи виявляємо за списком відомих CRT-констант.
    pub fn imports(&self, image: &[u8]) -> Result<Vec<ImportSymbol>, String> {
        let mut out = Vec::new();
        let dd_off_imports = 1; // directory index 1 = Import Table
        let pe_off = u32_at(image, 0x3C)? as usize;
        let opt_off = pe_off + 24;
        let import_rva = u32_at(image, opt_off + 112 + dd_off_imports * 8)?;
        if import_rva == 0 {
            return Ok(out);
        }
        const DATA_SYMBOLS: &[&str] = &[
            "_commode", "_fmode", "_iob", "environ", "_environ", "__initenv", "_wenviron",
            "_wcmdln", "_acmdln", "_mbctype", "_ctype", "__app_type", "_pgmptr", "_wpgmptr",
            "_dstbias", "_timezone", "_daylight", "tzname", "__mb_cur_max", "_sys_nerr",
            "_nhandle", "_osver", "_winver", "_winmajor", "_winminor", "_osplatform",
        ];
        let mut imp_off = self
            .rva_to_offset(import_rva)
            .ok_or("imports RVA не в секції")?;
        loop {
            if imp_off + 20 > image.len() {
                break;
            }
            let iat_rva = u32_at(image, imp_off + 16)?;
            let name_rva = u32_at(image, imp_off + 12)?;
            let int_rva = u32_at(image, imp_off)?; // OriginalFirstThunk
            if iat_rva == 0 && name_rva == 0 && int_rva == 0 {
                break; // кінець таблиці
            }
            let dll = match self.rva_to_offset(name_rva) {
                Some(o) => String::from_utf8_lossy(read_cstr(image, o)).into_owned(),
                None => String::new(),
            };
            let thunk_rva = if int_rva != 0 { int_rva } else { iat_rva };
            let mut slot_rva = iat_rva;
            let mut t_off = match self.rva_to_offset(thunk_rva) {
                Some(o) => o,
                None => {
                    imp_off += 20;
                    continue;
                }
            };
            loop {
                if t_off + 8 > image.len() || slot_rva == 0 {
                    break;
                }
                let thunk = u64_at(image, t_off)?;
                if thunk == 0 {
                    break;
                }
                let name = if thunk & 0x8000000000000000 != 0 {
                    format!("#{}", thunk & 0xFFFF)
                } else {
                    match self.rva_to_offset((thunk & 0xFFFFFFFF) as u32) {
                        Some(o) => {
                            // IMAGE_IMPORT_BY_NAME: 2 байти hint + ім'я
                            String::from_utf8_lossy(read_cstr(image, o + 2)).into_owned()
                        }
                        None => String::new(),
                    }
                };
                let is_data = DATA_SYMBOLS.iter().any(|d| *d == name);
                out.push(ImportSymbol {
                    dll: dll.clone(),
                    name,
                    iat_rva: slot_rva,
                    is_data,
                });
                t_off += 8;
                slot_rva = slot_rva.wrapping_add(8);
            }
            imp_off += 20;
        }
        Ok(out)
    }

    /// .pdata → список RUNTIME_FUNCTION.
    pub fn runtime_functions(&self, image: &[u8]) -> Vec<RuntimeFunction> {
        let mut out = Vec::new();
        if self.exception_dir_rva == 0 {
            return out;
        }
        let Some(mut off) = self.rva_to_offset(self.exception_dir_rva) else {
            return out;
        };
        let count = (self.exception_dir_size / 12) as usize;
        for _ in 0..count {
            if off + 12 > image.len() {
                break;
            }
            let begin = match u32_at(image, off) {
                Ok(v) => v,
                Err(_) => break,
            };
            let end = match u32_at(image, off + 4) {
                Ok(v) => v,
                Err(_) => break,
            };
            let unwind = match u32_at(image, off + 8) {
                Ok(v) => v,
                Err(_) => break,
            };
            if begin == 0 && end == 0 {
                break;
            }
            out.push(RuntimeFunction {
                begin_rva: begin,
                end_rva: end,
                unwind_rva: unwind,
            });
            off += 12;
        }
        out
    }
}

fn read_cstr(b: &[u8], off: usize) -> &[u8] {
    let mut end = off;
    while end < b.len() && b[end] != 0 {
        end += 1;
    }
    &b[off..end]
}

/// Форматований дамп імпортів по DLL (для діагностики).
pub fn imports_by_dll(image: &[u8]) -> Result<String, String> {
    let info = parse(image)?;
    let imports = info.imports(image)?;
    let mut dlls: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for i in imports {
        dlls.entry(i.dll).or_default().push(i.name);
    }
    let mut s = String::new();
    for (dll, names) in dlls {
        s.push_str(&format!("{dll}: {} символів\n", names.len()));
    }
    Ok(s)
}
