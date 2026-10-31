// ============================================================================
// poler-api v1.0.0 — AI-ШЛЮЗ К POLER-ENGINE (самодокументируемая поверхность)
// ----------------------------------------------------------------------------
// Проблема, которую решает шлюз:
//   * poler-engine v0.61.0 имеет 250 CLI-флагов (947 строк --help) — модель
//     не может discover'ить возможности; скилл-док обещает 6 MCP-инструментов,
//     --help врёт про 8, а живой MCP-сервер отдаёт 32 (calc/quantum/neural/
//     fly/ssn/literary скрыты).
//   * eigen калькулятора ломается на n>=28 (взрыв коэффициентов charpoly,
//     феномен Уилкинсона). Штурм-бисенция расширяет границу до 256+.
//
// Архитектура: ФАСАД (подкоманды + schema) + MCP-ПРОКСИ (32 инструмента
// движка насквозь + 2 нативных) + НАТИВНОЕ ШТУРМ-ЯДРО (LU / Хаусхолдер /
// Стurm-бисекция, без характеристического полинома).
//
// Компиляция: rustc -O poler-api.rs -o poler-api  (чистый std, ноль крейтов)
// ============================================================================

use std::env;
use std::fs;
use std::io::{self, Write, BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

const ENGINE_BIN: &str = "poler-engine";
const API_VERSION: &str = "1.0.0";

// ============================================================================
// МИНИ-JSON: парсер + сериализатор (стандартная библиотека не умеет JSON)
// ============================================================================

#[derive(Clone, Debug)]
enum J {
    Nul,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn s(v: &str) -> J { J::Str(v.to_string()) }
    fn n(v: f64) -> J { J::Num(v) }
    fn b(v: bool) -> J { J::Bool(v) }

    fn get(&self, key: &str) -> Option<&J> {
        if let J::Obj(m) = self {
            m.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        } else { None }
    }
    fn as_str(&self) -> Option<&str> {
        if let J::Str(s) = self { Some(s) } else { None }
    }
    fn as_f64(&self) -> Option<f64> {
        if let J::Num(x) = self { Some(*x) } else { None }
    }
    fn as_bool(&self) -> Option<bool> {
        if let J::Bool(x) = self { Some(*x) } else { None }
    }
    fn as_arr(&self) -> Option<&Vec<J>> {
        if let J::Arr(a) = self { Some(a) } else { None }
    }
    fn is_nul(&self) -> bool { matches!(self, J::Nul) }

    fn dump(&self) -> String { json_dump(self, 0) }
}

fn json_escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn fmt_num(x: f64) -> String {
    if x.is_nan() || x.is_infinite() { return "null".to_string(); }
    if x == x.trunc() && x.abs() < 9.0e15 {
        format!("{}", x as i64)
    } else {
        format!("{}", x) // Rust Display = кратчайший round-trip, без экспоненты
    }
}

fn json_dump(j: &J, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let pad2 = "  ".repeat(indent + 1);
    match j {
        J::Nul => "null".into(),
        J::Bool(b) => if *b { "true".into() } else { "false".into() },
        J::Num(x) => fmt_num(*x),
        J::Str(s) => { let mut o = String::new(); json_escape(s, &mut o); o }
        J::Arr(a) => {
            if a.is_empty() { return "[]".into(); }
            // числовые (и короткие строковые) массивы — компактно в одну строку
            let all_num = a.iter().all(|v| matches!(v, J::Num(_)));
            let small_strs = a.len() <= 16 && a.iter().all(|v| matches!(v, J::Str(s) if s.len() <= 28));
            if all_num || small_strs {
                let parts: Vec<String> = a.iter().map(|v| json_dump(v, 0)).collect();
                return format!("[{}]", parts.join(", "));
            }
            let mut o = String::from("[\n");
            for (i, v) in a.iter().enumerate() {
                o.push_str(&pad2);
                o.push_str(&json_dump(v, indent + 1));
                if i + 1 < a.len() { o.push(','); }
                o.push('\n');
            }
            o.push_str(&pad); o.push(']'); o
        }
        J::Obj(m) => {
            if m.is_empty() { return "{}".into(); }
            let mut o = String::from("{\n");
            for (i, (k, v)) in m.iter().enumerate() {
                o.push_str(&pad2);
                let mut ks = String::new(); json_escape(k, &mut ks);
                o.push_str(&ks);
                o.push_str(": ");
                o.push_str(&json_dump(v, indent + 1));
                if i + 1 < m.len() { o.push(','); }
                o.push('\n');
            }
            o.push_str(&pad); o.push('}'); o
        }
    }
}

fn json_compact(j: &J) -> String {
    // ОДНОСТРОЧНЫЙ JSON — для JSON-RPC фреймов (транспорт движка построчный!)
    match j {
        J::Nul => "null".into(),
        J::Bool(b) => if *b { "true".into() } else { "false".into() },
        J::Num(x) => fmt_num(*x),
        J::Str(s) => { let mut o = String::new(); json_escape(s, &mut o); o }
        J::Arr(a) => {
            let parts: Vec<String> = a.iter().map(json_compact).collect();
            format!("[{}]", parts.join(","))
        }
        J::Obj(m) => {
            let parts: Vec<String> = m.iter().map(|(k, v)| {
                let mut ks = String::new(); json_escape(k, &mut ks);
                format!("{}:{}", ks, json_compact(v))
            }).collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

struct JParser<'a> { b: &'a [u8], i: usize }

fn jparse(s: &str) -> Result<J, String> {
    let mut p = JParser { b: s.as_bytes(), i: 0 };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() { return Err(format!("JSON: мусор на позиции {}", p.i)); }
    Ok(v)
}

impl<'a> JParser<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<u8> { self.b.get(self.i).copied() }
    fn value(&mut self) -> Result<J, String> {
        match self.peek() {
            None => Err("JSON: внезапный конец".into()),
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(J::Str(self.string()?)),
            Some(b't') => self.lit("true", J::Bool(true)),
            Some(b'f') => self.lit("false", J::Bool(false)),
            Some(b'n') => self.lit("null", J::Nul),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            Some(c) => Err(format!("JSON: неожиданный символ '{}'", c as char)),
        }
    }
    fn lit(&mut self, word: &str, v: J) -> Result<J, String> {
        if self.b[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len(); Ok(v)
        } else { Err(format!("JSON: ожидалось '{}'", word)) }
    }
    fn object(&mut self) -> Result<J, String> {
        self.i += 1; // '{'
        let mut m = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') { self.i += 1; return Ok(J::Obj(m)); }
        loop {
            self.ws();
            if self.peek() != Some(b'"') { return Err("JSON: ожидался ключ-строка".into()); }
            let k = self.string()?;
            self.ws();
            if self.peek() != Some(b':') { return Err("JSON: ожидалось ':'".into()); }
            self.i += 1;
            self.ws();
            let v = self.value()?;
            m.push((k, v));
            self.ws();
            match self.peek() {
                Some(b',') => { self.i += 1; }
                Some(b'}') => { self.i += 1; return Ok(J::Obj(m)); }
                _ => return Err("JSON: ожидалось ',' или '}'".into()),
            }
        }
    }
    fn array(&mut self) -> Result<J, String> {
        self.i += 1; // '['
        let mut a = Vec::new();
        self.ws();
        if self.peek() == Some(b']') { self.i += 1; return Ok(J::Arr(a)); }
        loop {
            self.ws();
            let v = self.value()?;
            a.push(v);
            self.ws();
            match self.peek() {
                Some(b',') => { self.i += 1; }
                Some(b']') => { self.i += 1; return Ok(J::Arr(a)); }
                _ => return Err("JSON: ожидалось ',' или ']'".into()),
            }
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.i += 1; // '"'
        let mut o = String::new();
        loop {
            match self.peek() {
                None => return Err("JSON: незакрытая строка".into()),
                Some(b'"') => { self.i += 1; return Ok(o); }
                Some(b'\\') => {
                    self.i += 1;
                    match self.peek() {
                        Some(b'"') => { o.push('"'); self.i += 1; }
                        Some(b'\\') => { o.push('\\'); self.i += 1; }
                        Some(b'/') => { o.push('/'); self.i += 1; }
                        Some(b'n') => { o.push('\n'); self.i += 1; }
                        Some(b't') => { o.push('\t'); self.i += 1; }
                        Some(b'r') => { o.push('\r'); self.i += 1; }
                        Some(b'b') => { o.push('\u{08}'); self.i += 1; }
                        Some(b'f') => { o.push('\u{0c}'); self.i += 1; }
                        Some(b'u') => {
                            self.i += 1;
                            let cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp) {
                                // суррогатная пара
                                if self.peek() == Some(b'\\') {
                                    self.i += 1;
                                    if self.peek() == Some(b'u') {
                                        self.i += 1;
                                        let lo = self.hex4()?;
                                        let c = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                        o.push(char::from_u32(c as u32).unwrap_or('\u{FFFD}'));
                                    } else { o.push('\u{FFFD}'); }
                                } else { o.push('\u{FFFD}'); }
                            } else {
                                o.push(char::from_u32(cp as u32).unwrap_or('\u{FFFD}'));
                            }
                        }
                        _ => return Err("JSON: битый escape".into()),
                    }
                }
                Some(_) => {
                    // многобайтовый UTF-8: копируем весь символ
                    let start = self.i;
                    let len = utf8_len(self.b[start]);
                    if start + len > self.b.len() { return Err("JSON: битый UTF-8".into()); }
                    match std::str::from_utf8(&self.b[start..start + len]) {
                        Ok(s) => { o.push_str(s); self.i += len; }
                        Err(_) => return Err("JSON: битый UTF-8".into()),
                    }
                }
            }
        }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        if self.i + 4 > self.b.len() { return Err("JSON: битый \\u".into()); }
        let s = std::str::from_utf8(&self.b[self.i..self.i + 4])
            .map_err(|_| "JSON: битый \\u".to_string())?;
        let v = u32::from_str_radix(s, 16).map_err(|_| "JSON: битый \\u".to_string())?;
        self.i += 4;
        Ok(v)
    }
    fn number(&mut self) -> Result<J, String> {
        let start = self.i;
        if self.peek() == Some(b'-') { self.i += 1; }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == b'.' || c == b'e' || c == b'E' || c == b'+' || c == b'-' {
                self.i += 1;
            } else { break; }
        }
        let s = std::str::from_utf8(&self.b[start..self.i]).map_err(|_| "JSON: битое число".to_string())?;
        s.parse::<f64>().map(J::Num).map_err(|_| format!("JSON: битое число '{}'", s))
    }
}

fn utf8_len(b: u8) -> usize {
    if b < 0x80 { 1 } else if b >> 5 == 0b110 { 2 } else if b >> 4 == 0b1110 { 3 } else { 4 }
}

// хелперы сборки JSON
fn jobj(pairs: Vec<(&str, J)>) -> J {
    J::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}
fn jarr(a: Vec<J>) -> J { J::Arr(a) }

// ============================================================================
// НАТИВНОЕ МАТ-ЯДРО: без характеристического полинома => без Уилкинсона
//   * det: LU с частичным выбором ведущего (плотная) / континуанта (трёхdiag)
//   * eigen: Хаусхолдер -> трёхдиагонализация -> Штурм-бисекция
// ============================================================================

struct Tridiag { d: Vec<f64>, e: Vec<f64> } // diag[n], offdiag[n-1]

enum MatSpec {
    ToeplitzTridiag { diag: f64, off: f64, n: usize },
    Dense { a: Vec<f64>, n: usize }, // row-major
}

fn parse_mat_spec(spec: &str) -> Result<MatSpec, String> {
    let spec = spec.trim();
    if let Some(rest) = spec.strip_prefix("tridiag:") {
        let parts: Vec<&str> = rest.split(',').map(|s| s.trim()).collect();
        if parts.len() != 3 { return Err("tridiag: ожидаю DIAG,OFFDIAG,N (например tridiag:2,-1,256)".into()); }
        let diag: f64 = parts[0].parse().map_err(|_| "tridiag: DIAG не число")?;
        let off: f64 = parts[1].parse().map_err(|_| "tridiag: OFFDIAG не число")?;
        let n: usize = parts[2].parse().map_err(|_| "tridiag: N не целое")?;
        if n < 1 || n > 100_000 { return Err("tridiag: N вне диапазона 1..100000".into()); }
        Ok(MatSpec::ToeplitzTridiag { diag, off, n })
    } else if let Some(path) = spec.strip_prefix("file:") {
        let text = fs::read_to_string(path.trim())
            .map_err(|e| format!("file: не могу прочитать '{}': {}", path.trim(), e))?;
        parse_dense(&text).map(|(a, n)| MatSpec::Dense { a, n })
    } else if let Some(inline) = spec.strip_prefix("inline:") {
        parse_dense(inline).map(|(a, n)| MatSpec::Dense { a, n })
    } else {
        Err("источник матрицы: жду tridiag:DIAG,OFFDIAG,N | file:ПУТЬ | inline:a,b;c,d".into())
    }
}

fn parse_dense(text: &str) -> Result<(Vec<f64>, usize), String> {
    // строки: '\n' (файл) или ';' (inline); столбцы: ',' / пробел / таб
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for (ln, chunk) in text.split(|c| c == '\n' || c == ';').enumerate() {
        let line = chunk.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") { continue; }
        let toks: Vec<&str> = line.split(|c: char| c == ',' || c == ' ' || c == '\t')
            .map(|t| t.trim()).filter(|t| !t.is_empty()).collect();
        if toks.is_empty() { continue; }
        let row: Result<Vec<f64>, _> = toks.iter().map(|t| t.parse::<f64>()).collect();
        match row {
            Ok(r) => rows.push(r),
            Err(_) => return Err(format!("строка {}: нечисловой элемент", ln + 1)),
        }
    }
    let n = rows.len();
    if n == 0 { return Err("пустая матрица".into()); }
    for (i, r) in rows.iter().enumerate() {
        if r.len() != n {
            return Err(format!("матрица должна быть квадратной: строка {} имеет {} элементов (нужно {})", i + 1, r.len(), n));
        }
    }
    let mut a = Vec::with_capacity(n * n);
    for r in &rows { a.extend(r.iter().copied()); }
    Ok((a, n))
}

fn is_symmetric(a: &[f64], n: usize) -> bool {
    let mut max_abs = 0.0f64;
    for &x in a { max_abs = max_abs.max(x.abs()); }
    let tol = 1e-10 * max_abs.max(1.0);
    for i in 0..n {
        for j in (i + 1)..n {
            if (a[i * n + j] - a[j * n + i]).abs() > tol { return false; }
        }
    }
    true
}

// --- детерминанты ---

fn det_lu(a: &mut [f64], n: usize) -> f64 {
    // LU с частичным выбором ведущего; det = (-1)^s * prod(u_ii)
    let mut det = 1.0f64;
    let mut swaps = 0i64;
    for k in 0..n {
        // ведущий элемент
        let mut piv = k;
        let mut best = a[k * n + k].abs();
        for i in (k + 1)..n {
            let v = a[i * n + k].abs();
            if v > best { best = v; piv = i; }
        }
        if best == 0.0 { return 0.0; }
        if piv != k {
            for j in 0..n {
                a.swap(k * n + j, piv * n + j);
            }
            swaps += 1;
        }
        let piv_val = a[k * n + k];
        for i in (k + 1)..n {
            let f = a[i * n + k] / piv_val;
            if f != 0.0 {
                for j in (k + 1)..n {
                    a[i * n + j] -= f * a[k * n + j];
                }
            }
            a[i * n + k] = f;
        }
        det *= piv_val;
    }
    if swaps % 2 == 1 { -det } else { det }
}

fn det_tridiag_cont(t: &Tridiag) -> f64 {
    // континуанта: D_0=1, D_1=d_0, D_k = d_{k-1} D_{k-1} - e_{k-1}^2 D_{k-2}
    let n = t.d.len();
    if n == 0 { return 1.0; }
    let mut prev2 = 1.0f64;
    let mut prev1 = t.d[0];
    for i in 1..n {
        let cur = t.d[i] * prev1 - t.e[i - 1] * t.e[i - 1] * prev2;
        prev2 = prev1;
        prev1 = cur;
    }
    prev1
}

fn trace_dense(a: &[f64], n: usize) -> f64 {
    (0..n).map(|i| a[i * n + i]).sum()
}

// --- Хаусхолдер: симметричная A -> трёхдиагональная T (d, e) ---

fn householder_tridiag(a: &mut [f64], n: usize) -> Tridiag {
    let mut d = vec![0.0f64; n];
    let mut e = vec![0.0f64; n.max(1) - 1];
    if n == 0 { return Tridiag { d, e }; }
    if n == 1 { d[0] = a[0]; return Tridiag { d, e }; }
    for k in 0..n.saturating_sub(2) {
        let m = n - (k + 1); // размер хвостового блока
        // x = A[k+1..n, k]
        let mut v = vec![0.0f64; m];
        for (idx, i) in (k + 1..n).enumerate() { v[idx] = a[i * n + k]; }
        let xnorm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if xnorm == 0.0 { e[k] = 0.0; continue; }
        let alpha = if v[0] >= 0.0 { -xnorm } else { xnorm };
        v[0] -= alpha; // v = x - alpha*e1
        let vnorm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if vnorm < f64::MIN_POSITIVE { e[k] = alpha; continue; }
        for vi in v.iter_mut() { *vi /= vnorm; }
        // w = A_sub * v (хвостовый блок, симметричный)
        let mut w = vec![0.0f64; m];
        for j in 0..m {
            let mut acc = 0.0;
            for i in 0..m { acc += a[(k + 1 + i) * n + (k + 1 + j)] * v[i]; }
            w[j] = acc;
        }
        let s = v.iter().zip(w.iter()).map(|(a1, b1)| a1 * b1).sum::<f64>();
        // A' = A - 2 v w^T - 2 w v^T + 4 s v v^T
        for i in 0..m {
            for j in 0..m {
                a[(k + 1 + i) * n + (k + 1 + j)] +=
                    -2.0 * v[i] * w[j] - 2.0 * w[i] * v[j] + 4.0 * s * v[i] * v[j];
            }
        }
        e[k] = alpha; // поддиагональ после отражения: Px = alpha*e1
    }
    // финальный 2x2 хвост обрабатывается самой структурой
    for i in 0..n { d[i] = a[i * n + i]; }
    if n >= 2 { e[n - 2] = a[(n - 1) * n + (n - 2)]; }
    Tridiag { d, e }
}

// --- Штурм-бисекция ---

fn sturm_count(t: &Tridiag, x: f64) -> usize {
    // число собственных значений T, строго меньших x (LDL^T, счёт отрицательных)
    // ВАЖНО (семантика LAPACK dstebz): квазинулевой опорный ЗАМЕНЯЕТСЯ на -pivmin
    // ДО подсчёта знака — заменённый опорный отрицателен и считается.
    let n = t.d.len();
    let e2max = t.e.iter().map(|v| v * v).fold(0.0f64, f64::max).max(1.0);
    let pivmin = f64::MIN_POSITIVE * e2max;
    let mut count = 0usize;
    let mut q = t.d[0] - x;
    if q.abs() < pivmin { q = -pivmin; }
    if q < 0.0 { count += 1; }
    for i in 1..n {
        q = t.d[i] - x - t.e[i - 1] * t.e[i - 1] / q;
        if q.abs() < pivmin { q = -pivmin; }
        if q < 0.0 { count += 1; }
    }
    count
}

fn gershgorin(t: &Tridiag) -> (f64, f64) {
    let n = t.d.len();
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for i in 0..n {
        let mut r = 0.0;
        if i > 0 { r += t.e[i - 1].abs(); }
        if i + 1 < n { r += t.e[i].abs(); }
        lo = lo.min(t.d[i] - r);
        hi = hi.max(t.d[i] + r);
    }
    (lo, hi)
}

fn eigen_sturm_all(t: &Tridiag) -> Vec<f64> {
    // все собственные значения по возрастанию; k-я (0-индекс) — бисекцией
    let n = t.d.len();
    let (lo, hi) = gershgorin(t);
    let span = (hi - lo).abs().max(1e-300);
    let tol = (2.0e-14 * span).max(f64::MIN_POSITIVE * 4.0);
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let mut a = lo;
        let mut b = hi;
        // инвариант: count(a) <= k < count(b)  =>  lambda_k in (a, b]
        while b - a > tol {
            let m = 0.5 * (a + b);
            if m <= a || m >= b { break; } // исчерпано разрешение f64
            if sturm_count(t, m) <= k { a = m; } else { b = m; }
        }
        out.push(0.5 * (a + b));
    }
    out.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn prod_sorted(vals: &[f64]) -> f64 {
    // произведение по возрастанию модуля — минимум under/overflow
    let mut v: Vec<f64> = vals.to_vec();
    v.sort_by(|a, b| a.abs().partial_cmp(&b.abs()).unwrap_or(std::cmp::Ordering::Equal));
    let mut p = 1.0f64;
    for x in v { p *= x; }
    p
}

// Унифицированный вычислитель: источник -> {det, trace, eigen}
struct MatResult {
    n: usize,
    source: String,
    method_det: &'static str,
    method_eigen: &'static str,
    det: f64,
    trace: f64,
    eigen: Option<Vec<f64>>,
    prod_lambda: Option<f64>,
    sum_lambda: Option<f64>,
    symmetric: bool,
    note: String,
}

fn compute_matrix(spec: &str, want_eigen: bool) -> Result<MatResult, String> {
    match parse_mat_spec(spec)? {
        MatSpec::ToeplitzTridiag { diag, off, n } => {
            let t = Tridiag { d: vec![diag; n], e: vec![off; n - 1] };
            let det = det_tridiag_cont(&t);
            let trace = diag * n as f64;
            let eigen = if want_eigen { Some(eigen_sturm_all(&t)) } else { None };
            let prod_lambda = eigen.as_ref().map(|v| prod_sorted(v));
            let sum_lambda = eigen.as_ref().map(|v| v.iter().sum());
            Ok(MatResult {
                n, source: format!("tridiag({},{},{})", diag, off, n),
                method_det: "континуанта (O(n), точная)",
                method_eigen: "Штурм-бисекция (без charpoly)",
                det, trace, eigen, prod_lambda, sum_lambda,
                symmetric: true,
                note: "аналитика: lam_k = DIAG + 2*OFF*cos(k*pi/(n+1)); det = континуанта".into(),
            })
        }
        MatSpec::Dense { a, n } => {
            let symmetric = is_symmetric(&a, n);
            let mut a_lu = a.clone();
            let det = det_lu(&mut a_lu, n);
            let trace = trace_dense(&a, n);
            let (eigen, method_eigen, note) = if want_eigen {
                if symmetric {
                    let mut ac = a.clone();
                    let t = householder_tridiag(&mut ac, n);
                    (Some(eigen_sturm_all(&t)),
                     "Хаусхолдер + Штурм-бисекция",
                     format!("симметричная {}x{}: инварианты sum(lam)=trace, prod(lam)=det", n, n))
                } else {
                    (None, "недоступно",
                     "НЕсимметричная матрица: Штурм требует симметрии; eigen движка (Фаддеев-Дюран-Кернер) надёжен только до n<=24".into())
                }
            } else {
                (None, "-", String::new())
            };
            let prod_lambda = eigen.as_ref().map(|v| prod_sorted(v));
            let sum_lambda = eigen.as_ref().map(|v| v.iter().sum());
            Ok(MatResult {
                n, source: format!("dense {}x{}", n, n),
                method_det: "LU с частичным ведущим",
                method_eigen, det, trace, eigen, prod_lambda, sum_lambda,
                symmetric, note,
            })
        }
    }
}

// ============================================================================
// ДЕЛЕГАТЫ ДВИЖКА (единственный канал к poler-engine — его собственный CLI/MCP)
// ============================================================================

fn engine_version() -> String {
    match Command::new(ENGINE_BIN).arg("--version").output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => format!("{} <недоступен>", ENGINE_BIN),
    }
}

fn engine_available() -> bool {
    Command::new(ENGINE_BIN).arg("--version").output().is_ok()
}

/// Одноразовый --exec с JSON-конвертом: {cmd, ok, exit_code, duration_ms, output}
fn engine_exec(cmd: &str) -> Result<(bool, String, String), String> {
    let out = Command::new(ENGINE_BIN)
        .args(["--exec", cmd, "--json"])
        .output()
        .map_err(|e| format!("не могу запустить {}: {}", ENGINE_BIN, e))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let j = jparse(&text).map_err(|e| format!("движок вернул не-JSON ({}): {}", e, &text[..text.len().min(200)]))?;
    let ok = j.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let output = j.get("output").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let dur = j.get("duration_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
    Ok((ok, output, format!("{:.0}", dur)))
}

fn engine_calc(expr: &str) -> Result<String, String> {
    let (ok, output, _) = engine_exec(&format!("calc {}", expr))?;
    if !ok { return Err(format!("движок отклонил выражение: {}", output)); }
    Ok(output)
}

/// Живой запрос к MCP-серверу движка (initialize + method), ответ по id.
fn engine_mcp_request(method: &str, params: Option<J>) -> Result<J, String> {
    let mut child = Command::new(ENGINE_BIN)
        .arg("--mcp")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().map_err(|e| format!("не могу запустить {} --mcp: {}", ENGINE_BIN, e))?;
    let mut stdin = child.stdin.take().ok_or("нет stdin у движка")?;
    let stdout = child.stdout.take().ok_or("нет stdout у движка")?;
    let req = jobj(vec![
        ("jsonrpc", J::s("2.0")),
        ("id", J::n(1.0)),
        ("method", J::s(method)),
    ]).with_params(params);
    let init = jobj(vec![
        ("jsonrpc", J::s("2.0")),
        ("id", J::n(0.0)),
        ("method", J::s("initialize")),
        ("params", jobj(vec![
            ("protocolVersion", J::s("2024-11-05")),
            ("capabilities", J::Obj(vec![])),
            ("clientInfo", jobj(vec![("name", J::s("poler-api")), ("version", J::s(API_VERSION))])),
        ])),
    ]);
    {
        let mut w = io::BufWriter::new(&mut stdin);
        writeln!(w, "{}", json_compact(&init)).map_err(|e| e.to_string())?;
        writeln!(w, "{}", json_compact(&req)).map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())?;
        drop(w);
    }
    let reader = BufReader::new(stdout);
    let mut answer: Option<J> = None;
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() { continue; }
        if let Ok(j) = jparse(&line) {
            if j.get("id").and_then(|v| v.as_f64()) == Some(1.0) {
                answer = Some(j);
                break;
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    answer.ok_or_else(|| "движок не ответил на MCP-запрос".to_string())
}

trait JWithParams { fn with_params(self, p: Option<J>) -> J; }
impl JWithParams for J {
    fn with_params(mut self, p: Option<J>) -> J {
        if let Some(p) = p {
            if let J::Obj(ref mut m) = self { m.push(("params".into(), p)); }
        }
        self
    }
}

fn engine_mcp_tools() -> Result<Vec<J>, String> {
    let resp = engine_mcp_request("tools/list", None)?;
    resp.get("result").and_then(|r| r.get("tools"))
        .and_then(|t| t.as_arr().cloned())
        .ok_or_else(|| "движок не вернул tools".to_string())
}

// ============================================================================
// НАТИВНЫЕ MCP-ИНСТРУМЕНТЫ (то, чего нет в движке: Штурм без Уилкинсона)
// ============================================================================

fn native_mcp_tools() -> Vec<J> {
    vec![
        jobj(vec![
            ("name", J::s("poler_matrix_det")),
            ("description", J::s("Детерминант матрицы БЕЗ ограничения tridiag(n<=64) движка: Toeplitz-трёхдиагональ через точную континуанту O(n), плотная — LU с частичным ведущим. Источник source='tridiag:DIAG,OFFDIAG,N' | 'file:ПУТЬ.csv' | 'inline:1,2;3,4'.")),
            ("inputSchema", jobj(vec![
                ("type", J::s("object")),
                ("properties", jobj(vec![
                    ("source", jobj(vec![("type", J::s("string")), ("description", J::s("tridiag:2,-1,256 | file:/path/matrix.csv | inline:1,2;3,4"))])),
                ])),
                ("required", jarr(vec![J::s("source")])),
            ])),
        ]),
        jobj(vec![
            ("name", J::s("poler_matrix_eigen_sturm")),
            ("description", J::s("ВСЕ собственные значения симметричной матрицы методом Штурма-бисекции (нативное ядро poler-api, БЕЗ характеристического полинома). Не подвержен взрыву Уилкинсона: eigen движка надёжен до n<=24 и ЛОМАЕТСЯ на n>=28 (коэффициенты charpoly растут O(n!)); Штурм даёт машинную точность ~5e-13 при n=256..512. Плотная симметричная -> Хаусхолдер-трёхдиагонализация -> Штурм. verify=true сверяет prod(lambda) с det и sum(lambda) с trace.")),
            ("inputSchema", jobj(vec![
                ("type", J::s("object")),
                ("properties", jobj(vec![
                    ("source", jobj(vec![("type", J::s("string")), ("description", J::s("tridiag:2,-1,256 | file:/path/matrix.csv | inline:1,2;3,4"))])),
                    ("verify", jobj(vec![("type", J::s("boolean")), ("default", J::b(true)), ("description", J::s("сверка prod(lambda)=det, sum(lambda)=trace"))])),
                ])),
                ("required", jarr(vec![J::s("source")])),
            ])),
        ]),
    ]
}

fn is_native_tool(name: &str) -> bool {
    matches!(name, "poler_matrix_det" | "poler_matrix_eigen_sturm")
}

/// JSON-результат матричного вычисления (CLI и MCP получают одно и то же)
fn matrix_json(tool: &str, spec: &str, want_eigen: bool, verify: bool) -> Result<J, String> {
    let started = Instant::now();
    let r = compute_matrix(spec, want_eigen)?;
    let mut fields: Vec<(String, J)> = vec![
        ("ok".into(), J::b(true)),
        ("tool".into(), J::s(tool)),
        ("source".into(), J::Str(r.source.clone())),
        ("n".into(), J::n(r.n as f64)),
        ("symmetric".into(), J::b(r.symmetric)),
        ("det".into(), J::n(r.det)),
        ("det_method".into(), J::s(r.method_det)),
        ("trace".into(), J::n(r.trace)),
        ("eigen_method".into(), J::s(r.method_eigen)),
        ("note".into(), J::Str(r.note.clone())),
    ];
    // аналитический контроль для Toeplitz-трёхдиагонали: lam_k = D + 2*O*cos(k*pi/(n+1))
    if let MatSpec::ToeplitzTridiag { diag, off, n } = parse_mat_spec(spec)? {
        if let Some(ev) = &r.eigen {
            let mut max_err = 0.0f64;
            for (i, &lam) in ev.iter().enumerate() {
                let k = (i + 1) as f64;
                let exact = diag + 2.0 * off * (std::f64::consts::PI * k / (n as f64 + 1.0)).cos();
                max_err = max_err.max((lam - exact).abs());
            }
            fields.push(("analytic_check".into(), jobj(vec![
                ("formula".into(), J::s("lam_k = DIAG + 2*OFF*cos(k*pi/(n+1))")),
                ("max_abs_err".into(), J::n(max_err)),
            ])));
        }
    }
    if let Some(ev) = &r.eigen {
        let show: Vec<J> = if ev.len() > 1024 {
            ev.iter().take(1024).map(|x| J::n(*x)).collect()
        } else {
            ev.iter().map(|x| J::n(*x)).collect()
        };
        if ev.len() > 1024 {
            fields.push(("eigenvalues_truncated".into(), jobj(vec![
                ("shown".into(), J::n(1024.0)), ("total".into(), J::n(ev.len() as f64)),
            ])));
        }
        fields.push(("lambda_min".into(), J::n(*ev.first().unwrap())));
        fields.push(("lambda_max".into(), J::n(*ev.last().unwrap())));
        fields.push(("eigenvalues".into(), J::Arr(show)));
        if verify {
            let pl = r.prod_lambda.unwrap();
            let sl = r.sum_lambda.unwrap();
            let det_diff = (pl - r.det).abs();
            let trace_diff = (sl - r.trace).abs();
            let scale_d = r.det.abs().max(1.0);
            let scale_t = r.trace.abs().max(1.0);
            let passed = det_diff / scale_d < 1e-8 && trace_diff / scale_t < 1e-8;
            fields.push(("verify".into(), jobj(vec![
                ("prod_lambda".into(), J::n(pl)),
                ("det_vs_prod".into(), J::n(det_diff)),
                ("sum_lambda".into(), J::n(sl)),
                ("trace_vs_sum".into(), J::n(trace_diff)),
                ("passed".into(), J::b(passed)),
            ])));
        }
    } else if want_eigen {
        fields.push(("eigenvalues".into(), J::Nul));
    }
    fields.push(("elapsed_ms".into(), J::n(started.elapsed().as_secs_f64() * 1000.0)));
    Ok(J::Obj(fields))
}

fn handle_native_tool(tool: &str, args: &J) -> Result<J, String> {
    let spec = args.get("source").and_then(|s| s.as_str())
        .ok_or_else(|| format!("инструмент {}: нет обязательного параметра source", tool))?;
    let verify = args.get("verify").and_then(|v| v.as_bool()).unwrap_or(true);
    match tool {
        "poler_matrix_det" => matrix_json("poler_matrix_det", spec, false, false),
        "poler_matrix_eigen_sturm" => matrix_json("poler_matrix_eigen_sturm", spec, true, verify),
        _ => Err(format!("неизвестный нативный инструмент {}", tool)),
    }
}

// ============================================================================
// SCHEMA — САМОДОКУМЕНТИРУЮЩИЙСЯ МАНИФЕСТ (живые инструменты движка + натив)
// ============================================================================

fn subcommands_table() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("schema", "poler-api schema", "этот манифест: machine-readable карта ВСЕХ возможностей"),
        ("mcp", "poler-api mcp", "MCP-сервер stdio JSON-RPC 2.0: инструменты движка насквозь + нативное Штурм-ядро"),
        ("calc", "poler-api calc 'det(tridiag(2,-1,64))'", "Калькулятор Всего движка: арифметика, единицы, матрицы expm/eigen, solve"),
        ("solve", "poler-api solve 'x^2 - 4 = 0'", "уравнения (calc solve)"),
        ("convert", "poler-api convert '5 km to mi'", "конвертация единиц"),
        ("hw", "poler-api hw", "скрытые параметры ПК: кеши L1/L2/L3, ISA AVX-512, топология ядер"),
        ("matrix det", "poler-api matrix det tridiag:2,-1,128", "детерминант: континуанта O(n) / LU — БЕЗ лимита tridiag n<=64"),
        ("matrix eigen", "poler-api matrix eigen tridiag:2,-1,256", "ВСЕ собственные значения Штурм-бисекцией — БЕЗ взрыва Уилкинсона (eigen движка умирает на n>=28)"),
        ("matrix trace", "poler-api matrix trace inline:2,1;1,2", "след матрицы"),
        ("neural speak", "poler-api neural speak 'привет, движок'", "внутренняя нейросеть движка (Триединство: муха + вихрь + троичный кристалл)"),
        ("quantum run", "poler-api quantum run qft --n 20", "квантовый симулятор движка: bell|ghz|qft|iqft|grover|bv|dj|period|teleport, n<=26"),
        ("search", "poler-api search ~/my-repo 'authentication'", "топографический резонансный поиск: сцены + плотность eps + резонанс R"),
        ("grep", "poler-api grep ~/my-repo 'pattern' --regex", "точный grep-режим движка"),
        ("selftest", "poler-api selftest", "полная батарея проверок: делегаты + Штурм-ядро + MCP-прокси"),
    ]
}

fn build_schema() -> J {
    let engine = engine_version();
    let live = engine_mcp_tools().unwrap_or_default();
    let native = native_mcp_tools();
    let engine_names: Vec<J> = live.iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(J::s))
        .collect();
    let native_names: Vec<J> = native.iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(J::s))
        .collect();
    let mut all = live.clone();
    all.extend(native.clone());
    let subs: Vec<J> = subcommands_table().into_iter()
        .map(|(n, c, d)| jobj(vec![("name", J::s(n)), ("call", J::s(c)), ("description", J::s(d))]))
        .collect();
    let quick: Vec<J> = vec![
        J::s("poler-api calc '2^10'"),
        J::s("poler-api calc 'det(tridiag(2,-1,64))'"),
        J::s("poler-api matrix eigen tridiag:2,-1,256"),
        J::s("poler-api quantum run grover --n 10 --marks 42"),
        J::s("poler-api neural speak 'проверка связи'"),
        J::s("poler-api search ~/my-repo 'резонанс'"),
        J::s("poler-api schema | jq .mcp.tools_total"),
    ];
    jobj(vec![
        ("gateway", jobj(vec![
            ("name", J::s("poler-api")),
            ("version", J::s(API_VERSION)),
            ("engine", J::Str(engine)),
            ("philosophy", J::s("схема раскрывает весь движок одной командой; MCP-прокси отдаёт живые инструменты движка насквозь и добавляет нативное Штурм-ядро; логика движка не дублируется — делегируется")),
        ])),
        ("subcommands", jarr(subs)),
        ("mcp", jobj(vec![
            ("endpoint", J::s("poler-api mcp")),
            ("transport", J::s("stdio, JSON-RPC 2.0 (Model Context Protocol)")),
            ("tools_total", J::n(all.len() as f64)),
            ("engine_tools_count", J::n(live.len() as f64)),
            ("native_tools_count", J::n(native.len() as f64)),
            ("engine_tools", jarr(engine_names)),
            ("native_tools", jarr(native_names)),
            ("tools", jarr(all)),
        ])),
        ("boundaries", jobj(vec![ // честный конверт возможностей
            ("engine_eigen", J::s("надёжен n<=24; на n>=28 взрыв Уилкинсона: коэффициенты charpoly O(n!), комплексный мусор на вещественно-симметричных")),
            ("native_eigen_sturm", J::s("симметричные матрицы; проверено до n=512 с max_err ~5e-13 против аналитики; несимметричные — вне области (нужен QR/Hessenberg)")),
            ("engine_tridiag_builder", J::s("n<=64; источник tridiag: шлюза — до 100000 (континуанта O(n))")),
            ("quantum_dense_statevector", J::s("n<=26 (RAM = 2^N * 16 байт; на хостах 3 ГБ — фактически n<=24); выше — нужны стабилизаторы Клиффорда/MPS")),
            ("neural_triune", J::s("живая популяция: критичность ~0.55, STDP-обучение; динамика ограничена по построению (ротор J + диссипация)")),
        ])),
        ("quickstart", jarr(quick)),
    ])
}

fn cmd_schema(compact: bool) {
    let s = build_schema();
    if compact { println!("{}", json_compact(&s)); }
    else { println!("{}", s.dump()); }
}

// ============================================================================
// ЧЕЛОВЕКОЧИТАЕМАЯ СВОДКА (вызывается без аргументов)
// ============================================================================

fn cmd_capabilities() {
    let engine = engine_version();
    let live = engine_mcp_tools().unwrap_or_default();
    let native = native_mcp_tools();
    println!("poler-api v{} — AI-шлюз к {} ", API_VERSION, engine);
    println!("──────────────────────────────────────────────────────────────");
    println!("Подкоманды (полная карта: poler-api schema):");
    for (n, c, d) in subcommands_table() {
        println!("  {:<14} {:<46} {}", n, "", d);
        let _ = c;
    }
    println!();
    println!("MCP:   poler-api mcp  →  {} инструмента движка + {} нативных (Штурм-ядро)", live.len(), native.len());
    println!("Границы (честно):");
    println!("  * eigen движка      n<=24 надёжен, n>=28 взрыв Уилкинсона (charpoly O(n!))");
    println!("  * Штурм-ядро шлюза  симметричные, n<=512+ на машинной точности ~5e-13");
    println!("  * квант движка      n<=26 (плотный statevector 2^N x 16 байт)");
    println!("  * tridiag движка    n<=64; источник tridiag: шлюза — до 100000");
    println!();
    println!("Быстрый старт:");
    println!("  poler-api calc 'det(tridiag(2,-1,64))'");
    println!("  poler-api matrix eigen tridiag:2,-1,256");
    println!("  poler-api quantum run qft --n 20");
    println!("  poler-api neural speak 'привет'");
}

// ============================================================================
// MCP-ПРОКСИ: stdio JSON-RPC 2.0 -> poler-engine --mcp насквозь,
// tools/list обогащается нативными инструментами, tools/call нативных — свой
// ============================================================================

fn jget_mut<'a>(j: &'a mut J, key: &str) -> Option<&'a mut J> {
    if let J::Obj(m) = j {
        m.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    } else { None }
}

fn emit(line: &str) {
    let stdout = io::stdout();
    let mut l = stdout.lock();
    let _ = writeln!(l, "{}", line);
    let _ = l.flush();
}

fn id_key(j: &J) -> String {
    match j {
        J::Num(x) => fmt_num(*x),
        J::Str(s) => format!("s:{}", s),
        other => other.dump(),
    }
}

fn augment_tools_list(resp: &J) -> String {
    let mut j = resp.clone();
    if let Some(r) = jget_mut(&mut j, "result") {
        if let Some(t) = jget_mut(r, "tools") {
            if let J::Arr(a) = t {
                a.extend(native_mcp_tools());
            }
        }
    }
    json_compact(&j)
}

fn native_tool_response(id: Option<J>, tool: &str, args: &J) -> String {
    let (text, is_err) = match handle_native_tool(tool, args) {
        Ok(j) => (j.dump(), false),
        Err(e) => (jobj(vec![("ok", J::b(false)), ("error", J::Str(e))]).dump(), true),
    };
    json_compact(&jobj(vec![
        ("jsonrpc", J::s("2.0")),
        ("id", id.unwrap_or(J::Nul)),
        ("result", jobj(vec![
            ("content", jarr(vec![jobj(vec![
                ("type", J::s("text")),
                ("text", J::Str(text)),
            ])])),
            ("isError", J::b(is_err)),
        ])),
    ]))
}

fn mcp_proxy() -> i32 {
    let mut child = match Command::new(ENGINE_BIN)
        .arg("--mcp")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => { eprintln!("poler-api: не могу запустить {} --mcp: {}", ENGINE_BIN, e); return 1; }
    };
    let child_stdin = child.stdin.take().expect("stdin движка");
    let child_stdout = child.stdout.take().expect("stdout движка");
    // id запросов tools/list, чьи ответы надо обогатить нативными инструментами
    let pending: Arc<Mutex<std::collections::HashSet<String>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));

    // Поток B: ответы движка -> клиент (tools/list обогащаем)
    let pending_b = pending.clone();
    let t_b = thread::spawn(move || {
        let reader = BufReader::new(child_stdout);
        for line in reader.lines() {
            let line = match line { Ok(l) => l, Err(_) => break };
            if line.trim().is_empty() { continue; }
            if let Ok(j) = jparse(&line) {
                if let Some(idj) = j.get("id") {
                    let key = id_key(idj);
                    let is_tools_list = pending_b.lock().map(|mut p| p.remove(&key)).unwrap_or(false);
                    if is_tools_list {
                        emit(&augment_tools_list(&j));
                        continue;
                    }
                }
            }
            emit(&line);
        }
    });

    // Поток A: запросы клиента -> движку (нативные tools/call обслуживаем сами)
    let t_a = thread::spawn(move || {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin.lock());
        let mut fwd = child_stdin;
        for line in reader.lines() {
            let line = match line { Ok(l) => l, Err(_) => break };
            if line.trim().is_empty() { continue; }
            if let Ok(j) = jparse(&line) {
                let method = j.get("method").and_then(|m| m.as_str()).unwrap_or("");
                if method == "tools/list" {
                    if let Some(idj) = j.get("id") {
                        let _ = pending.lock().map(|mut p| p.insert(id_key(idj)));
                    }
                    let _ = writeln!(&mut fwd, "{}", line);
                    let _ = fwd.flush();
                    continue;
                }
                if method == "tools/call" {
                    let tool = j.get("params")
                        .and_then(|p| p.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    if is_native_tool(tool) {
                        let args = j.get("params")
                            .and_then(|p| p.get("arguments"))
                            .cloned()
                            .unwrap_or(J::Obj(vec![]));
                        let id = j.get("id").cloned();
                        emit(&native_tool_response(id, tool, &args));
                        continue;
                    }
                }
            }
            // всё остальное — дословно движку (initialize, ping, notifications, чужие tools/call)
            if writeln!(&mut fwd, "{}", line).is_err() { break; }
            let _ = fwd.flush();
        }
        drop(fwd); // EOF клиенту -> EOF движку
    });

    let _ = t_a.join();
    let _ = t_b.join();
    let _ = child.kill();
    child.wait().map(|s| s.code().unwrap_or(0)).unwrap_or(0)
}

// ============================================================================
// ПОДКОМАНДЫ CLI
// ============================================================================

fn flag_val(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}
fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn cmd_calc(rest: &[String], prefix: &str) -> i32 {
    if rest.is_empty() {
        eprintln!("использование: poler-api calc '<выражение>' [--json]");
        return 2;
    }
    let expr = rest[0].as_str();
    let cmd = if prefix.is_empty() { format!("calc {}", expr) } else { format!("{} {}", prefix, expr) };
    match engine_exec(&cmd) {
        Ok((ok, output, dur)) => {
            if has_flag(&rest[1..], "--json") {
                println!("{}", jobj(vec![
                    ("expr", J::s(expr)),
                    ("ok", J::b(ok)),
                    ("result", J::Str(output.clone())),
                    ("engine", J::s(&engine_version())),
                    ("engine_duration_ms", J::Str(dur)),
                ]).dump());
            } else if ok {
                println!("{}", output);
            } else {
                eprintln!("движок: {}", output);
                return 1;
            }
            0
        }
        Err(e) => { eprintln!("{}", e); 1 }
    }
}

fn cmd_hw(rest: &[String]) -> i32 {
    let cmd = if has_flag(rest, "--json") { "hw --json" } else { "hw" };
    match engine_exec(cmd) {
        Ok((ok, output, _)) => {
            println!("{}", output);
            if ok { 0 } else { 1 }
        }
        Err(e) => { eprintln!("{}", e); 1 }
    }
}

fn cmd_matrix(rest: &[String]) -> i32 {
    if rest.len() < 2 {
        eprintln!("использование: poler-api matrix det|eigen|trace <источник> [--no-verify]");
        eprintln!("источник: tridiag:2,-1,256 | file:/путь/к/matrix.csv | inline:1,2;3,4");
        return 2;
    }
    let sub = rest[0].as_str();
    let spec = rest[1].as_str();
    let flags = &rest[2..];
    let res = match sub {
        "det" => matrix_json("poler_matrix_det", spec, false, false),
        "trace" => matrix_json("matrix_trace", spec, false, false),
        "eigen" => matrix_json("poler_matrix_eigen_sturm", spec, true, !has_flag(flags, "--no-verify")),
        other => Err(format!("неизвестная операция '{}' (жду det|eigen|trace)", other)),
    };
    match res {
        Ok(j) => { println!("{}", j.dump()); 0 }
        Err(e) => { eprintln!("ошибка: {}", e); 1 }
    }
}

fn cmd_neural(rest: &[String]) -> i32 {
    if rest.is_empty() || rest[0] != "speak" || rest.len() < 2 {
        eprintln!("использование: poler-api neural speak '<текст>' [--tokens N] [--seed S] [--json]");
        return 2;
    }
    // текст = аргументы до первого флага (можно без кавычек)
    let mut text_parts: Vec<&str> = Vec::new();
    let mut i = 1usize;
    while i < rest.len() && !rest[i].starts_with("--") {
        text_parts.push(&rest[i]);
        i += 1;
    }
    let text = text_parts.join(" ");
    let flags = &rest[i..];
    let mut cmd = Command::new(ENGINE_BIN);
    cmd.arg("--triune-speak").arg(&text);
    if let Some(t) = flag_val(flags, "--tokens") { cmd.args(["--triune-tokens", &t]); }
    if let Some(s) = flag_val(flags, "--seed") { cmd.args(["--triune-seed", &s]); }
    let out = cmd.output();
    match out {
        Ok(o) => {
            let text_out = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            if has_flag(flags, "--json") {
                println!("{}", jobj(vec![
                    ("text", J::s(&text)),
                    ("output", J::Str(text_out)),
                ]).dump());
            } else {
                print!("{}", text_out);
            }
            0
        }
        Err(e) => { eprintln!("не могу запустить нейросеть движка: {}", e); 1 }
    }
}

fn cmd_quantum(rest: &[String]) -> i32 {
    if rest.is_empty() {
        eprintln!("использование: poler-api quantum <run|verify|teleport|bloch|list|qaoa|qcasm> [алгоритм] [флаги]");
        eprintln!("  алгоритмы (для run): bell|ghz|qft|iqft|grover|bv|dj|period|teleport");
        eprintln!("  флаги: --n N --shots S --seed S --marks 1,2 --secret N --theta F --check unitary|equivalence|teleport --noise ideal|ibm-heron|google-willow --json");
        return 2;
    }
    let action = rest[0].as_str();
    let mut algo = String::new();
    let mut fi = 1usize;
    if action == "run" {
        if let Some(a) = rest.get(1) {
            if !a.starts_with("--") { algo = a.clone(); fi = 2; }
        }
    }
    let flags = &rest[fi..];
    let mut args: Vec<(String, J)> = vec![("action".into(), J::s(action))];
    if !algo.is_empty() { args.push(("algorithm".into(), J::s(&algo))); }
    if let Some(v) = flag_val(flags, "--n") {
        if let Ok(x) = v.parse::<f64>() { args.push(("n".into(), J::n(x))); }
    }
    if let Some(v) = flag_val(flags, "--shots") {
        if let Ok(x) = v.parse::<f64>() { args.push(("shots".into(), J::n(x))); }
    }
    if let Some(v) = flag_val(flags, "--seed") {
        if let Ok(x) = v.parse::<f64>() { args.push(("seed".into(), J::n(x))); }
    }
    if let Some(v) = flag_val(flags, "--marks") {
        let marks: Vec<J> = v.split(',').filter_map(|m| m.trim().parse::<f64>().ok()).map(J::n).collect();
        args.push(("marks".into(), J::Arr(marks)));
    }
    if let Some(v) = flag_val(flags, "--secret") {
        if let Ok(x) = v.parse::<f64>() { args.push(("secret".into(), J::n(x))); }
    }
    if let Some(v) = flag_val(flags, "--theta") {
        if let Ok(x) = v.parse::<f64>() { args.push(("theta".into(), J::n(x))); }
    }
    if let Some(v) = flag_val(flags, "--check") { args.push(("check".into(), J::s(&v))); }
    if let Some(v) = flag_val(flags, "--noise") { args.push(("noise".into(), J::s(&v))); }
    if let Some(v) = flag_val(flags, "--alpha") { args.push(("alpha".into(), J::Str(v))); }
    if let Some(v) = flag_val(flags, "--beta") { args.push(("beta".into(), J::Str(v))); }
    if has_flag(flags, "--exact") { args.push(("exact".into(), J::b(true))); }
    let params = jobj(vec![
        ("name", J::s("poler_quantum")),
        ("arguments", J::Obj(args)),
    ]);
    match engine_mcp_request("tools/call", Some(params)) {
        Ok(resp) => {
            let text = resp.get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_arr())
                .and_then(|a| a.first())
                .and_then(|t| t.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("(движок не вернул текст)").to_string();
            if has_flag(flags, "--json") {
                println!("{}", resp.dump());
            } else {
                println!("{}", text);
            }
            0
        }
        Err(e) => { eprintln!("квантовый мост недоступен: {}", e); 1 }
    }
}

fn cmd_search(rest: &[String]) -> i32 {
    if rest.len() < 2 {
        eprintln!("использование: poler-api search <путь> '<запрос>' [--format simple|md|ai-json]");
        return 2;
    }
    let path = &rest[0];
    let query = &rest[1];
    let fmt = flag_val(&rest[2..], "--format").unwrap_or_else(|| "simple".into());
    let status = Command::new(ENGINE_BIN)
        .arg(path)
        .args(["-q", query, "--format", &fmt])
        .status();
    match status {
        Ok(s) => s.code().unwrap_or(0),
        Err(e) => { eprintln!("не могу запустить поиск движка: {}", e); 1 }
    }
}

fn cmd_grep(rest: &[String]) -> i32 {
    if rest.len() < 2 {
        eprintln!("использование: poler-api grep <путь> '<паттерн>' [--regex] [-i] [--count] [--list] [--json]");
        return 2;
    }
    let path = &rest[0];
    let pattern = &rest[1];
    let flags = &rest[2..];
    let mut cmd = Command::new(ENGINE_BIN);
    cmd.arg(path).arg("--grep").arg(pattern);
    if has_flag(flags, "--regex") { cmd.arg("--grep-regex"); }
    if has_flag(flags, "-i") || has_flag(flags, "--ignore-case") { cmd.arg("--grep-i"); }
    if has_flag(flags, "--count") { cmd.arg("--grep-count"); }
    if has_flag(flags, "--list") { cmd.arg("--grep-list"); }
    if has_flag(flags, "--json") { cmd.arg("--grep-json"); }
    match cmd.status() {
        Ok(s) => s.code().unwrap_or(0),
        Err(e) => { eprintln!("не могу запустить grep движка: {}", e); 1 }
    }
}

// ============================================================================
// SELFTEST — батарея честности: делегаты движка + Штурм-ядро + MCP-прокси
// ============================================================================

struct Check { name: String, detail: String, pass: bool }

fn add(checks: &mut Vec<Check>, name: &str, detail: String, pass: bool) {
    checks.push(Check { name: name.into(), detail, pass });
}

fn fnum(s: &str) -> Option<f64> { s.trim().parse::<f64>().ok() }
fn near(s: &str, target: f64, tol: f64) -> bool {
    fnum(s).map(|x| (x - target).abs() < tol).unwrap_or(false)
}

/// Спавним сами себя (poler-api mcp) и прогоняем полный MCP-диалог
fn mcp_selfcall(requests: &[J]) -> Result<Vec<J>, String> {
    let exe = env::current_exe().map_err(|e| e.to_string())?;
    let mut child = Command::new(&exe).arg("mcp")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("нет stdin")?;
    {
        let mut w = io::BufWriter::new(&mut stdin);
        for l in requests {
            writeln!(w, "{}", json_compact(l)).map_err(|e| e.to_string())?;
        }
        w.flush().map_err(|e| e.to_string())?;
    }
    drop(stdin); // EOF -> прокси завершает движок и сам выходит
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut resp = Vec::new();
    for ln in text.lines() {
        if ln.trim().is_empty() { continue; }
        if let Ok(j) = jparse(ln) { resp.push(j); }
    }
    Ok(resp)
}

fn find_by_id(resps: &[J], id: f64) -> Option<J> {
    resps.iter().find(|j| j.get("id").and_then(|v| v.as_f64()) == Some(id)).cloned()
}

fn run_selftest() -> i32 {
    let mut c: Vec<Check> = Vec::new();
    println!("poler-api v{} selftest: делегаты движка + нативное Штурм-ядро + MCP-прокси", API_VERSION);
    println!("──────────────────────────────────────────────────────────────────────");

    // 1. движок доступен
    let ver = engine_version();
    add(&mut c, "движок poler-engine", ver.clone(), ver.starts_with("poler-engine"));

    // 2. делегат калькулятора: 2^10
    match engine_calc("2^10") {
        Ok(o) => add(&mut c, "calc '2^10' (через движок)", o.clone(), near(&o, 1024.0, 1e-9)),
        Err(e) => add(&mut c, "calc '2^10' (через движок)", e, false),
    }

    // 3. делегат: det(tridiag(2,-1,64)) — движок точен на 64x64
    match engine_calc("det(tridiag(2,-1,64))") {
        Ok(o) => add(&mut c, "движок det(tridiag(2,-1,64)) = 65", o.clone(), near(&o, 65.0, 1e-9)),
        Err(e) => add(&mut c, "движок det(tridiag(2,-1,64)) = 65", e, false),
    }

    // 4. нативная континуанта: 128 (за лимитом tridiag(n<=64) движка)
    match compute_matrix("tridiag:2,-1,128", false) {
        Ok(r) => add(&mut c, "нативный det tridiag:2,-1,128 = 129", format!("det = {} (континуанта, O(n))", fmt_num(r.det)), (r.det - 129.0).abs() < 1e-9),
        Err(e) => add(&mut c, "нативный det tridiag:2,-1,128 = 129", e, false),
    }

    // 5. континуанта на 512
    match compute_matrix("tridiag:2,-1,512", false) {
        Ok(r) => add(&mut c, "нативный det tridiag:2,-1,512 = 513", format!("det = {}", fmt_num(r.det)), (r.det - 513.0).abs() < 1e-9),
        Err(e) => add(&mut c, "нативный det tridiag:2,-1,512 = 513", e, false),
    }

    // 6. Штурм против аналитики: 128 собственных значений, lam_k = 2-2cos(k*PI/129)
    match compute_matrix("tridiag:2,-1,128", true) {
        Ok(r) => {
            if let Some(ev) = r.eigen {
                let mut max_err = 0.0f64;
                for (i, &lam) in ev.iter().enumerate() {
                    let exact = 2.0 - 2.0 * ((i as f64 + 1.0) * std::f64::consts::PI / 129.0).cos();
                    max_err = max_err.max((lam - exact).abs());
                }
                add(&mut c, "Штурм-бисекция n=128 против аналитики",
                    format!("{} собств. значений, max_err = {:.3e}", ev.len(), max_err),
                    max_err < 1e-9);
            } else { add(&mut c, "Штурм-бисекция n=128", "нет спектра".into(), false); }
        }
        Err(e) => add(&mut c, "Штурм-бисекция n=128 против аналитики", e, false),
    }

    // 7. Штурм 256: prod(lambda) = det = 257 (граница движка была 24!)
    match compute_matrix("tridiag:2,-1,256", true) {
        Ok(r) => {
            let pl = r.prod_lambda.unwrap_or(f64::NAN);
            let sl = r.sum_lambda.unwrap_or(f64::NAN);
            let ok = (pl - 257.0).abs() / 257.0 < 1e-8 && (sl - 512.0).abs() < 1e-6;
            add(&mut c, "Штурм n=256: prod(lam)=det=257, sum=trace=512",
                format!("prod = {}, sum = {}", fmt_num(pl), fmt_num(sl)), ok);
        }
        Err(e) => add(&mut c, "Штурм n=256: prod(lam)=det=257", e, false),
    }

    // 8. плотная 2x2 (Хаусхолдер путь): [[2,1],[1,2]] -> {1, 3}
    match compute_matrix("inline:2,1;1,2", true) {
        Ok(r) => {
            let ev = r.eigen.clone().unwrap_or_default();
            let ok = ev.len() == 2 && (ev[0] - 1.0).abs() < 1e-9 && (ev[1] - 3.0).abs() < 1e-9;
            add(&mut c, "Хаусхолдер+Штурм dense [[2,1],[1,2]]",
                format!("eigen = {:?}", ev), ok);
        }
        Err(e) => add(&mut c, "Хаусхолдер+Штурм dense [[2,1],[1,2]]", e, false),
    }

    // 9. плотная симметричная 40x40 из файла: sum(lam)=trace, prod(lam)=det
    {
        let n = 40usize;
        let mut seed: u64 = 42;
        let mut lcg = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as f64 / 9007199254740992.0) * 2.0 - 1.0
        };
        // верхний треугольник + диагональ со сдвигом 8 (хорошо обусловлена)
        let mut vals = vec![vec![0.0f64; n]; n];
        for i in 0..n {
            for j in 0..n {
                if i == j { vals[i][j] = 8.0 + lcg(); }
                else if i < j { vals[i][j] = lcg(); }
            }
        }
        // симметризация: нижний = верхний
        for i in 0..n {
            for j in 0..i {
                vals[i][j] = vals[j][i];
            }
        }
        let mut full = String::new();
        for i in 0..n {
            let row: Vec<String> = vals[i].iter().map(|v| format!("{:.6}", v)).collect();
            full.push_str(&row.join(","));
            full.push('\n');
        }
        let tmp = std::env::temp_dir().join("poler_api_selftest_dense_40.csv");
        let path_str = tmp.to_string_lossy();
        let path = path_str.as_ref();
        let _ = fs::write(path, &full);
        match compute_matrix(&format!("file:{}", path), true) {
            Ok(r) => {
                let sl = r.sum_lambda.unwrap_or(f64::NAN);
                let pl = r.prod_lambda.unwrap_or(f64::NAN);
                let ok = (sl - r.trace).abs() / r.trace.abs().max(1.0) < 1e-9
                    && (pl - r.det).abs() / r.det.abs().max(1.0) < 1e-9;
                add(&mut c, "файл 40x40 симм.: sum(lam)=trace, prod(lam)=det",
                    format!("trace = {}, sum = {:.9}; det = {:.3e}, prod = {:.3e}", fmt_num(r.trace), sl, r.det, pl), ok);
            }
            Err(e) => add(&mut c, "файл 40x40 симм.", e, false),
        }
    }

    // 10. НЕсимметричная — честный отказ без паники
    match compute_matrix("inline:1,2;3,4", true) {
        Ok(r) => add(&mut c, "НЕсимметричная — честный отказ", r.note.clone(), r.eigen.is_none() && r.note.contains("НЕсимметричная")),
        Err(e) => add(&mut c, "НЕсимметричная — честный отказ", e, false),
    }

    // 11+12. MCP-прокси: хендшейк, tools/list с нативными, нативный tools/call
    {
        let live_count = engine_mcp_tools().map(|t| t.len()).unwrap_or(0);
        let requests = vec![
            jobj(vec![("jsonrpc", J::s("2.0")), ("id", J::n(1.0)), ("method", J::s("initialize")),
                ("params", jobj(vec![("protocolVersion", J::s("2024-11-05")), ("capabilities", J::Obj(vec![])),
                    ("clientInfo", jobj(vec![("name", J::s("selftest")), ("version", J::s("1.0"))]))]))]),
            jobj(vec![("jsonrpc", J::s("2.0")), ("id", J::n(2.0)), ("method", J::s("tools/list"))]),
            jobj(vec![("jsonrpc", J::s("2.0")), ("id", J::n(3.0)), ("method", J::s("tools/call")),
                ("params", jobj(vec![("name", J::s("poler_matrix_eigen_sturm")),
                    ("arguments", jobj(vec![("source", J::s("tridiag:2,-1,64")), ("verify", J::b(true))]))]))]),
        ];
        match mcp_selfcall(&requests) {
            Ok(resps) => {
                let init_ok = find_by_id(&resps, 1.0)
                    .and_then(|r| r.get("result").and_then(|x| x.get("serverInfo")).is_some().then_some(true))
                    .unwrap_or(false);
                add(&mut c, "MCP initialize через прокси", format!("serverInfo присутствует"), init_ok);
                let tools = find_by_id(&resps, 2.0)
                    .and_then(|r| r.get("result").and_then(|x| x.get("tools")).and_then(|t| t.as_arr().cloned()))
                    .unwrap_or_default();
                let expected = live_count + native_mcp_tools().len();
                add(&mut c, "MCP tools/list: движок + нативные",
                    format!("{} инструментов (движок {} + нативные {})", tools.len(), live_count, native_mcp_tools().len()),
                    !tools.is_empty() && tools.len() == expected);
                let native_txt = find_by_id(&resps, 3.0)
                    .and_then(|r| r.get("result").and_then(|x| x.get("content")).and_then(|t| t.as_arr().cloned()))
                    .and_then(|a| a.first().and_then(|t| t.get("text")).and_then(|t| t.as_str().map(String::from)))
                    .unwrap_or_default();
                let prod = jparse(&native_txt).ok()
                    .and_then(|j| j.get("verify").and_then(|v| v.get("prod_lambda")).and_then(|p| p.as_f64()));
                add(&mut c, "MCP tools/call poler_matrix_eigen_sturm (64)",
                    format!("prod_lambda = {}", prod.map(fmt_num).unwrap_or_else(|| "?".into())),
                    prod.map(|p| (p - 65.0).abs() < 1e-8).unwrap_or(false));
            }
            Err(e) => {
                add(&mut c, "MCP прокси", e.clone(), false);
                add(&mut c, "MCP tools/call poler_matrix_eigen_sturm (64)", "прокси не поднялся".into(), false);
            }
        }
    }

    // 13. квантовый мост движка: qft на 8 кубитах
    {
        let params = jobj(vec![
            ("name", J::s("poler_quantum")),
            ("arguments", jobj(vec![("action", J::s("run")), ("algorithm", J::s("qft")), ("n", J::n(8.0)), ("shots", J::n(64.0))])),
        ]);
        match engine_mcp_request("tools/call", Some(params)) {
            Ok(resp) => {
                let txt = resp.get("result").and_then(|r| r.get("content")).and_then(|t| t.as_arr())
                    .and_then(|a| a.first()).and_then(|t| t.get("text")).and_then(|t| t.as_str()).unwrap_or("");
                add(&mut c, "квантовый мост: qft n=8 (через poler_quantum)",
                    format!("{} символов ответа", txt.len()), txt.len() > 0);
            }
            Err(e) => add(&mut c, "квантовый мост: qft n=8", e, false),
        }
    }

    // 14. нейросеть движка
    {
        let out = Command::new(ENGINE_BIN)
            .args(["--triune-speak", "самотест шлюза", "--triune-tokens", "4"])
            .output();
        match out {
            Ok(o) => {
                let t = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
                add(&mut c, "нейросеть движка: triune speak", format!("{} символов телеметрии", t.len()), t.len() > 100);
            }
            Err(e) => add(&mut c, "нейросеть движка: triune speak", e.to_string(), false),
        }
    }

    // итог
    let passed = c.iter().filter(|x| x.pass).count();
    for chk in &c {
        let mark = if chk.pass { "PASS" } else { "FAIL" };
        println!("  [{}] {:<44} {}", mark, chk.name, if chk.detail.chars().count() > 60 { let s: String = chk.detail.chars().take(54).collect(); format!("{}…", s) } else { chk.detail.clone() });
    }
    println!("──────────────────────────────────────────────────────────────────────");
    println!("  {} / {} PASS", passed, c.len());
    if passed == c.len() { 0 } else { 1 }
}

// ============================================================================
// MAIN — диспетчер подкоманд
// ============================================================================

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        cmd_capabilities();
        return;
    }
    let code = match args[0].as_str() {
        "schema" => { cmd_schema(has_flag(&args[1..], "--compact")); 0 }
        "mcp" => mcp_proxy(),
        "calc" => cmd_calc(&args[1..], ""),
        "solve" => cmd_calc(&args[1..], "calc solve"),
        "convert" => cmd_calc(&args[1..], "="),
        "hw" => cmd_hw(&args[1..]),
        "matrix" => cmd_matrix(&args[1..]),
        "neural" => cmd_neural(&args[1..]),
        "quantum" => cmd_quantum(&args[1..]),
        "search" => cmd_search(&args[1..]),
        "grep" => cmd_grep(&args[1..]),
        "selftest" => run_selftest(),
        "capabilities" | "help" | "--help" | "-h" => { cmd_capabilities(); 0 }
        "version" | "--version" => { println!("poler-api v{} ({})", API_VERSION, engine_version()); 0 }
        other => {
            eprintln!("poler-api: неизвестная подкоманда '{}'. Полная карта: poler-api (без аргументов) или poler-api schema", other);
            2
        }
    };
    std::process::exit(code);
}

