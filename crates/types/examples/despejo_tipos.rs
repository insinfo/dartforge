//! Despejo de tipos: grava, por expressão de corpo, o tipo estático que a
//! nossa inferência deu, no mesmo formato do oráculo do `package:analyzer`
//! (`tools/oraculo_tipos/oraculo.dart`). É ferramenta de diagnóstico da
//! inferência, não produto.
//!
//! ```text
//! cargo run --release -p dartforge-types --example despejo_tipos -- \
//!     <entrada.dart> --packages <package_config.json> -o <saida.tsv> \
//!     [--arquivos <lista.txt>] [--avisos <avisos.txt>] [--sdk <lib>]
//! ```
//!
//! Linha: `caminho \t offset \t comprimento \t nó \t tipo \t resolução`.
//! Expressão que a inferência nunca visitou sai com o tipo `?`.
//! `--arquivos` recebe a lista de unidades (não-SDK) para o oráculo;
//! `--avisos` recebe os avisos, um por linha, com o arquivo.

use dartforge_elements::load::load_lenient;
use dartforge_elements::model::Program;
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::ast::ExprKind;
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_types::{resolve_outline, BodyInferrer, Resolved};
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut entrada = None;
    let mut packages = None;
    let mut saida = None;
    let mut arquivos = None;
    let mut avisos = None;
    let mut sdk_dir = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--packages" => packages = it.next().map(PathBuf::from),
            "-o" => saida = it.next().map(PathBuf::from),
            "--arquivos" => arquivos = it.next().map(PathBuf::from),
            "--avisos" => avisos = it.next().map(PathBuf::from),
            "--sdk" => sdk_dir = it.next().map(PathBuf::from),
            _ => entrada = Some(PathBuf::from(a)),
        }
    }
    let entrada = entrada.expect("uso: despejo_tipos <entrada.dart> --packages <cfg> -o <saida.tsv>");
    let sdk_dir = sdk_dir
        .or_else(SdkLayout::discover)
        .unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let sdk = SdkLayout::load(&sdk_dir, "dartdevc").expect("SDK");

    // Corpos profundos recursam fundo: pilha própria, como o `compile-js`.
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let mut interner = Interner::new();
            let (prog, _) = load_lenient(&entrada, &sdk, packages.as_deref(), &mut interner);
            let mut table = TypeTable::new();
            let core = CoreTypes::init(&mut table, &prog, &interner);
            let (mut outline, diags_outline) = resolve_outline(&prog, &interner, &mut table, &core);
            let t = std::time::Instant::now();
            let mut inf = BodyInferrer::new(&prog, &interner, &mut table, &core, &mut outline);
            // Sentinela: o que continuar com ela não foi visitado.
            for u in &mut inf.body_types.units {
                u.static_types.fill(NAO_VISITADA);
            }
            let (bodies, diags) = inf.infer_all();
            eprintln!(
                "inferência: {:?}; avisos: {} (outline {} + corpos {})",
                t.elapsed(),
                diags_outline.len() + diags.len(),
                diags_outline.len(),
                diags.len()
            );

            let mut out = std::io::BufWriter::new(std::fs::File::create(saida.expect("-o")).unwrap());
            let mut lista = Vec::new();
            let (mut total, mut nao_visitadas) = (0usize, 0usize);
            for (ui, unit) in prog.units.iter().enumerate() {
                if prog.library(unit.library).is_sdk {
                    continue;
                }
                let Some(path) = &unit.path else { continue };
                let caminho = path.to_string_lossy().replace('\\', "/");
                lista.push(caminho.clone());
                let bt = &bodies.units[ui];
                for (i, e) in unit.ast.exprs.iter().enumerate() {
                    let ty = bt.static_types.get(i).copied().unwrap_or(NAO_VISITADA);
                    let tipo = if ty == NAO_VISITADA {
                        nao_visitadas += 1;
                        "?".to_string()
                    } else {
                        formatar(&table, ty, &interner, &prog)
                    };
                    total += 1;
                    let res = bt.resolved.get(i).and_then(|r| r.as_ref()).map(|r| resolucao(r, &prog, &interner)).unwrap_or_else(|| "-".into());
                    writeln!(
                        out,
                        "{caminho}\t{}\t{}\t{}\t{tipo}\t{res}",
                        e.span.start,
                        e.span.end - e.span.start,
                        nome_no(&e.kind)
                    )
                    .unwrap();
                }
            }
            eprintln!("despejo: {} unidades, {total} expressões, {nao_visitadas} não visitadas", lista.len());
            if let Some(a) = arquivos {
                std::fs::write(a, lista.join("\n")).unwrap();
            }
            if let Some(a) = avisos {
                let mut f = std::io::BufWriter::new(std::fs::File::create(a).unwrap());
                for d in diags_outline.iter().chain(diags.iter()) {
                    writeln!(f, "{}\t{}", d.span.start, d.message.replace('\n', " ")).unwrap();
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

const NAO_VISITADA: TypeId = TypeId(u32::MAX);

fn nome_no(k: &ExprKind) -> &'static str {
    match k {
        ExprKind::Int(_) => "Int",
        ExprKind::Double(_) => "Double",
        ExprKind::Bool(_) => "Bool",
        ExprKind::Null => "Null",
        ExprKind::String(_) => "String",
        ExprKind::Symbol(_) => "Symbol",
        ExprKind::Identifier(_) => "Identifier",
        ExprKind::This => "This",
        ExprKind::Super => "Super",
        ExprKind::Parenthesized(_) => "Parenthesized",
        ExprKind::List { .. } => "List",
        ExprKind::SetOrMap { .. } => "SetOrMap",
        ExprKind::Record { .. } => "Record",
        ExprKind::InstanceCreation { .. } => "InstanceCreation",
        ExprKind::FunctionExpression(_) => "FunctionExpression",
        ExprKind::Property { .. } => "Property",
        ExprKind::Index { .. } => "Index",
        ExprKind::Call { .. } => "Call",
        ExprKind::TypeArguments { .. } => "TypeArguments",
        ExprKind::Unary { .. } => "Unary",
        ExprKind::Binary { .. } => "Binary",
        ExprKind::Conditional { .. } => "Conditional",
        ExprKind::Is { .. } => "Is",
        ExprKind::As { .. } => "As",
        ExprKind::Assign { .. } => "Assign",
        ExprKind::PatternAssign { .. } => "PatternAssign",
        ExprKind::Cascade { .. } => "Cascade",
        ExprKind::CascadeTarget => "CascadeTarget",
        ExprKind::Await(_) => "Await",
        ExprKind::Throw(_) => "Throw",
        ExprKind::Rethrow => "Rethrow",
        ExprKind::Switch { .. } => "Switch",
        ExprKind::DotShorthand { .. } => "DotShorthand",
    }
}

fn resolucao(r: &Resolved, prog: &Program, i: &Interner) -> String {
    match r {
        Resolved::Local(_) => "LOCAL".into(),
        Resolved::Parameter { name, .. } => format!("PARAMETER:{}", i.resolve(*name)),
        Resolved::TypeParameter(_) => "TYPE_PARAMETER".into(),
        Resolved::Element(e) => format!("ELEMENT:{e:?}"),
        Resolved::Member { class, member, .. } => {
            let c = i.resolve(prog.class(*class).name);
            let n = match member {
                dartforge_types::MemberRef::Function(f) => i.resolve(prog.function(*f).name),
                dartforge_types::MemberRef::Variable(v) => i.resolve(prog.variable(*v).name),
            };
            format!("MEMBER:{c}.{n}")
        }
        Resolved::Prefix(_) => "PREFIX".into(),
        Resolved::Dynamic => "DYNAMIC".into(),
        Resolved::ExtensionMember { member, .. } => format!("EXTENSION:{}", i.resolve(prog.function(*member).name)),
        Resolved::Constructor(f) => format!("CONSTRUCTOR:{}", i.resolve(prog.function(*f).name)),
    }
}

/// Formata como o `DartType.getDisplayString()` do analyzer.
fn formatar(t: &TypeTable, ty: TypeId, i: &Interner, p: &Program) -> String {
    let q = |n: bool| if n { "?" } else { "" };
    match t.get(ty) {
        Type::Dynamic => "dynamic".into(),
        Type::Void => "void".into(),
        Type::Never => "Never".into(),
        Type::Null => "Null".into(),
        Type::Interface { class, args, nullable } | Type::ExtensionType { decl: class, args, nullable } => {
            let nome = i.resolve(p.class(*class).name);
            if args.is_empty() {
                format!("{nome}{}", q(*nullable))
            } else {
                let a: Vec<String> = args.iter().map(|&x| formatar(t, x, i, p)).collect();
                format!("{nome}<{}>{}", a.join(", "), q(*nullable))
            }
        }
        Type::FutureOr { arg, nullable } => format!("FutureOr<{}>{}", formatar(t, *arg, i, p), q(*nullable)),
        Type::TypeParameter { param, nullable } => format!("{}{}", i.resolve(t.param(*param).name), q(*nullable)),
        Type::Record { positional, named, nullable } => {
            let mut partes: Vec<String> = positional.iter().map(|&x| formatar(t, x, i, p)).collect();
            let mut nomeados: Vec<(String, String)> =
                named.iter().map(|(n, x)| (i.resolve(*n).to_string(), formatar(t, *x, i, p))).collect();
            nomeados.sort();
            if !nomeados.is_empty() {
                let n: Vec<String> = nomeados.iter().map(|(n, x)| format!("{x} {n}")).collect();
                partes.push(format!("{{{}}}", n.join(", ")));
            } else if positional.len() == 1 {
                return format!("({},){}", partes[0], q(*nullable));
            }
            format!("({}){}", partes.join(", "), q(*nullable))
        }
        Type::Function { type_params, ret, positional, optional, named, nullable } => {
            let mut s = format!("{} Function", formatar(t, *ret, i, p));
            if !type_params.is_empty() {
                let tps: Vec<String> = type_params
                    .iter()
                    .map(|&tp| {
                        let d = t.param(tp);
                        let nome = i.resolve(d.name).to_string();
                        match t.get(d.bound) {
                            Type::Interface { nullable: true, class, .. }
                                if i.resolve(p.class(*class).name) == "Object" =>
                            {
                                nome
                            }
                            Type::Dynamic => nome,
                            _ => format!("{nome} extends {}", formatar(t, d.bound, i, p)),
                        }
                    })
                    .collect();
                s.push_str(&format!("<{}>", tps.join(", ")));
            }
            let mut partes: Vec<String> = positional.iter().map(|&x| formatar(t, x, i, p)).collect();
            if !optional.is_empty() {
                let o: Vec<String> = optional.iter().map(|&x| formatar(t, x, i, p)).collect();
                partes.push(format!("[{}]", o.join(", ")));
            }
            if !named.is_empty() {
                let mut n: Vec<(String, String)> = named
                    .iter()
                    .map(|(n, x, r)| {
                        (i.resolve(*n).to_string(), format!("{}{} {}", if *r { "required " } else { "" }, formatar(t, *x, i, p), i.resolve(*n)))
                    })
                    .collect();
                n.sort();
                let n: Vec<String> = n.into_iter().map(|(_, s)| s).collect();
                partes.push(format!("{{{}}}", n.join(", ")));
            }
            format!("{s}({}){}", partes.join(", "), q(*nullable))
        }
    }
}
