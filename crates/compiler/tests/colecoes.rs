//! Conjuntos, espalhamentos, elementos `if`/`for`, `?.` e operadores de bits.
//!
//! A saída esperada de `tests/conformance/cases/colecoes.dart` foi copiada de
//! `dart run` sem edição e é idêntica nos dois SDKs — Dart 3.6.2 (`dart` no
//! PATH) e Dart 3.13.4 (`D:/DartSDKs/3.13.4/dart-sdk/bin/dart.exe`) — e também
//! em `dart compile js -O2` rodado no Node. O fixture passa limpo por
//! `dart analyze` nos dois SDKs.
//!
//! `tests/conformance/cases/colecoes_bits.dart` é o caso em que a VM e o alvo
//! web **não** concordam: `int` na web é um `double` de 64 bits e os operadores
//! de bits trabalham sobre 32 bits sem sinal. O DartForge segue o alvo web, que
//! é o oráculo do `scripts/conformance.ps1`, então a saída esperada desse
//! fixture vem de `dart compile js -O2`; a saída da VM está registrada em
//! `ESPERADO_BITS_VM` para que a divergência fique explícita e conferível.
//! O contrato completo está em `docs/COLECOES-OPERADORES.md`.
use dartforge_compiler::{
    CompileOptions, Optimization, compile, compile_llvm, compile_with_options,
};

const FIXTURE: &str = include_str!("../../../tests/conformance/cases/colecoes.dart");
const BITS: &str = include_str!("../../../tests/conformance/cases/colecoes_bits.dart");

/// Saída exata de `dart run tests/conformance/cases/colecoes.dart`.
const ESPERADO: &str = "{3, 1, 2}\n\
3\n\
true\n\
false\n\
false\n\
true\n\
{3, 1, 2, 9}\n\
3\n\
1\n\
2\n\
9\n\
true\n\
false\n\
{}\n\
{}\n\
5\n\
[5, 6]\n\
1\n\
[1, 2, 3]\n\
{1, 2, 4}\n\
talvez\n\
[9]\n\
talvez\n\
[1, 2, 9]\n\
{a: 1, b: 2}\n\
{b: 2}\n\
{a: 7}\n\
[1, 2]\n\
[sim]\n\
[]\n\
[2]\n\
[0, 1, 4]\n\
[10, 20, 20, 40]\n\
[2]\n\
[0, 1, 2, 3, 11, 12]\n\
{1, 2}\n\
{x: 1}\n\
{p: 7, q: 7}\n\
{k: 1}\n\
fonte\n\
null\n\
fonte\n\
valor\n\
1\n\
fonte\n\
valor\n\
2\n\
fonte\n\
null\n\
fonte\n\
dobro\n\
2\n\
talvez\n\
null\n\
talvez\n\
1\n\
2\n\
null\n\
1\n\
3\n\
2\n\
7\n\
5\n\
251\n\
48\n\
1073741824\n\
15\n\
15\n\
0\n\
1\n\
20\n\
11\n\
24\n";

/// Saída exata de `dart compile js -O2` para `colecoes_bits.dart`, no Node.
const ESPERADO_BITS_WEB: &str = "4294967295\n\
4294967290\n\
4294967295\n\
4294967294\n\
4294967292\n\
15\n\
4294967295\n\
4294967294\n\
2147483648\n\
0\n\
0\n\
0\n\
4294967295\n\
2147483648\n";

/// Saída exata de `dart run` para o mesmo programa: a VM usa int de 64 bits.
///
/// Está aqui como registro da divergência conhecida entre a VM e o alvo web,
/// não como resultado esperado do DartForge. Só as duas últimas linhas
/// coincidem com `ESPERADO_BITS_WEB`.
#[allow(dead_code)]
const ESPERADO_BITS_VM: &str = "-1\n\
-6\n\
-1\n\
-2\n\
-4\n\
68719476735\n\
-1\n\
-2\n\
2147483648\n\
4294967296\n\
1099511627776\n\
0\n\
-1\n\
2147483648\n";

/// Exercita as transformações opcionais, que não podem mudar o observável.
fn opcoes() -> impl Iterator<Item = CompileOptions> {
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

/// Compila o programa e devolve a mensagem do diagnóstico esperado.
fn recusa(fonte: &str) -> String {
    let programa = format!(
        "int? talvez() => null;\nList<int>? lista() => null;\nvoid main() {{\n{fonte}\n}}\n"
    );
    match compile(&programa) {
        Ok(_) => panic!("aceito sem diagnóstico: {fonte}"),
        Err(erro) => erro.message,
    }
}

/// Compila e devolve o JavaScript, com mensagem útil quando falha.
fn javascript(fonte: &str) -> String {
    compile(fonte).unwrap_or_else(|erro| panic!("{erro:?}"))
}

/// Executa o módulo no Node e devolve a saída normalizada.
fn node(javascript: &str) -> String {
    let saida = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", javascript])
        .output()
        .expect("Node.js necessário");
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    String::from_utf8(saida.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

/// Os dois fixtures compilam em todos os modos, mesmo sem Node no ambiente.
#[test]
fn fixtures_de_colecoes_compilam_em_todos_os_modos() {
    for opcoes in opcoes() {
        for fonte in [FIXTURE, BITS] {
            compile_with_options(fonte, opcoes)
                .unwrap_or_else(|erro| panic!("{opcoes:?}: {erro:?}"));
        }
    }
}

/// Node precisa reproduzir a saída do SDK, inclusive a ordem dos efeitos.
#[test]
#[ignore = "requer Node.js no PATH"]
fn colecoes_batem_com_a_saida_do_sdk_em_node() {
    for opcoes in opcoes() {
        let modulo = compile_with_options(FIXTURE, opcoes).unwrap();
        assert_eq!(node(&modulo), ESPERADO, "{opcoes:?}");
    }
}

/// Os operadores de bits seguem o alvo web, conferido contra `dart compile js`.
#[test]
#[ignore = "requer Node.js no PATH"]
fn bits_batem_com_o_alvo_web_em_node() {
    for opcoes in opcoes() {
        let modulo = compile_with_options(BITS, opcoes).unwrap();
        assert_eq!(node(&modulo), ESPERADO_BITS_WEB, "{opcoes:?}");
    }
    assert_ne!(
        ESPERADO_BITS_WEB, ESPERADO_BITS_VM,
        "a divergência com a VM é parte do contrato registrado"
    );
}

/// `...` sobre null só é alcançável por um cast, que lança antes do espalhamento.
#[test]
#[ignore = "requer Node.js no PATH"]
fn espalhar_null_lanca_em_execucao() {
    let modulo = javascript(
        "List<int>? talvez(bool b) => b ? <int>[1] : null;\n\
         void main() {\n  print([...(talvez(false) as List<int>)]);\n}\n",
    );
    let saida = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &modulo])
        .output()
        .expect("Node.js necessário");
    assert!(!saida.status.success(), "espalhar null deveria lançar");
    assert!(String::from_utf8_lossy(&saida.stdout).is_empty());
}

/// O receptor de uma cadeia `?.` aparece uma única vez no JavaScript emitido.
///
/// Duplicar a subexpressão seria invisível em muitos programas e visível
/// justamente onde importa: com efeitos observáveis. O teste trava a forma
/// emitida, não só o resultado, porque o resultado sozinho não distingue uma
/// segunda avaliação de uma expressão pura.
#[test]
fn cadeia_null_aware_avalia_o_receptor_uma_vez() {
    let modulo = javascript(
        "class Caixa {\n  Caixa? proxima;\n  int v;\n  Caixa(this.v);\n}\n\
         Caixa? fonte() => null;\n\
         void main() {\n  print(fonte()?.proxima?.v);\n}\n",
    );
    let main_body = modulo.split("export function main").nth(1).unwrap();
    assert_eq!(main_body.matches("$df_fonte()").count(), 1, "{modulo}");
    assert_eq!(modulo.matches("$dartforgeShort0").count(), 3, "{modulo}");
    assert_eq!(modulo.matches("$dartforgeShort1").count(), 3, "{modulo}");
}

/// O operando de `...?` também aparece uma única vez.
#[test]
fn espalhamento_null_aware_avalia_o_operando_uma_vez() {
    let modulo =
        javascript("List<int>? fonte() => null;\nvoid main() {\n  print([...?fonte(), 1]);\n}\n");
    let main_body = modulo.split("export function main").nth(1).unwrap();
    assert_eq!(main_body.matches("$df_fonte()").count(), 1, "{modulo}");
    assert!(modulo.contains("$dartforgeSpread("), "{modulo}");
}

/// O caminho comum continua sendo o literal de array, sem construtor imperativo.
#[test]
fn literal_simples_nao_usa_construtor_imperativo() {
    let modulo = javascript("void main() {\n  print(<int>[1, 2, 3]);\n}\n");
    assert!(modulo.contains("new $dartforgeList([1,2,3]"), "{modulo}");
    assert!(!modulo.contains("$dartforgeBuild"), "{modulo}");
    let com_espalhamento =
        javascript("void main() {\n  final a = <int>[1];\n  print(<int>[...a, 2]);\n}\n");
    assert!(
        !com_espalhamento.contains("$dartforgeBuild"),
        "{com_espalhamento}"
    );
}

/// Um conjunto usa a representação própria e preserva a ordem de inserção.
#[test]
fn conjunto_usa_representacao_propria() {
    let modulo = javascript("void main() {\n  print(<int>{2, 1});\n}\n");
    assert!(
        modulo.contains("new $dartforgeSet([2,1],['int'])"),
        "{modulo}"
    );
    assert!(modulo.contains("class $dartforgeSet"), "{modulo}");
}

/// `{}` continua sendo mapa, menos quando o contexto pede um conjunto.
#[test]
fn chaves_vazias_seguem_o_contexto() {
    let mapa = javascript("void main() {\n  print(<String, int>{});\n}\n");
    assert!(mapa.contains("new $dartforgeMap("), "{mapa}");
    let conjunto = javascript("void main() {\n  Set<int> s = {};\n  print(s.length);\n}\n");
    assert!(
        conjunto.contains("new $dartforgeSet([],['int'])"),
        "{conjunto}"
    );
    assert!(!conjunto.contains("new $dartforgeMap("), "{conjunto}");
}

/// Formas de literal que o parser recusa, com a mensagem exata.
#[test]
fn limites_sintaticos_de_literais_de_colecao() {
    assert_eq!(
        recusa("print({1, 'a': 2});"),
        "set literals do not accept `key: value` entries"
    );
    assert_eq!(
        recusa("print(<String, int>{1});"),
        "expected Symbol(':'); supported subset only"
    );
    assert_eq!(
        recusa("print({'a': 1, 2});"),
        "map literals require `key: value` entries"
    );
    assert_eq!(
        recusa("final a = <int>[1];\n  print({...a});"),
        "a literal built only from spreads needs explicit <T> or <K, V> type arguments"
    );
    assert_eq!(
        recusa("print({if (1 < 2) 'a': 1});"),
        "if and for elements in map literals require explicit <K, V> type arguments"
    );
    assert_eq!(
        recusa("final a = <int>[1];\n  print([...?...?a]);"),
        "expected a supported expression"
    );
    assert_eq!(
        recusa("print([? ?1]);"),
        "nested null-aware collection elements are not supported"
    );
    assert_eq!(
        recusa("final a = <int>[1];\n  print([for await (final x in a) x]);"),
        "await for in collection literals is not supported"
    );
}

/// Formas que a análise semântica recusa, com a mensagem exata.
#[test]
fn limites_semanticos_de_colecoes_e_operadores() {
    assert_eq!(
        recusa("print({});"),
        "Empty Map requires an explicit or contextual value type"
    );
    assert_eq!(
        recusa("print([...lista()]);"),
        "A nullable expression cannot be spread; use `...?`"
    );
    assert_eq!(
        recusa("print(<String, int>{...lista()});"),
        "A nullable expression cannot be spread; use `...?`"
    );
    assert_eq!(
        recusa("print([...1]);"),
        "Spread requires a List, Set or Iterable"
    );
    assert_eq!(
        recusa("final a = <int>[1];\n  print(<String, int>{...a});"),
        "Spread in a map requires a Map"
    );
    assert_eq!(
        recusa("final a = <int>[1];\n  print(a?.length);"),
        "Null-aware access requires a nullable receiver"
    );
    assert_eq!(
        recusa("print(1.5 & 2);"),
        "Type mismatch: expected Int, found Double"
    );
    assert_eq!(
        recusa("print(~1.5);"),
        "Type mismatch: expected Int, found Double"
    );
    assert_eq!(
        recusa("print(1 << talvez());"),
        "Type mismatch: expected Int, found NullableInt"
    );
    assert_eq!(
        recusa("print([if (1) 2]);"),
        "Type mismatch: expected Bool, found Int"
    );
}

/// O `>` de argumentos de tipo continua fechando genéricos, não deslocando.
#[test]
fn deslocamento_nao_captura_o_fecha_de_genericos() {
    javascript("void main() {\n  print(<List<int>>[<int>[1]]);\n}\n");
    let modulo = javascript("void main() {\n  print(8 >> 1);\n  print(8 > 1);\n}\n");
    assert!(modulo.contains("$dartforgeShr("), "{modulo}");
    // `a > > b` com espaço não é deslocamento; o parser recusa a segunda `>`.
    assert_eq!(recusa("print(8 > > 1);"), "expected a supported expression");
}

/// O backend LLVM recusa as formas novas antes do driver, com mensagem própria.
#[test]
fn llvm_recusa_conjuntos_espalhamentos_e_bits() {
    let recusa_llvm = |fonte: &str| {
        let programa = format!("void main() {{\n{fonte}\n}}\n");
        match compile_llvm(&programa) {
            Ok(_) => panic!("LLVM aceitou: {fonte}"),
            Err(erro) => erro.message,
        }
    };
    assert_eq!(
        recusa_llvm("print(1 & 2);"),
        "LLVM AOT ainda não suporta operadores de bits e deslocamento (`&`, `|`, `^`, `~`, `<<`, `>>`, `>>>`)"
    );
    assert_eq!(
        recusa_llvm("print(1 << 2);"),
        "LLVM AOT ainda não suporta operadores de bits e deslocamento (`&`, `|`, `^`, `~`, `<<`, `>>`, `>>>`)"
    );
    assert_eq!(
        recusa_llvm("print(~1);"),
        "LLVM AOT ainda não suporta o complemento de bits `~`"
    );
}
