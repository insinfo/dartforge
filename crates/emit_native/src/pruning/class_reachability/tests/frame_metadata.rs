//! Display publication neither activates nor makes a generated frame allocatable.
use super::*;

fn startup() -> String {
    let frame = crate::lower::async_sm::ASYNC_FRAME_ID;
    let name = crate::lower::async_sm::ASYNC_FRAME_NAME;
    format!(
        "@frame = private constant [{} x i8] c\"{name}\"\ndefine void @df.register.dart$3atest() {{\n call void @dartforge_register_class_name(i64 {frame}, ptr @frame, i64 {})\n ret void\n}}\n",
        name.len(),
        name.len()
    )
}

fn module_with_startup(
    inventory: std::sync::Arc<crate::pruning::allocation_inventory::imports::Inventory>,
) -> Module {
    let mut module = fixture();
    module.sdk_records.push("df.register.dart$3atest".into());
    module.imported_allocation_inventory = Some(inventory);
    module
}

#[test]
fn frame_display_does_not_expand_the_closed_class_universe() {
    let (_directory, summary, inventory) = checked_scalar_sdk(&startup());
    let frame = crate::lower::async_sm::ASYNC_FRAME_ID;
    let mut module = module_with_startup(inventory.clone());
    audit(&mut module, &[&summary]).unwrap();
    assert_eq!(
        module.closed_class_tables.as_ref().unwrap(),
        &HashSet::from([300])
    );
    assert!(!summary.nominal_classes.contains(&i64::from(frame)));
    for declared in [false, true] {
        let mut module = module_with_startup(inventory.clone());
        if declared {
            module.classes.push(ClassDef {
                id: frame,
                name: "_AsyncFrame".into(),
                field_count: 0,
                vtable: vec![],
                to_string_symbol: None,
            });
        }
        module.functions[0].blocks[0].instructions.push((
            ValueId(1),
            Instruction::AllocObject {
                class_id: frame,
                fields: vec![],
            },
            Type::Ref,
        ));
        assert!(audit(&mut module, &[&summary]).is_err());
        assert!(module.closed_class_tables.is_none());
    }
}

#[test]
#[ignore = "requires Clang and the native runtime"]
fn compiled_frame_display_preserves_subtype_queries_under_gc_stress() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let sdk_ir = startup();
        let (directory, summary, inventory) = checked_scalar_sdk(&sdk_ir);
        let frame = crate::lower::async_sm::ASYNC_FRAME_ID;
        for closed in [false, true] {
            let mut module = module_with_startup(inventory.clone());
            module.runtime_cids = dartforge_runtime::layout::cid::SDK_CLASSES.iter()
                .map(|(cid, _, _)| i64::from(*cid)).collect();
            if closed { audit(&mut module, &[&summary]).unwrap(); }
            let emitted = crate::llvm::LlvmEmitter::new(&module).emit_all();
            let declaration = "declare void @df.register.dart$3atest()\n";
            assert_eq!(emitted.matches(declaration).count(), 1);
            let emitted = emitted.replacen(declaration, &sdk_ir, 1);
            let wrapper = "define i32 @main() {\n  %r = call i32 @dartforge_init(ptr @dartforge_entry, ptr @dartforge_dispatch_toString)\n  ret i32 %r\n}\n";
            assert_eq!(emitted.matches(wrapper).count(), 1);
            let mut emitted = emitted.replacen(wrapper, "", 1);
            let start = emitted.find("define void @dartforge_entry() {").unwrap();
            let end = start + emitted[start..].find("\n}\n").unwrap();
            let position = start + emitted[start..end].rfind("  ret void").unwrap();
            // A scalar observation after certified startup, with no object allocation.
            emitted.insert_str(position, "  call void @frame_subtype_probe()\n");
            if !emitted.contains("declare void @llvm.trap()") {
                emitted.push_str("\ndeclare void @llvm.trap()\n");
            }
            emitted.push_str(&format!("\ndefine void @frame_subtype_probe() {{\n %a = call i8 @dartforge_is_subclass(i64 {frame}, i64 300)\n %b = call i8 @dartforge_is_subclass(i64 300, i64 {frame})\n %x = icmp eq i8 %a, 0\n %y = icmp eq i8 %b, 0\n %ok = and i1 %x, %y\n br i1 %ok, label %pass, label %fail\npass:\n ret void\nfail:\n call void @llvm.trap()\n unreachable\n}}\n"));
            let executable = directory.path().join(crate::target::executable_name(&format!("frame-name-{closed}")));
            crate::driver::compile_and_link(&emitted, &executable, &crate::driver::NativeDriverOptions::default()).unwrap();
            for stress in [false, true] {
                let mut command = std::process::Command::new(&executable);
                command.env_remove("DARTFORGE_GC_STRESS");
                if stress { command.env("DARTFORGE_GC_STRESS", "1"); }
                let output = command.output().unwrap();
                assert!(output.status.success(), "closed={closed}, stress={stress}: {}", String::from_utf8_lossy(&output.stderr));
                assert!(output.stdout.is_empty() && output.stderr.is_empty());
            }
        }
    }).unwrap().join().unwrap();
}
