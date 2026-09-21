//! Classes avançadas: construtores nomeados, listas de inicialização, `super`
//! explícito, construtores `const` canônicos, membros estáticos, variáveis de
//! topo, getters e fábricas com redirecionamento.
//!
//! O oráculo é o **Dart SDK 3.6.2 instalado nesta máquina**, conferido também
//! com o **Dart SDK 3.13.4**. Os programas abaixo foram executados com
//! `dart run` nos dois SDKs antes de virarem testes; os testes marcados com
//! `#[ignore]` conferem byte a byte que o JavaScript emitido imprime o mesmo
//! texto no Node.
use dartforge_compiler::{CompileOptions, Optimization, compile, compile_path_with_options};
use dartforge_diagnostics::Span;

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
    assert_eq!(
        source.rfind(needle),
        Some(start),
        "trecho ambíguo: {needle}"
    );
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

/// Caminho do módulo de conformidade compartilhado por Dart e JavaScript.
fn modulo() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/modules/classes_adv/main.dart")
}

/// O módulo de conformidade compila em todos os modos de otimização.
#[test]
fn classes_adv_compile_in_all_modes() {
    for optimization in [Optimization::None, Optimization::Constants] {
        for merge_identical_functions in [false, true] {
            compile_path_with_options(
                &modulo(),
                CompileOptions {
                    optimization,
                    merge_identical_functions,
                    tree_shaking: false,
                },
            )
            .unwrap();
        }
    }
}

/// A execução JS do módulo imprime exatamente o mesmo texto que o Dart 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn classes_adv_match_dart_in_all_modes() {
    let expected =
        include_str!("../../../tests/native/modules/classes_adv/main.stdout").replace("\r\n", "\n");
    for optimization in [Optimization::None, Optimization::Constants] {
        for merge_identical_functions in [false, true] {
            let js = compile_path_with_options(
                &modulo(),
                CompileOptions {
                    optimization,
                    merge_identical_functions,
                    tree_shaking: false,
                },
            )
            .unwrap();
            assert_eq!(
                node(&js),
                expected,
                "{optimization:?} merge={merge_identical_functions}"
            );
        }
    }
}

/// Construtores nomeados alocam pela entrada própria de cada nome.
#[test]
fn named_constructors_allocate_each_entry() {
    let source = "class C{int x;C(this.x);C.named(this.x);C.zero():x=0;}\
        void main(){print(C(1).x);print(C.named(2).x);print(C.zero().x);}";
    let js = javascript(source);
    assert!(js.contains("$dartforgeNew0$df_named("), "{js}");
    assert!(js.contains("$dartforgeNew0$df_zero("), "{js}");
}

/// A execução dos nomeados coincide com o Dart 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn named_constructors_match_dart() {
    let source = "class C{int x;C(this.x);C.named(this.x);C.zero():x=0;}\
        void main(){print(C(1).x);print(C.named(2).x);print(C.zero().x);}";
    assert_eq!(node(&javascript(source)), "1\n2\n0\n");
}

/// Listas de inicialização e `super` explícito com argumentos.
#[test]
fn initializer_lists_and_explicit_super() {
    let source = "class Base{int v;Base(this.v);Base.named(this.v);}\
        class D extends Base{int w;D(int v):w=v*10,super(v+1);}\
        class E extends Base{E(int v):super.named(v*2);}\
        void main(){var d=D(5);print(d.w);print(d.v);print(E(6).v);}";
    let js = javascript(source);
    assert!(js.contains("$dartforgeInit"), "{js}");
}

/// A execução da cadeia coincide com o Dart 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn initializer_lists_match_dart() {
    let source = "class C{int x;int y;C(int a,int b):x=a*2,y=b+1;}\
        class Base{int v;Base(this.v);Base.named(this.v);}\
        class D extends Base{int w;D(int v):w=v*10,super(v+1);}\
        class E extends Base{E(int v):super.named(v*2);}\
        void main(){var c=C(3,4);print(c.x);print(c.y);\
        var d=D(5);print(d.w);print(d.v);print(E(6).v);}";
    assert_eq!(node(&javascript(source)), "6\n5\n50\n6\n12\n");
}

/// `assert` na lista de inicialização é rejeitado com diagnóstico explícito.
#[test]
fn assert_in_initializer_list_is_rejected() {
    let source = "class C{int x;C():assert(x>0),x=1;void main(){}}";
    rejeita(
        source,
        "assert in an initializer list is not supported yet",
        trecho(source, "assert"),
    );
}

/// Construtores `const` canônicos: `identical` entre iguais é verdadeiro.
#[test]
fn const_constructors_are_canonical() {
    let source = "class C{final int x;const C(this.x);}\
        void main(){print(identical(const C(1),const C(1)));print(const C(2).x);}";
    let js = javascript(source);
    assert!(js.contains("$dartforgeConstInstance("), "{js}");
}

/// A execução do `const` canônico coincide com o Dart 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn const_constructors_match_dart() {
    let source = "class C{final int x;const C(this.x);}\
        void main(){print(identical(const C(1),const C(1)));print(const C(2).x);}";
    assert_eq!(node(&javascript(source)), "true\n2\n");
}

/// Membros estáticos resolvem pela classe declaradora, sem herança.
#[test]
fn static_members_resolve_on_declaring_class() {
    let source = "class C{static int x=1;static int f()=>C.x+1;}\
        void main(){print(C.x);print(C.f());}";
    let js = javascript(source);
    assert!(js.contains("static "), "{js}");
}

/// A execução dos estáticos coincide com o Dart 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn static_members_match_dart() {
    let source = "class C{static int x=1;static int f()=>C.x+1;}\
        void main(){print(C.x);print(C.f());}";
    assert_eq!(node(&javascript(source)), "1\n2\n");
}

/// Estático herdado pelo nome da subclasse é rejeitado com a origem.
#[test]
fn inherited_statics_are_rejected() {
    let source = "class C{static int x=1;}class Sub extends C{}\
        void main(){print(Sub.x);}";
    rejeita(
        source,
        "Static member 'x' belongs to 'C' and is not inherited",
        trecho(source, "Sub.x"),
    );
}

/// Variáveis de topo tipadas leem e escrevem como globais do módulo.
#[test]
fn top_level_variables_read_and_write() {
    let source = "int x=1;final int y=2;int z=3;\
        void main(){print(x);print(y);print(z);x=10;z=30;print(x);print(z);}";
    let js = javascript(source);
    assert!(js.contains("let $df_x"), "{js}");
}

/// A execução das variáveis de topo coincide com o Dart 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn top_level_variables_match_dart() {
    let source = "int x=1;final int y=2;int z=3;\
        void main(){print(x);print(y);print(z);x=10;z=30;print(x);print(z);}";
    assert_eq!(node(&javascript(source)), "1\n2\n3\n10\n30\n");
}

/// Getters de instância leem sem chamada.
#[test]
fn instance_getters_read_without_call() {
    let source = "class C{int _x=3;int get value=>_x;}\
        void main(){print(C().value);}";
    let js = javascript(source);
    assert!(js.contains("get "), "{js}");
}

/// A execução do getter coincide com o Dart 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn instance_getters_match_dart() {
    let source = "class C{int _x=3;int get value=>_x;}\
        void main(){print(C().value);}";
    assert_eq!(node(&javascript(source)), "3\n");
}

/// Setters de instância emitem `set` no JavaScript.
#[test]
fn instance_setters_emit_setter() {
    let source = "class C{int _x=0;void set value(int v){_x=v;}}void main(){}";
    let js = javascript(source);
    assert!(js.contains("set "), "{js}");
}

/// Getters estáticos são rejeitados com diagnóstico explícito.
#[test]
fn static_getters_are_rejected() {
    let source = "class C{static int _x=4;static int get value=>C._x;}void main(){}";
    rejeita(
        source,
        "static getters are not supported yet",
        trecho(source, "get"),
    );
}

/// Setters estáticos são rejeitados com diagnóstico explícito.
#[test]
fn static_setters_are_rejected() {
    let source = "class C{static void set value(int v){}}void main(){}";
    rejeita(
        source,
        "static setters are not supported yet",
        trecho(source, "set"),
    );
}

/// Escritas em campos estáticos são rejeitadas com diagnóstico explícito.
#[test]
fn static_field_assignments_are_rejected() {
    let source = "class C{static int x=1;}void main(){C.x=2;}";
    rejeita(
        source,
        "static field assignments are not supported yet",
        trecho(source, "C.x"),
    );
}

/// Getters de topo são rejeitados com diagnóstico explícito.
#[test]
fn top_level_getters_are_rejected() {
    let source = "int get x=>1;void main(){print(x);}";
    rejeita(
        source,
        "top-level getters are not supported yet",
        trecho(source, "get"),
    );
}

/// Setters de topo são rejeitados com diagnóstico explícito.
#[test]
fn top_level_setters_are_rejected() {
    let source = "void set x(int v){}void main(){}";
    rejeita(
        source,
        "top-level setters are not supported yet",
        trecho(source, "set"),
    );
}

/// Declarações `operator` (exceto `==`) são rejeitadas com diagnóstico explícito.
#[test]
fn operators_are_rejected() {
    let source = "class C{int operator +(C other)=>42;}void main(){}";
    rejeita(
        source,
        "only 'operator ==' is supported; other operator declarations are not supported yet",
        trecho(source, "+"),
    );
}

/// Chamadas `super.metodo()` são rejeitadas com diagnóstico explícito.
#[test]
fn super_method_calls_are_rejected() {
    let source = "class B{int f()=>1;}class D extends B{int g()=>super.f();}void main(){}";
    rejeita(
        source,
        "super method calls are not supported yet",
        trecho(source, "super"),
    );
}

/// Fábricas com redirecionamento repassam os argumentos ao alvo.
#[test]
fn factory_redirects_forward_to_target() {
    let source = "class C{int x;C(this.x);factory C.make(int v)=D;factory C.makeNamed(int v)=D.named;}\
        class D extends C{D(int v):super(v);D.named(int v):super(v);}\
        void main(){print(C.make(5).x);print(C.makeNamed(6).x);}";
    let js = javascript(source);
    assert!(js.contains("$dartforgeFactory"), "{js}");
}

/// A execução do redirecionamento coincide com o Dart 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn factory_redirects_match_dart() {
    let source = "class C{int x;C(this.x);factory C.make(int v)=D;factory C.makeNamed(int v)=D.named;}\
        class D extends C{D(int v):super(v);D.named(int v):super(v);}\
        void main(){print(C.make(5).x);print(C.makeNamed(6).x);}";
    assert_eq!(node(&javascript(source)), "5\n6\n");
}

/// Super-parâmetros são rejeitados com diagnóstico explícito.
#[test]
fn super_parameters_are_rejected() {
    let source = "class B{int x;B(this.x);}class D extends B{D(super.x);}void main(){}";
    rejeita(
        source,
        "super parameters are not supported yet",
        trecho(source, "super"),
    );
}

/// `covariant` é rejeitado com diagnóstico explícito.
#[test]
fn covariant_is_rejected() {
    let source = "class A{void eat(covariant int food){}}void main(){}";
    rejeita(
        source,
        "covariant parameters are not supported yet",
        trecho(source, "covariant"),
    );
}

/// Construtores de redirecionamento são rejeitados com diagnóstico explícito.
#[test]
fn redirecting_constructors_are_rejected() {
    let source = "class C{int x=0;C.zero():this(0);}void main(){}";
    rejeita(
        source,
        "redirecting constructors are not supported yet",
        trecho(source, "this"),
    );
}

/// LLVM rejeita construtores nomeados até o lowering existir.
#[test]
fn llvm_rejects_named_constructors() {
    let source = "class C{int x;C(this.x);C.named(this.x);}void main(){print(C.named(1).x);}";
    let dir = std::env::temp_dir().join("dartforge-classes-adv");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("named.dart");
    std::fs::write(&path, source).unwrap();
    let error = dartforge_compiler::compile_path_llvm(&path).expect_err("LLVM deveria rejeitar");
    assert!(
        error.message.contains("construtores nomeados"),
        "mensagem inesperada: {}",
        error.message
    );
}

/// LLVM rejeita membros estáticos até o lowering existir.
#[test]
fn llvm_rejects_static_members() {
    let source = "class C{static int x=1;}void main(){print(C.x);}";
    let dir = std::env::temp_dir().join("dartforge-classes-adv");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("static.dart");
    std::fs::write(&path, source).unwrap();
    let error = dartforge_compiler::compile_path_llvm(&path).expect_err("LLVM deveria rejeitar");
    assert!(
        error.message.contains("membros est"),
        "mensagem inesperada: {}",
        error.message
    );
}
