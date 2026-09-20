//! Construtores e membros implícitos: programa original conferido no Dart VM 3.6.2.
use dartforge_diagnostics::Diagnostic;
use std::path::PathBuf;

const SOURCE: &str = r"
int mark(int value) { print(value); return value; }
String text(String value) { print(value); return value; }
class Base {
  int baseField = mark(30);
  Base() { print(baseField); }
}
class Usuario extends Base {
  String username;
  String email;
  int count = mark(20);
  int? optional;
  Usuario(this.username, this.email) {
    username = username + '!';
    count += 1;
    print(count);
  }
  String describe(String username) { return username + this.username; }
  int shadow(int count) {
    count += 2;
    { int count = 100; print(count); }
    return count;
  }
  int get value => count;
  void increase() { count = count + 1; }
  int run() { increase(); return value; }
}
class Replaced {
  int x = mark(99);
  Replaced(this.x);
}
void main() {
  var user = Usuario(text('a'), text('b'));
  print(user.username);
  print(user.email);
  print(user.describe('local:'));
  print(user.shadow(5));
  print(user.run());
  print(user.optional);
  print(Replaced(2).x);
}
";
const EXPECTED: &str = "a\nb\n20\n30\n30\n21\na!\nb\nlocal:a!\n100\n7\n22\nnull\n99\n2\n";

/// Usa frontend real e metadados semânticos, inclusive distinção entre locais e campos.
fn emit(source: &str) -> Result<String, Diagnostic> {
    let tokens = dartforge_lexer::lex(source)?;
    let program = dartforge_parser::parse(&tokens, source.len())?;
    let resolution = dartforge_semantic::analyze(&program)?;
    dartforge_llvm::emit(&dartforge_hir::lower_resolved(program, resolution))
}

/// Inicializadores formais são parâmetros do factory, enquanto corpos recebem this separado.
#[test]
fn constructors_emit_arguments_bodies_and_implicit_members() {
    let ir = emit(SOURCE).unwrap();
    assert!(ir.contains("define i64 @df_new_1(i64 %a0, i64 %a1)"));
    assert!(ir.contains("call void @df_ctorbody_0(i64 %this)"));
    assert!(ir.contains("call void @df_ctorbody_1(i64 %this, i64 %a0, i64 %a1)"));
}

/// Recursos ainda sem lowering não podem desaparecer dentro dos argumentos de construtor.
#[test]
fn unsupported_constructor_argument_is_rejected_even_when_unused() {
    let source =
        "class Box { List<int> value; Box(this.value); } void main(){if(false){Box(<int>[1]);}}";
    assert!(emit(source).is_err());
}

/// Diretório temporário exclusivo para executáveis de regressão.
struct Fixture(PathBuf);
impl Fixture {
    /// Reserva um diretório sem depender de caminhos estáticos compartilhados.
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dartforge-constructors-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    /// Limpa apenas os artefatos criados pelo teste.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// LLVM O0/O2 deve preservar ordem, sombreamento e argumentos gerenciados sob GC forçado.
#[test]
#[ignore = "requer Clang e rustc/linker nativos ou DARTFORGE_CLANG/DARTFORGE_RUSTC"]
fn native_constructors_match_dart_3_6_2_with_gc_stress() {
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
            String::from_utf8(result.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            EXPECTED
        );
    }
}
