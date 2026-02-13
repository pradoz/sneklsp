use lsp_types::{Diagnostic, DiagnosticSeverity, DiagnosticTag, Position, Range};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::analysis::AnalysisHost;
use crate::handlers::to_lsp_range;
use sneklsp_db::{ParseErrorKind, SerializedParseError};
use sneklsp_index::OwnedIndex;
use sneklsp_text::LineIndex;

pub fn serialized_errors_to_diagnostics(
    errors: &[SerializedParseError],
    line_index: &LineIndex,
) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|e| serialized_error_to_diagnostic(e, line_index))
        .collect()
}

pub fn semantic_diagnostics(
    index: &OwnedIndex,
    line_index: &LineIndex,
    analysis: &crate::analysis::AnalysisHost,
) -> Vec<Diagnostic> {
    let mut collector = DiagnosticCollector::new(index, line_index, analysis);
    collector.run();
    collector.diagnostics
}

pub fn scoped_semantic_diagnostics(
    index: &OwnedIndex,
    line_index: &LineIndex,
    analysis: &crate::analysis::AnalysisHost,
    edit_range: sneklsp_text::TextRange,
) -> Vec<Diagnostic> {
    let mut collector = DiagnosticCollector::new(index, line_index, analysis);
    collector.run_scoped(edit_range);
    collector.diagnostics
}

struct DiagnosticCollector<'a> {
    index: &'a OwnedIndex,
    line_index: &'a LineIndex,
    diagnostics: Vec<Diagnostic>,
    ref_counts: FxHashMap<u32, u32>,
    cross_file_names: FxHashSet<String>,
}

impl<'a> DiagnosticCollector<'a> {
    fn new(index: &'a OwnedIndex, line_index: &'a LineIndex, analysis: &AnalysisHost) -> Self {
        let mut ref_counts: FxHashMap<u32, u32> = FxHashMap::default();
        for reference in index.references() {
            if let Some(sym_id) = reference.resolved {
                *ref_counts.entry(sym_id).or_default() += 1;
            }
        }

        let mut cross_file_names = FxHashSet::default();
        for file_id in analysis.file_ids() {
            if let Some(exports) = analysis.exported_symbols(file_id) {
                for export in exports {
                    cross_file_names.insert(export.name.clone());
                }
            }
        }

        Self {
            index,
            line_index,
            diagnostics: Vec::new(),
            ref_counts,
            cross_file_names,
        }
    }

    fn run(&mut self) {
        self.check_references();
        self.check_symbols();
        self.check_scopes();
    }

    fn run_scoped(&mut self, edit_range: sneklsp_text::TextRange) {
        self.check_references_in_range(edit_range);
        self.check_symbols_in_range(edit_range);
    }

    fn check_references(&mut self) {
        for reference in self.index.references() {
            if reference.resolved.is_some() {
                continue;
            }

            let name = self.index.reference_name(reference);

            if crate::builtins::lookup(name).is_some() {
                continue;
            }

            if name.starts_with("__") && name.ends_with("__") {
                continue;
            }

            if self.cross_file_names.contains(name) {
                continue;
            }

            self.push(
                reference.range,
                DiagnosticSeverity::WARNING,
                format!("'{}' is possibly undefined", name),
                None,
            );
        }
    }

    fn check_symbols(&mut self) {
        let mut import_names: FxHashSet<String> = FxHashSet::default();

        for symbol in self.index.symbols() {
            let name = self.index.symbol_name(symbol);
            let has_refs = self.ref_counts.get(&symbol.id).copied().unwrap_or(0) > 0;

            match symbol.kind {
                sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol => {
                    import_names.insert(name.to_string());

                    if !has_refs {
                        self.push(
                            symbol.selection_range,
                            DiagnosticSeverity::HINT,
                            format!("'{}' is imported but unused", name),
                            Some(vec![DiagnosticTag::UNNECESSARY]),
                        );
                    }
                }

                sneklsp_index::SymbolKind::Variable => {
                    if import_names.contains(name) {
                        self.push(
                            symbol.selection_range,
                            DiagnosticSeverity::HINT,
                            format!("'{}' shadows an import of the same name", name),
                            None,
                        );
                    }

                    if !has_refs && !name.starts_with('_') && symbol.scope != 0 {
                        self.push(
                            symbol.selection_range,
                            DiagnosticSeverity::HINT,
                            format!("'{}' is assigned but never used", name),
                            Some(vec![DiagnosticTag::UNNECESSARY]),
                        );
                    }
                }

                _ => {}
            }
        }
    }

    fn check_scopes(&mut self) {
        for scope in self.index.scopes() {
            self.check_duplicate_definitions(scope);
            self.check_missing_self(scope);
        }
    }

    fn check_references_in_range(&mut self, edit_range: sneklsp_text::TextRange) {
        for reference in self.index.references() {
            if !ranges_overlap_text(reference.range, edit_range) {
                continue;
            }

            if reference.resolved.is_some() {
                continue;
            }

            let name = self.index.reference_name(reference);

            if crate::builtins::lookup(name).is_some() {
                continue;
            }

            if name.starts_with("__") && name.ends_with("__") {
                continue;
            }

            if self.cross_file_names.contains(name) {
                continue;
            }

            self.push(
                reference.range,
                DiagnosticSeverity::WARNING,
                format!("'{}' is possibly undefined", name),
                None,
            );
        }
    }

    fn check_symbols_in_range(&mut self, edit_range: sneklsp_text::TextRange) {
        let mut import_names: FxHashSet<String> = FxHashSet::default();

        for symbol in self.index.symbols() {
            if matches!(
                symbol.kind,
                sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol
            ) {
                import_names.insert(self.index.symbol_name(symbol).to_string());
            }

            if !ranges_overlap_text(symbol.range, edit_range) {
                continue;
            }

            let name = self.index.symbol_name(symbol);
            let has_refs = self.ref_counts.get(&symbol.id).copied().unwrap_or(0) > 0;

            match symbol.kind {
                sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol => {
                    if !has_refs {
                        self.push(
                            symbol.selection_range,
                            DiagnosticSeverity::HINT,
                            format!("'{}' is imported but unused", name),
                            Some(vec![DiagnosticTag::UNNECESSARY]),
                        );
                    }
                }

                sneklsp_index::SymbolKind::Variable => {
                    if import_names.contains(name) {
                        self.push(
                            symbol.selection_range,
                            DiagnosticSeverity::HINT,
                            format!("'{}' shadows an import of the same name", name),
                            None,
                        );
                    }

                    if !has_refs && !name.starts_with('_') && symbol.scope != 0 {
                        self.push(
                            symbol.selection_range,
                            DiagnosticSeverity::HINT,
                            format!("'{}' is assigned but never used", name),
                            Some(vec![DiagnosticTag::UNNECESSARY]),
                        );
                    }
                }

                _ => {}
            }
        }
    }

    fn check_duplicate_definitions(&mut self, scope: &sneklsp_index::ScopeData) {
        let mut seen: FxHashMap<&str, (u32, sneklsp_text::TextRange)> = FxHashMap::default();

        for &sym_id in &scope.symbols {
            let Some(symbol) = self.index.symbol(sym_id) else {
                continue;
            };

            if !matches!(
                symbol.kind,
                sneklsp_index::SymbolKind::Function
                    | sneklsp_index::SymbolKind::Method
                    | sneklsp_index::SymbolKind::Class
            ) {
                continue;
            }

            let name = self.index.symbol_name(symbol);

            if let Some(&(first_id, first_range)) = seen.get(name) {
                if first_id != sym_id {
                    let first_pos = self.line_index.position(first_range.start());
                    self.push(
                        symbol.selection_range,
                        DiagnosticSeverity::WARNING,
                        format!(
                            "'{}' is already defined on line {}",
                            name,
                            first_pos.line + 1
                        ),
                        None,
                    );
                }
            } else {
                seen.insert(name, (sym_id, symbol.selection_range));
            }
        }
    }

    fn check_missing_self(&mut self, scope: &sneklsp_index::ScopeData) {
        if scope.kind != sneklsp_index::ScopeKind::Class {
            return;
        }

        for &child_id in &scope.children {
            let Some(child_scope) = self.index.scope(child_id) else {
                continue;
            };

            if child_scope.kind != sneklsp_index::ScopeKind::Function {
                continue;
            }

            let Some(method) = self.index.find_scope_owner(scope, child_scope) else {
                continue;
            };
            let method_name = self.index.symbol_name(method);

            if method_name.starts_with("__") && method_name.ends_with("__") {
                continue;
            }

            if !self.index.scope_has_self_or_cls(child_scope) && !child_scope.symbols.is_empty() {
                let has_any_param = child_scope.symbols.iter().any(|&sym_id| {
                    self.index
                        .symbol(sym_id)
                        .map_or(false, |s| s.kind == sneklsp_index::SymbolKind::Parameter)
                });

                if has_any_param {
                    self.push(
                        method.selection_range,
                        DiagnosticSeverity::WARNING,
                        format!(
                            "method '{}' does not have 'self' or 'cls' as first parameter",
                            method_name
                        ),
                        None,
                    );
                } else {
                    self.push(
                        method.selection_range,
                        DiagnosticSeverity::WARNING,
                        format!("method '{}' is missing 'self' parameter", method_name),
                        None,
                    );
                }
            }
        }
    }

    #[inline]
    fn push(
        &mut self,
        range: sneklsp_text::TextRange,
        severity: DiagnosticSeverity,
        message: String,
        tags: Option<Vec<DiagnosticTag>>,
    ) {
        self.diagnostics.push(Diagnostic {
            range: to_lsp_range(range, self.line_index),
            severity: Some(severity),
            code: None,
            code_description: None,
            source: Some("sneklsp".to_string()),
            message,
            related_information: None,
            tags,
            data: None,
        });
    }
}

#[inline]
fn ranges_overlap_text(a: sneklsp_text::TextRange, b: sneklsp_text::TextRange) -> bool {
    a.start() < b.end() && b.start() < a.end()
}

#[inline]
fn zero_width_range(line: u32, character: u32) -> Range {
    let position = Position { line, character };
    Range {
        start: position,
        end: position,
    }
}

fn serialized_error_to_diagnostic(
    error: &SerializedParseError,
    line_index: &LineIndex,
) -> Diagnostic {
    let range = match &error.kind {
        ParseErrorKind::UnexpectedToken { range: error_range } => {
            to_lsp_range(*error_range, line_index)
        }
        ParseErrorKind::UnexpectedEof => {
            let line = line_index.line_count().saturating_sub(1) as u32;
            zero_width_range(line, 0)
        }
        ParseErrorKind::InvalidSyntax { offset } => {
            let pos = line_index.position(*offset);
            let start = Position {
                line: pos.line,
                character: pos.column,
            };
            let end = Position {
                line: pos.line,
                character: pos.column + 1,
            };
            Range { start, end }
        }
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("sneklsp".to_string()),
        message: error.message.clone(),
        related_information: None,
        tags: None,
        data: None,
    }
}
