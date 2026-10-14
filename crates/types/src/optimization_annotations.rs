//! Resolve optional DartForge metadata into preferences and pending obligations.
//!
//! Recognition is not proof: this module never supplies ownership, escape,
//! acyclicity, or realtime safety facts. It retains declaration identity and
//! source dependencies so a caller can apply an explicit verification policy.

use crate::annotations::{InvokedElement, invoked_element};
use crate::const_eval::evaluation::{Constant, Engine};
use crate::const_eval::value::InstanceState;
use crate::{BodyTypes, CoreTypes, OutlineTypes, Type, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{
    ClassId, Element, FunctionElementId, FunctionRef, LibraryId, Program, UnitId,
};
use dartforge_frontend::ast::{self, AsyncModifier, DeclKind, FunctionBody, MemberKind, StmtKind};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

/// The actual defining library, independent of public import spelling.
pub const DEFINING_URI: &str = "package:dartforge/src/annotations.dart";
/// The metadata schema understood by this normalization layer.
pub const SUPPORTED_SCHEMA: i64 = 1;

/// An optimization search preference, never a safety assertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Preference {
    Region,
    Acyclic,
}
/// A requested property that remains unproved after normalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Obligation {
    NoEscape,
    Realtime,
}
/// Preferences and verification obligations cannot be confused by their type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnnotationKind {
    Preference(Preference),
    Obligation(Obligation),
}

/// An AST function and its element when one exists (locals have no element).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FunctionTarget {
    pub function: ast::FunctionId,
    pub element: Option<FunctionElementId>,
}
/// Identity of the declaration or formal to which metadata applies.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AnnotationTarget {
    Class(ClassId),
    Constructor(FunctionElementId),
    Function(FunctionTarget),
    LocalVariables {
        statement: ast::StmtId,
        name_offsets: Vec<usize>,
    },
    Parameter {
        owner: FunctionTarget,
        index: usize,
    },
    Unsupported,
}
/// Source-body information; a missing body is not an implicit external summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyAvailability {
    Available,
    Unavailable,
    NotApplicable,
}

/// A recognized request, with its exact origin. No field represents a proof.
#[derive(Clone, Debug)]
pub struct NormalizedAnnotation {
    pub unit: UnitId,
    pub span: Span,
    pub kind: AnnotationKind,
    pub target: AnnotationTarget,
    pub annotation_class: ClassId,
    pub body: BodyAvailability,
}
/// Exact source dependency, suitable for inclusion in a caller's content hash.
#[derive(Clone, Debug)]
pub struct SourceDependency {
    pub unit: UnitId,
    pub uri: String,
    pub source: String,
}
/// Resolved package identity. Language version is deliberately not package version.
#[derive(Clone, Debug)]
pub struct AnnotationPackage {
    pub library: LibraryId,
    pub defining_uri: String,
    pub schema: Option<i64>,
    pub sources: Vec<SourceDependency>,
}
/// Diagnostic categories allow the caller to distinguish schema policy from targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationDiagnosticKind {
    UnsupportedSchema,
    InvalidMetadata,
    InvalidTarget,
}
/// A DartForge diagnostic, separate from the upstream Dart diagnostic code table.
#[derive(Clone, Debug)]
pub struct AnnotationDiagnostic {
    pub unit: UnitId,
    pub span: Span,
    pub kind: AnnotationDiagnosticKind,
    pub message: String,
}
/// Deterministically ordered metadata requests and validation diagnostics.
#[derive(Clone, Debug, Default)]
pub struct Normalization {
    pub package: Option<AnnotationPackage>,
    pub records: Vec<NormalizedAnnotation>,
    pub diagnostics: Vec<AnnotationDiagnostic>,
}

#[derive(Clone)]
struct TargetInfo {
    target: AnnotationTarget,
    body: BodyAvailability,
    synchronous: bool,
    callable: bool,
}
impl TargetInfo {
    fn plain(target: AnnotationTarget) -> Self {
        Self {
            target,
            body: BodyAvailability::NotApplicable,
            synchronous: false,
            callable: false,
        }
    }
}

/// Resolves annotations using constant values, without proving their requests.
///
/// Supply bodies inferred with local bindings, and include the annotation library
/// in `inferred` when its initializers are available. Invalid metadata and target
/// errors remain separate from unsupported-schema diagnostics; the caller owns
/// verify/ignore policy. Existing Dart resolution/constant diagnostics must also
/// be preserved by the caller. Only non-SDK libraries in `libraries` are visited.
///
/// ```
/// use dartforge_types::optimization_annotations::{AnnotationKind, Obligation};
/// let request = AnnotationKind::Obligation(Obligation::NoEscape);
/// assert!(matches!(request, AnnotationKind::Obligation(_)));
/// ```
#[allow(clippy::too_many_arguments)]
pub fn normalize(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    inferred: &HashSet<LibraryId>,
    libraries: &[LibraryId],
) -> Normalization {
    let mut result = Normalization::default();
    let Some((index, library)) = program
        .libraries
        .iter()
        .enumerate()
        .find(|(_, l)| l.uri == DEFINING_URI)
    else {
        return result;
    };
    let library_id = LibraryId(index as u32);
    let mut classes = HashMap::new();
    for (id, class) in program.classes.iter().enumerate() {
        if class.library != library_id {
            continue;
        }
        let kind = match interner.resolve(class.name) {
            "PreferRegion" => AnnotationKind::Preference(Preference::Region),
            "PreferAcyclic" => AnnotationKind::Preference(Preference::Acyclic),
            "NoEscape" => AnnotationKind::Obligation(Obligation::NoEscape),
            "Realtime" => AnnotationKind::Obligation(Obligation::Realtime),
            _ => continue,
        };
        classes.insert(ClassId(id as u32), kind);
    }
    let mut engine = Engine::new(program, interner, table, core, outline, bodies, inferred);
    let schema_variable = interner
        .lookup("dartForgeAnnotationsSchema")
        .and_then(|name| library.declared.get(&name))
        .and_then(|b| match b.getter {
            Some(Element::Variable(v)) => Some(v),
            Some(Element::Function(f)) => program.function(f).variable,
            _ => None,
        });
    let schema = schema_variable
        .filter(|v| program.variable(*v).const_)
        .and_then(|v| engine.variable_value(v))
        .and_then(|value| match value {
            Constant::Value(v) => match v.state {
                InstanceState::Int(value) => value,
                _ => None,
            },
            Constant::Invalid(_) => None,
        });
    result.package = Some(AnnotationPackage {
        library: library_id,
        defining_uri: library.uri.clone(),
        schema,
        sources: library
            .units
            .iter()
            .map(|&unit| SourceDependency {
                unit,
                uri: program.unit(unit).uri.clone(),
                source: program.unit(unit).source.to_string(),
            })
            .collect(),
    });
    let mut visited = HashSet::new();
    for &lib in libraries {
        if program.library(lib).is_sdk {
            continue;
        }
        for &unit in &program.library(lib).units {
            if !visited.insert(unit) {
                continue;
            }
            let u = program.unit(unit);
            let targets = target_map(program, bodies, unit);
            for metadata in dartforge_frontend::parents::all_annotations(&u.ast, &u.unit) {
                let invoked = invoked_element(program, interner, unit, metadata);
                // This gate preserves lexical shadowing and ambiguous bindings.
                let local = bodies
                    .units
                    .get(unit.0 as usize)
                    .and_then(|b| b.metadata_local_declarations.get(&metadata.span.start))
                    .copied();
                if local.is_none() && ambiguous_binding(program, unit, metadata) {
                    continue;
                }
                if local.is_none() && matches!(invoked, InvokedElement::Other) {
                    continue;
                }
                let value = engine.evaluate_annotation(unit, metadata);
                let evaluated_class = value.as_ref().and_then(|v| match engine.table.get(v.ty) {
                    Type::Interface { class, .. } => Some(*class),
                    _ => None,
                });
                let candidate = match invoked {
                    InvokedElement::Constructor(class, _) | InvokedElement::Class(class) => {
                        Some(class)
                    }
                    InvokedElement::Getter(f) => program.function(f).variable.and_then(|v| {
                        let t = outline.variables.get(v.0 as usize)?;
                        let t = t.inferred.or(t.declared_type)?;
                        match engine.table.get(t) {
                            Type::Interface { class, .. } => Some(*class),
                            _ => None,
                        }
                    }),
                    _ => local
                        .and_then(|offset| bodies.units.get(unit.0 as usize)?.local_type(offset))
                        .and_then(|t| match engine.table.get(t) {
                            Type::Interface { class, .. } => Some(*class),
                            _ => None,
                        }),
                };
                let Some(class) = evaluated_class
                    .or(candidate)
                    .filter(|c| program.class(*c).library == library_id)
                else {
                    continue;
                };
                if schema != Some(SUPPORTED_SCHEMA) {
                    result.diagnostics.push(AnnotationDiagnostic { unit, span: metadata.span,
                        kind: AnnotationDiagnosticKind::UnsupportedSchema,
                        message: format!("DartForge annotation schema must be {SUPPORTED_SCHEMA}; found {schema:?}"), });
                    continue;
                }
                let Some(&kind) = classes.get(&class) else {
                    continue;
                };
                let well_formed = value.as_ref().is_some_and(|v| evaluated_class == Some(class)
                    && matches!(&v.state, InstanceState::Generic { fields, unknown: false } if fields.is_empty()))
                    && metadata.type_args.is_empty()
                    && metadata.arguments.as_ref().is_none_or(|a| a.args.is_empty());
                if !well_formed {
                    result.diagnostics.push(AnnotationDiagnostic { unit, span: metadata.span,
                        kind: AnnotationDiagnosticKind::InvalidMetadata,
                        message: "DartForge v1 annotations must evaluate to a fieldless constant with no arguments".into(), });
                    continue;
                }
                let info = targets
                    .get(&metadata.span.start)
                    .cloned()
                    .unwrap_or_else(|| TargetInfo::plain(AnnotationTarget::Unsupported));
                let valid = match kind {
                    AnnotationKind::Preference(Preference::Region)
                    | AnnotationKind::Obligation(Obligation::Realtime) => {
                        matches!(info.target, AnnotationTarget::Function(_))
                            && info.callable
                            && info.synchronous
                            && info.body == BodyAvailability::Available
                    }
                    AnnotationKind::Preference(Preference::Acyclic) => matches!(
                        info.target,
                        AnnotationTarget::Class(_)
                            | AnnotationTarget::Constructor(_)
                            | AnnotationTarget::LocalVariables { .. }
                    ),
                    AnnotationKind::Obligation(Obligation::NoEscape) => {
                        matches!(info.target, AnnotationTarget::Parameter { .. }) && info.callable
                    }
                };
                if !valid {
                    result.diagnostics.push(AnnotationDiagnostic {
                        unit,
                        span: metadata.span,
                        kind: AnnotationDiagnosticKind::InvalidTarget,
                        message: format!(
                            "Invalid target for DartForge annotation {kind:?}: {:?}",
                            info.target
                        ),
                    });
                    continue;
                }
                result.records.push(NormalizedAnnotation {
                    unit,
                    span: metadata.span,
                    kind,
                    target: info.target,
                    annotation_class: class,
                    body: info.body,
                });
            }
        }
    }
    result
        .records
        .sort_by_key(|r| (r.unit.0, r.span.start, r.span.end));
    result
        .diagnostics
        .sort_by_key(|r| (r.unit.0, r.span.start, r.span.end));
    result
}

// The shared annotation helpers intentionally model analyzer recovery targets;
// a compiler contract cannot grant authority to an ambiguous recovery binding.
fn ambiguous_binding(program: &Program, unit: UnitId, metadata: &ast::Annotation) -> bool {
    let Some(first) = metadata.name.first() else {
        return false;
    };
    let binding = program.lookup_in_unit(unit, first.sym);
    if binding.is_some_and(|b| b.ambiguous) {
        return true;
    }
    if metadata.name.len() > 1
        && !binding.is_some_and(|b| matches!(b.getter, Some(Element::Class(_))))
    {
        return program
            .lookup_prefixed_in_unit(unit, first.sym, metadata.name[1].sym)
            .is_some_and(|b| b.ambiguous);
    }
    false
}

fn target_map(program: &Program, bodies: &BodyTypes, unit: UnitId) -> HashMap<usize, TargetInfo> {
    let ast = &program.unit(unit).ast;
    let mut result = HashMap::new();
    let function = |id: ast::FunctionId| {
        let f = ast.function(id);
        let element = program
            .functions
            .iter()
            .enumerate()
            .find_map(|(i, f)| match f.node {
                FunctionRef::Function { unit: u, function } if u == unit && function == id => {
                    Some(FunctionElementId(i as u32))
                }
                _ => None,
            });
        TargetInfo {
            target: AnnotationTarget::Function(FunctionTarget {
                function: id,
                element,
            }),
            body: if !f.external
                && matches!(f.body, FunctionBody::Block(_) | FunctionBody::Expression(_))
            {
                BodyAvailability::Available
            } else {
                BodyAvailability::Unavailable
            },
            synchronous: f.modifier == AsyncModifier::None,
            callable: matches!(
                f.kind,
                ast::FunctionKind::Function | ast::FunctionKind::Operator
            ),
        }
    };
    let mut attach = |metadata: &[ast::Annotation], info: TargetInfo| {
        for m in metadata {
            result.insert(m.span.start, info.clone());
        }
    };
    for (i, decl) in ast.decls.iter().enumerate() {
        let info = match &decl.kind {
            DeclKind::Function(id) => function(*id),
            DeclKind::Class(_) => program
                .classes
                .iter()
                .enumerate()
                .find_map(|(ci, c)| {
                    c.decl
                        .filter(|d| d.unit == unit && d.decl.0 as usize == i)
                        .map(|_| TargetInfo::plain(AnnotationTarget::Class(ClassId(ci as u32))))
                })
                .unwrap_or_else(|| TargetInfo::plain(AnnotationTarget::Unsupported)),
            _ => TargetInfo::plain(AnnotationTarget::Unsupported),
        };
        attach(&decl.metadata, info);
    }
    for (i, member) in ast.members.iter().enumerate() {
        let info = match member.kind {
            MemberKind::Method(id) => function(id),
            MemberKind::Constructor(_) => program
                .functions
                .iter()
                .enumerate()
                .find_map(|(fi, f)| match f.node {
                    FunctionRef::Constructor { unit: u, member }
                        if u == unit && member.0 as usize == i =>
                    {
                        Some(TargetInfo::plain(AnnotationTarget::Constructor(
                            FunctionElementId(fi as u32),
                        )))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| TargetInfo::plain(AnnotationTarget::Unsupported)),
            _ => TargetInfo::plain(AnnotationTarget::Unsupported),
        };
        attach(&member.metadata, info);
    }
    for (i, f) in ast.functions.iter().enumerate() {
        let info = function(ast::FunctionId(i as u32));
        if let (AnnotationTarget::Function(owner), Some(parameters)) = (&info.target, &f.parameters)
        {
            for (index, parameter) in parameters.iter().enumerate() {
                let mut formal = info.clone();
                formal.target = AnnotationTarget::Parameter {
                    owner: *owner,
                    index,
                };
                attach(&parameter.metadata, formal);
            }
        }
    }
    for (statement, metadata) in &ast.local_metadata {
        let info = match &ast.stmt(*statement).kind {
            StmtKind::Function(id) => function(*id),
            StmtKind::Variables(v) => TargetInfo::plain(AnnotationTarget::LocalVariables {
                statement: *statement,
                name_offsets: v.variables.iter().map(|v| v.name.span.start).collect(),
            }),
            StmtKind::PatternVariables { pattern, .. } => {
                let span = ast.pattern(*pattern).span;
                let mut name_offsets: Vec<_> = bodies
                    .units
                    .get(unit.0 as usize)
                    .into_iter()
                    .flat_map(|b| b.local_types.keys().copied())
                    .filter(|offset| *offset >= span.start && *offset < span.end)
                    .collect();
                name_offsets.sort_unstable();
                TargetInfo::plain(AnnotationTarget::LocalVariables {
                    statement: *statement,
                    name_offsets,
                })
            }
            _ => TargetInfo::plain(AnnotationTarget::Unsupported),
        };
        attach(metadata, info);
    }
    for (i, stmt) in ast.stmts.iter().enumerate() {
        if let StmtKind::ForIn {
            target: ast::ForInTarget::Declared { metadata, name, .. },
            ..
        } = &stmt.kind
        {
            attach(
                metadata,
                TargetInfo::plain(AnnotationTarget::LocalVariables {
                    statement: ast::StmtId(i as u32),
                    name_offsets: vec![name.span.start],
                }),
            );
        }
    }
    result
}
