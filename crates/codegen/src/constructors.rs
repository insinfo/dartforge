//! Construção explícita preserva a ordem de inicialização do Dart.
//!
//! A cadeia é emitida em duas camadas. `$dartforgeNew{id}[${nome}]` aloca o
//! objeto e devolve-o; `$dartforgeInit{id}[${nome}]` executa, nesta ordem, os
//! inicializadores de declaração da própria classe, os formais `this.campo`, a
//! lista de inicialização, a construção da base e, por último, o corpo. Como a
//! base é inicializada no meio da função derivada, o corpo da base termina
//! antes de o corpo da derivada começar — exatamente o que o Dart 3.6.2 faz.
use super::*;
use dartforge_syntax::{Constructor, ConstructorExtras, ConstructorParameter};
use std::collections::{HashMap, HashSet};

/// Nome do receptor dentro das funções de inicialização.
const THIS: &str = "$dartforgeThis";

/// Seleciona em ordem topológica apenas classes cuja cadeia contém construtor explícito.
pub(super) fn factory_ids(classes: &[Class<'_>]) -> HashSet<u32> {
    let mut selected = HashSet::new();
    for index in class_order(classes) {
        let class = &classes[index];
        if class.constructor.is_some()
            || !class.named_constructors.is_empty()
            || class
                .superclass
                .is_some_and(|base| selected.contains(&base))
        {
            selected.insert(class.id);
        }
    }
    selected
}

/// Classes que precisam de função de inicialização própria.
///
/// São as selecionadas por [`factory_ids`] mais todas as suas bases, porque a
/// cadeia derivada chama a inicialização da base mesmo quando esta não declara
/// construtor algum.
fn init_ids(classes: &[Class<'_>], output: &Output<'_>) -> HashSet<u32> {
    let by_id: HashMap<_, _> = classes.iter().map(|class| (class.id, class)).collect();
    let mut selected = HashSet::new();
    for class in classes {
        if !output.constructor_factories.contains(&class.id) {
            continue;
        }
        let mut current = Some(class.id);
        while let Some(id) = current {
            if !selected.insert(id) {
                break;
            }
            current = by_id.get(&id).and_then(|class| class.superclass);
        }
    }
    selected
}

/// Emite entradas e funções de inicialização de toda a hierarquia.
pub(super) fn emit(classes: &[Class<'_>], output: &mut Output<'_>) {
    let by_id: HashMap<_, _> = classes.iter().map(|class| (class.id, class)).collect();
    let initializers = init_ids(classes, output);
    for class in classes {
        if class.is_library_globals || !output.constructor_factories.contains(&class.id) {
            continue;
        }
        if let Some(constructor) = &class.constructor {
            entry(class, None, &constructor.parameters, output);
        }
        for declared in &class.named_constructors {
            entry(
                class,
                Some(declared.name),
                &declared.constructor.parameters,
                output,
            );
        }
    }
    let plain = ConstructorExtras::default();
    for class in classes {
        if class.is_library_globals || !initializers.contains(&class.id) {
            continue;
        }
        if let Some(constructor) = &class.constructor {
            let extras = class.constructor_extras.as_deref().unwrap_or(&plain);
            initialize(class, None, Some(constructor), extras, &by_id, output);
        } else if class.named_constructors.is_empty() {
            initialize(class, None, None, &plain, &by_id, output);
        }
        for declared in &class.named_constructors {
            initialize(
                class,
                Some(declared.name),
                Some(&declared.constructor),
                &declared.extras,
                &by_id,
                output,
            );
        }
    }
}

/// Escreve `$dartforgeNew{id}[${nome}]`, que aloca sem executar construtor JS.
fn entry(
    class: &Class<'_>,
    name: Option<&str>,
    parameters: &[ConstructorParameter<'_>],
    output: &mut Output<'_>,
) {
    write!(output, "function $dartforgeNew{}", class.id).unwrap();
    if let Some(name) = name {
        identifier(name, output);
    }
    output.push('(');
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
    output.push_str("  $dartforgeInit");
    write!(output, "{}", class.id).unwrap();
    if let Some(name) = name {
        identifier(name, output);
    }
    output.push_str("($dartforgeObject");
    for index in 0..parameters.len() {
        write!(output, ", $dartforgeArgument{index}").unwrap();
    }
    output.push_str(");\n  return $dartforgeObject;\n}\n");
}

/// Escreve o parâmetro que recebe o valor do índice indicado do construtor.
fn init_parameter(parameter: &ConstructorParameter<'_>, index: usize, output: &mut Output<'_>) {
    if parameter.field.is_some() {
        // Um formal `this.campo` não introduz variável local no corpo.
        write!(output, "$dartforgeFormal{index}").unwrap();
    } else {
        identifier(parameter.name, output);
    }
}

/// Escreve `$dartforgeInit{id}[${nome}]` com a ordem de inicialização do Dart.
fn initialize(
    class: &Class<'_>,
    name: Option<&str>,
    constructor: Option<&Constructor<'_>>,
    extras: &ConstructorExtras<'_>,
    by_id: &HashMap<u32, &Class<'_>>,
    output: &mut Output<'_>,
) {
    let parameters = constructor.map_or(&[][..], |declared| declared.parameters.as_slice());
    output.push_str("function $dartforgeInit");
    write!(output, "{}", class.id).unwrap();
    if let Some(name) = name {
        identifier(name, output);
    }
    write!(output, "({THIS}").unwrap();
    for (index, parameter) in parameters.iter().enumerate() {
        output.push(',');
        init_parameter(parameter, index, output);
    }
    output.push_str(") {\n");
    // No Dart um formal `this.campo` também é parâmetro, e a lista de
    // inicialização o lê pelo nome escrito: `C(this.x) : assert(x > 0)` é
    // legal. O alias existe só quando há asserção, para que o caminho comum
    // continue emitindo exatamente o mesmo texto de antes.
    if !extras.asserts.is_empty() {
        for (index, parameter) in parameters.iter().enumerate() {
            if parameter.field.is_some() {
                output.push_str("  const ");
                identifier(parameter.name, output);
                writeln!(output, " = $dartforgeFormal{index};").unwrap();
            }
        }
    }
    // Um redirecionador delega inteiramente: não inicializa campo algum, não
    // chama `super` e não executa corpo próprio. Só as asserções da lista
    // rodam aqui, antes de o alvo montar o objeto — a mesma ordem do Dart.
    if let Some(call) = &extras.redirect {
        // Todas, sem filtrar por posição: um redirecionador não tem entrada
        // `campo = valor` com que intercalar, então `before` é sempre zero.
        for entry in &extras.asserts {
            output.push_str("  ");
            fluxo::assert_statement(&entry.condition, entry.message.as_ref(), output);
        }
        output.push_str("  $dartforgeInit");
        write!(output, "{}", class.id).unwrap();
        if let Some(target) = call.name {
            identifier(target, output);
        }
        write!(output, "({THIS}").unwrap();
        forward_arguments(
            constructor_parameters(class, call.name),
            &call.arguments,
            output,
        );
        output.push_str(");\n}\n");
        return;
    }
    for field in &class.fields {
        let formal = parameters.iter().position(|p| p.field == Some(field.name));
        if let Some(initializer) = &field.initializer {
            write!(output, "  {THIS}.").unwrap();
            identifier(field.name, output);
            output.push_str(" = ");
            expression(initializer, output);
            output.push_str(";\n");
        } else if formal.is_none() {
            write!(output, "  {THIS}.").unwrap();
            identifier(field.name, output);
            output.push_str(" = null;\n");
        }
        if let Some(index) = formal {
            write!(output, "  {THIS}.").unwrap();
            identifier(field.name, output);
            writeln!(output, " = $dartforgeFormal{index};").unwrap();
        }
    }
    for (index, entry) in extras.initializers.iter().enumerate() {
        // `assert` e `campo = valor` são entradas da mesma lista e o Dart
        // avalia uma depois da outra: `before` diz quantas entradas de campo
        // precedem cada asserção, e é essa ordem que a emissão preserva.
        asserts_before(extras, index, output);
        write!(output, "  {THIS}.").unwrap();
        identifier(entry.field, output);
        output.push_str(" = ");
        expression(&entry.value, output);
        output.push_str(";\n");
    }
    asserts_before(extras, extras.initializers.len(), output);
    if let Some(base) = class.superclass {
        output.push_str("  $dartforgeInit");
        write!(output, "{base}").unwrap();
        let target = extras.super_call.as_ref().and_then(|call| call.name);
        if let Some(target) = target {
            identifier(target, output);
        }
        write!(output, "({THIS}").unwrap();
        super_arguments(base, target, extras, by_id, output);
        output.push_str(");\n");
    }
    if let Some(body) = constructor.filter(|declared| !declared.body.is_empty()) {
        // O corpo enxerga os parâmetros comuns pelo nome interno; formais
        // `this.campo` não estão em escopo no corpo e chegam renomeados sem
        // uso. Os valores repassados são as ligações da própria `Init`.
        output.push_str("  (function(");
        for (index, parameter) in parameters.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            init_parameter(parameter, index, output);
        }
        output.push_str(") {\n");
        block(&body.body, 2, output);
        write!(output, "\n  }}).call({THIS}").unwrap();
        for (index, parameter) in parameters.iter().enumerate() {
            output.push_str(", ");
            init_parameter(parameter, index, output);
        }
        output.push_str(");\n");
    }
    output.push_str("}\n");
}

/// Emite as asserções da lista de inicialização escritas nesta posição.
///
/// A asserção da lista é a mesma construção do `assert` de instrução — inclusive
/// a mensagem, que neste subconjunto não carrega arquivo, linha e texto da
/// condição como a do SDK — e roda antes do corpo e antes de `super`.
fn asserts_before(extras: &ConstructorExtras<'_>, position: usize, output: &mut Output<'_>) {
    for entry in extras
        .asserts
        .iter()
        .filter(|entry| entry.before == position)
    {
        output.push_str("  ");
        fluxo::assert_statement(&entry.condition, entry.message.as_ref(), output);
    }
}

/// Parâmetros do construtor designado da classe: o sem nome ou um nomeado.
///
/// Devolve uma fatia vazia quando o construtor não existe; a análise semântica
/// já recusou esse caso, e devolver vazio mantém a emissão total sem `panic`.
fn constructor_parameters<'a, 'b>(
    class: &'a Class<'b>,
    name: Option<&str>,
) -> &'a [ConstructorParameter<'b>] {
    match name {
        None => class
            .constructor
            .as_ref()
            .map_or(&[][..], |declared| declared.parameters.as_slice()),
        Some(name) => class
            .named_constructors
            .iter()
            .find(|declared| declared.name == name)
            .map_or(&[][..], |declared| {
                declared.constructor.parameters.as_slice()
            }),
    }
}

/// Repassa os argumentos de `super` na ordem declarada pela base.
///
/// Um rótulo escrito na chamada é reposicionado para o índice do parâmetro
/// correspondente da base; o que a chamada omite recebe o padrão declarado ou
/// `null`, porque o JavaScript não tem o `undefined` do Dart.
fn super_arguments(
    base: u32,
    target: Option<&str>,
    extras: &ConstructorExtras<'_>,
    by_id: &HashMap<u32, &Class<'_>>,
    output: &mut Output<'_>,
) {
    let Some(class) = by_id.get(&base) else {
        return;
    };
    let parameters = constructor_parameters(class, target);
    let arguments = extras
        .super_call
        .as_ref()
        .map_or(&[][..], |call| call.arguments.as_slice());
    forward_arguments(parameters, arguments, output);
}

/// Casa os argumentos escritos com os parâmetros declarados pelo alvo.
///
/// Serve tanto a `super` quanto ao redirecionamento `: this(...)`: nos dois
/// casos o alvo é uma função de inicialização com a posição fixa de cada
/// parâmetro, e um rótulo escrito na chamada precisa ir para essa posição.
fn forward_arguments(
    parameters: &[ConstructorParameter<'_>],
    arguments: &[Expr<'_>],
    output: &mut Output<'_>,
) {
    let mut positional = 0usize;
    for parameter in parameters {
        output.push_str(", ");
        let written = if parameter.kind.is_named() {
            arguments.iter().find_map(|argument| match &argument.kind {
                ExprKind::NamedArgument { label, value } if *label == parameter.label() => {
                    Some(&**value)
                }
                _ => None,
            })
        } else {
            let found = arguments
                .get(positional)
                .filter(|argument| !matches!(argument.kind, ExprKind::NamedArgument { .. }));
            positional += 1;
            found
        };
        match written {
            Some(value) => expression(value, output),
            None => match parameter.default.as_deref() {
                Some(default) => literal_default(default, output),
                None => output.push_str("null"),
            },
        }
    }
}
