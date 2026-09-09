use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum LuaValue {
    Number(f64),
    String(String),
    Table(Vec<(LuaKey, LuaValue)>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum LuaKey {
    Integer(i64),
    String(String),
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at byte {}: {}", self.position, self.message)
    }
}

impl std::error::Error for ParseError {}

pub struct FileHeader {
    pub category: String,
    pub variant: String,
}

pub struct ParsedFile {
    pub header: FileHeader,
    pub entries: Vec<(LuaKey, LuaValue)>,
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, pos: 0 }
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError {
            message: msg.into(),
            position: self.pos,
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let b = self.input.get(self.pos).copied()?;
        self.pos += 1;
        Some(b)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
            if self.pos + 1 < self.input.len()
                && self.input[self.pos] == b'-'
                && self.input[self.pos + 1] == b'-'
            {
                while self.pos < self.input.len() && self.input[self.pos] != b'\n' {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, b: u8) -> Result<(), ParseError> {
        self.skip_whitespace_and_comments();
        match self.peek() {
            Some(c) if c == b => {
                self.pos += 1;
                Ok(())
            }
            Some(c) => Err(self.err(format!("expected '{}', got '{}'", b as char, c as char))),
            None => Err(self.err(format!("expected '{}', got EOF", b as char))),
        }
    }

    fn expect_str(&mut self, s: &[u8]) -> Result<(), ParseError> {
        self.skip_whitespace_and_comments();
        if self.pos + s.len() <= self.input.len() && &self.input[self.pos..self.pos + s.len()] == s
        {
            self.pos += s.len();
            Ok(())
        } else {
            let got = std::str::from_utf8(
                &self.input[self.pos..std::cmp::min(self.pos + s.len() + 10, self.input.len())],
            )
            .unwrap_or("???");
            Err(self.err(format!(
                "expected '{}', got '{}'",
                std::str::from_utf8(s).unwrap(),
                got
            )))
        }
    }

    fn parse_quoted_key(&mut self) -> Result<String, ParseError> {
        self.expect(b'[')?;
        self.expect(b'"')?;
        let start = self.pos;
        while self.pos < self.input.len() && self.input[self.pos] != b'"' {
            self.pos += 1;
        }
        let key = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| self.err("invalid UTF-8 in key"))?
            .to_string();
        self.expect(b'"')?;
        self.expect(b']')?;
        Ok(key)
    }

    fn parse_number(&mut self) -> Result<f64, ParseError> {
        self.skip_whitespace_and_comments();
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos < self.input.len() && self.input[self.pos] == b'.' {
            self.pos += 1;
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        if self.pos == start {
            return Err(self.err("expected number"));
        }
        let s = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| self.err("invalid UTF-8 in number"))?;
        s.parse::<f64>()
            .map_err(|e| self.err(format!("invalid number '{}': {}", s, e)))
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        self.expect(b'"')?;
        let mut result = String::new();
        loop {
            match self.advance() {
                Some(b'"') => break,
                Some(b'\\') => match self.advance() {
                    Some(b'n') => result.push('\n'),
                    Some(b'\\') => result.push('\\'),
                    Some(b'"') => result.push('"'),
                    Some(b'\'') => result.push('\''),
                    Some(c) => {
                        result.push('\\');
                        result.push(c as char);
                    }
                    None => return Err(self.err("unexpected EOF in string escape")),
                },
                Some(b) => result.push(b as char),
                None => return Err(self.err("unexpected EOF in string")),
            }
        }
        Ok(result)
    }

    fn parse_table(&mut self) -> Result<Vec<(LuaKey, LuaValue)>, ParseError> {
        self.expect(b'{')?;
        let mut entries = Vec::new();
        let mut positional_index: i64 = 1;
        loop {
            self.skip_whitespace_and_comments();
            if self.peek() == Some(b'}') {
                self.pos += 1;
                break;
            }
            if self.at_end() {
                return Err(self.err("unexpected EOF in table"));
            }

            if self.peek() == Some(b'[') {
                let key = self.parse_key()?;
                self.skip_whitespace_and_comments();
                self.expect(b'=')?;
                let value = self.parse_value()?;
                entries.push((key, value));
            } else {
                let value = self.parse_value()?;
                entries.push((LuaKey::Integer(positional_index), value));
                positional_index += 1;
            }

            self.skip_whitespace_and_comments();
            if self.peek() == Some(b',') {
                self.pos += 1;
            }
        }
        Ok(entries)
    }

    fn parse_key(&mut self) -> Result<LuaKey, ParseError> {
        self.skip_whitespace_and_comments();
        self.expect(b'[')?;
        self.skip_whitespace_and_comments();
        match self.peek() {
            Some(b'"') => {
                let s = self.parse_string()?;
                self.skip_whitespace_and_comments();
                self.expect(b']')?;
                Ok(LuaKey::String(s))
            }
            Some(c) if c == b'-' || c.is_ascii_digit() => {
                let n = self.parse_number()?;
                self.skip_whitespace_and_comments();
                self.expect(b']')?;
                Ok(LuaKey::Integer(n as i64))
            }
            Some(c) => Err(self.err(format!("unexpected char in key: '{}'", c as char))),
            None => Err(self.err("unexpected EOF in key")),
        }
    }

    fn parse_value(&mut self) -> Result<LuaValue, ParseError> {
        self.skip_whitespace_and_comments();
        match self.peek() {
            Some(b'{') => {
                let entries = self.parse_table()?;
                Ok(LuaValue::Table(entries))
            }
            Some(b'"') => {
                let s = self.parse_string()?;
                Ok(LuaValue::String(s))
            }
            Some(c) if c == b'-' || c.is_ascii_digit() => {
                let n = self.parse_number()?;
                Ok(LuaValue::Number(n))
            }
            Some(c) => Err(self.err(format!("unexpected char in value: '{}'", c as char))),
            None => Err(self.err("unexpected EOF in value")),
        }
    }
}

pub fn parse_file(input: &str) -> Result<ParsedFile, ParseError> {
    let mut parser = Parser::new(input.as_bytes());
    parser.skip_whitespace_and_comments();

    parser.expect_str(b"pfDB")?;
    let category = {
        parser.skip_whitespace_and_comments();
        parser.parse_quoted_key()?
    };
    let variant = {
        parser.skip_whitespace_and_comments();
        parser.parse_quoted_key()?
    };
    parser.skip_whitespace_and_comments();
    parser.expect(b'=')?;

    let entries = parser.parse_table()?;

    Ok(ParsedFile {
        header: FileHeader { category, variant },
        entries,
    })
}

impl LuaValue {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            LuaValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            LuaValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_table(&self) -> Option<&[(LuaKey, LuaValue)]> {
        match self {
            LuaValue::Table(t) => Some(t),
            _ => None,
        }
    }

    pub fn get_field(&self, key: &str) -> Option<&LuaValue> {
        self.as_table()?.iter().find_map(|(k, v)| match k {
            LuaKey::String(s) if s == key => Some(v),
            _ => None,
        })
    }

    pub fn get_int_keys(&self) -> Option<Vec<(i64, &LuaValue)>> {
        Some(
            self.as_table()?
                .iter()
                .filter_map(|(k, v)| match k {
                    LuaKey::Integer(i) => Some((*i, v)),
                    _ => None,
                })
                .collect(),
        )
    }

    pub fn as_int_array(&self) -> Vec<u32> {
        match self {
            LuaValue::Table(entries) => entries
                .iter()
                .filter_map(|(_, v)| v.as_number().map(|n| n as u32))
                .collect(),
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_locale() {
        let input = r#"pfDB["units"]["enUS"] = {
  [1] = "Young Wolf",
  [2] = "Stormwind Guard",
}"#;
        let result = parse_file(input).unwrap();
        assert_eq!(result.header.category, "units");
        assert_eq!(result.header.variant, "enUS");
        assert_eq!(result.entries.len(), 2);
        match &result.entries[0] {
            (LuaKey::Integer(1), LuaValue::String(s)) => assert_eq!(s, "Young Wolf"),
            _ => panic!("unexpected entry"),
        }
    }

    #[test]
    fn test_nested_data() {
        let input = r#"pfDB["quests"]["data"] = {
  [1] = {
    ["start"] = {
      ["U"] = { 288 },
    },
    ["end"] = {
      ["U"] = { 272 },
    },
    ["lvl"] = 4,
    ["min"] = 1,
    ["race"] = 77,
  },
}"#;
        let result = parse_file(input).unwrap();
        assert_eq!(result.entries.len(), 1);
        let (LuaKey::Integer(1), entry) = &result.entries[0] else {
            panic!("expected integer key 1");
        };
        assert_eq!(entry.get_field("lvl").unwrap().as_number().unwrap(), 4.0);
        assert_eq!(entry.get_field("race").unwrap().as_number().unwrap(), 77.0);

        let start = entry.get_field("start").unwrap();
        let units = start.get_field("U").unwrap().as_int_array();
        assert_eq!(units, vec![288]);
    }

    #[test]
    fn test_comments() {
        let input = r#"pfDB["quests"]["data-epoch"] = {
  -- This is a comment
  [100] = {
    ["lvl"] = 5, -- inline comment
  },
}"#;
        let result = parse_file(input).unwrap();
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn test_negative_numbers() {
        let input = r#"pfDB["zones"]["data"] = {
  [1] = { 0, 36.56, 24.53, -18.71, -25.68 },
}"#;
        let result = parse_file(input).unwrap();
        let (_, entry) = &result.entries[0];
        let values: Vec<f64> = entry
            .as_table()
            .unwrap()
            .iter()
            .filter_map(|(_, v)| v.as_number())
            .collect();
        assert_eq!(values.len(), 5);
        assert!(values[3] < 0.0);
    }

    #[test]
    fn test_empty_table() {
        let input = r#"pfDB["items"]["data"] = {
  [100] = {},
}"#;
        let result = parse_file(input).unwrap();
        let (_, entry) = &result.entries[0];
        assert_eq!(entry.as_table().unwrap().len(), 0);
    }

    #[test]
    fn test_string_escapes() {
        let input = r#"pfDB["quests"]["enUS"] = {
  [1] = {
    ["T"] = "Miner\'s Fortune",
    ["O"] = "Find the cave.\nKill the boss.",
    ["D"] = "Simple description",
  },
}"#;
        let result = parse_file(input).unwrap();
        let (_, entry) = &result.entries[0];
        let title = entry.get_field("T").unwrap().as_string().unwrap();
        assert_eq!(title, "Miner's Fortune");
        let obj = entry.get_field("O").unwrap().as_string().unwrap();
        assert!(obj.contains('\n'));
    }

    #[test]
    fn test_no_trailing_comma() {
        let input = r#"pfDB["items"]["data-epoch"] = {
  [100] = {
    ["U"] = {5623}
  }
}"#;
        let result = parse_file(input).unwrap();
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn test_next_as_table_or_number() {
        let input = r#"pfDB["quests"]["data"] = {
  [1] = {
    ["next"] = 465,
  },
  [2] = {
    ["next"] = { 26345 },
  },
}"#;
        let result = parse_file(input).unwrap();
        let (_, e1) = &result.entries[0];
        assert_eq!(e1.get_field("next").unwrap().as_number().unwrap(), 465.0);
        let (_, e2) = &result.entries[1];
        let next_table = e2.get_field("next").unwrap().as_int_array();
        assert_eq!(next_table, vec![26345]);
    }
}
