//! Lifetime of generated internal C entries published into the callback runtime.
use dartforge_jit::JitSession;

fn callback_module(value: i64) -> String {
    format!(
        r#"
@key = private constant [3 x i8] c"i_i"
declare void @dartforge_ffi_register_callback(ptr, i64, ptr)
define internal i32 @df.ffi.cbentry.0(ptr nest %context, i32 %argument) {{
  ret i32 {value}
}}
define i64 @publish() {{
  call void @dartforge_ffi_register_callback(ptr @key, i64 3, ptr @df.ffi.cbentry.0)
  ret i64 {value}
}}
"#
    )
}

#[test]
#[ignore = "requires the full LLVM distribution and native JIT execution"]
fn metadata_registration_does_not_pin_its_generation() {
    let mut session = JitSession::new().unwrap();
    session
        .add_reloadable_module("callbacks", &callback_module(1))
        .unwrap();
    let publish = session.stable_entry("publish").unwrap();
    // Registration stores metadata, but no executable trampoline has been
    // created. Retiring the sealed owner removes that metadata atomically.
    assert_eq!(publish.call(&session).unwrap(), 1);
    session
        .hot_reload("callbacks", &callback_module(2))
        .unwrap();
    // No NativeCallable trampoline refers to generation 1.
    assert_eq!(
        session.retained_generations(),
        1,
        "metadata alone must not retain generation 1"
    );
    assert_eq!(publish.call(&session).unwrap(), 2);
}

#[test]
#[ignore = "requires the full LLVM distribution and native JIT execution"]
fn cached_callback_metadata_can_be_unloaded() {
    let compiled = dartforge_jit::compile_module("callbacks", &callback_module(1)).unwrap();
    let mut session = JitSession::new().unwrap();
    session.add_compiled_module(&compiled).unwrap();
    session.remove_module(0).unwrap();
    assert!(session.module_names().is_empty());
}

#[test]
#[ignore = "requires the full LLVM distribution and native JIT execution"]
fn unused_callback_declaration_does_not_pin_ordinary_code() {
    let ir = "declare void @dartforge_ffi_register_callback(ptr, i64, ptr)\n\
              define i64 @plain() { ret i64 1 }\n";
    let mut session = JitSession::new().unwrap();
    session.add_ir_module("plain", ir).unwrap();
    session.remove_module(0).unwrap();
    assert!(session.module_names().is_empty());
    let mut reloadable = JitSession::new().unwrap();
    reloadable.add_reloadable_module("plain", ir).unwrap();
    reloadable
        .hot_reload("plain", &ir.replace("ret i64 1", "ret i64 2"))
        .unwrap();
    assert_eq!(reloadable.retained_generations(), 1);
}

#[test]
#[ignore = "requires the full LLVM distribution and native JIT execution"]
fn address_taken_registration_remains_conservatively_resident() {
    let ir = "@registration = constant ptr @dartforge_ffi_register_callback\n\
              declare void @dartforge_ffi_register_callback(ptr, i64, ptr)\n\
              define i64 @plain() { ret i64 1 }\n";
    let mut session = JitSession::new().unwrap();
    session.add_ir_module("unknown-registration", ir).unwrap();
    assert_eq!(session.remove_module(0).unwrap_err().stage, "resource");
}

#[test]
#[ignore = "requires the full LLVM distribution and native JIT execution"]
fn escaping_internal_closure_code_remains_conservatively_resident() {
    let ir = "declare i64 @dartforge_tearoff(i64)\n\
              define internal i64 @local(i64 %closure, ptr %args, ptr %descriptor) { ret i64 7 }\n\
              define i64 @saved_closure() {\n\
                %closure = call i64 @dartforge_tearoff(i64 ptrtoint (ptr @local to i64))\n\
                ret i64 %closure\n\
              }\n";
    let mut session = JitSession::new().unwrap();
    session.add_reloadable_module("internal-closure", ir).unwrap();
    session.hot_reload("internal-closure", &ir.replace("ret i64 7", "ret i64 11")).unwrap();
    assert_eq!(session.retained_generations(), 2);
}
