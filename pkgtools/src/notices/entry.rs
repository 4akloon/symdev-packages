//! `Entry`: one section of the notices file.

/// The line between and around entries.
const RULE: &str =
    "-------------------------------------------------------------------------------";

/// A title, an SPDX expression and the texts that go with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub title: String,
    pub license: String,
    /// (name shown, full text) of each licence file.
    pub files: Vec<(String, String)>,
    /// Where it comes from (a crate's `source`), or empty.
    pub source: String,
    pub note: String,
}

impl Entry {
    pub fn render(&self) -> String {
        let mut head = vec![
            RULE.to_string(),
            self.title.clone(),
            format!("License: {}", self.license),
        ];
        if !self.source.is_empty() {
            head.push(format!("Source: {}", self.source));
        }
        head.extend([RULE.to_string(), String::new()]);
        let mut body = Vec::new();
        if !self.note.is_empty() {
            body.push(self.note.clone());
        }
        if self.files.is_empty() {
            body.push("No licence file was shipped with it; its SPDX expression is above.".into());
        }
        for (name, text) in &self.files {
            body.push(format!(
                "== {name} ==\n\n{}",
                crate::py_text::PyText::rstrip(text)
            ));
        }
        format!("{}{}\n", head.join("\n"), body.join("\n\n"))
    }
}
