//! J05: as tabelas de linha do depurador nativo (`CompileOptions::depuracao`,
//! `llvm/depuracao.rs`), conferidas no IR. Sem Clang: só a emissão.

use dartforge_emit_native::{CompileOptions, emitir_ir_com};
use std::path::Path;

const SDK: &str = "C:/tools/dartsdk-3.6.2/lib";

const PROGRAMA: &str = "int dobro(int x) {
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
fn ir(depuracao: bool) -> Option<String> {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB").unwrap_or_else(|_| SDK.to_string());
    if !Path::new(&sdk).join("libraries.json").is_file() {
        eprintln!("SDK ausente em {sdk}; teste pulado");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("main.dart");
    std::fs::write(&entrada, PROGRAMA).unwrap();
    let texto = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let options = CompileOptions {
                sdk: Some(Path::new(&sdk)),
                packages: None,
                timings: false,
                optimize: false,
                versao_linguagem: None,
                experimentos: Vec::new(),
                depuracao,
                gerador: None,
                cpu: None,
            };
            emitir_ir_com(&entrada, &options).unwrap_or_else(|e| panic!("não compilou:\n{e}")).texto
        })
        .unwrap()
        .join()
        .unwrap();
    Some(texto)
}

/// O corpo de `define … @simbolo(` até o `}`.
fn funcao<'a>(ir: &'a str, simbolo: &str) -> &'a str {
    let ini = ir.find(&format!("@{simbolo}(")).unwrap_or_else(|| panic!("{simbolo} ausente"));
    let ini = ir[..ini].rfind("define ").unwrap();
    let fim = ir[ini..].find("\n}\n").map_or(ir.len(), |f| ini + f);
    &ir[ini..fim]
}

/// A linha da `DILocation` `!n`.
fn linha_de(ir: &str, n: &str) -> u32 {
    let def = ir.lines().find(|l| l.starts_with(&format!("{n} = !DILocation("))).unwrap_or_else(|| panic!("{n} ausente"));
    def.split("line: ").nth(1).and_then(|r| r.split(',').next()).and_then(|x| x.parse().ok()).unwrap()
}

#[test]
fn cada_instrucao_leva_a_linha_do_comando() {
    let Some(ir) = ir(true) else { return };
    assert!(ir.contains("!llvm.dbg.cu = !{!0}"), "sem unidade de compilação");
    assert!(ir.contains("emissionKind: FullDebug"));
    assert!(ir.contains("!DIFile(filename: \"main.dart\""));
    // Os subprogramas com o nome Dart e a linha do primeiro comando.
    for (nome, linha) in [("dobro", 2), ("Conta.depositar", 9), ("main", 14)] {
        assert!(
            ir.contains(&format!("!DISubprogram(name: \"{nome}\", ")) && ir.contains(&format!("line: {linha}, type:")),
            "subprograma {nome} (linha {linha})"
        );
    }
    // Toda instrução de `dobro` tem `!dbg`, e o `ret` é o do `return y;`
    // (linha 3), que não emite instrução própria.
    let dobro = funcao(&ir, "df.main$2edart..dobro");
    assert!(dobro.lines().next().unwrap().contains(" !dbg !"), "{dobro}");
    for l in dobro.lines().skip(1).filter(|l| l.starts_with("  ")) {
        assert!(l.contains(", !dbg !"), "instrução sem !dbg: {l}");
    }
    let ret = dobro.lines().find(|l| l.trim_start().starts_with("ret ")).expect("ret");
    let n = ret.rsplit(", !dbg ").next().unwrap();
    assert_eq!(linha_de(&ir, n), 3, "{ret}");
    // A função com posições não é embutida na HIR: `main` a chama.
    assert!(funcao(&ir, "dart_main").contains("@df.main$2edart..dobro("));
}

#[test]
fn sem_depuracao_nao_ha_metadados() {
    let Some(ir) = ir(false) else { return };
    assert!(!ir.contains("!dbg") && !ir.contains("!llvm.dbg.cu") && !ir.contains("; df.pos"));
}
