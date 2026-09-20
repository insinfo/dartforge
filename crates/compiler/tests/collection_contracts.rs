//! Contratos de falhas e identidade adicionais do subconjunto JS.
use dartforge_compiler::{compile, compile_llvm};

/// O backend nativo rejeita recursos sem lowering inclusive dentro de código morto.
#[test]
fn native_rejects_unlowered_collections_and_functions() {
    for source in [
        "void main() { if (false) { var xs = [1]; } }",
        "void main() { var f = () => 1; }",
        "int f()=>1; void main(){ var g=f; }",
        "void main() { if (false) { print(3 % 2); } }",
    ] {
        assert!(compile(source).is_ok(), "{source}");
        assert!(compile_llvm(source).is_err(), "{source}");
    }
}

/// Erros de índice e modificação estrutural não devem retornar undefined silenciosamente.
#[test]
#[ignore = "requer Node.js no PATH"]
fn runtime_bounds_and_iteration_errors() {
    for source in [
        "void main(){var xs=[1]; print(xs[1]);}",
        "void main(){var xs=[1]; xs[-1]=2;}",
        "void main(){var xs=[1]; xs.forEach((x){xs.add(x);});}",
        "void main(){print(1 % 0);}",
    ] {
        let js = compile(source).unwrap();
        let out = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &js])
            .output()
            .unwrap();
        assert!(!out.status.success(), "{source}");
        assert!(!out.stderr.is_empty());
    }
}

/// Referências à entrada e homônimos locais conservam resolução e identidade.
#[test]
#[ignore = "requer Node.js no PATH"]
fn main_tearoff_local_shadow_and_euclidean_modulo() {
    for (source, expected) in [
        (
            "void main(){int Function(int) f=(int x){var x=2;return x;};print(f(1));}",
            "2\n",
        ),
        ("void main(){var f=main;print(f==main);}", "true\n"),
        ("void main(){var main=()=>42;print(main());}", "42\n"),
        (
            "void main(){print(-5 % 3);print(5 % -3);print(-5 % -3);}",
            "1\n2\n1\n",
        ),
    ] {
        let js = compile(source).unwrap();
        let out = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &js])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            String::from_utf8(out.stdout).unwrap().replace("\r\n", "\n"),
            expected
        );
    }
}
