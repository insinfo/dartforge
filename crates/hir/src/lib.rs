//! Representação intermediária estrutural com alvos estáticos de chamadas resolvidos.
//!
//! Classes conservam IDs nominais; extensions usam uma tabela de alvos por intervalo
//! de expressão. Ainda não existe uma IR completa com IDs para todos os locais e tipos.
use dartforge_syntax::{Class, Extension, Function, Program, Resolution, Statement};

/// Módulo validado consumido pelo gerador de JavaScript.
#[derive(Debug)]
pub struct Module<'a> {
    /// Classes nominais e sua hierarquia validada.
    pub classes: Vec<Class<'a>>,
    /// Extensions nomeadas com métodos de despacho estático.
    pub extensions: Vec<Extension<'a>>,
    /// Alvos de chamadas de extension escolhidos durante a análise de tipos.
    pub resolution: Resolution,
    /// Funções de nível superior, exceto a função principal.
    pub functions: Vec<Function<'a>>,
    /// Corpo da função principal.
    pub statements: Vec<Statement<'a>>,
}

/// Transfere a AST validada sem alvos adicionais, para programas sem extensions.
///
/// Não valida nomes, tipos ou fluxo. Programas com chamadas de extension devem usar
/// [`lower_resolved`] e fornecer a resolução produzida pela análise semântica.
///
/// # Exemplos
/// ```
/// use dartforge_hir::lower;
/// use dartforge_syntax::Program;
/// let module = lower(Program { classes: vec![], extensions: vec![], functions: vec![], statements: vec![] });
/// assert!(module.functions.is_empty());
/// ```
pub fn lower(program: Program<'_>) -> Module<'_> {
    lower_resolved(program, Resolution::default())
}

/// Preserva a AST e os alvos estáticos já validados, sem copiar os corpos.
///
/// A tabela usa spans da mesma unidade lógica da AST e deve ter sido produzida
/// antes da emissão. Passes posteriores precisam preservar ou atualizar esses spans.
pub fn lower_resolved(program: Program<'_>, resolution: Resolution) -> Module<'_> {
    Module {
        classes: program.classes,
        extensions: program.extensions,
        resolution,
        functions: program.functions,
        statements: program.statements,
    }
}
