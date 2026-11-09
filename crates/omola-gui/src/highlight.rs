//! Small Modelica 3.6 tokenizer used by the text editor.

use std::ops::Range;

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

const KEYWORDS: &str = "algorithm and annotation block break class connect constant constrainedby der discrete each else elseif elsewhen encapsulated end enumeration equation expandable extends external false final flow for function if import impure in initial inner input loop model not operator or outer output package parameter partial protected public pure record redeclare replaceable return stream then true type when while within";
const TYPES: &str = "Real Integer Boolean String";
const BUILTINS: &str = "abs sign sqrt sin cos tan asin acos atan atan2 sinh cosh tanh exp log log10 floor ceil integer min max sum product size ndims zeros ones fill identity linspace der pre edge change reinit sample noEvent smooth homotopy assert terminate cardinality semiLinear delay";

/// Tokenize all of `text` into non-overlapping byte ranges that preserve every character.
pub fn tokenize(text: &str) -> Vec<(Range<usize>, TokenKind)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let c = chars[i].1;
        let kind;
        if c == '/' && chars.get(i + 1).is_some_and(|(_, next)| *next == '/') {
            i += 2;
            while i < chars.len() && chars[i].1 != '\n' {
                i += 1;
            }
            if i < chars.len() {
                i += 1;
            }
            kind = TokenKind::Comment;
        } else if c == '/' && chars.get(i + 1).is_some_and(|(_, next)| *next == '*') {
            i += 2;
            while i < chars.len() {
                if chars[i].1 == '*' && chars.get(i + 1).is_some_and(|(_, next)| *next == '/') {
                    i += 2;
                    break;
                }
                i += 1;
            }
            kind = TokenKind::Comment;
        } else if c == '"' {
            i += 1;
            while i < chars.len() {
                if chars[i].1 == '\\'
                    && chars
                        .get(i + 1)
                        .is_some_and(|(_, next)| matches!(*next, '"' | '\\'))
                {
                    i += 2;
                } else if chars[i].1 == '"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            kind = TokenKind::String;
        } else if c == '\'' {
            i += 1;
            while i < chars.len() {
                let current = chars[i].1;
                i += 1;
                if current == '\'' {
                    break;
                }
            }
            kind = TokenKind::QuotedIdent;
        } else if c.is_ascii_digit()
            || (c == '.'
                && chars
                    .get(i + 1)
                    .is_some_and(|(_, next)| next.is_ascii_digit()))
        {
            i = number_end(&chars, i);
            kind = TokenKind::Number;
        } else if is_ident_start(c) {
            i += 1;
            while i < chars.len() && is_ident_continue(chars[i].1) {
                i += 1;
            }
            let word: String = chars[start..i].iter().map(|(_, c)| c).collect();
            kind = if TYPES.split_ascii_whitespace().any(|entry| entry == word) {
                TokenKind::Type
            } else if BUILTINS.split_ascii_whitespace().any(|entry| entry == word) {
                TokenKind::Builtin
            } else if KEYWORDS.split_ascii_whitespace().any(|entry| entry == word) {
                TokenKind::Keyword
            } else {
                TokenKind::Plain
            };
        } else if is_operator(c) {
            i += 1;
            if matches!(c, ':' | '=' | '<' | '>')
                && chars.get(i).is_some_and(|(_, next)| {
                    matches!(
                        (c, *next),
                        (':', '=') | ('=', '=') | ('<', '=') | ('>', '=') | ('<', '>')
                    )
                })
            {
                i += 1;
            }
            kind = TokenKind::Operator;
        } else {
            i += 1;
            kind = TokenKind::Plain;
        }
        let byte_start = chars[start].0;
        let byte_end = chars.get(i).map_or(text.len(), |(byte, _)| *byte);
        tokens.push((byte_start..byte_end, kind));
    }
    tokens
}

fn number_end(chars: &[(usize, char)], start: usize) -> usize {
    let mut i = start;
    if chars[i].1 == '.' {
        i += 1;
        while i < chars.len() && chars[i].1.is_ascii_digit() {
            i += 1;
        }
    } else {
        while i < chars.len() && chars[i].1.is_ascii_digit() {
            i += 1;
        }
        if i + 1 < chars.len() && chars[i].1 == '.' && chars[i + 1].1.is_ascii_digit() {
            i += 1;
            while i < chars.len() && chars[i].1.is_ascii_digit() {
                i += 1;
            }
        }
    }
    if i < chars.len() && matches!(chars[i].1, 'e' | 'E') {
        let mut exponent = i + 1;
        if exponent < chars.len() && matches!(chars[exponent].1, '+' | '-') {
            exponent += 1;
        }
        let digits = exponent;
        while exponent < chars.len() && chars[exponent].1.is_ascii_digit() {
            exponent += 1;
        }
        if exponent > digits {
            i = exponent;
        }
    }
    i
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
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
            | ':'
            | ';'
            | ','
            | '.'
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

    fn covered(text: &str) -> Vec<(String, TokenKind)> {
        let tokens = tokenize(text);
        let mut next = 0;
        let mut found = Vec::new();
        for (range, kind) in tokens {
            assert_eq!(range.start, next, "gap or overlap at {range:?} in {text:?}");
            assert!(range.start < range.end);
            found.push((text[range.clone()].to_owned(), kind));
            next = range.end;
        }
        assert_eq!(next, text.len());
        found
    }

    #[test]
    fn tokenizes_modelica_keywords_types_builtins_strings_and_numbers() {
        let tokens =
            covered("model M \"doc\" Real x(start=1.5e-3); 1; equation der(x) = -x; end M;");
        for (word, kind) in [
            ("model", TokenKind::Keyword),
            ("equation", TokenKind::Keyword),
            ("end", TokenKind::Keyword),
            ("Real", TokenKind::Type),
            ("der", TokenKind::Builtin),
            ("\"doc\"", TokenKind::String),
            ("1.5e-3", TokenKind::Number),
            ("1", TokenKind::Number),
        ] {
            assert!(
                tokens.contains(&(word.to_owned(), kind)),
                "missing {word} as {kind:?}"
            );
        }
    }

    #[test]
    fn comments_and_non_ascii_text_are_safe_and_fully_covered() {
        let tokens = covered("/* 日本語 */ x");
        assert_eq!(tokens[0], ("/* 日本語 */".into(), TokenKind::Comment));
        assert_eq!(tokens[2], ("x".into(), TokenKind::Plain));
    }

    #[test]
    fn unterminated_strings_and_comments_reach_end_without_panicking() {
        assert_eq!(covered("\"abc"), vec![("\"abc".into(), TokenKind::String)]);
        assert_eq!(
            covered("/* abc"),
            vec![("/* abc".into(), TokenKind::Comment)]
        );
    }

    #[test]
    fn quoted_identifiers_and_identifier_boundaries_are_preserved() {
        let tokens = covered("'quoted name' Real2");
        assert_eq!(tokens[0], ("'quoted name'".into(), TokenKind::QuotedIdent));
        assert_eq!(tokens[2], ("Real2".into(), TokenKind::Plain));
    }

    #[test]
    fn strings_handle_escaped_quote_and_backslash() {
        let tokens = covered(r#""a\"b\\c"#);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].1, TokenKind::String);
    }
}
