use rustc_hash::FxHashMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeAnnotation {
    Name(String),
    Subscript(String, Vec<TypeAnnotation>),
    Union(Vec<TypeAnnotation>),
    Optional(Box<TypeAnnotation>),
    Callable {
        params: Vec<TypeAnnotation>,
        returns: Box<TypeAnnotation>,
    },
    Tuple(Vec<TypeAnnotation>),
    Literal(Vec<String>),
    Any,
    None,
    Unknown(String),
}

impl TypeAnnotation {
    pub fn display(&self) -> String {
        match self {
            Self::Name(n) => n.clone(),
            Self::Subscript(base, args) => {
                let args_str: Vec<_> = args.iter().map(|a| a.display()).collect();
                format!("{}[{}]", base, args_str.join(", "))
            }
            Self::Union(types) => {
                let types_str: Vec<_> = types.iter().map(|t| t.display()).collect();
                types_str.join(" | ")
            }
            Self::Optional(inner) => format!("{} | None", inner.display()),
            Self::Callable { params, returns } => {
                let params_str: Vec<_> = params.iter().map(|p| p.display()).collect();
                format!("({}) -> {}", params_str.join(", "), returns.display())
            }
            Self::Tuple(elts) => {
                let elts_str: Vec<_> = elts.iter().map(|e| e.display()).collect();
                format!("tuple[{}]", elts_str.join(", "))
            }
            Self::Literal(values) => format!("Literal[{}]", values.join(", ")),
            Self::Any => "Any".to_string(),
            Self::None => "None".to_string(),
            Self::Unknown(s) => s.clone(),
        }
    }

    pub fn parse(s: &str) -> Self {
        let s = s.trim();

        if s.is_empty() {
            return Self::Unknown(String::new());
        }
        if s == "None" {
            return Self::None;
        }
        if s == "Any" {
            return Self::Any;
        }

        if s.contains(" | ") {
            let parts: Vec<_> = s.split(" | ").map(|p| Self::parse(p)).collect();
            return Self::Union(parts);
        }

        if let Some(bracket_pos) = s.find('[') {
            if s.ends_with(']') {
                let base = &s[..bracket_pos];
                let args_str = &s[bracket_pos + 1..s.len() - 1];

                match base {
                    "Optional" => {
                        let inner = Self::parse(args_str);
                        return Self::Optional(Box::new(inner));
                    }
                    "Union" => {
                        let args = Self::parse_comma_separated(args_str);
                        return Self::Union(args);
                    }
                    "Callable" => {
                        // TODO
                        return Self::Unknown(s.to_string());
                    }
                    "Literal" => {
                        let values: Vec<_> =
                            args_str.split(',').map(|v| v.trim().to_string()).collect();
                        return Self::Literal(values);
                    }
                    _ => {
                        let args = Self::parse_comma_separated(args_str);
                        return Self::Subscript(base.to_string(), args);
                    }
                }
            }
        }

        Self::Name(s.to_string())
    }

    fn parse_comma_separated(s: &str) -> Vec<TypeAnnotation> {
        let mut result = Vec::new();
        let mut current = String::new();
        let mut depth = 0;

        for c in s.chars() {
            match c {
                '[' => {
                    depth += 1;
                    current.push(c);
                }
                ']' => {
                    depth -= 1;
                    current.push(c);
                }
                ',' if depth == 0 => {
                    if !current.trim().is_empty() {
                        result.push(TypeAnnotation::parse(current.trim()));
                    }
                    current.clear();
                }
                _ => current.push(c),
            }
        }

        if !current.trim().is_empty() {
            result.push(TypeAnnotation::parse(current.trim()));
        }

        result
    }
}

impl std::fmt::Display for TypeAnnotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    Normal,
    VarPositional,
    VarKeyword,
    PositionalOnly,
    KeywordOnly,
}

#[derive(Debug, Clone)]
pub struct StubParameter {
    pub name: String,
    pub annotation: Option<TypeAnnotation>,
    pub default: Option<String>,
    pub kind: ParameterKind,
}

#[derive(Debug, Clone)]
pub struct StubFunction {
    pub name: String,
    pub params: Vec<StubParameter>,
    pub returns: Option<TypeAnnotation>,
    pub docstrign: Option<String>,
    pub is_async: bool,
    pub overloads: Vec<StubFunction>,
}

impl StubFunction {
    pub fn signature(&self) -> String {
        let params: Vec<String> = self
            .params
            .iter()
            .map(|p| {
                let mut s = p.name.clone();
                if let Some(ann) = &p.annotation {
                    s.push_str(": ");
                    s.push_str(&ann.display());
                }
                if let Some(default) = &p.default {
                    s.push_str(" = ");
                    s.push_str(default);
                }
                s
            })
            .collect();

        let mut sig = format!("{}({})", self.name, params.join(", "));

        if let Some(ret) = &self.returns {
            sig.push_str(" -> ");
            sig.push_str(&ret.display());
        }
        sig
    }
}

#[derive(Debug, Clone)]
pub struct StubClass {
    pub name: String,
    pub bases: Vec<String>,
    pub methods: FxHashMap<String, StubFunction>,
    pub attributes: FxHashMap<String, TypeAnnotation>,
    pub docstring: Option<String>,
}

impl StubClass {
    pub fn new(name: String) -> Self {
        Self {
            name,
            bases: Vec::new(),
            methods: FxHashMap::default(),
            attributes: FxHashMap::default(),
            docstring: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StubModule {
    pub name: String,
    pub functions: FxHashMap<String, StubFunction>,
    pub classes: FxHashMap<String, StubClass>,
    pub variables: FxHashMap<String, TypeAnnotation>,
    pub submodules: Vec<String>,
    pub docstring: Option<String>,
}

impl StubModule {
    pub fn new(name: String) -> Self {
        Self {
            name,
            functions: FxHashMap::default(),
            classes: FxHashMap::default(),
            variables: FxHashMap::default(),
            submodules: Vec::new(),
            docstring: None,
        }
    }

    #[inline]
    pub fn get_function(&self, name: &str) -> Option<&StubFunction> {
        self.functions.get(name)
    }

    #[inline]
    pub fn get_class(&self, name: &str) -> Option<&StubClass> {
        self.classes.get(name)
    }
}
