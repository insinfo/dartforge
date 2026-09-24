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

/// Um handle guardado pelo chamador continua apontando para o mesmo objeto
/// enquanto a entrada estável passa a chamar o corpo da geração nova.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn contador_vivo_sobrevive_a_recarga() {
    let ir = |passo: i64| format!("\
declare i64 @dartforge_object_new(i64, i64)
declare i64 @dartforge_object_get(i64, i64)
declare void @dartforge_object_set(i64, i64, i64, i8)
define i64 @df_fn_0() {{
  %h = call i64 @dartforge_object_new(i64 7, i64 1)
  ret i64 %h
}}
define i64 @df_fn_1(i64 %h) {{
  %v = call i64 @dartforge_object_get(i64 %h, i64 0)
  %n = add i64 %v, {passo}
  call void @dartforge_object_set(i64 %h, i64 0, i64 %n, i8 0)
  ret i64 %n
}}
");

    let mut sessao = JitSession::new().expect("sessão");
    sessao.add_reloadable_module("contador", &ir(1)).expect("geração 1");
    let criar = sessao.stable_entry("df_fn_0").expect("criar contador");
    let somar = sessao.stable_entry("df_fn_1").expect("incrementar contador");
    let endereco = sessao.lookup("df_fn_1").expect("endereço estável");
    let contador = criar.call(&sessao).expect("objeto vivo");
    assert_eq!(somar.call_with(&sessao, contador).unwrap(), 1);
    assert_eq!(somar.call_with(&sessao, contador).unwrap(), 2);
    assert_eq!(somar.call_with(&sessao, contador).unwrap(), 3);

    sessao.hot_reload("contador", &ir(10)).expect("geração 2");
    assert_eq!(sessao.lookup("df_fn_1").unwrap(), endereco);
    assert_eq!(somar.call_with(&sessao, contador).unwrap(), 13);
    assert_eq!(somar.call_with(&sessao, contador).unwrap(), 23);
}
