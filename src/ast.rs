use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    num::ParseIntError,
    ops::{Index, IndexMut},
};

use crate::{
    error::{Error, ErrorKind},
    lex::{Lexer, Token, TokenKind},
    origin::{FileId, Origin},
    type_checker::Type,
};
use log::trace;
use serde::Serialize;

// TODO: u32?
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct NodeId(pub(crate) usize);

#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
pub(crate) enum NodeKind {
    Unknown,
    // TODO: Should we just use 'Block'?
    Number(u64),
    PrimaryToken(TokenKind),
    Cast(String, NodeId),
    ProbeSpecifier(String),
    ProbeSpecifiers(Vec<NodeId>),
    ProbeDefinition(NodeId, Option<NodeId>, Option<NodeId>),
    BinaryOp(NodeId, Token, NodeId),
    Identifier(String),
    Aggregation(String),
    Unary(TokenKind, NodeId),
    Assignment(NodeId, Token, NodeId),
    Arguments(Vec<NodeId>),
    CommaExpr(Vec<NodeId>),
    SizeofType(String),
    SizeofExpr(NodeId),
    StringofExpr(NodeId),
    TranslationUnit(Vec<NodeId>),
    If {
        cond: NodeId,
        then_block: NodeId,
        else_block: Option<NodeId>,
    },
    Block(Vec<NodeId>),
    PostfixIncDecrement(NodeId, TokenKind),
    ExprStmt(NodeId),
    EmptyStmt,
    PostfixArguments(NodeId, Option<NodeId>),
    TernaryExpr(NodeId, NodeId, NodeId),
    PostfixArrayAccess(NodeId, Option<NodeId>),
    FieldAccess(NodeId, TokenKind, Token),
    TypeName(NodeId, Option<NodeId>),
    OffsetOf(NodeId, Token),
    Declaration(NodeId, Option<NodeId>),
    DeclarationSpecifiers(Vec<NodeId>),
    DirectDeclarator(NodeId, Option<NodeId>),
    Declarator(Option<NodeId>, NodeId),
    InitDeclarators(Vec<NodeId>),
    TypeQualifier(TokenKind),
    DStorageClassSpecifier(TokenKind),
    StorageClassSpecifier(TokenKind),
    TypeSpecifier(TokenKind),
    EnumDeclaration(Option<String>, Option<NodeId>),
    EnumeratorDeclaration(String, Option<NodeId>),
    EnumeratorsDeclaration(Vec<NodeId>),
    StructDeclaration(Option<String>, Option<NodeId>),
    StructFieldsDeclaration(Vec<NodeId>),
    StructFieldDeclarator(NodeId, Option<NodeId>),
    StructFieldDeclaration(NodeId, Option<NodeId>),
    StructFieldDeclaratorList(Vec<NodeId>),
    SpecifierQualifierList(Vec<NodeId>),
    Xlate(NodeId, NodeId),
    DirectAbstractDeclarator(NodeId),
    DirectAbstractArray(Option<NodeId>, NodeId),
    DirectAbstractFunction(Option<NodeId>, NodeId),
    AbstractDeclarator(Option<NodeId>, Option<NodeId>),
    Pointer(Vec<NodeId>, Option<NodeId>),
    Array(Option<NodeId>),
    ParamEllipsis,
    Parameters(Vec<NodeId>),
    ParameterDeclarationSpecifiers(Vec<NodeId>),
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    pub origin: Origin,
}

impl IndexMut<NodeId> for [Node] {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        &mut self[index.0]
    }
}

impl Index<NodeId> for [Node] {
    type Output = Node;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self[index.0]
    }
}

impl IndexMut<NodeId> for Vec<Node> {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        &mut self[index.0]
    }
}

impl Index<NodeId> for Vec<Node> {
    type Output = Node;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self[index.0]
    }
}

#[derive(Debug)]
pub struct NameToDef {
    scopes: Vec<HashMap<String, NodeId>>,
    definitive: HashMap<String, NodeId>,
}

pub struct Parser<'a> {
    error_mode: bool,
    pub tokens: Vec<Token>,
    tokens_consumed: usize,
    pub errors: Vec<Error>,
    input: &'a str,
    file_id_to_name: &'a HashMap<FileId, String>,
    pub(crate) nodes: Vec<Node>,
    pub(crate) node_to_type: HashMap<NodeId, Type>,
    pub(crate) name_to_def: NameToDef,
    pub(crate) typenames: HashSet<String>,
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum ScopeResolution {
    Current,
    Ancestor,
}

impl NameToDef {
    fn new() -> Self {
        Self {
            scopes: Vec::new(),
            definitive: HashMap::new(),
        }
    }

    pub(crate) fn get_scoped(&self, name: &str) -> Option<(&NodeId, ScopeResolution)> {
        self.scopes.iter().rev().enumerate().find_map(|(i, scope)| {
            scope.get(name).map(|node_id| {
                let scope = if i == 0 {
                    ScopeResolution::Current
                } else {
                    ScopeResolution::Ancestor
                };
                (node_id, scope)
            })
        })
    }

    pub(crate) fn get_definitive(&self, name: &str) -> Option<&NodeId> {
        self.definitive.get(name)
    }

    pub(crate) fn insert(&mut self, name: String, node_id: NodeId) {
        // Technically, a variable cannot be redeclared inside the same block, so
        // we could panic if there is already an entry.
        // However, we in this case:
        // 1. Record the error
        // 2. Override the existing entry
        // 3. Keep going to report further errors
        self.scopes
            .last_mut()
            .unwrap()
            .insert(name.to_owned(), node_id);
        self.definitive.insert(name, node_id);
    }

    fn enter(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn leave(&mut self) {
        self.scopes.pop();
    }
}

impl<'a> Parser<'a> {
    pub fn new(
        input: &'a str,
        lexer: &Lexer,
        file_id_to_name: &'a HashMap<FileId, String>,
    ) -> Self {
        Self {
            error_mode: false,
            tokens: lexer.tokens.clone(),
            tokens_consumed: 0,
            errors: lexer.errors.clone(),
            input,
            file_id_to_name,
            nodes: Vec::new(),
            node_to_type: HashMap::new(),
            name_to_def: NameToDef::new(),
            typenames: HashSet::new(),
        }
    }

    fn new_node_unknown(&mut self) -> NodeId {
        self.new_node(Node {
            kind: NodeKind::Unknown,
            origin: self.current_or_last_origin_for_err(),
        })
    }

    fn new_node(&mut self, node: Node) -> NodeId {
        self.nodes.push(node);
        NodeId(self.nodes.len() - 1)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.tokens_consumed)
    }

    fn peek_peek(&self) -> Option<&Token> {
        self.tokens.get(self.tokens_consumed + 1)
    }

    fn eat_token(&mut self) -> Option<&Token> {
        assert!(self.tokens_consumed <= self.tokens.len());
        if self.tokens_consumed == self.tokens.len() {
            None
        } else {
            self.tokens_consumed += 1;
            Some(&self.tokens[self.tokens_consumed - 1])
        }
    }

    // Used to avoid an avalanche of errors for the same line.
    fn skip_to_next_line(&mut self) {
        let current_line = self.peek().map(|t| t.origin.line).unwrap_or(1);

        loop {
            match self.peek() {
                None => return,
                Some(t) if t.kind == TokenKind::Eof || t.origin.line > current_line => {
                    self.tokens_consumed += 1;
                    return;
                }
                _ => {
                    self.tokens_consumed += 1;
                }
            };
        }
    }

    fn add_error_with_explanation(&mut self, kind: ErrorKind, origin: Origin, explanation: String) {
        if self.error_mode {
            return;
        }

        self.errors.push(Error::new(kind, origin, explanation));
        self.error_mode = true;

        // Skip to the next newline to avoid having cascading errors.
        self.skip_to_next_line();
    }

    fn match_kind(&mut self, kind: TokenKind) -> Option<Token> {
        match self.peek() {
            Some(t) if t.kind == kind => {
                let res = Some(*t);
                self.tokens_consumed += 1;
                res
            }
            _ => None,
        }
    }

    //primary_expression      → IDENT
    //                        | AGG
    //                        | INT
    //                        | STRING
    //                        | "self"
    //                        | "this"
    //                        | "(" expression ")" ;
    fn parse_primary_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        match self.peek() {
            Some(
                tok @ Token {
                    kind: TokenKind::Identifier,
                    ..
                },
            ) => {
                let origin = tok.origin;
                self.eat_token();

                let identifier = Self::str_from_source(self.input, &origin).to_owned();

                Some(self.new_node(Node {
                    kind: if identifier.starts_with("@") {
                        NodeKind::Aggregation(identifier)
                    } else {
                        NodeKind::Identifier(identifier)
                    },
                    origin,
                }))
            }
            Some(Token {
                kind: TokenKind::LiteralNumber,
                ..
            }) => self.parse_literal_number(),
            Some(
                tok @ Token {
                    kind: TokenKind::LiteralString | TokenKind::KeywordSelf | TokenKind::KeywordThis,
                    ..
                },
            ) => {
                let origin = tok.origin;
                let kind = tok.kind;
                self.eat_token();

                Some(self.new_node(Node {
                    kind: NodeKind::PrimaryToken(kind),
                    origin,
                }))
            }
            Some(Token {
                kind: TokenKind::LeftParen,
                ..
            }) => {
                let left_paren = self.match_kind(TokenKind::LeftParen)?;
                let e = self.parse_expr().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        left_paren.origin,
                        format!(
                            "expected expression after parenthesis, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });
                let _ = self.expect_token_one(
                    TokenKind::RightParen,
                    "primary expression closing parenthesis",
                );
                Some(self.new_node(Node {
                    kind: NodeKind::Unary(TokenKind::LeftParen, e),
                    origin: left_paren.origin,
                }))
            }
            _ => None,
        }
    }

    //additive_expression     → multiplicative_expression
    //                        | additive_expression "+" multiplicative_expression
    //                        | additive_expression "-" multiplicative_expression ;
    fn parse_additive_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_multiplicative_expr()?;
        while let Some(Token {
            kind: TokenKind::Plus | TokenKind::Minus,
            ..
        }) = self.peek()
        {
            let op = *self.eat_token().unwrap();

            let rhs = match self.parse_multiplicative_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected multiplicative expression, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: op.origin,
            });
        }

        Some(lhs)
    }

    //multiplicative_expression
    //                        → cast_expression
    //                        | multiplicative_expression "*" cast_expression
    //                        | multiplicative_expression "/" cast_expression
    //                        | multiplicative_expression "%" cast_expression ;
    fn parse_multiplicative_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_cast_expr()?;
        loop {
            let op = match self.peek() {
                Some(
                    op @ Token {
                        kind: TokenKind::Star | TokenKind::Slash | TokenKind::Percent,
                        ..
                    },
                ) if self.peek_peek().map(|t| t.kind) != Some(TokenKind::LeftCurly) => *op,
                _ => {
                    break;
                }
            };
            self.eat_token();

            let rhs = match self.parse_cast_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected cast expression, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: op.origin,
            });
        }

        Some(lhs)
    }

    //cast_expression         → unary_expression
    //                        | "(" type_name ")" cast_expression ;
    fn parse_cast_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        if let Some(op) = self.match_kind(TokenKind::LeftParen) {
            let typ = self.expect_token_one(TokenKind::Identifier, "type in cast");
            let typ_str = if let Some(typ) = typ {
                Self::str_from_source(self.input, &typ.origin).to_owned()
            } else {
                String::new()
            };
            let right_paren =
                self.expect_token_one(TokenKind::RightParen, "closing cast right parenthesis");
            let node = self.parse_cast_expr().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingExpr,
                    right_paren.map(|t| t.origin).unwrap_or(op.origin),
                    format!(
                        "expected expression after parenthesis in cast, found: {:?}",
                        self.current_token_kind_for_err()
                    ),
                );
                self.new_node_unknown()
            });
            return Some(self.new_node(Node {
                kind: NodeKind::Cast(typ_str, node),
                origin: op.origin,
            }));
        }

        self.parse_unary_expr()
    }

    //expression              → assignment_expression ( "," assignment_expression )* ;
    fn parse_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let first_expr = self.parse_assignment_expr()?;

        if self.peek().map(|t| t.kind) != Some(TokenKind::Comma) {
            return Some(first_expr);
        }

        let mut exprs = vec![first_expr];
        let first_comma_origin = self.peek().map(|t| t.origin).unwrap();

        while let Some(tok) = self.match_kind(TokenKind::Comma) {
            let expr = self.parse_assignment_expr().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingExpr,
                    tok.origin,
                    format!(
                        "expected expression following comma, found: {:?}",
                        self.current_token_kind_for_err()
                    ),
                );
                self.new_node_unknown()
            });
            exprs.push(expr);
        }

        Some(self.new_node(Node {
            kind: NodeKind::CommaExpr(exprs),
            origin: first_comma_origin,
        }))
    }

    // unary_expression        → postfix_expression
    //                         | "++" unary_expression
    //                         | "--" unary_expression
    //                         | unary_operator cast_expression
    //                         | "sizeof" unary_expression
    //                         | "sizeof" "(" type_name ")"
    //                         | "stringof" unary_expression ;
    // unary_operator          → "&" | "*" | "+" | "-" | "~" | "!" ;
    fn parse_unary_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        match self.peek() {
            Some(Token {
                kind: TokenKind::PlusPlus | TokenKind::MinusMinus,
                ..
            }) => {
                let op = *self.eat_token().unwrap();
                let unary = self.parse_unary_expr().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected unary expression after {:?}, found: {:?}",
                            op.kind,
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });
                Some(self.new_node(Node {
                    kind: NodeKind::Unary(op.kind, unary),
                    origin: op.origin,
                }))
            }
            Some(Token {
                kind:
                    TokenKind::Ampersand
                    | TokenKind::Star
                    | TokenKind::Plus
                    | TokenKind::Minus
                    | TokenKind::Tilde
                    | TokenKind::Bang,
                ..
            }) => {
                let op = *self.eat_token().unwrap();

                let node = match self.parse_cast_expr() {
                    None => self.new_node_unknown(),
                    Some(n) => n,
                };
                Some(self.new_node(Node {
                    kind: NodeKind::Unary(op.kind, node),
                    origin: op.origin,
                }))
            }
            Some(Token {
                kind: TokenKind::KeywordSizeof,
                ..
            }) => {
                let op = *self.eat_token().unwrap();

                if self.match_kind(TokenKind::LeftParen).is_some() {
                    let typename = self
                        .expect_token_one(TokenKind::Identifier, "type name for sizeof")
                        .map(|t| Self::str_from_source(self.input, &t.origin).to_owned())
                        .unwrap_or_default();
                    self.expect_token_one(TokenKind::RightParen, "matching parenthesis for sizeof");

                    Some(self.new_node(Node {
                        kind: NodeKind::SizeofType(typename),
                        origin: op.origin,
                    }))
                } else {
                    let unary = self.parse_unary_expr().unwrap_or_else(|| {
                        self.add_error_with_explanation(
                            ErrorKind::MissingExpr,
                            op.origin,
                            format!(
                                "expected unary expression after sizeof, found: {:?}",
                                self.current_token_kind_for_err()
                            ),
                        );
                        self.new_node_unknown()
                    });

                    Some(self.new_node(Node {
                        kind: NodeKind::SizeofExpr(unary),
                        origin: op.origin,
                    }))
                }
            }
            Some(Token {
                kind: TokenKind::KeywordStringof,
                ..
            }) => {
                let op = *self.eat_token().unwrap();

                let unary = self.parse_unary_expr().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected unary expression after stringof, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });

                Some(self.new_node(Node {
                    kind: NodeKind::StringofExpr(unary),
                    origin: op.origin,
                }))
            }

            _ => self.parse_postfix_expr(),
        }
    }

    // Certain DTrace-specific keywords may appear as struct/union member names in member-access
    // expressions.
    fn parse_keyword_as_ident(&mut self) -> Option<Token> {
        match self.peek().map(|t| t.kind) {
            Some(
                TokenKind::Identifier
                | TokenKind::KeywordProbe
                | TokenKind::KeywordProvider
                | TokenKind::KeywordSelf
                | TokenKind::KeywordString
                | TokenKind::KeywordStringof
                | TokenKind::KeywordUserland
                | TokenKind::KeywordXlate
                | TokenKind::KeywordTranslator,
            ) => self.eat_token().copied(),
            _ => None,
        }
    }

    //postfix_expression      → primary_expression
    //                        | postfix_expression "[" argument_expression_list "]"
    //                        | postfix_expression "(" argument_expression_list? ")"
    //                        | postfix_expression "."  ( IDENT | TNAME | keyword_as_ident )
    //                        | postfix_expression "->" ( IDENT | TNAME | keyword_as_ident )
    //                        | postfix_expression "++"
    //                        | postfix_expression "--"
    //                        | "offsetof" "(" type_name ","
    //                          ( IDENT | TNAME | keyword_as_ident ) ")"
    //                        | "xlate" "<" type_name ">" "(" expression ")" ;
    //
    fn parse_postfix_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        // Handle `offsetof` and `xlate` first.
        match self.peek() {
            Some(Token {
                kind: TokenKind::KeywordOffsetOf,
                ..
            }) => {
                let op = *self.eat_token().unwrap();
                let left_paren = self
                    .expect_token_one(TokenKind::LeftParen, "opening parenthesis after offsetof");
                let type_name = self.parse_type_name().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingTypeName,
                        left_paren.map(|t| t.origin).unwrap_or(op.origin),
                        format!(
                            "expected type name after offsetof, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });
                let comma = self.expect_token_one(TokenKind::Comma, "comma after type name");
                let field = if let Some(identifier) = self.match_kind(TokenKind::Identifier) {
                    identifier
                } else if let Some(keyword_as_ident) = self.parse_keyword_as_ident() {
                    keyword_as_ident
                } else {
                    self.add_error_with_explanation(
                        ErrorKind::MissingFieldOrKeywordInMemberAccess,
                        comma.map(|t| t.origin).unwrap_or(op.origin),
                        format!(
                            "expected field or keyword as offsetof last argument, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    Token::default()
                };
                self.expect_token_one(TokenKind::RightParen, "closing parenthesis after field");
                return Some(self.new_node(Node {
                    kind: NodeKind::OffsetOf(type_name, field),
                    origin: op.origin,
                }));
            }
            Some(Token {
                kind: TokenKind::KeywordXlate,
                ..
            }) => {
                let op = *self.eat_token().unwrap();
                let lt = self.expect_token_one(TokenKind::Lt, "'<' after xlate");
                let type_name = self.parse_type_name().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingTypeName,
                        lt.map(|t| t.origin).unwrap_or(op.origin),
                        format!(
                            "expected type name after xlate, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });
                self.expect_token_one(TokenKind::Gt, "'>' after type name");
                let left_paren =
                    self.expect_token_one(TokenKind::LeftParen, "opening parenthesis after '>'");
                let expr = self.parse_expr().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        left_paren.map(|t| t.origin).unwrap_or(op.origin),
                        format!(
                            "expected expression for xlate after type name, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });
                self.expect_token_one(
                    TokenKind::RightParen,
                    "closing parenthesis after expression",
                );

                return Some(self.new_node(Node {
                    kind: NodeKind::Xlate(type_name, expr),
                    origin: op.origin,
                }));
            }
            _ => {}
        }

        let mut lhs = self.parse_primary_expr()?;

        loop {
            match self.peek() {
                Some(Token {
                    kind: TokenKind::LeftSquareBracket,
                    ..
                }) => {
                    let token = *self.eat_token().unwrap();

                    let rhs = self.parse_argument_expr_list();
                    self.expect_token_one(
                        TokenKind::RightSquareBracket,
                        "matching square bracket in argument list",
                    );

                    lhs = self.new_node(Node {
                        kind: NodeKind::PostfixArrayAccess(lhs, rhs),
                        origin: token.origin,
                    });
                }
                Some(Token {
                    kind: TokenKind::LeftParen,
                    ..
                }) => {
                    let token = *self.eat_token().unwrap();

                    let rhs = self.parse_argument_expr_list();
                    self.expect_token_one(
                        TokenKind::RightParen,
                        "matching parenthesis in argument list",
                    );

                    lhs = self.new_node(Node {
                        kind: NodeKind::PostfixArguments(lhs, rhs),
                        origin: token.origin,
                    });
                }
                Some(Token {
                    kind: TokenKind::Dot | TokenKind::Arrow,
                    ..
                }) => {
                    let op = *self.eat_token().unwrap();
                    if let Some(keyword_as_ident) = self.parse_keyword_as_ident() {
                        lhs = self.new_node(Node {
                            kind: NodeKind::FieldAccess(lhs, op.kind, keyword_as_ident),
                            origin: op.origin,
                        });
                    } else {
                        self.add_error_with_explanation(
                            ErrorKind::MissingFieldOrKeywordInMemberAccess,
                            op.origin,
                            format!(
                                "expected identifier or keyword in member access, found: {:?}",
                                self.current_token_kind_for_err()
                            ),
                        );
                        lhs = self.new_node(Node {
                            kind: NodeKind::FieldAccess(lhs, op.kind, Token::default()),
                            origin: op.origin,
                        });
                    }
                }
                Some(Token {
                    kind: TokenKind::PlusPlus | TokenKind::MinusMinus,
                    ..
                }) => {
                    let op = *self.eat_token().unwrap();

                    lhs = self.new_node(Node {
                        kind: NodeKind::PostfixIncDecrement(lhs, op.kind),
                        origin: op.origin,
                    });
                }
                _ => break,
            }
        }

        Some(lhs)
    }

    // argument_expression_list
    //                        → assignment_expression ( "," assignment_expression )* ;
    fn parse_argument_expr_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let expr = self.parse_assignment_expr()?;
        let first_comma_origin = if self.peek().map(|t| t.kind) != Some(TokenKind::Comma) {
            return Some(expr);
        } else {
            self.peek().unwrap().origin
        };

        let mut args = vec![expr];
        while let Some(op) = self.match_kind(TokenKind::Comma) {
            let arg = self.parse_assignment_expr().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingExpr,
                    op.origin,
                    format!(
                        "expected assignment expression in argument list after comma, found: {:?}",
                        self.current_token_kind_for_err()
                    ),
                );
                self.new_node_unknown()
            });
            args.push(arg);
        }

        Some(self.new_node(Node {
            kind: NodeKind::Arguments(args),
            origin: first_comma_origin,
        }))
    }

    // type_name               → specifier_qualifier_list abstract_declarator? ;
    fn parse_type_name(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }
        let specifier = self.parse_specifier_qualifier_list()?;

        let abstract_declarator = self.parse_abstract_declarator();

        Some(self.new_node(Node {
            kind: NodeKind::TypeName(specifier, abstract_declarator),
            origin: self.origin(specifier),
        }))
    }

    fn origin(&self, node_id: NodeId) -> Origin {
        self.nodes[node_id].origin
    }

    // specifier_qualifier_list→ ( type_specifier | type_qualifier )+ ;
    fn parse_specifier_qualifier_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }
        let type_specifier = self
            .parse_type_specifier()
            .or_else(|| self.parse_type_qualifier())?;

        let mut list = vec![type_specifier];

        while let Some(x) = self
            .parse_type_specifier()
            .or_else(|| self.parse_type_qualifier())
        {
            list.push(x);
        }

        Some(self.new_node(Node {
            kind: NodeKind::SpecifierQualifierList(list),
            origin: self.origin(type_specifier),
        }))
    }

    //fn parse_block(&mut self) -> Option<NodeId> {
    //    let left_curly = self.match_kind(TokenKind::LeftCurly)?;
    //
    //    let mut stmts = Vec::new();
    //
    //    for _ in 0..self.remaining_tokens_count() {
    //        match self.peek_token().map(|t| t.kind) {
    //            None | Some(TokenKind::Eof) | Some(TokenKind::RightCurly) => break,
    //            _ => {}
    //        }
    //
    //        let stmt = self.parse_statement()?;
    //        stmts.push(stmt);
    //    }
    //    self.expect_token_one(TokenKind::RightCurly, "block")?;
    //
    //    Some(self.new_node(Node {
    //        kind: NodeKind::Block(stmts),
    //        origin: left_curly.origin,
    //    }))
    //}

    //fn parse_statement_if(&mut self) -> Option<NodeId> {
    //    if self.error_mode {
    //        return None;
    //    }
    //
    //    let keyword_if = self.match_kind(TokenKind::KeywordIf)?;
    //    let cond = self.parse_expr()?;
    //
    //    let then_block = if let Some(b) = self.parse_block() {
    //        b
    //    } else {
    //        let found = self.current_token_kind_for_err();
    //        self.add_error_with_explanation(
    //            ErrorKind::MissingExpectedToken(TokenKind::LeftCurly),
    //            keyword_if.origin,
    //            format!("expected block following if(cond), found: {:?}", found),
    //        );
    //
    //        return None;
    //    };
    //
    //    let else_block = if self.match_kind(TokenKind::KeywordElse).is_some() {
    //        let block = self.parse_block().or_else(|| {
    //            let found = self.current_token_kind_for_err();
    //            self.add_error_with_explanation(
    //                ErrorKind::MissingExpectedToken(TokenKind::LeftCurly),
    //                keyword_if.origin,
    //                format!("expected block following else, found: {:?}", found),
    //            );
    //
    //            None
    //        })?;
    //
    //        Some(block)
    //    } else {
    //        None
    //    };
    //
    //    Some(self.new_node(Node {
    //        kind: NodeKind::If {
    //            cond,
    //            then_block,
    //            else_block,
    //        },
    //        origin: keyword_if.origin,
    //    }))
    //}

    //fn parse_statement_var_decl(&mut self) -> Option<NodeId> {
    //    let identifier = self.expect_token_one(TokenKind::Identifier, "var declaration")?;
    //    let eq = self.expect_token_one(TokenKind::Eq, "var declaration")?;
    //    let expr = if let Some(expr) = self.parse_expr() {
    //        expr
    //    } else {
    //        let found = self.current_token_kind_for_err();
    //        self.add_error_with_explanation(
    //            ErrorKind::MissingExpr,
    //            eq.origin,
    //            format!(
    //                "expected expression in variable declaration following '=' but found: {:?}",
    //                found
    //            ),
    //        );
    //        return None;
    //    };
    //
    //    let identifier_str = Self::str_from_source(self.input, &identifier.origin);
    //
    //    Some(self.new_node(Node {
    //        kind: NodeKind::VarDecl(identifier_str.to_owned(), expr),
    //        origin: identifier.origin,
    //    }))
    //}

    //assignment_expression   → conditional_expression
    //                        | unary_expression assignment_operator
    //                          assignment_expression ;
    fn parse_assignment_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let lhs = self.parse_conditional_expr()?;

        match self.peek().map(|t| t.kind) {
            Some(
                TokenKind::Eq
                | TokenKind::PlusEq
                | TokenKind::MinusEq
                | TokenKind::StarEq
                | TokenKind::SlashEq
                | TokenKind::PercentEq
                | TokenKind::LtLtEq
                | TokenKind::GtGtEq
                | TokenKind::AmpersandEq
                | TokenKind::CaretEq
                | TokenKind::PipeEq,
            ) => {
                let op = *self.eat_token().unwrap();
                let rhs = self.parse_assignment_expr().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected unary expression after assignment operator, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });
                Some(self.new_node(Node {
                    kind: NodeKind::Assignment(lhs, op, rhs),
                    origin: op.origin,
                }))
            }
            _ => Some(lhs),
        }
    }

    //conditional_expression  → logical_or_expression
    //                        | logical_or_expression "?" expression
    //                          ":" conditional_expression ;
    fn parse_conditional_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let lhs = self.parse_logical_or_expr()?;

        if let Some(question_mark) = self.match_kind(TokenKind::Question) {
            let mhs = self.parse_expr().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingExpr,
                    question_mark.origin,
                    format!(
                        "expected expression in ternary condition after question mark, found: {:?}",
                        self.current_token_kind_for_err()
                    ),
                );
                self.new_node_unknown()
            });
            self.expect_token_one(TokenKind::Colon, "colon in ternary expression");
            let rhs = self.parse_conditional_expr().unwrap_or_else(||{
                    self.add_error_with_explanation(ErrorKind::MissingExpr, self.current_or_last_origin_for_err(), format!("expected conditional expression in ternary condition after colon, found: {:?}", self.current_token_kind_for_err()));
                    self.new_node_unknown()
                });

            Some(self.new_node(Node {
                kind: NodeKind::TernaryExpr(lhs, mhs, rhs),
                origin: question_mark.origin,
            }))
        } else {
            Some(lhs)
        }
    }

    //logical_or_expression   → logical_xor_expression
    //                        | logical_or_expression "||" logical_xor_expression ;
    fn parse_logical_or_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_logical_xor_expr()?;
        while let Some(op) = self.match_kind(TokenKind::PipePipe) {
            let rhs = match self.parse_logical_xor_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected logical xor expression, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: op.origin,
            });
        }

        Some(lhs)
    }

    //logical_xor_expression  → logical_and_expression
    //                        | logical_xor_expression "^^" logical_and_expression ;
    fn parse_logical_xor_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_logical_and_expr()?;
        while let Some(op) = self.match_kind(TokenKind::CaretCaret) {
            let rhs = match self.parse_logical_and_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected logical and expression, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: op.origin,
            });
        }

        Some(lhs)
    }

    //logical_and_expression  → inclusive_or_expression
    //                        | logical_and_expression "&&" inclusive_or_expression ;
    fn parse_logical_and_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_inclusive_or_expr()?;
        while let Some(op) = self.match_kind(TokenKind::AmpersandAmpersand) {
            let rhs = match self.parse_inclusive_or_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected logical or expression, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: op.origin,
            });
        }

        Some(lhs)
    }

    //inclusive_or_expression → exclusive_or_expression
    //                        | inclusive_or_expression "|" exclusive_or_expression ;
    fn parse_inclusive_or_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_exclusive_or_expr()?;
        while let Some(op) = self.match_kind(TokenKind::Pipe) {
            let rhs = match self.parse_exclusive_or_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected exclusive or expression, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: op.origin,
            });
        }

        Some(lhs)
    }

    //exclusive_or_expression → and_expression
    //                        | exclusive_or_expression "^" and_expression ;
    fn parse_exclusive_or_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_and_expr()?;
        while let Some(op) = self.match_kind(TokenKind::Caret) {
            let rhs = match self.parse_and_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected logical or expression, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: op.origin,
            });
        }

        Some(lhs)
    }

    //and_expression          → equality_expression
    //                        | and_expression "&" equality_expression ;
    fn parse_and_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_equality_expr()?;
        while let Some(op) = self.match_kind(TokenKind::Ampersand) {
            let rhs = match self.parse_equality_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected equality expression, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: op.origin,
            });
        }

        Some(lhs)
    }

    //equality_expression     → relational_expression
    //                        | equality_expression "==" relational_expression
    //                        | equality_expression "!=" relational_expression ;
    fn parse_equality_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_relational_expr()?;
        while let Some(Token {
            kind: TokenKind::EqEq | TokenKind::BangEq,
            ..
        }) = self.peek()
        {
            let op = *self.eat_token().unwrap();

            let rhs = match self.parse_relational_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected equality expression, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: op.origin,
            });
        }

        Some(lhs)
    }

    //relational_expression   → shift_expression
    //                        | relational_expression "<"  shift_expression
    //                        | relational_expression ">"  shift_expression
    //                        | relational_expression "<=" shift_expression
    //                        | relational_expression ">=" shift_expression ;
    fn parse_relational_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_shift_expr()?;
        while let Some(Token {
            kind: TokenKind::Gt | TokenKind::Lt | TokenKind::LtEq | TokenKind::GtEq,
            ..
        }) = self.peek()
        {
            let op = *self.eat_token().unwrap();

            let rhs = match self.parse_shift_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected equality expression, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: op.origin,
            });
        }

        Some(lhs)
    }

    //shift_expression        → additive_expression
    //                        | shift_expression "<<" additive_expression
    //                        | shift_expression ">>" additive_expression ;
    fn parse_shift_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_additive_expr()?;

        while let Some(TokenKind::LtLt | TokenKind::GtGt) = self.peek().map(|t| t.kind) {
            let op = *self.eat_token().unwrap();
            let rhs = self.parse_additive_expr().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingExpr,
                    op.origin,
                    format!(
                        "expected additive expression after shift operator, found: {:?}",
                        self.current_token_kind_for_err()
                    ),
                );
                self.new_node_unknown()
            });

            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: op.origin,
            });
        }

        Some(lhs)
    }

    //statement               → ";"
    //                        | expression ";"
    //                        | "if" "(" expression ")" statement_or_block
    //                        | "if" "(" expression ")" statement_or_block
    //                          "else" statement_or_block ;

    fn parse_statement(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        if self.match_kind(TokenKind::SemiColon).is_some() {
            return None;
        }

        match self.peek().map(|t| t.kind) {
            Some(TokenKind::KeywordIf) => {
                let if_token = *self.eat_token().unwrap();

                self.expect_token_one(TokenKind::LeftParen, "opening parenthesis in if expression");
                let cond = self.parse_expr().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        self.current_or_last_origin_for_err(),
                        format!(
                            "expected expression in if, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });
                self.expect_token_one(
                    TokenKind::RightParen,
                    "closing parenthesis in if expression",
                );
                let then_block = self.parse_statement_or_block().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingStatementOrBlock,
                        self.current_or_last_origin_for_err(),
                        format!(
                            "expected statement or block after if condition, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });

                let else_block: Option<NodeId> =
                    self.match_kind(TokenKind::KeywordElse).map(|_else_token| {
                        self.parse_statement_or_block().unwrap_or_else(|| {
                            self.add_error_with_explanation(
                                ErrorKind::MissingStatementOrBlock,
                                self.current_or_last_origin_for_err(),
                                format!(
                                    "expected statement or block after else, found: {:?}",
                                    self.current_token_kind_for_err()
                                ),
                            );
                            self.new_node_unknown()
                        })
                    });

                Some(self.new_node(Node {
                    kind: NodeKind::If {
                        cond,
                        then_block,
                        else_block,
                    },
                    origin: if_token.origin,
                }))
            }
            _ => self.parse_expr(),
        }
    }

    // statement_or_block      → statement | "{" statement_list "}" ;
    fn parse_statement_or_block(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        if let Some(_left_curly) = self.match_kind(TokenKind::LeftCurly) {
            let stmt_list = self.parse_statement_list();
            self.expect_token_one(
                TokenKind::RightCurly,
                "matching right curly brace after block",
            );
            return stmt_list;
        }
        self.parse_statement()
    }

    // Best effort to find the closest token when doing error reporting.
    fn current_or_last_origin_for_err(&self) -> Origin {
        assert!(self.tokens_consumed <= self.tokens.len());

        if self.tokens_consumed == self.tokens.len() {
            return self
                .tokens
                .last()
                .map(|t| t.origin)
                .unwrap_or_else(Origin::new_unknown);
        }

        let token = &self.tokens[self.tokens_consumed];
        if token.kind != TokenKind::Eof {
            token.origin
        } else if self.tokens_consumed > 0 {
            self.tokens[self.tokens_consumed - 1].origin
        } else {
            Origin::new_unknown()
        }
    }

    pub(crate) fn str_from_source(src: &'a str, origin: &Origin) -> &'a str {
        &src[origin.offset as usize..origin.offset as usize + origin.len as usize]
    }

    fn remaining_tokens_count(&self) -> usize {
        self.tokens.len() - self.tokens_consumed
    }

    fn expect_token_one(&mut self, token_kind: TokenKind, context: &str) -> Option<Token> {
        if let Some(token) = self.match_kind(token_kind) {
            Some(token)
        } else {
            self.add_error_with_explanation(
                ErrorKind::MissingExpectedToken(token_kind),
                self.current_or_last_origin_for_err(),
                format!("failed to parse {}: missing {:?}", context, token_kind),
            );
            None
        }
    }

    fn parse_literal_number(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let tok = self.peek();
        let origin = tok.map(|t| t.origin).unwrap_or(Origin::new_unknown());
        self.eat_token().unwrap();
        let src = Self::str_from_source(self.input, &origin);
        let num: u64 = str::parse(src)
            .map_err(|err: ParseIntError| {
                self.add_error_with_explanation(
                    ErrorKind::InvalidLiteralNumber,
                    origin,
                    err.to_string(),
                );
            })
            .ok()?;

        let node_id = self.new_node(Node {
            kind: NodeKind::Number(num),
            origin,
        });
        self.node_to_type.insert(node_id, Type::new_int());
        Some(node_id)
    }

    //probe_specifier         → PSPEC | INT ;
    fn parse_probe_specifier(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        if matches!(self.peek().map(|t| t.kind), Some(TokenKind::LiteralNumber)) {
            return self.parse_literal_number();
        }

        if let Some(tok) = self.match_kind(TokenKind::ProbeSpecifier) {
            let s = Self::str_from_source(self.input, &tok.origin).to_owned();
            let node_id = self.new_node(Node {
                kind: NodeKind::ProbeSpecifier(s),
                origin: tok.origin,
            });
            return Some(node_id);
        }
        if let Some(tok) = self.match_kind(TokenKind::Identifier) {
            let s = Self::str_from_source(self.input, &tok.origin).to_owned();
            let node_id = self.new_node(Node {
                kind: NodeKind::ProbeSpecifier(s),
                origin: tok.origin,
            });
            return Some(node_id);
        }

        None
    }

    // statement_list          → statement* expression? ;
    fn parse_statement_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }
        let mut stmts = Vec::new();
        for _ in 0..self.remaining_tokens_count() {
            match self.peek().map(|t| t.kind) {
                Some(TokenKind::RightCurly) => {
                    let origin = self.peek().unwrap().origin;
                    return Some(self.new_node(Node {
                        kind: NodeKind::Block(stmts),
                        origin,
                    }));
                }
                Some(TokenKind::KeywordIf) => {
                    let stmt = self.parse_statement().unwrap();
                    stmts.push(stmt);
                }
                Some(TokenKind::Eof) | None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingStatement,
                        self.current_or_last_origin_for_err(),
                        "reached EOF while parsing statement, did you forget a semicolon?"
                            .to_owned(),
                    );
                    return Some(self.new_node(Node {
                        kind: NodeKind::Block(stmts),
                        origin: self.current_or_last_origin_for_err(),
                    }));
                }
                Some(TokenKind::SemiColon) => {
                    let tok = *self.eat_token().unwrap();
                    stmts.push(self.new_node(Node {
                        kind: NodeKind::EmptyStmt,
                        origin: tok.origin,
                    }));
                }
                Some(_) => {
                    let origin = self
                        .peek()
                        .map(|t| t.origin)
                        .unwrap_or_else(Origin::new_unknown);

                    let expr = self.parse_expr().unwrap_or_else(|| {
                        self.add_error_with_explanation(
                            ErrorKind::MissingExpr,
                            self.current_or_last_origin_for_err(),
                            format!(
                                "expected expression in statement list, found: {:?}",
                                self.current_token_kind_for_err()
                            ),
                        );
                        self.new_node_unknown()
                    });

                    if let Some(tok) = self.match_kind(TokenKind::SemiColon) {
                        stmts.push(self.new_node(Node {
                            kind: NodeKind::ExprStmt(expr),
                            origin: tok.origin,
                        }));
                    } else {
                        stmts.push(self.new_node(Node {
                            kind: NodeKind::ExprStmt(expr),
                            origin: Origin::new_unknown(),
                        }));

                        return Some(self.new_node(Node {
                            kind: NodeKind::Block(stmts),
                            origin,
                        }));
                    }
                }
            }
        }
        None
    }

    // probe_specifier_list    → probe_specifier ( "," probe_specifier )* ;
    fn parse_probe_specifier_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let probe_specifier = self.parse_probe_specifier()?;

        if self.peek().map(|t| t.kind) != Some(TokenKind::Comma) {
            return Some(probe_specifier);
        }
        let first_comma_origin = self.peek().unwrap().origin;
        let mut specifiers = vec![probe_specifier];

        while let Some(comma) = self.match_kind(TokenKind::Comma) {
            let specifier = self.parse_probe_specifier().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingProbeSpecifier,
                    comma.origin,
                    format!(
                        "expected probe specifier following comma, found: {:?}",
                        self.current_token_kind_for_err()
                    ),
                );
                self.new_node_unknown()
            });
            specifiers.push(specifier);
        }

        Some(self.new_node(Node {
            kind: NodeKind::ProbeSpecifiers(specifiers),
            origin: first_comma_origin,
        }))
    }

    fn current_token_kind_for_err(&self) -> TokenKind {
        self.peek().map(|t| t.kind).unwrap_or(TokenKind::Eof)
    }

    fn parse_probe_specifiers(&mut self) -> Option<NodeId> {
        self.parse_probe_specifier_list()
    }

    //probe_definition        → probe_specifiers
    //                          | probe_specifiers "{" statement_list "}"
    //                          | probe_specifiers "/" expression "/" "{" statement_list "}" ;
    fn parse_probe_definition(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let probe_specifier = self.parse_probe_specifiers()?;

        // In file mode, a predication or action MUST follow.

        let predicate = if let Some(_slash) = self.match_kind(TokenKind::Slash) {
            let expr = self.parse_expr();
            self.expect_token_one(TokenKind::Slash, "matching slash after predicate");
            expr
        } else {
            None
        };

        if let Some(lcurly) = self.match_kind(TokenKind::LeftCurly) {
            let stmts = self.parse_statement_list();

            self.expect_token_one(
                TokenKind::RightCurly,
                "matching right curly bracket after action",
            );
            let node_id = self.new_node(Node {
                kind: NodeKind::ProbeDefinition(probe_specifier, predicate, stmts),
                origin: lcurly.origin,
            });

            return Some(node_id);
        }

        self.add_error_with_explanation(
            ErrorKind::MissingPredicateOrAction,
            self.current_or_last_origin_for_err(),
            format!(
                "a predicate or action must follow a probe specifier in file mode, found: {:?}",
                self.current_token_kind_for_err()
            ),
        );
        Some(self.new_node(Node {
            kind: NodeKind::ProbeDefinition(probe_specifier, predicate, None),
            origin: self.current_or_last_origin_for_err(),
        }))
    }

    // external_declaration    → inline_definition
    //                           | translator_definition
    //                           | provider_definition
    //                           | probe_definition
    //                           | declaration ;
    fn parse_external_declaration(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        // TODO: inline_definition.
        // TODO: translator_definition.
        // TODO: provider_definition.

        if let Some(stmt) = self.parse_probe_definition() {
            return Some(stmt);
        };

        self.parse_declaration()
    }

    fn is_at_end(&self) -> bool {
        matches!(
            self.peek(),
            Some(Token {
                kind: TokenKind::Eof,
                ..
            }) | None
        )
    }

    // translation_unit        → external_declaration+ ;
    fn parse_translation_unit(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        // Heuristic.
        let mut decls = Vec::with_capacity(self.remaining_tokens_count() / 8);

        for _i in 0..self.remaining_tokens_count() {
            if self.is_at_end() {
                break;
            }
            if let Some(decl) = self.parse_external_declaration() {
                decls.push(decl);
            }
        }

        if decls.is_empty() {
            self.add_error_with_explanation(
                ErrorKind::EmptyTranslationUnit,
                self.current_or_last_origin_for_err(),
                "empty translation unit, no declarations found".to_owned(),
            );
        }

        let node_id = self.new_node(Node {
            kind: NodeKind::TranslationUnit(decls),
            origin: Origin::new_unknown(),
        });
        Some(node_id)
    }

    // d_program               → translation_unit? ;
    fn parse_d_program(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        self.parse_translation_unit()
    }

    // program                 → d_expression | d_program | d_type ;
    fn parse_program(&mut self) -> Option<NodeId> {
        assert!(!self.error_mode);

        for _i in 0..self.tokens.len() {
            let token = match self.peek().map(|t| t.kind) {
                None | Some(TokenKind::Eof) => break,
                Some(t) => t,
            };

            if self.error_mode {
                self.skip_to_next_line();
                self.error_mode = false;
                continue;
            }

            // `d_expression` is the same `expression`.
            if let Some(expr) = self.parse_expr() {
                return Some(expr);
            }

            if let Some(prog) = self.parse_d_program() {
                return Some(prog);
            }

            if let Some(typ) = self.parse_type_name() {
                return Some(typ);
            }

            // Catch-all.
            self.add_error_with_explanation(
                ErrorKind::ParseProgram,
                self.current_or_last_origin_for_err(),
                format!(
                    "catch-all parse program error: encountered unexpected token {:?}",
                    token
                ),
            );
        }
        None
    }

    #[warn(unused_results)]
    pub fn parse(&mut self) -> Option<NodeId> {
        let root = self.parse_program();
        if let Some(root) = root {
            log(&self.nodes, root, 0);
        }

        // self.resolve_nodes();

        root
    }

    fn resolve_node(
        node_id: NodeId,
        nodes: &[Node],
        errors: &mut Vec<Error>,
        name_to_def: &mut NameToDef,
        file_id_to_name: &'a HashMap<FileId, String>,
    ) {
        let node = &nodes[node_id];

        match &node.kind {
            NodeKind::ProbeDefinition(probe, pred, actions) => {
                Self::resolve_node(*probe, nodes, errors, name_to_def, file_id_to_name);
                if let Some(pred) = pred {
                    Self::resolve_node(*pred, nodes, errors, name_to_def, file_id_to_name);
                }
                if let Some(actions) = actions {
                    Self::resolve_node(*actions, nodes, errors, name_to_def, file_id_to_name);
                }
            }
            NodeKind::Number(_) | NodeKind::ProbeSpecifier(_) => {}
            NodeKind::Unary(_, expr) => {
                Self::resolve_node(*expr, nodes, errors, name_to_def, file_id_to_name);
            }
            NodeKind::Assignment(lhs, _, rhs) => {
                Self::resolve_node(*lhs, nodes, errors, name_to_def, file_id_to_name);
                Self::resolve_node(*rhs, nodes, errors, name_to_def, file_id_to_name);
            }
            NodeKind::Identifier(name) => {
                let def_id = if let Some((def_id, _)) = name_to_def.get_scoped(name) {
                    def_id
                } else {
                    errors.push(Error::new(
                        ErrorKind::UnknownIdentifier,
                        node.origin,
                        format!("unknown identifier: {}", name),
                    ));
                    return;
                };

                let def = &nodes[*def_id];

                match def.kind {
                    NodeKind::Identifier(_) => {}
                    _ => {
                        panic!("identifier refers to invalid node: {:?}", def);
                    }
                }
            }
            NodeKind::BinaryOp(lhs, _, rhs) => {
                Self::resolve_node(*lhs, nodes, errors, name_to_def, file_id_to_name);
                Self::resolve_node(*rhs, nodes, errors, name_to_def, file_id_to_name);
            }
            NodeKind::Arguments(args) => {
                for arg in args {
                    Self::resolve_node(*arg, nodes, errors, name_to_def, file_id_to_name);
                }
            }
            NodeKind::Block(stmts) => {
                name_to_def.enter();

                for op in stmts {
                    Self::resolve_node(*op, nodes, errors, name_to_def, file_id_to_name);
                }

                name_to_def.leave();
            }
            NodeKind::If {
                cond,
                then_block,
                else_block,
            } => {
                Self::resolve_node(*cond, nodes, errors, name_to_def, file_id_to_name);
                Self::resolve_node(*then_block, nodes, errors, name_to_def, file_id_to_name);
                if let Some(else_block) = else_block {
                    Self::resolve_node(*else_block, nodes, errors, name_to_def, file_id_to_name);
                }
            }
            NodeKind::TranslationUnit(decls) => {
                for decl in decls {
                    Self::resolve_node(*decl, nodes, errors, name_to_def, file_id_to_name);
                }
            }
            NodeKind::Unknown => {}
            NodeKind::PrimaryToken(_) => {}
            NodeKind::Cast(_, _) => {}
            NodeKind::Aggregation(_) => {}
            NodeKind::CommaExpr(_node_ids) => {}
            NodeKind::SizeofType(_) => {}
            NodeKind::SizeofExpr(_node_id) => {}
            NodeKind::StringofExpr(_node_id) => {}
            NodeKind::PostfixIncDecrement(_node_id, _token_kind) => {}
            NodeKind::ExprStmt(_node_id) => {}
            NodeKind::EmptyStmt => {}
            NodeKind::PostfixArguments(_, _node_id) => {}
            NodeKind::TernaryExpr(_node_id, _node_id1, _node_id2) => {}
            NodeKind::PostfixArrayAccess(_node_id, _node_id1) => {}
            NodeKind::FieldAccess(_node_id, _token_kind, _token) => {}
            NodeKind::ProbeSpecifiers(_node_ids) => {}
            NodeKind::TypeName(_node_id, _node_id1) => {}
            NodeKind::OffsetOf(_node_id, _token) => {}
            NodeKind::Declaration(_node_id, _node_id1) => {}
            NodeKind::DeclarationSpecifiers(_tokens) => {}
            NodeKind::DirectDeclarator(_, _) => {}
            NodeKind::Declarator(_node_id, _node_id1) => {}
            NodeKind::InitDeclarators(_node_ids) => {}
            NodeKind::TypeQualifier(_token_kind) => {}
            NodeKind::DStorageClassSpecifier(_token_kind) => {}
            NodeKind::StorageClassSpecifier(_token_kind) => {}
            NodeKind::TypeSpecifier(_token_kind) => {}
            NodeKind::EnumDeclaration(_name, _node_id) => {}
            NodeKind::EnumeratorDeclaration(_name, _node_id) => {}
            NodeKind::EnumeratorsDeclaration(_node_ids) => {}
            NodeKind::StructDeclaration(_, _node_id) => {}
            NodeKind::StructFieldsDeclaration(_node_ids) => {}
            NodeKind::StructFieldDeclarator(_node_id, _node_id1) => {}
            NodeKind::StructFieldDeclaration(_node_id, _node_id1) => {}
            NodeKind::StructFieldDeclaratorList(_node_ids) => {}
            NodeKind::SpecifierQualifierList(_node_ids) => {}
            NodeKind::Xlate(_node_id, _node_id1) => {}
            NodeKind::DirectAbstractDeclarator(_node_id) => {}
            NodeKind::DirectAbstractArray(_node_id, _node_id1) => {}
            NodeKind::DirectAbstractFunction(_node_id, _node_id1) => {}
            NodeKind::AbstractDeclarator(_node_id, _node_id1) => {}
            NodeKind::Pointer(_node_ids, _node_id) => {}
            NodeKind::Array(_) => {}
            NodeKind::ParamEllipsis => {}
            NodeKind::Parameters(_) => {}
            NodeKind::ParameterDeclarationSpecifiers(_node_ids) => {}
        }
    }

    fn resolve_nodes(&mut self) {
        if self.nodes.is_empty() {
            return;
        }

        Self::resolve_node(
            NodeId(0),
            &self.nodes,
            &mut self.errors,
            &mut self.name_to_def,
            self.file_id_to_name,
        );
    }

    // abstract_declarator     → pointer
    //                        | pointer? direct_abstract_declarator ;
    fn parse_abstract_declarator(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let ptr = self.parse_pointer();
        let direct = self.parse_direct_abstract_declarator();
        if ptr.is_none() && direct.is_none() {
            return None;
        }

        Some(
            self.new_node(Node {
                kind: NodeKind::AbstractDeclarator(ptr, direct),
                origin: ptr
                    .map(|p| self.origin(p))
                    .or_else(|| direct.map(|d| self.origin(d)))
                    .unwrap(),
            }),
        )
    }

    // declaration             → declaration_specifiers init_declarator_list? ";" ;
    fn parse_declaration(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let decl_specifiers = self.parse_declaration_specifiers()?;

        let init_decl_list = self.parse_init_declarator_list();

        let semicolon =
            self.expect_token_one(TokenKind::SemiColon, "expected semicolon after declaration");

        Some(
            self.new_node(Node {
                kind: NodeKind::Declaration(decl_specifiers, init_decl_list),
                origin: semicolon
                    .map(|t| t.origin)
                    .unwrap_or_else(|| self.current_or_last_origin_for_err()),
            }),
        )
    }

    // declaration_specifiers  → ( d_storage_class_specifier
    //                            | type_specifier
    //                            | type_qualifier )+ ;
    fn parse_declaration_specifiers(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let specifier = self
            .parse_d_storage_class_specifier()
            .or_else(|| self.parse_type_specifier())
            .or_else(|| self.parse_type_qualifier())?;

        let mut specifiers = vec![specifier];
        while let Some(specifier) = self
            .parse_d_storage_class_specifier()
            .or_else(|| self.parse_type_specifier())
            .or_else(|| self.parse_type_qualifier())
        {
            specifiers.push(specifier);
        }

        Some(self.new_node(Node {
            kind: NodeKind::DeclarationSpecifiers(specifiers),
            origin: self.origin(specifier),
        }))
    }

    // init_declarator_list    → init_declarator ( "," init_declarator )* ;
    fn parse_init_declarator_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let init_declarator = self.parse_init_declarator()?;

        let mut declarators = vec![init_declarator];
        while let Some(comma) = self.match_kind(TokenKind::Comma) {
            let declarator = self.parse_init_declarator().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingInitDeclarator,
                    comma.origin,
                    format!(
                        "expected init declarator after comma, found: {:?}",
                        self.current_token_kind_for_err()
                    ),
                );
                self.new_node_unknown()
            });
            declarators.push(declarator);
        }

        Some(self.new_node(Node {
            kind: NodeKind::InitDeclarators(declarators),
            origin: self.origin(init_declarator),
        }))
    }

    // storage_class_specifier → "auto" | "register" | "static" | "extern" | "typedef" ;
    fn parse_storage_class_specifier(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        match self.peek().map(|t| t.kind) {
            Some(
                TokenKind::KeywordAuto
                | TokenKind::KeywordRegister
                | TokenKind::KeywordStatic
                | TokenKind::KeywordExtern
                | TokenKind::KeywordTypedef,
            ) => {
                let tok = *self.eat_token().unwrap();
                Some(self.new_node(Node {
                    kind: NodeKind::StorageClassSpecifier(tok.kind),
                    origin: tok.origin,
                }))
            }
            _ => None,
        }
    }
    // type_specifier          → "void" | "char" | "short" | "int" | "long"
    //                          | "float" | "double" | "signed" | "unsigned"
    //                          | "userland" | "string" | TNAME
    //                          | struct_or_union_specifier
    //                          | enum_specifier ;
    fn parse_type_specifier(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        match self.peek().map(|t| t.kind) {
            Some(
                TokenKind::KeywordVoid
                | TokenKind::KeywordChar
                | TokenKind::KeywordShort
                | TokenKind::KeywordInt
                | TokenKind::KeywordLong
                | TokenKind::KeywordFloat
                | TokenKind::KeywordDouble
                | TokenKind::KeywordSigned
                | TokenKind::KeywordUnsigned
                | TokenKind::KeywordUserland
                | TokenKind::KeywordString,
            ) => {
                let tok = *self.eat_token().unwrap();
                Some(self.new_node(Node {
                    kind: NodeKind::TypeSpecifier(tok.kind),
                    origin: tok.origin,
                }))
            }
            Some(TokenKind::Identifier) => {
                let origin = self.peek().unwrap().origin;
                let kind = self.peek().unwrap().kind;
                let name = Self::str_from_source(self.input, &origin);
                if self.typenames.contains(name) {
                    Some(self.new_node(Node {
                        kind: NodeKind::TypeSpecifier(kind),
                        origin,
                    }))
                } else {
                    None
                }
            }
            Some(TokenKind::KeywordStruct | TokenKind::KeywordUnion) => {
                self.parse_struct_or_union_specifier()
            }
            Some(TokenKind::KeywordEnum) => self.parse_enum_specifier(),
            _ => None,
        }
    }

    // type_qualifier          → "const" | "restrict" | "volatile" ;
    fn parse_type_qualifier(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        match self.peek().map(|t| t.kind) {
            Some(
                TokenKind::KeywordConst | TokenKind::KeywordRestrict | TokenKind::KeywordVolatile,
            ) => {
                let tok = *self.eat_token().unwrap();
                Some(self.new_node(Node {
                    kind: NodeKind::TypeQualifier(tok.kind),
                    origin: tok.origin,
                }))
            }
            _ => None,
        }
    }

    // d_storage_class_specifier
    //                          → storage_class_specifier | "self" | "this" ;
    fn parse_d_storage_class_specifier(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        match self.peek().map(|t| t.kind) {
            Some(TokenKind::KeywordSelf | TokenKind::KeywordThis) => {
                let tok = *self.eat_token().unwrap();
                Some(self.new_node(Node {
                    kind: NodeKind::DStorageClassSpecifier(tok.kind),
                    origin: tok.origin,
                }))
            }
            _ => self.parse_storage_class_specifier(),
        }
    }

    // init_declarator         → declarator ;
    fn parse_init_declarator(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        self.parse_declarator()
    }

    // declarator              → pointer? direct_declarator ;
    fn parse_declarator(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let ptr = self.parse_pointer();
        let direct_declarator = self.parse_direct_declarator();
        if ptr.is_none() && direct_declarator.is_none() {
            return None;
        }
        let direct_declarator = direct_declarator.unwrap_or_else(|| {
            self.add_error_with_explanation(
                ErrorKind::MissingDirectDeclarator,
                self.current_or_last_origin_for_err(),
                format!(
                    "expected directed declarator in declaration, found: {:?}",
                    self.current_token_kind_for_err()
                ),
            );
            self.new_node_unknown()
        });

        Some(self.new_node(Node {
            kind: NodeKind::Declarator(ptr, direct_declarator),
            origin: self.origin(direct_declarator),
        }))
    }

    // direct_declarator       → IDENT
    //                          | "(" declarator ")"
    //                          | direct_declarator array
    //                          | direct_declarator function ;
    fn parse_direct_declarator(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = match self.peek().map(|t| t.kind) {
            Some(TokenKind::Identifier) => {
                let token = *self.eat_token().unwrap();
                let identifier = Self::str_from_source(self.input, &token.origin).to_owned();
                let identifier_node = self.new_node(Node {
                    kind: NodeKind::Identifier(identifier),
                    origin: token.origin,
                });
                Some(self.new_node(Node {
                    kind: NodeKind::DirectDeclarator(identifier_node, None),
                    origin: token.origin,
                }))
            }
            Some(TokenKind::LeftParen) => {
                let left_paren = *self.eat_token().unwrap();
                let decl = self.parse_declarator().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingDeclarator,
                        left_paren.origin,
                        format!(
                            "expected declarator after parenthesis, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });
                self.expect_token_one(
                    TokenKind::LeftParen,
                    "matching parenthesis after declarator",
                );
                Some(self.new_node(Node {
                    kind: NodeKind::DirectDeclarator(decl, None),
                    origin: left_paren.origin,
                }))
            }
            _ => None,
        }?;

        while let Some(node) = self.parse_array().or_else(|| self.parse_function()) {
            lhs = self.new_node(Node {
                kind: NodeKind::DirectDeclarator(lhs, Some(node)),
                origin: self.origin(node),
            });
        }

        Some(lhs)
    }

    fn parse_pointer(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let star = self.match_kind(TokenKind::Star)?;

        let mut type_qualifiers = Vec::new();
        while let Some(typ) = self.parse_type_qualifier() {
            type_qualifiers.push(typ);
        }

        let ptr = self.parse_pointer();

        Some(self.new_node(Node {
            kind: NodeKind::Pointer(type_qualifiers, ptr),
            origin: star.origin,
        }))
    }

    // enum_specifier          → "enum" ( IDENT | TNAME )? "{" enumerator_list "}"
    //                          | "enum" ( IDENT | TNAME ) ;
    fn parse_enum_specifier(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let enum_tok = self.match_kind(TokenKind::KeywordEnum)?;
        let name_tok = self.match_kind(TokenKind::Identifier);
        let name = name_tok.map(|t| Self::str_from_source(self.input, &t.origin).to_owned());
        if let Some(name) = &name {
            self.typenames.insert(name.clone());
        }

        let enumerator_list: Option<NodeId> =
            if let Some(left_curly) = self.match_kind(TokenKind::LeftCurly) {
                let enumerator_list = self.parse_enumerator_list().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingEnumerators,
                        left_curly.origin,
                        format!(
                            "expected at least one enumerator in enum definition, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });
                self.expect_token_one(
                    TokenKind::RightCurly,
                    "closing curly brace after enumerator list",
                );
                Some(enumerator_list)
            } else {
                None
            };

        Some(self.new_node(Node {
            kind: NodeKind::EnumDeclaration(name, enumerator_list),
            origin: enum_tok.origin,
        }))
    }

    // enumerator_list         → enumerator ( "," enumerator )* ;
    fn parse_enumerator_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let enumerator = self.parse_enumerator()?;

        let mut enumerators = vec![enumerator];
        while let Some(comma) = self.match_kind(TokenKind::Comma) {
            let enumerator = self.parse_enumerator().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingEnumerator,
                    comma.origin,
                    format!(
                        "expected enumerator following comma, found: {:?}",
                        self.current_token_kind_for_err()
                    ),
                );
                self.new_node_unknown()
            });
            enumerators.push(enumerator);
        }

        Some(self.new_node(Node {
            kind: NodeKind::EnumeratorsDeclaration(enumerators),
            origin: self.origin(enumerator),
        }))
    }

    fn parse_enumerator(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let identifier_tok = self.match_kind(TokenKind::Identifier)?;
        let expr = self.match_kind(TokenKind::Eq).map(|eq| {
            self.parse_conditional_expr().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingExpr,
                    eq.origin,
                    format!(
                        "expected expression following enumerator, found: {:?}",
                        self.current_token_kind_for_err()
                    ),
                );
                self.new_node_unknown()
            })
        });

        let identifier = Self::str_from_source(self.input, &identifier_tok.origin).to_owned();
        Some(self.new_node(Node {
            kind: NodeKind::EnumeratorDeclaration(identifier, expr),
            origin: identifier_tok.origin,
        }))
    }

    // struct_or_union_specifier → struct_or_union ( IDENT | TNAME )?
    //                          "{" struct_declaration_list "}"
    //                        | struct_or_union ( IDENT | TNAME ) ;
    fn parse_struct_or_union_specifier(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }
        let tok = self
            .match_kind(TokenKind::KeywordStruct)
            .or_else(|| self.match_kind(TokenKind::KeywordUnion))?;

        let name_tok = self.match_kind(TokenKind::Identifier);
        let name = name_tok.map(|t| Self::str_from_source(self.input, &t.origin).to_owned());

        if let Some(name) = &name {
            self.typenames.insert(name.clone());
        }

        let decl_list = if let Some(left_curly) = self.match_kind(TokenKind::LeftCurly) {
            let decl_list = self.parse_struct_declaration_list().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingStructDeclarationList,
                    left_curly.origin,
                    format!(
                        "expected unary expression after {:?}, found: {:?}",
                        left_curly.kind,
                        self.current_token_kind_for_err()
                    ),
                );
                self.new_node_unknown()
            });
            self.expect_token_one(
                TokenKind::RightCurly,
                "closing curly brace after struct definition",
            );
            Some(decl_list)
        } else {
            None
        };

        Some(self.new_node(Node {
            kind: NodeKind::StructDeclaration(name, decl_list),
            origin: tok.origin,
        }))
    }

    // struct_declaration_list → struct_declaration+ ;
    fn parse_struct_declaration_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }
        let decl = self.parse_struct_declaration()?;

        let mut decls = vec![decl];

        while let Some(decl) = self.parse_struct_declaration() {
            decls.push(decl);
        }

        Some(self.new_node(Node {
            kind: NodeKind::StructFieldsDeclaration(decls),
            origin: self.origin(decl),
        }))
    }

    // struct_declaration      → specifier_qualifier_list struct_declarator_list ";" ;
    fn parse_struct_declaration(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }
        let spec = self.parse_specifier_qualifier_list()?;
        let struct_declarator_list = self.parse_struct_declarator_list();
        self.expect_token_one(
            TokenKind::SemiColon,
            "semicolon after field in struct declaration",
        );

        Some(self.new_node(Node {
            kind: NodeKind::StructFieldDeclaration(spec, struct_declarator_list),
            origin: self.origin(spec),
        }))
    }

    // struct_declarator_list  → struct_declarator ( "," struct_declarator )* ;
    fn parse_struct_declarator_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }
        let decl = self.parse_struct_declarator()?;
        let mut decls = vec![decl];
        while let Some(comma) = self.match_kind(TokenKind::Comma) {
            let decl = self.parse_struct_declarator().unwrap_or_else(||{
                self.add_error_with_explanation(ErrorKind::MissingStructFieldDeclarator, comma.origin, format!("expected a struct field declarator after comma in struct field declaration, found: {:?}", self.current_token_kind_for_err()));
                self.new_node_unknown()
            });

            decls.push(decl);
        }
        Some(self.new_node(Node {
            kind: NodeKind::StructFieldDeclaratorList(decls),
            origin: self.origin(decl),
        }))
    }

    // struct_declarator       → declarator ( ":" constant_expression )?
    fn parse_struct_declarator(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }
        let declarator = self.parse_declarator()?;

        let const_expr = if let Some(colon) = self.match_kind(TokenKind::Colon) {
            let expr = self.parse_conditional_expr().unwrap_or_else(||{
                self.add_error_with_explanation(ErrorKind::MissingConstantExpr, colon.origin, format!("expected a constant expression after colon in struct field declaration, found: {:?}", self.current_token_kind_for_err()));
                self.new_node_unknown()
            });
            Some(expr)
        } else {
            None
        };

        Some(self.new_node(Node {
            kind: NodeKind::StructFieldDeclarator(declarator, const_expr),
            origin: self.origin(declarator),
        }))
    }

    // direct_abstract_declarator
    //                        → "(" abstract_declarator ")"
    //                        | direct_abstract_declarator? array
    //                        | direct_abstract_declarator? function ;
    fn parse_direct_abstract_declarator(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = match self.peek().map(|t| t.kind) {
            Some(TokenKind::LeftParen)
                if matches!(
                    self.peek_peek().map(|t| t.kind),
                    Some(TokenKind::Star | TokenKind::LeftParen | TokenKind::LeftSquareBracket,)
                ) =>
            {
                let tok = *self.eat_token().unwrap();

                let abstract_decl = self.parse_abstract_declarator().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingAbstractDeclarator,
                        tok.origin,
                        format!(
                            "expected abstract declarator after parenthesis, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });
                self.expect_token_one(
                    TokenKind::RightParen,
                    "matching parenthesis in direct abstract declarator",
                );

                Some(self.new_node(Node {
                    kind: NodeKind::DirectAbstractDeclarator(abstract_decl),
                    origin: tok.origin,
                }))
            }
            Some(TokenKind::LeftParen) => {
                let func = self.parse_function().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingFunction,
                        self.current_or_last_origin_for_err(),
                        format!(
                            "expected function after parenthesis, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });
                Some(func)
            }
            Some(TokenKind::LeftSquareBracket) => {
                let array = self.parse_array().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingArray,
                        self.current_or_last_origin_for_err(),
                        format!(
                            "expected array after square bracket, found: {:?}",
                            self.current_token_kind_for_err()
                        ),
                    );
                    self.new_node_unknown()
                });
                // TODO: `DirectAbstractArray(None, array)`?
                Some(array)
            }
            _ => None,
        };

        loop {
            match self.peek().map(|t| t.kind) {
                Some(TokenKind::LeftSquareBracket) => {
                    let origin = self.peek().unwrap().origin;
                    let array = self.parse_array().unwrap_or_else(|| {
                        self.add_error_with_explanation(
                            ErrorKind::MissingArray,
                            self.current_or_last_origin_for_err(),
                            format!(
                                "expected array after square bracket, found: {:?}",
                                self.current_token_kind_for_err()
                            ),
                        );
                        self.new_node_unknown()
                    });
                    lhs = Some(self.new_node(Node {
                        kind: NodeKind::DirectAbstractArray(lhs, array),
                        origin,
                    }));
                }
                Some(TokenKind::LeftParen)
                    if !matches!(
                        self.peek_peek().map(|t| t.kind),
                        Some(TokenKind::Star | TokenKind::LeftParen | TokenKind::LeftSquareBracket,)
                    ) =>
                {
                    let origin = self.peek().unwrap().origin;

                    let func = self.parse_array().unwrap_or_else(|| {
                        self.add_error_with_explanation(
                            ErrorKind::MissingFunction,
                            self.current_or_last_origin_for_err(),
                            format!(
                                "expected function after parenthesis, found: {:?}",
                                self.current_token_kind_for_err()
                            ),
                        );
                        self.new_node_unknown()
                    });
                    lhs = Some(self.new_node(Node {
                        kind: NodeKind::DirectAbstractFunction(lhs, func),
                        origin,
                    }))
                }
                _ => {
                    break;
                }
            }
        }

        lhs
    }

    // array                   → "[" array_parameters "]" ;
    fn parse_array(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let left_square_bracket = self.match_kind(TokenKind::LeftSquareBracket)?;

        let params = self.parse_array_parameters();

        self.expect_token_one(
            TokenKind::LeftSquareBracket,
            "match square bracket for array",
        );

        Some(self.new_node(Node {
            kind: NodeKind::Array(params),
            origin: left_square_bracket.origin,
        }))
    }

    fn parse_function(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        todo!()
    }

    // array_parameters        → /* empty */ | constant_expression | parameter_type_list ;
    fn parse_array_parameters(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        // Empty (valid).
        if let Some(Token {
            kind: TokenKind::RightSquareBracket,
            ..
        }) = self.peek()
        {
            return None;
        }

        if let Some(tok) = self.match_kind(TokenKind::DotDotDot) {
            let param = self.new_node(Node {
                kind: NodeKind::ParamEllipsis,
                origin: tok.origin,
            });
            return Some(self.new_node(Node {
                kind: NodeKind::Parameters(vec![param]),
                origin: tok.origin,
            }));
        }

        todo!()
    }

    // parameter_type_list → parameter_list ( "," "..." )?
    fn parse_parameter_type_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        todo!()
    }

    // parameter_list          → parameter_declaration ( "," parameter_declaration )* ;
    fn parse_parameter_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        todo!()
    }

    // parameter_declaration → parameter_declaration_specifiers
    //                        ( declarator | abstract_declarator )? ;
    fn parse_parameter_declaration(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let _param_decl_specifiers = self
            .parse_parameter_declaration_specifiers()
            .unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingParameterDeclarationSpecifiers,
                    self.current_or_last_origin_for_err(),
                    format!(
                        "expected parameter declaration, found: {:?}",
                        self.current_token_kind_for_err()
                    ),
                );

                self.new_node_unknown()
            });
        todo!()
    }

    // parameter_declaration_specifiers → ( storage_class_specifier
    //                                   | type_specifier
    //                                   | type_qualifier )+ ;
    fn parse_parameter_declaration_specifiers(&mut self) -> Option<NodeId> {
        let mut specifiers = Vec::new();

        while let Some(spec) = self
            .parse_storage_class_specifier()
            .or_else(|| self.parse_type_specifier())
            .or_else(|| self.parse_type_qualifier())
        {
            specifiers.push(spec);
        }
        if specifiers.is_empty() {
            self.add_error_with_explanation(
                ErrorKind::MissingParameterDeclarationSpecifiers,
                self.current_or_last_origin_for_err(),
                format!(
                    "expected parameter declaration specifiers, found: {:?}",
                    self.current_token_kind_for_err()
                ),
            );
        }

        let origin = specifiers
            .first()
            .map(|n| self.origin(*n))
            .unwrap_or_else(|| self.current_or_last_origin_for_err());

        Some(self.new_node(Node {
            kind: NodeKind::ParameterDeclarationSpecifiers(specifiers),
            origin,
        }))
    }
}

fn log(nodes: &[Node], node_id: NodeId, indent: usize) {
    let node = &nodes[node_id];
    trace!(
        "{:indent$} id={} kind={:?}",
        "",
        node_id.0,
        node.kind,
        indent = indent
    );
    match &node.kind {
        NodeKind::Unknown => {}
        NodeKind::Block(node_ids) | NodeKind::Arguments(node_ids) => {
            for id in node_ids {
                log(nodes, *id, indent + 2);
            }
        }
        NodeKind::ProbeDefinition(probe, pred, actions) => {
            log(nodes, *probe, indent + 2);
            if let Some(pred) = pred {
                log(nodes, *pred, indent + 2);
            }

            if let Some(actions) = actions {
                log(nodes, *actions, indent + 2);
            }
        }
        NodeKind::Number(_) | NodeKind::Identifier(_) | NodeKind::ProbeSpecifier(_) => {}
        NodeKind::Assignment(lhs, _, rhs) | NodeKind::BinaryOp(lhs, _, rhs) => {
            log(nodes, *lhs, indent + 2);
            log(nodes, *rhs, indent + 2);
        }
        NodeKind::If {
            cond,
            then_block,
            else_block,
        } => {
            log(nodes, *cond, indent + 2);
            log(nodes, *then_block, indent + 2);
            if let Some(else_block) = else_block {
                log(nodes, *else_block, indent + 2);
            }
        }
        NodeKind::TranslationUnit(decls) => {
            for decl in decls {
                log(nodes, *decl, indent + 2);
            }
        }
        NodeKind::PrimaryToken(_) => {}
        NodeKind::Cast(_, _) => {}
        NodeKind::Aggregation(_) => {}
        NodeKind::ProbeSpecifiers(node_ids) | NodeKind::CommaExpr(node_ids) => {
            for node in node_ids {
                log(nodes, *node, indent + 2);
            }
        }
        NodeKind::SizeofType(_) => {}
        NodeKind::SizeofExpr(node_id) => log(nodes, *node_id, indent + 2),
        NodeKind::StringofExpr(node_id) => log(nodes, *node_id, indent + 2),
        NodeKind::PostfixIncDecrement(node_id, _token_kind) => log(nodes, *node_id, indent + 2),
        NodeKind::ExprStmt(node_id) => log(nodes, *node_id, indent + 2),
        NodeKind::EmptyStmt => {}
        NodeKind::PostfixArrayAccess(primary, args) | NodeKind::PostfixArguments(primary, args) => {
            log(nodes, *primary, indent + 2);
            if let Some(node_id) = args {
                log(nodes, *node_id, indent + 2)
            }
        }
        NodeKind::TernaryExpr(lhs, mhs, rhs) => {
            log(nodes, *lhs, indent + 2);
            log(nodes, *mhs, indent + 2);
            log(nodes, *rhs, indent + 2);
        }
        NodeKind::FieldAccess(node_id, _, _) => {
            log(nodes, *node_id, indent + 2);
        }
        NodeKind::TypeName(specifier, declarator) => {
            log(nodes, *specifier, indent + 2);
            if let Some(declarator) = declarator {
                log(nodes, *declarator, indent + 2);
            }
        }
        NodeKind::OffsetOf(node_id, _token) => {
            log(nodes, *node_id, indent + 2);
        }
        NodeKind::Declaration(decl_specifiers, init_declarator_list) => {
            log(nodes, *decl_specifiers, indent + 2);
            if let Some(init_declarator_list) = init_declarator_list {
                log(nodes, *init_declarator_list, indent + 2);
            }
        }
        NodeKind::DeclarationSpecifiers(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::DirectDeclarator(base, suffix) => {
            log(nodes, *base, indent + 2);
            if let Some(node_id) = suffix {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::Declarator(ptr, declarator) => {
            if let Some(ptr) = ptr {
                log(nodes, *ptr, indent + 2);
            }
            log(nodes, *declarator, indent + 2);
        }
        NodeKind::InitDeclarators(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::TypeQualifier(_)
        | NodeKind::DStorageClassSpecifier(_)
        | NodeKind::StorageClassSpecifier(_)
        | NodeKind::TypeSpecifier(_) => {}
        NodeKind::EnumDeclaration(_token, node_id) => {
            if let Some(node_id) = node_id {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::EnumeratorDeclaration(_token, node_id) => {
            if let Some(node_id) = node_id {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::EnumeratorsDeclaration(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::StructDeclaration(_, node_id) => {
            if let Some(node_id) = node_id {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::StructFieldsDeclaration(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::StructFieldDeclarator(declarator, const_expr) => {
            log(nodes, *declarator, indent + 2);
            if let Some(node_id) = const_expr {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::StructFieldDeclaration(specifier_qualifier_list, declarator_list) => {
            log(nodes, *specifier_qualifier_list, indent + 2);
            if let Some(node_id) = declarator_list {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::StructFieldDeclaratorList(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::SpecifierQualifierList(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::Xlate(type_name, expr) => {
            log(nodes, *type_name, indent + 2);
            log(nodes, *expr, indent + 2);
        }
        NodeKind::DirectAbstractDeclarator(node_id) => {
            log(nodes, *node_id, indent + 2);
        }
        NodeKind::DirectAbstractArray(base, suffix) => {
            if let Some(base) = base {
                log(nodes, *base, indent + 2);
            }
            log(nodes, *suffix, indent + 2);
        }
        NodeKind::DirectAbstractFunction(base, suffix) => {
            if let Some(base) = base {
                log(nodes, *base, indent + 2);
            }
            log(nodes, *suffix, indent + 2);
        }
        NodeKind::AbstractDeclarator(ptr, abstract_decl) => {
            if let Some(node_id) = ptr {
                log(nodes, *node_id, indent + 2);
            }
            if let Some(node_id) = abstract_decl {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::Pointer(type_qualifiers, ptr) => {
            for node_id in type_qualifiers {
                log(nodes, *node_id, indent + 2);
            }
            if let Some(node_id) = ptr {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::Array(params) => {
            if let Some(node_id) = params {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::ParamEllipsis => {}
        NodeKind::Parameters(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::ParameterDeclarationSpecifiers(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2);
            }
        }
        NodeKind::Unary(_token_kind, node_id) => log(nodes, *node_id, indent + 2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_number() {
        let input = "123 ";
        let mut lexer = Lexer::new(1);
        lexer.lex(&input);

        assert!(lexer.errors.is_empty());

        let mut file_id_to_name = HashMap::new();
        file_id_to_name.insert(1, String::from("test.go"));

        let mut parser = Parser::new(input, &lexer, &file_id_to_name);
        let root_id = parser.parse_expr().unwrap();
        let root = &parser.nodes[root_id];

        assert!(parser.errors.is_empty());

        {
            assert!(matches!(root.kind, NodeKind::Number(123)));
        }
    }

    #[test]
    fn parse_add() {
        let input = "123 + 45 + 0";
        let mut lexer = Lexer::new(1);
        lexer.lex(&input);

        assert!(lexer.errors.is_empty());

        let mut file_id_to_name = HashMap::new();
        file_id_to_name.insert(1, String::from("test.go"));

        let mut parser = Parser::new(input, &lexer, &file_id_to_name);
        let root_id = parser.parse_expr().unwrap();
        let root = &parser.nodes[root_id];

        assert!(parser.errors.is_empty());

        match &root.kind {
            NodeKind::BinaryOp(
                lhs,
                Token {
                    kind: TokenKind::Plus,
                    ..
                },
                rhs,
            ) => {
                let lhs = &parser.nodes[*lhs];
                assert!(matches!(lhs.kind, NodeKind::Number(123)));
                let rhs = &parser.nodes[*rhs];
                match rhs.kind {
                    NodeKind::BinaryOp(
                        mhs,
                        Token {
                            kind: TokenKind::Plus,
                            ..
                        },
                        rhs,
                    ) => {
                        let mhs = &parser.nodes[mhs];
                        let rhs = &parser.nodes[rhs];
                        assert!(matches!(mhs.kind, NodeKind::Number(45)));
                        assert!(matches!(rhs.kind, NodeKind::Number(0)));
                    }
                    _ => panic!("Expected Add"),
                }
            }
            _ => panic!("Expected Add"),
        }
    }
}
