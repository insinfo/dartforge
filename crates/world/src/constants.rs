//! Boolean conditions that are constant across the whole program
//! (`docs/JS-PRODUCAO-TAMANHO.md` §3.3).
//!
//! With `assert`s disabled (the production profile's default, as in
//! `dart2js`), ngdart's `isDevMode` —
//! `bool get _assertionsEnabled { var enabled = false; assert(enabled = true); return enabled; }` —
//! is `false`, and with it `isDevToolsEnabled` (`isDevMode && _isDevToolsEnabled`)
//! and `debugThrowIfChanged`. dart2js proves this through inference
//! (`inferrer/engine.dart:850-884`) and deletes the dead branch. Here the proof
//! is syntactic and conservative: literals, `!`, `&&`/`||` (with
//! short-circuiting), `==`/`!=` between literals, top-level `const` and `final`
//! with an evaluable initializer, and top-level or static parameterless
//! *getters* whose body is an evaluable expression or a block made only of
//! local declarations with a literal, `assert` (ignored without asserts) and a
//! final `return`. Anything else is unknown, and the code stays.
//!
//! The world (the dead branch is not traversed) and the emitter (the dead
//! branch is not emitted) use the same answer, so the text verifier agrees.
//!
//! A formal parameter that the world proved always `null` (`params.rs`: every
//! call that reaches it passes `null` or omits it with a `null` default, and
//! the body never writes it) reads as the `null` literal, so `p == null` and
//! `p != null` fold too. The set of those parameters comes from the world
//! ([`Constants::set_null_params`] on the emitter's side).

use crate::WorldInput;
use dartforge_elements::model::{
    Element, FunctionElementId, FunctionKind, FunctionRef, UnitId, VariableId, VariableRef,
};
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, FunctionBody, StmtKind, UnaryOp};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::Resolved;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// A formal parameter of a declared function or constructor: its unit and the
/// offset of its name in the declaration.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct ParamKey {
    pub unit: UnitId,
    pub offset: u32,
    pub name: SymbolId,
}

/// Per unit: the parameters of every function, constructor and closure (by
/// name: the span of the declaring node and the parameter's offset) and the
/// positions where each name is written (`x = …`, `x++`, `for (x in …)`, a
/// variable pattern) or declared as a local (variable, loop variable, `catch`
/// clause, local function).
#[derive(Default)]
struct UnitParams {
    by_name: HashMap<SymbolId, Vec<(usize, usize, u32)>>,
    writes: HashMap<SymbolId, Vec<usize>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Value {
    Bool(bool),
    Null,
}

/// The evaluator, memoized per *getter* and per variable.
pub struct Constants<'a> {
    e: WorldInput<'a>,
    no_asserts: bool,
    memo_fn: RefCell<HashMap<FunctionElementId, Option<Value>>>,
    memo_var: RefCell<HashMap<VariableId, Option<Value>>>,
    /// Parameters proved always `null`.
    null_params: RefCell<HashSet<ParamKey>>,
    /// How many of [`Constants::null_params`] have each name (a cheap
    /// filter before looking the declaration up).
    null_names: RefCell<HashMap<SymbolId, u32>>,
    units: RefCell<HashMap<UnitId, Rc<UnitParams>>>,
    /// No record exists at run time ([`crate::Options::without_records`]).
    without_records: std::cell::Cell<bool>,
    /// No legacy (`T*`) type exists at run time
    /// ([`crate::Options::without_legacy`]).
    without_legacy: std::cell::Cell<bool>,
}

impl<'a> Constants<'a> {
    pub fn new(e: WorldInput<'a>, no_asserts: bool) -> Constants<'a> {
        Constants {
            e,
            no_asserts,
            memo_fn: RefCell::new(HashMap::new()),
            memo_var: RefCell::new(HashMap::new()),
            null_params: RefCell::new(HashSet::new()),
            null_names: RefCell::new(HashMap::new()),
            units: RefCell::new(HashMap::new()),
            without_records: std::cell::Cell::new(false),
            without_legacy: std::cell::Cell::new(false),
        }
    }

    /// No record type or value exists at run time: `k == Rti.kindRecord`
    /// (`k` a local or a `dart:_rti` getter call) is `false`, and so is
    /// `x is Record` (`x` a local).
    pub fn set_without_records(&self, b: bool) {
        self.without_records.set(b);
    }

    /// No legacy type exists at run time (sound null safety and no `*`
    /// recipe): `k == Rti.kindStar` is `false`, its `case`s never match, and
    /// a comparison with `LEGACY_TYPE_REF<T>()` is `false`.
    pub fn set_without_legacy(&self, b: bool) {
        self.without_legacy.set(b);
    }

    /// [`Constants::set_without_legacy`].
    pub fn without_legacy(&self) -> bool {
        self.without_legacy.get()
    }

    /// `JS_GET_FLAG('X')` of `dart:_foreign_helper`, with the values the
    /// emitter writes (`emit_js::own_sdk::SdkMode::flag`: DDC with sound null
    /// safety). `DARTFORGE_JSPROD_SDK_FLAGS=0` leaves them unknown here.
    fn flag(&self, u: UnitId, target: ExprId, arguments: &ast::Arguments) -> Option<Value> {
        let p = self.e.program;
        let callee = match self.resolved(u, target)? {
            Resolved::Element(Element::Function(f)) => *f,
            _ => return None,
        };
        let func = p.function(callee);
        if p.library(func.library).uri != "dart:_foreign_helper"
            || self.e.interner.resolve(func.name) != "JS_GET_FLAG"
            || arguments.args.len() != 1
            || std::env::var("DARTFORGE_JSPROD_SDK_FLAGS").is_ok_and(|v| v == "0")
        {
            return None;
        }
        let ExprKind::String(lit) = &p.unit(u).ast.expr(arguments.args[0].value).kind else {
            return None;
        };
        let name = lit.constant_value()?.to_string_lossy();
        match name.as_str() {
            "DEV_COMPILER" | "SOUND_NULL_SAFETY" | "VARIANCE" => Some(Value::Bool(true)),
            "MINIFIED" | "LEGACY" | "EXTRA_NULL_SAFETY_CHECKS" | "PRINT_LEGACY_STARS" => {
                Some(Value::Bool(false))
            }
            _ => None,
        }
    }

    /// A `case` that can never match: its pattern is a constant that no
    /// value at run time equals (the record kind and recipe characters
    /// without records). `DARTFORGE_JSPROD_DEAD_CASES=0` keeps every case.
    pub fn dead_case(&self, u: UnitId, pattern: ast::PatternId) -> bool {
        if std::env::var("DARTFORGE_JSPROD_DEAD_CASES").is_ok_and(|v| v == "0") {
            return false;
        }
        match &self.e.program.unit(u).ast.pattern(pattern).kind {
            ast::PatternKind::Constant(x) => self.gated_constant(u, *x),
            _ => false,
        }
    }

    /// A constant no value at run time equals: `Rti.kindRecord`,
    /// `Recipe.startRecord` or `_Parser.gotoRecord` without records,
    /// `Rti.kindStar` or `Recipe.wrapStar` without legacy types.
    fn gated_constant(&self, u: UnitId, x: ExprId) -> bool {
        let p = self.e.program;
        let v = match self.resolved(u, x) {
            Some(Resolved::Element(Element::Variable(v))) => v,
            Some(Resolved::Member {
                member: dartforge_types::resolved::MemberRef::Variable(v),
                ..
            }) => v,
            _ => return false,
        };
        let var = p.variable(*v);
        let Some(c) = var.class else {
            return false;
        };
        let lib = p.library(var.library).uri.as_str();
        let class = self.e.interner.resolve(p.class(c).name);
        let name = self.e.interner.resolve(var.name);
        let record = matches!(
            (lib, class, name),
            ("dart:_rti", "Rti", "kindRecord")
                | ("dart:_rti", "_Parser", "gotoRecord")
                | ("dart:_recipe_syntax", "Recipe", "startRecord")
        );
        let legacy = matches!(
            (lib, class, name),
            ("dart:_rti", "Rti", "kindStar") | ("dart:_recipe_syntax", "Recipe", "wrapStar")
        );
        (record && self.without_records.get()) || (legacy && self.without_legacy.get())
    }

    /// `_Utils.isIdentical(x, LEGACY_TYPE_REF<T>())` (either order, `x` pure).
    fn legacy_identity(&self, u: UnitId, target: ExprId, arguments: &ast::Arguments) -> bool {
        let p = self.e.program;
        if !self.without_legacy.get() || arguments.args.len() != 2 {
            return false;
        }
        let is = |x: ExprId, lib: &str, name: &str| {
            let f = match self.resolved(u, x) {
                Some(Resolved::Element(Element::Function(f))) => *f,
                Some(Resolved::Member {
                    member: dartforge_types::resolved::MemberRef::Function(f),
                    ..
                }) => *f,
                _ => return false,
            };
            let func = p.function(f);
            p.library(func.library).uri == lib && self.e.interner.resolve(func.name) == name
        };
        if !is(target, "dart:_rti", "isIdentical") {
            return false;
        }
        let legacy_ref = |x: ExprId| {
            let mut x = x;
            if let ExprKind::TypeArguments { target, .. } = &p.unit(u).ast.expr(x).kind {
                x = *target;
            }
            match &p.unit(u).ast.expr(x).kind {
                ExprKind::Call { target, .. } => {
                    let mut t = *target;
                    if let ExprKind::TypeArguments { target, .. } = &p.unit(u).ast.expr(t).kind {
                        t = *target;
                    }
                    is(t, "dart:_foreign_helper", "LEGACY_TYPE_REF")
                }
                _ => false,
            }
        };
        let (a, b) = (arguments.args[0].value, arguments.args[1].value);
        (legacy_ref(b) && self.pure_rti_read(u, a, 0))
            || (legacy_ref(a) && self.pure_rti_read(u, b, 0))
    }

    /// An expression whose evaluation has no effect: a local, a parameter,
    /// or a call of a `_get…` function of `dart:_rti` (the field accessors)
    /// with such arguments.
    fn pure_rti_read(&self, u: UnitId, x: ExprId, depth: u32) -> bool {
        if depth > 6 {
            return false;
        }
        let p = self.e.program;
        match &p.unit(u).ast.expr(x).kind {
            ExprKind::Parenthesized(i) => self.pure_rti_read(u, *i, depth + 1),
            ExprKind::Identifier(_) => matches!(
                self.resolved(u, x),
                Some(Resolved::Local(_) | Resolved::Parameter { .. })
            ),
            ExprKind::Call { target, arguments } => {
                let callee = match self.resolved(u, x).or_else(|| self.resolved(u, *target)) {
                    Some(Resolved::Element(Element::Function(f))) => *f,
                    _ => return false,
                };
                let func = p.function(callee);
                p.library(func.library).uri == "dart:_rti"
                    && (func.static_ || func.class.is_none())
                    && self.e.interner.resolve(func.name).starts_with("_get")
                    && arguments.type_args.is_empty()
                    && arguments
                        .args
                        .iter()
                        .all(|a| a.name.is_none() && self.pure_rti_read(u, a.value, depth + 1))
            }
            _ => false,
        }
    }

    /// `x is Record` with `Record` of `dart:core`, `x` a local.
    fn is_record_test(&self, u: UnitId, value: ExprId, ty: ast::TypeId) -> bool {
        let p = self.e.program;
        let unit = p.unit(u);
        let ast::TypeKind::Named { name, args } = &unit.ast.ty(ty).kind else {
            return false;
        };
        if name.len() != 1 || !args.is_empty() || unit.ast.ty(ty).nullable {
            return false;
        }
        let is_record = matches!(
            p.lookup(unit.library, name[0].sym).and_then(|b| b.getter),
            Some(Element::Class(c)) if Some(p.class(c).library) == p.core
                && self.e.interner.resolve(p.class(c).name) == "Record"
        );
        is_record && self.pure_rti_read(u, value, 0)
    }

    /// Replaces the parameters proved always `null` (the emitter's side: the
    /// set of the finished world, [`crate::World::null_params`]).
    pub fn set_null_params(&self, keys: &[ParamKey]) {
        let mut set = self.null_params.borrow_mut();
        let mut names = self.null_names.borrow_mut();
        set.clear();
        names.clear();
        for k in keys {
            if set.insert(*k) {
                *names.entry(k.name).or_insert(0) += 1;
            }
        }
    }

    /// The parameters proved always `null`, sorted.
    pub fn null_params(&self) -> Vec<ParamKey> {
        let mut v: Vec<ParamKey> = self.null_params.borrow().iter().copied().collect();
        v.sort();
        v
    }

    pub(crate) fn add_null_param(&self, k: ParamKey) {
        if self.null_params.borrow_mut().insert(k) {
            *self.null_names.borrow_mut().entry(k.name).or_insert(0) += 1;
        }
    }

    /// Removes `k`; whether it was there.
    pub(crate) fn remove_null_param(&self, k: ParamKey) -> bool {
        if !self.null_params.borrow_mut().remove(&k) {
            return false;
        }
        let mut names = self.null_names.borrow_mut();
        if let Some(n) = names.get_mut(&k.name) {
            *n -= 1;
            if *n == 0 {
                names.remove(&k.name);
            }
        }
        true
    }

    /// Whether `x` (in unit `u`) is proved to be `null`: the literal, a
    /// constant whose value is `null` or a parameter proved always `null`.
    pub fn is_null(&self, u: UnitId, x: ExprId) -> bool {
        self.expr(u, x, &HashMap::new(), 0) == Some(Value::Null)
    }

    /// Whether some write or local declaration of the name `name` lies
    /// inside `start..end` of unit `u`: a parameter written in its body is not
    /// a constant, and a local of the same name would make a read of that
    /// name ambiguous (a read resolves as a local either way). A write to a
    /// shadowing local counts too, which is only conservative.
    pub(crate) fn written_in(&self, u: UnitId, start: usize, end: usize, name: SymbolId) -> bool {
        let up = self.unit_params(u);
        up.writes
            .get(&name)
            .is_some_and(|ws| ws.iter().any(|w| *w >= start && *w < end))
    }

    /// The value of a read of parameter `name` at `x`: `null` when the
    /// innermost function that contains `x` and declares `name` has it in
    /// [`Constants::null_params`].
    fn parameter(&self, u: UnitId, x: ExprId, name: SymbolId) -> Option<Value> {
        if !self.null_names.borrow().contains_key(&name) {
            return None;
        }
        let up = self.unit_params(u);
        let at = self.e.program.unit(u).ast.expr(x).span;
        let mut best: Option<(usize, u32)> = None;
        for (start, end, offset) in up.by_name.get(&name)?.iter() {
            if *start <= at.start && at.end <= *end {
                let size = end - start;
                if best.is_none_or(|(s, _)| size < s) {
                    best = Some((size, *offset));
                }
            }
        }
        let (_, offset) = best?;
        let key = ParamKey {
            unit: u,
            offset,
            name,
        };
        self.null_params
            .borrow()
            .contains(&key)
            .then_some(Value::Null)
    }

    fn unit_params(&self, u: UnitId) -> Rc<UnitParams> {
        if let Some(up) = self.units.borrow().get(&u) {
            return Rc::clone(up);
        }
        let up = Rc::new(unit_params(&self.e.program.unit(u).ast));
        self.units.borrow_mut().insert(u, Rc::clone(&up));
        up
    }

    pub(crate) fn program(&self) -> &'a dartforge_elements::model::Program {
        self.e.program
    }

    /// `assert`s are disabled (the emitter does not write them).
    pub fn without_asserts(&self) -> bool {
        self.no_asserts
    }

    /// The constant boolean value of `x` (in unit `u`), if proved.
    pub fn bool_of(&self, u: UnitId, x: ExprId) -> Option<bool> {
        match self.expr(u, x, &HashMap::new(), 0)? {
            Value::Bool(b) => Some(b),
            Value::Null => None,
        }
    }

    /// The constant value of the condition `x` in the forms the world and
    /// the emitter fold (`impact::constant_condition`,
    /// `emit_js::expr::Emitter::constant`): not a bare literal.
    pub fn folded_condition(&self, u: UnitId, x: ExprId) -> Option<bool> {
        match &self.e.program.unit(u).ast.expr(x).kind {
            ExprKind::Identifier(_)
            | ExprKind::Property { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Binary { .. }
            | ExprKind::Is { .. }
            | ExprKind::Call { .. }
            | ExprKind::Parenthesized(_) => self.bool_of(u, x),
            _ => None,
        }
    }

    /// Whether statement `s` always leaves the function (`return`, `throw`,
    /// `rethrow`, a block with such a statement, an `if` whose folded
    /// condition picks a branch that leaves): what follows it in a block is
    /// dead. The emitter mirrors this rule (`emit_js::body`,
    /// `Emitter::always_exits`).
    pub fn always_exits(&self, u: UnitId, s: ast::StmtId) -> bool {
        let ast = &self.e.program.unit(u).ast;
        match &ast.stmt(s).kind {
            StmtKind::Return(_) => true,
            StmtKind::Expression(x) => {
                matches!(ast.expr(*x).kind, ExprKind::Throw(_) | ExprKind::Rethrow)
            }
            StmtKind::Block(ss) => ss.iter().any(|t| self.always_exits(u, *t)),
            StmtKind::If {
                condition,
                case_pattern: None,
                guard: None,
                then,
                else_,
            } => match self.folded_condition(u, *condition) {
                Some(true) => self.always_exits(u, *then),
                Some(false) => else_.is_some_and(|e| self.always_exits(u, e)),
                None => false,
            },
            _ => false,
        }
    }

    fn resolved(&self, u: UnitId, x: ExprId) -> Option<&'a Resolved> {
        self.e.bodies.units.get(u.0 as usize)?.get_resolved(x)
    }

    fn expr(
        &self,
        u: UnitId,
        x: ExprId,
        locals: &HashMap<SymbolId, Value>,
        depth: u32,
    ) -> Option<Value> {
        if depth > 24 {
            return None;
        }
        let ast = &self.e.program.unit(u).ast;
        match &ast.expr(x).kind {
            ExprKind::Bool(b) => Some(Value::Bool(*b)),
            ExprKind::Null => Some(Value::Null),
            ExprKind::Parenthesized(i) => self.expr(u, *i, locals, depth + 1),
            ExprKind::Unary {
                op: UnaryOp::Not,
                operand,
            } => match self.expr(u, *operand, locals, depth + 1)? {
                Value::Bool(b) => Some(Value::Bool(!b)),
                Value::Null => None,
            },
            ExprKind::Binary {
                op: BinaryOp::And,
                left,
                right,
            } => match self.expr(u, *left, locals, depth + 1)? {
                Value::Bool(false) => Some(Value::Bool(false)),
                Value::Bool(true) => match self.expr(u, *right, locals, depth + 1)? {
                    Value::Bool(b) => Some(Value::Bool(b)),
                    Value::Null => None,
                },
                Value::Null => None,
            },
            ExprKind::Binary {
                op: BinaryOp::Or,
                left,
                right,
            } => match self.expr(u, *left, locals, depth + 1)? {
                Value::Bool(true) => Some(Value::Bool(true)),
                Value::Bool(false) => match self.expr(u, *right, locals, depth + 1)? {
                    Value::Bool(b) => Some(Value::Bool(b)),
                    Value::Null => None,
                },
                Value::Null => None,
            },
            ExprKind::Binary {
                op: op @ (BinaryOp::Eq | BinaryOp::NotEq),
                left,
                right,
            } if (self.gated_constant(u, *left) && self.pure_rti_read(u, *right, 0))
                || (self.gated_constant(u, *right) && self.pure_rti_read(u, *left, 0)) =>
            {
                Some(Value::Bool(*op == BinaryOp::NotEq))
            }
            ExprKind::Is { value, ty, negated }
                if self.without_records.get() && self.is_record_test(u, *value, *ty) =>
            {
                Some(Value::Bool(*negated))
            }
            ExprKind::Binary {
                op: op @ (BinaryOp::Eq | BinaryOp::NotEq),
                left,
                right,
            } => {
                let a = self.expr(u, *left, locals, depth + 1)?;
                let b = self.expr(u, *right, locals, depth + 1)?;
                Some(Value::Bool((a == b) == (*op == BinaryOp::Eq)))
            }
            ExprKind::Call { target, arguments } if self.legacy_identity(u, *target, arguments) => {
                Some(Value::Bool(false))
            }
            ExprKind::Call { target, arguments } => self.flag(u, *target, arguments),
            ExprKind::Identifier(n) => match self.resolved(u, x)? {
                // A parameter read resolves as a local (or, in some
                // scopes, as a parameter).
                Resolved::Local(_) => match locals.get(&n.sym) {
                    Some(v) => Some(*v),
                    None => self.parameter(u, x, n.sym),
                },
                Resolved::Parameter { name, .. } => self.parameter(u, x, *name),
                Resolved::Element(el) => self.element(*el, depth),
                _ => None,
            },
            ExprKind::Property { .. } => match self.resolved(u, x)? {
                Resolved::Element(el) => self.element(*el, depth),
                _ => None,
            },
            _ => None,
        }
    }

    fn element(&self, el: Element, depth: u32) -> Option<Value> {
        match el {
            Element::Function(f) => self.getter(f, depth),
            Element::Variable(v) => self.variable(v, depth),
            _ => None,
        }
    }

    fn variable(&self, v: VariableId, depth: u32) -> Option<Value> {
        if let Some(r) = self.memo_var.borrow().get(&v) {
            return *r;
        }
        self.memo_var.borrow_mut().insert(v, None);
        let p = self.e.program;
        let var = p.variable(v);
        let r = (|| {
            if var.external || var.extension.is_some() {
                return None;
            }
            let (u, list) = match var.node {
                VariableRef::TopLevel { unit, decl, index } => {
                    match &p.unit(unit).ast.decl(decl).kind {
                        ast::DeclKind::Variables(l) => (unit, (l, index)),
                        _ => return None,
                    }
                }
                VariableRef::Field {
                    unit,
                    member,
                    index,
                } => match &p.unit(unit).ast.member(member).kind {
                    ast::MemberKind::Field(l) => (unit, (l, index)),
                    _ => return None,
                },
                _ => return None,
            };
            let (l, index) = list;
            // `const`, or a non-`late` top-level/static `final`: never rewritten.
            if !(l.const_ || (l.final_ && !l.late)) || (var.class.is_some() && !var.static_) {
                return None;
            }
            let init = l.variables.get(index)?.initializer?;
            self.expr(u, init, &HashMap::new(), depth + 1)
        })();
        self.memo_var.borrow_mut().insert(v, r);
        r
    }

    fn getter(&self, f: FunctionElementId, depth: u32) -> Option<Value> {
        if let Some(r) = self.memo_fn.borrow().get(&f) {
            return *r;
        }
        // Recursion: unknown until it finishes.
        self.memo_fn.borrow_mut().insert(f, None);
        let p = self.e.program;
        let func = p.function(f);
        let r = (|| {
            if let Some(v) = func.variable
                && func.kind == FunctionKind::ImplicitAccessor
            {
                return self.variable(v, depth);
            }
            if func.kind != FunctionKind::Getter
                || func.external
                || (func.class.is_some() && !func.static_)
                || func.extension.is_some()
            {
                return None;
            }
            let FunctionRef::Function { unit, function } = func.node else {
                return None;
            };
            let ast = &p.unit(unit).ast;
            let fun = ast.function(function);
            if fun.modifier != ast::AsyncModifier::None {
                return None;
            }
            match fun.body {
                FunctionBody::Expression(x) => self.expr(unit, x, &HashMap::new(), depth + 1),
                FunctionBody::Block(b) => self.block(unit, b, depth),
                _ => None,
            }
        })();
        self.memo_fn.borrow_mut().insert(f, r);
        r
    }

    /// `{ var a = lit; …; assert(…); return e; }` — the `assert` disappears
    /// only with asserts disabled; with them enabled, any `assert` makes the
    /// body unknown (it may write to a local).
    fn block(&self, u: UnitId, b: ast::StmtId, depth: u32) -> Option<Value> {
        let ast = &self.e.program.unit(u).ast;
        let StmtKind::Block(ss) = &ast.stmt(b).kind else {
            return None;
        };
        let mut locals: HashMap<SymbolId, Value> = HashMap::new();
        for (i, s) in ss.iter().enumerate() {
            match &ast.stmt(*s).kind {
                StmtKind::Variables(l) if !l.late => {
                    for v in l.variables.iter() {
                        let x = v.initializer?;
                        let val = self.expr(u, x, &locals, depth + 1)?;
                        locals.insert(v.name.sym, val);
                    }
                }
                StmtKind::Assert { .. } if self.no_asserts => {}
                StmtKind::Return(Some(x)) if i + 1 == ss.len() => {
                    return self.expr(u, *x, &locals, depth + 1);
                }
                _ => return None,
            }
        }
        None
    }
}

/// Records the identifier that `x` (through parentheses) writes.
fn write_target(a: &ast::Ast, x: ExprId, out: &mut Vec<(SymbolId, usize)>) {
    let mut cur = x;
    while let ExprKind::Parenthesized(i) = &a.expr(cur).kind {
        cur = *i;
    }
    if let ExprKind::Identifier(n) = &a.expr(cur).kind {
        out.push((n.sym, a.expr(cur).span.start));
    }
}

/// The parameter declarations and the writes of a unit ([`UnitParams`]).
fn unit_params(a: &ast::Ast) -> UnitParams {
    let mut up = UnitParams::default();
    let mut declare = |start: usize, end: usize, ps: &[ast::Parameter]| {
        for p in ps {
            if let Some(n) = p.name {
                up.by_name
                    .entry(n.sym)
                    .or_default()
                    .push((start, end, n.span.start as u32));
            }
        }
    };
    for f in a.functions.iter() {
        if let Some(ps) = f.parameters.as_deref() {
            declare(f.span.start, f.span.end, ps);
        }
    }
    for m in a.members.iter() {
        if let ast::MemberKind::Constructor(k) = &m.kind {
            declare(m.span.start, m.span.end, &k.parameters);
        }
    }
    let mut writes: Vec<(SymbolId, usize)> = Vec::new();
    fn elements(a: &ast::Ast, els: &[ast::CollectionElement], out: &mut Vec<(SymbolId, usize)>) {
        for el in els {
            match el {
                ast::CollectionElement::ForIn {
                    target: ast::ForInTarget::Expression(x),
                    body,
                    ..
                } => {
                    write_target(a, *x, out);
                    elements(a, std::slice::from_ref(&**body), out);
                }
                ast::CollectionElement::ForIn {
                    target: ast::ForInTarget::Declared { name, .. },
                    body,
                    ..
                } => {
                    out.push((name.sym, name.span.start));
                    elements(a, std::slice::from_ref(&**body), out);
                }
                ast::CollectionElement::For {
                    init: Some(ast::ForInit::Variables(l)),
                    body,
                    ..
                } => {
                    for v in l.variables.iter() {
                        out.push((v.name.sym, v.name.span.start));
                    }
                    elements(a, std::slice::from_ref(&**body), out);
                }
                ast::CollectionElement::ForIn { body, .. }
                | ast::CollectionElement::For { body, .. } => {
                    elements(a, std::slice::from_ref(&**body), out)
                }
                ast::CollectionElement::If { then, else_, .. } => {
                    elements(a, std::slice::from_ref(&**then), out);
                    if let Some(e) = else_ {
                        elements(a, std::slice::from_ref(&**e), out);
                    }
                }
                _ => {}
            }
        }
    }
    for x in a.exprs.iter() {
        match &x.kind {
            ExprKind::Assign { target: t, .. } => write_target(a, *t, &mut writes),
            ExprKind::Unary {
                op:
                    UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec,
                operand,
            } => write_target(a, *operand, &mut writes),
            ExprKind::List { elements: els, .. } | ExprKind::SetOrMap { elements: els, .. } => {
                elements(a, els, &mut writes)
            }
            _ => {}
        }
    }
    let variables = |l: &ast::VariableList, out: &mut Vec<(SymbolId, usize)>| {
        for v in l.variables.iter() {
            out.push((v.name.sym, v.name.span.start));
        }
    };
    for s in a.stmts.iter() {
        match &s.kind {
            StmtKind::ForIn {
                target: ast::ForInTarget::Expression(x),
                ..
            } => write_target(a, *x, &mut writes),
            StmtKind::ForIn {
                target: ast::ForInTarget::Declared { name, .. },
                ..
            } => writes.push((name.sym, name.span.start)),
            StmtKind::Variables(l)
            | StmtKind::For {
                init: Some(ast::ForInit::Variables(l)),
                ..
            } => variables(l, &mut writes),
            StmtKind::Function(f) => {
                if let Some(n) = a.function(*f).name {
                    writes.push((n.sym, n.span.start));
                }
            }
            StmtKind::Try { catches, .. } => {
                for c in catches.iter() {
                    for n in [c.exception, c.stack_trace].into_iter().flatten() {
                        writes.push((n.sym, n.span.start));
                    }
                }
            }
            _ => {}
        }
    }
    // A variable pattern names a variable it declares or (in a pattern
    // assignment) writes: every one counts.
    for p in a.patterns.iter() {
        if let ast::PatternKind::Variable { name, .. } = &p.kind {
            writes.push((name.sym, name.span.start));
        }
    }
    for (n, at) in writes {
        up.writes.entry(n).or_default().push(at);
    }
    up
}
