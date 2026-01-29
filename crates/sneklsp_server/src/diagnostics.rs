use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use sneklsp_parser::ParseError;
use sneklsp_text::LineIndex;

pub fn to_diagnostics(errors: &[ParseError], line_index: &LineIndex) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|e| to_diagnostic(e, line_index))
        .collect()
}

fn to_diagnostic(error: &ParseError, line_index: &LineIndex) -> Diagnostic {
    let (range, message) = match error {
        ParseError::UnexpectedToken {
            offset,
            expected,
            found,
        } => {
            let pos = line_index.position(*offset);
            let position = Position {
                line: pos.line,
                character: pos.column,
            };
            let range = Range {
                start: position,
                end: Position {
                    line: pos.line,
                    character: pos.column + 1,
                },
            };

            (range, format!("expected {expected}, found {found}"))
        }

        ParseError::UnexpectedEof => {
            let position = Position {
                line: line_index.line_count().saturating_sub(1) as u32,
                character: 0,
            };
            let range = Range {
                start: position,
                end: position,
            };

            (range, "unexpected end of file".to_string())
        }

        ParseError::InvalidSyntax(offset) => {
            let pos = line_index.position(*offset);
            let position = Position {
                line: pos.line,
                character: pos.column,
            };
            let range = Range {
                start: position,
                end: Position {
                    line: pos.line,
                    character: pos.column + 1,
                },
            };

            (range, "invalid syntax".to_string())
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
