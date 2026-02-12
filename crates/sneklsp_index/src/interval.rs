use sneklsp_text::{TextRange, TextSize};

/// invariant: intervals are sorted by start position.
#[derive(Debug, Clone)]
pub struct IntervalTree<T> {
    entries: Vec<Entry<T>>,
}

#[derive(Debug, Clone)]
struct Entry<T> {
    start: u32,
    end: u32,
    max_end: u32,
    value: T,
}

impl<T: Copy> IntervalTree<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn insert(&mut self, range: TextRange, value: T) {
        self.entries.push(Entry {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
            max_end: range.end().to_u32(),
            value,
        });
    }

    #[inline]
    pub fn finish(&mut self) {
        self.entries.sort_unstable_by_key(|e| e.start);

        // build suffix-max for end values
        let len = self.entries.len();
        if len == 0 {
            return;
        }
        let mut running_max = 0u32;
        for i in (0..len).rev() {
            if self.entries[i].end > running_max {
                running_max = self.entries[i].end;
            }
            self.entries[i].max_end = running_max;
        }
    }

    pub fn find_containing(&self, pos: TextSize) -> Vec<T> {
        let p = pos.to_u32();
        let mut results = Vec::new();

        for entry in &self.entries {
            if entry.start > p {
                break;
            }
            // pruning
            if p < entry.end {
                results.push(entry.value);
            }
        }

        results
    }

    pub fn find_innermost(&self, pos: TextSize) -> Option<T> {
        let p = pos.to_u32();
        let mut best: Option<(u32, T)> = None;

        for entry in &self.entries {
            if entry.start > p {
                break;
            }
            // pruning
            if entry.max_end <= p {
                continue;
            }
            if p < entry.end {
                let width = entry.end - entry.start;
                match best {
                    Some((bw, _)) if width < bw => best = Some((width, entry.value)),
                    None => best = Some((width, entry.value)),
                    _ => {}
                }
            }
        }

        best.map(|(_, v)| v)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
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

    #[test]
    fn empty_tree() {
        let tree: IntervalTree<u32> = IntervalTree::new();
        assert_eq!(tree.find_innermost(TextSize::new(0)), None);
        assert!(tree.find_containing(TextSize::new(0)).is_empty());
    }

    #[test]
    fn no_match() {
        let mut tree = IntervalTree::new();
        tree.insert(TextRange::new(TextSize::new(10), TextSize::new(20)), 0);
        tree.finish();

        assert_eq!(tree.find_innermost(TextSize::new(5)), None);
        assert_eq!(tree.find_innermost(TextSize::new(25)), None);
        assert!(tree.find_containing(TextSize::new(5)).is_empty());
    }

    #[test]
    fn many_intervals() {
        let mut tree = IntervalTree::new();
        for i in 0..100u32 {
            tree.insert(
                TextRange::new(TextSize::new(i * 10), TextSize::new((i + 1) * 10 + 50)),
                i,
            );
        }
        tree.finish();

        let result = tree.find_innermost(TextSize::new(55));
        assert!(result.is_some());
    }

    #[test]
    fn adjacent_intervals() {
        let mut tree = IntervalTree::new();
        tree.insert(TextRange::new(TextSize::new(0), TextSize::new(10)), 0);
        tree.insert(TextRange::new(TextSize::new(10), TextSize::new(20)), 1);
        tree.finish();

        assert_eq!(tree.find_innermost(TextSize::new(10)), Some(1));
        assert_eq!(tree.find_innermost(TextSize::new(9)), Some(0));
    }
}
