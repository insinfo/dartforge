use super::*;
use crate::{hir::*, llvm::LlvmEmitter};

fn module(values: &[i64], reversed_registration: bool) -> Module {
    let mut module = Module::new();
    module.sdk_mode = true;
    module.arc_fixed_ancestry = true;
    let arrays = if reversed_registration { vec![vec![9], values.to_vec()] }
        else { vec![values.to_vec(), vec![9]] };
    let mut instructions: Vec<_> = arrays.into_iter().enumerate().map(|(i, values)|
        (ValueId(i as u32 + 2), Instruction::ConstArray(values), Type::Ptr)).collect();
    instructions.push((ValueId(4), Instruction::FunctionTable(vec!["table_target".into()]), Type::Ptr));
    instructions.push((ValueId(5), Instruction::CallClosure {
        closure: Operand::Val(ValueId(0)), args: vec![], names: vec![],
        tuple_types: Operand::Constant(Constant::Int(0)), ret_ty: Type::Ref,
    }, Type::Ref));
    instructions.push((ValueId(6), Instruction::CallSelector {
        selector: "c:probe".into(), recv: Operand::Val(ValueId(1)), args: vec![], names: vec![],
        tuple_types: Operand::Constant(Constant::Int(0)),
    }, Type::Ref));
    module.functions.push(Function {
        symbol: "vector_probe".into(), name: "vector_probe".into(), debug_info: None,
        params: vec![(ValueId(0), "closure".into(), Type::Ref), (ValueId(1), "receiver".into(), Type::Ref)],
        return_ty: Type::Void,
        blocks: vec![BasicBlock { id: BlockId(0), instructions, terminator: Terminator::Return(None) }],
    });
    module
}

#[test]
fn identity_binds_type_length_signed_content_and_order() {
    let values = [123, 2, 124, 3];
    assert_eq!(symbol(&values), symbol(&values));
    assert_ne!(symbol(&values), symbol(&[123, 2, 124, 4]));
    assert_ne!(symbol(&values), symbol(&[124, 3, 123, 2]));
    assert_ne!(symbol(&[-1]), symbol(&[i64::MAX]));
    assert_ne!(symbol(&[]), symbol(&[0]));
    assert_ne!(digest("i64", values.len(), &values), digest("ptr", values.len(), &values));
    assert_ne!(digest("i64", values.len(), &values), digest("i64", values.len() + 1, &values));
    assert!(symbol(&values).ends_with(&digest("i64", values.len(), &values).to_hex().to_string()));
}

#[test]
fn independent_modules_share_content_without_registration_order_dependency() {
    let values = [123, 2, 124, 3];
    let name = symbol(&values);
    for reversed in [false, true] {
        let module = module(&values, reversed);
        let mut emitter = LlvmEmitter::new(&module).with_body_without_outside(true);
        emitter.numeric_vector_pooling = true;
        let ir = emitter.emit_all();
        assert!(ir.contains(&format!("@{name} = linkonce_odr hidden unnamed_addr constant [4 x i64] [i64 123, i64 2, i64 124, i64 3]")));
        assert!(ir.contains(&format!("getelementptr i64, ptr @{name}, i64 0")));
        assert!(!ir.contains("@df.arr."));
        // Closure, selector and dispatch-toString descriptors must all refer to
        // the same definition, not to the former module-private alias.
        let descriptor = symbol(&[0, 0]);
        assert_eq!(ir.matches(&format!("@{descriptor} =")).count(), 1);
        assert!(ir.matches(&format!("ptr @{descriptor}")).count() >= 3);
        assert!(ir.contains("@df.fns.0 = private unnamed_addr constant [1 x ptr] [ptr @table_target]"));
    }
}

#[test]
fn pooling_requires_explicit_fixed_image_lifecycle_not_nominal_metadata() {
    let mut module = module(&[1, 2], false);
    assert!(!LlvmEmitter::new(&module).with_production(true).pool_numeric_vectors());
    module.arc_fixed_ancestry = false;
    assert!(!LlvmEmitter::new(&module).with_body_without_outside(true).pool_numeric_vectors());
    module.arc_fixed_ancestry = true;
    let mut emitter = LlvmEmitter::new(&module).with_body_without_outside(true);
    emitter.numeric_vector_pooling = true;
    assert!(emitter.pool_numeric_vectors());
    assert!(!LlvmEmitter::new(&module).with_body_without_outside(true).with_previous_area(Some(vec![])).pool_numeric_vectors());
    assert!(!LlvmEmitter::new(&module).with_body_without_outside(true).with_maps_in_jit(true).pool_numeric_vectors());
    assert!(!LlvmEmitter::new(&module).with_body_without_outside(true).with_stack_trace_in_jit(true).pool_numeric_vectors());
}

#[test]
fn disabled_and_reloadable_images_keep_private_vectors_and_references() {
    let module = module(&[123, 2], false);
    for explicitly_disabled in [false, true] {
        let mut emitter = LlvmEmitter::new(&module).with_body_without_outside(explicitly_disabled);
        emitter.numeric_vector_pooling = !explicitly_disabled;
        let ir = emitter.emit_all();
        assert!(ir.contains("@df.arr.0 = private unnamed_addr constant [2 x i64] [i64 123, i64 2]"));
        assert!(ir.contains("getelementptr i64, ptr @df.arr.0, i64 0"));
        assert!(!ir.contains("@df.const.i64."));
        assert!(ir.contains("@df.fns.0 = private unnamed_addr constant"));
    }
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires Clang and the macOS SDK"]
fn independent_macho_objects_coalesce_equal_vectors_and_keep_different_contents() {
    use std::{fs, process::Command};
    let directory = std::env::temp_dir().join(format!("dartforge-vector-link-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir(&directory).unwrap();
    let clang = crate::driver::NativeDriverOptions::default().clang;
    let sdk = Command::new("xcrun").arg("--show-sdk-path").output().expect("macOS SDK");
    assert!(sdk.status.success(), "{}", String::from_utf8_lossy(&sdk.stderr));
    let sdk = String::from_utf8(sdk.stdout).unwrap();
    let equal = symbol(&[123, 2]);
    let different = symbol(&[123, 3]);
    let declaration = |values: &[i64]| {
        let module = module(values, false);
        let mut emitter = LlvmEmitter::new(&module).with_body_without_outside(true);
        emitter.numeric_vector_pooling = true;
        let ir = emitter.emit_all();
        ir.lines().find(|line| line.starts_with(&format!("@{} =", symbol(values)))).unwrap().to_string()
    };
    let getter = |name: &str, vector: &str| format!(
        "define i64 @{name}() {{\nentry:\n  %p = getelementptr [2 x i64], ptr @{vector}, i64 0, i64 1\n  %v = load i64, ptr %p, align 8\n  ret i64 %v\n}}\n");
    for (name, ir) in [
        ("left", format!("{}\n{}", declaration(&[123, 2]), getter("left_value", &equal))),
        ("right", format!("{}\n{}\n{}{}", declaration(&[123, 2]), declaration(&[123, 3]),
            getter("right_value", &equal), getter("other_value", &different))),
    ] {
        let source = directory.join(format!("{name}.ll"));
        fs::write(&source, ir).unwrap();
        let output = Command::new(&clang).args(["-O0", "-Wno-override-module", "-c", "-x", "ir"])
            .arg(&source).arg("-o").arg(directory.join(format!("{name}.o"))).output().expect("Clang IR compilation");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }
    let main = directory.join("main.c");
    fs::write(&main, "extern long long left_value(void), right_value(void), other_value(void);\nint main(void) { return !(left_value()==2 && right_value()==2 && other_value()==3); }\n").unwrap();
    let binary = directory.join("program");
    let output = Command::new(&clang).arg("-isysroot").arg(sdk.trim()).arg("-mmacosx-version-min=11.0")
        .arg(directory.join("left.o")).arg(directory.join("right.o")).arg(&main).arg("-o").arg(&binary)
        .output().expect("Mach-O link");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(Command::new(&binary).status().unwrap().success(), "pooled vectors changed loaded contents");
    let symbols = Command::new("nm").arg("-U").arg(&binary).output().expect("Mach-O symbols");
    assert!(symbols.status.success());
    let symbols = String::from_utf8(symbols.stdout).unwrap();
    for vector in [&equal, &different] {
        assert_eq!(symbols.lines().filter(|line| line.split_whitespace().last() == Some(&format!("_{vector}"))).count(), 1,
            "equal definitions must coalesce; different contents must remain: {symbols}");
    }
    fs::remove_dir_all(directory).unwrap();
}
