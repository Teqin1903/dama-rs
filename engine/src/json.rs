//! A small JSON value: written for results/ladder/ladder.json and the web API's answers, and read
//! for the web API's requests (docs/web-api.md). Numbers are f64; numbers that are not finite are
//! written as `null`.

use std::fmt;

/// A JSON value. An object keeps its fields in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

/// The deepest nesting `parse` accepts, so that no input can overflow the stack.
const MAX_DEPTH: usize = 64;

impl Json {
    /// An object with these fields, in this order.
    pub fn obj(fields: Vec<(&str, Json)>) -> Json {
        Json::Obj(
            fields
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    /// `x` rounded to `decimals` decimals.
    pub fn num(x: f64, decimals: i32) -> Json {
        let scale = 10f64.powi(decimals);
        Json::Num((x * scale).round() / scale)
    }

    /// `x` rounded, or `null`.
    pub fn opt(x: Option<f64>, decimals: i32) -> Json {
        x.map_or(Json::Null, |x| Json::num(x, decimals))
    }

    pub fn str(s: &str) -> Json {
        Json::Str(s.to_string())
    }

    /// The field `key` of an object; `None` if it is missing or this is not an object.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(x) => Some(*x),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }

    /// The value with two spaces of indentation per level; arrays of plain values stay on one line.
    pub fn write(&self, out: &mut String, indent: usize) {
        let pad = |n: usize| "  ".repeat(n);
        match self {
            Json::Arr(items)
                if !items
                    .iter()
                    .all(|i| !matches!(i, Json::Arr(_) | Json::Obj(_))) =>
            {
                out.push_str("[\n");
                for (k, item) in items.iter().enumerate() {
                    out.push_str(&pad(indent + 1));
                    item.write(out, indent + 1);
                    out.push_str(if k + 1 < items.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad(indent));
                out.push(']');
            }
            Json::Obj(fields) => {
                out.push_str("{\n");
                for (k, (key, value)) in fields.iter().enumerate() {
                    out.push_str(&pad(indent + 1));
                    write_string(out, key);
                    out.push_str(": ");
                    value.write(out, indent + 1);
                    out.push_str(if k + 1 < fields.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad(indent));
                out.push('}');
            }
            plain => plain.write_compact(out, ", "),
        }
    }

    /// The value on one line, items separated by `separator`.
    fn write_compact(&self, out: &mut String, separator: &str) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(x) if x.is_finite() => out.push_str(&format!("{x}")),
            Json::Num(_) => out.push_str("null"),
            Json::Str(s) => write_string(out, s),
            Json::Arr(items) => {
                out.push('[');
                for (k, item) in items.iter().enumerate() {
                    if k > 0 {
                        out.push_str(separator);
                    }
                    item.write_compact(out, separator);
                }
                out.push(']');
            }
            Json::Obj(fields) => {
                out.push('{');
                for (k, (key, value)) in fields.iter().enumerate() {
                    if k > 0 {
                        out.push_str(separator);
                    }
                    write_string(out, key);
                    out.push(':');
                    value.write_compact(out, separator);
                }
                out.push('}');
            }
        }
    }

    /// Reads one JSON value, with nothing but white space around it.
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut reader = Reader {
            text,
            bytes: text.as_bytes(),
            at: 0,
        };
        let value = reader.value(0)?;
        reader.space();
        if reader.at < reader.bytes.len() {
            return Err(reader.error("more after the value"));
        }
        Ok(value)
    }
}

/// On one line, without spaces: the web API's answers.
impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        self.write_compact(&mut out, ",");
        f.write_str(&out)
    }
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A recursive-descent reader over the bytes of the text.
struct Reader<'a> {
    text: &'a str,
    bytes: &'a [u8],
    /// Always at the start of a character.
    at: usize,
}

impl Reader<'_> {
    fn error(&self, what: &str) -> String {
        format!("bad JSON at character {}: {what}", self.at + 1)
    }

    fn space(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| b" \t\r\n".contains(b))
        {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    /// Takes `word` if the text goes on with it.
    fn take(&mut self, word: &str) -> bool {
        let found = self.bytes[self.at..].starts_with(word.as_bytes());
        if found {
            self.at += word.len();
        }
        found
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err(self.error("nested too deeply"));
        }
        self.space();
        match self.peek() {
            None => Err(self.error("a value is missing")),
            Some(b'n') if self.take("null") => Ok(Json::Null),
            Some(b't') if self.take("true") => Ok(Json::Bool(true)),
            Some(b'f') if self.take("false") => Ok(Json::Bool(false)),
            Some(b'"') => self.string().map(Json::Str),
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if self.take("]") {
                    return Ok(Json::Arr(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    self.space();
                    if self.take("]") {
                        return Ok(Json::Arr(items));
                    }
                    if !self.take(",") {
                        return Err(self.error("expected , or ]"));
                    }
                }
            }
            Some(b'{') => {
                self.at += 1;
                let mut fields = Vec::new();
                self.space();
                if self.take("}") {
                    return Ok(Json::Obj(fields));
                }
                loop {
                    self.space();
                    if self.peek() != Some(b'"') {
                        return Err(self.error("expected a field name in quotes"));
                    }
                    let key = self.string()?;
                    self.space();
                    if !self.take(":") {
                        return Err(self.error("expected :"));
                    }
                    fields.push((key, self.value(depth + 1)?));
                    self.space();
                    if self.take("}") {
                        return Ok(Json::Obj(fields));
                    }
                    if !self.take(",") {
                        return Err(self.error("expected , or }"));
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("not a JSON value")),
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_digit() || b"+-.eE".contains(&b))
        {
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).expect("ASCII");
        // Rust's parser takes a few forms JSON does not ("+1", "1."); those are refused here.
        let digits = text.trim_start_matches('-').as_bytes();
        let leading_zero = digits.len() > 1 && digits[0] == b'0' && digits[1].is_ascii_digit();
        let json_form = !text.starts_with('+')
            && !leading_zero
            && !text.ends_with('.')
            && !text.contains(".e")
            && !text.contains(".E");
        match text.parse::<f64>() {
            Ok(x) if json_form && x.is_finite() => Ok(Json::Num(x)),
            _ => {
                self.at = start;
                Err(self.error(&format!("bad number {text:?}")))
            }
        }
    }

    /// A string in quotes, at the opening quote.
    fn string(&mut self) -> Result<String, String> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let Some(b) = self.peek() else {
                return Err(self.error("a string is not closed"));
            };
            match b {
                b'"' => {
                    self.at += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.at += 1;
                    let escaped = self
                        .peek()
                        .ok_or_else(|| self.error("a string is not closed"))?;
                    self.at += 1;
                    match escaped {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let first = self.hex4()?;
                            let code = if (0xd800..0xdc00).contains(&first) {
                                if !self.take("\\u") {
                                    return Err(self.error("a surrogate pair is cut"));
                                }
                                let second = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&second) {
                                    return Err(self.error("a surrogate pair is cut"));
                                }
                                0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00)
                            } else {
                                first
                            };
                            out.push(
                                char::from_u32(code)
                                    .ok_or_else(|| self.error("not a character"))?,
                            );
                        }
                        _ => return Err(self.error("bad escape")),
                    }
                }
                b if b < 0x20 => return Err(self.error("a control character in a string")),
                _ => {
                    let c = self.text[self.at..].chars().next().expect("not at the end");
                    out.push(c);
                    self.at += c.len_utf8();
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.at..self.at + 4)
            .and_then(|d| std::str::from_utf8(d).ok())
            .and_then(|d| u32::from_str_radix(d, 16).ok())
            .ok_or_else(|| self.error("expected four hex digits"))?;
        self.at += 4;
        Ok(digits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_read_back_as_written() {
        let value = Json::obj(vec![
            ("a", Json::Num(-1.5e-3)),
            (
                "b",
                Json::Arr(vec![
                    Json::Null,
                    Json::Bool(true),
                    Json::str("x\"\\\n\u{1}é😀"),
                ]),
            ),
            ("c", Json::obj(vec![("d", Json::Arr(vec![]))])),
        ]);
        let compact = value.to_string();
        assert_eq!(Json::parse(&compact), Ok(value.clone()));
        let mut pretty = String::new();
        value.write(&mut pretty, 0);
        assert_eq!(Json::parse(&pretty), Ok(value));
    }

    #[test]
    fn escapes_and_white_space() {
        assert_eq!(
            Json::parse(" { \"k\" : [ 1 , 2e2 , \"\\u00e9\\ud83d\\ude00\\/\" ] } \n"),
            Ok(Json::obj(vec![(
                "k",
                Json::Arr(vec![Json::Num(1.0), Json::Num(200.0), Json::str("é😀/")])
            )]))
        );
    }

    #[test]
    fn bad_text_is_an_error_not_a_panic() {
        for bad in [
            "",
            "{",
            "[1,]",
            "01",
            "{\"a\" 1}",
            "nul",
            "01x",
            "+1",
            "1.",
            "\"abc",
            "\"\\x\"",
            "[1] 2",
            "\"\\ud800\"",
            "{1: 2}",
            "\"\u{1}\"",
            "NaN",
            "1e999",
        ] {
            assert!(Json::parse(bad).is_err(), "{bad:?}");
        }
        let deep = "[".repeat(100_000);
        assert!(Json::parse(&deep).is_err());
    }
}
