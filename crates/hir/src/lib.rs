//! Fronteira de redução estrutural do subconjunto Dart suportado.
//!
//! Preserva funções, instruções, intervalos de origem e referências ao texto.
//! Ainda não constitui uma representação intermediária tipada com símbolos
//! resolvidos; a análise semântica deve validar o programa antes desta etapa.
use dartforge_syntax::{Class, Function, Program, Statement};

/// Módulo estrutural consumido pelo gerador de JavaScript.
#[derive(Debug)]
pub struct Module<'a> {
    /// Classes nominais e sua hierarquia validada.
    pub classes: Vec<Class<'a>>,
    /// Funções de nível superior, exceto a função principal.
    pub functions: Vec<Function<'a>>,
    /// Corpo da função principal.
    pub statements: Vec<Statement<'a>>,
}

/// Transfere o programa validado para a representação estrutural sem copiar a AST.
///
/// Esta função não valida nomes, tipos ou controle de fluxo. O chamador deve
/// realizar a análise semântica antes de gerar código a partir do resultado.
///
/// ```
/// use dartforge_hir::lower;
/// use dartforge_syntax::Program;
/// let module = lower(Program { classes: vec![], functions: vec![], statements: vec![] });
/// assert!(module.functions.is_empty());
/// assert!(module.statements.is_empty());
/// ```
pub fn lower(program: Program<'_>) -> Module<'_> {
    Module {
        classes: program.classes,
        functions: program.functions,
        statements: program.statements,
    }
}
