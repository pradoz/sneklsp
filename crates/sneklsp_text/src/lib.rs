mod line_index;
mod range;
mod size;

pub use line_index::{LineIndex, Position};
pub use range::TextRange;
pub use size::TextSize;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_size() {
        let size = TextSize::of("hello");
        assert_eq!(size.to_u32(), 5);
    }

    #[test]
    fn test_line_index() {
        let index = LineIndex::new("hello\nworld");
        assert_eq!(index.line_count(), 2);
        assert_eq!(
            index.position(TextSize::new(6)),
            Position { line: 1, column: 0 }
        );
    }
}
