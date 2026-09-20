//! Contratos de biblioteca, propagação de modificadores e exaustividade sealed.
/// Analisa declarações e permite atribuir bibliotecas distintas sem reescrever fontes.
fn check(source: &str, foreign: &[&str]) -> Result<(), String> {
    let tokens = dartforge_lexer::lex(source).map_err(|e| e.message)?;
    let mut ast = dartforge_parser::parse(&tokens, source.len()).map_err(|e| e.message)?;
    for class in &mut ast.classes {
        if foreign.contains(&class.name) {
            class.library_id = 1;
        }
    }
    dartforge_semantic::validate(&ast).map_err(|e| e.message)
}
/// Fronteiras de biblioteca restringem arestas; base/final propagam pelo fecho transitivo.
#[test]
fn library_boundaries_and_transitive_base_requirements() {
    for source in [
        "base class A {} base class B implements A {} void main() {}",
        "final class A {} sealed class B extends A {} base class C extends B {} void main() {}",
        "interface class A {} class B extends A {} void main() {}",
        "base class A {} enum E implements A { one } void main() {}",
        "mixin M { int get n; int value() => n; } class C implements M { int get n => 1; int value()=>2; } void main() {}",
        "mixin class M {} class C extends M {} void main() { M m=M(); }",
    ] {
        check(source, &[]).unwrap_or_else(|e| panic!("{source}: {e}"));
    }
    for source in [
        "base class A {} class B extends A {} void main() {}",
        "final class A {} class B implements A {} void main() {}",
        "base class A {} sealed class B extends A {} class C extends B {} void main() {}",
        "mixin M {} class C extends M {} void main() {}",
        "mixin M {} void main() { var m=M(); }",
        "class A {} mixin class M extends A {} void main() {}",
    ] {
        assert!(check(source, &[]).is_err(), "{source}");
    }
    for source in [
        "interface class A {} class B extends A {} void main() {}",
        "base class A {} base class B implements A {} void main() {}",
        "final class A {} base class B extends A {} void main() {}",
        "sealed class A {} class B implements A {} void main() {}",
        "base class A {} sealed class B extends A {} base class C implements B {} void main() {}",
    ] {
        assert!(check(source, &["A"]).is_err(), "{source}");
    }
    check(
        "base class A {} base class B extends A {} void main() {}",
        &["A"],
    )
    .unwrap();
    check(
        "interface class A {} class B implements A {} void main() {}",
        &["A"],
    )
    .unwrap();
}
/// Subtipos sealed são fechados; enumerar filhos de um subtipo aberto não o cobre.
#[test]
fn sealed_cones_guards_and_nullable_coverage() {
    for source in [
        "sealed class S {} class A extends S {} class B implements S {} int f(S x)=>switch(x){ A()=>1,B()=>2 }; void main() {}",
        "sealed class S {} sealed class A extends S {} class B extends A {} class C extends S {} int f(S x)=>switch(x){ B()=>1,C()=>2 }; void main() {}",
        "sealed class S {} class A extends S {} int f(S? x)=>switch(x){ A()=>1,null=>0 }; void main() {}",
        "sealed class S {} class A extends S {} int f(S x){ switch(x){ case A():return 1; } } void main() {}",
        "sealed class S {} enum E implements S { a,b } int f(S x)=>switch(x){E.a=>1,E.b=>2}; void main() {}",
        "abstract class I {} sealed class S {} class A extends S implements I {} int f(S x)=>switch(x){I()=>1}; void main() {}",
    ] {
        check(source, &[]).unwrap_or_else(|e| panic!("{source}: {e}"));
    }
    for source in [
        "sealed class S {} class A extends S {} class B extends A {} int f(S x)=>switch(x){B()=>1}; void main() {}",
        "sealed class S {} abstract class A extends S {} class B extends A {} int f(S x)=>switch(x){B()=>1}; void main() {}",
        "sealed class S {} class A extends S {} int f(S x)=>switch(x){A() when true=>1}; void main() {}",
        "sealed class S {} class A extends S {} int f(S? x)=>switch(x){A()=>1}; void main() {}",
        "sealed class S {} class A extends S {} class B extends S {} void f(S x){switch(x){case A():break;}} void main() {}",
        "sealed class S {} void main(){ var s=S(); }",
    ] {
        assert!(check(source, &[]).is_err(), "{source}");
    }
}
