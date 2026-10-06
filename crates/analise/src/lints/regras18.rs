//! O décimo oitavo lote de regras de lint (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), escritas direto dos emissores da 3.6.2
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), pela semântica
//! da unidade:
//!
//! * `avoid_equals_and_hash_code_on_mutable_classes`: `==`/`hashCode`
//!   declarados numa classe sem `@immutable` (nela ou nos supertipos), no
//!   primeiro token depois das anotações.
//! * `package_api_docs`: o emissor da 3.6.2 não relata nada (o `check` volta
//!   logo).
//! * `unnecessary_null_checks`: o `!` cujo tipo esperado
//!   (`getExpectedType`, `super::regras15`) é anulável, e o padrão `p!` com o
//!   tipo casado anulável.
//! * `avoid_unnecessary_containers`: o `Container` cujo único argumento é o
//!   `child:` com um widget.
//! * `avoid_implementing_value_types`: o `implements` de uma classe cujo
//!   `==` concreto é de uma classe (que não é `Object`).
//! * `use_full_hex_values_for_flutter_colors`: o `Color(…)` do `dart.ui`
//!   com literal inteiro que não é `0x` com oito dígitos.
//! * `use_test_throws_matchers`: o `fail()` do `test_api` no fim de um `try`
//!   com um `catch` e sem `finally`.
//! * `do_not_use_environment`: `bool/int/String.fromEnvironment` e
//!   `bool.hasEnvironment`.
//! * `use_if_null_to_convert_nulls_to_bools`: `b == true` e `b != false` com
//!   `b` de tipo `bool?`.
//! * `annotate_redeclares`: o membro de tipo de extensão sem `@redeclare`
//!   cujo nome é de um membro das interfaces implementadas.
//! * `avoid_types_on_closure_parameters`: os tipos escritos nos parâmetros
//!   de uma closure com contexto de tipo de função
//!   (`super::regras16::contexto_aproximado`).
//! * `deprecated_consistency`: o construtor de classe obsoleta e o parâmetro
//!   `this.x` que não concordam com o campo.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::andar::{andar, No};
use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, LibraryId};
use dartforge_frontend::ast::{self, Annotation, Ast, BinaryOp, DeclId, DeclKind, ExprId, ExprKind, MemberKind, PatternId, PatternKind, StmtKind, TypeKind, UnaryOp};
use dartforge_intern::Interner;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::{Type, TypeId, TypeTable};
use std::collections::{HashMap, HashSet};

fn anulavel(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Intersection { bound, .. } => anulavel(table, *bound),
        Type::Interface { nullable, .. } | Type::TypeParameter { nullable, .. } | Type::ExtensionType { nullable, .. } => *nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => *nullable,
        Type::FutureOr { arg, nullable } => *nullable || anulavel(table, *arg),
        Type::Never => false,
    }
}

fn nome_da_biblioteca(s: &super::Semantica<'_>, interner: &Interner, l: LibraryId) -> Option<String> {
    s.program.library(l).name.as_ref().map(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join("."))
}

fn classe_da_decl(s: &super::Semantica<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unidade && r.decl == d))
}

/// `hasDeprecated`: `@deprecated` ou `@Deprecated(…)` do `dart:core`.
fn obsoleto(s: &super::Semantica<'_>, interner: &Interner, u: dartforge_elements::model::UnitId, metadata: &[Annotation]) -> bool {
    metadata.iter().any(|m| {
        dartforge_types::anotacoes::e_getter_de(s.program, interner, u, m, "dart.core", "deprecated")
            || dartforge_types::anotacoes::e_construtor_de(s.program, interner, u, m, "dart.core", "Deprecated")
    })
}

/// O token que começa em `pos` (identificador ou um caractere).
fn token_em(fonte: &str, pos: usize) -> Span {
    let b = fonte.as_bytes();
    let mut fim = pos;
    while fim < b.len() && (b[fim].is_ascii_alphanumeric() || b[fim] == b'_' || b[fim] == b'$') {
        fim += 1;
    }
    if fim == pos {
        fim = (pos + fonte[pos..].chars().next().map_or(1, char::len_utf8)).min(b.len());
    }
    Span { start: pos, end: fim }
}

/// O nome do construtor como escrito (`ConstructorName`): o tipo e o
/// `.nome`.
fn nome_do_construtor(a: &Ast, e: ExprId) -> Span {
    match &a.expr(e).kind {
        ExprKind::InstanceCreation { ty, constructor, .. } => {
            let t = a.ty(*ty).span;
            Span { start: t.start, end: constructor.map_or(t.end, |n| n.span.end) }
        }
        ExprKind::Call { target, .. } => a.expr(*target).span,
        _ => a.expr(e).span,
    }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    // `package_api_docs`: o emissor da 3.6.2 volta sem relatar.
    let _ = ligada("package_api_docs");
    let Some(s) = sem else { return out };
    let a = u.ast;
    let fonte = u.fonte;
    let program = s.program;
    let table = s.table;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let lib = program.unit(s.unidade).library;
    let criacoes: Vec<(ExprId, dartforge_elements::model::FunctionElementId)> = a
        .exprs
        .iter()
        .enumerate()
        .filter_map(|(k, _)| match s.corpo.get_resolved(ExprId(k as u32)) {
            Some(Resolved::Constructor(f)) => Some((ExprId(k as u32), *f)),
            _ => None,
        })
        .collect();
    let argumentos_de = |e: ExprId| -> Option<&ast::Arguments> {
        match &a.expr(e).kind {
            ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => Some(&**arguments),
            _ => None,
        }
    };

    // `avoid_equals_and_hash_code_on_mutable_classes`.
    if ligada("avoid_equals_and_hash_code_on_mutable_classes") {
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::Class(x) = &d.kind else { continue };
            let Some(classe) = classe_da_decl(s, DeclId(k as u32)) else { continue };
            let imutavel = |c: ClassId| -> bool {
                let Some(r) = program.class(c).decl else { return false };
                program.unit(r.unit).ast.decl(r.decl).metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, r.unit, m, "meta", "immutable"))
            };
            let tem = imutavel(classe) || s.outline.hierarchy.get(classe).is_some_and(|h| h.supertypes.keys().any(|k| imutavel(*k)));
            if tem {
                continue;
            }
            for &mid in &x.members {
                let m = a.member(mid);
                let MemberKind::Method(f) = &m.kind else { continue };
                if m.augment {
                    continue;
                }
                let Some(n) = a.function(*f).name else { continue };
                let texto = interner.resolve(n.sym);
                if texto != "==" && texto != "hashCode" {
                    continue;
                }
                let pos = match m.metadata.last() {
                    Some(x) => dartforge_frontend::fonte::pular_brancos(fonte.as_bytes(), x.span.end),
                    None => {
                        // Depois do comentário de documentação.
                        dartforge_frontend::fonte::pular_brancos(fonte.as_bytes(), m.span.start)
                    }
                };
                relatar(&c::AVOID_EQUALS_AND_HASH_CODE_ON_MUTABLE_CLASSES, token_em(fonte, pos), &[texto]);
            }
        }
    }

    // `unnecessary_null_checks`.
    if ligada("unnecessary_null_checks") {
        for (k, p) in a.patterns.iter().enumerate() {
            if let PatternKind::NullAssert(_) = &p.kind
                && let Some(&t) = s.corpo.tipos_casados.get(&PatternId(k as u32))
                && anulavel(table, t)
            {
                relatar(&c::UNNECESSARY_NULL_CHECKS, Span { start: p.span.end - 1, end: p.span.end }, &[]);
            }
        }
        let mut pai: HashMap<ExprId, No> = HashMap::new();
        let mut bangs: Vec<(ExprId, Vec<No>)> = Vec::new();
        andar(u, &mut |no, ancestrais| {
            if let No::Expr(e) = no {
                if let Some(p) = ancestrais.last() {
                    pai.insert(e, *p);
                }
                if matches!(a.expr(e).kind, ExprKind::Unary { op: UnaryOp::NullAssert, .. }) {
                    bangs.push((e, ancestrais.to_vec()));
                }
            }
        });
        let mut cascatas: HashMap<ExprId, ExprId> = HashMap::new();
        for e in a.exprs.iter() {
            let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
            for &sec in sections.iter() {
                let mut x = sec;
                loop {
                    let prox = match &a.expr(x).kind {
                        ExprKind::Property { target, .. } | ExprKind::Index { target, .. } | ExprKind::Call { target, .. } | ExprKind::Assign { target, .. } => *target,
                        ExprKind::CascadeTarget => {
                            cascatas.insert(x, *target);
                            break;
                        }
                        _ => break,
                    };
                    x = prox;
                }
            }
        }
        let real = |x: ExprId| if matches!(a.expr(x).kind, ExprKind::CascadeTarget) { cascatas.get(&x).copied() } else { Some(x) };
        let funcoes = super::regras15::Funcoes::de(s, a);
        bangs.sort_by_key(|(e, _)| a.expr(*e).span.start);
        for (no, ancestrais) in bangs {
            let esperado = super::regras15::tipo_esperado(s, a, interner, &funcoes, &pai, &ancestrais, no, &real);
            if esperado.is_some_and(|t| anulavel(table, t)) {
                let sp = a.expr(no).span;
                relatar(&c::UNNECESSARY_NULL_CHECKS, Span { start: sp.end - 1, end: sp.end }, &[]);
            }
        }
    }

    // `avoid_unnecessary_containers`.
    if ligada("avoid_unnecessary_containers") {
        let pais = super::regras16::pais_da_unidade(u, &|e| matches!(s.corpo.get_resolved(e), Some(Resolved::Constructor(_))));
        for &(e, _) in &criacoes {
            if !s.corpo.get_type(e).is_some_and(|t| super::flutter::e_widget_tipo(s, interner, t)) {
                continue;
            }
            let Some(No::Expr(p)) = pais.expr.get(&e).copied() else { continue };
            let Some(args) = argumentos_de(p) else { continue };
            let Some(arg) = args.args.iter().find(|x| x.value == e) else { continue };
            if arg.name.is_none_or(|n| interner.resolve(n.sym) != "child") || args.args.len() != 1 {
                continue;
            }
            // A criação mais próxima que contém o argumento.
            let dona = if matches!(s.corpo.get_resolved(p), Some(Resolved::Constructor(_))) {
                Some(p)
            } else {
                pais.ancestrais.get(&e).and_then(|anc| {
                    anc.iter().rev().find_map(|n| match n {
                        No::Expr(x) if matches!(s.corpo.get_resolved(*x), Some(Resolved::Constructor(_))) => Some(*x),
                        _ => None,
                    })
                })
            };
            if let Some(d) = dona
                && s.corpo.get_type(d).is_some_and(|t| super::flutter::e_container(s, interner, t))
            {
                relatar(&c::AVOID_UNNECESSARY_CONTAINERS, nome_do_construtor(a, d), &[]);
            }
        }
    }

    // `avoid_implementing_value_types`.
    if ligada("avoid_implementing_value_types") {
        let object = s.core.object_class;
        let sobrescreve_igual = |c: ClassId| -> bool {
            let Some(chave) = interner.lookup("==") else { return false };
            let mut vistos: HashSet<ClassId> = HashSet::new();
            let mut atual = Some(c);
            while let Some(x) = atual {
                if !vistos.insert(x) {
                    break;
                }
                let k = program.class(x);
                let candidatas = std::iter::once(x).chain(k.mixin_classes.iter().rev().copied());
                for cc in candidatas {
                    if let Some(&f) = program.class(cc).instance_members.get(&chave)
                        && !program.function(f).abstract_
                    {
                        let dona = program.class(cc);
                        return dona.kind == ClassKind::Class && Some(cc) != object;
                    }
                }
                atual = k.supertype_class;
            }
            false
        };
        for d in a.decls.iter() {
            let DeclKind::Class(x) = &d.kind else { continue };
            for &t in x.implements.iter() {
                if let Some(&tipo) = s.outline.tipos_escritos.get(&(s.unidade, t))
                    && let Type::Interface { class, .. } = table.get(tipo)
                    && sobrescreve_igual(*class)
                {
                    relatar(&c::AVOID_IMPLEMENTING_VALUE_TYPES, a.ty(t).span, &[]);
                }
            }
        }
    }

    // `use_full_hex_values_for_flutter_colors`.
    if ligada("use_full_hex_values_for_flutter_colors") {
        for &(e, f) in &criacoes {
            let g = program.function(f);
            let Some(cl) = g.class else { continue };
            let k = program.class(cl);
            if interner.resolve(k.name) != "Color" || !interner.resolve(g.name).is_empty() || nome_da_biblioteca(s, interner, k.library).as_deref() != Some("dart.ui") {
                continue;
            }
            let Some(args) = argumentos_de(e) else { continue };
            let Some(primeiro) = args.args.first() else { continue };
            if primeiro.name.is_some() {
                continue;
            }
            let ExprKind::Int(sp) = &a.expr(primeiro.value).kind else { continue };
            let lexema = &fonte[sp.start..sp.end];
            if lexema.starts_with('-') {
                continue;
            }
            let valor: String = lexema.to_lowercase().chars().filter(|c| *c != '_').collect();
            if !valor.starts_with("0x") || valor.chars().count() != 10 {
                relatar(&c::USE_FULL_HEX_VALUES_FOR_FLUTTER_COLORS, a.expr(primeiro.value).span, &[]);
            }
        }
    }

    // `use_test_throws_matchers`.
    if ligada("use_test_throws_matchers") {
        for st in a.stmts.iter() {
            let StmtKind::Try { body, catches, finally_ } = &st.kind else { continue };
            if catches.len() != 1 || finally_.is_some() {
                continue;
            }
            let StmtKind::Block(cmds) = &a.stmt(*body).kind else { continue };
            let Some(&ultimo) = cmds.last() else { continue };
            let StmtKind::Expression(e) = &a.stmt(ultimo).kind else { continue };
            let ExprKind::Call { target, .. } = &a.expr(*e).kind else { continue };
            let e_fail = match s.corpo.get_resolved(*target) {
                Some(Resolved::Element(Element::Function(f))) => {
                    let g = program.function(*f);
                    interner.resolve(g.name) == "fail" && g.class.is_none() && program.library(g.library).uri == "package:test_api/src/frontend/expect.dart"
                }
                _ => false,
            };
            if e_fail {
                relatar(&c::USE_TEST_THROWS_MATCHERS, a.stmt(ultimo).span, &[]);
            }
        }
    }

    // `do_not_use_environment`.
    if ligada("do_not_use_environment") {
        for &(e, f) in &criacoes {
            let g = program.function(f);
            if !g.factory {
                continue;
            }
            let nome = interner.resolve(g.name);
            let Some(t) = s.corpo.get_type(e) else { continue };
            let Type::Interface { class, .. } = table.get(t) else { continue };
            let cl = program.class(*class);
            if program.library(cl.library).uri != "dart:core" {
                continue;
            }
            let classe = interner.resolve(cl.name);
            let relata = (matches!(classe, "bool" | "int" | "String") && nome == "fromEnvironment") || (classe == "bool" && nome == "hasEnvironment");
            if relata {
                relatar(&c::DO_NOT_USE_ENVIRONMENT, nome_do_construtor(a, e), &[]);
            }
        }
    }

    // `use_if_null_to_convert_nulls_to_bools`.
    if ligada("use_if_null_to_convert_nulls_to_bools") {
        let bool_anulavel = |t: TypeId| matches!(table.get(t), Type::Interface { class, nullable: true, .. } if Some(*class) == s.core.bool_class);
        for e in a.exprs.iter() {
            let ExprKind::Binary { op, left, right } = &e.kind else { continue };
            let Some(t) = s.corpo.get_type(*left) else { continue };
            if !bool_anulavel(t) {
                continue;
            }
            let relata = match (op, &a.expr(*right).kind) {
                (BinaryOp::Eq, ExprKind::Bool(true)) => true,
                (BinaryOp::NotEq, ExprKind::Bool(false)) => true,
                _ => false,
            };
            if relata {
                relatar(&c::USE_IF_NULL_TO_CONVERT_NULLS_TO_BOOLS, e.span, &[]);
            }
        }
    }

    // `annotate_redeclares`.
    if ligada("annotate_redeclares") {
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::ExtensionType(x) = &d.kind else { continue };
            let _ = k;
            // Os nomes dos membros das interfaces implementadas (com os
            // herdados).
            let mut nomes: HashSet<dartforge_intern::SymbolId> = HashSet::new();
            for &t in x.implements.iter() {
                let Some(&tipo) = s.outline.tipos_escritos.get(&(s.unidade, t)) else { continue };
                let (Type::Interface { class: ic, .. } | Type::ExtensionType { decl: ic, .. }) = table.get(tipo) else { continue };
                let mut todas = vec![*ic];
                if let Some(h) = s.outline.hierarchy.get(*ic) {
                    todas.extend(h.supertypes.keys().copied());
                }
                if let Some(o) = s.core.object_class {
                    todas.push(o);
                }
                for cc in todas {
                    let kc = program.class(cc);
                    for (nome, f) in kc.instance_members.iter() {
                        let privado = interner.resolve(*nome).starts_with('_');
                        if !privado || program.function(*f).library == lib {
                            nomes.insert(*nome);
                        }
                    }
                }
            }
            for &mid in &x.members {
                let m = a.member(mid);
                let MemberKind::Method(f) = &m.kind else { continue };
                let f = a.function(*f);
                if f.static_ {
                    continue;
                }
                let Some(n) = f.name else { continue };
                if m.metadata.iter().any(|x| dartforge_types::anotacoes::e_getter_de(program, interner, s.unidade, x, "meta", "redeclare")) {
                    continue;
                }
                let texto = interner.resolve(n.sym);
                let chave = if f.kind == ast::FunctionKind::Setter { interner.lookup(&format!("{texto}_=")) } else { Some(n.sym) };
                if chave.is_some_and(|ch| nomes.contains(&ch)) {
                    relatar(&c::ANNOTATE_REDECLARES, n.span, &[texto]);
                }
            }
        }
    }

    // `avoid_types_on_closure_parameters`.
    if ligada("avoid_types_on_closure_parameters") {
        let pais = super::regras16::pais_da_unidade(u, &|e| matches!(a.expr(e).kind, ExprKind::FunctionExpression(_) | ExprKind::Call { .. } | ExprKind::InstanceCreation { .. }));
        for (k, e) in a.exprs.iter().enumerate() {
            let ExprKind::FunctionExpression(f) = &e.kind else { continue };
            let ctx = super::regras16::contexto_aproximado(s, interner, a, &pais, ExprId(k as u32));
            let super::regras16::Contexto::Tipo(t) = ctx else { continue };
            if !matches!(table.get(t), Type::Function { .. }) {
                continue;
            }
            let Some(ps) = &a.function(*f).parameters else { continue };
            for p in ps.iter() {
                if p.function_parameters.is_some() {
                    relatar(&c::AVOID_TYPES_ON_CLOSURE_PARAMETERS, p.span, &[]);
                    continue;
                }
                let Some(ty) = p.ty else { continue };
                if !matches!(a.ty(ty).kind, TypeKind::Named { .. }) {
                    continue;
                }
                let dinamico = s.corpo.tipos_de_anotacoes.get(&ty).is_some_and(|x| matches!(table.get(*x), Type::Dynamic));
                if !dinamico {
                    relatar(&c::AVOID_TYPES_ON_CLOSURE_PARAMETERS, a.ty(ty).span, &[]);
                }
            }
        }
    }

    // `deprecated_consistency`.
    if ligada("deprecated_consistency") {
        for d in a.decls.iter() {
            let (membros, nome_tipo) = match &d.kind {
                DeclKind::Class(x) => (&x.members, x.name),
                DeclKind::Enum(x) => (&x.members, x.name),
                DeclKind::Mixin(x) => (&x.members, x.name),
                DeclKind::ExtensionType(x) => (&x.members, x.name),
                _ => continue,
            };
            let classe_obsoleta = obsoleto(s, interner, s.unidade, &d.metadata);
            for &mid in membros {
                let m = a.member(mid);
                let MemberKind::Constructor(k) = &m.kind else { continue };
                if classe_obsoleta && !obsoleto(s, interner, s.unidade, &m.metadata) {
                    let alvo = k.name.unwrap_or(k.class_name).span;
                    relatar(&c::DEPRECATED_CONSISTENCY_CONSTRUCTOR, alvo, &[]);
                }
                for p in k.parameters.iter() {
                    if !p.this_ {
                        continue;
                    }
                    let Some(n) = p.name else { continue };
                    // O campo de mesmo nome da declaração.
                    let campo = membros.iter().find_map(|mm| match &a.member(*mm).kind {
                        MemberKind::Field(l) => l.variables.iter().find(|v| v.name.sym == n.sym).map(|v| (*mm, v.name)),
                        _ => None,
                    });
                    let Some((mc, nome_campo)) = campo else { continue };
                    let campo_obsoleto = obsoleto(s, interner, s.unidade, &a.member(mc).metadata);
                    let param_obsoleto = obsoleto(s, interner, s.unidade, &p.metadata);
                    if campo_obsoleto && !param_obsoleto {
                        relatar(&c::DEPRECATED_CONSISTENCY_FIELD, p.span, &[]);
                    }
                    if !campo_obsoleto && param_obsoleto {
                        relatar(&c::DEPRECATED_CONSISTENCY_PARAMETER, nome_campo.span, &[]);
                    }
                }
            }
            let _ = nome_tipo;
        }
    }

    out
}
