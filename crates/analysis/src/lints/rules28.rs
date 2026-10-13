//! The twenty-eighth batch of lint rules
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `public_member_api_docs`: only in a pub package (there is a
//!   `pubspec.yaml` above the library) and inside its `lib/`. The public
//!   declarations without a documentation comment that do not override an
//!   inherited member (`overriddenMember`: some supertype of the class, mixin
//!   or enum that contains them has an instance member with the name, a
//!   private one only in the same library), with the emitter's exclusions:
//!   `@internal` (`meta`) on the enclosing top-level declaration, a private
//!   name, `main`, a setter whose getter is documented, an enum constructor
//!   and that of a `sealed`, `abstract final` or `abstract interface` class,
//!   an unnamed extension, an extension type instance field. The reported
//!   node is the name (the return type for an unnamed constructor).
//!
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::rules::LintReport;
use super::rules14::{after_annotations, doc_for, skip_augment};
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::ClassId;
use dartforge_frontend::ast::{self, Annotation, DeclId, DeclKind, FunctionKind, MemberId, MemberKind};
use dartforge_frontend::comentarios::Comentarios;
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    if !enabled("public_member_api_docs") {
        return out;
    }
    let a = u.ast;
    let source = u.fonte;
    let program = s.program;
    // `package.canHavePublicApi` (only a pub package) and `isInLibDir` (by the
    // unit that defines the library).
    let lib = program.library(program.unit(s.unit).library);
    let Some(file) = lib.units.first().and_then(|x| program.unit(*x).path.clone()) else { return out };
    let Some(root) = file.ancestors().skip(1).find(|d| d.join("pubspec.yaml").is_file()).map(std::path::Path::to_path_buf) else { return out };
    if !file.starts_with(root.join("lib")) {
        return out;
    }
    let comments = Comentarios::de(source);
    let is_private = |n: SymbolId| interner.resolve(n).starts_with('_');
    let is_internal = |metadata: &[Annotation]| metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unit, m, "meta", "internal"));
    let is_documented = |metadata: &[Annotation], pos: usize| doc_for(source, &comments, metadata, pos).is_some();
    let class_of_decl = |d: DeclId| -> Option<ClassId> {
        (0..program.classes.len()).map(|i| ClassId(i as u32)).find(|c| program.class(*c).decl.is_some_and(|r| r.unit == s.unit && r.decl == d))
    };
    // `overriddenMember`: the name among the instance members of the
    // supertypes of the enclosing class (a setter with `_=`).
    let overrides = |class_id: Option<ClassId>, name: SymbolId, setter: bool| -> bool {
        let Some(class_id) = class_id else { return false };
        let key = if setter { interner.lookup(&format!("{}_=", interner.resolve(name))) } else { Some(name) };
        let Some(key) = key else { return false };
        let own = program.class(class_id).library;
        let is_private_name = is_private(name);
        s.outline.hierarchy.get(class_id).is_some_and(|h| {
            h.supertypes.keys().any(|sc| {
                *sc != class_id && program.class(*sc).instance_members.get(&key).is_some_and(|f| !is_private_name || program.function(*f).library == own)
            })
        })
    };
    let mut findings: Vec<Span> = Vec::new();
    let check = |doc: bool, overridden_member: bool, node: Span, findings: &mut Vec<Span>| -> bool {
        if !doc && !overridden_member {
            findings.push(node);
            return true;
        }
        false
    };
    let member_pos = |m: MemberId| after_annotations(source, &a.member(m).metadata, a.member(m).span.start);
    let decl_pos = |d: &ast::Decl| {
        let mut pos = after_annotations(source, &d.metadata, d.span.start);
        if d.augment && matches!(d.kind, DeclKind::Enum(_) | DeclKind::Extension(_) | DeclKind::ExtensionType(_) | DeclKind::Typedef(_) | DeclKind::Mixin(_)) {
            pos = skip_augment(source, pos);
        }
        pos
    };
    // `checkMethods`: the public methods (getters, setters, operators).
    let methods = |members: &[MemberId], class_id: Option<ClassId>, findings: &mut Vec<Span>| {
        let mut getters: HashMap<SymbolId, MemberId> = HashMap::new();
        let mut getter_order: Vec<SymbolId> = Vec::new();
        let mut setters: Vec<MemberId> = Vec::new();
        let mut others: Vec<MemberId> = Vec::new();
        for &m in members {
            let MemberKind::Method(f) = &a.member(m).kind else { continue };
            let f = a.function(*f);
            let Some(n) = f.name else { continue };
            if is_private(n.sym) {
                continue;
            }
            match f.kind {
                FunctionKind::Getter => {
                    if getters.insert(n.sym, m).is_none() {
                        getter_order.push(n.sym);
                    }
                }
                FunctionKind::Setter => setters.push(m),
                _ => others.push(m),
            }
        }
        let mut undocumented: Vec<MemberId> = Vec::new();
        let check_method = |m: MemberId, findings: &mut Vec<Span>| -> bool {
            let MemberKind::Method(f) = &a.member(m).kind else { return false };
            let f = a.function(*f);
            let Some(n) = f.name else { return false };
            let doc = is_documented(&a.member(m).metadata, member_pos(m));
            let overridden_member = overrides(class_id, n.sym, f.kind == FunctionKind::Setter);
            if !doc && !overridden_member {
                findings.push(n.span);
                return true;
            }
            false
        };
        for n in getter_order {
            let m = getters[&n];
            if check_method(m, findings) {
                undocumented.push(m);
            }
        }
        for m in setters {
            let MemberKind::Method(f) = &a.member(m).kind else { continue };
            let Some(n) = a.function(*f).name else { continue };
            if getters.get(&n.sym).is_some_and(|g| undocumented.contains(g)) {
                check_method(m, findings);
            }
        }
        for m in others {
            check_method(m, findings);
        }
    };

    // `visitCompilationUnit`: the top-level functions.
    {
        let mut getters: HashMap<SymbolId, &ast::Decl> = HashMap::new();
        let mut order: Vec<SymbolId> = Vec::new();
        let mut setters: Vec<&ast::Decl> = Vec::new();
        let mut functions: Vec<&ast::Decl> = Vec::new();
        for d in a.decls.iter() {
            let DeclKind::Function(f) = &d.kind else { continue };
            let f = a.function(*f);
            let Some(n) = f.name else { continue };
            if is_private(n.sym) || interner.resolve(n.sym) == "main" {
                continue;
            }
            match f.kind {
                FunctionKind::Getter => {
                    if getters.insert(n.sym, d).is_none() {
                        order.push(n.sym);
                    }
                }
                FunctionKind::Setter => setters.push(d),
                _ => functions.push(d),
            }
        }
        let name = |d: &ast::Decl| match &d.kind {
            DeclKind::Function(f) => a.function(*f).name,
            _ => None,
        };
        let mut undocumented: Vec<*const ast::Decl> = Vec::new();
        for n in order {
            let d = getters[&n];
            if check(is_documented(&d.metadata, decl_pos(d)), false, name(d).map_or(d.span, |x| x.span), &mut findings) {
                undocumented.push(d as *const ast::Decl);
            }
        }
        for d in setters {
            let Some(n) = name(d) else { continue };
            if getters.get(&n.sym).is_some_and(|g| undocumented.contains(&(*g as *const ast::Decl))) {
                check(is_documented(&d.metadata, decl_pos(d)), false, n.span, &mut findings);
            }
        }
        for d in functions {
            // `isEffectivelyPrivate`: the function's own `@internal`.
            if is_internal(&d.metadata) {
                continue;
            }
            let n = name(d).map_or(d.span, |x| x.span);
            check(is_documented(&d.metadata, decl_pos(d)), false, n, &mut findings);
        }
    }

    for (k, d) in a.decls.iter().enumerate() {
        let id = DeclId(k as u32);
        let class_id = class_of_decl(id);
        let doc = || is_documented(&d.metadata, decl_pos(d));
        // The type itself "overrides" when a supertype has a member with its
        // name (`overriddenMember` of the class's element).
        match &d.kind {
            DeclKind::Class(x) if x.mixin_application => {
                // `visitClassTypeAlias`.
                if !is_private(x.name.sym) {
                    let overridden = overrides(class_id, x.name.sym, false);
                    check(doc(), overridden, x.name.span, &mut findings);
                }
                continue;
            }
            DeclKind::Class(x) => {
                if is_internal(&d.metadata) || is_private(x.name.sym) {
                    continue;
                }
                let overridden = overrides(class_id, x.name.sym, false);
                check(doc(), overridden, x.name.span, &mut findings);
                methods(&x.members, class_id, &mut findings);
            }
            DeclKind::Mixin(x) => {
                if is_internal(&d.metadata) || is_private(x.name.sym) {
                    continue;
                }
                let overridden = overrides(class_id, x.name.sym, false);
                check(doc(), overridden, x.name.span, &mut findings);
                methods(&x.members, class_id, &mut findings);
            }
            DeclKind::Enum(x) => {
                if is_private(x.name.sym) || is_internal(&d.metadata) {
                    continue;
                }
                let overridden = overrides(class_id, x.name.sym, false);
                check(doc(), overridden, x.name.span, &mut findings);
                methods(&x.members, class_id, &mut findings);
            }
            DeclKind::Extension(x) => {
                let Some(n) = x.name else { continue };
                if is_private(n.sym) || is_internal(&d.metadata) {
                    continue;
                }
                check(doc(), false, n.span, &mut findings);
                methods(&x.members, None, &mut findings);
            }
            DeclKind::ExtensionType(x) => {
                if is_internal(&d.metadata) || is_private(x.name.sym) {
                    continue;
                }
                let overridden = overrides(class_id, x.name.sym, false);
                check(doc(), overridden, x.name.span, &mut findings);
                methods(&x.members, class_id, &mut findings);
            }
            DeclKind::Typedef(x) => {
                if !is_private(x.name.sym) {
                    check(doc(), false, x.name.span, &mut findings);
                }
                continue;
            }
            DeclKind::Variables(l) => {
                // `visitTopLevelVariableDeclaration`: the comment is the
                // declaration's.
                for v in l.variables.iter() {
                    if !is_private(v.name.sym) {
                        check(doc(), false, v.name.span, &mut findings);
                    }
                }
                continue;
            }
            DeclKind::Function(_) => continue,
        }
    }

    // `visitConstructorDeclaration`, `visitFieldDeclaration` and
    // `visitEnumConstantDeclaration`, through the parent.
    for (k, d) in a.decls.iter().enumerate() {
        let id = DeclId(k as u32);
        let class_id = class_of_decl(id);
        // The parent's `inPrivateMember`, and the top-level declaration's `isInternal`.
        let (parent_name, members, is_enum, is_extension_type): (Option<SymbolId>, &[MemberId], bool, bool) = match &d.kind {
            DeclKind::Class(x) => (Some(x.name.sym), &x.members, false, false),
            DeclKind::Mixin(x) => (Some(x.name.sym), &x.members, false, false),
            DeclKind::Enum(x) => (Some(x.name.sym), &x.members, true, false),
            DeclKind::Extension(x) => (x.name.map(|n| n.sym), &x.members, false, false),
            DeclKind::ExtensionType(x) => (Some(x.name.sym), &x.members, false, true),
            _ => continue,
        };
        let parent_private = parent_name.is_none_or(is_private);
        let parent_internal = is_internal(&d.metadata);
        // The parent's `isEffectivelyPrivate` (for constructors).
        let parent_effectively_private = parent_internal
            || match &d.kind {
                DeclKind::Class(x) => {
                    let m = &x.modifiers;
                    m.sealed || (m.abstract_ && (m.final_ || m.interface))
                }
                _ => false,
            };
        if let DeclKind::Enum(x) = &d.kind
            && !parent_internal
            && !parent_private
        {
            for k in x.constants.iter() {
                if is_private(k.name.sym) {
                    continue;
                }
                let doc = is_documented(&k.metadata, k.name.span.start);
                let overridden = overrides(class_id, k.name.sym, false);
                check(doc, overridden, k.name.span, &mut findings);
            }
        }
        for &m in members {
            let member = a.member(m);
            match &member.kind {
                MemberKind::Constructor(kc) => {
                    if parent_private || kc.name.is_some_and(|n| is_private(n.sym)) || is_enum || parent_effectively_private {
                        continue;
                    }
                    let node = kc.name.map_or(kc.class_name.span, |n| n.span);
                    let name = match kc.name {
                        Some(n) => Some(n.sym),
                        None => interner.lookup(""),
                    };
                    let overridden = name.is_some_and(|n| overrides(class_id, n, false));
                    check(is_documented(&member.metadata, member_pos(m)), overridden, node, &mut findings);
                }
                MemberKind::Field(l) => {
                    if parent_internal || parent_private || (is_extension_type && !l.static_) {
                        continue;
                    }
                    let doc = is_documented(&member.metadata, member_pos(m));
                    for v in l.variables.iter() {
                        if is_private(v.name.sym) {
                            continue;
                        }
                        let overridden = overrides(class_id, v.name.sym, false);
                        check(doc, overridden, v.name.span, &mut findings);
                    }
                }
                _ => {}
            }
        }
    }

    findings.sort_by_key(|x| (x.start, x.end));
    findings.dedup();
    for sp in findings {
        report(&mut out, &c::PUBLIC_MEMBER_API_DOCS, sp);
    }
    out
}

fn report(out: &mut Vec<LintReport>, code: &'static CodigoLint, span: Span) {
    out.push(LintReport { code, span, args: Vec::new() });
}
