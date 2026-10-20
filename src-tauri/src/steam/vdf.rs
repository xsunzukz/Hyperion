use std::collections::HashMap;

/// A node in a Valve KeyValues (VDF / ACF) tree
#[derive(Debug, Clone, PartialEq)]
pub enum VdfValue {
    String(String),
    Object(HashMap<String, VdfValue>),
}

impl VdfValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            VdfValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&HashMap<String, VdfValue>> {
        match self {
            VdfValue::Object(map) => Some(map),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&VdfValue> {
        self.as_object()?.get(key)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }
}

/// Tokenizer for VDF / ACF KeyValues format
#[derive(Debug, Clone, PartialEq)]
enum Token {
    String(String),
    OpenBrace,
    CloseBrace,
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        // Skip whitespace
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // Single-line comment `// ...`
        if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            i += 2;
            while i < len && chars[i] != '\n' && chars[i] != '\r' {
                i += 1;
            }
            continue;
        }

        // Structural tokens
        if c == '{' {
            tokens.push(Token::OpenBrace);
            i += 1;
            continue;
        }
        if c == '}' {
            tokens.push(Token::CloseBrace);
            i += 1;
            continue;
        }

        // Quoted string
        if c == '"' {
            i += 1;
            let mut s = String::new();
            while i < len {
                let sc = chars[i];
                if sc == '\\' && i + 1 < len {
                    let next = chars[i + 1];
                    match next {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        '\\' => s.push('\\'),
                        '"' => s.push('"'),
                        _ => {
                            s.push('\\');
                            s.push(next);
                        }
                    }
                    i += 2;
                } else if sc == '"' {
                    i += 1; // consume closing quote
                    break;
                } else {
                    s.push(sc);
                    i += 1;
                }
            }
            tokens.push(Token::String(s));
            continue;
        }

        // Unquoted bare token (rare in standard VDF, but supported)
        let mut s = String::new();
        while i < len && !chars[i].is_whitespace() && chars[i] != '{' && chars[i] != '}' && chars[i] != '"' {
            s.push(chars[i]);
            i += 1;
        }
        if !s.is_empty() {
            tokens.push(Token::String(s));
        }
    }

    tokens
}

/// Parses a VDF/ACF string into a root KeyValues mapping
pub fn parse_vdf(input: &str) -> HashMap<String, VdfValue> {
    let tokens = tokenize(input);
    let mut cursor = 0;
    parse_object_body(&tokens, &mut cursor, false)
}

fn parse_object_body(tokens: &[Token], cursor: &mut usize, stop_at_close_brace: bool) -> HashMap<String, VdfValue> {
    let mut map = HashMap::new();

    while *cursor < tokens.len() {
        match &tokens[*cursor] {
            Token::CloseBrace => {
                if stop_at_close_brace {
                    *cursor += 1;
                    return map;
                } else {
                    *cursor += 1;
                }
            }
            Token::String(key) => {
                let key_str = key.clone();
                *cursor += 1;

                if *cursor >= tokens.len() {
                    break;
                }

                match &tokens[*cursor] {
                    Token::OpenBrace => {
                        *cursor += 1;
                        let child = parse_object_body(tokens, cursor, true);
                        map.insert(key_str, VdfValue::Object(child));
                    }
                    Token::String(val) => {
                        let val_str = val.clone();
                        *cursor += 1;
                        map.insert(key_str, VdfValue::String(val_str));
                    }
                    Token::CloseBrace => {
                        // Malformed or empty key value, skip
                        *cursor += 1;
                    }
                }
            }
            Token::OpenBrace => {
                // Unexpected open brace without preceding key, step over
                *cursor += 1;
            }
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_appmanifest() {
        let sample = r#"
"AppState"
{
    "appid"       "730"
    "Universe"    "1"
    "name"        "Counter-Strike 2"
    "installdir"  "Counter-Strike Global Offensive"
    "SizeOnDisk"  "35249582080"
    "StateFlags"  "4"
}
"#;
        let vdf = parse_vdf(sample);
        let app_state = vdf.get("AppState").expect("AppState root");
        assert_eq!(app_state.get_str("appid"), Some("730"));
        assert_eq!(app_state.get_str("name"), Some("Counter-Strike 2"));
        assert_eq!(app_state.get_str("SizeOnDisk"), Some("35249582080"));
        assert_eq!(app_state.get_str("StateFlags"), Some("4"));
    }

    #[test]
    fn test_parse_libraryfolders() {
        let sample = r#"
"libraryfolders"
{
    "0"
    {
        "path" "C:\\Program Files (x86)\\Steam"
        "apps"
        {
            "730" "35249582080"
        }
    }
    "1"
    {
        "path" "F:\\Steam"
        "apps"
        {
            "1091500" "71239120480"
        }
    }
}
"#;
        let vdf = parse_vdf(sample);
        let libs = vdf.get("libraryfolders").expect("libraryfolders");
        let lib1 = libs.get("1").expect("folder 1");
        assert_eq!(lib1.get_str("path"), Some("F:\\Steam"));
        let apps = lib1.get("apps").expect("apps");
        assert!(apps.get("1091500").is_some());
    }
}
