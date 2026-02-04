use crate::background::IndexedModule;
use lsp_types::TextDocumentContentChangeEvent;
use sneklsp_text::LineIndex;

#[derive(Debug)]
pub struct Document {
    pub content: String,
    pub line_index: LineIndex,
    pub version: i32,
    pub index: Option<IndexedModule>,
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
        }
    }

    pub fn apply_changes(&mut self, changes: Vec<TextDocumentContentChangeEvent>, version: i32) {
        for change in changes {
            self.apply_change(change);
        }
        self.version = version;
        self.line_index = LineIndex::new(&self.content);
        self.index = None;
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
    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.line_index = LineIndex::new(&self.content);
    }

    #[inline]
    pub fn set_index(&mut self, index: IndexedModule) {
        self.index = Some(index);
    }

    #[inline]
    pub fn take_content(&mut self) -> String {
        std::mem::take(&mut self.content)
    }

    #[inline]
    pub fn has_index(&self) -> bool {
        self.index.is_some()
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
