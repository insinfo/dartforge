//! The analysis server 3.6.2 `CompletionTarget.forOffset`
//! (`AS:src/provisional/completion/dart/completion_target.dart:141-260`), the
//! replacement range (`computeReplacementRange`, `:470-526`) and the two
//! prefixes: the matcher's (`TokenData.fromSelection`,
//! `completion_manager.dart:484-539`) and the request's (`targetPrefix`,
//! `:388-445`); docs/LSP-ESPECIFICACAO.md §14.13.1-3.
//!
//! The entities of a node are its children and the tokens of the text inside
//! the node that no child covers. The DartForge tree has no synthetic tokens
//! (the parser recovers without creating them): a token that fasta would
//! create as synthetic simply is not there; the synthetic-token rules
//! (length 0) apply to the tree's empty nodes, which sit at the next real
//! token where fasta would put them.
//! Written without compiling or running (2026-10-05).

use super::isp::{Initial, Isp};
use dartforge_frontend::token::Kind;

/// A child entity (`childEntities`): a tree node or a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Entity {
    No(usize),
    Tok(usize),
    /// A comment (Dart's `CommentToken`): the range.
    Comment(usize, usize),
}

/// The request's target.
#[derive(Debug, Clone, Copy)]
pub(super) struct Target {
    pub node: usize,
    pub entity: Option<Entity>,
    /// `isCommentText`: the cursor is in the text of an ordinary comment.
    pub comment_text: bool,
}

impl<'a, 'c> Isp<'a, 'c> {
    /// The entities of a node, in source order.
    pub(super) fn entities(&self, n: usize) -> Vec<Entity> {
        let children: Vec<usize> = self.children(n).to_vec();
        let (start, end) = (self.start(n), self.end(n));
        let mut v: Vec<(usize, Entity)> = children.iter().map(|&f| (self.start(f), Entity::No(f))).collect();
        let mut i = self.tok_since(start);
        while let Some(t) = self.toks.get(i) {
            if t.span.start >= end || t.kind == Kind::Eof {
                break;
            }
            let inside_child = children.iter().any(|&f| self.start(f) <= t.span.start && t.span.end <= self.end(f) && self.start(f) < self.end(f));
            if !inside_child {
                v.push((t.span.start, Entity::Tok(i)));
            }
            i += 1;
        }
        v.sort_by_key(|(p, e)| (*p, matches!(e, Entity::No(_))));
        v.into_iter().map(|(_, e)| e).collect()
    }

    /// The last token of a node (`endToken`); `None` for an empty node.
    pub(super) fn last_token(&self, n: usize) -> Option<usize> {
        self.tok_before(self.end(n)).filter(|&i| self.span_tok(i).start >= self.start(n) && self.start(n) < self.end(n))
    }

    /// `_isCandidateToken` for a real token.
    fn candidate_tok(&self, i: usize) -> bool {
        let s = self.span_tok(i);
        if self.offset < s.end {
            return true;
        }
        if self.offset == s.end {
            return self.word(i);
        }
        false
    }

    /// `_isCandidateToken(node, entity.endToken)` of a node: an empty one is a
    /// synthetic token of length 0 at the next real token.
    fn candidate_end(&self, n: usize) -> bool {
        match self.last_token(n) {
            // The node ends in a synthetic token (the `)`/`;` that fasta inserts, of
            // length 0 at the end of the node), after the last real one.
            Some(i) if self.span_tok(i).end < self.end(n) => self.offset < self.end(n),
            Some(i) => self.candidate_tok(i),
            None => self.offset <= self.synthetic(self.start(n)).start,
        }
    }

    /// `_isCandidateNode`.
    fn candidate_node(&self, n: usize) -> bool {
        match self.initial(n) {
            Initial::Tok(i) if self.span_tok(i).start < self.end(n).max(self.start(n) + 1) && self.word(i) => self.candidate_tok(i),
            _ if self.start(n) >= self.end(n) => {
                // Empty: the synthetic identifier (word) or another synthetic token, both
                // at `offset <= position`.
                self.offset <= self.synthetic(self.start(n)).start
            }
            _ => self.offset <= self.start(n),
        }
    }

    /// `_getContainingCommentToken`: among the comments preceding the token that
    /// starts at `before` (EOF with `eof`), the one containing the cursor (a
    /// line comment with an inclusive end, a block one without).
    fn containing_comment(&self, before: usize, eof: bool) -> Option<(usize, usize)> {
        if !eof && self.offset >= before {
            return None;
        }
        let from = self.tok_before(before).map_or(0, |i| self.span_tok(i).end);
        for c in dartforge_frontend::comments::Comments::scan(self.source).all() {
            if c.start < from || c.end > before {
                continue;
            }
            if self.offset <= c.start {
                return None;
            }
            if self.offset <= c.end {
                let line = self.source[c.start..].starts_with("//");
                if line || self.offset < c.end {
                    return Some((c.start, c.end));
                }
            }
        }
        None
    }

    /// `_getContainingDocComment`: the node has the documentation comment that
    /// is `c`.
    fn documentation_comment(&self, n: usize, c: (usize, usize)) -> Option<usize> {
        self.children_of(n, &["Comment"]).into_iter().find(|&k| self.start(k) <= c.0 && c.1 <= self.end(k))
    }

    /// `CompletionTarget.forOffset(unit, offset)`.
    pub(super) fn target(&self) -> Target {
        let mut node = 0usize;
        'external: loop {
            if self.kind(node) == "Comment" {
                for r in self.children_of(node, &["CommentReference"]) {
                    if self.start(r) <= self.offset && self.offset <= self.end(r) {
                        node = r;
                        continue 'external;
                    }
                }
            }
            for e in self.entities(node) {
                match e {
                    Entity::Tok(i) => {
                        if !self.candidate_tok(i) {
                            continue;
                        }
                        if let Some(c) = self.containing_comment(self.span_tok(i).start, false) {
                            return match self.documentation_comment(node, c) {
                                Some(d) => Target { node: d, entity: Some(Entity::Comment(c.0, c.1)), comment_text: false },
                                None => Target { node: 0, entity: Some(Entity::Comment(c.0, c.1)), comment_text: true },
                            };
                        }
                        return Target { node, entity: Some(e), comment_text: false };
                    }
                    Entity::No(f) => {
                        if !self.candidate_end(f) {
                            continue;
                        }
                        if self.candidate_node(f) {
                            let start = match self.initial(f) {
                                Initial::Tok(i) => self.span_tok(i).start,
                                _ => self.start(f),
                            };
                            if let Some(c) = self.containing_comment(start, false) {
                                return match self.documentation_comment(node, c) {
                                    Some(d) => Target { node: d, entity: Some(Entity::Comment(c.0, c.1)), comment_text: false },
                                    None => Target { node: 0, entity: Some(Entity::Comment(c.0, c.1)), comment_text: true },
                                };
                            }
                            return Target { node, entity: Some(e), comment_text: false };
                        }
                        node = f;
                        continue 'external;
                    }
                    Entity::Comment(..) => {}
                }
            }
            break;
        }
        if node == 0
            && let Some(c) = self.containing_comment(self.source.len(), true)
        {
            return Target { node: 0, entity: Some(Entity::Comment(c.0, c.1)), comment_text: true };
        }
        Target { node, entity: None, comment_text: false }
    }

    /// The start token of an entity.
    fn entity_token(&self, e: Entity) -> Option<usize> {
        match e {
            Entity::Tok(i) => Some(i),
            Entity::No(n) => match self.initial(n) {
                Initial::Tok(i) if self.span_tok(i).start < self.end(n).max(self.start(n) + 1) => Some(i),
                _ => None,
            },
            Entity::Comment(..) => None,
        }
    }

    /// `containingNode.findPrevious(token)`: the previous token, if it still
    /// belongs to the node.
    fn previous_in_node(&self, node: usize, i: usize) -> Option<usize> {
        let j = i.checked_sub(1)?;
        (self.span_tok(j).start >= self.start(node)).then_some(j)
    }

    /// `_computeDroppedToken`: a word token outside the tree between the child
    /// before the entity and the entity, which contains the cursor.
    pub(super) fn dropped_token(&self, target: &Target) -> Option<usize> {
        let e = target.entity?;
        let start_and = match e {
            Entity::No(n) => self.start(n),
            Entity::Tok(i) => self.span_tok(i).start,
            Entity::Comment(a, _) => a,
        };
        let previous: Vec<usize> = self.children(target.node).iter().copied().filter(|&f| self.end(f) <= start_and && self.kind(f) != "Comment").collect();
        let last = *previous.last()?;
        let mut i = self.last_token(last)? + 1;
        while let Some(t) = self.toks.get(i) {
            if t.span.start >= start_and || t.kind == Kind::Eof {
                break;
            }
            let inside = self.children(target.node).iter().any(|&f| self.start(f) <= t.span.start && t.span.end <= self.end(f));
            if !inside && self.word(i) && t.span.start <= self.offset && self.offset <= t.span.end {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// `computeReplacementRange`: `(start, length)`.
    pub(super) fn replacement_range(&self, target: &Target) -> (usize, usize) {
        let mut token = self.dropped_token(target).or_else(|| target.entity.and_then(|e| self.entity_token(e)));
        if let Some(t) = token
            && self.offset < self.span_tok(t).start
        {
            token = self.previous_in_node(target.node, t);
        }
        if let Some(t) = token {
            let mut t = Some(t);
            if let Some(x) = t
                && self.offset == self.span_tok(x).start
                && !self.word(x)
            {
                t = self.previous_in_node(target.node, x);
            }
            if let Some(x) = t {
                let s = self.span_tok(x);
                if self.word(x) && s.start <= self.offset && self.offset <= s.end {
                    return (s.start, s.end - s.start);
                }
                if matches!(self.toks[x].kind, Kind::Str(_)) {
                    // The URI of a directive (or of its configuration).
                    let uri = match self.kind(target.node) {
                        "ImportDirective" | "ExportDirective" => self
                            .children(target.node)
                            .iter()
                            .copied()
                            .find(|&k| self.kind(k) == "SimpleStringLiteral" && self.start(k) <= self.offset && self.offset <= self.end(k))
                            .or_else(|| {
                                self.children_of(target.node, &["Configuration"])
                                    .into_iter()
                                    .flat_map(|c| self.children_of(c, &["SimpleStringLiteral"]))
                                    .find(|&k| self.start(k) <= self.offset && self.offset <= self.end(k))
                            }),
                        "SimpleStringLiteral" => {
                            let parent = self.parent(target.node);
                            let directive = parent.filter(|&p| self.is_directive(p) || (self.kind(p) == "Configuration" && self.parent(p).is_some_and(|d| self.is_directive(d))));
                            directive.map(|_| target.node)
                        }
                        _ => None,
                    };
                    if let Some(u) = uri {
                        let (ci, cf) = (self.content_start(u), self.content_end(u));
                        if ci <= self.offset && self.offset <= cf {
                            return (ci, cf - ci);
                        }
                    }
                }
            }
        }
        (self.offset, 0)
    }

    /// `contentsEnd`: before the closing quote (if it is there).
    pub(super) fn content_end(&self, n: usize) -> usize {
        let start_c = self.content_start(n);
        let t = &self.source[self.start(n)..self.end(n)];
        let opens = &self.source[self.start(n)..start_c];
        let quote = opens.trim_start_matches('r');
        if t.len() > opens.len() && t.ends_with(quote) && !quote.is_empty() {
            self.end(n) - quote.len()
        } else {
            self.end(n)
        }
    }

    /// `TokenData.fromSelection`: the matcher's prefix.
    pub(super) fn matcher_prefix(&self) -> String {
        let Some(c) = self.covered_node() else { return String::new() };
        let Some(mut t) = self.tok_before(self.end(c)).or_else(|| self.tok_after(self.end(c))) else { return String::new() };
        loop {
            let s = self.span_tok(t);
            if s.start > self.offset || (s.start == self.offset && !self.word(t)) {
                match t.checked_sub(1) {
                    Some(j) => t = j,
                    None => return String::new(),
                }
                continue;
            }
            break;
        }
        let s = self.span_tok(t);
        if self.toks[t].kind == Kind::Eof {
            return String::new();
        }
        if self.offset > s.end {
            return String::new();
        }
        if self.word(t) {
            return self.source[s.start..self.offset].to_string();
        }
        if let Kind::Str(_) | Kind::StrBegin(..) = self.toks[t].kind {
            let text = &self.source[s.start..s.end];
            let skip = if text.starts_with("r'''") || text.starts_with("r\"\"\"") {
                4
            } else if text.starts_with("r'") || text.starts_with("r\"") {
                2
            } else if text.starts_with("'''") || text.starts_with("\"\"\"") {
                3
            } else {
                1
            };
            let start = s.start + skip;
            if self.offset < start {
                return String::new();
            }
            return self.source[start..self.offset].to_string();
        }
        String::new()
    }

    /// `targetPrefix`: the request's prefix (the handler's filter).
    pub(super) fn request_prefix(&self, target: &Target) -> String {
        let Some(e) = target.entity else { return String::new() };
        let of_token = |i: usize| -> String {
            let s = self.span_tok(i);
            if s.start <= self.offset && self.offset < s.end {
                self.source[s.start..self.offset].to_string()
            } else if self.offset == s.end {
                self.source[s.start..s.end].to_string()
            } else {
                String::new()
            }
        };
        if let Entity::Tok(i) = e {
            if let Some(j) = i.checked_sub(1)
                && self.span_tok(j).end == self.offset
                && self.word(j)
            {
                return self.text_tok(j).to_string();
            }
            if let Kind::Str(_) = self.toks[i].kind {
                let s = self.span_tok(i);
                let uri = (target.node != 0 && self.kind(target.node) == "SimpleStringLiteral" && self.parent(target.node).is_some_and(|p| self.is_directive(p))).then_some(target.node);
                if s.start < self.offset
                    && self.offset < s.end
                    && let Some(u) = uri
                {
                    let ci = self.content_start(u);
                    if self.offset >= ci {
                        return self.source[ci..self.offset].to_string();
                    }
                }
            }
            if self.word(i) && self.span_tok(i).end == self.offset {
                return self.text_tok(i).to_string();
            }
            return String::new();
        }
        let Entity::No(mut n) = e else { return String::new() };
        if self.kind(n) == "DeclaredVariablePattern"
            && let Some(name) = self.variable_pattern_name(n)
            && self.span_tok(name).start <= self.offset
        {
            return of_token(name);
        }
        loop {
            if self.kind(n) == "SimpleIdentifier" {
                return match self.initial(n) {
                    Initial::Tok(i) => of_token(i),
                    _ => String::new(),
                };
            }
            // The first `childEntity`: a node descends; a token stops.
            match self.entities(n).first().copied() {
                Some(Entity::No(f)) => n = f,
                Some(Entity::Tok(i)) => return of_token(i),
                _ => return String::new(),
            }
        }
    }

    /// `inConstantContext` of the request: the entity is an expression in a
    /// constant context.
    pub(super) fn request_in_constant_context(&self, target: &Target) -> bool {
        match target.entity {
            Some(Entity::No(n)) => super::isp::EXPRESSION_KINDS.contains(&self.kind(n)) && self.constant_context(n),
            _ => false,
        }
    }

}
