//! The analyzer 3.6.2 `OverrideVerifier` and `RedeclareVerifier`
//! (`analyzer/lib/src/error/override_verifier.dart`,
//! `redeclare_verifier.dart`; docs/ANALYZER-ESPECIFICACAO-INFRA.md, Part
//! III, step 6), the `invalid_override_of_non_virtual_member` of the
//! `BestPracticesVerifier` and the lint `annotate_overrides`, all on top of
//! the `InheritanceManager3` of [`crate::heranca`]:
//!
//! * `override_on_non_overriding_{method,getter,setter,field}`: a member with
//!   `@override` (the `override` getter of `dart.core`) whose `getOverridden2`
//!   in the class, enum or mixin is null; in extensions and extension types
//!   the verifier has no current class, and every `@override` is reported;
//! * `redeclare_on_non_redeclaring_member`: an extension type instance member
//!   with `@redeclare` (from `package:meta`) whose name is not in the
//!   interface's `redeclared`;
//! * `invalid_override_of_non_virtual_member`: a member whose
//!   `getMember2(…, forSuper: true)` has `@nonVirtual` from `package:meta`;
//! * `annotate_overrides`: an instance field or method without `@override`
//!   whose `getInherited` (the inherited map) is not null.
//!
//! The names are the `Name`s of the verified library.
//! Written without compiling or running (2026-10-05).

use crate::heranca::{Especie, Heranca, Membro, Nome, ProvedorDoOutline};
use crate::resolve::OutlineTypes;
use crate::table::{CoreTypes, TypeTable};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, FunctionKind as Fk, FunctionRef, LibraryId, Program, UnitId, UnitRole, VariableRef};
use dartforge_frontend::ast::{self, DeclKind, FunctionKind, MemberKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

/// `getOverridden2(class, Name(lib, key)) != null`.
pub(crate) fn sobrescreve(h: &mut Heranca, p: &mut ProvedorDoOutline<'_, '_>, class: ClassId, lib: LibraryId, key: Option<SymbolId>) -> bool {
    let Some(key) = key else { return false };
    let name = Nome::novo(p.interner, lib, key);
    h.sobrescritos(p, class, name).is_some()
}

/// `hasOverride`: the `override` getter of `dart.core`.
fn with_override(program: &Program, interner: &Interner, u: UnitId, metadata: &[ast::Annotation]) -> bool {
    metadata.iter().any(|m| crate::anotacoes::e_getter_de(program, interner, u, m, "dart.core", "override"))
}

/// The setter key of `name`.
fn setter_key(interner: &Interner, name: SymbolId) -> Option<SymbolId> {
    interner.lookup(&format!("{}_=", interner.resolve(name)))
}

/// The field has a setter: not `const`, and not `final` (except `late final`
/// without an initializer).
fn has_setter(l: &ast::VariableList, v: &ast::Variable) -> bool {
    !l.const_ && (!l.final_ || (l.late && v.initializer.is_none()))
}

/// The declarations of the library's units, with the class of each one.
fn declaracoes(program: &Program, lib: LibraryId) -> Vec<(UnitId, ast::DeclId, Option<ClassId>)> {
    let mut classes: HashMap<(UnitId, ast::DeclId), ClassId> = HashMap::new();
    for (i, c) in program.classes.iter().enumerate() {
        if c.library == lib
            && let Some(d) = c.decl
        {
            classes.insert((d.unit, d.decl), ClassId(i as u32));
        }
    }
    let mut v = Vec::new();
    for &u in &program.library(lib).units {
        if program.unit(u).role == UnitRole::Patch {
            continue;
        }
        for &d in program.unit(u).unit.declarations.iter() {
            v.push((u, d, classes.get(&(u, d)).copied()));
        }
    }
    v
}

/// The members of a type declaration.
fn membros(d: &ast::Decl) -> &[ast::MemberId] {
    match &d.kind {
        DeclKind::Class(x) => &x.members,
        DeclKind::Enum(x) => &x.members,
        DeclKind::Mixin(x) => &x.members,
        DeclKind::Extension(x) => &x.members,
        DeclKind::ExtensionType(x) => &x.members,
        _ => &[],
    }
}

/// The `override_on_non_overriding_*` of library `lib`.
pub fn sem_sobrescrita(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    let mut h = Heranca::default();
    let mut p = ProvedorDoOutline { program, interner, core, outline, table };
    let empty: [&str; 0] = [];
    for (u, did, class) in declaracoes(program, lib) {
        let a = &program.unit(u).ast;
        let d = a.decl(did);
        // `_currentClass`: only class, enum and mixin.
        let current = match &d.kind {
            DeclKind::Class(_) | DeclKind::Enum(_) | DeclKind::Mixin(_) => class,
            _ => None,
        };
        let primary = match &d.kind {
            DeclKind::Class(x) => x.primary_constructor,
            DeclKind::Enum(x) => x.primary_constructor,
            _ => None,
        };
        for &mid in membros(d) {
            let mem = a.member(mid);
            // The declaring parameter of the primary constructor (3.13) is the field:
            // its annotation applies to the field, which the elaboration creates
            // without metadata. At the parameter name.
            if primary == Some(mid)
                && let MemberKind::Constructor(k) = &mem.kind
            {
                for par in k.parameters.iter().filter(|x| x.declarante) {
                    let Some(name) = par.name else { continue };
                    if !with_override(program, interner, u, &par.metadata) {
                        continue;
                    }
                    let through_getter = current.is_some_and(|c| sobrescreve(&mut h, &mut p, c, lib, Some(name.sym)));
                    let through_setter = !through_getter
                        && !par.final_
                        && current.is_some_and(|c| sobrescreve(&mut h, &mut p, c, lib, setter_key(interner, name.sym)));
                    if !through_getter && !through_setter {
                        output.push((u, Diagnostic::com_codigo(w::OVERRIDE_ON_NON_OVERRIDING_FIELD, name.span, empty)));
                    }
                }
            }
            if !with_override(program, interner, u, &mem.metadata) {
                continue;
            }
            match &mem.kind {
                MemberKind::Field(l) => {
                    for var in l.variables.iter() {
                        let through_getter = current.is_some_and(|c| sobrescreve(&mut h, &mut p, c, lib, Some(var.name.sym)));
                        let through_setter = !through_getter
                            && has_setter(l, var)
                            && current.is_some_and(|c| sobrescreve(&mut h, &mut p, c, lib, setter_key(interner, var.name.sym)));
                        if !through_getter && !through_setter {
                            output.push((u, Diagnostic::com_codigo(w::OVERRIDE_ON_NON_OVERRIDING_FIELD, var.name.span, empty)));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let func = a.function(*f);
                    let Some(name) = func.name else { continue };
                    let (key, code) = match func.kind {
                        FunctionKind::Getter => (Some(name.sym), w::OVERRIDE_ON_NON_OVERRIDING_GETTER),
                        FunctionKind::Setter => (setter_key(interner, name.sym), w::OVERRIDE_ON_NON_OVERRIDING_SETTER),
                        FunctionKind::Function | FunctionKind::Operator => (Some(name.sym), w::OVERRIDE_ON_NON_OVERRIDING_METHOD),
                    };
                    if !current.is_some_and(|c| sobrescreve(&mut h, &mut p, c, lib, key)) {
                        output.push((u, Diagnostic::com_codigo(code, name.span, empty)));
                    }
                }
                MemberKind::Constructor(_) => {}
            }
        }
    }
    output
}

/// The `RedeclareVerifier`: `redeclare_on_non_redeclaring_member` at the name
/// of a non-static extension type member with `@redeclare` whose name is not
/// in the interface's `redeclared`.
pub fn sem_redeclaracao(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    let mut h = Heranca::default();
    let mut p = ProvedorDoOutline { program, interner, core, outline, table };
    for (u, did, class) in declaracoes(program, lib) {
        let a = &program.unit(u).ast;
        let d = a.decl(did);
        let (DeclKind::ExtensionType(x), Some(c)) = (&d.kind, class) else { continue };
        for &mid in x.members.iter() {
            let mem = a.member(mid);
            let MemberKind::Method(f) = &mem.kind else { continue };
            let func = a.function(*f);
            if func.static_ {
                continue;
            }
            if !mem.metadata.iter().any(|m| crate::anotacoes::e_getter_de(program, interner, u, m, "meta", "redeclare")) {
                continue;
            }
            let Some(name) = func.name else { continue };
            let (key, kind) = match func.kind {
                FunctionKind::Getter => (Some(name.sym), "getter"),
                FunctionKind::Setter => (setter_key(interner, name.sym), "setter"),
                FunctionKind::Function | FunctionKind::Operator => (Some(name.sym), "method"),
            };
            let redeclares = key.is_some_and(|k| {
                let n = Nome::novo(interner, lib, k);
                h.interface(&mut p, c).redeclared.contains_key(&n)
            });
            if !redeclares {
                output.push((u, Diagnostic::com_codigo(w::REDECLARE_ON_NON_REDECLARING_MEMBER, name.span, [kind])));
            }
        }
    }
    output
}

/// The name of an inherited member (`ExecutableElement.name`: a setter with `=`).
fn member_name(program: &Program, interner: &Interner, m: &Membro) -> String {
    let base = interner.resolve(program.function(m.funcao).name);
    let base = base.strip_suffix("_=").unwrap_or(base);
    if m.especie == Especie::Setter { format!("{base}=") } else { base.to_string() }
}

/// The lint `annotate_overrides` (`linter/lib/src/rules/annotate_overrides.dart`):
/// an instance field or method of a class, enum or mixin without `@override`
/// whose `overriddenMember` (`getInherited` by the `Name` of the class's
/// library; a field by the getter name) is not null. Returns the unit, the
/// position of the name and the name of the inherited member.
pub fn sem_anotacao_de_override(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Span, String)> {
    let mut output = Vec::new();
    let mut h = Heranca::default();
    let mut p = ProvedorDoOutline { program, interner, core, outline, table };
    for (u, did, class) in declaracoes(program, lib) {
        let a = &program.unit(u).ast;
        let d = a.decl(did);
        let Some(c) = class else { continue };
        if !matches!(d.kind, DeclKind::Class(_) | DeclKind::Enum(_) | DeclKind::Mixin(_)) {
            continue;
        }
        let library = program.class(c).library;
        for &mid in membros(d) {
            let mem = a.member(mid);
            if mem.augment || with_override(program, interner, u, &mem.metadata) {
                continue;
            }
            match &mem.kind {
                MemberKind::Field(l) if !l.static_ => {
                    for var in l.variables.iter() {
                        let n = Nome::novo(interner, library, var.name.sym);
                        if let Some(m) = h.herdado(&mut p, c, n) {
                            output.push((u, var.name.span, member_name(program, interner, &m)));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let func = a.function(*f);
                    let Some(name) = func.name else { continue };
                    if func.static_ {
                        continue;
                    }
                    let key = match func.kind {
                        FunctionKind::Setter => setter_key(interner, name.sym),
                        _ => Some(name.sym),
                    };
                    let Some(key) = key else { continue };
                    let n = Nome::novo(interner, library, key);
                    if let Some(m) = h.herdado(&mut p, c, n) {
                        output.push((u, name.span, member_name(program, interner, &m)));
                    }
                }
                _ => {}
            }
        }
    }
    output
}

/// `_hasNonVirtualAnnotation`: the `@nonVirtual` of `package:meta` on the
/// member (on a synthetic accessor, on the field). The synthetic member from
/// inheritance (covariance) has no annotations.
fn nao_virtual(program: &Program, interner: &Interner, m: &Membro) -> bool {
    if m.sintetico {
        return false;
    }
    let e = program.function(m.funcao);
    let (unit_id, metadata): (UnitId, &[ast::Annotation]) = if e.kind == Fk::ImplicitAccessor {
        let Some(v) = e.variable else { return false };
        match program.variable(v).node {
            VariableRef::Field { unit, member, .. } => (unit, &program.unit(unit).ast.member(member).metadata[..]),
            _ => return false,
        }
    } else {
        let unit_id = match e.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => unit,
            FunctionRef::None => return false,
        };
        (unit_id, crate::fase_resultado::function_annotations(program, m.funcao))
    };
    metadata.iter().any(|a| crate::anotacoes::e_getter_de(program, interner, unit_id, a, "meta", "nonVirtual"))
}

/// `invalid_override_of_non_virtual_member` (`BestPracticesVerifier`,
/// `visitFieldDeclaration` and `visitMethodDeclaration`): a class, enum or
/// mixin member whose `getMember2(…, forSuper: true)` (the field: through the
/// getter, otherwise through the setter) has `@nonVirtual`.
pub fn sobrescritas_de_nao_virtuais(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    let mut h = Heranca::default();
    let mut p = ProvedorDoOutline { program, interner, core, outline, table };
    for (u, did, class) in declaracoes(program, lib) {
        let a = &program.unit(u).ast;
        let d = a.decl(did);
        let Some(c) = class else { continue };
        if !matches!(d.kind, DeclKind::Class(_) | DeclKind::Enum(_) | DeclKind::Mixin(_) | DeclKind::ExtensionType(_)) {
            continue;
        }
        for &mid in membros(d) {
            match &a.member(mid).kind {
                MemberKind::Field(l) => {
                    for var in l.variables.iter() {
                        let n = Nome::novo(interner, lib, var.name.sym);
                        let mut finding = h.membro(&mut p, c, n, false, None, true);
                        if finding.is_none()
                            && let Some(s) = setter_key(interner, var.name.sym)
                        {
                            finding = h.membro(&mut p, c, Nome::novo(interner, lib, s), false, None, true);
                        }
                        if let Some(m) = finding
                            && nao_virtual(program, interner, &m)
                        {
                            let text = interner.resolve(var.name.sym);
                            let owner = interner.resolve(program.class(m.classe).name);
                            output.push((u, Diagnostic::com_codigo(w::INVALID_OVERRIDE_OF_NON_VIRTUAL_MEMBER, var.name.span, [text, owner])));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let func = a.function(*f);
                    let Some(name) = func.name else { continue };
                    let key = match func.kind {
                        FunctionKind::Setter => setter_key(interner, name.sym),
                        _ => Some(name.sym),
                    };
                    let Some(key) = key else { continue };
                    if let Some(m) = h.membro(&mut p, c, Nome::novo(interner, lib, key), false, None, true)
                        && nao_virtual(program, interner, &m)
                    {
                        let text = interner.resolve(name.sym);
                        let owner = interner.resolve(program.class(m.classe).name);
                        output.push((u, Diagnostic::com_codigo(w::INVALID_OVERRIDE_OF_NON_VIRTUAL_MEMBER, name.span, [text, owner])));
                    }
                }
                MemberKind::Constructor(_) => {}
            }
        }
    }
    output
}
