//! The three defects that the JIT × AOT differential (`crates/jit/tests/execution.rs`)
//! found in the native emitter, each one pinned by the IR that the representation
//! contract produces (docs/NATIVO-PLANO.md §6). Without Clang: emission only.

use dartforge_emit_native::{CompileOptions, emit_ir_with};
use std::path::Path;

const SDK: &str = "C:/tools/dartsdk-3.6.2/lib";

/// The complete IR of the program, or `None` without the SDK on the machine.
fn ir_of(source: &str) -> Option<String> {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB").unwrap_or_else(|_| SDK.to_string());
    if !Path::new(&sdk).join("libraries.json").is_file() {
        eprintln!("SDK missing at {sdk}; test skipped");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("main.dart");
    std::fs::write(&entry, source).unwrap();
    let ir = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let options = CompileOptions {
                sdk: Some(Path::new(&sdk)),
                packages: None,
                timings: false,
                optimize: false,
                language_version: None,
                experiments: Vec::new(),
                debug: false,
                generator: None,
                cpu: None,
            };
            // With the source SDK (the only mode since the unified space,
            // docs/NATIVO-ESPACO-UNIFICADO.md §4.7).
            emit_ir_with(&entry, &options)
                .unwrap_or_else(|e| panic!("did not compile:\n{e}"))
                .text
        })
        .unwrap()
        .join()
        .unwrap();
    Some(ir)
}

/// The IR of the program's `dart_main`, or `None` without the SDK on the machine.
fn main_of(source: &str) -> Option<String> {
    let ir = ir_of(source)?;
    let ini = ir.find("define void @dart_main(").expect("dart_main");
    let end = ir[ini..].find("\n}\n").map_or(ir.len(), |f| ini + f);
    Some(ir[ini..end].to_string())
}

fn source_ir(source: &str) -> Option<String> {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB").unwrap_or_else(|_| SDK.to_string());
    if !Path::new(&sdk).join("libraries.json").is_file() { return None; }
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("main.dart");
    std::fs::write(&entry, source).unwrap();
    Some(std::thread::Builder::new().stack_size(64 << 20).spawn(move || {
        let options = CompileOptions { sdk: Some(Path::new(&sdk)), packages: None, timings: false, optimize: false, language_version: None, experiments: Vec::new(), debug: false, generator: None, cpu: None };
        emit_ir_with(&entry, &options).unwrap_or_else(|e| panic!("did not compile:\n{e}")).text
    }).unwrap().join().unwrap())
}

#[test]
fn symbol_literal_uses_sdk_class_and_canonical_constant() {
    let Some(ir) = source_ir("void main() { final a = #foo; final b = #foo; print(identical(a, b)); print(a); print(const Symbol('foo') == a); print(#_privado); }\n") else { return };
    let main = ir.split("define void @dart_main(").nth(1).expect("main");
    let main = main.split("\n}\n").next().expect("end of main");
    // Both occurrences of `#foo` go through the same getter of the canonical
    // constant `@dfc.<hash>.get`: called, or copied into `main` by the inliner
    // (the copy keeps the object with `dartforge_mark_constant` and the key of the
    // getter).
    let getters: Vec<&str> = main
        .match_indices("@dfc.")
        .map(|(i, _)| {
            let rest = &main[i..];
            &rest[..rest.find(".get").expect("constant getter") + 4]
        })
        .collect();
    let foo = getters.first().copied().expect("constant of #foo");
    assert!(getters.iter().filter(|g| **g == foo).count() >= 2, "literal not canonicalized:\n{main}");
    assert!(ir.contains("@dartforge_object_new"), "Symbol not allocated as an SDK object");
}

/// The arity error of a closure must be the real SDK class and have a
/// `toString` usable by the program; the detailed text of the VM still varies
/// with the name and the signature of the function.
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn closure_arity_throws_no_such_method_error_from_source() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("arity_nsm.dart");
    let exe = dir.path().join("arity_nsm.exe");
    std::fs::write(&entry, include_str!("fixtures/arity_nsm.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"), "true\ntrue\n");
}

/// `C.getter(args)` evaluates the getter before the arguments, then calls the
/// returned closure. Ordinary static methods keep the direct call.
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn static_getter_returning_function_is_called_as_value() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("static_getter_function.dart");
    let exe = dir.path().join("static_getter_function.exe");
    std::fs::write(&entry, include_str!("fixtures/static_getter_function.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "getter\narg\n42\n6\ngetter\n8\n");
}

/// A redirecting factory passes the types of the destination's annotation, even
/// when it changes class and permutes the type parameters.
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn redirecting_factory_preserves_target_rti() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("redirect_factory_rti.dart");
    let exe = dir.path().join("redirect_factory_rti.exe");
    std::fs::write(&entry, include_str!("fixtures/redirect_factory_rti.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "true\nMemory<String>\ntrue\nPairImpl<String, int>\n");
}

/// The abstract getter of an interface may be implemented by a field or
/// by an explicit getter; each object chooses its own implementation.
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn interface_getter_dispatches_field_and_getter() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("interface_field_getter.dart");
    let exe = dir.path().join("interface_field_getter.exe");
    std::fs::write(&entry, include_str!("fixtures/interface_field_getter.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "r2: bip r2\nloro: currupaco\n");
}

/// The reified argument of the collection, and not the type of the covariant reference,
/// protects the mutation before changing a List, Map or Set.
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn covariant_collection_write_throws_type_error_without_mutating() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("covariant_collection_write.dart");
    let exe = dir.path().join("covariant_collection_write.exe");
    std::fs::write(&entry, include_str!("fixtures/covariant_collection_write.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "true\n[1]\ntrue\n{a: 1}\ntrue\n{1}\n");
}

/// The dynamic call must check the nominal type of the SDK parameter before
/// passing an `int` as a handle to the `String_concat` native.
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn wrong_dynamic_sdk_argument_throws_type_error() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("dynamic_sdk_string_argument.dart");
    let exe = dir.path().join("dynamic_sdk_string_argument.exe");
    std::fs::write(&entry, include_str!("fixtures/dynamic_sdk_string_argument.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"), "foobar\ntrue\nfoo\n");
}

/// `Iterable.generate<E>` uses `id is E Function(int)` to decide whether the
/// implicit integer generator can serve `E`.
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn iterable_generate_checks_generic_signature_in_sdk() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("iterable_generate_function_rti.dart");
    let exe = dir.path().join("iterable_generate_function_rti.exe");
    std::fs::write(&entry, include_str!("fixtures/iterable_generate_function_rti.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "[0, 1, 2]\ntrue\n[v0, v1]\n");
}

/// Implicit `name`/`index` (`$name`, `index`, `this.name`) in the body of a
/// member of a program enum (specification §13 "Enums"): the resolved
/// element is the implicit getter, without a body — the value lives in the two implicit
/// fields of the object (positions 1 and 0), the same ones that the explicit access
/// (`E.a.name`, `enum_member`) reads. Without Clang: emission only.
#[test]
fn implicit_enum_name_index_reads_value_fields() {
    let source = "enum E { a, b; String d() => 'v=$name'; int i() => index + 1; }\n\
        void main() { print(E.a.d()); print(E.b.i()); }\n";
    let Some(ir) = source_ir(source) else { return };
    // The field is read inline, from the body of the object (the header at `h - 2`,
    // the fields 16 bytes after, in the object space of the runtime): the
    // word at index 1 or 0.
    assert!(
        ir.contains(", i64 16\n") && ir.contains("getelementptr inbounds i64, ptr %fp") && ir.contains(", i64 1\n"),
        "the implicit $name reads field 1:\n{ir}"
    );
    assert!(ir.contains(", i64 0\n"), "the implicit index reads field 0:\n{ir}");
}

/// The program above executed (AOT with the source SDK): implicit and explicit
/// give the same text as the VM.
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn implicit_enum_name_index_in_sdk() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("enum_name_index_implicito.dart");
    let exe = dir.path().join("enum_name_index_implicito.exe");
    std::fs::write(&entry, include_str!("fixtures/enum_name_index_implicito.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "terra:1\nmercurio:0\nterra\n0\nterra\n0\n");
}

/// Implicit `values` (without the `E.` prefix) in the body of an enum member:
/// the list of the values, like the explicit path. The implicit getter has no
/// body — before, a call to a never-defined symbol came out, and the
/// link failed. Without Clang: the emission leaves no call to `.values()`.
#[test]
fn implicit_enum_values_becomes_value_list() {
    let source = "enum E { a, b; E get proximo => values[(index + 1) % 2]; static E primeiro() => values.first; }\n\
        void main() { print(E.a.proximo); print(E.primeiro()); }\n";
    let Some(ir) = source_ir(source) else { return };
    let calls = ir
        .lines()
        .filter(|l| l.contains("call ") && l.contains(".values("))
        .collect::<Vec<_>>();
    assert!(calls.is_empty(), "call to values without a define:\n{calls:?}\n{ir}");
}

/// The program above executed (AOT with the source SDK).
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn implicit_enum_values_in_sdk() {    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("enum_values_implicito.dart");
    let exe = dir.path().join("enum_values_implicito.exe");
    std::fs::write(&entry, include_str!("fixtures/enum_values_implicito.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "E.b\nE.a\nE.a\n2\n");
}

/// The default `toString()` of an instance of a generic class includes the arguments
/// (`Instance of 'Caixa<int>'`, like the VM); without arguments, the registered
/// name, as before. Only execution proves it (the runtime builds the text).
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn generic_class_default_tostring_in_sdk() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("classe_generica_tostring_padrao.dart");
    let exe = dir.path().join("classe_generica_tostring_padrao.exe");
    std::fs::write(&entry, include_str!("fixtures/classe_generica_tostring_padrao.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "true\nInstance of 'Caixa<String>'\nInstance of 'Caixa<Caixa<int>>'\ntrue\n");
}

/// A static `noSuchMethod` forwarder of a method with positionals only
/// (cases 57/216/223, first slice): `p.saudacao('world')` with an abstract
/// `Servico.saudacao` and `Proxy.noSuchMethod` builds
/// `Invocation.method(#saudacao, ['world'])` and calls the nsm — before it was
/// "chamada de membro sem implementação compilada". Without Clang: emission only.
#[test]
fn nsm_forwarder_positional_method_emits_nsm() {
    let source = "abstract class Servico { String saudacao(String quem); }\n\
        class Proxy implements Servico {\n\
        \x20 dynamic noSuchMethod(Invocation i) => 'nsm(${i.positionalArguments.join(',')})';\n\
        }\n\
        void main() { final p = Proxy(); print(p.saudacao('mundo')); }\n";
    let Some(ir) = source_ir(source) else { return };
    assert!(ir.contains("noSuchMethod"), "no call to nsm:\n{ir}");
}

/// The program above executed (AOT with the source SDK): the output matches VM 3.6.2.
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn nsm_forwarder_method_in_sdk() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("nsm_encaminhador_metodo.dart");
    let exe = dir.path().join("nsm_encaminhador_metodo.exe");
    std::fs::write(&entry, include_str!("fixtures/nsm_encaminhador_metodo.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "nsm(mundo)\nSymbol(\"saudacao\")\n");
}

/// A static `noSuchMethod` forwarder of a getter (case 216, `p.version`):
/// the abstract getter without a concrete implementation builds
/// `Invocation.getter(#version)` and calls the nsm — before it was "chamada de
/// membro sem implementação compilada". Without Clang: emission only.
#[test]
fn nsm_forwarder_getter_emits_nsm() {
    let source = "abstract class Servico { int get versao; }\n\
        class Proxy implements Servico {\n\
        \x20 dynamic noSuchMethod(Invocation i) => 42;\n\
        }\n\
        void main() { final p = Proxy(); print(p.versao); }\n";
    let Some(ir) = source_ir(source) else { return };
    assert!(ir.contains("noSuchMethod"), "no call to nsm:\n{ir}");
}

/// The program above executed (AOT with the source SDK): the output matches VM 3.6.2.
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn nsm_forwarder_getter_in_sdk() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("nsm_encaminhador_getter.dart");
    let exe = dir.path().join("nsm_encaminhador_getter.exe");
    std::fs::write(&entry, include_str!("fixtures/nsm_encaminhador_getter.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "42\n");
}

/// A program enum is a subtype of the SDK `Enum` (specification §13): the
/// edge goes into the registration of the program module, and `is`/`as` see it.
#[test]
#[ignore = "AOT fixture with the source SDK and LLVM; run in the Pesado workflow"]
fn enum_is_subtype_of_enum_in_sdk() {    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("test SDK");
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("enum_e_subtipo_de_enum.dart");
    let exe = dir.path().join("enum_e_subtipo_de_enum.exe");
    std::fs::write(&entry, include_str!("fixtures/enum_e_subtipo_de_enum.dart")).unwrap();
    let exe_for_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with(&entry, &exe_for_thread, &options)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "true\nfalse\ntrue\nfalse\n");
}

/// The separate getter prevents `late String x = x` from expanding its own AST
/// indefinitely; the test also pins the per-object reentrancy check.
#[test]
fn self_referential_late_field_emits_guarded_getter() {
    let Some(ir) = ir_of("class C { late String x = x; }\nvoid main() { print(C().x); }\n") else { return };
    assert!(ir.contains("@dartforge_late_field_initializing"), "{ir}");
    assert!(ir.contains("@dartforge_stack_overflow_error_new"), "{ir}");
    assert!(ir.contains("@dartforge_late_field_mark_initialized"), "{ir}");
}

/// (1) `for` whose variable is reassigned in the body: the condition rereads its
/// value on every iteration — before it was an SSA value in the by-name map, and the
/// condition always saw the initial one. The local lives in an `alloca` (R6), which
/// the `mem2reg` of the HIR optimizer promotes: the read is a `phi` that joins the
/// value from before the `if` and the `i = 7`, and it is what `print` and `i++` use.
#[test]
fn for_with_reassigned_variable_reads_local() {
    let Some(main) = main_of(
        "void main() {\n  for (var i = 0; i < 10; i++) {\n    if (i == 3) i = 7;\n    print(i);\n  }\n}\n",
    ) else {
        return;
    };
    assert!(main.matches("= phi i64").count() >= 2, "the loop round trip and the `if` join the value of the local:\n{main}");
    assert!(main.contains("add i64 0, 7"), "the `i = 7` reaches the `phi`:\n{main}");
}

/// (2) `c.double_of()`: the call of the method through the resolved element (N1) — before it
/// fell into the `Const(Int(0))` of "unrecognized call" and became `print(0)`.
#[test]
fn method_call_with_return() {
    let Some(main) = main_of(
        "class C {\n  int v;\n  C(this.v);\n  int dobro() => v * 2;\n}\nvoid main() {\n  final c = C(21);\n  print(c.dobro());\n}\n",
    ) else {
        return;
    };
    // The method (and the object, which does not escape) vanishes in the HIR optimizer:
    // `v * 2` remains, with the `v` of the constructor.
    assert!(main.contains(".dobro(i64") || main.contains("mul i64"), "{main}");
}

/// (3) `print(sum(40, 2))` with an `int` return: the result is an `I64` (R1), and the
/// SDK `print` receives it in a `Ref` position — before it went to `print_handle`
/// as if it were a handle.
#[test]
fn top_level_function_with_int_return_prints_scalar() {
    let Some(main) =
        main_of("int soma(int a, int b) => a + b;\nvoid main() {\n  print(soma(40, 2));\n}\n")
    else {
        return;
    };
    // The call, or its body inlined by the HIR optimizer.
    assert!(main.contains("= call i64 @df.") || main.contains("add i64 %v"), "{main}");
    assert!(!main.contains("@dartforge_print_handle"), "{main}");
}
