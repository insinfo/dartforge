//! Despejo de tipos e comparação com o oráculo (`package:analyzer`,
//! `tools/oraculo_tipos/oraculo.dart`). Ferramenta de conformidade da
//! inferência, não produto: usada pelos exemplos `despejo_tipos`,
//! `comparar_tipos` e `gravar_corpus` e pelo teste do corpus
//! `corpus/inferencia` (`tests/corpus_inferencia.rs`).
//!
//! Linha do despejo: `caminho \t offset \t comprimento \t nó \t tipo \t
//! resolução`, offsets em unidades UTF-16 (os do analyzer), ordenada por
//! `(offset, comprimento)`. Expressão que a inferência nunca visitou sai com
//! o tipo `?`.

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

/// Sentinela das expressões não visitadas.
pub const NAO_VISITADA: TypeId = TypeId(u32::MAX);

/// Uma expressão despejada.
#[derive(Debug, Clone)]
pub struct Linha {
    pub offset: usize,
    pub comprimento: usize,
    pub no: &'static str,
    pub tipo: String,
    pub resolucao: String,
}

/// Uma unidade (arquivo) despejada.
pub struct Unidade {
    /// Caminho com `/`.
    pub caminho: String,
    pub linhas: Vec<Linha>,
}

/// Resultado do despejo de um programa.
pub struct Despejo {
    /// Unidades fora do SDK, ordenadas pelo caminho.
    pub unidades: Vec<Unidade>,
    /// Avisos do outline e dos corpos.
    pub avisos: Vec<Diagnostic>,
    pub nao_visitadas: usize,
    pub tempo_inferencia: std::time::Duration,
}

/// Carrega `entrada`, infere e despeja as unidades fora do SDK.
pub fn despejar(entrada: &Path, sdk: &SdkLayout, packages: Option<&Path>) -> Despejo {
    let mut interner = Interner::new();
    let (prog, _) = load_lenient(entrada, sdk, packages, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, diags_outline) = resolve_outline(&prog, &interner, &mut table, &core);
    let t = std::time::Instant::now();
    let mut inf = BodyInferrer::new(&prog, &interner, &mut table, &core, &mut outline);
    for u in &mut inf.body_types.units {
        u.static_types.fill(NAO_VISITADA);
    }
    let (bodies, diags) = inf.infer_all();
    let tempo_inferencia = t.elapsed();
    let mut ordem: Vec<(String, usize)> = prog
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| !prog.library(u.library).is_sdk)
        .filter_map(|(ui, u)| u.path.as_ref().map(|p| (p.to_string_lossy().replace('\\', "/"), ui)))
        .collect();
    ordem.sort();
    ordem.dedup_by(|a, b| a.0 == b.0);
    let mut nao_visitadas = 0;
    let unidades = ordem
        .into_iter()
        .map(|(caminho, ui)| {
            let linhas = linhas_da_unidade(&prog, &bodies, &table, &interner, UnitId(ui as u32));
            nao_visitadas += linhas.iter().filter(|l| l.tipo == "?").count();
            Unidade { caminho, linhas }
        })
        .collect();
    let mut avisos = diags_outline;
    avisos.extend(diags);
    Despejo { unidades, avisos, nao_visitadas, tempo_inferencia }
}

/// Linhas de uma unidade, ordenadas por `(offset, comprimento)` UTF-16.
pub fn linhas_da_unidade(prog: &Program, bodies: &BodyTypes, table: &TypeTable, interner: &Interner, u: UnitId) -> Vec<Linha> {
    let unit = prog.unit(u);
    let bt = &bodies.units[u.0 as usize];
    let utf16 = mapa_utf16(&unit.source);
    let mut linhas: Vec<Linha> = unit
        .ast
        .exprs
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let ty = bt.static_types.get(i).copied().unwrap_or(NAO_VISITADA);
            let tipo = if ty == NAO_VISITADA { "?".to_string() } else { formatar(table, ty, interner, prog) };
            let resolucao = bt
                .resolved
                .get(i)
                .and_then(|r| r.as_ref())
                .map(|r| formatar_resolucao(r, prog, interner))
                .unwrap_or_else(|| "-".into());
            let (ini, fim) = (utf16(e.span.start), utf16(e.span.end));
            Linha { offset: ini, comprimento: fim - ini, no: nome_no(&e.kind), tipo, resolucao }
        })
        .collect();
    linhas.sort_by_key(|l| (l.offset, l.comprimento));
    linhas
}

/// Offset em bytes UTF-8 → offset em unidades UTF-16.
pub fn mapa_utf16(fonte: &str) -> impl Fn(usize) -> usize + '_ {
    let mut acumulado = Vec::with_capacity(fonte.len() + 1);
    let mut u = 0usize;
    for c in fonte.chars() {
        for _ in 0..c.len_utf8() {
            acumulado.push(u);
        }
        u += c.len_utf16();
    }
    acumulado.push(u);
    move |b| acumulado.get(b).copied().unwrap_or(u)
}

pub fn nome_no(k: &ExprKind) -> &'static str {
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
    }
}

pub fn formatar_resolucao(r: &Resolved, prog: &Program, i: &Interner) -> String {
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

/// Formata como o `DartType.getDisplayString()` do analyzer.
pub fn formatar(t: &TypeTable, ty: TypeId, i: &Interner, p: &Program) -> String {
    let q = |n: bool| if n { "?" } else { "" };
    match t.get(ty) {
        Type::Intersection { param, bound } => format!("{} & {}", i.resolve(t.param(*param).name), formatar(t, *bound, i, p)),
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
                            Type::Interface { nullable: true, class, .. } if i.resolve(p.class(*class).name) == "Object" => nome,
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

// -------------------------------------------------------------------
// Comparação
// -------------------------------------------------------------------

/// Linhas de um arquivo: `(offset, comprimento) -> [(nó, tipo)]`.
pub type Bloco = BTreeMap<(usize, usize), Vec<(String, String)>>;

/// Uma expressão cujo tipo diverge do oráculo.
#[derive(Debug, Clone)]
pub struct Divergencia {
    pub ini: usize,
    pub fim: usize,
    pub no: String,
    pub nosso: String,
    pub no_oraculo: String,
    pub oraculo: String,
}

/// Resultado da comparação de um arquivo.
#[derive(Debug, Default)]
pub struct Comparacao {
    pub comparadas: usize,
    pub iguais: usize,
    /// Ordenadas por início e, no mesmo início, da maior para a menor.
    pub divergencias: Vec<Divergencia>,
}

impl Comparacao {
    /// Índices das divergências que são **causa**: nenhuma outra
    /// divergência está estritamente contida nela (as que a contêm são
    /// cascata).
    pub fn causas(&self) -> Vec<usize> {
        let div = &self.divergencias;
        let mut v = Vec::new();
        for i in 0..div.len() {
            let (ini, fim) = (div[i].ini, div[i].fim);
            let contida = div[i + 1..]
                .iter()
                .take_while(|d| d.ini < fim)
                .any(|d| d.fim <= fim && (d.ini, d.fim) != (ini, fim));
            if !contida {
                v.push(i);
            }
        }
        v
    }
}

/// Compara o bloco nosso com o do oráculo (só as expressões que o oráculo
/// tipa; `-` é nó sem tipo, como um nome de classe).
pub fn comparar_bloco(nosso: &Bloco, oraculo: &Bloco) -> Comparacao {
    let mut c = Comparacao::default();
    for (&(ini, comp), nossos) in nosso {
        let Some(orcs) = oraculo.get(&(ini, comp)) else { continue };
        let orcs: Vec<&(String, String)> = orcs.iter().filter(|o| o.1 != "-").collect();
        if orcs.is_empty() {
            continue;
        }
        for (no, tipo) in nossos {
            c.comparadas += 1;
            if orcs.iter().any(|o| &o.1 == tipo) {
                c.iguais += 1;
                continue;
            }
            c.divergencias.push(Divergencia {
                ini,
                fim: ini + comp,
                no: no.clone(),
                nosso: tipo.clone(),
                no_oraculo: orcs[0].0.clone(),
                oraculo: orcs[0].1.clone(),
            });
        }
    }
    c.divergencias.sort_by(|a, b| (a.ini, std::cmp::Reverse(a.fim)).cmp(&(b.ini, std::cmp::Reverse(b.fim))));
    c
}

/// Bloco a partir das linhas do despejo.
pub fn bloco_de_linhas(linhas: &[Linha]) -> Bloco {
    let mut b = Bloco::new();
    for l in linhas {
        b.entry((l.offset, l.comprimento)).or_default().push((l.no.to_string(), l.tipo.clone()));
    }
    b
}

/// Bloco a partir de um `.esperado.tsv` do corpus (`linha coluna offset
/// comprimento nó tipo elemento marca trecho`).
pub fn bloco_de_esperado(texto: &str) -> Bloco {
    let mut b = Bloco::new();
    for l in texto.lines() {
        if l.starts_with('#') {
            continue;
        }
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 6 {
            continue;
        }
        let (Ok(ini), Ok(comp)) = (c[2].parse(), c[3].parse()) else { continue };
        b.entry((ini, comp)).or_default().push((c[4].to_string(), c[5].replace('*', "")));
    }
    b
}

/// Tipo sem argumentos e sem `?` (para a forma da diferença).
fn sem_args(s: &str) -> &str {
    let s = match s.find('<') {
        Some(i) => &s[..i],
        None => s,
    };
    s.trim_end_matches('?')
}

/// Forma da diferença entre o nosso tipo e o do oráculo.
pub fn forma(nosso: &str, oraculo: &str) -> &'static str {
    if nosso == "?" {
        return "não visitada";
    }
    if nosso == "dynamic" {
        return "nós dynamic";
    }
    if oraculo == "dynamic" {
        return "oráculo dynamic";
    }
    if nosso.trim_end_matches('?') == oraculo.trim_end_matches('?') {
        return "nulabilidade";
    }
    if sem_args(nosso) == sem_args(oraculo) {
        return "argumentos de tipo";
    }
    if oraculo.contains("Function") || nosso.contains("Function") {
        return "tipo de função";
    }
    "outro tipo"
}
