use crate::TextSize;

/// position in a source file
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

/// index to convert between byte offsets and line/column positions
#[derive(Clone, Debug)]
pub struct LineIndex {
    line_starts: Vec<TextSize>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![TextSize::new(0)];
        for (i, c) in text.char_indices() {
            if c == '\n' {
                line_starts.push(TextSize::new((i + 1) as u32));
            }
        }
        Self { line_starts }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

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

    pub fn offset(&self, position: Position) -> Option<TextSize> {
        let line_start = self.line_starts.get(position.line as usize)?;
        Some(*line_start + TextSize::new(position.column))
    }
}
