//! Resolution of outline type annotations to [`TypeId`].
//!
//! Walks the whole [`Program`] (classes, mixins, enums, extensions, extension types,
//! typedefs, functions, methods, getters, setters, constructors and variables/fields)
//! and resolves every written type annotation (`TypeAnnotation`) to its [`TypeId`].
//!
//! Supports nested scopes (local type parameters shadow the library), transparent
//! `typedef` expansion, *override inference* of inherited members without an
//! explicit annotation, and accumulation of diagnostics without stopping early.

use crate::hierarchy::{ClassHierarchy, build_class_hierarchy};
use crate::ops::{nullable, substitute};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeParamOwner, TypeTable, Variance};
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, ExtensionId, FunctionElement, FunctionElementId, FunctionKind,
    FunctionRef, LibraryId, Program, TypedefId, UnitId, VariableRef,
};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

type ParameterComponents = (Vec<TypeId>, Vec<TypeId>, Vec<(SymbolId, TypeId, bool)>);

/// Type data of a function/method/constructor in the outline.
#[derive(Debug, Clone)]
pub struct FunctionTypeData {
    /// Full signature as a function type (`Type::Function`).
    pub signature: TypeId,
    /// Return type.
    pub return_type: TypeId,
    /// Generic type parameters declared in this function.
    pub type_params: Box<[TypeParamId]>,
    /// Metadata and types of each formal parameter.
    pub parameters: Box<[ParameterTypeData]>,
}

/// Metadata of a function's formal parameter.
#[derive(Debug, Clone)]
pub struct ParameterTypeData {
    /// Local name (the one in the body scope and, in `this.x`, the field's).
    pub name: Option<SymbolId>,
    /// External name: the one in the signature and the call. Differs from
    /// `name` only for a private named parameter in 3.12 (`{this._x}` is passed
    /// as `x:`).
    pub externo: Option<SymbolId>,
    pub ty: TypeId,
    pub required: bool,
    pub kind: ParameterKind,
}

/// Type data of a top-level variable or field.
#[derive(Debug, Clone)]
pub struct VariableTypeData {
    /// Type explicitly annotated in the declaration (if any).
    pub declared_type: Option<TypeId>,
    /// Resolved / inferred type (in the outline, `None` if it depends on the initializer).
    pub inferred: Option<TypeId>,
}

/// Annotated type data of a class, mixin, enum or extension type.
#[derive(Debug, Clone)]
pub struct ClassTypeData {
    pub type_params: Box<[TypeParamId]>,
    pub supertype: Option<TypeId>,
    pub mixins: Box<[TypeId]>,
    pub interfaces: Box<[TypeId]>,
    pub on: Box<[TypeId]>,
}

/// Annotated type data of an extension.
#[derive(Debug, Clone)]
pub struct ExtensionTypeData {
    pub type_params: Box<[TypeParamId]>,
    pub on: TypeId,
}

impl OutlineTypes {
    /// Replaces the `ast::TypeId`s of `u` in [`OutlineTypes::tipos_escritos`]
    /// using `map` (a unit's text swap without reloading,
    /// `dartforge_elements::incremental`). Refuses, without changing anything, if
    /// any key of the unit is not in the map.
    pub fn remapear_tipos_escritos(&mut self, u: UnitId, map: &HashMap<ast::TypeId, ast::TypeId>) -> bool {
        let of_unit: Vec<((UnitId, ast::TypeId), TypeId)> = self.tipos_escritos.iter().filter(|((un, _), _)| *un == u).map(|(k, v)| (*k, *v)).collect();
        if of_unit.iter().any(|((_, t), _)| !map.contains_key(t)) {
            return false;
        }
        for (k, _) in &of_unit {
            self.tipos_escritos.remove(k);
        }
        for ((un, t), v) in of_unit {
            self.tipos_escritos.insert((un, map[&t]), v);
        }
        true
    }
}

/// Type data of a typedef.
#[derive(Debug, Clone)]
pub struct TypedefTypeData {
    pub type_params: Box<[TypeParamId]>,
    pub target_type: TypeId,
}

/// Set of all outline type side tables, indexed by element IDs.
#[derive(Debug, Clone)]
pub struct OutlineTypes {
    pub functions: Vec<FunctionTypeData>,
    pub variables: Vec<VariableTypeData>,
    pub classes: Vec<ClassTypeData>,
    pub extensions: Vec<ExtensionTypeData>,
    pub typedefs: Vec<TypedefTypeData>,
    pub hierarchy: ClassHierarchy,
    /// Overrides whose inherited type comes from a field without a written type
    /// (`var the_list = [1.5];`): the field's type only exists after inferring the
    /// initializer, and body inference completes the signature before anything
    /// else.
    pub field_overrides: Vec<FieldOverride>,
    /// The type of each type annotation that the outline resolved (signatures,
    /// clauses, bounds, and the annotations inside them), by unit and node: the
    /// analyzer's `TypeAnnotation.type`. Those of the bodies are in
    /// [`crate::resolved::UnitBodyTypes::tipos_de_anotacoes`].
    pub tipos_escritos: HashMap<(UnitId, ast::TypeId), TypeId>,
    /// The faithful override inference (`InstanceMemberInferrer`) has already
    /// run over these types (in the first body inference).
    pub inferred_overrides: bool,
}

/// Omitted part of an override that inherits the type of a field without a
/// written type in the supertype (override inference over an inferred type).
#[derive(Debug, Clone)]
pub struct FieldOverride {
    /// The overriding member (getter, setter or method).
    pub function: FunctionElementId,
    /// The overridden field.
    pub field: dartforge_elements::model::VariableId,
    /// Type parameters of the field's class -> arguments as seen from the
    /// overrider's class.
    pub subst: Vec<(TypeParamId, TypeId)>,
    /// `None`: the return; `Some(i)`: the i-th parameter.
    pub parameter: Option<usize>,
}

/// Where a type name appears: the `parent` of the `NamedType` that
/// `NamedTypeResolver` consults to choose the code for a name that does not
/// resolve to a type (`an611:src/dart/resolver/named_type_resolver.dart:515-643`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypeContext {
    /// Ordinary annotation (variable, parameter, return, bound…).
    #[default]
    Normal,
    /// Element of a type argument list (`List<X>`, `f<X>()`).
    TypeArgument,
    /// The type of `on T` in a `catch`.
    Catch,
    /// The type of `e as T`.
    As,
    /// The type of `e is T` / `e is! T`.
    Is,
    /// A type in `extends`, `implements` or `with` (of a class, enum, mixin,
    /// class alias or extension type): `reportNullOrNonTypeElement` does not
    /// report an undefined name or a name that is not a type there ("The error
    /// will be reported elsewhere", `named_type_resolver.dart:601-608`), only
    /// `boolean`, which comes first.
    Hierarchy,
}

/// The analyzer's code and arguments for a type name that does not resolve
/// to a type (`_ErrorHelper.reportNullOrNonTypeElement`,
/// `an611:src/dart/resolver/named_type_resolver.dart:515-643`): `found` means
/// the lookup found an element (that is not a type). Inheritance clauses and
/// instance creation do not go through here (other codes).
pub fn type_name_code(context: TypeContext, found: bool, the_name: &str) -> dartforge_diagnostics::Code {
    use dartforge_diagnostics::codes::compile_time_error as c;
    if the_name == "boolean" {
        return c::UNDEFINED_CLASS_BOOLEAN;
    }
    match context {
        TypeContext::Catch => c::NON_TYPE_IN_CATCH_CLAUSE,
        TypeContext::As => c::CAST_TO_NON_TYPE,
        TypeContext::Is if found => c::TYPE_TEST_WITH_NON_TYPE,
        TypeContext::Is => c::TYPE_TEST_WITH_UNDEFINED_NAME,
        TypeContext::TypeArgument => c::NON_TYPE_AS_TYPE_ARGUMENT,
        TypeContext::Normal if found => c::NOT_A_TYPE,
        TypeContext::Normal if the_name == "await" => c::UNDEFINED_IDENTIFIER_AWAIT,
        TypeContext::Normal | TypeContext::Hierarchy => c::UNDEFINED_CLASS,
    }
}

/// The diagnostic of [`type_name_code`] at the name (`range`: from
/// the prefix, if any, to the end of the name, as in `_getErrorRange`).
pub fn type_name_diagnostic(context: TypeContext, found: bool, the_name: &str, range: dartforge_diagnostics::Span) -> Diagnostic {
    let code = type_name_code(context, found, the_name);
    if code == dartforge_diagnostics::codes::compile_time_error::UNDEFINED_IDENTIFIER_AWAIT {
        return Diagnostic::with_code(code, range, std::iter::empty::<&str>());
    }
    Diagnostic::with_code(code, range, [the_name])
}

/// `WRONG_NUMBER_OF_TYPE_ARGUMENTS` (`_buildTypeArguments`,
/// `an611:src/dart/resolver/named_type_resolver.dart:136-150`): on the whole
/// type, with the name, the number of parameters and the number of
/// arguments.
pub fn type_arguments_diagnostic(the_name: &str, params_list: usize, args_list: usize, span: dartforge_diagnostics::Span) -> Diagnostic {
    Diagnostic::with_code(
        dartforge_diagnostics::codes::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS,
        span,
        [the_name.to_string(), params_list.to_string(), args_list.to_string()],
    )
}

/// The type parameter belongs to a class, mixin, enum or extension.
pub(crate) fn class_param(table: &TypeTable, p: TypeParamId) -> bool {
    matches!(table.param(p).owner, TypeParamOwner::Class(_) | TypeParamOwner::Extension(_))
}

/// `TYPE_PARAMETER_REFERENCED_BY_STATIC` (`_checkForTypeParameterReferencedByStatic`,
/// `an611:src/generated/error_verifier.dart:5419-5434`): a type parameter of the
/// class cited in a static method or field, at the name.
pub fn parameter_in_static_diagnostic(range: dartforge_diagnostics::Span) -> Diagnostic {
    Diagnostic::with_code(
        dartforge_diagnostics::codes::compile_time_error::TYPE_PARAMETER_REFERENCED_BY_STATIC,
        range,
        std::iter::empty::<&str>(),
    )
}

/// The name `sym` in the instance scope of a container (class, mixin, enum,
/// extension type or extension): the **declared** members (getters,
/// including the implicit ones of fields and enum constants, methods,
/// setters), static or not (`an611:src/dart/element/scope.dart:273-279`,
/// `ExtensionScope` at `:92-100`). An enum's synthetic `values` counts (it is
/// a static field declared by the analyzer); the synthetic `index` does not
/// (it is inherited from `Enum`).
pub(crate) fn name_in_container(
    program: &Program,
    interner: &Interner,
    the_class: Option<ClassId>,
    extension: Option<ExtensionId>,
    sym: SymbolId,
) -> Option<InContainer> {
    let (instance, statics, fields) = match (the_class, extension) {
        (Some(c), _) => {
            let k = program.class(c);
            (&k.instance_members, &k.static_members, k.fields.iter().chain(&k.enum_constants).copied().collect::<Vec<_>>())
        }
        (None, Some(e)) => {
            let x = &program.extensions[e.0 as usize];
            (&x.instance_members, &x.static_members, x.fields.clone())
        }
        _ => return None,
    };
    let the_declared = |f: FunctionElementId| {
        let fe = &program.functions[f.0 as usize];
        !matches!(fe.node, FunctionRef::None) || fe.variable.is_some() || (fe.static_ && interner.resolve(fe.name) == "values")
    };
    let getter = [instance, statics]
        .iter()
        .any(|m| m.get(&sym).is_some_and(|&f| the_declared(f) && program.functions[f.0 as usize].kind != FunctionKind::Constructor))
        || fields.iter().any(|v| program.variables[v.0 as usize].name == sym);
    if getter {
        return Some(InContainer::Getter);
    }
    let setter_name = format!("{}_=", interner.resolve(sym));
    let setter = interner
        .lookup(&setter_name)
        .is_some_and(|k| [instance, statics].iter().any(|m| m.get(&k).is_some_and(|&f| the_declared(f))));
    setter.then_some(InContainer::OnlySetter)
}

/// `NamedTypeResolver.resolve` (`named_type_resolver.dart:89-121`): the first
/// name of `p.N` is looked up in the `ResolutionVisitor` scope; if the
/// element found is not a prefix, class or alias, it is
/// `PREFIX_SHADOWED_BY_LOCAL_DECLARATION` at the prefix, and the type is
/// invalid. The scope, from inside out: the type parameters and the locals
/// (`local`, decided by the caller), the declared members of the container
/// (the `LocalScope` in which `ResolutionVisitor` defines `accessors` and
/// `methods` by their names: a setter goes under `x=` and does not hide
/// `x`) and the top-level scope ([`top_level_shadowed_prefix`]).
pub(crate) fn shadowed_prefix(
    program: &Program,
    interner: &Interner,
    unit: UnitId,
    p: SymbolId,
    local: bool,
    the_class: Option<ClassId>,
    extension: Option<ExtensionId>,
) -> bool {
    local
        || matches!(name_in_container(program, interner, the_class, extension, p), Some(InContainer::Getter))
        || top_level_shadowed_prefix(program, unit, p)
}

/// `LibraryFragmentScope.lookup` (`scope.dart:424-440`): the library's
/// declarations come before the prefixes and imports. A function, variable or
/// extension (or an ambiguous name, the `MultiplyDefinedElement`) with the
/// prefix's name hides it; only a setter is a null `getter` (undefined type,
/// not a shadow).
pub(crate) fn top_level_shadowed_prefix(program: &Program, unit: UnitId, p: SymbolId) -> bool {
    let lib = program.library(program.unit(unit).library);
    let b = match lib.declared.get(&p) {
        Some(b) => *b,
        None if program.prefixes_in_unit(unit).contains_key(&p) => return false,
        None => match program.lookup_in_unit(unit, p) {
            Some(b) => b,
            None => return false,
        },
    };
    b.ambiguous || matches!(b.getter, Some(Element::Function(_) | Element::Variable(_) | Element::Extension(_)))
}

/// `PREFIX_SHADOWED_BY_LOCAL_DECLARATION` at the prefix, with its name.
pub fn shadowed_prefix_diagnostic(the_name: &str, range: dartforge_diagnostics::Span) -> Diagnostic {
    Diagnostic::with_code(dartforge_diagnostics::codes::compile_time_error::PREFIX_SHADOWED_BY_LOCAL_DECLARATION, range, [the_name])
}

/// A declaration whose members form a scope (the analyzer's `InstanceScope`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container {
    Classe(ClassId),
    Extensao(ExtensionId),
}

/// What a name is in the instance scope of a container.
pub(crate) enum InContainer {
    /// Getter (including the implicit one of a field or enum constant) or
    /// method: the lookup result has a `getter`, and is not a type.
    Getter,
    /// Setter only: the `InstanceScope` lookup (the linker's, which gives the
    /// element's type) stops here, without a `getter`: invalid type. The
    /// `ResolutionVisitor`'s (the one that reports) does not see it and goes on
    /// outward.
    OnlySetter,
}

/// Resolution context with access to the program and state accumulators.
pub struct OutlineResolver<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a mut TypeTable,
    pub core: &'a CoreTypes,
    pub diagnostics: Vec<Diagnostic>,
    /// Unit of each diagnostic (parallel to `diagnostics`): the file in which it
    /// is reported, without having to guess from the range.
    pub warning_units: Vec<UnitId>,
    /// The container (class, mixin, enum, extension type or extension) whose
    /// members are in scope in the annotation being resolved: the analyzer's
    /// `InstanceScope` (docs/ANALISADOR-PARIDADE-PLANO.md §3.3).
    container: Option<Container>,
    /// The context of the annotation that the next call to `resolve_annotation`
    /// resolves (read and cleared on entry; type arguments set it to
    /// [`TypeContext::TypeArgument`]).
    type_context: TypeContext,
    /// Resolving the signature of a static method or field (the class's type
    /// parameters do not apply there).
    static_member: bool,
    /// Type parameters allocated per class: `[ClassId] -> Box<[TypeParamId]>`
    pub class_type_params: Vec<Box<[TypeParamId]>>,
    /// Type parameters allocated per typedef: `[TypedefId] -> Box<[TypeParamId]>`
    pub typedef_type_params: Vec<Box<[TypeParamId]>>,
    /// Type parameters allocated per extension: `[ExtensionId] -> Box<[TypeParamId]>`
    pub extension_type_params: Vec<Box<[TypeParamId]>>,
    /// Expanded target types of typedefs: `[TypedefId] -> Option<TypeId>`
    pub typedef_targets: Vec<Option<TypeId>>,
    /// Cache of the `hasSelfReference` of each `typedef` (`auto_referencia`).
    self_referencing_typedefs: Vec<Option<bool>>,
    /// The bounds that `_breakRawTypeCycles` replaces with `dynamic`
    /// ([`crate::raw_cycles`]); computed on the first read of the bounds.
    broken_bounds: Option<std::collections::HashSet<(dartforge_elements::model::DeclRef, usize)>>,
    /// Ver [`OutlineTypes::field_overrides`].
    pub field_overrides: Vec<FieldOverride>,
    /// Ver [`OutlineTypes::tipos_escritos`].
    pub tipos_escritos: HashMap<(UnitId, ast::TypeId), TypeId>,
}

impl<'a> OutlineResolver<'a> {
    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a mut TypeTable,
        core: &'a CoreTypes,
    ) -> Self {
        let num_classes = program.classes.len();
        let num_typedefs = program.typedefs.len();
        let num_extensions = program.extensions.len();

        Self {
            program,
            interner,
            table,
            core,
            diagnostics: Vec::new(),
            warning_units: Vec::new(),
            container: None,
            type_context: TypeContext::Normal,
            static_member: false,
            class_type_params: vec![Box::new([]); num_classes],
            typedef_type_params: vec![Box::new([]); num_typedefs],
            extension_type_params: vec![Box::new([]); num_extensions],
            typedef_targets: vec![None; num_typedefs],
            self_referencing_typedefs: vec![None; num_typedefs],
            broken_bounds: None,
            field_overrides: Vec::new(),
            tipos_escritos: HashMap::new(),
        }
    }

    /// Records a diagnostic reported in `the_unit`.

    /// `reportNullOrNonTypeElement` (named_type_resolver.dart:518-521): the
    /// synthetic name (empty, from parser recovery) is not reported.
    fn warn_type_name(&mut self, unit_id: UnitId, context: TypeContext, found: bool, text: &str, range: dartforge_diagnostics::Span) {
        if text.is_empty() || context == TypeContext::Hierarchy && text != "boolean" {
            return;
        }
        self.warn(unit_id, type_name_diagnostic(context, found, text, range));
    }
    /// The name `sym` from the `dart:core` namespace is in scope without a
    /// prefix: the library does not import core explicitly (the implicit
    /// import), or imports it without a prefix with combinators that let it
    /// through.
    fn visible_core(&self, the_unit: UnitId, sym: SymbolId) -> bool {
        let lib = self.program.unit(the_unit).library;
        if Some(lib) == self.program.core {
            return true;
        }
        // Like the outline's implicit core (`elements::outline`): only the import
        // of the defining unit counts (`LibraryFileKind`'s
        // `hasDartCoreImport`); an augmentation's
        // `import 'dart:core' as prefix0;` does not hide the library's `dynamic`.
        let principal = self.program.library(lib).units.first().copied();
        let imports = &self.program.library(lib).imports;
        let do_core: Vec<_> = imports.iter().filter(|i| Some(i.library) == self.program.core && Some(i.unit) == principal).collect();
        if do_core.is_empty() {
            return true;
        }
        do_core.iter().any(|i| {
            i.prefix.is_none()
                && i.combinators.iter().all(|c| match c {
                    ast::Combinator::Show(ns) => ns.iter().any(|n| n.sym == sym),
                    ast::Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == sym),
                })
        })
    }

    fn warn(&mut self, the_unit: UnitId, d: Diagnostic) {
        self.diagnostics.push(d);
        self.warning_units.push(the_unit);
    }

    /// The name `sym` in the instance scope of the container being resolved
    /// ([`name_in_container`]).
    fn in_container(&self, sym: SymbolId) -> Option<InContainer> {
        let (the_class, extension) = match self.container? {
            Container::Classe(c) => (Some(c), None),
            Container::Extensao(e) => (None, Some(e)),
        };
        name_in_container(self.program, self.interner, the_class, extension, sym)
    }

    /// Resolves the program's whole outline, returning the side tables and the accumulated diagnostics.
    pub fn resolve_all(self) -> (OutlineTypes, Vec<Diagnostic>) {
        let (o, d, _) = self.resolve_all_with_units();
        (o, d)
    }

    /// Like [`OutlineResolver::resolve_all`], with the unit of each diagnostic.
    pub fn resolve_all_with_units(mut self) -> (OutlineTypes, Vec<Diagnostic>, Vec<UnitId>) {
        // 1. Allocate TypeParamIds for classes, typedefs and extensions
        self.allocate_outline_type_params();

        // 2. Resolve typedefs (supporting mutual references)
        self.resolve_typedefs();

        // 3. Resolve the classes' supertype annotations and build the instantiated hierarchy
        let (class_type_data, hierarchy) = self.resolve_classes_and_hierarchy();

        // 4. Resolver extensions
        let extension_type_data = self.resolve_extensions();

        // 5. Resolve top-level variables and fields
        let mut variable_type_data = self.resolve_variables();

        // 6. Resolve functions, methods and constructors (with override inference)
        let function_type_data = self.resolve_functions(&hierarchy, &mut variable_type_data);

        let typedef_type_data: Vec<TypedefTypeData> = self
            .typedef_type_params
            .iter()
            .zip(self.typedef_targets.iter())
            .map(|(params, target)| TypedefTypeData {
                type_params: params.clone(),
                target_type: target.unwrap_or(self.core.dynamic_),
            })
            .collect();

        let outline = OutlineTypes {
            functions: function_type_data,
            variables: variable_type_data,
            classes: class_type_data,
            extensions: extension_type_data,
            typedefs: typedef_type_data,
            hierarchy,
            field_overrides: std::mem::take(&mut self.field_overrides),
            tipos_escritos: std::mem::take(&mut self.tipos_escritos),
            inferred_overrides: false,
        };

        (outline, self.diagnostics, self.warning_units)
    }

    fn allocate_outline_type_params(&mut self) {
        for (i, class) in self.program.classes.iter().enumerate() {
            let class_id = ClassId(i as u32);
            let mut params = Vec::with_capacity(class.type_params.len());
            // The written variance (`class C<out T>`), kept even with the
            // experiment disabled, like the analyzer's `ElementBuilder`.
            let written_ones: &[dartforge_frontend::ast::TypeParameter] = match class.decl {
                Some(d) => match &self.program.unit(d.unit).ast.decl(d.decl).kind {
                    dartforge_frontend::ast::DeclKind::Class(k) => &k.type_params,
                    dartforge_frontend::ast::DeclKind::Mixin(k) => &k.type_params,
                    dartforge_frontend::ast::DeclKind::Enum(k) => &k.type_params,
                    _ => &[],
                },
                None => &[],
            };
            for (j, p) in class.type_params.iter().enumerate() {
                let variance = match written_ones.get(j).and_then(|t| t.variance).map(|v| v.0) {
                    Some(dartforge_frontend::ast::Variance::In) => Variance::Contravariant,
                    Some(dartforge_frontend::ast::Variance::Out) => Variance::Covariant,
                    Some(dartforge_frontend::ast::Variance::Inout) => Variance::Invariant,
                    None => Variance::Unspecified,
                };
                let pid = self.table.alloc_type_param(
                    p.name,
                    TypeParamOwner::Class(class_id),
                    self.core.object_nullable,
                    variance,
                );
                params.push(pid);
            }
            self.class_type_params[i] = params.into_boxed_slice();
        }

        for (i, typedef) in self.program.typedefs.iter().enumerate() {
            let typedef_id = TypedefId(i as u32);
            let mut params = Vec::with_capacity(typedef.type_params.len());
            for p in typedef.type_params.iter() {
                let pid = self.table.alloc_type_param(
                    p.name,
                    TypeParamOwner::Typedef(typedef_id),
                    self.core.object_nullable,
                    Variance::Unspecified,
                );
                params.push(pid);
            }
            self.typedef_type_params[i] = params.into_boxed_slice();
        }

        for (i, ext) in self.program.extensions.iter().enumerate() {
            let ext_id = ExtensionId(i as u32);
            let mut params = Vec::with_capacity(ext.type_params.len());
            for p in ext.type_params.iter() {
                let pid = self.table.alloc_type_param(
                    p.name,
                    TypeParamOwner::Extension(ext_id),
                    self.core.object_nullable,
                    Variance::Unspecified,
                );
                params.push(pid);
            }
            self.extension_type_params[i] = params.into_boxed_slice();
        }

        // Update written bounds for classes. Three passes: a raw type in a
        // bound (`I extends Restriction`) is instantiated to the bounds of
        // the other class, which may not have been read yet in the first one
        // (it would come out as `Restriction<Object?>` instead of
        // `Restriction<Object>`); chains of raw bounds up to three levels;
        // only the last one reports.
        let (n_diag, n_units) = (self.diagnostics.len(), self.warning_units.len());
        for passed in 0..3 {
            if passed > 0 {
                self.diagnostics.truncate(n_diag);
                self.warning_units.truncate(n_units);
            }
            self.resolve_class_bounds();
        }

        // And those of typedefs (`typedef F<X extends num> = X Function();`):
        // without them, the bound was `Object?` and `F<Object>` passed.
        for (i, typedef) in self.program.typedefs.iter().enumerate() {
            let params = self.typedef_type_params[i].clone();
            let mut scope = HashMap::with_capacity(params.len());
            for (p_elem, &pid) in typedef.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
            for (j, (p_elem, &pid)) in typedef.type_params.iter().zip(params.iter()).enumerate() {
                if let Some((unit_id, ast_ty_id)) = p_elem.bound {
                    let bound_ty = self.resolve_annotation(unit_id, ast_ty_id, typedef.library, &scope);
                    let broken = self.broken_bound(typedef.decl, j);
                    self.table.set_type_param_bound(pid, if broken { self.core.dynamic_ } else { bound_ty });
                }
            }
        }
    }

    /// The written bounds of the type parameters of all classes.
    fn resolve_class_bounds(&mut self) {
        for (i, class) in self.program.classes.iter().enumerate() {
            let params = self.class_type_params[i].clone();
            let mut scope = HashMap::with_capacity(params.len());
            for (p_elem, &pid) in class.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
            for (j, (p_elem, &pid)) in class.type_params.iter().zip(params.iter()).enumerate() {
                if let Some((unit_id, ast_ty_id)) = p_elem.bound {
                    let bound_ty = self.resolve_annotation(unit_id, ast_ty_id, class.library, &scope);
                    let broken = class.decl.is_some_and(|d| self.broken_bound(d, j));
                    self.table.set_type_param_bound(pid, if broken { self.core.dynamic_ } else { bound_ty });
                }
            }
        }
    }

    /// The bound of parameter `j` of declaration `d` becomes `dynamic`
    /// (`_breakRawTypeCycles`, [`crate::raw_cycles`]).
    fn broken_bound(&mut self, d: dartforge_elements::model::DeclRef, j: usize) -> bool {
        if self.broken_bounds.is_none() {
            self.broken_bounds = Some(crate::raw_cycles::broken_bounds(self.program));
        }
        self.broken_bounds.as_ref().is_some_and(|q| q.contains(&(d, j)))
    }

    fn resolve_typedefs(&mut self) {
        for i in 0..self.program.typedefs.len() {
            let typedef_id = TypedefId(i as u32);
            self.ensure_typedef_resolved(typedef_id);
        }
    }

    /// The `typedef` reaches itself (`hasSelfReference`): instantiates as
    /// `dynamic` (`TypeAliasElementImpl.instantiate`, `element.dart:9690-9696`).
    fn self_referencing_typedef(&mut self, id: TypedefId) -> bool {
        let i = id.0 as usize;
        if let Some(r) = self.self_referencing_typedefs[i] {
            return r;
        }
        let r = crate::auto_referencia::typedef_auto_referente(self.program, self.program.typedefs[i].decl);
        self.self_referencing_typedefs[i] = Some(r);
        r
    }

    fn ensure_typedef_resolved(&mut self, id: TypedefId) -> TypeId {
        let idx = id.0 as usize;
        if let Some(target) = self.typedef_targets[idx] {
            return target;
        }

        let typedef = &self.program.typedefs[idx];
        let params = self.typedef_type_params[idx].clone();
        let mut scope = HashMap::with_capacity(params.len());
        for (p_elem, &pid) in typedef.type_params.iter().zip(params.iter()) {
            scope.insert(p_elem.name, pid);
        }

        // Sets a temporary fallback to avoid cycles
        self.typedef_targets[idx] = Some(self.core.dynamic_);

        let unit_id = typedef.decl.unit;
        let decl = self.program.unit(unit_id).ast.decl(typedef.decl.decl);
        let resolved_target = match &decl.kind {
            DeclKind::Typedef(d) => match &d.kind {
                ast::TypedefKind::Alias(ast_ty) => {
                    self.resolve_annotation(unit_id, *ast_ty, typedef.library, &scope)
                }
                ast::TypedefKind::Legacy {
                    return_type,
                    parameters,
                } => {
                    let ret = if let Some(r) = return_type {
                        self.resolve_annotation(unit_id, *r, typedef.library, &scope)
                    } else {
                        self.core.dynamic_
                    };
                    let (pos, opt, named) = self.resolve_ast_parameter_types(
                        unit_id,
                        parameters,
                        typedef.library,
                        &scope,
                    );
                    self.table.intern(Type::Function {
                        type_params: Box::new([]),
                        ret,
                        positional: pos.into_boxed_slice(),
                        optional: opt.into_boxed_slice(),
                        named: named.into_boxed_slice(),
                        nullable: false,
                    })
                }
            },
            _ => self.core.dynamic_,
        };

        self.typedef_targets[idx] = Some(resolved_target);
        resolved_target
    }

    fn resolve_classes_and_hierarchy(&mut self) -> (Vec<ClassTypeData>, ClassHierarchy) {
        let mut class_type_data = Vec::with_capacity(self.program.classes.len());
        let mut hierarchy_inputs: Vec<Option<crate::hierarchy::ImmediateSupertypeInput>> =
            Vec::with_capacity(self.program.classes.len());

        let enum_type = self.program.core.and_then(|core| {
            let sym = self.interner.lookup("Enum")?;
            match self.program.lookup(core, sym)?.getter {
                Some(dartforge_elements::model::Element::Class(c)) => {
                    Some(self.table.intern(Type::Interface { class: c, args: Box::new([]), nullable: false }))
                }
                _ => None,
            }
        });
        for (i, class) in self.program.classes.iter().enumerate() {
            let params = self.class_type_params[i].clone();
            let mut scope = HashMap::with_capacity(params.len());
            for (p_elem, &pid) in class.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }

            let supertype = class.supertype.map(|(unit, ast_id)| {
                self.type_context = TypeContext::Hierarchy;
                self.resolve_annotation(unit, ast_id, class.library, &scope)
            });

            let mixins: Vec<TypeId> = class
                .mixins
                .iter()
                .map(|&(unit, ast_id)| {
                    self.type_context = TypeContext::Hierarchy;
                    self.resolve_annotation(unit, ast_id, class.library, &scope)
                })
                .collect();

            let interfaces: Vec<TypeId> = class
                .interfaces
                .iter()
                .map(|&(unit, ast_id)| {
                    self.type_context = TypeContext::Hierarchy;
                    self.resolve_annotation(unit, ast_id, class.library, &scope)
                })
                .collect();

            let on: Vec<TypeId> = class
                .on
                .iter()
                .map(|&(unit, ast_id)| self.resolve_annotation(unit, ast_id, class.library, &scope))
                .collect();

            let mut all_direct = Vec::new();
            if let Some(s) = supertype {
                all_direct.push(s);
            }
            all_direct.extend_from_slice(&mixins);
            all_direct.extend_from_slice(&interfaces);
            all_direct.extend_from_slice(&on);
            // Every `enum` declaration is a subtype of `Enum` (§14.1: the class
            // implicitly extends `_Enum`, which implements `Enum`), even without a
            // written superclass: `Cor <: Enum` decides the bounds of
            // `extension EnumByName<T extends Enum> on Iterable<T>`.
            if class.kind == dartforge_elements::model::ClassKind::Enum {
                if let Some(e) = enum_type {
                    all_direct.push(e);
                }
            }

            hierarchy_inputs.push(Some((params.clone(), all_direct)));

            class_type_data.push(ClassTypeData {
                type_params: params,
                supertype,
                mixins: mixins.into_boxed_slice(),
                interfaces: interfaces.into_boxed_slice(),
                on: on.into_boxed_slice(),
            });
        }

        // T1.1 d: the hierarchy of a same-named class is that of the group's
        // owner (the analyzer's `ClassHierarchy._map` is per element, and
        // same-named classes are the same element as key). The type's
        // identity remains that of each class.
        for i in 0..hierarchy_inputs.len() {
            let owner = self.program.class_owner(ClassId(i as u32));
            if owner.0 as usize != i && (owner.0 as usize) < hierarchy_inputs.len() {
                hierarchy_inputs[i] = hierarchy_inputs[owner.0 as usize].clone();
            }
        }
        let mut hierarchy = build_class_hierarchy(
            self.program.classes.len(),
            &hierarchy_inputs,
            self.table,
            self.core,
        );
        // `_MixinInference` (3.6.2 `types_builder.dart:461-605`): a generic
        // mixin written without arguments (`with M`) has its arguments
        // inferred from its constraints (the mixin's `on`; the superclass and
        // mixins of the class used as a mixin, without the last one of a class
        // alias) matched against the already applied supertypes (the
        // superclass and the previous mixins, with their supertypes:
        // `InterfacesMerger`), through `matchSupertypeConstraints`
        // (`type_system.dart:1448-1487`). Without a match, the raw type stays.
        // When something changes, the hierarchy is rebuilt and the pass repeats
        // (the superclass may have inferred mixins).
        for _ in 0..4 {
            let mut changed = false;
            for i in 0..self.program.classes.len() {
                for j in 0..self.program.classes[i].mixins.len() {
                    if let Some(the_new_) = self.infer_mixin(&class_type_data, &hierarchy, i, j) {
                        class_type_data[i].mixins[j] = the_new_;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
            for i in 0..self.program.classes.len() {
                let d = &class_type_data[i];
                let mut all = Vec::new();
                all.extend(d.supertype);
                all.extend_from_slice(&d.mixins);
                all.extend_from_slice(&d.interfaces);
                all.extend_from_slice(&d.on);
                if self.program.classes[i].kind == dartforge_elements::model::ClassKind::Enum {
                    if let Some(e) = enum_type {
                        all.push(e);
                    }
                }
                hierarchy_inputs[i] = Some((d.type_params.clone(), all));
            }
            for i in 0..hierarchy_inputs.len() {
                let owner = self.program.class_owner(ClassId(i as u32));
                if owner.0 as usize != i && (owner.0 as usize) < hierarchy_inputs.len() {
                    hierarchy_inputs[i] = hierarchy_inputs[owner.0 as usize].clone();
                }
            }
            hierarchy = build_class_hierarchy(self.program.classes.len(), &hierarchy_inputs, self.table, self.core);
        }
        // An extension type is a subtype of `Object` only if some
        // superinterface (the `implements`) is: a class, or another extension
        // type that is.
        let mut memo: HashMap<usize, bool> = HashMap::new();
        fn with_object(
            i: usize,
            the_data: &[ClassTypeData],
            program: &Program,
            table: &TypeTable,
            memo: &mut HashMap<usize, bool>,
            prof: u32,
        ) -> bool {
            if program.classes[i].kind != ClassKind::ExtensionType {
                return true;
            }
            if let Some(&r) = memo.get(&i) {
                return r;
            }
            if prof > 64 {
                return true;
            }
            memo.insert(i, true);
            let r = the_data[i].interfaces.iter().any(|&t| match table.get(t) {
                Type::Interface { .. } => true,
                Type::ExtensionType { decl, .. } => with_object(decl.0 as usize, the_data, program, table, memo, prof + 1),
                _ => false,
            });
            memo.insert(i, r);
            r
        }
        for i in 0..self.program.classes.len() {
            if self.program.classes[i].kind == ClassKind::ExtensionType
                && !with_object(i, &class_type_data, self.program, self.table, &mut memo, 0)
            {
                hierarchy.extensions_without_object.insert(ClassId(i as u32));
            }
        }

        (class_type_data, hierarchy)
    }

    /// `_MixinInference._inferSingle` for mixin `j` of class `i`: the new type,
    /// when inference changes it.
    fn infer_mixin(&mut self, the_data: &[ClassTypeData], the_hierarchy: &ClassHierarchy, i: usize, j: usize) -> Option<TypeId> {
        let (unit, ast_id) = self.program.classes[i].mixins[j];
        let without_args = matches!(&self.program.unit(unit).ast.ty(ast_id).kind, ast::TypeKind::Named { args, .. } if args.is_empty());
        if !without_args {
            return None;
        }
        let mt = *the_data[i].mixins.get(j)?;
        let Type::Interface { class: m, nullable, .. } = self.table.get(mt).clone() else { return None };
        let params = the_data[m.0 as usize].type_params.clone();
        if params.is_empty() {
            return None;
        }
        // `gatherMixinSupertypeConstraintsForInference` (`type_system.dart:500-518`).
        let mk = &self.program.classes[m.0 as usize];
        let candidates: Vec<TypeId> = if mk.kind == dartforge_elements::model::ClassKind::Mixin {
            the_data[m.0 as usize].on.to_vec()
        } else {
            let s = the_data[m.0 as usize].supertype?;
            let mut v = vec![s];
            v.extend(the_data[m.0 as usize].mixins.iter().copied());
            let alias = mk.decl.is_some_and(|d| {
                matches!(&self.program.unit(d.unit).ast.decl(d.decl).kind, DeclKind::Class(x) if x.mixin_application)
            });
            if alias {
                v.pop();
            }
            v
        };
        let constraints: Vec<(TypeId, ClassId)> = candidates
            .into_iter()
            .filter_map(|r| match self.table.get(r) {
                Type::Interface { class, .. } if !the_data[class.0 as usize].type_params.is_empty() => Some((r, *class)),
                _ => None,
            })
            .collect();
        // `InterfacesMerger.typeList` of the superclass and the previous mixins,
        // per element like `_ClassInterfaceType.update`
        // (`class_hierarchy.dart:170-196`): the first type; a different one
        // then joins the ones normalized by `topMerge`, and a failure keeps the
        // previous result.
        let sources: Vec<TypeId> = the_data[i].supertype.into_iter().chain(the_data[i].mixins[..j].iter().copied()).collect();
        let mut targets: Vec<TypeId> = Vec::new();
        for &(_, rc) in constraints.iter() {
            let (mut unique, mut current, mut error): (Option<TypeId>, Option<TypeId>, bool) = (None, None, false);
            for &f in sources.iter() {
                let Some(t) = the_hierarchy.supertype_of(f, rc, self.table, self.core) else { continue };
                if error {
                    continue;
                }
                if current.is_none() {
                    match unique {
                        None => {
                            unique = Some(t);
                            continue;
                        }
                        Some(u) if u == t => continue,
                        Some(u) => current = Some(crate::ops::normalize(u, self.table, self.core)),
                    }
                }
                let nt = crate::ops::normalize(t, self.table, self.core);
                match crate::ops::top_merge(self.table, self.core, current.expect("posto acima"), nt) {
                    Some(r) => current = Some(r),
                    None => error = true,
                }
            }
            targets.push(current.or(unique)?);
        }
        let mut gi = crate::constraints::GenericInferrer::new(&params);
        let types = {
            let mut env = crate::subtyping::SubtypeEnv::new(self.table, the_hierarchy, self.core);
            for (k, &(src, _)) in constraints.iter().enumerate() {
                gi.constrain_return(src, targets[k], &mut env);
                gi.constrain_return(targets[k], src, &mut env);
            }
            gi.choose_final(&mut env)
        };
        let map: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(types.iter().copied()).collect();
        for (k, &(src, _)) in constraints.iter().enumerate() {
            if substitute(src, &map, self.table) != targets[k] {
                return None;
            }
        }
        let the_new_ = self.table.intern(Type::Interface { class: m, args: types.into_boxed_slice(), nullable });
        (the_new_ != mt).then_some(the_new_)
    }

    fn resolve_extensions(&mut self) -> Vec<ExtensionTypeData> {
        let mut data = Vec::with_capacity(self.program.extensions.len());
        for (i, ext) in self.program.extensions.iter().enumerate() {
            let params = self.extension_type_params[i].clone();
            let mut scope = HashMap::with_capacity(params.len());
            for (p_elem, &pid) in ext.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
            // Limites escritos (`extension X<T extends Enum> on ...`).
            for (p_elem, &pid) in ext.type_params.iter().zip(params.iter()) {
                if let Some((unit_id, ast_ty_id)) = p_elem.bound {
                    let b = self.resolve_annotation(unit_id, ast_ty_id, ext.library, &scope);
                    self.table.set_type_param_bound(pid, b);
                }
            }
            let on_ty = self.resolve_annotation(ext.on.0, ext.on.1, ext.library, &scope);
            data.push(ExtensionTypeData {
                type_params: params,
                on: on_ty,
            });
        }
        data
    }

    fn resolve_variables(&mut self) -> Vec<VariableTypeData> {
        let mut data = Vec::with_capacity(self.program.variables.len());
        for var in self.program.variables.iter() {
            let (declared, inferred) = match var.node {
                VariableRef::TopLevel { unit, decl, .. } => {
                    let decl_node = self.program.unit(unit).ast.decl(decl);
                    if let DeclKind::Variables(var_list) = &decl_node.kind {
                        if let Some(ast_ty) = var_list.ty {
                            let ty =
                                self.resolve_annotation(unit, ast_ty, var.library, &HashMap::new());
                            (Some(ty), Some(ty))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                }
                VariableRef::Field { unit, member, .. } => {
                    let mem_node = &self.program.unit(unit).ast.members[member.0 as usize];
                    if let MemberKind::Field(var_list) = &mem_node.kind {
                        let scope = self.get_enclosing_type_param_scope(var.class, var.extension);
                        if let Some(ast_ty) = var_list.ty {
                            self.container = container_of(var.class, var.extension);
                            self.static_member = var.static_;
                            let ty = self.resolve_annotation(unit, ast_ty, var.library, &scope);
                            self.container = None;
                            self.static_member = false;
                            (Some(ty), Some(ty))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                }
                VariableRef::EnumConstant { unit, decl, index } => {
                    if let Some(cls) = var.class {
                        // Generic enum: `a<int>()` gives `E<int>`; without written
                        // arguments, the bounds (inference from the constructor's
                        // arguments is left out). Without this the type came out as `E`
                        // without arguments and the UP of two constants leaked `E<T>`.
                        let n = self.class_type_params[cls.0 as usize].len();
                        let written_ones: Vec<ast::TypeId> = match &self.program.unit(unit).ast.decl(decl).kind {
                            DeclKind::Enum(en) => en.constants.get(index).map(|k| k.type_args.to_vec()).unwrap_or_default(),
                            _ => Vec::new(),
                        };
                        let with_arguments = match &self.program.unit(unit).ast.decl(decl).kind {
                            DeclKind::Enum(en) => en.constants.get(index).is_some_and(|k| k.arguments.is_some()),
                            _ => false,
                        };
                        // An enum constant is implicitly typed: the type is
                        // that of the inferred creation `E(args)` (the type
                        // arguments from the arguments, `enum E<T> { v('') }`
                        // gives `E<String>`). A generic one, without written
                        // type arguments and with arguments, is left to
                        // inference (`functions.rs::infer_untyped_variable_type`).
                        if n > 0 && written_ones.is_empty() && with_arguments {
                            (None, None)
                        } else {
                        let args: Vec<TypeId> = if n == 0 {
                            Vec::new()
                        } else if written_ones.len() == n {
                            // In the enum's scope: `enum E<T> { v<T>() }` sees the
                            // `T` (3.6.2 reports nothing).
                            let the_scope = self.get_enclosing_type_param_scope(var.class, var.extension);
                            written_ones.iter().map(|&t| self.resolve_annotation(unit, t, var.library, &the_scope)).collect()
                        } else {
                            let formals = self.class_type_params[cls.0 as usize].clone();
                            self.instantiate_to_bounds(&formals)
                        };
                        let ty = self.table.intern(Type::Interface {
                            class: cls,
                            args: args.into_boxed_slice(),
                            nullable: false,
                        });
                        (Some(ty), Some(ty))
                        }
                    } else {
                        (None, None)
                    }
                }
                VariableRef::Representation { unit, decl } => {
                    let decl_node = self.program.unit(unit).ast.decl(decl);
                    if let DeclKind::ExtensionType(ext) = &decl_node.kind {
                        // The representation is resolved before the instance
                        // scope (`an611:src/summary2/reference_resolver.dart:204-207`).
                        let scope = self.get_enclosing_type_param_scope(var.class, None);
                        let ty = self.resolve_annotation(
                            unit,
                            ext.representation_type,
                            var.library,
                            &scope,
                        );
                        (Some(ty), Some(ty))
                    } else {
                        (None, None)
                    }
                }
                VariableRef::None => (None, None),
            };

            data.push(VariableTypeData {
                declared_type: declared,
                inferred,
            });
        }
        data
    }

    fn resolve_functions(
        &mut self,
        hierarchy: &ClassHierarchy,
        variables: &mut [VariableTypeData],
    ) -> Vec<FunctionTypeData> {
        let mut data: Vec<FunctionTypeData> = Vec::with_capacity(self.program.functions.len());
        let augmented = self.augmented_declarations();

        for (i, func) in self.program.functions.iter().enumerate() {
            let func_id = FunctionElementId(i as u32);
            let (mut sig, mut ret, mut params, tparams) =
                self.resolve_function_signature(func_id, func, hierarchy, variables);
            if let Some(&previous) = augmented.get(&func_id)
                && let Some(inherited) = data.get(previous.0 as usize)
                && let Some((s2, r2, p2)) = self.inherit_signature(func, inherited, &params, ret, &tparams)
            {
                (sig, ret, params) = (s2, r2, p2);
            }

            // If it is an implicit variable accessor, sync it if needed
            if let Some(var_id) = func.variable {
                let var_data = &mut variables[var_id.0 as usize];
                if func.kind == FunctionKind::Getter && var_data.declared_type.is_none() {
                    var_data.inferred = Some(ret);
                }
            }

            data.push(FunctionTypeData {
                signature: sig,
                return_type: ret,
                type_params: tparams,
                parameters: params,
            });
        }

        data
    }

    /// For each function of an augmentation in user code (not the SDK
    /// patches), the declaration it augments: the first in the chain
    /// (docs/AUGMENTATIONS.md §2: the previous elements point, through
    /// `patched_by`, to the effective one).
    fn augmented_declarations(&self) -> HashMap<FunctionElementId, FunctionElementId> {
        let mut m: HashMap<FunctionElementId, FunctionElementId> = HashMap::new();
        for (i, f) in self.program.functions.iter().enumerate() {
            let Some(effective) = f.patched_by else { continue };
            if self.program.library(f.library).is_sdk {
                continue;
            }
            let current = FunctionElementId(i as u32);
            m.entry(effective)
                .and_modify(|x| {
                    if current.0 < x.0 {
                        *x = current;
                    }
                })
                .or_insert(current);
        }
        m
    }

    /// Types omitted in an augmentation come from the augmented declaration
    /// (augmentations spec, "Augmenting functions": the signature may omit
    /// types, which take those of the previous declaration). Positional
    /// parameters by position, named ones by name; the return, if omitted.
    /// Generic functions (on either side) stay as they are: inheriting would
    /// require substituting the type parameters. Checked against the CFE and
    /// analyzer 3.13.4 (`augment f(x) => …` with `String f(int x);` before).
    fn inherit_signature(
        &mut self,
        func: &FunctionElement,
        previous: &FunctionTypeData,
        params: &[ParameterTypeData],
        ret: TypeId,
        tparams: &[TypeParamId],
    ) -> Option<(TypeId, TypeId, Box<[ParameterTypeData]>)> {
        let FunctionRef::Function { unit, function } = func.node else { return None };
        if !tparams.is_empty() || !previous.type_params.is_empty() {
            return None;
        }
        let ast_func = &self.program.unit(unit).ast.functions[function.0 as usize];
        let written_ones: Vec<bool> = match &ast_func.parameters {
            Some(ps) => ps.iter().map(|p| p.ty.is_some() || p.function_parameters.is_some()).collect(),
            None => Vec::new(),
        };
        if written_ones.len() != params.len() {
            return None;
        }
        let mut new_: Vec<ParameterTypeData> = params.to_vec();
        let positional_before: Vec<&ParameterTypeData> =
            previous.parameters.iter().filter(|p| p.kind != ParameterKind::Named).collect();
        let mut position = 0usize;
        for (i, p) in new_.iter_mut().enumerate() {
            let corresponding = if p.kind == ParameterKind::Named {
                previous.parameters.iter().find(|q| q.kind == ParameterKind::Named && q.externo == p.externo)
            } else {
                let q = positional_before.get(position).copied();
                position += 1;
                q
            };
            if !written_ones[i]
                && let Some(q) = corresponding
            {
                p.ty = q.ty;
            }
        }
        let new_ret = if ast_func.return_type.is_none() && func.kind != FunctionKind::Setter {
            previous.return_type
        } else {
            ret
        };
        let mut positional = Vec::new();
        let mut optional = Vec::new();
        let mut named = Vec::new();
        for p in &new_ {
            match p.kind {
                ParameterKind::Required => positional.push(p.ty),
                ParameterKind::Optional => optional.push(p.ty),
                ParameterKind::Named => {
                    if let Some(n) = p.externo {
                        named.push((n, p.ty, p.required));
                    }
                }
            }
        }
        let sig = self.table.intern(Type::Function {
            type_params: Box::new([]),
            ret: new_ret,
            positional: positional.into_boxed_slice(),
            optional: optional.into_boxed_slice(),
            named: named.into_boxed_slice(),
            nullable: false,
        });
        Some((sig, new_ret, new_.into_boxed_slice()))
    }

    /// A member's signature is resolved with its container's instance scope
    /// (`InstanceScope`).
    fn resolve_function_signature(
        &mut self,
        func_id: FunctionElementId,
        func: &FunctionElement,
        hierarchy: &ClassHierarchy,
        variables: &[VariableTypeData],
    ) -> (TypeId, TypeId, Box<[ParameterTypeData]>, Box<[TypeParamId]>) {
        self.container = container_of(func.class, func.extension);
        self.static_member = func.static_
            && !func.factory
            && func.kind != FunctionKind::Constructor
            && (func.class.is_some() || func.extension.is_some());
        let r = self.resolve_function_signature_in_scope(func_id, func, hierarchy, variables);
        self.container = None;
        self.static_member = false;
        r
    }

    fn resolve_function_signature_in_scope(
        &mut self,
        func_id: FunctionElementId,
        func: &FunctionElement,
        hierarchy: &ClassHierarchy,
        variables: &[VariableTypeData],
    ) -> (TypeId, TypeId, Box<[ParameterTypeData]>, Box<[TypeParamId]>) {
        let mut scope = self.get_enclosing_type_param_scope(func.class, func.extension);

        match func.node {
            FunctionRef::Function { unit, function } => {
                let ast_func = &self.program.unit(unit).ast.functions[function.0 as usize];

                // Type parameters of the generic function
                let mut func_type_params = Vec::with_capacity(ast_func.type_params.len());
                for tp in ast_func.type_params.iter() {
                    let pid = self.table.alloc_type_param(
                        tp.name.sym,
                        TypeParamOwner::Function(func_id),
                        self.core.object_nullable,
                        Variance::Unspecified,
                    );
                    func_type_params.push(pid);
                    scope.insert(tp.name.sym, pid);
                }

                // Bounds of the type parameters
                for (tp, &pid) in ast_func.type_params.iter().zip(func_type_params.iter()) {
                    if let Some(ast_bound) = tp.bound {
                        let b = self.resolve_annotation(unit, ast_bound, func.library, &scope);
                        self.table.set_type_param_bound(pid, b);
                    }
                }

                // Resolution of formal parameters
                let mut param_types = Vec::new();
                let mut positional = Vec::new();
                let mut optional = Vec::new();
                let mut named = Vec::new();

                if let Some(parameters) = &ast_func.parameters {
                    let mut positional_idx = 0usize;
                    for p in parameters.iter() {
                        let current_pos = positional_idx;
                        if p.kind != ParameterKind::Named {
                            positional_idx += 1;
                        }
                        let p_name = p.name.as_ref().map(|n| n.sym);
                        let p_ty = if p.function_parameters.is_some() {
                            self.resolve_function_typed_parameter(unit, p, func.library, &scope)
                        } else if let Some(ast_ty) = p.ty {
                            self.resolve_annotation(unit, ast_ty, func.library, &scope)
                        } else {
                            // Try override inference if it is an instance method
                            self.infer_override_parameter_type(func, p_name, p.kind, current_pos, hierarchy, 0)
                                .unwrap_or_else(|| {
                                    if func.kind == FunctionKind::Setter {
                                        self.defer_field_override(func_id, func, Some(param_types.len()), hierarchy);
                                    }
                                    self.core.dynamic_
                                })
                        };

                        param_types.push(ParameterTypeData {
                            name: p_name,
                            externo: p.external_name().map(|n| n.sym),
                            ty: p_ty,
                            required: p.required,
                            kind: p.kind,
                        });

                        match p.kind {
                            ParameterKind::Required => positional.push(p_ty),
                            ParameterKind::Optional => optional.push(p_ty),
                            ParameterKind::Named => {
                                if let Some(n) = p.external_name() {
                                    named.push((n.sym, p_ty, p.required));
                                }
                            }
                        }
                    }
                }

                // Function return
                let ret_ty = if let Some(ast_ret) = ast_func.return_type {
                    self.resolve_annotation(unit, ast_ret, func.library, &scope)
                } else if func.kind == FunctionKind::Setter {
                    self.core.void_
                } else {
                    // Try override inference for the return
                    self.infer_override_return_type(func, hierarchy, 0).unwrap_or_else(|| {
                        if func.kind == FunctionKind::Getter {
                            self.defer_field_override(func_id, func, None, hierarchy);
                        }
                        self.core.dynamic_
                    })
                };

                let sig = self.table.intern(Type::Function {
                    type_params: func_type_params.clone().into_boxed_slice(),
                    ret: ret_ty,
                    positional: positional.into_boxed_slice(),
                    optional: optional.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                });

                (
                    sig,
                    ret_ty,
                    param_types.into_boxed_slice(),
                    func_type_params.into_boxed_slice(),
                )
            }
            FunctionRef::Constructor { unit, member } => {
                let mem_node = &self.program.unit(unit).ast.members[member.0 as usize];
                let ctor = match &mem_node.kind {
                    MemberKind::Constructor(c) => c,
                    _ => unreachable!(),
                };

                let mut param_types = Vec::new();
                let mut positional = Vec::new();
                let mut optional = Vec::new();
                let mut named = Vec::new();

                for p in ctor.parameters.iter() {
                    let p_name = p.name.as_ref().map(|n| n.sym);
                    let p_ty = if p.function_parameters.is_some() {
                        self.resolve_function_typed_parameter(unit, p, func.library, &scope)
                    } else if let Some(ast_ty) = p.ty {
                        self.resolve_annotation(unit, ast_ty, func.library, &scope)
                    } else if p.this_ {
                        self.field_type_for_this_param(func, p_name).unwrap_or(self.core.dynamic_)
                    } else if p.super_ {
                        self.super_param_type(func, ctor, p, 0, hierarchy).unwrap_or(self.core.dynamic_)
                    } else {
                        self.core.dynamic_
                    };

                    param_types.push(ParameterTypeData {
                        name: p_name,
                        externo: p.external_name().map(|n| n.sym),
                        ty: p_ty,
                        required: p.required,
                        kind: p.kind,
                    });

                    match p.kind {
                        ParameterKind::Required => positional.push(p_ty),
                        ParameterKind::Optional => optional.push(p_ty),
                        ParameterKind::Named => {
                            if let Some(n) = p.external_name() {
                                named.push((n.sym, p_ty, p.required));
                            }
                        }
                    }
                }

                // A constructor instantiates the owner class with its own type parameters
                let ret_ty = self.instantiate_self_class(func.class);

                let sig = self.table.intern(Type::Function {
                    type_params: Box::new([]),
                    ret: ret_ty,
                    positional: positional.into_boxed_slice(),
                    optional: optional.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                });

                (sig, ret_ty, param_types.into_boxed_slice(), Box::new([]))
            }
            FunctionRef::None => {
                // Synthetic members or implicit accessors
                if let Some(var_id) = func.variable {
                    let var_data = &variables[var_id.0 as usize];
                    let var_ty = var_data
                        .declared_type
                        .or(var_data.inferred)
                        .unwrap_or(self.core.dynamic_);

                    let is_getter = func.kind == FunctionKind::Getter
                        || self.program.variable(var_id).getter == Some(func_id);
                    if is_getter {
                        let sig = self.table.intern(Type::Function {
                            type_params: Box::new([]),
                            ret: var_ty,
                            positional: Box::new([]),
                            optional: Box::new([]),
                            named: Box::new([]),
                            nullable: false,
                        });
                        (sig, var_ty, Box::new([]), Box::new([]))
                    } else {
                        // Setter
                        let param = ParameterTypeData {
                            name: None,
                            externo: None,
                            ty: var_ty,
                            required: true,
                            kind: ParameterKind::Required,
                        };
                        let sig = self.table.intern(Type::Function {
                            type_params: Box::new([]),
                            ret: self.core.void_,
                            positional: Box::new([var_ty]),
                            optional: Box::new([]),
                            named: Box::new([]),
                            nullable: false,
                        });
                        (sig, self.core.void_, Box::new([param]), Box::new([]))
                    }
                } else if func.kind == FunctionKind::Getter {
                    // Synthetic enum getters: `values`, `index`, `name`.
                    let name = self.interner.resolve(func.name);
                    let ret = if name == "values" {
                        let elem = self.instantiate_self_class(func.class);
                        match self.core.list_class {
                            Some(l) => self.table.intern(Type::Interface { class: l, args: Box::new([elem]), nullable: false }),
                            None => self.core.dynamic_,
                        }
                    } else if name == "index" {
                        self.core.int
                    } else {
                        self.core.string
                    };
                    let sig = self.table.intern(Type::Function {
                        type_params: Box::new([]),
                        ret,
                        positional: Box::new([]),
                        optional: Box::new([]),
                        named: Box::new([]),
                        nullable: false,
                    });
                    (sig, ret, Box::new([]), Box::new([]))
                } else {
                    // Default synthetic constructor
                    let ret_ty = self.instantiate_self_class(func.class);
                    let sig = self.table.intern(Type::Function {
                        type_params: Box::new([]),
                        ret: ret_ty,
                        positional: Box::new([]),
                        optional: Box::new([]),
                        named: Box::new([]),
                        nullable: false,
                    });
                    (sig, ret_ty, Box::new([]), Box::new([]))
                }
            }
        }
    }

    /// Arguments of a class used in an annotation: the written ones, or the
    /// instantiation to bounds when the class is used raw (`Map` is
    /// `Map<dynamic, dynamic>`, `C` with `T extends num` is `C<num>`).
    fn args_or_bounds(&mut self, cid: ClassId, args: Vec<TypeId>) -> Vec<TypeId> {
        let formals = self.class_type_params[cid.0 as usize].clone();
        if args.len() == formals.len() {
            return args;
        }
        self.instantiate_to_bounds(&formals)
    }

    /// Instantiation to bounds (`instantiate to bounds`): the written bound, or
    /// `dynamic`; bounds that mention their own parameters (F-bounds) have
    /// those parameters replaced by `dynamic`.
    fn instantiate_to_bounds(&mut self, formals: &[TypeParamId]) -> Vec<TypeId> {
        crate::ops::instantiate_to_bounds(formals, &[], self.table, self.core)
    }

    fn instantiate_self_class(&mut self, class_opt: Option<ClassId>) -> TypeId {
        if let Some(cls) = class_opt {
            let params = self.class_type_params[cls.0 as usize].clone();
            let args: Vec<TypeId> = params
                .iter()
                .map(|&p| {
                    self.table.intern(Type::TypeParameter {
                        param: p,
                        nullable: false,
                    })
                })
                .collect();
            let is_ext = self.program.class(cls).kind == ClassKind::ExtensionType;
            if is_ext {
                self.table.intern(Type::ExtensionType {
                    decl: cls,
                    args: args.into_boxed_slice(),
                    nullable: false,
                })
            } else {
                self.table.intern(Type::Interface {
                    class: cls,
                    args: args.into_boxed_slice(),
                    nullable: false,
                })
            }
        } else {
            self.core.dynamic_
        }
    }

    /// Same-named member in the supertypes of `func`'s class, in lookup order
    /// (superclasses and mixins, then interfaces): `(the_class, função)`.
    fn overlapping_member(&self, func: &FunctionElement, hierarchy: &ClassHierarchy) -> Option<(ClassId, FunctionElementId)> {
        let class_id = func.class?;
        if func.static_ {
            return None;
        }
        let the_key = if func.kind == FunctionKind::Setter {
            self.interner.lookup(&format!("{}_=", self.interner.resolve(func.name)))?
        } else {
            func.name
        };
        for (sup, _) in crate::scope::ordered_supertypes(self.program, hierarchy, class_id) {
            if let Some(&f) = self.program.class(sup).instance_members.get(&the_key) {
                return Some((sup, f));
            }
        }
        None
    }

    /// Substitutes the parameters of superclass `sup` with what `func`'s class
    /// passes to it.
    fn instantiate_from_super(&mut self, func: &FunctionElement, sup: ClassId, t: TypeId, hierarchy: &ClassHierarchy) -> Option<TypeId> {
        let class_id = func.class?;
        let this = self.instantiate_self_class(Some(class_id));
        let super_ty = hierarchy.supertype_of(this, sup, self.table, self.core)?;
        let args = match self.table.get(super_ty).clone() {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args,
            _ => return None,
        };
        let params = self.class_type_params[sup.0 as usize].clone();
        let subst: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
        Some(substitute(t, &subst, self.table))
    }

    /// Type of field `v` written in the declaration, in the scope of class
    /// `sup`.
    fn written_field_type(&mut self, v: dartforge_elements::model::VariableId, sup: ClassId) -> Option<TypeId> {
        let (unit, ast_ty) = match self.program.variable(v).node {
            VariableRef::Field { unit, member, .. } => match &self.program.unit(unit).ast.member(member).kind {
                MemberKind::Field(vl) => (unit, vl.ty?),
                _ => return None,
            },
            _ => return None,
        };
        let lib = self.program.variable(v).library;
        let scope = self.get_enclosing_type_param_scope(Some(sup), None);
        Some(self.resolve_annotation(unit, ast_ty, lib, &scope))
    }

    /// Records the override whose overridden member is the implicit accessor of
    /// a field without a written type: the type comes from the initializer's
    /// inference ([`OutlineTypes::field_overrides`]).
    fn defer_field_override(
        &mut self,
        func_id: FunctionElementId,
        func: &FunctionElement,
        parameter: Option<usize>,
        hierarchy: &ClassHierarchy,
    ) {
        let Some((sup, sf)) = self.overlapping_member(func, hierarchy) else { return };
        let super_func = self.program.function(sf);
        if !matches!(super_func.node, FunctionRef::None) {
            return;
        }
        let Some(v) = super_func.variable else { return };
        if !matches!(self.program.variable(v).node, VariableRef::Field { .. }) {
            return;
        }
        let Some(class_id) = func.class else { return };
        let this = self.instantiate_self_class(Some(class_id));
        let Some(super_ty) = hierarchy.supertype_of(this, sup, self.table, self.core) else { return };
        let args = match self.table.get(super_ty) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.clone(),
            _ => return,
        };
        let params = self.class_type_params[sup.0 as usize].clone();
        let subst = params.iter().copied().zip(args.iter().copied()).collect();
        self.field_overrides.push(FieldOverride { function: func_id, field: v, subst, parameter });
    }

    /// Inherited return type (override inference): that of the overridden
    /// member, written or, if omitted there too, inherited recursively.
    fn infer_override_return_type(&mut self, func: &FunctionElement, hierarchy: &ClassHierarchy, prof: u32) -> Option<TypeId> {
        if prof > 16 {
            return None;
        }
        let (sup, sf) = self.overlapping_member(func, hierarchy)?;
        let super_func = self.program.function(sf);
        let t = match (super_func.kind, super_func.node) {
            (FunctionKind::ImplicitAccessor, _) => {
                let v = super_func.variable?;
                self.written_field_type(v, sup)?
            }
            (_, FunctionRef::Function { unit, function }) => {
                let ast_func = &self.program.unit(unit).ast.functions[function.0 as usize];
                if !ast_func.type_params.is_empty() {
                    return None;
                }
                match ast_func.return_type {
                    Some(r) => {
                        let scope = self.get_enclosing_type_param_scope(Some(sup), None);
                        self.resolve_annotation(unit, r, super_func.library, &scope)
                    }
                    None => {
                        let t = self.infer_override_return_type(super_func, hierarchy, prof + 1)?;
                        return self.instantiate_from_super(func, sup, t, hierarchy).or(Some(t));
                    }
                }
            }
            _ => return None,
        };
        self.instantiate_from_super(func, sup, t, hierarchy)
    }

    /// Inherited type of a parameter: a named one by name, a positional one by
    /// position, in the overridden member (recursive if omitted there too).
    fn infer_override_parameter_type(
        &mut self,
        func: &FunctionElement,
        param_name: Option<SymbolId>,
        kind: ParameterKind,
        position: usize,
        hierarchy: &ClassHierarchy,
        prof: u32,
    ) -> Option<TypeId> {
        if prof > 16 {
            return None;
        }
        let (sup, sf) = self.overlapping_member(func, hierarchy)?;
        let super_func = self.program.function(sf);
        let t = match super_func.node {
            FunctionRef::Function { unit, function } => {
                let ast_func = &self.program.unit(unit).ast.functions[function.0 as usize];
                if !ast_func.type_params.is_empty() {
                    return None;
                }
                let params = ast_func.parameters.as_ref()?;
                let tgt = if kind == ParameterKind::Named {
                    params.iter().find(|p| p.kind == ParameterKind::Named && p.name.map(|n| n.sym) == param_name)?
                } else {
                    params.iter().filter(|p| p.kind != ParameterKind::Named).nth(position)?
                };
                if tgt.function_parameters.is_some() {
                    let scope = self.get_enclosing_type_param_scope(Some(sup), None);
                    self.resolve_function_typed_parameter(unit, tgt, super_func.library, &scope)
                } else if let Some(ast_ty) = tgt.ty {
                    let scope = self.get_enclosing_type_param_scope(Some(sup), None);
                    self.resolve_annotation(unit, ast_ty, super_func.library, &scope)
                } else {
                    let t = self.infer_override_parameter_type(super_func, param_name, kind, position, hierarchy, prof + 1)?;
                    return self.instantiate_from_super(func, sup, t, hierarchy).or(Some(t));
                }
            }
            FunctionRef::None if func.kind == FunctionKind::Setter => {
                let v = super_func.variable?;
                self.written_field_type(v, sup)?
            }
            _ => return None,
        };
        self.instantiate_from_super(func, sup, t, hierarchy)
    }

    fn get_enclosing_type_param_scope(
        &self,
        class: Option<ClassId>,
        ext: Option<ExtensionId>,
    ) -> HashMap<SymbolId, TypeParamId> {
        let mut scope = HashMap::new();
        if let Some(cls) = class {
            let elem = self.program.class(cls);
            let params = &self.class_type_params[cls.0 as usize];
            for (p_elem, &pid) in elem.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
        } else if let Some(e) = ext {
            let elem = self.program.extension(e);
            let params = &self.extension_type_params[e.0 as usize];
            for (p_elem, &pid) in elem.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
        }
        scope
    }

    fn resolve_annotation(
        &mut self,
        unit_id: UnitId,
        ast_ty_id: ast::TypeId,
        library: LibraryId,
        type_param_scope: &HashMap<SymbolId, TypeParamId>,
    ) -> TypeId {
        let t = self.resolve_written_annotation(unit_id, ast_ty_id, library, type_param_scope);
        // `TypeAnnotation.type`, for the rules that read it.
        self.tipos_escritos.insert((unit_id, ast_ty_id), t);
        // `ResolutionVisitor.visitNamedType` resolves the type arguments before
        // the name, whatever it is (type parameter, `dynamic`, undefined name or
        // one that is not a type): those that the name path did not resolve are
        // resolved here, with their errors.
        let args: Vec<ast::TypeId> = match &self.program.unit(unit_id).ast.ty(ast_ty_id).kind {
            ast::TypeKind::Named { args, .. } => args.to_vec(),
            _ => Vec::new(),
        };
        for a in args {
            if !self.tipos_escritos.contains_key(&(unit_id, a)) {
                self.type_context = TypeContext::TypeArgument;
                self.resolve_annotation(unit_id, a, library, type_param_scope);
            }
        }
        t
    }

    fn resolve_written_annotation(
        &mut self,
        unit_id: UnitId,
        ast_ty_id: ast::TypeId,
        library: LibraryId,
        type_param_scope: &HashMap<SymbolId, TypeParamId>,
    ) -> TypeId {
        let annot = self.program.unit(unit_id).ast.ty(ast_ty_id);
        let is_nullable = annot.nullable;
        let span = annot.span;
        let context = std::mem::take(&mut self.type_context);

        match &annot.kind {
            ast::TypeKind::Void => self.core.void_,
            ast::TypeKind::Named { name, args } => {
                // The name (with the prefix) is the range of the name errors; the whole
                // type, that of the argument count.
                let range = dartforge_diagnostics::Span { start: name[0].span.start, end: name[name.len() - 1].span.end };
                let text = self.interner.resolve(name[name.len() - 1].sym).to_string();
                if name.len() == 1 {
                    let sym = name[0].sym;

                    // 1. Check the scope of the current type parameters
                    if let Some(&param_id) = type_param_scope.get(&sym) {
                        if !args.is_empty() {
                            self.warn(unit_id, type_arguments_diagnostic(&text, 0, args.len(), span));
                        }
                        if self.static_member && class_param(self.table, param_id) {
                            self.warn(unit_id, parameter_in_static_diagnostic(range));
                        }
                        return self.table.intern(Type::TypeParameter {
                            param: param_id,
                            nullable: is_nullable,
                        });
                    }

                    // 2. Check the system's special types (`dynamic<int>`,
                    // `Never<int>`: `_buildTypeArguments`'s wrong count, on the
                    // whole type). `dynamic` and `Never` come from the `dart:core`
                    // namespace: with core only through a prefix, they are undefined.
                    let special = self.interner.lookup("dynamic") == Some(sym) || self.interner.lookup("Never") == Some(sym);
                    if special && !self.visible_core(unit_id, sym) {
                        if !crate::scope::should_ignore_undefined(self.program, self.interner, unit_id, None, sym) {
                            self.warn_type_name(unit_id, context, false, &text, range);
                        }
                        return self.table.invalid(self.core.dynamic_);
                    }
                    let builtin = self.interner.lookup("dynamic") == Some(sym) || self.interner.lookup("Never") == Some(sym);
                    if builtin && !args.is_empty() {
                        self.warn(unit_id, type_arguments_diagnostic(&text, 0, args.len(), span));
                    }
                    if self.interner.lookup("dynamic") == Some(sym) {
                        return self.core.dynamic_;
                    }
                    if self.interner.lookup("void") == Some(sym) {
                        return self.core.void_;
                    }
                    if self.interner.lookup("Never") == Some(sym) {
                        return if is_nullable {
                            self.table.decorate(self.core.null, crate::table::Exibicao::NeverAnulavel)
                        } else {
                            self.core.never
                        };
                    }
                    if self.interner.lookup("Null") == Some(sym) {
                        return self.core.null;
                    }
                    // `FutureOr` is only `dart:async`'s when it is in scope
                    // (`dart:core` does not export it): without an import, the name is
                    // undefined like any other.
                    let futureor_in_scope = matches!(
                        self.program.lookup_in_unit(unit_id, sym).and_then(|b| b.getter),
                        Some(Element::Class(c)) if self.program.library(self.program.class(c).library).uri == "dart:async"
                    );
                    if self.interner.lookup("FutureOr") == Some(sym) && futureor_in_scope {
                        if args.len() == 1 {
                            let arg_ty = self.resolve_annotation(
                                unit_id,
                                args[0],
                                library,
                                type_param_scope,
                            );
                            return self.table.intern(Type::FutureOr {
                                arg: arg_ty,
                                nullable: is_nullable,
                            });
                        } else if args.is_empty() {
                            // Raw `FutureOr`: instantiated to the bound.
                            return self.table.intern(Type::FutureOr {
                                arg: self.core.dynamic_,
                                nullable: is_nullable,
                            });
                        } else {
                            self.warn(unit_id, type_arguments_diagnostic(&text, 1, args.len(), span));
                            return self.core.dynamic_;
                        }
                    }

                    // 3. The container's instance scope hides the top-level one.
                    // (Synthetic recovery name, without width: nothing.)
                    match if span.start == span.end { None } else { self.in_container(sym) } {
                        Some(InContainer::Getter) => {
                            // The linker's `NamedTypeBuilder` gives `dynamic` (not
                            // `InvalidType`) to an element that is not a type
                            // (`named_type_builder.dart:121`); `InvalidType` is only
                            // for a name not found (`reference_resolver.dart:424`).
                            // `int get foo` with an `int` field in the class: the getter
                            // is `dynamic`, which is not a subtype of `int`.
                            self.warn_type_name(unit_id, context, true, &text, range);
                            return self.core.dynamic_;
                        }
                        Some(InContainer::OnlySetter) => {
                            // The linker does not find the type (invalid, without a
                            // warning); `ResolutionVisitor`, which reports, finds the
                            // outer one.
                            return self.table.invalid(self.core.dynamic_);
                        }
                        None => {}
                    }

                    // 4. Resolution in the library scope
                    let binding = self.program.lookup_in_unit(unit_id, sym);
                    match binding {
                        Some(b) => {
                            if b.ambiguous {
                                let the_list = crate::scope::ambiguous_libraries(self.program, unit_id, None, sym);
                                self.warn(
                                    unit_id,
                                    Diagnostic::with_code(dartforge_diagnostics::codes::compile_time_error::AMBIGUOUS_IMPORT, name[0].span, [text.as_str(), the_list.as_str()]),
                                );
                                return self.table.invalid(self.core.dynamic_);
                            }
                            match b.getter {
                                Some(Element::Class(cid)) => {
                                    let resolved_args: Vec<TypeId> = args
                                        .iter()
                                        .map(|&a| {
                                            self.type_context = TypeContext::TypeArgument;
                                            self.resolve_annotation(
                                                unit_id,
                                                a,
                                                library,
                                                type_param_scope,
                                            )
                                        })
                                        .collect();

                                    let n_params = self.class_type_params[cid.0 as usize].len();
                                    if !args.is_empty() && args.len() != n_params {
                                        self.warn(unit_id, type_arguments_diagnostic(&text, n_params, args.len(), span));
                                    }
                                    // Wrong number of type arguments: all of them become invalid
                                    // (`_buildTypeArguments`, `named_type_resolver.dart:148`).
                                    let resolved_args = if !args.is_empty() && args.len() != n_params {
                                        vec![self.table.invalid(self.core.dynamic_); n_params]
                                    } else {
                                        self.args_or_bounds(cid, resolved_args)
                                    };
                                    let is_ext =
                                        self.program.class(cid).kind == ClassKind::ExtensionType;
                                    if is_ext {
                                        self.table.intern(Type::ExtensionType {
                                            decl: cid,
                                            args: resolved_args.into_boxed_slice(),
                                            nullable: is_nullable,
                                        })
                                    } else {
                                        self.table.intern(Type::Interface {
                                            class: cid,
                                            args: resolved_args.into_boxed_slice(),
                                            nullable: is_nullable,
                                        })
                                    }
                                }
                                Some(Element::Typedef(tid)) if self.self_referencing_typedef(tid) => self.core.dynamic_,
                                Some(Element::Typedef(tid)) => {
                                    let resolved_args: Vec<TypeId> = args
                                        .iter()
                                        .map(|&a| {
                                            self.type_context = TypeContext::TypeArgument;
                                            self.resolve_annotation(
                                                unit_id,
                                                a,
                                                library,
                                                type_param_scope,
                                            )
                                        })
                                        .collect();

                                    let target_ty = self.ensure_typedef_resolved(tid);
                                    let formals = self.typedef_type_params[tid.0 as usize].clone();
                                    if !args.is_empty() && args.len() != formals.len() {
                                        self.warn(unit_id, type_arguments_diagnostic(&text, formals.len(), args.len(), span));
                                    }
                                    let resolved_args = if resolved_args.len() == formals.len() {
                                        resolved_args
                                    } else if !args.is_empty() {
                                        // Wrong number of type arguments: all invalid.
                                        vec![self.table.invalid(self.core.dynamic_); formals.len()]
                                    } else {
                                        self.instantiate_to_bounds(&formals)
                                    };
                                    let mut subst = HashMap::with_capacity(formals.len());
                                    for (&f, &a) in formals.iter().zip(resolved_args.iter()) {
                                        subst.insert(f, a);
                                    }
                                    let expanded = substitute(target_ty, &subst, self.table);
                                    let expanded = self.table.decorate(
                                        expanded,
                                        crate::table::Exibicao::Alias { typedef: tid, args: resolved_args.clone().into_boxed_slice() },
                                    );
                                    if is_nullable {
                                        nullable(expanded, self.table)
                                    } else {
                                        expanded
                                    }
                                }
                                // An element that is not a type: `dynamic` in the linker;
                                // only a setter (null getter) is a name not found.
                                g => {
                                    self.warn_type_name(unit_id, context, true, &text, range);
                                    if g.is_some() { self.core.dynamic_ } else { self.table.invalid(self.core.dynamic_) }
                                }
                            }
                        }
                        None => {
                            // Synthetic name from parser recovery (empty, without
                            // width): the analyzer does not report it; nor what may
                            // come from an import or part that does not exist
                            // (`shouldIgnoreUndefinedNamedType`).
                            if span.start != span.end
                                && !crate::scope::should_ignore_undefined(self.program, self.interner, unit_id, None, name[0].sym)
                            {
                                self.warn_type_name(unit_id, context, false, &text, range);
                            }
                            self.table.invalid(self.core.dynamic_)
                        }
                    }
                } else if name.len() == 2 {
                    let prefix = name[0].sym;
                    let member = name[1].sym;
                    if name[0].span.start != name[0].span.end {
                        let (the_class, extension) = match self.container {
                            Some(Container::Classe(c)) => (Some(c), None),
                            Some(Container::Extensao(e)) => (None, Some(e)),
                            None => (None, None),
                        };
                        let local = type_param_scope.contains_key(&prefix);
                        if shadowed_prefix(self.program, self.interner, unit_id, prefix, local, the_class, extension) {
                            let the_name = self.interner.resolve(prefix).to_string();
                            self.warn(unit_id, shadowed_prefix_diagnostic(&the_name, name[0].span));
                            return self.table.invalid(self.core.dynamic_);
                        }
                        if matches!(self.in_container(prefix), Some(InContainer::OnlySetter)) {
                            return self.table.invalid(self.core.dynamic_);
                        }
                    }
                    let binding = self.program.lookup_prefixed_in_unit(unit_id, prefix, member);
                    match binding {
                        Some(b) => match b.getter {
                            Some(Element::Class(cid)) => {
                                let resolved_args: Vec<TypeId> = args
                                    .iter()
                                    .map(|&a| {
                                        self.type_context = TypeContext::TypeArgument;
                                        self.resolve_annotation(
                                            unit_id,
                                            a,
                                            library,
                                            type_param_scope,
                                        )
                                    })
                                    .collect();

                                let n_params = self.class_type_params[cid.0 as usize].len();
                                if !args.is_empty() && args.len() != n_params {
                                    self.warn(unit_id, type_arguments_diagnostic(&text, n_params, args.len(), span));
                                }
                                // Wrong number of type arguments: all of them become invalid
                                // (`_buildTypeArguments`, `named_type_resolver.dart:148`).
                                let resolved_args = if !args.is_empty() && args.len() != n_params {
                                    vec![self.table.invalid(self.core.dynamic_); n_params]
                                } else {
                                    self.args_or_bounds(cid, resolved_args)
                                };
                                let is_ext =
                                    self.program.class(cid).kind == ClassKind::ExtensionType;
                                if is_ext {
                                    self.table.intern(Type::ExtensionType {
                                        decl: cid,
                                        args: resolved_args.into_boxed_slice(),
                                        nullable: is_nullable,
                                    })
                                } else {
                                    self.table.intern(Type::Interface {
                                        class: cid,
                                        args: resolved_args.into_boxed_slice(),
                                        nullable: is_nullable,
                                    })
                                }
                            }
                            Some(Element::Typedef(tid)) if self.self_referencing_typedef(tid) => self.core.dynamic_,
                            Some(Element::Typedef(tid)) => {
                                let resolved_args: Vec<TypeId> = args
                                    .iter()
                                    .map(|&a| {
                                        self.type_context = TypeContext::TypeArgument;
                                        self.resolve_annotation(
                                            unit_id,
                                            a,
                                            library,
                                            type_param_scope,
                                        )
                                    })
                                    .collect();

                                let target_ty = self.ensure_typedef_resolved(tid);
                                let formals = self.typedef_type_params[tid.0 as usize].clone();
                                if !args.is_empty() && args.len() != formals.len() {
                                    self.warn(unit_id, type_arguments_diagnostic(&text, formals.len(), args.len(), span));
                                }
                                let resolved_args = if resolved_args.len() == formals.len() {
                                    resolved_args
                                } else if !args.is_empty() {
                                    // Wrong number of type arguments: all invalid.
                                    vec![self.table.invalid(self.core.dynamic_); formals.len()]
                                } else {
                                    self.instantiate_to_bounds(&formals)
                                };
                                let mut subst = HashMap::with_capacity(formals.len());
                                for (&f, &a) in formals.iter().zip(resolved_args.iter()) {
                                    subst.insert(f, a);
                                }
                                let expanded = substitute(target_ty, &subst, self.table);
                                let expanded = self.table.decorate(
                                    expanded,
                                    crate::table::Exibicao::Alias { typedef: tid, args: resolved_args.clone().into_boxed_slice() },
                                );
                                if is_nullable {
                                    nullable(expanded, self.table)
                                } else {
                                    expanded
                                }
                            }
                            _ => {
                                self.warn_type_name(unit_id, context, true, &text, range);
                                self.table.invalid(self.core.dynamic_)
                            }
                        },
                        None => {
                            // `_rewriteToConstructorName` (`named_type_resolver.dart:101-109`,
                            // `:329-382`): the "prefix" is a class or an alias, not an import
                            // prefix (`A.foo` read as `prefixo.Nome`). Outside a creation,
                            // `NOT_A_TYPE` with `A.foo`, from the prefix to the end of the
                            // name, in any context; never the undefined name.
                            let prefix_and_type = matches!(
                                self.program.lookup_in_unit(unit_id, prefix).and_then(|b| b.getter),
                                Some(Element::Class(_) | Element::Typedef(_))
                            );
                            if prefix_and_type {
                                let complete = format!("{}.{}", self.interner.resolve(prefix), text);
                                self.warn(
                                    unit_id,
                                    Diagnostic::with_code(dartforge_diagnostics::codes::compile_time_error::NOT_A_TYPE, range, [complete.as_str()]),
                                );
                            } else if !crate::scope::should_ignore_undefined(self.program, self.interner, unit_id, Some(name[0].sym), name[1].sym) {
                                self.warn_type_name(unit_id, context, false, &text, range);
                            }
                            self.table.invalid(self.core.dynamic_)
                        }
                    }
                } else {
                    self.table.invalid(self.core.dynamic_)
                }
            }
            ast::TypeKind::Function {
                return_type,
                type_params,
                parameters,
            } => {
                let mut local_scope = type_param_scope.clone();
                let mut local_params = Vec::with_capacity(type_params.len());
                for tp in type_params.iter() {
                    let pid = self.table.alloc_type_param(
                        tp.name.sym,
                        TypeParamOwner::GenericFunctionType,
                        self.core.object_nullable,
                        Variance::Unspecified,
                    );
                    local_params.push(pid);
                    local_scope.insert(tp.name.sym, pid);
                }

                for (tp, &pid) in type_params.iter().zip(local_params.iter()) {
                    if let Some(ast_bound) = tp.bound {
                        let b = self.resolve_annotation(unit_id, ast_bound, library, &local_scope);
                        self.table.set_type_param_bound(pid, b);
                    }
                }

                let ret = if let Some(r) = return_type {
                    self.resolve_annotation(unit_id, *r, library, &local_scope)
                } else {
                    self.core.dynamic_
                };

                let (pos, opt, named) =
                    self.resolve_ast_parameter_types(unit_id, parameters, library, &local_scope);

                self.table.intern(Type::Function {
                    type_params: local_params.into_boxed_slice(),
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opt.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: is_nullable,
                })
            }
            ast::TypeKind::Record { positional, named } => {
                let mut pos = Vec::with_capacity(positional.len());
                for &p in positional.iter() {
                    pos.push(self.resolve_annotation(unit_id, p, library, type_param_scope));
                }

                let mut n = Vec::with_capacity(named.len());
                for (name, ty) in named.iter() {
                    let resolved = self.resolve_annotation(unit_id, *ty, library, type_param_scope);
                    n.push((name.sym, resolved));
                }

                self.table.intern(Type::Record {
                    positional: pos.into_boxed_slice(),
                    named: n.into_boxed_slice(),
                    nullable: is_nullable,
                })
            }
        }
    }

    /// Parameter in the old form `R the_name(P p)`: a function type with return `p.ty`.
    fn resolve_function_typed_parameter(
        &mut self,
        unit_id: UnitId,
        p: &ast::Parameter,
        library: LibraryId,
        scope: &HashMap<SymbolId, TypeParamId>,
    ) -> TypeId {
        let mut local_scope = scope.clone();
        let mut local_params = Vec::new();
        for tp in p.function_type_params.iter() {
            let pid = self.table.alloc_type_param(
                tp.name.sym,
                TypeParamOwner::GenericFunctionType,
                self.core.object_nullable,
                Variance::Unspecified,
            );
            local_params.push(pid);
            local_scope.insert(tp.name.sym, pid);
        }
        let ret = match p.ty {
            Some(t) => self.resolve_annotation(unit_id, t, library, &local_scope),
            None => self.core.dynamic_,
        };
        let params: &[ast::Parameter] = p.function_parameters.as_deref().unwrap_or(&[]);
        let (pos, opt, named) = self.resolve_ast_parameter_types(unit_id, params, library, &local_scope);
        self.table.intern(Type::Function {
            type_params: local_params.into_boxed_slice(),
            ret,
            positional: pos.into_boxed_slice(),
            optional: opt.into_boxed_slice(),
            named: named.into_boxed_slice(),
            nullable: p.function_nullable,
        })
    }

    /// Type of a `super.x` parameter without an annotation: that of the
    /// same-named parameter of the called superclass constructor
    /// (`super(...)`/`super.the_name(...)`; without an initializer, the unnamed
    /// one), with the superclass's type parameters replaced by what the class
    /// passes to it.
    fn super_param_type(&mut self, func: &FunctionElement, ctor: &ast::Constructor, p: &ast::Parameter, depth: u32, hierarchy: &ClassHierarchy) -> Option<TypeId> {
        let t = self.super_param_type_raw(func, ctor, p, depth, hierarchy)?;
        let sup = self.program.class(func.class?).supertype_class?;
        Some(self.instantiate_from_super(func, sup, t, hierarchy).unwrap_or(t))
    }

    fn super_param_type_raw(&mut self, func: &FunctionElement, ctor: &ast::Constructor, p: &ast::Parameter, depth: u32, hierarchy: &ClassHierarchy) -> Option<TypeId> {
        if depth > 8 {
            return None;
        }
        let class = func.class?;
        let sup = self.program.class(class).supertype_class?;
        let super_name = ctor.initializers.iter().find_map(|i| match i {
            ast::Initializer::Super { constructor, .. } => Some(constructor.map(|n| n.sym)),
            _ => None,
        });
        let key = match super_name {
            Some(Some(n)) => n,
            _ => self.interner.lookup("")?,
        };
        let sfid = *self.program.class(sup).constructors.get(&key)?;
        let sfunc = self.program.function(sfid);
        let FunctionRef::Constructor { unit, member } = sfunc.node else { return None };
        let mem = self.program.unit(unit).ast.member(member);
        let MemberKind::Constructor(sctor) = &mem.kind else { return None };
        let pname = p.name?.sym;
        let sp = if p.kind == ParameterKind::Named {
            sctor.parameters.iter().find(|q| q.kind == ParameterKind::Named && q.external_name().map(|n| n.sym) == Some(pname))?
        } else {
            // Positional: the index among the `super.` positionals matches the remaining
            // positionals of the super constructor after those passed in `super(...)`.
            let explicit = ctor
                .initializers
                .iter()
                .find_map(|i| match i {
                    ast::Initializer::Super { arguments, .. } => Some(arguments.args.iter().filter(|a| a.name.is_none()).count()),
                    _ => None,
                })
                .unwrap_or(0);
            let my_index = ctor.parameters.iter().filter(|q| q.super_ && q.kind != ParameterKind::Named).position(|q| std::ptr::eq(q, p))?;
            sctor.parameters.iter().filter(|q| q.kind != ParameterKind::Named).nth(explicit + my_index)?
        };
        let scope = self.get_enclosing_type_param_scope(Some(sup), None);
        if let Some(t) = sp.ty {
            return Some(self.resolve_annotation(unit, t, sfunc.library, &scope));
        }
        if sp.this_ {
            return self.field_type_for_this_param(sfunc, sp.name.map(|n| n.sym));
        }
        if sp.super_ {
            let sfunc2 = self.program.function(sfid);
            return self.super_param_type(sfunc2, sctor, sp, depth + 1, hierarchy);
        }
        None
    }

    /// Type of the field for a `this.x` parameter without an annotation.
    fn field_type_for_this_param(&mut self, func: &FunctionElement, name: Option<SymbolId>) -> Option<TypeId> {
        let class = func.class?;
        let name = name?;
        let cls = self.program.class(class);
        let vid = cls.fields.iter().copied().find(|v| self.program.variable(*v).name == name)?;
        let v = self.program.variable(vid);
        let VariableRef::Field { unit, member, index } = v.node else { return None };
        let mem = self.program.unit(unit).ast.member(member);
        let MemberKind::Field(list) = &mem.kind else { return None };
        let _ = index;
        let t = list.ty?;
        let scope = self.get_enclosing_type_param_scope(Some(class), None);
        Some(self.resolve_annotation(unit, t, v.library, &scope))
    }

    fn resolve_ast_parameter_types(
        &mut self,
        unit_id: UnitId,
        parameters: &[ast::Parameter],
        library: LibraryId,
        scope: &HashMap<SymbolId, TypeParamId>,
    ) -> ParameterComponents {
        let mut positional = Vec::new();
        let mut optional = Vec::new();
        let mut named = Vec::new();

        for p in parameters.iter() {
            let p_ty = if p.function_parameters.is_some() {
                self.resolve_function_typed_parameter(unit_id, p, library, scope)
            } else if let Some(ast_ty) = p.ty {
                self.resolve_annotation(unit_id, ast_ty, library, scope)
            } else {
                self.core.dynamic_
            };

            match p.kind {
                ParameterKind::Required => positional.push(p_ty),
                ParameterKind::Optional => optional.push(p_ty),
                ParameterKind::Named => {
                    if let Some(n) = &p.name {
                        named.push((n.sym, p_ty, p.required));
                    }
                }
            }
        }

        (positional, optional, named)
    }
}

/// The container of a member (class or extension), if any.
fn container_of(the_class: Option<ClassId>, extension: Option<ExtensionId>) -> Option<Container> {
    match (the_class, extension) {
        (Some(c), _) => Some(Container::Classe(c)),
        (None, Some(e)) => Some(Container::Extensao(e)),
        _ => None,
    }
}
