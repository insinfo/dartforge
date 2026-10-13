//! Constant evaluation and verification like analyzer 6.11
//! (`src/dart/constant/`): [`valor`] (`value.dart`), [`avaliador`]
//! (`evaluation.dart`), [`potential`] (`potentially_constant.dart`) and
//! [`verifier`] (`constant_verifier.dart`).
//!
//! Runs after body inference, over its side tables (static types and
//! resolutions): [`verificar`] gives the diagnostics of a library.

pub mod avaliador;
pub mod cycles;
pub mod exhaustiveness;
pub mod graph;
pub mod potential;
pub mod valor;
pub mod verifier;

use crate::resolve::OutlineTypes;
use crate::resolved::BodyTypes;
use crate::table::{CoreTypes, TypeTable};
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_intern::Interner;
use std::collections::HashSet;

pub use exhaustiveness::ParteDeTestemunha;
pub use verifier::CODIGOS;

/// The witnesses with parts of the non-exhaustive `switch`es of library
/// `lib`, by (unit, offset of the `switch`): the `diagnostic.data` that
/// `AddMissingSwitchCases` reads. Runs the constant verifier only for that
/// (the diagnostics are discarded).
#[allow(clippy::too_many_arguments)]
pub fn testemunhas_de_switch(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    lib: LibraryId,
) -> std::collections::HashMap<(UnitId, usize), Vec<Vec<ParteDeTestemunha>>> {
    let mut m = avaliador::Motor::novo(program, interner, table, core, outline, body, the_inferred);
    m.witnesses = Some(std::collections::HashMap::new());
    let _ = verifier::verificar(&mut m, lib);
    m.witnesses.take().unwrap_or_default()
}

/// The constant errors of the libraries `libs`. `the_inferred` are the
/// libraries whose bodies `body` has (the others are treated as opaque:
/// their constant values are unknown).
#[allow(clippy::too_many_arguments)]
pub fn verificar(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    libs: &[LibraryId],
) -> Vec<(UnitId, Diagnostic)> {
    verificar_com_quebras(program, interner, table, core, outline, body, the_inferred, libs).0
}

/// [`verificar`], with the libraries on which analyzer 3.6.2 throws
/// (exhaustiveness over a type parameter without a `defaultType`,
/// `exhaustiveness.dart:771`): their `LibraryAnalyzer` fails entirely and
/// the caller discards the diagnostics of their files.
#[allow(clippy::too_many_arguments)]
pub fn verificar_com_quebras(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    libs: &[LibraryId],
) -> (Vec<(UnitId, Diagnostic)>, Vec<LibraryId>) {
    let mut m = avaliador::Motor::novo(program, interner, table, core, outline, body, the_inferred);
    let mut out = Vec::new();
    let mut broken = Vec::new();
    for &lib in libs {
        m.analyzer_break = false;
        out.extend(verifier::verificar(&mut m, lib));
        if m.analyzer_break && program.referencia_da_biblioteca(lib) == dartforge_diagnostics::Referencia::V3_6 {
            broken.push(lib);
        }
    }
    (out, broken)
}
