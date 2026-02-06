use sneklsp_text::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // id
    Name,

    // literals
    Int,
    Float,
    String,

    // keywords
    And,
    As,
    Assert,
    Async,
    Await,
    Break,
    Class,
    Continue,
    Def,
    Del,
    Elif,
    Else,
    Except,
    False,
    Finally,
    For,
    From,
    Global,
    If,
    Import,
    In,
    Is,
    Lambda,
    None,
    Nonlocal,
    Not,
    Or,
    Pass,
    Raise,
    Return,
    True,
    Try,
    While,
    With,
    Yield,

    // bracketry things
    LParen,   // (
    RParen,   // )
    LBracket, // [
    RBracket, // ]
    LBrace,   // {
    RBrace,   // }
    Colon,    // :
    Comma,    // ,
    Semi,     // ;
    Dot,      // .
    Arrow,    // ->
    At,       // @

    // operators
    Plus,        // +
    Minus,       // -
    Star,        // *
    Slash,       // /
    DoubleSlash, // //
    Percent,     // %
    DoubleStar,  // **
    Amp,         // &
    Pipe,        // |
    Caret,       // ^
    Tilde,       // ~
    LtLt,        // <<
    GtGt,        // >>

    // comparison
    Eq,    // =
    EqEq,  // ==
    NotEq, // !=
    Lt,    // <
    LtEq,  // <=
    Gt,    // >
    GtEq,  // >=

    // augmented assignment
    PlusEq,    // +=
    MinusEq,   // -=
    StarEq,    // *=
    SlashEq,   // /=
    PercentEq, // %=
    AmpEq,     // &=
    PipeEq,    // |=
    CaretEq,   // ^=

    // whitespace
    Newline,
    Indent,
    Dedent,

    // fancy
    ColonEq,
    Ellipsis,

    // special
    Eof,
    Error,
}

impl TokenKind {
    #[inline]
    pub fn from_keyword(text: &str) -> Option<Self> {
        let kind = match text {
            "and" => Self::And,
            "as" => Self::As,
            "assert" => Self::Assert,
            "async" => Self::Async,
            "await" => Self::Await,
            "break" => Self::Break,
            "class" => Self::Class,
            "continue" => Self::Continue,
            "def" => Self::Def,
            "del" => Self::Del,
            "elif" => Self::Elif,
            "else" => Self::Else,
            "except" => Self::Except,
            "False" => Self::False,
            "finally" => Self::Finally,
            "for" => Self::For,
            "from" => Self::From,
            "global" => Self::Global,
            "if" => Self::If,
            "import" => Self::Import,
            "in" => Self::In,
            "is" => Self::Is,
            "lambda" => Self::Lambda,
            "None" => Self::None,
            "nonlocal" => Self::Nonlocal,
            "not" => Self::Not,
            "or" => Self::Or,
            "pass" => Self::Pass,
            "raise" => Self::Raise,
            "return" => Self::Return,
            "True" => Self::True,
            "try" => Self::Try,
            "while" => Self::While,
            "with" => Self::With,
            "yield" => Self::Yield,
            _ => return Option::None,
        };
        Some(kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub range: TextRange,
}

impl Token {
    #[inline]
    pub const fn new(kind: TokenKind, range: TextRange) -> Self {
        Self { kind, range }
    }
}
