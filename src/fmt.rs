use std::io::Write;

use crate::{
    ast::{Node, NodeId, NodeKind},
    lex::{self, Comment, CommentKind, ControlDirective, ControlDirectiveKind, TokenKind},
};

struct Formatter<'a, W> {
    w: &'a mut W,
    nodes: &'a [Node],
    /// All comments from the lexer, sorted by source position.
    comments: &'a [Comment],
    /// Index of the next comment not yet emitted.
    comment_idx: usize,
    /// All control directives (pragmas, `#line`, shebangs) from the lexer, sorted by position.
    directives: &'a [ControlDirective],
    /// Index of the next directive not yet emitted.
    directive_idx: usize,
    input: &'a str,
}

impl<'a, W: Write> Formatter<'a, W> {
    fn indent(&mut self, n: usize) -> std::io::Result<()> {
        write!(self.w, "{:width$}", "", width = n)
    }

    /// Emits one comment at the current `comment_idx`, advancing the index.
    fn emit_one_comment(&mut self, indent: usize) -> std::io::Result<()> {
        let comment = &self.comments[self.comment_idx];
        self.indent(indent)?;
        let text = lex::str_from_source(self.input, comment.origin);
        self.w.write_all(text.as_bytes())?;
        // `//` comments stop before the newline; `/* */` does not include a trailing
        // newline, so we always add one.
        self.w.write_all(b"\n")?;
        // Blank line after multi-line comments to visually separate them from the
        // following declaration or statement.
        if comment.kind == CommentKind::MultiLine {
            self.w.write_all(b"\n")?;
        }
        self.comment_idx += 1;
        Ok(())
    }

    /// Emits one directive at the current `directive_idx`, advancing the index.
    fn emit_one_directive(&mut self, indent: usize) -> std::io::Result<()> {
        let directive = &self.directives[self.directive_idx];
        match &directive.kind {
            ControlDirectiveKind::Ignored => {
                // Null directives (`#` with nothing after) have a zero-length origin;
                // skip them entirely.  Non-null ignored directives (`#ident`, unknown
                // pragmas) are preserved verbatim.
                let text = lex::str_from_source(self.input, directive.origin);
                if !text.is_empty() {
                    self.indent(indent)?;
                    self.w.write_all(text.as_bytes())?;
                    self.w.write_all(b"\n")?;
                }
            }
            ControlDirectiveKind::PragmaError(msg) => {
                // The lexer stores only the message portion in the origin, so
                // reconstruct the full directive header.
                self.indent(indent)?;
                writeln!(self.w, "#pragma D error {}", msg)?;
            }
            _ => {
                // All other directive kinds have origins that span from `#` to the
                // end of the directive line, so the raw source text is complete.
                self.indent(indent)?;
                let text = lex::str_from_source(self.input, directive.origin);
                self.w.write_all(text.as_bytes())?;
                self.w.write_all(b"\n")?;
            }
        }
        self.directive_idx += 1;
        Ok(())
    }

    /// Emits every not-yet-emitted comment or directive whose start byte is strictly
    /// less than `before_byte`, in source order.  Both queues are advanced together so
    /// the interleaved original order is preserved.
    ///
    /// After all annotations are emitted, a blank line is inserted when the source
    /// contains one between the last annotation and the following node — except when
    /// the last annotation was a multi-line comment, which already appends a blank line
    /// unconditionally.
    fn emit_pending_annotations(&mut self, before_byte: u32, indent: usize) -> std::io::Result<()> {
        // Inclusive byte offset of the last annotation emitted in this call, if any.
        let mut last_annotation_end: Option<u32> = None;
        // Whether the very last annotation was a multi-line comment (which already emits
        // a trailing blank line, so we must not emit a second one).
        let mut last_was_multiline_comment = false;

        loop {
            let next_comment = self
                .comments
                .get(self.comment_idx)
                .map(|c| c.origin.start.byte_offset)
                .unwrap_or(u32::MAX);
            let next_directive = self
                .directives
                .get(self.directive_idx)
                .map(|d| d.origin.start.byte_offset)
                .unwrap_or(u32::MAX);
            let next = next_comment.min(next_directive);
            if next >= before_byte {
                break;
            }
            if next_comment <= next_directive {
                last_was_multiline_comment =
                    self.comments[self.comment_idx].kind == CommentKind::MultiLine;
                last_annotation_end = Some(self.comments[self.comment_idx].origin.end.byte_offset);
                self.emit_one_comment(indent)?;
            } else {
                last_was_multiline_comment = false;
                last_annotation_end =
                    Some(self.directives[self.directive_idx].origin.end.byte_offset);
                self.emit_one_directive(indent)?;
            }
        }

        // When the source has a blank line between the last annotation and the following
        // node, preserve it in the output.  Multi-line comments are excluded because
        // `emit_one_comment` already appends a blank line unconditionally.
        if !last_was_multiline_comment && let Some(end) = last_annotation_end {
            // `origin.end.byte_offset` is the exclusive end (one past the last
            // content byte), so the gap starts directly at `end`.
            let gap_start = end as usize;
            let gap_end = (before_byte as usize).min(self.input.len());
            if gap_start < gap_end && self.input[gap_start..gap_end].contains("\n\n") {
                self.w.write_all(b"\n")?;
            }
        }

        Ok(())
    }

    /// Returns `true` if the innermost `Pointer` chain ends with a type-qualifier keyword
    /// rather than a bare `*`. Callers use this to decide whether a space is needed between
    /// a pointer and the following declarator name (e.g. `* const x` vs. `*x`).
    fn pointer_ends_with_qualifier(nodes: &[Node], ptr_id: NodeId) -> bool {
        match &nodes[ptr_id].kind {
            NodeKind::Pointer(qualifiers, inner) => {
                if let Some(inner_ptr) = inner {
                    Self::pointer_ends_with_qualifier(nodes, *inner_ptr)
                } else {
                    !qualifiers.is_empty()
                }
            }
            _ => false,
        }
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
                for id in &node_ids {
                    let start_byte = self.nodes[*id].origin.start.byte_offset;
                    self.emit_pending_annotations(start_byte, indent + 2)?;
                    self.indent(indent + 2)?;
                    self.fmt(*id, indent + 2)?;
                    self.w.write_all(b"\n")?;
                }
                // Flush any annotations between the last statement and the closing `}`.
                self.emit_pending_annotations(origin.end.byte_offset + 1, indent + 2)?;
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
                for (i, decl) in decls.iter().enumerate() {
                    let start_byte = self.nodes[*decl].origin.start.byte_offset;
                    self.emit_pending_annotations(start_byte, indent)?;
                    self.fmt(*decl, indent)?;
                    // Separate top-level declarations with a blank line so the output
                    // matches conventional C/D style.
                    if i != decls.len() - 1 {
                        self.w.write_all(b"\n")?;
                    }
                }
                // Flush any trailing annotations that appear after the last declaration.
                self.emit_pending_annotations(u32::MAX, indent)?;
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
            NodeKind::PostfixArrayAccess(primary, args) => {
                self.fmt(primary, indent)?;
                self.w.write_all(b"[")?;
                self.fmt(args, indent)?;
                self.w.write_all(b"]")?;
            }
            NodeKind::TernaryExpr(lhs, mhs, rhs) => {
                self.fmt(lhs, indent)?;
                self.w.write_all(b" ? ")?;
                self.fmt(mhs, indent)?;
                self.w.write_all(b" : ")?;
                self.fmt(rhs, indent)?;
            }
            NodeKind::FieldAccess(node_id, dot_or_arrow, ident) => {
                self.fmt(node_id, indent)?;

                let s = lex::str_from_source(self.input, dot_or_arrow.origin);
                self.w.write_all(s.as_bytes())?;

                let s = lex::str_from_source(self.input, ident.origin);
                self.w.write_all(s.as_bytes())?;
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
            NodeKind::Declaration(decl_specifiers, init_declarator_list) => {
                self.fmt(decl_specifiers, indent)?;
                if let Some(init_decl_list) = init_declarator_list {
                    self.w.write_all(b" ")?;
                    self.fmt(init_decl_list, indent)?;
                }
                self.w.write_all(b";\n")?;
            }
            NodeKind::DeclarationSpecifiers(node_ids) => {
                for (i, id) in node_ids.iter().enumerate() {
                    if i > 0 {
                        self.w.write_all(b" ")?;
                    }
                    self.fmt(*id, indent)?;
                }
            }
            NodeKind::DirectDeclarator(base, suffix) => {
                // Parenthesised declarators (e.g. function-pointer `(*fp)`) require
                // wrapping the inner declarator in parens at this level.
                let needs_parens = matches!(self.nodes[base].kind, NodeKind::Declarator(..));
                if needs_parens {
                    self.w.write_all(b"(")?;
                }
                self.fmt(base, indent)?;
                if needs_parens {
                    self.w.write_all(b")")?;
                }
                if let Some(suffix_id) = suffix {
                    self.fmt(suffix_id, indent)?;
                }
            }
            NodeKind::Declarator(ptr, direct_declarator) => {
                if let Some(ptr_id) = ptr {
                    self.fmt(ptr_id, indent)?;
                    // A qualifier keyword (e.g. `const`) at the end of the pointer chain
                    // needs a space before the declarator name.
                    if Self::pointer_ends_with_qualifier(self.nodes, ptr_id) {
                        self.w.write_all(b" ")?;
                    }
                }
                self.fmt(direct_declarator, indent)?;
            }
            NodeKind::InitDeclarators(node_ids) => {
                for (i, id) in node_ids.iter().enumerate() {
                    if i > 0 {
                        self.w.write_all(b", ")?;
                    }
                    self.fmt(*id, indent)?;
                }
            }
            NodeKind::TypeQualifier(_)
            | NodeKind::DStorageClassSpecifier(_)
            | NodeKind::StorageClassSpecifier(_)
            | NodeKind::TypeSpecifier(_) => {
                let s = lex::str_from_source(self.input, origin);
                self.w.write_all(s.as_bytes())?;
            }
            NodeKind::EnumDeclaration(name_tok, enumerator_list) => {
                self.w.write_all(b"enum")?;
                if let Some(name) = name_tok {
                    let s = lex::str_from_source(self.input, name.origin);
                    write!(self.w, " {}", s)?;
                }
                if let Some(enumerators_id) = enumerator_list {
                    self.w.write_all(b" {\n")?;
                    // `EnumeratorsDeclaration` adds indentation and newlines for each item.
                    self.fmt(enumerators_id, indent + 2)?;
                    self.indent(indent)?;
                    self.w.write_all(b"}")?;
                }
            }
            NodeKind::EnumeratorDeclaration(identifier, expr) => {
                self.w.write_all(identifier.as_bytes())?;
                if let Some(expr_id) = expr {
                    self.w.write_all(b" = ")?;
                    self.fmt(expr_id, indent)?;
                }
            }
            NodeKind::EnumeratorsDeclaration(node_ids) => {
                for (i, id) in node_ids.iter().enumerate() {
                    self.indent(indent)?;
                    self.fmt(*id, indent)?;
                    // Trailing comma only between items, not after the last one.
                    if i != node_ids.len() - 1 {
                        self.w.write_all(b",")?;
                    }
                    self.w.write_all(b"\n")?;
                }
            }
            NodeKind::UnionDeclaration(name_tok, decl_list) => {
                self.w.write_all(b"union")?;
                if let Some(name) = name_tok {
                    let s = lex::str_from_source(self.input, name.origin);
                    write!(self.w, " {}", s)?;
                }
                if let Some(fields_id) = decl_list {
                    self.w.write_all(b" {\n")?;
                    self.fmt(fields_id, indent + 2)?;
                    self.indent(indent)?;
                    self.w.write_all(b"}")?;
                }
            }
            NodeKind::StructDeclaration(name_tok, decl_list) => {
                self.w.write_all(b"struct")?;
                if let Some(name) = name_tok {
                    let s = lex::str_from_source(self.input, name.origin);
                    write!(self.w, " {}", s)?;
                }
                if let Some(fields_id) = decl_list {
                    self.w.write_all(b" {\n")?;
                    self.fmt(fields_id, indent + 2)?;
                    self.indent(indent)?;
                    self.w.write_all(b"}")?;
                }
            }
            NodeKind::StructFieldsDeclaration(node_ids) => {
                for id in &node_ids {
                    self.indent(indent)?;
                    self.fmt(*id, indent)?;
                    self.w.write_all(b"\n")?;
                }
            }
            NodeKind::StructFieldDeclarator(declarator, const_expr) => {
                self.fmt(declarator, indent)?;
                if let Some(expr_id) = const_expr {
                    // Bit-field width after a colon.
                    self.w.write_all(b" : ")?;
                    self.fmt(expr_id, indent)?;
                }
            }
            NodeKind::StructFieldDeclaration(specifier_qualifier_list, declarator_list) => {
                self.fmt(specifier_qualifier_list, indent)?;
                if let Some(decl_list_id) = declarator_list {
                    self.w.write_all(b" ")?;
                    self.fmt(decl_list_id, indent)?;
                }
                self.w.write_all(b";")?;
            }
            NodeKind::StructFieldDeclaratorList(node_ids) => {
                for (i, id) in node_ids.iter().enumerate() {
                    if i > 0 {
                        self.w.write_all(b", ")?;
                    }
                    self.fmt(*id, indent)?;
                }
            }
            NodeKind::SpecifierQualifierList(node_ids) => {
                for (i, node_id) in node_ids.iter().enumerate() {
                    self.fmt(*node_id, indent)?;
                    if i != node_ids.len() - 1 {
                        self.w.write_all(b" ")?;
                    }
                }
            }
            NodeKind::Xlate(type_name, expr) => {
                self.w.write_all(b"xlate <")?;
                self.fmt(type_name, indent)?;
                self.w.write_all(b">(")?;
                self.fmt(expr, indent)?;
                self.w.write_all(b")")?;
            }
            NodeKind::DirectAbstractDeclarator(node_id) => {
                self.w.write_all(b"(")?;
                self.fmt(node_id, indent)?;
                self.w.write_all(b")")?;
            }
            NodeKind::DirectAbstractArray(base, suffix) => {
                if let Some(base_id) = base {
                    self.fmt(base_id, indent)?;
                }
                self.fmt(suffix, indent)?;
            }
            NodeKind::DirectAbstractFunction(base, suffix) => {
                if let Some(base_id) = base {
                    self.fmt(base_id, indent)?;
                }
                self.fmt(suffix, indent)?;
            }
            NodeKind::AbstractDeclarator(ptr, abstract_decl) => {
                if let Some(ptr_id) = ptr {
                    self.fmt(ptr_id, indent)?;
                    if let Some(decl_id) = abstract_decl {
                        if Self::pointer_ends_with_qualifier(self.nodes, ptr_id) {
                            self.w.write_all(b" ")?;
                        }
                        self.fmt(decl_id, indent)?;
                    }
                } else if let Some(decl_id) = abstract_decl {
                    self.fmt(decl_id, indent)?;
                }
            }
            NodeKind::Pointer(type_qualifiers, ptr) => {
                self.w.write_all(b"*")?;
                for qual_id in &type_qualifiers {
                    self.w.write_all(b" ")?;
                    self.fmt(*qual_id, indent)?;
                }
                if let Some(ptr_id) = ptr {
                    self.fmt(ptr_id, indent)?;
                }
            }
            NodeKind::Array(params) => {
                self.w.write_all(b"[")?;
                if let Some(params_id) = params {
                    self.fmt(params_id, indent)?;
                }
                self.w.write_all(b"]")?;
            }
            NodeKind::Parameters(node_ids) => {
                for (i, id) in node_ids.iter().enumerate() {
                    if i > 0 {
                        self.w.write_all(b", ")?;
                    }
                    self.fmt(*id, indent)?;
                }
            }
            NodeKind::ParameterDeclarationSpecifiers(node_ids) => {
                for (i, id) in node_ids.iter().enumerate() {
                    if i > 0 {
                        self.w.write_all(b" ")?;
                    }
                    self.fmt(*id, indent)?;
                }
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
            NodeKind::ArgumentsDeclaration(args) => {
                self.w.write_all(b"(")?;
                if let Some(args_id) = args {
                    self.fmt(args_id, indent)?;
                }
                self.w.write_all(b")")?;
            }
            NodeKind::InlineDefinition(decl_specifiers, declarator, expr) => {
                self.w.write_all(b"inline ")?;
                self.fmt(decl_specifiers, indent)?;
                self.w.write_all(b" ")?;
                self.fmt(declarator, indent)?;
                self.w.write_all(b" = ")?;
                self.fmt(expr, indent)?;
                self.w.write_all(b";\n")?;
            }
            NodeKind::ArgumentsExpr(node_ids) => {
                for (i, node_id) in node_ids.iter().enumerate() {
                    self.fmt(*node_id, indent)?;
                    if i != node_ids.len() - 1 {
                        self.w.write_all(b", ")?;
                    }
                }
            }
            NodeKind::ParameterTypeList { params, ellipsis } => {
                if let Some(params_id) = params {
                    self.fmt(params_id, indent)?;
                    if ellipsis.is_some() {
                        self.w.write_all(b", ")?;
                    }
                }
                if let Some(ellipsis_id) = ellipsis {
                    self.fmt(ellipsis_id, indent)?;
                }
            }
            NodeKind::ParameterDeclaration {
                param_decl_specifiers,
                declarator,
            } => {
                self.fmt(param_decl_specifiers, indent)?;
                if let Some(decl_id) = declarator {
                    self.w.write_all(b" ")?;
                    self.fmt(decl_id, indent)?;
                }
            }
            NodeKind::TranslatorDefinition {
                from_type,
                to_type,
                ident,
                members,
            } => {
                self.w.write_all(b"translator ")?;
                self.fmt(from_type, indent)?;
                self.w.write_all(b" < ")?;
                self.fmt(to_type, indent)?;
                write!(self.w, " {} >", ident)?;
                self.w.write_all(b" {\n")?;
                if let Some(members_id) = members {
                    self.fmt(members_id, indent + 2)?;
                }
                self.indent(indent)?;
                self.w.write_all(b"};\n")?;
            }
            NodeKind::TranslatorMembers(ids) => {
                for id in &ids {
                    self.indent(indent)?;
                    self.fmt(*id, indent)?;
                    self.w.write_all(b"\n")?;
                }
            }
            NodeKind::TranslatorMember { ident, expr } => {
                write!(self.w, "{} = ", ident)?;
                self.fmt(expr, indent)?;
                self.w.write_all(b";")?;
            }
            NodeKind::ProviderDefinition { name, probes } => {
                write!(self.w, "provider {} {{\n", name)?;
                if let Some(probes_id) = probes {
                    self.fmt(probes_id, indent + 2)?;
                }
                self.indent(indent)?;
                self.w.write_all(b"};\n")?;
            }
            NodeKind::ProviderProbes(ids) => {
                for id in &ids {
                    self.indent(indent)?;
                    self.fmt(*id, indent)?;
                    self.w.write_all(b"\n")?;
                }
            }
            NodeKind::ProviderProbe {
                name,
                args,
                return_args,
            } => {
                write!(self.w, "probe {}", name)?;
                self.fmt(args, indent)?;
                if let Some(ret) = return_args {
                    self.w.write_all(b" : ")?;
                    self.fmt(ret, indent)?;
                }
                self.w.write_all(b";")?;
            }
        }
        Ok(())
    }
}

pub fn format<W: Write>(
    w: &mut W,
    node_id: NodeId,
    nodes: &[Node],
    comments: &[lex::Comment],
    directives: &[lex::ControlDirective],
    input: &str,
) -> std::io::Result<()> {
    Formatter {
        w,
        nodes,
        comments,
        comment_idx: 0,
        directives,
        directive_idx: 0,
        input,
    }
    .fmt(node_id, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ast::Parser, lex::Lexer};

    const FILE_ID: u32 = 1;

    fn fmt(input: &str) -> String {
        let lexer = Lexer::new(FILE_ID, input);
        let mut parser = Parser::new(lexer);
        let root_id = parser.parse().unwrap();
        let mut out = Vec::new();
        format(
            &mut out,
            root_id,
            &parser.nodes,
            &parser.lexer.comments,
            &parser.lexer.control_directives,
            input,
        )
        .unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn test_probe_no_pred_no_body() {
        let input = "syscall::open:entry  {  }";
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
        let input = "syscall::open:entry  /  pid  ==  42  /  {  }";
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
        let input = "syscall::open:entry  {  x  =  1  ;  }";
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
        let input = "syscall::open:entry  {  x  =  1  ;  y  =  2  ;  }";
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
        let input = "syscall::open:entry  {  if  (  x  ==  1  )  {  y  =  2  ;  }  }";
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
        let input = "BEGIN  ,  END  {  }";
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
        let input = "BEGIN  {  a  =  1  ,  2  ;  }";
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
        let input = "BEGIN  {  print  (  )  ;  }";
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
        let input = "BEGIN  {  print  (  a  )  ;  }";
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
        let input = "BEGIN  ,  END  {  a  =  1  ,  2  ;  print  (  a  )  ;  }";
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
        let input = "BEGIN  {  x  =  sizeof  (  int  )  ;  }";
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
        let input = "BEGIN  {  x  =  sizeof  (  const  int  )  ;  }";
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
        // `sizeof y` (no parens) produces `Sizeof(Identifier, false)`. The formatter preserves
        // the no-paren form and the single space between the keyword and operand.
        let input = "BEGIN  {  x  =  sizeof   y  ;  }";
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
        let input = "syscall::open:entry  {  x  =  1  ;  }  syscall::close:entry  {  x  =  2  ;  }";
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
    fn test_unary_minus() {
        let input = "BEGIN  {  x  =  -  y  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = -y;
}
"
        );
    }

    #[test]
    fn test_unary_logical_not() {
        let input = "BEGIN  {  x  =  !  y  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = !y;
}
"
        );
    }

    #[test]
    fn test_unary_bitwise_not() {
        let input = "BEGIN  {  x  =  ~  y  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = ~y;
}
"
        );
    }

    #[test]
    fn test_unary_deref() {
        let input = "BEGIN  {  x  =  *  y  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = *y;
}
"
        );
    }

    #[test]
    fn test_unary_address_of() {
        let input = "BEGIN  {  x  =  &  y  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = &y;
}
"
        );
    }

    #[test]
    fn test_unary_prefix_increment() {
        let input = "BEGIN  {  ++  x  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  ++x;
}
"
        );
    }

    #[test]
    fn test_unary_prefix_decrement() {
        let input = "BEGIN  {  --  x  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  --x;
}
"
        );
    }

    #[test]
    fn test_unary_paren_expr() {
        // Parenthesised expressions are stored as `Unary(LeftParen, inner)` and require the
        // closing `)` to be emitted explicitly, unlike all other prefix operators.
        let input = "BEGIN  {  x  =  (  y  )  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = (y);
}
"
        );
    }

    #[test]
    fn test_postfix_increment() {
        let input = "BEGIN  {  x  ++  ;  }";
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
        let input = "BEGIN  {  x  --  ;  }";
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
        let input = "BEGIN  {  x  =  a  ?  b  :  c  ;  }";
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
        let input = "BEGIN  {  x  =  stringof   y  ;  }";
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
        let input = "BEGIN  {  x  =  stringof  (  y  )  ;  }";
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
    fn test_field_access_dot() {
        let input = "BEGIN  {  x  =  a  .  b  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = a.b;
}
"
        );
    }

    #[test]
    fn test_field_access_arrow() {
        let input = "BEGIN  {  x  =  a  ->  b  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = a->b;
}
"
        );
    }

    #[test]
    fn test_field_access_chained() {
        // Each access level is a separate `FieldAccess` node wrapping the previous one.
        let input = "BEGIN  {  x  =  a  .  b  .  c  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = a.b.c;
}
"
        );
    }

    #[test]
    fn test_function_call_multiple_args() {
        // Two or more arguments are stored as `ArgumentsExpr`; single arguments are not.
        let input = "BEGIN  {  print  (  a  ,  b  ,  c  )  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  print(a, b, c);
}
"
        );
    }

    #[test]
    fn test_array_access() {
        let input = "BEGIN  {  x  =  a  [  1  ]  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = a[1];
}
"
        );
    }

    #[test]
    fn test_array_access_nested() {
        // Each `[]` level is a separate `PostfixArrayAccess` node; both must be formatted.
        let input = "BEGIN  {  x  =  a  [  i  ]  [  j  ]  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = a[i][j];
}
"
        );
    }

    #[test]
    fn test_offsetof() {
        let input = "BEGIN  {  n  =  offsetof  (  int  ,  x  )  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  n = offsetof(int, x);
}
"
        );
    }

    #[test]
    fn test_declaration_simple() {
        let input = "int x;";
        assert_eq!(fmt(input), "int x;\n");
    }

    #[test]
    fn test_declaration_const_qualified() {
        let input = "const int x;";
        assert_eq!(fmt(input), "const int x;\n");
    }

    #[test]
    fn test_declaration_multiple_declarators() {
        let input = "int x, y;";
        assert_eq!(fmt(input), "int x, y;\n");
    }

    #[test]
    fn test_declaration_pointer() {
        let input = "int *x;";
        assert_eq!(fmt(input), "int *x;\n");
    }

    #[test]
    fn test_declaration_pointer_const() {
        // The `* const` qualifier ends the pointer chain with a keyword, so a space is
        // inserted between the qualifier and the declarator name.
        let input = "int * const x;";
        assert_eq!(fmt(input), "int * const x;\n");
    }

    #[test]
    fn test_declaration_double_pointer() {
        // A bare double pointer has no qualifiers, so no space is added.
        let input = "int **x;";
        assert_eq!(fmt(input), "int **x;\n");
    }

    #[test]
    fn test_sizeof_pointer_type() {
        // Abstract declarator with a plain pointer — no qualifier, so no extra space.
        let input = "BEGIN { n = sizeof(int *); }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  n = sizeof(int *);
}
"
        );
    }

    #[test]
    fn test_sizeof_const_pointer_type() {
        // Abstract declarator with a qualified pointer — space before the next component.
        let input = "BEGIN { n = sizeof(int * const); }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  n = sizeof(int * const);
}
"
        );
    }

    #[test]
    fn test_inline_definition() {
        let input = "inline int x = 42;";
        assert_eq!(fmt(input), "inline int x = 42;\n");
    }

    #[test]
    fn test_xlate_expr() {
        let input = "BEGIN { x = xlate <int>(ptr); }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = xlate <int>(ptr);
}
"
        );
    }

    #[test]
    fn test_struct_declaration() {
        let input = "struct Foo { int x; };";
        assert_eq!(
            fmt(input),
            "struct Foo {
  int x;
};
"
        );
    }

    #[test]
    fn test_struct_declaration_multiple_fields() {
        let input = "struct Foo { int x; int y; };";
        assert_eq!(
            fmt(input),
            "struct Foo {
  int x;
  int y;
};
"
        );
    }

    #[test]
    fn test_struct_forward_declaration() {
        // A struct with no body is a forward declaration; no braces are emitted.
        let input = "struct Foo;";
        assert_eq!(fmt(input), "struct Foo;\n");
    }

    #[test]
    fn test_union_declaration() {
        let input = "union Bar { int i; char c; };";
        assert_eq!(
            fmt(input),
            "union Bar {
  int i;
  char c;
};
"
        );
    }

    #[test]
    fn test_enum_declaration() {
        let input = "enum Color { RED, GREEN, BLUE };";
        assert_eq!(
            fmt(input),
            "enum Color {
  RED,
  GREEN,
  BLUE
};
"
        );
    }

    #[test]
    fn test_enum_declaration_with_values() {
        let input = "enum Color { RED = 0, GREEN = 1, BLUE = 2 };";
        assert_eq!(
            fmt(input),
            "enum Color {
  RED = 0,
  GREEN = 1,
  BLUE = 2
};
"
        );
    }

    #[test]
    fn test_enum_forward_reference() {
        // An enum used by name only (forward reference, no body).
        let input = "enum Color c;";
        assert_eq!(fmt(input), "enum Color c;\n");
    }

    #[test]
    fn test_struct_pointer_field() {
        // Struct with a pointer field exercises `Declarator` with a non-null pointer.
        let input = "struct Node { int *value; };";
        assert_eq!(
            fmt(input),
            "struct Node {
  int *value;
};
"
        );
    }

    #[test]
    fn test_single_line_comment_top_level() {
        let input = "// A comment\nint  x  ;";
        assert_eq!(fmt(input), "// A comment\nint x;\n");
    }

    #[test]
    fn test_multi_line_comment_top_level() {
        let input = "/* A comment */\nint  x  ;";
        assert_eq!(fmt(input), "/* A comment */\n\nint x;\n");
    }

    #[test]
    fn test_single_line_comment_in_probe_body() {
        let input = "BEGIN  {  // A comment\n  x  =  1  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  // A comment
  x = 1;
}
"
        );
    }

    #[test]
    fn test_pragma_option_before_declaration() {
        // A pragma directive appearing before a top-level declaration must be emitted
        // before that declaration, preserving its source order.
        let input = "#pragma D option quiet\nint  x  ;";
        assert_eq!(fmt(input), "#pragma D option quiet\nint x;\n");
    }

    #[test]
    fn test_pragma_option_key_value_before_declaration() {
        // A pragma with a `key=value` option must be preserved verbatim.
        let input = "#pragma D option bufsize=4m\nint  x  ;";
        assert_eq!(fmt(input), "#pragma D option bufsize=4m\nint x;\n");
    }

    #[test]
    fn test_pragma_depends_on_before_declaration() {
        // A `depends_on` pragma must be emitted before the following declaration.
        let input = "#pragma D depends_on module isa\nint  x  ;";
        assert_eq!(fmt(input), "#pragma D depends_on module isa\nint x;\n");
    }

    #[test]
    fn test_pragma_blank_line_before_declaration() {
        // A blank line between a pragma and the following declaration must be preserved
        // so that the formatter does not collapse intentional vertical whitespace.
        let input = "#pragma D option quiet\n\nint  x  ;";
        assert_eq!(fmt(input), "#pragma D option quiet\n\nint x;\n");
    }

    #[test]
    fn test_pragma_no_blank_line_before_declaration_unchanged() {
        // When no blank line is present in the source, none should be added.
        let input = "#pragma D option quiet\nint  x  ;";
        assert_eq!(fmt(input), "#pragma D option quiet\nint x;\n");
    }

    #[test]
    fn test_pragma_interleaved_with_comment() {
        // When a comment and a pragma both precede a declaration, they must be
        // emitted in the original source order.
        let input = "// A comment\n#pragma D option quiet\nint  x  ;";
        assert_eq!(fmt(input), "// A comment\n#pragma D option quiet\nint x;\n");
    }

    #[test]
    fn test_all_in_one_idempotent() {
        // Parse and format the comprehensive example file (pass 1), then parse and
        // format the result again (pass 2).  The two passes must produce identical
        // output: the formatter must be stable under repeated application.
        let input = include_str!("../examples/all-in-one.d");
        let pass1 = fmt(input);
        let pass2 = fmt(&pass1);
        assert_eq!(
            pass1, pass2,
            "formatter output changed on second pass:\n{pass1}"
        );
    }
}
