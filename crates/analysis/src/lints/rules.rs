//! The first batch of lint rules, the ones that only look at the tree
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8): `camel_case_types`,
//! `camel_case_extensions`, `non_constant_identifier_names`,
//! `empty_catches`, `avoid_empty_else`,
//! `curly_braces_in_flow_control_structures`,
//! `use_string_in_part_of_directives`,
//! `prefer_generic_function_type_aliases`, `provide_deprecation_message` and
//! `avoid_relative_lib_imports`.
//!
//! Written with the emitters of the SDK's `main` open and then checked
//! against those of 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter`,
//! extracted from the tag on 2026-10-05). None of the ten differs in the rule.
//! `non_constant_identifier_names` follows the original's twelve visitors
//! (also record fields, the extension type constructor name, the variable of
//! a collection `for`/`for-in`, the parameters of `Function(…)`, the pattern
//! variable of a declaration without a keyword, the `:name` shorthand left
//! out and, in an augmentation, only the named parameters skipped);
//! `prefer_generic_function_type_aliases` builds the suggestion with `toSource`
//! (`dartforge_frontend::fonte`); `provide_deprecation_message` needs the
//! annotation's element (`dartforge_types::anotacoes`, only with the unit's
//! semantics) and looks at every annotation of the unit.
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, DeclKind, DirectiveKind, ForInTarget, FunctionKind, MemberKind, PatternKind, StmtId, StmtKind, TypedefKind,
};
use dartforge_intern::Interner;
use std::collections::HashSet;

/// A lint report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintReport {
    pub code: &'static CodigoLint,
    pub span: Span,
    pub args: Vec<String>,
}

impl LintReport {
    /// The message with the arguments in place.
    pub fn message(&self) -> String {
        let mut out = self.code.mensagem.to_string();
        for (i, a) in self.args.iter().enumerate() {
            out = out.replace(&format!("{{{i}}}"), a);
        }
        out
    }

    /// The correction message, with the arguments in place.
    pub fn correction(&self) -> Option<String> {
        self.code.correcao.map(|molde| {
            let mut out = molde.to_string();
            for (i, a) in self.args.iter().enumerate() {
                out = out.replace(&format!("{{{i}}}"), a);
            }
            out
        })
    }
}

/// The names of the unit's `DeclaredVariablePattern`s that a
/// `visitDeclaredVariablePattern` visitor guarded by `isFieldNameShortcut`
/// looks at: the pattern variables outside assignment patterns (where they are
/// `AssignedVariablePattern`s), except the one that is directly the pattern of
/// a `:name` field. In a refutable pattern (`case`, `if-case`), a loose name
/// (without `var`, `final` or a type) is a constant pattern, not a variable.
pub fn pattern_declared_variables(a: &ast::Ast) -> Vec<ast::Name> {
    let mut in_assignment: HashSet<ast::PatternId> = HashSet::new();
    let mut stack: Vec<ast::PatternId> = a
        .exprs
        .iter()
        .filter_map(|e| match &e.kind {
            ast::ExprKind::PatternAssign { pattern, .. } => Some(*pattern),
            _ => None,
        })
        .collect();
    while let Some(p) = stack.pop() {
        if !in_assignment.insert(p) {
            continue;
        }
        match &a.pattern(p).kind {
            PatternKind::Or(l, r) | PatternKind::And(l, r) => stack.extend([*l, *r]),
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => stack.push(*x),
            PatternKind::List { elements, .. } => stack.extend(elements.iter().filter_map(|e| match e {
                ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => Some(*x),
                ast::ListPatternElement::Rest(None) => None,
            })),
            PatternKind::Map { entries, .. } => stack.extend(entries.iter().map(|e| e.value)),
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => stack.extend(fields.iter().map(|f| f.pattern)),
            _ => {}
        }
    }
    // `isFieldNameShortcut`: the direct pattern of a `:name` field, also behind
    // a `?` and a `!` (the `NullCheckPattern` and the `NullAssertPattern` that
    // wrap it).
    let mut shorthands: HashSet<ast::PatternId> = HashSet::new();
    for p in a.patterns.iter() {
        if let PatternKind::Record { fields } | PatternKind::Object { fields, .. } = &p.kind {
            for f in fields.iter() {
                let sp = a.pattern(f.pattern).span;
                if f.name.is_some_and(|n| n.span.start >= sp.start && n.span.end <= sp.end) {
                    shorthands.insert(f.pattern);
                    match &a.pattern(f.pattern).kind {
                        PatternKind::NullAssert(x) => {
                            shorthands.insert(*x);
                            if let PatternKind::NullCheck(y) = &a.pattern(*x).kind {
                                shorthands.insert(*y);
                            }
                        }
                        PatternKind::NullCheck(x) => {
                            shorthands.insert(*x);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    // The ranges of the refutable patterns.
    let mut refutaveis: Vec<Span> = Vec::new();
    for s in a.stmts.iter() {
        match &s.kind {
            StmtKind::Switch { cases, .. } => refutaveis.extend(cases.iter().filter_map(|c| c.pattern).map(|p| a.pattern(p).span)),
            StmtKind::If { case_pattern: Some(p), .. } => refutaveis.push(a.pattern(*p).span),
            _ => {}
        }
    }
    fn in_collection(a: &ast::Ast, el: &ast::CollectionElement, out: &mut Vec<Span>) {
        match el {
            ast::CollectionElement::If { case_pattern, then, else_, .. } => {
                out.extend(case_pattern.map(|p| a.pattern(p).span));
                in_collection(a, then, out);
                if let Some(x) = else_ {
                    in_collection(a, x, out);
                }
            }
            ast::CollectionElement::For { body, .. } | ast::CollectionElement::ForIn { body, .. } => in_collection(a, body, out),
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        match &e.kind {
            ast::ExprKind::Switch { cases, .. } => refutaveis.extend(cases.iter().map(|c| a.pattern(c.pattern).span)),
            ast::ExprKind::List { elements, .. } | ast::ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    in_collection(a, el, &mut refutaveis);
                }
            }
            _ => {}
        }
    }
    let mut v = Vec::new();
    for (k, p) in a.patterns.iter().enumerate() {
        let id = ast::PatternId(k as u32);
        if let PatternKind::Variable { name, final_, var_, ty } = &p.kind
            && !in_assignment.contains(&id)
            && !shorthands.contains(&id)
        {
            let solto = !*final_ && !*var_ && ty.is_none();
            if solto && refutaveis.iter().any(|r| r.start <= p.span.start && p.span.end <= r.end) {
                continue;
            }
            v.push(*name);
        }
    }
    v
}

/// `isCamelCase`: `^_*(?:\$+_+)*[$?A-Z][$?a-zA-Z\d]*$`.
pub fn is_camel_case(name: &str) -> bool {
    let mut rest = name.trim_start_matches('_');
    // `$…_…` groups, only when there is an underscore after the dollar signs.
    loop {
        let without_dollars = rest.trim_start_matches('$');
        if without_dollars.len() == rest.len() || !without_dollars.starts_with('_') {
            break;
        }
        rest = without_dollars.trim_start_matches('_');
    }
    let mut letras = rest.chars();
    letras.next().is_some_and(|p| p == '$' || p == '?' || p.is_ascii_uppercase())
        && letras.all(|x| x == '$' || x == '?' || x.is_ascii_alphanumeric())
}

/// `isLowerCamelCase`: a lone capital letter, `_`, or
/// `^_*[?$a-z][a-z\d?$]*(?:(?:[A-Z]|_\d)[a-z\d?$]*)*_?$`.
pub fn is_lower_camel_case(name: &str) -> bool {
    if name == "_" || (name.len() == 1 && name.as_bytes()[0].is_ascii_uppercase()) {
        return true;
    }
    let b = name.trim_start_matches('_').as_bytes();
    let lower = |x: u8| x.is_ascii_lowercase() || x.is_ascii_digit() || x == b'?' || x == b'$';
    if !b.first().is_some_and(|p| p.is_ascii_lowercase() || *p == b'?' || *p == b'$') {
        return false;
    }
    let mut i = 1;
    while i < b.len() {
        if lower(b[i]) || b[i].is_ascii_uppercase() {
            i += 1;
        } else if b[i] == b'_' && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
            i += 2;
        } else {
            // A trailing `_` is accepted; anything else is not.
            return b[i] == b'_' && i + 1 == b.len();
        }
    }
    true
}

struct Ctx<'a> {
    u: Unidade<'a>,
    interner: &'a Interner,
    enabled: &'a dyn Fn(&str) -> bool,
    out: Vec<LintReport>,
}

impl<'a> Ctx<'a> {
    fn text(&self, n: ast::Name) -> &'a str {
        self.interner.resolve(n.sym)
    }

    fn report(&mut self, code: &'static CodigoLint, span: Span, args: &[&str]) {
        self.out.push(LintReport { code, span, args: args.iter().map(|a| a.to_string()).collect() });
    }

    /// `camel_case_types` and `camel_case_extensions`.
    fn type_names(&mut self) {
        let (types, extensions) = ((self.enabled)("camel_case_types"), (self.enabled)("camel_case_extensions"));
        for &d in self.u.unit.declarations.iter() {
            let decl = self.u.ast.decl(d);
            if decl.augment {
                continue;
            }
            let (name, from_extension) = match &decl.kind {
                DeclKind::Class(x) => (Some(x.name), false),
                DeclKind::Mixin(x) => (Some(x.name), false),
                DeclKind::Enum(x) => (Some(x.name), false),
                DeclKind::ExtensionType(x) => (Some(x.name), false),
                DeclKind::Typedef(x) => (Some(x.name), false),
                DeclKind::Extension(x) => (x.name, true),
                _ => (None, false),
            };
            let Some(name) = name else { continue };
            let texto = self.text(name);
            if is_camel_case(texto) {
                continue;
            }
            if from_extension && extensions {
                self.report(&c::CAMEL_CASE_EXTENSIONS, name.span, &[texto]);
            } else if !from_extension && types {
                self.report(&c::CAMEL_CASE_TYPES, name.span, &[texto]);
            }
        }
    }

    fn identifier(&mut self, name: ast::Name, underscores_allowed: bool) {
        let texto = self.text(name);
        if underscores_allowed && !texto.is_empty() && texto.bytes().all(|b| b == b'_') {
            return;
        }
        if !is_lower_camel_case(texto) {
            self.report(&c::NON_CONSTANT_IDENTIFIER_NAMES, name.span, &[texto]);
        }
    }

    /// `visitFormalParameterList`: the names of every parameter that is not
    /// `this.x` (`super.x` counts), with underscores allowed; in an
    /// augmentation's list, the named ones are left out. The nested lists (of
    /// function-typed parameters) are visited without the augmentation rule.
    fn params(&mut self, list: &'a [ast::Parameter], in_augmentation: bool) {
        for p in list {
            if !(in_augmentation && p.kind == ast::ParameterKind::Named)
                && let (false, Some(n)) = (p.this_, p.name)
            {
                self.identifier(n, true);
            }
            if let Some(inner) = &p.function_parameters {
                self.params(inner, false);
            }
        }
    }

    /// The name of a record type's positional field written after type `t`
    /// (the tree does not keep it): the identifier the parser consumed.
    fn positional_name(&self, t: ast::TypeId) -> Option<(Span, &'a str)> {
        let source = self.u.fonte;
        let b = source.as_bytes();
        let i = dartforge_frontend::fonte::pular_brancos(b, self.u.ast.ty(t).span.end);
        let identificador = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
        if !b.get(i).is_some_and(|&c| identificador(c) && !c.is_ascii_digit()) {
            return None;
        }
        let mut j = i;
        while j < b.len() && identificador(b[j]) {
            j += 1;
        }
        Some((Span { start: i, end: j }, &source[i..j]))
    }

    /// `non_constant_identifier_names`: the original's twelve visitors.
    fn non_constant_names(&mut self) {
        let a = self.u.ast;
        // The functions of `augment` declarations (their parameter list skips
        // the named ones; their name is not looked at).
        let mut aumentadas: HashSet<ast::FunctionId> = HashSet::new();
        for d in a.decls.iter().filter(|d| d.augment) {
            if let DeclKind::Function(f) = &d.kind {
                aumentadas.insert(*f);
            }
        }
        for m in a.members.iter().filter(|m| m.augment) {
            if let MemberKind::Method(f) = &m.kind {
                aumentadas.insert(*f);
            }
        }
        // `visitVariableDeclaration`/`visitVariableDeclarationStatement`: the
        // lists without `const`, outside augmentations.
        let mut lists: Vec<&'a ast::VariableList> = Vec::new();
        // `visitForEachPartsWithDeclaration`: the `for-in` variable.
        let mut from_loop: Vec<ast::Name> = Vec::new();
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) => lists.push(l),
                StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => lists.push(l),
                StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, .. } => from_loop.push(*name),
                // `visitCatchClause`.
                StmtKind::Try { catches, .. } => {
                    for k in catches.iter() {
                        for n in k.exception.iter().chain(k.stack_trace.iter()) {
                            self.identifier(*n, true);
                        }
                    }
                }
                _ => {}
            }
        }
        // The collection `for` and `for-in`.
        fn in_collection<'b>(el: &'b ast::CollectionElement, lists: &mut Vec<&'b ast::VariableList>, from_loop: &mut Vec<ast::Name>) {
            match el {
                ast::CollectionElement::For { init, body, .. } => {
                    if let Some(ast::ForInit::Variables(l)) = init {
                        lists.push(l);
                    }
                    in_collection(body, lists, from_loop);
                }
                ast::CollectionElement::ForIn { target, body, .. } => {
                    if let ForInTarget::Declared { name, .. } = target {
                        from_loop.push(*name);
                    }
                    in_collection(body, lists, from_loop);
                }
                ast::CollectionElement::If { then, else_, .. } => {
                    in_collection(then, lists, from_loop);
                    if let Some(x) = else_ {
                        in_collection(x, lists, from_loop);
                    }
                }
                _ => {}
            }
        }
        for e in a.exprs.iter() {
            if let ast::ExprKind::List { elements, .. } | ast::ExprKind::SetOrMap { elements, .. } = &e.kind {
                for el in elements.iter() {
                    in_collection(el, &mut lists, &mut from_loop);
                }
            }
        }
        for d in a.decls.iter().filter(|d| !d.augment) {
            if let DeclKind::Variables(l) = &d.kind {
                lists.push(l);
            }
        }
        for m in a.members.iter().filter(|m| !m.augment) {
            if let MemberKind::Field(l) = &m.kind {
                lists.push(l);
            }
        }
        for l in lists {
            if !l.const_ {
                for v in l.variables.iter() {
                    self.identifier(v.name, false);
                }
            }
        }
        for n in from_loop {
            self.identifier(n, false);
        }
        // `visitDeclaredVariablePattern` and `visitPatternField`.
        for n in pattern_declared_variables(a) {
            self.identifier(n, false);
        }
        // `visitExtensionTypeDeclaration`: the name of the representation's
        // constructor.
        for d in a.decls.iter() {
            if let DeclKind::ExtensionType(x) = &d.kind
                && let Some(n) = x.constructor
            {
                self.identifier(n, false);
            }
        }
        // `visitConstructorDeclaration` (outside augmentations) and the
        // constructor's parameter list.
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                if !m.augment
                    && let Some(n) = k.name
                {
                    self.identifier(n, true);
                }
                self.params(&k.parameters, m.augment);
            }
        }
        // `visitFunctionDeclaration` and `visitMethodDeclaration` (not an
        // operator, outside augmentations), and the parameter lists.
        for (k, f) in a.functions.iter().enumerate() {
            let aumentada = aumentadas.contains(&ast::FunctionId(k as u32));
            if let (Some(n), false, false) = (f.name, f.kind == FunctionKind::Operator, aumentada) {
                self.identifier(n, false);
            }
            if let Some(ps) = &f.parameters {
                self.params(ps, aumentada);
            }
        }
        // The lists of old-style typedefs and of `Function(…)` types;
        // `visitRecordTypeAnnotation`: the field names.
        for d in a.decls.iter() {
            if let DeclKind::Typedef(x) = &d.kind
                && let TypedefKind::Legacy { parameters, .. } = &x.kind
            {
                self.params(parameters, false);
            }
        }
        for t in a.types.iter() {
            match &t.kind {
                ast::TypeKind::Function { parameters, .. } => self.params(parameters, false),
                ast::TypeKind::Record { positional, named } => {
                    for &p in positional.iter() {
                        if let Some((span, texto)) = self.positional_name(p)
                            && !is_lower_camel_case(texto)
                        {
                            self.report(&c::NON_CONSTANT_IDENTIFIER_NAMES, span, &[texto]);
                        }
                    }
                    for (n, _) in named.iter() {
                        self.identifier(*n, false);
                    }
                }
                _ => {}
            }
        }
        // `visitRecordLiteral`: the labels of the named fields.
        for e in a.exprs.iter() {
            if let ast::ExprKind::Record { named, .. } = &e.kind {
                for (n, _) in named.iter() {
                    self.identifier(*n, false);
                }
            }
        }
    }

    /// `empty_catches`, `avoid_empty_else` and
    /// `curly_braces_in_flow_control_structures`.
    fn stmts(&mut self) {
        let a = self.u.ast;
        let source = self.u.fonte;
        let (empty, empty_else, keys) =
            ((self.enabled)("empty_catches"), (self.enabled)("avoid_empty_else"), (self.enabled)("curly_braces_in_flow_control_structures"));
        let is_block = |s: StmtId| matches!(a.stmt(s).kind, StmtKind::Block(_));
        let line = |offset: usize| source.as_bytes()[..offset.min(source.len())].iter().filter(|b| **b == b'\n').count();
        // The `if`s that are the `else` of another `if`.
        let from_else: Vec<StmtId> = a
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::If { else_: Some(e), .. } if matches!(a.stmt(*e).kind, StmtKind::If { .. }) => Some(*e),
                _ => None,
            })
            .collect();
        for (i, s) in a.stmts.iter().enumerate() {
            match &s.kind {
                StmtKind::Try { catches, .. } if empty => {
                    for k in catches.iter() {
                        let only_underscores = k.exception.is_some_and(|n| {
                            let t = self.text(n);
                            !t.is_empty() && t.bytes().all(|b| b == b'_')
                        });
                        let body = a.stmt(k.body);
                        let without_stmts = matches!(&body.kind, StmtKind::Block(l) if l.is_empty());
                        // No comment before the `}`: only blanks between the braces.
                        let miolo = source.get(body.span.start + 1..body.span.end.saturating_sub(1)).unwrap_or("x");
                        if !only_underscores && without_stmts && miolo.trim().is_empty() {
                            self.report(&c::EMPTY_CATCHES, body.span, &[]);
                        }
                    }
                }
                StmtKind::If { then, else_, .. } => {
                    if empty_else
                        && let Some(e) = else_
                        && matches!(a.stmt(*e).kind, StmtKind::Empty)
                        && a.stmt(*e).span.end > a.stmt(*e).span.start
                    {
                        self.report(&c::AVOID_EMPTY_ELSE, a.stmt(*e).span, &[]);
                    }
                    if !keys {
                        continue;
                    }
                    match else_ {
                        None => {
                            if from_else.contains(&StmtId(i as u32)) {
                                if !is_block(*then) {
                                    self.report(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*then).span, &["an if"]);
                                }
                            } else if !is_block(*then) && line(s.span.start) != line(a.stmt(*then).span.end) {
                                // An `if` without `else` on a single line is accepted.
                                self.report(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*then).span, &["an if"]);
                            }
                        }
                        Some(e) => {
                            if !is_block(*then) {
                                self.report(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*then).span, &["an if"]);
                            }
                            if !is_block(*e) && !matches!(a.stmt(*e).kind, StmtKind::If { .. }) {
                                self.report(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*e).span, &["an if"]);
                            }
                        }
                    }
                }
                StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } if keys && !is_block(*body) => {
                    self.report(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*body).span, &["a for"]);
                }
                StmtKind::While { body, .. } if keys && !is_block(*body) => {
                    self.report(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*body).span, &["a while"]);
                }
                StmtKind::DoWhile { body, .. } if keys && !is_block(*body) => {
                    self.report(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*body).span, &["a do"]);
                }
                _ => {}
            }
        }
    }

    /// `use_string_in_part_of_directives` and `avoid_relative_lib_imports`.
    fn directives(&mut self) {
        let (part_of, relativo) = ((self.enabled)("use_string_in_part_of_directives"), (self.enabled)("avoid_relative_lib_imports"));
        for d in self.u.unit.directives.iter() {
            match &d.kind {
                DirectiveKind::PartOf { uri: None, name } if part_of && !name.is_empty() => {
                    self.report(&c::USE_STRING_IN_PART_OF_DIRECTIVES, d.span, &[]);
                }
                DirectiveKind::Import { uri, .. } if relativo => {
                    // No scheme and `/lib/` in the path.
                    let Some(texto) = dartforge_elements::load::string_lit_value(uri) else { continue };
                    let without_scheme = !texto.split(['/', '?', '#']).next().is_some_and(|p| p.contains(':'));
                    let path = texto.split(['?', '#']).next().unwrap_or("");
                    if without_scheme && path.contains("/lib/") {
                        self.report(&c::AVOID_RELATIVE_LIB_IMPORTS, uri.span, &[]);
                    }
                }
                _ => {}
            }
        }
    }

    /// `prefer_generic_function_type_aliases`: the old-style `typedef` (with
    /// the `;` written), with the suggestion built by the `toSource` of the
    /// return type, the type parameters and the parameters.
    fn old_typedefs(&mut self) {
        let a = self.u.ast;
        let source = self.u.fonte;
        let interner = self.interner;
        for &d in self.u.unit.declarations.iter() {
            let decl = a.decl(d);
            let DeclKind::Typedef(x) = &decl.kind else { continue };
            let TypedefKind::Legacy { return_type, parameters } = &x.kind else { continue };
            // `node.semicolon.isSynthetic`: without the `;`, nothing.
            if !source.get(..decl.span.end).is_some_and(|t| t.trim_end().ends_with(';')) {
                continue;
            }
            let ret = match return_type {
                Some(t) => format!("{} ", dartforge_frontend::fonte::de_tipo(a, source, interner, *t)),
                None => String::new(),
            };
            let types = dartforge_frontend::fonte::de_parametros_de_tipo(a, source, interner, &x.type_params);
            let list = dartforge_frontend::fonte::de_parametros(a, source, interner, parameters);
            let sugestao = format!("{ret}Function{types}{list}");
            self.report(&c::PREFER_GENERIC_FUNCTION_TYPE_ALIASES, x.name.span, &[sugestao.as_str()]);
        }
    }

    /// `provide_deprecation_message`: every annotation without arguments whose
    /// element is `dart:core`'s `deprecated` (`isDeprecated`). Needs the unit's
    /// semantics.
    fn deprecated_without_message(&mut self, without: Option<&super::Semantics<'_>>) {
        let Some(without) = without else { return };
        let interner = self.interner;
        let findings: Vec<Span> = dartforge_frontend::pais::todas_as_anotacoes(self.u.ast, self.u.unit)
            .into_iter()
            .filter(|m| m.arguments.is_none() && dartforge_types::anotacoes::e_deprecated_do_core(without.program, interner, without.unit, m))
            .map(|m| m.span)
            .collect();
        for span in findings {
            self.report(&c::PROVIDE_DEPRECATION_MESSAGE, span, &[]);
        }
    }
}

/// Runs the enabled rules (`enabled(name)`) over a unit.
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, without: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut ctx = Ctx { u, interner, enabled, out: Vec::new() };
    if enabled("camel_case_types") || enabled("camel_case_extensions") {
        ctx.type_names();
    }
    if enabled("non_constant_identifier_names") {
        ctx.non_constant_names();
    }
    if enabled("empty_catches") || enabled("avoid_empty_else") || enabled("curly_braces_in_flow_control_structures") {
        ctx.stmts();
    }
    if enabled("use_string_in_part_of_directives") || enabled("avoid_relative_lib_imports") {
        ctx.directives();
    }
    if enabled("prefer_generic_function_type_aliases") {
        ctx.old_typedefs();
    }
    if enabled("provide_deprecation_message") {
        ctx.deprecated_without_message(without);
    }
    ctx.out.sort_by_key(|r| (r.span.start, r.span.end));
    ctx.out
}

#[cfg(test)]
mod testes {
    use super::*;

    /// `(rule, range text)` of each report, with all rules enabled.
    fn findings(source: &str) -> Vec<(&'static str, String)> {
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte: source };
        run(u, &names, &|_| true, None).into_iter().map(|r| (r.code.nome, source[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn formas_dos_nomes() {
        assert!(is_camel_case("Foo") && is_camel_case("_Foo") && is_camel_case("$Foo") && is_camel_case("F"));
        assert!(!is_camel_case("foo") && !is_camel_case("Foo_Bar"));
        assert!(is_lower_camel_case("foo") && is_lower_camel_case("_fooBar") && is_lower_camel_case("foo_1") && is_lower_camel_case("A"));
        assert!(!is_lower_camel_case("Foo") && !is_lower_camel_case("foo_bar") && !is_lower_camel_case("__"));
    }

    #[test]
    fn nomes() {
        assert_eq!(findings("class foo {}\n"), vec![("camel_case_types", "foo".to_string())]);
        assert_eq!(findings("extension minha on int {}\n"), vec![("camel_case_extensions", "minha".to_string())]);
        assert_eq!(findings("void Faz(int um_dois) {}\n"), vec![
            ("non_constant_identifier_names", "Faz".to_string()),
            ("non_constant_identifier_names", "um_dois".to_string()),
        ]);
        assert!(findings("const MAX = 1;\nvoid f(int _, int __) {}\n").is_empty());
    }

    #[test]
    fn comandos_de_fluxo() {
        assert_eq!(findings("void f() {\n  try {} catch (e) {}\n}\n"), vec![("empty_catches", "{}".to_string())]);
        assert!(findings("void f() {\n  try {} catch (_) {}\n  try {} catch (e) {\n    // nada\n  }\n}\n").is_empty());
        assert_eq!(
            findings("void f(bool b) {\n  if (b) return;\n  if (b)\n    return;\n  while (b) f(b);\n}\n"),
            vec![
                ("curly_braces_in_flow_control_structures", "return;".to_string()),
                ("curly_braces_in_flow_control_structures", "f(b);".to_string()),
            ]
        );
        assert_eq!(findings("void f(bool b) {\n  if (b) {} else ;\n}\n"), vec![
            ("avoid_empty_else", ";".to_string()),
            ("curly_braces_in_flow_control_structures", ";".to_string()),
        ]);
    }

    #[test]
    fn diretivas_e_typedefs() {
        assert_eq!(findings("import '../lib/a.dart';\n"), vec![("avoid_relative_lib_imports", "'../lib/a.dart'".to_string())]);
        assert!(findings("import 'package:a/lib/a.dart';\n").is_empty());
        let reports = {
            let source = "typedef int F(int x);\n";
            let mut names = Interner::new();
            let p = dartforge_frontend::parser::parse(source, &mut names);
            run(Unidade { ast: &p.ast, unit: &p.unit, fonte: source }, &names, &|_| true, None)
        };
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].args, vec!["int Function(int x)".to_string()]);
        // Without the unit's semantics, the annotation's element is not known.
        assert!(findings("@deprecated\nvoid f() {}\n").is_empty());
    }
}
