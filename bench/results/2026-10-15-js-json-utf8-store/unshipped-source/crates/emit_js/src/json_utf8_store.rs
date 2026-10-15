//! Experimental checked native store at the SDK JSON UTF8 writer's private site.
//!
//! The selected SDK source closure is byte-certified, including the unmodified
//! lifecycle owner and native typed-array implementation. The writer and its
//! only pretty-print subclass never publish `this`. `buffer` is assigned only a
//! fresh Uint8List in construction, after a full-chunk callback, and at terminal
//! flush. Callbacks can retain/detach old chunks, but the next store uses a fresh
//! allocation made after that callback returns. Reentrant encoding has a separate
//! writer. No source/foreign-code escape means the live buffer stays exact native
//! Uint8 storage. This proves its physical access, not bounds: the canonical
//! bounds/error function and original evaluation order remain at every store.

use crate::{
    body::FnEmitter,
    ctx::Ctx,
    js::{Js, P_COMMA},
    ty::Ty,
};
use dartforge_elements::model::{ClassId, Element, FunctionElementId, FunctionRef, UnitId};
use dartforge_frontend::ast::{AssignOp, ExprId, ExprKind, FunctionBody, StmtKind, UnaryOp};

pub(crate) struct Certificate {
    unit: UnitId,
    class: ClassId,
    target: ExprId,
    buffer_type: Ty,
    check: FunctionElementId,
}

fn enabled(ctx: &Ctx) -> bool {
    ctx.production()
        && ctx.sdk.is_some()
        && std::env::var("DARTFORGE_JSPROD_JSON_UTF8_STORE").is_ok_and(|v| v == "1")
}

fn certify(ctx: &Ctx) -> Option<Certificate> {
    if !crate::fresh_list_append::canonical_selected_sdk(ctx) {
        return None;
    }
    // A future compiler patch on either lifecycle unit needs its own audit;
    // reverse-normalization alone is deliberately insufficient here.
    for relative in [
        "convert/json.dart",
        "_internal/js_dev_runtime/private/native_typed_data.dart",
    ] {
        let selected = ctx
            .program
            .units
            .iter()
            .filter(|u| {
                u.path.as_ref().is_some_and(|p| {
                    p.to_string_lossy()
                        .replace('\\', "/")
                        .ends_with(&format!("/{relative}"))
                })
            })
            .collect::<Vec<_>>();
        if selected.len() != 1
            || !crate::fresh_list_append::raw_canonical_unit(relative, &selected[0].source)
        {
            return None;
        }
    }
    let classes = ctx
        .program
        .classes
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            ctx.program.library(c.library).uri == "dart:convert"
                && ctx.name(c.name) == "_JsonUtf8Stringifier"
        })
        .collect::<Vec<_>>();
    let [(id, c)] = classes.as_slice() else {
        return None;
    };
    let class = ClassId(*id as u32);
    if ctx.program.class_with_homonym(class) || ctx.program.augmentations.contains_key(&class) {
        return None;
    }
    let fid = *c.instance_members.get(&ctx.sym("writeByte")?)?;
    let f = ctx.program.function(fid);
    if f.class != Some(class)
        || f.external
        || f.patched_by.is_some()
        || ctx.fn_state(fid) != crate::filter::State::Live
    {
        return None;
    }
    let FunctionRef::Function { unit, function } = f.node else {
        return None;
    };
    let source = ctx.program.unit(unit);
    let FunctionBody::Block(body) = source.ast.function(function).body else {
        return None;
    };
    let StmtKind::Block(statements) = &source.ast.stmt(body).kind else {
        return None;
    };
    let StmtKind::Expression(store) = source.ast.stmt(*statements.last()?).kind else {
        return None;
    };
    let ExprKind::Assign {
        op: AssignOp::Assign,
        target,
        value,
    } = source.ast.expr(store).kind
    else {
        return None;
    };
    let ExprKind::Index {
        target: receiver,
        index,
        null_aware: false,
    } = source.ast.expr(target).kind
    else {
        return None;
    };
    let ExprKind::Unary {
        op: UnaryOp::PostfixInc,
        operand,
    } = source.ast.expr(index).kind
    else {
        return None;
    };
    let named = |e, expected| matches!(source.ast.expr(e).kind, ExprKind::Identifier(n) if ctx.name(n.sym)==expected);
    if !named(receiver, "buffer") || !named(operand, "index") || !named(value, "byte") {
        return None;
    }
    // The actual AST target must still be the audited source expression.
    let span = source.ast.expr(store).span;
    if &source.source[span.start..span.end] != "buffer[index++] = byte" {
        return None;
    }
    let variables = ctx
        .program
        .variables
        .iter()
        .enumerate()
        .filter(|(_, v)| v.class == Some(class) && ctx.name(v.name) == "buffer")
        .collect::<Vec<_>>();
    let [(variable, _)] = variables.as_slice() else {
        return None;
    };
    let buffer_type = ctx.var_ty(dartforge_elements::model::VariableId(*variable as u32));
    let Ty::Iface {
        class: buffer_class,
        args,
        nullable: false,
    } = &buffer_type
    else {
        return None;
    };
    if !args.is_empty()
        || ctx.name(ctx.program.class(*buffer_class).name) != "Uint8List"
        || ctx
            .program
            .library(ctx.program.class(*buffer_class).library)
            .uri
            != "dart:typed_data"
    {
        return None;
    }
    let checks = ctx
        .program
        .functions
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            f.class.is_none()
                && ctx.name(f.name) == "_checkValidIndex"
                && ctx.program.library(f.library).uri == "dart:_native_typed_data"
        })
        .collect::<Vec<_>>();
    let [(id, check)] = checks.as_slice() else {
        return None;
    };
    let check_id = FunctionElementId(*id as u32);
    if check.external
        || check.patched_by.is_some()
        || ctx.fn_state(check_id) != crate::filter::State::Live
    {
        return None;
    }
    Some(Certificate {
        unit,
        class,
        target,
        buffer_type,
        check: check_id,
    })
}

impl FnEmitter<'_, '_> {
    pub(crate) fn emit_json_utf8_store(
        &mut self,
        target: ExprId,
        receiver: &Js,
        receiver_type: &Ty,
        index: &Js,
        value: &Js,
    ) -> Option<Js> {
        if !enabled(self.ctx) {
            return None;
        }
        let certificate = self
            .ctx
            .sdk
            .as_ref()?
            .json_utf8_store
            .get_or_init(|| certify(self.ctx))
            .as_ref()?;
        if self.unit != certificate.unit
            || self.class != Some(certificate.class)
            || target != certificate.target
            || receiver_type != &certificate.buffer_type
        {
            return None;
        }
        let check = self.element_ref(Element::Function(certificate.check))?.0;
        let list = self.temp();
        let idx = self.temp();
        let byte = self.temp();
        // The receiver is captured before index++, which is captured before
        // the RHS; an exception keeps those effects and prevents the store.
        Some(Js::new(format!("/* certified JSON UTF8 store */ {list} = {}, {idx} = {}, {byte} = {}, {check}({idx}, {list}, {list}.length), {list}[{idx}] = {byte}, {byte}",receiver.code,index.code,value.code),P_COMMA).paren())
    }
}
