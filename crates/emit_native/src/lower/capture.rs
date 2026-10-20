//! Capture analysis (P1): which local variables a closure or local
//! function sees from outside, and which of them live in a cell.
//!
//! The specification (§17.11 "Function Expressions", §10 "Variables") says that the
//! closure captures the **variable**, not the value: a write made after the
//! creation is seen by it, and a write of its own is seen from outside. `dartdevc`
//! and `dart2js` lower this into two classes, as here:
//!
//! * captured and **never assigned** after the declaration (the common case:
//!   a parameter, a `final`, a `var` only initialized): the value is copied into the
//!   environment when the closure is created — there is nothing to observe;
//! * captured and assigned at any point (outside or inside some
//!   closure, before or after the capture): the variable lives in a heap `Cell`
//!   from its declaration on, and both the function and the closures go through it.
//!   A local function counts as assigned (the name is bound after the
//!   closure exists — that is how it calls itself).
//!
//! The analysis is per **lowered function** (the top-level one and each closure, when
//! it is lowered): it looks at the whole body, with the nested closures, and decides the
//! cells of the variables declared in **that** function. The free variables of
//! a closure are computed at its creation, against the visible locals.
//!
//! A reference is local when the resolution says `Local`/`Parameter` or says
//! nothing (closure bodies are not inferred yet: everything in them is
//! `dynamic` and without a resolution); an identifier that resolves to a member or
//! a top-level element is never a capture. The scopes follow those of the specification:
//! block, `for` (the loop variable), `catch`, `case`, parameters.

use crate::context::Context;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{
    self, CollectionElement, ExprId, ExprKind, ForInTarget, ForInit, FunctionBody, FunctionId,
    ListPatternElement, PatternId, PatternKind, StmtId, StmtKind, UnaryOp,
};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::Resolved;
use std::collections::{HashMap, HashSet};

/// A declaration seen by the walk: the offset of the name and the function
/// depth (0 = the analyzed function).
#[derive(Clone, Copy)]
struct Decl {
    offset: usize,
    prof: u32,
}

/// A single walk over the body of a function, with the scope stack.
struct Walk<'x, 'a> {
    ctx: &'x Context<'a>,
    unit: UnitId,
    ast: &'x ast::Ast,
    scopes: Vec<HashMap<SymbolId, Decl>>,
    prof: u32,
    /// Declarations of depth 0 read or written from inside a
    /// nested function.
    captured: HashSet<usize>,
    /// Declarations of depth 0 written at some point (or local
    /// functions).
    assigned: HashSet<usize>,
    /// Names read or written without a declaration in the walk (the free
    /// variables, in order of first reference).
    free_set: Vec<SymbolId>,
    /// Names without a declaration in the walk that it writes.
    stored_free: HashSet<SymbolId>,
    uses_this: bool,
    // --- direct local functions (`direct_functions.rs`) ---
    /// The open functions, from the outermost to the current one: the named local
    /// function (`Some`) or the function expression (`None`). The length is `prof`.
    stack: Vec<Option<FunctionId>>,
    /// Name (offset) of each local function declared in the walk → the function.
    local_functions: HashMap<usize, FunctionId>,
    /// Each reference to a local function: the function, whether it is the target of an
    /// `f(…)` call, and the functions crossed between the declaration and the
    /// reference.
    refs: Vec<(FunctionId, bool, Vec<Option<FunctionId>>)>,
    /// Local functions that cannot be direct because of their shape (generic,
    /// `async`/generator, optional or named parameter, reads an outer `late`).
    discarded_set: HashSet<FunctionId>,
    /// Offsets of the `late` declarations.
    lates: HashSet<usize>,
    /// Names from outside the root that are `late`.
    outside_lates: HashSet<SymbolId>,
    /// The identifier that is the target of the call being walked.
    call_target: Option<ExprId>,
    /// The direct local functions (second pass): calling them captures
    /// nothing, their name is not a variable, and what they read from outside goes by
    /// parameter — the variable only needs a cell if some escaping
    /// function also captures it.
    directs: Option<HashSet<FunctionId>>,
}

impl<'x, 'a> Walk<'x, 'a> {
    fn new(ctx: &'x Context<'a>, unit: UnitId, ast: &'x ast::Ast) -> Self {
        Self {
            ctx,
            unit,
            ast,
            scopes: vec![HashMap::new()],
            prof: 0,
            captured: HashSet::new(),
            assigned: HashSet::new(),
            free_set: Vec::new(),
            stored_free: HashSet::new(),
            uses_this: false,
            stack: Vec::new(),
            local_functions: HashMap::new(),
            refs: Vec::new(),
            discarded_set: HashSet::new(),
            lates: HashSet::new(),
            outside_lates: HashSet::new(),
            call_target: None,
            directs: None,
        }
    }

    fn open(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn close(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: ast::Name) {
        let d = Decl {
            offset: name.span.start,
            prof: self.prof,
        };
        self.scopes.last_mut().expect("scope").insert(name.sym, d);
    }

    fn lookup(&self, sym: SymbolId) -> Option<Decl> {
        self.scopes.iter().rev().find_map(|e| e.get(&sym).copied())
    }

    /// A reference to a name (read or write) in expression `e`.
    fn reference(&mut self, e: ExprId, sym: SymbolId, writes: bool) {
        match self.ctx.get_resolved(self.unit, e) {
            None | Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) => {}
            Some(_) => return,
        }
        match self.lookup(sym) {
            Some(d) => {
                let crossed = self.stack[(d.prof as usize).min(self.stack.len())..].to_vec();
                if let Some(&f) = self.local_functions.get(&d.offset) {
                    let target = !writes && self.call_target == Some(e);
                    self.refs.push((f, target, crossed.clone()));
                    // The name of a direct function is not a variable.
                    if self.directs.as_ref().is_some_and(|s| s.contains(&f)) {
                        return;
                    }
                }
                if self.lates.contains(&d.offset) {
                    self.discarded_set.extend(crossed.iter().flatten().copied());
                }
                self.use_declaration(d, writes, &crossed);
            }
            None => {
                if self.outside_lates.contains(&sym) {
                    self.discarded_set
                        .extend(self.stack.iter().flatten().copied());
                }
                if !self.free_set.contains(&sym) {
                    self.free_set.push(sym);
                }
                if writes {
                    self.stored_free.insert(sym);
                }
            }
        }
    }

    /// A read or write of declaration `d`, seen from inside the functions
    /// `crossed`: from depth 0, it is a capture (through the environment) if
    /// some of them escapes, and a direct capture (by parameter) if all are
    /// direct local functions.
    fn use_declaration(&mut self, d: Decl, writes: bool, crossed: &[Option<FunctionId>]) {
        if d.prof != 0 {
            return;
        }
        if self.prof > 0 {
            let only_directs = self
                .directs
                .as_ref()
                .is_some_and(|s| crossed.iter().all(|f| f.is_some_and(|f| s.contains(&f))));
            // Only by direct functions: the variable stays in the frame of its
            // declarer and goes by address (`direct_functions.rs`).
            if !only_directs {
                self.captured.insert(d.offset);
            }
        }
        if writes {
            self.assigned.insert(d.offset);
        }
    }

    /// A nested function (closure or local): parameters and body one
    /// depth further. `local`: it is the declaration of a local function.
    fn function(&mut self, fid: FunctionId) {
        self.function_of(fid, false);
    }

    fn function_of(&mut self, fid: FunctionId, local: bool) {
        let f = self.ast.function(fid);
        self.stack.push(local.then_some(fid));
        self.prof += 1;
        self.open();
        if let Some(params) = &f.parameters {
            self.parameters(params);
        }
        self.body(&f.body);
        self.close();
        self.prof -= 1;
        self.stack.pop();
    }

    fn parameters(&mut self, params: &[ast::Parameter]) {
        for p in params {
            if let Some(d) = p.default_value {
                self.expr(d);
            }
            if let Some(n) = p.name {
                self.declare(n);
            }
        }
    }

    fn body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(s) => self.stmt(*s),
            FunctionBody::Expression(e) => self.expr(*e),
            FunctionBody::Empty | FunctionBody::Native(_) => {}
        }
    }

    fn variable_list(&mut self, list: &ast::VariableList) {
        for v in list.variables.iter() {
            if let Some(i) = v.initializer {
                self.expr(i);
            }
            self.declare(v.name);
            if list.late {
                self.lates.insert(v.name.span.start);
            }
            // Even without an explicit write, the initialization mark of a
            // `late` without an initializer must be shared with the closure.
            if list.late && v.initializer.is_none() {
                self.assigned.insert(v.name.span.start);
            }
        }
    }

    fn stmt(&mut self, s: StmtId) {
        let ast = self.ast;
        match &ast.stmt(s).kind {
            StmtKind::Block(ss) => {
                self.open();
                for &x in ss.iter() {
                    self.stmt(x);
                }
                self.close();
            }
            StmtKind::Variables(list) => self.variable_list(list),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.expr(*value);
                self.pattern(*pattern, true);
            }
            StmtKind::Function(fid) => {
                // The name exists before the body (recursion) and is bound after
                // the closure exists: it counts as assigned — except for the
                // direct function, whose name is not a variable.
                let f = ast.function(*fid);
                if let Some(n) = f.name {
                    self.declare(n);
                    self.local_functions.insert(n.span.start, *fid);
                    let direct = self.directs.as_ref().is_some_and(|s| s.contains(fid));
                    if self.prof == 0 && !direct {
                        self.assigned.insert(n.span.start);
                    }
                }
                let params = f.parameters.as_deref().unwrap_or(&[]);
                if !f.type_params.is_empty()
                    || f.modifier != ast::AsyncModifier::None
                    || params.iter().any(|p| {
                        p.kind != ast::ParameterKind::Required || p.default_value.is_some()
                    })
                    || !matches!(f.body, FunctionBody::Block(_) | FunctionBody::Expression(_))
                {
                    self.discarded_set.insert(*fid);
                }
                self.function_of(*fid, true);
            }
            StmtKind::Expression(e) => self.expr(*e),
            StmtKind::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                self.expr(*condition);
                self.open();
                if let Some(p) = case_pattern {
                    self.pattern(*p, true);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.stmt(*then);
                self.close();
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
                self.open();
                match init {
                    Some(ForInit::Variables(l)) => self.variable_list(l),
                    Some(ForInit::Expression(e)) => self.expr(*e),
                    Some(ForInit::Pattern { pattern, value, .. }) => {
                        self.expr(*value);
                        self.pattern(*pattern, true);
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for &u in updates.iter() {
                    self.expr(u);
                }
                self.stmt(*body);
                self.close();
            }
            StmtKind::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                self.expr(*iterable);
                self.open();
                self.for_in_target(target);
                self.stmt(*body);
                self.close();
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
                for c in cases.iter() {
                    self.open();
                    if let Some(p) = c.pattern {
                        self.pattern(p, true);
                    }
                    if let Some(g) = c.guard {
                        self.expr(g);
                    }
                    for &x in c.body.iter() {
                        self.stmt(x);
                    }
                    self.close();
                }
            }
            StmtKind::Return(e) => {
                if let Some(e) = e {
                    self.expr(*e);
                }
            }
            StmtKind::Yield { value, .. } => self.expr(*value),
            StmtKind::Try {
                body,
                catches,
                finally_,
            } => {
                self.stmt(*body);
                for c in catches.iter() {
                    self.open();
                    if let Some(n) = c.exception {
                        self.declare(n);
                    }
                    if let Some(n) = c.stack_trace {
                        self.declare(n);
                    }
                    self.stmt(c.body);
                    self.close();
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

    fn for_in_target(&mut self, target: &ForInTarget) {
        match target {
            ForInTarget::Declared { name, .. } => self.declare(*name),
            ForInTarget::Pattern { pattern, .. } => self.pattern(*pattern, true),
            ForInTarget::Expression(e) => self.assignment_target(*e),
        }
    }

    /// The target of an assignment: an identifier is a write; the rest,
    /// reads of the subexpressions.
    fn assignment_target(&mut self, e: ExprId) {
        if let ExprKind::Identifier(n) = &self.ast.expr(e).kind {
            self.reference(e, n.sym, true);
        } else {
            self.expr(e);
        }
    }

    /// A pattern: `declares` for declaration ones (`var (a, b) = …`, `case`),
    /// otherwise the names are writes of existing variables (`(a, b) = …`).
    fn pattern(&mut self, p: PatternId, declares: bool) {
        let ast = self.ast;
        match &ast.pattern(p).kind {
            PatternKind::Wildcard { .. } => {}
            PatternKind::Variable { name, .. } => {
                if declares {
                    self.declare(*name);
                } else {
                    match self.lookup(name.sym) {
                        Some(d) => {
                            let crossed =
                                self.stack[(d.prof as usize).min(self.stack.len())..].to_vec();
                            if self.lates.contains(&d.offset) {
                                self.discarded_set.extend(crossed.iter().flatten().copied());
                            }
                            self.use_declaration(d, true, &crossed);
                        }
                        None => {
                            if !self.free_set.contains(&name.sym) {
                                self.free_set.push(name.sym);
                            }
                        }
                    }
                }
            }
            PatternKind::Constant(e) => self.expr(*e),
            PatternKind::Relational { value, .. } => self.expr(*value),
            PatternKind::Or(a, b) | PatternKind::And(a, b) => {
                self.pattern(*a, declares);
                self.pattern(*b, declares);
            }
            PatternKind::NullCheck(x)
            | PatternKind::NullAssert(x)
            | PatternKind::Parenthesized(x)
            | PatternKind::Cast { pattern: x, .. } => self.pattern(*x, declares),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) => {
                            self.pattern(*x, declares)
                        }
                        ListPatternElement::Rest(None) => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for en in entries.iter() {
                    self.expr(en.key);
                    self.pattern(en.value, declares);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.pattern(f.pattern, declares);
                }
            }
        }
    }

    fn element(&mut self, el: &CollectionElement) {
        match el {
            CollectionElement::Expression(e) | CollectionElement::NullAwareExpression(e) => {
                self.expr(*e)
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
                self.open();
                if let Some(p) = case_pattern {
                    self.pattern(*p, true);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.element(then);
                self.close();
                if let Some(e) = else_ {
                    self.element(e);
                }
            }
            CollectionElement::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                self.open();
                match init {
                    Some(ForInit::Variables(l)) => self.variable_list(l),
                    Some(ForInit::Expression(e)) => self.expr(*e),
                    Some(ForInit::Pattern { pattern, value, .. }) => {
                        self.expr(*value);
                        self.pattern(*pattern, true);
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for &u in updates.iter() {
                    self.expr(u);
                }
                self.element(body);
                self.close();
            }
            CollectionElement::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                self.expr(*iterable);
                self.open();
                self.for_in_target(target);
                self.element(body);
                self.close();
            }
        }
    }

    fn arguments(&mut self, a: &ast::Arguments) {
        for x in a.args.iter() {
            self.expr(x.value);
        }
    }

    fn expr(&mut self, e: ExprId) {
        let ast = self.ast;
        match &ast.expr(e).kind {
            ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::Symbol(_)
            | ExprKind::CascadeTarget
            // `.id` (3.10) names a static member of the declaration of the
            // context: it does not read a local.
            | ExprKind::DotShorthand { .. }
            | ExprKind::Rethrow => {}
            ExprKind::String(s) => {
                for p in s.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(*x);
                    }
                }
            }
            ExprKind::Identifier(n) => self.reference(e, n.sym, false),
            ExprKind::This | ExprKind::Super => self.uses_this = true,
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    self.element(el);
                }
            }
            ExprKind::Record {
                positional, named, ..
            } => {
                for &x in positional.iter() {
                    self.expr(x);
                }
                for (_, x) in named.iter() {
                    self.expr(*x);
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => self.arguments(arguments),
            ExprKind::FunctionExpression(fid) => self.function(*fid),
            ExprKind::Property { target, .. } => self.expr(*target),
            ExprKind::Index { target, index, .. } => {
                self.expr(*target);
                self.expr(*index);
            }
            ExprKind::Call { target, arguments } => {
                let saved = self.call_target.replace(*target);
                self.expr(*target);
                self.call_target = saved;
                self.arguments(arguments);
            }
            ExprKind::TypeArguments { target, .. } => self.expr(*target),
            ExprKind::Unary { op, operand } => match op {
                UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec => {
                    self.assignment_target(*operand)
                }
                _ => self.expr(*operand),
            },
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
                self.assignment_target(*target);
                self.expr(*value);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.expr(*value);
                self.pattern(*pattern, false);
            }
            ExprKind::Cascade {
                target, sections, ..
            } => {
                self.expr(*target);
                for &s in sections.iter() {
                    self.expr(s);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value);
                for c in cases.iter() {
                    self.open();
                    self.pattern(c.pattern, true);
                    if let Some(g) = c.guard {
                        self.expr(g);
                    }
                    self.expr(c.body);
                    self.close();
                }
            }
        }
    }
}

/// What the analysis of a function hands to the lowering.
pub struct Captures {
    /// Offsets of the declarations of this function that live in a cell.
    pub cells: HashSet<usize>,
    /// Offsets of the declarations of this function written at some point.
    pub assigned: HashSet<usize>,
    /// The local functions (at any depth) called only
    /// directly, from inside functions that are also direct
    /// (`direct_functions.rs`).
    pub directs: HashSet<FunctionId>,
}

/// The body of a function to analyze: parameters and the executable parts.
pub struct Root<'x> {
    pub parameters: &'x [ast::Parameter],
    pub body: Option<&'x FunctionBody>,
    /// The initializer list of a constructor.
    pub initializers: &'x [ast::Initializer],
}

/// The cells of the variables declared in function `root` (parameters and
/// locals of the body, outside the nested closures).
pub fn analyze(ctx: &Context, unit: UnitId, ast: &ast::Ast, root: Root) -> Captures {
    analyze_with(ctx, unit, ast, root, false, &HashSet::new())
}

/// [`analyze`]; with `with_direct`, the local functions that do not escape become
/// direct (`direct_functions.rs`): first the references to each local
/// function, and the escape analysis — only a function whose name appears
/// only as the target of an `f(…)` call, from inside the function that declares it
/// or from other direct functions, is direct (an escaping closure could call it
/// after the outer function returns, with the addresses of its
/// locals) —; then the cells with that set.
///
/// `outside_lates`: the names from outside the root (already bound captures) that are
/// `late`; a function that reads them cannot be direct either.
pub fn analyze_with(
    ctx: &Context,
    unit: UnitId,
    ast: &ast::Ast,
    root: Root,
    with_direct: bool,
    outside_lates: &HashSet<SymbolId>,
) -> Captures {
    let directs = if with_direct {
        let mut p = Walk::new(ctx, unit, ast);
        p.outside_lates = outside_lates.clone();
        p.walk(&root);
        let mut directs: HashSet<FunctionId> = p
            .local_functions
            .values()
            .copied()
            .filter(|f| !p.discarded_set.contains(f))
            .collect();
        loop {
            let before = directs.len();
            for (f, target, crossed) in &p.refs {
                if directs.contains(f)
                    && !(*target
                        && crossed
                            .iter()
                            .all(|g| g.is_some_and(|g| directs.contains(&g))))
                {
                    directs.remove(f);
                }
            }
            if directs.len() == before {
                break;
            }
        }
        directs
    } else {
        HashSet::new()
    };
    let mut p = Walk::new(ctx, unit, ast);
    p.directs = Some(directs.clone());
    p.walk(&root);
    let cells: HashSet<usize> = p.captured.intersection(&p.assigned).copied().collect();
    Captures {
        cells,
        assigned: p.assigned,
        directs,
    }
}

impl Walk<'_, '_> {
    /// Walks the whole function `root`: parameters, initializer
    /// list and body.
    fn walk(&mut self, root: &Root) {
        self.parameters(root.parameters);
        for i in root.initializers {
            match i {
                ast::Initializer::Field { value, .. } => self.expr(*value),
                ast::Initializer::Super { arguments, .. }
                | ast::Initializer::Redirect { arguments, .. } => self.arguments(arguments),
                ast::Initializer::Assert {
                    condition, message, ..
                } => {
                    self.expr(*condition);
                    if let Some(m) = message {
                        self.expr(*m);
                    }
                }
            }
        }
        if let Some(c) = root.body {
            self.body(c);
        }
    }
}

/// The free variables of the nested function `fid` (in order of first
/// reference) and whether it uses `this` explicitly.
pub fn free_set(
    ctx: &Context,
    unit: UnitId,
    ast: &ast::Ast,
    fid: FunctionId,
) -> (Vec<SymbolId>, bool) {
    let mut p = Walk::new(ctx, unit, ast);
    let f = ast.function(fid);
    if let Some(params) = &f.parameters {
        p.parameters(params);
    }
    p.body(&f.body);
    (p.free_set, p.uses_this)
}

/// The names declared outside `body` and `exprs` that they write
/// (assignment, `++`/`--`, compound assignment, pattern; also from inside
/// nested closures): what decides whether a `for` loop is counted
/// (`statements.rs`).
pub fn stored_from_outside(
    ctx: &Context,
    unit: UnitId,
    ast: &ast::Ast,
    body: StmtId,
    exprs: &[ExprId],
) -> HashSet<SymbolId> {
    let mut p = Walk::new(ctx, unit, ast);
    p.stmt(body);
    for &e in exprs {
        p.expr(e);
    }
    p.stored_free
}
