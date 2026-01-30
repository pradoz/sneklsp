use lsp_types::TextDocumentContentChangeEvent;
use sneklsp_text::LineIndex;

#[derive(Debug)]
pub struct Document {
    pub content: String,
    pub line_index: LineIndex,
    pub version: i32,
}

impl Document {
    #[inline]
    pub fn new(content: String, version: i32) -> Self {
        let line_index = LineIndex::new(&content);
        Self {
            content,
            line_index,
            version,
        }
    }

    pub fn apply_changes(&mut self, changes: Vec<TextDocumentContentChangeEvent>, version: i32) {
        for change in changes {
            self.apply_change(change);
        }
        self.version = version;
        self.line_index = LineIndex::new(&self.content);
    }

    fn apply_change(&mut self, change: TextDocumentContentChangeEvent) {
        match change.range {
            // incremental content change
            Some(range) => {
                let start_offset = self.line_index.offset(sneklsp_text::Position {
                    line: range.start.line,
                    column: range.start.character,
                });
                let end_offset = self.line_index.offset(sneklsp_text::Position {
                    line: range.end.line,
                    column: range.end.character,
                });

                if let (Some(start), Some(end)) = (start_offset, end_offset) {
                    let start = start.to_usize();
                    let end = end.to_usize();

                    // bounds check
                    if start <= end && end <= self.content.len() {
                        self.content.replace_range(start..end, &change.text);
                    } else {
                        tracing::warn!("invalid change range. using full replacement");
                        self.content = change.text;
                    }
                } else {
                    tracing::warn!("couldn't compute offsets. using full replacement");
                    self.content = change.text;
                }
            }
            // full content change
            None => {
                self.content = change.text;
            }
        }
    }

    #[inline]
    pub fn content(&self) -> &str {
        &self.content
    }

    #[inline]
    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.line_index = LineIndex::new(&self.content);
    }

    #[inline]
    pub fn take_content(&mut self) -> String {
        std::mem::take(&mut self.content)
    }

    #[inline]
    pub fn take_for_parsing(&mut self) -> (String, i32) {
        (std::mem::take(&mut self.content), self.version)
    }
}

impl From<(String, i32)> for Document {
    #[inline]
    fn from((content, version): (String, i32)) -> Self {
        Self::new(content, version)
    }
}
