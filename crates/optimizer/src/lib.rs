//! Simplifica subárvores constantes após a validação semântica.
//! O passe preserva efeitos, curto circuito, intervalos de origem e operações
//! inteiras que ultrapassem i32. Não realiza propagação de variáveis nem remove blocos.
use dartforge_syntax::{BinaryOp, Expr, ExprKind, Program, Statement, StatementKind, UnaryOp};

/// Limite de bytes de uma nova string produzida por concatenação constante.
const MAX_FOLDED_STRING_BYTES: usize = 64 * 1024;

/// Quantidade de expressões substituídas pelo passe de constantes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FoldStats {
    /// Número de operadores substituídos, incluindo seleções constantes de ??.
    pub folded_expressions: usize,
}

/// Simplifica constantes em um programa que já passou pela análise semântica.
///
/// Chamadas, acessos a campos e construções nunca são avaliados pelo compilador.
/// Em ?? com lado esquerdo literal, somente o ramo executável é preservado.
/// Operações inteiras com resultado fora de i32 permanecem para execução normal.
///
/// # Exemplos
/// ```
/// use dartforge_syntax::Program;
/// let mut programa = Program { types: vec![], extensions: vec![], classes: vec![], functions: vec![], statements: vec![] };
/// assert_eq!(dartforge_optimizer::fold_constants(&mut programa).folded_expressions, 0);
/// ```
pub fn fold_constants(program: &mut Program<'_>) -> FoldStats {
    let mut stats = FoldStats::default();
    for class in &mut program.classes {
        for field in &mut class.fields {
            fold_expression(&mut field.initializer, &mut stats);
        }
        for method in &mut class.methods {
            fold_statements(&mut method.body, &mut stats);
        }
    }
    for extension in &mut program.extensions {
        for method in &mut extension.methods {
            fold_statements(&mut method.body, &mut stats);
        }
    }
    for function in &mut program.functions {
        fold_statements(&mut function.body, &mut stats);
    }
    fold_statements(&mut program.statements, &mut stats);
    stats
}

/// Visita instruções em ordem sem alterar a estrutura de controle do programa.
fn fold_statements(statements: &mut [Statement<'_>], stats: &mut FoldStats) {
    for statement in statements {
        fold_statement(statement, stats);
    }
}

/// Percorre todas as posições de expressão de uma instrução.
fn fold_statement(statement: &mut Statement<'_>, stats: &mut FoldStats) {
    match &mut statement.kind {
        StatementKind::Switch { scrutinee, cases } => {
            fold_expression(scrutinee, stats);
            for c in cases {
                if let Some(g) = &mut c.guard {
                    fold_expression(g, stats);
                }
                fold_statements(&mut c.body, stats);
            }
        }
        StatementKind::IndexAssign {
            receiver,
            index,
            value,
        } => {
            fold_expression(receiver, stats);
            fold_expression(index, stats);
            fold_expression(value, stats);
        }
        StatementKind::Variable { initializer, .. } => fold_expression(initializer, stats),
        StatementKind::Assign { value, .. }
        | StatementKind::Print(value)
        | StatementKind::Expression(value) => fold_expression(value, stats),
        StatementKind::FieldAssign {
            receiver, value, ..
        } => {
            fold_expression(receiver, stats);
            fold_expression(value, stats);
        }
        StatementKind::Return(value) => {
            if let Some(value) = value {
                fold_expression(value, stats);
            }
        }
        StatementKind::If {
            condition,
            then_body,
            else_body,
        } => {
            fold_expression(condition, stats);
            fold_statements(then_body, stats);
            if let Some(body) = else_body {
                fold_statements(body, stats);
            }
        }
        StatementKind::While { condition, body } | StatementKind::DoWhile { body, condition } => {
            fold_expression(condition, stats);
            fold_statements(body, stats);
        }
        StatementKind::For {
            initializer,
            condition,
            update,
            body,
        } => {
            if let Some(initializer) = initializer {
                fold_statement(initializer, stats);
            }
            if let Some(condition) = condition {
                fold_expression(condition, stats);
            }
            if let Some(update) = update {
                fold_statement(update, stats);
            }
            fold_statements(body, stats);
        }
        StatementKind::Block(body) => fold_statements(body, stats),
        StatementKind::Break | StatementKind::Continue => {}
    }
}

/// Simplifica filhos puros e substitui somente operadores com resultado comprovado.
fn fold_expression(expression: &mut Expr<'_>, stats: &mut FoldStats) {
    let replacement = match &mut expression.kind {
        ExprKind::Const(e) => {
            fold_expression(e, stats);
            None
        }
        ExprKind::Switch { scrutinee, arms } => {
            fold_expression(scrutinee, stats);
            for a in arms {
                if let Some(g) = &mut a.guard {
                    fold_expression(g, stats);
                }
                fold_expression(&mut a.value, stats);
            }
            None
        }
        ExprKind::Closure { body, .. } => {
            fold_statements(body, stats);
            None
        }
        ExprKind::List { elements, .. } => {
            for e in elements {
                fold_expression(e, stats);
            }
            None
        }
        ExprKind::Index { receiver, index } => {
            fold_expression(receiver, stats);
            fold_expression(index, stats);
            None
        }
        ExprKind::Invoke { callee, arguments } => {
            fold_expression(callee, stats);
            for e in arguments {
                fold_expression(e, stats);
            }
            None
        }
        ExprKind::Unary { op, operand } => {
            fold_expression(operand, stats);
            match (op, &operand.kind) {
                (UnaryOp::Negate, ExprKind::Int(value)) => value.checked_neg().map(ExprKind::Int),
                (UnaryOp::Not, ExprKind::Bool(value)) => Some(ExprKind::Bool(!value)),
                _ => None,
            }
        }
        ExprKind::Binary { op, left, right } => {
            fold_expression(left, stats);
            if *op == BinaryOp::IfNull && matches!(left.kind, ExprKind::Null) {
                fold_expression(right, stats);
                // A chamada selecionada conserva sua identidade na tabela de resolução.
                expression.span = right.span;
                Some(std::mem::replace(&mut right.kind, ExprKind::Null))
            } else if *op == BinaryOp::IfNull && is_literal(&left.kind) {
                // O ramo direito não executa; não visita nem contabiliza suas constantes.
                expression.span = left.span;
                Some(std::mem::replace(&mut left.kind, ExprKind::Null))
            } else {
                fold_expression(right, stats);
                fold_binary(*op, &left.kind, &right.kind)
            }
        }
        ExprKind::Call { arguments, .. } | ExprKind::GenericCall { arguments, .. } => {
            for argument in arguments {
                fold_expression(argument, stats);
            }
            None
        }
        ExprKind::MethodCall {
            receiver,
            arguments,
            ..
        } => {
            fold_expression(receiver, stats);
            for argument in arguments {
                fold_expression(argument, stats);
            }
            None
        }
        ExprKind::Member { receiver, .. } => {
            fold_expression(receiver, stats);
            None
        }
        _ => None,
    };
    if let Some(kind) = replacement {
        expression.kind = kind;
        stats.folded_expressions += 1;
    }
}

/// Reconhece literais sem acesso ao ambiente nem efeitos de execução.
fn is_literal(kind: &ExprKind<'_>) -> bool {
    matches!(
        kind,
        ExprKind::Null
            | ExprKind::Int(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::OwnedString(_)
    )
}

/// Obtém o texto de um literal sem copiar strings emprestadas ou alocadas.
fn string_value<'a>(kind: &'a ExprKind<'_>) -> Option<&'a str> {
    match kind {
        ExprKind::String(value) => Some(value),
        ExprKind::OwnedString(value) => Some(value),
        _ => None,
    }
}

/// Calcula igualdade apenas entre valores literais suportados.
fn literal_equal(left: &ExprKind<'_>, right: &ExprKind<'_>) -> Option<bool> {
    if !is_literal(left) || !is_literal(right) {
        return None;
    }
    Some(match (left, right) {
        (ExprKind::Null, ExprKind::Null) => true,
        (ExprKind::Int(a), ExprKind::Int(b)) => a == b,
        (ExprKind::Bool(a), ExprKind::Bool(b)) => a == b,
        _ => match (string_value(left), string_value(right)) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        },
    })
}

/// Dobra operações literais sem transbordamento ou alocação desproporcional.
fn fold_binary<'a>(
    op: BinaryOp,
    left: &ExprKind<'a>,
    right: &ExprKind<'a>,
) -> Option<ExprKind<'a>> {
    match op {
        BinaryOp::Equal => return literal_equal(left, right).map(ExprKind::Bool),
        BinaryOp::NotEqual => {
            return literal_equal(left, right).map(|value| ExprKind::Bool(!value));
        }
        _ => {}
    }
    if let (ExprKind::Int(a), ExprKind::Int(b)) = (left, right) {
        return match op {
            BinaryOp::Add => a.checked_add(*b).map(ExprKind::Int),
            BinaryOp::Subtract => a.checked_sub(*b).map(ExprKind::Int),
            BinaryOp::Multiply => a.checked_mul(*b).map(ExprKind::Int),
            BinaryOp::Less => Some(ExprKind::Bool(a < b)),
            BinaryOp::LessEqual => Some(ExprKind::Bool(a <= b)),
            BinaryOp::Greater => Some(ExprKind::Bool(a > b)),
            BinaryOp::GreaterEqual => Some(ExprKind::Bool(a >= b)),
            _ => None,
        };
    }
    if let (ExprKind::Bool(a), ExprKind::Bool(b)) = (left, right) {
        return match op {
            BinaryOp::And => Some(ExprKind::Bool(*a && *b)),
            BinaryOp::Or => Some(ExprKind::Bool(*a || *b)),
            _ => None,
        };
    }
    if op == BinaryOp::Add
        && let (Some(a), Some(b)) = (string_value(left), string_value(right))
    {
        let length = a.len().checked_add(b.len())?;
        if length > MAX_FOLDED_STRING_BYTES {
            return None;
        }
        let mut result = String::with_capacity(length);
        result.push_str(a);
        result.push_str(b);
        return Some(ExprKind::OwnedString(result));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_diagnostics::Span;
    use dartforge_syntax::{Class, Field, Function, Type};
    const SPAN: Span = Span { start: 5, end: 20 };
    /// Constrói uma expressão mantendo uma posição de origem conhecida.
    fn expr(kind: ExprKind<'static>) -> Expr<'static> {
        Expr { kind, span: SPAN }
    }
    /// Constrói um literal inteiro para testar aritmética constante.
    fn int(value: i32) -> Expr<'static> {
        expr(ExprKind::Int(value))
    }
    /// Constrói uma operação binária para testar o passe.
    fn binary(op: BinaryOp, left: Expr<'static>, right: Expr<'static>) -> Expr<'static> {
        expr(ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }
    /// Constrói uma chamada opaca com efeito observável potencial.
    fn call() -> Expr<'static> {
        expr(ExprKind::Call {
            name: "effect",
            arguments: vec![],
        })
    }
    /// Constrói uma instrução de teste.
    fn stmt(kind: StatementKind<'static>) -> Statement<'static> {
        Statement { kind, span: SPAN }
    }
    /// Aplica o passe a uma expressão isolada e devolve o resultado e suas estatísticas.
    fn folded(mut value: Expr<'static>) -> (Expr<'static>, FoldStats) {
        let mut stats = FoldStats::default();
        fold_expression(&mut value, &mut stats);
        (value, stats)
    }
    /// Verifica recursão, posições preservadas e normalização de zero inteiro.
    #[test]
    fn checked_integer_folding_preserves_spans_and_zero() {
        let (value, stats) = folded(binary(
            BinaryOp::Multiply,
            binary(BinaryOp::Add, int(2), int(3)),
            int(4),
        ));
        assert!(matches!(value.kind, ExprKind::Int(20)));
        assert_eq!(value.span, SPAN);
        assert_eq!(stats.folded_expressions, 2);
        let (value, _) = folded(expr(ExprKind::Unary {
            op: UnaryOp::Negate,
            operand: Box::new(int(0)),
        }));
        assert!(matches!(value.kind, ExprKind::Int(0)));
        let (value, _) = folded(binary(BinaryOp::Subtract, int(4), int(6)));
        assert!(matches!(value.kind, ExprKind::Int(-2)));
    }
    /// Mantém operações cujo resultado não cabe no domínio inteiro do literal.
    #[test]
    fn integer_overflow_stays_unfolded() {
        for value in [
            binary(BinaryOp::Add, int(i32::MAX), int(1)),
            binary(BinaryOp::Subtract, int(i32::MIN), int(1)),
            binary(BinaryOp::Multiply, int(i32::MAX), int(2)),
            expr(ExprKind::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(int(i32::MIN)),
            }),
        ] {
            let (value, stats) = folded(value);
            assert_eq!(stats.folded_expressions, 0);
            assert!(matches!(
                value.kind,
                ExprKind::Unary { .. } | ExprKind::Binary { .. }
            ));
        }
    }
    /// Verifica operadores booleanos, comparação de inteiros e igualdade heterogênea.
    #[test]
    fn boolean_and_equality_folding_matches_literal_values() {
        for (op, expected) in [
            (BinaryOp::Less, true),
            (BinaryOp::LessEqual, true),
            (BinaryOp::Greater, false),
            (BinaryOp::GreaterEqual, false),
            (BinaryOp::Equal, false),
            (BinaryOp::NotEqual, true),
        ] {
            let (value, _) = folded(binary(op, int(1), int(2)));
            assert!(matches!(value.kind,ExprKind::Bool(actual) if actual==expected));
        }
        let (value, _) = folded(binary(BinaryOp::Equal, int(1), expr(ExprKind::Bool(true))));
        assert!(matches!(value.kind, ExprKind::Bool(false)));
        let (value, _) = folded(binary(
            BinaryOp::Equal,
            expr(ExprKind::Null),
            expr(ExprKind::Null),
        ));
        assert!(matches!(value.kind, ExprKind::Bool(true)));
        let (value, _) = folded(expr(ExprKind::Unary {
            op: UnaryOp::Not,
            operand: Box::new(expr(ExprKind::Bool(true))),
        }));
        assert!(matches!(value.kind, ExprKind::Bool(false)));
        for op in [BinaryOp::And, BinaryOp::Or] {
            let (value, _) = folded(binary(
                op,
                expr(ExprKind::Bool(true)),
                expr(ExprKind::Bool(false)),
            ));
            assert!(matches!(value.kind, ExprKind::Bool(_)));
        }
    }
    /// Limita crescimento de strings e compara representações emprestadas e alocadas.
    #[test]
    fn string_folding_is_bounded_and_unicode_safe() {
        let (value, _) = folded(binary(
            BinaryOp::Add,
            expr(ExprKind::String("á")),
            expr(ExprKind::OwnedString("🦀".into())),
        ));
        assert!(matches!(value.kind,ExprKind::OwnedString(ref text) if text=="á🦀"));
        let (value, _) = folded(binary(
            BinaryOp::Equal,
            expr(ExprKind::String("á")),
            expr(ExprKind::OwnedString("á".into())),
        ));
        assert!(matches!(value.kind, ExprKind::Bool(true)));
        let (value, stats) = folded(binary(
            BinaryOp::Add,
            expr(ExprKind::OwnedString("a".repeat(MAX_FOLDED_STRING_BYTES))),
            expr(ExprKind::String("b")),
        ));
        assert!(matches!(value.kind, ExprKind::Binary { .. }));
        assert_eq!(stats.folded_expressions, 0);
    }
    /// Preserva efeitos e asserções que podem lançar erro durante a execução.
    #[test]
    fn effects_and_null_assertions_are_not_evaluated() {
        for value in [
            binary(BinaryOp::Multiply, int(0), call()),
            binary(BinaryOp::And, expr(ExprKind::Bool(false)), call()),
            binary(BinaryOp::Or, expr(ExprKind::Bool(true)), call()),
            expr(ExprKind::Unary {
                op: UnaryOp::NullAssert,
                operand: Box::new(expr(ExprKind::Null)),
            }),
            binary(
                BinaryOp::Equal,
                expr(ExprKind::Construct { class_id: 0 }),
                expr(ExprKind::Construct { class_id: 0 }),
            ),
        ] {
            let (value, stats) = folded(value);
            assert!(matches!(
                value.kind,
                ExprKind::Binary { .. } | ExprKind::Unary { .. }
            ));
            assert_eq!(stats.folded_expressions, 0);
        }
        let (value, stats) = folded(expr(ExprKind::MethodCall {
            receiver: Box::new(expr(ExprKind::Construct { class_id: 0 })),
            name: "effect",
            arguments: vec![binary(BinaryOp::Add, int(1), int(2))],
        }));
        assert!(matches!(value.kind, ExprKind::MethodCall { .. }));
        assert_eq!(stats.folded_expressions, 1);
    }
    /// Seleciona ?? somente com literal esquerdo e mantém exatamente o efeito executável.
    #[test]
    fn coalesce_folding_preserves_lazy_selection() {
        let (value, stats) = folded(binary(BinaryOp::IfNull, expr(ExprKind::Null), call()));
        assert!(matches!(value.kind, ExprKind::Call { .. }));
        assert_eq!(stats.folded_expressions, 1);
        let (value, stats) = folded(binary(BinaryOp::IfNull, int(1), call()));
        assert!(matches!(value.kind, ExprKind::Int(1)));
        assert_eq!(stats.folded_expressions, 1);
        let (value, stats) = folded(binary(BinaryOp::IfNull, call(), int(2)));
        assert!(matches!(value.kind, ExprKind::Binary { .. }));
        assert_eq!(stats.folded_expressions, 0);
        let (_, stats) = folded(binary(
            BinaryOp::IfNull,
            int(1),
            binary(BinaryOp::Multiply, int(3), int(4)),
        ));
        assert_eq!(stats.folded_expressions, 1);
    }
    /// Percorre campos, métodos, funções, cabeçalhos e corpos de laços sem remover efeitos.
    #[test]
    fn traverses_class_members_and_loop_positions() {
        let sum = || binary(BinaryOp::Add, int(1), int(2));
        let mut program = Program {
            types: vec![],
            extensions: vec![],
            classes: vec![Class {
                modifier: dartforge_syntax::ClassModifier::None,
                kind: dartforge_syntax::ClassKind::Class,
                mixins: vec![],
                is_mixin_application: false,
                mixin_origin: None,
                enum_arguments: vec![],
                enum_constructor_fields: vec![],
                is_abstract: false,
                is_interface: false,
                library_id: 0,
                interfaces: vec![],
                abstract_methods: vec![],
                enum_values: vec![],
                id: 0,
                name: "C",
                superclass: None,
                fields: vec![Field {
                    name: "x",
                    ty: Type::Int,
                    is_final: false,
                    initializer: sum(),
                    span: SPAN,
                }],
                methods: vec![Function {
                    is_getter: false,
                    type_parameters: vec![],
                    name: "f",
                    return_type: Type::Int,
                    parameters: vec![],
                    body: vec![stmt(StatementKind::Return(Some(sum())))],
                    span: SPAN,
                }],
                span: SPAN,
            }],
            functions: vec![Function {
                is_getter: false,
                type_parameters: vec![],
                name: "g",
                return_type: Type::Void,
                parameters: vec![],
                body: vec![stmt(StatementKind::Print(sum()))],
                span: SPAN,
            }],
            statements: vec![stmt(StatementKind::For {
                initializer: Some(Box::new(stmt(StatementKind::Variable {
                    is_const: false,
                    name: "i",
                    annotation: None,
                    is_final: false,
                    initializer: sum(),
                }))),
                condition: Some(binary(BinaryOp::Less, int(1), int(2))),
                update: Some(Box::new(stmt(StatementKind::Assign {
                    name: "i",
                    value: sum(),
                }))),
                body: vec![
                    stmt(StatementKind::FieldAssign {
                        receiver: expr(ExprKind::Construct { class_id: 0 }),
                        name: "x",
                        value: sum(),
                    }),
                    stmt(StatementKind::Continue),
                ],
            })],
        };
        let stats = fold_constants(&mut program);
        assert_eq!(stats.folded_expressions, 7);
        assert!(matches!(
            program.classes[0].fields[0].initializer.kind,
            ExprKind::Int(3)
        ));
        assert!(matches!(
            program.statements[0].kind,
            StatementKind::For { .. }
        ));
        assert_eq!(fold_constants(&mut program).folded_expressions, 0);
    }
}

mod merge;
pub use merge::{MergeStats, merge_identical_functions};
