//! Invalid overrides (`invalid_override`), like the
//! `_ClassVerifier._checkDeclaredMember` of `InheritanceOverrideVerifier`
//! (analyzer 3.6.2, `src/error/inheritance_override.dart`) and
//! `CorrectOverrideHelper` (`src/error/correct_override.dart`): each instance
//! member declared in the class is compared, by subtyping of the function
//! type, with the same-named member of the interface of each direct
//! superinterface (superclass, `on` constraints, mixins and interfaces, in
//! the analyzer's order). Covariant parameters count as `Object?`.
//!
//! The interface queries (`getMember`, `getInherited`, `implemented`) are
//! those of the `InheritanceManager3` in [`crate::inheritance`], with the
//! combined signature and `topMerge`; the covariance of each parameter is the
//! written one plus the inherited one; the omitted types are those of the
//! faithful override inference (`inference::overrides`), which runs before
//! these passes.
//!
//! A class is only checked if the analyzer's `verify()` would get this far
//! (the caller filters: a forbidden supertype, `Enum` in a concrete class
//! and recursive inheritance stop earlier).

use crate::dump::display_string;
use crate::ops::substitute;
use crate::resolve::OutlineTypes;
use crate::subtyping::{SubtypeEnv, is_subtype};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::{Diagnostic, Span, codes::compile_time_error as c};
use dartforge_elements::model::{
    ClassId, FunctionElementId, FunctionKind, FunctionRef, Program, UnitId, VariableRef,
};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

/// The kind of a member for `member.kind != superMember.kind`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Method,
    Getter,
    Setter,
}

struct Ctx<'a> {
    program: &'a Program,
    interner: &'a Interner,
    table: &'a mut TypeTable,
    core: &'a CoreTypes,
    outline: &'a OutlineTypes,
    /// O `InheritanceManager3`.
    inheritance: crate::inheritance::InheritanceManager,
    /// The library of the verified class (the `Name` of private members).
    library: dartforge_elements::model::LibraryId,
}

/// A same-named member found in the interface of a supertype.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Finding {
    owner: ClassId,
    function: FunctionElementId,
}

impl Ctx<'_> {
    fn kind(&self, f: FunctionElementId) -> Option<Kind> {
        let func = self.program.function(f);
        match func.kind {
            FunctionKind::Function | FunctionKind::Operator => Some(Kind::Method),
            FunctionKind::Getter => Some(Kind::Getter),
            FunctionKind::Setter => Some(Kind::Setter),
            FunctionKind::ImplicitAccessor => {
                let v = self.program.variable(func.variable?);
                if v.getter == Some(f) {
                    Some(Kind::Getter)
                } else {
                    Some(Kind::Setter)
                }
            }
            _ => None,
        }
    }

    /// The member's key in `instance_members` (`x_=` for a setter).
    fn key(&self, f: FunctionElementId) -> Option<SymbolId> {
        let func = self.program.function(f);
        if self.kind(f)? == Kind::Setter {
            self.interner
                .lookup(&format!("{}_=", self.interner.resolve(func.name)))
        } else {
            Some(func.name)
        }
    }

    /// The member `key` in the interface of class `d` (`getMember2`) or
    /// inherited by it (`getInherited2`), by the `Name` of the verified library.
    fn inheritance_member(
        &mut self,
        d: ClassId,
        key: SymbolId,
        inherited: bool,
    ) -> Option<crate::inheritance::InterfaceMember> {
        let the_name = crate::inheritance::MemberName::new(self.interner, self.library, key);
        let mut h = std::mem::take(&mut self.inheritance);
        let r = {
            let mut p = crate::inheritance::OutlineProvider {
                program: self.program,
                interner: self.interner,
                core: self.core,
                outline: self.outline,
                table: &mut *self.table,
            };
            if inherited {
                h.inherited(&mut p, d, the_name)
            } else {
                h.member(&mut p, d, the_name, false, None, false)
            }
        };
        self.inheritance = h;
        r
    }

    /// `InheritanceManager3.getMember(s, the_name)`: the member in the interface of
    /// interface type `s` (the combined signature, with `topMerge`), with the
    /// type as seen by `s`.
    fn in_interface(&mut self, s: TypeId, key: SymbolId, _prof: u32) -> Option<(Finding, TypeId)> {
        self.in_interface_ex(s, key, false)
    }

    /// `InheritanceManager3.getInherited(s, the_name)`.
    fn inherited(&mut self, s: TypeId, key: SymbolId) -> Option<(Finding, TypeId)> {
        self.in_interface_ex(s, key, true)
    }

    fn in_interface_ex(
        &mut self,
        s: TypeId,
        key: SymbolId,
        inherited: bool,
    ) -> Option<(Finding, TypeId)> {
        let (d, args) = match self.table.get(s).clone() {
            Type::Interface { class, args, .. } => (class, args),
            _ => return None,
        };
        let m = self.inheritance_member(d, key, inherited)?;
        let formals: Box<[TypeParamId]> =
            self.outline.classes.get(d.0 as usize)?.type_params.clone();
        let t = if !formals.is_empty() && formals.len() == args.len() {
            let subst: HashMap<TypeParamId, TypeId> =
                formals.iter().copied().zip(args.iter().copied()).collect();
            substitute(m.ty, &subst, self.table)
        } else {
            m.ty
        };
        Some((
            Finding {
                owner: m.class,
                function: m.function,
            },
            t,
        ))
    }

    /// The type of member `a` as seen by supertype `s` (which inherits or
    /// declares it).
    fn seen_type(&mut self, s: TypeId, a: Finding) -> Option<TypeId> {
        let sig = self.outline.functions.get(a.function.0 as usize)?.signature;
        let seen = self
            .outline
            .hierarchy
            .supertype_of(s, a.owner, self.table, self.core)?;
        let args = match self.table.get(seen) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.clone(),
            _ => return None,
        };
        let params: Box<[TypeParamId]> = self
            .outline
            .classes
            .get(a.owner.0 as usize)?
            .type_params
            .clone();
        if params.len() != args.len() {
            return None;
        }
        let subst: HashMap<TypeParamId, TypeId> =
            params.iter().copied().zip(args.iter().copied()).collect();
        Some(substitute(sig, &subst, self.table))
    }

    /// `ParameterElement.isCovariant` of member `f` as seen by class `c`: the one
    /// declared in it (written plus inherited from the overridden members) or the
    /// implemented one (with inherited covariance).
    fn effective_covariant(
        &mut self,
        c: ClassId,
        f: FunctionElementId,
        key: SymbolId,
    ) -> (Vec<usize>, Vec<SymbolId>) {
        let the_name =
            crate::inheritance::MemberName::new(self.interner, self.program.class(c).library, key);
        let mut h = std::mem::take(&mut self.inheritance);
        let i = {
            let mut p = crate::inheritance::OutlineProvider {
                program: self.program,
                interner: self.interner,
                core: self.core,
                outline: self.outline,
                table: &mut *self.table,
            };
            h.interface(&mut p, c)
        };
        self.inheritance = h;
        let m = i
            .declared
            .get(&the_name)
            .filter(|m| m.function == f)
            .or_else(|| i.implemented.get(&the_name).filter(|m| m.function == f))
            .or_else(|| i.declared.get(&the_name))
            .or_else(|| i.implemented.get(&the_name));
        let mut pos = Vec::new();
        let mut names = Vec::new();
        for d in m.map(|m| &m.the_covariant[..]).unwrap_or(&[]) {
            match d {
                crate::inheritance::Param::Index(k) => pos.push(*k),
                crate::inheritance::Param::MemberName(n) => names.push(*n),
            }
        }
        (pos, names)
    }

    /// `CovariantParametersVerifier`: each covariant parameter against the
    /// corresponding one of the same-named member declared in each supertype of
    /// the class; types with no subtype relation in either direction are an
    /// error. The message shows the supertype's member without substitution.
    fn covariant_against_supertypes(
        &mut self,
        m: &Check,
        key: SymbolId,
        output: &mut Vec<Diagnostic>,
    ) {
        let (pos, names) = self.effective_covariant(m.declaring, m.function, key);
        if pos.is_empty() && names.is_empty() {
            return;
        }
        let Some(own) = self.outline.functions.get(m.function.0 as usize) else {
            return;
        };
        let the_own: Vec<(usize, ParameterKind, Option<SymbolId>, TypeId)> = own
            .parameters
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.kind, p.external_name, p.ty))
            .collect();
        let Some(data) = self.outline.hierarchy.get(m.declaring) else {
            return;
        };
        let mut supers: Vec<(ClassId, TypeId)> = data
            .supertypes
            .iter()
            .filter(|(k, _)| **k != m.declaring)
            .map(|(k, v)| (*k, *v))
            .collect();
        supers.sort_by_key(|(k, _)| k.0);
        let the_kind = self.kind(m.function);
        for (s, st) in supers {
            let Some(&g) = self.program.class(s).instance_members.get(&key) else {
                continue;
            };
            if self.kind(g) != the_kind {
                continue;
            }
            let Some(data_g) = self.outline.functions.get(g.0 as usize) else {
                continue;
            };
            let args = match self.table.get(st) {
                Type::Interface { args, .. } => args.clone(),
                _ => continue,
            };
            let Some(params_s) = self
                .outline
                .classes
                .get(s.0 as usize)
                .map(|d| d.type_params.clone())
            else {
                continue;
            };
            if params_s.len() != args.len() {
                continue;
            }
            let mut subst: HashMap<TypeParamId, TypeId> =
                params_s.iter().copied().zip(args.iter().copied()).collect();
            // `_superSubstitution` (3.6.2 `correct_override.dart:195-215`): with
            // the same count, the type parameters of the supertype's method become
            // those of the method itself.
            if !own.type_params.is_empty() && own.type_params.len() == data_g.type_params.len() {
                for (&sp, &tp) in data_g.type_params.iter().zip(own.type_params.iter()) {
                    let t = self.table.intern(Type::TypeParameter {
                        param: tp,
                        nullable: false,
                    });
                    subst.insert(sp, t);
                }
            }
            let super_params: Vec<(ParameterKind, Option<SymbolId>, TypeId)> = data_g
                .parameters
                .iter()
                .map(|p| (p.kind, p.external_name, p.ty))
                .collect();
            let super_sig = data_g.signature;
            let mut positional_one = 0usize;
            for &(i, kind, the_name, ty) in &the_own {
                let covariant = if kind == ParameterKind::Named {
                    the_name.is_some_and(|n| names.contains(&n))
                } else {
                    pos.contains(&positional_one)
                };
                let pos_index = positional_one;
                if kind != ParameterKind::Named {
                    positional_one += 1;
                }
                let _ = i;
                if !covariant {
                    continue;
                }
                let corresponding = if kind == ParameterKind::Named {
                    super_params
                        .iter()
                        .find(|p| p.0 == ParameterKind::Named && p.1 == the_name)
                        .map(|p| p.2)
                } else {
                    super_params
                        .iter()
                        .filter(|p| p.0 != ParameterKind::Named)
                        .nth(pos_index)
                        .map(|p| p.2)
                };
                let Some(sup_ty) = corresponding else {
                    continue;
                };
                let sup_ty = substitute(sup_ty, &subst, self.table);
                let related = {
                    let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                    is_subtype(sup_ty, ty, &mut env) || is_subtype(ty, sup_ty, &mut env)
                };
                if !related {
                    let mut name_m = self
                        .interner
                        .resolve(self.program.function(m.function).name)
                        .to_string();
                    if the_kind == Some(Kind::Setter) {
                        name_m.push('=');
                    }
                    let args = [
                        name_m,
                        self.interner
                            .resolve(self.program.class(m.declaring).name)
                            .to_string(),
                        display_string(self.table, m.ty, self.interner, self.program),
                        self.interner
                            .resolve(self.program.class(s).name)
                            .to_string(),
                        display_string(self.table, super_sig, self.interner, self.program),
                    ];
                    output.push(Diagnostic::with_code(c::INVALID_OVERRIDE, m.span, args));
                }
            }
        }
    }

    /// `_computeThisTypeForSubtype`: covariant parameters become `Object?`.
    fn for_subtype(&mut self, sig: TypeId, pos: &[usize], names: &[SymbolId]) -> TypeId {
        if pos.is_empty() && names.is_empty() {
            return sig;
        }
        let Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable,
        } = self.table.get(sig).clone()
        else {
            return sig;
        };
        let obj = self.core.object_nullable;
        let n_pos = positional.len();
        let positional: Box<[TypeId]> = positional
            .iter()
            .enumerate()
            .map(|(i, &t)| if pos.contains(&i) { obj } else { t })
            .collect();
        let optional: Box<[TypeId]> = optional
            .iter()
            .enumerate()
            .map(|(i, &t)| if pos.contains(&(i + n_pos)) { obj } else { t })
            .collect();
        let named: Box<[(SymbolId, TypeId, bool)]> = named
            .iter()
            .map(|&(n, t, r)| (n, if names.contains(&n) { obj } else { t }, r))
            .collect();
        self.table.intern(Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable,
        })
    }
}

/// A member to check: the function, its type in the class that receives it,
/// the class that declares it (name in the message, covariance, inference)
/// and where to report.
#[derive(Clone, Copy)]
struct Check {
    function: FunctionElementId,
    ty: TypeId,
    declaring: ClassId,
    span: Span,
}

impl Ctx<'_> {
    /// `_checkDeclaredMember` against the superinterfaces `supers`.
    fn check(&mut self, m: Check, supers: &[TypeId], output: &mut Vec<Diagnostic>) {
        let (Some(the_kind), Some(key)) = (self.kind(m.function), self.key(m.function)) else {
            return;
        };
        let the_name = self
            .interner
            .resolve(self.program.function(m.function).name)
            .to_string();
        let library = self.program.class(m.declaring).library;
        let (pos, names) = self.effective_covariant(m.declaring, m.function, key);
        let to_sub = self.for_subtype(m.ty, &pos, &names);
        for &s in supers {
            let Some((finding, from_super)) = self.in_interface(s, key, 0) else {
                continue;
            };
            if the_name.starts_with('_') && self.program.class(finding.owner).library != library {
                continue;
            }
            if self.kind(finding.function) != Some(the_kind) {
                continue;
            }
            let ok = {
                let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                is_subtype(to_sub, from_super, &mut env)
            };
            if !ok {
                let code = if the_kind == Kind::Setter {
                    c::INVALID_OVERRIDE_SETTER
                } else {
                    c::INVALID_OVERRIDE
                };
                let args = [
                    the_name.clone(),
                    self.interner
                        .resolve(self.program.class(m.declaring).name)
                        .to_string(),
                    display_string(self.table, m.ty, self.interner, self.program),
                    self.interner
                        .resolve(self.program.class(finding.owner).name)
                        .to_string(),
                    display_string(self.table, from_super, self.interner, self.program),
                ];
                // `invalidOverride`: the overridden member, at its non-synthetic
                // name (in its own file, if it is a different one).
                let mut d = Diagnostic::with_code(code, m.span, args);
                if let Some((u, span)) = self.program.non_synthetic_function_name(finding.function)
                {
                    let error_unit = self
                        .program
                        .non_synthetic_function_name(m.function)
                        .map(|(x, _)| x);
                    let file = (Some(u) != error_unit).then(|| self.program.unit_path(u).into());
                    let text = if the_kind == Kind::Setter {
                        "The setter being overridden."
                    } else {
                        "The member being overridden."
                    };
                    d.context.push(dartforge_diagnostics::ContextMessage {
                        file,
                        span,
                        message: text.into(),
                    });
                }
                output.push(d);
            }
        }
    }
}

/// The `invalid_override` diagnostics of the members declared in the classes
/// `classes` and of the mixins they apply (each with the unit of the class
/// declaration).
pub fn invalid_overrides(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    classes: &[ClassId],
) -> Vec<(UnitId, Diagnostic)> {
    // `_reportNoCombinedSuperSignature` reported: the method skips
    // `_checkDeclaredMember` (`inheritance_override.dart:259-263`).
    let without_signature: std::collections::HashSet<FunctionElementId> =
        crate::phase_inheritance::methods_without_combined_signature(
            program, interner, table, core, outline, classes,
        )
        .into_iter()
        .map(|(_, f, _, _)| f)
        .collect();
    // The classes whose declared members are read: the checked ones and the
    // mixins they apply (the engine calls this once per library, so the maps
    // below cover only these, not every class of the program).
    let mut needed = vec![false; program.classes.len()];
    for &cid in classes {
        needed[cid.0 as usize] = true;
        if let Some(data) = outline.classes.get(cid.0 as usize) {
            for &mt in &data.mixins {
                if let Type::Interface { class: m, .. } = table.get(mt) {
                    needed[m.0 as usize] = true;
                }
            }
        }
    }
    let mut cx = Ctx {
        program,
        interner,
        table,
        core,
        outline,
        inheritance: crate::inheritance::InheritanceManager::default(),
        library: dartforge_elements::model::LibraryId(0),
    };
    let mut output = Vec::new();
    // Accessors and methods by source node.
    let mut by_function: HashMap<(UnitId, u32), FunctionElementId> = HashMap::new();
    type Accessors = (Option<FunctionElementId>, Option<FunctionElementId>);
    let mut by_variable: HashMap<(UnitId, u32, usize), Accessors> = HashMap::new();
    for (i, f) in program.functions.iter().enumerate() {
        if f.static_ || !f.class.is_some_and(|c| needed[c.0 as usize]) {
            continue;
        }
        let id = FunctionElementId(i as u32);
        match f.node {
            FunctionRef::Function { unit, function } => {
                by_function.insert((unit, function.0), id);
            }
            FunctionRef::None => {
                if let Some(v) = f.variable {
                    let var = program.variable(v);
                    if let VariableRef::Field {
                        unit,
                        member,
                        index,
                    } = var.node
                    {
                        by_variable.insert((unit, member.0, index), (var.getter, var.setter));
                    }
                }
            }
            FunctionRef::Constructor { .. } => {}
        }
    }
    // The instance members declared in a class: `(function, the name in the error)`.
    let the_declared = |cid: ClassId| -> Vec<(FunctionElementId, Span)> {
        let mut v = Vec::new();
        let Some(decl) = program.class(cid).decl else {
            return v;
        };
        let ast_ = &program.unit(decl.unit).ast;
        let members: &[ast::MemberId] = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) => &d.members,
            DeclKind::Mixin(d) => &d.members,
            DeclKind::Enum(d) => &d.members,
            _ => return v,
        };
        for &m in members {
            match &ast_.member(m).kind {
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    if let (Some(&id), Some(n)) = (by_function.get(&(decl.unit, f.0)), af.name) {
                        v.push((id, n.span));
                    }
                }
                MemberKind::Field(vl) if !vl.static_ => {
                    for (i, var) in vl.variables.iter().enumerate() {
                        if let Some(&(g, s)) = by_variable.get(&(decl.unit, m.0, i)) {
                            v.extend(g.into_iter().chain(s).map(|f| (f, var.name.span)));
                        }
                    }
                }
                _ => {}
            }
        }
        v
    };
    for &cid in classes {
        let cls = program.class(cid);
        cx.library = cls.library;
        let Some(decl) = cls.decl else { continue };
        let Some(data) = outline.classes.get(cid.0 as usize) else {
            continue;
        };
        let ast_ = &program.unit(decl.unit).ast;
        let with: &[ast::TypeId] = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) => &d.with,
            DeclKind::Enum(d) => &d.with,
            _ => &[],
        };
        // `directSuperInterfaces`: superclass (`Object` when omitted in a class)
        // and `on` constraints; the mixins come in one by one after being
        // checked; finally the interfaces.
        let mut supers: Vec<TypeId> = Vec::new();
        match (data.supertype, cls.kind) {
            (Some(t), _) => supers.push(t),
            (None, dartforge_elements::model::ClassKind::Class) => supers.push(core.object),
            (None, dartforge_elements::model::ClassKind::Enum) => {
                if let (Some(e), Some(this_one)) = (cx.core_class("Enum"), cx.own_type(cid))
                    && let Some(t) = outline.hierarchy.supertype_of(this_one, e, cx.table, core)
                {
                    supers.push(t);
                }
            }
            _ => {}
        }
        supers.extend(data.on.iter().copied());
        let mut diags = Vec::new();
        for (i, &mt) in data.mixins.iter().enumerate() {
            if let Type::Interface { class: m, .. } = cx.table.get(mt).clone()
                && with.len() == data.mixins.len()
            {
                let span = ast_.ty(with[i]).span;
                for (f, _) in the_declared(m) {
                    // The type of the member of the applied mixin (`M<args>`).
                    let Some(typ) = cx.seen_type(
                        mt,
                        Finding {
                            owner: m,
                            function: f,
                        },
                    ) else {
                        continue;
                    };
                    cx.check(
                        Check {
                            function: f,
                            ty: typ,
                            declaring: m,
                            span,
                        },
                        &supers.clone(),
                        &mut diags,
                    );
                }
            }
            supers.push(mt);
        }
        supers.extend(data.interfaces.iter().copied());
        supers.retain(|&t| match cx.table.get(t) {
            Type::Interface { class, .. } => !cx.outside_hierarchy(cid, *class),
            _ => true,
        });
        for (f, span) in the_declared(cid) {
            if without_signature.contains(&f) {
                continue;
            }
            let Some(typ) = outline.functions.get(f.0 as usize).map(|d| d.signature) else {
                continue;
            };
            let conf = Check {
                function: f,
                ty: typ,
                declaring: cid,
                span,
            };
            cx.check(Check { ..conf }, &supers, &mut diags);
            if let Some(key) = cx.key(f) {
                cx.covariant_against_supertypes(&conf, key, &mut diags);
            }
        }
        output.extend(diags.into_iter().map(|d| (decl.unit, d)));
    }
    output
}

/// Where the name of function `f` is (that of the field or variable, for an
/// implicit accessor).
fn name_of(program: &Program, f: FunctionElementId) -> Option<(UnitId, Span)> {
    let func = program.function(f);
    match func.node {
        FunctionRef::Function { unit, function } => Some((
            unit,
            program.unit(unit).ast.functions[function.0 as usize]
                .name?
                .span,
        )),
        FunctionRef::None => variable_name(program, func.variable?),
        FunctionRef::Constructor { .. } => None,
    }
}

/// The declared name of a variable (field, top-level, enum constant,
/// representation).
fn variable_name(
    program: &Program,
    v: dartforge_elements::model::VariableId,
) -> Option<(UnitId, Span)> {
    {
        match program.variable(v).node {
            VariableRef::Field {
                unit,
                member,
                index,
            } => match &program.unit(unit).ast.member(member).kind {
                MemberKind::Field(vl) => Some((unit, vl.variables.get(index)?.name.span)),
                _ => None,
            },
            VariableRef::TopLevel { unit, decl, index } => {
                match &program.unit(unit).ast.decl(decl).kind {
                    DeclKind::Variables(vl) => Some((unit, vl.variables.get(index)?.name.span)),
                    _ => None,
                }
            }
            VariableRef::EnumConstant { unit, decl, index } => {
                match &program.unit(unit).ast.decl(decl).kind {
                    DeclKind::Enum(d) => Some((unit, d.constants.get(index)?.name.span)),
                    _ => None,
                }
            }
            VariableRef::Representation { unit, decl } => {
                match &program.unit(unit).ast.decl(decl).kind {
                    DeclKind::ExtensionType(d) => Some((unit, d.representation_name.span)),
                    _ => None,
                }
            }
            VariableRef::None => None,
        }
    }
}

impl Ctx<'_> {
    /// The getter and setter types of an accessor outside an interface (top
    /// level, static, extension): the written one, `dynamic` when omitted in a
    /// function, nothing for a field or variable without a written type (the
    /// type would come from the initializer).
    fn accessor_type(&mut self, f: FunctionElementId, getter: bool) -> Option<TypeId> {
        let func = self.program.function(f);
        let data = self.outline.functions.get(f.0 as usize)?;
        match func.node {
            FunctionRef::Function { unit, function } => {
                let af = &self.program.unit(unit).ast.functions[function.0 as usize];
                let (written, t) = if getter {
                    (af.return_type, data.return_type)
                } else {
                    (
                        af.parameters
                            .as_deref()
                            .and_then(|p| p.first())
                            .and_then(|p| p.ty),
                        data.parameters.first()?.ty,
                    )
                };
                // A written type that does not resolve (`InvalidType` in the analyzer,
                // which is a subtype of everything and accepts everything): undecided.
                if let Some(e) = written
                    && t == self.core.dynamic_
                    && !matches!(&self.program.unit(unit).ast.ty(e).kind,
                        ast::TypeKind::Named { name, .. } if name.len() == 1 && self.interner.resolve(name[0].sym) == "dynamic")
                {
                    return None;
                }
                Some(t)
            }
            FunctionRef::None => {
                let v = func.variable?;
                let data_v = self.outline.variables.get(v.0 as usize)?;
                match self.program.variable(v).node {
                    // Enum constant and representation: the type is known.
                    VariableRef::EnumConstant { .. } | VariableRef::Representation { .. } => {
                        data_v.declared_type.or(data_v.inferred)
                    }
                    _ => data_v.declared_type,
                }
            }
            FunctionRef::Constructor { .. } => None,
        }
    }
}

/// `GetterSetterTypesVerifier` (`src/error/getter_setter_types_verifier.dart`):
/// the getter's return type must be a subtype of the parameter type of the
/// same-named setter. Top-level accessors (per unit), static ones of classes
/// and enums and extension ones (`checkStaticAccessors`/`checkExtension`),
/// and the interface of the classes `classes` (`checkInterface`, which
/// `InheritanceOverrideVerifier` calls after the overrides).
pub fn getters_and_setters(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
    classes: &[ClassId],
) -> Vec<(UnitId, Diagnostic)> {
    let mut cx = Ctx {
        program,
        interner,
        table,
        core,
        outline,
        inheritance: crate::inheritance::InheritanceManager::default(),
        library: dartforge_elements::model::LibraryId(0),
    };
    let mut output = Vec::new();
    // Local accessors: `(holder, the_name)` → getters and setters.
    #[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
    enum Holder {
        Unit(UnitId),
        Class(ClassId),
        Instance(ClassId),
        Extension(u32),
    }
    // A local accessor: a function (written getter/setter or implicit one of a
    // class field), a variable without an access function in the model (enum
    // constant, representation, extension field) or the enum's synthetic
    // `values`.
    #[derive(PartialEq, Eq, Clone, Copy, Debug)]
    enum Accessor {
        Function(FunctionElementId),
        Variable(dartforge_elements::model::VariableId),
        Values(ClassId),
    }
    let mut groups: HashMap<(Holder, SymbolId), (Vec<Accessor>, Vec<Accessor>)> = HashMap::new();
    // `augment` accessors (and what they augment) are left out: the parser's
    // recovery without the experiment reads them differently.
    let mut augmented: std::collections::HashSet<(UnitId, u32)> = std::collections::HashSet::new();
    for &u in &program.library(lib).units {
        let ast_ = &program.unit(u).ast;
        for m in &ast_.members {
            if m.augment {
                match &m.kind {
                    MemberKind::Method(f) => {
                        augmented.insert((u, f.0));
                    }
                    // A field with `augment`: the whole library is left out.
                    MemberKind::Field(_) => return Vec::new(),
                    MemberKind::Constructor(_) => {}
                }
            }
        }
        for d in &ast_.decls {
            if d.augment
                && let DeclKind::Function(f) = &d.kind
            {
                augmented.insert((u, f.0));
            }
        }
    }
    for (i, f) in program.index().library_functions[lib.0 as usize]
        .iter()
        .map(|&x| (x.0 as usize, program.function(x)))
    {
        if f.library != lib || f.patched_by.is_some() {
            continue;
        }
        if let FunctionRef::Function { unit, function } = f.node
            && augmented.contains(&(unit, function.0))
        {
            continue;
        }
        let id = FunctionElementId(i as u32);
        // The enum's synthetic members: `values` is the static getter (at the
        // enum's name); `index` and `name` come from `Enum` through the interface.
        if matches!(f.node, FunctionRef::None) && f.variable.is_none() {
            if f.static_
                && let Some(c) = f.class
                && program.class(c).kind == dartforge_elements::model::ClassKind::Enum
                && interner.resolve(f.name) == "values"
            {
                groups
                    .entry((Holder::Class(c), f.name))
                    .or_default()
                    .0
                    .push(Accessor::Values(c));
            }
            continue;
        }
        let Some(the_kind) = cx.kind(id) else {
            continue;
        };
        if the_kind == Kind::Method {
            continue;
        }
        let holder = match (f.class, f.extension) {
            (_, Some(e)) => Holder::Extension(e.0),
            (Some(c), None) if f.static_ => match program.class(c).kind {
                dartforge_elements::model::ClassKind::Class
                | dartforge_elements::model::ClassKind::Enum
                | dartforge_elements::model::ClassKind::ExtensionType => Holder::Class(c),
                _ => continue,
            },
            // Instance members of an extension type: those declared in it
            // (`checkExtensionType`, the part of the interface it declares itself),
            // including the representation.
            (Some(c), None)
                if program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType =>
            {
                Holder::Instance(c)
            }
            (Some(_), None) => continue,
            (None, None) => match name_of(program, id) {
                Some((u, _)) => Holder::Unit(u),
                None => continue,
            },
        };
        let g = groups.entry((holder, f.name)).or_default();
        if the_kind == Kind::Getter {
            g.0.push(Accessor::Function(id))
        } else {
            g.1.push(Accessor::Function(id))
        }
    }
    // Variables without an access function: the enum constant (static
    // getter), the representation (instance getter of the extension type) and
    // the extension field (getter, and setter if neither `final` nor `const`).
    for (i, v) in program.index().library_variables[lib.0 as usize]
        .iter()
        .map(|&x| (x.0 as usize, program.variable(x)))
    {
        if v.library != lib || v.getter.is_some() {
            continue;
        }
        let vid = dartforge_elements::model::VariableId(i as u32);
        match (v.node, v.class, v.extension) {
            (VariableRef::EnumConstant { .. }, Some(c), _) => {
                groups
                    .entry((Holder::Class(c), v.name))
                    .or_default()
                    .0
                    .push(Accessor::Variable(vid));
            }
            (VariableRef::Representation { .. }, Some(c), _) => {
                groups
                    .entry((Holder::Instance(c), v.name))
                    .or_default()
                    .0
                    .push(Accessor::Variable(vid));
            }
            (VariableRef::Field { .. }, None, Some(e)) => {
                let g = groups.entry((Holder::Extension(e.0), v.name)).or_default();
                g.0.push(Accessor::Variable(vid));
                if !v.final_ && !v.const_ {
                    g.1.push(Accessor::Variable(vid));
                }
            }
            _ => {}
        }
    }
    let this_type = |table: &mut TypeTable, c: ClassId| -> TypeId {
        let params = outline
            .classes
            .get(c.0 as usize)
            .map(|d| d.type_params.clone())
            .unwrap_or_default();
        let args: Box<[TypeId]> = params
            .iter()
            .map(|&p| {
                table.intern(Type::TypeParameter {
                    param: p,
                    nullable: false,
                })
            })
            .collect();
        table.intern(Type::Interface {
            class: c,
            args,
            nullable: false,
        })
    };
    let mut keys: Vec<_> = groups.keys().copied().collect();
    keys.sort_by_key(|(r, n)| (interner.resolve(*n).to_string(), format!("{r:?}")));
    for k in keys {
        let (gs, ss) = &groups[&k];
        let ([g], [s]) = (gs.as_slice(), ss.as_slice()) else {
            continue;
        };
        let typ = |cx: &mut Ctx<'_>, a: Accessor, getter: bool| -> Option<TypeId> {
            match a {
                Accessor::Function(f) => cx.accessor_type(f, getter),
                Accessor::Variable(v) => outline
                    .variables
                    .get(v.0 as usize)
                    .and_then(|d| d.declared_type.or(d.inferred)),
                Accessor::Values(c) => {
                    let list = core.list_class?;
                    let e = this_type(cx.table, c);
                    Some(cx.table.intern(Type::Interface {
                        class: list,
                        args: Box::new([e]),
                        nullable: false,
                    }))
                }
            }
        };
        let (Some(tg), Some(ts)) = (typ(&mut cx, *g, true), typ(&mut cx, *s, false)) else {
            continue;
        };
        let ok = {
            let mut env = SubtypeEnv::new(cx.table, &outline.hierarchy, core);
            is_subtype(tg, ts, &mut env)
        };
        // In an extension type, the representation getter gives way to the setter.
        let representation = matches!(k.0, Holder::Instance(_))
            && matches!(*g, Accessor::Variable(v) if matches!(program.variable(v).node, VariableRef::Representation { .. }));
        let place_of = |a: Accessor| -> Option<(UnitId, Span)> {
            match a {
                Accessor::Function(f) => name_of(program, f),
                Accessor::Variable(v) => variable_name(program, v),
                // The synthetic `values` is at the enum's name.
                Accessor::Values(c) => {
                    let d = program.class(c).decl?;
                    match &program.unit(d.unit).ast.decl(d.decl).kind {
                        DeclKind::Enum(e) => Some((d.unit, e.name.span)),
                        _ => None,
                    }
                }
            }
        };
        let place = if representation {
            place_of(*s)
        } else {
            place_of(*g)
        };
        if !ok && let Some((u, span)) = place {
            let the_name = interner.resolve(k.1).to_string();
            let args = [
                the_name.clone(),
                display_string(cx.table, tg, interner, program),
                display_string(cx.table, ts, interner, program),
                the_name,
            ];
            output.push((
                u,
                Diagnostic::with_code(c::GETTER_NOT_SUBTYPE_SETTER_TYPES, span, args),
            ));
        }
    }
    // `dart:core`'s `Enum`: an enum's synthetic getters `index` and `name`
    // are, in the interface, its own (`'Enum.index'`).
    let core_enum = program
        .classes
        .iter()
        .position(|k| {
            interner.resolve(k.name) == "Enum" && program.library(k.library).uri == "dart:core"
        })
        .map(|i| ClassId(i as u32));
    let enum_synthetic = |f: FunctionElementId| {
        let func = program.function(f);
        matches!(func.node, FunctionRef::None)
            && func.variable.is_none()
            && !func.static_
            && func.class.is_some_and(|c| {
                program.class(c).kind == dartforge_elements::model::ClassKind::Enum
            })
    };
    // The interface of each class (`InheritanceOverrideVerifier`'s
    // `checkInterface`) and of each extension type (`checkExtensionType`, from
    // `ErrorVerifier`); for an extension type, the pair it declares itself was
    // already handled by the local check above.
    let extensions: Vec<ClassId> = program
        .classes
        .iter()
        .enumerate()
        .filter(|(_, k)| {
            k.library == lib && k.kind == dartforge_elements::model::ClassKind::ExtensionType
        })
        .map(|(i, _)| ClassId(i as u32))
        .filter(|c| !classes.contains(c))
        .collect();
    for &cid in classes.iter().chain(extensions.iter()) {
        let cls = program.class(cid);
        let of_extension = cls.kind == dartforge_elements::model::ClassKind::ExtensionType;
        cx.library = cls.library;
        let Some(decl) = cls.decl else { continue };
        let Some(data_h) = outline.hierarchy.get(cid) else {
            continue;
        };
        let Some(params) = outline
            .classes
            .get(cid.0 as usize)
            .map(|d| d.type_params.clone())
        else {
            continue;
        };
        let args: Box<[TypeId]> = params
            .iter()
            .map(|&p| {
                cx.table.intern(Type::TypeParameter {
                    param: p,
                    nullable: false,
                })
            })
            .collect();
        let this_one = cx.table.intern(Type::Interface {
            class: cid,
            args,
            nullable: false,
        });
        let mut names: Vec<SymbolId> = Vec::new();
        for s in std::iter::once(cid).chain(data_h.supertypes.keys().copied()) {
            for (&key, &f) in &program.class(s).instance_members {
                if cx.kind(f) == Some(Kind::Getter) && !names.contains(&key) {
                    names.push(key);
                }
            }
        }
        names.sort_by_key(|n| interner.resolve(*n).to_string());
        for the_name in names {
            let text = interner.resolve(the_name).to_string();
            let Some(setter_key) = interner.lookup(&format!("{text}_=")) else {
                continue;
            };
            let Some((mut g, tg)) = cx.in_interface(this_one, the_name, 0) else {
                continue;
            };
            let Some((s, ts)) = cx.in_interface(this_one, setter_key, 0) else {
                continue;
            };
            if of_extension && g.owner == cid && s.owner == cid {
                continue;
            }
            if enum_synthetic(g.function)
                && let Some(e) = core_enum
            {
                g.owner = e;
            }
            if text.starts_with('_')
                && (program.class(g.owner).library != cls.library
                    || program.class(s.owner).library != cls.library)
            {
                continue;
            }
            if cx.kind(g.function) != Some(Kind::Getter)
                || cx.kind(s.function) != Some(Kind::Setter)
            {
                continue;
            }
            let (
                Type::Function { ret, .. },
                Type::Function {
                    positional,
                    optional,
                    named,
                    ..
                },
            ) = (cx.table.get(tg).clone(), cx.table.get(ts).clone())
            else {
                continue;
            };
            if positional.len() + optional.len() + named.len() != 1 {
                continue;
            }
            let Some(&param) = positional.first().or(optional.first()) else {
                continue;
            };
            let ok = {
                let mut env = SubtypeEnv::new(cx.table, &outline.hierarchy, core);
                is_subtype(ret, param, &mut env)
            };
            if ok {
                continue;
            }
            let place = if g.owner == cid {
                name_of(program, g.function)
            } else if s.owner == cid {
                name_of(program, s.function)
            } else {
                let ast_ = &program.unit(decl.unit).ast;
                match &ast_.decl(decl.decl).kind {
                    DeclKind::Class(d) => Some((decl.unit, d.name.span)),
                    DeclKind::Enum(d) => Some((decl.unit, d.name.span)),
                    DeclKind::Mixin(d) => Some((decl.unit, d.name.span)),
                    DeclKind::ExtensionType(d) => Some((decl.unit, d.name.span)),
                    _ => None,
                }
            };
            let Some((u, span)) = place else { continue };
            let qualified = |owner: ClassId| {
                if owner == cid {
                    text.clone()
                } else {
                    format!("{}.{text}", interner.resolve(program.class(owner).name))
                }
            };
            let args = [
                qualified(g.owner),
                display_string(cx.table, ret, interner, program),
                display_string(cx.table, param, interner, program),
                qualified(s.owner),
            ];
            output.push((
                u,
                Diagnostic::with_code(c::GETTER_NOT_SUBTYPE_SETTER_TYPES, span, args),
            ));
        }
    }
    output
}

/// Start of an annotated node (`AnnotatedNodeImpl.beginToken`): the
/// documentation comment right before it, if any.
/// The source before `pos` without the whitespace and comments that end
/// there (`// … method.` before a member is not a `.` before its name).
fn before_skipping_comments<'s>(
    source: &'s str,
    comments: &dartforge_frontend::comments::Comments,
    pos: usize,
) -> &'s str {
    let mut end = pos;
    loop {
        let trimmed = source[..end].trim_end();
        match comments.all().iter().find(|c| {
            c.end <= end
                && source[c.end..end].trim().is_empty()
                && c.end > 0
                && c.start < trimmed.len()
        }) {
            Some(c) => end = c.start,
            None => return trimmed,
        }
    }
}

fn start_with_documentation(source: &str, start: usize) -> usize {
    let without_space = source[..start].trim_end();
    if let Some(without_end) = without_space.strip_suffix("*/") {
        if let Some(i) = without_end.rfind("/**")
            && !without_end[i..].contains("*/")
        {
            return i;
        }
        return start;
    }
    let mut result = start;
    let mut end = without_space.len();
    loop {
        let start_line = without_space[..end].rfind('\n').map_or(0, |i| i + 1);
        let line = without_space[start_line..end].trim_start();
        if !(line.starts_with("///") && !line.starts_with("////")) {
            break;
        }
        result = end - line.len();
        if start_line == 0 {
            break;
        }
        end = without_space[..start_line].trim_end().len();
    }
    result
}

impl Ctx<'_> {
    /// `getMember2(d, the_name, concrete: true)`: the interface's `implemented`
    /// (with the `noSuchMethod` forwarders and inherited covariance).
    fn implemented(&mut self, d: ClassId, key: SymbolId, _prof: u32) -> Option<Finding> {
        let the_name = crate::inheritance::MemberName::new(self.interner, self.library, key);
        let mut h = std::mem::take(&mut self.inheritance);
        let r = {
            let mut p = crate::inheritance::OutlineProvider {
                program: self.program,
                interner: self.interner,
                core: self.core,
                outline: self.outline,
                table: &mut *self.table,
            };
            h.member(&mut p, d, the_name, true, None, false)
        };
        self.inheritance = h;
        r.map(|m| Finding {
            owner: m.class,
            function: m.function,
        })
    }

    /// `InterfaceElement.supertype`: the model's, and `Enum` for an enum
    /// (`LibraryBuilder.buildEnumChildren`).
    fn superclass(&self, d: ClassId) -> Option<ClassId> {
        let cls = self.program.class(d);
        match cls.supertype_class {
            // `extends` of an enum or extension type (`extends_non_class`): the
            // supertype becomes `Object`.
            Some(s) if self.outside_hierarchy(d, s) => self.core_class("Object"),
            Some(s) => Some(s),
            None if cls.kind == dartforge_elements::model::ClassKind::Enum => {
                self.core_class("Enum")
            }
            None => None,
        }
    }

    /// The supertype `s` of `d` is an enum or extension type in a class, mixin
    /// or enum (the analyzer drops it from the clause).
    fn outside_hierarchy(&self, d: ClassId, s: ClassId) -> bool {
        use dartforge_elements::model::ClassKind as K;
        if self.program.class(d).kind != K::ExtensionType
            && matches!(self.program.class(s).kind, K::Enum | K::ExtensionType)
        {
            return true;
        }
        // A class (not an enum) only reaches `dart:core`'s `Enum`/`_Enum` through
        // the `implements` of an enum, which is `implements_non_class`: the type
        // leaves the clause, and with it its supertypes.
        let sc = self.program.class(s);
        self.program.class(d).kind != K::Enum
            && matches!(self.interner.resolve(sc.name), "Enum" | "_Enum")
            && self.program.library(sc.library).uri == "dart:core"
    }

    fn of_object(&self, c: ClassId) -> bool {
        let cls = self.program.class(c);
        self.interner.resolve(cls.name) == "Object"
            && self.program.library(cls.library).uri == "dart:core"
    }

    fn core_class(&self, the_name: &str) -> Option<ClassId> {
        let lib = self.program.core?;
        let sym = self.interner.lookup(the_name)?;
        match self.program.library(lib).declared.get(&sym)?.getter? {
            dartforge_elements::model::Element::Class(c) => Some(c),
            _ => None,
        }
    }

    /// The private name `k` is declared by `c` or by a supertype from library
    /// `lib`.
    fn library_private(
        &self,
        c: ClassId,
        k: SymbolId,
        lib: dartforge_elements::model::LibraryId,
    ) -> bool {
        let supers = self
            .outline
            .hierarchy
            .get(c)
            .map(|d| d.supertypes.keys().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        std::iter::once(c).chain(supers).any(|s| {
            self.program.class(s).library == lib
                && self.program.class(s).instance_members.contains_key(&k)
        })
    }

    /// Some class of the hierarchy of `c` (itself included) has a
    /// `GetterMethodConflict` for `key`: the name is left out of its
    /// interface, and so of the interfaces that inherit from it, so the
    /// absence is decided (3.6.2 `inheritance_manager3.dart`,
    /// `_checkForGetterMethodConflict`).
    fn getter_method_conflict(&mut self, c: ClassId, key: SymbolId) -> bool {
        let the_name = crate::inheritance::MemberName::new(self.interner, self.library, key);
        let supers = self
            .outline
            .hierarchy
            .get(c)
            .map(|d| d.supertypes.keys().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut h = std::mem::take(&mut self.inheritance);
        let found = {
            let mut p = crate::inheritance::OutlineProvider {
                program: self.program,
                interner: self.interner,
                core: self.core,
                outline: self.outline,
                table: &mut *self.table,
            };
            std::iter::once(c).chain(supers).any(|s| {
                h.interface(&mut p, s).conflicts.iter().any(|x| {
                    matches!(x, crate::inheritance::Conflict::GetterMethod { name, .. } if *name == the_name)
                })
            })
        };
        self.inheritance = h;
        found
    }

    /// All instance member keys of `c` and of its supertypes.
    fn hierarchy_keys(&self, c: ClassId) -> Vec<SymbolId> {
        let mut v: Vec<SymbolId> = Vec::new();
        let supers = self
            .outline
            .hierarchy
            .get(c)
            .map(|d| d.supertypes.keys().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        for s in std::iter::once(c).chain(supers) {
            if s != c && self.outside_hierarchy(c, s) {
                continue;
            }
            for &k in self.program.class(s).instance_members.keys() {
                if !v.contains(&k) {
                    v.push(k);
                }
            }
        }
        v.sort_by_key(|k| self.interner.resolve(*k).to_string());
        v
    }

    fn own_type(&mut self, c: ClassId) -> Option<TypeId> {
        let params = self.outline.classes.get(c.0 as usize)?.type_params.clone();
        let args: Box<[TypeId]> = params
            .iter()
            .map(|&p| {
                self.table.intern(Type::TypeParameter {
                    param: p,
                    nullable: false,
                })
            })
            .collect();
        Some(self.table.intern(Type::Interface {
            class: c,
            args,
            nullable: false,
        }))
    }
}

/// The part of `InheritanceOverrideVerifier`'s `verify()` about concrete
/// classes and enums (`src/error/inheritance_override.dart`): for each name of
/// the interface without a concrete implementation,
/// `concrete_class_with_abstract_member` (or `enum_with_abstract_member`) at the
/// abstract member that the class itself declares, otherwise
/// `non_abstract_class_inherits_abstract_member` at the class name, with the
/// inherited members (minus those that the concrete superclass already leaves
/// unimplemented); if there is a `noSuchMethod` other than `Object`'s, the
/// missing ones are forwarded. With a concrete implementation of another
/// signature, `invalid_implementation_override`.
pub fn abstract_members(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    classes: &[ClassId],
) -> Vec<(UnitId, Diagnostic)> {
    let mut cx = Ctx {
        program,
        interner,
        table,
        core,
        outline,
        inheritance: crate::inheritance::InheritanceManager::default(),
        library: dartforge_elements::model::LibraryId(0),
    };
    let mut output = Vec::new();
    let nsm = interner.lookup("noSuchMethod");
    for &cid in classes {
        let cls = program.class(cid);
        cx.library = cls.library;
        let Some(decl) = cls.decl else { continue };
        let ast_ = &program.unit(decl.unit).ast;
        let source = &program.unit(decl.unit).source;
        let (class_name, members): (ast::Name, &[ast::MemberId]) = match &ast_.decl(decl.decl).kind
        {
            DeclKind::Class(d) if !d.modifiers.abstract_ && !d.modifiers.sealed => {
                (d.name, &d.members)
            }
            DeclKind::Enum(d) => (d.name, &d.members),
            _ => continue,
        };
        let is_enum = cls.kind == dartforge_elements::model::ClassKind::Enum;
        // An `augment` member, or a method whose name comes after `.` (a
        // constructor with the wrong class name): the parser's recovery
        // differs from the analyzer's and the class is undecided.
        let mut comments: Option<dartforge_frontend::comments::Comments> = None;
        let recovered = members.iter().any(|&m| {
            let mem = ast_.member(m);
            mem.augment
                || matches!(&mem.kind, MemberKind::Method(f)
                    if ast_.function(*f).name.is_some_and(|n| {
                        let cs = comments.get_or_insert_with(|| dartforge_frontend::comments::Comments::scan(source));
                        before_skipping_comments(source, cs, n.span.start).ends_with('.')
                    }))
        });
        // The word `augment` at the start of a member, without the experiment,
        // is read differently by the analyzer's parser.
        let sp = ast_.decl(decl.decl).span;
        let decl_text = source.get(sp.start..sp.end).unwrap_or("");
        // Without the experiment, the parser reads `augment` as an identifier
        // and the recovery is the analyzer's (T1.1 f): nothing to skip.
        let augment_in_text = program.library_has_augmentations(cls.library)
            && decl_text
                .lines()
                .any(|l| l.trim_start().starts_with("augment "));
        if recovered || augment_in_text || ast_.decl(decl.decl).augment {
            continue;
        }
        // T1.1 e: the interface is that of the owner of the same-named group (the
        // analyzer's cache is per element), and the abstract member is looked for
        // in the tree of the declaration itself
        // (`_reportConcreteClassWithAbstractMember`).
        let intf = program.class_owner(cid);
        let Some(this_one) = cx.own_type(intf) else {
            continue;
        };
        // An implemented `noSuchMethod` other than `Object`'s: the name without a
        // concrete implementation gets the forwarder (the interface member
        // becomes the `implemented`, 3.6.2 `inheritance_manager3.dart`), so
        // nothing is reported for it; an existing concrete implementation is
        // still checked (`Enum.index` against a `Never get index` in the
        // interface).
        let forwards = nsm.is_some_and(|n| {
            cx.implemented(intf, n, 0)
                .is_some_and(|a| program.library(program.class(a.owner).library).uri != "dart:core")
        });
        // `_isNotImplementedInConcreteSuperClass`: the declared superclass,
        // if concrete.
        let concrete_superclass = program.class(intf).supertype_class.filter(|&s| {
            let sc = program.class(s);
            sc.decl.is_some()
                && sc.kind == dartforge_elements::model::ClassKind::Class
                && !sc.modifiers.abstract_
                && !sc.modifiers.sealed
        });
        let mut the_inherited: Vec<String> = Vec::new();
        let mut uncertain = false;
        for key in cx.hierarchy_keys(intf) {
            let text = interner.resolve(key).to_string();
            // A private name declared only in other libraries (`_name` of
            // `_Enum`) is a different `Name` for this class: it is not in the
            // interface it sees.
            if text.starts_with('_') && !cx.library_private(intf, key, cls.library) {
                continue;
            }
            let Some((mem, member_type)) = cx.in_interface(this_one, key, 0) else {
                // A conflict or combined signature that is not decided here;
                // a getter/method conflict leaves the name out for certain.
                if !program.class(intf).instance_members.contains_key(&key)
                    && !cx.getter_method_conflict(intf, key)
                {
                    uncertain = true;
                }
                continue;
            };
            if text.starts_with('_') && program.class(mem.owner).library != cls.library {
                continue;
            }
            let Some(the_kind) = cx.kind(mem.function) else {
                continue;
            };
            let displayed = text.strip_suffix("_=").unwrap_or(&text).to_string();
            match cx.implemented(intf, key, 0) {
                None if forwards => {}
                None => {
                    // `_reportConcreteClassWithAbstractMember` (3.6.2
                    // `inheritance_override.dart:809-845`): the first member of
                    // the declaration with the name, static or not; a field also
                    // gives `the_name=` when it is not `final` (the node's `isFinal`
                    // does not count `const`).
                    let declared = members.iter().find(|&&m| match &ast_.member(m).kind {
                        MemberKind::Method(f) => {
                            let af = ast_.function(*f);
                            af.name.is_some_and(|n| {
                                let base = interner.resolve(n.sym);
                                if af.kind == ast::FunctionKind::Setter {
                                    format!("{base}_=") == text
                                } else {
                                    base == text && af.kind != ast::FunctionKind::Setter
                                }
                            })
                        }
                        MemberKind::Field(vl) => vl.variables.iter().any(|v| {
                            let n = interner.resolve(v.name.sym);
                            n == text || (!vl.final_ && format!("{n}_=") == text)
                        }),
                        MemberKind::Constructor(_) => false,
                    });
                    if let Some(&m) = declared {
                        let mut sp = ast_.member(m).span;
                        // `var get a;`, `const m();`, `abstract void m();`…: the
                        // modifiers that `MethodDeclaration` does not keep (syntax
                        // errors) are outside the node; `external`/`static` stay.
                        if let MemberKind::Method(_) = &ast_.member(m).kind {
                            const OUTSIDE: [&str; 7] = [
                                "var",
                                "const",
                                "final",
                                "abstract",
                                "covariant",
                                "late",
                                "required",
                            ];
                            while let Some(rest) = OUTSIDE.iter().find_map(|k| {
                                source[sp.start..sp.end]
                                    .strip_prefix(k)
                                    .filter(|r| r.starts_with(char::is_whitespace))
                            }) {
                                sp.start = sp.end - rest.trim_start().len();
                            }
                        }
                        let span = Span {
                            start: start_with_documentation(source, sp.start),
                            end: sp.end,
                        };
                        let code = if is_enum {
                            c::ENUM_WITH_ABSTRACT_MEMBER
                        } else {
                            c::CONCRETE_CLASS_WITH_ABSTRACT_MEMBER
                        };
                        output.push((
                            decl.unit,
                            Diagnostic::with_code(
                                code,
                                span,
                                [displayed.as_str(), interner.resolve(cls.name)],
                            ),
                        ));
                        continue;
                    }
                    if let Some(s) = concrete_superclass
                        && let Some(st) =
                            outline.hierarchy.supertype_of(this_one, s, cx.table, core)
                        && cx.in_interface(st, key, 0).is_some()
                    {
                        continue;
                    }
                    if is_enum && (text == "values" || text == "values_=") {
                        continue;
                    }
                    let prefix = match the_kind {
                        Kind::Getter => "getter ",
                        Kind::Setter => "setter ",
                        Kind::Method => "",
                    };
                    the_inherited.push(format!(
                        "{prefix}{}.{displayed}",
                        interner.resolve(program.class(mem.owner).name)
                    ));
                }
                Some(concrete) => {
                    // The concrete implementation against the interface signature.
                    if concrete == mem || cx.kind(concrete.function) != Some(the_kind) {
                        continue;
                    }
                    let Some(concrete_type) = cx.seen_type(this_one, concrete) else {
                        continue;
                    };
                    // `_inheritCovariance`: the covariance comes from all the
                    // same-named members in the class hierarchy.
                    let (pos, names) = cx.effective_covariant(intf, concrete.function, key);
                    let to_sub = cx.for_subtype(concrete_type, &pos, &names);
                    let ok = {
                        let mut env = SubtypeEnv::new(cx.table, &outline.hierarchy, core);
                        is_subtype(to_sub, member_type, &mut env)
                    };
                    if !ok {
                        let code = if the_kind == Kind::Setter {
                            c::INVALID_IMPLEMENTATION_OVERRIDE_SETTER
                        } else {
                            c::INVALID_IMPLEMENTATION_OVERRIDE
                        };
                        let args = [
                            displayed.clone(),
                            interner
                                .resolve(program.class(concrete.owner).name)
                                .to_string(),
                            display_string(cx.table, concrete_type, interner, program),
                            interner.resolve(program.class(mem.owner).name).to_string(),
                            display_string(cx.table, member_type, interner, program),
                        ];
                        output.push((
                            decl.unit,
                            Diagnostic::with_code(code, class_name.span, args),
                        ));
                    }
                }
            }
        }
        if uncertain || the_inherited.is_empty() {
            continue;
        }
        the_inherited.sort();
        let code = match the_inherited.len() {
            1 => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_ONE,
            2 => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_TWO,
            3 => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_THREE,
            4 => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_FOUR,
            _ => c::NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_FIVE_PLUS,
        };
        let mut args: Vec<String> = the_inherited.iter().take(4).cloned().collect();
        if the_inherited.len() > 4 {
            args.push((the_inherited.len() - 4).to_string());
        }
        output.push((
            decl.unit,
            Diagnostic::with_code(code, class_name.span, args),
        ));
    }
    output
}

/// `TypeSystemImpl.isNullable`: the type certainly accepts `null`.
fn is_nullable(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Never => false,
        Type::Interface { nullable, .. }
        | Type::Function { nullable, .. }
        | Type::Record { nullable, .. }
        | Type::TypeParameter { nullable, .. }
        | Type::ExtensionType { nullable, .. } => *nullable,
        Type::FutureOr { arg, nullable } => *nullable || is_nullable(table, *arg),
        Type::Intersection { bound, .. } => is_nullable(table, *bound),
    }
}

/// `NOT_INITIALIZED_NON_NULLABLE_VARIABLE`
/// (`_checkForNotInitializedNonNullableVariable`,
/// `an611:src/generated/error_verifier.dart:4872-4911`): a top-level
/// variable (not `final`) or static field, without `const`, `late` or
/// `external`, with a written potentially non-nullable type and no
/// initializer; at the name.
pub fn uninitialized_variables(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    use dartforge_elements::model::VariableRef;
    let mut output = Vec::new();
    for (i, v) in program.index().library_variables[lib.0 as usize]
        .iter()
        .map(|&x| (x.0 as usize, program.variable(x)))
    {
        if v.library != lib {
            continue;
        }
        let (unit, list, top) = match v.node {
            VariableRef::TopLevel { unit, decl, index } => {
                match &program.unit(unit).ast.decl(decl).kind {
                    ast::DeclKind::Variables(l) => (unit, (l, index), true),
                    _ => continue,
                }
            }
            VariableRef::Field {
                unit,
                member,
                index,
            } => match &program.unit(unit).ast.member(member).kind {
                MemberKind::Field(l) if l.static_ => (unit, (l, index), false),
                _ => continue,
            },
            _ => continue,
        };
        let (l, index) = list;
        if l.const_ || (top && l.final_) || l.late || l.external || l.ty.is_none() {
            continue;
        }
        let Some(var) = l.variables.get(index) else {
            continue;
        };
        if var.initializer.is_some() {
            continue;
        }
        let Some(t) = outline.variables.get(i).and_then(|d| d.declared_type) else {
            continue;
        };
        if is_nullable(table, t) {
            continue;
        }
        // Also at the synthetic (empty) name, like the analyzer.
        let the_name = var.name;
        output.push((
            unit,
            Diagnostic::with_code(
                c::NOT_INITIALIZED_NON_NULLABLE_VARIABLE,
                the_name.span,
                [interner.resolve(the_name.sym)],
            ),
        ));
    }
    output
}

/// `ErrorVerifier._checkUseOfDefaultValuesInParameters`: an optional
/// parameter without a default value whose type may not accept `null`
/// (`missing_default_value_for_parameter`, `_POSITIONAL`,
/// `_WITH_ANNOTATION` with `@required`), in the library's top-level
/// functions, methods and constructors where a default value is expected
/// (not abstract, not `external`, not native, not a redirecting `factory`).
/// The type counts when written or for a `this.x`; without a written type
/// (inferred from the override or from the superclass constructor), it is
/// undecided.
pub fn default_values(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    default_values_with(program, interner, table, None, outline, None, lib)
}

/// The `super.x` parameter (at index `i` in `f`) and the superclass
/// constructor parameter it forwards to (`superConstructorParameter`, 3.6.2
/// `element.dart:9291-9310`): by name, if named; otherwise by position among
/// the constructor's `super.x` parameters, in the positionals of the
/// superclass constructor (that of `super.the_name(…)`, or the unnamed one).
fn super_parameter(
    program: &Program,
    interner: &Interner,
    f: FunctionElementId,
    i: usize,
) -> Option<(FunctionElementId, usize)> {
    let FunctionRef::Constructor { unit, member } = program.function(f).node else {
        return None;
    };
    let MemberKind::Constructor(k) = &program.unit(unit).ast.member(member).kind else {
        return None;
    };
    let p = k.parameters.get(i)?;
    let c = program.function(f).class?;
    let s = program.class(c).supertype_class?;
    let super_name_of = k.initializers.iter().find_map(|x| match x {
        ast::Initializer::Super { constructor, .. } => Some(*constructor),
        _ => None,
    });
    let key = match super_name_of {
        Some(Some(n)) if interner.resolve(n.sym) != "new" => Some(n.sym),
        _ => interner.lookup(""),
    }?;
    let fs = *program.class(s).constructors.get(&key)?;
    let FunctionRef::Constructor {
        unit: su,
        member: sm,
    } = program.function(fs).node
    else {
        return None;
    };
    let MemberKind::Constructor(sk) = &program.unit(su).ast.member(sm).kind else {
        return None;
    };
    if p.kind == ast::ParameterKind::Named {
        let the_name = p.external_name()?.sym;
        let j = sk.parameters.iter().position(|q| {
            q.kind == ast::ParameterKind::Named
                && q.external_name().is_some_and(|n| n.sym == the_name)
        })?;
        return Some((fs, j));
    }
    let the_index = k.parameters[..i].iter().filter(|q| q.super_).count();
    let positional_args: Vec<usize> = sk
        .parameters
        .iter()
        .enumerate()
        .filter(|(_, q)| q.kind != ast::ParameterKind::Named)
        .map(|(j, _)| j)
        .collect();
    positional_args.get(the_index).map(|&j| (fs, j))
}

/// The type of the default value of parameter `i` of `f` (its
/// `computeConstantValue`: the written value, or the one forwarded through
/// the `super.x` chain), if any; `None` without a value. The value's type is
/// the static type of the written default (from the bodies), or the
/// parameter's without them.
fn parameter_default_value(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: Option<&CoreTypes>,
    outline: &OutlineTypes,
    bodies: Option<&crate::resolved::BodyTypes>,
    (f, i, prof): (FunctionElementId, usize, u32),
) -> Option<TypeId> {
    if prof > 32 {
        return None;
    }
    let FunctionRef::Constructor { unit, member } = program.function(f).node else {
        return None;
    };
    let MemberKind::Constructor(k) = &program.unit(unit).ast.member(member).kind else {
        return None;
    };
    let p = k.parameters.get(i)?;
    let type_p = outline.functions.get(f.0 as usize)?.parameters.get(i)?.ty;
    if let Some(d) = p.default_value {
        return Some(
            bodies
                .and_then(|c| c.units.get(unit.0 as usize))
                .and_then(|u| u.get_type(d))
                .unwrap_or(type_p),
        );
    }
    if !p.super_ {
        return None;
    }
    // `_superConstructorParameterDefaultValue`: the value of the superclass
    // parameter, if its type fits this one's.
    let (fs, j) = super_parameter(program, interner, f, i)?;
    let v = parameter_default_value(
        program,
        interner,
        table,
        core,
        outline,
        bodies,
        (fs, j, prof + 1),
    )?;
    let core = core?;
    // The value's type is not known here (the superclass's library without
    // inferred bodies, as when analyzing a single file in the LSP): the value
    // exists, and the check is left to whoever knows it (no false
    // `missing_default_value_for_parameter`).
    if v == core.dynamic_ {
        return Some(type_p);
    }
    let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
    is_subtype(v, type_p, &mut env).then_some(v)
}

/// `defaultValueCode != null` of parameter `i` of `f` (the
/// `hasDefaultValue`): the written default; for an optional `super.x`
/// without a default, the superclass parameter's, if the forwarded value
/// exists (`DefaultSuperFormalParameterElementImpl.defaultValueCode`,
/// `element.dart:1732-1748`); `required` never has one.
#[allow(clippy::too_many_arguments)]
fn has_default(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: Option<&CoreTypes>,
    outline: &OutlineTypes,
    bodies: Option<&crate::resolved::BodyTypes>,
    f: FunctionElementId,
    i: usize,
    prof: u32,
) -> bool {
    if prof > 32 {
        return false;
    }
    let FunctionRef::Constructor { unit, member } = program.function(f).node else {
        return false;
    };
    let MemberKind::Constructor(k) = &program.unit(unit).ast.member(member).kind else {
        return false;
    };
    let Some(p) = k.parameters.get(i) else {
        return false;
    };
    if p.default_value.is_some() {
        return true;
    }
    if !p.super_ || p.required || p.kind == ast::ParameterKind::Required {
        return false;
    }
    if parameter_default_value(
        program,
        interner,
        table,
        core,
        outline,
        bodies,
        (f, i, prof),
    )
    .is_none()
    {
        return false;
    }
    match super_parameter(program, interner, f, i) {
        Some((fs, j)) => has_default(
            program,
            interner,
            table,
            core,
            outline,
            bodies,
            fs,
            j,
            prof + 1,
        ),
        None => false,
    }
}

/// Like [`default_values`], with the core types and the bodies (the type of
/// the default value forwarded through `super.x`).
#[allow(clippy::too_many_arguments)]
pub fn default_values_with(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: Option<&CoreTypes>,
    outline: &OutlineTypes,
    bodies: Option<&crate::resolved::BodyTypes>,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    for (i, f) in program.index().library_functions[lib.0 as usize]
        .iter()
        .map(|&x| (x.0 as usize, program.function(x)))
    {
        if f.library != lib || f.patched_by.is_some() {
            continue;
        }
        let Some(data) = outline.functions.get(i) else {
            continue;
        };
        let (unit, params, expected): (UnitId, &[ast::Parameter], bool) = match f.node {
            FunctionRef::Function { unit, function } => {
                let af = &program.unit(unit).ast.functions[function.0 as usize];
                let native = matches!(af.body, ast::FunctionBody::Native(_));
                let the_abstract_ =
                    f.class.is_some() && matches!(af.body, ast::FunctionBody::Empty);
                (
                    unit,
                    af.parameters.as_deref().unwrap_or(&[]),
                    !af.external && !native && !the_abstract_,
                )
            }
            FunctionRef::Constructor { unit, member } => {
                match &program.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(k) => (
                        unit,
                        &k.parameters[..],
                        !k.external && !(k.factory && k.redirect.is_some()),
                    ),
                    _ => continue,
                }
            }
            FunctionRef::None => continue,
        };
        if !expected || params.len() != data.parameters.len() {
            continue;
        }
        // The line of the declaration itself, or that of its enclosing class,
        // starts with `augment`: without the experiment, the analyzer reads it
        // differently.
        let source = &program.unit(unit).source;
        let line_starts_with_augment = |pos: usize| {
            let begin = source[..pos.min(source.len())]
                .rfind('\n')
                .map_or(0, |i| i + 1);
            source[begin..].trim_start().starts_with("augment ")
        };
        let start = match f.node {
            FunctionRef::Function { function, .. } => {
                program.unit(unit).ast.functions[function.0 as usize]
                    .span
                    .start
            }
            FunctionRef::Constructor { member, .. } => {
                program.unit(unit).ast.member(member).span.start
            }
            FunctionRef::None => 0,
        };
        let of_class = f
            .class
            .and_then(|c| program.class(c).decl)
            .is_some_and(|d| {
                d.unit == unit
                    && line_starts_with_augment(program.unit(unit).ast.decl(d.decl).span.start)
            });
        let the_augmented = program.library_has_augmentations(program.unit(unit).library)
            && (line_starts_with_augment(start) || of_class);
        if the_augmented {
            continue;
        }
        for (idx, (p, pd)) in params.iter().zip(data.parameters.iter()).enumerate() {
            if p.kind == ast::ParameterKind::Required || p.required || p.default_value.is_some() {
                continue;
            }
            // Optional `super.x`: the default forwarded from the superclass
            // constructor counts (`hasDefaultValue`); the positional wildcard
            // `_` with the feature is left out
            // (`_isWildcardSuperFormalPositionalParameter`).
            if p.super_ {
                // A repeated `_` (without the wildcards feature, `duplicate_definition`)
                // does not reach the superclass parameter: no inherited default
                // (checked against the 3.6.2 oracle, `C(this.c, super._, [super._])`).
                let repeated = p.name.is_some_and(|n| {
                    interner.resolve(n.sym) == "_"
                        && params[..idx]
                            .iter()
                            .any(|q| q.name.is_some_and(|m| m.sym == n.sym))
                });
                if !repeated
                    && has_default(
                        program,
                        interner,
                        table,
                        core,
                        outline,
                        bodies,
                        FunctionElementId(i as u32),
                        idx,
                        0,
                    )
                {
                    continue;
                }
                let wildcard = p.kind == ast::ParameterKind::Optional
                    && p.name.is_some_and(|n| interner.resolve(n.sym) == "_")
                    && program
                        .library(lib)
                        .features
                        .has(dartforge_frontend::Feature::WildcardVariables);
                if wildcard {
                    continue;
                }
            }
            // A written type that does not resolve (`dynamic` in the outline): nothing.
            if is_nullable(table, pd.ty) {
                continue;
            }
            let Some(the_name) = p.name else { continue };
            let annotated = p.metadata.iter().any(|a| {
                a.name
                    .last()
                    .is_some_and(|n| interner.resolve(n.sym) == "required")
            });
            let d = if annotated {
                Diagnostic::with_code(
                    c::MISSING_DEFAULT_VALUE_FOR_PARAMETER_WITH_ANNOTATION,
                    the_name.span,
                    [] as [&str; 0],
                )
            } else {
                let code = if p.kind == ast::ParameterKind::Optional {
                    c::MISSING_DEFAULT_VALUE_FOR_PARAMETER_POSITIONAL
                } else {
                    c::MISSING_DEFAULT_VALUE_FOR_PARAMETER
                };
                Diagnostic::with_code(code, the_name.span, [interner.resolve(the_name.sym)])
            };
            output.push((unit, d));
        }
    }
    output
}

/// `ErrorVerifier._checkForConflictingClassMembers` (the library's classes,
/// mixins and extension types): a declared static member against an
/// inherited member (`conflicting_static_and_instance`, with the class that
/// owns the inherited one), a declared method against an inherited field
/// (`conflicting_method_and_field`), a declared accessor against an
/// inherited method (`conflicting_field_and_method`), and an inherited
/// method and setter with the same name
/// (`conflicting_inherited_method_and_setter`), through `getInherited2`.
pub fn conflicting_members(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    use dartforge_elements::model::ClassKind as K;
    let mut cx = Ctx {
        program,
        interner,
        table,
        core,
        outline,
        inheritance: crate::inheritance::InheritanceManager::default(),
        library: dartforge_elements::model::LibraryId(0),
    };
    let mut output = Vec::new();
    for (ci, cls) in program.classes.iter().enumerate() {
        cx.library = cls.library;
        let cid = ClassId(ci as u32);
        if cls.library != lib || !matches!(cls.kind, K::Class | K::Mixin | K::ExtensionType) {
            continue;
        }
        let Some(decl) = cls.decl else { continue };
        let ast_ = &program.unit(decl.unit).ast;
        if ast_.decl(decl.decl).augment {
            continue;
        }
        let source = &program.unit(decl.unit).source;
        if program.library_has_augmentations(lib)
            && source
                .lines()
                .any(|l| l.trim_start().starts_with("augment "))
        {
            continue;
        }
        let (class_name, members): (ast::Name, &[ast::MemberId]) = match &ast_.decl(decl.decl).kind
        {
            DeclKind::Class(d) => (d.name, &d.members),
            DeclKind::Mixin(d) => (d.name, &d.members),
            DeclKind::ExtensionType(d) => (d.name, &d.members),
            _ => continue,
        };
        // T1.1 e: the inherited interface is that of the owner of the same-named
        // group; the declared members are those of the declaration itself.
        let intf = program.class_owner(cid);
        let Some(this_one) = cx.own_type(intf) else {
            continue;
        };
        let class_text = interner.resolve(cls.name).to_string();
        let visible_inherited = |cx: &mut Ctx<'_>, the_name: &str| -> Option<Finding> {
            let key = interner.lookup(the_name)?;
            let (a, _) = cx.inherited(this_one, key)?;
            if the_name.starts_with('_') && program.class(a.owner).library != lib {
                return None;
            }
            Some(a)
        };
        let owner = |a: Finding| interner.resolve(program.class(a.owner).name).to_string();
        let mut conflicting: Vec<String> = Vec::new();
        // Declared methods against inherited accessors.
        for &m in members {
            let MemberKind::Method(f) = &ast_.member(m).kind else {
                continue;
            };
            let af = ast_.function(*f);
            if !matches!(
                af.kind,
                ast::FunctionKind::Function | ast::FunctionKind::Operator
            ) {
                continue;
            }
            let Some(n) = af.name else { continue };
            let the_name = interner.resolve(n.sym).to_string();
            let getter = visible_inherited(&mut cx, &the_name);
            let setter = visible_inherited(&mut cx, &format!("{the_name}_="));
            if af.static_
                && let Some(a) = getter.or(setter)
            {
                output.push((
                    decl.unit,
                    Diagnostic::with_code(
                        c::CONFLICTING_STATIC_AND_INSTANCE,
                        n.span,
                        [class_text.clone(), the_name.clone(), owner(a)],
                    ),
                ));
                continue;
            }
            if cls.kind == K::ExtensionType {
                continue;
            }
            let accessor = |a: Option<Finding>| {
                a.filter(|a| cx.kind(a.function).is_some_and(|e| e != Kind::Method))
            };
            if let Some(a) = accessor(getter).or(accessor(setter)) {
                output.push((
                    decl.unit,
                    Diagnostic::with_code(
                        c::CONFLICTING_METHOD_AND_FIELD,
                        n.span,
                        [class_text.clone(), the_name.clone(), owner(a)],
                    ),
                ));
            }
        }
        // Declared accessors (getters, setters and those of fields) against
        // inherited methods.
        for &m in members {
            let mut accessors: Vec<(String, Span, bool)> = Vec::new();
            match &ast_.member(m).kind {
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    if matches!(
                        af.kind,
                        ast::FunctionKind::Getter | ast::FunctionKind::Setter
                    ) && let Some(n) = af.name
                    {
                        accessors.push((interner.resolve(n.sym).to_string(), n.span, af.static_));
                    }
                }
                MemberKind::Field(vl) => {
                    for v in vl.variables.iter() {
                        let the_name = interner.resolve(v.name.sym).to_string();
                        accessors.push((the_name.clone(), v.name.span, vl.static_));
                        if !vl.final_ && !vl.const_ {
                            accessors.push((the_name, v.name.span, vl.static_));
                        }
                    }
                }
                MemberKind::Constructor(_) => {}
            }
            for (the_name, span, static_) in accessors {
                let inherited = visible_inherited(&mut cx, &the_name)
                    .or_else(|| visible_inherited(&mut cx, &format!("{the_name}_=")));
                if static_ && let Some(a) = inherited {
                    output.push((
                        decl.unit,
                        Diagnostic::with_code(
                            c::CONFLICTING_STATIC_AND_INSTANCE,
                            span,
                            [class_text.clone(), the_name.clone(), owner(a)],
                        ),
                    ));
                    conflicting.push(the_name);
                } else if let Some(a) = inherited
                    && cx.kind(a.function) == Some(Kind::Method)
                {
                    if cls.kind == K::ExtensionType {
                        continue;
                    }
                    output.push((
                        decl.unit,
                        Diagnostic::with_code(
                            c::CONFLICTING_FIELD_AND_METHOD,
                            span,
                            [class_text.clone(), the_name.clone(), owner(a)],
                        ),
                    ));
                    conflicting.push(the_name);
                }
            }
        }
        // An inherited method and setter with the same name.
        for key in cx.hierarchy_keys(intf) {
            let the_name = interner.resolve(key).to_string();
            if the_name.ends_with("_=") || conflicting.contains(&the_name) {
                continue;
            }
            let Some(method) = visible_inherited(&mut cx, &the_name) else {
                continue;
            };
            if cx.kind(method.function) != Some(Kind::Method) {
                continue;
            }
            if let Some(setter) = visible_inherited(&mut cx, &format!("{the_name}_="))
                && cx.kind(setter.function) == Some(Kind::Setter)
            {
                let typ = match cls.kind {
                    K::Mixin => "mixin",
                    K::ExtensionType => "extension type",
                    _ => "class",
                };
                // The two context messages: where the method and the setter come from
                // (the type and the name of the owner class, at the member's name).
                let mut d = Diagnostic::with_code(
                    c::CONFLICTING_INHERITED_METHOD_AND_SETTER,
                    class_name.span,
                    [typ, class_text.as_str(), the_name.as_str()],
                );
                for (a, template) in [
                    (method, "The method is inherited from the"),
                    (setter, "The setter is inherited from the"),
                ] {
                    let p = program;
                    let (f, owner) = (a.function, a.owner);
                    let owner_kind = match p.class(owner).kind {
                        K::Mixin => "mixin",
                        K::Enum => "enum",
                        K::ExtensionType => "extension type",
                        _ => "class",
                    };
                    let Some((u, span)) = p.non_synthetic_function_name(f) else {
                        continue;
                    };
                    let file = (u != decl.unit).then(|| p.unit_path(u).into());
                    let text = format!(
                        "{template} {owner_kind} '{}'.",
                        interner.resolve(p.class(owner).name)
                    );
                    d.context.push(dartforge_diagnostics::ContextMessage {
                        file,
                        span,
                        message: text.into(),
                    });
                }
                output.push((decl.unit, d));
            }
        }
    }
    output
}

/// `DuplicateDefinitionVerifier._checkEnumStatic`: each static accessor
/// (those of the constants and fields, declared getters and setters; except
/// `values`) and each static method of an enum whose name is in its
/// interface (`getMember2` of the name or of the setter, with the members
/// inherited from `Enum` and `Object`), at the name, with the enum as owner.
pub fn enum_statics(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    use dartforge_elements::model::ClassKind as K;
    let mut cx = Ctx {
        program,
        interner,
        table,
        core,
        outline,
        inheritance: crate::inheritance::InheritanceManager::default(),
        library: lib,
    };
    let mut output = Vec::new();
    for (ci, cls) in program.classes.iter().enumerate() {
        if cls.library != lib || cls.kind != K::Enum {
            continue;
        }
        let Some(decl) = cls.decl else { continue };
        let ast_ = &program.unit(decl.unit).ast;
        let DeclKind::Enum(d) = &ast_.decl(decl.decl).kind else {
            continue;
        };
        let owner = program.class_owner(ClassId(ci as u32));
        let enum_text = interner.resolve(program.class(owner).name).to_string();
        let in_interface = |cx: &mut Ctx<'_>, the_name: &str| -> bool {
            // The model's synthetic `name` getter (`outline.rs`, for the back ends)
            // is not in the analyzer's interface: an enum's `name` is `dart:core`'s
            // `EnumName` extension (`Enum` only has `_name`), and `static int name`
            // in an enum does not conflict.
            let synthetic = |m: &crate::inheritance::InterfaceMember| {
                let f = program.function(m.function);
                m.class == owner
                    && f.node == dartforge_elements::model::FunctionRef::None
                    && interner.resolve(f.name) == "name"
            };
            let found = |cx: &mut Ctx<'_>, text: &str| {
                interner.lookup(text).is_some_and(|k| {
                    cx.inheritance_member(owner, k, false)
                        .is_some_and(|m| !synthetic(&m))
                })
            };
            found(cx, the_name) || found(cx, &format!("{the_name}_="))
        };
        // The accessors: the constants (getters), the static fields (getter and,
        // if not final, setter) and the declared getters and setters.
        let mut accessors: Vec<(String, Span)> = Vec::new();
        for k in d.constants.iter() {
            accessors.push((interner.resolve(k.name.sym).to_string(), k.name.span));
        }
        let mut methods: Vec<(String, Span)> = Vec::new();
        for &m in d.members.iter() {
            match &ast_.member(m).kind {
                MemberKind::Field(vl) if vl.static_ => {
                    for v in vl.variables.iter() {
                        let the_name = interner.resolve(v.name.sym).to_string();
                        accessors.push((the_name.clone(), v.name.span));
                        if !vl.final_ && !vl.const_ {
                            accessors.push((the_name, v.name.span));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    if !af.static_ {
                        continue;
                    }
                    let Some(n) = af.name else { continue };
                    let the_name = interner.resolve(n.sym).to_string();
                    match af.kind {
                        ast::FunctionKind::Getter | ast::FunctionKind::Setter => {
                            accessors.push((the_name, n.span))
                        }
                        _ => methods.push((the_name, n.span)),
                    }
                }
                _ => {}
            }
        }
        for (the_name, span) in accessors {
            if the_name != "values" && in_interface(&mut cx, &the_name) {
                output.push((
                    decl.unit,
                    Diagnostic::with_code(
                        c::CONFLICTING_STATIC_AND_INSTANCE,
                        span,
                        [enum_text.as_str(), the_name.as_str(), enum_text.as_str()],
                    ),
                ));
            }
        }
        for (the_name, span) in methods {
            if in_interface(&mut cx, &the_name) {
                output.push((
                    decl.unit,
                    Diagnostic::with_code(
                        c::CONFLICTING_STATIC_AND_INSTANCE,
                        span,
                        [enum_text.as_str(), the_name.as_str(), enum_text.as_str()],
                    ),
                ));
            }
        }
        // `_checkEnum` (3.6.2 `duplicate_definition_verifier.dart:600-640`): the
        // declared instance accessor (getter, setter, those of a field) against
        // the inherited method, and the instance method against the inherited
        // accessor (`_getInheritedMember`: the inherited getter, otherwise the
        // setter; a private name from another library is not seen). With a
        // same-named enum in the same unit (the `augment enum E` that 3.6.2 reads
        // as another declaration, or a repeated `enum E`), 3.6.2 reports nothing,
        // in any order: both elements are equal by location.
        let same_name = program.classes.iter().enumerate().any(|(j, k)| {
            j != ci
                && k.kind == K::Enum
                && k.name == cls.name
                && k.decl.is_some_and(|x| x.unit == decl.unit)
        });
        if same_name {
            continue;
        }
        let Some(this_one) = cx.own_type(owner) else {
            continue;
        };
        let inherited = |cx: &mut Ctx<'_>, the_name: &str| -> Option<Finding> {
            let visible = |cx: &mut Ctx<'_>, text: &str| -> Option<Finding> {
                let key = interner.lookup(text)?;
                let (a, _) = cx.inherited(this_one, key)?;
                if text.starts_with('_') && program.class(a.owner).library != lib {
                    return None;
                }
                Some(a)
            };
            visible(cx, the_name).or_else(|| visible(cx, &format!("{the_name}_=")))
        };
        let mut accessors: Vec<(String, Span)> = Vec::new();
        let mut methods: Vec<(String, Span)> = Vec::new();
        for &m in d.members.iter() {
            match &ast_.member(m).kind {
                MemberKind::Field(vl) if !vl.static_ => {
                    for v in vl.variables.iter() {
                        let the_name = interner.resolve(v.name.sym).to_string();
                        accessors.push((the_name.clone(), v.name.span));
                        if !vl.final_ && !vl.const_ {
                            accessors.push((the_name, v.name.span));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    if af.static_ {
                        continue;
                    }
                    let Some(n) = af.name else { continue };
                    let the_name = interner.resolve(n.sym).to_string();
                    match af.kind {
                        ast::FunctionKind::Getter | ast::FunctionKind::Setter => {
                            accessors.push((the_name, n.span))
                        }
                        _ => methods.push((the_name, n.span)),
                    }
                }
                _ => {}
            }
        }
        let owner_of = |a: &Finding| interner.resolve(program.class(a.owner).name).to_string();
        for (the_name, span) in accessors {
            if let Some(a) = inherited(&mut cx, &the_name)
                && cx.kind(a.function) == Some(Kind::Method)
            {
                let d_ = owner_of(&a);
                output.push((
                    decl.unit,
                    Diagnostic::with_code(
                        c::CONFLICTING_FIELD_AND_METHOD,
                        span,
                        [enum_text.as_str(), the_name.as_str(), d_.as_str()],
                    ),
                ));
            }
        }
        for (the_name, span) in methods {
            if let Some(a) = inherited(&mut cx, &the_name)
                && cx.kind(a.function).is_some_and(|e| e != Kind::Method)
            {
                let d_ = owner_of(&a);
                output.push((
                    decl.unit,
                    Diagnostic::with_code(
                        c::CONFLICTING_METHOD_AND_FIELD,
                        span,
                        [enum_text.as_str(), the_name.as_str(), d_.as_str()],
                    ),
                ));
            }
        }
    }
    output
}

/// The part of `verify()` about classes that implement `Enum`
/// (`implementsDartCoreEnum`, through the transitive supertypes, and every
/// enum): concrete `index`, `hashCode` and `==` declared
/// (`illegal_concrete_enum_member`, in the class, the enum or the mixin) or
/// inherited from a class other than `Object`/`Enum` (at the class name),
/// and an instance member `values` declared (outside an enum) or inherited
/// (`illegal_enum_values`).
pub fn enum_members(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    classes: &[ClassId],
) -> Vec<(UnitId, Diagnostic)> {
    use dartforge_elements::model::ClassKind as K;
    let mut cx = Ctx {
        program,
        interner,
        table,
        core,
        outline,
        inheritance: crate::inheritance::InheritanceManager::default(),
        library: dartforge_elements::model::LibraryId(0),
    };
    let mut output = Vec::new();
    let Some(enum_core) = cx.core_class("Enum") else {
        return output;
    };
    for &cid in classes {
        let cls = program.class(cid);
        cx.library = cls.library;
        let Some(decl) = cls.decl else { continue };
        let implements_enum = cls.kind == K::Enum
            || outline
                .hierarchy
                .get(cid)
                .is_some_and(|d| d.supertypes.contains_key(&enum_core));
        if !implements_enum || cid == enum_core {
            continue;
        }
        let ast_ = &program.unit(decl.unit).ast;
        let source = &program.unit(decl.unit).source;
        if program.library_has_augmentations(cls.library)
            && source
                .lines()
                .any(|l| l.trim_start().starts_with("augment "))
        {
            continue;
        }
        let (class_name, members): (ast::Name, &[ast::MemberId]) = match &ast_.decl(decl.decl).kind
        {
            DeclKind::Class(d) => (d.name, &d.members),
            DeclKind::Enum(d) => (d.name, &d.members),
            DeclKind::Mixin(d) => (d.name, &d.members),
            _ => continue,
        };
        let is_enum = cls.kind == K::Enum;
        let forbidden = |n: &str| matches!(n, "index" | "hashCode" | "==");
        // Declarados.
        for &m in members {
            match &ast_.member(m).kind {
                MemberKind::Field(vl) if !vl.static_ => {
                    for v in vl.variables.iter() {
                        let n = interner.resolve(v.name.sym);
                        if !is_enum && n == "values" {
                            output.push((
                                decl.unit,
                                Diagnostic::with_code(
                                    c::ILLEGAL_ENUM_VALUES_DECLARATION,
                                    v.name.span,
                                    [] as [&str; 0],
                                ),
                            ));
                        }
                        if forbidden(n) {
                            output.push((
                                decl.unit,
                                Diagnostic::with_code(
                                    c::ILLEGAL_CONCRETE_ENUM_MEMBER_DECLARATION,
                                    v.name.span,
                                    [n],
                                ),
                            ));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let af = ast_.function(*f);
                    let Some(the_name) = af.name else { continue };
                    let n = interner.resolve(the_name.sym);
                    let the_abstract_ = !af.external && matches!(af.body, ast::FunctionBody::Empty);
                    let setter = af.kind == ast::FunctionKind::Setter;
                    if !(af.static_ || the_abstract_ || setter) && forbidden(n) {
                        output.push((
                            decl.unit,
                            Diagnostic::with_code(
                                c::ILLEGAL_CONCRETE_ENUM_MEMBER_DECLARATION,
                                the_name.span,
                                [n],
                            ),
                        ));
                    }
                    if !af.static_ && !is_enum && n == "values" {
                        output.push((
                            decl.unit,
                            Diagnostic::with_code(
                                c::ILLEGAL_ENUM_VALUES_DECLARATION,
                                the_name.span,
                                [] as [&str; 0],
                            ),
                        ));
                    }
                }
                _ => {}
            }
        }
        // Inherited (`getInheritedConcreteMap2`; mixins do not inherit).
        if cls.kind != K::Mixin {
            for n in ["hashCode", "==", "index"] {
                let Some(key) = interner.lookup(n) else {
                    continue;
                };
                let mut inherited = None;
                for &m in cls.mixin_classes.iter().rev() {
                    if cx.outside_hierarchy(cid, m) {
                        continue;
                    }
                    if let Some(a) = cx.implemented(m, key, 1)
                        && !cx.of_object(a.owner)
                    {
                        inherited = Some(a);
                        break;
                    }
                }
                if inherited.is_none()
                    && let Some(s) = cx.superclass(cid)
                {
                    inherited = cx.implemented(s, key, 1);
                }
                let Some(a) = inherited else { continue };
                let owner = program.class(a.owner);
                let count = if owner.kind != K::Class && owner.kind != K::MixinApplication {
                    true
                } else if n == "index" {
                    a.owner != enum_core
                } else {
                    !cx.of_object(a.owner)
                };
                if count {
                    output.push((
                        decl.unit,
                        Diagnostic::with_code(
                            c::ILLEGAL_CONCRETE_ENUM_MEMBER_INHERITANCE,
                            class_name.span,
                            [n, interner.resolve(owner.name)],
                        ),
                    ));
                }
            }
        }
        let Some(this_one) = cx.own_type(cid) else {
            continue;
        };
        let values = interner.lookup("values");
        let values_set = interner.lookup("values_=");
        let inherited = values
            .and_then(|k| cx.inherited(this_one, k))
            .or_else(|| values_set.and_then(|k| cx.inherited(this_one, k)));
        if let Some((a, _)) = inherited {
            output.push((
                decl.unit,
                Diagnostic::with_code(
                    c::ILLEGAL_ENUM_VALUES_INHERITANCE,
                    class_name.span,
                    [interner.resolve(program.class(a.owner).name)],
                ),
            ));
        }
    }
    output
}
