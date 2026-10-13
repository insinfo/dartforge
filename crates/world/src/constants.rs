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

use crate::WorldInput;
use dartforge_elements::model::{Element, FunctionElementId, FunctionKind, FunctionRef, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, FunctionBody, StmtKind, UnaryOp};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::Resolved;
use std::cell::RefCell;
use std::collections::HashMap;

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
}

impl<'a> Constants<'a> {
    pub fn new(e: WorldInput<'a>, no_asserts: bool) -> Constants<'a> {
        Constants { e, no_asserts, memo_fn: RefCell::new(HashMap::new()), memo_var: RefCell::new(HashMap::new()) }
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

    fn resolved(&self, u: UnitId, x: ExprId) -> Option<&'a Resolved> {
        self.e.bodies.units.get(u.0 as usize)?.get_resolved(x)
    }

    fn expr(&self, u: UnitId, x: ExprId, locals: &HashMap<SymbolId, Value>, depth: u32) -> Option<Value> {
        if depth > 24 {
            return None;
        }
        let ast = &self.e.program.unit(u).ast;
        match &ast.expr(x).kind {
            ExprKind::Bool(b) => Some(Value::Bool(*b)),
            ExprKind::Null => Some(Value::Null),
            ExprKind::Parenthesized(i) => self.expr(u, *i, locals, depth + 1),
            ExprKind::Unary { op: UnaryOp::Not, operand } => match self.expr(u, *operand, locals, depth + 1)? {
                Value::Bool(b) => Some(Value::Bool(!b)),
                Value::Null => None,
            },
            ExprKind::Binary { op: BinaryOp::And, left, right } => match self.expr(u, *left, locals, depth + 1)? {
                Value::Bool(false) => Some(Value::Bool(false)),
                Value::Bool(true) => match self.expr(u, *right, locals, depth + 1)? {
                    Value::Bool(b) => Some(Value::Bool(b)),
                    Value::Null => None,
                },
                Value::Null => None,
            },
            ExprKind::Binary { op: BinaryOp::Or, left, right } => match self.expr(u, *left, locals, depth + 1)? {
                Value::Bool(true) => Some(Value::Bool(true)),
                Value::Bool(false) => match self.expr(u, *right, locals, depth + 1)? {
                    Value::Bool(b) => Some(Value::Bool(b)),
                    Value::Null => None,
                },
                Value::Null => None,
            },
            ExprKind::Binary { op: op @ (BinaryOp::Eq | BinaryOp::NotEq), left, right } => {
                let a = self.expr(u, *left, locals, depth + 1)?;
                let b = self.expr(u, *right, locals, depth + 1)?;
                Some(Value::Bool((a == b) == (*op == BinaryOp::Eq)))
            }
            ExprKind::Identifier(n) => match self.resolved(u, x)? {
                Resolved::Local(_) => locals.get(&n.sym).copied(),
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
                VariableRef::TopLevel { unit, decl, index } => match &p.unit(unit).ast.decl(decl).kind {
                    ast::DeclKind::Variables(l) => (unit, (l, index)),
                    _ => return None,
                },
                VariableRef::Field { unit, member, index } => match &p.unit(unit).ast.member(member).kind {
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
            if let Some(v) = func.variable {
                if func.kind == FunctionKind::ImplicitAccessor {
                    return self.variable(v, depth);
                }
            }
            if func.kind != FunctionKind::Getter || func.external || (func.class.is_some() && !func.static_) || func.extension.is_some() {
                return None;
            }
            let FunctionRef::Function { unit, function } = func.node else { return None };
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
        let StmtKind::Block(ss) = &ast.stmt(b).kind else { return None };
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
                StmtKind::Return(Some(x)) if i + 1 == ss.len() => return self.expr(u, *x, &locals, depth + 1),
                _ => return None,
            }
        }
        None
    }
}
