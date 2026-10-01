//! Relevância dos itens do completar como o servidor do Dart 3.6.2 a calcula
//! (`services/completion/dart/relevance_computer.dart` e
//! `feature_computer.dart`): uma média ponderada de características,
//! levada a `0..1000`, e `sortText = 9999 − relevância`.
//!
//! * **tipo de contexto** — o tipo que a posição espera (parâmetro do
//!   argumento, alvo da atribuição, tipo escrito da variável, retorno da
//!   função, condição `bool`) contra o tipo do item: igual 1,0; subtipo 0,40;
//!   supertipo 0,02; sem relação 0,13; sem um dos dois, 0;
//! * **espécie do elemento** — a faixa de probabilidade da espécie (classe,
//!   local, parâmetro, campo, método, função…) no local do completar
//!   (`Block_statement`, `ArgumentList_method_unnamed`,
//!   `PropertyAccess_propertyName`…), das tabelas geradas do Dart
//!   (`relevancia_tabelas.rs`); o meio da faixa, ou, com distância
//!   (locais pela proximidade, membros pela herança), `meio + (alto −
//!   baixo) × distância / 2`;
//! * **palavra-chave** — o alto da faixa da palavra no local;
//! * **não importado** −1, **começa com `$`** −1 (peso 0,5), **noSuchMethod**
//!   −1, **distância do local** `0,9^d`.
//!
//! `relevância = ⌊((Σ vᵢ·pᵢ / Σ pᵢ) + 1) / 2 × 1000⌋`, com os pesos do Dart
//! (todos 1, menos depreciado e `$`, 0,5). Argumentos nomeados têm valor
//! fixo (900; 950 se obrigatório), como no Dart.

use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, StmtKind};

/// Espécie de elemento das tabelas do Dart (`computeElementKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Especie {
    Classe,
    Construtor,
    Enum,
    Campo,
    Funcao,
    AliasDeFuncao,
    Local,
    Metodo,
    Mixin,
    Parametro,
    Prefixo,
    VariavelDeTopo,
    ParametroDeTipo,
    Desconhecido,
    /// Espécies sem tabela (constante de enum, extensão, alias de tipo):
    /// característica de espécie 0.
    SemTabela,
}

/// O que se sabe de um item para pontuá-lo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Rel {
    /// Espécie, quando o item é um elemento.
    pub especie: Option<Especie>,
    /// O tipo do item (variável, retorno, getter), para o tipo de contexto.
    pub tipo: Option<dartforge_types::TypeId>,
    /// Distância (`d`, que vira `0,9^d`): local pela proximidade, membro
    /// pela herança.
    pub distancia: Option<u32>,
    /// Local ou parâmetro: a distância entra também como característica.
    pub local: bool,
    pub nao_importado: bool,
    /// Palavra-chave (o texto).
    pub palavra: Option<&'static str>,
    /// Relevância fixa (argumentos nomeados).
    pub fixa: Option<i32>,
    pub comeca_com_dolar: bool,
    pub no_such_method: bool,
}

/// `distanceToPercent`.
pub(crate) fn porcentagem(distancia: usize) -> f64 {
    0.9f64.powi(distancia as i32)
}

/// A característica de espécie no `local`.
fn caracteristica_de_especie(especie: Especie, local: Option<&str>, distancia: Option<f64>) -> f64 {
    let Some(local) = local else { return 0.0 };
    let Some((_, baixo, alto)) = super::relevancia_tabelas::especies(local).iter().find(|(e, _, _)| *e == especie) else {
        return 0.0;
    };
    let meio = (baixo + alto) / 2.0;
    match distancia {
        None => meio,
        Some(d) => meio + ((alto - baixo) * d / 2.0),
    }
}

/// A característica de palavra-chave no `local` (o alto da faixa).
fn caracteristica_de_palavra(palavra: &str, local: Option<&str>) -> f64 {
    let Some(local) = local else { return 0.0 };
    let tabela = super::relevancia_tabelas::palavras(local);
    let achar = |p: &str| tabela.iter().find(|(k, _, _)| *k == p).map(|(_, _, alto)| *alto);
    achar(palavra)
        .or_else(|| palavra.find(|c: char| !c.is_ascii_lowercase()).filter(|i| *i > 0).and_then(|i| achar(&palavra[..i])))
        .unwrap_or(0.0)
}

/// A relevância (`0..1000`) de um item.
pub(crate) fn relevancia(rel: &Rel, local: Option<&str>, tipo_de_contexto: f64) -> i32 {
    if let Some(f) = rel.fixa {
        return f;
    }
    let distancia = rel.distancia.map(|d| porcentagem(d as usize));
    let especie = rel.especie.map_or(0.0, |e| caracteristica_de_especie(e, local, distancia));
    let palavra = rel.palavra.map_or(0.0, |p| caracteristica_de_palavra(p, local));
    let nao_importado = if rel.nao_importado { -1.0 } else { 0.0 };
    let dolar = if rel.comeca_com_dolar { -1.0 } else { 0.0 };
    let nsm = if rel.no_such_method { -1.0 } else { 0.0 };
    let distancia_local = if rel.local { distancia.unwrap_or(0.0) } else { 0.0 };
    // Pesos do Dart: contexto 1, espécie 1, depreciado 0,5, constante 1,
    // noSuchMethod 1, não importado 1, palavra 1, `$` 0,5, super 1,
    // distância do local 1 — total 9.
    let soma = tipo_de_contexto + especie + nsm + nao_importado + palavra + 0.5 * dolar + distancia_local;
    let media = soma / 9.0;
    (((media + 1.0) / 2.0) * 1000.0) as i32
}

/// O texto do operador como o analyzer o escreve no local
/// (`BinaryExpression_+_rightOperand`).
fn operador(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::TruncDiv => "~/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::IfNull => "??",
    }
}

/// O local do completar (`completionLocation` do Dart) de uma expressão:
/// o papel dela no nó pai. `chamada_construtor` diz se uma chamada é a
/// criação de uma instância (`A(…)` sem `new`).
pub(crate) fn local_da_expressao(ast: &ast::Ast, expr: ExprId, chamada_construtor: &dyn Fn(ExprId) -> bool) -> Option<String> {
    let alvo = expr;
    // Comandos.
    for s in &ast.stmts {
        let local = match &s.kind {
            StmtKind::Expression(e) if *e == alvo => Some("Block_statement"),
            StmtKind::Return(Some(e)) if *e == alvo => Some("ReturnStatement_expression"),
            StmtKind::If { condition, .. } if *condition == alvo => Some("IfStatement_condition"),
            StmtKind::While { condition, .. } if *condition == alvo => Some("WhileStatement_condition"),
            StmtKind::DoWhile { condition, .. } if *condition == alvo => Some("DoStatement_condition"),
            StmtKind::For { condition: Some(c), .. } if *c == alvo => Some("ForParts_condition"),
            StmtKind::For { updates, .. } if updates.contains(&alvo) => Some("ForParts_updater"),
            StmtKind::ForIn { iterable, .. } if *iterable == alvo => Some("ForEachPartsWithDeclaration_iterable"),
            StmtKind::Yield { value, .. } if *value == alvo => Some("YieldStatement_expression"),
            StmtKind::Assert { condition, .. } if *condition == alvo => Some("AssertStatement_condition"),
            StmtKind::Assert { message: Some(m), .. } if *m == alvo => Some("AssertStatement_message"),
            StmtKind::Switch { value, .. } if *value == alvo => Some("SwitchStatement_expression"),
            StmtKind::Variables(l) if l.variables.iter().any(|v| v.initializer == Some(alvo)) => Some("VariableDeclaration_initializer"),
            _ => None,
        };
        if local.is_some() {
            return local.map(str::to_string);
        }
    }
    for d in &ast.decls {
        if let ast::DeclKind::Variables(l) = &d.kind
            && l.variables.iter().any(|v| v.initializer == Some(alvo))
        {
            return Some("VariableDeclaration_initializer".into());
        }
    }
    for m in &ast.members {
        match &m.kind {
            ast::MemberKind::Field(l) if l.variables.iter().any(|v| v.initializer == Some(alvo)) => {
                return Some("VariableDeclaration_initializer".into());
            }
            ast::MemberKind::Constructor(k) => {
                for i in k.initializers.iter() {
                    match i {
                        ast::Initializer::Field { value, .. } if *value == alvo => return Some("ConstructorFieldInitializer_expression".into()),
                        ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } => {
                            if let Some(a) = arguments.args.iter().find(|a| a.value == alvo) {
                                let tipo = if a.name.is_some() { "named" } else { "unnamed" };
                                return Some(format!("ArgumentList_constructorRedirect_{tipo}"));
                            }
                        }
                        _ => {}
                    }
                }
                for p in k.parameters.iter() {
                    if p.default_value == Some(alvo) {
                        return Some("DefaultFormalParameter_defaultValue".into());
                    }
                }
            }
            _ => {}
        }
    }
    for f in &ast.functions {
        if let ast::FunctionBody::Expression(e) = f.body
            && e == alvo
        {
            return Some("ExpressionFunctionBody_expression".into());
        }
        if f.parameters.iter().flatten().any(|p| p.default_value == Some(alvo)) {
            return Some("DefaultFormalParameter_defaultValue".into());
        }
    }
    // Expressões.
    for (i, e) in ast.exprs.iter().enumerate() {
        let id = ExprId(i as u32);
        let local: Option<String> = match &e.kind {
            ExprKind::Call { target, arguments } => {
                if *target == alvo {
                    // Começo de uma invocação sem alvo: o pai decide.
                    return local_da_expressao(ast, id, chamada_construtor);
                }
                arguments.args.iter().find(|a| a.value == alvo).map(|a| {
                    let contexto = if chamada_construtor(id) {
                        "constructor"
                    } else if matches!(ast.expr(*target).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) {
                        "method"
                    } else {
                        "function"
                    };
                    format!("ArgumentList_{contexto}_{}", if a.name.is_some() { "named" } else { "unnamed" })
                })
            }
            ExprKind::InstanceCreation { arguments, .. } => arguments
                .args
                .iter()
                .find(|a| a.value == alvo)
                .map(|a| format!("ArgumentList_constructor_{}", if a.name.is_some() { "named" } else { "unnamed" })),
            ExprKind::Property { target, .. } if *target == alvo => {
                // `a▮.b`: o começo do acesso; o pai decide.
                return local_da_expressao(ast, id, chamada_construtor);
            }
            ExprKind::Assign { value, .. } if *value == alvo => Some("AssignmentExpression_rightHandSide".into()),
            ExprKind::Binary { op, right, .. } if *right == alvo => Some(format!("BinaryExpression_{}_rightOperand", operador(*op))),
            ExprKind::Binary { left, .. } if *left == alvo => return local_da_expressao(ast, id, chamada_construtor),
            ExprKind::Conditional { then, .. } if *then == alvo => Some("ConditionalExpression_thenExpression".into()),
            ExprKind::Conditional { else_, .. } if *else_ == alvo => Some("ConditionalExpression_elseExpression".into()),
            ExprKind::Parenthesized(x) if *x == alvo => Some("ParenthesizedExpression_expression".into()),
            ExprKind::Await(x) if *x == alvo => Some("AwaitExpression_expression".into()),
            ExprKind::Throw(x) if *x == alvo => Some("ThrowExpression_expression".into()),
            ExprKind::Index { index, .. } if *index == alvo => Some("IndexExpression_index".into()),
            ExprKind::List { elements, .. } if elementos_contem(elements, alvo) => Some("ListLiteral_element".into()),
            ExprKind::SetOrMap { elements, .. } if elementos_contem(elements, alvo) => Some(
                if elementos_valor(elements, alvo) { "MapLiteralEntry_value" } else { "SetOrMapLiteral_element" }.into(),
            ),
            _ => None,
        };
        if local.is_some() {
            return local;
        }
    }
    None
}

fn elementos_contem(elementos: &[ast::CollectionElement], alvo: ExprId) -> bool {
    elementos.iter().any(|e| match e {
        ast::CollectionElement::Expression(x) | ast::CollectionElement::NullAwareExpression(x) => *x == alvo,
        ast::CollectionElement::MapEntry { key, value, .. } => *key == alvo || *value == alvo,
        _ => false,
    })
}

fn elementos_valor(elementos: &[ast::CollectionElement], alvo: ExprId) -> bool {
    elementos.iter().any(|e| matches!(e, ast::CollectionElement::MapEntry { value, .. } if *value == alvo))
}
