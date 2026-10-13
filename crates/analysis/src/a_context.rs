//! Family A checks that depend only on the tree and on the syntactic context
//! the node appears in (docs/ANALYZER-ESPECIFICACAO.md §A):
//!
//! * `super_in_invalid_context`, `super_in_extension`,
//!   `super_in_extension_type` — the analyzer's `SuperContext.of`
//!   (`generated/super_context.dart`, reported in
//!   `element_resolver.dart:404-424`);
//! * `async_for_in_wrong_context` — `await for` outside an `async` function;
//! * `await_in_late_local_variable_initializer`;
//! * `expected_one_list_type_arguments`, `expected_one_set_type_arguments`,
//!   `expected_two_map_type_arguments` (`type_arguments_verifier.dart:160-221`);
//! * `extension_type_constructor_with_super_invocation` and
//!   `extension_type_constructor_with_super_formal_parameter`
//!   (`error_verifier.dart:3393-3402`, `:1456-1466`).
//!
//! Written without compiling or running (2026-10-04).

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::{Codigo, Diagnostic, Span};
use dartforge_frontend::ast::{
    self, AsyncModifier, CollectionElement, DeclKind, ExprId, ExprKind, FunctionBody, MemberId, MemberKind, PatternId,
    PatternKind, StmtId, StmtKind,
};

/// The context of a `super` (`SuperContext`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sup {
    /// An instance member of a class, enum or mixin.
    Valid,
    /// Top level, static member, factory constructor, constructor initializer,
    /// static or non-`late` field initializer.
    Static,
    Extension,
    ExtensionType,
}

/// The type that contains the visited member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Class,
    Extension,
    ExtensionType,
}

impl Owner {
    fn sup(self) -> Sup {
        match self {
            Owner::Class => Sup::Valid,
            Owner::Extension => Sup::Extension,
            Owner::ExtensionType => Sup::ExtensionType,
        }
    }
}

struct Visitor<'a> {
    a: &'a ast::Ast,
    source: &'a str,
    out: Vec<Diagnostic>,
    sup: Sup,
    /// The body modifier of the innermost function.
    modifier: AsyncModifier,
    /// Inside the initializer of a `late` local variable (a function literal
    /// in between turns it off).
    in_late_local: bool,
}

/// The context diagnostics of a unit.
pub fn check(u: Unidade<'_>) -> Vec<Diagnostic> {
    let mut v = Visitor { a: u.ast, source: u.fonte, out: Vec::new(), sup: Sup::Static, modifier: AsyncModifier::None, in_late_local: false };
    for &d in u.unit.declarations.iter() {
        v.decl(d);
    }
    v.out
}

impl<'a> Visitor<'a> {
    fn report(&mut self, code: Codigo, span: Span, args: &[&str]) {
        self.out.push(Diagnostic::com_codigo(code, span, args.iter().copied()));
    }

    // -- Declarations -------------------------------------------------------------

    fn decl(&mut self, d: ast::DeclId) {
        let a = self.a;
        match &a.decl(d).kind {
            DeclKind::Variables(l) => {
                self.sup = Sup::Static;
                for var in l.variables.iter() {
                    if let Some(init) = var.initializer {
                        self.expr(init);
                    }
                }
            }
            DeclKind::Function(f) => {
                self.sup = Sup::Static;
                self.function(*f);
            }
            DeclKind::Class(k) => self.members(&k.members, Owner::Class),
            DeclKind::Mixin(k) => self.members(&k.members, Owner::Class),
            DeclKind::Enum(k) => {
                // The arguments of an enum constant are evaluated outside any
                // member: there is no instance context.
                self.sup = Sup::Valid;
                for cst in k.constants.iter() {
                    if let Some(args) = &cst.arguments {
                        for x in args.args.iter() {
                            self.expr(x.value);
                        }
                    }
                }
                self.members(&k.members, Owner::Class);
            }
            DeclKind::Extension(k) => self.members(&k.members, Owner::Extension),
            DeclKind::ExtensionType(k) => self.members(&k.members, Owner::ExtensionType),
            DeclKind::Typedef(_) => {}
        }
        self.sup = Sup::Static;
    }

    fn members(&mut self, members: &'a [MemberId], owner: Owner) {
        let a = self.a;
        for &mid in members {
            match &a.member(mid).kind {
                MemberKind::Field(l) => {
                    // A static field, or an instance one that is not `late`:
                    // the initializer runs without `this`.
                    self.sup = if l.static_ || !l.late { Sup::Static } else { owner.sup() };
                    for var in l.variables.iter() {
                        if let Some(init) = var.initializer {
                            self.expr(init);
                        }
                    }
                }
                MemberKind::Method(f) => {
                    self.sup = if a.function(*f).static_ { Sup::Static } else { owner.sup() };
                    self.function(*f);
                }
                MemberKind::Constructor(k) => {
                    let body_sup = if k.factory { Sup::Static } else { owner.sup() };
                    // Parameters: the default values and, in an extension
                    // type, the `super.x` it cannot declare.
                    self.sup = body_sup;
                    for p in k.parameters.iter() {
                        if owner == Owner::ExtensionType && p.super_ {
                            if let Some(n) = p.name {
                                // `super.x`: the word sits six bytes before the name.
                                let start = n.span.start.saturating_sub(6);
                                if self.source.get(start..start + 5) == Some("super") {
                                    self.report(c::EXTENSION_TYPE_CONSTRUCTOR_WITH_SUPER_FORMAL_PARAMETER, Span { start: start, end: start + 5 }, &[]);
                                }
                            }
                        }
                    }
                    self.params(&k.parameters);
                    // Every constructor initializer is a static context.
                    self.sup = Sup::Static;
                    for init in k.initializers.iter() {
                        match init {
                            ast::Initializer::Field { value, .. } => self.expr(*value),
                            ast::Initializer::Assert { condition, message, .. } => {
                                self.expr(*condition);
                                if let Some(m) = message {
                                    self.expr(*m);
                                }
                            }
                            ast::Initializer::Super { span, arguments, .. } => {
                                if owner == Owner::ExtensionType {
                                    let word = self.word(*span, "super");
                                    self.report(c::EXTENSION_TYPE_CONSTRUCTOR_WITH_SUPER_INVOCATION, word, &[]);
                                }
                                for x in arguments.args.iter() {
                                    self.expr(x.value);
                                }
                            }
                            ast::Initializer::Redirect { arguments, .. } => {
                                for x in arguments.args.iter() {
                                    self.expr(x.value);
                                }
                            }
                        }
                    }
                    self.sup = body_sup;
                    let before = std::mem::replace(&mut self.modifier, AsyncModifier::None);
                    self.body(&k.body);
                    self.modifier = before;
                }
            }
        }
    }

    /// The word `text` inside `span` (the start of the range, when it is not
    /// found).
    fn word(&self, span: Span, text: &str) -> Span {
        let start = self
            .source
            .get(span.start..span.end.min(self.source.len()))
            .and_then(|t| t.find(text))
            .map_or(span.start, |i| span.start + i);
        Span { start: start, end: start + text.len() }
    }

    fn params(&mut self, ps: &'a [ast::Parameter]) {
        for p in ps {
            if let Some(fp) = &p.function_parameters {
                self.params(fp);
            }
            if let Some(d) = p.default_value {
                self.expr(d);
            }
        }
    }

    /// A function (declared or literal): the body has its modifier, and the
    /// outer `late` initializer does not apply inside it.
    fn function(&mut self, f: ast::FunctionId) {
        let a = self.a;
        let func = a.function(f);
        if let Some(ps) = &func.parameters {
            self.params(ps);
        }
        let modifier = std::mem::replace(&mut self.modifier, func.modifier);
        let in_late = std::mem::replace(&mut self.in_late_local, false);
        self.body(&func.body);
        self.modifier = modifier;
        self.in_late_local = in_late;
    }

    fn body(&mut self, b: &FunctionBody) {
        match b {
            FunctionBody::Block(s) => self.stmt(*s),
            FunctionBody::Expression(e) => self.expr(*e),
            _ => {}
        }
    }

    // -- Comandos ------------------------------------------------------------------

    /// `await for` outside an `async`/`async*` function: on the `await` token.
    fn await_for(&mut self, start: usize) {
        if matches!(self.modifier, AsyncModifier::Async | AsyncModifier::AsyncStar) {
            return;
        }
        let word = self.word(Span { start: start, end: start + 16 }, "await");
        self.report(c::ASYNC_FOR_IN_WRONG_CONTEXT, word, &[]);
    }

    fn local_list(&mut self, l: &'a ast::VariableList) {
        let before = self.in_late_local;
        if l.late {
            self.in_late_local = true;
        }
        for var in l.variables.iter() {
            if let Some(init) = var.initializer {
                self.expr(init);
            }
        }
        self.in_late_local = before;
    }

    fn for_init(&mut self, init: &'a Option<ast::ForInit>) {
        match init {
            Some(ast::ForInit::Variables(l)) => self.local_list(l),
            Some(ast::ForInit::Expression(e)) => self.expr(*e),
            Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                self.pattern(*pattern);
                self.expr(*value);
            }
            None => {}
        }
    }

    fn for_in_target(&mut self, target: &'a ast::ForInTarget) {
        match target {
            ast::ForInTarget::Declared { .. } => {}
            ast::ForInTarget::Pattern { pattern, .. } => self.pattern(*pattern),
            ast::ForInTarget::Expression(e) => self.expr(*e),
        }
    }

    fn stmt(&mut self, s: StmtId) {
        let a = self.a;
        let node = a.stmt(s);
        match &node.kind {
            StmtKind::Block(xs) => {
                for x in xs.iter() {
                    self.stmt(*x);
                }
            }
            StmtKind::Variables(l) => self.local_list(l),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.pattern(*pattern);
                self.expr(*value);
            }
            StmtKind::Function(f) => self.function(*f),
            StmtKind::Expression(e) => self.expr(*e),
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.pattern(*p);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.stmt(*then);
                if let Some(e) = else_ {
                    self.stmt(*e);
                }
            }
            StmtKind::For { init, condition, updates, body, .. } => {
                self.for_init(init);
                if let Some(x) = condition {
                    self.expr(*x);
                }
                for x in updates.iter() {
                    self.expr(*x);
                }
                self.stmt(*body);
            }
            StmtKind::ForIn { await_, target, iterable, body } => {
                if *await_ {
                    self.await_for(node.span.start);
                }
                self.for_in_target(target);
                self.expr(*iterable);
                self.stmt(*body);
            }
            StmtKind::While { condition, body } => {
                self.expr(*condition);
                self.stmt(*body);
            }
            StmtKind::DoWhile { body, condition } => {
                self.stmt(*body);
                self.expr(*condition);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(*value);
                for case in cases.iter() {
                    if let Some(p) = case.pattern {
                        self.pattern(p);
                    }
                    if let Some(g) = case.guard {
                        self.expr(g);
                    }
                    for x in case.body.iter() {
                        self.stmt(*x);
                    }
                }
            }
            StmtKind::Return(Some(e)) => self.expr(*e),
            StmtKind::Yield { value, .. } => self.expr(*value),
            StmtKind::Try { body, catches, finally_ } => {
                self.stmt(*body);
                for k in catches.iter() {
                    self.stmt(k.body);
                }
                if let Some(f) = finally_ {
                    self.stmt(*f);
                }
            }
            StmtKind::Labeled { body, .. } => self.stmt(*body),
            StmtKind::Assert { condition, message } => {
                self.expr(*condition);
                if let Some(m) = message {
                    self.expr(*m);
                }
            }
            _ => {}
        }
    }

    // -- Patterns -------------------------------------------------------------------

    fn pattern(&mut self, p: PatternId) {
        let a = self.a;
        match &a.pattern(p).kind {
            PatternKind::Constant(e) => self.expr(*e),
            PatternKind::Relational { value, .. } => self.expr(*value),
            PatternKind::Or(x, y) | PatternKind::And(x, y) => {
                self.pattern(*x);
                self.pattern(*y);
            }
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => self.pattern(*x),
            PatternKind::Cast { pattern, .. } => self.pattern(*pattern),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => self.pattern(*x),
                        _ => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for en in entries.iter() {
                    self.expr(en.key);
                    self.pattern(en.value);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.pattern(f.pattern);
                }
            }
            _ => {}
        }
    }

    // -- Expressions ----------------------------------------------------------------

    fn element(&mut self, el: &'a CollectionElement, collection_start: usize) {
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => self.expr(*x),
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(*key);
                self.expr(*value);
            }
            CollectionElement::Spread { value, .. } => self.expr(*value),
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.pattern(*p);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.element(then, collection_start);
                if let Some(x) = else_ {
                    self.element(x, collection_start);
                }
            }
            CollectionElement::For { init, condition, updates, body, .. } => {
                self.for_init(init);
                if let Some(x) = condition {
                    self.expr(*x);
                }
                for x in updates.iter() {
                    self.expr(*x);
                }
                self.element(body, collection_start);
            }
            CollectionElement::ForIn { await_, target, iterable, body } => {
                if *await_ {
                    // The element does not keep its own range: the `await` is
                    // the last one before the iterable.
                    let end = self.a.expr(*iterable).span.start;
                    let fragment = self.source.get(collection_start..end).unwrap_or("");
                    if !matches!(self.modifier, AsyncModifier::Async | AsyncModifier::AsyncStar) {
                        if let Some(i) = fragment.rfind("await") {
                            let start = collection_start + i;
                            self.report(c::ASYNC_FOR_IN_WRONG_CONTEXT, Span { start: start, end: start + 5 }, &[]);
                        }
                    }
                }
                self.for_in_target(target);
                self.expr(*iterable);
                self.element(body, collection_start);
            }
        }
    }

    /// The `<…>` type-argument list of a literal: from the `<` before the
    /// first argument to the `>` after the last.
    fn type_list(&self, types: &[ast::TypeId]) -> Option<Span> {
        let first = self.a.ty(*types.first()?).span;
        let last = self.a.ty(*types.last()?).span;
        let bytes = self.source.as_bytes();
        let mut start = first.start;
        while start > 0 && bytes.get(start - 1).is_some_and(|b| b.is_ascii_whitespace()) {
            start -= 1;
        }
        if start == 0 || bytes.get(start - 1) != Some(&b'<') {
            return None;
        }
        let mut end = last.end;
        while bytes.get(end).is_some_and(|b| b.is_ascii_whitespace()) {
            end += 1;
        }
        if bytes.get(end) != Some(&b'>') {
            return None;
        }
        Some(Span { start: start - 1, end: end + 1 })
    }

    fn expr(&mut self, e: ExprId) {
        let a = self.a;
        let node = a.expr(e);
        match &node.kind {
            ExprKind::Super => {
                let code = match self.sup {
                    Sup::Valid => None,
                    Sup::Static => Some(c::SUPER_IN_INVALID_CONTEXT),
                    Sup::Extension => Some(c::SUPER_IN_EXTENSION),
                    Sup::ExtensionType => Some(c::SUPER_IN_EXTENSION_TYPE),
                };
                if let Some(code) = code {
                    self.report(code, node.span, &[]);
                }
            }
            ExprKind::Await(x) => {
                if self.in_late_local {
                    let word = Span { start: node.span.start, end: node.span.start + 5 };
                    self.report(c::AWAIT_IN_LATE_LOCAL_VARIABLE_INITIALIZER, word, &[]);
                }
                self.expr(*x);
            }
            ExprKind::List { type_args, elements, .. } => {
                if type_args.len() > 1 {
                    if let Some(sp) = self.type_list(type_args) {
                        let n = type_args.len().to_string();
                        self.report(c::EXPECTED_ONE_LIST_TYPE_ARGUMENTS, sp, &[n.as_str()]);
                    }
                }
                for el in elements.iter() {
                    self.element(el, node.span.start);
                }
            }
            ExprKind::SetOrMap { type_args, elements, .. } => {
                // With one argument it is a set and with two a map; with three
                // or more, the shape comes from the elements (an empty `{}` is a map).
                if type_args.len() > 2 {
                    if let Some(sp) = self.type_list(type_args) {
                        let n = type_args.len().to_string();
                        let has_entry = has_map_entry(elements);
                        let is_set = !has_entry && !elements.is_empty();
                        let code = if is_set { c::EXPECTED_ONE_SET_TYPE_ARGUMENTS } else { c::EXPECTED_TWO_MAP_TYPE_ARGUMENTS };
                        self.report(code, sp, &[n.as_str()]);
                    }
                }
                for el in elements.iter() {
                    self.element(el, node.span.start);
                }
            }
            ExprKind::Record { positional, named, .. } => {
                for x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                    self.expr(*x);
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => {
                for x in arguments.args.iter() {
                    self.expr(x.value);
                }
            }
            ExprKind::FunctionExpression(f) => self.function(*f),
            ExprKind::String(lit) => {
                for p in lit.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(*x);
                    }
                }
            }
            ExprKind::Parenthesized(x) | ExprKind::Throw(x) => self.expr(*x),
            ExprKind::Property { target, .. } => self.expr(*target),
            ExprKind::Index { target, index, .. } => {
                self.expr(*target);
                self.expr(*index);
            }
            ExprKind::Call { target, arguments } => {
                self.expr(*target);
                for x in arguments.args.iter() {
                    self.expr(x.value);
                }
            }
            ExprKind::TypeArguments { target, .. } => self.expr(*target),
            ExprKind::Unary { operand, .. } => self.expr(*operand),
            ExprKind::Binary { left, right, .. } => {
                self.expr(*left);
                self.expr(*right);
            }
            ExprKind::Conditional { condition, then, else_ } => {
                self.expr(*condition);
                self.expr(*then);
                self.expr(*else_);
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.expr(*value),
            ExprKind::Assign { target, value, .. } => {
                self.expr(*target);
                self.expr(*value);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.pattern(*pattern);
                self.expr(*value);
            }
            ExprKind::Cascade { target, sections, .. } => {
                self.expr(*target);
                for s in sections.iter() {
                    self.expr(*s);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value);
                for case in cases.iter() {
                    self.pattern(case.pattern);
                    if let Some(g) = case.guard {
                        self.expr(g);
                    }
                    self.expr(case.body);
                }
            }
            _ => {}
        }
    }
}

/// Some top-level element of the literal (going through `if`/`for`) is a
/// `key: value` entry.
fn has_map_entry(elements: &[CollectionElement]) -> bool {
    fn has(el: &CollectionElement) -> bool {
        match el {
            CollectionElement::MapEntry { .. } => true,
            CollectionElement::If { then, else_, .. } => has(then) || else_.as_deref().is_some_and(has),
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => has(body),
            _ => false,
        }
    }
    elements.iter().any(has)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(source: &str) -> Vec<(&'static str, usize, usize)> {
        let mut names = dartforge_intern::Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte: source };
        check(u).into_iter().map(|d| (d.code.map_or("", |c| c.info().nome), d.span.start, d.span.end)).collect()
    }

    #[test]
    fn super_in_instance_method_is_valid() {
        assert!(codes("class A { void m() { super.toString(); } }").is_empty());
    }

    #[test]
    fn super_in_static_context() {
        // 0         1         2         3
        // 0123456789012345678901234567890123456789
        // class A { static void m() { super.x; } }
        let r = codes("class A { static void m() { super.x; } }");
        assert_eq!(r, vec![("super_in_invalid_context", 28, 33)]);
    }

    #[test]
    fn super_in_non_late_field_and_in_initializer() {
        let r = codes("class A { var a = super.hashCode; late var b = super.hashCode; A() : a = super.hashCode; }");
        let names: Vec<&str> = r.iter().map(|x| x.0).collect();
        assert_eq!(names, vec!["super_in_invalid_context", "super_in_invalid_context"]);
    }

    #[test]
    fn super_in_extension_and_extension_type() {
        let r = codes("extension E on int { void m() { super.toString(); } }");
        assert_eq!(r[0].0, "super_in_extension");
        let r = codes("extension type T(int i) { void m() { super.toString(); } }");
        assert_eq!(r[0].0, "super_in_extension_type");
    }

    #[test]
    fn await_for_outside_async() {
        // void f(s) { await for (var x in s) {} }
        let r = codes("void f(s) { await for (var x in s) {} }");
        assert_eq!(r, vec![("async_for_in_wrong_context", 12, 17)]);
        assert!(codes("void f(s) async { await for (var x in s) {} }").is_empty());
    }

    #[test]
    fn await_in_late_local() {
        // void f(g) async { late var x = await g; }
        let r = codes("void f(g) async { late var x = await g; }");
        assert_eq!(r, vec![("await_in_late_local_variable_initializer", 31, 36)]);
        // The function literal in between does not count.
        assert!(codes("void f(g) async { late var x = () async => await g; }").is_empty());
    }

    #[test]
    fn literal_type_arguments() {
        // var a = <int, int>[];
        let r = codes("var a = <int, int>[];");
        assert_eq!(r, vec![("expected_one_list_type_arguments", 8, 18)]);
        let r = codes("var a = <int, int, int>{};");
        assert_eq!(r[0].0, "expected_two_map_type_arguments");
        let r = codes("var a = <int, int, int>{1};");
        assert_eq!(r[0].0, "expected_one_set_type_arguments");
    }
}
