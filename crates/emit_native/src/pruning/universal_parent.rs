//! Certification of the terminal subtype parent, distinct from heap class identity.
//!
//! Lowering inserts CID zero as a synthetic parent in each module. Sharing that
//! declaration cannot authorize a table, layout or allocation with CID zero.
use super::{AllocationState, Summary};
use crate::hir::{Instruction, Module};

pub(crate) fn declared(module: &Module) -> bool {
    let mut roots = module.classes.iter().filter(|class| class.id == 0);
    let Some(root) = roots.next() else {
        return false;
    };
    module.pruning_class_ids.contains(&0)
        && module.pruning_metadata_only_class_ids.contains(&0)
        && !module.pruning_ambiguous_class_ids.contains(&0)
        && roots.next().is_none()
        && root.name == "Object"
        && root.field_count == 0
        && root.vtable.is_empty()
        && root.to_string_symbol.is_none()
        && !module.method_tables.iter().any(|(cid, _, _)| *cid == 0)
        && !module.table_functions.contains_key(&0)
        && !module.declared_field_layouts.contains_key(&0)
        && !module.runtime_cids.contains(&0)
        && !module.program_ids.iter().any(|(cid, _, _)| *cid == 0)
        && !module.subtyping_edges.iter().any(|(child, _)| *child == 0)
        && !module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .any(|(_, instruction, _)| {
                matches!(instruction, Instruction::AllocObject { class_id: 0, .. })
            })
}

pub(crate) fn summarized(summary: &Summary) -> bool {
    summary.universal_parent
        && summary.nominal_classes.iter().filter(|cid| **cid == 0).count() == 1
        && !summary.ambiguous_classes.contains(&0)
        && !summary.tables.iter().any(|table| table.cid == 0)
        && !summary.declared_field_layouts.contains_key(&0)
        && !summary.parent_edges.iter().any(|(child, _)| *child == 0)
        && !summary.allocation_inventory.iter().any(|record|
            matches!(&record.state, AllocationState::Known(classes) if classes.contains(&0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::ClassDef;

    fn root() -> Module {
        let mut module = Module::new();
        module.classes.push(ClassDef {
            id: 0,
            name: "Object".into(),
            field_count: 0,
            vtable: vec![],
            to_string_symbol: None,
        });
        module.pruning_class_ids.insert(0);
        module.pruning_metadata_only_class_ids.insert(0);
        module
    }

    #[test]
    fn only_the_terminal_parent_declaration_emits_a_shareable_marker() {
        let module = root();
        assert!(declared(&module));
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        assert!(ir.contains("; df.universal-parent 0\n"));
        let summary = super::super::summarize(&ir, "dart:test", &[]);
        assert!(summarized(&summary));
        assert_eq!(Summary::from_string(&summary.to_text()).unwrap(), summary);
        assert!(super::super::hir_summary(&module).universal_parent);
        for mutation in 0..14 {
            let mut changed = root();
            match mutation {
                0 => {
                    changed.pruning_metadata_only_class_ids.clear();
                }
                1 => {
                    changed.classes[0].field_count = 1;
                }
                2 => {
                    changed.classes[0].name = "UserClass".into();
                }
                3 => {
                    changed.classes[0].to_string_symbol = Some("df.fake".into());
                }
                4 => {
                    changed.classes.push(changed.classes[0].clone());
                }
                5 => {
                    changed.pruning_ambiguous_class_ids.insert(0);
                }
                6 => {
                    changed.method_tables.push((0, "df.fake".into(), vec![]));
                }
                7 => {
                    changed.table_functions.insert(0, "df.fake".into());
                }
                8 => {
                    changed.declared_field_layouts.insert(0, vec![]);
                }
                9 => {
                    changed.runtime_cids.push(0);
                }
                10 => {
                    changed.subtyping_edges.push((0, 1));
                }
                11 => {
                    changed.pruning_class_ids.clear();
                }
                12 => {
                    changed
                        .program_ids
                        .push((0, "dart:core".into(), "Object".into()));
                }
                _ => {
                    changed.classes[0].vtable.push((1, "df.fake".into()));
                }
            }
            assert!(!declared(&changed), "mutation {mutation}");
            assert!(
                !crate::llvm::LlvmEmitter::new(&changed)
                    .emit_all()
                    .contains("; df.universal-parent 0\n"),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn serialized_root_markers_refuse_heap_identity_and_malformed_claims() {
        let ir = "; df.universal-parent 0\n; df.cone-class 0\n";
        let summary = super::super::summarize(ir, "dart:test", &[]);
        let text = summary.to_text();
        assert!(Summary::from_string(&text).is_ok());
        for text in [
            text.replace("U\t0", "U\t1"),
            text.replace("U\t0", "U\t0\tignored"),
            text.replace("U\t0", "U\t0\nU\t0"),
            text.replace("K\t0\n", ""),
            format!("{text}A\t0\n"),
            format!("{text}P\t0\t1\n"),
            format!("{text}L\t0\t[]\n"),
        ] {
            assert!(Summary::from_string(&text).is_err(), "{text}");
        }
        let mut allocated = summary.clone();
        allocated
            .allocation_inventory
            .push(super::super::AllocationRecord {
                definition: 0,
                state: AllocationState::Known(vec![0]),
            });
        assert!(!summarized(&allocated));
        let mut table = summary;
        table.tables.push(super::super::SummarizedTable {
            node: 0,
            cid: 0,
            pairs: vec![],
        });
        assert!(!summarized(&table));
    }
}
