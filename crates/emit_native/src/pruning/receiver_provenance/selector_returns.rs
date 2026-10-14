//! Getter result facts from complete tables for independently proved physical receivers.
use super::*;

pub(super) struct Inventory {
    tables: HashMap<u32, HashMap<String, String>>,
    dependencies: HashMap<String, BTreeSet<String>>,
}

impl Inventory {
    pub(super) fn new(
        modules: &[&Module],
        functions: &HashMap<String, &Function>,
        duplicates: &HashSet<String>,
        shapes: &HashMap<String, Shape>,
    ) -> Self {
        let mut result = Self { tables: HashMap::new(), dependencies: HashMap::new() };
        if std::env::var("DARTFORGE_NO_SELECTOR_RETURN_PROVENANCE").as_deref() == Ok("1")
            || modules.iter().any(|m| !m.arc_fixed_ancestry || m.arc_memory || m.arc_prepared || m.arc_aot_ports.is_some())
            || functions.keys().any(|name| matches!(name.as_str(), "dartforge_selector" | "dartforge_register_methods"))
        { return result; }
        let mut counts = HashMap::new();
        let mut table_counts = HashMap::new();
        for module in modules {
            for class in &module.classes { *counts.entry(class.id).or_insert(0usize) += 1; }
            for (cid, _, _) in &module.method_tables { *table_counts.entry(*cid).or_insert(0usize) += 1; }
        }
        for module in modules {
            for (cid, table, pairs) in &module.method_tables {
                if *cid < dartforge_runtime::layout::FIRST_FREE_CID as u32
                    || counts.get(cid) != Some(&1) || table_counts.get(cid) != Some(&1)
                    || module.table_functions.get(cid) != Some(table)
                    || module.pruning_ambiguous_class_ids.contains(cid)
                    || module.arc_external_ancestry.nominal_classes.contains(cid)
                    || module.arc_external_ancestry.ambiguous_classes.contains(cid)
                { continue; }
                let mut hashes = HashMap::new();
                for (name, _) in pairs {
                    *hashes.entry(crate::lower::closures::hash_name(name)).or_insert(0usize) += 1;
                }
                let mut getters = HashMap::new();
                for (name, symbol) in pairs {
                    if !name.starts_with("g:") || hashes[&crate::lower::closures::hash_name(name)] != 1
                        || duplicates.contains(symbol)
                    { continue; }
                    let Some(function) = functions.get(symbol) else { continue; };
                    if function.return_ty != Type::Ref
                        || function.params.iter().map(|(_, _, ty)| *ty).collect::<Vec<_>>() != [Type::Ref, Type::Ptr, Type::Ptr]
                        || !shapes.contains_key(symbol)
                    { continue; }
                    getters.insert(name.clone(), symbol.clone());
                    result.dependencies.entry(name.clone()).or_default().insert(symbol.clone());
                }
                result.tables.insert(*cid, getters);
            }
        }
        result
    }

    /// Include every possible getter target before SCC analysis; a partially known
    /// receiver set must never bootstrap a recursive return certificate.
    pub(super) fn dependencies(&self, selector: &str) -> impl Iterator<Item = &String> {
        self.dependencies.get(selector).into_iter().flatten()
    }

    pub(super) fn returns(
        &self, selector: &str, receivers: &Classes, summaries: &HashMap<String, Classes>,
    ) -> Option<Classes> {
        if receivers.is_empty() { return None; }
        let mut result = Classes::new();
        for cid in receivers {
            let target = self.tables.get(cid)?.get(selector)?;
            let classes = summaries.get(target)?;
            if classes.is_empty() { return None; }
            result.extend(classes);
        }
        Some(result)
    }
}

pub(super) fn getter(instruction: &I) -> Option<(&str, &Operand)> {
    match instruction {
        I::CallSelector { selector, recv, args, names, tuple_types }
            if selector.starts_with("g:") && args.is_empty() && names.is_empty()
                && *tuple_types == Operand::Constant(Constant::Int(0)) => Some((selector, recv)),
        _ => None,
    }
}

#[cfg(test)]
mod source_tests;
