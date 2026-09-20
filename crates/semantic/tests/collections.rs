//! Regressões de funções como valores, inferência de coleções e segurança de capturas.
/// Analisa uma fonte real e devolve a tabela usada pelos backends.
fn analyze(source: &str) -> Result<dartforge_syntax::Resolution, String> {
    let tokens = dartforge_lexer::lex(source).map_err(|e| e.message)?;
    let program = dartforge_parser::parse(&tokens, source.len()).map_err(|e| e.message)?;
    dartforge_semantic::analyze(&program).map_err(|e| e.message)
}
/// Callbacks recebem contexto do elemento e devolvem formas inferidas persistentes.
#[test]
fn collection_callbacks_and_resolved_types() {
    let result=analyze("void main() { var xs = [1, 2, 3]; var ys = xs.where((x) => x % 2 == 0).map((x) => x + 1).toList(); ys.add(4); ys[0] = 2; print(ys[0]); print(ys.length); print(ys.isEmpty); print(ys.isNotEmpty); print(ys.first); print(ys.last); print(ys.any((x) => x > 0)); ys.forEach((x) { print(x); }); }").unwrap();
    assert!(result.types.iter().any(|t| matches!(
        t,
        dartforge_syntax::TypeShape::Function {
            result: dartforge_syntax::Type::Bool,
            ..
        }
    )));
    assert!(!result.expr_types.is_empty());
}
/// Valores funcionais mantêm assinaturas em variáveis, parâmetros, resultados e aliases.
#[test]
fn function_values_and_mutable_captures() {
    for source in [
        "int twice(int x) => x * 2; int apply(int Function(int) f, int n) => f(n); void main() { var f = twice; print(apply(f, 3)); int Function(int) g = (x) => x + 1; print(g(4)); }",
        "int Function(int) make(int n) { return (x) => x + n; } void main() { var f = make(2); print(f(3)); }",
        "void main() { var count = 0; var inc = () { count = count + 1; }; inc(); inc(); print(count); }",
        "void main() { var f = (int x) => x + 1; var alias = f; print(alias(1)); print(((int x) => x * 2)(3)); }",
        "void main() { List<int> xs = []; Iterable<int> ys = xs; xs.add(1); print(ys.first); List<int> copy = ys.toList(); print(copy); }",
        "void main() { List<List<int>> xs = [[], [1]]; print(xs[0]); }",
        "class C { List<int> values = []; } void main() { var c = C(); c.values.add(1); }",
    ] {
        analyze(source).unwrap_or_else(|e| panic!("{source}\n{e}"));
    }
}
/// List é invariante; callbacks inválidos, ausência de contexto e índices errados falham.
#[test]
fn rejects_unsafe_aliases_and_callback_types() {
    for source in [
        "void main() { List<int> xs = [1]; List<int?> alias = xs; alias.add(null); }",
        "void main() { var xs = [1]; xs.add(false); }",
        "void main() { var xs = [1]; xs[true] = 2; }",
        "void main() { var xs = [1]; xs[0] = false; }",
        "void main() { var xs = [1]; xs.where((x) => x + 1); }",
        "void main() { var xs = [1]; xs.forEach((bool x) { print(x); }); }",
        "void main() { var xs = []; }",
        "void main() { var xs = [1, false]; }",
        "void main() { var f = (x) => x + 1; }",
        "void main() { int Function(int) f = (x) => true; }",
        "void main() { var f = () => 1; print(f); }",
        "void main() { var f = (int x) => x; f(true); }",
        "void main() { var f = () { break; }; }",
        "void main() { while (true) { var f = () { continue; }; break; } }",
        "void main() { final n = 0; var f = () { n = 1; }; }",
    ] {
        assert!(analyze(source).is_err(), "unexpected success: {source}");
    }
}
/// Escritas capturadas invalidam promoções, inclusive antes da criação ou chamada.
#[test]
fn closures_do_not_reuse_unsafe_promotions() {
    for source in [
        "void main() { int? x = 1; var clear = () { x = null; }; if (x != null) { clear(); print(x + 1); } }",
        "void main() { int? x = 1; var read = () => x + 1; x = null; print(read()); }",
        "void main() { int? x = 1; if (x != null) { var clear = () { x = null; }; print(x + 1); } }",
    ] {
        assert!(analyze(source).is_err(), "unexpected success: {source}");
    }
    analyze("void main() { final int? x = 1; var read = () => x + 1; print(read()); }").unwrap();
    analyze("void main() { int? x = 1; var read = () { if (x == null) { return 0; } return x + 1; }; print(read()); }").unwrap();
}
/// O mesmo tipo estrutural pode ter IDs locais distintos sem alterar sua equivalência.
#[test]
fn duplicated_structural_ids_compare_by_shape() {
    use dartforge_syntax::{Type, TypeShape};
    let source = "int apply(int Function(int) f) => f(1); int twice(int x) => x * 2; void main() { print(apply(twice)); }";
    let tokens = dartforge_lexer::lex(source).unwrap();
    let mut program = dartforge_parser::parse(&tokens, source.len()).unwrap();
    let shape = TypeShape::Function {
        result: Type::Int,
        parameters: vec![Type::Int],
    };
    program.types.push(shape.clone());
    program.types.push(shape);
    let id = (program.types.len() - 1) as u32;
    program.functions[0].parameters[0].ty = Type::Applied(id);
    dartforge_semantic::analyze(&program).unwrap();
}

/// Arrow em contexto void descarta valor; bloco com return de valor é erro Dart.
#[test]
fn contextual_void_distinguishes_arrow_and_block() {
    analyze("void main() { void Function() f = () => 1; f(); [1].forEach((x) => x + 1); }")
        .unwrap();
    assert!(analyze("void main() { void Function() f = () { return 1; }; }").is_err());
    assert!(analyze("void main() { [1].forEach((x) { return x + 1; }); }").is_err());
    analyze("void main() { var f = (int x) { return x + 1; }; [1].forEach(f); }").unwrap();
}

/// Impressão recursiva não aplica silenciosamente toString JavaScript a funções e objetos.
#[test]
fn collection_print_rejects_unsupported_element_to_string() {
    assert!(analyze("class C {} void main() { print([C()]); }").is_err());
    assert!(analyze("void main() { print([() => 1]); }").is_err());
    assert!(analyze("void main() { print([[() => 1]]); }").is_err());
    analyze("void main() { print([[1, 2], [3]]); print([true, false]); print([1, null]); }")
        .unwrap();
}
