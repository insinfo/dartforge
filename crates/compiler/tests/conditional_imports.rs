//! Diretivas condicionais: seleção, filtros, ciclos e cache; oráculo Dart 3.6.2.
use dartforge_compiler::{
    CompilationEnvironment, CompileOptions, CompilerSession, Optimization,
    compile_path_llvm_with_environment, compile_path_with_environment,
};
use dartforge_packages::load_with_environment;
use std::path::PathBuf;

const FILES: &[(&str, &str)] = &[
    (
        "wasm.dart",
        include_str!("../../../tests/conformance/modules/conditional15/wasm.dart"),
    ),
    (
        ".dart_tool/package_config.json",
        include_str!(
            "../../../tests/conformance/modules/conditional15/.dart_tool/package_config.json"
        ),
    ),
    (
        "absent.dart",
        include_str!("../../../tests/conformance/modules/conditional15/absent.dart"),
    ),
    (
        "cycle_a.dart",
        include_str!("../../../tests/conformance/modules/conditional15/cycle_a.dart"),
    ),
    (
        "cycle_b.dart",
        include_str!("../../../tests/conformance/modules/conditional15/cycle_b.dart"),
    ),
    (
        "export_default.dart",
        include_str!("../../../tests/conformance/modules/conditional15/export_default.dart"),
    ),
    (
        "export_javascript.dart",
        include_str!("../../../tests/conformance/modules/conditional15/export_javascript.dart"),
    ),
    (
        "export_native.dart",
        include_str!("../../../tests/conformance/modules/conditional15/export_native.dart"),
    ),
    (
        "exports.dart",
        include_str!("../../../tests/conformance/modules/conditional15/exports.dart"),
    ),
    (
        "fallback.dart",
        include_str!("../../../tests/conformance/modules/conditional15/fallback.dart"),
    ),
    (
        "flavor_blue.dart",
        include_str!("../../../tests/conformance/modules/conditional15/flavor_blue.dart"),
    ),
    (
        "flavor_default.dart",
        include_str!("../../../tests/conformance/modules/conditional15/flavor_default.dart"),
    ),
    (
        "javascript.dart",
        include_str!("../../../tests/conformance/modules/conditional15/javascript.dart"),
    ),
    (
        "lib/choice.dart",
        include_str!("../../../tests/conformance/modules/conditional15/lib/choice.dart"),
    ),
    (
        "lib/javascript_choice.dart",
        include_str!("../../../tests/conformance/modules/conditional15/lib/javascript_choice.dart"),
    ),
    (
        "lib/native_choice.dart",
        include_str!("../../../tests/conformance/modules/conditional15/lib/native_choice.dart"),
    ),
    (
        "main.dart",
        include_str!("../../../tests/conformance/modules/conditional15/main.dart"),
    ),
    (
        "native.dart",
        include_str!("../../../tests/conformance/modules/conditional15/native.dart"),
    ),
];
const EXPECTED_JS: &str =
    "javascript\nabsent=false\nexport javascript\npackage javascript\ndefault\n3\n";

/// Isola alterações de fontes, arestas e configuração dos demais testes.
struct Fixture(PathBuf);
impl Fixture {
    /// Materializa cópias independentes das fontes e da configuração de pacotes.
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dartforge-conditional15-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        for (relative, source) in FILES {
            let target = path.join(relative);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, source).unwrap();
        }
        Self(path)
    }
    /// Retorna a entrada principal do grafo temporário.
    fn entry(&self) -> PathBuf {
        self.0.join("main.dart")
    }
    /// Altera somente uma fonte pertencente a este fixture.
    fn write(&self, name: &str, source: &str) {
        std::fs::write(self.0.join(name), source).unwrap();
    }
}
impl Drop for Fixture {
    /// Remove apenas o diretório exclusivo reservado por este teste.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Confere os três perfis, a prioridade dos ramos e a deduplicação de ciclos.
#[test]
fn profiles_select_first_match_and_ignore_inactive_invalid_uris() {
    let fixture = Fixture::new();
    for (environment, selected, exported, package) in [
        (
            CompilationEnvironment::javascript(),
            "javascript.dart",
            "export_javascript.dart",
            "lib/javascript_choice.dart",
        ),
        (
            CompilationEnvironment::native(),
            "native.dart",
            "export_native.dart",
            "lib/native_choice.dart",
        ),
        (
            CompilationEnvironment::wasm(),
            "wasm.dart",
            "export_javascript.dart",
            "lib/javascript_choice.dart",
        ),
    ] {
        let graph = load_with_environment(&fixture.entry(), &environment).unwrap();
        for name in [
            selected,
            exported,
            package,
            "cycle_a.dart",
            "cycle_b.dart",
            "flavor_default.dart",
        ] {
            let path = fixture.0.join(name).canonicalize().unwrap();
            assert!(graph.units.iter().any(|unit| unit.path == path), "{name}");
        }
        assert_eq!(graph.environment, environment);
        assert_eq!(
            graph.units.len(),
            9,
            "somente bibliotecas selecionadas, ciclo deduplicado"
        );
        assert_eq!(
            graph,
            load_with_environment(&fixture.entry(), &environment).unwrap()
        );
    }
}

/// Diferencia nomes comuns vazios de bibliotecas reservadas desconhecidas.
#[test]
fn absent_custom_empty_value_matches_but_unknown_reserved_name_does_not() {
    let fixture = Fixture::new();
    fixture.write("main.dart", "import 'missing.dart' if (custom.flag == '') 'absent.dart'; import 'cycle_a.dart' if (dart.library.unknown == '') 'missing.dart'; void main(){print(absentLabel());print(cycleValue());}");
    for env in [
        CompilationEnvironment::javascript(),
        CompilationEnvironment::native(),
        CompilationEnvironment::wasm(),
    ] {
        load_with_environment(&fixture.entry(), &env).unwrap();
    }
}

/// Mantém os filtros dos exports e rejeita emissão com um perfil incompatível.
#[test]
fn selected_export_filters_and_backend_profiles_are_enforced() {
    let fixture = Fixture::new();
    let options = CompileOptions::default();
    let js = CompilationEnvironment::javascript();
    let native = CompilationEnvironment::native();
    let wasm = CompilationEnvironment::wasm();
    compile_path_with_environment(&fixture.entry(), options, &js).unwrap();
    compile_path_llvm_with_environment(&fixture.entry(), options, &native).unwrap();
    for wrong in [&native, &wasm] {
        assert!(compile_path_with_environment(&fixture.entry(), options, wrong).is_err());
    }
    for wrong in [&js, &wasm] {
        assert!(compile_path_llvm_with_environment(&fixture.entry(), options, wrong).is_err());
    }
    fixture.write(
        "main.dart",
        "import 'exports.dart'; void main(){print(hiddenValue());}",
    );
    assert!(compile_path_with_environment(&fixture.entry(), options, &js).is_err());
}

/// Exige diagnósticos localizados para URIs ativas inválidas e sintaxe incorreta.
#[test]
fn selected_errors_and_malformed_conditions_have_source_spans() {
    let fixture = Fixture::new();
    let env = CompilationEnvironment::javascript();
    for source in [
        "import 'absent.dart' if (dart.library.js_interop) 'missing.dart'; void main(){}",
        "import 'absent.dart' if (dart.library.js_interop) 'dart:unsupported'; void main(){}",
        "import 'absent.dart' if () 'native.dart'; void main(){}",
        "import 'absent.dart' if (dart.library.io = 'true') 'native.dart'; void main(){}",
        "import 'absent.dart' if (dart.library.io == true) 'native.dart'; void main(){}",
    ] {
        fixture.write("main.dart", source);
        let error = load_with_environment(&fixture.entry(), &env).unwrap_err();
        assert_eq!(
            error.path.canonicalize().unwrap(),
            fixture.entry().canonicalize().unwrap()
        );
        let span = error.span.expect("diretiva deve identificar intervalo");
        assert!(span.start < span.end && span.end <= source.len());
    }
}

/// Reutiliza apenas grafos idênticos e invalida a saída anterior após falhas.
#[test]
fn cache_observes_selected_sources_and_edges_and_clears_after_errors() {
    let fixture = Fixture::new();
    let env = CompilationEnvironment::javascript();
    let options = CompileOptions::default();
    let mut session = CompilerSession::new();
    let mut compile = |env: &CompilationEnvironment| {
        session.compile_path_with_environment(&fixture.entry(), options, env)
    };
    let initial = compile(&env).unwrap();
    assert!(!initial.stats.cache_hit);
    assert!(compile(&env).unwrap().stats.cache_hit);
    fixture.write("native.dart", "String label(){return 'unused edit';}");
    assert!(compile(&env).unwrap().stats.cache_hit);
    fixture.write("javascript.dart", "String label(){return 'updated';}");
    let changed = compile(&env).unwrap();
    assert!(!changed.stats.cache_hit);
    assert_ne!(initial.javascript, changed.javascript);
    assert!(compile(&env).unwrap().stats.cache_hit);
    assert!(compile(&CompilationEnvironment::native()).is_err());
    assert!(!compile(&env).unwrap().stats.cache_hit);
    std::fs::remove_file(fixture.0.join("javascript.dart")).unwrap();
    assert!(compile(&env).is_err());
    fixture.write("javascript.dart", "String label(){return 'restored';}");
    assert!(!compile(&env).unwrap().stats.cache_hit);
    let main = FILES
        .iter()
        .find(|(name, _)| *name == "main.dart")
        .unwrap()
        .1;
    fixture.write(
        "main.dart",
        &main.replace(
            "if (dart.library.io) 'native.dart'",
            "if (dart.library.js_interop) 'native.dart'",
        ),
    );
    let redirected = compile(&env).unwrap();
    assert!(!redirected.stats.cache_hit);
    assert!(redirected.javascript.contains("unused edit"));
}

/// As quatro combinações de passes devem reproduzir o stdout oficial do dart2js 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn conditional_imports_match_dart_3_6_2_in_all_modes() {
    let fixture = Fixture::new();
    for optimization in [Optimization::None, Optimization::Constants] {
        for merge_identical_functions in [false, true] {
            let options = CompileOptions {
                optimization,
                merge_identical_functions,
            };
            let output = compile_path_with_environment(
                &fixture.entry(),
                options,
                &CompilationEnvironment::javascript(),
            )
            .unwrap();
            let result = std::process::Command::new("node")
                .args(["--input-type=module", "--eval", &output])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                EXPECTED_JS
            );
        }
    }
}
