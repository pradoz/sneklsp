use std::sync::Arc;

use lsp_types::TextDocumentContentChangeEvent;

use sneklsp_index::OwnedIndex;
use sneklsp_lexer::Token;
use sneklsp_text::{LineIndex, TextRange};

#[derive(Debug)]
pub struct Document {
    content: Arc<str>,
    pub line_index: LineIndex,
    pub version: i32,
    pub index: Option<OwnedIndex>,
    pub tokens: Vec<Token>,
    pub last_edit_range: Option<TextRange>,
}

impl Document {
    #[inline]
    pub fn new(content: String, version: i32) -> Self {
        let line_index = LineIndex::new(&content);
        Self {
            content: Arc::from(content),
            line_index,
            version,
            index: None,
            tokens: Vec::new(),
            last_edit_range: None,
        }
    }

    #[inline]
    pub fn content(&self) -> Arc<str> {
        Arc::clone(&self.content)
    }

    pub fn apply_changes(&mut self, changes: Vec<TextDocumentContentChangeEvent>, version: i32) {
        for change in changes {
            self.apply_change(change);
        }
        self.version = version;
        self.line_index = LineIndex::new(&self.content);

        self.index = None; // invalidate index
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
                    let start_usize = start.to_usize();
                    let end_usize = end.to_usize();

                    if start_usize <= end_usize && end_usize <= self.content.len() {
                        self.last_edit_range = Some(TextRange::new(start, end));
                        let mut buf = String::from(&*self.content);
                        buf.replace_range(start_usize..end_usize, &change.text);
                        self.content = Arc::from(buf);
                    } else {
                        tracing::warn!("invalid change range. using full replacement");
                        self.full_replace(change.text);
                    }
                } else {
                    tracing::warn!("couldn't compute offsets. using full replacement");
                    self.full_replace(change.text);
                }
            }
            // full content change
            None => {
                self.full_replace(change.text);
            }
        }
    }

    fn full_replace(&mut self, content: String) {
        self.content = Arc::from(content);
        self.tokens.clear();
        self.index = None;
        self.last_edit_range = None;
    }

    pub fn set_tokens(&mut self, tokens: Vec<Token>) {
        self.tokens = tokens;
    }

    #[inline]
    pub fn set_index_from_analysis(&mut self, index: &OwnedIndex, line_index: &LineIndex) {
        self.line_index = line_index.clone();
        self.index = Some(index.clone());
    }
}
