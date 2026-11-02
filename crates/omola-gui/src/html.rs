//! A small HTML subset parser for Modelica `Documentation(info=...)` strings.
//!
//! It produces block-level structure (paragraphs, headings, lists, tables, images,
//! preformatted text) with styled inline runs. It is deliberately forgiving: unknown tags
//! are dropped but their text is kept, unclosed tags are closed at the end, and scripts or
//! styles are discarded. No network or script execution ever happens here.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub code: bool,
    pub sup: bool,
    pub sub: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text {
        text: String,
        style: Style,
        link: Option<String>,
    },
    LineBreak,
    Image(ImageRef),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageRef {
    pub src: String,
    pub alt: String,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Paragraph(Vec<Inline>),
    Heading(u8, Vec<Inline>),
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
    },
    Table(Vec<Vec<TableCell>>),
    Preformatted(String),
    Rule,
    Quote(Vec<Block>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableCell {
    pub header: bool,
    pub blocks: Vec<Block>,
}

/// Parse an HTML fragment (with or without `<html>`/`<body>`) into blocks.
pub fn parse(html: &str) -> Vec<Block> {
    let tokens = tokenize(html);
    let mut parser = Parser { tokens, pos: 0 };
    parser.blocks_until(&[])
}

/// Plain text of blocks (for search and copying).
#[cfg_attr(not(test), allow(dead_code))]
pub fn plain_text(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        match block {
            Block::Paragraph(inl) | Block::Heading(_, inl) => {
                inline_text(inl, &mut out);
                out.push('\n');
            }
            Block::List { items, .. } => {
                for item in items {
                    out.push_str("- ");
                    out.push_str(&plain_text(item));
                }
            }
            Block::Table(rows) => {
                for row in rows {
                    let cells: Vec<String> = row
                        .iter()
                        .map(|c| plain_text(&c.blocks).trim().to_owned())
                        .collect();
                    out.push_str(&cells.join("\t"));
                    out.push('\n');
                }
            }
            Block::Preformatted(t) => {
                out.push_str(t);
                out.push('\n');
            }
            Block::Rule => out.push_str("----\n"),
            Block::Quote(inner) => out.push_str(&plain_text(inner)),
        }
    }
    out
}

#[cfg_attr(not(test), allow(dead_code))]
fn inline_text(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text { text, .. } => out.push_str(text),
            Inline::LineBreak => out.push('\n'),
            Inline::Image(img) => out.push_str(&img.alt),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Tokenizer

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Open {
        name: String,
        attrs: Vec<(String, String)>,
        self_closing: bool,
    },
    Close(String),
    Text(String),
}

fn tokenize(html: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let bytes = html.as_bytes();
    let mut i = 0;
    let mut text_start = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // Comments and doctype.
        if html[i..].starts_with("<!--") {
            push_text(&mut out, &html[text_start..i]);
            i = html[i..]
                .find("-->")
                .map(|e| i + e + 3)
                .unwrap_or(bytes.len());
            text_start = i;
            continue;
        }
        if html[i..].starts_with("<!") || html[i..].starts_with("<?") {
            push_text(&mut out, &html[text_start..i]);
            i = html[i..]
                .find('>')
                .map(|e| i + e + 1)
                .unwrap_or(bytes.len());
            text_start = i;
            continue;
        }
        let next = bytes.get(i + 1).copied().unwrap_or(b' ');
        if !(next.is_ascii_alphabetic() || next == b'/') {
            i += 1; // a literal '<'
            continue;
        }
        let Some(end) = find_tag_end(html, i) else {
            break;
        };
        push_text(&mut out, &html[text_start..i]);
        let inner = &html[i + 1..end];
        if let Some(name) = inner.strip_prefix('/') {
            out.push(Token::Close(tag_name(name)));
        } else {
            let self_closing = inner.trim_end().ends_with('/');
            let inner = inner.trim_end().trim_end_matches('/');
            let name = tag_name(inner);
            let attrs = parse_attrs(&inner[name.len().min(inner.len())..]);
            // Drop the content of script/style entirely.
            if name == "script" || name == "style" {
                let close = format!("</{name}");
                let lower = html[end..].to_ascii_lowercase();
                let skip = lower.find(&close).map(|e| end + e).unwrap_or(bytes.len());
                i = html[skip..]
                    .find('>')
                    .map(|e| skip + e + 1)
                    .unwrap_or(bytes.len());
                text_start = i;
                continue;
            }
            out.push(Token::Open {
                name,
                attrs,
                self_closing,
            });
        }
        i = end + 1;
        text_start = i;
    }
    push_text(&mut out, &html[text_start.min(html.len())..]);
    out
}

fn find_tag_end(html: &str, start: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (offset, b) in html.as_bytes()[start..].iter().enumerate() {
        match (quote, *b) {
            (Some(q), c) if c == q => quote = None,
            (None, b'"' | b'\'') => quote = Some(*b),
            (None, b'>') => return Some(start + offset),
            _ => {}
        }
    }
    None
}

fn tag_name(s: &str) -> String {
    s.trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '=' {
            i += 1;
        }
        let name: String = chars[start..i]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase();
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < chars.len() && chars[i] == '=' {
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                let q = chars[i];
                i += 1;
                let vs = i;
                while i < chars.len() && chars[i] != q {
                    i += 1;
                }
                value = chars[vs..i].iter().collect();
                i += 1;
            } else {
                let vs = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                value = chars[vs..i].iter().collect();
            }
        }
        if !name.is_empty() {
            out.push((name, decode_entities(&value)));
        } else {
            i += 1;
        }
    }
    out
}

fn push_text(out: &mut Vec<Token>, raw: &str) {
    if !raw.is_empty() {
        out.push(Token::Text(decode_entities(raw)));
    }
}

/// Decode named and numeric character references.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        let semi = tail[..tail.len().min(12)].find(';');
        if let Some(semi) = semi {
            let name = &tail[1..semi];
            if let Some(c) = entity(name) {
                out.push_str(&c);
                rest = &tail[semi + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &tail[1..];
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<String> {
    if let Some(num) = name.strip_prefix('#') {
        let code = if let Some(hex) = num.strip_prefix(['x', 'X']) {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            num.parse().ok()?
        };
        return char::from_u32(code).map(String::from);
    }
    let c = match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" | "#39" => "'",
        "nbsp" => "\u{a0}",
        "reg" => "®",
        "copy" => "©",
        "trade" => "™",
        "deg" => "°",
        "micro" => "µ",
        "middot" => "·",
        "times" => "×",
        "divide" => "÷",
        "plusmn" => "±",
        "minus" => "−",
        "ndash" => "–",
        "mdash" => "—",
        "hellip" => "…",
        "lsquo" => "‘",
        "rsquo" => "’",
        "ldquo" => "“",
        "rdquo" => "”",
        "bull" => "•",
        "rarr" => "→",
        "larr" => "←",
        "harr" => "↔",
        "uarr" => "↑",
        "darr" => "↓",
        "rArr" => "⇒",
        "le" => "≤",
        "ge" => "≥",
        "ne" => "≠",
        "asymp" => "≈",
        "infin" => "∞",
        "sum" => "∑",
        "int" => "∫",
        "part" => "∂",
        "radic" => "√",
        "sup2" => "²",
        "sup3" => "³",
        "frac12" => "½",
        "auml" => "ä",
        "ouml" => "ö",
        "uuml" => "ü",
        "Auml" => "Ä",
        "Ouml" => "Ö",
        "Uuml" => "Ü",
        "szlig" => "ß",
        "eacute" => "é",
        "egrave" => "è",
        "aacute" => "á",
        "agrave" => "à",
        "ccedil" => "ç",
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "Delta" => "Δ",
        "epsilon" => "ε",
        "eta" => "η",
        "theta" => "θ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "pi" => "π",
        "rho" => "ρ",
        "sigma" => "σ",
        "tau" => "τ",
        "phi" => "φ",
        "omega" => "ω",
        "Omega" => "Ω",
        _ => return None,
    };
    Some(c.to_owned())
}

// ---------------------------------------------------------------------------------------
// Parser

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

const BLOCK_TAGS: &[&str] = &[
    "p",
    "div",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "table",
    "tr",
    "td",
    "th",
    "pre",
    "hr",
    "blockquote",
    "dl",
    "dt",
    "dd",
    "center",
    "body",
    "html",
    "head",
    "thead",
    "tbody",
    "tfoot",
    "caption",
    "title",
];

fn is_block(name: &str) -> bool {
    BLOCK_TAGS.contains(&name)
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// Parse blocks until a closing tag in `stop` (not consumed) or the end.
    fn blocks_until(&mut self, stop: &[&str]) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut inline = InlineBuilder::default();
        while let Some(token) = self.peek().cloned() {
            match token {
                Token::Close(name) if stop.contains(&name.as_str()) => break,
                // A new list item / cell / row closes the current one implicitly.
                Token::Open { ref name, .. }
                    if stop.contains(&"li") && name == "li"
                        || stop.contains(&"td")
                            && (name == "td" || name == "th" || name == "tr")
                        || stop.contains(&"tr") && name == "tr"
                        || stop.contains(&"dd") && (name == "dt" || name == "dd") =>
                {
                    break
                }
                Token::Text(text) => {
                    self.pos += 1;
                    inline.text(&text);
                }
                Token::Open {
                    name,
                    attrs,
                    self_closing,
                } => {
                    self.pos += 1;
                    match name.as_str() {
                        "br" => inline.line_break(),
                        "img" => inline.image(image_ref(&attrs)),
                        "hr" => {
                            inline.flush(&mut blocks);
                            blocks.push(Block::Rule);
                        }
                        "head" | "title" if !self_closing => {
                            self.skip_until(&name);
                        }
                        "p" | "div" | "center" | "body" | "html" | "dt" | "dd" | "caption" => {
                            inline.flush(&mut blocks);
                            if !self_closing {
                                let inner = self.blocks_until(&[name.as_str()]);
                                self.consume_close(&name);
                                blocks.extend(inner);
                            }
                        }
                        "dl" | "thead" | "tbody" | "tfoot" => {
                            inline.flush(&mut blocks);
                            let inner = self.blocks_until(&[name.as_str()]);
                            self.consume_close(&name);
                            blocks.extend(inner);
                        }
                        "blockquote" => {
                            inline.flush(&mut blocks);
                            let inner = self.blocks_until(&["blockquote"]);
                            self.consume_close("blockquote");
                            blocks.push(Block::Quote(inner));
                        }
                        h if h.len() == 2
                            && h.starts_with('h')
                            && h.as_bytes()[1].is_ascii_digit() =>
                        {
                            inline.flush(&mut blocks);
                            let level = h.as_bytes()[1] - b'0';
                            let inner = self.blocks_until(&[h]);
                            self.consume_close(h);
                            let mut inlines = Vec::new();
                            for block in inner {
                                if let Block::Paragraph(mut i) = block {
                                    inlines.append(&mut i);
                                }
                            }
                            blocks.push(Block::Heading(level.clamp(1, 6), inlines));
                        }
                        "ul" | "ol" | "menu" | "dir" => {
                            inline.flush(&mut blocks);
                            let items = self.list_items(&name);
                            blocks.push(Block::List {
                                ordered: name == "ol",
                                items,
                            });
                        }
                        "li" => {
                            // A stray <li> outside a list: treat as a one-item list.
                            inline.flush(&mut blocks);
                            let item = self.blocks_until(&["li", "ul", "ol"]);
                            self.consume_close("li");
                            blocks.push(Block::List {
                                ordered: false,
                                items: vec![item],
                            });
                        }
                        "table" => {
                            inline.flush(&mut blocks);
                            let rows = self.table_rows();
                            if !rows.is_empty() {
                                blocks.push(Block::Table(rows));
                            }
                        }
                        "pre" => {
                            inline.flush(&mut blocks);
                            let text = self.raw_text_until("pre");
                            let text = text.strip_prefix('\n').unwrap_or(&text);
                            blocks.push(Block::Preformatted(text.trim_end().to_owned()));
                        }
                        "tr" | "td" | "th" => {
                            // Table parts outside a table: keep their text.
                        }
                        other => inline.open_inline(other, &attrs, self_closing),
                    }
                }
                Token::Close(name) => {
                    self.pos += 1;
                    if !is_block(&name) {
                        inline.close_inline(&name);
                    } else if name == "p" || name == "div" {
                        inline.flush(&mut blocks);
                    }
                }
            }
        }
        inline.flush(&mut blocks);
        blocks
    }

    fn consume_close(&mut self, name: &str) {
        if matches!(self.peek(), Some(Token::Close(n)) if n == name) {
            self.pos += 1;
        }
    }

    fn skip_until(&mut self, name: &str) {
        while let Some(token) = self.peek() {
            let done = matches!(token, Token::Close(n) if n == name);
            self.pos += 1;
            if done {
                break;
            }
        }
    }

    fn list_items(&mut self, list: &str) -> Vec<Vec<Block>> {
        let mut items = Vec::new();
        while let Some(token) = self.peek().cloned() {
            match token {
                Token::Close(name) if name == list => {
                    self.pos += 1;
                    break;
                }
                Token::Open { name, .. } if name == "li" => {
                    self.pos += 1;
                    let item = self.blocks_until(&["li", list]);
                    self.consume_close("li");
                    items.push(item);
                }
                Token::Text(t) if t.trim().is_empty() => self.pos += 1,
                _ => {
                    // Content directly inside <ul>: make it an item.
                    let item = self.blocks_until(&["li", list]);
                    if item.is_empty() {
                        self.pos += 1;
                    } else {
                        items.push(item);
                    }
                }
            }
        }
        items
    }

    fn table_rows(&mut self) -> Vec<Vec<TableCell>> {
        let mut rows = Vec::new();
        while let Some(token) = self.peek().cloned() {
            match token {
                Token::Close(name) if name == "table" => {
                    self.pos += 1;
                    break;
                }
                Token::Open { name, .. } if name == "tr" => {
                    self.pos += 1;
                    let mut cells = Vec::new();
                    while let Some(t) = self.peek().cloned() {
                        match t {
                            Token::Open { name, .. } if name == "td" || name == "th" => {
                                self.pos += 1;
                                let blocks = self.blocks_until(&["td", "th", "tr", "table"]);
                                self.consume_close(&name);
                                cells.push(TableCell {
                                    header: name == "th",
                                    blocks,
                                });
                            }
                            Token::Close(n) if n == "tr" => {
                                self.pos += 1;
                                break;
                            }
                            Token::Open { name, .. } if name == "tr" => break,
                            Token::Close(n) if n == "table" => break,
                            _ => self.pos += 1,
                        }
                    }
                    rows.push(cells);
                }
                Token::Open { name, .. } if name == "td" || name == "th" => {
                    // Cells without <tr>.
                    self.tokens.insert(
                        self.pos,
                        Token::Open {
                            name: "tr".into(),
                            attrs: vec![],
                            self_closing: false,
                        },
                    );
                }
                _ => self.pos += 1,
            }
        }
        rows
    }

    fn raw_text_until(&mut self, name: &str) -> String {
        let mut out = String::new();
        while let Some(token) = self.peek().cloned() {
            self.pos += 1;
            match token {
                Token::Close(n) if n == name => break,
                Token::Text(t) => out.push_str(&t),
                Token::Open { name: n, .. } if n == "br" => out.push('\n'),
                _ => {}
            }
        }
        out
    }
}

fn image_ref(attrs: &[(String, String)]) -> ImageRef {
    let get = |k: &str| attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let px = |k: &str| get(k).and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok());
    ImageRef {
        src: get("src").unwrap_or_default(),
        alt: get("alt").unwrap_or_default(),
        width: px("width"),
        height: px("height"),
    }
}

#[derive(Default)]
struct InlineBuilder {
    inlines: Vec<Inline>,
    stack: Vec<(String, Style, Option<String>)>,
}

impl InlineBuilder {
    fn current(&self) -> (Style, Option<String>) {
        self.stack
            .last()
            .map(|(_, s, l)| (*s, l.clone()))
            .unwrap_or_default()
    }

    fn text(&mut self, raw: &str) {
        // Collapse whitespace like a browser (outside <pre>).
        let mut collapsed = String::with_capacity(raw.len());
        let mut last_space = self.ends_with_space();
        for c in raw.chars() {
            if c.is_whitespace() && c != '\u{a0}' {
                if !last_space {
                    collapsed.push(' ');
                }
                last_space = true;
            } else {
                collapsed.push(c);
                last_space = false;
            }
        }
        if collapsed.is_empty() {
            return;
        }
        let (style, link) = self.current();
        if let Some(Inline::Text {
            text,
            style: s,
            link: l,
        }) = self.inlines.last_mut()
        {
            if *s == style && *l == link {
                text.push_str(&collapsed);
                return;
            }
        }
        self.inlines.push(Inline::Text {
            text: collapsed,
            style,
            link,
        });
    }

    fn ends_with_space(&self) -> bool {
        match self.inlines.last() {
            None | Some(Inline::LineBreak) => true,
            Some(Inline::Text { text, .. }) => text.ends_with(' '),
            Some(Inline::Image(_)) => false,
        }
    }

    fn line_break(&mut self) {
        if let Some(Inline::Text { text, .. }) = self.inlines.last_mut() {
            let trimmed = text.trim_end().len();
            text.truncate(trimmed);
        }
        self.inlines.push(Inline::LineBreak);
    }

    fn image(&mut self, img: ImageRef) {
        self.inlines.push(Inline::Image(img));
    }

    fn open_inline(&mut self, name: &str, attrs: &[(String, String)], self_closing: bool) {
        if self_closing {
            return;
        }
        let (mut style, mut link) = self.current();
        match name {
            "b" | "strong" => style.bold = true,
            "i" | "em" | "cite" | "var" | "dfn" => style.italic = true,
            "u" | "ins" => style.underline = true,
            "code" | "tt" | "kbd" | "samp" => style.code = true,
            "sup" => style.sup = true,
            "sub" => style.sub = true,
            "a" => {
                if let Some((_, href)) = attrs.iter().find(|(n, _)| n == "href") {
                    link = Some(href.clone());
                }
            }
            _ => {}
        }
        self.stack.push((name.to_owned(), style, link));
    }

    fn close_inline(&mut self, name: &str) {
        if let Some(pos) = self.stack.iter().rposition(|(n, _, _)| n == name) {
            self.stack.truncate(pos);
        }
    }

    /// Emit the accumulated inline run as a paragraph (trimmed) if it has content.
    fn flush(&mut self, blocks: &mut Vec<Block>) {
        let mut inlines = std::mem::take(&mut self.inlines);
        while matches!(inlines.last(), Some(Inline::LineBreak)) {
            inlines.pop();
        }
        if let Some(Inline::Text { text, .. }) = inlines.first_mut() {
            *text = text.trim_start().to_owned();
        }
        if let Some(Inline::Text { text, .. }) = inlines.last_mut() {
            *text = text.trim_end().to_owned();
        }
        inlines.retain(|i| !matches!(i, Inline::Text { text, .. } if text.is_empty()));
        if !inlines.is_empty() {
            blocks.push(Block::Paragraph(inlines));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> Inline {
        Inline::Text {
            text: t.into(),
            style: Style::default(),
            link: None,
        }
    }

    #[test]
    fn paragraphs_entities_and_bold() {
        let blocks =
            parse("<html><p>a &lt; b &reg;</p>\n<p>The <strong>Modelica</strong> lib</p></html>");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0], Block::Paragraph(vec![text("a < b ®")]));
        let Block::Paragraph(inl) = &blocks[1] else {
            panic!()
        };
        assert_eq!(inl.len(), 3);
        assert!(
            matches!(&inl[1], Inline::Text { text, style, .. } if text == "Modelica" && style.bold)
        );
    }

    #[test]
    fn links_lists_and_images() {
        let blocks = parse(
            "<div><img src=\"modelica://Modelica/a.svg\" width=\"250\"></div><ul><li> <a href=\"modelica://M.U\">Overview</a> text</li><li>two</ul>",
        );
        assert_eq!(
            blocks[0],
            Block::Paragraph(vec![Inline::Image(ImageRef {
                src: "modelica://Modelica/a.svg".into(),
                alt: String::new(),
                width: Some(250.0),
                height: None
            })])
        );
        let Block::List { ordered, items } = &blocks[1] else {
            panic!("{blocks:?}")
        };
        assert!(!ordered);
        assert_eq!(items.len(), 2);
        let Block::Paragraph(first) = &items[0][0] else {
            panic!()
        };
        assert!(
            matches!(&first[0], Inline::Text { text, link: Some(l), .. } if text == "Overview" && l == "modelica://M.U")
        );
        assert_eq!(first[1], text(" text"));
        assert_eq!(items[1], vec![Block::Paragraph(vec![text("two")])]);
    }

    #[test]
    fn line_breaks_pre_headings_tables() {
        let blocks = parse(
            "<h4>Title</h4><p>x<br>\ny</p><pre>\n  a\n  b</pre><table><tr><th>N</th><td>D</td></tr><tr><td>1<td>2</tr></table>",
        );
        assert_eq!(blocks[0], Block::Heading(4, vec![text("Title")]));
        assert_eq!(
            blocks[1],
            Block::Paragraph(vec![text("x"), Inline::LineBreak, text("y")])
        );
        assert_eq!(blocks[2], Block::Preformatted("  a\n  b".into()));
        let Block::Table(rows) = &blocks[3] else {
            panic!()
        };
        assert_eq!(rows.len(), 2);
        assert!(rows[0][0].header);
        assert_eq!(rows[1].len(), 2);
    }

    #[test]
    fn scripts_comments_and_broken_markup_are_safe() {
        let blocks = parse("<script>alert(1)</script><!-- c -->ok <b>bold <p>after");
        let txt = plain_text(&blocks);
        assert!(txt.contains("ok"));
        assert!(txt.contains("after"));
        assert!(!txt.contains("alert"));
        let _ = parse("<");
        let _ = parse("<a href=\"x");
        let _ = parse("a < b and c > d");
        assert_eq!(plain_text(&parse("a < b")).trim(), "a < b");
    }

    #[test]
    fn decodes_numeric_entities() {
        assert_eq!(decode_entities("&#169; &#x3A9; &bogus; &"), "© Ω &bogus; &");
    }
}
