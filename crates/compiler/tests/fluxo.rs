//! Controle de fluxo: try/on/catch/finally, throw, rethrow, assert, for-in,
//! rótulos e o operador condicional.
//!
//! O oráculo é o **Dart SDK 3.6.2 instalado nesta máquina**, conferido também
//! com o **Dart SDK 3.13.4**. A saída em
//! `tests/conformance/modules/fluxo26/main.stdout` foi produzida por
//! `dart run --enable-asserts main.dart` sobre o mesmo arquivo que o DartForge
//! compila aqui; o teste marcado com `#[ignore]` confere byte a byte que o
//! JavaScript emitido imprime exatamente o mesmo texto no Node.
//!
//! O contrato completo e os limites que permanecem estão em `docs/FLUXO.md`.
use dartforge_compiler::{CompileOptions, Optimization, compile, compile_path_with_options};
use dartforge_diagnostics::Span;

/// Caminho do módulo de conformidade compartilhado por Dart e JavaScript.
fn modulo() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/modules/fluxo26/main.dart")
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
    assert_eq!(source.rfind(needle), Some(start), "trecho ambíguo: {needle}");
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

/// O texto impresso pelo JavaScript emitido é igual ao do Dart 3.6.2.
///
/// A comparação roda em todas as combinações de opções que o compilador expõe,
/// inclusive com a dobra de constantes e o tree shaking ligados: a asserção
/// precisa sobreviver às duas, conforme a política de `docs/FLUXO.md`.
#[test]
#[ignore = "requer Node.js no PATH"]
fn flow_matches_dart_in_all_modes() {
    let expected = include_str!("../../../tests/conformance/modules/fluxo26/main.stdout")
        .replace("\r\n", "\n");
    for optimization in [Optimization::None, Optimization::Constants] {
        for tree_shaking in [false, true] {
            let js = compile_path_with_options(
                &modulo(),
                CompileOptions {
                    optimization,
                    merge_identical_functions: false,
                    tree_shaking,
                },
            )
            .unwrap();
            assert_eq!(node(&js), expected, "{optimization:?} shake={tree_shaking}");
        }
    }
}

/// `finally` roda na saída por `return` sem alterar o valor já calculado.
#[test]
fn finally_runs_after_return_without_changing_the_value() {
    let js = javascript(
        "int f(){var x=1;try{x=2;return x;}finally{x=3;print('f $x');}}\
         void main(){print(f());}",
    );
    assert!(js.contains("try {"), "{js}");
    assert!(js.contains("} finally {"), "{js}");
    // O valor do return é calculado antes do finally; o JavaScript preserva
    // isso sem nenhuma reescrita da nossa parte.
    assert!(js.contains("return $df_x;"), "{js}");
}

/// Sem cláusula `on`, o `catch` recebe qualquer valor lançado.
#[test]
fn catch_without_on_accepts_every_thrown_value() {
    let js = javascript("void main(){try{throw 1;}catch(e){print('x');}}");
    assert!(js.contains("catch ($dartforgeCaught0)"), "{js}");
    assert!(js.contains("const $df_e = $dartforgeCaught0;"), "{js}");
    // Uma cláusula que aceita tudo não pode gerar relançamento.
    assert!(!js.contains("throw $dartforgeCaught0;"), "{js}");
}

/// `on T catch (e)` testa o tipo em execução e propaga o que não casa.
#[test]
fn on_clause_tests_the_runtime_type_and_rethrows_the_rest() {
    let js = javascript("void main(){try{throw 'a';}on String catch(e){print(e);}}");
    assert!(
        js.contains("if ($dartforgeIs($dartforgeCaught0,['string'])) {"),
        "{js}"
    );
    assert!(js.contains("else { throw $dartforgeCaught0; }"), "{js}");
}

/// `on T` sem `catch` não liga variável alguma.
#[test]
fn on_clause_without_catch_binds_nothing() {
    let js = javascript("void main(){try{throw 'a';}on String{print('t');}}");
    assert!(js.contains("if ($dartforgeIs("), "{js}");
    assert!(!js.contains("const $df_"), "{js}");
}

/// `catch (e, s)` liga o valor lançado e um rastro de pilha opaco.
#[test]
fn catch_binds_the_value_and_an_opaque_stack_trace() {
    let js = javascript("void main(){try{throw 'a';}catch(e,s){print('t');}}");
    assert!(js.contains("const $df_e = $dartforgeCaught0;"), "{js}");
    assert!(
        js.contains("const $df_s = $dartforgeStack($dartforgeCaught0);"),
        "{js}"
    );
    assert!(js.contains("function $dartforgeStack(error)"), "{js}");
}

/// `rethrow` relança o valor da cláusula mais interna, e não a de fora.
#[test]
fn rethrow_uses_the_innermost_catch_variable() {
    let js = javascript(
        "void main(){try{try{throw 'a';}catch(e){rethrow;}}catch(e){print('f');}}",
    );
    assert!(js.contains("throw $dartforgeCaught1;"), "{js}");
}

/// `rethrow` fora de uma cláusula `catch` é erro de compilação.
#[test]
fn rethrow_outside_a_catch_clause_is_rejected() {
    let source = "void main(){rethrow;}";
    rejeita(
        source,
        "rethrow requires an enclosing catch clause",
        trecho(source, "rethrow;"),
    );
    let dentro_do_try = "void main(){try{rethrow;}catch(e){print('x');}}";
    rejeita(
        dentro_do_try,
        "rethrow requires an enclosing catch clause",
        trecho(dentro_do_try, "rethrow;"),
    );
    let dentro_do_finally = "void main(){try{print('a');}finally{rethrow;}}";
    rejeita(
        dentro_do_finally,
        "rethrow requires an enclosing catch clause",
        trecho(dentro_do_finally, "rethrow;"),
    );
}

/// Uma closure declarada dentro de `catch` não herda o direito de `rethrow`.
#[test]
fn rethrow_does_not_cross_a_closure_boundary() {
    let source = "void main(){try{throw 'a';}catch(e){void Function() f=(){rethrow;};f();}}";
    rejeita(
        source,
        "rethrow requires an enclosing catch clause",
        trecho(source, "rethrow;"),
    );
}

/// `try` precisa de pelo menos uma cláusula ou de `finally`.
#[test]
fn try_without_any_clause_is_rejected() {
    let source = "void main(){try{print('a');}}";
    rejeita(
        source,
        "try requires at least one on, catch or finally clause",
        trecho(source, "}"),
    );
}

/// O código depois de `throw` é inalcançável e prova o retorno obrigatório.
#[test]
fn throw_proves_the_required_return() {
    assert!(compile("int f(){throw 'x';}void main(){print(f());}").is_ok());
    assert!(
        compile("int f(){try{return 1;}catch(e){throw 'x';}}void main(){print(f());}").is_ok()
    );
    assert!(
        compile("int f(){try{return 1;}catch(e){rethrow;}}void main(){print(f());}").is_ok()
    );
    // Uma cláusula que não sai deixa um caminho sem retorno.
    assert!(compile("int f(){try{return 1;}catch(e){print('x');}}void main(){print(f());}").is_err());
    // O finally que retorna encerra a função por qualquer caminho.
    assert!(compile("int f(){try{print('a');}finally{return 2;}}void main(){print(f());}").is_ok());
}

/// `throw` só aceita um valor que nunca é nulo.
#[test]
fn throw_rejects_a_possibly_null_value() {
    let source = "void main(){String? s='a';throw s;}";
    rejeita(
        source,
        "Cannot throw a value that may be null",
        trecho(source, "s;").into_start(),
    );
}

/// `a ?? (throw e)` recebe o tipo do lado esquerdo sem null.
#[test]
fn if_null_with_a_throw_keeps_the_left_type() {
    let js = javascript("int? n()=>null;void main(){int x=n() ?? (throw 'f');print(x);}");
    assert!(js.contains("$dartforgeThrow("), "{js}");
    assert!(js.contains("function $dartforgeThrow(value) { throw value; }"), "{js}");
}

/// Dart 3.6.2 e 3.13.4 exigem parênteses em `a ?? throw e`; o parser também.
#[test]
fn if_null_requires_parentheses_around_throw() {
    assert!(compile("int? n()=>null;void main(){int x=n() ?? throw 'f';print(x);}").is_err());
}

/// `for-in` avalia o iterável exatamente uma vez e infere o elemento.
#[test]
fn for_in_evaluates_the_iterable_once_and_infers_the_element() {
    let js = javascript(
        "List<int> f(){print('e');return <int>[1,2];}\
         void main(){for(final x in f()){print(x + 1);}}",
    );
    assert!(js.contains("for (const $df_x of "), "{js}");
    assert_eq!(js.matches("$df_f()").count(), 1, "{js}");
    // O tipo do elemento veio do Iterable, então a soma inteira é aceita.
    assert!(js.contains("($df_x + 1)"), "{js}");
}

/// `for (var x in ...)` declara uma variável mutável no corpo.
#[test]
fn for_in_var_declares_a_mutable_element() {
    let js = javascript("void main(){for(var x in <int>[1]){x=2;print(x);}}");
    assert!(js.contains("for (let $df_x of "), "{js}");
}

/// O elemento de `for-in` respeita o tipo anotado e o `final`.
#[test]
fn for_in_checks_the_annotation_and_finality() {
    assert!(compile("void main(){for(int x in <int>[1]){print(x);}}").is_ok());
    assert!(compile("void main(){for(String x in <int>[1]){print(x);}}").is_err());
    let source = "void main(){for(final x in <int>[1]){x=2;}}";
    rejeita(
        source,
        "Cannot assign to final variable 'x'",
        trecho(source, "x=2;"),
    );
}

/// `for-in` exige uma List ou um Iterable.
#[test]
fn for_in_requires_a_list_or_iterable() {
    let source = "void main(){for(final x in 1){print(x);}}";
    rejeita(
        source,
        "for-in requires a List or Iterable",
        trecho(source, "1)").into_start(),
    );
}

/// `for (x in ...)` sobre variável existente é rejeitado explicitamente.
#[test]
fn for_in_over_an_existing_variable_is_rejected() {
    let source = "void main(){var x=0;for(x in <int>[1]){print(x);}}";
    let error = compile(source).expect_err(source);
    assert_eq!(
        error.message,
        "for-in requires final, var or a type; assigning to an existing variable is not supported"
    );
}

/// Rótulos alcançam laços externos em `for`, `while` e `do/while`.
#[test]
fn labels_reach_the_outer_loop_in_every_loop_form() {
    let js = javascript(
        "void main(){a:for(var i=0;i<2;i++){b:while(true){continue a;}}}",
    );
    assert!(js.contains("$df_a: for ("), "{js}");
    assert!(js.contains("$df_b: while ("), "{js}");
    assert!(js.contains("continue $df_a;"), "{js}");
    let repete = javascript("void main(){r:do{break r;}while(false);}");
    assert!(repete.contains("$df_r: do "), "{repete}");
    assert!(repete.contains("break $df_r;"), "{repete}");
}

/// Um rótulo desconhecido é erro com o intervalo da instrução.
#[test]
fn an_unknown_label_is_rejected_with_its_span() {
    let quebra = "void main(){for(var i=0;i<2;i++){break fora;}}";
    rejeita(
        quebra,
        "Unknown loop label 'fora'",
        trecho(quebra, "break fora;"),
    );
    let segue = "void main(){for(var i=0;i<2;i++){continue fora;}}";
    rejeita(
        segue,
        "Unknown loop label 'fora'",
        trecho(segue, "continue fora;"),
    );
    // O rótulo sai de escopo ao fim do laço que ele nomeia.
    let depois = "void main(){a:for(var i=0;i<2;i++){print(i);}break a;}";
    rejeita(depois, "Unknown loop label 'a'", trecho(depois, "break a;"));
}

/// Um rótulo não atravessa a fronteira de uma closure.
#[test]
fn a_label_does_not_cross_a_closure_boundary() {
    let source = "void main(){a:for(var i=0;i<2;i++){void Function() f=(){break a;};f();}}";
    let error = compile(source).expect_err(source);
    assert_eq!(error.message, "break and continue require an enclosing loop");
}

/// Rótulos repetidos no mesmo aninhamento são rejeitados.
#[test]
fn a_duplicate_label_is_rejected() {
    let source = "void main(){a:for(var i=0;i<2;i++){a:while(true){break a;}}}";
    let error = compile(source).expect_err(source);
    assert_eq!(error.message, "Duplicate loop label 'a'");
}

/// Rótulo em algo que não é laço é rejeitado com o intervalo da instrução.
#[test]
fn a_label_on_a_non_loop_is_rejected() {
    let source = "void main(){a:print('x');}";
    let error = compile(source).expect_err(source);
    assert_eq!(error.message, "labels are supported only on loops");
}

/// A precedência do ternário fica abaixo de `??`, `||` e `&&`.
#[test]
fn the_conditional_binds_looser_than_if_null_and_the_boolean_operators() {
    // `(a ?? b) ? ... : ...` é o agrupamento do Dart 3.6.2.
    let js = javascript("void main(){bool? b;print((b ?? true) ? 1 : 2);}");
    assert!(js.contains("(($df_b ?? true) ? 1 : 2)"), "{js}");
    let ou = javascript("void main(){print(true || false ? 'x' : 'y');}");
    assert!(ou.contains("((true || false) ? \"x\" : \"y\")"), "{ou}");
}

/// O ternário é associativo à direita.
#[test]
fn the_conditional_is_right_associative() {
    let js = javascript("void main(){print(true ? 1 : true ? 2 : 3);}");
    assert!(js.contains("(true ? 1 : (true ? 2 : 3))"), "{js}");
}

/// `x is T ? a : b` não confunde o `?` com a marca de nulabilidade.
#[test]
fn a_type_test_before_a_conditional_is_not_a_nullable_marker() {
    let js = javascript("void main(){Object o=1;print(o is int ? 'i' : 'o');}");
    assert!(js.contains(" ? \"i\" : \"o\")"), "{js}");
    // A forma nulável continua sendo lida como tipo quando nada segue.
    assert!(compile("void main(){Object? o;print(o is int?);}").is_ok());
}

/// Os dois ramos do ternário recebem a promoção da condição.
#[test]
fn both_conditional_branches_get_the_null_safety_promotion() {
    assert!(compile("int? n()=>null;void main(){int? v=n();print(v==null?0:v+1);}").is_ok());
    assert!(compile("int? n()=>null;void main(){int? v=n();print(v!=null?v+1:0);}").is_ok());
    // A promoção não escapa para o outro ramo.
    assert!(compile("int? n()=>null;void main(){int? v=n();print(v==null?v+1:0);}").is_err());
    // Nem para a junção depois da expressão.
    assert!(compile("int? n()=>null;void main(){int? v=n();print(v==null?0:1);print(v+1);}").is_err());
}

/// Um ramo `throw` empresta o tipo do outro ramo.
#[test]
fn a_throwing_branch_takes_the_other_branch_type() {
    let js = javascript("void main(){int x=false ? throw 'e' : 1;print(x);}");
    assert!(js.contains("$dartforgeThrow(\"e\")"), "{js}");
}

/// A asserção é sempre emitida, inclusive com dobra de constantes e shaking.
///
/// `CompileOptions` não tem um perfil de produção: `Optimization::Constants`
/// dobra constantes e `tree_shaking` remove declarações inalcançáveis, e
/// nenhum dos dois é o "modo release" do Dart. Enquanto esse perfil não
/// existir, a asserção acompanha o modo de desenvolvimento e é emitida sempre.
#[test]
fn assertions_are_always_emitted() {
    let source = "void main(){var x=1;assert(x==1);assert(x==1,'m');print(x);}";
    for optimization in [Optimization::None, Optimization::Constants] {
        for tree_shaking in [false, true] {
            let js = compile_path_with_options_source(source, optimization, tree_shaking);
            assert_eq!(js.matches("$dartforgeAssertionError(").count(), 2, "{js}");
            assert!(js.contains("function $dartforgeAssertionError("), "{js}");
        }
    }
}

/// Compila uma fonte em memória com as opções pedidas.
fn compile_path_with_options_source(
    source: &str,
    optimization: Optimization,
    tree_shaking: bool,
) -> String {
    dartforge_compiler::compile_with_options(
        source,
        CompileOptions {
            optimization,
            merge_identical_functions: false,
            tree_shaking,
        },
    )
    .unwrap_or_else(|error| panic!("{source}: {}", error.message))
}

/// A mensagem da asserção só é avaliada quando a condição falha.
#[test]
fn the_assert_message_is_evaluated_only_on_failure() {
    let js = javascript("String m(){return 'x';}void main(){assert(1==1,m());}");
    assert!(
        js.contains("if (!((1 === 1))) { throw $dartforgeAssertionError($df_m()); }"),
        "{js}"
    );
}

/// A condição da asserção precisa ser bool e a mensagem precisa ser String.
#[test]
fn assert_checks_the_condition_and_the_message_types() {
    let condicao = "void main(){assert(1);}";
    let error = compile(condicao).expect_err(condicao);
    assert_eq!(error.message, "Expected bool");
    let mensagem = "void main(){assert(true,1);}";
    let error = compile(mensagem).expect_err(mensagem);
    assert_eq!(error.message, "Expected String");
}

/// `late` é rejeitado por não ter a verificação de leitura antes da escrita.
#[test]
fn late_is_rejected_with_an_explicit_diagnostic() {
    let source = "void main(){late int x;x=1;print(x);}";
    rejeita(
        source,
        "late variables are not supported: the read-before-write check is not implemented",
        trecho(source, "late"),
    );
    let final_tardio = "void main(){late final int x;x=1;print(x);}";
    rejeita(
        final_tardio,
        "late variables are not supported: the read-before-write check is not implemented",
        trecho(final_tardio, "late"),
    );
}

/// O bare `catch (e)` liga Object, que este subconjunto não sabe imprimir.
#[test]
fn a_bare_catch_variable_is_object_and_needs_narrowing() {
    let source = "void main(){try{throw 'a';}catch(e){print(e);}}";
    let error = compile(source).expect_err(source);
    assert_eq!(
        error.message,
        "Printing objects or function values requires unsupported toString semantics"
    );
    // `on T` e a promoção por `is` resolvem o caso.
    assert!(compile("void main(){try{throw 'a';}on String catch(e){print(e);}}").is_ok());
    assert!(
        compile("void main(){try{throw 'a';}catch(e){if(e is String){print(e);}}}").is_ok()
    );
}

/// O backend nativo rejeita o fluxo novo com mensagem própria.
#[test]
fn the_native_backend_rejects_the_new_flow() {
    for (source, feature) in [
        ("void main(){try{print('a');}finally{print('b');}}", "try, catch, finally e rethrow"),
        ("void main(){assert(true);}", "assert"),
        ("void main(){for(final x in <int>[1]){print(x);}}", "for-in"),
        ("void main(){a:for(var i=0;i<1;i++){break a;}}", "rótulos de laço"),
        ("void main(){print(true ? 1 : 2);}", "o operador condicional"),
        ("void main(){throw 'a';}", "throw"),
    ] {
        let error = dartforge_compiler::compile_llvm(source).expect_err(source);
        assert_eq!(
            error.message,
            format!("LLVM AOT ainda não suporta {feature}"),
            "{source}"
        );
    }
}

/// Encurta um intervalo ao seu primeiro caractere significativo.
trait IntoStart {
    /// Devolve o intervalo sem o último caractere do trecho procurado.
    fn into_start(self) -> Span;
}

impl IntoStart for Span {
    fn into_start(self) -> Span {
        Span {
            start: self.start,
            end: self.end - 1,
        }
    }
}
