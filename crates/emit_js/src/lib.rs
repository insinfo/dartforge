//! Emissão de JavaScript no contrato de módulos do DDC (docs/EMISSAO-DDC.md).
//!
//! Um módulo ES6 por biblioteca do usuário, ligando contra `dart_sdk.js`.
#![allow(clippy::too_many_arguments, clippy::collapsible_if, clippy::collapsible_else_if)]

pub mod body;
pub mod call;
pub mod ctx;
pub mod expr;
pub mod filtro;
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

/// O programa analisado (elementos, outline, corpos), entregue a quem compila
/// no perfil de produção para calcular o mundo fechado antes de emitir.
pub struct Analise<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a OutlineTypes,
    pub bodies: &'a BodyTypes,
}

impl<'a> Analise<'a> {
    /// Contexto de emissão sem filtro (para consultas: interop, nomes JS).
    pub fn ctx(&self) -> ctx::Ctx<'a> {
        ctx::Ctx::new(self.program, self.interner, self.table, self.core, self.outline, self.bodies)
    }

    /// Emite o programa. `filtro: None` é exatamente [`emitir_programa`].
    pub fn emitir<'b>(&'b self, filtro: Option<&'b dyn filtro::Vivos>) -> Result<Emitido, String> {
        let mut ctx: ctx::Ctx<'b> = ctx::Ctx::new(self.program, self.interner, self.table, self.core, self.outline, self.bodies);
        ctx.filtro = filtro;
        module::emitir_com_cache(&ctx, None, None).map_err(|ds| ds.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n"))
    }
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
    emitir_programa_com_cache(program, interner, table, core, outline, bodies, None)
}

/// Como [`emitir_programa`], guardando os fragmentos emitidos para que a
/// próxima compilação reuse o que não mudou.
pub fn emitir_programa_com_cache(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    cache: Option<&std::cell::RefCell<module::CacheFragmentos>>,
) -> Result<Emitido, Vec<Diagnostic>> {
    let ctx = ctx::Ctx::new(program, interner, table, core, outline, bodies);
    module::emitir_com_cache(&ctx, None, cache)
}

/// Emite só os módulos que contêm alguma das bibliotecas dadas, para a sessão
/// residente (`dartforge dev`): o JS de uma biblioteca que não mudou e cujas
/// dependências não mudaram de API continua valendo.
///
/// `Emitido::modulos` traz apenas os módulos emitidos; `Emitido::entrada`
/// (o `main.mjs`) vem sempre, porque é barato e depende só da entrada.
/// Devolve também quanto custou montar o contexto de emissão.
pub fn emitir_modulos(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    bibliotecas: &[dartforge_elements::model::LibraryId],
) -> Result<(Emitido, std::time::Duration), Vec<Diagnostic>> {
    emitir_modulos_com_cache(program, interner, table, core, outline, bodies, bibliotecas, None)
}

/// Como [`emitir_modulos`], reusando os fragmentos (texto por classe e por
/// biblioteca) já emitidos. A sessão invalida os das bibliotecas alteradas
/// antes de chamar: é o que faz uma edição reemitir uma classe em vez das
/// 1.343 de um módulo com 345 bibliotecas em ciclo.
#[allow(clippy::too_many_arguments)]
pub fn emitir_modulos_com_cache(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    bibliotecas: &[dartforge_elements::model::LibraryId],
    cache: Option<&std::cell::RefCell<module::CacheFragmentos>>,
) -> Result<(Emitido, std::time::Duration), Vec<Diagnostic>> {
    let t = std::time::Instant::now();
    let ctx = ctx::Ctx::new(program, interner, table, core, outline, bodies);
    let contexto = t.elapsed();
    let so: std::collections::HashSet<u32> = bibliotecas.iter().map(|l| l.0).collect();
    module::emitir_com_cache(&ctx, Some(&so), cache).map(|e| (e, contexto))
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
    /// Fontes Dart vindas da geração em memória, não do disco.
    pub fontes_geradas: usize,
    pub avisos_outline: usize,
    pub avisos_corpos: usize,
    pub modulos: usize,
    /// `"lido"`, `"construído"` ou `""` (desligado/indisponível).
    pub sdk_cache: &'static str,
    /// Sub-fases de `load_lenient` (impressas sob "carregar programa").
    pub carga: dartforge_elements::model::TemposCarga,
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
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
        for (nome, d) in &self.fases {
            out.push_str(&format!("{nome:<24}{:>9.1} ms\n", ms(*d)));
            total += *d;
            if *nome == "carregar programa" {
                let c = &self.carga;
                let sub = [
                    ("leitura+lex paralelos", c.leitura_lex_paralelo),
                    ("leitura+lex em série", c.leitura),
                    ("parse", c.parse),
                    ("SDK do cache", c.sdk_cache),
                    ("diretivas/URIs", c.diretivas),
                    ("outline: declarações", c.outline_declaracoes),
                    ("outline: reexports", c.outline_reexports),
                    ("outline: escopos", c.outline_escopos),
                    ("outline: supertipos", c.outline_supertipos),
                ];
                for (n, d) in sub {
                    out.push_str(&format!("  {n:<22}{:>9.1} ms\n", ms(d)));
                }
                out.push_str(&format!(
                    "  {:<22}{:>9} arquivos, {:.2} MiB, {} ondas\n",
                    "lidos",
                    c.arquivos_lidos,
                    c.bytes_lidos as f64 / 1_048_576.0,
                    c.ondas
                ));
            }
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
/// `package_config.json` do projeto, pelo caminho dado ou descoberto a partir
/// da entrada.
fn configuracao_de_pacotes(
    entrada: &std::path::Path,
    packages: Option<&std::path::Path>,
) -> Option<dartforge_elements::config::PackageConfig> {
    packages
        .map(|p| p.to_path_buf())
        .or_else(|| dartforge_elements::config::PackageConfig::discover(entrada))
        .and_then(|p| dartforge_elements::config::PackageConfig::load(&p).ok())
}

/// Nome do pacote cuja raiz é `raiz` — é ele que aparece nas URIs `asset:`
/// que o ngdart usa em modo de desenvolvimento.
fn nome_do_pacote(cfg: &dartforge_elements::config::PackageConfig, raiz: &std::path::Path) -> String {
    cfg.packages
        .iter()
        .find(|(_, p)| {
            p.root_uri
                .to_file_path()
                .is_ok_and(|d| dartforge_elements::config::sem_verbatim(d) == raiz)
        })
        .map(|(n, _)| n.clone())
        .unwrap_or_default()
}

/// `DARTFORGE_GERADOS_PKGS=a,b` restringe a geração a esses pacotes — serve
/// para comparar com o disco sem mudar mais nada.
fn filtro_de_pacotes() -> Option<std::collections::HashSet<String>> {
    std::env::var("DARTFORGE_GERADOS_PKGS")
        .ok()
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
}

pub fn compilar_com_relatorio(
    entrada: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
) -> Result<(Emitido, Relatorio), String> {
    compilar_com_relatorio_e_gerador(entrada, sdk_lib, packages, None)
}

/// Quem produz as fontes geradas (o motor de build, `crates/build`) a partir
/// do programa carregado sem elas — o `BuildStep.resolver` numa passada.
pub type Gerador<'a> = &'a dyn Fn(
    &dartforge_elements::model::Program,
    &Interner,
) -> Result<std::sync::Arc<dartforge_elements::gerado::Geracao>, String>;

/// Como [`compilar_com_relatorio`], com o gerador de fontes do projeto.
pub fn compilar_com_relatorio_e_gerador(
    entrada: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
    gerador: Option<Gerador<'_>>,
) -> Result<(Emitido, Relatorio), String> {
    let (emitido, mut rel) = compilar_com_gerador(entrada, sdk_lib, packages, gerador, |a| {
        emitir_programa(a.program, a.interner, a.table, a.core, a.outline, a.bodies)
            .map_err(|ds| ds.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n"))
    })?;
    rel.modulos = emitido.modulos.len();
    Ok((emitido, rel))
}

/// O pipeline inteiro até a inferência de corpos; a emissão é de quem chama
/// (`fim`), que recebe o programa analisado. O perfil de produção calcula o
/// mundo fechado aqui dentro, antes de emitir (`crates/emit_js_producao`).
pub fn compilar_com<R>(
    entrada: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
    fim: impl FnOnce(&Analise<'_>) -> Result<R, String>,
) -> Result<(R, Relatorio), String> {
    compilar_com_gerador(entrada, sdk_lib, packages, None, fim)
}

/// Como [`compilar_com`], com o gerador de fontes (o motor de build).
pub fn compilar_com_gerador<R>(
    entrada: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
    gerador: Option<Gerador<'_>>,
    fim: impl FnOnce(&Analise<'_>) -> Result<R, String>,
) -> Result<(R, Relatorio), String> {
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
    // Fontes geradas em memória. Com `gerador` (o motor de build, ligado
    // pelo `compile-js` quando o projeto usa `build_runner`), a geração vem
    // dele. `DARTFORGE_GERADOS` fica um ciclo como sinônimo:
    // `build_runner` = só o apoio (o que o `build_runner` já escreveu — é
    // como se verifica que ler de memória dá o mesmo JS que ler do disco),
    // `ng` = o motor (o padrão quando há gerador), `nenhum` = desliga.
    let modo = std::env::var("DARTFORGE_GERADOS").ok();
    let gerados = match (modo.as_deref(), gerador) {
        (Some("nenhum"), _) => None,
        (Some("ng"), None) => {
            return Err("DARTFORGE_GERADOS=ng exige o motor de build: o projeto não usa build_runner".into());
        }
        (Some("ng") | None, Some(g)) => {
            // Fase 1: carregar o projeto sem os gerados, só para o motor ter
            // banco semântico — é o `BuildStep.resolver` do `package:build`.
            // A carga é tolerante: os `.template.dart` que faltam viram
            // diagnóstico e o resto do programa fica de pé.
            let t_fase1 = Instant::now();
            let mut nomes_fase1 = Interner::new();
            let (programa, _) = dartforge_elements::load::load_lenient(entrada, &sdk, packages, &mut nomes_fase1);
            rel.fase("motor: carga de resolução", t_fase1);
            let t_motor = Instant::now();
            let g = g(&programa, &nomes_fase1)?;
            rel.fase("motor de build", t_motor);
            Some(g)
        }
        (Some("build_runner"), _) => {
            let cfg = configuracao_de_pacotes(entrada, packages);
            cfg.map(|c| {
                dartforge_elements::gerado::do_build_runner(
                    &c,
                    &[".template.dart", ".css.shim.dart"],
                    filtro_de_pacotes().as_ref(),
                )
            })
        }
        _ => None,
    };
    if let Some(g) = &gerados {
        rel.fontes_geradas = g.len();
        if std::env::var("DARTFORGE_GERADOS_DEBUG").is_ok() {
            let mut v: Vec<String> = g.caminhos().map(|p| p.display().to_string()).collect();
            v.sort();
            eprintln!("geração: {} fontes", v.len());
            for c in v.iter().take(3) { eprintln!("  {c}"); }
            for c in v.iter().filter(|c| c.contains("limitless")).take(3) { eprintln!("  L {c}"); }
        }
    }
    let (program, elements_diags) = dartforge_elements::load::load_lenient_gerados(
        entrada,
        &sdk,
        packages,
        &mut interner,
        cache,
        None,
        gerados,
    );
    rel.fase("carregar programa", t);
    rel.carga = program.tempos.clone();
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
        if std::env::var("DARTFORGE_AVISOS_RESUMO").is_ok() {
            // Mensagens mais frequentes (sem a posição).
            let mut contagem: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for d in outline_diags.iter().chain(body_diags.iter()) {
                *contagem.entry(d.message.clone()).or_default() += 1;
            }
            let mut v: Vec<(String, usize)> = contagem.into_iter().collect();
            // Empate na contagem desempata pela mensagem (o `HashMap` não tem ordem).
            v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            for (m, n) in v.iter().take(30) {
                eprintln!("{n:6}  {m}");
            }
        }
    }
    let t = Instant::now();
    let emitido = fim(&Analise { program: &program, interner: &interner, table: &table, core: &core, outline: &outline, bodies: &bodies })?;
    rel.fase("emissão", t);
    let t = Instant::now();
    drop(bodies);
    drop(outline);
    drop(table);
    drop(program);
    drop(interner);
    rel.fase("liberar memória", t);
    Ok((emitido, rel))
}

/// Grava `texto` em `p` só se o conteúdo atual for diferente (timestamp
/// intacto quando nada mudou: o Vite/HMR não recarrega). Devolve se gravou.
fn gravar_se_mudou(p: &std::path::Path, texto: &str, comparar: bool) -> Result<bool, String> {
    if !comparar {
        std::fs::write(p, texto).map_err(|e| format!("{}: {e}", p.display()))?;
        return Ok(true);
    }
    if let Ok(atual) = std::fs::read(p) {
        if atual == texto.as_bytes() {
            return Ok(false);
        }
    }
    std::fs::write(p, texto).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(true)
}

/// Escreve os módulos e o `main.mjs` em `dir` e copia o `dart_sdk.js`.
///
/// Devolve quantos arquivos foram de fato gravados (os idênticos ao que já
/// estava no disco não são reescritos).
pub fn escrever(emitido: &Emitido, dir: &std::path::Path, dart_sdk_js: &std::path::Path) -> Result<usize, String> {
    escrever_opcoes(emitido, dir, dart_sdk_js, true)
}

/// Como [`escrever`], podendo pular a comparação com o arquivo em disco.
///
/// A sessão residente (`dartforge dev`) já sabe, pelo texto da compilação
/// anterior que mantém em memória, quais módulos mudaram: reler o arquivo
/// para comparar custaria uma leitura de megabytes por módulo grande.
pub fn escrever_opcoes(
    emitido: &Emitido,
    dir: &std::path::Path,
    dart_sdk_js: &std::path::Path,
    comparar: bool,
) -> Result<usize, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut escritos = 0usize;
    for (path, text) in &emitido.modulos {
        let p = dir.join(path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        escritos += usize::from(gravar_se_mudou(&p, text, comparar)?);
    }
    escritos += usize::from(gravar_se_mudou(&dir.join("main.mjs"), &emitido.entrada, true)?);
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
