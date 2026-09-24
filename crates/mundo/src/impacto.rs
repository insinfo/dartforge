//! O impacto de um elemento: o que o corpo dele usa.
//!
//! É o análogo do `ImpactBuilder` do dart2js (`ir/impact.dart`,
//! `kernel/kernel_impact.dart`), sobre a nossa AST e os `BodyTypes`. O
//! percurso é **exaustivo**: todo `match` sobre `ExprKind`, `StmtKind`,
//! `PatternKind` e `CollectionElement` enumera as variantes sem braço `_`,
//! para que uma variante nova da AST quebre a compilação daqui em vez de ser
//! esquecida em silêncio — um nome esquecido seria código podado com alguém
//! ainda chamando.
//!
//! Três fontes, unidas (nunca uma no lugar da outra):
//!
//! 1. todo **nome** de membro escrito (`x.nome`, `nome` solto, seção de
//!    cascata, campo de padrão de objeto) vira seletor, qualquer que seja a
//!    resolução — é o que torna o mundo independente da inferência —, com o
//!    **cone do receptor** quando o tipo estático dele é conhecido (`x: T`
//!    mantém o membro só nas classes subtipo de `T`; `nome` solto usa a
//!    classe envolvente; receptor dinâmico/desconhecido continua irrestrito);
//! 2. o `Resolved` dos `BodyTypes`, para os usos estáticos;
//! 3. a resolução do nome no escopo (`Program::lookup`), para quando o
//!    `Resolved` falta ou diverge do que o emissor resolveria.
//!
//! Tipos: as anotações escritas e o **tipo estático de toda expressão**
//! percorrida (cobre os argumentos de tipo inferidos que o emissor escreve
//! nas receitas rti).

use crate::Motor;
use dartforge_elements::model::{ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, FunctionRef, LibraryId, UnitId, VariableId};
use dartforge_frontend::ast::{self, AssignOp, CollectionElement, ExprId, ExprKind, ForInTarget, ForInit, FunctionBody, Initializer, PatternId, PatternKind, StmtId, StmtKind, StringPart, TypeKind, UnaryOp};
use dartforge_types::resolved::{MemberRef, Resolved};

/// Onde o corpo sendo percorrido está.
#[derive(Clone, Copy)]
pub(crate) struct Contexto {
    pub unidade: UnitId,
    pub biblioteca: LibraryId,
    pub classe: Option<ClassId>,
    pub extensao: Option<ExtensionId>,
}

/// O que um alvo de `.` denota estaticamente.
enum Alvo {
    Classe(ClassId),
    Extensao(ExtensionId),
    Prefixo(dartforge_intern::SymbolId),
}

pub(crate) fn de_funcao(m: &mut Motor<'_>, f: FunctionElementId) {
    let e = m.e;
    let p = e.program;
    let func = p.function(f);
    if let Some(ft) = e.outline.functions.get(f.0 as usize) {
        let s = ft.signature;
        m.tipo_de(s);
    }
    match m.no_da_funcao(f) {
        FunctionRef::Function { unit, function } => {
            let ctx = Contexto { unidade: unit, biblioteca: func.library, classe: func.class, extensao: func.extension };
            percorrer_funcao(m, &ctx, function);
        }
        FunctionRef::Constructor { unit, member } => {
            let ctx = Contexto { unidade: unit, biblioteca: func.library, classe: func.class, extensao: None };
            let ast = &p.unit(unit).ast;
            let ast::MemberKind::Constructor(c) = &ast.member(member).kind else { return };
            let Some(k) = func.class else { return };
            percorrer_parametros(m, &ctx, &c.parameters);
            let mut explicito = false;
            for ini in c.initializers.iter() {
                match ini {
                    Initializer::Field { value, .. } => expr(m, &ctx, *value),
                    Initializer::Super { constructor, arguments, .. } => {
                        explicito = true;
                        let nome = constructor.map(|n| m.e.interner.resolve(n.sym)).unwrap_or("");
                        if let Some(sc) = m.construtor_super(k, nome) {
                            m.usar_construtor_por_inicializador(sc);
                        }
                        argumentos(m, &ctx, arguments);
                    }
                    Initializer::Redirect { constructor, arguments, .. } => {
                        explicito = true;
                        let nome = constructor.map(|n| m.e.interner.resolve(n.sym)).unwrap_or("");
                        if let Some(rc) = m.construtor(k, nome) {
                            m.usar_construtor_por_inicializador(rc);
                        }
                        argumentos(m, &ctx, arguments);
                    }
                    Initializer::Assert { condition, message, .. } => {
                        expr(m, &ctx, *condition);
                        if let Some(x) = message {
                            expr(m, &ctx, *x);
                        }
                    }
                }
            }
            if let Some(r) = &c.redirect {
                // `factory C() = D.n;` — as chamadas diretas já vão ao alvo
                // (`module.rs::emit_factory`), mas o corpo encaminha para o
                // tearoff, e o alvo é instanciado.
                tipo_ast(m, &ctx, r.ty);
                if let Some((dc, nome)) = classe_e_construtor(m, &ctx, r.ty, r.constructor) {
                    m.criar(dc, nome, false);
                }
            } else if !c.factory && !explicito {
                // Super implícito: o construtor sem nome da superclasse.
                if let Some(sc) = m.construtor_super(k, "") {
                    m.usar_construtor_por_inicializador(sc);
                }
            }
            corpo(m, &ctx, &c.body);
        }
        FunctionRef::None => {
            if func.kind == FunctionKind::SyntheticConstructor {
                if let Some(k) = func.class {
                    // Aplicação de mixin sintética encaminha para o construtor
                    // homônimo da superclasse; a classe comum, para o sem nome.
                    let nome = if p.class(k).decl.is_none() { m.e.interner.resolve(func.name) } else { "" };
                    if let Some(sc) = m.construtor_super(k, nome) {
                        m.usar_construtor_por_inicializador(sc);
                    }
                }
            }
        }
    }
}

pub(crate) fn de_variavel(m: &mut Motor<'_>, v: VariableId) {
    let e = m.e;
    let var = e.program.variable(v);
    if let Some(vt) = e.outline.variables.get(v.0 as usize) {
        if let Some(t) = vt.declared_type.or(vt.inferred) {
            m.tipo_de(t);
        }
    }
    if let Some((unit, Some(init))) = m.no_da_variavel(v) {
        let ctx = Contexto { unidade: unit, biblioteca: var.library, classe: var.class, extensao: var.extension };
        expr(m, &ctx, init);
    }
}

/// Inicializadores dos campos de instância (e as constantes de um enum) de
/// uma classe que acabou de ser instanciada — `fieldInit`,
/// `kernel_impact.dart:969`.
pub(crate) fn de_campos(m: &mut Motor<'_>, c: ClassId) {
    let e = m.e;
    let p = e.program;
    let class = p.class(c);
    for &vid in &class.fields {
        let var = p.variable(vid);
        if var.static_ {
            continue;
        }
        if let Some(vt) = e.outline.variables.get(vid.0 as usize) {
            if let Some(t) = vt.declared_type.or(vt.inferred) {
                m.tipo_de(t);
            }
        }
        if let Some((unit, Some(init))) = m.no_da_variavel(vid) {
            let ctx = Contexto { unidade: unit, biblioteca: var.library, classe: Some(c), extensao: None };
            expr(m, &ctx, init);
        }
    }
    if class.kind == ClassKind::Enum {
        // O emissor escreve todas as constantes e `values` junto com a classe.
        for &vid in &class.enum_constants {
            m.usar_variavel(vid);
        }
        let Some(d) = class.decl else { return };
        let ast = &p.unit(d.unit).ast;
        if let ast::DeclKind::Enum(ed) = &ast.decl(d.decl).kind {
            let ctx = Contexto { unidade: d.unit, biblioteca: class.library, classe: Some(c), extensao: None };
            for k in ed.constants.iter() {
                let nome = k.constructor.map(|n| m.e.interner.resolve(n.sym)).unwrap_or("");
                m.criar(c, nome, false);
                for t in k.type_args.iter() {
                    tipo_ast(m, &ctx, *t);
                }
                if let Some(a) = &k.arguments {
                    argumentos(m, &ctx, a);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- funções

fn percorrer_funcao(m: &mut Motor<'_>, ctx: &Contexto, id: ast::FunctionId) {
    let e = m.e;
    let f = e.program.unit(ctx.unidade).ast.function(id);
    for tp in f.type_params.iter() {
        if let Some(b) = tp.bound {
            tipo_ast(m, ctx, b);
        }
    }
    if let Some(r) = f.return_type {
        tipo_ast(m, ctx, r);
    }
    if let Some(ps) = &f.parameters {
        percorrer_parametros(m, ctx, ps);
    }
    corpo(m, ctx, &f.body);
}

fn corpo(m: &mut Motor<'_>, ctx: &Contexto, b: &FunctionBody) {
    match b {
        FunctionBody::Block(s) => stmt(m, ctx, *s),
        FunctionBody::Expression(x) => expr(m, ctx, *x),
        FunctionBody::Empty | FunctionBody::Native(_) => {}
    }
}

fn percorrer_parametros(m: &mut Motor<'_>, ctx: &Contexto, ps: &[ast::Parameter]) {
    for p in ps {
        if let Some(t) = p.ty {
            tipo_ast(m, ctx, t);
        }
        if let Some(d) = p.default_value {
            expr(m, ctx, d);
        }
        for tp in p.function_type_params.iter() {
            if let Some(b) = tp.bound {
                tipo_ast(m, ctx, b);
            }
        }
        if let Some(fp) = &p.function_parameters {
            percorrer_parametros(m, ctx, fp);
        }
    }
}

fn argumentos(m: &mut Motor<'_>, ctx: &Contexto, a: &ast::Arguments) {
    for t in a.type_args.iter() {
        tipo_ast(m, ctx, *t);
    }
    for arg in a.args.iter() {
        expr(m, ctx, arg.value);
    }
}

// ---------------------------------------------------------------- tipos escritos

/// Classe nomeada por uma anotação de tipo (`C`, `p.C`, typedef de `C`).
fn classe_do_tipo_ast(m: &Motor<'_>, ctx: &Contexto, t: ast::TypeId) -> Option<ClassId> {
    let e = m.e;
    let p = e.program;
    let TypeKind::Named { name, .. } = &p.unit(ctx.unidade).ast.ty(t).kind else { return None };
    let b = match name.len() {
        1 => p.lookup(ctx.biblioteca, name[0].sym)?,
        2 => p.lookup_prefixed(ctx.biblioteca, name[0].sym, name[1].sym)?,
        _ => return None,
    };
    match b.getter? {
        Element::Class(c) => Some(c),
        Element::Typedef(td) => {
            let tt = e.outline.typedefs.get(td.0 as usize)?.target_type;
            match e.table.get(tt) {
                dartforge_types::table::Type::Interface { class, .. } => Some(*class),
                _ => None,
            }
        }
        _ => None,
    }
}

/// `C`, `p.C` ou `C.nome` (o parser não distingue prefixo de construtor
/// nomeado em `new C.nome()` / `factory X() = C.nome`): a classe e o nome do
/// construtor.
fn classe_e_construtor<'n>(m: &Motor<'n>, ctx: &Contexto, t: ast::TypeId, construtor: Option<ast::Name>) -> Option<(ClassId, &'n str)> {
    let e = m.e;
    let p = e.program;
    let nome = construtor.map(|n| e.interner.resolve(n.sym)).unwrap_or("");
    if let TypeKind::Named { name, .. } = &p.unit(ctx.unidade).ast.ty(t).kind {
        if name.len() == 2 {
            if let Some(Element::Class(c)) = p.lookup(ctx.biblioteca, name[0].sym).and_then(|b| b.getter) {
                return Some((c, e.interner.resolve(name[1].sym)));
            }
        }
    }
    classe_do_tipo_ast(m, ctx, t).map(|c| (c, nome))
}

fn tipo_ast(m: &mut Motor<'_>, ctx: &Contexto, t: ast::TypeId) {
    let e = m.e;
    let p = e.program;
    let ta = p.unit(ctx.unidade).ast.ty(t);
    match &ta.kind {
        TypeKind::Named { name, args } => {
            let b = match name.len() {
                1 => p.lookup(ctx.biblioteca, name[0].sym),
                2 => p.lookup_prefixed(ctx.biblioteca, name[0].sym, name[1].sym),
                _ => None,
            };
            if let Some(el) = b.and_then(|b| b.getter) {
                if matches!(el, Element::Class(_) | Element::Typedef(_)) {
                    m.usar_elemento(el);
                }
            }
            for a in args.iter() {
                tipo_ast(m, ctx, *a);
            }
        }
        TypeKind::Void => {}
        TypeKind::Function { return_type, type_params, parameters } => {
            if let Some(r) = return_type {
                tipo_ast(m, ctx, *r);
            }
            for tp in type_params.iter() {
                if let Some(b) = tp.bound {
                    tipo_ast(m, ctx, b);
                }
            }
            percorrer_parametros(m, ctx, parameters);
        }
        TypeKind::Record { positional, named } => {
            for x in positional.iter() {
                tipo_ast(m, ctx, *x);
            }
            for (_, x) in named.iter() {
                tipo_ast(m, ctx, *x);
            }
        }
    }
}

// ---------------------------------------------------------------- statements

fn variaveis(m: &mut Motor<'_>, ctx: &Contexto, l: &ast::VariableList) {
    if let Some(t) = l.ty {
        tipo_ast(m, ctx, t);
    }
    for v in l.variables.iter() {
        if let Some(i) = v.initializer {
            expr(m, ctx, i);
        }
    }
}

fn alvo_for_in(m: &mut Motor<'_>, ctx: &Contexto, t: &ForInTarget) {
    match t {
        ForInTarget::Declared { ty, .. } => {
            if let Some(t) = ty {
                tipo_ast(m, ctx, *t);
            }
        }
        ForInTarget::Pattern { pattern, .. } => padrao(m, ctx, *pattern),
        ForInTarget::Expression(x) => {
            // `for (alvo in ...)` escreve no alvo (e o lê, se composto).
            escrita_no_alvo(m, ctx, *x);
            expr(m, ctx, *x);
        }
    }
    // `for-in` chama o protocolo de iteração (`kernel_impact.dart:906-918`).
    for s in ["iterator", "moveNext", "current"] {
        m.novo_seletor(s);
    }
}

/// A classe do tipo estático do `receptor`, quando conhecida e nominal.
/// `None` é receptor dinâmico ou desconhecido: o seletor continua irrestrito
/// (nunca se poda por falta de informação).
fn classe_do_receptor(m: &Motor<'_>, ctx: &Contexto, receptor: ExprId) -> Option<ClassId> {
    let u = m.e.bodies.units.get(ctx.unidade.0 as usize)?;
    m.classe_do_tipo(u.get_type(receptor)?)
}

/// O cone do receptor de um uso de membro (`x.nome`, `nome` solto):
///
/// * `x.nome` com alvo estático (`C.nome`, `p.nome`) não é despacho em
///   instância — irrestrito, como antes;
/// * `x.nome` com `x: T` conhecido restringe ao cone de `T`;
/// * `nome` solto é `this.nome` implícito: o cone é a classe envolvente
///   (método de topo e estático não têm `this`, então `ctx.classe` é `None`
///   e o seletor continua irrestrito).
fn receptor_do_uso(m: &Motor<'_>, ctx: &Contexto, id: ExprId) -> Option<ClassId> {
    let ast = &m.e.program.unit(ctx.unidade).ast;
    match &ast.expr(id).kind {
        ExprKind::Property { target, .. } => {
            if alvo_estatico(m, ctx, *target).is_some() {
                return None;
            }
            // `super.s()` executa a implementação da superclasse, mas o tipo
            // estático do receptor `super` é a classe atual (`tipo_this`):
            // restringir pelo tipo podaria a superclasse em execução. O cone
            // é a classe onde o membro resolveu (despacho estático).
            if matches!(&ast.expr(*target).kind, ExprKind::Super) {
                return m
                    .e
                    .bodies
                    .units
                    .get(ctx.unidade.0 as usize)
                    .and_then(|u| u.get_resolved(id))
                    .and_then(|r| match r {
                        Resolved::Member { class, .. } => Some(*class),
                        _ => None,
                    });
            }
            classe_do_receptor(m, ctx, *target)
        }
        ExprKind::Identifier(_) => ctx.classe,
        _ => None,
    }
}

/// Um uso de escrita fora do `Assign` simples (`+=`, `++`, alvo de
/// `for-in`): registra a espécie de escrita (`nome_=`, a chave do
/// `instance_members`) sem suprimir a leitura, que o percurso normal
/// registra em seguida. O cone é o do alvo, pela mesma regra do uso.
fn escrita_no_alvo(m: &mut Motor<'_>, ctx: &Contexto, alvo: ExprId) {
    let e = m.e;
    let ast = &e.program.unit(ctx.unidade).ast;
    let nome = match &ast.expr(alvo).kind {
        ExprKind::Property { name, .. } => e.interner.resolve(name.sym),
        ExprKind::Identifier(n) => e.interner.resolve(n.sym),
        _ => return,
    };
    let receptor = receptor_do_uso(m, ctx, alvo);
    m.novo_seletor_com_receptor(&format!("{nome}_="), receptor);
}

fn for_init(m: &mut Motor<'_>, ctx: &Contexto, i: &ForInit) {
    match i {
        ForInit::Variables(l) => variaveis(m, ctx, l),
        ForInit::Expression(x) => expr(m, ctx, *x),
    }
}

fn stmt(m: &mut Motor<'_>, ctx: &Contexto, id: StmtId) {
    let e = m.e;
    let ast = &e.program.unit(ctx.unidade).ast;
    match &ast.stmt(id).kind {
        StmtKind::Block(ss) => {
            for s in ss.iter() {
                stmt(m, ctx, *s);
            }
        }
        StmtKind::Variables(l) => variaveis(m, ctx, l),
        StmtKind::PatternVariables { pattern, value, .. } => {
            padrao(m, ctx, *pattern);
            expr(m, ctx, *value);
        }
        StmtKind::Function(f) => percorrer_funcao(m, ctx, *f),
        StmtKind::Expression(x) => expr(m, ctx, *x),
        StmtKind::If { condition, case_pattern, guard, then, else_ } => {
            expr(m, ctx, *condition);
            if let Some(pp) = case_pattern {
                padrao(m, ctx, *pp);
            }
            if let Some(g) = guard {
                expr(m, ctx, *g);
            }
            stmt(m, ctx, *then);
            if let Some(x) = else_ {
                stmt(m, ctx, *x);
            }
        }
        StmtKind::For { init, condition, updates, body, .. } => {
            if let Some(i) = init {
                for_init(m, ctx, i);
            }
            if let Some(c) = condition {
                expr(m, ctx, *c);
            }
            for u in updates.iter() {
                expr(m, ctx, *u);
            }
            stmt(m, ctx, *body);
        }
        StmtKind::ForIn { target, iterable, body, .. } => {
            alvo_for_in(m, ctx, target);
            expr(m, ctx, *iterable);
            stmt(m, ctx, *body);
        }
        StmtKind::While { condition, body } | StmtKind::DoWhile { body, condition } => {
            expr(m, ctx, *condition);
            stmt(m, ctx, *body);
        }
        StmtKind::Switch { value, cases } => {
            expr(m, ctx, *value);
            for c in cases.iter() {
                if let Some(pp) = c.pattern {
                    padrao(m, ctx, pp);
                }
                if let Some(g) = c.guard {
                    expr(m, ctx, g);
                }
                for s in c.body.iter() {
                    stmt(m, ctx, *s);
                }
            }
        }
        StmtKind::Break(_) | StmtKind::Continue(_) | StmtKind::Empty => {}
        StmtKind::Return(x) => {
            if let Some(x) = x {
                expr(m, ctx, *x);
            }
        }
        StmtKind::Yield { value, .. } => expr(m, ctx, *value),
        StmtKind::Try { body, catches, finally_ } => {
            stmt(m, ctx, *body);
            for c in catches.iter() {
                if let Some(t) = c.on_type {
                    tipo_ast(m, ctx, t);
                }
                stmt(m, ctx, c.body);
            }
            if let Some(f) = finally_ {
                stmt(m, ctx, *f);
            }
        }
        StmtKind::Labeled { body, .. } => stmt(m, ctx, *body),
        StmtKind::Assert { condition, message } => {
            expr(m, ctx, *condition);
            if let Some(x) = message {
                expr(m, ctx, *x);
            }
        }
    }
}

// ---------------------------------------------------------------- padrões

/// Nomes das variáveis declaradas num padrão (`:x` num padrão de objeto é o
/// getter `x`).
fn nomes_de_variavel(ast: &ast::Ast, p: PatternId, out: &mut Vec<dartforge_intern::SymbolId>) {
    match &ast.pattern(p).kind {
        PatternKind::Variable { name, .. } => out.push(name.sym),
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => nomes_de_variavel(ast, *x, out),
        PatternKind::Or(a, b) | PatternKind::And(a, b) => {
            nomes_de_variavel(ast, *a, out);
            nomes_de_variavel(ast, *b, out);
        }
        PatternKind::Wildcard { .. } | PatternKind::Constant(_) | PatternKind::Relational { .. } | PatternKind::List { .. } | PatternKind::Map { .. } | PatternKind::Record { .. } | PatternKind::Object { .. } => {}
    }
}

fn padrao(m: &mut Motor<'_>, ctx: &Contexto, id: PatternId) {
    let e = m.e;
    let ast = &e.program.unit(ctx.unidade).ast;
    match &ast.pattern(id).kind {
        PatternKind::Wildcard { ty } => {
            if let Some(t) = ty {
                tipo_ast(m, ctx, *t);
            }
        }
        PatternKind::Variable { ty, name, var_, final_ } => {
            if let Some(t) = ty {
                tipo_ast(m, ctx, *t);
            }
            // `case NOME:` — o identificador solto num `case` é constante (o
            // parser o entrega como padrão de variável e a resolução decide).
            if ty.is_none() && !*var_ && !*final_ {
                m.usar_nome(ctx, name.sym);
            }
        }
        PatternKind::Constant(x) => {
            expr(m, ctx, *x);
            m.novo_seletor("==");
        }
        PatternKind::Relational { value, .. } => expr(m, ctx, *value),
        PatternKind::Or(a, b) | PatternKind::And(a, b) => {
            padrao(m, ctx, *a);
            padrao(m, ctx, *b);
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => padrao(m, ctx, *x),
        PatternKind::Cast { pattern, ty } => {
            tipo_ast(m, ctx, *ty);
            padrao(m, ctx, *pattern);
        }
        PatternKind::List { type_args, elements } => {
            for t in type_args.iter() {
                tipo_ast(m, ctx, *t);
            }
            for el in elements.iter() {
                match el {
                    ast::ListPatternElement::Pattern(x) => padrao(m, ctx, *x),
                    ast::ListPatternElement::Rest(x) => {
                        if let Some(x) = x {
                            padrao(m, ctx, *x);
                        }
                    }
                }
            }
        }
        PatternKind::Map { type_args, entries, .. } => {
            for t in type_args.iter() {
                tipo_ast(m, ctx, *t);
            }
            for en in entries.iter() {
                expr(m, ctx, en.key);
                padrao(m, ctx, en.value);
            }
        }
        PatternKind::Record { fields } => {
            for f in fields.iter() {
                padrao(m, ctx, f.pattern);
            }
        }
        PatternKind::Object { ty, fields } => {
            tipo_ast(m, ctx, *ty);
            for f in fields.iter() {
                match f.name {
                    Some(n) => {
                        let s = e.interner.resolve(n.sym);
                        m.novo_seletor(s);
                    }
                    None => {
                        let mut ns = Vec::new();
                        nomes_de_variavel(ast, f.pattern, &mut ns);
                        for n in ns {
                            let s = e.interner.resolve(n);
                            m.novo_seletor(s);
                        }
                    }
                }
                padrao(m, ctx, f.pattern);
            }
        }
    }
}

// ---------------------------------------------------------------- expressões

fn elemento(m: &mut Motor<'_>, ctx: &Contexto, el: &CollectionElement) {
    match el {
        CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => expr(m, ctx, *x),
        CollectionElement::MapEntry { key, value, .. } => {
            expr(m, ctx, *key);
            expr(m, ctx, *value);
        }
        CollectionElement::Spread { value, .. } => {
            expr(m, ctx, *value);
            for s in ["iterator", "moveNext", "current"] {
                m.novo_seletor(s);
            }
        }
        CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
            expr(m, ctx, *condition);
            if let Some(pp) = case_pattern {
                padrao(m, ctx, *pp);
            }
            if let Some(g) = guard {
                expr(m, ctx, *g);
            }
            elemento(m, ctx, then);
            if let Some(x) = else_ {
                elemento(m, ctx, x);
            }
        }
        CollectionElement::For { init, condition, updates, body, .. } => {
            if let Some(i) = init {
                for_init(m, ctx, i);
            }
            if let Some(c) = condition {
                expr(m, ctx, *c);
            }
            for u in updates.iter() {
                expr(m, ctx, *u);
            }
            elemento(m, ctx, body);
        }
        CollectionElement::ForIn { target, iterable, body, .. } => {
            alvo_for_in(m, ctx, target);
            expr(m, ctx, *iterable);
            elemento(m, ctx, body);
        }
    }
}

fn aplicar_resolved(m: &mut Motor<'_>, r: &Resolved, receptor: Option<ClassId>) {
    match r {
        Resolved::Element(el) => m.usar_elemento(*el),
        Resolved::Member { member, .. } => match member {
            MemberRef::Function(f) => m.usar_funcao_com_receptor(*f, receptor),
            MemberRef::Variable(v) => m.usar_variavel_com_receptor(*v, receptor),
        },
        Resolved::ExtensionMember { member, .. } => m.usar_funcao(*member),
        Resolved::Constructor(f) => m.usar_construtor(*f, false),
        Resolved::Local(_) | Resolved::Parameter { .. } | Resolved::TypeParameter(_) | Resolved::Prefix(_) | Resolved::Dynamic => {}
    }
}

/// O que `alvo` denota quando está à esquerda de `.`: classe (estáticos e
/// construtores nomeados), extensão (estáticos) ou prefixo de import.
fn alvo_estatico(m: &Motor<'_>, ctx: &Contexto, alvo: ExprId) -> Option<Alvo> {
    let e = m.e;
    let p = e.program;
    let ast = &p.unit(ctx.unidade).ast;
    let unid = e.bodies.units.get(ctx.unidade.0 as usize);
    match &ast.expr(alvo).kind {
        ExprKind::Identifier(n) => {
            match unid.and_then(|u| u.get_resolved(alvo)) {
                Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) => return None,
                Some(Resolved::Element(Element::Class(c))) => return Some(Alvo::Classe(*c)),
                Some(Resolved::Element(Element::Extension(x))) => return Some(Alvo::Extensao(*x)),
                Some(Resolved::Prefix(_)) => return Some(Alvo::Prefixo(n.sym)),
                _ => {}
            }
            match p.lookup(ctx.biblioteca, n.sym).and_then(|b| b.getter)? {
                Element::Class(c) => Some(Alvo::Classe(c)),
                Element::Extension(x) => Some(Alvo::Extensao(x)),
                Element::Prefix(..) => Some(Alvo::Prefixo(n.sym)),
                Element::Typedef(td) => {
                    let tt = e.outline.typedefs.get(td.0 as usize)?.target_type;
                    match e.table.get(tt) {
                        dartforge_types::table::Type::Interface { class, .. } => Some(Alvo::Classe(*class)),
                        _ => None,
                    }
                }
                Element::Function(_) | Element::Variable(_) => None,
            }
        }
        // `C<int>.new`, `p.C<T>.nome`
        ExprKind::TypeArguments { target, .. } => alvo_estatico(m, ctx, *target),
        ExprKind::Property { target, name, .. } => {
            let ExprKind::Identifier(pre) = &ast.expr(*target).kind else { return None };
            match p.lookup_prefixed(ctx.biblioteca, pre.sym, name.sym).and_then(|b| b.getter)? {
                Element::Class(c) => Some(Alvo::Classe(c)),
                Element::Extension(x) => Some(Alvo::Extensao(x)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// `alvo.nome` com alvo estático.
fn membro_estatico(m: &mut Motor<'_>, ctx: &Contexto, alvo: Alvo, nome: dartforge_intern::SymbolId) {
    let e = m.e;
    let p = e.program;
    let texto = e.interner.resolve(nome);
    let setter = e.interner.lookup(&format!("{texto}_="));
    match alvo {
        Alvo::Classe(c) => {
            m.marcar_tipo(c);
            let class = p.class(c);
            let mut fs: Vec<FunctionElementId> = Vec::new();
            fs.extend(class.static_members.get(&nome).copied());
            if let Some(s) = setter {
                fs.extend(class.static_members.get(&s).copied());
            }
            for f in fs {
                m.usar_funcao(f);
            }
            // Chamada ou tearoff (`C.nome` sem chamada): o tearoff estático
            // custa poucos bytes, então vale para os dois.
            m.criar(c, texto, true);
        }
        Alvo::Extensao(x) => m.usar_estatico_de_extensao(x, nome),
        Alvo::Prefixo(pre) => {
            if let Some(b) = p.lookup_prefixed(ctx.biblioteca, pre, nome) {
                for el in [b.getter, b.setter].into_iter().flatten() {
                    m.usar_elemento(el);
                }
            }
        }
    }
}

fn expr(m: &mut Motor<'_>, ctx: &Contexto, id: ExprId) {
    let e = m.e;
    let p = e.program;
    let ast = &p.unit(ctx.unidade).ast;
    // O cone do receptor, quando o uso é despacho em instância com tipo
    // estático conhecido: vale para o `Resolved` (que não carrega o tipo do
    // receptor) e para o nome escrito (que independe da resolução).
    let receptor = receptor_do_uso(m, ctx, id);
    if let Some(u) = e.bodies.units.get(ctx.unidade.0 as usize) {
        if let Some(t) = u.get_type(id) {
            m.tipo_de(t);
        }
        if let Some(r) = u.get_resolved(id) {
            let r = r.clone();
            aplicar_resolved(m, &r, receptor);
        }
    }
    match &ast.expr(id).kind {
        ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::Bool(_) | ExprKind::Null | ExprKind::Symbol(_) | ExprKind::This | ExprKind::Super | ExprKind::CascadeTarget | ExprKind::Rethrow => {}
        ExprKind::String(lit) => {
            for part in lit.parts.iter() {
                match part {
                    StringPart::Text(_) => {}
                    StringPart::Interpolation(x) => expr(m, ctx, *x),
                }
            }
        }
        ExprKind::Identifier(n) => {
            // `nome` solto pode ser `this.nome` implícito. No alvo de um
            // `Assign` simples é escrita (`nome_=`, a chave do setter no
            // `instance_members`); senão, leitura. O cone é o da classe
            // envolvente (`receptor`, acima).
            if m.alvo_de_escrita == Some(id) {
                m.alvo_de_escrita = None;
                let s = e.interner.resolve(n.sym);
                m.novo_seletor_com_receptor(&format!("{s}_="), receptor);
            } else {
                let s = e.interner.resolve(n.sym);
                m.novo_seletor_com_receptor(s, receptor);
            }
            m.usar_nome(ctx, n.sym);
        }
        ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => expr(m, ctx, *x),
        ExprKind::List { type_args, elements, .. } | ExprKind::SetOrMap { type_args, elements, .. } => {
            for t in type_args.iter() {
                tipo_ast(m, ctx, *t);
            }
            for el in elements.iter() {
                elemento(m, ctx, el);
            }
        }
        ExprKind::Record { positional, named, .. } => {
            for x in positional.iter() {
                expr(m, ctx, *x);
            }
            for (_, x) in named.iter() {
                expr(m, ctx, *x);
            }
        }
        ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
            tipo_ast(m, ctx, *ty);
            if let Some((c, nome)) = classe_e_construtor(m, ctx, *ty, *constructor) {
                m.criar(c, nome, false);
            }
            argumentos(m, ctx, arguments);
        }
        ExprKind::FunctionExpression(f) => percorrer_funcao(m, ctx, *f),
        ExprKind::Property { target, name, .. } => {
            // No alvo de um `Assign` simples é escrita (`nome_=`, a chave do
            // setter no `instance_members`); senão, é leitura ou chamada. O
            // receptor continua sendo leitura, e o `Resolved` (que já
            // distingue getter de setter) continua valendo. O cone é o do
            // receptor (`receptor`, acima); acesso estático (`C.nome`) já
            // voltou `None` por lá.
            if m.alvo_de_escrita == Some(id) {
                m.alvo_de_escrita = None;
                let s = e.interner.resolve(name.sym);
                m.novo_seletor_com_receptor(&format!("{s}_="), receptor);
            } else {
                let s = e.interner.resolve(name.sym);
                m.novo_seletor_com_receptor(s, receptor);
            }
            if let Some(a) = alvo_estatico(m, ctx, *target) {
                membro_estatico(m, ctx, a, name.sym);
            }
            expr(m, ctx, *target);
        }
        ExprKind::Index { target, index, .. } => {
            expr(m, ctx, *target);
            expr(m, ctx, *index);
        }
        // Atalho de ponto (3.10): `.nome` é `D.nome`, com `D` gravada por
        // `types` em `Resolved` (docs/VERSOES-LINGUAGEM.md §4.3). Onde
        // `types` não chega (padrões de `switch`), vale toda declaração que
        // tem estático, constante de enum ou construtor com esse nome: mais
        // vivo, nunca menos.
        ExprKind::DotShorthand { name, .. } => {
            let resolvida = e.bodies.units.get(ctx.unidade.0 as usize).and_then(|u| u.get_resolved(id)).and_then(|r| match r {
                Resolved::Element(Element::Class(c)) => Some(*c),
                _ => None,
            });
            let classes: Vec<ClassId> = match resolvida {
                Some(c) => vec![c],
                None => {
                    let texto = e.interner.resolve(name.sym);
                    let chave_ctor = if texto == "new" { e.interner.lookup("") } else { Some(name.sym) };
                    (0..p.classes.len() as u32)
                        .map(ClassId)
                        .filter(|&c| {
                            let k = p.class(c);
                            k.static_members.contains_key(&name.sym)
                                || k.enum_constants.iter().any(|v| p.variable(*v).name == name.sym)
                                || chave_ctor.is_some_and(|s| k.constructors.contains_key(&s))
                        })
                        .collect()
                }
            };
            for c in classes {
                membro_estatico(m, ctx, Alvo::Classe(c), name.sym);
            }
        }
        ExprKind::Call { target, arguments } => {
            // `C(..)` / `p.C(..)` sem `new`: criação pelo construtor sem nome.
            if let Some(Alvo::Classe(c)) = alvo_estatico(m, ctx, *target) {
                m.criar(c, "", false);
            }
            expr(m, ctx, *target);
            argumentos(m, ctx, arguments);
        }
        ExprKind::TypeArguments { target, type_args } => {
            expr(m, ctx, *target);
            for t in type_args.iter() {
                tipo_ast(m, ctx, *t);
            }
        }
        ExprKind::Unary { op, operand } => {
            // `++`/`--` lê e escreve (`x++` chama o getter e o setter).
            if matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec) {
                escrita_no_alvo(m, ctx, *operand);
            }
            expr(m, ctx, *operand);
        }
        ExprKind::Binary { left, right, .. } => {
            expr(m, ctx, *left);
            expr(m, ctx, *right);
        }
        ExprKind::Conditional { condition, then, else_ } => {
            expr(m, ctx, *condition);
            expr(m, ctx, *then);
            expr(m, ctx, *else_);
        }
        ExprKind::Is { value, ty, .. } | ExprKind::As { value, ty } => {
            expr(m, ctx, *value);
            tipo_ast(m, ctx, *ty);
        }
        ExprKind::Assign { op, target, value } => {
            match op {
                // Escrita pura: só o membro mais externo do alvo é `nome=`;
                // o resto do alvo (receptor, índice) continua leitura.
                AssignOp::Assign => {
                    // `(x.w) = v`: o alvo real está dentro dos parênteses.
                    let mut alvo = *target;
                    loop {
                        if let ExprKind::Parenthesized(x) = &e.program.unit(ctx.unidade).ast.expr(alvo).kind {
                            alvo = *x;
                        } else {
                            break;
                        }
                    }
                    m.alvo_de_escrita = Some(alvo);
                    expr(m, ctx, *target);
                    m.alvo_de_escrita = None;
                    expr(m, ctx, *value);
                }
                // Composto (`+=`, `??=`): lê e escreve.
                AssignOp::Compound(_) => {
                    escrita_no_alvo(m, ctx, *target);
                    expr(m, ctx, *target);
                    expr(m, ctx, *value);
                }
            }
        }
        ExprKind::PatternAssign { pattern, value } => {
            padrao(m, ctx, *pattern);
            expr(m, ctx, *value);
        }
        ExprKind::Cascade { target, sections, .. } => {
            expr(m, ctx, *target);
            for s in sections.iter() {
                expr(m, ctx, *s);
            }
        }
        ExprKind::Switch { value, cases } => {
            expr(m, ctx, *value);
            for c in cases.iter() {
                padrao(m, ctx, c.pattern);
                if let Some(g) = c.guard {
                    expr(m, ctx, g);
                }
                expr(m, ctx, c.body);
            }
        }
    }
}
