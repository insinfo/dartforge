//! Execução pelo JIT e acordo com o executável AOT.
//!
//! O teste central é o diferencial: o **mesmo** LLVM IR, produzido uma única
//! vez pelo emissor nativo, executado pelos dois perfis — ORCv2 (pelo executor
//! isolado `dartforge-executar-ir`) e AOT (Clang + ligação com o runtime) —
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
struct Execucao {
    stdout: String,
    codigo: Option<i32>,
    stderr: String,
}

/// Limite de tempo de cada processo executado pelos testes.
///
/// Um programa que não termina não pode travar a bateria. No CI isso aconteceu
/// e o job só parou no limite de 60 min, sem dizer onde estava.
const LIMITE_PROCESSO: std::time::Duration = std::time::Duration::from_secs(120);

/// Executa um processo com limite de tempo e normaliza as quebras de linha.
///
/// stdout e stderr são lidos em threads próprias, para que um pipe cheio não
/// bloqueie o filho. Estourado o limite, o processo é morto e o teste falha
/// dizendo qual comando era.
fn executar(comando: &mut Command) -> Execucao {
    use std::io::Read;
    use std::process::Stdio;
    let descricao = format!("{comando:?}");
    let mut filho = comando
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("processo não iniciou");
    let mut out = filho.stdout.take().unwrap();
    let mut err = filho.stderr.take().unwrap();
    let leitor_out = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out.read_to_end(&mut b);
        b
    });
    let leitor_err = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err.read_to_end(&mut b);
        b
    });
    let inicio = std::time::Instant::now();
    let status = loop {
        if let Some(status) = filho.try_wait().unwrap() {
            break status;
        }
        if inicio.elapsed() > LIMITE_PROCESSO {
            let _ = filho.kill();
            let _ = filho.wait();
            panic!("{descricao} passou de {LIMITE_PROCESSO:?} e foi morto");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let stdout = leitor_out.join().unwrap();
    let stderr = leitor_err.join().unwrap();
    Execucao {
        stdout: String::from_utf8_lossy(&stdout).replace("\r\n", "\n"),
        codigo: status.code(),
        stderr: String::from_utf8_lossy(&stderr).replace("\r\n", "\n"),
    }
}

/// Registra em stderr a etapa do diferencial e o tempo desde o início.
///
/// Sem isso, um teste lento ou travado no CI não diz em que etapa está. A
/// escrita vai direto ao `stderr` do processo, e não por `eprintln!`, que o
/// harness de teste captura e só mostra se o teste falhar. Assim a linha
/// aparece no log enquanto o teste roda.
fn etapa(inicio: std::time::Instant, nome: &str) {
    use std::io::Write;
    let linha = format!("[diferencial {:>7.1?}] {nome}\n", inicio.elapsed());
    let _ = std::io::stderr().write_all(linha.as_bytes());
}

/// Executa um IR pelo executor isolado do JIT.
fn executar_pelo_jit(dir: &Path, ir: &str) -> Execucao {
    let arquivo = dir.join("programa.ll");
    std::fs::write(&arquivo, ir).unwrap();
    executar(&mut Command::new(env!("CARGO_BIN_EXE_dartforge-executar-ir")).arg(&arquivo))
}

// ─── Tabela de símbolos × emissor, sem LLVM ───────────────────────────────

/// Nomes `@dartforge_*` que o emissor nativo declara no IR.
///
/// Lidos do fonte do emissor (`emit_runtime_decls`), porque é lá que a lista
/// mora e porque este teste precisa rodar sem Clang nem programa Dart.
fn declaracoes_do_emissor() -> Vec<String> {
    let fonte = Path::new(env!("CARGO_MANIFEST_DIR")).join("../emit_native/src/llvm/mod.rs");
    let texto = std::fs::read_to_string(&fonte).unwrap();
    let mut nomes: Vec<String> = texto
        .lines()
        .filter(|linha| linha.contains("\"declare "))
        .filter_map(|linha| {
            let depois = &linha[linha.find("@dartforge_")? + 1..];
            Some(
                depois
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect(),
            )
        })
        .collect();
    nomes.sort();
    nomes.dedup();
    nomes
}

/// Todo símbolo que o emissor declara existe na tabela publicada pelo JIT.
///
/// A tabela é gerada da fonte do runtime; o emissor mantém a lista de
/// `declare` à parte. Quando os dois divergem, o AOT falha na ligação e o JIT
/// na pré-verificação — este teste denuncia a divergência antes, sem Clang.
#[test]
fn cada_declare_do_emissor_existe_no_runtime() {
    let declarados = declaracoes_do_emissor();
    assert!(declarados.len() > 100, "a leitura do emissor falhou: {declarados:?}");
    let faltando: Vec<&String> = declarados
        .iter()
        .filter(|nome| !RUNTIME_SYMBOLS.contains(&nome.as_str()))
        .collect();
    assert!(
        faltando.is_empty(),
        "o emissor declara símbolos que o runtime não define: {faltando:?}"
    );
}

// ─── Sessão em processo, IR escrito à mão ─────────────────────────────────

/// IR inválido vira `Result` com a etapa e o diagnóstico do LLVM, sem `panic`.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn ir_invalido_vira_result_com_mensagem() {
    let erro = dartforge_jit::run_ir("isto definitivamente não é LLVM IR").unwrap_err();
    assert_eq!(erro.stage, "parse-ir");
    assert!(erro.message.starts_with("IR inválido"), "{erro}");

    // Um IR sintaticamente válido mas sem a entrada falha no lookup, não antes.
    let mut sessao = JitSession::new().unwrap();
    sessao
        .add_ir_module("sem_entrada", "define i64 @sem_entrada() {\n  ret i64 0\n}\n")
        .unwrap();
    let erro = sessao.run_entry().unwrap_err();
    assert_eq!(erro.stage, "lookup");
    assert!(erro.message.contains("dartforge_entry"), "{erro}");
}

/// Externo que nem o runtime nem a CRT listada definem é recusado com o nome.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn externo_desconhecido_falha_alto() {
    let mut sessao = JitSession::new().unwrap();
    let erro = sessao
        .add_ir_module(
            "desconhecido",
            "declare void @sqlite3_open()\ndefine void @dartforge_entry() {\n  \
             call void @sqlite3_open()\n  ret void\n}\n",
        )
        .unwrap_err();
    assert_eq!(erro.stage, "símbolos");
    assert!(erro.message.contains("sqlite3_open"), "{erro}");
    assert!(sessao.module_names().is_empty());
}

/// Alvo divergente é recusado mostrando as duas strings; o do host passa.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn alvo_divergente_falha_alto() {
    let mut sessao = JitSession::new().unwrap();
    let erro = sessao
        .add_ir_module(
            "outro_alvo",
            "target triple = \"aarch64-unknown-linux-gnu\"\ndefine void @f() {\n  ret void\n}\n",
        )
        .unwrap_err();
    assert_eq!(erro.stage, "layout");
    assert!(erro.message.contains("aarch64-unknown-linux-gnu"), "{erro}");

    // O cabeçalho que o emissor nativo escreve (llvm/mod.rs, emit_header) tem
    // de ser exatamente o da LLJIT deste processo.
    let cabecalho = "target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                     target triple = \"x86_64-pc-windows-msvc\"\n";
    sessao
        .add_ir_module("alvo_do_emissor", &format!("{cabecalho}define void @g() {{\n  ret void\n}}\n"))
        .unwrap();
}

/// Dois módulos coexistem na sessão e um resolve símbolos do outro.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn dois_modulos_numa_sessao() {
    let biblioteca = "define i64 @jit_dobro(i64 %n) {\n  %r = mul i64 %n, 2\n  ret i64 %r\n}\n";
    let programa = "declare i64 @jit_dobro(i64)\ndefine void @dartforge_entry() {\n  \
                    %v = call i64 @jit_dobro(i64 21)\n  ret void\n}\n";
    let mut sessao = JitSession::new().unwrap();
    sessao.add_ir_module("biblioteca", biblioteca).unwrap();
    sessao.add_ir_module("programa", programa).unwrap();
    assert_eq!(sessao.module_names(), vec!["biblioteca", "programa"]);
    assert_ne!(sessao.lookup("jit_dobro").unwrap(), 0);
    let relatorio = sessao.run_entry().unwrap();
    assert_eq!(relatorio.exit_code, 0);
    assert!(relatorio.total >= relatorio.lookup);
}

/// Dois programas completos não cabem na mesma sessão: ambos definem a entrada.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn entrada_duplicada_vira_result() {
    let minimo = "define void @dartforge_entry() {\n  ret void\n}\n";
    let mut sessao = JitSession::new().unwrap();
    sessao.add_ir_module("primeiro", minimo).unwrap();
    let erro = sessao.add_ir_module("segundo", minimo).unwrap_err();
    assert_eq!(erro.stage, "add-module");
    assert!(erro.message.contains("dartforge_entry"), "{erro}");
    assert_eq!(sessao.module_names(), vec!["primeiro"]);
}

/// Remover o módulo descarrega seu código e a entrada deixa de ser resolvível.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn remover_modulo_descarrega_a_entrada() {
    let mut sessao = JitSession::new().unwrap();
    sessao
        .add_ir_module("programa", "define void @dartforge_entry() {\n  ret void\n}\n")
        .unwrap();
    assert_eq!(sessao.run_entry().unwrap().exit_code, 0);
    // Uma segunda execução começa de um runtime limpo e também termina bem.
    assert_eq!(sessao.run_entry().unwrap().exit_code, 0);
    sessao.remove_module(0).unwrap();
    assert!(sessao.module_names().is_empty());
    assert_eq!(sessao.run_entry().unwrap_err().stage, "lookup");
    assert_eq!(sessao.remove_module(0).unwrap_err().stage, "resource");
    assert_eq!(sessao.remove_module(7).unwrap_err().stage, "resource");
}

// ─── Executor isolado, IR escrito à mão ───────────────────────────────────

/// O executor imprime pelo runtime e devolve o código do programa.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn executor_imprime_pelo_runtime() {
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
    let execucao = executar_pelo_jit(&dir.0, ir);
    assert_eq!(execucao.stdout, "-42\ntrue\nola\n", "{execucao:?}");
    assert_eq!(execucao.codigo, Some(0), "{execucao:?}");
}

/// Exceção não capturada termina como no AOT: mensagem em stderr e código 101.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn executor_excecao_nao_capturada_sai_com_101() {
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
    let execucao = executar_pelo_jit(&dir.0, ir);
    assert_eq!(execucao.stdout, "1\n", "{execucao:?}");
    assert_eq!(execucao.codigo, Some(101), "{execucao:?}");
    assert!(execucao.stderr.starts_with("Uncaught exception: 7"), "{execucao:?}");
}

/// `double` e quadro maior que 4 KiB: o caminho COFF que o JIT antigo nunca
/// exercitou (`_fltused`, `__chkstk`, constantes de ponto flutuante).
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn executor_double_e_quadro_grande() {
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
    let execucao = executar_pelo_jit(&dir.0, ir);
    assert_eq!(execucao.stdout, "2.5\n0.1\n", "{execucao:?}");
    assert_eq!(execucao.codigo, Some(0), "{execucao:?}");
}

/// Falha do próprio JIT sai com 70 e a etapa em stderr, sem stdout.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn executor_falha_do_jit_sai_com_70() {
    let dir = OutputDir::new("falha");
    let execucao = executar_pelo_jit(&dir.0, "declare void @nada()\ndefine void @dartforge_entry() {\n  call void @nada()\n  ret void\n}\n");
    assert_eq!(execucao.codigo, Some(70), "{execucao:?}");
    assert!(execucao.stderr.contains("símbolos"), "{execucao:?}");
    assert!(execucao.stdout.is_empty());
}

// ─── Diferencial JIT × AOT sobre o IR do emissor nativo ───────────────────

/// Programa que o emissor de hoje executa certo: strings e uma função de topo.
///
/// É pequeno por uma razão medida em 2026-09-23. O diferencial só prova alguma
/// coisa sobre a saída **esperada** se o AOT a produz, e três formas mais ricas
/// ainda saíam erradas do emissor, nos dois perfis:
/// - um `for` cuja variável é reatribuída (`i = i + 1`): o IR recalcula `i + 1`
///   e descarta o resultado, então o laço não termina;
/// - uma chamada de método de instância (`c.dobro()`), baixada como `print(0)`;
/// - `print` de um `int` devolvido por função, que passa por
///   `dartforge_print_handle` e cai em `handle não vivo` (ESTADO §2.5).
///
/// Quando o emissor fechar essas formas, elas voltam para cá. Enquanto isso,
/// elas estão em [`PROGRAMAS_COM_DEFEITO`], onde se exige só que JIT e AOT
/// concordem.
const PROGRAMA: &str = "\
String saudacao(String nome) { return 'ola ' + nome; }
void main() {
  print('inicio');
  print(saudacao('mundo'));
}
";

/// Saída esperada, com quebras `\n` em qualquer sistema.
const ESPERADO: &str = "inicio\nola mundo\n";

/// Programas em que o emissor de hoje erra, nos dois perfis.
///
/// O contrato vale mesmo assim: o mesmo IR tem de dar o mesmo stdout e o mesmo
/// código de saída pelo JIT e pelo AOT, inclusive quando o runtime aborta.
/// Esses programas não podem travar, porque um laço infinito não prova
/// acordo; o que trava fica fora daqui e está descrito em [`PROGRAMA`].
const PROGRAMAS_COM_DEFEITO: &[&str] = &["\
int soma(int a, int b) { return a + b; }
void main() {
  print('antes');
  print(soma(40, 2));
}
"];

/// Emite o LLVM IR de um arquivo Dart pela trilha nova (`emitir_ir`), o mesmo
/// texto que o driver AOT entrega ao Clang.
fn emitir_ir(entrada: &Path) -> String {
    let opcoes = dartforge_emit_native::CompileOptions { sdk: None, packages: None, timings: false, optimize: false };
    dartforge_emit_native::emitir_ir(entrada, &opcoes)
        .unwrap_or_else(|erro| panic!("o programa de teste não emitiu IR: {erro}"))
        .texto
}

/// Emite o IR de `fonte` uma vez e executa esse mesmo texto pelos dois perfis.
fn executar_nos_dois_perfis(rotulo: &str, fonte: &str) -> (Execucao, Execucao) {
    let inicio = std::time::Instant::now();
    let dir = OutputDir::new(rotulo);
    let arquivo = dir.0.join("programa.dart");
    std::fs::write(&arquivo, fonte).unwrap();
    etapa(inicio, "emitir_ir");
    // Pilha grande como a CLI: o lowering é recursivo sobre a AST.
    let ir = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || emitir_ir(&arquivo))
        .unwrap()
        .join()
        .unwrap();
    etapa(inicio, &format!("IR com {} bytes; executar pelo JIT", ir.len()));

    let jit = executar_pelo_jit(&dir.0, &ir);
    etapa(inicio, &format!("JIT terminou ({:?}); compilar e ligar pelo AOT", jit.codigo));

    let executavel = dir.0.join("programa.exe");
    dartforge_emit_native::driver::compile_and_link(
        &ir,
        &executavel,
        &dartforge_emit_native::driver::NativeDriverOptions::default(),
    )
    .unwrap_or_else(|erro| panic!("o driver AOT falhou: {erro}"));
    etapa(inicio, "executar o AOT");
    let aot = executar(&mut Command::new(&executavel));
    etapa(inicio, &format!("AOT terminou ({:?})", aot.codigo));
    (jit, aot)
}

/// O mesmo IR produz a mesma saída e o mesmo código, e a saída é a esperada.
///
/// Contrato central: desenvolvimento e produção podem divergir em tempo de
/// compilação, nunca em resultado observável.
#[test]
#[ignore = "requer LLVM-C.dll no PATH, Clang (DARTFORGE_CLANG), rustc e o SDK Dart 3.6.2"]
fn jit_e_aot_concordam_no_mesmo_ir() {
    let (jit, aot) = executar_nos_dois_perfis("diferencial", PROGRAMA);
    assert_eq!(jit.stdout, aot.stdout, "JIT {jit:?}\nAOT {aot:?}");
    assert_eq!(jit.codigo, aot.codigo, "JIT {jit:?}\nAOT {aot:?}");
    assert_eq!(jit.stdout, ESPERADO, "os dois perfis concordam, mas o programa não fez o esperado: {jit:?}");
    assert_eq!(jit.codigo, Some(0), "{jit:?}");
}

/// Nos programas em que o emissor ainda erra, os dois perfis erram igual.
///
/// Só stdout e código são comparados. O stderr de um `panic` do runtime traz o
/// caminho do arquivo-fonte e a thread, que diferem entre a compilação avulsa
/// do AOT e a do crate.
#[test]
#[ignore = "requer LLVM-C.dll no PATH, Clang (DARTFORGE_CLANG), rustc e o SDK Dart 3.6.2"]
fn jit_e_aot_concordam_ate_no_defeito() {
    for (indice, fonte) in PROGRAMAS_COM_DEFEITO.iter().enumerate() {
        let (jit, aot) = executar_nos_dois_perfis(&format!("defeito-{indice}"), fonte);
        assert_eq!(jit.stdout, aot.stdout, "programa {indice}\nJIT {jit:?}\nAOT {aot:?}");
        assert_eq!(jit.codigo, aot.codigo, "programa {indice}\nJIT {jit:?}\nAOT {aot:?}");
    }
}
