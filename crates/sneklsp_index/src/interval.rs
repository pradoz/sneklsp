use sneklsp_text::{TextRange, TextSize};

/// TODO: use red-black interval tree
///
/// invariant: intervals are sorted by start position.
#[derive(Debug, Clone)]
pub struct IntervalTree<T> {
    intervals: Vec<Interval<T>>,
}

#[derive(Debug, Clone)]
struct Interval<T> {
    range: TextRange,
    value: T,
}

impl<T: Copy> IntervalTree<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            intervals: Vec::new(),
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            intervals: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn insert(&mut self, range: TextRange, value: T) {
        self.intervals.push(Interval { range, value });
    }

    #[inline]
    pub fn finish(&mut self) {
        self.intervals
            .sort_unstable_by_key(|i| i.range.start().to_u32());
    }

    pub fn find_containing(&self, pos: TextSize) -> Vec<T> {
        let mut results = Vec::new();

        for interval in &self.intervals {
            // intervals are sorted by start position
            if interval.range.start() > pos {
                break;
            }

            if interval.range.contains(pos) {
                results.push(interval.value);
            }
        }

        results
    }

    pub fn find_innermost(&self, pos: TextSize) -> Option<T> {
        let mut best: Option<&Interval<T>> = None;

        for interval in &self.intervals {
            if interval.range.start() > pos {
                break;
            }

            if interval.range.contains(pos) {
                match best {
                    // finds the smallest interval containing position
                    Some(b) if interval.range.len() < b.range.len() => {
                        best = Some(interval);
                    }
                    None => best = Some(interval),
                    _ => {}
                }
            }
        }

        best.map(|i| i.value)
    }

    pub fn find_exact(&self, range: TextRange) -> Option<T> {
        // binary search for start position
        let start = self
            .intervals
            .partition_point(|i| i.range.start() < range.start());

        // linear scan from start
        for interval in &self.intervals[start..] {
            if interval.range.start() > range.start() {
                break;
            }
            if interval.range == range {
                return Some(interval.value);
            }
        }

        None
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.intervals.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (TextRange, T)> + '_ {
        self.intervals.iter().map(|i| (i.range, i.value))
    }
}

impl<T: Copy> Default for IntervalTree<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_containing() {
        let mut tree = IntervalTree::new();

        tree.insert(TextRange::new(TextSize::new(0), TextSize::new(100)), 0);
        tree.insert(TextRange::new(TextSize::new(10), TextSize::new(50)), 1);
        tree.insert(TextRange::new(TextSize::new(20), TextSize::new(30)), 2);
        tree.finish();

        let containing = tree.find_containing(TextSize::new(25));
        assert_eq!(containing, vec![0, 1, 2]);

        let containing = tree.find_containing(TextSize::new(5));
        assert_eq!(containing, vec![0]);

        let containing = tree.find_containing(TextSize::new(60));
        assert_eq!(containing, vec![0]);
    }

    #[test]
    fn find_innermost() {
        let mut tree = IntervalTree::new();

        tree.insert(TextRange::new(TextSize::new(0), TextSize::new(100)), 0);
        tree.insert(TextRange::new(TextSize::new(10), TextSize::new(50)), 1);
        tree.insert(TextRange::new(TextSize::new(20), TextSize::new(30)), 2);
        tree.finish();

        assert_eq!(tree.find_innermost(TextSize::new(25)), Some(2));
        assert_eq!(tree.find_innermost(TextSize::new(15)), Some(1));
        assert_eq!(tree.find_innermost(TextSize::new(5)), Some(0));
    }
}
