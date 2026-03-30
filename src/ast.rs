use std::{
    collections::HashMap,
    hash::Hash,
    num::ParseIntError,
    ops::{Index, IndexMut},
};

use crate::{
    error::{Error, ErrorKind},
    lex::{Lexer, Token, TokenKind},
    origin::{FileId, Origin, OriginKind},
    type_checker::Type,
};
use log::trace;
use serde::Serialize;

// TODO: u32?
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct NodeId(pub(crate) usize);

#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
pub struct FnDef {
    pub(crate) name: String,
    pub(crate) args: Vec<NodeId>,
    pub(crate) ret: Option<NodeId>,
    pub(crate) body: NodeId,
}

#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
pub enum NodeKind {
    Unknown,
    // TODO: Should we just use 'Block'?
    File(Vec<NodeId>), // Root.
    Number(u64),
    Bool(bool),
    PrimaryToken(TokenKind),
    Cast(String, NodeId),
    ProbeSpecifier(String),
    ProbeDefinition(NodeId, Option<NodeId>, Vec<NodeId>),
    BinaryOp(NodeId, TokenKind, NodeId),
    Identifier(String),
    Aggregation(String),
    Unary(TokenKind, NodeId),
    Assignment(NodeId, Token, NodeId),
    Arguments(Vec<NodeId>),
    CommaExpr(Vec<NodeId>),
    SizeofType(String),
    SizeofExpr(NodeId),
    StringofExpr(NodeId),
    FnCall {
        // Can be a variable (function pointer), or a string.
        callee: NodeId,
        args: NodeId,
    },
    FnDef(FnDef),
    TranslationUnit(Vec<NodeId>),
    If {
        cond: NodeId,
        then_block: NodeId,
        else_block: Option<NodeId>,
    },
    Block(Vec<NodeId>),
    VarDecl(String, NodeId),
    PostfixIncDecrement(NodeId, TokenKind),
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
        }
    }

    fn new_node_unknown(&mut self) -> NodeId {
        self.new_node(Node {
            kind: NodeKind::Unknown,
            origin: self.current_origin_for_err(),
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
                let lparen = self.match_kind(TokenKind::LeftParen)?;
                let e = self.parse_expr().unwrap_or_else(|| self.new_node_unknown());
                let _ = self.expect_token_one(
                    TokenKind::RightParen,
                    "primary expression closing parenthesis",
                );
                Some(self.new_node(Node {
                    kind: NodeKind::Unary(TokenKind::LeftParen, e),
                    origin: lparen.origin,
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
                        ErrorKind::MissingExpected,
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
                kind: NodeKind::BinaryOp(lhs, op.kind, rhs),
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
                        ErrorKind::MissingExpected,
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
                kind: NodeKind::BinaryOp(lhs, op.kind, rhs),
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
            self.expect_token_one(TokenKind::RightParen, "closing cast right parenthesis");
            let node = self
                .parse_cast_expr()
                .unwrap_or_else(|| self.new_node_unknown());
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

        let first_expr = match self.parse_assignment_expr() {
            None => {
                return None;
            }
            Some(expr) => expr,
        };

        if self.peek().map(|t| t.kind) == Some(TokenKind::Comma) {
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
                        ErrorKind::MissingExpected,
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
                            ErrorKind::MissingExpected,
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
                        ErrorKind::MissingExpected,
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
    // Transformed to the equivalent (but easier to parse):
    // postfix_expression → primary_expression postfix_tail*
    //postfix_tail → "[" argument_expression_list "]"
    //             | "(" argument_expression_list? ")"
    //             | "."  ( IDENT | TNAME | keyword_as_ident )
    //             | "->" ( IDENT | TNAME | keyword_as_ident )
    //             | "++"
    //             | "--"_
    fn parse_postfix_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let mut expr = self.parse_primary_expr()?;

        loop {
            match self.peek() {
                Some(Token {
                    kind: TokenKind::PlusPlus | TokenKind::MinusMinus,
                    ..
                }) => {
                    let op = *self.eat_token().unwrap();

                    expr = self.new_node(Node {
                        kind: NodeKind::PostfixIncDecrement(expr, op.kind),
                        origin: op.origin,
                    });
                }
                // TODO
                _ => break,
            }
        }

        Some(expr)
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
    //            format!("expect block following if(cond), found: {:?}", found),
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
    //                format!("expect block following else, found: {:?}", found),
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

        // TODO: unary_expr...
        self.parse_conditional_expr()
    }

    //conditional_expression  → logical_or_expression
    //                        | logical_or_expression "?" expression
    //                          ":" conditional_expression ;
    fn parse_conditional_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        if let Some(e) = self.parse_logical_or_expr() {
            // TODO
            return Some(e);
        }

        None
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
                        ErrorKind::MissingExpected,
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
                kind: NodeKind::BinaryOp(lhs, op.kind, rhs),
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
                        ErrorKind::MissingExpected,
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
                kind: NodeKind::BinaryOp(lhs, op.kind, rhs),
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
                        ErrorKind::MissingExpected,
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
                kind: NodeKind::BinaryOp(lhs, op.kind, rhs),
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
                        ErrorKind::MissingExpected,
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
                kind: NodeKind::BinaryOp(lhs, op.kind, rhs),
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
                        ErrorKind::MissingExpected,
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
                kind: NodeKind::BinaryOp(lhs, op.kind, rhs),
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
                        ErrorKind::MissingExpected,
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
                kind: NodeKind::BinaryOp(lhs, op.kind, rhs),
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
                        ErrorKind::MissingExpected,
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
                kind: NodeKind::BinaryOp(lhs, op.kind, rhs),
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
            kind: TokenKind::Gt | TokenKind::Lt,
            ..
        }) = self.peek()
        {
            // TODO: Gte, Lte
            let op = *self.eat_token().unwrap();

            let rhs = match self.parse_shift_expr() {
                None => {
                    self.add_error_with_explanation(
                        ErrorKind::MissingExpected,
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
                kind: NodeKind::BinaryOp(lhs, op.kind, rhs),
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

        // TODO: Shift operators.

        self.parse_additive_expr()
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

        // TODO

        let e = self.parse_expr();
        self.match_kind(TokenKind::SemiColon);

        e
    }

    // Best effort to find the closest token when doing error reporting.
    fn current_or_last_token_origin(&self) -> Option<Origin> {
        assert!(self.tokens_consumed <= self.tokens.len());

        if self.tokens_consumed == self.tokens.len() {
            return self.tokens.last().map(|t| t.origin);
        }

        let token = &self.tokens[self.tokens_consumed];
        if token.kind != TokenKind::Eof {
            Some(token.origin)
        } else if self.tokens_consumed > 0 {
            Some(self.tokens[self.tokens_consumed - 1].origin)
        } else {
            None
        }
    }

    fn str_from_source(src: &'a str, origin: &Origin) -> &'a str {
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
                self.current_or_last_token_origin().unwrap(),
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

        None
    }

    //statement               → ";"
    //                        | expression ";"
    //                        | "if" "(" expression ")" statement_or_block
    //                        | "if" "(" expression ")" statement_or_block
    //                          "else" statement_or_block ;
    fn parse_statement_list(&mut self) -> Option<Vec<NodeId>> {
        // TODO: if.
        self.parse_expr().map(|x| vec![x])
    }

    fn parse_probe_specifier_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let probe_specifier = if let Some(x) = self.parse_probe_specifier() {
            x
        } else {
            let found = self.current_token_kind_for_err();
            self.errors.push(Error::new(
                ErrorKind::MissingProbeSpecifier,
                self.current_or_last_token_origin()
                    .unwrap_or(Origin::new_unknown()),
                format!("expected probe specifier, found: {:?}", found),
            ));
            return None;
        };
        // TODO: More probe specifiers separated by commas.

        // In file mode, a predication or action MUST follow.

        let predicate = if let Some(_slash) = self.match_kind(TokenKind::Slash) {
            let expr = self.parse_expr();
            self.expect_token_one(TokenKind::Slash, "matching slash after predicate");
            expr
        } else {
            None
        };

        if let Some(lcurly) = self.match_kind(TokenKind::LeftCurly) {
            let stmts = self.parse_statement_list().unwrap_or_default();

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
            self.current_origin_for_err(),
            format!(
                "a predicate or action must follow a probe specifier in file mode, found: {:?}",
                self.current_token_kind_for_err()
            ),
        );
        None
    }

    fn current_origin_for_err(&self) -> Origin {
        let tok = self.peek();
        tok.map(|t| t.origin).unwrap_or(Origin::new_unknown())
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
        // TODO: trailing stuff.

        Some(probe_specifier)
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

        // TODO: declaration.

        None
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

            // TODO: d_expr

            if let Some(prog) = self.parse_d_program() {
                return Some(prog);
            }

            // TODO: d_type

            // Catch-all.
            self.add_error_with_explanation(
                ErrorKind::ParseProgram,
                self.current_or_last_token_origin().unwrap(),
                format!(
                    "catch-all parse program error: encountered unexpected token {:?}",
                    token
                ),
            );
        }
        None
    }

    #[warn(unused_results)]
    pub fn parse(&mut self) {
        let root = self.parse_program();
        if let Some(root) = root {
            log(&self.nodes, root, 0);
        }

        self.resolve_nodes();
    }

    fn resolve_node(
        node_id: NodeId,
        nodes: &[Node],
        errors: &mut Vec<Error>,
        name_to_def: &mut NameToDef,
        file_id_to_name: &'a HashMap<FileId, String>,
    ) {
        let node = &nodes[node_id];
        if !matches!(node.kind, NodeKind::File(_)) && node.origin.kind == OriginKind::Builtin {
            return;
        }

        match &node.kind {
            NodeKind::File(decls) => {
                // Already called `.enter()` for builtins.
                assert_eq!(name_to_def.scopes.len(), 1);

                for decl in decls {
                    Self::resolve_node(*decl, nodes, errors, name_to_def, file_id_to_name);
                }

                name_to_def.leave();
            }
            NodeKind::ProbeDefinition(probe, pred, actions) => {
                Self::resolve_node(*probe, nodes, errors, name_to_def, file_id_to_name);
                if let Some(pred) = pred {
                    Self::resolve_node(*pred, nodes, errors, name_to_def, file_id_to_name);
                }
                for action in actions {
                    Self::resolve_node(*action, nodes, errors, name_to_def, file_id_to_name);
                }
            }
            NodeKind::Number(_) | NodeKind::Bool(_) | NodeKind::ProbeSpecifier(_) => {}
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
                    NodeKind::FnDef { .. } => {}
                    NodeKind::VarDecl(_, _) => {}
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
            NodeKind::VarDecl(identifier, expr) => {
                Self::resolve_node(*expr, nodes, errors, name_to_def, file_id_to_name);

                if let Some((prev, scope)) = name_to_def.get_scoped(identifier)
                    && scope == ScopeResolution::Current
                {
                    let prev_origin = nodes[*prev].origin;
                    errors.push(Error::new(
                        ErrorKind::NameAlreadyDefined,
                        node.origin,
                        format!(
                            "{} redeclared, already declared here: {}",
                            identifier,
                            prev_origin.display(file_id_to_name)
                        ),
                    ));
                }

                name_to_def.insert(identifier.to_owned(), node_id);
            }
            NodeKind::FnCall { callee, args } => {
                Self::resolve_node(*callee, nodes, errors, name_to_def, file_id_to_name);
                let callee_name = nodes[*callee].kind.as_identifier().unwrap();
                let def_id = name_to_def.get_scoped(callee_name);
                if def_id.is_none() {
                    errors.push(Error {
                        kind: ErrorKind::UnknownIdentifier,
                        origin: node.origin,
                        explanation: format!("unknown identifier: {}", callee_name),
                    });

                    // TODO: Should we pretend we found it?
                    return;
                }
                let def = &nodes[*def_id.unwrap().0];

                match def.kind {
                    NodeKind::FnDef { .. } => {} // All good.
                    _ => {
                        // Once function pointers are supported, VarDecl is also a viable option.
                        errors.push(Error {
                            kind: ErrorKind::CallingANonFunction,
                            origin: node.origin,
                            explanation: String::from("calling a non-function"),
                        });
                    }
                }

                Self::resolve_node(*args, nodes, errors, name_to_def, file_id_to_name);
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
            NodeKind::FnDef(FnDef {
                name,
                args,
                ret,
                body,
            }) => {
                if let Some((prev, _)) = name_to_def.get_scoped(name) {
                    let prev = &nodes[*prev];
                    errors.push(Error::new(
                        ErrorKind::NameAlreadyDefined,
                        node.origin,
                        format!(
                            "name {} already defined here: {}",
                            name,
                            prev.origin.display(file_id_to_name)
                        ),
                    ));
                }
                // TODO: Check shadowing of function name?
                name_to_def.insert(name.to_owned(), node_id);

                for arg in args {
                    Self::resolve_node(*arg, nodes, errors, name_to_def, file_id_to_name);
                }

                if let Some(ret) = ret {
                    Self::resolve_node(*ret, nodes, errors, name_to_def, file_id_to_name);
                }

                Self::resolve_node(*body, nodes, errors, name_to_def, file_id_to_name);
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
            NodeKind::Cast(_, _) => {
                todo!()
            }
            NodeKind::Aggregation(_) => todo!(),
            NodeKind::CommaExpr(_node_ids) => todo!(),
            NodeKind::SizeofType(_) => todo!(),
            NodeKind::SizeofExpr(_node_id) => todo!(),
            NodeKind::StringofExpr(_node_id) => todo!(),
            NodeKind::PostfixIncDecrement(_node_id, _token_kind) => todo!(),
        }
    }

    fn resolve_nodes(&mut self) {
        assert!(!self.nodes.is_empty());

        Self::resolve_node(
            NodeId(0),
            &self.nodes,
            &mut self.errors,
            &mut self.name_to_def,
            self.file_id_to_name,
        );
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
        NodeKind::Block(node_ids) | NodeKind::Arguments(node_ids) | NodeKind::File(node_ids) => {
            for id in node_ids {
                log(nodes, *id, indent + 2);
            }
        }
        NodeKind::ProbeDefinition(probe, pred, actions) => {
            log(nodes, *probe, indent + 2);
            if let Some(pred) = pred {
                log(nodes, *pred, indent + 2);
            }

            for action in actions {
                log(nodes, *action, indent + 2);
            }
        }
        NodeKind::Number(_)
        | NodeKind::Identifier(_)
        | NodeKind::Bool(_)
        | NodeKind::ProbeSpecifier(_) => {}
        NodeKind::Assignment(lhs, _, rhs) | NodeKind::BinaryOp(lhs, _, rhs) => {
            log(nodes, *lhs, indent + 2);
            log(nodes, *rhs, indent + 2);
        }
        NodeKind::VarDecl(_, node_id) | NodeKind::Unary(_, node_id) => {
            log(nodes, *node_id, indent + 2);
        }
        NodeKind::FnCall { callee, args } => {
            log(nodes, *callee, indent + 2);
            log(nodes, *args, indent + 2);
        }
        NodeKind::FnDef(FnDef {
            name: _,
            args,
            ret,
            body,
        }) => {
            for id in args {
                log(nodes, *id, indent + 2);
            }

            if let Some(ret) = ret {
                log(nodes, *ret, indent + 2);
            }
            log(nodes, *body, indent + 2);
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
        NodeKind::Aggregation(_) => todo!(),
        NodeKind::CommaExpr(node_ids) => {
            for node in node_ids {
                log(nodes, *node, indent + 2);
            }
        }
        NodeKind::SizeofType(_) => {}
        NodeKind::SizeofExpr(node_id) => log(nodes, *node_id, indent + 2),
        NodeKind::StringofExpr(node_id) => log(nodes, *node_id, indent + 2),
        NodeKind::PostfixIncDecrement(node_id, _token_kind) => log(nodes, *node_id, indent + 2),
    }
}

impl NodeKind {
    fn as_file_mut(&mut self) -> Option<&mut Vec<NodeId>> {
        match self {
            NodeKind::File(v) => Some(v),
            _ => None,
        }
    }

    pub(crate) fn as_identifier(&self) -> Option<&str> {
        match self {
            NodeKind::Identifier(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_arguments(&self) -> Option<&[NodeId]> {
        match self {
            NodeKind::Arguments(args) => Some(args),
            _ => None,
        }
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
            NodeKind::BinaryOp(lhs, TokenKind::Plus, rhs) => {
                let lhs = &parser.nodes[*lhs];
                assert!(matches!(lhs.kind, NodeKind::Number(123)));
                let rhs = &parser.nodes[*rhs];
                match rhs.kind {
                    NodeKind::BinaryOp(mhs, TokenKind::Plus, rhs) => {
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
