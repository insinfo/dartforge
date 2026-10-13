//! Typed lint rules from the `recommended` set
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the
//! 3.6.2 emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`).
//! As in [`crate::lints_tipados`], they return neutral findings.
//!
//! * `exhaustive_cases`: a `switch` (statement) over an "enum-like" class
//!   (`asEnumLikeClass`: concrete, only private non-factory constructors,
//!   two or more static constants of the class type, no subclass in the
//!   library's defining unit), removing the case values (identifier or
//!   property access) by constant value; `default` silences it. Reports each
//!   remaining value, from the `switch` to the `)`, with the name of the
//!   value's first non-`@deprecated` field.
//! * `use_super_parameters`: the `super(…)` of the initializers, the
//!   positional ones that are constructor parameters (in order, without
//!   `this.`/`super.` and not cited in the body) and the named ones forwarded
//!   with the same name, with the type of the superclass constructor's
//!   parameter assignable to that of the parameter.
//! * `invalid_runtime_check_with_js_interop_types`: `canBeSubtypeOf` with the
//!   erasure that keeps the `dart:js_interop` types
//!   (`EraseNonJSInteropTypes`) and decides the code at the first comparison
//!   that asks for it.
//!
//! Written without compiling or running (2026-10-05).

use crate::constantes::avaliador::{Constante, Motor};
use crate::lints_tipados::Finding;
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved, UnitBodyTypes};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, LibraryId, Program, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, ExprId, ExprKind, Initializer, MemberKind, ParameterKind, PatternKind, StmtKind};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

fn without_parentheses(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

/// The findings of this module's rules in unit `u`.
#[allow(clippy::too_many_arguments)]
pub fn achados(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    u: UnitId,
) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let Some(body) = bodies.units.get(u.0 as usize) else { return out };
    exhaustive_cases(program, interner, table, core, outline, bodies, the_inferred, u, &mut out);
    named_constants_and_declarations(program, interner, table, core, outline, bodies, the_inferred, u, &mut out);
    super_parameters(program, interner, table, core, outline, body, u, &mut out);
    js_checks(program, interner, table, core, outline, body, u, &mut out);
    out
}

/// The `thisType` of a class.
fn this_type(table: &mut TypeTable, outline: &OutlineTypes, c: ClassId) -> TypeId {
    let params: Vec<TypeId> = outline
        .classes
        .get(c.0 as usize)
        .map(|x| x.type_params.iter().map(|p| table.intern(Type::TypeParameter { param: *p, nullable: false })).collect())
        .unwrap_or_default();
    table.intern(Type::Interface { class: c, args: params.into(), nullable: false })
}

/// `exhaustive_cases`.
#[allow(clippy::too_many_arguments)]
fn exhaustive_cases(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    u: UnitId,
    out: &mut Vec<Finding>,
) {
    let unit_id = program.unit(u);
    let a = &unit_id.ast;
    let source = unit_id.source.as_str();
    let Some(body) = bodies.units.get(u.0 as usize) else { return };
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
    let deprecated = |v: VariableId| -> bool {
        let VariableRef::Field { unit, member, .. } = program.variable(v).node else { return false };
        let m = program.unit(unit).ast.member(member);
        m.metadata.iter().any(|x| {
            crate::anotacoes::e_getter_de(program, interner, unit, x, "dart.core", "deprecated")
                || crate::anotacoes::e_construtor_de(program, interner, unit, x, "dart.core", "Deprecated")
        })
    };
    // First pass (with the table): the candidates and the `thisType`.
    let mut candidates: Vec<(Span, ExprId, &[ast::SwitchCase], ClassId, TypeId)> = Vec::new();
    for (span, the_value, the_cases) in switches {
        let Some(t) = body.get_type(the_value) else { continue };
        let Type::Interface { class, .. } = table.get(t).clone() else { continue };
        let this_t = this_type(table, outline, class);
        candidates.push((span, the_value, the_cases, class, this_t));
    }
    if candidates.is_empty() {
        return;
    }
    let mut engine = Motor::novo(program, interner, table, core, outline, bodies, the_inferred);
    let exhaustive_on = true;
    for (span, the_value, the_cases, class, this_t) in candidates {
        // `no_default_cases`: the `default` of a `switch` over an enum or an
        // "enum-like" class (the first `default`).
        let e_enum = program.class(class).kind == ClassKind::Enum;
        let k = program.class(class);
        let switch_pattern = the_cases.iter().find(|c| c.pattern.is_none()).map(|c| c.span);
        if e_enum {
            if let Some(sp) = switch_pattern {
                out.push((sp, "no_default_cases", Vec::new()));
            }
            continue;
        }
        if !matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication) || k.modifiers.abstract_ {
            continue;
        }
        // Only private non-factory constructors (with none declared, the
        // default one is public).
        let constructors = k.construtores();
        if constructors.is_empty() {
            continue;
        }
        let all_private = constructors.iter().all(|(name, f)| interner.resolve(*name).starts_with('_') && !program.function(*f).factory);
        if !all_private {
            continue;
        }
        // The static constants of the class type, by value.
        let mut constantes: Vec<(crate::constantes::valor::Valor, Vec<VariableId>)> = Vec::new();
        let mut count = 0;
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
            let Some(Constante::Valor(val)) = engine.valor_de_variavel(v) else { continue };
            count += 1;
            match constantes.iter().position(|(w, _)| engine.iguais(&val, w)) {
                Some(i) => constantes[i].1.push(v),
                None => constantes.push((val, vec![v])),
            }
        }
        if count < 2 {
            continue;
        }
        // No subclass in the library's defining unit.
        let defining = program.library(k.library).units.first().copied();
        let has_subclass = (0..program.classes.len()).map(|i| ClassId(i as u32)).any(|cand| {
            let ck = program.class(cand);
            if !ck.decl.is_some_and(|r| Some(r.unit) == defining) {
                return false;
            }
            let mut seen = HashSet::new();
            let mut current = ck.supertype_class;
            while let Some(sc) = current {
                if !seen.insert(sc) {
                    break;
                }
                if sc == class {
                    return true;
                }
                current = program.class(sc).supertype_class;
            }
            false
        });
        if has_subclass {
            continue;
        }
        // The class is "enum-like": the `default` is `no_default_cases`.
        if let Some(sp) = switch_pattern {
            out.push((sp, "no_default_cases", Vec::new()));
        }
        let mut pattern = false;
        for case in the_cases {
            let Some(p) = case.pattern else {
                pattern = true;
                break;
            };
            let mut p = p;
            while let PatternKind::Parenthesized(x) = &a.pattern(p).kind {
                p = *x;
            }
            let PatternKind::Constant(e) = &a.pattern(p).kind else { continue };
            let e = without_parentheses(a, *e);
            if !matches!(a.expr(e).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) {
                continue;
            }
            let variable = match body.get_resolved(e) {
                Some(Resolved::Member { member: MemberRef::Variable(v), .. }) | Some(Resolved::Element(Element::Variable(v))) => Some(*v),
                Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) => program.function(*f).variable,
                _ => None,
            };
            let Some(v) = variable else { continue };
            if let Some(Constante::Valor(val)) = engine.valor_de_variavel(v)
                && let Some(i) = constantes.iter().position(|(w, _)| engine.iguais(&val, w))
            {
                constantes.remove(i);
            }
        }
        if pattern {
            continue;
        }
        if !exhaustive_on {
            continue;
        }
        // From the `switch` to the `)` after the expression.
        let b = source.as_bytes();
        let after = dartforge_frontend::source::skip_whitespace(b, a.expr(the_value).span.end);
        let end = if b.get(after) == Some(&b')') { after + 1 } else { a.expr(the_value).span.end };
        for (_, fields) in constantes {
            let preferred = fields.iter().copied().find(|v| !deprecated(*v)).unwrap_or(fields[0]);
            let name = interner.resolve(program.variable(preferred).name).to_string();
            out.push((Span { start: span.start, end: end }, "exhaustive_cases", vec![name]));
        }
    }
}

/// `use_named_constants` and `prefer_const_declarations` (the constants
/// engine).
#[allow(clippy::too_many_arguments)]
fn named_constants_and_declarations(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    u: UnitId,
    out: &mut Vec<Finding>,
) {
    let unit_id = program.unit(u);
    let a = &unit_id.ast;
    let source = unit_id.source.as_str();
    let lib = unit_id.library;
    let Some(body) = bodies.units.get(u.0 as usize) else { return };
    let pais = crate::lints_tipados::unit_parents(program, u);
    // `use_named_constants`: the constant creations of a class.
    let mut creations: Vec<(ExprId, ClassId)> = Vec::new();
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        if !matches!(body.get_resolved(id), Some(Resolved::Constructor(_))) {
            continue;
        }
        let written_const = matches!(&e.kind, ExprKind::InstanceCreation { keyword: Some(ast::CreationKeyword::Const), .. });
        if !written_const && !pais.in_constant_context(a, id) {
            continue;
        }
        let Some(t) = body.get_type(id) else { continue };
        let Type::Interface { class, .. } = table.get(t) else { continue };
        if program.class(*class).kind != ClassKind::Class {
            continue;
        }
        // Inside the initializer of a field of the class itself.
        let inside_field = program.class(*class).fields.iter().any(|v| match program.variable(*v).node {
            VariableRef::Field { unit, member, index } if unit == u => match &a.member(member).kind {
                MemberKind::Field(l) => l.variables.get(index).and_then(|x| x.initializer).is_some_and(|i| {
                    let (ie, ce) = (a.expr(i).span, e.span);
                    ce.start >= ie.start && ce.end <= ie.end
                }),
                _ => false,
            },
            _ => false,
        });
        if !inside_field {
            creations.push((id, *class));
        }
    }
    // `prefer_const_declarations`: the `final` lists (static, top-level and
    // local fields) with the start of the list and the initializers.
    let mut lists: Vec<(usize, usize, Vec<ExprId>)> = Vec::new();
    let list = |start: usize, l: &ast::VariableList, lists: &mut Vec<(usize, usize, Vec<ExprId>)>| {
        if l.const_ || !l.final_ {
            return;
        }
        let mut inits = Vec::new();
        for v in l.variables.iter() {
            let Some(i) = v.initializer else { return };
            // `TypedLiteral` without `const`.
            if matches!(a.expr(i).kind, ExprKind::List { const_: false, .. } | ExprKind::SetOrMap { const_: false, .. }) {
                return;
            }
            inits.push(i);
        }
        let Some(last) = l.variables.last() else { return };
        let end = last.initializer.map_or(last.name.span.end, |i| a.expr(i).span.end);
        lists.push((start, end, inits));
    };
    let skip_word = |pos: usize, p: &str| -> usize {
        if source[pos..].starts_with(p) && !source.as_bytes().get(pos + p.len()).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
            dartforge_frontend::source::skip_whitespace(source.as_bytes(), pos + p.len())
        } else {
            pos
        }
    };
    let after_annotations = |metadata: &[ast::Annotation], start: usize| match metadata.last() {
        Some(m) => dartforge_frontend::source::skip_whitespace(source.as_bytes(), m.span.end),
        None => start,
    };
    for d in a.decls.iter() {
        if let ast::DeclKind::Variables(l) = &d.kind {
            let begin = after_annotations(&d.metadata, d.span.start);
            list(begin, l, &mut lists);
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Field(l) = &m.kind
            && l.static_
        {
            let begin = skip_word(after_annotations(&m.metadata, m.span.start), "static");
            list(begin, l, &mut lists);
        }
    }
    for (k, st) in a.stmts.iter().enumerate() {
        if let StmtKind::Variables(l) = &st.kind {
            let meta = a.local_metadata.iter().find(|(x, _)| x.0 as usize == k).map_or(&[][..], |(_, m)| &m[..]);
            let begin = after_annotations(meta, st.span.start);
            list(begin, l, &mut lists);
        }
    }
    if creations.is_empty() && lists.is_empty() {
        return;
    }
    let mut engine = Motor::novo(program, interner, table, core, outline, bodies, the_inferred);
    let cx = crate::constantes::avaliador::Ctx::simples(u, lib);
    for (e, class) in creations {
        let Constante::Valor(the_value) = engine.avaliar(&cx, e, true) else { continue };
        let k = program.class(class);
        for &v in &k.fields {
            let x = program.variable(v);
            if !x.static_ || !x.const_ {
                continue;
            }
            let name = interner.resolve(x.name);
            if name.starts_with('_') && k.library != lib {
                continue;
            }
            if let Some(Constante::Valor(w)) = engine.valor_de_variavel(v)
                && engine.iguais(&the_value, &w)
            {
                let text = format!("{}.{}", interner.resolve(k.name), name);
                out.push((a.expr(e).span, "use_named_constants", vec![text]));
                break;
            }
        }
    }
    for (begin, end, inits) in lists {
        let without_error = inits.iter().all(|i| matches!(engine.avaliar(&cx, *i, true), Constante::Valor(_)));
        if without_error {
            out.push((Span { start: begin, end: end }, "prefer_const_declarations", Vec::new()));
        }
    }
}

/// `use_super_parameters`.
#[allow(clippy::too_many_arguments)]
fn super_parameters(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &UnitBodyTypes,
    u: UnitId,
    out: &mut Vec<Finding>,
) {
    let unit_id = program.unit(u);
    // `Feature.super_parameters` (2.17).
    if program.library(unit_id.library).features.version() < dartforge_frontend::features::LanguageVersion::new(2, 17) {
        return;
    }
    let a = &unit_id.ast;
    for (k_decl, d) in a.decls.iter().enumerate() {
        let members = match &d.kind {
            ast::DeclKind::Class(x) => &x.members,
            ast::DeclKind::Enum(x) => &x.members,
            _ => continue,
        };
        let did = ast::DeclId(k_decl as u32);
        let Some(cls) = (0..program.classes.len()).map(|i| ClassId(i as u32)).find(|c| program.class(*c).decl.is_some_and(|r| r.unit == u && r.decl == did)) else {
            continue;
        };
        for &mid in members {
            let MemberKind::Constructor(k) = &a.member(mid).kind else { continue };
            // The last `super(…)` of the initializers.
            let Some((super_name_of, args)) = k.initializers.iter().rev().find_map(|i| match i {
                Initializer::Super { constructor, arguments, .. } => Some((*constructor, arguments)),
                _ => None,
            }) else {
                continue;
            };
            // The superclass constructor (the `staticElement`).
            let Some(sup) = program.class(cls).supertype_class else { continue };
            let key = match super_name_of {
                Some(n) => n.sym,
                None => match interner.lookup("") {
                    Some(s) => s,
                    None => continue,
                },
            };
            let Some(&ksup) = program.class(sup).constructors.get(&key) else { continue };
            // The parameters cited in the body.
            let body_span = match &k.body {
                ast::FunctionBody::Block(s) => Some(a.stmt(*s).span),
                ast::FunctionBody::Expression(e) => Some(a.expr(*e).span),
                _ => None,
            };
            let mut cited: HashSet<dartforge_intern::SymbolId> = HashSet::new();
            if let Some(cs) = body_span {
                for (i, e) in a.exprs.iter().enumerate() {
                    if e.span.start >= cs.start
                        && e.span.end <= cs.end
                        && let ExprKind::Identifier(n) = &e.kind
                        && matches!(body.get_resolved(ExprId(i as u32)), Some(Resolved::Parameter { .. }))
                    {
                        cited.insert(n.sym);
                    }
                }
            }
            let ps = &k.parameters;
            // `_checkForConvertiblePositionalParams`.
            let mut positional: Vec<ast::Name> = Vec::new();
            let mut invalid = false;
            for x in args.args.iter() {
                match x.name {
                    None => match &a.expr(x.value).kind {
                        ExprKind::Identifier(n) => positional.push(*n),
                        _ => {
                            invalid = true;
                            break;
                        }
                    },
                    Some(_) => {}
                }
            }
            if invalid {
                continue;
            }
            let mut identifiers: Vec<String> = Vec::new();
            let mut seen: HashSet<dartforge_intern::SymbolId> = HashSet::new();
            let mut matched_index = 0usize;
            let mut aborts = false;
            'args: for n in &positional {
                // The argument's element is a parameter of this constructor
                // (the initializer scope).
                let Some(p) = ps.iter().find(|p| p.name.is_some_and(|q| q.sym == n.sym)) else {
                    aborts = true;
                    break;
                };
                if p.kind == ParameterKind::Named || !seen.insert(n.sym) {
                    aborts = true;
                    break;
                }
                let mut matched = false;
                for (i, q) in ps.iter().enumerate() {
                    if matched {
                        break;
                    }
                    if q.this_ || q.super_ {
                        aborts = true;
                        break 'args;
                    }
                    let Some(qn) = q.name else { continue };
                    if cited.contains(&qn.sym) {
                        aborts = true;
                        break 'args;
                    }
                    if qn.sym == n.sym {
                        matched = true;
                        identifiers.push(interner.resolve(qn.sym).to_string());
                        if i < matched_index {
                            aborts = true;
                            break 'args;
                        }
                        matched_index = i;
                    }
                }
            }
            if aborts {
                continue;
            }
            // The named ones.
            let sup_types = named_parameters(program, table, outline, ksup, cls, sup, core);
            for p in ps.iter() {
                if p.this_ || p.kind != ParameterKind::Named {
                    continue;
                }
                let Some(pn) = p.name else { continue };
                if cited.contains(&pn.sym) {
                    continue;
                }
                let Some(&sup_type) = sup_types.get(&pn.sym) else { continue };
                let passed_on = args.args.iter().any(|x| {
                    x.name.is_some_and(|l| l.sym == pn.sym)
                        && matches!(&a.expr(x.value).kind, ExprKind::Identifier(m) if m.sym == pn.sym)
                        && matches!(body.get_resolved(x.value), Some(Resolved::Parameter { .. }) | None)
                });
                if !passed_on {
                    continue;
                }
                // The type of this parameter.
                let own_type = p.ty.and_then(|t| outline.tipos_escritos.get(&(u, t)).copied()).unwrap_or(core.dynamic_);
                let assignable = matches!(table.get(sup_type), Type::Dynamic) || {
                    let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                    is_subtype(sup_type, own_type, &mut env)
                };
                if assignable {
                    identifiers.push(interner.resolve(pn.sym).to_string());
                }
            }
            if identifiers.is_empty() {
                continue;
            }
            let target = k.name.unwrap_or(k.class_name).span;
            if identifiers.len() > 1 {
                let cited: Vec<String> = identifiers.iter().map(|i| format!("'{i}'")).collect();
                let msg = match cited.len() {
                    2 => format!("{} and {}", cited[0], cited[1]),
                    n => format!("{}, and {}", cited[..n - 1].join(", "), cited[n - 1]),
                };
                out.push((target, "use_super_parameters_multiple", vec![msg]));
            } else {
                out.push((target, "use_super_parameters_single", vec![identifiers[0].clone()]));
            }
        }
    }
}

/// The types of the named parameters of constructor `f` of superclass `sup`,
/// instantiated in the supertype of class `cls`.
fn named_parameters(
    program: &Program,
    table: &mut TypeTable,
    outline: &OutlineTypes,
    f: dartforge_elements::model::FunctionElementId,
    cls: ClassId,
    sup: ClassId,
    core: &CoreTypes,
) -> HashMap<dartforge_intern::SymbolId, TypeId> {
    let mut m = HashMap::new();
    let Some(data) = outline.functions.get(f.0 as usize) else { return m };
    let mut sig = data.signature;
    let this_t = this_type(table, outline, cls);
    if let Some(inst) = outline.hierarchy.supertype_of(this_t, sup, table, core)
        && let Type::Interface { args, .. } = table.get(inst).clone()
    {
        let params = outline.classes.get(sup.0 as usize).map(|x| x.type_params.clone()).unwrap_or_default();
        let map: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
        sig = crate::ops::substitute(sig, &map, table);
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

/// The JS interop queries.
struct Js<'a> {
    program: &'a Program,
    interner: &'a Interner,
    outline: &'a OutlineTypes,
    core: &'a CoreTypes,
    /// The `JSObject` of `dart:js_interop`.
    js_object: Option<ClassId>,
}

impl Js<'_> {
    fn uri(&self, l: LibraryId) -> &str {
        &self.program.library(l).uri
    }

    fn bound(&self, table: &TypeTable, t: TypeId) -> TypeId {
        match table.get(t) {
            Type::TypeParameter { param, .. } => table.param(*param).bound,
            Type::Intersection { bound, .. } => *bound,
            _ => t,
        }
    }

    /// `isDartJsInteropType`.
    fn dart_js(&self, table: &TypeTable, t: TypeId) -> bool {
        match table.get(t) {
            Type::TypeParameter { .. } | Type::Intersection { .. } => self.dart_js(table, self.bound(table, t)),
            Type::ExtensionType { decl, .. } => self.uri(self.program.class(*decl).library) == "dart:js_interop",
            _ => false,
        }
    }

    /// The declared representation type of an extension type.
    fn representation(&self, decl: ClassId) -> Option<TypeId> {
        let v = self.program.class(decl).representation?;
        self.outline.variables.get(v.0 as usize)?.declared_type
    }

    /// `getJsTypeForStaticInterop`: the class with `dart:js_interop`'s `@JS()`
    /// and `@staticInterop`.
    fn static_interop(&self, c: ClassId) -> bool {
        let k = self.program.class(c);
        if k.kind != ClassKind::Class {
            return false;
        }
        let Some(r) = k.decl else { return false };
        let a = &self.program.unit(r.unit).ast;
        let mut js = false;
        let mut static_ = false;
        for m in a.decl(r.decl).metadata.iter() {
            match crate::anotacoes::invoked_element(self.program, self.interner, r.unit, m) {
                crate::anotacoes::InvokedElement::Construtor(cl, _) => {
                    let x = self.program.class(cl);
                    if self.uri(x.library) == "dart:js_interop" && self.interner.resolve(x.name) == "JS" {
                        js = true;
                    }
                }
                crate::anotacoes::InvokedElement::Getter(f) => {
                    let g = self.program.function(f);
                    if self.uri(g.library) == "dart:_js_annotations" && self.interner.resolve(g.name) == "staticInterop" {
                        static_ = true;
                    }
                }
                _ => {}
            }
        }
        js && static_ && self.js_object.is_some()
    }

    /// `isUserJsInteropType`.
    fn js_user(&self, table: &TypeTable, t: TypeId, prof: u32) -> bool {
        if prof > 16 {
            return false;
        }
        match table.get(t) {
            Type::TypeParameter { .. } | Type::Intersection { .. } => self.js_user(table, self.bound(table, t), prof + 1),
            Type::ExtensionType { decl, .. } => self.representation(*decl).is_some_and(|r| self.dart_js(table, r) || self.js_user(table, r, prof + 1)),
            Type::Interface { class, .. } => self.static_interop(*class),
            _ => false,
        }
    }

    /// `isWasmIncompatibleJsInterop`.
    fn incompatible(&self, table: &TypeTable, t: TypeId) -> bool {
        let (Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. }) = table.get(self.bound(table, t)) else { return false };
        let k = self.program.class(*class);
        let has_js = k.decl.is_some_and(|r| {
            let a = &self.program.unit(r.unit).ast;
            a.decl(r.decl).metadata.iter().any(|m| match crate::anotacoes::invoked_element(self.program, self.interner, r.unit, m) {
                crate::anotacoes::InvokedElement::Construtor(cl, _) => {
                    let x = self.program.class(cl);
                    self.interner.resolve(x.name) == "JS" && (self.uri(x.library) == "package:js/js.dart" || self.uri(x.library) == "dart:_js_annotations")
                }
                _ => false,
            })
        });
        let uri = self.uri(k.library);
        has_js || WEB_SDK.contains(&uri) || uri == "dart:js"
    }

    /// `EraseNonJSInteropTypes.perform`.
    fn erase(&self, table: &mut TypeTable, t: TypeId, keep_user: bool, seen: &mut HashSet<TypeParamId>) -> TypeId {
        let keeps = if keep_user { self.js_user(table, t, 0) } else { self.dart_js(table, t) };
        match table.get(t).clone() {
            Type::Interface { class, args, nullable } => {
                if keeps {
                    return this_type(table, self.outline, class);
                }
                if self.static_interop(class)
                    && let Some(jo) = self.js_object
                {
                    return table.intern(Type::ExtensionType { decl: jo, args: Box::new([]), nullable: false });
                }
                let new_: Vec<TypeId> = args.iter().map(|x| self.erase(table, *x, keep_user, seen)).collect();
                table.intern(Type::Interface { class, args: new_.into(), nullable })
            }
            Type::ExtensionType { decl, args, nullable } => {
                if keeps {
                    let params: Vec<TypeId> = self
                        .outline
                        .classes
                        .get(decl.0 as usize)
                        .map(|x| x.type_params.iter().map(|p| table.intern(Type::TypeParameter { param: *p, nullable: false })).collect())
                        .unwrap_or_default();
                    return table.intern(Type::ExtensionType { decl, args: params.into(), nullable: false });
                }
                // The erasure: the instantiated representation.
                let Some(rep) = self.representation(decl) else { return t };
                let params = self.outline.classes.get(decl.0 as usize).map(|x| x.type_params.clone()).unwrap_or_default();
                let map: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
                let inst = crate::ops::substitute(rep, &map, table);
                let erased = self.erase(table, inst, keep_user, seen);
                if nullable { crate::ops::nullable(erased, table) } else { erased }
            }
            Type::TypeParameter { param, .. } => {
                if !seen.insert(param) {
                    return t;
                }
                let bnd = table.param(param).bound;
                let the_new_ = self.erase(table, bnd, keep_user, seen);
                table.intern(Type::Intersection { param, bound: the_new_ })
            }
            Type::Intersection { param, bound } => {
                if !seen.insert(param) {
                    return t;
                }
                let the_new_ = self.erase(table, bound, keep_user, seen);
                table.intern(Type::Intersection { param, bound: the_new_ })
            }
            Type::FutureOr { arg, nullable } => {
                let a2 = self.erase(table, arg, keep_user, seen);
                table.intern(Type::FutureOr { arg: a2, nullable })
            }
            _ => t,
        }
    }

    /// `extensionTypeErasure` comum.
    fn erase_extension(&self, table: &mut TypeTable, t: TypeId) -> TypeId {
        let outline = self.outline;
        let program = self.program;
        let rep = |decl: ClassId, args: &[TypeId], table: &mut TypeTable| -> Option<TypeId> {
            let v = program.class(decl).representation?;
            let t = outline.variables[v.0 as usize].declared_type?;
            let params = &outline.classes[decl.0 as usize].type_params;
            if params.is_empty() || params.len() != args.len() {
                return Some(t);
            }
            let map: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            Some(crate::ops::substitute(t, &map, table))
        };
        crate::ops::erase_extension_type(t, table, &rep)
    }

    fn non_nullable(&self, table: &TypeTable, t: TypeId) -> bool {
        match table.get(t) {
            Type::Dynamic | Type::Void | Type::Null => false,
            Type::Intersection { bound, .. } => self.non_nullable(table, *bound),
            Type::TypeParameter { param, nullable } => !*nullable && self.non_nullable(table, table.param(*param).bound),
            Type::FutureOr { arg, nullable } => !*nullable && self.non_nullable(table, *arg),
            Type::Interface { nullable, .. } | Type::ExtensionType { nullable, .. } => !*nullable,
            Type::Function { nullable, .. } | Type::Record { nullable, .. } => !*nullable,
            Type::Never => true,
        }
    }

    fn sub(&self, table: &mut TypeTable, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    /// The emitter's `eraseTypes`: decides the code (once) and returns the types
    /// to continue the comparison.
    fn erase_pair(&self, table: &mut TypeTable, l: TypeId, r: TypeId, check: bool, code: &mut Option<&'static str>) -> (TypeId, TypeId) {
        let el0 = self.erase(table, l, false, &mut HashSet::new());
        let mut el = crate::ops::non_nullable(el0, table);
        let er0 = self.erase(table, r, false, &mut HashSet::new());
        let mut er = crate::ops::non_nullable(er0, table);
        let l_js = self.dart_js(table, el);
        let r_js = self.dart_js(table, er);
        if code.is_none() && (l_js || r_js) && !self.incompatible(table, el) && !self.incompatible(table, er) {
            let l_sub = self.sub(table, el, er);
            let r_sub = self.sub(table, er, el);
            let l_din = matches!(table.get(el), Type::Dynamic);
            let r_din = matches!(table.get(er), Type::Dynamic);
            if check {
                if !l_sub && !r_din {
                    *code = Some(if l_js && r_js {
                        "invalid_runtime_check_with_js_interop_types_js_is_inconsistent_js"
                    } else if l_js {
                        "invalid_runtime_check_with_js_interop_types_js_is_dart"
                    } else {
                        "invalid_runtime_check_with_js_interop_types_dart_is_js"
                    });
                } else if l_sub && l_js && r_js && self.js_user(table, r, 0) {
                    let ul = self.erase(table, l, true, &mut HashSet::new());
                    let ur = self.erase(table, r, true, &mut HashSet::new());
                    if !self.sub(table, ul, ur) {
                        *code = Some("invalid_runtime_check_with_js_interop_types_js_is_unrelated_js");
                    }
                }
            } else if !l_sub && !r_sub && !l_din && !r_din {
                *code = Some(if l_js && r_js {
                    "invalid_runtime_check_with_js_interop_types_js_as_incompatible_js"
                } else if l_js {
                    "invalid_runtime_check_with_js_interop_types_js_as_dart"
                } else {
                    "invalid_runtime_check_with_js_interop_types_dart_as_js"
                });
            }
        }
        if l_js {
            el = self.erase_extension(table, l);
        }
        if r_js {
            er = self.erase_extension(table, r);
        }
        (el, er)
    }

    /// `canBeSubtypeOf` with `eraseTypes` (the result guides the recursion).
    fn can(&self, table: &mut TypeTable, l: TypeId, r: TypeId, check: bool, code: &mut Option<&'static str>, prof: u32) -> bool {
        if prof > 16 {
            return true;
        }
        let (l, r) = self.erase_pair(table, l, r, check, code);
        let l_nullable = !self.non_nullable(table, l);
        let r_nullable = !self.non_nullable(table, r);
        if matches!(table.get(l), Type::Null) {
            return r_nullable;
        }
        if matches!(table.get(r), Type::Null) {
            return l_nullable;
        }
        if l_nullable && r_nullable {
            return true;
        }
        let is_class = |c: ClassId, target: Option<ClassId>| Some(c) == target;
        match (table.get(l).clone(), table.get(r).clone()) {
            (Type::Function { .. }, Type::Interface { class, .. }) => return is_class(class, self.core.function_class) || is_class(class, self.core.object_class),
            (Type::Interface { class, .. }, Type::Function { .. }) => return is_class(class, self.core.function_class) || is_class(class, self.core.object_class),
            _ => {}
        }
        if let Type::FutureOr { arg, .. } = table.get(l).clone() {
            let fut = self.future(table, arg);
            return self.can(table, arg, r, check, code, prof + 1) || self.can(table, fut, r, check, code, prof + 1);
        }
        if let Type::FutureOr { arg, .. } = table.get(r).clone() {
            let fut = self.future(table, arg);
            return self.can(table, l, arg, check, code, prof + 1) || self.can(table, l, fut, check, code, prof + 1);
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
                    if !self.can(table, *x, *y, check, code, prof + 1) {
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
                if !self.can(table, *x, *y, check, code, prof + 1) {
                    return false;
                }
            }
            if ln.len() != rn.len() {
                return false;
            }
            for ((at, x), (nb, y)) in ln.iter().zip(rn.iter()) {
                if at != nb || !self.can(table, *x, *y, check, code, prof + 1) {
                    return false;
                }
            }
        }
        true
    }

    fn future(&self, table: &mut TypeTable, arg: TypeId) -> TypeId {
        match self.core.future_class {
            Some(c) => table.intern(Type::Interface { class: c, args: Box::new([arg]), nullable: false }),
            None => arg,
        }
    }
}

/// `invalid_runtime_check_with_js_interop_types`.
#[allow(clippy::too_many_arguments)]
fn js_checks(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &UnitBodyTypes,
    u: UnitId,
    out: &mut Vec<Finding>,
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
        let (the_value, ty, check) = match &e.kind {
            ExprKind::Is { value, ty, .. } => (*value, *ty, true),
            ExprKind::As { value, ty } => (*value, *ty, false),
            _ => continue,
        };
        let Some(l) = body.get_type(the_value).filter(|t| !core.is_unknown(table, *t)) else { continue };
        let Some(r) = body.tipos_de_anotacoes.get(&ty).copied().or_else(|| outline.tipos_escritos.get(&(u, ty)).copied()) else { continue };
        let mut code: Option<&'static str> = None;
        js.can(table, l, r, check, &mut code, 0);
        if let Some(c) = code {
            let args = vec![crate::despejo::formatar(table, l, interner, program), crate::despejo::formatar(table, r, interner, program)];
            out.push((e.span, c, args));
        }
    }
}
