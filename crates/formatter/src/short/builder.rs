//! Port of `short/chunk_builder.dart` and `short/nesting_builder.dart`:
//! receives the visitor's incremental output (text, spaces, splits,
//! comments) and builds the chunk tree.
//!
//! The original swaps the visitor's `builder` for a child block's builder in
//! `startBlock` and goes back to the parent in `endBlock`. Here the builders
//! live on a stack in the visitor; [`ChunkBuilder::block`] remembers the
//! block chunk whose children this builder writes, and `endBlock` moves the
//! list there.

use super::arena::{Arenas, ChunkId, NestingId, RuleId, cost, indentation};

/// Comment kind (`CommentType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    Doc,
    Line,
    Block,
    InlineBlock,
}

/// A comment ready to be written (`SourceComment`).
#[derive(Debug, Clone)]
pub struct SourceComment {
    pub text: String,
    pub kind_: CommentKind,
    pub lines_before: i32,
    pub flush_left: bool,
}

/// `NestingBuilder`.
#[derive(Debug, Clone)]
struct NestingStack {
    stack: Vec<i32>,
    pending: Option<NestingId>,
    current: NestingId,
}

impl NestingStack {
    fn indentation(&self) -> i32 {
        *self.stack.last().expect("indentation stack")
    }

    fn effective(&self) -> NestingId {
        self.pending.unwrap_or(self.current)
    }

    fn nest(&mut self, a: &mut Arenas, indentation: i32) {
        let base = self.pending.unwrap_or(self.current);
        self.pending = Some(a.nest(base, indentation));
    }

    fn unnest_(&mut self, a: &Arenas) {
        let base = self.pending.unwrap_or(self.current);
        self.pending = a.nestings[base as usize].parent;
        debug_assert!(self.pending.is_some(), "unnest without nest");
    }

    fn commit(&mut self) {
        if let Some(p) = self.pending.take() {
            self.current = p;
        }
    }
}

/// `ChunkBuilder`.
#[derive(Debug)]
pub struct ChunkBuilder {
    /// The block chunk whose children this builder writes; `None` for the
    /// top-level builder.
    pub block: Option<ChunkId>,
    pub chunks: Vec<ChunkId>,
    pending_newlines: u8,
    pending_space: bool,
    pending_nested: bool,
    pending_flush_left: bool,
    after_comment: bool,
    rules: Vec<RuleId>,
    hard_rules: Vec<RuleId>,
    lazy_rules_: Vec<RuleId>,
    open_spans: Vec<(usize, i32)>,
    nesting: NestingStack,
    argument_nesting: Vec<NestingId>,
    prevent_split: i32,
}

fn ends_with_ident(text: &str) -> bool {
    text.as_bytes().last().is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

fn is_generic_method_comment(c: &SourceComment) -> bool {
    c.text.starts_with("/*<") || c.text.starts_with("/*=")
}

impl ChunkBuilder {
    fn base(a: &mut Arenas, block: Option<ChunkId>) -> ChunkBuilder {
        let root = a.new_root_nesting();
        let mut c = ChunkBuilder {
            block,
            chunks: Vec::new(),
            pending_newlines: 0,
            pending_space: false,
            pending_nested: false,
            pending_flush_left: false,
            after_comment: false,
            rules: Vec::new(),
            hard_rules: Vec::new(),
            lazy_rules_: Vec::new(),
            open_spans: Vec::new(),
            nesting: NestingStack {
                stack: vec![0],
                pending: None,
                current: root,
            },
            argument_nesting: Vec::new(),
            prevent_split: 0,
        };
        c.start_argument_nesting();
        c
    }

    /// `ChunkBuilder(formatter, source)`: the top-level builder.
    pub fn top(a: &mut Arenas, initial_indent: i32) -> ChunkBuilder {
        let mut c = ChunkBuilder::base(a, None);
        // `indent(_formatter.indent)` before `startBlockArgumentNesting`.
        c.nesting.stack.push(initial_indent);
        c
    }

    /// The innermost rule (`rule`).
    pub fn rule(&self) -> RuleId {
        *self.rules.last().expect("active rule")
    }

    /// `write(string, mergeEmptySplits:)`.
    pub fn write(&mut self, a: &mut Arenas, text: &str, merge_empty: bool) {
        self.emit_pending_whitespace(a, false, merge_empty);
        self.write_text(a, text, None);
        let lazy_rules = std::mem::take(&mut self.lazy_rules_);
        for r in lazy_rules {
            self.activate_rule(a, r);
        }
        self.nesting.commit();
        self.after_comment = false;
    }

    /// `writeNewline(isDouble:, flushLeft:, nest:)`.
    pub fn newline(&mut self, double: bool, flush_left: bool, nest: bool) {
        self.pending_newlines = if double { 2 } else { 1 };
        self.pending_flush_left = flush_left;
        self.pending_nested = nest;
    }

    /// `writeSpace()`.
    pub fn space(&mut self) {
        self.pending_space = true;
    }

    /// `split(nest:, space:)`.
    pub fn split(&mut self, a: &mut Arenas, nest: bool, space: bool) -> ChunkId {
        if self.prevent_split > 0 {
            self.pending_newlines = 0;
            self.pending_nested = false;
            if space {
                self.pending_space = true;
            }
            return a.dummy_chunk();
        }
        if self.pending_newlines > 0 {
            return a.dummy_chunk();
        }
        self.write_split(a, false, false, nest, space, true)
    }

    /// `writeComments(comments, linesBeforeToken, token)`.
    pub fn write_comments(&mut self, a: &mut Arenas, comments: &mut [SourceComment], lines_before_token: i32, token: &str) {
        if self.pending_newlines == 2 && comments[0].lines_before < 2 {
            if lines_before_token > 1 {
                self.newline(false, false, false);
            } else {
                for c in comments.iter().skip(1) {
                    if c.lines_before > 1 {
                        self.newline(false, false, false);
                        break;
                    }
                }
            }
        }

        if self.after_comment && self.pending_newlines > 0 {
            comments[0].lines_before = 1;
            self.pending_newlines = 0;
        }

        if lines_before_token == 0
            && i32::from(self.pending_newlines) > comments[0].lines_before
            && comments.iter().all(|c| c.kind_ == CommentKind::InlineBlock)
        {
            comments[0].lines_before = i32::from(self.pending_newlines);
        }

        for i in 0..comments.len() {
            let comment = comments[i].clone();
            let chunk = self.chunk_for_comment(a, &comment, token);
            if let Some(chunk) = chunk {
                self.pending_space = self.needs_space_before(a, &comment, chunk);
                let last = *self.chunks.last().expect("chunk");
                if self.pending_space && chunk != last {
                    a.update_split(last, None, false, Some(true));
                }
            } else {
                if self.pending_newlines == 0 {
                    if comment.lines_before > 0 && (self.after_comment || comment.kind_ != CommentKind::InlineBlock) {
                        let double = self.needs_blank_line(a, &comment);
                        self.newline(double, comment.flush_left, true);
                    } else if let Some(&last) = self.chunks.last() {
                        self.pending_space = self.needs_space_before(a, &comment, last);
                    }
                } else {
                    self.pending_flush_left = comment.flush_left;
                }
                let double = self.needs_blank_line(a, &comment);
                self.emit_pending_whitespace(a, double, true);
            }

            self.write_text(a, &comment.text, chunk);

            let mut lines_after;
            if i < comments.len() - 1 {
                lines_after = comments[i + 1].lines_before;
            } else {
                lines_after = lines_before_token;
                if lines_after == 0 && comments.last().expect("comment").text.contains('\n') {
                    lines_after = 1;
                }
            }
            if lines_after > 0 {
                let double = self.pending_newlines == 2 || lines_after > 1;
                self.newline(double, false, true);
            }
        }

        let last = comments.last().expect("comment").clone();
        self.pending_space = self.needs_space_after(a, &last, token);
        self.after_comment = true;
    }

    /// `indent([spaces])`.
    pub fn indent(&mut self, spaces: Option<i32>) {
        let e = spaces.unwrap_or(indentation::BLOCK);
        debug_assert!(self.nesting.pending.is_none());
        let top = self.nesting.indentation();
        self.nesting.stack.push(top + e);
    }

    /// `unindent()`.
    pub fn unindent_(&mut self) {
        debug_assert!(self.nesting.pending.is_none());
        self.nesting.stack.pop();
        debug_assert!(!self.nesting.stack.is_empty());
    }

    /// `startSpan([cost])`.
    pub fn start_span_(&mut self, cost: i32) {
        self.open_spans.push((self.chunks.len(), cost));
    }

    /// `endSpan()`.
    pub fn end_span_(&mut self, a: &mut Arenas) {
        let (start, cost) = self.open_spans.pop().expect("open span");
        let end = self.chunks.len();
        if start == end {
            return;
        }
        let span = a.new_span(cost);
        for i in start..end {
            let c = self.chunks[i];
            let r = a.chunks[c as usize].rule;
            if !a.is_hardened(r) {
                a.chunks[c as usize].spans.push(span);
            }
        }
    }

    /// `startRule([rule])`.
    pub fn start_rule_(&mut self, a: &mut Arenas, rule: Option<RuleId>) {
        let r = rule.unwrap_or_else(|| a.rule(cost::NORMAL));
        let lazy_rules = std::mem::take(&mut self.lazy_rules_);
        for p in lazy_rules {
            self.activate_rule(a, p);
        }
        self.activate_rule(a, r);
    }

    fn activate_rule(&mut self, a: &mut Arenas, r: RuleId) {
        for &outer in &self.rules {
            if !a.splits_with_inner(outer) {
                continue;
            }
            a.constrain_when_split(r, outer);
        }
        self.rules.push(r);
    }

    /// `startLazyRule([rule])`.
    pub fn start_lazy_rule_(&mut self, a: &mut Arenas, rule: Option<RuleId>) {
        let r = rule.unwrap_or_else(|| a.rule(cost::NORMAL));
        self.lazy_rules_.push(r);
    }

    /// `endRule()`.
    pub fn end_rule_(&mut self) {
        if !self.lazy_rules_.is_empty() {
            self.lazy_rules_.pop();
        } else {
            self.rules.pop();
        }
    }

    /// `forceRules()`.
    pub fn force_rules_(&mut self, a: &Arenas) {
        self.handle_hard_split(a);
    }

    /// `nestExpression(indent:, now:)`.
    pub fn nest_expression_at(&mut self, a: &mut Arenas, indentation: Option<i32>, now: bool) {
        self.nesting.nest(a, indentation.unwrap_or(indentation::EXPRESSION));
        if now {
            self.nesting.commit();
        }
    }

    /// `unnest(now:)`.
    pub fn unnest_(&mut self, a: &Arenas, now: bool) {
        self.nesting.unnest_(a);
        if now {
            self.nesting.commit();
        }
    }

    /// `startBlockArgumentNesting()`.
    pub fn start_argument_nesting(&mut self) {
        self.argument_nesting.push(self.nesting.effective());
    }

    /// `endBlockArgumentNesting()`.
    pub fn end_argument_nesting(&mut self) {
        self.argument_nesting.pop();
    }

    /// `startBlock(argumentChunk:, indent:, space:)`: creates the block chunk
    /// here and returns the builder for the children.
    pub fn start_block(&mut self, a: &mut Arenas, argument: Option<ChunkId>, indent: bool, space: bool) -> ChunkBuilder {
        let rule = *self.rules.last().expect("block rule");
        let nesting_ = *self.argument_nesting.last().expect("argument nesting");
        let chunk = a.new_block(argument, rule, self.nesting.indentation(), nesting_, space, self.pending_flush_left);
        self.chunks.push(chunk);
        self.pending_flush_left = false;

        let mut child = ChunkBuilder::base(a, Some(chunk));
        if indent {
            child.indent(None);
        }
        let hard = a.hard_rule();
        child.start_rule_(a, Some(hard));
        child.split(a, false, space);
        child
    }

    /// `endBlock(forceSplit:)`, first half: closes the children, moves them to
    /// the block chunk and tells whether the parent must force its rules.
    pub fn end_block(mut self, a: &mut Arenas, force: bool) -> bool {
        self.divide_chunks(a);
        let mut force = force || self.pending_nested;
        if !force {
            let mut length = 0usize;
            let first_ = self.rules.first().copied();
            for &c in &self.chunks {
                length += a.chunks[c as usize].length() + a.block_length(c);
                if length > a.page_width {
                    force = true;
                    break;
                }
                let r = a.chunks[c as usize].rule;
                if a.is_hardened(r) && Some(r) != first_ {
                    force = true;
                    break;
                }
            }
        }
        let block = self.block.expect("endBlock outside a block");
        if let Some(b) = &mut a.chunks[block as usize].block {
            b.children = std::mem::take(&mut self.chunks);
        }
        force
    }

    /// `end()`: divides the top-level chunks and returns the list.
    pub fn finish(mut self, a: &mut Arenas) -> Vec<ChunkId> {
        debug_assert!(self.rules.is_empty());
        self.divide_chunks(a);
        self.chunks
    }

    /// `preventSplit()`.
    pub fn prevent_splits(&mut self) {
        self.prevent_split += 1;
    }

    /// `endPreventSplit()`.
    pub fn allow_splits(&mut self) {
        self.prevent_split -= 1;
        debug_assert!(self.prevent_split >= 0);
    }

    fn emit_pending_whitespace(&mut self, a: &mut Arenas, double: bool, merge_empty: bool) {
        if self.pending_newlines == 0 {
            return;
        }
        let double = double || self.pending_newlines == 2;
        let nest = self.pending_nested;
        self.write_split(a, true, double, nest, false, merge_empty);
    }

    fn chunk_for_comment(&self, a: &Arenas, c: &SourceComment, token: &str) -> Option<ChunkId> {
        if self.chunks.is_empty() {
            return None;
        }
        if c.lines_before != 0 {
            return None;
        }
        if c.kind_ == CommentKind::Doc || c.kind_ == CommentKind::Block {
            return None;
        }
        let mut chunk = *self.chunks.last().expect("chunk");
        if a.chunks[chunk as usize].text.is_empty() && self.chunks.len() > 1 && (!is_generic_method_comment(c) || token != "(") {
            chunk = self.chunks[self.chunks.len() - 2];
        }
        let text = &a.chunks[chunk as usize].text;
        if text.ends_with(',') && c.kind_ == CommentKind::InlineBlock {
            return None;
        }
        if text.ends_with('(') || text.ends_with('[') || (text.ends_with('{') && !text.ends_with("${")) {
            return None;
        }
        Some(chunk)
    }

    fn needs_space_before(&self, a: &Arenas, c: &SourceComment, chunk: ChunkId) -> bool {
        let text = &a.chunks[chunk as usize].text;
        if text.is_empty() {
            return false;
        }
        if c.kind_ == CommentKind::Line {
            return true;
        }
        if is_generic_method_comment(c) && ends_with_ident(text) {
            return false;
        }
        !text.ends_with('(') && !text.ends_with('[') && !text.ends_with('{')
    }

    fn needs_space_after(&self, a: &Arenas, c: &SourceComment, token: &str) -> bool {
        let last = *self.chunks.last().expect("chunk");
        if a.chunks[last as usize].text.is_empty() {
            return false;
        }
        if self.pending_newlines > 0 {
            return false;
        }
        if is_generic_method_comment(c) && token == "(" {
            return false;
        }
        !matches!(token, ")" | "]" | "}" | "," | ";" | "")
    }

    fn needs_blank_line(&self, a: &Arenas, c: &SourceComment) -> bool {
        if c.lines_before < 2 {
            return false;
        }
        let Some(&last) = self.chunks.last() else {
            return false;
        };
        let text = &a.chunks[last as usize].text;
        if text.ends_with('{') || text.ends_with('[') {
            return false;
        }
        true
    }

    fn write_split(&mut self, a: &mut Arenas, hard: bool, double: bool, nest: bool, space: bool, merge_empty: bool) -> ChunkId {
        let chunk;
        let last_empty = self.chunks.last().is_some_and(|&c| a.chunks[c as usize].text.is_empty());
        if merge_empty && last_empty {
            chunk = *self.chunks.last().expect("chunk");
            let mut double = double;
            if double && self.chunks.len() > 1 && a.chunks[self.chunks[self.chunks.len() - 2] as usize].text.ends_with('{') {
                double = false;
            }
            let r = a.chunks[chunk as usize].rule;
            a.harden(r);
            a.update_split(chunk, Some(self.pending_flush_left), double, None);
        } else {
            let nesting_ = if nest { self.nesting.current } else { a.new_root_nesting() };
            chunk = self.start_chunk(a, nesting_, hard, double, space);
        }
        let r = a.chunks[chunk as usize].rule;
        if a.is_hardened(r) {
            self.handle_hard_split(a);
        }
        self.pending_newlines = 0;
        self.pending_nested = false;
        chunk
    }

    fn write_text(&mut self, a: &mut Arenas, text: &str, chunk: Option<ChunkId>) {
        let chunk = match chunk {
            Some(c) => c,
            None => {
                if self.chunks.is_empty() {
                    let n = a.new_root_nesting();
                    self.start_chunk(a, n, true, false, false);
                }
                *self.chunks.last().expect("chunk")
            }
        };
        if self.pending_space && !a.chunks[chunk as usize].text.is_empty() {
            a.append(chunk, " ");
        }
        self.pending_space = false;
        a.append(chunk, text);
    }

    fn start_chunk(&mut self, a: &mut Arenas, nesting_: NestingId, hard: bool, double: bool, space: bool) -> ChunkId {
        let rule = if hard { a.hard_rule() } else { *self.rules.last().expect("active rule") };
        let c = a.new_chunk(rule, self.nesting.indentation(), nesting_, space, self.pending_flush_left, double);
        self.chunks.push(c);
        self.pending_flush_left = false;
        c
    }

    fn divide_chunks(&mut self, a: &mut Arenas) {
        self.harden_rules(a);
        for (i, &c) in self.chunks.iter().enumerate() {
            if !self.can_divide(a, i, c) {
                a.chunks[c as usize].splittable = false;
            }
        }
    }

    fn can_divide(&self, a: &Arenas, i: usize, c: ChunkId) -> bool {
        if i == 0 {
            return false;
        }
        let ch = &a.chunks[c as usize];
        if !a.is_hardened(ch.rule) {
            return false;
        }
        if a.is_nested(ch.nesting_) {
            return false;
        }
        if ch.block.is_some() {
            return false;
        }
        true
    }

    fn handle_hard_split(&mut self, a: &Arenas) {
        let Some(&last_item) = self.rules.last() else {
            return;
        };
        if !a.splits_with_inner(last_item) {
            return;
        }
        if !self.hard_rules.contains(&last_item) {
            self.hard_rules.push(last_item);
        }
    }

    fn harden_rules(&mut self, a: &mut Arenas) {
        if self.hard_rules.is_empty() {
            return;
        }
        fn walk(a: &mut Arenas, r: RuleId) {
            a.harden(r);
            let mut i = 0;
            while let Some(other) = a.constrained(r, i) {
                i += 1;
                if other == r {
                    continue;
                }
                let total = a.full_value(r);
                if !a.is_hardened(other) && a.constrains(r, total, other) == Some(a.full_value(other)) {
                    walk(a, other);
                }
            }
        }
        for &r in &self.hard_rules.clone() {
            walk(a, r);
        }
        for &c in &self.chunks {
            let r = a.chunks[c as usize].rule;
            if a.is_hardened(r) {
                a.chunks[c as usize].spans.clear();
            }
        }
    }
}
