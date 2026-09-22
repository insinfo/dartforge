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

/// Tempos por fase e contagens de uma compilação (`dartforge compile-js --timings`).
#[derive(Debug, Default, Clone)]
pub struct Relatorio {
    /// `(fase, duração)` na ordem em que as fases correram.
    pub fases: Vec<(&'static str, std::time::Duration)>,
    /// Unidades (`.dart`) do SDK, patches incluídos.
    pub unidades_sdk: usize,
    /// Unidades do usuário e de pacotes.
    pub unidades_usuario: usize,
    pub bibliotecas: usize,
    pub avisos_outline: usize,
    pub avisos_corpos: usize,
    pub modulos: usize,
    /// `"lido"`, `"construído"` ou `""` (desligado/indisponível).
    pub sdk_cache: &'static str,
}

impl Relatorio {
    /// Registra uma fase medida a partir de `inicio`.
    pub fn fase(&mut self, nome: &'static str, inicio: std::time::Instant) {
        self.fases.push((nome, inicio.elapsed()));
    }

    /// Texto de uma linha por fase, com o total.
    pub fn texto(&self) -> String {
        let mut out = String::new();
        let mut total = std::time::Duration::ZERO;
        for (nome, d) in &self.fases {
            out.push_str(&format!("{nome:<24}{:>9.1} ms\n", d.as_secs_f64() * 1000.0));
            total += *d;
        }
        out.push_str(&format!("{:<24}{:>9.1} ms\n", "total", total.as_secs_f64() * 1000.0));
        out.push_str(&format!(
            "unidades: {} SDK + {} usuário/pacotes; {} bibliotecas; {} módulos; avisos: {} outline + {} corpos; cache do SDK: {}\n",
            self.unidades_sdk, self.unidades_usuario, self.bibliotecas, self.modulos, self.avisos_outline, self.avisos_corpos,
            if self.sdk_cache.is_empty() { "não usado" } else { self.sdk_cache }
        ));
        out
    }
}

/// Roda o pipeline inteiro (elementos → outline → corpos → emissão) para uma entrada.
///
/// `sdk_lib` é o `lib/` do SDK (descoberto pelo `PATH` quando `None`).
pub fn compilar(
    entrada: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
) -> Result<Emitido, String> {
    compilar_com_relatorio(entrada, sdk_lib, packages).map(|(e, _)| e)
}

/// Como [`compilar`], devolvendo também o [`Relatorio`] de tempos por fase.
pub fn compilar_com_relatorio(
    entrada: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
) -> Result<(Emitido, Relatorio), String> {
    use dartforge_elements::sdk::SdkLayout;
    use std::time::Instant;
    let mut rel = Relatorio::default();
    let t = Instant::now();
    let sdk_dir = match sdk_lib {
        Some(p) => p.to_path_buf(),
        None => SdkLayout::discover().unwrap_or_else(|| std::path::PathBuf::from("C:/tools/dartsdk-3.6.2/lib")),
    };
    let sdk = SdkLayout::load(&sdk_dir, "dartdevc")?;
    rel.fase("layout do SDK", t);
    // Cache do SDK analisado (`target/dartforge/sdk-<hash>.bin`); a primeira
    // compilação o constrói. `DARTFORGE_SDK_CACHE=0` desliga.
    let t = Instant::now();
    let cache = if std::env::var("DARTFORGE_SDK_CACHE").is_ok_and(|v| v == "0") {
        None
    } else {
        match dartforge_elements::SdkCache::abrir_ou_construir(&sdk, "dartdevc") {
            Ok((c, aberto)) => {
                rel.sdk_cache = if aberto { "lido" } else { "construído" };
                Some(c)
            }
            Err(e) => {
                eprintln!("aviso: cache do SDK indisponível ({e}); lendo o SDK dos arquivos");
                None
            }
        }
    };
    rel.fase("cache do SDK", t);
    let t = Instant::now();
    let mut interner = Interner::new();
    let (program, elements_diags) =
        dartforge_elements::load::load_lenient_com_cache(entrada, &sdk, packages, &mut interner, cache);
    rel.fase("carregar programa", t);
    // Partes do SDK têm URI `file:///…/lib/core/int.dart`; o que decide é a biblioteca.
    rel.unidades_sdk = program.units.iter().filter(|u| program.library(u.library).is_sdk).count();
    rel.unidades_usuario = program.units.len() - rel.unidades_sdk;
    rel.bibliotecas = program.libraries.len();
    // Nenhuma fase falha em silêncio: diagnósticos de carregamento (arquivo ou
    // pacote não encontrado, sintaxe) abortam; os de tipos são avisos por
    // enquanto, porque a inferência ainda tem lacunas e o emissor recua para
    // despacho dinâmico onde o tipo é desconhecido.
    if !elements_diags.is_empty() {
        for d in &elements_diags {
            eprintln!("erro: {d}");
        }
        return Err(format!("{} erro(s) ao carregar o programa", elements_diags.len()));
    }
    let t = Instant::now();
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, outline_diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    rel.fase("outline (tipos)", t);
    let t = Instant::now();
    let (bodies, body_diags) =
        dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
    rel.fase("inferência de corpos", t);
    rel.avisos_outline = outline_diags.len();
    rel.avisos_corpos = body_diags.len();
    let avisos = outline_diags.len() + body_diags.len();
    if avisos > 0 {
        let limite = std::env::var("DARTFORGE_AVISOS").ok().and_then(|v| v.parse().ok()).unwrap_or(20usize);
        for d in outline_diags.iter().chain(body_diags.iter()).take(limite) {
            eprintln!("aviso: {d}");
        }
        eprintln!("({avisos} aviso(s) de tipos; DARTFORGE_AVISOS=N mostra mais)");
    }
    let t = Instant::now();
    let emitido = emitir_programa(&program, &interner, &table, &core, &outline, &bodies)
        .map_err(|ds| ds.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n"))?;
    rel.fase("emissão", t);
    rel.modulos = emitido.modulos.len();
    let t = Instant::now();
    drop(bodies);
    drop(outline);
    drop(table);
    drop(program);
    drop(interner);
    rel.fase("liberar memória", t);
    Ok((emitido, rel))
}

/// Escreve os módulos e o `main.mjs` em `dir` e copia o `dart_sdk.js`.
///
/// Devolve quantos arquivos foram de fato gravados.
pub fn escrever(emitido: &Emitido, dir: &std::path::Path, dart_sdk_js: &std::path::Path) -> Result<usize, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut escritos = 0usize;
    for (path, text) in &emitido.modulos {
        let p = dir.join(path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
        escritos += 1;
    }
    std::fs::write(dir.join("main.mjs"), &emitido.entrada).map_err(|e| format!("main.mjs: {e}"))?;
    escritos += 1;
    let dest = dir.join("dart_sdk.js");
    if !dest.exists() {
        std::fs::copy(dart_sdk_js, &dest)
            .map_err(|e| format!("{} → {}: {e}", dart_sdk_js.display(), dest.display()))?;
    }
    Ok(escritos)
}

/// Caminho do `dart_sdk.js` gerado por `scripts/gerar-dart-sdk.ps1`.
pub fn dart_sdk_js_padrao() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("DARTFORGE_DART_SDK_JS") {
        return std::path::PathBuf::from(p);
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/ddc/dart_sdk.js")
}
