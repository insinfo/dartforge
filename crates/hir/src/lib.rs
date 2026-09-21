//! Representação intermediária estrutural com alvos estáticos de chamadas resolvidos.
//!
//! Classes conservam IDs nominais; extensions usam uma tabela de alvos por intervalo
//! de expressão. Ainda não existe uma IR completa com IDs para todos os locais e tipos.
use dartforge_syntax::{Class, Extension, Function, Program, Resolution, Statement};
mod mixins;
pub use mixins::expand_mixins;

/// Módulo validado consumido pelos backends de emissão.
#[derive(Debug)]
pub struct Module<'a> {
    /// Distingue regras de retorno arrow da entrada original.
    pub main_is_arrow: bool,
    /// Preserva a entrada async mesmo quando seu corpo nao contem await.
    pub main_is_async: bool,
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
/// let module = lower(Program { main_is_arrow: false, main_is_async: false, types: vec![], classes: vec![], extensions: vec![], functions: vec![], statements: vec![] });
/// assert!(module.functions.is_empty());
/// ```
pub fn lower(program: Program<'_>) -> Module<'_> {
    let resolution = Resolution {
        types: program.types.clone(),
        ..Resolution::default()
    };
    lower_resolved(program, resolution)
}

/// Preserva a AST e os alvos estáticos já validados, sem copiar os corpos.
///
/// A tabela usa spans da mesma unidade lógica da AST e deve ter sido produzida
/// antes da emissão. Passes posteriores precisam preservar ou atualizar esses spans.
pub fn lower_resolved(program: Program<'_>, resolution: Resolution) -> Module<'_> {
    Module {
        main_is_arrow: program.main_is_arrow,
        main_is_async: program.main_is_async,
        classes: program.classes,
        extensions: program.extensions,
        resolution,
        functions: program.functions,
        statements: program.statements,
    }
}
