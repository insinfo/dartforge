//! Recursos de linguagem posteriores ao Dart 3.6.2 aceitos pelo front-end.
//!
//! Cobre curingas `_` (3.7), elementos null-aware `?valor` (3.8), atalhos de
//! ponto `.membro` (3.10) e construtores primários `class C(...)` (3.13).
use dartforge_compiler::{CompileOptions, Optimization, compile, compile_path_with_options};

/// Executa a fixture completa em todas as combinações de otimização.
#[test]
#[ignore = "requer Node.js no PATH"]
fn modern_features_run_in_every_optimization_mode() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/modules/modern24/main.dart");
    let expected = include_str!("../../../tests/conformance/modules/modern24/main.stdout")
        .replace("\r\n", "\n");
    for optimization in [Optimization::None, Optimization::Constants] {
        for tree_shaking in [false, true] {
            let js = compile_path_with_options(
                &path,
                CompileOptions {
                    optimization,
                    merge_identical_functions: false,
                    tree_shaking,
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
                expected,
                "otimização {optimization:?}, tree shaking {tree_shaking}"
            );
        }
    }
}

/// Curingas repetidos não colidem e nunca aparecem como referência no JavaScript.
#[test]
fn wildcards_never_bind_a_readable_name() {
    let js = compile(
        "int aplicar(int Function(int, int) f) => f(3, 4);
         void main() {
           var _ = 1;
           var _ = 2;
           print(aplicar((a, _) => a + 10));
         }",
    )
    .unwrap();
    assert!(js.contains("$df_$wild0"));
    assert!(js.contains("$df_$wild1"));
    assert!(js.contains("$df_$wild2"));
    let error = compile("void main() { var _ = 1; print(_); }").unwrap_err();
    assert_eq!(error.message, "Unknown identifier '_'");
}

/// O operando de `?` é avaliado uma única vez e o elemento some quando é null.
#[test]
fn null_aware_elements_drop_null_without_repeating_effects() {
    let js = compile(
        "String? valor(String? v) { print('efeito'); return v; }
         void main() {
           var lista = [?valor(null), ?valor('x')];
           print(lista.length);
         }",
    )
    .unwrap();
    // Uma ocorrência é a declaração da função; as outras duas são as chamadas.
    assert_eq!(js.matches("$df_valor(").count(), 3);
    let error = compile("void main() { print(?null); }").unwrap_err();
    assert_eq!(error.message, "expected a supported expression");
}

/// A entrada inteira do mapa desaparece quando qualquer posição `?` é null.
#[test]
fn null_aware_map_entries_disappear_as_a_whole() {
    let js = compile(
        "void main() {
           String? ausente = null;
           var mapa = <String, int>{'a': 1, ?ausente: 2};
           print(mapa.length);
         }",
    )
    .unwrap();
    assert!(js.contains("$dartforgeKey === null"));
}

/// Sem tipo de contexto o atalho de ponto é rejeitado, não inferido.
#[test]
fn dot_shorthands_require_a_context_type() {
    let error = compile(
        "enum Cor { azul }
         void main() { var c = .azul; print(c); }",
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "Dot shorthand requires a context type in this position"
    );
    let error = compile(
        "enum Cor { azul }
         void pintar(Cor c) { print(c.name); }
         void main() { pintar(.verde); }",
    )
    .unwrap_err();
    assert_eq!(error.message, "'verde' is not an enum value of 'Cor'");
}

/// `.new` só designa o construtor sem nome e exige lista de argumentos.
#[test]
fn dot_shorthand_new_targets_the_unnamed_constructor() {
    let js = compile(
        "class Ponto { final int x; Ponto(this.x); }
         void mostrar(Ponto p) { print(p.x); }
         void main() { mostrar(.new(7)); }",
    )
    .unwrap();
    assert!(js.contains("$dartforgeNew0(7)") || js.contains("new $dartforgeClass0(7)"));
    let error = compile(
        "class Ponto { final int x; Ponto(this.x); }
         void mostrar(Ponto p) { print(p.x); }
         void main() { mostrar(.new); }",
    )
    .unwrap_err();
    assert_eq!(error.message, "`.new` requires an argument list");
}

/// A lista primária declara campos na ordem escrita e dispensa o corpo.
#[test]
fn primary_constructors_declare_fields_in_header_order() {
    let js = compile(
        "class Cliente(String nome, String email);
         void main() {
           var c = Cliente('a', 'b');
           print(c.nome);
           print(c.email);
         }",
    )
    .unwrap();
    assert!(js.contains("$df_nome"));
    assert!(js.contains("$df_email"));
    let error = compile(
        "class Cliente(String nome) { final String nome; }
         void main() { print(Cliente('a').nome); }",
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "a primary constructor field cannot be redeclared in the class body"
    );
}

/// `this.campo` na lista primária exige o campo declarado no corpo.
#[test]
fn primary_this_parameters_require_a_declared_field() {
    let js = compile(
        "class Legado(this.codigo) { final int codigo; int dobro() => codigo * 2; }
         void main() { print(Legado(21).dobro()); }",
    )
    .unwrap();
    assert!(js.contains("$df_codigo"));
    let error = compile(
        "class Legado(this.codigo);
         void main() { print(Legado(21).codigo); }",
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "a this. primary parameter requires the field declared in the class body"
    );
}

/// Um construtor sem nome escrito no corpo conflita com a lista primária.
#[test]
fn primary_constructors_reject_a_second_unnamed_constructor() {
    let error = compile(
        "class Cliente(String nome) { Cliente(this.nome); }
         void main() { print(Cliente('a').nome); }",
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "a class with a primary constructor cannot declare another unnamed constructor"
    );
}
