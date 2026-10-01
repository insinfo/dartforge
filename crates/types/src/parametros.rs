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
fn alvo_do_argumento(ps: &[ast::Parameter], args: &ast::Arguments, i: usize) -> Option<usize> {
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
fn correspondente(ps: &[ast::Parameter], j: usize, outros: &[ast::Parameter]) -> Option<usize> {
    let p = &ps[j];
    if p.kind == ParameterKind::Named {
        outros.iter().position(|o| o.kind == ParameterKind::Named && nome_externo(o) == nome_externo(p))
    } else if j < outros.len() {
        Some(j)
    } else {
        None
    }
}

fn obrigatorio(p: &ast::Parameter) -> bool {
    p.kind == ParameterKind::Required || (p.kind == ParameterKind::Named && p.required)
}

fn funcao_resolvida(r: Option<&Resolved>) -> Option<FunctionElementId> {
    match r? {
        Resolved::Element(Element::Function(f)) => Some(*f),
        Resolved::Member { member: MemberRef::Function(f), .. } => Some(*f),
        Resolved::ExtensionMember { member, .. } => Some(*member),
        Resolved::Constructor(f) => Some(*f),
        _ => None,
    }
}

/// O construtor `nome` (ou o sem nome) da classe `c`.
fn construtor(program: &Program, interner: &Interner, c: ClassId, nome: Option<ast::Name>) -> Option<FunctionElementId> {
    let s = match nome {
        Some(n) => n.sym,
        None => interner.lookup("")?,
    };
    program.class(c).constructors.get(&s).copied()
}

/// O construtor da superclasse que `k` (da classe `c`) invoca, explícito
/// (`super.nome(...)`) ou implícito (o sem nome).
fn construtor_da_superclasse(program: &Program, interner: &Interner, c: ClassId, k: &ast::Constructor) -> Option<FunctionElementId> {
    if k.factory || k.initializers.iter().any(|i| matches!(i, ast::Initializer::Redirect { .. })) {
        return None;
    }
    let mut sup = program.class(c).supertype_class?;
    // As aplicações sintéticas de mixin repassam os construtores da superclasse.
    let mut guarda = 0;
    while program.class(sup).decl.is_none() && guarda < 50 {
        sup = program.class(sup).supertype_class?;
        guarda += 1;
    }
    let nome = k.initializers.iter().find_map(|i| match i {
        ast::Initializer::Super { constructor, .. } => Some(*constructor),
        _ => None,
    });
    construtor(program, interner, sup, nome.flatten())
}

/// Os diagnósticos de `lib`.
pub fn parametros_nao_usados(
    program: &Program,
    interner: &Interner,
    outline: &OutlineTypes,
    body: &BodyTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut usados: HashSet<(FunctionElementId, usize)> = HashSet::new();
    let marcar = |usados: &mut HashSet<(FunctionElementId, usize)>, f: FunctionElementId, args: &ast::Arguments| {
        if let Some((_, ps)) = parametros(program, f) {
            for i in 0..args.args.len() {
                if let Some(j) = alvo_do_argumento(ps, args, i) {
                    usados.insert((f, j));
                }
            }
        }
    };
    // Executáveis cujos parâmetros contam todos como usados: referência que
    // não é chamada (tear-off; `visitSimpleIdentifier`, `:290-305`), e o nome
    // de `this.nome(...)`/`super.nome(...)` (o identificador não está numa
    // `MethodInvocation`).
    let mut todos: HashSet<FunctionElementId> = HashSet::new();
    // Nomes lidos por identificadores, por unidade (`_useIdentifierElement`:
    // ler o parâmetro o usa), com o offset.
    let mut leituras: Vec<(UnitId, SymbolId, usize)> = Vec::new();
    let mut atalhos: HashSet<SymbolId> = HashSet::new();
    let unidades = program.library(lib).units.clone();
    for &u in &unidades {
        let a = &program.unit(u).ast;
        let Some(b) = body.units.get(u.0 as usize) else { continue };
        // Alvos de chamada (o nome chamado e o `Property` do método).
        let mut alvos: HashSet<ast::ExprId> = HashSet::new();
        for e in a.exprs.iter() {
            if let ExprKind::Call { target, .. } = &e.kind {
                alvos.insert(*target);
                if let ExprKind::Property { .. } = &a.expr(*target).kind {
                    alvos.insert(*target);
                }
            }
        }
        for (i, e) in a.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            match &e.kind {
                ExprKind::Call { target, arguments } => {
                    let f = funcao_resolvida(b.get_resolved(id)).or_else(|| funcao_resolvida(b.get_resolved(*target)));
                    if let Some(f) = f {
                        marcar(&mut usados, f, arguments);
                    }
                }
                ExprKind::InstanceCreation { arguments, .. } => {
                    if let Some(f) = funcao_resolvida(b.get_resolved(id)) {
                        marcar(&mut usados, f, arguments);
                    }
                }
                ExprKind::Identifier(n) => {
                    leituras.push((u, n.sym, n.span.start));
                    if !alvos.contains(&id) {
                        if let Some(f) = funcao_resolvida(b.get_resolved(id)) {
                            todos.insert(f);
                        }
                    }
                }
                ExprKind::Property { name, .. } => {
                    if !alvos.contains(&id) {
                        if let Some(f) = funcao_resolvida(b.get_resolved(id)) {
                            todos.insert(f);
                        }
                        // `A._named` (tear-off de construtor): pelo nome.
                        leituras.push((u, name.sym, usize::MAX));
                    }
                }
                ExprKind::DotShorthand { name, .. } => {
                    atalhos.insert(name.sym);
                }
                _ => {}
            }
        }
    }
    // Argumentos de anotações (`@A(valor: 1)`): o rótulo e a posição usam o
    // parâmetro do construtor.
    for &u in &unidades {
        let a = &program.unit(u).ast;
        let mut anotacoes: Vec<&ast::Annotation> = Vec::new();
        for d in a.decls.iter() {
            anotacoes.extend(d.metadata.iter());
        }
        for m in a.members.iter() {
            anotacoes.extend(m.metadata.iter());
        }
        for an in anotacoes {
            let Some(args) = &an.arguments else { continue };
            let (classe, ctor) = match &an.name[..] {
                [c] => (program.lookup_na_unidade(u, c.sym), None),
                [c, n] => match program.lookup_na_unidade(u, c.sym) {
                    Some(b) if matches!(b.getter, Some(Element::Class(_))) => (Some(b), Some(*n)),
                    _ => (program.lookup_prefixed_na_unidade(u, c.sym, n.sym), None),
                },
                [p, c, n] => (program.lookup_prefixed_na_unidade(u, p.sym, c.sym), Some(*n)),
                _ => (None, None),
            };
            if let Some(Element::Class(c)) = classe.and_then(|b| b.getter) {
                if let Some(f) = construtor(program, interner, c, ctor) {
                    marcar(&mut usados, f, args);
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
            for ini in kk.initializers.iter() {
                match ini {
                    ast::Initializer::Super { constructor: n, arguments, .. } => {
                        if let Some(s) = construtor_da_superclasse(program, interner, c, kk) {
                            marcar(&mut usados, s, arguments);
                            if n.is_some() {
                                todos.insert(s);
                            }
                        }
                    }
                    ast::Initializer::Redirect { constructor: n, arguments, .. } => {
                        if let Some(s) = construtor(program, interner, c, *n) {
                            marcar(&mut usados, s, arguments);
                            if n.is_some() {
                                todos.insert(s);
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
                    let alvo = match (&name[..], rd.constructor) {
                        ([cn], ctor) => (program.lookup_na_unidade(unit, cn.sym), ctor),
                        ([a, b], None) => match program.lookup_na_unidade(unit, a.sym) {
                            Some(x) if matches!(x.getter, Some(Element::Class(_))) => (Some(x), Some(*b)),
                            _ => (program.lookup_prefixed_na_unidade(unit, a.sym, b.sym), None),
                        },
                        ([p, cn], ctor) => (program.lookup_prefixed_na_unidade(unit, p.sym, cn.sym), ctor),
                        ([p, cn, n], None) => (program.lookup_prefixed_na_unidade(unit, p.sym, cn.sym), Some(*n)),
                        _ => (None, None),
                    };
                    if let (Some(Element::Class(dc)), ctor) = (alvo.0.and_then(|b| b.getter), alvo.1) {
                        if let Some(t) = construtor(program, interner, dc, ctor) {
                            if let Some((_, tps)) = parametros(program, t) {
                                for (j, _) in kk.parameters.iter().enumerate() {
                                    if let Some(k2) = correspondente(&kk.parameters, j, tps) {
                                        usados.insert((t, k2));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if kk.parameters.iter().any(|p| p.super_) {
                if let Some(s) = construtor_da_superclasse(program, interner, c, kk) {
                    if let Some((_, sps)) = parametros(program, s) {
                        let mut pos = 0usize;
                        for p in kk.parameters.iter() {
                            if p.kind != ParameterKind::Named && p.super_ {
                                if let Some((j, _)) = sps.iter().enumerate().filter(|(_, x)| x.kind != ParameterKind::Named).nth(pos) {
                                    usados.insert((s, j));
                                }
                            } else if p.super_ {
                                if let Some(j) = sps.iter().position(|x| x.kind == ParameterKind::Named && nome_externo(x) == nome_externo(p)) {
                                    usados.insert((s, j));
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
                    for constante in en.constants.iter() {
                        if let (Some(args), Some(f)) = (&constante.arguments, construtor(program, interner, c, constante.constructor)) {
                            marcar(&mut usados, f, args);
                        }
                    }
                }
            }
        }
    }

    let mut saida = Vec::new();
    for (fi, fe) in program.functions.iter().enumerate() {
        let f = FunctionElementId(fi as u32);
        if fe.library != lib {
            continue;
        }
        let construtor_ = fe.kind == FunctionKind::Constructor;
        if !(construtor_ || fe.kind == FunctionKind::Function || fe.kind == FunctionKind::Operator) {
            continue;
        }
        let Some((unit, ps)) = parametros(program, f) else { continue };
        if todos.contains(&f) || atalhos.contains(&fe.name) {
            continue;
        }
        // O nome do construtor nomeado lido como propriedade (tear-off).
        if construtor_ && leituras.iter().any(|&(_, n, o)| o == usize::MAX && n == fe.name) {
            continue;
        }
        let regiao = regiao_do_executavel(program, f);
        if !ps.iter().any(|p| p.kind != ParameterKind::Required && !p.required) {
            continue;
        }
        // Executável genérico, ou construtor de classe genérica.
        if !outline.functions[fi].type_params.is_empty() {
            continue;
        }
        if construtor_ && fe.class.is_some_and(|c| !program.class(c).type_params.is_empty()) {
            continue;
        }
        if publicamente_acessivel(program, interner, f) {
            continue;
        }
        let superior = match (construtor_, fe.class, program.function(f).node) {
            (true, Some(c), FunctionRef::Constructor { unit: uu, member }) => match &program.unit(uu).ast.member(member).kind {
                MemberKind::Constructor(kk) => construtor_da_superclasse(program, interner, c, kk),
                _ => None,
            },
            _ => None,
        };
        let sobrescritos = if !construtor_ && !fe.static_ { sobrescritos(program, outline, f) } else { Vec::new() };
        for (j, p) in ps.iter().enumerate() {
            if obrigatorio(p) || p.kind == ParameterKind::Required {
                continue;
            }
            let Some(nome) = p.name else { continue };
            if p.metadata.iter().any(|m| m.name.iter().any(|n| interner.resolve(n.sym) == "pragma")) {
                continue;
            }
            if usados.contains(&(f, j)) {
                continue;
            }
            // Lido no corpo ou nos inicializadores.
            if let Some((ru, ini, fim)) = regiao {
                if leituras.iter().any(|&(u, n, o)| u == ru && n == nome.sym && o >= ini && o < fim && o != nome.span.start) {
                    continue;
                }
            }
            if let Some(s) = superior {
                if let Some((_, sps)) = parametros(program, s) {
                    if correspondente(ps, j, sps).is_some_and(|k| obrigatorio(&sps[k])) {
                        continue;
                    }
                }
            }
            let mut usado = false;
            for &o in &sobrescritos {
                if let Some((_, ops)) = parametros(program, o) {
                    if let Some(k) = correspondente(ps, j, ops) {
                        if obrigatorio(&ops[k]) || usados.contains(&(o, k)) {
                            usado = true;
                            break;
                        }
                    }
                }
            }
            if usado {
                continue;
            }
            let texto = interner.resolve(nome.sym).to_string();
            saida.push((
                unit,
                Diagnostic::com_codigo(w::UNUSED_ELEMENT_PARAMETER, Span { start: nome.span.start, end: nome.span.end }, [texto]),
            ));
        }
    }
    saida
}

/// O intervalo da declaração do executável (onde as leituras dos
/// parâmetros contam).
fn regiao_do_executavel(program: &Program, f: FunctionElementId) -> Option<(UnitId, usize, usize)> {
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
fn publicamente_acessivel(program: &Program, interner: &Interner, f: FunctionElementId) -> bool {
    let fe = program.function(f);
    let nome = interner.resolve(fe.name);
    if nome.starts_with('_') {
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
