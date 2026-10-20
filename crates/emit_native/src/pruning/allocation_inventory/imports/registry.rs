//! Entry-only certificates for nominal and textual metadata registration.
//! This domain changes metadata, not heap class activation or helper bindings.
use super::{Function, Summary};
use std::collections::{HashMap, HashSet};

pub(super) const SUBCLASS: &str = "dartforge_register_subclass";
const CLASS_NAME: &str = "dartforge_register_class_name";
const ARGUMENT_NAME: &str = "dartforge_register_argument_name";

pub(super) fn metadata_api(symbol: &str) -> bool {
    matches!(symbol, SUBCLASS | CLASS_NAME | ARGUMENT_NAME)
}

fn literal(argument: &str) -> Option<i64> {
    argument.strip_prefix("i64 ")?.parse().ok()
}

fn text_constant<'a>(
    module: &str,
    pointer: &str,
    length: &str,
    bytes: &'a HashMap<String, Vec<u8>>,
) -> Option<(&'a str, String)> {
    let symbol = pointer.strip_prefix("ptr @")?;
    if symbol.is_empty()
        || !symbol
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_$.-".contains(&b))
    {
        return None;
    }
    let key = format!("{module}#{symbol}");
    let content = bytes.get(&key)?;
    if usize::try_from(literal(length)?).ok()? != content.len() {
        return None;
    }
    Some((std::str::from_utf8(content).ok()?, key))
}

/// Accept only complete startup bodies whose sole external effect publishes
/// literal nominal edges and bounds-checked immutable UTF-8 metadata.
pub(super) fn metadata_startups(
    functions: &[Function],
    summaries: &[Summary],
    by_key: &HashMap<String, Vec<usize>>,
    byte_contents: &HashMap<String, Vec<u8>>,
) -> HashSet<String> {
    let nominal: HashSet<_> = summaries
        .iter()
        .flat_map(|s| &s.nominal_classes)
        .copied()
        .collect();
    let ambiguous: HashSet<_> = summaries
        .iter()
        .flat_map(|s| &s.ambiguous_classes)
        .copied()
        .collect();
    let mut owners: HashMap<i64, Vec<&Summary>> = HashMap::new();
    for summary in summaries {
        for cid in &summary.nominal_classes {
            owners.entry(*cid).or_default().push(summary);
        }
    }
    let frame = i64::from(crate::lower::async_sm::ASYNC_FRAME_ID);
    // The reserved frame display label has no factory role in the runtime's
    // name-based allocation paths. It must not alias any nominal/table/layout
    // declaration or introduce subtype premises. This grants no allocation fact.
    let frame_display_only = summaries.iter().all(|summary| {
        !summary.nominal_classes.contains(&frame)
            && !summary.ambiguous_classes.contains(&frame)
            && !summary.tables.iter().any(|table| table.cid == frame)
            && !summary.declared_field_layouts.contains_key(&(frame as u32))
            && !summary.parent_edges.iter().any(|(child, parent)| *child == frame || *parent == frame)
    });
    let mut accepted = HashSet::new();
    for function in functions {
        let Some(summary) = summaries.iter().find(|s| s.module == function.module) else {
            continue;
        };
        let expected = crate::sdk_module::registration_symbol(&function.module);
        if function.key != expected
            || function.symbol != expected
            || function.local
            || !function.unique_linkage
            || !function.complete_body
            || by_key
                .get(&function.key)
                .is_none_or(|items| items.len() != 1)
            || !function
                .abi
                .as_ref()
                .is_some_and(|abi| abi.result == "void" && abi.parameters.is_empty())
            || !function.direct_classes.is_empty()
            || function.calls.is_empty()
            || !function.dependencies.is_empty()
            || function.exposure_obligations != ["image_registry_or_helper_binding"]
            || function.calls.iter().any(|call| {
                !metadata_api(&call.symbol)
                    || by_key.contains_key(&call.symbol)
                    || by_key.contains_key(&format!("{}#{}", function.module, call.symbol))
            })
        {
            continue;
        }
        let expected_unknown: std::collections::BTreeSet<_> = function
            .calls
            .iter()
            .map(|call| format!("image_registry_or_helper_binding:{}", call.symbol))
            .collect();
        if function
            .unknown_obligations
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            != expected_unknown.iter().collect()
        {
            continue;
        }
        let mut covered_names: HashSet<String> = function
            .calls
            .iter()
            .map(|call| call.symbol.clone())
            .collect();
        let valid_cid = |cid: i64| {
            (0..=i64::from(i32::MAX)).contains(&cid)
                && cid != frame
                && nominal.contains(&cid)
                && !ambiguous.contains(&cid)
                && owners.get(&cid).is_some_and(|items| {
                    items.len() == 1
                        || cid == 0
                            && items
                                .iter()
                                .all(|s| crate::pruning::universal_parent::summarized(s))
                })
        };
        let mut class_names: HashMap<i64, String> = HashMap::new();
        if !function.calls.iter().all(|call| {
            if call.abi.result != "void" {
                return false;
            }
            match call.symbol.as_str() {
                SUBCLASS if call.abi.parameters == ["i64", "i64"] && call.arguments.len() == 2 => {
                    let Some(child) = literal(&call.arguments[0]) else {
                        return false;
                    };
                    let Some(parent) = literal(&call.arguments[1]) else {
                        return false;
                    };
                    valid_cid(child)
                        && valid_cid(parent)
                        && summary.nominal_classes.contains(&child)
                        && summary.parent_edges.contains(&(child, parent))
                }
                CLASS_NAME
                    if call.abi.parameters == ["i64", "ptr", "i64"]
                        && call.arguments.len() == 3 =>
                {
                    let Some(cid) = literal(&call.arguments[0]) else {
                        return false;
                    };
                    let Some((text, key)) = text_constant(
                        &function.module,
                        &call.arguments[1],
                        &call.arguments[2],
                        byte_contents,
                    ) else {
                        return false;
                    };
                    if cid == frame {
                        if !frame_display_only || text != crate::lower::async_sm::ASYNC_FRAME_NAME {
                            return false;
                        }
                    } else if !valid_cid(cid) || !summary.nominal_classes.contains(&cid) {
                        return false;
                    }
                    covered_names.insert(key);
                    match class_names.insert(cid, text.into()) {
                        Some(previous) => previous == text,
                        None => true,
                    }
                }
                ARGUMENT_NAME
                    if call.abi.parameters == ["ptr", "i64"] && call.arguments.len() == 2 =>
                {
                    let Some((_, key)) = text_constant(
                        &function.module,
                        &call.arguments[0],
                        &call.arguments[1],
                        byte_contents,
                    ) else {
                        return false;
                    };
                    covered_names.insert(key);
                    true
                }
                _ => false,
            }
        }) {
            continue;
        }
        let definitions: Vec<_> = summary
            .definitions
            .iter()
            .filter(|d| summary.names.get(d.name as usize) == Some(&function.key))
            .collect();
        if definitions.len() != 1
            || !definitions[0].calls.is_empty()
            || definitions[0].refs.is_empty()
            || definitions[0].refs.iter().any(|node| {
                summary
                    .names
                    .get(*node as usize)
                    .is_none_or(|name| !covered_names.contains(name))
            })
        {
            continue;
        }
        accepted.insert(function.key.clone());
    }
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn certificate(ir: &str, mutate: impl FnOnce(&mut Vec<Summary>)) -> bool {
        let bundle = crate::pruning::allocation_inventory::closure::audit(&[("dart:test", ir)]);
        let raw: super::super::Payload = serde_json::from_value(bundle.to_json()).unwrap();
        let mut summaries = vec![crate::pruning::summarize(ir, "dart:test", &[])];
        mutate(&mut summaries);
        let mut keys: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, f) in raw.functions.iter().enumerate() {
            keys.entry(f.key.clone()).or_default().push(index);
        }
        metadata_startups(&raw.functions, &summaries, &keys, &HashMap::new())
            .contains("df.register.dart$3atest")
    }

    #[test]
    fn nominal_ownership_and_summary_edges_are_independent_premises() {
        let ir = "; df.cone-class 900\n; df.cone-class 901\n; df.cone-parent 900 901\ndefine void @df.register.dart$3atest() {\n call void @dartforge_register_subclass(i64 900, i64 901)\n ret void\n}\n";
        assert!(certificate(ir, |_| {}));
        for variant in 0..7 {
            assert!(
                !certificate(ir, |summaries| {
                    let summary = &mut summaries[0];
                    match variant {
                        0 => summary.ambiguous_classes.push(901),
                        1 => summary.parent_edges.clear(),
                        2 => summary.definitions[0].refs.clear(),
                        3 => summary.definitions[0].calls.push(123),
                        4 => summary.definitions.push(summary.definitions[0].clone()),
                        5 => summaries.push(crate::pruning::summarize(
                            "; df.cone-class 901\n",
                            "dart:other",
                            &[],
                        )),
                        _ => {
                            summary.nominal_classes.retain(|cid| *cid != 900);
                            summaries.push(crate::pruning::summarize(
                                "; df.cone-class 900\n",
                                "dart:other",
                                &[],
                            ));
                        }
                    }
                }),
                "variant {variant}"
            );
        }
    }

    #[test]
    fn shared_zero_requires_the_existing_universal_parent_certificate() {
        let ir = "; df.universal-parent 0\n; df.cone-class 0\n; df.cone-class 900\n; df.cone-parent 900 0\ndefine void @df.register.dart$3atest() {\n call void @dartforge_register_subclass(i64 900, i64 0)\n ret void\n}\n";
        assert!(certificate(ir, |summaries| summaries.push(
            crate::pruning::summarize(
                "; df.universal-parent 0\n; df.cone-class 0\n",
                "dart:other",
                &[]
            )
        )));
        assert!(!certificate(ir, |summaries| summaries.push(
            crate::pruning::summarize("; df.cone-class 0\n", "dart:other", &[])
        )));
    }
}
