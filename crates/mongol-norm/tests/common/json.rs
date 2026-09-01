//! A tiny dependency-free JSON reader for the golden fixtures: objects, arrays, strings with
//! escapes (incl. surrogate pairs), numbers, `true`/`false`/`null`. It panics on malformed
//! input, which is fine for committed fixtures.

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(text: &str) -> Json {
        let mut parser = Parser {
            chars: text.chars().collect(),
            pos: 0,
        };
        parser.skip_ws();
        let value = parser.value();
        parser.skip_ws();
        assert_eq!(
            parser.pos,
            parser.chars.len(),
            "trailing data after JSON value"
        );
        value
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn index(&self, key: &str) -> &Json {
        self.get(key)
            .unwrap_or_else(|| panic!("missing key {key:?}"))
    }

    pub fn as_str(&self) -> &str {
        match self {
            Json::String(s) => s,
            other => panic!("expected string, got {other:?}"),
        }
    }

    /// `None` for `null`, else the string.
    pub fn as_str_opt(&self) -> Option<&str> {
        match self {
            Json::Null => None,
            other => Some(other.as_str()),
        }
    }

    pub fn as_array(&self) -> &[Json] {
        match self {
            Json::Array(items) => items,
            other => panic!("expected array, got {other:?}"),
        }
    }

    pub fn as_u64(&self) -> u64 {
        match self {
            Json::Number(n) if n.fract() == 0.0 && *n >= 0.0 => *n as u64,
            other => panic!("expected non-negative integer, got {other:?}"),
        }
    }

    pub fn as_usize(&self) -> usize {
        usize::try_from(self.as_u64()).expect("fits usize")
    }

    /// An array of strings.
    pub fn strings(&self) -> Vec<String> {
        self.as_array()
            .iter()
            .map(|v| v.as_str().to_owned())
            .collect()
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> char {
        let c = self.chars[self.pos];
        self.pos += 1;
        c
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\n' | '\r' | '\t')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, expected: char) {
        let got = self.bump();
        assert_eq!(got, expected, "at char {}", self.pos - 1);
    }

    fn literal(&mut self, word: &str) {
        for expected in word.chars() {
            self.expect(expected);
        }
    }

    fn value(&mut self) -> Json {
        match self.peek().expect("unexpected end of JSON") {
            '{' => self.object(),
            '[' => self.array(),
            '"' => Json::String(self.string()),
            't' => {
                self.literal("true");
                Json::Bool(true)
            }
            'f' => {
                self.literal("false");
                Json::Bool(false)
            }
            'n' => {
                self.literal("null");
                Json::Null
            }
            _ => self.number(),
        }
    }

    fn object(&mut self) -> Json {
        self.expect('{');
        let mut fields = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Json::Object(fields);
        }
        loop {
            self.skip_ws();
            let key = self.string();
            self.skip_ws();
            self.expect(':');
            self.skip_ws();
            let value = self.value();
            fields.push((key, value));
            self.skip_ws();
            match self.bump() {
                ',' => continue,
                '}' => break,
                other => panic!("unexpected {other:?} in object"),
            }
        }
        Json::Object(fields)
    }

    fn array(&mut self) -> Json {
        self.expect('[');
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Json::Array(items);
        }
        loop {
            self.skip_ws();
            items.push(self.value());
            self.skip_ws();
            match self.bump() {
                ',' => continue,
                ']' => break,
                other => panic!("unexpected {other:?} in array"),
            }
        }
        Json::Array(items)
    }

    fn string(&mut self) -> String {
        self.expect('"');
        let mut out = String::new();
        loop {
            match self.bump() {
                '"' => break,
                '\\' => match self.bump() {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'u' => {
                        let unit = self.hex4();
                        let cp = if (0xD800..0xDC00).contains(&unit) {
                            self.expect('\\');
                            self.expect('u');
                            let low = self.hex4();
                            0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)
                        } else {
                            unit
                        };
                        out.push(char::from_u32(cp).expect("valid escaped code point"));
                    }
                    other => panic!("bad escape \\{other}"),
                },
                other => out.push(other),
            }
        }
        out
    }

    fn hex4(&mut self) -> u32 {
        let mut value = 0;
        for _ in 0..4 {
            value = value * 16 + self.bump().to_digit(16).expect("hex digit");
        }
        value
    }

    fn number(&mut self) -> Json {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))
        {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        Json::Number(
            text.parse()
                .unwrap_or_else(|_| panic!("bad number {text:?}")),
        )
    }
}

#[test]
fn reads_the_shapes_the_goldens_use() {
    let value = Json::parse(
        r#"{"id":"x","cps":[6176, 6155],"shape":["A","Aa"],"n":null,"t":true,"e":{},"s":"ᠠ\n"}"#,
    );
    assert_eq!(value.index("id").as_str(), "x");
    assert_eq!(value.index("cps").as_array()[1].as_u64(), 6155);
    assert_eq!(value.index("shape").strings(), vec!["A", "Aa"]);
    assert_eq!(value.index("n").as_str_opt(), None);
    assert_eq!(value.index("t"), &Json::Bool(true));
    assert_eq!(value.index("e"), &Json::Object(vec![]));
    assert_eq!(value.index("s").as_str(), "\u{1820}\n");
}
