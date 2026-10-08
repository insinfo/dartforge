//! Avaliação e verificação de constantes como o analyzer 6.11
//! (`src/dart/constant/`): [`valor`] (`value.dart`), [`avaliador`]
//! (`evaluation.dart`), [`potencial`] (`potentially_constant.dart`) e
//! [`verificador`] (`constant_verifier.dart`).
//!
//! Roda depois da inferência de corpos, sobre as tabelas laterais dela
//! (tipos estáticos e resoluções): [`verificar`] dá os diagnósticos de uma
//! biblioteca.

pub mod avaliador;
pub mod ciclos;
pub mod exaustividade;
pub mod grafo;
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

pub use exaustividade::ParteDeTestemunha;
pub use verificador::CODIGOS;

/// As testemunhas com partes dos `switch` não exaustivos da biblioteca
/// `lib`, por (unidade, offset do `switch`): o `diagnostic.data` que o
/// `AddMissingSwitchCases` lê. Roda o verificador de constantes só para
/// isso (os diagnósticos são descartados).
#[allow(clippy::too_many_arguments)]
pub fn testemunhas_de_switch(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &BodyTypes,
    inferidas: &HashSet<LibraryId>,
    lib: LibraryId,
) -> std::collections::HashMap<(UnitId, usize), Vec<Vec<ParteDeTestemunha>>> {
    let mut m = avaliador::Motor::novo(program, interner, table, core, outline, body, inferidas);
    m.testemunhas = Some(std::collections::HashMap::new());
    let _ = verificador::verificar(&mut m, lib);
    m.testemunhas.take().unwrap_or_default()
}

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
    verificar_com_quebras(program, interner, table, core, outline, body, inferidas, libs).0
}

/// [`verificar`], com as bibliotecas em que o analyzer 3.6.2 lança (a
/// exaustividade sobre um parâmetro de tipo sem `defaultType`,
/// `exhaustiveness.dart:771`): o `LibraryAnalyzer` delas falha inteiro e
/// quem chama descarta os diagnósticos dos arquivos delas.
#[allow(clippy::too_many_arguments)]
pub fn verificar_com_quebras(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &BodyTypes,
    inferidas: &HashSet<LibraryId>,
    libs: &[LibraryId],
) -> (Vec<(UnitId, Diagnostic)>, Vec<LibraryId>) {
    let mut m = avaliador::Motor::novo(program, interner, table, core, outline, body, inferidas);
    let mut out = Vec::new();
    let mut quebradas = Vec::new();
    for &lib in libs {
        m.quebra_do_analyzer = false;
        out.extend(verificador::verificar(&mut m, lib));
        if m.quebra_do_analyzer && program.referencia_da_biblioteca(lib) == dartforge_diagnostics::Referencia::V3_6 {
            quebradas.push(lib);
        }
    }
    (out, quebradas)
}
