//! Experimento medido: Cranelift JIT contra o caminho LLVM já existente.
//!
//! Executa a metodologia de `docs/DESEMPENHO.md` — aquecimento, amostras,
//! mediana e p95 — sobre o mesmo conjunto de programas nos dois caminhos, e
//! imprime as tabelas prontas para `docs/CRANELIFT.md`.
//!
//! Uso:
//!
//! ```text
//! cargo run --release --example experimento -p dartforge-cranelift-jit
//! ```
//!
//! A parte AOT exige Clang e rustc: defina `DARTFORGE_CLANG` (e opcionalmente
//! `DARTFORGE_RUSTC`) ou tenha ambos no PATH. Sem eles o experimento imprime
//! apenas o lado JIT e diz que o lado AOT foi omitido, em vez de inventar
//! números. A máquina pode estar ocupada com outras compilações; a saída
//! sempre traz os contadores que não dependem da carga.
use dartforge_cranelift_jit::Otimizacao;
use dartforge_diagnostics::Diagnostic;
use dartforge_hir::Module;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Amostras cronometradas por medida, depois do aquecimento.
const AMOSTRAS: usize = 20;
/// Execuções descartadas antes de começar a medir.
const AQUECIMENTOS: usize = 3;
/// Amostras da execução do código gerado, que é muito mais cara por amostra.
const AMOSTRAS_EXECUCAO: usize = 7;

/// Programa do laço pesado: somatório com cem milhões de iterações.
const SOMATORIO: &str = r"
int somatorio(int n) {
  int total = 0;
  for (int i = 0; i < n; i = i + 1) {
    total = total + i;
  }
  return total;
}

void main() {
  print(somatorio(100000000));
}
";

/// Programa mínimo, usado para estimar o custo fixo de iniciar um processo AOT.
const TRIVIAL: &str = r"
void main() {
  print(0);
}
";

/// Recursão de árvore: muitas chamadas, pouco trabalho por chamada.
const FIB: &str = r"
int fib(int n) {
  if (n < 2) {
    return n;
  }
  return fib(n - 1) + fib(n - 2);
}

void main() {
  print(fib(32));
}
";

/// Programa maior em número de declarações, para o custo de geração por função.
const MUITAS_FUNCOES: &str = r"
int f0(int a) { return a + 0; }
int f1(int a) { return f0(a) + 1; }
int f2(int a) { return f1(a) + 2; }
int f3(int a) { return f2(a) + 3; }
int f4(int a) { return f3(a) + 4; }
int f5(int a) { return f4(a) + 5; }
int f6(int a) { return f5(a) + 6; }
int f7(int a) { return f6(a) + 7; }
int f8(int a) { return f7(a) + 8; }
int f9(int a) { return f8(a) + 9; }

int laco(int n) {
  int total = 0;
  for (int i = 0; i < n; i = i + 1) {
    if (i % 2 == 0) {
      total = total + f9(i);
    } else {
      total = total - f4(i);
    }
  }
  return total;
}

void main() {
  print(f9(1));
}
";

/// Variante de `MUITAS_FUNCOES` sem `%`, que está fora da fatia do JIT.
const MUITAS_FUNCOES_FATIA: &str = r"
int f0(int a) { return a + 0; }
int f1(int a) { return f0(a) + 1; }
int f2(int a) { return f1(a) + 2; }
int f3(int a) { return f2(a) + 3; }
int f4(int a) { return f3(a) + 4; }
int f5(int a) { return f4(a) + 5; }
int f6(int a) { return f5(a) + 6; }
int f7(int a) { return f6(a) + 7; }
int f8(int a) { return f7(a) + 8; }
int f9(int a) { return f8(a) + 9; }

int laco(int n) {
  int total = 0;
  for (int i = 0; i < n; i = i + 1) {
    if (i > 10) {
      total = total + f9(i);
    } else {
      total = total - f4(i);
    }
  }
  return total;
}

void main() {
  print(f9(1) + laco(50));
}
";

/// Traduz a fonte pelo frontend real, do jeito que o compilador faria.
fn hir(fonte: &str) -> Result<Module<'_>, Diagnostic> {
    let tokens = dartforge_lexer::lex(fonte)?;
    let programa = dartforge_parser::parse(&tokens, fonte.len())?;
    let resolucao = dartforge_semantic::analyze(&programa)?;
    Ok(dartforge_hir::lower_resolved(programa, resolucao))
}

/// Mediana e p95 de uma amostra, por posto, como em `docs/DESEMPENHO.md`.
fn resumo(mut amostras: Vec<Duration>) -> (Duration, Duration) {
    amostras.sort_unstable();
    let mediana = amostras[amostras.len() / 2];
    let posto = (amostras.len() * 95).div_ceil(100).saturating_sub(1);
    (mediana, amostras[posto])
}

/// Formata uma duração em milissegundos com três casas e vírgula decimal.
fn ms(duracao: Duration) -> String {
    let micros = duracao.as_micros();
    format!("{},{:03}", micros / 1000, micros % 1000)
}

/// Mede o tempo de geração de código em memória, por fase, de um programa.
fn medir_jit(nome: &str, fonte: &str, otimizacao: Otimizacao) {
    let rotulo = match otimizacao {
        Otimizacao::Nenhuma => "none",
        Otimizacao::Velocidade => "speed",
    };
    let nome = &format!("{nome} (opt={rotulo})");
    let modulo = match hir(fonte) {
        Ok(modulo) => modulo,
        Err(erro) => {
            println!("| {nome} | frontend recusou: {erro} |");
            return;
        }
    };
    for _ in 0..AQUECIMENTOS {
        if let Err(erro) = dartforge_cranelift_jit::compilar_com(&modulo, otimizacao) {
            println!("| {nome} | JIT recusou: {erro} |");
            return;
        }
    }
    let mut traducao = Vec::with_capacity(AMOSTRAS);
    let mut geracao = Vec::with_capacity(AMOSTRAS);
    let mut total = Vec::with_capacity(AMOSTRAS);
    let mut instrucoes = 0;
    let mut bytes = 0;
    for _ in 0..AMOSTRAS {
        let compilado =
            dartforge_cranelift_jit::compilar_com(&modulo, otimizacao).expect("já compilou antes");
        let medicoes = compilado.medicoes();
        traducao.push(medicoes.traducao);
        geracao.push(medicoes.geracao);
        total.push(medicoes.total);
        instrucoes = medicoes.instrucoes_clif;
        bytes = medicoes.bytes_codigo;
    }
    let (traducao_p50, traducao_p95) = resumo(traducao);
    let (geracao_p50, geracao_p95) = resumo(geracao);
    let (total_p50, total_p95) = resumo(total);
    println!(
        "| {nome} | {} | {} | {} | {} | {} | {} | {instrucoes} | {bytes} |",
        ms(traducao_p50),
        ms(traducao_p95),
        ms(geracao_p50),
        ms(geracao_p95),
        ms(total_p50),
        ms(total_p95)
    );
}

/// Mede a emissão de LLVM IR textual do mesmo módulo, para comparar o lado do front.
fn medir_emissao_llvm(nome: &str, fonte: &str) {
    let modulo = match hir(fonte) {
        Ok(modulo) => modulo,
        Err(erro) => {
            println!("| {nome} | frontend recusou: {erro} |");
            return;
        }
    };
    for _ in 0..AQUECIMENTOS {
        if dartforge_llvm::emit(&modulo).is_err() {
            println!("| {nome} | LLVM recusou |");
            return;
        }
    }
    let mut amostras = Vec::with_capacity(AMOSTRAS);
    let mut bytes = 0;
    for _ in 0..AMOSTRAS {
        let inicio = Instant::now();
        let ir = dartforge_llvm::emit(&modulo).expect("já emitiu antes");
        amostras.push(inicio.elapsed());
        bytes = ir.len();
    }
    let (p50, p95) = resumo(amostras);
    println!("| {nome} | {} | {} | {bytes} |", ms(p50), ms(p95));
}

/// Mede o tempo de execução do código gerado pelo JIT, já compilado.
fn medir_execucao_jit(nome: &str, fonte: &str, otimizacao: Otimizacao) {
    let rotulo = match otimizacao {
        Otimizacao::Nenhuma => "none",
        Otimizacao::Velocidade => "speed",
    };
    let modulo = hir(fonte).expect("programa da fatia");
    let compilado =
        dartforge_cranelift_jit::compilar_com(&modulo, otimizacao).expect("programa da fatia");
    for _ in 0..1 {
        let _ = compilado.executar_capturando();
    }
    let mut amostras = Vec::with_capacity(AMOSTRAS_EXECUCAO);
    for _ in 0..AMOSTRAS_EXECUCAO {
        let inicio = Instant::now();
        let saida = compilado.executar_capturando();
        amostras.push(inicio.elapsed());
        assert!(!saida.is_empty());
    }
    let (p50, p95) = resumo(amostras);
    println!(
        "| {nome} | Cranelift JIT (opt_level={rotulo}) | {} | {} |",
        ms(p50),
        ms(p95)
    );
}

/// Compila o programa pelo caminho AOT e mede a execução do processo resultante.
fn medir_execucao_aot(nome: &str, fonte: &str, diretorio: &Path, otimizar: bool) -> Option<()> {
    let modulo = hir(fonte).ok()?;
    let ir = dartforge_llvm::emit(&modulo).ok()?;
    let opcoes = dartforge_native::NativeOptions {
        optimize: otimizar,
        ..dartforge_native::NativeOptions::default()
    };
    let nivel = if otimizar { "O2" } else { "O0" };
    let executavel = diretorio.join(format!(
        "{nome}-{nivel}{}",
        if cfg!(windows) { ".exe" } else { "" }
    ));
    let relatorio = match dartforge_native::build_executable_with_report(&ir, &executavel, &opcoes)
    {
        Ok(relatorio) => relatorio,
        Err(erro) => {
            eprintln!("AOT indisponível para {nome} ({nivel}): {erro}");
            return None;
        }
    };
    let saida = std::process::Command::new(&executavel).output().ok()?;
    assert!(saida.status.success());
    let mut amostras = Vec::with_capacity(AMOSTRAS_EXECUCAO);
    for _ in 0..AMOSTRAS_EXECUCAO {
        let inicio = Instant::now();
        let saida = std::process::Command::new(&executavel).output().ok()?;
        amostras.push(inicio.elapsed());
        assert!(saida.status.success());
    }
    let (p50, p95) = resumo(amostras);
    println!(
        "| {nome} | LLVM AOT {nivel} (processo inteiro) | {} | {} |",
        ms(p50),
        ms(p95)
    );
    eprintln!(
        "build AOT {nome} {nivel}: clang {} ms, rustc+link {} ms, total {} ms, {} bytes",
        ms(relatorio.clang),
        ms(relatorio.rustc_link),
        ms(relatorio.total),
        relatorio.executable_bytes
    );
    Some(())
}

/// Diretório temporário exclusivo, removido ao final do experimento.
struct Fixture(PathBuf);
impl Fixture {
    /// Reserva um diretório sem depender de caminho estático compartilhado.
    fn new() -> Self {
        let marca = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let caminho = std::env::temp_dir().join(format!(
            "dartforge-experimento-{}-{marca}",
            std::process::id()
        ));
        std::fs::create_dir(&caminho).unwrap();
        Self(caminho)
    }
}
impl Drop for Fixture {
    /// Remove apenas o que o experimento criou.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Roda os dois eixos mensuráveis localmente e imprime as tabelas.
fn main() {
    let programas: [(&str, &str); 4] = [
        ("trivial", TRIVIAL),
        ("fib(32)", FIB),
        ("somatorio(1e8)", SOMATORIO),
        ("dez-funcoes", MUITAS_FUNCOES_FATIA),
    ];

    println!("### Eixo 1 — geração de código em memória (Cranelift JIT)\n");
    println!(
        "| programa | tradução p50 (ms) | tradução p95 | geração p50 (ms) | geração p95 | total p50 (ms) | total p95 | instruções CLIF | bytes de código |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for (nome, fonte) in programas {
        medir_jit(nome, fonte, Otimizacao::Nenhuma);
        medir_jit(nome, fonte, Otimizacao::Velocidade);
    }

    println!("\n### Eixo 1b — emissão de LLVM IR textual (mesma HIR, backend AOT)\n");
    println!("| programa | emissão p50 (ms) | p95 | bytes de IR |");
    println!("| --- | --- | --- | --- |");
    for (nome, fonte) in programas {
        medir_emissao_llvm(nome, fonte);
    }

    println!("\n### Eixo 2 — execução do código gerado\n");
    println!("| programa | caminho | p50 (ms) | p95 (ms) |");
    println!("| --- | --- | --- | --- |");
    let fixture = Fixture::new();
    for (nome, fonte) in [
        ("trivial", TRIVIAL),
        ("somatorio(1e8)", SOMATORIO),
        ("fib(32)", FIB),
    ] {
        medir_execucao_jit(nome, fonte, Otimizacao::Nenhuma);
        medir_execucao_jit(nome, fonte, Otimizacao::Velocidade);
        medir_execucao_aot(nome, fonte, &fixture.0, false);
        medir_execucao_aot(nome, fonte, &fixture.0, true);
    }

    // O programa com `%` fica aqui só para registrar que ele existe e está
    // fora da fatia: o JIT o recusa e o experimento não o mede.
    let modulo = hir(MUITAS_FUNCOES).expect("o frontend aceita");
    match dartforge_cranelift_jit::compilar(&modulo) {
        Ok(_) => println!("\nAVISO: `%` deveria estar fora da fatia e foi aceito."),
        Err(erro) => println!("\nFora da fatia, como esperado: {erro}"),
    }
}
