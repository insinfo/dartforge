//! Casos fixos do formatador: cada `tests/casos/<nome>.dart` formatado tem de
//! ser igual, byte a byte, a `<nome>.esperado`, gerado com
//! `dart format -o show --summary none` do SDK 3.6.2. Para (re)gerar um
//! esperado: `dart format -o show --summary none casos/x.dart > casos/x.esperado`.

use std::path::Path;

use dartforge_formatter::{Opcoes, formatar};

#[test]
fn casos_iguais_ao_dart_format() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos");
    let mut nomes: Vec<_> = std::fs::read_dir(&dir)
        .expect("tests/casos")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "dart"))
        .collect();
    nomes.sort();
    assert!(!nomes.is_empty());
    let mut falhas = Vec::new();
    for p in &nomes {
        let fonte = std::fs::read_to_string(p).expect("caso");
        let esperado = std::fs::read_to_string(p.with_extension("esperado")).expect("esperado");
        match formatar(&fonte, &Opcoes::default()) {
            Ok(saida) if saida == esperado => {}
            Ok(saida) => {
                let linha = saida
                    .lines()
                    .zip(esperado.lines())
                    .position(|(a, b)| a != b)
                    .unwrap_or(saida.lines().count().min(esperado.lines().count()));
                falhas.push(format!(
                    "{}: difere na linha {}:\n  nosso:    {:?}\n  esperado: {:?}",
                    p.display(),
                    linha + 1,
                    saida.lines().nth(linha).unwrap_or(""),
                    esperado.lines().nth(linha).unwrap_or("")
                ));
            }
            Err(e) => falhas.push(format!("{}: erro {e}", p.display())),
        }
    }
    assert!(falhas.is_empty(), "{}", falhas.join("\n"));
}

#[test]
fn erro_de_sintaxe_e_recusado() {
    assert!(formatar("void main( {", &Opcoes::default()).is_err());
}

#[test]
fn largura_de_linha_configuravel() {
    let o = Opcoes {
        largura: 20,
        ..Opcoes::default()
    };
    let saida = formatar("void main(){var x=[1,2,3,];print(x);}", &o).unwrap();
    assert_eq!(saida, "void main() {\n  var x = [\n    1,\n    2,\n    3,\n  ];\n  print(x);\n}\n");
}
