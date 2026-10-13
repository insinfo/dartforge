//! Type dump and comparison with the oracle (`package:analyzer`,
//! `tools/oraculo_tipos/oracle.dart`). An inference conformance tool, not a
//! product: used by the examples `despejo_tipos`, `comparar_tipos` and
//! `gravar_corpus` and by the corpus test `corpus/inference`
//! (`tests/corpus_inferencia.rs`).
//!
//! Dump line: `caminho \t offset \t comprimento \t nó \t tipo \t
//! resolução`, offsets in UTF-16 units (the analyzer's), sorted by
//! `(offset, comprimento)`. An expression the inference never visited is
//! output with the type `?`.

use crate::resolved::{BodyTypes, MemberRef, Resolved};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use crate::{resolve_outline, BodyInferrer};
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::load::load_lenient;
use dartforge_elements::model::{Program, UnitId};
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::ast::ExprKind;
use dartforge_intern::Interner;
use std::collections::BTreeMap;
use std::path::Path;

/// Sentinel for unvisited expressions.
pub const NOT_VISITED: TypeId = TypeId(u32::MAX);

/// A dumped expression.
#[derive(Debug, Clone)]
pub struct Line {
    pub offset: usize,
    pub comprimento: usize,
    pub no: &'static str,
    pub tipo: String,
    pub resolution: String,
}

/// A dumped unit (file).
pub struct Unidade {
    /// Path with `/`.
    pub caminho: String,
    pub linhas: Vec<Line>,
}

/// Result of dumping a program.
pub struct Dump {
    /// Units outside the SDK, sorted by path.
    pub unidades: Vec<Unidade>,
    /// Warnings from the outline and the bodies.
    pub avisos: Vec<Diagnostic>,
    pub not_visited: usize,
    pub inference_time: std::time::Duration,
}

/// Loads `entry`, infers and dumps the units outside the SDK.
pub fn dump(entry: &Path, sdk: &SdkLayout, packages: Option<&Path>) -> Dump {
    let mut interner = Interner::new();
    let (prog, _) = load_lenient(entry, sdk, packages, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, diags_outline) = resolve_outline(&prog, &interner, &mut table, &core);
    let t = std::time::Instant::now();
    let mut inf = BodyInferrer::new(&prog, &interner, &mut table, &core, &mut outline);
    for u in &mut inf.body_types.units {
        u.static_types.fill(NOT_VISITED);
    }
    let (bodies, diags) = inf.infer_all();
    let inference_time = t.elapsed();
    let mut order: Vec<(String, usize)> = prog
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| !prog.library(u.library).is_sdk)
        .filter_map(|(ui, u)| u.path.as_ref().map(|p| (p.to_string_lossy().replace('\\', "/"), ui)))
        .collect();
    order.sort();
    order.dedup_by(|a, b| a.0 == b.0);
    let mut not_visited = 0;
    let units = order
        .into_iter()
        .map(|(path, ui)| {
            let lines = unit_lines(&prog, &bodies, &table, &interner, UnitId(ui as u32));
            not_visited += lines.iter().filter(|l| l.tipo == "?").count();
            Unidade { caminho: path, linhas: lines }
        })
        .collect();
    let mut warnings = diags_outline;
    warnings.extend(diags);
    Dump { unidades: units, avisos: warnings, not_visited, inference_time }
}

/// Lines of a unit, sorted by UTF-16 `(offset, comprimento)`.
pub fn unit_lines(prog: &Program, bodies: &BodyTypes, table: &TypeTable, interner: &Interner, u: UnitId) -> Vec<Line> {
    let unit = prog.unit(u);
    let bt = &bodies.units[u.0 as usize];
    let utf16 = utf16_map(&unit.source);
    let mut lines: Vec<Line> = unit
        .ast
        .exprs
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let ty = bt.static_types.get(i).copied().unwrap_or(NOT_VISITED);
            let typ = if ty == NOT_VISITED {
                "?".to_string()
            } else if bt.invalid_types.contains(&dartforge_frontend::ast::ExprId(i as u32)) {
                "InvalidType".to_string()
            } else {
                formatar(table, ty, interner, prog)
            };
            let resolution = bt
                .resolved
                .get(i)
                .and_then(|r| r.as_ref())
                .map(|r| format_resolution(r, prog, interner))
                .unwrap_or_else(|| "-".into());
            let (start, end) = (utf16(e.span.start), utf16(e.span.end));
            Line { offset: start, comprimento: end - start, no: name_in(&e.kind), tipo: typ, resolution }
        })
        .collect();
    lines.sort_by_key(|l| (l.offset, l.comprimento));
    lines
}

/// Offset in UTF-8 bytes → offset in UTF-16 units.
pub fn utf16_map(source: &str) -> impl Fn(usize) -> usize + '_ {
    let mut accumulated = Vec::with_capacity(source.len() + 1);
    let mut u = 0usize;
    for c in source.chars() {
        for _ in 0..c.len_utf8() {
            accumulated.push(u);
        }
        u += c.len_utf16();
    }
    accumulated.push(u);
    move |b| accumulated.get(b).copied().unwrap_or(u)
}

pub fn name_in(k: &ExprKind) -> &'static str {
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

pub fn format_resolution(r: &Resolved, prog: &Program, i: &Interner) -> String {
    match r {
        Resolved::Local(_) => "LOCAL".into(),
        Resolved::Parameter { name, .. } => format!("PARAMETER:{}", i.resolve(*name)),
        Resolved::TypeParameter(_) => "TYPE_PARAMETER".into(),
        Resolved::Element(e) => format!("ELEMENT:{e:?}"),
        Resolved::Member { class, member, .. } => {
            let c = i.resolve(prog.class(*class).name);
            let n = match member {
                MemberRef::Function(f) => i.resolve(prog.function(*f).name),
                MemberRef::Variable(v) => i.resolve(prog.variable(*v).name),
            };
            format!("MEMBER:{c}.{n}")
        }
        Resolved::Prefix(_) => "PREFIX".into(),
        Resolved::Dynamic => "DYNAMIC".into(),
        Resolved::ExtensionMember { member, .. } => format!("EXTENSION:{}", i.resolve(prog.function(*member).name)),
        Resolved::Constructor(f) => format!("CONSTRUCTOR:{}", i.resolve(prog.function(*f).name)),
    }
}

/// Formats like the analyzer's `DartType.getDisplayString()`.
pub fn formatar(t: &TypeTable, ty: TypeId, i: &Interner, p: &Program) -> String {
    let q = |n: bool| if n { "?" } else { "" };
    match t.get(ty) {
        Type::Intersection { param, bound } => format!("{} & {}", i.resolve(t.param(*param).name), formatar(t, *bound, i, p)),
        Type::Dynamic => "dynamic".into(),
        Type::Void => "void".into(),
        Type::Never => "Never".into(),
        Type::Null => "Null".into(),
        Type::Interface { class, args, nullable } | Type::ExtensionType { decl: class, args, nullable } => {
            let the_name = i.resolve(p.class(*class).name);
            if args.is_empty() {
                format!("{the_name}{}", q(*nullable))
            } else {
                let a: Vec<String> = args.iter().map(|&x| formatar(t, x, i, p)).collect();
                format!("{the_name}<{}>{}", a.join(", "), q(*nullable))
            }
        }
        Type::FutureOr { arg, nullable } => format!("FutureOr<{}>{}", formatar(t, *arg, i, p), q(*nullable)),
        Type::TypeParameter { param, nullable } => format!("{}{}", i.resolve(t.param(*param).name), q(*nullable)),
        Type::Record { positional, named, nullable } => {
            let mut parts: Vec<String> = positional.iter().map(|&x| formatar(t, x, i, p)).collect();
            let mut named_args: Vec<(String, String)> =
                named.iter().map(|(n, x)| (i.resolve(*n).to_string(), formatar(t, *x, i, p))).collect();
            named_args.sort();
            if !named_args.is_empty() {
                let n: Vec<String> = named_args.iter().map(|(n, x)| format!("{x} {n}")).collect();
                parts.push(format!("{{{}}}", n.join(", ")));
            } else if positional.len() == 1 {
                return format!("({},){}", parts[0], q(*nullable));
            }
            format!("({}){}", parts.join(", "), q(*nullable))
        }
        Type::Function { type_params, ret, positional, optional, named, nullable } => {
            let mut s = format!("{} Function", formatar(t, *ret, i, p));
            if !type_params.is_empty() {
                let tps: Vec<String> = type_params
                    .iter()
                    .map(|&tp| {
                        let d = t.param(tp);
                        let the_name = i.resolve(d.name).to_string();
                        match t.get(d.bound) {
                            Type::Interface { nullable: true, class, .. } if !d.explicito && i.resolve(p.class(*class).name) == "Object" => the_name,
                            Type::Dynamic => the_name,
                            _ => format!("{the_name} extends {}", formatar(t, d.bound, i, p)),
                        }
                    })
                    .collect();
                s.push_str(&format!("<{}>", tps.join(", ")));
            }
            let mut parts: Vec<String> = positional.iter().map(|&x| formatar(t, x, i, p)).collect();
            if !optional.is_empty() {
                let o: Vec<String> = optional.iter().map(|&x| formatar(t, x, i, p)).collect();
                parts.push(format!("[{}]", o.join(", ")));
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
                parts.push(format!("{{{}}}", n.join(", ")));
            }
            format!("{s}({}){}", parts.join(", "), q(*nullable))
        }
    }
}

// -------------------------------------------------------------------
// Comparison
// -------------------------------------------------------------------

/// Lines of a file: `(offset, comprimento) -> [(nó, typ)]`.
pub type Bloco = BTreeMap<(usize, usize), Vec<(String, String)>>;

/// An expression whose type diverges from the oracle.
#[derive(Debug, Clone)]
pub struct Divergence {
    pub ini: usize,
    pub fim: usize,
    pub no: String,
    pub ours: String,
    pub in_oracle: String,
    pub oraculo: String,
}

/// Result of comparing a file.
#[derive(Debug, Default)]
pub struct Comparison {
    pub compared: usize,
    pub iguais: usize,
    /// Sorted by start and, for the same start, from largest to smallest.
    pub divergences: Vec<Divergence>,
}

impl Comparison {
    /// Indices of the divergences that are a **cause**: no other divergence is
    /// strictly contained in it (the ones that contain it are cascade).
    pub fn causas(&self) -> Vec<usize> {
        let div = &self.divergences;
        let mut v = Vec::new();
        for i in 0..div.len() {
            let (start, end) = (div[i].ini, div[i].fim);
            let contained = div[i + 1..]
                .iter()
                .take_while(|d| d.ini < end)
                .any(|d| d.fim <= end && (d.ini, d.fim) != (start, end));
            if !contained {
                v.push(i);
            }
        }
        v
    }
}

/// Compares our block with the oracle's (only the expressions the oracle
/// types; `-` is a node without a type, such as a class name).
pub fn compare_block(ours: &Bloco, oracle: &Bloco) -> Comparison {
    let mut c = Comparison::default();
    for (&(start, comp), the_ours) in ours {
        let Some(orcs) = oracle.get(&(start, comp)) else { continue };
        let orcs: Vec<&(String, String)> = orcs.iter().filter(|o| o.1 != "-").collect();
        if orcs.is_empty() {
            continue;
        }
        for (no, typ) in the_ours {
            c.compared += 1;
            if orcs.iter().any(|o| &o.1 == typ) {
                c.iguais += 1;
                continue;
            }
            c.divergences.push(Divergence {
                ini: start,
                fim: start + comp,
                no: no.clone(),
                ours: typ.clone(),
                in_oracle: orcs[0].0.clone(),
                oraculo: orcs[0].1.clone(),
            });
        }
    }
    c.divergences.sort_by(|a, b| (a.ini, std::cmp::Reverse(a.fim)).cmp(&(b.ini, std::cmp::Reverse(b.fim))));
    c
}

/// Block from the dump lines.
pub fn line_block(lines: &[Line]) -> Bloco {
    let mut b = Bloco::new();
    for l in lines {
        b.entry((l.offset, l.comprimento)).or_default().push((l.no.to_string(), l.tipo.clone()));
    }
    b
}

/// Block from a corpus `.esperado.tsv` (`linha coluna offset
/// comprimento nó tipo elemento marca trecho`).
pub fn expected_block(text: &str) -> Bloco {
    let mut b = Bloco::new();
    for l in text.lines() {
        if l.starts_with('#') {
            continue;
        }
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 6 {
            continue;
        }
        let (Ok(start), Ok(comp)) = (c[2].parse(), c[3].parse()) else { continue };
        b.entry((start, comp)).or_default().push((c[4].to_string(), c[5].replace('*', "")));
    }
    b
}

/// Type without arguments and without `?` (for the shape of the difference).
fn without_args(s: &str) -> &str {
    let s = match s.find('<') {
        Some(i) => &s[..i],
        None => s,
    };
    s.trim_end_matches('?')
}

/// Shape of the difference between our type and the oracle's.
pub fn forma(ours: &str, oracle: &str) -> &'static str {
    if ours == "?" {
        return "not visited";
    }
    if ours == "dynamic" {
        return "ours dynamic";
    }
    if oracle == "dynamic" {
        return "oracle dynamic";
    }
    if ours.trim_end_matches('?') == oracle.trim_end_matches('?') {
        return "nullability";
    }
    if without_args(ours) == without_args(oracle) {
        return "type arguments";
    }
    if oracle.contains("Function") || ours.contains("Function") {
        return "function type";
    }
    "other type"
}
