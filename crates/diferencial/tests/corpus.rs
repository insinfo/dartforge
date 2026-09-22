//! Testes do corpus (precisam de `dart` e `node` no PATH; rodar com `--ignored`):
//!
//! ```
//! cargo test -p dartforge-diferencial --test corpus -- --ignored --nocapture
//! ```

use dartforge_diferencial::{Ambiente, Opcoes, executar_corpus, listar, relatorio};

fn programas(amb: &Ambiente) -> Vec<dartforge_diferencial::Programa> {
    let p = listar(&amb.raiz.join("corpus/js"), None);
    assert!(p.len() >= 120, "o corpus tem {} programas; mínimo 120", p.len());
    p
}

/// O harness inteiro no corpus: imprime o relatório (DartForge incluído quando o binário
/// existe) e falha se algum programa não roda no `dart run` — o oráculo tem de ser válido.
#[test]
#[ignore]
fn corpus_roda_no_dart_run() {
    let amb = Ambiente::detectar();
    let programas = programas(&amb);
    let op = Opcoes { com_forge: amb.dartforge_bin.is_some(), threads: 0, ..Default::default() };
    let resultados = executar_corpus(&amb, &programas, op, |_| {});
    print!("{}", relatorio(&resultados));
    let invalidos: Vec<String> = resultados
        .iter()
        .filter(|r| r.dart.codigo != 0)
        .map(|r| format!("{} (código {}): {}", r.programa.nome, r.dart.codigo, r.dart.primeira_linha_stderr()))
        .collect();
    assert!(invalidos.is_empty(), "programas que não rodam no dart run:\n{}", invalidos.join("\n"));
}

/// `dartdevc`+Node reproduz a VM em 100% do corpus, salvo os programas cujo cabeçalho
/// declara `// diverge-ddc: motivo` — esses têm de divergir de fato.
#[test]
#[ignore]
fn ddc_bate_com_dart_run() {
    let amb = Ambiente::detectar();
    let programas = programas(&amb);
    let resultados = executar_corpus(&amb, &programas, Opcoes { com_forge: false, threads: 0, ..Default::default() }, |_| {});
    let mut problemas = Vec::new();
    for r in &resultados {
        match (r.ddc_vs_dart(), &r.programa.diverge_ddc) {
            (Some(d), None) => problemas.push(format!("{}: DDC ≠ VM ({d:?}); stderr ddc: {}", r.programa.nome, r.ddc.primeira_linha_stderr())),
            (None, Some(m)) => problemas.push(format!("{}: declara divergência ({m}) mas as saídas batem", r.programa.nome)),
            _ => {}
        }
    }
    let declarados: Vec<&str> = resultados.iter().filter_map(|r| r.programa.diverge_ddc.as_ref().map(|m| m.as_str())).collect();
    println!("{} programas; {} com divergência declarada: {:?}", resultados.len(), declarados.len(), declarados);
    assert!(problemas.is_empty(), "{}", problemas.join("\n"));
}
