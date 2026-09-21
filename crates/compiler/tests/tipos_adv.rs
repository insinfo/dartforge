//! Sistema de tipos avançado com erasure honesto, comparado com o SDK Dart.
//!
//! Classes genéricas apagam argumentos para o bound (`T` vira o bound no
//! parse); `typedef` moderno/legado resolve para o tipo no parse; extension
//! types viram a representação com despacho estático; `late` com inicializador
//! é ansioso; `dynamic` segue rejeitado no parse (gap documentado). Saídas de
//! execução conferidas com `dart run` (SDK 3.6.2 no PATH e 3.13.4) e
//! `dart analyze` limpo (só lint `unnecessary_type_check` nos `is`).
use dartforge_compiler::{compile, compile_llvm};

/// Classe genérica com bound, herança parametrizada, `is` e inferência em `C(1)`.
const GENERIC_CLASS: &str = "class Box<T extends int> {\n  T v;\n  Box(this.v);\n  T bump(T x) {\n    return x;\n  }\n}\nclass DBox extends Box<int> {\n  DBox(int v) : super(v) {}\n}\nvoid main() {\n  var b = Box(1);\n  Box<int> c = Box(2);\n  var d = DBox(3);\n  print(b.v + c.v + d.v);\n  print(b is Box<int>);\n  print(d is Box<int>);\n  print(b.bump(10));\n}\n";

/// Classe genérica mínima sem `is` (forma que o LLVM AOT aceita por erasure).
const GENERIC_CLASS_LLVM: &str = "class C<T extends int> {\n  T v;\n  C(this.v);\n}\nvoid main() {\n  var c = C(41);\n  print(c.v + 1);\n}\n";

/// `typedef` moderno e legado, uso em assinatura e `is`.
const TYPEDEF: &str = "typedef F = int Function(int);\ntypedef int G(int x);\nint applyF(F f, int x) {\n  return f(x);\n}\nvoid main() {\n  F f = (int x) {\n    return x + 1;\n  };\n  print(applyF(f, 1));\n  G g = (int x) {\n    return x * 2;\n  };\n  print(g(3));\n  print(f is F);\n}\n";

/// Extension type com erasure para a representação e despacho estático.
const EXTENSION_TYPE: &str = "extension type Id(int v) {\n  int answer() {\n    return 42;\n  }\n}\nvoid main() {\n  var id = Id(21);\n  print(id);\n  print(id.answer());\n  print(id is Id);\n}\n";

/// `late` com inicializador (erasure ansioso; sem célula de verificação ainda).
const LATE_CELL: &str = "void main() {\n  late int x;\n  late final int y;\n  x = 40 + 2;\n  y = 7;\n  print(x + y);\n}\n";

/// Erasure de classe genérica compila para JavaScript com a representação única.
#[test]
fn generic_classes_compile_with_erased_bounds() {
    let js = compile(GENERIC_CLASS).unwrap();
    assert!(js.contains("export function main"));
}

/// `typedef` moderno e legado compilam; o apelido vira o tipo subjacente.
#[test]
fn typedefs_compile_to_the_aliased_type() {
    let js = compile(TYPEDEF).unwrap();
    assert!(js.contains("export function main"));
}

/// Extension type compila: representação + método estático + `is` da representação.
#[test]
fn extension_types_compile_with_representation_erasure() {
    let js = compile(EXTENSION_TYPE).unwrap();
    assert!(js.contains("export function main"));
}

/// `late` com inicializador é recusado: a avaliação seria ansiosa, e em Dart o
/// inicializador só roda na primeira leitura. O span cobre a palavra `late`.
#[test]
fn late_with_an_initializer_is_rejected_with_an_exact_span() {
    let source = "void main(){late int x = 1; print(x);}";
    let error = compile(source).expect_err("late com inicializador deve ser recusado");
    assert_eq!(
        error.message,
        "late with an initializer is not supported: in Dart the initializer runs on the first read and a write before that read cancels it; declare `late T name;` and assign before reading"
    );
    assert_eq!((error.span.start, error.span.end), (12, 16));
}

/// Parâmetro de classe duplicado informa mensagem e span exatos do segundo `T`.
#[test]
fn duplicate_class_type_parameter_reports_exact_span() {
    let source = "class C<T, T> {} void main(){}";
    let error = compile(source).expect_err("parâmetro duplicado deve ser rejeitado");
    assert_eq!(error.message, "duplicate class type parameter");
    assert_eq!((error.span.start, error.span.end), (11, 12));
}

/// `typedef` duplicado informa mensagem e span exatos do segundo apelido.
#[test]
fn duplicate_typedef_reports_exact_span() {
    let source = "typedef F = int; typedef F = int; void main(){}";
    let error = compile(source).expect_err("apelido duplicado deve ser rejeitado");
    assert_eq!(error.message, "duplicate typedef name");
    assert_eq!((error.span.start, error.span.end), (25, 26));
}

/// `typedef` genérico informa a rejeição honesta no ponto do `<`.
#[test]
fn generic_typedef_reports_exact_span() {
    let source = "typedef F<T> = T; void main(){}";
    let error = compile(source).expect_err("typedef genérico deve ser rejeitado");
    assert_eq!(error.message, "generic typedefs are not supported yet");
    assert_eq!((error.span.start, error.span.end), (9, 10));
}

/// Construtor explícito em extension type informa mensagem e span exatos.
#[test]
fn extension_type_constructor_reports_exact_span() {
    let source = "extension type Id(int v) { Id(int x) { return x; } } void main(){}";
    let error = compile(source).expect_err("construtor explícito deve ser rejeitado");
    assert_eq!(
        error.message,
        "extension type constructors are not supported yet: use the primary representation constructor"
    );
    assert_eq!((error.span.start, error.span.end), (27, 29));
}

/// `late` sem inicializador é aceito e emite a checagem de inicialização.
#[test]
fn late_without_initializer_emits_the_initialization_check() {
    let js = compile("void main(){late int x; x = 1; print(x);}").unwrap();
    assert!(js.contains("$dartforgeLate = Symbol"));
    assert!(js.contains("$dartforgeLateRead($df_x, \"Local\", \"x\")"));
}

/// `late const` é inválido em Dart e segue rejeitado com span do `const`.
#[test]
fn late_const_reports_exact_span() {
    let source = "void main(){late const int x = 1;}";
    let error = compile(source).expect_err("late const deve ser rejeitado");
    assert_eq!(error.message, "late const is not supported");
    assert_eq!((error.span.start, error.span.end), (18, 23));
}

/// `dynamic` segue rejeitado no parse (gap: exige `Type::Dynamic` + aceite total).
#[test]
fn dynamic_reports_exact_span() {
    let source = "void main(){dynamic x = 1;}";
    let error = compile(source).expect_err("dynamic deve ser rejeitado no parse");
    assert_eq!(error.message, "expected a supported expression");
    assert_eq!((error.span.start, error.span.end), (13, 20));
}

/// LLVM compila a classe genérica apagada (sem parâmetros simbólicos no HIR).
#[test]
fn llvm_compiles_erased_generic_classes() {
    compile_llvm(GENERIC_CLASS_LLVM).unwrap();
}

/// LLVM rejeita `is`/`as` reificados, mesmo em código morto.
#[test]
fn llvm_rejects_reified_type_tests() {
    let error = compile_llvm("void main(){print(1 is int);}")
        .expect_err("LLVM não deve apagar testes de tipo");
    assert!(
        error.message.contains("LLVM"),
        "mensagem inesperada: {error:?}"
    );
}

/// LLVM rejeita os métodos estáticos de extension types como extensions.
#[test]
fn llvm_rejects_extension_type_methods() {
    let error = compile_llvm(EXTENSION_TYPE).expect_err("LLVM não cobre extensions");
    assert!(
        error.message.contains("extensions"),
        "mensagem inesperada: {error:?}"
    );
}

/// Saídas idênticas às do SDK (`dart run` 3.6.2 e 3.13.4) via Node.js.
#[test]
#[ignore = "requer Node.js no PATH"]
fn advanced_types_match_dart_stdout() {
    for (source, expected) in [
        (GENERIC_CLASS, "6\ntrue\ntrue\n10\n"),
        (GENERIC_CLASS_LLVM, "42\n"),
        (TYPEDEF, "2\n6\ntrue\n"),
        (EXTENSION_TYPE, "21\n42\ntrue\n"),
        (LATE_CELL, "49\n"),
    ] {
        let js = compile(source).unwrap();
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &js])
            .output()
            .expect("Node.js necessário");
        assert!(
            output.status.success(),
            "{source}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            expected,
            "{source}"
        );
    }
}
