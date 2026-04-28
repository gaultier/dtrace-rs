use std::collections::HashMap;

use crate::{
    ast::{Node, NodeId},
    error::{Error, ErrorKind},
    lex::str_from_source,
};

pub(crate) struct Resolver<'a> {
    declarations: HashMap<String, NodeId>,
    nodes: &'a [Node],
    errors: &'a mut Vec<Error>,
    input: &'a str,
}

impl<'a> Resolver<'a> {
    pub(crate) fn resolve(&mut self, node_id: NodeId) {
        let node = &self.nodes[node_id];
        match &node.kind {
            crate::ast::NodeKind::Declaration {
                specifiers,
                declarators,
            } => {
                self.resolve(*specifiers);
                if let Some(declarators) = declarators {
                    self.resolve(*declarators);
                }
            }
            crate::ast::NodeKind::DeclarationSpecifiers(node_ids) => {
                for node_id in node_ids {
                    self.resolve(*node_id);
                }
            }
            crate::ast::NodeKind::DirectDeclarator { ident: _, suffix } => {
                // TODO: Handle `ident`.

                if let Some(suffix) = suffix {
                    self.resolve(*suffix);
                }
            }
            crate::ast::NodeKind::Declarator { pointer, direct } => {
                if let Some(pointer) = pointer {
                    self.resolve(*pointer);
                }
                self.resolve(*direct);
            }
            crate::ast::NodeKind::InitDeclarators(node_ids) => {
                for node_id in node_ids {
                    self.resolve(*node_id);
                }
            }
            crate::ast::NodeKind::EnumDeclaration { name, enumerators } => {
                if let Some(name) = name {
                    let s = str_from_source(self.input, name.origin);
                    let existing = self.declarations.insert(format!("enum {}", s), node_id);

                    if let Some(existing) = existing {
                        self.errors.push(Error {
                            kind: ErrorKind::Redeclaration,
                            origin: node.origin,
                            explanation: String::from("enum already declared"),
                            related_origin: Some(self.nodes[existing].origin),
                        });
                    }
                }

                if let Some(enumerators) = enumerators {
                    self.resolve(*enumerators);
                }
            }
            crate::ast::NodeKind::EnumeratorDeclaration { name: _, value } => {
                // TODO: Handle `name`.

                if let Some(value) = value {
                    self.resolve(*value);
                }
            }
            crate::ast::NodeKind::EnumeratorsDeclaration(node_ids) => {
                for node_id in node_ids {
                    self.resolve(*node_id);
                }
            }
            crate::ast::NodeKind::StructDeclaration { name, fields } => {
                if let Some(name) = name {
                    let s = str_from_source(self.input, name.origin);
                    let existing = self.declarations.insert(format!("struct {}", s), node_id);

                    if let Some(existing) = existing {
                        self.errors.push(Error {
                            kind: ErrorKind::Redeclaration,
                            origin: node.origin,
                            explanation: String::from("struct already declared"),
                            related_origin: Some(self.nodes[existing].origin),
                        });
                    }
                }

                if let Some(fields) = fields {
                    self.resolve(*fields);
                }
            }
            crate::ast::NodeKind::StructFieldsDeclaration(node_ids) => {
                for node_id in node_ids {
                    self.resolve(*node_id);
                }
            }
            crate::ast::NodeKind::StructFieldDeclarator {
                declarator: _,
                bit_field: _,
            } => {}
            crate::ast::NodeKind::StructFieldDeclaration {
                specifiers,
                declarators,
            } => {
                self.resolve(*specifiers);
                if let Some(declarators) = declarators {
                    self.resolve(*declarators);
                }
            }
            crate::ast::NodeKind::StructFieldDeclaratorList(node_ids) => {
                for node_id in node_ids {
                    self.resolve(*node_id);
                }
            }
            crate::ast::NodeKind::SpecifierQualifierList(node_ids) => {
                for node_id in node_ids {
                    self.resolve(*node_id);
                }
            }
            crate::ast::NodeKind::InlineDefinition {
                typ,
                declarator,
                expr,
            } => {
                self.resolve(*typ);
                self.resolve(*declarator);
                self.resolve(*expr);
            }
            crate::ast::NodeKind::UnionDeclaration { name, fields } => {
                if let Some(name) = name {
                    let s = str_from_source(self.input, name.origin);
                    let existing = self.declarations.insert(format!("union {}", s), node_id);

                    if let Some(existing) = existing {
                        self.errors.push(Error {
                            kind: ErrorKind::Redeclaration,
                            origin: node.origin,
                            explanation: String::from("union already declared"),
                            related_origin: Some(self.nodes[existing].origin),
                        });
                    }
                }
                if let Some(fields) = fields {
                    self.resolve(*fields);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn new(nodes: &'a [Node], input: &'a str, errors: &'a mut Vec<Error>) -> Self {
        Self {
            declarations: HashMap::with_capacity(128),
            nodes,
            errors,
            input,
        }
    }
}
