//! J05: o depurador nativo para num `arquivo.dart:linha` do executável
//! compilado com `--depuracao` e mostra a pilha com os nomes e as linhas Dart.
//! Roda com o `gdb` (Linux); sem ele, o teste avisa e passa.
#![cfg(feature = "nativo")]

use std::process::Command;

const PROGRAMA: &str = "int dobro(int x) {
  final y = x * 2;
  return y;
}

class Conta {
  int saldo = 0;
  void depositar(int v) {
    saldo += v;
    print('saldo $saldo');
  }
}

void main() {
  var total = 0;
  for (var i = 0; i < 3; i++) {
    total += dobro(i);
  }
  final c = Conta();
  c.depositar(total);
  print(total);
}
";

#[test]
#[ignore = "compila com o SDK da fonte e liga com o Clang; roda no CI do Linux (gdb)"]
fn gdb_para_na_linha_dart_e_mostra_a_pilha() {
    if Command::new("gdb").arg("--version").output().is_err() {
        eprintln!("sem gdb: teste de depuração não roda");
        return;
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/tmp-depuracao-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("diretório do teste");
    let fonte = dir.join("main.dart");
    std::fs::write(&fonte, PROGRAMA).unwrap();
    let exe = dir.join(if cfg!(windows) { "main.exe" } else { "main" });
    let compilacao = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("compile-native")
        .arg(&fonte)
        .arg("-o")
        .arg(&exe)
        .arg("--depuracao")
        .output()
        .expect("CLI");
    assert!(compilacao.status.success(), "{}", String::from_utf8_lossy(&compilacao.stderr));

    // Dois pontos de parada: no `return` de `dobro` (sem instrução própria) e
    // no corpo do método; a pilha de cada um e a saída do programa.
    let gdb = Command::new("gdb")
        .args(["-batch", "-nx"])
        .args(["-ex", "break main.dart:3", "-ex", "break main.dart:9", "-ex", "run", "-ex", "bt 2"])
        .args(["-ex", "delete 1", "-ex", "continue", "-ex", "bt 2", "-ex", "continue"])
        .arg(&exe)
        .output()
        .expect("gdb");
    let _ = std::fs::remove_dir_all(&dir);
    let saida = String::from_utf8_lossy(&gdb.stdout).replace("\r\n", "\n");
    let quadros: Vec<&str> = saida.lines().filter(|l| l.starts_with('#')).collect();
    let pilha = |l: &str| {
        // `#0  0x… in dobro () at main.dart:3` ou `#0  dobro () at main.dart:3`.
        let depois = l.split_once(" in ").map_or_else(|| l.splitn(2, "  ").nth(1).unwrap_or(l), |(_, r)| r);
        let nome = depois.split(" (").next().unwrap_or("");
        let local = depois.rsplit(" at ").next().unwrap_or("");
        format!("{nome} {local}")
    };
    let vista: Vec<String> = quadros.iter().map(|l| pilha(l)).collect();
    assert_eq!(
        vista,
        ["dobro main.dart:3", "main main.dart:17", "Conta.depositar main.dart:9", "main main.dart:20"],
        "saída do gdb:\n{saida}\n{}",
        String::from_utf8_lossy(&gdb.stderr)
    );
    assert!(saida.contains("saldo 6\n6\n"), "a saída do programa:\n{saida}");
}
