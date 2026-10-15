//! Experimental removal of a redundant append cast for a confined fresh list.
//!
//! Proofs are local to a complete synchronous function body. Runtime guards
//! retain the original method, evaluation order, RTI and checked fallback.

use crate::body::{AsyncKind, FnEmitter};
use crate::ctx::Ctx;
use crate::ty::Ty;
use dartforge_frontend::ast::{self, ExprId, ExprKind, StmtId, StmtKind};
use dartforge_intern::SymbolId;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Clone)]
pub(crate) struct FreshList {
    pub token: String,
    pub initializer: ExprId,
    pub element: Ty,
    pub closed_window: bool,
}

#[path = "fresh_list_sdk_sources.rs"]
mod sdk_sources;

pub(crate) fn enabled(ctx: &Ctx) -> bool {
    ctx.production()
        && ctx.sdk.is_some()
        && (std::env::var("DARTFORGE_JSPROD_FRESH_LIST_APPEND").is_ok_and(|v| v == "1")
            || closed_enabled())
}

pub(crate) fn closed_enabled() -> bool {
    std::env::var("DARTFORGE_JSPROD_FRESH_LIST_APPEND_CLOSED").is_ok_and(|v| v == "1")
}

pub(crate) fn certified(ctx: &Ctx) -> bool {
    if !enabled(ctx) {
        return false;
    }
    *ctx.sdk
        .as_ref()
        .unwrap()
        .fresh_list_append_certified
        .get_or_init(|| certified_source(ctx))
}

fn reject(reason: &str) -> bool {
    if std::env::var("DARTFORGE_TRACE_FRESH_LIST_APPEND").is_ok_and(|v| v == "1") {
        eprintln!("fresh-list append: {reason}");
    }
    false
}

fn canonical_source_digest(relative: &str, selected: &str, expected: &str) -> bool {
    if format!("{:x}", Sha256::digest(selected.as_bytes())) == expected {
        return true;
    }
    crate::sdk_patches::original_if_current_patch(relative, selected)
        .is_some_and(|source| format!("{:x}", Sha256::digest(source.as_bytes())) == expected)
}

fn certified_source(ctx: &Ctx) -> bool {
    let mut seen = std::collections::HashSet::new();
    for unit in &ctx.program.units {
        let library = ctx.program.library(unit.library);
        if !library.uri.starts_with("dart:") {
            continue;
        }
        let Some(path) = unit.path.as_ref() else {
            return reject(&format!("SDK unit without path: {}", unit.uri));
        };
        let path = path.to_string_lossy().replace('\\', "/");
        let Some((relative, digest, owner)) = sdk_sources::SOURCES
            .iter()
            .find(|(relative, _, _)| path.ends_with(&format!("/{relative}")))
        else {
            return reject(&format!("unrecognized SDK unit path: {path}"));
        };
        if *owner != Some(library.uri.as_str())
            || !seen.insert(*relative)
            || !canonical_source_digest(relative, &unit.source, digest)
        {
            return reject(&format!(
                "SDK source identity mismatch: {relative}, owner={} expected={owner:?}",
                library.uri
            ));
        }
    }
    if !seen.contains("_internal/js_dev_runtime/private/js_array.dart") {
        return reject("selected SDK or foreign-code premise rejected");
    }
    // No foreign code may mutate private SDK metadata while the proof is live.
    if ctx.program.functions.iter().enumerate().any(|(i, f)| {
        !ctx.program.library(f.library).uri.starts_with("dart:")
            && (f.external
                || ctx.is_js_member(dartforge_elements::model::FunctionElementId(i as u32)))
    }) {
        return reject("selected SDK or foreign-code premise rejected");
    }
    for library in &ctx.program.libraries {
        if library.uri.starts_with("dart:") {
            continue;
        }
        if library.imports.iter().any(|import| {
            let uri = &ctx.program.library(import.library).uri;
            uri.starts_with("dart:_")
                || matches!(
                    uri.as_str(),
                    "dart:js"
                        | "dart:js_util"
                        | "dart:js_interop"
                        | "dart:js_interop_unsafe"
                        | "dart:html"
                )
        }) {
            return reject("selected SDK or foreign-code premise rejected");
        }
    }
    if ctx
        .js_libs
        .keys()
        .any(|l| !ctx.program.library(*l).uri.starts_with("dart:"))
    {
        return reject("selected SDK or foreign-code premise rejected");
    }
    let Some(array) = ctx.js_array else {
        return reject("selected SDK or foreign-code premise rejected");
    };
    let c = ctx.program.class(array);
    if ctx.program.library(c.library).uri != "dart:_interceptors" {
        return reject("selected SDK or foreign-code premise rejected");
    }
    if !c.mixins.is_empty() || ctx.program.augmentations.contains_key(&array) {
        return reject("selected SDK or foreign-code premise rejected");
    }
    for name in ["add", "checkGrowable"] {
        let Some(fid) = ctx
            .sym(name)
            .and_then(|s| c.instance_members.get(&s))
            .copied()
        else {
            return reject("selected SDK or foreign-code premise rejected");
        };
        if ctx.fn_state(fid) != crate::filter::State::Live {
            return reject("selected SDK or foreign-code premise rejected");
        }
    }
    let Some(fid) = ctx.sym("of").and_then(|s| c.constructors.get(&s)).copied() else {
        return reject("selected SDK or foreign-code premise rejected");
    };
    let valid = ctx.fn_state(fid) == crate::filter::State::Live;
    if !valid {
        reject(&format!("SDK factory of state {:?}", ctx.fn_state(fid)));
    }
    valid
}

pub(crate) fn helper_name(ctx: &Ctx) -> String {
    format!("_interceptors.t$freshListAppend{}", ctx.js_array.unwrap().0)
}

impl FnEmitter<'_, '_> {
    fn fresh_name(&self, e: ExprId, name: SymbolId) -> bool {
        matches!(self.expr(e).kind, ExprKind::Identifier(n) if n.sym == name)
    }
    fn fresh_expression(&self, e: ExprId, name: SymbolId, calls: &mut Vec<ExprId>) -> bool {
        match &self.expr(e).kind {
            ExprKind::Identifier(n) => n.sym != name,
            ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::This
            | ExprKind::Super
            | ExprKind::Symbol(_) => true,
            ExprKind::String(s) => s.parts.iter().all(|p| match p {
                ast::StringPart::Text(_) => true,
                ast::StringPart::Interpolation(e) => self.fresh_expression(*e, name, calls),
            }),
            ExprKind::Parenthesized(e)
            | ExprKind::As { value: e, .. }
            | ExprKind::Is { value: e, .. }
            | ExprKind::Property { target: e, .. }
            | ExprKind::TypeArguments { target: e, .. }
            | ExprKind::Unary { operand: e, .. } => self.fresh_expression(*e, name, calls),
            ExprKind::Binary { left, right, .. }
            | ExprKind::Assign {
                target: left,
                value: right,
                ..
            } => {
                self.fresh_expression(*left, name, calls)
                    && self.fresh_expression(*right, name, calls)
            }
            ExprKind::Index { target, index, .. } => {
                self.fresh_expression(*target, name, calls)
                    && self.fresh_expression(*index, name, calls)
            }
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } => [*condition, *then, *else_]
                .into_iter()
                .all(|e| self.fresh_expression(e, name, calls)),
            ExprKind::Call { target, arguments } => {
                if let ExprKind::Property {
                    target: receiver,
                    name: member,
                    null_aware: false,
                } = &self.expr(*target).kind
                {
                    if self.fresh_name(*receiver, name) {
                        if self.name(member.sym) != "add"
                            || arguments.args.len() != 1
                            || !arguments.type_args.is_empty()
                            || arguments.args[0].name.is_some()
                        {
                            return false;
                        }
                        calls.push(*target);
                        return self.fresh_expression(arguments.args[0].value, name, calls);
                    }
                }
                self.fresh_expression(*target, name, calls)
                    && arguments
                        .args
                        .iter()
                        .all(|a| self.fresh_expression(a.value, name, calls))
            }
            ExprKind::InstanceCreation { arguments, .. } => arguments
                .args
                .iter()
                .all(|a| self.fresh_expression(a.value, name, calls)),
            _ => false,
        }
    }
    fn fresh_statement(&self, s: StmtId, name: SymbolId, calls: &mut Vec<ExprId>) -> bool {
        match &self.ast().stmt(s).kind {
            StmtKind::Block(ss) => ss.iter().all(|s| self.fresh_statement(*s, name, calls)),
            StmtKind::Expression(e) => self.fresh_expression(*e, name, calls),
            StmtKind::Variables(v) => v.variables.iter().all(|v| {
                v.name.sym != name
                    && v.initializer
                        .is_none_or(|e| self.fresh_expression(e, name, calls))
            }),
            StmtKind::For {
                await_: false,
                init,
                condition,
                updates,
                body,
            } => {
                let init_ok = match init {
                    None => true,
                    Some(ast::ForInit::Expression(e)) => self.fresh_expression(*e, name, calls),
                    Some(ast::ForInit::Variables(v)) => v.variables.iter().all(|v| {
                        v.name.sym != name
                            && v.initializer
                                .is_none_or(|e| self.fresh_expression(e, name, calls))
                    }),
                    _ => false,
                };
                init_ok
                    && condition.is_none_or(|e| self.fresh_expression(e, name, calls))
                    && updates
                        .iter()
                        .all(|e| self.fresh_expression(*e, name, calls))
                    && self.fresh_statement(*body, name, calls)
            }
            StmtKind::If {
                condition,
                case_pattern: None,
                guard: None,
                then,
                else_,
            } => {
                self.fresh_expression(*condition, name, calls)
                    && self.fresh_statement(*then, name, calls)
                    && else_.is_none_or(|s| self.fresh_statement(s, name, calls))
            }
            StmtKind::Empty => true,
            _ => false,
        }
    }
    pub(crate) fn fresh_lists(&mut self, statements: &[StmtId]) -> HashMap<ExprId, FreshList> {
        let mut out = HashMap::new();
        if closed_enabled()
            && std::env::var("DARTFORGE_TRACE_FRESH_LIST_APPEND").is_ok_and(|v| v == "1")
            && !self.ctx.program.library(self.lib).uri.starts_with("dart:")
        {
            eprintln!(
                "closed-list candidate: production={} supported={} async={:?} first={:?}",
                self.ctx.production(),
                certified(self.ctx),
                self.async_kind,
                statements
                    .first()
                    .map(|s| self.text(self.ast().stmt(*s).span))
            );
        }
        if !certified(self.ctx)
            || self.async_kind != AsyncKind::None
            || self.ctx.program.library(self.lib).uri.starts_with("dart:")
            || statements.len() < 2
        {
            return out;
        }
        let StmtKind::Variables(v) = &self.ast().stmt(statements[0]).kind else {
            return out;
        };
        let [variable] = v.variables.as_ref() else {
            return out;
        };
        if v.late || v.const_ || v.external {
            return out;
        }
        let Some(initializer) = variable.initializer else {
            return out;
        };
        let ExprKind::List {
            const_: false,
            type_args,
            elements,
        } = &self.expr(initializer).kind
        else {
            return out;
        };
        if !elements.is_empty() || type_args.len() != 1 {
            return out;
        }
        let element = self.resolve_type(type_args[0]);
        if closed_enabled() && std::env::var("DARTFORGE_TRACE_FRESH_LIST_APPEND").is_ok_and(|v|v=="1") { eprintln!("closed-list element: {:?}",element); }

        let Ty::Iface {
            class,
            args,
            nullable: false,
        } = &element
        else {
            return out;
        };
        if !args.is_empty()
            || self
                .ctx
                .program
                .library(self.ctx.program.class(*class).library)
                .uri
                .starts_with("dart:")
            || crate::own_sdk::is_native_class(self.ctx, *class)
        {
            return out;
        }
        let StmtKind::Return(Some(value)) = self.ast().stmt(*statements.last().unwrap()).kind
        else {
            return out;
        };
        if !self.fresh_name(value, variable.name.sym) {
            return out;
        }
        let mut calls = Vec::new();
        if !statements[1..statements.len() - 1]
            .iter()
            .all(|s| self.fresh_statement(*s, variable.name.sym, &mut calls))
            || calls.is_empty()
        {
            return out;
        }
        if closed_enabled() && std::env::var("DARTFORGE_TRACE_FRESH_LIST_APPEND").is_ok_and(|v|v=="1") { eprintln!("closed-list census passed: {} calls",calls.len()); }
        let closed_window = closed_enabled();
        if closed_window
            && !self.closed_list_window(
                &statements[1..statements.len() - 1],
                variable.name.sym,
                &element,
            )
        {
            return out;
        }
        let fact = FreshList {
            token: self.temp(),
            initializer,
            element,
            closed_window,
        };
        out.insert(initializer, fact.clone());
        for call in calls {
            out.insert(call, fact.clone());
        }
        out
    }
}

impl FnEmitter<'_, '_> {
    pub(crate) fn fresh_append_fact(
        &self,
        node: Option<ExprId>,
        recv: &Ty,
        arguments: &ast::Arguments,
    ) -> Option<FreshList> {
        let fact = self.fresh_list_append.get(&node?).cloned()?;
        if arguments.args.len() != 1
            || !arguments.type_args.is_empty()
            || arguments.args[0].name.is_some()
            || recv != &self.ctx.t_list(fact.element.clone())
        {
            return None;
        }
        let value = arguments.args[0].value;
        let resolution = self
            .common_resolution(value)
            .or_else(|| match self.expr(value).kind {
                ExprKind::Call { target, .. } => self.common_resolution(target),
                _ => None,
            })?;
        let dartforge_types::resolved::Resolved::Constructor(fid) = resolution else {
            return None;
        };
        let constructor = self.ctx.program.function(*fid);
        if constructor.factory || constructor.external || constructor.patched_by.is_some() {
            return None;
        }
        let class = constructor.class?;
        let c = self.ctx.program.class(class);
        if !c.type_params.is_empty()
            || self.ctx.program.library(c.library).uri.starts_with("dart:")
            || crate::own_sdk::is_native_class(self.ctx, class)
        {
            return None;
        }
        if !self.ctx.is_subtype(&Ty::iface(class), &fact.element) {
            return None;
        }
        Some(fact)
    }
}

/// Emit after the SDK's canonical extension methods have been installed.
pub(crate) fn emit_helper(
    ctx: &Ctx,
    m: &crate::module::ModState,
    class: dartforge_elements::model::ClassId,
    cref: &str,
) -> String {
    if ctx.js_array != Some(class) || !certified(ctx) {
        return String::new();
    }
    let name = helper_name(ctx);
    let add = m.dartx("add");
    let grow = m.dartx("checkGrowable");
    format!(
        r#"
{name} = ((owner, addKey, growKey) => {{
  const apply = Reflect.apply, descriptor = Object.getOwnPropertyDescriptor;
  const hasOwn = Object.prototype.hasOwnProperty;
  const symbols = Object.getOwnPropertySymbols, prototype = Object.getPrototypeOf;
  const arrayPrototype = Array.prototype;
  const originalFactory = owner.of, originalAdd = owner.prototype[addKey];
  const data = (object, key) => {{
    const d = descriptor(object, key);
    return d && apply(hasOwn, d, ['value']) ? d : null;
  }};
  const ready = (token, list) => token.valid && token.list === list &&
    prototype(list) === arrayPrototype && !descriptor(list, addKey) &&
    data(arrayPrototype, addKey)?.value === originalAdd;
  return {{
    create(factory, receiver, rti, array) {{
      const list = apply(factory, receiver, [rti, array]);
      const token = {{list, rti, key: null, valid: false}};
      if (factory === originalFactory && list === array && prototype(array) === arrayPrototype) {{
        const keys = symbols(array);
        if (keys.length === 1 && data(array, keys[0])?.value === rti) {{
          token.key = keys[0]; token.valid = true;
          token.valid = ready(token, array);
        }}
      }}
      return token;
    }},
    ready,
    // This entry is emitted only for a certified closed source window.
    // External JS mutation of compiler-private bindings is outside that proof.
    appendClosed(token, list, callee, value) {{
      if (!token.valid || callee !== originalAdd) return apply(callee, list, [value]);
      list[growKey]('add');
      list.push(value);
    }},
    append(token, list, callee, before, value) {{
      if (!before || !ready(token, list) || callee !== originalAdd || data(list, token.key)?.value !== token.rti)
        return apply(callee, list, [value]);
      list[growKey]('add');
      list.push(value);
    }}
  }};
}})({cref}, {add}, {grow});
"#
    )
}
