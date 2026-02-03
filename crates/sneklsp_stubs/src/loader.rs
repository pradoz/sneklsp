use crate::parser::StubParser;
use crate::{ParameterKind, StubClass, StubFunction, StubModule, StubParameter, TypeAnnotation};
use rustc_hash::FxHashMap;
use std::path::Path;

pub struct StubLoader {
    cache: FxHashMap<String, StubModule>,
}

impl StubLoader {
    pub fn new() -> Self {
        Self {
            cache: FxHashMap::default(),
        }
    }

    pub fn load_bundled_builtins(&self) -> Option<StubModule> {
        let source = include_str!("bundled/builtins.pyi");
        Some(StubParser::parse_module("builtins", source))
    }

    pub fn load_bundled_module(&self, name: &str) -> Option<StubModule> {
        // Map module name to bundled file
        let source = match name {
            "os" => include_str!("bundled/os.pyi"),
            // "sys" => include_str!("bundled/sys.pyi"),
            // "typing" => include_str!("bundled/typing.pyi"),
            // "collections" => include_str!("bundled/collections.pyi"),
            // "pathlib" => include_str!("bundled/pathlib.pyi"),
            // "json" => include_str!("bundled/json.pyi"),
            // "re" => include_str!("bundled/re.pyi"),
            // "io" => include_str!("bundled/io.pyi"),
            // "abc" => include_str!("bundled/abc.pyi"),
            // "dataclasses" => include_str!("bundled/dataclasses.pyi"),
            _ => return None,
        };

        Some(StubParser::parse_module(name, source))
    }
}
