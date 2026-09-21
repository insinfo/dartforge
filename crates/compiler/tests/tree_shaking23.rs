//! Alcance global opcional sem enfraquecer análise estática ou identidades.
use dartforge_compiler::{CompileOptions, Optimization, compile_with_options};

/// Combina o novo passe com folding e fusão para detectar dependências entre passes.
fn modes() -> impl Iterator<Item = CompileOptions> {
    [Optimization::None, Optimization::Constants]
        .into_iter()
        .flat_map(|optimization| {
            [false, true]
                .into_iter()
                .map(move |merge_identical_functions| CompileOptions {
                    optimization,
                    merge_identical_functions,
                    tree_shaking: true,
                })
        })
}

/// O modo padrão não paga o passe, enquanto o opt-in remove ilhas recursivas mortas.
#[test]
fn optional_and_transitive() {
    let source = "int unusedA()=>unusedB();int unusedB()=>unusedA();class UnusedClass{} int kept()=>9;void main(){print(kept());}";
    let full = compile_with_options(source, CompileOptions::default()).unwrap();
    for options in modes() {
        let trimmed = compile_with_options(source, options).unwrap();
        assert!(trimmed.len() < full.len());
        assert!(!trimmed.contains("unusedA"));
        assert!(!trimmed.contains("unusedB"));
        assert!(trimmed.contains("kept"));
    }
}

/// Erros em declarações não alcançadas continuam sendo erros de compilação.
#[test]
fn dead_code_is_checked_before_pruning() {
    for options in modes() {
        assert!(compile_with_options("int bad()=>missing();void main(){}", options).is_err());
        assert!(compile_with_options("int bad()=>true;void main(){}", options).is_err());
    }
}

/// Executa closures, despacho virtual, campos com efeitos e tipos após a poda.
#[test]
#[ignore = "requer Node.js no PATH"]
fn preserves_execution_and_function_identity() {
    let source = r#"
int effect(){print('field');return 7;}
int a()=>1;
int b()=>1;
int dead()=>999;
class Base{int read()=>2;}
class Unused{}
class Child extends Base{int value=effect();int read()=>value;}
bool accepts(Object value)=>value is Child;
void main(){Base object=Child();print(object.read());print(accepts(object));var fs=<int Function()>[a,b];print(fs[0]==fs[1]);print(fs[0]());}
"#;
    for options in modes() {
        let js = compile_with_options(source, options).unwrap();
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
            "field\n7\ntrue\nfalse\n1\n"
        );
    }
}

/// Expansão de macros e linking precedem a análise de alcance.
#[test]
#[ignore = "requer Node.js no PATH"]
fn factories_maps_and_fields_remain_live() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/modules/maps21/main.dart");
    for options in modes() {
        let js = dartforge_compiler::compile_path_with_options(&path, options).unwrap();
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
            include_str!("../../../tests/conformance/modules/maps21/main.stdout")
                .replace("\r\n", "\n")
        );
    }
}

/// Uma classe com macro não usada pode desaparecer, sem remover membros da classe viva.
#[test]
#[ignore = "requer Node.js no PATH"]
fn macros_expand_before_tree_shaking() {
    for options in modes() {
        let js = compile_with_options("@JsonCodable() class Used{final int x;}@JsonCodable() class Unused{final int x;}void main(){print(Used.fromJson({'x':7}).x);}", options).unwrap();
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
            "7\n"
        );
    }
}
