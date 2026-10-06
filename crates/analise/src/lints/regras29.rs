//! O vigésimo nono lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `unreachable_from_main`: as declarações da biblioteca
//!   (`_DeclarationGatherer`: as de topo, cada variável de topo, e os
//!   construtores, campos e métodos públicos dos tipos de topo públicos que
//!   não sobrescrevem nada nem são métodos de teste) que nenhum ponto de
//!   entrada (`main` ou `@pragma('vm:entry-point')` de topo) alcança pelas
//!   referências (`_ReferenceVisitor`). As referências de cada declaração
//!   são as da subárvore dela: os identificadores lidos (o lado esquerdo de
//!   atribuição e o operando de `++`/`--` entram pelos elementos de leitura e
//!   de escrita), os nomes de construtor (fora de padrão constante, com o
//!   tipo), as anotações, os campos de padrão de objeto, os tipos nomeados
//!   só quando são typedef, tipo de extensão, argumento de tipo ou tipo de
//!   membro `external`, as cláusulas de supertipo de classe e de tipo de
//!   extensão, o construtor sem nome da superclasse (classe sem construtor,
//!   ou construtor sem `super(…)`), os `super(…)`/`this(…)`, o alvo de
//!   factory redirecionadora e o `toJson` de instância. Cada referência
//!   acrescenta a declaração de topo que contém o elemento e, se o elemento
//!   e o tipo que o contém são públicos, a do próprio membro.
//!
//!   Divergência registrada: as referências de comentário de documentação
//!   (`[nome]`) não entram; o `@pragma` só vale escrito com a string
//!   literal.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, FunctionRef, Program, TypedefId, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, Annotation, Ast, DeclId, DeclKind, ExprId, ExprKind, Initializer, MemberId, MemberKind, PatternKind, TypeKind, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolved::{MemberRef, Resolved, UnitBodyTypes};
use dartforge_types::table::Type;
use std::collections::{HashMap, HashSet};

/// Uma declaração que a regra pode relatar.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Dec {
    Topo(UnitId, DeclId),
    VarTopo(UnitId, DeclId, usize),
    Membro(UnitId, MemberId),
    Campo(UnitId, MemberId, usize),
}

impl Dec {
    fn unidade(self) -> UnitId {
        match self {
            Dec::Topo(u, ..) | Dec::VarTopo(u, ..) | Dec::Membro(u, ..) | Dec::Campo(u, ..) => u,
        }
    }
}

/// Um elemento referenciado.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Ref {
    Classe(ClassId),
    Extensao(ExtensionId),
    Typedef(TypedefId),
    Funcao(FunctionElementId),
    Variavel(VariableId),
    /// Um elemento local (variável, parâmetro, parâmetro de tipo): a
    /// declaração de topo que contém o ponto.
    Posicao(UnitId, usize),
}

#[derive(Clone, Copy, Debug)]
enum Acao {
    /// `_addDeclaration`.
    Elemento(Ref),
    /// `_addNamedType`: só a declaração do próprio elemento.
    Direto(Ref),
}

struct Cx<'a> {
    program: &'a Program,
    interner: &'a Interner,
    s: &'a super::Semantica<'a>,
    declaracoes: HashSet<Dec>,
    /// As regiões de topo de cada unidade: (início, fim, declaração).
    topos: HashMap<UnitId, Vec<(usize, usize, Dec)>>,
}

impl Cx<'_> {
    fn privado(&self, n: SymbolId) -> bool {
        self.interner.resolve(n).starts_with('_')
    }

    fn classe_da_decl(&self, u: UnitId, d: DeclId) -> Option<ClassId> {
        (0..self.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| self.program.class(*c).decl.is_some_and(|r| r.unit == u && r.decl == d))
    }

    /// `declarationMap[element]`.
    fn dec_de(&self, r: Ref) -> Option<Dec> {
        let program = self.program;
        let d = match r {
            Ref::Classe(c) => {
                let x = program.class(c).decl?;
                Dec::Topo(x.unit, x.decl)
            }
            Ref::Extensao(e) => {
                let x = program.extension(e).decl;
                Dec::Topo(x.unit, x.decl)
            }
            Ref::Typedef(t) => {
                let x = program.typedef(t).decl;
                Dec::Topo(x.unit, x.decl)
            }
            Ref::Funcao(f) => {
                let g = program.function(f);
                match g.node {
                    FunctionRef::Constructor { unit, member } => Dec::Membro(unit, member),
                    FunctionRef::Function { unit, function } => {
                        let a = &program.unit(unit).ast;
                        if let Some(m) = a.members.iter().position(|m| matches!(m.kind, MemberKind::Method(x) if x == function)) {
                            Dec::Membro(unit, MemberId(m as u32))
                        } else {
                            let d = a.decls.iter().position(|d| matches!(d.kind, DeclKind::Function(x) if x == function))?;
                            Dec::Topo(unit, DeclId(d as u32))
                        }
                    }
                    FunctionRef::None => return self.dec_de(Ref::Variavel(g.variable?)),
                }
            }
            Ref::Variavel(v) => match program.variable(v).node {
                VariableRef::TopLevel { unit, decl, index } => Dec::VarTopo(unit, decl, index),
                VariableRef::Field { unit, member, index } => Dec::Campo(unit, member, index),
                _ => return None,
            },
            Ref::Posicao(u, p) => {
                let topos = self.topos.get(&u)?;
                topos.iter().find(|(i, f, _)| *i <= p && p < *f)?.2
            }
        };
        self.declaracoes.contains(&d).then_some(d)
    }

    /// O nome do elemento é privado (`isPrivate`).
    fn elemento_privado(&self, r: Ref) -> bool {
        let program = self.program;
        match r {
            Ref::Classe(c) => self.privado(program.class(c).name),
            Ref::Extensao(e) => program.extension(e).name.is_none_or(|n| self.privado(n)),
            Ref::Typedef(t) => self.privado(program.typedef(t).name),
            Ref::Funcao(f) => match program.function(f).node {
                FunctionRef::Constructor { unit, member } => match &program.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(k) => k.name.is_some_and(|n| self.privado(n.sym)),
                    _ => false,
                },
                _ => self.privado(program.function(f).name),
            },
            Ref::Variavel(v) => self.privado(program.variable(v).name),
            Ref::Posicao(..) => false,
        }
    }

    /// `_addDeclaration`.
    fn adicionar(&self, r: Ref, out: &mut HashSet<Dec>) {
        let program = self.program;
        let (classe, extensao) = match r {
            Ref::Funcao(f) => (program.function(f).class, program.function(f).extension),
            Ref::Variavel(v) => (program.variable(v).class, program.variable(v).extension),
            _ => (None, None),
        };
        let topo = match (classe, extensao) {
            (Some(c), _) => Ref::Classe(c),
            (None, Some(e)) => Ref::Extensao(e),
            _ => r,
        };
        if let Some(d) = self.dec_de(topo) {
            out.insert(d);
        }
        if self.elemento_privado(r) {
            return;
        }
        let dono_privado = match (classe, extensao) {
            (Some(c), _) => self.elemento_privado(Ref::Classe(c)),
            (None, Some(e)) => self.elemento_privado(Ref::Extensao(e)),
            _ => return,
        };
        if dono_privado {
            return;
        }
        if let Some(d) = self.dec_de(r) {
            out.insert(d);
        }
    }

    fn agir(&self, a: Acao, out: &mut HashSet<Dec>) {
        match a {
            Acao::Elemento(r) => self.adicionar(r, out),
            Acao::Direto(r) => {
                if let Some(d) = self.dec_de(r) {
                    out.insert(d);
                }
            }
        }
    }

    /// O elemento de uma resolução.
    fn de_resolvido(&self, u: UnitId, corpo: &UnitBodyTypes, e: ExprId, pos: usize) -> Option<Ref> {
        Some(match corpo.get_resolved(e)? {
            Resolved::Local(_) | Resolved::Parameter { .. } | Resolved::TypeParameter(_) => Ref::Posicao(u, pos),
            Resolved::Member { member: MemberRef::Variable(v), .. } | Resolved::Element(Element::Variable(v)) => Ref::Variavel(*v),
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) => Ref::Funcao(*f),
            Resolved::ExtensionMember { member, .. } => Ref::Funcao(*member),
            Resolved::Constructor(f) => Ref::Funcao(*f),
            Resolved::Element(Element::Class(c)) => Ref::Classe(*c),
            Resolved::Element(Element::Extension(x)) => Ref::Extensao(*x),
            Resolved::Element(Element::Typedef(t)) => Ref::Typedef(*t),
            _ => return None,
        })
    }

    /// O elemento de leitura que acompanha o de escrita (o getter do mesmo
    /// nome, para o setter escrito).
    fn leitura_de(&self, r: Ref) -> Option<Ref> {
        let Ref::Funcao(f) = r else { return Some(r) };
        let g = self.program.function(f);
        if g.kind != FunctionKind::Setter {
            return Some(r);
        }
        if let Some(v) = g.variable {
            return Some(Ref::Variavel(v));
        }
        let texto = self.interner.resolve(g.name);
        let nome = self.interner.lookup(texto.strip_suffix("_=").unwrap_or(texto))?;
        let membros = match (g.class, g.extension) {
            (Some(c), _) => {
                let x = self.program.class(c);
                x.instance_members.get(&nome).or_else(|| x.static_members.get(&nome)).copied()
            }
            (None, Some(e)) => {
                let x = self.program.extension(e);
                x.instance_members.get(&nome).or_else(|| x.static_members.get(&nome)).copied()
            }
            _ => match self.program.lookup_na_unidade(self.s.unidade, nome)?.getter? {
                Element::Function(h) => Some(h),
                Element::Variable(v) => return Some(Ref::Variavel(v)),
                _ => None,
            },
        };
        membros.map(Ref::Funcao)
    }

    /// O elemento de um nome de tipo (`NamedType.element`), sem o parâmetro
    /// de tipo.
    fn elemento_do_tipo(&self, u: UnitId, corpo: &UnitBodyTypes, t: ast::TypeId) -> Option<Ref> {
        let a = &self.program.unit(u).ast;
        let TypeKind::Named { name, .. } = &a.ty(t).kind else { return None };
        let tipo = corpo.tipos_de_anotacoes.get(&t).copied().or_else(|| self.s.outline.tipos_escritos.get(&(u, t)).copied());
        if tipo.is_some_and(|x| matches!(self.s.table.get(x), Type::TypeParameter { .. })) {
            return None;
        }
        let b = match &name[..] {
            [n] => self.program.lookup_na_unidade(u, n.sym),
            [p, n] => self.program.lookup_prefixed_na_unidade(u, p.sym, n.sym),
            _ => None,
        }?;
        match b.getter? {
            Element::Class(c) => Some(Ref::Classe(c)),
            Element::Typedef(x) => Some(Ref::Typedef(x)),
            Element::Extension(x) => Some(Ref::Extensao(x)),
            _ => None,
        }
    }

    /// `getOverridden2(container, Name(lib, nome)) != null`.
    fn sobrescreve(&self, classe: ClassId, nome: SymbolId, setter: bool) -> bool {
        let chave = if setter { self.interner.lookup(&format!("{}_=", self.interner.resolve(nome))) } else { Some(nome) };
        let Some(chave) = chave else { return false };
        let minha = self.program.class(classe).library;
        let e_privado = self.privado(nome);
        self.s.outline.hierarchy.get(classe).is_some_and(|h| {
            h.supertypes.keys().any(|sc| {
                *sc != classe && self.program.class(*sc).instance_members.get(&chave).is_some_and(|f| !e_privado || self.program.function(*f).library == minha)
            })
        })
    }

    /// O construtor `nome` (`None`: o sem nome) da classe.
    fn construtor(&self, c: ClassId, nome: Option<SymbolId>) -> Option<FunctionElementId> {
        let chave = match nome {
            Some(n) => n,
            None => self.interner.lookup("")?,
        };
        self.program.class(c).constructors.get(&chave).copied()
    }
}

/// `_DeclarationGatherer.addDeclarations` de uma unidade.
fn declaracoes_da_unidade(cx: &Cx<'_>, u: UnitId) -> Vec<Dec> {
    let a = &cx.program.unit(u).ast;
    let mut v = Vec::new();
    for (k, d) in a.decls.iter().enumerate() {
        let id = DeclId(k as u32);
        if let DeclKind::Variables(l) = &d.kind {
            v.extend((0..l.variables.len()).map(|i| Dec::VarTopo(u, id, i)));
            continue;
        }
        v.push(Dec::Topo(u, id));
        let (nome, membros, interface): (Option<SymbolId>, &[MemberId], Option<ClassId>) = match &d.kind {
            DeclKind::Class(x) if !x.mixin_application => (Some(x.name.sym), &x.members, cx.classe_da_decl(u, id)),
            DeclKind::Enum(x) => (Some(x.name.sym), &x.members, cx.classe_da_decl(u, id)),
            DeclKind::Mixin(x) => (Some(x.name.sym), &x.members, cx.classe_da_decl(u, id)),
            DeclKind::Extension(x) => (x.name.map(|n| n.sym), &x.members, None),
            DeclKind::ExtensionType(x) => (Some(x.name.sym), &x.members, None),
            _ => continue,
        };
        // O elemento privado (a extensão sem nome também) não tem membros.
        if nome.is_none_or(|n| cx.privado(n)) {
            continue;
        }
        let e_enum = matches!(d.kind, DeclKind::Enum(_));
        let sobrescreve = |n: SymbolId, setter: bool| interface.is_some_and(|c| cx.sobrescreve(c, n, setter));
        for &m in membros {
            match &a.member(m).kind {
                MemberKind::Constructor(k) => {
                    if !k.name.is_some_and(|n| cx.privado(n.sym)) && !e_enum {
                        v.push(Dec::Membro(u, m));
                    }
                }
                MemberKind::Field(l) => {
                    for (i, x) in l.variables.iter().enumerate() {
                        if !cx.privado(x.name.sym) && !sobrescreve(x.name.sym, false) {
                            v.push(Dec::Campo(u, m, i));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let f = a.function(*f);
                    let Some(n) = f.name else { continue };
                    if cx.privado(n.sym) {
                        continue;
                    }
                    let texto = cx.interner.resolve(n.sym);
                    let de_teste = texto.starts_with("test_") || texto.starts_with("solo_test_") || texto == "setUp" || texto == "tearDown";
                    if !sobrescreve(n.sym, f.kind == ast::FunctionKind::Setter) && !de_teste {
                        v.push(Dec::Membro(u, m));
                    }
                }
            }
        }
    }
    v
}

/// As regiões (a subárvore) de uma declaração.
fn regioes(a: &Ast, d: Dec) -> Vec<(usize, usize)> {
    let com_anotacoes = |sp: Span, metadata: &[Annotation]| {
        let inicio = metadata.iter().map(|m| m.span.start).min().map_or(sp.start, |x| x.min(sp.start));
        (inicio, sp.end)
    };
    let variavel = |l: &ast::VariableList, i: usize| {
        let x = &l.variables[i];
        let fim = x.initializer.map_or(x.name.span.end, |e| a.expr(e).span.end);
        let mut v = vec![(x.name.span.start, fim)];
        if let Some(t) = l.ty {
            let sp = a.ty(t).span;
            v.push((sp.start, sp.end));
        }
        v
    };
    match d {
        Dec::Topo(_, k) => {
            let x = a.decl(k);
            vec![com_anotacoes(x.span, &x.metadata)]
        }
        Dec::Membro(_, m) => {
            let x = a.member(m);
            vec![com_anotacoes(x.span, &x.metadata)]
        }
        Dec::VarTopo(_, k, i) => match &a.decl(k).kind {
            DeclKind::Variables(l) => variavel(l, i),
            _ => Vec::new(),
        },
        Dec::Campo(_, m, i) => match &a.member(m).kind {
            MemberKind::Field(l) => variavel(l, i),
            _ => Vec::new(),
        },
    }
}

/// As referências posicionais de uma unidade: (offset, ação).
fn referencias(cx: &Cx<'_>, u: UnitId) -> Vec<(usize, Acao)> {
    let program = cx.program;
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let corpo = &cx.s.corpos.units[u.0 as usize];
    let mut out: Vec<(usize, Acao)> = Vec::new();
    // O lado esquerdo das atribuições e os operandos de `++`/`--`: sem
    // `staticElement`, entram pelos elementos de leitura e escrita.
    let mut escritos: HashSet<ExprId> = HashSet::new();
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Assign { target, .. } => {
                escritos.insert(*target);
            }
            ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                escritos.insert(*operand);
            }
            _ => {}
        }
    }
    // Os padrões constantes (`_patternLevel`).
    let mut constantes: Vec<Span> = Vec::new();
    for p in a.patterns.iter() {
        if let PatternKind::Constant(e) = &p.kind {
            constantes.push(a.expr(*e).span);
        }
    }
    let em_padrao = |sp: Span| constantes.iter().any(|c| c.start <= sp.start && sp.end <= c.end);
    // As partes do nome de construtor (`ConstructorName`): o tipo e o nome.
    let mut nome_de_construtor: HashSet<ExprId> = HashSet::new();
    let marcar = |mut x: ExprId, nomes: &mut HashSet<ExprId>| loop {
        nomes.insert(x);
        match &a.expr(x).kind {
            ExprKind::Property { target, .. } | ExprKind::TypeArguments { target, .. } => x = *target,
            _ => break,
        }
    };
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        if !matches!(corpo.get_resolved(id), Some(Resolved::Constructor(_))) {
            continue;
        }
        match &e.kind {
            ExprKind::Call { target, .. } => marcar(*target, &mut nome_de_construtor),
            ExprKind::Property { .. } => marcar(id, &mut nome_de_construtor),
            _ => {}
        }
    }
    let tipo_de_construtor = |f: FunctionElementId, alvo: Option<ExprId>| -> Option<Ref> {
        // O tipo como escrito: o typedef quando o nome é de um typedef.
        let mut x = alvo?;
        loop {
            match &a.expr(x).kind {
                ExprKind::Identifier(_) => break,
                ExprKind::Property { target, .. } | ExprKind::TypeArguments { target, .. } => x = *target,
                _ => return program.function(f).class.map(Ref::Classe),
            }
        }
        match corpo.get_resolved(x) {
            Some(Resolved::Element(Element::Typedef(t))) => Some(Ref::Typedef(*t)),
            Some(Resolved::Element(Element::Class(c))) => Some(Ref::Classe(*c)),
            _ => program.function(f).class.map(Ref::Classe),
        }
    };
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        let pos = e.span.start;
        match &e.kind {
            ExprKind::Identifier(_) | ExprKind::Property { .. } if !escritos.contains(&id) && !nome_de_construtor.contains(&id) => {
                if let Some(r) = cx.de_resolvido(u, corpo, id, pos) {
                    out.push((pos, Acao::Elemento(r)));
                }
            }
            ExprKind::Assign { op, target, .. } => {
                if matches!(a.expr(*target).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                    && let Some(r) = cx.de_resolvido(u, corpo, *target, pos)
                {
                    out.push((pos, Acao::Elemento(r)));
                    if *op != ast::AssignOp::Assign
                        && let Some(l) = cx.leitura_de(r)
                    {
                        out.push((pos, Acao::Elemento(l)));
                    }
                }
            }
            ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                if matches!(a.expr(*operand).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                    && let Some(r) = cx.de_resolvido(u, corpo, *operand, pos)
                {
                    out.push((pos, Acao::Elemento(r)));
                    if let Some(l) = cx.leitura_de(r) {
                        out.push((pos, Acao::Elemento(l)));
                    }
                }
            }
            _ => {}
        }
        // `visitConstructorName`: a criação e o tear-off de construtor.
        if let Some(Resolved::Constructor(f)) = corpo.get_resolved(id)
            && !em_padrao(e.span)
        {
            let alvo = match &e.kind {
                ExprKind::Call { target, .. } => Some(*target),
                ExprKind::Property { .. } => Some(id),
                _ => None,
            };
            out.push((pos, Acao::Elemento(Ref::Funcao(*f))));
            let tipo = match &e.kind {
                ExprKind::InstanceCreation { ty, .. } => cx.elemento_do_tipo(u, corpo, *ty).or_else(|| program.function(*f).class.map(Ref::Classe)),
                _ => tipo_de_construtor(*f, alvo),
            };
            if let Some(t) = tipo {
                out.push((pos, Acao::Elemento(t)));
            }
        }
    }
    // `visitNamedType`: os tipos em lista de argumentos de tipo e os de
    // membro `external`.
    let mut em_argumentos: HashSet<ast::TypeId> = HashSet::new();
    fn marcar_tipo(a: &Ast, t: ast::TypeId, v: &mut HashSet<ast::TypeId>) {
        if !v.insert(t) {
            return;
        }
        match &a.ty(t).kind {
            TypeKind::Named { args, .. } => {
                for x in args.iter() {
                    marcar_tipo(a, *x, v);
                }
            }
            TypeKind::Function { return_type, type_params, parameters } => {
                if let Some(r) = return_type {
                    marcar_tipo(a, *r, v);
                }
                for p in type_params.iter() {
                    if let Some(b) = p.bound {
                        marcar_tipo(a, b, v);
                    }
                }
                fn params(a: &Ast, ps: &[ast::Parameter], v: &mut HashSet<ast::TypeId>) {
                    for p in ps {
                        if let Some(t) = p.ty {
                            marcar_tipo(a, t, v);
                        }
                        if let Some(i) = &p.function_parameters {
                            params(a, i, v);
                        }
                    }
                }
                params(a, parameters, v);
            }
            TypeKind::Record { positional, named } => {
                for x in positional.iter() {
                    marcar_tipo(a, *x, v);
                }
                for (_, x) in named.iter() {
                    marcar_tipo(a, *x, v);
                }
            }
            TypeKind::Void => {}
        }
    }
    let mut raizes: Vec<ast::TypeId> = Vec::new();
    for t in a.types.iter() {
        if let TypeKind::Named { args, .. } = &t.kind {
            raizes.extend(args.iter().copied());
        }
    }
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::List { type_args, .. } | ExprKind::SetOrMap { type_args, .. } | ExprKind::TypeArguments { type_args, .. } => raizes.extend(type_args.iter().copied()),
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => raizes.extend(arguments.type_args.iter().copied()),
            _ => {}
        }
    }
    for p in a.patterns.iter() {
        match &p.kind {
            PatternKind::List { type_args, .. } | PatternKind::Map { type_args, .. } => raizes.extend(type_args.iter().copied()),
            _ => {}
        }
    }
    for m in dartforge_frontend::pais::todas_as_anotacoes(a, &unidade.unit) {
        raizes.extend(m.type_args.iter().copied());
    }
    for d in a.decls.iter() {
        if let DeclKind::Enum(x) = &d.kind {
            for k in x.constants.iter() {
                raizes.extend(k.type_args.iter().copied());
            }
        }
    }
    for t in raizes {
        marcar_tipo(a, t, &mut em_argumentos);
    }
    let mut externos: HashSet<ast::TypeId> = HashSet::new();
    for m in a.members.iter() {
        match &m.kind {
            MemberKind::Method(f) if a.function(*f).external => {
                if let Some(r) = a.function(*f).return_type {
                    marcar_tipo(a, r, &mut externos);
                }
            }
            MemberKind::Field(l) if l.external => {
                if let Some(t) = l.ty {
                    marcar_tipo(a, t, &mut externos);
                }
            }
            _ => {}
        }
    }
    for d in a.decls.iter() {
        if let DeclKind::Variables(l) = &d.kind
            && l.external
            && let Some(t) = l.ty
        {
            marcar_tipo(a, t, &mut externos);
        }
    }
    for (k, t) in a.types.iter().enumerate() {
        let id = ast::TypeId(k as u32);
        if !matches!(t.kind, TypeKind::Named { .. }) {
            continue;
        }
        let Some(r) = cx.elemento_do_tipo(u, corpo, id) else { continue };
        let tipo_de_extensao = matches!(r, Ref::Classe(c) if program.class(c).kind == ClassKind::ExtensionType);
        if matches!(r, Ref::Typedef(_)) || tipo_de_extensao || em_argumentos.contains(&id) || externos.contains(&id) {
            out.push((t.span.start, Acao::Elemento(r)));
        }
    }
    // `visitPatternField`: o getter do campo de padrão de objeto.
    for p in a.patterns.iter() {
        let PatternKind::Object { ty, fields } = &p.kind else { continue };
        let tipo = corpo.tipos_de_anotacoes.get(ty).copied().or_else(|| cx.s.outline.tipos_escritos.get(&(u, *ty)).copied());
        let classe = match tipo.map(|t| cx.s.table.get(t)) {
            Some(Type::Interface { class, .. }) => Some(*class),
            Some(Type::ExtensionType { decl, .. }) => Some(*decl),
            _ => None,
        };
        for f in fields.iter() {
            let nome = f.name.map(|n| n.sym).or_else(|| nome_do_atalho(a, f.pattern));
            let Some(nome) = nome else { continue };
            let getter = classe.and_then(|c| {
                std::iter::once(c)
                    .chain(cx.s.outline.hierarchy.get(c).map(|h| h.supertypes.keys().copied().collect::<Vec<_>>()).unwrap_or_default())
                    .find_map(|x| program.class(x).instance_members.get(&nome).copied())
            });
            if let Some(g) = getter {
                out.push((f.span.start, Acao::Elemento(Ref::Funcao(g))));
            }
        }
    }
    // `visitAnnotation`: o elemento da anotação e os identificadores dela.
    for m in dartforge_frontend::pais::todas_as_anotacoes(a, &unidade.unit) {
        let pos = m.span.start;
        let mut acoes: Vec<Ref> = Vec::new();
        let elemento = |b: Option<dartforge_elements::model::Binding>| b.and_then(|b| b.getter);
        match &m.name[..] {
            [n] => match elemento(program.lookup_na_unidade(u, n.sym)) {
                Some(Element::Class(c)) => {
                    acoes.push(Ref::Classe(c));
                    if let Some(k) = cx.construtor(c, None) {
                        acoes.push(Ref::Funcao(k));
                    }
                }
                Some(Element::Variable(v)) => acoes.push(Ref::Variavel(v)),
                Some(Element::Function(f)) => acoes.push(Ref::Funcao(f)),
                _ => {}
            },
            [p, n] => match elemento(program.lookup_prefixed_na_unidade(u, p.sym, n.sym)) {
                Some(Element::Class(c)) => {
                    acoes.push(Ref::Classe(c));
                    if let Some(k) = cx.construtor(c, None) {
                        acoes.push(Ref::Funcao(k));
                    }
                }
                Some(Element::Variable(v)) => acoes.push(Ref::Variavel(v)),
                Some(Element::Function(f)) => acoes.push(Ref::Funcao(f)),
                _ => {
                    // `Classe.nome`: o construtor nomeado ou o membro estático.
                    if let Some(Element::Class(c)) = elemento(program.lookup_na_unidade(u, p.sym)) {
                        acoes.push(Ref::Classe(c));
                        if let Some(k) = cx.construtor(c, Some(n.sym)) {
                            acoes.push(Ref::Funcao(k));
                        } else if let Some(f) = program.class(c).static_members.get(&n.sym) {
                            acoes.push(Ref::Funcao(*f));
                        }
                    }
                }
            },
            [p, k, n] => {
                if let Some(Element::Class(c)) = elemento(program.lookup_prefixed_na_unidade(u, p.sym, k.sym)) {
                    acoes.push(Ref::Classe(c));
                    if let Some(f) = cx.construtor(c, Some(n.sym)) {
                        acoes.push(Ref::Funcao(f));
                    }
                }
            }
            _ => {}
        }
        for r in acoes {
            out.push((pos, Acao::Elemento(r)));
        }
    }
    // As cláusulas de supertipo, os construtores implícitos de superclasse,
    // os `super(…)`/`this(…)`, os alvos de redirecionamento e o `toJson`.
    for (k, d) in a.decls.iter().enumerate() {
        let id = DeclId(k as u32);
        match &d.kind {
            DeclKind::Class(x) if !x.mixin_application => {
                for t in x.extends.iter().chain(x.with.iter()).chain(x.implements.iter()) {
                    if let Some(r) = cx.elemento_do_tipo(u, corpo, *t) {
                        out.push((a.ty(*t).span.start, Acao::Direto(r)));
                    }
                }
                let Some(classe) = cx.classe_da_decl(u, id) else { continue };
                let superior = program.class(classe).supertype_class;
                let tem_construtores = x.members.iter().any(|m| matches!(a.member(*m).kind, MemberKind::Constructor(_)));
                if !tem_construtores
                    && let Some(s) = superior
                    && let Some(k) = cx.construtor(s, None)
                {
                    out.push((x.name.span.start, Acao::Elemento(Ref::Funcao(k))));
                }
                // `@reflectiveTest` do `test_reflective_loader`.
                if d.metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, cx.interner, u, m, "test_reflective_loader", "reflectiveTest"))
                    && let Some(k) = cx.construtor(classe, None)
                {
                    out.push((x.name.span.start, Acao::Elemento(Ref::Funcao(k))));
                }
                for &m in x.members.iter() {
                    let MemberKind::Constructor(kc) = &a.member(m).kind else { continue };
                    if !kc.initializers.iter().any(|i| matches!(i, Initializer::Super { .. }))
                        && let Some(s) = superior
                        && let Some(k) = cx.construtor(s, None)
                    {
                        out.push((a.member(m).span.start.max(kc.class_name.span.start), Acao::Elemento(Ref::Funcao(k))));
                    }
                }
            }
            DeclKind::ExtensionType(x) => {
                for t in x.implements.iter() {
                    if let Some(r) = cx.elemento_do_tipo(u, corpo, *t) {
                        out.push((a.ty(*t).span.start, Acao::Direto(r)));
                    }
                }
            }
            _ => {}
        }
        let membros: &[MemberId] = match &d.kind {
            DeclKind::Class(x) => &x.members,
            DeclKind::Mixin(x) => &x.members,
            DeclKind::Enum(x) => &x.members,
            DeclKind::Extension(x) => &x.members,
            DeclKind::ExtensionType(x) => &x.members,
            _ => continue,
        };
        let classe = cx.classe_da_decl(u, id);
        for &m in membros {
            match &a.member(m).kind {
                MemberKind::Constructor(kc) => {
                    let superior = classe.and_then(|c| program.class(c).supertype_class);
                    for i in kc.initializers.iter() {
                        match i {
                            Initializer::Super { span, constructor, .. } => {
                                if let Some(f) = superior.and_then(|s| cx.construtor(s, constructor.map(|n| n.sym))) {
                                    out.push((span.start, Acao::Elemento(Ref::Funcao(f))));
                                }
                            }
                            Initializer::Redirect { span, constructor, .. } => {
                                if let Some(f) = classe.and_then(|c| cx.construtor(c, constructor.map(|n| n.sym))) {
                                    out.push((span.start, Acao::Elemento(Ref::Funcao(f))));
                                }
                            }
                            _ => {}
                        }
                    }
                    // O alvo de `factory C() = D.nome;`.
                    if let Some(r) = &kc.redirect
                        && let Some(Ref::Classe(alvo)) = cx.elemento_do_tipo(u, corpo, r.ty)
                    {
                        out.push((r.span.start, Acao::Elemento(Ref::Classe(alvo))));
                        if let Some(f) = cx.construtor(alvo, r.constructor.map(|n| n.sym)) {
                            out.push((r.span.start, Acao::Elemento(Ref::Funcao(f))));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let g = a.function(*f);
                    if let Some(n) = g.name
                        && !g.static_
                        && cx.interner.resolve(n.sym) == "toJson"
                    {
                        // O elemento do próprio método.
                        let fid = (0..program.functions.len()).map(|i| FunctionElementId(i as u32)).find(|x| matches!(program.function(*x).node, FunctionRef::Function { unit, function } if unit == u && function == *f));
                        if let Some(fid) = fid {
                            out.push((n.span.start, Acao::Elemento(Ref::Funcao(fid))));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// O nome do campo de atalho `:x` (a variável do padrão, através de `?`,
/// `!` e do cast).
fn nome_do_atalho(a: &Ast, mut p: ast::PatternId) -> Option<SymbolId> {
    loop {
        match &a.pattern(p).kind {
            PatternKind::Variable { name, .. } => return Some(name.sym),
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Cast { pattern: x, .. } => p = *x,
            _ => return None,
        }
    }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Some(s) = sem else { return out };
    if !ligada("unreachable_from_main") {
        return out;
    }
    let program = s.program;
    let biblioteca = program.unit(s.unidade).library;
    let unidades: Vec<UnitId> = program.library(biblioteca).units.clone();
    let mut cx = Cx { program, interner, s, declaracoes: HashSet::new(), topos: HashMap::new() };
    let mut todas: Vec<Dec> = Vec::new();
    for &x in unidades.iter() {
        todas.extend(declaracoes_da_unidade(&cx, x));
    }
    cx.declaracoes = todas.iter().copied().collect();
    for &d in todas.iter() {
        if let Dec::Topo(x, _) | Dec::VarTopo(x, ..) = d {
            let a = &program.unit(x).ast;
            for (i, f) in regioes(a, d) {
                cx.topos.entry(x).or_default().push((i, f, d));
            }
        }
    }
    // Os pontos de entrada.
    let pragma_de_entrada = |x: UnitId, m: &Annotation| -> bool {
        let Some(n) = m.name.last() else { return false };
        if interner.resolve(n.sym) != "pragma" {
            return false;
        }
        let b = match &m.name[..] {
            [n] => program.lookup_na_unidade(x, n.sym),
            [p, n] => program.lookup_prefixed_na_unidade(x, p.sym, n.sym),
            _ => None,
        };
        let Some(Element::Class(c)) = b.and_then(|b| b.getter) else { return false };
        if program.library(program.class(c).library).uri != "dart:core" {
            return false;
        }
        let Some(args) = &m.arguments else { return false };
        let a = &program.unit(x).ast;
        args.args.iter().find(|y| y.name.is_none()).is_some_and(|y| matches!(&a.expr(y.value).kind, ExprKind::String(sl) if sl.constant_value().is_some_and(|v| v == *"vm:entry-point")))
    };
    let entradas: Vec<Dec> = todas
        .iter()
        .copied()
        .filter(|d| {
            let Dec::Topo(x, k) = *d else { return false };
            let a = &program.unit(x).ast;
            let decl = a.decl(k);
            let DeclKind::Function(f) = &decl.kind else { return false };
            a.function(*f).name.is_some_and(|n| interner.resolve(n.sym) == "main") || decl.metadata.iter().any(|m| pragma_de_entrada(x, m))
        })
        .collect();
    if entradas.is_empty() {
        return out;
    }
    // As dependências de cada declaração, pela varredura das regiões
    // aninhadas de cada unidade.
    let mut dependencias: HashMap<Dec, HashSet<Dec>> = todas.iter().map(|d| (*d, HashSet::new())).collect();
    for &x in unidades.iter() {
        let a = &program.unit(x).ast;
        let mut intervalos: Vec<(usize, usize, Dec)> = Vec::new();
        for &d in todas.iter().filter(|d| d.unidade() == x) {
            for (i, f) in regioes(a, d) {
                intervalos.push((i, f, d));
            }
        }
        intervalos.sort_by(|p, q| p.0.cmp(&q.0).then(q.1.cmp(&p.1)));
        let mut itens = referencias(&cx, x);
        itens.sort_by_key(|(p, _)| *p);
        let mut pilha: Vec<(usize, usize, Dec)> = Vec::new();
        let mut proximo = 0;
        for (p, acao) in itens {
            while proximo < intervalos.len() && intervalos[proximo].0 <= p {
                let iv = intervalos[proximo];
                while pilha.last().is_some_and(|t| t.1 <= iv.0) {
                    pilha.pop();
                }
                pilha.push(iv);
                proximo += 1;
            }
            // As regiões aninham, mas um intervalo de dentro pode ter
            // terminado antes de um de fora: filtra pelos que contêm `p`.
            let mut alvo: HashSet<Dec> = HashSet::new();
            cx.agir(acao, &mut alvo);
            if alvo.is_empty() {
                continue;
            }
            for iv in pilha.iter().filter(|iv| iv.0 <= p && p < iv.1) {
                if let Some(dep) = dependencias.get_mut(&iv.2) {
                    dep.extend(alvo.iter().copied());
                }
            }
        }
    }
    let mut usados: HashSet<Dec> = entradas.iter().copied().collect();
    let mut fila: Vec<Dec> = entradas;
    while let Some(d) = fila.pop() {
        if let Some(deps) = dependencias.get(&d) {
            for &x in deps {
                if usados.insert(x) {
                    fila.push(x);
                }
            }
        }
    }
    // Os não alcançados da unidade, públicos e sem `@visibleForTesting`.
    let a = u.ast;
    let visivel_para_teste = |metadata: &[Annotation]| metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unidade, m, "meta", "visibleForTesting"));
    let mut achados: Vec<(Span, String)> = Vec::new();
    for d in declaracoes_da_unidade(&cx, s.unidade) {
        if usados.contains(&d) {
            continue;
        }
        match d {
            Dec::Topo(_, k) => {
                let decl = a.decl(k);
                if visivel_para_teste(&decl.metadata) {
                    continue;
                }
                let nome = match &decl.kind {
                    DeclKind::Class(x) => Some(x.name),
                    DeclKind::Mixin(x) => Some(x.name),
                    DeclKind::Enum(x) => Some(x.name),
                    DeclKind::ExtensionType(x) => Some(x.name),
                    DeclKind::Typedef(x) => Some(x.name),
                    DeclKind::Function(f) => a.function(*f).name,
                    DeclKind::Extension(x) => match x.name {
                        Some(n) => Some(n),
                        // A extensão sem nome é privada.
                        None => continue,
                    },
                    DeclKind::Variables(_) => None,
                };
                let Some(n) = nome else { continue };
                if cx.privado(n.sym) {
                    continue;
                }
                achados.push((n.span, interner.resolve(n.sym).to_string()));
            }
            Dec::VarTopo(_, k, i) => {
                let decl = a.decl(k);
                let DeclKind::Variables(l) = &decl.kind else { continue };
                let n = l.variables[i].name;
                if cx.privado(n.sym) || visivel_para_teste(&decl.metadata) {
                    continue;
                }
                achados.push((n.span, interner.resolve(n.sym).to_string()));
            }
            Dec::Membro(_, m) => {
                let membro = a.member(m);
                if visivel_para_teste(&membro.metadata) {
                    continue;
                }
                match &membro.kind {
                    MemberKind::Constructor(k) => {
                        let classe = interner.resolve(k.class_name.sym);
                        match k.name {
                            Some(n) => achados.push((n.span, format!("{classe}.{}", interner.resolve(n.sym)))),
                            None => achados.push((k.class_name.span, format!("{classe}.new"))),
                        }
                    }
                    MemberKind::Method(f) => {
                        let Some(n) = a.function(*f).name else { continue };
                        achados.push((n.span, interner.resolve(n.sym).to_string()));
                    }
                    _ => {}
                }
            }
            Dec::Campo(_, m, i) => {
                let membro = a.member(m);
                let MemberKind::Field(l) = &membro.kind else { continue };
                if visivel_para_teste(&membro.metadata) {
                    continue;
                }
                let n = l.variables[i].name;
                achados.push((n.span, interner.resolve(n.sym).to_string()));
            }
        }
    }
    achados.sort_by_key(|(sp, _)| (sp.start, sp.end));
    for (sp, nome) in achados {
        out.push(RelatoDeLint { codigo: &c::UNREACHABLE_FROM_MAIN, span: sp, args: vec![nome] });
    }
    out
}
