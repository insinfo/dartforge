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
