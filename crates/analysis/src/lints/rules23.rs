//! The twenty-third batch of lint rules
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`),
//! through the unit's semantics:
//!
//! * `comment_references`: the `[this]`/`[null]`/`[true]`/`[false]` of each
//!   comment token, and the references (`_parseReferences`, outside code)
//!   that the `CommentReferenceResolver` does not resolve (the scope of the
//!   documented declaration: parameters and type parameters, the class's
//!   declared members, the library's; and the inherited members of the class
//!   or of the extended type), except link definitions (`[x]: …`).
//!   Known deviation: the comments of parameters, type parameters and local
//!   variables are not read.
//! * `prefer_void_to_null`: the written `Null` type, except the positions the
//!   emitter skips.
//! * `type_annotate_public_apis`: the public declarations without a type.
//!
//! Written without compiling or running (2026-10-05).

use super::LintCode;
use super::codes_g as c;
use super::rules::LintReport;
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, ExtensionId};
use dartforge_frontend::ast::{
    Annotation, Ast, DeclId, DeclKind, ExprKind, ForInit, MemberKind, Parameter, PatternKind,
    StmtKind, TypeId, TypeKind, TypedefKind,
};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::table::Type;
use std::collections::HashSet;

fn class_of_decl(s: &super::Semantics<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len())
        .map(|i| ClassId(i as u32))
        .find(|c| {
            s.program
                .class(*c)
                .decl
                .is_some_and(|r| r.unit == s.unit && r.decl == d)
        })
}

/// The owner of a documentation comment and its scope.
struct DocOwner<'a> {
    /// The names declared by the declaration itself (parameters and type
    /// parameters).
    own_names: Vec<SymbolId>,
    /// The declared members of the enclosing class (the `InstanceScope`) and
    /// its type parameters.
    class_names: Vec<SymbolId>,
    class_id: Option<ClassId>,
    extension: Option<ExtensionId>,
    metadata: &'a [Annotation],
    start: usize,
}

/// The declared names of a member list (accessors and methods).
fn member_names(a: &Ast, members: &[dartforge_frontend::ast::MemberId]) -> Vec<SymbolId> {
    let mut v = Vec::new();
    for &m in members {
        match &a.member(m).kind {
            MemberKind::Field(l) => v.extend(l.variables.iter().map(|x| x.name.sym)),
            MemberKind::Method(f) => v.extend(a.function(*f).name.map(|n| n.sym)),
            MemberKind::Constructor(_) => {}
        }
    }
    v
}

fn parameter_names(ps: &[Parameter]) -> Vec<SymbolId> {
    ps.iter().filter_map(|p| p.name.map(|n| n.sym)).collect()
}

/// The element that a name resolves to in the owner's scope.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ResolvedElem {
    Nothing,
    Prefix,
    Class(ClassId),
    Extension(ExtensionId),
    Other,
}

/// The member `name` (instance, static or named constructor) of the class.
fn has_class_member(
    s: &super::Semantics<'_>,
    interner: &Interner,
    c: ClassId,
    name: &str,
    inherited: bool,
) -> bool {
    let program = s.program;
    let Some(sym) = interner.lookup(name) else {
        return false;
    };
    let setter = interner.lookup(&format!("{name}_="));
    let has = |k: ClassId| {
        let x = program.class(k);
        x.instance_members.contains_key(&sym)
            || setter.is_some_and(|st| x.instance_members.contains_key(&st))
            || x.static_members.contains_key(&sym)
            || setter.is_some_and(|st| x.static_members.contains_key(&st))
    };
    if has(c) || program.class(c).constructors.contains_key(&sym) {
        return true;
    }
    if inherited {
        let mut all_found: Vec<ClassId> = s
            .outline
            .hierarchy
            .get(c)
            .map(|d| d.supertypes.keys().copied().collect())
            .unwrap_or_default();
        all_found.extend(s.core.object_class);
        return all_found.into_iter().any(|k| {
            let x = program.class(k);
            x.instance_members.contains_key(&sym)
                || setter.is_some_and(|st| x.instance_members.contains_key(&st))
        });
    }
    false
}

fn has_extension_member(
    s: &super::Semantics<'_>,
    interner: &Interner,
    e: ExtensionId,
    name: &str,
) -> bool {
    let x = &s.program.extensions[e.0 as usize];
    let Some(sym) = interner.lookup(name) else {
        return false;
    };
    let setter = interner.lookup(&format!("{name}_="));
    x.instance_members.contains_key(&sym)
        || x.static_members.contains_key(&sym)
        || setter.is_some_and(|st| {
            x.instance_members.contains_key(&st) || x.static_members.contains_key(&st)
        })
}

/// `_resolveSimpleIdentifier`.
fn resolve_simple(
    s: &super::Semantics<'_>,
    interner: &Interner,
    owner: &DocOwner<'_>,
    name: &str,
) -> ResolvedElem {
    let program = s.program;
    if let Some(sym) = interner.lookup(name) {
        if owner.own_names.contains(&sym) || owner.class_names.contains(&sym) {
            return ResolvedElem::Other;
        }
        if program.prefixes_in_unit(s.unit).contains_key(&sym) {
            return ResolvedElem::Prefix;
        }
        if let Some(b) = program.lookup_in_unit(s.unit, sym)
            && let Some(e) = b.getter.or(b.setter)
        {
            return match e {
                Element::Class(c) => ResolvedElem::Class(c),
                Element::Extension(x) => ResolvedElem::Extension(x),
                Element::Prefix(..) => ResolvedElem::Prefix,
                _ => ResolvedElem::Other,
            };
        }
    }
    // The type of the enclosing class (or the extended one): the members, also
    // the inherited ones.
    if let Some(c) = owner.class_id
        && has_class_member(s, interner, c, name, true)
    {
        return ResolvedElem::Other;
    }
    if let Some(e) = owner.extension {
        let on = s.outline.extensions.get(e.0 as usize).map(|x| x.on);
        let on_class = on.and_then(|t| match s.table.get(t) {
            Type::Interface { class, .. } => Some(*class),
            Type::Function { .. } => s.core.function_class,
            Type::TypeParameter { param, .. } => match s.table.get(s.table.param(*param).bound) {
                Type::Interface { class, .. } => Some(*class),
                _ => None,
            },
            _ => None,
        });
        if on_class.is_some_and(|c| has_class_member(s, interner, c, name, true)) {
            return ResolvedElem::Other;
        }
    }
    ResolvedElem::Nothing
}

/// The reference resolves.
fn resolve(
    s: &super::Semantics<'_>,
    interner: &Interner,
    owner: &DocOwner<'_>,
    parts: &[&str],
) -> bool {
    let program = s.program;
    match parts {
        [x] => resolve_simple(s, interner, owner, x) != ResolvedElem::Nothing,
        [p, x] => match resolve_simple(s, interner, owner, p) {
            ResolvedElem::Prefix => {
                let (Some(ps), Some(xs)) = (interner.lookup(p), interner.lookup(x)) else {
                    return false;
                };
                program.lookup_prefixed_in_unit(s.unit, ps, xs).is_some()
            }
            ResolvedElem::Class(c) => has_class_member(s, interner, c, x, true),
            ResolvedElem::Extension(e) => has_extension_member(s, interner, e, x),
            _ => false,
        },
        [p, k, x] => {
            if resolve_simple(s, interner, owner, p) != ResolvedElem::Prefix {
                return false;
            }
            let (Some(ps), Some(ks)) = (interner.lookup(p), interner.lookup(k)) else {
                return false;
            };
            match program
                .lookup_prefixed_in_unit(s.unit, ps, ks)
                .and_then(|b| b.getter.or(b.setter))
            {
                Some(Element::Class(c)) => has_class_member(s, interner, c, x, false),
                Some(Element::Extension(e)) => has_extension_member(s, interner, e, x),
                _ => false,
            }
        }
        _ => true,
    }
}

/// The references of a comment (`_parseReferences`, outside code blocks and
/// backtick spans), with the parts and the positions.
fn references(
    source: &str,
    lines: &[(usize, &str)],
    in_block: &dyn Fn(usize) -> bool,
) -> Vec<Vec<(String, Span)>> {
    let _ = source;
    let mut v = Vec::new();
    for (k, &(base, content)) in lines.iter().enumerate() {
        if in_block(k) {
            continue;
        }
        let b = content.as_bytes();
        let mut i = 0;
        let mut only_blanks = true;
        while i < b.len() {
            match b[i] {
                b'[' => {
                    i += 1;
                    if b.get(i) == Some(&b':') {
                        match content[i + 1..].find(":]") {
                            Some(k) => i = i + 1 + k + 1,
                            None => break,
                        }
                    } else {
                        let start_rel = i;
                        let Some(k) = content[i..].find(']') else {
                            break;
                        };
                        let end = i + k;
                        let mut j = end + 1;
                        let is_link = match b.get(j) {
                            Some(b'(') => true,
                            Some(b':') if only_blanks => true,
                            _ => {
                                while b.get(j).is_some_and(|c| c.is_ascii_whitespace()) {
                                    j += 1;
                                }
                                b.get(j) == Some(&b'[')
                            }
                        };
                        if !is_link {
                            // The parts with the positions.
                            let mut text_start = start_rel;
                            let mut text = &content[start_rel..end];
                            let cut = text.len() - text.trim_start().len();
                            text_start += cut;
                            text = text.trim();
                            if let Some(r) = text.strip_prefix("new ") {
                                let c2 = text.len() - r.trim_start().len();
                                text_start += c2;
                                text = r.trim_start();
                            }
                            let mut parts: Vec<(String, Span)> = Vec::new();
                            let mut pos = text_start;
                            let mut ok = true;
                            for p in text.split('.') {
                                let lead = p.len() - p.trim_start().len();
                                let t = p.trim();
                                let id = !t.is_empty()
                                    && t.chars().next().is_some_and(|c| {
                                        c.is_ascii_alphabetic() || c == '_' || c == '$'
                                    })
                                    && t.chars()
                                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
                                if !id {
                                    ok = false;
                                    break;
                                }
                                parts.push((
                                    t.to_string(),
                                    Span {
                                        start: base + pos + lead,
                                        end: base + pos + lead + t.len(),
                                    },
                                ));
                                pos += p.len() + 1;
                            }
                            if ok && !parts.is_empty() && parts.len() <= 3 {
                                v.push(parts);
                            }
                        }
                        i = end;
                    }
                    only_blanks = false;
                }
                b'`' => {
                    if let Some(k) = content[i + 1..].find('`') {
                        i = i + 1 + k;
                    }
                    only_blanks = false;
                }
                c if !c.is_ascii_whitespace() => only_blanks = false,
                _ => {}
            }
            i += 1;
        }
    }
    v
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(
    u: Unit<'_>,
    interner: &Interner,
    enabled: &dyn Fn(&str) -> bool,
    sem: Option<&super::Semantics<'_>>,
) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let source = u.source;
    let program = s.program;
    let table = s.table;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport {
            code,
            span,
            args: args.iter().map(|x| x.to_string()).collect(),
        });
    };

    // `comment_references`.
    if enabled("comment_references") {
        let comments = dartforge_frontend::comments::Comments::scan(source);
        // The owners: directives, top-level declarations, members, constants.
        let mut owners: Vec<DocOwner<'_>> = Vec::new();
        for d in &u.unit.directives {
            owners.push(DocOwner {
                own_names: Vec::new(),
                class_names: Vec::new(),
                class_id: None,
                extension: None,
                metadata: &d.metadata,
                start: d.span.start,
            });
        }
        for (k, d) in a.decls.iter().enumerate() {
            let did = DeclId(k as u32);
            let mut owner = DocOwner {
                own_names: Vec::new(),
                class_names: Vec::new(),
                class_id: None,
                extension: None,
                metadata: &d.metadata,
                start: d.span.start,
            };
            let (members, tps, constants): (
                &[dartforge_frontend::ast::MemberId],
                Vec<SymbolId>,
                &[dartforge_frontend::ast::EnumConstant],
            ) = match &d.kind {
                DeclKind::Class(x) => (
                    &x.members,
                    x.type_params.iter().map(|t| t.name.sym).collect(),
                    &[],
                ),
                DeclKind::Mixin(x) => (
                    &x.members,
                    x.type_params.iter().map(|t| t.name.sym).collect(),
                    &[],
                ),
                DeclKind::Enum(x) => (
                    &x.members,
                    x.type_params.iter().map(|t| t.name.sym).collect(),
                    &x.constants,
                ),
                DeclKind::ExtensionType(x) => (
                    &x.members,
                    x.type_params.iter().map(|t| t.name.sym).collect(),
                    &[],
                ),
                DeclKind::Extension(x) => (
                    &x.members,
                    x.type_params.iter().map(|t| t.name.sym).collect(),
                    &[],
                ),
                DeclKind::Function(f) => {
                    let f = a.function(*f);
                    owner.own_names = f.type_params.iter().map(|t| t.name.sym).collect();
                    owner.own_names.extend(
                        f.parameters
                            .as_deref()
                            .map(parameter_names)
                            .unwrap_or_default(),
                    );
                    owners.push(owner);
                    continue;
                }
                DeclKind::Typedef(x) => {
                    owner.own_names = x.type_params.iter().map(|t| t.name.sym).collect();
                    if let TypedefKind::Legacy { parameters, .. } = &x.kind {
                        owner.own_names.extend(parameter_names(parameters));
                    }
                    owners.push(owner);
                    continue;
                }
                DeclKind::Variables(_) => {
                    owners.push(owner);
                    continue;
                }
            };
            let class_id = if matches!(d.kind, DeclKind::Extension(_)) {
                None
            } else {
                class_of_decl(s, did)
            };
            let extension = if matches!(d.kind, DeclKind::Extension(_)) {
                program.extension_of_decl(s.unit, did)
            } else {
                None
            };
            let mut class_names = member_names(a, members);
            class_names.extend(constants.iter().map(|k| k.name.sym));
            class_names.extend(tps.iter().copied());
            owner.class_names = class_names.clone();
            owner.class_id = class_id;
            owner.extension = extension;
            owners.push(owner);
            for kc in constants {
                owners.push(DocOwner {
                    own_names: Vec::new(),
                    class_names: class_names.clone(),
                    class_id,
                    extension,
                    metadata: &kc.metadata,
                    start: kc.name.span.start,
                });
            }
            for &mid in members {
                let m = a.member(mid);
                let own_names = match &m.kind {
                    MemberKind::Method(f) => {
                        let f = a.function(*f);
                        let mut v: Vec<SymbolId> =
                            f.type_params.iter().map(|t| t.name.sym).collect();
                        v.extend(
                            f.parameters
                                .as_deref()
                                .map(parameter_names)
                                .unwrap_or_default(),
                        );
                        v
                    }
                    MemberKind::Constructor(kc) => parameter_names(&kc.parameters),
                    MemberKind::Field(_) => Vec::new(),
                };
                owners.push(DocOwner {
                    own_names,
                    class_names: class_names.clone(),
                    class_id,
                    extension,
                    metadata: &m.metadata,
                    start: m.span.start,
                });
            }
        }
        let mut seen: HashSet<usize> = HashSet::new();
        for owner in &owners {
            let after = match owner.metadata.last() {
                Some(m) => {
                    dartforge_frontend::source::skip_whitespace(source.as_bytes(), m.span.end)
                }
                None => owner.start,
            };
            let Some(doc_start) = comments.dart_doc(source, after).or_else(|| {
                owner
                    .metadata
                    .iter()
                    .rev()
                    .find_map(|m| comments.dart_doc(source, m.span.start))
            }) else {
                continue;
            };
            if !seen.insert(doc_start.start) {
                continue;
            }
            // The `Comment`'s tokens.
            let parent = if comments.dart_doc(source, after) == Some(doc_start) {
                after
            } else {
                owner
                    .metadata
                    .iter()
                    .rev()
                    .find(|m| comments.dart_doc(source, m.span.start) == Some(doc_start))
                    .map_or(after, |m| m.span.start)
            };
            let chain: Vec<Span> = comments
                .before(source, parent)
                .into_iter()
                .filter(|x| x.start >= doc_start.start)
                .collect();
            let mut tokens = vec![doc_start];
            if source[doc_start.start..doc_start.end].starts_with("///") {
                tokens.extend(
                    chain
                        .iter()
                        .skip(1)
                        .filter(|x| source[x.start..x.end].starts_with("///"))
                        .copied(),
                );
            }
            // `visitComment`: the link definitions and the special cases.
            let mut links: Vec<String> = Vec::new();
            for t in &tokens {
                let lexeme = &source[t.start..t.end];
                let mut start = 0usize;
                let mut first = true;
                while let Some(l) = lexeme[start..].find('[').map(|x| x + start) {
                    let Some(r) = lexeme[l..].find(']').map(|x| x + l) else {
                        break;
                    };
                    let reference = &lexeme[l + 1..r];
                    if first {
                        let prefix = &lexeme[..l];
                        let is_start = prefix.starts_with("///")
                            && prefix
                                .trim_start_matches('/')
                                .chars()
                                .all(char::is_whitespace);
                        if is_start && lexeme[r + 1..].starts_with(':') {
                            links.push(reference.to_string());
                        }
                        first = false;
                    }
                    if matches!(reference, "this" | "null" | "true" | "false") {
                        report(
                            &c::COMMENT_REFERENCES,
                            Span {
                                start: t.start + l + 1,
                                end: t.start + l + 1 + reference.len(),
                            },
                            &[],
                        );
                    }
                    start = r;
                }
            }
            // The unresolved references.
            let doc = super::rules14::Doc {
                tokens: tokens.clone(),
                chain: chain.clone(),
                anchor: parent,
            };
            let lines = super::rules14::doc_lines(source, &doc);
            let blocks = super::rules14::code_blocks(&lines);
            let in_block = |k: usize| {
                let pos = lines[k].0;
                blocks
                    .iter()
                    .any(|b| b.lines.iter().any(|(p, _)| *p == pos))
            };
            for parts in references(source, &lines, &in_block) {
                let names: Vec<&str> = parts.iter().map(|(n, _)| n.as_str()).collect();
                if resolve(s, interner, owner, &names) {
                    continue;
                }
                let text = names.join(".");
                if links.contains(&text) {
                    continue;
                }
                // `[p.C.x]`: only reports the access with the prefixed target.
                let sp = Span {
                    start: parts[0].1.start,
                    end: parts[parts.len() - 1].1.end,
                };
                report(&c::COMMENT_REFERENCES, sp, &[]);
            }
        }
    }

    // `prefer_void_to_null`.
    if enabled("prefer_void_to_null") {
        let mut excluded: HashSet<TypeId> = HashSet::new();
        for t in a.types.iter() {
            if let TypeKind::Function {
                return_type,
                parameters,
                ..
            } = &t.kind
            {
                excluded.extend(*return_type);
                excluded.extend(parameters.iter().filter_map(|p| p.ty));
            }
        }
        for p in a.patterns.iter() {
            if let PatternKind::Cast { ty, .. } = &p.kind {
                excluded.insert(*ty);
            }
        }
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::As { ty, .. } => {
                    excluded.insert(*ty);
                }
                ExprKind::List {
                    type_args,
                    elements,
                    ..
                }
                | ExprKind::SetOrMap {
                    type_args,
                    elements,
                    ..
                } if elements.is_empty() => {
                    excluded.extend(type_args.iter().copied());
                }
                _ => {}
            }
        }
        for st in a.stmts.iter() {
            match &st.kind {
                StmtKind::Variables(l)
                | StmtKind::For {
                    init: Some(ForInit::Variables(l)),
                    ..
                } => excluded.extend(l.ty),
                _ => {}
            }
        }
        let mut augmented: Vec<Span> = Vec::new();
        for d in a.decls.iter() {
            match &d.kind {
                DeclKind::Variables(l) => excluded.extend(l.ty),
                DeclKind::Extension(x) => {
                    excluded.insert(x.on);
                }
                DeclKind::ExtensionType(x) => {
                    excluded.insert(x.representation_type);
                }
                _ => {}
            }
            if d.augment {
                augmented.push(d.span);
            }
        }
        for m in a.members.iter() {
            if m.augment {
                augmented.push(m.span);
            }
        }
        // The return of a method that overrides a member whose return is
        // neither `void` nor `FutureOr<void>`.
        for (k, d) in a.decls.iter().enumerate() {
            let members = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => continue,
            };
            let Some(class_id) = class_of_decl(s, DeclId(k as u32)) else {
                continue;
            };
            for &mid in members {
                let MemberKind::Method(fid) = &a.member(mid).kind else {
                    continue;
                };
                let f = a.function(*fid);
                let (Some(rt), Some(n)) = (f.return_type, f.name) else {
                    continue;
                };
                // The overridden member (`overriddenMember`): the first of the
                // supertypes with the name.
                let key = if f.kind == dartforge_frontend::ast::FunctionKind::Setter {
                    interner.lookup(&format!("{}_=", interner.resolve(n.sym)))
                } else {
                    Some(n.sym)
                };
                let Some(ch) = key else { continue };
                let mut sup: Vec<ClassId> = s
                    .outline
                    .hierarchy
                    .get(class_id)
                    .map(|h| h.supertypes.keys().copied().collect())
                    .unwrap_or_default();
                sup.sort();
                let overridden = sup
                    .iter()
                    .find_map(|sc| program.class(*sc).instance_members.get(&ch).copied());
                if let Some(g) = overridden
                    && let Some(data) = s.outline.functions.get(g.0 as usize)
                {
                    let r = data.return_type;
                    let is_void = matches!(table.get(r), Type::Void)
                        || matches!(table.get(r), Type::FutureOr { arg, .. } if matches!(table.get(*arg), Type::Void));
                    if !is_void {
                        excluded.insert(rt);
                    }
                }
            }
        }
        for (k, t) in a.types.iter().enumerate() {
            let id = TypeId(k as u32);
            let TypeKind::Named { name, .. } = &t.kind else {
                continue;
            };
            if excluded.contains(&id)
                || augmented
                    .iter()
                    .any(|sp| t.span.start >= sp.start && t.span.end <= sp.end)
            {
                continue;
            }
            let ty = s
                .body
                .annotation_types
                .get(&id)
                .copied()
                .or_else(|| s.outline.written_types.get(&(s.unit, id)).copied());
            if ty.is_some_and(|x| matches!(table.get(x), Type::Null)) {
                report(&c::PREFER_VOID_TO_NULL, name[name.len() - 1].span, &[]);
            }
        }
    }

    // `type_annotate_public_apis`.
    if enabled("type_annotate_public_apis") {
        let is_private = |n: SymbolId| interner.resolve(n).starts_with('_');
        let underscores = |n: SymbolId| {
            let t = interner.resolve(n);
            !t.is_empty() && t.bytes().all(|c| c == b'_')
        };
        let mut findings: Vec<Span> = Vec::new();
        fn check_params(
            ps: &[Parameter],
            underscores: &dyn Fn(SymbolId) -> bool,
            findings: &mut Vec<Span>,
        ) {
            for p in ps {
                if let Some(inner) = &p.function_parameters {
                    check_params(inner, underscores, findings);
                    continue;
                }
                if p.this_ || p.super_ {
                    continue;
                }
                if p.ty.is_none()
                    && let Some(n) = p.name
                    && !underscores(n.sym)
                {
                    findings.push(p.span);
                }
            }
        }
        let variables = |l: &dartforge_frontend::ast::VariableList, findings: &mut Vec<Span>| {
            if l.ty.is_some() {
                return;
            }
            for v in l.variables.iter() {
                if is_private(v.name.sym) || l.const_ {
                    continue;
                }
                let inferred = v
                    .initializer
                    .and_then(|i| s.body.get_type(i))
                    .is_some_and(|t| !matches!(table.get(t), Type::Dynamic | Type::Null));
                if l.final_ && inferred {
                    continue;
                }
                findings.push(v.name.span);
            }
        };
        for d in a.decls.iter() {
            if d.augment {
                continue;
            }
            match &d.kind {
                DeclKind::Variables(l) => variables(l, &mut findings),
                DeclKind::Function(f) => {
                    let f = a.function(*f);
                    let Some(n) = f.name else { continue };
                    if is_private(n.sym) {
                        continue;
                    }
                    if f.return_type.is_none()
                        && f.kind != dartforge_frontend::ast::FunctionKind::Setter
                    {
                        findings.push(n.span);
                    } else if let Some(ps) = &f.parameters {
                        check_params(ps, &underscores, &mut findings);
                    }
                }
                DeclKind::Typedef(x) => {
                    if is_private(x.name.sym) {
                        continue;
                    }
                    if let TypedefKind::Legacy {
                        return_type,
                        parameters,
                    } = &x.kind
                    {
                        if return_type.is_none() {
                            findings.push(x.name.span);
                        } else {
                            check_params(parameters, &underscores, &mut findings);
                        }
                    }
                }
                _ => {}
            }
        }
        for m in a.members.iter() {
            if m.augment {
                continue;
            }
            match &m.kind {
                MemberKind::Field(l) => variables(l, &mut findings),
                MemberKind::Method(f) => {
                    let f = a.function(*f);
                    let Some(n) = f.name else { continue };
                    if is_private(n.sym) {
                        continue;
                    }
                    if f.return_type.is_none()
                        && f.kind != dartforge_frontend::ast::FunctionKind::Setter
                    {
                        findings.push(n.span);
                    } else if let Some(ps) = &f.parameters {
                        check_params(ps, &underscores, &mut findings);
                    }
                }
                MemberKind::Constructor(_) => {}
            }
        }
        findings.sort_by_key(|x| x.start);
        for sp in findings {
            report(&c::TYPE_ANNOTATE_PUBLIC_APIS, sp, &[]);
        }
    }

    out
}
