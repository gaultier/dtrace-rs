use serde::Serialize;
use std::{iter::Peekable, str::Chars};

use crate::{
    error::{Error, ErrorKind},
    origin::{FileId, Origin},
};

#[derive(Debug)]
pub struct Lexer {
    origin: Origin,
    error_mode: bool,
    pub errors: Vec<Error>,
    pub tokens: Vec<Token>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Copy, Clone)]
pub enum TokenKind {
    LiteralNumber,
    LiteralString,
    LiteralCharacter,
    LiteralBool,
    Identifier,
    ProbeSpecifier,
    Dot,
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

    fn lex_keyword(&mut self, input: &str, it: &mut Peekable<Chars<'_>>) {
        let start_origin = self.origin;
        let first = it.next().unwrap();
        assert!(first.is_ascii_alphabetic() || first == '@');
        self.origin.column += 1;
        self.origin.offset += 1;

        while let Some(c) = it.peek() {
            if !(c.is_alphanumeric() || *c == '_') {
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
            "true" | "false" => TokenKind::LiteralBool,
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
            "offsetOf" => TokenKind::KeywordOffsetOf,
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
                '@' => self.lex_keyword(input, &mut it),
                _ if c.is_ascii_alphabetic() => self.lex_keyword(input, &mut it),
                _ if is_character_probe_specifier_start(c)
                    && is_character_probe_specifier_rest(peek2(&it).unwrap_or_default()) =>
                {
                    self.lex_probe_specifier(&mut it)
                }
                _ if c.is_whitespace() => {
                    self.advance(c, &mut it);
                }
                _ if c.is_ascii_digit() => self.lex_literal_number(&mut it),
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
