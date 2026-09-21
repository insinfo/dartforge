//! Conjuntos, espalhamentos, elementos `if`/`for`, `?.` e operadores de bits.
//!
//! Duas regras atravessam este módulo. A primeira é a ordem de avaliação do
//! Dart: cada operando aparece uma única vez no JavaScript emitido, inclusive
//! em `?.` encadeado e em `...?`, onde um temporário guarda o valor em vez de
//! repetir a subexpressão. A segunda é o caminho comum: um literal sem
//! espalhamento e sem `if`/`for` continua saindo como o mesmo literal de array
//! de antes, sem construtor imperativo e sem alocação extra no compilador.
use super::*;

/// Indica que o literal precisa do construtor imperativo.
///
/// `...` e `...?` cabem no próprio literal de array do JavaScript; só `if` e
/// `for` exigem instruções, porque produzem uma quantidade variável de
/// elementos. O teste é um `any` sobre um slice emprestado, sem alocar.
pub(super) fn needs_builder(elements: &[Expr<'_>]) -> bool {
    elements.iter().any(|element| {
        matches!(
            element.kind,
            ExprKind::CollectionIf { .. } | ExprKind::CollectionFor { .. }
        )
    })
}

/// Indica que o literal de mapa precisa do construtor imperativo.
pub(super) fn entries_need_builder(entries: &[(Expr<'_>, Option<Expr<'_>>)]) -> bool {
    entries.iter().any(|(element, value)| {
        value.is_none()
            && matches!(
                element.kind,
                ExprKind::CollectionIf { .. } | ExprKind::CollectionFor { .. }
            )
    })
}

/// Emite o array de valores de uma lista ou conjunto, com espalhamentos.
pub(super) fn sequence_values(elements: &[Expr<'_>], output: &mut Output<'_>) {
    output.push('[');
    for (index, element) in elements.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        match &element.kind {
            ExprKind::NullAwareElement(inner) => {
                // Dart 3.8: avalia uma vez e omite o elemento quando é null.
                output.push_str(
                    "...(($dartforgeElement) => $dartforgeElement === null ? [] : [$dartforgeElement])(",
                );
                expression(inner, output);
                output.push(')');
            }
            ExprKind::Spread {
                operand,
                null_aware,
            } => spread_values(operand, *null_aware, output),
            _ => expression(element, output),
        }
    }
    output.push(']');
}

/// Emite `...operando` de lista ou conjunto avaliando o operando uma vez.
fn spread_values(operand: &Expr<'_>, null_aware: bool, output: &mut Output<'_>) {
    output.push_str("...");
    if null_aware {
        // O auxiliar recebe o valor já avaliado: nada é repetido no texto.
        output.spread_used = true;
        output.push_str("$dartforgeSpread(");
        expression(operand, output);
        output.push(')');
        return;
    }
    expression(operand, output);
}

/// Emite `...operando` de mapa, que espalha pares `[chave, valor]`.
fn spread_entries(operand: &Expr<'_>, null_aware: bool, output: &mut Output<'_>) {
    output.push_str("...");
    if null_aware {
        output.spread_used = true;
        output.push_str("$dartforgeSpreadEntries(");
        expression(operand, output);
        output.push(')');
        return;
    }
    expression(operand, output);
    output.push_str(".values");
}

/// Emite o array de pares de um literal de mapa, com espalhamentos.
pub(super) fn entry_values(entries: &[(Expr<'_>, Option<Expr<'_>>)], output: &mut Output<'_>) {
    output.push('[');
    for (index, (key, value)) in entries.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        match value {
            Some(value) => entry_pair(key, value, output),
            None => match &key.kind {
                ExprKind::Spread {
                    operand,
                    null_aware,
                } => spread_entries(operand, *null_aware, output),
                ExprKind::MapEntry { key, value } => entry_pair(key, value, output),
                _ => panic!("AST inválida: elemento de mapa sem valor nem forma de controle"),
            },
        }
    }
    output.push(']');
}

/// Emite um par `[chave, valor]`, com o `?` do Dart 3.8 nas duas posições.
fn entry_pair(key: &Expr<'_>, value: &Expr<'_>, output: &mut Output<'_>) {
    let (optional_key, key) = match &key.kind {
        ExprKind::NullAwareElement(inner) => (true, inner.as_ref()),
        _ => (false, key),
    };
    let (optional_value, value) = match &value.kind {
        ExprKind::NullAwareElement(inner) => (true, inner.as_ref()),
        _ => (false, value),
    };
    if optional_key || optional_value {
        // A entrada inteira desaparece quando qualquer posição `?` é null.
        output.push_str("...(($dartforgeKey, $dartforgeValue) => ");
        if optional_key {
            output.push_str("$dartforgeKey === null");
        }
        if optional_key && optional_value {
            output.push_str(" || ");
        }
        if optional_value {
            output.push_str("$dartforgeValue === null");
        }
        output.push_str(" ? [] : [[$dartforgeKey, $dartforgeValue]])(");
        expression(key, output);
        output.push(',');
        expression(value, output);
        output.push(')');
        return;
    }
    output.push('[');
    expression(key, output);
    output.push(',');
    expression(value, output);
    output.push(']');
}

/// Emite o construtor imperativo de um literal com `if` ou `for`.
///
/// O acumulador é um array local de uma função seta invocada na hora, então o
/// literal continua sendo uma expressão e o temporário não escapa. Cada
/// elemento entra em ordem, e um `for` percorre o iterável uma única vez.
pub(super) fn builder(
    elements: &[Expr<'_>],
    map: bool,
    depth: usize,
    output: &mut Output<'_>,
) {
    let id = output.next_build;
    output.next_build += 1;
    let accumulator = format!("$dartforgeBuild{id}");
    writeln!(output, "(() => {{").expect("escrever em String não falha");
    indent(depth + 1, output);
    writeln!(output, "const {accumulator} = [];").expect("escrever em String não falha");
    for element in elements {
        indent(depth + 1, output);
        append(element, &accumulator, map, depth + 1, output);
    }
    indent(depth + 1, output);
    writeln!(output, "return {accumulator};").expect("escrever em String não falha");
    indent(depth, output);
    output.push_str("})()");
}

/// Acrescenta ao acumulador tudo o que um elemento produz, na ordem escrita.
fn append(
    element: &Expr<'_>,
    accumulator: &str,
    map: bool,
    depth: usize,
    output: &mut Output<'_>,
) {
    match &element.kind {
        ExprKind::CollectionIf {
            condition,
            then_element,
            else_element,
        } => {
            output.push_str("if (");
            expression(condition, output);
            output.push_str(") { ");
            append(then_element, accumulator, map, depth, output);
            output.push_str("}");
            if let Some(other) = else_element {
                output.push_str(" else { ");
                append(other, accumulator, map, depth, output);
                output.push('}');
            }
            output.push('\n');
        }
        ExprKind::CollectionFor { header, element } => {
            match &header.kind {
                StatementKind::ForIn {
                    is_final,
                    name,
                    iterable,
                    ..
                } => {
                    output.push_str(if *is_final { "for (const " } else { "for (let " });
                    declaration(name, output);
                    output.push_str(" of ");
                    expression(iterable, output);
                    output.push_str(") { ");
                }
                StatementKind::For {
                    initializer,
                    condition,
                    update,
                    ..
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
                    output.push_str(") { ");
                }
                _ => panic!("AST inválida: cabeçalho de for de literal fora das duas formas"),
            }
            append(element, accumulator, map, depth, output);
            output.push_str("}\n");
        }
        ExprKind::Spread {
            operand,
            null_aware,
        } => {
            write!(output, "{accumulator}.push(").expect("escrever em String não falha");
            if map {
                spread_entries(operand, *null_aware, output);
            } else {
                spread_values(operand, *null_aware, output);
            }
            output.push_str(");\n");
        }
        ExprKind::MapEntry { key, value } => {
            write!(output, "{accumulator}.push(...[").expect("escrever em String não falha");
            entry_pair(key, value, output);
            output.push_str("]);\n");
        }
        ExprKind::NullAwareElement(_) => {
            write!(output, "{accumulator}.push(...[").expect("escrever em String não falha");
            sequence_spreadable(element, output);
            output.push_str("]);\n");
        }
        _ => {
            write!(output, "{accumulator}.push(").expect("escrever em String não falha");
            expression(element, output);
            output.push_str(");\n");
        }
    }
}

/// Emite um elemento `?valor` como item espalhável de um array literal.
fn sequence_spreadable(element: &Expr<'_>, output: &mut Output<'_>) {
    let ExprKind::NullAwareElement(inner) = &element.kind else {
        unreachable!("chamado apenas para o elemento `?valor`")
    };
    output.push_str(
        "...(($dartforgeElement) => $dartforgeElement === null ? [] : [$dartforgeElement])(",
    );
    expression(inner, output);
    output.push(')');
}

/// Emite a cadeia null-aware avaliando o receptor exatamente uma vez.
///
/// O receptor vai para o parâmetro da função seta; quando é null, nenhum
/// seletor da cadeia executa e o resultado é null. Cadeias aninhadas empilham
/// temporários distintos, como a cascata já fazia com o seu receptor.
pub(super) fn null_short(receiver: &Expr<'_>, chain: &Expr<'_>, output: &mut Output<'_>) {
    let id = output.next_short;
    output.next_short += 1;
    let temporary = format!("$dartforgeShort{id}");
    write!(output, "(({temporary}) => {temporary} === null ? null : ")
        .expect("escrever em String não falha");
    output.null_short_targets.push(temporary);
    expression(chain, output);
    output.null_short_targets.pop();
    output.push_str(")(");
    expression(receiver, output);
    output.push(')');
}

/// Runtime dos operadores de bits e de espalhamento, incluído sob demanda.
///
/// A semântica é a do alvo web (dart2js): `&`, `|`, `^` e `~` devolvem o
/// inteiro sem sinal de 32 bits, `<<` e `>>>` devolvem 0 a partir de 32
/// posições, `>>` satura em 31 para receptor não positivo e uma contagem
/// negativa lança `ArgumentError`. Está conferida contra `dart compile js -O2`
/// numa grade de 696 casos, com negativos, o limite de 2^31 e valores acima
/// de 2^32; ver docs/COLECOES-OPERADORES.md para a divergência com a VM.
pub(super) const BITWISE_RUNTIME: &str = "function $dartforgeShiftCount(count) { if (count < 0) throw new Error('Invalid argument: ' + count); return count; }\nfunction $dartforgeShl(a,b) { return $dartforgeShiftCount(b) > 31 ? 0 : ((a << b) >>> 0); }\nfunction $dartforgeShr(a,b) { $dartforgeShiftCount(b); return a > 0 ? (b > 31 ? 0 : (a >>> b)) : ((a >> (b > 31 ? 31 : b)) >>> 0); }\nfunction $dartforgeUshr(a,b) { return $dartforgeShiftCount(b) > 31 ? 0 : (a >>> b); }\n";

/// Runtime do espalhamento null-aware, incluído sob demanda.
pub(super) const SPREAD_RUNTIME: &str = "function $dartforgeSpread(value) { return value === null ? [] : value; }\nfunction $dartforgeSpreadEntries(value) { return value === null ? [] : value.values; }\n";

/// Emite o construtor imperativo de um literal de mapa com `if` ou `for`.
///
/// As entradas comuns e os elementos de controle entram no mesmo acumulador,
/// na ordem escrita, para que a última ocorrência de uma chave repetida seja a
/// que vence — a mesma regra do `LinkedHashMap` do Dart.
pub(super) fn builder_entries(
    entries: &[(Expr<'_>, Option<Expr<'_>>)],
    depth: usize,
    output: &mut Output<'_>,
) {
    let id = output.next_build;
    output.next_build += 1;
    let accumulator = format!("$dartforgeBuild{id}");
    writeln!(output, "(() => {{").expect("escrever em String não falha");
    indent(depth + 1, output);
    writeln!(output, "const {accumulator} = [];").expect("escrever em String não falha");
    for (key, value) in entries {
        indent(depth + 1, output);
        match value {
            Some(value) => {
                write!(output, "{accumulator}.push(...[").expect("escrever em String não falha");
                entry_pair(key, value, output);
                output.push_str("]);\n");
            }
            None => append(key, &accumulator, true, depth + 1, output),
        }
    }
    indent(depth + 1, output);
    writeln!(output, "return {accumulator};").expect("escrever em String não falha");
    indent(depth, output);
    output.push_str("})()");
}
