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
    // TODO: Should we just use 'Block'?
    File(Vec<NodeId>), // Root.
    Number(u64),
    Bool(bool),
    ProbeSpecifier(String),
    Add(NodeId, NodeId),
    Multiply(NodeId, NodeId),
    Divide(NodeId, NodeId),
    Cmp(NodeId, NodeId),
    Identifier(String),
    Unary(TokenKind, NodeId),
    Assignment(NodeId, Token, NodeId),
    Arguments(Vec<NodeId>),
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
    VarDecl(String, NodeId), // TODO: Vec in case of identifier list.
    Break,                   // TODO: label argument.
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

    fn new_node(&mut self, node: Node) -> NodeId {
        self.nodes.push(node);
        NodeId(self.nodes.len() - 1)
    }

    pub(crate) fn builtins(&mut self) {
        assert!(self.nodes.is_empty());

        let origin = Origin::new_builtin();

        let root = self.new_node(Node {
            kind: NodeKind::File(Vec::new()),
            origin,
        });
        self.name_to_def.enter();

        let any = self.new_node(Node {
            kind: NodeKind::Identifier(String::from("any")),
            origin,
        });
        self.name_to_def.insert(String::from("any"), any);
        self.node_to_type.insert(any, Type::new_any());

        let body = self.new_node(Node {
            kind: NodeKind::Block(Vec::new()),
            origin,
        });

        let println = self.new_node(Node {
            kind: NodeKind::FnDef(FnDef {
                name: String::from("println"),
                args: vec![any],
                ret: None,
                body,
            }),
            origin,
        });
        let println_type = Type::new_function(
            &Type::new_void(),
            &[Type::new_any()],
            &Origin::new_builtin(),
        );
        self.nodes[root].kind.as_file_mut().unwrap().push(println);
        self.name_to_def.insert(String::from("println"), println);
        self.node_to_type.insert(println, println_type);
    }

    fn peek_token(&self) -> Option<&Token> {
        assert!(self.tokens_consumed <= self.tokens.len());
        if self.tokens_consumed == self.tokens.len() {
            None
        } else {
            Some(&self.tokens[self.tokens_consumed])
        }
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
        let current_line = self.peek_token().map(|t| t.origin.line).unwrap_or(1);

        loop {
            match self.peek_token() {
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
        match self.peek_token() {
            Some(t) if t.kind == kind => {
                let res = Some(*t);
                self.tokens_consumed += 1;
                res
            }
            _ => None,
        }
    }

    // Operand     = Literal | OperandName [ TypeArgs ] | "(" Expression ")" .
    // Literal     = BasicLit | CompositeLit | FunctionLit .
    // BasicLit    = int_lit | float_lit | imaginary_lit | rune_lit | string_lit .
    // OperandName = identifier | QualifiedIdent .
    fn parse_operand(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let tok = self.peek_token();
        let origin = tok.map(|t| t.origin).unwrap_or(Origin::new_unknown());
        match tok.map(|t| t.kind) {
            Some(TokenKind::LeftParen) => {
                self.eat_token().unwrap();
                let e = self.parse_expr().or_else(|| {
                    let found = self.peek_token().map(|t| t.kind).unwrap_or(TokenKind::Eof);
                    self.errors.push(Error::new(
                        ErrorKind::MissingExpr,
                        origin,
                        format!("expected expression after '(' but found: {:?}", found),
                    ));
                    None
                })?;
                self.expect_token_one(TokenKind::RightParen, "parenthesized operand");
                Some(e)
            }
            Some(TokenKind::Identifier) => {
                self.eat_token().unwrap();
                Some(self.new_node(Node {
                    kind: NodeKind::Identifier(
                        Self::str_from_source(self.input, &origin).to_owned(),
                    ),
                    origin,
                }))
            }
            Some(TokenKind::LiteralNumber) => {
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
            Some(TokenKind::LiteralBool) => {
                self.eat_token().unwrap();
                let src = Self::str_from_source(self.input, &origin);

                assert!(src == "true" || src == "false");

                let node_id = self.new_node(Node {
                    kind: NodeKind::Bool(src == "true"),
                    origin,
                });
                self.node_to_type.insert(node_id, Type::new_bool());
                Some(node_id)
            }
            _ => None,
        }
    }

    // Arguments     = "(" [ ( ExpressionList | Type [ "," ExpressionList ] ) [ "..." ] [ "," ] ] ")" .
    fn parse_arguments(&mut self) -> Option<NodeId> {
        // TODO: ExpressionList.

        let lparen = self.match_kind(TokenKind::LeftParen)?;
        // TODO: Multiple arguments
        let e = self.parse_expr();
        let args = if let Some(e) = e { vec![e] } else { Vec::new() };
        let _rparen = self.expect_token_one(TokenKind::RightParen, "arguments");
        Some(self.new_node(Node {
            kind: NodeKind::Arguments(args),
            origin: lparen.origin,
        }))
    }

    // PrimaryExpr   = Operand |
    //                 Conversion |
    //                 MethodExpr |
    //                 PrimaryExpr Selector |
    //                 PrimaryExpr Index |
    //                 PrimaryExpr Slice |
    //                 PrimaryExpr TypeAssertion |
    //                 PrimaryExpr Arguments .
    // Selector      = "." identifier .
    // Index         = "[" Expression [ "," ] "]" .
    // Slice         = "[" [ Expression ] ":" [ Expression ] "]" |
    //                 "[" [ Expression ] ":" Expression ":" Expression "]" .
    // TypeAssertion = "." "(" Type ")" .
    // Arguments     = "(" [ ( ExpressionList | Type [ "," ExpressionList ] ) [ "..." ] [ "," ] ] ")" .
    fn parse_primary_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        if let Some(args) = self.parse_arguments() {
            return Some(args);
        }

        if let Some(op) = self.parse_operand() {
            return Some(op);
        }

        // TODO: Conversion.
        // TODO: MethodExpr.
        // TODO: PrimaryExpr Selector.
        // TODO: PrimaryExpr Index.
        // TODO: PrimaryExpr Slice.
        // TODO: PrimaryExpr TypeAssertion.
        // TODO: PrimaryExpr Arguments.

        None
    }

    // TODO
    fn parse_bin_expr_logic_or(&mut self) -> Option<NodeId> {
        self.parse_bin_expr_logic_and()
    }

    fn parse_bin_expr_logic_and(&mut self) -> Option<NodeId> {
        self.parse_bin_expr_equality()
    }

    fn parse_bin_expr_equality(&mut self) -> Option<NodeId> {
        let lhs = self.parse_bin_expr_cmp()?;

        match self.peek_token().map(|t| t.kind) {
            Some(TokenKind::EqEq) => {
                let op = *self.eat_token().unwrap();
                let rhs = self.parse_bin_expr_equality().or_else(|| {
                    let found = self.peek_token().map(|t| t.kind).unwrap_or(TokenKind::Eof);
                    self.errors.push(Error::new(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected expression after {:?} but found: {:?}",
                            op.kind, found
                        ),
                    ));
                    None
                })?;

                Some(self.new_node(Node {
                    kind: NodeKind::Cmp(lhs, rhs),
                    origin: op.origin,
                }))
            }
            _ => Some(lhs),
        }
    }

    fn parse_bin_expr_cmp(&mut self) -> Option<NodeId> {
        self.parse_bin_expr_add()
    }

    fn parse_bin_expr_add(&mut self) -> Option<NodeId> {
        let lhs = self.parse_bin_expr_mul()?;

        match self.peek_token().map(|t| t.kind) {
            Some(TokenKind::Plus) => {
                let op = *self.eat_token().unwrap();
                let rhs = self.parse_bin_expr_add().or_else(|| {
                    let found = self.peek_token().map(|t| t.kind).unwrap_or(TokenKind::Eof);
                    self.errors.push(Error::new(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected expression after {:?} but found: {:?}",
                            op.kind, found
                        ),
                    ));
                    None
                })?;

                Some(self.new_node(Node {
                    kind: NodeKind::Add(lhs, rhs),
                    origin: op.origin,
                }))
            }
            _ => Some(lhs),
        }
    }

    fn parse_bin_expr_mul(&mut self) -> Option<NodeId> {
        let lhs = self.parse_unary_expr()?;

        match self.peek_token().map(|t| t.kind) {
            Some(TokenKind::Star | TokenKind::Slash) => {
                let op = *self.eat_token().unwrap();
                let rhs = self.parse_bin_expr_mul().or_else(|| {
                    let found = self.peek_token().map(|t| t.kind).unwrap_or(TokenKind::Eof);
                    self.errors.push(Error::new(
                        ErrorKind::MissingExpr,
                        op.origin,
                        format!(
                            "expected expression after {:?} but found: {:?}",
                            op.kind, found
                        ),
                    ));
                    None
                })?;

                Some(self.new_node(Node {
                    kind: if op.kind == TokenKind::Star {
                        NodeKind::Multiply(lhs, rhs)
                    } else {
                        NodeKind::Divide(lhs, rhs)
                    },
                    origin: op.origin,
                }))
            }
            _ => Some(lhs),
        }
    }

    // Expression = UnaryExpr | Expression binary_op Expression .
    // binary_op  = "||" | "&&" | rel_op | add_op | mul_op .
    // rel_op     = "==" | "!=" | "<" | "<=" | ">" | ">=" .
    // add_op     = "+" | "-" | "|" | "^" .
    // mul_op     = "*" | "/" | "%" | "<<" | ">>" | "&" | "&^" .
    //
    // Precedence    Operator
    //    5             *  /  %  <<  >>  &  &^
    //    4             +  -  |  ^
    //    3             ==  !=  <  <=  >  >=
    //    2             &&
    //    1             ||
    fn parse_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        self.parse_bin_expr_logic_or()
    }

    fn parse_call_expr(&mut self) -> Option<NodeId> {
        let prim = self.parse_primary_expr()?;

        let tok = self.peek_token();
        if let Some(Token {
            kind: TokenKind::LeftParen,
            ..
        }) = tok
        {
            let origin = tok.unwrap().origin;
            let args = self.parse_arguments()?;
            Some(self.new_node(Node {
                kind: NodeKind::FnCall { callee: prim, args },
                origin,
            }))
        } else {
            Some(prim)
        }
    }

    // UnaryExpr  = PrimaryExpr | unary_op UnaryExpr .
    // unary_op   = "+" | "-" | "!" | "^" | "*" | "&" | "<-" .
    fn parse_unary_expr(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        match self.peek_token().map(|t| t.kind) {
            // TODO: More
            Some(TokenKind::Plus | TokenKind::Star) => {
                let token = *self.eat_token().unwrap();
                let expr = self.parse_unary_expr().or_else(|| {
                    let found = self.peek_token().map(|t| t.kind).unwrap_or(TokenKind::Eof);
                    self.errors.push(Error::new(
                        ErrorKind::MissingExpr,
                        token.origin,
                        format!("expected expression in unary expression after operator but found: {:?}", found)));
                    None
                })?;
                Some(self.new_node(Node {
                    kind: NodeKind::Unary(token.kind, expr),
                    origin: token.origin,
                }))
            }
            _ => self.parse_call_expr(),
        }
    }

    // Block         = "{" StatementList "}" .
    // StatementList = { Statement ";" } .
    fn parse_block(&mut self) -> Option<NodeId> {
        let left_curly = self.match_kind(TokenKind::LeftCurly)?;

        let mut stmts = Vec::new();

        for _ in 0..self.remaining_tokens_count() {
            match self.peek_token().map(|t| t.kind) {
                None | Some(TokenKind::Eof) | Some(TokenKind::RightCurly) => break,
                _ => {}
            }

            let stmt = self.parse_statement()?;
            stmts.push(stmt);
        }
        self.expect_token_one(TokenKind::RightCurly, "block")?;

        Some(self.new_node(Node {
            kind: NodeKind::Block(stmts),
            origin: left_curly.origin,
        }))
    }

    // IfStmt = "if" [ SimpleStmt ";" ] Expression Block [ "else" ( IfStmt | Block ) ] .
    fn parse_statement_if(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let keyword_if = self.match_kind(TokenKind::KeywordIf)?;
        let cond = self.parse_expr()?;

        let then_block = if let Some(b) = self.parse_block() {
            b
        } else {
            let found = self.peek_token().map(|t| t.kind).unwrap_or(TokenKind::Eof);
            self.add_error_with_explanation(
                ErrorKind::MissingExpected(TokenKind::LeftCurly),
                keyword_if.origin,
                format!("expect block following if(cond), found: {:?}", found),
            );

            return None;
        };

        let else_block = if self.match_kind(TokenKind::KeywordElse).is_some() {
            let block = self.parse_block().or_else(|| {
                let found = self.peek_token().map(|t| t.kind).unwrap_or(TokenKind::Eof);
                self.add_error_with_explanation(
                    ErrorKind::MissingExpected(TokenKind::LeftCurly),
                    keyword_if.origin,
                    format!("expect block following else, found: {:?}", found),
                );

                None
            })?;

            Some(block)
        } else {
            None
        };

        Some(self.new_node(Node {
            kind: NodeKind::If {
                cond,
                then_block,
                else_block,
            },
            origin: keyword_if.origin,
        }))
    }

    // VarDecl = "var" ( VarSpec | "(" { VarSpec ";" } ")" ) .
    // VarSpec = IdentifierList ( Type [ "=" ExpressionList ] | "=" ExpressionList ) .
    fn parse_statement_var_decl(&mut self) -> Option<NodeId> {
        let identifier = self.expect_token_one(TokenKind::Identifier, "var declaration")?;
        let eq = self.expect_token_one(TokenKind::Eq, "var declaration")?;
        let expr = if let Some(expr) = self.parse_expr() {
            expr
        } else {
            let found = self.peek_token().map(|t| t.kind).unwrap_or(TokenKind::Eof);
            self.add_error_with_explanation(
                ErrorKind::MissingExpr,
                eq.origin,
                format!(
                    "expected expression in variable declaration following '=' but found: {:?}",
                    found
                ),
            );
            return None;
        };

        let identifier_str = Self::str_from_source(self.input, &identifier.origin);

        Some(self.new_node(Node {
            kind: NodeKind::VarDecl(identifier_str.to_owned(), expr),
            origin: identifier.origin,
        }))
    }

    // Assignment = ExpressionList assign_op ExpressionList .
    fn parse_assignment(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }
        // TODO: Expression list.

        let lhs = self.parse_expr()?;
        // TODO: More operators.

        if self.peek_token().map(|t| t.kind).unwrap_or(TokenKind::Eof) != TokenKind::Eq {
            return Some(lhs);
        }

        let eq = self.expect_token_one(TokenKind::Eq, "assignment")?;
        let rhs = self.parse_expr().or_else(|| {
            let found = self.peek_token().map(|t| t.kind).unwrap_or(TokenKind::Eof);
            self.add_error_with_explanation(
                ErrorKind::MissingExpr,
                eq.origin,
                format!("expected expression in assignment, found: {:?}", found),
            );
            None
        })?;

        Some(self.new_node(Node {
            kind: NodeKind::Assignment(lhs, eq, rhs),
            origin: eq.origin,
        }))
    }

    // SimpleStmt = EmptyStmt | ExpressionStmt | SendStmt | IncDecStmt | Assignment | ShortVarDecl .
    fn parse_simple_statement(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        if let Some(stmt) = self.parse_assignment() {
            return Some(stmt);
        };

        // TODO: EmptyStmt ???
        if let Some(expr_stmt) = self.parse_expr() {
            return Some(expr_stmt);
        }

        // TODO: SendStmt.
        // TODO: IncDecStmt.

        // TODO: ShortVarDecl.

        None
    }

    // Statement  = Declaration | LabeledStmt | SimpleStmt |
    //              GoStmt | ReturnStmt | BreakStmt | ContinueStmt | GotoStmt |
    //              FallthroughStmt | Block | IfStmt | SwitchStmt | SelectStmt | ForStmt |
    //              DeferStmt .
    fn parse_statement(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        if let Some(stmt) = self.parse_statement_if() {
            return Some(stmt);
        };

        if let Some(stmt) = self.parse_block() {
            return Some(stmt);
        };

        if let Some(stmt) = self.parse_simple_statement() {
            return Some(stmt);
        }

        if let Some(stmt) = self.parse_external_declaration() {
            return Some(stmt);
        }

        // TODO: Labeled stmt.

        // TODO: Go stmt.
        // TODO: Return stmt.
        // TODO: Break stmt.
        // TODO: Continue stmt.
        // TODO: Goto stmt.
        // TODO: Fallthrough stmt.

        // TODO: Switch stmt.
        // TODO: Select stmt.

        None
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
                ErrorKind::MissingExpected(token_kind),
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

        let tok = self.peek_token();
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

        if matches!(
            self.peek_token().map(|t| t.kind),
            Some(TokenKind::LiteralNumber)
        ) {
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

    fn parse_probe_specifier_list(&mut self) -> Option<NodeId> {
        if self.error_mode {
            return None;
        }

        let probe_specifier = if let Some(x) = self.parse_probe_specifier() {
            x
        } else {
            let found = self.peek_token().map(|t| t.kind).unwrap_or(TokenKind::Eof);
            self.errors.push(Error::new(
                ErrorKind::MissingProbeSpecifier,
                self.current_or_last_token_origin()
                    .unwrap_or(Origin::new_unknown()),
                format!("expected probe specifier, found: {:?}", found),
            ));
            return None;
        };

        // TODO: More probe specifiers separated by commas.
        Some(probe_specifier)
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
        match self.peek_token() {
            Some(Token {
                kind: TokenKind::Eof,
                ..
            })
            | None => true,
            _ => false,
        }
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
            let token = match self.peek_token().map(|t| t.kind) {
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
            // Nothing to do.
            NodeKind::Break
            | NodeKind::Number(_)
            | NodeKind::Bool(_)
            | NodeKind::ProbeSpecifier(_) => {}

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

            // Recurse.
            NodeKind::Add(lhs, rhs)
            | NodeKind::Multiply(lhs, rhs)
            | NodeKind::Divide(lhs, rhs)
            | NodeKind::Cmp(lhs, rhs) => {
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
        NodeKind::Block(node_ids) | NodeKind::Arguments(node_ids) | NodeKind::File(node_ids) => {
            for id in node_ids {
                log(nodes, *id, indent + 2);
            }
        }

        NodeKind::Break
        | NodeKind::Number(_)
        | NodeKind::Identifier(_)
        | NodeKind::Bool(_)
        | NodeKind::ProbeSpecifier(_) => {}

        NodeKind::Assignment(lhs, _, rhs)
        | NodeKind::Divide(lhs, rhs)
        | NodeKind::Cmp(lhs, rhs)
        | NodeKind::Multiply(lhs, rhs)
        | NodeKind::Add(lhs, rhs) => {
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
            NodeKind::Add(lhs, rhs) => {
                let lhs = &parser.nodes[*lhs];
                assert!(matches!(lhs.kind, NodeKind::Number(123)));
                let rhs = &parser.nodes[*rhs];
                match rhs.kind {
                    NodeKind::Add(mhs, rhs) => {
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
