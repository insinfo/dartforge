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
//! * [`operadores`]: aridade de métodos `operator` (`ErrorVerifier`).
//! * [`enums`]: enum sem constantes após augmentations (`ErrorVerifier`).

pub mod duplicatas;
pub mod enums;
pub mod externos;
pub mod heranca;
pub mod importacoes;
pub mod locais;
pub mod operadores;
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
