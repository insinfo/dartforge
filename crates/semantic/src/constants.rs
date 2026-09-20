//! Avaliação pura do subconjunto const; não executa funções ou getters do usuário.
//! SDK Dart 3.6.2: listas em contexto const são transitivamente constantes e
//! canônicas. Operandos mortos ainda precisam ser expressões constantes válidas,
//! mas &&, || e ?? não avaliam sua aritmética. Inteiros aqui permanecem i32.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{
    BinaryOp, ConstValue, Expr, ExprKind, Resolution, Type, TypeShape, UnaryOp,
};

/// Avalia expressão já tipada, consultando somente bindings declarados const.
pub(super) fn evaluate(
    expression: &Expr<'_>,
    resolution: &Resolution,
    lookup: &impl Fn(&str) -> Option<ConstValue>,
) -> Result<ConstValue, Diagnostic> {
    validate(expression, lookup)?;
    evaluate_inner(expression, resolution, lookup)
}

/// Preserva o intervalo do operando que violou o contrato constante.
fn error(span: Span, message: &str) -> Diagnostic {
    Diagnostic::new(format!("Const expression: {message}"), span)
}

/// Valida também ramos mortos: chamadas e identificadores não const são proibidos.
fn validate(
    expression: &Expr<'_>,
    lookup: &impl Fn(&str) -> Option<ConstValue>,
) -> Result<(), Diagnostic> {
    match &expression.kind {
        ExprKind::Int(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::OwnedString(_)
        | ExprKind::Null
        | ExprKind::EnumValue { .. } => {}
        ExprKind::Identifier(name) => {
            if lookup(name).is_none() {
                return Err(error(expression.span, "identifier is not a const binding"));
            }
        }
        ExprKind::Const(value) | ExprKind::Unary { operand: value, .. } => validate(value, lookup)?,
        ExprKind::Binary { left, right, .. } => {
            validate(left, lookup)?;
            validate(right, lookup)?;
        }
        ExprKind::List { elements, .. } => {
            for element in elements {
                validate(element, lookup)?;
            }
        }
        _ => {
            return Err(error(
                expression.span,
                "calls, getters and this expression are unsupported",
            ));
        }
    }
    Ok(())
}

/// Calcula valores sem efeitos; curto-circuito evita erros de operandos não executados.
fn evaluate_inner(
    expression: &Expr<'_>,
    resolution: &Resolution,
    lookup: &impl Fn(&str) -> Option<ConstValue>,
) -> Result<ConstValue, Diagnostic> {
    let span = expression.span;
    Ok(match &expression.kind {
        ExprKind::Int(value) => ConstValue::Int(*value),
        ExprKind::Bool(value) => ConstValue::Bool(*value),
        ExprKind::String(value) => ConstValue::String((*value).into()),
        ExprKind::OwnedString(value) => ConstValue::String(value.clone()),
        ExprKind::Null => ConstValue::Null,
        ExprKind::EnumValue { class_id, name } => ConstValue::Enum {
            class_id: *class_id,
            name: (*name).into(),
        },
        ExprKind::Identifier(name) => {
            lookup(name).ok_or_else(|| error(span, "identifier is not a const binding"))?
        }
        ExprKind::Const(value) => evaluate_inner(value, resolution, lookup)?,
        ExprKind::List {
            element_type,
            elements,
        } => {
            let resolved = resolution
                .expr_types
                .get(&(span.start, span.end))
                .and_then(|ty| {
                    if let Type::Applied(id) = ty {
                        resolution.types.get(*id as usize)
                    } else {
                        None
                    }
                });
            let element_type = if let Some(TypeShape::List(element)) = resolved {
                *element
            } else {
                element_type.ok_or_else(|| {
                    error(span, "list element type must be resolved before evaluation")
                })?
            };
            if !closed_type(element_type, resolution) {
                return Err(error(
                    span,
                    "const list requires a closed element type without type parameters",
                ));
            }
            let values = elements
                .iter()
                .map(|value| evaluate_inner(value, resolution, lookup))
                .collect::<Result<Vec<_>, _>>()?;
            ConstValue::List {
                element_type,
                values,
            }
        }
        ExprKind::Unary { op, operand } => {
            let value = evaluate_inner(operand, resolution, lookup)?;
            match (op, value) {
                (UnaryOp::Negate, ConstValue::Int(n)) => {
                    ConstValue::Int(n.checked_neg().ok_or_else(|| {
                        error(span, "integer overflow outside supported i32 range")
                    })?)
                }
                (UnaryOp::Not, ConstValue::Bool(value)) => ConstValue::Bool(!value),
                (UnaryOp::NullAssert, ConstValue::Null) => {
                    return Err(error(span, "null assertion failed"));
                }
                (UnaryOp::NullAssert, value) => value,
                _ => return Err(error(span, "invalid unary operand")),
            }
        }
        ExprKind::Binary { op, left, right } => {
            let left = evaluate_inner(left, resolution, lookup)?;
            match (*op, &left) {
                (BinaryOp::And, ConstValue::Bool(false)) => return Ok(ConstValue::Bool(false)),
                (BinaryOp::Or, ConstValue::Bool(true)) => return Ok(ConstValue::Bool(true)),
                (BinaryOp::IfNull, value) if *value != ConstValue::Null => return Ok(left),
                _ => {}
            }
            let right = evaluate_inner(right, resolution, lookup)?;
            binary(*op, left, right, resolution, span)?
        }
        _ => return Err(error(span, "expression is not supported")),
    })
}

/// Const listas não podem depender de parâmetros de tipo, mesmo quando vazias.
fn closed_type(ty: Type, resolution: &Resolution) -> bool {
    match ty {
        Type::Parameter(_) | Type::Inferred | Type::Void => false,
        Type::Applied(id) => match resolution.types.get(id as usize) {
            Some(TypeShape::List(element) | TypeShape::Iterable(element)) => {
                closed_type(*element, resolution)
            }
            Some(TypeShape::Function { result, parameters }) => {
                (*result == Type::Void || closed_type(*result, resolution))
                    && parameters.iter().all(|ty| closed_type(*ty, resolution))
            }
            None => false,
        },
        _ => true,
    }
}

/// Compara tipos estruturais por forma, não pelo índice local da tabela de tipos.
fn same_type(left: Type, right: Type, resolution: &Resolution) -> bool {
    if left == right {
        return true;
    }
    match (left, right) {
        (Type::Applied(a), Type::Applied(b)) => match (
            resolution.types.get(a as usize),
            resolution.types.get(b as usize),
        ) {
            (Some(TypeShape::List(a)), Some(TypeShape::List(b)))
            | (Some(TypeShape::Iterable(a)), Some(TypeShape::Iterable(b))) => {
                same_type(*a, *b, resolution)
            }
            (
                Some(TypeShape::Function {
                    result: a,
                    parameters: ap,
                }),
                Some(TypeShape::Function {
                    result: b,
                    parameters: bp,
                }),
            ) => {
                same_type(*a, *b, resolution)
                    && ap.len() == bp.len()
                    && ap
                        .iter()
                        .zip(bp)
                        .all(|(a, b)| same_type(*a, *b, resolution))
            }
            _ => false,
        },
        _ => false,
    }
}

/// Igualdade de listas const corresponde à identidade de seus valores canônicos.
fn equal(left: &ConstValue, right: &ConstValue, resolution: &Resolution) -> bool {
    match (left, right) {
        (
            ConstValue::List {
                element_type: a,
                values: av,
            },
            ConstValue::List {
                element_type: b,
                values: bv,
            },
        ) => {
            same_type(*a, *b, resolution)
                && av.len() == bv.len()
                && av.iter().zip(bv).all(|(a, b)| equal(a, b, resolution))
        }
        _ => left == right,
    }
}

/// Aplica operadores escalares; o remainder segue o módulo não negativo de Dart.
fn binary(
    op: BinaryOp,
    left: ConstValue,
    right: ConstValue,
    resolution: &Resolution,
    span: Span,
) -> Result<ConstValue, Diagnostic> {
    if matches!(op, BinaryOp::Equal | BinaryOp::NotEqual) {
        return Ok(ConstValue::Bool(
            equal(&left, &right, resolution) == (op == BinaryOp::Equal),
        ));
    }
    if op == BinaryOp::IfNull {
        return Ok(if left == ConstValue::Null {
            right
        } else {
            left
        });
    }
    match (left, right) {
        (ConstValue::Int(a), ConstValue::Int(b)) => {
            let arithmetic = match op {
                BinaryOp::Add => a.checked_add(b),
                BinaryOp::Subtract => a.checked_sub(b),
                BinaryOp::Multiply => a.checked_mul(b),
                BinaryOp::Remainder => {
                    if b == 0 {
                        return Err(error(span, "remainder by zero"));
                    } else {
                        Some(i64::from(a).rem_euclid(i64::from(b)) as i32)
                    }
                }
                BinaryOp::Less => return Ok(ConstValue::Bool(a < b)),
                BinaryOp::LessEqual => return Ok(ConstValue::Bool(a <= b)),
                BinaryOp::Greater => return Ok(ConstValue::Bool(a > b)),
                BinaryOp::GreaterEqual => return Ok(ConstValue::Bool(a >= b)),
                _ => return Err(error(span, "invalid integer operator")),
            };
            Ok(ConstValue::Int(arithmetic.ok_or_else(|| {
                error(span, "integer overflow outside supported i32 range")
            })?))
        }
        (ConstValue::Bool(a), ConstValue::Bool(b)) => match op {
            BinaryOp::And => Ok(ConstValue::Bool(a && b)),
            BinaryOp::Or => Ok(ConstValue::Bool(a || b)),
            _ => Err(error(span, "invalid boolean operator")),
        },
        (ConstValue::String(mut a), ConstValue::String(b)) if op == BinaryOp::Add => {
            a.push_str(&b);
            Ok(ConstValue::String(a))
        }
        _ => Err(error(span, "incompatible constant operands")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monta AST pequena com intervalo estável para verificar diagnósticos locais.
    fn expression(kind: ExprKind<'_>) -> Expr<'_> {
        Expr {
            kind,
            span: Span { start: 2, end: 8 },
        }
    }
    /// Combina operandos constantes sem depender do parser em edição paralela.
    fn operation<'a>(op: BinaryOp, left: Expr<'a>, right: Expr<'a>) -> Expr<'a> {
        expression(ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }
    #[test]
    /// Curto-circuito evita aritmética inválida, mas não legaliza chamadas não const.
    fn lazy_arithmetic_but_dead_calls_still_rejected() {
        let bad = operation(
            BinaryOp::Remainder,
            expression(ExprKind::Int(1)),
            expression(ExprKind::Int(0)),
        );
        let condition = operation(BinaryOp::Equal, bad, expression(ExprKind::Int(0)));
        let lazy = operation(BinaryOp::And, expression(ExprKind::Bool(false)), condition);
        assert_eq!(
            evaluate(&lazy, &Resolution::default(), &|_| None).unwrap(),
            ConstValue::Bool(false)
        );
        let call = expression(ExprKind::Call {
            name: "sideEffect",
            arguments: vec![],
        });
        let lazy = operation(BinaryOp::And, expression(ExprKind::Bool(false)), call);
        assert!(evaluate(&lazy, &Resolution::default(), &|_| None).is_err());
    }
    #[test]
    /// Overflow mantém diagnóstico localizado; módulo negativo segue o oracle Dart.
    fn checked_integer_range_and_euclidean_remainder() {
        let overflow = operation(
            BinaryOp::Add,
            expression(ExprKind::Int(i32::MAX)),
            expression(ExprKind::Int(1)),
        );
        assert_eq!(
            evaluate(&overflow, &Resolution::default(), &|_| None)
                .unwrap_err()
                .span
                .start,
            2
        );
        let remainder = operation(
            BinaryOp::Remainder,
            expression(ExprKind::Int(-5)),
            expression(ExprKind::Int(-3)),
        );
        assert_eq!(
            evaluate(&remainder, &Resolution::default(), &|_| None).unwrap(),
            ConstValue::Int(1)
        );
    }
    #[test]
    /// Const implícito alcança elementos e resolve somente bindings constantes.
    fn const_context_descends_into_lists_and_uses_bindings() {
        let value = expression(ExprKind::List {
            element_type: Some(Type::Int),
            elements: vec![expression(ExprKind::Identifier("n"))],
        });
        let result = evaluate(&value, &Resolution::default(), &|name| {
            (name == "n").then_some(ConstValue::Int(2))
        })
        .unwrap();
        assert_eq!(
            result,
            ConstValue::List {
                element_type: Type::Int,
                values: vec![ConstValue::Int(2)]
            }
        );
        assert!(evaluate(&value, &Resolution::default(), &|_| None).is_err());
    }
    #[test]
    /// Tipos equivalentes com IDs diferentes preservam identidade canônica de listas.
    fn canonical_equality_includes_structural_element_type() {
        let resolution = Resolution {
            types: vec![
                TypeShape::List(Type::Int),
                TypeShape::List(Type::Int),
                TypeShape::List(Type::String),
            ],
            ..Resolution::default()
        };
        let value = |id| ConstValue::List {
            element_type: Type::Applied(id),
            values: vec![],
        };
        assert!(equal(&value(0), &value(1), &resolution));
        assert!(!equal(&value(0), &value(2), &resolution));
    }
}
