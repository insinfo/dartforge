//! Variance positions of the type parameters of classes, mixins, enums and
//! extension types, like the `ErrorVerifier` of analyzer 6.11.0
//! (`src/generated/error_verifier.dart`):
//!
//! * `_checkForWrongTypeParameterVarianceInField` and
//!   `_checkForWrongTypeParameterVarianceInMethod`:
//!   `WRONG_TYPE_PARAMETER_VARIANCE_POSITION` for a type parameter with a
//!   written variance (`in`/`out`/`inout`) used in an incompatible variance
//!   position in the type of a field, a parameter, the return or the bound
//!   of a method type parameter;
//! * `_checkForWrongTypeParameterVarianceInSuperinterfaces`:
//!   `WRONG_EXPLICIT_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE` (with a
//!   written variance) and `WRONG_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE`
//!   (without one, the "legacy covariant") in the direct supertypes.
//!
//! The variance of a parameter in a type is that of
//! `TypeParameterElementImpl.computeVarianceInType`, with the `Variance`
//! lattice of `_fe_analyzer_shared` (`combine`, `meet`,
//! `greaterThanOrEqual`). The analyzer stores the written variance in the
//! element even with the `variance` experiment off; here it comes from the
//! tree (`TypeParameter.variance`), recorded in the table by `resolve`.

use crate::despejo::formatar;
use crate::resolve::OutlineTypes;
use crate::table::{Type, TypeId, TypeParamId, TypeTable, Variance as Declared};
use dartforge_diagnostics::{Diagnostic, Span, codigos::compile_time_error as c};
use dartforge_elements::model::{FunctionRef, LibraryId, Program, UnitId, VariableRef};
use dartforge_frontend::ast::{self, DeclKind, MemberKind};
use dartforge_intern::Interner;
use std::collections::HashMap;

/// `_fe_analyzer_shared`'s `Variance` (the order is the encoding: `meet` is
/// the `|` of the indices).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum V {
    Unrelated = 0,
    Covariant = 1,
    Contravariant = 2,
    Invariant = 3,
}

impl V {
    fn de(bits: u8) -> V {
        match bits {
            0 => V::Unrelated,
            1 => V::Covariant,
            2 => V::Contravariant,
            _ => V::Invariant,
        }
    }

    fn combine(self, o: V) -> V {
        if self == V::Unrelated || o == V::Unrelated {
            return V::Unrelated;
        }
        if self == V::Invariant || o == V::Invariant {
            return V::Invariant;
        }
        if self == o {
            V::Covariant
        } else {
            V::Contravariant
        }
    }

    fn meet(self, o: V) -> V {
        V::de(self as u8 | o as u8)
    }

    fn greater_or_equal(self, o: V) -> bool {
        match self {
            V::Unrelated => true,
            V::Covariant => matches!(o, V::Covariant | V::Invariant),
            V::Contravariant => matches!(o, V::Contravariant | V::Invariant),
            V::Invariant => o == V::Invariant,
        }
    }

    fn palavra(self) -> &'static str {
        match self {
            V::Unrelated => "",
            V::Covariant => "out",
            V::Contravariant => "in",
            V::Invariant => "inout",
        }
    }

    /// The variance of a type parameter (without a modifier: covariant).
    fn of_parameter(d: Declared) -> V {
        match d {
            Declared::Unspecified | Declared::Covariant => V::Covariant,
            Declared::Contravariant => V::Contravariant,
            Declared::Invariant => V::Invariant,
        }
    }
}

/// `computeVarianceInType`: the variance of `x` in `t`.
fn variance_in(table: &TypeTable, outline: &OutlineTypes, x: TypeParamId, t: TypeId) -> V {
    match table.get(t) {
        Type::TypeParameter { param, .. } | Type::Intersection { param, .. } => {
            if *param == x {
                V::Covariant
            } else {
                V::Unrelated
            }
        }
        Type::Interface { class, args, .. }
        | Type::ExtensionType {
            decl: class, args, ..
        } => {
            let params = outline
                .classes
                .get(class.0 as usize)
                .map(|d| &d.type_params[..])
                .unwrap_or(&[]);
            let mut r = V::Unrelated;
            for (i, &a) in args.iter().enumerate() {
                let pv = params
                    .get(i)
                    .map_or(V::Covariant, |&p| V::of_parameter(table.param(p).variance));
                r = r.meet(pv.combine(variance_in(table, outline, x, a)));
            }
            r
        }
        // `FutureOr<T>` is an interface with a covariant parameter in the analyzer.
        Type::FutureOr { arg, .. } => {
            V::Unrelated.meet(V::Covariant.combine(variance_in(table, outline, x, *arg)))
        }
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            ..
        } => {
            let mut r = variance_in(table, outline, x, *ret);
            for &tp in type_params.iter() {
                let d = table.param(tp);
                if variance_in(table, outline, x, d.bound) != V::Unrelated {
                    r = V::Invariant;
                }
            }
            for &p in positional
                .iter()
                .chain(optional.iter())
                .chain(named.iter().map(|(_, t, _)| t))
            {
                r = r.meet(V::Contravariant.combine(variance_in(table, outline, x, p)));
            }
            r
        }
        _ => V::Unrelated,
    }
}

/// The variance diagnostics of the declarations of `lib`.
/// Type parameters without a modifier only take part in the supertype check
/// (`WRONG_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE`).
pub fn variancia(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    let units = &program.library(lib).units;
    let mut functions: HashMap<(UnitId, u32), usize> = HashMap::new();
    for (i, e) in program.functions.iter().enumerate() {
        if let FunctionRef::Function { unit, function } = e.node
            && units.contains(&unit)
        {
            functions.insert((unit, function.0), i);
        }
    }
    let mut fields: HashMap<(UnitId, u32), usize> = HashMap::new();
    for (i, v) in program.variables.iter().enumerate() {
        if let VariableRef::Field {
            unit,
            member,
            index: 0,
        } = v.node
            && units.contains(&unit)
        {
            fields.insert((unit, member.0), i);
        }
    }
    for (i, cls) in program.classes.iter().enumerate() {
        if cls.library != lib {
            continue;
        }
        let Some(decl) = cls.decl else { continue };
        let Some(data) = outline.classes.get(i) else {
            continue;
        };
        let ast_ = &program.unit(decl.unit).ast;
        let (tps, members): (&[ast::TypeParameter], &[ast::MemberId]) =
            match &ast_.decl(decl.decl).kind {
                DeclKind::Class(d) => (&d.type_params, &d.members),
                DeclKind::Mixin(d) => (&d.type_params, &d.members),
                DeclKind::Enum(d) => (&d.type_params, &d.members),
                DeclKind::ExtensionType(d) => (&d.type_params, &d.members),
                _ => continue,
            };
        if tps.len() != data.type_params.len() {
            continue;
        }
        let mut diags = Vec::new();
        let report = |span: Span, x: TypeParamId, v: V, diags: &mut Vec<Diagnostic>| {
            let d = V::of_parameter(table.param(x).variance);
            if !v.greater_or_equal(d) {
                let name = interner.resolve(table.param(x).name);
                diags.push(Diagnostic::com_codigo(
                    c::WRONG_TYPE_PARAMETER_VARIANCE_POSITION,
                    span,
                    [d.palavra(), name, v.palavra()],
                ));
            }
        };
        let explicit: Vec<TypeParamId> = data
            .type_params
            .iter()
            .copied()
            .filter(|&p| table.param(p).variance != Declared::Unspecified)
            .collect();
        // Members, in source order.
        if !explicit.is_empty() {
            for &m in members {
                match &ast_.member(m).kind {
                    MemberKind::Field(vl) => {
                        let Some(first) = vl.variables.first() else {
                            continue;
                        };
                        let Some(ty) = fields
                            .get(&(decl.unit, m.0))
                            .and_then(|&v| outline.variables.get(v))
                            .and_then(|d| d.declared_type.or(d.inferred))
                        else {
                            continue;
                        };
                        for &x in &explicit {
                            let v = variance_in(table, outline, x, ty);
                            report(first.name.span, x, v, &mut diags);
                            if !vl.final_ && !vl.covariant {
                                report(
                                    first.name.span,
                                    x,
                                    V::Contravariant.combine(v),
                                    &mut diags,
                                );
                            }
                        }
                    }
                    MemberKind::Method(f) => {
                        let Some(&fid) = functions.get(&(decl.unit, f.0)) else {
                            continue;
                        };
                        let Some(data_f) = outline.functions.get(fid) else {
                            continue;
                        };
                        let af = ast_.function(*f);
                        for &x in &explicit {
                            for (tp, &p) in af.type_params.iter().zip(data_f.type_params.iter()) {
                                if tp.bound.is_none() {
                                    continue;
                                }
                                let v = V::Invariant.combine(variance_in(
                                    table,
                                    outline,
                                    x,
                                    table.param(p).bound,
                                ));
                                report(tp.span, x, v, &mut diags);
                            }
                            if let Some(ps) = &af.parameters {
                                for (p, dp) in ps.iter().zip(data_f.parameters.iter()) {
                                    if p.covariant {
                                        continue;
                                    }
                                    let v = V::Contravariant
                                        .combine(variance_in(table, outline, x, dp.ty));
                                    report(p.span, x, v, &mut diags);
                                }
                            }
                            if let Some(rt) = af.return_type {
                                let v = variance_in(table, outline, x, data_f.return_type);
                                report(ast_.ty(rt).span, x, v, &mut diags);
                            }
                        }
                    }
                    MemberKind::Constructor(_) => {}
                }
            }
        }
        // Direct supertypes: superclass, interfaces, mixins and `on` constraints.
        let supers: Vec<TypeId> = data
            .supertype
            .iter()
            .chain(data.interfaces.iter())
            .chain(data.mixins.iter())
            .chain(data.on.iter())
            .copied()
            .collect();
        // The displayed supertype is the element's (`supertype`, `interfaces`,
        // `mixins`, `superclassConstraints`), which keeps the alias written in the
        // arguments (`Covariant<CovFunction<T>>`): the clause annotation with
        // the same structure.
        let written_ones: Vec<TypeId> = {
            let (e, w, i, o): (Option<ast::TypeId>, &[ast::TypeId], &[ast::TypeId], &[ast::TypeId]) =
                match &ast_.decl(decl.decl).kind {
                    DeclKind::Class(d) => (d.extends, &d.with, &d.implements, &[]),
                    DeclKind::Mixin(d) => (None, &[], &d.implements, &d.on),
                    DeclKind::Enum(d) => (None, &d.with, &d.implements, &[]),
                    DeclKind::ExtensionType(d) => (None, &[], &d.implements, &[]),
                    _ => (None, &[], &[], &[]),
                };
            e.iter()
                .chain(w.iter())
                .chain(i.iter())
                .chain(o.iter())
                .filter_map(|x| outline.tipos_escritos.get(&(decl.unit, *x)).copied())
                .collect()
        };
        for &s in &supers {
            for (tp, &x) in tps.iter().zip(data.type_params.iter()) {
                let v = variance_in(table, outline, x, s);
                let d = V::of_parameter(table.param(x).variance);
                if v.greater_or_equal(d) {
                    continue;
                }
                let name = interner.resolve(tp.name.sym);
                let structure = table.format_sem_alias(s, interner, program);
                let displayed = match written_ones
                    .iter()
                    .find(|&&w| table.format_sem_alias(w, interner, program) == structure)
                {
                    Some(&w) => table.format(w, interner, program),
                    None => formatar(table, s, interner, program),
                };
                if table.param(x).variance == Declared::Unspecified {
                    diags.push(Diagnostic::com_codigo(
                        c::WRONG_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE,
                        tp.name.span,
                        [name, displayed.as_str()],
                    ));
                } else {
                    diags.push(Diagnostic::com_codigo(
                        c::WRONG_EXPLICIT_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE,
                        tp.name.span,
                        [name, d.palavra(), v.palavra(), displayed.as_str()],
                    ));
                }
            }
        }
        output.extend(diags.into_iter().map(|d| (decl.unit, d)));
    }
    output
}

/// The variance of `NonCovariantTypeParameterPositionVisitor`
/// (`analyzer/lib/src/dart/element/non_covariant_type_parameter_position.dart`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Posicao {
    CovariantPos,
    ContravariantPos,
    InvariantPos,
}

impl Posicao {
    /// `Variance.combine`.
    fn the_with(self, other: Posicao) -> Posicao {
        if self == Posicao::InvariantPos || other == Posicao::InvariantPos {
            Posicao::InvariantPos
        } else if self == other {
            Posicao::CovariantPos
        } else {
            Posicao::ContravariantPos
        }
    }
}

/// The type parameter `x` appears in `t` in a non-covariant position.
fn in_non_covariant_position(table: &TypeTable, x: TypeParamId, t: TypeId, v: Posicao, prof: u32) -> bool {
    if prof > 64 {
        return false;
    }
    match table.get(t) {
        Type::TypeParameter { param, .. } => v != Posicao::CovariantPos && *param == x,
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.iter().any(|a| in_non_covariant_position(table, x, *a, v, prof + 1)),
        Type::FutureOr { arg, .. } => in_non_covariant_position(table, x, *arg, v, prof + 1),
        Type::Record { positional, named, .. } => {
            positional.iter().any(|a| in_non_covariant_position(table, x, *a, v, prof + 1))
                || named.iter().any(|(_, a)| in_non_covariant_position(table, x, *a, v, prof + 1))
        }
        Type::Function { type_params, ret, positional, optional, named, .. } => {
            if in_non_covariant_position(table, x, *ret, v, prof + 1) {
                return true;
            }
            // The written bounds of the function's type parameters: invariant.
            for tp in type_params.iter() {
                let d = table.param(*tp);
                if d.explicito && in_non_covariant_position(table, x, d.bound, Posicao::InvariantPos, prof + 1) {
                    return true;
                }
            }
            let against = v.the_with(Posicao::ContravariantPos);
            positional.iter().chain(optional.iter()).any(|a| in_non_covariant_position(table, x, *a, against, prof + 1))
                || named.iter().any(|(_, a, _)| in_non_covariant_position(table, x, *a, against, prof + 1))
        }
        _ => false,
    }
}

/// `ErrorVerifier._checkForNonCovariantTypeParameterPositionInRepresentationType`
/// (`analyzer/lib/src/generated/error_verifier.dart:4752-4780`): each type
/// parameter of an extension type that appears in a non-covariant position of
/// the representation type, at the type parameter's node.
pub fn posicoes_nao_covariantes_na_representacao(program: &Program, table: &TypeTable, outline: &OutlineTypes, lib: LibraryId) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    for (i, k) in program.classes.iter().enumerate() {
        if k.library != lib || k.kind != dartforge_elements::model::ClassKind::ExtensionType {
            continue;
        }
        let (Some(d), Some(rep)) = (k.decl, k.representation) else { continue };
        let Some(t) = outline.variables.get(rep.0 as usize).and_then(|x| x.declared_type) else { continue };
        let Some(formals) = outline.classes.get(i).map(|x| x.type_params.clone()) else { continue };
        let a = &program.unit(d.unit).ast;
        let DeclKind::ExtensionType(x) = &a.decl(d.decl).kind else { continue };
        for (tp, &pid) in x.type_params.iter().zip(formals.iter()) {
            if in_non_covariant_position(table, pid, t, Posicao::CovariantPos, 0) {
                let start = tp.metadata.first().map_or(tp.span.start, |m| m.span.start.min(tp.span.start));
                let sp = Span { start: start, end: tp.span.end };
                output.push((d.unit, Diagnostic::com_codigo(c::NON_COVARIANT_TYPE_PARAMETER_POSITION_IN_REPRESENTATION_TYPE, sp, std::iter::empty::<&str>())));
            }
        }
    }
    output
}
