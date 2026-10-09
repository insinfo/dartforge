//! A tabela parcial deve distinguir referências de escalares e ausência de contrato.

// Inclui as rejeições do gerador na suíte Cargo, sem executar seu main.
#[allow(dead_code)]
#[path = "../build.rs"]
mod gerador;

use dartforge_runtime::{
    efeitos::EFEITOS,
    ownership::{CONTRATOS, ModoParametro, ModoResultado},
    simbolos::NOMES,
};

#[test]
fn contratos_arc_cobrem_exportacoes_sem_inventar_contratos_para_outras_externs() {
    let auditadas: Vec<_> = NOMES
        .iter()
        .filter(|n| {
            n.starts_with("dartforge_arc_")
                || matches!(
                    **n,
                    "dartforge_gc_global_root"
                        | "dartforge_marcar_constante"
                        | "dartforge_gc_collect"
                        | "dartforge_marcar_permanente"
                        | "dartforge_nativo_DartForge_record_fieldAt"
                )
        })
        .collect();
    assert_eq!(auditadas.len(), CONTRATOS.len());
    for nome in auditadas {
        let c = CONTRATOS.iter().find(|c| c.nome == *nome).unwrap();
        let efeito = EFEITOS.iter().find(|e| e.0 == c.nome).unwrap();
        assert_eq!(efeito.2, c.pode_falhar);
        assert!(!efeito.3);
    }
    let receber = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_global_receber_v1")
        .unwrap();
    assert_eq!(
        receber.parametros,
        &[ModoParametro::Native, ModoParametro::Consume]
    );
    assert!(receber.retencao_persistente && receber.invalida_borrows);
    let carregar = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_quadro_carregar_v1")
        .unwrap();
    assert_eq!(carregar.resultado, ModoResultado::Owned);
    let campo = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_nativo_DartForge_record_fieldAt")
        .unwrap();
    assert_eq!(campo.resultado, ModoResultado::BorrowArg(0));
    assert!(!CONTRATOS.iter().any(|c| c.nome == "dartforge_alocar"));
    for nome in ["dartforge_gc_global_root", "dartforge_marcar_constante"] {
        let c = CONTRATOS.iter().find(|c| c.nome == nome).unwrap();
        assert!(c.retencao_persistente);
        assert!(!c.parametros.contains(&ModoParametro::Consume));
    }
}
