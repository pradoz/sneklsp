use crate::handlers::to_lsp_range;
use lsp_types::{Diagnostic, DiagnosticSeverity, DiagnosticTag, Position, Range};
use sneklsp_index::OwnedIndex;
use sneklsp_parser::ParseError;
use sneklsp_text::LineIndex;

#[inline]
pub fn parse_diagnostics(errors: &[ParseError], line_index: &LineIndex) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::with_capacity(errors.len());
    for e in errors {
        diagnostics.push(to_parse_diagnostic(e, line_index));
    }
    diagnostics
}

pub fn semantic_diagnostics(index: &OwnedIndex, line_index: &LineIndex) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    unresolved_reference_diagnostics(index, line_index, &mut diagnostics);
    unused_import_diagnostics(index, line_index, &mut diagnostics);
    diagnostics
}

fn unresolved_reference_diagnostics(
    index: &OwnedIndex,
    line_index: &LineIndex,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for reference in index.references() {
        if reference.resolved.is_some() {
            continue;
        }

        let name = index.reference_name(reference);

        // builtins are valid but not indexed
        // dunder names are probably magic globals
        if crate::builtins::lookup(name).is_some() || name.starts_with("__") && name.ends_with("__")
        {
            continue;
        }

        let range = to_lsp_range(reference.range, line_index);
        diagnostics.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::WARNING),
            code: None,
            code_description: None,
            source: Some("sneklsp".to_string()),
            message: format!("'{}' is possibly undefined", name),
            related_information: None,
            tags: None,
            data: None,
        });
    }
}

fn unused_import_diagnostics(
    index: &OwnedIndex,
    line_index: &LineIndex,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for symbol in index.symbols() {
        if !matches!(
            symbol.kind,
            sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol
        ) {
            continue;
        }

        let has_refs = index.references_to(symbol.id).next().is_some();
        if has_refs {
            continue;
        }

        let name = index.symbol_name(symbol);
        let range = to_lsp_range(symbol.selection_range, line_index);
        diagnostics.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::HINT),
            code: None,
            code_description: None,
            source: Some("sneklsp".to_string()),
            message: format!("'{}' is imported but unused", name),
            related_information: None,
            tags: Some(vec![DiagnosticTag::UNNECESSARY]),
            data: None,
        });
    }
}

#[inline]
fn zero_width_range(line: u32, character: u32) -> Range {
    let position = Position { line, character };
    Range {
        start: position,
        end: position,
    }
}

#[inline]
fn to_parse_diagnostic(error: &ParseError, line_index: &LineIndex) -> Diagnostic {
    let (range, message) = match error {
        ParseError::UnexpectedToken {
            offset,
            expected,
            found,
        } => {
            let pos = line_index.position(*offset);
            let start = Position {
                line: pos.line,
                character: pos.column,
            };
            let end = Position {
                line: pos.line,
                character: pos.column + 1,
            };
            (
                Range { start, end },
                format!("expected {expected}, found {found}"),
            )
        }

        ParseError::UnexpectedEof => {
            let line = line_index.line_count().saturating_sub(1) as u32;
            (
                zero_width_range(line, 0),
                "unexpected end of file".to_string(),
            )
        }

        ParseError::InvalidSyntax(offset) => {
            let pos = line_index.position(*offset);
            let start = Position {
                line: pos.line,
                character: pos.column,
            };
            let end = Position {
                line: pos.line,
                character: pos.column + 1,
            };
            (Range { start, end }, "invalid syntax".to_string())
        }
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("sneklsp".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}
