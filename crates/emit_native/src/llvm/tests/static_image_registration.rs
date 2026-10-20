//! One bundle image owner, with standalone defaults and unchanged static bytes.
use super::*;

fn sdk_module() -> Module {
    let mut module = Module::new();
    module.sdk_library = true;
    module.record = Some("df.register.test".into());
    module.functions.push(function(
        "literal",
        vec![],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(0),
                Instruction::Const(Constant::String("static text".into())),
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
        }],
    ));
    module
}

#[test]
fn standalone_defaults_and_delegation_change_only_the_registration_call() {
    let module = sdk_module();
    let standalone = LlvmEmitter::new(&module)
        .with_static_objects(true)
        .emit_all();
    let explicit = LlvmEmitter::new(&module)
        .with_static_objects(true)
        .with_static_image_registration(true)
        .emit_all();
    assert_eq!(standalone, explicit);
    let delegated = LlvmEmitter::new(&module)
        .with_static_objects(true)
        .with_static_image_registration(false)
        .emit_all();
    let (_, start, end) = crate::target::image_markers();
    let call = format!("  call void @dartforge_register_image(ptr {start}, ptr {end})\n");
    assert_eq!(standalone.matches(&call).count(), 1);
    assert_eq!(standalone.replacen(&call, "", 1), delegated);
    assert!(delegated.contains("linkonce_odr constant { i64, i64, i64, ["));
}

#[test]
fn delegation_never_enables_static_storage_or_registration() {
    let module = sdk_module();
    for owner in [false, true] {
        let ir = LlvmEmitter::new(&module)
            .with_static_image_registration(owner)
            .emit_all();
        assert!(!ir.contains("  call void @dartforge_register_image("));
        assert!(!ir.contains("linkonce_odr constant { i64, i64, i64, ["));
    }
}

#[test]
fn program_keeps_its_own_image_and_calls_sdk_owner_before_later_startups() {
    let mut module = Module::new();
    module.sdk_mode = true;
    module.sdk_records = crate::sdk_module::sdk_records();
    let ir = LlvmEmitter::new(&module)
        .with_static_objects(true)
        .emit_all();
    assert_eq!(
        ir.matches("  call void @dartforge_register_image(").count(),
        1
    );
    let preparation = ir
        .split("define void @df.prepare_isolate() {")
        .nth(1)
        .unwrap();
    let preparation = preparation.split("\n}").next().unwrap();
    let mut previous = 0;
    for symbol in &module.sdk_records {
        let position = preparation
            .find(&format!("  call void @{symbol}()"))
            .unwrap();
        assert!(position >= previous);
        previous = position;
    }
    assert!(
        preparation
            .find("  call void @df.register.program()")
            .unwrap()
            > previous
    );
}
