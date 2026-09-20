//! Escopos de this implícito e inicialização definida em construtores generativos.
/// Analisa fontes pelo frontend real e preserva metadados de resolução de membros.
fn check(source: &str) -> Result<dartforge_syntax::Resolution, String> {
    let tokens = dartforge_lexer::lex(source).map_err(|e| e.message)?;
    let program = dartforge_parser::parse(&tokens, source.len()).map_err(|e| e.message)?;
    dartforge_semantic::analyze(&program).map_err(|e| e.message)
}
/// Formais inicializam campos; no corpo o nome resolve campo, enquanto parâmetros comuns ocultam.
#[test]
fn initializing_formals_and_ordinary_parameter_scopes() {
    for source in [
        "class C { final int x; C(this.x); int value()=>x; } void main(){print(C(3).value());}",
        "class C { int x; C(this.x){x+=1;} } void main(){print(C(2).x);}",
        "class C { int x=1; C(int x){x+=1; this.x=x;} int f(int x){return x+this.x;} } void main(){print(C(2).f(4));}",
        "class C { int? x; C(); } void main(){print(C().x);}",
        "class C { final int? x; C(this.x); } void main(){print(C(null).x);}",
        "int init()=>1; class C { int x=init(); C(this.x); } void main(){print(C(2).x);}",
        "class C { int x=1; C(){var f=(){return 2;}; x=f(); return;} } void main(){print(C().x);}",
        "class A {A();} class B extends A {int x;B(this.x);} void main(){print(B(2).x);}",
    ] {
        check(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
    let resolution =
        check("class C{int x;C(this.x){x+=1;}int f()=>x;} void main(){print(C(1).f());}").unwrap();
    assert!(!resolution.implicit_members.is_empty());
}
/// Inicialização, aridade, tipos e retorno são verificados antes de executar qualquer corpo.
#[test]
fn constructor_rejects_uninitialized_or_incompatible_fields() {
    for source in [
        "class C{int x;C();} void main(){}",
        "class C{final int? x;C();} void main(){}",
        "class C{final int x=1;C(this.x);} void main(){}",
        "class C{int x;C(this.x,this.x);} void main(){}",
        "class C{int x;C(int x){this.x=x;}} void main(){}",
        "class A{int x=1;} class C extends A{C(this.x);} void main(){}",
        "class C{int x;C(this.x);} void main(){C(true);}",
        "class C{int x;C(this.x);} void main(){C();}",
        "class A{int x;A(this.x);} class B extends A{} void main(){}",
        "class C{C(){return 1;}} void main(){}",
        "void f(){} class C{C(){return f();}} void main(){}",
        "class C{C(){break;}} void main(){}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}
/// O local mais próximo vence o membro inclusive antes da declaração, sem escapar para globais.
#[test]
fn implicit_member_precedence_and_temporal_shadowing() {
    check("int f()=>99; class C{int x=1;int f()=>x;int g()=>f();int h(int x){return x;}int k(){int x=2;{int x=3;print(x);}return x;}} void main(){print(C().g());}").unwrap();
    for source in [
        "class C{int x=1;int f(){print(x);int x=2;return x;}} void main(){}",
        "class C{int x=1;int f(){int x=x;return x;}} void main(){}",
        "int f()=>99;class C{int f()=>1;int g(){print(f());int f=2;return f;}} void main(){}",
        "class C{int x=1;C(this.x){print(x);int x=2;}} void main(){}",
        "class C{int x=1;C(int x){int y=x;int x=2;}} void main(){}",
        "int f()=>this.x;void main(){}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}
