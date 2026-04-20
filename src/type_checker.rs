use std::{collections::HashMap, fmt::Display};

use serde::Serialize;

use crate::{
    ast::{Node, NodeId, NodeKind},
    error::Error,
    lex::{Token, TokenKind},
    origin::Origin,
};

#[derive(Serialize, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum TypeKind {
    Void,
    Number,
    Any,
    Function(Type, Vec<Type>),
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size {
    _8,
    _16,
    _32,
    _64,
}

#[derive(Serialize, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Type {
    pub kind: Box<TypeKind>,
    pub size: Option<Size>,
    pub origin: Origin,
}

impl Default for Type {
    fn default() -> Self {
        Self {
            kind: Box::new(TypeKind::Any),
            size: None,
            origin: Origin::default(),
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self.kind {
            TypeKind::Void => f.write_str("void"),
            TypeKind::Number => write!(f, "int"),
            TypeKind::Any => f.write_str("any"),
            TypeKind::Function(ret, args) => {
                f.write_str("(")?;
                for arg in args {
                    arg.fmt(f)?;
                }
                f.write_str(")")?;

                match &*ret.kind {
                    TypeKind::Any => panic!("invalid return type: {:#?}", ret),
                    TypeKind::Void => {} // Noop
                    _ => {
                        ret.fmt(f)?;
                    }
                };
                Ok(())
            }
        }?;

        Ok(())
    }
}

impl Type {
    // TODO: Intern.
    pub(crate) fn new(kind: &TypeKind, size: &Option<Size>, origin: &Origin) -> Self {
        Self {
            kind: Box::new(kind.clone()),
            size: *size,
            origin: *origin,
        }
    }

    pub(crate) fn merge(&self, other: &Type, origin: &Origin) -> Result<Type, Error> {
        match (&*self.kind, &*other.kind) {
            (TypeKind::Function(_, _), TypeKind::Function(_, _)) => {
                if self == other {
                    Ok(self.clone())
                } else {
                    Err(Error::new_incompatible_types(origin, self, other))
                }
            }
            (TypeKind::Any, x) if x != &TypeKind::Any => Ok(other.clone()),
            (x, TypeKind::Any) if x != &TypeKind::Any => Ok(self.clone()),
            (TypeKind::Void, TypeKind::Void) => Ok(self.clone()),
            (TypeKind::Number, TypeKind::Number) => {
                if self.size == other.size {
                    Ok(self.clone())
                } else {
                    Err(Error::new_incompatible_types(origin, self, other))
                }
            }

            _ => Err(Error::new_incompatible_types(origin, self, other)),
        }
    }

    pub(crate) fn new_int() -> Self {
        Type::new(&TypeKind::Number, &Some(Size::_64), &Origin::new_builtin())
    }

    pub(crate) fn new_any() -> Self {
        Type::new(&TypeKind::Any, &Some(Size::_64), &Origin::new_builtin())
    }

    pub(crate) fn new_void() -> Self {
        Type::new(&TypeKind::Void, &None, &Origin::new_builtin())
    }

    pub(crate) fn new_function(return_type: &Type, args: &[Type], origin: &Origin) -> Self {
        Type::new(
            &TypeKind::Function(return_type.clone(), args.to_owned()),
            &Some(Size::_64),
            origin,
        )
    }
}

pub fn check_node(
    node_id: NodeId,
    nodes: &[Node],
    errs: &mut Vec<Error>,
    node_to_type: &mut HashMap<NodeId, Type>,
) {
    let node = &nodes[node_id];
    match &node.kind {
        NodeKind::Unknown => {}
        NodeKind::TranslationUnit(decls) => {
            for decl in decls {
                check_node(*decl, nodes, errs, node_to_type);
            }
        }
        NodeKind::Number(..) => {
            assert!(matches!(
                *node_to_type.get(&node_id).unwrap().kind,
                TypeKind::Number
            ));
        }
        NodeKind::ProbeSpecifier(_) => todo!(),
        NodeKind::Identifier(_identifier) => {
            todo!()
        }
        NodeKind::BinaryOp(
            lhs,
            Token {
                kind: TokenKind::Plus | TokenKind::Star | TokenKind::Slash,
                ..
            },
            rhs,
        ) => {
            check_node(*lhs, nodes, errs, node_to_type);
            check_node(*rhs, nodes, errs, node_to_type);

            let lhs_type = node_to_type.get(lhs).unwrap();
            let rhs_type = node_to_type.get(rhs).unwrap();
            let typ = lhs_type.merge(rhs_type, &node.origin);
            match typ {
                Ok(typ) => {
                    node_to_type.insert(node_id, typ);
                }
                Err(err) => {
                    errs.push(err);
                    // To avoid cascading errors, pretend the type is fine.
                    node_to_type.insert(node_id, Type::new_int());
                }
            }
        }
        NodeKind::BinaryOp(
            lhs,
            Token {
                kind: TokenKind::EqEq,
                ..
            },
            rhs,
        ) => {
            check_node(*lhs, nodes, errs, node_to_type);
            check_node(*rhs, nodes, errs, node_to_type);

            let lhs_type = node_to_type.get(lhs).unwrap();
            let rhs_type = node_to_type.get(rhs).unwrap();
            let typ = lhs_type.merge(rhs_type, &node.origin);
            if let Err(err) = typ {
                errs.push(err);
            }
            node_to_type.insert(node_id, Type::new_int());
        }
        NodeKind::BinaryOp(_lhs, _, _rhs) => {
            unreachable!()
        }
        NodeKind::If {
            cond,
            then_block,
            else_block,
        } => {
            check_node(*cond, nodes, errs, node_to_type);
            check_node(*then_block, nodes, errs, node_to_type);
            if let Some(else_block) = else_block {
                check_node(*else_block, nodes, errs, node_to_type);
            }
        }
        NodeKind::Block(stmts) => {
            for stmt in stmts {
                check_node(*stmt, nodes, errs, node_to_type);
            }
        }
        NodeKind::Unary(_, expr) => {
            check_node(*expr, nodes, errs, node_to_type);
        }
        NodeKind::ArgumentsDeclaration(args) => {
            if let Some(_args) = args {
                todo!()
            }
        }
        NodeKind::ProbeDefinition(probe, pred, actions) => {
            check_node(*probe, nodes, errs, node_to_type);
            if let Some(pred) = pred {
                check_node(*pred, nodes, errs, node_to_type);
            }

            if let Some(actions) = actions {
                check_node(*actions, nodes, errs, node_to_type);
            }
        }
        NodeKind::Assignment(lhs, op, rhs) => {
            check_node(*lhs, nodes, errs, node_to_type);
            check_node(*rhs, nodes, errs, node_to_type);

            let lhs_type = node_to_type.get(lhs).unwrap();
            let rhs_type = node_to_type.get(rhs).unwrap();
            if let Err(err) = lhs_type.merge(rhs_type, &op.origin) {
                errs.push(err);
            }
        }
        NodeKind::PrimaryToken(_) => {}
        NodeKind::Cast(_, _) => todo!(),
        NodeKind::Aggregation => todo!(),
        NodeKind::CommaExpr(_node_ids) => todo!(),
        NodeKind::Sizeof(_) => todo!(),
        NodeKind::StringofExpr(_node_id) => todo!(),
        NodeKind::PostfixIncDecrement(_node_id, _token_kind) => todo!(),
        NodeKind::ExprStmt(_node_id) => todo!(),
        NodeKind::EmptyStmt => todo!(),
        NodeKind::PostfixArguments(_, _node_id) => todo!(),
        NodeKind::TernaryExpr(_, _, _) => todo!(),
        NodeKind::PostfixArrayAccess(_node_id, _node_id1) => todo!(),
        NodeKind::FieldAccess(_node_id, _token_kind, _token) => todo!(),
        NodeKind::ProbeSpecifiers(_node_ids) => todo!(),
        NodeKind::TypeName(_node_id, _node_id1) => todo!(),
        NodeKind::OffsetOf(_node_id, _token) => todo!(),
        NodeKind::Declaration(_, _) => todo!(),
        NodeKind::DeclarationSpecifiers(_tokens) => todo!(),
        NodeKind::DirectDeclarator(_, _) => todo!(),
        NodeKind::Declarator(_node_id, _node_id1) => todo!(),
        NodeKind::InitDeclarators(_node_ids) => todo!(),
        NodeKind::TypeQualifier(_token_kind) => todo!(),
        NodeKind::DStorageClassSpecifier(_token_kind) => todo!(),
        NodeKind::StorageClassSpecifier(_token_kind) => todo!(),
        NodeKind::TypeSpecifier(_token_kind) => todo!(),
        NodeKind::EnumDeclaration(_token, _node_ids) => todo!(),
        NodeKind::EnumeratorDeclaration(_token, _node_id) => todo!(),
        NodeKind::EnumeratorsDeclaration(_node_ids) => todo!(),
        NodeKind::StructDeclaration(_, _node_id) => todo!(),
        NodeKind::StructFieldsDeclaration(_node_ids) => todo!(),
        NodeKind::StructFieldDeclarator(_node_id, _node_id1) => todo!(),
        NodeKind::StructFieldDeclaration(_node_id, _node_id1) => todo!(),
        NodeKind::StructFieldDeclaratorList(_node_ids) => todo!(),
        NodeKind::SpecifierQualifierList(_node_ids) => todo!(),
        NodeKind::Xlate(_node_id, _node_id1) => todo!(),
        NodeKind::DirectAbstractDeclarator(_node_id) => todo!(),
        NodeKind::DirectAbstractArray(_node_id, _node_id1) => todo!(),
        NodeKind::DirectAbstractFunction(_node_id, _node_id1) => todo!(),
        NodeKind::AbstractDeclarator(_node_id, _node_id1) => todo!(),
        NodeKind::Pointer(_node_ids, _node_id) => todo!(),
        NodeKind::Array(_node_id) => todo!(),
        NodeKind::ParamEllipsis => todo!(),
        NodeKind::Parameters(_node_ids) => todo!(),
        NodeKind::ParameterDeclarationSpecifiers(_node_ids) => todo!(),
        NodeKind::Character(_) => todo!(),
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
}

pub fn check_nodes(nodes: &[Node], node_to_type: &mut HashMap<NodeId, Type>) -> Vec<Error> {
    assert!(!nodes.is_empty());

    let mut errs = Vec::new();

    check_node(NodeId(0), nodes, &mut errs, node_to_type);

    errs
}

impl Size {
    pub(crate) fn as_bytes_count(&self) -> usize {
        match self {
            Size::_8 => 1,
            Size::_16 => 2,
            Size::_32 => 4,
            Size::_64 => 8,
        }
    }
}

impl Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Size::_8 => "BYTE PTR",
            Size::_16 => "WORD PTR",
            Size::_32 => "DWORD PTR",
            Size::_64 => "QWORD PTR",
        };
        f.write_str(s)
    }
}
