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
                        | "dartforge_nativo_DartForge_record_numFields"
                        | "dartforge_nativo_DartForge_record_shape"
                        | "dartforge_print_handle"
                        | "dartforge_exception_pending"
                        | "dartforge_exception_clear"
                        | "dartforge_unbox_int"
                        | "dartforge_unbox_double"
                        | "dartforge_unbox_bool"
                )
        })
        .collect();
    assert_eq!(auditadas.len(), CONTRATOS.len());
    for nome in auditadas {
        let c = CONTRATOS.iter().find(|c| c.nome == *nome).unwrap();
        let efeito = EFEITOS.iter().find(|e| e.0 == c.nome).unwrap();
        assert_eq!(efeito.2, c.pode_falhar);
        assert_eq!(efeito.3, c.chama_dart);
        assert!(!c.chama_dart || c.invalida_borrows);
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
    let caixa = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_box_int_owned_v1")
        .unwrap();
    assert_eq!(caixa.parametros, &[ModoParametro::Scalar]);
    assert_eq!(caixa.resultado, ModoResultado::Owned);
    assert!(!caixa.pode_falhar && !caixa.retencao_persistente);
    assert!(caixa.invalida_borrows);
    assert!(EFEITOS.iter().find(|e| e.0 == caixa.nome).unwrap().1);
    let lancar = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_lancar_ref_v1")
        .unwrap();
    assert_eq!(lancar.parametros, &[ModoParametro::Borrow]);
    assert_eq!(lancar.resultado, ModoResultado::Void);
    assert!(lancar.pode_falhar && lancar.retencao_persistente && lancar.invalida_borrows);
    let limpar = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_exception_clear")
        .unwrap();
    assert!(limpar.parametros.is_empty());
    assert!(!limpar.pode_falhar && !limpar.retencao_persistente);
    assert!(limpar.invalida_borrows);
    let campo = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_nativo_DartForge_record_fieldAt")
        .unwrap();
    assert_eq!(campo.resultado, ModoResultado::BorrowArg(0));
    for nome in [
        "dartforge_nativo_DartForge_record_numFields",
        "dartforge_nativo_DartForge_record_shape",
    ] {
        let c = CONTRATOS.iter().find(|c| c.nome == nome).unwrap();
        assert_eq!(c.parametros, &[ModoParametro::Borrow]);
        assert_eq!(c.resultado, ModoResultado::ScalarI64);
        assert!(c.pode_falhar);
        assert!(!c.retencao_persistente && !c.invalida_borrows);
    }
    let imprimir = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_print_handle")
        .unwrap();
    assert_eq!(imprimir.parametros, &[ModoParametro::Borrow]);
    assert_eq!(imprimir.resultado, ModoResultado::Void);
    assert!(imprimir.pode_falhar);
    assert!(!imprimir.retencao_persistente && !imprimir.invalida_borrows);
    assert!(!CONTRATOS.iter().any(|c| c.nome == "dartforge_alocar"));
    for nome in ["dartforge_gc_global_root", "dartforge_marcar_constante"] {
        let c = CONTRATOS.iter().find(|c| c.nome == nome).unwrap();
        assert!(c.retencao_persistente);
        assert!(!c.parametros.contains(&ModoParametro::Consume));
    }
}
