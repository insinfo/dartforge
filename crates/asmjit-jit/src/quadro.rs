//! Cálculo do quadro de pilha antes de emitir uma única instrução.
//!
//! Um montador emite o prólogo antes de conhecer o corpo, mas o prólogo precisa
//! saber de quanta pilha a função vai precisar. Há duas saídas: emitir um
//! `sub rsp, imm32` remendável e corrigi-lo no fim, ou percorrer o corpo antes
//! e descobrir o tamanho. Este crate faz a segunda, que é determinística e
//! testável sem depender do mecanismo de *patch* da biblioteca.
//!
//! São duas contagens independentes:
//!
//! * **locais** — uma posição por declaração de variável encontrada no corpo,
//!   inclusive em escopos aninhados. Posições não são reaproveitadas entre
//!   escopos irmãos: gasta-se pilha para não precisar de análise de tempo de
//!   vida, que é exatamente o tipo de trabalho que um montador não faz.
//! * **temporários** — a profundidade máxima da pilha de avaliação de
//!   expressões. Como não há alocador de registradores, todo operando esquerdo
//!   de uma operação binária é derramado numa posição enquanto o direito é
//!   avaliado.
//!
//! O resultado vale para qualquer caminho do programa porque é um máximo, não
//! uma soma por caminho.
use dartforge_syntax::{Expr, ExprKind, Statement, StatementKind};

/// Conta as declarações de variável do corpo, inclusive as aninhadas.
///
/// Sobrecontagem é deliberada: duas variáveis em escopos irmãos recebem
/// posições distintas. O custo é pilha, e o ganho é não precisar de análise de
/// tempo de vida num backend que se propõe a ser o mais simples possível.
pub(crate) fn contar_locais(corpo: &[Statement<'_>]) -> usize {
    corpo.iter().map(locais_da_instrucao).sum()
}

/// Declarações de variável de uma instrução e de tudo que ela contém.
fn locais_da_instrucao(instrucao: &Statement<'_>) -> usize {
    match &instrucao.kind {
        StatementKind::Variable { .. } => 1,
        StatementKind::Block(corpo) => contar_locais(corpo),
        StatementKind::If {
            then_body,
            else_body,
            ..
        } => contar_locais(then_body) + else_body.as_deref().map_or(0, contar_locais),
        StatementKind::While { body, .. } | StatementKind::DoWhile { body, .. } => {
            contar_locais(body)
        }
        StatementKind::For {
            initializer,
            update,
            body,
            ..
        } => {
            initializer.as_deref().map_or(0, locais_da_instrucao)
                + update.as_deref().map_or(0, locais_da_instrucao)
                + contar_locais(body)
        }
        // As demais formas estão fora da fatia e serão recusadas pelo tradutor
        // com mensagem e span próprios; contar zero aqui não muda o resultado.
        _ => 0,
    }
}

/// Profundidade máxima da pilha de avaliação exigida pelo corpo.
pub(crate) fn profundidade(corpo: &[Statement<'_>]) -> usize {
    corpo
        .iter()
        .map(profundidade_da_instrucao)
        .max()
        .unwrap_or(0)
}

/// Profundidade máxima exigida por uma instrução e por tudo que ela contém.
fn profundidade_da_instrucao(instrucao: &Statement<'_>) -> usize {
    match &instrucao.kind {
        StatementKind::Variable { initializer, .. } => necessidade(initializer),
        StatementKind::Assign { value, .. }
        | StatementKind::Print(value)
        | StatementKind::Expression(value) => necessidade(value),
        StatementKind::Return(valor) => valor.as_ref().map_or(0, necessidade),
        StatementKind::Block(corpo) => profundidade(corpo),
        StatementKind::If {
            condition,
            then_body,
            else_body,
        } => necessidade(condition)
            .max(profundidade(then_body))
            .max(else_body.as_deref().map_or(0, profundidade)),
        StatementKind::While { condition, body } | StatementKind::DoWhile { condition, body } => {
            necessidade(condition).max(profundidade(body))
        }
        StatementKind::For {
            initializer,
            condition,
            update,
            body,
        } => initializer
            .as_deref()
            .map_or(0, profundidade_da_instrucao)
            .max(condition.as_ref().map_or(0, necessidade))
            .max(update.as_deref().map_or(0, profundidade_da_instrucao))
            .max(profundidade(body)),
        _ => 0,
    }
}

/// Posições temporárias necessárias para avaliar uma expressão.
///
/// O esquema de avaliação é fixo e sem registradores livres:
///
/// * folha (literal, identificador): nenhuma posição;
/// * unário: o que o operando exigir;
/// * binário: o esquerdo é derramado na posição corrente e o direito é avaliado
///   uma posição acima, logo `max(esq, 1 + dir)`;
/// * `&&` e `||`: nada é derramado, porque o resultado de um lado já é o
///   resultado da expressão, logo `max(esq, dir)`;
/// * chamada de `n` argumentos: o argumento `i` é avaliado com `i` posições já
///   ocupadas e derramado na posição `i`, logo `max(n, max_i(i + arg_i))`.
///
/// `print` é tratado como chamada de um argumento cujo valor vai direto para o
/// registrador da ABI, sem derrame: `necessidade(argumento)`.
fn necessidade(expressao: &Expr<'_>) -> usize {
    match &expressao.kind {
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Identifier(_) => 0,
        ExprKind::Unary { operand, .. } => necessidade(operand),
        ExprKind::Binary { op, left, right } => {
            if matches!(
                op,
                dartforge_syntax::BinaryOp::And | dartforge_syntax::BinaryOp::Or
            ) {
                necessidade(left).max(necessidade(right))
            } else {
                necessidade(left).max(1 + necessidade(right))
            }
        }
        ExprKind::Call { name, arguments } => {
            if *name == "print" {
                return arguments.first().map_or(0, necessidade);
            }
            let derrames = arguments
                .iter()
                .enumerate()
                .map(|(indice, argumento)| indice + necessidade(argumento))
                .max()
                .unwrap_or(0);
            derrames.max(arguments.len())
        }
        // Formas fora da fatia não chegam à emissão: o tradutor as recusa antes.
        _ => 0,
    }
}
