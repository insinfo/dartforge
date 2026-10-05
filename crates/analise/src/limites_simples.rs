//! Os limites dos parâmetros de tipo (docs/ANALYZER-ESPECIFICACAO.md, §A):
//!
//! * `computeSimplyBounded` (`analyzer/lib/src/summary2/simply_bounded.dart`):
//!   uma classe, mixin, enum, tipo de extensão ou alias é *simplesmente
//!   limitado* quando os limites dos seus parâmetros de tipo (e, num alias,
//!   os tipos do lado direito) não citam parâmetro de tipo (no alias, o lado
//!   direito pode) nem tipo genérico cru que não seja simplesmente
//!   limitado; um ciclo de tipos crus (`class A<T extends A>`) não é;
//! * `NOT_INSTANTIATED_BOUND` (`_UninstantiatedBoundChecker`,
//!   `error_verifier.dart:7167-7190`): no limite de qualquer parâmetro de
//!   tipo, cada tipo nomeado cru cujo elemento não é simplesmente limitado;
//! * `TYPE_PARAMETER_SUPERTYPE_OF_ITS_BOUND`
//!   (`_checkForTypeParameterBoundRecursion`, `:5382-5416`): seguindo os
//!   limites que são parâmetros da mesma lista (apagando tipos de extensão),
//!   `parameters.length` passos fecham um ciclo.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, ClassKind, Element, LibraryId, Program, TypedefId, UnitId};
use dartforge_frontend::ast::{self, DeclKind, Parameter, TypeKind, TypeParameter, TypedefKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum No {
    Classe(ClassId),
    Alias(TypedefId),
}

/// As declarações que não são simplesmente limitadas.
#[derive(Default)]
pub struct LimitesSimples {
    nao_simples: HashSet<No>,
}

/// `_TypeCollector`: os tipos de um tipo de função (retorno, limites dos
/// parâmetros de tipo, parâmetros).
fn coletar_de_parametros(ps: &[Parameter], saida: &mut Vec<ast::TypeId>) {
    for p in ps {
        if p.this_ {
            continue;
        }
        if let Some(t) = p.ty {
            saida.push(t);
        }
        if let Some(inner) = &p.function_parameters {
            coletar_de_parametros(inner, saida);
        }
    }
}

fn coletar_de_funcao(retorno: Option<ast::TypeId>, tps: &[TypeParameter], ps: &[Parameter], saida: &mut Vec<ast::TypeId>) {
    if let Some(r) = retorno {
        saida.push(r);
    }
    for tp in tps {
        if let Some(b) = tp.bound {
            saida.push(b);
        }
    }
    coletar_de_parametros(ps, saida);
}

/// O elemento de tipo de um nome (`p.Nome` ou `Nome`) no escopo da unidade.
fn elemento(programa: &Program, u: UnitId, nome: &[ast::Name]) -> Option<Element> {
    let b = match nome {
        [n] => programa.lookup_na_unidade(u, n.sym),
        [p, n] => programa.lookup_prefixed_na_unidade(u, p.sym, n.sym),
        _ => None,
    }?;
    b.getter
}

fn no_de(programa: &Program, el: Element) -> Option<No> {
    match el {
        Element::Class(c) if programa.class(c).decl.is_some() => Some(No::Classe(c)),
        Element::Typedef(t) => Some(No::Alias(t)),
        _ => None,
    }
}

/// `SimplyBoundedNode._visitType`: `false` decide (não simplesmente
/// limitado); os tipos crus de declarações viram dependências.
fn visitar(programa: &Program, u: UnitId, a: &ast::Ast, t: ast::TypeId, permite_parametros: bool, escopo: &[SymbolId], deps: &mut Vec<No>) -> bool {
    match &a.ty(t).kind {
        TypeKind::Named { name, args } => {
            if let [n] = &name[..]
                && escopo.contains(&n.sym)
            {
                return permite_parametros;
            }
            if !args.is_empty() {
                return args.iter().all(|&x| visitar(programa, u, a, x, permite_parametros, escopo, deps));
            }
            if let Some(no) = elemento(programa, u, name).and_then(|el| no_de(programa, el)) {
                deps.push(no);
            }
            true
        }
        TypeKind::Void => true,
        TypeKind::Function { return_type, type_params, parameters } => {
            let mut escopo2 = escopo.to_vec();
            escopo2.extend(type_params.iter().map(|p| p.name.sym));
            let mut tipos = Vec::new();
            coletar_de_funcao(*return_type, type_params, parameters, &mut tipos);
            tipos.into_iter().all(|x| visitar(programa, u, a, x, permite_parametros, &escopo2, deps))
        }
        TypeKind::Record { positional, named } => {
            positional.iter().chain(named.iter().map(|(_, x)| x)).all(|&x| visitar(programa, u, a, x, permite_parametros, escopo, deps))
        }
    }
}

/// Os parâmetros de tipo e os tipos do lado direito de um nó.
fn partes_do_no<'a>(programa: &'a Program, no: No) -> Option<(UnitId, &'a [TypeParameter], Vec<ast::TypeId>)> {
    let d = match no {
        No::Classe(c) => programa.class(c).decl?,
        No::Alias(t) => programa.typedef(t).decl,
    };
    let a = &programa.unit(d.unit).ast;
    let decl = a.decl(d.decl);
    let (tps, rhs): (&'a [TypeParameter], Vec<ast::TypeId>) = match &decl.kind {
        DeclKind::Class(x) => (&x.type_params, Vec::new()),
        DeclKind::Mixin(x) => (&x.type_params, Vec::new()),
        DeclKind::Enum(x) => (&x.type_params, Vec::new()),
        DeclKind::ExtensionType(x) => (&x.type_params, Vec::new()),
        DeclKind::Typedef(x) => {
            let mut rhs = Vec::new();
            match &x.kind {
                TypedefKind::Alias(t) => match &a.ty(*t).kind {
                    TypeKind::Function { return_type, type_params, parameters } => coletar_de_funcao(*return_type, type_params, parameters, &mut rhs),
                    _ => rhs.push(*t),
                },
                TypedefKind::Legacy { return_type, parameters } => coletar_de_funcao(*return_type, &[], parameters, &mut rhs),
            }
            (&x.type_params, rhs)
        }
        _ => return None,
    };
    Some((d.unit, tps, rhs))
}

/// `computeSimplyBounded` sobre todas as declarações do programa.
pub fn calcular(programa: &Program) -> LimitesSimples {
    let mut nos: Vec<No> = Vec::new();
    for (i, ce) in programa.classes.iter().enumerate() {
        if ce.decl.is_some() && matches!(ce.kind, ClassKind::Class | ClassKind::Mixin | ClassKind::Enum | ClassKind::ExtensionType | ClassKind::MixinApplication) {
            nos.push(No::Classe(ClassId(i as u32)));
        }
    }
    for i in 0..programa.typedefs.len() {
        nos.push(No::Alias(TypedefId(i as u32)));
    }
    // `computeDependencies`: `None` decide falso de imediato.
    let mut deps: HashMap<No, Option<Vec<No>>> = HashMap::new();
    for &no in &nos {
        let Some((u, tps, rhs)) = partes_do_no(programa, no) else {
            deps.insert(no, Some(Vec::new()));
            continue;
        };
        let a = &programa.unit(u).ast;
        let escopo: Vec<SymbolId> = tps.iter().map(|p| p.name.sym).collect();
        let mut v = Vec::new();
        let mut ok = true;
        for tp in tps {
            if let Some(b) = tp.bound
                && !visitar(programa, u, a, b, false, &escopo, &mut v)
            {
                ok = false;
                break;
            }
        }
        if ok {
            for &t in &rhs {
                if !visitar(programa, u, a, t, true, &escopo, &mut v) {
                    ok = false;
                    break;
                }
            }
        }
        deps.insert(no, ok.then_some(v));
    }
    // Tarjan (o `DependencyWalker`): as componentes saem com as dependências
    // já avaliadas; ciclo (ou auto-laço) não é simplesmente limitado.
    struct Estado<'d> {
        deps: &'d HashMap<No, Option<Vec<No>>>,
        indice: HashMap<No, usize>,
        baixo: HashMap<No, usize>,
        pilha: Vec<No>,
        na_pilha: HashSet<No>,
        proximo: usize,
        nao_simples: HashSet<No>,
    }
    fn conectar(e: &mut Estado<'_>, v: No) {
        e.indice.insert(v, e.proximo);
        e.baixo.insert(v, e.proximo);
        e.proximo += 1;
        e.pilha.push(v);
        e.na_pilha.insert(v);
        let mut auto = false;
        let vizinhos: Vec<No> = match e.deps.get(&v) {
            Some(Some(d)) => d.clone(),
            _ => Vec::new(),
        };
        for w in vizinhos.iter().copied() {
            if !e.deps.contains_key(&w) {
                continue;
            }
            if w == v {
                auto = true;
            } else if !e.indice.contains_key(&w) {
                conectar(e, w);
                let bw = e.baixo[&w];
                if bw < e.baixo[&v] {
                    e.baixo.insert(v, bw);
                }
            } else if e.na_pilha.contains(&w) {
                let iw = e.indice[&w];
                if iw < e.baixo[&v] {
                    e.baixo.insert(v, iw);
                }
            }
        }
        if e.baixo[&v] == e.indice[&v] {
            let mut componente = Vec::new();
            while let Some(x) = e.pilha.pop() {
                e.na_pilha.remove(&x);
                componente.push(x);
                if x == v {
                    break;
                }
            }
            if componente.len() > 1 || auto {
                e.nao_simples.extend(componente);
            } else {
                // `_evaluate`: falso de imediato, ou alguma dependência falsa.
                let falso = match e.deps.get(&v) {
                    Some(None) => true,
                    Some(Some(d)) => d.iter().any(|x| e.nao_simples.contains(x)),
                    None => false,
                };
                if falso {
                    e.nao_simples.insert(v);
                }
            }
        }
    }
    let mut e = Estado { deps: &deps, indice: HashMap::new(), baixo: HashMap::new(), pilha: Vec::new(), na_pilha: HashSet::new(), proximo: 1, nao_simples: HashSet::new() };
    for &v in &nos {
        if !e.indice.contains_key(&v) {
            conectar(&mut e, v);
        }
    }
    LimitesSimples { nao_simples: e.nao_simples }
}

/// Uma lista de parâmetros de tipo e onde seus nomes valem.
struct Lista<'a> {
    parametros: &'a [TypeParameter],
    escopo: Span,
}

fn listas_de_parametros<'a>(ps: &'a [Parameter], saida: &mut Vec<Lista<'a>>) {
    for p in ps {
        if !p.function_type_params.is_empty() {
            saida.push(Lista { parametros: &p.function_type_params, escopo: p.span });
        }
        if let Some(inner) = &p.function_parameters {
            listas_de_parametros(inner, saida);
        }
    }
}

/// Todas as listas de parâmetros de tipo da unidade.
fn listas(a: &ast::Ast) -> Vec<Lista<'_>> {
    let mut v = Vec::new();
    for d in &a.decls {
        let tps: &[TypeParameter] = match &d.kind {
            DeclKind::Class(x) => &x.type_params,
            DeclKind::Mixin(x) => &x.type_params,
            DeclKind::Enum(x) => &x.type_params,
            DeclKind::ExtensionType(x) => &x.type_params,
            DeclKind::Extension(x) => &x.type_params,
            DeclKind::Typedef(x) => {
                if let TypedefKind::Legacy { parameters, .. } = &x.kind {
                    listas_de_parametros(parameters, &mut v);
                }
                &x.type_params
            }
            _ => &[],
        };
        if !tps.is_empty() {
            v.push(Lista { parametros: tps, escopo: d.span });
        }
    }
    for f in &a.functions {
        if !f.type_params.is_empty() {
            v.push(Lista { parametros: &f.type_params, escopo: f.span });
        }
        if let Some(ps) = &f.parameters {
            listas_de_parametros(ps, &mut v);
        }
    }
    for m in &a.members {
        if let ast::MemberKind::Constructor(k) = &m.kind {
            listas_de_parametros(&k.parameters, &mut v);
        }
    }
    for t in &a.types {
        if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
            if !type_params.is_empty() {
                v.push(Lista { parametros: type_params, escopo: t.span });
            }
            listas_de_parametros(parameters, &mut v);
        }
    }
    v
}

/// O parâmetro da lista que o limite `t` designa depois de apagar tipos de
/// extensão (`boundType.extensionTypeErasure.element`).
fn parametro_do_limite(programa: &Program, u: UnitId, a: &ast::Ast, t: ast::TypeId, lista: &[TypeParameter], prof: u32) -> Option<usize> {
    if prof > 8 {
        return None;
    }
    let TypeKind::Named { name, args } = &a.ty(t).kind else { return None };
    if let [n] = &name[..]
        && let Some(i) = lista.iter().position(|p| p.name.sym == n.sym)
    {
        return Some(i);
    }
    // Tipo de extensão cuja representação é um dos seus parâmetros: o
    // apagamento é o argumento correspondente.
    let Some(Element::Class(c)) = elemento(programa, u, name) else { return None };
    let ce = programa.class(c);
    if ce.kind != ClassKind::ExtensionType {
        return None;
    }
    let d = ce.decl?;
    let ad = &programa.unit(d.unit).ast;
    let DeclKind::ExtensionType(et) = &ad.decl(d.decl).kind else { return None };
    let TypeKind::Named { name: rn, args: ra } = &ad.ty(et.representation_type).kind else { return None };
    if !ra.is_empty() {
        return None;
    }
    let [r] = &rn[..] else { return None };
    let k = et.type_params.iter().position(|p| p.name.sym == r.sym)?;
    let &arg = args.get(k)?;
    parametro_do_limite(programa, u, a, arg, lista, prof + 1)
}

/// `_UninstantiatedBoundChecker` num limite.
fn nao_instanciados(programa: &Program, u: UnitId, a: &ast::Ast, t: ast::TypeId, visiveis: &dyn Fn(usize, SymbolId) -> bool, limites: &LimitesSimples, saida: &mut Vec<(UnitId, Diagnostic)>) {
    let no_tipo = a.ty(t);
    match &no_tipo.kind {
        TypeKind::Named { name, args } => {
            if !args.is_empty() {
                for &x in args.iter() {
                    nao_instanciados(programa, u, a, x, visiveis, limites, saida);
                }
                return;
            }
            if let [n] = &name[..]
                && visiveis(n.span.start, n.sym)
            {
                return;
            }
            if let Some(no) = elemento(programa, u, name).and_then(|el| no_de(programa, el))
                && limites.nao_simples.contains(&no)
            {
                saida.push((u, Diagnostic::com_codigo(c::NOT_INSTANTIATED_BOUND, no_tipo.span, [] as [&str; 0])));
            }
        }
        TypeKind::Void => {}
        TypeKind::Function { return_type, type_params, parameters } => {
            if let Some(r) = return_type {
                nao_instanciados(programa, u, a, *r, visiveis, limites, saida);
            }
            for tp in type_params.iter() {
                if let Some(b) = tp.bound {
                    nao_instanciados(programa, u, a, b, visiveis, limites, saida);
                }
            }
            let mut tipos = Vec::new();
            coletar_de_parametros(parameters, &mut tipos);
            for x in tipos {
                nao_instanciados(programa, u, a, x, visiveis, limites, saida);
            }
        }
        TypeKind::Record { positional, named } => {
            for &x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                nao_instanciados(programa, u, a, x, visiveis, limites, saida);
            }
        }
    }
}

/// O tipo escrito é um tipo de função genérico (direto ou por um alias de um).
fn funcao_generica(programa: &Program, u: UnitId, a: &ast::Ast, t: ast::TypeId, prof: u32) -> bool {
    match &a.ty(t).kind {
        TypeKind::Function { type_params, .. } => !type_params.is_empty(),
        TypeKind::Named { name, args } if args.is_empty() && prof < 8 => {
            let Some(Element::Typedef(td)) = elemento(programa, u, name) else { return false };
            let d = programa.typedef(td).decl;
            let ad = &programa.unit(d.unit).ast;
            match &ad.decl(d.decl).kind {
                DeclKind::Typedef(x) => match &x.kind {
                    TypedefKind::Alias(corpo) => funcao_generica(programa, d.unit, ad, *corpo, prof + 1),
                    TypedefKind::Legacy { .. } => false,
                },
                _ => false,
            }
        }
        _ => false,
    }
}

pub fn verificar(programa: &Program, lib: LibraryId, nomes: &Interner, limites: &LimitesSimples) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    // `generic-metadata` (2.14): antes dele, tipo de função genérico como
    // argumento de tipo escrito é `GENERIC_FUNCTION_TYPE_CANNOT_BE_TYPE_ARGUMENT`
    // (`type_arguments_verifier.dart:300-310`), no argumento.
    let sem_metadados_genericos = programa.library(lib).features.versao() < dartforge_frontend::features::LanguageVersion::new(2, 14);
    for &u in &programa.library(lib).units {
        let a = &programa.unit(u).ast;
        if sem_metadados_genericos {
            for t in &a.types {
                let TypeKind::Named { args, .. } = &t.kind else { continue };
                for &x in args.iter() {
                    if funcao_generica(programa, u, a, x, 0) {
                        saida.push((u, Diagnostic::com_codigo(c::GENERIC_FUNCTION_TYPE_CANNOT_BE_TYPE_ARGUMENT, a.ty(x).span, [] as [&str; 0])));
                    }
                }
            }
        }
        let todas = listas(a);
        // Os parâmetros de tipo em escopo numa posição: os das listas cujo
        // dono a contém.
        let visiveis = |pos: usize, nome: SymbolId| {
            todas.iter().any(|l| l.escopo.start <= pos && pos <= l.escopo.end && l.parametros.iter().any(|p| p.name.sym == nome))
        };
        for l in &todas {
            // `visitTypeParameter`: o limite de cada parâmetro.
            for tp in l.parametros {
                if let Some(b) = tp.bound {
                    nao_instanciados(programa, u, a, b, &visiveis, limites, &mut saida);
                }
            }
            // `_checkForTypeParameterBoundRecursion`.
            let n = l.parametros.len();
            for (i, tp) in l.parametros.iter().enumerate() {
                if tp.bound.is_none() {
                    continue;
                }
                let mut atual = Some(i);
                let mut passo = 0usize;
                while let Some(k) = atual {
                    atual = l.parametros[k].bound.and_then(|b| parametro_do_limite(programa, u, a, b, l.parametros, 0));
                    if passo == n {
                        // `{1}` (só na correção): o limite como escrito.
                        let nome = nomes.resolve(tp.name.sym).to_string();
                        let limite = tp.bound.map(|b| a.ty(b).span).map(|s| programa.unit(u).source[s.start..s.end].to_string()).unwrap_or_default();
                        saida.push((u, Diagnostic::com_codigo(c::TYPE_PARAMETER_SUPERTYPE_OF_ITS_BOUND, tp.name.span, [nome.as_str(), limite.as_str()])));
                        break;
                    }
                    passo += 1;
                }
            }
        }
    }
    saida
}
