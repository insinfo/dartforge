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
fn closure_owned_mantem_contexto_celula_e_arestas_repetidas_ate_o_drop() {
    use dartforge_runtime::{abi::*, layout::cid};
    extern "C" fn entrada(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
    extern "C" fn corpo(_: i64) -> i64 { 7 }
    std::thread::spawn(|| {
        dartforge_memoria_arc_v1();
        for ciclo in [false, true] {
            let valor = dartforge_arc_box_int_owned_v1(i64::MAX);
            let celula = dartforge_arc_objeto_owned_v1(i64::from(cid::CELULA), 1);
            dartforge_arc_gravar_campo_ref_v1(celula, 0, valor);
            dartforge_arc_release(valor);
            let env = dartforge_arc_objeto_owned_v1(i64::from(cid::CONTEXTO), 40);
            for i in [0, 39] { dartforge_arc_gravar_campo_ref_v1(env, i, celula); }
            dartforge_arc_release(celula);
            let codigo = entrada as usize as i64;
            let tipado = corpo as usize as i64;
            let closure = dartforge_arc_closure_owned_v1(codigo, env, tipado, 7);
            assert_eq!(dartforge_arc_observar_heap_v1(env), 3);
            dartforge_arc_release(env);
            if ciclo { dartforge_arc_gravar_campo_ref_v1(env, 38, closure); }
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(closure), 3);
            for h in [env, celula, valor] { assert_eq!(dartforge_arc_observar_heap_v1(h), 1); }
            assert_eq!(dartforge_closure_code(closure), codigo);
            assert_eq!(dartforge_closure_tipada(closure, 7), tipado);
            assert_eq!(dartforge_arc_ler_campo_ref_v1(closure, 1), env);
            assert_eq!(dartforge_arc_ler_campo_ref_v1(env, 39), celula);
            assert_eq!(dartforge_unbox_int(dartforge_arc_ler_campo_ref_v1(celula, 0)), i64::MAX);
            dartforge_arc_release(closure);
            dartforge_arc_collect();
            for h in [closure, env, celula, valor] { assert_eq!(dartforge_arc_observar_heap_v1(h), 0); }
        }
    }).join().unwrap();
}

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
                        | "dartforge_interrupcao_pendente"
                        | "dartforge_ponto_seguro"
                        | "dartforge_exception_clear"
                        | "dartforge_rti_definir"
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
    let instancia = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_objeto_owned_v1")
        .unwrap();
    assert_eq!(
        instancia.parametros,
        &[ModoParametro::Native, ModoParametro::Scalar]
    );
    assert_eq!(instancia.resultado, ModoResultado::Owned);
    assert!(instancia.invalida_borrows);
    assert!(!instancia.pode_falhar && !instancia.chama_dart && !instancia.retencao_persistente);
    assert!(EFEITOS.iter().find(|e| e.0 == instancia.nome).unwrap().1);
    let escalar = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_ler_campo_escalar_v1")
        .unwrap();
    assert_eq!(
        escalar.parametros,
        &[ModoParametro::Borrow, ModoParametro::Scalar]
    );
    assert_eq!(escalar.resultado, ModoResultado::ScalarI64);
    assert!(
        !escalar.pode_falhar
            && !escalar.chama_dart
            && !escalar.retencao_persistente
            && !escalar.invalida_borrows
    );
    for (nome, valor, retencao) in [
        (
            "dartforge_arc_gravar_campo_escalar_v1",
            ModoParametro::Scalar,
            false,
        ),
        (
            "dartforge_arc_gravar_campo_ref_v1",
            ModoParametro::Borrow,
            true,
        ),
    ] {
        let c = CONTRATOS.iter().find(|c| c.nome == nome).unwrap();
        assert_eq!(
            c.parametros,
            &[ModoParametro::Borrow, ModoParametro::Scalar, valor]
        );
        assert_eq!(c.resultado, ModoResultado::Void);
        assert_eq!(c.retencao_persistente, retencao);
        assert!(c.invalida_borrows && !c.pode_falhar && !c.chama_dart);
        assert!(!EFEITOS.iter().find(|e| e.0 == nome).unwrap().1);
    }
    let lancar = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_lancar_ref_v1")
        .unwrap();
    let leitura_ref = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_ler_campo_ref_v1")
        .unwrap();
    assert_eq!(
        leitura_ref.parametros,
        &[ModoParametro::Borrow, ModoParametro::Scalar]
    );
    assert_eq!(leitura_ref.resultado, ModoResultado::BorrowArg(0));
    assert!(
        !leitura_ref.pode_falhar
            && !leitura_ref.chama_dart
            && !leitura_ref.retencao_persistente
            && !leitura_ref.invalida_borrows
    );
    assert!(!EFEITOS.iter().find(|e| e.0 == leitura_ref.nome).unwrap().1);
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
