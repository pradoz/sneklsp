use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    Unknown,
    None,
    Bool,
    Int,
    Float,
    Str,
    Bytes,
    List(Arc<Ty>),
    Tuple(Arc<[Ty]>),
    Dict(Arc<Ty>, Arc<Ty>),
    Set(Arc<Ty>),
    Optional(Arc<Ty>),
    Union(Arc<[Ty]>),
    Callable { params: Arc<[Ty]>, ret: Arc<Ty> },
    Class(String),
    Module(String),
    Ellipsis,
}

impl Ty {
    #[inline]
    pub fn is_unknown(&self) -> bool {
        matches!(self, Ty::Unknown)
    }

    pub fn display(&self) -> String {
        match self {
            Ty::Unknown => "Unknown".to_string(),
            Ty::None => "None".to_string(),
            Ty::Bool => "bool".to_string(),
            Ty::Int => "int".to_string(),
            Ty::Float => "float".to_string(),
            Ty::Str => "str".to_string(),
            Ty::Bytes => "bytes".to_string(),
            Ty::Ellipsis => "...".to_string(),
            Ty::List(inner) => format!("list[{}]", inner.display()),
            Ty::Set(inner) => format!("set[{}]", inner.display()),
            Ty::Tuple(elts) => {
                let inner: Vec<_> = elts.iter().map(|t| t.display()).collect();
                format!("tuple[{}]", inner.join(", "))
            }
            Ty::Dict(k, v) => format!("dict[{}, {}]", k.display(), v.display()),
            Ty::Optional(inner) => format!("{} | None", inner.display()),
            Ty::Union(members) => {
                let parts: Vec<_> = members.iter().map(|t| t.display()).collect();
                parts.join(" | ")
            }
            Ty::Callable { params, ret } => {
                let param_strs: Vec<_> = params.iter().map(|t| t.display()).collect();
                format!("({}) -> {}", param_strs.join(", "), ret.display())
            }
            Ty::Class(name) => name.clone(),
            Ty::Module(name) => format!("module '{}'", name),
        }
    }
}

pub fn infer_symbol_type(
    index: &sneklsp_index::OwnedIndex,
    symbol: &sneklsp_index::SymbolData,
) -> Ty {
    match symbol.kind {
        sneklsp_index::SymbolKind::Function | sneklsp_index::SymbolKind::Method => Ty::Callable {
            params: Arc::from([]),
            ret: Arc::new(Ty::Unknown),
        },
        sneklsp_index::SymbolKind::Class => Ty::Class(index.symbol_name(symbol).to_string()),
        sneklsp_index::SymbolKind::Import => Ty::Module(index.symbol_name(symbol).to_string()),
        sneklsp_index::SymbolKind::Variable => infer_variable_type(index, symbol),
        sneklsp_index::SymbolKind::Parameter => infer_parameter_type(index, symbol),
        _ => Ty::Unknown,
    }
}

fn infer_parameter_type(
    index: &sneklsp_index::OwnedIndex,
    symbol: &sneklsp_index::SymbolData,
) -> Ty {
    let source = index.source();
    let start = symbol.range.start().to_usize();
    let end = symbol.range.end().to_usize().min(source.len());
    let slice = &source[start..end];

    // parameter range includes `name: annotation = default`
    let name = index.symbol_name(symbol);
    let after_name = if slice.starts_with(name) {
        &slice[name.len()..]
    } else {
        slice
    };

    let trimmed = after_name.trim_start();
    if !trimmed.starts_with(':') {
        return Ty::Unknown;
    }

    // extract annotation text between `:` and `=`
    let annotation_start = &trimmed[1..];
    let annotation = match annotation_start.find('=') {
        Some(eq_pos) => annotation_start[..eq_pos].trim(),
        None => annotation_start.trim(),
    };

    annotation_to_type(annotation)
}

fn infer_variable_type(
    index: &sneklsp_index::OwnedIndex,
    symbol: &sneklsp_index::SymbolData,
) -> Ty {
    let source = index.source();
    let name_start = symbol.selection_range.start().to_usize();

    // Find the line containing this symbol
    let line_start = source[..name_start].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let line_end = source[name_start..]
        .find('\n')
        .map(|p| name_start + p)
        .unwrap_or(source.len());
    let line = &source[line_start..line_end];

    // Find `=` that isn't `==`, `!=`, `<=`, `>=`, `:=`
    let Some(rhs) = find_assignment_rhs(line) else {
        return Ty::Unknown;
    };

    infer_literal_type(rhs.trim())
}

fn find_assignment_rhs(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'=' => {
                // skip strict equal `==`
                if i + 1 < len && bytes[i + 1] == b'=' {
                    i += 2;
                    continue;
                }
                // skip non-equality comparisons `!=`, `<=`, `>=`, `:=`
                if i > 0 && matches!(bytes[i - 1], b'!' | b'<' | b'>' | b':') {
                    i += 1;
                    continue;
                }
                // skip augmented assign `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`
                if i > 0
                    && matches!(
                        bytes[i - 1],
                        b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^'
                    )
                {
                    i += 1;
                    continue;
                }
                return Some(&line[i + 1..]);
            }
            // Don't search inside strings
            b'\'' | b'"' => return None,
            // Don't search past comment
            b'#' => return None,
            _ => {}
        }
        i += 1;
    }
    None
}

fn infer_literal_type(text: &str) -> Ty {
    if text.is_empty() {
        return Ty::Unknown;
    }

    match text {
        "None" => return Ty::None,
        "True" | "False" => return Ty::Bool,
        "..." => return Ty::Ellipsis,
        "[]" => return Ty::List(Arc::new(Ty::Unknown)),
        "{}" => return Ty::Dict(Arc::new(Ty::Unknown), Arc::new(Ty::Unknown)),
        "()" => return Ty::Tuple(Arc::from([])),
        _ => {}
    }

    // int literal
    let num_text = text.strip_prefix('-').unwrap_or(text);
    if !num_text.is_empty() && num_text.bytes().all(|b| b.is_ascii_digit() || b == b'_') {
        return Ty::Int;
    }

    // float literal
    if text.contains('.') || text.contains('e') || text.contains('E') {
        let cleaned = text.replace('_', "");
        if cleaned.parse::<f64>().is_ok() {
            return Ty::Float;
        }
    }

    // string/fstring/bytes prefixes
    let first = text.as_bytes()[0];
    match first {
        b'"' | b'\'' => return Ty::Str,
        b'f' | b'F' if text.len() > 1 && matches!(text.as_bytes()[1], b'"' | b'\'') => {
            return Ty::Str;
        }
        b'b' | b'B' if text.len() > 1 && matches!(text.as_bytes()[1], b'"' | b'\'') => {
            return Ty::Bytes;
        }
        b'r' | b'R' if text.len() > 1 && matches!(text.as_bytes()[1], b'"' | b'\'') => {
            return Ty::Str;
        }
        _ => {}
    }

    Ty::Unknown
}

fn annotation_to_type(text: &str) -> Ty {
    if text.is_empty() {
        return Ty::Unknown;
    }

    match text {
        "int" => Ty::Int,
        "float" => Ty::Float,
        "str" => Ty::Str,
        "bool" => Ty::Bool,
        "bytes" => Ty::Bytes,
        "None" => Ty::None,
        "object" => Ty::Class("object".to_string()),
        _ => {
            // list[X], dict[K, V], etc; just return class name for now
            if text.contains('[') {
                let bracket = text.find('[').unwrap();
                let base = &text[..bracket];
                match base {
                    "list" | "List" => Ty::List(Arc::new(Ty::Unknown)),
                    "dict" | "Dict" => Ty::Dict(Arc::new(Ty::Unknown), Arc::new(Ty::Unknown)),
                    "set" | "Set" => Ty::Set(Arc::new(Ty::Unknown)),
                    "tuple" | "Tuple" => Ty::Tuple(Arc::from([])),
                    "Optional" => Ty::Optional(Arc::new(Ty::Unknown)),
                    _ => Ty::Class(text.to_string()),
                }
            } else {
                Ty::Class(text.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotation_types() {
        // basic
        assert_eq!(annotation_to_type("int"), Ty::Int);
        assert_eq!(annotation_to_type("str"), Ty::Str);
        assert_eq!(annotation_to_type("float"), Ty::Float);
        assert_eq!(annotation_to_type("bool"), Ty::Bool);
        assert_eq!(annotation_to_type("None"), Ty::None);
        // containers
        assert_eq!(
            annotation_to_type("list[int]"),
            Ty::List(Arc::new(Ty::Unknown))
        );
        assert_eq!(
            annotation_to_type("dict[str, int]"),
            Ty::Dict(Arc::new(Ty::Unknown), Arc::new(Ty::Unknown))
        );
        // class
        assert_eq!(
            annotation_to_type("MyClass"),
            Ty::Class("MyClass".to_string())
        );
    }

    #[test]
    fn literals() {
        // integer
        assert_eq!(infer_literal_type("42"), Ty::Int);
        assert_eq!(infer_literal_type("1_000"), Ty::Int);
        assert_eq!(infer_literal_type("-5"), Ty::Int);
        // float
        assert_eq!(infer_literal_type("3.14"), Ty::Float);
        assert_eq!(infer_literal_type("1e10"), Ty::Float);
        // string/bytes
        assert_eq!(infer_literal_type("\"hello\""), Ty::Str);
        assert_eq!(infer_literal_type("'hello'"), Ty::Str);
        assert_eq!(infer_literal_type("f\"hi\""), Ty::Str);
        assert_eq!(infer_literal_type("b\"data\""), Ty::Bytes);
        // bool
        assert_eq!(infer_literal_type("True"), Ty::Bool);
        assert_eq!(infer_literal_type("False"), Ty::Bool);
        // null
        assert_eq!(infer_literal_type("None"), Ty::None);
        // list
        assert_eq!(infer_literal_type("[]"), Ty::List(Arc::new(Ty::Unknown)));
        // tuple
        assert_eq!(infer_literal_type("()"), Ty::Tuple(Arc::from([])));
        // unknown
        assert_eq!(infer_literal_type("foo()"), Ty::Unknown);
        assert_eq!(infer_literal_type("a + b"), Ty::Unknown);
    }

    #[test]
    fn find_rhs() {
        // simple
        assert_eq!(find_assignment_rhs("x = 42").unwrap().trim(), "42");
        assert_eq!(
            find_assignment_rhs("name = \"hello\"").unwrap().trim(),
            "\"hello\""
        );
        // skip comparison
        assert!(find_assignment_rhs("x == 42").is_none());
        assert!(find_assignment_rhs("x != 42").is_none());
        assert!(find_assignment_rhs("x >= 42").is_none());
        // skip augmented
        assert!(find_assignment_rhs("x += 1").is_none());
        assert!(find_assignment_rhs("x -= 1").is_none());
        // no equal sign
        assert!(find_assignment_rhs("print(x)").is_none());
        assert!(find_assignment_rhs("return x").is_none());
    }

    #[test]
    fn display_types() {
        assert_eq!(Ty::Int.display(), "int");
        assert_eq!(Ty::List(Arc::new(Ty::Str)).display(), "list[str]");
        assert_eq!(
            Ty::Dict(Arc::new(Ty::Str), Arc::new(Ty::Int)).display(),
            "dict[str, int]"
        );
        assert_eq!(Ty::Optional(Arc::new(Ty::Int)).display(), "int | None");
    }

    #[test]
    fn infer_from_source_line() {
        let source = "x = 42\ny = \"hello\"\nz = True".to_string();
        let arena = sneklsp_ast::AstArena::new();
        let module = sneklsp_parser::parse(&source, &arena).unwrap();
        let idx = sneklsp_index::index_module(&source, &module);
        let owned = sneklsp_index::OwnedIndex::new(source.clone(), &idx);

        let x = owned.symbol_at(sneklsp_text::TextSize::new(0)).unwrap();
        assert_eq!(infer_symbol_type(&owned, x), Ty::Int);

        let y = owned.symbol_at(sneklsp_text::TextSize::new(7)).unwrap();
        assert_eq!(infer_symbol_type(&owned, y), Ty::Str);

        let z = owned.symbol_at(sneklsp_text::TextSize::new(19)).unwrap();
        assert_eq!(infer_symbol_type(&owned, z), Ty::Bool);
    }
}
