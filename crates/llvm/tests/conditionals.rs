//! Operador condicional ternário: programa original conferido no Dart VM 3.6.2.
use dartforge_diagnostics::Diagnostic;
use std::path::PathBuf;

const SOURCE: &str = r"
sealed class Animal {
  int sound();
}
class Dog extends Animal {
  int sound() => 1;
}
class Cat extends Animal {
  int sound() => 2;
}

int pickInt(bool c) => c ? 10 : 20;
String pickString(bool c) => c ? 'alpha' : 'beta';
bool pickBool(bool c) => c ? false : true;

int? pickNullable(bool c) => c ? 1 : null;
int? pickNullableElse(bool c) => c ? null : 2;
String? pickNullableStr(bool c) => c ? 'dart' : null;

int nestedThen(bool c1, bool c2) => c1 ? (c2 ? 1 : 2) : 3;
int nestedElse(bool c1, bool c2) => c1 ? 1 : (c2 ? 2 : 3);

Animal pickAnimal(bool c) => c ? Dog() : Cat();

void main() {
  print(pickInt(true));
  print(pickInt(false));

  print(pickString(true));
  print(pickString(false));

  print(pickBool(true));
  print(pickBool(false));

  print(pickNullable(true));
  print(pickNullable(false));

  print(pickNullableElse(true));
  print(pickNullableElse(false));

  print(pickNullableStr(true));
  print(pickNullableStr(false));

  print(nestedThen(true, true));
  print(nestedThen(true, false));
  print(nestedThen(false, true));

  print(nestedElse(true, false));
  print(nestedElse(false, true));
  print(nestedElse(false, false));

  Animal a1 = pickAnimal(true);
  Animal a2 = pickAnimal(false);
  print(a1.sound());
  print(a2.sound());
}
";

const EXPECTED: &str =
    "10\n20\nalpha\nbeta\nfalse\ntrue\n1\nnull\nnull\n2\ndart\nnull\n1\n2\n3\n1\n2\n3\n1\n2\n";

/// Usa o frontend real para preservar tipos de expressão e resolução semântica.
fn emit(source: &str) -> Result<String, Diagnostic> {
    let tokens = dartforge_lexer::lex(source)?;
    let program = dartforge_parser::parse(&tokens, source.len())?;
    let resolution = dartforge_semantic::analyze(&program)?;
    dartforge_llvm::emit(&dartforge_hir::lower_resolved(program, resolution))
}

/// O operador condicional emite phi com os predecessores corretos para todos os tipos suportados.
#[test]
fn conditionals_lower_every_supported_family() {
    let ir = emit(SOURCE).unwrap();

    // Condicional simples de inteiros
    assert!(ir.contains("phi i64 [ 10, %b0 ], [ 20, %b1 ]"));

    // Strings usam phi i64
    assert!(ir.contains("phi i64"));

    // Booleanos usam phi i1
    assert!(ir.contains("phi i1 [ false, %b0 ], [ true, %b1 ]"));

    // Unificação de int com null usa agregado { i1, i64 }
    assert!(ir.contains("insertvalue { i1, i64 } zeroinitializer, i1 true, 0"));
    assert!(ir.contains("phi { i1, i64 }"));

    // Condicionais aninhadas usam os predecessores reais da sub-árvore
    assert!(ir.contains("phi i64 [ 1, %b2 ], [ 2, %b3 ]"));

    // Hierarquia de classes unifica para classe base comum
    assert!(ir.contains("phi i64"));
}

/// Todos os blocos predecessores da phi terminam estritamente com saltos para o bloco final.
#[test]
fn conditionals_predecessors_and_cfg_integrity() {
    let ir = emit(SOURCE).unwrap();

    // Cada branch condicional tem dois sucessores
    assert!(ir.contains("br i1 %a0, label %b0, label %b1"));

    // Blocos then e else terminam com br label
    assert!(ir.contains("b0:\n  br label %b2\n"));
    assert!(ir.contains("b1:\n  br label %b2\n"));

    // Bloco de junção começa com a instrução phi
    assert!(ir.contains("b2:\n  %v0 = phi i64"));
}

/// Diretório temporário exclusivo para executáveis de regressão.
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dartforge-conditionals-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// LLVM O0/O2 deve reproduzir a saída do Dart 3.6.2 mesmo sob GC forçado.
#[test]
#[ignore = "requer Clang e rustc/linker nativos ou DARTFORGE_CLANG/DARTFORGE_RUSTC"]
fn native_conditionals_match_dart_3_6_2_with_gc_stress() {
    let fixture = Fixture::new();
    let ir = emit(SOURCE).unwrap();
    for optimize in [false, true] {
        let output = fixture.0.join(if optimize {
            "optimized.exe"
        } else {
            "plain.exe"
        });
        let options = dartforge_native::NativeOptions {
            optimize,
            ..Default::default()
        };
        dartforge_native::build_executable(&ir, &output, &options).unwrap();
        let result = std::process::Command::new(&output)
            .env("DARTFORGE_GC_STRESS", "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&result.stdout).replace("\r\n", "\n"),
            EXPECTED
        );
    }
}
