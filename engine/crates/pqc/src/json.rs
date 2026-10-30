//! Минимальный JSON без внешних зависимостей — протокол кросс-теста
//! паритета с Qiskit (экспорт амплитуд/счётчиков, импорт вердикта).
//!
//! Поддерживается строгое подмножество JSON (RFC 8259): `null`, `true`,
//! `false`, числа (включая экспоненту), строки с escape-последовательностями
//! (в т.ч. `\uXXXX` и суррогатные пары), массивы и объекты с сохранением
//! порядка ключей (детерминизм сериализации). Глубина вложенности
//! ограничена 128 — защита стека от враждебного ввода.
//!
//! Числа сериализуются форматом `{:e}` (кратчайшее round-trip
//! представление Rust) и парсятся через `f64::from_str` — бит-в-бит
//! точный обмен с Python (`json.dumps`/`json.loads` используют ту же
//! кратчайшую десятичную форму).

use core::fmt;

/// Ошибка разбора JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonError {
    /// Позиция в байтах.
    pub pos: usize,
    /// Описание.
    pub kind: &'static str,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "json error at byte {}: {}", self.pos, self.kind)
    }
}

impl std::error::Error for JsonError {}

/// Значение JSON (объекты хранят порядок ключей).
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    /// `null`.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// Число (не-конечные значения запрещены контрактом протокола).
    Num(f64),
    /// Строка.
    Str(String),
    /// Массив.
    Arr(Vec<Json>),
    /// Объект — пары ключ/значение в исходном порядке.
    Obj(Vec<(String, Json)>),
}

/// Потолок вложенности при разборе.
const MAX_DEPTH: usize = 128;

impl Json {
    /// Число; NaN/±∞ отображаются в `null` (JSON их не кодирует).
    pub fn num(x: f64) -> Json {
        if x.is_finite() {
            Json::Num(x)
        } else {
            Json::Null
        }
    }

    /// Строка.
    pub fn str(s: impl Into<String>) -> Json {
        Json::Str(s.into())
    }

    /// Массив чисел.
    pub fn num_arr(xs: impl IntoIterator<Item = f64>) -> Json {
        Json::Arr(xs.into_iter().map(Json::num).collect())
    }

    /// Значение ключа объекта.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Числовое значение.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(x) => Some(*x),
            _ => None,
        }
    }

    /// Строковое значение.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Булево значение.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Срез массива.
    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(v) => Some(v),
            _ => None,
        }
    }

    /// Массив чисел (`Vec<f64>`).
    pub fn as_f64_vec(&self) -> Option<Vec<f64>> {
        self.as_arr()
            .map(|a| a.iter().filter_map(Json::as_f64).collect())
    }

    /// Компактная сериализация.
    pub fn to_string(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Num(x) => {
                if x.is_finite() {
                    out.push_str(&format!("{x:e}"));
                } else {
                    out.push_str("null");
                }
            }
            Json::Str(s) => write_escaped(out, s),
            Json::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Obj(pairs) => {
                out.push('{');
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_escaped(out, k);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }

    /// Разбор JSON-документа (после значения допустимы только пробелы).
    pub fn parse(text: &str) -> Result<Json, JsonError> {
        let mut p = Parser {
            bytes: text.as_bytes(),
            pos: 0,
        };
        p.skip_ws();
        let v = p.value(0)?;
        p.skip_ws();
        if p.pos != p.bytes.len() {
            return Err(p.err("trailing characters after the value"));
        }
        Ok(v)
    }
}

/// Экранирование строки.
fn write_escaped(out: &mut String, s: &str) {
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

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, kind: &'static str) -> JsonError {
        JsonError {
            pos: self.pos,
            kind,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, b: u8) -> Result<(), JsonError> {
        if self.peek() == Some(b) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.err("unexpected character"))
        }
    }

    fn literal(&mut self, word: &str) -> Result<(), JsonError> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(())
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        match self.peek() {
            Some(b'n') => {
                self.literal("null")?;
                Ok(Json::Null)
            }
            Some(b't') => {
                self.literal("true")?;
                Ok(Json::Bool(true))
            }
            Some(b'f') => {
                self.literal("false")?;
                Ok(Json::Bool(false))
            }
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                self.skip_ws();
                if self.peek() == Some(b']') {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                loop {
                    self.skip_ws();
                    items.push(self.value(depth + 1)?);
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b']') => {
                            self.pos += 1;
                            return Ok(Json::Arr(items));
                        }
                        _ => return Err(self.err("expected ',' or ']' in array")),
                    }
                }
            }
            Some(b'{') => {
                self.pos += 1;
                let mut pairs = Vec::new();
                self.skip_ws();
                if self.peek() == Some(b'}') {
                    self.pos += 1;
                    return Ok(Json::Obj(pairs));
                }
                loop {
                    self.skip_ws();
                    if self.peek() != Some(b'"') {
                        return Err(self.err("expected string key in object"));
                    }
                    let key = self.string()?;
                    self.skip_ws();
                    self.expect(b':')?;
                    self.skip_ws();
                    let val = self.value(depth + 1)?;
                    pairs.push((key, val));
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b'}') => {
                            self.pos += 1;
                            return Ok(Json::Obj(pairs));
                        }
                        _ => return Err(self.err("expected ',' or '}' in object")),
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.err("unexpected token")),
        }
    }

    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(self.err("invalid number")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            let int_digits = matches!(self.peek(), Some(b'0'..=b'9'));
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if !int_digits {
                return Err(self.err("invalid number fraction"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            let exp_digits = matches!(self.peek(), Some(b'0'..=b'9'));
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if !exp_digits {
                return Err(self.err("invalid number exponent"));
            }
        }
        let text =
            core::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_| self.err("utf8"))?;
        text.parse::<f64>()
            .map(Json::Num)
            .map_err(|_| self.err("number out of range"))
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let b = self.peek().ok_or_else(|| self.err("unterminated string"))?;
            match b {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    let esc = self.peek().ok_or_else(|| self.err("bad escape"))?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp) {
                                // Суррогатная пара.
                                if self.peek() == Some(b'\\') {
                                    self.pos += 1;
                                    if self.peek() != Some(b'u') {
                                        return Err(self.err("bad surrogate pair"));
                                    }
                                    self.pos += 1;
                                    let low = self.hex4()?;
                                    if !(0xDC00..0xE000).contains(&low) {
                                        return Err(self.err("bad surrogate pair"));
                                    }
                                    let c = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                                    out.push(
                                        char::from_u32(c)
                                            .ok_or_else(|| self.err("bad codepoint"))?,
                                    );
                                } else {
                                    return Err(self.err("lone surrogate"));
                                }
                            } else if (0xDC00..0xE000).contains(&cp) {
                                return Err(self.err("lone surrogate"));
                            } else {
                                out.push(
                                    char::from_u32(cp).ok_or_else(|| self.err("bad codepoint"))?,
                                );
                            }
                        }
                        _ => return Err(self.err("bad escape")),
                    }
                }
                _ if b < 0x20 => return Err(self.err("control character in string")),
                _ => {
                    // Многобайтовый UTF-8: копируем весь кластер.
                    let len = utf8_len(b);
                    let start = self.pos;
                    self.pos += len;
                    let s = core::str::from_utf8(self.bytes.get(start..self.pos).unwrap_or(&[]))
                        .map_err(|_| self.err("invalid utf8"))?;
                    out.push_str(s);
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let mut v = 0u32;
        for _ in 0..4 {
            let b = self.peek().ok_or_else(|| self.err("bad \\u escape"))?;
            let d = (b as char)
                .to_digit(16)
                .ok_or_else(|| self.err("bad \\u escape"))?;
            v = v * 16 + d;
            self.pos += 1;
        }
        Ok(v)
    }
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first < 0xE0 {
        2
    } else if first < 0xF0 {
        3
    } else {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_scalars() {
        for text in [
            "null", "true", "false", "0", "-0", "3.25", "-7e-9", "1e300", "2.5e+18",
        ] {
            let v = Json::parse(text).unwrap();
            let s = v.to_string();
            let v2 = Json::parse(&s).unwrap();
            assert_eq!(v, v2, "{text} → {s}");
        }
        assert_eq!(Json::parse("true").unwrap(), Json::Bool(true));
        assert_eq!(Json::parse("-0").unwrap(), Json::Num(-0.0));
        assert_eq!(Json::parse("1e300").unwrap(), Json::Num(1e300));
    }

    #[test]
    fn roundtrip_nested() {
        let doc = Json::Obj(vec![
            ("schema".into(), Json::str("pqc-parity/1")),
            ("amplitudes".into(), Json::num_arr([0.5, -0.25, 1e-12, 0.0])),
            (
                "counts".into(),
                Json::Obj(vec![
                    ("3".into(), Json::Num(12345.0)),
                    ("7".into(), Json::Num(9.0)),
                ]),
            ),
            (
                "nested".into(),
                Json::Arr(vec![Json::Arr(vec![]), Json::Null]),
            ),
        ]);
        let s = doc.to_string();
        let back = Json::parse(&s).unwrap();
        assert_eq!(doc, back);
        // Порядок ключей сохраняется.
        assert_eq!(
            s.find("\"schema\"").unwrap() < s.find("\"counts\"").unwrap(),
            true
        );
    }

    #[test]
    fn strings_and_escapes() {
        let cases = [
            ("", "\"\""),
            ("a\"b", "\"a\\\"b\""),
            ("a\\b", "\"a\\\\b\""),
            ("line\nbreak", "\"line\\nbreak\""),
            ("таб\t", "\"таб\\t\""),
            // Управляющий символ сериализуется как \u0001.
            ("\u{1}", "\"\\u0001\""),
        ];
        for (raw, json) in cases {
            assert_eq!(Json::str(raw).to_string(), json);
            assert_eq!(Json::parse(json).unwrap(), Json::Str(raw.to_string()));
        }
        // Кириллица проходит как есть (UTF-8).
        assert_eq!(Json::parse("\"ядро\"").unwrap(), Json::Str("ядро".into()));
        // Суррогатная пара: U+1D53D (𝔽).
        assert_eq!(
            Json::parse("\"\\ud835\\udd3d\"").unwrap(),
            Json::Str("\u{1D53D}".into())
        );
        assert_eq!(Json::str("\u{1D53D}").to_string(), "\"\u{1D53D}\"");
    }

    #[test]
    fn rejects_malformed() {
        for bad in [
            "",
            "   ",
            "nul",
            "tru",
            "{",
            "}",
            "[1,",
            "[1 2]",
            "{\"a\":}",
            "{\"a\" 1}",
            "01",
            "1.",
            "1e",
            "-",
            "+1",
            "\"unterminated",
            "\"\\q\"",
            "\"\\u12\"",
            "\"\\ud800 alone\"",
            "1 2",
            "[1] x",
            "{}extra",
        ] {
            assert!(Json::parse(bad).is_err(), "должен быть ошибкой: {bad:?}");
        }
    }

    #[test]
    fn depth_limit() {
        let mut s = String::new();
        for _ in 0..200 {
            s.push('[');
        }
        for _ in 0..200 {
            s.push(']');
        }
        assert!(Json::parse(&s).is_err());
    }

    #[test]
    fn accessors() {
        let doc =
            Json::parse(r#"{"pass":true,"p_value":0.42,"name":"chi2","xs":[1e-3,2]}"#).unwrap();
        assert_eq!(doc.get("pass").and_then(Json::as_bool), Some(true));
        assert_eq!(doc.get("p_value").and_then(Json::as_f64), Some(0.42));
        assert_eq!(doc.get("name").and_then(Json::as_str), Some("chi2"));
        assert_eq!(
            doc.get("xs").and_then(Json::as_f64_vec),
            Some(vec![1e-3, 2.0])
        );
        assert!(doc.get("missing").is_none());
        assert_eq!(doc.get("pass").and_then(Json::as_f64), None);
    }

    #[test]
    fn num_maps_nonfinite_to_null() {
        assert_eq!(Json::num(f64::NAN), Json::Null);
        assert_eq!(Json::num(f64::INFINITY), Json::Null);
        assert_eq!(Json::num(1.5), Json::Num(1.5));
    }

    /// Побитная точность round-trip f64: кратчайшая форма `{:e}`.
    #[test]
    fn f64_bit_exact_roundtrip() {
        let mut x = 0.1234567890123456789_f64;
        for _ in 0..1000 {
            let s = Json::Num(x).to_string();
            let y = Json::parse(&s).unwrap().as_f64().unwrap();
            assert_eq!(x.to_bits(), y.to_bits(), "{s}");
            x = (x * 1.0000000001).fract();
        }
    }
}
