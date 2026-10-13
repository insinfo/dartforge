//! The interface conflicts in `InheritanceOverrideVerifier.verify()`
//! (`analyzer/lib/src/error/inheritance_override.dart:186-217`, `:847-875`)
//! and the `overrideNoCombinedSuperSignature` of override inference
//! (`analyzer/lib/src/task/strong_mode.dart:403-457`), reported in
//! `_reportNoCombinedSuperSignature` (`inheritance_override.dart:953-971`):
//!
//! * `inconsistent_inheritance_getter_and_method` and `inconsistent_inheritance`,
//!   one per conflict, at the declaration name;
//! * `no_combined_super_signature`, at the name of the instance method with an
//!   omitted type whose overridden members (all methods) do not combine.
//!
//! Only in the classes in which `verify()` gets past the checks that end it
//! early (`clausulas::verificador_de_heranca_prossegue`).
//!
//! Written without compiling or running (2026-10-05).

use crate::heranca::{combinar, Conflict, Especie, Heranca, Membro, Nome, ProvedorDoOutline};
use crate::resolve::OutlineTypes;
use crate::table::{CoreTypes, TypeTable};
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{ClassId, FunctionElementId, FunctionRef, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, MemberKind};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

/// The analyzer's `Name.name`: a setter is `x=` (the model's key is `x_=`).
fn member_name(interner: &Interner, n: &Nome) -> String {
    let text = interner.resolve(n.chave);
    text.strip_suffix("_=").map_or_else(|| text.to_string(), |s| format!("{s}="))
}

/// `'Classe.name (ty)'` of each candidate, joined with `, `.
fn displayed_candidates(program: &Program, interner: &Interner, table: &TypeTable, name: &str, candidates: &[Membro]) -> String {
    candidates
        .iter()
        .map(|m| {
            let class = interner.resolve(program.class(m.classe).name);
            let ty = table.format_sem_alias(m.tipo, interner, program);
            format!("{class}.{name} ({ty})")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The name and the members of a class, mixin, enum or alias declaration.
fn declaracao(program: &Program, c: ClassId) -> Option<(UnitId, ast::Name, &[ast::MemberId])> {
    let d = program.class(c).decl?;
    let a = &program.unit(d.unit).ast;
    let decl = a.decl(d.decl);
    if decl.augment {
        return None;
    }
    match &decl.kind {
        DeclKind::Class(x) => Some((d.unit, x.name, &x.members)),
        DeclKind::Mixin(x) => Some((d.unit, x.name, &x.members)),
        DeclKind::Enum(x) => Some((d.unit, x.name, &x.members)),
        _ => None,
    }
}

/// The instance methods (and operators) with an omitted type whose overridden
/// members do not combine: `(class, método, name, explicação)`.
pub fn methods_without_combined_signature(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    classes: &[ClassId],
) -> Vec<(ClassId, FunctionElementId, ast::Name, String)> {
    let mut by_function: HashMap<(UnitId, u32), FunctionElementId> = HashMap::new();
    for (i, f) in program.functions.iter().enumerate() {
        if f.class.is_some()
            && !f.static_
            && let FunctionRef::Function { unit, function } = f.node
        {
            by_function.insert((unit, function.0), FunctionElementId(i as u32));
        }
    }
    let mut inheritance = Heranca::default();
    let mut output = Vec::new();
    for &cid in classes {
        let Some((u, _, members)) = declaracao(program, cid) else { continue };
        let a = &program.unit(u).ast;
        let lib = program.class(cid).library;
        for &m in members {
            let MemberKind::Method(fid) = &a.member(m).kind else { continue };
            let af = a.function(*fid);
            if af.static_ || !matches!(af.kind, ast::FunctionKind::Function | ast::FunctionKind::Operator) {
                continue;
            }
            let (Some(&f), Some(n)) = (by_function.get(&(u, fid.0)), af.name) else { continue };
            let implicit = af.return_type.is_none()
                || af.parameters.as_deref().is_some_and(|ps| ps.iter().any(|p| p.ty.is_none() && p.function_parameters.is_none()));
            if !implicit {
                continue;
            }
            let name = Nome::novo(interner, lib, program.function(f).name);
            let mut p = ProvedorDoOutline { program, interner, core, outline, table: &mut *table };
            let Some(overridden) = inheritance.sobrescritos(&mut p, cid, name) else { continue };
            if overridden.is_empty() || overridden.iter().any(|s| s.especie != Especie::Metodo) {
                continue;
            }
            let mut conflicts: Vec<Conflict> = Vec::new();
            if combinar(&mut p, cid, &overridden, true, name, Some(&mut conflicts)).is_some() {
                continue;
            }
            let name_text = member_name(interner, &name);
            let explanation = match conflicts.as_slice() {
                [Conflict::Candidates { candidatos: candidates, .. }] => displayed_candidates(program, interner, table, &name_text, candidates),
                _ => "<unknown>".to_string(),
            };
            output.push((cid, f, n, explanation));
        }
    }
    output
}

pub fn inconsistencias(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    classes: &[ClassId],
) -> Vec<(UnitId, Diagnostic)> {
    let mut inheritance = Heranca::default();
    let mut output = Vec::new();
    for &cid in classes {
        let Some((u, class_name, _)) = declaracao(program, cid) else { continue };
        let interface = {
            let mut p = ProvedorDoOutline { program, interner, core, outline, table: &mut *table };
            inheritance.interface(&mut p, cid)
        };
        // The same conflict may come from the `on` constraints and from the
        // interfaces of a mixin: one report per text.
        let mut seen: HashSet<(bool, String, String)> = HashSet::new();
        for conflict in interface.conflicts.iter() {
            match conflict {
                Conflict::GetterMethod { nome: name, getter, metodo: method } => {
                    let n = member_name(interner, name);
                    let g = interner.resolve(program.class(getter.classe).name).to_string();
                    let m = interner.resolve(program.class(method.classe).name).to_string();
                    if seen.insert((true, n.clone(), format!("{g}\u{0}{m}"))) {
                        output.push((u, Diagnostic::com_codigo(c::INCONSISTENT_INHERITANCE_GETTER_AND_METHOD, class_name.span, [n.as_str(), g.as_str(), m.as_str()])));
                    }
                }
                Conflict::Candidates { nome: name, candidatos: candidates } => {
                    let n = member_name(interner, name);
                    let list = displayed_candidates(program, interner, table, &n, candidates);
                    if seen.insert((false, n.clone(), list.clone())) {
                        output.push((u, Diagnostic::com_codigo(c::INCONSISTENT_INHERITANCE, class_name.span, [n.as_str(), list.as_str()])));
                    }
                }
                Conflict::ExtensionAndNonExtension { .. } | Conflict::NonUniqueExtension { .. } => {}
            }
        }
    }
    // `_reportNoCombinedSuperSignature`, in the member loop (after the
    // conflicts of all classes: different offsets).
    for (cid, _, n, explanation) in methods_without_combined_signature(program, interner, table, core, outline, classes) {
        let Some((u, _, _)) = declaracao(program, cid) else { continue };
        let class = interner.resolve(program.class(cid).name).to_string();
        output.push((u, Diagnostic::com_codigo(c::NO_COMBINED_SUPER_SIGNATURE, n.span, [class.as_str(), explanation.as_str()])));
    }
    output
}
