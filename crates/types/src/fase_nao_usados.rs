//! `UNUSED_ELEMENT` e `UNUSED_FIELD` das declarações de biblioteca: o
//! `GatherUsedLocalElementsVisitor` e o `UnusedLocalElementsVerifier` da
//! 3.6.2 (`analyzer/lib/src/error/unused_local_elements_verifier.dart`),
//! pelo elemento resolvido de cada referência (a parte local, variáveis e
//! funções locais, e a de parâmetros ficam em `analise::locais` e
//! `parametros`).
//!
//! Coleta (sobre todas as unidades da biblioteca, sem as referências de
//! comentário):
//! * o identificador (e o nome de propriedade) resolvido: a leitura de uma
//!   variável de topo pelo acessor sintético usa a variável; a de um acessor
//!   de topo explícito o põe nos membros e nos lidos (o setter lido conta o
//!   getter correspondente); qualquer outro elemento desta biblioteca é usado
//!   (`_useIdentifierElement`), salvo a própria classe (só de
//!   `ClassDeclaration`) e o próprio executável (função declarada ou
//!   método); o membro de classe ou de extensão vai para os membros e, lido,
//!   para os lidos; o `values` de um enum lê todas as constantes; o nome não
//!   resolvido e lido vai para os não resolvidos;
//! * "lido" é o `_isReadIdentifier`: em contexto de leitura (não o alvo de
//!   `=`), e, se o pai é o comando de expressão inteiro, nem o operando de um
//!   prefixo/pós-fixo nem o alvo de uma atribuição que não seja `??=` (o
//!   nome de propriedade tem a propriedade como pai, e só a primeira regra
//!   vale);
//! * o nome de tipo (`visitNamedType`) usa o elemento, salvo a própria classe
//!   e a interface no tipo de uma lista de variáveis que não é de campo ou de
//!   um `is`;
//! * o nome de anotação, o construtor de cada constante de enum, o
//!   construtor nomeado de uma criação, de um redirecionamento (`= C.n`,
//!   `this.n(…)`) e de um `super.n(…)`, e os construtores públicos de um
//!   tipo de interface que um `typedef` público nomeia.
//!
//! Relato: a classe, o mixin, o enum e o tipo de extensão (o `@JS` da
//! classe a usa), o typedef, a função e a variável de topo privados não
//! usados (`_isUsedElement`, com o `@pragma('vm:entry-point')`); o método,
//! o acessor e o construtor nomeado (de classe com mais de um) não
//! acessíveis de fora sem uso nem sobrescrita de membro usado
//! (`_isUsedMember`); o campo e a constante de enum não lidos
//! (`_isReadMember`: o público só conta como não lido quando é estático de
//! classe ou extensão privada).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::constantes::avaliador::{Constante, Ctx, Motor};
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved};
use crate::table::{CoreTypes, TypeTable};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, LibraryId, Program, TypedefId, UnitId, UnitRole, VariableId,
};
use dartforge_frontend::ast::{self, DeclKind, ExprId, ExprKind, ForInit, Initializer, MemberKind, StmtKind, TypeKind, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

/// Um elemento do verificador.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum El {
    Classe(ClassId),
    Extensao(ExtensionId),
    Typedef(TypedefId),
    Funcao(FunctionElementId),
    /// A variável de topo ou o campo.
    Variavel(VariableId),
    /// O acessor sintético de leitura de uma variável.
    Getter(VariableId),
    /// O acessor sintético de escrita de uma variável.
    Setter(VariableId),
}

#[derive(Default)]
struct Used {
    elementos: HashSet<El>,
    membros: HashSet<El>,
    read_values: HashSet<El>,
    unresolved: HashSet<SymbolId>,
}

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

fn privado(interner: &Interner, s: SymbolId) -> bool {
    interner.resolve(s).starts_with('_')
}

struct Contexto<'a> {
    program: &'a Program,
    interner: &'a Interner,
    lib: LibraryId,
}

impl Contexto<'_> {
    /// O elemento de um membro de função (o acessor sintético vale pela
    /// variável).
    fn of_function(&self, f: FunctionElementId) -> El {
        let g = self.program.function(f);
        match g.variable {
            Some(v) => {
                if self.program.variable(v).setter == Some(f) {
                    El::Setter(v)
                } else {
                    El::Getter(v)
                }
            }
            None => El::Funcao(f),
        }
    }

    fn of_element(&self, e: Element) -> Option<El> {
        Some(match e {
            Element::Class(c) => El::Classe(c),
            Element::Extension(x) => El::Extensao(x),
            Element::Typedef(t) => El::Typedef(t),
            Element::Function(f) => self.of_function(f),
            Element::Variable(v) => El::Variavel(v),
            Element::Prefix(..) => return None,
        })
    }

    /// A biblioteca do elemento.
    fn biblioteca(&self, e: El) -> LibraryId {
        let p = self.program;
        match e {
            El::Classe(c) => p.class(c).library,
            El::Extensao(x) => p.extension(x).library,
            El::Typedef(t) => p.typedefs[t.0 as usize].library,
            El::Funcao(f) => p.function(f).library,
            El::Variavel(v) | El::Getter(v) | El::Setter(v) => p.variable(v).library,
        }
    }

    /// O contêiner (classe ou extensão) e a chave do membro.
    fn member_key(&self, e: El) -> Option<(Option<ClassId>, Option<ExtensionId>, String)> {
        let p = self.program;
        match e {
            El::Funcao(f) => {
                let g = p.function(f);
                let mut the_name = self.interner.resolve(g.name).to_string();
                if g.kind == FunctionKind::Setter && !the_name.ends_with("_=") {
                    the_name.push_str("_=");
                }
                Some((g.class, g.extension, the_name))
            }
            El::Getter(v) => {
                let x = p.variable(v);
                Some((x.class, x.extension, self.interner.resolve(x.name).to_string()))
            }
            El::Setter(v) => {
                let x = p.variable(v);
                Some((x.class, x.extension, format!("{}_=", self.interner.resolve(x.name))))
            }
            _ => None,
        }
    }

    /// O getter correspondente a um setter (`correspondingGetter`).
    fn getter_de(&self, e: El) -> Option<El> {
        match e {
            El::Setter(v) => Some(El::Getter(v)),
            El::Funcao(f) if self.program.function(f).kind == FunctionKind::Setter => {
                let (cls, extension, the_name) = self.member_key(e)?;
                let base = the_name.strip_suffix("_=").unwrap_or(&the_name).to_string();
                let s = self.interner.lookup(&base)?;
                let tgt = match (cls, extension) {
                    (Some(c), _) => {
                        let k = self.program.class(c);
                        k.instance_members.get(&s).or_else(|| k.static_members.get(&s)).copied()
                    }
                    (None, Some(x)) => {
                        let k = self.program.extension(x);
                        k.instance_members.get(&s).or_else(|| k.static_members.get(&s)).copied()
                    }
                    (None, None) => match self.program.library(self.program.function(f).library).declared.get(&s)?.getter? {
                        Element::Function(g) => Some(g),
                        _ => None,
                    },
                };
                tgt.map(|g| self.of_function(g))
            }
            _ => None,
        }
    }

    /// `_isPubliclyAccessible` de um método, acessor ou construtor.
    fn accessible(&self, e: El, the_name: SymbolId) -> bool {
        if privado(self.interner, the_name) {
            return false;
        }
        let p = self.program;
        let (cls, extension, static_, ctor, factory) = match e {
            El::Funcao(f) => {
                let g = p.function(f);
                let ctor = matches!(g.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor);
                (g.class, g.extension, g.static_, ctor, g.factory)
            }
            El::Getter(v) | El::Setter(v) => {
                let x = p.variable(v);
                (x.class, x.extension, x.static_, false, false)
            }
            _ => return true,
        };
        if let Some(c) = cls {
            let k = p.class(c);
            if k.kind == ClassKind::Enum && ctor && !factory {
                return false;
            }
            if privado(self.interner, k.name) && (static_ || ctor) {
                return false;
            }
        }
        if let Some(x) = extension {
            return p.extension(x).name.is_some_and(|n| !privado(self.interner, n));
        }
        true
    }
}

/// `_overriddenElements`: o `getOverridden2` na classe do membro, pelo `Name`
/// da biblioteca verificada (`e.declaration` de cada um).
fn sobrescritos(cx: &Contexto<'_>, h: &mut crate::heranca::Heranca, p: &mut crate::heranca::ProvedorDoOutline<'_, '_>, e: El) -> Vec<El> {
    let Some((Some(c), _, text)) = cx.member_key(e) else { return Vec::new() };
    let Some(s) = cx.interner.lookup(&text) else { return Vec::new() };
    let the_name = crate::heranca::Nome::novo(cx.interner, cx.lib, s);
    h.sobrescritos(p, c, the_name).unwrap_or_default().into_iter().map(|m| cx.of_function(m.funcao)).collect()
}

/// `_overridesUsedElement`.
fn overrides_used(
    cx: &Contexto<'_>,
    h: &mut crate::heranca::Heranca,
    p: &mut crate::heranca::ProvedorDoOutline<'_, '_>,
    e: El,
    used: &Used,
    seen: &mut HashSet<El>,
) -> bool {
    if !seen.insert(e) {
        return false;
    }
    for o in sobrescritos(cx, h, p, e) {
        if used.membros.contains(&o) || overrides_used(cx, h, p, o, used, seen) {
            return true;
        }
    }
    false
}

/// Os tipos escritos numa lista de variáveis que não é de campo e no `is`,
/// com toda a subárvore (argumentos de tipo, partes de tipo de função e de
/// registro): neles o tipo de interface nomeado não conta como uso. O
/// `visitNamedType` passa o próprio `NamedType` como `parent`, então a regra
/// vale enquanto `_enclosingVariableDeclaration` (só durante o tipo da
/// lista, `visitVariableDeclarationList`) ou `_enclosingIsExpression` (só
/// durante o tipo do `is`) estiver posto (3.6.2,
/// `unused_local_elements_verifier.dart:189-198`, `:336-347`, `:386-400`).
fn types_without_interface_use(a: &ast::Ast) -> HashSet<ast::TypeId> {
    let roots = types_without_interface_use_roots(a);
    let mut v = HashSet::new();
    fn descend(a: &ast::Ast, t: ast::TypeId, v: &mut HashSet<ast::TypeId>) {
        if !v.insert(t) {
            return;
        }
        match &a.ty(t).kind {
            TypeKind::Named { args, .. } => {
                for &x in args.iter() {
                    descend(a, x, v);
                }
            }
            TypeKind::Void => {}
            TypeKind::Function { return_type, type_params, parameters } => {
                if let Some(r) = return_type {
                    descend(a, *r, v);
                }
                for tp in type_params.iter() {
                    if let Some(b) = tp.bound {
                        descend(a, b, v);
                    }
                }
                parametros(a, parameters, v);
            }
            TypeKind::Record { positional, named } => {
                for &x in positional.iter() {
                    descend(a, x, v);
                }
                for (_, x) in named.iter() {
                    descend(a, *x, v);
                }
            }
        }
    }
    fn parametros(a: &ast::Ast, ps: &[ast::Parameter], v: &mut HashSet<ast::TypeId>) {
        for p in ps.iter() {
            if let Some(x) = p.ty {
                descend(a, x, v);
            }
            for tp in p.function_type_params.iter() {
                if let Some(b) = tp.bound {
                    descend(a, b, v);
                }
            }
            if let Some(f) = &p.function_parameters {
                parametros(a, f, v);
            }
        }
    }
    for t in roots {
        descend(a, t, &mut v);
    }
    v
}

/// As raízes de [`tipos_sem_uso_de_interface`]: o tipo escrito da lista e o
/// do `is`.
fn types_without_interface_use_roots(a: &ast::Ast) -> HashSet<ast::TypeId> {
    let mut v = HashSet::new();
    fn lista(l: &ast::VariableList, v: &mut HashSet<ast::TypeId>) {
        if let Some(t) = l.ty {
            v.insert(t);
        }
    }
    fn of_collection(el: &ast::CollectionElement, v: &mut HashSet<ast::TypeId>) {
        match el {
            ast::CollectionElement::For { init, body, .. } => {
                if let Some(ForInit::Variables(l)) = init {
                    lista(l, v);
                }
                of_collection(body, v);
            }
            ast::CollectionElement::ForIn { body, .. } => of_collection(body, v),
            ast::CollectionElement::If { then, else_, .. } => {
                of_collection(then, v);
                if let Some(x) = else_ {
                    of_collection(x, v);
                }
            }
            _ => {}
        }
    }
    for d in a.decls.iter() {
        if let DeclKind::Variables(l) = &d.kind {
            lista(l, &mut v);
        }
    }
    for s in a.stmts.iter() {
        match &s.kind {
            StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => lista(l, &mut v),
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Is { ty, .. } => {
                v.insert(*ty);
            }
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    of_collection(el, &mut v);
                }
            }
            _ => {}
        }
    }
    v
}

/// O `@pragma('vm:entry-point')` (`isPragmaVmEntryPoint`: o construtor de
/// `pragma` do `dart.core` com o campo `name` igual a `vm:entry-point`) ou,
/// com `js`, o `@JS(…)` (o construtor de `JS` da biblioteca
/// `_js_annotations`) numa lista de anotações da unidade `u`.
fn marked(cx: &Contexto<'_>, engine: &mut Option<Motor<'_>>, u: UnitId, metadata: &[ast::Annotation], js: bool) -> bool {
    let p = cx.program;
    let library_name = |l: LibraryId| {
        p.library(l).name.as_ref().map(|n| n.iter().map(|s| cx.interner.resolve(*s)).collect::<Vec<_>>().join(".")).unwrap_or_default()
    };
    for m in metadata {
        let Some(args) = &m.arguments else { continue };
        let Some(Element::Class(c)) = crate::anotacoes::annotation_element(p, cx.interner, u, m) else { continue };
        let k = p.class(c);
        let the_name = cx.interner.resolve(k.name);
        if js && the_name == "JS" && library_name(k.library) == "_js_annotations" {
            return true;
        }
        if the_name == "pragma" && library_name(k.library) == "dart.core" {
            // O `name` do `pragma` é o primeiro argumento posicional.
            let Some(first) = args.args.iter().find(|x| x.name.is_none()) else { continue };
            let a = &p.unit(u).ast;
            let value = match &a.expr(first.value).kind {
                ExprKind::String(s) => s.constant_value().map(|t| t.to_string_lossy()),
                _ => engine.as_mut().and_then(|mm| {
                    let cxm = Ctx::simples(u, p.unit(u).library);
                    match mm.avaliar(&cxm, first.value, true) {
                        Constante::Valor(v) => match v.estado {
                            crate::constantes::valor::Estado::Str(Some(t)) => Some(String::from_utf16_lossy(&t)),
                            _ => None,
                        },
                        Constante::Invalid(_) => None,
                    }
                }),
            };
            if value.as_deref() == Some("vm:entry-point") {
                return true;
            }
        }
    }
    false
}

/// Os relatos da biblioteca `lib`.
#[allow(clippy::too_many_arguments)]
pub fn elementos_nao_usados(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let cx = Contexto { program, interner, lib };
    let mut used = Used::default();
    let units: Vec<UnitId> = program.library(lib).units.iter().copied().filter(|u| program.unit(*u).role != UnitRole::Patch).collect();
    let decl_class = |u: UnitId, d: ast::DeclId| {
        program.classes.iter().position(|c| c.decl.is_some_and(|r| r.unit == u && r.decl == d)).map(|i| ClassId(i as u32))
    };
    let node_function = |u: UnitId, f: ast::FunctionId| {
        program
            .functions
            .iter()
            .position(|e| matches!(e.node, dartforge_elements::model::FunctionRef::Function { unit, function } if unit == u && function == f))
            .map(|i| FunctionElementId(i as u32))
    };
    let member_constructor = |u: UnitId, m: ast::MemberId| {
        program
            .functions
            .iter()
            .position(|e| matches!(e.node, dartforge_elements::model::FunctionRef::Constructor { unit, member } if unit == u && member == m))
            .map(|i| FunctionElementId(i as u32))
    };

    for &u in &units {
        let a = &program.unit(u).ast;
        let Some(the_body) = bodies.units.get(u.0 as usize) else { continue };
        let pais = crate::lints_tipados::pais_da_unidade(program, u);
        // As classes (`ClassDeclaration`) e os executáveis (funções
        // declaradas e métodos) que contêm cada lugar.
        let classes: Vec<(Span, ClassId)> = a
            .decls
            .iter()
            .enumerate()
            .filter(|(_, d)| matches!(d.kind, DeclKind::Class(_)))
            .filter_map(|(i, d)| decl_class(u, ast::DeclId(i as u32)).map(|c| (d.span, c)))
            .collect();
        let mut executables: Vec<(Span, FunctionElementId)> = Vec::new();
        for d in a.decls.iter() {
            if let DeclKind::Function(f) = &d.kind
                && let Some(e) = node_function(u, *f)
            {
                executables.push((a.function(*f).span, e));
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Method(f) = &m.kind
                && let Some(e) = node_function(u, *f)
            {
                executables.push((a.function(*f).span, e));
            }
        }
        let class_in = |s: Span| classes.iter().filter(|(r, _)| dentro(s, *r)).min_by_key(|(r, _)| r.end - r.start).map(|x| x.1);
        let executable_in = |s: Span| executables.iter().filter(|(r, _)| dentro(s, *r)).min_by_key(|(r, _)| r.end - r.start).map(|x| x.1);
        // `_useIdentifierElement`.
        let use_it = |used: &mut Used, e: El, where_: Span| {
            if cx.biblioteca(e) != lib {
                return;
            }
            if let El::Classe(c) = e
                && class_in(where_) == Some(c)
            {
                return;
            }
            if let El::Funcao(f) = e
                && executable_in(where_) == Some(f)
            {
                return;
            }
            used.elementos.insert(e);
        };
        // `_addMemberAndCorrespondingGetter`.
        let read_member = |used: &mut Used, e: El| match cx.getter_de(e) {
            Some(g) => {
                used.membros.insert(g);
                used.read_values.insert(g);
            }
            None if matches!(e, El::Setter(_)) => {}
            None => {
                used.read_values.insert(e);
            }
        };
        // Os comandos de expressão.
        let commands: HashSet<ExprId> = a
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Expression(e) => Some(*e),
                _ => None,
            })
            .collect();
        let parent_of = |e: ExprId| match pais.pai(e) {
            dartforge_frontend::pais::Pai::Expr(p) => Some(p),
            _ => None,
        };

        // O executável de uma resolução de operador ou de `call` (o
        // `staticElement` do nó).
        let executable = |r: Option<&Resolved>| match r {
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::ExtensionMember { member: f, .. }) => {
                Some(cx.of_function(*f))
            }
            _ => None,
        };
        for (k, ex) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let (the_name, property) = match &ex.kind {
                ExprKind::Identifier(n) => (*n, false),
                ExprKind::Property { name, .. } => (*name, true),
                // `visitBinaryExpression`, `visitPrefixExpression`,
                // `visitPostfixExpression`, `visitIndexExpression` e
                // `visitAssignmentExpression`: o operador resolvido é membro
                // usado (`addMember`).
                ExprKind::Binary { .. } | ExprKind::Unary { .. } | ExprKind::Index { .. } | ExprKind::Assign { .. } => {
                    // O índice alvo de uma atribuição (`a[i] = v`, `a[i] ??= v`):
                    // o `writeOrReadElement` (3.6.2
                    // `unused_local_elements_verifier.dart:176-180`) é o `[]=`; o
                    // `[]` lido pela composta não conta.
                    let assignment_target = matches!(ex.kind, ExprKind::Index { .. })
                        && parent_of(id).is_some_and(|p| matches!(&a.expr(p).kind, ExprKind::Assign { target, .. } if *target == id));
                    let write = if assignment_target {
                        match the_body.get_resolved(id) {
                            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::ExtensionMember { member: f, .. }) => {
                                let g = program.function(*f);
                                let the_name = interner.lookup("[]=");
                                let sibling = the_name.and_then(|n| match (g.class, g.extension) {
                                    (Some(c), _) => program.class(c).instance_members.get(&n).copied(),
                                    (None, Some(x)) => program.extension(x).instance_members.get(&n).copied(),
                                    _ => None,
                                });
                                if interner.resolve(g.name) == "[]" { sibling } else { Some(*f) }
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };
                    match write {
                        Some(f) => {
                            used.membros.insert(cx.of_function(f));
                        }
                        None if !assignment_target => {
                            if let Some(el) = executable(the_body.get_resolved(id)) {
                                used.membros.insert(el);
                            }
                        }
                        None => {}
                    }
                    continue;
                }
                // `visitDotShorthandConstructorInvocation`,
                // `visitDotShorthandInvocation` e
                // `visitDotShorthandPropertyAccess` (3.13, checkout main
                // `unused_local_elements_verifier.dart:203-238`): a declaração
                // do contexto é usada (`addElement`, sem a regra da própria
                // classe); o membro estático pelo nome (`visitSimpleIdentifier`
                // do `memberName`/`propertyName`). O construtor fica na chamada.
                ExprKind::DotShorthand { name, .. } => {
                    if let Some(Resolved::Element(Element::Class(d))) = the_body.get_resolved(id) {
                        if cx.biblioteca(El::Classe(*d)) == lib {
                            used.elementos.insert(El::Classe(*d));
                        }
                        if let Some(&f) = program.class(*d).static_members.get(&name.sym) {
                            let el = cx.of_function(f);
                            use_it(&mut used, el, ex.span);
                            used.membros.insert(el);
                            read_member(&mut used, el);
                        } else if let Some(&v) = program.class(*d).enum_constants.iter().find(|&&v| program.variable(v).name == name.sym) {
                            // A constante de enum (`.v`) não tem getter no
                            // modelo: lida como a de `_E.v`.
                            let el = El::Getter(v);
                            use_it(&mut used, el, ex.span);
                            used.membros.insert(el);
                            read_member(&mut used, el);
                        }
                    }
                    continue;
                }
                // A criação: o construtor nomeado (o identificador dele; em
                // `const C.n()` o parser deixa `C.n` como tipo de duas partes,
                // sem `constructor`).
                ExprKind::InstanceCreation { .. } | ExprKind::Call { .. } => {
                    // `visitFunctionExpressionInvocation`: o `call` implícito
                    // (`addElement`).
                    if matches!(ex.kind, ExprKind::Call { .. })
                        && let Some(el) = executable(the_body.get_resolved(id))
                    {
                        used.elementos.insert(el);
                    }
                    if let Some(Resolved::Constructor(f)) = the_body.get_resolved(id) {
                        let written = match &ex.kind {
                            ExprKind::InstanceCreation { .. } => true,
                            ExprKind::Call { target, .. } => matches!(a.expr(*target).kind, ExprKind::Property { .. } | ExprKind::DotShorthand { .. }),
                            _ => false,
                        };
                        let named_one = !interner.resolve(program.function(*f).name).is_empty();
                        if written && named_one {
                            use_it(&mut used, El::Funcao(*f), ex.span);
                            used.membros.insert(El::Funcao(*f));
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            let parent = parent_of(id);
            // `inGetterContext` e `_isReadIdentifier`.
            let target_of = |p: ExprId| matches!(&a.expr(p).kind, ExprKind::Assign { target, .. } if *target == id);
            let simple_assignment = parent.is_some_and(|p| matches!(&a.expr(p).kind, ExprKind::Assign { op: ast::AssignOp::Assign, target, .. } if *target == id));
            let mut read_value = !simple_assignment;
            if read_value
                && !property
                && let Some(p) = parent
                && commands.contains(&p)
            {
                match &a.expr(p).kind {
                    ExprKind::Unary { operand, .. } if *operand == id => read_value = false,
                    ExprKind::Assign { op, target, .. } if *target == id => read_value = matches!(op, ast::AssignOp::Compound(ast::BinaryOp::IfNull)),
                    _ => {}
                }
            }
            let written = parent.is_some_and(|p| {
                target_of(p)
                    || matches!(&a.expr(p).kind, ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } if *operand == id)
            });
            match the_body.get_resolved(id) {
                None | Some(Resolved::Dynamic) => {
                    if read_value {
                        used.unresolved.insert(the_name.sym);
                    }
                }
                Some(Resolved::Local(_) | Resolved::Parameter { .. } | Resolved::TypeParameter(_) | Resolved::Prefix(_)) => {}
                Some(Resolved::Element(e)) => {
                    let Some(el) = cx.of_element(*e) else { continue };
                    match el {
                        // O campo estático de extensão (no modelo, lido pela
                        // variável): o getter sintético do campo é o membro
                        // lido (`_A.f1`, `_baz` dentro da extensão).
                        El::Variavel(v) | El::Getter(v) | El::Setter(v)
                            if program.variable(v).class.is_none() && program.variable(v).extension.is_some() =>
                        {
                            if read_value {
                                used.membros.insert(El::Getter(v));
                                read_member(&mut used, El::Getter(v));
                            }
                            if written {
                                use_it(&mut used, El::Setter(v), ex.span);
                            }
                        }
                        // A variável de topo (acessores sintéticos).
                        El::Variavel(v) | El::Getter(v) | El::Setter(v) if program.variable(v).class.is_none() => {
                            if read_value {
                                used.elementos.insert(El::Variavel(v));
                            }
                        }
                        // O acessor de topo explícito.
                        El::Funcao(f) if matches!(program.function(f).kind, FunctionKind::Getter | FunctionKind::Setter) => {
                            if read_value {
                                used.membros.insert(el);
                                read_member(&mut used, el);
                            }
                            if written {
                                // O `writeElement`: o setter de mesmo nome, ou
                                // o getter da recuperação (sem setter).
                                let setter = program.library(program.function(f).library).declared.get(&program.function(f).name).and_then(|b| b.setter);
                                match setter {
                                    Some(Element::Function(g)) => use_it(&mut used, El::Funcao(g), ex.span),
                                    _ => use_it(&mut used, el, ex.span),
                                }
                                // O `readElement` da composta e do `++`/`--`:
                                // o getter (`_useIdentifierElement`, 3.6.2
                                // `unused_local_elements_verifier.dart:284`).
                                if !simple_assignment {
                                    let getter = if program.function(f).kind == FunctionKind::Getter { Some(el) } else { cx.getter_de(el) };
                                    if let Some(g) = getter {
                                        use_it(&mut used, g, ex.span);
                                    }
                                }
                            }
                            if !read_value && !written {
                                use_it(&mut used, el, ex.span);
                            }
                        }
                        _ => use_it(&mut used, el, ex.span),
                    }
                }
                Some(Resolved::Member { class, member, .. }) => {
                    let (el_read, el_write) = match member {
                        MemberRef::Variable(v) => (El::Getter(*v), El::Setter(*v)),
                        MemberRef::Function(f) => {
                            let el = cx.of_function(*f);
                            match el {
                                El::Getter(v) | El::Setter(v) => (El::Getter(v), El::Setter(v)),
                                other => (other, other),
                            }
                        }
                    };
                    // `values` de enum: todas as constantes lidas.
                    let k = program.class(*class);
                    if k.kind == ClassKind::Enum && interner.resolve(the_name.sym) == "values" {
                        for &c in k.enum_constants.iter() {
                            used.read_values.insert(El::Getter(c));
                        }
                        continue;
                    }
                    let mut targets: Vec<El> = Vec::new();
                    if read_value || !written {
                        targets.push(el_read);
                    }
                    if written {
                        targets.push(el_write);
                        // O `readElement` da composta e do `++`/`--`.
                        if !simple_assignment && !(read_value || !written) {
                            targets.push(cx.getter_de(el_write).unwrap_or(el_read));
                        }
                    }
                    for el in targets {
                        use_it(&mut used, el, ex.span);
                        if matches!(el, El::Funcao(f) if executable_in(ex.span) == Some(f)) {
                            continue;
                        }
                        used.membros.insert(el);
                        if read_value {
                            read_member(&mut used, el);
                        }
                    }
                }
                Some(Resolved::ExtensionMember { member, .. }) => {
                    let el = cx.of_function(*member);
                    use_it(&mut used, el, ex.span);
                    if !matches!(el, El::Funcao(f) if executable_in(ex.span) == Some(f)) {
                        used.membros.insert(el);
                        if read_value {
                            read_member(&mut used, el);
                        }
                    }
                }
                Some(Resolved::Constructor(f)) => {
                    use_it(&mut used, El::Funcao(*f), ex.span);
                    used.membros.insert(El::Funcao(*f));
                }
            }
        }

        // O nome solto de um padrão refutável (`case _limite:`): o
        // `SimpleIdentifier` do `ConstantPattern`, lido.
        for (p, r) in the_body.pattern_names.iter() {
            let where_ = a.pattern(*p).span;
            match r {
                Resolved::Element(e) => {
                    let Some(el) = cx.of_element(*e) else { continue };
                    match el {
                        El::Variavel(v) | El::Getter(v) | El::Setter(v) if program.variable(v).class.is_none() && program.variable(v).extension.is_some() => {
                            used.membros.insert(El::Getter(v));
                            read_member(&mut used, El::Getter(v));
                        }
                        El::Variavel(v) | El::Getter(v) | El::Setter(v) if program.variable(v).class.is_none() => {
                            used.elementos.insert(El::Variavel(v));
                        }
                        El::Funcao(f) if matches!(program.function(f).kind, FunctionKind::Getter | FunctionKind::Setter) => {
                            used.membros.insert(el);
                            read_member(&mut used, el);
                        }
                        _ => use_it(&mut used, el, where_),
                    }
                }
                Resolved::Member { member, .. } => {
                    let el = match member {
                        MemberRef::Variable(v) => El::Getter(*v),
                        MemberRef::Function(f) => match cx.of_function(*f) {
                            El::Getter(v) | El::Setter(v) => El::Getter(v),
                            other => other,
                        },
                    };
                    use_it(&mut used, el, where_);
                    used.membros.insert(el);
                    read_member(&mut used, el);
                }
                Resolved::ExtensionMember { member, .. } => {
                    let el = cx.of_function(*member);
                    use_it(&mut used, el, where_);
                    used.membros.insert(el);
                    read_member(&mut used, el);
                }
                _ => {}
            }
        }

        // `visitPatternField`: `addMember` e `addReadMember` do elemento do
        // campo.
        for p in a.patterns.iter() {
            let ast::PatternKind::Object { fields, .. } = &p.kind else { continue };
            for f in fields.iter() {
                let el = match the_body.field_elements.get(&f.pattern) {
                    Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => El::Getter(*v),
                    Some(Resolved::Member { member: MemberRef::Function(g), .. }) | Some(Resolved::ExtensionMember { member: g, .. }) => cx.of_function(*g),
                    _ => continue,
                };
                used.membros.insert(el);
                read_member(&mut used, el);
            }
        }

        // `visitRelationalPattern` (`unused_local_elements_verifier.dart:247-251`):
        // `addMember` e `addReadMember` do operador.
        for el in the_body.relational_operators.values() {
            let el = match el {
                Resolved::Member { member: MemberRef::Function(g), .. } | Resolved::ExtensionMember { member: g, .. } => cx.of_function(*g),
                _ => continue,
            };
            used.membros.insert(el);
            read_member(&mut used, el);
        }

        // `visitNamedType`.
        let without_interface = types_without_interface_use(a);
        for (ti, t) in a.types.iter().enumerate() {
            let TypeKind::Named { name, .. } = &t.kind else { continue };
            let element = match &name[..] {
                [n] => {
                    if crate::anotacoes::sombreado(a, interner, n.span.start, n.sym) {
                        continue;
                    }
                    program.lookup_na_unidade(u, n.sym).and_then(|b| b.getter)
                }
                [p, n, ..] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym).and_then(|b| b.getter),
                [] => None,
            };
            let Some(el) = element.and_then(|e| cx.of_element(e)) else { continue };
            if matches!(el, El::Classe(_)) && without_interface.contains(&ast::TypeId(ti as u32)) {
                continue;
            }
            use_it(&mut used, el, t.span);
        }
        // Os nomes de anotação.
        for m in dartforge_frontend::pais::todas_as_anotacoes(a, &program.unit(u).unit) {
            if let Some(el) = crate::anotacoes::annotation_element(program, interner, u, m).and_then(|e| cx.of_element(e)) {
                use_it(&mut used, el, m.span);
            }
        }
        // Os construtores das constantes de enum, os de redirecionamento e
        // os de `super.n(…)`.
        for (di, d) in a.decls.iter().enumerate() {
            let DeclKind::Enum(x) = &d.kind else { continue };
            let Some(c) = decl_class(u, ast::DeclId(di as u32)) else { continue };
            for k in x.constants.iter() {
                let the_name = k.constructor.map(|n| n.sym).or_else(|| interner.lookup(""));
                if let Some(f) = the_name.and_then(|n| program.class(c).constructors.get(&n)) {
                    used.elementos.insert(El::Funcao(*f));
                }
            }
        }
        for (mi, m) in a.members.iter().enumerate() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            // O `returnType` do `ConstructorDeclaration` é um
            // `SimpleIdentifier` com o tipo dono: fora de uma
            // `ClassDeclaration` (o `_enclosingClass` só é posto nela), ele
            // usa o enum ou o tipo de extensão (3.6.2,
            // `unused_local_elements_verifier.dart:255-286`, `:380-382`). O
            // primário (3.13) não tem esse identificador.
            let primary = a.decls.iter().any(|d| match &d.kind {
                DeclKind::Class(x) => x.primary_constructor == Some(ast::MemberId(mi as u32)),
                DeclKind::Enum(x) => x.primary_constructor == Some(ast::MemberId(mi as u32)),
                _ => false,
            });
            if !primary
                && let Some(eu) = member_constructor(u, ast::MemberId(mi as u32))
                && let Some(cls) = program.function(eu).class
                && matches!(program.class(cls).kind, ClassKind::Enum | ClassKind::ExtensionType)
            {
                used.elementos.insert(El::Classe(cls));
            }
            let mut mark = |c: ClassId, n: Option<ast::Name>| {
                let Some(n) = n else { return };
                if let Some(&f) = program.class(c).constructors.get(&n.sym) {
                    use_it(&mut used, El::Funcao(f), m.span);
                    used.membros.insert(El::Funcao(f));
                }
            };
            // O redirecionamento é resolvido mesmo num construtor duplicado
            // (que também tem elemento no analyzer, e não no nosso modelo).
            if let Some(r) = &k.redirect
                && let Some((c, ctor)) = crate::redirection::class_and_constructor(program, u, r)
            {
                mark(c, ctor);
            }
            let Some(eu) = member_constructor(u, ast::MemberId(mi as u32)) else { continue };
            let Some(cls) = program.function(eu).class else { continue };
            for i in k.initializers.iter() {
                match i {
                    Initializer::Redirect { constructor, .. } => mark(cls, *constructor),
                    Initializer::Super { constructor, .. } => {
                        if let Some(s) = program.class(cls).supertype_class {
                            mark(s, *constructor);
                        }
                    }
                    _ => {}
                }
            }
        }
        // `visitGenericTypeAlias`: o typedef público de um tipo de interface
        // usa os construtores públicos dele.
        for d in a.decls.iter() {
            let DeclKind::Typedef(t) = &d.kind else { continue };
            let ast::TypedefKind::Alias(ty) = &t.kind else { continue };
            if privado(interner, t.name.sym) {
                continue;
            }
            let Some(tgt) = outline.tipos_escritos.get(&(u, *ty)) else { continue };
            if let crate::table::Type::Interface { class, .. } | crate::table::Type::ExtensionType { decl: class, .. } = table.get(*tgt) {
                for (n, f) in program.class(*class).constructors.iter() {
                    if !privado(interner, *n) {
                        used.elementos.insert(El::Funcao(*f));
                    }
                }
            }
        }
    }

    // `_overridesUsedElement` de cada membro de instância e construtor das
    // classes da biblioteca, pelo `InheritanceManager3`, antes do motor (que
    // fica com a tabela).
    let mut override_used: HashSet<El> = HashSet::new();
    {
        let mut inheritance = crate::heranca::Heranca::default();
        let mut prov = crate::heranca::ProvedorDoOutline { program, interner, core, outline, table: &mut *table };
        let mut candidates: Vec<El> = Vec::new();
        for (i, k) in program.classes.iter().enumerate() {
            if k.library != lib || k.decl.is_none() {
                continue;
            }
            let c = ClassId(i as u32);
            for &f in k.instance_members.values().chain(k.constructors.values()) {
                candidates.push(cx.of_function(f));
            }
            for &v in &k.fields {
                candidates.push(El::Getter(v));
            }
            let _ = c;
        }
        for el in candidates {
            if overrides_used(&cx, &mut inheritance, &mut prov, el, &used, &mut HashSet::new()) {
                override_used.insert(el);
            }
        }
    }
    // O relato.
    let mut engine: Option<Motor<'_>> = None;
    let needs_engine = units.iter().any(|&u| {
        dartforge_frontend::pais::todas_as_anotacoes(&program.unit(u).ast, &program.unit(u).unit)
            .iter()
            .any(|m| m.name.last().is_some_and(|n| interner.resolve(n.sym) == "pragma"))
    });
    if needs_engine {
        engine = Some(Motor::novo(program, interner, table, core, outline, bodies, the_inferred));
    }
    let mut output: Vec<(UnitId, Diagnostic)> = Vec::new();
    // Os campos declarados por parâmetro do construtor primário (3.13), com
    // dois argumentos: juntados a `saida` no fim.
    let mut of_primary: Vec<(UnitId, Diagnostic)> = Vec::new();
    let mut report = |u: UnitId, code, n: ast::Name, displayed: String| {
        output.push((u, Diagnostic::com_codigo(code, n.span, [displayed.as_str()])));
    };
    let used_element = |el: El, metadata: &[ast::Annotation], u: UnitId, engine: &mut Option<Motor<'_>>| {
        used.elementos.contains(&el) || marked(&cx, engine, u, metadata, false)
    };
    let used_member = |el: El, the_name: SymbolId, metadata: &[ast::Annotation], u: UnitId, engine: &mut Option<Motor<'_>>| {
        cx.accessible(el, the_name)
            || marked(&cx, engine, u, metadata, false)
            || used.membros.contains(&el)
            || used.elementos.contains(&el)
            || override_used.contains(&el)
    };
    // `_isReadMember` de um campo ou constante de enum.
    let read_value = |v: VariableId| {
        let x = program.variable(v);
        let static_ = x.static_ || x.const_ && x.class.is_some_and(|c| program.class(c).kind == ClassKind::Enum);
        let private_container = x.class.is_some_and(|c| privado(interner, program.class(c).name))
            || x.extension.is_some_and(|e| program.extension(e).name.is_none_or(|n| privado(interner, n)));
        if !privado(interner, x.name) && !(private_container && static_) {
            return true;
        }
        if used.read_values.contains(&El::Getter(v)) || used.unresolved.contains(&x.name) {
            return true;
        }
        if static_ {
            return false;
        }
        override_used.contains(&El::Getter(v))
    };
    let variables_of = |u: UnitId, decl: Option<ast::DeclId>, mem: Option<ast::MemberId>| -> HashMap<usize, VariableId> {
        let mut m = HashMap::new();
        for (i, x) in program.variables.iter().enumerate() {
            match x.node {
                dartforge_elements::model::VariableRef::TopLevel { unit, decl: d, index } if unit == u && Some(d) == decl => {
                    m.insert(index, VariableId(i as u32));
                }
                dartforge_elements::model::VariableRef::Field { unit, member, index } if unit == u && Some(member) == mem => {
                    m.insert(index, VariableId(i as u32));
                }
                dartforge_elements::model::VariableRef::EnumConstant { unit, decl: d, index } if unit == u && Some(d) == decl => {
                    m.insert(index, VariableId(i as u32));
                }
                _ => {}
            }
        }
        m
    };
    for &u in &units {
        let a = &program.unit(u).ast;
        for (di, d) in a.decls.iter().enumerate() {
            let did = ast::DeclId(di as u32);
            let mut members: &[ast::MemberId] = &[];
            let mut primary: Option<ast::MemberId> = None;
            match &d.kind {
                DeclKind::Class(x) => {
                    members = &x.members;
                    // O alias de classe (`class _A = S with M;`) não é
                    // visitado pelo verificador do 3.6.2 (não há
                    // `visitClassTypeAlias`): nunca é relatado.
                    if let Some(c) = decl_class(u, did)
                        && !x.mixin_application
                        && privado(interner, x.name.sym)
                        && !used_element(El::Classe(c), &d.metadata, u, &mut engine)
                        && !marked(&cx, &mut engine, u, &d.metadata, true)
                    {
                        report(u, w::UNUSED_ELEMENT, x.name, interner.resolve(x.name.sym).to_string());
                    }
                    primary = x.primary_constructor;
                }
                DeclKind::Mixin(x) => {
                    members = &x.members;
                    if let Some(c) = decl_class(u, did)
                        && privado(interner, x.name.sym)
                        && !used_element(El::Classe(c), &d.metadata, u, &mut engine)
                    {
                        report(u, w::UNUSED_ELEMENT, x.name, interner.resolve(x.name.sym).to_string());
                    }
                }
                DeclKind::Enum(x) => {
                    members = &x.members;
                    if let Some(c) = decl_class(u, did)
                        && privado(interner, x.name.sym)
                        && !used_element(El::Classe(c), &d.metadata, u, &mut engine)
                    {
                        report(u, w::UNUSED_ELEMENT, x.name, interner.resolve(x.name.sym).to_string());
                    }
                    // As constantes.
                    let vs = variables_of(u, Some(did), None);
                    for (i, k) in x.constants.iter().enumerate() {
                        if let Some(&v) = vs.get(&i)
                            && !read_value(v)
                        {
                            report(u, w::UNUSED_FIELD, k.name, interner.resolve(k.name.sym).to_string());
                        }
                    }
                    primary = x.primary_constructor;
                }
                DeclKind::ExtensionType(x) => {
                    members = &x.members;
                    if let Some(c) = decl_class(u, did)
                        && privado(interner, x.name.sym)
                        && !used_element(El::Classe(c), &d.metadata, u, &mut engine)
                    {
                        report(u, w::UNUSED_ELEMENT, x.name, interner.resolve(x.name.sym).to_string());
                    }
                }
                DeclKind::Extension(x) => members = &x.members,
                DeclKind::Typedef(t) => {
                    let tgt = program.typedefs.iter().position(|e| e.decl.unit == u && e.decl.decl == did).map(|i| TypedefId(i as u32));
                    if let Some(tid) = tgt
                        && privado(interner, t.name.sym)
                        && !used_element(El::Typedef(tid), &d.metadata, u, &mut engine)
                    {
                        report(u, w::UNUSED_ELEMENT, t.name, interner.resolve(t.name.sym).to_string());
                    }
                }
                DeclKind::Function(f) => {
                    let g = a.function(*f);
                    let (Some(n), Some(e)) = (g.name, node_function(u, *f)) else { continue };
                    let el = cx.of_function(e);
                    let the_used = match g.kind {
                        ast::FunctionKind::Getter | ast::FunctionKind::Setter => used_member(el, n.sym, &d.metadata, u, &mut engine),
                        _ => !privado(interner, n.sym) || used_element(el, &d.metadata, u, &mut engine),
                    };
                    if !the_used {
                        report(u, w::UNUSED_ELEMENT, n, interner.resolve(n.sym).to_string());
                    }
                }
                DeclKind::Variables(l) => {
                    let vs = variables_of(u, Some(did), None);
                    for (i, v) in l.variables.iter().enumerate() {
                        if let Some(&id) = vs.get(&i)
                            && privado(interner, v.name.sym)
                            && !used_element(El::Variavel(id), &d.metadata, u, &mut engine)
                        {
                            report(u, w::UNUSED_ELEMENT, v.name, interner.resolve(v.name.sym).to_string());
                        }
                    }
                }
            }
            // Os membros.
            let type_name = match &d.kind {
                DeclKind::Class(x) => interner.resolve(x.name.sym).to_string(),
                DeclKind::Mixin(x) => interner.resolve(x.name.sym).to_string(),
                DeclKind::Enum(x) => interner.resolve(x.name.sym).to_string(),
                DeclKind::ExtensionType(x) => interner.resolve(x.name.sym).to_string(),
                DeclKind::Extension(x) => x.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default(),
                _ => String::new(),
            };
            // `visitPrimaryConstructorDeclaration` (3.13,
            // `unused_local_elements_verifier.dart:1022-1057` do checkout
            // main): o campo de um parâmetro declarante (`final`/`var`) de
            // classe ou enum não lido é `UNUSED_FIELD_FROM_PRIMARY_CONSTRUCTOR`
            // no nome do parâmetro, com o nome e a palavra; o tipo de extensão
            // nunca relata. O parser elabora o campo com o nome do parâmetro.
            let declaring_names: HashSet<usize> = match primary.map(|m| &a.member(m).kind) {
                Some(MemberKind::Constructor(k)) if matches!(d.kind, DeclKind::Class(_) | DeclKind::Enum(_)) => {
                    k.parameters.iter().filter(|p| p.this_).filter_map(|p| p.name).map(|n| n.span.start).collect()
                }
                _ => HashSet::new(),
            };
            for &mid in members {
                if Some(mid) == primary {
                    continue;
                }
                let mem = a.member(mid);
                match &mem.kind {
                    MemberKind::Method(f) => {
                        let g = a.function(*f);
                        let (Some(n), Some(e)) = (g.name, node_function(u, *f)) else { continue };
                        let el = cx.of_function(e);
                        if !used_member(el, n.sym, &mem.metadata, u, &mut engine) {
                            report(u, w::UNUSED_ELEMENT, n, interner.resolve(n.sym).to_string());
                        }
                    }
                    MemberKind::Field(l) => {
                        let vs = variables_of(u, None, Some(mid));
                        for (i, v) in l.variables.iter().enumerate() {
                            // O campo de um parâmetro declarante `_` é curinga:
                            // não tem getter, e o `_isReadMember` dele é falso
                            // (`field.getter == null`) mesmo com leituras de `_`.
                            let wildcard = declaring_names.contains(&v.name.span.start) && interner.resolve(v.name.sym) == "_";
                            if let Some(&id) = vs.get(&i)
                                && (wildcard || !read_value(id))
                            {
                                if declaring_names.contains(&v.name.span.start) {
                                    let keyword = if l.final_ { "final" } else { "var" };
                                    of_primary.push((
                                        u,
                                        Diagnostic::com_codigo(w::UNUSED_FIELD_FROM_PRIMARY_CONSTRUCTOR, v.name.span, [interner.resolve(v.name.sym), keyword]),
                                    ));
                                } else {
                                    report(u, w::UNUSED_FIELD, v.name, interner.resolve(v.name.sym).to_string());
                                }
                            }
                        }
                    }
                    MemberKind::Constructor(k) => {
                        let (Some(n), Some(e)) = (k.name, member_constructor(u, mid)) else { continue };
                        if interner.resolve(n.sym) == "new" || k.parte_primaria {
                            continue;
                        }
                        let Some(c) = program.function(e).class else { continue };
                        // O construtor da representação de um tipo de
                        // extensão não está no modelo, mas conta.
                        let representation = usize::from(program.class(c).kind == ClassKind::ExtensionType);
                        if program.class(c).constructors.len() + representation > 1 && !used_member(El::Funcao(e), n.sym, &mem.metadata, u, &mut engine) {
                            report(u, w::UNUSED_ELEMENT, n, format!("{type_name}.{}", interner.resolve(n.sym)));
                        }
                    }
                }
            }
        }
    }
    output.extend(of_primary);
    output.sort_by_key(|(u, d)| (u.0, d.span.start));
    output.dedup_by(|x, y| x.0 == y.0 && x.1.span == y.1.span && x.1.code == y.1.code);
    output
}
