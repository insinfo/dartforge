//! Regressões de execução para inicialização, herança e tipos nominais.
use dartforge_compiler::compile;

/// Executa o módulo gerado e devolve stdout, exigindo Node.js como nos testes do backend.
fn execute(source: &str) -> String {
    let js = compile(source).expect("fonte válida no subconjunto");
    let result = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &js])
        .output()
        .expect("Node.js necessário");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

/// Preserva a ordem Dart: campos derivados antes dos ancestrais, em ordem textual.
#[test]
#[ignore = "requer Node.js no PATH"]
fn three_level_field_initializer_order() {
    let source = "
int mark(String label) { print(label); return 1; }
class Leaf extends Middle { int d=mark('leaf1'); int e=mark('leaf2'); }
class Base { int a=mark('base1'); int b=mark('base2'); }
class Middle extends Base { int c=mark('middle'); }
void main() { var item=Leaf(); print(item.a+item.b+item.c+item.d+item.e); }
";
    assert_eq!(execute(source), "leaf1\nleaf2\nmiddle\nbase1\nbase2\n5\n");
}

/// Mantém despacho dinâmico e promoção local de uma referência nominal anulável.
#[test]
#[ignore = "requer Node.js no PATH"]
fn nullable_base_reference_preserves_dispatch_and_field_assignment() {
    let source = "
class Base { int value=1; int read() { return this.value; } }
class Child extends Base { int read() { return this.value+10; } }
void main() { Base? item=Child(); if(item!=null) { item.value=5; print(item.read()); } print((item ?? Base()).read()); }
";
    assert_eq!(execute(source), "15\n15\n");
}
