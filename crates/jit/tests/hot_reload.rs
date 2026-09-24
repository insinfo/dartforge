//! Cenários de hot reload migrados da trilha antiga para o IR do emit_native.

use dartforge_jit::JitSession;

/// IR inválido ou referência ausente não substitui a geração em execução.
/// Uma versão válida ainda pode ser publicada depois das duas falhas.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn falha_de_recarga_nao_destroi_a_versao_boa() {
    let ir = |valor: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {valor}\n}}\n");
    let mut sessao = JitSession::new().expect("sessão");
    sessao.add_reloadable_module("app", &ir(7)).expect("geração 1");
    let entrada = sessao.stable_entry("df_fn_0").expect("entrada estável");
    let endereco = sessao.lookup("df_fn_0").expect("endereço estável");
    assert_eq!(entrada.call(&sessao).unwrap(), 7);

    let erro = sessao.hot_reload("app", "ISTO NÃO É LLVM IR @@@ {{{").unwrap_err();
    assert_eq!(erro.stage, "parse-ir");

    let ir_sem_externo = "declare i64 @sqlite3_open(i64)\n\
                          define i64 @df_fn_0() {\n\
                            %r = call i64 @sqlite3_open(i64 0)\n\
                            ret i64 %r\n}\n";
    let erro = sessao.hot_reload("app", ir_sem_externo).unwrap_err();
    assert_eq!(erro.stage, "contract");
    assert!(erro.message.contains("sqlite3_open"), "{erro}");

    assert_eq!(sessao.generation("app"), Some(1));
    assert_eq!(sessao.retained_generations(), 1);
    assert_eq!(sessao.lookup("df_fn_0").unwrap(), endereco);
    assert_eq!(entrada.call(&sessao).unwrap(), 7);

    sessao.hot_reload("app", &ir(8)).expect("recarga após falhas");
    assert_eq!(sessao.generation("app"), Some(2));
    assert_eq!(sessao.lookup("df_fn_0").unwrap(), endereco);
    assert_eq!(entrada.call(&sessao).unwrap(), 8);
}
