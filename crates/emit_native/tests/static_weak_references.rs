//! Native weak references follow AOT reachability while reloadable IR keeps VM calls.
use dartforge_emit_native::{CompileOptions, DartFlags};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn sdk() -> PathBuf {
    let root = PathBuf::from(std::env::var_os("DARTFORGE_DART_SDK").expect("DARTFORGE_DART_SDK"));
    assert_eq!(
        std::fs::read_to_string(root.join("version"))
            .unwrap()
            .trim(),
        "3.6.2"
    );
    root
}

fn dart(root: &Path) -> PathBuf {
    root.join("bin")
        .join(if cfg!(windows) { "dart.exe" } else { "dart" })
}

fn options(sdk: &Path, optimize: bool) -> CompileOptions<'_> {
    CompileOptions {
        contracts: dartforge_emit_native::contracts::ContractMode::Verify,
        sdk: Some(sdk),
        packages: None,
        timings: false,
        optimize,
        defines: Vec::new(),
        language_version: None,
        experiments: Vec::new(),
        debug: false,
        generator: None,
        cpu: None,
    }
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn large_stack<T: Send + 'static>(run: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap()
}

#[test]
#[ignore = "requires the host Dart 3.6.2 SDK and the native LLVM/linker toolchain"]
fn aot_weak_references_keep_identity_hash_types_and_strong_uses() {
    let root = sdk();
    let directory = tempfile::tempdir().unwrap();
    for (name, source) in [
        (
            "basic",
            include_str!("fixtures/weak_reference_contracts.dart"),
        ),
        (
            "advanced",
            include_str!("fixtures/weak_reference_advanced.dart"),
        ),
        (
            "locals",
            include_str!("fixtures/weak_reference_locals.dart"),
        ),
    ] {
        let input = directory.path().join(format!("{name}.dart"));
        std::fs::write(&input, source).unwrap();
        let reference = directory.path().join(format!("{name}-reference.exe"));
        success(
            &Command::new(dart(&root))
                .args(["compile", "exe"])
                .arg(&input)
                .arg("-o")
                .arg(&reference)
                .output()
                .unwrap(),
        );
        let expected = Command::new(&reference).output().unwrap();
        success(&expected);
        let candidate = directory.path().join(format!("{name}-candidate.exe"));
        let output = candidate.clone();
        let sdk = root.join("lib");
        large_stack(move || {
            dartforge_emit_native::compile_with_flags(
                &input,
                &output,
                &options(&sdk, true),
                DartFlags::PRODUCTION,
            )
        })
        .unwrap();
        let actual = Command::new(candidate).output().unwrap();
        success(&actual);
        assert_eq!(actual.stdout, expected.stdout, "{name}");
        assert_eq!(actual.stderr, expected.stderr, "{name}");
    }
}

#[test]
#[ignore = "requires the host Dart 3.6.2 SDK and the native LLVM/linker toolchain"]
fn reloadable_weak_references_preserve_vm_arguments_and_wrapper_bodies() {
    let root = sdk();
    let directory = tempfile::tempdir().unwrap();
    for (name, source) in [
        (
            "direct",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x)=>x; int target()=>7; void main() { final ref=weak(target); if(ref==null || ref()!=7) throw StateError('lost JIT target'); print('JIT weak target retained'); }",
        ),
        (
            "local",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x)=>x; int target()=>7; void main() { const alias=target; final ref=weak(alias); if(ref==null || ref()!=7) throw StateError('lost JIT local'); print('JIT local weak alias ok'); }",
        ),
        (
            "body",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x) { print('body executed'); return null; } int target()=>7; void main() { if(weak(target)!=null) throw StateError('body result'); print('JIT weak body ok'); }",
        ),
    ] {
        let input = directory.path().join(format!("{name}.dart"));
        std::fs::write(&input, source).unwrap();
        let expected = Command::new(dart(&root)).arg(&input).output().unwrap();
        success(&expected);
        let candidate = directory.path().join(format!("{name}-reloadable.exe"));
        let output = candidate.clone();
        let sdk = root.join("lib");
        large_stack(move || {
            let ir =
                dartforge_emit_native::emit_reloadable_ir(&input, &options(&sdk, false), None)?;
            // Link the exact reloadable IR to make liveness and body behavior
            // observable without requiring a second copy of the JIT crate.
            let driver = dartforge_emit_native::driver::NativeDriverOptions {
                sdk_lib: Some(ir.sdk_lib),
                flags: DartFlags::DEVELOPMENT,
                ..Default::default()
            };
            dartforge_emit_native::driver::compile_and_link(&ir.text, &output, &driver).map(|_| ())
        })
        .unwrap();
        let actual = Command::new(candidate).output().unwrap();
        success(&actual);
        assert_eq!(actual.stdout, expected.stdout, "{name}");
        assert_eq!(actual.stderr, expected.stderr, "{name}");
    }
}

#[test]
#[ignore = "requires the host Dart 3.6.2 SDK"]
fn native_weak_declarations_and_targets_are_validated_in_both_modes() {
    let root = sdk();
    let directory = tempfile::tempdir().unwrap();
    for (name, source) in [
        (
            "instance",
            "class C { @pragma('weak-tearoff-reference') Function? weak(Function? x)=>x; } void main() {}",
        ),
        (
            "null",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x)=>x; void main() { weak(null); }",
        ),
        (
            "constructor",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x)=>x; class C {} void main() { weak(C.new); }",
        ),
        (
            "cast",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x)=>x; int target()=>1; void main() { weak(target as Function); }",
        ),
        (
            "conditional",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x)=>x; int a()=>1; int b()=>2; void main() { weak(true?a:b); }",
        ),
        (
            "parameter",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x)=>x; int target([int x=1])=>x; void main() { weak(target); }",
        ),
        (
            "local-final",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x)=>x; int target()=>1; void main() { final alias=target; weak(alias); }",
        ),
    ] {
        let input = directory.path().join(format!("{name}.dart"));
        std::fs::write(&input, source).unwrap();
        assert!(
            !Command::new(dart(&root))
                .arg(&input)
                .output()
                .unwrap()
                .status
                .success(),
            "{name}"
        );
        for reloadable in [false, true] {
            let entry = input.clone();
            let sdk = root.join("lib");
            let error = large_stack(move || {
                let options = options(&sdk, !reloadable);
                let result = if reloadable {
                    dartforge_emit_native::emit_reloadable_ir(&entry, &options, None)
                } else {
                    dartforge_emit_native::emit_ir_with_flags(
                        &entry,
                        &options,
                        DartFlags::PRODUCTION,
                    )
                };
                match result {
                    Ok(_) => panic!("invalid weak use must fail before pruning"),
                    Err(error) => error,
                }
            });
            assert!(
                error.contains("invalid static weak reference"),
                "{name}: {error}"
            );
        }
    }
}

#[test]
#[ignore = "requires the host Dart 3.6.2 SDK and the native LLVM/linker toolchain"]
fn weak_retention_respects_resolved_vm_entry_point_values() {
    let root = sdk();
    let directory = tempfile::tempdir().unwrap();
    for (name, extra, annotation) in [
        ("direct", "", "@core.pragma('vm:entry-point')"),
        (
            "type-alias",
            "typedef Entry = core.pragma;",
            "@Entry('vm:entry-point')",
        ),
        (
            "constant-name",
            "const name = 'vm:' + 'entry-point';",
            "@core.pragma(name)",
        ),
        (
            "constant-marker",
            "const marker = core.pragma('vm:entry-point');",
            "@marker",
        ),
        ("disabled", "", "@core.pragma('vm:entry-point', false)"),
        ("enabled", "", "@core.pragma('vm:entry-point', true)"),
        (
            "custom",
            "class pragma { const pragma(core.String name); }",
            "@pragma('vm:entry-point')",
        ),
        (
            "unknown-suffix",
            "",
            "@core.pragma('vm:entry-point-invalid')",
        ),
    ] {
        let input = directory.path().join(format!("{name}.dart"));
        std::fs::write(&input, format!(
            "import 'dart:core' as core; {extra}\n@core.pragma('weak-tearoff-reference') core.Function? weak(core.Function? value)=>value;\n{annotation} core.int target()=>7;\nvoid main() {{ core.print(weak(target)==null); }}"
        )).unwrap();
        let reference = directory.path().join(format!("{name}-reference.exe"));
        success(
            &Command::new(dart(&root))
                .args(["compile", "exe"])
                .arg(&input)
                .arg("-o")
                .arg(&reference)
                .output()
                .unwrap(),
        );
        let expected = Command::new(&reference).output().unwrap();
        success(&expected);
        let candidate = directory.path().join(format!("{name}-candidate.exe"));
        let output = candidate.clone();
        let sdk = root.join("lib");
        large_stack(move || {
            dartforge_emit_native::compile_with_flags(
                &input,
                &output,
                &options(&sdk, true),
                DartFlags::PRODUCTION,
            )
        })
        .unwrap();
        let actual = Command::new(candidate).output().unwrap();
        success(&actual);
        assert_eq!(actual.stdout, expected.stdout, "{name}");
        assert_eq!(actual.stderr, expected.stderr, "{name}");
    }
}
