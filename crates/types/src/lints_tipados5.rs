//! Regras de lint tipadas do conjunto `recommended`
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`). Como
//! em [`crate::lints_tipados`], devolvem achados neutros.
//!
//! * `exhaustive_cases`: o `switch` (comando) sobre uma classe "enum-like"
//!   (`asEnumLikeClass`: concreta, só construtores privados que não são
//!   fábrica, duas ou mais constantes estáticas do tipo da classe, nenhuma
//!   subclasse na unidade que define a biblioteca), tirando os valores dos
//!   casos (identificador ou acesso a propriedade) pelo valor constante; o
//!   `default` cala. Relata cada valor que sobrou, do `switch` ao `)`, com o
//!   nome do primeiro campo não `@deprecated` do valor.
//! * `use_super_parameters`: o `super(…)` dos inicializadores, os
//!   posicionais que são parâmetros do construtor (na ordem, sem `this.`/
//!   `super.` e não citados no corpo) e os nomeados repassados com o mesmo
//!   nome, com o tipo do parâmetro do construtor da superclasse atribuível
//!   ao do parâmetro.
//! * `invalid_runtime_check_with_js_interop_types`: o `canBeSubtypeOf` com o
//!   apagamento que mantém os tipos de `dart:js_interop`
//!   (`EraseNonJSInteropTypes`) e decide o código na primeira comparação que
//!   o pede.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::constantes::avaliador::{Constante, Motor};
use crate::lints_tipados::Achado;
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved, UnitBodyTypes};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, LibraryId, Program, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, ExprId, ExprKind, Initializer, MemberKind, ParameterKind, PatternKind, StmtKind};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

fn sem_parenteses(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

/// Os achados das regras deste módulo na unidade `u`.
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
    casos_exaustivos(program, interner, table, core, outline, corpos, inferidas, u, &mut out);
    constantes_nomeadas_e_declaracoes(program, interner, table, core, outline, corpos, inferidas, u, &mut out);
    parametros_super(program, interner, table, core, outline, corpo, u, &mut out);
    checagens_js(program, interner, table, core, outline, corpo, u, &mut out);
    out
}

/// O `thisType` de uma classe.
fn tipo_this(table: &mut TypeTable, outline: &OutlineTypes, c: ClassId) -> TypeId {
    let params: Vec<TypeId> = outline
        .classes
        .get(c.0 as usize)
        .map(|x| x.type_params.iter().map(|p| table.intern(Type::TypeParameter { param: *p, nullable: false })).collect())
        .unwrap_or_default();
    table.intern(Type::Interface { class: c, args: params.into(), nullable: false })
}

/// `exhaustive_cases`.
#[allow(clippy::too_many_arguments)]
fn casos_exaustivos(
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
    let a = &unidade.ast;
    let fonte = unidade.source.as_str();
    let Some(corpo) = corpos.units.get(u.0 as usize) else { return };
    let switches: Vec<(Span, ExprId, &[ast::SwitchCase])> = a
        .stmts
        .iter()
        .filter_map(|s| match &s.kind {
            StmtKind::Switch { value, cases } => Some((s.span, *value, &cases[..])),
            _ => None,
        })
        .collect();
    if switches.is_empty() {
        return;
    }
    let deprecado = |v: VariableId| -> bool {
        let VariableRef::Field { unit, member, .. } = program.variable(v).node else { return false };
        let m = program.unit(unit).ast.member(member);
        m.metadata.iter().any(|x| {
            crate::anotacoes::e_getter_de(program, interner, unit, x, "dart.core", "deprecated")
                || crate::anotacoes::e_construtor_de(program, interner, unit, x, "dart.core", "Deprecated")
        })
    };
    // Primeira passada (com a tabela): as candidatas e o `thisType`.
    let mut candidatas: Vec<(Span, ExprId, &[ast::SwitchCase], ClassId, TypeId)> = Vec::new();
    for (span, valor, casos) in switches {
        let Some(t) = corpo.get_type(valor) else { continue };
        let Type::Interface { class, .. } = table.get(t).clone() else { continue };
        let this_t = tipo_this(table, outline, class);
        candidatas.push((span, valor, casos, class, this_t));
    }
    if candidatas.is_empty() {
        return;
    }
    let mut motor = Motor::novo(program, interner, table, core, outline, corpos, inferidas);
    let exaustivos_ligado = true;
    for (span, valor, casos, class, this_t) in candidatas {
        // `no_default_cases`: o `default` de um `switch` sobre enum ou
        // classe "enum-like" (o primeiro `default`).
        let e_enum = program.class(class).kind == ClassKind::Enum;
        let k = program.class(class);
        let padrao_do_switch = casos.iter().find(|c| c.pattern.is_none()).map(|c| c.span);
        if e_enum {
            if let Some(sp) = padrao_do_switch {
                out.push((sp, "no_default_cases", Vec::new()));
            }
            continue;
        }
        if !matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication) || k.modifiers.abstract_ {
            continue;
        }
        // Só construtores privados que não são fábrica (sem nenhum
        // declarado, o padrão é público).
        let construtores = k.construtores();
        if construtores.is_empty() {
            continue;
        }
        let todos_privados = construtores.iter().all(|(nome, f)| interner.resolve(*nome).starts_with('_') && !program.function(*f).factory);
        if !todos_privados {
            continue;
        }
        // As constantes estáticas do tipo da classe, por valor.
        let mut constantes: Vec<(crate::constantes::valor::Valor, Vec<VariableId>)> = Vec::new();
        let mut contagem = 0;
        for &v in &k.fields {
            let x = program.variable(v);
            if !x.const_ || !x.static_ {
                continue;
            }
            let Some(d) = outline.variables.get(v.0 as usize) else { continue };
            let Some(tv) = d.declared_type.or(d.inferred) else { continue };
            if tv != this_t {
                continue;
            }
            let Some(Constante::Valor(val)) = motor.valor_de_variavel(v) else { continue };
            contagem += 1;
            match constantes.iter().position(|(w, _)| motor.iguais(&val, w)) {
                Some(i) => constantes[i].1.push(v),
                None => constantes.push((val, vec![v])),
            }
        }
        if contagem < 2 {
            continue;
        }
        // Nenhuma subclasse na unidade que define a biblioteca.
        let definidora = program.library(k.library).units.first().copied();
        let tem_subclasse = (0..program.classes.len()).map(|i| ClassId(i as u32)).any(|cand| {
            let ck = program.class(cand);
            if !ck.decl.is_some_and(|r| Some(r.unit) == definidora) {
                return false;
            }
            let mut vistos = HashSet::new();
            let mut atual = ck.supertype_class;
            while let Some(sc) = atual {
                if !vistos.insert(sc) {
                    break;
                }
                if sc == class {
                    return true;
                }
                atual = program.class(sc).supertype_class;
            }
            false
        });
        if tem_subclasse {
            continue;
        }
        // A classe é "enum-like": o `default` é `no_default_cases`.
        if let Some(sp) = padrao_do_switch {
            out.push((sp, "no_default_cases", Vec::new()));
        }
        let mut padrao = false;
        for caso in casos {
            let Some(p) = caso.pattern else {
                padrao = true;
                break;
            };
            let mut p = p;
            while let PatternKind::Parenthesized(x) = &a.pattern(p).kind {
                p = *x;
            }
            let PatternKind::Constant(e) = &a.pattern(p).kind else { continue };
            let e = sem_parenteses(a, *e);
            if !matches!(a.expr(e).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) {
                continue;
            }
            let variavel = match corpo.get_resolved(e) {
                Some(Resolved::Member { member: MemberRef::Variable(v), .. }) | Some(Resolved::Element(Element::Variable(v))) => Some(*v),
                Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) => program.function(*f).variable,
                _ => None,
            };
            let Some(v) = variavel else { continue };
            if let Some(Constante::Valor(val)) = motor.valor_de_variavel(v)
                && let Some(i) = constantes.iter().position(|(w, _)| motor.iguais(&val, w))
            {
                constantes.remove(i);
            }
        }
        if padrao {
            continue;
        }
        if !exaustivos_ligado {
            continue;
        }
        // Do `switch` ao `)` depois da expressão.
        let b = fonte.as_bytes();
        let depois = dartforge_frontend::fonte::pular_brancos(b, a.expr(valor).span.end);
        let fim = if b.get(depois) == Some(&b')') { depois + 1 } else { a.expr(valor).span.end };
        for (_, campos) in constantes {
            let preferido = campos.iter().copied().find(|v| !deprecado(*v)).unwrap_or(campos[0]);
            let nome = interner.resolve(program.variable(preferido).name).to_string();
            out.push((Span { start: span.start, end: fim }, "exhaustive_cases", vec![nome]));
        }
    }
}

/// `use_named_constants` e `prefer_const_declarations` (o motor de
/// constantes).
#[allow(clippy::too_many_arguments)]
fn constantes_nomeadas_e_declaracoes(
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
    let a = &unidade.ast;
    let fonte = unidade.source.as_str();
    let lib = unidade.library;
    let Some(corpo) = corpos.units.get(u.0 as usize) else { return };
    let pais = crate::lints_tipados::pais_da_unidade(program, u);
    // `use_named_constants`: as criações constantes de uma classe.
    let mut criacoes: Vec<(ExprId, ClassId)> = Vec::new();
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        if !matches!(corpo.get_resolved(id), Some(Resolved::Constructor(_))) {
            continue;
        }
        let escrito_const = matches!(&e.kind, ExprKind::InstanceCreation { keyword: Some(ast::CreationKeyword::Const), .. });
        if !escrito_const && !pais.em_contexto_constante(a, id) {
            continue;
        }
        let Some(t) = corpo.get_type(id) else { continue };
        let Type::Interface { class, .. } = table.get(t) else { continue };
        if program.class(*class).kind != ClassKind::Class {
            continue;
        }
        // Dentro do inicializador de um campo da própria classe.
        let dentro_de_campo = program.class(*class).fields.iter().any(|v| match program.variable(*v).node {
            VariableRef::Field { unit, member, index } if unit == u => match &a.member(member).kind {
                MemberKind::Field(l) => l.variables.get(index).and_then(|x| x.initializer).is_some_and(|i| {
                    let (ie, ce) = (a.expr(i).span, e.span);
                    ce.start >= ie.start && ce.end <= ie.end
                }),
                _ => false,
            },
            _ => false,
        });
        if !dentro_de_campo {
            criacoes.push((id, *class));
        }
    }
    // `prefer_const_declarations`: as listas `final` (campos estáticos, de
    // topo e locais) com o começo da lista e os inicializadores.
    let mut listas: Vec<(usize, usize, Vec<ExprId>)> = Vec::new();
    let lista = |inicio: usize, l: &ast::VariableList, listas: &mut Vec<(usize, usize, Vec<ExprId>)>| {
        if l.const_ || !l.final_ {
            return;
        }
        let mut inits = Vec::new();
        for v in l.variables.iter() {
            let Some(i) = v.initializer else { return };
            // `TypedLiteral` sem `const`.
            if matches!(a.expr(i).kind, ExprKind::List { const_: false, .. } | ExprKind::SetOrMap { const_: false, .. }) {
                return;
            }
            inits.push(i);
        }
        let Some(ultima) = l.variables.last() else { return };
        let fim = ultima.initializer.map_or(ultima.name.span.end, |i| a.expr(i).span.end);
        listas.push((inicio, fim, inits));
    };
    let pular_palavra = |pos: usize, p: &str| -> usize {
        if fonte[pos..].starts_with(p) && !fonte.as_bytes().get(pos + p.len()).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
            dartforge_frontend::fonte::pular_brancos(fonte.as_bytes(), pos + p.len())
        } else {
            pos
        }
    };
    let depois_das_anotacoes = |metadata: &[ast::Annotation], inicio: usize| match metadata.last() {
        Some(m) => dartforge_frontend::fonte::pular_brancos(fonte.as_bytes(), m.span.end),
        None => inicio,
    };
    for d in a.decls.iter() {
        if let ast::DeclKind::Variables(l) = &d.kind {
            let ini = depois_das_anotacoes(&d.metadata, d.span.start);
            lista(ini, l, &mut listas);
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Field(l) = &m.kind
            && l.static_
        {
            let ini = pular_palavra(depois_das_anotacoes(&m.metadata, m.span.start), "static");
            lista(ini, l, &mut listas);
        }
    }
    for (k, st) in a.stmts.iter().enumerate() {
        if let StmtKind::Variables(l) = &st.kind {
            let meta = a.metadados_locais.iter().find(|(x, _)| x.0 as usize == k).map_or(&[][..], |(_, m)| &m[..]);
            let ini = depois_das_anotacoes(meta, st.span.start);
            lista(ini, l, &mut listas);
        }
    }
    if criacoes.is_empty() && listas.is_empty() {
        return;
    }
    let mut motor = Motor::novo(program, interner, table, core, outline, corpos, inferidas);
    let cx = crate::constantes::avaliador::Ctx::simples(u, lib);
    for (e, class) in criacoes {
        let Constante::Valor(valor) = motor.avaliar(&cx, e, true) else { continue };
        let k = program.class(class);
        for &v in &k.fields {
            let x = program.variable(v);
            if !x.static_ || !x.const_ {
                continue;
            }
            let nome = interner.resolve(x.name);
            if nome.starts_with('_') && k.library != lib {
                continue;
            }
            if let Some(Constante::Valor(w)) = motor.valor_de_variavel(v)
                && motor.iguais(&valor, &w)
            {
                let texto = format!("{}.{}", interner.resolve(k.name), nome);
                out.push((a.expr(e).span, "use_named_constants", vec![texto]));
                break;
            }
        }
    }
    for (ini, fim, inits) in listas {
        let sem_erro = inits.iter().all(|i| matches!(motor.avaliar(&cx, *i, true), Constante::Valor(_)));
        if sem_erro {
            out.push((Span { start: ini, end: fim }, "prefer_const_declarations", Vec::new()));
        }
    }
}

/// `use_super_parameters`.
#[allow(clippy::too_many_arguments)]
fn parametros_super(
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
    // `Feature.super_parameters` (2.17).
    if program.library(unidade.library).features.versao() < dartforge_frontend::features::LanguageVersion::new(2, 17) {
        return;
    }
    let a = &unidade.ast;
    for (k_decl, d) in a.decls.iter().enumerate() {
        let membros = match &d.kind {
            ast::DeclKind::Class(x) => &x.members,
            ast::DeclKind::Enum(x) => &x.members,
            _ => continue,
        };
        let did = ast::DeclId(k_decl as u32);
        let Some(classe) = (0..program.classes.len()).map(|i| ClassId(i as u32)).find(|c| program.class(*c).decl.is_some_and(|r| r.unit == u && r.decl == did)) else {
            continue;
        };
        for &mid in membros {
            let MemberKind::Constructor(k) = &a.member(mid).kind else { continue };
            // O último `super(…)` dos inicializadores.
            let Some((nome_super, args)) = k.initializers.iter().rev().find_map(|i| match i {
                Initializer::Super { constructor, arguments, .. } => Some((*constructor, arguments)),
                _ => None,
            }) else {
                continue;
            };
            // O construtor da superclasse (o `staticElement`).
            let Some(sup) = program.class(classe).supertype_class else { continue };
            let chave = match nome_super {
                Some(n) => n.sym,
                None => match interner.lookup("") {
                    Some(s) => s,
                    None => continue,
                },
            };
            let Some(&ksup) = program.class(sup).constructors.get(&chave) else { continue };
            // Os parâmetros citados no corpo.
            let corpo_span = match &k.body {
                ast::FunctionBody::Block(s) => Some(a.stmt(*s).span),
                ast::FunctionBody::Expression(e) => Some(a.expr(*e).span),
                _ => None,
            };
            let mut citados: HashSet<dartforge_intern::SymbolId> = HashSet::new();
            if let Some(cs) = corpo_span {
                for (i, e) in a.exprs.iter().enumerate() {
                    if e.span.start >= cs.start
                        && e.span.end <= cs.end
                        && let ExprKind::Identifier(n) = &e.kind
                        && matches!(corpo.get_resolved(ExprId(i as u32)), Some(Resolved::Parameter { .. }))
                    {
                        citados.insert(n.sym);
                    }
                }
            }
            let ps = &k.parameters;
            // `_checkForConvertiblePositionalParams`.
            let mut posicionais: Vec<ast::Name> = Vec::new();
            let mut invalido = false;
            for x in args.args.iter() {
                match x.name {
                    None => match &a.expr(x.value).kind {
                        ExprKind::Identifier(n) => posicionais.push(*n),
                        _ => {
                            invalido = true;
                            break;
                        }
                    },
                    Some(_) => {}
                }
            }
            if invalido {
                continue;
            }
            let mut identificadores: Vec<String> = Vec::new();
            let mut vistos: HashSet<dartforge_intern::SymbolId> = HashSet::new();
            let mut indice_casado = 0usize;
            let mut aborta = false;
            'args: for n in &posicionais {
                // O elemento do argumento é um parâmetro deste construtor
                // (o escopo dos inicializadores).
                let Some(p) = ps.iter().find(|p| p.name.is_some_and(|q| q.sym == n.sym)) else {
                    aborta = true;
                    break;
                };
                if p.kind == ParameterKind::Named || !vistos.insert(n.sym) {
                    aborta = true;
                    break;
                }
                let mut casou = false;
                for (i, q) in ps.iter().enumerate() {
                    if casou {
                        break;
                    }
                    if q.this_ || q.super_ {
                        aborta = true;
                        break 'args;
                    }
                    let Some(qn) = q.name else { continue };
                    if citados.contains(&qn.sym) {
                        aborta = true;
                        break 'args;
                    }
                    if qn.sym == n.sym {
                        casou = true;
                        identificadores.push(interner.resolve(qn.sym).to_string());
                        if i < indice_casado {
                            aborta = true;
                            break 'args;
                        }
                        indice_casado = i;
                    }
                }
            }
            if aborta {
                continue;
            }
            // Os nomeados.
            let tipos_sup = parametros_nomeados(program, table, outline, ksup, classe, sup, core);
            for p in ps.iter() {
                if p.this_ || p.kind != ParameterKind::Named {
                    continue;
                }
                let Some(pn) = p.name else { continue };
                if citados.contains(&pn.sym) {
                    continue;
                }
                let Some(&tipo_sup) = tipos_sup.get(&pn.sym) else { continue };
                let repassado = args.args.iter().any(|x| {
                    x.name.is_some_and(|l| l.sym == pn.sym)
                        && matches!(&a.expr(x.value).kind, ExprKind::Identifier(m) if m.sym == pn.sym)
                        && matches!(corpo.get_resolved(x.value), Some(Resolved::Parameter { .. }) | None)
                });
                if !repassado {
                    continue;
                }
                // O tipo deste parâmetro.
                let tipo_proprio = p.ty.and_then(|t| outline.tipos_escritos.get(&(u, t)).copied()).unwrap_or(core.dynamic_);
                let atribuivel = matches!(table.get(tipo_sup), Type::Dynamic) || {
                    let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                    is_subtype(tipo_sup, tipo_proprio, &mut env)
                };
                if atribuivel {
                    identificadores.push(interner.resolve(pn.sym).to_string());
                }
            }
            if identificadores.is_empty() {
                continue;
            }
            let alvo = k.name.unwrap_or(k.class_name).span;
            if identificadores.len() > 1 {
                let citados: Vec<String> = identificadores.iter().map(|i| format!("'{i}'")).collect();
                let msg = match citados.len() {
                    2 => format!("{} and {}", citados[0], citados[1]),
                    n => format!("{}, and {}", citados[..n - 1].join(", "), citados[n - 1]),
                };
                out.push((alvo, "use_super_parameters_multiple", vec![msg]));
            } else {
                out.push((alvo, "use_super_parameters_single", vec![identificadores[0].clone()]));
            }
        }
    }
}

/// Os tipos dos parâmetros nomeados do construtor `f` da superclasse `sup`,
/// instanciados no supertipo da classe `classe`.
fn parametros_nomeados(
    program: &Program,
    table: &mut TypeTable,
    outline: &OutlineTypes,
    f: dartforge_elements::model::FunctionElementId,
    classe: ClassId,
    sup: ClassId,
    core: &CoreTypes,
) -> HashMap<dartforge_intern::SymbolId, TypeId> {
    let mut m = HashMap::new();
    let Some(dados) = outline.functions.get(f.0 as usize) else { return m };
    let mut sig = dados.signature;
    let this_t = tipo_this(table, outline, classe);
    if let Some(inst) = outline.hierarchy.supertype_of(this_t, sup, table, core)
        && let Type::Interface { args, .. } = table.get(inst).clone()
    {
        let params = outline.classes.get(sup.0 as usize).map(|x| x.type_params.clone()).unwrap_or_default();
        let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
        sig = crate::ops::substitute(sig, &mapa, table);
    }
    if let Type::Function { named, .. } = table.get(sig) {
        for (n, t, _) in named.iter() {
            m.insert(*n, *t);
        }
    }
    let _ = program;
    m
}

const WEB_SDK: [&str; 6] = ["dart:html", "dart:indexed_db", "dart:svg", "dart:web_audio", "dart:web_gl", "dart:js_util"];

/// As consultas de interop JS.
struct Js<'a> {
    program: &'a Program,
    interner: &'a Interner,
    outline: &'a OutlineTypes,
    core: &'a CoreTypes,
    /// O `JSObject` do `dart:js_interop`.
    js_object: Option<ClassId>,
}

impl Js<'_> {
    fn uri(&self, l: LibraryId) -> &str {
        &self.program.library(l).uri
    }

    fn limite(&self, table: &TypeTable, t: TypeId) -> TypeId {
        match table.get(t) {
            Type::TypeParameter { param, .. } => table.param(*param).bound,
            Type::Intersection { bound, .. } => *bound,
            _ => t,
        }
    }

    /// `isDartJsInteropType`.
    fn dart_js(&self, table: &TypeTable, t: TypeId) -> bool {
        match table.get(t) {
            Type::TypeParameter { .. } | Type::Intersection { .. } => self.dart_js(table, self.limite(table, t)),
            Type::ExtensionType { decl, .. } => self.uri(self.program.class(*decl).library) == "dart:js_interop",
            _ => false,
        }
    }

    /// O tipo de representação declarado de um tipo de extensão.
    fn representacao(&self, decl: ClassId) -> Option<TypeId> {
        let v = self.program.class(decl).representation?;
        self.outline.variables.get(v.0 as usize)?.declared_type
    }

    /// `getJsTypeForStaticInterop`: a classe com `@JS()` do
    /// `dart:js_interop` e `@staticInterop`.
    fn static_interop(&self, c: ClassId) -> bool {
        let k = self.program.class(c);
        if k.kind != ClassKind::Class {
            return false;
        }
        let Some(r) = k.decl else { return false };
        let a = &self.program.unit(r.unit).ast;
        let mut js = false;
        let mut estatico = false;
        for m in a.decl(r.decl).metadata.iter() {
            match crate::anotacoes::elemento_invocado(self.program, self.interner, r.unit, m) {
                crate::anotacoes::ElementoInvocado::Construtor(cl, _) => {
                    let x = self.program.class(cl);
                    if self.uri(x.library) == "dart:js_interop" && self.interner.resolve(x.name) == "JS" {
                        js = true;
                    }
                }
                crate::anotacoes::ElementoInvocado::Getter(f) => {
                    let g = self.program.function(f);
                    if self.uri(g.library) == "dart:_js_annotations" && self.interner.resolve(g.name) == "staticInterop" {
                        estatico = true;
                    }
                }
                _ => {}
            }
        }
        js && estatico && self.js_object.is_some()
    }

    /// `isUserJsInteropType`.
    fn usuario_js(&self, table: &TypeTable, t: TypeId, prof: u32) -> bool {
        if prof > 16 {
            return false;
        }
        match table.get(t) {
            Type::TypeParameter { .. } | Type::Intersection { .. } => self.usuario_js(table, self.limite(table, t), prof + 1),
            Type::ExtensionType { decl, .. } => self.representacao(*decl).is_some_and(|r| self.dart_js(table, r) || self.usuario_js(table, r, prof + 1)),
            Type::Interface { class, .. } => self.static_interop(*class),
            _ => false,
        }
    }

    /// `isWasmIncompatibleJsInterop`.
    fn incompativel(&self, table: &TypeTable, t: TypeId) -> bool {
        let (Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. }) = table.get(self.limite(table, t)) else { return false };
        let k = self.program.class(*class);
        let tem_js = k.decl.is_some_and(|r| {
            let a = &self.program.unit(r.unit).ast;
            a.decl(r.decl).metadata.iter().any(|m| match crate::anotacoes::elemento_invocado(self.program, self.interner, r.unit, m) {
                crate::anotacoes::ElementoInvocado::Construtor(cl, _) => {
                    let x = self.program.class(cl);
                    self.interner.resolve(x.name) == "JS" && (self.uri(x.library) == "package:js/js.dart" || self.uri(x.library) == "dart:_js_annotations")
                }
                _ => false,
            })
        });
        let uri = self.uri(k.library);
        tem_js || WEB_SDK.contains(&uri) || uri == "dart:js"
    }

    /// `EraseNonJSInteropTypes.perform`.
    fn apagar(&self, table: &mut TypeTable, t: TypeId, manter_usuario: bool, vistos: &mut HashSet<TypeParamId>) -> TypeId {
        let mantem = if manter_usuario { self.usuario_js(table, t, 0) } else { self.dart_js(table, t) };
        match table.get(t).clone() {
            Type::Interface { class, args, nullable } => {
                if mantem {
                    return tipo_this(table, self.outline, class);
                }
                if self.static_interop(class)
                    && let Some(jo) = self.js_object
                {
                    return table.intern(Type::ExtensionType { decl: jo, args: Box::new([]), nullable: false });
                }
                let novos: Vec<TypeId> = args.iter().map(|x| self.apagar(table, *x, manter_usuario, vistos)).collect();
                table.intern(Type::Interface { class, args: novos.into(), nullable })
            }
            Type::ExtensionType { decl, args, nullable } => {
                if mantem {
                    let params: Vec<TypeId> = self
                        .outline
                        .classes
                        .get(decl.0 as usize)
                        .map(|x| x.type_params.iter().map(|p| table.intern(Type::TypeParameter { param: *p, nullable: false })).collect())
                        .unwrap_or_default();
                    return table.intern(Type::ExtensionType { decl, args: params.into(), nullable: false });
                }
                // O apagamento: a representação instanciada.
                let Some(rep) = self.representacao(decl) else { return t };
                let params = self.outline.classes.get(decl.0 as usize).map(|x| x.type_params.clone()).unwrap_or_default();
                let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
                let inst = crate::ops::substitute(rep, &mapa, table);
                let apagado = self.apagar(table, inst, manter_usuario, vistos);
                if nullable { crate::ops::nullable(apagado, table) } else { apagado }
            }
            Type::TypeParameter { param, .. } => {
                if !vistos.insert(param) {
                    return t;
                }
                let limite = table.param(param).bound;
                let novo = self.apagar(table, limite, manter_usuario, vistos);
                table.intern(Type::Intersection { param, bound: novo })
            }
            Type::Intersection { param, bound } => {
                if !vistos.insert(param) {
                    return t;
                }
                let novo = self.apagar(table, bound, manter_usuario, vistos);
                table.intern(Type::Intersection { param, bound: novo })
            }
            Type::FutureOr { arg, nullable } => {
                let a2 = self.apagar(table, arg, manter_usuario, vistos);
                table.intern(Type::FutureOr { arg: a2, nullable })
            }
            _ => t,
        }
    }

    /// `extensionTypeErasure` comum.
    fn apagar_extensao(&self, table: &mut TypeTable, t: TypeId) -> TypeId {
        let outline = self.outline;
        let program = self.program;
        let rep = |decl: ClassId, args: &[TypeId], table: &mut TypeTable| -> Option<TypeId> {
            let v = program.class(decl).representation?;
            let t = outline.variables[v.0 as usize].declared_type?;
            let params = &outline.classes[decl.0 as usize].type_params;
            if params.is_empty() || params.len() != args.len() {
                return Some(t);
            }
            let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            Some(crate::ops::substitute(t, &mapa, table))
        };
        crate::ops::erase_extension_type(t, table, &rep)
    }

    fn nao_anulavel(&self, table: &TypeTable, t: TypeId) -> bool {
        match table.get(t) {
            Type::Dynamic | Type::Void | Type::Null => false,
            Type::Intersection { bound, .. } => self.nao_anulavel(table, *bound),
            Type::TypeParameter { param, nullable } => !*nullable && self.nao_anulavel(table, table.param(*param).bound),
            Type::FutureOr { arg, nullable } => !*nullable && self.nao_anulavel(table, *arg),
            Type::Interface { nullable, .. } | Type::ExtensionType { nullable, .. } => !*nullable,
            Type::Function { nullable, .. } | Type::Record { nullable, .. } => !*nullable,
            Type::Never => true,
        }
    }

    fn sub(&self, table: &mut TypeTable, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    /// O `eraseTypes` do emissor: decide o código (uma vez) e devolve os
    /// tipos para seguir a comparação.
    fn apagar_par(&self, table: &mut TypeTable, l: TypeId, r: TypeId, check: bool, codigo: &mut Option<&'static str>) -> (TypeId, TypeId) {
        let el0 = self.apagar(table, l, false, &mut HashSet::new());
        let mut el = crate::ops::non_nullable(el0, table);
        let er0 = self.apagar(table, r, false, &mut HashSet::new());
        let mut er = crate::ops::non_nullable(er0, table);
        let l_js = self.dart_js(table, el);
        let r_js = self.dart_js(table, er);
        if codigo.is_none() && (l_js || r_js) && !self.incompativel(table, el) && !self.incompativel(table, er) {
            let l_sub = self.sub(table, el, er);
            let r_sub = self.sub(table, er, el);
            let l_din = matches!(table.get(el), Type::Dynamic);
            let r_din = matches!(table.get(er), Type::Dynamic);
            if check {
                if !l_sub && !r_din {
                    *codigo = Some(if l_js && r_js {
                        "invalid_runtime_check_with_js_interop_types_js_is_inconsistent_js"
                    } else if l_js {
                        "invalid_runtime_check_with_js_interop_types_js_is_dart"
                    } else {
                        "invalid_runtime_check_with_js_interop_types_dart_is_js"
                    });
                } else if l_sub && l_js && r_js && self.usuario_js(table, r, 0) {
                    let ul = self.apagar(table, l, true, &mut HashSet::new());
                    let ur = self.apagar(table, r, true, &mut HashSet::new());
                    if !self.sub(table, ul, ur) {
                        *codigo = Some("invalid_runtime_check_with_js_interop_types_js_is_unrelated_js");
                    }
                }
            } else if !l_sub && !r_sub && !l_din && !r_din {
                *codigo = Some(if l_js && r_js {
                    "invalid_runtime_check_with_js_interop_types_js_as_incompatible_js"
                } else if l_js {
                    "invalid_runtime_check_with_js_interop_types_js_as_dart"
                } else {
                    "invalid_runtime_check_with_js_interop_types_dart_as_js"
                });
            }
        }
        if l_js {
            el = self.apagar_extensao(table, l);
        }
        if r_js {
            er = self.apagar_extensao(table, r);
        }
        (el, er)
    }

    /// `canBeSubtypeOf` com o `eraseTypes` (o resultado guia a recursão).
    fn pode(&self, table: &mut TypeTable, l: TypeId, r: TypeId, check: bool, codigo: &mut Option<&'static str>, prof: u32) -> bool {
        if prof > 16 {
            return true;
        }
        let (l, r) = self.apagar_par(table, l, r, check, codigo);
        let l_anul = !self.nao_anulavel(table, l);
        let r_anul = !self.nao_anulavel(table, r);
        if matches!(table.get(l), Type::Null) {
            return r_anul;
        }
        if matches!(table.get(r), Type::Null) {
            return l_anul;
        }
        if l_anul && r_anul {
            return true;
        }
        let e_classe = |c: ClassId, alvo: Option<ClassId>| Some(c) == alvo;
        match (table.get(l).clone(), table.get(r).clone()) {
            (Type::Function { .. }, Type::Interface { class, .. }) => return e_classe(class, self.core.function_class) || e_classe(class, self.core.object_class),
            (Type::Interface { class, .. }, Type::Function { .. }) => return e_classe(class, self.core.function_class) || e_classe(class, self.core.object_class),
            _ => {}
        }
        if let Type::FutureOr { arg, .. } = table.get(l).clone() {
            let fut = self.futuro(table, arg);
            return self.pode(table, arg, r, check, codigo, prof + 1) || self.pode(table, fut, r, check, codigo, prof + 1);
        }
        if let Type::FutureOr { arg, .. } = table.get(r).clone() {
            let fut = self.futuro(table, arg);
            return self.pode(table, l, arg, check, codigo, prof + 1) || self.pode(table, l, fut, check, codigo, prof + 1);
        }
        if let (Type::Interface { class: lc, args: la, .. }, Type::Interface { class: rc, args: ra, .. }) = (table.get(l).clone(), table.get(r).clone()) {
            if (Some(lc) == self.core.int_class && Some(rc) == self.core.double_class) || (Some(lc) == self.core.double_class && Some(rc) == self.core.int_class) {
                return true;
            }
            if self.program.class(lc).kind == ClassKind::Enum {
                return self.sub(table, l, r);
            }
            if lc == rc {
                for (x, y) in la.iter().zip(ra.iter()) {
                    if !self.pode(table, *x, *y, check, codigo, prof + 1) {
                        return false;
                    }
                }
                return true;
            }
        }
        if let (Type::Record { positional: lp, named: ln, .. }, Type::Record { positional: rp, named: rn, .. }) = (table.get(l).clone(), table.get(r).clone()) {
            if lp.len() != rp.len() {
                return false;
            }
            for (x, y) in lp.iter().zip(rp.iter()) {
                if !self.pode(table, *x, *y, check, codigo, prof + 1) {
                    return false;
                }
            }
            if ln.len() != rn.len() {
                return false;
            }
            for ((na, x), (nb, y)) in ln.iter().zip(rn.iter()) {
                if na != nb || !self.pode(table, *x, *y, check, codigo, prof + 1) {
                    return false;
                }
            }
        }
        true
    }

    fn futuro(&self, table: &mut TypeTable, arg: TypeId) -> TypeId {
        match self.core.future_class {
            Some(c) => table.intern(Type::Interface { class: c, args: Box::new([arg]), nullable: false }),
            None => arg,
        }
    }
}

/// `invalid_runtime_check_with_js_interop_types`.
#[allow(clippy::too_many_arguments)]
fn checagens_js(
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
    let Some(lib_js) = program.libraries.iter().position(|l| l.uri == "dart:js_interop").map(|i| LibraryId(i as u32)) else { return };
    let js_object = interner
        .lookup("JSObject")
        .and_then(|s| program.library(lib_js).declared.get(&s).and_then(|b| b.getter))
        .and_then(|e| match e {
            Element::Class(c) => Some(c),
            _ => None,
        });
    let js = Js { program, interner, outline, core, js_object };
    for e in a.exprs.iter() {
        let (valor, ty, check) = match &e.kind {
            ExprKind::Is { value, ty, .. } => (*value, *ty, true),
            ExprKind::As { value, ty } => (*value, *ty, false),
            _ => continue,
        };
        let Some(l) = corpo.get_type(valor).filter(|t| !core.is_unknown(table, *t)) else { continue };
        let Some(r) = corpo.tipos_de_anotacoes.get(&ty).copied().or_else(|| outline.tipos_escritos.get(&(u, ty)).copied()) else { continue };
        let mut codigo: Option<&'static str> = None;
        js.pode(table, l, r, check, &mut codigo, 0);
        if let Some(c) = codigo {
            let args = vec![crate::despejo::formatar(table, l, interner, program), crate::despejo::formatar(table, r, interner, program)];
            out.push((e.span, c, args));
        }
    }
}
