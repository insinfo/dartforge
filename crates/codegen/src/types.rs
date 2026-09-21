//! Reificação explícita de tipos sem apagar argumentos de funções genéricas.
//! SDK 3.6.2 tests/language/generic/function_bounds_test.dart fundamenta bounds;
//! descritores e representação JavaScript abaixo são próprios deste compilador.
use super::*;
use dartforge_syntax::TypeShape;

/// Emite um descritor estrutural ou referência ao ambiente genérico da ativação.
pub(super) fn descriptor(ty: Type, output: &mut Output<'_>) {
    output.runtime_types_used = true;
    match ty {
        Type::Parameter(index) => write!(output, "$dartforgeTypes[{index}]").unwrap(),
        Type::NullableParameter(index) => {
            write!(output, "['nullable',$dartforgeTypes[{index}]]").unwrap()
        }
        Type::NullableObject
        | Type::NullableInt
        | Type::NullableString
        | Type::NullableBool
        | Type::NullableClass(_) => {
            output.push_str("['nullable',");
            descriptor(
                match ty {
                    Type::NullableObject => Type::Object,
                    Type::NullableInt => Type::Int,
                    Type::NullableString => Type::String,
                    Type::NullableBool => Type::Bool,
                    Type::NullableClass(id) => Type::Class(id),
                    _ => unreachable!(),
                },
                output,
            );
            output.push(']');
        }
        Type::Class(id) => write!(output, "['class',{id}]").unwrap(),
        Type::Applied(id) => match output.resolution.types[id as usize].clone() {
            TypeShape::Future(element)
            | TypeShape::List(element)
            | TypeShape::Iterable(element)
            | TypeShape::Nullable(element) => {
                let tag = match &output.resolution.types[id as usize] {
                    TypeShape::Future(_) => "future",
                    TypeShape::List(_) => "list",
                    TypeShape::Iterable(_) => "iterable",
                    _ => "nullable",
                };
                write!(output, "['{tag}',").unwrap();
                descriptor(element, output);
                output.push(']');
            }
            TypeShape::Map { key, value } => {
                output.push_str("['map',");
                descriptor(key, output);
                output.push(',');
                descriptor(value, output);
                output.push(']');
            }
            TypeShape::Record { positional, named } => {
                output.push_str("['record',[");
                for (index, field) in positional.iter().enumerate() {
                    if index > 0 {
                        output.push(',');
                    }
                    descriptor(*field, output);
                }
                output.push_str("],[");
                for (index, (name, field)) in named.iter().enumerate() {
                    if index > 0 {
                        output.push(',');
                    }
                    output.push('[');
                    string_literal(name, output);
                    output.push(',');
                    descriptor(*field, output);
                    output.push(']');
                }
                output.push_str("]]");
            }
            TypeShape::Function { result, parameters } => {
                function_descriptor(result, &parameters, output)
            }
        },
        _ => output.push_str(match ty {
            Type::Duration => "['duration']",
            Type::Timer => "['timer']",
            Type::Int => "['int']",
            Type::String => "['string']",
            Type::Bool => "['bool']",
            Type::Object => "['object']",
            Type::Null => "['null']",
            Type::Void => "['void']",
            _ => panic!("tipo inferido não resolvido na emissão de descritor"),
        }),
    }
}

/// Conserva covariância de retorno e contravariância dos parâmetros no runtime.
pub(super) fn function_descriptor(result: Type, parameters: &[Type], output: &mut Output<'_>) {
    output.push_str("['function',");
    descriptor(result, output);
    output.push_str(",[");
    for (index, parameter) in parameters.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        descriptor(*parameter, output);
    }
    output.push_str("]]");
}

/// Obtém o tipo real do elemento inferido, sem inspecionar os valores da lista vazia.
pub(super) fn element_type(expression: &Expr<'_>, output: &Output<'_>) -> Type {
    collection_element(expression, output).expect("coleção sem tipo resolvido na HIR JavaScript")
}

/// Reconhece coleções pelo tipo estático e não pelo nome coincidente de um método.
pub(super) fn collection_element(expression: &Expr<'_>, output: &Output<'_>) -> Option<Type> {
    let ty = output
        .resolution
        .expr_types
        .get(&(expression.span.start, expression.span.end));
    if let Some(Type::Applied(id)) = ty
        && let TypeShape::List(element) | TypeShape::Iterable(element) =
            output.resolution.types[*id as usize]
    {
        return Some(element);
    }
    None
}

/// Registra tipos nominais e funções sem mudar identidade observável dos objetos.
pub(super) fn metadata(module: &Module<'_>, output: &mut Output<'_>) {
    output.push_str(if module.main_is_async {
        "$dartforgeTyped(main,['function',['future',['void']],[]]);\n"
    } else {
        "$dartforgeTyped(main,['function',['void'],[]]);\n"
    });
    let members: std::collections::BTreeMap<_, _> = output
        .nominal_members
        .iter()
        .map(|(id, members)| (*id, members.clone()))
        .collect();
    writeln!(
        output,
        "$dartforgeNominalMembers={};",
        serde_json::to_string(&members).unwrap()
    )
    .unwrap();
    for class in &module.classes {
        if class.enum_values.is_empty() {
            writeln!(
                output,
                "$dartforgeTyped($dartforgeClass{}.prototype,['class',{}]);",
                class.id, class.id
            )
            .unwrap();
        }
    }
    for function in &module.functions {
        // O descritor reificado só descreve posicionais obrigatórios; a análise
        // semântica já rejeita tear-offs de assinaturas com grupo opcional.
        if function
            .parameters
            .iter()
            .any(|parameter| parameter.kind != dartforge_syntax::ParameterKind::RequiredPositional)
        {
            continue;
        }
        if function.type_parameters.is_empty() {
            output.push_str("$dartforgeTyped(");
            identifier(function.name, output);
            output.push(',');
            function_descriptor(
                function.return_type,
                &function
                    .parameters
                    .iter()
                    .map(|parameter| parameter.ty)
                    .collect::<Vec<_>>(),
                output,
            );
            output.push_str(");\n");
        }
    }
}
