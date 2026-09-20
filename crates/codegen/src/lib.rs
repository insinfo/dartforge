//! Gera módulos JavaScript para o subconjunto Dart validado semanticamente.
//!
//! Literais inteiros são i32, mas operações usam Number do JavaScript; precisão
//! e transbordamento ainda não equivalem à semântica completa de inteiros Dart.
//! Parênteses preservam a árvore e a ordem de avaliação. Negação e multiplicação
//! normalizam zero inteiro para zero positivo. Não reproduzimos a variação de
//! impressão de zero negativo observada no dart2js 3.6.2 conforme outros tipos
//! impressos no mesmo programa. Laços nativos preservam break e continue.
//! A asserção de não nulo avalia seu operando uma vez e lança TypeError do
//! JavaScript para null; isso não implementa integralmente o TypeError Dart.
//! Classes usam métodos nativos e construtor sem argumentos que preserva a ordem Dart, com
//! herança única previamente validada. Não incluem reflexão ou runtimeType Dart.
use dartforge_hir::Module;
use dartforge_syntax::{
    BinaryOp, Class, Expr, ExprKind, Function, Statement, StatementKind, Type, UnaryOp,
};
use std::fmt::Write;

/// Estado local de emissão com a resolução estática fornecida pela análise.
struct Output<'a> {
    text: String,
    resolution: &'a dartforge_syntax::Resolution,
    collections: bool,
    modulo_used: bool,
}
impl std::ops::Deref for Output<'_> {
    type Target = String;
    /// Permite consultar o texto sem copiar o buffer.
    fn deref(&self) -> &String {
        &self.text
    }
}
impl std::ops::DerefMut for Output<'_> {
    /// Encaminha escrita ao buffer único do módulo.
    fn deref_mut(&mut self) -> &mut String {
        &mut self.text
    }
}

/// Gera um módulo ES com funções, exportação de main e chamada inicial de main.
///
/// O módulo deve ter passado pela análise semântica: nomes válidos, tipos
/// compatíveis, break/continue dentro de laços e cabeçalhos de for válidos.
/// A resolução deve incluir cada chamada de extension selecionada semanticamente;
/// chamadas sem entrada são tratadas como métodos de instância. O emissor não
/// infere tipos nem escolhe extensions. Não inclui runtime Dart completo, mapas
/// de origem ou otimizações globais.
///
/// # Pânicos
///
/// Falha se um cabeçalho de for contém uma instrução incompatível. A inicialização
/// aceita variável, atribuição ou expressão; a atualização aceita somente as
/// duas últimas. Também falha em hierarquias cíclicas ou bases ausentes.
/// Essas falhas indicam uma violação do contrato interno da AST.
///
/// ```
/// use dartforge_hir::lower;
/// use dartforge_syntax::Program;
/// let module = lower(Program { types: vec![], extensions: vec![], classes: vec![], functions: vec![], statements: vec![] });
/// let javascript = dartforge_codegen::emit(&module);
/// assert!(javascript.contains("export function main()"));
/// assert!(javascript.ends_with("main();\n"));
/// ```
pub fn emit(module: &Module<'_>) -> String {
    let mut output = Output {
        text: String::from("// Saída do subconjunto DartForge\n"),
        resolution: &module.resolution,
        modulo_used: false,
        collections: module.resolution.types.iter().any(|t| {
            matches!(
                t,
                dartforge_syntax::TypeShape::List(_) | dartforge_syntax::TypeShape::Iterable(_)
            )
        }),
    };
    if output.collections {
        output.push_str(include_str!("core.js"));
    }

    if module.extensions.iter().any(|extension| {
        extension
            .methods
            .iter()
            .any(|method| statements_need_null_assert(&method.body))
    }) || statements_need_null_assert(&module.statements)
        || module
            .functions
            .iter()
            .any(|function| statements_need_null_assert(&function.body))
        || module.classes.iter().any(|class| {
            class
                .fields
                .iter()
                .any(|field| expression_needs_null_assert(&field.initializer))
                || class
                    .methods
                    .iter()
                    .any(|method| statements_need_null_assert(&method.body))
        })
    {
        // Nome fora do prefixo $df_ reservado aos identificadores do usuário.
        output.push_str("function $dartforgeNullAssert(value) {\n  if (value === null) { throw new TypeError(\"Null check operator used on a null value\"); }\n  return value;\n}\n");
    }
    for extension in &module.extensions {
        for (index, method) in extension.methods.iter().enumerate() {
            write!(
                output,
                "function $dartforgeExtension{}Method{}(",
                extension.id, index
            )
            .expect("escrever em String não falha");
            for (index, parameter) in method.parameters.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                identifier(parameter.name, &mut output);
            }
            output.push_str(") ");
            function_body(method, 0, &mut output);
            output.push('\n');
        }
    }
    emit_classes(&module.classes, &mut output);
    for function in &module.functions {
        output.push_str("function ");
        identifier(function.name, &mut output);
        output.push('(');
        for (index, parameter) in function.parameters.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            identifier(parameter.name, &mut output);
        }
        output.push_str(") ");
        function_body(function, 0, &mut output);
        output.push('\n');
    }
    output.push_str("export function main() {\n");
    statements(&module.statements, 1, &mut output);
    output.push_str("}\n");
    if output.modulo_used {
        output.push_str("function $dartforgeModulo(a,b) { if (b === 0) throw new RangeError('Integer division by zero'); const d = Math.abs(b), r = a % d; return r < 0 ? r + d : r + 0; }\n");
    }
    output.push_str("const $df_main = main;\nmain();\n");
    output.text
}

/// Ordena a herança em O(V+E) esperado, com IDs esparsos e sem recursão.
///
/// A fila e as listas de filhos seguem a ordem da fonte; não iteramos o HashMap.
/// Inicializadores continuam dentro dos construtores e não são reordenados.
fn class_order(classes: &[Class<'_>]) -> Vec<usize> {
    use std::collections::{HashMap, VecDeque};
    let mut indexes = HashMap::with_capacity(classes.len());
    for (index, class) in classes.iter().enumerate() {
        assert!(
            indexes.insert(class.id, index).is_none(),
            "AST inválida: ID de classe duplicado"
        );
    }
    let mut children = vec![Vec::new(); classes.len()];
    let mut ready = VecDeque::new();
    for (index, class) in classes.iter().enumerate() {
        if let Some(base) = class.superclass {
            let parent = *indexes
                .get(&base)
                .expect("AST inválida: classe base ausente");
            children[parent].push(index);
        } else {
            ready.push_back(index);
        }
    }
    let mut order = Vec::with_capacity(classes.len());
    while let Some(index) = ready.pop_front() {
        order.push(index);
        ready.extend(children[index].iter().copied());
    }
    assert_eq!(order.len(), classes.len(), "AST inválida: ciclo na herança");
    order
}

/// Emite classes em ordem de herança, inclusive quando a base aparece depois.
fn emit_classes(classes: &[Class<'_>], output: &mut Output<'_>) {
    for index in class_order(classes) {
        let class = &classes[index];
        if !class.enum_values.is_empty() {
            write!(
                output,
                "const $dartforgeClass{} = Object.freeze({{",
                class.id
            )
            .unwrap();
            for (index, name) in class.enum_values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                identifier(name, output);
                output.push_str(":Object.freeze({$df_name:");
                string_literal(name, output);
                write!(output, ",$df_index:{index}}})").unwrap();
            }
            output.push_str("});\n");
            continue;
        }
        write!(output, "class $dartforgeClass{}", class.id).expect("escrever em String não falha");
        if let Some(base) = class.superclass {
            write!(output, " extends $dartforgeClass{base}").expect("escrever em String não falha");
        }
        output.push_str(" {\n");
        indent(1, output);
        output.push_str("constructor() {\n");
        // Dart avalia os campos da classe derivada antes dos campos da base.
        // Temporários não usam this antes de super, como exige o JavaScript.
        for (index, field) in class.fields.iter().enumerate() {
            indent(2, output);
            write!(output, "const $dartforgeField{index} = ")
                .expect("escrever em String não falha");
            expression(&field.initializer, output);
            output.push_str(";\n");
        }
        if class.superclass.is_some() {
            indent(2, output);
            output.push_str("super();\n");
        }
        for (index, field) in class.fields.iter().enumerate() {
            indent(2, output);
            output.push_str("this.");
            identifier(field.name, output);
            writeln!(output, " = $dartforgeField{index};").expect("escrever em String não falha");
        }
        indent(1, output);
        output.push_str("}\n");
        for method in &class.methods {
            indent(1, output);
            identifier(method.name, output);
            output.push('(');
            for (index, parameter) in method.parameters.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                identifier(parameter.name, output);
            }
            output.push_str(") ");
            function_body(method, 1, output);
            output.push('\n');
        }
        output.push_str("}\n");
    }
}

/// Separa parâmetros dos locais e produz null no retorno nullable implícito.
fn function_body(function: &Function<'_>, depth: usize, output: &mut Output<'_>) {
    output.push_str("{\n");
    // O bloco interno permite que um local Dart sombreie um parâmetro JavaScript.
    indent(depth + 1, output);
    block(&function.body, depth + 1, output);
    output.push('\n');
    if matches!(
        function.return_type,
        Type::Null
            | Type::NullableInt
            | Type::NullableString
            | Type::NullableBool
            | Type::NullableClass(_)
    ) {
        indent(depth + 1, output);
        output.push_str("return null;\n");
    }
    indent(depth, output);
    output.push('}');
}

/// Detecta asserções em qualquer expressão, inclusive argumentos e operandos.
fn expression_needs_null_assert(value: &Expr<'_>) -> bool {
    match &value.kind {
        ExprKind::Closure { body, .. } => statements_need_null_assert(body),
        ExprKind::List { elements, .. } => elements.iter().any(expression_needs_null_assert),
        ExprKind::Index { receiver, index } => {
            expression_needs_null_assert(receiver) || expression_needs_null_assert(index)
        }
        ExprKind::Invoke { callee, arguments } => {
            expression_needs_null_assert(callee)
                || arguments.iter().any(expression_needs_null_assert)
        }
        ExprKind::Unary { op, operand } => {
            *op == UnaryOp::NullAssert || expression_needs_null_assert(operand)
        }
        ExprKind::Binary { left, right, .. } => {
            expression_needs_null_assert(left) || expression_needs_null_assert(right)
        }
        ExprKind::Call { arguments, .. } => arguments.iter().any(expression_needs_null_assert),
        ExprKind::Member { receiver, .. } => expression_needs_null_assert(receiver),
        ExprKind::MethodCall {
            receiver,
            arguments,
            ..
        } => {
            expression_needs_null_assert(receiver)
                || arguments.iter().any(expression_needs_null_assert)
        }
        _ => false,
    }
}

/// Percorre todos os corpos e cabeçalhos para incluir o auxiliar somente se usado.
fn statements_need_null_assert(body: &[Statement<'_>]) -> bool {
    body.iter().any(statement_needs_null_assert)
}

/// Verifica uma instrução sem alterar ordem de avaliação ou alocar cópias da AST.
fn statement_needs_null_assert(statement: &Statement<'_>) -> bool {
    match &statement.kind {
        StatementKind::IndexAssign {
            receiver,
            index,
            value,
        } => {
            expression_needs_null_assert(receiver)
                || expression_needs_null_assert(index)
                || expression_needs_null_assert(value)
        }
        StatementKind::Variable { initializer, .. } => expression_needs_null_assert(initializer),
        StatementKind::FieldAssign {
            receiver, value, ..
        } => expression_needs_null_assert(receiver) || expression_needs_null_assert(value),
        StatementKind::Assign { value, .. }
        | StatementKind::Print(value)
        | StatementKind::Expression(value) => expression_needs_null_assert(value),
        StatementKind::Return(value) => value.as_ref().is_some_and(expression_needs_null_assert),
        StatementKind::If {
            condition,
            then_body,
            else_body,
        } => {
            expression_needs_null_assert(condition)
                || statements_need_null_assert(then_body)
                || else_body
                    .as_ref()
                    .is_some_and(|body| statements_need_null_assert(body))
        }
        StatementKind::While { condition, body } | StatementKind::DoWhile { condition, body } => {
            expression_needs_null_assert(condition) || statements_need_null_assert(body)
        }
        StatementKind::For {
            initializer,
            condition,
            update,
            body,
        } => {
            initializer
                .as_deref()
                .is_some_and(statement_needs_null_assert)
                || condition.as_ref().is_some_and(expression_needs_null_assert)
                || update.as_deref().is_some_and(statement_needs_null_assert)
                || statements_need_null_assert(body)
        }
        StatementKind::Block(body) => statements_need_null_assert(body),
        StatementKind::Break | StatementKind::Continue => false,
    }
}

/// Aplica o prefixo estável usado em declarações e referências.
fn identifier(name: &str, output: &mut Output<'_>) {
    // O prefixo injetivo evita palavras reservadas e globais do JavaScript.
    // Blocos léxicos preservam sombreamento; referências recebem o mesmo prefixo.
    output.push_str("$df_");
    output.push_str(name);
}

/// Acrescenta dois espaços por nível léxico.
fn indent(depth: usize, output: &mut Output<'_>) {
    for _ in 0..depth {
        output.push_str("  ");
    }
}

/// Emite instruções na ordem original e preserva os respectivos escopos.
fn statements(body: &[Statement<'_>], depth: usize, output: &mut Output<'_>) {
    for statement in body {
        indent(depth, output);
        match &statement.kind {
            StatementKind::Variable {
                name,
                is_final,
                initializer,
                ..
            } => {
                output.push_str(if *is_final { "const " } else { "let " });
                identifier(name, output);
                output.push_str(" = ");
                expression(initializer, output);
                output.push_str(";\n");
            }
            StatementKind::Assign { name, value } => {
                identifier(name, output);
                output.push_str(" = ");
                expression(value, output);
                output.push_str(";\n");
            }
            StatementKind::FieldAssign {
                receiver,
                name,
                value,
            } => {
                expression(receiver, output);
                output.push('.');
                identifier(name, output);
                output.push_str(" = ");
                expression(value, output);
                output.push_str(";\n");
            }
            StatementKind::IndexAssign {
                receiver,
                index,
                value,
            } => {
                output.push_str("$dartforgeIndexSet(");
                expression(receiver, output);
                output.push(',');
                expression(index, output);
                output.push(',');
                expression(value, output);
                output.push_str(");\n");
            }
            StatementKind::Print(value) => {
                let collections = output.collections;
                output.push_str(if collections {
                    "console.log($dartforgeFormat("
                } else {
                    "console.log("
                });
                expression(value, output);
                let collections = output.collections;
                output.push_str(if collections { "));\n" } else { ");\n" });
            }
            StatementKind::Return(value) => {
                output.push_str("return");
                if let Some(value) = value {
                    output.push(' ');
                    expression(value, output);
                }
                output.push_str(";\n");
            }
            StatementKind::Expression(value) => {
                expression(value, output);
                output.push_str(";\n");
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                output.push_str("if (");
                expression(condition, output);
                output.push_str(") {\n");
                statements(then_body, depth + 1, output);
                indent(depth, output);
                output.push('}');
                if let Some(else_body) = else_body {
                    output.push_str(" else {\n");
                    statements(else_body, depth + 1, output);
                    indent(depth, output);
                    output.push('}');
                }
                output.push('\n');
            }
            StatementKind::While { condition, body } => {
                output.push_str("while (");
                expression(condition, output);
                output.push_str(") ");
                block(body, depth, output);
                output.push('\n');
            }
            StatementKind::DoWhile { body, condition } => {
                output.push_str("do ");
                block(body, depth, output);
                output.push_str(" while (");
                expression(condition, output);
                output.push_str(");\n");
            }
            StatementKind::For {
                initializer,
                condition,
                update,
                body,
            } => {
                output.push_str("for (");
                if let Some(initializer) = initializer {
                    for_clause(initializer, true, output);
                }
                output.push_str("; ");
                if let Some(condition) = condition {
                    expression(condition, output);
                }
                output.push_str("; ");
                if let Some(update) = update {
                    for_clause(update, false, output);
                }
                output.push_str(") ");
                block(body, depth, output);
                output.push('\n');
            }
            StatementKind::Break => output.push_str("break;\n"),
            StatementKind::Continue => output.push_str("continue;\n"),
            StatementKind::Block(body) => {
                output.push_str("{\n");
                statements(body, depth + 1, output);
                indent(depth, output);
                output.push_str("}\n");
            }
        }
    }
}

/// Emite um bloco sem recuo inicial ou quebra de linha final.
fn block(body: &[Statement<'_>], depth: usize, output: &mut Output<'_>) {
    output.push_str("{\n");
    statements(body, depth + 1, output);
    indent(depth, output);
    output.push('}');
}

/// Emite uma cláusula de for validada, sem separadores ou quebras de linha.
fn for_clause(statement: &Statement<'_>, allow_variable: bool, output: &mut Output<'_>) {
    match &statement.kind {
        StatementKind::Variable {
            name,
            is_final,
            initializer,
            ..
        } if allow_variable => {
            output.push_str(if *is_final { "const " } else { "let " });
            identifier(name, output);
            output.push_str(" = ");
            expression(initializer, output);
        }
        StatementKind::Assign { name, value } => {
            identifier(name, output);
            output.push_str(" = ");
            expression(value, output);
        }
        StatementKind::Expression(value) => expression(value, output),
        _ => panic!("AST inválida: instrução incompatível com cabeçalho de for"),
    }
}

/// Serializa texto já decodificado, preservando controles, Unicode e aspas.
///
/// O escape JSON impede que conteúdo da string seja interpretado como código
/// JavaScript. Strings emprestadas e alocadas seguem exatamente o mesmo caminho.
fn string_literal(value: &str, output: &mut Output<'_>) {
    output.push_str(&serde_json::to_string(value).expect("serializar uma string não falha"));
}

/// Emite uma expressão sem duplicar a avaliação de operandos.
fn expression(value: &Expr<'_>, output: &mut Output<'_>) {
    match &value.kind {
        ExprKind::Closure {
            parameters, body, ..
        } => {
            output.push_str("((");
            for (i, p) in parameters.iter().enumerate() {
                if i > 0 {
                    output.push(',');
                }
                identifier(p.name, output);
            }
            output.push_str(") => {\n");
            block(body, 1, output);
            output.push_str("\nreturn null;\n})");
        }
        ExprKind::List { elements, .. } => {
            output.push_str("new $dartforgeList([");
            for (i, e) in elements.iter().enumerate() {
                if i > 0 {
                    output.push(',');
                }
                expression(e, output);
            }
            output.push_str("])");
        }
        ExprKind::Index { receiver, index } => {
            output.push_str("$dartforgeIndex(");
            expression(receiver, output);
            output.push(',');
            expression(index, output);
            output.push(')');
        }
        ExprKind::Invoke { callee, arguments } => {
            output.push('(');
            expression(callee, output);
            output.push_str(")(");
            for (i, e) in arguments.iter().enumerate() {
                if i > 0 {
                    output.push(',');
                }
                expression(e, output);
            }
            output.push(')');
        }
        ExprKind::EnumValue { class_id, name } => {
            write!(output, "$dartforgeClass{class_id}.").unwrap();
            identifier(name, output);
        }
        ExprKind::Null => output.push_str("null"),
        ExprKind::This => output.push_str("this"),
        ExprKind::Construct { class_id } => {
            write!(output, "new $dartforgeClass{class_id}()")
                .expect("escrever em String não falha");
        }
        ExprKind::Member { receiver, name } => {
            expression(receiver, output);
            output.push('.');
            identifier(name, output);
        }
        ExprKind::MethodCall {
            receiver,
            name,
            arguments,
        } => {
            if let Some((extension, method)) = output
                .resolution
                .extension_calls
                .get(&(value.span.start, value.span.end))
                .map(|target| (target.extension_id, target.method_index))
            {
                write!(output, "$dartforgeExtension{extension}Method{method}.call(")
                    .expect("escrever em String não falha");
                expression(receiver, output);
                for argument in arguments {
                    output.push_str(", ");
                    expression(argument, output);
                }
                output.push(')');
                return;
            }
            expression(receiver, output);
            output.push('.');
            identifier(name, output);
            output.push('(');
            for (index, argument) in arguments.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                expression(argument, output);
            }
            output.push(')');
        }
        ExprKind::Int(value) => write!(output, "{value}").expect("escrever em String não falha"),
        ExprKind::String(value) => string_literal(value, output),
        ExprKind::OwnedString(value) => string_literal(value, output),
        ExprKind::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        ExprKind::Identifier(name) => identifier(name, output),
        ExprKind::Call { name, arguments } => {
            match *name {
                "main" => output.push_str("$df_main"),
                "print" => {
                    let collections = output.collections;
                    output.push_str(if collections {
                        "$dartforgePrint"
                    } else {
                        "console.log"
                    });
                }
                _ => identifier(name, output),
            }
            output.push('(');
            for (index, argument) in arguments.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                expression(argument, output);
            }
            output.push(')');
        }
        ExprKind::Unary {
            op: UnaryOp::NullAssert,
            operand,
        } => {
            output.push_str("$dartforgeNullAssert(");
            expression(operand, output);
            output.push(')');
        }
        ExprKind::Unary { op, operand } => {
            output.push('(');
            output.push_str(if *op == UnaryOp::Negate { "-" } else { "!" });
            output.push(' ');
            expression(operand, output);
            if *op == UnaryOp::Negate {
                // O operando é int: normaliza o zero negativo do JavaScript.
                // Somar zero preserva precisão e transbordamento de Number.
                output.push_str(" + 0");
            }
            output.push(')');
        }
        ExprKind::Binary {
            op: BinaryOp::Remainder,
            left,
            right,
        } => {
            output.modulo_used = true;
            output.push_str("$dartforgeModulo(");
            expression(left, output);
            output.push(',');
            expression(right, output);
            output.push(')');
        }
        ExprKind::Binary { op, left, right } => {
            output.push('(');
            expression(left, output);
            output.push_str(match op {
                BinaryOp::Add => " + ",
                BinaryOp::Subtract => " - ",
                BinaryOp::Multiply => " * ",
                BinaryOp::Remainder => unreachable!("emissão dedicada"),
                BinaryOp::Equal => " === ",
                BinaryOp::NotEqual => " !== ",
                BinaryOp::Less => " < ",
                BinaryOp::LessEqual => " <= ",
                BinaryOp::Greater => " > ",
                BinaryOp::GreaterEqual => " >= ",
                BinaryOp::And => " && ",
                BinaryOp::Or => " || ",
                BinaryOp::IfNull => " ?? ",
            });
            expression(right, output);
            if *op == BinaryOp::Multiply {
                // Um inteiro negativo multiplicado por zero continua sendo zero inteiro.
                output.push_str(" + 0");
            }
            output.push(')');
        }
    }
}

#[cfg(test)]
mod tests {
    /// Enums usam singletons congelados e nomes originais mesmo quando não são identificadores JS.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn enum_singletons_are_frozen_and_have_metadata() {
        let mut enumeration = empty_class(12, None);
        enumeration.enum_values = vec!["name", "other"];
        let program = Program {
            types: vec![],
            classes: vec![enumeration],
            extensions: vec![],
            functions: vec![],
            statements: vec![],
        };
        let module = dartforge_hir::lower(program);
        let mut js = emit(&module);
        js.push_str("console.log($dartforgeClass12.$df_name === $dartforgeClass12.$df_name); console.log($dartforgeClass12.$df_name === $dartforgeClass12.$df_other); console.log($dartforgeClass12.$df_name.$df_name); console.log($dartforgeClass12.$df_other.$df_index); console.log(Object.isFrozen($dartforgeClass12.$df_name));");
        let run = std::process::Command::new("node")
            .args(["--eval", &js])
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&run.stdout).replace("\r\n", "\n"),
            "true\nfalse\nname\n1\ntrue\n"
        );
    }
    use super::*;
    use dartforge_diagnostics::Span;
    use dartforge_syntax::{Function, Parameter, Program, Type};

    fn expr(kind: ExprKind<'_>) -> Expr<'_> {
        Expr {
            kind,
            span: Span { start: 0, end: 0 },
        }
    }
    fn statement(kind: StatementKind<'_>) -> Statement<'_> {
        Statement {
            kind,
            span: Span { start: 0, end: 0 },
        }
    }
    fn print(value: Expr<'_>) -> Statement<'_> {
        statement(StatementKind::Print(value))
    }
    fn binary<'a>(op: BinaryOp, left: Expr<'a>, right: Expr<'a>) -> Expr<'a> {
        expr(ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }
    fn variable<'a>(name: &'a str, is_final: bool, initializer: Expr<'a>) -> Statement<'a> {
        statement(StatementKind::Variable {
            name,
            annotation: None,
            is_final,
            initializer,
        })
    }
    fn compile(statements: Vec<Statement<'_>>) -> String {
        emit(&dartforge_hir::lower(Program {
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![],
            statements,
        }))
    }

    #[test]
    fn preserves_nested_expression_tree_and_unary_tokens() {
        let value = binary(
            BinaryOp::Multiply,
            binary(
                BinaryOp::Add,
                expr(ExprKind::Int(2)),
                expr(ExprKind::Int(3)),
            ),
            expr(ExprKind::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(expr(ExprKind::Int(-4))),
            }),
        );
        assert!(compile(vec![print(value)]).contains("console.log(((2 + 3) * (- -4 + 0) + 0));"));
    }

    #[test]
    fn protects_globals_keywords_and_nested_shadowing() {
        let output = compile(vec![
            variable("console", false, expr(ExprKind::Int(1))),
            variable("main", true, expr(ExprKind::Int(2))),
            variable("function", false, expr(ExprKind::Bool(true))),
            variable("$df_console", true, expr(ExprKind::Int(3))),
            statement(StatementKind::Block(vec![
                variable("console", false, expr(ExprKind::Int(4))),
                statement(StatementKind::Assign {
                    name: "console",
                    value: expr(ExprKind::Int(5)),
                }),
                print(expr(ExprKind::Identifier("console"))),
            ])),
            print(expr(ExprKind::Identifier("console"))),
        ]);
        assert!(output.contains("let $df_console = 1;"));
        assert!(output.contains("const $df_main = 2;"));
        assert!(output.contains("let $df_function = true;"));
        assert!(output.contains("const $df_$df_console = 3;"));
        assert!(output.contains("  {\n    let $df_console = 4;\n    $df_console = 5;\n    console.log($df_console);\n  }\n  console.log($df_console);"));
        assert!(output.ends_with("}\nconst $df_main = main;\nmain();\n"));
    }

    #[test]
    fn strings_cannot_break_out_of_javascript_literal() {
        let value = "\"\\\n\r\t'); throw Error('bad'); // 🦀";
        let output = compile(vec![print(expr(ExprKind::String(value)))]);
        let serialized = output
            .split("console.log(")
            .nth(1)
            .unwrap()
            .split(");\n")
            .next()
            .unwrap();
        assert_eq!(serde_json::from_str::<String>(serialized).unwrap(), value);
    }

    #[test]
    fn maps_equality_and_short_circuit_operators() {
        for (op, text) in [
            (BinaryOp::Equal, " === "),
            (BinaryOp::NotEqual, " !== "),
            (BinaryOp::And, " && "),
            (BinaryOp::Or, " || "),
            (BinaryOp::LessEqual, " <= "),
            (BinaryOp::GreaterEqual, " >= "),
        ] {
            let output = compile(vec![print(binary(
                op,
                expr(ExprKind::Bool(true)),
                expr(ExprKind::Bool(false)),
            ))]);
            assert!(output.contains(&format!("(true{text}false)")));
        }
    }
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn numeric_int_zero_is_canonical_and_large_values_match_js() {
        let output = compile(vec![
            variable("zero", false, expr(ExprKind::Int(0))),
            variable("large", false, expr(ExprKind::Int(i32::MAX))),
            print(expr(ExprKind::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(expr(ExprKind::Identifier("zero"))),
            })),
            print(expr(ExprKind::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(expr(ExprKind::Int(0))),
            })),
            print(binary(
                BinaryOp::Multiply,
                expr(ExprKind::Identifier("large")),
                expr(ExprKind::Identifier("large")),
            )),
            print(binary(
                BinaryOp::Multiply,
                binary(
                    BinaryOp::Multiply,
                    expr(ExprKind::Identifier("large")),
                    expr(ExprKind::Identifier("large")),
                ),
                expr(ExprKind::Identifier("large")),
            )),
            variable(
                "negativeZero",
                false,
                expr(ExprKind::Unary {
                    op: UnaryOp::Negate,
                    operand: Box::new(expr(ExprKind::Int(0))),
                }),
            ),
            print(binary(
                BinaryOp::Multiply,
                expr(ExprKind::Int(-1)),
                expr(ExprKind::Identifier("negativeZero")),
            )),
            print(expr(ExprKind::Bool(true))),
            print(expr(ExprKind::String("plain"))),
        ]);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        // Mantém zero inteiro positivo também nas operações seguintes.
        // dart2js 3.6.2 imprime 0 em programas que imprimem apenas int, mas pode
        // imprimir -0.0 se outros tipos também forem impressos. Preservamos zero
        // inteiro estável sem reproduzir esse efeito da otimização global.
        // Os valores grandes seguem dart2js, não a aritmética de 64 bits da VM.
        assert_eq!(
            String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
            "0\n0\n4611686014132420600\n9.903520300447984e+27\n0\ntrue\nplain\n"
        );
    }
    fn call(name: &'static str, arguments: Vec<Expr<'static>>) -> Expr<'static> {
        expr(ExprKind::Call { name, arguments })
    }

    fn function(
        name: &'static str,
        return_type: Type,
        parameters: &[(&'static str, Type)],
        body: Vec<Statement<'static>>,
    ) -> Function<'static> {
        Function {
            name,
            return_type,
            parameters: parameters
                .iter()
                .map(|&(name, ty)| Parameter {
                    name,
                    ty,
                    span: Span { start: 0, end: 0 },
                })
                .collect(),
            body,
            span: Span { start: 0, end: 0 },
        }
    }

    fn functions_fixture() -> String {
        let functions = vec![
            function(
                "returnedPrint",
                Type::Void,
                &[],
                vec![statement(StatementKind::Return(Some(call(
                    "print",
                    vec![expr(ExprKind::String("returned print"))],
                ))))],
            ),
            function(
                "factorial",
                Type::Int,
                &[("n", Type::Int)],
                vec![statement(StatementKind::If {
                    condition: binary(
                        BinaryOp::LessEqual,
                        expr(ExprKind::Identifier("n")),
                        expr(ExprKind::Int(1)),
                    ),
                    then_body: vec![statement(StatementKind::Return(Some(expr(ExprKind::Int(
                        1,
                    )))))],
                    else_body: Some(vec![statement(StatementKind::Return(Some(binary(
                        BinaryOp::Multiply,
                        expr(ExprKind::Identifier("n")),
                        call(
                            "factorial",
                            vec![binary(
                                BinaryOp::Subtract,
                                expr(ExprKind::Identifier("n")),
                                expr(ExprKind::Int(1)),
                            )],
                        ),
                    ))))]),
                })],
            ),
            function(
                "mark",
                Type::Int,
                &[("console", Type::Int)],
                vec![
                    print(expr(ExprKind::Identifier("console"))),
                    statement(StatementKind::Return(Some(expr(ExprKind::Identifier(
                        "console",
                    ))))),
                ],
            ),
            function(
                "function",
                Type::Int,
                &[("main", Type::Int), ("second", Type::Int)],
                vec![statement(StatementKind::Return(Some(binary(
                    BinaryOp::Add,
                    binary(
                        BinaryOp::Multiply,
                        expr(ExprKind::Identifier("main")),
                        expr(ExprKind::Int(10)),
                    ),
                    expr(ExprKind::Identifier("second")),
                ))))],
            ),
            function(
                "stop",
                Type::Void,
                &[("flag", Type::Bool)],
                vec![
                    statement(StatementKind::If {
                        condition: expr(ExprKind::Identifier("flag")),
                        then_body: vec![
                            print(expr(ExprKind::String("stop"))),
                            statement(StatementKind::Return(None)),
                        ],
                        else_body: None,
                    }),
                    print(expr(ExprKind::String("go"))),
                ],
            ),
        ];
        let statements = vec![
            statement(StatementKind::Expression(call("returnedPrint", vec![]))),
            print(call("factorial", vec![expr(ExprKind::Int(5))])),
            print(call(
                "function",
                vec![
                    call("mark", vec![expr(ExprKind::Int(1))]),
                    call("mark", vec![expr(ExprKind::Int(2))]),
                ],
            )),
            statement(StatementKind::Expression(call(
                "stop",
                vec![expr(ExprKind::Bool(true))],
            ))),
            statement(StatementKind::Expression(call(
                "stop",
                vec![expr(ExprKind::Bool(false))],
            ))),
        ];
        emit(&dartforge_hir::lower(Program {
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions,
            statements,
        }))
    }

    #[test]
    fn emits_named_functions_parameters_returns_and_branches() {
        let output = functions_fixture();
        assert!(output.contains("function $df_function($df_main, $df_second) {"));
        assert!(output.contains("function $df_mark($df_console) {"));
        assert!(output.contains("if (($df_n <= 1)) {\n      return 1;\n    } else {"));
        assert!(output.contains("    return;\n"));
        assert!(output.contains("$df_stop(true);"));
        assert!(output.contains("return console.log(\"returned print\");"));
        assert!(!output.contains("$df_print("));
        let main_call = compile(vec![statement(StatementKind::Expression(call(
            "main",
            vec![],
        )))]);
        assert!(main_call.contains("  $df_main();\n"));
        assert!(main_call.contains("const $df_main = main;"));
    }

    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn functions_execute_recursion_ordered_arguments_and_early_return() {
        let output = functions_fixture();
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
            "returned print\n120\n1\n2\n12\nstop\ngo\n"
        );
    }
    fn assign(name: &'static str, value: Expr<'static>) -> Statement<'static> {
        statement(StatementKind::Assign { name, value })
    }
    fn name(value: &'static str) -> Expr<'static> {
        expr(ExprKind::Identifier(value))
    }
    fn int(value: i32) -> Expr<'static> {
        expr(ExprKind::Int(value))
    }
    fn when(condition: Expr<'static>, body: Vec<Statement<'static>>) -> Statement<'static> {
        statement(StatementKind::If {
            condition,
            then_body: body,
            else_body: None,
        })
    }
    fn loops_fixture() -> String {
        let functions = vec![
            function(
                "tick",
                Type::Int,
                &[("n", Type::Int)],
                vec![
                    print(expr(ExprKind::String("update"))),
                    print(name("n")),
                    statement(StatementKind::Return(Some(binary(
                        BinaryOp::Add,
                        name("n"),
                        int(1),
                    )))),
                ],
            ),
            function(
                "check",
                Type::Bool,
                &[("n", Type::Int)],
                vec![
                    print(expr(ExprKind::String("condition"))),
                    print(name("n")),
                    statement(StatementKind::Return(Some(binary(
                        BinaryOp::Less,
                        name("n"),
                        int(2),
                    )))),
                ],
            ),
        ];
        let statements = vec![
            variable("i", false, int(77)),
            statement(StatementKind::For {
                initializer: Some(Box::new(variable("i", false, int(0)))),
                condition: Some(binary(BinaryOp::Less, name("i"), int(3))),
                update: Some(Box::new(assign("i", call("tick", vec![name("i")])))),
                body: vec![
                    variable("j", false, int(0)),
                    statement(StatementKind::While {
                        condition: binary(BinaryOp::Less, name("j"), int(3)),
                        body: vec![
                            assign("j", binary(BinaryOp::Add, name("j"), int(1))),
                            when(
                                binary(BinaryOp::Equal, name("j"), int(1)),
                                vec![statement(StatementKind::Continue)],
                            ),
                            when(
                                binary(BinaryOp::Equal, name("j"), int(3)),
                                vec![statement(StatementKind::Break)],
                            ),
                            print(binary(
                                BinaryOp::Add,
                                binary(BinaryOp::Multiply, name("i"), int(10)),
                                name("j"),
                            )),
                        ],
                    }),
                    when(
                        binary(BinaryOp::Equal, name("i"), int(1)),
                        vec![statement(StatementKind::Continue)],
                    ),
                    print(name("i")),
                ],
            }),
            print(name("i")),
            variable("d", false, int(0)),
            statement(StatementKind::DoWhile {
                body: vec![
                    assign("d", binary(BinaryOp::Add, name("d"), int(1))),
                    statement(StatementKind::Continue),
                ],
                condition: call("check", vec![name("d")]),
            }),
            print(name("d")),
            statement(StatementKind::For {
                initializer: None,
                condition: None,
                update: None,
                body: vec![statement(StatementKind::Break)],
            }),
        ];
        emit(&dartforge_hir::lower(Program {
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions,
            statements,
        }))
    }

    #[test]
    fn loops_keep_native_control_flow_and_header_scope() {
        let output = loops_fixture();
        assert!(output.contains("for (let $df_i = 0; ($df_i < 3); $df_i = $df_tick($df_i)) {"));
        assert!(output.contains("while (($df_j < 3)) {"));
        assert!(output.contains("} while ($df_check($df_d));"));
        assert!(output.contains("for (; ; ) {"));
    }

    #[test]
    #[should_panic(expected = "AST inválida")]
    fn rejects_invalid_for_header_instead_of_emitting_broken_javascript() {
        compile(vec![statement(StatementKind::For {
            initializer: None,
            condition: None,
            update: Some(Box::new(variable("invalid", false, int(0)))),
            body: vec![],
        })]);
    }

    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn loops_execute_nested_break_continue_updates_and_conditions() {
        let output = loops_fixture();
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
            "2\n0\nupdate\n0\n12\nupdate\n1\n22\n2\nupdate\n2\n77\ncondition\n1\ncondition\n2\n2\n"
        );
    }
    #[test]
    fn borrowed_and_decoded_strings_emit_identical_literals() {
        let value = "aspas: \"' barra: \\ controles: \n\r\t\0 Unicode: 😀\u{2028}\u{2029}\"); process.exit(17); //";
        let borrowed = compile(vec![print(expr(ExprKind::String(value)))]);
        let owned = compile(vec![print(expr(ExprKind::OwnedString(value.to_owned())))]);
        assert_eq!(borrowed, owned);
        let literal = owned
            .split("console.log(")
            .nth(1)
            .unwrap()
            .split(");\n")
            .next()
            .unwrap();
        assert_eq!(serde_json::from_str::<String>(literal).unwrap(), value);
    }

    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn decoded_strings_execute_without_injection_or_unicode_loss() {
        let value = "aspas: \"' barra: \\ controles: \n\r\t\0 Unicode: 😀\u{2028}\u{2029}\"); process.exit(17); //";
        let output = compile(vec![
            print(expr(ExprKind::OwnedString(value.to_owned()))),
            print(expr(ExprKind::String(value))),
        ]);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        // Compara bytes: normalizar novas linhas esconderia corrupção de \r.
        assert_eq!(run.stdout, format!("{value}\n{value}\n").as_bytes());
    }
    /// Confere sombreamento de parâmetro sem declaração JavaScript duplicada.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn function_body_locals_can_shadow_parameters() {
        let module = dartforge_hir::lower(Program {
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![function(
                "f",
                Type::Int,
                &[("x", Type::Int)],
                vec![
                    variable("x", false, int(1)),
                    statement(StatementKind::Return(Some(name("x")))),
                ],
            )],
            statements: vec![print(call("f", vec![int(99)]))],
        });
        let output = emit(&module);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(run.stdout, b"1\n");
    }
    /// Monta uma asserção pós-fixa preservando o operando e seu efeito.
    fn asserted(value: Expr<'static>) -> Expr<'static> {
        expr(ExprKind::Unary {
            op: UnaryOp::NullAssert,
            operand: Box::new(value),
        })
    }

    /// Verifica inclusão sob demanda do auxiliar e agrupamento do operador ?? .
    #[test]
    fn null_helper_is_emitted_only_when_needed() {
        let plain = compile(vec![print(binary(
            BinaryOp::IfNull,
            expr(ExprKind::Null),
            int(4),
        ))]);
        assert!(plain.contains("console.log((null ?? 4));"));
        assert!(!plain.contains("$dartforgeNullAssert"));
        let module = dartforge_hir::lower(Program {
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![function(
                "checked",
                Type::Int,
                &[("n", Type::NullableInt)],
                vec![statement(StatementKind::Return(Some(asserted(name("n")))))],
            )],
            statements: vec![],
        });
        let checked = emit(&module);
        assert_eq!(checked.matches("function $dartforgeNullAssert(").count(), 1);
        assert!(checked.contains("return $dartforgeNullAssert($df_n);"));
    }

    /// Confere curto-circuito, valores falsy não nulos e avaliação única com efeitos.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn null_operators_preserve_lazy_evaluation_and_single_effects() {
        let module = dartforge_hir::lower(Program {
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![function(
                "mark",
                Type::NullableInt,
                &[("n", Type::NullableInt)],
                vec![
                    print(name("n")),
                    statement(StatementKind::Return(Some(name("n")))),
                ],
            )],
            statements: vec![
                print(binary(
                    BinaryOp::IfNull,
                    call("mark", vec![int(0)]),
                    call("mark", vec![int(9)]),
                )),
                print(binary(
                    BinaryOp::IfNull,
                    call("mark", vec![expr(ExprKind::Null)]),
                    call("mark", vec![int(7)]),
                )),
                print(asserted(call("mark", vec![int(8)]))),
                print(binary(
                    BinaryOp::IfNull,
                    expr(ExprKind::Bool(false)),
                    expr(ExprKind::Bool(true)),
                )),
                print(binary(
                    BinaryOp::IfNull,
                    expr(ExprKind::String("")),
                    expr(ExprKind::String("fallback")),
                )),
                print(expr(ExprKind::Null)),
            ],
        });
        let output = emit(&module);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(run.stdout, b"0\n0\nnull\n7\n7\n8\n8\nfalse\n\nnull\n");
    }

    /// A falha usa TypeError JavaScript do subconjunto, após um único efeito.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn null_assert_throws_after_evaluating_operand_once() {
        let module = dartforge_hir::lower(Program {
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![function(
                "missing",
                Type::NullableInt,
                &[],
                vec![
                    print(expr(ExprKind::String("evaluated"))),
                    statement(StatementKind::Return(Some(expr(ExprKind::Null)))),
                ],
            )],
            statements: vec![
                print(asserted(call("missing", vec![]))),
                print(expr(ExprKind::String("unreachable"))),
            ],
        });
        let output = emit(&module);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(!run.status.success());
        assert_eq!(run.stdout, b"evaluated\n");
        assert!(
            String::from_utf8_lossy(&run.stderr)
                .contains("TypeError: Null check operator used on a null value")
        );
    }
    /// Retornos nullable implícitos não podem expor undefined do JavaScript.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn nullable_function_fallthrough_returns_null() {
        let module = dartforge_hir::lower(Program {
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![
                function("number", Type::NullableInt, &[], vec![]),
                function("text", Type::NullableString, &[], vec![]),
                function("flag", Type::NullableBool, &[], vec![]),
                function("nothing", Type::Null, &[], vec![]),
            ],
            statements: vec![
                print(call("number", vec![])),
                print(call("text", vec![])),
                print(call("flag", vec![])),
                print(call("nothing", vec![])),
            ],
        });
        let output = emit(&module);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(run.stdout, b"null\nnull\nnull\nnull\n");
    }
    /// Cria um acesso explícito de membro, sem duplicar avaliação do receptor.
    fn member(receiver: Expr<'static>, field: &'static str) -> Expr<'static> {
        expr(ExprKind::Member {
            receiver: Box::new(receiver),
            name: field,
        })
    }
    /// Cria chamada de método com ligação dinâmica pelo receptor.
    fn method(receiver: Expr<'static>, method: &'static str) -> Expr<'static> {
        expr(ExprKind::MethodCall {
            receiver: Box::new(receiver),
            name: method,
            arguments: vec![],
        })
    }

    /// Verifica herança antecipada, despacho, inicializadores e atribuição com efeito.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn classes_preserve_inheritance_dispatch_and_receiver_effects() {
        use dartforge_syntax::Field;
        let span = Span { start: 0, end: 0 };
        let base = Class {
            is_interface: false,
            library_id: 0,
            is_abstract: false,
            interfaces: vec![],
            abstract_methods: vec![],
            enum_values: vec![],
            id: 0,
            name: "Base",
            superclass: None,
            span,
            fields: vec![Field {
                name: "value",
                ty: Type::Int,
                is_final: false,
                initializer: asserted(call("initialize", vec![])),
                span,
            }],
            methods: vec![function(
                "get",
                Type::Int,
                &[],
                vec![statement(StatementKind::Return(Some(member(
                    expr(ExprKind::This),
                    "value",
                ))))],
            )],
        };
        let child = Class {
            is_interface: false,
            library_id: 0,
            is_abstract: false,
            interfaces: vec![],
            abstract_methods: vec![],
            enum_values: vec![],
            id: 1,
            name: "Child",
            superclass: Some(0),
            fields: vec![],
            span,
            methods: vec![function(
                "get",
                Type::Int,
                &[],
                vec![statement(StatementKind::Return(Some(binary(
                    BinaryOp::Add,
                    member(expr(ExprKind::This), "value"),
                    int(10),
                ))))],
            )],
        };
        let module = dartforge_hir::lower(Program {
            types: vec![],
            extensions: vec![],
            classes: vec![child, base],
            functions: vec![
                function(
                    "initialize",
                    Type::NullableInt,
                    &[],
                    vec![
                        print(expr(ExprKind::String("initialize"))),
                        statement(StatementKind::Return(Some(int(2)))),
                    ],
                ),
                function(
                    "receiver",
                    Type::Class(0),
                    &[("item", Type::Class(0))],
                    vec![
                        print(expr(ExprKind::String("receiver"))),
                        statement(StatementKind::Return(Some(name("item")))),
                    ],
                ),
            ],
            statements: vec![
                variable("class_0", false, expr(ExprKind::Construct { class_id: 1 })),
                print(method(name("class_0"), "get")),
                statement(StatementKind::FieldAssign {
                    receiver: call("receiver", vec![name("class_0")]),
                    name: "value",
                    value: int(5),
                }),
                print(method(name("class_0"), "get")),
            ],
        });
        let output = emit(&module);
        assert!(
            output.find("class $dartforgeClass0").unwrap()
                < output.find("class $dartforgeClass1").unwrap()
        );
        assert!(output.contains("function $dartforgeNullAssert("));
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(run.stdout, b"initialize\n12\nreceiver\n15\n");
    }
    /// Gera classes vazias com IDs arbitrários para validar o contrato interno.
    fn empty_class(id: u32, superclass: Option<u32>) -> Class<'static> {
        Class {
            is_interface: false,
            library_id: 0,
            is_abstract: false,
            interfaces: vec![],
            abstract_methods: vec![],
            enum_values: vec![],
            id,
            name: "Synthetic",
            superclass,
            fields: vec![],
            methods: vec![],
            span: Span { start: 0, end: 0 },
        }
    }

    /// IDs esparsos e bases posteriores mantêm ordem determinística por fonte.
    #[test]
    fn class_order_handles_sparse_forward_ids_deterministically() {
        let classes = vec![
            empty_class(9000, Some(42)),
            empty_class(7, None),
            empty_class(42, None),
            empty_class(18, Some(9000)),
        ];
        assert_eq!(class_order(&classes), [1, 2, 0, 3]);
        assert_eq!(class_order(&classes), class_order(&classes));
        let chain: Vec<_> = (0..10000)
            .rev()
            .map(|id| empty_class(id, id.checked_sub(1)))
            .collect();
        assert_eq!(class_order(&chain), (0..10000).rev().collect::<Vec<_>>());
    }

    /// Uma base ausente deve falhar antes da emissão de JavaScript inválido.
    #[test]
    #[should_panic(expected = "classe base ausente")]
    fn class_order_rejects_unknown_base() {
        class_order(&[empty_class(7, Some(99))]);
    }

    /// Ciclos não podem bloquear a fila nem provocar recursão sem limite.
    #[test]
    #[should_panic(expected = "ciclo na herança")]
    fn class_order_rejects_cycle() {
        class_order(&[empty_class(7, Some(99)), empty_class(99, Some(7))]);
    }

    /// IDs repetidos violam a identidade nominal, mesmo em classes sem base.
    #[test]
    #[should_panic(expected = "ID de classe duplicado")]
    fn class_order_rejects_duplicate_ids() {
        class_order(&[empty_class(7, None), empty_class(7, None)]);
    }
    /// Despacho resolvido mantém this primitivo, efeitos ordenados e auxiliar !.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn extensions_use_static_targets_and_preserve_primitive_this() {
        use dartforge_syntax::{Extension, ExtensionTarget, Resolution};
        let span = Span { start: 10, end: 20 };
        let mut selected = expr(ExprKind::MethodCall {
            receiver: Box::new(call("mark", vec![int(7)])),
            name: "add",
            arguments: vec![call("mark", vec![int(2)])],
        });
        selected.span = span;
        let program = Program {
            types: vec![],
            classes: vec![],
            extensions: vec![Extension {
                id: 42,
                name: "Numbers",
                on_type: Type::Int,
                span,
                methods: vec![function(
                    "add",
                    Type::Int,
                    &[("n", Type::Int)],
                    vec![
                        print(binary(BinaryOp::Equal, expr(ExprKind::This), int(7))),
                        statement(StatementKind::Return(Some(asserted(binary(
                            BinaryOp::Add,
                            expr(ExprKind::This),
                            name("n"),
                        ))))),
                    ],
                )],
            }],
            functions: vec![function(
                "mark",
                Type::Int,
                &[("n", Type::Int)],
                vec![
                    print(name("n")),
                    statement(StatementKind::Return(Some(name("n")))),
                ],
            )],
            statements: vec![print(selected)],
        };
        let resolution = Resolution {
            types: vec![],
            expr_types: Default::default(),
            extension_calls: std::collections::BTreeMap::from([(
                (10, 20),
                ExtensionTarget {
                    extension_id: 42,
                    method_index: 0,
                },
            )]),
        };
        let output = emit(&dartforge_hir::lower_resolved(program, resolution));
        assert!(output.contains("$dartforgeExtension42Method0.call($df_mark(7), $df_mark(2))"));
        assert!(output.contains("function $dartforgeNullAssert("));
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(run.stdout, b"7\n2\ntrue\n9\n");
    }
}
