use std::ops::Range;

use log::info;
use serde::Serialize;

use crate::{
    error::{Error, ErrorKind},
    origin::{FileId, Origin, Position, PositionKind},
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

#[derive(Debug, Serialize, PartialEq)]
pub struct PragmaAttribute {
    pub name: Option<Stability>,
    pub data: Option<Stability>,
    pub class: Option<Class>,
}

#[derive(Debug, Serialize, PartialEq)]
pub enum ControlDirectiveKind {
    Line(usize, Option<String>, Option<usize>),
    PragmaError(String),
    PragmaBinding(Version, String),
    PragmaDependsOn(PragmaDependsOnKind, String),
    PragmaAttributes {
        attribute: PragmaAttribute,
        name: String,
    },
    Ignored,
    PragmaOption(String, Option<String>),
    Shebang(String),
}

#[derive(Debug, Serialize, PartialEq)]
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

#[derive(Debug, Serialize, PartialEq)]
pub enum Class {
    Cpu,
    Platform,
    Group,
    Isa,
    Common,
}

#[derive(Debug, Serialize, PartialEq)]
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
impl ControlDirective {
    pub fn log(&self, file_id_to_name: &std::collections::HashMap<u32, String>) {
        info!("{}: {:?}", self.origin.display(file_id_to_name), self.kind);
    }
}

#[derive(Debug, Serialize, PartialEq)]
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

#[derive(Debug, Serialize, PartialEq)]
pub struct Attribute {
    pub origin: Origin,
}

#[derive(Debug)]
pub struct Lexer<'a> {
    pub(crate) position: Position,
    pub errors: Vec<Error>,
    pub(crate) state: LexerState,
    pub(crate) input: &'a str,
    pub control_directives: Vec<ControlDirective>,
    pub comments: Vec<Comment>,
    pub(crate) chars: Vec<char>,
    pub(crate) chars_idx: usize,
    pub(crate) attributes: Vec<Attribute>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Clone)]
pub enum NumberOrString {
    Number(usize),
    String(String),
}

#[derive(PartialEq, Eq, Debug, Serialize, Copy, Clone)]
pub enum TokenKind {
    LiteralNumber(u64),
    LiteralString,
    LiteralCharacter(isize),
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
    PercentEq,
    SlashEq,
    StarEq,
    MinusEq,
    PlusEq,
    LtLt,
    LtLtEq,
    Question,
    GtEq,
    LtEq,
    GtGt,
    GtGtEq,
    ClosePredicateDelimiter,
    Aggregation,
    MacroArgumentReference(Option<u32>),
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

pub(crate) fn str_from_source(src: &str, origin: Origin) -> &str {
    &src[Range::from(origin)]
}

pub(crate) fn quoted_string_from_source(src: &str, origin: Origin) -> (&str, Origin) {
    let s = str_from_source(src, origin);
    assert_eq!(s.chars().next().unwrap(), '"');
    assert_eq!(s.chars().nth(s.len() - 1).unwrap(), '"');
    (&s[1..s.len() - 1], origin.forwards(1).backwards(1))
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
            origin: Origin::default(),
        }
    }
}

fn is_character_probe_specifier_start(c: char) -> bool {
    matches!(c ,
   '-' | '<' | '>' | '+' | '$' | ':' | 'a'..='z'  |  'A'..='Z' | '_' | '.' | '?' | '*' | '\\' | '[' | ']' | '!')
}

fn is_character_probe_specifier_rest(c: char) -> bool {
    matches!(c ,
   '-' | '<' | '>' | '+' | '$' | ':' | '0'..='9' | 'a'..='z'  |  'A'..='Z' | '_' | '`' | '.' | '?' | '*' | '\\' | '[' | ']' | '!' | '(' | ')' )
}

impl<'a> Lexer<'a> {
    pub fn new(file_id: FileId, input: &'a str) -> Self {
        Self {
            position: Position {
                kind: PositionKind::File(file_id),
                ..Position::default()
            },
            errors: Vec::new(),
            state: LexerState::ProgramOuterScope,
            control_directives: Vec::new(),
            comments: Vec::new(),
            input,
            chars: input.chars().collect(),
            chars_idx: 0,
            attributes: Vec::new(),
        }
    }

    fn add_error(&mut self, kind: ErrorKind, origin: Origin, explanation: &str) {
        self.errors.push(Error::new(kind, origin, explanation.to_owned()));
    }

    fn is_identifier_character_trailing(&self, c: char) -> bool {
        match self.state {
            LexerState::ProgramOuterScope | LexerState::InsideClauseAndExpr => {
                c.is_alphanumeric() || c == '_' || c == '`'
            }
            LexerState::InsideControlDirective(_) => !(c.is_whitespace() || c == '"'),
        }
    }

    fn is_identifier_character_leading(&self, c: char) -> bool {
        match self.state {
            LexerState::ProgramOuterScope | LexerState::InsideClauseAndExpr => {
                c.is_alphanumeric() || c == '_' || c == '@' || c == '`'
            }
            LexerState::InsideControlDirective(_) => !(c.is_whitespace() || c == '"'),
        }
    }

    fn lex_identifier(&mut self) -> Token {
        let start = self.position;
        let (first, _) = self.advance(1);
        let first = first.unwrap();
        assert!(!(first.is_ascii_whitespace() || first == '"'));

        let mut end = self.position;
        while let Some(c) = self.peek1() {
            if !self.is_identifier_character_trailing(c) {
                break;
            }

            (_, end) = self.advance(1);
        }

        let origin = start.extend_to_inclusive(end);

        Token {
            kind: TokenKind::Identifier,
            origin,
        }
    }

    fn lex_pragma_identifier(&mut self) -> Token {
        let start = self.position;
        let (first, _) = self.advance(1);
        let first = first.unwrap();
        assert!(!(first.is_ascii_whitespace() || first == '"'));

        while let Some(c) = self.peek1() {
            if c.is_ascii_whitespace() || c == '"' {
                break;
            }

            self.advance(1);
        }

        let origin = start.extend_to_inclusive(self.position);

        Token {
            kind: TokenKind::Identifier,
            origin,
        }
    }

    fn lex_convert_to_keyword(&mut self, token: Token) -> Token {
        let s = str_from_source(self.input, token.origin);
        let kind = match (self.state, s) {
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
            (LexerState::InsideClauseAndExpr, "typedef") => TokenKind::KeywordTypedef,
            (LexerState::InsideClauseAndExpr, "union") => TokenKind::KeywordUnion,
            (LexerState::InsideClauseAndExpr, "unsigned") => TokenKind::KeywordUnsigned,
            (LexerState::InsideClauseAndExpr, "userland") => TokenKind::KeywordUserland,
            (LexerState::InsideClauseAndExpr, "void") => TokenKind::KeywordVoid,
            (LexerState::InsideClauseAndExpr, "volatile") => TokenKind::KeywordVolatile,
            (LexerState::InsideClauseAndExpr, "while") => TokenKind::KeywordWhile,
            (LexerState::InsideClauseAndExpr, "xlate") => TokenKind::KeywordXlate,
            _ => token.kind,
        };

        Token {
            kind,
            origin: token.origin,
        }
    }

    fn lex_probe_specifier(&mut self) -> Token {
        let start = self.position;
        let (first, _) = self.advance(1);
        let first = first.unwrap();
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

        let origin = start.extend_to_inclusive(self.position);

        Token {
            kind: TokenKind::ProbeSpecifier,
            origin,
        }
    }

    fn lex_literal_string(&mut self) -> Token {
        let start = self.position;
        let (first, _) = self.advance(1);
        assert_eq!(first, Some('"'));

        loop {
            match self.peek2() {
                // End.
                (Some('"'), _) => {
                    self.advance(1);
                    break;
                }
                // Ordinary characters.
                (Some(c), _) if !(c == '"' || c == '\\' || c == '\n') => {
                    self.advance(1);
                }
                // Escaped.
                (Some('\\'), Some(c)) if c != '\n' => {
                    self.advance(2);
                }
                (Some('\\'), Some('\n')) => {
                    self.add_error(ErrorKind::InvalidLiteralString, self.position.into(), "backslash-newline is not allowed inside a string literal");
                    self.advance(2);
                }
                (Some('\n'), _) => {
                    self.add_error(ErrorKind::InvalidLiteralString, self.position.into(), "newline is not allowed inside a string literal");
                    self.advance(1);
                }
                (Some(_), _) => {
                    self.advance(1);
                }
                (None, _) => {
                    self.add_error(ErrorKind::InvalidLiteralString, self.position.into(), "unterminated string literal");
                    break;
                }
            }
        }

        let origin = start.extend_to_inclusive(self.position);

        Token {
            kind: TokenKind::LiteralString,
            origin,
        }
    }

    fn lex_literal_character(&mut self) -> Token {
        let start = self.position;
        let (first, _) = self.advance(1);
        assert_eq!(first, Some('\''));

        let mut bytes = Vec::with_capacity(std::mem::size_of::<isize>());
        loop {
            match self.peek2() {
                // End.
                (Some('\''), _) => {
                    self.advance(1);
                    break;
                }
                (Some('\\'), Some('a')) => {
                    self.advance(2);
                    bytes.push(7);
                }
                (Some('\\'), Some('b')) => {
                    self.advance(2);
                    bytes.push(8);
                }
                (Some('\\'), Some('f')) => {
                    self.advance(2);
                    bytes.push(12);
                }
                (Some('\\'), Some('n')) => {
                    self.advance(2);
                    bytes.push(10);
                }
                (Some('\\'), Some('r')) => {
                    self.advance(2);
                    bytes.push(13);
                }
                (Some('\\'), Some('t')) => {
                    self.advance(2);
                    bytes.push(9);
                }
                (Some('\\'), Some('v')) => {
                    self.advance(2);
                    bytes.push(11);
                }
                // Octal sequence. DTrace allows up to 3 octal digits (like C).
                // The value is always truncated to 1 byte, matching C `char` semantics.
                (Some('\\'), Some('0'..='7')) => {
                    self.advance(1);
                    let start = self.position.byte_offset as usize;
                    for _ in 0..3 {
                        if matches!(self.peek1(), Some('0'..='7')) {
                            self.advance(1);
                        } else {
                            break;
                        }
                    }

                    let s = &self.input[start..self.position.byte_offset as usize];
                    let v = u32::from_str_radix(s, 8).unwrap();
                    bytes.push(v as u8); // always 1 byte, wraps like C `char`
                }
                // Hex sequence.
                (Some('\\'), Some('x')) => {
                    // The official implementation neither checks for a minimum (1) number of hex characters nor for a maximum (2), nor for overflow.
                    // In case of no characters or overflow, we record an error. But all hex characters are still consumed to match the official behavior.

                    self.advance(2);
                    let start = self.position.byte_offset as usize;

                    while let Some('0'..='9' | 'a'..='z' | 'A'..='Z') = self.peek1() {
                        self.advance(1);
                    }
                    let s = &self.input[start..self.position.byte_offset as usize];

                    if s.is_empty() {
                        self.add_error(ErrorKind::InvalidLiteralCharacter, self.position.into(), "hex escape sequence requires at least one hex digit");
                    } else {
                        // Small, but probably inconsequential difference with the official implementation:
                        // the official implementation casts (truncates) the hex sequence to `char`,
                        // which might be signed or unsigned depending on the platform.
                        // We always use `u8`.
                        let byte = match u8::from_str_radix(s, 16) {
                            Ok(c) => c,
                            Err(_) => {
                                self.add_error(
                                    ErrorKind::InvalidLiteralCharacter,
                                    self.position.into(),
                                    "hex escape sequence value does not fit in a byte (maximum 0xFF)",
                                );
                                0
                            }
                        };
                        bytes.push(byte);
                    }
                }
                // Ordinary characters.
                (Some(c), _) if !(c == '\'' || c == '\\' || c == '\n') => {
                    self.advance(1);
                    bytes.push(c as u8);
                }
                (Some('\\'), Some('\n')) => {
                    self.add_error(ErrorKind::InvalidLiteralCharacter, self.position.into(), "backslash-newline is not allowed inside a character literal");
                    self.advance(2);
                }
                // Known escapes that are not letters: produce 1 byte.
                (Some('\\'), Some('"')) => {
                    self.advance(2);
                    bytes.push(b'"');
                }
                (Some('\\'), Some('\\')) => {
                    self.advance(2);
                    bytes.push(b'\\');
                }
                // Unknown escape: keep both the backslash and the character,
                // matching the `default` case of the official `stresc2chr()`.
                (Some('\\'), Some(c)) => {
                    self.advance(2);
                    bytes.push(b'\\');
                    bytes.push(c as u8);
                }
                (Some('\n'), _) => {
                    self.add_error(ErrorKind::InvalidLiteralCharacter, self.position.into(), "newline is not allowed inside a character literal");
                    self.advance(1);
                }
                (Some(c), _) => {
                    self.advance(1);
                    bytes.push(c as u8);
                }
                (None, _) => {
                    self.add_error(ErrorKind::InvalidLiteralCharacter, self.position.into(), "unterminated character literal");
                    break;
                }
            }
        }

        if bytes.is_empty() {
            self.add_error(ErrorKind::InvalidLiteralCharacter, self.position.into(), "empty character literal");
        }

        let bytes_8: [u8; 8] = if bytes.len() > 8 {
            self.add_error(ErrorKind::InvalidLiteralCharacter, self.position.into(), "character literal is too long (maximum 8 bytes)");
            [0; 8]
        } else {
            let mut bytes_8 = [0u8; 8];
            bytes_8[8 - bytes.len()..].copy_from_slice(bytes.as_slice());
            bytes_8
        };

        let value = isize::from_be_bytes(bytes_8);

        Token {
            kind: TokenKind::LiteralCharacter(value),
            origin: start.extend_to_inclusive(self.position),
        }
    }

    fn lex_literal_number(&mut self) -> Token {
        let start = self.position;
        let (first, _) = self.advance(1);
        let first = first.unwrap();
        assert!(first.is_ascii_digit());

        let value: u64;

        if let Some(second) = self.peek1()
            && first == '0'
            && (second == 'x' || second == 'X')
        {
            // Hex literal: `0x...` or `0X...`.
            self.advance(1);
            let hex_start = self.position.byte_offset as usize;
            let mut count = 0;
            while let Some(c) = self.peek1() {
                match c {
                    '0'..='9' | 'a'..='f' | 'A'..='F' => {
                        self.advance(1);
                        count += 1;
                    }
                    _ => {
                        break;
                    }
                }
            }
            if count == 0 {
                self.add_error(ErrorKind::InvalidLiteralNumber, self.position.into(), "hex literal requires at least one hex digit after '0x'");
                value = 0;
            } else {
                let hex_digits = &self.input[hex_start..self.position.byte_offset as usize];
                value = u64::from_str_radix(hex_digits, 16).unwrap_or(0);
            }
        } else {
            // Decimal or octal literal.
            let digits_start = start.byte_offset as usize;
            let mut is_float = false;
            while let Some(c) = self.peek1() {
                match c {
                    '0'..='9' => {
                        self.advance(1);
                    }
                    '.' => {
                        self.add_error(
                            ErrorKind::UnsupportedLiteralFloatNumber,
                            self.position.into(),
                            "floating-point literals are not supported",
                        );
                        self.advance(1);
                        self.skip_until_end_of_float();
                        is_float = true;
                        break;
                    }
                    _ => {
                        break;
                    }
                }
            }
            // Check for an exponent without a dot (e.g. `1e5`, `1E+3`):
            // these are float literals per `RGX_FP` in the official lexer.
            if !is_float {
                if let Some('e' | 'E') = self.peek1() {
                    self.add_error(
                        ErrorKind::UnsupportedLiteralFloatNumber,
                        self.position.into(),
                        "floating-point literals are not supported",
                    );
                    self.skip_until_end_of_float();
                    is_float = true;
                }
            }
            if is_float {
                value = 0;
            } else {
                let digits = &self.input[digits_start..self.position.byte_offset as usize];
                // A leading `0` with more digits means octal (C convention).
                if digits.len() > 1 && digits.starts_with('0') {
                    value = u64::from_str_radix(&digits[1..], 8).unwrap_or(0);
                } else {
                    value = u64::from_str_radix(digits, 10).unwrap_or(0);
                }
            }
        }

        // Optional suffix: `u`/`U`, then up to two `l`/`L`.
        if let Some('u' | 'U') = self.peek1() {
            self.advance(1);
        }
        if let Some('l' | 'L') = self.peek1() {
            self.advance(1);
        }
        if let Some('l' | 'L') = self.peek1() {
            self.advance(1);
        }

        let origin = start.extend_to_inclusive(self.position);

        Token {
            kind: TokenKind::LiteralNumber(value),
            origin,
        }
    }

    pub(crate) fn advance(&mut self, count: usize) -> (Option<char>, Position) {
        let mut last = None;
        let mut position = self.position;
        for _ in 0..count {
            last = self.peek1();
            match last {
                None => {
                    break;
                }
                Some('\n') => {
                    self.position.byte_offset += 1;
                    self.position.column = 1;
                    self.position.line += 1;
                    self.chars_idx += 1;

                    position = self.position;
                }
                Some(c) => {
                    let len = c.len_utf8() as u32;
                    self.position.byte_offset += len;
                    self.position.column += len;
                    self.chars_idx += 1;
                    position = self.position;
                }
            }
        }
        (last, position)
    }

    fn peek1(&self) -> Option<char> {
        self.chars.get(self.chars_idx).copied()
    }

    fn peek2(&self) -> (Option<char>, Option<char>) {
        (
            self.chars.get(self.chars_idx).copied(),
            self.chars.get(self.chars_idx + 1).copied(),
        )
    }

    fn peek3(&self) -> (Option<char>, Option<char>, Option<char>) {
        (
            self.chars.get(self.chars_idx).copied(),
            self.chars.get(self.chars_idx + 1).copied(),
            self.chars.get(self.chars_idx + 2).copied(),
        )
    }

    pub fn lex(&mut self) -> Token {
        match (self.peek3(), &self.state) {
            ((None, _, _), _) => Token {
                kind: TokenKind::Eof,
                origin: self.position.into(),
            },
            ((Some('\n'), _, _), _) => {
                self.advance(1);
                self.lex()
            }
            ((Some('#'), Some('!'), _), _) => {
                // The official rule `^[\f\t\v ]*#!.*` requires that only
                // horizontal whitespace precedes `#!` on the same line.
                let line_start = self.input[..self.position.byte_offset as usize]
                    .rfind('\n')
                    .map_or(0, |i| i + 1);
                let prefix_on_line = &self.input[line_start..self.position.byte_offset as usize];
                if prefix_on_line.bytes().any(|b| !matches!(b, b'\t' | b'\x0C' | b'\x0B' | b' ')) {
                    self.add_error(
                        ErrorKind::ShebangMustComeFirst,
                        Position::default().extend_to_inclusive(self.position),
                        "only horizontal whitespace (space, tab, form feed, vertical tab) is allowed before #! on the same line",
                    );
                }
                let start = self.position;
                self.advance(2);
                self.skip_until_exclusive('\n');
                let origin = start.extend_to_inclusive(self.position);
                let s = str_from_source(self.input, origin.forwards(2))
                    .trim_ascii()
                    .to_owned();
                self.control_directives.push(ControlDirective {
                    origin,
                    kind: ControlDirectiveKind::Shebang(s),
                });
                self.lex()
            }
            ((Some('#'), _, _), LexerState::ProgramOuterScope) => {
                self.state = LexerState::InsideControlDirective(self.position.line);
                let start = self.position;
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
                match self.control_directive(&tokens, start.into()) {
                    Ok(directive) => self.control_directives.push(directive),
                    Err(err) => self.errors.push(err),
                }
                self.lex()
            }
            ((Some('-'), Some('-'), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::MinusMinus,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('-'), Some('='), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::MinusEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('-'), Some('>'), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::Arrow,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('-'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Minus,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('+'), Some('+'), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::PlusPlus,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('+'), Some('='), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::PlusEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('+'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Plus,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('.'), Some('.'), Some('.')), LexerState::ProgramOuterScope) => {
                let start = self.position;
                self.advance(3);
                Token {
                    kind: TokenKind::DotDotDot,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('.'), Some(d), _), _) if d.is_ascii_digit() => {
                let start = self.position;
                self.advance(2);
                self.add_error(
                    ErrorKind::UnsupportedLiteralFloatNumber,
                    start.extend_to_inclusive(self.position),
                    "floating-point literals are not supported",
                );
                Token {
                    kind: TokenKind::LiteralNumber(0),
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('.'), _, _), LexerState::ProgramOuterScope) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Dot,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('.'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                self.add_error(
                    ErrorKind::UnexpectedPeriod,
                    start.extend_to_inclusive(self.position),
                    "unexpected '.'; did you mean '->' for pointer member access?",
                );
                Token {
                    kind: TokenKind::Dot,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('*'), Some('='), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::StarEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('*'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Star,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('>'), Some('>'), Some('=')), _) => {
                let start = self.position;
                self.advance(3);
                Token {
                    kind: TokenKind::GtGtEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('>'), Some('>'), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::GtGt,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('>'), Some('='), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::GtEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('>'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Gt,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('<'), Some('<'), Some('=')), _) => {
                let start = self.position;
                self.advance(3);
                Token {
                    kind: TokenKind::LtLtEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('<'), Some('<'), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::LtLt,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('<'), Some('='), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::LtEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('<'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Lt,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('^'), Some('='), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::CaretEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('^'), Some('^'), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::CaretCaret,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('^'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Caret,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('&'), Some('='), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::AmpersandEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('&'), Some('&'), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::AmpersandAmpersand,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('&'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Ampersand,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('?'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Question,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('|'), Some('='), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::PipeEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('|'), Some('|'), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::PipePipe,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('|'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Pipe,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some(':'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Colon,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('!'), Some('='), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::BangEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('!'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Bang,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('='), Some('='), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::EqEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('='), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Eq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('/'), Some('='), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::SlashEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('/'), Some('/'), _), _) => {
                self.single_line_comment();
                self.lex()
            }
            ((Some('/'), Some('*'), _), _) => {
                self.multi_line_comment();
                self.lex()
            }
            ((Some('/'), _, _), LexerState::InsideClauseAndExpr) => {
                let start = self.position;
                self.advance(1);
                let end = self.position; // Capture end before consuming lookahead whitespace.

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
                    None | Some(';' | '{' | '/') => TokenKind::ClosePredicateDelimiter,
                    _ => TokenKind::Slash,
                };

                Token {
                    kind,
                    origin: start.extend_to_inclusive(end),
                }
            }
            ((Some('/'), _, _), LexerState::ProgramOuterScope) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Slash,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('%'), Some('='), _), _) => {
                let start = self.position;
                self.advance(2);
                Token {
                    kind: TokenKind::PercentEq,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('%'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Percent,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('~'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Tilde,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('{'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                if self.state == LexerState::ProgramOuterScope {
                    self.state = LexerState::InsideClauseAndExpr;
                }
                Token {
                    kind: TokenKind::LeftCurly,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('}'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                if self.state == LexerState::InsideClauseAndExpr {
                    self.state = LexerState::ProgramOuterScope;
                }
                Token {
                    kind: TokenKind::RightCurly,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('('), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::LeftParen,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some(')'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::RightParen,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some(','), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::Comma,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('['), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::LeftSquareBracket,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some(']'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::RightSquareBracket,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some(';'), _, _), _) => {
                let start = self.position;
                self.advance(1);
                Token {
                    kind: TokenKind::SemiColon,
                    origin: start.extend_to_inclusive(self.position),
                }
            }
            ((Some('"'), _, _), _) => self.lex_literal_string(),
            ((Some('\''), _, _), _) => self.lex_literal_character(),
            // Macro.
            ((Some('$'), Some('$'), Some('0'..='9')), LexerState::InsideClauseAndExpr) => {
                let origin = self.position;
                self.advance(2);
                self.macro_argument_reference(origin)
            }
            ((Some('$'), Some(d), _), LexerState::InsideClauseAndExpr) if d.is_ascii_digit() => {
                let origin = self.position;
                self.advance(1);
                self.macro_argument_reference(origin)
            }
            ((Some('@'), _, _), LexerState::InsideClauseAndExpr) => self.lex_aggregation(),

            // Skip: `__attribute__  (( ... )); ...`.
            // ^"__attribute__"[\f\n\r\t\v ]*"(("[^\n]*"));"
            (
                (Some('_'), Some('_'), Some('a')),
                LexerState::ProgramOuterScope | LexerState::InsideClauseAndExpr,
            ) if self.position.column == 1
                && self.input[self.position.byte_offset as usize + 3..]
                    .starts_with("ttribute__") =>
            {
                if let Some(attr) = self.lex_attribute_line() {
                    self.attributes.push(attr);
                    self.skip_until_exclusive('\n');

                    // Now lex normally.
                    self.lex()
                } else if let Some(attr) = self.lex_attribute() {
                    // No terminating `;`, but `RGX_ATTR` still matches at column 1.
                    self.attributes.push(attr);
                    self.lex()
                } else {
                    // It was not a real attribute after all, fall back to normal token lexing.
                    match self.state {
                        LexerState::ProgramOuterScope => self.lex_probe_specifier(),
                        LexerState::InsideControlDirective(_) => self.lex_pragma_identifier(),
                        LexerState::InsideClauseAndExpr => self.lex_identifier(),
                    }
                }
            }
            // Skip: `__attribute__  (( ... ))`.
            // "__attribute__"[\f\n\r\t\v ]*"(("[^\n]*"))"
            (
                (Some('_'), Some('_'), Some('a')),
                LexerState::ProgramOuterScope | LexerState::InsideClauseAndExpr,
            ) if self.input[self.position.byte_offset as usize + 3..].starts_with("ttribute__") => {
                if let Some(attr) = self.lex_attribute() {
                    self.attributes.push(attr);

                    // Now lex normally.
                    self.lex()
                } else {
                    // It was not a real attribute after all, fall back to normal token lexing.
                    match self.state {
                        LexerState::ProgramOuterScope => self.lex_probe_specifier(),
                        LexerState::InsideControlDirective(_) => self.lex_pragma_identifier(),
                        LexerState::InsideClauseAndExpr => self.lex_identifier(),
                    }
                }
            }
            ((Some(c), _, _), _) if c.is_ascii_digit() => self.lex_literal_number(),
            ((Some(c), _, _), _) if c.is_whitespace() => {
                self.advance(1);
                self.lex()
            }
            ((Some(c), _, _), LexerState::InsideControlDirective(_))
                if !c.is_ascii_whitespace() && c != '"' =>
            {
                self.lex_pragma_identifier()
            }
            ((Some(c), _, _), LexerState::ProgramOuterScope)
                if is_character_probe_specifier_start(c) =>
            {
                // TODO: Handle ambiguity of '*'.
                let token = self.lex_probe_specifier();
                self.lex_convert_to_keyword(token)
            }
            ((Some(c), _, _), LexerState::InsideClauseAndExpr | LexerState::ProgramOuterScope)
                if self.is_identifier_character_leading(c) =>
            {
                let token = self.lex_identifier();
                self.lex_convert_to_keyword(token)
            }
            ((Some(c), _, _), _) => {
                let start = self.position;
                self.errors.push(Error::new(
                    ErrorKind::UnknownToken,
                    start.extend_to_inclusive(self.position),
                    format!("unexpected character '{c}'"),
                ));
                self.advance(1);
                Token {
                    kind: TokenKind::Unknown(Some(c)),
                    origin: start.extend_to_inclusive(self.position),
                }
            }
        }
    }

    #[warn(unused_results)]
    fn control_directive(
        &mut self,
        tokens: &[Token],
        origin: Origin,
    ) -> Result<ControlDirective, Error> {
        match tokens.first() {
            None => {
                // According to K&R[A12.9], we silently ignore null directive lines.
                Ok(ControlDirective {
                    kind: ControlDirectiveKind::Ignored,
                    origin,
                })
            }
            Some(Token {
                kind: TokenKind::LiteralNumber(_),
                ..
            }) => self.control_directive_line(tokens, origin),
            Some(Token {
                kind: TokenKind::Identifier,
                origin: origin_ident,
            }) => {
                let src = str_from_source(self.input, *origin_ident);
                match src {
                    "line" => self.control_directive_line(&tokens[1..], origin),
                    "pragma" if tokens.len() > 1 => {
                        self.control_directive_pragma(&tokens[1..], origin)
                    }
                    // Ignore any #ident or #pragma ident lines.
                    "pragma" if tokens.len() == 1 => Ok(ControlDirective {
                        kind: ControlDirectiveKind::Ignored,
                        origin: origin.start.extend_to_inclusive(
                            tokens.last().map_or(origin.end, |t| t.origin.end),
                        ),
                    }),

                    "ident" => Ok(ControlDirective {
                        kind: ControlDirectiveKind::Ignored,
                        origin: origin.start.extend_to_inclusive(
                            tokens.last().map_or(origin.end, |t| t.origin.end),
                        ),
                    }),
                    "error" => self.control_directive_error(&tokens[1..]),
                    _ => Err(Error::new(
                        ErrorKind::InvalidControlDirective,
                        origin.start.extend_to_inclusive(
                            tokens.last().map_or(origin.end, |t| t.origin.end),
                        ),
                        String::new(),
                    )),
                }
            }
            Some(_) => Err(Error::new(
                ErrorKind::InvalidControlDirective,
                origin
                    .start
                    .extend_to_inclusive(tokens.last().map_or(origin.end, |t| t.origin.end)),
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
                    kind: TokenKind::LiteralNumber(_),
                    ..
                },
            ] => (line, None, None),
            // `5 "foo.d"`
            [
                line @ Token {
                    kind: TokenKind::LiteralNumber(_),
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
                    kind: TokenKind::LiteralNumber(_),
                    ..
                },
                file @ Token {
                    kind: TokenKind::LiteralString,
                    ..
                },
                trailing @ Token {
                    kind: TokenKind::LiteralNumber(_),
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

        let line_src = str_from_source(self.input, line.origin);
        let file_src = file.map(|f| {
            let s = str_from_source(self.input, f.origin);
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
            match str::parse::<usize>(str_from_source(self.input, trailing.origin)) {
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
            origin: origin
                .start
                .extend_to_inclusive(tokens.last().map_or(origin.end, |t| t.origin.end)),
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
                Some(str_from_source(self.input, *origin1)),
                Some(str_from_source(self.input, *origin2)),
            ),
            (
                Some(Token {
                    kind: TokenKind::Identifier,
                    origin: origin1,
                }),
                _,
            ) => (Some(str_from_source(self.input, *origin1)), None),
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
                origin: origin
                    .start
                    .extend_to_inclusive(tokens.last().map_or(origin.end, |last| last.origin.end)),
            }),
        }
    }

    #[warn(unused_results)]
    fn control_directive_error(&mut self, tokens: &[Token]) -> Result<ControlDirective, Error> {
        let src = match (tokens.get(1), tokens.last()) {
            (Some(start), Some(end)) => self.input
                [start.origin.start.byte_offset as usize..end.origin.end.byte_offset as usize]
                .to_owned(),
            _ => String::new(),
        };

        Ok(ControlDirective {
            origin: tokens[0]
                .origin
                .start
                .extend_to_inclusive(tokens.last().map_or(tokens[0].origin.end, |t| t.origin.end)),
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
                let s1 = str_from_source(self.input, *origin1);
                let s2 = str_from_source(self.input, *origin2);
                (s1, s2)
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidControlDirective,
                    origin
                        .start
                        .extend_to_inclusive(tokens.last().map_or(origin.end, |t| t.origin.end)),
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
                origin: {
                    let skip = (s1.len() - trailing.len()) as u32;
                    let n = trailing.len() as u32;
                    let _start = crate::origin::Position {
                        byte_offset: origin_identifier_first.start.byte_offset + skip,
                        column: origin_identifier_first.start.column + skip,
                        ..origin_identifier_first.start
                    };
                    let _end = crate::origin::Position {
                        byte_offset: _start.byte_offset + n,
                        column: _start.column + n,
                        .._start
                    };
                    _start.extend_to_inclusive(_end)
                },
                explanation: String::from(
                    "expected up to 3 parts in attribute but found an extraneous part",
                ),
            });
        }
        let name = name_str
            .map(|s| {
                Stability::try_from(*s).map_err(|kind| Error {
                    kind,
                    origin: {
                        let n = s.len() as u32;
                        let _s = origin_identifier_first.start;
                        _s.extend_to_inclusive(crate::origin::Position {
                            byte_offset: _s.byte_offset + n,
                            column: _s.column + n,
                            .._s
                        })
                    },
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
                    origin: {
                        let n = s.len() as u32;
                        let sk = skip as u32;
                        let _start = crate::origin::Position {
                            byte_offset: origin_identifier_first.start.byte_offset + sk,
                            column: origin_identifier_first.start.column + sk,
                            ..origin_identifier_first.start
                        };
                        _start.extend_to_inclusive(crate::origin::Position {
                            byte_offset: _start.byte_offset + n,
                            column: _start.column + n,
                            .._start
                        })
                    },
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
                    origin: {
                        let n = s.len() as u32;
                        let sk = skip as u32;
                        let _start = crate::origin::Position {
                            byte_offset: origin_identifier_first.start.byte_offset + sk,
                            column: origin_identifier_first.start.column + sk,
                            ..origin_identifier_first.start
                        };
                        _start.extend_to_inclusive(crate::origin::Position {
                            byte_offset: _start.byte_offset + n,
                            column: _start.column + n,
                            .._start
                        })
                    },
                    explanation: format!(
                        "invalid class, possible values are: {}",
                        CLASS_POSSIBLE_VALUES
                    ),
                })
            })
            .transpose()?;
        let attribute = PragmaAttribute { name, data, class };

        Ok(ControlDirective {
            kind: ControlDirectiveKind::PragmaAttributes {
                attribute,
                // TODO: `s2` may be `provider`, etc and should be handled differently in these
                // cases.
                name: s2.to_owned(),
            },
            origin: origin
                .start
                .extend_to_inclusive(tokens.last().map_or(origin.end, |t| t.origin.end)),
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
                let (version_str, version_origin) = quoted_string_from_source(self.input, *origin1);
                let version = version_str2num(version_str, version_origin)?;
                let identifier = str_from_source(self.input, *origin2).to_owned();

                Ok(ControlDirective {
                    origin: origin
                        .start
                        .extend_to_inclusive(tokens.last().map_or(origin.end, |t| t.origin.end)),
                    kind: ControlDirectiveKind::PragmaBinding(version, identifier),
                })
            }
            _ => Err(Error::new(
                ErrorKind::InvalidControlDirective,
                origin
                    .start
                    .extend_to_inclusive(tokens.last().map_or(origin.end, |t| t.origin.end)),
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

                let s = str_from_source(self.input, *origin1);
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
                            origin: origin.start.extend_to_inclusive(
                                tokens.last().map_or(origin.end, |t| t.origin.end),
                            ),
                        })
                    }
                } else {
                    Ok(ControlDirective {
                        kind: ControlDirectiveKind::PragmaOption(s.to_owned(), None),
                        origin: origin.start.extend_to_inclusive(
                            tokens.last().map_or(origin.end, |t| t.origin.end),
                        ),
                    })
                }
            }
            other => Err(Error {
                kind: ErrorKind::InvalidControlDirective,
                origin: origin
                    .start
                    .extend_to_inclusive(other.last().map_or(origin.end, |t| t.origin.end)),
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
                let kind = str_from_source(self.input, *origin1);
                let name = str_from_source(self.input, *origin2);
                (kind, name)
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidControlDirective,
                    origin
                        .start
                        .extend_to_inclusive(tokens.last().map_or(origin.end, |t| t.origin.end)),
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
            origin: origin
                .start
                .extend_to_inclusive(tokens.last().map_or(origin.end, |t| t.origin.end)),
        })
    }

    fn single_line_comment(&mut self) {
        let origin = self.position;

        let first = self.peek1().unwrap();
        assert_eq!(first, '/');
        self.advance(1);

        let second = self.peek1().unwrap();
        assert_eq!(second, '/');
        self.advance(1);

        loop {
            match self.peek2() {
                (Some('\n'), _) => {
                    break;
                }
                (Some('/'), Some('/' | '*')) => {
                    self.errors.push(Error {
                        kind: ErrorKind::NestedComment,
                        origin: self.position.extend_to_inclusive(crate::origin::Position {
                            byte_offset: self.position.byte_offset + 2,
                            column: self.position.column + 2,
                            ..self.position
                        }),
                        explanation: String::from("nested comment"),
                    });
                    self.advance(1);
                }
                (Some('*'), Some('/')) => {
                    self.errors.push(Error {
                        kind: ErrorKind::NestedComment,
                        origin: self.position.extend_to_inclusive(crate::origin::Position {
                            byte_offset: self.position.byte_offset + 2,
                            column: self.position.column + 2,
                            ..self.position
                        }),
                        explanation: String::from("nested comment"),
                    });
                    self.advance(1);
                }
                (Some(_), _) => {
                    self.advance(1);
                }
                (None, _) => {
                    break;
                }
            }
        }

        let origin = origin.extend_to_inclusive(self.position);

        self.comments.push(Comment {
            kind: CommentKind::SingleLine,
            origin,
        });
    }

    fn multi_line_comment(&mut self) {
        let origin = self.position;

        let first = self.peek1().unwrap();
        assert_eq!(first, '/');
        self.advance(1);

        let second = self.peek1().unwrap();
        assert_eq!(second, '*');
        self.advance(1);

        loop {
            match self.peek2() {
                (Some('/'), Some('*')) => {
                    self.errors.push(Error {
                        kind: ErrorKind::NestedComment,
                        origin: self.position.extend_to_inclusive(crate::origin::Position {
                            byte_offset: self.position.byte_offset + 2,
                            column: self.position.column + 2,
                            ..self.position
                        }),
                        explanation: String::from("nested comment"),
                    });
                    self.advance(1);
                }
                (Some('*'), Some('/')) => {
                    self.advance(1);
                    self.advance(1);
                    break;
                }
                (Some(_), _) => {
                    self.advance(1);
                }
                (None, _) => {
                    break;
                }
            }
        }

        let origin = origin.extend_to_inclusive(self.position);

        self.comments.push(Comment {
            kind: CommentKind::MultiLine,
            // FIXME: Known issue: this is the only token spanning multiple lines and `origin` only
            // supports single-line tokens. So the `line` field means `line_start`, *not*
            // `line_end` which might be bigger.
            origin,
        });
    }

    fn lex_aggregation(&mut self) -> Token {
        let start = self.position;
        let (first, _) = self.advance(1);
        assert_eq!(first, Some('@'));

        let second = self.peek1();
        match second {
            Some('a'..'z' | 'A'..'Z' | '_') => {
                self.advance(1);
            }
            _ => {
                return Token {
                    kind: TokenKind::Aggregation,
                    origin: start.extend_to_inclusive(self.position),
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
            origin: start.extend_to_inclusive(self.position),
        }
    }

    fn skip_until_inclusive_or_newline(&mut self, arg: char) {
        while let Some(c) = self.peek1() {
            self.advance(1);
            if c == arg || c == '\n' {
                break;
            }
        }
    }

    fn skip_until_exclusive(&mut self, arg: char) {
        while let Some(c) = self.peek1() {
            if c == arg {
                break;
            }
            self.advance(1);
        }
    }

    pub(crate) fn begin(&mut self, state: LexerState) {
        self.state = state;
    }

    fn macro_argument_reference(&mut self, start: Position) -> Token {
        while let Some(c) = self.peek1() {
            if c.is_ascii_digit() {
                self.advance(1);
            } else {
                break;
            }
        }

        let s = &self.input[start.byte_offset as usize..self.position.byte_offset as usize]
            .trim_start_matches('$');
        dbg!(s);
        assert!(!s.is_empty());

        let num: Option<u32> = match s.parse::<i32>() {
            Ok(n) => Some(n as u32),
            Err(_) => None,
        };

        Token {
            kind: TokenKind::MacroArgumentReference(num),
            origin: start.extend_to_inclusive(self.position),
        }
    }

    // Regexp: ([0-9]+("."?)[0-9]*|"."[0-9]+)((e|E)("+"|-)?[0-9]+)?[fFlL]?
    // But this function gets called when encountering digits followed by a period.
    // This is a best effort to skip what ressembles a float and hopefully synchronize as well as
    // possible.
    fn skip_until_end_of_float(&mut self) {
        while let Some('0'..='9') = self.peek1() {
            self.advance(1);
        }

        if let Some('e' | 'E') = self.peek1() {
            self.advance(1);
        }
        if let Some('+' | '-') = self.peek1() {
            self.advance(1);
        }

        while let Some('0'..='9') = self.peek1() {
            self.advance(1);
        }

        if let Some('f' | 'F' | 'l' | 'L') = self.peek1() {
            self.advance(1);
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek1()
            && c.is_ascii_whitespace()
        {
            self.advance(1);
        }
    }

    fn lex_attribute_line(&mut self) -> Option<Attribute> {
        assert_eq!(self.position.column, 1);
        assert!(self.input[self.position.byte_offset as usize..].starts_with("__attribute__"));

        // For rollbacking.
        let (bck_position, bck_chars_idx) = (self.position, self.chars_idx);

        self.advance("__attribute__".len());
        self.skip_whitespace();

        if let (Some('('), Some('(')) = self.peek2() {
        } else {
            // Rollback.
            self.position = bck_position;
            self.chars_idx = bck_chars_idx;
            return None;
        }

        loop {
            match self.peek3() {
                // End of attribute.
                (Some(')'), Some(')'), Some(';')) => {
                    self.advance(3);
                    return Some(Attribute {
                        origin: bck_position.extend_to_inclusive(self.position),
                    });
                }
                (Some('\n'), _, _) | (None, _, _) => {
                    // Rollback.
                    self.position = bck_position;
                    self.chars_idx = bck_chars_idx;
                    return None;
                }
                (Some(_), _, _) => {
                    self.advance(1);
                }
            }
        }
    }

    fn lex_attribute(&mut self) -> Option<Attribute> {
        assert!(self.input[self.position.byte_offset as usize..].starts_with("__attribute__"));

        // For rollbacking.
        let (bck_position, bck_chars_idx) = (self.position, self.chars_idx);

        self.advance("__attribute__".len());
        self.skip_whitespace();

        if let (Some('('), Some('(')) = self.peek2() {
        } else {
            // Rollback.
            self.position = bck_position;
            self.chars_idx = bck_chars_idx;
            return None;
        }

        loop {
            match self.peek2() {
                // End of attribute.
                (Some(')'), Some(')')) => {
                    self.advance(2);
                    return Some(Attribute {
                        origin: bck_position.extend_to_inclusive(self.position),
                    });
                }
                (Some('\n'), _) | (None, _) => {
                    // Rollback.
                    self.position = bck_position;
                    self.chars_idx = bck_chars_idx;
                    return None;
                }
                (Some(_), _) => {
                    self.advance(1);
                }
            }
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
            origin: {
                let skip = (version_str.len() - trailing.len()) as u32;
                let n = trailing.len() as u32;
                let _start = crate::origin::Position {
                    byte_offset: origin.start.byte_offset + skip,
                    column: origin.start.column + skip,
                    ..origin.start
                };
                let _end = crate::origin::Position {
                    byte_offset: _start.byte_offset + n,
                    column: _start.column + n,
                    .._start
                };
                _start.extend_to_inclusive(_end)
            },
            explanation: String::from(
                "expected up to 3 parts in version string but found an extraneous part",
            ),
        });
    }

    let major = str::parse::<u8>(major_str).map_err(|err| Error {
        kind: ErrorKind::InvalidVersionString,
        origin: {
            let n = major_str.len() as u32;
            let _s = origin.start;
            _s.extend_to_inclusive(crate::origin::Position {
                byte_offset: _s.byte_offset + n,
                column: _s.column + n,
                .._s
            })
        },
        explanation: format!(
            "invalid major version in version string, expected a number up to 255: {}",
            err
        ),
    })?;

    let origin = origin.forwards(major_str.len() + 1);
    let minor = str::parse::<u16>(minor_str).map_err(|err| Error {
        kind: ErrorKind::InvalidVersionString,
        origin: {
            let n = minor_str.len() as u32;
            let _s = origin.start;
            _s.extend_to_inclusive(crate::origin::Position {
                byte_offset: _s.byte_offset + n,
                column: _s.column + n,
                .._s
            })
        },
        explanation: format!(
            "invalid minor version in version string, expected a number: {}",
            err
        ),
    })?;
    if minor > 0xfff {
        return Err(Error {
            kind: ErrorKind::InvalidVersionString,
            origin: {
                let n = minor_str.len() as u32;
                let _s = origin.start;
                _s.extend_to_inclusive(crate::origin::Position {
                    byte_offset: _s.byte_offset + n,
                    column: _s.column + n,
                    .._s
                })
            },
            explanation: String::from(
                "minor version too high in version string, expected a number up to 4095",
            ),
        });
    }

    let origin = origin.forwards(minor_str.len() + 1);
    let patch = if let Some(patch_str) = patch_str {
        let num = str::parse::<u16>(patch_str).map_err(|err| Error {
            kind: ErrorKind::InvalidVersionString,
            origin: {
                let n = patch_str.len() as u32;
                let _s = origin.start;
                _s.extend_to_inclusive(crate::origin::Position {
                    byte_offset: _s.byte_offset + n,
                    column: _s.column + n,
                    .._s
                })
            },
            explanation: format!(
                "invalid patch version in version string, expected a number: {}",
                err
            ),
        })?;
        if num > 0xfff {
            return Err(Error {
                kind: ErrorKind::InvalidVersionString,
                origin: {
                    let n = patch_str.len() as u32;
                    let _s = origin.start;
                    _s.extend_to_inclusive(crate::origin::Position {
                        byte_offset: _s.byte_offset + n,
                        column: _s.column + n,
                        .._s
                    })
                },
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
mod tests {
    use crate::{
        error::ErrorKind,
        lex::{
            ControlDirectiveKind, Lexer, LexerState, PragmaDependsOnKind, TokenKind,
            str_from_source,
        },
        origin::{Position, PositionKind},
    };

    const FILE_ID: u32 = 1;

    fn pos(line: u32, column: u32, byte_offset: u32) -> Position {
        Position {
            line,
            column,
            byte_offset,
            kind: PositionKind::File(FILE_ID),
        }
    }

    #[test]
    fn test_probe_specifier() {
        let input = "syscall::open:entry{}";
        let mut lexer = Lexer::new(1, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::ProbeSpecifier);
        assert_eq!(str_from_source(input, token.origin), "syscall::open:entry");
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LeftCurly);
            assert_eq!(str_from_source(input, token.origin), "{");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::RightCurly);
            assert_eq!(str_from_source(input, token.origin), "}");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_probe_with_predicate() {
        let input = "fbt::: /self->spec/ {}";
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::ProbeSpecifier);
            assert_eq!(str_from_source(input, token.origin), "fbt:::");
        }
        lexer.begin(LexerState::InsideClauseAndExpr);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Slash);
            assert_eq!(str_from_source(input, token.origin), "/");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::KeywordSelf);
            assert_eq!(str_from_source(input, token.origin), "self");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Arrow);
            assert_eq!(str_from_source(input, token.origin), "->");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Identifier);
            assert_eq!(str_from_source(input, token.origin), "spec");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::ClosePredicateDelimiter);
            assert_eq!(str_from_source(input, token.origin), "/");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LeftCurly);
            assert_eq!(str_from_source(input, token.origin), "{");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::RightCurly);
            assert_eq!(str_from_source(input, token.origin), "}");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_slash() {
        let input = "BEGIN / 1 / 2 / {}";
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::ProbeSpecifier);
            assert_eq!(str_from_source(input, token.origin), "BEGIN");
        }
        lexer.begin(LexerState::InsideClauseAndExpr);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Slash);
            assert_eq!(str_from_source(input, token.origin), "/");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralNumber(1));
            assert_eq!(str_from_source(input, token.origin), "1");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Slash);
            assert_eq!(str_from_source(input, token.origin), "/");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralNumber(2));
            assert_eq!(str_from_source(input, token.origin), "2");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::ClosePredicateDelimiter);
            assert_eq!(str_from_source(input, token.origin), "/");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LeftCurly);
            assert_eq!(str_from_source(input, token.origin), "{");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::RightCurly);
            assert_eq!(str_from_source(input, token.origin), "}");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_predicate() {
        let input = r#"syscall::read:entry,
        syscall::write:entry
        /pid == 102429/
        {
        }
"#;
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::ProbeSpecifier);
            let s = str_from_source(input, token.origin);
            assert_eq!(s, "syscall::read:entry");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Comma);
            assert_eq!(str_from_source(input, token.origin), ",");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::ProbeSpecifier);
            assert_eq!(str_from_source(input, token.origin), "syscall::write:entry");
        }
        lexer.begin(LexerState::InsideClauseAndExpr);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Slash);
            assert_eq!(str_from_source(input, token.origin), "/");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Identifier);
            assert_eq!(str_from_source(input, token.origin), "pid");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::EqEq);
            assert_eq!(str_from_source(input, token.origin), "==");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralNumber(102429));
            assert_eq!(str_from_source(input, token.origin), "102429");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::ClosePredicateDelimiter);
            assert_eq!(str_from_source(input, token.origin), "/");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LeftCurly);
            assert_eq!(str_from_source(input, token.origin), "{");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::RightCurly);
            assert_eq!(str_from_source(input, token.origin), "}");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_hex_number() {
        let input = "0xcafebabe";
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralNumber(0xcafe_babe));
            let s = str_from_source(input, token.origin);
            assert_eq!(s, "0xcafebabe");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal() {
        let input = "'r'";
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralCharacter('r' as isize));
            let s = str_from_source(input, token.origin);
            assert_eq!(s, input);
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_escape_sequence_c() {
        let input = "'\r'";
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralCharacter(13));
            let s = str_from_source(input, token.origin);
            assert_eq!(s, input);
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_escape_sequence_octal() {
        // 3-digit octal: \103 = 67 = 0x43
        let input = r#"'\103'"#;
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralCharacter(0o103));
            let s = str_from_source(input, token.origin);
            assert_eq!(s, input);
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_escape_sequence_octal_overflow() {
        // octal 400 = 256, truncated to u8 = 0, matching C `char` truncation.
        let input = r#"'\400'"#;
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralCharacter(0));
            let s = str_from_source(input, token.origin);
            assert_eq!(s, input);
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_unknown_escape() {
        // \z is not a known escape: official `stresc2chr` default → 2 bytes: '\' (0x5C) + 'z' (0x7A).
        let input = r#"'\z'"#;
        let mut lexer = Lexer::new(1, input);
        let token = lexer.lex();
        let expected = isize::from_be_bytes([0, 0, 0, 0, 0, 0, 0x5c, 0x7a]);
        assert_eq!(token.kind, TokenKind::LiteralCharacter(expected));
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_octal_followed_by_literal_chars() {
        // \010 (octal, 3-digit limit) = 8, then '3','3','3','3','3' as literal chars
        // bytes = [8, 51, 51, 51, 51, 51] → 0x083333333333
        let input = r#"'\01033333'"#;
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            let expected = isize::from_be_bytes([0, 0, 8, 51, 51, 51, 51, 51]);
            assert_eq!(token.kind, TokenKind::LiteralCharacter(expected));
            let s = str_from_source(input, token.origin);
            assert_eq!(s, input);
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_escape_sequence_hex() {
        let input = r#"'\x4e'"#;
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralCharacter(0x4e));
            let s = str_from_source(input, token.origin);
            assert_eq!(s, "'\\x4e'");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_multiple() {
        let input = r#"'\"\ba';"#;
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            // '\"\ba': '\"' is a known escape → '"' (0x22); '\b' is backspace (0x08); 'a' is 0x61.
            // DTrace confirms: `sudo dtrace -n 'BEGIN {this->c = '\"\ba'; print(this->c); exit(0);}'` → 0x220861.
            let expected = isize::from_be_bytes([0, 0, 0, 0, 0, 0x22, 0x08, 0x61]);
            assert_eq!(token.kind, TokenKind::LiteralCharacter(expected));
            assert_eq!(
                str_from_source(input, token.origin),
                &input[0..input.len() - 1]
            );
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::SemiColon);
            assert_eq!(str_from_source(input, token.origin), ";");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_empty() {
        let input = "''";
        let mut lexer = Lexer::new(1, input);
        let token = lexer.lex();
        // Empty character literal produces an error and a None value token.
        assert_eq!(token.kind, TokenKind::LiteralCharacter(0));
        assert_eq!(str_from_source(input, token.origin), "''");
        assert_eq!(lexer.errors.len(), 1, "expected 1 error(s)");
        assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidLiteralCharacter);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error(s), no new error");
    }

    #[test]
    fn test_lex_probe_specifier_with_macro_argument_reference() {
        let input = "BEG$$1 ";
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::ProbeSpecifier);
            let s = str_from_source(input, token.origin);
            assert_eq!(s, &input[0..input.len() - 1]);
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Eof);
        }
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_origin_single_char_token() {
        let input = "+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(token.origin.start, pos(1, 1, 0));
        assert_eq!(token.origin.end, pos(1, 2, 1));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_origin_two_char_token() {
        let input = "++";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::PlusPlus);
        assert_eq!(token.origin.start, pos(1, 1, 0));
        assert_eq!(token.origin.end, pos(1, 3, 2));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_origin_token_after_whitespace() {
        let input = "   +";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(token.origin.start, pos(1, 4, 3));
        assert_eq!(token.origin.end, pos(1, 5, 4));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_origin_token_on_second_line() {
        let input = "\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(token.origin.start, pos(2, 1, 1));
        assert_eq!(token.origin.end, pos(2, 2, 2));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_origin_identifier() {
        let input = "hello {}";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::ProbeSpecifier);
        assert_eq!(token.origin.start, pos(1, 1, 0));
        assert_eq!(token.origin.end, pos(1, 6, 5));
        assert_eq!(lexer.lex().kind, TokenKind::LeftCurly);
        assert_eq!(lexer.lex().kind, TokenKind::RightCurly);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_origin_identifier_second_line() {
        let input = "\nhello {}";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::ProbeSpecifier);
        assert_eq!(token.origin.start, pos(2, 1, 1));
        assert_eq!(token.origin.end, pos(2, 6, 6));
        assert_eq!(lexer.lex().kind, TokenKind::LeftCurly);
        assert_eq!(lexer.lex().kind, TokenKind::RightCurly);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_origin_string_literal() {
        let input = r#""hello""#;
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralString);
        assert_eq!(token.origin.start, pos(1, 1, 0));
        assert_eq!(token.origin.end, pos(1, 8, 7));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_origin_unicode_string_literal() {
        // 18 Unicode characters, 52 UTF-8 bytes of content, 54 bytes total with quotes.
        // column tracks UTF-8 byte offsets, so end.column = 55, not 21 (code-point count).
        let input = r#""朝日新聞:朝日新聞社のニュースサイト""#;
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralString);
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(token.origin.start, pos(1, 1, 0));
        assert_eq!(token.origin.end, pos(1, 55, 54));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_string_literal_multiline() {
        // A literal newline inside a string is an error, but the lexer continues
        // past it and recovers at the closing quote. Subsequent tokens lex correctly.
        let input = "\"hello\nworld\" 42";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralString);
            assert_eq!(str_from_source(input, token.origin), "\"hello\nworld\"");
            assert_eq!(token.origin.start, pos(1, 1, 0));
            assert_eq!(token.origin.end, pos(2, 7, 13));
            assert_eq!(lexer.errors.len(), 1, "expected 1 error(s)");
            assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidLiteralString);
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralNumber(42));
            assert_eq!(str_from_source(input, token.origin), "42");
            assert_eq!(token.origin.start, pos(2, 8, 14));
            assert_eq!(token.origin.end, pos(2, 10, 16));
            assert_eq!(lexer.errors.len(), 1, "expected 1 error(s), no new error");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error(s), no new error");
    }

    #[test]
    fn test_lex_string_literal_backslash_newline() {
        // A backslash immediately followed by a newline inside a string is an error;
        // both chars are consumed and lexing continues on the next line.
        let input = "\"hello\\\nworld\" 42";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralString);
            assert_eq!(str_from_source(input, token.origin), "\"hello\\\nworld\"");
            assert_eq!(token.origin.start, pos(1, 1, 0));
            assert_eq!(token.origin.end, pos(2, 7, 14));
            assert_eq!(lexer.errors.len(), 1, "expected 1 error(s)");
            assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidLiteralString);
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralNumber(42));
            assert_eq!(str_from_source(input, token.origin), "42");
            assert_eq!(token.origin.start, pos(2, 8, 15));
            assert_eq!(token.origin.end, pos(2, 10, 17));
            assert_eq!(lexer.errors.len(), 1, "expected 1 error(s), no new error");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error(s), no new error");
    }

    #[test]
    fn test_lex_string_literal_unterminated() {
        // EOF before the closing quote records an error. The token spans everything consumed.
        let input = "\"hello";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralString);
        assert_eq!(str_from_source(input, token.origin), "\"hello");
        assert_eq!(token.origin.start, pos(1, 1, 0));
        assert_eq!(token.origin.end, pos(1, 7, 6));
        assert_eq!(lexer.errors.len(), 1, "expected 1 error(s)");
        assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidLiteralString);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error(s), no new error");
    }

    #[test]
    fn test_lex_string_literal_escape_sequence() {
        // A \n escape sequence (backslash then 'n', not a real newline) is valid — no error.
        let input = r#""hello\nworld""#;
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralString);
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(token.origin.start, pos(1, 1, 0));
        assert_eq!(token.origin.end, pos(1, 15, 14));
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_origin_single_line_comment() {
        // Comment ends at newline (exclusive), next token follows.
        let input = "// hi\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        // comment is consumed internally, lex() returns the next token
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.comments.len(), 1);
        let comment = &lexer.comments[0];
        assert_eq!(comment.origin.start, pos(1, 1, 0));
        assert_eq!(comment.origin.end, pos(1, 6, 5));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_origin_multiline_comment() {
        // "/* hi\nworld */" — 14 bytes, spans 2 lines.
        let input = "/* hi\nworld */+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.comments.len(), 1);
        let comment = &lexer.comments[0];
        assert_eq!(comment.origin.start, pos(1, 1, 0));
        // After consuming "/* hi\nworld */": line=2, column=9, byte_offset=14
        assert_eq!(comment.origin.end, pos(2, 9, 14));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_single_line_comment_nested_double_slash() {
        // "//" inside a single-line comment is forbidden.
        let input = "// hello // world\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(str_from_source(input, token.origin), "+");
        assert_eq!(lexer.comments.len(), 1);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::NestedComment);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_single_line_comment_nested_block_open() {
        // "/*" inside a single-line comment is forbidden.
        let input = "// hello /* world\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(str_from_source(input, token.origin), "+");
        assert_eq!(lexer.comments.len(), 1);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::NestedComment);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_single_line_comment_nested_block_close() {
        // "*/" inside a single-line comment is forbidden.
        let input = "// hello */ world\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(str_from_source(input, token.origin), "+");
        assert_eq!(lexer.comments.len(), 1);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::NestedComment);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_single_line_comment_nested_block_open_and_close() {
        // Both "/*" and "*/" inside a single-line comment each produce an error.
        let input = "// hello /* */\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(str_from_source(input, token.origin), "+");
        assert_eq!(lexer.comments.len(), 1);
        assert_eq!(lexer.errors.len(), 2, "expected 2 errors");
        assert_eq!(lexer.errors[0].kind, ErrorKind::NestedComment);
        assert_eq!(lexer.errors[1].kind, ErrorKind::NestedComment);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 2, "no new errors after Eof");
    }

    #[test]
    fn test_lex_multi_line_comment_nested_block_open() {
        // "/*" inside a block comment is forbidden.
        let input = "/* hello /* world */+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(str_from_source(input, token.origin), "+");
        assert_eq!(lexer.comments.len(), 1);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::NestedComment);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_multi_line_comment_double_slash_allowed() {
        // "//" inside a block comment is NOT an error (only "/*" is forbidden).
        let input = "/* hello // world */+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(str_from_source(input, token.origin), "+");
        assert_eq!(lexer.comments.len(), 1);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_macro_argument_reference() {
        let input = "BEGIN {print($$1) } ";
        let mut lexer = Lexer::new(1, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::ProbeSpecifier);
            let s = str_from_source(input, token.origin);
            assert_eq!(s, "BEGIN");
        }
        lexer.begin(LexerState::InsideClauseAndExpr);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LeftCurly);
            assert_eq!(str_from_source(input, token.origin), "{");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Identifier);
            assert_eq!(str_from_source(input, token.origin), "print");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LeftParen);
            assert_eq!(str_from_source(input, token.origin), "(");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::MacroArgumentReference(Some(1)));
            assert_eq!(str_from_source(input, token.origin), "$$1");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::RightParen);
            assert_eq!(str_from_source(input, token.origin), ")");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::RightCurly);
            assert_eq!(str_from_source(input, token.origin), "}");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Eof);
        }
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_literal_number_decimal() {
        let input = "42";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(42));
        assert_eq!(str_from_source(input, token.origin), "42");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_unknown() {
        let input = "\x01";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Unknown(Some('\x01')));
        assert_eq!(str_from_source(input, token.origin), "\x01");
        assert_eq!(lexer.errors.len(), 1, "expected 1 error(s)");
        assert_eq!(lexer.errors[0].kind, ErrorKind::UnknownToken);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error(s), no new error");
    }

    #[test]
    fn test_lex_paren() {
        let input = "()";
        let mut lexer = Lexer::new(FILE_ID, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LeftParen);
            assert_eq!(str_from_source(input, token.origin), "(");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::RightParen);
            assert_eq!(str_from_source(input, token.origin), ")");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_square_brackets() {
        let input = "[]";
        let mut lexer = Lexer::new(FILE_ID, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LeftSquareBracket);
            assert_eq!(str_from_source(input, token.origin), "[");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::RightSquareBracket);
            assert_eq!(str_from_source(input, token.origin), "]");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_dot() {
        let input = ".";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Dot);
        assert_eq!(str_from_source(input, token.origin), ".");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_dot_dot_dot() {
        let input = "...";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::DotDotDot);
        assert_eq!(str_from_source(input, token.origin), "...");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_tilde() {
        let input = "~";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Tilde);
        assert_eq!(str_from_source(input, token.origin), "~");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_question() {
        let input = "?";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Question);
        assert_eq!(str_from_source(input, token.origin), "?");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_eq() {
        let input = "=";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Eq);
        assert_eq!(str_from_source(input, token.origin), "=");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_bang_operators() {
        let input = "! !=";
        let mut lexer = Lexer::new(FILE_ID, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Bang);
            assert_eq!(str_from_source(input, token.origin), "!");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::BangEq);
            assert_eq!(str_from_source(input, token.origin), "!=");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_plus_eq() {
        let input = "+=";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::PlusEq);
        assert_eq!(str_from_source(input, token.origin), "+=");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_minus_operators() {
        let input = "- -- -=";
        let mut lexer = Lexer::new(FILE_ID, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Minus);
            assert_eq!(str_from_source(input, token.origin), "-");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::MinusMinus);
            assert_eq!(str_from_source(input, token.origin), "--");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::MinusEq);
            assert_eq!(str_from_source(input, token.origin), "-=");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_star_operators() {
        let input = "* *=";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Star);
            assert_eq!(str_from_source(input, token.origin), "*");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::StarEq);
            assert_eq!(str_from_source(input, token.origin), "*=");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_percent_operators() {
        let input = "% %=";
        let mut lexer = Lexer::new(FILE_ID, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Percent);
            assert_eq!(str_from_source(input, token.origin), "%");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::PercentEq);
            assert_eq!(str_from_source(input, token.origin), "%=");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_caret_operators() {
        let input = "^ ^^ ^=";
        let mut lexer = Lexer::new(FILE_ID, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Caret);
            assert_eq!(str_from_source(input, token.origin), "^");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::CaretCaret);
            assert_eq!(str_from_source(input, token.origin), "^^");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::CaretEq);
            assert_eq!(str_from_source(input, token.origin), "^=");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_ampersand_operators() {
        let input = "& && &=";
        let mut lexer = Lexer::new(FILE_ID, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Ampersand);
            assert_eq!(str_from_source(input, token.origin), "&");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::AmpersandAmpersand);
            assert_eq!(str_from_source(input, token.origin), "&&");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::AmpersandEq);
            assert_eq!(str_from_source(input, token.origin), "&=");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_pipe_operators() {
        let input = "| || |=";
        let mut lexer = Lexer::new(FILE_ID, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Pipe);
            assert_eq!(str_from_source(input, token.origin), "|");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::PipePipe);
            assert_eq!(str_from_source(input, token.origin), "||");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::PipeEq);
            assert_eq!(str_from_source(input, token.origin), "|=");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_comparison_operators() {
        let input = "> >= >>= >> < <= <<= <<";
        let mut lexer = Lexer::new(FILE_ID, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Gt);
            assert_eq!(str_from_source(input, token.origin), ">");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::GtEq);
            assert_eq!(str_from_source(input, token.origin), ">=");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::GtGtEq);
            assert_eq!(str_from_source(input, token.origin), ">>=");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::GtGt);
            assert_eq!(str_from_source(input, token.origin), ">>");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Lt);
            assert_eq!(str_from_source(input, token.origin), "<");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LtEq);
            assert_eq!(str_from_source(input, token.origin), "<=");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LtLtEq);
            assert_eq!(str_from_source(input, token.origin), "<<=");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LtLt);
            assert_eq!(str_from_source(input, token.origin), "<<");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_colon_operators() {
        let input = ": :=";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Colon);
            assert_eq!(str_from_source(input, token.origin), ":");
        }
        // ':=' is not a DTrace operator; tokenizes as ':' followed by '='
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Colon);
            assert_eq!(str_from_source(input, token.origin), ":");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Eq);
            assert_eq!(str_from_source(input, token.origin), "=");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_aggregation() {
        let input = "@count @";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Aggregation);
            assert_eq!(str_from_source(input, token.origin), "@count");
        }
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::Aggregation);
            assert_eq!(str_from_source(input, token.origin), "@");
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_macro_argument_reference_none() {
        // Number overflows i32 → MacroArgumentReference(None).
        let input = "$$99999999999";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::MacroArgumentReference(None));
        assert_eq!(str_from_source(input, token.origin), "$$99999999999");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_keywords_type_declarators() {
        for (kw, kind) in [
            ("auto", TokenKind::KeywordAuto),
            ("char", TokenKind::KeywordChar),
            ("const", TokenKind::KeywordConst),
            ("double", TokenKind::KeywordDouble),
            ("enum", TokenKind::KeywordEnum),
            ("float", TokenKind::KeywordFloat),
            ("int", TokenKind::KeywordInt),
            ("long", TokenKind::KeywordLong),
            ("short", TokenKind::KeywordShort),
            ("signed", TokenKind::KeywordSigned),
            ("struct", TokenKind::KeywordStruct),
        ] {
            let mut lexer = Lexer::new(FILE_ID, kw);
            let token = lexer.lex();
            assert_eq!(token.kind, kind, "kind mismatch for keyword {kw}");
            assert_eq!(
                str_from_source(kw, token.origin),
                kw,
                "origin mismatch for keyword {kw}"
            );
            assert_eq!(lexer.lex().kind, TokenKind::Eof);
            assert!(
                lexer.errors.is_empty(),
                "unexpected errors: {:?}",
                lexer.errors
            );
        }
    }

    #[test]
    fn test_lex_keywords_storage_class() {
        // All recognized as keywords in ProgramOuterScope (S2).
        for (kw, kind) in [
            ("counter", TokenKind::KeywordCounter),
            ("extern", TokenKind::KeywordExtern),
            ("inline", TokenKind::KeywordInline),
            ("register", TokenKind::KeywordRegister),
            ("restrict", TokenKind::KeywordRestrict),
            ("static", TokenKind::KeywordStatic),
        ] {
            let mut lexer = Lexer::new(FILE_ID, kw);
            let token = lexer.lex();
            assert_eq!(token.kind, kind, "kind mismatch for keyword {kw}");
            assert_eq!(
                str_from_source(kw, token.origin),
                kw,
                "origin mismatch for keyword {kw}"
            );
            assert_eq!(lexer.lex().kind, TokenKind::Eof);
            assert!(
                lexer.errors.is_empty(),
                "unexpected errors: {:?}",
                lexer.errors
            );
        }

        // counter and inline are S2-only keywords; in InsideClauseAndExpr they are identifiers.
        for kw in ["counter", "inline"] {
            let mut lexer = Lexer::new(FILE_ID, kw);
            lexer.begin(LexerState::InsideClauseAndExpr);
            let token = lexer.lex();
            assert_eq!(
                token.kind,
                TokenKind::Identifier,
                "{kw} should be Identifier in InsideClauseAndExpr"
            );
            assert_eq!(str_from_source(kw, token.origin), kw);
            assert_eq!(lexer.lex().kind, TokenKind::Eof);
            assert!(
                lexer.errors.is_empty(),
                "unexpected errors: {:?}",
                lexer.errors
            );
        }
    }

    #[test]
    fn test_lex_keywords_dtrace_types() {
        // All recognized as keywords in ProgramOuterScope (S2).
        for (kw, kind) in [
            ("import", TokenKind::KeywordImport),
            ("provider", TokenKind::KeywordProvider),
            ("string", TokenKind::KeywordString),
            ("translator", TokenKind::KeywordTranslator),
        ] {
            let mut lexer = Lexer::new(FILE_ID, kw);
            let token = lexer.lex();
            assert_eq!(token.kind, kind, "kind mismatch for keyword {kw}");
            assert_eq!(
                str_from_source(kw, token.origin),
                kw,
                "origin mismatch for keyword {kw}"
            );
            assert_eq!(lexer.lex().kind, TokenKind::Eof);
            assert!(
                lexer.errors.is_empty(),
                "unexpected errors: {:?}",
                lexer.errors
            );
        }

        // provider and translator are S2-only keywords; in InsideClauseAndExpr they are identifiers.
        for kw in ["provider", "translator"] {
            let mut lexer = Lexer::new(FILE_ID, kw);
            lexer.begin(LexerState::InsideClauseAndExpr);
            let token = lexer.lex();
            assert_eq!(
                token.kind,
                TokenKind::Identifier,
                "{kw} should be Identifier in InsideClauseAndExpr"
            );
            assert_eq!(str_from_source(kw, token.origin), kw);
            assert_eq!(lexer.lex().kind, TokenKind::Eof);
            assert!(
                lexer.errors.is_empty(),
                "unexpected errors: {:?}",
                lexer.errors
            );
        }
    }

    #[test]
    fn test_lex_keywords_this() {
        let mut lexer = Lexer::new(FILE_ID, "this");
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::KeywordThis);
        assert_eq!(str_from_source("this", token.origin), "this");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_keywords_control_flow() {
        let input = "break case continue default do else for goto if return switch while";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        for (kind, text) in [
            (TokenKind::KeywordBreak, "break"),
            (TokenKind::KeywordCase, "case"),
            (TokenKind::KeywordContinue, "continue"),
            (TokenKind::KeywordDefault, "default"),
            (TokenKind::KeywordDo, "do"),
            (TokenKind::KeywordElse, "else"),
            (TokenKind::KeywordFor, "for"),
            (TokenKind::KeywordGoto, "goto"),
            (TokenKind::KeywordIf, "if"),
            (TokenKind::KeywordReturn, "return"),
            (TokenKind::KeywordSwitch, "switch"),
            (TokenKind::KeywordWhile, "while"),
        ] {
            let token = lexer.lex();
            assert_eq!(token.kind, kind);
            assert_eq!(str_from_source(input, token.origin), text);
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_keywords_type_qualifiers() {
        let input = "offsetof sizeof typedef union unsigned userland void volatile";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        for (kind, text) in [
            (TokenKind::KeywordOffsetOf, "offsetof"),
            (TokenKind::KeywordSizeof, "sizeof"),
            (TokenKind::KeywordTypedef, "typedef"),
            (TokenKind::KeywordUnion, "union"),
            (TokenKind::KeywordUnsigned, "unsigned"),
            (TokenKind::KeywordUserland, "userland"),
            (TokenKind::KeywordVoid, "void"),
            (TokenKind::KeywordVolatile, "volatile"),
        ] {
            let token = lexer.lex();
            assert_eq!(token.kind, kind);
            assert_eq!(str_from_source(input, token.origin), text);
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_keywords_dtrace_expr() {
        let input = "probe stringof xlate";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        for (kind, text) in [
            (TokenKind::KeywordProbe, "probe"),
            (TokenKind::KeywordStringof, "stringof"),
            (TokenKind::KeywordXlate, "xlate"),
        ] {
            let token = lexer.lex();
            assert_eq!(token.kind, kind);
            assert_eq!(str_from_source(input, token.origin), text);
        }
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_slash_eq() {
        // /= must be lexed in InsideClauseAndExpr so '/' is not a predicate delimiter.
        let input = "/=";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::SlashEq);
        assert_eq!(str_from_source(input, token.origin), "/=");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_identifier_backtick() {
        // Backtick is valid in kernel type names such as `int or struct `foo.
        // In ProgramOuterScope, backtick-led names are not probe specifier starts,
        // so they lex as Identifier.
        let input = "`foo";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Identifier);
        assert_eq!(str_from_source(input, token.origin), "`foo");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_identifier_backtick_in_clause() {
        // In InsideClauseAndExpr backtick forms part of an identifier.
        let input = "foo`bar";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Identifier);
        assert_eq!(str_from_source(input, token.origin), "foo`bar");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_probe_specifier_backtick() {
        // Backtick in probe specifier continuation (e.g. kernel`malloc).
        let input = "fbt:kernel`module:malloc:entry {}";
        let mut lexer = Lexer::new(FILE_ID, input);
        {
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::ProbeSpecifier);
            assert_eq!(
                str_from_source(input, token.origin),
                "fbt:kernel`module:malloc:entry"
            );
        }
        assert_eq!(lexer.lex().kind, TokenKind::LeftCurly);
        assert_eq!(lexer.lex().kind, TokenKind::RightCurly);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_escape_a() {
        let input = r#"'\a'"#;
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter(7)); // \a = BEL
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_escape_f() {
        let input = r#"'\f'"#;
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter(12)); // \f = FF
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_escape_n() {
        let input = r#"'\n'"#;
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter(10)); // \n = LF
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_escape_t() {
        let input = r#"'\t'"#;
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter(9)); // \t = TAB
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_escape_v() {
        let input = r#"'\v'"#;
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter(11)); // \v = VT
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_escape_backslash() {
        let input = r#"'\\'"#;
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter(92)); // '\\' = backslash = 0x5C
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_literal_newline() {
        // A bare newline inside a char literal is an error; lexing recovers and collects 'a'.
        let input = "'\na'";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter('a' as isize));
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidLiteralCharacter);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_character_literal_backslash_newline() {
        // A backslash+newline inside a char literal is an error; lexing recovers and collects 'a'.
        let input = "'\\\na'";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter('a' as isize));
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidLiteralCharacter);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_character_literal_unterminated() {
        let input = "'a";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter('a' as isize));
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidLiteralCharacter);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_character_literal_too_long() {
        // 9 bytes exceeds the 8-byte limit; an error is recorded and value is 0.
        let input = "'abcdefghi'";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter(0));
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidLiteralCharacter);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_literal_number_hex_empty() {
        let input = "0x";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), "0x");
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidLiteralNumber);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_literal_number_float() {
        // `skip_until_end_of_float` consumes the fractional digits too;
        // the whole "1.5" is a single `LiteralNumber` token.
        let input = "1.5";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), "1.5");
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(
            lexer.errors[0].kind,
            ErrorKind::UnsupportedLiteralFloatNumber
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_float_with_exponent() {
        // `1.5e3` — fraction + exponent, no sign.
        let input = "1.5e3+";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), "1.5e3");
        assert_eq!(lexer.errors.len(), 1);
        assert_eq!(
            lexer.errors[0].kind,
            ErrorKind::UnsupportedLiteralFloatNumber
        );
        assert_eq!(lexer.lex().kind, TokenKind::Plus);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(
            lexer.errors.len(),
            1,
            "no new errors after remaining tokens"
        );
    }

    #[test]
    fn test_lex_float_with_positive_exponent() {
        // `1.5e+3` — fraction + exponent with explicit positive sign.
        let input = "1.5e+3;";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), "1.5e+3");
        assert_eq!(lexer.errors.len(), 1);
        assert_eq!(
            lexer.errors[0].kind,
            ErrorKind::UnsupportedLiteralFloatNumber
        );
        assert_eq!(lexer.lex().kind, TokenKind::SemiColon);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(
            lexer.errors.len(),
            1,
            "no new errors after remaining tokens"
        );
    }

    #[test]
    fn test_lex_float_with_negative_exponent() {
        // `1.5e-3` — fraction + exponent with negative sign.
        let input = "1.5e-3;";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), "1.5e-3");
        assert_eq!(lexer.errors.len(), 1);
        assert_eq!(
            lexer.errors[0].kind,
            ErrorKind::UnsupportedLiteralFloatNumber
        );
        assert_eq!(lexer.lex().kind, TokenKind::SemiColon);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(
            lexer.errors.len(),
            1,
            "no new errors after remaining tokens"
        );
    }

    #[test]
    fn test_lex_float_with_suffix_f() {
        // `1.5f` — fraction + `f` type suffix, no exponent.
        let input = "1.5f;";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), "1.5f");
        assert_eq!(lexer.errors.len(), 1);
        assert_eq!(
            lexer.errors[0].kind,
            ErrorKind::UnsupportedLiteralFloatNumber
        );
        assert_eq!(lexer.lex().kind, TokenKind::SemiColon);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(
            lexer.errors.len(),
            1,
            "no new errors after remaining tokens"
        );
    }

    #[test]
    fn test_lex_float_with_suffix_l() {
        // `1.5L` — fraction + `L` type suffix, no exponent.
        let input = "1.5L;";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), "1.5L");
        assert_eq!(lexer.errors.len(), 1);
        assert_eq!(
            lexer.errors[0].kind,
            ErrorKind::UnsupportedLiteralFloatNumber
        );
        assert_eq!(lexer.lex().kind, TokenKind::SemiColon);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(
            lexer.errors.len(),
            1,
            "no new errors after remaining tokens"
        );
    }

    #[test]
    fn test_lex_float_with_exponent_and_suffix() {
        // `1.5e3f` — fraction + exponent + suffix.
        let input = "1.5e3f;";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), "1.5e3f");
        assert_eq!(lexer.errors.len(), 1);
        assert_eq!(
            lexer.errors[0].kind,
            ErrorKind::UnsupportedLiteralFloatNumber
        );
        assert_eq!(lexer.lex().kind, TokenKind::SemiColon);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(
            lexer.errors.len(),
            1,
            "no new errors after remaining tokens"
        );
    }

    #[test]
    fn test_lex_float_trailing_dot_only() {
        // `1.` — trailing dot with no fraction, exponent, or suffix.
        let input = "1.;";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), "1.");
        assert_eq!(lexer.errors.len(), 1);
        assert_eq!(
            lexer.errors[0].kind,
            ErrorKind::UnsupportedLiteralFloatNumber
        );
        assert_eq!(lexer.lex().kind, TokenKind::SemiColon);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(
            lexer.errors.len(),
            1,
            "no new errors after remaining tokens"
        );
    }

    #[test]
    fn test_lex_float_dot_with_exponent_only() {
        // `1.e5` — dot immediately followed by exponent, no fractional digits.
        let input = "1.e5;";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), "1.e5");
        assert_eq!(lexer.errors.len(), 1);
        assert_eq!(
            lexer.errors[0].kind,
            ErrorKind::UnsupportedLiteralFloatNumber
        );
        assert_eq!(lexer.lex().kind, TokenKind::SemiColon);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(
            lexer.errors.len(),
            1,
            "no new errors after remaining tokens"
        );
    }

    #[test]
    fn test_lex_literal_number_leading_zero() {
        // Leading zero makes this an octal literal: `05` = 5 decimal.
        let input = "05";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(5));
        assert_eq!(str_from_source(input, token.origin), "05");
        assert!(lexer.errors.is_empty(), "unexpected errors: {:?}", lexer.errors);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(lexer.errors.is_empty(), "unexpected errors after Eof: {:?}", lexer.errors);
    }

    #[test]
    fn test_lex_literal_number_suffix_u() {
        for input in ["42u", "42U"] {
            let mut lexer = Lexer::new(FILE_ID, input);
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralNumber(42), "input={input}");
            assert_eq!(str_from_source(input, token.origin), input, "input={input}");
            assert_eq!(lexer.lex().kind, TokenKind::Eof);
            assert!(
                lexer.errors.is_empty(),
                "unexpected errors for {input}: {:?}",
                lexer.errors
            );
        }
    }

    #[test]
    fn test_lex_literal_number_suffix_ul() {
        for input in ["42ul", "42UL", "42uL", "42Ul"] {
            let mut lexer = Lexer::new(FILE_ID, input);
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralNumber(42), "input={input}");
            assert_eq!(str_from_source(input, token.origin), input, "input={input}");
            assert_eq!(lexer.lex().kind, TokenKind::Eof);
            assert!(
                lexer.errors.is_empty(),
                "unexpected errors for {input}: {:?}",
                lexer.errors
            );
        }
    }

    #[test]
    fn test_lex_literal_number_suffix_ull() {
        for input in ["42ull", "42ULL", "42uLL", "42lL", "42LL"] {
            let mut lexer = Lexer::new(FILE_ID, input);
            let token = lexer.lex();
            assert_eq!(token.kind, TokenKind::LiteralNumber(42), "input={input}");
            assert_eq!(str_from_source(input, token.origin), input, "input={input}");
            assert_eq!(lexer.lex().kind, TokenKind::Eof);
            assert!(
                lexer.errors.is_empty(),
                "unexpected errors for {input}: {:?}",
                lexer.errors
            );
        }
    }

    #[test]
    fn test_lex_string_literal_backslash_at_eof() {
        // Backslash immediately before EOF (no closing quote): the backslash hits the
        // catch-all arm, then EOF fires the unterminated-string error.
        let input = "\"hello\\";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralString);
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidLiteralString);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_shebang() {
        let input = "#!/usr/bin/dtrace\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex(); // Shebang is consumed; lexer recurses and returns '+'.
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.control_directives.len(), 1);
        // The `#!` prefix is stripped; only the interpreter path is stored.
        assert_eq!(
            lexer.control_directives[0].kind,
            ControlDirectiveKind::Shebang(String::from("/usr/bin/dtrace"))
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_shebang_not_first() {
        // Non-horizontal-whitespace before `#!` on the same line triggers an error.
        // A shebang on a later line (with nothing before it on that line) is fine.
        let input = "foo#!/usr/bin/dtrace\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let _foo = lexer.lex(); // `foo` is lexed first as a probe specifier token.
        let token = lexer.lex(); // `#!` triggers error, shebang is consumed, lexer recurses to '+'.
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::ShebangMustComeFirst);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_float_dot_start() {
        // ".5" is a float literal starting with a dot, which is unsupported.
        let input = ".5";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), ".5");
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(
            lexer.errors[0].kind,
            ErrorKind::UnsupportedLiteralFloatNumber
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_dot_in_clause() {
        // A bare '.' in expression context is an `UnexpectedPeriod` error.
        let input = ".";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Dot);
        assert_eq!(str_from_source(input, token.origin), ".");
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::UnexpectedPeriod);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_slash_in_outer_scope() {
        // A lone '/' in outer scope produces a `Slash` token (used as predicate delimiter).
        let input = "/";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Slash);
        assert_eq!(str_from_source(input, token.origin), "/");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_macro_argument_reference_single_dollar() {
        let input = "$1";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::MacroArgumentReference(Some(1)));
        assert_eq!(str_from_source(input, token.origin), "$1");
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_character_literal_escape_r() {
        // '\r' is carriage return (0x0D = 13).
        let input = r#"'\r'"#;
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter(13));
        assert_eq!(str_from_source(input, token.origin), input);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_character_literal_escape_hex_empty() {
        // '\x' with no hex digits is an error; the char literal is also empty so a second error fires.
        let input = r#"'\x'"#;
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter(0));
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.errors.len(), 2, "expected 2 errors");
        assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidLiteralCharacter);
        assert_eq!(lexer.errors[1].kind, ErrorKind::InvalidLiteralCharacter);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 2, "no new errors after Eof");
    }

    #[test]
    fn test_lex_character_literal_escape_hex_overflow() {
        // '\xfff' has 3 hex digits; `u8::from_str_radix("fff", 16)` fails → error, value 0.
        let input = r#"'\xfff'"#;
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralCharacter(0));
        assert_eq!(str_from_source(input, token.origin), input);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidLiteralCharacter);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_literal_number_hex_trailing_nonhex() {
        // "0x1a+" — the '+' is not a hex digit, so the hex loop breaks normally.
        let input = "0x1a+";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0x1a));
        assert_eq!(str_from_source(input, token.origin), "0x1a");
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        let token2 = lexer.lex();
        assert_eq!(token2.kind, TokenKind::Plus);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_control_directive_pragma_option() {
        // `#pragma D option quiet` — key only, no value.
        let input = "#pragma D option quiet\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex(); // Directive is consumed; lexer recurses and returns '+'.
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.control_directives.len(), 1);
        assert_eq!(
            lexer.control_directives[0].kind,
            ControlDirectiveKind::PragmaOption(String::from("quiet"), None)
        );
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_control_directive_pragma_option_key_value() {
        // `#pragma D option bufsize=4m` — key=value pair.
        let input = "#pragma D option bufsize=4m\n";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.lex(); // Directive consumed, returns Eof.
        assert_eq!(lexer.control_directives.len(), 1);
        assert_eq!(
            lexer.control_directives[0].kind,
            ControlDirectiveKind::PragmaOption(String::from("bufsize"), Some(String::from("4m")))
        );
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_control_directive_pragma_error() {
        // `#pragma D error some message` — records a `PragmaError`.
        let input = "#pragma D error something went wrong\n";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.lex();
        assert_eq!(lexer.control_directives.len(), 1);
        assert!(
            matches!(
                &lexer.control_directives[0].kind,
                ControlDirectiveKind::PragmaError(_)
            ),
            "expected PragmaError, got {:?}",
            lexer.control_directives[0].kind
        );
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_control_directive_line_number() {
        // `#line 42` — sets the source line to 42.
        let input = "#line 42\n";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.lex();
        assert_eq!(lexer.control_directives.len(), 1);
        assert_eq!(
            lexer.control_directives[0].kind,
            ControlDirectiveKind::Line(42, None, None)
        );
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_control_directive_line_number_with_file() {
        // `#line 10 "foo.d"` — sets line and filename.
        let input = "#line 10 \"foo.d\"\n";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.lex();
        assert_eq!(lexer.control_directives.len(), 1);
        assert_eq!(
            lexer.control_directives[0].kind,
            ControlDirectiveKind::Line(10, Some(String::from("foo.d")), None)
        );
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_control_directive_pragma_depends_on() {
        // `#pragma D depends_on provider dtrace` — records a `PragmaDependsOn`.
        let input = "#pragma D depends_on provider dtrace\n";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.lex();
        assert_eq!(lexer.control_directives.len(), 1);
        assert_eq!(
            lexer.control_directives[0].kind,
            ControlDirectiveKind::PragmaDependsOn(
                PragmaDependsOnKind::Provider,
                String::from("dtrace")
            )
        );
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_control_directive_ignored() {
        // A bare `#pragma` with no further tokens is silently ignored.
        let input = "#pragma\n";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.lex();
        assert_eq!(lexer.control_directives.len(), 1);
        assert_eq!(
            lexer.control_directives[0].kind,
            ControlDirectiveKind::Ignored
        );
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_control_directive_null_directive() {
        // A bare `#` with no tokens is a null directive and is silently ignored (K&R A12.9).
        let input = "#\n";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.lex();
        assert_eq!(lexer.control_directives.len(), 1);
        assert_eq!(
            lexer.control_directives[0].kind,
            ControlDirectiveKind::Ignored
        );
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_control_directive_invalid() {
        // An unrecognised directive keyword produces an `InvalidControlDirective` error.
        let input = "#badkeyword\n";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.lex();
        assert_eq!(lexer.control_directives.len(), 0);
        assert_eq!(lexer.errors.len(), 1, "expected 1 error");
        assert_eq!(lexer.errors[0].kind, ErrorKind::InvalidControlDirective);
    }

    #[test]
    fn test_lex_attribute_basic() {
        // A bare `__attribute__((unused));` on its own line is silently skipped.
        let input = "__attribute__((unused));\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.attributes.len(), 1);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_attribute_with_whitespace_before_parens() {
        // Whitespace between `__attribute__` and `((` is allowed.
        let input = "__attribute__   ((noreturn));\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.attributes.len(), 1);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_attribute_complex_content() {
        // Content inside `(( ... ))` may be arbitrary (no newlines).
        let input = "__attribute__((format(printf, 1, 2)));\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.attributes.len(), 1);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_attribute_followed_by_more_on_same_line() {
        // Any tokens after `));` on the same line are skipped too
        // (`skip_until_exclusive('\n')` discards them).
        let input = "__attribute__((unused)); int x;\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.attributes.len(), 1);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_attribute_inside_clause() {
        // The rule also fires in `InsideClauseAndExpr` state (column 1 required).
        let input = "__attribute__((unused));\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.attributes.len(), 1);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_attribute_line_col1_no_semicolon() {
        // `__attribute__((foo))` at column 1 without a terminating `;`:
        // `lex_attribute_line` fails (requires `));`), but `lex_attribute`
        // (the `RGX_ATTR` rule) must still match and consume it.
        let input = "__attribute__((foo))+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.attributes.len(), 1);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_attribute_no_double_paren_falls_back() {
        // `__attribute__` at column 1 but without `((` is not a valid attribute;
        // the lexer rolls back and lexes it as a probe specifier instead.
        let input = "__attribute__\n";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::ProbeSpecifier);
        assert_eq!(str_from_source(input, token.origin), "__attribute__");
        assert_eq!(lexer.attributes.len(), 0);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_attribute_unterminated_falls_back() {
        // `__attribute__((` with no matching `));` before end-of-line: rollback,
        // lex as a probe specifier.
        let input = "__attribute__((no closing\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::ProbeSpecifier);
        assert_eq!(lexer.attributes.len(), 0);
    }

    // Tests for the inline form: `__attribute__(( ... ))` — no semicolon, any column.

    #[test]
    fn test_lex_attribute_inline_basic() {
        // Inline `__attribute__((unused))` not at column 1 is skipped; the next
        // token on the same line is returned.
        let input = " __attribute__((unused))+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.attributes.len(), 1);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_attribute_inline_with_whitespace_before_parens() {
        // Whitespace between `__attribute__` and `((` is allowed.
        let input = " __attribute__   ((packed))+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.attributes.len(), 1);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_attribute_inline_complex_content() {
        // Content inside `(( ... ))` may include identifiers, numbers, and commas.
        // Note: nested parentheses inside `((...))` cause early termination at the
        // first `))` seen, so this test uses content without nested parens.
        let input = " __attribute__((deprecated, \"reason\"))+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.attributes.len(), 1);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_attribute_inline_in_clause_state() {
        // The inline rule fires in `InsideClauseAndExpr` too.
        let input = " __attribute__((packed))+";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.attributes.len(), 1);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_attribute_inline_no_double_paren_falls_back() {
        // `__attribute__` not followed by `((` rolls back and lexes as an identifier.
        let input = " __attribute__+";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Identifier);
        assert_eq!(str_from_source(input, token.origin), "__attribute__");
        assert_eq!(lexer.attributes.len(), 0);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_attribute_inline_unterminated_falls_back() {
        // No `))` before end-of-line: rollback, lex as identifier.
        let input = " __attribute__((no closing\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Identifier);
        assert_eq!(str_from_source(input, token.origin), "__attribute__");
        assert_eq!(lexer.attributes.len(), 0);
    }

    #[test]
    fn test_lex_attribute_inline_origin_spans_directive() {
        // The stored `Attribute` origin covers `__attribute__((unused))` exactly,
        // not including the leading space or any tokens after.
        let input = " __attribute__((unused))+";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.lex();
        assert_eq!(lexer.attributes.len(), 1);
        assert_eq!(
            str_from_source(input, lexer.attributes[0].origin),
            "__attribute__((unused))"
        );
    }

    #[test]
    fn test_lex_attribute_not_at_column_one() {
        // `__attribute__((unused))` preceded by whitespace is not at column 1,
        // so the column-1 / semicolon form does not fire — but the inline form
        // fires and consumes it. The `;` that follows is the next token.
        let input = " __attribute__((unused));\n";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::SemiColon);
        assert_eq!(lexer.attributes.len(), 1);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
    }

    #[test]
    fn test_lex_attribute_origin_spans_full_directive() {
        // The stored `Attribute` origin covers from `__attribute__` through `));`.
        let input = "__attribute__((unused));\n";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.lex();
        assert_eq!(lexer.attributes.len(), 1);
        let attr_src = str_from_source(input, lexer.attributes[0].origin);
        assert_eq!(attr_src, "__attribute__((unused));");
    }

    #[test]
    fn test_lex_shebang_leading_whitespace() {
        // The official `RGX_INTERP` allows leading horizontal whitespace before
        // `#!`. The Rust lexer checks that all previous characters on the same
        // line are whitespace, so `\t#!/usr/bin/dtrace` should be accepted.
        let input = "\t#!/usr/bin/dtrace\n+";
        let mut lexer = Lexer::new(FILE_ID, input);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Plus);
        assert_eq!(lexer.control_directives.len(), 1);
        assert!(
            lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            lexer.errors
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_nul_byte() {
        // In Rust, `\0` is a valid `char`; it does NOT terminate the input.
        // It falls through to the `Unknown` arm, unlike C where NUL ends a string.
        let input = "\0+";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::Unknown(Some('\0')));
        assert_eq!(lexer.lex().kind, TokenKind::Plus);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_exponent_only_float() {
        // `1e5` matches `RGX_FP` in the official lexer (exponent without dot).
        // The whole `1e5` is a single `LiteralNumber` token with an error.
        let input = "1e5";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), "1e5");
        assert_eq!(lexer.errors.len(), 1);
        assert_eq!(
            lexer.errors[0].kind,
            ErrorKind::UnsupportedLiteralFloatNumber
        );
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(lexer.errors.len(), 1, "no new errors after Eof");
    }

    #[test]
    fn test_lex_exponent_float_uppercase() {
        // `1E+3` — uppercase `E` with explicit positive sign.
        let input = "1E+3;";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(LexerState::InsideClauseAndExpr);
        let token = lexer.lex();
        assert_eq!(token.kind, TokenKind::LiteralNumber(0));
        assert_eq!(str_from_source(input, token.origin), "1E+3");
        assert_eq!(lexer.errors.len(), 1);
        assert_eq!(
            lexer.errors[0].kind,
            ErrorKind::UnsupportedLiteralFloatNumber
        );
        assert_eq!(lexer.lex().kind, TokenKind::SemiColon);
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert_eq!(
            lexer.errors.len(),
            1,
            "no new errors after remaining tokens"
        );
    }

    #[test]
    fn test_lex_number_decimal_value() {
        // Plain decimal literal: `42` → value 42.
        let input = "42";
        let mut lexer = Lexer::new(FILE_ID, input);
        assert_eq!(lexer.lex().kind, TokenKind::LiteralNumber(42));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(lexer.errors.is_empty());
    }

    #[test]
    fn test_lex_number_octal_value() {
        // DTrace follows C convention: a leading zero means octal.
        // `034` = 3*8 + 4 = 28 decimal.
        let input = "034";
        let mut lexer = Lexer::new(FILE_ID, input);
        assert_eq!(lexer.lex().kind, TokenKind::LiteralNumber(28));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(lexer.errors.is_empty());
    }

    #[test]
    fn test_lex_number_octal_large_value() {
        // `0400` = 256 decimal.
        let input = "0400";
        let mut lexer = Lexer::new(FILE_ID, input);
        assert_eq!(lexer.lex().kind, TokenKind::LiteralNumber(256));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(lexer.errors.is_empty());
    }

    #[test]
    fn test_lex_number_zero_value() {
        // Bare `0` is decimal zero, not treated as octal (no further digits).
        let input = "0";
        let mut lexer = Lexer::new(FILE_ID, input);
        assert_eq!(lexer.lex().kind, TokenKind::LiteralNumber(0));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(lexer.errors.is_empty());
    }

    #[test]
    fn test_lex_number_hex_lowercase_value() {
        // `0x1c` = 28 decimal.
        let input = "0x1c";
        let mut lexer = Lexer::new(FILE_ID, input);
        assert_eq!(lexer.lex().kind, TokenKind::LiteralNumber(0x1c));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(lexer.errors.is_empty());
    }

    #[test]
    fn test_lex_number_hex_uppercase_prefix_value() {
        // `0X1C` — uppercase `X` prefix is also accepted.
        let input = "0X1C";
        let mut lexer = Lexer::new(FILE_ID, input);
        assert_eq!(lexer.lex().kind, TokenKind::LiteralNumber(0x1c));
        assert_eq!(lexer.lex().kind, TokenKind::Eof);
        assert!(lexer.errors.is_empty());
    }

    #[test]
    fn test_lex_number_suffix_strips_value() {
        // Suffixes `u`/`U`/`l`/`L`/`ll`/`LL` are consumed but do not affect the value.
        for (input, expected) in [
            ("42u", 42u64),
            ("42U", 42),
            ("100UL", 100),
            ("7ll", 7),
            ("7LL", 7),
        ] {
            let mut lexer = Lexer::new(FILE_ID, input);
            assert_eq!(
                lexer.lex().kind,
                TokenKind::LiteralNumber(expected),
                "input={input}"
            );
            assert_eq!(lexer.lex().kind, TokenKind::Eof);
            assert!(lexer.errors.is_empty(), "input={input}");
        }
    }
}
