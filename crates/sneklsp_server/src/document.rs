use crate::background::IndexedModule;
use lsp_types::TextDocumentContentChangeEvent;
use sneklsp_lexer::Token;
use sneklsp_text::{LineIndex, TextRange, TextSize};

#[derive(Debug, Clone)]
pub struct EditRecord {
    pub range: TextRange,
    pub new_len: TextSize,
    pub old_content: String,
}

#[derive(Debug)]
pub struct Document {
    pub content: String,
    pub line_index: LineIndex,
    pub version: i32,
    pub index: Option<IndexedModule>,
    pub pending_edits: Vec<EditRecord>,
    pub tokens: Vec<Token>,
}

impl Document {
    #[inline]
    pub fn new(content: String, version: i32) -> Self {
        let line_index = LineIndex::new(&content);
        Self {
            content,
            line_index,
            version,
            index: None,
            pending_edits: Vec::new(),
            tokens: Vec::new(),
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
                    let start_usize = start.to_usize();
                    let end_usize = end.to_usize();

                    // bounds check
                    if start_usize <= end_usize && end_usize <= self.content.len() {
                        let old_content = self.content[start_usize..end_usize].to_string();

                        self.pending_edits.push(EditRecord {
                            range: TextRange::new(start, end),
                            new_len: TextSize::new(change.text.len() as u32),
                            old_content,
                        });

                        self.content
                            .replace_range(start_usize..end_usize, &change.text);
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
        self.content = content;
        self.pending_edits.clear();
        self.tokens.clear();
        self.index = None;
    }

    #[inline]
    pub fn take_edits(&mut self) -> Vec<EditRecord> {
        std::mem::take(&mut self.pending_edits)
    }

    #[inline]
    pub fn has_tokens(&self) -> bool {
        !self.tokens.is_empty()
    }

    pub fn set_tokens(&mut self, tokens: Vec<Token>) {
        self.tokens = tokens;
    }

    #[inline]
    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.line_index = LineIndex::new(&self.content);
    }

    #[inline]
    pub fn set_index(&mut self, index: IndexedModule) {
        self.index = Some(index);
    }
}

impl From<(String, i32)> for Document {
    #[inline]
    fn from((content, version): (String, i32)) -> Self {
        Self::new(content, version)
    }
}

impl std::fmt::Debug for IndexedModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexedModule")
            .field("symbols", &self.symbols.len())
            .field("scopes", &self.scopes.len())
            .field("references", &self.references.len())
            .finish()
    }
}
