//! Aceite de A2: a sonda de 7 erros sai com o **mesmo JSON, byte a byte**, que
//! o `dart analyze --format=json` do SDK 3.6.2 gravou
//! (`corpus/diagnosticos/sonda/dart-analyze.json`, com o caminho do arquivo
//! trocado por `<SONDA>`), dados o código, os argumentos e o intervalo de cada
//! diagnóstico — isto é: a tabela gerada, a renderização dos moldes, a
//! severidade, o tipo, a URL da documentação, a conversão UTF-16/linha/coluna
//! e a ordem de relato estão certas. Quem decide código, argumentos e
//! intervalo é a análise (`types`, pedido T1); o placar do grupo `sonda` mede
//! quanto dela já chega lá.

use dartforge_diagnostics::{Codigo, Diagnostic, Span};
use dartforge_paridade::json;

fn d(unico: &str, inicio: usize, fim: usize, args: &[&str]) -> Diagnostic {
    let c = Codigo::por_unico(unico).unwrap_or_else(|| panic!("código {unico}"));
    Diagnostic::com_codigo(c, Span { start: inicio, end: fim }, args.iter().copied())
}

#[test]
fn sonda_de_7_erros_igual_ao_oraculo() {
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnosticos/sonda");
    let fonte = std::fs::read_to_string(raiz.join("lib/a.dart")).expect("sonda");
    let esperado = std::fs::read_to_string(raiz.join("dart-analyze.json")).expect("oráculo gravado");
    let linhas = json::Linhas::new(&fonte);
    let diags = [
        d("CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_FUNCTION", 41, 44, &["String", "int", "f"]),
        d("CompileTimeErrorCode.UNDEFINED_FUNCTION", 83, 84, &["h"]),
        d("CompileTimeErrorCode.NOT_ASSIGNED_POTENTIALLY_NON_NULLABLE_LOCAL_VARIABLE", 105, 106, &["y"]),
        d("CompileTimeErrorCode.ARGUMENT_TYPE_NOT_ASSIGNABLE", 113, 116, &["String", "int", ""]),
        d("CompileTimeErrorCode.NON_BOOL_CONDITION", 125, 126, &[]),
        d("CompileTimeErrorCode.UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE", 148, 154, &["length"]),
        d("WarningCode.UNUSED_LOCAL_VARIABLE", 162, 167, &["nunca"]),
    ];
    // Ordem de propósito embaralhada: a ordem de relato é do `json::ordenar`.
    let mut v: Vec<_> = diags.iter().rev().map(|x| json::para_json("<SONDA>", &linhas, x, false)).collect();
    json::ordenar(&mut v);
    assert_eq!(json::escrever(v), esperado.trim_end());
}
