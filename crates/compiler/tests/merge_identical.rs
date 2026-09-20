//! Regressões do passe opcional de fusão e das opções completas no cache.
use dartforge_compiler::*;
use std::{path::PathBuf, process::Command};
/// Diretório temporário exclusivo, removido apenas pelo proprietário.
struct Temp(PathBuf);
impl Temp {
    /// Cria um diretório sem usar entradas externas como destino.
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("dartforge-merge-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    /// Remove apenas o diretório temporário exclusivo deste teste.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
/// Opções explícitas para fusão sem avaliação de constantes.
fn merging() -> CompileOptions {
    CompileOptions {
        merge_identical_functions: true,
        ..Default::default()
    }
}
/// Funções iguais desaparecem do JS e LLVM; main exportado permanece.
#[test]
fn definition_reduction_is_optional_and_preserves_output() {
    let src =
        "int a(int x) => x + 1; int b(int x) => x + 1; void main() { print(a(2)); print(b(4)); }";
    let plain = compile(src).unwrap();
    let merged = compile_with_options(src, merging()).unwrap();
    assert!(plain.contains("function $df_b("));
    assert!(!merged.contains("function $df_b("));
    assert!(merged.contains("export function main"));
    let ir = compile_llvm(src).unwrap();
    let merged_ir = compile_llvm_with_options(src, merging()).unwrap();
    assert_eq!(
        ir.matches("define ").count(),
        merged_ir.matches("define ").count() + 1
    );
    let dir = Temp::new();
    let file = dir.0.join("main.mjs");
    std::fs::write(&file, merged).unwrap();
    let output = Command::new("node")
        .arg(file)
        .output()
        .expect("Node necessário para teste diferencial");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().replace('\r', ""),
        "3\n5\n"
    );
}
/// Grafo importado preserva namespaces e invalida cache quando somente a fusão muda.
#[test]
fn imported_arrows_and_cache_options() {
    let dir = Temp::new();
    let main = dir.0.join("main.dart");
    std::fs::write(dir.0.join("a.dart"), "int alpha(int x) => x + 2;").unwrap();
    std::fs::write(dir.0.join("b.dart"), "int beta(int x) => x + 2;").unwrap();
    std::fs::write(
        &main,
        "import 'a.dart'; import 'b.dart'; void main() { print(alpha(1)); print(beta(2)); }",
    )
    .unwrap();
    let mut session = CompilerSession::new();
    let plain = session.compile_path(&main, Optimization::None).unwrap();
    let merged = session.compile_path_with_options(&main, merging()).unwrap();
    assert!(!merged.stats.cache_hit);
    assert!(merged.javascript.len() < plain.javascript.len());
    assert!(
        session
            .compile_path_with_options(&main, merging())
            .unwrap()
            .stats
            .cache_hit
    );
    assert!(
        !session
            .compile_path(&main, Optimization::None)
            .unwrap()
            .stats
            .cache_hit
    );
    let file = dir.0.join("linked.mjs");
    std::fs::write(&file, merged.javascript.as_bytes()).unwrap();
    let out = Command::new("node").arg(file).output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().replace('\r', ""),
        "3\n4\n"
    );
    assert!(compile_path_llvm_with_options(&main, merging()).is_ok());
}
/// Constantes e fusão são independentes e podem ser combinadas explicitamente.
#[test]
fn constants_remain_independent() {
    let src = "int a() { return 1 + 1; } int b() { return 2; } void main() { print(b()); }";
    assert!(
        compile_with_options(src, merging())
            .unwrap()
            .contains("function $df_b(")
    );
    let both = CompileOptions {
        optimization: Optimization::Constants,
        ..merging()
    };
    assert!(
        !compile_with_options(src, both)
            .unwrap()
            .contains("function $df_b(")
    );
}
/// Integra LLVM, clang e runtime real quando a toolchain nativa é solicitada.
#[test]
#[ignore = "requer clang e rustc configurados"]
fn merged_native_executes() {
    let ir = compile_llvm_with_options(
        "int a(int x) => x * 2; int b(int x) => x * 2; void main() { print(a(3)); print(b(4)); }",
        merging(),
    )
    .unwrap();
    let dir = Temp::new();
    let output = dir.0.join("merged.exe");
    dartforge_native::build_executable(&ir, &output, &Default::default()).unwrap();
    let result = Command::new(output).output().unwrap();
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout).unwrap().replace('\r', ""),
        "6\n8\n"
    );
}
