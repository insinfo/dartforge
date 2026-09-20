//! Regressões da API LLVM e execução real de programas nativos nos dois modos.
use dartforge_compiler::{compile_llvm, compile_path_llvm};
use std::path::{Path, PathBuf};

/// Diretório exclusivo; o teste não remove caminhos fornecidos externamente.
struct OutputDir(PathBuf);
impl OutputDir {
    /// Reserva um diretório único mesmo em execuções simultâneas.
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dartforge-native-integration-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for OutputDir {
    /// Limpa somente o diretório criado pelo próprio teste.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Nenhuma otimização pode esconder recursos ainda sem runtime nativo.
#[test]
fn rejects_unsupported_native_features_even_when_unused() {
    let source = "extension Extra on int { int twice() { return this + this; } } void main() {}";
    let error = compile_llvm(source).unwrap_err();
    assert!(error.message.contains("LLVM"), "{error:?}");
    assert!(error.span.end <= source.len());
}

/// O pipeline LLVM deve preservar caminho e intervalo de erros em imports.
#[test]
fn imported_error_is_localized() {
    let dir = OutputDir::new();
    std::fs::write(
        dir.0.join("main.dart"),
        "import 'lib.dart'; void main() { print(value()); }",
    )
    .unwrap();
    let source = "int value() { return missing; }";
    std::fs::write(dir.0.join("lib.dart"), source).unwrap();
    let error = compile_path_llvm(&dir.0.join("main.dart")).unwrap_err();
    assert_eq!(error.path.file_name().unwrap(), "lib.dart");
    let span = error.span.unwrap();
    assert!(span.start <= span.end && span.end <= source.len());
    assert!(error.message.contains("missing"));
}

/// Confere resultados obtidos previamente com Dart 3.6.2; exige ferramentas reais.
#[test]
#[ignore = "requer clang LLVM 17+ e rustc/linker nativo no PATH ou DARTFORGE_CLANG/RUSTC"]
fn executes_native_corpus_at_o0_and_o2() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = OutputDir::new();
    for (index, fixture) in [
        "cases/arithmetic.dart",
        "cases/calls.dart",
        "cases/control_flow.dart",
        "cases/edge_cfg.dart",
        "cases/null_safety.dart",
        "cases/null_flow_loops.dart",
        "cases/coalesce_promotion.dart",
        "cases/expression_bodies.dart",
        "cases/managed_objects.dart",
        "cases/managed_lifetimes.dart",
        "cases/root_slots.dart",
        "cases/gc_root_slots.dart",
        "cases/interfaces_enums.dart",
        "modules/packages/main.dart",
        "modules/managed/main.dart",
        "modules/interfaces/main.dart",
    ]
    .iter()
    .enumerate()
    {
        let input = root.join("tests/native").join(fixture);
        let expected = std::fs::read_to_string(input.with_extension("stdout")).unwrap();
        let ir = compile_path_llvm(&input).unwrap();
        for optimize in [false, true] {
            let output = dir.0.join(format!(
                "case-{index}-{optimize}{}",
                std::env::consts::EXE_SUFFIX
            ));
            let options = dartforge_native::NativeOptions {
                optimize,
                ..Default::default()
            };
            dartforge_native::build_executable(&ir, &output, &options).unwrap();
            let actual = std::process::Command::new(&output).output().unwrap();
            assert!(actual.status.success(), "{fixture}: {:?}", actual);
            assert_eq!(
                String::from_utf8(actual.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                expected.replace("\r\n", "\n"),
                "{fixture}, optimize={optimize}"
            );
        }
    }
}

/// Confere que repetições reutilizam raízes sem perder o valor anterior de um local.
#[test]
#[ignore = "requer clang e rustc/linker nativo"]
fn root_slots_remain_bounded_across_loop_iterations() {
    let template = "void main() { String keep = 'start'; for (var i = 0; i < COUNT; i++) { String previous = keep; keep = 'left' + 'right'; if (i > 0 && previous != 'leftright') { print('lost'); return; } } print(keep); }";
    let dir = OutputDir::new();
    for optimize in [false, true] {
        let mut first_peak = None;
        for count in [300, 6000] {
            let source = template.replace("COUNT", &count.to_string());
            let ir = compile_llvm(&source).unwrap();
            let path = dir.0.join(format!(
                "roots-{optimize}-{count}{}",
                std::env::consts::EXE_SUFFIX
            ));
            dartforge_native::build_executable(
                &ir,
                &path,
                &dartforge_native::NativeOptions {
                    optimize,
                    ..Default::default()
                },
            )
            .unwrap();
            for stress in [false, true] {
                let mut command = std::process::Command::new(&path);
                command.env("DARTFORGE_GC_STATS", "1");
                if stress {
                    command.env("DARTFORGE_GC_STRESS", "1");
                } else {
                    command.env_remove("DARTFORGE_GC_STRESS");
                }
                let output = command.output().unwrap();
                assert!(output.status.success(), "{output:?}");
                assert_eq!(
                    String::from_utf8(output.stdout).unwrap().replace('\r', ""),
                    "leftright\n"
                );
                let report: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
                let stats = &report["dartforge_gc"];
                let peak = stats["peak_root_slots"].as_u64().unwrap();
                assert!(peak > 0 && peak < 128, "{stats}");
                assert_eq!(
                    *first_peak.get_or_insert(peak),
                    peak,
                    "roots grew with iterations or mode: {stats}"
                );
                assert_eq!(stats["root_slots"].as_u64(), Some(0));
                assert_eq!(stats["live_roots"].as_u64(), Some(0));
                assert!(stats["allocations"].as_u64().unwrap() >= count as u64);
                assert!(stats["reclaimed"].as_u64().unwrap() > 0, "{stats}");
                assert!(stats["reserved_slots"].as_u64().unwrap() < 1024, "{stats}");
            }
        }
    }
}

/// Uma asserção nula termina antes do próximo efeito, sem UB em O0 ou O2.
#[test]
#[ignore = "requer clang e rustc/linker nativo"]
fn null_assert_failure_stops_execution() {
    let source =
        "int? absent() { print(7); return null; } void main() { print(absent()!); print(99); }";
    let ir = compile_llvm(source).unwrap();
    let dir = OutputDir::new();
    for optimize in [false, true] {
        let output = dir.0.join(format!(
            "null-failure-{optimize}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let options = dartforge_native::NativeOptions {
            optimize,
            ..Default::default()
        };
        dartforge_native::build_executable(&ir, &output, &options).unwrap();
        let actual = std::process::Command::new(&output).output().unwrap();
        assert_eq!(actual.status.code(), Some(101));
        assert_eq!(
            String::from_utf8(actual.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            "7\n"
        );
        assert!(
            String::from_utf8(actual.stderr)
                .unwrap()
                .contains("Null check operator used on a null value")
        );
    }
}

/// As coerções do backend não podem aceitar acessos nullable rejeitados pela linguagem.
#[test]
fn nullable_semantic_errors_are_not_hidden_by_runtime_checks() {
    for source in [
        "int f(int? n) { return n + 1; } void main() {}",
        "void f(bool? b) { if (b) { print(1); } } void main() {}",
        "int f() { return null; } void main() {}",
        "void f(int? n) { if (n != null) { n = null; print(n + 1); } } void main() {}",
        "void main() { int? n = true; print(n); }",
    ] {
        assert!(compile_llvm(source).is_err(), "{source}");
    }
}
