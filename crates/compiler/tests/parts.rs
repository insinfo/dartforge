//! Contrato público de `part`/`part of`: namespace, privacidade e diagnósticos.
use dartforge_compiler::{Optimization, compile_path};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Diretório exclusivo de teste, removido somente depois de todas as leituras.
struct Fixture(PathBuf);
impl Fixture {
    /// Cria arquivos mínimos sem depender do diretório atual ou do SDK instalado.
    fn new(files: &[(&str, &str)]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "dartforge-parts-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        let fixture = Self(path);
        fixture.write(files);
        fixture
    }
    /// Sobrescreve arquivos da fixture, criando subdiretórios quando necessário.
    fn write(&self, files: &[(&str, &str)]) {
        for (name, text) in files {
            let path = self.0.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }
    /// Devolve a entrada da fixture.
    fn entry(&self) -> PathBuf {
        self.0.join("main.dart")
    }
}
impl Drop for Fixture {
    /// Remove apenas o diretório temporário exclusivo criado pela própria fixture.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A parte declara símbolos da biblioteca pai e compartilha públicos e privados.
#[test]
fn part_shares_public_and_private_declarations() {
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "part 'sub/parte.dart'; int _lerDe(Caixa c)=>c._valor; void main(){ print(ajuda() + _segredo() + _lerDe(Caixa())); }",
        ),
        (
            "sub/parte.dart",
            "part of '../main.dart'; int ajuda()=>1; int _segredo()=>2; class Caixa{int _valor=4;}",
        ),
    ]);
    for optimization in [Optimization::None, Optimization::Constants] {
        let javascript = compile_path(&fixture.entry(), optimization).unwrap();
        assert!(javascript.contains("$lib0$ajuda"), "{javascript}");
        assert!(javascript.contains("$lib0$_segredo"), "{javascript}");
        assert!(javascript.contains("$lib0$_valor"), "{javascript}");
    }
}

/// A parte usa imports do pai e pode declarar o main exigido pela entrada.
#[test]
fn part_reuses_parent_imports_and_may_declare_main() {
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "library app.exemplo; import 'outra.dart'; part 'parte.dart';",
        ),
        ("outra.dart", "int externo()=>7;"),
        (
            "parte.dart",
            "part of app.exemplo; void main(){ print(externo()); }",
        ),
    ]);
    let javascript = compile_path(&fixture.entry(), Optimization::None).unwrap();
    assert!(javascript.contains("$lib0$main"), "{javascript}");
    assert!(javascript.contains("$lib1$externo"), "{javascript}");
    // Sem a biblioteca declarante o mesmo arquivo não enxerga o import do pai.
    fixture.write(&[("main.dart", "import 'parte.dart'; void main(){}")]);
    assert!(compile_path(&fixture.entry(), Optimization::None).is_err());
}

/// Nomes duplicados entre pai e parte colidem no mesmo namespace da biblioteca.
#[test]
fn part_and_parent_share_a_single_namespace() {
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "part 'parte.dart'; int ajuda()=>1; void main(){ print(ajuda()); }",
        ),
        ("parte.dart", "part of 'main.dart'; int ajuda()=>2;"),
    ]);
    let error = compile_path(&fixture.entry(), Optimization::None).unwrap_err();
    assert_eq!(
        error.path.file_name(),
        Some(std::ffi::OsStr::new("parte.dart"))
    );
    assert!(error.message.contains("duplicado"), "{error}");
    let span = error.span.expect("diagnóstico de parte tem posição");
    let source = std::fs::read_to_string(&error.path).unwrap();
    assert!(span.start <= span.end && span.end <= source.len());
}

/// Erros da parte mantêm arquivo e bytes originais do arquivo que os provoca.
#[test]
fn part_diagnostics_keep_file_and_local_span() {
    let source = "part of 'main.dart'; int ajuda(){ return 'ação'; }";
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "part 'parte.dart'; void main(){ print(ajuda()); }",
        ),
        ("parte.dart", source),
    ]);
    let error = compile_path(&fixture.entry(), Optimization::None).unwrap_err();
    assert_eq!(
        error.path.file_name(),
        Some(std::ffi::OsStr::new("parte.dart"))
    );
    let span = error.span.expect("erro semântico tem posição");
    assert!(
        span.start <= span.end && span.end <= source.len(),
        "{error}"
    );
    assert!(source.is_char_boundary(span.start) && source.is_char_boundary(span.end));
    assert!(source[span.start..span.end].contains("ação"), "{error}");
}

/// Diretivas proibidas na parte e reivindicações conflitantes são rejeitadas.
#[test]
fn rejected_part_configurations() {
    let fixture = Fixture::new(&[
        ("main.dart", "part 'parte.dart'; void main(){}"),
        ("parte.dart", "part of 'main.dart';"),
        ("outra.dart", "int externo()=>7;"),
    ]);
    assert!(compile_path(&fixture.entry(), Optimization::None).is_ok());
    for (parent, part, needle) in [
        (
            "part 'parte.dart'; void main(){}",
            "part of 'main.dart'; import 'outra.dart';",
            "não pode declarar import",
        ),
        (
            "part 'parte.dart'; void main(){}",
            "part of 'outra.dart';",
            "em vez de",
        ),
        (
            "part 'parte.dart'; void main(){}",
            "int ajuda()=>1;",
            "exige part of",
        ),
        (
            "import 'outra.dart'; part 'parte.dart'; void main(){}",
            "part of 'main.dart';",
            "parte de outra biblioteca",
        ),
    ] {
        fixture.write(&[
            ("main.dart", parent),
            ("parte.dart", part),
            (
                "outra.dart",
                if needle == "parte de outra biblioteca" {
                    "part 'parte.dart';"
                } else {
                    "int externo()=>7;"
                },
            ),
        ]);
        let error = compile_path(&fixture.entry(), Optimization::None).unwrap_err();
        assert!(error.message.contains(needle), "{parent} | {part}: {error}");
    }
}

/// O programa ligado com uma parte executa e imprime o resultado combinado.
#[test]
#[ignore = "requer Node.js no PATH"]
fn linked_part_program_executes() {
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "import 'outra.dart'; part 'parte.dart'; int _base()=>3; void main(){ print(total()); }",
        ),
        ("outra.dart", "int externo()=>7;"),
        (
            "parte.dart",
            "part of 'main.dart'; class Caixa{int _valor=2;} int total()=>_base() + externo() + Caixa()._valor;",
        ),
    ]);
    for optimization in [Optimization::None, Optimization::Constants] {
        let javascript = compile_path(&fixture.entry(), optimization).unwrap();
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "-e", &javascript])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "12");
    }
}
