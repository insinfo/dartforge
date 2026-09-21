//! Macro Rust experimental: compara expansão real com equivalente manual validado no SDK 3.6.2.
use dartforge_compiler::{
    CompileOptions, Optimization, compile_with_options, macro_expansion_report,
};

const SOURCE: &str = include_str!("../../../tests/experimental/macros21/main.dart");
const MANUAL: &str = include_str!("../../../tests/experimental/macros21/manual_expanded.dart");
const EXPECTED: &str = include_str!("../../../tests/experimental/macros21/expected.stdout");

/// Exercita os passes independentes sem presumir que o merge é habilitado para todo corpo.
fn options() -> impl Iterator<Item = CompileOptions> {
    [Optimization::None, Optimization::Constants]
        .into_iter()
        .flat_map(|optimization| {
            [false, true]
                .into_iter()
                .map(move |merge_identical_functions| CompileOptions {
                    optimization,
                    merge_identical_functions,
                    tree_shaking: false,
                })
        })
}

/// Construtores sintetizados/existentes e classe vazia devem analisar após expansão Rust.
#[test]
fn json_codable_expands_and_compiles_in_all_modes() {
    for options in options() {
        compile_with_options(SOURCE, options).unwrap();
        compile_with_options(MANUAL, options).unwrap();
    }
}

/// Proveniência é determinística, exclusiva e externa ao intervalo dos bytes originais.
#[test]
fn expansion_report_has_unique_deterministic_generated_spans() {
    let first = macro_expansion_report(SOURCE).unwrap();
    let second = macro_expansion_report(SOURCE).unwrap();
    assert_eq!(first.applications, 3);
    assert_eq!(first.generated_declarations, 8);
    assert_eq!(first.extent, second.extent);
    let spans = |report: &dartforge_macros::ExpansionReport| {
        report
            .origins
            .iter()
            .map(|origin| {
                (
                    origin.generated.start,
                    origin.generated.end,
                    origin.annotation.start,
                    origin.annotation.end,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(spans(&first), spans(&second));
    let mut seen = std::collections::BTreeSet::new();
    for origin in &first.origins {
        assert!(origin.generated.start > SOURCE.len());
        assert!(origin.generated.end <= first.extent);
        assert!(seen.insert((origin.generated.start, origin.generated.end)));
        assert!(origin.annotation.end <= SOURCE.len());
        assert!(SOURCE[origin.annotation.start..origin.annotation.end].contains("JsonCodable"));
    }
}

/// Uma expansão posterior inválida não deixa a primeira classe parcialmente expandida.
#[test]
fn expansion_failure_is_atomic_and_collisions_are_localized() {
    let source = "@JsonCodable() class Good{final int x;} @JsonCodable() class Bad{int toJson()=>1;} void main(){}";
    let tokens = dartforge_lexer::lex(source).unwrap();
    let mut program = dartforge_parser::parse(&tokens, source.len()).unwrap();
    let before = format!("{program:?}");
    let error = dartforge_macros::expand(&mut program, source.len()).unwrap_err();
    assert_eq!(format!("{program:?}"), before);
    assert!(error.message.contains("conflict"));
    assert!(error.span.start >= source.find("@JsonCodable() class Bad").unwrap());
    for source in [
        "@JsonCodable() @JsonCodable() class C{} void main(){}",
        "@JsonCodable() class C{final List<int> values;} void main(){}",
        "@JsonCodable() class C{int x=1;} void main(){}",
        "@JsonCodable() class C{final int x;C(this.x){print(x);}} void main(){}",
        "@JsonCodable() class C{factory C.fromJson(Map<String,Object?> json)=>C();} void main(){}",
    ] {
        assert!(
            compile_with_options(source, CompileOptions::default()).is_err(),
            "{source}"
        );
    }
}

/// Executa módulos ESM, mantendo stderr para diagnosticar diferenças observáveis.
fn run(javascript: &str) -> std::process::Output {
    std::process::Command::new("node")
        .args(["--input-type=module", "--eval", javascript])
        .output()
        .unwrap()
}

/// As quatro combinações de passes reproduzem o stdout do equivalente manual SDK.
#[test]
#[ignore = "requer Node.js no PATH"]
fn json_codable_matches_manual_dart_3_6_2_in_all_modes() {
    for options in options() {
        for source in [SOURCE, MANUAL] {
            let output = run(&compile_with_options(source, options).unwrap());
            assert!(
                output.status.success(),
                "{options:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                EXPECTED.replace("\r\n", "\n")
            );
        }
    }
}

/// Campos não nullable ausentes ou com tipos incorretos falham nos casts gerados.
#[test]
#[ignore = "requer Node.js no PATH"]
fn generated_from_json_rejects_missing_or_incorrect_fields() {
    for options in options() {
        for map in [
            "<String,Object?>{}",
            "<String,Object?>{'value':'wrong'}",
            "<String,Object?>{'value':true}",
        ] {
            let source = format!(
                "@JsonCodable() class C{{final int value;}} void main(){{print('before');C.fromJson({map});print('after');}}"
            );
            let output = run(&compile_with_options(&source, options).unwrap());
            assert!(!output.status.success(), "{options:?}: {map}");
            assert_eq!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                "before\n"
            );
            assert!(String::from_utf8_lossy(&output.stderr).contains("TypeError"));
        }
    }
}
