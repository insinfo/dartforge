//! Selected-source premise for the experimental primitive `clamp(0, 1)` pass.
use crate::context::Context;
use dartforge_elements::model::{Element, FunctionKind, FunctionRef};
use dartforge_frontend::ast::ParameterKind;
use std::collections::HashSet;

const INTEGER: &str = include_str!("../../../../native_sdk/core/integers.dart");
const DOUBLE: &str = include_str!("../../../../native_sdk/core/double.dart");

macro_rules! reject {
    ($($reason:tt)*) => {{
        #[cfg(test)] eprintln!($($reason)*);
        return false;
    }};
}

/// This premise does not certify mutable dispatch tables. The consuming pass
/// independently audits the complete lowered module after reachability pruning.
pub(super) fn certify(ctx: &Context) -> bool {
    if !super::double_identity::reviewed_selected_sources(ctx) {
        reject!("clamp source fingerprint/native hash binding rejected");
    }
    let Some(core) = ctx.program.core else {
        reject!("clamp selected core is absent");
    };
    if !ctx.compiled_library(core) {
        reject!("clamp selected core not compiled");
    }
    for (owner, source, physical, methods) in [
        (
            "_IntegerImplementation",
            INTEGER,
            None,
            &["clamp", "compareTo"][..],
        ),
        (
            "_Smi",
            INTEGER,
            Some(dartforge_runtime::layout::cid::SMI),
            &[][..],
        ),
        (
            "_Mint",
            INTEGER,
            Some(dartforge_runtime::layout::cid::MINT),
            &[][..],
        ),
        (
            "_Double",
            DOUBLE,
            Some(dartforge_runtime::layout::cid::DOUBLE),
            &["clamp", "compareTo"][..],
        ),
    ] {
        let Some(name) = ctx.interner.lookup(owner) else {
            reject!("clamp owner name absent: {owner}");
        };
        let Some(Element::Class(cid)) = ctx.program.lookup(core, name).and_then(|e| e.getter)
        else {
            reject!("clamp selected owner lookup rejected: {owner}");
        };
        let class = ctx.program.class(cid);
        if class.library != core
            || ctx.program.class_with_homonym(cid)
            || !class.mixins.is_empty()
            || !class.mixin_classes.is_empty()
            || !class.shadowed_instance_members.is_empty()
            || class
                .decl
                .is_none_or(|decl| ctx.program.unit(decl.unit).source != source)
            || physical.is_some_and(|expected| ctx.class_id_of(cid) != Some(expected as u32))
        {
            reject!(
                "clamp class rejected: owner={owner} homonym={} physical={:?} mixes={} shadows={} source_match={}",
                ctx.program.class_with_homonym(cid),
                ctx.class_id_of(cid),
                class.mixins.len() + class.mixin_classes.len(),
                class.shadowed_instance_members.len(),
                class
                    .decl
                    .is_some_and(|decl| ctx.program.unit(decl.unit).source == source)
            );
        }
        for method in methods {
            let Some(name) = ctx.interner.lookup(method) else {
                reject!("clamp method name absent: {owner}.{method}");
            };
            let Some(&id) = class.instance_members.get(&name) else {
                reject!("clamp member absent: {owner}.{method}");
            };
            let function = ctx.program.function(id);
            let FunctionRef::Function { unit, .. } = function.node else {
                reject!("clamp method body declaration rejected: {owner}.{method}");
            };
            let signature = &ctx.outline.functions[id.0 as usize];
            let (count, ret) = if *method == "clamp" {
                (2, ctx.core.num)
            } else {
                (1, ctx.core.int)
            };
            if function.library != core
                || function.class != Some(cid)
                || function.name != name
                || function.kind != FunctionKind::Function
                || function.static_
                || function.external
                || function.patched_by.is_some()
                || ctx.program.unit(unit).source != source
                || signature.return_type != ret
                || !signature.type_params.is_empty()
                || signature.parameters.len() != count
                || signature
                    .parameters
                    .iter()
                    .any(|p| p.ty != ctx.core.num || p.kind != ParameterKind::Required)
            {
                reject!(
                    "clamp signature/body rejected: {owner}.{method} kind={:?} static={} external={} patched={} source_match={} return={:?} expected={ret:?} params={:?}",
                    function.kind,
                    function.static_,
                    function.external,
                    function.patched_by.is_some(),
                    ctx.program.unit(unit).source == source,
                    signature.return_type,
                    signature.parameters
                );
            }
        }
    }
    let body_symbols = targets(ctx);
    if body_symbols.len() != 2
        || body_symbols.iter().any(|symbol| {
            ctx.program
                .functions
                .iter()
                .enumerate()
                .filter(|(index, _)| super::symbol_of(ctx, *index) == *symbol)
                .count()
                != 1
        })
    {
        reject!("clamp canonical body symbols not unique: {body_symbols:?}");
    }
    // Reject extra aliases, including aliases declared in an ordinary program.
    // A source fingerprint alone does not bind native declarations outside core.
    let required = [
        "Double_equal",
        "Double_equalToInteger",
        "Double_greaterThan",
        "Double_greaterThanFromInteger",
        "Double_getIsNegative",
        "Double_getIsNaN",
        "Double_doubleFromInteger",
        "Integer_equalToInteger",
        "Integer_greaterThanFromInteger",
    ];
    let mut observed = HashSet::new();
    for function in &ctx.program.functions {
        if !function.external {
            continue;
        }
        let Some(native) = crate::natives::pragmas(ctx.program, ctx.interner, function).0 else {
            continue;
        };
        // Foreign runtime-native aliases can expose resolver/registration ABI
        // without a normal dart:ffi NativeCall. Keep this experimental proof
        // closed to them, including aliases unrelated to numeric comparisons.
        if !ctx.program.library(function.library).is_sdk {
            reject!("clamp foreign runtime-native alias exposure: {native}");
        }
        // These are the transitive comparison/conversion bindings used for
        // the proved literal range. Other Integer_/Double_ natives, including
        // int.fromEnvironment, are not members of this call closure.
        if !required.contains(&native.as_str()) {
            continue;
        }
        let expected = if native.starts_with("Double_") {
            ("_Double", DOUBLE)
        } else {
            ("_IntegerImplementation", INTEGER)
        };
        let Some(owner) = function.class else {
            reject!("clamp native alias has no owner: {native}");
        };
        let unit = match function.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => unit,
            FunctionRef::None => reject!("clamp native alias has no body declaration: {native}"),
        };
        if function.library != core
            || ctx.symbol_name(ctx.program.class(owner).name) != expected.0
            || function.patched_by.is_some()
            || ctx.program.unit(unit).source != expected.1
            || !observed.insert(native.clone())
        {
            reject!(
                "clamp native alias census rejected: {native} owner={} expected={} patched={} source_match={}",
                ctx.symbol_name(ctx.program.class(owner).name),
                expected.0,
                function.patched_by.is_some(),
                ctx.program.unit(unit).source == expected.1
            );
        }
        if required.contains(&native.as_str())
            && crate::natives::native(&native)
                .is_none_or(|n| n.state != crate::natives::State::Runtime)
        {
            reject!("clamp native ABI lacks reviewed runtime implementation: {native}");
        }
    }
    #[cfg(test)]
    if !required.iter().all(|name| observed.contains(*name)) {
        eprintln!(
            "clamp native bindings missing: {:?}",
            required
                .iter()
                .filter(|name| !observed.contains(**name))
                .collect::<Vec<_>>()
        );
    }
    required.iter().all(|name| observed.contains(*name))
}

pub(super) fn enabled_certificate(ctx: &Context) -> bool {
    !ctx.sdk_library
        && !ctx.arc_memory
        && ctx.primitive_clamp_enabled
        && std::env::var("DARTFORGE_NO_PRIMITIVE_CLAMP").as_deref() != Ok("1")
        && certify(ctx)
}

pub(super) fn targets(ctx: &Context) -> HashSet<String> {
    let Some(core) = ctx.program.core else {
        return HashSet::new();
    };
    let Some(clamp) = ctx.interner.lookup("clamp") else {
        return HashSet::new();
    };
    ["_IntegerImplementation", "_Double"]
        .into_iter()
        .filter_map(|owner| {
            let name = ctx.interner.lookup(owner)?;
            let Element::Class(class) = ctx.program.lookup(core, name)?.getter? else {
                return None;
            };
            let function = *ctx.program.class(class).instance_members.get(&clamp)?;
            Some(super::symbol_of(ctx, function.0 as usize))
        })
        .collect()
}

#[cfg(test)]
mod tests;
