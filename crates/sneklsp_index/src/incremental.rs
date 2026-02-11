use crate::OwnedIndex;
use sneklsp_text::TextRange;

pub fn find_affected_scopes(index: &OwnedIndex, edit_range: TextRange) -> Vec<u32> {
    let mut affected = Vec::new();

    for scope in index.scopes() {
        if scope.kind == crate::ScopeKind::Module {
            continue; // module always overlaps
        }
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

pub fn can_update_incrementally(index: &OwnedIndex, edit_range: TextRange) -> Option<u32> {
    let affected = find_affected_scopes(index, edit_range);

    if affected.len() == 1 {
        let scope = index.scope(affected[0])?;
        if matches!(
            scope.kind,
            crate::ScopeKind::Function | crate::ScopeKind::Class
        ) {
            return Some(affected[0]);
        }
    }

    None
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

    #[test]
    fn single_function_is_incremental() {
        let source = "x = 1\ndef foo():\n    pass\ny = 2";
        let arena = sneklsp_ast::AstArena::new();
        let module = sneklsp_parser::parse(source, &arena).unwrap();
        let idx = crate::index_module(source, &module);
        let owned = crate::OwnedIndex::new(source.to_string(), &idx);

        // edit inside function body
        let edit = TextRange::new(TextSize::new(21), TextSize::new(22));

        let affected = find_affected_scopes(&owned, edit);
        assert_eq!(
            affected.len(),
            1,
            "expected 1 affected scope, got {:?} (scopes: {:?})",
            affected,
            owned
                .scopes()
                .iter()
                .map(|s| (s.id, s.kind, s.range))
                .collect::<Vec<_>>()
        );
        assert!(can_update_incrementally(&owned, edit).is_some());
    }

    #[test]
    fn module_level_edit_not_incremental() {
        let source = "x = 1\ndef foo():\n    pass\ny = 2";
        let arena = sneklsp_ast::AstArena::new();
        let module = sneklsp_parser::parse(source, &arena).unwrap();
        let idx = crate::index_module(source, &module);
        let owned = crate::OwnedIndex::new(source.to_string(), &idx);

        // edit at module level
        let edit = range(0, 5);
        assert!(can_update_incrementally(&owned, edit).is_none());
    }
}
