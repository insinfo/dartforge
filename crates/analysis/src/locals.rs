//! Local variables and functions never read: the local part of the
//! `UnusedLocalElementsVerifier` of analyzer 6.11.0
//! (`src/error/unused_local_elements_verifier.dart`).
//!
//! A local's scope is lexical, so resolution is syntactic: each simple
//! identifier is bound to the innermost local with the same name in scope
//! (parameters enter the scope and hide outer ones, but are never reported
//! here). A read is what the analyzer's `_isReadIdentifier` calls a read:
//! * a simple assignment (`x = e`) does not read `x`;
//! * `x++;`, `++x;` and `x += e;` **as a statement** do not read (`x ??= e;`
//!   reads); inside an expression, they read;
//! * everything else is a read.
//!
//! Reported (`_visitLocalVariableElement`):
//! * `UNUSED_LOCAL_VARIABLE`: a local variable declared in a statement, in the
//!   declaration part of a `for`, in a `for-in` or in a pattern (from a pattern
//!   declaration only if **none** of its variables is read);
//! * `UNUSED_ELEMENT`: a local function never referenced.
//!
//! Names made only of `_` (`_`, `__`) are not reported; with
//! `wildcard-variables` (3.7+), `_` does not even declare.

use crate::Unit;
use dartforge_diagnostics::codes::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{
    self, AssignOp, Ast, BinaryOp, CollectionElement, DeclKind, ExprId, ExprKind, ForInTarget, ForInit,
    FunctionBody, FunctionId, Initializer, ListPatternElement, MemberKind, Parameter, PatternId, PatternKind,
    StmtId, StmtKind, StringPart,
};
use dartforge_intern::{Interner, SymbolId};

#[derive(Clone, Copy, PartialEq, Eq)]
enum LocalKind {
    /// A reportable local variable.
    Variable,
    /// A local function.
    Function,
    /// A parameter, a `catch` variable etc.: hides names, is not reported.
    Hidden,
    /// The exception of `on T catch (e)` without a stack (`UNUSED_CATCH_CLAUSE`).
    CatchException,
    /// The stack of `catch (e, s)` (`UNUSED_CATCH_STACK`).
    CatchStack,
}

struct Local {
    name: SymbolId,
    span: Span,
    kind: LocalKind,
    read: bool,
    /// The pattern declaration it belongs to (all or none).
    group: Option<usize>,
    /// The same variable as another one (shared patterns).
    alias: Option<usize>,
    /// Read by its own case's guard (before the join): only it, not the join
    /// (`JoinPatternVariableElement.transitiveVariables` only in the body).
    read_by_own_guard: bool,
}

struct Visitor<'a> {
    ast: &'a Ast,
    interner: &'a Interner,
    wildcard: bool,
    locals: Vec<Local>,
    scopes: Vec<Vec<usize>>,
    source: &'a str,
    groups: usize,
    /// Inside a refutable pattern.
    refutable: bool,
    /// Start (index in `locals`) of a region of shared patterns.
    shared_region: Option<usize>,
    /// The variables of the pattern of the case whose guard is being visited.
    guard_vars: Vec<usize>,
    /// The local function whose body is being visited (the
    /// `GatherUsedLocalElementsVisitor`'s `_enclosingExec`, which only changes
    /// on a `FunctionDeclaration`): a reference to it from inside does not use it.
    executable: Option<usize>,
    /// Start (index in `locals`) of the variables of the pattern being declared.
    pattern_start: Option<usize>,
}

impl<'a> Visitor<'a> {
    fn enter(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn exit(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, n: ast::Name, kind: LocalKind, group: Option<usize>) {
        if self.wildcard && self.interner.resolve(n.sym) == "_" {
            return;
        }
        let i = self.locals.len();
        self.locals.push(Local { name: n.sym, span: n.span, kind, read: false, group, alias: None, read_by_own_guard: false });
        if let Some(e) = self.scopes.last_mut() {
            // `for (int i = 0, i = 0; …)`, `var a; var a;`: the redeclaration
            // (`duplicate_definition`) does not enter the scope — the uses are
            // the first's, and the second, never read, is reported.
            if kind != LocalKind::Hidden && e.iter().any(|&j| self.locals[j].name == n.sym) {
                return;
            }
            e.push(i);
        }
    }

    fn find(&self, name: SymbolId) -> Option<usize> {
        self.scopes.iter().rev().flat_map(|e| e.iter().rev()).copied().find(|&i| self.locals[i].name == name)
    }

    /// A reference to identifier `n`: `read` says whether it reads the value. A
    /// local function counts as used by any reference.
    fn root(&self, mut i: usize) -> usize {
        while let Some(a) = self.locals[i].alias {
            i = a;
        }
        i
    }

    fn refer(&mut self, n: ast::Name, read_: bool) {
        // In the guard, the name is the case's own variable.
        if let Some(&i) = self.guard_vars.iter().rev().find(|&&i| self.locals[i].name == n.sym) {
            if read_ {
                self.locals[i].read_by_own_guard = true;
            }
            return;
        }
        if let Some(i) = self.find(n.sym).map(|i| self.root(i)) {
            // `identical(element, _enclosingExec)`
            // (`unused_local_elements_verifier.dart:383-385`).
            if Some(i) == self.executable {
                return;
            }
            let l = &mut self.locals[i];
            if read_ || l.kind == LocalKind::Function {
                l.read = true;
            }
        }
    }

    // -- Expressions ---------------------------------------------------------

    fn expr(&mut self, e: ExprId) {
        self.expr_ctx(e, false);
    }

    /// `stmt`: the expression is the whole statement (`ExpressionStatement`).
    fn expr_ctx(&mut self, e: ExprId, is_stmt: bool) {
        let ast = self.ast;
        match &ast.expr(e).kind {
            ExprKind::Identifier(n) => self.refer(*n, true),
            ExprKind::String(s) => self.string(s),
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    self.element(el);
                }
            }
            ExprKind::Record { positional, named, .. } => {
                for x in positional.iter() {
                    self.expr(*x);
                }
                for (_, x) in named.iter() {
                    self.expr(*x);
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => self.arguments(arguments),
            ExprKind::FunctionExpression(f) => self.function(*f, false),
            ExprKind::Property { target, .. } => self.expr(*target),
            ExprKind::Index { target, index, .. } => {
                self.expr(*target);
                self.expr(*index);
            }
            ExprKind::Call { target, arguments } => {
                self.expr(*target);
                self.arguments(arguments);
            }
            ExprKind::TypeArguments { target, .. } => self.expr(*target),
            ExprKind::Unary { op, operand } => {
                // `_isReadIdentifier`: the identifier of **any**
                // `PrefixExpression`/`PostfixExpression` (`-x`, `~x`, `!x`,
                // `x!`, `++x`, `x--`) that is the whole statement is not a read.
                let _ = op;
                if let ExprKind::Identifier(n) = &ast.expr(*operand).kind {
                    self.refer(*n, !is_stmt);
                    return;
                }
                self.expr(*operand);
            }
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
            ExprKind::Assign { op, target, value } => {
                match &ast.expr(*target).kind {
                    ExprKind::Identifier(n) => {
                        let read_ = match op {
                            AssignOp::Assign => false,
                            AssignOp::Compound(BinaryOp::IfNull) => true,
                            AssignOp::Compound(_) => !is_stmt,
                        };
                        self.refer(*n, read_);
                    }
                    _ => self.expr(*target),
                }
                self.expr(*value);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.expr(*value);
                self.assignment_pattern(*pattern);
            }
            ExprKind::Cascade { target, sections, .. } => {
                self.expr(*target);
                for s in sections.iter() {
                    self.expr(*s);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value);
                for c in cases.iter() {
                    self.enter();
                    self.case_pattern(c.pattern);
                    if let Some(g) = c.guard {
                        self.expr(g);
                    }
                    self.expr(c.body);
                    self.exit();
                }
            }
            ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::Symbol(_)
            | ExprKind::This
            | ExprKind::Super
            | ExprKind::CascadeTarget
            | ExprKind::Rethrow
            | ExprKind::DotShorthand { .. } => {}
        }
    }

    fn string(&mut self, s: &ast::StringLit) {
        for p in s.parts.iter() {
            if let StringPart::Interpolation(x) = p {
                self.expr(*x);
            }
        }
    }

    fn arguments(&mut self, a: &ast::Arguments) {
        for x in a.args.iter() {
            self.expr(x.value);
        }
    }

    fn element(&mut self, el: &CollectionElement) {
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => self.expr(*x),
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(*key);
                self.expr(*value);
            }
            CollectionElement::Spread { value, .. } => self.expr(*value),
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition);
                self.enter();
                if let Some(p) = case_pattern {
                    self.case_pattern(*p);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.element(then);
                self.exit();
                if let Some(e) = else_ {
                    self.element(e);
                }
            }
            CollectionElement::For { init, condition, updates, body, .. } => {
                self.enter();
                self.for_init(init.as_ref());
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for u in updates.iter() {
                    self.expr(*u);
                }
                self.element(body);
                self.exit();
            }
            CollectionElement::ForIn { target, iterable, body, .. } => {
                self.expr(*iterable);
                self.enter();
                self.for_in_target(target);
                self.element(body);
                self.exit();
            }
        }
    }

    // -- Patterns ------------------------------------------------------------

    /// A `case`, `if-case` or switch expression case pattern: refutable.
    fn case_pattern(&mut self, p: PatternId) {
        let before = self.refutable;
        self.refutable = true;
        self.declared_pattern(p, None);
        self.refutable = before;
    }

    /// A pattern that declares variables (`var (a, b) = e`, `case`, `if-case`):
    /// first the variables, then the constant expressions — `== b && var b`
    /// already refers to the pattern's own `b` (`referenced_before_declaration`).
    fn declared_pattern(&mut self, p: PatternId, group: Option<usize>) {
        let before = self.pattern_start.replace(self.locals.len());
        self.declare_from_pattern(p, group);
        self.pattern_start = before;
        self.read_from_pattern(p);
    }

    /// Declares a pattern variable; in a shared region (both sides of `||`,
    /// the `case`s that share a body), the same name already declared there is
    /// the **same** variable (`JoinPatternVariableElement`).
    fn declare_pattern_name(&mut self, n: ast::Name, group: Option<usize>) {
        if let Some(region) = self.shared_region {
            if let Some(i) = self.find(n.sym).filter(|&i| i >= region) {
                let root = self.root(i);
                if self.wildcard && self.interner.resolve(n.sym) == "_" {
                    return;
                }
                self.locals.push(Local { name: n.sym, span: n.span, kind: LocalKind::Variable, read: false, group, alias: Some(root), read_by_own_guard: false });
                return;
            }
        }
        // `var a && var a` (`duplicate_variable_pattern`): the second does not
        // enter the scope; the uses of `a` are the first's.
        if let Some(start) = self.pattern_start {
            if self.find(n.sym).is_some_and(|i| i >= start) {
                return;
            }
        }
        self.declare(n, LocalKind::Variable, group);
    }

    fn declare_from_pattern(&mut self, p: PatternId, group: Option<usize>) {
        let ast = self.ast;
        match &ast.pattern(p).kind {
            PatternKind::Variable { name, final_, var_, ty } => {
                // In a refutable pattern, the loose identifier is a constant.
                if !(self.refutable && !*final_ && !*var_ && ty.is_none()) {
                    self.declare_pattern_name(*name, group);
                }
            }
            PatternKind::Or(a, b) => {
                let before = self.shared_region;
                self.shared_region = Some(before.unwrap_or(self.locals.len()));
                self.declare_from_pattern(*a, group);
                self.declare_from_pattern(*b, group);
                self.shared_region = before;
            }
            PatternKind::And(a, b) => {
                self.declare_from_pattern(*a, group);
                self.declare_from_pattern(*b, group);
            }
            PatternKind::NullCheck(a) | PatternKind::NullAssert(a) | PatternKind::Parenthesized(a) => {
                self.declare_from_pattern(*a, group)
            }
            PatternKind::Cast { pattern, .. } => self.declare_from_pattern(*pattern, group),
            PatternKind::List { elements, .. } => {
                for e in elements.iter() {
                    if let ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) = e {
                        self.declare_from_pattern(*x, group);
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for e in entries.iter() {
                    self.declare_from_pattern(e.value, group);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.declare_from_pattern(f.pattern, group);
                }
            }
            PatternKind::Wildcard { .. } | PatternKind::Constant(_) | PatternKind::Relational { .. } => {}
        }
    }

    fn read_from_pattern(&mut self, p: PatternId) {
        let ast = self.ast;
        match &ast.pattern(p).kind {
            PatternKind::Variable { name, final_, var_, ty } => {
                if self.refutable && !*final_ && !*var_ && ty.is_none() {
                    self.refer(*name, true)
                }
            }
            PatternKind::Constant(x) | PatternKind::Relational { value: x, .. } => self.expr(*x),
            PatternKind::Or(a, b) | PatternKind::And(a, b) => {
                self.read_from_pattern(*a);
                self.read_from_pattern(*b);
            }
            PatternKind::NullCheck(a) | PatternKind::NullAssert(a) | PatternKind::Parenthesized(a) => {
                self.read_from_pattern(*a)
            }
            PatternKind::Cast { pattern, .. } => self.read_from_pattern(*pattern),
            PatternKind::List { elements, .. } => {
                for e in elements.iter() {
                    if let ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) = e {
                        self.read_from_pattern(*x);
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for e in entries.iter() {
                    self.expr(e.key);
                    self.read_from_pattern(e.value);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.read_from_pattern(f.pattern);
                }
            }
            PatternKind::Wildcard { .. } => {}
        }
    }
    /// An assignment pattern: the variables are written, not declared; with
    /// `var`, `final` or a type, the `DeclaredVariablePattern` declares the
    /// local in the block's scope (`ResolutionVisitor.visitDeclaredVariablePattern`).
    fn assignment_pattern(&mut self, p: PatternId) {
        let ast = self.ast;
        match &ast.pattern(p).kind {
            PatternKind::Variable { name, var_, final_, ty } if *var_ || *final_ || ty.is_some() => self.declare_pattern_name(*name, None),
            PatternKind::Variable { name, .. } => self.refer(*name, false),
            PatternKind::Constant(x) | PatternKind::Relational { value: x, .. } => self.expr(*x),
            PatternKind::Or(a, b) | PatternKind::And(a, b) => {
                self.assignment_pattern(*a);
                self.assignment_pattern(*b);
            }
            PatternKind::NullCheck(a) | PatternKind::NullAssert(a) | PatternKind::Parenthesized(a) => {
                self.assignment_pattern(*a)
            }
            PatternKind::Cast { pattern, .. } => self.assignment_pattern(*pattern),
            PatternKind::List { elements, .. } => {
                for e in elements.iter() {
                    if let ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) = e {
                        self.assignment_pattern(*x);
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for e in entries.iter() {
                    self.expr(e.key);
                    self.assignment_pattern(e.value);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.assignment_pattern(f.pattern);
                }
            }
            PatternKind::Wildcard { .. } => {}
        }
    }

    // -- Comandos ------------------------------------------------------------

    fn for_init(&mut self, init: Option<&ForInit>) {
        match init {
            Some(ForInit::Variables(vl)) => self.variables(vl),
            Some(ForInit::Expression(x)) => self.expr(*x),
            // Like the pattern declaration: the group is only reported if
            // none of its variables is read.
            Some(ForInit::Pattern { pattern, value, .. }) => {
                self.expr(*value);
                let g = self.new_group();
                self.declared_pattern(*pattern, Some(g));
            }
            None => {}
        }
    }

    fn for_in_target(&mut self, t: &ForInTarget) {
        match t {
            ForInTarget::Declared { name, .. } => self.declare(*name, LocalKind::Variable, None),
            ForInTarget::Pattern { pattern, .. } => {
                let g = self.new_group();
                self.declared_pattern(*pattern, Some(g));
            }
            ForInTarget::Expression(x) => match &self.ast.expr(*x).kind {
                ExprKind::Identifier(n) => self.refer(*n, false),
                _ => self.expr(*x),
            },
        }
    }

    fn new_group(&mut self) -> usize {
        self.groups += 1;
        self.groups
    }

    fn variables(&mut self, vl: &ast::VariableList) {
        for v in vl.variables.iter() {
            if let Some(i) = v.initializer {
                self.expr(i);
            }
            self.declare(v.name, LocalKind::Variable, None);
        }
    }

    /// `@name` in the text `[from, until)`: a read of local `name`, if any.
    fn annotations_in_text(&mut self, from: usize, until: usize) {
        let Some(fragment) = self.source.get(from..until) else { return };
        let b = fragment.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'@' {
                let start = i + 1;
                let mut j = start;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'$') {
                    j += 1;
                }
                if j > start {
                    if let Some(sym) = self.interner.lookup(&fragment[start..j]) {
                        let span = Span { start: from + start, end: from + j };
                        self.refer(ast::Name { sym, span }, true);
                    }
                }
                i = j.max(i + 1);
            } else {
                i += 1;
            }
        }
    }

    fn block(&mut self, list: &[StmtId], start: usize) {
        self.enter();
        self.stmts(list, start);
        self.exit();
    }

    /// A statement list in the current scope. The scope of a local declaration
    /// is the whole block (a reference before it already is to it, and that is
    /// the `referenced_before_declaration` error): declarations are hoisted.
    fn stmts(&mut self, list: &[StmtId], list_start: usize) {
        let ast = self.ast;
        for &s in list {
            match &ast.stmt(s).kind {
                StmtKind::Variables(vl) => {
                    for v in vl.variables.iter() {
                        self.declare(v.name, LocalKind::Variable, None);
                    }
                }
                StmtKind::Function(f) => {
                    if let Some(n) = ast.function(*f).name {
                        self.declare(n, LocalKind::Function, None);
                    }
                }
                StmtKind::PatternVariables { pattern, .. } => {
                    let g = self.new_group();
                    self.declared_pattern(*pattern, Some(g));
                }
                _ => {}
            }
        }
        let mut previous_end = Some(list_start);
        for &s in list {
            // Annotations of a local declaration (`@a var b;`) read what they
            // name, and the parser does not keep them: looked up in the text
            // between the previous statement and this one.
            // Up to the declared name: the annotation may be inside the statement's range.
            let name = match &ast.stmt(s).kind {
                StmtKind::Variables(vl) => vl.variables.first().map(|v| v.name.span.start),
                StmtKind::Function(f) => ast.function(*f).name.map(|n| n.span.start),
                _ => None,
            };
            if let (Some(from), Some(until)) = (previous_end, name) {
                self.annotations_in_text(from, until);
            }
            previous_end = Some(ast.stmt(s).span.end);
            match &ast.stmt(s).kind {
                StmtKind::Variables(vl) => {
                    for v in vl.variables.iter() {
                        if let Some(i) = v.initializer {
                            self.expr(i);
                        }
                    }
                }
                StmtKind::Function(f) => self.local_function(*f),
                StmtKind::PatternVariables { value, .. } => self.expr(*value),
                _ => self.stmt(s),
            }
        }
    }

    /// The body of a local function, with it as the `_enclosingExec`.
    fn local_function(&mut self, f: ast::FunctionId) {
        let i = self.ast.function(f).name.and_then(|n| self.find(n.sym));
        let saved = std::mem::replace(&mut self.executable, i);
        self.function(f, false);
        self.executable = saved;
    }

    fn stmt(&mut self, s: StmtId) {
        let ast = self.ast;
        match &ast.stmt(s).kind {
            StmtKind::Block(l) => self.block(l, ast.stmt(s).span.start),
            StmtKind::Variables(vl) => self.variables(vl),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.expr(*value);
                let g = self.new_group();
                self.declared_pattern(*pattern, Some(g));
            }
            StmtKind::Function(f) => {
                if let Some(n) = ast.function(*f).name {
                    self.declare(n, LocalKind::Function, None);
                }
                self.local_function(*f);
            }
            StmtKind::Expression(x) => self.expr_ctx(*x, true),
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition);
                self.enter();
                if let Some(p) = case_pattern {
                    self.case_pattern(*p);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.stmt(*then);
                self.exit();
                // Each branch is a scope (`_visitStatementInScope`): the
                // `else var x = 0;` does not leak.
                if let Some(e) = else_ {
                    self.enter();
                    self.stmt(*e);
                    self.exit();
                }
            }
            StmtKind::For { init, condition, updates, body, .. } => {
                self.enter();
                self.for_init(init.as_ref());
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for u in updates.iter() {
                    self.expr(*u);
                }
                self.stmt(*body);
                self.exit();
            }
            StmtKind::ForIn { target, iterable, body, .. } => {
                self.expr(*iterable);
                self.enter();
                self.for_in_target(target);
                self.stmt(*body);
                self.exit();
            }
            StmtKind::While { condition, body } => {
                self.expr(*condition);
                self.enter();
                self.stmt(*body);
                self.exit();
            }
            StmtKind::DoWhile { body, condition } => {
                self.enter();
                self.stmt(*body);
                self.exit();
                self.expr(*condition);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(*value);
                // `case A: case B: body` — the cases without a body share the
                // next one's, and the variables with the same name are one.
                let mut i = 0;
                while i < cases.len() {
                    let mut j = i;
                    while j + 1 < cases.len() && cases[j].body.is_empty() {
                        j += 1;
                    }
                    self.enter();
                    let before = self.shared_region;
                    self.shared_region = Some(self.locals.len());
                    for c in &cases[i..=j] {
                        let first = self.locals.len();
                        if let Some(p) = c.pattern {
                            self.case_pattern(p);
                        }
                        if let Some(g) = c.guard {
                            let guard = std::mem::replace(&mut self.guard_vars, (first..self.locals.len()).collect());
                            self.expr(g);
                            self.guard_vars = guard;
                        }
                    }
                    self.shared_region = before;
                    self.stmts(&cases[j].body, cases[j].span.start);
                    self.exit();
                    i = j + 1;
                }
            }
            StmtKind::Return(x) => {
                if let Some(x) = x {
                    self.expr(*x);
                }
            }
            StmtKind::Yield { value, .. } => self.expr(*value),
            StmtKind::Try { body, catches, finally_ } => {
                self.stmt(*body);
                for c in catches.iter() {
                    self.enter();
                    // `visitCatchClause`: the exception counts as used when
                    // there is a stack or there is no `on`.
                    if let Some(n) = c.exception {
                        let used = c.stack_trace.is_some() || c.on_type.is_none();
                        self.declare(n, if used { LocalKind::Hidden } else { LocalKind::CatchException }, None);
                    }
                    if let Some(n) = c.stack_trace {
                        self.declare(n, LocalKind::CatchStack, None);
                    }
                    self.stmt(c.body);
                    self.exit();
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
            StmtKind::Break(_) | StmtKind::Continue(_) | StmtKind::Empty => {}
        }
    }

    // -- Functions -----------------------------------------------------------

    fn params(&mut self, ps: &[Parameter]) {
        for p in ps {
            // `g(@a x)`: the annotation reads local `a` (before the parameters
            // enter the scope).
            for a in p.metadata.iter() {
                if let Some(n) = a.name.first() {
                    self.refer(*n, true);
                }
                if let Some(args) = &a.arguments {
                    self.arguments(args);
                }
            }
            if let Some(d) = p.default_value {
                self.expr(d);
            }
        }
        for p in ps {
            if let Some(n) = p.name {
                self.declare(n, LocalKind::Hidden, None);
            }
        }
    }

    fn body(&mut self, b: &FunctionBody) {
        match b {
            FunctionBody::Block(s) => self.stmt(*s),
            FunctionBody::Expression(x) => self.expr(*x),
            FunctionBody::Empty | FunctionBody::Native(_) => {}
        }
    }

    /// A function (top-level, member, local or expression): the parameters in
    /// a scope of their own, then the body.
    fn function(&mut self, f: FunctionId, _root: bool) {
        let ast = self.ast;
        let func = ast.function(f);
        self.enter();
        if let Some(ps) = &func.parameters {
            self.params(ps);
        }
        self.body(&func.body);
        self.exit();
    }

    fn initializers(&mut self, inits: &[Initializer]) {
        for i in inits {
            match i {
                Initializer::Field { value, .. } => self.expr(*value),
                Initializer::Super { arguments, .. } | Initializer::Redirect { arguments, .. } => {
                    self.arguments(arguments)
                }
                Initializer::Assert { condition, message, .. } => {
                    self.expr(*condition);
                    if let Some(m) = message {
                        self.expr(*m);
                    }
                }
            }
        }
    }

    fn member(&mut self, m: ast::MemberId) {
        let ast = self.ast;
        match &ast.member(m).kind {
            MemberKind::Field(vl) => {
                for v in vl.variables.iter() {
                    if let Some(i) = v.initializer {
                        self.expr(i);
                    }
                }
            }
            MemberKind::Method(f) => self.function(*f, true),
            MemberKind::Constructor(k) => {
                self.enter();
                self.params(&k.parameters);
                self.initializers(&k.initializers);
                self.body(&k.body);
                self.exit();
            }
        }
    }
}

/// The ranges of the unit's executable declarations: a top-level function or
/// variable, and each member of a class, mixin, extension, extension type and
/// enum.
fn executables(u: Unit<'_>) -> Vec<Span> {
    let ast = u.ast;
    let mut out = Vec::new();
    for &d in &u.unit.declarations {
        let decl = ast.decl(d);
        let members: &[ast::MemberId] = match &decl.kind {
            DeclKind::Class(x) => &x.members,
            DeclKind::Mixin(x) => &x.members,
            DeclKind::Extension(x) => &x.members,
            DeclKind::ExtensionType(x) => &x.members,
            DeclKind::Enum(x) => &x.members,
            DeclKind::Function(_) | DeclKind::Variables(_) => {
                out.push(decl.span);
                continue;
            }
            DeclKind::Typedef(_) => continue,
        };
        out.extend(members.iter().map(|m| ast.member(*m).span));
    }
    out
}

/// The unused locals of a unit.
///
/// `syntax_errors`: the parser's diagnostics in the unit. In an executable
/// declaration with a syntax error, fasta's recovery keeps fragments that ours
/// drops (and vice versa); a read the analyzer sees and we do not would become
/// a false positive. A local declared before a syntax error of the same
/// executable declaration is not reported — on the safe side, like the imports.
/// Like [`unused`], with the T5 rule instead of the syntax-error gate: a
/// local is only left out when its name is cited in a fragment the parser
/// skipped inside the same executable declaration (a use fasta may have
/// kept).
pub fn unused_with_skipped(u: Unit<'_>, interner: &Interner, wildcard: bool, skipped: crate::Skipped<'_>) -> Vec<Diagnostic> {
    let mut out = unused_unfiltered(u, interner, wildcard);
    if !skipped.is_empty() {
        let execs = executables(u);
        let blocks = block_scopes(u.ast);
        out.retain(|d| {
            let name = u.source.get(d.span.start..d.span.end).unwrap_or("");
            // Only a fragment inside the local's scope and after the declared
            // name may hold a use of it: the smallest block, `switch` case,
            // loop, `if` or `try` that contains the declaration.
            !execs.iter().any(|e| {
                if !(d.span.start >= e.start && d.span.end <= e.end) {
                    return false;
                }
                let end = blocks
                    .iter()
                    .filter(|b| b.start <= d.span.start && d.span.end <= b.end && e.start <= b.start && b.end <= e.end)
                    .min_by_key(|b| b.end - b.start)
                    .map_or(e.end, |b| b.end);
                skipped.mentions_within(name, dartforge_diagnostics::Span { start: d.span.end, end })
            })
        });
    }
    out
}

/// The ranges that delimit local scopes inside a body: blocks, `switch` cases
/// (pattern, guard and body), loops, `if` and `try`.
fn block_scopes(ast: &Ast) -> Vec<Span> {
    let mut out = Vec::new();
    for s in ast.stmts.iter() {
        match &s.kind {
            StmtKind::Block(_)
            | StmtKind::For { .. }
            | StmtKind::ForIn { .. }
            | StmtKind::If { .. }
            | StmtKind::Try { .. }
            | StmtKind::While { .. }
            | StmtKind::DoWhile { .. } => out.push(s.span),
            StmtKind::Switch { cases, .. } => out.extend(cases.iter().map(|c| c.span)),
            _ => {}
        }
    }
    out
}

/// The unused locals of the unit, without the skipped-fragments rule (a unit
/// the parser read entirely).
pub fn unused(u: Unit<'_>, interner: &Interner, wildcard: bool) -> Vec<Diagnostic> {
    unused_unfiltered(u, interner, wildcard)
}

fn unused_unfiltered(u: Unit<'_>, interner: &Interner, wildcard: bool) -> Vec<Diagnostic> {
    let ast = u.ast;
    let mut v = Visitor { ast, interner, wildcard, source: u.source, locals: Vec::new(), scopes: vec![Vec::new()], groups: 0, refutable: false, shared_region: None, pattern_start: None, guard_vars: Vec::new(), executable: None };
    for &d in &u.unit.declarations {
        match &ast.decl(d).kind {
            DeclKind::Function(f) => v.function(*f, true),
            DeclKind::Variables(vl) => {
                for var in vl.variables.iter() {
                    if let Some(i) = var.initializer {
                        v.expr(i);
                    }
                }
            }
            DeclKind::Class(x) => x.members.iter().for_each(|m| v.member(*m)),
            DeclKind::Mixin(x) => x.members.iter().for_each(|m| v.member(*m)),
            DeclKind::Extension(x) => x.members.iter().for_each(|m| v.member(*m)),
            DeclKind::ExtensionType(x) => x.members.iter().for_each(|m| v.member(*m)),
            DeclKind::Enum(x) => {
                for k in &x.constants {
                    if let Some(a) = &k.arguments {
                        v.arguments(a);
                    }
                }
                x.members.iter().for_each(|m| v.member(*m));
            }
            DeclKind::Typedef(_) => {}
        }
    }
    // A variable is read when its root (the declaration it shares) was read.
    let read_set: Vec<bool> = (0..v.locals.len()).map(|i| v.locals[v.root(i)].read || v.locals[i].read_by_own_guard).collect();
    let mut read_groups = std::collections::HashSet::new();
    for (i, l) in v.locals.iter().enumerate() {
        if let (Some(g), true) = (l.group, read_set[i]) {
            read_groups.insert(g);
        }
    }
    let mut out = Vec::new();
    for (i, l) in v.locals.iter().enumerate() {
        let name = interner.resolve(l.name);
        // `_isNamedWildcard`: only `_`s (with the feature, only `_`, which does
        // not even declare). The local function is only left out with the
        // feature (`_visitFunctionElement`, `unused_local_elements_verifier.dart:1033-1040`).
        let only_underscores = match l.kind {
            LocalKind::Function => wildcard && name == "_",
            _ => name.bytes().all(|b| b == b'_') && !(wildcard && name.len() > 1),
        };
        if read_set[i] || only_underscores || l.group.is_some_and(|g| read_groups.contains(&g)) {
            continue;
        }
        match l.kind {
            LocalKind::Variable => out.push(Diagnostic::with_code(w::UNUSED_LOCAL_VARIABLE, l.span, [name])),
            LocalKind::Function => out.push(Diagnostic::with_code(w::UNUSED_ELEMENT, l.span, [name])),
            LocalKind::CatchException => out.push(Diagnostic::with_code(w::UNUSED_CATCH_CLAUSE, l.span, [name])),
            LocalKind::CatchStack => out.push(Diagnostic::with_code(w::UNUSED_CATCH_STACK, l.span, [name])),
            LocalKind::Hidden => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> Vec<(String, String)> {
        let mut interner = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut interner);
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        let u = Unit { ast: &p.ast, unit: &p.unit, source };
        let mut v: Vec<_> = unused(u, &interner, false)
            .into_iter()
            .map(|d| (d.code.unwrap().info().name.to_string(), source[d.span.start..d.span.end].to_string()))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn reads_and_writes() {
        let f = "void f(int p) {\n  var a = 1;\n  var b = 2;\n  b = 3;\n  var c = 0;\n  c++;\n  var d = 0;\n  print(d++);\n  var e = 0;\n  e += 1;\n  int? g;\n  g ??= 1;\n  var (h, i) = (1, 2);\n  print(h);\n  var (j, k) = (1, 2);\n  for (var x in [1]) {}\n  void l() {}\n  var _ = 1;\n  var p2 = () => a;\n  print(p2);\n}\n";
        let r = run(f);
        let names: Vec<&str> = r.iter().map(|x| x.1.as_str()).collect();
        assert_eq!(names, vec!["l", "b", "c", "e", "j", "k", "x"], "{r:?}");
        assert_eq!(r[0].0, "unused_element");
    }

    #[test]
    fn shared_patterns_and_for_update() {
        let f = "void f(Object? x) {\n  if (x case [var a || var a]) { print(a); }\n  switch (x) {\n    case (0, int y):\n    case (1, final int y):\n      print(y);\n  }\n  if (x case == b && var b) {}\n  for (int i = 0; i < 3; i++) {}\n}\n";
        let r = run(f);
        assert_eq!(r, vec![], "{r:?}");
    }

    #[test]
    fn refutable_case_reference_anchor_and_hoisting() {
        // `case a` reads the constant `a`; `@a` annotates and reads; `v;` before
        // `var v` already is the inner declaration.
        let f = "void f(x) {\n  var a = 0;\n  if (x case a) {}\n  const c = 0;\n  @c\n  var b = 1;\n  print(b);\n  var v = 1;\n  {\n    v;\n    var v = 2;\n  }\n}\n";
        let r = run(f);
        assert_eq!(r, vec![("unused_local_variable".to_string(), "v".to_string())], "{r:?}");
    }

    #[test]
    fn shadow_and_closure() {
        let f = "void f() {\n  var a = 1;\n  void g(int a) { print(a); }\n  g(0);\n}\n";
        let r = run(f);
        assert_eq!(r, vec![("unused_local_variable".to_string(), "a".to_string())]);
    }

    #[test]
    fn redeclaration_in_same_scope() {
        // The `i` read in the condition is the first; the second is reported.
        let f = "void f() {\n  for (int i = 0, i = 0; i < 5;) {}\n}\n";
        let mut interner = Interner::new();
        let p = dartforge_frontend::parser::parse(f, &mut interner);
        let u = Unit { ast: &p.ast, unit: &p.unit, source: f };
        let r: Vec<usize> = unused(u, &interner, false).iter().map(|d| d.span.start).collect();
        assert_eq!(r, vec![f.find(", i").unwrap() + 2]);
    }

    #[test]
    fn skipped_fragment_citing_the_name_does_not_report() {
        let f = "void f() {\n  var a = 1;\n  // a b c\n}\nvoid g() {\n  var b = 1;\n}\n";
        let mut interner = Interner::new();
        let p = dartforge_frontend::parser::parse(f, &mut interner);
        let u = Unit { ast: &p.ast, unit: &p.unit, source: f };
        // A skipped fragment in the body of `f` that cites `a`: `a` is not
        // reported (fasta would have read the read); `b`, from `g`, is.
        let fragment = Span { start: f.find("a b c").unwrap(), end: f.find("a b c").unwrap() + 5 };
        let skipped = crate::Skipped { source: f, spans: &[fragment] };
        let r: Vec<usize> = unused_with_skipped(u, &interner, false, skipped).iter().map(|d| d.span.start).collect();
        assert_eq!(r, vec![f.find("b = 1;\n}\n").unwrap()]);
        // The fragment only counts inside the executable declaration it touches.
        let outside = Span { start: f.find("void g").unwrap(), end: f.find("void g").unwrap() + 4 };
        let skipped = crate::Skipped { source: f, spans: &[outside] };
        assert_eq!(unused_with_skipped(u, &interner, false, skipped).len(), 2);
    }

    #[test]
    fn duplicate_pattern_and_parameter_annotation() {
        // `var a && var a`: the use is the first `a`'s (the second does not even
        // enter the scope); `g(@a x)` reads `a`.
        let f = "void f(int x) {\n  if (x case var a && var a) {\n    a;\n  }\n  const b = 0;\n  g(@b y) {}\n  g(0);\n}\n";
        let r = run(f);
        assert_eq!(r, vec![], "{r:?}");
    }

    /// `UnusedLocalElementsVerifier.visitCatchClause`: the exception is only
    /// reported in `on T catch (e)` without a stack; the stack, whenever not read.
    #[test]
    fn catch_variables() {
        let f = "void f() {\n  try {} on Exception catch (e) {}\n  try {} catch (e) {}\n  try {} on Exception catch (e, s) {}\n  try {} catch (e, t) { print(t); }\n  try {} on Error catch (_) {}\n}\n";
        let r = run(f);
        assert_eq!(
            r,
            vec![("unused_catch_clause".to_string(), "e".to_string()), ("unused_catch_stack".to_string(), "s".to_string())]
        );
    }
}
