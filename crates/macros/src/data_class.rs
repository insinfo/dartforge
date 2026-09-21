//! Materialização de `@DataClass()`: `copyWith`, `igualA` e `descrever`.
//!
//! As três assinaturas são escolhidas pelo que o subconjunto aceita hoje, não pelo
//! que Dart 3.6.2 permitiria a uma macro completa. Cada desvio está documentado
//! junto da função que o produz e reaparece no texto de aumento de [`crate::augment`].
use crate::{ExpansionReport, MacroPlan, Origin, nullable};
use dartforge_diagnostics::Diagnostic;
use dartforge_syntax::*;

/// Nome do método que devolve uma cópia com campos substituídos.
pub(crate) const COPY_WITH: &str = "copyWith";
/// Nome do método de igualdade estrutural; `operator ==` não é declarável.
pub(crate) const EQUALS: &str = "igualA";
/// Nome do método de descrição textual; `toString` é reservado pela análise.
pub(crate) const DESCRIBE: &str = "descrever";
/// Nome do parâmetro de `igualA`; campos homônimos continuam acessíveis por `this`.
pub(crate) const OTHER: &str = "outro";
/// Conjunto reservado pela macro, conferido contra os membros já declarados.
pub(crate) const MEMBERS: [&str; 3] = [COPY_WITH, EQUALS, DESCRIBE];

/// Índices dos métodos reservados, na ordem em que entram em `Class::methods`.
pub(crate) struct Reserved {
    pub copy_with: usize,
    pub equals: usize,
    pub describe: usize,
}

/// Pedaço de `descrever()`: texto constante ou o valor de um campo `String`.
///
/// A mesma sequência alimenta a materialização em AST e a emissão textual, de modo
/// que as duas não podem divergir no formato produzido.
pub(crate) enum DescribePiece {
    /// Literal já acumulado, incluindo nomes de campos e marcadores de tipo.
    Literal(String),
    /// Valor de um campo `String`; anulável recebe `?? 'null'` para permanecer `String`.
    Field { index: usize, nullable: bool },
}

/// Texto que substitui o valor de um campo que o subconjunto não sabe concatenar.
///
/// `+` aceita apenas dois `int` ou dois `String`, não há `toString()` nem
/// interpolação, então `int` e `bool` não têm representação textual no subconjunto.
fn placeholder(ty: Type) -> &'static str {
    match ty {
        Type::Int => "<int>",
        Type::NullableInt => "<int?>",
        Type::Bool => "<bool>",
        Type::NullableBool => "<bool?>",
        _ => "<?>",
    }
}

/// Monta o formato `Nome(campo: valor, ...)` fundindo literais adjacentes.
///
/// Literais vizinhos são unidos antes da materialização, o que reduz o número de
/// nós sintéticos e mantém o texto emitido idêntico ao código gerado.
pub(crate) fn describe_pieces(class_name: &str, plan: &MacroPlan) -> Vec<DescribePiece> {
    let mut pieces = Vec::new();
    let mut literal = format!("{class_name}(");
    for (index, field) in plan.fields.iter().enumerate() {
        if index > 0 {
            literal.push_str(", ");
        }
        literal.push_str(&field.name);
        literal.push_str(": ");
        match field.ty {
            Type::String | Type::NullableString => {
                pieces.push(DescribePiece::Literal(std::mem::take(&mut literal)));
                pieces.push(DescribePiece::Field {
                    index,
                    nullable: field.ty == Type::NullableString,
                });
            }
            other => literal.push_str(placeholder(other)),
        }
    }
    literal.push(')');
    pieces.push(DescribePiece::Literal(literal));
    pieces
}

/// Reserva as três assinaturas sem gerar nenhum corpo de método.
///
/// `copyWith` recebe um parâmetro posicional anulável por campo, na ordem de
/// declaração e com o nome do próprio campo. O subconjunto não tem parâmetros
/// nomeados nem opcionais, então todos os argumentos são obrigatórios e `null`
/// é o único jeito de dizer "mantém o valor atual"; um campo anulável não pode
/// ser zerado por `copyWith`.
pub(crate) fn reserve(
    class: &mut Class<'_>,
    plan: &MacroPlan,
    origin: Origin,
    report: &mut ExpansionReport,
) -> Result<Reserved, Diagnostic> {
    let class_id = class.id;
    let parameters = class
        .fields
        .iter()
        .zip(&plan.fields)
        .map(|(field, planned)| {
            Ok(Parameter::required(
                field.name,
                nullable(planned.ty),
                report.span(origin)?,
            ))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let copy_with = class.methods.len();
    class.methods.push(Function {
        is_arrow: false,
        is_async: false,
        annotations: vec![],
        native_binding: None,
        type_parameters: vec![],
        is_getter: false,
        name: COPY_WITH,
        return_type: Type::Class(class_id),
        parameters,
        body: vec![],
        span: report.span(origin)?,
    });
    let equals = class.methods.len();
    class.methods.push(Function {
        is_arrow: false,
        is_async: false,
        annotations: vec![],
        native_binding: None,
        type_parameters: vec![],
        is_getter: false,
        name: EQUALS,
        return_type: Type::Bool,
        parameters: vec![Parameter::required(
            OTHER,
            Type::Class(class_id),
            report.span(origin)?,
        )],
        body: vec![],
        span: report.span(origin)?,
    });
    let describe = class.methods.len();
    class.methods.push(Function {
        is_arrow: false,
        is_async: false,
        annotations: vec![],
        native_binding: None,
        type_parameters: vec![],
        is_getter: false,
        name: DESCRIBE,
        return_type: Type::String,
        parameters: vec![],
        body: vec![],
        span: report.span(origin)?,
    });
    report.declared(MEMBERS.len());
    Ok(Reserved {
        copy_with,
        equals,
        describe,
    })
}

/// Preenche os três corpos após todas as assinaturas de todas as classes existirem.
pub(crate) fn define(
    class: &mut Class<'_>,
    plan: &MacroPlan,
    reserved: &Reserved,
    origin: Origin,
    report: &mut ExpansionReport,
) -> Result<(), Diagnostic> {
    define_copy_with(class, reserved.copy_with, origin, report)?;
    define_equals(class, reserved.equals, origin, report)?;
    define_describe(class, plan, reserved.describe, origin, report)
}

/// `return Nome(arg0 ?? this.campo0, ...)`; `??` preserva o tipo declarado do campo.
fn define_copy_with(
    class: &mut Class<'_>,
    method: usize,
    origin: Origin,
    report: &mut ExpansionReport,
) -> Result<(), Diagnostic> {
    let class_id = class.id;
    let mut arguments = Vec::with_capacity(class.fields.len());
    for field in &class.fields {
        let left = report.expr(ExprKind::Identifier(field.name), origin)?;
        let this = report.expr(ExprKind::This, origin)?;
        let right = report.expr(
            ExprKind::Member {
                receiver: Box::new(this),
                name: field.name,
            },
            origin,
        )?;
        arguments.push(report.expr(
            ExprKind::Binary {
                op: BinaryOp::IfNull,
                left: Box::new(left),
                right: Box::new(right),
            },
            origin,
        )?);
    }
    let value = report.expr(
        ExprKind::Construct {
            class_id,
            arguments,
        },
        origin,
    )?;
    class.methods[method].body = report.ret(value, origin)?;
    Ok(())
}

/// `return this.a == outro.a && ...`; classe sem campos devolve `true`.
///
/// A comparação usa `==` campo a campo, que no subconjunto é igualdade de valor
/// para `int`, `bool` e `String` e aceita `null` dos dois lados. Não substitui
/// `operator ==`: `a == b` entre instâncias continua sendo identidade.
fn define_equals(
    class: &mut Class<'_>,
    method: usize,
    origin: Origin,
    report: &mut ExpansionReport,
) -> Result<(), Diagnostic> {
    let mut condition: Option<Expr<'_>> = None;
    for field in &class.fields {
        let this = report.expr(ExprKind::This, origin)?;
        let left = report.expr(
            ExprKind::Member {
                receiver: Box::new(this),
                name: field.name,
            },
            origin,
        )?;
        let other = report.expr(ExprKind::Identifier(OTHER), origin)?;
        let right = report.expr(
            ExprKind::Member {
                receiver: Box::new(other),
                name: field.name,
            },
            origin,
        )?;
        let compared = report.expr(
            ExprKind::Binary {
                op: BinaryOp::Equal,
                left: Box::new(left),
                right: Box::new(right),
            },
            origin,
        )?;
        condition = Some(match condition {
            None => compared,
            Some(accumulated) => report.expr(
                ExprKind::Binary {
                    op: BinaryOp::And,
                    left: Box::new(accumulated),
                    right: Box::new(compared),
                },
                origin,
            )?,
        });
    }
    let value = match condition {
        Some(expression) => expression,
        None => report.expr(ExprKind::Bool(true), origin)?,
    };
    class.methods[method].body = report.ret(value, origin)?;
    Ok(())
}

/// Concatena os pedaços de `descrever()` com `+` associando à esquerda.
///
/// Todos os operandos são `String`: literais, campos `String` e campos `String?`
/// protegidos por `?? 'null'`. Uma classe sem campos `String` produz um único
/// literal, sem nenhuma concatenação.
fn define_describe(
    class: &mut Class<'_>,
    plan: &MacroPlan,
    method: usize,
    origin: Origin,
    report: &mut ExpansionReport,
) -> Result<(), Diagnostic> {
    let mut value: Option<Expr<'_>> = None;
    for piece in describe_pieces(class.name, plan) {
        let operand = match piece {
            DescribePiece::Literal(text) => report.expr(ExprKind::OwnedString(text), origin)?,
            DescribePiece::Field { index, nullable } => {
                let this = report.expr(ExprKind::This, origin)?;
                let member = report.expr(
                    ExprKind::Member {
                        receiver: Box::new(this),
                        name: class.fields[index].name,
                    },
                    origin,
                )?;
                if nullable {
                    let fallback =
                        report.expr(ExprKind::OwnedString(String::from("null")), origin)?;
                    report.expr(
                        ExprKind::Binary {
                            op: BinaryOp::IfNull,
                            left: Box::new(member),
                            right: Box::new(fallback),
                        },
                        origin,
                    )?
                } else {
                    member
                }
            }
        };
        value = Some(match value {
            None => operand,
            Some(accumulated) => report.expr(
                ExprKind::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(accumulated),
                    right: Box::new(operand),
                },
                origin,
            )?,
        });
    }
    let value = value.expect("describe_pieces sempre produz ao menos um literal");
    class.methods[method].body = report.ret(value, origin)?;
    Ok(())
}
