use sneklsp_text::{TextRange, TextSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextEdit {
    pub range: TextRange,
    pub new_len: TextSize,
}

impl TextEdit {
    #[inline]
    pub fn new(range: TextRange, new_len: TextSize) -> Self {
        Self { range, new_len }
    }

    #[inline]
    pub fn offset_delta(&self) -> i64 {
        self.new_len.to_u32() as i64 - self.range.len().to_u32() as i64
    }
}
