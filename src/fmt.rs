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
            f.write_str(src)?;
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
        NodeKind::Number(_) | NodeKind::Identifier(_) | NodeKind::ProbeSpecifier(_) => {}
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
    }
    Ok(())
}
