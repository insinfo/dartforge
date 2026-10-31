//! Locals that are never negative in a function body (production), for the
//! shift and the remainder by a literal (`docs/JS-PRODUCAO-VELOCIDADE.md`
//! §12).
//!
//! `x >> k` of an `int` is `JSNumber.>>`: `x > 0 ? x >>> k : (x >> k) >>> 0`
//! (`dart.shrCount`). For an `x` that is not negative both branches give
//! `x >>> k` (`0`, `-0`, `NaN` and the infinities included), which is what
//! dart2js writes after its type inference proves the receiver
//! non-negative. `x % k` is `(x % k + k) % k`; for a dividend that is never
//! negative (and never `-0`: no form below produces it from literals) the
//! JavaScript remainder alone is Dart's. The proof here is flow-insensitive
//! and local to one
//! function body: a local declared exactly once in the body, not visible
//! from outside it, whose initializer and every assignment produce a value
//! that is never negative assuming the same of the other candidates (the
//! largest fixed point: candidates leave until none has a doubtful
//! assignment). A body with a closure, a local function, a pattern, a
//! `switch`, a `try` or a foreign intrinsic (`JS(…)`, which can write a
//! local through its template) has no candidates: those forms capture,
//! declare or rebind locals in ways this pass does not follow.
//!
//! Never negative, for an expression: a non-negative integer literal, a
//! candidate, `a + b`, `a * b` and `a ~/ b` of never-negative operands, and,
//! with a never-negative receiver (a number, not an object with its own
//! operator), `a % b` (Dart's remainder is never negative), a shift by a
//! literal count (the web's shifts are unsigned) and `a & k` with a literal
//! `0 <= k < 2^31`; a conditional whose two branches are never negative.
//! `DARTFORGE_JSPROD_NONNEGATIVE_SHIFTS=0` turns the pass off.

use crate::body::FnEmitter;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    AssignOp, BinaryOp, ExprId, ExprKind, ForInTarget, ForInit, StmtKind, UnaryOp,
};
use dartforge_intern::SymbolId;
use std::collections::{HashMap, HashSet};

/// The body being emitted and, once asked, its never-negative locals.
pub(crate) struct NonNegative {
    span: Span,
    /// Locals visible when the body starts (parameters, the enclosing
    /// function's locals): never candidates.
    outer: HashSet<SymbolId>,
    locals: Option<HashSet<SymbolId>>,
}

impl NonNegative {
    pub(crate) fn new(span: Span, outer: HashSet<SymbolId>) -> Self {
        NonNegative {
            span,
            outer,
            locals: None,
        }
    }
}

pub(crate) fn enabled() -> bool {
    std::env::var("DARTFORGE_JSPROD_NONNEGATIVE_SHIFTS").map_or(true, |v| v != "0")
}

/// What an assignment to a candidate requires.
enum Write {
    /// The value must be never negative.
    Value(ExprId),
    /// Never negative whatever the operand (`%=`, a shift).
    Always,
    /// Never negative when the variable and the operand are (`+=`, `++`).
    Keeps(Option<ExprId>),
    /// Can be negative.
    Doubtful,
}

impl FnEmitter<'_, '_> {
    /// Is `binary` (the node of `x >> k`) a shift whose receiver is a
    /// never-negative local of the current body?
    pub(crate) fn nonnegative_receiver(&mut self, binary: ExprId) -> bool {
        let ExprKind::Binary { left, .. } = self.expr(binary).kind else {
            return false;
        };
        let mut left = left;
        while let ExprKind::Parenthesized(e) = self.expr(left).kind {
            left = e;
        }
        if !matches!(self.expr(left).kind, ExprKind::Identifier(_)) {
            return false;
        }
        self.nonnegative_operand(left)
    }

    /// Is `e` never negative in the current body? Every name it reads must
    /// resolve to a local here: a top-level of the same name used in a
    /// sibling block of the declaration is not the candidate.
    pub(crate) fn nonnegative_operand(&mut self, e: ExprId) -> bool {
        let Some(state) = self.nonnegative.as_ref() else {
            return false;
        };
        if state.locals.is_none() {
            let locals = self.nonnegative_locals(state.span, &state.outer);
            if let Some(s) = self.nonnegative.as_mut() {
                s.locals = Some(locals);
            }
        }
        let Some(set) = self.nonnegative.as_ref().and_then(|s| s.locals.as_ref()) else {
            return false;
        };
        self.never_negative(e, set) && self.names_are_locals(e)
    }

    /// Every identifier of `e` resolves to a local of the current scope.
    fn names_are_locals(&self, e: ExprId) -> bool {
        match &self.expr(e).kind {
            ExprKind::Identifier(n) => self.lookup_local(n.sym).is_some(),
            ExprKind::Parenthesized(x) => self.names_are_locals(*x),
            ExprKind::Binary { left, right, .. } => {
                self.names_are_locals(*left) && self.names_are_locals(*right)
            }
            ExprKind::Conditional { then, else_, .. } => {
                self.names_are_locals(*then) && self.names_are_locals(*else_)
            }
            _ => true,
        }
    }

    fn nonnegative_locals(&self, span: Span, outer: &HashSet<SymbolId>) -> HashSet<SymbolId> {
        let ast = self.ast();
        let inside = |s: Span| s.start >= span.start && s.end <= span.end;
        if ast.patterns.iter().any(|p| inside(p.span)) {
            return HashSet::new();
        }
        let mut declared: HashMap<SymbolId, usize> = HashMap::new();
        let mut writes: Vec<(SymbolId, Write)> = Vec::new();
        for s in ast.stmts.iter().filter(|s| inside(s.span)) {
            let list = match &s.kind {
                StmtKind::Function(_)
                | StmtKind::PatternVariables { .. }
                | StmtKind::Switch { .. }
                | StmtKind::Try { .. } => return HashSet::new(),
                StmtKind::Variables(list) => Some(list),
                StmtKind::For {
                    init: Some(ForInit::Variables(list)),
                    ..
                } => Some(list),
                StmtKind::For {
                    init: Some(ForInit::Pattern { .. }),
                    ..
                } => return HashSet::new(),
                StmtKind::ForIn { target, .. } => {
                    match target {
                        ForInTarget::Declared { name, .. } => {
                            *declared.entry(name.sym).or_default() += 1;
                            writes.push((name.sym, Write::Doubtful));
                        }
                        _ => return HashSet::new(),
                    }
                    None
                }
                _ => None,
            };
            if let Some(list) = list {
                for v in list.variables.iter() {
                    *declared.entry(v.name.sym).or_default() += 1;
                    if list.late {
                        writes.push((v.name.sym, Write::Doubtful));
                    } else if let Some(i) = v.initializer {
                        writes.push((v.name.sym, Write::Value(i)));
                    }
                }
            }
        }
        for e in ast.exprs.iter().filter(|e| inside(e.span)) {
            match &e.kind {
                ExprKind::FunctionExpression(_)
                | ExprKind::PatternAssign { .. }
                | ExprKind::Switch { .. } => return HashSet::new(),
                // A foreign intrinsic (`JS('', '# = …', x)`) can write a local
                // the AST does not show as written.
                ExprKind::Call { target, .. }
                    if crate::own_sdk::called_function(self, *target).is_some_and(|f| {
                        self.ctx
                            .sdk
                            .as_ref()
                            .is_some_and(|sdk| sdk.intrinsics.contains_key(&f))
                    }) =>
                {
                    return HashSet::new();
                }
                ExprKind::Assign { op, target, value } => {
                    let ExprKind::Identifier(n) = self.expr(*target).kind else {
                        continue;
                    };
                    let w = match op {
                        AssignOp::Assign => Write::Value(*value),
                        AssignOp::Compound(BinaryOp::Add | BinaryOp::Mul | BinaryOp::TruncDiv) => {
                            Write::Keeps(Some(*value))
                        }
                        AssignOp::Compound(BinaryOp::IfNull) => Write::Value(*value),
                        AssignOp::Compound(BinaryOp::Rem) => Write::Always,
                        AssignOp::Compound(BinaryOp::Shl | BinaryOp::Shr | BinaryOp::UShr)
                            if self.literal_count(*value) =>
                        {
                            Write::Always
                        }
                        AssignOp::Compound(BinaryOp::BitAnd) if self.small_literal(*value) => {
                            Write::Always
                        }
                        _ => Write::Doubtful,
                    };
                    writes.push((n.sym, w));
                }
                ExprKind::Unary { op, operand } => {
                    let ExprKind::Identifier(n) = self.expr(*operand).kind else {
                        continue;
                    };
                    match op {
                        UnaryOp::PrefixInc | UnaryOp::PostfixInc => {
                            writes.push((n.sym, Write::Keeps(None)))
                        }
                        UnaryOp::PrefixDec | UnaryOp::PostfixDec => {
                            writes.push((n.sym, Write::Doubtful))
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        let mut set: HashSet<SymbolId> = declared
            .iter()
            .filter(|&(s, &n)| n == 1 && !outer.contains(s))
            .map(|(&s, _)| s)
            .collect();
        loop {
            let doubtful: Vec<SymbolId> = writes
                .iter()
                .filter(|(s, w)| {
                    set.contains(s)
                        && match w {
                            Write::Value(v) => !self.never_negative(*v, &set),
                            Write::Always => false,
                            Write::Keeps(v) => v.is_some_and(|v| !self.never_negative(v, &set)),
                            Write::Doubtful => true,
                        }
                })
                .map(|(s, _)| *s)
                .collect();
            if doubtful.is_empty() {
                return set;
            }
            for s in doubtful {
                set.remove(&s);
            }
        }
    }

    /// A decimal or hexadecimal literal from 0 to 31.
    fn literal_count(&self, e: ExprId) -> bool {
        self.int_literal(e).is_some_and(|v| v <= 31)
    }

    /// A literal from 0 to 2^31 - 1.
    fn small_literal(&self, e: ExprId) -> bool {
        self.int_literal(e).is_some_and(|v| v <= 0x7fff_ffff)
    }

    fn int_literal(&self, e: ExprId) -> Option<u64> {
        let ExprKind::Int(span) = self.expr(e).kind else {
            return None;
        };
        let text = self.text(span).replace('_', "");
        match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            Some(h) => u64::from_str_radix(h, 16).ok(),
            None => text.parse().ok(),
        }
    }

    fn never_negative(&self, e: ExprId, set: &HashSet<SymbolId>) -> bool {
        match &self.expr(e).kind {
            ExprKind::Int(_) => self.int_literal(e).is_some(),
            ExprKind::Parenthesized(x) => self.never_negative(*x, set),
            ExprKind::Identifier(n) => set.contains(&n.sym),
            ExprKind::Binary { op, left, right } => match op {
                BinaryOp::Add | BinaryOp::Mul | BinaryOp::TruncDiv => {
                    self.never_negative(*left, set) && self.never_negative(*right, set)
                }
                // The receiver never negative makes it a number (a literal or
                // arithmetic of candidates), not an object with its own
                // operator.
                BinaryOp::Rem => self.never_negative(*left, set),
                BinaryOp::Shl | BinaryOp::Shr | BinaryOp::UShr => {
                    self.never_negative(*left, set) && self.literal_count(*right)
                }
                BinaryOp::BitAnd => {
                    self.never_negative(*left, set)
                        && self.never_negative(*right, set)
                        && (self.small_literal(*left) || self.small_literal(*right))
                }
                _ => false,
            },
            ExprKind::Conditional { then, else_, .. } => {
                self.never_negative(*then, set) && self.never_negative(*else_, set)
            }
            _ => false,
        }
    }
}
