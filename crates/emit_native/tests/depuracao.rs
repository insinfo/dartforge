//! J05: as tabelas de linha do depurador nativo (`CompileOptions::debug_info`,
//! `llvm/debug_info.rs`), conferidas no IR. Sem Clang: só a emissão.

use dartforge_emit_native::{CompileOptions, emit_ir_with};
use std::path::Path;

const SDK: &str = "C:/tools/dartsdk-3.6.2/lib";

const PROGRAM: &str = "int dobro(int x) {
  final y = x * 2;
  return y;
}

class Conta {
  int saldo = 0;
  void depositar(int v) {
    saldo += v;
  }
}

void main() {
  var total = 0;
  for (var i = 0; i < 3; i++) {
    total += dobro(i);
  }
  Conta().depositar(total);
  print(total);
}
";

/// O IR do programa, com ou sem depuração, ou `None` sem o SDK na máquina.
fn ir(debug_info: bool) -> Option<String> {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB").unwrap_or_else(|_| SDK.to_string());
    if !Path::new(&sdk).join("libraries.json").is_file() {
        eprintln!("SDK ausente em {sdk}; teste pulado");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("main.dart");
    std::fs::write(&entry, PROGRAM).unwrap();
    let text = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let options = CompileOptions {
                sdk: Some(Path::new(&sdk)),
                packages: None,
                timings: false,
                optimize: false,
                versao_linguagem: None,
                experimentos: Vec::new(),
                depuracao: debug_info,
                gerador: None,
                cpu: None,
            };
            emit_ir_with(&entry, &options).unwrap_or_else(|e| panic!("não compilou:\n{e}")).texto
        })
        .unwrap()
        .join()
        .unwrap();
    Some(text)
}

/// O corpo de `define … @symbol(` até o `}`.
fn function<'a>(ir: &'a str, symbol: &str) -> &'a str {
    let ini = ir.find(&format!("@{symbol}(")).unwrap_or_else(|| panic!("{symbol} ausente"));
    let ini = ir[..ini].rfind("define ").unwrap();
    let end = ir[ini..].find("\n}\n").map_or(ir.len(), |f| ini + f);
    &ir[ini..end]
}

/// A linha da `DILocation` `!n`.
fn line_of(ir: &str, n: &str) -> u32 {
    let def = ir.lines().find(|l| l.starts_with(&format!("{n} = !DILocation("))).unwrap_or_else(|| panic!("{n} ausente"));
    def.split("line: ").nth(1).and_then(|r| r.split(',').next()).and_then(|x| x.parse().ok()).unwrap()
}

#[test]
fn each_instruction_carries_statement_line() {
    let Some(ir) = ir(true) else { return };
    assert!(ir.contains("!llvm.dbg.cu = !{!0}"), "sem unidade de compilação");
    assert!(ir.contains("emissionKind: FullDebug"));
    assert!(ir.contains("!DIFile(filename: \"main.dart\""));
    // Os subprogramas com o nome Dart e a linha do primeiro comando.
    for (name, line) in [("dobro", 2), ("Conta.depositar", 9), ("main", 14)] {
        assert!(
            ir.contains(&format!("!DISubprogram(name: \"{name}\", ")) && ir.contains(&format!("line: {line}, type:")),
            "subprograma {name} (linha {line})"
        );
    }
    // Toda instrução de `double_of` tem `!dbg`, e o `ret` é o do `return y;`
    // (linha 3), que não emite instrução própria.
    let double_of = function(&ir, "df.main$2edart..dobro");
    assert!(double_of.lines().next().unwrap().contains(" !dbg !"), "{double_of}");
    for l in double_of.lines().skip(1).filter(|l| l.starts_with("  ")) {
        assert!(l.contains(", !dbg !"), "instrução sem !dbg: {l}");
    }
    let ret = double_of.lines().find(|l| l.trim_start().starts_with("ret ")).expect("ret");
    let n = ret.rsplit(", !dbg ").next().unwrap();
    assert_eq!(line_of(&ir, n), 3, "{ret}");
    // A função com posições não é embutida na HIR: `main` a chama.
    assert!(function(&ir, "dart_main").contains("@df.main$2edart..dobro("));
}

#[test]
fn without_debug_there_is_no_metadata() {
    let Some(ir) = ir(false) else { return };
    assert!(!ir.contains("!dbg") && !ir.contains("!llvm.dbg.cu") && !ir.contains("; df.pos"));
}
