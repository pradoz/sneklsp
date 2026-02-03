use std::path::{Path, PathBuf};
use rustc_hash::FxHashMap;
use crate::{StubModule, StubFunction, StubClass};
use crate::{StubModule, StubFunction, StubClass, loader::StubLoader};

pub struct StubResolver {
    modules: FxHashMap<String, StubModule>,
    builtins: Option<StubModule>,
    search_paths: Vec<PathBuf>,
    loader: StubLoader,
}
