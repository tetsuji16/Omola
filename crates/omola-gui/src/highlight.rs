//! A Modelica tokenizer for syntax colouring (G24).
//!
//! `syntect` was used with an Ada grammar before, because it has no Modelica definition;
//! Modelica keywords were coloured wrong and strings inside `annotation` broke. This is a
//! small hand-written scanner instead: it walks the text once, character by character, so
//! it cannot get stuck on regex backtracking and it never splits a multi-byte character.

use std::ops::Range;

/// What a piece of Modelica text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    Type,
    Builtin,
    Comment,
    String,
    Number,
    QuotedIdent,
    Operator,
    Plain,
}

/// Whether `word` is a Modelica reserved word (G31 rejects these as component names).
pub fn is_keyword(word: &str) -> bool {
    KEYWORDS.contains(&word)
}

/// Modelica 3.6 reserved words.
///
/// `der` is in the specification's keyword list but is a built-in operator function, so it
/// is coloured as one (it is listed in [`BUILTINS`] instead).
const KEYWORDS: &[&str] = &[
    "algorithm",
    "and",
    "annotation",
    "block",
    "break",
    "class",
    "connect",
    "connector",
    "constant",
    "constrainedby",
    "discrete",
    "each",
    "else",
    "elseif",
    "elsewhen",
    "encapsulated",
    "end",
    "enumeration",
    "equation",
    "expandable",
    "extends",
    "external",
    "false",
    "final",
    "flow",
    "for",
    "function",
    "if",
    "import",
    "impure",
    "in",
    "initial",
    "inner",
    "input",
    "loop",
    "model",
    "not",
    "operator",
    "or",
    "outer",
    "output",
    "package",
    "parameter",
    "partial",
    "protected",
    "public",
    "pure",
    "record",
    "redeclare",
    "replaceable",
    "return",
    "stream",
    "then",
    "true",
    "type",
    "when",
    "while",
    "within",
];

/// Built-in types.
const TYPES: &[&str] = &["Real", "Integer", "Boolean", "String"];

/// Built-in operators and functions of the Modelica language.
const BUILTINS: &[&str] = &[
    "abs",
    "sign",
    "sqrt",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "atan2",
    "sinh",
    "cosh",
    "tanh",
    "exp",
    "log",
    "log10",
    "floor",
    "ceil",
    "integer",
    "min",
    "max",
    "sum",
    "product",
    "size",
    "ndims",
    "zeros",
    "ones",
    "fill",
    "identity",
    "linspace",
    "der",
    "pre",
    "edge",
    "change",
    "reinit",
    "sample",
    "noEvent",
    "smooth",
    "homotopy",
    "assert",
    "terminate",
    "cardinality",
    "semiLinear",
    "delay",
];

/// `(byte range, kind)` covering `text` without gaps or overlaps.
pub fn tokenize(text: &str) -> Vec<(Range<usize>, TokenKind)> {
    let mut out: Vec<(Range<usize>, TokenKind)> = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let c = rest.chars().next().expect("i is on a character boundary");

        if rest.starts_with("//") {
            let end = rest.find('\n').map_or(text.len(), |p| i + p);
            push(&mut out, i..end, TokenKind::Comment);
            i = end;
            continue;
        }
        if let Some(after) = rest.strip_prefix("/*") {
            let end = after.find("*/").map_or(text.len(), |p| i + 2 + p + 2);
            push(&mut out, i..end, TokenKind::Comment);
            i = end;
            continue;
        }
        if c == '"' || c == '\'' {
            let kind = if c == '"' {
                TokenKind::String
            } else {
                TokenKind::QuotedIdent
            };
            let end = scan_quoted(text, i, c);
            push(&mut out, i..end, kind);
            i = end;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && rest[1..].starts_with(|d: char| d.is_ascii_digit())) {
            let end = scan_number(text, i);
            push(&mut out, i..end, TokenKind::Number);
            i = end;
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let end = text[i..]
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .map_or(text.len(), |p| i + p);
            let word = &text[i..end];
            let kind = if KEYWORDS.contains(&word) {
                TokenKind::Keyword
            } else if TYPES.contains(&word) {
                TokenKind::Type
            } else if BUILTINS.contains(&word) {
                TokenKind::Builtin
            } else {
                TokenKind::Plain
            };
            push(&mut out, i..end, kind);
            i = end;
            continue;
        }
        let kind = if is_operator(c) {
            TokenKind::Operator
        } else {
            TokenKind::Plain
        };
        push(&mut out, i..i + c.len_utf8(), kind);
        i += c.len_utf8();
    }
    out
}

/// Append a token, merging it with the previous one when both are contiguous and equal.
fn push(out: &mut Vec<(Range<usize>, TokenKind)>, range: Range<usize>, kind: TokenKind) {
    if range.start >= range.end {
        return;
    }
    if let Some((last, last_kind)) = out.last_mut() {
        if *last_kind == kind && last.end == range.start {
            last.end = range.end;
            return;
        }
    }
    out.push((range, kind));
}

/// End of a `"..."` or `'...'` token starting at `start` (unterminated: end of text).
///
/// A backslash escapes the next character, so `"a\"b"` is one string.
fn scan_quoted(text: &str, start: usize, quote: char) -> usize {
    let mut i = start + quote.len_utf8();
    while i < text.len() {
        let c = text[i..].chars().next().expect("on a boundary");
        if c == '\\' {
            i += 1;
            if let Some(next) = text[i..].chars().next() {
                i += next.len_utf8();
            }
            continue;
        }
        i += c.len_utf8();
        if c == quote {
            return i;
        }
    }
    text.len()
}

/// End of a number starting at `start`: `12`, `1.5`, `1e-3`, `2.5E+10`, `.5`.
fn scan_number(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut i = start;
    if bytes[i] == b'.' {
        // `.5`
        i += 1;
    } else {
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'.' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        let mut j = i + 1;
        if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        if j < bytes.len() && bytes[j].is_ascii_digit() {
            i = j;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
    }
    i
}

fn is_operator(c: char) -> bool {
    matches!(
        c,
        '+' | '-'
            | '*'
            | '/'
            | '^'
            | '='
            | '<'
            | '>'
            | '.'
            | ','
            | ';'
            | ':'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tokens must cover the whole text exactly once.
    fn assert_contiguous(text: &str) {
        let tokens = tokenize(text);
        let mut at = 0;
        for (range, _) in &tokens {
            assert_eq!(range.start, at, "gap or overlap at {at} in {text:?}");
            assert!(range.end > range.start);
            at = range.end;
        }
        assert_eq!(at, text.len(), "the tokens do not reach the end");
    }

    fn kinds(text: &str, needle: &str) -> Vec<TokenKind> {
        let at = text.find(needle).expect("needle in text");
        tokenize(text)
            .into_iter()
            .filter(|(range, _)| range.start <= at && at < range.end)
            .map(|(_, kind)| kind)
            .collect()
    }

    #[test]
    fn every_byte_is_covered() {
        for text in [
            "",
            "model M end M;",
            "// only a comment",
            "/* unterminated",
            "\"unterminated",
            "model 日本語\n  Real x;\nend 日本語;",
            "x = .5 + 1e-3;",
        ] {
            assert_contiguous(text);
        }
    }

    #[test]
    fn keywords_types_builtins_strings_and_numbers() {
        let text = "model M \"doc\" Real x(start=1.5e-3); equation der(x) = -x; end M;";
        assert_eq!(kinds(text, "model"), [TokenKind::Keyword]);
        assert_eq!(kinds(text, "equation"), [TokenKind::Keyword]);
        assert_eq!(kinds(text, "end"), [TokenKind::Keyword]);
        assert_eq!(kinds(text, "Real"), [TokenKind::Type]);
        assert_eq!(kinds(text, "der"), [TokenKind::Builtin]);
        assert_eq!(kinds(text, "\"doc\""), [TokenKind::String]);
        assert_eq!(kinds(text, "1.5e-3"), [TokenKind::Number]);
        assert_eq!(kinds(text, "start"), [TokenKind::Plain]);
        // The `-` before `x` is an operator; the identifier itself stays plain.
        assert_eq!(kinds(text, "-x"), [TokenKind::Operator]);
        assert_eq!(kinds(text, "x; end"), [TokenKind::Plain]);
    }

    #[test]
    fn comments_may_contain_any_text() {
        let text = "/* 日本語 */ x";
        let tokens = tokenize(text);
        assert_eq!(tokens[0].1, TokenKind::Comment);
        assert_eq!(&text[tokens[0].0.clone()], "/* 日本語 */");
        assert_eq!(tokens[1].1, TokenKind::Plain);
        // A line comment stops at the newline.
        let text = "// a\nx";
        assert_eq!(&text[tokenize(text)[0].0.clone()], "// a");
    }

    #[test]
    fn unterminated_constructs_do_not_panic() {
        assert_eq!(kinds("\"abc", "\"abc"), [TokenKind::String]);
        assert_eq!(kinds("/* abc", "/*"), [TokenKind::Comment]);
        assert_eq!(kinds("'abc", "'"), [TokenKind::QuotedIdent]);
        // An escaped quote does not end the string.
        let text = r#""a\"b" x"#;
        assert_eq!(kinds(text, r#""a\"b""#), [TokenKind::String]);
        assert_eq!(kinds(text, "x"), [TokenKind::Plain]);
    }

    #[test]
    fn quoted_identifiers_are_their_own_kind() {
        let text = "'quoted name' x";
        let tokens = tokenize(text);
        assert_eq!(&text[tokens[0].0.clone()], "'quoted name'");
        assert_eq!(tokens[0].1, TokenKind::QuotedIdent);
    }

    #[test]
    fn a_prefix_of_a_keyword_is_plain() {
        assert_eq!(kinds("Real2", "Real2"), [TokenKind::Plain]);
        assert_eq!(kinds("modelled", "modelled"), [TokenKind::Plain]);
        assert_eq!(kinds("x1", "x1"), [TokenKind::Plain]);
    }

    #[test]
    fn numbers_keep_their_shape() {
        let text = "12 1.5 1e-3 2.5E+10 .5";
        let numbers: Vec<&str> = tokenize(text)
            .into_iter()
            .filter(|(_, kind)| *kind == TokenKind::Number)
            .map(|(range, _)| &text[range])
            .collect();
        assert_eq!(numbers, ["12", "1.5", "1e-3", "2.5E+10", ".5"]);
    }
}
