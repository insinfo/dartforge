//! Entry-only certificates for nominal subtype registration.
//! This domain changes metadata, not heap class activation or helper bindings.
use super::{Function, Summary};
use std::collections::{HashMap, HashSet};

pub(super) const SUBCLASS: &str = "dartforge_register_subclass";

/// Accept only complete startup bodies whose sole external effect publishes
/// literal edges already present in the reconciled nominal image.
pub(super) fn subtype_startups(
    functions: &[Function],
    summaries: &[Summary],
    by_key: &HashMap<String, Vec<usize>>,
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
            || function.unknown_obligations
                != [format!("image_registry_or_helper_binding:{SUBCLASS}")]
            || function.exposure_obligations != ["image_registry_or_helper_binding"]
            || by_key.contains_key(SUBCLASS)
            || by_key.contains_key(&format!("{}#{SUBCLASS}", function.module))
        {
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
            || definitions[0]
                .refs
                .iter()
                .any(|node| summary.names.get(*node as usize).map(String::as_str) != Some(SUBCLASS))
        {
            continue;
        }
        let valid_cid = |cid: i64| {
            (0..=i64::from(i32::MAX)).contains(&cid)
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
        if !function.calls.iter().all(|call| {
            if call.symbol != SUBCLASS
                || call.abi.result != "void"
                || call.abi.parameters != ["i64", "i64"]
                || call.arguments.len() != 2
            {
                return false;
            }
            let literal = |argument: &str| argument.strip_prefix("i64 ")?.parse::<i64>().ok();
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
        }) {
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
        subtype_startups(&raw.functions, &summaries, &keys).contains("df.register.dart$3atest")
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
