//! Construção explícita preserva campos derivados antes dos corpos das bases.
use super::*;
use std::collections::{HashMap, HashSet};

/// Seleciona em ordem topológica apenas classes cuja cadeia contém construtor explícito.
pub(super) fn factory_ids(classes: &[Class<'_>]) -> HashSet<u32> {
    let mut selected = HashSet::new();
    for index in class_order(classes) {
        let class = &classes[index];
        if class.constructor.is_some()
            || class
                .superclass
                .is_some_and(|base| selected.contains(&base))
        {
            selected.insert(class.id);
        }
    }
    selected
}

/// Aloca sem executar construtores JS e depois aplica a ordem de inicialização Dart.
/// Os argumentos temporários não ocultam funções globais usadas nos campos.
pub(super) fn emit(classes: &[Class<'_>], output: &mut Output<'_>) {
    let by_id: HashMap<_, _> = classes.iter().map(|class| (class.id, class)).collect();
    for class in classes {
        if !output.constructor_factories.contains(&class.id) {
            continue;
        }
        write!(output, "function $dartforgeNew{}(", class.id).unwrap();
        let parameters = class
            .constructor
            .as_ref()
            .map_or(&[][..], |ctor| ctor.parameters.as_slice());
        // Posicionais mantêm a posição declarada; nomeados chegam pelo objeto
        // desestruturado, sempre ligados ao mesmo `$dartforgeArgument{índice}`.
        let mut wrote = false;
        for (index, parameter) in parameters.iter().enumerate() {
            if parameter.kind.is_named() {
                continue;
            }
            if wrote {
                output.push(',');
            }
            wrote = true;
            write!(output, "$dartforgeArgument{index}").unwrap();
            default_value(parameter.kind, parameter.default.as_deref(), output);
        }
        if parameters.iter().any(|parameter| parameter.kind.is_named()) {
            if wrote {
                output.push(',');
            }
            output.push('{');
            let mut first = true;
            for (index, parameter) in parameters.iter().enumerate() {
                if !parameter.kind.is_named() {
                    continue;
                }
                if !first {
                    output.push(',');
                }
                first = false;
                write!(
                    output,
                    "{NAMED_KEY}{}: $dartforgeArgument{index}",
                    parameter.label()
                )
                .unwrap();
                default_value(parameter.kind, parameter.default.as_deref(), output);
            }
            output.push_str("} = {}");
        }
        writeln!(
            output,
            ") {{\n  const $dartforgeObject = Object.create($dartforgeClass{}.prototype);",
            class.id
        )
        .unwrap();
        let mut chain = vec![class];
        while let Some(base) = chain.last().unwrap().superclass {
            chain.push(by_id[&base]);
        }
        for owner in &chain {
            for field in &owner.fields {
                let formal = if owner.id == class.id {
                    parameters.iter().position(|p| p.field == Some(field.name))
                } else {
                    None
                };
                if let Some(initializer) = &field.initializer {
                    output.push_str("  $dartforgeObject.");
                    identifier(field.name, output);
                    output.push_str(" = ");
                    expression(initializer, output);
                    output.push_str(";\n");
                } else if formal.is_none() {
                    output.push_str("  $dartforgeObject.");
                    identifier(field.name, output);
                    output.push_str(" = null;\n");
                }
                if let Some(index) = formal {
                    output.push_str("  $dartforgeObject.");
                    identifier(field.name, output);
                    writeln!(output, " = $dartforgeArgument{index};").unwrap();
                }
            }
        }
        for owner in chain.iter().rev() {
            let Some(ctor) = &owner.constructor else {
                continue;
            };
            if ctor.body.is_empty() {
                continue;
            }
            output.push_str("  (function(");
            for (index, parameter) in ctor.parameters.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                if parameter.field.is_some() {
                    write!(output, "$dartforgeFormal{index}").unwrap();
                } else {
                    identifier(parameter.name, output);
                }
            }
            output.push_str(") {\n");
            block(&ctor.body, 2, output);
            output.push_str("\n  }).call($dartforgeObject");
            if owner.id == class.id {
                for index in 0..parameters.len() {
                    write!(output, ", $dartforgeArgument{index}").unwrap();
                }
            }
            output.push_str(");\n");
        }
        output.push_str("  return $dartforgeObject;\n}\n");
    }
}
