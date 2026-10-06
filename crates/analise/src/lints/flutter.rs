//! Os predicados do `flutter_utils.dart` do linter 3.6.2 (`_Flutter`, com o
//! prefixo `package:flutter`): o elemento é exatamente a classe de nome e
//! URI dados (a URI da unidade que declara a classe), ou a tem entre os
//! supertipos.
//! Escrito sem compilar nem executar (2026-10-05).

use dartforge_elements::model::{ClassId, Program};
use dartforge_intern::Interner;
use dartforge_types::table::{Type, TypeId, TypeTable};

const FRAMEWORK: &str = "package:flutter/src/widgets/framework.dart";
const CONTAINER: &str = "package:flutter/src/widgets/container.dart";
const BASIC: &str = "package:flutter/src/widgets/basic.dart";
const FOUNDATION: &str = "package:flutter/src/foundation/constants.dart";

/// `isExactly(element, nome, uri)`.
pub fn exatamente(program: &Program, interner: &Interner, c: ClassId, nome: &str, uri: &str) -> bool {
    let k = program.class(c);
    interner.resolve(k.name) == nome && k.decl.is_some_and(|r| program.unit(r.unit).uri == uri)
}

/// O elemento ou algum supertipo é exatamente `nome` de `uri`.
fn ou_supertipo(s: &super::Semantica<'_>, interner: &Interner, c: ClassId, nome: &str, uri: &str) -> bool {
    exatamente(s.program, interner, c, nome, uri)
        || s.outline.hierarchy.get(c).is_some_and(|d| d.supertypes.keys().any(|k| exatamente(s.program, interner, *k, nome, uri)))
}

fn classe(table: &TypeTable, t: TypeId) -> Option<ClassId> {
    match table.get(t) {
        Type::Interface { class, .. } => Some(*class),
        _ => None,
    }
}

/// `isWidgetType`.
pub fn e_widget_tipo(s: &super::Semantica<'_>, interner: &Interner, t: TypeId) -> bool {
    classe(s.table, t).is_some_and(|c| ou_supertipo(s, interner, c, "Widget", FRAMEWORK))
}

/// `isWidget(element)`.
pub fn e_widget(s: &super::Semantica<'_>, interner: &Interner, c: ClassId) -> bool {
    ou_supertipo(s, interner, c, "Widget", FRAMEWORK)
}

/// `isExactWidgetTypeContainer`.
pub fn e_container(s: &super::Semantica<'_>, interner: &Interner, t: TypeId) -> bool {
    classe(s.table, t).is_some_and(|c| exatamente(s.program, interner, c, "Container", CONTAINER))
}

/// `isExactWidgetTypeSizedBox`.
pub fn e_sized_box(s: &super::Semantica<'_>, interner: &Interner, t: TypeId) -> bool {
    classe(s.table, t).is_some_and(|c| exatamente(s.program, interner, c, "SizedBox", BASIC))
}

/// `isState`.
pub fn e_state(s: &super::Semantica<'_>, interner: &Interner, c: ClassId) -> bool {
    ou_supertipo(s, interner, c, "State", FRAMEWORK)
}

/// `isStatefulWidget`.
pub fn e_stateful_widget(s: &super::Semantica<'_>, interner: &Interner, c: ClassId) -> bool {
    ou_supertipo(s, interner, c, "StatefulWidget", FRAMEWORK)
}

/// `isExactWidget`.
pub fn e_widget_exato(s: &super::Semantica<'_>, interner: &Interner, c: ClassId) -> bool {
    exatamente(s.program, interner, c, "Widget", FRAMEWORK)
}

/// `isBuildContext(type, skipNullable)`.
pub fn e_build_context(s: &super::Semantica<'_>, interner: &Interner, t: TypeId, pular_anulavel: bool) -> bool {
    match s.table.get(t) {
        Type::Interface { class, nullable, .. } => !(pular_anulavel && *nullable) && exatamente(s.program, interner, *class, "BuildContext", FRAMEWORK),
        _ => false,
    }
}

/// `hasWidgetAsAscendant`: a cadeia de superclasses chega a `Widget`.
pub fn tem_widget_ascendente(s: &super::Semantica<'_>, interner: &Interner, c: ClassId) -> bool {
    let mut vistos = std::collections::HashSet::new();
    let mut atual = Some(c);
    while let Some(x) = atual {
        if exatamente(s.program, interner, x, "Widget", FRAMEWORK) {
            return true;
        }
        if !vistos.insert(x) {
            return false;
        }
        atual = s.program.class(x).supertype_class;
    }
    false
}

/// `isKDebugMode`: a constante `kDebugMode` do `foundation/constants.dart`.
pub fn e_k_debug_mode(program: &Program, interner: &Interner, v: dartforge_elements::model::VariableId) -> bool {
    let x = program.variable(v);
    if interner.resolve(x.name) != "kDebugMode" {
        return false;
    }
    match x.node {
        dartforge_elements::model::VariableRef::TopLevel { unit, .. } => program.unit(unit).uri == FOUNDATION,
        _ => false,
    }
}
