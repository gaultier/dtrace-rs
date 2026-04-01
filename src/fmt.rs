use std::io::Write;

use crate::ast::{Node, NodeId, NodeKind, Parser};

fn indentify<W: Write>(w: &mut W, indent: usize, initial_indent: bool) -> std::io::Result<()> {
    if initial_indent {
        write!(w, "{:width$}", "", width = indent)?;
    }
    Ok(())
}

pub fn format<W: Write>(
    w: &mut W,
    node_id: NodeId,
    nodes: &[Node],
    input: &str,
    indent: usize,
    initial_indent: bool,
) -> std::io::Result<()> {
    let node = &nodes[node_id];

    match &node.kind {
        NodeKind::Unknown | NodeKind::Character => {
            let src = Parser::str_from_source(input, &node.origin);
            w.write_all(src.as_bytes())?;
        }
        NodeKind::Block(node_ids) => {
            indentify(w, indent, initial_indent)?;
            w.write_all(b"{\n")?;
            for id in node_ids {
                format(w, *id, nodes, input, indent + 2, true)?;
            }
            indentify(w, indent, true)?;
            w.write_all(b"}\n")?;
        }
        NodeKind::ProbeDefinition(probe, pred, actions) => {
            format(w, *probe, nodes, input, indent, true)?;
            w.write_all(b"\n")?;

            if let Some(pred) = pred {
                indentify(w, indent, initial_indent)?;
                w.write_all(b"/ ")?;
                format(w, *pred, nodes, input, indent, true)?;
                w.write_all(b" /")?;
            }

            if let Some(actions) = actions {
                format(w, *actions, nodes, input, indent, true)?;
            }
            w.write_all(b"\n")?;
        }
        NodeKind::Number(_) | NodeKind::Identifier(_) | NodeKind::ProbeSpecifier(_) => {
            let src = Parser::str_from_source(input, &node.origin);
            w.write_all(src.as_bytes())?;
        }
        NodeKind::Assignment(lhs, tok, rhs) | NodeKind::BinaryOp(lhs, tok, rhs) => {
            format(w, *lhs, nodes, input, indent, true)?;
            let src = Parser::str_from_source(input, &tok.origin);
            write!(w, " {} ", src)?;
            format(w, *rhs, nodes, input, indent, true)?;
        }
        NodeKind::If {
            cond,
            then_block,
            else_block,
        } => {
            indentify(w, indent, initial_indent)?;
            w.write_all(b"if (")?;
            format(w, *cond, nodes, input, indent, true)?;
            w.write_all(b") ")?;

            let then_block_node = &nodes[*then_block];
            if matches!(then_block_node.kind, NodeKind::Block { .. }) {
                format(w, *then_block, nodes, input, indent, true)?;
            } else {
                // Simulate block.
                w.write_all(b"{\n")?;

                indentify(w, indent + 2, true)?;
                format(w, *then_block, nodes, input, indent + 2, true)?;
                w.write_all(b";\n")?;

                indentify(w, indent, true)?;
                w.write_all(b"}\n")?;
            }

            if let Some(else_block) = else_block {
                indentify(w, indent, initial_indent)?;
                w.write_all(b"else")?;

                let else_block_node = &nodes[*else_block];
                if matches!(else_block_node.kind, NodeKind::If { .. }) {
                    w.write_all(b" ")?;
                    format(w, *else_block, nodes, input, indent, false)?;
                } else {
                    w.write_all(b"\n")?;
                    format(w, *else_block, nodes, input, indent, true)?;
                }
            }
        }
        NodeKind::TranslationUnit(decls) => {
            for decl in decls {
                format(w, *decl, nodes, input, indent, true)?;
            }
        }
        NodeKind::PrimaryToken(_) => {
            let src = Parser::str_from_source(input, &node.origin);
            w.write_all(src.as_bytes())?;
        }
        NodeKind::Cast(_, _) => {
            todo!()
        }
        NodeKind::Aggregation(s) => {
            w.write_all(s.as_bytes())?;
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
            indentify(w, indent, initial_indent)?;
            format(w, *node_id, nodes, input, indent, true)?;
            w.write_all(b";\n")?;
        }
        NodeKind::EmptyStmt => {}
        NodeKind::PostfixArrayAccess(primary, args) => {
            todo!()
        }
        NodeKind::PostfixArguments(primary, args) => {
            format(w, *primary, nodes, input, indent, true)?;
            w.write_all(b"(")?;
            if let Some(args) = args {
                format(w, *args, nodes, input, indent, true)?;
            }
            w.write_all(b")")?;
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
