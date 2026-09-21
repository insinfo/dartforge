//! JIT em Rust puro, com Cranelift, para a fatia escalar do subconjunto DartForge.
//!
//! Este crate é a alternativa medida ao caminho LLVM ORCv2: compila para código
//! de máquina **em memória**, sem clang, sem linker e sem uma instalação do LLVM
//! na máquina. O ponto de partida é a mesma `dartforge_hir::Module` que o
//! backend AOT consome — não há tradução de LLVM IR textual no caminho.
//!
//! # Fatia coberta
//!
//! Funções de topo com parâmetros e retorno `int`/`bool`/`void`, variáveis
//! locais, aritmética (`+`, `-`, `*`, `-` unário), comparações, `&&`, `||`, `!`,
//! `if`/`else`, `while`, `do`/`while`, `for`, `break`, `continue`, `return`,
//! chamadas entre funções do programa — inclusive recursivas e mútuas — e
//! `print` de inteiros e booleanos pelo runtime. `int` é i64 com estouro
//! modular e a ordem de avaliação é a do Dart, igual ao backend LLVM.
//!
//! # Fora da fatia
//!
//! Tudo o mais é recusado com diagnóstico e span da AST, jamais aceito em
//! silêncio: strings, `double`, tipos anuláveis, classes, enums, coleções,
//! closures, records, `switch`, `try`, `for-in`, `assert`, rótulos, `async` e
//! genéricos. As mensagens começam com `Cranelift JIT ainda não suporta `, o
//! espelho de `LLVM AOT ainda não suporta ` do backend nativo. O prefixo difere
//! de propósito: um mesmo programa pode ser aceito por um backend e recusado
//! pelo outro, e a mensagem precisa dizer qual dos dois recusou.
//!
//! # Exemplos
//!
//! ```
//! use dartforge_syntax::Program;
//! let programa = Program {
//!     main_is_arrow: false,
//!     main_is_async: false,
//!     types: vec![],
//!     classes: vec![],
//!     extensions: vec![],
//!     functions: vec![],
//!     statements: vec![],
//! };
//! let modulo = dartforge_hir::lower(programa);
//! let compilado = dartforge_cranelift_jit::compilar(&modulo)?;
//! assert_eq!(compilado.executar_capturando(), "");
//! # Ok::<(), dartforge_diagnostics::Diagnostic>(())
//! ```
mod execucao;
mod medicoes;
mod runtime;
mod tradutor;

pub use execucao::ProgramaCompilado;
pub use medicoes::Medicoes;

use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Module as _, default_libcall_names};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_hir::Module;
use std::time::Instant;

/// Compila o módulo para código executável em memória e mede cada fase.
///
/// As duas fases são cronometradas separadamente: a tradução da HIR para a IR
/// do Cranelift e a geração do código de máquina com as relocações resolvidas.
/// Nenhuma delas inclui lexer, parser ou análise semântica.
///
/// # Erros
///
/// Devolve [`Diagnostic`] com o span original quando o programa usa qualquer
/// construção fora da fatia escalar, inclusive em trechos inalcançáveis, e
/// quando o próprio Cranelift recusa a IR produzida.
///
/// # Exemplos
///
/// ```
/// use dartforge_syntax::{Expr, ExprKind, Program, Statement, StatementKind};
/// use dartforge_diagnostics::Span;
/// let span = Span { start: 0, end: 1 };
/// let programa = Program {
///     main_is_arrow: false,
///     main_is_async: false,
///     types: vec![],
///     classes: vec![],
///     extensions: vec![],
///     functions: vec![],
///     statements: vec![Statement {
///         kind: StatementKind::Print(Expr { kind: ExprKind::Int(7), span }),
///         span,
///     }],
/// };
/// let modulo = dartforge_hir::lower(programa);
/// let compilado = dartforge_cranelift_jit::compilar(&modulo)?;
/// assert_eq!(compilado.executar_capturando(), "7\n");
/// assert!(compilado.medicoes().instrucoes_clif > 0);
/// # Ok::<(), dartforge_diagnostics::Diagnostic>(())
/// ```
pub fn compilar(modulo: &Module<'_>) -> Result<ProgramaCompilado, Diagnostic> {
    compilar_com(modulo, Otimizacao::Nenhuma)
}

/// Nível de otimização do gerador de código do Cranelift.
///
/// O perfil de desenvolvimento quer código pronto depressa, não código rápido,
/// e por isso [`compilar`] usa [`Otimizacao::Nenhuma`] — o padrão do Cranelift.
/// [`Otimizacao::Velocidade`] existe para o experimento de `docs/CRANELIFT.md`
/// poder medir os dois pontos da troca em vez de supor um deles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Otimizacao {
    /// `opt_level = "none"`: o padrão do Cranelift.
    Nenhuma,
    /// `opt_level = "speed"`.
    Velocidade,
}
impl Otimizacao {
    /// Valor aceito pela configuração `opt_level` do `cranelift-codegen`.
    fn bandeira(self) -> &'static str {
        match self {
            Self::Nenhuma => "none",
            Self::Velocidade => "speed",
        }
    }
}

/// Compila como [`compilar`], escolhendo o nível de otimização do Cranelift.
///
/// # Erros
///
/// Os mesmos de [`compilar`], mais a recusa da própria configuração quando o
/// ISA do host não aceita o nível pedido.
///
/// # Exemplos
///
/// ```
/// use dartforge_cranelift_jit::Otimizacao;
/// use dartforge_syntax::Program;
/// let programa = Program {
///     main_is_arrow: false,
///     main_is_async: false,
///     types: vec![],
///     classes: vec![],
///     extensions: vec![],
///     functions: vec![],
///     statements: vec![],
/// };
/// let modulo = dartforge_hir::lower(programa);
/// let compilado = dartforge_cranelift_jit::compilar_com(&modulo, Otimizacao::Velocidade)?;
/// assert_eq!(compilado.executar_capturando(), "");
/// # Ok::<(), dartforge_diagnostics::Diagnostic>(())
/// ```
pub fn compilar_com(
    modulo: &Module<'_>,
    otimizacao: Otimizacao,
) -> Result<ProgramaCompilado, Diagnostic> {
    let inicio = Instant::now();
    let sem_span = Span { start: 0, end: 0 };
    let mut construtor = JITBuilder::with_flags(
        &[("opt_level", otimizacao.bandeira())],
        default_libcall_names(),
    )
    .map_err(|e| Diagnostic::new(format!("Cranelift JIT indisponível: {e}"), sem_span))?;
    for (nome, endereco) in runtime::simbolos() {
        construtor.symbol(nome, endereco);
    }
    let mut jit = JITModule::new(construtor);

    let fase = Instant::now();
    let traducao = tradutor::traduzir(modulo, &mut jit)?;
    let tempo_traducao = fase.elapsed();

    let fase = Instant::now();
    let mut contexto = jit.make_context();
    let mut bytes_codigo = 0usize;
    for (id, corpo) in traducao.corpos {
        jit.clear_context(&mut contexto);
        contexto.func = corpo;
        jit.define_function(id, &mut contexto)
            .map_err(|e| Diagnostic::new(format!("Cranelift recusou a IR: {e}"), sem_span))?;
        if let Some(codigo) = contexto.compiled_code() {
            bytes_codigo += codigo.code_info().total_size as usize;
        }
    }
    jit.clear_context(&mut contexto);
    jit.finalize_definitions()
        .map_err(|e| Diagnostic::new(format!("Cranelift não finalizou o módulo: {e}"), sem_span))?;
    let tempo_geracao = fase.elapsed();

    let entrada = jit.get_finalized_function(traducao.entrada);
    Ok(ProgramaCompilado::novo(
        jit,
        entrada,
        Medicoes {
            traducao: tempo_traducao,
            geracao: tempo_geracao,
            total: inicio.elapsed(),
            instrucoes_clif: traducao.instrucoes,
            bytes_codigo,
        },
    ))
}
