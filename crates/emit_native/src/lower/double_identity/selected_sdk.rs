//! Bind identity observations to reviewed selected SDK sources and part structure.
use crate::context::Context;
use dartforge_elements::model::{FunctionKind, FunctionRef, UnitRole};
use dartforge_frontend::ast::DirectiveKind;
use std::collections::{HashMap, HashSet};

fn role(role: UnitRole) -> Option<u8> {
    match role {
        UnitRole::Library => Some(1),
        UnitRole::Part => Some(2),
        UnitRole::Patch => Some(3),
        UnitRole::Augmentation => None,
    }
}

fn bytes(hash: &mut blake3::Hasher, value: &[u8]) {
    hash.update(&(value.len() as u64).to_le_bytes());
    hash.update(value);
}

pub(super) fn core_fingerprint(ctx: &Context) -> Option<String> {
    let core = ctx.program.core?;
    let library = ctx.program.library(core);
    if library.uri != "dart:core" || !library.is_sdk || library.units.is_empty() {
        return None;
    }
    let mut ordinals = HashMap::new();
    for (index, id) in library.units.iter().enumerate() {
        if ordinals.insert(*id, index).is_some() {
            return None;
        }
        let unit = ctx.program.units.get(id.0 as usize)?;
        if unit.library != core || !unit.skipped.is_empty() {
            return None;
        }
        role(unit.role)?;
    }
    if ctx
        .program
        .units
        .iter()
        .filter(|u| u.library == core)
        .count()
        != library.units.len()
    {
        return None;
    }
    let first = ctx.program.unit(library.units[0]);
    if first.role != UnitRole::Library || first.uri != "dart:core" {
        return None;
    }
    let mut parents = HashMap::new();
    for (parent, id) in library.units.iter().enumerate() {
        let unit = ctx.program.unit(*id);
        for directive in &unit.unit.directives {
            let DirectiveKind::Part { uri } = &directive.kind else {
                if matches!(
                    directive.kind,
                    DirectiveKind::ImportAugment { .. } | DirectiveKind::AugmentLibrary { .. }
                ) {
                    return None;
                }
                continue;
            };
            let relative = dartforge_elements::load::string_lit_value(uri)?;
            let path = unit.path.as_ref()?.parent()?.join(relative);
            let mut normalized = std::path::PathBuf::new();
            for component in path.components() {
                match component {
                    std::path::Component::CurDir => {}
                    std::path::Component::ParentDir => {
                        if !normalized.pop() {
                            return None;
                        }
                    }
                    component => normalized.push(component.as_os_str()),
                }
            }
            let path = normalized;
            let mut matches = library.units.iter().enumerate().filter(|(_, child)| {
                ctx.program.unit(**child).path.as_deref() == Some(path.as_path())
            });
            let (child, child_id) = matches.next()?;
            if matches.next().is_some()
                || child <= parent
                || parents.insert(*child_id, parent).is_some()
            {
                return None;
            }
            let expected = if unit.role == UnitRole::Patch {
                UnitRole::Patch
            } else {
                UnitRole::Part
            };
            if ctx.program.unit(*child_id).role != expected {
                return None;
            }
        }
    }
    for (child, parent) in &library.unit_parents {
        if parents.get(child) != Some(ordinals.get(parent)?) {
            return None;
        }
    }
    let mut hash = blake3::Hasher::new();
    bytes(&mut hash, b"dartforge.checked-double.core.v1");
    bytes(&mut hash, library.uri.as_bytes());
    hash.update(&(library.units.len() as u64).to_le_bytes());
    for (index, id) in library.units.iter().enumerate() {
        let unit = ctx.program.unit(*id);
        let parent = if let Some(parent) = parents.get(id) {
            *parent as u64
        } else {
            if index != 0
                && !(unit.role == UnitRole::Patch && unit.uri.starts_with("dart:core#patch_"))
            {
                return None;
            }
            if index != 0
                && unit
                    .unit
                    .directives
                    .iter()
                    .any(|d| matches!(d.kind, DirectiveKind::PartOf { .. }))
            {
                return None;
            }
            u64::MAX
        };
        hash.update(&[role(unit.role)?]);
        hash.update(&parent.to_le_bytes());
        bytes(&mut hash, unit.source.as_bytes());
    }
    Some(hash.finalize().to_hex().to_string())
}

/// Every loaded SDK source must match its reviewed semantic library and role.
pub(super) fn trusted_sources(ctx: &Context) -> bool {
    let Ok(manifest) =
        serde_json::from_str::<serde_json::Value>(include_str!("selected_sdk_manifest.json"))
    else {
        return false;
    };
    let Some(expected_core) = manifest["core_fingerprint"].as_str() else {
        return false;
    };
    if core_fingerprint(ctx).as_deref() != Some(expected_core) {
        return false;
    }
    let Some(entries) = manifest["units"].as_array() else {
        return false;
    };
    let trusted: HashSet<_> = entries
        .iter()
        .filter_map(|e| {
            Some((
                e["library"].as_str()?,
                e["role"].as_u64()? as u8,
                e["source_blake3"].as_str()?,
            ))
        })
        .collect();
    for unit in &ctx.program.units {
        let library = ctx.program.library(unit.library);
        if library.is_sdk != library.uri.starts_with("dart:") {
            return false;
        }
        if !library.is_sdk {
            continue;
        }
        if !unit.skipped.is_empty() {
            return false;
        }
        let Some(role) = role(unit.role) else {
            return false;
        };
        let digest = blake3::hash(unit.source.as_bytes()).to_hex().to_string();
        if !trusted.contains(&(library.uri.as_str(), role, digest.as_str())) {
            return false;
        }
    }
    true
}

/// Reject aliases of the header identity primitive outside its canonical helper.
pub(super) fn canonical_hash_binding(ctx: &Context) -> bool {
    let Some(core) = ctx.program.core else {
        return false;
    };
    let Some(name) = ctx.interner.lookup("_getHash") else {
        return false;
    };
    let Some(dartforge_elements::model::Element::Function(canonical)) =
        ctx.program.lookup(core, name).and_then(|e| e.getter)
    else {
        return false;
    };
    let mut count = 0;
    for (index, function) in ctx.program.functions.iter().enumerate() {
        if !function.external {
            continue;
        }
        let (native, _) = crate::natives::pragmas(ctx.program, ctx.interner, function);
        if native.as_deref() != Some("Object_getHash") {
            continue;
        }
        if index != canonical.0 as usize
            || function.library != core
            || function.class.is_some()
            || function.name != name
            || function.kind != FunctionKind::Function
            || function.patched_by.is_some()
            || !matches!(function.node, FunctionRef::Function { .. })
        {
            return false;
        }
        count += 1;
    }
    count == 1
}
