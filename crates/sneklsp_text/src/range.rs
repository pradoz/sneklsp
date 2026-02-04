use crate::TextSize;
use std::ops::Range;

/// range in source text
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextRange {
    start: TextSize,
    end: TextSize,
}

impl TextRange {
    #[inline]
    pub const fn new(start: TextSize, end: TextSize) -> Self {
        Self { start, end }
    }

    #[inline]
    pub const fn empty(offset: TextSize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    #[inline]
    pub const fn start(self) -> TextSize {
        self.start
    }

    #[inline]
    pub const fn end(self) -> TextSize {
        self.end
    }

    #[inline]
    pub const fn len(self) -> TextSize {
        TextSize::new(self.end.to_u32() - self.start.to_u32())
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.start.to_u32() == self.end.to_u32()
    }

    #[inline]
    pub const fn contains(self, offset: TextSize) -> bool {
        self.start.to_u32() <= offset.to_u32() && offset.to_u32() < self.end.to_u32()
    }

    #[inline]
    pub const fn contains_inclusive(self, offset: TextSize) -> bool {
        self.start.to_u32() <= offset.to_u32() && offset.to_u32() <= self.end.to_u32()
    }
}

impl From<TextRange> for Range<usize> {
    fn from(range: TextRange) -> Self {
        range.start.to_usize()..range.end.to_usize()
    }
}
