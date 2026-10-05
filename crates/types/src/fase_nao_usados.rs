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
struct Usados {
    elementos: HashSet<El>,
    membros: HashSet<El>,
    lidos: HashSet<El>,
    nao_resolvidos: HashSet<SymbolId>,
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
    outline: &'a OutlineTypes,
    lib: LibraryId,
}

impl Contexto<'_> {
    /// O elemento de um membro de função (o acessor sintético vale pela
    /// variável).
    fn da_funcao(&self, f: FunctionElementId) -> El {
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

    fn do_elemento(&self, e: Element) -> Option<El> {
        Some(match e {
            Element::Class(c) => El::Classe(c),
            Element::Extension(x) => El::Extensao(x),
            Element::Typedef(t) => El::Typedef(t),
            Element::Function(f) => self.da_funcao(f),
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
    fn chave_de_membro(&self, e: El) -> Option<(Option<ClassId>, Option<ExtensionId>, String)> {
        let p = self.program;
        match e {
            El::Funcao(f) => {
                let g = p.function(f);
                let mut nome = self.interner.resolve(g.name).to_string();
                if g.kind == FunctionKind::Setter && !nome.ends_with("_=") {
                    nome.push_str("_=");
                }
                Some((g.class, g.extension, nome))
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
                let (classe, extensao, nome) = self.chave_de_membro(e)?;
                let base = nome.strip_suffix("_=").unwrap_or(&nome).to_string();
                let s = self.interner.lookup(&base)?;
                let alvo = match (classe, extensao) {
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
                alvo.map(|g| self.da_funcao(g))
            }
            _ => None,
        }
    }

    /// `_overriddenElements` (o `getOverridden2` na classe do membro, com o
    /// nome privado só da biblioteca).
    fn sobrescritos(&self, e: El) -> Vec<El> {
        let Some((Some(c), _, nome)) = self.chave_de_membro(e) else { return Vec::new() };
        let Some(s) = self.interner.lookup(&nome) else { return Vec::new() };
        let privado = nome.starts_with('_');
        let Some(h) = self.outline.hierarchy.get(c) else { return Vec::new() };
        let mut v = Vec::new();
        for &k in h.supertypes.keys() {
            if k == c {
                continue;
            }
            let x = self.program.class(k);
            if privado && x.library != self.lib {
                continue;
            }
            if let Some(&f) = x.instance_members.get(&s) {
                v.push(self.da_funcao(f));
            }
        }
        v
    }

    /// `_overridesUsedElement`.
    fn sobrescreve_usado(&self, e: El, usados: &Usados, vistos: &mut HashSet<El>) -> bool {
        if !vistos.insert(e) {
            return false;
        }
        self.sobrescritos(e).into_iter().any(|o| usados.membros.contains(&o) || self.sobrescreve_usado(o, usados, vistos))
    }

    /// `_isPubliclyAccessible` de um método, acessor ou construtor.
    fn acessivel(&self, e: El, nome: SymbolId) -> bool {
        if privado(self.interner, nome) {
            return false;
        }
        let p = self.program;
        let (classe, extensao, estatico, construtor, fabrica) = match e {
            El::Funcao(f) => {
                let g = p.function(f);
                let construtor = matches!(g.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor);
                (g.class, g.extension, g.static_, construtor, g.factory)
            }
            El::Getter(v) | El::Setter(v) => {
                let x = p.variable(v);
                (x.class, x.extension, x.static_, false, false)
            }
            _ => return true,
        };
        if let Some(c) = classe {
            let k = p.class(c);
            if k.kind == ClassKind::Enum && construtor && !fabrica {
                return false;
            }
            if privado(self.interner, k.name) && (estatico || construtor) {
                return false;
            }
        }
        if let Some(x) = extensao {
            return p.extension(x).name.is_some_and(|n| !privado(self.interner, n));
        }
        true
    }
}

/// Os tipos escritos direto numa lista de variáveis que não é de campo e no
/// `is`: neles o tipo de interface nomeado não conta como uso (o pai do
/// `NamedType` é a lista ou o `IsExpression`).
fn tipos_sem_uso_de_interface(a: &ast::Ast) -> HashSet<ast::TypeId> {
    let mut v = HashSet::new();
    fn lista(l: &ast::VariableList, v: &mut HashSet<ast::TypeId>) {
        if let Some(t) = l.ty {
            v.insert(t);
        }
    }
    fn de_colecao(el: &ast::CollectionElement, v: &mut HashSet<ast::TypeId>) {
        match el {
            ast::CollectionElement::For { init, body, .. } => {
                if let Some(ForInit::Variables(l)) = init {
                    lista(l, v);
                }
                de_colecao(body, v);
            }
            ast::CollectionElement::ForIn { body, .. } => de_colecao(body, v),
            ast::CollectionElement::If { then, else_, .. } => {
                de_colecao(then, v);
                if let Some(x) = else_ {
                    de_colecao(x, v);
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
                    de_colecao(el, &mut v);
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
fn marcado(cx: &Contexto<'_>, motor: &mut Option<Motor<'_>>, u: UnitId, metadata: &[ast::Annotation], js: bool) -> bool {
    let p = cx.program;
    let nome_da_biblioteca = |l: LibraryId| {
        p.library(l).name.as_ref().map(|n| n.iter().map(|s| cx.interner.resolve(*s)).collect::<Vec<_>>().join(".")).unwrap_or_default()
    };
    for m in metadata {
        let Some(args) = &m.arguments else { continue };
        let Some(Element::Class(c)) = crate::anotacoes::elemento_da_anotacao(p, cx.interner, u, m) else { continue };
        let k = p.class(c);
        let nome = cx.interner.resolve(k.name);
        if js && nome == "JS" && nome_da_biblioteca(k.library) == "_js_annotations" {
            return true;
        }
        if nome == "pragma" && nome_da_biblioteca(k.library) == "dart.core" {
            // O `name` do `pragma` é o primeiro argumento posicional.
            let Some(primeiro) = args.args.iter().find(|x| x.name.is_none()) else { continue };
            let a = &p.unit(u).ast;
            let valor = match &a.expr(primeiro.value).kind {
                ExprKind::String(s) => s.constant_value().map(|t| t.to_string_lossy()),
                _ => motor.as_mut().and_then(|mm| {
                    let cxm = Ctx::simples(u, p.unit(u).library);
                    match mm.avaliar(&cxm, primeiro.value, true) {
                        Constante::Valor(v) => match v.estado {
                            crate::constantes::valor::Estado::Str(Some(t)) => Some(String::from_utf16_lossy(&t)),
                            _ => None,
                        },
                        Constante::Invalida(_) => None,
                    }
                }),
            };
            if valor.as_deref() == Some("vm:entry-point") {
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
    corpos: &BodyTypes,
    inferidas: &HashSet<LibraryId>,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let cx = Contexto { program, interner, outline, lib };
    let mut usados = Usados::default();
    let unidades: Vec<UnitId> = program.library(lib).units.iter().copied().filter(|u| program.unit(*u).role != UnitRole::Patch).collect();
    let classe_da_decl = |u: UnitId, d: ast::DeclId| {
        program.classes.iter().position(|c| c.decl.is_some_and(|r| r.unit == u && r.decl == d)).map(|i| ClassId(i as u32))
    };
    let funcao_do_no = |u: UnitId, f: ast::FunctionId| {
        program
            .functions
            .iter()
            .position(|e| matches!(e.node, dartforge_elements::model::FunctionRef::Function { unit, function } if unit == u && function == f))
            .map(|i| FunctionElementId(i as u32))
    };
    let construtor_do_membro = |u: UnitId, m: ast::MemberId| {
        program
            .functions
            .iter()
            .position(|e| matches!(e.node, dartforge_elements::model::FunctionRef::Constructor { unit, member } if unit == u && member == m))
            .map(|i| FunctionElementId(i as u32))
    };

    for &u in &unidades {
        let a = &program.unit(u).ast;
        let Some(corpo) = corpos.units.get(u.0 as usize) else { continue };
        let pais = crate::lints_tipados::pais_da_unidade(program, u);
        // As classes (`ClassDeclaration`) e os executáveis (funções
        // declaradas e métodos) que contêm cada lugar.
        let classes: Vec<(Span, ClassId)> = a
            .decls
            .iter()
            .enumerate()
            .filter(|(_, d)| matches!(d.kind, DeclKind::Class(_)))
            .filter_map(|(i, d)| classe_da_decl(u, ast::DeclId(i as u32)).map(|c| (d.span, c)))
            .collect();
        let mut executaveis: Vec<(Span, FunctionElementId)> = Vec::new();
        for d in a.decls.iter() {
            if let DeclKind::Function(f) = &d.kind
                && let Some(e) = funcao_do_no(u, *f)
            {
                executaveis.push((a.function(*f).span, e));
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Method(f) = &m.kind
                && let Some(e) = funcao_do_no(u, *f)
            {
                executaveis.push((a.function(*f).span, e));
            }
        }
        let classe_em = |s: Span| classes.iter().filter(|(r, _)| dentro(s, *r)).min_by_key(|(r, _)| r.end - r.start).map(|x| x.1);
        let executavel_em = |s: Span| executaveis.iter().filter(|(r, _)| dentro(s, *r)).min_by_key(|(r, _)| r.end - r.start).map(|x| x.1);
        // `_useIdentifierElement`.
        let usar = |usados: &mut Usados, e: El, onde: Span| {
            if cx.biblioteca(e) != lib {
                return;
            }
            if let El::Classe(c) = e
                && classe_em(onde) == Some(c)
            {
                return;
            }
            if let El::Funcao(f) = e
                && executavel_em(onde) == Some(f)
            {
                return;
            }
            usados.elementos.insert(e);
        };
        // `_addMemberAndCorrespondingGetter`.
        let membro_lido = |usados: &mut Usados, e: El| match cx.getter_de(e) {
            Some(g) => {
                usados.membros.insert(g);
                usados.lidos.insert(g);
            }
            None if matches!(e, El::Setter(_)) => {}
            None => {
                usados.lidos.insert(e);
            }
        };
        // Os comandos de expressão.
        let comandos: HashSet<ExprId> = a
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Expression(e) => Some(*e),
                _ => None,
            })
            .collect();
        let pai_de = |e: ExprId| match pais.pai(e) {
            dartforge_frontend::pais::Pai::Expr(p) => Some(p),
            _ => None,
        };

        for (k, ex) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let (nome, propriedade) = match &ex.kind {
                ExprKind::Identifier(n) => (*n, false),
                ExprKind::Property { name, .. } => (*name, true),
                // A criação: o construtor nomeado (o identificador dele).
                ExprKind::InstanceCreation { constructor: Some(_), .. } | ExprKind::Call { .. } => {
                    if let Some(Resolved::Constructor(f)) = corpo.get_resolved(id) {
                        let escrito = match &ex.kind {
                            ExprKind::InstanceCreation { .. } => true,
                            ExprKind::Call { target, .. } => matches!(a.expr(*target).kind, ExprKind::Property { .. }),
                            _ => false,
                        };
                        let nomeado = !interner.resolve(program.function(*f).name).is_empty();
                        if escrito && nomeado {
                            usar(&mut usados, El::Funcao(*f), ex.span);
                            usados.membros.insert(El::Funcao(*f));
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            let pai = pai_de(id);
            // `inGetterContext` e `_isReadIdentifier`.
            let alvo_de = |p: ExprId| matches!(&a.expr(p).kind, ExprKind::Assign { target, .. } if *target == id);
            let atribuicao_simples = pai.is_some_and(|p| matches!(&a.expr(p).kind, ExprKind::Assign { op: ast::AssignOp::Assign, target, .. } if *target == id));
            let mut lido = !atribuicao_simples;
            if lido
                && !propriedade
                && let Some(p) = pai
                && comandos.contains(&p)
            {
                match &a.expr(p).kind {
                    ExprKind::Unary { operand, .. } if *operand == id => lido = false,
                    ExprKind::Assign { op, target, .. } if *target == id => lido = matches!(op, ast::AssignOp::Compound(ast::BinaryOp::IfNull)),
                    _ => {}
                }
            }
            let escrito = pai.is_some_and(|p| {
                alvo_de(p)
                    || matches!(&a.expr(p).kind, ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } if *operand == id)
            });
            match corpo.get_resolved(id) {
                None | Some(Resolved::Dynamic) => {
                    if lido {
                        usados.nao_resolvidos.insert(nome.sym);
                    }
                }
                Some(Resolved::Local(_) | Resolved::Parameter { .. } | Resolved::TypeParameter(_) | Resolved::Prefix(_)) => {}
                Some(Resolved::Element(e)) => {
                    let Some(el) = cx.do_elemento(*e) else { continue };
                    match el {
                        // A variável de topo (acessores sintéticos).
                        El::Variavel(v) | El::Getter(v) | El::Setter(v) if program.variable(v).class.is_none() => {
                            if lido {
                                usados.elementos.insert(El::Variavel(v));
                            }
                        }
                        // O acessor de topo explícito.
                        El::Funcao(f) if matches!(program.function(f).kind, FunctionKind::Getter | FunctionKind::Setter) => {
                            if lido {
                                usados.membros.insert(el);
                                membro_lido(&mut usados, el);
                            }
                            if escrito {
                                // O `writeElement`: o setter de mesmo nome.
                                let setter = program.library(program.function(f).library).declared.get(&program.function(f).name).and_then(|b| b.setter);
                                if let Some(Element::Function(g)) = setter {
                                    usar(&mut usados, El::Funcao(g), ex.span);
                                }
                            }
                            if !lido && !escrito {
                                usar(&mut usados, el, ex.span);
                            }
                        }
                        _ => usar(&mut usados, el, ex.span),
                    }
                }
                Some(Resolved::Member { class, member, .. }) => {
                    let (el_leitura, el_escrita) = match member {
                        MemberRef::Variable(v) => (El::Getter(*v), El::Setter(*v)),
                        MemberRef::Function(f) => {
                            let el = cx.da_funcao(*f);
                            match el {
                                El::Getter(v) | El::Setter(v) => (El::Getter(v), El::Setter(v)),
                                outro => (outro, outro),
                            }
                        }
                    };
                    // `values` de enum: todas as constantes lidas.
                    let k = program.class(*class);
                    if k.kind == ClassKind::Enum && interner.resolve(nome.sym) == "values" {
                        for &c in k.enum_constants.iter() {
                            usados.lidos.insert(El::Getter(c));
                        }
                        continue;
                    }
                    let mut alvos: Vec<El> = Vec::new();
                    if lido || !escrito {
                        alvos.push(el_leitura);
                    }
                    if escrito {
                        alvos.push(el_escrita);
                    }
                    for el in alvos {
                        usar(&mut usados, el, ex.span);
                        if matches!(el, El::Funcao(f) if executavel_em(ex.span) == Some(f)) {
                            continue;
                        }
                        usados.membros.insert(el);
                        if lido {
                            membro_lido(&mut usados, el);
                        }
                    }
                }
                Some(Resolved::ExtensionMember { member, .. }) => {
                    let el = cx.da_funcao(*member);
                    usar(&mut usados, el, ex.span);
                    if !matches!(el, El::Funcao(f) if executavel_em(ex.span) == Some(f)) {
                        usados.membros.insert(el);
                        if lido {
                            membro_lido(&mut usados, el);
                        }
                    }
                }
                Some(Resolved::Constructor(f)) => {
                    usar(&mut usados, El::Funcao(*f), ex.span);
                    usados.membros.insert(El::Funcao(*f));
                }
            }
        }

        // `visitNamedType`.
        let sem_interface = tipos_sem_uso_de_interface(a);
        for (ti, t) in a.types.iter().enumerate() {
            let TypeKind::Named { name, .. } = &t.kind else { continue };
            let elemento = match &name[..] {
                [n] => {
                    if crate::anotacoes::sombreado(a, interner, n.span.start, n.sym) {
                        continue;
                    }
                    program.lookup_na_unidade(u, n.sym).and_then(|b| b.getter)
                }
                [p, n, ..] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym).and_then(|b| b.getter),
                [] => None,
            };
            let Some(el) = elemento.and_then(|e| cx.do_elemento(e)) else { continue };
            if matches!(el, El::Classe(_)) && sem_interface.contains(&ast::TypeId(ti as u32)) {
                continue;
            }
            usar(&mut usados, el, t.span);
        }
        // Os nomes de anotação.
        for m in dartforge_frontend::pais::todas_as_anotacoes(a, &program.unit(u).unit) {
            if let Some(el) = crate::anotacoes::elemento_da_anotacao(program, interner, u, m).and_then(|e| cx.do_elemento(e)) {
                usar(&mut usados, el, m.span);
            }
        }
        // Os construtores das constantes de enum, os de redirecionamento e
        // os de `super.n(…)`.
        for (di, d) in a.decls.iter().enumerate() {
            let DeclKind::Enum(x) = &d.kind else { continue };
            let Some(c) = classe_da_decl(u, ast::DeclId(di as u32)) else { continue };
            for k in x.constants.iter() {
                let nome = k.constructor.map(|n| n.sym).or_else(|| interner.lookup(""));
                if let Some(f) = nome.and_then(|n| program.class(c).constructors.get(&n)) {
                    usados.elementos.insert(El::Funcao(*f));
                }
            }
        }
        for (mi, m) in a.members.iter().enumerate() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            let Some(eu) = construtor_do_membro(u, ast::MemberId(mi as u32)) else { continue };
            let Some(classe) = program.function(eu).class else { continue };
            let mut marcar = |c: ClassId, n: Option<ast::Name>| {
                let Some(n) = n else { return };
                if let Some(&f) = program.class(c).constructors.get(&n.sym) {
                    usar(&mut usados, El::Funcao(f), m.span);
                    usados.membros.insert(El::Funcao(f));
                }
            };
            for i in k.initializers.iter() {
                match i {
                    Initializer::Redirect { constructor, .. } => marcar(classe, *constructor),
                    Initializer::Super { constructor, .. } => {
                        if let Some(s) = program.class(classe).supertype_class {
                            marcar(s, *constructor);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(r) = &k.redirect
                && let TypeKind::Named { name, .. } = &a.ty(r.ty).kind
            {
                let alvo = match &name[..] {
                    [n] => program.lookup_na_unidade(u, n.sym).and_then(|b| b.getter),
                    [p, n, ..] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym).and_then(|b| b.getter),
                    [] => None,
                };
                if let Some(Element::Class(c)) = alvo {
                    marcar(c, r.constructor);
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
            let Some(alvo) = outline.tipos_escritos.get(&(u, *ty)) else { continue };
            if let crate::table::Type::Interface { class, .. } = table.get(*alvo) {
                for (n, f) in program.class(*class).constructors.iter() {
                    if !privado(interner, *n) {
                        usados.elementos.insert(El::Funcao(*f));
                    }
                }
            }
        }
    }

    // O relato.
    let mut motor: Option<Motor<'_>> = None;
    let _ = (&core, &inferidas);
    let precisa_de_motor = unidades.iter().any(|&u| {
        dartforge_frontend::pais::todas_as_anotacoes(&program.unit(u).ast, &program.unit(u).unit)
            .iter()
            .any(|m| m.name.last().is_some_and(|n| interner.resolve(n.sym) == "pragma"))
    });
    if precisa_de_motor {
        motor = Some(Motor::novo(program, interner, table, core, outline, corpos, inferidas));
    }
    let mut saida: Vec<(UnitId, Diagnostic)> = Vec::new();
    let mut relatar = |u: UnitId, codigo, n: ast::Name, exibido: String| {
        saida.push((u, Diagnostic::com_codigo(codigo, n.span, [exibido.as_str()])));
    };
    let usado_elemento = |el: El, metadata: &[ast::Annotation], u: UnitId, motor: &mut Option<Motor<'_>>| {
        usados.elementos.contains(&el) || marcado(&cx, motor, u, metadata, false)
    };
    let usado_membro = |el: El, nome: SymbolId, metadata: &[ast::Annotation], u: UnitId, motor: &mut Option<Motor<'_>>| {
        cx.acessivel(el, nome)
            || marcado(&cx, motor, u, metadata, false)
            || usados.membros.contains(&el)
            || usados.elementos.contains(&el)
            || cx.sobrescreve_usado(el, &usados, &mut HashSet::new())
    };
    // `_isReadMember` de um campo ou constante de enum.
    let lido = |v: VariableId| {
        let x = program.variable(v);
        let estatico = x.static_ || x.const_ && x.class.is_some_and(|c| program.class(c).kind == ClassKind::Enum);
        let conteiner_privado = x.class.is_some_and(|c| privado(interner, program.class(c).name))
            || x.extension.is_some_and(|e| program.extension(e).name.is_none_or(|n| privado(interner, n)));
        if !privado(interner, x.name) && !(conteiner_privado && estatico) {
            return true;
        }
        if usados.lidos.contains(&El::Getter(v)) || usados.nao_resolvidos.contains(&x.name) {
            return true;
        }
        if estatico {
            return false;
        }
        cx.sobrescreve_usado(El::Getter(v), &usados, &mut HashSet::new())
    };
    let variaveis_de = |u: UnitId, decl: Option<ast::DeclId>, membro: Option<ast::MemberId>| -> HashMap<usize, VariableId> {
        let mut m = HashMap::new();
        for (i, x) in program.variables.iter().enumerate() {
            match x.node {
                dartforge_elements::model::VariableRef::TopLevel { unit, decl: d, index } if unit == u && Some(d) == decl => {
                    m.insert(index, VariableId(i as u32));
                }
                dartforge_elements::model::VariableRef::Field { unit, member, index } if unit == u && Some(member) == membro => {
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
    for &u in &unidades {
        let a = &program.unit(u).ast;
        for (di, d) in a.decls.iter().enumerate() {
            let did = ast::DeclId(di as u32);
            let mut membros: &[ast::MemberId] = &[];
            let mut primario: Option<ast::MemberId> = None;
            match &d.kind {
                DeclKind::Class(x) => {
                    membros = &x.members;
                    if let Some(c) = classe_da_decl(u, did)
                        && privado(interner, x.name.sym)
                        && !usado_elemento(El::Classe(c), &d.metadata, u, &mut motor)
                        && !marcado(&cx, &mut motor, u, &d.metadata, true)
                    {
                        relatar(u, w::UNUSED_ELEMENT, x.name, interner.resolve(x.name.sym).to_string());
                    }
                    primario = x.primary_constructor;
                }
                DeclKind::Mixin(x) => {
                    membros = &x.members;
                    if let Some(c) = classe_da_decl(u, did)
                        && privado(interner, x.name.sym)
                        && !usado_elemento(El::Classe(c), &d.metadata, u, &mut motor)
                    {
                        relatar(u, w::UNUSED_ELEMENT, x.name, interner.resolve(x.name.sym).to_string());
                    }
                }
                DeclKind::Enum(x) => {
                    membros = &x.members;
                    if let Some(c) = classe_da_decl(u, did)
                        && privado(interner, x.name.sym)
                        && !usado_elemento(El::Classe(c), &d.metadata, u, &mut motor)
                    {
                        relatar(u, w::UNUSED_ELEMENT, x.name, interner.resolve(x.name.sym).to_string());
                    }
                    // As constantes.
                    let vs = variaveis_de(u, Some(did), None);
                    for (i, k) in x.constants.iter().enumerate() {
                        if let Some(&v) = vs.get(&i)
                            && !lido(v)
                        {
                            relatar(u, w::UNUSED_FIELD, k.name, interner.resolve(k.name.sym).to_string());
                        }
                    }
                    primario = x.primary_constructor;
                }
                DeclKind::ExtensionType(x) => {
                    membros = &x.members;
                    if let Some(c) = classe_da_decl(u, did)
                        && privado(interner, x.name.sym)
                        && !usado_elemento(El::Classe(c), &d.metadata, u, &mut motor)
                    {
                        relatar(u, w::UNUSED_ELEMENT, x.name, interner.resolve(x.name.sym).to_string());
                    }
                }
                DeclKind::Extension(x) => membros = &x.members,
                DeclKind::Typedef(t) => {
                    let alvo = program.typedefs.iter().position(|e| e.decl.unit == u && e.decl.decl == did).map(|i| TypedefId(i as u32));
                    if let Some(tid) = alvo
                        && privado(interner, t.name.sym)
                        && !usado_elemento(El::Typedef(tid), &d.metadata, u, &mut motor)
                    {
                        relatar(u, w::UNUSED_ELEMENT, t.name, interner.resolve(t.name.sym).to_string());
                    }
                }
                DeclKind::Function(f) => {
                    let g = a.function(*f);
                    let (Some(n), Some(e)) = (g.name, funcao_do_no(u, *f)) else { continue };
                    let el = cx.da_funcao(e);
                    let usado = match g.kind {
                        ast::FunctionKind::Getter | ast::FunctionKind::Setter => usado_membro(el, n.sym, &d.metadata, u, &mut motor),
                        _ => !privado(interner, n.sym) || usado_elemento(el, &d.metadata, u, &mut motor),
                    };
                    if !usado {
                        relatar(u, w::UNUSED_ELEMENT, n, interner.resolve(n.sym).to_string());
                    }
                }
                DeclKind::Variables(l) => {
                    let vs = variaveis_de(u, Some(did), None);
                    for (i, v) in l.variables.iter().enumerate() {
                        if let Some(&id) = vs.get(&i)
                            && privado(interner, v.name.sym)
                            && !usado_elemento(El::Variavel(id), &d.metadata, u, &mut motor)
                        {
                            relatar(u, w::UNUSED_ELEMENT, v.name, interner.resolve(v.name.sym).to_string());
                        }
                    }
                }
            }
            // Os membros.
            let nome_do_tipo = match &d.kind {
                DeclKind::Class(x) => interner.resolve(x.name.sym).to_string(),
                DeclKind::Mixin(x) => interner.resolve(x.name.sym).to_string(),
                DeclKind::Enum(x) => interner.resolve(x.name.sym).to_string(),
                DeclKind::ExtensionType(x) => interner.resolve(x.name.sym).to_string(),
                DeclKind::Extension(x) => x.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default(),
                _ => String::new(),
            };
            for &mid in membros {
                if Some(mid) == primario {
                    continue;
                }
                let membro = a.member(mid);
                match &membro.kind {
                    MemberKind::Method(f) => {
                        let g = a.function(*f);
                        let (Some(n), Some(e)) = (g.name, funcao_do_no(u, *f)) else { continue };
                        let el = cx.da_funcao(e);
                        if !usado_membro(el, n.sym, &membro.metadata, u, &mut motor) {
                            relatar(u, w::UNUSED_ELEMENT, n, interner.resolve(n.sym).to_string());
                        }
                    }
                    MemberKind::Field(l) => {
                        let vs = variaveis_de(u, None, Some(mid));
                        for (i, v) in l.variables.iter().enumerate() {
                            if let Some(&id) = vs.get(&i)
                                && !lido(id)
                            {
                                relatar(u, w::UNUSED_FIELD, v.name, interner.resolve(v.name.sym).to_string());
                            }
                        }
                    }
                    MemberKind::Constructor(k) => {
                        let (Some(n), Some(e)) = (k.name, construtor_do_membro(u, mid)) else { continue };
                        if interner.resolve(n.sym) == "new" || k.parte_primaria {
                            continue;
                        }
                        let Some(c) = program.function(e).class else { continue };
                        if program.class(c).constructors.len() > 1 && !usado_membro(El::Funcao(e), n.sym, &membro.metadata, u, &mut motor) {
                            relatar(u, w::UNUSED_ELEMENT, n, format!("{nome_do_tipo}.{}", interner.resolve(n.sym)));
                        }
                    }
                }
            }
        }
    }
    saida.sort_by_key(|(u, d)| (u.0, d.span.start));
    saida.dedup_by(|x, y| x.0 == y.0 && x.1.span == y.1.span && x.1.code == y.1.code);
    saida
}
