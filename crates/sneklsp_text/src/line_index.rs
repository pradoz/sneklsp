use crate::TextSize;

/// position in a source file
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

/// index to convert between byte offsets and line/column positions
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineIndex {
    line_starts: Vec<TextSize>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let estimated_lines = (text.len() / 40).max(1);
        let mut line_starts = Vec::with_capacity(estimated_lines);
        line_starts.push(TextSize::new(0));

        // newline is always 1 byte
        for (i, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(TextSize::new((i + 1) as u32));
            }
        }
        Self { line_starts }
    }

    #[inline]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    #[inline]
    pub fn position(&self, offset: TextSize) -> Position {
        let line = self
            .line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line];
        Position {
            line: line as u32,
            column: (offset - line_start).to_u32(),
        }
    }

    #[inline]
    pub fn offset(&self, position: Position) -> Option<TextSize> {
        let line_start = self.line_starts.get(position.line as usize)?;
        Some(*line_start + TextSize::new(position.column))
    }
}
