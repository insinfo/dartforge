//! Execução em memória pelo JIT e acordo com o executável AOT.
//!
//! O teste central deste arquivo é o diferencial: o mesmo LLVM IR, executado
//! pelos dois perfis, precisa produzir exatamente a mesma saída. Tudo o mais —
//! erro de IR, dois módulos na sessão, remoção de módulo — protege esse
//! contrato contra regressões na fronteira com a API C do LLVM.
use dartforge_jit::{JitSession, run_ir_capturing};
use std::path::PathBuf;

/// Programa Dart do subconjunto nativo que exercita escalares, objetos e strings.
///
/// Passa por `dartforge_print_i64`, `dartforge_print_bool`,
/// `dartforge_print_string`, os frames do coletor e as operações de objeto, ou
/// seja, cobre a maior parte dos símbolos que a sessão precisa publicar.
const PROGRAMA: &str = "\
class Contador {
  int valor;
  Contador(this.valor);
  int dobro() { return valor * 2; }
}
String saudacao(String nome) { return 'ola ' + nome; }
void main() {
  int total = 0;
  for (int i = 1; i <= 4; i = i + 1) {
    total = total + i;
  }
  print(total);
  print(total > 5);
  Contador c = Contador(21);
  print(c.dobro());
  print(saudacao('mundo'));
}
";

/// Saída esperada, com quebras `\n` em qualquer sistema.
const ESPERADO: &str = "10\ntrue\n42\nola mundo\n";

/// Emite o IR nativo do programa de teste, falhando com o diagnóstico completo.
fn ir_do_programa() -> String {
    dartforge_compiler::compile_llvm(PROGRAMA)
        .unwrap_or_else(|error| panic!("o programa de teste saiu do subconjunto nativo: {error:?}"))
}

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

/// Um programa Dart simples compila e executa inteiramente em memória.
#[test]
fn dart_program_runs_entirely_in_memory() {
    let ir = ir_do_programa();
    let (saida, relatorio) = run_ir_capturing(&ir).unwrap();
    assert_eq!(saida, ESPERADO);
    assert_eq!(relatorio.module.ir_bytes, ir.len());
    assert!(relatorio.module.total >= relatorio.module.parse_ir + relatorio.module.add_module);
    assert!(relatorio.entry.total >= relatorio.entry.lookup + relatorio.entry.execute);
    assert!(
        relatorio.total >= relatorio.session + relatorio.module.total + relatorio.entry.total,
        "{relatorio:?}"
    );
}

/// O mesmo IR precisa produzir a mesma saída pelo JIT e pelo executável AOT.
///
/// Este é o contrato central dos dois perfis: desenvolvimento e produção podem
/// divergir em tempo de compilação, nunca em resultado observável.
#[test]
#[ignore = "requer Clang e rustc nativos no PATH ou DARTFORGE_CLANG/DARTFORGE_RUSTC"]
fn jit_and_aot_agree_on_the_same_ir() {
    let ir = ir_do_programa();
    let (saida_jit, _) = run_ir_capturing(&ir).unwrap();

    let dir = OutputDir::new("diferencial");
    let executavel = dir.0.join(if cfg!(windows) {
        "programa.exe"
    } else {
        "programa"
    });
    dartforge_native::build_executable(
        &ir,
        &executavel,
        &dartforge_native::NativeOptions::default(),
    )
    .unwrap_or_else(|error| panic!("o driver AOT falhou: {error}"));
    let resultado = std::process::Command::new(&executavel).output().unwrap();
    assert!(
        resultado.status.success(),
        "{}",
        String::from_utf8_lossy(&resultado.stderr)
    );
    let saida_aot = String::from_utf8(resultado.stdout)
        .unwrap()
        .replace("\r\n", "\n");

    assert_eq!(saida_jit, saida_aot);
    assert_eq!(saida_jit, ESPERADO);
}

/// IR inválido vira `Result` com a etapa e o diagnóstico do LLVM, sem `panic`.
#[test]
fn invalid_ir_becomes_a_result_with_a_message() {
    let erro = run_ir_capturing("isto definitivamente não é LLVM IR").unwrap_err();
    assert_eq!(erro.stage, "parse-ir");
    assert!(erro.message.starts_with("IR inválido"), "{erro}");
    assert!(erro.to_string().starts_with("parse-ir: IR inválido"));

    // Um IR sintaticamente válido mas sem a entrada falha no lookup, não antes.
    let mut sessao = JitSession::new().unwrap();
    sessao
        .add_ir_module(
            "sem_entrada",
            "define i64 @sem_entrada() {\n  ret i64 0\n}\n",
        )
        .unwrap();
    let erro = sessao.run_entry().unwrap_err();
    assert_eq!(erro.stage, "lookup");
    assert!(erro.message.contains("dartforge_entry"), "{erro}");
}

/// Dois módulos coexistem na sessão e um resolve símbolos do outro.
#[test]
fn two_modules_share_one_session() {
    let biblioteca = "\
define i64 @jit_dobro(i64 %n) {
  %r = mul i64 %n, 2
  ret i64 %r
}
";
    let programa = "\
declare void @dartforge_print_i64(i64)
declare i64 @jit_dobro(i64)
define void @dartforge_entry() {
  %v = call i64 @jit_dobro(i64 21)
  call void @dartforge_print_i64(i64 %v)
  ret void
}
";
    let mut sessao = JitSession::new().unwrap();
    sessao.add_ir_module("biblioteca", biblioteca).unwrap();
    sessao.add_ir_module("programa", programa).unwrap();
    assert_eq!(sessao.module_names(), vec!["biblioteca", "programa"]);
    assert_ne!(sessao.lookup("jit_dobro").unwrap(), 0);

    let (saida, relatorio) = sessao.run_entry_capturing().unwrap();
    assert_eq!(saida, "42\n");
    assert!(relatorio.total >= relatorio.lookup);
}

/// Dois programas Dart completos não cabem na mesma sessão: ambos definem a entrada.
#[test]
fn duplicate_entry_definition_becomes_a_result() {
    let ir = ir_do_programa();
    let mut sessao = JitSession::new().unwrap();
    sessao.add_ir_module("primeiro", &ir).unwrap();
    let erro = sessao.add_ir_module("segundo", &ir).unwrap_err();
    assert_eq!(erro.stage, "add-module");
    assert!(erro.message.contains("dartforge_entry"), "{erro}");
    assert_eq!(sessao.module_names(), vec!["primeiro"]);
}

/// Remover o módulo descarrega seu código e a entrada deixa de ser resolvível.
///
/// A remoção só acontece aqui porque nada daquele módulo está executando: é
/// exatamente a pré-condição documentada em `JitSession::remove_module`.
#[test]
fn removing_a_module_unloads_its_entry() {
    let ir = ir_do_programa();
    let mut sessao = JitSession::new().unwrap();
    sessao.add_ir_module("programa", &ir).unwrap();
    let (saida, _) = sessao.run_entry_capturing().unwrap();
    assert_eq!(saida, ESPERADO);

    sessao.remove_module(0).unwrap();
    assert!(sessao.module_names().is_empty());
    let erro = sessao.run_entry().unwrap_err();
    assert_eq!(erro.stage, "lookup");

    let erro = sessao.remove_module(0).unwrap_err();
    assert_eq!(erro.stage, "resource");
    let erro = sessao.remove_module(7).unwrap_err();
    assert_eq!(erro.stage, "resource");
}

/// A sessão reaproveitada executa a mesma entrada mais de uma vez.
#[test]
fn one_session_runs_the_entry_twice() {
    let ir = ir_do_programa();
    let mut sessao = JitSession::new().unwrap();
    sessao.add_ir_module("programa", &ir).unwrap();
    let (saida, primeira) = sessao.run_entry_capturing().unwrap();
    assert_eq!(saida, ESPERADO);
    let (saida, segunda) = sessao.run_entry_capturing().unwrap();
    assert_eq!(saida, ESPERADO);
    // A geração de código acontece uma vez; a segunda resolução já encontra o
    // símbolo materializado. A comparação é entre as duas resoluções da mesma
    // sessão, não contra um limite absoluto, para não depender da máquina.
    assert!(segunda.lookup <= primeira.lookup);
    assert!(sessao.gc_stats().allocations > 0);
}
