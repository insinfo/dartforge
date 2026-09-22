//! Mede memória (vivos e pico) e tempo de cada fase do `compile-js`, com o
//! alocador contador de `crates/instrument` — o mesmo harness das fases
//! anteriores (`crates/frontend/examples/memoria.rs`, `crates/types/examples/memoria.rs`).
//!
//! `cargo run -q --release -p dartforge-emit-js --example memoria -- <entrada.dart> [package_config.json] [dir_saida]`
//!
//! Sem argumentos usa `C:/MyDartProjects/new_sali/core/test/arvore_processo_item_test.dart`
//! e o `package_config.json` de `new_sali/core`.
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_elements::load::load_lenient_com_cache;
use dartforge_elements::sdk::SdkLayout;
use dartforge_elements::SdkCache;
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use std::path::PathBuf;
use std::time::Instant;

const MB: f64 = 1_048_576.0;

struct Sonda {
    linhas: Vec<(&'static str, f64, usize, usize, usize)>,
    inicio: Instant,
    alocacoes: usize,
}

impl Sonda {
    fn nova() -> Self {
        dartforge_instrument::reset_peak();
        Self { linhas: Vec::new(), inicio: Instant::now(), alocacoes: dartforge_instrument::allocation_count() }
    }
    /// Registra a fase que acabou: tempo, vivos, pico desde o início e alocações da fase.
    fn fase(&mut self, nome: &'static str) {
        let alocacoes = dartforge_instrument::allocation_count();
        self.linhas.push((
            nome,
            self.inicio.elapsed().as_secs_f64() * 1000.0,
            dartforge_instrument::live_bytes(),
            dartforge_instrument::peak_bytes(),
            alocacoes - self.alocacoes,
        ));
        self.inicio = Instant::now();
        self.alocacoes = alocacoes;
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let entrada = PathBuf::from(
        args.first().cloned().unwrap_or_else(|| "C:/MyDartProjects/new_sali/core/test/arvore_processo_item_test.dart".into()),
    );
    let packages = match args.get(1) {
        Some(p) => Some(PathBuf::from(p)),
        None => {
            let p = PathBuf::from("C:/MyDartProjects/new_sali/core/.dart_tool/package_config.json");
            p.exists().then_some(p)
        }
    };
    let saida = PathBuf::from(args.get(2).cloned().unwrap_or_else(|| "target/memoria-emit-js".into()));
    if !entrada.exists() {
        eprintln!("entrada não existe: {}", entrada.display());
        return;
    }

    println!("== compile-js: memória e tempo por fase — {}", entrada.display());
    let base = dartforge_instrument::live_bytes();
    let mut sonda = Sonda::nova();

    let sdk_dir = SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let sdk = SdkLayout::load(&sdk_dir, "dartdevc").expect("libraries.json");
    let (cache, lido) = SdkCache::abrir_ou_construir(&sdk, "dartdevc").expect("cache do SDK");
    sonda.fase(if lido { "cache do SDK (lido)" } else { "cache do SDK (construído)" });

    let mut interner = Interner::new();
    let (program, diags) = load_lenient_com_cache(&entrada, &sdk, packages.as_deref(), &mut interner, Some(cache));
    sonda.fase("carregar programa");
    if !diags.is_empty() {
        eprintln!("{} diagnóstico(s) de carregamento; o primeiro: {}", diags.len(), diags[0]);
        return;
    }
    let unidades_sdk = program.units.iter().filter(|u| program.library(u.library).is_sdk).count();
    let unidades_usuario = program.units.len() - unidades_sdk;

    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, outline_diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    sonda.fase("outline (tipos)");

    let (bodies, body_diags) =
        dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
    sonda.fase("inferência de corpos");

    let emitido = match dartforge_emit_js::emitir_programa(&program, &interner, &table, &core, &outline, &bodies) {
        Ok(e) => e,
        Err(ds) => {
            eprintln!("emissão falhou: {}", ds[0]);
            return;
        }
    };
    sonda.fase("emissão");

    drop(bodies);
    drop(outline);
    drop(table);
    drop(program);
    drop(interner);
    sonda.fase("liberar memória");

    let escritos = dartforge_emit_js::escrever(&emitido, &saida, &dartforge_emit_js::dart_sdk_js_padrao()).expect("escrita");
    sonda.fase("escrita");
    let modulos = emitido.modulos.len();
    let js_bytes: usize = emitido.modulos.iter().map(|(_, t)| t.len()).sum::<usize>() + emitido.entrada.len();
    drop(emitido);
    sonda.fase("fim (JS liberado)");

    println!("{:<28}{:>10}{:>12}{:>12}{:>12}", "fase", "ms", "vivos MB", "pico MB", "alocações");
    for (nome, ms, vivos, pico, alocs) in &sonda.linhas {
        println!(
            "{nome:<28}{ms:>10.1}{:>12.2}{:>12.2}{alocs:>12}",
            vivos.saturating_sub(base) as f64 / MB,
            pico.saturating_sub(base) as f64 / MB
        );
    }
    println!(
        "unidades: {} SDK + {} usuário/pacotes; módulos: {} ({:.2} MB de JS, {} arquivo(s) gravado(s)); avisos: {} outline + {} corpos",
        unidades_sdk,
        unidades_usuario,
        modulos,
        js_bytes as f64 / MB,
        escritos,
        outline_diags.len(),
        body_diags.len()
    );
}
