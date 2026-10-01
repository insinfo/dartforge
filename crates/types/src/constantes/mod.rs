//! Avaliação e verificação de constantes como o analyzer 6.11
//! (`src/dart/constant/`): [`valor`] (`value.dart`), [`avaliador`]
//! (`evaluation.dart`), [`potencial`] (`potentially_constant.dart`) e
//! [`verificador`] (`constant_verifier.dart`).
//!
//! Roda depois da inferência de corpos, sobre as tabelas laterais dela
//! (tipos estáticos e resoluções): [`verificar`] dá os diagnósticos de uma
//! biblioteca.

pub mod avaliador;
pub mod exaustividade;
pub mod potencial;
pub mod valor;
pub mod verificador;

use crate::resolve::OutlineTypes;
use crate::resolved::BodyTypes;
use crate::table::{CoreTypes, TypeTable};
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_intern::Interner;
use std::collections::HashSet;

pub use verificador::CODIGOS;

/// Os erros de constantes das bibliotecas `libs`. `inferidas` são as
/// bibliotecas cujos corpos `body` tem (as outras são tratadas como
/// opacas: seus valores constantes são desconhecidos).
#[allow(clippy::too_many_arguments)]
pub fn verificar(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &BodyTypes,
    inferidas: &HashSet<LibraryId>,
    libs: &[LibraryId],
) -> Vec<(UnitId, Diagnostic)> {
    let mut m = avaliador::Motor::novo(program, interner, table, core, outline, body, inferidas);
    let mut out = Vec::new();
    for &lib in libs {
        out.extend(verificador::verificar(&mut m, lib));
    }
    out
}
