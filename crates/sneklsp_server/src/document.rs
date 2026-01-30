use lsp_types::TextDocumentContentChangeEvent;
use sneklsp_text::LineIndex;

#[derive(Debug)]
pub struct Document {
    pub content: String,
    pub line_index: LineIndex,
    pub version: i32,
}

impl Document {
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
            None => todo!(),
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn content_clone(&self) -> String {
        // take ownership of the content string for background parsing
        self.content.clone()
    }
}
