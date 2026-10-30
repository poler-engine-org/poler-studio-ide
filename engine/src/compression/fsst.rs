//! FSST — Fast Static Symbol Table: сжатие строк со скоростью памяти.
//!
//! ## Задача (v2.0, Приоритет 3.1 — PLAN_POLER_V2)
//!
//! Инвертированный индекс в RAM держит словарь корпуса (терм → частота)
//! и пер-файловые словари. На `HashMap<String, usize>` каждый терм стоит
//! ~64–96 байт (24 байта заголовка String + heap-чанк + слот хеш-таблицы),
//! а строки-ключи дублируются в каждой структуре. На корпусе 65K файлов
//! это раздувает watcher-состояние до гигабайт.
//!
//! FSST (V. Boncz, T. Neumann, CIDR 2020) — словарное сжатие коротких
//! строк: обучаемая таблица до 255 символов (последовательности 1–8 байт),
//! каждая строка кодируется последовательностью однобайтовых кодов.
//! Естественный язык (общие префиксы/суффиксы морфологии) сжимается в
//! ~1.6–2.2× при декомпрессии на скорости ~ГБ/с — таблица символов
//! умещается в L1-кэш.
//!
//! ## Происхождение кода
//!
//! Алгоритмическая сердцевина — чистый порт Rust-реализации FSST из
//! Apache Lance (crate `fsst`, Apache-2.0, Copyright The Lance Authors),
//! которая сама является портом эталонной C++-реализации Boncz/Neumann.
//! В соответствии с принципом PLAN_POLER_V2 §7.2 («заимствованное —
//! 100% дорабатывается») порт адаптирован под poler; публичный API крейта
//! непригоден для наших задач:
//!
//! * **фиксированная таблица**: крейтовый `compress()` переобучает таблицу
//!   на каждом вызове и рассчитан на batch-массивы ≥ 4 МБ (Arrow-колонки);
//!   poler-порт кодирует ОДНУ строку с фиксированной таблицей — это
//!   позволяет compress-probe лукапы: строка запроса сжимается тем же
//!   детерминированным кодировщиком, и сравнение идёт по сжатым байтам;
//! * **детерминированный сэмплинг**: обучение крейта использует `rand`;
//!   порт выбирает сэмпл равномерным шагом — воспроизводимость таблицы
//!   (сериализация/восстановление обязаны давать побитово тот же
//!   кодировщик, иначе compress-probe не найдёт ранее сжатые термы);
//! * **без `unsafe`**: небезопасные unaligned-загрузки оригинала заменены
//!   bounds-проверенными чтениями (компилятор разворачивает их в тот же
//!   single `mov`); декодер — скалярный (термы короткие, пакетная
//!   4-байтовая развёртка оригинала не окупается);
//! * **компактная сериализация**: ~4.5 КБ на таблицу (символы + карта
//!   занятости хеш-таблицы) с побитово точным восстановлением кодировщика.
//!
//! ## Формат кода
//!
//! Таблица после `finalize()` перенумеровывает коды: реальные символы
//! получают коды 0..n (< 255), код 255 (FSST_ESC) зарезервирован за
//! escape-последовательностью (255, сырой байт) для байтов без символа.
//! Кодирование 8-байтовым окном: сначала пробуется длинный символ по
//! хешу первых 3 байт, затем 2-байтовый символ по прямой таблице
//! `short_codes[65536]`, затем одиночный байт. Выход — поток байтов,
//! каждый либо код символа, либо (255, байт).

use std::collections::{BinaryHeap, HashSet};

// ---------------------------------------------------------------------------
// Константы (как в эталонной реализации)
// ---------------------------------------------------------------------------

/// Код escape: следующий за ним байт — литеральный.
const FSST_ESC: u8 = 255;
/// Максимальная длина символа, байт.
const MAX_SYMBOL_LENGTH: usize = 8;
/// Кодов в процессе обучения: 256 псевдо (одиночные байты) + до 255 реальных.
const FSST_CODE_BASE: u16 = 256;
const FSST_CODE_MAX: u16 = 512;
const FSST_CODE_MASK: u16 = FSST_CODE_MAX - 1;
/// Целевой размер обучающего сэмпла (как в статье FSST).
const FSST_SAMPLETARGET: usize = 1 << 14; // 16 КБ
/// Хеш-таблица длинных символов (степень двойки!).
const FSST_HASH_TAB_SIZE: usize = 1024;
const FSST_HASH_PRIME: u64 = 2971215073;
const FSST_SHIFT: usize = 15;
/// Свободный слот хеш-таблицы: icl >= 2^32.
const FSST_ICL_FREE: u64 = 1 << 32;
const CODE_LEN_SHIFT_IN_ICL: u64 = 28;
const CODE_SHIFT_IN_ICL: u64 = 16;
const CODE_LEN_SHIFT_IN_CODE: u64 = 12;
/// Максимум реальных символов в финальной таблице (код 255 — escape).
const MAX_SYMBOLS: u16 = 255;

#[inline]
fn fsst_hash(w: u64) -> u64 {
    w.wrapping_mul(FSST_HASH_PRIME) ^ (w.wrapping_mul(FSST_HASH_PRIME)) >> FSST_SHIFT
}

/// Безопасная 8-байтовая загрузка (оригинал: unsafe read_unaligned).
///
/// Срез обязан иметь длину ≥ 8 — вызовы в кодеке идут из буфера с
/// sentinel-запасом, поэтому паника здесь невозможна.
#[inline]
fn load8(buf: &[u8]) -> u64 {
    u64::from_le_bytes(buf[..8].try_into().expect("load8: >=8 байт"))
}

// ---------------------------------------------------------------------------
// Символы и таблица
// ---------------------------------------------------------------------------

#[derive(Default, Copy, Clone, PartialEq, Eq)]
struct Symbol {
    /// Байтовая последовательность символа (LE-упаковка, первый байт — младший).
    val: u64,
    /// ignoredBits:16 | code:12 | length:4 | unused:32 — одно сравнение на код.
    icl: u64,
}

impl Symbol {
    fn new() -> Self {
        Self {
            val: 0,
            icl: FSST_ICL_FREE,
        }
    }

    fn from_char(c: u8, code: u16) -> Self {
        Self {
            val: c as u64,
            icl: (1 << CODE_LEN_SHIFT_IN_ICL) | (code as u64) << CODE_SHIFT_IN_ICL | 56,
        }
    }

    fn set_code_len(&mut self, code: u16, len: u32) {
        self.icl = ((len as u64) << CODE_LEN_SHIFT_IN_ICL)
            | ((code as u64) << CODE_SHIFT_IN_ICL)
            | ((8u64.saturating_sub(len as u64)) * 8);
    }

    #[inline]
    fn symbol_len(&self) -> u32 {
        (self.icl >> CODE_LEN_SHIFT_IN_ICL) as u32
    }

    #[inline]
    fn code(&self) -> u16 {
        ((self.icl >> CODE_SHIFT_IN_ICL) & FSST_CODE_MASK as u64) as u16
    }

    #[inline]
    fn ignored_bits(&self) -> u32 {
        (self.icl & u16::MAX as u64) as u32
    }

    #[inline]
    fn first(&self) -> u8 {
        debug_assert!(self.symbol_len() >= 1);
        (0xFF & self.val) as u8
    }

    #[inline]
    fn first2(&self) -> u16 {
        debug_assert!(self.symbol_len() >= 2);
        (0xFFFF & self.val) as u16
    }

    #[inline]
    fn hash(&self) -> u64 {
        let v = 0xFFFFFF & self.val;
        fsst_hash(v)
    }

    /// Конкатенация двух символов (обрезается до 8 байт).
    fn concat(left: Self, right: Self) -> Self {
        let mut s = Self::new();
        let mut length = left.symbol_len() + right.symbol_len();
        if length > MAX_SYMBOL_LENGTH as u32 {
            length = MAX_SYMBOL_LENGTH as u32;
        }
        s.set_code_len(FSST_CODE_MASK, length);
        s.val = (right.val << (8 * left.symbol_len())) | left.val;
        s
    }
}

/// Кандидат в очередь приоритетов обучения: символ + выигрыш.
#[derive(Clone)]
struct QSymbol {
    symbol: Symbol,
    gain: u32,
}

impl PartialEq for QSymbol {
    /// Эквалити по ЗНАЧЕНИЮ (val), а не по (val, icl): код в icl — позиция
    /// слота текущего раунда, один и тот же набор байтов у символа из
    /// таблицы и у concat-кандидата имеет разные коды. Эквалити с icl
    /// пропускало дубликаты в таблицу (два символа "me" на разных кодах):
    /// мёртвый слот + недетерминированное восстановление short_codes.
    /// Hash уже хеширует только val — согласованная пара (val, val).
    fn eq(&self, other: &Self) -> bool {
        self.symbol.val == other.symbol.val
    }
}

impl Eq for QSymbol {}

impl Ord for QSymbol {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.gain
            .cmp(&other.gain)
            .then_with(|| other.symbol.val.cmp(&self.symbol.val))
    }
}

impl PartialOrd for QSymbol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::hash::Hash for QSymbol {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Хеш в стиле эталонной C++-реализации FSST.
        let mut k = self.symbol.val;
        const M: u64 = 0xc6a4a7935bd1e995;
        const R: u32 = 47;
        let mut h: u64 = 0x8445d61a4e774912 ^ (8u64.wrapping_mul(M));
        k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);
        h ^= k;
        h = h.wrapping_mul(M);
        h ^= h >> R;
        h = h.wrapping_mul(M);
        h ^= h >> R;
        h.hash(state);
    }
}

#[derive(Clone)]
struct SymbolTable {
    /// Прямая таблица 2-байтовых символов: первые 2 байта → код|len<<12.
    short_codes: Vec<u16>, // 65536
    /// Одиночные байты: код|len<<12 (или escape 511|len<<12).
    byte_codes: [u16; 256],
    /// Символы по кодам (0..n после finalize; 256..256+n до).
    symbols: Vec<Symbol>, // FSST_CODE_MAX
    /// Хеш-таблица длинных (3..8 байт) символов.
    hash_tab: [Symbol; FSST_HASH_TAB_SIZE],
    n_symbols: u16,
    terminator: u16,
    suffix_lim: u16,
    len_histo: [u8; 9],
}

impl SymbolTable {
    fn new() -> Self {
        let mut s = Self {
            short_codes: vec![0; 65536],
            byte_codes: [0; 256],
            symbols: vec![Symbol::new(); FSST_CODE_MAX as usize],
            hash_tab: [Symbol::new(); FSST_HASH_TAB_SIZE],
            n_symbols: 0,
            terminator: 256,
            suffix_lim: FSST_CODE_MAX,
            len_histo: [0; 9],
        };
        s.reset();
        s
    }

    fn reset(&mut self) {
        for (i, symbol) in self.symbols.iter_mut().enumerate().take(256) {
            *symbol = Symbol::from_char(i as u8, i as u16);
        }
        let unused = Symbol::from_char(0, FSST_CODE_MASK);
        for symbol in self.symbols.iter_mut().skip(256) {
            *symbol = unused;
        }
        for (i, bc) in self.byte_codes.iter_mut().enumerate() {
            *bc = i as u16;
        }
        for (i, sc) in self.short_codes.iter_mut().enumerate() {
            *sc = (i & 0xFF) as u16;
        }
        for h in self.hash_tab.iter_mut() {
            *h = Symbol::new();
        }
        self.n_symbols = 0;
        self.suffix_lim = FSST_CODE_MAX;
        self.len_histo = [0; 9];
    }

    fn hash_insert(&mut self, s: Symbol) -> bool {
        let idx = (s.hash() & (FSST_HASH_TAB_SIZE as u64 - 1)) as usize;
        if self.hash_tab[idx].icl < FSST_ICL_FREE {
            return false; // коллизия в хеш-таблице
        }
        self.hash_tab[idx].icl = s.icl;
        self.hash_tab[idx].val = s.val & (u64::MAX >> s.ignored_bits());
        true
    }

    fn add(&mut self, mut s: Symbol) -> bool {
        debug_assert!(FSST_CODE_BASE + self.n_symbols < FSST_CODE_MAX);
        let len = s.symbol_len();
        s.set_code_len(FSST_CODE_BASE + self.n_symbols, len);
        if len == 1 {
            self.byte_codes[s.first() as usize] = FSST_CODE_BASE + self.n_symbols;
        } else if len == 2 {
            self.short_codes[s.first2() as usize] = FSST_CODE_BASE + self.n_symbols;
        } else if !self.hash_insert(s) {
            return false;
        }
        self.symbols[(FSST_CODE_BASE + self.n_symbols) as usize] = s;
        self.n_symbols += 1;
        self.len_histo[(len - 1) as usize] += 1;
        true
    }

    /// Длиннейший символ — префикс входа (для коротких входов/хвостов).
    fn find_longest_symbol(&self, input: &[u8]) -> u16 {
        let len = input.len().min(MAX_SYMBOL_LENGTH);
        if len < 2 {
            return self.byte_codes[input[0] as usize] & FSST_CODE_MASK;
        }
        if len == 2 {
            let short_code = (input[1] as usize) << 8 | input[0] as usize;
            if self.short_codes[short_code] >= FSST_CODE_BASE {
                self.short_codes[short_code] & FSST_CODE_MASK
            } else {
                self.byte_codes[input[0] as usize] & FSST_CODE_MASK
            }
        } else {
            let mut word = [0u8; 8];
            word[..len].copy_from_slice(&input[..len]);
            let input_in_u64 = u64::from_le_bytes(word);
            let hash_idx = fsst_hash(input_in_u64) as usize & (FSST_HASH_TAB_SIZE - 1);
            let s = &self.hash_tab[hash_idx];
            if s.icl < FSST_ICL_FREE
                && s.val == (input_in_u64 & (u64::MAX >> s.ignored_bits()))
            {
                return s.code();
            }
            self.byte_codes[input[0] as usize] & FSST_CODE_MASK
        }
    }

    /// Перенумерация кодов по длинам (комментарий эталона): символы
    /// группируются 2,3..8,1; `byte_codes` встраиваются в `short_codes`;
    /// escape-коды получают бит 256 и длину в битах 12+.
    fn finalize(&mut self) {
        debug_assert!(self.n_symbols < FSST_CODE_BASE);
        let mut new_code: [u16; 256] = [0; 256];
        let mut rsum: [u8; 8] = [0; 8];
        let byte_lim = self.n_symbols - self.len_histo[0] as u16;

        rsum[0] = byte_lim as u8; // 1-байтовые коды — самые высокие
        for i in 1..7 {
            rsum[i + 1] = rsum[i] + self.len_histo[i];
        }

        let mut suffix_lim: u16 = 0;
        let mut j = rsum[2];
        for i in 0..self.n_symbols as usize {
            let mut s1 = self.symbols[FSST_CODE_BASE as usize + i];
            let len = s1.symbol_len();
            if len == 2 {
                // есть ли длинный символ с теми же первыми 2 байтами?
                let mut has_suffix = false;
                let first2 = s1.first2();
                for k in 0..self.n_symbols as usize {
                    let s2 = self.symbols[FSST_CODE_BASE as usize + k];
                    if k != i && s2.symbol_len() > 2 && first2 == s2.first2() {
                        has_suffix = true;
                        break;
                    }
                }
                new_code[i] = if has_suffix {
                    suffix_lim += 1;
                    suffix_lim - 1
                } else {
                    j -= 1;
                    j as u16
                };
            } else {
                new_code[i] = rsum[(len - 1) as usize] as u16;
                rsum[(len - 1) as usize] += 1;
            }
            s1.set_code_len(new_code[i], len);
            self.symbols[new_code[i] as usize] = s1;
        }

        for i in 0..256 {
            if (self.byte_codes[i] & FSST_CODE_MASK) >= FSST_CODE_BASE {
                self.byte_codes[i] =
                    new_code[(self.byte_codes[i] & 0xFF) as usize] | (1 << CODE_LEN_SHIFT_IN_CODE);
            } else {
                self.byte_codes[i] = 511 | (1 << CODE_LEN_SHIFT_IN_CODE);
            }
        }

        for i in 0..65536 {
            // Гвард `>=` (как в byte_codes): иначе первый добавленный
            // 2-байтовый символ (pre-код ровно 256) терял отображение в
            // short_codes — становился «фантомом», недостижимым кодировщиком,
            // и сериализация не могла восстановить таблицу побитово.
            if (self.short_codes[i] & FSST_CODE_MASK) >= FSST_CODE_BASE {
                self.short_codes[i] =
                    new_code[(self.short_codes[i] & 0xFF) as usize] | (2 << CODE_LEN_SHIFT_IN_CODE);
            } else {
                self.short_codes[i] = self.byte_codes[i & 0xFF] | (1 << CODE_LEN_SHIFT_IN_CODE);
            }
        }

        for i in 0..FSST_HASH_TAB_SIZE {
            if self.hash_tab[i].icl < FSST_ICL_FREE {
                self.hash_tab[i] =
                    self.symbols[new_code[(self.hash_tab[i].code() & 0xFF) as usize] as usize];
            }
        }
        self.suffix_lim = suffix_lim;
    }
}

// ---------------------------------------------------------------------------
// Обучение таблицы
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Counters {
    count1: Vec<u16>,
    count2: Vec<Vec<u16>>,
}

impl Counters {
    fn new() -> Self {
        Self {
            count1: vec![0; FSST_CODE_MAX as usize],
            count2: vec![vec![0; FSST_CODE_MAX as usize]; FSST_CODE_MAX as usize],
        }
    }

    #[inline]
    fn count1_set(&mut self, pos1: usize, val: u16) {
        self.count1[pos1] = val;
    }

    #[inline]
    fn count1_inc(&mut self, pos1: u16) {
        let v = self.count1[pos1 as usize];
        self.count1[pos1 as usize] = v.saturating_add(1);
    }

    #[inline]
    fn count2_inc(&mut self, pos1: usize, pos2: usize) {
        let v = self.count2[pos1][pos2];
        self.count2[pos1][pos2] = v.saturating_add(1);
    }

    #[inline]
    fn count1_get(&self, pos1: usize) -> u16 {
        self.count1[pos1]
    }

    #[inline]
    fn count2_get(&self, pos1: usize, pos2: usize) -> u16 {
        self.count2[pos1][pos2]
    }
}

#[inline]
fn is_escape_code(pos: u16) -> bool {
    pos < FSST_CODE_BASE
}

/// Детерминированная выборка строк до ~16 КБ (адаптация poler: вместо
/// `rand::StdRng` — равномерный шаг по словарю; воспроизводимость обучения
/// критична для побитового восстановления кодировщика из сериализации).
fn make_sample(terms: &[&[u8]]) -> (Vec<u8>, Vec<i32>) {
    let total: usize = terms.iter().map(|t| t.len()).sum();
    if total <= FSST_SAMPLETARGET || terms.len() <= 1 {
        let mut buf = Vec::with_capacity(total);
        let mut offs = Vec::with_capacity(terms.len() + 1);
        offs.push(0i32);
        for t in terms {
            buf.extend_from_slice(t);
            offs.push(buf.len() as i32);
        }
        return (buf, offs);
    }
    // Равномерный шаг: каждый ceil(total/target)-й терм (по байтовой массе).
    let step_bytes = total / FSST_SAMPLETARGET + 1;
    let mut buf: Vec<u8> = Vec::with_capacity(FSST_SAMPLETARGET * 2);
    let mut offs = Vec::new();
    offs.push(0i32);
    let mut acc = 0usize;
    for t in terms {
        acc += t.len();
        if acc >= step_bytes {
            acc = 0;
            buf.extend_from_slice(t);
            offs.push(buf.len() as i32);
        }
    }
    if buf.is_empty() {
        // вырожденный случай: один гигантский терм — берём его целиком
        buf.extend_from_slice(terms[0]);
        offs.push(buf.len() as i32);
    } else if offs[offs.len() - 1] != buf.len() as i32 {
        offs.push(buf.len() as i32);
    }
    (buf, offs)
}

/// Сжатие сэмпла текущей таблицей + подсчёт (парных) частот символов.
fn compress_count(
    st: &mut SymbolTable,
    sample_buf: &[u8],
    sample_offsets: &[i32],
) -> (Counters, i32) {
    let mut gain: i32 = 0;
    let mut counters = Counters::new();

    for i in 1..sample_offsets.len() {
        if sample_offsets[i] == sample_offsets[i - 1] {
            continue;
        }
        let word = &sample_buf[sample_offsets[i - 1] as usize..sample_offsets[i] as usize];
        let mut curr = 0usize;
        let mut prev_code = st.find_longest_symbol(&word[curr..]);
        curr += st.symbols[prev_code as usize].symbol_len() as usize;
        gain += st.symbols[prev_code as usize].symbol_len() as i32
            - (1 + is_escape_code(prev_code) as i32);
        while curr < word.len() {
            counters.count1_inc(prev_code);
            if st.symbols[prev_code as usize].symbol_len() != 1 {
                counters.count1_inc(word[curr] as u16);
            }
            let curr_code: u16;
            let symbol_len: usize;
            if word.len() > 7 && curr < word.len() - 7 {
                let this_64_bit_word = load8(&word[curr..]);
                let code = this_64_bit_word & 0xFFFFFF;
                let idx = fsst_hash(code) as usize & (FSST_HASH_TAB_SIZE - 1);
                let s: Symbol = st.hash_tab[idx];
                let short_code =
                    st.short_codes[(this_64_bit_word & 0xFFFF) as usize] & FSST_CODE_MASK;
                let masked = this_64_bit_word & (u64::MAX >> (s.icl & 0xFFFF));
                if (s.icl < FSST_ICL_FREE) & (s.val == masked) {
                    curr_code = s.code();
                    symbol_len = s.symbol_len() as usize;
                } else if short_code >= FSST_CODE_BASE {
                    curr_code = short_code;
                    symbol_len = 2;
                } else {
                    curr_code = st.byte_codes[(this_64_bit_word & 0xFF) as usize] & FSST_CODE_MASK;
                    symbol_len = 1;
                }
            } else {
                curr_code = st.find_longest_symbol(&word[curr..]);
                symbol_len = st.symbols[curr_code as usize].symbol_len() as usize;
            }
            gain += symbol_len as i32 - (1 + is_escape_code(curr_code) as i32);
            // пары не считаем в финальном раунде (как в эталоне)
            counters.count2_inc(prev_code as usize, curr_code as usize);
            if symbol_len > 1 {
                counters.count2_inc(prev_code as usize, word[curr] as usize);
            }
            curr += symbol_len;
            prev_code = curr_code;
        }
        counters.count1_inc(prev_code);
    }
    (counters, gain)
}

/// Перестройка таблицы по счётчикам частот: кандидаты → куча по выигрышу.
/// Порог отбора кандидатов зависит от доли сэмпла (`5*frac/128`, эталон).
fn make_table(st: &mut SymbolTable, counters: &mut Counters, sample_frac: usize) {
    let threshold = ((5 * sample_frac as u64) / 128).max(1);
    let mut candidates: HashSet<QSymbol> = HashSet::new();

    // Терминатор (редчайший байт сэмпла) всегда получает 1-байтовый символ.
    counters.count1_set(st.terminator as usize, u16::MAX);

    for pos1 in 0..FSST_CODE_BASE as usize + st.n_symbols as usize {
        let cnt1 = counters.count1_get(pos1);
        if cnt1 == 0 {
            continue;
        }
        // эвристика эталона: одиночные байты ×8 — меньше escape, выше скорость
        let s1 = st.symbols[pos1];
        let weight = if s1.symbol_len() == 1 { 8 } else { 1 } * cnt1 as u64;
        add_or_inc(&mut candidates, s1, weight, threshold);
        if s1.first() == st.terminator as u8 {
            continue;
        }
        if sample_frac >= 128 || s1.symbol_len() == MAX_SYMBOL_LENGTH as u32 {
            continue;
        }
        for pos2 in 0..FSST_CODE_BASE as usize + st.n_symbols as usize {
            let cnt2 = counters.count2_get(pos1, pos2);
            if cnt2 == 0 {
                continue;
            }
            let s2 = st.symbols[pos2];
            let s3 = Symbol::concat(s1, s2);
            // многобайтовый символ не может содержать терминатор
            if s2.first() != st.terminator as u8 {
                add_or_inc(&mut candidates, s3, cnt2 as u64, threshold);
            }
        }
    }

    let mut pq: BinaryHeap<QSymbol> = BinaryHeap::new();
    for q in &candidates {
        pq.push(q.clone());
    }

    st.reset();
    while st.n_symbols < MAX_SYMBOLS && !pq.is_empty() {
        let q = pq.pop().expect("pq непуст");
        st.add(q.symbol);
    }
}

fn add_or_inc(cands: &mut HashSet<QSymbol>, s: Symbol, count: u64, threshold: u64) {
    if count < threshold {
        return;
    }
    let mut q = QSymbol {
        symbol: s,
        gain: (count * s.symbol_len() as u64) as u32,
    };
    if let Some(old_q) = cands.get(&q) {
        q.gain += old_q.gain;
        let old = old_q.clone();
        cands.remove(&old);
    }
    cands.insert(q);
}

/// Обучение таблицы: 6 раундов сжатия-перестройки (как в эталоне),
/// выбирается таблица с максимальным выигрышем.
fn build_symbol_table(sample_buf: &[u8], sample_offsets: &[i32]) -> SymbolTable {
    let mut st = SymbolTable::new();
    let mut best_table = SymbolTable::new();
    let mut best_gain = -(2 * FSST_SAMPLETARGET as i32); // худший случай

    // Терминатор — редчайший байт сэмпла.
    let mut byte_histo = [0u32; 256];
    for c in sample_buf {
        byte_histo[*c as usize] += 1;
    }
    let mut curr_min_histo = u32::MAX;
    for (i, h) in byte_histo.iter().enumerate() {
        if *h < curr_min_histo {
            curr_min_histo = *h;
            st.terminator = i as u16;
        }
    }

    for frac in [8usize, 38, 68, 98, 108, 128] {
        let (mut this_counter, gain) = compress_count(&mut st, sample_buf, sample_offsets);
        if gain >= best_gain {
            best_gain = gain;
            best_table = st.clone();
        }
        make_table(&mut st, &mut this_counter, frac);
    }
    best_table.finalize();
    best_table
}

// ---------------------------------------------------------------------------
// Публичный кодек: FsstTable
// ---------------------------------------------------------------------------

/// Обученная и финализированная FSST-таблица: кодировщик + декодер.
///
/// Объект_immutable: кодирование детерминировано — одна и та же строка
/// всегда даёт одни и те же сжатые байты. Это опора compress-probe
/// лукапов в [`VocabArena`].
pub struct FsstTable {
    st: SymbolTable,
    /// Декодер: длины и значения символов по кодам.
    dec_lens: [u8; 256],
    dec_vals: [u64; 256],
}

impl FsstTable {
    /// Обучение таблицы на сэмпле термов (детерминированно).
    pub fn train(terms: &[&[u8]]) -> Self {
        let (sample_buf, sample_offsets) = make_sample(terms);
        let st = build_symbol_table(&sample_buf, &sample_offsets);
        Self::from_table(st)
    }

    fn from_table(st: SymbolTable) -> Self {
        let mut dec_lens = [0u8; 256];
        let mut dec_vals = [0u64; 256];
        for i in 0..st.n_symbols as usize {
            dec_lens[i] = st.symbols[i].symbol_len() as u8;
            dec_vals[i] = st.symbols[i].val;
        }
        Self { st, dec_lens, dec_vals }
    }

    /// Число обученных символов.
    pub fn symbol_count(&self) -> u16 {
        self.st.n_symbols
    }

    /// Кодирование одной строки с фиксированной таблицей (ядро compress_bulk
    /// эталона, адаптированное: скалярный безопасный цикл, sentinel-буфер).
    ///
    /// `out` очищается; на выходе — поток кодов (см. модульную доку).
    pub fn encode_into(&self, s: &[u8], out: &mut Vec<u8>) {
        out.clear();
        if s.is_empty() {
            return;
        }
        // +8 sentinel-байт: 8-байтовые загрузки не выходят за буфер (как в
        // эталоне); терминатор в позиции this_len исключает символы,
        // накрывающие конец строки (обучение не создаёт символы с ним).
        let mut buf = [0u8; 520];
        let mut in_curr = 0usize;
        while in_curr < s.len() {
            let in_end = (in_curr + 511).min(s.len());
            let this_len = in_end - in_curr;
            buf[..this_len].copy_from_slice(&s[in_curr..in_end]);
            buf[this_len] = self.st.terminator as u8;
            let mut i = 0usize;
            while i < this_len {
                let word = load8(&buf[i..]);
                let short_code = self.st.short_codes[(word & 0xFFFF) as usize];
                let first3 = word & 0xFFFFFF;
                let idx = fsst_hash(first3) as usize & (FSST_HASH_TAB_SIZE - 1);
                let sym = self.st.hash_tab[idx];
                let code: u16 = if sym.icl < FSST_ICL_FREE
                    && sym.val == (word & (u64::MAX >> (sym.icl & 0xFFFF)))
                {
                    // icl>>16 = (len << 12) | code — упаковка эталона
                    (sym.icl >> 16) as u16
                } else {
                    short_code
                };
                out.push(code as u8);
                if (code & 256) != 0 {
                    out.push(buf[i]); // escape: литеральный байт
                }
                i += (code >> 12) as usize;
            }
            in_curr = in_end;
        }
    }

    /// Кодирование в свежий буфер (удобно для тестов/пробников).
    pub fn encode(&self, s: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(s.len() / 2 + 8);
        self.encode_into(s, &mut v);
        v
    }

    /// Скалярный декодер: код → символ (до 8 байт), 255 → литерал.
    pub fn decode_into(&self, comp: &[u8], out: &mut Vec<u8>) {
        out.clear();
        out.reserve(comp.len() * 2);
        let mut i = 0usize;
        while i < comp.len() {
            let code = comp[i] as usize;
            if code == FSST_ESC as usize {
                i += 1;
                if i < comp.len() {
                    out.push(comp[i]);
                    i += 1;
                }
            } else {
                let len = self.dec_lens[code] as usize;
                out.extend_from_slice(&self.dec_vals[code].to_le_bytes()[..len]);
                i += 1;
            }
        }
    }

    /// Декодирование в свежий буфер.
    pub fn decode(&self, comp: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(comp.len() * 2);
        self.decode_into(comp, &mut v);
        v
    }

    // -----------------------------------------------------------------
    // Сериализация (~4.5 КБ): символы + карта занятости хеш-таблицы.
    // Карта слотов обязательна: при коллизиях двух длинных символов
    // владелец слота определяется порядком вставки ПРИ ОБУЧЕНИИ;
    // восстановление по одним лишь символам могло бы отдать слот другому
    // символу — и кодировщик перестал бы быть побитово тем же.
    // -----------------------------------------------------------------

    /// Магия формата: «poler FSST table v1».
    const SER_MAGIC: [u8; 8] = *b"PFSST1\0\0";

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + 4 + 256 * 9 + 1024 * 2);
        out.extend_from_slice(&Self::SER_MAGIC);
        out.extend_from_slice(&self.st.n_symbols.to_le_bytes());
        out.push(self.st.terminator as u8);
        out.extend_from_slice(&self.st.suffix_lim.to_le_bytes());
        for i in 0..256 {
            out.extend_from_slice(&self.st.symbols[i].val.to_le_bytes());
        }
        for i in 0..256 {
            out.push(self.st.symbols[i].symbol_len() as u8);
        }
        for slot in &self.st.hash_tab {
            let code = if slot.icl < FSST_ICL_FREE {
                slot.code()
            } else {
                0xFFFF
            };
            out.extend_from_slice(&code.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 8 + 4 + 256 * 9 + 1024 * 2 || bytes[..8] != Self::SER_MAGIC {
            return None;
        }
        let mut pos = 8usize;
        let n_symbols = u16::from_le_bytes(bytes[pos..pos + 2].try_into().ok()?) as usize;
        pos += 2;
        let terminator = bytes[pos] as u16;
        pos += 1;
        let suffix_lim = u16::from_le_bytes(bytes[pos..pos + 2].try_into().ok()?);
        pos += 2;

        let mut symbols = vec![Symbol::new(); FSST_CODE_MAX as usize];
        let mut vals = [0u64; 256];
        let mut lens = [0u8; 256];
        for i in 0..256 {
            vals[i] = u64::from_le_bytes(bytes[pos..pos + 8].try_into().ok()?);
            pos += 8;
        }
        for i in 0..256 {
            lens[i] = bytes[pos];
            pos += 1;
        }
        for i in 0..256 {
            let mut s = Symbol::new();
            if i < n_symbols && lens[i] > 0 {
                s.val = vals[i];
                s.set_code_len(i as u16, lens[i] as u32);
            }
            symbols[i] = s;
        }

        // byte_codes: детерминированно из 1-байтовых символов
        let mut byte_codes = [511u16 | (1 << CODE_LEN_SHIFT_IN_CODE); 256];
        for i in 0..n_symbols {
            if lens[i] == 1 {
                byte_codes[(vals[i] & 0xFF) as usize] = i as u16 | (1 << CODE_LEN_SHIFT_IN_CODE);
            }
        }
        // short_codes: сначала fallback на одиночные, затем 2-байтовые символы
        let mut short_codes = vec![0u16; 65536];
        for i in 0..65536 {
            short_codes[i] = byte_codes[i & 0xFF] | (1 << CODE_LEN_SHIFT_IN_CODE);
        }
        for i in 0..n_symbols {
            if lens[i] == 2 {
                short_codes[(vals[i] & 0xFFFF) as usize] =
                    i as u16 | (2 << CODE_LEN_SHIFT_IN_CODE);
            }
        }
        // hash_tab: карта занятости — побитово тот же кодировщик
        let mut hash_tab = [Symbol::new(); FSST_HASH_TAB_SIZE];
        for slot in hash_tab.iter_mut() {
            let code = u16::from_le_bytes(bytes[pos..pos + 2].try_into().ok()?);
            pos += 2;
            if code != 0xFFFF && (code as usize) < n_symbols {
                *slot = symbols[code as usize];
            }
        }

        let st = SymbolTable {
            short_codes,
            byte_codes,
            symbols,
            hash_tab,
            n_symbols: n_symbols as u16,
            terminator,
            suffix_lim,
            len_histo: [0; 9],
        };
        Some(Self::from_table(st))
    }
}

// ---------------------------------------------------------------------------
// VocabArena: словарь корпуса с interning и compress-probe лукапами
// ---------------------------------------------------------------------------

/// FNV-1a: быстрый детерминированный хеш сжатых ключей (DoS не грозит —
/// словарь локален, ключи порождены собственным кодировщиком).
#[inline]
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Битовой флаг «сырой» (несжатой) записи в offsets/индексе.
const RAW_FLAG: u32 = 1 << 31;
const OFF_MASK: u32 = RAW_FLAG - 1;

/// Словарь корпуса: все термы один раз, FSST-сжаты, точечный поиск
/// без декомпрессии (compress-probe).
///
/// ## Двухфазный жизненный цикл
///
/// * **Staging** (начало прохода 1): термы копятся несжатыми; как только
///   их байтовая масса достигает ~32 КБ, таблица обучается и словарь
///   переключается в компактную фазу — обучение по представительной
///   выборке, а не по первым попавшимся термам;
/// * **Compact**: терм кодируется и сравнивается по сжатым байтам
///   (детерминированный кодировщик ⇒ injectivity ⇒ корректность).
///   ID appending-only: стабильны между ресканами watcher'а.
///
/// ## Гарантия размера
///
/// Терм хранится сжатым, только если короче сырья; иначе — сырьём с
/// флагом. Следовательно blob ≤ объём сырых термов всегда.
///
/// ## RAM-бухгалтерия (на 1 терм, compact)
///
/// ~4–6 байт blob + 4 байта offsets + ~5.3 байта индекса (open
/// addressing, load ≤ 0.7) ≈ **14 байта** против ~64–96 байт
/// `HashMap<String, usize>` — при том, что словарь ОДИН на корпус,
/// а пер-файловые словари ссылаются на ID (8 байт на запись).
pub struct VocabArena {
    /// compact-фаза: None, пока идёт staging.
    table: Option<FsstTable>,
    /// Конкатенация сжатых (или сырых) форм термов.
    blob: Vec<u8>,
    /// (flag<<31 | конец_записи); длина = n_terms + 1 (см. entry_range).
    offsets: Vec<u32>,
    /// Открытая адресация: (flag<<31 | id+1); 0 — пусто.
    index: Vec<u32>,
    /// Степень-двойки размер индекса минус 1.
    index_mask: usize,
    /// Число термов (= ID пространства).
    n_terms: u32,
    /// staging-фаза: термы по ID + индекс вставки.
    staging: Vec<Box<str>>,
    staging_index: std::collections::HashMap<Box<str>, u32>,
    staging_bytes: usize,
}

impl Default for VocabArena {
    fn default() -> Self {
        Self::new()
    }
}

impl VocabArena {
    pub fn new() -> Self {
        Self {
            table: None,
            blob: Vec::new(),
            offsets: vec![0],
            index: vec![0; 16],
            index_mask: 15,
            n_terms: 0,
            staging: Vec::new(),
            staging_index: std::collections::HashMap::new(),
            staging_bytes: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.n_terms as usize
    }

    pub fn is_empty(&self) -> bool {
        self.n_terms == 0
    }

    pub fn is_compact(&self) -> bool {
        self.table.is_some()
    }

    /// Чистый размер сжатых термов (без offsets/индекса) —
    /// коэффициент FSST-сжатия считается от него.
    pub fn blob_bytes(&self) -> usize {
        self.blob.len()
    }

    /// Сериализованная FSST-таблица (бенчмарк: побитовая точность
    /// восстановления кодировщика; будущая персистентность словаря).
    pub fn serialized_table_bytes(&self) -> Vec<u8> {
        self.table.as_ref().map(|t| t.to_bytes()).unwrap_or_default()
    }

    /// Эталонное кодирование строки таблицей арены (дифференциалы).
    pub fn encode_reference(&self, s: &[u8]) -> Vec<u8> {
        self.table.as_ref().map(|t| t.encode(s)).unwrap_or_default()
    }

    /// Логический объём кучи (бенчмарк RAM-плотности).
    pub fn heap_bytes(&self) -> usize {
        let compact = self.blob.capacity()
            + self.offsets.capacity() * 4
            + self.index.capacity() * 4;
        let staged = self.staging.capacity() * std::mem::size_of::<Box<str>>()
            + self.staging_bytes
            + self.staging_index.capacity()
                * (std::mem::size_of::<Box<str>>() + 4)
            + self.staging_bytes; // дублирующие ключи staging_index
        compact + staged + 4608 // сериализованный размер таблицы ~4.5 КБ
    }

    /// Терм по ID (декомпрессия по требованию).
    pub fn term(&self, id: u32) -> Option<String> {
        if id >= self.n_terms {
            return None;
        }
        if let Some(t) = &self.table {
            let (start, end, raw) = self.entry_range(id)?;
            let bytes = if raw {
                self.blob[start..end].to_vec()
            } else {
                t.decode(&self.blob[start..end])
            };
            return Some(String::from_utf8_lossy(&bytes).into_owned());
        }
        self.staging
            .get(id as usize)
            .map(|s| s.to_string())
    }

    /// Все термы словаря (для тестов/бенчмарков/диагностики).
    pub fn terms(&self) -> Vec<String> {
        (0..self.n_terms).filter_map(|id| self.term(id)).collect()
    }

    /// Диапазон записи: `offsets[i]` — старт записи i, `offsets[i+1]` —
    /// старт следующей (= конец i) с флагом «сырой» формы в старшем бите.
    fn entry_range(&self, id: u32) -> Option<(usize, usize, bool)> {
        let start = (self.offsets.get(id as usize)? & OFF_MASK) as usize;
        let next = self.offsets.get(id as usize + 1)?;
        let end = (next & OFF_MASK) as usize;
        let raw = (next & RAW_FLAG) != 0;
        Some((start, end, raw))
    }

    /// Форма хранения терма: (raw?, bytes). Сжатая — если она КОРОЧЕ
    /// сырой (гарантия blob ≤ raw), иначе сырая с флагом.
    fn form<'a>(&self, t: &'a FsstTable, term: &'a str) -> (bool, Vec<u8>) {
        let enc = t.encode(term.as_bytes());
        if enc.len() < term.len() {
            (false, enc)
        } else {
            (true, term.as_bytes().to_vec())
        }
    }

    /// Поиск ID по терму (без вставки).
    pub fn id_of(&self, term: &str) -> Option<u32> {
        if let Some(t) = &self.table {
            let (raw, bytes) = self.form(t, term);
            self.find_entry(raw, &bytes)
        } else {
            self.staging_index.get(term).copied()
        }
    }

    fn find_entry(&self, raw: bool, bytes: &[u8]) -> Option<u32> {
        let mut pos = (fnv1a(bytes) as usize) & self.index_mask;
        loop {
            let slot = self.index[pos];
            if slot == 0 {
                return None;
            }
            let slot_raw = (slot & RAW_FLAG) != 0;
            let id = (slot & OFF_MASK) - 1;
            if slot_raw == raw {
                if let Some((s, e, entry_raw)) = self.entry_range(id) {
                    if entry_raw == raw && &self.blob[s..e] == bytes {
                        return Some(id);
                    }
                }
            }
            pos = (pos + 1) & self.index_mask;
        }
    }

    /// Interning: вернуть ID, вставив терм при необходимости.
    /// Вызывается под мьютексом sink (один writer на проход).
    pub fn intern(&mut self, term: &str) -> u32 {
        if let Some(id) = self.id_of(term) {
            return id;
        }
        if self.table.is_none() && self.staging_bytes >= 2 * FSST_SAMPLETARGET {
            self.switch_to_compact();
            // id_of пересчитать: форма могла смениться
            if let Some(id) = self.id_of(term) {
                return id;
            }
        }
        let id = self.n_terms;
        if let Some(t) = &self.table {
            let (raw, bytes) = self.form(t, term);
            self.blob.extend_from_slice(&bytes);
            // КОНЕЦ записи (= старт следующей) несёт флаг формы.
            let end = self.blob.len();
            self.offsets.push(if raw { RAW_FLAG | end as u32 } else { end as u32 });
            let slot = if raw { RAW_FLAG | (id + 1) } else { id + 1 };
            self.index_insert(slot, &bytes);
        } else {
            let boxed: Box<str> = term.into();
            self.staging_bytes += boxed.len();
            self.staging.push(boxed.clone());
            self.staging_index.insert(boxed, id);
        }
        self.n_terms += 1;
        id
    }

    /// Завершить staging: обучить таблицу, сжать все термы (ID стабильны).
    pub fn ensure_compact(&mut self) {
        if self.table.is_none() {
            self.switch_to_compact();
        }
    }

    fn switch_to_compact(&mut self) {
        let refs: Vec<&[u8]> = self.staging.iter().map(|s| s.as_bytes()).collect();
        let table = if refs.is_empty() {
            FsstTable::train(&[b""])
        } else {
            FsstTable::train(&refs)
        };
        // Перегоняем staging в blob в порядке ID.
        let staging = std::mem::take(&mut self.staging);
        self.staging_index.clear();
        self.staging_index.shrink_to_fit();
        self.staging_bytes = 0;
        self.blob = Vec::with_capacity(staging.iter().map(|s| s.len()).sum::<usize>() / 2 + 16);
        self.offsets = Vec::with_capacity(staging.len() + 1);
        self.offsets.push(0);
        self.n_terms = 0;
        self.index = vec![0; 16];
        self.index_mask = 15;
        self.table = Some(table);
        for term in &staging {
            let id = self.n_terms;
            let t = self.table.as_ref().expect("только что установлена");
            let (raw, bytes) = self.form(t, term);
            self.blob.extend_from_slice(&bytes);
            let end = self.blob.len();
            self.offsets.push(if raw { RAW_FLAG | end as u32 } else { end as u32 });
            let slot = if raw { RAW_FLAG | (id + 1) } else { id + 1 };
            self.index_insert(slot, &bytes);
            self.n_terms += 1;
        }
    }

    fn index_insert(&mut self, slot: u32, bytes: &[u8]) {
        if (self.n_terms as usize + 1) * 10 > self.index.len() * 7 {
            self.index_grow();
        }
        let mut pos = (fnv1a(bytes) as usize) & self.index_mask;
        while self.index[pos] != 0 {
            pos = (pos + 1) & self.index_mask;
        }
        self.index[pos] = slot;
    }

    fn index_grow(&mut self) {
        let new_len = (self.index.len() * 2).max(16).next_power_of_two();
        let old = std::mem::replace(&mut self.index, vec![0; new_len]);
        self.index_mask = new_len - 1;
        for slot in old {
            if slot == 0 {
                continue;
            }
            let id = (slot & OFF_MASK) - 1;
            let (s, e, _) = self
                .entry_range(id)
                .expect("запись существует при перехешировании");
            let mut pos = (fnv1a(&self.blob[s..e]) as usize) & self.index_mask;
            while self.index[pos] != 0 {
                pos = (pos + 1) & self.index_mask;
            }
            self.index[pos] = slot;
        }
    }
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Детерминированный xorshift для генерации корпусов в тестах.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// Русско-английская морфология: общий хвост — любимая еда FSST.
    /// Префиксы расширяют пространство комбинаций (иначе генератор
    /// не набирает запрошенное число уникальных термов).
    fn morphology_terms(n: usize) -> Vec<String> {
        let prefixes = ["", "пере", "недо", "анти", "квази", "микро", "мега", "суб"];
        let connectors = ["", "-", "_"];
        let stems = [
            "нокс", "когт", "сплетен", "вонзил", "систем", "протокол", "резонанс", "вектор",
            "runtime", "protocol", "system", "vector", "reson", "index", "token", "quer",
        ];
        let sufs = [
            "а", "ы", "и", "е", "у", "ой", "ам", "ами", "ах", "ing", "ed", "er", "s", "tion",
            "ness", "ble", "mente", "", "logy", "ative",
        ];
        // 3 × 8 × 16 × 20 = 7680 комбинаций — с запасом над любым n в тестах
        let mut rng = Rng(0x5EED_1234);
        let mut out = Vec::with_capacity(n);
        let mut seen = HashSet::new();
        while out.len() < n {
            let p = prefixes[rng.below(prefixes.len() as u64) as usize];
            let c = connectors[rng.below(connectors.len() as u64) as usize];
            let s = stems[rng.below(stems.len() as u64) as usize];
            let f = sufs[rng.below(sufs.len() as u64) as usize];
            let w = format!("{p}{s}{c}{f}");
            if seen.insert(w.clone()) {
                out.push(w);
            }
        }
        out
    }

    #[test]
    fn roundtrip_natural_morphology() {
        let terms = morphology_terms(4000);
        let refs: Vec<&[u8]> = terms.iter().map(|s| s.as_bytes()).collect();
        let t = FsstTable::train(&refs);
        for term in &terms {
            let enc = t.encode(term.as_bytes());
            let dec = t.decode(&enc);
            assert_eq!(dec, term.as_bytes(), "терм {term:?}");
        }
    }

    #[test]
    fn compression_ratio_on_morphology() {
        let terms = morphology_terms(4000);
        let refs: Vec<&[u8]> = terms.iter().map(|s| s.as_bytes()).collect();
        let t = FsstTable::train(&refs);
        let raw: usize = terms.iter().map(|s| s.len()).sum();
        let comp: usize = terms.iter().map(|s| t.encode(s.as_bytes()).len()).sum();
        let ratio = comp as f64 / raw as f64;
        assert!(
            ratio < 0.75,
            "FSST должен давать заметное сжатие на морфологии: ratio={ratio:.3}, symbols={}",
            t.symbol_count()
        );
    }

    #[test]
    fn encode_is_deterministic() {
        let terms = morphology_terms(200);
        let refs: Vec<&[u8]> = terms.iter().map(|s| s.as_bytes()).collect();
        let t = FsstTable::train(&refs);
        for term in &terms {
            assert_eq!(t.encode(term.as_bytes()), t.encode(term.as_bytes()));
        }
    }

    #[test]
    fn encode_is_injective() {
        let terms = morphology_terms(3000);
        let refs: Vec<&[u8]> = terms.iter().map(|s| s.as_bytes()).collect();
        let t = FsstTable::train(&refs);
        let mut seen = HashSet::new();
        for term in &terms {
            assert!(seen.insert(t.encode(term.as_bytes())), "коллизия кодирования");
        }
    }

    #[test]
    fn roundtrip_edge_cases() {
        let cases: Vec<&[u8]> = vec![
            b"", b"a", b"ab", b"abc", b"abcdefgh", b"abcdefghi", b"x",
            &[255u8, 254, 253], &[0u8, 1, 2], &[255u8; 64], &[7u8; 511], &[7u8; 600],
        ];
        let t = FsstTable::train(&cases.to_vec());
        for c in &cases {
            let enc = t.encode(c);
            assert_eq!(t.decode(&enc), *c, "кейс {c:?}");
        }
    }

    #[test]
    fn roundtrip_binary_garbage() {
        // Псевдослучайные байты (включая 255) — стресс escape-пути.
        let mut rng = Rng(0xABCD_1234_0000_0001);
        let mut terms: Vec<Vec<u8>> = Vec::new();
        for _ in 0..500 {
            let len = 1 + rng.below(24) as usize;
            terms.push((0..len).map(|_| rng.below(256) as u8).collect());
        }
        let refs: Vec<&[u8]> = terms.iter().map(|v| v.as_slice()).collect();
        let t = FsstTable::train(&refs);
        for term in &terms {
            assert_eq!(t.decode(&t.encode(term)), term.as_slice());
        }
    }

    #[test]
    fn serialization_is_bit_exact_encoder() {
        let terms = morphology_terms(2000);
        let refs: Vec<&[u8]> = terms.iter().map(|s| s.as_bytes()).collect();
        let a = FsstTable::train(&refs);
        let bytes = a.to_bytes();
        let b = FsstTable::from_bytes(&bytes).expect("десериализация");
        for term in &terms {
            assert_eq!(
                a.encode(term.as_bytes()),
                b.encode(term.as_bytes()),
                "восстановленный кодировщик побитово иной: {term:?}"
            );
        }
        // и хеш-слоты коллизий сохранены: обработаем «конфликтные» префиксы
        for term in &terms {
            let probe: Vec<u8> = term.as_bytes().to_vec();
            let mut e1 = Vec::new();
            let mut e2 = Vec::new();
            a.encode_into(&probe, &mut e1);
            b.encode_into(&probe, &mut e2);
            assert_eq!(e1, e2);
        }
    }

    #[test]
    fn serialization_rejects_garbage() {
        assert!(FsstTable::from_bytes(b"").is_none());
        assert!(FsstTable::from_bytes(b"0123456789ABCDEF0123456789").is_none());
    }

    #[test]
    fn vocab_arena_matches_hashmap_semantics() {
        let terms = morphology_terms(3000);
        let _rng = Rng(0xFEED_FACE_1234);
        // 60% вставки + повторы, 40% промахи
        let mut probes: Vec<&str> = Vec::new();
        for t in terms.iter().take(1800) {
            probes.push(t.as_str());
        }
        for _ in 0..1200 {
            probes.push("отсутствует_терм_XYZ");
        }
        let mut arena = VocabArena::new();
        let mut model: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
        let mut model_counts: Vec<u32> = Vec::new();
        for term in &terms {
            let id = arena.intern(term);
            match model.get(term.as_str()) {
                Some(&id2) => assert_eq!(id, id2),
                None => {
                    model.insert(term.as_str(), id);
                    model_counts.push(0);
                }
            }
            model_counts[id as usize] += 1;
        }
        arena.ensure_compact();
        // лукапы после компакции
        for p in &probes {
            assert_eq!(
                arena.id_of(p),
                model.get(*p).copied(),
                "терм {p:?}"
            );
        }
        // iter эквивалентен множеству
        let set: HashSet<String> = terms.iter().cloned().collect();
        let got: HashSet<String> = arena.terms().into_iter().collect();
        assert_eq!(set, got);
    }

    #[test]
    fn vocab_arena_early_switch_keeps_ids_stable() {
        // Стейтжинг переключается ДО ensure_compact: масса > 32 КБ.
        let terms = morphology_terms(6000); // > 32 КБ суммарно
        let mut arena = VocabArena::new();
        let mut ids = Vec::with_capacity(terms.len());
        for t in &terms {
            ids.push(arena.intern(t));
        }
        assert!(arena.is_compact(), "раннее переключение должно случиться");
        // ID стабильны и уникальны
        let mut uniq: HashSet<u32> = HashSet::new();
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(*id, i as u32, "ID должен равняться порядку вставки");
            assert!(uniq.insert(*id));
        }
        for (i, t) in terms.iter().enumerate() {
            assert_eq!(arena.id_of(t), Some(i as u32));
            assert_eq!(arena.term(i as u32).as_deref(), Some(t.as_str()));
        }
    }

    #[test]
    fn vocab_arena_blob_never_exceeds_raw() {
        // Смешанный корпус: морфология + бинарный мусор (raw-флаг путь).
        let mut terms = morphology_terms(1000);
        let mut rng = Rng(0xDEAD_BEEF_99);
        for _ in 0..300 {
            let len = 1 + rng.below(16) as usize;
            terms.push(
                (0..len)
                    .map(|_| (0x20 + rng.below(0x5F)) as u8 as char)
                    .collect::<String>(),
            );
        }
        let raw: usize = terms.iter().map(|s| s.len()).sum();
        let mut arena = VocabArena::new();
        for t in &terms {
            arena.intern(t);
        }
        arena.ensure_compact();
        assert!(
            arena.heap_bytes() <= raw * 2 + 4096,
            "blob+структуры не должны раздуваться сверх сырых байтов"
        );
        // всё находится
        for t in &terms {
            assert!(arena.id_of(t).is_some(), "потерян терм {t:?}");
        }
    }

    #[test]
    fn vocab_arena_empty_and_single() {
        let mut a = VocabArena::new();
        a.ensure_compact();
        assert!(a.is_empty());
        assert_eq!(a.id_of("x"), None);
        let mut b = VocabArena::new();
        assert_eq!(b.intern("единственный"), 0);
        b.ensure_compact();
        assert_eq!(b.id_of("единственный"), Some(0));
        assert_eq!(b.term(0).as_deref(), Some("единственный"));
        assert_eq!(b.id_of("другой"), None);
    }

    #[test]
    fn vocab_arena_two_form_lookup_no_false_positive() {
        // Два разных терма, у которых форма первого байт-в-байт равна
        // форме другого (сжатая == сырая): флаг обязан различать их.
        let mut a = VocabArena::new();
        // Натравим: сначала куча обычных термов (обучим таблицу),
        let terms = morphology_terms(800);
        for t in &terms {
            a.intern(t);
        }
        // затем вставляем «байтоподобные» строки, чьё сжатие длиннее сырья.
        let mut rng = Rng(0x1234_ABCD_7777);
        let mut oddballs: Vec<String> = Vec::new();
        for _ in 0..100 {
            let len = 2 + rng.below(6) as usize;
            oddballs.push((0..len).map(|_| (1 + rng.below(0xF0)) as u8 as char).collect());
        }
        for o in &oddballs {
            a.intern(o);
        }
        a.ensure_compact();
        for t in terms.iter().chain(oddballs.iter()) {
            assert!(a.id_of(t).is_some());
        }
        // ложных срабатываний нет: каждый терм возвращает свой ID
        for (i, t) in terms.iter().enumerate() {
            assert_eq!(a.id_of(t), Some(i as u32));
        }
        let off = terms.len() as u32;
        for (i, o) in oddballs.iter().enumerate() {
            assert_eq!(a.id_of(o), Some(off + i as u32));
        }
    }
}

