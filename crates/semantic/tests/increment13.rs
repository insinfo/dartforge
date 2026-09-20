//! Regressões de genéricos, const, enums aprimorados, getters e cobertura de switch.
/// Analisa a fonte pelo frontend real e retorna metadados ou diagnóstico textual.
fn analyze(source: &str) -> Result<dartforge_syntax::Resolution, String> {
    let tokens = dartforge_lexer::lex(source).map_err(|e| e.message)?;
    let program = dartforge_parser::parse(&tokens, source.len()).map_err(|e| e.message)?;
    dartforge_semantic::analyze(&program).map_err(|e| e.message)
}
/// Identidade, coleções e callbacks inferem parâmetros sem introduzir dynamic.
#[test]
fn generic_calls_infer_and_substitute_structural_types() {
    for source in [
        "T id<T>(T value) => value; List<T> one<T>(T x) => <T>[x]; void main() { print(id<int>(1)); print(id('x')); print(one(2)[0]); }",
        "R apply<T,R>(T x,R Function(T) f) => f(x); void main() { print(apply(1,(n) => n + 1)); print(apply<int,String>(1,(n) => 'x')); }",
        "List<R> convert<T,R>(List<T> xs,R Function(T) f) => xs.map(f).toList(); void main() { print(convert([1],(n) => n + 1)); }",
    ] {
        analyze(source).unwrap_or_else(|e| panic!("{source}\n{e}"));
    }
}
/// Const referencia apenas outros const e preserva metadados para canonicalização.
#[test]
fn const_bindings_and_types_are_checked() {
    let result=analyze("void main() { const x = 1 + 1; const a = <int>[x]; const b = <int>[2]; print(a == b); print(const <int>[]); }").unwrap();
    assert!(result.constant_values.len() >= 4);
    for source in [
        "void main() { final x = 1; const y = x; }",
        "int f() => 1; void main() { const x = f(); }",
        "void main() { const x = 1; x = 2; }",
        "void main() { const xs = <int>[true]; }",
        "void f<T>() { const xs = <T>[]; } void main() {}",
        "T id<T>(T x) => x; void main() { var f = id; }",
        "T id<T>(T x) => x; void main() { id<int>(true); }",
        "T id<T>(T x) => x; void main() { id<int,bool>(1); }",
        "T bad<T>(T x) => x ?? 1; void main() {}",
        "T bad<T>(T x) { int T = 1; T y = x; return y; } void main() {}",
    ] {
        assert!(analyze(source).is_err(), "{source}");
    }
}
/// Campos finais do enum recebem escalares constantes; getters podem usar this implícito.
#[test]
fn enhanced_enum_getters_and_interfaces() {
    let result=analyze("abstract class I { String describe(); } enum E implements I { a('A',1), b('B',2); final String text; final int code; const E(this.text,this.code); String get label => text + '!'; String describe() => label; int twice() => code + code; int call() => twice(); } void main() { I i = E.a; print(i.describe()); print(E.b.label); }").unwrap();
    assert!(!result.implicit_members.is_empty());
    assert!(!result.getter_accesses.is_empty());
    analyze("enum E { a; int get name => 1; } void main() { int n = E.a.name; print(n); }")
        .unwrap();
    for source in [
        "enum E { a(true); final int code; const E(this.code); } void main() {}",
        "int f() => 1; enum E { a(f()); final int code; const E(this.code); } void main() {}",
        "enum E { a(1); final int code; const E(this.code); } void main() { E.a.code = 2; }",
        "class C { int get x => 1; } void main() { print(C().x()); }",
        "abstract class I { int get x; } class C implements I { int x() => 1; } void main() {}",
    ] {
        assert!(analyze(source).is_err(), "{source}");
    }
}
/// Guardas nunca contam como cobertura e casos recebem escopos independentes.
#[test]
fn switch_coverage_and_pattern_bindings() {
    for source in [
        "enum E { a,b } int f(E e) => switch(e) { E.a => 1, E.b => 2 }; void main() { print(f(E.a)); }",
        "int f(bool? b) => switch(b) { true => 1, false => 2, null => 3 }; void main() {}",
        "int f(bool b) { switch(b) { case true: return 1; case false: return 2; } } void main() {}",
        "int f(int n) => switch(n) { int v when v > 2 => v, _ => 0 }; void main() {}",
        "void main() { switch(1) { case 1: break; } }",
    ] {
        analyze(source).unwrap_or_else(|e| panic!("{source}\n{e}"));
    }
    for source in [
        "enum E { a,b } int f(E e) => switch(e) { E.a when true => 1, E.b => 2 }; void main() {}",
        "int f(bool? b) => switch(b) { true => 1, false => 2 }; void main() {}",
        "void f(bool b) { switch(b) { case true: print(1); } } void main() {}",
        "void main() { switch(1) { case int x: print(x); } print(x); }",
        "void main() { switch(1) { case 1: continue; } }",
        "void main() { switch(1) { case 1 when 2: print(1); } }",
        "void main() { int? x = 1; var clear = () { x = null; return true; }; if (x != null) { print(switch(1) { 1 when clear() => x + 1, _ => 0 }); } }",
    ] {
        assert!(analyze(source).is_err(), "{source}");
    }
}
/// Fixtures diferenciais compartilhados permanecem aceitos pelo contrato semântico.
#[test]
fn differential_fixtures_are_accepted() {
    analyze(include_str!(
        "../../../tests/conformance/cases/generics_constants.dart"
    ))
    .unwrap();
    analyze(include_str!(
        "../../../tests/conformance/cases/enhanced_enums_switch.dart"
    ))
    .unwrap();
}
