//! Measures memory (live and peak) and time of each `compile-js` phase, with the
//! counting allocator from `crates/instrument` — the same harness as the earlier
//! phases (`crates/frontend/examples/memory_frontend.rs`, `crates/types/examples/memory_types.rs`).
//!
//! `cargo run -q --release -p dartforge-emit-js --example memory_emit_js -- <entry.dart> [package_config.json] [out_dir]`
//!
//! Without arguments it uses `C:/MyDartProjects/new_sali/core/test/arvore_processo_item_test.dart`
//! and the `package_config.json` of `new_sali/core`.
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_elements::SdkCache;
use dartforge_elements::load::load_lenient_with_cache;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use std::path::PathBuf;
use std::time::Instant;

const MB: f64 = 1_048_576.0;

struct Probe {
    lines: Vec<(&'static str, f64, usize, usize, usize)>,
    start: Instant,
    allocations: usize,
}

impl Probe {
    fn new() -> Self {
        dartforge_instrument::reset_peak();
        Self {
            lines: Vec::new(),
            start: Instant::now(),
            allocations: dartforge_instrument::allocation_count(),
        }
    }
    /// Records the phase that just ended: time, live bytes, peak since the start
    /// and the phase's allocations.
    fn phase(&mut self, name: &'static str) {
        let allocations = dartforge_instrument::allocation_count();
        self.lines.push((
            name,
            self.start.elapsed().as_secs_f64() * 1000.0,
            dartforge_instrument::live_bytes(),
            dartforge_instrument::peak_bytes(),
            allocations - self.allocations,
        ));
        self.start = Instant::now();
        self.allocations = allocations;
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let entry = PathBuf::from(args.first().cloned().unwrap_or_else(|| {
        "C:/MyDartProjects/new_sali/core/test/arvore_processo_item_test.dart".into()
    }));
    let packages = match args.get(1) {
        Some(p) => Some(PathBuf::from(p)),
        None => {
            let p = PathBuf::from("C:/MyDartProjects/new_sali/core/.dart_tool/package_config.json");
            p.exists().then_some(p)
        }
    };
    let out_dir = PathBuf::from(
        args.get(2)
            .cloned()
            .unwrap_or_else(|| "target/memoria-emit-js".into()),
    );
    if !entry.exists() {
        eprintln!("entry point does not exist: {}", entry.display());
        return;
    }

    println!(
        "== compile-js: memory and time per phase — {}",
        entry.display()
    );
    let base = dartforge_instrument::live_bytes();
    let mut probe = Probe::new();

    let sdk_dir =
        SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let sdk = SdkLayout::load(&sdk_dir, "dartdevc").expect("libraries.json");
    let (cache, read) = SdkCache::open_or_build(&sdk, "dartdevc").expect("SDK cache");
    probe.phase(if read {
        "SDK cache (read)"
    } else {
        "SDK cache (built)"
    });

    let mut interner = Interner::new();
    let (program, diags) = load_lenient_with_cache(
        &entry,
        &sdk,
        packages.as_deref(),
        &mut interner,
        Some(cache),
    );
    probe.phase("load program");
    if !diags.is_empty() {
        eprintln!(
            "{} load diagnostic(s); the first: {}",
            diags.len(),
            diags[0]
        );
        return;
    }
    let sdk_units = program
        .units
        .iter()
        .filter(|u| program.library(u.library).is_sdk)
        .count();
    let user_units = program.units.len() - sdk_units;
    {
        let c = &program.times;
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
        println!(
            "load: parallel read+lex {:.1} ms ({} waves), serial {:.1} ms ({} files, {:.2} MiB), parse {:.1} ms, SDK from cache {:.1} ms, directives {:.1} ms, outline decl {:.1} / reexports {:.1} / scopes {:.1} / supertypes {:.1} ms",
            ms(c.parallel_read_lex),
            c.waves,
            ms(c.read),
            c.files_read,
            c.bytes_read as f64 / MB,
            ms(c.parse),
            ms(c.sdk_cache),
            ms(c.directives),
            ms(c.outline_declarations),
            ms(c.outline_reexports),
            ms(c.outline_scopes),
            ms(c.outline_supertypes)
        );
    }

    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, outline_diags) =
        dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    probe.phase("outline (types)");

    let (bodies, body_diags) =
        dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
    probe.phase("body inference");

    let emitted = match dartforge_emit_js::emit_program(
        &program, &interner, &table, &core, &outline, &bodies,
    ) {
        Ok(e) => e,
        Err(ds) => {
            eprintln!("emission failed: {}", ds[0]);
            return;
        }
    };
    probe.phase("emission");
    {
        use dartforge_emit_js::js;
        use std::sync::atomic::Ordering::Relaxed;
        println!(
            "emission: Js built {} ({:.2} MB of intermediate text), at() borrows {} + parentheses {}, names/literals {}",
            js::JS_BUILT.load(Relaxed),
            js::JS_BYTES.load(Relaxed) as f64 / MB,
            js::AT_BORROWS.load(Relaxed),
            js::AT_PARENS.load(Relaxed),
            js::NAMES.load(Relaxed)
        );
    }

    drop(bodies);
    drop(outline);
    drop(table);
    drop(program);
    drop(interner);
    probe.phase("free memory");

    let written = dartforge_emit_js::write(
        &emitted,
        &out_dir,
        &dartforge_emit_js::default_dart_sdk_js(),
    )
    .expect("write");
    probe.phase("write");
    let modules = emitted.modules.len();
    let js_bytes: usize =
        emitted.modules.iter().map(|(_, t)| t.len()).sum::<usize>() + emitted.entry.len();
    drop(emitted);
    probe.phase("end (JS freed)");

    println!(
        "{:<28}{:>10}{:>12}{:>12}{:>12}",
        "phase", "ms", "live MB", "peak MB", "allocations"
    );
    for (name, ms, live, peak, allocs) in &probe.lines {
        println!(
            "{name:<28}{ms:>10.1}{:>12.2}{:>12.2}{allocs:>12}",
            live.saturating_sub(base) as f64 / MB,
            peak.saturating_sub(base) as f64 / MB
        );
    }
    println!(
        "units: {} SDK + {} user/packages; modules: {} ({:.2} MB of JS, {} file(s) written); warnings: {} outline + {} bodies",
        sdk_units,
        user_units,
        modules,
        js_bytes as f64 / MB,
        written,
        outline_diags.len(),
        body_diags.len()
    );
}
