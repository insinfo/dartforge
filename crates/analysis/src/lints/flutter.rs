//! The predicates of the 3.6.2 linter's `flutter_utils.dart` (`_Flutter`,
//! with the `package:flutter` prefix): the element is exactly the class with
//! the given name and URI (the URI of the unit that declares the class), or
//! has it among its supertypes.
//! Written without compiling or running (2026-10-05).

use dartforge_elements::model::{ClassId, Program};
use dartforge_intern::Interner;
use dartforge_types::table::{Type, TypeId, TypeTable};

const FRAMEWORK: &str = "package:flutter/src/widgets/framework.dart";
const CONTAINER: &str = "package:flutter/src/widgets/container.dart";
const BASIC: &str = "package:flutter/src/widgets/basic.dart";
const FOUNDATION: &str = "package:flutter/src/foundation/constants.dart";

/// `isExactly(element, name, uri)`.
pub fn is_exactly(
    program: &Program,
    interner: &Interner,
    c: ClassId,
    name: &str,
    uri: &str,
) -> bool {
    let k = program.class(c);
    interner.resolve(k.name) == name && k.decl.is_some_and(|r| program.unit(r.unit).uri == uri)
}

/// The element or some supertype is exactly `name` from `uri`.
fn is_or_subtype_of(
    s: &super::Semantics<'_>,
    interner: &Interner,
    c: ClassId,
    name: &str,
    uri: &str,
) -> bool {
    is_exactly(s.program, interner, c, name, uri)
        || s.outline.hierarchy.get(c).is_some_and(|d| {
            d.supertypes
                .keys()
                .any(|k| is_exactly(s.program, interner, *k, name, uri))
        })
}

fn class_of(table: &TypeTable, t: TypeId) -> Option<ClassId> {
    match table.get(t) {
        Type::Interface { class, .. } => Some(*class),
        _ => None,
    }
}

/// `isWidgetType`.
pub fn is_widget_type(s: &super::Semantics<'_>, interner: &Interner, t: TypeId) -> bool {
    class_of(s.table, t).is_some_and(|c| is_or_subtype_of(s, interner, c, "Widget", FRAMEWORK))
}

/// `isWidget(element)`.
pub fn is_widget(s: &super::Semantics<'_>, interner: &Interner, c: ClassId) -> bool {
    is_or_subtype_of(s, interner, c, "Widget", FRAMEWORK)
}

/// `isExactWidgetTypeContainer`.
pub fn is_container(s: &super::Semantics<'_>, interner: &Interner, t: TypeId) -> bool {
    class_of(s.table, t).is_some_and(|c| is_exactly(s.program, interner, c, "Container", CONTAINER))
}

/// `isExactWidgetTypeSizedBox`.
pub fn is_sized_box(s: &super::Semantics<'_>, interner: &Interner, t: TypeId) -> bool {
    class_of(s.table, t).is_some_and(|c| is_exactly(s.program, interner, c, "SizedBox", BASIC))
}

/// `isState`.
pub fn is_state(s: &super::Semantics<'_>, interner: &Interner, c: ClassId) -> bool {
    is_or_subtype_of(s, interner, c, "State", FRAMEWORK)
}

/// `isStatefulWidget`.
pub fn is_stateful_widget(s: &super::Semantics<'_>, interner: &Interner, c: ClassId) -> bool {
    is_or_subtype_of(s, interner, c, "StatefulWidget", FRAMEWORK)
}

/// `isExactWidget`.
pub fn is_exactly_widget(s: &super::Semantics<'_>, interner: &Interner, c: ClassId) -> bool {
    is_exactly(s.program, interner, c, "Widget", FRAMEWORK)
}

/// `isBuildContext(type, skipNullable)`.
pub fn is_build_context(
    s: &super::Semantics<'_>,
    interner: &Interner,
    t: TypeId,
    skip_nullable: bool,
) -> bool {
    match s.table.get(t) {
        Type::Interface {
            class, nullable, ..
        } => {
            !(skip_nullable && *nullable)
                && is_exactly(s.program, interner, *class, "BuildContext", FRAMEWORK)
        }
        _ => false,
    }
}

/// `hasWidgetAsAscendant`: the superclass chain reaches `Widget`.
pub fn has_widget_as_ascendant(s: &super::Semantics<'_>, interner: &Interner, c: ClassId) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut current = Some(c);
    while let Some(x) = current {
        if is_exactly(s.program, interner, x, "Widget", FRAMEWORK) {
            return true;
        }
        if !seen.insert(x) {
            return false;
        }
        current = s.program.class(x).supertype_class;
    }
    false
}

/// `isKDebugMode`: the `kDebugMode` constant of `foundation/constants.dart`.
pub fn is_k_debug_mode(
    program: &Program,
    interner: &Interner,
    v: dartforge_elements::model::VariableId,
) -> bool {
    let x = program.variable(v);
    if interner.resolve(x.name) != "kDebugMode" {
        return false;
    }
    match x.node {
        dartforge_elements::model::VariableRef::TopLevel { unit, .. } => {
            program.unit(unit).uri == FOUNDATION
        }
        _ => false,
    }
}
