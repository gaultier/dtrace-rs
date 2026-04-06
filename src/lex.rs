use serde::Serialize;
use std::{iter::Peekable, str::Chars};

use crate::{
    error::{Error, ErrorKind},
    origin::{FileId, Origin},
};

const STABILITY_POSSIBLE_VALUES: &str =
    "Internal, Private, Obsolete, External, Unstable, Evolving, Stable, Standard";

const CLASS_POSSIBLE_VALUES: &str = "Cpu, Platform, Group, Isa, Common";

const DEPENDS_ON_POSSIBLE_VALUES: &str = "provider, module, library";

#[derive(PartialEq, Eq, Debug)]
enum LexerState {
    Default,
    InsideControlDirective(u32 /* line */),
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
    Option(String, Option<String>),
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

#[derive(Debug)]
pub struct Lexer {
    origin: Origin,
    error_mode: bool,
    pub errors: Vec<Error>,
    pub tokens: Vec<Token>,
    state: LexerState,
    pub control_directives: Vec<ControlDirective>,
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
    Unknown,
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
            kind: TokenKind::Unknown,
            origin: Origin::new_unknown(),
        }
    }
}

fn peek2(it: &Peekable<Chars<'_>>) -> Option<char> {
    let mut cpy = it.clone();
    cpy.next().and_then(|_| cpy.next())
}

fn peek3(it: &Peekable<Chars<'_>>) -> (Option<char>, Option<char>) {
    let mut cpy = it.clone();
    let c1 = cpy.next().and_then(|_| cpy.next());
    let c2 = cpy.next();
    (c1, c2)
}

fn is_character_probe_specifier_start(c: char) -> bool {
    matches!(c ,
   '-' | '<' | '>' | '+' | '$' | ':' | 'a'..='z'  |  'A'..='Z' | '_' | '.' | '?' | '*' | '\\' | '[' | ']' | '!')
}

fn is_character_probe_specifier_rest(c: char) -> bool {
    matches!(c ,
   '-' | '<' | '>' | '+' | '$' | ':' | '0'..='9' | 'a'..='z'  |  'A'..='Z' | '_' | '.' | '?' | '*' | '\\' | '[' | ']' | '!' | '(' | ')' )
}

impl Lexer {
    pub fn new(file_id: FileId) -> Self {
        Self {
            origin: Origin::new(1, 1, 0, 0, file_id),
            error_mode: false,
            errors: Vec::new(),
            tokens: Vec::new(),
            state: LexerState::Default,
            control_directives: Vec::new(),
        }
    }

    fn add_error(&mut self, kind: ErrorKind, len: u32) {
        let origin = Origin { len, ..self.origin };
        self.errors.push(Error::new(kind, origin, String::new()));
        self.error_mode = true;
    }

    fn add_error_at(&mut self, kind: ErrorKind, origin: Origin) {
        self.errors.push(Error::new(kind, origin, String::new()));
        self.error_mode = true;
    }

    fn is_identifier_character_trailing(&self, c: char) -> bool {
        match self.state {
            LexerState::Default => c.is_alphanumeric() || c == '_',
            LexerState::InsideControlDirective(_) => !(c.is_whitespace() || c == '"'),
        }
    }

    fn is_identifier_character_leading(&self, c: char) -> bool {
        match self.state {
            LexerState::Default => c.is_alphanumeric() || c == '_' || c == '@',
            LexerState::InsideControlDirective(_) => !(c.is_whitespace() || c == '"'),
        }
    }

    fn lex_keyword(&mut self, input: &str, it: &mut Peekable<Chars<'_>>) {
        let start_origin = self.origin;
        let first = it.next().unwrap();
        assert!(self.is_identifier_character_leading(first));
        self.origin.column += 1;
        self.origin.offset += 1;

        while let Some(c) = it.peek() {
            if !self.is_identifier_character_trailing(*c) {
                break;
            }

            self.advance(*c, it);
        }

        let len = self.origin.offset - start_origin.offset;
        let origin = Origin {
            len,
            ..start_origin
        };

        let lit = &input[origin.offset as usize..origin.offset as usize + len as usize];
        let kind = match lit {
            "auto" => TokenKind::KeywordAuto,
            "break" => TokenKind::KeywordBreak,
            "case" => TokenKind::KeywordCase,
            "char" => TokenKind::KeywordChar,
            "const" => TokenKind::KeywordConst,
            "continue" => TokenKind::KeywordContinue,
            "counter" => TokenKind::KeywordCounter,
            "default" => TokenKind::KeywordDefault,
            "do" => TokenKind::KeywordDo,
            "double" => TokenKind::KeywordDouble,
            "else" => TokenKind::KeywordElse,
            "enum" => TokenKind::KeywordEnum,
            "extern" => TokenKind::KeywordExtern,
            "float" => TokenKind::KeywordFloat,
            "for" => TokenKind::KeywordFor,
            "goto" => TokenKind::KeywordGoto,
            "if" => TokenKind::KeywordIf,
            "import" => TokenKind::KeywordImport,
            "inline" => TokenKind::KeywordInline,
            "int" => TokenKind::KeywordInt,
            "long" => TokenKind::KeywordLong,
            "offsetof" => TokenKind::KeywordOffsetOf,
            "probe" => TokenKind::KeywordProbe,
            "provider" => TokenKind::KeywordProvider,
            "register" => TokenKind::KeywordRegister,
            "restrict" => TokenKind::KeywordRestrict,
            "return" => TokenKind::KeywordReturn,
            "self" => TokenKind::KeywordSelf,
            "short" => TokenKind::KeywordShort,
            "signed" => TokenKind::KeywordSigned,
            "sizeof" => TokenKind::KeywordSizeof,
            "static" => TokenKind::KeywordStatic,
            "string" => TokenKind::KeywordString,
            "stringof" => TokenKind::KeywordStringof,
            "struct" => TokenKind::KeywordStruct,
            "switch" => TokenKind::KeywordSwitch,
            "this" => TokenKind::KeywordThis,
            "translator" => TokenKind::KeywordTranslator,
            "typedef" => TokenKind::KeywordTypedef,
            "union" => TokenKind::KeywordUnion,
            "unsigned" => TokenKind::KeywordUnsigned,
            "userland" => TokenKind::KeywordUserland,
            "void" => TokenKind::KeywordVoid,
            "volatile" => TokenKind::KeywordVolatile,
            "while" => TokenKind::KeywordWhile,
            "xlate" => TokenKind::KeywordXlate,
            _ => TokenKind::Identifier,
        };

        self.tokens.push(Token { kind, origin });
    }

    fn lex_probe_specifier(&mut self, it: &mut Peekable<Chars<'_>>) {
        let start_origin = self.origin;
        let first = it.next().unwrap();
        assert!(is_character_probe_specifier_start(first));
        self.origin.column += 1;
        self.origin.offset += 1;

        loop {
            match it.peek() {
                None => {
                    break;
                }
                Some(c) if !is_character_probe_specifier_rest(*c) => {
                    break;
                }
                Some(c) => {
                    self.advance(*c, it);
                }
            }
        }

        let len = self.origin.offset - start_origin.offset;
        let origin = Origin {
            len,
            ..start_origin
        };

        self.tokens.push(Token {
            kind: TokenKind::ProbeSpecifier,
            origin,
        });
    }

    fn lex_literal_string(&mut self, it: &mut Peekable<Chars<'_>>) {
        let start_origin = self.origin;
        let first = it.next().unwrap();
        assert_eq!(first, '"');
        self.origin.column += 1;
        self.origin.offset += 1;

        loop {
            match it.peek() {
                Some(c @ '"') => {
                    self.advance(*c, it);
                    break;
                }
                Some('\n') | None => {
                    self.add_error_at(ErrorKind::InvalidLiteralString, self.origin);
                    return;
                }
                Some(c) => {
                    self.advance(*c, it);
                }
            }
        }

        let len = self.origin.offset - start_origin.offset;
        let origin = Origin {
            len,
            ..start_origin
        };

        self.tokens.push(Token {
            kind: TokenKind::LiteralString,
            origin,
        });
    }

    fn lex_literal_character(&mut self, it: &mut Peekable<Chars<'_>>) {
        let start_origin = self.origin;
        let first = it.next().unwrap();
        assert_eq!(first, '\'');
        self.origin.column += 1;
        self.origin.offset += 1;

        loop {
            match it.peek() {
                Some(c @ '\'') => {
                    self.advance(*c, it);
                    break;
                }
                Some('\n') | None => {
                    self.add_error_at(ErrorKind::InvalidLiteralCharacter, self.origin);
                    return;
                }
                Some(c) => {
                    self.advance(*c, it);
                }
            }
        }

        let len = self.origin.offset - start_origin.offset;
        // TODO: Limit length.
        let origin = Origin {
            len,
            ..start_origin
        };

        self.tokens.push(Token {
            kind: TokenKind::LiteralCharacter,
            origin,
        });
    }

    fn lex_literal_number(&mut self, it: &mut Peekable<Chars<'_>>) {
        let start_origin = self.origin;
        let first = it.next().unwrap();
        assert!(first.is_ascii_digit());
        self.origin.column += 1;
        self.origin.offset += 1;

        while let Some(c) = it.peek() {
            if !c.is_ascii_digit() {
                break;
            }

            self.advance(*c, it);
        }

        let len = self.origin.offset - start_origin.offset;
        let origin = Origin {
            len,
            ..start_origin
        };

        if first == '0' && len > 1 {
            self.add_error_at(ErrorKind::InvalidLiteralNumber, origin);
            return;
        }

        self.tokens.push(Token {
            kind: TokenKind::LiteralNumber,
            origin,
        });
    }

    fn advance(&mut self, c: char, it: &mut Peekable<Chars>) {
        self.origin.offset += c.len_utf8() as u32;

        if c == '\n' {
            self.origin.column = 1;
            self.origin.line += 1;
        } else {
            self.origin.column += 1;
        }
        it.next();
    }

    pub fn lex(&mut self, input: &str) {
        let mut it = input.chars().peekable();

        while let Some(c) = it.peek() {
            let c = *c;
            if c != '\n' && self.error_mode {
                self.origin.column += 1;
                self.origin.offset += 1;
                it.next();
                continue;
            }
            match c {
                '\n' => {
                    if let LexerState::InsideControlDirective(line) = self.state {
                        let tokens: Vec<Token> = self
                            .tokens
                            .extract_if(.., |tok| tok.origin.line == line)
                            .collect::<Vec<Token>>();
                        match self.control_directive(input, &tokens) {
                            Ok(directive) => self.control_directives.push(directive),
                            Err(err) => self.errors.push(err),
                        }
                        self.state = LexerState::Default;
                    }
                    self.advance(c, &mut it);
                }
                '#' if !self.has_any_previous_tokens_on_same_line() => {
                    self.state = LexerState::InsideControlDirective(self.origin.line);
                    self.advance(c, &mut it);
                }
                '-' if peek2(&it) == Some('-') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::MinusMinus,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '-' if peek2(&it) == Some('=') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::MinusEq,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '-' if peek2(&it) == Some('>') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Arrow,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '-' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Minus,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '+' if peek2(&it) == Some('+') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::PlusPlus,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '+' if peek2(&it) == Some('=') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::PlusEq,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '+' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Plus,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '.' if peek3(&it) == (Some('.'), Some('.')) => {
                    let origin = Origin {
                        len: 3,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::DotDotDot,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '.' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Dot,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '*' if peek2(&it) == Some('=') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::StarEq,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '*' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Star,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '>' if peek3(&it) == (Some('>'), Some('=')) => {
                    let origin = Origin {
                        len: 3,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::GtGtEq,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '>' if peek2(&it) == Some('>') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::GtGt,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '>' if peek2(&it) == Some('=') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::GtEq,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '>' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Gt,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '<' if peek3(&it) == (Some('<'), Some('=')) => {
                    let origin = Origin {
                        len: 3,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::LtLtEq,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '<' if peek2(&it) == Some('<') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::LtLt,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '<' if peek2(&it) == Some('=') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::LtEq,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '<' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Lt,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '^' if peek2(&it) == Some('=') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::CaretEq,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '^' if peek2(&it) == Some('^') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::CaretCaret,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '^' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Caret,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '&' if peek2(&it) == Some('=') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::AmpersandEq,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '&' if peek2(&it) == Some('&') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::AmpersandAmpersand,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '&' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Ampersand,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '?' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Question,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '|' if peek2(&it) == Some('=') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::PipeEq,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '|' if peek2(&it) == Some('|') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::PipePipe,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '|' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Pipe,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                ':' => {
                    if let Some(next) = it.peek()
                        && *next == '='
                    {
                        let origin = Origin {
                            len: 2,
                            ..self.origin
                        };
                        self.tokens.push(Token {
                            kind: TokenKind::ColonEq,
                            origin,
                        });
                        self.advance(c, &mut it);
                        self.advance(c, &mut it);
                    } else {
                        let origin = Origin {
                            len: 1,
                            ..self.origin
                        };
                        self.tokens.push(Token {
                            kind: TokenKind::Colon,
                            origin,
                        });
                        self.advance(c, &mut it);
                    }
                }
                '!' if peek2(&it) == Some('=') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::BangEq,
                        origin,
                    });
                    self.advance(c, &mut it);
                    self.advance(c, &mut it);
                }
                '!' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Bang,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '=' => {
                    let origin = self.origin;
                    self.advance(c, &mut it);
                    if let Some(next) = it.peek()
                        && *next == '='
                    {
                        self.tokens.push(Token {
                            kind: TokenKind::EqEq,
                            origin: Origin { len: 2, ..origin },
                        });
                        self.advance(c, &mut it);
                    } else {
                        self.tokens.push(Token {
                            kind: TokenKind::Eq,
                            origin: Origin { len: 1, ..origin },
                        });
                    }
                }
                '/' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Slash,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '%' if peek2(&it) == Some('=') => {
                    let origin = Origin {
                        len: 2,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::PercentEq,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '%' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Percent,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '~' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Tilde,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '{' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::LeftCurly,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '}' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::RightCurly,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '(' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::LeftParen,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                ')' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::RightParen,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                ',' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::Comma,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '[' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::LeftSquareBracket,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                ']' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::RightSquareBracket,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                ';' => {
                    let origin = Origin {
                        len: 1,
                        ..self.origin
                    };
                    self.tokens.push(Token {
                        kind: TokenKind::SemiColon,
                        origin,
                    });
                    self.advance(c, &mut it);
                }
                '"' => {
                    self.lex_literal_string(&mut it);
                }
                '\'' => {
                    self.lex_literal_character(&mut it);
                }
                _ if c.is_ascii_digit() => self.lex_literal_number(&mut it),
                _ if c.is_whitespace() => {
                    self.advance(c, &mut it);
                }
                _ if self.is_identifier_character_leading(c) => self.lex_keyword(input, &mut it),
                _ if is_character_probe_specifier_start(c)
                    && is_character_probe_specifier_rest(peek2(&it).unwrap_or_default()) =>
                {
                    self.lex_probe_specifier(&mut it)
                }
                _ => {
                    self.tokens.push(Token {
                        kind: TokenKind::Unknown,
                        origin: Origin {
                            len: 1,
                            ..self.origin
                        },
                    });

                    self.add_error(ErrorKind::UnknownToken, 1);
                    self.advance(c, &mut it);
                }
            }
        }
        let origin = Origin {
            len: 0,
            ..self.origin
        };
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            origin,
        });
    }

    fn has_any_previous_tokens_on_same_line(&self) -> bool {
        self.tokens
            .iter()
            .rev()
            .any(|tok| tok.origin.line == self.origin.line)
    }

    #[warn(unused_results)]
    fn control_directive(
        &mut self,
        input: &str,
        tokens: &[Token],
    ) -> Result<ControlDirective, Error> {
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
                ..
            }) => self.control_directive_line(tokens, tokens[0].origin, input),
            Some(
                tok @ Token {
                    kind: TokenKind::Identifier,
                    ..
                },
            ) => {
                let src = str_from_source(input, &tok.origin);
                match src {
                    "line" => self.control_directive_line(&tokens[1..], tokens[0].origin, input),
                    "pragma" if tokens.len() > 1 => {
                        self.control_directive_pragma(&tokens[1..], tokens[0].origin, input)
                    }
                    // Ignore any #ident or #pragma ident lines.
                    "pragma" if tokens.len() == 1 => Ok(ControlDirective {
                        kind: ControlDirectiveKind::Ignored,
                        origin: Origin::new_unknown(),
                    }),

                    "ident" => Ok(ControlDirective {
                        kind: ControlDirectiveKind::Ignored,
                        origin: Origin::new_unknown(),
                    }),
                    "error" => self.control_directive_error(&tokens[1..], input),
                    _ => Err(Error::new(
                        ErrorKind::InvalidControlDirective,
                        tok.origin,
                        String::new(),
                    )),
                }
            }
            Some(other) => Err(Error::new(
                ErrorKind::InvalidControlDirective,
                other.origin,
                String::new(),
            )),
        }
    }

    #[warn(unused_results)]
    fn control_directive_line(
        &mut self,
        tokens: &[Token],
        origin: Origin,
        input: &str,
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

        let line_src = str_from_source(input, &line.origin);
        let file_src = file.map(|f| {
            let s = str_from_source(input, &f.origin);
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
            match str::parse::<usize>(str_from_source(input, &trailing.origin)) {
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
            origin: line.origin,
            kind: ControlDirectiveKind::Line(line_num, file_src, trailing_num),
        })
    }

    #[warn(unused_results)]
    fn control_directive_pragma(
        &mut self,
        tokens: &[Token],
        origin: Origin,
        input: &str,
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
                Some(str_from_source(input, origin1)),
                Some(str_from_source(input, origin2)),
            ),
            (
                Some(Token {
                    kind: TokenKind::Identifier,
                    origin: origin1,
                }),
                _,
            ) => (Some(str_from_source(input, origin1)), None),
            _ => (None, None),
        };

        match (directive1, directive2) {
            // `#pragma error`, or  `#pragma D error`.
            (Some("D"), Some("error")) => self.control_directive_error(&tokens[2..], input),
            (Some("error"), _) => self.control_directive_error(&tokens[1..], input),

            // `#pragma line`.
            (Some("D"), Some("line")) => self.control_directive_line(&tokens[2..], origin, input),
            (Some("line"), _) => self.control_directive_line(&tokens[1..], origin, input),
            //
            // `#pragma depends_on`.
            (Some("D"), Some("depends_on")) => self.pragma_depends_on(&tokens[2..], origin, input),

            (Some("depends_on"), _) => self.pragma_depends_on(&tokens[1..], origin, input),

            // `#pragma attributes`.
            (Some("D"), Some("attributes")) => self.pragma_attributes(&tokens[2..], origin, input),
            (Some("attributes"), _) => self.pragma_attributes(&tokens[1..], origin, input),

            // `#pragma binding`.
            (Some("D"), Some("binding")) => self.pragma_binding(&tokens[2..], origin, input),
            (Some("binding"), _) => self.pragma_binding(&tokens[1..], origin, input),

            // `#pragma option`.
            (Some("D"), Some("option")) => self.pragma_option(&tokens[2..], origin, input),
            (Some("option"), _) => self.pragma_option(&tokens[1..], origin, input),

            // `#pragma`, `#pragma ident`,  `#pragma D ident`, or `#pragma someunknownstuff`: Ignore.
            _ => Ok(ControlDirective {
                kind: ControlDirectiveKind::Ignored,
                origin: Origin::new_unknown(),
            }),
        }
    }

    #[warn(unused_results)]
    fn control_directive_error(
        &mut self,
        tokens: &[Token],
        input: &str,
    ) -> Result<ControlDirective, Error> {
        let src = match (tokens.get(1), tokens.last()) {
            (Some(start), Some(end)) => input[start.origin.offset as usize
                ..end.origin.offset as usize + end.origin.len as usize]
                .to_owned(),
            _ => String::new(),
        };

        Ok(ControlDirective {
            origin: tokens[0].origin,
            kind: ControlDirectiveKind::PragmaError(src),
        })
    }

    #[warn(unused_results)]
    fn pragma_attributes(
        &self,
        tokens: &[Token],
        origin: Origin,
        input: &str,
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
                let s1 = str_from_source(input, origin1);
                let s2 = str_from_source(input, origin2);
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

        let origin = tokens[0].origin;
        let split: Vec<_> = s1.splitn(4, "/").collect();
        let (name_str, data_str, class_str, trailing) =
            (split.get(0), split.get(1), split.get(2), split.get(3));
        if let Some(trailing) = trailing {
            return Err(Error {
                kind: ErrorKind::InvalidControlDirective,
                origin: origin
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
                    origin: origin.with_len(s.len()),
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
                    origin: origin.skip(skip).with_len(s.len()),
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
                    origin: origin.skip(skip).with_len(s.len()),
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
            origin,
        })
    }

    #[warn(unused_results)]
    fn pragma_binding(
        &mut self,
        tokens: &[Token],
        origin: Origin,
        input: &str,
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
                let (version_str, origin) = quoted_string_from_source(input, origin1);
                let version = version_str2num(version_str, origin)?;
                let identifier = str_from_source(input, origin2).to_owned();

                Ok(ControlDirective {
                    origin: *origin1,
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
    fn pragma_option(
        &self,
        tokens: &[Token],
        origin: Origin,
        input: &str,
    ) -> Result<ControlDirective, Error> {
        match tokens {
            [
                Token {
                    kind: TokenKind::Identifier,
                    origin,
                },
            ] => {
                // TODO: Validate option key against a list of known values?

                let s = str_from_source(input, origin);
                if let Some((key, value)) = s.split_once('=') {
                    if value.contains('=') {
                        Err(Error {
                            kind: ErrorKind::InvalidControlDirective,
                            origin: *origin,
                            explanation: String::from(
                                "expected option of the form key=value, found additional equal sign",
                            ),
                        })
                    } else {
                        Ok(ControlDirective {
                            kind: ControlDirectiveKind::Option(
                                key.to_owned(),
                                Some(value.to_owned()),
                            ),
                            origin: *origin,
                        })
                    }
                } else {
                    Ok(ControlDirective {
                        kind: ControlDirectiveKind::Option(s.to_owned(), None),
                        origin: *origin,
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
        input: &str,
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
                let kind = str_from_source(input, origin1);
                let name = str_from_source(input, origin2);
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
mod tests {
    use super::*;

    #[test]
    fn lex_number() {
        let mut lexer = Lexer::new(0);
        lexer.lex("123 4567\n 01");

        assert_eq!(lexer.errors.len(), 1);
        assert_eq!(lexer.tokens.len(), 3);

        {
            let token = &lexer.tokens[0];
            assert_eq!(token.kind, TokenKind::LiteralNumber);
            assert_eq!(token.origin.offset, 0);
            assert_eq!(token.origin.line, 1);
            assert_eq!(token.origin.column, 1);
            assert_eq!(token.origin.len, 3);
        }
        {
            let token = &lexer.tokens[1];
            assert_eq!(token.kind, TokenKind::LiteralNumber);
            assert_eq!(token.origin.offset, 4);
            assert_eq!(token.origin.line, 1);
            assert_eq!(token.origin.column, 5);
            assert_eq!(token.origin.len, 4);
        }
        {
            let token = &lexer.tokens[2];
            assert_eq!(token.kind, TokenKind::Eof);
        }
        {
            let err = &lexer.errors[0];
            assert_eq!(err.kind, ErrorKind::InvalidLiteralNumber);
            assert_eq!(err.origin.offset, 10);
            assert_eq!(err.origin.line, 2);
            assert_eq!(err.origin.column, 2);
            assert_eq!(err.origin.len, 2);
        }
    }

    #[test]
    fn lex_unknown() {
        let mut lexer = Lexer::new(0);
        lexer.lex(" &");

        assert_eq!(lexer.tokens.len(), 2);
        assert_eq!(lexer.errors.len(), 1);

        {
            let token = &lexer.tokens[0];
            assert_eq!(token.kind, TokenKind::Unknown);
        }
        {
            let token = &lexer.tokens[1];
            assert_eq!(token.kind, TokenKind::Eof);
        }

        let err = &lexer.errors[0];
        assert_eq!(err.kind, ErrorKind::UnknownToken);
        assert_eq!(err.origin.offset, 1);
        assert_eq!(err.origin.line, 1);
        assert_eq!(err.origin.column, 2);
        assert_eq!(err.origin.len, 1);
    }
}
