//! JIT por montador, sem IR e sem otimização, para a fatia escalar do DartForge.
//!
//! Este crate é o terceiro experimento de backend do perfil de desenvolvimento,
//! ao lado de `dartforge-jit` (LLVM ORCv2) e `dartforge-cranelift-jit`
//! (Cranelift). Ele ocupa o extremo oposto do espectro: **não há representação
//! intermediária e não há otimização nenhuma**. A HIR é percorrida uma única
//! vez e cada nó vira instruções x86-64 imediatamente.
//!
//! O ponto de partida é a mesma `dartforge_hir::Module` que o backend AOT
//! consome — não há tradução de LLVM IR textual no caminho.
//!
//! # Qual montador, e por quê
//!
//! O experimento nasceu como "AsmJit", a biblioteca C++. A comparação das três
//! bibliotecas candidatas — AsmJit em C++ por FFI, `asmkit-rs` (porte da AsmJit
//! para Rust) e `dynasm-rs` — está em `docs/ASMJIT.md`, com os números que
//! levaram à escolha do `dynasm-rs`. O nome do crate preserva o do experimento.
//!
//! # Fatia coberta
//!
//! Funções de topo com até quatro parâmetros e retorno `int`/`bool`/`void`,
//! variáveis locais, aritmética (`+`, `-`, `*`, `-` unário), comparações,
//! `&&`, `||`, `!`, `if`/`else`, `while`, `do`/`while`, `for`, `break`,
//! `continue`, `return`, chamadas entre funções do programa — inclusive
//! recursivas e mútuas — e `print` de inteiros e booleanos pelo runtime. `int`
//! é i64 com estouro modular e a ordem de avaliação é a do Dart, igual ao
//! backend LLVM.
//!
//! # Fora da fatia
//!
//! Tudo o mais é recusado com diagnóstico e span da AST, jamais aceito em
//! silêncio: strings, `double`, tipos anuláveis, classes, enums, coleções,
//! closures, records, `switch`, `try`, `for-in`, `assert`, rótulos, `async` e
//! genéricos. As mensagens começam com `asmjit JIT ainda não suporta `, o
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
//! let compilado = dartforge_asmjit_jit::compilar(&modulo)?;
//! assert_eq!(compilado.executar_capturando(), "");
//! # Ok::<(), dartforge_diagnostics::Diagnostic>(())
//! ```
mod abi;
mod execucao;
mod medicoes;
mod quadro;
mod runtime;
mod tradutor;

pub use execucao::ProgramaCompilado;
pub use medicoes::Medicoes;

use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_hir::Module;
use dynasmrt::DynasmApi;
use dynasmrt::x64::Assembler;
use std::time::Instant;

/// Monta o módulo em código executável na memória e mede cada fase.
///
/// As duas fases são cronometradas separadamente: a travessia da HIR, que já
/// produz os bytes das instruções, e a confirmação do bloco, que resolve os
/// saltos e chamadas pendentes e publica as páginas executáveis. Nenhuma delas
/// inclui lexer, parser ou análise semântica.
///
/// # Erros
///
/// Devolve [`Diagnostic`] com o span original quando o programa usa qualquer
/// construção fora da fatia escalar, inclusive em trechos inalcançáveis; quando
/// o alvo não é x86-64; e quando o próprio montador não consegue resolver uma
/// relocação ou mapear memória executável.
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
/// let compilado = dartforge_asmjit_jit::compilar(&modulo)?;
/// assert_eq!(compilado.executar_capturando(), "7\n");
/// assert!(compilado.medicoes().instrucoes_emitidas > 0);
/// # Ok::<(), dartforge_diagnostics::Diagnostic>(())
/// ```
pub fn compilar(modulo: &Module<'_>) -> Result<ProgramaCompilado, Diagnostic> {
    let sem_span = Span { start: 0, end: 0 };
    if !cfg!(target_arch = "x86_64") {
        // O gerador só sabe emitir x86-64. Recusar aqui é o que impede que uma
        // máquina de outra arquitetura receba um ponteiro para bytes que não
        // são instruções dela.
        return Err(Diagnostic::new(
            "asmjit JIT ainda não suporta arquiteturas fora de x86-64",
            sem_span,
        ));
    }
    let inicio = Instant::now();
    let mut ops = Assembler::new().map_err(|e| {
        Diagnostic::new(
            format!("o montador asmjit não pôde mapear memória: {e}"),
            sem_span,
        )
    })?;

    let fase = Instant::now();
    let traducao = tradutor::traduzir(modulo, &mut ops)?;
    let tempo_traducao = fase.elapsed();
    // O deslocamento corrente do montador é exatamente quantos bytes de código
    // foram emitidos. Ele é lido aqui, antes de publicar o bloco, porque
    // `ExecutableBuffer::size` devolve o tamanho do *mapeamento*, arredondado
    // para cima em páginas: 4096 bytes para qualquer programa desta fatia, o que
    // seria um contador inútil.
    let bytes_codigo = ops.offset().0;

    let fase = Instant::now();
    // `commit` é chamado explicitamente porque `finalize` entra em pânico se
    // encontrar relocação impossível; aqui ela vira diagnóstico.
    ops.commit().map_err(|e| {
        Diagnostic::new(
            format!("o montador asmjit não resolveu uma relocação: {e}"),
            sem_span,
        )
    })?;
    let bloco = ops.finalize().map_err(|_| {
        Diagnostic::new(
            "o montador asmjit não pôde publicar o bloco executável",
            sem_span,
        )
    })?;
    let tempo_geracao = fase.elapsed();

    Ok(ProgramaCompilado::novo(
        bloco,
        traducao.entrada,
        Medicoes {
            traducao: tempo_traducao,
            geracao: tempo_geracao,
            total: inicio.elapsed(),
            instrucoes_emitidas: traducao.instrucoes,
            bytes_codigo,
        },
    ))
}
