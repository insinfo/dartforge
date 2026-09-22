//! Emissão de JavaScript no contrato de módulos do DDC (docs/EMISSAO-DDC.md).
//!
//! Um módulo ES6 por biblioteca do usuário, ligando contra `dart_sdk.js`.
#![allow(clippy::too_many_arguments, clippy::collapsible_if, clippy::collapsible_else_if)]

pub mod body;
pub mod call;
pub mod ctx;
pub mod expr;
pub mod js;
pub mod module;
pub mod pattern;
pub mod ty;

use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::Program;
use dartforge_intern::Interner;
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::resolved::BodyTypes;
use dartforge_types::table::{CoreTypes, TypeTable};

/// Resultado da emissão: módulos (nome de arquivo, texto) e o `main.mjs`.
pub struct Emitido {
    pub modulos: Vec<(String, String)>,
    pub entrada: String,
}

/// Emite um módulo por biblioteca não-SDK do programa e o `main.mjs`.
pub fn emitir_programa(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
) -> Result<Emitido, Vec<Diagnostic>> {
    let ctx = ctx::Ctx::new(program, interner, table, core, outline, bodies);
    module::emitir(&ctx)
}

/// Roda o pipeline inteiro (elementos → outline → corpos → emissão) para uma entrada.
///
/// `sdk_lib` é o `lib/` do SDK (descoberto pelo `PATH` quando `None`).
pub fn compilar(
    entrada: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
) -> Result<Emitido, String> {
    use dartforge_elements::load::load_lenient;
    use dartforge_elements::sdk::SdkLayout;
    let sdk_dir = match sdk_lib {
        Some(p) => p.to_path_buf(),
        None => SdkLayout::discover().unwrap_or_else(|| std::path::PathBuf::from("C:/tools/dartsdk-3.6.2/lib")),
    };
    let sdk = SdkLayout::load(&sdk_dir, "dartdevc")?;
    let mut interner = Interner::new();
    let (program, _elements_diags) = load_lenient(entrada, &sdk, packages, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _outline_diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let (bodies, _body_diags) =
        dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
    emitir_programa(&program, &interner, &table, &core, &outline, &bodies)
        .map_err(|ds| ds.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n"))
}

/// Escreve os módulos e o `main.mjs` em `dir` e copia o `dart_sdk.js`.
pub fn escrever(emitido: &Emitido, dir: &std::path::Path, dart_sdk_js: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (path, text) in &emitido.modulos {
        let p = dir.join(path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    std::fs::write(dir.join("main.mjs"), &emitido.entrada).map_err(|e| format!("main.mjs: {e}"))?;
    let dest = dir.join("dart_sdk.js");
    if !dest.exists() {
        std::fs::copy(dart_sdk_js, &dest)
            .map_err(|e| format!("{} → {}: {e}", dart_sdk_js.display(), dest.display()))?;
    }
    Ok(())
}

/// Caminho do `dart_sdk.js` gerado por `scripts/gerar-dart-sdk.ps1`.
pub fn dart_sdk_js_padrao() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("DARTFORGE_DART_SDK_JS") {
        return std::path::PathBuf::from(p);
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/ddc/dart_sdk.js")
}
