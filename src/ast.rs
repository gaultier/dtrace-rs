use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    num::ParseIntError,
    ops::{Index, IndexMut},
};

use crate::{
    error::{Error, ErrorKind},
    lex::{self, Lexer, Token, TokenKind},
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
    ArgumentsExpr(Vec<NodeId>),
    ArgumentsDeclaration(Option<NodeId>),
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
    pub(crate) lexer: Lexer<'a>,
    pub(crate) nodes: Vec<Node>,
    pub(crate) node_to_type: HashMap<NodeId, Type>,
    pub(crate) name_to_def: NameToDef,
    pub(crate) typenames: HashSet<String>,
    error_mode: bool,
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
    pub fn new(lexer: Lexer<'a>) -> Self {
        Self {
            nodes: Vec::new(),
            node_to_type: HashMap::new(),
            name_to_def: NameToDef::new(),
            typenames: HashSet::new(),
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

    fn peek(&self) -> Token {
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
        };
        cpy.lex()
    }

    fn peek_peek(&self) -> Token {
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
        };
        let _ = cpy.lex();
        cpy.lex()
    }

    // Used to avoid an avalanche of errors for the same line.
    fn skip_to_next_line(&mut self) {
        // TODO: Could just in the lexer skip to the next '\n' char.
        let current_line = self.lexer.position.line;

        loop {
            match self.peek() {
                Token {
                    kind: TokenKind::Eof,
                    ..
                } => return,
                Token { origin, .. } if origin.start.line > current_line => {
                    return;
                }
                _ => {
                    self.lexer.advance(1);
                }
            };
        }
    }

    fn add_error_with_explanation(&mut self, kind: ErrorKind, origin: Origin, explanation: String) {
        if self.error_mode {
            return;
        }

        self.lexer
            .errors
            .push(Error::new(kind, origin, explanation));
        self.error_mode = true;

        // Skip to the next newline to avoid having cascading errors.
        self.skip_to_next_line();
    }

    fn match_kind(&mut self, kind: TokenKind) -> Option<Token> {
        let t = self.peek();
        if t.kind == kind {
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

        let tok = self.peek();
        match tok {
            Token {
                kind: TokenKind::Identifier,
                ..
            } => {
                let tok = self.lexer.lex();

                let identifier = lex::str_from_source(self.lexer.input, tok.origin).to_owned();

                Some(self.new_node(Node {
                    kind: if identifier.starts_with("@") {
                        NodeKind::Aggregation(identifier)
                    } else {
                        NodeKind::Identifier(identifier)
                    },
                    origin: tok.origin,
                }))
            }
            Token {
                kind: TokenKind::LiteralNumber,
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
                    | TokenKind::MacroArgumentReference(_), /* Addition to avoid resolving macro argument references. */
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
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        left_paren.origin,
                        String::from("expected expression after parenthesis"),
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
        } = self.peek()
        {
            let op = self.lexer.lex();

            let rhs = match self.parse_multiplicative_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected multiplicative expression"),
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
            let op = self.peek();
            match op {
                Token {
                    kind: TokenKind::Star | TokenKind::Slash | TokenKind::Percent,
                    ..
                } if self.peek_peek().kind != TokenKind::LeftCurly => op,
                _ => {
                    break;
                }
            };
            let op = self.lexer.lex();

            let rhs = match self.parse_cast_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected cast expression"),
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

    // cast_expression         → unary_expression
    //                        | "(" type_name ")" cast_expression ;
    fn parse_cast_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        if let Some(op) = self.match_kind(TokenKind::LeftParen) {
            let typ = self.expect_token_one(TokenKind::Identifier, "type in cast");
            let typ_str = if let Some(typ) = typ {
                lex::str_from_source(self.lexer.input, typ.origin).to_owned()
            } else {
                String::new()
            };
            let right_paren =
                self.expect_token_one(TokenKind::RightParen, "closing cast right parenthesis");
            let node = self.parse_cast_expr().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingExpr,
                    right_paren.map(|t| t.origin).unwrap_or(op.origin),
                    String::from("expected expression after parenthesis in cast"),
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

    // expression              → assignment_expression ( "," assignment_expression )* ;
    fn parse_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let first_expr = self.parse_assignment_expr()?;

        if self.peek().kind != TokenKind::Comma {
            return Some(first_expr);
        }

        let mut exprs = vec![first_expr];
        let first_comma_origin = self.peek().origin;

        while let Some(tok) = self.match_kind(TokenKind::Comma) {
            let expr = self.parse_assignment_expr().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingExpr,
                    tok.origin,
                    String::from("expected expression following comma"),
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
            Token {
                kind: TokenKind::PlusPlus | TokenKind::MinusMinus,
                ..
            } => {
                let op = self.lexer.lex();
                let unary = self.parse_unary_expr().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!("expected unary expression after {:?}", op.kind,),
                    );
                    self.new_node_unknown()
                });
                Some(self.new_node(Node {
                    kind: NodeKind::Unary(op.kind, unary),
                    origin: op.origin,
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
                Some(self.new_node(Node {
                    kind: NodeKind::Unary(op.kind, node),
                    origin: op.origin,
                }))
            }
            Token {
                kind: TokenKind::KeywordSizeof,
                ..
            } => {
                let op = self.lexer.lex();

                if self.match_kind(TokenKind::LeftParen).is_some() {
                    let typename = self
                        .expect_token_one(TokenKind::Identifier, "type name for sizeof")
                        .map(|t| lex::str_from_source(self.lexer.input, t.origin).to_owned())
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
                            String::from("expected unary expression after sizeof"),
                        );
                        self.new_node_unknown()
                    });

                    Some(self.new_node(Node {
                        kind: NodeKind::SizeofExpr(unary),
                        origin: op.origin,
                    }))
                }
            }
            Token {
                kind: TokenKind::KeywordStringof,
                ..
            } => {
                let op = self.lexer.lex();

                let unary = self.parse_unary_expr().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected unary expression after stringof"),
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
        match self.peek().kind {
            TokenKind::Identifier
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
        match self.peek() {
            Token {
                kind: TokenKind::KeywordOffsetOf,
                ..
            } => {
                let op = self.lexer.lex();
                let left_paren = self
                    .expect_token_one(TokenKind::LeftParen, "opening parenthesis after offsetof");
                let type_name = self.parse_type_name().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingTypeName,
                        left_paren.map(|t| t.origin).unwrap_or(op.origin),
                        String::from("expected type name after offsetof"),
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
                        String::from("expected field or keyword as offsetof last argument"),
                    );
                    Token::default()
                };
                self.expect_token_one(TokenKind::RightParen, "closing parenthesis after field");
                return Some(self.new_node(Node {
                    kind: NodeKind::OffsetOf(type_name, field),
                    origin: op.origin,
                }));
            }
            Token {
                kind: TokenKind::KeywordXlate,
                ..
            } => {
                let op = self.lexer.lex();
                let lt = self.expect_token_one(TokenKind::Lt, "'<' after xlate");
                let type_name = self.parse_type_name().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingTypeName,
                        lt.map(|t| t.origin).unwrap_or(op.origin),
                        String::from("expected type name after xlate"),
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
                        String::from("expected expression for xlate after type name"),
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
                Token {
                    kind: TokenKind::LeftSquareBracket,
                    ..
                } => {
                    let op = self.lexer.lex();

                    let rhs = self.parse_argument_expr_list();
                    self.expect_token_one(
                        TokenKind::RightSquareBracket,
                        "matching square bracket in argument list",
                    );

                    lhs = self.new_node(Node {
                        kind: NodeKind::PostfixArrayAccess(lhs, rhs),
                        origin: op.origin,
                    });
                }
                Token {
                    kind: TokenKind::LeftParen,
                    ..
                } => {
                    let op = self.lexer.lex();

                    let rhs = self.parse_argument_expr_list();
                    self.expect_token_one(
                        TokenKind::RightParen,
                        "matching parenthesis in argument list",
                    );

                    lhs = self.new_node(Node {
                        kind: NodeKind::PostfixArguments(lhs, rhs),
                        origin: op.origin,
                    });
                }
                Token {
                    kind: TokenKind::Dot | TokenKind::Arrow,
                    ..
                } => {
                    let op = self.lexer.lex();
                    if let Some(keyword_as_ident) = self.parse_keyword_as_ident() {
                        lhs = self.new_node(Node {
                            kind: NodeKind::FieldAccess(lhs, op.kind, keyword_as_ident),
                            origin: op.origin,
                        });
                    } else {
                        self.add_error_with_explanation(
                            ErrorKind::MissingFieldOrKeywordInMemberAccess,
                            op.origin,
                            String::from("expected identifier or keyword in member access"),
                        );
                        lhs = self.new_node(Node {
                            kind: NodeKind::FieldAccess(lhs, op.kind, Token::default()),
                            origin: op.origin,
                        });
                    }
                }
                Token {
                    kind: TokenKind::PlusPlus | TokenKind::MinusMinus,
                    ..
                } => {
                    let op = self.lexer.lex();

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
        let first_comma_origin = if self.peek().kind != TokenKind::Comma {
            return Some(expr);
        } else {
            self.peek().origin
        };

        let mut args = vec![expr];
        while let Some(op) = self.match_kind(TokenKind::Comma) {
            let arg = self.parse_assignment_expr().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingExpr,
                    op.origin,
                    String::from("expected assignment expression in argument list after comma"),
                );
                self.new_node_unknown()
            });
            args.push(arg);
        }

        Some(self.new_node(Node {
            kind: NodeKind::ArgumentsExpr(args),
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

    // assignment_expression   → conditional_expression
    //                        | unary_expression assignment_operator
    //                          assignment_expression ;
    fn parse_assignment_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let lhs = self.parse_conditional_expr()?;

        match self.peek().kind {
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
                let op = self.lexer.lex();
                let rhs = self.parse_assignment_expr().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected expression after assignment operator"),
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
                self.add_error_with_explanation(
                    ErrorKind::MissingExpr,
                    question_mark.origin,
                    String::from("expected expression in ternary condition after question mark"),
                );
                self.new_node_unknown()
            });
            self.expect_token_one(TokenKind::Colon, "colon in ternary expression");
            let rhs = self.parse_conditional_expr().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingExpr,
                    self.current_or_last_origin_for_err(),
                    String::from(
                        "expected conditional expression in ternary condition after colon",
                    ),
                );
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
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected logical xor expression"),
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
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected logical and expression"),
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
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected logical or expression"),
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
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected exclusive or expression"),
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
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected logical or expression"),
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
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected equality expression"),
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
        } = self.peek()
        {
            let op = self.lexer.lex();

            let rhs = match self.parse_relational_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected equality expression"),
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
        } = self.peek()
        {
            let op = self.lexer.lex();
            let rhs = match self.parse_shift_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        op.origin,
                        String::from("expected equality expression"),
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

    // shift_expression        → additive_expression
    //                        | shift_expression "<<" additive_expression
    //                        | shift_expression ">>" additive_expression ;
    fn parse_shift_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut lhs = self.parse_additive_expr()?;

        while let TokenKind::LtLt | TokenKind::GtGt = self.peek().kind {
            let op = self.lexer.lex();
            let rhs = self.parse_additive_expr().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingExpr,
                    op.origin,
                    String::from("expected additive expression after shift operator"),
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

        match self.peek().kind {
            TokenKind::KeywordIf => {
                let if_token = self.lexer.lex();

                self.expect_token_one(TokenKind::LeftParen, "opening parenthesis in if expression");
                let cond = self.parse_expr().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpr,
                        self.current_or_last_origin_for_err(),
                        String::from("expected expression in if"),
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
                        String::from("expected statement or block after if condition"),
                    );
                    self.new_node_unknown()
                });

                let else_block: Option<NodeId> =
                    self.match_kind(TokenKind::KeywordElse).map(|_else_token| {
                        self.parse_statement_or_block().unwrap_or_else(|| {
                            self.add_error_with_explanation(
                                ErrorKind::MissingStatementOrBlock,
                                self.current_or_last_origin_for_err(),
                                String::from("expected statement or block after else"),
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

        let tok = self.lexer.lex();
        let src = lex::str_from_source(self.lexer.input, tok.origin);
        let num: u64 = str::parse(src)
            .map_err(|err: ParseIntError| {
                self.add_error_with_explanation(
                    ErrorKind::InvalidLiteralNumber,
                    tok.origin,
                    err.to_string(),
                );
            })
            .ok()?;

        let node_id = self.new_node(Node {
            kind: NodeKind::Number(num),
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

        if self.peek().kind == TokenKind::LiteralNumber {
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
            match self.peek().kind {
                TokenKind::RightCurly => {
                    let origin = self.peek().origin;
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
                    self.add_error_with_explanation(
                        ErrorKind::MissingStatement,
                        self.current_or_last_origin_for_err(),
                        "reached EOF while parsing statement, did you forget a semicolon or closing curly brace?"
                            .to_owned(),
                    );
                    return Some(self.new_node(Node {
                        kind: NodeKind::Block(stmts),
                        origin: self.current_or_last_origin_for_err(),
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
                    let origin = self.peek().origin;

                    let expr = self.parse_expr().unwrap_or_else(|| {
                        self.add_error_with_explanation(
                            ErrorKind::MissingExpr,
                            self.current_or_last_origin_for_err(),
                            String::from("expected expression in statement list"),
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
                            origin: Origin::default(),
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

        if self.peek().kind != TokenKind::Comma {
            self.lexer.begin(lex::LexerState::InsideClauseAndExpr);
            return Some(probe_specifier);
        }
        let first_comma_origin = self.peek().origin;
        let mut specifiers = vec![probe_specifier];

        while let Some(comma) = self.match_kind(TokenKind::Comma) {
            let specifier = self.parse_probe_specifier().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingProbeSpecifier,
                    comma.origin,
                    String::from("expected probe specifier following comma"),
                );
                self.new_node_unknown()
            });
            specifiers.push(specifier);
        }

        self.lexer.begin(lex::LexerState::InsideClauseAndExpr);

        Some(self.new_node(Node {
            kind: NodeKind::ProbeSpecifiers(specifiers),
            origin: first_comma_origin,
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
            self.expect_token_one(
                TokenKind::ClosePredicateDelimiter,
                "matching slash after predicate",
            );
            expr
        } else {
            None
        };
        if let Some(left_curly) = self.match_kind(TokenKind::LeftCurly) {
            let stmts = self.parse_statement_list();

            self.expect_token_one(
                TokenKind::RightCurly,
                "matching right curly bracket after action",
            );

            let node_id = self.new_node(Node {
                kind: NodeKind::ProbeDefinition(probe_specifier, predicate, stmts),
                origin: left_curly.origin,
            });

            self.lexer.begin(lex::LexerState::ProgramOuterScope);
            return Some(node_id);
        }

        self.add_error_with_explanation(
            ErrorKind::MissingPredicateOrAction,
            self.current_or_last_origin_for_err(),
            String::from("a predicate or action must follow a probe specifier in file mode"),
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
            self.add_error_with_explanation(
                ErrorKind::MissingDeclarationSpecifiers,
                tok.origin,
                String::from("expected declaration specifiers"),
            );
            self.new_node_unknown()
        });
        let declarator = self.parse_declarator().unwrap_or_else(|| {
            self.add_error_with_explanation(
                ErrorKind::MissingDeclarator,
                tok.origin,
                String::from("expected declarator"),
            );
            self.new_node_unknown()
        });

        self.expect_token_one(TokenKind::Eq, "equal sign after declarator");

        let expr = self.parse_assignment_expr().unwrap_or_else(|| {
            self.add_error_with_explanation(
                ErrorKind::MissingExpr,
                tok.origin,
                String::from("expected expression after equal sign"),
            );
            self.new_node_unknown()
        });

        self.expect_token_one(
            TokenKind::SemiColon,
            "semicolon at the end of an inline definition",
        );

        Some(self.new_node(Node {
            kind: NodeKind::InlineDefinition(decl_specifiers, declarator, expr),
            origin: tok.origin,
        }))
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
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

        let node_id = self.new_node(Node {
            kind: NodeKind::TranslationUnit(decls),
            origin: Origin::default(),
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
            let token_kind = match self.peek().kind {
                TokenKind::Eof => break,
                t => t,
            };

            if self.error_mode {
                trace!("skipping to next line");
                self.skip_to_next_line();
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

            // Catch-all.
            self.add_error_with_explanation(
                ErrorKind::ParseProgram,
                self.current_or_last_origin_for_err(),
                format!(
                    "catch-all parse program error: encountered unexpected token {:?}",
                    token_kind
                ),
            );
        }
        None
    }

    #[warn(unused_results)]
    pub fn parse(&mut self) -> Option<NodeId> {
        // self.resolve_nodes();

        self.parse_program()
    }

    fn resolve_node(
        node_id: NodeId,
        nodes: &[Node],
        errors: &mut Vec<Error>,
        name_to_def: &mut NameToDef,
    ) {
        let node = &nodes[node_id];

        match &node.kind {
            NodeKind::ProbeDefinition(probe, pred, actions) => {
                Self::resolve_node(*probe, nodes, errors, name_to_def);
                if let Some(pred) = pred {
                    Self::resolve_node(*pred, nodes, errors, name_to_def);
                }
                if let Some(actions) = actions {
                    Self::resolve_node(*actions, nodes, errors, name_to_def);
                }
            }
            NodeKind::Number(_) | NodeKind::ProbeSpecifier(_) => {}
            NodeKind::Unary(_, expr) => {
                Self::resolve_node(*expr, nodes, errors, name_to_def);
            }
            NodeKind::Assignment(lhs, _, rhs) => {
                Self::resolve_node(*lhs, nodes, errors, name_to_def);
                Self::resolve_node(*rhs, nodes, errors, name_to_def);
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
                Self::resolve_node(*lhs, nodes, errors, name_to_def);
                Self::resolve_node(*rhs, nodes, errors, name_to_def);
            }
            NodeKind::ArgumentsDeclaration(args) => {
                if let Some(args) = args {
                    Self::resolve_node(*args, nodes, errors, name_to_def);
                }
            }
            NodeKind::Block(stmts) => {
                name_to_def.enter();

                for op in stmts {
                    Self::resolve_node(*op, nodes, errors, name_to_def);
                }

                name_to_def.leave();
            }
            NodeKind::If {
                cond,
                then_block,
                else_block,
            } => {
                Self::resolve_node(*cond, nodes, errors, name_to_def);
                Self::resolve_node(*then_block, nodes, errors, name_to_def);
                if let Some(else_block) = else_block {
                    Self::resolve_node(*else_block, nodes, errors, name_to_def);
                }
            }
            NodeKind::TranslationUnit(decls) => {
                for decl in decls {
                    Self::resolve_node(*decl, nodes, errors, name_to_def);
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
            NodeKind::Character(_) => {}
            NodeKind::InlineDefinition(_node_id, _node_id1, _node_id2) => {}
            NodeKind::ParameterTypeList {
                params: _,
                ellipsis: _,
            } => {}
            NodeKind::ArgumentsExpr(_node_ids) => {}
            NodeKind::ParameterDeclaration {
                param_decl_specifiers: _,
                declarator: _,
            } => {}
        }
    }

    fn resolve_nodes(&mut self) {
        if self.nodes.is_empty() {
            return;
        }

        Self::resolve_node(
            NodeId(0),
            &self.nodes,
            &mut self.lexer.errors,
            &mut self.name_to_def,
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

        self.lexer.begin(lex::LexerState::ProgramOuterScope);
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
                    String::from("expected init declarator after comma"),
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

        match self.peek().kind {
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

        match self.peek().kind {
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
            TokenKind::Identifier => {
                let origin = self.peek().origin;
                let kind = self.peek().kind;
                let name = lex::str_from_source(self.lexer.input, origin);
                if self.typenames.contains(name) {
                    Some(self.new_node(Node {
                        kind: NodeKind::TypeSpecifier(kind),
                        origin,
                    }))
                } else {
                    None
                }
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

        match self.peek().kind {
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

        match self.peek().kind {
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
            self.add_error_with_explanation(
                ErrorKind::MissingDirectDeclarator,
                self.current_or_last_origin_for_err(),
                String::from("expected directed declarator in declaration"),
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

        let mut lhs = match self.peek().kind {
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
                    self.add_error_with_explanation(
                        ErrorKind::MissingDeclarator,
                        left_paren.origin,
                        String::from("expected declarator after parenthesis"),
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
        let name = name_tok.map(|t| lex::str_from_source(self.lexer.input, t.origin).to_owned());
        if let Some(name) = &name {
            self.typenames.insert(name.clone());
        }

        let enumerator_list: Option<NodeId> =
            if let Some(left_curly) = self.match_kind(TokenKind::LeftCurly) {
                let enumerator_list = self.parse_enumerator_list().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingEnumerators,
                        left_curly.origin,
                        String::from("expected at least one enumerator in enum definition"),
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
                    String::from("expected enumerator following comma"),
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
                    String::from("expected expression following enumerator"),
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
        let tok = self
            .match_kind(TokenKind::KeywordStruct)
            .or_else(|| self.match_kind(TokenKind::KeywordUnion))?;

        let name_tok = self.match_kind(TokenKind::Identifier);
        let name = name_tok.map(|t| lex::str_from_source(self.lexer.input, t.origin).to_owned());

        if let Some(name) = &name {
            self.typenames.insert(name.clone());
        }

        let decl_list = if let Some(left_curly) = self.match_kind(TokenKind::LeftCurly) {
            let decl_list = self.parse_struct_declaration_list().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingStructDeclarationList,
                    left_curly.origin,
                    String::from("expected unary expression after opening curly brace"),
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
            let decl = self.parse_struct_declarator().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingStructFieldDeclarator,
                    comma.origin,
                    String::from(
                        "expected a struct field declarator after comma in struct field declaration"
                    ),
                );
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
            let expr = self.parse_const_expr().unwrap_or_else(|| {
                self.add_error_with_explanation(
                    ErrorKind::MissingConstantExpr,
                    colon.origin,
                    String::from(
                        "expected a constant expression after colon in struct field declaration",
                    ),
                );
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

        let mut lhs = match self.peek().kind {
            TokenKind::LeftParen
                if matches!(
                    self.peek_peek(),
                    Token {
                        kind: TokenKind::Star | TokenKind::LeftParen | TokenKind::LeftSquareBracket,
                        ..
                    }
                ) =>
            {
                let tok = self.lexer.lex();

                let abstract_decl = self.parse_abstract_declarator().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingAbstractDeclarator,
                        tok.origin,
                        String::from("expected abstract declarator after parenthesis"),
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
            TokenKind::LeftParen => {
                let func = self.parse_function().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingFunction,
                        self.current_or_last_origin_for_err(),
                        String::from("expected function after opening parenthesis"),
                    );
                    self.new_node_unknown()
                });
                Some(func)
            }
            TokenKind::LeftSquareBracket => {
                let array = self.parse_array().unwrap_or_else(|| {
                    self.add_error_with_explanation(
                        ErrorKind::MissingArray,
                        self.current_or_last_origin_for_err(),
                        String::from("expected array after opening square bracket"),
                    );
                    self.new_node_unknown()
                });
                // TODO: `DirectAbstractArray(None, array)`?
                Some(array)
            }
            _ => None,
        };

        loop {
            match self.peek().kind {
                TokenKind::LeftSquareBracket => {
                    let origin = self.peek().origin;
                    let array = self.parse_array().unwrap_or_else(|| {
                        self.add_error_with_explanation(
                            ErrorKind::MissingArray,
                            self.current_or_last_origin_for_err(),
                            String::from("expected array after opening square bracket"),
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
                        self.peek_peek(),
                        Token {
                            kind: TokenKind::Star
                                | TokenKind::LeftParen
                                | TokenKind::LeftSquareBracket,
                            ..
                        }
                    ) =>
                {
                    let origin = self.peek().origin;

                    let func = self.parse_array().unwrap_or_else(|| {
                        self.add_error_with_explanation(
                            ErrorKind::MissingFunction,
                            self.current_or_last_origin_for_err(),
                            String::from("expected function after opening parenthesis"),
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

    // function                → "(" function_parameters ")" ;
    fn parse_function(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let left_paren = self.match_kind(TokenKind::LeftParen)?;
        let args = self.parse_function_parameters();
        self.expect_token_one(TokenKind::RightParen, "matching parenthesis for function");

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
        } = self.peek()
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
            self.add_error_with_explanation(
                ErrorKind::MissingFunctionParameters,
                self.current_or_last_origin_for_err(),
                String::from("missing function parameters"),
            );
            self.new_node_unknown()
        });

        let ellipsis = if let Some(comma) = self.match_kind(TokenKind::Comma) {
            self.expect_token_one(TokenKind::DotDotDot, "ellipsis parameter after comma");
            Some(self.new_node(Node {
                kind: NodeKind::ParamEllipsis,
                origin: comma.origin,
            }))
        } else {
            None
        };

        Some(self.new_node(Node {
            kind: NodeKind::ParameterTypeList {
                params: Some(params),
                ellipsis,
            },
            origin: self.origin(params),
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
                self.add_error_with_explanation(
                    ErrorKind::MissingFunctionParameter,
                    comma.origin,
                    String::from("expected function parameter after comma"),
                );
                self.new_node_unknown()
            });
            params.push(param);
        }

        Some(self.new_node(Node {
            kind: NodeKind::Parameters(params),
            origin: self.origin(param),
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

        Some(self.new_node(Node {
            kind: NodeKind::ParameterDeclaration {
                param_decl_specifiers,
                declarator,
            },
            origin: self.origin(param_decl_specifiers),
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
            self.add_error_with_explanation(
                ErrorKind::MissingParameterDeclarationSpecifiers,
                self.current_or_last_origin_for_err(),
                String::from("expected parameter declaration specifiers"),
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

    // function_parameters     → /* empty */ | parameter_type_list ;
    fn parse_function_parameters(&mut self) -> Option<NodeId> {
        match self.peek().kind {
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
        NodeKind::Number(_) | NodeKind::Identifier(_) | NodeKind::ProbeSpecifier(_) => {}
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
        NodeKind::Aggregation(_) => {}
        NodeKind::ProbeSpecifiers(node_ids) | NodeKind::CommaExpr(node_ids) => {
            for node in node_ids {
                log(nodes, *node, indent + 2, file_id_to_name);
            }
        }
        NodeKind::SizeofType(_) => {}
        NodeKind::SizeofExpr(node_id) => log(nodes, *node_id, indent + 2, file_id_to_name),
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
        NodeKind::StructDeclaration(_, node_id) => {
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
    // use super::*;

    // #[test]
    // fn test_probe_with_predicate() {
    //     let input = "fbt::: /self->spec/ {}";
    //     let lexer = Lexer::new(1, input);
    //     let mut parser = Parser::new(lexer);
    //     let root_id = parser.parse();
    //     let root = &parser.nodes[root_id.unwrap()];
    // }

    //
    //#[test]
    //fn parse_number() {
    //    let input = "123 ";
    //    let mut lexer = Lexer::new(1);
    //    lexer.lex(&input);
    //
    //    assert!(lexer.errors.is_empty());
    //
    //    let mut parser = Parser::new(input);
    //    let root_id = parser.parse_expr().unwrap();
    //    let root = &parser.nodes[root_id];
    //
    //    assert!(parser.errors.is_empty());
    //
    //    {
    //        assert!(matches!(root.kind, NodeKind::Number(123)));
    //    }
    //}
    //
    //#[test]
    //fn parse_add() {
    //    let input = "123 + 45 + 0";
    //    let mut lexer = Lexer::new(1);
    //    lexer.lex(&input);
    //
    //    assert!(lexer.errors.is_empty());
    //
    //    let mut parser = Parser::new(input, &lexer);
    //    let root_id = parser.parse_expr().unwrap();
    //    let root = &parser.nodes[root_id];
    //
    //    assert!(parser.errors.is_empty());
    //
    //    match &root.kind {
    //        NodeKind::BinaryOp(
    //            lhs,
    //            Token {
    //                kind: TokenKind::Plus,
    //                ..
    //            },
    //            rhs,
    //        ) => {
    //            let lhs = &parser.nodes[*lhs];
    //            assert!(matches!(lhs.kind, NodeKind::Number(123)));
    //            let rhs = &parser.nodes[*rhs];
    //            match rhs.kind {
    //                NodeKind::BinaryOp(
    //                    mhs,
    //                    Token {
    //                        kind: TokenKind::Plus,
    //                        ..
    //                    },
    //                    rhs,
    //                ) => {
    //                    let mhs = &parser.nodes[mhs];
    //                    let rhs = &parser.nodes[rhs];
    //                    assert!(matches!(mhs.kind, NodeKind::Number(45)));
    //                    assert!(matches!(rhs.kind, NodeKind::Number(0)));
    //                }
    //                _ => panic!("Expected Add"),
    //            }
    //        }
    //        _ => panic!("Expected Add"),
    //    }
    //}
}
