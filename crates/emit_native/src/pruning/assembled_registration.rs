//! Register the actual assembled SDK tables only after their getters are known.
use super::allocation_inventory::closure::table_getters;
use std::collections::HashSet;
use std::fmt::Write;

pub(crate) const BATCH_MIN: usize = 8;

/// The array replaces scalar CID arguments, so every getter must be pure and
/// return a complete immutable table with exactly that embedded CID.
pub(super) fn emit(
    output: &mut String,
    record: &str,
    registrations: &[(i64, String)],
    table_fragment: std::ops::Range<usize>,
    declared: &mut HashSet<String>,
) {
    let batch = registrations.len() >= BATCH_MIN && {
        let getters = table_getters::observe(&[("assembled-sdk-tables", &output[table_fragment])]);
        registrations.iter().all(|(cid, function)| {
            function
                .strip_prefix('@')
                .and_then(|name| getters.get(name))
                .is_some_and(|found| i64::from(*found) == *cid)
        })
    };
    if batch {
        let occupied: HashSet<_> = declared
            .iter()
            .filter_map(|name| table_getters::canonical_symbol(name))
            .collect();
        let mut array = format!("df.table_batch.{record}");
        while occupied.contains(&array) {
            array.push('_');
        }
        declared.insert(array.clone());
        let pointers = registrations
            .iter()
            .map(|(_, function)| format!("ptr {function}"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            output,
            "@{array} = private unnamed_addr constant [{} x ptr] [{pointers}], align 8",
            registrations.len()
        )
        .unwrap();
        writeln!(output, "define void @{record}() {{\nb0:\n  call void @dartforge_register_tables(ptr @{array}, i64 {})\n  ret void\n}}", registrations.len()).unwrap();
        if declared.insert("dartforge_register_tables".into()) {
            output.push_str("declare void @dartforge_register_tables(ptr, i64)\n");
        }
    } else {
        writeln!(output, "define void @{record}() {{\nb0:").unwrap();
        for (cid, function) in registrations {
            writeln!(
                output,
                "  call void @dartforge_register_table(i64 {cid}, ptr {function})"
            )
            .unwrap();
        }
        output.push_str("  ret void\n}\n");
        if declared.insert("dartforge_register_table".into()) {
            output.push_str("declare void @dartforge_register_table(i64, ptr)\n");
        }
    }
}
