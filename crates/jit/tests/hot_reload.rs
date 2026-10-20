//! Hot reload scenarios migrated from the old track to the emit_native IR.

use dartforge_jit::JitSession;

/// A shared library with two argument-less exports,
/// `GetTickCount` (any value) and `GetCurrentProcessId` (the pid), and the
/// `exportados.def` next to it with the requested `names`. It serves to test the
/// SDK library boundary without compiling the whole SDK.
///
/// On Windows it is a copy of `kernel32.dll`, which exports both. On other
/// systems, a two-function library compiled now by Clang
/// (`DARTFORGE_CLANG`, otherwise the one on `PATH`), with `GetCurrentProcessId` on top of
/// libc's `getpid`.
fn library_with_two_exports(dir: &std::path::Path, names: &[&str]) -> std::path::PathBuf {
    let mut def = String::from("EXPORTS\n");
    for n in names {
        def.push_str(n);
        def.push('\n');
    }
    std::fs::write(dir.join("exportados.def"), def).unwrap();
    if cfg!(windows) {
        let origin = std::path::PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot"))
            .join("System32/kernel32.dll");
        let dll = dir.join("kernel32.dll");
        std::fs::copy(&origin, &dll).expect("copy of kernel32 into the worktree target");
        return dll;
    }
    let ll = dir.join("dois.ll");
    std::fs::write(
        &ll,
        "declare i32 @getpid()\n\
         define i32 @GetTickCount() {\n  ret i32 1\n}\n\
         define i32 @GetCurrentProcessId() {\n  %p = call i32 @getpid()\n  ret i32 %p\n}\n",
    )
    .unwrap();
    let dll = dir.join(if cfg!(target_os = "macos") {
        "libdois.dylib"
    } else {
        "libdois.so"
    });
    let clang = std::env::var_os("DARTFORGE_CLANG").unwrap_or_else(|| "clang".into());
    let status = std::process::Command::new(clang)
        .args(["-x", "ir", "-shared", "-fPIC"])
        .arg(&ll)
        .args(dartforge_emit_native::target::link_arguments())
        .arg("-o")
        .arg(&dll)
        .status()
        .expect("Clang");
    assert!(status.success(), "Clang did not compile the test library");
    dll
}

/// Helpers with internal linkage belong to each LLVM module; the current
/// emitter generates `df_clo_invalido` like that. They cannot become public entries.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn internal_helpers_do_not_become_trampolines() {
    let ir = |value: i64| {
        format!(
            "\
define internal i64 @df_clo_invalido() {{ ret i64 {value} }}
define i64 @df_fn_0() {{
  %r = call i64 @df_clo_invalido()
  ret i64 %r
}}
"
        )
    };
    let mut session = JitSession::new().expect("session");
    let first_one = session
        .add_reloadable_module("app", &ir(1))
        .expect("first generation");
    assert_eq!(first_one.entries, 1);
    assert_eq!(session.stable_entries("app"), vec!["df_fn_0"]);
    let input = session.stable_entry("df_fn_0").unwrap();
    assert_eq!(input.call(&session).unwrap(), 1);
    session
        .hot_reload("app", &ir(2))
        .expect("second generation");
    assert_eq!(input.call(&session).unwrap(), 2);
}

/// The `main` with the source SDK has ABI `i32 ()` and must use the updated
/// trampoline. A system library is enough to test the boundary without
/// compiling the whole SDK locally; the remote CLI test covers the real one.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn session_main_with_dll_calls_new_generation() {
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dir = Directory(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-jit-main-dll-{}",
            std::process::id()
        )),
    );
    std::fs::create_dir_all(&dir.0).unwrap();
    let dll = library_with_two_exports(&dir.0, &["GetTickCount"]);

    let ir = |value: i32| format!("define i32 @main() {{ ret i32 {value} }}\n");
    let mut session = JitSession::new_with_sdk(&dll, &[]).unwrap();
    session.add_reloadable_module("app", &ir(7)).unwrap();
    assert_eq!(session.run_reloadable_main().unwrap().exit_code, 7);
    session.hot_reload("app", &ir(9)).unwrap();
    assert_eq!(session.run_reloadable_main().unwrap().exit_code, 9);
}

/// Invalid IR or a missing reference does not replace the running generation.
/// A valid version can still be published after both failures.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn reload_failure_does_not_destroy_good_version() {
    let ir = |value: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {value}\n}}\n");
    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", &ir(7))
        .expect("generation 1");
    let input = session.stable_entry("df_fn_0").expect("stable entry");
    let address = session.lookup("df_fn_0").expect("stable address");
    assert_eq!(input.call(&session).unwrap(), 7);

    let error = session
        .hot_reload("app", "THIS IS NOT LLVM IR @@@ {{{")
        .unwrap_err();
    assert_eq!(error.stage, "parse-ir");

    let ir_without_external = "declare i64 @sqlite3_open(i64)\n\
                          define i64 @df_fn_0() {\n\
                            %r = call i64 @sqlite3_open(i64 0)\n\
                            ret i64 %r\n}\n";
    let error = session.hot_reload("app", ir_without_external).unwrap_err();
    assert_eq!(error.stage, "contract");
    assert!(error.message.contains("sqlite3_open"), "{error}");

    assert_eq!(session.generation("app"), Some(1));
    assert_eq!(session.retained_generations(), 1);
    assert_eq!(session.lookup("df_fn_0").unwrap(), address);
    assert_eq!(input.call(&session).unwrap(), 7);

    session
        .hot_reload("app", &ir(8))
        .expect("reload after failures");
    assert_eq!(session.generation("app"), Some(2));
    assert_eq!(session.lookup("df_fn_0").unwrap(), address);
    assert_eq!(input.call(&session).unwrap(), 8);
}

/// A handle kept by the caller keeps pointing to the same object
/// while the stable entry starts calling the body of the new generation.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn live_counter_survives_reload() {
    let ir = |step: i64| {
        format!(
            "\
declare i64 @dartforge_object_new(i64, i64)
declare i64 @dartforge_object_get(i64, i64)
declare void @dartforge_object_set(i64, i64, i64, i8)
define i64 @df_fn_0() {{
  %h = call i64 @dartforge_object_new(i64 7, i64 1)
  ret i64 %h
}}
define i64 @df_fn_1(i64 %h) {{
  %v = call i64 @dartforge_object_get(i64 %h, i64 0)
  %n = add i64 %v, {step}
  call void @dartforge_object_set(i64 %h, i64 0, i64 %n, i8 0)
  ret i64 %n
}}
"
        )
    };

    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("contador", &ir(1))
        .expect("generation 1");
    let create = session.stable_entry("df_fn_0").expect("create counter");
    let add_up = session.stable_entry("df_fn_1").expect("increment counter");
    let address = session.lookup("df_fn_1").expect("stable address");
    let counter = create.call(&session).expect("live object");
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 1);
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 2);
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 3);

    session
        .hot_reload("contador", &ir(10))
        .expect("generation 2");
    assert_eq!(session.lookup("df_fn_1").unwrap(), address);
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 13);
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 23);
}

/// Five consecutive reloads keep the public address stable and advance
/// generation by generation; each one unloads the previous, which no entry
/// reaches anymore (J02): code memory does not grow with the reloads.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn five_reloads_preserve_stable_entry() {
    let ir = |value: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {value}\n}}\n");
    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", &ir(0))
        .expect("initial generation");
    let input = session.stable_entry("df_fn_0").expect("stable entry");
    let address = session.lookup("df_fn_0").expect("public address");
    assert_eq!(input.call(&session).unwrap(), 0);

    for value in 1..=5 {
        let report_data = session.hot_reload("app", &ir(value)).expect("recarga");
        assert_eq!(report_data.generation, value as u32 + 1);
        assert_eq!(report_data.new_entries, 0);
        assert_eq!(report_data.retained_generations, 1);
        assert_eq!(session.lookup("df_fn_0").unwrap(), address);
        assert_eq!(input.call(&session).unwrap(), value);
        assert_eq!(session.module_names(), vec!["app"]);
    }
}

/// A function that disappears from the new code stays reachable through the stable
/// entry (an old tear-off): its generation stays retained until another
/// generation implements it again.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn generation_of_removed_function_is_retained() {
    let with_both = |value: i64| {
        format!(
            "define i64 @df_fn_0() {{\n  ret i64 {value}\n}}\ndefine i64 @df_fn_1() {{\n  ret i64 {}\n}}\n",
            value + 100
        )
    };
    let only_the_first = |value: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {value}\n}}\n");
    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", &with_both(1))
        .expect("generation 1");
    let second_one = session.stable_entry("df_fn_1").expect("stable entry");
    assert_eq!(second_one.call(&session).unwrap(), 101);
    let r = session
        .hot_reload("app", &only_the_first(2))
        .expect("generation 2");
    // `df_fn_1` still calls the body of generation 1.
    assert_eq!(r.retained_generations, 2);
    assert_eq!(second_one.call(&session).unwrap(), 101);
    let r = session
        .hot_reload("app", &only_the_first(3))
        .expect("generation 3");
    // Generation 2 goes away; 1 stays because of `df_fn_1`.
    assert_eq!(r.retained_generations, 2);
    assert_eq!(second_one.call(&session).unwrap(), 101);
    let r = session
        .hot_reload("app", &with_both(4))
        .expect("generation 4");
    // Both come back in 4: 1 and 3 leave.
    assert_eq!(r.retained_generations, 1);
    assert_eq!(second_one.call(&session).unwrap(), 104);
}

/// Promotion refuses a new function with a name already defined by another module
/// before unloading the old program. The previous entry stays alive.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn collision_with_other_module_does_not_destroy_promotion() {
    let old = "define i32 @main() { ret i32 7 }\n";
    let new = "define i32 @main() { ret i32 8 }\n\
define i64 @df_fn_1() { ret i64 1 }\n";
    let mut session = JitSession::new().expect("session");
    session.add_ir_module("app", old).expect("old program");
    session
        .add_ir_module("biblioteca", "define i64 @df_fn_1() { ret i64 2 }\n")
        .expect("another resident definition");
    assert_eq!(session.run_main().unwrap().exit_code, 7);

    let error = session.hot_reload("app", new).unwrap_err();
    assert_eq!(error.stage, "contract");
    assert!(error.message.contains("df_fn_1"), "{error}");
    assert_eq!(session.run_main().unwrap().exit_code, 7);
    assert_eq!(session.generation("app"), None);
}

/// A missing external global is also a contract error prior to promotion.
/// The name is not a function declaration, so it exercises the separate
/// data check and keeps the old `main` runnable.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn missing_external_global_does_not_destroy_promotion() {
    let mut session = JitSession::new().expect("session");
    session
        .add_ir_module("app", "define i32 @main() { ret i32 7 }\n")
        .unwrap();
    assert_eq!(session.run_main().unwrap().exit_code, 7);

    let ir = "@missing = external global i64\n\
define i32 @main() {\n\
  %v = load i64, ptr @missing\n\
  %r = trunc i64 %v to i32\n\
  ret i32 %r\n}\n";
    let error = session.hot_reload("app", ir).unwrap_err();
    assert_eq!(error.stage, "contract");
    assert!(error.message.contains("missing"), "{error}");
    assert_eq!(session.run_main().unwrap().exit_code, 7);
    assert_eq!(session.generation("app"), None);
}

/// A mistake in the name does not pick the session's only module by approximation.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn unknown_name_does_not_promote_the_only_module() {
    let mut session = JitSession::new().expect("session");
    session
        .add_ir_module("app", "define i32 @main() { ret i32 7 }\n")
        .unwrap();
    let error = session
        .hot_reload("other", "define i32 @main() { ret i32 8 }\n")
        .unwrap_err();
    assert_eq!(error.stage, "contract");
    assert!(error.message.contains("other"), "{error}");
    assert_eq!(session.module_names(), vec!["app"]);
    assert_eq!(session.run_main().unwrap().exit_code, 7);
}

/// The reloadable generation has its own statics. Consecutive executions
/// of `main` reset the flag, before and after the generation swap.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn reloadable_main_resets_active_generation_statics() {
    let ir = "@dfg_0_ok = internal global i8 0\n\
define i32 @main() {\n\
  %anterior = load i8, ptr @dfg_0_ok\n\
  store i8 1, ptr @dfg_0_ok\n\
  %codigo = zext i8 %anterior to i32\n\
  ret i32 %codigo\n}\n";
    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", ir)
        .expect("generation 1");
    assert_eq!(session.run_main().unwrap().exit_code, 0);
    assert_eq!(session.run_main().unwrap().exit_code, 0);

    session.hot_reload("app", ir).expect("generation 2");
    assert_eq!(session.run_main().unwrap().exit_code, 0);
    assert_eq!(session.run_main().unwrap().exit_code, 0);
}

/// Different libraries can use the same internal `@dfg_*` name without
/// their data cells getting mixed in the shared JITDylib.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn equal_statics_in_distinct_modules_stay_independent() {
    let ir = |function: &str| {
        format!(
            "@dfg_0_ok = internal global i64 0\n\
define i64 @{function}() {{\n\
  %anterior = load i64, ptr @dfg_0_ok\n\
  %novo = add i64 %anterior, 1\n\
  store i64 %novo, ptr @dfg_0_ok\n\
  ret i64 %anterior\n}}\n"
        )
    };
    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", &ir("df_fn_0"))
        .expect("app");
    session
        .add_reloadable_module("biblioteca", &ir("df_fn_1"))
        .expect("biblioteca");
    let app = session.stable_entry("df_fn_0").expect("app entry");
    let library = session.stable_entry("df_fn_1").expect("library entry");
    assert_eq!(app.call(&session).unwrap(), 0);
    assert_eq!(library.call(&session).unwrap(), 0);
    assert_eq!(app.call(&session).unwrap(), 1);
    assert_eq!(library.call(&session).unwrap(), 1);
}

/// A body reload keeps the program's statics already initialized.
/// The dispatch cache of the previous generation must start empty in the new code.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn reload_preserves_program_statics_without_copying_caches() {
    let ir = |step: i64| {
        format!(
            "@dfg_0 = internal global i64 0\n\
@dfg_0$ok = internal global i8 0\n\
@df.ic.0 = private global [2 x i64] zeroinitializer\n\
define i64 @df_fn_0() {{\n\
  %anterior = load i64, ptr @dfg_0\n\
  %novo = add i64 %anterior, {step}\n\
  store i64 %novo, ptr @dfg_0\n\
  store i8 1, ptr @dfg_0$ok\n\
  ret i64 %novo\n}}\n\
define i64 @df_fn_1() {{\n\
  %celula = getelementptr [2 x i64], ptr @df.ic.0, i64 0, i64 0\n\
  %anterior = load i64, ptr %celula\n\
  %novo = add i64 %anterior, 1\n\
  store i64 %novo, ptr %celula\n\
  ret i64 %novo\n}}\n\
define i64 @df_fn_2() {{\n\
  %ok = load i8, ptr @dfg_0$ok\n\
  %largo = zext i8 %ok to i64\n\
  ret i64 %largo\n}}\n"
        )
    };
    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", &ir(1))
        .expect("generation 1");
    let next_offset = session.stable_entry("df_fn_0").expect("static");
    let cache = session.stable_entry("df_fn_1").expect("cache");
    let initialized = session.stable_entry("df_fn_2").expect("indicador");
    assert_eq!(next_offset.call(&session).unwrap(), 1);
    assert_eq!(next_offset.call(&session).unwrap(), 2);
    assert_eq!(cache.call(&session).unwrap(), 1);
    assert_eq!(cache.call(&session).unwrap(), 2);
    assert_eq!(initialized.call(&session).unwrap(), 1);

    session.hot_reload("app", &ir(10)).expect("generation 2");
    assert_eq!(initialized.call(&session).unwrap(), 1);
    assert_eq!(next_offset.call(&session).unwrap(), 12);
    assert_eq!(cache.call(&session).unwrap(), 1);

    let changed_type = "@dfg_0 = internal global i32 0\n\
define i64 @df_fn_0() { ret i64 99 }\n\
define i64 @df_fn_1() { ret i64 99 }\n\
define i64 @df_fn_2() { ret i64 99 }\n";
    let error = session.hot_reload("app", changed_type).unwrap_err();
    assert_eq!(error.stage, "contract");
    assert!(error.message.contains("dfg_0"), "{error}");
    assert_eq!(next_offset.call(&session).unwrap(), 22);
}

/// The current emitter names globals as `dfg.<library>.<class>.<field>`;
/// the old IR used `dfg_`. The value and the flag must cross the
/// reload using the names the compiler actually publishes.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn reload_preserves_globals_with_current_emitter_name() {
    let ir = |step: i64| {
        format!(
            "@dfg.app.C.contador = internal global i64 0\n\
@dfg.app.C.contador$ok = internal global i8 0\n\
define i64 @df_fn_0() {{\n\
  %anterior = load i64, ptr @dfg.app.C.contador\n\
  %novo = add i64 %anterior, {step}\n\
  store i64 %novo, ptr @dfg.app.C.contador\n\
  store i8 1, ptr @dfg.app.C.contador$ok\n\
  ret i64 %novo\n}}\n\
define i64 @df_fn_1() {{\n\
  %ok = load i8, ptr @dfg.app.C.contador$ok\n\
  %valor = zext i8 %ok to i64\n\
  ret i64 %valor\n}}\n"
        )
    };
    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", &ir(1))
        .expect("generation 1");
    let advance = session.stable_entry("df_fn_0").expect("advance");
    let initialized = session.stable_entry("df_fn_1").expect("indicador");
    assert_eq!(advance.call(&session).unwrap(), 1);
    assert_eq!(advance.call(&session).unwrap(), 2);
    assert_eq!(initialized.call(&session).unwrap(), 1);
    session.hot_reload("app", &ir(10)).expect("generation 2");
    assert_eq!(initialized.call(&session).unwrap(), 1);
    assert_eq!(advance.call(&session).unwrap(), 12);
}

/// An edit that starts using another DLL export publishes that name before
/// materializing the new generation. A missing name does not touch the running version.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn reload_publishes_new_dll_external() {
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    // The copy stays in the worktree target (E: on the owner's machine),
    // never in the TEMP of C:. kernel32 offers two exports without compiling the SDK.
    let dir = Directory(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("tmp-jit-sdk-export-{}", std::process::id())),
    );
    std::fs::create_dir_all(&dir.0).unwrap();
    let dll = library_with_two_exports(&dir.0, &["GetTickCount", "GetCurrentProcessId"]);

    let ir = |name: &str, fixed_return: bool| {
        format!(
            "\
declare i32 @{name}()
define i64 @df_fn_0() {{
  %valor = call i32 @{name}()
  %largo = zext i32 %valor to i64
  {}
}}
",
            if fixed_return {
                "ret i64 1"
            } else {
                "ret i64 %largo"
            }
        )
    };
    let mut session = JitSession::new_with_sdk(&dll, &["GetTickCount".to_owned()]).unwrap();
    session
        .add_reloadable_module("app", &ir("GetTickCount", true))
        .unwrap();
    let input = session.stable_entry("df_fn_0").unwrap();
    assert_eq!(input.call(&session).unwrap(), 1);

    let error = session
        .hot_reload("app", &ir("GetMissingExport", false))
        .unwrap_err();
    assert_eq!(error.stage, "contract");
    assert_eq!(session.generation("app"), Some(1));
    assert_eq!(input.call(&session).unwrap(), 1);

    session
        .hot_reload("app", &ir("GetCurrentProcessId", false))
        .unwrap();
    assert_eq!(session.generation("app"), Some(2));
    assert_eq!(input.call(&session).unwrap(), i64::from(std::process::id()));
}

/// Structural reload (J03): the signature of a function changes. The function gets
/// another stable entry (`name$sig1`) and its callers in the new code use the
/// new one; the original entry keeps the old body (an old tear-off). The
/// same signature reuses the same entry, and going back to the original reuses the
/// original.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn changed_signature_gets_new_entry() {
    let v1 = "define i64 @df_fn_0() {\n  ret i64 1\n}\n\
              define i64 @df_uses() {\n  %r = call i64 @df_fn_0()\n  ret i64 %r\n}\n";
    let v2 = |k: i64| {
        format!(
            "define i64 @df_fn_0(i64 %a) {{\n  %r = add i64 %a, {k}\n  ret i64 %r\n}}\n\
             define i64 @df_uses() {{\n  %r = call i64 @df_fn_0(i64 5)\n  ret i64 %r\n}}\n"
        )
    };
    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", v1)
        .expect("generation 1");
    let uses = session.stable_entry("df_uses").expect("entry");
    let old_value = session.stable_entry("df_fn_0").expect("entry");
    assert_eq!(uses.call(&session).unwrap(), 1);

    session
        .hot_reload("app", &v2(0))
        .expect("new signature accepted");
    assert_eq!(uses.call(&session).unwrap(), 5);
    assert_eq!(
        old_value.call(&session).unwrap(),
        1,
        "the original entry keeps the old body"
    );
    let new_area = session.stable_entry("df_fn_0$sig1").expect("new entry");
    assert_eq!(new_area.call_with(&session, 9).unwrap(), 9);

    session
        .hot_reload("app", &v2(100))
        .expect("same signature, new body");
    assert_eq!(uses.call(&session).unwrap(), 105);
    assert_eq!(new_area.call_with(&session, 9).unwrap(), 109);
    assert!(
        session.stable_entry("df_fn_0$sig2").is_err(),
        "the same signature reuses the entry"
    );

    session
        .hot_reload("app", v1)
        .expect("back to the original signature");
    assert_eq!(uses.call(&session).unwrap(), 1);
    assert_eq!(old_value.call(&session).unwrap(), 1);
}

/// J04: the new generation compiles only what changed. A function whose code (text
/// and what it reaches that is local to the generation) is that of the published implementation
/// stays in it; the calls to it, and its own, go through the trampolines, so the
/// kept caller already reaches the new body of what changed.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn unchanged_function_stays_in_live_implementation() {
    let ir = |value: i64, helper: i64| {
        format!(
            "define i64 @df_fn_0() {{\n  %r = call i64 @df_fn_1()\n  ret i64 %r\n}}\n\
             define i64 @df_fn_1() {{\n  ret i64 {value}\n}}\n\
             define i64 @df_fn_2() {{\n  %s = call i64 @auxiliar()\n  ret i64 %s\n}}\n\
             define internal i64 @auxiliar() {{\n  ret i64 {helper}\n}}\n"
        )
    };
    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", &ir(1, 7))
        .expect("generation 1");
    let caller = session.stable_entry("df_fn_0").expect("entry");
    let uses_local = session.stable_entry("df_fn_2").expect("entry");
    assert_eq!(caller.call(&session).unwrap(), 1);

    // Only `df_fn_1` changes: it is the only one compiled; the kept caller reaches it.
    let r = session.hot_reload("app", &ir(2, 7)).expect("generation 2");
    assert_eq!((r.entries, r.kept), (1, 2), "{r:?}");
    assert_eq!(caller.call(&session).unwrap(), 2);
    assert_eq!(uses_local.call(&session).unwrap(), 7);
    // Generation 1 stays: it still implements `df_fn_0` and `df_fn_2`.
    assert_eq!(r.retained_generations, 2);

    // The local function changes: whoever reaches it is compiled again.
    let r = session.hot_reload("app", &ir(2, 8)).expect("generation 3");
    assert_eq!((r.entries, r.kept), (1, 2), "{r:?}");
    assert_eq!(uses_local.call(&session).unwrap(), 8);
    assert_eq!(caller.call(&session).unwrap(), 2);
    assert_eq!(r.retained_generations, 3);

    // Nothing changed: nothing is compiled, and everything keeps answering.
    let r = session.hot_reload("app", &ir(2, 8)).expect("generation 4");
    assert_eq!((r.entries, r.kept), (0, 3), "{r:?}");
    assert_eq!(caller.call(&session).unwrap(), 2);
    assert_eq!(uses_local.call(&session).unwrap(), 8);
    // Generation 4 implements nothing; those that implement some entry stay.
    assert_eq!(r.retained_generations, 4);

    // Everything changes again: the old generations leave.
    let r = session
        .hot_reload(
            "app",
            &ir(3, 9).replace("%r = call", "%q = add i64 0, 0\n  %r = call"),
        )
        .expect("generation 5");
    assert_eq!((r.entries, r.kept), (3, 0), "{r:?}");
    assert_eq!(caller.call(&session).unwrap(), 3);
    assert_eq!(r.retained_generations, 1);
}

/// Both compiler generations republish RTI registration, including unchanged
/// initialization bodies, rather than retaining the prior generation's entry.
#[test]
#[ignore = "requires the native LLVM and runtime"]
fn current_and_legacy_rti_entries_are_republished_on_every_generation() {
    for symbols in [
        ["df.prepare_rti", "df.prepare_rti", "df.prepare_rti"],
        [
            "dartforge_rti_init",
            "dartforge_rti_init",
            "dartforge_rti_init",
        ],
        ["dartforge_rti_init", "df.prepare_rti", "df.prepare_rti"],
    ] {
        let ir = |symbol: &str| {
            format!(
                r#"
@dfg.rti_counter = internal global i64 0
define void @{symbol}() {{
  %old = load i64, ptr @dfg.rti_counter
  %new = add i64 %old, 1
  store i64 %new, ptr @dfg.rti_counter
  ret void
}}
define i64 @df_fn_0() {{
  %value = load i64, ptr @dfg.rti_counter
  ret i64 %value
}}
"#
            )
        };
        let mut session = JitSession::new().unwrap();
        session
            .add_reloadable_module("rti", &ir(symbols[0]))
            .unwrap();
        let counter = session.stable_entry("df_fn_0").unwrap();
        assert_eq!(
            counter.call(&session).unwrap(),
            0,
            "{symbols:?}: initial load"
        );
        for generation in 1..=2 {
            session
                .hot_reload("rti", &ir(symbols[generation as usize]))
                .unwrap();
            assert_eq!(
                counter.call(&session).unwrap(),
                generation,
                "{symbols:?}: reload {generation}"
            );
        }
    }
}
