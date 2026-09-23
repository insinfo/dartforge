//! A regra de publicação (plano §2.3), comum ao `dartforge analyze` e ao LSP:
//! diagnóstico sintático é publicado sempre; um código semântico só é
//! publicado se estiver em `verificados.txt` — 100% de acerto (posição e
//! mensagem) nos casos dele no corpus de paridade e 0 falso positivo nos três
//! projetos reais. O resto existe internamente e vai só para o placar.

use dartforge_diagnostics::{Diagnostic, TipoErro};

/// A lista versionada de códigos semânticos verificados.
pub const VERIFICADOS: &str = include_str!("../verificados.txt");

/// Os códigos de [`VERIFICADOS`] (sem comentários).
pub fn verificados() -> Vec<&'static str> {
    VERIFICADOS
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .collect()
}

/// O diagnóstico vai para o editor/CLI? `sintaxe`: veio do lexer/parser.
pub fn publicado(d: &Diagnostic, sintaxe: bool) -> bool {
    if sintaxe {
        return true;
    }
    match d.code {
        Some(c) if c.info().tipo == TipoErro::SyntacticError => true,
        Some(c) => verificados().contains(&c.info().nome),
        None => false,
    }
}
