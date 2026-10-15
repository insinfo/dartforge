//! Bounded proof for hoisting SDK list-identity guards to fresh allocation.
//!
//! Requires the existing complete source/foreign-code and confinement proof.
//! The remaining synchronous window contains only primitive local operations
//! and writes to separately allocated plain user objects. No Dart callback or
//! getter can run there. Arbitrary external JS mutation of compiler-private
//! class/function/RTI bindings is outside this closed-source certificate.

use crate::body::FnEmitter;
use crate::ty::Ty;
use dartforge_elements::model::FunctionRef;
use dartforge_frontend::ast::{
    self, BinaryOp, ExprId, ExprKind, FunctionBody, MemberKind, StmtId, StmtKind, UnaryOp,
};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::Resolved;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Primitive {
    Int,
    Bool,
    Double,
    String,
}

impl FnEmitter<'_, '_> {
    fn window_primitive_type(&self, ty: &Ty) -> Option<Primitive> {
        if self.ctx.is_int(ty) {
            Some(Primitive::Int)
        } else if self.ctx.is_bool(ty) {
            Some(Primitive::Bool)
        } else if self.ctx.is_double(ty) {
            Some(Primitive::Double)
        } else if self.ctx.is_string(ty) {
            Some(Primitive::String)
        } else {
            None
        }
    }

    fn window_local(&self, e: ExprId) -> bool {
        let valid = matches!(self.expr(e).kind, ExprKind::Identifier(_))
            && matches!(
                self.common_resolution(e),
                Some(Resolved::Local(_) | Resolved::Parameter { .. })
            );
        if !valid && std::env::var("DARTFORGE_TRACE_FRESH_LIST_APPEND").is_ok_and(|v| v == "1") {
            eprintln!(
                "closed-list local rejected: {} type={:?} resolution={:?}",
                self.text(self.expr(e).span),
                self.common_type(e),
                self.common_resolution(e)
            );
        }
        valid
    }

    fn window_primitive(&self, e: ExprId) -> Option<Primitive> {
        match &self.expr(e).kind {
            ExprKind::Int(_) => Some(Primitive::Int),
            ExprKind::Double(_) => Some(Primitive::Double),
            ExprKind::Bool(_) => Some(Primitive::Bool),
            ExprKind::String(s)
                if s.parts
                    .iter()
                    .all(|p| matches!(p, ast::StringPart::Text(_))) =>
            {
                Some(Primitive::String)
            }
            ExprKind::Identifier(_) if self.window_local(e) => {
                let ty = self.common_type(e);
                if std::env::var("DARTFORGE_TRACE_FRESH_LIST_APPEND").is_ok_and(|v| v == "1") {
                    eprintln!(
                        "closed-list primitive {} type={:?}",
                        self.text(self.expr(e).span),
                        ty
                    );
                }
                self.window_primitive_type(&ty?)
            }
            ExprKind::Parenthesized(value) => self.window_primitive(*value),
            ExprKind::Unary { op, operand } => {
                let kind = self.window_primitive(*operand)?;
                match op {
                    UnaryOp::Neg | UnaryOp::BitNot if kind == Primitive::Int => Some(kind),
                    UnaryOp::Not if kind == Primitive::Bool => Some(kind),
                    UnaryOp::PrefixInc
                    | UnaryOp::PrefixDec
                    | UnaryOp::PostfixInc
                    | UnaryOp::PostfixDec
                        if kind == Primitive::Int && self.window_local(*operand) =>
                    {
                        Some(kind)
                    }
                    _ => None,
                }
            }
            ExprKind::Binary { op, left, right } => {
                let (a, b) = (
                    self.window_primitive(*left)?,
                    self.window_primitive(*right)?,
                );
                match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::BitAnd
                    | BinaryOp::BitOr
                    | BinaryOp::BitXor
                    | BinaryOp::Shl
                    | BinaryOp::Shr
                    | BinaryOp::UShr
                        if a == Primitive::Int && b == a =>
                    {
                        Some(a)
                    }
                    BinaryOp::Eq | BinaryOp::NotEq if a == b => Some(Primitive::Bool),
                    BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq
                        if a == Primitive::Int && b == a =>
                    {
                        Some(Primitive::Bool)
                    }
                    BinaryOp::And | BinaryOp::Or if a == Primitive::Bool && b == a => Some(a),
                    _ => None,
                }
            }
            ExprKind::Assign { op, target, value } if self.window_local(*target) => {
                let (a, b) = (
                    self.window_primitive(*target)?,
                    self.window_primitive(*value)?,
                );
                if a != b {
                    return None;
                }
                match op {
                    ast::AssignOp::Assign => Some(a),
                    ast::AssignOp::Compound(
                        BinaryOp::Add
                        | BinaryOp::Sub
                        | BinaryOp::Mul
                        | BinaryOp::BitAnd
                        | BinaryOp::BitOr
                        | BinaryOp::BitXor,
                    ) if a == Primitive::Int => Some(a),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn window_field_constructor(&self, e: ExprId, element: &Ty) -> bool {
        let (target, arguments) = match &self.expr(e).kind {
            ExprKind::Call { target, arguments } => (*target, arguments.as_ref()),
            ExprKind::InstanceCreation {
                keyword, arguments, ..
            } if !matches!(keyword, Some(ast::CreationKeyword::Const)) => (e, arguments.as_ref()),
            _ => return false,
        };
        if !arguments.type_args.is_empty()
            || arguments
                .args
                .iter()
                .any(|a| a.name.is_some() || self.window_primitive(a.value).is_none())
        {
            return false;
        }
        let Some(Resolved::Constructor(fid)) = self
            .common_resolution(e)
            .or_else(|| self.common_resolution(target))
        else {
            return false;
        };
        let f = self.ctx.program.function(*fid);
        if f.factory || f.external || f.const_ || f.patched_by.is_some() {
            return false;
        }
        let Some(class) = f.class else {
            return false;
        };
        let c = self.ctx.program.class(class);
        if std::env::var("DARTFORGE_TRACE_FRESH_LIST_APPEND").is_ok_and(|v| v == "1") {
            eprintln!(
                "closed-list ctor: {} parent={:?} object={:?} shadow={} homonym={} constructor={:?}",
                self.ctx.class_name(class),
                c.supertype_class,
                self.ctx.object,
                c.shadowed_instance_members.len(),
                self.ctx.program.class_with_homonym(class),
                f.node
            );
        }

        if !self.ctx.is_subtype(&Ty::iface(class), element)
            || self.ctx.program.library(c.library).uri.starts_with("dart:")
            || !c.type_params.is_empty()
            || !c.mixins.is_empty()
            || !c.mixin_classes.is_empty()
            || c.supertype_class != self.ctx.object
            || !c.shadowed_instance_members.is_empty()
            || self.ctx.program.class_with_homonym(class)
            || self.ctx.program.augmentations.contains_key(&class)
            || crate::own_sdk::is_native_class(self.ctx, class)
        {
            return false;
        }
        if c.instance_members
            .keys()
            .chain(c.static_members.keys())
            .any(|s| self.ctx.name(*s).contains('$'))
        {
            return false;
        }
        let FunctionRef::Constructor { unit, member } = f.node else {
            return false;
        };
        let source = self.ctx.program.unit(unit);
        let MemberKind::Constructor(ctor) = &source.ast.member(member).kind else {
            return false;
        };
        if ctor.external
            || ctor.factory
            || ctor.redirect.is_some()
            || !ctor.initializers.is_empty()
            || ctor.parameters.len() != arguments.args.len()
        {
            return false;
        }
        match ctor.body {
            FunctionBody::Empty => {}
            FunctionBody::Block(body) if matches!(&source.ast.stmt(body).kind,StmtKind::Block(stmts) if stmts.is_empty()) =>
                {}
            _ => return false,
        }
        let Some(declaration) = c.decl else {
            return false;
        };
        let class_source = self.ctx.program.unit(declaration.unit);
        let ast::DeclKind::Class(decl) = &class_source.ast.decl(declaration.decl).kind else {
            return false;
        };
        if decl.primary_constructor.is_some() || decl.mixin_application {
            return false;
        }
        let emitter = FnEmitter::new(self.ctx, self.m, declaration.unit, Some(class), false);
        let mut fields = std::collections::HashMap::new();
        for member in &decl.members {
            // A user setter could intercept emitted field-formal assignment.
            // Keep this first certificate to plain field/constructor DTOs.
            if matches!(
                &class_source.ast.member(*member).kind,
                MemberKind::Method(_)
            ) {
                return false;
            }
            if let MemberKind::Field(v) = &class_source.ast.member(*member).kind {
                if v.static_
                    || v.late
                    || v.external
                    || v.variables
                        .iter()
                        .any(|v| v.initializer.is_some() || self.ctx.name(v.name.sym).contains('$'))
                {
                    return false;
                }
                let Some(ty) = v.ty else {
                    return false;
                };
                let Some(primitive) = emitter.window_primitive_type(&emitter.resolve_type(ty))
                else {
                    return false;
                };
                for field in v.variables.iter() {
                    if self.ctx.object.is_some_and(|object| {
                        self.ctx
                            .program
                            .class(object)
                            .instance_members
                            .contains_key(&field.name.sym)
                    }) {
                        return false;
                    }
                    if fields.insert(field.name.sym, primitive).is_some() {
                        return false;
                    }
                }
            }
        }
        let mut assigned = HashSet::new();
        ctor.parameters
            .iter()
            .zip(arguments.args.iter())
            .all(|(parameter, argument)| {
                let Some(name) = parameter.name else {
                    return false;
                };
                parameter.this_
                    && !parameter.super_
                    && parameter.kind == ast::ParameterKind::Required
                    && parameter.default_value.is_none()
                    && parameter.function_parameters.is_none()
                    && parameter.function_type_params.is_empty()
                    && fields.get(&name.sym).copied() == self.window_primitive(argument.value)
                    && assigned.insert(name.sym)
            })
            && assigned.len() == fields.len()
    }

    // Symbol names are sufficient only after this whole-window census has
    // excluded shadowing, including parameters already installed in scopes.
    // In particular the bounded for-counter induction below must never confuse
    // two distinct resolved locals which happen to share an interned name.
    fn window_unique_bindings(&self, s: StmtId, names: &mut HashSet<SymbolId>) -> bool {
        let variables = |v: &ast::VariableList, names: &mut HashSet<SymbolId>| {
            v.variables
                .iter()
                .all(|v| self.lookup_local(v.name.sym).is_none() && names.insert(v.name.sym))
        };
        match &self.ast().stmt(s).kind {
            StmtKind::Block(statements) => statements
                .iter()
                .all(|s| self.window_unique_bindings(*s, names)),
            StmtKind::Variables(v) => variables(v, names),
            StmtKind::For { init, body, .. } => {
                !matches!(init, Some(ast::ForInit::Variables(v)) if !variables(v, names))
                    && self.window_unique_bindings(*body, names)
            }
            StmtKind::If { then, else_, .. } => {
                self.window_unique_bindings(*then, names)
                    && else_.is_none_or(|s| self.window_unique_bindings(s, names))
            }
            StmtKind::Empty | StmtKind::Expression(_) => true,
            _ => false,
        }
    }

    fn window_statement(&self, s: StmtId, list: SymbolId, element: &Ty) -> bool {
        match &self.ast().stmt(s).kind {
            StmtKind::Block(statements) => statements
                .iter()
                .all(|s| self.window_statement(*s, list, element)),
            StmtKind::Empty => true,
            StmtKind::Variables(v) => {
                !v.late
                    && !v.external
                    && v.variables.iter().all(|v| {
                        v.name.sym != list
                            && v.initializer
                                .is_some_and(|e| self.window_primitive(e).is_some())
                    })
            }
            StmtKind::Expression(e) => {
                if self.window_primitive(*e).is_some() {
                    return true;
                }
                let ExprKind::Call { target, arguments } = &self.expr(*e).kind else {
                    return false;
                };
                let ExprKind::Property {
                    target: receiver,
                    name,
                    null_aware: false,
                } = &self.expr(*target).kind
                else {
                    return false;
                };
                matches!(self.expr(*receiver).kind,ExprKind::Identifier(n) if n.sym==list)
                    && self.name(name.sym) == "add"
                    && arguments.args.len() == 1
                    && arguments.type_args.is_empty()
                    && arguments.args[0].name.is_none()
                    && self.window_field_constructor(arguments.args[0].value, element)
            }
            StmtKind::For {
                await_: false,
                init,
                condition,
                updates,
                body,
            } => {
                // Shared inference deliberately leaves a write-only ++ operand
                // dynamic. A for-local initialized to int and changed only by
                // primitive writes and unit increments remains an int.
                let counter = match init {
                    Some(ast::ForInit::Variables(v)) if v.variables.len() == 1 => {
                        let variable = &v.variables[0];
                        variable
                            .initializer
                            .filter(|e| self.window_primitive(*e) == Some(Primitive::Int))
                            .map(|_| variable.name.sym)
                    }
                    _ => None,
                };
                let init = match init {
                    None => true,
                    Some(ast::ForInit::Expression(e)) => self.window_primitive(*e).is_some(),
                    Some(ast::ForInit::Variables(v)) => {
                        !v.late
                            && !v.external
                            && v.variables.iter().all(|v| {
                                v.name.sym != list
                                    && v.initializer
                                        .is_some_and(|e| self.window_primitive(e).is_some())
                            })
                    }
                    _ => false,
                };
                init && condition.is_none_or(|e| self.window_primitive(e) == Some(Primitive::Bool))
                    && updates.iter().all(|e| {
                        self.window_primitive(*e).is_some() || counter.is_some_and(|counter| {
                            matches!(self.expr(*e).kind, ExprKind::Unary {op:UnaryOp::PrefixInc | UnaryOp::PostfixInc, operand} if self.window_local(operand) && matches!(self.expr(operand).kind,ExprKind::Identifier(name) if name.sym==counter))
                        })
                    })
                    && self.window_statement(*body, list, element)
            }
            StmtKind::If {
                condition,
                case_pattern: None,
                guard: None,
                then,
                else_,
            } => {
                self.window_primitive(*condition) == Some(Primitive::Bool)
                    && self.window_statement(*then, list, element)
                    && else_.is_none_or(|s| self.window_statement(s, list, element))
            }
            _ => false,
        }
    }

    // The caller has already validated the first fresh-list declaration and
    // terminal return. Reject the entire window on one unsupported operation.
    pub(crate) fn closed_list_window(
        &self,
        middle: &[StmtId],
        list: SymbolId,
        element: &Ty,
    ) -> bool {
        let mut names = HashSet::from([list]);
        middle
            .iter()
            .all(|s| self.window_unique_bindings(*s, &mut names))
            && middle
                .iter()
                .all(|s| self.window_statement(*s, list, element))
    }
}
