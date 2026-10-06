//! `avoid_futureor_void` e `unnecessary_lambdas`
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2, com a variância e a subtipagem do motor.
//!
//! * `avoid_futureor_void`: o `FutureOr<void>` escrito em posição que não é
//!   só de entrada (tipos de `as`/`is`/padrões, cláusulas de supertipo,
//!   tipos de variáveis, retornos, limites), com a variância dos parâmetros
//!   de tipo do elemento (o legado é covariante).
//! * `unnecessary_lambdas`: a closure que só repassa os parâmetros a uma
//!   invocação cujo alvo é final, ou a um construtor (com tear-offs de
//!   construtor).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::lints_tipados::Achado;
use crate::resolve::OutlineTypes;
use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionKind, FunctionRef, Program, UnitId, VariableRef};
use dartforge_frontend::ast::{self, ExprId, ExprKind, FunctionBody, MemberKind, ParameterKind, PatternKind, StmtKind, TypeKind};
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variancia {
    Saida,
    Entrada,
    Ambas,
}

impl Variancia {
    fn inversa(self) -> Variancia {
        match self {
            Variancia::Saida => Variancia::Entrada,
            Variancia::Entrada => Variancia::Saida,
            Variancia::Ambas => Variancia::Ambas,
        }
    }
}

/// Os achados na unidade `u`.
#[allow(clippy::too_many_arguments)]
pub fn achados(program: &Program, table: &mut TypeTable, core: &CoreTypes, outline: &OutlineTypes, corpo: &UnitBodyTypes, u: UnitId) -> Vec<Achado> {
    let mut out = Vec::new();
    futureor_void(program, table, outline, corpo, u, &mut out);
    lambdas(program, table, core, outline, corpo, u, &mut out);
    out
}

/// `avoid_futureor_void`.
fn futureor_void(program: &Program, table: &TypeTable, outline: &OutlineTypes, corpo: &UnitBodyTypes, u: UnitId, out: &mut Vec<Achado>) {
    let a = &program.unit(u).ast;
    let resolvido = |t: ast::TypeId| corpo.tipos_de_anotacoes.get(&t).copied().or_else(|| outline.tipos_escritos.get(&(u, t)).copied());
    let mut achados: HashSet<(usize, usize)> = HashSet::new();
    // As variâncias dos parâmetros de tipo do elemento de um tipo nomeado.
    let variancias = |t: TypeId| -> Option<Vec<Option<ast::Variance>>> {
        let (class, _) = match table.get(t) {
            Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => (*class, args.len()),
            _ => return None,
        };
        let r = program.class(class).decl?;
        let a2 = &program.unit(r.unit).ast;
        let tps: &[ast::TypeParameter] = match &a2.decl(r.decl).kind {
            ast::DeclKind::Class(x) => &x.type_params,
            ast::DeclKind::Mixin(x) => &x.type_params,
            ast::DeclKind::Enum(x) => &x.type_params,
            _ => return None,
        };
        Some(tps.iter().map(|p| p.variance.map(|v| v.0)).collect())
    };
    fn checar(
        a: &ast::Ast,
        table: &TypeTable,
        resolvido: &dyn Fn(ast::TypeId) -> Option<TypeId>,
        variancias: &dyn Fn(TypeId) -> Option<Vec<Option<ast::Variance>>>,
        v: Variancia,
        t: ast::TypeId,
        achados: &mut HashSet<(usize, usize)>,
    ) {
        match &a.ty(t).kind {
            TypeKind::Named { args, .. } => {
                let tipo = resolvido(t);
                if !args.is_empty() {
                    let vs = tipo.and_then(variancias).filter(|x| x.len() == args.len());
                    match vs {
                        Some(vs) => {
                            for (arg, var) in args.iter().zip(vs) {
                                let pv = match var {
                                    None | Some(ast::Variance::Out) => v,
                                    Some(ast::Variance::In) => v.inversa(),
                                    Some(ast::Variance::Inout) => Variancia::Ambas,
                                };
                                checar(a, table, resolvido, variancias, pv, *arg, achados);
                            }
                        }
                        None => {
                            for arg in args.iter() {
                                checar(a, table, resolvido, variancias, v, *arg, achados);
                            }
                        }
                    }
                }
                let Some(tipo) = tipo else { return };
                if v == Variancia::Entrada {
                    return;
                }
                if let Type::FutureOr { arg, .. } = table.get(tipo)
                    && matches!(table.get(*arg), Type::Void)
                {
                    let sp = a.ty(t).span;
                    achados.insert((sp.start, sp.end));
                }
            }
            TypeKind::Function { return_type, parameters, .. } => {
                if let Some(r) = return_type {
                    checar(a, table, resolvido, variancias, v, *r, achados);
                }
                for p in parameters.iter() {
                    parametro(a, table, resolvido, variancias, v.inversa(), p, achados);
                }
            }
            TypeKind::Record { positional, named } => {
                for x in positional.iter() {
                    checar(a, table, resolvido, variancias, v, *x, achados);
                }
                for (_, x) in named.iter() {
                    checar(a, table, resolvido, variancias, v, *x, achados);
                }
            }
            _ => {}
        }
    }
    fn parametro(
        a: &ast::Ast,
        table: &TypeTable,
        resolvido: &dyn Fn(ast::TypeId) -> Option<TypeId>,
        variancias: &dyn Fn(TypeId) -> Option<Vec<Option<ast::Variance>>>,
        v: Variancia,
        p: &ast::Parameter,
        achados: &mut HashSet<(usize, usize)>,
    ) {
        if let Some(internos) = &p.function_parameters {
            if let Some(r) = p.ty {
                checar(a, table, resolvido, variancias, v, r, achados);
            }
            for tp in p.function_type_params.iter() {
                if let Some(b) = tp.bound {
                    checar(a, table, resolvido, variancias, Variancia::Ambas, b, achados);
                }
            }
            for q in internos.iter() {
                parametro(a, table, resolvido, variancias, v.inversa(), q, achados);
            }
            return;
        }
        if let Some(t) = p.ty {
            checar(a, table, resolvido, variancias, v, t, achados);
        }
    }
    let saida = Variancia::Saida;
    let ch = |v: Variancia, t: ast::TypeId, achados: &mut HashSet<(usize, usize)>| checar(a, table, &resolvido, &variancias, v, t, achados);
    // `as`, `is`, padrões de cast e de objeto.
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::As { ty, .. } | ExprKind::Is { ty, .. } => ch(saida, *ty, &mut achados),
            _ => {}
        }
    }
    for p in a.patterns.iter() {
        match &p.kind {
            PatternKind::Cast { ty, .. } | PatternKind::Object { ty, .. } => ch(saida, *ty, &mut achados),
            _ => {}
        }
    }
    // Cláusulas de supertipo, `on`, representação, listas de variáveis.
    for d in a.decls.iter() {
        match &d.kind {
            ast::DeclKind::Class(x) => {
                // `class C = S with M`: o `S` não está num `ExtendsClause`.
                let extends = if x.mixin_application { None } else { x.extends };
                for t in extends.iter().chain(x.with.iter()).chain(x.implements.iter()) {
                    ch(saida, *t, &mut achados);
                }
            }
            ast::DeclKind::Mixin(x) => {
                for t in x.on.iter().chain(x.implements.iter()) {
                    ch(saida, *t, &mut achados);
                }
            }
            ast::DeclKind::Enum(x) => {
                for t in x.with.iter().chain(x.implements.iter()) {
                    ch(saida, *t, &mut achados);
                }
            }
            ast::DeclKind::Extension(x) => ch(saida, x.on, &mut achados),
            ast::DeclKind::ExtensionType(x) => {
                ch(saida, x.representation_type, &mut achados);
                for t in x.implements.iter() {
                    ch(saida, *t, &mut achados);
                }
            }
            ast::DeclKind::Variables(l) => {
                if let Some(t) = l.ty {
                    ch(saida, t, &mut achados);
                }
            }
            _ => {}
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Field(l) = &m.kind
            && let Some(t) = l.ty
        {
            ch(saida, t, &mut achados);
        }
    }
    for s in a.stmts.iter() {
        match &s.kind {
            StmtKind::Variables(l) | StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => {
                if let Some(t) = l.ty {
                    ch(saida, t, &mut achados);
                }
            }
            _ => {}
        }
    }
    // Os limites de todos os parâmetros de tipo.
    let mut limites: Vec<ast::TypeId> = Vec::new();
    for d in a.decls.iter() {
        let tps: &[ast::TypeParameter] = match &d.kind {
            ast::DeclKind::Class(x) => &x.type_params,
            ast::DeclKind::Mixin(x) => &x.type_params,
            ast::DeclKind::Enum(x) => &x.type_params,
            ast::DeclKind::Extension(x) => &x.type_params,
            ast::DeclKind::ExtensionType(x) => &x.type_params,
            ast::DeclKind::Typedef(x) => &x.type_params,
            _ => &[],
        };
        limites.extend(tps.iter().filter_map(|p| p.bound));
    }
    for f in a.functions.iter() {
        limites.extend(f.type_params.iter().filter_map(|p| p.bound));
    }
    // Os parâmetros de tipo de parâmetros-função (`void f(T g<T extends X>())`),
    // em qualquer lista de parâmetros.
    fn de_parametros(ps: &[ast::Parameter], limites: &mut Vec<ast::TypeId>) {
        for p in ps.iter() {
            limites.extend(p.function_type_params.iter().filter_map(|q| q.bound));
            if let Some(internos) = &p.function_parameters {
                de_parametros(internos, limites);
            }
        }
    }
    for t in a.types.iter() {
        if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
            limites.extend(type_params.iter().filter_map(|p| p.bound));
            de_parametros(parameters, &mut limites);
        }
    }
    for f in a.functions.iter() {
        de_parametros(f.parameters.as_deref().unwrap_or(&[]), &mut limites);
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            de_parametros(&k.parameters, &mut limites);
        }
    }
    for b in limites {
        ch(Variancia::Ambas, b, &mut achados);
    }
    // Funções declaradas e métodos: o retorno e os parâmetros (de entrada).
    let mut declaradas: Vec<ast::FunctionId> = Vec::new();
    for d in a.decls.iter() {
        if let ast::DeclKind::Function(f) = &d.kind {
            declaradas.push(*f);
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Method(f) = &m.kind {
            declaradas.push(*f);
        }
    }
    for s in a.stmts.iter() {
        if let StmtKind::Function(f) = &s.kind {
            declaradas.push(*f);
        }
    }
    for fid in declaradas {
        let f = a.function(fid);
        if let Some(r) = f.return_type {
            ch(saida, r, &mut achados);
        }
        for p in f.parameters.as_deref().unwrap_or(&[]) {
            parametro(a, table, &resolvido, &variancias, Variancia::Entrada, p, &mut achados);
        }
    }
    let mut v: Vec<(usize, usize)> = achados.into_iter().collect();
    v.sort();
    for (i, f) in v {
        out.push((Span { start: i, end: f }, "avoid_futureor_void", Vec::new()));
    }
}

/// O parâmetro `nome` da função ou do construtor mais de dentro que contém
/// `e` e o declara (o elemento de `Resolved::Parameter`).
fn parametro_declarado(a: &ast::Ast, e: Span, nome: dartforge_intern::SymbolId) -> Option<&ast::Parameter> {
    let mut melhor: Option<(usize, &ast::Parameter)> = None;
    let mut listas: Vec<(Span, &[ast::Parameter])> = Vec::new();
    for f in a.functions.iter() {
        if let Some(ps) = f.parameters.as_deref() {
            listas.push((f.span, ps));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            listas.push((m.span, &k.parameters[..]));
        }
    }
    for (sp, ps) in listas {
        if e.start < sp.start || e.end > sp.end {
            continue;
        }
        if let Some(p) = ps.iter().find(|p| p.name.is_some_and(|n| n.sym == nome)) {
            let tamanho = sp.end - sp.start;
            if melhor.is_none_or(|(t, _)| tamanho < t) {
                melhor = Some((tamanho, p));
            }
        }
    }
    melhor.map(|(_, p)| p)
}

/// O `isFinal` do elemento do local declarado em `d` (o offset do nome):
/// a variável `final` e não `late` (o `const` não é `final` no elemento);
/// a função local não é variável e conta como final.
fn local_final(a: &ast::Ast, d: usize) -> bool {
    for s in a.stmts.iter() {
        match &s.kind {
            StmtKind::Variables(l) | StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } if l.variables.iter().any(|v| v.name.span.start == d) => {
                return l.final_ && !l.late;
            }
            StmtKind::Function(f) if a.function(*f).name.is_some_and(|n| n.span.start == d) => return true,
            StmtKind::ForIn { target: ast::ForInTarget::Declared { final_, name, .. }, .. } if name.span.start == d => return *final_,
            _ => {}
        }
    }
    // A variável de padrão: o `final` dela ou o da declaração que a contém.
    for p in a.patterns.iter() {
        let PatternKind::Variable { final_, name, .. } = &p.kind else { continue };
        if name.span.start != d {
            continue;
        }
        if *final_ {
            return true;
        }
        let contem = |q: ast::PatternId| {
            let sp = a.pattern(q).span;
            sp.start <= d && d < sp.end
        };
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::PatternVariables { final_, pattern, .. }
                | StmtKind::For { init: Some(ast::ForInit::Pattern { final_, pattern, .. }), .. }
                | StmtKind::ForIn { target: ast::ForInTarget::Pattern { final_, pattern }, .. }
                    if contem(*pattern) =>
                {
                    return *final_;
                }
                _ => {}
            }
        }
        return false;
    }
    false
}

/// O `isFinal` de uma variável do modelo (pelo getter sintético dela).
fn variavel_final(program: &Program, v: dartforge_elements::model::VariableId) -> bool {
    let x = program.variable(v);
    x.final_ && !x.const_ && !x.late
}

/// O `isFinal` de uma função do modelo: o getter sintético segue a
/// variável; o getter escrito não é final; o resto não é variável.
fn funcao_final(program: &Program, f: dartforge_elements::model::FunctionElementId) -> bool {
    let g = program.function(f);
    match g.kind {
        FunctionKind::Getter | FunctionKind::Setter => match (g.node, g.variable) {
            (FunctionRef::None, Some(v)) => variavel_final(program, v),
            _ => false,
        },
        _ => true,
    }
}

/// `unnecessary_lambdas`.
#[allow(clippy::too_many_arguments)]
fn lambdas(program: &Program, table: &mut TypeTable, core: &CoreTypes, outline: &OutlineTypes, corpo: &UnitBodyTypes, u: UnitId, out: &mut Vec<Achado>) {
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let biblioteca = program.library(unidade.library);
    let tearoffs = biblioteca.features.versao() >= dartforge_frontend::features::LanguageVersion::new(2, 15);
    let sem_parenteses = |mut e: ExprId| {
        while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
            e = *x;
        }
        e
    };
    // `isFinal` do `staticElement` de um identificador (o nulo conta).
    let elemento_final = |e: ExprId| -> bool {
        match corpo.get_resolved(e) {
            Some(Resolved::Local(_)) => corpo.declaracao_local(e).is_some_and(|d| local_final(a, d)),
            Some(Resolved::Parameter { name, .. }) => parametro_declarado(a, a.expr(e).span, *name).is_some_and(|p| p.final_),
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) | Some(Resolved::Element(Element::Variable(v))) => variavel_final(program, *v),
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) => funcao_final(program, *f),
            Some(Resolved::ExtensionMember { member, .. }) => funcao_final(program, *member),
            _ => true,
        }
    };
    // O identificador `p` que nomeia um prefixo de importação adiada.
    let prefixo_adiado = |e: ExprId| -> bool {
        let ExprKind::Identifier(n) = &a.expr(e).kind else { return false };
        if matches!(
            corpo.get_resolved(e),
            Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) | Some(Resolved::Member { .. }) | Some(Resolved::ExtensionMember { .. })
        ) || matches!(corpo.get_resolved(e), Some(Resolved::Element(x)) if !matches!(x, Element::Prefix(..)))
        {
            return false;
        }
        biblioteca.imports.iter().any(|i| i.unit == u && i.prefix == Some(n.sym) && i.deferred)
    };
    // `mightBeDeferred`: o identificador simples ou o prefixo de `p.x`.
    let pode_ser_adiado = |e: Option<ExprId>| -> bool {
        let Some(e) = e else { return false };
        match &a.expr(e).kind {
            ExprKind::Identifier(_) => prefixo_adiado(e),
            ExprKind::Property { target, null_aware: false, .. } => prefixo_adiado(*target),
            _ => false,
        }
    };
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        let ExprKind::FunctionExpression(fid) = &e.kind else { continue };
        let f = a.function(*fid);
        if f.name.is_some() || !matches!(f.modifier, ast::AsyncModifier::None) {
            continue;
        }
        let ps: &[ast::Parameter] = f.parameters.as_deref().unwrap_or(&[]);
        let (inv, corpo_de_expressao) = match &f.body {
            FunctionBody::Block(b) => match &a.stmt(*b).kind {
                StmtKind::Block(cmds) if cmds.len() == 1 => match &a.stmt(cmds[0]).kind {
                    StmtKind::Expression(x) | StmtKind::Return(Some(x)) => (*x, false),
                    _ => continue,
                },
                _ => continue,
            },
            FunctionBody::Expression(x) => (*x, true),
            _ => continue,
        };
        // A criação de instância (`C(e)`, `new C(e)`, `p.C.n(e)`).
        let criacao: Option<(ExprId, &ast::Arguments, bool)> = match &a.expr(inv).kind {
            ExprKind::InstanceCreation { keyword, ty, arguments, .. } => {
                let adiado = match &a.ty(*ty).kind {
                    TypeKind::Named { name, .. } if name.len() == 2 => biblioteca.imports.iter().any(|i| i.unit == u && i.prefix == Some(name[0].sym) && i.deferred),
                    _ => false,
                };
                let constante = matches!(keyword, Some(ast::CreationKeyword::Const));
                Some((inv, &**arguments, adiado || constante))
            }
            ExprKind::Call { target, arguments } if matches!(corpo.get_resolved(inv), Some(Resolved::Constructor(_))) => {
                // O identificador mais à esquerda do nome do construtor.
                let mut x = *target;
                let mut partes = 0;
                while let ExprKind::Property { target: t, .. } = &a.expr(x).kind {
                    x = *t;
                    partes += 1;
                }
                let adiado = partes > 0 && prefixo_adiado(x);
                Some((inv, &**arguments, adiado))
            }
            _ => None,
        };
        if let Some((criada, arguments, pula)) = criacao {
            // Só no corpo de expressão, com tear-offs de construtor.
            if !corpo_de_expressao || !tearoffs || pula {
                continue;
            }
            let (Some(tipo_closure), Some(Resolved::Constructor(kf))) = (corpo.get_type(id), corpo.get_resolved(criada)) else { continue };
            let Some(dados) = outline.functions.get(kf.0 as usize) else { continue };
            let mut tipo_construtor = dados.signature;
            // O `ConstructorMember` instanciado pelos argumentos do tipo criado.
            if let Some(classe) = program.function(*kf).class
                && let Some(criado) = corpo.get_type(criada)
                && let Type::Interface { args, .. } = table.get(criado).clone()
            {
                let params = outline.classes.get(classe.0 as usize).map(|x| x.type_params.clone()).unwrap_or_default();
                let mapa: std::collections::HashMap<crate::table::TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
                tipo_construtor = crate::ops::substitute(tipo_construtor, &mapa, table);
            }
            let atribuivel = {
                let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                is_subtype(tipo_construtor, tipo_closure, &mut env)
            };
            if !atribuivel {
                continue;
            }
            if arguments.args.iter().any(|x| x.name.is_some() || !matches!(a.expr(x.value).kind, ExprKind::Identifier(_))) {
                continue;
            }
            if ps.len() != arguments.args.len() {
                continue;
            }
            let iguais = arguments.args.iter().zip(ps.iter()).all(|(x, p)| matches!(&a.expr(x.value).kind, ExprKind::Identifier(n) if Some(n.sym) == p.name.map(|q| q.sym)));
            if iguais {
                out.push((e.span, "unnecessary_lambdas", Vec::new()));
            }
            continue;
        }
        let ExprKind::Call { target, arguments } = &a.expr(inv).kind else { continue };
        // `argumentsMatchParameters`: o `canonicalElement` de cada argumento
        // é o parâmetro de mesma posição (ou de mesmo nome) da closure.
        // O parâmetro da closure a que `x` resolve: os parâmetros de uma
        // expressão de função ficam no escopo como locais declarados no nome
        // deles.
        let parametro_da_closure = |x: ExprId| -> Option<dartforge_intern::SymbolId> {
            let ExprKind::Identifier(n) = &a.expr(x).kind else { return None };
            let p = ps.iter().find(|p| p.name.is_some_and(|q| q.sym == n.sym))?;
            match corpo.get_resolved(x) {
                Some(Resolved::Parameter { .. }) => Some(n.sym),
                Some(Resolved::Local(_)) if corpo.declaracao_local(x) == p.name.map(|q| q.span.start) => Some(n.sym),
                _ => None,
            }
        };
        let elemento_do_argumento = |x: ExprId| -> Option<dartforge_intern::SymbolId> { parametro_da_closure(sem_parenteses(x)) };
        let posicionais: Vec<dartforge_intern::SymbolId> = ps.iter().filter(|p| p.kind != ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
        let nomeados: Vec<dartforge_intern::SymbolId> = ps.iter().filter(|p| p.kind == ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
        let mut args_pos = Vec::new();
        let mut args_nom: Vec<(dartforge_intern::SymbolId, Option<dartforge_intern::SymbolId>)> = Vec::new();
        let mut casam = true;
        for x in arguments.args.iter() {
            let el = elemento_do_argumento(x.value);
            match x.name {
                Some(n) => args_nom.push((n.sym, el)),
                None => match el {
                    Some(el) => args_pos.push(el),
                    None => {
                        casam = false;
                        break;
                    }
                },
            }
        }
        if !casam || posicionais != args_pos || nomeados.len() != args_nom.len() {
            continue;
        }
        if !nomeados.iter().all(|n| args_nom.iter().any(|(r, el)| r == n && *el == Some(*n))) {
            continue;
        }
        let params: HashSet<dartforge_intern::SymbolId> = ps.iter().filter_map(|p| p.name.map(|n| n.sym)).collect();
        // O identificador que referencia um parâmetro da closure.
        let e_parametro = |x: ExprId| -> bool {
            matches!(&a.expr(x).kind, ExprKind::Identifier(n) if params.contains(&n.sym))
                && parametro_da_closure(x).is_some()
                && a.expr(x).span.start >= f.span.start
                && a.expr(x).span.end <= f.span.end
        };
        // `_FinalExpressionChecker.isFinalNode`.
        fn final_no(a: &ast::Ast, e_parametro: &dyn Fn(ExprId) -> bool, elemento_final: &dyn Fn(ExprId) -> bool, e: Option<ExprId>) -> bool {
            let Some(mut e) = e else { return true };
            while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
                e = *x;
            }
            match &a.expr(e).kind {
                ExprKind::FunctionExpression(g) => {
                    let sp = a.function(*g).span;
                    !a.exprs.iter().enumerate().any(|(k, x)| x.span.start >= sp.start && x.span.end <= sp.end && e_parametro(ExprId(k as u32)))
                }
                // `PrefixedIdentifier` e `PropertyAccess`: o alvo e o nome.
                ExprKind::Property { target, .. } => final_no(a, e_parametro, elemento_final, Some(*target)) && elemento_final(e),
                ExprKind::Identifier(_) => !e_parametro(e) && elemento_final(e),
                _ => false,
            }
        }
        if !crate::lints_tipados4::e_invocacao_de_metodo(program, a, corpo, *target) {
            // `FunctionExpressionInvocation`.
            if pode_ser_adiado(Some(*target)) {
                continue;
            }
            if final_no(a, &e_parametro, &elemento_final, Some(*target)) {
                out.push((e.span, "unnecessary_lambdas", Vec::new()));
            }
            continue;
        }
        // `MethodInvocation`.
        let (alvo, null_aware) = match &a.expr(*target).kind {
            ExprKind::Property { target: t, null_aware, .. } => (Some(*t), *null_aware),
            ExprKind::Identifier(_) => (None, false),
            _ => continue,
        };
        if pode_ser_adiado(alvo) {
            continue;
        }
        // `staticInvokeType`: o tipo do membro, instanciado pelos argumentos
        // de tipo da chamada.
        let Some(mut tearoff) = corpo.get_type(*target) else { continue };
        if let Some(inst) = corpo.instanciacao(arguments.span.start).map(|x| x.to_vec())
            && let Type::Function { type_params, .. } = table.get(tearoff)
            && type_params.len() == inst.len()
        {
            let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
            tearoff = crate::constraints::instanciar_funcao(tearoff, &inst, &mut env);
        }
        // O pai da closure: o argumento nomeado (o tipo estático é o da
        // própria closure) ou a declaração de variável.
        let pai_nomeado = a.exprs.iter().any(|x| match &x.kind {
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => arguments.args.iter().any(|y| y.value == id && y.name.is_some()),
            _ => false,
        });
        let destino: Option<Option<TypeId>> = if pai_nomeado {
            Some(corpo.get_type(id))
        } else {
            let mut d = None;
            for s in a.stmts.iter() {
                if let StmtKind::Variables(l) | StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } = &s.kind
                    && let Some(v) = l.variables.iter().find(|v| v.initializer == Some(id))
                {
                    d = Some(corpo.tipo_local(v.name.span.start));
                }
            }
            if d.is_none() {
                let tipo_da_variavel = |r: VariableRef| {
                    program.variables.iter().position(|x| x.node == r).and_then(|i| outline.variables.get(i)).and_then(|x| x.declared_type.or(x.inferred))
                };
                for (mi, m) in a.members.iter().enumerate() {
                    if let MemberKind::Field(l) = &m.kind
                        && let Some(index) = l.variables.iter().position(|v| v.initializer == Some(id))
                    {
                        d = Some(tipo_da_variavel(VariableRef::Field { unit: u, member: ast::MemberId(mi as u32), index }));
                    }
                }
                for (di, x) in a.decls.iter().enumerate() {
                    if let ast::DeclKind::Variables(l) = &x.kind
                        && let Some(index) = l.variables.iter().position(|v| v.initializer == Some(id))
                    {
                        d = Some(tipo_da_variavel(VariableRef::TopLevel { unit: u, decl: ast::DeclId(di as u32), index }));
                    }
                }
            }
            d
        };
        if let Some(dt) = destino {
            let Some(dt) = dt else { continue };
            let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
            if !is_subtype(tearoff, dt, &mut env) {
                continue;
            }
        }
        // `containsNullAwareInvocationInChain`.
        let mut cadeia_nula = null_aware;
        let mut x = alvo;
        while let Some(y) = x {
            match &a.expr(y).kind {
                ExprKind::Property { target, null_aware, .. } => {
                    cadeia_nula |= *null_aware;
                    x = Some(*target);
                }
                ExprKind::Call { target, .. } if crate::lints_tipados4::e_invocacao_de_metodo(program, a, corpo, *target) => match &a.expr(*target).kind {
                    ExprKind::Property { target: t, null_aware, .. } => {
                        cadeia_nula |= *null_aware;
                        x = Some(*t);
                    }
                    _ => break,
                },
                ExprKind::Index { target, null_aware, .. } => {
                    cadeia_nula |= *null_aware;
                    x = Some(*target);
                }
                _ => break,
            }
        }
        if !cadeia_nula && final_no(a, &e_parametro, &elemento_final, alvo) && elemento_final(*target) && arguments.type_args.is_empty() {
            out.push((e.span, "unnecessary_lambdas", Vec::new()));
        }
    }
}
