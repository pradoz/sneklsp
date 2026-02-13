use crate::OwnedIndex;
use sneklsp_text::TextRange;

pub fn find_affected_scopes(index: &OwnedIndex, edit_range: TextRange) -> Vec<u32> {
    let mut affected = Vec::new();

    for scope in index.scopes() {
        if scope.kind == crate::ScopeKind::Module {
            continue; // module always overlaps
        }
        if scope.range.overlaps(edit_range) {
            affected.push(scope.id);
        }
    }

    affected
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
        let edit = TextRange::new(TextSize::new(0), TextSize::new(5));
        assert!(can_update_incrementally(&owned, edit).is_none());
    }
}
