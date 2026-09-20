//! Contratos nominais, classes abstratas e enums simples do subconjunto Dart 3.6.2.
/// Analisa o mesmo AST usado pelos backends e retorna apenas o diagnóstico textual.
fn check(source: &str) -> Result<(), String> {
    let tokens = dartforge_lexer::lex(source).map_err(|e| e.message)?;
    let program = dartforge_parser::parse(&tokens, source.len()).map_err(|e| e.message)?;
    dartforge_semantic::validate(&program).map_err(|e| e.message)
}
/// Contratos herdados transitivamente permitem parâmetros e retornos polimórficos.
#[test]
fn transitive_interfaces_and_abstract_redeclaration() {
    check("abstract class I { int f(); } abstract class J implements I {} class A implements J { int f() => 2; } abstract class B extends A { int f(); } class C extends B {} int use(I x) => x.f(); void main() { print(use(C())); }").unwrap();
    check("class X {} class Y extends X {} class A { X f() => X(); } abstract class B extends A { Y f(); } class C extends B { Y f() => Y(); } void main() {}").unwrap();
    check(
        "class A { int f() => 1; } class B extends A { int f(); } void main() { print(B().f()); }",
    )
    .unwrap();
}
/// implements exige corpo próprio ou herdado por extends; nunca importa implementação.
#[test]
fn missing_contracts_and_cycles_are_rejected() {
    for source in [
        "class I { int f() => 1; } class A implements I {} void main() {}",
        "abstract class I { int f(); } abstract class J implements I {} class A extends J {} void main() {}",
        "abstract class A implements B {} abstract class B implements A {} void main() {}",
        "abstract class A extends B {} abstract class B implements A {} void main() {}",
        "abstract class I { int f(); } class A implements I { int f(int x) => x; } void main() {}",
        "abstract class A { int f(); } void main() { var a = A(); }",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}
/// Retorno é covariante, parâmetros contravariantes e void admite valor descartado.
#[test]
fn variance_is_checked_for_every_interface() {
    check("class X {} class Y extends X {} abstract class I { X f(Y x); } class C implements I { Y f(X x) => Y(); } void main() { I c = C(); c.f(Y()); }").unwrap();
    check("abstract class I { void f(); } class C implements I { int f() => 1; } void main() { I c = C(); c.f(); }").unwrap();
    assert!(check("class X {} class Y extends X {} abstract class I { X f(X x); } class C implements I { X f(Y x) => X(); } void main() {}").is_err());
    assert!(check("abstract class I { int f(); } abstract class J { bool f(); } class C implements I, J { int f() => 1; } void main() {}").is_err());
    check("abstract class I { int f(); } abstract class J { int f(); } class C implements I, J { int f() => 1; } void main() {}").unwrap();
}
/// Getters/setters implícitos de campos de interface exigem representação futura explícita.
#[test]
fn interfaces_with_fields_are_explicitly_unsupported() {
    for source in [
        "class I { int x = 1; } class C implements I { int x = 1; } void main() {}",
        "class Base { int x = 1; } class I extends Base {} abstract class C implements I {} void main() {}",
    ] {
        assert!(check(source).unwrap_err().contains("getter/setter"));
    }
}
/// Enums têm valores nominais e somente propriedades name/index de leitura.
#[test]
fn enums_are_nominal_and_read_only() {
    check("enum E { first, name } E same(E x) => x; void main() { E e = same(E.first); print(e.name); print(e.index); print(E.name == E.first); }").unwrap();
    for source in [
        "enum E { a } void main() { var e = E(); }",
        "enum E { a } class C extends E {} void main() {}",
        "enum E { a } class C implements E {} void main() {}",
        "enum E { a } enum F { a } void main() { E x = F.a; }",
        "enum E { a } void main() { E.a.index = 2; }",
        "enum E { a } void main() { E.a.name = 'x'; }",
        "enum E { a } void main() { print(E.a); }",
        "enum E { a } void main() { var E = 1; print(E.a); }",
        "enum E { index } void main() {}",
        "enum E { values } void main() {}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}

/// O modificador interface restringe extends pela biblioteca de origem, não implements.
#[test]
fn interface_class_respects_library_boundary() {
    let source =
        "interface class I { int f() => 1; } class C extends I {} void main() { print(C().f()); }";
    check(source).unwrap();
    let tokens = dartforge_lexer::lex(source).unwrap();
    let mut program = dartforge_parser::parse(&tokens, source.len()).unwrap();
    program.classes[1].library_id = 1;
    assert!(
        dartforge_semantic::validate(&program)
            .unwrap_err()
            .message
            .contains("another library")
    );
    let source = "abstract interface class I { int f(); } class C implements I { int f() => 1; } void main() {}";
    let tokens = dartforge_lexer::lex(source).unwrap();
    let mut program = dartforge_parser::parse(&tokens, source.len()).unwrap();
    program.classes[1].library_id = 1;
    dartforge_semantic::validate(&program).unwrap();
    assert!(check("abstract interface class I { int f(); } void main() { I(); }").is_err());
}
