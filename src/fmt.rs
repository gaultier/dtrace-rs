use crate::ast::{Node, NodeId, NodeKind, Parser};

pub fn format(
    f: &mut std::fmt::Formatter<'_>,
    node_id: NodeId,
    nodes: &[Node],
    input: &str,
    indent: usize,
) -> std::fmt::Result {
    let node = &nodes[node_id];

    match &node.kind {
        NodeKind::Unknown => {
            let src = Parser::str_from_source(input, &node.origin);
            write!(f, "{:width$}{src}", "", width = indent, src = src)?;
        }
        NodeKind::Block(node_ids) => {
            write!(f, "{:width$}{{", "", width = indent)?;
            for id in node_ids {
                format(f, *id, nodes, input, indent + 2)?;
            }
            write!(f, "{:width$}}}", "", width = indent)?;
        }
        NodeKind::ProbeDefinition(probe, pred, actions) => {
            format(f, *probe, nodes, input, indent)?;

            if let Some(pred) = pred {
                write!(f, "\n{:width$}/", "", width = indent)?;
                format(f, *pred, nodes, input, indent)?;
                writeln!(f, " /")?;
            }

            if let Some(actions) = actions {
                write!(f, "{:width$}{{", "", width = indent)?;
                format(f, *actions, nodes, input, indent + 2)?;
                write!(f, "{:width$}}}", "", width = indent)?;
            }
        }
        NodeKind::Number(_) | NodeKind::Identifier(_) | NodeKind::ProbeSpecifier(_) => {
            let src = Parser::str_from_source(input, &node.origin);
            write!(f, "{:width$}{src}", "", width = indent, src = src)?;
        }
        NodeKind::Assignment(lhs, tok, rhs) | NodeKind::BinaryOp(lhs, tok, rhs) => {
            format(f, *lhs, nodes, input, indent)?;
            let src = Parser::str_from_source(input, &tok.origin);
            write!(f, " {} ", src)?;
            format(f, *rhs, nodes, input, indent)?;
        }
        NodeKind::If {
            cond,
            then_block,
            else_block,
        } => {
            todo!()
        }
        NodeKind::TranslationUnit(decls) => {
            todo!()
        }
        NodeKind::PrimaryToken(_) => {}
        NodeKind::Cast(_, _) => {}
        NodeKind::Aggregation(_) => {}
        NodeKind::ProbeSpecifiers(node_ids) | NodeKind::CommaExpr(node_ids) => {
            todo!()
        }
        NodeKind::SizeofType(_) => {
            todo!()
        }
        NodeKind::SizeofExpr(node_id) => todo!(),
        NodeKind::StringofExpr(node_id) => todo!(),
        NodeKind::PostfixIncDecrement(node_id, _token_kind) => todo!(),
        NodeKind::ExprStmt(node_id) => todo!(),
        NodeKind::EmptyStmt => {}
        NodeKind::PostfixArrayAccess(primary, args) | NodeKind::PostfixArguments(primary, args) => {
            todo!()
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
        | NodeKind::TypeSpecifier(_) => {}
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
        NodeKind::ParamEllipsis => {}
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
