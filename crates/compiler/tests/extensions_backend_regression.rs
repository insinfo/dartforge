//! Regressões de execução para o contrato entre resolução, otimização e codegen.
use dartforge_compiler::{Optimization, compile_with_optimization};

/// Compila e executa um módulo em Node; testes dependentes são opt-in explícitos.
fn execute(source: &str, optimization: Optimization) -> std::process::Output {
    let javascript = compile_with_optimization(source, optimization).unwrap();
    std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &javascript])
        .output()
        .expect("Node.js necessário")
}

/// Promover um operando de ?? não pode perder o alvo estático da extension.
#[test]
#[ignore = "requer Node.js no PATH"]
fn selected_extension_calls_keep_resolution_after_constant_folding() {
    let source = r#"
        extension Numbers on int {
            int twice() { print('extension'); return this * 2; }
        }
        int fallback() { print('fallback'); return 99; }
        void main() {
            var x = 3;
            int? known = 4;
            print(known ?? fallback());
            print(null ?? x.twice());
            print(null ?? (null ?? x.twice()));
        }
    "#;
    for optimization in [Optimization::None, Optimization::Constants] {
        let run = execute(source, optimization);
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(run.stdout, b"4\nextension\n6\nextension\n6\n");
    }
}

/// Uma asserção que falha não executa o método nem duplica o receptor com efeito.
#[test]
#[ignore = "requer Node.js no PATH"]
fn failing_null_assert_stops_extension_dispatch_in_both_modes() {
    let source = r#"
        extension Numbers on int {
            int value() { print('unreachable'); return this; }
        }
        int? missing() { print('effect'); return null; }
        void main() { print(missing()!.value()); }
    "#;
    for optimization in [Optimization::None, Optimization::Constants] {
        let run = execute(source, optimization);
        assert!(!run.status.success());
        assert_eq!(run.stdout, b"effect\n");
        assert!(
            String::from_utf8_lossy(&run.stderr)
                .contains("TypeError: Null check operator used on a null value")
        );
    }
}
