use serde::Serialize;

use crate::{
    error::{Error, ErrorKind},
    origin::{FileId, Origin},
};

const STABILITY_POSSIBLE_VALUES: &str =
    "Internal, Private, Obsolete, External, Unstable, Evolving, Stable, Standard";

const CLASS_POSSIBLE_VALUES: &str = "Cpu, Platform, Group, Isa, Common";

const DEPENDS_ON_POSSIBLE_VALUES: &str = "provider, module, library";

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub(crate) enum LexerState {
    // S2.
    ProgramOuterScope,
    InsideControlDirective(u32 /* line */),
    // S0.
    InsideClauseAndExpr,
}

#[derive(Debug, Serialize)]
pub struct Attribute {
    pub name: Option<Stability>,
    pub data: Option<Stability>,
    pub class: Option<Class>,
}

#[derive(Debug, Serialize)]
pub enum ControlDirectiveKind {
    Line(usize, Option<String>, Option<usize>),
    PragmaError(String),
    PragmaBinding(Version, String),
    PragmaDependsOn(PragmaDependsOnKind, String),
    PragmaAttributes { attribute: Attribute, name: String },
    Ignored,
    PragmaOption(String, Option<String>),
}

#[derive(Debug, Serialize)]
pub enum Stability {
    Internal,
    Private,
    Obsolete,
    External,
    Unstable,
    Evolving,
    Stable,
    Standard,
}

#[derive(Debug, Serialize)]
pub enum Class {
    Cpu,
    Platform,
    Group,
    Isa,
    Common,
}

#[derive(Debug, Serialize)]
pub enum PragmaDependsOnKind {
    Provider,
    Module,
    Library,
}

#[derive(Debug, Serialize)]
pub struct ControlDirective {
    pub origin: Origin,
    pub kind: ControlDirectiveKind,
}

#[derive(Debug, Serialize)]
pub struct Version {
    pub major: u8,
    pub minor: u16,
    pub patch: Option<u16>,
}

#[derive(Debug, Serialize)]
pub enum CommentKind {
    SingleLine,
    MultiLine,
}

#[derive(Debug, Serialize)]
pub struct Comment {
    pub origin: Origin,
    pub kind: CommentKind,
}

#[derive(Debug)]
pub struct Lexer<'a> {
    pub(crate) origin: Origin,
    pub(crate) error_mode: bool,
    pub errors: Vec<Error>,
    pub(crate) state: LexerState,
    pub(crate) input: &'a str,
    pub control_directives: Vec<ControlDirective>,
    pub comments: Vec<Comment>,
    pub(crate) chars: Vec<char>,
    pub(crate) chars_idx: usize,
}

#[derive(PartialEq, Eq, Debug, Serialize, Copy, Clone)]
pub enum TokenKind {
    LiteralNumber,
    LiteralString,
    LiteralCharacter,
    Identifier,
    ProbeSpecifier,
    Dot,
    DotDotDot,
    Caret,
    CaretCaret,
    Ampersand,
    AmpersandAmpersand,
    Pipe,
    PipePipe,
    Plus,
    PlusPlus,
    Minus,
    MinusMinus,
    Arrow,
    Star,
    Slash,
    Percent,
    Tilde,
    LeftParen,
    RightParen,
    LeftSquareBracket,
    RightSquareBracket,
    LeftCurly,
    RightCurly,
    Eq,
    EqEq,
    Bang,
    BangEq,
    Comma,
    SemiColon,
    Colon,
    ColonEq,
    Gt,
    Lt,
    Eof,
    KeywordAuto,
    KeywordBreak,
    KeywordCase,
    KeywordChar,
    KeywordConst,
    KeywordContinue,
    KeywordCounter,
    KeywordDefault,
    KeywordDo,
    KeywordDouble,
    KeywordElse,
    KeywordEnum,
    KeywordExtern,
    KeywordFloat,
    KeywordFor,
    KeywordGoto,
    KeywordIf,
    KeywordImport,
    KeywordInline,
    KeywordInt,
    KeywordLong,
    KeywordOffsetOf,
    KeywordProbe,
    KeywordProvider,
    KeywordRegister,
    KeywordRestrict,
    KeywordReturn,
    KeywordSelf,
    KeywordShort,
    KeywordSigned,
    KeywordSizeof,
    KeywordStatic,
    KeywordString,
    KeywordStringof,
    KeywordStruct,
    KeywordSwitch,
    KeywordThis,
    KeywordTranslator,
    KeywordTypedef,
    KeywordUnion,
    KeywordUnsigned,
    KeywordUserland,
    KeywordVoid,
    KeywordVolatile,
    KeywordWhile,
    KeywordXlate,
    Unknown(Option<char>),
    PipeEq,
    CaretEq,
    AmpersandEq,
    GtGtEq,
    LtLtEq,
    PercentEq,
    SlashEq,
    StarEq,
    MinusEq,
    PlusEq,
    LtLt,
    Question,
    GtEq,
    LtEq,
    GtGt,
    PredicateDelimiter,
    Aggregation,
}

impl TryFrom<&str> for Stability {
    type Error = ErrorKind;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "Internal" => Ok(Stability::Internal),
            "Private" => Ok(Stability::Private),
            "Obsolete" => Ok(Stability::Obsolete),
            "External" => Ok(Stability::External),
            "Unstable" => Ok(Stability::Unstable),
            "Evolving" => Ok(Stability::Evolving),
            "Stable" => Ok(Stability::Stable),
            "Standard" => Ok(Stability::Standard),
            _ => Err(ErrorKind::InvalidStability),
        }
    }
}

impl TryFrom<&str> for Class {
    type Error = ErrorKind;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "Cpu" => Ok(Class::Cpu),
            "Platform" => Ok(Class::Platform),
            "Group" => Ok(Class::Group),
            "Isa" => Ok(Class::Isa),
            "Common" => Ok(Class::Common),
            _ => Err(ErrorKind::InvalidClass),
        }
    }
}

pub(crate) fn str_from_source<'a>(src: &'a str, origin: &Origin) -> &'a str {
    &src[origin.offset as usize..origin.offset as usize + origin.len as usize]
}

pub(crate) fn quoted_string_from_source<'a>(src: &'a str, origin: &Origin) -> (&'a str, Origin) {
    let s = &src[origin.offset as usize..origin.offset as usize + origin.len as usize];
    (
        &s[1..s.len() - 1],
        Origin {
            column: origin.column + 1,
            offset: origin.offset + 1,
            len: origin.len - 1,
            ..*origin
        },
    )
}

#[derive(Serialize, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub origin: Origin,
}

impl Default for Token {
    fn default() -> Self {
        Self {
            kind: TokenKind::Unknown(None),
            origin: Origin::new_unknown(),
        }
    }
}

fn is_character_probe_specifier_start(c: char) -> bool {
    matches!(c ,
   '-' | '<' | '>' | '+' | '$' | ':' | 'a'..='z'  |  'A'..='Z' | '_' | '.' | '?' | '*' | '\\' | '[' | ']' | '!')
}

fn is_character_probe_specifier_rest(c: char) -> bool {
    matches!(c ,
   '-' | '<' | '>' | '+' | '$' | ':' | '0'..='9' | 'a'..='z'  |  'A'..='Z' | '_' | '.' | '?' | '*' | '\\' | '[' | ']' | '!' | '(' | ')' )
}

impl<'a> Lexer<'a> {
    pub fn new(file_id: FileId, input: &'a str) -> Self {
        Self {
            origin: Origin::new(1, 1, 0, 0, file_id),
            error_mode: false,
            errors: Vec::new(),
            state: LexerState::ProgramOuterScope,
            control_directives: Vec::new(),
            comments: Vec::new(),
            input,
            chars: input.chars().collect(),
            chars_idx: 0,
        }
    }

    fn add_error(&mut self, kind: ErrorKind) {
        self.errors
            .push(Error::new(kind, self.origin, String::new()));
        self.error_mode = true;
    }

    fn is_identifier_character_trailing(&self, c: char) -> bool {
        match self.state {
            LexerState::ProgramOuterScope | LexerState::InsideClauseAndExpr => {
                c.is_alphanumeric() || c == '_'
            }
            LexerState::InsideControlDirective(_) => !(c.is_whitespace() || c == '"'),
        }
    }

    fn is_identifier_character_leading(&self, c: char) -> bool {
        match self.state {
            LexerState::ProgramOuterScope | LexerState::InsideClauseAndExpr => {
                c.is_alphanumeric() || c == '_' || c == '@'
            }
            LexerState::InsideControlDirective(_) => !(c.is_whitespace() || c == '"'),
        }
    }

    fn lex_identifier(&mut self) -> Token {
        let start_origin = self.origin;
        let first = self.advance(1).unwrap();
        assert!(!(first.is_ascii_whitespace() || first == '"'));

        while let Some(c) = self.peek1() {
            if c.is_ascii_whitespace() || c == '"' {
                break;
            }

            self.advance(1);
        }

        let len = self.origin.offset - start_origin.offset;
        let origin = Origin {
            len,
            ..start_origin
        };

        Token {
            kind: TokenKind::Identifier,
            origin,
        }
    }

    fn lex_keyword(&mut self) -> Token {
        let start_origin = self.origin;
        let first = self.advance(1).unwrap();
        assert!(self.is_identifier_character_leading(first));

        while let Some(c) = self.peek1() {
            if !self.is_identifier_character_trailing(c) {
                break;
            }

            self.advance(1);
        }

        let len = self.origin.offset - start_origin.offset;
        let origin = Origin {
            len,
            ..start_origin
        };

        let lit = &self.input[origin.offset as usize..origin.offset as usize + len as usize];
        let kind = match (self.state, lit) {
            (LexerState::ProgramOuterScope, "auto") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordAuto
            }
            (LexerState::InsideClauseAndExpr, "auto") => TokenKind::KeywordAuto,
            (LexerState::InsideClauseAndExpr, "break") => TokenKind::KeywordBreak,
            (LexerState::InsideClauseAndExpr, "case") => TokenKind::KeywordCase,
            (LexerState::InsideClauseAndExpr, "char") => TokenKind::KeywordChar,
            (LexerState::ProgramOuterScope, "char") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordChar
            }
            (LexerState::ProgramOuterScope, "const") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordConst
            }
            (LexerState::InsideClauseAndExpr, "const") => TokenKind::KeywordConst,
            (LexerState::InsideClauseAndExpr, "continue") => TokenKind::KeywordContinue,
            (LexerState::ProgramOuterScope, "counter") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordCounter
            }
            (LexerState::InsideClauseAndExpr, "counter") => TokenKind::KeywordCounter,
            (LexerState::InsideClauseAndExpr, "default") => TokenKind::KeywordDefault,
            (LexerState::InsideClauseAndExpr, "do") => TokenKind::KeywordDo,
            (LexerState::ProgramOuterScope, "double") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordDouble
            }
            (LexerState::InsideClauseAndExpr, "double") => TokenKind::KeywordDouble,
            (LexerState::InsideClauseAndExpr, "else") => TokenKind::KeywordElse,
            (LexerState::ProgramOuterScope, "enum") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordEnum
            }
            (LexerState::InsideClauseAndExpr, "enum") => TokenKind::KeywordEnum,
            (LexerState::ProgramOuterScope, "extern") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordExtern
            }
            (LexerState::InsideClauseAndExpr, "extern") => TokenKind::KeywordExtern,
            (LexerState::ProgramOuterScope, "float") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordFloat
            }
            (LexerState::InsideClauseAndExpr, "float") => TokenKind::KeywordFloat,
            (LexerState::InsideClauseAndExpr, "for") => TokenKind::KeywordFor,
            (LexerState::InsideClauseAndExpr, "goto") => TokenKind::KeywordGoto,
            (LexerState::InsideClauseAndExpr, "if") => TokenKind::KeywordIf,
            (LexerState::ProgramOuterScope, "import") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordImport
            }
            (LexerState::InsideClauseAndExpr, "import") => TokenKind::KeywordImport,
            (LexerState::ProgramOuterScope, "inline") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordInline
            }
            (LexerState::InsideClauseAndExpr, "inline") => TokenKind::KeywordInline,
            (LexerState::ProgramOuterScope, "int") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordInt
            }
            (LexerState::InsideClauseAndExpr, "int") => TokenKind::KeywordInt,
            (LexerState::ProgramOuterScope, "long") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordLong
            }
            (LexerState::InsideClauseAndExpr, "long") => TokenKind::KeywordLong,
            (LexerState::InsideClauseAndExpr, "offsetof") => TokenKind::KeywordOffsetOf,
            (LexerState::InsideClauseAndExpr, "probe") => TokenKind::KeywordProbe,
            (LexerState::ProgramOuterScope, "provider") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordProvider
            }
            (LexerState::InsideClauseAndExpr, "provider") => TokenKind::KeywordProvider,
            (LexerState::ProgramOuterScope, "register") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordRegister
            }
            (LexerState::InsideClauseAndExpr, "register") => TokenKind::KeywordRegister,
            (LexerState::ProgramOuterScope, "restrict") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordRestrict
            }
            (LexerState::InsideClauseAndExpr, "restrict") => TokenKind::KeywordRestrict,
            (LexerState::InsideClauseAndExpr, "return") => TokenKind::KeywordReturn,
            (LexerState::ProgramOuterScope, "self") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordSelf
            }
            (LexerState::InsideClauseAndExpr, "self") => TokenKind::KeywordSelf,
            (LexerState::ProgramOuterScope, "short") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordShort
            }
            (LexerState::InsideClauseAndExpr, "short") => TokenKind::KeywordShort,
            (LexerState::ProgramOuterScope, "signed") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordSigned
            }
            (LexerState::InsideClauseAndExpr, "signed") => TokenKind::KeywordSigned,
            (LexerState::InsideClauseAndExpr, "sizeof") => TokenKind::KeywordSizeof,
            (LexerState::ProgramOuterScope, "static") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordStatic
            }
            (LexerState::InsideClauseAndExpr, "static") => TokenKind::KeywordStatic,
            (LexerState::ProgramOuterScope, "string") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordString
            }
            (LexerState::InsideClauseAndExpr, "string") => TokenKind::KeywordString,
            (LexerState::InsideClauseAndExpr, "stringof") => TokenKind::KeywordStringof,
            (LexerState::ProgramOuterScope, "struct") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordStruct
            }
            (LexerState::InsideClauseAndExpr, "struct") => TokenKind::KeywordStruct,
            (LexerState::InsideClauseAndExpr, "switch") => TokenKind::KeywordSwitch,
            (LexerState::ProgramOuterScope, "this") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordThis
            }
            (LexerState::InsideClauseAndExpr, "this") => TokenKind::KeywordThis,
            (LexerState::ProgramOuterScope, "translator") => {
                self.state = LexerState::InsideClauseAndExpr;
                TokenKind::KeywordTranslator
            }
            (LexerState::InsideClauseAndExpr, "translator") => TokenKind::KeywordTranslator,
            (LexerState::InsideClauseAndExpr, "typedef") => TokenKind::KeywordTypedef,
            (LexerState::InsideClauseAndExpr, "union") => TokenKind::KeywordUnion,
            (LexerState::InsideClauseAndExpr, "unsigned") => TokenKind::KeywordUnsigned,
            (LexerState::InsideClauseAndExpr, "userland") => TokenKind::KeywordUserland,
            (LexerState::InsideClauseAndExpr, "void") => TokenKind::KeywordVoid,
            (LexerState::InsideClauseAndExpr, "volatile") => TokenKind::KeywordVolatile,
            (LexerState::InsideClauseAndExpr, "while") => TokenKind::KeywordWhile,
            (LexerState::InsideClauseAndExpr, "xlate") => TokenKind::KeywordXlate,
            _ => TokenKind::Identifier,
        };

        Token { kind, origin }
    }

    fn lex_probe_specifier(&mut self) -> Token {
        let start_origin = self.origin;
        let first = self.advance(1).unwrap();
        assert!(is_character_probe_specifier_start(first));

        /*
         * S2 has an ambiguity because RGX_PSPEC includes '*'
         * as a glob character and '*' also can be DT_TOK_STAR.
         * Since lex always matches the longest token, this
         * rule can be matched by an input string like "int*",
         * which could begin a global variable declaration such
         * as "int*x;" or could begin a RGX_PSPEC with globbing
         * such as "int* { trace(timestamp); }".  If C_PSPEC is
         * not set, we must resolve the ambiguity in favor of
         * the type and perform lexer pushback if the fragment
         * before '*' or entire fragment matches a type name.
         * If C_PSPEC is set, we always return a PSPEC token.
         * If C_PSPEC is off, the user can avoid ambiguity by
         * including a ':' delimiter in the specifier, which
         * they should be doing anyway to specify the provider.
         */
        while let Some(c) = self.peek1() {
            match c {
                _ if !is_character_probe_specifier_rest(c) => {
                    break;
                }
                _ => {
                    self.advance(1);
                }
            }
        }

        let len = self.origin.offset - start_origin.offset;
        let origin = Origin {
            len,
            ..start_origin
        };

        Token {
            kind: TokenKind::ProbeSpecifier,
            origin,
        }
    }

    fn lex_literal_string(&mut self) -> Token {
        let start_origin = self.origin;
        let first = self.advance(1);
        assert_eq!(first, Some('"'));

        loop {
            match self.peek1() {
                Some('"') => {
                    self.advance(1);
                    break;
                }
                Some('\n') | None => {
                    self.add_error(ErrorKind::InvalidLiteralString);
                    break;
                }
                Some(_) => {
                    self.advance(1);
                }
            }
        }

        let len = self.origin.offset - start_origin.offset;
        let origin = Origin {
            len,
            ..start_origin
        };

        Token {
            kind: TokenKind::LiteralString,
            origin,
        }
    }

    fn lex_literal_character(&mut self) -> Token {
        let start_origin = self.origin;
        let first = self.advance(1);
        assert_eq!(first, Some('\''));

        loop {
            match self.peek1() {
                Some('\'') => {
                    self.advance(1);
                    break;
                }
                Some('\n') | None => {
                    self.add_error(ErrorKind::InvalidLiteralCharacter);
                    break;
                }
                Some(_) => {
                    self.advance(1);
                }
            }
        }

        let len = self.origin.offset - start_origin.offset;
        // TODO: Limit length.
        let origin = Origin {
            len,
            ..start_origin
        };

        Token {
            kind: TokenKind::LiteralCharacter,
            origin,
        }
    }

    fn lex_literal_number(&mut self) -> Token {
        let start_origin = self.origin;
        let first = self.advance(1).unwrap();
        assert!(first.is_ascii_digit());

        if let Some(second) = self.peek1()
            && first == '0'
            && (second == 'x' || second == 'X')
        {
            self.advance(1);
            let mut count = 0;
            while let Some(c) = self.peek1() {
                match c {
                    '0'..'9' | 'a'..'f' | 'A'..'Z' => {
                        self.advance(1);
                        count += 1;
                    }
                    _ => {
                        break;
                    }
                }
            }
            if count == 0 {
                self.add_error(ErrorKind::InvalidLiteralNumber);
            }
        } else {
            while let Some(c) = self.peek1() {
                match c {
                    '0'..'9' => {
                        self.advance(1);
                    }
                    '.' => {
                        self.add_error(ErrorKind::UnsupportedLiteralFloatNumber);
                        self.advance(1);
                        break;
                    }
                    _ => {
                        break;
                    }
                }
            }
            let len = self.origin.offset - start_origin.offset;
            if first == '0' && len > 1 {
                self.add_error(ErrorKind::InvalidLiteralNumber);
            }
        }

        if let Some('u' | 'U') = self.peek1() {
            self.advance(1);
        }
        if let Some('l' | 'L') = self.peek1() {
            self.advance(1);
        }
        if let Some('l' | 'L') = self.peek1() {
            self.advance(1);
        }

        let len = self.origin.offset - start_origin.offset;
        let origin = Origin {
            len,
            ..start_origin
        };

        Token {
            kind: TokenKind::LiteralNumber,
            origin,
        }
    }

    pub(crate) fn advance(&mut self, count: usize) -> Option<char> {
        let mut last = None;
        for _ in 0..count {
            last = self.peek1();
            match last {
                None => {
                    break;
                }
                Some('\n') => {
                    self.origin.offset += 1;
                    self.origin.column = 1;
                    self.origin.line += 1;
                    self.chars_idx += 1;
                }
                Some(c) => {
                    self.origin.offset += c.len_utf8() as u32;
                    self.origin.column += 1;
                    self.chars_idx += 1;
                }
            }
        }
        last
    }

    fn peek1(&self) -> Option<char> {
        self.chars.get(self.chars_idx).copied()
    }

    fn peek2(&self) -> Option<char> {
        self.chars.get(self.chars_idx + 1).copied()
    }

    fn peek3(&self) -> (Option<char>, Option<char>) {
        (
            self.chars.get(self.chars_idx + 1).copied(),
            self.chars.get(self.chars_idx + 2).copied(),
        )
    }

    pub fn lex(&mut self) -> Token {
        if self.error_mode {
            while let Some(c) = self.peek1()
                && c != '\n'
            {
                self.advance(1);
            }
            self.error_mode = false;
        }

        if self.peek1().is_none() {
            let origin = Origin {
                len: 0,
                ..self.origin
            };
            return Token {
                kind: TokenKind::Eof,
                origin,
            };
        }
        let c = self.peek1().unwrap();

        match (&self.state, c) {
            (_, '\n') => {
                self.advance(1);
                self.lex()
            }
            (_, '#') if self.peek2() == Some('!') => todo!(),
            (LexerState::ProgramOuterScope, '#') => {
                self.state = LexerState::InsideControlDirective(self.origin.line);
                self.advance(1);
                let mut tokens = Vec::with_capacity(8);
                loop {
                    if let Some('\n') = self.peek1() {
                        self.state = LexerState::ProgramOuterScope;
                        break;
                    }
                    let token = self.lex();
                    tokens.push(token);
                }
                match self.control_directive(&tokens) {
                    Ok(directive) => self.control_directives.push(directive),
                    Err(err) => self.errors.push(err),
                }
                self.lex()
            }
            (_, '-') if self.peek2() == Some('-') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::MinusMinus,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '-') if self.peek2() == Some('=') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::MinusEq,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '-') if self.peek2() == Some('>') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Arrow,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '-') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Minus,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '+') if self.peek2() == Some('+') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::PlusPlus,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '+') if self.peek2() == Some('=') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::PlusEq,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '+') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Plus,
                    origin,
                };
                self.advance(1);
                token
            }
            (LexerState::ProgramOuterScope, '.') if self.peek3() == (Some('.'), Some('.')) => {
                let origin = Origin {
                    len: 3,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::DotDotDot,
                    origin,
                };
                self.advance(3);
                token
            }
            (_, '.') if self.peek2().map(|c| c.is_ascii_digit()).unwrap_or_default() => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                self.add_error(ErrorKind::UnsupportedLiteralFloatNumber);
                self.advance(2);
                Token {
                    kind: TokenKind::LiteralNumber,
                    origin,
                }
            }

            (LexerState::ProgramOuterScope, '.') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Dot,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '.') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Dot,
                    origin,
                };
                self.advance(1);
                self.add_error(ErrorKind::UnexpectedPeriod);
                token
            }
            (_, '*') if self.peek2() == Some('=') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::StarEq,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '*') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Star,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '>') if self.peek3() == (Some('>'), Some('=')) => {
                let origin = Origin {
                    len: 3,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::GtGtEq,
                    origin,
                };
                self.advance(3);
                token
            }
            (_, '>') if self.peek2() == Some('>') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::GtGt,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '>') if self.peek2() == Some('=') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::GtEq,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '>') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Gt,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '<') if self.peek3() == (Some('<'), Some('=')) => {
                let origin = Origin {
                    len: 3,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::LtLtEq,
                    origin,
                };
                self.advance(3);
                token
            }
            (_, '<') if self.peek2() == Some('<') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::LtLt,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '<') if self.peek2() == Some('=') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::LtEq,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '<') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Lt,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '^') if self.peek2() == Some('=') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::CaretEq,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '^') if self.peek2() == Some('^') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::CaretCaret,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '^') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Caret,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '&') if self.peek2() == Some('=') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::AmpersandEq,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '&') if self.peek2() == Some('&') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::AmpersandAmpersand,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '&') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Ampersand,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '?') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Question,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '|') if self.peek2() == Some('=') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::PipeEq,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '|') if self.peek2() == Some('|') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::PipePipe,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '|') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Pipe,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, ':') => {
                if let Some(next) = self.peek1()
                    && next == '='
                {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    let token = Token {
                        kind: TokenKind::ColonEq,
                        origin,
                    };
                    self.advance(2);
                    token
                } else {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    let token = Token {
                        kind: TokenKind::Colon,
                        origin,
                    };
                    self.advance(1);
                    token
                }
            }
            (_, '!') if self.peek2() == Some('=') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::BangEq,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '!') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Bang,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '=') => {
                let origin = self.origin;
                self.advance(1);
                if let Some(next) = self.peek1()
                    && next == '='
                {
                    let token = Token {
                        kind: TokenKind::EqEq,
                        origin: Origin { len: 2, ..origin },
                    };
                    self.advance(2);
                    token
                } else {
                    Token {
                        kind: TokenKind::Eq,
                        origin: Origin { len: 1, ..origin },
                    }
                }
            }
            (_, '/') if self.peek2() == Some('/') => {
                self.single_line_comment();
                self.lex()
            }
            (_, '/') if self.peek2() == Some('*') => {
                self.multi_line_comment();
                self.lex()
            }
            (LexerState::ProgramOuterScope, '/') => {
                let origin = self.origin;
                self.advance(1);
                /*
                 * The use of "/" as the predicate delimiter and as the
                 * integer division symbol requires special lookahead
                 * to avoid a shift/reduce conflict in the D grammar.
                 * We look ahead to the next non-whitespace character.
                 * If we encounter EOF, ";", "{", or "/", then this "/"
                 * closes the predicate and we return DT_TOK_EPRED.
                 * If we encounter anything else, it's DT_TOK_DIV.
                 */
                while let Some(c) = self.peek1() {
                    if c.is_ascii_whitespace() {
                        self.advance(1);
                    } else {
                        break;
                    }
                }

                let kind = match self.peek1() {
                    None | Some(';' | '{' | '/') => TokenKind::PredicateDelimiter,
                    _ => TokenKind::Slash,
                };

                let len = self.origin.offset - origin.offset;

                Token {
                    kind,
                    origin: Origin { len, ..self.origin },
                }
            }
            (LexerState::InsideClauseAndExpr, '/') => {
                let token = Token {
                    kind: TokenKind::Slash,
                    origin: self.origin.with_len(1),
                };
                self.advance(1);
                token
            }
            (_, '%') if self.peek2() == Some('=') => {
                let origin = Origin {
                    len: 2,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::PercentEq,
                    origin,
                };
                self.advance(2);
                token
            }
            (_, '%') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Percent,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '~') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Tilde,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '{') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::LeftCurly,
                    origin,
                };
                self.advance(1);

                if self.state == LexerState::ProgramOuterScope {
                    self.state = LexerState::InsideClauseAndExpr;
                }
                token
            }
            (_, '}') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::RightCurly,
                    origin,
                };
                self.advance(1);

                if self.state == LexerState::InsideClauseAndExpr {
                    self.state = LexerState::ProgramOuterScope;
                }
                token
            }
            (_, '(') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::LeftParen,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, ')') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::RightParen,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, ',') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::Comma,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '[') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::LeftSquareBracket,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, ']') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::RightSquareBracket,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, ';') => {
                let origin = Origin {
                    len: 1,
                    ..self.origin
                };
                let token = Token {
                    kind: TokenKind::SemiColon,
                    origin,
                };
                self.advance(1);
                token
            }
            (_, '"') => self.lex_literal_string(),
            (_, '\'') => self.lex_literal_character(),
            (LexerState::ProgramOuterScope, '$') => todo!(),
            _ if c.is_ascii_digit() => self.lex_literal_number(),
            _ if c.is_whitespace() => {
                self.advance(1);
                self.lex()
            }
            (LexerState::ProgramOuterScope, _) if self.is_identifier_character_leading(c) => {
                self.lex_keyword()
            }
            (LexerState::ProgramOuterScope, '@') => self.lex_aggregation(),
            (LexerState::InsideClauseAndExpr, _) if is_character_probe_specifier_start(c) => {
                // TODO: Handle ambiguity of '*'.
                self.lex_probe_specifier()
            }
            (LexerState::InsideControlDirective(_), _)
                if !(c.is_ascii_whitespace() || c == '"') =>
            {
                self.lex_identifier()
            }
            _ => {
                let token = Token {
                    kind: TokenKind::Unknown(Some(c)),
                    origin: Origin {
                        len: 1,
                        ..self.origin
                    },
                };

                self.add_error(ErrorKind::UnknownToken);
                self.advance(1);
                token
            }
        }
    }

    #[warn(unused_results)]
    fn control_directive(&mut self, tokens: &[Token]) -> Result<ControlDirective, Error> {
        match tokens.first() {
            None => {
                // According to K&R[A12.9], we silently ignore null directive lines.
                Ok(ControlDirective {
                    kind: ControlDirectiveKind::Ignored,
                    origin: Origin::new_unknown(),
                })
            }
            Some(Token {
                kind: TokenKind::LiteralNumber,
                origin,
            }) => self.control_directive_line(tokens, *origin),
            Some(Token {
                kind: TokenKind::Identifier,
                origin,
            }) => {
                let src = str_from_source(self.input, origin);
                match src {
                    "line" => self.control_directive_line(&tokens[1..], *origin),
                    "pragma" if tokens.len() > 1 => {
                        self.control_directive_pragma(&tokens[1..], *origin)
                    }
                    // Ignore any #ident or #pragma ident lines.
                    "pragma" if tokens.len() == 1 => Ok(ControlDirective {
                        kind: ControlDirectiveKind::Ignored,
                        origin: origin.extend_to(tokens.last().map(|t| t.origin)),
                    }),

                    "ident" => Ok(ControlDirective {
                        kind: ControlDirectiveKind::Ignored,
                        origin: origin.extend_to(tokens.last().map(|t| t.origin)),
                    }),
                    "error" => self.control_directive_error(&tokens[1..]),
                    _ => Err(Error::new(
                        ErrorKind::InvalidControlDirective,
                        origin.extend_to(tokens.last().map(|t| t.origin)),
                        String::new(),
                    )),
                }
            }
            Some(other) => Err(Error::new(
                ErrorKind::InvalidControlDirective,
                other.origin.extend_to(tokens.last().map(|t| t.origin)),
                String::new(),
            )),
        }
    }

    #[warn(unused_results)]
    fn control_directive_line(
        &mut self,
        tokens: &[Token],
        origin: Origin,
    ) -> Result<ControlDirective, Error> {
        let (line, file, trailing) = match tokens {
            // `5`
            [
                line @ Token {
                    kind: TokenKind::LiteralNumber,
                    ..
                },
            ] => (line, None, None),
            // `5 "foo.d"`
            [
                line @ Token {
                    kind: TokenKind::LiteralNumber,
                    ..
                },
                file @ Token {
                    kind: TokenKind::LiteralString,
                    ..
                },
            ] => (line, Some(file), None),
            // `5 "foo.d" 0`
            [
                line @ Token {
                    kind: TokenKind::LiteralNumber,
                    ..
                },
                file @ Token {
                    kind: TokenKind::LiteralString,
                    ..
                },
                trailing @ Token {
                    kind: TokenKind::LiteralNumber,
                    ..
                },
            ] => (line, Some(file), Some(trailing)),
            _other => {
                return Err(Error::new(
                    ErrorKind::InvalidControlDirective,
                    origin,
                    String::new(),
                ));
            }
        };

        let line_src = str_from_source(self.input, &line.origin);
        let file_src = file.map(|f| {
            let s = str_from_source(self.input, &f.origin);
            // Without the double quotes.
            s[1..s.len() - 1].to_owned()
        });
        let line_num: usize = match str::parse::<usize>(line_src) {
            Err(err) => {
                return Err(Error::new(
                    ErrorKind::InvalidLiteralNumber,
                    line.origin,
                    err.to_string(),
                ));
            }
            Ok(n) => n,
        };

        let trailing_num = if let Some(trailing) = trailing {
            match str::parse::<usize>(str_from_source(self.input, &trailing.origin)) {
                Ok(num) => Some(num),
                Err(err) => {
                    return Err(Error::new(
                        ErrorKind::InvalidLiteralNumber,
                        trailing.origin,
                        err.to_string(),
                    ));
                }
            }
        } else {
            None
        };

        Ok(ControlDirective {
            origin: origin.extend_to(tokens.last().map(|t| t.origin)),
            kind: ControlDirectiveKind::Line(line_num, file_src, trailing_num),
        })
    }

    #[warn(unused_results)]
    fn control_directive_pragma(
        &mut self,
        tokens: &[Token],
        origin: Origin,
    ) -> Result<ControlDirective, Error> {
        let (directive1, directive2) = match (tokens.first(), tokens.get(1)) {
            (
                Some(Token {
                    kind: TokenKind::Identifier,
                    origin: origin1,
                }),
                Some(Token {
                    kind: TokenKind::Identifier,
                    origin: origin2,
                }),
            ) => (
                Some(str_from_source(self.input, origin1)),
                Some(str_from_source(self.input, origin2)),
            ),
            (
                Some(Token {
                    kind: TokenKind::Identifier,
                    origin: origin1,
                }),
                _,
            ) => (Some(str_from_source(self.input, origin1)), None),
            _ => (None, None),
        };

        match (directive1, directive2) {
            // `#pragma error`, or  `#pragma D error`.
            (Some("D"), Some("error")) => self.control_directive_error(&tokens[2..]),
            (Some("error"), _) => self.control_directive_error(&tokens[1..]),

            // `#pragma line`.
            (Some("D"), Some("line")) => self.control_directive_line(&tokens[2..], origin),
            (Some("line"), _) => self.control_directive_line(&tokens[1..], origin),
            //
            // `#pragma depends_on`.
            (Some("D"), Some("depends_on")) => self.pragma_depends_on(&tokens[2..], origin),

            (Some("depends_on"), _) => self.pragma_depends_on(&tokens[1..], origin),

            // `#pragma attributes`.
            (Some("D"), Some("attributes")) => self.pragma_attributes(&tokens[2..], origin),
            (Some("attributes"), _) => self.pragma_attributes(&tokens[1..], origin),

            // `#pragma binding`.
            (Some("D"), Some("binding")) => self.pragma_binding(&tokens[2..], origin),
            (Some("binding"), _) => self.pragma_binding(&tokens[1..], origin),

            // `#pragma option`.
            (Some("D"), Some("option")) => self.pragma_option(&tokens[2..], origin),
            (Some("option"), _) => self.pragma_option(&tokens[1..], origin),

            // `#pragma`, `#pragma ident`,  `#pragma D ident`, or `#pragma someunknownstuff`: Ignore.
            _ => Ok(ControlDirective {
                kind: ControlDirectiveKind::Ignored,
                origin: origin.extend_to(tokens.last().map(|last| last.origin)),
            }),
        }
    }

    #[warn(unused_results)]
    fn control_directive_error(&mut self, tokens: &[Token]) -> Result<ControlDirective, Error> {
        let src = match (tokens.get(1), tokens.last()) {
            (Some(start), Some(end)) => self.input[start.origin.offset as usize
                ..end.origin.offset as usize + end.origin.len as usize]
                .to_owned(),
            _ => String::new(),
        };

        Ok(ControlDirective {
            origin: tokens[0].origin.extend_to(tokens.last().map(|t| t.origin)),
            kind: ControlDirectiveKind::PragmaError(src),
        })
    }

    #[warn(unused_results)]
    fn pragma_attributes(
        &self,
        tokens: &[Token],
        origin: Origin,
    ) -> Result<ControlDirective, Error> {
        let (s1, s2) = match tokens {
            [
                Token {
                    kind: TokenKind::Identifier,
                    origin: origin1,
                },
                Token {
                    kind: TokenKind::Identifier,
                    origin: origin2,
                },
            ] => {
                let s1 = str_from_source(self.input, origin1);
                let s2 = str_from_source(self.input, origin2);
                (s1, s2)
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidControlDirective,
                    origin.extend_to(tokens.last().map(|t| t.origin)),
                    String::from("expected pragma attributes of the form: identifier identifier"),
                ));
            }
        };

        let origin_identifier_first = tokens[0].origin;
        let split: Vec<_> = s1.splitn(4, "/").collect();
        let (name_str, data_str, class_str, trailing) =
            (split.first(), split.get(1), split.get(2), split.get(3));
        if let Some(trailing) = trailing {
            return Err(Error {
                kind: ErrorKind::InvalidControlDirective,
                origin: origin_identifier_first
                    .skip(s1.len() - trailing.len())
                    .with_len(trailing.len()),
                explanation: String::from(
                    "expected up to 3 parts in attribute but found an extraneous part",
                ),
            });
        }
        let name = name_str
            .map(|s| {
                Stability::try_from(*s).map_err(|kind| Error {
                    kind,
                    origin: origin_identifier_first.with_len(s.len()),
                    explanation: format!(
                        "invalid stability, possible values are: {}",
                        STABILITY_POSSIBLE_VALUES,
                    ),
                })
            })
            .transpose()?;

        let skip = name_str.map(|s| s.len() + 1).unwrap_or_default();
        let data = data_str
            .map(|s| {
                Stability::try_from(*s).map_err(|kind| Error {
                    kind,
                    origin: origin_identifier_first.skip(skip).with_len(s.len()),
                    explanation: format!(
                        "invalid stability, possible values are: {}",
                        STABILITY_POSSIBLE_VALUES,
                    ),
                })
            })
            .transpose()?;

        let skip = name_str.map(|s| s.len() + 1).unwrap_or_default()
            + data_str.map(|s| s.len() + 1).unwrap_or_default();
        let class: Option<Class> = class_str
            .map(|s| {
                Class::try_from(*s).map_err(|kind| Error {
                    kind,
                    origin: origin_identifier_first.skip(skip).with_len(s.len()),
                    explanation: format!(
                        "invalid class, possible values are: {}",
                        CLASS_POSSIBLE_VALUES
                    ),
                })
            })
            .transpose()?;
        let attribute = Attribute { name, data, class };

        Ok(ControlDirective {
            kind: ControlDirectiveKind::PragmaAttributes {
                attribute,
                // TODO: `s2` may be `provider`, etc and should be handled differently in these
                // cases.
                name: s2.to_owned(),
            },
            origin: origin.extend_to(tokens.last().map(|t| t.origin)),
        })
    }

    #[warn(unused_results)]
    fn pragma_binding(
        &mut self,
        tokens: &[Token],
        origin: Origin,
    ) -> Result<ControlDirective, Error> {
        match tokens {
            [
                Token {
                    kind: TokenKind::LiteralString,
                    origin: origin1,
                },
                Token {
                    kind: TokenKind::Identifier,
                    origin: origin2,
                },
            ] => {
                let (version_str, version_origin) = quoted_string_from_source(self.input, origin1);
                let version = version_str2num(version_str, version_origin)?;
                let identifier = str_from_source(self.input, origin2).to_owned();

                Ok(ControlDirective {
                    origin: origin.extend_to(tokens.last().map(|t| t.origin)),
                    kind: ControlDirectiveKind::PragmaBinding(version, identifier),
                })
            }
            _ => Err(Error::new(
                ErrorKind::InvalidControlDirective,
                origin.extend_to(tokens.last().map(|t| t.origin)),
                String::from("expected pragma binding of the form: \"version\" identifier"),
            )),
        }
    }

    #[warn(unused_results)]
    fn pragma_option(&self, tokens: &[Token], origin: Origin) -> Result<ControlDirective, Error> {
        match tokens {
            [
                Token {
                    kind: TokenKind::Identifier,
                    origin: origin1,
                },
            ] => {
                // TODO: Validate option key against a list of known values?

                let s = str_from_source(self.input, origin1);
                if let Some((key, value)) = s.split_once('=') {
                    if value.contains('=') {
                        Err(Error {
                            kind: ErrorKind::InvalidControlDirective,
                            origin: *origin1,
                            explanation: String::from(
                                "expected option of the form key=value, found additional equal sign",
                            ),
                        })
                    } else {
                        Ok(ControlDirective {
                            kind: ControlDirectiveKind::PragmaOption(
                                key.to_owned(),
                                Some(value.to_owned()),
                            ),
                            origin: origin.extend_to(tokens.last().map(|t| t.origin)),
                        })
                    }
                } else {
                    Ok(ControlDirective {
                        kind: ControlDirectiveKind::PragmaOption(s.to_owned(), None),
                        origin: origin.extend_to(tokens.last().map(|t| t.origin)),
                    })
                }
            }
            other => Err(Error {
                kind: ErrorKind::InvalidControlDirective,
                origin: origin.extend_to(other.last().map(|t| t.origin)),
                explanation: String::from("expected pragma option of the form key=value"),
            }),
        }
    }

    #[warn(unused_results)]
    fn pragma_depends_on(
        &mut self,
        tokens: &[Token],
        origin: Origin,
    ) -> Result<ControlDirective, Error> {
        let (kind_str, name) = match tokens {
            [
                Token {
                    kind: TokenKind::Identifier,
                    origin: origin1,
                },
                Token {
                    kind: TokenKind::Identifier,
                    origin: origin2,
                },
            ] => {
                let kind = str_from_source(self.input, origin1);
                let name = str_from_source(self.input, origin2);
                (kind, name)
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidControlDirective,
                    origin.extend_to(tokens.last().map(|t| t.origin)),
                    String::from("expected pragma depends_on of the form: identifier identifier"),
                ));
            }
        };

        let kind = match kind_str {
            "provider" => PragmaDependsOnKind::Provider,
            "module" => PragmaDependsOnKind::Module,
            "library" => PragmaDependsOnKind::Library,
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidControlDirective,
                    tokens[0].origin,
                    format!(
                        "invalid depends_on class, possible values: {}",
                        DEPENDS_ON_POSSIBLE_VALUES
                    ),
                ));
            }
        };

        Ok(ControlDirective {
            kind: ControlDirectiveKind::PragmaDependsOn(kind, name.to_owned()),
            origin: origin.extend_to(tokens.last().map(|t| t.origin)),
        })
    }

    fn single_line_comment(&mut self) {
        let origin = self.origin;

        let first = self.peek1().unwrap();
        assert_eq!(first, '/');
        self.advance(1);

        let second = self.peek1().unwrap();
        assert_eq!(second, '/');
        self.advance(1);

        while let Some(c) = self.peek1() {
            match c {
                '\n' => {
                    break;
                }
                '/' if self.peek2() == Some('/') || self.peek2() == Some('*') => {
                    self.errors.push(Error {
                        kind: ErrorKind::NestedComment,
                        origin: self.origin.with_len(2),
                        explanation: String::from("nested comment"),
                    });
                    self.advance(1);
                }
                '*' if self.peek2() == Some('/') => {
                    self.errors.push(Error {
                        kind: ErrorKind::NestedComment,
                        origin: self.origin.with_len(2),
                        explanation: String::from("nested comment"),
                    });
                    self.advance(1);
                }
                _ => {
                    self.advance(1);
                }
            }
        }

        let origin = Origin {
            len: self.origin.offset - origin.offset,
            ..origin
        };

        self.comments.push(Comment {
            kind: CommentKind::SingleLine,
            origin,
        });
    }

    fn multi_line_comment(&mut self) {
        let origin = self.origin;

        let first = self.peek1().unwrap();
        assert_eq!(first, '/');
        self.advance(1);

        let second = self.peek1().unwrap();
        assert_eq!(second, '*');
        self.advance(1);

        while let Some(c) = self.peek1() {
            match c {
                '/' if self.peek2() == Some('*') => {
                    self.errors.push(Error {
                        kind: ErrorKind::NestedComment,
                        origin: self.origin.with_len(2),
                        explanation: String::from("nested comment"),
                    });
                    self.advance(1);
                }
                '*' if self.peek2() == Some('/') => {
                    self.advance(1);
                    self.advance(1);
                    break;
                }
                _ => {
                    self.advance(1);
                }
            }
        }

        let origin = Origin {
            len: self.origin.offset - origin.offset,
            ..origin
        };

        self.comments.push(Comment {
            kind: CommentKind::MultiLine,
            // FIXME: Known issue: this is the only token spanning multiple lines and `origin` only
            // supports single-line tokens. So the `line` field means `line_start`, *not*
            // `line_end` which might be bigger.
            origin,
        });
    }

    fn lex_aggregation(&mut self) -> Token {
        let start_origin = self.origin;
        let first = self.advance(1);
        assert_eq!(first, Some('@'));

        let second = self.peek1();
        match second {
            Some('a'..'z' | 'A'..'Z' | '_') => {
                self.advance(1);
            }
            _ => {
                return Token {
                    kind: TokenKind::Aggregation,
                    origin: self.origin.with_len(1),
                };
            }
        }

        while let Some(c) = self.peek1()
            && c.is_ascii_alphanumeric()
        {
            self.advance(1);
        }

        Token {
            kind: TokenKind::Aggregation,
            origin: Origin {
                len: self.origin.offset - start_origin.offset,
                ..start_origin
            },
        }
    }
}

fn version_str2num(version_str: &str, origin: Origin) -> Result<Version, Error> {
    let split: Vec<_> = version_str.splitn(4, ".").collect();
    if split.len() < 2 {
        return Err(Error {
            kind: ErrorKind::InvalidVersionString,
            origin,
            explanation: String::from(
                "expected version string as \"major.minor\" or \"major.minor.patch\"",
            ),
        });
    }
    let (major_str, minor_str, patch_str, trailing) =
        (split[0], split[1], split.get(2), split.get(3));
    if let Some(trailing) = trailing {
        return Err(Error {
            kind: ErrorKind::InvalidControlDirective,
            origin: origin
                .skip(version_str.len() - trailing.len())
                .with_len(trailing.len()),
            explanation: String::from(
                "expected up to 3 parts in version string but found an extraneous part",
            ),
        });
    }

    let major = str::parse::<u8>(major_str).map_err(|err| Error {
        kind: ErrorKind::InvalidVersionString,
        origin: origin.with_len(major_str.len()),
        explanation: format!(
            "invalid major version in version string, expected a number up to 255: {}",
            err
        ),
    })?;

    let origin = origin.skip(major_str.len() + 1);
    let minor = str::parse::<u16>(minor_str).map_err(|err| Error {
        kind: ErrorKind::InvalidVersionString,
        origin: origin.with_len(minor_str.len()),
        explanation: format!(
            "invalid minor version in version string, expected a number: {}",
            err
        ),
    })?;
    if minor > 0xfff {
        return Err(Error {
            kind: ErrorKind::InvalidVersionString,
            origin: origin.with_len(minor_str.len()),
            explanation: String::from(
                "minor version too high in version string, expected a number up to 4095",
            ),
        });
    }

    let origin = origin.skip(minor_str.len() + 1);
    let patch = if let Some(patch_str) = patch_str {
        let num = str::parse::<u16>(patch_str).map_err(|err| Error {
            kind: ErrorKind::InvalidVersionString,
            origin: origin.with_len(patch_str.len()),
            explanation: format!(
                "invalid patch version in version string, expected a number: {}",
                err
            ),
        })?;
        if num > 0xfff {
            return Err(Error {
                kind: ErrorKind::InvalidVersionString,
                origin: origin.with_len(patch_str.len()),
                explanation: String::from(
                    "patch version too high in version string, expected a number up to 4095",
                ),
            });
        }
        Some(num)
    } else {
        None
    };

    Ok(Version {
        major,
        minor,
        patch,
    })
}

#[cfg(test)]
mod tests {}
