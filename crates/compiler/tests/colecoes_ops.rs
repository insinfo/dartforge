//! Coleções e operadores inteiros: contrato, limites e rejeições explícitas.
//!
//! A saída esperada dos programas executáveis foi copiada de `dart run` sem
//! edição, idêntica nos SDKs Dart 3.6.2 (`dart` no PATH) e 3.13.4
//! (`D:/DartSDKs/3.13.4/dart-sdk/bin/dart.exe`). A fixture integral está em
//! `tests/conformance/modules/colecoesops/main.dart` (limpa no
//! `dart analyze` dos dois SDKs).
//!
//! Estado de implementação: apenas o módulo `%` está completo no backend
//! JavaScript (parser, semântica, const-eval com `rem_euclid`, emissão via
//! `$dartforgeModulo` e rejeição no LLVM). Os demais itens — literais Set,
//! spreads `...`/`...?`, elementos `if`/`for`, acesso `?.`, `~/`, bitwise,
//! `Enum.values` e Map const — estão bloqueados por trabalho concorrente
//! inacabado na árvore (lexer/sintaxe com variantes `Double`, `Divide` e
//! `TruncDivide` sem os braços semânticos; parser com métodos
//! `typedef_decl`/`extension_type` ainda ausentes) e por nós AST ainda sem
//! representação no subconjunto. Os testes desses itens ficam `#[ignore]`
//! como especificação executável até o desbloqueio; os testes ativos travam
//! somente comportamentos já estáveis na árvore.
use dartforge_compiler::{compile, compile_llvm};

/// Saída exata de `dart run` para a fixture integral
/// (`tests/conformance/modules/colecoesops/main.dart`).
const ESPERADO_FIXTURE: &str = "{1, 2}\n\
2\n\
[0, 1, 2, 3]\n\
{y: 2, x: 1}\n\
{0, 1, 2}\n\
origem\n\
origem\n\
[7, 7]\n\
[1]\n\
gerador\n\
[10, 20]\n\
null\n\
null\n\
2\n\
null\n\
3\n\
[0]\n\
false\n\
3\n\
null\n\
4\n\
null\n\
[0, 9]\n\
efeito\n\
1\n\
3\n\
-3\n\
-3\n\
2\n\
1\n\
7\n\
1\n\
6\n\
-6\n\
8\n\
2\n\
-2\n\
[Cor.vermelho, Cor.verde, Cor.azul]\n\
3\n\
Cor.vermelho\n\
true\n\
{a: 1, b: 2}\n\
2\n\
1\n";

/// Saída exata de `dart run` para o programa de módulo euclidiano abaixo.
const ESPERADO_MODULO: &str = "1\n2\n1\n0\n";

/// Confere mensagem e span de um programa rejeitado antes da emissão.
///
/// `ancora` é um trecho único da fonte e `trecho` é o texto que o span precisa
/// cobrir, procurado a partir do início da âncora.
fn rejeita_apos(source: &str, message: &str, ancora: &str, trecho: &str) {
    let error = match compile(source) {
        Ok(_) => panic!("deveria falhar: {source}"),
        Err(error) => error,
    };
    assert_eq!(error.message, message, "fonte: {source}");
    let base = source
        .find(ancora)
        .unwrap_or_else(|| panic!("âncora ausente na fonte: {ancora}"));
    let start = base
        + source[base..]
            .find(trecho)
            .unwrap_or_else(|| panic!("trecho ausente na fonte: {trecho}"));
    assert_eq!(
        (error.span.start, error.span.end),
        (start, start + trecho.len()),
        "span apontou {:?} em {source}",
        source.get(error.span.start..error.span.end)
    );
}

/// Caso comum em que a âncora já é o próprio trecho esperado.
fn rejeita(source: &str, message: &str, trecho: &str) {
    rejeita_apos(source, message, trecho, trecho);
}

/// Executa o módulo emitido em Node e devolve a saída normalizada em LF.
fn executar(js: &str) -> String {
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", js])
        .output()
        .expect("executar node");
    assert!(
        output.status.success(),
        "{}\n{js}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

// ---------------------------------------------------------------------------
// Módulo `%`: implementado de ponta a ponta no backend JavaScript.
// ---------------------------------------------------------------------------

/// O resto usa o auxiliar euclidiano do runtime, não o `%` do JavaScript.
#[test]
fn modulo_emite_auxiliar_euclidiano() {
    let js = compile("void main() { print(7 % 3); print(-7 % 3); }").unwrap();
    assert!(js.contains("$dartforgeModulo(7,3)"), "{js}");
    assert!(js.contains("$dartforgeModulo(-7,3)"), "{js}");
    assert!(
        js.contains("const d = Math.abs(b), r = a % d; return r < 0 ? r + d : r + 0;"),
        "{js}"
    );
}

/// Operandos precisam ser inteiros, como nos demais operadores aritméticos.
#[test]
fn modulo_exige_operandos_int() {
    rejeita(
        "void main() { print('a' % 3); }",
        "Type mismatch: expected Int, found String",
        "'a'",
    );
}

/// O const-eval compartilha o módulo euclidiano (`i64::rem_euclid`).
#[test]
fn modulo_const_e_aceito() {
    compile("void main() { const r = -5 % 3; print(r); }").unwrap();
    compile("void main() { const r = 7 % -3; print(r); }").unwrap();
}

/// O backend nativo rejeita o módulo euclidiano com diagnóstico próprio.
#[test]
fn modulo_no_llvm_e_rejeitado() {
    let error = compile_llvm("void main() { print(7 % 3); }").unwrap_err();
    assert_eq!(
        error.message,
        "LLVM AOT ainda não suporta módulo euclidiano"
    );
}

/// Semantics do oráculo Dart 3.6.2/3.13.4, incluindo divisor zero.
#[test]
#[ignore = "requer Node.js no PATH"]
fn modulo_reproduz_o_oraculo() {
    let js = compile(
        "void main() { print(7 % 3); print(-7 % 3); print(7 % -3); print(8 % 4); }",
    )
    .unwrap();
    assert_eq!(executar(&js), ESPERADO_MODULO);
}

/// Divisor zero lança `RangeError('Integer division by zero')`, como o
/// auxiliar `$dartforgeModulo` já faz; o oráculo Dart lança
/// `IntegerDivisionByZeroException`, sem equivalente no subconjunto.
#[test]
#[ignore = "requer Node.js no PATH"]
fn modulo_por_zero_lanca_range_error() {
    let js = compile("void main() { print(1 % 0); }").unwrap();
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &js])
        .output()
        .expect("executar node");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Integer division by zero"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ---------------------------------------------------------------------------
// Diagnósticos atuais travados (comportamento estável da árvore).
//
// Quando cada item for implementado, o teste correspondente deve ser
// atualizado para o novo diagnóstico próprio.
// ---------------------------------------------------------------------------

/// `{1, 2}` ainda cai na exigência de `:` do literal de mapa.
#[test]
fn conjunto_sem_tipo_cai_na_exigencia_de_dois_pontos() {
    rejeita(
        "void main() { var s = {1, 2}; print(s); }",
        "expected Symbol(':'); supported subset only",
        ",",
    );
}

/// `<String>{}` e `<int>{}` já dizem que sets não são suportados.
#[test]
fn conjunto_tipado_diz_que_sets_nao_sao_suportados() {
    rejeita(
        "void main() { var s = <String>{}; print(s); }",
        "map literals require two type arguments; sets are not supported",
        "}",
    );
    rejeita(
        "void main() { var s = <int>{}; print(s); }",
        "map literals require two type arguments; sets are not supported",
        "}",
    );
}

/// `{}` continua sendo mapa vazio, que exige tipo de valor explícito.
#[test]
fn mapa_vazio_exige_tipo_de_valor() {
    rejeita(
        "void main() { var m = {}; print(m); }",
        "Empty Map requires an explicit or contextual value type",
        "{}",
    );
}

/// Spreads ainda não iniciam um elemento de coleção válido.
#[test]
fn spread_ainda_nao_inicia_elemento() {
    rejeita(
        "void main() { var a = [1]; var b = [0, ...a]; print(b); }",
        "expected a supported expression",
        "..",
    );
}

/// Elementos `if`/`for` ainda não iniciam um elemento de coleção válido.
#[test]
fn elementos_if_for_ainda_nao_iniciam_elemento() {
    rejeita(
        "void main() { var b = [if (true) 1 else 2]; print(b); }",
        "expected a supported expression",
        "if",
    );
    rejeita(
        "void main() { var b = [for (var x in [1]) x]; print(b); }",
        "expected a supported expression",
        "for",
    );
}

/// `Cor.values` ainda não resolve o getter estático da lista canônica.
#[test]
fn enum_values_ainda_nao_resolve() {
    rejeita_apos(
        "enum Cor { vermelho, verde } void main() { print(Cor.values); }",
        "Unknown enum value or static field",
        "print(Cor.values)",
        "Cor.values",
    );
}

/// Map const ainda cai na recusa genérica de expressões não escalares.
#[test]
fn mapa_const_ainda_cai_na_recusa_generica() {
    rejeita_apos(
        "void main() { const m = {'a': 1}; print(m); }",
        "Const expression: calls, getters and this expression are unsupported",
        "const m = {'a': 1}",
        "{'a': 1}",
    );
}

// ---------------------------------------------------------------------------
// Especificação executável dos itens bloqueados (#[ignore] até o desbloqueio).
//
// Mensagens e spans abaixo são o contrato proposto; confirmar ao implementar.
// Diagnósticos novos seguem o estilo da árvore: inglês no frontend, português
// no backend LLVM, sempre com o span do trecho culpado.
// ---------------------------------------------------------------------------

/// Item 1: `{x}` é Set; `{}` é Map; `<T>{}` vazio com um argumento é Set.
#[test]
#[ignore = "requer literais Set (nó AST e tipo ainda ausentes no subconjunto)"]
fn conjuntos_distinguem_mapa_vazio_de_set() {
    rejeita_apos(
        "void main() { var s = {1, 2}; print(s); }",
        "Set literals are not supported yet",
        "{1, 2}",
        "1",
    );
    rejeita(
        "void main() { var s = <int>{1, 2}; print(s); }",
        "map literals require two type arguments; sets are not supported",
        "}",
    );
}

/// Item 2: spreads avaliam uma única vez e preservam a ordem.
#[test]
#[ignore = "requer elementos spread (nó AST ainda ausente no subconjunto)"]
fn spreads_sao_rejeitados_com_mensagem_propria() {
    rejeita_apos(
        "void main() { var a = [1]; var b = [0, ...a]; print(b); }",
        "Spread elements are not supported yet",
        "...a",
        "..",
    );
    rejeita_apos(
        "void main() { List<int>? n = null; var b = [0, ...?n]; print(b); }",
        "Spread elements are not supported yet",
        "...?n",
        "..",
    );
}

/// Item 3: elementos `if`/`for`, com o iterável do `for` avaliado uma vez.
#[test]
#[ignore = "requer elementos if/for (nós AST ainda ausentes no subconjunto)"]
fn elementos_if_for_tem_mensagem_propria() {
    rejeita(
        "void main() { var b = [if (true) 1 else 2]; print(b); }",
        "Collection if-elements are not supported yet",
        "if",
    );
    rejeita(
        "void main() { var b = [for (var x in [1]) x]; print(b); }",
        "Collection for-elements are not supported yet",
        "for",
    );
}

/// Item 4: `?.` curto-circuita para null sem avaliar o resto da cadeia.
#[test]
#[ignore = "requer acesso null-aware (nó AST ainda ausente no subconjunto)"]
fn acesso_null_aware_tem_mensagem_propria() {
    rejeita_apos(
        "void main() { int? z = null; print(z?.isEven); }",
        "Null-aware access is not supported yet",
        "z?.isEven",
        "?.",
    );
}

/// Item 5a: `~/` trunca para zero e erra em divisor zero (oráculo Dart).
#[test]
#[ignore = "requer TruncDivide ponta a ponta (braços do agente de double pendentes)"]
fn divisao_inteira_reproduz_o_oraculo() {
    let js = compile("void main() { print(7 ~/ 2); print(-7 ~/ 2); print(7 ~/ -2); }")
        .unwrap();
    assert_eq!(executar(&js), "3\n-3\n-3\n");
}

/// Item 5b: bitwise com semântica i32 (o JS já opera em 32 bits; literais do
/// subconjunto limitados a i32 pelo parser).
#[test]
#[ignore = "requer operadores bitwise (tokens ainda ausentes no lexer)"]
fn bitwise_reproduz_o_oraculo_i32() {
    let js = compile(
        "void main() { print(5 | 3); print(5 & 3); print(5 ^ 3); print(~5); print(1 << 3); print(8 >> 2); print(-8 >> 2); }",
    )
    .unwrap();
    assert_eq!(executar(&js), "7\n1\n6\n-6\n8\n2\n-2\n");
}

/// Item 6: `Enum.values` é a lista canônica congelada dos valores, em ordem.
#[test]
#[ignore = "requer Enum.values (tipagem da lista canônica ainda ausente)"]
fn enum_values_e_a_lista_canonica() {
    let js = compile(
        "enum Cor { vermelho, verde, azul }\
         void main() { print(Cor.values.length); print(Cor.values[0]); print(Cor.values[2].index); }",
    )
    .unwrap();
    assert_eq!(executar(&js), "3\nCor.vermelho\n2\n");
}

/// Item 7: Map const canônico, como as listas const já são.
#[test]
#[ignore = "requer Map const (valor ConstValue::Map ainda ausente no subconjunto)"]
fn mapa_const_tem_mensagem_propria() {
    rejeita_apos(
        "void main() { const m = {'a': 1}; print(m); }",
        "Const Map literals are not supported yet",
        "const m = {'a': 1}",
        "{'a': 1}",
    );
}

/// Item 6 (LLVM): o backend nativo rejeita `Enum.values` com span exato.
#[test]
#[ignore = "requer Enum.values no frontend antes da rejeição nativa"]
fn enum_values_no_llvm_e_rejeitado() {
    let error =
        compile_llvm("enum Cor { vermelho, verde } void main() { print(Cor.values); }")
            .unwrap_err();
    assert_eq!(
        error.message,
        "LLVM AOT ainda não suporta Enum.values no backend nativo"
    );
}

/// Fixture integral executada em Node contra a saída dos dois SDKs.
#[test]
#[ignore = "requer itens 1-7 implementados no backend JavaScript"]
fn fixture_integral_reproduz_os_oraculos() {
    const FONTE: &str = include_str!("../../../tests/conformance/modules/colecoesops/main.dart");
    assert_eq!(executar(&compile(FONTE).unwrap()), ESPERADO_FIXTURE);
}
