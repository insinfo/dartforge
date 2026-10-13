//! Execução pelo JIT e acordo com o executável AOT.
//!
//! O teste central é o diferencial: o **mesmo** LLVM IR, produzido uma única
//! vez pelo emissor nativo, executado pelos dois perfis — ORCv2 (pelo executor
//! isolado `dartforge-execute_fn-ir`) e AOT (Clang + ligação com o runtime) —
//! precisa produzir o mesmo stdout e o mesmo código de saída. Tudo o mais
//! protege a fronteira com a API C do LLVM e a pré-verificação de externos.
//!
//! Execução por subprocesso não é detalhe: o runtime encerra o processo
//! (`process::exit`) nos mesmos casos em que o executável AOT termina, então um
//! programa com exceção não capturada derrubaria o processo de teste.
use dartforge_jit::{JitSession, RUNTIME_SYMBOLS};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Diretório exclusivo; o teste não remove caminhos fornecidos externamente.
struct OutputDir(PathBuf);
impl OutputDir {
    /// Reserva um diretório único mesmo em execuções simultâneas.
    fn new(label: &str) -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dartforge-jit-{label}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for OutputDir {
    /// Limpa somente o diretório criado pelo próprio teste.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Resultado observável de uma execução: o que os dois perfis têm de igualar.
#[derive(Debug, PartialEq, Eq)]
struct Execution {
    stdout: String,
    code: Option<i32>,
    stderr: String,
}

/// Limite de tempo de cada processo executado pelos testes.
///
/// Um programa que não termina não pode travar a bateria. No CI isso aconteceu
/// e o job só parou no limite de 60 min, sem dizer onde estava.
const PROCESS_LIMIT: std::time::Duration = std::time::Duration::from_secs(120);

/// Executa um processo com limite de tempo e normaliza as quebras de linha.
///
/// stdout e stderr são lidos em threads próprias, para que um pipe cheio não
/// bloqueie o filho. Estourado o limite, o processo é morto e o teste falha
/// dizendo qual comando era.
fn execute_fn(command: &mut Command) -> Execution {
    use std::io::Read;
    use std::process::Stdio;
    let description = format!("{command:?}");
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("processo não iniciou");
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let out_reader = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out.read_to_end(&mut b);
        b
    });
    let err_reader = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err.read_to_end(&mut b);
        b
    });
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > PROCESS_LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{description} passou de {PROCESS_LIMIT:?} e foi morto");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let stdout = out_reader.join().unwrap();
    let stderr = err_reader.join().unwrap();
    Execution {
        stdout: String::from_utf8_lossy(&stdout).replace("\r\n", "\n"),
        code: status.code(),
        stderr: String::from_utf8_lossy(&stderr).replace("\r\n", "\n"),
    }
}

/// Registra em stderr a etapa do diferencial e o tempo desde o início.
///
/// Sem isso, um teste lento ou travado no CI não diz em que etapa está. A
/// escrita vai direto ao `stderr` do processo, e não por `eprintln!`, que o
/// harness de teste captura e só mostra se o teste falhar. Assim a linha
/// aparece no log enquanto o teste roda.
fn stage(start: std::time::Instant, nome: &str) {
    use std::io::Write;
    let line = format!("[diferencial {:>7.1?}] {nome}\n", start.elapsed());
    let _ = std::io::stderr().write_all(line.as_bytes());
}

/// Executa um IR pelo executor isolado do JIT.
fn run_through_jit(dir: &Path, ir: &str) -> Execution {
    let file = dir.join("programa.ll");
    std::fs::write(&file, ir).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge-executar-ir"));
    command.arg(&file);
    // O IR com o SDK da fonte (o padrão) importa o runtime e o SDK da
    // biblioteca compartilhada, que o executor recebe pelo ambiente, como o
    // `dartforge run` a acha no cache.
    if dartforge_jit::ir_usa_sdk_da_fonte(ir) && std::env::var_os("DARTFORGE_SDK_DLL").is_none() {
        let dll = dartforge_emit_native::sdk_modulo::dll_do_sdk_da_fonte().expect("biblioteca do SDK da fonte");
        command.env("DARTFORGE_SDK_DLL", dll);
    }
    execute_fn(&mut command)
}

// ─── Tabela de símbolos × emissor, sem LLVM ───────────────────────────────

/// Todo símbolo que o emissor declara existe na tabela publicada pelo JIT.
///
/// A lista de externs do emissor é `dartforge_emit_native::llvm::externs::EXTERNS`
/// (a tabela de efeitos, fonte única do lado do emissor). A tabela do JIT é
/// gerada da fonte do runtime. Quando as duas divergem, o AOT falha na ligação
/// e o JIT na pré-verificação; este teste denuncia antes, sem LLVM nem Clang.
#[test]
fn every_emitter_extern_exists_in_runtime() {
    let declared_set: Vec<&str> = dartforge_emit_native::llvm::externs::EXTERNS.iter().map(|e| e.nome()).collect();
    assert!(declared_set.len() > 100, "tabela de externs inesperada: {declared_set:?}");
    let missing_list: Vec<&&str> = declared_set.iter().filter(|nome| !RUNTIME_SYMBOLS.contains(nome)).collect();
    assert!(missing_list.is_empty(), "o emissor declara externs que o runtime não define: {missing_list:?}");
}

// ─── Sessão em processo, IR escrito à mão ─────────────────────────────────

/// IR inválido vira `Result` com a etapa e o diagnóstico do LLVM, sem `panic`.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn invalid_ir_becomes_result_with_message() {
    let error = dartforge_jit::run_ir("isto definitivamente não é LLVM IR").unwrap_err();
    assert_eq!(error.stage, "parse-ir");
    assert!(error.message.starts_with("IR inválido"), "{error}");

    // Um IR sintaticamente válido mas sem a entrada falha no lookup, não antes.
    let mut session = JitSession::new().unwrap();
    session
        .add_ir_module("sem_entrada", "define i64 @sem_entrada() {\n  ret i64 0\n}\n")
        .unwrap();
    let error = session.run_entry().unwrap_err();
    assert_eq!(error.stage, "lookup");
    assert!(error.message.contains("dartforge_entry"), "{error}");
}

/// IR que o analisador aceita mas o verificador recusa (valor que não domina o
/// uso) é recusado pelo JIT, como o Clang recusa no AOT.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn ir_rejected_by_verifier_does_not_run() {
    let ir = "define void @dartforge_entry() {\nentrada:\n  br i1 true, label %a, label %b\n\
              a:\n  %x = add i64 1, 2\n  br label %b\nb:\n  %y = add i64 %x, 1\n  ret void\n}\n";
    let error = dartforge_jit::run_ir(ir).unwrap_err();
    assert_eq!(error.stage, "parse-ir");
    assert!(error.message.contains("verificador"), "{error}");
}

/// Externo que nem o runtime nem a CRT listada definem é recusado com o nome.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn unknown_external_fails_loudly() {
    let mut session = JitSession::new().unwrap();
    let error = session
        .add_ir_module(
            "desconhecido",
            "declare void @sqlite3_open()\ndefine void @dartforge_entry() {\n  \
             call void @sqlite3_open()\n  ret void\n}\n",
        )
        .unwrap_err();
    assert_eq!(error.stage, "símbolos");
    assert!(error.message.contains("sqlite3_open"), "{error}");
    assert!(session.module_names().is_empty());
}

/// Alvo divergente é recusado mostrando as duas strings; o do host passa.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn divergent_target_fails_loudly() {
    let mut session = JitSession::new().unwrap();
    // Um triple que nunca é o do hospedeiro.
    let other_one = "riscv64-unknown-linux-gnu";
    let error = session
        .add_ir_module("outro_alvo", &format!("target triple = \"{other_one}\"\ndefine void @f() {{\n  ret void\n}}\n"))
        .unwrap_err();
    assert_eq!(error.stage, "layout");
    assert!(error.message.contains(other_one), "{error}");

    // O cabeçalho que o emissor nativo escreve (`alvo::cabecalho_ir`) tem de
    // ser compatível com o da LLJIT deste processo: mesma arquitetura,
    // sistema e ABI, ainda que o LLVM instalado grafe o triple de outro
    // jeito (`x86_64-pc-linux-gnu`, `arm64-apple-macosx15.7.3`).
    let header = dartforge_emit_native::alvo::cabecalho_ir();
    session
        .add_ir_module("alvo_do_emissor", &format!("{header}define void @g() {{\n  ret void\n}}\n"))
        .unwrap();
}

/// Dois módulos coexistem na sessão e um resolve símbolos do outro.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn two_modules_in_one_session() {
    let library = "define i64 @jit_dobro(i64 %n) {\n  %r = mul i64 %n, 2\n  ret i64 %r\n}\n";
    let program = "declare i64 @jit_dobro(i64)\ndefine void @dartforge_entry() {\n  \
                    %v = call i64 @jit_dobro(i64 21)\n  ret void\n}\n";
    let mut session = JitSession::new().unwrap();
    session.add_ir_module("biblioteca", library).unwrap();
    session.add_ir_module("programa", program).unwrap();
    assert_eq!(session.module_names(), vec!["biblioteca", "programa"]);
    assert_ne!(session.lookup("jit_dobro").unwrap(), 0);
    let report_data = session.run_entry().unwrap();
    assert_eq!(report_data.exit_code, 0);
    assert!(report_data.total >= report_data.lookup);
}

/// Dois programas completos não cabem na mesma sessão: ambos definem a entrada.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn duplicate_entry_becomes_result() {
    let min = "define void @dartforge_entry() {\n  ret void\n}\n";
    let mut session = JitSession::new().unwrap();
    session.add_ir_module("primeiro", min).unwrap();
    let error = session.add_ir_module("segundo", min).unwrap_err();
    assert_eq!(error.stage, "add-module");
    assert!(error.message.contains("dartforge_entry"), "{error}");
    assert_eq!(session.module_names(), vec!["primeiro"]);
}

/// Remover o módulo descarrega seu código e a entrada deixa de ser resolvível.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn removing_module_unloads_entry() {
    let mut session = JitSession::new().unwrap();
    session
        .add_ir_module("programa", "define void @dartforge_entry() {\n  ret void\n}\n")
        .unwrap();
    assert_eq!(session.run_entry().unwrap().exit_code, 0);
    // Uma segunda execução começa de um runtime limpo e também termina bem.
    assert_eq!(session.run_entry().unwrap().exit_code, 0);
    session.remove_module(0).unwrap();
    assert!(session.module_names().is_empty());
    assert_eq!(session.run_entry().unwrap_err().stage, "lookup");
    assert_eq!(session.remove_module(0).unwrap_err().stage, "resource");
    assert_eq!(session.remove_module(7).unwrap_err().stage, "resource");
}

// ─── Executor isolado, IR escrito à mão ───────────────────────────────────

/// O executor imprime pelo runtime e devolve o código do programa.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn runner_prints_through_runtime() {
    let dir = OutputDir::new("executor");
    let ir = "\
@.s = private unnamed_addr constant [3 x i8] c\"ola\"
declare void @dartforge_print_i64(i64)
declare void @dartforge_print_bool(i8)
declare i64 @dartforge_string_new(ptr, i64)
declare void @dartforge_print_string(i64)
define void @dartforge_entry() {
  call void @dartforge_print_i64(i64 -42)
  call void @dartforge_print_bool(i8 1)
  %s = call i64 @dartforge_string_new(ptr @.s, i64 3)
  call void @dartforge_print_string(i64 %s)
  ret void
}
";
    let execution = run_through_jit(&dir.0, ir);
    assert_eq!(execution.stdout, "-42\ntrue\nola\n", "{execution:?}");
    assert_eq!(execution.code, Some(0), "{execution:?}");
}

/// Exceção não capturada termina como no AOT e na VM: `Unhandled exception:`
/// e a mensagem em stderr, código 255.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn runner_uncaught_exception_exits_with_255() {
    let dir = OutputDir::new("excecao");
    let ir = "\
declare void @dartforge_print_i64(i64)
declare void @dartforge_exception_throw(i64, i8)
define void @dartforge_entry() {
  call void @dartforge_print_i64(i64 1)
  call void @dartforge_exception_throw(i64 7, i8 1)
  ret void
}
";
    let execution = run_through_jit(&dir.0, ir);
    assert_eq!(execution.stdout, "1\n", "{execution:?}");
    assert_eq!(execution.code, Some(255), "{execution:?}");
    assert!(execution.stderr.starts_with("Unhandled exception:\n7\n"), "{execution:?}");
}

/// `double` e quadro maior que 4 KiB: o caminho COFF que o JIT antigo nunca
/// exercitou (`_fltused`, `__chkstk`, constantes de ponto flutuante).
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn runner_double_and_large_frame() {
    let dir = OutputDir::new("coff");
    let ir = "\
declare void @dartforge_print_f64(double)
define double @soma(double %a, double %b) {
  %buf = alloca [16384 x i8]
  store volatile i8 1, ptr %buf
  %r = fadd double %a, %b
  ret double %r
}
define void @dartforge_entry() {
  %v = call double @soma(double 1.25, double 1.25)
  call void @dartforge_print_f64(double %v)
  call void @dartforge_print_f64(double 0.1)
  ret void
}
";
    let execution = run_through_jit(&dir.0, ir);
    assert_eq!(execution.stdout, "2.5\n0.1\n", "{execution:?}");
    assert_eq!(execution.code, Some(0), "{execution:?}");
}

/// Falha do próprio JIT sai com 70 e a etapa em stderr, sem stdout.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn runner_jit_failure_exits_with_70() {
    let dir = OutputDir::new("falha");
    let execution = run_through_jit(&dir.0, "declare void @nada()\ndefine void @dartforge_entry() {\n  call void @nada()\n  ret void\n}\n");
    assert_eq!(execution.code, Some(70), "{execution:?}");
    assert!(execution.stderr.contains("símbolos"), "{execution:?}");
    assert!(execution.stdout.is_empty());
}

// ─── Diferencial JIT × AOT sobre o IR do emissor nativo ───────────────────

/// Programas da trilha nova com a saída que a VM dá, cada um passando pelos
/// dois perfis a partir do mesmo IR.
///
/// Os três primeiros são as formas que o emissor errava em 2026-09-23 (achadas
/// por este diferencial): `for` com a variável reatribuída, método de
/// instância e `print` de `int` devolvido por função. Foram corrigidas com o
/// contrato de representação do nativo (`crates/emit_native/tests/contrato.rs`)
/// e ficam aqui como regressão dos dois perfis.
const PROGRAMS: &[(&str, &str, &str)] = &[
    (
        "for",
        "\
void main() {
  int total = 0;
  for (int i = 1; i <= 4; i = i + 1) {
    total = total + i;
  }
  print(total);
  print(total > 5);
}
",
        "10\ntrue\n",
    ),
    (
        "metodo",
        "\
class Contador {
  int valor;
  Contador(this.valor);
  int dobro() { return valor * 2; }
}
void main() {
  Contador c = Contador(21);
  print(c.dobro());
}
",
        "42\n",
    ),
    (
        "retorno_int",
        "\
int soma(int a, int b) { return a + b; }
int maior(int a, int b) {
  if (a > b) {
    return a;
  } else {
    return b;
  }
}
void main() {
  print(soma(40, 2));
  print(soma(1, 2) > 2);
  print(maior(3, 9));
}
",
        "42\ntrue\n9\n",
    ),
    (
        "strings",
        "\
String saudacao(String nome) { return 'ola ' + nome; }
void main() {
  print('inicio');
  print(saudacao('mundo'));
}
",
        "inicio\nola mundo\n",
    ),
];

/// Emite o LLVM IR de um arquivo Dart como o JIT o recebe
/// (`emitir_ir_recarregavel`: sem objetos estáticos, que o JIT não tem,
/// docs/NATIVO-ESPACO-UNIFICADO.md §2.11); o driver AOT liga esse mesmo texto.
fn emitir_ir(input: &Path) -> String {
    let options = dartforge_emit_native::CompileOptions { sdk: None, packages: None, timings: false, optimize: false, versao_linguagem: None, experimentos: Vec::new(), depuracao: false, gerador: None, cpu: None };
    dartforge_emit_native::emitir_ir_recarregavel(input, &options, None)
        .unwrap_or_else(|error| panic!("o programa de teste não emitiu IR: {error}"))
        .texto
}

/// Emite o IR de `source` uma vez e executa esse mesmo texto pelos dois perfis.
fn run_in_both_profiles(label_name: &str, source: &str) -> (Execution, Execution) {
    let start = std::time::Instant::now();
    let dir = OutputDir::new(label_name);
    let file = dir.0.join("programa.dart");
    std::fs::write(&file, source).unwrap();
    stage(start, "emitir_ir");
    // Pilha grande como a CLI: o lowering é recursivo sobre a AST.
    let ir = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || emitir_ir(&file))
        .unwrap()
        .join()
        .unwrap();
    stage(start, &format!("IR com {} bytes; executar pelo JIT", ir.len()));

    let jit = run_through_jit(&dir.0, &ir);
    stage(start, &format!("JIT terminou ({:?}); compilar e ligar pelo AOT", jit.code));

    let executable = dir.0.join("programa.exe");
    dartforge_emit_native::driver::compile_and_link(
        &ir,
        &executable,
        &dartforge_emit_native::driver::NativeDriverOptions::default(),
    )
    .unwrap_or_else(|error| panic!("o driver AOT falhou: {error}"));
    stage(start, "executar o AOT");
    let aot = execute_fn(&mut Command::new(&executable));
    stage(start, &format!("AOT terminou ({:?})", aot.code));
    (jit, aot)
}

/// O mesmo IR produz a mesma saída e o mesmo código nos dois perfis, e a
/// saída é a da VM.
///
/// Contrato central: desenvolvimento e produção podem divergir em tempo de
/// compilação, nunca em resultado observável. A divergência entre os perfis
/// num programa que falha é coberta em escala pelo job `jit` do CI
/// (`dartforge-differential --jit-aot`, corpus inteiro).
#[test]
#[ignore = "requer LLVM-C.dll no PATH, Clang (DARTFORGE_CLANG), rustc e o SDK Dart 3.6.2"]
fn jit_and_aot_agree_on_same_ir() {
    for (label_name, source, expected) in PROGRAMS {
        let (jit, aot) = run_in_both_profiles(label_name, source);
        assert_eq!(jit.stdout, aot.stdout, "{label_name}\nJIT {jit:?}\nAOT {aot:?}");
        assert_eq!(jit.code, aot.code, "{label_name}\nJIT {jit:?}\nAOT {aot:?}");
        assert_eq!(&jit.stdout, expected, "{label_name}: os dois perfis concordam, mas não com a VM: {jit:?}");
        assert_eq!(jit.code, Some(0), "{label_name}: {jit:?}");
    }
}

/// Casos consecutivos que rotulam o mesmo corpo devem entregar as ligações
/// do padrão vencedor, inclusive quando o corpo captura a variável.
#[test]
#[ignore = "requer LLVM-C.dll no PATH, Clang (DARTFORGE_CLANG), rustc e o SDK Dart 3.6.2"]
fn shared_cases_deliver_variables_to_body() {
    let source = r#"
sealed class Expr {}
class A extends Expr { final int x; A(this.x); }
class B extends Expr { final int x; B(this.x); }
int Function() ler(Expr e) {
  switch (e) {
    case A(:final x):
    case B(:final x):
      return () => x;
  }
}
void main() {
  print(ler(A(3))());
  print(ler(B(7))());
}
"#;
    let (jit, aot) = run_in_both_profiles("cases-compartilhados", source);
    assert_eq!(jit.stdout, "3\n7\n", "{jit:?}");
    assert_eq!(jit.stdout, aot.stdout, "JIT {jit:?}\nAOT {aot:?}");
    assert_eq!(jit.code, Some(0), "{jit:?}");
    assert_eq!(jit.code, aot.code, "JIT {jit:?}\nAOT {aot:?}");
}
