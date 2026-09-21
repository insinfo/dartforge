//! Emissão JavaScript do controle de fluxo: try/catch/finally, throw, rethrow,
//! assert, for-in, rótulos e o operador condicional.
//!
//! O `try` do JavaScript já garante a ordem observável que Dart exige: o
//! `finally` roda na saída normal, no `return`, no `break`, no `continue` e na
//! exceção, sem alterar o valor já calculado do `return` e sem engolir a
//! exceção. Por isso a emissão não reordena nada: ela traduz as cláusulas para
//! um único `catch` do JavaScript com um teste de tipo reificado por cláusula,
//! e relança o valor quando nenhuma cláusula o aceita.
use super::*;
use dartforge_syntax::CatchClause;

/// Escreve o rótulo JavaScript correspondente a um rótulo Dart.
///
/// Rótulos vivem num espaço de nomes separado do das variáveis em JavaScript,
/// então o prefixo serve apenas para evitar palavras reservadas da linguagem.
pub(super) fn label(name: &str, output: &mut Output<'_>) {
    output.push_str("$df_");
    output.push_str(name);
}

/// Emite `try`, as cláusulas de captura e o bloco final.
///
/// As cláusulas viram uma cadeia `if`/`else if` dentro de um único `catch` do
/// JavaScript, na ordem escrita. A primeira cláusula sem `on` encerra a cadeia,
/// porque aceita qualquer valor; sem nenhuma cláusula sem `on`, o valor que não
/// casa é relançado, e não silenciosamente engolido.
pub(super) fn try_statement(
    body: &[Statement<'_>],
    catches: &[CatchClause<'_>],
    finally_body: Option<&Vec<Statement<'_>>>,
    depth: usize,
    output: &mut Output<'_>,
) {
    output.push_str("try {\n");
    statements(body, depth + 1, output);
    indent(depth, output);
    output.push('}');
    if !catches.is_empty() {
        let id = output.next_catch;
        output.next_catch += 1;
        let caught = format!("$dartforgeCaught{id}");
        writeln!(output, " catch ({caught}) {{").unwrap();
        output.caught.push(caught.clone());
        let mut chain = 0usize;
        let mut exhaustive = false;
        for clause in catches {
            match clause.exception_type {
                Some(ty) => {
                    indent(depth + 1, output);
                    if chain > 0 {
                        output.push_str("else ");
                    }
                    write!(output, "if ($dartforgeIs({caught},").unwrap();
                    types::descriptor(ty, output);
                    output.push_str(")) {\n");
                    bind_catch(clause, &caught, depth + 2, output);
                    statements(&clause.body, depth + 2, output);
                    indent(depth + 1, output);
                    output.push_str("}\n");
                    chain += 1;
                }
                None => {
                    if chain > 0 {
                        indent(depth + 1, output);
                        output.push_str("else {\n");
                        bind_catch(clause, &caught, depth + 2, output);
                        statements(&clause.body, depth + 2, output);
                        indent(depth + 1, output);
                        output.push_str("}\n");
                    } else {
                        bind_catch(clause, &caught, depth + 1, output);
                        statements(&clause.body, depth + 1, output);
                    }
                    exhaustive = true;
                    // As cláusulas seguintes seriam inalcançáveis.
                    break;
                }
            }
        }
        if !exhaustive && chain > 0 {
            indent(depth + 1, output);
            writeln!(output, "else {{ throw {caught}; }}").unwrap();
        }
        output.caught.pop();
        indent(depth, output);
        output.push('}');
    }
    if let Some(finally_body) = finally_body {
        output.push_str(" finally {\n");
        statements(finally_body, depth + 1, output);
        indent(depth, output);
        output.push('}');
    }
    output.push('\n');
}

/// Liga as variáveis de `catch (e)` e `catch (e, s)` da cláusula.
fn bind_catch(clause: &CatchClause<'_>, caught: &str, depth: usize, output: &mut Output<'_>) {
    if let Some(name) = clause.exception {
        indent(depth, output);
        output.push_str("const ");
        declaration(name, output);
        writeln!(output, " = {caught};").unwrap();
    }
    if let Some(name) = clause.stack_trace {
        output.stack_used = true;
        indent(depth, output);
        output.push_str("const ");
        declaration(name, output);
        writeln!(output, " = $dartforgeStack({caught});").unwrap();
    }
}

/// Relança o valor da cláusula `catch` mais interna.
///
/// A análise semântica já rejeitou `rethrow` fora de um `catch`, então a pilha
/// nunca está vazia aqui.
pub(super) fn rethrow(output: &mut Output<'_>) {
    let caught = output
        .caught
        .last()
        .cloned()
        .expect("HIR inválida: rethrow fora de catch");
    writeln!(output, "throw {caught};").unwrap();
}

/// Emite `assert`, avaliando a mensagem somente quando a condição falha.
///
/// A asserção é sempre emitida; o contrato e a justificativa estão em
/// `docs/FLUXO.md`.
pub(super) fn assert_statement(
    condition: &Expr<'_>,
    message: Option<&Expr<'_>>,
    output: &mut Output<'_>,
) {
    output.assert_used = true;
    output.push_str("if (!(");
    expression(condition, output);
    output.push_str(")) { throw $dartforgeAssertionError(");
    if let Some(message) = message {
        expression(message, output);
    }
    output.push_str("); }\n");
}

/// Emite `for-in`; `for...of` avalia o iterável exatamente uma vez.
pub(super) fn for_in(
    is_final: bool,
    name: &str,
    iterable: &Expr<'_>,
    body: &[Statement<'_>],
    depth: usize,
    output: &mut Output<'_>,
) {
    output.break_targets.push(None);
    output.push_str(if is_final { "for (const " } else { "for (let " });
    declaration(name, output);
    output.push_str(" of ");
    expression(iterable, output);
    output.push_str(") ");
    block(body, depth, output);
    output.break_targets.pop();
    output.push('\n');
}

/// Emite o operador condicional preservando a avaliação de um único ramo.
pub(super) fn conditional(
    condition: &Expr<'_>,
    then_value: &Expr<'_>,
    else_value: &Expr<'_>,
    output: &mut Output<'_>,
) {
    output.push('(');
    expression(condition, output);
    output.push_str(" ? ");
    expression(then_value, output);
    output.push_str(" : ");
    expression(else_value, output);
    output.push(')');
}

/// Emite `throw` em posição de expressão, onde o JavaScript exige uma chamada.
pub(super) fn throw_expression(value: &Expr<'_>, output: &mut Output<'_>) {
    output.throw_used = true;
    output.push_str("$dartforgeThrow(");
    expression(value, output);
    output.push(')');
}

/// Devolve os auxiliares de runtime exigidos pelas formas efetivamente emitidas.
pub(super) fn runtime(output: &Output<'_>) -> String {
    let mut text = String::new();
    if output.throw_used {
        text.push_str("function $dartforgeThrow(value) { throw value; }\n");
    }
    if output.assert_used {
        text.push_str(
            "function $dartforgeAssertionError(message) { return new Error(message === undefined ? 'Failed assertion: is not true.' : 'Failed assertion: ' + message); }\n",
        );
    }
    if output.stack_used {
        // O rastro é opaco: sem etiqueta de tipo ele não satisfaz String nem
        // qualquer tipo nominal, apenas Object, como o contrato documenta.
        text.push_str(
            "function $dartforgeStack(error) { return { toString() { return error && error.stack ? String(error.stack) : ''; } }; }\n",
        );
    }
    text
}
