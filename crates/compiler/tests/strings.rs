//! Literais de string completos: interpolação, aspas triplas e adjacência.
//!
//! O oráculo destes testes é o **Dart SDK 3.6.2 instalado nesta máquina**. A
//! saída de `tests/conformance/modules/strings25/main.stdout` foi produzida por
//! `dart run main.dart` sobre o mesmo arquivo que o DartForge compila aqui, e o
//! teste marcado com `#[ignore]` confere byte a byte que o JavaScript emitido
//! imprime exatamente o mesmo texto no Node.
use dartforge_compiler::{CompileOptions, Optimization, compile, compile_path_with_options};
use dartforge_diagnostics::Span;

/// Caminho do módulo de conformidade compartilhado por Dart e JavaScript.
fn modulo() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/modules/strings25/main.dart")
}

/// Compila uma fonte válida e devolve o módulo JavaScript emitido.
fn javascript(source: &str) -> String {
    compile(source).unwrap_or_else(|error| panic!("{source}: {}", error.message))
}

/// Confere mensagem e intervalo exatos de um programa rejeitado.
fn rejeita(source: &str, message: &str, span: Span) {
    let error = compile(source).expect_err(source);
    assert_eq!(error.message, message, "{source}");
    assert_eq!(error.span, span, "{source}");
}

/// Calcula o intervalo de um trecho único da fonte, em bytes.
fn trecho(source: &str, needle: &str) -> Span {
    let start = source.find(needle).expect(needle);
    assert_eq!(
        source.rfind(needle),
        Some(start),
        "trecho ambíguo: {needle}"
    );
    Span {
        start,
        end: start + needle.len(),
    }
}

/// Calcula o intervalo do identificador de uma interpolação `$nome`.
fn nome_interpolado(source: &str, needle: &str) -> Span {
    let span = trecho(source, needle);
    Span {
        start: span.start + 1,
        end: span.end,
    }
}

/// O texto impresso pelo JavaScript emitido é igual ao do Dart 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn strings_match_dart_in_all_modes() {
    let expected = include_str!("../../../tests/conformance/modules/strings25/main.stdout")
        .replace("\r\n", "\n");
    for optimization in [Optimization::None, Optimization::Constants] {
        for merge_identical_functions in [false, true] {
            let js = compile_path_with_options(
                &modulo(),
                CompileOptions {
                    optimization,
                    merge_identical_functions,
                    tree_shaking: false,
                },
            )
            .unwrap();
            let output = std::process::Command::new("node")
                .args(["--input-type=module", "--eval", &js])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                expected
            );
        }
    }
}

/// Interpolação simples e com expressão produzem uma única concatenação.
#[test]
fn interpolation_emits_one_ordered_concatenation() {
    let js = javascript("void main(){var n=3;var s='x';print('a $n b ${n + 1} $s');}");
    assert!(
        js.contains(r#"("a " + $dartforgeString($df_n) + " b " + $dartforgeString(($df_n + 1)) + " " + ($df_s))"#),
        "{js}"
    );
    // A String já é o próprio texto: só os demais tipos pagam a conversão.
    assert_eq!(js.matches("$dartforgeString(").count(), 3);
}

/// `$nome.campo` interpola só o nome; `${obj.campo}` interpola a expressão.
#[test]
fn simple_name_stops_before_member_access() {
    let js = javascript(
        "class P{String nome='a';int idade=1;}\
         void main(){var nome='x';var p=P();print('$nome.idade|${p.nome}|${p.idade}');}",
    );
    assert!(
        js.contains(r#"(($df_nome) + ".idade|" + ($df_p.$df_nome) + "|" + $dartforgeString($df_p.$df_idade))"#),
        "{js}"
    );
}

/// Cada expressão interpolada é avaliada uma única vez, na ordem escrita.
#[test]
fn interpolated_expressions_run_once_in_order() {
    let js = javascript(
        "class C{int v=0;int p(){v=v+1;return v;}}\
         void main(){var c=C();print('${c.p()}-${c.p()}');}",
    );
    let corpo = js
        .split("export function main()")
        .nth(1)
        .expect("corpo de main");
    assert_eq!(corpo.matches("$df_c.$df_p()").count(), 2, "{js}");
    assert!(
        corpo.contains(
            r#"($dartforgeString($df_c.$df_p()) + "-" + $dartforgeString($df_c.$df_p()))"#
        ),
        "{js}"
    );
}

/// O escape `\$` e as strings raw mantêm o cifrão literal.
#[test]
fn escaped_dollar_and_raw_strings_never_interpolate() {
    let js = javascript(r"void main(){var n=1;print('\$n'); print(r'$n'); print('''a\$n''');}");
    assert_eq!(js.matches(r#""$n""#).count(), 2, "{js}");
    assert!(js.contains(r#""a$n""#), "{js}");
    assert!(!js.contains("$dartforgeString"), "{js}");
}

/// Aspas triplas descartam a primeira linha em branco e aceitam interpolação.
#[test]
fn triple_quoted_strings_drop_the_first_line() {
    let js = javascript("void main(){var n=1;print('''\n  a\nb''');print('''x\ny''');}");
    assert!(js.contains(r#""  a\nb""#), "{js}");
    assert!(js.contains(r#""x\ny""#), "{js}");
    // A linha inicial some apenas quando não tem nada além de brancos.
    let js = javascript("void main(){print('''   \nz''');print('''  c\nz''');}");
    assert!(js.contains(r#""z""#), "{js}");
    assert!(js.contains(r#""  c\nz""#), "{js}");
    // Triplas raw preservam a barra invertida e o cifrão.
    let js = javascript("void main(){print(r'''\n\\n$x''');}");
    assert!(js.contains(r#""\\n$x""#), "{js}");
}

/// Literais adjacentes concatenam no parser, com ou sem interpolação.
#[test]
fn adjacent_literals_concatenate_at_compile_time() {
    let js = javascript("void main(){print('a' 'b' r'$c' '''d''');}");
    assert!(js.contains(r#"console.log("ab$cd")"#), "{js}");
    let js = javascript("void main(){var n=1;print('a' '$n' 'b');}");
    assert!(
        js.contains(r#"("a" + $dartforgeString($df_n) + "b")"#),
        "{js}"
    );
}

/// A interpolação de null imprime "null", como em Dart.
#[test]
fn null_is_interpolated_as_the_word_null() {
    let js = javascript("void main(){int? z=null;print('[$z]');}");
    assert!(
        js.contains(r#"("[" + $dartforgeString($df_z) + "]")"#),
        "{js}"
    );
    assert!(js.contains("if (value === null || value === undefined) return 'null';"));
}

/// Interpolações aninhadas param no limite do parser, sem estourar a pilha.
#[test]
fn deeply_nested_interpolation_is_bounded() {
    let mut source = String::from("void main(){print(");
    for _ in 0..500 {
        source.push_str("'${");
    }
    source.push('1');
    for _ in 0..500 {
        source.push_str("}'");
    }
    source.push_str(");}");
    let error = compile(&source).expect_err("aninhamento além do limite");
    assert!(error.span.end <= source.len(), "{}", error.message);
}

/// Cada limite do subconjunto é rejeitado com mensagem e intervalo exatos.
#[test]
fn rejected_string_forms_report_message_and_span() {
    let source = "void main(){print('a$ b');}";
    rejeita(
        source,
        "a '$' inside a string must be followed by an identifier or '{'",
        trecho(source, "$"),
    );

    let source = "void main(){print('$this');}";
    rejeita(
        source,
        "'this' can't be used as an identifier because it's a keyword",
        trecho(source, "this"),
    );

    let source = "void main(){var x=1;print('${x 2}');}";
    rejeita(
        source,
        "string interpolation accepts a single expression",
        trecho(source, "2"),
    );

    let source = "void main(){var s='a\nb';}";
    rejeita(
        source,
        "single-quoted strings must end on the same line",
        trecho(source, "\n"),
    );

    let source = "void main(){var s='${1";
    rejeita(
        source,
        "unterminated string interpolation",
        Span {
            start: source.find('\'').unwrap(),
            end: source.len(),
        },
    );

    let source = "void main(){var s='''a";
    rejeita(
        source,
        "unterminated string",
        Span {
            start: source.find('\'').unwrap(),
            end: source.len(),
        },
    );

    // Instâncias de classe e enums ainda não têm toString representável.
    let source = "class C{int v=0;}void main(){var c=C();print('$c');}";
    rejeita(
        source,
        "String interpolation requires unsupported toString semantics for this value",
        nome_interpolado(source, "$c"),
    );

    let source = "enum E{a,b}void main(){var e=E.a;print('$e');}";
    rejeita(
        source,
        "String interpolation requires unsupported toString semantics for this value",
        nome_interpolado(source, "$e"),
    );

    // Interpolação ainda não é expressão constante.
    let source = "void main(){const s='a${1}b';print(s);}";
    rejeita(
        source,
        "Const expression: calls, getters and this expression are unsupported",
        trecho(source, "'a${1}b'"),
    );
}
