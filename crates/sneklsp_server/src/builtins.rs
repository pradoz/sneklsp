use lsp_types::CompletionItemKind;

pub struct BuiltinInfo {
    pub name: &'static str,
    pub kind: CompletionItemKind,
    pub detail: &'static str,
}

pub static BUILTINS: &[BuiltinInfo] = &[
    BuiltinInfo {
        name: "abs",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "all",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "any",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "ascii",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "bin",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "bool",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "breakpoint",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "bytearray",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "bytes",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "callable",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "chr",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "classmethod",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "compile",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "complex",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "delattr",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "dict",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "dir",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "divmod",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "enumerate",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "eval",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "exec",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "filter",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "float",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "format",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "frozenset",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "getattr",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "globals",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "hasattr",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "hash",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "help",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "hex",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "id",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "input",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "int",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "isinstance",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "issubclass",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "iter",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "len",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "list",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "locals",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "map",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "max",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "memoryview",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "min",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "next",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "object",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "oct",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "open",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "ord",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "pow",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "print",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "property",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "range",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "repr",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "reversed",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "round",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "set",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "setattr",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "slice",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "sorted",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "staticmethod",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "str",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "sum",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "super",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "tuple",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "type",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "vars",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "zip",
        kind: CompletionItemKind::FUNCTION,
        detail: "builtin",
    },
    // constants
    BuiltinInfo {
        name: "True",
        kind: CompletionItemKind::CONSTANT,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "False",
        kind: CompletionItemKind::CONSTANT,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "None",
        kind: CompletionItemKind::CONSTANT,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "Ellipsis",
        kind: CompletionItemKind::CONSTANT,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "__name__",
        kind: CompletionItemKind::CONSTANT,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "__doc__",
        kind: CompletionItemKind::CONSTANT,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "__file__",
        kind: CompletionItemKind::CONSTANT,
        detail: "builtin",
    },
    // common exceptions
    BuiltinInfo {
        name: "Exception",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "BaseException",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "TypeError",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "ValueError",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "KeyError",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "IndexError",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "AttributeError",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "ImportError",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "RuntimeError",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "StopIteration",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "OSError",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "IOError",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "FileNotFoundError",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "NotImplementedError",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
    BuiltinInfo {
        name: "AssertionError",
        kind: CompletionItemKind::CLASS,
        detail: "builtin",
    },
];
