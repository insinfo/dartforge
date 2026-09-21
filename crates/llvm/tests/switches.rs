//! Switch instrução e expressão: programa original conferido no Dart VM 3.6.2.
use dartforge_diagnostics::Diagnostic;
use std::path::PathBuf;

const SOURCE: &str = r"
enum Color { red, green, blue }

sealed class Shape {}
class Circle extends Shape {}
class Square extends Shape {}

int scrutinee(int value) {
  print(value);
  return value;
}

String describe(Color color) {
  return switch (color) {
    Color.red => 'vermelho',
    Color.green => 'verde',
    _ => 'outro',
  };
}

int area(Shape shape) {
  switch (shape) {
    case Circle c:
      return 3;
    case Square():
      return 4;
  }
}

int guarded(int value) {
  switch (value) {
    case int v when v > 10:
      return v * 2;
    case 10:
      return 0;
    default:
      return -1;
  }
}

void main() {
  switch (scrutinee(2)) {
    case 1:
      print(10);
    case 2:
      print(20);
    default:
      print(30);
  }
  String word = 'b';
  switch (word) {
    case 'a':
      print(1);
    case 'b':
      print(2);
  }
  bool flag = true;
  print(switch (flag) { true => 'sim', false => 'nao' });
  print(describe(Color.green));
  print(describe(Color.blue));
  print(area(Circle()));
  print(area(Square()));
  print(guarded(11));
  print(guarded(10));
  print(guarded(1));
  int? maybe = 5;
  switch (maybe) {
    case null:
      print(0);
    case int v:
      print(v + 1);
  }
  for (var i = 0; i < 4; i++) {
    switch (i) {
      case 0:
        continue;
      case 1:
        break;
      default:
        print(i);
    }
    print(100 + i);
  }
}
";
const EXPECTED: &str = "2\n20\n2\nsim\nverde\noutro\n3\n4\n22\n0\n-1\n6\n101\n2\n102\n3\n103\n";

/// Usa o frontend real para preservar tipos de expressão e cobertura dos padrões.
fn emit(source: &str) -> Result<String, Diagnostic> {
    let tokens = dartforge_lexer::lex(source)?;
    let program = dartforge_parser::parse(&tokens, source.len())?;
    let resolution = dartforge_semantic::analyze(&program)?;
    dartforge_llvm::emit(&dartforge_hir::lower_resolved(program, resolution))
}

/// Cada família de discriminante usa a comparação correspondente do runtime.
#[test]
fn switches_lower_every_supported_scrutinee_family() {
    let ir = emit(SOURCE).unwrap();
    // int compara bits, String compara conteúdo e enum compara identidade canônica.
    assert!(ir.contains("icmp eq i64 %v0, 1"));
    assert!(ir.contains("call i8 @dartforge_string_equal"));
    assert!(ir.contains("call i64 @dartforge_enum_get"));
    // O cone selado consulta a classe concreta, sem despacho virtual.
    assert!(ir.contains("call i64 @dartforge_object_class"));
    // A guarda usa o binding já materializado no bloco do caso.
    assert!(ir.contains("icmp sgt i64"));
    // Switch expressão junta braços com phi; nenhum resultado fica indefinido.
    assert!(ir.contains("phi i64"));
    assert!(ir.contains("call void @dartforge_null_assert_fail()\n  unreachable"));
}

/// Nenhum caso cai no seguinte: todo corpo termina com salto ou return próprio.
#[test]
fn cases_never_fall_through_to_the_next_pattern() {
    let ir = emit(SOURCE).unwrap();
    for printed in [10, 20, 30] {
        let body = ir
            .split(&format!("call void @dartforge_print_i64(i64 {printed})\n"))
            .nth(1)
            .unwrap();
        assert!(body.starts_with("  br label %"), "{printed}");
    }
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
        let path =
            std::env::temp_dir().join(format!("dartforge-switches-{}-{stamp}", std::process::id()));
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

/// LLVM O0/O2 deve reproduzir a saída do Dart 3.6.2 mesmo sob GC forçado.
#[test]
#[ignore = "requer Clang e rustc/linker nativos ou DARTFORGE_CLANG/DARTFORGE_RUSTC"]
fn native_switches_match_dart_3_6_2_with_gc_stress() {
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
