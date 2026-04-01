use std::io::Write;

use crate::ast::{Node, NodeId, NodeKind, Parser};

fn indentify<W: Write>(w: &mut W, indent: usize, with_heading_indent: bool) -> std::io::Result<()> {
    if with_heading_indent {
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
    with_heading_indent: bool,
    with_trailing_newline: bool,
) -> std::io::Result<()> {
    let node = &nodes[node_id];

    match &node.kind {
        NodeKind::Unknown | NodeKind::Character => {
            let src = Parser::str_from_source(input, &node.origin);
            w.write_all(src.as_bytes())?;
        }
        NodeKind::Block(node_ids) => {
            indentify(w, indent, with_heading_indent)?;
            w.write_all(b"{\n")?;
            for id in node_ids {
                format(w, *id, nodes, input, indent + 2, true, true)?;
            }
            indentify(w, indent, true)?;
            w.write_all(b"}")?;
            if with_trailing_newline {
                w.write_all(b"\n")?;
            }
        }
        NodeKind::ProbeDefinition(probe, pred, actions) => {
            format(w, *probe, nodes, input, indent, true, true)?;
            w.write_all(b"\n")?;

            if let Some(pred) = pred {
                indentify(w, indent, with_heading_indent)?;
                w.write_all(b"/ ")?;
                format(w, *pred, nodes, input, indent, true, true)?;
                w.write_all(b" /")?;
            }

            if let Some(actions) = actions {
                format(w, *actions, nodes, input, indent, true, true)?;
            }
            w.write_all(b"\n")?;
        }
        NodeKind::Number(_) | NodeKind::Identifier(_) | NodeKind::ProbeSpecifier(_) => {
            let src = Parser::str_from_source(input, &node.origin);
            w.write_all(src.as_bytes())?;
        }
        NodeKind::Assignment(lhs, tok, rhs) | NodeKind::BinaryOp(lhs, tok, rhs) => {
            format(w, *lhs, nodes, input, indent, true, true)?;
            let src = Parser::str_from_source(input, &tok.origin);
            write!(w, " {} ", src)?;
            format(w, *rhs, nodes, input, indent, true, true)?;
        }
        NodeKind::If {
            cond,
            then_block,
            else_block,
        } => {
            indentify(w, indent, with_heading_indent)?;
            w.write_all(b"if (")?;
            format(w, *cond, nodes, input, indent, false, false)?;
            w.write_all(b") ")?;

            let then_block_node = &nodes[*then_block];
            if matches!(then_block_node.kind, NodeKind::Block { .. }) {
                format(w, *then_block, nodes, input, indent, false, false)?;
            } else {
                // Simulate block.
                w.write_all(b"{\n")?;

                indentify(w, indent + 2, true)?;
                format(w, *then_block, nodes, input, indent + 2, true, false)?;
                w.write_all(b";\n")?;

                indentify(w, indent, true)?;
                w.write_all(b"}\n")?;
            }

            if let Some(else_block) = else_block {
                w.write_all(b" else ")?;

                let else_block_node = &nodes[*else_block];
                if matches!(
                    else_block_node.kind,
                    NodeKind::Block { .. } | NodeKind::If { .. }
                ) {
                    format(w, *else_block, nodes, input, indent, false, true)?;
                } else {
                    // Simulate block.
                    w.write_all(b"{\n")?;

                    indentify(w, indent + 2, true)?;
                    format(w, *else_block, nodes, input, indent + 2, true, false)?;
                    w.write_all(b";\n")?;

                    indentify(w, indent, true)?;
                    w.write_all(b"}\n")?;
                }
            } else {
                // No else, we need to write the newline ourselves.
                w.write_all(b"\n")?;
            }
        }
        NodeKind::TranslationUnit(decls) => {
            for decl in decls {
                format(w, *decl, nodes, input, indent, true, true)?;
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
        NodeKind::ProbeSpecifiers(_node_ids) | NodeKind::CommaExpr(_node_ids) => {
            todo!()
        }
        NodeKind::SizeofType(_) => {
            todo!()
        }
        NodeKind::SizeofExpr(_node_id) => todo!(),
        NodeKind::StringofExpr(_node_id) => todo!(),
        NodeKind::PostfixIncDecrement(_node_id, _token_kind) => todo!(),
        NodeKind::ExprStmt(node_id) => {
            indentify(w, indent, with_heading_indent)?;
            format(w, *node_id, nodes, input, indent, true, true)?;
            w.write_all(b";")?;
            if with_trailing_newline {
                w.write_all(b"\n")?;
            }
        }
        NodeKind::EmptyStmt => {}
        NodeKind::PostfixArrayAccess(_primary, _args) => {
            todo!()
        }
        NodeKind::PostfixArguments(primary, args) => {
            format(w, *primary, nodes, input, indent, true, true)?;
            w.write_all(b"(")?;
            if let Some(args) = args {
                format(w, *args, nodes, input, indent, true, true)?;
            }
            w.write_all(b")")?;
        }
        NodeKind::TernaryExpr(_lhs, _mhs, _rhs) => {
            todo!()
        }
        NodeKind::FieldAccess(_node_id, _, _) => {
            todo!()
        }
        NodeKind::TypeName(_specifier, _declarator) => {
            todo!()
        }
        NodeKind::OffsetOf(_node_id, _token) => {
            todo!()
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
            todo!()
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
        NodeKind::SpecifierQualifierList(_node_ids) => {
            todo!()
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
        NodeKind::ParamEllipsis => {
            todo!()
        }
        NodeKind::Parameters(_node_ids) => {
            todo!()
        }
        NodeKind::ParameterDeclarationSpecifiers(_node_ids) => {
            todo!()
        }
        NodeKind::Unary(_token_kind, _node_id) => todo!(),
        NodeKind::Arguments(_node_ids) => todo!(),
    }
    Ok(())
}
