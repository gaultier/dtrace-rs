use std::io::Write;

use crate::{
    ast::{Node, NodeId, NodeKind},
    lex::{self, TokenKind},
};

struct Formatter<'a, W> {
    w: &'a mut W,
    nodes: &'a [Node],
    input: &'a str,
}

impl<'a, W: Write> Formatter<'a, W> {
    fn indent(&mut self, n: usize) -> std::io::Result<()> {
        write!(self.w, "{:width$}", "", width = n)
    }

    /// Formats an `if`/`else` branch, always emitting surrounding braces. If `node_id`
    /// is already a `Block`, its children are inlined directly to avoid double braces.
    fn fmt_branch(&mut self, node_id: NodeId, indent: usize) -> std::io::Result<()> {
        let children = match self.nodes[node_id].kind.clone() {
            NodeKind::Block(children) => children,
            _ => {
                self.w.write_all(b"{\n")?;
                self.indent(indent + 2)?;
                self.fmt(node_id, indent + 2)?;
                self.w.write_all(b"\n")?;
                self.indent(indent)?;
                self.w.write_all(b"}")?;
                return Ok(());
            }
        };
        self.w.write_all(b"{\n")?;
        for child_id in children {
            self.indent(indent + 2)?;
            self.fmt(child_id, indent + 2)?;
            self.w.write_all(b"\n")?;
        }
        self.indent(indent)?;
        self.w.write_all(b"}")?;
        Ok(())
    }

    /// Formats a single node. Does not emit leading indent or trailing newline;
    /// the caller is responsible for surrounding whitespace.
    fn fmt(&mut self, node_id: NodeId, indent: usize) -> std::io::Result<()> {
        // Clone to avoid holding a shared borrow of `self.nodes` across recursive calls.
        let kind = self.nodes[node_id].kind.clone();
        let origin = self.nodes[node_id].origin;

        match kind {
            NodeKind::Unknown | NodeKind::Character(_) | NodeKind::ParamEllipsis => {
                let src = lex::str_from_source(self.input, origin);
                self.w.write_all(src.as_bytes())?;
            }
            NodeKind::Block(node_ids) => {
                self.w.write_all(b"{\n")?;
                for id in node_ids {
                    self.indent(indent + 2)?;
                    self.fmt(id, indent + 2)?;
                    self.w.write_all(b"\n")?;
                }
                self.indent(indent)?;
                self.w.write_all(b"}")?;
            }
            NodeKind::ProbeDefinition(probe, pred, actions) => {
                self.fmt(probe, indent)?;
                self.w.write_all(b"\n")?;

                if let Some(pred_id) = pred {
                    self.w.write_all(b"/ ")?;
                    self.fmt(pred_id, indent)?;
                    self.w.write_all(b" /\n")?;
                }

                if let Some(actions_id) = actions {
                    self.fmt(actions_id, indent)?;
                }
                self.w.write_all(b"\n")?;
            }
            NodeKind::Number(..)
            | NodeKind::Identifier(_)
            | NodeKind::ProbeSpecifier(_)
            | NodeKind::PrimaryToken(_)
            | NodeKind::Aggregation => {
                let src = lex::str_from_source(self.input, origin);
                self.w.write_all(src.as_bytes())?;
            }
            NodeKind::Assignment(lhs, tok, rhs) | NodeKind::BinaryOp(lhs, tok, rhs) => {
                self.fmt(lhs, indent)?;
                let src = lex::str_from_source(self.input, tok.origin);
                write!(self.w, " {} ", src)?;
                self.fmt(rhs, indent)?;
            }
            NodeKind::If {
                cond,
                then_block,
                else_block,
            } => {
                self.w.write_all(b"if (")?;
                self.fmt(cond, indent)?;
                self.w.write_all(b") ")?;
                self.fmt_branch(then_block, indent)?;

                if let Some(else_id) = else_block {
                    self.w.write_all(b" else ")?;
                    // `else if` chains are not wrapped in an extra set of braces.
                    if matches!(self.nodes[else_id].kind, NodeKind::If { .. }) {
                        self.fmt(else_id, indent)?;
                    } else {
                        self.fmt_branch(else_id, indent)?;
                    }
                }
            }
            NodeKind::TranslationUnit(decls) => {
                for decl in decls {
                    self.fmt(decl, indent)?;
                }
            }
            NodeKind::Cast(type_name, inner) => {
                write!(self.w, "({})", &type_name)?;
                self.fmt(inner, indent)?;
            }
            NodeKind::ExprStmt(inner) => {
                self.fmt(inner, indent)?;
                self.w.write_all(b";")?;
            }
            NodeKind::EmptyStmt => {}
            NodeKind::PostfixArguments(primary, args) => {
                self.fmt(primary, indent)?;
                self.w.write_all(b"(")?;
                if let Some(args_id) = args {
                    self.fmt(args_id, indent)?;
                }
                self.w.write_all(b")")?;
            }
            NodeKind::ProbeSpecifiers(node_ids) => {
                for (i, node_id) in node_ids.iter().enumerate() {
                    self.fmt(*node_id, indent)?;
                    if i != node_ids.len() - 1 {
                        self.w.write_all(b",\n")?;
                    }
                }
            }
            NodeKind::CommaExpr(node_ids) => {
                for (i, node_id) in node_ids.iter().enumerate() {
                    self.fmt(*node_id, indent)?;
                    if i != node_ids.len() - 1 {
                        self.w.write_all(b", ")?;
                    }
                }
            }
            NodeKind::Sizeof(node_id, with_paren) => {
                self.w.write_all(b"sizeof")?;
                if with_paren {
                    self.w.write_all(b"(")?;
                } else {
                    self.w.write_all(b" ")?;
                }
                self.fmt(node_id, indent)?;
                if with_paren {
                    self.w.write_all(b")")?;
                }
            }
            NodeKind::StringofExpr(node_id, with_paren) => {
                self.w.write_all(b"stringof")?;
                if !with_paren {
                    self.w.write_all(b" ")?;
                }
                self.fmt(node_id, indent)?;
            }
            NodeKind::PostfixIncDecrement(node_id, token) => {
                self.fmt(node_id, indent)?;
                let s = lex::str_from_source(self.input, token.origin);
                self.w.write_all(s.as_bytes())?;
            }
            NodeKind::PostfixArrayAccess(_primary, _args) => {
                todo!()
            }
            NodeKind::TernaryExpr(lhs, mhs, rhs) => {
                self.fmt(lhs, indent)?;
                self.w.write_all(b" ? ")?;
                self.fmt(mhs, indent)?;
                self.w.write_all(b" : ")?;
                self.fmt(rhs, indent)?;
            }
            NodeKind::FieldAccess(_node_id, _, _) => {
                todo!()
            }
            NodeKind::TypeName(specifier, declarator) => {
                self.fmt(specifier, indent)?;
                if let Some(declarator) = declarator {
                    self.w.write_all(b" ")?;
                    self.fmt(declarator, indent)?;
                };
            }
            NodeKind::OffsetOf(node_id, token) => {
                self.w.write_all(b"offsetof(")?;
                self.fmt(node_id, indent)?;
                self.w.write_all(b", ")?;
                let s = lex::str_from_source(self.input, token.origin);
                self.w.write_all(s.as_bytes())?;
                self.w.write_all(b")")?;
            }
            NodeKind::Declaration(_decl_specifiers, _init_declarator_list) => {
                todo!()
            }
            NodeKind::DeclarationSpecifiers(_node_ids) => {
                todo!()
            }
            NodeKind::DirectDeclarator(_base, _suffix) => {
                todo!()
            }
            NodeKind::Declarator(_ptr, _declarator) => {
                todo!()
            }
            NodeKind::InitDeclarators(_node_ids) => {
                todo!()
            }
            NodeKind::TypeQualifier(_)
            | NodeKind::DStorageClassSpecifier(_)
            | NodeKind::StorageClassSpecifier(_)
            | NodeKind::TypeSpecifier(_) => {
                let s = lex::str_from_source(self.input, origin);
                self.w.write_all(s.as_bytes())?;
            }
            NodeKind::EnumDeclaration(_token, _node_id) => {
                todo!()
            }
            NodeKind::EnumeratorDeclaration(_token, _node_id) => {
                todo!()
            }
            NodeKind::EnumeratorsDeclaration(_node_ids) => {
                todo!()
            }
            NodeKind::UnionDeclaration(_, _node_id) => {
                todo!()
            }
            NodeKind::StructDeclaration(_, _node_id) => {
                todo!()
            }
            NodeKind::StructFieldsDeclaration(_node_ids) => {
                todo!()
            }
            NodeKind::StructFieldDeclarator(_declarator, _const_expr) => {
                todo!()
            }
            NodeKind::StructFieldDeclaration(_specifier_qualifier_list, _declarator_list) => {
                todo!()
            }
            NodeKind::StructFieldDeclaratorList(_node_ids) => {
                todo!()
            }
            NodeKind::SpecifierQualifierList(node_ids) => {
                for (i, node_id) in node_ids.iter().enumerate() {
                    self.fmt(*node_id, indent)?;
                    if i != node_ids.len() - 1 {
                        self.w.write_all(b" ")?;
                    }
                }
            }
            NodeKind::Xlate(_type_name, _expr) => {
                todo!()
            }
            NodeKind::DirectAbstractDeclarator(_node_id) => {
                todo!()
            }
            NodeKind::DirectAbstractArray(_base, _suffix) => {
                todo!()
            }
            NodeKind::DirectAbstractFunction(_base, _suffix) => {
                todo!()
            }
            NodeKind::AbstractDeclarator(_ptr, _abstract_decl) => {
                todo!()
            }
            NodeKind::Pointer(_type_qualifiers, _ptr) => {
                todo!()
            }
            NodeKind::Array(_params) => {
                todo!()
            }
            NodeKind::Parameters(_node_ids) => {
                todo!()
            }
            NodeKind::ParameterDeclarationSpecifiers(_node_ids) => {
                todo!()
            }
            NodeKind::Unary(token, node_id) => {
                let s = lex::str_from_source(self.input, token.origin);
                self.w.write_all(s.as_bytes())?;
                self.fmt(node_id, indent)?;

                if token.kind == TokenKind::LeftParen {
                    // Need to close the parenthesis manually - all other operators are prefix
                    // operators, so no need there.
                    self.w.write_all(b")")?;
                }
            }
            NodeKind::ArgumentsDeclaration(_node_ids) => todo!(),
            NodeKind::InlineDefinition(_node_id, _node_id1, _node_id2) => todo!(),
            NodeKind::ArgumentsExpr(_node_ids) => todo!(),
            NodeKind::ParameterTypeList {
                params: _,
                ellipsis: _,
            } => todo!(),
            NodeKind::ParameterDeclaration {
                param_decl_specifiers: _,
                declarator: _,
            } => todo!(),
        }
        Ok(())
    }
}

pub fn format<W: Write>(
    w: &mut W,
    node_id: NodeId,
    nodes: &[Node],
    input: &str,
) -> std::io::Result<()> {
    Formatter { w, nodes, input }.fmt(node_id, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast::{NodeId, Parser},
        lex::Lexer,
    };

    const FILE_ID: u32 = 1;

    fn parse_program(input: &'static str) -> (Parser<'static>, NodeId) {
        let lexer = Lexer::new(FILE_ID, input);
        let mut parser = Parser::new(lexer);
        let root_id = parser.parse().unwrap();
        (parser, root_id)
    }

    fn fmt(input: &'static str) -> String {
        let (parser, root_id) = parse_program(input);
        let mut out = Vec::new();
        format(&mut out, root_id, &parser.nodes, input).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn test_probe_no_pred_no_body() {
        let input = "syscall::open:entry {}";
        assert_eq!(
            fmt(input),
            "syscall::open:entry
{
}
"
        );
    }

    #[test]
    fn test_probe_with_predicate() {
        let input = "syscall::open:entry / pid == 42 / {}";
        assert_eq!(
            fmt(input),
            "syscall::open:entry
/ pid == 42 /
{
}
"
        );
    }

    #[test]
    fn test_probe_with_body() {
        let input = "syscall::open:entry { x = 1; }";
        assert_eq!(
            fmt(input),
            "syscall::open:entry
{
  x = 1;
}
"
        );
    }

    #[test]
    fn test_multiple_statements_in_body() {
        let input = "syscall::open:entry { x = 1; y = 2; }";
        assert_eq!(
            fmt(input),
            "syscall::open:entry
{
  x = 1;
  y = 2;
}
"
        );
    }

    #[test]
    fn test_if_with_block_body() {
        let input = "syscall::open:entry { if (x == 1) { y = 2; } }";
        assert_eq!(
            fmt(input),
            "syscall::open:entry
{
  if (x == 1) {
    y = 2;
  }
}
"
        );
    }

    #[test]
    fn test_multiple_probe_specifiers() {
        let input = "BEGIN, END {}";
        assert_eq!(
            fmt(input),
            "BEGIN,
END
{
}
"
        );
    }

    #[test]
    fn test_comma_expr() {
        let input = "BEGIN { a = 1, 2; }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  a = 1, 2;
}
"
        );
    }

    #[test]
    fn test_function_call_no_args() {
        let input = "BEGIN { print(); }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  print();
}
"
        );
    }

    #[test]
    fn test_function_call_single_arg() {
        let input = "BEGIN { print(a); }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  print(a);
}
"
        );
    }

    #[test]
    fn test_multiple_probe_specifiers_with_body() {
        let input = "BEGIN, END { a = 1, 2; print(a); }";
        assert_eq!(
            fmt(input),
            "BEGIN,
END
{
  a = 1, 2;
  print(a);
}
"
        );
    }

    #[test]
    fn test_sizeof_simple_type() {
        let input = "BEGIN { x = sizeof(int); }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = sizeof(int);
}
"
        );
    }

    #[test]
    fn test_sizeof_qualified_type() {
        // `const` is a type qualifier; the formatter must join qualifier and specifier with a space.
        let input = "BEGIN { x = sizeof(const int); }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = sizeof(const int);
}
"
        );
    }

    #[test]
    fn test_sizeof_expr() {
        // `sizeof x` (no parens) takes the unary-expression path in the parser, producing
        // `Sizeof(Identifier)` rather than `Sizeof(TypeName(...))`. The formatter normalizes
        // both forms to `sizeof(...)`.
        let input = "BEGIN { x = sizeof y; }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = sizeof y;
}
"
        );
    }

    #[test]
    fn test_multiple_probes() {
        let input = "syscall::open:entry { x = 1; } syscall::close:entry { x = 2; }";
        assert_eq!(
            fmt(input),
            "syscall::open:entry
{
  x = 1;
}
syscall::close:entry
{
  x = 2;
}
"
        );
    }

    #[test]
    fn test_postfix_increment() {
        let input = "BEGIN { x++; }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x++;
}
"
        );
    }

    #[test]
    fn test_postfix_decrement() {
        let input = "BEGIN { x--; }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x--;
}
"
        );
    }

    #[test]
    fn test_ternary_expr() {
        let input = "BEGIN { x = a ? b : c; }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = a ? b : c;
}
"
        );
    }

    #[test]
    fn test_stringof_no_paren_expr() {
        let input = "BEGIN { x = stringof y; }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = stringof y;
}
"
        );
    }

    #[test]
    fn test_stringof_paren_expr() {
        let input = "BEGIN { x = stringof  ( y  ) ; }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = stringof(y);
}
"
        );
    }

    #[test]
    fn test_offsetof() {
        // Using a plain type specifier avoids struct-declaration formatting, which is not yet
        // implemented. The offsetof formatter only needs the type_name node and the field token.
        let input = "BEGIN { n = offsetof(int, x); }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  n = offsetof(int, x);
}
"
        );
    }
}
