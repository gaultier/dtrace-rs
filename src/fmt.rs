use std::io::Write;

use crate::{
    ast::{Node, NodeId, NodeKind},
    lex::{
        self, Attribute, Comment, CommentKind, ControlDirective, ControlDirectiveKind, TokenKind,
    },
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
    /// All `__attribute__((...))` annotations from the lexer, sorted by source position.
    attributes: &'a [Attribute],
    /// Index of the next attribute not yet emitted.
    attribute_idx: usize,
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

    /// Emits one `__attribute__((...))` annotation at the current `attribute_idx`, advancing the index.
    fn emit_one_attribute(&mut self, indent: usize) -> std::io::Result<()> {
        let attr = &self.attributes[self.attribute_idx];
        self.indent(indent)?;
        let text = lex::str_from_source(self.input, attr.origin);
        self.w.write_all(text.as_bytes())?;
        self.w.write_all(b"\n")?;
        self.attribute_idx += 1;
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

    /// Emits every not-yet-emitted comment, directive, or `__attribute__` annotation whose
    /// start byte is strictly less than `before_byte`, in source order.  All three queues
    /// are advanced together so the interleaved original order is preserved.
    ///
    /// After all annotations are emitted, a blank line is inserted when the source
    /// contains one between the last annotation and the following node — except when
    /// the last annotation was a multi-line comment, which already appends a blank line
    /// unconditionally.
    fn emit_pending_annotations(&mut self, before_byte: u32, indent: usize) -> std::io::Result<()> {
        // Exclusive byte offset of the last annotation emitted in this call, if any.
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
            let next_attribute = self
                .attributes
                .get(self.attribute_idx)
                .map(|a| a.origin.start.byte_offset)
                .unwrap_or(u32::MAX);
            let next = next_comment.min(next_directive).min(next_attribute);
            if next >= before_byte {
                break;
            }
            // Preserve a blank line between two consecutive annotations
            // when the source had one — e.g. a shebang followed by a
            // blank line and then a pragma.
            if !last_was_multiline_comment && let Some(prev_end) = last_annotation_end {
                let gap_start = prev_end as usize;
                let gap_end = (next as usize).min(self.input.len());
                if gap_start < gap_end && self.input[gap_start..gap_end].contains("\n\n") {
                    self.w.write_all(b"\n")?;
                }
            }

            if next_comment <= next_directive && next_comment <= next_attribute {
                last_was_multiline_comment =
                    self.comments[self.comment_idx].kind == CommentKind::MultiLine;
                last_annotation_end = Some(self.comments[self.comment_idx].origin.end.byte_offset);
                self.emit_one_comment(indent)?;
            } else if next_directive <= next_attribute {
                last_was_multiline_comment = false;
                last_annotation_end =
                    Some(self.directives[self.directive_idx].origin.end.byte_offset);
                self.emit_one_directive(indent)?;
            } else {
                last_was_multiline_comment = false;
                last_annotation_end =
                    Some(self.attributes[self.attribute_idx].origin.end.byte_offset);
                self.emit_one_attribute(indent)?;
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

    /// Drains pending `/* */` comments whose start byte is strictly before
    /// `before_byte`, emitting each followed by a single space. Used at every
    /// `fmt` entry to surface comments that appear *between* sibling tokens
    /// of the surrounding construct (e.g. `int /* x */ y;`). Single-line
    /// `//` comments are deferred to `emit_pending_annotations` because they
    /// require a newline that would break inline contexts.
    fn drain_inline_comments_before(&mut self, before_byte: u32) -> std::io::Result<()> {
        while let Some(c) = self.comments.get(self.comment_idx) {
            if c.origin.start.byte_offset >= before_byte || c.kind != CommentKind::MultiLine {
                break;
            }
            let text = lex::str_from_source(self.input, c.origin);
            write!(self.w, "{} ", text)?;
            self.comment_idx += 1;
        }
        Ok(())
    }

    /// Drains any pending comments whose start byte is on the same source
    /// line as `after_byte` (i.e. before the next `\n`) AND strictly before
    /// `max_byte`. Each is emitted preceded by a single space so trailing
    /// annotations such as `stmt; // explanation` stay attached to the
    /// statement they belong to. The trailing newline is left to the
    /// caller. `max_byte` is used to prevent a probe specifier's drain
    /// from claiming a comment that actually sits inside the following
    /// `{ ... }` body on the same source line.
    fn drain_trailing_line_comments(
        &mut self,
        after_byte: u32,
        max_byte: u32,
    ) -> std::io::Result<()> {
        let start = (after_byte as usize).min(self.input.len());
        let line_end = self.input[start..]
            .find('\n')
            .map(|i| (start + i) as u32)
            .unwrap_or(self.input.len() as u32);
        let limit = line_end.min(max_byte);
        while let Some(c) = self.comments.get(self.comment_idx) {
            if c.origin.start.byte_offset >= limit {
                break;
            }
            let text = lex::str_from_source(self.input, c.origin);
            write!(self.w, " {}", text)?;
            self.comment_idx += 1;
        }
        Ok(())
    }

    /// Variant of `drain_inline_comments_before` for the position just before
    /// a closing token (`]`, `)`, etc.): the preceding child has already been
    /// emitted without a trailing space, so each comment is prefixed by a
    /// leading space instead of suffixed by one.
    fn drain_inline_comments_before_close(&mut self, before_byte: u32) -> std::io::Result<()> {
        while let Some(c) = self.comments.get(self.comment_idx) {
            if c.origin.start.byte_offset >= before_byte || c.kind != CommentKind::MultiLine {
                break;
            }
            let text = lex::str_from_source(self.input, c.origin);
            write!(self.w, " {}", text)?;
            self.comment_idx += 1;
        }
        Ok(())
    }

    /// Returns `true` if the innermost `Pointer` chain ends with a type-qualifier keyword
    /// rather than a bare `*`. Callers use this to decide whether a space is needed between
    /// a pointer and the following declarator name (e.g. `* const x` vs. `*x`).
    fn pointer_ends_with_qualifier(nodes: &[Node], ptr_id: NodeId) -> bool {
        match &nodes[ptr_id].kind {
            NodeKind::Pointer { qualifiers, inner } => {
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
        let (children, block_end) = match self.nodes[node_id].kind.clone() {
            NodeKind::Block(children) => (children, self.nodes[node_id].origin.end.byte_offset),
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
        self.w.write_all(b"{")?;
        // Same-line trailing comment after `{` — `if (foo) { // remark`.
        let block_start = self.nodes[node_id].origin.start.byte_offset;
        self.drain_trailing_line_comments(block_start + 1, u32::MAX)?;
        self.w.write_all(b"\n")?;
        for child_id in children {
            let start_byte = self.nodes[child_id].origin.start.byte_offset;
            self.emit_pending_annotations(start_byte, indent + 2)?;
            self.indent(indent + 2)?;
            self.fmt(child_id, indent + 2)?;
            // Same-line trailing comments stay with the statement.
            self.drain_trailing_line_comments(
                self.nodes[child_id].origin.end.byte_offset,
                u32::MAX,
            )?;
            self.w.write_all(b"\n")?;
        }
        // Flush comments/directives between the last statement and `}` so
        // patterns like `if (cond) { /* x */ }` don't lose the annotation
        // through the brace.
        self.emit_pending_annotations(block_end + 1, indent + 2)?;
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

        // Emit any pending `/* */` comments that appear *before* this node's
        // start so inter-token annotations like `int /* x */ y;` land in the
        // right place. `TranslationUnit` and `Block` drive their own
        // newline-style emission via `emit_pending_annotations`, so skip the
        // inline drain there to avoid double-emitting (and to keep top-level
        // multi-line comments on their own line).
        if !matches!(kind, NodeKind::TranslationUnit(_) | NodeKind::Block(_)) {
            self.drain_inline_comments_before(origin.start.byte_offset)?;
        }

        match kind {
            NodeKind::Unknown | NodeKind::Character(_) | NodeKind::ParamEllipsis => {
                let src = lex::str_from_source(self.input, origin);
                self.w.write_all(src.as_bytes())?;
            }
            NodeKind::Block(node_ids) => {
                self.w.write_all(b"{")?;
                // Same-line trailing comment after `{` (e.g. `BEGIN { // x`).
                self.drain_trailing_line_comments(origin.start.byte_offset + 1, u32::MAX)?;
                self.w.write_all(b"\n")?;
                let mut prev_end: Option<u32> = None;
                for id in &node_ids {
                    let start_byte = self.nodes[*id].origin.start.byte_offset;
                    // Preserve a blank line between two consecutive statements
                    // when the source had one. Two-or-more newlines in the
                    // gap means at least one empty line was there.
                    if let Some(prev) = prev_end {
                        let gap_start = prev as usize;
                        let gap_end = (start_byte as usize).min(self.input.len());
                        if gap_start < gap_end
                            && self.input[gap_start..gap_end]
                                .bytes()
                                .filter(|&b| b == b'\n')
                                .count()
                                >= 2
                        {
                            self.w.write_all(b"\n")?;
                        }
                    }
                    self.emit_pending_annotations(start_byte, indent + 2)?;
                    self.indent(indent + 2)?;
                    self.fmt(*id, indent + 2)?;
                    // Keep any comment that sits on the same source line as
                    // this statement attached to it — `stmt; // remark`.
                    self.drain_trailing_line_comments(
                        self.nodes[*id].origin.end.byte_offset,
                        u32::MAX,
                    )?;
                    self.w.write_all(b"\n")?;
                    prev_end = Some(self.nodes[*id].origin.end.byte_offset);
                }
                // Flush any annotations between the last statement and the closing `}`.
                self.emit_pending_annotations(origin.end.byte_offset + 1, indent + 2)?;
                self.indent(indent)?;
                self.w.write_all(b"}")?;
            }
            NodeKind::ProbeDefinition {
                probe_specifiers: probe,
                predicate: pred,
                action: actions,
            } => {
                self.fmt(probe, indent)?;
                // Same-line trailing comments stay with the probe specifier
                // line — `pid$target::foo:entry // remark`. Cap the drain
                // at the start of the next significant node (predicate or
                // body) so a comment that's actually *inside* `{ … }` on
                // the same source line isn't pulled out.
                let probe_trailing_max = pred
                    .or(actions)
                    .map(|n| self.nodes[n].origin.start.byte_offset)
                    .unwrap_or(u32::MAX);
                self.drain_trailing_line_comments(
                    self.nodes[probe].origin.end.byte_offset,
                    probe_trailing_max,
                )?;
                self.w.write_all(b"\n")?;

                if let Some(pred_id) = pred {
                    // Flush comments/directives sitting between the probe
                    // specifier and the `/.../` predicate so they land on
                    // their own lines (rather than getting picked up by an
                    // inner expression's `//`-after-op drain).
                    let pred_start = self.nodes[pred_id].origin.start.byte_offset;
                    self.emit_pending_annotations(pred_start, indent)?;
                    self.w.write_all(b"/ ")?;
                    self.fmt(pred_id, indent)?;
                    self.w.write_all(b" /")?;
                    // Same-line trailing comments after `/ pred /` stay with
                    // the predicate line. Cap at the body's start so a
                    // comment that lives inside `{ … }` on the same line
                    // isn't pulled out.
                    let pred_trailing_max = actions
                        .map(|n| self.nodes[n].origin.start.byte_offset)
                        .unwrap_or(u32::MAX);
                    self.drain_trailing_line_comments(
                        self.nodes[pred_id].origin.end.byte_offset,
                        pred_trailing_max,
                    )?;
                    self.w.write_all(b"\n")?;
                }

                if let Some(actions_id) = actions {
                    // Same idea between the probe spec / predicate and the
                    // action body `{ ... }`.
                    let actions_start = self.nodes[actions_id].origin.start.byte_offset;
                    self.emit_pending_annotations(actions_start, indent)?;
                    self.fmt(actions_id, indent)?;
                }
                self.w.write_all(b"\n")?;
            }
            NodeKind::Number { .. }
            | NodeKind::Identifier(_)
            | NodeKind::ProbeSpecifier(_)
            | NodeKind::PrimaryToken(_)
            | NodeKind::Aggregation => {
                let src = lex::str_from_source(self.input, origin);
                self.w.write_all(src.as_bytes())?;
            }
            NodeKind::Assignment { lhs, op: tok, rhs }
            | NodeKind::BinaryOp { lhs, op: tok, rhs } => {
                self.fmt(lhs, indent)?;
                let src = lex::str_from_source(self.input, tok.origin);
                write!(self.w, " {} ", src)?;
                // A `//` comment sitting between the operator and the
                // right-hand operand must stay attached to the operator's
                // line: it extends to end-of-line in the source, so emit it
                // followed by a newline and a continuation indent so the
                // rest of the expression lands on the next line (aligning
                // roughly under the opening token of the enclosing
                // construct, e.g. `if (`).
                let rhs_start = self.nodes[rhs].origin.start.byte_offset;
                while let Some(c) = self.comments.get(self.comment_idx) {
                    if c.origin.start.byte_offset >= rhs_start || c.kind != CommentKind::SingleLine
                    {
                        break;
                    }
                    let text = lex::str_from_source(self.input, c.origin);
                    writeln!(self.w, "{}", text)?;
                    self.indent(indent + 4)?;
                    self.comment_idx += 1;
                }
                self.fmt(rhs, indent)?;
            }
            NodeKind::If {
                cond,
                cond_close_paren_byte,
                then_block,
                else_block,
            } => {
                self.w.write_all(b"if (")?;
                self.fmt(cond, indent)?;
                // Comments that sit inside the parens — `if (cond /* x */)` —
                // must be drained before `)` is emitted. The parser records
                // the `)`'s byte offset for exactly this purpose.
                self.drain_inline_comments_before_close(cond_close_paren_byte)?;
                self.w.write_all(b") ")?;
                // Comments between `)` and `{` — `if (cond) /* x */ {` —
                // are drained before delegating to `fmt_branch`, so the
                // brace is preceded by them.
                let then_start = self.nodes[then_block].origin.start.byte_offset;
                self.drain_inline_comments_before(then_start)?;
                self.fmt_branch(then_block, indent)?;

                if let Some(else_id) = else_block {
                    self.w.write_all(b" else ")?;
                    // Same idea between `else` and the `{` or `if`.
                    let else_start = self.nodes[else_id].origin.start.byte_offset;
                    self.drain_inline_comments_before(else_start)?;
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
            NodeKind::Cast {
                typ: type_name,
                expr: inner,
            } => {
                write!(self.w, "({})", &type_name)?;
                self.fmt(inner, indent)?;
            }
            NodeKind::ExprStmt(inner) => {
                self.fmt(inner, indent)?;
                self.w.write_all(b";")?;
            }
            NodeKind::EmptyStmt => {}
            NodeKind::PostfixArguments {
                callee: primary,
                args,
            } => {
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
            NodeKind::Sizeof {
                expr: node_id,
                parenthesized: with_paren,
            } => {
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
            NodeKind::StringofExpr {
                expr: node_id,
                parenthesized: with_paren,
            } => {
                self.w.write_all(b"stringof")?;
                if !with_paren {
                    self.w.write_all(b" ")?;
                }
                self.fmt(node_id, indent)?;
            }
            NodeKind::PostfixIncDecrement {
                expr: node_id,
                op: token,
            } => {
                self.fmt(node_id, indent)?;
                let s = lex::str_from_source(self.input, token.origin);
                self.w.write_all(s.as_bytes())?;
            }
            NodeKind::PostfixArrayAccess {
                array: primary,
                index: args,
            } => {
                self.fmt(primary, indent)?;
                self.w.write_all(b"[")?;
                self.fmt(args, indent)?;
                self.w.write_all(b"]")?;
            }
            NodeKind::TernaryExpr {
                cond: lhs,
                then_expr: mhs,
                else_expr: rhs,
            } => {
                self.fmt(lhs, indent)?;
                self.w.write_all(b" ? ")?;
                self.fmt(mhs, indent)?;
                self.w.write_all(b" : ")?;
                self.fmt(rhs, indent)?;
            }
            NodeKind::FieldAccess {
                expr: node_id,
                op: dot_or_arrow,
                field: ident,
            } => {
                self.fmt(node_id, indent)?;

                let s = lex::str_from_source(self.input, dot_or_arrow.origin);
                self.w.write_all(s.as_bytes())?;

                let s = lex::str_from_source(self.input, ident.origin);
                self.w.write_all(s.as_bytes())?;
            }
            NodeKind::TypeName {
                specifiers: specifier,
                abstract_declarator: declarator,
            } => {
                self.fmt(specifier, indent)?;
                if let Some(declarator) = declarator {
                    self.w.write_all(b" ")?;
                    self.fmt(declarator, indent)?;
                };
            }
            NodeKind::OffsetOf {
                typ: node_id,
                field: token,
            } => {
                self.w.write_all(b"offsetof(")?;
                self.fmt(node_id, indent)?;
                self.w.write_all(b", ")?;
                let s = lex::str_from_source(self.input, token.origin);
                self.w.write_all(s.as_bytes())?;
                self.w.write_all(b")")?;
            }
            NodeKind::Declaration {
                specifiers: decl_specifiers,
                declarators: init_declarator_list,
            } => {
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
            NodeKind::DirectDeclarator {
                ident: base,
                suffix,
            } => {
                // Parenthesised declarators (e.g. function-pointer `(*fp)`) require
                // wrapping the inner declarator in parens at this level.
                let needs_parens = matches!(self.nodes[base].kind, NodeKind::Declarator { .. });
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
            NodeKind::Declarator {
                pointer: ptr,
                direct: direct_declarator,
            } => {
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
            NodeKind::EnumDeclaration {
                name: name_tok,
                enumerators: enumerator_list,
            } => {
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
            NodeKind::EnumeratorDeclaration {
                name: identifier,
                value: expr,
            } => {
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
            NodeKind::UnionDeclaration {
                name: name_tok,
                fields: decl_list,
            } => {
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
            NodeKind::StructDeclaration {
                name: name_tok,
                fields: decl_list,
            } => {
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
            NodeKind::StructFieldDeclarator {
                declarator,
                bit_field: const_expr,
            } => {
                self.fmt(declarator, indent)?;
                if let Some(expr_id) = const_expr {
                    // Bit-field width after a colon.
                    self.w.write_all(b" : ")?;
                    self.fmt(expr_id, indent)?;
                }
            }
            NodeKind::StructFieldDeclaration {
                specifiers: specifier_qualifier_list,
                declarators: declarator_list,
            } => {
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
            NodeKind::Xlate {
                typ: type_name,
                expr,
            } => {
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
            NodeKind::DirectAbstractArray {
                inner: base,
                size: suffix,
            } => {
                if let Some(base_id) = base {
                    self.fmt(base_id, indent)?;
                }
                self.fmt(suffix, indent)?;
            }
            NodeKind::DirectAbstractFunction {
                inner: base,
                params: suffix,
            } => {
                if let Some(base_id) = base {
                    self.fmt(base_id, indent)?;
                }
                self.fmt(suffix, indent)?;
            }
            NodeKind::AbstractDeclarator {
                pointer: ptr,
                direct: abstract_decl,
            } => {
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
            NodeKind::Pointer {
                qualifiers: type_qualifiers,
                inner: ptr,
            } => {
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
                // Drain any `/* */` comments between the last child and `]`,
                // e.g. `arr[uintptr_t /* data ptr */]`.
                self.drain_inline_comments_before_close(origin.end.byte_offset)?;
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
            NodeKind::Unary {
                op: token,
                expr: node_id,
            } => {
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
            NodeKind::InlineDefinition {
                typ: decl_specifiers,
                declarator,
                expr,
            } => {
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
                writeln!(self.w, "provider {} {{", name)?;
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
    attributes: &[lex::Attribute],
    input: &str,
) -> std::io::Result<()> {
    Formatter {
        w,
        nodes,
        comments,
        comment_idx: 0,
        directives,
        directive_idx: 0,
        attributes,
        attribute_idx: 0,
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
        assert!(lexer.errors.is_empty());
        let mut parser = Parser::new(lexer);
        assert!(parser.lexer.errors.is_empty());
        let root_id = parser.parse().unwrap();
        let mut out = Vec::new();
        format(
            &mut out,
            root_id,
            &parser.nodes,
            &parser.lexer.comments,
            &parser.lexer.control_directives,
            &parser.lexer.attributes,
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
    fn test_compound_shift_assign_operators() {
        // `<<=` and `>>=` are valid `assignment_operator`s in the official
        // `dt_grammar.y`. The parser used to list `<=` / `>=` in their slot
        // — a copy-paste typo — so neither shift-assign parsed.
        let input = "BEGIN { x <<= 3; y >>= 1; }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x <<= 3;
  y >>= 1;
}
"
        );
    }

    #[test]
    fn test_if_else_braceless_bodies() {
        // The official grammar's `statement_or_block` allows either a
        // braced block or a single statement (e.g. `if (c) x = 1;`).
        // The braceless form must parse, and the formatter wraps the
        // single statement in `{ ... }` for consistency.
        let input = "BEGIN {\n  if (1)\n    n = 0;\n  else\n    n = 1;\n}";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  if (1) {
    n = 0;
  } else {
    n = 1;
  }
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
    fn test_cast_to_backtick_scoped_type() {
        // `(D``env_vars_32_t *)x` — dtrace's scoped-type syntax. The lexer
        // emits a single `Identifier` for `D``env_vars_32_t` (backtick is a
        // valid identifier rest character in `InsideClauseAndExpr`); the
        // official `libdtrace` would classify it as `DT_TOK_TNAME` via CTF
        // resolution. We don't have CTF, but the lexeme shape is
        // unambiguous: a backtick-containing identifier in a `(…)` cast
        // position is always a scoped type reference, so the cast
        // lookahead accepts it.
        let input = "BEGIN { p = (D`env_vars_32_t *)q; }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  p = (D`env_vars_32_t *)q;
}
"
        );
    }

    #[test]
    fn test_sizeof_and_cast_with_float_double() {
        // `float` and `double` are valid `type_specifier`s in our parser
        // (see `parse_type_specifier`), but the cast and `sizeof '(' type ')'`
        // lookaheads had forgotten to list them. The official `dt_grammar.y`
        // includes them via `type_specifier: DT_KEY_FLOAT | DT_KEY_DOUBLE`.
        // (Semantic rejection of float operations in dtrace is a later
        // pass; this only confirms the parser accepts the shape.)
        let input = "BEGIN { x = sizeof(float); y = (double)z; }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = sizeof(float);
  y = (double)z;
}
"
        );
    }

    #[test]
    fn test_sizeof_paren_full_expression() {
        // `sizeof (10 * 'c')` parses as `sizeof <unary_expression>` per
        // the official grammar, where the unary is the parenthesised
        // primary `(10 * 'c')`. Output: `sizeof (10 * 'c')` — the `(` is
        // part of the inner primary, not part of the `sizeof '(' type ')'`
        // alternative, hence the space after `sizeof`.
        let input = "BEGIN { trace(sizeof (10 * 'c')); }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  trace(sizeof (10 * 'c'));
}
"
        );
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
    fn test_xlate_expr_followed_by_arrow_field_access() {
        // Regression: `xlate <T>(expr)` is a `postfix_expression` in
        // `dt_grammar.y`, so a subsequent `->field` (or `.field`, `[idx]`,
        // etc.) must be parsed via the postfix loop. The parser used to
        // `return` immediately after constructing the `Xlate` node and
        // never reached the loop.
        let input = "BEGIN { x = xlate <struct vtype2str *>(arg0)->code; }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = xlate <struct vtype2str *>(arg0)->code;
}
"
        );
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
    fn test_cast_to_union_pointer() {
        // Regression: the cast lookahead included `KeywordStruct` but
        // forgot `KeywordUnion`, so `(union u *)expr` never matched the
        // cast production and fell into the parenthesised-primary path.
        let input = "BEGIN { myi = (union u *) p; }";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  myi = (union u *)p;
}
"
        );
    }

    #[test]
    fn test_vararg_function_declaration_only_ellipsis() {
        // `extern f(...);` — vararg with no fixed parameters. Needed both
        // the lexer's `...` arm to fire in `InsideClauseAndExpr` (was
        // restricted to `ProgramOuterScope`), and the parser to route the
        // bare ellipsis through `parse_function_parameters`.
        let input = "extern int varargs1(...);";
        assert_eq!(fmt(input), "extern int varargs1(...);\n");
    }

    #[test]
    fn test_vararg_function_declaration_with_fixed_params() {
        // `extern f(int x, ...);` — also exercises (a) `parse_parameter_list`
        // stopping before a trailing `, ...` so the ellipsis is captured by
        // `parse_parameter_type_list` and (b) the ellipsis-node origin
        // pointing at `...` (not the preceding comma) so the formatter
        // prints `...` rather than `,`.
        let input = "extern int f(int x, ...);";
        assert_eq!(fmt(input), "extern int f(int x, ...);\n");
    }

    #[test]
    fn test_function_pointer_declarator() {
        // The C `direct_declarator: '(' declarator ')'` production is used
        // for function pointers like `int (*fp)();`. The parser's closing
        // `)` arm previously expected `LeftParen` instead of `RightParen`
        // — a copy-paste bug analogous to the earlier `parse_array` one.
        let input = "int (*fp)();";
        assert_eq!(fmt(input), "int (*fp)();\n");
    }

    #[test]
    fn test_function_pointer_returning_pointer() {
        // `int *(*fp)(void)` — pointer to function returning pointer.
        let input = "int *(*fp)(void);";
        assert_eq!(fmt(input), "int *(*fp)(void);\n");
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
        // A `//` comment on the same source line as the opening `{` of a
        // probe body stays attached to that line, not hoisted onto its
        // own line inside the body.
        let input = "BEGIN  {  // A comment\n  x  =  1  ;  }";
        assert_eq!(
            fmt(input),
            "BEGIN
{ // A comment
  x = 1;
}
"
        );
    }

    #[test]
    fn test_pragma_inside_probe_body() {
        // `dt_lex.l`'s `<S0>{RGX_CTL} | <S2>{RGX_CTL}` rule accepts a
        // control directive (`#pragma …`) inside a probe body, not just
        // at top level. Mirrors `test/tst/common/probes/tst.pragmainside.d`
        // in the official dtrace test corpus.
        let input = "tick-10ms\n{\n#pragma D option quiet\n  exit(0);\n}";
        assert_eq!(
            fmt(input),
            "tick-10ms
{
  #pragma D option quiet
  exit(0);
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
    fn test_blank_line_between_shebang_and_pragma_preserved() {
        // Two consecutive top-level annotations (here a shebang and a
        // pragma) separated by a blank line in the source must remain
        // separated by a blank line in the output. `emit_pending_annotations`
        // checks the gap between each annotation pair, not just between
        // the last annotation and the next AST node.
        let input = "#!/usr/sbin/dtrace -s\n\n#pragma D option strsize=16K\nBEGIN { trace(1); }";
        assert_eq!(
            fmt(input),
            "#!/usr/sbin/dtrace -s

#pragma D option strsize=16K
BEGIN
{
  trace(1);
}
"
        );
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
    fn test_attribute_before_declaration() {
        // An `__attribute__((...))` annotation before a declaration must be emitted
        // verbatim in its original source position, before the declaration.
        let input = "__attribute__((nodtrace));\nint  x  ;";
        assert_eq!(fmt(input), "__attribute__((nodtrace));\nint x;\n");
    }

    #[test]
    fn test_attribute_interleaved_with_pragma() {
        // When a pragma and an `__attribute__` both precede a declaration they must
        // be emitted in the original source order.
        let input = "#pragma D option quiet\n__attribute__((nodtrace));\nint  x  ;";
        assert_eq!(
            fmt(input),
            "#pragma D option quiet\n__attribute__((nodtrace));\nint x;\n"
        );
    }

    #[test]
    fn test_cpp_include_preserved() {
        // `#include` reaches the D lexer only when the user has not run `cpp`
        // beforehand. The formatter must pass it through verbatim, just like
        // `#pragma`.
        let input = "#include \"stdio.h\"\nBEGIN { trace(1); }";
        assert_eq!(
            fmt(input),
            "#include \"stdio.h\"
BEGIN
{
  trace(1);
}
"
        );
    }

    #[test]
    fn test_cpp_define_preserved() {
        // A single-line `#define` must be emitted verbatim and the body that
        // references the macro identifier must still format.
        let input = "#define VALUE 5\nBEGIN { x = VALUE; }";
        assert_eq!(
            fmt(input),
            "#define VALUE 5
BEGIN
{
  x = VALUE;
}
"
        );
    }

    #[test]
    fn test_cpp_define_with_line_continuation_preserved() {
        // A `\\` at end of line continues a `#define` onto the next physical
        // line. The directive origin spans both physical lines, so the raw
        // source text — including the backslash and newline — is emitted
        // unchanged.
        let input = "#define\tTST(name)\t\\\n\tprintf(\"foo\\n\", name)\nBEGIN { trace(1); }";
        assert_eq!(
            fmt(input),
            "#define\tTST(name)\t\\\n\tprintf(\"foo\\n\", name)
BEGIN
{
  trace(1);
}
"
        );
    }

    #[test]
    fn test_cpp_undef_preserved() {
        // `#undef` is opaque to the D lexer; pass through verbatim.
        let input = "#undef VALUE\nBEGIN { trace(1); }";
        assert_eq!(
            fmt(input),
            "#undef VALUE
BEGIN
{
  trace(1);
}
"
        );
    }

    #[test]
    fn test_cpp_ifdef_else_endif_preserved() {
        // `#ifdef … #else … #endif` is opaque to the D lexer: each directive
        // is stored individually and emitted in source order. The body
        // between directives still parses (here, two `#define` lines).
        let input = "#ifdef FLAG\n#define VALUE 5\n#else\n#define VALUE 10\n#endif\nBEGIN { trace(VALUE); }";
        assert_eq!(
            fmt(input),
            "#ifdef FLAG
#define VALUE 5
#else
#define VALUE 10
#endif
BEGIN
{
  trace(VALUE);
}
"
        );
    }

    #[test]
    fn test_cpp_if_defined_preserved() {
        // `#if defined(FLAG)` — the `if` keyword is a D statement keyword
        // but at column 1 after `#` it can only be the preprocessor
        // directive, so the lexer routes it to the directive branch.
        let input = "#if defined (FLAG)\n#define VALUE 5\n#endif\nBEGIN { trace(1); }";
        assert_eq!(
            fmt(input),
            "#if defined (FLAG)
#define VALUE 5
#endif
BEGIN
{
  trace(1);
}
"
        );
    }

    #[test]
    fn test_cpp_ifndef_elif_preserved() {
        // `#ifndef` and `#elif` must also pass through. The body between
        // each directive still parses.
        let input = "#ifndef VALUE\n#define VALUE 1\n#elif VALUE == 2\n#define VALUE 3\n#endif\nBEGIN { trace(1); }";
        assert_eq!(
            fmt(input),
            "#ifndef VALUE
#define VALUE 1
#elif VALUE == 2
#define VALUE 3
#endif
BEGIN
{
  trace(1);
}
"
        );
    }

    #[test]
    fn test_cpp_warning_preserved() {
        // `#warning` reaches the D lexer when `cpp` was not run; pass
        // through verbatim rather than treating as a D `#error` directive.
        let input = "#warning deprecated\nBEGIN { trace(1); }";
        assert_eq!(
            fmt(input),
            "#warning deprecated
BEGIN
{
  trace(1);
}
"
        );
    }

    #[test]
    fn test_multi_line_comment_between_close_paren_and_open_brace_of_if() {
        // A comment sitting between `)` and `{` of an `if` must stay there.
        let input = "BEGIN {\n  if (foo) /* bar */ {\n  }\n}";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  if (foo) /* bar */ {
  }
}
"
        );
    }

    #[test]
    fn test_trailing_comment_on_probe_spec_and_predicate_lines() {
        // A `//` comment sitting on the same source line as the probe
        // specifier or the predicate must stay attached to that line in
        // the output, not get pushed onto its own line below.
        let input = "io:::start // Listen to I/O requests.\n\
                     /execname == \"go\"/ // Filter by exec name.\n\
                     {\n  this->p = args[2]->fi_pathname;\n}";
        assert_eq!(
            fmt(input),
            "io:::start // Listen to I/O requests.
/ execname == \"go\" / // Filter by exec name.
{
  this->p = args[2]->fi_pathname;
}
"
        );
    }

    #[test]
    fn test_single_line_comment_between_probe_spec_and_predicate() {
        // A `//` comment placed between the probe specifier and the
        // `/.../` predicate must land on its own line between them, not
        // get hoovered up by an inner `BinaryOp`'s `//`-after-operator
        // drain (which would split `t != 0` into `t != // ...\n 0`).
        let input = "pid$target::runtime.gopark:entry\n// arg3 = traceBlockReason.\n/ t!=0/ \n{}";
        assert_eq!(
            fmt(input),
            "pid$target::runtime.gopark:entry
// arg3 = traceBlockReason.
/ t != 0 /
{
}
"
        );
    }

    #[test]
    fn test_comment_at_end_of_probe_body_stays_inside_braces() {
        // A comment placed after the last statement but before `}` of a
        // probe body must stay inside the braces, not be swapped past `}`.
        let input = "BEGIN {\n  x = 1;\n  // last comment\n}";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = 1;
  // last comment
}
"
        );
    }

    #[test]
    fn test_trailing_line_comment_stays_attached_to_statement() {
        // A comment on the same line as a statement — `stmt; // remark` —
        // must remain on that line, not get punted to the next line.
        let input = "BEGIN {\n  this->query = stringof(copyin(arg3, arg4)); // Query string.\n}";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  this->query = stringof(copyin(arg3, arg4)); // Query string.
}
"
        );
    }

    #[test]
    fn test_trailing_block_comment_stays_attached_to_statement() {
        // Same rule for `/* */` comments tagged onto the same line.
        let input = "BEGIN {\n  x = 1; /* trailing */\n  y = 2;\n}";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  x = 1; /* trailing */
  y = 2;
}
"
        );
    }

    #[test]
    fn test_blank_line_between_expr_statements_in_block_preserved() {
        // A blank line between two plain statements inside a probe body
        // must be preserved — it carries paragraph-grouping intent.
        let input = "BEGIN {\n  this->a = 1;\n  this->b = 2;\n\n  this->c = 3;\n}";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  this->a = 1;
  this->b = 2;

  this->c = 3;
}
"
        );
    }

    #[test]
    fn test_blank_line_after_if_block_inside_probe_body_preserved() {
        // Regression: a blank line between an `if` statement and the next
        // statement in the same body was collapsed because the gap check
        // used the `if` node's end byte. Verified the `if`'s `then_block`
        // origin spans the braces, so the gap (which includes the blank
        // line) is detected.
        let input = "BEGIN {\n  if (foo) {\n  }\n\n  print(1);\n}";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  if (foo) {
  }

  print(1);
}
"
        );
    }

    #[test]
    fn test_single_line_comments_inside_multiline_if_condition() {
        // A multi-line `if` condition where each `&&` operand is followed
        // by a `//` comment, plus a trailing `/* */` comment between `)`
        // and `{`. None of the comments may be hoisted out of the
        // condition or past the brace.
        let input = "BEGIN {\n  if (this->theirs.tid !=0 &&  // 'if a thread is concurrently accessing the same memory...'\n      this->theirs.tid != this->goroutine_id &&  // 'and this is another thread as the current one...'\n      (this->my_access_kind == AccessKindWrite || this->theirs.kind == AccessKindWrite)) /* 'and at least one access is a write...' */ {\n    printf(\"possible data race: my_access_kind:%d my_tid=%d my_ts=%d their_access_kind:%d their_tid=%d their_ts=%d mem_ptr=%p\\n\", this->my_access_kind, this->goroutine_id, this->now, this->theirs.kind, this->theirs.tid, this->theirs.ts, this->mem_ptr);\n    ustack();\n  }\n}";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  if (this->theirs.tid != 0 && // 'if a thread is concurrently accessing the same memory...'
      this->theirs.tid != this->goroutine_id && // 'and this is another thread as the current one...'
      (this->my_access_kind == AccessKindWrite || this->theirs.kind == AccessKindWrite)) /* 'and at least one access is a write...' */ {
    printf(\"possible data race: my_access_kind:%d my_tid=%d my_ts=%d their_access_kind:%d their_tid=%d their_ts=%d mem_ptr=%p\\n\", this->my_access_kind, this->goroutine_id, this->now, this->theirs.kind, this->theirs.tid, this->theirs.ts, this->mem_ptr);
    ustack();
  }
}
"
        );
    }

    #[test]
    fn test_multi_line_comment_inside_if_condition() {
        // A comment sitting inside the parens of an `if` condition must
        // stay inside the parens, not get moved before or after them.
        let input = "BEGIN {\n  if (foo /* bar */) {\n  }\n}";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  if (foo /* bar */) {
  }
}
"
        );
    }

    #[test]
    fn test_trailing_comment_on_if_open_brace_line() {
        // A `//` comment on the same line as `if (cond) { …` stays on
        // that line instead of being moved into the body.
        let input = "BEGIN {\n  if (foo) { // bar\n  }\n}";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  if (foo) { // bar
  }
}
"
        );
    }

    #[test]
    fn test_multi_line_comment_inside_if_block_body() {
        // A `/* */` comment on the same source line as the opening `{` of
        // an `if`-block body stays attached to that line.
        let input = "BEGIN {\n  if (foo) { /* bar */\n  }\n}";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  if (foo) { /* bar */
  }
}
"
        );
    }

    #[test]
    fn test_inline_multi_line_comment_between_specifier_and_declarator() {
        // A `/* */` comment sitting between two sibling tokens of the same
        // declaration must land at its original position rather than being
        // flushed to the next top-level boundary.
        let input = "int /* x */ y;";
        assert_eq!(fmt(input), "int /* x */ y;\n");
    }

    #[test]
    fn test_struct_with_array_field_and_inline_forward_struct_pointer() {
        // Two regressions combined:
        //   - `uint8_t pad[48];` parses without falsely going down the
        //     parameter-list path inside the array brackets,
        //   - `struct m *m;` parses even though `m` was just registered as
        //     a struct tag (the lexer returns `TypeName` for the declarator
        //     position, and `parse_direct_declarator` accepts it).
        let input = "struct g {\n  uint8_t pad[48];\n  struct m* m;\n};";
        assert_eq!(
            fmt(input),
            "struct g {
  uint8_t pad[48];
  struct m *m;
};
"
        );
    }

    #[test]
    fn test_struct_with_unresolved_identifier_field_types() {
        // A struct body referencing types that haven't been declared in
        // the program (e.g. forward-referenced or external types) must
        // still parse and round-trip; `parse_specifier_qualifier_list`
        // accepts a plain `Identifier` as a fallback typedef name.
        let input = "typedef struct {\n\
                     GoType type;\n\
                     GoType* elem;\n\
                     GoType* slice;\n\
                     uintptr_t len;\n\
                     } GoArrayType;";
        assert_eq!(
            fmt(input),
            "typedef struct {
  GoType type;
  GoType *elem;
  GoType *slice;
  uintptr_t len;
} GoArrayType;
"
        );
    }

    #[test]
    fn test_paren_expression_with_unresolved_ident_lhs() {
        // Regression: `(timestamp - x)` was misparsed as the start of a
        // cast because the cast lookahead matched any `( <Identifier> …)`,
        // including bare identifiers followed by binary operators. The
        // tightened rule only treats `( <Identifier> )` or
        // `( <Identifier> * )` as a cast.
        let input = "pid$target::*NewMigrationBox:return {\n  this->duration = (timestamp - self->t)/1000000;\n}";
        assert_eq!(
            fmt(input),
            "pid$target::*NewMigrationBox:return
{
  this->duration = (timestamp - self->t) / 1000000;
}
"
        );
    }

    #[test]
    fn test_cast_to_builtin_type_pointer() {
        // Regression: `(uintptr_t*)expr` was misparsed because the cast
        // lookahead didn't include `TokenKind::TypeName`, so registered
        // built-in types like `uintptr_t` were not recognised as the start
        // of a cast.
        let input =
            "BEGIN {\n  this->ptr_to_slice_header = *(uintptr_t*)copyin(uregs[R_X26] + 16, 8);\n}";
        assert_eq!(
            fmt(input),
            "BEGIN
{
  this->ptr_to_slice_header = *(uintptr_t*)copyin(uregs[R_X26] + 16, 8);
}
"
        );
    }

    #[test]
    fn test_array_decl_keyed_by_builtin_type_with_inline_comment() {
        // The array subscript holds a type name (`uintptr_t`) and an inline
        // multi-line comment. Both must round-trip in place.
        let input = "typedef struct {\n  int x;\n} Access;\n\
                     Access accesses[uintptr_t /* data ptr */];";
        assert_eq!(
            fmt(input),
            "typedef struct {
  int x;
} Access;

Access accesses[uintptr_t /* data ptr */];\n"
        );
    }

    #[test]
    fn test_typedef_struct_using_builtin_type_after_pragmas() {
        // Reproduces a user-reported failure: `size_t` is a built-in dtrace
        // type (pre-registered by the lexer, not a user typedef) and must be
        // recognised as a type specifier inside a struct body — including
        // when the file starts with `#pragma` directives.
        let input = "#pragma D option dynvarsize=16m\n\
                     #pragma D option cleanrate=100hz\n\
                     \n\
                     typedef enum {AccessKindRead=1, AccessKindWrite=2} AccessKind;\n\
                     \n\
                     typedef struct {\n  AccessKind kind;\n  size_t tid;\n  int ts;\n} Access;\n";
        assert_eq!(
            fmt(input),
            "#pragma D option dynvarsize=16m
#pragma D option cleanrate=100hz

typedef enum {
  AccessKindRead = 1,
  AccessKindWrite = 2
} AccessKind;

typedef struct {
  AccessKind kind;
  size_t tid;
  int ts;
} Access;
"
        );
    }

    #[test]
    fn test_typedef_struct_referencing_prior_typedef_names() {
        // Three back-to-back typedefs: the last one's struct body refers to
        // `AccessKind` and `size_t`, which the formatter must accept as
        // type specifiers thanks to typedef-name registration.
        let input = "typedef enum {AccessKindRead=1, AccessKindWrite=2} AccessKind;\n\
                     typedef unsigned long size_t;\n\
                     typedef struct {AccessKind kind; size_t tid; int ts;} Access;";
        assert_eq!(
            fmt(input),
            "typedef enum {
  AccessKindRead = 1,
  AccessKindWrite = 2
} AccessKind;

typedef unsigned long size_t;

typedef struct {
  AccessKind kind;
  size_t tid;
  int ts;
} Access;
"
        );
    }

    #[test]
    fn test_typedef_anonymous_enum_with_declarator() {
        let input = "typedef enum {AccessKindRead=1, AccessKindWrite=2} AccessKind;";
        assert_eq!(
            fmt(input),
            "typedef enum {
  AccessKindRead = 1,
  AccessKindWrite = 2
} AccessKind;
"
        );
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
