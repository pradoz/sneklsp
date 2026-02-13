use std::sync::Arc;

use lsp_types::TextDocumentContentChangeEvent;

use sneklsp_index::OwnedIndex;
use sneklsp_lexer::{TextEdit as LexerTextEdit, Token, relex};
use sneklsp_text::{LineIndex, TextRange, TextSize};

#[derive(Debug)]
pub struct Document {
    content: Arc<str>,
    prev_content: Option<Arc<str>>,
    pub line_index: LineIndex,
    pub version: i32,
    pub index: Option<OwnedIndex>,
    pub tokens: Arc<[Token]>,
    pub last_edit_range: Option<TextRange>,
    pub tokens_dirty: bool,
}

impl Document {
    #[inline]
    pub fn new(content: String, version: i32) -> Self {
        let line_index = LineIndex::new(&content);
        Self {
            content: Arc::from(content),
            prev_content: None,
            line_index,
            version,
            index: None,
            tokens: Arc::from([]),
            last_edit_range: None,
            tokens_dirty: false,
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
                        let old_content = Arc::clone(&self.content);

                        let mut buf = String::from(&*self.content);
                        buf.replace_range(start_usize..end_usize, &change.text);
                        self.content = Arc::from(buf);

                        self.try_incremental_relex(
                            &old_content,
                            start,
                            end,
                            TextSize::new(change.text.len() as u32),
                        );
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

    fn try_incremental_relex(
        &mut self,
        old_source: &str,
        edit_start: TextSize,
        edit_end: TextSize,
        new_len: TextSize,
    ) {
        if self.tokens.is_empty() {
            return;
        }

        let edit = LexerTextEdit::new(TextRange::new(edit_start, edit_end), new_len);
        let result = relex(&self.tokens, old_source, &self.content, edit);
        self.tokens = Arc::from(result.tokens);
        self.tokens_dirty = true;
    }

    fn full_replace(&mut self, content: String) {
        self.prev_content = None;
        self.content = Arc::from(content);
        self.tokens = Arc::from([]);
        self.index = None;
        self.last_edit_range = None;
        self.tokens_dirty = false;
    }

    pub fn set_tokens(&mut self, tokens: &[Token]) {
        self.tokens = Arc::from(tokens);
        self.tokens_dirty = false;
    }

    #[inline]
    pub fn set_index_from_analysis(&mut self, index: &OwnedIndex, line_index: &LineIndex) {
        self.line_index = line_index.clone();
        self.index = Some(index.clone());
    }
}
