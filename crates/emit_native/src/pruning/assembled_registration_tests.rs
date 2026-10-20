//! Production registration follows the actual emitted tables, including empties.
use super::*;

const PROGRAM: &str = "define i32 @main() {\n  call void @df.register.sdk_tables()\n  ret i32 0\n}\ndeclare void @df.register.sdk_tables()\n";
const ARRAY: &str = "df.table_batch.df.register.sdk_tables";

fn sdk(count: u32) -> Summary {
    let mut ir = String::new();
    let tables = (0..count)
        .rev()
        .map(|index| {
            let method = format!("C{index}.dead");
            writeln!(
                ir,
                "define i64 @{method}(i64 %object, ptr %args, ptr %descriptor) {{\n  ret i64 0\n}}"
            )
            .unwrap();
            (
                900 + index,
                format!("df.mt.C{index}"),
                vec![("c:dead".into(), method)],
            )
        })
        .collect::<Vec<_>>();
    summarize(&ir, "dart:batch", &tables)
}

#[test]
fn sdk_batch_threshold_preserves_cid_order_and_every_empty_table() {
    for count in [0, 1, 7, 8, 9] {
        for prune in [false, true] {
            let summary = sdk(count);
            let program = if count == 0 {
                PROGRAM.to_string()
            } else {
                format!(
                    "{}declare ptr @df.mt.C0()\n",
                    PROGRAM.replace(
                        "  ret i32 0",
                        "  %table = call ptr @df.mt.C0()\n  ret i32 0"
                    )
                )
            };
            let output = assemble(&program, &[&summary], prune).unwrap().ir;
            if count < 8 {
                assert!(!output.contains("call void @dartforge_register_tables("));
                for index in 0..count {
                    assert!(output.contains(&format!(
                        "call void @dartforge_register_table(i64 {}, ptr @df.mt.C{index})",
                        900 + index
                    )));
                }
            } else {
                assert!(!output.contains("call void @dartforge_register_table("));
                let pointers = (0..count)
                    .map(|index| format!("ptr @df.mt.C{index}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                assert!(output.contains(&format!("@{ARRAY} = private unnamed_addr constant [{count} x ptr] [{pointers}], align 8")), "{output}");
                assert!(output.contains(&format!(
                    "call void @dartforge_register_tables(ptr @{ARRAY}, i64 {count})"
                )));
            }
            for index in 0..count {
                let fields = if !prune && index == 0 {
                    "{ i64, i64, i64, ptr }"
                } else {
                    "{ i64, i64 }"
                };
                assert!(
                    output.contains(&format!(
                        "@df.mt.C{index}$d = private unnamed_addr constant {fields}"
                    )),
                    "{output}"
                );
            }
        }
    }
}

#[test]
fn absent_startup_declaration_does_not_add_a_batch_or_registration() {
    let summary = sdk(9);
    let output = assemble("define i32 @main() {\n  ret i32 0\n}\n", &[&summary], false)
        .unwrap()
        .ir;
    assert!(!output.contains("df.register.sdk_tables"));
    assert!(!output.contains(ARRAY));
    assert!(!output.contains("call void @dartforge_register_table"));
}

#[test]
fn batch_storage_avoids_escaped_symbol_collisions_and_reuses_runtime_declarations() {
    let program = format!(
        "@\"df.table_batch.df.register.sdk_table\\73\" = private constant i8 1\n@{ARRAY}_ = private constant i8 2\n{PROGRAM}declare void @dartforge_register_tables(ptr, i64)\n"
    );
    let summary = sdk(8);
    let output = assemble(&program, &[&summary], false).unwrap().ir;
    assert!(
        output.contains(&format!(
            "@{ARRAY}__ = private unnamed_addr constant [8 x ptr]"
        )),
        "{output}"
    );
    assert_eq!(
        output
            .matches("declare void @dartforge_register_tables(ptr, i64)")
            .count(),
        1
    );
}

#[test]
fn unproved_callbacks_or_mismatched_embedded_cids_keep_scalar_registration() {
    let mut fragment = String::new();
    let registrations: Vec<_> = (0..8)
        .map(|index| {
            let name = format!("df.mt.C{index}");
            fragment.push_str(&table_line(
                &format!("@{name}$d = private unnamed_addr constant "),
                900 + index,
                &[],
            ));
            writeln!(
                fragment,
                "define ptr @{name}() {{\nb0:\n  ret ptr @{name}$d\n}}"
            )
            .unwrap();
            (900 + index, format!("@{name}"))
        })
        .collect();
    for (fragment, mut registrations) in [
        (
            fragment.replace(
                "  ret ptr @df.mt.C0$d",
                "  call void @effect()\n  ret ptr @df.mt.C0$d",
            ),
            registrations.clone(),
        ),
        (fragment.clone(), registrations.clone()),
        (
            fragment.replace(
                "private unnamed_addr constant",
                "private unnamed_addr global",
            ),
            registrations,
        ),
    ] {
        if fragment.contains("constant") && !fragment.contains("@effect") {
            registrations[0].0 = 999;
        }
        let mut output = fragment;
        let end = output.len();
        assembled_registration::emit(
            &mut output,
            "df.register.sdk_tables",
            &registrations,
            0..end,
            &mut HashSet::new(),
        );
        assert!(
            !output.contains("call void @dartforge_register_tables("),
            "{output}"
        );
        for (cid, function) in registrations {
            assert!(output.contains(&format!(
                "call void @dartforge_register_table(i64 {cid}, ptr {function})"
            )));
        }
    }
}
