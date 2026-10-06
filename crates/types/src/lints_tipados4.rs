//! Mais regras de lint tipadas do conjunto `core`
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`). Como
//! em [`crate::lints_tipados`], devolvem achados neutros.
//!
//! * `collection_methods_unrelated_type`: as definições na ordem do emissor
//!   (`contains` do `Iterable`, `remove` de `List`, `Map`, `Queue` do
//!   `dart.collection` e `Set`, `containsKey`/`containsValue` de `Map`,
//!   `lookup` de `Set`, e o `[]` de `Map`), o `asInstanceOf` pela hierarquia
//!   instanciada e o `typesAreUnrelated` de [`crate::lints_tipados3`]. Sem
//!   alvo, o tipo é o `thisType` da classe, mixin ou enum que envolve a
//!   chamada, ou o tipo estendido da extensão. Só o `MethodInvocation`: o
//!   nome resolvido a variável ou getter vira `FunctionExpressionInvocation`
//!   no analyzer.
//! * `void_checks`: a atribuição (pelo `writeType`), o `return` (pela
//!   `FunctionExpression` ou pelo método mais próximo; o construtor não
//!   conta), os argumentos de criação de instância e de `MethodInvocation`
//!   (pelo `staticParameterElement`, `UnitBodyTypes::tipos_de_parametros`) e
//!   o padrão de variável atribuída (pelo tipo casado).
//! * `unnecessary_overrides`: o membro concreto herdado
//!   (`lookUp…(concrete: true, inherited: true)`: os mixins do último para o
//!   primeiro, depois a superclasse, com o acesso a nomes privados só na
//!   própria biblioteca), a assinatura instanciada no supertipo, os
//!   metadados (`@override`, `@protected`), o comentário de documentação e
//!   o corpo com uma só instrução que repassa ao `super`. O `isCovariant`
//!   compara o `covariant` escrito (a covariância herdada de mais acima
//!   vale para os dois lados).
//! * `no_duplicate_case_values`: só o `SwitchCase` antigo (biblioteca antes
//!   da 3.0), com o motor de constantes e a igualdade do `DartObjectImpl`.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::constantes::avaliador::{Constante, Ctx, Motor};
use crate::lints_tipados::Achado;
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved, UnitBodyTypes};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, BinaryOp, DeclKind, ExprId, ExprKind, FunctionBody, MemberKind, ParameterKind, PatternKind, StmtKind, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

fn sem_parenteses(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// O `FunctionElementId` de cada função escrita da unidade.
fn elementos_das_funcoes(program: &Program, u: UnitId) -> HashMap<ast::FunctionId, FunctionElementId> {
    let mut m = HashMap::new();
    for (k, f) in program.functions.iter().enumerate() {
        if let FunctionRef::Function { unit, function } = f.node
            && unit == u
        {
            m.insert(function, FunctionElementId(k as u32));
        }
    }
    m
}

/// O `realTarget` de cada seção de cascata.
fn alvos_de_cascata(a: &ast::Ast) -> HashMap<ExprId, ExprId> {
    let mut m = HashMap::new();
    for e in a.exprs.iter() {
        let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
        for &s in sections.iter() {
            let mut x = s;
            loop {
                let proximo = match &a.expr(x).kind {
                    ExprKind::Property { target, .. }
                    | ExprKind::Index { target, .. }
                    | ExprKind::Call { target, .. }
                    | ExprKind::TypeArguments { target, .. }
                    | ExprKind::Assign { target, .. } => *target,
                    ExprKind::Unary { operand, .. } => *operand,
                    ExprKind::CascadeTarget => {
                        m.insert(x, *target);
                        break;
                    }
                    _ => break,
                };
                x = proximo;
            }
        }
    }
    m
}

/// A chamada é um `MethodInvocation` do analyzer (o nome não resolve a
/// variável nem a getter, que viram `FunctionExpressionInvocation`).
pub(crate) fn e_invocacao_de_metodo(program: &Program, a: &ast::Ast, corpo: &UnitBodyTypes, alvo: ExprId) -> bool {
    if !matches!(a.expr(alvo).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) {
        return false;
    }
    let funcao_nao_getter = |f: FunctionElementId| program.function(f).kind != FunctionKind::Getter;
    match corpo.get_resolved(alvo) {
        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => funcao_nao_getter(*f),
        Some(Resolved::Member { member: MemberRef::Variable(_), .. }) => false,
        Some(Resolved::Element(dartforge_elements::model::Element::Function(f))) => funcao_nao_getter(*f),
        Some(Resolved::Element(_)) => false,
        Some(Resolved::ExtensionMember { member, .. }) => funcao_nao_getter(*member),
        Some(Resolved::Local(_)) => {
            // A função local é `MethodInvocation`; a variável local, não.
            corpo.declaracao_local(alvo).is_some_and(|d| {
                a.stmts.iter().any(|s| matches!(&s.kind, StmtKind::Function(g) if a.function(*g).name.is_some_and(|n| n.span.start == d)))
            })
        }
        Some(Resolved::Parameter { .. }) => false,
        _ => true,
    }
}

/// Os achados das regras tipadas deste módulo na unidade `u`.
#[allow(clippy::too_many_arguments)]
pub fn achados(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpos: &BodyTypes,
    inferidas: &HashSet<LibraryId>,
    u: UnitId,
) -> Vec<Achado> {
    let mut out: Vec<Achado> = Vec::new();
    let Some(corpo) = corpos.units.get(u.0 as usize) else { return out };
    colecoes(program, interner, table, core, outline, corpo, u, &mut out);
    checagens_de_void(program, table, core, outline, corpo, u, &mut out);
    sobrescritas_desnecessarias(program, interner, table, core, outline, corpo, u, &mut out);
    casos_duplicados(program, interner, table, core, outline, corpos, inferidas, u, &mut out);
    out
}

/// Uma definição de `_MethodDefinition`.
struct Definicao {
    metodo: &'static str,
    /// A classe (`dart:core`), ou a interface pelo nome e a biblioteca.
    classe: Option<ClassId>,
    por_nome: Option<(&'static str, &'static str)>,
    indice: usize,
}

/// `collection_methods_unrelated_type`.
#[allow(clippy::too_many_arguments)]
fn colecoes(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpo: &UnitBodyTypes,
    u: UnitId,
    out: &mut Vec<Achado>,
) {
    let a = &program.unit(u).ast;
    let cascatas = alvos_de_cascata(a);
    let def = |metodo, classe: Option<ClassId>, indice| Definicao { metodo, classe, por_nome: None, indice };
    let metodos = [
        def("contains", core.iterable_class, 0),
        def("remove", core.list_class, 0),
        def("containsKey", core.map_class, 0),
        def("containsValue", core.map_class, 1),
        def("remove", core.map_class, 0),
        Definicao { metodo: "remove", classe: None, por_nome: Some(("dart.collection", "Queue")), indice: 0 },
        def("lookup", core.set_class, 0),
        def("remove", core.set_class, 0),
    ];
    let indices = [def("[]", core.map_class, 0)];
    let nome_da_biblioteca = |l: LibraryId| program.library(l).name.as_ref().map(|n| n.iter().map(|s| interner.resolve(*s)).collect::<Vec<_>>().join("."));
    // `collectionTypeFor`: o `asInstanceOf`.
    let colecao = |table: &mut TypeTable, t: TypeId, d: &Definicao| -> Option<TypeId> {
        if let Some(c) = d.classe {
            return outline.hierarchy.supertype_of(t, c, table, core);
        }
        let (biblioteca, interface) = d.por_nome?;
        let classe_de = |t: TypeId| match table.get(t) {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => Some(*class),
            _ => None,
        };
        let propria = classe_de(t)?;
        let mut candidatas = vec![propria];
        if let Some(d) = outline.hierarchy.get(propria) {
            let mut sup: Vec<ClassId> = d.supertypes.keys().copied().collect();
            sup.sort();
            candidatas.extend(sup);
        }
        let alvo = candidatas.into_iter().find(|c| {
            let k = program.class(*c);
            interner.resolve(k.name) == interface && nome_da_biblioteca(k.library).as_deref() == Some(biblioteca)
        })?;
        outline.hierarchy.supertype_of(t, alvo, table, core)
    };
    let e_interface = |table: &TypeTable, t: TypeId| matches!(table.get(t), Type::Interface { .. } | Type::ExtensionType { .. } | Type::FutureOr { .. });
    // O tipo do alvo implícito (`this`): a classe, mixin ou enum que envolve,
    // ou o tipo estendido da extensão.
    let alvo_implicito = |table: &mut TypeTable, span: Span| -> Option<TypeId> {
        let mut melhor: Option<(Span, TypeId)> = None;
        for &did in &program.unit(u).unit.declarations {
            let d = a.decl(did);
            if !dentro(span, d.span) {
                continue;
            }
            let t = match &d.kind {
                DeclKind::Class(_) | DeclKind::Mixin(_) | DeclKind::Enum(_) => {
                    let c = (0..program.classes.len()).map(|i| ClassId(i as u32)).find(|c| {
                        program.class(*c).decl.is_some_and(|r| r.unit == u && r.decl == did)
                    })?;
                    let params: Vec<TypeId> = outline
                        .classes
                        .get(c.0 as usize)
                        .map(|x| x.type_params.iter().map(|p| table.intern(Type::TypeParameter { param: *p, nullable: false })).collect())
                        .unwrap_or_default();
                    table.intern(Type::Interface { class: c, args: params.into(), nullable: false })
                }
                DeclKind::Extension(_) => {
                    let e = program.extensions.iter().position(|x| x.decl.unit == u && x.decl.decl == did)?;
                    outline.extensions.get(e)?.on
                }
                _ => return None,
            };
            melhor = Some((d.span, t));
        }
        melhor.map(|(_, t)| t)
    };
    let mut casos: Vec<(Span, ExprId, TypeId, &Definicao)> = Vec::new();
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Call { target, arguments } => {
                if arguments.args.len() != 1 || !e_invocacao_de_metodo(program, a, corpo, *target) {
                    continue;
                }
                let (alvo, nome) = match &a.expr(*target).kind {
                    ExprKind::Property { target: alvo, name, .. } => (Some(*alvo), *name),
                    ExprKind::Identifier(n) => (None, *n),
                    _ => continue,
                };
                let nome = interner.resolve(nome.sym);
                let candidatas: Vec<&Definicao> = metodos.iter().filter(|d| d.metodo == nome).collect();
                if candidatas.is_empty() {
                    continue;
                }
                let tipo_do_alvo = match alvo {
                    Some(x) => {
                        let x = if matches!(a.expr(x).kind, ExprKind::CascadeTarget) { cascatas.get(&x).copied() } else { Some(x) };
                        x.and_then(|x| corpo.get_type(x))
                    }
                    None => alvo_implicito(table, e.span),
                };
                let Some(t) = tipo_do_alvo.filter(|t| e_interface(table, *t)) else { continue };
                let arg = &arguments.args[0];
                let span = match arg.name {
                    Some(n) => Span { start: n.span.start, end: a.expr(arg.value).span.end },
                    None => a.expr(arg.value).span,
                };
                for d in candidatas {
                    if let Some(col) = colecao(table, t, d) {
                        casos.push((span, arg.value, col, d));
                        break;
                    }
                }
            }
            ExprKind::Index { target, index, .. } => {
                let x = if matches!(a.expr(*target).kind, ExprKind::CascadeTarget) { cascatas.get(target).copied() } else { Some(*target) };
                let Some(t) = x.and_then(|x| corpo.get_type(x)).filter(|t| e_interface(table, *t)) else { continue };
                for d in &indices {
                    if let Some(col) = colecao(table, t, d) {
                        casos.push((a.expr(*index).span, *index, col, d));
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    let future_or = interner
        .lookup("FutureOr")
        .and_then(|s| program.library(core.async_library.unwrap_or(LibraryId(0))).declared.get(&s))
        .and_then(|b| b.getter)
        .and_then(|e| match e {
            dartforge_elements::model::Element::Class(c) => Some(c),
            _ => None,
        });
    for (span, arg, col, d) in casos {
        let Some(ta) = corpo.get_type(arg).filter(|t| !core.is_unknown(table, *t)) else { continue };
        let Type::Interface { args, .. } = table.get(col).clone() else { continue };
        let Some(&tipo_arg) = args.get(d.indice) else { continue };
        let mut rel = crate::lints_tipados3::Relacao { program, table: &mut *table, core, outline, future_or, call: interner.lookup("call") };
        if rel.sem_relacao(ta, tipo_arg) {
            let argumentos = vec![crate::despejo::formatar(table, ta, interner, program), crate::despejo::formatar(table, tipo_arg, interner, program)];
            out.push((span, "collection_methods_unrelated_type", argumentos));
        }
    }
}

/// `isTypeAcceptableWhenExpectingVoid`.
fn aceito_como_void(program: &Program, table: &TypeTable, core: &CoreTypes, t: TypeId) -> bool {
    match table.get(t) {
        Type::Void | Type::Null | Type::Never => true,
        Type::Interface { class, args, .. } if Some(*class) == core.future_class => {
            args.first().is_some_and(|x| aceito_como_void(program, table, core, *x))
        }
        _ => false,
    }
}

/// `isTypeAcceptableWhenExpectingFutureOrVoid`.
fn aceito_como_future_or_void(program: &Program, table: &TypeTable, core: &CoreTypes, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::FutureOr { .. } => true,
        _ if aceito_como_void(program, table, core, t) => true,
        Type::Interface { class, args, .. } if Some(*class) == core.future_class => {
            args.first().is_some_and(|x| aceito_como_future_or_void(program, table, core, *x))
        }
        _ => false,
    }
}

/// `_check` do `void_checks`: `true` relata. `closure_de_bloco`: o nó
/// conferido é uma `FunctionExpression` de corpo que não é de expressão.
fn checar_void(program: &Program, table: &TypeTable, core: &CoreTypes, esperado: TypeId, t: TypeId, e_retorno: bool, closure_de_bloco: bool) -> bool {
    if core.is_unknown(table, esperado) || core.is_unknown(table, t) {
        return false;
    }
    let esperado_void = matches!(table.get(esperado), Type::Void);
    if esperado_void && !matches!(table.get(t), Type::Dynamic) && e_retorno {
        return false;
    }
    if esperado_void && !aceito_como_void(program, table, core, t) {
        return true;
    }
    if let Type::FutureOr { arg, .. } = table.get(esperado) {
        if matches!(table.get(*arg), Type::Void) && !aceito_como_future_or_void(program, table, core, t) {
            return true;
        }
        return false;
    }
    if esperado_void {
        return false;
    }
    if closure_de_bloco
        && let (Type::Function { ret: r1, .. }, Type::Function { ret: r2, .. }) = (table.get(esperado), table.get(t))
    {
        return checar_void(program, table, core, *r1, *r2, e_retorno, closure_de_bloco);
    }
    false
}

/// `void_checks`.
fn checagens_de_void(program: &Program, table: &mut TypeTable, core: &CoreTypes, outline: &OutlineTypes, corpo: &UnitBodyTypes, u: UnitId, out: &mut Vec<Achado>) {
    let table: &TypeTable = table;
    let a = &program.unit(u).ast;
    let tipo = |e: ExprId| corpo.get_type(e).filter(|t| !core.is_unknown(table, *t));
    let closure_de_bloco = |e: ExprId| match &a.expr(e).kind {
        ExprKind::FunctionExpression(f) => !matches!(a.function(*f).body, FunctionBody::Expression(_)),
        _ => false,
    };
    let relatar = |span: Span, out: &mut Vec<Achado>| out.push((span, "void_checks", Vec::new()));
    // Atribuições.
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        if let ExprKind::Assign { value, .. } = &e.kind
            && let (Some(&esperado), Some(t)) = (corpo.tipos_de_escrita.get(&id), tipo(*value))
            && checar_void(program, table, core, esperado, t, false, closure_de_bloco(*value))
        {
            relatar(e.span, out);
        }
    }
    // `return`: a função mais próxima (literal, local, de topo ou método).
    let elementos = elementos_das_funcoes(program, u);
    let literais: HashMap<ast::FunctionId, ExprId> = a
        .exprs
        .iter()
        .enumerate()
        .filter_map(|(k, e)| match &e.kind {
            ExprKind::FunctionExpression(f) => Some((*f, ExprId(k as u32))),
            _ => None,
        })
        .collect();
    let locais: HashMap<ast::FunctionId, usize> = a
        .stmts
        .iter()
        .filter_map(|s| match &s.kind {
            StmtKind::Function(f) => a.function(*f).name.map(|n| (*f, n.span.start)),
            _ => None,
        })
        .collect();
    // Os corpos de construtor (o `return` neles não tem função).
    let construtores: Vec<Span> = a
        .members
        .iter()
        .filter_map(|m| match &m.kind {
            MemberKind::Constructor(k) => match &k.body {
                FunctionBody::Block(s) => Some(a.stmt(*s).span),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let retorno_da_funcao = |f: ast::FunctionId| -> Option<TypeId> {
        let t = if let Some(&e) = literais.get(&f) {
            tipo(e)?
        } else if let Some(&p) = locais.get(&f) {
            corpo.tipo_local(p)?
        } else {
            return elementos.get(&f).and_then(|fe| outline.functions.get(fe.0 as usize)).map(|d| d.return_type);
        };
        match table.get(t) {
            Type::Function { ret, .. } => Some(*ret),
            _ => None,
        }
    };
    for s in a.stmts.iter() {
        let StmtKind::Return(Some(v)) = &s.kind else { continue };
        // A função mais interna que contém o `return`.
        let mais_interna = a
            .functions
            .iter()
            .enumerate()
            .filter(|(_, f)| dentro(s.span, f.span))
            .min_by_key(|(_, f)| f.span.end - f.span.start)
            .map(|(k, f)| (ast::FunctionId(k as u32), f.span));
        let Some((f, fspan)) = mais_interna else { continue };
        // Um construtor dentro da função (não acontece) ou a função dentro
        // de um construtor: vale a mais interna; o construtor mais interno
        // que a função tira a checagem.
        if construtores.iter().any(|c| dentro(s.span, *c) && dentro(*c, fspan) && *c != fspan) {
            continue;
        }
        if let (Some(esperado), Some(t)) = (retorno_da_funcao(f), tipo(*v))
            && checar_void(program, table, core, esperado, t, true, closure_de_bloco(*v))
        {
            relatar(s.span, out);
        }
    }
    // Argumentos de criação de instância e de `MethodInvocation`.
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        let argumentos = match &e.kind {
            ExprKind::InstanceCreation { arguments, .. } => arguments,
            ExprKind::Call { target, arguments } => {
                let construtor = matches!(corpo.get_resolved(id), Some(Resolved::Constructor(_)));
                if !construtor && !e_invocacao_de_metodo(program, a, corpo, *target) {
                    continue;
                }
                arguments
            }
            _ => continue,
        };
        for arg in argumentos.args.iter() {
            if let (Some(&p), Some(t)) = (corpo.tipos_de_parametros.get(&arg.value), tipo(arg.value))
                && checar_void(program, table, core, p, t, false, closure_de_bloco(arg.value))
            {
                relatar(a.expr(arg.value).span, out);
            }
        }
    }
    // O padrão de variável atribuída.
    let mut atribuidos: Vec<ast::PatternId> = Vec::new();
    fn variaveis(a: &ast::Ast, p: ast::PatternId, v: &mut Vec<ast::PatternId>) {
        match &a.pattern(p).kind {
            PatternKind::Variable { .. } => v.push(p),
            PatternKind::Or(x, y) | PatternKind::And(x, y) => {
                variaveis(a, *x, v);
                variaveis(a, *y, v);
            }
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => variaveis(a, *x, v),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    if let ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) = el {
                        variaveis(a, *x, v);
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for en in entries.iter() {
                    variaveis(a, en.value, v);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    variaveis(a, f.pattern, v);
                }
            }
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        if let ExprKind::PatternAssign { pattern, .. } = &e.kind {
            variaveis(a, *pattern, &mut atribuidos);
        }
    }
    for p in atribuidos {
        let (Some(&valor), Some(&decl)) = (corpo.tipos_casados.get(&p), corpo.declaracoes_de_padroes.get(&p)) else { continue };
        let Some(esperado) = corpo.tipo_local(decl) else { continue };
        if checar_void(program, table, core, esperado, valor, false, false) {
            relatar(a.pattern(p).span, out);
        }
    }
}

/// A espécie do membro: método (com operador), getter ou setter.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Especie {
    Metodo,
    Getter,
    Setter,
}

/// Um parâmetro de um membro: nome, espécie, `covariant` escrito, tipo e o
/// `defaultValueCode`.
struct ParametroDoMembro {
    nome: String,
    /// `_sameKind`: 0 obrigatório (posicional ou nomeado `required`), 1
    /// posicional opcional, 2 nomeado opcional.
    especie: u8,
    covariante: bool,
    tipo: TypeId,
    padrao: Option<String>,
}

/// `unnecessary_overrides`.
#[allow(clippy::too_many_arguments)]
fn sobrescritas_desnecessarias(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpo: &UnitBodyTypes,
    u: UnitId,
    out: &mut Vec<Achado>,
) {
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let fonte = unidade.source.as_str();
    let lib = unidade.library;
    let comentarios = dartforge_frontend::comentarios::Comentarios::de(fonte);
    let elementos = elementos_das_funcoes(program, u);
    let protegido = |u2: UnitId, m: &ast::Annotation| crate::anotacoes::e_getter_de(program, interner, u2, m, "meta", "protected");
    // As anotações de um membro (o do método, ou o do campo do acessor implícito).
    let metadados_do_membro = |f: FunctionElementId| -> Option<(UnitId, Vec<&ast::Annotation>)> {
        let fe = program.function(f);
        match fe.node {
            FunctionRef::Function { unit, function } => {
                let a2 = &program.unit(unit).ast;
                let m = a2.members.iter().find(|m| matches!(m.kind, MemberKind::Method(g) if g == function))?;
                Some((unit, m.metadata.iter().collect()))
            }
            _ => {
                let v = fe.variable?;
                match program.variable(v).node {
                    dartforge_elements::model::VariableRef::Field { unit, member, .. } => {
                        Some((unit, program.unit(unit).ast.member(member).metadata.iter().collect()))
                    }
                    _ => None,
                }
            }
        }
    };
    let tem_protegido = |f: FunctionElementId| metadados_do_membro(f).is_some_and(|(u2, ms)| ms.iter().any(|m| protegido(u2, m)));
    for d in a.decls.iter() {
        let (membros, e_interface) = match &d.kind {
            DeclKind::Class(x) => (&x.members, true),
            DeclKind::Mixin(x) => (&x.members, true),
            DeclKind::Enum(x) => (&x.members, true),
            DeclKind::ExtensionType(x) => (&x.members, true),
            _ => continue,
        };
        if !e_interface {
            continue;
        }
        for &mid in membros {
            let m = a.member(mid);
            let MemberKind::Method(fid) = &m.kind else { continue };
            let f = a.function(*fid);
            if f.static_ {
                continue;
            }
            let Some(nome) = f.name else { continue };
            let texto = interner.resolve(nome.sym);
            if texto == "noSuchMethod" {
                continue;
            }
            // O comentário de documentação.
            let depois = match m.metadata.last() {
                Some(x) => dartforge_frontend::fonte::pular_brancos(fonte.as_bytes(), x.span.end),
                None => m.span.start,
            };
            if comentarios.dart_doc(fonte, depois).is_some() || m.metadata.iter().rev().any(|x| comentarios.dart_doc(fonte, x.span.start).is_some()) {
                continue;
            }
            let Some(&proprio) = elementos.get(fid) else { continue };
            let Some(classe) = program.function(proprio).class else { continue };
            let especie = match f.kind {
                ast::FunctionKind::Getter => Especie::Getter,
                ast::FunctionKind::Setter => Especie::Setter,
                _ => Especie::Metodo,
            };
            let chave = match especie {
                Especie::Setter => interner.lookup(&format!("{texto}_=")),
                _ => Some(nome.sym),
            };
            let Some(chave) = chave else { continue };
            let Some((dona, herdado)) = herdado_concreto(program, classe, chave, lib, texto.starts_with('_'), especie) else { continue };
            // `_addsMetadata`.
            let herdado_protegido = tem_protegido(herdado);
            let acrescenta = m.metadata.iter().any(|x| {
                let e_override = crate::anotacoes::e_getter_de(program, interner, u, x, "dart.core", "override");
                let e_protegido = protegido(u, x);
                !(e_override || (e_protegido && herdado_protegido))
            });
            if acrescenta {
                continue;
            }
            // `_haveSameDeclaration`.
            let Some(proprios) = parametros_do_membro(program, interner, table, outline, proprio, None) else { continue };
            let this_type = {
                let params: Vec<TypeId> = outline
                    .classes
                    .get(classe.0 as usize)
                    .map(|x| x.type_params.iter().map(|p| table.intern(Type::TypeParameter { param: *p, nullable: false })).collect())
                    .unwrap_or_default();
                table.intern(Type::Interface { class: classe, args: params.into(), nullable: false })
            };
            let Some(instanciado) = outline.hierarchy.supertype_of(this_type, dona, table, core) else { continue };
            let Some(herdados) = parametros_do_membro(program, interner, table, outline, herdado, Some((dona, instanciado))) else { continue };
            let (ret_proprio, ps_proprios) = proprios;
            let (ret_herdado, ps_herdados) = herdados;
            if crate::ops::sem_exibicao(ret_proprio, table) != crate::ops::sem_exibicao(ret_herdado, table) || ps_proprios.len() != ps_herdados.len() {
                continue;
            }
            let mesmos = ps_proprios.iter().zip(ps_herdados.iter()).all(|(p, q)| {
                crate::ops::sem_exibicao(p.tipo, table) == crate::ops::sem_exibicao(q.tipo, table)
                    && p.nome == q.nome
                    && p.covariante == q.covariante
                    && p.especie == q.especie
                    && p.padrao == q.padrao
            });
            if !mesmos {
                continue;
            }
            // `_makesPublicFromProtected`.
            let proprio_protegido = m.metadata.iter().any(|x| protegido(u, x));
            if !proprio_protegido && herdado_protegido {
                continue;
            }
            if repassa_ao_super(a, fonte, interner, &comentarios, corpo, f, especie, texto) {
                out.push((nome.span, "unnecessary_overrides", Vec::new()));
            }
        }
    }
}

/// `lookUp{Method,Getter,Setter}2(concrete: true, inherited: true)`: a
/// classe que declara e o membro.
fn herdado_concreto(program: &Program, classe: ClassId, chave: SymbolId, lib: LibraryId, privado: bool, especie: Especie) -> Option<(ClassId, FunctionElementId)> {
    let serve = |c: ClassId| -> Option<FunctionElementId> {
        let e = program.class(c);
        if privado && e.library != lib {
            return None;
        }
        let f = *e.instance_members.get(&chave)?;
        let m = program.function(f);
        let da_especie = match especie {
            Especie::Metodo => matches!(m.kind, FunctionKind::Function | FunctionKind::Operator),
            Especie::Getter => m.kind == FunctionKind::Getter,
            Especie::Setter => m.kind == FunctionKind::Setter,
        };
        (!m.abstract_ && da_especie).then_some(f)
    };
    let mut vistos: HashSet<ClassId> = HashSet::new();
    let mut atual = Some(classe);
    let mut primeira = true;
    while let Some(c) = atual {
        if !vistos.insert(c) {
            break;
        }
        if !primeira && let Some(f) = serve(c) {
            return Some((c, f));
        }
        primeira = false;
        let e = program.class(c);
        for &mx in e.mixin_classes.iter().rev() {
            if let Some(f) = serve(mx) {
                return Some((mx, f));
            }
        }
        atual = e.supertype_class;
    }
    None
}

/// O tipo de retorno e os parâmetros (na ordem escrita) de um membro;
/// `instanciado`: a classe dona e o supertipo instanciado em que o membro é
/// visto (a substituição dos parâmetros de tipo dela).
fn parametros_do_membro(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    outline: &OutlineTypes,
    f: FunctionElementId,
    instanciado: Option<(ClassId, TypeId)>,
) -> Option<(TypeId, Vec<ParametroDoMembro>)> {
    let dados = outline.functions.get(f.0 as usize)?;
    let mut sig = dados.signature;
    if let Some((dona, sup)) = instanciado {
        let params = outline.classes.get(dona.0 as usize).map(|x| x.type_params.clone()).unwrap_or_default();
        if let Type::Interface { args, .. } = table.get(sup).clone() {
            let mapa: HashMap<crate::table::TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            sig = crate::ops::substitute(sig, &mapa, table);
        }
    }
    let Type::Function { positional, optional, named, ret, .. } = table.get(sig).clone() else { return None };
    let fe = program.function(f);
    let mut v = Vec::new();
    match fe.node {
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            let fonte = program.unit(unit).source.as_str();
            let ps = a.function(function).parameters.as_deref().unwrap_or(&[]);
            let (mut i_pos, mut i_opc) = (0usize, 0usize);
            for p in ps {
                let tipo = match p.kind {
                    ParameterKind::Required => {
                        i_pos += 1;
                        *positional.get(i_pos - 1)?
                    }
                    ParameterKind::Named => {
                        let n = p.public_name.or(p.name)?.sym;
                        named.iter().find(|(s, _, _)| *s == n).map(|(_, t, _)| *t)?
                    }
                    _ => {
                        i_opc += 1;
                        *optional.get(i_opc - 1)?
                    }
                };
                v.push(ParametroDoMembro {
                    nome: p.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default(),
                    especie: match p.kind {
                        ParameterKind::Required => 0,
                        ParameterKind::Named if p.required => 0,
                        ParameterKind::Named => 2,
                        _ => 1,
                    },
                    covariante: p.covariant,
                    tipo,
                    padrao: p.default_value.map(|d| dartforge_frontend::fonte::de_expr(a, fonte, interner, d)),
                });
            }
        }
        _ => {
            // O acessor implícito de um campo: o setter tem um parâmetro
            // `_<nome>`.
            if fe.kind == FunctionKind::Setter {
                let v_id = fe.variable?;
                let nome = interner.resolve(program.variable(v_id).name);
                let covariante = match program.variable(v_id).node {
                    dartforge_elements::model::VariableRef::Field { unit, member, .. } => match &program.unit(unit).ast.member(member).kind {
                        MemberKind::Field(l) => l.covariant,
                        _ => false,
                    },
                    _ => false,
                };
                v.push(ParametroDoMembro {
                    nome: format!("_{nome}"),
                    especie: 0,
                    covariante,
                    tipo: *positional.first()?,
                    padrao: None,
                });
            }
        }
    }
    Some((ret, v))
}

/// O corpo repassa ao `super` (os visitantes do emissor).
#[allow(clippy::too_many_arguments)]
fn repassa_ao_super(
    a: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    comentarios: &dartforge_frontend::comentarios::Comentarios,
    corpo: &UnitBodyTypes,
    f: &ast::Function,
    especie: Especie,
    nome: &str,
) -> bool {
    let comentado = |pos: usize| !comentarios.antes_de(fonte, pos).is_empty();
    // `visitSuperExpression`.
    let super_sem_comentario = |e: ExprId| {
        let e = sem_parenteses(a, e);
        matches!(a.expr(e).kind, ExprKind::Super) && !comentado(a.expr(e).span.start)
    };
    let ps = f.parameters.as_deref().unwrap_or(&[]);
    // O parâmetro do próprio método que a expressão cita (`canonicalElement`).
    let parametro = |e: ExprId| -> Option<SymbolId> {
        let e = sem_parenteses(a, e);
        match (&a.expr(e).kind, corpo.get_resolved(e)) {
            (ExprKind::Identifier(n), Some(Resolved::Parameter { .. })) if ps.iter().any(|p| p.name.is_some_and(|q| q.sym == n.sym)) => Some(n.sym),
            _ => None,
        }
    };
    let visitar_expr = |e: ExprId| -> bool {
        let e = sem_parenteses(a, e);
        match especie {
            Especie::Getter => match &a.expr(e).kind {
                ExprKind::Property { target, name, .. } if interner.resolve(name.sym) == nome => super_sem_comentario(*target),
                _ => false,
            },
            Especie::Setter => match &a.expr(e).kind {
                ExprKind::Assign { target, value, .. } => {
                    let [p] = ps else { return false };
                    if p.name.map(|n| n.sym) != parametro(*value) {
                        return false;
                    }
                    match &a.expr(sem_parenteses(a, *target)).kind {
                        ExprKind::Property { target, name, .. } if interner.resolve(name.sym) == nome => super_sem_comentario(*target),
                        _ => false,
                    }
                }
                _ => false,
            },
            Especie::Metodo if f.kind == ast::FunctionKind::Operator => match &a.expr(e).kind {
                ExprKind::Binary { op, left, right } => {
                    let [p] = ps else { return false };
                    texto_do_operador(*op) == Some(nome) && p.name.map(|n| n.sym) == parametro(*right) && super_sem_comentario(*left)
                }
                ExprKind::Unary { op, operand } => {
                    let mesmo = match op {
                        UnaryOp::Neg => nome == "unary-" || nome == "-",
                        UnaryOp::BitNot => nome == "~",
                        _ => false,
                    };
                    mesmo && ps.is_empty() && super_sem_comentario(*operand)
                }
                _ => false,
            },
            Especie::Metodo => match &a.expr(e).kind {
                ExprKind::Call { target, arguments } => {
                    let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind else { return false };
                    if interner.resolve(name.sym) != nome {
                        return false;
                    }
                    // `argumentsMatchParameters`.
                    let posicionais: Vec<SymbolId> = ps.iter().filter(|p| p.kind != ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
                    let nomeados: Vec<SymbolId> = ps.iter().filter(|p| p.kind == ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
                    let mut args_pos: Vec<SymbolId> = Vec::new();
                    let mut args_nom: Vec<(SymbolId, SymbolId)> = Vec::new();
                    for x in arguments.args.iter() {
                        let Some(el) = parametro(x.value) else { return false };
                        match x.name {
                            Some(n) => args_nom.push((n.sym, el)),
                            None => args_pos.push(el),
                        }
                    }
                    if posicionais != args_pos || nomeados.len() != args_nom.len() {
                        return false;
                    }
                    let todos_nomeados = nomeados.iter().all(|n| {
                        let externo = ps.iter().find(|p| p.name.is_some_and(|q| q.sym == *n)).and_then(|p| p.public_name.or(p.name)).map(|q| q.sym);
                        args_nom.iter().any(|(rotulo, el)| Some(*rotulo) == externo && el == n)
                    });
                    todos_nomeados && super_sem_comentario(*alvo)
                }
                _ => false,
            },
        }
    };
    match &f.body {
        FunctionBody::Expression(e) => visitar_expr(*e),
        FunctionBody::Block(b) => {
            let StmtKind::Block(cmds) = &a.stmt(*b).kind else { return false };
            let [s] = &cmds[..] else { return false };
            match &a.stmt(*s).kind {
                StmtKind::Expression(e) => visitar_expr(*e),
                StmtKind::Return(v) => {
                    if comentado(a.stmt(*s).span.start) {
                        return false;
                    }
                    v.is_some_and(visitar_expr)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// O texto do operador binário (o nome do `operator` que ele chama).
fn texto_do_operador(op: BinaryOp) -> Option<&'static str> {
    Some(match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::TruncDiv => "~/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Eq => "==",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        _ => return None,
    })
}

/// `no_duplicate_case_values`.
#[allow(clippy::too_many_arguments)]
fn casos_duplicados(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpos: &BodyTypes,
    inferidas: &HashSet<LibraryId>,
    u: UnitId,
    out: &mut Vec<Achado>,
) {
    let unidade = program.unit(u);
    let versao = program.library(unidade.library).features.versao();
    if versao >= dartforge_frontend::features::LanguageVersion::new(3, 0) {
        return;
    }
    let a = &unidade.ast;
    let fonte = unidade.source.as_str();
    let switches: Vec<&[ast::SwitchCase]> = a
        .stmts
        .iter()
        .filter_map(|s| match &s.kind {
            StmtKind::Switch { cases, .. } => Some(&cases[..]),
            _ => None,
        })
        .collect();
    if switches.is_empty() {
        return;
    }
    let mut motor = Motor::novo(program, interner, table, core, outline, corpos, inferidas);
    let cx = Ctx::simples(u, unidade.library);
    for casos in switches {
        let mut vistos: Vec<(crate::constantes::valor::Valor, ExprId)> = Vec::new();
        for caso in casos {
            let Some(p) = caso.pattern else { continue };
            let PatternKind::Constant(e) = &a.pattern(p).kind else { continue };
            let Constante::Valor(v) = motor.avaliar(&cx, *e, true) else { continue };
            if v.estado.desconhecido() {
                continue;
            }
            match vistos.iter().position(|(w, _)| motor.iguais(&v, w)) {
                Some(i) => {
                    let anterior = vistos[i].1;
                    let args = vec![dartforge_frontend::fonte::de_expr(a, fonte, interner, *e), dartforge_frontend::fonte::de_expr(a, fonte, interner, anterior)];
                    out.push((a.expr(*e).span, "no_duplicate_case_values", args));
                }
                None => vistos.push((v, *e)),
            }
        }
    }
}
