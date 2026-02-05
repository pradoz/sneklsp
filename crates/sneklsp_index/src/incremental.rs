use crate::{ModuleIndex, ScopeId};
use sneklsp_text::TextRange;

pub fn find_affected_scopes(index: &ModuleIndex<'_>, edit_range: TextRange) -> Vec<ScopeId> {
    let mut affected = Vec::new();

    for scope in index.scopes() {
        if ranges_overlap(scope.range, edit_range) {
            affected.push(scope.id);
        }
    }

    affected
}

#[inline]
fn ranges_overlap(a: TextRange, b: TextRange) -> bool {
    a.start() < b.end() && b.start() < a.end()
}

pub fn can_update_incrementally(index: &ModuleIndex<'_>, edit_range: TextRange) -> bool {
    let affected_scopes = find_affected_scopes(index, edit_range);

    if affected_scopes.len() == 1 {
        let scope = index.scope(affected_scopes[0]);
        return matches!(
            scope.kind,
            crate::ScopeKind::Function | crate::ScopeKind::Class
        );
    }

    if affected_scopes.is_empty() {
        return true;
    }

    false // multiple scoped affect. should re-index
}

#[cfg(test)]
mod tests {
    use super::*;
    use sneklsp_text::TextSize;

    fn range(start: u32, end: u32) -> TextRange {
        TextRange::new(TextSize::new(start), TextSize::new(end))
    }
    #[test]
    fn ranges_overlap_basic() {
        assert!(ranges_overlap(range(0, 10), range(5, 15)));
        assert!(ranges_overlap(range(5, 15), range(0, 10)));
        assert!(!ranges_overlap(range(0, 10), range(10, 20)));
        assert!(!ranges_overlap(range(0, 10), range(20, 30)));
    }

    #[test]
    fn ranges_overlap_contained() {
        assert!(ranges_overlap(range(0, 100), range(25, 75)));
        assert!(ranges_overlap(range(25, 75), range(0, 100)));
    }
}
