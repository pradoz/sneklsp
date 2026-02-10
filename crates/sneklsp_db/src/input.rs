#[salsa::input]
pub struct SourceProgram {
    #[returns(ref)]
    pub workspace_roots: Vec<String>,
}

#[salsa::input]
pub struct File {
    #[returns(ref)]
    pub path: String,

    #[returns(ref)]
    pub content: String,
}

#[salsa::interned]
pub struct ModuleName<'db> {
    #[returns(ref)]
    pub name: String,
}

#[salsa::input]
pub struct ModuleGraph {
    #[returns(ref)]
    pub entries: Vec<ModuleEntry>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ModuleEntry {
    pub name: String,
    pub file: File,
}
