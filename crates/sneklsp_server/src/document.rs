use lsp_types::TextDocumentContentChangeEvent;
use sneklsp_parser::ParseError;
use sneklsp_text::LineIndex;

#[derive(Debug)]
pub struct Document {
    pub content: String,
    pub line_index: LineIndex,
    pub errors: Vec<ParseError>,
    pub version: i32,
}

impl Document {
    pub fn new(content: String, version: i32) -> Self {
        let line_index = LineIndex::new(&content);
        let errors = Self::parse_content(&content);
        Self {
            content,
            line_index,
            errors,
            version,
        }
    }

    pub fn apply_changes(&mut self, changes: Vec<TextDocumentContentChangeEvent>, version: i32) {
        for change in changes {
            self.apply_change(change);
        }
        self.version = version;
        self.line_index = LineIndex::new(&self.content);
        self.errors = Self::parse_content(&self.content);
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

    pub fn replace_content(&mut self, content: String, version: i32) {
        self.content = content;
        self.version = version;
        self.line_index = LineIndex::new(&self.content);
        self.errors = Self::parse_content(&self.content);
    }

    fn parse_content(content: &str) -> Vec<ParseError> {
        sneklsp_parser::parse_and_collect_errors(content)
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}
