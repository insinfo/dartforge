//! Doubles e `num` com semântica do alvo WEB (JS Number), backend JavaScript primeiro.
//!
//! O oráculo destes testes é o **Dart SDK 3.6.2 instalado nesta máquina**,
//! conferido também com o **Dart SDK 3.13.4**: `int x = 1.0` é erro,
//! `double x = 1` vale `1.0`, `1/2` vale `0.5` (sempre double), `~/` sempre
//! retorna int truncado em direção a zero, e o `toString` de double imprime
//! `1.0` (não `1`). O teste marcado com `#[ignore]` confere byte a byte que
//! o JavaScript emitido imprime exatamente o mesmo texto que `dart run`
//! produz para a bateria abaixo, nos dois modos de otimização.
use dartforge_compiler::{Optimization, compile, compile_llvm};
use dartforge_diagnostics::Span;

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

/// Calcula o intervalo da ÚLTIMA ocorrência de um trecho (inicializadores
/// que repetem o nome declarado, como `double b = a;` após `num a`).
fn ultimo_trecho(source: &str, needle: &str) -> Span {
    let start = source.rfind(needle).expect(needle);
    Span {
        start,
        end: start + needle.len(),
    }
}

/// Executa o módulo emitido no Node e devolve a saída padrão normalizada.
fn node(js: &str) -> String {
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", js])
        .output()
        .expect("node no PATH");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("saída UTF-8")
        .replace("\r\n", "\n")
}

/// Bateria de execução: cada `print` foi conferido com `dart run` (3.6.2 e
/// 3.13.4); `7.5 % 0` vale NaN e `1.0 / 3.0` exercita a mantissa longa.
const BATERIA: &str = "void main() { print(0.0); print(-0.0); print(1.0); print(1.5); print(1e21); print(0.1 + 0.2); print(0.0 / 0.0); print(1.0 / 0.0); print(-1.0 / 0.0); print(1 + 1.5); print(1.5 * 2); print(1 / 2); print(7 ~/ 2); print(7.5 ~/ 2); print(-7 ~/ 2); print(7.5 % 2); print(2 < 2.5); double x = 1; print(x); num n = 1; print(n); n = 1.5; print(n); print('v=$x|${1.0}|${1 + 0.5}'); const c = 1.0 + 2.5; print(c); print(7.5 % 0); print(1.0 / 3.0); }";

/// Saída exata de `dart run` sobre a bateria acima.
const ESPERADO: &str = "0.0\n-0.0\n1.0\n1.5\n1e+21\n0.30000000000000004\nNaN\nInfinity\n-Infinity\n2.5\n3.0\n0.5\n3\n3\n-3\n1.5\ntrue\n1.0\n1\n1.5\nv=1.0|1.0|1.5\n3.5\nNaN\n0.3333333333333333\n";

/// O texto impresso pelo JavaScript emitido é igual ao do oráculo Dart.
#[test]
#[ignore = "requer Node.js no PATH"]
fn doubles_match_dart_in_all_modes() {
    for optimization in [Optimization::None, Optimization::Constants] {
        let js = dartforge_compiler::compile_with_optimization(BATERIA, optimization)
            .unwrap_or_else(|error| panic!("{}", error.message));
        assert_eq!(node(&js), ESPERADO, "{optimization:?}");
    }
}

/// `int x = 1.0` é erro com mensagem e intervalo exatos.
#[test]
fn int_rejects_double_literal() {
    let source = "void main(){int x = 1.0;}";
    rejeita(
        source,
        "Type mismatch: expected Int, found Double",
        trecho(source, "1.0"),
    );
}

/// `/` sempre produz double, mesmo entre inteiros: `int h = 1/2` é erro.
#[test]
fn int_rejects_division_result() {
    let source = "void main(){int h = 1/2;}";
    rejeita(
        source,
        "Type mismatch: expected Int, found Double",
        trecho(source, "1/2"),
    );
}

/// `num` não cabe em `double`: a conversão exige verificação em tempo de execução.
#[test]
fn double_rejects_num_operand() {
    let source = "void main(){num a = 1; double b = a;}";
    rejeita(
        source,
        "Type mismatch: expected Double, found Num",
        ultimo_trecho(source, "a"),
    );
}

/// Comparações mistas rejeitam o lado não numérico no intervalo do operando.
#[test]
fn comparisons_reject_non_numeric_side() {
    let source = "void main(){bool b = 1.5 < 'a';}";
    rejeita(
        source,
        "Type mismatch: expected Int, found String",
        trecho(source, "'a'"),
    );
}

/// Negação de não numérico mantém o diagnóstico escalar no operando.
#[test]
fn negation_rejects_bool_operand() {
    let source = "void main(){var x = -true;}";
    rejeita(
        source,
        "Type mismatch: expected Int, found Bool",
        trecho(source, "true"),
    );
}

/// Oráculo Dart 3.6.2/3.13.4: `1.` é literal inválido, rejeitado no léxico.
#[test]
fn trailing_dot_is_rejected_at_lexing() {
    let source = "void main(){var x = 1.;}";
    rejeita(
        source,
        "invalid double literal: '.' must be followed by a digit",
        trecho(source, "1."),
    );
}

/// `double x = 1` promove int→double; `num` aceita int e double.
#[test]
fn promotion_and_num_annotations_compile() {
    javascript("void main(){double x = 1; print(x);}");
    javascript("void main(){int i = 5; double d = i; print(d);}");
    javascript("double f(double x){return x;} void main(){print(f(1));}");
    javascript("double g(){return 1;} void main(){print(g());}");
    javascript("void main(){num a = 1; num b = 1.5; print(a); print(b);}");
    javascript("void main(){double d = 1 + 1.5; print(d);}");
    javascript("void main(){num n = 1 + 1.5; print(n);}");
    javascript("void main(){bool b = 2 < 2.5; print(b);}");
    javascript(
        "void main(){double d = -1.5; int i = -1; num n = -i; print(d); print(i); print(n);}",
    );
    javascript("void main(){double? x = 1.5; double? z = null; print(x); print(z);}");
    javascript("void main(){List<double> ds = [1, 2.5]; print(ds);}");
    // `is int` distingue via `Number.isInteger`; `is double`/`is num` usam
    // `typeof number` com o limite documentado para doubles de valor inteiro
    // (apenas compilação aqui; a bateria acima confere os textos impressos).
    javascript("void main(){num n = 2; print(n is int); print(n is double);}");
    javascript("void main(){num m = 2.5; print(m is int); print(m is double); print(m is num);}");
}

/// `~/` sempre retorna int (mesmo entre doubles) e `/` emite divisão direta.
#[test]
fn truncating_division_returns_int() {
    let js = javascript("void main(){int q = 7.5 ~/ 2; print(q);}");
    assert!(js.contains("$dartforgeTruncDiv("), "{js}");
    // int promovido a double continua válido no destino double.
    javascript("void main(){double d = 7 ~/ 2; print(d);}");
    let js = javascript("void main(){double h = 1/2; print(h);}");
    assert!(js.contains(" / "), "{js}");
}

/// `print` e interpolação de double usam o formatador com toString Dart.
#[test]
fn double_print_and_interpolation_use_formatter() {
    let js = javascript("void main(){print(1.0);}");
    assert!(js.contains("$dartforgeDouble("), "{js}");
    assert!(js.contains("function $dartforgeDouble(value)"), "{js}");
    let js = javascript("void main(){double x = 1; print('$x|${x}');}");
    assert_eq!(js.matches("$dartforgeDouble(").count(), 3, "{js}");
}

/// Doubles em contexto const avaliam `+ - * / % ~/` e comparações.
#[test]
fn const_doubles_evaluate() {
    javascript("void main(){const a = 1.5; print(a);}");
    javascript("void main(){const b = 1.0 + 2.5; print(b);}");
    javascript("void main(){const c = 2 * 1.5; print(c);}");
    javascript("void main(){const d = 1.5 < 2.5; print(d);}");
    javascript("void main(){const e = 7.5 ~/ 2; print(e);}");
    javascript("void main(){const f = 1/2; print(f);}");
    javascript("void main(){const g = -1.5; print(g);}");
    // `~/0` em const é erro com o intervalo da expressão.
    let source = "void main(){const e = 7.5 ~/ 0; print(e);}";
    let error = compile(source).expect_err(source);
    assert_eq!(
        error.message, "Const expression: integer division by zero",
        "{source}"
    );
    assert_eq!(error.span, trecho(source, "7.5 ~/ 0"), "{source}");
}

/// `is double`/`is num` usam checagem `typeof`; casts exigem operando estático.
#[test]
fn numeric_type_tests_and_casts() {
    javascript("void main(){bool a = 1.5 is double; print(a);}");
    javascript("void main(){bool a = 1 is num; print(a);}");
    javascript("void main(){double d = 1.5 as double; print(d);}");
    javascript("void main(){num n = 1 as num; print(n);}");
    let source = "void main(){double d = 1 as double; print(d);}";
    rejeita(
        source,
        "Casts to double or num require a statically known numeric operand",
        trecho(source, "1 as double"),
    );
}

/// O backend LLVM AOT rejeita doubles com diagnóstico explícito.
#[test]
fn llvm_rejects_doubles_explicitly() {
    let error = compile_llvm("void main(){double x = 1.0; print(x);}")
        .expect_err("LLVM deveria rejeitar double");
    assert_eq!(
        error.message, "LLVM AOT ainda não suporta double e num (use o backend JavaScript)",
        "{error:?}",
    );
    let error =
        compile_llvm("void main(){print(1.0);}").expect_err("LLVM deveria rejeitar literal double");
    assert_eq!(
        error.message, "LLVM AOT ainda não suporta literais double",
        "{error:?}",
    );
    let error = compile_llvm("void main(){int q = 7 ~/ 2; print(q);}")
        .expect_err("LLVM deveria rejeitar ~/");
    assert_eq!(
        error.message, "LLVM AOT ainda não suporta divisão double e truncada (`/`, `~/`)",
        "{error:?}",
    );
}

/// Literais com expoente e fração passam pelo parser com o valor exato.
#[test]
fn double_literals_cover_fraction_and_exponent() {
    let tokens = dartforge_lexer::lex("1.0 1e3 1.5e-3 .5").unwrap();
    let textos: Vec<_> = tokens
        .iter()
        .map(|token| format!("{:?}", token.kind))
        .collect();
    assert_eq!(textos.len(), 4, "{textos:?}");
    javascript(
        "void main(){double a = 1e3; double b = 1.5e-3; double c = .5; print(a); print(b); print(c);}",
    );
}

/// Emissão de literais especiais preserva o valor IEEE-754 no JavaScript.
#[test]
fn special_values_emit_js_identifiers() {
    let js = javascript("void main(){print(0.0/0.0); print(1.0/0.0);}");
    assert!(js.contains("console.log($dartforgeDouble("), "{js}");
}
