//! Regressões do cache de planos: fontes descartáveis, contexto novo e saídas atualizadas.
use dartforge_compiler::{
    CompileOptions, CompilerSession, MacroSession, Optimization, compile_with_macro_session,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SIMPLE: &str = "@JsonCodable() class C{final int x;} void main(){print(C(1).x);}";

/// Reserva arquivos privados deste teste sem depender de caminhos existentes.
struct Fixture(PathBuf);
impl Fixture {
    /// Cria um diretório exclusivo, inclusive após processos interrompidos.
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "dartforge-macro22-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("{error}"),
            }
        }
    }
    /// Grava somente dentro do diretório reservado.
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for Fixture {
    /// Remove somente os arquivos temporários deste teste.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Executa JavaScript emitido; os testes chamadores exigem Node explicitamente.
fn run(source: &str) -> String {
    let fixture = Fixture::new();
    let path = fixture.write("main.mjs", source);
    let output = std::process::Command::new("node")
        .arg(path)
        .output()
        .expect("Node necessário");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

/// O plano sobrevive à fonte e não reutiliza IDs nominais ou índices de tipos aplicados.
#[test]
fn disposable_sources_and_shifted_ids_compile_again() {
    let mut cache = MacroSession::default();
    {
        let source = String::from(SIMPLE);
        compile_with_macro_session(&source, CompileOptions::default(), &mut cache).unwrap();
    }
    let shifted = format!("class Before{{}} List<int> values() => [1]; {SIMPLE}");
    compile_with_macro_session(&shifted, CompileOptions::default(), &mut cache).unwrap();
    assert_eq!(cache.stats().misses, 1);
    assert_eq!(cache.stats().hits, 1);
}

/// Todo acerto materializa spans próprios e mantém a anotação da fonte atual.
#[test]
fn cached_plan_materializes_fresh_spans() {
    let mut cache = MacroSession::default();
    let mut prior_end = 0;
    for (index, source) in [
        SIMPLE.to_owned(),
        format!("// comentário desloca a origem\n{SIMPLE}"),
    ]
    .iter()
    .enumerate()
    {
        let tokens = dartforge_lexer::lex(source).unwrap();
        let mut program = dartforge_parser::parse(&tokens, source.len()).unwrap();
        let report = cache.expand(&mut program, source.len()).unwrap();
        assert_eq!(report.plan_hits, index);
        assert!(!report.origins.is_empty());
        let mut unique = std::collections::BTreeSet::new();
        for origin in &report.origins {
            assert!(origin.generated.start > source.len());
            assert!(unique.insert((origin.generated.start, origin.generated.end)));
            assert!(source[origin.annotation.start..origin.annotation.end].contains("JsonCodable"));
        }
        assert!(report.extent > prior_end);
        prior_end = report.extent;
    }
}

/// Alterações de corpo reutilizam plano; acertos JS completos não invocam macros.
#[test]
fn body_edit_hits_plan_but_not_full_output() {
    let fixture = Fixture::new();
    let path = fixture.write("main.dart", SIMPLE);
    let mut cache = CompilerSession::new();
    let first = cache.compile_path(&path, Optimization::None).unwrap();
    assert_eq!(
        (first.stats.macro_plan_hits, first.stats.macro_plan_misses),
        (0, 1)
    );
    fixture.write("main.dart", &SIMPLE.replace("C(1)", "C(2)"));
    let edited = cache.compile_path(&path, Optimization::None).unwrap();
    assert!(!edited.stats.cache_hit);
    assert_ne!(first.javascript, edited.javascript);
    assert_eq!(
        (edited.stats.macro_plan_hits, edited.stats.macro_plan_misses),
        (1, 0)
    );
    let before = cache.macro_cache_stats();
    let repeated = cache.compile_path(&path, Optimization::None).unwrap();
    assert!(repeated.stats.cache_hit);
    assert_eq!(
        (
            repeated.stats.macro_plan_hits,
            repeated.stats.macro_plan_misses
        ),
        (0, 0)
    );
    assert_eq!(before, cache.macro_cache_stats());
}

/// Ordem, nomes, nullability e forma do construtor integram o esquema validado.
#[test]
fn schema_changes_miss_and_warm_invalid_classes_fail() {
    let mut cache = MacroSession::default();
    for fields in [
        "final int x;final int y;",
        "final int y;final int x;",
        "final int z;final int x;",
        "final int? z;final int x;",
    ] {
        let source = format!("@JsonCodable() class C{{{fields}}} void main(){{}}");
        compile_with_macro_session(&source, CompileOptions::default(), &mut cache).unwrap();
    }
    assert_eq!(cache.stats().misses, 4);
    compile_with_macro_session(SIMPLE, CompileOptions::default(), &mut cache).unwrap();
    for body in ["int toJson()=>1;", "C(this.x){print(x);}"] {
        let source = format!("@JsonCodable() class C{{final int x;{body}}} void main(){{}}");
        assert!(
            compile_with_macro_session(&source, CompileOptions::default(), &mut cache).is_err()
        );
    }
    compile_with_macro_session(SIMPLE, CompileOptions::default(), &mut cache).unwrap();
}

/// Falhas quentes nunca entregam saída anterior; orçamento zero não retém planos ou JS.
#[test]
fn warm_failure_and_zero_budget_do_not_serve_stale_output() {
    let fixture = Fixture::new();
    let path = fixture.write("main.dart", SIMPLE);
    let mut cache = CompilerSession::new();
    cache.compile_path(&path, Optimization::None).unwrap();
    fixture.write(
        "main.dart",
        &SIMPLE.replace("final int x;", "final int x;int toJson()=>1;"),
    );
    assert!(cache.compile_path(&path, Optimization::None).is_err());
    fixture.write("main.dart", SIMPLE);
    assert!(
        !cache
            .compile_path(&path, Optimization::None)
            .unwrap()
            .stats
            .cache_hit
    );
    let mut disabled = CompilerSession::with_cache_limit_bytes(0);
    for _ in 0..2 {
        let result = disabled.compile_path(&path, Optimization::None).unwrap();
        assert!(!result.stats.cache_hit);
        assert_eq!(
            (result.stats.macro_plan_hits, result.stats.macro_plan_misses),
            (0, 1)
        );
        assert_eq!(disabled.macro_cache_stats().entries, 0);
    }
}

/// Materialização renovada mantém argumentos, casts e IDs corretos na execução.
#[test]
#[ignore = "requer Node no PATH"]
fn changed_schema_executes_current_meaning() {
    let mut cache = MacroSession::default();
    for (source, expected) in [
        (
            "@JsonCodable() class C{final int x;final int y;} void main(){var c=C(1,2);print(c.x);print(c.y);}",
            "1\n2\n",
        ),
        (
            "class Before{} @JsonCodable() class C{final int y;final int x;} void main(){var c=C(1,2);print(c.x);print(c.y);}",
            "2\n1\n",
        ),
        (
            "@JsonCodable() class C{final int? x;} void main(){print(C.fromJson(<String,Object?>{}).x);}",
            "null\n",
        ),
        (
            "class Before{} List<int> f()=>[1]; @JsonCodable() class C{final int? x;} void main(){print(C.fromJson(<String,Object?>{'x':7}).x);}",
            "7\n",
        ),
    ] {
        let javascript =
            compile_with_macro_session(source, CompileOptions::default(), &mut cache).unwrap();
        assert_eq!(run(&javascript), expected);
    }
    assert!(cache.stats().hits >= 1);
}

/// Mesmo esquema privado em duas bibliotecas compartilha plano, nunca identidade de membro.
#[test]
#[ignore = "requer Node no PATH"]
fn private_imported_fields_keep_library_identity_and_json_key() {
    let fixture = Fixture::new();
    let path = fixture.write(
        "main.dart",
        "import 'a.dart';import 'b.dart';void main(){print(readA());print(readB());}",
    );
    fixture.write("a.dart", "@JsonCodable() class A{final int _secret;} int readA()=>A.fromJson(<String,Object?>{'_secret':11}).toJson()['_secret'] as int;");
    fixture.write("b.dart", "@JsonCodable() class B{final int _secret;} int readB()=>B.fromJson(<String,Object?>{'_secret':22}).toJson()['_secret'] as int;");
    let mut cache = CompilerSession::new();
    let result = cache.compile_path(&path, Optimization::None).unwrap();
    assert_eq!(run(&result.javascript), "11\n22\n");
    assert_eq!(
        (result.stats.macro_plan_hits, result.stats.macro_plan_misses),
        (1, 1)
    );
}
