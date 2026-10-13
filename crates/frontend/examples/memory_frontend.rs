//! Measures memory and time of the new front end over a whole project, with
//! every tree retained — what a language server does with the project open.
//!
//! `cargo run -q --release -p dartforge-frontend --example memory_frontend -- C:/MyDartProjects/new_sali`
//!
//! Prints source bytes, live bytes with everything retained, peak, live/source
//! ratio, allocations and time. The reference this number is up against is in
//! PLANO.md: the Dart LSP reaches ~6 GB and `webdev` ~10 GB on this same
//! project.
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use std::path::{Path, PathBuf};
use std::time::Instant;

fn collect(dir: &Path, output: &mut Vec<PathBuf>) {
    let Ok(inputs) = std::fs::read_dir(dir) else {
        return;
    };
    for input in inputs.flatten() {
        let path = input.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n == ".dart_tool" || n == "build")
            {
                continue;
            }
            collect(&path, output);
        } else if path.extension().is_some_and(|e| e == "dart") {
            output.push(path);
        }
    }
}

fn main() {
    let mut files = Vec::new();
    for root in std::env::args().skip(1) {
        collect(Path::new(&root), &mut files);
    }
    files.sort();
    let start_offset = Instant::now();
    let base = dartforge_instrument::live_bytes();
    dartforge_instrument::reset_peak();
    let base_allocations = dartforge_instrument::allocation_count();
    let mut names = dartforge_intern::Interner::new();
    let mut retained = Vec::with_capacity(files.len());
    let mut bytes_fonte = 0usize;
    let mut rejected = 0usize;
    let mut total_exprs = 0usize;
    for file in &files {
        let Ok(fonte) = std::fs::read_to_string(file) else {
            continue;
        };
        bytes_fonte += fonte.len();
        let output = dartforge_frontend::parser::parse(&fonte, &mut names);
        if !output.diagnostics.is_empty() {
            rejected += 1;
        }
        total_exprs += output.ast.exprs.len();
        // Source and tree stay alive, as in an editor with the project open.
        retained.push((fonte, output));
    }
    let elapsed_time = start_offset.elapsed();
    // Where the bytes are: each arena, count × node size (reserved capacity
    // separately, because `Vec` grows in powers of two).
    {
        use dartforge_frontend::ast::*;
        use std::mem::size_of;
        let mut n = [0usize; 7];
        let mut cap = [0usize; 7];
        for (_, output) in &retained {
            let a = &output.ast;
            for (i, (len, c)) in [
                (a.exprs.len(), a.exprs.capacity()),
                (a.stmts.len(), a.stmts.capacity()),
                (a.types.len(), a.types.capacity()),
                (a.patterns.len(), a.patterns.capacity()),
                (a.decls.len(), a.decls.capacity()),
                (a.members.len(), a.members.capacity()),
                (a.functions.len(), a.functions.capacity()),
            ]
            .into_iter()
            .enumerate()
            {
                n[i] += len;
                cap[i] += c;
            }
        }
        let tam = [
            size_of::<Expr>(),
            size_of::<Stmt>(),
            size_of::<TypeAnnotation>(),
            size_of::<Pattern>(),
            size_of::<Decl>(),
            size_of::<Member>(),
            size_of::<Function>(),
        ];
        let names = [
            "exprs",
            "stmts",
            "types",
            "patterns",
            "decls",
            "members",
            "functions",
        ];
        let mut total = 0usize;
        for i in 0..7 {
            let used = n[i] * tam[i];
            let reserved = cap[i] * tam[i];
            total += reserved;
            println!(
                "  {:<10} {:>8} nodes × {:>3} B = {:>7.2} MiB used, {:>7.2} MiB reserved",
                names[i],
                n[i],
                tam[i],
                used as f64 / 1048576.0,
                reserved as f64 / 1048576.0
            );
        }
        println!(
            "  reserved arenas: {:.2} MiB (the rest is Vec/Box inside the nodes and the sources)",
            total as f64 / 1048576.0
        );
        // Histogram of expression variants, with what each one carries outside the
        // arena (`Vec`/`Box` bytes), to cut where the volume is.
        let mut hist: std::collections::BTreeMap<&'static str, (usize, usize)> = Default::default();
        for (_, output) in &retained {
            for e in &output.ast.exprs {
                let (nome, fora) = match &e.kind {
                    ExprKind::Int(_) => ("Int", 0),
                    ExprKind::Double(_) => ("Double", 0),
                    ExprKind::Bool(_) => ("Bool", 0),
                    ExprKind::Null => ("Null", 0),
                    ExprKind::String(l) => (
                        "String",
                        l.parts.len() * size_of::<StringPart>()
                            + l.parts
                                .iter()
                                .map(|p| match p {
                                    StringPart::Text(t) => t.as_bytes().len(),
                                    _ => 0,
                                })
                                .sum::<usize>(),
                    ),
                    ExprKind::Symbol(v) => ("Symbol", v.len() * size_of::<Name>()),
                    ExprKind::Identifier(_) => ("Identifier", 0),
                    ExprKind::This => ("This", 0),
                    ExprKind::Super => ("Super", 0),
                    ExprKind::Parenthesized(_) => ("Parenthesized", 0),
                    ExprKind::List {
                        type_args,
                        elements,
                        ..
                    } => (
                        "List",
                        type_args.len() * 4 + elements.len() * size_of::<CollectionElement>(),
                    ),
                    ExprKind::SetOrMap {
                        type_args,
                        elements,
                        ..
                    } => (
                        "SetOrMap",
                        type_args.len() * 4 + elements.len() * size_of::<CollectionElement>(),
                    ),
                    ExprKind::Record {
                        positional, named, ..
                    } => (
                        "Record",
                        positional.len() * 4 + named.len() * size_of::<(Name, ExprId)>(),
                    ),
                    ExprKind::InstanceCreation { arguments, .. } => (
                        "InstanceCreation",
                        size_of::<Arguments>()
                            + arguments.args.len() * size_of::<Argument>()
                            + arguments.type_args.len() * 4,
                    ),
                    ExprKind::FunctionExpression(_) => ("FunctionExpression", 0),
                    ExprKind::Property { .. } => ("Property", 0),
                    ExprKind::Index { .. } => ("Index", 0),
                    ExprKind::Call { arguments, .. } => (
                        "Call",
                        size_of::<Arguments>()
                            + arguments.args.len() * size_of::<Argument>()
                            + arguments.type_args.len() * 4,
                    ),
                    ExprKind::TypeArguments { type_args, .. } => {
                        ("TypeArguments", type_args.len() * 4)
                    }
                    ExprKind::Unary { .. } => ("Unary", 0),
                    ExprKind::Binary { .. } => ("Binary", 0),
                    ExprKind::Conditional { .. } => ("Conditional", 0),
                    ExprKind::Is { .. } => ("Is", 0),
                    ExprKind::As { .. } => ("As", 0),
                    ExprKind::Assign { .. } => ("Assign", 0),
                    ExprKind::PatternAssign { .. } => ("PatternAssign", 0),
                    ExprKind::Cascade { sections, .. } => ("Cascade", sections.len() * 4),
                    ExprKind::CascadeTarget => ("CascadeTarget", 0),
                    ExprKind::Await(_) => ("Await", 0),
                    ExprKind::Throw(_) => ("Throw", 0),
                    ExprKind::Rethrow => ("Rethrow", 0),
                    ExprKind::Switch { cases, .. } => {
                        ("Switch", cases.len() * size_of::<SwitchExprCase>())
                    }
                    ExprKind::DotShorthand { .. } => ("DotShorthand", 0),
                };
                let input = hist.entry(nome).or_default();
                input.0 += 1;
                input.1 += fora;
            }
        }
        let mut order: Vec<_> = hist.into_iter().collect();
        order.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
        println!("  expression variants (count, bytes outside the arena):");
        for (nome, (n, fora)) in order.iter().take(14) {
            println!(
                "    {:<18} {:>8}  {:>7.2} MiB fora",
                nome,
                n,
                *fora as f64 / 1048576.0
            );
        }
    }
    let live = dartforge_instrument::live_bytes() - base;
    let pico = dartforge_instrument::peak_bytes() - base;
    let allocations_ = dartforge_instrument::allocation_count() - base_allocations;
    let mib = |b: usize| b as f64 / (1024.0 * 1024.0);
    println!(
        "files: {} ({} rejected by the parser)",
        retained.len(),
        rejected
    );
    println!("source: {:.2} MiB", mib(bytes_fonte));
    println!(
        "live with everything retained: {:.2} MiB ({:.2}x the source)",
        mib(live),
        live as f64 / bytes_fonte as f64
    );
    println!("pico: {:.2} MiB", mib(pico));
    println!(
        "expression nodes: {total_exprs}; interned symbols: {} ({:.2} MiB)",
        names.len(),
        mib(names.payload_bytes())
    );
    println!(
        "allocations: {allocations_}; time: {:.0} ms ({:.1} MiB/s)",
        elapsed_time.as_secs_f64() * 1e3,
        mib(bytes_fonte) / elapsed_time.as_secs_f64()
    );
    drop(retained);
    println!(
        "after dropping everything: {} bytes above the baseline",
        dartforge_instrument::live_bytes().saturating_sub(base)
    );
}
