//! Identity of virtual tables assembled from the verified SDK summaries.
//! A declaration certificate does not authorize calling or allocating its class.
use super::Summary;
use std::collections::HashMap;

pub(super) fn declarations(summaries: &[Summary]) -> HashMap<String, u32> {
    let mut definitions = HashMap::new();
    let mut nodes = HashMap::new();
    let mut cids = HashMap::new();
    let mut externals = HashMap::new();
    for summary in summaries {
        for definition in &summary.definitions {
            if let Some(name) = summary.names.get(definition.name as usize) {
                *definitions.entry(name.as_str()).or_insert(0usize) += 1;
            }
        }
        for table in &summary.tables {
            if let Some(name) = summary.names.get(table.node as usize) {
                *nodes.entry(name.as_str()).or_insert(0usize) += 1;
            }
            *cids.entry(table.cid).or_insert(0usize) += 1;
        }
        for external in &summary.externals {
            if let Some(name) = summary.names.get(*external as usize) {
                *externals.entry(name.as_str()).or_insert(0usize) += 1;
            }
        }
    }
    let mut accepted = HashMap::new();
    for summary in summaries {
        let table_by_name: HashMap<_, _> = summary
            .tables
            .iter()
            .filter_map(|table| {
                summary
                    .names
                    .get(table.node as usize)
                    .map(|name| (name.as_str(), table))
            })
            .collect();
        let definition_by_id: HashMap<_, _> = summary
            .definitions
            .iter()
            .map(|definition| (definition.name, definition))
            .collect();
        for external in &summary.externals {
            let Some(name) = summary.names.get(*external as usize) else {
                continue;
            };
            let data = format!("{name}$d");
            if name.contains('#')
                || nodes.contains_key(name.as_str())
                || externals.contains_key(data.as_str())
                || definitions.get(name.as_str()) != Some(&1)
                || definitions.contains_key(data.as_str())
                || externals.get(name.as_str()) != Some(&1)
                || nodes.get(data.as_str()) != Some(&1)
            {
                continue;
            }
            let Some(table) = table_by_name.get(data.as_str()) else {
                continue;
            };
            if cids.get(&table.cid) != Some(&1)
                || !definition_by_id.get(external).is_some_and(|definition| {
                    definition.refs == [table.node] && definition.calls.is_empty()
                })
            {
                continue;
            }
            if let Ok(cid) = u32::try_from(table.cid) {
                accepted.insert(name.clone(), cid);
            }
        }
    }
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_identity_requires_unique_cid_symbols_and_exact_virtual_body() {
        let table = "df.mt.imported";
        let valid = crate::pruning::summarize("", "dart:test", &[(900, table.into(), vec![])]);
        assert_eq!(
            declarations(std::slice::from_ref(&valid)).get(table),
            Some(&900)
        );
        for variant in 0..10 {
            let mut summary = valid.clone();
            match variant {
                0 => summary.externals.clear(),
                1 => summary.externals.push(summary.externals[0]),
                2 => summary.definitions.push(summary.definitions[0].clone()),
                3 => summary.tables.push(summary.tables[0].clone()),
                4 => summary.definitions[0].refs.clear(),
                5 => summary.definitions[0].calls.push(123),
                6 => summary.tables[0].cid = -1,
                7 => {
                    let data = summary.tables[0].node;
                    summary.definitions.push(crate::pruning::Definition {
                        name: data,
                        ..Default::default()
                    });
                }
                8 => summary.tables[0].node = summary.externals[0],
                _ => summary.definitions[0].refs.push(summary.externals[0]),
            }
            assert!(
                !declarations(&[summary]).contains_key(table),
                "variant {variant}"
            );
        }
        assert!(!declarations(&[valid.clone(), valid]).contains_key(table));
    }
}
