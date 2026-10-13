//! O `CompletionTarget.forOffset` do analysis server 3.6.2
//! (`AS:src/provisional/completion/dart/completion_target.dart:141-260`), o
//! intervalo de substituição (`computeReplacementRange`, `:470-526`) e os
//! dois prefixos: o do casador (`TokenData.fromSelection`,
//! `completion_manager.dart:484-539`) e o do pedido (`targetPrefix`,
//! `:388-445`); docs/LSP-ESPECIFICACAO.md §14.13.1-3.
//!
//! As entidades de um nó são os filhos dele e os tokens do texto dentro do
//! nó que nenhum filho cobre. A árvore do DartForge não tem tokens
//! sintéticos (o parser recupera sem criá-los): um token que o fasta
//! criaria sintético simplesmente não está; as regras de token sintético
//! (comprimento 0) valem para os nós vazios da árvore, que ficam no
//! próximo token real como o fasta os poria.
//! Escrito sem compilar nem executar (2026-10-05).

use super::isp::{Initial, Isp};
use dartforge_frontend::token::{Kind, Op};

/// Uma entidade filha (`childEntities`): um nó da árvore ou um token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Entity {
    No(usize),
    Tok(usize),
    /// Um comentário (o `CommentToken` do Dart): o intervalo.
    Comment(usize, usize),
}

/// O alvo do pedido.
#[derive(Debug, Clone, Copy)]
pub(super) struct Target {
    pub no: usize,
    pub entity: Option<Entity>,
    /// `isCommentText`: o cursor está no texto de um comentário comum.
    pub comment_text: bool,
}

impl<'a, 'c> Isp<'a, 'c> {
    /// As entidades de um nó, na ordem do fonte.
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

    /// O último token de um nó (`endToken`); `None` num nó vazio.
    pub(super) fn last_token(&self, n: usize) -> Option<usize> {
        self.tok_before(self.end(n)).filter(|&i| self.span_tok(i).start >= self.start(n) && self.start(n) < self.end(n))
    }

    /// `_isCandidateToken` para um token real.
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

    /// `_isCandidateToken(node, entity.endToken)` de um nó: o vazio é um
    /// token sintético de comprimento 0 no próximo token real.
    fn candidate_end(&self, n: usize) -> bool {
        match self.last_token(n) {
            // O nó acaba num token sintético (o `)`/`;` que o fasta insere,
            // de comprimento 0 no fim do nó), depois do último real.
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
                // Vazio: o identificador sintético (palavra) ou outro token
                // sintético, ambos em `offset <= posição`.
                self.offset <= self.synthetic(self.start(n)).start
            }
            _ => self.offset <= self.start(n),
        }
    }

    /// `_getContainingCommentToken`: entre os comentários que precedem o
    /// token que começa em `before` (o EOF com `eof`), o que contém o
    /// cursor (o de linha com o fim inclusivo, o de bloco sem).
    fn containing_comment(&self, before: usize, eof: bool) -> Option<(usize, usize)> {
        if !eof && self.offset >= before {
            return None;
        }
        let de = self.tok_before(before).map_or(0, |i| self.span_tok(i).end);
        for c in dartforge_frontend::comentarios::Comentarios::de(self.source).todos() {
            if c.start < de || c.end > before {
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

    /// `_getContainingDocComment`: o nó tem o comentário de documentação
    /// que é o `c`.
    fn documentation_comment(&self, n: usize, c: (usize, usize)) -> Option<usize> {
        self.children_of(n, &["Comment"]).into_iter().find(|&k| self.start(k) <= c.0 && c.1 <= self.end(k))
    }

    /// `CompletionTarget.forOffset(unit, offset)`.
    pub(super) fn target(&self) -> Target {
        let mut no = 0usize;
        'external: loop {
            if self.kind(no) == "Comment" {
                for r in self.children_of(no, &["CommentReference"]) {
                    if self.start(r) <= self.offset && self.offset <= self.end(r) {
                        no = r;
                        continue 'external;
                    }
                }
            }
            for e in self.entities(no) {
                match e {
                    Entity::Tok(i) => {
                        if !self.candidate_tok(i) {
                            continue;
                        }
                        if let Some(c) = self.containing_comment(self.span_tok(i).start, false) {
                            return match self.documentation_comment(no, c) {
                                Some(d) => Target { no: d, entity: Some(Entity::Comment(c.0, c.1)), comment_text: false },
                                None => Target { no: 0, entity: Some(Entity::Comment(c.0, c.1)), comment_text: true },
                            };
                        }
                        return Target { no, entity: Some(e), comment_text: false };
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
                                return match self.documentation_comment(no, c) {
                                    Some(d) => Target { no: d, entity: Some(Entity::Comment(c.0, c.1)), comment_text: false },
                                    None => Target { no: 0, entity: Some(Entity::Comment(c.0, c.1)), comment_text: true },
                                };
                            }
                            return Target { no, entity: Some(e), comment_text: false };
                        }
                        no = f;
                        continue 'external;
                    }
                    Entity::Comment(..) => {}
                }
            }
            break;
        }
        if no == 0
            && let Some(c) = self.containing_comment(self.source.len(), true)
        {
            return Target { no: 0, entity: Some(Entity::Comment(c.0, c.1)), comment_text: true };
        }
        Target { no, entity: None, comment_text: false }
    }

    /// O token de início de uma entidade.
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

    /// `containingNode.findPrevious(token)`: o token anterior, se ainda é
    /// do nó.
    fn previous_in_node(&self, no: usize, i: usize) -> Option<usize> {
        let j = i.checked_sub(1)?;
        (self.span_tok(j).start >= self.start(no)).then_some(j)
    }

    /// `_computeDroppedToken`: um token de palavra fora da árvore entre o
    /// filho anterior à entidade e a entidade, que contém o cursor.
    pub(super) fn dropped_token(&self, target: &Target) -> Option<usize> {
        let e = target.entity?;
        let start_and = match e {
            Entity::No(n) => self.start(n),
            Entity::Tok(i) => self.span_tok(i).start,
            Entity::Comment(a, _) => a,
        };
        let previous: Vec<usize> = self.children(target.no).iter().copied().filter(|&f| self.end(f) <= start_and && self.kind(f) != "Comment").collect();
        let last = *previous.last()?;
        let mut i = self.last_token(last)? + 1;
        while let Some(t) = self.toks.get(i) {
            if t.span.start >= start_and || t.kind == Kind::Eof {
                break;
            }
            let inside = self.children(target.no).iter().any(|&f| self.start(f) <= t.span.start && t.span.end <= self.end(f));
            if !inside && self.word(i) && t.span.start <= self.offset && self.offset <= t.span.end {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// `computeReplacementRange`: `(início, length)`.
    pub(super) fn replacement_range(&self, target: &Target) -> (usize, usize) {
        let mut token = self.dropped_token(target).or_else(|| target.entity.and_then(|e| self.entity_token(e)));
        if let Some(t) = token
            && self.offset < self.span_tok(t).start
        {
            token = self.previous_in_node(target.no, t);
        }
        if let Some(t) = token {
            let mut t = Some(t);
            if let Some(x) = t
                && self.offset == self.span_tok(x).start
                && !self.word(x)
            {
                t = self.previous_in_node(target.no, x);
            }
            if let Some(x) = t {
                let s = self.span_tok(x);
                if self.word(x) && s.start <= self.offset && self.offset <= s.end {
                    return (s.start, s.end - s.start);
                }
                if matches!(self.toks[x].kind, Kind::Str(_)) {
                    // A URI de uma diretiva (ou da configuração dela).
                    let uri = match self.kind(target.no) {
                        "ImportDirective" | "ExportDirective" => self
                            .children(target.no)
                            .iter()
                            .copied()
                            .find(|&k| self.kind(k) == "SimpleStringLiteral" && self.start(k) <= self.offset && self.offset <= self.end(k))
                            .or_else(|| {
                                self.children_of(target.no, &["Configuration"])
                                    .into_iter()
                                    .flat_map(|c| self.children_of(c, &["SimpleStringLiteral"]))
                                    .find(|&k| self.start(k) <= self.offset && self.offset <= self.end(k))
                            }),
                        "SimpleStringLiteral" => {
                            let parent = self.parent(target.no);
                            let directive = parent.filter(|&p| self.is_directive(p) || (self.kind(p) == "Configuration" && self.parent(p).is_some_and(|d| self.is_directive(d))));
                            directive.map(|_| target.no)
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

    /// `contentsEnd`: antes da aspa de fechamento (se ela está).
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

    /// `TokenData.fromSelection`: o prefixo do casador.
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

    /// `targetPrefix`: o prefixo do pedido (o filtro do handler).
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
                let uri = (target.no != 0 && self.kind(target.no) == "SimpleStringLiteral" && self.parent(target.no).is_some_and(|p| self.is_directive(p))).then_some(target.no);
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
            // O primeiro `childEntity`: um nó desce; um token encerra.
            match self.entities(n).first().copied() {
                Some(Entity::No(f)) => n = f,
                Some(Entity::Tok(i)) => return of_token(i),
                _ => return String::new(),
            }
        }
    }

    /// `inConstantContext` do pedido: a entidade é expressão em contexto
    /// constante.
    pub(super) fn request_in_constant_context(&self, target: &Target) -> bool {
        match target.entity {
            Some(Entity::No(n)) => super::isp::EXPRESSION_KINDS.contains(&self.kind(n)) && self.constant_context(n),
            _ => false,
        }
    }

    /// O nó `Op` de uma entidade token (para os testes de vírgula e `)`).
    pub(super) fn entity_and_op(&self, e: Option<Entity>, op: Op) -> bool {
        matches!(e, Some(Entity::Tok(i)) if self.is_op(i, op))
    }
}
