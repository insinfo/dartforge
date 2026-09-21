//! Emissão de Futures encaixotados: await retira exatamente uma camada.
//! Referência: SDK 3.6.2 sdk/lib/async/future_impl.dart, propagação de listeners.
//! Geradores e scheduler são próprios; mantêm continuação pendente no mesmo turno.
use super::*;
use dartforge_syntax::{DurationUnit, TypeShape};

/// Obtém o resultado declarado do Future, preservando Future aninhado como valor.
pub(super) fn result_type(ty: Type, output: &Output<'_>) -> Type {
    match ty {
        Type::Applied(id) => match output.resolution.types[id as usize] {
            TypeShape::Future(result) => result,
            _ => panic!("função async sem retorno Future validado"),
        },
        Type::Void => Type::Void,
        _ => panic!("função async sem retorno Future validado"),
    }
}
/// Abre a ativação async lexical; sua execução começa antes de retornar o Future.
pub(super) fn begin(output: &mut Output<'_>) {
    output.async_used = true;
    output.runtime_types_used = true;
    output.push_str("return $dartforgeAsync(() => (function* () {\n");
}
/// Fecha a ativação mantendo a camada Future definida pelo tipo de retorno.
pub(super) fn end(result: Type, output: &mut Output<'_>) {
    output.push_str("return null;\n}).call(this),");
    types::descriptor(result, output);
    output.push_str(");\n");
}
/// Emite os construtores assíncronos e Duration preservando avaliação dos argumentos.
pub(super) fn expression(value: &Expr<'_>, output: &mut Output<'_>) {
    output.async_used = true;
    output.runtime_types_used = true;
    match &value.kind {
        ExprKind::Await(operand) => {
            output.push_str("(yield ");
            super::expression(operand, output);
            output.push_str(").value");
        }
        ExprKind::FutureValue { value: operand, .. } => {
            output.push_str("$dartforgeFutureValue(");
            if let Some(operand) = operand {
                super::expression(operand, output);
            } else {
                output.push_str("null");
            }
            future_result(value, output);
        }
        ExprKind::FutureDelayed {
            duration,
            computation,
            ..
        } => {
            output.push_str("$dartforgeDelayed(");
            super::expression(duration, output);
            output.push(',');
            if let Some(computation) = computation {
                super::expression(computation, output);
            } else {
                output.push_str("null");
            }
            future_result(value, output);
        }
        ExprKind::Duration { parts } => {
            output.push_str("new $dartforgeDuration([");
            for (index, (unit, part)) in parts.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                let factor: i64 = match unit {
                    DurationUnit::Days => 86_400_000_000,
                    DurationUnit::Hours => 3_600_000_000,
                    DurationUnit::Minutes => 60_000_000,
                    DurationUnit::Seconds => 1_000_000,
                    DurationUnit::Milliseconds => 1_000,
                    DurationUnit::Microseconds => 1,
                };
                write!(output, "[{factor},").unwrap();
                super::expression(part, output);
                output.push(']');
            }
            output.push_str("])");
        }
        _ => unreachable!(),
    }
}
/// Usa o tipo resolvido, inclusive quando o argumento genérico foi inferido.
fn future_result(value: &Expr<'_>, output: &mut Output<'_>) {
    let ty = output.resolution.expr_types[&(value.span.start, value.span.end)];
    let result = result_type(ty, output);
    output.push(',');
    types::descriptor(result, output);
    output.push(')');
}
