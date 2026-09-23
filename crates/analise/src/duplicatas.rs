//! Nomes definidos mais de uma vez: o `DuplicateDefinitionVerifier` e o
//! `MemberDuplicateDefinitionVerifier` do analyzer 6.11.0
//! (`src/error/duplicate_definition_verifier.dart`), chamados pelo
//! `ErrorVerifier` em `visitBlock`, `visitSwitchCase`,
//! `visitForPartsWithDeclarations`, `visitFormalParameterList`,
//! `visitTypeParameterList`, `visitCatchClause` e `visitCompilationUnit`, e
//! pelo `LibraryAnalyzer` para os membros de cada tipo.
//!
//! O modelo é o do analyzer: cada nome entra num escopo de getters (e, para
//! setters, num de setters); um campo ou variável de topo induz um getter e,
//! se não é `final` nem `const`, um setter; getter e setter do mesmo nome
//! formam par e não colidem. O erro vai no nome da **segunda** declaração.

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::{Codigo, Diagnostic, Span};
use dartforge_frontend::ast::{
    self, Ast, DeclKind, DirectiveKind, ForInit, FunctionKind, ListPatternElement, MemberId, MemberKind,
    Parameter, PatternId, PatternKind, StmtKind, TypeKind, TypeParameter, TypedefKind,
};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tipo {
    Getter,
    Setter,
    Metodo,
    Outro,
}

/// O que o analyzer sabe do elemento anterior para escolher o código.
#[derive(Clone, Copy)]
struct Elem {
    tipo: Tipo,
    /// Acessor sintético de um campo (`CONFLICTING_CONSTRUCTOR_AND_STATIC_FIELD`).
    de_campo: bool,
    /// Parâmetro `this.x` (`DUPLICATE_FIELD_FORMAL_PARAMETER`).
    formal_campo: bool,
}

const OUTRO: Elem = Elem { tipo: Tipo::Outro, de_campo: false, formal_campo: false };

type Escopo = HashMap<String, Elem>;

struct Relato<'a> {
    interner: &'a Interner,
    unidade: usize,
    out: Vec<(usize, Diagnostic)>,
}

impl Relato<'_> {
    fn nome(&self, n: ast::Name) -> String {
        self.interner.resolve(n.sym).to_string()
    }

    fn por(&mut self, codigo: Codigo, span: Span, args: &[&str]) {
        self.out.push((self.unidade, Diagnostic::com_codigo(codigo, span, args.iter().copied())));
    }

    /// `_checkDuplicateIdentifier` para um elemento que não induz propriedade.
    fn conferir(&mut self, getters: &mut Escopo, setters: Option<&mut Escopo>, nome: &str, span: Span, e: Elem) {
        // `identifier.isSynthetic`: a recuperação do parser não gera nome vazio
        // de verdade; um intervalo vazio é o equivalente aqui.
        if span.start == span.end {
            return;
        }
        match getters.get(nome).copied() {
            Some(anterior) => {
                let par = matches!((anterior.tipo, e.tipo), (Tipo::Getter, Tipo::Setter) | (Tipo::Setter, Tipo::Getter));
                if !par {
                    let codigo = if anterior.formal_campo && e.formal_campo {
                        c::DUPLICATE_FIELD_FORMAL_PARAMETER
                    } else {
                        c::DUPLICATE_DEFINITION
                    };
                    self.por(codigo, span, &[nome]);
                }
            }
            None => {
                getters.insert(nome.to_string(), e);
            }
        }
        if let Some(setters) = setters {
            if e.tipo == Tipo::Setter {
                if setters.contains_key(nome) {
                    self.por(c::DUPLICATE_DEFINITION, span, &[nome]);
                } else {
                    setters.insert(nome.to_string(), e);
                }
            }
        }
    }

    /// Campo ou variável de topo: o getter e, se mutável, o setter.
    fn conferir_propriedade(
        &mut self,
        getters: &mut Escopo,
        mut setters: Option<&mut Escopo>,
        nome: &str,
        span: Span,
        mutavel: bool,
    ) {
        let g = Elem { tipo: Tipo::Getter, de_campo: true, formal_campo: false };
        self.conferir(getters, setters.as_deref_mut(), nome, span, g);
        if mutavel {
            let s = Elem { tipo: Tipo::Setter, de_campo: true, formal_campo: false };
            self.conferir(getters, setters, nome, span, s);
        }
    }
}

fn tipo_da_funcao(k: FunctionKind) -> Tipo {
    match k {
        FunctionKind::Getter => Tipo::Getter,
        FunctionKind::Setter => Tipo::Setter,
        FunctionKind::Function | FunctionKind::Operator => Tipo::Metodo,
    }
}

/// Todos os diagnósticos de nomes duplicados de uma biblioteca, com o índice
/// da unidade em que cada um cai. `curinga`: a biblioteca tem
/// `wildcard-variables` (3.7+), e `_` local não declara nada.
pub fn duplicatas(unidades: &[Unidade<'_>], interner: &Interner, curinga: bool) -> Vec<(usize, Diagnostic)> {
    let mut rel = Relato { interner, unidade: 0, out: Vec::new() };
    let nomes_da_biblioteca = nomes_de_topo(unidades, interner);
    for (i, u) in unidades.iter().enumerate() {
        rel.unidade = i;
        unidade_de_topo(&mut rel, unidades, i, &nomes_da_biblioteca);
        locais(&mut rel, u.ast, curinga);
    }
    // Membros: primeiro os de instância de todas as unidades, depois os
    // estáticos (`MemberDuplicateDefinitionVerifier.checkLibrary`).
    let mut contextos: Vec<Vec<Contexto>> = Vec::new();
    for (i, u) in unidades.iter().enumerate() {
        rel.unidade = i;
        contextos.push(membros_da_unidade(&mut rel, u));
    }
    for (i, u) in unidades.iter().enumerate() {
        rel.unidade = i;
        estaticos_da_unidade(&mut rel, u, &contextos[i]);
    }
    rel.out
}

/// Nomes declarados no topo da biblioteca (`libraryDeclarations`).
fn nomes_de_topo(unidades: &[Unidade<'_>], interner: &Interner) -> HashSet<String> {
    let mut s = HashSet::new();
    for u in unidades {
        for &d in &u.unit.declarations {
            let nome = |n: ast::Name| interner.resolve(n.sym).to_string();
            match &u.ast.decl(d).kind {
                DeclKind::Class(x) => {
                    s.insert(nome(x.name));
                }
                DeclKind::Mixin(x) => {
                    s.insert(nome(x.name));
                }
                DeclKind::Enum(x) => {
                    s.insert(nome(x.name));
                }
                DeclKind::Extension(x) => {
                    if let Some(n) = x.name {
                        s.insert(nome(n));
                    }
                }
                DeclKind::ExtensionType(x) => {
                    s.insert(nome(x.name));
                }
                DeclKind::Typedef(x) => {
                    s.insert(nome(x.name));
                }
                DeclKind::Function(f) => {
                    if let Some(n) = u.ast.function(*f).name {
                        s.insert(nome(n));
                    }
                }
                DeclKind::Variables(vl) => {
                    for v in vl.variables.iter() {
                        s.insert(nome(v.name));
                    }
                }
            }
        }
    }
    s
}

/// `addWithoutChecking`: o que as unidades anteriores já declararam.
fn sem_conferir(rel: &Relato<'_>, u: &Unidade<'_>, getters: &mut Escopo) {
    let mut variaveis = Vec::new();
    for &d in &u.unit.declarations {
        match &u.ast.decl(d).kind {
            DeclKind::Function(f) => {
                let f = u.ast.function(*f);
                let Some(n) = f.name else { continue };
                let nome = rel.nome(n);
                match f.kind {
                    FunctionKind::Getter => {
                        getters.insert(nome, Elem { tipo: Tipo::Getter, ..OUTRO });
                    }
                    FunctionKind::Setter => {
                        getters.insert(format!("{nome}="), Elem { tipo: Tipo::Setter, ..OUTRO });
                    }
                    _ => {
                        getters.insert(nome, OUTRO);
                    }
                }
            }
            DeclKind::Variables(vl) => variaveis.push(vl),
            DeclKind::Class(x) => {
                getters.insert(rel.nome(x.name), OUTRO);
            }
            DeclKind::Mixin(x) => {
                getters.insert(rel.nome(x.name), OUTRO);
            }
            DeclKind::Enum(x) => {
                getters.insert(rel.nome(x.name), OUTRO);
            }
            DeclKind::Extension(x) => {
                if let Some(n) = x.name {
                    getters.insert(rel.nome(n), OUTRO);
                }
            }
            DeclKind::ExtensionType(x) => {
                getters.insert(rel.nome(x.name), OUTRO);
            }
            DeclKind::Typedef(x) => {
                getters.insert(rel.nome(x.name), OUTRO);
            }
        }
    }
    // As variáveis sobrescrevem os acessores sintéticos, como no analyzer.
    for vl in variaveis {
        for v in vl.variables.iter() {
            let nome = rel.nome(v.name);
            if !vl.final_ && !vl.const_ {
                getters.insert(format!("{nome}="), OUTRO);
            }
            getters.insert(nome, OUTRO);
        }
    }
}

/// `checkUnit`: declarações de topo e prefixos de import.
fn unidade_de_topo(rel: &mut Relato<'_>, unidades: &[Unidade<'_>], i: usize, nomes: &HashSet<String>) {
    let u = unidades[i];
    // PREFIX_COLLIDES_WITH_TOP_LEVEL_MEMBER, uma vez por prefixo.
    let mut prefixos = HashSet::new();
    for d in &u.unit.directives {
        if let DirectiveKind::Import { prefix: Some(p), .. } = &d.kind {
            let nome = rel.nome(*p);
            if prefixos.insert(nome.clone()) && nomes.contains(&nome) {
                rel.por(c::PREFIX_COLLIDES_WITH_TOP_LEVEL_MEMBER, p.span, &[&nome]);
            }
        }
    }
    let mut getters = Escopo::new();
    let mut setters = Escopo::new();
    if i > 0 {
        for anterior in &unidades[..i] {
            sem_conferir(rel, anterior, &mut getters);
        }
    }
    for &d in &u.unit.declarations {
        let (nome, e) = match &u.ast.decl(d).kind {
            DeclKind::Class(x) => (x.name, OUTRO),
            DeclKind::Mixin(x) => (x.name, OUTRO),
            DeclKind::Enum(x) => (x.name, OUTRO),
            DeclKind::ExtensionType(x) => (x.name, OUTRO),
            DeclKind::Typedef(x) => (x.name, OUTRO),
            DeclKind::Extension(x) => match x.name {
                Some(n) => (n, OUTRO),
                None => continue,
            },
            DeclKind::Function(f) => {
                let f = u.ast.function(*f);
                let Some(n) = f.name else { continue };
                let tipo = match f.kind {
                    FunctionKind::Getter => Tipo::Getter,
                    FunctionKind::Setter => Tipo::Setter,
                    _ => Tipo::Outro,
                };
                (n, Elem { tipo, ..OUTRO })
            }
            DeclKind::Variables(vl) => {
                for v in vl.variables.iter() {
                    let nome = rel.nome(v.name);
                    rel.conferir_propriedade(&mut getters, Some(&mut setters), &nome, v.name.span, !vl.final_ && !vl.const_);
                }
                continue;
            }
        };
        let texto = rel.nome(nome);
        rel.conferir(&mut getters, Some(&mut setters), &texto, nome.span, e);
    }
}

// ---------------------------------------------------------------------------
// Corpos: blocos, `for`, parâmetros, parâmetros de tipo, `catch`.

fn e_curinga(curinga: bool, nome: &str) -> bool {
    curinga && nome == "_"
}

fn locais(rel: &mut Relato<'_>, ast: &Ast, curinga: bool) {
    for s in &ast.stmts {
        match &s.kind {
            StmtKind::Block(lista) => comandos(rel, ast, lista, curinga),
            StmtKind::Switch { cases, .. } => {
                for caso in cases.iter() {
                    comandos(rel, ast, &caso.body, curinga);
                }
            }
            StmtKind::For { init: Some(ForInit::Variables(vl)), .. } => {
                let mut escopo = Escopo::new();
                for v in vl.variables.iter() {
                    let nome = rel.nome(v.name);
                    if !e_curinga(curinga, &nome) {
                        rel.conferir(&mut escopo, None, &nome, v.name.span, OUTRO);
                    }
                }
            }
            StmtKind::Try { catches, .. } => {
                for cc in catches.iter() {
                    if let (Some(e), Some(st)) = (cc.exception, cc.stack_trace) {
                        let nome = rel.nome(e);
                        if !e_curinga(curinga, &nome) && nome == rel.nome(st) {
                            rel.por(c::DUPLICATE_DEFINITION, st.span, &[&nome]);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Listas de parâmetros e de parâmetros de tipo.
    for f in &ast.functions {
        parametros_de_tipo(rel, &f.type_params, curinga);
        if let Some(ps) = &f.parameters {
            parametros(rel, ps, curinga);
        }
    }
    for m in &ast.members {
        if let MemberKind::Constructor(k) = &m.kind {
            parametros(rel, &k.parameters, curinga);
        }
    }
    for d in &ast.decls {
        match &d.kind {
            DeclKind::Class(x) => parametros_de_tipo(rel, &x.type_params, curinga),
            DeclKind::Mixin(x) => parametros_de_tipo(rel, &x.type_params, curinga),
            DeclKind::Enum(x) => parametros_de_tipo(rel, &x.type_params, curinga),
            DeclKind::Extension(x) => parametros_de_tipo(rel, &x.type_params, curinga),
            DeclKind::ExtensionType(x) => parametros_de_tipo(rel, &x.type_params, curinga),
            DeclKind::Typedef(x) => {
                parametros_de_tipo(rel, &x.type_params, curinga);
                if let TypedefKind::Legacy { parameters, .. } = &x.kind {
                    parametros(rel, parameters, curinga);
                }
            }
            _ => {}
        }
    }
    for t in &ast.types {
        if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
            parametros_de_tipo(rel, type_params, curinga);
            parametros(rel, parameters, curinga);
        }
    }
}

/// `checkStatements`: variáveis, funções locais e variáveis de padrão da mesma lista.
fn comandos(rel: &mut Relato<'_>, ast: &Ast, lista: &[ast::StmtId], curinga: bool) {
    let mut escopo = Escopo::new();
    for &id in lista {
        match &ast.stmt(id).kind {
            StmtKind::Variables(vl) => {
                for v in vl.variables.iter() {
                    let nome = rel.nome(v.name);
                    if !e_curinga(curinga, &nome) {
                        rel.conferir(&mut escopo, None, &nome, v.name.span, OUTRO);
                    }
                }
            }
            StmtKind::Function(f) => {
                if let Some(n) = ast.function(*f).name {
                    let nome = rel.nome(n);
                    if !e_curinga(curinga, &nome) {
                        rel.conferir(&mut escopo, None, &nome, n.span, OUTRO);
                    }
                }
            }
            StmtKind::PatternVariables { pattern, .. } => {
                let mut nomes = Vec::new();
                variaveis_do_padrao(ast, *pattern, &mut nomes);
                for n in nomes {
                    let nome = rel.nome(n);
                    if !e_curinga(curinga, &nome) {
                        rel.conferir(&mut escopo, None, &nome, n.span, OUTRO);
                    }
                }
            }
            _ => {}
        }
    }
}

fn variaveis_do_padrao(ast: &Ast, p: PatternId, out: &mut Vec<ast::Name>) {
    match &ast.pattern(p).kind {
        PatternKind::Variable { name, .. } => out.push(*name),
        PatternKind::Or(a, b) | PatternKind::And(a, b) => {
            variaveis_do_padrao(ast, *a, out);
            variaveis_do_padrao(ast, *b, out);
        }
        PatternKind::NullCheck(a) | PatternKind::NullAssert(a) | PatternKind::Parenthesized(a) => {
            variaveis_do_padrao(ast, *a, out)
        }
        PatternKind::Cast { pattern, .. } => variaveis_do_padrao(ast, *pattern, out),
        PatternKind::List { elements, .. } => {
            for e in elements.iter() {
                match e {
                    ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) => variaveis_do_padrao(ast, *x, out),
                    ListPatternElement::Rest(None) => {}
                }
            }
        }
        PatternKind::Map { entries, .. } => {
            for e in entries.iter() {
                variaveis_do_padrao(ast, e.value, out);
            }
        }
        PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
            for f in fields.iter() {
                variaveis_do_padrao(ast, f.pattern, out);
            }
        }
        PatternKind::Wildcard { .. } | PatternKind::Constant(_) | PatternKind::Relational { .. } => {}
    }
}

/// `checkParameters`, e o mesmo nas listas dos parâmetros-função da forma antiga.
fn parametros(rel: &mut Relato<'_>, ps: &[Parameter], curinga: bool) {
    let mut escopo = Escopo::new();
    for p in ps {
        if let Some(n) = p.name {
            let nome = rel.nome(n);
            // `super._` com curinga; e `_` comum, que não declara nada.
            if !(e_curinga(curinga, &nome) && !p.this_) {
                let e = Elem { formal_campo: p.this_, ..OUTRO };
                rel.conferir(&mut escopo, None, &nome, n.span, e);
            }
        }
        parametros_de_tipo(rel, &p.function_type_params, curinga);
        if let Some(fp) = &p.function_parameters {
            parametros(rel, fp, curinga);
        }
    }
}

fn parametros_de_tipo(rel: &mut Relato<'_>, tps: &[TypeParameter], curinga: bool) {
    let mut escopo = Escopo::new();
    for t in tps {
        let nome = rel.nome(t.name);
        if !e_curinga(curinga, &nome) {
            rel.conferir(&mut escopo, None, &nome, t.name.span, OUTRO);
        }
    }
}

// ---------------------------------------------------------------------------
// Membros de classes, mixins, enums, extensions e extension types.

#[derive(Default)]
struct Contexto {
    /// Nome da declaração (`None` em extension anônima).
    nome: Option<String>,
    especie: Especie,
    construtores: HashSet<String>,
    ig: Escopo,
    is: Escopo,
    sg: Escopo,
    ss: Escopo,
    membros: Vec<MemberId>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Especie {
    #[default]
    Classe,
    Enum,
    Extension,
}

fn membros_da_unidade(rel: &mut Relato<'_>, u: &Unidade<'_>) -> Vec<Contexto> {
    let mut v = Vec::new();
    for &d in &u.unit.declarations {
        let mut ctx = Contexto::default();
        match &u.ast.decl(d).kind {
            DeclKind::Class(x) => {
                ctx.nome = Some(rel.nome(x.name));
                ctx.membros = x.members.clone();
            }
            DeclKind::Mixin(x) => {
                ctx.nome = Some(rel.nome(x.name));
                ctx.membros = x.members.clone();
            }
            DeclKind::Extension(x) => {
                ctx.nome = x.name.map(|n| rel.nome(n));
                ctx.especie = Especie::Extension;
                ctx.membros = x.members.clone();
            }
            DeclKind::ExtensionType(x) => {
                ctx.nome = Some(rel.nome(x.name));
                ctx.construtores.insert(x.constructor.map(|n| rel.nome(n)).unwrap_or_default());
                ctx.ig.insert(rel.nome(x.representation_name), Elem { tipo: Tipo::Getter, de_campo: true, formal_campo: false });
                ctx.membros = x.members.clone();
            }
            DeclKind::Enum(x) => {
                let nome = rel.nome(x.name);
                ctx.especie = Especie::Enum;
                for k in &x.constants {
                    let n = rel.nome(k.name);
                    if n == nome {
                        rel.por(c::ENUM_CONSTANT_SAME_NAME_AS_ENCLOSING, k.name.span, &[]);
                    }
                    let e = Elem { tipo: Tipo::Getter, de_campo: true, formal_campo: false };
                    rel.conferir(&mut ctx.sg, None, &n, k.name.span, e);
                    if n == "values" {
                        rel.por(c::VALUES_DECLARATION_IN_ENUM, k.name.span, &[]);
                    }
                }
                ctx.nome = Some(nome);
                ctx.membros = x.members.clone();
            }
            _ => continue,
        }
        membros(rel, u.ast, &mut ctx);
        if let DeclKind::Enum(x) = &u.ast.decl(d).kind {
            if ctx.nome.as_deref() == Some("values") {
                rel.por(c::ENUM_WITH_NAME_VALUES, x.name.span, &[]);
            }
        }
        v.push(ctx);
    }
    v
}

/// `_checkClassMembers`.
fn membros(rel: &mut Relato<'_>, ast: &Ast, ctx: &mut Contexto) {
    let e_enum = ctx.especie == Especie::Enum;
    let mut nomeados: Vec<(String, Span)> = Vec::new();
    for &m in &ctx.membros {
        match &ast.member(m).kind {
            MemberKind::Constructor(k) => {
                if k.parte_primaria || ctx.nome.as_deref() != Some(rel.nome(k.class_name).as_str()) {
                    continue;
                }
                let mut nome = k.name.map(|n| rel.nome(n)).unwrap_or_default();
                if nome == "new" {
                    nome.clear();
                }
                if let Some(n) = k.name {
                    if !nome.is_empty() {
                        nomeados.push((nome.clone(), n.span));
                    }
                }
                if !ctx.construtores.insert(nome.clone()) {
                    // `atConstructorDeclaration`: do nome da classe ao fim do nome.
                    let fim = k.name.unwrap_or(k.class_name).span.end;
                    let span = Span { start: k.class_name.span.start, end: fim };
                    if nome.is_empty() {
                        rel.por(c::DUPLICATE_CONSTRUCTOR_DEFAULT, span, &[]);
                    } else {
                        rel.por(c::DUPLICATE_CONSTRUCTOR_NAME, span, &[&nome]);
                    }
                }
            }
            MemberKind::Field(vl) => {
                for v in vl.variables.iter() {
                    let nome = rel.nome(v.name);
                    let (g, s) = if vl.static_ { (&mut ctx.sg, &mut ctx.ss) } else { (&mut ctx.ig, &mut ctx.is) };
                    rel.conferir_propriedade(g, Some(s), &nome, v.name.span, !vl.final_ && !vl.const_);
                    if e_enum && nome == "values" {
                        rel.por(c::VALUES_DECLARATION_IN_ENUM, v.name.span, &[]);
                    }
                }
            }
            MemberKind::Method(f) => {
                let f = ast.function(*f);
                let Some(n) = f.name else { continue };
                let nome = rel.nome(n);
                let tipo = tipo_da_funcao(f.kind);
                let (g, s) = if f.static_ { (&mut ctx.sg, &mut ctx.ss) } else { (&mut ctx.ig, &mut ctx.is) };
                rel.conferir(g, Some(s), &nome, n.span, Elem { tipo, ..OUTRO });
                if e_enum && nome == "values" && !(f.static_ && tipo == Tipo::Setter) {
                    rel.por(c::VALUES_DECLARATION_IN_ENUM, n.span, &[]);
                }
            }
        }
    }
    // `_checkConflictingConstructorAndStatic` (tipos com interface).
    if ctx.especie != Especie::Extension {
        for (nome, span) in nomeados {
            let estatico = ctx.sg.get(&nome).or_else(|| ctx.ss.get(&nome)).copied();
            let codigo = match estatico {
                Some(e) if e.de_campo => Some(c::CONFLICTING_CONSTRUCTOR_AND_STATIC_FIELD),
                Some(Elem { tipo: Tipo::Getter, .. }) => Some(c::CONFLICTING_CONSTRUCTOR_AND_STATIC_GETTER),
                Some(Elem { tipo: Tipo::Setter, .. }) => Some(c::CONFLICTING_CONSTRUCTOR_AND_STATIC_SETTER),
                Some(Elem { tipo: Tipo::Metodo, .. }) => Some(c::CONFLICTING_CONSTRUCTOR_AND_STATIC_METHOD),
                _ => None,
            };
            if let Some(codigo) = codigo {
                rel.por(codigo, span, &[&nome]);
            }
        }
    }
}

/// `_checkClassStatic` / `_checkExtensionStatic`: estático com o nome de um
/// membro de instância declarado ali. (O do enum consulta a interface
/// herdada e fica para quando houver o gerente de herança.)
fn estaticos_da_unidade(rel: &mut Relato<'_>, u: &Unidade<'_>, contextos: &[Contexto]) {
    for ctx in contextos {
        if ctx.especie == Especie::Enum {
            continue;
        }
        for &m in &ctx.membros {
            let nomes: Vec<ast::Name> = match &u.ast.member(m).kind {
                MemberKind::Field(vl) if vl.static_ => vl.variables.iter().map(|v| v.name).collect(),
                MemberKind::Method(f) if u.ast.function(*f).static_ => u.ast.function(*f).name.into_iter().collect(),
                _ => continue,
            };
            for n in nomes {
                let nome = rel.nome(n);
                if ctx.ig.contains_key(&nome) || ctx.is.contains_key(&nome) {
                    match (ctx.especie, &ctx.nome) {
                        (Especie::Extension, _) => rel.por(c::EXTENSION_CONFLICTING_STATIC_AND_INSTANCE, n.span, &[&nome]),
                        (_, Some(classe)) => {
                            let classe = classe.clone();
                            rel.por(c::CONFLICTING_STATIC_AND_INSTANCE, n.span, &[&classe, &nome, &classe]);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn rodar(fonte: &str) -> Vec<(String, usize, usize)> {
        let mut interner = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut interner);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        let mut v: Vec<_> = duplicatas(&[u], &interner, false)
            .into_iter()
            .map(|(_, d)| (d.code.unwrap().info().nome.to_string(), d.span.start, d.span.end))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn topo_membros_e_locais() {
        let f = "int a = 1;\nvoid a() {}\nint get g => 1;\nset g(int v) {}\nclass C {\n  int x = 0;\n  void x() {}\n  C();\n  C();\n  static int y = 0;\n  int get y => 1;\n}\nvoid m(int p, int p) {\n  var q = 1;\n  var q = 2;\n  try {} catch (e, e) {}\n}\n";
        let r = rodar(f);
        let codigos: Vec<&str> = r.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(
            codigos.iter().filter(|c| **c == "duplicate_definition").count(),
            5,
            "{r:?}"
        );
        assert!(codigos.contains(&"duplicate_constructor"), "{r:?}");
        assert!(codigos.contains(&"conflicting_static_and_instance"), "{r:?}");
        // o erro cai na segunda declaração: `void a()` (offset 16)
        assert!(r.contains(&("duplicate_definition".to_string(), 16, 17)), "{r:?}");
    }

    #[test]
    fn getter_e_setter_nao_colidem() {
        assert!(rodar("int get g => 1;\nset g(int v) {}\nclass C { int get x => 1; set x(int v) {} }\n").is_empty());
        let r = rodar("class C { int x = 0; set x(int v) {} }\n");
        assert_eq!(r.len(), 1, "{r:?}");
    }
}
