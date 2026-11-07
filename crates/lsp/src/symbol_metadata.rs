//! Resolved annotation flags over the current program's document outline.
//! The resident can answer this without inferring any additional bodies.

use crate::project::Project;
use crate::utf16::LineTable;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{Annotation, DeclKind, MemberKind, Name};
use serde_json::Value;
use std::collections::HashSet;

/// Builds the existing outline, then decorates declarations by their exact
/// UTF-16 name position. Owner annotations do not propagate to children.
pub(crate) fn for_project(project: &Project, unit: UnitId) -> Vec<Value> {
    let data = project.program().unit(unit);
    let mut symbols = crate::symbols::for_document(&data.source, data.features);
    let lines = LineTable::build(&data.source);
    let mut deprecated = HashSet::new();
    let mut add = |name: Option<Name>, metadata: &[Annotation]| {
        if let Some(name) = name
            && !metadata.is_empty()
            && project.query.has_deprecated_metadata(unit, metadata)
        {
            deprecated.insert(lines.offset_position(&data.source, name.span.start));
        }
    };
    for id in &data.unit.declarations {
        let declaration = data.ast.decl(*id);
        let name = match &declaration.kind {
            DeclKind::Class(class) => Some(class.name),
            DeclKind::Mixin(mixin) => Some(mixin.name),
            DeclKind::Enum(value) => Some(value.name),
            DeclKind::Extension(extension) => extension.name,
            DeclKind::ExtensionType(extension) => Some(extension.name),
            DeclKind::Typedef(alias) => Some(alias.name),
            DeclKind::Function(function) => data.ast.function(*function).name,
            DeclKind::Variables(variables) => {
                if project.query.display_reference == crate::query::DisplayReference::Legacy {
                    for variable in &variables.variables {
                        add(Some(variable.name), &declaration.metadata);
                    }
                }
                None
            }
        };
        add(name, &declaration.metadata);
    }
    for member in &data.ast.members {
        let name = match &member.kind {
            MemberKind::Method(function) => data.ast.function(*function).name,
            MemberKind::Constructor(constructor) => {
                Some(constructor.name.unwrap_or(constructor.class_name))
            }
            MemberKind::Field(field) => {
                if project.query.display_reference == crate::query::DisplayReference::Legacy {
                    for variable in &field.variables {
                        add(Some(variable.name), &member.metadata);
                    }
                }
                None
            }
        };
        add(name, &member.metadata);
    }
    // Enum constants keep their own annotation list, independent of the enum.
    for declaration in &data.ast.decls {
        if let DeclKind::Enum(value) = &declaration.kind {
            for constant in &value.constants {
                add(Some(constant.name), &constant.metadata);
            }
        }
    }
    fn decorate(symbols: &mut [Value], deprecated: &HashSet<(u32, u32)>) {
        for symbol in symbols {
            let start = &symbol["selectionRange"]["start"];
            if let (Some(line), Some(character)) =
                (start["line"].as_u64(), start["character"].as_u64())
                && let (Ok(line), Ok(character)) = (u32::try_from(line), u32::try_from(character))
                && deprecated.contains(&(line, character))
            {
                symbol["deprecated"] = Value::Bool(true);
            }
            if let Some(children) = symbol.get_mut("children").and_then(Value::as_array_mut) {
                decorate(children, deprecated);
            }
        }
    }
    decorate(&mut symbols, &deprecated);
    symbols
}
