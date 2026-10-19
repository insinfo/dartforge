use super::*;

#[test]
fn native_byte_arrays_preserve_exact_escaped_contents_and_lengths() {
    for (line, expected) in [
        (
            "@s = private unnamed_addr constant [4 x i8] c\"a\\22\\5C;\" ; comment",
            vec![b'a', b'"', b'\\', b';'],
        ),
        (
            "@s = private constant [2 x i8] c\"\\C3\\A9\", align 1",
            vec![0xc3, 0xa9],
        ),
        ("@s = private constant [0 x i8] c\"\"", vec![]),
        ("@s = private constant [3 x i8] c\"a@b\"", b"a@b".to_vec()),
    ] {
        assert_eq!(native_byte_content(line), Some(expected), "{line}");
        let bundle = audit(&[("sdk", line)]);
        assert_eq!(bundle.globals[0]["native_initializer"], line);
        assert_eq!(bundle.globals[0]["references"], json!([]));
    }
}

#[test]
fn malformed_or_nonliteral_byte_initializers_remain_unclassified() {
    for line in [
        "@s = private constant [2 x i8] c\"a\"",
        "@s = private constant [1 x i8] c\"ab\"",
        "@s = private constant [1 x i8] c\"\\ZZ\"",
        "@s = private constant [1 x i8] c\"\\1\"",
        "@s = private constant [1 x i8] c\"a\", align 0",
        "@s = private constant [1 x i8] c\"a\" garbage",
        "@s = private constant ptr @target",
        "@s = private global [1 x i8] c\"a\"",
        "@s = private constant [999999999999999999999999 x i8] c\"a\"",
        "@s = private constant ptr null ; constant [1 x i8] c\"a\"",
    ] {
        assert!(native_byte_content(line).is_none(), "{line}");
        let bundle = audit(&[("sdk", line)]);
        assert!(bundle.globals[0]["native_initializer"].is_null());
    }
}

fn state(bundle: &Bundle, symbol: &str) -> Option<Vec<u32>> {
    let index = bundle.definitions.iter().position(|d| d.key == symbol).unwrap();
    bundle.closures[index].as_ref().map(|values| values.iter().copied().collect())
}
#[test]
fn scalar_return_does_not_hide_an_allocation_and_wrappers_propagate_it() {
    let ir = "define i64 @allocate() {\n %o = call i64 @dartforge_object_new(i64 300, i64 0)\n ret i64 7\n}\ndefine i64 @wrapper() {\n %x = call i64 @allocate()\n ret i64 %x\n}\n";
    let bundle = audit(&[("sdk", ir)]);
    assert_eq!(state(&bundle, "allocate"), Some(vec![300]));
    assert_eq!(state(&bundle, "wrapper"), Some(vec![300]));
}

#[test]
fn quoted_header_braces_cannot_hide_the_allocating_body() {
    for header in [
        "define i64 @allocate(i64 %\"{ ret i64 0 }\") {",
        "define i64 @allocate(i64 %x) \"probe\"=\"{ ret i64 0 }\" {",
        "define i64 @allocate(i64 %\"; { ret i64 0 }\") {",
        "define i64 @allocate(i64 %\"\\22{ ret i64 0 }\\22\") {",
    ] {
        let ir = format!(
            "{header}\n %o = call i64 @dartforge_object_new(i64 399, i64 0)\n ret i64 7\n}}\ndefine i64 @wrapper() {{\n %v = call i64 @allocate(i64 0)\n ret i64 %v\n}}\n"
        );
        let bundle = audit(&[("sdk", &ir)]);
        assert_eq!(state(&bundle, "allocate"), Some(vec![399]), "{header}");
        assert_eq!(state(&bundle, "wrapper"), Some(vec![399]), "{header}");
    }
}

#[test]
fn quoted_inline_returns_and_commented_closing_braces_keep_body_boundaries() {
    let ir = "define i64 @identity(i64 %\"}\") { ret i64 %\"}\" }\ndefine i64 @allocate() {\n %o = call i64 @dartforge_object_new(i64 399, i64 0)\n ret i64 7\n} ; a closing comment with an unmatched quote \"\ndefine i64 @following() { ret i64 8 }\n";
    let bundle = audit(&[("sdk", ir)]);
    assert_eq!(bundle.definitions.len(), 3);
    assert_eq!(state(&bundle, "identity"), Some(vec![]));
    assert_eq!(state(&bundle, "allocate"), Some(vec![399]));
    assert_eq!(state(&bundle, "following"), Some(vec![]));
}

#[test]
fn mutually_recursive_finite_allocation_union_is_complete() {
    let ir = "define void @a() {\n %x = call i64 @dartforge_object_new(i64 300, i64 1)\n call void @b()\n ret void\n}\ndefine void @b() {\n %x = call i64 @dartforge_object_new(i64 301, i64 2)\n call void @a()\n ret void\n}\n";
    let bundle = audit(&[("sdk", ir)]);
    assert_eq!(state(&bundle, "a"), Some(vec![300,301]));
    assert_eq!(state(&bundle, "b"), Some(vec![300,301]));
    let pure = ir.replace(" %x = call i64 @dartforge_object_new(i64 300, i64 1)\n", "").replace(" %x = call i64 @dartforge_object_new(i64 301, i64 2)\n", "");
    assert_eq!(state(&audit(&[("sdk", &pure)]), "a"), Some(vec![]));
}
#[test]
fn opaque_runtime_selector_and_helper_boundaries_poison_the_complete_scc() {
    for boundary in ["opaque", "dartforge_stack_overflow", "dartforge_nsm_call", "dartforge_selector", "dartforge_register_helper"] {
        let ir = format!("define void @a() {{\n call void @b()\n ret void\n}}\ndefine void @b() {{\n call void @{boundary}()\n call void @a()\n ret void\n}}\n");
        let bundle = audit(&[("sdk", &ir)]);
        assert_eq!(state(&bundle, "a"), None, "{boundary}");
        assert_eq!(state(&bundle, "b"), None, "{boundary}");
    }
}
#[test]
fn only_exact_module_target_and_abi_can_close_a_direct_call() {
    let a = "define internal i64 @helper() {\n ret i64 3\n}\ndefine i64 @a() {\n %x = call i64 @helper()\n ret i64 %x\n}\n";
    let b = "define internal i64 @helper() {\n %x = call i64 @opaque()\n ret i64 %x\n}\ndefine i64 @b() {\n %x = call i64 @helper()\n ret i64 %x\n}\n";
    let bundle = audit(&[("a",a),("b",b)]);
    assert_eq!(state(&bundle,"a"),Some(vec![]));
    assert_eq!(state(&bundle,"b"),None);
    let wrong = a.replace("call i64 @helper()", "call i32 @helper()");
    assert_eq!(state(&audit(&[("a",&wrong)]),"a"),None);
    let duplicate = a.replace("internal ", "");
    assert_eq!(state(&audit(&[("a",&duplicate),("b",&duplicate)]),"a"),None);
}
#[test]
fn neutral_runtime_abi_and_local_actual_helpers_are_not_name_only_proofs() {
    let neutral = "define ptr @body() {\n %x = call ptr @dartforge_context()\n ret ptr %x\n}\n";
    assert_eq!(state(&audit(&[("sdk",neutral)]),"body"),Some(vec![]));
    assert_eq!(state(&audit(&[("sdk", &neutral.replace("call ptr", "call i64"))]),"body"),None);
    let shadow = format!("{neutral}define ptr @dartforge_context() {{\n %x = call ptr @opaque()\n ret ptr %x\n}}\n");
    assert_eq!(state(&audit(&[("sdk", &shadow)]),"body"),None);
    let modified = "define i64 @df.list_len() {\n %x = call i64 @dartforge_object_new(i64 311, i64 0)\n ret i64 %x\n}\ndefine i64 @use() {\n %x = call i64 @df.list_len()\n ret i64 %x\n}\n";
    assert_eq!(state(&audit(&[("sdk",modified)]),"use"),Some(vec![311]));
}
#[test]
fn globals_function_addresses_and_indirect_calls_keep_distinct_open_obligations() {
    let ir = "@bytes = private unnamed_addr constant [2 x i8] c\"ab\"\n@image = private constant {i64,i64} {i64 300,i64 0}\n@callbacks = global ptr @target\ndefine i64 @target() {\n ret i64 0\n}\ndefine ptr @bytes_ptr() {\n ret ptr @bytes\n}\ndefine ptr @image_ptr() {\n ret ptr @image\n}\ndefine void @indirect(ptr %fn) {\n call void %fn()\n ret void\n}\n";
    let bundle = audit(&[("sdk",ir)]);
    assert_eq!(bundle.globals.len(),3);
    assert_eq!(bundle.definitions.len(),4,"globals are not function bodies");
    assert_eq!(state(&bundle,"bytes_ptr"),Some(vec![]));
    assert_eq!(state(&bundle,"image_ptr"),None);
    assert_eq!(state(&bundle,"indirect"),None);
    assert_eq!(bundle.globals[2]["references"],json!(["target"]));
}
#[test]
fn unsupported_body_syntax_nonconstant_cid_and_weak_definitions_are_unknown() {
    for body in ["%p = call i64 @dartforge_object_new(i64 %cid, i64 0)","%p = invoke i64 @f() to label %a unwind label %b", "call fastcc void @f()", "fence seq_cst"] {
        let ir=format!("define void @body(i64 %cid) {{\n {body}\n ret void\n}}\n");
        assert_eq!(state(&audit(&[("sdk",&ir)]),"body"),None,"{body}");
    }
    assert_eq!(state(&audit(&[("sdk","define weak void @body() {\n ret void\n}\n")]),"body"),None);
}
#[test]
fn actual_emitter_helpers_keep_unresolved_memory_allocations_unknown() {
    let mut module = crate::hir::Module::new();
    module.sdk_mode = true;
    let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
    let bundle = audit(&[("sdk", &ir)]);
    assert_eq!(state(&bundle, "sdk#df.list_len"), None);
    let definition = bundle.definitions.iter().find(|d|d.key == "sdk#df.list_len").unwrap();
    assert!(definition.exposures.contains("raw_memory_or_pointer_operation"));
    assert!(definition.unknown.contains("unresolved_raw_memory_allocation"));
    assert!(bundle.definitions.iter().all(|d| !d.body_digest.is_empty()));
    assert!(bundle.globals.iter().all(|g|g["kind"] != "function"));
}
#[test]
fn duplicate_modules_and_hidden_inline_calls_never_gain_empty_closure() {
    let body="define void @body() {\n ret void\n}\n";
    assert_eq!(state(&audit(&[("same",body),("same","define void @another() {\n ret void\n}\n")]),"body"),None);
    let inline="define i64 @body() { ret i64 0 call void @opaque() }\n";
    assert_eq!(state(&audit(&[("sdk",inline)]),"body"),None);
    let intrinsic="define i64 @body() {\n %x = call i64 @llvm.expect.i1(i64 1, i64 1)\n ret i64 %x\n}\n";
    assert_eq!(state(&audit(&[("sdk",intrinsic)]),"body"),None);
}

#[test]
fn comment_brace_does_not_hide_a_real_allocating_body() {
    let ir="define void @body() { ; } is only a comment\n %x = call i64 @dartforge_object_new(i64 399, i64 0)\n ret void\n}\n";
    assert_eq!(state(&audit(&[("sdk",ir)]),"body"),Some(vec![399]));
    let unterminated="define void @body() { ; }\n %x = call i64 @dartforge_object_new(i64 399, i64 0)\n";
    assert_eq!(state(&audit(&[("sdk",unterminated)]),"body"),None);
}
#[test]
fn call_bundles_neutral_image_registration_and_initializer_comments_stay_open() {
    let bundled="define void @leaf() { ret void }\ndefine void @body() {\n call void @leaf() [\"deopt\"(i32 1)]\n ret void\n}\n";
    assert_eq!(state(&audit(&[("sdk",bundled)]),"body"),None);
    let image="@bytes = private unnamed_addr constant [2 x i8] c\"ab\"\ndefine void @body() {\n call void @dartforge_register_image(ptr @bytes, ptr @bytes)\n ret void\n}\n";
    assert_eq!(state(&audit(&[("sdk",image)]),"body"),None,"allocation-neutral catalog entry is not a managed-image proof");
    let comment="@image = private constant {i64,i64} {i64 300,i64 0} ; constant [2 x i8] c\"ab\"\ndefine ptr @body() {\n ret ptr @image\n}\n";
    let bundle=audit(&[("sdk",comment)]);
    assert_eq!(bundle.globals[0]["kind"],json!("unclassified_initializer"));
    assert_eq!(state(&bundle,"body"),None);
}

#[test]
fn valid_inline_heap_bump_allocation_is_not_a_proven_empty_footprint() {
    // This is the emitter's allocation fast-path shape: reserve a heap block,
    // initialize its object header, then tag its address as a Dart reference.
    let ir = r#"define i64 @bump(ptr %context) {
entry:
  %top_slot = getelementptr inbounds i8, ptr %context, i64 80
  %top = load ptr, ptr %top_slot, align 8
  %next = getelementptr inbounds i8, ptr %top, i64 24
  store ptr %next, ptr %top_slot, align 8
  store i64 1288490188801, ptr %top, align 8
  %field = getelementptr inbounds i8, ptr %top, i64 16
  store i64 0, ptr %field, align 8
  %address = ptrtoint ptr %top to i64
  %reference = add i64 %address, 2
  ret i64 %reference
}
define i64 @wrapper(ptr %context) {
  %object = call i64 @bump(ptr %context)
  ret i64 %object
}
"#;
    let bundle = audit(&[("sdk", ir)]);
    assert_eq!(state(&bundle, "bump"), None);
    assert_eq!(state(&bundle, "wrapper"), None);
    let bump = bundle.definitions.iter().find(|definition| definition.key == "bump").unwrap();
    assert!(bump.unknown.contains("unresolved_raw_memory_allocation"));
    assert!(bump.exposures.contains("raw_memory_or_pointer_operation"));
    assert!(bump.calls.is_empty(), "no allocator call may hide the direct heap allocation");
}

#[test]
fn quoted_section_operands_keep_the_direct_registration_call_visible() {
    let ir = "define void @df.register.sdk() {\n call void @dartforge_register_map_llvm(ptr @\"\\01section$start$__LLVM_STACKMAPS$__llvm_stackmaps\", ptr @\"\\01section$end$__LLVM_STACKMAPS$__llvm_stackmaps\")\n ret void\n}\n";
    let bundle = audit(&[("sdk", ir)]);
    let function = &bundle.to_json()["functions"][0];
    assert_eq!(function["calls"].as_array().unwrap().len(), 1);
    assert_eq!(
        function["calls"][0]["symbol"],
        "dartforge_register_map_llvm"
    );
    assert_eq!(
        function["calls"][0]["arguments"][0],
        "ptr @\"\\01section$start$__LLVM_STACKMAPS$__llvm_stackmaps\""
    );
    assert!(
        state(&bundle, "df.register.sdk").is_none(),
        "observing registration is not authorization"
    );
}

#[test]
fn direct_calls_publish_typed_operands_without_changing_allocation_facts() {
    let ir = "define i64 @allocate() {\n %v = call i64 @dartforge_object_new(i64 300, i64 2)\n ret i64 %v\n}\n";
    let bundle = audit(&[("sdk", ir)]);
    let function = &bundle.to_json()["functions"][0];
    assert_eq!(
        function["calls"][0]["arguments"],
        serde_json::json!(["i64 300", "i64 2"])
    );
    assert_eq!(state(&bundle, "allocate"), Some(vec![300]));
}

#[test]
fn quoted_operand_delimiters_escapes_and_comments_do_not_change_the_call_abi() {
    for operand in [
        "ptr @\"a),[b]{c}<d>\"",
        "ptr @\"escaped\\22quote\\5Cslash\"",
        "ptr @\"semi;colon\"",
        "ptr getelementptr ([4 x i8], ptr @\"a,(b)\", i64 0, i64 1)",
    ] {
        let instruction = format!("call void @opaque({operand}, i64 7)");
        let (_, abi, values) = signature(&instruction).unwrap();
        assert_eq!(abi.parameters, ["ptr", "i64"]);
        assert_eq!(values, [operand, "i64 7"]);
        let ir = format!(
            "define void @body() {{\n {instruction} ; an ignored comment with an unmatched quote \"\n ret void\n}}\n"
        );
        let bundle = audit(&[("sdk", &ir)]);
        assert_eq!(bundle.definitions[0].calls.len(), 1, "{operand}");
        assert!(state(&bundle, "body").is_none());
    }
}

#[test]
fn malformed_lists_and_unreviewed_abis_stay_unproved() {
    for instruction in [
        "call void @opaque(ptr @\"open)",
        "call void @opaque(ptr @\"bad\\ZZescape\")",
        "call void @opaque(ptr @\"short\\1\")",
        "call void @opaque(ptr getelementptr ([1 x i8}, ptr @buffer, i64 0))",
        "call void @opaque(i64 0,)",
        "call void @opaque(,i64 0)",
        "call void @opaque(ptr noundef @buffer)",
        "call void @\"quoted_callee\"()",
        "call fastcc void @opaque()",
        "call void %indirect()",
    ] {
        assert!(signature(instruction).is_none(), "{instruction}");
        let ir = format!("define void @body() {{\n {instruction}\n ret void\n}}\n");
        assert!(
            state(&audit(&[("sdk", &ir)]), "body").is_none(),
            "{instruction}"
        );
    }
}
