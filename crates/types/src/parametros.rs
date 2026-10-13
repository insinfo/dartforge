//! Parâmetros opcionais que nenhuma chamada da biblioteca preenche
//! (`UNUSED_ELEMENT_PARAMETER`, código `unused_element` no 3.6): a parte de
//! parâmetros do `UnusedLocalElementsVerifier` do analyzer 6.11
//! (`an611:src/error/unused_local_elements_verifier.dart`):
//!
//! * `visitFormalParameterList` (`:600-609`) relata cada parâmetro que não
//!   é usado;
//! * `_isUsedElement` (`:860-923`): só parâmetros **opcionais** de
//!   construtores, funções e métodos; fora os de construtor de classe
//!   genérica e os de executável genérico; fora os de executável acessível
//!   de fora da biblioteca (`_isPubliclyAccessible`, `:799-823`); fora o que
//!   corresponde a um obrigatório do construtor da superclasse ou de um
//!   membro sobrescrito, ou a um parâmetro usado do sobrescrito
//!   (`_overridesUsedParameter`, `:970-992`);
//! * "usado" é receber argumento em alguma chamada da biblioteca
//!   (`_addParametersForArguments`, `:360-365`: o `staticParameterElement`
//!   de cada argumento), ou ser o alvo de um `super.x` (`:116-118`).
//!
//! A ligação argumento → parâmetro vem das resoluções que a inferência de
//! corpos guarda (`crate::resolved`).

use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId,
};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, MemberKind, ParameterKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashSet;

/// Os parâmetros formais escritos de `f` (`None` sem nó).
fn parametros<'p>(program: &'p Program, f: FunctionElementId) -> Option<(UnitId, &'p [ast::Parameter])> {
    match program.function(f).node {
        FunctionRef::Function { unit, function } => {
            program.unit(unit).ast.function(function).parameters.as_deref().map(|p| (unit, p))
        }
        FunctionRef::Constructor { unit, member } => match &program.unit(unit).ast.member(member).kind {
            MemberKind::Constructor(k) => Some((unit, &k.parameters[..])),
            _ => None,
        },
        FunctionRef::None => None,
    }
}

fn nome_externo(p: &ast::Parameter) -> Option<SymbolId> {
    p.public_name.or(p.name).map(|n| n.sym)
}

/// O parâmetro de `ps` que recebe o argumento `i` de `args`.
fn argument_target(ps: &[ast::Parameter], args: &ast::Arguments, i: usize) -> Option<usize> {
    let a = &args.args[i];
    match a.name {
        Some(n) => ps.iter().position(|p| p.kind == ParameterKind::Named && nome_externo(p) == Some(n.sym)),
        None => {
            let pos = args.args[..i].iter().filter(|x| x.name.is_none()).count();
            ps.iter().enumerate().filter(|(_, p)| p.kind != ParameterKind::Named).nth(pos).map(|(j, _)| j)
        }
    }
}

/// `_getCorrespondingParameter`: pelo nome (nomeado) ou pela posição na
/// lista.
fn corresponding(ps: &[ast::Parameter], j: usize, others: &[ast::Parameter]) -> Option<usize> {
    let p = &ps[j];
    if p.kind == ParameterKind::Named {
        others.iter().position(|o| o.kind == ParameterKind::Named && nome_externo(o) == nome_externo(p))
    } else if j < others.len() {
        Some(j)
    } else {
        None
    }
}

fn obrigatorio(p: &ast::Parameter) -> bool {
    p.kind == ParameterKind::Required || (p.kind == ParameterKind::Named && p.required)
}

fn resolved_function(r: Option<&Resolved>) -> Option<FunctionElementId> {
    match r? {
        Resolved::Element(Element::Function(f)) => Some(*f),
        Resolved::Member { member: MemberRef::Function(f), .. } => Some(*f),
        Resolved::ExtensionMember { member, .. } => Some(*member),
        Resolved::Constructor(f) => Some(*f),
        _ => None,
    }
}

/// O construtor `nome` (ou o sem nome) da classe `c`.
fn construtor(program: &Program, interner: &Interner, c: ClassId, the_name: Option<ast::Name>) -> Option<FunctionElementId> {
    let s = match the_name {
        Some(n) => n.sym,
        None => interner.lookup("")?,
    };
    program.class(c).constructors.get(&s).copied()
}

/// O construtor da superclasse que `k` (da classe `c`) invoca, explícito
/// (`super.nome(...)`) ou implícito (o sem nome).
fn superclass_constructor(program: &Program, interner: &Interner, c: ClassId, k: &ast::Constructor) -> Option<FunctionElementId> {
    if k.factory || k.initializers.iter().any(|i| matches!(i, ast::Initializer::Redirect { .. })) {
        return None;
    }
    let mut sup = program.class(c).supertype_class?;
    // As aplicações sintéticas de mixin repassam os construtores da superclasse.
    let mut guard = 0;
    while program.class(sup).decl.is_none() && guard < 50 {
        sup = program.class(sup).supertype_class?;
        guard += 1;
    }
    let the_name = k.initializers.iter().find_map(|i| match i {
        ast::Initializer::Super { constructor, .. } => Some(*constructor),
        _ => None,
    });
    construtor(program, interner, sup, the_name.flatten())
}

/// Os diagnósticos de `lib`.
pub fn parametros_nao_usados(
    program: &Program,
    interner: &Interner,
    outline: &OutlineTypes,
    body: &BodyTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut used: HashSet<(FunctionElementId, usize)> = HashSet::new();
    let mark = |used: &mut HashSet<(FunctionElementId, usize)>, f: FunctionElementId, args: &ast::Arguments| {
        if let Some((_, ps)) = parametros(program, f) {
            for i in 0..args.args.len() {
                if let Some(j) = argument_target(ps, args, i) {
                    used.insert((f, j));
                }
            }
        }
    };
    // Executáveis cujos parâmetros contam todos como usados: referência que
    // não é chamada (tear-off; `visitSimpleIdentifier`, `:290-305`), e o nome
    // de `this.nome(...)`/`super.nome(...)` (o identificador não está numa
    // `MethodInvocation`).
    let mut all: HashSet<FunctionElementId> = HashSet::new();
    // Nomes lidos por identificadores, por unidade (`_useIdentifierElement`:
    // ler o parâmetro o usa), com o offset.
    let mut reads: Vec<(UnitId, SymbolId, usize)> = Vec::new();
    let mut shorthands: HashSet<SymbolId> = HashSet::new();
    let units = program.library(lib).units.clone();
    for &u in &units {
        let a = &program.unit(u).ast;
        let Some(b) = body.units.get(u.0 as usize) else { continue };
        // Alvos de chamada (o nome chamado e o `Property` do método).
        let mut targets: HashSet<ast::ExprId> = HashSet::new();
        for e in a.exprs.iter() {
            if let ExprKind::Call { target, .. } = &e.kind {
                targets.insert(*target);
                if let ExprKind::Property { .. } = &a.expr(*target).kind {
                    targets.insert(*target);
                }
            }
        }
        for (i, e) in a.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            match &e.kind {
                ExprKind::Call { target, arguments } => {
                    let f = resolved_function(b.get_resolved(id)).or_else(|| resolved_function(b.get_resolved(*target)));
                    if let Some(f) = f {
                        mark(&mut used, f, arguments);
                    }
                }
                ExprKind::InstanceCreation { arguments, .. } => {
                    if let Some(f) = resolved_function(b.get_resolved(id)) {
                        mark(&mut used, f, arguments);
                    }
                }
                ExprKind::Identifier(n) => {
                    reads.push((u, n.sym, n.span.start));
                    if !targets.contains(&id) {
                        if let Some(f) = resolved_function(b.get_resolved(id)) {
                            all.insert(f);
                        }
                    }
                }
                ExprKind::Property { name, .. } => {
                    if !targets.contains(&id) {
                        if let Some(f) = resolved_function(b.get_resolved(id)) {
                            all.insert(f);
                        }
                        // `A._named` (tear-off de construtor): pelo nome.
                        reads.push((u, name.sym, usize::MAX));
                    }
                }
                ExprKind::DotShorthand { name, .. } => {
                    shorthands.insert(name.sym);
                }
                _ => {}
            }
        }
    }
    // Argumentos de anotações (`@A(valor: 1)`): o rótulo e a posição usam o
    // parâmetro do construtor.
    for &u in &units {
        let a = &program.unit(u).ast;
        let mut annotations: Vec<&ast::Annotation> = Vec::new();
        for d in a.decls.iter() {
            annotations.extend(d.metadata.iter());
        }
        for m in a.members.iter() {
            annotations.extend(m.metadata.iter());
        }
        for an in annotations {
            let Some(args) = &an.arguments else { continue };
            let (class, ctor) = match &an.name[..] {
                [c] => (program.lookup_na_unidade(u, c.sym), None),
                [c, n] => match program.lookup_na_unidade(u, c.sym) {
                    Some(b) if matches!(b.getter, Some(Element::Class(_))) => (Some(b), Some(*n)),
                    _ => (program.lookup_prefixed_na_unidade(u, c.sym, n.sym), None),
                },
                [p, c, n] => (program.lookup_prefixed_na_unidade(u, p.sym, c.sym), Some(*n)),
                _ => (None, None),
            };
            if let Some(Element::Class(c)) = class.and_then(|b| b.getter) {
                if let Some(f) = construtor(program, interner, c, ctor) {
                    mark(&mut used, f, args);
                }
            }
        }
    }
    // Inicializadores (`super(...)`, `this(...)`), `super.x` e constantes de enum.
    for (ci, k) in program.classes.iter().enumerate() {
        let c = ClassId(ci as u32);
        if k.library != lib {
            continue;
        }
        for (_, &f) in k.constructors.iter() {
            let FunctionRef::Constructor { unit, member } = program.function(f).node else { continue };
            let MemberKind::Constructor(kk) = &program.unit(unit).ast.member(member).kind else { continue };
            for start in kk.initializers.iter() {
                match start {
                    ast::Initializer::Super { constructor: n, arguments, .. } => {
                        if let Some(s) = superclass_constructor(program, interner, c, kk) {
                            mark(&mut used, s, arguments);
                            if n.is_some() {
                                all.insert(s);
                            }
                        }
                    }
                    ast::Initializer::Redirect { constructor: n, arguments, .. } => {
                        if let Some(s) = construtor(program, interner, c, *n) {
                            mark(&mut used, s, arguments);
                            if n.is_some() {
                                all.insert(s);
                            }
                        }
                    }
                    _ => {}
                }
            }
            // Fábrica redirecionadora (`factory C() = D._;`): os parâmetros
            // correspondentes do alvo contam como usados
            // (`visitConstructorDeclaration`, `_matchParameters`, `:93-110`).
            if let Some(rd) = &kk.redirect {
                let ast_u = &program.unit(unit).ast;
                if let ast::TypeKind::Named { name, .. } = &ast_u.ty(rd.ty).kind {
                    let tgt = match (&name[..], rd.constructor) {
                        ([cn], ctor) => (program.lookup_na_unidade(unit, cn.sym), ctor),
                        ([a, b], None) => match program.lookup_na_unidade(unit, a.sym) {
                            Some(x) if matches!(x.getter, Some(Element::Class(_))) => (Some(x), Some(*b)),
                            _ => (program.lookup_prefixed_na_unidade(unit, a.sym, b.sym), None),
                        },
                        ([p, cn], ctor) => (program.lookup_prefixed_na_unidade(unit, p.sym, cn.sym), ctor),
                        ([p, cn, n], None) => (program.lookup_prefixed_na_unidade(unit, p.sym, cn.sym), Some(*n)),
                        _ => (None, None),
                    };
                    if let (Some(Element::Class(dc)), ctor) = (tgt.0.and_then(|b| b.getter), tgt.1) {
                        if let Some(t) = construtor(program, interner, dc, ctor) {
                            if let Some((_, tps)) = parametros(program, t) {
                                for (j, _) in kk.parameters.iter().enumerate() {
                                    if let Some(k2) = corresponding(&kk.parameters, j, tps) {
                                        used.insert((t, k2));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if kk.parameters.iter().any(|p| p.super_) {
                if let Some(s) = superclass_constructor(program, interner, c, kk) {
                    if let Some((_, sps)) = parametros(program, s) {
                        let mut pos = 0usize;
                        for p in kk.parameters.iter() {
                            if p.kind != ParameterKind::Named && p.super_ {
                                if let Some((j, _)) = sps.iter().enumerate().filter(|(_, x)| x.kind != ParameterKind::Named).nth(pos) {
                                    used.insert((s, j));
                                }
                            } else if p.super_ {
                                if let Some(j) = sps.iter().position(|x| x.kind == ParameterKind::Named && nome_externo(x) == nome_externo(p)) {
                                    used.insert((s, j));
                                }
                            }
                            if p.kind != ParameterKind::Named && p.super_ {
                                pos += 1;
                            }
                        }
                    }
                }
            }
        }
        if k.kind == ClassKind::Enum {
            if let Some(d) = k.decl {
                if let DeclKind::Enum(en) = &program.unit(d.unit).ast.decl(d.decl).kind {
                    for constant in en.constants.iter() {
                        if let (Some(args), Some(f)) = (&constant.arguments, construtor(program, interner, c, constant.constructor)) {
                            mark(&mut used, f, args);
                        }
                    }
                }
            }
        }
    }

    let mut output = Vec::new();
    for (fi, fe) in program.functions.iter().enumerate() {
        let f = FunctionElementId(fi as u32);
        if fe.library != lib {
            continue;
        }
        let constructor_ = fe.kind == FunctionKind::Constructor;
        if !(constructor_ || fe.kind == FunctionKind::Function || fe.kind == FunctionKind::Operator) {
            continue;
        }
        let Some((unit, ps)) = parametros(program, f) else { continue };
        if all.contains(&f) || shorthands.contains(&fe.name) {
            continue;
        }
        // O nome do construtor nomeado lido como propriedade (tear-off).
        if constructor_ && reads.iter().any(|&(_, n, o)| o == usize::MAX && n == fe.name) {
            continue;
        }
        let region = executable_region(program, f);
        if !ps.iter().any(|p| p.kind != ParameterKind::Required && !p.required) {
            continue;
        }
        // Executável genérico, ou construtor de classe genérica.
        if !outline.functions[fi].type_params.is_empty() {
            continue;
        }
        if constructor_ && fe.class.is_some_and(|c| !program.class(c).type_params.is_empty()) {
            continue;
        }
        if publicly_accessible(program, interner, f) {
            continue;
        }
        let superior = match (constructor_, fe.class, program.function(f).node) {
            (true, Some(c), FunctionRef::Constructor { unit: uu, member }) => match &program.unit(uu).ast.member(member).kind {
                MemberKind::Constructor(kk) => superclass_constructor(program, interner, c, kk),
                _ => None,
            },
            _ => None,
        };
        let overridden = if !constructor_ && !fe.static_ { sobrescritos(program, outline, f) } else { Vec::new() };
        for (j, p) in ps.iter().enumerate() {
            if obrigatorio(p) || p.kind == ParameterKind::Required {
                continue;
            }
            let Some(the_name) = p.name else { continue };
            if p.metadata.iter().any(|m| m.name.iter().any(|n| interner.resolve(n.sym) == "pragma")) {
                continue;
            }
            if used.contains(&(f, j)) {
                continue;
            }
            // Lido no corpo ou nos inicializadores.
            if let Some((ru, start, end)) = region {
                if reads.iter().any(|&(u, n, o)| u == ru && n == the_name.sym && o >= start && o < end && o != the_name.span.start) {
                    continue;
                }
            }
            if let Some(s) = superior {
                if let Some((_, sps)) = parametros(program, s) {
                    if corresponding(ps, j, sps).is_some_and(|k| obrigatorio(&sps[k])) {
                        continue;
                    }
                }
            }
            let mut the_used = false;
            for &o in &overridden {
                if let Some((_, ops)) = parametros(program, o) {
                    if let Some(k) = corresponding(ps, j, ops) {
                        if obrigatorio(&ops[k]) || used.contains(&(o, k)) {
                            the_used = true;
                            break;
                        }
                    }
                }
            }
            if the_used {
                continue;
            }
            let text = interner.resolve(the_name.sym).to_string();
            output.push((
                unit,
                Diagnostic::com_codigo(w::UNUSED_ELEMENT_PARAMETER, Span { start: the_name.span.start, end: the_name.span.end }, [text]),
            ));
        }
    }
    output
}

/// O intervalo da declaração do executável (onde as leituras dos
/// parâmetros contam).
fn executable_region(program: &Program, f: FunctionElementId) -> Option<(UnitId, usize, usize)> {
    match program.function(f).node {
        FunctionRef::Function { unit, function } => {
            let s = program.unit(unit).ast.function(function).span;
            Some((unit, s.start, s.end))
        }
        FunctionRef::Constructor { unit, member } => {
            let s = program.unit(unit).ast.member(member).span;
            Some((unit, s.start, s.end))
        }
        FunctionRef::None => None,
    }
}

/// `_isPubliclyAccessible`.
fn publicly_accessible(program: &Program, interner: &Interner, f: FunctionElementId) -> bool {
    let fe = program.function(f);
    let the_name = interner.resolve(fe.name);
    if the_name.starts_with('_') {
        return false;
    }
    if let Some(c) = fe.class {
        let k = program.class(c);
        if k.kind == ClassKind::Enum && fe.kind == FunctionKind::Constructor && !fe.factory {
            return false;
        }
        if interner.resolve(k.name).starts_with('_') && (fe.static_ || fe.kind == FunctionKind::Constructor) {
            return false;
        }
    }
    if let Some(x) = fe.extension {
        return program.extension(x).name.is_some_and(|n| !interner.resolve(n).starts_with('_'));
    }
    true
}

/// Os membros que o método de instância `f` sobrescreve (`getOverridden2`):
/// os de mesmo nome nos supertipos.
fn sobrescritos(program: &Program, outline: &OutlineTypes, f: FunctionElementId) -> Vec<FunctionElementId> {
    let fe = program.function(f);
    let Some(c) = fe.class else { return Vec::new() };
    let mut r = Vec::new();
    let Some(h) = outline.hierarchy.get(c) else { return r };
    for (&s, _) in h.supertypes.iter() {
        if s == c {
            continue;
        }
        if let Some(&o) = program.class(s).instance_members.get(&fe.name) {
            if o != f && !r.contains(&o) {
                r.push(o);
            }
        }
    }
    r
}
