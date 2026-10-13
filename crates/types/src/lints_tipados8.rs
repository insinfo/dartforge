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
enum Variance_ {
    Saida,
    Entrada,
    Both,
}

impl Variance_ {
    fn inverse(self) -> Variance_ {
        match self {
            Variance_::Saida => Variance_::Entrada,
            Variance_::Entrada => Variance_::Saida,
            Variance_::Both => Variance_::Both,
        }
    }
}

/// Os achados na unidade `u`.
#[allow(clippy::too_many_arguments)]
pub fn achados(program: &Program, table: &mut TypeTable, core: &CoreTypes, outline: &OutlineTypes, body: &UnitBodyTypes, u: UnitId) -> Vec<Achado> {
    let mut out = Vec::new();
    futureor_void(program, table, outline, body, u, &mut out);
    lambdas(program, table, core, outline, body, u, &mut out);
    out
}

/// `avoid_futureor_void`.
fn futureor_void(program: &Program, table: &TypeTable, outline: &OutlineTypes, body: &UnitBodyTypes, u: UnitId, out: &mut Vec<Achado>) {
    let a = &program.unit(u).ast;
    let resolved = |t: ast::TypeId| body.tipos_de_anotacoes.get(&t).copied().or_else(|| outline.tipos_escritos.get(&(u, t)).copied());
    let mut findings: HashSet<(usize, usize)> = HashSet::new();
    // As variâncias dos parâmetros de tipo do elemento de um tipo nomeado.
    let variances = |t: TypeId| -> Option<Vec<Option<ast::Variance>>> {
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
        resolved: &dyn Fn(ast::TypeId) -> Option<TypeId>,
        variances: &dyn Fn(TypeId) -> Option<Vec<Option<ast::Variance>>>,
        v: Variance_,
        t: ast::TypeId,
        findings: &mut HashSet<(usize, usize)>,
    ) {
        match &a.ty(t).kind {
            TypeKind::Named { args, .. } => {
                let typ = resolved(t);
                if !args.is_empty() {
                    let vs = typ.and_then(variances).filter(|x| x.len() == args.len());
                    match vs {
                        Some(vs) => {
                            for (arg, var) in args.iter().zip(vs) {
                                let pv = match var {
                                    None | Some(ast::Variance::Out) => v,
                                    Some(ast::Variance::In) => v.inverse(),
                                    Some(ast::Variance::Inout) => Variance_::Both,
                                };
                                checar(a, table, resolved, variances, pv, *arg, findings);
                            }
                        }
                        None => {
                            for arg in args.iter() {
                                checar(a, table, resolved, variances, v, *arg, findings);
                            }
                        }
                    }
                }
                let Some(typ) = typ else { return };
                if v == Variance_::Entrada {
                    return;
                }
                if let Type::FutureOr { arg, .. } = table.get(typ)
                    && matches!(table.get(*arg), Type::Void)
                {
                    let sp = a.ty(t).span;
                    findings.insert((sp.start, sp.end));
                }
            }
            TypeKind::Function { return_type, parameters, .. } => {
                if let Some(r) = return_type {
                    checar(a, table, resolved, variances, v, *r, findings);
                }
                for p in parameters.iter() {
                    parametro(a, table, resolved, variances, v.inverse(), p, findings);
                }
            }
            TypeKind::Record { positional, named } => {
                for x in positional.iter() {
                    checar(a, table, resolved, variances, v, *x, findings);
                }
                for (_, x) in named.iter() {
                    checar(a, table, resolved, variances, v, *x, findings);
                }
            }
            _ => {}
        }
    }
    fn parametro(
        a: &ast::Ast,
        table: &TypeTable,
        resolved: &dyn Fn(ast::TypeId) -> Option<TypeId>,
        variances: &dyn Fn(TypeId) -> Option<Vec<Option<ast::Variance>>>,
        v: Variance_,
        p: &ast::Parameter,
        findings: &mut HashSet<(usize, usize)>,
    ) {
        if let Some(internal) = &p.function_parameters {
            if let Some(r) = p.ty {
                checar(a, table, resolved, variances, v, r, findings);
            }
            for tp in p.function_type_params.iter() {
                if let Some(b) = tp.bound {
                    checar(a, table, resolved, variances, Variance_::Both, b, findings);
                }
            }
            for q in internal.iter() {
                parametro(a, table, resolved, variances, v.inverse(), q, findings);
            }
            return;
        }
        if let Some(t) = p.ty {
            checar(a, table, resolved, variances, v, t, findings);
        }
    }
    let output = Variance_::Saida;
    let ch = |v: Variance_, t: ast::TypeId, findings: &mut HashSet<(usize, usize)>| checar(a, table, &resolved, &variances, v, t, findings);
    // `as`, `is`, padrões de cast e de objeto.
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::As { ty, .. } | ExprKind::Is { ty, .. } => ch(output, *ty, &mut findings),
            _ => {}
        }
    }
    for p in a.patterns.iter() {
        match &p.kind {
            PatternKind::Cast { ty, .. } | PatternKind::Object { ty, .. } => ch(output, *ty, &mut findings),
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
                    ch(output, *t, &mut findings);
                }
            }
            ast::DeclKind::Mixin(x) => {
                for t in x.on.iter().chain(x.implements.iter()) {
                    ch(output, *t, &mut findings);
                }
            }
            ast::DeclKind::Enum(x) => {
                for t in x.with.iter().chain(x.implements.iter()) {
                    ch(output, *t, &mut findings);
                }
            }
            ast::DeclKind::Extension(x) => ch(output, x.on, &mut findings),
            ast::DeclKind::ExtensionType(x) => {
                ch(output, x.representation_type, &mut findings);
                for t in x.implements.iter() {
                    ch(output, *t, &mut findings);
                }
            }
            ast::DeclKind::Variables(l) => {
                if let Some(t) = l.ty {
                    ch(output, t, &mut findings);
                }
            }
            _ => {}
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Field(l) = &m.kind
            && let Some(t) = l.ty
        {
            ch(output, t, &mut findings);
        }
    }
    for s in a.stmts.iter() {
        match &s.kind {
            StmtKind::Variables(l) | StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => {
                if let Some(t) = l.ty {
                    ch(output, t, &mut findings);
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
            if let Some(internal) = &p.function_parameters {
                de_parametros(internal, limites);
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
        ch(Variance_::Both, b, &mut findings);
    }
    // Funções declaradas e métodos: o retorno e os parâmetros (de entrada).
    let mut the_declared: Vec<ast::FunctionId> = Vec::new();
    for d in a.decls.iter() {
        if let ast::DeclKind::Function(f) = &d.kind {
            the_declared.push(*f);
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Method(f) = &m.kind {
            the_declared.push(*f);
        }
    }
    for s in a.stmts.iter() {
        if let StmtKind::Function(f) = &s.kind {
            the_declared.push(*f);
        }
    }
    for fid in the_declared {
        let f = a.function(fid);
        if let Some(r) = f.return_type {
            ch(output, r, &mut findings);
        }
        for p in f.parameters.as_deref().unwrap_or(&[]) {
            parametro(a, table, &resolved, &variances, Variance_::Entrada, p, &mut findings);
        }
    }
    let mut v: Vec<(usize, usize)> = findings.into_iter().collect();
    v.sort();
    for (i, f) in v {
        out.push((Span { start: i, end: f }, "avoid_futureor_void", Vec::new()));
    }
}

/// O parâmetro `nome` da função ou do construtor mais de dentro que contém
/// `e` e o declara (o elemento de `Resolved::Parameter`).
fn declared_parameter(a: &ast::Ast, e: Span, the_name: dartforge_intern::SymbolId) -> Option<&ast::Parameter> {
    let mut best: Option<(usize, &ast::Parameter)> = None;
    let mut lists: Vec<(Span, &[ast::Parameter])> = Vec::new();
    for f in a.functions.iter() {
        if let Some(ps) = f.parameters.as_deref() {
            lists.push((f.span, ps));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            lists.push((m.span, &k.parameters[..]));
        }
    }
    for (sp, ps) in lists {
        if e.start < sp.start || e.end > sp.end {
            continue;
        }
        if let Some(p) = ps.iter().find(|p| p.name.is_some_and(|n| n.sym == the_name)) {
            let size = sp.end - sp.start;
            if best.is_none_or(|(t, _)| size < t) {
                best = Some((size, p));
            }
        }
    }
    best.map(|(_, p)| p)
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
        let contains = |q: ast::PatternId| {
            let sp = a.pattern(q).span;
            sp.start <= d && d < sp.end
        };
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::PatternVariables { final_, pattern, .. }
                | StmtKind::For { init: Some(ast::ForInit::Pattern { final_, pattern, .. }), .. }
                | StmtKind::ForIn { target: ast::ForInTarget::Pattern { final_, pattern }, .. }
                    if contains(*pattern) =>
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
fn final_variable(program: &Program, v: dartforge_elements::model::VariableId) -> bool {
    let x = program.variable(v);
    x.final_ && !x.const_ && !x.late
}

/// O `isFinal` de uma função do modelo: o getter sintético segue a
/// variável; o getter escrito não é final; o resto não é variável.
fn final_function(program: &Program, f: dartforge_elements::model::FunctionElementId) -> bool {
    let g = program.function(f);
    match g.kind {
        FunctionKind::Getter | FunctionKind::Setter => match (g.node, g.variable) {
            (FunctionRef::None, Some(v)) => final_variable(program, v),
            _ => false,
        },
        _ => true,
    }
}

/// `unnecessary_lambdas`.
#[allow(clippy::too_many_arguments)]
fn lambdas(program: &Program, table: &mut TypeTable, core: &CoreTypes, outline: &OutlineTypes, body: &UnitBodyTypes, u: UnitId, out: &mut Vec<Achado>) {
    let unit = program.unit(u);
    let a = &unit.ast;
    let library = program.library(unit.library);
    let tearoffs = library.features.versao() >= dartforge_frontend::features::LanguageVersion::new(2, 15);
    let without_parentheses = |mut e: ExprId| {
        while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
            e = *x;
        }
        e
    };
    // `isFinal` do `staticElement` de um identificador (o nulo conta).
    let final_element = |e: ExprId| -> bool {
        match body.get_resolved(e) {
            Some(Resolved::Local(_)) => body.declaracao_local(e).is_some_and(|d| local_final(a, d)),
            Some(Resolved::Parameter { name, .. }) => declared_parameter(a, a.expr(e).span, *name).is_some_and(|p| p.final_),
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) | Some(Resolved::Element(Element::Variable(v))) => final_variable(program, *v),
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) => final_function(program, *f),
            Some(Resolved::ExtensionMember { member, .. }) => final_function(program, *member),
            _ => true,
        }
    };
    // O identificador `p` que nomeia um prefixo de importação adiada.
    let deferred_prefix = |e: ExprId| -> bool {
        let ExprKind::Identifier(n) = &a.expr(e).kind else { return false };
        if matches!(
            body.get_resolved(e),
            Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) | Some(Resolved::Member { .. }) | Some(Resolved::ExtensionMember { .. })
        ) || matches!(body.get_resolved(e), Some(Resolved::Element(x)) if !matches!(x, Element::Prefix(..)))
        {
            return false;
        }
        library.imports.iter().any(|i| i.unit == u && i.prefix == Some(n.sym) && i.deferred)
    };
    // `mightBeDeferred`: o identificador simples ou o prefixo de `p.x`.
    let can_be_deferred = |e: Option<ExprId>| -> bool {
        let Some(e) = e else { return false };
        match &a.expr(e).kind {
            ExprKind::Identifier(_) => deferred_prefix(e),
            ExprKind::Property { target, null_aware: false, .. } => deferred_prefix(*target),
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
        let (inv, expression_body) = match &f.body {
            FunctionBody::Block(b) => match &a.stmt(*b).kind {
                StmtKind::Block(the_cmds) if the_cmds.len() == 1 => match &a.stmt(the_cmds[0]).kind {
                    StmtKind::Expression(x) | StmtKind::Return(Some(x)) => (*x, false),
                    _ => continue,
                },
                _ => continue,
            },
            FunctionBody::Expression(x) => (*x, true),
            _ => continue,
        };
        // A criação de instância (`C(e)`, `new C(e)`, `p.C.n(e)`).
        let creation: Option<(ExprId, &ast::Arguments, bool)> = match &a.expr(inv).kind {
            ExprKind::InstanceCreation { keyword, ty, arguments, .. } => {
                let deferred = match &a.ty(*ty).kind {
                    TypeKind::Named { name, .. } if name.len() == 2 => library.imports.iter().any(|i| i.unit == u && i.prefix == Some(name[0].sym) && i.deferred),
                    _ => false,
                };
                let constant = matches!(keyword, Some(ast::CreationKeyword::Const));
                Some((inv, &**arguments, deferred || constant))
            }
            ExprKind::Call { target, arguments } if matches!(body.get_resolved(inv), Some(Resolved::Constructor(_))) => {
                // O identificador mais à esquerda do nome do construtor.
                let mut x = *target;
                let mut parts = 0;
                while let ExprKind::Property { target: t, .. } = &a.expr(x).kind {
                    x = *t;
                    parts += 1;
                }
                let deferred = parts > 0 && deferred_prefix(x);
                Some((inv, &**arguments, deferred))
            }
            _ => None,
        };
        if let Some((created, arguments, skips)) = creation {
            // Só no corpo de expressão, com tear-offs de construtor.
            if !expression_body || !tearoffs || skips {
                continue;
            }
            let (Some(closure_type), Some(Resolved::Constructor(kf))) = (body.get_type(id), body.get_resolved(created)) else { continue };
            let Some(data) = outline.functions.get(kf.0 as usize) else { continue };
            let mut constructor_type = data.signature;
            // O `ConstructorMember` instanciado pelos argumentos do tipo criado.
            if let Some(cls) = program.function(*kf).class
                && let Some(the_created) = body.get_type(created)
                && let Type::Interface { args, .. } = table.get(the_created).clone()
            {
                let params = outline.classes.get(cls.0 as usize).map(|x| x.type_params.clone()).unwrap_or_default();
                let map: std::collections::HashMap<crate::table::TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
                constructor_type = crate::ops::substitute(constructor_type, &map, table);
            }
            let assignable = {
                let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                is_subtype(constructor_type, closure_type, &mut env)
            };
            if !assignable {
                continue;
            }
            if arguments.args.iter().any(|x| x.name.is_some() || !matches!(a.expr(x.value).kind, ExprKind::Identifier(_))) {
                continue;
            }
            if ps.len() != arguments.args.len() {
                continue;
            }
            let equal = arguments.args.iter().zip(ps.iter()).all(|(x, p)| matches!(&a.expr(x.value).kind, ExprKind::Identifier(n) if Some(n.sym) == p.name.map(|q| q.sym)));
            if equal {
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
        let closure_parameter = |x: ExprId| -> Option<dartforge_intern::SymbolId> {
            let ExprKind::Identifier(n) = &a.expr(x).kind else { return None };
            let p = ps.iter().find(|p| p.name.is_some_and(|q| q.sym == n.sym))?;
            match body.get_resolved(x) {
                Some(Resolved::Parameter { .. }) => Some(n.sym),
                Some(Resolved::Local(_)) if body.declaracao_local(x) == p.name.map(|q| q.span.start) => Some(n.sym),
                _ => None,
            }
        };
        let argument_element = |x: ExprId| -> Option<dartforge_intern::SymbolId> { closure_parameter(without_parentheses(x)) };
        let positional_args: Vec<dartforge_intern::SymbolId> = ps.iter().filter(|p| p.kind != ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
        let named_args: Vec<dartforge_intern::SymbolId> = ps.iter().filter(|p| p.kind == ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
        let mut args_pos = Vec::new();
        let mut the_named_args: Vec<(dartforge_intern::SymbolId, Option<dartforge_intern::SymbolId>)> = Vec::new();
        let mut the_matches = true;
        for x in arguments.args.iter() {
            let el = argument_element(x.value);
            match x.name {
                Some(n) => the_named_args.push((n.sym, el)),
                None => match el {
                    Some(el) => args_pos.push(el),
                    None => {
                        the_matches = false;
                        break;
                    }
                },
            }
        }
        if !the_matches || positional_args != args_pos || named_args.len() != the_named_args.len() {
            continue;
        }
        if !named_args.iter().all(|n| the_named_args.iter().any(|(r, el)| r == n && *el == Some(*n))) {
            continue;
        }
        let params: HashSet<dartforge_intern::SymbolId> = ps.iter().filter_map(|p| p.name.map(|n| n.sym)).collect();
        // O identificador que referencia um parâmetro da closure.
        let is_parameter = |x: ExprId| -> bool {
            matches!(&a.expr(x).kind, ExprKind::Identifier(n) if params.contains(&n.sym))
                && closure_parameter(x).is_some()
                && a.expr(x).span.start >= f.span.start
                && a.expr(x).span.end <= f.span.end
        };
        // `_FinalExpressionChecker.isFinalNode`.
        fn final_no(a: &ast::Ast, is_parameter: &dyn Fn(ExprId) -> bool, final_element: &dyn Fn(ExprId) -> bool, e: Option<ExprId>) -> bool {
            let Some(mut e) = e else { return true };
            while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
                e = *x;
            }
            match &a.expr(e).kind {
                ExprKind::FunctionExpression(g) => {
                    let sp = a.function(*g).span;
                    !a.exprs.iter().enumerate().any(|(k, x)| x.span.start >= sp.start && x.span.end <= sp.end && is_parameter(ExprId(k as u32)))
                }
                // `PrefixedIdentifier` e `PropertyAccess`: o alvo e o nome.
                ExprKind::Property { target, .. } => final_no(a, is_parameter, final_element, Some(*target)) && final_element(e),
                ExprKind::Identifier(_) => !is_parameter(e) && final_element(e),
                _ => false,
            }
        }
        if !crate::lints_tipados4::e_invocacao_de_metodo(program, a, body, *target) {
            // `FunctionExpressionInvocation`.
            if can_be_deferred(Some(*target)) {
                continue;
            }
            if final_no(a, &is_parameter, &final_element, Some(*target)) {
                out.push((e.span, "unnecessary_lambdas", Vec::new()));
            }
            continue;
        }
        // `MethodInvocation`.
        let (tgt, null_aware) = match &a.expr(*target).kind {
            ExprKind::Property { target: t, null_aware, .. } => (Some(*t), *null_aware),
            ExprKind::Identifier(_) => (None, false),
            _ => continue,
        };
        if can_be_deferred(tgt) {
            continue;
        }
        // `staticInvokeType`: o tipo do membro, instanciado pelos argumentos
        // de tipo da chamada.
        let Some(mut tearoff) = body.get_type(*target) else { continue };
        if let Some(inst) = body.instanciacao(arguments.span.start).map(|x| x.to_vec())
            && let Type::Function { type_params, .. } = table.get(tearoff)
            && type_params.len() == inst.len()
        {
            let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
            tearoff = crate::constraints::instantiate_function(tearoff, &inst, &mut env);
        }
        // O pai da closure: o argumento nomeado (o tipo estático é o da
        // própria closure) ou a declaração de variável.
        let named_parent = a.exprs.iter().any(|x| match &x.kind {
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => arguments.args.iter().any(|y| y.value == id && y.name.is_some()),
            _ => false,
        });
        let destination: Option<Option<TypeId>> = if named_parent {
            Some(body.get_type(id))
        } else {
            let mut d = None;
            for s in a.stmts.iter() {
                if let StmtKind::Variables(l) | StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } = &s.kind
                    && let Some(v) = l.variables.iter().find(|v| v.initializer == Some(id))
                {
                    d = Some(body.tipo_local(v.name.span.start));
                }
            }
            if d.is_none() {
                let variable_type = |r: VariableRef| {
                    program.variables.iter().position(|x| x.node == r).and_then(|i| outline.variables.get(i)).and_then(|x| x.declared_type.or(x.inferred))
                };
                for (mi, m) in a.members.iter().enumerate() {
                    if let MemberKind::Field(l) = &m.kind
                        && let Some(index) = l.variables.iter().position(|v| v.initializer == Some(id))
                    {
                        d = Some(variable_type(VariableRef::Field { unit: u, member: ast::MemberId(mi as u32), index }));
                    }
                }
                for (di, x) in a.decls.iter().enumerate() {
                    if let ast::DeclKind::Variables(l) = &x.kind
                        && let Some(index) = l.variables.iter().position(|v| v.initializer == Some(id))
                    {
                        d = Some(variable_type(VariableRef::TopLevel { unit: u, decl: ast::DeclId(di as u32), index }));
                    }
                }
            }
            d
        };
        if let Some(dt) = destination {
            let Some(dt) = dt else { continue };
            let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
            if !is_subtype(tearoff, dt, &mut env) {
                continue;
            }
        }
        // `containsNullAwareInvocationInChain`.
        let mut null_chain = null_aware;
        let mut x = tgt;
        while let Some(y) = x {
            match &a.expr(y).kind {
                ExprKind::Property { target, null_aware, .. } => {
                    null_chain |= *null_aware;
                    x = Some(*target);
                }
                ExprKind::Call { target, .. } if crate::lints_tipados4::e_invocacao_de_metodo(program, a, body, *target) => match &a.expr(*target).kind {
                    ExprKind::Property { target: t, null_aware, .. } => {
                        null_chain |= *null_aware;
                        x = Some(*t);
                    }
                    _ => break,
                },
                ExprKind::Index { target, null_aware, .. } => {
                    null_chain |= *null_aware;
                    x = Some(*target);
                }
                _ => break,
            }
        }
        if !null_chain && final_no(a, &is_parameter, &final_element, tgt) && final_element(*target) && arguments.type_args.is_empty() {
            out.push((e.span, "unnecessary_lambdas", Vec::new()));
        }
    }
}
