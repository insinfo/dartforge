//! Family C checks that depend only on the tree
//! (docs/ANALYZER-ESPECIFICACAO.md §C):
//!
//! * `rethrow_outside_catch` (`error_verifier.dart:5199-5207`);
//! * `return_in_generator` (the parser's `messageGeneratorReturnsValue`);
//! * `label_undefined`, `label_in_outer_scope`, `continue_label_invalid`
//!   (`ResolverVisitor._lookupBreakOrContinueTarget`, `resolver.dart:5314-5366`);
//! * `invalid_modifier_on_setter` (the parser's `messageSetterNotSync`);
//! * `uri_with_interpolation` (`library_analyzer.dart:719`, `:947`, `:995`);
//! * `main_is_not_function` (`error_verifier.dart:4125-4141`);
//! * family E: `break_outside_of_loop`, `continue_outside_of_loop`,
//!   `continue_without_label_in_case` (the `loopState` of fasta's parser);
//! * family B: `break_label_on_switch_member`, `duplicate_variable_pattern`,
//!   `duplicate_pattern_assignment_variable`, `duplicate_pattern_field`,
//!   `rest_element_in_map_pattern`, `empty_map_pattern`,
//!   `expected_one_list_pattern_type_arguments`,
//!   `expected_two_map_pattern_type_arguments`;
//! * family F: `unused_label` (`dead_code_verifier.dart:159-174`);
//! * `yield_in_non_generator` on the statement (`YieldStatementResolver`,
//!   `yield_statement_resolver.dart:184-196`; the parser's is on the `yield`);
//! * family B: `unnecessary_final` (`best_practices_verifier.dart:828-835`) and
//!   `late_final_field_with_const_constructor` (`error_verifier.dart:4068-4091`).
//!
//! Written without compiling or running (2026-10-04).

use crate::Unit;
use dartforge_diagnostics::codes::{compile_time_error as c, parser as pe, warning as w};
use dartforge_diagnostics::{Code, Diagnostic, Span};
use dartforge_frontend::ast::{
    self, AsyncModifier, CollectionElement, DeclKind, DirectiveKind, ExprId, ExprKind,
    FunctionBody, FunctionKind, MemberId, MemberKind, PatternId, PatternKind, StmtId, StmtKind,
};
use dartforge_intern::{Interner, SymbolId};

/// A visible label: the name and whether the target accepts `continue` (a loop
/// or a `switch` member).
#[derive(Clone, Copy)]
struct Label {
    name: SymbolId,
    span: Span,
    accepts_continue: bool,
    /// The label is on a `case`/`default`.
    on_switch_member: bool,
    used: bool,
}

/// The `loopState` of fasta's parser: where `break` and `continue` are valid.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LoopState {
    Outside,
    InSwitch,
    InLoop,
}

/// The context of a top-level pattern.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PatternContext {
    /// `var (a, b) = …`: every identifier declares a variable.
    Declaration,
    /// `case`: only declares what has `var`, `final` or a type.
    Refutable,
    /// `(a, b) = …`: the identifiers are existing variables.
    Assignment,
}

struct Visitor<'a> {
    a: &'a ast::Ast,
    source: &'a str,
    names: &'a Interner,
    out: Vec<Diagnostic>,
    /// The labels in scope; `boundary` marks where the current function
    /// starts (what comes before belongs to an outer method).
    labels: Vec<Label>,
    boundary: usize,
    /// How many `catch`es of the current function enclose the point.
    catch_depth: u32,
    modifier: AsyncModifier,
    loop_state: LoopState,
    /// The primary constructor of the type whose members are being visited.
    primary: Option<MemberId>,
}

/// The syntactic diagnostics of a unit.
pub fn check(u: Unit<'_>, names: &Interner) -> Vec<Diagnostic> {
    let mut v = Visitor {
        a: u.ast,
        source: u.source,
        names,
        out: Vec::new(),
        labels: Vec::new(),
        boundary: 0,
        catch_depth: 0,
        modifier: AsyncModifier::None,
        loop_state: LoopState::Outside,
        primary: None,
    };
    // Directive URIs cannot have interpolation.
    for d in u.unit.directives.iter() {
        let uri = match &d.kind {
            DirectiveKind::Import { uri, .. }
            | DirectiveKind::Export { uri, .. }
            | DirectiveKind::Part { uri } => Some(uri),
            _ => None,
        };
        if let Some(uri) = uri
            && uri
                .parts
                .iter()
                .any(|p| matches!(p, ast::StringPart::Interpolation(_)))
        {
            v.report(c::URI_WITH_INTERPOLATION, uri.span, &[]);
        }
    }
    let main_sym = names.lookup("main");
    for &d in u.unit.declarations.iter() {
        v.decl(d, main_sym);
    }
    v.out
}

impl<'a> Visitor<'a> {
    fn report(&mut self, code: Code, span: Span, args: &[&str]) {
        self.out
            .push(Diagnostic::with_code(code, span, args.iter().copied()));
    }

    fn decl(&mut self, d: ast::DeclId, main_sym: Option<SymbolId>) {
        let a = self.a;
        // A top-level `main` that is not a function.
        let not_function =
            |n: ast::Name| -> Option<Span> { (Some(n.sym) == main_sym).then_some(n.span) };
        match &a.decl(d).kind {
            DeclKind::Variables(l) => {
                for var in l.variables.iter() {
                    if let Some(sp) = not_function(var.name) {
                        self.report(c::MAIN_IS_NOT_FUNCTION, sp, &[]);
                    }
                    if let Some(init) = var.initializer {
                        self.expr(init);
                    }
                }
            }
            DeclKind::Function(f) => {
                let func = a.function(*f);
                if matches!(func.kind, FunctionKind::Getter | FunctionKind::Setter)
                    && let Some(sp) = func.name.and_then(not_function)
                {
                    self.report(c::MAIN_IS_NOT_FUNCTION, sp, &[]);
                }
                self.function(*f);
            }
            DeclKind::Class(k) => {
                if let Some(sp) = not_function(k.name) {
                    self.report(c::MAIN_IS_NOT_FUNCTION, sp, &[]);
                }
                let has_const = k.members.iter().any(|m| matches!(&a.member(*m).kind, MemberKind::Constructor(x) if x.const_ && !x.factory));
                self.primary = k.primary_constructor;
                self.members(&k.members, has_const);
                self.primary = None;
            }
            DeclKind::Mixin(k) => {
                if let Some(sp) = not_function(k.name) {
                    self.report(c::MAIN_IS_NOT_FUNCTION, sp, &[]);
                }
                self.members(&k.members, false);
            }
            DeclKind::Enum(k) => {
                if let Some(sp) = not_function(k.name) {
                    self.report(c::MAIN_IS_NOT_FUNCTION, sp, &[]);
                }
                for cst in k.constants.iter() {
                    if let Some(args) = &cst.arguments {
                        for x in args.args.iter() {
                            self.expr(x.value);
                        }
                    }
                }
                // An enum's constructors are always `const`.
                self.primary = k.primary_constructor;
                self.members(&k.members, true);
                self.primary = None;
            }
            DeclKind::Extension(k) => self.members(&k.members, false),
            DeclKind::ExtensionType(k) => self.members(&k.members, false),
            DeclKind::Typedef(t) => {
                if let Some(sp) = not_function(t.name) {
                    self.report(c::MAIN_IS_NOT_FUNCTION, sp, &[]);
                }
            }
        }
    }

    /// `has_const_generative`: the type has some `const` generative constructor.
    fn members(&mut self, members: &'a [MemberId], has_const_generative: bool) {
        let a = self.a;
        for &mid in members {
            match &a.member(mid).kind {
                MemberKind::Field(l) => {
                    // A `late final` instance field in a type with a `const`
                    // generative constructor: on the word `late`.
                    if has_const_generative && l.late && l.final_ && !l.static_ {
                        let sp = a.member(mid).span;
                        if let Some(i) = self
                            .source
                            .get(sp.start..sp.end.min(self.source.len()))
                            .and_then(|t| first_word(t, "late"))
                        {
                            self.report(
                                c::LATE_FINAL_FIELD_WITH_CONST_CONSTRUCTOR,
                                Span {
                                    start: sp.start + i,
                                    end: sp.start + i + 4,
                                },
                                &[],
                            );
                        }
                    }
                    for var in l.variables.iter() {
                        if let Some(init) = var.initializer {
                            self.expr(init);
                        }
                    }
                }
                MemberKind::Method(f) => self.function(*f),
                MemberKind::Constructor(k) => {
                    // In the primary constructor, `final`/`var` with
                    // `this.`/`super.` is the parser's error
                    // (`INITIALIZING_DECLARING_PARAMETER`), not this warning.
                    let primary = self.primary == Some(mid) || k.primary_body_part;
                    if !primary {
                        self.params(&k.parameters);
                    } else {
                        for p in k.parameters.iter() {
                            if let Some(d) = p.default_value {
                                self.expr(d);
                            }
                        }
                    }
                    for init in k.initializers.iter() {
                        match init {
                            ast::Initializer::Field { value, .. } => self.expr(*value),
                            ast::Initializer::Assert {
                                condition, message, ..
                            } => {
                                self.expr(*condition);
                                if let Some(m) = message {
                                    self.expr(*m);
                                }
                            }
                            ast::Initializer::Super { arguments, .. }
                            | ast::Initializer::Redirect { arguments, .. } => {
                                for x in arguments.args.iter() {
                                    self.expr(x.value);
                                }
                            }
                        }
                    }
                    self.in_function(AsyncModifier::None, |v| v.body(&k.body, None));
                }
            }
        }
    }

    fn params(&mut self, ps: &'a [ast::Parameter]) {
        for p in ps {
            // `final this.x` / `final super.x`: the parameter is already final.
            // In the primary constructor `final` makes it declaring, and the
            // error is `INITIALIZING_DECLARING_PARAMETER` (without this warning).
            if p.final_ && (p.this_ || p.super_) && !p.declaring {
                let end = p.name.map_or(p.span.end, |n| n.span.start);
                if let Some(i) = self
                    .source
                    .get(p.span.start..end)
                    .and_then(|t| first_word(t, "final"))
                {
                    self.report(
                        w::UNNECESSARY_FINAL,
                        Span {
                            start: p.span.start + i,
                            end: p.span.start + i + 5,
                        },
                        &[],
                    );
                }
            }
            if let Some(fp) = &p.function_parameters {
                self.params(fp);
            }
            if let Some(d) = p.default_value {
                self.expr(d);
            }
        }
    }

    /// A function body: labels and `catch`es from outside are not valid inside.
    fn in_function(&mut self, modifier: AsyncModifier, f: impl FnOnce(&mut Self)) {
        let boundary = std::mem::replace(&mut self.boundary, self.labels.len());
        let level = std::mem::replace(&mut self.catch_depth, 0);
        let before = std::mem::replace(&mut self.modifier, modifier);
        let loop_state = std::mem::replace(&mut self.loop_state, LoopState::Outside);
        f(self);
        self.loop_state = loop_state;
        self.modifier = before;
        self.catch_depth = level;
        // Each labeled statement already closed its own; what is left goes here.
        self.labels.truncate(self.boundary);
        self.boundary = boundary;
    }

    fn function(&mut self, f: ast::FunctionId) {
        let a = self.a;
        let func = a.function(f);
        if let Some(ps) = &func.parameters {
            self.params(ps);
        }
        // A setter with `async`, `async*` or `sync*`: on the first token of
        // the modifier (the `*` is not included).
        if func.kind == FunctionKind::Setter && func.modifier != AsyncModifier::None {
            let word = if func.modifier == AsyncModifier::SyncStar {
                "sync"
            } else {
                "async"
            };
            let end = match &func.body {
                FunctionBody::Block(s) => a.stmt(*s).span.start,
                FunctionBody::Expression(e) => a.expr(*e).span.start,
                _ => func.span.end,
            };
            let start = func.name.map_or(func.span.start, |n| n.span.end);
            if let Some(i) = self.source.get(start..end).and_then(|t| last_word(t, word)) {
                let start = start + i;
                self.report(
                    c::INVALID_MODIFIER_ON_SETTER,
                    Span {
                        start,
                        end: start + word.len(),
                    },
                    &[],
                );
            }
        }
        let after_params = func.name.map_or(func.span.start, |n| n.span.end);
        self.in_function(func.modifier, |v| v.body(&func.body, Some(after_params)));
    }

    /// `from`: where to look for the `=>` of an expression body.
    fn body(&mut self, b: &FunctionBody, from: Option<usize>) {
        match b {
            FunctionBody::Block(s) => self.stmt(*s),
            FunctionBody::Expression(e) => {
                // `f() sync* => 0;`: a generator returns no value; on the arrow.
                if matches!(
                    self.modifier,
                    AsyncModifier::SyncStar | AsyncModifier::AsyncStar
                ) {
                    let end = self.a.expr(*e).span.start;
                    let start = from.unwrap_or(0).min(end);
                    if let Some(i) = self.source.get(start..end).and_then(|t| t.rfind("=>")) {
                        let start = start + i;
                        self.report(
                            c::RETURN_IN_GENERATOR,
                            Span {
                                start,
                                end: start + 2,
                            },
                            &[],
                        );
                    }
                }
                self.expr(*e);
            }
            _ => {}
        }
    }

    // -- Labels ----------------------------------------------------------------------

    /// `break L` / `continue L`: the label in the current function, otherwise
    /// in an outer method, otherwise undefined.
    fn jump(&mut self, label: ast::Name, is_continue: bool, stmt: Span) {
        let text = self.names.resolve(label.sym).to_string();
        // The use counts for the innermost label with the name, even one of an
        // outer method (`_LabelTracker.recordUsage`).
        if let Some(r) = self.labels.iter_mut().rev().find(|r| r.name == label.sym) {
            r.used = true;
        }
        let local = self.labels[self.boundary..]
            .iter()
            .rev()
            .find(|r| r.name == label.sym)
            .copied();
        match local {
            Some(r) => {
                if is_continue && !r.accepts_continue {
                    self.report(c::CONTINUE_LABEL_INVALID, stmt, &[]);
                }
                if !is_continue && r.on_switch_member {
                    self.report(c::BREAK_LABEL_ON_SWITCH_MEMBER, label.span, &[]);
                }
            }
            None => {
                if self.labels[..self.boundary]
                    .iter()
                    .any(|r| r.name == label.sym)
                {
                    self.report(c::LABEL_IN_OUTER_SCOPE, label.span, &[&text]);
                } else {
                    self.report(c::LABEL_UNDEFINED, label.span, &[&text]);
                }
            }
        }
    }

    /// Removes the labels from `from` on from scope, reporting the unused ones.
    fn close_labels(&mut self, from: usize) {
        let closed: Vec<Label> = self.labels.drain(from..).collect();
        for r in closed {
            if !r.used {
                let text = self.names.resolve(r.name).to_string();
                // On the whole `Label` node: the name and the colon.
                let rest = self.source.get(r.span.end..).unwrap_or("");
                let end = rest
                    .find(':')
                    .filter(|&i| rest[..i].trim().is_empty())
                    .map_or(r.span.end, |i| r.span.end + i + 1);
                self.report(
                    w::UNUSED_LABEL,
                    Span {
                        start: r.span.start,
                        end,
                    },
                    &[&text],
                );
            }
        }
    }

    /// A loop body: `break` and `continue` are valid.
    fn loop_body(&mut self, s: StmtId) {
        let before = std::mem::replace(&mut self.loop_state, LoopState::InLoop);
        self.stmt(s);
        self.loop_state = before;
    }

    // -- Comandos ------------------------------------------------------------------

    fn local_list(&mut self, l: &'a ast::VariableList) {
        for var in l.variables.iter() {
            if let Some(init) = var.initializer {
                self.expr(init);
            }
        }
    }

    fn for_init(&mut self, init: &'a Option<ast::ForInit>) {
        match init {
            Some(ast::ForInit::Variables(l)) => self.local_list(l),
            Some(ast::ForInit::Expression(e)) => self.expr(*e),
            Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                self.top_level_pattern(*pattern, PatternContext::Declaration);
                self.expr(*value);
            }
            None => {}
        }
    }

    fn for_in_target(&mut self, target: &'a ast::ForInTarget) {
        match target {
            ast::ForInTarget::Declared { .. } => {}
            ast::ForInTarget::Pattern { pattern, .. } => {
                self.top_level_pattern(*pattern, PatternContext::Declaration)
            }
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
                self.top_level_pattern(*pattern, PatternContext::Declaration);
                self.expr(*value);
            }
            StmtKind::Function(f) => self.function(*f),
            StmtKind::Expression(e) => self.expr(*e),
            StmtKind::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.top_level_pattern(*p, PatternContext::Refutable);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.stmt(*then);
                if let Some(e) = else_ {
                    self.stmt(*e);
                }
            }
            StmtKind::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                self.for_init(init);
                if let Some(x) = condition {
                    self.expr(*x);
                }
                for x in updates.iter() {
                    self.expr(*x);
                }
                self.loop_body(*body);
            }
            StmtKind::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                self.for_in_target(target);
                self.expr(*iterable);
                self.loop_body(*body);
            }
            StmtKind::While { condition, body } => {
                self.expr(*condition);
                self.loop_body(*body);
            }
            StmtKind::DoWhile { body, condition } => {
                self.loop_body(*body);
                self.expr(*condition);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(*value);
                // The members' labels are valid in the whole `switch`.
                let before = self.labels.len();
                for case in cases.iter() {
                    for l in case.labels.iter() {
                        self.labels.push(Label {
                            name: l.sym,
                            span: l.span,
                            accepts_continue: true,
                            on_switch_member: true,
                            used: false,
                        });
                    }
                }
                // Inside a loop the `switch` does not change the state: an
                // unlabeled `continue` belongs to the loop.
                let loop_state = self.loop_state;
                if loop_state == LoopState::Outside {
                    self.loop_state = LoopState::InSwitch;
                }
                for case in cases.iter() {
                    if let Some(p) = case.pattern {
                        self.top_level_pattern(p, PatternContext::Refutable);
                    }
                    if let Some(g) = case.guard {
                        self.expr(g);
                    }
                    for x in case.body.iter() {
                        self.stmt(*x);
                    }
                }
                self.loop_state = loop_state;
                self.close_labels(before);
            }
            StmtKind::Break(label) => match label {
                Some(r) => self.jump(*r, false, node.span),
                None => {
                    if self.loop_state == LoopState::Outside {
                        self.report(
                            pe::BREAK_OUTSIDE_OF_LOOP,
                            Span {
                                start: node.span.start,
                                end: node.span.start + 5,
                            },
                            &[],
                        );
                    }
                }
            },
            StmtKind::Continue(label) => {
                let word = Span {
                    start: node.span.start,
                    end: node.span.start + 8,
                };
                if self.loop_state == LoopState::Outside {
                    self.report(pe::CONTINUE_OUTSIDE_OF_LOOP, word, &[]);
                } else if label.is_none() && self.loop_state == LoopState::InSwitch {
                    self.report(pe::CONTINUE_WITHOUT_LABEL_IN_CASE, word, &[]);
                }
                if let Some(r) = label {
                    self.jump(*r, true, node.span);
                }
            }
            StmtKind::Return(Some(e)) => {
                // `return value;` in a generator: on the `return` token.
                if matches!(
                    self.modifier,
                    AsyncModifier::SyncStar | AsyncModifier::AsyncStar
                ) {
                    self.report(
                        c::RETURN_IN_GENERATOR,
                        Span {
                            start: node.span.start,
                            end: node.span.start + 6,
                        },
                        &[],
                    );
                }
                self.expr(*e);
            }
            StmtKind::Yield { star, value } => {
                // `YieldStatementResolver._resolve_notGenerator`: on the whole
                // statement, `YIELD_EACH_IN_NON_GENERATOR` with `*`.
                if !matches!(
                    self.modifier,
                    AsyncModifier::SyncStar | AsyncModifier::AsyncStar
                ) {
                    let code = if *star {
                        c::YIELD_EACH_IN_NON_GENERATOR
                    } else {
                        c::YIELD_IN_NON_GENERATOR
                    };
                    self.report(code, node.span, &[]);
                }
                self.expr(*value)
            }
            StmtKind::Try {
                body,
                catches,
                finally_,
            } => {
                self.stmt(*body);
                for k in catches.iter() {
                    self.catch_depth += 1;
                    self.stmt(k.body);
                    self.catch_depth -= 1;
                }
                if let Some(f) = finally_ {
                    self.stmt(*f);
                }
            }
            StmtKind::Labeled { labels, body } => {
                let loop_state = matches!(
                    a.stmt(*body).kind,
                    StmtKind::For { .. }
                        | StmtKind::ForIn { .. }
                        | StmtKind::While { .. }
                        | StmtKind::DoWhile { .. }
                );
                let before = self.labels.len();
                for l in labels.iter() {
                    self.labels.push(Label {
                        name: l.sym,
                        span: l.span,
                        accepts_continue: loop_state,
                        on_switch_member: false,
                        used: false,
                    });
                }
                self.stmt(*body);
                self.close_labels(before);
            }
            StmtKind::Assert { condition, message } => {
                self.expr(*condition);
                if let Some(m) = message {
                    self.expr(*m);
                }
            }
            _ => {}
        }
    }

    // -- Patterns and expressions ----------------------------------------------------

    /// The variables the pattern declares (or assigns), with the name of each
    /// occurrence; both sides of a `||` declare the same ones and count once.
    /// A repetition outside that is reported on the second occurrence.
    fn pattern_variables(
        &mut self,
        p: PatternId,
        ctx: PatternContext,
        seen: &mut Vec<(SymbolId, Span)>,
    ) {
        let a = self.a;
        match &a.pattern(p).kind {
            PatternKind::Variable {
                final_,
                var_,
                ty,
                name,
            } => {
                let declares = match ctx {
                    PatternContext::Declaration | PatternContext::Assignment => true,
                    PatternContext::Refutable => *final_ || *var_ || ty.is_some(),
                };
                if !declares {
                    return;
                }
                if let Some(&(_, first)) = seen.iter().find(|(s, _)| *s == name.sym) {
                    let text = self.names.resolve(name.sym).to_string();
                    // `DiagnosticFactory`: the assignment points at the first
                    // assigned pattern; the declaration, at the first
                    // definition (`duplicateDefinitionForNodes`).
                    let (code, context) = if ctx == PatternContext::Assignment {
                        (
                            c::DUPLICATE_PATTERN_ASSIGNMENT_VARIABLE,
                            "The first assigned variable pattern.",
                        )
                    } else {
                        (
                            c::DUPLICATE_VARIABLE_PATTERN,
                            "The first definition of this name.",
                        )
                    };
                    self.out.push(
                        Diagnostic::with_code(code, name.span, [text.as_str()])
                            .with_context(first, context),
                    );
                } else {
                    seen.push((name.sym, name.span));
                }
            }
            PatternKind::Or(x, y) => {
                let mut left = seen.clone();
                let mut right = seen.clone();
                self.pattern_variables(*x, ctx, &mut left);
                self.pattern_variables(*y, ctx, &mut right);
                for n in left.into_iter().chain(right) {
                    if !seen.iter().any(|(s, _)| *s == n.0) {
                        seen.push(n);
                    }
                }
            }
            PatternKind::And(x, y) => {
                self.pattern_variables(*x, ctx, seen);
                self.pattern_variables(*y, ctx, seen);
            }
            PatternKind::NullCheck(x)
            | PatternKind::NullAssert(x)
            | PatternKind::Parenthesized(x) => self.pattern_variables(*x, ctx, seen),
            PatternKind::Cast { pattern, .. } => self.pattern_variables(*pattern, ctx, seen),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x)
                        | ast::ListPatternElement::Rest(Some(x)) => {
                            self.pattern_variables(*x, ctx, seen)
                        }
                        _ => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for en in entries.iter() {
                    self.pattern_variables(en.value, ctx, seen);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.pattern_variables(f.pattern, ctx, seen);
                }
            }
            _ => {}
        }
    }

    /// A top-level pattern: the repeated variables and, then, the shape of
    /// each subpattern.
    fn top_level_pattern(&mut self, p: PatternId, ctx: PatternContext) {
        let mut seen = Vec::new();
        self.pattern_variables(p, ctx, &mut seen);
        self.pattern(p);
    }

    /// The `<…>` type-argument list of a pattern.
    fn type_list(&self, types: &[ast::TypeId]) -> Option<Span> {
        let first = self.a.ty(*types.first()?).span;
        let last = self.a.ty(*types.last()?).span;
        let bytes = self.source.as_bytes();
        let mut start = first.start;
        while start > 0
            && bytes
                .get(start - 1)
                .is_some_and(|b| b.is_ascii_whitespace())
        {
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
        Some(Span {
            start: start - 1,
            end: end + 1,
        })
    }

    /// Repeated fields of a record or object pattern: on the repeated field's
    /// name (on the whole field, when the name is implicit, `:var x`).
    fn repeated_fields(&mut self, fields: &'a [ast::PatternField]) {
        let a = self.a;
        let mut seen: Vec<(SymbolId, Span)> = Vec::new();
        for f in fields.iter() {
            let (name, span) = match f.name {
                // `:var x` with the kept name (the variable's, inside the
                // subpattern): the target is the `:` that precedes it.
                Some(n)
                    if {
                        let sp = a.pattern(f.pattern).span;
                        sp.start <= n.span.start && n.span.end <= sp.end
                    } =>
                {
                    let bytes = self.source.as_bytes();
                    let mut i = a.pattern(f.pattern).span.start;
                    while i > 0 && bytes.get(i - 1).is_some_and(|b| b.is_ascii_whitespace()) {
                        i -= 1;
                    }
                    let colon = if i > 0 && bytes.get(i - 1) == Some(&b':') {
                        Span {
                            start: i - 1,
                            end: i,
                        }
                    } else {
                        n.span
                    };
                    (Some(n.sym), colon)
                }
                Some(n) => (Some(n.sym), n.span),
                None => {
                    // `:var x` (the field starts at `:`): the name is the variable's.
                    let implicit = self.source.as_bytes().get(f.span.start) == Some(&b':');
                    let mut q = f.pattern;
                    let name = loop {
                        match &a.pattern(q).kind {
                            PatternKind::Variable { name, .. } => break Some(name.sym),
                            PatternKind::NullCheck(x)
                            | PatternKind::NullAssert(x)
                            | PatternKind::Parenthesized(x) => q = *x,
                            PatternKind::Cast { pattern, .. } => q = *pattern,
                            _ => break None,
                        }
                    };
                    // The target of the implicit name is the `:` (`name ?? colon`).
                    (
                        if implicit { name } else { None },
                        Span {
                            start: f.span.start,
                            end: f.span.start + 1,
                        },
                    )
                }
            };
            let Some(name) = name else { continue };
            if let Some(&(_, first)) = seen.iter().find(|(s, _)| *s == name) {
                let text = self.names.resolve(name).to_string();
                self.out.push(
                    Diagnostic::with_code(c::DUPLICATE_PATTERN_FIELD, span, [text.as_str()])
                        .with_context(first, "The first field."),
                );
            } else {
                seen.push((name, span));
            }
        }
    }

    fn pattern(&mut self, p: PatternId) {
        let a = self.a;
        let pattern_span = a.pattern(p).span;
        match &a.pattern(p).kind {
            PatternKind::Constant(e) => self.expr(*e),
            PatternKind::Relational { value, .. } => self.expr(*value),
            PatternKind::Or(x, y) | PatternKind::And(x, y) => {
                self.pattern(*x);
                self.pattern(*y);
            }
            PatternKind::NullCheck(x)
            | PatternKind::NullAssert(x)
            | PatternKind::Parenthesized(x) => self.pattern(*x),
            PatternKind::Cast { pattern, .. } => self.pattern(*pattern),
            PatternKind::List {
                type_args,
                elements,
            } => {
                if type_args.len() > 1
                    && let Some(sp) = self.type_list(type_args)
                {
                    let n = type_args.len().to_string();
                    self.report(c::EXPECTED_ONE_LIST_PATTERN_TYPE_ARGUMENTS, sp, &[&n]);
                }
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x)
                        | ast::ListPatternElement::Rest(Some(x)) => self.pattern(*x),
                        _ => {}
                    }
                }
            }
            PatternKind::Map {
                type_args,
                entries,
                rest,
            } => {
                if !type_args.is_empty()
                    && type_args.len() != 2
                    && let Some(sp) = self.type_list(type_args)
                {
                    let n = type_args.len().to_string();
                    self.report(c::EXPECTED_TWO_MAP_PATTERN_TYPE_ARGUMENTS, sp, &[&n]);
                }
                // A map pattern has no rest: on the `...`.
                if *rest {
                    let from = entries
                        .last()
                        .map_or(pattern_span.start, |en| a.pattern(en.value).span.end);
                    if let Some(i) = self
                        .source
                        .get(from..pattern_span.end)
                        .and_then(|t| t.find("..."))
                    {
                        self.report(
                            c::REST_ELEMENT_IN_MAP_PATTERN,
                            Span {
                                start: from + i,
                                end: from + i + 3,
                            },
                            &[],
                        );
                    }
                }
                for en in entries.iter() {
                    self.expr(en.key);
                    self.pattern(en.value);
                }
                // The rest is an element (`MapPattern.elements` with the
                // `RestPatternElement`): `{...}` is not empty, it only has
                // `REST_ELEMENT_IN_MAP_PATTERN`.
                if entries.is_empty() && !*rest {
                    self.report(c::EMPTY_MAP_PATTERN, pattern_span, &[]);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                self.repeated_fields(fields);
                for f in fields.iter() {
                    self.pattern(f.pattern);
                }
            }
            _ => {}
        }
    }

    fn element(&mut self, el: &'a CollectionElement) {
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => {
                self.expr(*x)
            }
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(*key);
                self.expr(*value);
            }
            CollectionElement::Spread { value, .. } => self.expr(*value),
            CollectionElement::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.top_level_pattern(*p, PatternContext::Refutable);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.element(then);
                if let Some(x) = else_ {
                    self.element(x);
                }
            }
            CollectionElement::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                self.for_init(init);
                if let Some(x) = condition {
                    self.expr(*x);
                }
                for x in updates.iter() {
                    self.expr(*x);
                }
                self.element(body);
            }
            CollectionElement::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                self.for_in_target(target);
                self.expr(*iterable);
                self.element(body);
            }
        }
    }

    fn expr(&mut self, e: ExprId) {
        let a = self.a;
        let node = a.expr(e);
        match &node.kind {
            ExprKind::Rethrow => {
                if self.catch_depth == 0 {
                    self.report(c::RETHROW_OUTSIDE_CATCH, node.span, &[]);
                }
            }
            ExprKind::FunctionExpression(f) => self.function(*f),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    self.element(el);
                }
            }
            ExprKind::Record {
                positional, named, ..
            } => {
                for x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                    self.expr(*x);
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => {
                for x in arguments.args.iter() {
                    self.expr(x.value);
                }
            }
            ExprKind::String(lit) => {
                for p in lit.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(*x);
                    }
                }
            }
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x),
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
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
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
                self.top_level_pattern(*pattern, PatternContext::Assignment);
                self.expr(*value);
            }
            ExprKind::Cascade {
                target, sections, ..
            } => {
                self.expr(*target);
                for s in sections.iter() {
                    self.expr(*s);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value);
                for case in cases.iter() {
                    self.top_level_pattern(case.pattern, PatternContext::Refutable);
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

/// The first occurrence of `word` in `text` as a whole word.
fn first_word(text: &str, word: &str) -> Option<usize> {
    let b = text.as_bytes();
    let is_word_char = |x: u8| x.is_ascii_alphanumeric() || x == b'_';
    let mut from = 0;
    while let Some(i) = text.get(from..).and_then(|t| t.find(word)) {
        let start = from + i;
        let after = start + word.len();
        if (start == 0 || !is_word_char(b[start - 1]))
            && (after >= b.len() || !is_word_char(b[after]))
        {
            return Some(start);
        }
        from = after;
    }
    None
}

/// The last occurrence of `word` in `text` as a whole word.
fn last_word(text: &str, word: &str) -> Option<usize> {
    let b = text.as_bytes();
    let is_word_char = |x: u8| x.is_ascii_alphanumeric() || x == b'_';
    let mut end = text.len();
    while let Some(i) = text[..end].rfind(word) {
        let after = i + word.len();
        if (i == 0 || !is_word_char(b[i - 1])) && (after >= b.len() || !is_word_char(b[after])) {
            return Some(i);
        }
        if i == 0 {
            break;
        }
        end = i;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(source: &str) -> Vec<(&'static str, usize, usize)> {
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        let u = Unit {
            ast: &p.ast,
            unit: &p.unit,
            source,
        };
        check(u, &names)
            .into_iter()
            .map(|d| {
                (
                    d.code.map_or("", |c| c.info().name),
                    d.span.start,
                    d.span.end,
                )
            })
            .collect()
    }

    #[test]
    fn rethrow_outside_and_inside_catch() {
        // void f() { rethrow; }
        assert_eq!(
            codes("void f() { rethrow; }"),
            vec![("rethrow_outside_catch", 11, 18)]
        );
        assert!(codes("void f() { try {} catch (e) { rethrow; } }").is_empty());
        // The function literal inside the `catch` restarts the count.
        let r = codes("void f() { try {} catch (e) { () { rethrow; }; } }");
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn return_with_value_in_generator() {
        // f() async* { return 0; }
        assert_eq!(
            codes("f() async* { return 0; }"),
            vec![("return_in_generator", 13, 19)]
        );
        // f() sync* => 0;
        assert_eq!(
            codes("f() sync* => 0;"),
            vec![("return_in_generator", 10, 12)]
        );
        assert!(codes("f() async* { return; }").is_empty());
    }

    #[test]
    fn labels() {
        // void f() { break L; }
        assert_eq!(
            codes("void f() { break L; }"),
            vec![("label_undefined", 17, 18)]
        );
        let r = codes("void f() { L: while (true) { () { break L; }; } }");
        assert_eq!(r[0].0, "label_in_outer_scope");
        // Outside a loop both come out: the parser's and the resolution's.
        let r = codes("void f() { L: { continue L; } }");
        let names: Vec<&str> = r.iter().map(|x| x.0).collect();
        assert_eq!(
            names,
            vec!["continue_outside_of_loop", "continue_label_invalid"]
        );
        assert!(codes("void f() { L: while (true) { continue L; } }").is_empty());
    }

    #[test]
    fn jumps_outside_loop() {
        // void f() { break; }
        assert_eq!(
            codes("void f() { break; }"),
            vec![("break_outside_of_loop", 11, 16)]
        );
        assert_eq!(
            codes("void f() { continue; }"),
            vec![("continue_outside_of_loop", 11, 19)]
        );
        let r = codes("void f(x) { switch (x) { case 1: continue; } }");
        assert_eq!(r[0].0, "continue_without_label_in_case");
        assert!(codes("void f(x) { while (x) { switch (x) { case 1: continue; } } }").is_empty());
        assert!(codes("void f(x) { switch (x) { case 1: break; } }").is_empty());
    }

    #[test]
    fn unused_label_and_member_label() {
        // void f() { L: while (true) {} }
        assert_eq!(
            codes("void f() { L: while (true) {} }"),
            vec![("unused_label", 11, 13)]
        ); // the `Label` node: `L:`
        let r = codes("void f(x) { switch (x) { L: case 1: break L; } }");
        assert_eq!(r[0].0, "break_label_on_switch_member");
    }

    #[test]
    fn pattern_shapes() {
        let r = codes("void f(x) { var (a, a) = x; }");
        assert_eq!(r[0].0, "duplicate_variable_pattern");
        let r = codes("void f(x) { var a, b; (a, a) = x; }");
        assert_eq!(r[0].0, "duplicate_pattern_assignment_variable");
        let r = codes("void f(x) { if (x case {}) {} }");
        assert_eq!(r[0].0, "empty_map_pattern");
        // Both sides of `||` declare the same variable: not a repetition.
        assert!(codes("void f(x) { if (x case int a || int a) {} }").is_empty());
    }

    #[test]
    fn unnecessary_final_and_late_final() {
        // class A { int x; A(final this.x); }
        let r = codes("class A { int x; A(final this.x); }");
        assert_eq!(r, vec![("unnecessary_final", 19, 24)]);
        // class A { late final int x; const A(); }
        let r = codes("class A { late final int x; const A(); }");
        assert_eq!(r, vec![("late_final_field_with_const_constructor", 10, 14)]);
        assert!(codes("class A { late final int x; A(); }").is_empty());
    }

    #[test]
    fn main_that_is_not_a_function() {
        // var main = 1;
        assert_eq!(codes("var main = 1;"), vec![("main_is_not_function", 4, 8)]);
        assert!(codes("void main() {}").is_empty());
    }
}
