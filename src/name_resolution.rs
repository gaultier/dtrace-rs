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
            // The root of every file. Without this arm `resolve(root)` fell
            // straight into the catch-all below and the whole module was a
            // no-op, so none of the redeclaration checks ever ran.
            crate::ast::NodeKind::TranslationUnit(node_ids)
            | crate::ast::NodeKind::Block(node_ids) => {
                for node_id in node_ids {
                    self.resolve(*node_id);
                }
            }
            crate::ast::NodeKind::ProbeDefinition {
                probe_specifiers: _,
                predicate,
                action,
            } => {
                if let Some(predicate) = predicate {
                    self.resolve(*predicate);
                }
                if let Some(action) = action {
                    self.resolve(*action);
                }
            }
            crate::ast::NodeKind::ExprStmt(node_id) => {
                self.resolve(*node_id);
            }
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
            crate::ast::NodeKind::DirectDeclarator {
                ident: _,
                suffix: Some(suffix),
            } => {
                // TODO: Handle `ident`.

                self.resolve(*suffix);
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
                // A tag without a body is a forward declaration, which may
                // repeat and may precede the definition. Only definitions
                // conflict with each other.
                if let Some(name) = name
                    && enumerators.is_some()
                {
                    let s = str_from_source(self.input, name.origin);
                    let existing = self.declarations.insert(format!("enum {}", s), node_id);

                    if let Some(existing) = existing {
                        self.errors.push(Error {
                            kind: ErrorKind::Redeclaration,
                            // Point at `enum Name` rather than the whole
                            // declaration, whose body can span many lines.
                            origin: Self::tag_origin(node, name.origin),
                            explanation: format!("{} is already defined", s),
                            related_origin: Some(Self::tag_origin(
                                &self.nodes[existing],
                                self.tag_name_origin(existing),
                            )),
                        });
                    }
                }

                if let Some(enumerators) = enumerators {
                    self.resolve(*enumerators);
                }
            }
            crate::ast::NodeKind::EnumeratorDeclaration {
                name: _,
                value: Some(value),
            } => {
                // TODO: Handle `name`.

                self.resolve(*value);
            }
            crate::ast::NodeKind::EnumeratorsDeclaration(node_ids) => {
                for node_id in node_ids {
                    self.resolve(*node_id);
                }
            }
            crate::ast::NodeKind::StructDeclaration { name, fields } => {
                // A tag without a body is a forward declaration, which may
                // repeat and may precede the definition. Only definitions
                // conflict with each other.
                if let Some(name) = name
                    && fields.is_some()
                {
                    let s = str_from_source(self.input, name.origin);
                    let existing = self.declarations.insert(format!("struct {}", s), node_id);

                    if let Some(existing) = existing {
                        self.errors.push(Error {
                            kind: ErrorKind::Redeclaration,
                            // Point at `struct Name` rather than the whole
                            // declaration, whose body can span many lines.
                            origin: Self::tag_origin(node, name.origin),
                            explanation: format!("{} is already defined", s),
                            related_origin: Some(Self::tag_origin(
                                &self.nodes[existing],
                                self.tag_name_origin(existing),
                            )),
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
                // A tag without a body is a forward declaration, which may
                // repeat and may precede the definition. Only definitions
                // conflict with each other.
                if let Some(name) = name
                    && fields.is_some()
                {
                    let s = str_from_source(self.input, name.origin);
                    let existing = self.declarations.insert(format!("union {}", s), node_id);

                    if let Some(existing) = existing {
                        self.errors.push(Error {
                            kind: ErrorKind::Redeclaration,
                            // Point at `union Name` rather than the whole
                            // declaration, whose body can span many lines.
                            origin: Self::tag_origin(node, name.origin),
                            explanation: format!("{} is already defined", s),
                            related_origin: Some(Self::tag_origin(
                                &self.nodes[existing],
                                self.tag_name_origin(existing),
                            )),
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

    /// The origin of `struct Name` / `union Name` / `enum Name`: from the
    /// start of the declaration (the keyword) through the end of the tag.
    fn tag_origin(node: &Node, name_origin: crate::origin::Origin) -> crate::origin::Origin {
        node.origin.merge(name_origin)
    }

    /// The origin of the tag name of a previously recorded declaration.
    fn tag_name_origin(&self, node_id: NodeId) -> crate::origin::Origin {
        match &self.nodes[node_id].kind {
            crate::ast::NodeKind::StructDeclaration {
                name: Some(name), ..
            }
            | crate::ast::NodeKind::UnionDeclaration {
                name: Some(name), ..
            }
            | crate::ast::NodeKind::EnumDeclaration {
                name: Some(name), ..
            } => name.origin,
            _ => self.nodes[node_id].origin,
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
