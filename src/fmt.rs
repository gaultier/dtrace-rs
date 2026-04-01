use std::io::Write;

use crate::ast::{Node, NodeId, NodeKind, Parser};

pub fn format<W: Write>(
    w: &mut W,
    node_id: NodeId,
    nodes: &[Node],
    input: &str,
    indent: usize,
) -> std::io::Result<()> {
    let node = &nodes[node_id];

    match &node.kind {
        NodeKind::Unknown | NodeKind::Character => {
            let src = Parser::str_from_source(input, &node.origin);
            write!(w, "{:width$}{src}", "", width = indent, src = src)?;
        }
        NodeKind::Block(node_ids) => {
            writeln!(w, "{:width$}{{", "", width = indent)?;
            for id in node_ids {
                format(w, *id, nodes, input, indent + 2)?;
            }
            writeln!(w, "{:width$}}}", "", width = indent)?;
        }
        NodeKind::ProbeDefinition(probe, pred, actions) => {
            format(w, *probe, nodes, input, indent)?;
            writeln!(w, "")?;

            if let Some(pred) = pred {
                write!(w, "{:width$}/", "", width = indent)?;
                format(w, *pred, nodes, input, indent)?;
                writeln!(w, " /")?;
            }

            if let Some(actions) = actions {
                format(w, *actions, nodes, input, indent)?;
            }
            writeln!(w, "")?;
        }
        NodeKind::Number(_) | NodeKind::Identifier(_) | NodeKind::ProbeSpecifier(_) => {
            let src = Parser::str_from_source(input, &node.origin);
            write!(w, "{:width$}{src}", "", width = indent, src = src)?;
        }
        NodeKind::Assignment(lhs, tok, rhs) | NodeKind::BinaryOp(lhs, tok, rhs) => {
            format(w, *lhs, nodes, input, indent)?;
            let src = Parser::str_from_source(input, &tok.origin);
            write!(w, " {} ", src)?;
            format(w, *rhs, nodes, input, indent)?;
        }
        NodeKind::If {
            cond,
            then_block,
            else_block,
        } => {
            todo!()
        }
        NodeKind::TranslationUnit(decls) => {
            for decl in decls {
                format(w, *decl, nodes, input, indent)?;
            }
        }
        NodeKind::PrimaryToken(_) => {
            let src = Parser::str_from_source(input, &node.origin);
            w.write(src.as_bytes())?;
        }
        NodeKind::Cast(_, _) => {
            todo!()
        }
        NodeKind::Aggregation(_) => {
            todo!()
        }
        NodeKind::ProbeSpecifiers(node_ids) | NodeKind::CommaExpr(node_ids) => {
            todo!()
        }
        NodeKind::SizeofType(_) => {
            todo!()
        }
        NodeKind::SizeofExpr(node_id) => todo!(),
        NodeKind::StringofExpr(node_id) => todo!(),
        NodeKind::PostfixIncDecrement(node_id, _token_kind) => todo!(),
        NodeKind::ExprStmt(node_id) => {
            format(w, *node_id, nodes, input, indent)?;
            writeln!(w, ";")?;
        }
        NodeKind::EmptyStmt => {
            todo!()
        }
        NodeKind::PostfixArrayAccess(primary, args) => {
            todo!()
        }
        NodeKind::PostfixArguments(primary, args) => {
            format(w, *primary, nodes, input, indent)?;
            write!(w, "(")?;
            if let Some(args) = args {
                format(w, *args, nodes, input, indent)?;
            }
            write!(w, ")")?;
        }
        NodeKind::TernaryExpr(lhs, mhs, rhs) => {
            todo!()
        }
        NodeKind::FieldAccess(node_id, _, _) => {
            todo!()
        }
        NodeKind::TypeName(specifier, declarator) => {
            todo!()
        }
        NodeKind::OffsetOf(node_id, _token) => {
            todo!()
        }
        NodeKind::Declaration(decl_specifiers, init_declarator_list) => {
            todo!()
        }
        NodeKind::DeclarationSpecifiers(node_ids) => {
            todo!()
        }
        NodeKind::DirectDeclarator(base, suffix) => {
            todo!()
        }
        NodeKind::Declarator(ptr, declarator) => {
            todo!()
        }
        NodeKind::InitDeclarators(node_ids) => {
            todo!()
        }
        NodeKind::TypeQualifier(_)
        | NodeKind::DStorageClassSpecifier(_)
        | NodeKind::StorageClassSpecifier(_)
        | NodeKind::TypeSpecifier(_) => {
            todo!()
        }
        NodeKind::EnumDeclaration(_token, node_id) => {
            todo!()
        }
        NodeKind::EnumeratorDeclaration(_token, node_id) => {
            todo!()
        }
        NodeKind::EnumeratorsDeclaration(node_ids) => {
            todo!()
        }
        NodeKind::StructDeclaration(_, node_id) => {
            todo!()
        }
        NodeKind::StructFieldsDeclaration(node_ids) => {
            todo!()
        }
        NodeKind::StructFieldDeclarator(declarator, const_expr) => {
            todo!()
        }
        NodeKind::StructFieldDeclaration(specifier_qualifier_list, declarator_list) => {
            todo!()
        }
        NodeKind::StructFieldDeclaratorList(node_ids) => {
            todo!()
        }
        NodeKind::SpecifierQualifierList(node_ids) => {
            todo!()
        }
        NodeKind::Xlate(type_name, expr) => {
            todo!()
        }
        NodeKind::DirectAbstractDeclarator(node_id) => {
            todo!()
        }
        NodeKind::DirectAbstractArray(base, suffix) => {
            todo!()
        }
        NodeKind::DirectAbstractFunction(base, suffix) => {
            todo!()
        }
        NodeKind::AbstractDeclarator(ptr, abstract_decl) => {
            todo!()
        }
        NodeKind::Pointer(type_qualifiers, ptr) => {
            todo!()
        }
        NodeKind::Array(params) => {
            todo!()
        }
        NodeKind::ParamEllipsis => {
            todo!()
        }
        NodeKind::Parameters(node_ids) => {
            todo!()
        }
        NodeKind::ParameterDeclarationSpecifiers(node_ids) => {
            todo!()
        }
        NodeKind::Unary(token_kind, node_id) => todo!(),
        NodeKind::Arguments(node_ids) => todo!(),
    }
    Ok(())
}
