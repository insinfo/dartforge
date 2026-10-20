//! Registration batching preserves the selected getters and their order.
use super::*;

fn module(count: u32) -> Module {
    let mut module = Module {
        record: Some("df.register.program".into()),
        ..Module::default()
    };
    for index in 0..count {
        module
            .method_tables
            .push((900 + index, format!("df.mt.C{index}"), vec![]));
    }
    module
}

fn registration(module: &Module) -> String {
    let mut emitter = LlvmEmitter::new(module);
    emitter.emit_registration("df.register.program");
    emitter.out
}

#[test]
fn batch_threshold_and_getter_order_are_explicit() {
    for count in [0, 1, 7, 8, 9] {
        let ir = registration(&module(count));
        if count < 8 {
            assert!(!ir.contains("@df.table_batch."));
            assert!(!ir.contains("call void @dartforge_register_tables("));
            assert_eq!(
                ir.matches("call void @dartforge_register_table(").count(),
                count as usize
            );
        } else {
            assert!(!ir.contains("call void @dartforge_register_table("));
            assert_eq!(
                ir.matches("call void @dartforge_register_tables(").count(),
                1
            );
            let getters = (0..count)
                .map(|index| format!("ptr @df.mt.C{index}"))
                .collect::<Vec<_>>()
                .join(", ");
            assert!(
                ir.contains(&format!("constant [{count} x ptr] [{getters}], align 8")),
                "{ir}"
            );
            assert!(ir.contains(&format!("call void @dartforge_register_tables(ptr @df.table_batch.df.register.program, i64 {count})")));
        }
    }
}

#[test]
fn closed_class_selection_precedes_batching() {
    let mut module = module(12);
    for active in [vec![901, 904], (902..910).collect()] {
        module.closed_class_tables = Some(active.iter().copied().collect());
        let ir = registration(&module);
        for index in 0..12 {
            assert_eq!(
                [',', ')', ']']
                    .iter()
                    .any(|end| ir.contains(&format!("ptr @df.mt.C{index}{end}"))),
                active.contains(&(900 + index)),
                "{ir}"
            );
        }
        assert_eq!(
            ir.contains("call void @dartforge_register_tables("),
            active.len() >= 8
        );
    }
}

#[test]
fn sdk_startup_keeps_its_independent_trace_registration() {
    let mut module = module(12);
    module.sdk_library = true;
    module.classes.push(ClassDef {
        id: 905,
        name: "_StackTrace".into(),
        field_count: 1,
        vtable: vec![],
        to_string_symbol: None,
    });
    let ir = registration(&module);
    assert!(!ir.contains("@df.table_batch."));
    assert_eq!(
        ir.matches("call void @dartforge_register_table(").count(),
        1
    );
    assert!(ir.contains("call void @dartforge_register_table(i64 905, ptr @df.mt.C5)"));
}
