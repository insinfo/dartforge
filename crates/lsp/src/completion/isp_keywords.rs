//! O `KeywordHelper` do analysis server 3.6.2
//! (`AS:src/services/completion/dart/keyword_helper.dart`, lido inteiro) e a
//! `KeywordSuggestion` (`candidate_suggestion.dart:452-512`).
//! Escrito sem compilar nem executar (2026-10-05).

use super::isp::{Isp, Feature};
use super::*;
use dartforge_frontend::token::Op;
use std::sync::{Mutex, OnceLock};

/// O texto de uma sugestão de palavra-chave como `&'static str` (o
/// `Rel.word`): os textos possíveis são finitos (a palavra mais um texto
/// anotado fixo do passe), guardados uma vez cada.
fn is_static(s: &str) -> &'static str {
    static TEXTS: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut m = TEXTS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&x) = m.get(s) {
        return x;
    }
    let x: &'static str = Box::leak(s.to_string().into_boxed_str());
    m.insert(x);
    x
}

/// `annotatedText.withoutCaret`.
fn without_caret(t: &str) -> (String, Option<usize>) {
    match t.find('^') {
        None => (t.to_string(), None),
        Some(i) => (format!("{}{}", &t[..i], &t[i + 1..]), Some(i)),
    }
}

/// `CollectionElement` que não é `Expression`.
fn element_not_expression(e: &str) -> bool {
    matches!(e, "IfElement" | "ForElement" | "SpreadElement" | "MapLiteralEntry" | "NullAwareElement")
}

impl<'a, 'c> Isp<'a, 'c> {
    /// Empurra a `KeywordSuggestion` (`completion`, `selectionOffset`).
    pub(super) fn word_suggestion(&mut self, completion: String, selection: usize, score: f64) {
        let before = self.collector.items.len();
        let end = completion.encode_utf16().count();
        let item = self.collector.push(group::WORD, kind::WORD_KEY, completion.clone(), completion.clone(), None);
        item.rel.word = Some(is_static(&completion));
        item.of_pass = true;
        if selection != end {
            item.selection = Some((selection, 0));
        }
        self.register(before, score);
    }

    /// `addKeyword`: o casamento é pelo lexema.
    pub(super) fn kw(&mut self, word: &str) {
        let s = self.score(word);
        if s != -1.0 {
            self.word_suggestion(word.to_string(), word.encode_utf16().count(), s);
        }
    }

    /// `addKeywordAndText`: o casamento é pelo lexema; o texto vai depois.
    pub(super) fn kw_and_text(&mut self, word: &str, annotated: &str) {
        let s = self.score(word);
        if s != -1.0 {
            let (raw, c) = without_caret(annotated);
            let sel = word.encode_utf16().count() + c.map_or(raw.encode_utf16().count(), |i| raw[..i].encode_utf16().count());
            self.word_suggestion(format!("{word}{raw}"), sel, s);
        }
    }

    /// `addText`: o casamento é pelo texto sem o `^`.
    pub(super) fn annotated_text(&mut self, annotated: &str) {
        let (raw, c) = without_caret(annotated);
        let s = self.score(&raw);
        if s != -1.0 {
            let sel = c.map_or(raw.encode_utf16().count(), |i| raw[..i].encode_utf16().count());
            self.word_suggestion(raw, sel, s);
        }
    }

    /// `_isAbsentOrIn`.
    pub(super) fn missing_or_in(&self, t: Option<usize>) -> bool {
        t.is_none_or(|i| {
            let s = self.span_tok(i);
            s.start <= self.offset && self.offset <= s.end
        })
    }

    /// A palavra que abre a cláusula filha `kind` de `n`
    /// (`extendsKeyword`, `withKeyword`…).
    pub(super) fn clause_word(&self, n: usize, kind: &str) -> Option<usize> {
        let c = self.child(n, kind)?;
        let i = self.tok_after(self.start(c))?;
        (self.span_tok(i).start < self.end(c).max(self.start(c) + 1) && self.word(i)).then_some(i)
    }

    /// Um modificador escrito antes da palavra `until` (`class`, `mixin`) da
    /// declaração.
    pub(super) fn modifier_before(&self, n: usize, word: &str, until: &str) -> Option<usize> {
        let start = self.first_after_annotations(n).map_or(self.start(n), |i| self.span_tok(i).start);
        let end = self.word_in(until, start, self.end(n)).map_or(self.end(n), |i| self.span_tok(i).start);
        self.word_in(word, start, end)
    }

    /// `addClassDeclarationKeywords`.
    pub(super) fn class_declaration_kw(&mut self, n: usize) {
        if self.missing_or_in(self.clause_word(n, "ExtendsClause")) {
            self.kw("extends");
        }
        if self.missing_or_in(self.clause_word(n, "WithClause")) {
            self.kw("with");
        }
        if self.missing_or_in(self.clause_word(n, "ImplementsClause")) {
            self.kw("implements");
        }
    }

    /// `addClassMemberKeywords`.
    pub(super) fn class_member_kw(&mut self) {
        for p in ["const", "covariant", "dynamic", "factory", "final", "get", "operator", "set", "static", "var", "void", "late"] {
            self.kw(p);
        }
    }

    /// `addClassModifiers`.
    pub(super) fn class_modifiers_kw(&mut self, n: usize) {
        let abstract_ = self.modifier_before(n, "abstract", "class");
        let sealed = self.modifier_before(n, "sealed", "class");
        let base = self.modifier_before(n, "base", "class");
        let final_ = self.modifier_before(n, "final", "class");
        let interface = self.modifier_before(n, "interface", "class");
        let mixin = self.modifier_before(n, "mixin", "class");
        if self.missing_or_in(abstract_) && sealed.is_none() {
            self.kw("abstract");
        }
        if self.has_feature(Feature::ClassModifiers) && self.has_feature(Feature::SealedClass) {
            if base.is_none() && final_.is_none() && interface.is_none() && mixin.is_none() && sealed.is_none() {
                if abstract_.is_none() {
                    self.kw("sealed");
                } else {
                    self.kw("base");
                    self.kw("final");
                    self.kw("interface");
                    self.kw("mixin");
                }
            }
            if base.is_some() && self.missing_or_in(mixin) {
                self.kw("mixin");
            }
            if mixin.is_some() && self.missing_or_in(base) {
                self.kw("base");
            }
        }
    }

    /// O `else` de um `IfElement` e o elemento depois dele.
    pub(super) fn if_else(&self, n: usize) -> (Option<usize>, Option<usize>) {
        let opens = self.op_in(Op::LParen, self.start(n), self.end(n));
        let after = opens.and_then(|a| self.pair(a)).map_or(self.start(n), |f| self.span_tok(f).end);
        let Some(e) = self.word_in("else", after, self.end(n)) else { return (None, None) };
        let end_e = self.span_tok(e).end;
        (Some(e), self.children(n).iter().copied().find(|&k| self.start(k) >= end_e))
    }

    /// `couldHaveTrailingElse`.
    pub(super) fn can_have_else(&self, el: usize) -> bool {
        let mut f = el;
        loop {
            match self.kind(f) {
                "IfElement" => match self.if_else(f) {
                    (Some(_), Some(s)) => f = s,
                    _ => break,
                },
                "ForElement" => match self.children(f).last() {
                    Some(&b) => f = b,
                    None => break,
                },
                _ => break,
            }
        }
        if self.kind(f) != "IfElement" || self.if_else(f).0.is_some() {
            return false;
        }
        // `thenElement` não sintético: há um elemento depois do `)`.
        let opens = self.op_in(Op::LParen, self.start(f), self.end(f));
        let Some(closes) = opens.and_then(|a| self.pair(a)) else { return false };
        let end_f = self.span_tok(closes).end;
        self.children(f).iter().any(|&k| self.start(k) >= end_f && !self.all_synthetic(k))
    }

    /// `addCollectionElementKeywords`.
    pub(super) fn collection_element_kw(&mut self, literal: usize, elements: &[usize], constant: bool, is_static: bool) {
        self.kw("for");
        self.kw("if");
        if let Some(previous) = self.element_before(elements) {
            let next = self.tok_after(self.end(previous));
            if next.is_none_or(|i| self.offset <= self.span_tok(i).start) {
                if self.can_have_else(previous) {
                    self.kw("else");
                } else if let Some(k) = elements.iter().position(|&e| e == previous)
                    && k > 0
                    && self.can_have_else(elements[k - 1])
                {
                    self.kw("else");
                }
            }
        }
        self.kw_expression(Some(literal), constant, is_static);
    }

    /// `addCompilationUnitDeclarationKeywords`.
    pub(super) fn unit_declaration_kw(&mut self) {
        for p in ["abstract", "class", "const", "covariant", "dynamic", "enum", "external", "final", "mixin", "typedef", "var", "void"] {
            self.kw(p);
        }
        if self.has_feature(Feature::ExtensionMethods) {
            self.kw("extension");
        }
        self.kw("late");
        if self.has_feature(Feature::ClassModifiers) {
            self.kw("base");
            self.kw("interface");
        }
        if self.has_feature(Feature::SealedClass) {
            self.kw("sealed");
        }
    }

    /// `addConstantExpressionKeywords`.
    pub(super) fn constant_expression_kw(&mut self, in_constant_context: bool) {
        self.kw("false");
        self.kw("null");
        self.kw("true");
        if !in_constant_context {
            self.kw("const");
        }
    }

    /// `addConstructorInitializerKeywords`. A lista vazia (o fasta sempre
    /// cria ao menos um inicializador, sintético) conta como um
    /// `ConstructorFieldInitializer` sintético.
    pub(super) fn constructor_initializer_kw(&mut self, ctor: usize, init: Option<usize>) {
        self.kw("assert");
        let inits = self.children_of(ctor, &["ConstructorFieldInitializer", "SuperConstructorInvocation", "RedirectingConstructorInvocation", "AssertInitializer"]);
        let last = inits.last().copied();
        if init.is_none() || last == init {
            let not_synthetic = match last {
                Some(u) if self.all_synthetic(u) && inits.len() > 1 => Some(inits[inits.len() - 2]),
                u => u,
            };
            let ok = (not_synthetic.is_some() && not_synthetic == init)
                || not_synthetic.is_none_or(|u| !matches!(self.kind(u), "SuperConstructorInvocation" | "RedirectingConstructorInvocation"));
            if ok {
                if self.parent(ctor).is_none_or(|p| self.kind(p) != "ExtensionTypeDeclaration") {
                    self.kw("super");
                }
                self.kw("this");
            }
        } else if let Some(i) = init
            && self.kind(i) == "ConstructorFieldInitializer"
        {
            let (end_equal, next) = match self.op_in(Op::Assign, self.start(i), self.end(i).max(self.start(i) + 1)) {
                Some(eq) => (self.span_tok(eq).end, self.toks.get(eq + 1).map_or(self.source.len(), |t| t.span.start)),
                None => {
                    let p = self.synthetic(self.end(i)).start;
                    (p, p)
                }
            };
            if end_equal <= self.offset && self.offset <= next {
                self.kw("this");
            }
        }
    }

    /// `addDirectiveKeywords`.
    pub(super) fn kw_directive(&mut self, before: Option<usize>) {
        let has_library = self.children(0).iter().any(|&k| self.kind(k) == "LibraryDirective");
        if before.is_none() && !has_library {
            self.kw("library");
        }
        self.kw_and_text("import", " '^';");
        self.kw_and_text("export", " '^';");
        self.kw_and_text("part", " '^';");
        let without_directives = !self.children(0).iter().any(|&k| self.is_directive(k));
        if without_directives {
            self.annotated_text("part of '^';");
        }
    }

    /// `addEnumDeclarationKeywords`.
    pub(super) fn enum_declaration_kw(&mut self, n: usize) {
        if self.missing_or_in(self.clause_word(n, "WithClause")) {
            self.kw("with");
        }
        if self.missing_or_in(self.clause_word(n, "ImplementsClause")) {
            self.kw("implements");
        }
    }

    /// `addEnumMemberKeywords`.
    pub(super) fn enum_member_kw(&mut self) {
        for p in ["const", "dynamic", "final", "get", "late", "operator", "set", "static", "var", "void"] {
            self.kw(p);
        }
    }

    /// `constIsValid` de `addExpressionKeywords`.
    pub(super) fn valid_const(&self, n: usize) -> bool {
        let mut n = n;
        if element_not_expression(self.kind(n)) {
            match self.parent(n) {
                Some(p) => n = p,
                None => return false,
            }
        }
        let e = self.kind(n);
        if super::isp::EXPRESSION_KINDS.contains(&e) {
            return !self.constant_context(n);
        }
        match e {
            "Block" | "EmptyStatement" | "ExpressionStatement" | "IfStatement" | "PatternVariableDeclaration" | "SwitchPatternCase"
            | "SwitchStatement" | "WhenClause" => true,
            "RecordPattern" => self.children_of(n, &["PatternField"]).is_empty(),
            "VariableDeclaration" => !self.parent(n).and_then(|l| self.real_list(l)).is_some_and(|l| l.const_),
            "VariableDeclarationStatement" => !self.child(n, "VariableDeclarationList").and_then(|l| self.real_list(l)).is_some_and(|l| l.const_),
            _ => false,
        }
    }

    /// O `:` de um `SwitchPatternCase` (o offset; sintético: o do próximo
    /// token depois do padrão).
    pub(super) fn case_colon(&self, n: usize) -> usize {
        let after = self.child(n, "GuardedPattern").map_or_else(|| self.tok_after(self.start(n)).map_or(self.start(n), |i| self.span_tok(i).end), |g| self.end(g));
        match self.op_in(Op::Colon, after, self.end(n).max(after + 1)) {
            Some(i) => self.span_tok(i).start,
            None => self.synthetic(after).start,
        }
    }

    /// `switchIsValid` de `addExpressionKeywords`.
    pub(super) fn valid_switch(&self, n: usize) -> bool {
        let mut n = n;
        if element_not_expression(self.kind(n)) {
            match self.parent(n) {
                Some(p) => n = p,
                None => return true,
            }
        }
        !(self.kind(n) == "SwitchPatternCase" && self.offset <= self.case_colon(n))
    }

    /// `addExpressionKeywords`.
    pub(super) fn kw_expression(&mut self, no: Option<usize>, constant: bool, is_static: bool) {
        self.kw("false");
        self.kw("null");
        self.kw("true");
        match no {
            Some(n) => {
                if self.valid_const(n) {
                    self.kw("const");
                }
                if !constant && !is_static {
                    self.kw("super");
                    self.kw("this");
                }
                if self.in_async(n) || self.in_generator(n) {
                    self.kw("await");
                }
                if self.valid_switch(n) && self.has_feature(Feature::Patterns) {
                    self.kw("switch");
                }
            }
            None => {
                if self.has_feature(Feature::Patterns) {
                    self.kw("switch");
                }
            }
        }
    }

    /// `addExtensionDeclarationKeywords`.
    pub(super) fn extension_declaration_kw(&mut self, n: usize) {
        let on = self.child(n, "ExtensionOnClause").and_then(|c| self.word_in("on", self.start(c), self.end(c).max(self.start(c) + 1)));
        if on.is_none() {
            self.kw("on");
            let unnamed = matches!(self.real_decl(n).map(|d| &d.kind), Some(ast::DeclKind::Extension(x)) if x.name.is_none());
            if unnamed && self.has_feature(Feature::InlineClass) {
                self.annotated_text("type");
            }
        }
    }

    /// `addExtensionMemberKeywords`.
    pub(super) fn extension_member_kw(&mut self, is_static: bool) {
        self.kw("const");
        self.kw("dynamic");
        self.kw("final");
        self.kw("get");
        if !is_static {
            self.kw("operator");
        }
        self.kw("set");
        if !is_static {
            self.kw("static");
        }
        self.kw("var");
        self.kw("void");
    }

    /// `addExtensionTypeMemberKeywords`.
    pub(super) fn extension_type_member_kw(&mut self, is_static: bool) {
        self.extension_member_kw(is_static);
    }

    /// `addFieldDeclarationKeywords`: `word` é a palavra-chave que a
    /// recuperação usou como nome da única variável.
    pub(super) fn field_declaration_kw(&mut self, n: usize, word: Option<&str>) {
        let Some(list_node) = self.child(n, "VariableDeclarationList") else { return };
        let Some(l) = self.real_list(list_node) else { return };
        let first = l.variables.first();
        let until = first.map_or(self.end(n), |v| v.name.span.start);
        let start = self.first_after_annotations(n).map_or(self.start(n), |i| self.span_tok(i).start);
        let external = self.word_in("external", start, until);
        let is_static = self.word_in("static", start, until);
        let abstract_ = self.word_in("abstract", start, until);
        let covariant = self.word_in("covariant", start, until);
        let late = self.word_in("late", start, until);
        if self.missing_or_in(external) && word != Some("external") {
            self.kw("external");
        }
        if l.ty.is_none() {
            self.kw("dynamic");
            self.kw("void");
        }
        if is_static.is_none() && word != Some("static") {
            if self.missing_or_in(abstract_) && word != Some("abstract") {
                self.kw("abstract");
            }
            if self.missing_or_in(covariant) && word != Some("covariant") {
                self.kw("covariant");
            }
            if self.missing_or_in(late) && word != Some("late") {
                self.kw("late");
            }
            self.kw("static");
        }
        if first.is_some() && !l.const_ && !l.final_ && !matches!(word, Some("const" | "final" | "var")) {
            self.kw("const");
            self.kw("final");
            if l.ty.is_none() {
                self.kw("var");
            }
        }
    }

    /// `FormalParameterList.inNamedGroup`.
    pub(super) fn in_named_group(&self, list: usize) -> bool {
        let de = self.start(list) + 1;
        let end = self.end(list);
        let key = self.op_in(Op::LBrace, de, end);
        let bracket = self.op_in(Op::LBracket, de, end);
        let left_ = match (key, bracket) {
            (Some(a), Some(b)) if b < a => return false,
            (Some(a), _) => a,
            _ => return false,
        };
        let left = self.span_tok(left_).end;
        let right = self
            .pair(left_)
            .filter(|&j| self.span_tok(j).start < end)
            .map(|j| self.span_tok(j).start)
            .or_else(|| self.tok_before(end).filter(|&j| self.is_op(j, Op::RParen)).map(|j| self.span_tok(j).start))
            .unwrap_or(end);
        left <= self.offset && self.offset <= right
    }

    /// `addFormalParameterKeywords`.
    pub(super) fn formal_parameter_kw(&mut self, list: usize) {
        self.kw("covariant");
        self.kw("final");
        if self.in_named_group(list) {
            self.kw("required");
        }
        if self.parent(list).is_some_and(|p| self.kind(p) == "ConstructorDeclaration") {
            if self.has_feature(Feature::SuperParameters) {
                self.kw("super");
            }
            self.kw("this");
        }
    }

    /// `addFunctionBodyModifiers`.
    pub(super) fn body_modifiers_kw(&mut self, body: Option<usize>) {
        let word = body.and_then(|c| {
            let i = self.tok_after(self.start(c))?;
            (matches!(self.text_tok(i), "async" | "sync") && self.span_tok(i).start < self.end(c).max(self.start(c) + 1)).then_some(i)
        });
        if self.missing_or_in(word) {
            self.kw("async");
            if body.is_none_or(|c| self.kind(c) != "ExpressionFunctionBody") {
                self.kw_and_text("async", "*");
                self.kw_and_text("sync", "*");
            }
        }
    }

    /// `addImportDirectiveKeywords`.
    pub(super) fn import_directive_kw(&mut self, n: usize) {
        let (start, end) = (self.start(n), self.end(n));
        let deferred = self.word_in("deferred", start, end);
        let as_kw = self.word_in("as", start, end);
        let first = self.children_of(n, &["ShowCombinator", "HideCombinator"]).first().copied();
        if first.is_none_or(|c| self.offset < self.start(c)) {
            match deferred {
                None => match as_kw {
                    None => {
                        self.kw_and_text("deferred", " as");
                        self.kw("as");
                        self.kw("hide");
                        self.kw("show");
                    }
                    Some(a) if self.offset < self.span_tok(a).start => self.kw("deferred"),
                    Some(a) => {
                        // `prefix`: o identificador depois do `as` (sintético:
                        // no próximo token).
                        let end_a = self.span_tok(a).end;
                        let prefix_end = match self.tok_after(end_a) {
                            Some(i) if self.is_identifier(i) => self.span_tok(i).end,
                            _ => self.synthetic(end_a).start,
                        };
                        if self.offset > prefix_end {
                            self.kw("hide");
                            self.kw("show");
                        }
                    }
                },
                Some(d) if self.offset > self.span_tok(d).end && as_kw.is_none() => self.kw("as"),
                Some(_) => {
                    self.kw("hide");
                    self.kw("show");
                }
            }
        } else {
            self.kw("hide");
            self.kw("show");
        }
    }

    /// `addMixinDeclarationKeywords`.
    pub(super) fn mixin_declaration_kw(&mut self, n: usize) {
        if self.missing_or_in(self.clause_word(n, "MixinOnClause")) {
            self.kw("on");
        }
        if self.missing_or_in(self.clause_word(n, "ImplementsClause")) {
            self.kw("implements");
        }
    }

    /// `addMixinMemberKeywords`.
    pub(super) fn mixin_member_kw(&mut self) {
        for p in ["const", "covariant", "dynamic", "final", "get", "operator", "set", "static", "var", "void", "late"] {
            self.kw(p);
        }
    }

    /// `addMixinModifiers`.
    pub(super) fn mixin_modifiers_kw(&mut self, n: usize) {
        if self.missing_or_in(self.modifier_before(n, "base", "mixin")) {
            self.kw("base");
        }
    }

    /// `addPatternKeywords`.
    pub(super) fn kw_pattern(&mut self) {
        self.constant_expression_kw(false);
        self.variable_pattern_kw();
    }

    /// `addStatementKeywords`.
    pub(super) fn kw_command(&mut self, n: usize) {
        if self.in_async(n) {
            self.kw("await");
        } else if self.in_generator(n) {
            self.kw("await");
            self.kw("yield");
            self.kw_and_text("yield", "*");
        }
        if self.in_loop(n) {
            self.kw("break");
            self.kw("continue");
        }
        if self.in_switch(n) {
            self.kw("break");
        }
        self.kw("assert");
        self.kw("do");
        self.kw("dynamic");
        self.kw("final");
        self.kw("for");
        self.kw("if");
        if self.in_catch(n) {
            self.kw("rethrow");
        }
        self.kw("return");
        if !self.has_feature(Feature::Patterns) {
            self.kw("switch");
        }
        self.kw("throw");
        self.kw("try");
        self.kw("var");
        self.kw("void");
        self.kw("while");
        if self.in_generator(n) {
            self.kw("yield");
            self.kw_and_text("yield", "*");
        }
        self.kw("late");
    }

    /// `addTryClauseKeywords`.
    pub(super) fn try_clauses_kw(&mut self, can_have_finally: bool) {
        self.kw("catch");
        if can_have_finally {
            self.kw("finally");
        }
        self.kw("on");
    }

    /// `addVariablePatternKeywords`.
    pub(super) fn variable_pattern_kw(&mut self) {
        self.kw("final");
        self.kw("var");
    }
}
