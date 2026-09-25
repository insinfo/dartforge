//! Cenários de hot reload migrados da trilha antiga para o IR do emit_native.

use dartforge_jit::JitSession;

/// Uma biblioteca compartilhada com dois exports sem argumentos,
/// `GetTickCount` (qualquer valor) e `GetCurrentProcessId` (o pid), e o
/// `exportados.def` ao lado com os `nomes` pedidos. Serve para testar a
/// fronteira da biblioteca do SDK sem compilar o SDK inteiro.
///
/// No Windows é uma cópia da `kernel32.dll`, que exporta os dois. Nos outros
/// sistemas, uma biblioteca de duas funções compilada agora pelo Clang
/// (`DARTFORGE_CLANG`, senão o do `PATH`), com `GetCurrentProcessId` sobre o
/// `getpid` da libc.
fn biblioteca_com_dois_exports(dir: &std::path::Path, nomes: &[&str]) -> std::path::PathBuf {
    let mut def = String::from("EXPORTS\n");
    for n in nomes {
        def.push_str(n);
        def.push('\n');
    }
    std::fs::write(dir.join("exportados.def"), def).unwrap();
    if cfg!(windows) {
        let origem = std::path::PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot"))
            .join("System32/kernel32.dll");
        let dll = dir.join("kernel32.dll");
        std::fs::copy(&origem, &dll).expect("cópia de kernel32 no target da worktree");
        return dll;
    }
    let ll = dir.join("dois.ll");
    std::fs::write(
        &ll,
        "declare i32 @getpid()\n\
         define i32 @GetTickCount() {\n  ret i32 1\n}\n\
         define i32 @GetCurrentProcessId() {\n  %p = call i32 @getpid()\n  ret i32 %p\n}\n",
    )
    .unwrap();
    let dll = dir.join(if cfg!(target_os = "macos") { "libdois.dylib" } else { "libdois.so" });
    let clang = std::env::var_os("DARTFORGE_CLANG").unwrap_or_else(|| "clang".into());
    let status = std::process::Command::new(clang)
        .args(["-x", "ir", "-shared", "-fPIC"])
        .arg(&ll)
        .arg("-o")
        .arg(&dll)
        .status()
        .expect("Clang");
    assert!(status.success(), "o Clang não compilou a biblioteca de teste");
    dll
}

/// Auxiliares com linkage interno pertencem a cada módulo LLVM; o emissor
/// atual gera `df_clo_invalido` assim. Eles não podem virar entradas públicas.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn auxiliares_internos_nao_viram_trampolins() {
    let ir = |valor: i64| format!("\
define internal i64 @df_clo_invalido() {{ ret i64 {valor} }}
define i64 @df_fn_0() {{
  %r = call i64 @df_clo_invalido()
  ret i64 %r
}}
");
    let mut sessao = JitSession::new().expect("sessão");
    let primeira = sessao.add_reloadable_module("app", &ir(1)).expect("primeira geração");
    assert_eq!(primeira.entries, 1);
    assert_eq!(sessao.stable_entries("app"), vec!["df_fn_0"]);
    let entrada = sessao.stable_entry("df_fn_0").unwrap();
    assert_eq!(entrada.call(&sessao).unwrap(), 1);
    sessao.hot_reload("app", &ir(2)).expect("segunda geração");
    assert_eq!(entrada.call(&sessao).unwrap(), 2);
}

/// O `main` com SDK da fonte tem ABI `i32 ()` e deve usar o trampolim
/// atualizado. Uma biblioteca de sistema basta para testar a fronteira sem
/// compilar o SDK inteiro localmente; o teste de CLI remoto cobre a real.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn main_da_sessao_com_dll_chama_geracao_nova() {
    struct Diretorio(std::path::PathBuf);
    impl Drop for Diretorio {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    let dir = Diretorio(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-jit-main-dll-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let dll = biblioteca_com_dois_exports(&dir.0, &["GetTickCount"]);

    let ir = |valor: i32| format!("define i32 @main() {{ ret i32 {valor} }}\n");
    let mut sessao = JitSession::new_com_sdk(&dll, &[]).unwrap();
    sessao.add_reloadable_module("app", &ir(7)).unwrap();
    assert_eq!(sessao.run_reloadable_main().unwrap().exit_code, 7);
    sessao.hot_reload("app", &ir(9)).unwrap();
    assert_eq!(sessao.run_reloadable_main().unwrap().exit_code, 9);
}

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

/// Cinco recargas consecutivas mantêm o endereço público estável e avançam
/// geração por geração. O código antigo permanece retido pela política atual;
/// este teste não afirma ausência de crescimento de memória.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn cinco_recargas_preservam_entrada_estavel() {
    let ir = |valor: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {valor}\n}}\n");
    let mut sessao = JitSession::new().expect("sessão");
    sessao.add_reloadable_module("app", &ir(0)).expect("geração inicial");
    let entrada = sessao.stable_entry("df_fn_0").expect("entrada estável");
    let endereco = sessao.lookup("df_fn_0").expect("endereço público");
    assert_eq!(entrada.call(&sessao).unwrap(), 0);

    for valor in 1..=5 {
        let relatorio = sessao.hot_reload("app", &ir(valor)).expect("recarga");
        assert_eq!(relatorio.generation, valor as u32 + 1);
        assert_eq!(relatorio.new_entries, 0);
        assert_eq!(relatorio.retained_generations, valor as usize + 1);
        assert_eq!(sessao.lookup("df_fn_0").unwrap(), endereco);
        assert_eq!(entrada.call(&sessao).unwrap(), valor);
        assert_eq!(sessao.module_names(), vec!["app"]);
    }
}

/// A promoção recusa uma função nova com o nome já definido por outro módulo
/// antes de descarregar o programa antigo. A entrada anterior continua viva.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn colisao_com_outro_modulo_nao_destroi_promocao() {
    let antigo = "define i32 @main() { ret i32 7 }\n";
    let novo = "define i32 @main() { ret i32 8 }\n\
define i64 @df_fn_1() { ret i64 1 }\n";
    let mut sessao = JitSession::new().expect("sessão");
    sessao.add_ir_module("app", antigo).expect("programa antigo");
    sessao.add_ir_module("biblioteca", "define i64 @df_fn_1() { ret i64 2 }\n")
        .expect("outra definição residente");
    assert_eq!(sessao.run_main().unwrap().exit_code, 7);

    let erro = sessao.hot_reload("app", novo).unwrap_err();
    assert_eq!(erro.stage, "contract");
    assert!(erro.message.contains("df_fn_1"), "{erro}");
    assert_eq!(sessao.run_main().unwrap().exit_code, 7);
    assert_eq!(sessao.generation("app"), None);
}

/// Uma global externa ausente também é erro de contrato prévio à promoção.
/// O nome não é uma declaração de função, portanto exercita a verificação
/// separada de dados e mantém o `main` antigo executável.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn global_externa_ausente_nao_destroi_promocao() {
    let mut sessao = JitSession::new().expect("sessão");
    sessao.add_ir_module("app", "define i32 @main() { ret i32 7 }\n").unwrap();
    assert_eq!(sessao.run_main().unwrap().exit_code, 7);

    let ir = "@missing = external global i64\n\
define i32 @main() {\n\
  %v = load i64, ptr @missing\n\
  %r = trunc i64 %v to i32\n\
  ret i32 %r\n}\n";
    let erro = sessao.hot_reload("app", ir).unwrap_err();
    assert_eq!(erro.stage, "contract");
    assert!(erro.message.contains("missing"), "{erro}");
    assert_eq!(sessao.run_main().unwrap().exit_code, 7);
    assert_eq!(sessao.generation("app"), None);
}

/// Um erro no nome não escolhe por aproximação o único módulo da sessão.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn nome_desconhecido_nao_promove_o_unico_modulo() {
    let mut sessao = JitSession::new().expect("sessão");
    sessao.add_ir_module("app", "define i32 @main() { ret i32 7 }\n").unwrap();
    let erro = sessao.hot_reload("outro", "define i32 @main() { ret i32 8 }\n")
        .unwrap_err();
    assert_eq!(erro.stage, "contract");
    assert!(erro.message.contains("outro"), "{erro}");
    assert_eq!(sessao.module_names(), vec!["app"]);
    assert_eq!(sessao.run_main().unwrap().exit_code, 7);
}

/// A geração recarregável tem seus próprios estáticos. Execuções consecutivas
/// do `main` reiniciam o indicador, antes e depois da troca de geração.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn main_recarregavel_reinicia_estaticos_da_geracao_ativa() {
    let ir = "@dfg_0_ok = internal global i8 0\n\
define i32 @main() {\n\
  %anterior = load i8, ptr @dfg_0_ok\n\
  store i8 1, ptr @dfg_0_ok\n\
  %codigo = zext i8 %anterior to i32\n\
  ret i32 %codigo\n}\n";
    let mut sessao = JitSession::new().expect("sessão");
    sessao.add_reloadable_module("app", ir).expect("geração 1");
    assert_eq!(sessao.run_main().unwrap().exit_code, 0);
    assert_eq!(sessao.run_main().unwrap().exit_code, 0);

    sessao.hot_reload("app", ir).expect("geração 2");
    assert_eq!(sessao.run_main().unwrap().exit_code, 0);
    assert_eq!(sessao.run_main().unwrap().exit_code, 0);
}

/// Bibliotecas diferentes podem usar o mesmo nome interno `@dfg_*` sem que
/// suas células de dados se misturem na JITDylib compartilhada.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn estaticos_iguais_em_modulos_distintos_ficam_independentes() {
    let ir = |funcao: &str| format!("@dfg_0_ok = internal global i64 0\n\
define i64 @{funcao}() {{\n\
  %anterior = load i64, ptr @dfg_0_ok\n\
  %novo = add i64 %anterior, 1\n\
  store i64 %novo, ptr @dfg_0_ok\n\
  ret i64 %anterior\n}}\n");
    let mut sessao = JitSession::new().expect("sessão");
    sessao.add_reloadable_module("app", &ir("df_fn_0")).expect("app");
    sessao.add_reloadable_module("biblioteca", &ir("df_fn_1")).expect("biblioteca");
    let app = sessao.stable_entry("df_fn_0").expect("entrada app");
    let biblioteca = sessao.stable_entry("df_fn_1").expect("entrada biblioteca");
    assert_eq!(app.call(&sessao).unwrap(), 0);
    assert_eq!(biblioteca.call(&sessao).unwrap(), 0);
    assert_eq!(app.call(&sessao).unwrap(), 1);
    assert_eq!(biblioteca.call(&sessao).unwrap(), 1);
}

/// Uma recarga de corpo mantém os estáticos do programa já inicializados.
/// O cache de despacho da geração anterior deve começar vazio no código novo.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn recarga_preserva_estaticos_do_programa_sem_copiar_caches() {
    let ir = |passo: i64| format!("@dfg_0 = internal global i64 0\n\
@dfg_0$ok = internal global i8 0\n\
@df.ic.0 = private global [2 x i64] zeroinitializer\n\
define i64 @df_fn_0() {{\n\
  %anterior = load i64, ptr @dfg_0\n\
  %novo = add i64 %anterior, {passo}\n\
  store i64 %novo, ptr @dfg_0\n\
  store i8 1, ptr @dfg_0$ok\n\
  ret i64 %novo\n}}\n\
define i64 @df_fn_1() {{\n\
  %celula = getelementptr [2 x i64], ptr @df.ic.0, i64 0, i64 0\n\
  %anterior = load i64, ptr %celula\n\
  %novo = add i64 %anterior, 1\n\
  store i64 %novo, ptr %celula\n\
  ret i64 %novo\n}}\n\
define i64 @df_fn_2() {{\n\
  %ok = load i8, ptr @dfg_0$ok\n\
  %largo = zext i8 %ok to i64\n\
  ret i64 %largo\n}}\n");
    let mut sessao = JitSession::new().expect("sessão");
    sessao.add_reloadable_module("app", &ir(1)).expect("geração 1");
    let proximo = sessao.stable_entry("df_fn_0").expect("estático");
    let cache = sessao.stable_entry("df_fn_1").expect("cache");
    let inicializado = sessao.stable_entry("df_fn_2").expect("indicador");
    assert_eq!(proximo.call(&sessao).unwrap(), 1);
    assert_eq!(proximo.call(&sessao).unwrap(), 2);
    assert_eq!(cache.call(&sessao).unwrap(), 1);
    assert_eq!(cache.call(&sessao).unwrap(), 2);
    assert_eq!(inicializado.call(&sessao).unwrap(), 1);

    sessao.hot_reload("app", &ir(10)).expect("geração 2");
    assert_eq!(inicializado.call(&sessao).unwrap(), 1);
    assert_eq!(proximo.call(&sessao).unwrap(), 12);
    assert_eq!(cache.call(&sessao).unwrap(), 1);

    let tipo_alterado = "@dfg_0 = internal global i32 0\n\
define i64 @df_fn_0() { ret i64 99 }\n\
define i64 @df_fn_1() { ret i64 99 }\n\
define i64 @df_fn_2() { ret i64 99 }\n";
    let erro = sessao.hot_reload("app", tipo_alterado).unwrap_err();
    assert_eq!(erro.stage, "contract");
    assert!(erro.message.contains("dfg_0"), "{erro}");
    assert_eq!(proximo.call(&sessao).unwrap(), 22);
}

/// O emissor atual nomeia globais como `dfg.<biblioteca>.<classe>.<campo>`;
/// a IR antiga usava `dfg_`. O valor e o indicador precisam atravessar a
/// recarga usando os nomes que o compilador realmente publica.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn recarga_preserva_globais_com_nome_do_emissor_atual() {
    let ir = |passo: i64| format!("@dfg.app.C.contador = internal global i64 0\n\
@dfg.app.C.contador$ok = internal global i8 0\n\
define i64 @df_fn_0() {{\n\
  %anterior = load i64, ptr @dfg.app.C.contador\n\
  %novo = add i64 %anterior, {passo}\n\
  store i64 %novo, ptr @dfg.app.C.contador\n\
  store i8 1, ptr @dfg.app.C.contador$ok\n\
  ret i64 %novo\n}}\n\
define i64 @df_fn_1() {{\n\
  %ok = load i8, ptr @dfg.app.C.contador$ok\n\
  %valor = zext i8 %ok to i64\n\
  ret i64 %valor\n}}\n");
    let mut sessao = JitSession::new().expect("sessão");
    sessao.add_reloadable_module("app", &ir(1)).expect("geração 1");
    let avancar = sessao.stable_entry("df_fn_0").expect("avançar");
    let inicializado = sessao.stable_entry("df_fn_1").expect("indicador");
    assert_eq!(avancar.call(&sessao).unwrap(), 1);
    assert_eq!(avancar.call(&sessao).unwrap(), 2);
    assert_eq!(inicializado.call(&sessao).unwrap(), 1);
    sessao.hot_reload("app", &ir(10)).expect("geração 2");
    assert_eq!(inicializado.call(&sessao).unwrap(), 1);
    assert_eq!(avancar.call(&sessao).unwrap(), 12);
}

/// Uma edição que passa a usar outro export da DLL publica esse nome antes
/// de materializar a geração nova. Nome ausente não toca a versão em execução.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn recarga_publica_novo_externo_da_dll() {
    struct Diretorio(std::path::PathBuf);
    impl Drop for Diretorio {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    // A cópia fica no target da worktree (E: na máquina do proprietário),
    // nunca no TEMP do C:. kernel32 oferece dois exports sem compilar o SDK.
    let dir = Diretorio(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("tmp-jit-sdk-export-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let dll = biblioteca_com_dois_exports(&dir.0, &["GetTickCount", "GetCurrentProcessId"]);

    let ir = |nome: &str, retorno_fixo: bool| format!("\
declare i32 @{nome}()
define i64 @df_fn_0() {{
  %valor = call i32 @{nome}()
  %largo = zext i32 %valor to i64
  {}
}}
", if retorno_fixo { "ret i64 1" } else { "ret i64 %largo" });
    let mut sessao = JitSession::new_com_sdk(&dll, &["GetTickCount".to_owned()]).unwrap();
    sessao.add_reloadable_module("app", &ir("GetTickCount", true)).unwrap();
    let entrada = sessao.stable_entry("df_fn_0").unwrap();
    assert_eq!(entrada.call(&sessao).unwrap(), 1);

    let erro = sessao.hot_reload("app", &ir("GetMissingExport", false)).unwrap_err();
    assert_eq!(erro.stage, "contract");
    assert_eq!(sessao.generation("app"), Some(1));
    assert_eq!(entrada.call(&sessao).unwrap(), 1);

    sessao.hot_reload("app", &ir("GetCurrentProcessId", false)).unwrap();
    assert_eq!(sessao.generation("app"), Some(2));
    assert_eq!(entrada.call(&sessao).unwrap(), i64::from(std::process::id()));
}
