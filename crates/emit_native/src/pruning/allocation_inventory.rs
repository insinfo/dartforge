//! Direct allocation audits of the actual final emitted definitions.
//!
//! This first producer proves only leaf LLVM bodies without calls, memory/global
//! access or pointer conversion. Everything else remains explicitly unknown.
//! It does not certify transitive allocation closure or enable import pruning.
pub(crate) mod closure;
pub(crate) mod images;
pub(crate) mod imports;
mod integrity_io;
pub(crate) mod publication;
mod register_read;

use super::{AllocationRecord, AllocationState, Summary};
use std::collections::HashMap;

pub(crate) fn audit_emitted(ir: &str, summary: &mut Summary) -> usize {
    let mut bodies: HashMap<String, Vec<bool>> = HashMap::new();
    let mut lines = ir.lines();
    while let Some(header) = lines.next() {
        if !header.starts_with("define ") {
            continue;
        }
        let Some(symbol) = header
            .split_once('@')
            .and_then(|(_, value)| value.split_once('('))
            .map(|(name, _)| name)
        else {
            continue;
        };
        let mut body = Vec::new();
        let mut complete = false;
        for line in lines.by_ref() {
            if line.trim() == "}" {
                complete = true;
                break;
            }
            body.push(line);
        }
        bodies
            .entry(symbol.to_string())
            .or_default()
            .push(complete && no_allocation_leaf(&body));
    }
    // Every actual definition gets an explicit state. In particular, an int or
    // void result is not evidence that its body allocates no Dart objects.
    let mut nodes = std::collections::HashSet::new();
    summary.allocation_inventory = summary
        .definitions
        .iter()
        .filter(|definition| nodes.insert(definition.name))
        .map(|definition| {
            let symbol = &summary.names[definition.name as usize];
            let state = if bodies.get(symbol).is_some_and(|candidates| {
                !candidates.is_empty() && candidates.iter().all(|known| *known)
            }) {
                AllocationState::Known(Vec::new())
            } else {
                AllocationState::Unknown
            };
            AllocationRecord {
                definition: definition.name,
                state,
            }
        })
        .collect();
    summary
        .definitions
        .iter()
        .filter(|definition| summary.audited_allocations(definition.name).is_some())
        .count()
}

fn no_allocation_leaf(body: &[&str]) -> bool {
    !body.is_empty()
        && body.iter().all(|line| {
            let line = line.split(';').next().unwrap_or_default().trim();
            if line.is_empty() || line.ends_with(':') {
                return true;
            }
            // No global/object descriptor or possible symbol invocation is accepted.
            if line.contains('@') {
                return false;
            }
            let operation = line.split_once('=').map_or(line, |(_, value)| value.trim());
            let opcode = operation.split_whitespace().next().unwrap_or_default();
            matches!(
                opcode,
                "ret"
                    | "br"
                    | "unreachable"
                    | "add"
                    | "sub"
                    | "mul"
                    | "shl"
                    | "lshr"
                    | "ashr"
                    | "and"
                    | "or"
                    | "xor"
                    | "fadd"
                    | "fsub"
                    | "fmul"
                    | "fdiv"
                    | "fneg"
                    | "icmp"
                    | "fcmp"
                    | "select"
                    | "phi"
                    | "zext"
                    | "sext"
                    | "trunc"
                    | "sitofp"
                    | "uitofp"
                    | "fptosi"
                    | "fptoui"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_scalar_leaf_gets_empty_audit_but_allocating_scalar_return_stays_unknown() {
        let ir = "define i64 @leaf(i64 %v) {\nb0:\n  %sum = add i64 %v, 1\n  ret i64 %sum\n}\ndefine i64 @allocating() {\nb0:\n  %object = call i64 @dartforge_object_new(i64 300, i64 0)\n  ret i64 7\n}\n";
        let mut summary = super::super::summarize(ir, "sdk", &[]);
        assert_eq!(audit_emitted(ir, &mut summary), 1);
        let node = |symbol: &str| {
            summary
                .names
                .iter()
                .position(|name| name == symbol)
                .unwrap() as u32
        };
        assert_eq!(
            summary.audited_allocations(node("leaf")),
            Some([].as_slice())
        );
        assert_eq!(summary.audited_allocations(node("allocating")), None);
        assert_eq!(Summary::from_string(&summary.to_text()).unwrap(), summary);
    }
    #[test]
    fn all_possible_emitted_definitions_must_agree_on_coverage() {
        let safe = "define linkonce_odr i64 @shared() {\nb0:\n  ret i64 0\n}\n";
        let unsafe_body = "define linkonce_odr i64 @shared() {\nb0:\n  %object = call i64 @unknown()\n  ret i64 0\n}\n";
        let ir = format!("{safe}{unsafe_body}");
        let mut summary = super::super::summarize(&ir, "sdk", &[]);
        assert_eq!(audit_emitted(&ir, &mut summary), 0);
        assert!(
            summary
                .allocation_inventory
                .iter()
                .all(|record| record.state == AllocationState::Unknown)
        );
    }
    #[test]
    fn memory_globals_pointer_conversion_and_unterminated_bodies_are_unknown() {
        for body in [
            "%p = alloca i64",
            "%v = load i64, ptr %p",
            "store i64 1, ptr %p",
            "%v = ptrtoint ptr %p to i64",
            "ret ptr @object",
            "call void @opaque()",
        ] {
            assert!(!no_allocation_leaf(&["b0:", body]));
        }
        let ir = "define i64 @broken() {\nb0:\n ret i64 0\n";
        let mut summary = super::super::summarize(ir, "sdk", &[]);
        assert_eq!(audit_emitted(ir, &mut summary), 0);
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    fn summary() -> Summary {
        super::super::summarize("define void @body() {\nb0:\n ret void\n}\n", "sdk", &[])
    }
    #[test]
    fn explicit_known_empty_and_cid_union_roundtrip_distinct_from_absent_or_unknown() {
        let mut summary = summary();
        let node = summary.definitions[0].name;
        assert_eq!(
            summary.audited_allocations(node),
            None,
            "absence never invents an empty audit"
        );
        for state in [
            AllocationState::Unknown,
            AllocationState::Known(vec![]),
            AllocationState::Known(vec![300, 301]),
        ] {
            summary.allocation_inventory = vec![AllocationRecord {
                definition: node,
                state: state.clone(),
            }];
            let parsed = Summary::from_string(&summary.to_text()).unwrap();
            assert_eq!(parsed, summary);
            match state {
                AllocationState::Unknown => assert_eq!(parsed.audited_allocations(node), None),
                AllocationState::Known(classes) => {
                    assert_eq!(parsed.audited_allocations(node), Some(classes.as_slice()))
                }
            }
        }
    }
    #[test]
    fn allocation_audits_and_ambiguous_classes_roundtrip_independently() {
        let mut summary = summary();
        summary.ambiguous_classes = vec![128, 300];
        summary.allocation_inventory = vec![AllocationRecord {
            definition: summary.definitions[0].name,
            state: AllocationState::Known(vec![300]),
        }];
        let text = summary.to_text();
        assert!(text.contains("\nA\t128\n"));
        assert!(text.contains("\nI\t"));
        let parsed = Summary::from_string(&text).unwrap();
        assert_eq!(parsed, summary);
        assert_eq!(parsed.ambiguous_classes, [128, 300]);
        assert_eq!(
            parsed.audited_allocations(summary.definitions[0].name),
            Some([300].as_slice())
        );
    }

    #[test]
    fn malformed_duplicate_old_and_nondefinition_audits_are_rejected() {
        let summary = summary();
        let node = summary.definitions[0].name;
        let text = summary.to_text();
        for record in [
            format!("I\t{node}\tknown\t-1\n"),
            format!("I\t{node}\tknown\t4294967296\n"),
            format!("I\t{node}\tknown\t300 300\n"),
            format!("I\t{node}\tunknown\t300\n"),
            format!("I\t{node}\tknown\t300\textra\n"),
            "I\t99999\tknown\t300\n".to_string(),
            format!("I\t{node}\tknown\t\nI\t{node}\tunknown\t\n"),
        ] {
            assert!(
                Summary::from_string(&format!("{text}{record}")).is_err(),
                "malformed {record:?}"
            );
        }
        assert!(
            Summary::from_string(&text.replace("dartforge-poda\t9", "dartforge-poda\t8")).is_err()
        );
        let mut summary = summary;
        let orphan = summary.names.len() as u32;
        summary.names.push("orphan".into());
        assert!(
            Summary::from_string(&format!("{}I\t{orphan}\tknown\t300\n", summary.to_text()))
                .is_err()
        );
    }
}
