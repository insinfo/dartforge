//! Bounded explanations from verified startup records, never proof premises.
use super::Function;
use std::collections::{BTreeSet, HashMap};

pub(super) fn startup_obligations(
    functions: &[Function],
    by_key: &HashMap<String, Vec<usize>>,
) -> HashMap<String, String> {
    let mut output = HashMap::new();
    for function in functions {
        if function.symbol != crate::sdk_module::registration_symbol(&function.module)
            || function.key != function.symbol
            || by_key
                .get(&function.key)
                .is_none_or(|items| items.len() != 1)
        {
            continue;
        }
        let mut obligations: BTreeSet<String> =
            function.unknown_obligations.iter().cloned().collect();
        let mut helpers = BTreeSet::new();
        for call in &function.calls {
            if call.symbol != "dartforge_register_helper" {
                continue;
            }
            let Some(target) = call
                .arguments
                .get(2)
                .and_then(|arg| arg.strip_prefix("ptr @"))
            else {
                helpers.insert("unresolved helper target operand".into());
                continue;
            };
            let local = format!("{}#{target}", function.module);
            let candidates = by_key.get(&local).or_else(|| by_key.get(target));
            let state = match candidates.map(Vec::as_slice) {
                Some([index]) => {
                    let target = &functions[*index];
                    if !target.complete_body || !target.unique_linkage || target.abi.is_none() {
                        "unproved definition or ABI"
                    } else if target.transitive_classes.is_none() {
                        "allocation closure unknown"
                    } else {
                        // Even an empty allocation footprint does not prove the
                        // runtime invocation ABI or inputs of an escaped address.
                        "allocation footprint observed; invocation coverage unproved"
                    }
                }
                Some(_) => "ambiguous definition",
                None => "definition missing",
            };
            helpers.insert(format!("{target:?}: {state}"));
        }
        let mut sections = Vec::new();
        if !obligations.is_empty() {
            sections.push(bounded("obligations", &obligations));
        }
        if !helpers.is_empty() {
            sections.push(bounded("helper targets", &helpers));
        }
        if sections.is_empty() {
            obligations
                .insert("startup body, exposure or input ABI lacks an entry certificate".into());
            sections.push(bounded("obligations", &obligations));
        }
        output.insert(function.key.clone(), sections.join("; "));
    }
    output
}

fn bounded(label: &str, items: &BTreeSet<String>) -> String {
    let shown: Vec<_> = items.iter().take(3).map(String::as_str).collect();
    let suffix = if items.len() > shown.len() {
        format!(", ... ({} total)", items.len())
    } else {
        String::new()
    };
    format!("{label}: {}{suffix}", shown.join(", "))
}
