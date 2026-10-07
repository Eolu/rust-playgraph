//! A small, dependency-free Rust syntax tokenizer used for highlighting.
//!
//! It intentionally covers the surface that appears in playground snippets
//! (keywords, types, numbers, strings, comments, attributes, macros and
//! lifetimes) rather than trying to be a full Rust lexer.

/// Classification of a highlighted token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// Anything unclassified: identifiers, whitespace, punctuation, operators.
    Plain,
    Comment,
    /// String, byte string, raw string, or char literal.
    Str,
    Number,
    Keyword,
    /// A capitalised identifier (type, trait, enum variant, const).
    Type,
    /// An identifier immediately followed by `!`, e.g. `println!`.
    Macro,
    /// An outer `#[...]` or inner `#![...]` attribute.
    Attribute,
    /// A lifetime such as `'a` or `'static`.
    Lifetime,
}

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "become", "box", "break", "const", "continue", "crate", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "if", "impl", "in", "let", "loop",
    "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return", "self",
    "Self", "static", "struct", "super", "trait", "true", "try", "type", "typeof", "unsafe",
    "unsized", "use", "virtual", "where", "while", "yield",
];

const PRIMITIVES: &[&str] = &[
    "bool", "char", "str", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16",
    "u32", "u64", "u128", "usize",
];

/// Split `source` into classified tokens. Adjacent tokens of the same kind are
/// merged so the result maps to the minimal number of highlighted spans.
pub fn tokenize(source: &str) -> Vec<(TokenKind, String)> {
    let mut out: Vec<(TokenKind, String)> = Vec::new();
    let len = source.len();
    let mut i = 0;

    while i < len {
        let c = source[i..].chars().next().unwrap();
        let rest = &source[i + c.len_utf8()..];

        if c == '/' && rest.starts_with('/') {
            let end = source[i..]
                .find('\n')
                .map(|offset| i + offset)
                .unwrap_or(len);
            push(&mut out, TokenKind::Comment, &source[i..end]);
            i = end;
        } else if c == '/' && rest.starts_with('*') {
            let mut depth = 0usize;
            let mut j = i;
            while j < len {
                if source[j..].starts_with("/*") {
                    depth += 1;
                    j += 2;
                } else if source[j..].starts_with("*/") {
                    depth -= 1;
                    j += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    j += next_len(source, j);
                }
            }
            push(&mut out, TokenKind::Comment, &source[i..j.min(len)]);
            i = j.min(len);
        } else if c == '#' && (rest.starts_with('[') || rest.starts_with("![")) {
            let mut depth = 0i32;
            let mut j = i + 1;
            while j < len {
                let ch = source[j..].chars().next().unwrap();
                if ch == '[' {
                    depth += 1;
                    j += 1;
                } else if ch == ']' {
                    depth -= 1;
                    j += 1;
                    if depth == 0 {
                        break;
                    }
                } else if ch == '"' {
                    j = skip_string(source, j);
                } else {
                    j += ch.len_utf8();
                }
            }
            push(&mut out, TokenKind::Attribute, &source[i..j]);
            i = j;
        } else if (c == 'r' || c == 'b')
            && let Some(end) = string_end(source, i)
        {
            push(&mut out, TokenKind::Str, &source[i..end]);
            i = end;
        } else if c == '"' {
            let end = skip_string(source, i);
            push(&mut out, TokenKind::Str, &source[i..end]);
            i = end;
        } else if c == '\'' {
            if is_char_literal(source, i) {
                let end = skip_char(source, i);
                push(&mut out, TokenKind::Str, &source[i..end]);
                i = end;
            } else {
                let mut j = i + 1;
                while j < len {
                    let ch = source[j..].chars().next().unwrap();
                    if ch == '_' || ch.is_alphanumeric() {
                        j += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                push(&mut out, TokenKind::Lifetime, &source[i..j]);
                i = j;
            }
        } else if c.is_ascii_digit() {
            let mut j = i;
            while j < len {
                let ch = source[j..].chars().next().unwrap();
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
                    j += ch.len_utf8();
                } else {
                    break;
                }
            }
            push(&mut out, TokenKind::Number, &source[i..j]);
            i = j;
        } else if c == '_' || c.is_alphabetic() {
            let mut j = i;
            while j < len {
                let ch = source[j..].chars().next().unwrap();
                if ch == '_' || ch.is_alphanumeric() {
                    j += ch.len_utf8();
                } else {
                    break;
                }
            }
            let word = &source[i..j];
            let after = &source[j..];
            if after.starts_with('!') && !after.starts_with("!=") {
                push(&mut out, TokenKind::Macro, &source[i..j + 1]);
                i = j + 1;
            } else {
                let kind = if KEYWORDS.contains(&word) {
                    TokenKind::Keyword
                } else if PRIMITIVES.contains(&word)
                    || word.chars().next().is_some_and(char::is_uppercase)
                {
                    TokenKind::Type
                } else {
                    TokenKind::Plain
                };
                push(&mut out, kind, word);
                i = j;
            }
        } else {
            let end = i + c.len_utf8();
            push(&mut out, TokenKind::Plain, &source[i..end]);
            i = end;
        }
    }

    out
}

fn push(out: &mut Vec<(TokenKind, String)>, kind: TokenKind, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut()
        && last.0 == kind
    {
        last.1.push_str(text);
        return;
    }
    out.push((kind, text.to_string()));
}

fn next_len(source: &str, i: usize) -> usize {
    source[i..].chars().next().map(char::len_utf8).unwrap_or(1)
}

/// End index (exclusive) of a `"..."` string starting at `start` (the quote).
fn skip_string(source: &str, start: usize) -> usize {
    let len = source.len();
    let mut j = start + 1;
    while j < len {
        let ch = source[j..].chars().next().unwrap();
        if ch == '\\' {
            j += 1;
            j += next_len(source, j);
        } else if ch == '"' {
            return j + 1;
        } else {
            j += ch.len_utf8();
        }
    }
    len
}

/// End index of a `'...'` char literal starting at `start` (the quote).
fn skip_char(source: &str, start: usize) -> usize {
    let mut j = start + 1;
    if source[j..].starts_with('\\') {
        j += 1;
        j += next_len(source, j);
    } else {
        j += next_len(source, j);
    }
    if source[j..].starts_with('\'') {
        j += 1;
    }
    j
}

/// Distinguish `'a'`/`'\n'` (char literal) from `'a`/`'static` (lifetime).
fn is_char_literal(source: &str, start: usize) -> bool {
    let after = start + 1;
    let Some(c) = source[after..].chars().next() else {
        return false;
    };
    if c == '\\' {
        return true;
    }
    source[after + c.len_utf8()..].starts_with('\'')
}

/// If a byte/raw string starts at `start`, return its end index (exclusive).
fn string_end(source: &str, start: usize) -> Option<usize> {
    let rest = &source[start..];
    if rest.starts_with("b\"") {
        return Some(skip_string(source, start + 1));
    }
    let r_at = if rest.starts_with('r') {
        start
    } else if rest.starts_with("br") {
        start + 1
    } else {
        return None;
    };

    let bytes = source.as_bytes();
    let mut j = r_at + 1;
    let mut hashes = 0;
    while bytes.get(j) == Some(&b'#') {
        hashes += 1;
        j += 1;
    }
    if bytes.get(j) != Some(&b'"') {
        return None;
    }
    j += 1;
    let close = format!("\"{}", "#".repeat(hashes));
    match source[j..].find(&close) {
        Some(offset) => Some(j + offset + close.len()),
        None => Some(source.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind_of(tokens: &[(TokenKind, String)], text: &str) -> Option<TokenKind> {
        tokens
            .iter()
            .find(|(_, value)| value == text)
            .map(|(kind, _)| *kind)
    }

    #[test]
    fn classifies_basics() {
        let tokens = tokenize("fn main() { let x: u32 = 42; // hi\n}");
        assert_eq!(kind_of(&tokens, "fn"), Some(TokenKind::Keyword));
        assert_eq!(kind_of(&tokens, "let"), Some(TokenKind::Keyword));
        assert_eq!(kind_of(&tokens, "u32"), Some(TokenKind::Type));
        assert_eq!(kind_of(&tokens, "42"), Some(TokenKind::Number));
        assert!(
            tokens
                .iter()
                .any(|(kind, value)| *kind == TokenKind::Comment && value.contains("hi"))
        );
    }

    #[test]
    fn distinguishes_char_and_lifetime() {
        let tokens = tokenize("'a' 'static '_");
        assert_eq!(kind_of(&tokens, "'a'"), Some(TokenKind::Str));
        assert_eq!(kind_of(&tokens, "'static"), Some(TokenKind::Lifetime));
        assert_eq!(kind_of(&tokens, "'_"), Some(TokenKind::Lifetime));
    }

    #[test]
    fn macros_and_attributes() {
        let tokens = tokenize("#[derive(Clone)]\nprintln!(\"{}\", x);");
        assert!(
            tokens
                .iter()
                .any(|(kind, value)| *kind == TokenKind::Attribute && value == "#[derive(Clone)]")
        );
        assert_eq!(kind_of(&tokens, "println!"), Some(TokenKind::Macro));
    }

    #[test]
    fn raw_and_byte_strings() {
        let tokens = tokenize(r##"let a = r#"x"#; let b = b"y";"##);
        assert!(
            tokens
                .iter()
                .any(|(kind, value)| *kind == TokenKind::Str && value == "r#\"x\"#")
        );
        assert!(
            tokens
                .iter()
                .any(|(kind, value)| *kind == TokenKind::Str && value == "b\"y\"")
        );
    }

    #[test]
    fn merges_adjacent_plain_runs() {
        let tokens = tokenize("a + b + c");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].0, TokenKind::Plain);
    }
}
