use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Index, IndexMut},
};

use crate::{
    error::{Error, ErrorKind},
    lex::{
        self, Declaration, DeclarationKind, Declarations, Lexer, NumberSuffix, Token, TokenKind,
    },
    origin::{FileId, Origin},
    type_checker::Type,
};
use log::trace;
use serde::Serialize;

// TODO: u32?
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct NodeId(pub(crate) usize);

#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
pub enum NodeKind {
    Unknown,
    Number(u64, NumberSuffix),
    PrimaryToken(TokenKind),
    Cast(String, NodeId),
    ProbeSpecifier(String),
    ProbeSpecifiers(Vec<NodeId>),
    ProbeDefinition(NodeId, Option<NodeId>, Option<NodeId>),
    BinaryOp(NodeId, Token, NodeId),
    Identifier(String),
    Aggregation,
    Unary(TokenKind, NodeId),
    Assignment(NodeId, Token, NodeId),
    ArgumentsExpr(Vec<NodeId>),
    ArgumentsDeclaration(Option<NodeId>),
    CommaExpr(Vec<NodeId>),
    Sizeof(NodeId, bool /* with parenthesis */),
    StringofExpr(NodeId),
    TranslationUnit(Vec<NodeId>),
    If {
        cond: NodeId,
        then_block: NodeId,
        else_block: Option<NodeId>,
    },
    Block(Vec<NodeId>),
    PostfixIncDecrement(NodeId, Token),
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
    EnumDeclaration(Option<Token>, Option<NodeId>),
    EnumeratorDeclaration(String, Option<NodeId>),
    EnumeratorsDeclaration(Vec<NodeId>),
    StructDeclaration(Option<Token>, Option<NodeId>),
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
    Character(isize),
    InlineDefinition(NodeId, NodeId, NodeId),
    ParameterTypeList {
        params: Option<NodeId>,
        ellipsis: Option<NodeId>,
    },
    ParameterDeclaration {
        param_decl_specifiers: NodeId,
        declarator: Option<NodeId>,
    },
    UnionDeclaration(Option<Token>, Option<NodeId>),
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

pub struct Parser<'a> {
    pub(crate) lexer: Lexer<'a>,
    pub(crate) nodes: Vec<Node>,
    pub(crate) node_to_type: HashMap<NodeId, Type>,
    error_mode: bool,
}

fn record_type_decl(
    decls: &mut Declarations,
    errors: &mut Vec<Error>,
    name: &str,
    kind: DeclarationKind,
    is_forward: bool,
    origin: Origin,
) {
    let conflicting = if is_forward {
        None
    } else {
        decls
            .iter()
            .rev()
            .find(|(n, decl)| !decl.is_forward && decl.kind == kind && n == name)
    };
    if let Some((_, conflicting)) = conflicting {
        errors.push(Error {
            kind: ErrorKind::Redeclaration,
            origin,
            explanation: format!("{} is already declared", name),
            related_origin: Some(conflicting.origin),
        });
    }

    let decl = Declaration {
        kind,
        origin,
        is_forward,
    };
    decls.push((name.to_owned(), decl));
}

fn lookup_type<'a>(
    decls: &'a Declarations,
    name: &'a str,
    kind: DeclarationKind,
) -> Option<&'a Declaration> {
    // Build the filtered iterator lazily; we need it twice because `find` exhausts the
    // iterator, so calling `next` on the same one after finding nothing would always return
    // `None`.
    let iter = || {
        decls
            .iter()
            .rev()
            .filter(move |(n, decl)| decl.kind == kind && n == name)
            .map(|(_, decl)| decl)
    };

    // Prefer the full (non-forward) declaration; fall back to a forward one.
    iter()
        .find(|decl| !decl.is_forward)
        .or_else(|| iter().next())
}

impl<'a> Parser<'a> {
    pub fn new(lexer: Lexer<'a>) -> Self {
        Self {
            nodes: Vec::new(),
            node_to_type: HashMap::new(),
            lexer,
            error_mode: false,
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

    fn peek1(&self) -> Token {
        let mut cpy = Lexer {
            position: self.lexer.position,
            state: self.lexer.state,
            input: self.lexer.input,
            control_directives: Vec::new(),
            comments: Vec::new(),
            errors: Vec::new(),
            attributes: Vec::new(),
            chars: self.lexer.chars.clone(),
            chars_idx: self.lexer.chars_idx,
            decls: Vec::new(),
            globals: HashMap::new(),
            identifiers: HashMap::new(),
            curly_depth: self.lexer.curly_depth,
        };
        cpy.lex()
    }

    fn peek2(&self) -> Token {
        let mut cpy = Lexer {
            position: self.lexer.position,
            state: self.lexer.state,
            input: self.lexer.input,
            control_directives: Vec::new(),
            comments: Vec::new(),
            errors: Vec::new(),
            attributes: Vec::new(),
            chars: self.lexer.chars.clone(),
            chars_idx: self.lexer.chars_idx,
            decls: Vec::new(),
            globals: HashMap::new(),
            identifiers: HashMap::new(),
            curly_depth: self.lexer.curly_depth,
        };
        let _ = cpy.lex();
        cpy.lex()
    }

    // Advance past tokens until a sync token is found, leaving the lexer positioned just
    // before it. The sync token itself is not consumed so the caller can inspect it.
    fn sync_to(&mut self, sync_tokens: &[TokenKind]) {
        loop {
            match self.peek1().kind {
                TokenKind::Eof => return,
                kind if sync_tokens.contains(&kind) => return,
                _ => {
                    self.lexer.advance(1);
                }
            }
        }
    }

    fn error(
        &mut self,
        kind: ErrorKind,
        origin: Origin,
        explanation: String,
        sync_tokens: &[TokenKind],
    ) {
        if self.error_mode {
            return;
        }

        self.lexer
            .errors
            .push(Error::new(kind, origin, explanation));
        self.error_mode = true;

        self.sync_to(sync_tokens);
    }

    fn match_kind(&mut self, kind: TokenKind) -> Option<Token> {
        let t = self.peek1();
        if t.kind == kind {
            return Some(self.lexer.lex());
        }
        None
    }

    fn match_kind1_or_kind2(&mut self, kind1: TokenKind, kind2: TokenKind) -> Option<Token> {
        let t = self.peek1();
        if t.kind == kind1 || t.kind == kind2 {
            return Some(self.lexer.lex());
        }
        None
    }

    // primary_expression      → IDENT
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

        let tok = self.peek1();
        match tok {
            Token {
                kind: TokenKind::Aggregation,
                ..
            } => {
                let tok = self.lexer.lex();
                Some(self.new_node(Node {
                    kind: NodeKind::Aggregation,
                    origin: tok.origin,
                }))
            }
            Token {
                kind: TokenKind::Identifier,
                ..
            } => {
                let tok = self.lexer.lex();

                let identifier = lex::str_from_source(self.lexer.input, tok.origin).to_owned();

                Some(self.new_node(Node {
                    kind: NodeKind::Identifier(identifier),
                    origin: tok.origin,
                }))
            }
            Token {
                kind: TokenKind::LiteralNumber(..),
                ..
            } => self.parse_literal_number(),
            Token {
                kind: TokenKind::LiteralCharacter(c),
                ..
            } => {
                let tok = self.lexer.lex();
                Some(self.new_node(Node {
                    kind: NodeKind::Character(c),
                    origin: tok.origin,
                }))
            }
            Token {
                kind:
                    TokenKind::LiteralString
                    | TokenKind::KeywordSelf
                    | TokenKind::KeywordThis
                    // Addition to avoid resolving macro argument references.
                    | TokenKind::MacroArgumentReferenceNumerical(_)
                    | TokenKind::MacroArgumentReferenceIdentifier,
                ..
            } => {
                let tok = self.lexer.lex();

                Some(self.new_node(Node {
                    kind: NodeKind::PrimaryToken(tok.kind),
                    origin: tok.origin,
                }))
            }
            Token {
                kind: TokenKind::LeftParen,
                ..
            } => {
                let left_paren = self.match_kind(TokenKind::LeftParen)?;
                let e = self.parse_expr().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingExpr,
                        left_paren.origin,
                        String::from("expected expression after parenthesis"),
                        &[TokenKind::RightParen, TokenKind::SemiColon, TokenKind::RightCurly],
                    );
                    self.new_node_unknown()
                });
                let right_paren = self.expect(
                    TokenKind::RightParen,
                    "primary expression closing parenthesis",
                );
                let end_origin = right_paren
                    .map(|t| t.origin)
                    .unwrap_or_else(|| self.origin(e));
                Some(self.new_node(Node {
                    kind: NodeKind::Unary(TokenKind::LeftParen, e),
                    origin: left_paren.origin.merge(end_origin),
                }))
            }
            _ => None,
        }
    }

    // additive_expression     → multiplicative_expression
    //                        | additive_expression "+" multiplicative_expression
    //                        | additive_expression "-" multiplicative_expression ;
    fn parse_additive_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_multiplicative_expr()?;
        while let Token {
            kind: TokenKind::Plus | TokenKind::Minus,
            ..
        } = self.peek1()
        {
            let op = self.lexer.lex();

            let rhs = match self.parse_multiplicative_expr() {
                None => {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected multiplicative expression"),
                        &[
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                            TokenKind::RightParen,
                        ],
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            let lhs_origin = self.origin(lhs);
            let rhs_origin = self.origin(rhs);
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: lhs_origin.merge(rhs_origin),
            });
        }

        Some(lhs)
    }

    // multiplicative_expression
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
            let op = self.peek1();
            match op {
                Token {
                    kind: TokenKind::Star | TokenKind::Slash | TokenKind::Percent,
                    ..
                } if self.peek2().kind != TokenKind::LeftCurly => op,
                _ => {
                    break;
                }
            };
            let op = self.lexer.lex();

            let rhs = match self.parse_cast_expr() {
                None => {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected cast expression"),
                        &[
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                            TokenKind::RightParen,
                        ],
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            let lhs_origin = self.origin(lhs);
            let rhs_origin = self.origin(rhs);
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: lhs_origin.merge(rhs_origin),
            });
        }

        Some(lhs)
    }

    // cast_expression         → unary_expression
    //                        | "(" type_name ")" cast_expression ;
    fn parse_cast_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        if let Some(op) = self.match_kind(TokenKind::LeftParen) {
            let typ = self.expect(TokenKind::Identifier, "type in cast");
            let typ_str = if let Some(typ) = typ {
                lex::str_from_source(self.lexer.input, typ.origin).to_owned()
            } else {
                String::new()
            };
            let right_paren = self.expect(TokenKind::RightParen, "closing cast right parenthesis");
            let node = self.parse_cast_expr().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingExpr,
                    right_paren.map(|t| t.origin).unwrap_or(op.origin),
                    String::from("expected expression after parenthesis in cast"),
                    &[
                        TokenKind::RightParen,
                        TokenKind::SemiColon,
                        TokenKind::RightCurly,
                    ],
                );
                self.new_node_unknown()
            });
            let node_origin = self.origin(node);
            return Some(self.new_node(Node {
                kind: NodeKind::Cast(typ_str, node),
                origin: op.origin.merge(node_origin),
            }));
        }

        self.parse_unary_expr()
    }

    // expression              → assignment_expression ( "," assignment_expression )* ;
    fn parse_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let first_expr = self.parse_assignment_expr()?;

        if self.peek1().kind != TokenKind::Comma {
            return Some(first_expr);
        }

        let mut exprs = vec![first_expr];

        while let Some(tok) = self.match_kind(TokenKind::Comma) {
            let expr = self.parse_assignment_expr().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingExpr,
                    tok.origin,
                    String::from("expected expression following comma"),
                    &[
                        TokenKind::Comma,
                        TokenKind::SemiColon,
                        TokenKind::RightCurly,
                        TokenKind::RightParen,
                    ],
                );
                self.new_node_unknown()
            });
            exprs.push(expr);
        }

        let first_origin = self.origin(exprs[0]);
        let last_origin = self.origin(*exprs.last().unwrap());
        Some(self.new_node(Node {
            kind: NodeKind::CommaExpr(exprs),
            origin: first_origin.merge(last_origin),
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

        match self.peek1() {
            Token {
                kind: TokenKind::PlusPlus | TokenKind::MinusMinus,
                ..
            } => {
                let op = self.lexer.lex();
                let unary = self.parse_unary_expr().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!("expected unary expression after {:?}", op.kind,),
                        &[TokenKind::SemiColon, TokenKind::RightCurly],
                    );
                    self.new_node_unknown()
                });
                let unary_origin = self.origin(unary);
                Some(self.new_node(Node {
                    kind: NodeKind::Unary(op.kind, unary),
                    origin: op.origin.merge(unary_origin),
                }))
            }
            Token {
                kind:
                    TokenKind::Ampersand
                    | TokenKind::Star
                    | TokenKind::Plus
                    | TokenKind::Minus
                    | TokenKind::Tilde
                    | TokenKind::Bang,
                ..
            } => {
                let op = self.lexer.lex();

                let node = match self.parse_cast_expr() {
                    None => self.new_node_unknown(),
                    Some(n) => n,
                };
                let node_origin = self.origin(node);
                Some(self.new_node(Node {
                    kind: NodeKind::Unary(op.kind, node),
                    origin: op.origin.merge(node_origin),
                }))
            }
            Token {
                kind: TokenKind::KeywordSizeof,
                ..
            } => {
                let op = self.lexer.lex();
                let left_paren = self.match_kind(TokenKind::LeftParen);

                let operand = if left_paren.is_none() {
                    // Parenthesis absent: must be a unary expression.
                    self.parse_unary_expr().unwrap_or_else(|| {
                        self.error(
                            ErrorKind::MissingExpr,
                            left_paren.map(|t| t.origin).unwrap_or(op.origin),
                            String::from("expected expression after `sizeof`"),
                            &[
                                TokenKind::SemiColon,
                                TokenKind::RightCurly,
                                TokenKind::RightParen,
                            ],
                        );
                        self.new_node_unknown()
                    })
                } else {
                    // Parenthesis present: could be a unary expression or a typename.
                    self.parse_type_name()
                        .or_else(|| self.parse_unary_expr())
                        .unwrap_or_else(|| {
                            self.error(
                                ErrorKind::MissingExprOrTypename,
                                left_paren.map(|t| t.origin).unwrap_or(op.origin),
                                String::from("expected type name or expression after `sizeof(`"),
                                &[
                                    TokenKind::RightParen,
                                    TokenKind::SemiColon,
                                    TokenKind::RightCurly,
                                ],
                            );
                            self.new_node_unknown()
                        })
                };
                // Bottom line: `sizeof typename` e.g. `sizeof int` is forbidden.

                let end_origin = if left_paren.is_some() {
                    self.expect(TokenKind::RightParen, "matching parenthesis for sizeof")
                        .map(|t| t.origin)
                        .unwrap_or(self.origin(operand))
                } else {
                    self.origin(operand)
                };

                Some(self.new_node(Node {
                    kind: NodeKind::Sizeof(operand, left_paren.is_some()),
                    origin: op.origin.merge(end_origin),
                }))
            }
            Token {
                kind: TokenKind::KeywordStringof,
                ..
            } => {
                let op = self.lexer.lex();

                let unary = self.parse_unary_expr().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected unary expression after stringof"),
                        &[TokenKind::SemiColon, TokenKind::RightCurly],
                    );
                    self.new_node_unknown()
                });
                let unary_origin = self.origin(unary);

                Some(self.new_node(Node {
                    kind: NodeKind::StringofExpr(unary),
                    origin: op.origin.merge(unary_origin),
                }))
            }

            _ => self.parse_postfix_expr(),
        }
    }

    // Certain DTrace-specific keywords may appear as struct/union member names in member-access
    // expressions.
    fn parse_keyword_as_ident(&mut self) -> Option<Token> {
        match self.peek1().kind {
            TokenKind::Identifier
            // `TypeName` can appear as a field name in member access and `offsetof`, matching the
            // grammar rules: `postfix_expression "." DT_TOK_TNAME` and
            // `DT_TOK_OFFSETOF "(" type_name "," DT_TOK_TNAME ")"`.
            | TokenKind::TypeName
            | TokenKind::KeywordProbe
            | TokenKind::KeywordProvider
            | TokenKind::KeywordSelf
            | TokenKind::KeywordString
            | TokenKind::KeywordStringof
            | TokenKind::KeywordUserland
            | TokenKind::KeywordXlate
            | TokenKind::KeywordTranslator => Some(self.lexer.lex()),
            _ => None,
        }
    }

    // postfix_expression      → primary_expression
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
        match self.peek1() {
            Token {
                kind: TokenKind::KeywordOffsetOf,
                ..
            } => {
                let op = self.lexer.lex();
                let left_paren =
                    self.expect(TokenKind::LeftParen, "opening parenthesis after offsetof");
                let type_name = self.parse_type_name().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingTypeName,
                        left_paren.map(|t| t.origin).unwrap_or(op.origin),
                        String::from("expected type name after offsetof"),
                        &[
                            TokenKind::Comma,
                            TokenKind::RightParen,
                            TokenKind::SemiColon,
                        ],
                    );
                    self.new_node_unknown()
                });
                let comma = self.expect(TokenKind::Comma, "comma after type name");
                let field = if let Some(identifier) =
                    self.match_kind1_or_kind2(TokenKind::Identifier, TokenKind::TypeName)
                {
                    identifier
                } else if let Some(keyword_as_ident) = self.parse_keyword_as_ident() {
                    keyword_as_ident
                } else {
                    self.error(
                        ErrorKind::MissingFieldOrKeywordInMemberAccess,
                        comma.map(|t| t.origin).unwrap_or(op.origin),
                        String::from("expected field or keyword as offsetof last argument"),
                        &[
                            TokenKind::RightParen,
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                        ],
                    );
                    Token::default()
                };
                let right_paren =
                    self.expect(TokenKind::RightParen, "closing parenthesis after field");
                let end_origin = right_paren.map(|t| t.origin).unwrap_or(op.origin);
                return Some(self.new_node(Node {
                    kind: NodeKind::OffsetOf(type_name, field),
                    origin: op.origin.merge(end_origin),
                }));
            }
            Token {
                kind: TokenKind::KeywordXlate,
                ..
            } => {
                let op = self.lexer.lex();
                let lt = self.expect(TokenKind::Lt, "'<' after xlate");
                let type_name = self.parse_type_name().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingTypeName,
                        lt.map(|t| t.origin).unwrap_or(op.origin),
                        String::from("expected type name after xlate"),
                        &[TokenKind::Gt, TokenKind::SemiColon, TokenKind::RightCurly],
                    );
                    self.new_node_unknown()
                });
                self.expect(TokenKind::Gt, "'>' after type name");
                let left_paren = self.expect(TokenKind::LeftParen, "opening parenthesis after '>'");
                let expr = self.parse_expr().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingExpr,
                        left_paren.map(|t| t.origin).unwrap_or(op.origin),
                        String::from("expected expression for xlate after type name"),
                        &[
                            TokenKind::RightParen,
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                        ],
                    );
                    self.new_node_unknown()
                });
                let right_paren = self.expect(
                    TokenKind::RightParen,
                    "closing parenthesis after expression",
                );
                let end_origin = right_paren.map(|t| t.origin).unwrap_or(op.origin);

                return Some(self.new_node(Node {
                    kind: NodeKind::Xlate(type_name, expr),
                    origin: op.origin.merge(end_origin),
                }));
            }
            _ => {}
        }

        let mut lhs = self.parse_primary_expr()?;

        loop {
            match self.peek1() {
                Token {
                    kind: TokenKind::LeftSquareBracket,
                    ..
                } => {
                    let lhs_origin = self.origin(lhs);
                    let op = self.lexer.lex();

                    let rhs = self.parse_argument_expr_list();
                    let right_bracket = self.expect(
                        TokenKind::RightSquareBracket,
                        "matching square bracket in argument list",
                    );
                    let end_origin = right_bracket.map(|t| t.origin).unwrap_or(op.origin);

                    lhs = self.new_node(Node {
                        kind: NodeKind::PostfixArrayAccess(lhs, rhs),
                        origin: lhs_origin.merge(end_origin),
                    });
                }
                Token {
                    kind: TokenKind::LeftParen,
                    ..
                } => {
                    let lhs_origin = self.origin(lhs);
                    let op = self.lexer.lex();

                    let rhs = self.parse_argument_expr_list();
                    let right_paren = self.expect(
                        TokenKind::RightParen,
                        "matching parenthesis in argument list",
                    );
                    let end_origin = right_paren.map(|t| t.origin).unwrap_or(op.origin);

                    lhs = self.new_node(Node {
                        kind: NodeKind::PostfixArguments(lhs, rhs),
                        origin: lhs_origin.merge(end_origin),
                    });
                }
                Token {
                    kind: TokenKind::Dot | TokenKind::Arrow,
                    ..
                } => {
                    let lhs_origin = self.origin(lhs);
                    let op = self.lexer.lex();
                    if let Some(keyword_as_ident) = self.parse_keyword_as_ident() {
                        lhs = self.new_node(Node {
                            kind: NodeKind::FieldAccess(lhs, op.kind, keyword_as_ident),
                            origin: lhs_origin.merge(keyword_as_ident.origin),
                        });
                    } else {
                        self.error(
                            ErrorKind::MissingFieldOrKeywordInMemberAccess,
                            op.origin,
                            String::from("expected identifier or keyword in member access"),
                            &[
                                TokenKind::SemiColon,
                                TokenKind::RightCurly,
                                TokenKind::RightParen,
                                TokenKind::Dot,
                                TokenKind::Arrow,
                            ],
                        );
                        lhs = self.new_node(Node {
                            kind: NodeKind::FieldAccess(lhs, op.kind, Token::default()),
                            origin: lhs_origin.merge(op.origin),
                        });
                    }
                }
                Token {
                    kind: TokenKind::PlusPlus | TokenKind::MinusMinus,
                    ..
                } => {
                    let lhs_origin = self.origin(lhs);
                    let op = self.lexer.lex();

                    lhs = self.new_node(Node {
                        kind: NodeKind::PostfixIncDecrement(lhs, op),
                        origin: lhs_origin.merge(op.origin),
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
        if self.peek1().kind != TokenKind::Comma {
            return Some(expr);
        }

        let mut args = vec![expr];
        while let Some(op) = self.match_kind(TokenKind::Comma) {
            let arg = self.parse_assignment_expr().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingExpr,
                    op.origin,
                    String::from("expected assignment expression in argument list after comma"),
                    &[
                        TokenKind::Comma,
                        TokenKind::RightParen,
                        TokenKind::SemiColon,
                    ],
                );
                self.new_node_unknown()
            });
            args.push(arg);
        }

        let first_origin = self.origin(args[0]);
        let last_origin = self.origin(*args.last().unwrap());
        Some(self.new_node(Node {
            kind: NodeKind::ArgumentsExpr(args),
            origin: first_origin.merge(last_origin),
        }))
    }

    // type_name               → specifier_qualifier_list abstract_declarator? ;
    fn parse_type_name(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }
        let specifier = self.parse_specifier_qualifier_list()?;

        let abstract_declarator = self.parse_abstract_declarator();

        let specifier_origin = self.origin(specifier);
        let end_origin = abstract_declarator
            .map(|d| self.origin(d))
            .unwrap_or(specifier_origin);
        Some(self.new_node(Node {
            kind: NodeKind::TypeName(specifier, abstract_declarator),
            origin: specifier_origin.merge(end_origin),
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

        let first_origin = self.origin(list[0]);
        let last_origin = self.origin(*list.last().unwrap());
        Some(self.new_node(Node {
            kind: NodeKind::SpecifierQualifierList(list),
            origin: first_origin.merge(last_origin),
        }))
    }

    // assignment_expression   → conditional_expression
    //                        | unary_expression assignment_operator
    //                          assignment_expression ;
    fn parse_assignment_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let lhs = self.parse_conditional_expr()?;

        match self.peek1().kind {
            TokenKind::Eq
            | TokenKind::PlusEq
            | TokenKind::MinusEq
            | TokenKind::StarEq
            | TokenKind::SlashEq
            | TokenKind::PercentEq
            | TokenKind::LtEq
            | TokenKind::GtEq
            | TokenKind::AmpersandEq
            | TokenKind::CaretEq
            | TokenKind::PipeEq => {
                let lhs_origin = self.origin(lhs);
                let op = self.lexer.lex();
                let rhs = self.parse_assignment_expr().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected expression after assignment operator"),
                        &[
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                            TokenKind::RightParen,
                            TokenKind::Comma,
                        ],
                    );
                    self.new_node_unknown()
                });
                let rhs_origin = self.origin(rhs);
                Some(self.new_node(Node {
                    kind: NodeKind::Assignment(lhs, op, rhs),
                    origin: lhs_origin.merge(rhs_origin),
                }))
            }
            _ => Some(lhs),
        }
    }

    // conditional_expression  → logical_or_expression
    //                        | logical_or_expression "?" expression
    //                          ":" conditional_expression ;
    fn parse_conditional_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let lhs = self.parse_logical_or_expr()?;

        if let Some(question_mark) = self.match_kind(TokenKind::Question) {
            let mhs = self.parse_expr().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingExpr,
                    question_mark.origin,
                    String::from("expected expression in ternary condition after question mark"),
                    &[
                        TokenKind::Colon,
                        TokenKind::SemiColon,
                        TokenKind::RightCurly,
                    ],
                );
                self.new_node_unknown()
            });
            self.expect(TokenKind::Colon, "colon in ternary expression");
            let rhs = self.parse_conditional_expr().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingExpr,
                    self.current_or_last_origin_for_err(),
                    String::from(
                        "expected conditional expression in ternary condition after colon",
                    ),
                    &[
                        TokenKind::SemiColon,
                        TokenKind::RightCurly,
                        TokenKind::RightParen,
                    ],
                );
                self.new_node_unknown()
            });
            let lhs_origin = self.origin(lhs);
            let rhs_origin = self.origin(rhs);

            Some(self.new_node(Node {
                kind: NodeKind::TernaryExpr(lhs, mhs, rhs),
                origin: lhs_origin.merge(rhs_origin),
            }))
        } else {
            Some(lhs)
        }
    }

    // logical_or_expression   → logical_xor_expression
    //                        | logical_or_expression "||" logical_xor_expression ;
    fn parse_logical_or_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_logical_xor_expr()?;
        while let Some(op) = self.match_kind(TokenKind::PipePipe) {
            let rhs = match self.parse_logical_xor_expr() {
                None => {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected logical xor expression"),
                        &[
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                            TokenKind::RightParen,
                        ],
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            let lhs_origin = self.origin(lhs);
            let rhs_origin = self.origin(rhs);
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: lhs_origin.merge(rhs_origin),
            });
        }

        Some(lhs)
    }

    // logical_xor_expression  → logical_and_expression
    //                        | logical_xor_expression "^^" logical_and_expression ;
    fn parse_logical_xor_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_logical_and_expr()?;
        while let Some(op) = self.match_kind(TokenKind::CaretCaret) {
            let rhs = match self.parse_logical_and_expr() {
                None => {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected logical and expression"),
                        &[
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                            TokenKind::RightParen,
                        ],
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            let lhs_origin = self.origin(lhs);
            let rhs_origin = self.origin(rhs);
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: lhs_origin.merge(rhs_origin),
            });
        }

        Some(lhs)
    }

    // logical_and_expression  → inclusive_or_expression
    //                        | logical_and_expression "&&" inclusive_or_expression ;
    fn parse_logical_and_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_inclusive_or_expr()?;
        while let Some(op) = self.match_kind(TokenKind::AmpersandAmpersand) {
            let rhs = match self.parse_inclusive_or_expr() {
                None => {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected logical or expression"),
                        &[
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                            TokenKind::RightParen,
                        ],
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            let lhs_origin = self.origin(lhs);
            let rhs_origin = self.origin(rhs);
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: lhs_origin.merge(rhs_origin),
            });
        }

        Some(lhs)
    }

    // inclusive_or_expression → exclusive_or_expression
    //                        | inclusive_or_expression "|" exclusive_or_expression ;
    fn parse_inclusive_or_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_exclusive_or_expr()?;
        while let Some(op) = self.match_kind(TokenKind::Pipe) {
            let rhs = match self.parse_exclusive_or_expr() {
                None => {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected exclusive or expression"),
                        &[
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                            TokenKind::RightParen,
                        ],
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            let lhs_origin = self.origin(lhs);
            let rhs_origin = self.origin(rhs);
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: lhs_origin.merge(rhs_origin),
            });
        }

        Some(lhs)
    }

    // exclusive_or_expression → and_expression
    //                        | exclusive_or_expression "^" and_expression ;
    fn parse_exclusive_or_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_and_expr()?;
        while let Some(op) = self.match_kind(TokenKind::Caret) {
            let rhs = match self.parse_and_expr() {
                None => {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected logical or expression"),
                        &[
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                            TokenKind::RightParen,
                        ],
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            let lhs_origin = self.origin(lhs);
            let rhs_origin = self.origin(rhs);
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: lhs_origin.merge(rhs_origin),
            });
        }

        Some(lhs)
    }

    // and_expression          → equality_expression
    //                        | and_expression "&" equality_expression ;
    fn parse_and_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_equality_expr()?;
        while let Some(op) = self.match_kind(TokenKind::Ampersand) {
            let rhs = match self.parse_equality_expr() {
                None => {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected equality expression"),
                        &[
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                            TokenKind::RightParen,
                        ],
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            let lhs_origin = self.origin(lhs);
            let rhs_origin = self.origin(rhs);
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: lhs_origin.merge(rhs_origin),
            });
        }

        Some(lhs)
    }

    // equality_expression     → relational_expression
    //                        | equality_expression "==" relational_expression
    //                        | equality_expression "!=" relational_expression ;
    fn parse_equality_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_relational_expr()?;
        while let Token {
            kind: TokenKind::EqEq | TokenKind::BangEq,
            ..
        } = self.peek1()
        {
            let op = self.lexer.lex();

            let rhs = match self.parse_relational_expr() {
                None => {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected equality expression"),
                        &[
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                            TokenKind::RightParen,
                        ],
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            let lhs_origin = self.origin(lhs);
            let rhs_origin = self.origin(rhs);
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: lhs_origin.merge(rhs_origin),
            });
        }

        Some(lhs)
    }

    // relational_expression   → shift_expression
    //                        | relational_expression "<"  shift_expression
    //                        | relational_expression ">"  shift_expression
    //                        | relational_expression "<=" shift_expression
    //                        | relational_expression ">=" shift_expression ;
    fn parse_relational_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_shift_expr()?;
        while let Token {
            kind: TokenKind::Gt | TokenKind::Lt | TokenKind::LtEq | TokenKind::GtEq,
            ..
        } = self.peek1()
        {
            let op = self.lexer.lex();
            let rhs = match self.parse_shift_expr() {
                None => {
                    self.error(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected equality expression"),
                        &[
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                            TokenKind::RightParen,
                        ],
                    );
                    self.new_node_unknown()
                }
                Some(x) => x,
            };
            let lhs_origin = self.origin(lhs);
            let rhs_origin = self.origin(rhs);
            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: lhs_origin.merge(rhs_origin),
            });
        }

        Some(lhs)
    }

    // shift_expression        → additive_expression
    //                        | shift_expression "<<" additive_expression
    //                        | shift_expression ">>" additive_expression ;
    fn parse_shift_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_additive_expr()?;

        while let TokenKind::LtLt | TokenKind::GtGt = self.peek1().kind {
            let op = self.lexer.lex();
            let rhs = self.parse_additive_expr().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingExpr,
                    op.origin,
                    String::from("expected additive expression after shift operator"),
                    &[
                        TokenKind::SemiColon,
                        TokenKind::RightCurly,
                        TokenKind::RightParen,
                    ],
                );
                self.new_node_unknown()
            });
            let lhs_origin = self.origin(lhs);
            let rhs_origin = self.origin(rhs);

            lhs = self.new_node(Node {
                kind: NodeKind::BinaryOp(lhs, op, rhs),
                origin: lhs_origin.merge(rhs_origin),
            });
        }

        Some(lhs)
    }

    // statement               → ";"
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

        match self.peek1().kind {
            TokenKind::KeywordIf => {
                let if_token = self.lexer.lex();

                self.expect(TokenKind::LeftParen, "opening parenthesis in if expression");
                let cond = self.parse_expr().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingExpr,
                        self.current_or_last_origin_for_err(),
                        String::from("expected expression in if"),
                        &[
                            TokenKind::RightParen,
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                        ],
                    );
                    self.new_node_unknown()
                });
                self.expect(
                    TokenKind::RightParen,
                    "closing parenthesis in if expression",
                );
                let then_block = self.parse_statement_or_block().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingStatementOrBlock,
                        self.current_or_last_origin_for_err(),
                        String::from("expected statement or block after if condition"),
                        &[TokenKind::SemiColon, TokenKind::RightCurly],
                    );
                    self.new_node_unknown()
                });

                let else_block: Option<NodeId> =
                    self.match_kind(TokenKind::KeywordElse).map(|_else_token| {
                        self.parse_statement_or_block().unwrap_or_else(|| {
                            self.error(
                                ErrorKind::MissingStatementOrBlock,
                                self.current_or_last_origin_for_err(),
                                String::from("expected statement or block after else"),
                                &[TokenKind::SemiColon, TokenKind::RightCurly],
                            );
                            self.new_node_unknown()
                        })
                    });

                let end_origin = self.origin(else_block.unwrap_or(then_block));
                Some(self.new_node(Node {
                    kind: NodeKind::If {
                        cond,
                        then_block,
                        else_block,
                    },
                    origin: if_token.origin.merge(end_origin),
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
            self.expect(
                TokenKind::RightCurly,
                "matching right curly brace after block",
            );
            return stmt_list;
        }
        self.parse_statement()
    }

    // Best effort to find the closest token when doing error reporting.
    fn current_or_last_origin_for_err(&self) -> Origin {
        self.lexer.position.into()

        //if self.tokens_consumed == self.tokens.len() {
        //    return self
        //        .tokens
        //        .last()
        //        .map(|t| t.origin)
        //        .unwrap_or_else(Origin::new_unknown);
        //}
        //
        //let token = &self.tokens[self.tokens_consumed];
        //if token.kind != TokenKind::Eof {
        //    token.origin
        //} else if self.tokens_consumed > 0 {
        //    self.tokens[self.tokens_consumed - 1].origin
        //} else {
        //    Origin::default()
        //}
    }

    fn remaining_chars_count(&self) -> usize {
        self.lexer.chars.len() - self.lexer.chars_idx
    }

    fn expect(&mut self, token_kind: TokenKind, context: &str) -> Option<Token> {
        if let Some(token) = self.match_kind(token_kind) {
            Some(token)
        } else {
            // Sync to the expected token: if it appears later in the input (e.g. a missing
            // `)` with the `)` still present further ahead), we land just before it so the
            // outer structure can still close cleanly.
            self.error(
                ErrorKind::MissingExpectedToken(token_kind),
                self.current_or_last_origin_for_err(),
                format!("failed to parse {}: missing {:?}", context, token_kind),
                &[token_kind],
            );
            None
        }
    }

    fn parse_literal_number(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let tok = self.lexer.lex();
        let TokenKind::LiteralNumber(num, suffix) = tok.kind else {
            unreachable!("parse_literal_number called on non-number token");
        };

        let node_id = self.new_node(Node {
            kind: NodeKind::Number(num, suffix),
            origin: tok.origin,
        });
        self.node_to_type.insert(node_id, Type::new_int());
        Some(node_id)
    }

    // probe_specifier         → PSPEC | INT ;
    fn parse_probe_specifier(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        if matches!(self.peek1().kind, TokenKind::LiteralNumber(..)) {
            return self.parse_literal_number();
        }

        if let Some(tok) = self.match_kind(TokenKind::ProbeSpecifier) {
            let s = lex::str_from_source(self.lexer.input, tok.origin).to_owned();
            let node_id = self.new_node(Node {
                kind: NodeKind::ProbeSpecifier(s),
                origin: tok.origin,
            });
            return Some(node_id);
        }
        if let Some(tok) = self.match_kind(TokenKind::Identifier) {
            let s = lex::str_from_source(self.lexer.input, tok.origin).to_owned();
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
        for _ in 0..self.remaining_chars_count() {
            match self.peek1().kind {
                TokenKind::RightCurly => {
                    let origin = if stmts.is_empty() {
                        self.peek1().origin
                    } else {
                        let first = self.origin(stmts[0]);
                        let last = self.origin(*stmts.last().unwrap());
                        first.merge(last)
                    };
                    return Some(self.new_node(Node {
                        kind: NodeKind::Block(stmts),
                        origin,
                    }));
                }
                TokenKind::KeywordIf => {
                    let stmt = self.parse_statement().unwrap();
                    stmts.push(stmt);
                }
                TokenKind::Eof => {
                    self.error(
                        ErrorKind::MissingStatement,
                        self.current_or_last_origin_for_err(),
                        "reached EOF while parsing statement, did you forget a semicolon or closing curly brace?"
                            .to_owned(),
                        &[TokenKind::RightCurly],
                    );
                    let origin = if stmts.is_empty() {
                        self.current_or_last_origin_for_err()
                    } else {
                        let first = self.origin(stmts[0]);
                        let last = self.origin(*stmts.last().unwrap());
                        first.merge(last)
                    };
                    return Some(self.new_node(Node {
                        kind: NodeKind::Block(stmts),
                        origin,
                    }));
                }
                TokenKind::SemiColon => {
                    let tok = self.lexer.lex();
                    stmts.push(self.new_node(Node {
                        kind: NodeKind::EmptyStmt,
                        origin: tok.origin,
                    }));
                }
                _ => {
                    let expr = self.parse_expr().unwrap_or_else(|| {
                        self.error(
                            ErrorKind::MissingExpr,
                            self.current_or_last_origin_for_err(),
                            String::from("expected expression in statement list"),
                            &[TokenKind::SemiColon, TokenKind::RightCurly],
                        );
                        self.new_node_unknown()
                    });

                    let expr_origin = self.origin(expr);
                    if let Some(tok) = self.match_kind(TokenKind::SemiColon) {
                        stmts.push(self.new_node(Node {
                            kind: NodeKind::ExprStmt(expr),
                            origin: expr_origin.merge(tok.origin),
                        }));
                    } else {
                        let block_first_origin = if stmts.is_empty() {
                            expr_origin
                        } else {
                            self.origin(stmts[0])
                        };
                        stmts.push(self.new_node(Node {
                            kind: NodeKind::ExprStmt(expr),
                            origin: expr_origin,
                        }));

                        let last_origin = self.origin(*stmts.last().unwrap());
                        return Some(self.new_node(Node {
                            kind: NodeKind::Block(stmts),
                            origin: block_first_origin.merge(last_origin),
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

        if self.peek1().kind != TokenKind::Comma {
            self.lexer.begin(lex::LexerState::InsideClauseAndExpr);
            return Some(probe_specifier);
        }
        let mut specifiers = vec![probe_specifier];

        while let Some(comma) = self.match_kind(TokenKind::Comma) {
            let specifier = self.parse_probe_specifier().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingProbeSpecifier,
                    comma.origin,
                    String::from("expected probe specifier following comma"),
                    &[TokenKind::Comma, TokenKind::LeftCurly, TokenKind::Slash],
                );
                self.new_node_unknown()
            });
            specifiers.push(specifier);
        }

        self.lexer.begin(lex::LexerState::InsideClauseAndExpr);

        let first_origin = self.origin(specifiers[0]);
        let last_origin = self.origin(*specifiers.last().unwrap());
        Some(self.new_node(Node {
            kind: NodeKind::ProbeSpecifiers(specifiers),
            origin: first_origin.merge(last_origin),
        }))
    }

    fn parse_probe_specifiers(&mut self) -> Option<NodeId> {
        self.parse_probe_specifier_list()
    }

    // probe_definition        → probe_specifiers
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
            self.expect(
                TokenKind::ClosePredicateDelimiter,
                "matching slash after predicate",
            );
            expr
        } else {
            None
        };
        let probe_specifier_origin = self.origin(probe_specifier);
        if let Some(_left_curly) = self.match_kind(TokenKind::LeftCurly) {
            let stmts = self.parse_statement_list();

            let right_curly = self.expect(
                TokenKind::RightCurly,
                "matching right curly bracket after action",
            );
            let end_origin = right_curly
                .map(|t| t.origin)
                .unwrap_or(probe_specifier_origin);

            let node_id = self.new_node(Node {
                kind: NodeKind::ProbeDefinition(probe_specifier, predicate, stmts),
                origin: probe_specifier_origin.merge(end_origin),
            });

            self.lexer.begin(lex::LexerState::ProgramOuterScope);
            return Some(node_id);
        }

        self.error(
            ErrorKind::MissingPredicateOrAction,
            self.current_or_last_origin_for_err(),
            String::from("a predicate or action must follow a probe specifier in file mode"),
            &[TokenKind::SemiColon, TokenKind::RightCurly],
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

        // TODO: translator_definition.
        // TODO: provider_definition.

        if let Some(stmt) = self.parse_inline_definition() {
            return Some(stmt);
        };

        if let Some(stmt) = self.parse_probe_definition() {
            return Some(stmt);
        };

        self.parse_declaration()
    }

    // inline_definition       → "inline" declaration_specifiers declarator
    //                          "=" assignment_expression ";" ;
    fn parse_inline_definition(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let tok = self.match_kind(TokenKind::KeywordInline)?;

        let decl_specifiers = self.parse_declaration_specifiers().unwrap_or_else(|| {
            self.error(
                ErrorKind::MissingDeclarationSpecifiers,
                tok.origin,
                String::from("expected declaration specifiers"),
                &[TokenKind::Eq, TokenKind::SemiColon],
            );
            self.new_node_unknown()
        });
        let declarator = self.parse_declarator().unwrap_or_else(|| {
            self.error(
                ErrorKind::MissingDeclarator,
                tok.origin,
                String::from("expected declarator"),
                &[TokenKind::Eq, TokenKind::SemiColon],
            );
            self.new_node_unknown()
        });

        self.expect(TokenKind::Eq, "equal sign after declarator");

        let expr = self.parse_assignment_expr().unwrap_or_else(|| {
            self.error(
                ErrorKind::MissingExpr,
                tok.origin,
                String::from("expected expression after equal sign"),
                &[TokenKind::SemiColon],
            );
            self.new_node_unknown()
        });

        let semicolon = self.expect(
            TokenKind::SemiColon,
            "semicolon at the end of an inline definition",
        );
        let end_origin = semicolon
            .map(|t| t.origin)
            .unwrap_or_else(|| self.origin(expr));

        Some(self.new_node(Node {
            kind: NodeKind::InlineDefinition(decl_specifiers, declarator, expr),
            origin: tok.origin.merge(end_origin),
        }))
    }

    fn is_at_end(&self) -> bool {
        self.peek1().kind == TokenKind::Eof
    }

    // translation_unit        → external_declaration+ ;
    fn parse_translation_unit(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let decl = self.parse_external_declaration()?;

        // Heuristic.
        let mut decls = vec![decl];

        for _i in 0..self.remaining_chars_count() {
            if self.is_at_end() {
                break;
            }
            if let Some(decl) = self.parse_external_declaration() {
                decls.push(decl);
            }
        }

        let first_origin = self.origin(decls[0]);
        let last_origin = self.origin(*decls.last().unwrap());
        let node_id = self.new_node(Node {
            kind: NodeKind::TranslationUnit(decls),
            origin: first_origin.merge(last_origin),
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

        loop {
            let token_kind = match self.peek1().kind {
                TokenKind::Eof => break,
                t => t,
            };

            if self.error_mode {
                // The error site already synced to a structural boundary; consume that
                // token so the next iteration starts at a fresh position.
                if !matches!(self.peek1().kind, TokenKind::Eof) {
                    self.lexer.lex();
                }
                self.error_mode = false;
                continue;
            }

            if let Some(prog) = self.parse_d_program() {
                return Some(prog);
            }

            if let Some(typ) = self.parse_type_name() {
                return Some(typ);
            }

            // `d_expression` is the same `expression`.
            if let Some(expr) = self.parse_expr() {
                return Some(expr);
            }

            // Catch-all: sync past the unrecognised token to a statement boundary.
            self.error(
                ErrorKind::ParseProgram,
                self.current_or_last_origin_for_err(),
                format!(
                    "catch-all parse program error: encountered unexpected token {:?}",
                    token_kind
                ),
                &[TokenKind::SemiColon, TokenKind::RightCurly],
            );
        }
        None
    }

    #[warn(unused_results)]
    pub fn parse(&mut self) -> Option<NodeId> {
        // self.resolve_nodes();

        self.parse_program()
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

        let semicolon = self.expect(TokenKind::SemiColon, "expected semicolon after declaration");

        self.lexer.begin(lex::LexerState::ProgramOuterScope);
        let start_origin = self.origin(decl_specifiers);
        let end_origin = semicolon
            .map(|t| t.origin)
            .unwrap_or_else(|| self.current_or_last_origin_for_err());
        Some(self.new_node(Node {
            kind: NodeKind::Declaration(decl_specifiers, init_decl_list),
            origin: start_origin.merge(end_origin),
        }))
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

        let first_origin = self.origin(specifiers[0]);
        let last_origin = self.origin(*specifiers.last().unwrap());
        Some(self.new_node(Node {
            kind: NodeKind::DeclarationSpecifiers(specifiers),
            origin: first_origin.merge(last_origin),
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
                self.error(
                    ErrorKind::MissingInitDeclarator,
                    comma.origin,
                    String::from("expected init declarator after comma"),
                    &[TokenKind::Comma, TokenKind::SemiColon],
                );
                self.new_node_unknown()
            });
            declarators.push(declarator);
        }

        let first_origin = self.origin(declarators[0]);
        let last_origin = self.origin(*declarators.last().unwrap());
        Some(self.new_node(Node {
            kind: NodeKind::InitDeclarators(declarators),
            origin: first_origin.merge(last_origin),
        }))
    }

    // storage_class_specifier → "auto" | "register" | "static" | "extern" | "typedef" ;
    fn parse_storage_class_specifier(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        match self.peek1().kind {
            TokenKind::KeywordAuto
            | TokenKind::KeywordRegister
            | TokenKind::KeywordStatic
            | TokenKind::KeywordExtern
            | TokenKind::KeywordTypedef => {
                let tok = self.lexer.lex();
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

        match self.peek1().kind {
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
            | TokenKind::KeywordString => {
                let tok = self.lexer.lex();
                Some(self.new_node(Node {
                    kind: NodeKind::TypeSpecifier(tok.kind),
                    origin: tok.origin,
                }))
            }
            TokenKind::TypeName => {
                // The lexer already confirmed this name is a registered typedef or struct/enum
                // name via `id_or_type`, so it is unconditionally a type specifier.
                let tok = self.lexer.lex();
                Some(self.new_node(Node {
                    kind: NodeKind::TypeSpecifier(tok.kind),
                    origin: tok.origin,
                }))
            }
            TokenKind::KeywordStruct | TokenKind::KeywordUnion => {
                self.parse_struct_or_union_specifier()
            }
            TokenKind::KeywordEnum => self.parse_enum_specifier(),
            _ => None,
        }
    }

    // type_qualifier          → "const" | "restrict" | "volatile" ;
    fn parse_type_qualifier(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        match self.peek1().kind {
            TokenKind::KeywordConst | TokenKind::KeywordRestrict | TokenKind::KeywordVolatile => {
                let tok = self.lexer.lex();
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

        match self.peek1().kind {
            TokenKind::KeywordSelf | TokenKind::KeywordThis => {
                let tok = self.lexer.lex();
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
            self.error(
                ErrorKind::MissingDirectDeclarator,
                self.current_or_last_origin_for_err(),
                String::from("expected directed declarator in declaration"),
                &[
                    TokenKind::SemiColon,
                    TokenKind::RightCurly,
                    TokenKind::Comma,
                ],
            );
            self.new_node_unknown()
        });

        let start_origin = ptr
            .map(|p| self.origin(p))
            .unwrap_or_else(|| self.origin(direct_declarator));
        let end_origin = self.origin(direct_declarator);
        Some(self.new_node(Node {
            kind: NodeKind::Declarator(ptr, direct_declarator),
            origin: start_origin.merge(end_origin),
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

        let mut lhs = match self.peek1().kind {
            TokenKind::Identifier => {
                let tok = self.lexer.lex();
                let identifier = lex::str_from_source(self.lexer.input, tok.origin).to_owned();
                let identifier_node = self.new_node(Node {
                    kind: NodeKind::Identifier(identifier),
                    origin: tok.origin,
                });
                Some(self.new_node(Node {
                    kind: NodeKind::DirectDeclarator(identifier_node, None),
                    origin: tok.origin,
                }))
            }
            TokenKind::LeftParen => {
                let left_paren = self.lexer.lex();
                let decl = self.parse_declarator().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingDeclarator,
                        left_paren.origin,
                        String::from("expected declarator after parenthesis"),
                        &[
                            TokenKind::RightParen,
                            TokenKind::SemiColon,
                            TokenKind::RightCurly,
                        ],
                    );
                    self.new_node_unknown()
                });
                self.expect(
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
        let name_tok = self.match_kind1_or_kind2(TokenKind::Identifier, TokenKind::TypeName);
        let left_curly = self.match_kind(TokenKind::LeftCurly);
        if let Some(name) = name_tok {
            let is_forward = left_curly.is_none();
            record_type_decl(
                &mut self.lexer.decls,
                &mut self.lexer.errors,
                lex::str_from_source(self.lexer.input, name.origin),
                DeclarationKind::Enum,
                is_forward,
                enum_tok.origin.merge(name.origin),
            );
        }

        let mut end_origin = name_tok.map(|t| t.origin).unwrap_or(enum_tok.origin);
        let enumerator_list: Option<NodeId> = if let Some(left_curly) = left_curly {
            let enumerator_list = self.parse_enumerator_list().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingEnumerators,
                    left_curly.origin,
                    String::from("expected at least one enumerator in enum definition"),
                    &[TokenKind::RightCurly, TokenKind::SemiColon],
                );
                self.new_node_unknown()
            });
            let right_curly = self.expect(
                TokenKind::RightCurly,
                "closing curly brace after enumerator list",
            );
            end_origin = right_curly.map(|t| t.origin).unwrap_or(left_curly.origin);
            Some(enumerator_list)
        } else {
            None
        };

        Some(self.new_node(Node {
            kind: NodeKind::EnumDeclaration(name_tok, enumerator_list),
            origin: enum_tok.origin.merge(end_origin),
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
                self.error(
                    ErrorKind::MissingEnumerator,
                    comma.origin,
                    String::from("expected enumerator following comma"),
                    &[TokenKind::Comma, TokenKind::RightCurly],
                );
                self.new_node_unknown()
            });
            enumerators.push(enumerator);
        }

        let first_origin = self.origin(enumerators[0]);
        let last_origin = self.origin(*enumerators.last().unwrap());
        Some(self.new_node(Node {
            kind: NodeKind::EnumeratorsDeclaration(enumerators),
            origin: first_origin.merge(last_origin),
        }))
    }

    fn parse_enumerator(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let identifier_tok = self.match_kind(TokenKind::Identifier)?;
        let expr = self.match_kind(TokenKind::Eq).map(|eq| {
            self.parse_conditional_expr().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingExpr,
                    eq.origin,
                    String::from("expected expression following enumerator"),
                    &[TokenKind::Comma, TokenKind::RightCurly],
                );
                self.new_node_unknown()
            })
        });

        let identifier = lex::str_from_source(self.lexer.input, identifier_tok.origin).to_owned();
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
        let tok = self.match_kind1_or_kind2(TokenKind::KeywordStruct, TokenKind::KeywordUnion)?;

        let name_tok = self.match_kind1_or_kind2(TokenKind::Identifier, TokenKind::TypeName);

        let left_curly = self.match_kind(TokenKind::LeftCurly);

        if let Some(name) = name_tok {
            let is_forward = left_curly.is_none();
            let kind = match tok.kind {
                TokenKind::KeywordStruct => DeclarationKind::Struct,
                TokenKind::KeywordUnion => DeclarationKind::Union,
                _ => unreachable!(),
            };
            record_type_decl(
                &mut self.lexer.decls,
                &mut self.lexer.errors,
                lex::str_from_source(self.lexer.input, name.origin),
                kind,
                is_forward,
                tok.origin.merge(name.origin),
            );
        }

        let mut end_origin = name_tok.map(|t| t.origin).unwrap_or(tok.origin);
        let decl_list = if let Some(left_curly) = left_curly {
            let decl_list = self.parse_struct_declaration_list().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingStructDeclarationList,
                    left_curly.origin,
                    String::from("expected unary expression after opening curly brace"),
                    &[TokenKind::RightCurly, TokenKind::SemiColon],
                );
                self.new_node_unknown()
            });
            let right_curly = self.expect(
                TokenKind::RightCurly,
                "closing curly brace after struct definition",
            );
            end_origin = right_curly.map(|t| t.origin).unwrap_or(left_curly.origin);
            Some(decl_list)
        } else {
            None
        };

        let kind = match tok.kind {
            TokenKind::KeywordStruct => NodeKind::StructDeclaration(name_tok, decl_list),
            TokenKind::KeywordUnion => NodeKind::UnionDeclaration(name_tok, decl_list),
            _ => unreachable!(),
        };
        Some(self.new_node(Node {
            kind,
            origin: tok.origin.merge(end_origin),
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

        let first_origin = self.origin(decls[0]);
        let last_origin = self.origin(*decls.last().unwrap());
        Some(self.new_node(Node {
            kind: NodeKind::StructFieldsDeclaration(decls),
            origin: first_origin.merge(last_origin),
        }))
    }

    // struct_declaration      → specifier_qualifier_list struct_declarator_list ";" ;
    fn parse_struct_declaration(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }
        let spec = self.parse_specifier_qualifier_list()?;
        let struct_declarator_list = self.parse_struct_declarator_list();
        let semicolon = self.expect(
            TokenKind::SemiColon,
            "semicolon after field in struct declaration",
        );

        let start_origin = self.origin(spec);
        let end_origin = semicolon.map(|t| t.origin).unwrap_or(start_origin);
        Some(self.new_node(Node {
            kind: NodeKind::StructFieldDeclaration(spec, struct_declarator_list),
            origin: start_origin.merge(end_origin),
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
            let decl = self.parse_struct_declarator().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingStructFieldDeclarator,
                    comma.origin,
                    String::from(
                        "expected a struct field declarator after comma in struct field declaration"
                    ),
                    &[TokenKind::Comma, TokenKind::SemiColon, TokenKind::RightCurly],
                );
                self.new_node_unknown()
            });

            decls.push(decl);
        }
        let first_origin = self.origin(decls[0]);
        let last_origin = self.origin(*decls.last().unwrap());
        Some(self.new_node(Node {
            kind: NodeKind::StructFieldDeclaratorList(decls),
            origin: first_origin.merge(last_origin),
        }))
    }

    // struct_declarator       → declarator ( ":" constant_expression )?
    fn parse_struct_declarator(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }
        let declarator = self.parse_declarator()?;

        let const_expr = if let Some(colon) = self.match_kind(TokenKind::Colon) {
            let expr = self.parse_const_expr().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingConstantExpr,
                    colon.origin,
                    String::from(
                        "expected a constant expression after colon in struct field declaration",
                    ),
                    &[TokenKind::SemiColon, TokenKind::RightCurly],
                );
                self.new_node_unknown()
            });
            Some(expr)
        } else {
            None
        };

        let start_origin = self.origin(declarator);
        let end_origin = const_expr.map(|e| self.origin(e)).unwrap_or(start_origin);
        Some(self.new_node(Node {
            kind: NodeKind::StructFieldDeclarator(declarator, const_expr),
            origin: start_origin.merge(end_origin),
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

        let mut lhs = match self.peek1().kind {
            TokenKind::LeftParen
                if matches!(
                    self.peek2(),
                    Token {
                        kind: TokenKind::Star | TokenKind::LeftParen | TokenKind::LeftSquareBracket,
                        ..
                    }
                ) =>
            {
                let tok = self.lexer.lex();

                let abstract_decl = self.parse_abstract_declarator().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingAbstractDeclarator,
                        tok.origin,
                        String::from("expected abstract declarator after parenthesis"),
                        &[TokenKind::RightParen],
                    );
                    self.new_node_unknown()
                });
                self.expect(
                    TokenKind::RightParen,
                    "matching parenthesis in direct abstract declarator",
                );

                Some(self.new_node(Node {
                    kind: NodeKind::DirectAbstractDeclarator(abstract_decl),
                    origin: tok.origin,
                }))
            }
            TokenKind::LeftParen => {
                let func = self.parse_function().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingFunction,
                        self.current_or_last_origin_for_err(),
                        String::from("expected function after opening parenthesis"),
                        &[TokenKind::RightParen],
                    );
                    self.new_node_unknown()
                });
                Some(func)
            }
            TokenKind::LeftSquareBracket => {
                let array = self.parse_array().unwrap_or_else(|| {
                    self.error(
                        ErrorKind::MissingArray,
                        self.current_or_last_origin_for_err(),
                        String::from("expected array after opening square bracket"),
                        &[TokenKind::RightSquareBracket],
                    );
                    self.new_node_unknown()
                });
                // TODO: `DirectAbstractArray(None, array)`?
                Some(array)
            }
            _ => None,
        };

        loop {
            match self.peek1().kind {
                TokenKind::LeftSquareBracket => {
                    let origin = self.peek1().origin;
                    let array = self.parse_array().unwrap_or_else(|| {
                        self.error(
                            ErrorKind::MissingArray,
                            self.current_or_last_origin_for_err(),
                            String::from("expected array after opening square bracket"),
                            &[TokenKind::RightSquareBracket],
                        );
                        self.new_node_unknown()
                    });
                    lhs = Some(self.new_node(Node {
                        kind: NodeKind::DirectAbstractArray(lhs, array),
                        origin,
                    }));
                }
                TokenKind::LeftParen
                    if !matches!(
                        self.peek2(),
                        Token {
                            kind: TokenKind::Star
                                | TokenKind::LeftParen
                                | TokenKind::LeftSquareBracket,
                            ..
                        }
                    ) =>
                {
                    let origin = self.peek1().origin;

                    let func = self.parse_array().unwrap_or_else(|| {
                        self.error(
                            ErrorKind::MissingFunction,
                            self.current_or_last_origin_for_err(),
                            String::from("expected function after opening parenthesis"),
                            &[TokenKind::RightParen],
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

        self.expect(
            TokenKind::LeftSquareBracket,
            "match square bracket for array",
        );

        Some(self.new_node(Node {
            kind: NodeKind::Array(params),
            origin: left_square_bracket.origin,
        }))
    }

    // function                → "(" function_parameters ")" ;
    fn parse_function(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let left_paren = self.match_kind(TokenKind::LeftParen)?;
        let args = self.parse_function_parameters();
        self.expect(TokenKind::RightParen, "matching parenthesis for function");

        Some(self.new_node(Node {
            kind: NodeKind::ArgumentsDeclaration(args),
            origin: left_paren.origin,
        }))
    }

    // array_parameters        → /* empty */ | constant_expression | parameter_type_list ;
    fn parse_array_parameters(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        // Empty (valid).
        if let Token {
            kind: TokenKind::RightSquareBracket,
            ..
        } = self.peek1()
        {
            return None;
        }

        self.parse_parameter_type_list()
            .or_else(|| self.parse_const_expr())
    }

    // parameter_type_list → parameter_list ( "," "..." )?
    //                      | "..." ;
    fn parse_parameter_type_list(&mut self) -> Option<NodeId> {
        // The 'empty' case has already been handled with an early return in the caller so
        // it's an error to have no parameters here.

        if let Some(tok) = self.match_kind(TokenKind::DotDotDot) {
            let param = self.new_node(Node {
                kind: NodeKind::ParamEllipsis,
                origin: tok.origin,
            });
            return Some(self.new_node(Node {
                kind: NodeKind::ParameterTypeList {
                    params: None,
                    ellipsis: Some(param),
                },
                origin: self.origin(param),
            }));
        }

        let params = self.parse_parameter_list().unwrap_or_else(|| {
            self.error(
                ErrorKind::MissingFunctionParameters,
                self.current_or_last_origin_for_err(),
                String::from("missing function parameters"),
                &[TokenKind::RightParen, TokenKind::Comma],
            );
            self.new_node_unknown()
        });

        let ellipsis = if let Some(comma) = self.match_kind(TokenKind::Comma) {
            self.expect(TokenKind::DotDotDot, "ellipsis parameter after comma");
            Some(self.new_node(Node {
                kind: NodeKind::ParamEllipsis,
                origin: comma.origin,
            }))
        } else {
            None
        };

        let params_origin = self.origin(params);
        let end_origin = ellipsis.map(|e| self.origin(e)).unwrap_or(params_origin);
        Some(self.new_node(Node {
            kind: NodeKind::ParameterTypeList {
                params: Some(params),
                ellipsis,
            },
            origin: params_origin.merge(end_origin),
        }))
    }

    // parameter_list          → parameter_declaration ( "," parameter_declaration )* ;
    fn parse_parameter_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let param = self.parse_parameter_declaration()?;
        let mut params = vec![param];

        while let Some(comma) = self.match_kind(TokenKind::Comma) {
            let param = self.parse_parameter_declaration().unwrap_or_else(|| {
                self.error(
                    ErrorKind::MissingFunctionParameter,
                    comma.origin,
                    String::from("expected function parameter after comma"),
                    &[TokenKind::Comma, TokenKind::RightParen],
                );
                self.new_node_unknown()
            });
            params.push(param);
        }

        let first_origin = self.origin(params[0]);
        let last_origin = self.origin(*params.last().unwrap());
        Some(self.new_node(Node {
            kind: NodeKind::Parameters(params),
            origin: first_origin.merge(last_origin),
        }))
    }

    // parameter_declaration → parameter_declaration_specifiers
    //                        ( declarator | abstract_declarator )? ;
    fn parse_parameter_declaration(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let param_decl_specifiers = self.parse_parameter_declaration_specifiers()?;

        let declarator = self
            .parse_declarator()
            .or_else(|| self.parse_abstract_declarator());

        let start_origin = self.origin(param_decl_specifiers);
        let end_origin = declarator.map(|d| self.origin(d)).unwrap_or(start_origin);
        Some(self.new_node(Node {
            kind: NodeKind::ParameterDeclaration {
                param_decl_specifiers,
                declarator,
            },
            origin: start_origin.merge(end_origin),
        }))
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
            self.error(
                ErrorKind::MissingParameterDeclarationSpecifiers,
                self.current_or_last_origin_for_err(),
                String::from("expected parameter declaration specifiers"),
                &[TokenKind::Comma, TokenKind::RightParen],
            );
        }

        let first_origin = specifiers
            .first()
            .map(|n| self.origin(*n))
            .unwrap_or_else(|| self.current_or_last_origin_for_err());
        let last_origin = specifiers
            .last()
            .map(|n| self.origin(*n))
            .unwrap_or(first_origin);

        Some(self.new_node(Node {
            kind: NodeKind::ParameterDeclarationSpecifiers(specifiers),
            origin: first_origin.merge(last_origin),
        }))
    }

    // function_parameters     → /* empty */ | parameter_type_list ;
    fn parse_function_parameters(&mut self) -> Option<NodeId> {
        match self.peek1().kind {
            TokenKind::RightParen | TokenKind::Eof => {
                return None;
            }
            _ => {}
        }

        if let Some(tok) = self.match_kind(TokenKind::DotDotDot) {
            return Some(self.new_node(Node {
                kind: NodeKind::ParamEllipsis,
                origin: tok.origin,
            }));
        }

        self.parse_parameter_type_list()
    }

    // constant_expression     → conditional_expression ;
    fn parse_const_expr(&mut self) -> Option<NodeId> {
        self.parse_conditional_expr()
    }
}

pub fn log(
    nodes: &[Node],
    node_id: NodeId,
    indent: usize,
    file_id_to_name: &HashMap<FileId, String>,
) {
    let node = &nodes[node_id];
    trace!(
        "{:indent$}{}: id={} kind={:?}",
        "",
        node.origin.display(file_id_to_name),
        node_id.0,
        node.kind,
        indent = indent
    );
    match &node.kind {
        NodeKind::Unknown => {}
        NodeKind::Block(node_ids) => {
            for id in node_ids {
                log(nodes, *id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::ProbeDefinition(probe, pred, actions) => {
            log(nodes, *probe, indent + 2, file_id_to_name);
            if let Some(pred) = pred {
                log(nodes, *pred, indent + 2, file_id_to_name);
            }

            if let Some(actions) = actions {
                log(nodes, *actions, indent + 2, file_id_to_name);
            }
        }
        NodeKind::Number(..) | NodeKind::Identifier(_) | NodeKind::ProbeSpecifier(_) => {}
        NodeKind::Assignment(lhs, _, rhs) | NodeKind::BinaryOp(lhs, _, rhs) => {
            log(nodes, *lhs, indent + 2, file_id_to_name);
            log(nodes, *rhs, indent + 2, file_id_to_name);
        }
        NodeKind::If {
            cond,
            then_block,
            else_block,
        } => {
            log(nodes, *cond, indent + 2, file_id_to_name);
            log(nodes, *then_block, indent + 2, file_id_to_name);
            if let Some(else_block) = else_block {
                log(nodes, *else_block, indent + 2, file_id_to_name);
            }
        }
        NodeKind::TranslationUnit(decls) => {
            for decl in decls {
                log(nodes, *decl, indent + 2, file_id_to_name);
            }
        }
        NodeKind::PrimaryToken(_) => {}
        NodeKind::Cast(_, _) => {}
        NodeKind::Aggregation => {}
        NodeKind::ProbeSpecifiers(node_ids) | NodeKind::CommaExpr(node_ids) => {
            for node in node_ids {
                log(nodes, *node, indent + 2, file_id_to_name);
            }
        }
        NodeKind::Sizeof(node_id, _) => log(nodes, *node_id, indent + 2, file_id_to_name),
        NodeKind::StringofExpr(node_id) => log(nodes, *node_id, indent + 2, file_id_to_name),
        NodeKind::PostfixIncDecrement(node_id, _token_kind) => {
            log(nodes, *node_id, indent + 2, file_id_to_name)
        }
        NodeKind::ExprStmt(node_id) => log(nodes, *node_id, indent + 2, file_id_to_name),
        NodeKind::EmptyStmt => {}
        NodeKind::PostfixArrayAccess(primary, args) | NodeKind::PostfixArguments(primary, args) => {
            log(nodes, *primary, indent + 2, file_id_to_name);
            if let Some(node_id) = args {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::TernaryExpr(lhs, mhs, rhs) => {
            log(nodes, *lhs, indent + 2, file_id_to_name);
            log(nodes, *mhs, indent + 2, file_id_to_name);
            log(nodes, *rhs, indent + 2, file_id_to_name);
        }
        NodeKind::FieldAccess(node_id, _, _) => {
            log(nodes, *node_id, indent + 2, file_id_to_name);
        }
        NodeKind::TypeName(specifier, declarator) => {
            log(nodes, *specifier, indent + 2, file_id_to_name);
            if let Some(declarator) = declarator {
                log(nodes, *declarator, indent + 2, file_id_to_name);
            }
        }
        NodeKind::OffsetOf(node_id, _token) => {
            log(nodes, *node_id, indent + 2, file_id_to_name);
        }
        NodeKind::Declaration(decl_specifiers, init_declarator_list) => {
            log(nodes, *decl_specifiers, indent + 2, file_id_to_name);
            if let Some(init_declarator_list) = init_declarator_list {
                log(nodes, *init_declarator_list, indent + 2, file_id_to_name);
            }
        }
        NodeKind::DeclarationSpecifiers(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::DirectDeclarator(base, suffix) => {
            log(nodes, *base, indent + 2, file_id_to_name);
            if let Some(node_id) = suffix {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::Declarator(ptr, declarator) => {
            if let Some(ptr) = ptr {
                log(nodes, *ptr, indent + 2, file_id_to_name);
            }
            log(nodes, *declarator, indent + 2, file_id_to_name);
        }
        NodeKind::InitDeclarators(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::TypeQualifier(_)
        | NodeKind::DStorageClassSpecifier(_)
        | NodeKind::StorageClassSpecifier(_)
        | NodeKind::TypeSpecifier(_) => {}
        NodeKind::EnumDeclaration(_token, node_id) => {
            if let Some(node_id) = node_id {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::EnumeratorDeclaration(_token, node_id) => {
            if let Some(node_id) = node_id {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::EnumeratorsDeclaration(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::UnionDeclaration(_, node_id) | NodeKind::StructDeclaration(_, node_id) => {
            if let Some(node_id) = node_id {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::StructFieldsDeclaration(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::StructFieldDeclarator(declarator, const_expr) => {
            log(nodes, *declarator, indent + 2, file_id_to_name);
            if let Some(node_id) = const_expr {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::StructFieldDeclaration(specifier_qualifier_list, declarator_list) => {
            log(
                nodes,
                *specifier_qualifier_list,
                indent + 2,
                file_id_to_name,
            );
            if let Some(node_id) = declarator_list {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::StructFieldDeclaratorList(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::SpecifierQualifierList(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::Xlate(type_name, expr) => {
            log(nodes, *type_name, indent + 2, file_id_to_name);
            log(nodes, *expr, indent + 2, file_id_to_name);
        }
        NodeKind::DirectAbstractDeclarator(node_id) => {
            log(nodes, *node_id, indent + 2, file_id_to_name);
        }
        NodeKind::DirectAbstractArray(base, suffix) => {
            if let Some(base) = base {
                log(nodes, *base, indent + 2, file_id_to_name);
            }
            log(nodes, *suffix, indent + 2, file_id_to_name);
        }
        NodeKind::DirectAbstractFunction(base, suffix) => {
            if let Some(base) = base {
                log(nodes, *base, indent + 2, file_id_to_name);
            }
            log(nodes, *suffix, indent + 2, file_id_to_name);
        }
        NodeKind::AbstractDeclarator(ptr, abstract_decl) => {
            if let Some(node_id) = ptr {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
            if let Some(node_id) = abstract_decl {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::Pointer(type_qualifiers, ptr) => {
            for node_id in type_qualifiers {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
            if let Some(node_id) = ptr {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::Array(params) => {
            if let Some(node_id) = params {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::ParamEllipsis => {}
        NodeKind::Parameters(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::ParameterDeclarationSpecifiers(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::Unary(_token_kind, node_id) => log(nodes, *node_id, indent + 2, file_id_to_name),
        NodeKind::Character(_) => {}
        NodeKind::InlineDefinition(decl_specifiers, declarator, expr) => {
            log(nodes, *decl_specifiers, indent + 2, file_id_to_name);
            log(nodes, *declarator, indent + 2, file_id_to_name);
            log(nodes, *expr, indent + 2, file_id_to_name);
        }
        NodeKind::ArgumentsExpr(node_ids) => {
            for node_id in node_ids {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::ParameterTypeList { params, ellipsis } => {
            if let Some(params) = params {
                log(nodes, *params, indent + 2, file_id_to_name);
            }
            if let Some(ellipsis) = ellipsis {
                log(nodes, *ellipsis, indent + 2, file_id_to_name);
            }
        }
        NodeKind::ArgumentsDeclaration(node_id) => {
            if let Some(node_id) = node_id {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
        NodeKind::ParameterDeclaration {
            param_decl_specifiers,
            declarator,
        } => {
            log(nodes, *param_decl_specifiers, indent + 2, file_id_to_name);
            if let Some(node_id) = declarator {
                log(nodes, *node_id, indent + 2, file_id_to_name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lex::{self, Lexer};

    const FILE_ID: u32 = 1;

    // Parses the given input as an expression and returns the parser and root node id.
    // The lexer state is set to `InsideClauseAndExpr` so that identifiers are lexed
    // correctly rather than as probe specifiers.
    fn parse_expr_input(input: &str) -> (Parser<'_>, NodeId) {
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        let root_id = parser.parse_expr().unwrap();
        (parser, root_id)
    }

    // Parses the given input as a full D program and returns the parser and root node id.
    fn parse_program_input(input: &'static str) -> (Parser<'static>, NodeId) {
        let lexer = Lexer::new(FILE_ID, input);
        let mut parser = Parser::new(lexer);
        let root_id = parser.parse().unwrap();
        (parser, root_id)
    }

    fn origin_str<'a>(input: &'a str, parser: &Parser<'_>, node_id: NodeId) -> &'a str {
        lex::str_from_source(input, parser.nodes[node_id].origin)
    }

    #[test]
    fn test_number_origin() {
        let input = "42";
        let (parser, root_id) = parse_expr_input(input);
        assert_eq!(origin_str(input, &parser, root_id), "42");
    }

    #[test]
    fn test_identifier_origin() {
        let input = "myvar";
        let (parser, root_id) = parse_expr_input(input);
        assert_eq!(origin_str(input, &parser, root_id), "myvar");
    }

    #[test]
    fn test_binary_op_add_origin() {
        let input = "1 + 2";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::BinaryOp(..)));
        assert_eq!(origin_str(input, &parser, root_id), "1 + 2");
    }

    #[test]
    fn test_binary_op_nested_origin() {
        let input = "1 + 2 + 3";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::BinaryOp(..)));
        assert_eq!(origin_str(input, &parser, root_id), "1 + 2 + 3");
    }

    #[test]
    fn test_binary_op_multiply_origin() {
        let input = "a * b";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::BinaryOp(..)));
        assert_eq!(origin_str(input, &parser, root_id), "a * b");
    }

    #[test]
    fn test_binary_op_modulo_origin() {
        let input = "3%2";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::BinaryOp(..)));
        assert_eq!(origin_str(input, &parser, root_id), "3%2");
    }

    #[test]
    fn test_binary_op_nested_precedence_origin() {
        // `1+3%2` parses as `1+(3%2)`, so the rhs `3%2` should start at column 3.
        let input = "1+3%2";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::BinaryOp(..)));
        assert_eq!(origin_str(input, &parser, root_id), "1+3%2");
        if let NodeKind::BinaryOp(_, _, rhs) = parser.nodes[root_id].kind {
            assert_eq!(origin_str(input, &parser, rhs), "3%2");
        }
    }

    #[test]
    fn test_binary_op_equality_origin() {
        let input = "x == y";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::BinaryOp(..)));
        assert_eq!(origin_str(input, &parser, root_id), "x == y");
    }

    #[test]
    fn test_binary_op_logical_and_origin() {
        let input = "a && b";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::BinaryOp(..)));
        assert_eq!(origin_str(input, &parser, root_id), "a && b");
    }

    #[test]
    fn test_binary_op_relational_origin() {
        let input = "a < b";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::BinaryOp(..)));
        assert_eq!(origin_str(input, &parser, root_id), "a < b");
    }

    #[test]
    fn test_assignment_origin() {
        let input = "x = 5";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(
            parser.nodes[root_id].kind,
            NodeKind::Assignment(..)
        ));
        assert_eq!(origin_str(input, &parser, root_id), "x = 5");
    }

    #[test]
    fn test_unary_minus_origin() {
        let input = "-1";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::Unary(..)));
        assert_eq!(origin_str(input, &parser, root_id), "-1");
    }

    #[test]
    fn test_unary_deref_origin() {
        let input = "*ptr";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::Unary(..)));
        assert_eq!(origin_str(input, &parser, root_id), "*ptr");
    }

    #[test]
    fn test_prefix_increment_origin() {
        let input = "++x";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::Unary(..)));
        assert_eq!(origin_str(input, &parser, root_id), "++x");
    }

    #[test]
    fn test_postfix_increment_origin() {
        let input = "x++";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(
            parser.nodes[root_id].kind,
            NodeKind::PostfixIncDecrement(..)
        ));
        assert_eq!(origin_str(input, &parser, root_id), "x++");
    }

    #[test]
    fn test_ternary_origin() {
        let input = "a ? b : c";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(
            parser.nodes[root_id].kind,
            NodeKind::TernaryExpr(..)
        ));
        assert_eq!(origin_str(input, &parser, root_id), "a ? b : c");
    }

    #[test]
    fn test_function_call_origin() {
        let input = "foo(1, 2)";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(
            parser.nodes[root_id].kind,
            NodeKind::PostfixArguments(..)
        ));
        assert_eq!(origin_str(input, &parser, root_id), "foo(1, 2)");
    }

    #[test]
    fn test_function_call_no_args_origin() {
        let input = "foo()";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(
            parser.nodes[root_id].kind,
            NodeKind::PostfixArguments(..)
        ));
        assert_eq!(origin_str(input, &parser, root_id), "foo()");
    }

    #[test]
    fn test_array_access_origin() {
        let input = "arr[0]";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(
            parser.nodes[root_id].kind,
            NodeKind::PostfixArrayAccess(..)
        ));
        assert_eq!(origin_str(input, &parser, root_id), "arr[0]");
    }

    #[test]
    fn test_field_access_dot_origin() {
        let input = "s.field";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(
            parser.nodes[root_id].kind,
            NodeKind::FieldAccess(..)
        ));
        assert_eq!(origin_str(input, &parser, root_id), "s.field");
    }

    #[test]
    fn test_field_access_arrow_origin() {
        let input = "p->field";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(
            parser.nodes[root_id].kind,
            NodeKind::FieldAccess(..)
        ));
        assert_eq!(origin_str(input, &parser, root_id), "p->field");
    }

    #[test]
    fn test_field_access_mixed_arrow_dot() {
        // `a->b->c.d` is `((a->b)->c).d`: two arrow dereferences followed by
        // a dot access on a struct value (not a pointer). All three must parse
        // without errors and the outermost node must span the full expression.
        let input = "curthread->last_processor->runq.count";
        let (parser, root_id) = parse_expr_input(input);
        assert!(
            parser.lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            parser.lexer.errors
        );
        // Outermost node: `.count` dot access.
        let NodeKind::FieldAccess(arrow_runq, dot, _) = parser.nodes[root_id].kind else {
            panic!(
                "expected FieldAccess at root, got {:?}",
                parser.nodes[root_id].kind
            );
        };
        assert_eq!(dot, TokenKind::Dot);
        assert_eq!(
            origin_str(input, &parser, root_id),
            "curthread->last_processor->runq.count"
        );

        // Middle node: `->runq` arrow access.
        let NodeKind::FieldAccess(arrow_last_processor, arrow, _) = parser.nodes[arrow_runq].kind
        else {
            panic!("expected FieldAccess for ->runq");
        };
        assert_eq!(arrow, TokenKind::Arrow);
        assert_eq!(
            origin_str(input, &parser, arrow_runq),
            "curthread->last_processor->runq"
        );

        // Innermost node: `->last_processor` arrow access.
        assert!(matches!(
            parser.nodes[arrow_last_processor].kind,
            NodeKind::FieldAccess(..)
        ));
        assert_eq!(
            origin_str(input, &parser, arrow_last_processor),
            "curthread->last_processor"
        );
    }

    #[test]
    fn test_sizeof_type_origin() {
        // `mytype` is lexed as an `Identifier` (not a keyword), matching what the
        // `sizeof` parser expects.
        let input = "sizeof(mytype)";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::Sizeof(..)));
        assert_eq!(origin_str(input, &parser, root_id), "sizeof(mytype)");
    }

    #[test]
    fn test_sizeof_expr_origin() {
        let input = "sizeof x";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::Sizeof(..)));
        assert_eq!(origin_str(input, &parser, root_id), "sizeof x");
    }

    #[test]
    fn test_sizeof_paren_negative_number() {
        // `sizeof(-2)` — a parenthesized expression; the parenthesized path tries
        // `parse_type_name` first, fails (no type keyword), then succeeds with
        // `parse_unary_expr` on the negation. Produces `Sizeof(Unary(Minus, 2), true)`.
        let input = "sizeof(-2)";
        let (parser, root_id) = parse_expr_input(input);
        assert!(parser.lexer.errors.is_empty());
        let NodeKind::Sizeof(operand_id, with_paren) = parser.nodes[root_id].kind else {
            panic!("expected Sizeof node");
        };
        assert!(with_paren);
        assert!(matches!(
            parser.nodes[operand_id].kind,
            NodeKind::Unary(TokenKind::Minus, _)
        ));
    }

    #[test]
    fn test_sizeof_unparenthesized_negative_number() {
        // `sizeof -2` — expression without parentheses; the no-paren path calls
        // `parse_unary_expr` directly, which handles the leading `-`. Produces
        // `Sizeof(Unary(Minus, 2), false)`.
        let input = "sizeof -2";
        let (parser, root_id) = parse_expr_input(input);
        assert!(parser.lexer.errors.is_empty());
        let NodeKind::Sizeof(operand_id, with_paren) = parser.nodes[root_id].kind else {
            panic!("expected Sizeof node");
        };
        assert!(!with_paren);
        assert!(matches!(
            parser.nodes[operand_id].kind,
            NodeKind::Unary(TokenKind::Minus, _)
        ));
    }

    #[test]
    fn test_sizeof_paren_type() {
        // `sizeof(int)` — parenthesized type name; the parenthesized path succeeds on
        // `parse_type_name` before trying `parse_unary_expr`. Produces
        // `Sizeof(TypeName(SpecifierQualifierList([TypeSpecifier(KeywordInt)]), None), true)`.
        let input = "sizeof(int)";
        let (parser, root_id) = parse_expr_input(input);
        assert!(parser.lexer.errors.is_empty());
        let NodeKind::Sizeof(operand_id, with_paren) = parser.nodes[root_id].kind else {
            panic!("expected Sizeof node");
        };
        assert!(with_paren);
        assert!(matches!(
            parser.nodes[operand_id].kind,
            NodeKind::TypeName(..)
        ));
    }

    #[test]
    fn test_sizeof_bare_type_is_error() {
        // `sizeof int` (no parentheses around a bare type name) is forbidden in DTrace D.
        // The no-paren path calls `parse_unary_expr`, which cannot consume `int` as an
        // expression, producing a `MissingExpr` error.
        let input = "sizeof int";
        let (parser, _) = parse_expr_input(input);
        assert!(!parser.lexer.errors.is_empty());
        assert!(
            parser
                .lexer
                .errors
                .iter()
                .any(|e| matches!(e.kind, ErrorKind::MissingExpr))
        );
    }

    #[test]
    fn test_stringof_origin() {
        let input = "stringof x";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(
            parser.nodes[root_id].kind,
            NodeKind::StringofExpr(..)
        ));
        assert_eq!(origin_str(input, &parser, root_id), "stringof x");
    }

    #[test]
    fn test_cast_expr_origin() {
        // The cast parser `(type)expr` greedily consumes `(identifier)` and then parses
        // the cast expression, so this tests the full origin of a cast node.
        let input = "(mytype)x";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(parser.nodes[root_id].kind, NodeKind::Cast(..)));
        assert_eq!(origin_str(input, &parser, root_id), "(mytype)x");
    }

    #[test]
    fn test_comma_expr_origin() {
        let input = "a, b, c";
        let (parser, root_id) = parse_expr_input(input);
        assert!(matches!(
            parser.nodes[root_id].kind,
            NodeKind::CommaExpr(..)
        ));
        assert_eq!(origin_str(input, &parser, root_id), "a, b, c");
    }

    #[test]
    fn test_probe_definition_hyphen_specifier() {
        // Probe names with hyphens (e.g. `profile-1000hz`) must parse without errors.
        let input = "profile:::profile-1000hz {}";
        let (parser, root_id) = parse_program_input(input);
        assert!(
            parser.lexer.errors.is_empty(),
            "unexpected errors: {:?}",
            parser.lexer.errors
        );
        let NodeKind::TranslationUnit(ref decls) = parser.nodes[root_id].kind else {
            panic!("expected TranslationUnit");
        };
        assert!(matches!(
            parser.nodes[decls[0]].kind,
            NodeKind::ProbeDefinition(..)
        ));
        assert_eq!(
            origin_str(input, &parser, decls[0]),
            "profile:::profile-1000hz {}"
        );
    }

    #[test]
    fn test_probe_definition_origin() {
        let input = "syscall::open:entry {}";
        let (parser, root_id) = parse_program_input(input);
        // Root is a TranslationUnit; get the first child.
        let NodeKind::TranslationUnit(ref decls) = parser.nodes[root_id].kind else {
            panic!("expected TranslationUnit");
        };
        let probe_id = decls[0];
        assert!(matches!(
            parser.nodes[probe_id].kind,
            NodeKind::ProbeDefinition(..)
        ));
        assert_eq!(
            origin_str(input, &parser, probe_id),
            "syscall::open:entry {}"
        );
    }

    #[test]
    fn test_if_no_else_origin() {
        let input = "syscall::open:entry { if (x) y = 1; }";
        let (parser, root_id) = parse_program_input(input);
        let NodeKind::TranslationUnit(ref decls) = parser.nodes[root_id].kind else {
            panic!("expected TranslationUnit");
        };
        let NodeKind::ProbeDefinition(_, _, Some(block_id)) = parser.nodes[decls[0]].kind else {
            panic!("expected ProbeDefinition with block");
        };
        let NodeKind::Block(ref stmts) = parser.nodes[block_id].kind else {
            panic!("expected Block");
        };
        let if_id = stmts[0];
        assert!(matches!(parser.nodes[if_id].kind, NodeKind::If { .. }));
        // The semicolon after `y = 1` is consumed as a separate `EmptyStmt` in the
        // outer block, so the `if` node's origin ends at the end of its body expression.
        assert_eq!(origin_str(input, &parser, if_id), "if (x) y = 1");
    }

    #[test]
    fn test_assignment_in_probe_body_origin() {
        // Regression test: the Assignment node's origin must start at `a`, not at `=`.
        let input = "BEGIN { a = 1; }";
        let (parser, root_id) = parse_program_input(input);
        let NodeKind::TranslationUnit(ref decls) = parser.nodes[root_id].kind else {
            panic!("expected TranslationUnit");
        };
        let NodeKind::ProbeDefinition(_, _, Some(block_id)) = parser.nodes[decls[0]].kind else {
            panic!("expected ProbeDefinition with block");
        };
        let NodeKind::Block(ref stmts) = parser.nodes[block_id].kind else {
            panic!("expected Block");
        };
        // The first statement is `a = 1;` wrapped in an ExprStmt.
        let NodeKind::ExprStmt(assign_id) = parser.nodes[stmts[0]].kind else {
            panic!("expected ExprStmt");
        };
        assert!(matches!(
            parser.nodes[assign_id].kind,
            NodeKind::Assignment(..)
        ));
        assert_eq!(origin_str(input, &parser, assign_id), "a = 1");
    }

    #[test]
    fn test_enum_decl_records_typename() {
        // Parsing `enum Color { Red, Green }` must register `Color` in the lexer's `decls`
        // so that subsequent occurrences of `Color` lex as `TypeName` rather than `Identifier`.
        let input = "enum Color { Red, Green };";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_enum_specifier();

        let lookup = lookup_type(&parser.lexer.decls, "Color", DeclarationKind::Enum).unwrap();
        assert!(!lookup.is_forward);
    }

    #[test]
    fn test_struct_decl_records_typename() {
        // Parsing `struct Point { int x; }` must register `Point` in the lexer's `decls`
        // so that subsequent occurrences of `Point` lex as `TypeName`.
        let input = "struct Point { int x; };";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_struct_or_union_specifier();

        let lookup = lookup_type(&parser.lexer.decls, "Point", DeclarationKind::Struct).unwrap();
        assert!(!lookup.is_forward);
    }

    #[test]
    fn test_union_decl_records_typename() {
        // Parsing `union Data { int i; }` must register `Data` in the lexer's `decls`
        // so that subsequent occurrences of `Data` lex as `TypeName`.
        let input = "union Data { int i; };";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_struct_or_union_specifier();

        let lookup = lookup_type(&parser.lexer.decls, "Data", DeclarationKind::Union).unwrap();
        assert!(!lookup.is_forward);
    }

    #[test]
    fn test_enum_decl_without_name_records_nothing() {
        // An anonymous enum `enum { Red }` has no name to register.
        let input = "enum { Red };";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_enum_specifier();
        assert!(lookup_type(&parser.lexer.decls, "", DeclarationKind::Enum).is_none());
        assert!(lookup_type(&parser.lexer.decls, "Red", DeclarationKind::Enum).is_none());
    }

    #[test]
    fn test_struct_decl_without_name_records_nothing() {
        // An anonymous struct `struct { int x; }` has no name to register.
        let input = "struct { int x; };";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_struct_or_union_specifier();
        assert!(lookup_type(&parser.lexer.decls, "", DeclarationKind::Struct).is_none());
        assert!(lookup_type(&parser.lexer.decls, "x", DeclarationKind::Struct).is_none());
    }

    #[test]
    fn test_struct_bare_name_registers_as_forward() {
        // `struct Person` with no `{` must register `Person` as `Forward`, not `Struct`, so
        // that a later full definition can upgrade the entry without triggering a redefinition
        // error.
        let input = "struct Person;";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_struct_or_union_specifier();

        let lookup = lookup_type(&parser.lexer.decls, "Person", DeclarationKind::Struct).unwrap();
        assert!(lookup.is_forward);
    }

    #[test]
    fn test_enum_bare_name_registers_as_forward() {
        let input = "enum Color;";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_enum_specifier();
        let lookup = lookup_type(&parser.lexer.decls, "Color", DeclarationKind::Enum).unwrap();
        assert!(lookup.is_forward);
    }

    #[test]
    fn test_struct_forward_then_full_def_upgrades_kind_and_origin() {
        // A forward declaration followed by a full definition must upgrade the entry from
        // `Forward` to `Struct`. The stored origin must be from the full definition (line 2),
        // since that declaration carries the most information (field list).
        let input = "struct Person;\nstruct Person { int age; }";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_struct_or_union_specifier(); // consumes `struct Person` (no `{`)
        parser.lexer.lex(); // skip `;`
        parser.parse_struct_or_union_specifier(); // consumes `struct Person { int age; }`

        let lookup = lookup_type(&parser.lexer.decls, "Person", DeclarationKind::Struct).unwrap();
        assert!(!lookup.is_forward);

        // The origin must point to the full `struct Person` span in the full definition (line 2),
        // not in the forward declaration (line 1).
        assert_eq!(lookup.origin.start.line, 2);
        assert_eq!(lex::str_from_source(input, lookup.origin), "struct Person");
    }

    #[test]
    fn test_enum_forward_then_full_def_upgrades_kind_and_origin() {
        let input = "enum Color;\nenum Color { Red, Green }";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_enum_specifier();
        parser.lexer.lex(); // skip `;`
        parser.parse_enum_specifier();

        let lookup = lookup_type(&parser.lexer.decls, "Color", DeclarationKind::Enum).unwrap();
        assert!(!lookup.is_forward);
        assert_eq!(lookup.origin.start.line, 2);
        assert_eq!(lex::str_from_source(input, lookup.origin), "enum Color");
    }

    #[test]
    fn test_struct_ref_in_offsetof_does_not_overwrite_full_def() {
        let input =
            "struct Person { int age; int id; };\nBEGIN { print(offsetof(struct Person, id)); }";
        let (parser, _) = parse_program_input(input);

        let lookup = lookup_type(&parser.lexer.decls, "Person", DeclarationKind::Struct).unwrap();
        assert!(!lookup.is_forward);
        // Origin must still point to the full definition on line 1.
        assert_eq!(lookup.origin.start.line, 1);
        assert_eq!(lex::str_from_source(input, lookup.origin), "struct Person");
    }

    #[test]
    fn test_struct_redeclaration_produces_error() {
        // Redefining a struct with a body is a redeclaration error. The error origin must span
        // the second `struct Name` (the offending site), and `related_origin` must span the
        // first `struct Name` (the original declaration), so the diagnostics can point to both.
        let input = "struct Person { int age; }\nstruct Person { int id; }";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_struct_or_union_specifier();
        parser.parse_struct_or_union_specifier();

        assert_eq!(parser.lexer.errors.len(), 1);
        let err = &parser.lexer.errors[0];
        assert_eq!(err.kind, ErrorKind::Redeclaration);
        // Error origin: second `struct Person`, on line 2.
        assert_eq!(err.origin.start.line, 2);
        assert_eq!(lex::str_from_source(input, err.origin), "struct Person");
        // Related origin: first `struct Person`, on line 1.
        let related = err.related_origin.unwrap();
        assert_eq!(related.start.line, 1);
        assert_eq!(lex::str_from_source(input, related), "struct Person");
    }

    #[test]
    fn test_union_redeclaration_produces_error() {
        let input = "union Data { int i; }\nunion Data { char c; }";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_struct_or_union_specifier();
        parser.parse_struct_or_union_specifier();

        assert_eq!(parser.lexer.errors.len(), 1);
        let err = &parser.lexer.errors[0];
        assert_eq!(err.kind, ErrorKind::Redeclaration);
        assert_eq!(err.origin.start.line, 2);
        assert_eq!(lex::str_from_source(input, err.origin), "union Data");
        let related = err.related_origin.unwrap();
        assert_eq!(related.start.line, 1);
        assert_eq!(lex::str_from_source(input, related), "union Data");
    }

    #[test]
    fn test_enum_redeclaration_produces_error() {
        let input = "enum Color { Red }\nenum Color { Blue }";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_enum_specifier();
        parser.parse_enum_specifier();

        assert_eq!(parser.lexer.errors.len(), 1);
        let err = &parser.lexer.errors[0];
        assert_eq!(err.kind, ErrorKind::Redeclaration);
        assert_eq!(err.origin.start.line, 2);
        assert_eq!(lex::str_from_source(input, err.origin), "enum Color");
        let related = err.related_origin.unwrap();
        assert_eq!(related.start.line, 1);
        assert_eq!(lex::str_from_source(input, related), "enum Color");
    }

    #[test]
    fn test_forward_decl_then_redeclaration_produces_error() {
        // Defining a struct twice after a forward declaration: the forward decl upgrades
        // silently, but the second full definition is a redeclaration error.
        let input = "struct Person;\nstruct Person { int age; }\nstruct Person { int id; }";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_struct_or_union_specifier();
        parser.lexer.lex(); // skip `;`
        parser.parse_struct_or_union_specifier();
        parser.parse_struct_or_union_specifier();

        assert_eq!(parser.lexer.errors.len(), 1);
        let err = &parser.lexer.errors[0];
        assert_eq!(err.kind, ErrorKind::Redeclaration);
        // Error on the third declaration (line 3), related to the second (line 2).
        assert_eq!(err.origin.start.line, 3);
        let related = err.related_origin.unwrap();
        assert_eq!(related.start.line, 2);
    }

    #[test]
    fn test_cross_kind_forward_decls_do_not_conflict() {
        // Two forward declarations with the same name but different kinds are valid in DTrace:
        // `struct Person; enum Person` compiles without errors.
        let input = "struct Person;\nenum Person;";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_struct_or_union_specifier();
        parser.lexer.lex(); // skip `;`
        parser.parse_enum_specifier();

        assert!(parser.lexer.errors.is_empty());
    }

    #[test]
    fn test_same_name_different_kind_is_allowed() {
        // DTrace allows the same tag name for different type kinds:
        // `struct Person{int x;}; enum Person{ORANGE}` is valid.
        let input = "struct Person { int x; }\nenum Person { ORANGE }";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_struct_or_union_specifier();
        parser.parse_enum_specifier();

        assert!(parser.lexer.errors.is_empty());
    }

    #[test]
    fn test_same_name_same_kind_interleaved_with_other_kind_is_redeclaration() {
        // The flat `Declarations` list makes it possible to detect a same-kind redeclaration
        // even when a different-kind declaration with the same name appears in between. With
        // the old HashMap approach the first `enum Color` would have been overwritten.
        //
        //   enum Color { Purple };   <- first enum Color, no error
        //   struct Color { int x; }; <- different kind, allowed
        //   enum Color { Orange };   <- redeclaration of enum Color (same kind as line 1)
        let input = "enum Color { Purple }\nstruct Color { int x; }\nenum Color { Orange }";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        parser.parse_enum_specifier();
        parser.parse_struct_or_union_specifier();
        parser.parse_enum_specifier();

        assert_eq!(parser.lexer.errors.len(), 1);
        let err = &parser.lexer.errors[0];
        assert_eq!(err.kind, ErrorKind::Redeclaration);
        // The offending declaration is the second `enum Color` on line 3.
        assert_eq!(err.origin.start.line, 3);
        // The related origin points back to the first `enum Color` on line 1.
        assert_eq!(err.related_origin.unwrap().start.line, 1);
    }

    #[test]
    fn test_struct_with_anonymous_union_field() {
        // `struct Outer { union { int i; } foo; }` — the field's type specifier is an anonymous
        // union (no name token), and the field declarator is named `foo`. The anonymous union
        // must not produce any entry in `decls`; only `Outer` must be registered.
        let input = "struct Outer { union { int i; } foo; };";
        let mut lexer = Lexer::new(FILE_ID, input);
        lexer.begin(lex::LexerState::InsideClauseAndExpr);
        let mut parser = Parser::new(lexer);
        let root_id = parser.parse_struct_or_union_specifier().unwrap();

        assert!(parser.lexer.errors.is_empty());

        // The outer struct must be registered as a non-forward `Struct`.
        let outer = lookup_type(&parser.lexer.decls, "Outer", DeclarationKind::Struct).unwrap();
        assert!(!outer.is_forward);
        // The anonymous union leaves no named entry.
        assert_eq!(parser.lexer.decls.len(), 1);

        // Root: StructDeclaration with name token "Outer" and a body.
        let NodeKind::StructDeclaration(Some(outer_tok), Some(fields_id)) =
            parser.nodes[root_id].kind
        else {
            panic!("expected StructDeclaration with name and body");
        };
        assert_eq!(lex::str_from_source(input, outer_tok.origin), "Outer");

        // Body: StructFieldsDeclaration with exactly one field.
        let NodeKind::StructFieldsDeclaration(ref field_ids) = parser.nodes[fields_id].kind else {
            panic!("expected StructFieldsDeclaration");
        };
        assert_eq!(field_ids.len(), 1);

        // The single field: StructFieldDeclaration(specifier_qualifier_list, declarator_list).
        let NodeKind::StructFieldDeclaration(spec_id, Some(decl_list_id)) =
            parser.nodes[field_ids[0]].kind
        else {
            panic!("expected StructFieldDeclaration with declarator list");
        };

        // Specifier: SpecifierQualifierList containing the anonymous union.
        let NodeKind::SpecifierQualifierList(ref specs) = parser.nodes[spec_id].kind else {
            panic!("expected SpecifierQualifierList");
        };
        assert_eq!(specs.len(), 1);
        // The sole specifier must be an anonymous union (no name token).
        let NodeKind::UnionDeclaration(None, Some(_)) = parser.nodes[specs[0]].kind else {
            panic!("expected anonymous StructDeclaration (union with no name) in specifier");
        };

        // Declarator list: StructFieldDeclaratorList with one entry named "foo".
        let NodeKind::StructFieldDeclaratorList(ref declarators) = parser.nodes[decl_list_id].kind
        else {
            panic!("expected StructFieldDeclaratorList");
        };
        assert_eq!(declarators.len(), 1);
        let NodeKind::StructFieldDeclarator(declarator_id, None) =
            parser.nodes[declarators[0]].kind
        else {
            panic!("expected StructFieldDeclarator without bit-field");
        };
        assert_eq!(origin_str(input, &parser, declarator_id), "foo");
    }

    #[test]
    fn test_assignment_in_probe_body_multiline_origin() {
        // Regression test: the Assignment node's origin must start at `a` (column 3),
        // not at `=` (column 5), even when the probe body is on multiple lines.
        let input = "BEGIN {\n  a = 1;\n}";
        let (parser, root_id) = parse_program_input(input);
        let NodeKind::TranslationUnit(ref decls) = parser.nodes[root_id].kind else {
            panic!("expected TranslationUnit");
        };
        let NodeKind::ProbeDefinition(_, _, Some(block_id)) = parser.nodes[decls[0]].kind else {
            panic!("expected ProbeDefinition with block");
        };
        let NodeKind::Block(ref stmts) = parser.nodes[block_id].kind else {
            panic!("expected Block");
        };
        let NodeKind::ExprStmt(assign_id) = parser.nodes[stmts[0]].kind else {
            panic!("expected ExprStmt");
        };
        assert!(matches!(
            parser.nodes[assign_id].kind,
            NodeKind::Assignment(..)
        ));
        assert_eq!(origin_str(input, &parser, assign_id), "a = 1");
        // `a` is at line 2, column 3.
        assert_eq!(parser.nodes[assign_id].origin.start.line, 2);
        assert_eq!(parser.nodes[assign_id].origin.start.column, 3);
    }
}
