//! Cenários de hot reload migrados da trilha antiga para o IR do emit_native.

use dartforge_jit::JitSession;

/// Uma biblioteca compartilhada com dois exports sem argumentos,
/// `GetTickCount` (qualquer valor) e `GetCurrentProcessId` (o pid), e o
/// `exportados.def` ao lado com os `names` pedidos. Serve para testar a
/// fronteira da biblioteca do SDK sem compilar o SDK inteiro.
///
/// No Windows é uma cópia da `kernel32.dll`, que exporta os dois. Nos outros
/// sistemas, uma biblioteca de duas funções compilada agora pelo Clang
/// (`DARTFORGE_CLANG`, senão o do `PATH`), com `GetCurrentProcessId` sobre o
/// `getpid` da libc.
fn library_with_two_exports(dir: &std::path::Path, names: &[&str]) -> std::path::PathBuf {
    let mut def = String::from("EXPORTS\n");
    for n in names {
        def.push_str(n);
        def.push('\n');
    }
    std::fs::write(dir.join("exportados.def"), def).unwrap();
    if cfg!(windows) {
        let origin = std::path::PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot"))
            .join("System32/kernel32.dll");
        let dll = dir.join("kernel32.dll");
        std::fs::copy(&origin, &dll).expect("cópia de kernel32 no target da worktree");
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
        .args(dartforge_emit_native::alvo::argumentos_de_ligacao())
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
fn internal_helpers_do_not_become_trampolines() {
    let ir = |value: i64| format!("\
define internal i64 @df_clo_invalido() {{ ret i64 {value} }}
define i64 @df_fn_0() {{
  %r = call i64 @df_clo_invalido()
  ret i64 %r
}}
");
    let mut session = JitSession::new().expect("sessão");
    let first_one = session.add_reloadable_module("app", &ir(1)).expect("primeira geração");
    assert_eq!(first_one.entries, 1);
    assert_eq!(session.stable_entries("app"), vec!["df_fn_0"]);
    let input = session.stable_entry("df_fn_0").unwrap();
    assert_eq!(input.call(&session).unwrap(), 1);
    session.hot_reload("app", &ir(2)).expect("segunda geração");
    assert_eq!(input.call(&session).unwrap(), 2);
}

/// O `main` com SDK da fonte tem ABI `i32 ()` e deve usar o trampolim
/// atualizado. Uma biblioteca de sistema basta para testar a fronteira sem
/// compilar o SDK inteiro localmente; o teste de CLI remoto cobre a real.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn session_main_with_dll_calls_new_generation() {
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    let dir = Directory(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-jit-main-dll-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let dll = library_with_two_exports(&dir.0, &["GetTickCount"]);

    let ir = |value: i32| format!("define i32 @main() {{ ret i32 {value} }}\n");
    let mut session = JitSession::new_with_sdk(&dll, &[]).unwrap();
    session.add_reloadable_module("app", &ir(7)).unwrap();
    assert_eq!(session.run_reloadable_main().unwrap().exit_code, 7);
    session.hot_reload("app", &ir(9)).unwrap();
    assert_eq!(session.run_reloadable_main().unwrap().exit_code, 9);
}

/// IR inválido ou referência ausente não substitui a geração em execução.
/// Uma versão válida ainda pode ser publicada depois das duas falhas.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn reload_failure_does_not_destroy_good_version() {
    let ir = |value: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {value}\n}}\n");
    let mut session = JitSession::new().expect("sessão");
    session.add_reloadable_module("app", &ir(7)).expect("geração 1");
    let input = session.stable_entry("df_fn_0").expect("entrada estável");
    let address = session.lookup("df_fn_0").expect("endereço estável");
    assert_eq!(input.call(&session).unwrap(), 7);

    let error = session.hot_reload("app", "ISTO NÃO É LLVM IR @@@ {{{").unwrap_err();
    assert_eq!(error.stage, "parse-ir");

    let ir_without_external = "declare i64 @sqlite3_open(i64)\n\
                          define i64 @df_fn_0() {\n\
                            %r = call i64 @sqlite3_open(i64 0)\n\
                            ret i64 %r\n}\n";
    let error = session.hot_reload("app", ir_without_external).unwrap_err();
    assert_eq!(error.stage, "contract");
    assert!(error.message.contains("sqlite3_open"), "{error}");

    assert_eq!(session.generation("app"), Some(1));
    assert_eq!(session.retained_generations(), 1);
    assert_eq!(session.lookup("df_fn_0").unwrap(), address);
    assert_eq!(input.call(&session).unwrap(), 7);

    session.hot_reload("app", &ir(8)).expect("recarga após falhas");
    assert_eq!(session.generation("app"), Some(2));
    assert_eq!(session.lookup("df_fn_0").unwrap(), address);
    assert_eq!(input.call(&session).unwrap(), 8);
}

/// Um handle guardado pelo chamador continua apontando para o mesmo objeto
/// enquanto a entrada estável passa a chamar o corpo da geração nova.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn live_counter_survives_reload() {
    let ir = |step: i64| format!("\
declare i64 @dartforge_object_new(i64, i64)
declare i64 @dartforge_object_get(i64, i64)
declare void @dartforge_object_set(i64, i64, i64, i8)
define i64 @df_fn_0() {{
  %h = call i64 @dartforge_object_new(i64 7, i64 1)
  ret i64 %h
}}
define i64 @df_fn_1(i64 %h) {{
  %v = call i64 @dartforge_object_get(i64 %h, i64 0)
  %n = add i64 %v, {step}
  call void @dartforge_object_set(i64 %h, i64 0, i64 %n, i8 0)
  ret i64 %n
}}
");

    let mut session = JitSession::new().expect("sessão");
    session.add_reloadable_module("contador", &ir(1)).expect("geração 1");
    let create = session.stable_entry("df_fn_0").expect("criar contador");
    let add_up = session.stable_entry("df_fn_1").expect("incrementar contador");
    let address = session.lookup("df_fn_1").expect("endereço estável");
    let counter = create.call(&session).expect("objeto vivo");
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 1);
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 2);
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 3);

    session.hot_reload("contador", &ir(10)).expect("geração 2");
    assert_eq!(session.lookup("df_fn_1").unwrap(), address);
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 13);
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 23);
}

/// Cinco recargas consecutivas mantêm o endereço público estável e avançam
/// geração por geração; cada uma descarrega a anterior, que nenhuma entrada
/// alcança mais (J02): a memória de código não cresce com as recargas.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn five_reloads_preserve_stable_entry() {
    let ir = |value: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {value}\n}}\n");
    let mut session = JitSession::new().expect("sessão");
    session.add_reloadable_module("app", &ir(0)).expect("geração inicial");
    let input = session.stable_entry("df_fn_0").expect("entrada estável");
    let address = session.lookup("df_fn_0").expect("endereço público");
    assert_eq!(input.call(&session).unwrap(), 0);

    for value in 1..=5 {
        let report_data = session.hot_reload("app", &ir(value)).expect("recarga");
        assert_eq!(report_data.generation, value as u32 + 1);
        assert_eq!(report_data.new_entries, 0);
        assert_eq!(report_data.retained_generations, 1);
        assert_eq!(session.lookup("df_fn_0").unwrap(), address);
        assert_eq!(input.call(&session).unwrap(), value);
        assert_eq!(session.module_names(), vec!["app"]);
    }
}

/// Uma função que some do código novo continua alcançável pela entrada
/// estável (um tear-off antigo): a geração dela fica retida até outra
/// geração voltar a implementá-la.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn generation_of_removed_function_is_retained() {
    let with_both = |value: i64| {
        format!("define i64 @df_fn_0() {{\n  ret i64 {value}\n}}\ndefine i64 @df_fn_1() {{\n  ret i64 {}\n}}\n", value + 100)
    };
    let only_the_first = |value: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {value}\n}}\n");
    let mut session = JitSession::new().expect("sessão");
    session.add_reloadable_module("app", &with_both(1)).expect("geração 1");
    let second_one = session.stable_entry("df_fn_1").expect("entrada estável");
    assert_eq!(second_one.call(&session).unwrap(), 101);
    let r = session.hot_reload("app", &only_the_first(2)).expect("geração 2");
    // `df_fn_1` ainda chama o corpo da geração 1.
    assert_eq!(r.retained_generations, 2);
    assert_eq!(second_one.call(&session).unwrap(), 101);
    let r = session.hot_reload("app", &only_the_first(3)).expect("geração 3");
    // A 2 some; a 1 fica pela `df_fn_1`.
    assert_eq!(r.retained_generations, 2);
    assert_eq!(second_one.call(&session).unwrap(), 101);
    let r = session.hot_reload("app", &with_both(4)).expect("geração 4");
    // As duas voltam na 4: a 1 e a 3 saem.
    assert_eq!(r.retained_generations, 1);
    assert_eq!(second_one.call(&session).unwrap(), 104);
}

/// A promoção recusa uma função nova com o nome já definido por outro módulo
/// antes de descarregar o programa antigo. A entrada anterior continua viva.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn collision_with_other_module_does_not_destroy_promotion() {
    let old = "define i32 @main() { ret i32 7 }\n";
    let new = "define i32 @main() { ret i32 8 }\n\
define i64 @df_fn_1() { ret i64 1 }\n";
    let mut session = JitSession::new().expect("sessão");
    session.add_ir_module("app", old).expect("programa antigo");
    session.add_ir_module("biblioteca", "define i64 @df_fn_1() { ret i64 2 }\n")
        .expect("outra definição residente");
    assert_eq!(session.run_main().unwrap().exit_code, 7);

    let error = session.hot_reload("app", new).unwrap_err();
    assert_eq!(error.stage, "contract");
    assert!(error.message.contains("df_fn_1"), "{error}");
    assert_eq!(session.run_main().unwrap().exit_code, 7);
    assert_eq!(session.generation("app"), None);
}

/// Uma global externa ausente também é erro de contrato prévio à promoção.
/// O nome não é uma declaração de função, portanto exercita a verificação
/// separada de dados e mantém o `main` antigo executável.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn missing_external_global_does_not_destroy_promotion() {
    let mut session = JitSession::new().expect("sessão");
    session.add_ir_module("app", "define i32 @main() { ret i32 7 }\n").unwrap();
    assert_eq!(session.run_main().unwrap().exit_code, 7);

    let ir = "@missing = external global i64\n\
define i32 @main() {\n\
  %v = load i64, ptr @missing\n\
  %r = trunc i64 %v to i32\n\
  ret i32 %r\n}\n";
    let error = session.hot_reload("app", ir).unwrap_err();
    assert_eq!(error.stage, "contract");
    assert!(error.message.contains("missing"), "{error}");
    assert_eq!(session.run_main().unwrap().exit_code, 7);
    assert_eq!(session.generation("app"), None);
}

/// Um erro no nome não escolhe por aproximação o único módulo da sessão.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn unknown_name_does_not_promote_the_only_module() {
    let mut session = JitSession::new().expect("sessão");
    session.add_ir_module("app", "define i32 @main() { ret i32 7 }\n").unwrap();
    let error = session.hot_reload("outro", "define i32 @main() { ret i32 8 }\n")
        .unwrap_err();
    assert_eq!(error.stage, "contract");
    assert!(error.message.contains("outro"), "{error}");
    assert_eq!(session.module_names(), vec!["app"]);
    assert_eq!(session.run_main().unwrap().exit_code, 7);
}

/// A geração recarregável tem seus próprios estáticos. Execuções consecutivas
/// do `main` reiniciam o indicador, antes e depois da troca de geração.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn reloadable_main_resets_active_generation_statics() {
    let ir = "@dfg_0_ok = internal global i8 0\n\
define i32 @main() {\n\
  %anterior = load i8, ptr @dfg_0_ok\n\
  store i8 1, ptr @dfg_0_ok\n\
  %codigo = zext i8 %anterior to i32\n\
  ret i32 %codigo\n}\n";
    let mut session = JitSession::new().expect("sessão");
    session.add_reloadable_module("app", ir).expect("geração 1");
    assert_eq!(session.run_main().unwrap().exit_code, 0);
    assert_eq!(session.run_main().unwrap().exit_code, 0);

    session.hot_reload("app", ir).expect("geração 2");
    assert_eq!(session.run_main().unwrap().exit_code, 0);
    assert_eq!(session.run_main().unwrap().exit_code, 0);
}

/// Bibliotecas diferentes podem usar o mesmo nome interno `@dfg_*` sem que
/// suas células de dados se misturem na JITDylib compartilhada.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn equal_statics_in_distinct_modules_stay_independent() {
    let ir = |function: &str| format!("@dfg_0_ok = internal global i64 0\n\
define i64 @{function}() {{\n\
  %anterior = load i64, ptr @dfg_0_ok\n\
  %novo = add i64 %anterior, 1\n\
  store i64 %novo, ptr @dfg_0_ok\n\
  ret i64 %anterior\n}}\n");
    let mut session = JitSession::new().expect("sessão");
    session.add_reloadable_module("app", &ir("df_fn_0")).expect("app");
    session.add_reloadable_module("biblioteca", &ir("df_fn_1")).expect("biblioteca");
    let app = session.stable_entry("df_fn_0").expect("entrada app");
    let library = session.stable_entry("df_fn_1").expect("entrada biblioteca");
    assert_eq!(app.call(&session).unwrap(), 0);
    assert_eq!(library.call(&session).unwrap(), 0);
    assert_eq!(app.call(&session).unwrap(), 1);
    assert_eq!(library.call(&session).unwrap(), 1);
}

/// Uma recarga de corpo mantém os estáticos do programa já inicializados.
/// O cache de despacho da geração anterior deve começar vazio no código novo.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn reload_preserves_program_statics_without_copying_caches() {
    let ir = |step: i64| format!("@dfg_0 = internal global i64 0\n\
@dfg_0$ok = internal global i8 0\n\
@df.ic.0 = private global [2 x i64] zeroinitializer\n\
define i64 @df_fn_0() {{\n\
  %anterior = load i64, ptr @dfg_0\n\
  %novo = add i64 %anterior, {step}\n\
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
    let mut session = JitSession::new().expect("sessão");
    session.add_reloadable_module("app", &ir(1)).expect("geração 1");
    let next_offset = session.stable_entry("df_fn_0").expect("estático");
    let cache = session.stable_entry("df_fn_1").expect("cache");
    let initialized = session.stable_entry("df_fn_2").expect("indicador");
    assert_eq!(next_offset.call(&session).unwrap(), 1);
    assert_eq!(next_offset.call(&session).unwrap(), 2);
    assert_eq!(cache.call(&session).unwrap(), 1);
    assert_eq!(cache.call(&session).unwrap(), 2);
    assert_eq!(initialized.call(&session).unwrap(), 1);

    session.hot_reload("app", &ir(10)).expect("geração 2");
    assert_eq!(initialized.call(&session).unwrap(), 1);
    assert_eq!(next_offset.call(&session).unwrap(), 12);
    assert_eq!(cache.call(&session).unwrap(), 1);

    let changed_type = "@dfg_0 = internal global i32 0\n\
define i64 @df_fn_0() { ret i64 99 }\n\
define i64 @df_fn_1() { ret i64 99 }\n\
define i64 @df_fn_2() { ret i64 99 }\n";
    let error = session.hot_reload("app", changed_type).unwrap_err();
    assert_eq!(error.stage, "contract");
    assert!(error.message.contains("dfg_0"), "{error}");
    assert_eq!(next_offset.call(&session).unwrap(), 22);
}

/// O emissor atual nomeia globais como `dfg.<library>.<class>.<field>`;
/// a IR antiga usava `dfg_`. O valor e o indicador precisam atravessar a
/// recarga usando os nomes que o compilador realmente publica.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn reload_preserves_globals_with_current_emitter_name() {
    let ir = |step: i64| format!("@dfg.app.C.contador = internal global i64 0\n\
@dfg.app.C.contador$ok = internal global i8 0\n\
define i64 @df_fn_0() {{\n\
  %anterior = load i64, ptr @dfg.app.C.contador\n\
  %novo = add i64 %anterior, {step}\n\
  store i64 %novo, ptr @dfg.app.C.contador\n\
  store i8 1, ptr @dfg.app.C.contador$ok\n\
  ret i64 %novo\n}}\n\
define i64 @df_fn_1() {{\n\
  %ok = load i8, ptr @dfg.app.C.contador$ok\n\
  %valor = zext i8 %ok to i64\n\
  ret i64 %valor\n}}\n");
    let mut session = JitSession::new().expect("sessão");
    session.add_reloadable_module("app", &ir(1)).expect("geração 1");
    let advance = session.stable_entry("df_fn_0").expect("avançar");
    let initialized = session.stable_entry("df_fn_1").expect("indicador");
    assert_eq!(advance.call(&session).unwrap(), 1);
    assert_eq!(advance.call(&session).unwrap(), 2);
    assert_eq!(initialized.call(&session).unwrap(), 1);
    session.hot_reload("app", &ir(10)).expect("geração 2");
    assert_eq!(initialized.call(&session).unwrap(), 1);
    assert_eq!(advance.call(&session).unwrap(), 12);
}

/// Uma edição que passa a usar outro export da DLL publica esse nome antes
/// de materializar a geração nova. Nome ausente não toca a versão em execução.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn reload_publishes_new_dll_external() {
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    // A cópia fica no target da worktree (E: na máquina do proprietário),
    // nunca no TEMP do C:. kernel32 oferece dois exports sem compilar o SDK.
    let dir = Directory(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("tmp-jit-sdk-export-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let dll = library_with_two_exports(&dir.0, &["GetTickCount", "GetCurrentProcessId"]);

    let ir = |nome: &str, fixed_return: bool| format!("\
declare i32 @{nome}()
define i64 @df_fn_0() {{
  %valor = call i32 @{nome}()
  %largo = zext i32 %valor to i64
  {}
}}
", if fixed_return { "ret i64 1" } else { "ret i64 %largo" });
    let mut session = JitSession::new_with_sdk(&dll, &["GetTickCount".to_owned()]).unwrap();
    session.add_reloadable_module("app", &ir("GetTickCount", true)).unwrap();
    let input = session.stable_entry("df_fn_0").unwrap();
    assert_eq!(input.call(&session).unwrap(), 1);

    let error = session.hot_reload("app", &ir("GetMissingExport", false)).unwrap_err();
    assert_eq!(error.stage, "contract");
    assert_eq!(session.generation("app"), Some(1));
    assert_eq!(input.call(&session).unwrap(), 1);

    session.hot_reload("app", &ir("GetCurrentProcessId", false)).unwrap();
    assert_eq!(session.generation("app"), Some(2));
    assert_eq!(input.call(&session).unwrap(), i64::from(std::process::id()));
}

/// Recarga estrutural (J03): a assinatura de uma função muda. A função ganha
/// outra entrada estável (`nome$sig1`) e quem a chama no código novo usa a
/// nova; a entrada original fica com o corpo antigo (um tear-off antigo). A
/// mesma assinatura reusa a mesma entrada, e voltar à original reusa a
/// original.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn changed_signature_gets_new_entry() {
    let v1 = "define i64 @df_fn_0() {\n  ret i64 1\n}\n\
              define i64 @df_usa() {\n  %r = call i64 @df_fn_0()\n  ret i64 %r\n}\n";
    let v2 = |k: i64| {
        format!(
            "define i64 @df_fn_0(i64 %a) {{\n  %r = add i64 %a, {k}\n  ret i64 %r\n}}\n\
             define i64 @df_usa() {{\n  %r = call i64 @df_fn_0(i64 5)\n  ret i64 %r\n}}\n"
        )
    };
    let mut session = JitSession::new().expect("sessão");
    session.add_reloadable_module("app", v1).expect("geração 1");
    let uses = session.stable_entry("df_usa").expect("entrada");
    let old_value = session.stable_entry("df_fn_0").expect("entrada");
    assert_eq!(uses.call(&session).unwrap(), 1);

    session.hot_reload("app", &v2(0)).expect("assinatura nova aceita");
    assert_eq!(uses.call(&session).unwrap(), 5);
    assert_eq!(old_value.call(&session).unwrap(), 1, "a entrada original fica com o corpo antigo");
    let new_area = session.stable_entry("df_fn_0$sig1").expect("entrada nova");
    assert_eq!(new_area.call_with(&session, 9).unwrap(), 9);

    session.hot_reload("app", &v2(100)).expect("mesma assinatura, corpo novo");
    assert_eq!(uses.call(&session).unwrap(), 105);
    assert_eq!(new_area.call_with(&session, 9).unwrap(), 109);
    assert!(session.stable_entry("df_fn_0$sig2").is_err(), "a mesma assinatura reusa a entrada");

    session.hot_reload("app", v1).expect("volta à assinatura original");
    assert_eq!(uses.call(&session).unwrap(), 1);
    assert_eq!(old_value.call(&session).unwrap(), 1);
}

/// J04: a geração nova compila só o que mudou. Uma função cujo código (texto
/// e o que ela alcança de local à geração) é o da implementação publicada
/// fica nela; as chamadas a ela, e as dela, passam pelos trampolins, então o
/// chamador mantido já chega ao corpo novo do que mudou.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn unchanged_function_stays_in_live_implementation() {
    let ir = |value: i64, auxiliar: i64| {
        format!(
            "define i64 @df_fn_0() {{\n  %r = call i64 @df_fn_1()\n  ret i64 %r\n}}\n\
             define i64 @df_fn_1() {{\n  ret i64 {value}\n}}\n\
             define i64 @df_fn_2() {{\n  %s = call i64 @auxiliar()\n  ret i64 %s\n}}\n\
             define internal i64 @auxiliar() {{\n  ret i64 {auxiliar}\n}}\n"
        )
    };
    let mut session = JitSession::new().expect("sessão");
    session.add_reloadable_module("app", &ir(1, 7)).expect("geração 1");
    let caller = session.stable_entry("df_fn_0").expect("entrada");
    let uses_local = session.stable_entry("df_fn_2").expect("entrada");
    assert_eq!(caller.call(&session).unwrap(), 1);

    // Só `df_fn_1` muda: é a única compilada; o chamador mantido chega a ela.
    let r = session.hot_reload("app", &ir(2, 7)).expect("geração 2");
    assert_eq!((r.entries, r.kept), (1, 2), "{r:?}");
    assert_eq!(caller.call(&session).unwrap(), 2);
    assert_eq!(uses_local.call(&session).unwrap(), 7);
    // A geração 1 fica: ainda implementa `df_fn_0` e `df_fn_2`.
    assert_eq!(r.retained_generations, 2);

    // A função local muda: quem a alcança é compilado de novo.
    let r = session.hot_reload("app", &ir(2, 8)).expect("geração 3");
    assert_eq!((r.entries, r.kept), (1, 2), "{r:?}");
    assert_eq!(uses_local.call(&session).unwrap(), 8);
    assert_eq!(caller.call(&session).unwrap(), 2);
    assert_eq!(r.retained_generations, 3);

    // Nada mudou: nada é compilado, e tudo continua respondendo.
    let r = session.hot_reload("app", &ir(2, 8)).expect("geração 4");
    assert_eq!((r.entries, r.kept), (0, 3), "{r:?}");
    assert_eq!(caller.call(&session).unwrap(), 2);
    assert_eq!(uses_local.call(&session).unwrap(), 8);
    // A 4 não implementa nada; ficam as que implementam alguma entrada.
    assert_eq!(r.retained_generations, 4);

    // Tudo volta a mudar: as gerações antigas saem.
    let r = session.hot_reload("app", &ir(3, 9).replace("%r = call", "%q = add i64 0, 0\n  %r = call")).expect("geração 5");
    assert_eq!((r.entries, r.kept), (3, 0), "{r:?}");
    assert_eq!(caller.call(&session).unwrap(), 3);
    assert_eq!(r.retained_generations, 1);
}
