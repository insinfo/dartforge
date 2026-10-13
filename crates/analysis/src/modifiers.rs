//! Class modifiers (`base`, `final`, `interface`, `sealed`) across libraries:
//! the checks of the `ErrorVerifier` of analyzer 7.x
//! (`_checkForBaseClassOrMixinImplementedOutsideOfLibrary`,
//! `_checkForInterfaceClassOrMixinSuperclassOutsideOfLibrary`,
//! `_checkForFinalSupertypeOutsideOfLibrary`,
//! `_checkForSealedSupertypeOutsideOfLibrary`) and the
//! `BaseOrFinalTypeVerifier` (`subtype_of_base_or_final_is_not_base_final_or_sealed`).
//!
//! Only names and the declared hierarchy: each type of the
//! `extends`/`with`/`implements`/`on` clauses is resolved by name in the
//! library's scope. A clause that does not resolve to a class or mixin is not
//! checked, and the `ErrorVerifier` checks only run with the
//! `_checkClassInheritance` gate open (`clauses::gate`): a class with a
//! forbidden supertype (`extends int`, `extends Enum`), a deferred clause or a
//! mixin error is left out, as in the analyzer.

use dartforge_diagnostics::{codigos::compile_time_error as c, Codigo, Diagnostic, Span};
use dartforge_elements::model::{ClassId, ClassKind, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, TypeKind};
use dartforge_intern::Interner;

/// The clauses of a declaration, as written: `(written type, class)`.
struct Clauses {
    extends: Option<(ast::TypeId, Option<ClassId>)>,
    with: Vec<(ast::TypeId, Option<ClassId>)>,
    implements: Vec<(ast::TypeId, Option<ClassId>)>,
    on: Vec<(ast::TypeId, Option<ClassId>)>,
}

struct Ctx<'a> {
    program: &'a Program,
    names: &'a Interner,
    lib: LibraryId,
}

impl Ctx<'_> {
    fn class_of(&self, id: ClassId) -> &dartforge_elements::model::ClassElement {
        self.program.class(id)
    }

    fn name_of(&self, id: ClassId) -> String {
        self.names.resolve(self.class_of(id).name).to_string()
    }

    fn is_base(&self, id: ClassId) -> bool {
        let c = self.class_of(id);
        matches!(c.kind, ClassKind::Class | ClassKind::Mixin | ClassKind::MixinApplication) && c.modifiers.base
    }

    fn is_final(&self, id: ClassId) -> bool {
        let c = self.class_of(id);
        matches!(c.kind, ClassKind::Class | ClassKind::MixinApplication) && c.modifiers.final_
    }

    fn is_sealed(&self, id: ClassId) -> bool {
        let c = self.class_of(id);
        matches!(c.kind, ClassKind::Class | ClassKind::MixinApplication) && c.modifiers.sealed
    }

    fn is_interface(&self, id: ClassId) -> bool {
        let c = self.class_of(id);
        matches!(c.kind, ClassKind::Class | ClassKind::MixinApplication) && c.modifiers.interface
    }

    fn is_mixin(&self, id: ClassId) -> bool {
        self.class_of(id).kind == ClassKind::Mixin
    }

    /// Does the library of `id` have class modifiers (language 3.0+)?
    fn has_modifiers(&self, lib: LibraryId) -> bool {
        self.program.library(lib).features.versao().major >= 3
    }

    /// `_mayIgnoreClassModifiers`: a modifier of a platform library, seen from
    /// the SDK itself or from a library older than the modifiers (`dart:ffi`
    /// is not part of the `ErrorVerifier`'s first exception).
    fn may_ignore(&self, from: LibraryId, ffi_counts: bool) -> bool {
        let origem = self.program.library(from);
        if !origem.is_sdk {
            return false;
        }
        if ffi_counts && origem.uri == "dart:ffi" {
            return false;
        }
        let current = self.program.library(self.lib);
        current.is_sdk || !self.has_modifiers(self.lib)
    }

    /// The declared direct supertypes of a class (supertype, interfaces,
    /// mixins, `on`), through the elements already bound in the model.
    fn direct(&self, id: ClassId) -> Vec<ClassId> {
        let c = self.class_of(id);
        let mut v: Vec<ClassId> = c.supertype_class.into_iter().collect();
        v.extend(c.interface_classes.iter().copied());
        v.extend(c.mixin_classes.iter().copied());
        v.extend(c.on_classes.iter().copied());
        v
    }

    /// `[type, ...allSupertypes]`, without repetition, depth first.
    fn all_supertypes(&self, id: ClassId) -> Vec<ClassId> {
        let mut seen = vec![id];
        let mut stack = vec![id];
        while let Some(x) = stack.pop() {
            for s in self.direct(x) {
                if !seen.contains(&s) {
                    seen.push(s);
                    stack.push(s);
                }
            }
        }
        seen
    }

    /// `_getExplicitlyBaseOrFinalElement`.
    fn explicit_base_or_final(&self, id: ClassId, visited: &mut Vec<ClassId>) -> Option<ClassId> {
        if visited.contains(&id) {
            return None;
        }
        visited.push(id);
        if (self.is_base(id) || self.is_final(id)) && !self.is_sealed(id) {
            return Some(id);
        }
        let c = self.class_of(id);
        let order = c
            .supertype_class
            .into_iter()
            .chain(c.interface_classes.iter().copied())
            .chain(c.mixin_classes.iter().copied())
            .chain(if c.kind == ClassKind::Mixin { c.on_classes.clone() } else { Vec::new() });
        for s in order.collect::<Vec<_>>() {
            if let Some(r) = self.explicit_base_or_final(s, visited) {
                return Some(r);
            }
        }
        None
    }
}

/// The class-modifier diagnostics of library `lib`.
pub fn outside_library(program: &Program, lib: LibraryId, names: &Interner) -> Vec<(UnitId, Diagnostic)> {
    let cx = Ctx { program, names, lib };
    let mut out = Vec::new();
    for (i, class_of) in program.classes.iter().enumerate() {
        if class_of.library != lib {
            continue;
        }
        let Some(decl) = class_of.decl else { continue };
        let id = ClassId(i as u32);
        let ast = &program.unit(decl.unit).ast;
        let Some(cl) = clauses(program, decl.unit, ast, &ast.decl(decl.decl).kind) else { continue };
        let span = |t: ast::TypeId| ast.ty(t).span;
        // The analyzer drops the repeated error (same code, range and
        // message): the `ErrorVerifier` and the `BaseOrFinalTypeVerifier`
        // each report `class X implements SealedOfBase`.
        let mut by = |code: Codigo, sp: Span, args: &[&str], context: Option<dartforge_diagnostics::Contexto>| {
            let mut d = Diagnostic::com_codigo(code, sp, args.iter().copied());
            d.contexto.extend(context);
            if !out.iter().any(|(u, x): &(UnitId, Diagnostic)| *u == decl.unit && x.span == d.span && x.message == d.message) {
                out.push((decl.unit, d));
            }
        };

        // `ErrorVerifier`: only with the `_checkClassInheritance` gate open (no
        // forbidden supertype, deferred clause or mixin error).
        if crate::clauses::gate(program, lib, names, id) == crate::clauses::Gate::Open {
            // base implemented outside the library.
            for &(t, target) in &cl.implements {
                let Some(target) = target else { continue };
                for e in cx.all_supertypes(target) {
                    if cx.is_base(e) && cx.class_of(e).library != lib && !cx.may_ignore(cx.class_of(e).library, true) {
                        if cx.class_of(e).kind != ClassKind::Mixin && !cx.is_sealed(e) {
                            by(c::BASE_CLASS_IMPLEMENTED_OUTSIDE_OF_LIBRARY, span(t), &[&cx.name_of(e)], None);
                        } else if cx.is_mixin(e) {
                            by(c::BASE_MIXIN_IMPLEMENTED_OUTSIDE_OF_LIBRARY, span(t), &[&cx.name_of(e)], None);
                        }
                        break;
                    }
                }
            }
            // interface extended outside the library.
            if let Some((t, Some(s))) = cl.extends {
                // After `no_generative_constructors_in_superclass`
                // (`extends Finalizable` only reports that one; `extends
                // Pointer`, `final` and also only with `factory`, reports the modifier).
                if cx.is_interface(s)
                    && !cx.is_sealed(s)
                    && cx.class_of(s).library != lib
                    && !cx.may_ignore(cx.class_of(s).library, true)
                    && !without_generative_constructor(&cx, s)
                {
                    by(c::INTERFACE_CLASS_EXTENDED_OUTSIDE_OF_LIBRARY, span(t), &[&cx.name_of(s)], None);
                }
            }
            // final: extended, implemented, mixin constraint.
            if let Some((t, Some(s))) = cl.extends {
                if cx.is_final(s) && !cx.is_sealed(s) && cx.class_of(s).library != lib && !cx.may_ignore(cx.class_of(s).library, true) {
                    by(c::FINAL_CLASS_EXTENDED_OUTSIDE_OF_LIBRARY, span(t), &[&cx.name_of(s)], None);
                }
            }
            for &(t, target) in &cl.implements {
                let Some(target) = target else { continue };
                for e in cx.all_supertypes(target) {
                    if cx.is_final(e) && !cx.is_sealed(e) && cx.class_of(e).library != lib && !cx.may_ignore(cx.class_of(e).library, true) {
                        // Indirect in a library with modifiers: the nearest
                        // declaration already reports.
                        if e != target && cx.has_modifiers(cx.class_of(target).library) {
                            continue;
                        }
                        by(c::FINAL_CLASS_IMPLEMENTED_OUTSIDE_OF_LIBRARY, span(t), &[&cx.name_of(e)], None);
                        break;
                    }
                }
            }
            for &(t, target) in &cl.on {
                let Some(s) = target else { continue };
                if cx.is_final(s) && !cx.is_sealed(s) && cx.class_of(s).library != lib && !cx.may_ignore(cx.class_of(s).library, true) {
                    by(c::FINAL_CLASS_USED_AS_MIXIN_CONSTRAINT_OUTSIDE_OF_LIBRARY, span(t), &[&cx.name_of(s)], None);
                }
            }
            // sealed: qualquer cláusula.
            for &(t, target) in cl.extends.iter().chain(cl.with.iter()).chain(cl.implements.iter()).chain(cl.on.iter()) {
                let Some(s) = target else { continue };
                if cx.is_sealed(s) && cx.class_of(s).library != lib {
                    by(c::SEALED_CLASS_SUBTYPE_OUTSIDE_OF_LIBRARY, span(t), &[&cx.name_of(s)], None);
                }
            }
        }

        // `BaseOrFinalTypeVerifier.checkElement`: classes, class aliases and
        // mixins (not enums).
        if !matches!(class_of.kind, ClassKind::Class | ClassKind::Mixin | ClassKind::MixinApplication) {
            continue;
        }
        let Some(name_span) = decl_name(&ast.decl(decl.decl).kind) else { continue };
        let supertipo: Vec<(Option<ast::TypeId>, ClassId)> = cl.extends.and_then(|(_, c)| c).map(|c| (None, c)).into_iter().collect();
        let interfaces: Vec<(Option<ast::TypeId>, ClassId)> =
            cl.implements.iter().filter_map(|&(t, c)| c.map(|c| (Some(t), c))).collect();
        let mixins: Vec<(Option<ast::TypeId>, ClassId)> = cl.with.iter().filter_map(|&(_, c)| c.map(|c| (None, c))).collect();
        let on: Vec<(Option<ast::TypeId>, ClassId)> = cl.on.iter().filter_map(|&(_, c)| c.map(|c| (None, c))).collect();
        for group in [supertipo, interfaces, mixins, on] {
            let mut relatou = false;
            for (implemented, sup) in group {
                if let Some((code, sp, args, context)) = restriction(&cx, id, sup, implemented.map(span), name_span) {
                    let args: Vec<&str> = args.iter().map(String::as_str).collect();
                    // The context message points at the declaration's file
                    // when it is not the error's.
                    let context = context.map(|(u, span, message)| dartforge_diagnostics::Contexto {
                        arquivo: (u != decl.unit).then(|| program.caminho_da_unidade(u).into()),
                        span,
                        mensagem: message.into(),
                    });
                    by(code, sp, &args, context);
                    relatou = true;
                    break;
                }
            }
            if relatou {
                break;
            }
        }
    }
    out
}

/// `_reportRestrictionError`: the diagnostic, if any, with the context
/// message (unit, range and text) that points at the explicit `base`/`final`
/// when the direct supertype has it induced (it is `sealed`).
#[allow(clippy::type_complexity)]
fn restriction(
    cx: &Ctx<'_>,
    element: ClassId,
    sup: ClassId,
    implemented: Option<Span>,
    name_span: Span,
) -> Option<(Codigo, Span, Vec<String>, Option<(UnitId, Span, String)>)> {
    let lib_el = cx.class_of(element).library;
    let lib_sup = cx.class_of(sup).library;
    // A `sealed` that extends `base`/`final` inherits the modifier (it is an
    // induced `base`/`final`, as in the analyzer's element).
    let induzida = cx.is_sealed(sup) && cx.explicit_base_or_final(sup, &mut Vec::new()).is_some();
    if !(cx.is_base(sup) || cx.is_final(sup) || induzida || (!cx.has_modifiers(lib_sup) && cx.has_modifiers(lib_el))) {
        return None;
    }
    let bf = cx.explicit_base_or_final(sup, &mut Vec::new())?;
    let lib_bf = cx.class_of(bf).library;
    if cx.may_ignore(lib_bf, false) {
        return None;
    }
    let context = cx.program.nome_da_classe(bf).map(|(u, span)| {
        let (s, b) = (cx.name_of(sup), cx.name_of(bf));
        (u, span, format!("The type '{s}' is a subtype of '{b}', and '{b}' is defined here."))
    });
    let se_selada = |x: Option<(UnitId, Span, String)>| if cx.is_sealed(sup) { x } else { None };
    if let Some(sp) = implemented {
        if cx.is_sealed(sup) && lib_bf != lib_el && cx.is_base(bf) {
            let code = if cx.is_mixin(bf) { c::BASE_MIXIN_IMPLEMENTED_OUTSIDE_OF_LIBRARY } else { c::BASE_CLASS_IMPLEMENTED_OUTSIDE_OF_LIBRARY };
            return Some((code, sp, vec![cx.name_of(bf)], context));
        }
    }
    if cx.is_base(element) || cx.is_final(element) || cx.is_sealed(element) {
        return None;
    }
    let mixin = cx.is_mixin(element);
    if cx.is_final(bf) {
        if lib_bf != lib_el
            && (cx.has_modifiers(lib_sup) || !cx.program.library(lib_bf).is_sdk || implemented.is_some())
        {
            return None;
        }
        let code = if mixin { c::MIXIN_SUBTYPE_OF_FINAL_IS_NOT_BASE } else { c::SUBTYPE_OF_FINAL_IS_NOT_BASE_FINAL_OR_SEALED };
        return Some((code, name_span, vec![cx.name_of(element), cx.name_of(bf)], se_selada(context)));
    }
    if cx.is_base(bf) {
        let code = if mixin { c::MIXIN_SUBTYPE_OF_BASE_IS_NOT_BASE } else { c::SUBTYPE_OF_BASE_IS_NOT_BASE_FINAL_OR_SEALED };
        return Some((code, name_span, vec![cx.name_of(element), cx.name_of(bf)], se_selada(context)));
    }
    None
}

/// `_checkForNoGenerativeConstructorsInSuperclass`: the superclass only has
/// `factory` constructors (like `Finalizable`).
fn without_generative_constructor(cx: &Ctx<'_>, id: ClassId) -> bool {
    let c = cx.class_of(id);
    c.kind == ClassKind::Class
        && !c.constructors.is_empty()
        && c.constructors.values().all(|&f| cx.program.function(f).factory)
}

fn decl_name(k: &DeclKind) -> Option<Span> {
    match k {
        DeclKind::Class(x) => Some(x.name.span),
        DeclKind::Mixin(x) => Some(x.name.span),
        _ => None,
    }
}

fn clauses(program: &Program, u: UnitId, ast: &ast::Ast, k: &DeclKind) -> Option<Clauses> {
    let r = |t: ast::TypeId| (t, class_of_type(program, u, ast, t));
    match k {
        DeclKind::Class(x) => Some(Clauses {
            extends: x.extends.map(r),
            with: x.with.iter().map(|&t| r(t)).collect(),
            implements: x.implements.iter().map(|&t| r(t)).collect(),
            on: Vec::new(),
        }),
        DeclKind::Mixin(x) => Some(Clauses {
            extends: None,
            with: Vec::new(),
            implements: x.implements.iter().map(|&t| r(t)).collect(),
            on: x.on.iter().map(|&t| r(t)).collect(),
        }),
        DeclKind::Enum(x) => Some(Clauses {
            extends: None,
            with: x.with.iter().map(|&t| r(t)).collect(),
            implements: x.implements.iter().map(|&t| r(t)).collect(),
            on: Vec::new(),
        }),
        _ => None,
    }
}

/// The class (or mixin) the written type names in the scope of unit `u`.
fn class_of_type(program: &Program, u: UnitId, ast: &ast::Ast, t: ast::TypeId) -> Option<ClassId> {
    class_of_type_in(program, u, ast, t, 0)
}

/// The element of a clause's type is that of the already expanded interface:
/// the alias (`typedef T = FinalClass;`, also from another library) leads to
/// the class it names; what expands to a type parameter does not.
fn class_of_type_in(program: &Program, u: UnitId, ast: &ast::Ast, t: ast::TypeId, depth: u32) -> Option<ClassId> {
    if depth > 8 {
        return None;
    }
    let TypeKind::Named { name, .. } = &ast.ty(t).kind else { return None };
    let b = match &name[..] {
        [n] => program.lookup_na_unidade(u, n.sym),
        [p, n] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym),
        _ => None,
    }?;
    if b.ambiguous {
        return None;
    }
    match b.getter? {
        Element::Class(c) if matches!(program.class(c).kind, ClassKind::Class | ClassKind::Mixin | ClassKind::MixinApplication) => Some(c),
        Element::Typedef(tid) => {
            let td = program.typedef(tid);
            let ast_td = &program.unit(td.decl.unit).ast;
            let DeclKind::Typedef(d) = &ast_td.decl(td.decl.decl).kind else { return None };
            let ast::TypedefKind::Alias(body) = d.kind else { return None };
            if let TypeKind::Named { name: n2, .. } = &ast_td.ty(body).kind
                && let [unique] = &n2[..]
                && d.type_params.iter().any(|p| p.name.sym == unique.sym)
            {
                return None;
            }
            class_of_type_in(program, td.decl.unit, ast_td, body, depth + 1)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_elements::{load::load_lenient, sdk::SdkLayout};
    use std::fs;

    #[test]
    fn modifiers_from_another_library() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../target/tmp-agent/modificadores-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sdk/lib/core")).unwrap();
        fs::write(root.join("sdk/lib/libraries.json"), r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#).unwrap();
        fs::write(root.join("sdk/lib/core/core.dart"), "class Object {}").unwrap();
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        fs::write(
            root.join("a.dart"),
            "base class B {}\nfinal class F {}\ninterface class I {}\nsealed class S {}\nsealed class SB extends B {}\n",
        )
        .unwrap();
        let source = "import 'a.dart';\n\
            class X extends B {}\n\
            base class Y implements B {}\n\
            final class Z extends F {}\n\
            class W extends I {}\n\
            final class V extends S {}\n\
            base class Q implements SB {}\n\
            base class Ok extends B {}\n";
        let entry = root.join("main.dart");
        fs::write(&entry, source).unwrap();
        let mut names = Interner::new();
        let (program, _) = load_lenient(&entry, &sdk, None, &mut names);
        let diags = outside_library(&program, program.entry.unwrap(), &names);
        let v: Vec<(&str, &str, &str)> = diags
            .iter()
            .map(|(_, d)| (d.code.map_or("", |c| c.info().nome), &source[d.span.start..d.span.end], d.message.as_str()))
            .collect();
        assert_eq!(
            v,
            vec![
                ("subtype_of_base_or_final_is_not_base_final_or_sealed", "X", "The type 'X' must be 'base', 'final' or 'sealed' because the supertype 'B' is 'base'."),
                ("invalid_use_of_type_outside_library", "B", "The class 'B' can't be implemented outside of its library because it's a base class."),
                ("invalid_use_of_type_outside_library", "F", "The class 'F' can't be extended outside of its library because it's a final class."),
                ("invalid_use_of_type_outside_library", "I", "The class 'I' can't be extended outside of its library because it's an interface class."),
                ("invalid_use_of_type_outside_library", "S", "The class 'S' can't be extended, implemented, or mixed in outside of its library because it's a sealed class."),
                // The `ErrorVerifier` and the `BaseOrFinalTypeVerifier` report
                // the same error; the analyzer keeps only one.
                ("invalid_use_of_type_outside_library", "SB", "The class 'B' can't be implemented outside of its library because it's a base class."),
                ("invalid_use_of_type_outside_library", "SB", "The class 'SB' can't be extended, implemented, or mixed in outside of its library because it's a sealed class."),
            ],
            "{diags:?}"
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
