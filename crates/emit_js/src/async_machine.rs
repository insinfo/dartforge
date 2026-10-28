//! Production: an `async` body as a state machine instead of a JavaScript
//! generator (`docs/JS-PRODUCAO-VELOCIDADE.md` §9).
//!
//! The development contract (DDC) compiles `async` with a generator: each
//! `await` is a `yield`, and the step registered in the zone resumes it with
//! `next`/`throw`. `dart2js` writes a `switch` over a state variable instead,
//! and so does this lowering, over the same runtime helpers
//! (`_makeAsyncAwaitCompleter`, `_wrapJsFunctionForAsync`, `_asyncStartSync`,
//! `_asyncAwait`, `_asyncReturn`, `_asyncRethrow`): the same completer, the
//! same zone-registered step, the same microtasks, in the same order. Only the
//! generator object, its `{value, done}` results and its frames go away.
//!
//! The input is the generator body the emitter already wrote. It is parsed
//! (oxc) and rewritten by splicing: code without `yield` is copied as it is
//! (renaming only the hoisted variables), and only the control structure
//! around the `yield`s becomes states:
//!
//! * the bindings of the blocks that hold a `yield` move to the enclosing
//!   function (they must outlive each step), renamed when the name is taken;
//! * an expression with a `yield` is evaluated in order, the values computed
//!   before the `await` kept in temporaries unless nothing can change them;
//! * loops, `if`, `switch`, labels and `try`/`catch`/`finally` become states;
//!   an error (thrown in a step, or the error of an awaited future) goes to
//!   the state of the innermost handler, as in `dart2js` (`t$handler`), and
//!   `finally` continues to the states stacked in `t$next`.
//!
//! Whatever falls outside what this handles (`yield*`, patterns in hoisted
//! declarations, a `yield` in a template, a captured loop variable that would
//! lose its per-iteration binding, …) keeps the generator: the lowering
//! refuses, it never guesses.

use oxc_ast::ast::{
    Argument, ArrayExpressionElement, AssignmentTarget, BindingPattern, Expression,
    ForStatementInit, ForStatementLeft, ObjectPropertyKind, PropertyKey, Statement,
    VariableDeclaration,
};
use oxc_ast_visit::{Visit, walk};
use oxc_semantic::Scoping;
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{LogicalOperator, UnaryOperator};
use oxc_syntax::scope::ScopeFlags;
use oxc_syntax::symbol::SymbolId;
use std::collections::{HashMap, HashSet};

/// Why the generator stays (diagnostics: `DARTFORGE_JSPROD_ASYNC_MACHINE_DEBUG=1`).
type Refusal = &'static str;

/// The text that replaces the generator, from the completer to the
/// `_asyncStartSync` (the caller writes the head and the prologue).
pub(crate) fn lower_async(
    body: &str,
    rti: &str,
    outer_text: &str,
    mutable_free: &HashSet<String>,
) -> Result<String, Refusal> {
    let src = format!("(function*() {{\n{body}}})");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &src, oxc_span::SourceType::cjs()).parse();
    if !parsed.diagnostics.is_empty() {
        return Err("parse");
    }
    let Some(Statement::ExpressionStatement(es)) = parsed.program.body.first() else {
        return Err("shape");
    };
    let mut callee = &es.expression;
    while let Expression::ParenthesizedExpression(p) = callee {
        callee = &p.expression;
    }
    let Expression::FunctionExpression(f) = callee else {
        return Err("shape");
    };
    let Some(fbody) = &f.body else {
        return Err("shape");
    };
    let semantic = oxc_semantic::SemanticBuilder::new()
        .build(&parsed.program)
        .semantic;
    let scoping = semantic.scoping();

    let mut facts = Facts {
        scoping,
        depth: 0,
        yields: Vec::new(),
        captured: HashSet::new(),
        free_written: HashSet::new(),
        names: HashSet::new(),
        refusal: None,
    };
    for s in &fbody.statements {
        facts.visit_statement(s);
    }
    if let Some(r) = facts.refusal {
        return Err(r);
    }
    facts.yields.sort_unstable();
    let mut used_names = facts.names;
    let outer_words: HashSet<String> = words(outer_text).map(str::to_string).collect();
    used_names.extend(outer_words.iter().cloned());

    let mut lw = Lower {
        src: &src,
        scoping,
        yields: facts.yields,
        captured: facts.captured,
        free_written: facts.free_written,
        mutable_free,
        renames: HashMap::new(),
        hoisted: Vec::new(),
        hoisted_syms: HashSet::new(),
        used_names,
        outer_words,
        out: String::new(),
        states: 0,
        pending_goto: None,
        reachable: true,
        uses_loop: false,
        uses_label: false,
        has_try: false,
        uses_next: false,
        ret_state: None,
        handler: 0,
        top_handler: 0,
        finallies: Vec::new(),
        targets: Vec::new(),
        targets_floor: 0,
        in_finally_body: 0,
        pending_labels: Vec::new(),
        temp_pool: Vec::new(),
        temps_taken: Vec::new(),
        loop_depth: 0,
        inline_returns: Vec::new(),
        try_starts: Vec::new(),
    };
    // Bindings of the blocks that need states, with their renames, before
    // anything is rendered (a closure may name a binding declared later).
    lw.has_try = lw.plan(&fbody.statements, 0)?;
    lw.try_starts.sort_unstable();
    if lw.has_try {
        lw.top_handler = lw.state();
        lw.handler = lw.top_handler;
    }
    lw.out.push_str("case 0:\n");
    for s in &fbody.statements {
        lw.stmt(s)?;
        lw.release_temps();
    }
    if lw.reachable {
        lw.emit("return async._asyncReturn(void 0, t$completer);");
        lw.reachable = false;
    }
    lw.flush();
    if let Some(r) = lw.ret_state {
        lw.out.push_str(&format!(
            "case {r}:\nreturn async._asyncReturn(t$ret, t$completer);\n"
        ));
    }
    if lw.has_try {
        lw.out.push_str(&format!(
            "case {}:\nreturn async._asyncRethrow(t$err, t$completer);\n",
            lw.top_handler
        ));
    }

    let mut vars = format!("let t$completer = async._makeAsyncAwaitCompleter({rti}), t$goto = 0");
    if lw.has_try {
        vars.push_str(&format!(", t$handler = {}, t$err", lw.top_handler));
    }
    if lw.uses_next {
        vars.push_str(", t$next = []");
    }
    if lw.ret_state.is_some() {
        vars.push_str(", t$ret");
    }
    for h in &lw.hoisted {
        vars.push_str(", ");
        vars.push_str(h);
    }
    vars.push_str(";\n");
    let on_error = if lw.has_try {
        "if (t$code === 1) { t$err = t$res; t$goto = t$handler; }\n"
    } else {
        "if (t$code === 1) return async._asyncRethrow(t$res, t$completer);\n"
    };
    let dispatch = match (lw.uses_label, lw.uses_loop) {
        (true, _) => "t$sm: while (true) switch (t$goto) {\n",
        (false, true) => "while (true) switch (t$goto) {\n",
        (false, false) => "switch (t$goto) {\n",
    };
    Ok(format!(
        "{vars}let t$body = async._wrapJsFunctionForAsync((t$code, t$res) => {{\n{on_error}{dispatch}{}}}\n}});\nreturn async._asyncStartSync(t$body, t$completer);\n",
        lw.out
    ))
}

/// The body of the emitter's IIFE in a generator,
/// `yield* (function*() {…}).call(this)` (`FnEmitter::iife`).
fn inline_iife<'b, 'a>(e: &'b Expression<'a>) -> Option<&'b oxc_ast::ast::FunctionBody<'a>> {
    let Expression::CallExpression(c) = e else {
        return None;
    };
    let Expression::StaticMemberExpression(m) = &c.callee else {
        return None;
    };
    if m.property.name != "call" || !matches!(c.arguments.as_slice(), [Argument::ThisExpression(_)])
    {
        return None;
    }
    let mut f = &m.object;
    while let Expression::ParenthesizedExpression(p) = f {
        f = &p.expression;
    }
    match f {
        Expression::FunctionExpression(f)
            if f.generator && f.params.items.is_empty() && f.params.rest.is_none() =>
        {
            f.body.as_deref()
        }
        _ => None,
    }
}

/// The identifier-like words of a text (names the hoisted variables must not
/// take: the parameters and the prologue).
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
        .filter(|w| !w.is_empty())
}

/// What the lowering needs to know before it starts.
struct Facts<'s> {
    scoping: &'s Scoping,
    /// Nesting of functions and classes (0: the generator body).
    depth: u32,
    /// Start of each `yield` of the generator itself.
    yields: Vec<u32>,
    /// Bindings named from a nested function or class.
    captured: HashSet<SymbolId>,
    /// Free names the body assigns.
    free_written: HashSet<String>,
    /// Every name the body declares or cites.
    names: HashSet<String>,
    refusal: Option<Refusal>,
}

impl<'a> Visit<'a> for Facts<'_> {
    fn visit_function(&mut self, it: &oxc_ast::ast::Function<'a>, flags: ScopeFlags) {
        self.depth += 1;
        walk::walk_function(self, it, flags);
        self.depth -= 1;
    }
    fn visit_arrow_function_expression(&mut self, it: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        self.depth += 1;
        walk::walk_arrow_function_expression(self, it);
        self.depth -= 1;
    }
    fn visit_class(&mut self, it: &oxc_ast::ast::Class<'a>) {
        self.depth += 1;
        walk::walk_class(self, it);
        self.depth -= 1;
    }
    fn visit_yield_expression(&mut self, it: &oxc_ast::ast::YieldExpression<'a>) {
        if self.depth == 0 {
            self.yields.push(it.span.start);
            if it.delegate {
                // The emitter's IIFE in a generator: its body is part of
                // this one (its `yield`s are this function's `await`s).
                match it.argument.as_ref().and_then(inline_iife) {
                    Some(body) => {
                        for s in &body.statements {
                            self.visit_statement(s);
                        }
                    }
                    None => self.refusal = Some("yield*"),
                }
                return;
            }
        }
        walk::walk_yield_expression(self, it);
    }
    fn visit_new_target(&mut self, _it: &oxc_ast::ast::NewTarget) {
        self.refusal = Some("new.target");
    }
    fn visit_identifier_reference(&mut self, it: &oxc_ast::ast::IdentifierReference<'a>) {
        self.names.insert(it.name.to_string());
        if let Some(r) = it.reference_id.get() {
            let reference = self.scoping.get_reference(r);
            match reference.symbol_id() {
                Some(sym) if self.depth > 0 => {
                    self.captured.insert(sym);
                }
                Some(_) => {}
                None if reference.is_write() => {
                    self.free_written.insert(it.name.to_string());
                }
                None => {}
            }
        }
    }
    fn visit_binding_identifier(&mut self, it: &oxc_ast::ast::BindingIdentifier<'a>) {
        self.names.insert(it.name.to_string());
    }
}

/// A `break`/`continue` target of the states.
struct Target {
    labels: Vec<String>,
    /// Unlabeled `break` reaches it (a loop or a `switch`).
    breakable: bool,
    brk: u32,
    cont: Option<u32>,
    /// `finallies.len()` where the target is.
    fdepth: usize,
}

struct Lower<'s> {
    src: &'s str,
    scoping: &'s Scoping,
    yields: Vec<u32>,
    captured: HashSet<SymbolId>,
    free_written: HashSet<String>,
    mutable_free: &'s HashSet<String>,
    renames: HashMap<SymbolId, String>,
    /// Hoisted names (bindings and temporaries), in order.
    hoisted: Vec<String>,
    hoisted_syms: HashSet<SymbolId>,
    used_names: HashSet<String>,
    /// Names of the enclosing function's head and prologue.
    outer_words: HashSet<String>,
    /// The `case`s.
    out: String,
    states: u32,
    pending_goto: Option<u32>,
    reachable: bool,
    uses_loop: bool,
    uses_label: bool,
    has_try: bool,
    uses_next: bool,
    ret_state: Option<u32>,
    /// The state errors go to here.
    handler: u32,
    top_handler: u32,
    /// The `finally` states of the enclosing `try`s, innermost last.
    finallies: Vec<u32>,
    targets: Vec<Target>,
    /// Jumps from a `finally` body must stay inside it (`t$next` would keep
    /// a stale continuation).
    targets_floor: usize,
    in_finally_body: u32,
    pending_labels: Vec<String>,
    temp_pool: Vec<String>,
    temps_taken: Vec<String>,
    loop_depth: u32,
    /// The inlined IIFEs being lowered, innermost last: their `return`.
    inline_returns: Vec<InlineReturn>,
    /// Starts of the `try`s that need states, sorted.
    try_starts: Vec<u32>,
}

/// Where the `return` of an inlined IIFE goes.
struct InlineReturn {
    result: String,
    end: u32,
    fdepth: usize,
    finally_level: u32,
}

impl Lower<'_> {
    fn text(&self, span: Span) -> &str {
        &self.src[span.start as usize..span.end as usize]
    }

    fn has_yield(&self, span: Span) -> bool {
        let i = self.yields.partition_point(|&y| y < span.start);
        i < self.yields.len() && self.yields[i] < span.end
    }

    fn state(&mut self) -> u32 {
        self.states += 1;
        self.states
    }

    fn fresh(&mut self, base: &str) -> String {
        let mut n = 1;
        loop {
            let name = format!("{base}${n}");
            if self.used_names.insert(name.clone()) {
                return name;
            }
            n += 1;
        }
    }

    fn temp(&mut self) -> String {
        let t = self.temp_pool.pop().unwrap_or_else(|| {
            let t = self.fresh("t$t");
            self.hoisted.push(t.clone());
            t
        });
        self.temps_taken.push(t.clone());
        t
    }

    fn release_temps(&mut self) {
        self.temp_pool.append(&mut self.temps_taken);
    }

    /// A temporary of its own (never returned to the pool).
    fn own_temp(&mut self, base: &str) -> String {
        let t = self.fresh(base);
        self.hoisted.push(t.clone());
        t
    }

    // ---------------------------------------------------------------- plan

    /// Hoists the bindings of the statement lists that need states. Returns
    /// whether some `try` needs states.
    fn plan(&mut self, stmts: &[Statement<'_>], loops: u32) -> Result<bool, Refusal> {
        let mut has_try = false;
        for s in stmts {
            has_try |= self.plan_stmt(s, loops)?;
        }
        Ok(has_try)
    }

    fn hoist(&mut self, pattern: &BindingPattern<'_>, loops: u32) -> Result<(), Refusal> {
        let BindingPattern::BindingIdentifier(id) = pattern else {
            return Err("pattern declaration");
        };
        let Some(sym) = id.symbol_id.get() else {
            return Err("unbound declaration");
        };
        if self.hoisted_syms.contains(&sym) {
            return Ok(());
        }
        if loops > 0 && self.captured.contains(&sym) {
            return Err("captured loop binding");
        }
        let name = id.name.as_str();
        // The name stays when no other binding or free name uses it.
        let clash = self
            .scoping
            .symbol_ids()
            .any(|o| o != sym && self.scoping.symbol_name(o) == name)
            || self.scoping.root_unresolved_references().contains_key(name)
            || self.hoisted.iter().any(|h| h == name);
        let js = if clash || self.outer_words.contains(name) {
            let n = self.fresh(name);
            self.renames.insert(sym, n.clone());
            n
        } else {
            name.to_string()
        };
        if self.hoisted_syms.insert(sym) {
            self.hoisted.push(js);
        }
        Ok(())
    }

    fn plan_decl(&mut self, d: &VariableDeclaration<'_>, loops: u32) -> Result<(), Refusal> {
        for v in &d.declarations {
            self.hoist(&v.id, loops)?;
        }
        Ok(())
    }

    /// The IIFEs inlined in expressions: their bindings are hoisted too.
    fn plan_exprs(
        &mut self,
        exprs: &[Option<&Expression<'_>>],
        loops: u32,
    ) -> Result<bool, Refusal> {
        let mut v = PlanIife {
            lw: self,
            loops,
            depth: 0,
            has_try: false,
            refusal: None,
        };
        for e in exprs.iter().flatten() {
            v.visit_expression(e);
        }
        let has_try = v.has_try;
        match v.refusal {
            Some(r) => Err(r),
            None => Ok(has_try),
        }
    }

    fn plan_stmt(&mut self, s: &Statement<'_>, loops: u32) -> Result<bool, Refusal> {
        if let Statement::VariableDeclaration(d) = s {
            self.plan_decl(d, loops)?;
            if !self.has_yield(d.span) {
                return Ok(false);
            }
            let inits: Vec<Option<&Expression<'_>>> =
                d.declarations.iter().map(|v| v.init.as_ref()).collect();
            return self.plan_exprs(&inits, loops);
        }
        if let Statement::FunctionDeclaration(_) | Statement::ClassDeclaration(_) = s {
            return Err("declaration in a block with states");
        }
        if !self.has_yield(s.span()) {
            return Ok(false);
        }
        let exprs: Vec<Option<&Expression<'_>>> = match s {
            Statement::ExpressionStatement(e) => vec![Some(&e.expression)],
            Statement::ReturnStatement(r) => vec![r.argument.as_ref()],
            Statement::ThrowStatement(t) => vec![Some(&t.argument)],
            Statement::IfStatement(i) => vec![Some(&i.test)],
            Statement::WhileStatement(w) => vec![Some(&w.test)],
            Statement::DoWhileStatement(w) => vec![Some(&w.test)],
            Statement::ForStatement(f) => vec![
                f.init.as_ref().and_then(|i| i.as_expression()),
                f.test.as_ref(),
                f.update.as_ref(),
            ],
            Statement::ForOfStatement(f) => vec![Some(&f.right)],
            Statement::SwitchStatement(sw) => vec![Some(&sw.discriminant)],
            _ => Vec::new(),
        };
        let loops_in = match s {
            Statement::WhileStatement(_)
            | Statement::DoWhileStatement(_)
            | Statement::ForStatement(_) => loops + 1,
            _ => loops,
        };
        let in_exprs = self.plan_exprs(&exprs, loops_in)?;
        Ok(in_exprs
            || match s {
                Statement::BlockStatement(b) => self.plan(&b.body, loops)?,
                Statement::IfStatement(i) => {
                    let a = self.plan_stmt(&i.consequent, loops)?;
                    let b = match &i.alternate {
                        Some(e) => self.plan_stmt(e, loops)?,
                        None => false,
                    };
                    a || b
                }
                Statement::WhileStatement(w) => self.plan_stmt(&w.body, loops + 1)?,
                Statement::DoWhileStatement(w) => self.plan_stmt(&w.body, loops + 1)?,
                Statement::ForStatement(f) => {
                    if let Some(ForStatementInit::VariableDeclaration(d)) = &f.init {
                        self.plan_decl(d, loops + 1)?;
                    }
                    self.plan_stmt(&f.body, loops + 1)?
                }
                Statement::ForOfStatement(f) => {
                    if f.r#await {
                        return Err("for await");
                    }
                    if let ForStatementLeft::VariableDeclaration(d) = &f.left {
                        self.plan_decl(d, loops + 1)?;
                    }
                    self.plan_stmt(&f.body, loops + 1)?
                }
                Statement::LabeledStatement(l) => self.plan_stmt(&l.body, loops)?,
                Statement::SwitchStatement(sw) => {
                    let mut t = false;
                    for c in &sw.cases {
                        t |= self.plan(&c.consequent, loops)?;
                    }
                    t
                }
                Statement::TryStatement(t) => {
                    self.try_starts.push(t.span.start);
                    self.plan(&t.block.body, loops)?;
                    if let Some(h) = &t.handler {
                        if let Some(p) = &h.param {
                            self.hoist(&p.pattern, loops)?;
                        }
                        self.plan(&h.body.body, loops)?;
                    }
                    if let Some(f) = &t.finalizer {
                        self.plan(&f.body, loops)?;
                    }
                    true
                }
                Statement::ExpressionStatement(_)
                | Statement::ReturnStatement(_)
                | Statement::ThrowStatement(_) => false,
                _ => return Err("statement kind"),
            })
    }

    // ------------------------------------------------------------ emission

    fn flush(&mut self) {
        if let Some(s) = self.pending_goto.take() {
            self.out.push_str(&format!("t$goto = {s}; break;\n"));
            self.uses_loop = true;
        }
    }

    fn emit(&mut self, text: &str) {
        if !self.reachable {
            return;
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn label(&mut self, s: u32) {
        if self.pending_goto == Some(s) {
            self.pending_goto = None;
        } else {
            self.flush();
        }
        self.out.push_str(&format!("case {s}:\n"));
        self.reachable = true;
    }

    fn goto(&mut self, s: u32) {
        if self.reachable {
            self.pending_goto = Some(s);
            self.reachable = false;
        }
    }

    /// `if (cond) goto s;`
    fn goto_if(&mut self, cond: &str, s: u32) {
        self.uses_loop = true;
        self.emit(&format!("if ({cond}) {{ t$goto = {s}; break; }}"));
    }

    /// Text of a jump to `state` at `fdepth`, through the `finally`s in
    /// between. `verbatim`: from copied code (inside loops or `switch`es of
    /// its own, where `break` does not reach the dispatch).
    fn jump_text(&mut self, state: u32, fdepth: usize, verbatim: bool) -> String {
        let crossing: Vec<u32> = self.finallies[fdepth..].to_vec();
        let (first, then) = match crossing.split_last() {
            None => (state, String::new()),
            Some((&inner, outer)) => {
                self.uses_next = true;
                let mut pushed = vec![state.to_string()];
                pushed.extend(outer.iter().map(u32::to_string));
                (inner, format!("t$next.push({}); ", pushed.join(", ")))
            }
        };
        self.uses_loop = true;
        if verbatim {
            self.uses_label = true;
            format!("{then}t$goto = {first}; continue t$sm;")
        } else {
            format!("{then}t$goto = {first}; break;")
        }
    }

    fn ret_state(&mut self) -> u32 {
        match self.ret_state {
            Some(s) => s,
            None => {
                let s = self.state();
                self.ret_state = Some(s);
                s
            }
        }
    }

    /// Resolves a `break`/`continue` to the states' targets.
    fn resolve(&self, label: Option<&str>, is_continue: bool) -> Result<(u32, usize), Refusal> {
        let found = self
            .targets
            .iter()
            .enumerate()
            .rev()
            .find(|(_, t)| match label {
                Some(l) => t.labels.iter().any(|x| x == l) && (!is_continue || t.cont.is_some()),
                None if is_continue => t.cont.is_some(),
                None => t.breakable,
            });
        let Some((i, t)) = found else {
            return Err("jump target");
        };
        if i < self.targets_floor {
            return Err("jump out of finally");
        }
        Ok((
            if is_continue {
                t.cont.unwrap_or(t.brk)
            } else {
                t.brk
            },
            t.fdepth,
        ))
    }

    /// The `return` of a value already evaluated (`value`: an expression).
    /// `local_finally`: from copied code inside a JavaScript `try … finally`
    /// of its own, which must run before the completion.
    fn return_text(
        &mut self,
        value: &str,
        verbatim: bool,
        local_finally: bool,
    ) -> Result<String, Refusal> {
        let (state, fdepth, assign) = match self.inline_returns.last() {
            Some(ir) => {
                if self.in_finally_body > ir.finally_level {
                    return Err("return in finally");
                }
                (ir.end, ir.fdepth, format!("{} = {value};", ir.result))
            }
            None => {
                if self.in_finally_body > 0 {
                    return Err("return in finally");
                }
                if self.finallies.is_empty() && !local_finally {
                    return Ok(format!("return async._asyncReturn({value}, t$completer);"));
                }
                let r = self.ret_state();
                (r, 0, format!("t$ret = {value};"))
            }
        };
        if local_finally && self.finallies.len() > fdepth {
            return Err("return through two finallies");
        }
        let jump = self.jump_text(state, fdepth, verbatim);
        Ok(format!("{assign} {jump}"))
    }

    // -------------------------------------------------------------- render

    /// Copies a part without `yield`, renaming the hoisted bindings and
    /// rewriting the `return`s and the jumps that leave it.
    fn render(
        &mut self,
        span: Span,
        node: &dyn Fn(&mut Render<'_, '_>),
    ) -> Result<String, Refusal> {
        let mut r = Render {
            lw: self,
            edits: Vec::new(),
            depth: 0,
            loops: 0,
            switches: 0,
            labels: Vec::new(),
            local_finally: 0,
            refusal: None,
        };
        node(&mut r);
        if let Some(e) = r.refusal {
            return Err(e);
        }
        let mut edits = r.edits;
        edits.sort_by_key(|e| e.0);
        let mut out = String::new();
        let mut at = span.start;
        for (s, e, t) in edits {
            out.push_str(&self.src[at as usize..s as usize]);
            out.push_str(&t);
            at = e;
        }
        out.push_str(&self.src[at as usize..span.end as usize]);
        Ok(out)
    }

    fn render_expr(&mut self, e: &Expression<'_>) -> Result<String, Refusal> {
        self.render(e.span(), &|r| r.visit_expression(e))
    }

    fn render_stmt(&mut self, s: &Statement<'_>) -> Result<String, Refusal> {
        self.render(s.span(), &|r| r.visit_statement(s))
    }

    // ---------------------------------------------------------- statements

    fn stmt(&mut self, s: &Statement<'_>) -> Result<(), Refusal> {
        if let Statement::VariableDeclaration(d) = s {
            return self.declaration(d);
        }
        if !self.has_yield(s.span()) {
            let t = self.render_stmt(s)?;
            self.emit(&t);
            if matches!(
                s,
                Statement::ReturnStatement(_) | Statement::ThrowStatement(_)
            ) {
                self.reachable = false;
            }
            return Ok(());
        }
        match s {
            Statement::ExpressionStatement(e) => {
                let v = self.expr(&e.expression)?;
                self.discard(&v);
            }
            Statement::BlockStatement(b) => {
                for s in &b.body {
                    self.stmt(s)?;
                    self.release_temps();
                }
            }
            Statement::ReturnStatement(r) => {
                let v = match &r.argument {
                    Some(a) => self.expr(a)?,
                    None => "void 0".into(),
                };
                let t = self.return_text(&v, false, false)?;
                self.emit(&t);
                self.reachable = false;
            }
            Statement::ThrowStatement(t) => {
                let v = self.expr(&t.argument)?;
                self.emit(&format!("throw {v};"));
                self.reachable = false;
            }
            Statement::IfStatement(i) => {
                let test = self.expr(&i.test)?;
                self.release_temps();
                let needs = self.has_yield(i.consequent.span())
                    || i.alternate
                        .as_ref()
                        .is_some_and(|a| self.has_yield(a.span()));
                if !needs {
                    let c = self.render_stmt(&i.consequent)?;
                    let text = match &i.alternate {
                        Some(a) => {
                            let a = self.render_stmt(a)?;
                            format!("if ({test}) {c}\nelse {a}")
                        }
                        None => format!("if ({test}) {c}"),
                    };
                    self.emit(&text);
                    return Ok(());
                }
                let end = self.state();
                match &i.alternate {
                    None => {
                        self.goto_if(&format!("!{}", paren(&test)), end);
                        self.stmt(&i.consequent)?;
                    }
                    Some(a) => {
                        let other = self.state();
                        self.goto_if(&format!("!{}", paren(&test)), other);
                        self.stmt(&i.consequent)?;
                        self.goto(end);
                        self.label(other);
                        self.stmt(a)?;
                    }
                }
                self.label(end);
            }
            Statement::WhileStatement(w) => {
                let top = self.state();
                let end = self.state();
                self.label(top);
                self.reset_handler(s.span());
                let test = self.expr(&w.test)?;
                self.goto_if(&format!("!{}", paren(&test)), end);
                self.release_temps();
                self.loop_body(&w.body, end, top)?;
                self.goto(top);
                self.label(end);
                self.reset_handler(s.span());
            }
            Statement::DoWhileStatement(w) => {
                let top = self.state();
                let test_state = self.state();
                let end = self.state();
                self.label(top);
                self.reset_handler(s.span());
                self.loop_body(&w.body, end, test_state)?;
                self.label(test_state);
                self.reset_handler(s.span());
                let test = self.expr(&w.test)?;
                self.goto_if(&test, top);
                self.release_temps();
                self.label(end);
                self.reset_handler(s.span());
            }
            Statement::ForStatement(f) => {
                match &f.init {
                    Some(ForStatementInit::VariableDeclaration(d)) => self.declaration(d)?,
                    Some(init) => {
                        let e = init.to_expression();
                        let v = self.expr(e)?;
                        self.discard(&v);
                        self.release_temps();
                    }
                    None => {}
                }
                let top = self.state();
                let update = self.state();
                let end = self.state();
                self.label(top);
                self.reset_handler(s.span());
                if let Some(t) = &f.test {
                    let test = self.expr(t)?;
                    self.goto_if(&format!("!{}", paren(&test)), end);
                    self.release_temps();
                }
                self.loop_body(&f.body, end, update)?;
                self.label(update);
                self.reset_handler(s.span());
                if let Some(u) = &f.update {
                    let v = self.expr(u)?;
                    self.discard(&v);
                    self.release_temps();
                }
                self.goto(top);
                self.label(end);
                self.reset_handler(s.span());
            }
            Statement::ForOfStatement(f) => {
                let iterable = self.expr(&f.right)?;
                let it = self.own_temp("t$i");
                let step = self.own_temp("t$s");
                self.emit(&format!("{it} = {}[Symbol.iterator]();", paren(&iterable)));
                self.release_temps();
                let top = self.state();
                let end = self.state();
                self.label(top);
                self.reset_handler(s.span());
                self.emit(&format!("{step} = {it}.next();"));
                self.goto_if(&format!("{step}.done"), end);
                let target = match &f.left {
                    ForStatementLeft::VariableDeclaration(d) => {
                        let [v] = d.declarations.as_slice() else {
                            return Err("for-of declaration");
                        };
                        self.binding_name(&v.id)?
                    }
                    ForStatementLeft::AssignmentTargetIdentifier(id) => {
                        self.render(id.span, &|r| r.visit_identifier_reference(id))?
                    }
                    _ => return Err("for-of target"),
                };
                self.emit(&format!("{target} = {step}.value;"));
                self.loop_body(&f.body, end, top)?;
                self.goto(top);
                self.label(end);
                self.reset_handler(s.span());
            }
            Statement::LabeledStatement(l) => {
                self.pending_labels.push(l.label.name.to_string());
                if l.body.is_iteration_statement() {
                    self.stmt(&l.body)?;
                } else {
                    let labels = std::mem::take(&mut self.pending_labels);
                    let end = self.state();
                    self.targets.push(Target {
                        labels,
                        breakable: false,
                        brk: end,
                        cont: None,
                        fdepth: self.finallies.len(),
                    });
                    self.stmt(&l.body)?;
                    self.targets.pop();
                    self.label(end);
                    self.reset_handler(s.span());
                }
            }
            Statement::SwitchStatement(sw) => {
                let d = self.expr(&sw.discriminant)?;
                let disc = self.own_temp("t$d");
                self.emit(&format!("{disc} = {d};"));
                self.release_temps();
                let end = self.state();
                let mut states = Vec::new();
                let mut default = None;
                for c in &sw.cases {
                    let s = self.state();
                    states.push(s);
                    match &c.test {
                        Some(t) => {
                            if self.has_yield(t.span()) {
                                return Err("yield in case");
                            }
                            let t = self.render_expr(t)?;
                            self.goto_if(&format!("{disc} === ({t})"), s);
                        }
                        None => default = Some(s),
                    }
                }
                self.goto(default.unwrap_or(end));
                let labels = std::mem::take(&mut self.pending_labels);
                self.targets.push(Target {
                    labels,
                    breakable: true,
                    brk: end,
                    cont: None,
                    fdepth: self.finallies.len(),
                });
                for (c, s) in sw.cases.iter().zip(states) {
                    self.label(s);
                    for st in &c.consequent {
                        self.stmt(st)?;
                        self.release_temps();
                    }
                }
                self.targets.pop();
                self.label(end);
                self.reset_handler(s.span());
            }
            Statement::TryStatement(t) => self.try_stmt(t)?,
            _ => return Err("statement with yield"),
        }
        Ok(())
    }

    /// After a target reached by a jump from another handler region.
    fn reset_handler(&mut self, construct: Span) {
        // Only a jump from a `try` inside the construct changes the handler.
        let i = self.try_starts.partition_point(|&t| t < construct.start);
        if i < self.try_starts.len() && self.try_starts[i] < construct.end {
            let h = self.handler;
            self.emit(&format!("t$handler = {h};"));
        }
    }

    fn loop_body(&mut self, body: &Statement<'_>, brk: u32, cont: u32) -> Result<(), Refusal> {
        let labels = std::mem::take(&mut self.pending_labels);
        self.targets.push(Target {
            labels,
            breakable: true,
            brk,
            cont: Some(cont),
            fdepth: self.finallies.len(),
        });
        self.loop_depth += 1;
        self.stmt(body)?;
        self.release_temps();
        self.loop_depth -= 1;
        self.targets.pop();
        Ok(())
    }

    fn binding_name(&self, p: &BindingPattern<'_>) -> Result<String, Refusal> {
        let BindingPattern::BindingIdentifier(id) = p else {
            return Err("pattern declaration");
        };
        let sym = id.symbol_id.get().ok_or("unbound declaration")?;
        Ok(self
            .renames
            .get(&sym)
            .cloned()
            .unwrap_or_else(|| id.name.to_string()))
    }

    /// A hoisted declaration: its initializers become assignments.
    fn declaration(&mut self, d: &VariableDeclaration<'_>) -> Result<(), Refusal> {
        for v in &d.declarations {
            let name = self.binding_name(&v.id)?;
            match &v.init {
                Some(init) => {
                    let value = self.expr(init)?;
                    self.emit(&format!("{name} = {value};"));
                }
                // A binding re-entered in a loop starts `undefined` again.
                None if self.loop_depth > 0 => self.emit(&format!("{name} = void 0;")),
                None => {}
            }
            self.release_temps();
        }
        Ok(())
    }

    fn try_stmt(&mut self, t: &oxc_ast::ast::TryStatement<'_>) -> Result<(), Refusal> {
        if self.in_finally_body > 0 && t.handler.is_some() {
            return Err("try-catch in finally");
        }
        let outer = self.handler;
        let after = self.state();
        let catch = t.handler.as_ref().map(|_| self.state());
        let fin = t.finalizer.as_ref().map(|_| self.state());
        let errfin = fin.map(|_| self.state());
        let rethrow = fin.map(|_| self.state());
        let saved_error = fin.map(|_| self.own_temp("t$e"));
        let body_handler = catch.or(errfin).unwrap_or(outer);
        self.emit(&format!("t$handler = {body_handler};"));
        self.handler = body_handler;
        if let Some(f) = fin {
            self.finallies.push(f);
        }
        for s in &t.block.body {
            self.stmt(s)?;
            self.release_temps();
        }
        // Normal end of the block.
        self.leave_try(fin, after);
        if let (Some(c), Some(h)) = (catch, &t.handler) {
            self.label(c);
            let catch_handler = errfin.unwrap_or(outer);
            self.emit(&format!("t$handler = {catch_handler};"));
            self.handler = catch_handler;
            if let Some(p) = &h.param {
                let name = self.binding_name(&p.pattern)?;
                self.emit(&format!("{name} = t$err;"));
            }
            for s in &h.body.body {
                self.stmt(s)?;
                self.release_temps();
            }
            self.leave_try(fin, after);
        }
        if let (Some(f), Some(ef), Some(rt), Some(e), Some(block)) =
            (fin, errfin, rethrow, &saved_error, &t.finalizer)
        {
            self.finallies.pop();
            self.label(ef);
            self.uses_next = true;
            self.emit(&format!("{e} = t$err; t$next.push({rt});"));
            self.label(f);
            self.emit(&format!("t$handler = {outer};"));
            self.handler = outer;
            let floor = std::mem::replace(&mut self.targets_floor, self.targets.len());
            self.in_finally_body += 1;
            for s in &block.body {
                self.stmt(s)?;
                self.release_temps();
            }
            self.in_finally_body -= 1;
            self.targets_floor = floor;
            self.uses_loop = true;
            self.emit("t$goto = t$next.pop(); break;");
            self.reachable = false;
            self.label(rt);
            self.emit(&format!("t$err = {e}; t$goto = t$handler; break;"));
            self.reachable = false;
        }
        self.handler = outer;
        self.label(after);
        self.emit(&format!("t$handler = {outer};"));
        Ok(())
    }

    /// The normal end of a `try` block or `catch` body.
    fn leave_try(&mut self, fin: Option<u32>, after: u32) {
        match fin {
            Some(f) => {
                if self.reachable {
                    self.uses_next = true;
                    self.emit(&format!("t$next.push({after});"));
                    self.goto(f);
                }
            }
            None => self.goto(after),
        }
    }

    // --------------------------------------------------------- expressions

    /// An expression statement's value, discarded.
    fn discard(&mut self, v: &str) {
        let trivial = v.is_empty()
            || v == "t$res"
            || v == "void 0"
            || (v.starts_with("t$t") && v[3..].chars().all(|c| c.is_ascii_digit() || c == '$'));
        if !trivial {
            self.emit(&format!("{v};"));
        }
    }

    /// Nothing between its evaluation here and its use after an `await` can
    /// change its value.
    fn is_stable(&self, e: &Expression<'_>) -> bool {
        match e {
            Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::ThisExpression(_)
            | Expression::Super(_)
            | Expression::FunctionExpression(_)
            | Expression::ArrowFunctionExpression(_) => true,
            Expression::ParenthesizedExpression(p) => self.is_stable(&p.expression),
            Expression::UnaryExpression(u) => {
                u.operator == UnaryOperator::Void && self.is_stable(&u.argument)
            }
            // An rti: `t$R("R")`, `dart_rti._Universe.eval(…, "R", true)`,
            // `T[_eval]("R")`, `dart_rti.instanceType(this)` (types do not
            // change).
            Expression::CallExpression(c) => {
                let callee = self.text(c.callee.span());
                let args_stable = c.arguments.iter().all(|a| match a {
                    Argument::SpreadElement(_) => false,
                    other => self.is_stable(other.to_expression()),
                });
                let rti_callee = matches!(
                    callee,
                    "t$R"
                        | "dart_rti._Universe.eval"
                        | "dart_rti._theUniverse"
                        | "dart_rti.instanceType"
                ) || matches!(&c.callee, Expression::ComputedMemberExpression(m)
                    if matches!(self.text(m.expression.span()), "_eval" | "_bind")
                        && self.is_stable(&m.object));
                rti_callee && args_stable
            }
            Expression::Identifier(id) => {
                let Some(r) = id.reference_id.get() else {
                    return false;
                };
                match self.scoping.get_reference(r).symbol_id() {
                    Some(sym) => !self
                        .scoping
                        .get_resolved_references(sym)
                        .any(|r| r.is_write()),
                    None => {
                        let n = id.name.as_str();
                        !self.mutable_free.contains(n) && !self.free_written.contains(n)
                    }
                }
            }
            _ => false,
        }
    }

    /// A namespace path (`L$lib.C.new`, `core.print`) used as a callee: a
    /// class or a library member, constant for the program.
    fn is_static_path(&self, e: &Expression<'_>) -> bool {
        match e {
            Expression::Identifier(_) => self.is_stable(e),
            Expression::StaticMemberExpression(m) => !m.optional && self.is_static_path(&m.object),
            Expression::ParenthesizedExpression(p) => self.is_static_path(&p.expression),
            _ => false,
        }
    }

    /// Keeps `v` in a temporary unless it is stable.
    fn keep(&mut self, v: String, stable: bool) -> String {
        if stable {
            return v;
        }
        let t = self.temp();
        self.emit(&format!("{t} = {v};"));
        t
    }

    /// Evaluates the expressions in order; the ones before the last `yield`
    /// are kept.
    fn in_order(&mut self, items: &[&Expression<'_>]) -> Result<Vec<String>, Refusal> {
        let last = items.iter().rposition(|e| self.has_yield(e.span()));
        let mut out = Vec::with_capacity(items.len());
        for (i, e) in items.iter().enumerate() {
            let v = self.expr(e)?;
            let v = if last.is_some_and(|l| i < l) {
                let stable = self.is_stable(e);
                self.keep(v, stable)
            } else {
                v
            };
            out.push(v);
        }
        Ok(out)
    }

    /// The value of `e`, with what it needs before it emitted.
    fn expr(&mut self, e: &Expression<'_>) -> Result<String, Refusal> {
        if !self.has_yield(e.span()) {
            return self.render_expr(e);
        }
        Ok(match e {
            Expression::ParenthesizedExpression(p) => self.expr(&p.expression)?,
            Expression::YieldExpression(y) if y.delegate => {
                let Some(body) = y.argument.as_ref().and_then(inline_iife) else {
                    return Err("yield*");
                };
                self.inline(body)?
            }
            Expression::YieldExpression(y) => {
                let v = match &y.argument {
                    Some(a) => self.expr(a)?,
                    None => "void 0".into(),
                };
                let k = self.state();
                self.emit(&format!(
                    "t$goto = {k}; return async._asyncAwait({v}, t$body);"
                ));
                self.reachable = false;
                self.label(k);
                "t$res".into()
            }
            Expression::SequenceExpression(sq) => {
                let n = sq.expressions.len();
                let mut last = String::new();
                for (i, x) in sq.expressions.iter().enumerate() {
                    let v = self.expr(x)?;
                    if i + 1 == n {
                        last = v;
                    } else {
                        self.discard(&v);
                    }
                }
                last
            }
            Expression::BinaryExpression(b) => {
                let v = self.in_order(&[&b.left, &b.right])?;
                format!("{} {} {}", paren(&v[0]), b.operator.as_str(), paren(&v[1]))
            }
            Expression::UnaryExpression(u) => {
                if u.operator == UnaryOperator::Delete {
                    return Err("delete with yield");
                }
                format!(
                    "{} {}",
                    u.operator.as_str(),
                    paren(&self.expr(&u.argument)?)
                )
            }
            Expression::LogicalExpression(l) => {
                let left = self.expr(&l.left)?;
                if !self.has_yield(l.right.span()) {
                    let right = self.render_expr(&l.right)?;
                    return Ok(format!(
                        "{} {} {}",
                        paren(&left),
                        l.operator.as_str(),
                        paren(&right)
                    ));
                }
                let t = self.temp();
                self.emit(&format!("{t} = {left};"));
                let end = self.state();
                let skip = match l.operator {
                    LogicalOperator::And => format!("!{t}"),
                    LogicalOperator::Or => t.clone(),
                    LogicalOperator::Coalesce => format!("{t} != null"),
                };
                self.goto_if(&skip, end);
                let right = self.expr(&l.right)?;
                self.emit(&format!("{t} = {right};"));
                self.label(end);
                t
            }
            Expression::ConditionalExpression(c) => {
                let test = self.expr(&c.test)?;
                if !self.has_yield(c.consequent.span()) && !self.has_yield(c.alternate.span()) {
                    let a = self.render_expr(&c.consequent)?;
                    let b = self.render_expr(&c.alternate)?;
                    return Ok(format!("{} ? {} : {}", paren(&test), paren(&a), paren(&b)));
                }
                let t = self.temp();
                let other = self.state();
                let end = self.state();
                self.goto_if(&format!("!{}", paren(&test)), other);
                let a = self.expr(&c.consequent)?;
                self.emit(&format!("{t} = {a};"));
                self.goto(end);
                self.label(other);
                let b = self.expr(&c.alternate)?;
                self.emit(&format!("{t} = {b};"));
                self.label(end);
                t
            }
            Expression::AssignmentExpression(a) => {
                if a.operator.is_logical() {
                    return Err("logical assignment with yield");
                }
                let op = a.operator.as_str();
                let compound = op != "=";
                match &a.left {
                    AssignmentTarget::AssignmentTargetIdentifier(id) => {
                        let name = self.render(id.span, &|r| r.visit_identifier_reference(id))?;
                        if compound {
                            let old = self.keep(name.clone(), false);
                            let v = self.expr(&a.right)?;
                            let bin = &op[..op.len() - 1];
                            format!("{name} = ({old}) {bin} ({v})")
                        } else {
                            let v = self.expr(&a.right)?;
                            format!("{name} = {v}")
                        }
                    }
                    AssignmentTarget::StaticMemberExpression(m) => {
                        let v = self.in_order(&[&m.object, &a.right])?;
                        let target = format!("{}.{}", paren(&v[0]), m.property.name);
                        self.assign_member(target, compound, op, &v[1], &a.right)?
                    }
                    AssignmentTarget::ComputedMemberExpression(m) => {
                        let v = self.in_order(&[&m.object, &m.expression, &a.right])?;
                        let target = format!("{}[{}]", paren(&v[0]), v[1]);
                        self.assign_member(target, compound, op, &v[2], &a.right)?
                    }
                    _ => return Err("assignment target with yield"),
                }
            }
            Expression::CallExpression(c) => {
                if c.optional {
                    return Err("optional call with yield");
                }
                let mut callee = &c.callee;
                while let Expression::ParenthesizedExpression(p) = callee {
                    callee = &p.expression;
                }
                let args: Vec<&Expression<'_>> = c
                    .arguments
                    .iter()
                    .map(|a| match a {
                        Argument::SpreadElement(s) => &s.argument,
                        other => other.to_expression(),
                    })
                    .collect();
                let spread: Vec<bool> = c
                    .arguments
                    .iter()
                    .map(|a| matches!(a, Argument::SpreadElement(_)))
                    .collect();
                let (head, values) = match callee {
                    Expression::StaticMemberExpression(m) if !m.optional => {
                        let mut items = vec![&m.object];
                        items.extend(args.iter().copied());
                        let mut v = self.in_order(&items)?;
                        let o = v.remove(0);
                        (format!("{}.{}", paren(&o), m.property.name), v)
                    }
                    Expression::ComputedMemberExpression(m) if !m.optional => {
                        let mut items = vec![&m.object, &m.expression];
                        items.extend(args.iter().copied());
                        let mut v = self.in_order(&items)?;
                        let o = v.remove(0);
                        let k = v.remove(0);
                        (format!("{}[{k}]", paren(&o)), v)
                    }
                    Expression::StaticMemberExpression(_)
                    | Expression::ComputedMemberExpression(_)
                    | Expression::PrivateFieldExpression(_) => {
                        return Err("callee with yield");
                    }
                    _ => {
                        let mut items = vec![callee];
                        items.extend(args.iter().copied());
                        let mut v = self.in_order(&items)?;
                        let f = v.remove(0);
                        (paren(&f), v)
                    }
                };
                format!("{head}({})", join_args(&values, &spread))
            }
            Expression::NewExpression(n) => {
                let args: Vec<&Expression<'_>> = n
                    .arguments
                    .iter()
                    .map(|a| match a {
                        Argument::SpreadElement(s) => &s.argument,
                        other => other.to_expression(),
                    })
                    .collect();
                let spread: Vec<bool> = n
                    .arguments
                    .iter()
                    .map(|a| matches!(a, Argument::SpreadElement(_)))
                    .collect();
                if !self.has_yield(n.callee.span()) && self.is_static_path(&n.callee) {
                    let head = self.render_expr(&n.callee)?;
                    let v = self.in_order(&args)?;
                    format!("new {head}({})", join_args(&v, &spread))
                } else {
                    let mut items = vec![&n.callee];
                    items.extend(args.iter().copied());
                    let mut v = self.in_order(&items)?;
                    let f = v.remove(0);
                    format!("new {}({})", paren(&f), join_args(&v, &spread))
                }
            }
            Expression::StaticMemberExpression(m) if !m.optional => {
                format!("{}.{}", paren(&self.expr(&m.object)?), m.property.name)
            }
            Expression::ComputedMemberExpression(m) if !m.optional => {
                let v = self.in_order(&[&m.object, &m.expression])?;
                format!("{}[{}]", paren(&v[0]), v[1])
            }
            Expression::ArrayExpression(a) => {
                let mut items = Vec::new();
                let mut kinds = Vec::new();
                for el in &a.elements {
                    match el {
                        ArrayExpressionElement::SpreadElement(s) => {
                            items.push(&s.argument);
                            kinds.push(1u8);
                        }
                        ArrayExpressionElement::Elision(_) => kinds.push(2),
                        other => {
                            items.push(other.to_expression());
                            kinds.push(0);
                        }
                    }
                }
                let mut v = self.in_order(&items)?.into_iter();
                let parts: Vec<String> = kinds
                    .iter()
                    .map(|k| match k {
                        0 => v.next().unwrap_or_default(),
                        1 => format!("...{}", v.next().unwrap_or_default()),
                        _ => String::new(),
                    })
                    .collect();
                format!("[{}]", parts.join(", "))
            }
            Expression::ObjectExpression(o) => {
                // Keys are rendered as written (a computed key with a
                // `yield` refuses); values in order.
                let mut items = Vec::new();
                for p in &o.properties {
                    match p {
                        ObjectPropertyKind::ObjectProperty(p) => {
                            if p.computed && self.has_yield(p.key.span()) {
                                return Err("computed key with yield");
                            }
                            if p.computed && !matches!(p.key, PropertyKey::StringLiteral(_)) {
                                return Err("computed key");
                            }
                            if p.method || p.kind != oxc_ast::ast::PropertyKind::Init {
                                if self.has_yield(p.span) {
                                    return Err("accessor with yield");
                                }
                                continue;
                            }
                            items.push(&p.value);
                        }
                        ObjectPropertyKind::SpreadProperty(s) => items.push(&s.argument),
                    }
                }
                let mut v = self.in_order(&items)?.into_iter();
                let mut parts = Vec::new();
                for p in &o.properties {
                    match p {
                        ObjectPropertyKind::ObjectProperty(p) => {
                            if p.method || p.kind != oxc_ast::ast::PropertyKind::Init {
                                parts.push(self.render(p.span, &|r| r.visit_object_property(p))?);
                                continue;
                            }
                            let key = self.text(p.key.span()).to_string();
                            let key = if p.computed { format!("[{key}]") } else { key };
                            parts.push(format!("{key}: {}", v.next().unwrap_or_default()));
                        }
                        ObjectPropertyKind::SpreadProperty(_) => {
                            parts.push(format!("...{}", v.next().unwrap_or_default()));
                        }
                    }
                }
                format!("({{{}}})", parts.join(", "))
            }
            _ => return Err("expression with yield"),
        })
    }

    /// The statements of an inlined IIFE; its value is the temporary its
    /// `return`s assign.
    fn inline(&mut self, body: &oxc_ast::ast::FunctionBody<'_>) -> Result<String, Refusal> {
        let result = self.temp();
        let end = self.state();
        let outer_taken = std::mem::take(&mut self.temps_taken);
        let floor = std::mem::replace(&mut self.targets_floor, self.targets.len());
        let labels = std::mem::take(&mut self.pending_labels);
        let handler = self.handler;
        self.inline_returns.push(InlineReturn {
            result: result.clone(),
            end,
            fdepth: self.finallies.len(),
            finally_level: self.in_finally_body,
        });
        for s in &body.statements {
            self.stmt(s)?;
            self.release_temps();
        }
        self.inline_returns.pop();
        if self.reachable {
            self.emit(&format!("{result} = void 0;"));
        }
        self.goto(end);
        self.label(end);
        self.handler = handler;
        self.reset_handler(body.span);
        self.pending_labels = labels;
        self.targets_floor = floor;
        self.temps_taken = outer_taken;
        Ok(result)
    }

    fn assign_member(
        &mut self,
        target: String,
        compound: bool,
        op: &str,
        value: &str,
        right: &Expression<'_>,
    ) -> Result<String, Refusal> {
        if !compound {
            return Ok(format!("{target} = {value}"));
        }
        // `o.p op= v`: the old value is read before `v` is evaluated.
        if self.has_yield(right.span()) {
            return Err("compound member assignment with yield");
        }
        Ok(format!("{target} {op} {value}"))
    }
}

/// `v` as an operand: in parentheses unless it is a name or a member path.
fn paren(v: &str) -> String {
    if v.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '.')
    {
        v.to_string()
    } else {
        format!("({v})")
    }
}

/// Plans the IIFEs inlined in an expression.
struct PlanIife<'l, 's> {
    lw: &'l mut Lower<'s>,
    loops: u32,
    depth: u32,
    has_try: bool,
    refusal: Option<Refusal>,
}

impl<'a> Visit<'a> for PlanIife<'_, '_> {
    fn visit_function(&mut self, it: &oxc_ast::ast::Function<'a>, flags: ScopeFlags) {
        self.depth += 1;
        walk::walk_function(self, it, flags);
        self.depth -= 1;
    }
    fn visit_arrow_function_expression(&mut self, it: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        self.depth += 1;
        walk::walk_arrow_function_expression(self, it);
        self.depth -= 1;
    }
    fn visit_class(&mut self, it: &oxc_ast::ast::Class<'a>) {
        self.depth += 1;
        walk::walk_class(self, it);
        self.depth -= 1;
    }
    fn visit_yield_expression(&mut self, it: &oxc_ast::ast::YieldExpression<'a>) {
        if self.depth == 0
            && it.delegate
            && let Some(body) = it.argument.as_ref().and_then(inline_iife)
        {
            match self.lw.plan(&body.statements, self.loops) {
                Ok(t) => self.has_try |= t,
                Err(e) => self.refusal = Some(e),
            }
            return;
        }
        walk::walk_yield_expression(self, it);
    }
}

fn join_args(values: &[String], spread: &[bool]) -> String {
    values
        .iter()
        .zip(spread)
        .map(|(v, s)| if *s { format!("...{v}") } else { v.clone() })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Copies code without `yield`: the edits rename the hoisted bindings and
/// rewrite what leaves the copied part (`return`, `break`, `continue`).
struct Render<'r, 's> {
    lw: &'r mut Lower<'s>,
    edits: Vec<(u32, u32, String)>,
    /// Nested functions and classes.
    depth: u32,
    loops: u32,
    switches: u32,
    labels: Vec<String>,
    /// JavaScript `try … finally` of the copied code around this point.
    local_finally: u32,
    refusal: Option<Refusal>,
}

impl Render<'_, '_> {
    fn renamed(&self, it: &oxc_ast::ast::IdentifierReference<'_>) -> Option<String> {
        let r = it.reference_id.get()?;
        let sym = self.lw.scoping.get_reference(r).symbol_id()?;
        self.lw.renames.get(&sym).cloned()
    }

    fn sub_expr(&mut self, e: &Expression<'_>) -> String {
        let mut r = Render {
            lw: &mut *self.lw,
            edits: Vec::new(),
            depth: self.depth,
            loops: self.loops,
            switches: self.switches,
            labels: self.labels.clone(),
            local_finally: self.local_finally,
            refusal: None,
        };
        r.visit_expression(e);
        if let Some(x) = r.refusal {
            self.refusal = Some(x);
        }
        let mut edits = r.edits;
        edits.sort_by_key(|e| e.0);
        let span = e.span();
        let mut out = String::new();
        let mut at = span.start;
        for (s, en, t) in edits {
            out.push_str(&self.lw.src[at as usize..s as usize]);
            out.push_str(&t);
            at = en;
        }
        out.push_str(&self.lw.src[at as usize..span.end as usize]);
        out
    }

    fn jump(&mut self, span: Span, label: Option<&str>, is_continue: bool) {
        match self.lw.resolve(label, is_continue) {
            Ok((state, fdepth)) => {
                if self.local_finally > 0 && fdepth < self.lw.finallies.len() {
                    self.refusal = Some("jump through two finallies");
                    return;
                }
                let verbatim = self.loops + self.switches > 0;
                let t = self.lw.jump_text(state, fdepth, verbatim);
                self.edits
                    .push((span.start, span.end, format!("{{ {t} }}")));
            }
            Err(e) => self.refusal = Some(e),
        }
    }
}

impl<'a> Visit<'a> for Render<'_, '_> {
    fn visit_function(&mut self, it: &oxc_ast::ast::Function<'a>, flags: ScopeFlags) {
        self.depth += 1;
        walk::walk_function(self, it, flags);
        self.depth -= 1;
    }
    fn visit_arrow_function_expression(&mut self, it: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        self.depth += 1;
        walk::walk_arrow_function_expression(self, it);
        self.depth -= 1;
    }
    fn visit_class(&mut self, it: &oxc_ast::ast::Class<'a>) {
        self.depth += 1;
        walk::walk_class(self, it);
        self.depth -= 1;
    }
    fn visit_identifier_reference(&mut self, it: &oxc_ast::ast::IdentifierReference<'a>) {
        if let Some(n) = self.renamed(it) {
            self.edits.push((it.span.start, it.span.end, n));
        }
    }
    fn visit_object_property(&mut self, it: &oxc_ast::ast::ObjectProperty<'a>) {
        if it.shorthand
            && let Expression::Identifier(id) = &it.value
            && let Some(n) = self.renamed(id)
        {
            self.edits
                .push((it.span.start, it.span.end, format!("{}: {n}", id.name)));
            return;
        }
        walk::walk_object_property(self, it);
    }
    fn visit_assignment_target_property_identifier(
        &mut self,
        it: &oxc_ast::ast::AssignmentTargetPropertyIdentifier<'a>,
    ) {
        if self.renamed(&it.binding).is_some() {
            self.refusal = Some("shorthand assignment pattern");
            return;
        }
        walk::walk_assignment_target_property_identifier(self, it);
    }
    fn visit_return_statement(&mut self, it: &oxc_ast::ast::ReturnStatement<'a>) {
        if self.depth > 0 {
            walk::walk_return_statement(self, it);
            return;
        }
        let value = match &it.argument {
            Some(a) => self.sub_expr(a),
            None => "void 0".into(),
        };
        // A copied `finally` runs before the completion, as in the generator.
        let verbatim = self.loops + self.switches > 0;
        match self
            .lw
            .return_text(&value, verbatim, self.local_finally > 0)
        {
            Ok(t) if t.starts_with("return ") => self.edits.push((it.span.start, it.span.end, t)),
            Ok(t) => self
                .edits
                .push((it.span.start, it.span.end, format!("{{ {t} }}"))),
            Err(e) => self.refusal = Some(e),
        }
    }
    fn visit_break_statement(&mut self, it: &oxc_ast::ast::BreakStatement<'a>) {
        if self.depth > 0 {
            return;
        }
        let local = match &it.label {
            Some(l) => self.labels.iter().any(|x| x == l.name.as_str()),
            None => self.loops + self.switches > 0,
        };
        if !local {
            let label = it.label.as_ref().map(|l| l.name.to_string());
            self.jump(it.span, label.as_deref(), false);
        }
    }
    fn visit_continue_statement(&mut self, it: &oxc_ast::ast::ContinueStatement<'a>) {
        if self.depth > 0 {
            return;
        }
        let local = match &it.label {
            Some(l) => self.labels.iter().any(|x| x == l.name.as_str()),
            None => self.loops > 0,
        };
        if !local {
            let label = it.label.as_ref().map(|l| l.name.to_string());
            self.jump(it.span, label.as_deref(), true);
        }
    }
    fn visit_labeled_statement(&mut self, it: &oxc_ast::ast::LabeledStatement<'a>) {
        self.labels.push(it.label.name.to_string());
        walk::walk_labeled_statement(self, it);
        self.labels.pop();
    }
    fn visit_while_statement(&mut self, it: &oxc_ast::ast::WhileStatement<'a>) {
        self.loops += 1;
        walk::walk_while_statement(self, it);
        self.loops -= 1;
    }
    fn visit_do_while_statement(&mut self, it: &oxc_ast::ast::DoWhileStatement<'a>) {
        self.loops += 1;
        walk::walk_do_while_statement(self, it);
        self.loops -= 1;
    }
    fn visit_for_statement(&mut self, it: &oxc_ast::ast::ForStatement<'a>) {
        self.loops += 1;
        walk::walk_for_statement(self, it);
        self.loops -= 1;
    }
    fn visit_for_in_statement(&mut self, it: &oxc_ast::ast::ForInStatement<'a>) {
        self.loops += 1;
        walk::walk_for_in_statement(self, it);
        self.loops -= 1;
    }
    fn visit_for_of_statement(&mut self, it: &oxc_ast::ast::ForOfStatement<'a>) {
        self.loops += 1;
        walk::walk_for_of_statement(self, it);
        self.loops -= 1;
    }
    fn visit_switch_statement(&mut self, it: &oxc_ast::ast::SwitchStatement<'a>) {
        self.switches += 1;
        walk::walk_switch_statement(self, it);
        self.switches -= 1;
    }
    fn visit_try_statement(&mut self, it: &oxc_ast::ast::TryStatement<'a>) {
        if it.finalizer.is_none() || self.depth > 0 {
            walk::walk_try_statement(self, it);
            return;
        }
        self.local_finally += 1;
        self.visit_block_statement(&it.block);
        if let Some(h) = &it.handler {
            self.visit_catch_clause(h);
        }
        self.local_finally -= 1;
        if let Some(f) = &it.finalizer {
            self.visit_block_statement(f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower(body: &str) -> Result<String, Refusal> {
        lower_async(body, "|n", "function f(i) {", &HashSet::new())
    }

    #[test]
    fn straight_awaits_need_no_loop() {
        let out = lower(
            "let a = (yield L$m.leaf(i));\nlet b = (yield L$m.leaf(i + 1));\nreturn a + b;\n",
        )
        .unwrap();
        assert!(out.contains("switch (t$goto) {\ncase 0:\n"), "{out}");
        assert!(!out.contains("while (true)"), "{out}");
        assert!(
            out.contains("t$goto = 1; return async._asyncAwait(L$m.leaf(i), t$body);"),
            "{out}"
        );
        assert!(out.contains("a = t$res;"), "{out}");
        assert!(
            out.contains("return async._asyncReturn(a + b, t$completer);"),
            "{out}"
        );
        assert!(out.contains(", t$goto = 0, a, b;"), "{out}");
    }

    #[test]
    fn values_read_before_an_await_are_kept() {
        let out = lower("let s = 0;\nfor (let k = 0; k < 3; (k = k + 1)) {\n  (s = s + (yield f(k)));\n}\nreturn s;\n")
            .unwrap();
        assert!(out.contains("t$t$1 = s;"), "{out}");
        assert!(out.contains("s = t$t$1 + t$res;"), "{out}");
        assert!(out.contains("while (true)"), "{out}");
    }

    #[test]
    fn try_finally_routes_returns_and_errors() {
        let out = lower("try {\n  (yield f());\n  return 1;\n} finally {\n  g();\n}\n").unwrap();
        assert!(out.contains("t$handler"), "{out}");
        assert!(out.contains("t$next.push("), "{out}");
        assert!(out.contains("t$ret = 1;"), "{out}");
        assert!(out.contains("t$goto = t$next.pop(); break;"), "{out}");
    }

    #[test]
    fn inlines_the_emitter_iife() {
        let out = lower(
            "let t$0;\nf((yield* (function*() {\nt$0 = [];\nfor (let x of l) {\n  t$0.push((yield x));\n}\nreturn t$0;\n}).call(this)));\n",
        )
        .unwrap();
        assert!(!out.contains("function*"), "{out}");
        assert!(out.contains("t$t$2.push(t$res);"), "{out}");
        assert!(out.contains("f(t$t$1);"), "{out}");
    }

    #[test]
    fn refuses_what_it_does_not_handle() {
        assert_eq!(lower("yield* g();\n"), Err("yield*"));
        assert_eq!(
            lower("for (let x of l) {\n  (yield 0);\n  fs.push(() => x);\n}\n"),
            Err("captured loop binding")
        );
        assert_eq!(lower("let {a} = (yield 0);\n"), Err("pattern declaration"));
    }

    #[test]
    fn renames_a_hoisted_name_that_is_taken() {
        let out = lower("{\n  let i = (yield 1);\n  f(i);\n}\n").unwrap();
        // `i` is the enclosing function's parameter.
        assert!(out.contains("i$1 = t$res;"), "{out}");
        assert!(out.contains("f(i$1);"), "{out}");
    }
}
