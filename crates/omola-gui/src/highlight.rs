//! Modelica syntax highlighting for the Text layer.

/// Colour class of one token. Every byte of the text belongs to exactly one token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    Type,
    Comment,
    String,
    Number,
    Operator,
    Plain,
}

/// Reserved words of Modelica 3.6 (specification 2.3.3).
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
    "der",
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

/// Tokenize `text` into contiguous byte ranges. Ranges cover the whole string.
pub fn tokenize(text: &str) -> Vec<(std::ops::Range<usize>, TokenKind)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let (len, kind) = if rest.starts_with("//") {
            (rest.find('\n').unwrap_or(rest.len()), TokenKind::Comment)
        } else if let Some(after) = rest.strip_prefix("/*") {
            (
                after.find("*/").map(|p| p + 4).unwrap_or(rest.len()),
                TokenKind::Comment,
            )
        } else {
            let ch = rest.chars().next().expect("non-empty remainder");
            if ch == '"' {
                (string_len(rest), TokenKind::String)
            } else if ch.is_ascii_digit() {
                (number_len(rest), TokenKind::Number)
            } else if ch.is_alphabetic() || ch == '_' {
                let len = rest
                    .char_indices()
                    .take_while(|(_, c)| c.is_alphanumeric() || *c == '_')
                    .last()
                    .map(|(p, c)| p + c.len_utf8())
                    .unwrap_or(ch.len_utf8());
                let word = &rest[..len];
                let kind = if KEYWORDS.contains(&word) {
                    TokenKind::Keyword
                } else if matches!(word, "Real" | "Integer" | "Boolean" | "String") {
                    TokenKind::Type
                } else {
                    TokenKind::Plain
                };
                (len, kind)
            } else if ch.is_whitespace() {
                let len = rest
                    .char_indices()
                    .take_while(|(_, c)| c.is_whitespace())
                    .last()
                    .map(|(p, c)| p + c.len_utf8())
                    .unwrap_or(ch.len_utf8());
                (len, TokenKind::Plain)
            } else {
                (ch.len_utf8(), TokenKind::Operator)
            }
        };
        out.push((i..i + len, kind));
        i += len;
    }
    out
}

/// Length of a string literal at the start of `rest` (including the quotes).
/// An unterminated literal runs to the end of the text.
fn string_len(rest: &str) -> usize {
    let mut escaped = false;
    for (i, ch) in rest.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return i + ch.len_utf8();
        }
    }
    rest.len()
}

/// Length of a number literal at the start of `rest` (`1`, `1.5`, `1e-3`, `2.0E+5`).
fn number_len(rest: &str) -> usize {
    let bytes = rest.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
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

pub fn color(kind: TokenKind, dark: bool) -> egui::Color32 {
    let (keyword, ty, comment, string, number, operator, plain) = if dark {
        (
            (0x56, 0x9C, 0xD6),
            (0x4E, 0xC9, 0xB0),
            (0x6A, 0x99, 0x55),
            (0xCE, 0x91, 0x78),
            (0xB5, 0xCE, 0xA8),
            (0xD4, 0xD4, 0xD4),
            (0xD4, 0xD4, 0xD4),
        )
    } else {
        (
            (0x00, 0x00, 0xFF),
            (0x26, 0x7F, 0x99),
            (0x00, 0x80, 0x00),
            (0xA3, 0x15, 0x15),
            (0x09, 0x86, 0x58),
            (0x00, 0x00, 0x00),
            (0x00, 0x00, 0x00),
        )
    };
    let (r, g, b) = match kind {
        TokenKind::Keyword => keyword,
        TokenKind::Type => ty,
        TokenKind::Comment => comment,
        TokenKind::String => string,
        TokenKind::Number => number,
        TokenKind::Operator => operator,
        TokenKind::Plain => plain,
    };
    egui::Color32::from_rgb(r, g, b)
}

/// Layout job for a whole editor buffer, one section per token.
pub fn layout_job(text: &str, font: egui::FontId, dark: bool) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for (range, kind) in tokenize(text) {
        job.append(
            &text[range],
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color: color(kind, dark),
                ..Default::default()
            },
        );
    }
    job
}

/// Number of lines shown for `text`, counting the empty line after a final newline.
pub fn line_count(text: &str) -> usize {
    text.split('\n').count().max(1)
}

/// Text for the line-number gutter, one number per line plus a trailing newline.
pub fn line_numbers(text: &str) -> String {
    let mut out = String::new();
    for n in 1..=line_count(text) {
        out.push_str(&n.to_string());
        out.push('\n');
    }
    out
}

/// Character index (as used by `TextEdit`) of a 1-based line and column.
///
/// A column past the end of the line goes to the line end.
pub fn char_index(text: &str, line: usize, column: usize) -> usize {
    let target = line.max(1);
    let mut start = 0usize;
    let mut current = 1usize;
    for (i, ch) in text.char_indices() {
        if current == target {
            break;
        }
        if ch == '\n' {
            current += 1;
            start = i + 1;
        }
    }
    if current < target {
        return text.chars().count();
    }
    let line_end = text[start..]
        .find('\n')
        .map(|p| start + p)
        .unwrap_or(text.len());
    let mut byte = line_end;
    for (n, (i, _)) in text[start..line_end].char_indices().enumerate() {
        if n + 1 == column.max(1) {
            byte = start + i;
            break;
        }
    }
    text[..byte].chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(String, TokenKind)> {
        tokenize(text)
            .into_iter()
            .map(|(range, kind)| (text[range].to_owned(), kind))
            .collect()
    }

    #[test]
    fn tokens_cover_the_whole_text() {
        for text in [
            "model M\n  Real x(start=1) \"comment\";\nend M;\n",
            "日本語のコメント // x = 1\ny = 2;",
            "\"unterminated",
            "/* open",
            "",
        ] {
            let tokens = tokenize(text);
            let mut at = 0;
            for (range, _) in &tokens {
                assert_eq!(range.start, at, "{text:?}");
                at = range.end;
            }
            assert_eq!(at, text.len(), "{text:?}");
        }
    }

    #[test]
    fn keywords_types_and_numbers() {
        let tokens = kinds("model M parameter Real x = 1.5e-3;");
        assert!(tokens.contains(&("model".into(), TokenKind::Keyword)));
        assert!(tokens.contains(&("parameter".into(), TokenKind::Keyword)));
        assert!(tokens.contains(&("Real".into(), TokenKind::Type)));
        assert!(tokens.contains(&("1.5e-3".into(), TokenKind::Number)));
    }

    #[test]
    fn comments_and_strings_do_not_highlight_their_contents() {
        let tokens = kinds("// model Real\nx = \"model 1\";");
        assert!(tokens.contains(&("// model Real".into(), TokenKind::Comment)));
        assert!(tokens.contains(&("\"model 1\"".into(), TokenKind::String)));
        assert!(!tokens.contains(&("model".into(), TokenKind::Keyword)));
    }

    #[test]
    fn multibyte_text_keeps_byte_ranges() {
        let text = "// 日本語のコメント\nReal x;";
        let tokens = tokenize(text);
        assert_eq!(tokens[0].0, 0.."// 日本語のコメント".len());
        assert_eq!(tokens[1].0.start, tokens[0].0.end);
    }

    #[test]
    fn escaped_quote_ends_the_string_later() {
        let tokens = kinds("\"a\\\"b\" + c");
        assert_eq!(tokens[0], ("\"a\\\"b\"".into(), TokenKind::String));
        assert!(tokens
            .iter()
            .any(|(t, k)| t == "c" && *k == TokenKind::Plain));
    }

    #[test]
    fn line_and_column_become_a_char_index() {
        let text = "model M\n// 日本語\n  Real x;\nend M;\n";
        assert_eq!(char_index(text, 1, 1), 0);
        assert_eq!(char_index(text, 2, 1), 8);
        assert_eq!(char_index(text, 3, 3), 17);
        // Past the end of the line -> end of that line, not the next one.
        assert_eq!(char_index(text, 2, 99), 14);
        assert_eq!(char_index(text, 99, 1), text.chars().count());
    }

    #[test]
    fn line_numbers_match_line_count() {
        assert_eq!(line_count(""), 1);
        assert_eq!(line_count("a"), 1);
        assert_eq!(line_count("a\n"), 2);
        assert_eq!(line_numbers("a\nb"), "1\n2\n");
    }
}
