//! Mede memória e tempo do front-end novo sobre um projeto inteiro, com todas
//! as árvores retidas — o que um servidor de linguagem faz com o projeto aberto.
//!
//! `cargo run -q --release -p dartforge-frontend --example memoria -- C:/MyDartProjects/new_sali`
//!
//! Imprime bytes de fonte, bytes vivos com tudo retido, pico, razão vivo/fonte,
//! alocações e tempo. A referência que este número enfrenta está no PLANO.md:
//! o LSP do Dart chega a ~6 GB e o `webdev` a ~10 GB neste mesmo projeto.
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use std::path::{Path, PathBuf};
use std::time::Instant;

fn coletar(dir: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.is_dir() {
            if caminho
                .file_name()
                .is_some_and(|n| n == ".dart_tool" || n == "build")
            {
                continue;
            }
            coletar(&caminho, saida);
        } else if caminho.extension().is_some_and(|e| e == "dart") {
            saida.push(caminho);
        }
    }
}

fn main() {
    let mut arquivos = Vec::new();
    for raiz in std::env::args().skip(1) {
        coletar(Path::new(&raiz), &mut arquivos);
    }
    arquivos.sort();
    let inicio = Instant::now();
    let base = dartforge_instrument::live_bytes();
    dartforge_instrument::reset_peak();
    let alocacoes_base = dartforge_instrument::allocation_count();
    let mut nomes = dartforge_intern::Interner::new();
    let mut retidas = Vec::with_capacity(arquivos.len());
    let mut bytes_fonte = 0usize;
    let mut recusados = 0usize;
    let mut tokens_total = 0usize;
    for arquivo in &arquivos {
        let Ok(fonte) = std::fs::read_to_string(arquivo) else {
            continue;
        };
        bytes_fonte += fonte.len();
        let saida = dartforge_frontend::parser::parse(&fonte, &mut nomes);
        if !saida.diagnostics.is_empty() {
            recusados += 1;
        }
        tokens_total += saida.ast.exprs.len();
        // Fonte e árvore ficam vivas, como num editor com o projeto aberto.
        retidas.push((fonte, saida));
    }
    let tempo = inicio.elapsed();
    // Onde os bytes estão: cada arena, contagem × tamanho do nó (capacidade
    // reservada à parte, porque `Vec` cresce em potências de dois).
    {
        use dartforge_frontend::ast::*;
        use std::mem::size_of;
        let mut n = [0usize; 7];
        let mut cap = [0usize; 7];
        for (_, saida) in &retidas {
            let a = &saida.ast;
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
        let nomes = [
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
            let usado = n[i] * tam[i];
            let reservado = cap[i] * tam[i];
            total += reservado;
            println!(
                "  {:<10} {:>8} nós × {:>3} B = {:>7.2} MiB usados, {:>7.2} MiB reservados",
                nomes[i],
                n[i],
                tam[i],
                usado as f64 / 1048576.0,
                reservado as f64 / 1048576.0
            );
        }
        println!(
            "  arenas reservadas: {:.2} MiB (o resto é Vec/Box dentro dos nós e as fontes)",
            total as f64 / 1048576.0
        );
        // Histograma de variantes de expressão, com o que cada uma carrega
        // fora da arena (bytes de `Vec`/`Box`), para cortar onde há volume.
        let mut hist: std::collections::BTreeMap<&'static str, (usize, usize)> = Default::default();
        for (_, saida) in &retidas {
            for e in &saida.ast.exprs {
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
                };
                let entrada = hist.entry(nome).or_default();
                entrada.0 += 1;
                entrada.1 += fora;
            }
        }
        let mut ordem: Vec<_> = hist.into_iter().collect();
        ordem.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
        println!("  variantes de expressão (contagem, bytes fora da arena):");
        for (nome, (n, fora)) in ordem.iter().take(14) {
            println!(
                "    {:<18} {:>8}  {:>7.2} MiB fora",
                nome,
                n,
                *fora as f64 / 1048576.0
            );
        }
    }
    let vivos = dartforge_instrument::live_bytes() - base;
    let pico = dartforge_instrument::peak_bytes() - base;
    let alocacoes = dartforge_instrument::allocation_count() - alocacoes_base;
    let mib = |b: usize| b as f64 / (1024.0 * 1024.0);
    println!(
        "arquivos: {} ({} recusados pelo parser)",
        retidas.len(),
        recusados
    );
    println!("fonte: {:.2} MiB", mib(bytes_fonte));
    println!(
        "vivo com tudo retido: {:.2} MiB ({:.2}x a fonte)",
        mib(vivos),
        vivos as f64 / bytes_fonte as f64
    );
    println!("pico: {:.2} MiB", mib(pico));
    println!(
        "nós de expressão: {tokens_total}; símbolos internados: {} ({:.2} MiB)",
        nomes.len(),
        mib(nomes.payload_bytes())
    );
    println!(
        "alocações: {alocacoes}; tempo: {:.0} ms ({:.1} MiB/s)",
        tempo.as_secs_f64() * 1e3,
        mib(bytes_fonte) / tempo.as_secs_f64()
    );
    drop(retidas);
    println!(
        "após descartar tudo: {} bytes acima da base",
        dartforge_instrument::live_bytes().saturating_sub(base)
    );
}
