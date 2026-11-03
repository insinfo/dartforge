//! Dynamic invocations, the only code that can reach `noSuchMethod`
//! (WebAssembly modules).
//!
//! A statically checked call cannot miss: the receiver's class has the member
//! the static type promises (an implementation or a `noSuchMethod` forwarder),
//! and the arguments fit its signature. Only a dynamic invocation — a member of
//! a `dynamic` receiver, a call of a value whose static type is not a function
//! type (`Function`, `dynamic`, a type variable), a relational pattern —, a
//! `noSuchMethod` forwarder, `Function.apply` and the dynamic operators of the
//! runtime can make the runtime build an `Invocation` and call `noSuchMethod`
//! (`runtime/src/selectors.rs`, `closures.rs`, `dispatch.rs`), as in
//! dart2wasm, whose TFA keeps `noSuchMethod` only for dynamic calls.
//!
//! Each dynamic invocation calls `dartforge_dynamic_invocation` (an empty
//! runtime function the module's LTO removes), and the two Dart helpers of
//! that machinery are registered by the program under a `df.ref-when` on the
//! producers ([`guarded_helper`]): when no live function cites one, the link
//! leaves the registration out, and with it `NoSuchMethodError`,
//! `_InvocationMirror` and the `LinkedHashMap` of `namedArguments` (−330 KB
//! of a hello world). The same guard keeps the helper that indexes again the
//! maps of a copied message only when the program has ports. The program
//! first registers each one with address 0, so a path this analysis missed
//! stops with an internal error (`natives_lists.rs`, `helper`) instead of
//! answering with another error than the VM's.
//!
//! Native executables keep every helper (the mark is not emitted).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, ExprId, ExprKind, UnaryOp};
use dartforge_types::table::TypeId;

/// The runtime entry each dynamic invocation calls.
pub const MARK: &str = "dartforge_dynamic_invocation";

/// What lets the runtime call `_dartforgeNoSuchMethod`: the mark, the
/// forwarders of a class with its own `noSuchMethod`, `Function.apply` and the
/// runtime's operators over `dynamic`.
const NO_SUCH_METHOD_PRODUCERS: &[&str] = &[
    MARK,
    "dartforge_forward_nsm",
    "dartforge_native_Function_apply",
    "dartforge_dyn_op",
];

/// What lets the runtime build the SDK's `NoSuchMethodError` through
/// `_dartforgeCallError`: the producers of `noSuchMethod` and the generated
/// code that throws it directly (`FnBuilder::throw_nsm`).
const CALL_ERROR_PRODUCERS: &[&str] = &[
    MARK,
    "dartforge_forward_nsm",
    "dartforge_native_Function_apply",
    "dartforge_dyn_op",
    "dartforge_no_such_method_error_new",
];

/// What can deliver a copied message, whose maps and sets the runtime asks
/// `_dartforgeRebuildCopiedIndex` to index again (`runtime/src/ports.rs`,
/// `redo_copied_indices`): a port of the program and the isolate natives.
const PORT_PRODUCERS: &[&str] = &[
    "dartforge_native_DartForge_port_*",
    "dartforge_native_DartForge_ports_dispatcher",
    "dartforge_native_Isolate_*",
    "dartforge_api_post_*",
];

/// The producers (`df.ref-when` patterns) of a runtime helper the program
/// registers only when some live function cites one, on WebAssembly.
/// `None`: the SDK library registers it at startup.
pub fn guarded_helper(name: &str) -> Option<&'static [&'static str]> {
    if !crate::target::is_wasm() {
        return None;
    }
    match name {
        "_dartforgeNoSuchMethod" => Some(NO_SUCH_METHOD_PRODUCERS),
        "_dartforgeCallError" => Some(CALL_ERROR_PRODUCERS),
        "_dartforgeRebuildCopiedIndex" => Some(PORT_PRODUCERS),
        _ => None,
    }
}

/// `DARTFORGE_TRACE_DYNAMIC=1`: every function that gets the mark, and why, on
/// stderr (finding what keeps `noSuchMethod` in a module).
fn trace() -> bool {
    static T: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *T.get_or_init(|| std::env::var("DARTFORGE_TRACE_DYNAMIC").is_ok_and(|v| v == "1"))
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Marks a dynamic invocation (`why` names it for the trace).
    pub(super) fn note_dynamic_invocation(&mut self, why: &str) {
        if !crate::target::is_wasm() {
            return;
        }
        if trace() {
            eprintln!("[dynamic] {}: {why}", self.func.symbol);
        }
        self.emit(
            Instruction::CallRuntime {
                name: MARK.to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
    }

    /// Marks `expr` when it invokes a member of a `dynamic` receiver: the
    /// hook of every expression the lowering visits (`lower_expr`). The call
    /// of a value is decided where it is lowered ([`Self::note_value_call`]).
    pub(super) fn note_dynamic_expression(&mut self, ast: &ast::Ast, expr: ExprId) {
        if !crate::target::is_wasm() {
            return;
        }
        if let Some(why) = self.dynamic_invocation(ast, expr) {
            if trace() {
                let span = ast.expr(expr).span;
                let text = self.source().get(span.start..span.end).unwrap_or("");
                eprintln!(
                    "[dynamic]   `{}`",
                    text.chars().take(80).collect::<String>()
                );
            }
            self.note_dynamic_invocation(why);
        }
    }

    /// Marks the call of a function value whose static type `typ` (that of the
    /// called expression) is not a function type: `Function`, `dynamic`, a
    /// type variable or unknown — a call whose arguments nothing checked.
    pub(super) fn note_value_call(&mut self, typ: Option<TypeId>) {
        if !crate::target::is_wasm() || typ.is_some_and(|t| self.function_type(t)) {
            return;
        }
        self.note_dynamic_invocation("call of a value without a function type");
    }

    /// `(f as Function)(…)` (parentheses allowed) where `f` has a function type
    /// whose parameters accept the call's shape: the cast only loosens the
    /// static checks of the argument types, which the callee still makes
    /// (`TypeError`); the arity, which the static type fixed, cannot miss
    /// (`dart:_compact_hash` calls its `_equality` and `_hasher` this way).
    pub(super) fn call_shape_proved(
        &self,
        ast: &ast::Ast,
        target: ExprId,
        arguments: &ast::Arguments,
    ) -> bool {
        use dartforge_types::table::Type as T;
        let mut e = target;
        while let ExprKind::Parenthesized(inner) = ast.expr(e).kind {
            e = inner;
        }
        let ExprKind::As { value, .. } = ast.expr(e).kind else {
            return false;
        };
        let Some(t) = self.ctx.get_type_raw(self.unit_id, value) else {
            return false;
        };
        let T::Function {
            type_params,
            positional,
            optional,
            named,
            ..
        } = self.ctx.table.get(t)
        else {
            return false;
        };
        let given = arguments.args.iter().filter(|a| a.name.is_none()).count();
        let names: Vec<_> = arguments
            .args
            .iter()
            .filter_map(|a| a.name.map(|n| n.sym))
            .collect();
        (arguments.type_args.is_empty() || arguments.type_args.len() == type_params.len())
            && given >= positional.len()
            && given <= positional.len() + optional.len()
            && names.iter().all(|n| named.iter().any(|(m, _, _)| m == n))
            && named
                .iter()
                .all(|(m, _, required)| !required || names.contains(m))
    }

    /// A function type, also as the bound of a promoted type variable.
    fn function_type(&self, t: TypeId) -> bool {
        use dartforge_types::table::Type as T;
        match self.ctx.table.get(t) {
            T::Function { .. } => true,
            T::Intersection { bound, .. } => self.function_type(*bound),
            _ => false,
        }
    }

    /// The static type of `e` (an expression read as a value) is `dynamic`.
    /// With the extension types kept: the members of one are static even when
    /// its representation is `dynamic`. `this`, `super` and the receiver of a
    /// cascade section never are (the cascade's own target is checked).
    fn is_dynamic(&self, ast: &ast::Ast, e: ExprId) -> bool {
        use dartforge_elements::model::Element;
        use dartforge_types::resolved::Resolved;
        if matches!(
            ast.expr(e).kind,
            ExprKind::This | ExprKind::Super | ExprKind::CascadeTarget
        ) {
            return false;
        }
        // An import prefix or a type named as the target of a static member is
        // not a value (inference gives it no type).
        if matches!(
            self.ctx.get_resolved(self.unit_id, e),
            Some(
                Resolved::Prefix(_)
                    | Resolved::TypeParameter(_)
                    | Resolved::Element(
                        Element::Class(_)
                            | Element::Extension(_)
                            | Element::Typedef(_)
                            | Element::Prefix(..)
                    )
            )
        ) {
            return false;
        }
        self.ctx.get_type_raw(self.unit_id, e) == Some(self.ctx.core.dynamic_)
    }

    /// `t.x` or `t[i]` with a `dynamic` `t` (the target of an assignment or
    /// of `++`).
    fn dynamic_place(&self, ast: &ast::Ast, e: ExprId) -> bool {
        match &ast.expr(e).kind {
            ExprKind::Property { target, .. } | ExprKind::Index { target, .. } => {
                self.is_dynamic(ast, *target)
            }
            _ => false,
        }
    }

    /// Why `expr` is a dynamic invocation, if it is one. The members of
    /// `Object` read the way every class has them (`hashCode`, `runtimeType`,
    /// `toString()`, `==`) are not: the runtime's root selectors keep them in
    /// every live class.
    fn dynamic_invocation(&self, ast: &ast::Ast, expr: ExprId) -> Option<&'static str> {
        let name = |n: &ast::Name| self.ctx.symbol_name(n.sym);
        match &ast.expr(expr).kind {
            ExprKind::Property {
                target, name: n, ..
            } => (self.is_dynamic(ast, *target) && !matches!(name(n), "hashCode" | "runtimeType"))
                .then_some("member of a dynamic receiver"),
            ExprKind::Index { target, .. } => self
                .is_dynamic(ast, *target)
                .then_some("index of a dynamic receiver"),
            ExprKind::Call { target, arguments } => match &ast.expr(*target).kind {
                ExprKind::Property {
                    target: receiver,
                    name: n,
                    ..
                } => (self.is_dynamic(ast, *receiver)
                    && !(name(n) == "toString" && arguments.args.is_empty()))
                .then_some("method of a dynamic receiver"),
                _ => None,
            },
            ExprKind::Unary { op, operand } => match op {
                UnaryOp::Not | UnaryOp::NullAssert => None,
                UnaryOp::Neg | UnaryOp::BitNot => self
                    .is_dynamic(ast, *operand)
                    .then_some("operator of a dynamic operand"),
                // The place is not typed as a value: the increment has the type
                // of the value it reads.
                UnaryOp::PrefixInc
                | UnaryOp::PrefixDec
                | UnaryOp::PostfixInc
                | UnaryOp::PostfixDec => (self.is_dynamic(ast, expr)
                    || self.dynamic_place(ast, *operand))
                .then_some("increment of a dynamic value"),
            },
            ExprKind::Binary { op, left, .. } => (!matches!(
                op,
                BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull
            ) && self.is_dynamic(ast, *left))
            .then_some("operator of a dynamic operand"),
            // A compound assignment has the type of its operator's result.
            ExprKind::Assign { op, target, .. } => (self.dynamic_place(ast, *target)
                || (matches!(op, AssignOp::Compound(o) if *o != BinaryOp::IfNull)
                    && self.is_dynamic(ast, expr)))
            .then_some("assignment through a dynamic receiver"),
            ExprKind::Cascade { target, .. } => self
                .is_dynamic(ast, *target)
                .then_some("cascade on a dynamic receiver"),
            _ => None,
        }
    }
}
