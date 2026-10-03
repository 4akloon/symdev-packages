//! `HtmlText`: the text of an HTML page, as the Python tool's HTMLParser subclass gave it.

use crate::py_text::PyText;
use crate::tool_error::{Result, ToolError};

/// The body's text: `<pre>` verbatim, other text with its whitespace collapsed (each piece
/// between two tags on its own, as HTMLParser handed it over), a line break around each
/// block; the head is left out. Only what the toolchain's notice pages were seen to hold
/// is read: an entity other than `&amp; &lt; &gt; &quot; &apos;` and numeric ones, or a
/// script or style element, is an error.
pub struct HtmlText {
    parts: Vec<String>,
    line: String,
    pre: i32,
    head: i32,
}

impl HtmlText {
    const BLOCKS: [&str; 14] = [
        "p", "div", "h1", "h2", "h3", "h4", "li", "ul", "details", "summary", "br", "pre", "table",
        "tr",
    ];

    pub fn of(page: &str) -> Result<String> {
        let mut text = HtmlText {
            parts: Vec::new(),
            line: String::new(),
            pre: 0,
            head: 0,
        };
        text.feed(page)?;
        text.flush();
        Ok(text.parts.concat())
    }

    fn feed(&mut self, page: &str) -> Result<()> {
        let mut at = 0;
        while at < page.len() {
            let next = page[at..].find('<').map_or(page.len(), |i| at + i);
            if next > at {
                self.data(&Self::unescape(&page[at..next])?);
            }
            if next == page.len() {
                break;
            }
            at = next + self.markup(&page[next..])?;
        }
        Ok(())
    }

    /// The markup at the start of `rest` (which starts with `<`); returns its length.
    fn markup(&mut self, rest: &str) -> Result<usize> {
        let unterminated = || ToolError::new(format!("unterminated markup: {:.40}", rest));
        let after = &rest[1..];
        if after.starts_with(|c: char| c.is_ascii_alphabetic()) {
            let end = Self::tag_end(rest).ok_or_else(unterminated)?;
            let name = Self::tag_name(after);
            if name == "script" || name == "style" {
                return Err(ToolError::new(format!(
                    "a <{name}> element was never seen here"
                )));
            }
            self.start(&name);
            if rest[..end].ends_with('/') {
                self.end(&name);
            }
            return Ok(end + 1);
        }
        if let Some(close) = after.strip_prefix('/') {
            if !close.starts_with(|c: char| c.is_ascii_alphabetic()) {
                return Err(ToolError::new(format!("malformed end tag: {:.40}", rest)));
            }
            let end = rest.find('>').ok_or_else(unterminated)?;
            self.end(&Self::tag_name(close));
            return Ok(end + 1);
        }
        if rest.starts_with("<!--") {
            return Ok(rest.find("-->").ok_or_else(unterminated)? + 3);
        }
        if after.starts_with('!') || after.starts_with('?') {
            return Ok(rest.find('>').ok_or_else(unterminated)? + 1);
        }
        self.data("<");
        Ok(1)
    }

    /// The `>` that ends a start tag, past quoted attribute values.
    fn tag_end(rest: &str) -> Option<usize> {
        let mut quote = None;
        for (i, c) in rest.char_indices() {
            match (quote, c) {
                (None, '"' | '\'') => quote = Some(c),
                (Some(q), _) if c == q => quote = None,
                (None, '>') => return Some(i),
                _ => {}
            }
        }
        None
    }

    fn tag_name(after: &str) -> String {
        let end = after
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .unwrap_or(after.len());
        after[..end].to_ascii_lowercase()
    }

    fn flush(&mut self) {
        let line = PyText::strip(&self.line);
        if !line.is_empty() {
            self.parts.push(format!("{line}\n"));
        }
        self.line.clear();
    }

    fn start(&mut self, tag: &str) {
        if Self::BLOCKS.contains(&tag) {
            self.flush();
        }
        self.pre += i32::from(tag == "pre");
        self.head += i32::from(tag == "head");
    }

    fn end(&mut self, tag: &str) {
        self.pre -= i32::from(tag == "pre");
        self.head -= i32::from(tag == "head");
        if Self::BLOCKS.contains(&tag) {
            self.flush();
        }
    }

    fn data(&mut self, data: &str) {
        if self.head != 0 {
            return;
        }
        if self.pre != 0 {
            self.parts.push(data.to_string());
        } else {
            self.line.push_str(&PyText::collapse_spaces(data));
        }
    }

    /// `html.unescape`, for the references the pages were seen to use.
    fn unescape(text: &str) -> Result<String> {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(amp) = rest.find('&') {
            out.push_str(&rest[..amp]);
            let after = &rest[amp + 1..];
            let Some((char, used)) = Self::reference(after)? else {
                out.push('&');
                rest = after;
                continue;
            };
            out.push(char);
            rest = &after[used..];
        }
        out.push_str(rest);
        Ok(out)
    }

    /// The character `after` (the text after a `&`) names and how many bytes it takes;
    /// `None` when the `&` starts no reference and stays as it is.
    fn reference(after: &str) -> Result<Option<(char, usize)>> {
        if let Some(number) = after.strip_prefix('#') {
            let (digits, radix, skip) = match number.strip_prefix(['x', 'X']) {
                Some(hex) => (hex, 16, 2),
                None => (number, 10, 1),
            };
            let len = digits
                .find(|c: char| !c.is_digit(radix))
                .unwrap_or(digits.len());
            if len == 0 {
                return Ok(None);
            }
            let code = u32::from_str_radix(&digits[..len], radix).ok();
            let char = code.and_then(char::from_u32).filter(|&c| Self::plain(c));
            let char = char.ok_or_else(|| {
                ToolError::new(format!(
                    "the reference &#{} was never seen here",
                    &number[..len + skip - 1]
                ))
            })?;
            let semicolon = usize::from(digits[len..].starts_with(';'));
            return Ok(Some((char, skip + len + semicolon)));
        }
        let len = after
            .find(['\t', '\n', '\u{c}', ' ', '<', '&', '#', ';'])
            .unwrap_or(after.len());
        if len == 0 {
            return Ok(None);
        }
        let name = &after[..len];
        let char = match name {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                return Err(ToolError::new(format!(
                    "the entity &{name}; was never seen here"
                )));
            }
        };
        if !after[len..].starts_with(';') {
            return Err(ToolError::new(format!("the entity &{name} has no `;`")));
        }
        Ok(Some((char, len + 1)))
    }

    /// A character `html.unescape` gives back as it is for its number: not a control
    /// character but tab, line feed, form feed (carriage return it maps itself), not a
    /// noncharacter.
    fn plain(c: char) -> bool {
        let noncharacter = ('\u{fdd0}'..='\u{fdef}').contains(&c) || (c as u32 & 0xfffe) == 0xfffe;
        (!c.is_control() || matches!(c, '\t' | '\n' | '\u{c}')) && !noncharacter
    }
}
