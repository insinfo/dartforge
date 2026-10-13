//! Compilation unit, directives and top-level and member declarations
//! (fasta's `parseUnit`, `parseTopLevelDeclarationImpl`,
//! `parseTopLevelKeywordDeclaration`, `parseTopLevelMemberImpl`,
//! `parseClassOrMixinOrExtensionOrEnumMemberImpl`, `parseMethod`,
//! `parseFields`, `parseFactoryMethod`, `parseInitializers`, `parseEnum`,
//! `parseTypedef`, `parseImport`...), with the `AstBuilder` nodes.

use dartforge_frontend::token::{Keyword, Op};

use super::super::tree::{Node, NodeId};
use super::super::token::{TokenKind, Tk};
use super::lookahead::Shape;
use super::types::MemberKind;
use super::{Parser, R};

/// `DirectiveState` do fasta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum State {
    Unknown,
    Script,
    Library,
    ImportExport,
    Part,
    PartOf,
    Declarations,
}

/// `DeclarationKind` of the members.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Body {
    Class,
    Mixin,
    Extension,
    ExtensionType,
    Enum,
}

/// Modifiers read (`ModifierContext`).
#[derive(Debug, Default, Clone, Copy)]
struct Modifiers {
    abstract_: Option<usize>,
    augment: Option<usize>,
    const_: Option<usize>,
    covariant: Option<usize>,
    external: Option<usize>,
    final_: Option<usize>,
    late: Option<usize>,
    required: Option<usize>,
    static_: Option<usize>,
    var: Option<usize>,
}

impl Modifiers {
    fn vfc(&self) -> Option<usize> {
        self.var.or(self.final_).or(self.const_)
    }
}

fn tk(o: Option<usize>) -> Option<Tk> {
    o.map(|i| i as Tk)
}

impl Parser<'_> {
    /// `parseUnit`.
    pub(super) fn unit(&mut self) -> R<NodeId> {
        let mut state = State::Unknown;
        let mut script_tag = None;
        if self.kind_(0) == TokenKind::ScriptTag {
            script_tag = Some(0);
            state = State::Script;
            self.pos = 1;
        }
        let mut directives = Vec::new();
        let mut declarations = Vec::new();
        while !self.eof(self.pos) {
            let (directive, id) = self.top_level_declaration(&mut state)?;
            if directive {
                directives.push(id);
            } else {
                declarations.push(id);
            }
        }
        let eof = self.pos;
        Ok(self.node_between(
            Node::CompilationUnit {
                script_tag,
                directives,
                declarations,
            },
            0,
            eof,
        ))
    }

    fn top_level_word(&self, i: usize) -> bool {
        self.kw(i, Keyword::Class)
            || self.kw(i, Keyword::Enum)
            || (self.ident(i)
                && matches!(
                    self.txt(i),
                    "export" | "extension" | "import" | "library" | "mixin" | "part" | "typedef"
                ))
    }

    fn mark_declaration(state: &mut State) {
        if *state != State::PartOf {
            *state = State::Declarations;
        }
    }

    /// `parseTopLevelDeclarationImpl`.
    fn top_level_declaration(&mut self, state: &mut State) -> R<(bool, NodeId)> {
        let start_ = self.pos;
        let meta = self.metadata_()?;
        let next = self.pos;
        if self.top_level_word(next) {
            return self.word_declaration(start_, meta, next, next, [None; 4], state);
        }
        let beginning = next;
        let mut k = next;
        if self.modifier(next) {
            let next_class = |p: &Self, i: usize| {
                p.kw(i, Keyword::Class) || p.word(i, "mixin") || p.kw(i, Keyword::Enum)
            };
            if self.kw(next, Keyword::Var)
                || self.word(next, "late")
                || (self.kw(next, Keyword::Final) && !next_class(self, next + 1))
                || (self.kw(next, Keyword::Const) && !self.kw(next + 1, Keyword::Class))
            {
                Self::mark_declaration(state);
                return Ok((false, self.top_level_member(start_, meta)?));
            }
            while self.modifier(k) {
                k += 1;
            }
        }
        let mut extras: [Option<usize>; 4] = [None; 4]; // macro, sealed, base, interface
        let mut modifiers_start = beginning;
        if self.word(k, "macro") && self.kw(k + 1, Keyword::Class) {
            // The `macros` experiment is not enabled in 3.6: the `AstBuilder` reports
            // `EXPERIMENT_NOT_ENABLED`, which is a syntax error.
            return Err(self.error_at(
                k,
                "This requires the 'macros' language feature to be enabled.",
            ));
        }
        if false {
            extras[0] = Some(k);
            k += 1;
        } else if self.word(k, "sealed") {
            extras[1] = Some(k);
            if self.kw(k + 1, Keyword::Class)
                || self.word(k + 1, "mixin")
                || self.kw(k + 1, Keyword::Enum)
            {
                k += 1;
            } else if self.word(k + 1, "abstract") && self.kw(k + 2, Keyword::Class) {
                modifiers_start = k + 1;
                k += 2;
            }
        } else if self.word(k, "base") {
            extras[2] = Some(k);
            if self.kw(k + 1, Keyword::Class)
                || self.word(k + 1, "mixin")
                || self.kw(k + 1, Keyword::Enum)
            {
                k += 1;
            }
        } else if self.word(k, "interface") {
            extras[3] = Some(k);
            if self.kw(k + 1, Keyword::Class)
                || self.word(k + 1, "mixin")
                || self.kw(k + 1, Keyword::Enum)
            {
                k += 1;
            }
        }
        if self.top_level_word(k) {
            return self.word_declaration(start_, meta, modifiers_start, k, extras, state);
        }
        if self.word_(k) || k != beginning || self.op(k, Op::LParen) {
            Self::mark_declaration(state);
            return Ok((false, self.top_level_member(start_, meta)?));
        }
        Err(self.error_at(
            k,
            &format!("Expected a declaration, but got '{}'.", self.txt(k)),
        ))
    }

    /// Modifiers from `start` up to `end` (exclusive) accepted in a top-level
    /// keyword declaration.
    fn top_level_modifiers(&mut self, start: usize, end: usize, allowed: &[&str]) -> R<Modifiers> {
        let salvo = self.pos;
        self.pos = start;
        let mut m = Modifiers::default();
        let r = self.collect_modifiers(&mut m);
        let reached = self.pos;
        self.pos = salvo;
        r?;
        for i in start..reached {
            if !allowed.contains(&self.txt(i)) {
                return Err(
                    self.error_at(i, &format!("Can't have modifier '{}' here.", self.txt(i)))
                );
            }
        }
        if reached != end {
            return Err(self.error_at(reached, "Unexpected token."));
        }
        Ok(m)
    }

    /// `parseTopLevelKeywordDeclaration`.
    fn word_declaration(
        &mut self,
        start_: usize,
        meta: Vec<NodeId>,
        modifiers_start: usize,
        kw: usize,
        extras: [Option<usize>; 4],
        state: &mut State,
    ) -> R<(bool, NodeId)> {
        let [macro_, sealed, base, interface] = extras;
        if self.version < (3, 0)
            && let Some(x) = sealed.or(base).or(interface)
        {
            return Err(self.error_at(
                x,
                "This requires the 'class-modifiers' language feature to be enabled.",
            ));
        }
        // The separately recognized modifiers sit among the common ones.
        let modifiers_end = [macro_, sealed, base, interface]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(kw)
            .max(modifiers_start);
        let text = self.txt(kw);
        if self.kw(kw, Keyword::Class) {
            Self::mark_declaration(state);
            let m = self.top_level_modifiers(modifiers_start, modifiers_end, &["abstract", "augment", "final"])?;
            self.pos = kw;
            return Ok((false, self.class(start_, meta, m, extras, None)?));
        }
        if self.kw(kw, Keyword::Enum) {
            Self::mark_declaration(state);
            if base.is_some() || sealed.is_some() || interface.is_some() {
                return Err(self.error_at(
                    kw,
                    "Enums can't be declared to be 'base', 'sealed' or 'interface'.",
                ));
            }
            let m = self.top_level_modifiers(modifiers_start, modifiers_end, &["augment"])?;
            self.pos = kw;
            return Ok((false, self.enumeration(start_, meta, m)?));
        }
        let nv = kw + 1;
        let mut record_typedef = false;
        if text == "typedef" && self.op(nv, Op::LParen) {
            let f = self.pair(nv) + 1;
            if self.ident(f) || (self.op(f, Op::Question) && self.ident(f + 1)) {
                let t = self.type_at(nv, false, false, false);
                if t.shape == Shape::Complex && t.record {
                    record_typedef = true;
                }
            }
        }
        let member = |p: &mut Self, state: &mut State, meta: Vec<NodeId>| -> R<(bool, NodeId)> {
            Self::mark_declaration(state);
            p.pos = modifiers_start;
            Ok((false, p.top_level_member(start_, meta)?))
        };
        if (self.op(nv, Op::LParen) || self.op(nv, Op::Dot)) && !record_typedef {
            return member(self, state, meta);
        }
        if self.op(nv, Op::Lt) {
            if text == "extension"
                && let Some(a) = self.type_args_lt(nv, true)
                && self.word(a.end_index, "on")
            {
                Self::mark_declaration(state);
                let m = self.top_level_modifiers(modifiers_start, kw, &["augment"])?;
                self.pos = kw;
                return Ok((false, self.extension(start_, meta, m)?));
            }
            return member(self, state, meta);
        }
        if base.is_some() && text != "mixin"
            || sealed.is_some()
            || interface.is_some()
            || macro_.is_some()
        {
            return Err(self.error_at(kw, "Unexpected modifier."));
        }
        match text {
            "import" | "export" | "part" | "library" => {
                self.top_level_modifiers(modifiers_start, kw, &[])?;
                self.pos = kw;
                let d = match text {
                    "import" => {
                        self.check_import_export(state, kw)?;
                        self.import(start_, meta)?
                    }
                    "export" => {
                        self.check_import_export(state, kw)?;
                        self.export(start_, meta)?
                    }
                    "part" => self.part(start_, meta, state)?,
                    _ => {
                        if *state < State::Library {
                            *state = State::Library;
                        } else {
                            return Err(self.error_at(
                                kw,
                                "The library directive must appear before all other directives.",
                            ));
                        }
                        self.library(start_, meta)?
                    }
                };
                Ok((true, d))
            }
            "typedef" => {
                Self::mark_declaration(state);
                let m = self.top_level_modifiers(modifiers_start, kw, &["augment"])?;
                self.pos = kw;
                Ok((false, self.alias(start_, meta, m)?))
            }
            "mixin" => {
                if self.kw(nv, Keyword::Class) {
                    Self::mark_declaration(state);
                    if sealed.is_some() || interface.is_some() {
                        return Err(self.error_at(
                            kw,
                            "A mixin class can't be declared 'sealed' or 'interface'.",
                        ));
                    }
                    let m = self.top_level_modifiers(modifiers_start, modifiers_end, &["abstract", "augment"])?;
                    self.pos = nv;
                    return Ok((false, self.class(start_, meta, m, extras, Some(kw))?));
                }
                Self::mark_declaration(state);
                let m = self.top_level_modifiers(modifiers_start, modifiers_end, &["augment"])?;
                self.pos = kw;
                Ok((false, self.mixin(start_, meta, m, base)?))
            }
            "extension" => {
                Self::mark_declaration(state);
                let m = self.top_level_modifiers(modifiers_start, kw, &["augment"])?;
                self.pos = kw;
                Ok((false, self.extension(start_, meta, m)?))
            }
            _ => Err(self.error_at(kw, "Unexpected token.")),
        }
    }

    fn check_import_export(&self, state: &mut State, kw: usize) -> R<()> {
        match *state {
            State::Unknown | State::Script | State::Library | State::ImportExport => {
                *state = State::ImportExport;
                Ok(())
            }
            State::Part => Err(self.error_at(
                kw,
                "Import and export directives must come before part directives.",
            )),
            State::PartOf => Err(self.error_at(
                kw,
                "The part-of directive must be the only directive in a part.",
            )),
            State::Declarations => {
                Err(self.error_at(kw, "Directives must appear before any declarations."))
            }
        }
    }

    /// URI: a string literal (`ensureLiteralString`).
    fn uri(&mut self) -> R<NodeId> {
        if !self.text(self.pos) {
            return Err(self.error("Expected a string literal."));
        }
        self.string_literal()
    }

    /// `parseLibraryName`.
    fn library(&mut self, start_: usize, metadata: Vec<NodeId>) -> R<NodeId> {
        let l = self.advance();
        let name2 = if self.op(self.pos, Op::Semicolon) {
            self.feature(l, (2, 19), "unnamed-libraries");
            None
        } else {
            Some(self.qualified_name(true)?)
        };
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::LibraryDirective {
                metadata,
                library_keyword: l as Tk,
                name2,
                semicolon: s as Tk,
            },
            start_,
        ))
    }

    /// `a.b.c` as a [`Node::LibraryIdentifier`] (or [`Node::DottedName`]).
    fn qualified_name(&mut self, library: bool) -> R<NodeId> {
        let start_ = self.pos;
        let mut components = Vec::new();
        loop {
            let i = self.expect_ident()?;
            components.push(self.simple(i));
            if !self.op(self.pos, Op::Dot) {
                break;
            }
            self.advance();
        }
        let node = if library {
            Node::LibraryIdentifier { components }
        } else {
            Node::DottedName { components }
        };
        Ok(self.node(node, start_))
    }

    /// `parseConditionalUriStar`.
    fn configurations_(&mut self, v: &mut Vec<NodeId>) -> R<()> {
        while self.kw(self.pos, Keyword::If) {
            let i = self.advance();
            let lp = self.expect_op(Op::LParen)?;
            let name = self.qualified_name(false)?;
            let (equal_token, value) = if self.op(self.pos, Op::EqEq) {
                let e = self.advance();
                (Some(e as Tk), Some(self.uri()?))
            } else {
                (None, None)
            };
            let rp = self.expect_op(Op::RParen)?;
            let uri = self.uri()?;
            v.push(self.node(
                Node::Configuration {
                    if_keyword: i as Tk,
                    left_parenthesis: lp as Tk,
                    name,
                    equal_token,
                    value,
                    right_parenthesis: rp as Tk,
                    uri,
                },
                i,
            ));
        }
        Ok(())
    }

    /// `parseCombinatorStar`.
    fn combinators_(&mut self, v: &mut Vec<NodeId>) -> R<bool> {
        let mut any_ = false;
        while self.word(self.pos, "show") || self.word(self.pos, "hide") {
            let k = self.advance();
            let mut names = Vec::new();
            loop {
                let n = self.expect_ident()?;
                names.push(self.simple(n));
                if !self.op(self.pos, Op::Comma) {
                    break;
                }
                self.advance();
            }
            let node = if self.txt(k) == "show" {
                Node::ShowCombinator {
                    keyword: k as Tk,
                    shown_names: names,
                }
            } else {
                Node::HideCombinator {
                    keyword: k as Tk,
                    hidden_names: names,
                }
            };
            v.push(self.node(node, k));
            any_ = true;
        }
        Ok(any_)
    }

    /// `parseImport` with the recovery of out-of-order clauses
    /// (`parseImportRecovery`) in the cases fasta accepts without error.
    fn import(&mut self, start_: usize, metadata: Vec<NodeId>) -> R<NodeId> {
        let i = self.advance();
        if self.word(self.pos, "augment") {
            return Err(self.error("Augmentation imports aren't supported."));
        }
        let uri = self.uri()?;
        let mut configurations = Vec::new();
        let mut combinators = Vec::new();
        let mut deferred_keyword = None;
        let mut as_keyword = None;
        let mut prefix = None;
        let mut has_combinator = false;
        loop {
            let start = self.pos;
            self.configurations_(&mut configurations)?;
            if self.word(self.pos, "deferred") && !self.word(self.pos + 1, "as") {
                if deferred_keyword.is_some() {
                    return Err(
                        self.error("An import directive can only have one 'deferred' keyword.")
                    );
                }
                deferred_keyword = Some(self.advance() as Tk);
            } else if self.word(self.pos, "as")
                || (self.word(self.pos, "deferred") && self.word(self.pos + 1, "as"))
            {
                if self.word(self.pos, "deferred") {
                    if deferred_keyword.is_some() {
                        return Err(
                            self.error("An import directive can only have one 'deferred' keyword.")
                        );
                    }
                    if as_keyword.is_some() {
                        return Err(self.error("The deferred keyword should come immediately before the prefix ('as' clause)."));
                    }
                    deferred_keyword = Some(self.advance() as Tk);
                }
                if as_keyword.is_some() {
                    return Err(
                        self.error("An import directive can only have one prefix ('as' clause).")
                    );
                }
                if has_combinator {
                    return Err(self.error(
                        "The prefix ('as' clause) should come before any show/hide combinators.",
                    ));
                }
                as_keyword = Some(self.advance() as Tk);
                let n = self.expect_ident()?;
                prefix = Some(self.simple(n));
            }
            has_combinator |= self.combinators_(&mut combinators)?;
            if self.op(self.pos, Op::Semicolon) {
                break;
            }
            if self.pos == start {
                return Err(self.expected(";"));
            }
        }
        if deferred_keyword.is_some() && as_keyword.is_none() {
            return Err(self.error("Deferred imports should have a prefix."));
        }
        let s = self.advance();
        Ok(self.node(
            Node::ImportDirective {
                metadata,
                import_keyword: i as Tk,
                uri,
                configurations,
                deferred_keyword,
                as_keyword,
                prefix,
                combinators,
                semicolon: s as Tk,
            },
            start_,
        ))
    }

    /// `parseExport`.
    fn export(&mut self, start_: usize, metadata: Vec<NodeId>) -> R<NodeId> {
        let e = self.advance();
        let uri = self.uri()?;
        let mut configurations = Vec::new();
        self.configurations_(&mut configurations)?;
        let mut combinators = Vec::new();
        self.combinators_(&mut combinators)?;
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::ExportDirective {
                metadata,
                export_keyword: e as Tk,
                uri,
                configurations,
                combinators,
                semicolon: s as Tk,
            },
            start_,
        ))
    }

    /// `parsePartOrPartOf`.
    fn part(&mut self, start_: usize, metadata: Vec<NodeId>, state: &mut State) -> R<NodeId> {
        let p = self.advance();
        if self.word(self.pos, "of") {
            if *state != State::Unknown {
                return Err(self.error_at(
                    p,
                    "The part-of directive must be the only directive in a part.",
                ));
            }
            *state = State::PartOf;
            let o = self.advance();
            let (library_name, uri) = if self.ident(self.pos) {
                (Some(self.qualified_name(true)?), None)
            } else {
                (None, Some(self.uri()?))
            };
            let s = self.expect_op(Op::Semicolon)?;
            return Ok(self.node(
                Node::PartOfDirective {
                    metadata,
                    part_keyword: p as Tk,
                    of_keyword: o as Tk,
                    library_name,
                    uri,
                    semicolon: s as Tk,
                },
                start_,
            ));
        }
        match *state {
            State::PartOf => {
                return Err(self.error_at(
                    p,
                    "The part-of directive must be the only directive in a part.",
                ));
            }
            State::Declarations => {
                return Err(self.error_at(p, "Directives must appear before any declarations."));
            }
            _ => *state = State::Part,
        }
        let uri = self.uri()?;
        if self.kw(self.pos, Keyword::If) {
            return Err(self.error("Part directives can't have configurations."));
        }
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::PartDirective {
                metadata,
                part_keyword: p as Tk,
                uri,
                semicolon: s as Tk,
            },
            start_,
        ))
    }

    /// Port of `ModifierContext._parseModifiers`: reads modifiers in any order,
    /// with fasta's order and conflict errors.
    fn collect_modifiers(&mut self, m: &mut Modifiers) -> R<()> {
        while self.modifier(self.pos) {
            let i = self.pos;
            let fora = |p: &Self| {
                Err(p.error_at(i, &format!("The modifier '{}' is out of order.", p.txt(i))))
            };
            let conflict =
                |p: &Self| Err(p.error_at(i, &format!("Conflicting modifier '{}'.", p.txt(i))));
            match self.txt(i) {
                "abstract" => {
                    if m.abstract_.is_some() {
                        return conflict(self);
                    }
                    if m.vfc().is_some() || m.covariant.is_some() {
                        return fora(self);
                    }
                    m.abstract_ = Some(i);
                }
                "augment" => {
                    if m.augment.is_some() || m.external.is_some() {
                        return conflict(self);
                    }
                    if m.vfc().is_some()
                        || m.abstract_.is_some()
                        || m.covariant.is_some()
                        || m.late.is_some()
                        || m.static_.is_some()
                    {
                        return fora(self);
                    }
                    m.augment = Some(i);
                }
                "const" => {
                    if m.vfc().is_some() || m.covariant.is_some() || m.late.is_some() {
                        return conflict(self);
                    }
                    m.const_ = Some(i);
                }
                "covariant" => {
                    if m.const_.is_some() || m.covariant.is_some() || m.static_.is_some() {
                        return conflict(self);
                    }
                    if m.var.is_some() || m.final_.is_some() || m.late.is_some() {
                        return fora(self);
                    }
                    m.covariant = Some(i);
                }
                "external" => {
                    if m.external.is_some() || m.augment.is_some() {
                        return conflict(self);
                    }
                    if m.const_.is_some()
                        || m.static_.is_some()
                        || m.late.is_some()
                        || m.vfc().is_some()
                        || m.covariant.is_some()
                    {
                        return fora(self);
                    }
                    m.external = Some(i);
                }
                "final" => {
                    if m.vfc().is_some() {
                        return conflict(self);
                    }
                    m.final_ = Some(i);
                }
                "late" => {
                    if m.late.is_some() || m.const_.is_some() {
                        return conflict(self);
                    }
                    if m.var.is_some() || m.final_.is_some() {
                        return fora(self);
                    }
                    m.late = Some(i);
                }
                "required" => {
                    if m.required.is_some() {
                        return conflict(self);
                    }
                    if m.const_.is_some()
                        || m.covariant.is_some()
                        || m.final_.is_some()
                        || m.var.is_some()
                    {
                        return fora(self);
                    }
                    m.required = Some(i);
                }
                "static" => {
                    if m.covariant.is_some() || m.static_.is_some() {
                        return conflict(self);
                    }
                    if m.vfc().is_some() || m.late.is_some() {
                        return fora(self);
                    }
                    m.static_ = Some(i);
                }
                "var" => {
                    if m.vfc().is_some() {
                        return conflict(self);
                    }
                    m.var = Some(i);
                }
                _ => return Err(self.error_at(i, "Unexpected modifier.")),
            }
            self.advance();
        }
        Ok(())
    }

    /// `parseClassOrNamedMixinApplication`; `self.pos` is at the `class`.
    fn class(
        &mut self,
        start_: usize,
        metadata: Vec<NodeId>,
        m: Modifiers,
        extras: [Option<usize>; 4],
        mixin: Option<usize>,
    ) -> R<NodeId> {
        let [macro_, sealed, base, interface] = extras;
        if self.version < (3, 0) && (m.final_.is_some() || mixin.is_some()) {
            return Err(
                self.error("This requires the 'class-modifiers' language feature to be enabled.")
            );
        }
        let c = self.expect_kw(Keyword::Class)?;
        let name = self.expect_ident()?;
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.type_params()?)
        } else {
            None
        };
        if m.abstract_.is_some() {
            if sealed.is_some() {
                return Err(self.error_at(sealed.unwrap_or(c), "A 'sealed' class can't be marked 'abstract' because it's already implicitly abstract."));
            }
            if m.final_.is_some() && (base.is_some() || interface.is_some()) {
                return Err(self.error_at(c, "An 'abstract' class can't be declared as both 'final' and 'base' or 'interface'."));
            }
        }
        if self.op(self.pos, Op::Assign) {
            let equals = self.advance();
            let superclass = self.required_named_type()?;
            if !self.kw(self.pos, Keyword::With) {
                return Err(self.expected("with"));
            }
            let with_clause = Some(self.list_clause(Keyword::With)?);
            let implements_clause = self.implements_clause_()?;
            let s = self.expect_op(Op::Semicolon)?;
            return Ok(self.node(
                Node::ClassTypeAlias {
                    metadata,
                    augment_keyword: tk(m.augment),
                    abstract_keyword: tk(m.abstract_),
                    macro_keyword: tk(macro_),
                    sealed_keyword: tk(sealed),
                    base_keyword: tk(base),
                    interface_keyword: tk(interface),
                    final_keyword: tk(m.final_),
                    mixin_keyword: tk(mixin),
                    typedef_keyword: c as Tk,
                    name: name as Tk,
                    type_parameters,
                    equals: equals as Tk,
                    superclass,
                    with_clause,
                    implements_clause,
                    semicolon: s as Tk,
                },
                start_,
            ));
        }
        let extends_clause = if self.kw(self.pos, Keyword::Extends) {
            let e = self.advance();
            let superclass = self.required_named_type()?;
            if self.op(self.pos, Op::Comma) {
                return Err(self.error("Each class definition can have at most one extends clause."));
            }
            Some(self.node(
                Node::ExtendsClause {
                    extends_keyword: e as Tk,
                    superclass,
                },
                e,
            ))
        } else {
            None
        };
        let with_clause = if self.kw(self.pos, Keyword::With) {
            Some(self.list_clause(Keyword::With)?)
        } else {
            None
        };
        let implements_clause = self.implements_clause_()?;
        let native_clause = if self.word(self.pos, "native") {
            let n = self.advance();
            let name = if self.text(self.pos) {
                Some(self.string_literal()?)
            } else {
                None
            };
            Some(self.node(
                Node::NativeClause {
                    native_keyword: n as Tk,
                    name,
                },
                n,
            ))
        } else {
            None
        };
        let name_ = self.txt(name);
        let (lb, members, rb) = self.class_body(Body::Class, name_)?;
        Ok(self.node(
            Node::ClassDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                abstract_keyword: tk(m.abstract_),
                macro_keyword: tk(macro_),
                sealed_keyword: tk(sealed),
                base_keyword: tk(base),
                interface_keyword: tk(interface),
                final_keyword: tk(m.final_),
                mixin_keyword: tk(mixin),
                class_keyword: c as Tk,
                name: name as Tk,
                type_parameters,
                extends_clause,
                with_clause,
                implements_clause,
                native_clause,
                left_bracket: lb as Tk,
                members,
                right_bracket: rb as Tk,
            },
            start_,
        ))
    }

    /// `with T, U` ([`Node::WithClause`]).
    fn list_clause(&mut self, _k: Keyword) -> R<NodeId> {
        let w = self.advance();
        let mut types_ = Vec::new();
        loop {
            types_.push(self.required_named_type()?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        Ok(self.node(
            Node::WithClause {
                with_keyword: w as Tk,
                mixin_types: types_,
            },
            w,
        ))
    }

    /// Optional `implements T, U`.
    fn implements_clause_(&mut self) -> R<Option<NodeId>> {
        if !self.word(self.pos, "implements") {
            return Ok(None);
        }
        let i = self.advance();
        let mut types_ = Vec::new();
        loop {
            types_.push(self.required_named_type()?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        Ok(Some(self.node(
            Node::ImplementsClause {
                implements_keyword: i as Tk,
                interfaces: types_,
            },
            i,
        )))
    }

    /// `{ members_ }`.
    fn class_body(&mut self, body_: Body, name_: &str) -> R<(usize, Vec<NodeId>, usize)> {
        if !self.op(self.pos, Op::LBrace) {
            return Err(self.expected("{"));
        }
        let lb = self.advance();
        let mut members_ = Vec::new();
        while !self.op(self.pos, Op::RBrace) {
            if self.eof(self.pos) {
                return Err(self.expected("}"));
            }
            members_.push(self.member(body_, name_)?);
        }
        let rb = self.advance();
        Ok((lb, members_, rb))
    }

    /// `parseMixin`; `self.pos` is at the `mixin`.
    fn mixin(&mut self, start_: usize, metadata: Vec<NodeId>, m: Modifiers, base: Option<usize>) -> R<NodeId> {
        if m.final_.is_some() {
            return Err(self.error_at(
                m.final_.unwrap_or(start_),
                "A mixin can't be declared 'final'.",
            ));
        }
        let mk = self.advance();
        let name = self.expect_ident()?;
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.type_params()?)
        } else {
            None
        };
        let on_clause = if self.word(self.pos, "on") {
            let o = self.advance();
            let mut types_ = Vec::new();
            loop {
                types_.push(self.required_named_type()?);
                if !self.op(self.pos, Op::Comma) {
                    break;
                }
                self.advance();
            }
            Some(self.node(
                Node::MixinOnClause {
                    on_keyword: o as Tk,
                    superclass_constraints: types_,
                },
                o,
            ))
        } else {
            None
        };
        let implements_clause = self.implements_clause_()?;
        let name_ = self.txt(name);
        let (lb, members, rb) = self.class_body(Body::Mixin, name_)?;
        Ok(self.node(
            Node::MixinDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                base_keyword: tk(base),
                mixin_keyword: mk as Tk,
                name: name as Tk,
                type_parameters,
                on_clause,
                implements_clause,
                left_bracket: lb as Tk,
                members,
                right_bracket: rb as Tk,
            },
            start_,
        ))
    }

    /// `parseExtension`; `self.pos` is at the `extension`.
    fn extension(&mut self, start_: usize, metadata: Vec<NodeId>, m: Modifiers) -> R<NodeId> {
        let e = self.advance();
        self.feature(e, (2, 6), "extension-methods");
        if self.word(self.pos, "type")
            && !self.word(self.pos + 1, "on")
            && !self.op(self.pos + 1, Op::Lt)
            || self.word(self.pos, "type") && self.kw(self.pos + 1, Keyword::Const)
        {
            return self.extension_type(start_, metadata, m, e);
        }
        let name = if self.ident(self.pos) && !self.word(self.pos, "on") {
            Some(self.advance())
        } else {
            None
        };
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.type_params()?)
        } else {
            None
        };
        let o = self.expect_word("on")?;
        let extended_type = self.required_type()?;
        let on_clause = Some(self.node(
            Node::ExtensionOnClause {
                on_keyword: o as Tk,
                extended_type,
            },
            o,
        ));
        let name_ = name.map_or("", |n| self.txt(n));
        let (lb, members, rb) = self.class_body(Body::Extension, name_)?;
        Ok(self.node(
            Node::ExtensionDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                extension_keyword: e as Tk,
                name: tk(name),
                type_parameters,
                on_clause,
                left_bracket: lb as Tk,
                members,
                right_bracket: rb as Tk,
            },
            start_,
        ))
    }

    /// `parseExtensionTypeDeclaration`.
    fn extension_type(&mut self, start_: usize, metadata: Vec<NodeId>, m: Modifiers, e: usize) -> R<NodeId> {
        if self.version < (3, 3) {
            return Err(
                self.error("This requires the 'inline-class' language feature to be enabled.")
            );
        }
        let t = self.advance();
        let const_keyword = if self.kw(self.pos, Keyword::Const) {
            Some(self.advance() as Tk)
        } else {
            None
        };
        let name = self.expect_ident()?;
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.type_params()?)
        } else {
            None
        };
        let ri = self.pos;
        let constructor_name = if self.op(self.pos, Op::Dot) {
            let p = self.advance();
            if !(self.ident(self.pos) || self.kw(self.pos, Keyword::New)) {
                return Err(self.error("Expected an identifier."));
            }
            let n = self.advance();
            Some(self.node(
                Node::RepresentationConstructorName {
                    period: p as Tk,
                    name: n as Tk,
                },
                p,
            ))
        } else {
            None
        };
        let lp = self.expect_op(Op::LParen)?;
        let field_metadata = self.metadata_()?;
        if self.modifier(self.pos) {
            return Err(self.error("Representation fields can't have modifiers."));
        }
        let ti = self.type_at(self.pos, false, false, true);
        if ti.is_none() {
            return Err(self.error("Expected a representation type."));
        }
        let field_type = self.type_until(ti.end_index)?;
        let field_name = self.expect_ident()?;
        if self.op(self.pos, Op::Comma) {
            return Err(self.error("The representation field can't have a trailing comma."));
        }
        if self.txt(field_name) == self.txt(name) {
            return Err(self.error_at(
                field_name,
                "A class member can't have the same name as the enclosing class.",
            ));
        }
        let rp = self.expect_op(Op::RParen)?;
        let representation = self.node(
            Node::RepresentationDeclaration {
                constructor_name,
                left_parenthesis: lp as Tk,
                field_metadata,
                field_type,
                field_name: field_name as Tk,
                right_parenthesis: rp as Tk,
            },
            ri,
        );
        let implements_clause = self.implements_clause_()?;
        let name_ = self.txt(name);
        let (lb, members, rb) = self.class_body(Body::ExtensionType, name_)?;
        Ok(self.node(
            Node::ExtensionTypeDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                extension_keyword: e as Tk,
                type_keyword: t as Tk,
                const_keyword,
                name: name as Tk,
                type_parameters,
                representation,
                implements_clause,
                left_bracket: lb as Tk,
                members,
                right_bracket: rb as Tk,
            },
            start_,
        ))
    }

    /// `parseEnum`; `self.pos` is at the `enum`.
    fn enumeration(&mut self, start_: usize, metadata: Vec<NodeId>, m: Modifiers) -> R<NodeId> {
        let e = self.advance();
        let name = self.expect_ident()?;
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.type_params()?)
        } else {
            None
        };
        let with_clause = if self.kw(self.pos, Keyword::With) {
            Some(self.list_clause(Keyword::With)?)
        } else {
            None
        };
        let implements_clause = self.implements_clause_()?;
        let lb = self.expect_op(Op::LBrace)?;
        let mut constants = Vec::new();
        loop {
            if self.op(self.pos, Op::RBrace) || self.op(self.pos, Op::Semicolon) {
                break;
            }
            constants.push(self.enum_constant()?);
            if self.op(self.pos, Op::Comma) {
                self.advance();
            } else if self.op(self.pos, Op::RBrace) || self.op(self.pos, Op::Semicolon) {
                break;
            } else {
                return Err(self.expected("}"));
            }
        }
        if type_parameters.is_some()
            || with_clause.is_some()
            || implements_clause.is_some()
            || self.op(self.pos, Op::Semicolon)
        {
            self.feature(e, (2, 17), "enhanced-enums");
        }
        let mut semicolon = None;
        let mut members = Vec::new();
        if self.op(self.pos, Op::Semicolon) {
            semicolon = Some(self.advance() as Tk);
            let name_ = self.txt(name);
            while !self.op(self.pos, Op::RBrace) {
                if self.eof(self.pos) {
                    return Err(self.expected("}"));
                }
                members.push(self.member(Body::Enum, name_)?);
            }
        }
        let rb = self.expect_op(Op::RBrace)?;
        Ok(self.node(
            Node::EnumDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                enum_keyword: e as Tk,
                name: name as Tk,
                type_parameters,
                with_clause,
                implements_clause,
                left_bracket: lb as Tk,
                constants,
                semicolon,
                members,
                right_bracket: rb as Tk,
            },
            start_,
        ))
    }

    /// `parseEnumElement`.
    fn enum_constant(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        let metadata = self.metadata_()?;
        if self.word(self.pos, "augment") && self.ident(self.pos + 1) {
            return Err(self.error("Augmentations aren't supported here."));
        }
        let name = self.expect_ident()?;
        let ai = self.pos;
        let type_arguments = if self.op(self.pos, Op::Lt) {
            if self.type_args_at(name, false).is_none() {
                return Err(self.error("Invalid type arguments."));
            }
            Some(self.type_args()?)
        } else {
            None
        };
        let constructor_selector = if self.op(self.pos, Op::Dot) {
            let p = self.advance();
            if !(self.ident(self.pos) || self.kw(self.pos, Keyword::New)) {
                return Err(self.error("Expected an identifier."));
            }
            let n = self.advance();
            let s = self.simple(n);
            Some(self.node(
                Node::ConstructorSelector {
                    period: p as Tk,
                    name: s,
                },
                p,
            ))
        } else {
            None
        };
        let arguments = if self.op(self.pos, Op::LParen)
            || type_arguments.is_some()
            || constructor_selector.is_some()
        {
            self.feature(ai, (2, 17), "enhanced-enums");
            if !self.op(self.pos, Op::LParen) {
                return Err(self.expected("("));
            }
            let argument_list = self.arguments_()?;
            Some(self.node(
                Node::EnumConstantArguments {
                    type_arguments,
                    constructor_selector,
                    argument_list,
                },
                ai,
            ))
        } else {
            None
        };
        Ok(self.node(
            Node::EnumConstantDeclaration {
                metadata,
                name: name as Tk,
                arguments,
            },
            start_,
        ))
    }

    /// `parseTypedef`; `self.pos` is at the `typedef`.
    fn alias(&mut self, start_: usize, metadata: Vec<NodeId>, m: Modifiers) -> R<NodeId> {
        let td = self.advance();
        let ti = self.type_at(self.pos, false, false, false);
        let nx = ti.end_index;
        let after = self.type_args_at(nx, true).map_or(nx + 1, |a| a.end_index);
        if ti.is_none() && self.op(after, Op::Assign) {
            if !self.ident(self.pos) {
                return Err(self.error("Expected an identifier."));
            }
            let name = self.advance();
            let type_parameters = if self.op(self.pos, Op::Lt) {
                Some(self.type_params()?)
            } else {
                None
            };
            if self.built_in(name) {
                return Err(self.error_at(
                    name,
                    &format!(
                        "'{}' can't be used as an identifier because it's a keyword.",
                        self.txt(name)
                    ),
                ));
            }
            let equals = self.expect_op(Op::Assign)?;
            let type_ = self.required_type()?;
            if !matches!(self.tree_.node(type_), Node::GenericFunctionType { .. }) {
                self.feature(equals, (2, 13), "nonfunction-type-aliases");
            }
            let s = self.expect_op(Op::Semicolon)?;
            return Ok(self.node(
                Node::GenericTypeAlias {
                    metadata,
                    augment_keyword: tk(m.augment),
                    typedef_keyword: td as Tk,
                    name: name as Tk,
                    type_parameters,
                    equals: equals as Tk,
                    type_,
                    semicolon: s as Tk,
                },
                start_,
            ));
        }
        let return_type = if ti.is_none() {
            None
        } else {
            Some(self.type_until(ti.end_index)?)
        };
        let name = self.expect_ident()?;
        if self.built_in(name) {
            return Err(self.error_at(
                name,
                &format!(
                    "'{}' can't be used as an identifier because it's a keyword.",
                    self.txt(name)
                ),
            ));
        }
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.type_params()?)
        } else {
            None
        };
        if !self.op(self.pos, Op::LParen) {
            return Err(self.expected("("));
        }
        let parameters = self.formal_params(MemberKind::FunctionAlias)?;
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::FunctionTypeAlias {
                metadata,
                augment_keyword: tk(m.augment),
                typedef_keyword: td as Tk,
                return_type,
                name: name as Tk,
                type_parameters,
                parameters,
                semicolon: s as Tk,
            },
            start_,
        ))
    }

    /// `indicatesMethodOrField`.
    fn indicates_method_or_field(&self, i: usize) -> bool {
        matches!(
            self.kind_(i),
            TokenKind::Op(
                Op::Semicolon | Op::Assign | Op::LParen | Op::LBrace | Op::Arrow | Op::Lt
            )
        )
    }

    /// `parseTopLevelMemberImpl`.
    fn top_level_member(&mut self, start_: usize, metadata: Vec<NodeId>) -> R<NodeId> {
        let mut m = Modifiers::default();
        self.collect_modifiers(&mut m)?;
        for (x, name_) in [
            (m.abstract_, "abstract"),
            (m.covariant, "covariant"),
            (m.required, "required"),
            (m.static_, "static"),
        ] {
            if let Some(i) = x {
                return Err(self.error_at(i, &format!("Can't have modifier '{name_}' here.")));
            }
        }
        if m.vfc().is_some()
            && let Some(f) = self.skip_outer_pattern(self.pos)
            && self.op(f + 1, Op::Assign)
        {
            return Err(self.error(
                "Pattern variable declarations aren't allowed outside of functions or methods.",
            ));
        }
        let ti = self.type_at(self.pos, false, true, false);
        let mut nx = ti.end_index;
        let mut prop = None;
        if (self.word(nx, "get") || self.word(nx, "set")) && self.ident(nx + 1) {
            prop = Some(nx);
            nx += 1;
        }
        if ti.is_none()
            && m.vfc().is_none()
            && self.reserved(nx + 1)
            && self.indicates_method_or_field(nx + 2)
        {
            return Err(self.error_at(nx + 1, "Expected an identifier."));
        }
        if !self.plain_ident(nx) {
            let v = self.txt(nx);
            if v == "factory" || v == "operator" {
                if prop.is_none()
                    && !self.indicates_method_or_field(nx + 1)
                    && !self.op(nx + 1, Op::Comma)
                {
                    return Err(self.error_at(
                        nx,
                        "Top-level declarations can't be factories or operators.",
                    ));
                }
            } else if !self.ident(nx) {
                return Err(self.error_at(
                    nx,
                    &format!("Expected an identifier, but got '{}'.", self.txt(nx)),
                ));
            }
        }
        let after = nx + 1;
        if prop.is_some()
            || matches!(
                self.kind_(after),
                TokenKind::Op(Op::LParen | Op::LBrace | Op::Lt | Op::Dot | Op::Arrow)
            )
        {
            if let Some(v) = m.vfc().or(m.late) {
                return Err(self.error_at(
                    v,
                    "Functions can't be declared 'var', 'final', 'const' or 'late'.",
                ));
            }
            let return_type = if ti.is_none() {
                None
            } else {
                Some(self.type_until(ti.end_index)?)
            };
            if prop.is_some() {
                self.advance();
            }
            let name = self.expect_ident()?;
            let getter = prop.is_some_and(|p| self.txt(p) == "get");
            let fi = self.pos;
            let type_parameters = if prop.is_none() && self.op(self.pos, Op::Lt) {
                Some(self.type_params()?)
            } else {
                None
            };
            let parameters = if self.op(self.pos, Op::LParen) {
                if getter {
                    return Err(self.error("Getters must be declared without a parameter list."));
                }
                Some(self.formal_params(MemberKind::TopLevelFunction)?)
            } else if getter {
                None
            } else {
                return Err(self.error("Functions must have an explicit list of parameters."));
            };
            if m.external.is_some() && !self.op(self.pos, Op::Semicolon) {
                return Err(self.error("An external or native method can't have a body."));
            }
            let body = self.function_body_(false, m.external.is_some())?;
            let fe = self.node(
                Node::FunctionExpression {
                    type_parameters,
                    parameters,
                    body,
                },
                fi,
            );
            return Ok(self.node(
                Node::FunctionDeclaration {
                    metadata,
                    augment_keyword: tk(m.augment),
                    external_keyword: tk(m.external),
                    return_type,
                    property_keyword: tk(prop),
                    name: name as Tk,
                    function_expression: fe,
                },
                start_,
            ));
        }
        if prop.is_some() {
            return Err(self.error_at(nx, "Unexpected 'get'/'set'."));
        }
        let fields = self.fields_(&m, ti, None)?;
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::TopLevelVariableDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                external_keyword: tk(m.external),
                variables: fields,
                semicolon: s as Tk,
            },
            start_,
        ))
    }

    /// `parseFields` up to just before the `;`: the
    /// [`Node::VariableDeclarationList`].
    fn fields_(
        &mut self,
        m: &Modifiers,
        ti: super::lookahead::TypeInfo,
        class_name: Option<&str>,
    ) -> R<NodeId> {
        if let Some(c) = m.covariant
            && m.late.is_none()
            && m.final_.is_some()
        {
            return Err(self.error_at(
                c,
                "Members can't be declared to be both 'final' and 'covariant'.",
            ));
        }
        if ti.is_none() {
            if m.vfc().is_none() {
                return Err(self.error_at(ti.end_index, "Variables must be declared using the keywords 'const', 'final', 'var' or a type name."));
            }
        } else if let Some(v) = m.var {
            return Err(self.error_at(
                v,
                "Variables can't be declared using both 'var' and a type name.",
            ));
        }
        if let (Some(a), Some(_)) = (m.abstract_, m.external) {
            return Err(self.error_at(
                a,
                "Fields can't be declared both 'abstract' and 'external'.",
            ));
        }
        if let (Some(a), Some(_)) = (m.abstract_, m.static_) {
            return Err(self.error_at(a, "Static fields can't be declared 'abstract'."));
        }
        let li = self.pos;
        let type_ = if ti.is_none() {
            None
        } else {
            Some(self.type_until(ti.end_index)?)
        };
        let mut vars = Vec::new();
        loop {
            let n = self.expect_ident()?;
            if class_name.is_some_and(|c| c == self.txt(n)) {
                return Err(self.error_at(
                    n,
                    "A class member can't have the same name as the enclosing class.",
                ));
            }
            let (equals, initializer) = if self.op(self.pos, Op::Assign) {
                let e = self.advance();
                (Some(e as Tk), Some(self.expression()?))
            } else {
                (None, None)
            };
            vars.push(self.node(
                Node::VariableDeclaration {
                    name: n as Tk,
                    equals,
                    initializer,
                },
                n,
            ));
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        let start_ = [m.late, m.vfc()]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(li)
            .min(li);
        Ok(self.node(
            Node::VariableDeclarationList {
                metadata: Vec::new(),
                late_keyword: tk(m.late),
                keyword: tk(m.vfc()),
                type_,
                variables: vars,
            },
            start_,
        ))
    }

    /// `parseClassOrMixinOrExtensionOrEnumMemberImpl`.
    fn member(&mut self, body_: Body, class_name: &str) -> R<NodeId> {
        let start_ = self.pos;
        let metadata = self.metadata_()?;
        let mut m = Modifiers::default();
        self.collect_modifiers(&mut m)?;
        if let Some(r) = m.required {
            return Err(self.error_at(r, "Can't have modifier 'required' here."));
        }
        if m.vfc().is_some()
            && let Some(f) = self.skip_outer_pattern(self.pos)
            && self.op(f + 1, Op::Assign)
        {
            return Err(self.error(
                "Pattern variable declarations aren't allowed outside of functions or methods.",
            ));
        }
        let ti = self.type_at(self.pos, false, true, false);
        let mut nx = ti.end_index;
        let mut get_set = None;
        if !self.plain_ident(nx) {
            let v = self.txt(nx);
            if (v == "get" || v == "set") && self.ident(nx) {
                if self.ident(nx + 1) {
                    get_set = Some(nx);
                    nx += 1;
                } else if self.reserved(nx + 1) && self.indicates_method_or_field(nx + 2) {
                    return Err(self.error_at(nx + 1, "Expected an identifier."));
                }
            } else if v == "factory" && self.ident(nx) {
                let n2 = nx + 1;
                if self.ident(n2) || self.modifier(n2) {
                    if !ti.is_none() {
                        return Err(
                            self.error_at(nx, "Factory constructors cannot have a return type.")
                        );
                    }
                    if m.abstract_.is_some() {
                        return Err(
                            self.error("Members of classes can't be declared to be 'abstract'.")
                        );
                    }
                    return self.factory(start_, metadata, m, nx, body_);
                }
            } else if v == "operator" && self.ident(nx) {
                let n2 = nx + 1;
                if self.user_definable_operator(n2).is_some() && self.type_args_at(nx, false).is_none() {
                    return self.method(start_, metadata, m, ti, None, nx, body_, class_name);
                } else if !self.op(n2, Op::Assign)
                    && !self.op(n2, Op::Lt)
                    && !self.word_(n2)
                    && !self.op(n2, Op::LParen)
                    && !self.op(n2, Op::Semicolon)
                {
                    return Err(self.error_at(n2, "Invalid operator."));
                }
            } else if !self.ident(nx)
                || (v == "typedef" && ti.is_none() && start_ == self.pos && self.ident(nx + 1))
            {
                return Err(self.error_at(
                    nx,
                    &format!("Expected a class member, but got '{}'.", self.txt(nx)),
                ));
            }
        } else if ti.is_none() && m.vfc().is_none() {
            let n2 = nx + 1;
            if self.user_definable_operator(n2).is_some()
                && !self.op(n2, Op::LBracket)
                && (self.op(n2 + 1, Op::LParen)
                    || self.op(n2 + 1, Op::LBrace)
                    || self.op(n2 + 1, Op::Arrow))
            {
                return Err(self.error_at(
                    n2,
                    "Operator declarations must be preceded by the keyword 'operator'.",
                ));
            } else if self.reserved(n2) && self.indicates_method_or_field(n2 + 1) {
                return Err(self.error_at(n2, "Expected an identifier."));
            }
        }
        let after = nx + 1;
        if get_set.is_some()
            || matches!(
                self.kind_(after),
                TokenKind::Op(Op::LParen | Op::LBrace | Op::Lt | Op::Dot | Op::Arrow)
            )
        {
            return self.method(start_, metadata, m, ti, get_set, nx, body_, class_name);
        }
        if get_set.is_some() {
            return Err(self.error("Unexpected 'get'/'set'."));
        }
        if body_ == Body::Extension {
            if m.abstract_.is_some() {
                return Err(self.error("Extensions can't declare abstract members."));
            }
            if m.static_.is_none() && m.external.is_none() {
                return Err(self.error_at(nx, "Extensions can't declare instance fields."));
            }
        }
        let fields = self.fields_(&m, ti, Some(class_name))?;
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::FieldDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                abstract_keyword: tk(m.abstract_),
                covariant_keyword: tk(m.covariant),
                external_keyword: tk(m.external),
                static_keyword: tk(m.static_),
                fields,
                semicolon: s as Tk,
            },
            start_,
        ))
    }

    /// `parseMethod`: methods, getters, setters, operators and non-factory
    /// constructors. `name_` is the index of the name (or of `operator`).
    #[allow(clippy::too_many_arguments)]
    fn method(
        &mut self,
        start_: usize,
        metadata: Vec<NodeId>,
        m: Modifiers,
        ti: super::lookahead::TypeInfo,
        get_set: Option<usize>,
        name_: usize,
        body_: Body,
        class_name: &str,
    ) -> R<NodeId> {
        if let Some(a) = m.abstract_ {
            return Err(self.error_at(a, "Members of classes can't be declared to be 'abstract'."));
        }
        if let Some(l) = m.late {
            return Err(self.error_at(l, "Can't have modifier 'late' here."));
        }
        let operator_ = get_set.is_none()
            && self.word(name_, "operator")
            && self.user_definable_operator(name_ + 1).is_some()
            && self.type_args_at(name_, false).is_none();
        if m.static_.is_some() && operator_ {
            return Err(self.error("Operators can't be static."));
        }
        if m.static_.is_none()
            && let Some(c) = m.covariant
            && get_set.is_none_or(|g| self.txt(g) == "get")
        {
            return Err(self.error_at(
                c,
                "Getters, setters and methods can't be declared to be 'covariant'.",
            ));
        }
        if let Some(v) = m.var.or(m.final_) {
            return Err(self.error_at(v, "Methods can't be declared 'var' or 'final'."));
        }
        if m.const_.is_some() && get_set.is_some() {
            return Err(self.error("Getters and setters can't be 'const'."));
        }
        let return_type = if ti.is_none() {
            None
        } else {
            Some(self.type_until(ti.end_index)?)
        };
        if get_set.is_some() {
            self.advance();
        }
        let mut operator_keyword = None;
        let mut period = None;
        let mut name2 = None;
        let name = if operator_ {
            operator_keyword = Some(self.advance() as Tk);
            self.consume_user_definable_operator()?
        } else {
            let n = self.expect_ident()?;
            if self.op(self.pos, Op::Dot) {
                period = Some(self.advance());
                if !(self.ident(self.pos) || self.kw(self.pos, Keyword::New)) {
                    return Err(self.error("Expected an identifier."));
                }
                name2 = Some(self.advance());
            }
            n
        };
        let fi = self.pos;
        if operator_ && self.op(self.pos, Op::Lt) {
            return Err(self.error("Types parameters aren't allowed when defining an operator."));
        }
        let getter = get_set.is_some_and(|g| self.txt(g) == "get")
            && period.is_none()
            && !self.op(self.pos, Op::Colon);
        let type_parameters = if get_set.is_none() && self.op(self.pos, Op::Lt) {
            Some(self.type_params()?)
        } else {
            None
        };
        let member = match (body_, m.static_.is_some()) {
            (Body::Extension | Body::ExtensionType, _) => MemberKind::Extension,
            (_, true) => MemberKind::StaticMethod,
            _ => MemberKind::InstanceMethod,
        };
        let parameters = if self.op(self.pos, Op::LParen) {
            if getter {
                return Err(self.error("Getters must be declared without a parameter list."));
            }
            Some(self.formal_params(member)?)
        } else if getter {
            None
        } else {
            return Err(self.error("Methods must have an explicit list of parameters."));
        };
        let mut separator = None;
        let mut initializers = Vec::new();
        if self.op(self.pos, Op::Colon) {
            separator = Some(self.advance() as Tk);
            initializers = self.initializers()?;
        }
        let body_node = self.pos;
        if m.external.is_some()
            && !self.op(self.pos, Op::Semicolon)
            && !self.word(self.pos, "async")
            && !self.word(self.pos, "sync")
        {
            return Err(self.error("An external or native method can't have a body."));
        }
        if self.op(self.pos, Op::Assign) {
            return Err(self.error("Only factory constructor can specify '=' redirection."));
        }
        // fasta's `allowAbstract` requires a plain synchronous body.
        let async_kind = self.word(self.pos, "async") || self.word(self.pos, "sync");
        if async_kind && m.external.is_none() {
            let d = if self.op(self.pos + 1, Op::Star) {
                self.pos + 2
            } else {
                self.pos + 1
            };
            if self.op(d, Op::Semicolon) {
                return Err(self.error_at(d, "A function body must be provided."));
            }
        }
        let body = self.function_body_(false, m.static_.is_none() || m.external.is_some())?;
        let builder =
            period.is_some() || separator.is_some() || (self.txt(name) == class_name && !operator_);
        if builder && get_set.is_none() {
            if let Some(s) = m.static_ {
                return Err(self.error_at(s, "Constructors can't be static."));
            }
            if !ti.is_none() {
                return Err(self.error_at(
                    self.start_(return_type.unwrap_or(NodeId(0))),
                    "Constructors can't have a return type.",
                ));
            }
            if self.txt(name) != class_name {
                return Err(self.error_at(
                    name,
                    "The name of a constructor must match the name of the enclosing class.",
                ));
            }
            if separator.is_some() && m.external.is_some() {
                return Err(self.error("An external constructor can't have any initializers."));
            }
            if type_parameters.is_some() {
                return Err(self.error_at(fi, "Constructors can't have type parameters."));
            }
            if m.const_.is_some() && !matches!(self.tree_.node(body), Node::EmptyFunctionBody { .. }) {
                return Err(self.error_at(body_node, "Const constructors can't have a body."));
            }
            if m.external.is_some()
                && let Some(Node::FormalParameterList { parameters, .. }) =
                    parameters.map(|l| self.tree_.node(l))
                && parameters.iter().any(|&p| {
                    let p = match self.tree_.node(p) {
                        Node::DefaultFormalParameter { parameter, .. } => *parameter,
                        _ => p,
                    };
                    matches!(self.tree_.node(p), Node::FieldFormalParameter { .. })
                })
            {
                return Err(self.error_at(name, "An external constructor can't initialize fields."));
            }
            if matches!(body_, Body::Mixin | Body::Extension) {
                return Err(self.error_at(name, "Mixins and extensions can't declare constructors."));
            }
            if matches!(
                self.tree_.node(body),
                Node::BlockFunctionBody { .. } | Node::ExpressionFunctionBody { .. }
            ) && initializers
                .iter()
                .any(|&i| matches!(self.tree_.node(i), Node::RedirectingConstructorInvocation { .. }))
            {
                return Err(self.error_at(body_node, "Redirecting constructors can't have a body."));
            }
            let return_type = self.simple(name);
            return Ok(self.node(
                Node::ConstructorDeclaration {
                    metadata,
                    augment_keyword: tk(m.augment),
                    external_keyword: tk(m.external),
                    const_keyword: tk(m.const_),
                    factory_keyword: None,
                    return_type,
                    period: tk(period),
                    name: tk(name2),
                    parameters: parameters.unwrap_or(NodeId(0)),
                    separator,
                    initializers,
                    redirected_constructor: None,
                    body,
                },
                start_,
            ));
        }
        if builder {
            return Err(self.error_at(
                name,
                "A class member can't have the same name as the enclosing class.",
            ));
        }
        self.without_field_parameters(parameters)?;
        if m.const_.is_some() {
            return Err(self.error("Methods can't be declared 'const'."));
        }
        if body_ == Body::Extension
            && matches!(self.tree_.node(body), Node::EmptyFunctionBody { .. })
            && m.external.is_none()
        {
            return Err(self.error_at(name, "Extensions can't declare abstract members."));
        }
        Ok(self.node(
            Node::MethodDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                external_keyword: tk(m.external),
                modifier_keyword: tk(m.static_),
                return_type,
                property_keyword: tk(get_set),
                operator_keyword,
                name: name as Tk,
                type_parameters,
                parameters,
                body,
            },
            start_,
        ))
    }

    /// `parseFactoryMethod`; `f` is the index of the `factory`.
    fn factory(
        &mut self,
        start_: usize,
        metadata: Vec<NodeId>,
        m: Modifiers,
        f: usize,
        body_: Body,
    ) -> R<NodeId> {
        self.pos = f + 1;
        if !self.valid_type_ref(self.pos) {
            return Err(self.error("Modifiers must come before 'factory'."));
        }
        if let Some(x) = m.static_.or(m.covariant) {
            return Err(self.error_at(x, "Can't have this modifier on a factory."));
        }
        if let Some(v) = m.var.or(m.final_) {
            return Err(self.error_at(v, "Can't have this modifier on a factory."));
        }
        if matches!(body_, Body::Mixin | Body::Extension) {
            return Err(self.error_at(f, "Mixins and extensions can't declare constructors."));
        }
        let name = self.expect_ident()?;
        let (period, name2) = if self.op(self.pos, Op::Dot) {
            let p = self.advance();
            if !(self.ident(self.pos) || self.kw(self.pos, Keyword::New)) {
                return Err(self.error("Expected an identifier."));
            }
            (Some(p), Some(self.advance()))
        } else {
            (None, None)
        };
        if self.op(self.pos, Op::Lt) {
            return Err(self.error("Constructors can't have type parameters."));
        }
        if !self.op(self.pos, Op::LParen) {
            return Err(self.expected("("));
        }
        let parameters = self.formal_params(MemberKind::Factory)?;
        let mut separator = None;
        let mut redirected_constructor = None;
        let body = if self.op(self.pos, Op::Assign) {
            if m.external.is_some() {
                return Err(self.error("A redirecting factory can't be external."));
            }
            separator = Some(self.advance() as Tk);
            redirected_constructor = Some(self.constructor_name_()?);
            let s = self.expect_op(Op::Semicolon)?;
            self.node(Node::EmptyFunctionBody { semicolon: s as Tk }, s)
        } else if m.external.is_some() {
            if !self.op(self.pos, Op::Semicolon) {
                return Err(self.error("External factories can't have a body."));
            }
            self.function_body_(false, true)?
        } else {
            if m.const_.is_some() && !self.word(self.pos, "native") {
                return Err(self
                    .error("Only redirecting factory constructors can be declared to be 'const'."));
            }
            self.function_body_(false, false)?
        };
        let return_type = self.simple(name);
        Ok(self.node(
            Node::ConstructorDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                external_keyword: tk(m.external),
                const_keyword: tk(m.const_),
                factory_keyword: Some(f as Tk),
                return_type,
                period: tk(period),
                name: tk(name2),
                parameters,
                separator,
                initializers: Vec::new(),
                redirected_constructor,
                body,
            },
            start_,
        ))
    }

    /// `parseInitializers` (after the `:`).
    fn initializers(&mut self) -> R<Vec<NodeId>> {
        let old = self.functions;
        self.functions = false;
        let mut v = Vec::new();
        loop {
            v.push(self.initializer()?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        self.functions = old;
        Ok(v)
    }

    /// The right-hand side of `field = ...` (the rest of `parseExpression`
    /// after the `=` of an assignment).
    fn initializer_right(&mut self) -> R<NodeId> {
        if self.kw(self.pos, Keyword::Throw) {
            return self.expression_without_cascade();
        }
        self.enter_()?;
        let r = self.precedence(
            super::prec::ASSIGNMENT,
            true,
            super::expressions::ConstCtx::NoneKind,
        );
        self.sair();
        let r = r?;
        self.without_super(r)?;
        Ok(r)
    }

    /// `parseInitializer`.
    fn initializer(&mut self) -> R<NodeId> {
        let i = self.pos;
        if self.kw(i, Keyword::Assert) {
            let (a, lp, condition, comma, message, rp) = self.assert_parts()?;
            return Ok(self.node(
                Node::AssertInitializer {
                    assert_keyword: a as Tk,
                    left_parenthesis: lp as Tk,
                    condition,
                    comma: tk(comma),
                    message,
                    right_parenthesis: rp as Tk,
                },
                a,
            ));
        }
        if self.kw(i, Keyword::Super) {
            self.advance();
            let (period, constructor_name) = if self.op(self.pos, Op::Dot) {
                let p = self.advance();
                if !(self.ident(self.pos) || self.kw(self.pos, Keyword::New)) {
                    return Err(self.error("Expected an identifier."));
                }
                let n = self.advance();
                (Some(p as Tk), Some(self.simple(n)))
            } else {
                (None, None)
            };
            if !self.op(self.pos, Op::LParen) {
                return Err(self.expected("("));
            }
            let argument_list = self.arguments_()?;
            return Ok(self.node(
                Node::SuperConstructorInvocation {
                    super_keyword: i as Tk,
                    period,
                    constructor_name,
                    argument_list,
                },
                i,
            ));
        }
        if self.kw(i, Keyword::This) {
            self.advance();
            let mut period = None;
            let mut name_ = None;
            if self.op(self.pos, Op::Dot) {
                period = Some(self.advance());
                if !(self.ident(self.pos)
                    || (self.kw(self.pos, Keyword::New) && self.op(self.pos + 1, Op::LParen)))
                {
                    return Err(self.error("Expected an identifier."));
                }
                name_ = Some(self.advance());
                if self.op(self.pos, Op::Assign) {
                    let field_name = self.simple(name_.unwrap_or(0));
                    let eq = self.advance();
                    let expression = self.initializer_right()?;
                    return Ok(self.node(
                        Node::ConstructorFieldInitializer {
                            this_keyword: Some(i as Tk),
                            period: tk(period),
                            field_name,
                            equals: eq as Tk,
                            expression,
                        },
                        i,
                    ));
                }
            }
            if self.op(self.pos, Op::LParen) {
                let constructor_name = name_.map(|n| self.simple(n));
                let argument_list = self.arguments_()?;
                return Ok(self.node(
                    Node::RedirectingConstructorInvocation {
                        this_keyword: i as Tk,
                        period: tk(period),
                        constructor_name,
                        argument_list,
                    },
                    i,
                ));
            }
            return Err(self.expected("."));
        }
        if self.ident(i) && self.op(i + 1, Op::Assign) {
            self.advance();
            let field_name = self.simple(i);
            let eq = self.advance();
            let expression = self.initializer_right()?;
            return Ok(self.node(
                Node::ConstructorFieldInitializer {
                    this_keyword: None,
                    period: None,
                    field_name,
                    equals: eq as Tk,
                    expression,
                },
                i,
            ));
        }
        Err(self.error("Expected an initializer."))
    }
}
