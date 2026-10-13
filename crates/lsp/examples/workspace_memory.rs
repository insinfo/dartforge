//! Where the memory of a workspace package's typed analysis goes.
//!
//! `cargo run --release -p dartforge-lsp --example workspace_memory -- <package root> [sdk lib]`
//!
//! With the counting allocator, in three parts:
//!
//! 1. loads the package's program the way the typed analysis does (every
//!    library of the package as an entry, the SDK from its cache) and prints
//!    the live bytes, the largest packages, the sources and arenas split into
//!    SDK, dependencies and the package's own units, and what each structure
//!    frees when dropped (each arena with the heap its nodes own, the
//!    interner, the elements, the libraries' scopes);
//! 2. runs the engine's first phases over the same load (outline, the
//!    initializers' pass) with the growth of each;
//! 3. runs the whole analysis (`Engine::analyze_with`) with the live bytes at
//!    its cancellation points, its peak and what stays live after it.
//!
//! `WORKSPACE_MEMORY_DUMP=<file>` writes the program's units (URI and path).
//! The numbers behind `docs/LSP.md`, "Memória".
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn mib(b: usize) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

#[derive(Default)]
struct Share {
    units: usize,
    source: usize,
    arena_len: usize,
    arena_cap: usize,
}

fn arenas(ast: &dartforge_frontend::ast::Ast) -> (usize, usize) {
    fn v<T>(x: &Vec<T>) -> (usize, usize) {
        (x.len() * std::mem::size_of::<T>(), x.capacity() * std::mem::size_of::<T>())
    }
    let parts = [
        v(&ast.exprs),
        v(&ast.stmts),
        v(&ast.types),
        v(&ast.patterns),
        v(&ast.decls),
        v(&ast.members),
        v(&ast.functions),
    ];
    parts.iter().fold((0, 0), |(a, b), (x, y)| (a + x, b + y))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = PathBuf::from(&args[0]);
    let sdk_lib = args
        .get(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var("DARTFORGE_SDK_LIB").ok().map(PathBuf::from))
        .expect("SDK lib (argument or DARTFORGE_SDK_LIB)");
    let engine = dartforge_parity::analysis::Engine::new(&sdk_lib).expect("SDK");
    let options = dartforge_parity::filters::AnalysisOptions::read(&root);
    let files = dartforge_parity::projects::files(&root, &options);
    let package_config = root.join(".dart_tool").join("package_config.json");
    let package_config = package_config.is_file().then_some(package_config);
    println!("package {} ({} files)", root.display(), files.len());

    // 1. The load alone, with the breakdown.
    let base = dartforge_instrument::live_bytes();
    dartforge_instrument::reset_peak();
    {
        let sdk = dartforge_elements::SdkLayout::load(&sdk_lib, "dartdevc").expect("SDK");
        let cache = dartforge_elements::SdkCache::open_or_build(&sdk, "dartdevc").ok().map(|(c, _)| c);
        let after_cache = dartforge_instrument::live_bytes();
        let libraries: Vec<&Path> = files
            .iter()
            .filter(|f| std::fs::read_to_string(f).is_ok_and(|t| !dartforge_parity::analysis::is_part(&t)))
            .map(PathBuf::as_path)
            .collect();
        let mut interner = dartforge_intern::Interner::new();
        let (program, _diags) = dartforge_elements::load::load_lenient_entries(
            &libraries,
            &sdk,
            package_config.as_deref(),
            &mut interner,
            cache,
            None,
            None,
        );
        let live = dartforge_instrument::live_bytes();
        println!(
            "load: {:.1} MiB live (SDK cache handle {:.1} MiB), peak {:.1} MiB, {} units, {} libraries",
            mib(live - base),
            mib(after_cache - base),
            mib(dartforge_instrument::peak_bytes() - base),
            program.units.len(),
            program.libraries.len()
        );
        let mut shares: HashMap<&str, Share> = HashMap::new();
        for u in &program.units {
            let group = if program.library(u.library).is_sdk {
                "sdk"
            } else if u.path.as_deref().is_some_and(|p| p.starts_with(&root)) {
                "own"
            } else {
                "deps"
            };
            let s = shares.entry(group).or_default();
            s.units += 1;
            s.source += u.source.len();
            let (l, c) = arenas(&u.ast);
            s.arena_len += l;
            s.arena_cap += c;
        }
        let mut by_package: HashMap<String, (usize, usize)> = HashMap::new();
        for u in &program.units {
            let name = u.uri.strip_prefix("package:").and_then(|r| r.split('/').next()).unwrap_or(if u.uri.starts_with("dart:") { "dart:" } else { "file:" });
            let e = by_package.entry(name.to_string()).or_default();
            e.0 += 1;
            e.1 += u.source.len();
        }
        if let Ok(dump) = std::env::var("WORKSPACE_MEMORY_DUMP") {
            let lines: Vec<String> = program.units.iter().map(|u| format!("{}\t{}", u.uri, u.path.as_deref().map(|p| p.display().to_string()).unwrap_or_default())).collect();
            let _ = std::fs::write(dump, lines.join("\n"));
        }
        let mut by_package: Vec<_> = by_package.into_iter().collect();
        by_package.sort_by_key(|(_, (_, b))| std::cmp::Reverse(*b));
        for (name, (n, b)) in by_package.iter().take(12) {
            println!("  package {name:30} units {n:5} source {:6.1} MiB", mib(*b));
        }
        for (g, s) in &shares {
            println!(
                "  {g:5} units {:5} source {:7.1} MiB arenas {:7.1} MiB (capacity {:7.1} MiB)",
                s.units,
                mib(s.source),
                mib(s.arena_len),
                mib(s.arena_cap)
            );
        }
        println!(
            "  elements: classes {} functions {} variables {}",
            program.classes.len(),
            program.functions.len(),
            program.variables.len()
        );
        println!(
            "  node sizes: Expr {} Stmt {} Type {} Pattern {} Decl {} Member {} Function {}",
            std::mem::size_of::<dartforge_frontend::ast::Expr>(),
            std::mem::size_of::<dartforge_frontend::ast::Stmt>(),
            std::mem::size_of::<dartforge_frontend::ast::TypeAnnotation>(),
            std::mem::size_of::<dartforge_frontend::ast::Pattern>(),
            std::mem::size_of::<dartforge_frontend::ast::Decl>(),
            std::mem::size_of::<dartforge_frontend::ast::Member>(),
            std::mem::size_of::<dartforge_frontend::ast::Function>()
        );
        // What each arena holds (with the heap its nodes own), and what remains
        // without the arenas and the sources: drop them one by one.
        let mut program = program;
        let mut freed = |label: &str, f: &mut dyn FnMut(&mut dartforge_frontend::ast::Ast)| {
            let before = dartforge_instrument::live_bytes();
            for u in &mut program.units {
                f(&mut u.ast);
            }
            println!("  {label:9} {:7.1} MiB", mib(before - dartforge_instrument::live_bytes()));
        };
        freed("exprs", &mut |a| a.exprs = Vec::new());
        freed("stmts", &mut |a| a.stmts = Vec::new());
        freed("types", &mut |a| a.types = Vec::new());
        freed("patterns", &mut |a| a.patterns = Vec::new());
        freed("decls", &mut |a| a.decls = Vec::new());
        freed("members", &mut |a| a.members = Vec::new());
        freed("functions", &mut |a| a.functions = Vec::new());
        freed("rest", &mut |a| *a = Default::default());
        let no_ast = dartforge_instrument::live_bytes();
        for u in &mut program.units {
            u.source = String::new();
        }
        let no_source = dartforge_instrument::live_bytes();
        println!(
            "  the sources {:.1} MiB; {:.1} MiB remain (elements, directives, interner)",
            mib(no_ast - no_source),
            mib(no_source - base)
        );
        let before = dartforge_instrument::live_bytes();
        drop(interner);
        println!("  interner {:.1} MiB", mib(before - dartforge_instrument::live_bytes()));
        let before = dartforge_instrument::live_bytes();
        for u in &mut program.units {
            u.unit = Default::default();
        }
        println!("  compilation units (directives, declaration lists) {:.1} MiB", mib(before - dartforge_instrument::live_bytes()));
        let before = dartforge_instrument::live_bytes();
        program.functions = Vec::new();
        println!("  function elements {:.1} MiB", mib(before - dartforge_instrument::live_bytes()));
        let before = dartforge_instrument::live_bytes();
        program.variables = Vec::new();
        println!("  variable elements {:.1} MiB", mib(before - dartforge_instrument::live_bytes()));
        let before = dartforge_instrument::live_bytes();
        program.classes = Vec::new();
        println!("  class elements {:.1} MiB", mib(before - dartforge_instrument::live_bytes()));
        let before = dartforge_instrument::live_bytes();
        program.libraries = Vec::new();
        println!("  libraries (scopes, imports) {:.1} MiB", mib(before - dartforge_instrument::live_bytes()));
    }
    println!("after dropping the load: {:.1} MiB live", mib(dartforge_instrument::live_bytes() - base));

    // 2. The engine's first phases, one by one, over the same load.
    {
        let base = dartforge_instrument::live_bytes();
        let sdk = dartforge_elements::SdkLayout::load(&sdk_lib, "dartdevc").expect("SDK");
        let cache = dartforge_elements::SdkCache::open_or_build(&sdk, "dartdevc").ok().map(|(c, _)| c);
        let libraries: Vec<&Path> = files
            .iter()
            .filter(|f| std::fs::read_to_string(f).is_ok_and(|t| !dartforge_parity::analysis::is_part(&t)))
            .map(PathBuf::as_path)
            .collect();
        let mut interner = dartforge_intern::Interner::new();
        let (program, _diags) = dartforge_elements::load::load_lenient_entries(&libraries, &sdk, package_config.as_deref(), &mut interner, cache, None, None);
        let step = |label: &str, from: usize| {
            let now = dartforge_instrument::live_bytes();
            println!("  {label:40} +{:7.1} MiB (live {:7.1} MiB)", mib(now.saturating_sub(from)), mib(now - base));
            now
        };
        let mut at = step("load", base);
        let mut table = dartforge_types::TypeTable::new();
        table.preserve_decoration = true;
        let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, _, _) = dartforge_types::resolve_outline_with_units(&program, &interner, &mut table, &core);
        at = step("outline", at);
        dartforge_instrument::reset_peak();
        let (bodies, diags) = dartforge_types::infer_library_bodies(&program, &interner, &mut table, &core, &mut outline, &[]);
        let peak = dartforge_instrument::peak_bytes();
        at = step("initializers (result kept)", at);
        println!("  {:40} peak {:7.1} MiB", "", mib(peak - base));
        drop(bodies);
        drop(diags);
        at = step("initializers' side tables dropped", at);
        drop(outline);
        at = step("outline dropped", at);
        drop(table);
        step("type table dropped", at);
    }

    // 3. The whole analysis.
    dartforge_instrument::reset_peak();
    let start = std::time::Instant::now();
    let base = dartforge_instrument::live_bytes();
    let calls = std::cell::Cell::new(0usize);
    let result = engine
        .analyze_with(&root, &files, package_config.as_deref(), &HashMap::new(), &|| {
            // The cancellation points: after the load, after the declaration
            // checks, then before each library's bodies.
            let n = calls.get() + 1;
            calls.set(n);
            if n <= 3 || n.is_multiple_of(100) {
                println!(
                    "  cancellation point {n}: {:.1} MiB live, peak so far {:.1} MiB",
                    mib(dartforge_instrument::live_bytes() - base),
                    mib(dartforge_instrument::peak_bytes() - base)
                );
            }
            false
        })
        .expect("not cancelled");
    println!(
        "analysis: peak {:.1} MiB, {:.1} MiB live with the result ({} files), {:.1} s",
        mib(dartforge_instrument::peak_bytes() - base),
        mib(dartforge_instrument::live_bytes() - base),
        result.files.len(),
        start.elapsed().as_secs_f64()
    );
    drop(result);
    println!("after dropping it: {:.1} MiB live", mib(dartforge_instrument::live_bytes() - base));
}
