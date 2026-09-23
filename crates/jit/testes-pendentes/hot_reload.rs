//! Suíte de testes para Hot Reload em `crates/jit`.
//!
//! Hot reload é a capacidade de atualizar funções e pontos de entrada em uma
//! sessão JIT em execução, sem reiniciar o processo hospedeiro e preservando
//! o estado do heap gerenciado.

use dartforge_jit::JitSession;

/// Emite o LLVM IR de um código Dart, falhando com diagnóstico descritivo.
fn compile_dart_to_ir(source: &str) -> String {
    dartforge_compiler::compile_llvm(source).unwrap_or_else(|err| {
        panic!("falha ao compilar Dart para LLVM IR: {err:?}\nCódigo:\n{source}")
    })
}

/// Teste 1: Atualização do corpo de uma função.
///
/// Módulo v1: `int getValue() => 1; void main() { print(getValue()); }`
/// Executa na sessão -> saída: "1\n"
/// Hot reload com Módulo v2: `int getValue() => 42; void main() { print(getValue()); }`
/// Executa na mesma sessão -> saída: "42\n"
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn function_body_update_preserves_session() {
    let source_v1 = "\
int getValue() => 1;
void main() {
  print(getValue());
}
";
    let source_v2 = "\
int getValue() => 42;
void main() {
  print(getValue());
}
";

    let ir_v1 = compile_dart_to_ir(source_v1);
    let ir_v2 = compile_dart_to_ir(source_v2);

    let mut session = JitSession::new().expect("falha ao inicializar JitSession");
    session
        .add_ir_module("app", &ir_v1)
        .expect("falha ao adicionar módulo inicial");

    let (out1, _) = session
        .run_entry_capturing()
        .expect("falha ao executar entrada v1");
    assert_eq!(out1, "1\n");

    // Executa hot reload substituindo o módulo 'app' pela versão v2
    let report = session
        .hot_reload("app", &ir_v2)
        .expect("falha no hot reload para v2");
    assert_eq!(report.ir_bytes, ir_v2.len());

    let (out2, _) = session
        .run_entry_capturing()
        .expect("falha ao executar entrada v2 após reload");
    assert_eq!(out2, "42\n");
}

/// Teste 1 (IR direto): Atualização do corpo de função usando LLVM IR explícito.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn function_body_update_direct_llvm_ir() {
    let ir_v1 = "\
declare void @dartforge_print_i64(i64)
define i64 @getValue() {
  ret i64 1
}
define void @dartforge_entry() {
  %v = call i64 @getValue()
  call void @dartforge_print_i64(i64 %v)
  ret void
}
";

    let ir_v2 = "\
declare void @dartforge_print_i64(i64)
define i64 @getValue() {
  ret i64 42
}
define void @dartforge_entry() {
  %v = call i64 @getValue()
  call void @dartforge_print_i64(i64 %v)
  ret void
}
";

    let mut session = JitSession::new().expect("falha ao inicializar JitSession");
    session
        .add_ir_module("app", ir_v1)
        .expect("falha ao adicionar v1");

    let (out1, _) = session.run_entry_capturing().expect("execução v1");
    assert_eq!(out1, "1\n");

    session.hot_reload("app", ir_v2).expect("hot reload v2");

    let (out2, _) = session.run_entry_capturing().expect("execução v2");
    assert_eq!(out2, "42\n");
}

/// Teste 2: Preservação de estado do heap gerenciado através do reload.
///
/// Cria instâncias de classes no heap gerenciado, invoca hot reload e verifica
/// que os contadores e alocações do heap persistem através do reload.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn state_preservation_across_hot_reload() {
    let source_v1 = "\
class Contador {
  int valor;
  Contador(this.valor);
  int dobro() { return valor * 2; }
}
void main() {
  Contador c1 = Contador(21);
  Contador c2 = Contador(42);
  print(c1.dobro());
  print(c2.valor);
}
";

    let source_v2 = "\
class Contador {
  int valor;
  Contador(this.valor);
  int triplo() { return valor * 3; }
}
void main() {
  Contador c3 = Contador(10);
  print(c3.triplo());
}
";

    let ir_v1 = compile_dart_to_ir(source_v1);
    let ir_v2 = compile_dart_to_ir(source_v2);

    let mut session = JitSession::new().expect("falha ao inicializar JitSession");
    session.add_ir_module("app", &ir_v1).expect("adiciona v1");

    let (out1, _) = session.run_entry_capturing().expect("executa v1");
    assert_eq!(out1, "42\n42\n");

    let stats_after_v1 = session.gc_stats();
    // Duas alocações de Contador c1 e c2
    assert!(
        stats_after_v1.allocations >= 2,
        "esperava pelo menos 2 alocações, obteve {}",
        stats_after_v1.allocations
    );

    // Invoca hot reload
    session.hot_reload("app", &ir_v2).expect("hot reload v2");

    // Verifica que antes de executar v2, o heap preservou exatamente o estado anterior
    let stats_mid = session.gc_stats();
    assert_eq!(
        stats_mid.allocations, stats_after_v1.allocations,
        "contadores do heap não devem ser resetados pelo hot reload"
    );

    // Executa a nova entrada v2
    let (out2, _) = session.run_entry_capturing().expect("executa v2");
    assert_eq!(out2, "30\n");

    // Verifica que o heap acumulou novas alocações sobre o estado persistido
    let stats_after_v2 = session.gc_stats();
    assert!(
        stats_after_v2.allocations > stats_after_v1.allocations,
        "novas alocações devem acumular no heap persistente"
    );
}

/// Teste 3: Segurança transacional (Transactional Safety).
///
/// Tenta recarregar com IR inválido (erro de sintaxe no LLVM IR).
/// Deve retornar `Err` na etapa `"parse-ir"` e a chamada subsequente a `run_entry`
/// deve continuar executando o código anterior em funcionamento, sem corrupção ou crash.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn transactional_safety_on_invalid_ir() {
    let source_v1 = "\
int getValue() => 1;
void main() {
  print(getValue());
}
";
    let ir_v1 = compile_dart_to_ir(source_v1);

    let mut session = JitSession::new().expect("falha ao inicializar JitSession");
    session.add_ir_module("app", &ir_v1).expect("adiciona v1");

    let (out1, _) = session.run_entry_capturing().expect("executa v1");
    assert_eq!(out1, "1\n");

    // Tentativa de hot reload com IR sintaticamente inválido
    let invalid_ir = "ESTE TEXTO NÃO É LLVM IR VÁLIDO! @@@ {{{ }}}";
    let err = session
        .hot_reload("app", invalid_ir)
        .expect_err("hot reload com IR inválido deveria falhar");

    assert_eq!(err.stage, "parse-ir");
    assert!(
        err.message.starts_with("IR inválido"),
        "mensagem inesperada: {}",
        err.message
    );

    // Sessão continua íntegra: código anterior executa normalmente
    let (out_fallback, _) = session
        .run_entry_capturing()
        .expect("código anterior deve continuar executando sem erros");
    assert_eq!(out_fallback, "1\n");

    // Após a falha transacional, um reload com IR válido deve funcionar perfeitamente
    let source_v2 = "\
int getValue() => 42;
void main() {
  print(getValue());
}
";
    let ir_v2 = compile_dart_to_ir(source_v2);
    session
        .hot_reload("app", &ir_v2)
        .expect("reload válido subsequente deve funcionar");

    let (out2, _) = session.run_entry_capturing().expect("executa v2");
    assert_eq!(out2, "42\n");
}

/// Teste 4: Múltiplos recarregamentos consecutivos sem vazamento de memória.
///
/// Executa 5 recarregamentos em sequência confirmando a atualização de comportamento
/// e a estabilidade da sessão JIT.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn multiple_consecutive_reloads_without_leaks() {
    let mut session = JitSession::new().expect("falha ao inicializar JitSession");

    let initial_ir = compile_dart_to_ir("int f() => 0; void main() { print(f()); }");
    session
        .add_ir_module("app", &initial_ir)
        .expect("adiciona módulo inicial");

    let (out0, _) = session.run_entry_capturing().expect("executa inicial");
    assert_eq!(out0, "0\n");

    for i in 1..=5 {
        let code = format!("int f() => {i}; void main() {{ print(f()); }}");
        let ir = compile_dart_to_ir(&code);

        let report = session
            .hot_reload("app", &ir)
            .unwrap_or_else(|e| panic!("falha no reload #{i}: {e}"));

        assert_eq!(report.ir_bytes, ir.len());

        let (out, _) = session
            .run_entry_capturing()
            .unwrap_or_else(|e| panic!("falha na execução #{i}: {e}"));

        assert_eq!(out, format!("{i}\n"));

        // Garante que apenas 1 módulo reside na lista ativa da sessão
        assert_eq!(session.module_names(), vec!["app"]);
    }
}

// ───────────────────────────────────────────────────────────────────────────────
// Hot reload de comportamento: identidade estável, implementação por geração.
//
// Os testes acima entram pelo caminho de **promoção**: o módulo é incorporado por
// `add_ir_module` e a primeira recarga instala os trampolins. Os de baixo usam
// `add_reloadable_module`, que já nasce recarregável e é o caminho recomendado do
// laço de desenvolvimento.
// ───────────────────────────────────────────────────────────────────────────────

/// Recarga básica: o mesmo símbolo, corpo novo, na mesma sessão.
///
/// `dartforge_entry` chama `teste()` **pelo trampolim**, não pelo corpo, então
/// trocar a implementação de `teste` muda o que a entrada imprime sem que a
/// entrada tenha de ser reescrita.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn recarga_basica_troca_o_corpo_na_mesma_sessao() {
    let ir_v1 = compile_dart_to_ir("int teste() => 1;\nvoid main() { print(teste()); }\n");
    let ir_v2 = compile_dart_to_ir("int teste() => 2;\nvoid main() { print(teste()); }\n");

    let mut sessao = JitSession::new().expect("sessão");
    let primeira = sessao
        .add_reloadable_module("app", &ir_v1)
        .expect("geração 1");
    assert_eq!(primeira.generation, 1);
    assert!(!primeira.promoted);
    assert_eq!(sessao.generation("app"), Some(1));

    let (saida, _) = sessao.run_entry_capturing().expect("execução da geração 1");
    assert_eq!(saida, "1\n");

    let segunda = sessao.hot_reload("app", &ir_v2).expect("geração 2");
    assert_eq!(segunda.generation, 2);
    assert_eq!(
        segunda.new_entries, 0,
        "nenhuma entrada nova: só o corpo mudou"
    );
    assert!(segunda.entries >= 2, "{segunda:?}");
    assert_eq!(segunda.retained_generations, 2);
    assert!(
        segunda.total >= segunda.parse_ir + segunda.add_module + segunda.link + segunda.publish,
        "o total tem de cobrir as etapas: {segunda:?}"
    );

    let (saida, _) = sessao.run_entry_capturing().expect("execução da geração 2");
    assert_eq!(saida, "2\n");

    // A identidade da sessão não mudou: um módulo, duas gerações.
    assert_eq!(sessao.module_names(), vec!["app"]);
    assert_eq!(sessao.retained_generations(), 2);
}

/// IR mínimo de um contador no heap gerenciado, com o incremento parametrizado.
///
/// `df_fn_0` cria o contador e devolve o handle; `df_fn_1` soma `passo` ao campo
/// 0 e devolve o novo valor. As duas assinaturas são estáveis entre as versões —
/// o que muda é só o corpo de `df_fn_1`, que é o caso aceito da versão 1.
fn ir_do_contador(passo: i64) -> String {
    format!(
        "\
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
"
    )
}

/// Estado preservado: o contador atravessa a recarga sem ser zerado.
///
/// É isto que separa hot reload de reiniciar o processo. O contador vive no heap
/// gerenciado da thread, que pertence à sessão e não ao módulo: trocar a geração
/// descarrega nada do heap, e o objeto continua no mesmo handle, com o mesmo
/// valor, exatamente como a Dart VM mantém as instâncias no lugar e troca só os
/// ponteiros de código.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn contador_vivo_sobrevive_a_recarga() {
    let mut sessao = JitSession::new().expect("sessão");
    sessao
        .add_reloadable_module("contador", &ir_do_contador(1))
        .expect("geração 1");

    let criar = sessao.stable_entry("df_fn_0").expect("entrada de criação");
    let somar = sessao
        .stable_entry("df_fn_1")
        .expect("entrada de incremento");
    assert_eq!(criar.signature(), "i64 ()");
    assert_eq!(somar.signature(), "i64 (i64)");

    let contador = criar.call(&sessao).expect("cria o contador");
    assert_eq!(somar.call_with(&sessao, contador).unwrap(), 1);
    assert_eq!(somar.call_with(&sessao, contador).unwrap(), 2);
    assert_eq!(somar.call_with(&sessao, contador).unwrap(), 3);

    let antes = sessao.gc_stats();
    // Passo novo: 10 em vez de 1. Mesma assinatura, corpo diferente.
    sessao
        .hot_reload("contador", &ir_do_contador(10))
        .expect("geração 2");
    let depois = sessao.gc_stats();
    assert_eq!(
        (antes.allocations, antes.live_objects),
        (depois.allocations, depois.live_objects),
        "a recarga não pode alocar nem liberar nada no heap gerenciado"
    );

    // 3 é o valor acumulado pela geração 1; se o heap tivesse sido zerado, o
    // resultado seria 10. O incremento de 10 prova que a geração nova é a que
    // executa; o 13 prova que o estado dela não é novo.
    assert_eq!(
        somar.call_with(&sessao, contador).unwrap(),
        13,
        "o contador foi zerado pela recarga"
    );
    assert_eq!(somar.call_with(&sessao, contador).unwrap(), 23);
}

/// Callback registrado antes da edição passa a executar a implementação nova.
///
/// O endereço é capturado antes da recarga e usado depois, sem ser resolvido de
/// novo — é o que uma aplicação faz quando guarda um ponteiro de função num
/// registro de callbacks. Ele continua correto porque aponta para o trampolim.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn callback_registrado_antes_da_edicao_executa_o_corpo_novo() {
    let ir = |valor: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {valor}\n}}\n");

    let mut sessao = JitSession::new().expect("sessão");
    sessao
        .add_reloadable_module("app", &ir(1))
        .expect("geração 1");

    // O «registro de callbacks» da aplicação: um endereço guardado de véspera.
    let callback = sessao.stable_entry("df_fn_0").expect("entrada estável");
    let endereco_antes = sessao.lookup("df_fn_0").expect("endereço da entrada");
    assert_eq!(callback.call(&sessao).unwrap(), 1);

    sessao.hot_reload("app", &ir(2)).expect("geração 2");
    assert_eq!(
        sessao.lookup("df_fn_0").expect("endereço da entrada"),
        endereco_antes,
        "a entrada estável não pode mudar de endereço entre gerações"
    );
    assert_eq!(
        callback.call(&sessao).unwrap(),
        2,
        "o callback antigo continuou no corpo antigo"
    );

    sessao.hot_reload("app", &ir(3)).expect("geração 3");
    assert_eq!(callback.call(&sessao).unwrap(), 3);
    assert_eq!(sessao.retained_generations(), 3);
}

/// Uma falha de recarga não destrói a versão que está funcionando.
///
/// Este é o requisito mais importante do trabalho, e são três falhas diferentes:
/// Dart que não compila (nem chega ao JIT), IR inválido (etapa `parse-ir`) e IR
/// válido que a sessão não sabe ligar (etapa `contract`). Depois das três, a
/// versão boa continua executando, e uma recarga válida ainda funciona.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn falha_de_recarga_nao_destroi_a_versao_boa() {
    let ir_bom = compile_dart_to_ir("int teste() => 7;\nvoid main() { print(teste()); }\n");

    let mut sessao = JitSession::new().expect("sessão");
    sessao
        .add_reloadable_module("app", &ir_bom)
        .expect("geração 1");
    let (saida, _) = sessao.run_entry_capturing().expect("execução boa");
    assert_eq!(saida, "7\n");

    // 1. Dart que não compila: a falha é do front-end e o JIT nem é chamado.
    let erro = dartforge_compiler::compile_llvm("int teste() => ;\nvoid main() {}\n")
        .expect_err("este Dart não deveria compilar");
    assert!(!format!("{erro:?}").is_empty());

    // 2. IR inválido.
    let erro = sessao
        .hot_reload("app", "ISTO NÃO É LLVM IR @@@ {{{")
        .expect_err("IR inválido tem de falhar");
    assert_eq!(erro.stage, "parse-ir");

    // 3. IR válido cujas referências a sessão não sabe resolver. Recusado antes
    //    de qualquer efeito, com o nome que falta na mensagem.
    let erro = sessao
        .hot_reload(
            "app",
            "declare i64 @sqlite3_open(i64)\n\
             define i64 @df_fn_0() {\n  %r = call i64 @sqlite3_open(i64 0)\n  ret i64 %r\n}\n\
             define void @dartforge_entry() {\n  ret void\n}\n",
        )
        .expect_err("referência externa desconhecida tem de falhar");
    assert_eq!(erro.stage, "contract");
    assert!(erro.message.contains("sqlite3_open"), "{erro}");

    // A versão boa continua exatamente onde estava, na geração 1.
    assert_eq!(sessao.generation("app"), Some(1));
    assert_eq!(sessao.retained_generations(), 1);
    let (saida, _) = sessao
        .run_entry_capturing()
        .expect("a versão boa tem de continuar executando");
    assert_eq!(saida, "7\n");

    // E a sessão continua recarregável: nada nela ficou pela metade.
    let ir_novo = compile_dart_to_ir("int teste() => 8;\nvoid main() { print(teste()); }\n");
    sessao
        .hot_reload("app", &ir_novo)
        .expect("recarga válida depois das falhas");
    let (saida, _) = sessao.run_entry_capturing().expect("execução da geração 2");
    assert_eq!(saida, "8\n");
}

/// Mudança de assinatura é recusada com diagnóstico, sem corromper a sessão.
///
/// A mensagem nomeia o símbolo e as duas assinaturas, porque quem lê precisa
/// saber **o que** ficou incompatível para decidir reiniciar.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn assinatura_alterada_e_recusada_com_diagnostico() {
    let mut sessao = JitSession::new().expect("sessão");
    sessao
        .add_reloadable_module("app", "define i64 @df_fn_0() {\n  ret i64 1\n}\n")
        .expect("geração 1");
    let entrada = sessao.stable_entry("df_fn_0").expect("entrada estável");
    assert_eq!(entrada.call(&sessao).unwrap(), 1);

    let erro = sessao
        .hot_reload("app", "define i64 @df_fn_0(i64 %a0) {\n  ret i64 %a0\n}\n")
        .expect_err("assinatura alterada tem de ser recusada");
    assert_eq!(erro.stage, "contract");
    assert_eq!(
        erro.message,
        "a assinatura de df_fn_0 mudou de i64 () para i64 (i64); \
         mudança de contrato de chamada exige reiniciar a sessão"
    );

    // Uma função que desaparece também é recusada, e pelo mesmo motivo: a
    // entrada estável dela ficaria presa no corpo antigo.
    let erro = sessao
        .hot_reload("app", "define i64 @df_fn_9() {\n  ret i64 1\n}\n")
        .expect_err("função ausente tem de ser recusada");
    assert_eq!(erro.stage, "contract");
    assert!(erro.message.contains("df_fn_0"), "{erro}");

    // Sessão intacta: mesma geração, mesma implementação, mesmo resultado.
    assert_eq!(sessao.generation("app"), Some(1));
    assert_eq!(entrada.call(&sessao).unwrap(), 1);
    sessao
        .hot_reload("app", "define i64 @df_fn_0() {\n  ret i64 5\n}\n")
        .expect("o corpo compatível ainda recarrega");
    assert_eq!(entrada.call(&sessao).unwrap(), 5);
}

/// Mudança no número de campos de uma classe é recusada antes de corromper o heap.
///
/// Os objetos já vivos mantêm o layout com que foram alocados; código novo lendo
/// um campo que não existe encontraria `panic` do runtime, ou pior, outro campo.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn layout_de_classe_alterado_e_recusado() {
    let ir = |campos: i64| {
        format!(
            "declare i64 @dartforge_object_new(i64, i64)\n\
             define i64 @df_new_0() {{\n  \
             %h = call i64 @dartforge_object_new(i64 0, i64 {campos})\n  ret i64 %h\n}}\n"
        )
    };
    let mut sessao = JitSession::new().expect("sessão");
    sessao
        .add_reloadable_module("app", &ir(2))
        .expect("geração 1");
    let erro = sessao
        .hot_reload("app", &ir(3))
        .expect_err("layout alterado tem de ser recusado");
    assert_eq!(erro.stage, "contract");
    assert!(
        erro.message
            .contains("a classe de id 0 tinha 2 campos e passou a ter 3"),
        "{erro}"
    );
    assert_eq!(sessao.generation("app"), Some(1));
}

/// Diretório exclusivo para o executável do lado AOT do diferencial.
struct DiretorioTemporario(std::path::PathBuf);
impl DiretorioTemporario {
    /// Reserva um diretório único mesmo sob execução simultânea de testes.
    fn novo(rotulo: &str) -> Self {
        let marca = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let caminho = std::env::temp_dir().join(format!(
            "dartforge-reload-{rotulo}-{}-{marca}",
            std::process::id()
        ));
        std::fs::create_dir(&caminho).unwrap();
        Self(caminho)
    }
}
impl Drop for DiretorioTemporario {
    /// Remove apenas o diretório que este teste criou.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Diferencial: depois da recarga, JIT e AOT concordam no mesmo programa.
///
/// O contrato dos dois perfis não pode ser afrouxado pelo hot reload. O programa
/// recarregado tem de produzir, na sessão viva, exatamente o que o executável AOT
/// do **mesmo Dart** produz — inclusive tendo passado pelos trampolins, que só
/// existem no JIT.
#[test]
#[ignore = "requer LLVM-C.dll no PATH e Clang/rustc nativos (DARTFORGE_CLANG/DARTFORGE_RUSTC); use scripts/env.ps1"]
fn jit_recarregado_e_aot_concordam() {
    const V1: &str = "\
class Contador {
  int valor;
  Contador(this.valor);
  int calcular() { return valor * 2; }
}
int extra() => 1;
void main() {
  Contador c = Contador(21);
  print(c.calcular());
  print(extra());
}
";
    const V2: &str = "\
class Contador {
  int valor;
  Contador(this.valor);
  int calcular() { return valor * 3 + 1; }
}
int extra() => 9;
void main() {
  Contador c = Contador(21);
  print(c.calcular());
  print(extra());
}
";
    let ir_v1 = compile_dart_to_ir(V1);
    let ir_v2 = compile_dart_to_ir(V2);

    let mut sessao = JitSession::new().expect("sessão");
    sessao.add_reloadable_module("app", &ir_v1).expect("v1");
    let (saida_v1, _) = sessao.run_entry_capturing().expect("execução v1");
    assert_eq!(saida_v1, "42\n1\n");
    sessao.hot_reload("app", &ir_v2).expect("v2");
    let (saida_jit, _) = sessao.run_entry_capturing().expect("execução v2");

    let dir = DiretorioTemporario::novo("diferencial");
    let executavel = dir.0.join(if cfg!(windows) {
        "programa.exe"
    } else {
        "programa"
    });
    dartforge_native::build_executable(
        &ir_v2,
        &executavel,
        &dartforge_native::NativeOptions::default(),
    )
    .unwrap_or_else(|erro| panic!("o driver AOT falhou: {erro}"));
    let resultado = std::process::Command::new(&executavel).output().unwrap();
    assert!(
        resultado.status.success(),
        "{}",
        String::from_utf8_lossy(&resultado.stderr)
    );
    let saida_aot = String::from_utf8(resultado.stdout)
        .unwrap()
        .replace("\r\n", "\n");

    assert_eq!(saida_jit, saida_aot, "JIT recarregado divergiu do AOT");
    assert_eq!(saida_jit, "64\n9\n");
}

// ───────────────────────────────────────────────────────────────────────────────
// Medição do ciclo de recarga.
// ───────────────────────────────────────────────────────────────────────────────

/// Programa Dart do corpus de medição, com o corpo parametrizado pela geração.
///
/// Tem classe, método, despacho, string e função de topo, para que a análise
/// tenha trabalho comparável ao de uma edição real, e não ao de um `ret i64`.
fn fonte_de_medicao(geracao: u64) -> String {
    format!(
        "\
class Caixa {{
  int valor;
  Caixa(this.valor);
  int dobro() {{ return valor * 2; }}
}}
String etiqueta(String nome) {{ return 'caixa ' + nome; }}
int base() => {geracao};
void main() {{
  Caixa c = Caixa(base());
  int total = 0;
  for (int i = 1; i <= 8; i = i + 1) {{
    total = total + i;
  }}
  print(total + c.dobro());
  print(etiqueta('um'));
}}
"
    )
}

/// Análise do Dart e geração do IR, cronometradas separadamente.
///
/// Repete o pipeline de `dartforge_compiler::compile_llvm_with_options` porque a
/// função pública devolve as duas fases somadas, e `docs/DESEMPENHO.md` proíbe
/// obter uma fase por subtração. O IR devolvido é comparado com o da função
/// pública no próprio teste; se o pipeline do compilador mudar, o teste acusa.
fn compilar_medindo(fonte: &str) -> (std::time::Duration, std::time::Duration, String) {
    let inicio = std::time::Instant::now();
    let tokens = dartforge_lexer::lex(fonte).expect("lex");
    let mut ast = dartforge_parser::parse(&tokens, fonte.len()).expect("parse");
    let expansao = dartforge_macros::expand(&mut ast, fonte.len()).expect("macros");
    dartforge_hir::expand_mixins(&mut ast).expect("mixins");
    let resolucao = dartforge_semantic::analyze(&ast).expect("semântica");
    let analise = inicio.elapsed();
    drop(expansao);

    let inicio = std::time::Instant::now();
    let ir = dartforge_llvm::emit(&dartforge_hir::lower_resolved(ast, resolucao)).expect("emissão");
    let emissao = inicio.elapsed();
    (analise, emissao, ir)
}

/// Mediana e p95 de uma amostra, em microssegundos, pela metodologia do projeto.
fn mediana_e_p95(amostras: &mut [std::time::Duration]) -> (f64, f64) {
    amostras.sort_unstable();
    let micros = |d: std::time::Duration| d.as_secs_f64() * 1e6;
    let mediana = micros(amostras[amostras.len() / 2]);
    let indice = ((amostras.len() as f64) * 0.95).ceil() as usize - 1;
    (mediana, micros(amostras[indice.min(amostras.len() - 1)]))
}

/// Mede o ciclo de recarga por etapa e imprime a tabela de `docs/JIT.md`.
///
/// Execute com `-- --ignored --nocapture`. A leitura importante não é o total: é
/// a proporção. `PLANO.md` insiste nisso porque um backend duas vezes mais rápido
/// na geração nativa não torna a recarga duas vezes mais rápida se a geração for
/// uma fração pequena do ciclo — a mesma aritmética do incremento 26.
#[test]
#[ignore = "medição; requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1 e --nocapture"]
fn medicao_do_ciclo_de_recarga() {
    // O pipeline montado à mão tem de produzir o mesmo IR da API pública.
    let (_, _, ir_montado) = compilar_medindo(&fonte_de_medicao(0));
    assert_eq!(
        ir_montado,
        compile_dart_to_ir(&fonte_de_medicao(0)),
        "o pipeline de medição divergiu de dartforge_compiler::compile_llvm"
    );

    const AQUECIMENTO: u64 = 3;
    const AMOSTRAS: u64 = 20;

    let mut sessao = JitSession::new().expect("sessão");
    sessao
        .add_reloadable_module("app", &ir_montado)
        .expect("geração 1");
    sessao.run_entry_capturing().expect("execução inicial");

    let mut analise = Vec::new();
    let mut emissao = Vec::new();
    let mut parse = Vec::new();
    let mut contrato = Vec::new();
    let mut adicao = Vec::new();
    let mut trampolins = Vec::new();
    let mut ligacao = Vec::new();
    let mut publicacao = Vec::new();
    let mut jit_total = Vec::new();
    let mut ciclo = Vec::new();
    let mut bytes = 0usize;

    for geracao in 1..=(AQUECIMENTO + AMOSTRAS) {
        let fonte = fonte_de_medicao(geracao);
        let inicio = std::time::Instant::now();
        let (tempo_analise, tempo_emissao, ir) = compilar_medindo(&fonte);
        let relatorio = sessao
            .hot_reload("app", &ir)
            .unwrap_or_else(|erro| panic!("recarga {geracao}: {erro}"));
        let total_ciclo = inicio.elapsed();
        let (saida, _) = sessao.run_entry_capturing().expect("execução após recarga");
        assert_eq!(saida, format!("{}\ncaixa um\n", 36 + 2 * geracao));
        if geracao <= AQUECIMENTO {
            continue;
        }
        analise.push(tempo_analise);
        emissao.push(tempo_emissao);
        parse.push(relatorio.parse_ir);
        contrato.push(relatorio.contract);
        adicao.push(relatorio.add_module);
        trampolins.push(relatorio.stubs);
        ligacao.push(relatorio.link);
        publicacao.push(relatorio.publish);
        jit_total.push(relatorio.total);
        ciclo.push(total_ciclo);
        bytes = relatorio.ir_bytes;
    }

    println!("\n# Ciclo de recarga, {AMOSTRAS} amostras após {AQUECIMENTO} aquecimentos");
    println!(
        "# IR de {bytes} bytes; gerações retidas ao final: {}",
        sessao.retained_generations()
    );
    println!("| Etapa | Mediana (µs) | p95 (µs) |");
    println!("| --- | --- | --- |");
    for (rotulo, amostras) in [
        ("análise do Dart novo", &mut analise),
        ("geração do IR", &mut emissao),
        ("análise do IR (parse-ir)", &mut parse),
        ("verificação de contrato", &mut contrato),
        ("adição do módulo ao JIT", &mut adicao),
        ("publicação dos trampolins", &mut trampolins),
        ("ligação em memória", &mut ligacao),
        ("publicação dos ponteiros", &mut publicacao),
        ("total do hot_reload", &mut jit_total),
        ("ciclo completo", &mut ciclo),
    ] {
        let (mediana, p95) = mediana_e_p95(amostras);
        println!("| {rotulo} | {mediana:.1} | {p95:.1} |");
    }
}
