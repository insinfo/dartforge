//! Verificadores do analyzer que não dependem de tipos, sobre a árvore do
//! `frontend` (plano A3). Cada um reproduz um verificador do
//! `package:analyzer` 6.11.0 — os códigos, as mensagens em inglês e as
//! posições são os dele — e é conferido pelo placar de `crates/paridade`.
//!
//! * [`locais`]: variáveis e funções locais não usadas (`UnusedLocalElementsVerifier`).
//! * [`duplicatas`]: `DuplicateDefinitionVerifier` e
//!   `MemberDuplicateDefinitionVerifier` (`src/error/duplicate_definition_verifier.dart`).
//! * [`externos`]: inicializadores de campos e variáveis `external`
//!   (`ErrorVerifier`).
//! * [`privados`]: declarações privadas nunca referenciadas
//!   (`UnusedLocalElementsVerifier`, a parte de biblioteca).
//! * [`operadores`]: aridade, parâmetros opcionais e retorno de `[]=` em
//!   métodos `operator` (`ErrorVerifier`).
//! * [`enums`]: enum sem constantes após augmentations (`ErrorVerifier`).
//! * [`clausulas`]: cláusulas de herança (`subtype_of_disallowed_type`,
//!   erros de mixin, `class_used_as_mixin`) e a porta do `ErrorVerifier`
//!   que desliga as verificações seguintes.
//! * [`modificadores`]: `base`/`final`/`interface`/`sealed` usados fora da
//!   biblioteca (`ErrorVerifier` e `BaseOrFinalTypeVerifier`).
//! * [`membros`]: verificações locais de declarações e membros do
//!   `ErrorVerifier` (parâmetros de tipo em conflito, campos de enum e de
//!   tipo de extensão, setters, `this.x` fora de construtor, lista de
//!   inicializadores, `return` em construtor gerador).

pub mod clausulas;
pub mod duplicatas;
pub mod enums;
pub mod externos;
pub mod heranca;
pub mod importacoes;
pub mod inicializacao;
pub mod locais;
pub mod membros;
pub mod modificadores;
pub mod operadores;
pub mod privados;
pub mod publicacao;

use dartforge_frontend::ast::{Ast, CompilationUnit};

/// Uma unidade (arquivo) de uma biblioteca, na ordem da biblioteca: a que a
/// declara primeiro, depois as partes.
#[derive(Clone, Copy)]
pub struct Unidade<'a> {
    pub ast: &'a Ast,
    pub unit: &'a CompilationUnit,
    /// O texto da unidade (anotações locais não ficam na árvore).
    pub fonte: &'a str,
}
