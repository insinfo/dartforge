//! Emissão de constantes canônicas, enums avançadas e switches sem fallthrough.
use super::*;
use dartforge_syntax::{ConstValue, Pattern, SwitchArm, SwitchCase, TypeShape};

/// Codifica o tipo estrutural, sem depender dos IDs locais de bibliotecas.
fn type_key(ty: Type, resolution: &dartforge_syntax::Resolution) -> String {
    match ty {
        Type::Applied(id) => match &resolution.types[id as usize] {
            TypeShape::List(t) => format!("List<{}>", type_key(*t, resolution)),
            TypeShape::Iterable(t) => format!("Iterable<{}>", type_key(*t, resolution)),
            TypeShape::Nullable(t) => format!("Nullable<{}>", type_key(*t, resolution)),
            TypeShape::Record { positional, named } => format!(
                "Record({:?};{:?})",
                positional
                    .iter()
                    .map(|ty| type_key(*ty, resolution))
                    .collect::<Vec<_>>(),
                named
                    .iter()
                    .map(|(name, ty)| (name, type_key(*ty, resolution)))
                    .collect::<Vec<_>>()
            ),
            TypeShape::Function { result, parameters } => format!(
                "Fn({:?})->{}",
                parameters
                    .iter()
                    .map(|t| type_key(*t, resolution))
                    .collect::<Vec<_>>(),
                type_key(*result, resolution)
            ),
        },
        _ => format!("{ty:?}"),
    }
}
/// Produz chave sem ambiguidades para igualdade canônica de listas constantes.
fn constant_key(
    value: &ConstValue,
    resolution: &dartforge_syntax::Resolution,
) -> serde_json::Value {
    match value {
        ConstValue::Int(n) => serde_json::json!(["int", n]),
        ConstValue::Bool(b) => serde_json::json!(["bool", b]),
        ConstValue::String(s) => serde_json::json!(["string", s]),
        ConstValue::Null => serde_json::json!(["null"]),
        ConstValue::Enum { class_id, name } => serde_json::json!(["enum", class_id, name]),
        ConstValue::List {
            element_type,
            values,
        } => serde_json::json!([
            "list",
            type_key(*element_type, resolution),
            values
                .iter()
                .map(|v| constant_key(v, resolution))
                .collect::<Vec<_>>()
        ]),
    }
}
/// Emite valores já avaliados; listas recebem cache compartilhado e armazenamento congelado.
pub(super) fn constant(value: &ConstValue, output: &mut Output<'_>) {
    match value {
        ConstValue::Int(n) => write!(output, "{n}").unwrap(),
        ConstValue::Bool(b) => output.push_str(if *b { "true" } else { "false" }),
        ConstValue::String(s) => string_literal(s, output),
        ConstValue::Null => output.push_str("null"),
        ConstValue::Enum { class_id, name } => {
            write!(output, "$dartforgeClass{class_id}.").unwrap();
            identifier(name, output);
        }
        ConstValue::List {
            values,
            element_type,
        } => {
            output.push_str("$dartforgeConstList(");
            let key = constant_key(value, output.resolution).to_string();
            string_literal(&key, output);
            output.push_str(",[");
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    output.push(',');
                }
                constant(v, output);
            }
            output.push_str("],");
            types::descriptor(*element_type, output);
            output.push(')');
        }
    }
}
/// Emite classe interna e singletons cujos argumentos foram verificados como constantes escalares.
pub(super) fn enhanced_enum(class: &Class<'_>, output: &mut Output<'_>) {
    write!(
        output,
        "class $dartforgeEnum{} {{ constructor(i,n",
        class.id
    )
    .unwrap();
    for i in 0..class.enum_constructor_fields.len() {
        write!(output, ",a{i}").unwrap();
    }
    write!(
        output,
        ") {{this.$dartforgeEnumTag={};this.$df_index=i;",
        class.id
    )
    .unwrap();
    if !class
        .methods
        .iter()
        .any(|m| m.is_getter && m.name == "name")
    {
        output.push_str("this.$df_name=n;");
    }
    for (i, name) in class.enum_constructor_fields.iter().enumerate() {
        output.push_str("this.");
        identifier(name, output);
        write!(output, "=a{i};").unwrap();
    }
    output.push_str("Object.freeze(this); }\n");
    for method in &class.methods {
        if method.is_getter {
            output.push_str("get ");
        }
        identifier(method.name, output);
        output.push('(');
        for (i, p) in method.parameters.iter().enumerate() {
            if i > 0 {
                output.push(',');
            }
            identifier(p.name, output);
        }
        output.push_str(") ");
        function_body(method, 1, output);
        output.push('\n');
    }
    write!(
        output,
        "}}\nconst $dartforgeClass{}=Object.freeze({{",
        class.id
    )
    .unwrap();
    for (i, name) in class.enum_values.iter().enumerate() {
        if i > 0 {
            output.push(',');
        }
        identifier(name, output);
        write!(output, ":new $dartforgeEnum{}({i},", class.id).unwrap();
        string_literal(name, output);
        for arg in &class.enum_arguments[i] {
            output.push(',');
            expression(arg, output);
        }
        output.push(')');
    }
    output.push_str("});\n");
}
/// Emite teste de padrão sem repetir a avaliação do discriminante.
fn pattern(pattern: &Pattern<'_>, temp: &str, output: &mut Output<'_>) {
    match pattern {
        Pattern::Wildcard => output.push_str("true"),
        Pattern::Constant(e) => {
            output.push_str(temp);
            output.push_str("===");
            expression(e, output);
        }
        Pattern::Binding { ty, .. } | Pattern::Type(ty) => match ty {
            Type::Class(id) | Type::NullableClass(id) => {
                output.push('(');
                if matches!(ty, Type::NullableClass(_)) {
                    write!(output, "{temp}===null||").unwrap();
                }
                let members = output.nominal_members.get(id).cloned().unwrap_or_default();
                if members.is_empty() {
                    output.push_str("false");
                }
                for (index, member) in members.into_iter().enumerate() {
                    if index != 0 {
                        output.push_str("||");
                    }
                    if output.enum_ids.contains(&member) {
                        write!(
                            output,
                            "({temp}!==null&&{temp}.$dartforgeEnumTag==={member})"
                        )
                        .unwrap();
                    } else {
                        write!(output, "{temp} instanceof $dartforgeClass{member}").unwrap();
                    }
                }
                output.push(')');
            }
            Type::Int | Type::NullableInt => {
                if *ty == Type::NullableInt {
                    write!(output, "({temp}===null||Number.isInteger({temp}))").unwrap();
                } else {
                    write!(output, "Number.isInteger({temp})").unwrap();
                }
            }
            Type::String | Type::NullableString | Type::Bool | Type::NullableBool => {
                let name = if matches!(ty, Type::String | Type::NullableString) {
                    "string"
                } else {
                    "boolean"
                };
                output.push('(');
                if matches!(ty, Type::NullableString | Type::NullableBool) {
                    write!(output, "{temp}===null||").unwrap();
                }
                write!(output, "typeof {temp}==='{}')", name).unwrap();
            }
            _ => {
                write!(output, "$dartforgeIs({temp},").unwrap();
                types::descriptor(*ty, output);
                output.push(')');
            }
        },
    }
}
/// Abre escopo de binding e teste da guarda na ordem prescrita pelo Dart.
fn arm_open(p: &Pattern<'_>, guard: Option<&Expr<'_>>, temp: &str, output: &mut Output<'_>) {
    output.push_str("if (");
    pattern(p, temp, output);
    output.push_str(") {\n");
    if let Pattern::Binding { name, .. } = p {
        output.push_str("let ");
        identifier(name, output);
        writeln!(output, "={temp};").unwrap();
    }
    if let Some(guard) = guard {
        output.push_str("if (");
        expression(guard, output);
        output.push_str(") {\n");
    }
}
/// Fecha os escopos introduzidos pelo padrão e por sua guarda.
fn arm_close(guard: bool, output: &mut Output<'_>) {
    if guard {
        output.push_str("}\n");
    }
    output.push_str("}\n");
}
/// Avalia o discriminante uma vez e somente o braço selecionado.
pub(super) fn switch_expression(
    scrutinee: &Expr<'_>,
    arms: &[SwitchArm<'_>],
    output: &mut Output<'_>,
) {
    let id = output.next_switch;
    output.next_switch += 1;
    let temp = format!("$dartforgeSwitch{id}");
    writeln!(output, "(({temp})=>{{").unwrap();
    for arm in arms {
        arm_open(&arm.pattern, arm.guard.as_ref(), &temp, output);
        output.push_str("return ");
        expression(&arm.value, output);
        output.push_str(";\n");
        arm_close(arm.guard.is_some(), output);
    }
    output.push_str("throw new Error('Unreachable exhaustive switch');})(");
    expression(scrutinee, output);
    output.push(')');
}
/// Usa rótulo exclusivo para break, sem transformar continue de um laço externo.
pub(super) fn switch_statement(
    scrutinee: &Expr<'_>,
    cases: &[SwitchCase<'_>],
    depth: usize,
    output: &mut Output<'_>,
) {
    let id = output.next_switch;
    output.next_switch += 1;
    let temp = format!("$dartforgeSwitch{id}");
    let label = format!("$dartforgeCase{id}");
    write!(output, "{label}: {{ const {temp}=").unwrap();
    expression(scrutinee, output);
    output.push_str(";\n");
    output.break_targets.push(Some(label.clone()));
    for case in cases {
        arm_open(&case.pattern, case.guard.as_ref(), &temp, output);
        statements(&case.body, depth + 1, output);
        writeln!(output, "break {label};").unwrap();
        arm_close(case.guard.is_some(), output);
    }
    output.break_targets.pop();
    output.push_str("}\n");
}

/// Calcula implementações nominais incluindo interfaces transitivas, sem depender de instanceof da interface.
pub(super) fn nominal_members(classes: &[Class<'_>]) -> std::collections::HashMap<u32, Vec<u32>> {
    let by_id: std::collections::HashMap<_, _> =
        classes.iter().map(|class| (class.id, class)).collect();
    let mut result = std::collections::HashMap::<u32, Vec<u32>>::new();
    for class in classes.iter().filter(|class| !class.is_abstract) {
        let mut pending = vec![class.id];
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = pending.pop() {
            if !seen.insert(id) {
                continue;
            }
            result.entry(id).or_default().push(class.id);
            if let Some(parent) = by_id.get(&id) {
                pending.extend(parent.superclass);
                pending.extend(parent.interfaces.iter().copied());
            }
        }
    }
    result
}
