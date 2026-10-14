//! Conservative certificate for reusing a checked primitive double box.
use crate::context::Context;
use dartforge_elements::model::{Element, FunctionElementId, FunctionKind, FunctionRef};
use std::collections::HashSet;

mod selected_sdk;

const DOUBLE: &str = include_str!("../../../../native_sdk/core/double.dart");
const IDENTICAL: &str = include_str!("../../../../native_sdk/core/identical_patch.dart");

pub(super) fn certified(ctx: &Context) -> bool {
    if !selected_sdk::trusted_sources(ctx) || !selected_sdk::canonical_hash_binding(ctx) {
        return false;
    }
    let Some(core) = ctx.program.core else {
        return false;
    };
    let library = ctx.program.library(core);
    if library.uri != "dart:core" || !library.is_sdk {
        return false;
    }
    let selected = |id: FunctionElementId, owner, name, kind, expected: &str| {
        let mut id = id;
        let mut seen = HashSet::new();
        loop {
            if !seen.insert(id) {
                return false;
            }
            let f = ctx.program.function(id);
            if f.library != core
                || f.class != owner
                || f.name != name
                || f.kind != kind
                || f.variable.is_some()
            {
                return false;
            }
            if let Some(patch) = f.patched_by {
                id = patch;
                continue;
            }
            let FunctionRef::Function { unit, .. } = f.node else {
                return false;
            };
            let unit = ctx.program.unit(unit);
            return unit.library == core && unit.source == expected;
        }
    };
    let Some(name) = ctx.interner.lookup("_Double") else {
        return false;
    };
    let Some(Element::Class(cid)) = ctx.program.lookup(core, name).and_then(|e| e.getter) else {
        return false;
    };
    let class = ctx.program.class(cid);
    let Some(decl) = class.decl else {
        return false;
    };
    if class.library != core
        || ctx.program.unit(decl.unit).source != DOUBLE
        || !class.mixins.is_empty()
        || !class.mixin_classes.is_empty()
        || !class.shadowed_instance_members.is_empty()
    {
        return false;
    }
    for name in ["hashCode", "_identityHashCode"] {
        let Some(symbol) = ctx.interner.lookup(name) else {
            return false;
        };
        let Some(&f) = class.instance_members.get(&symbol) else {
            return false;
        };
        if !selected(f, Some(cid), symbol, FunctionKind::Getter, DOUBLE) {
            return false;
        }
    }
    for name in ["identical", "identityHashCode"] {
        let Some(symbol) = ctx.interner.lookup(name) else {
            return false;
        };
        let Some(Element::Function(f)) = ctx.program.lookup(core, symbol).and_then(|e| e.getter)
        else {
            return false;
        };
        if !selected(f, None, symbol, FunctionKind::Function, IDENTICAL) {
            return false;
        }
    }
    crate::natives::native("Identical_comparison")
        .is_some_and(|n| n.state == crate::natives::State::Runtime)
}

#[cfg(test)]
mod tests;
