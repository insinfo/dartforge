//! Mais regras de lint que pedem os tipos estáticos e a resolução da
//! inferência (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos
//! emissores da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`).
//! Como em [`crate::lints_tipados`], devolvem achados neutros (a posição, o
//! nome único do código e os argumentos), que quem chama só emite com a
//! regra ligada.
//!
//! Aqui: `avoid_bool_literals_in_conditional_expressions`,
//! `no_literal_bool_comparisons`, `unnecessary_string_interpolations`,
//! `prefer_contains` (os três códigos), `prefer_is_not_empty`,
//! `use_is_even_rather_than_modulo`, `unnecessary_to_list_in_spreads`,
//! `unnecessary_null_aware_assignments` e `await_only_futures`.
//!
//! Diferenças conhecidas:
//! - `getIntValue` do original avalia uma constante inteira qualquer; aqui
//!   só o literal, com `-` na frente.
//! - O contexto constante de `use_is_even_rather_than_modulo` é aproximado
//!   como em `lints_tipados` (variável `const`, anotação, coleção ou criação
//!   `const`).
//! - `prefer_is_not_empty` aceita o `isEmpty` de uma classe que declara
//!   `isNotEmpty` nela ou num supertipo (o original pergunta ao elemento que
//!   declara o `isEmpty`); o `isEmpty` de extensão fica fora.
//! - `unnecessary_string_interpolations` reconhece o literal único pelo
//!   texto (`'$x'`, `"${e}"`).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::lints_tipados::Achado;
use crate::resolve::OutlineTypes;
use crate::resolved::{Resolved, UnitBodyTypes};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Program, UnitId};
use dartforge_frontend::ast::{
    self, AssignOp, BinaryOp, CollectionElement, CreationKeyword, DeclKind, ExprId, ExprKind, Initializer, MemberKind, StmtKind, StringPart,
    UnaryOp,
};
use dartforge_intern::Interner;

fn sem_parenteses(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

/// O valor de um literal inteiro (decimal ou `0x`), sem sinal.
fn literal_inteiro(a: &ast::Ast, fonte: &str, e: ExprId) -> Option<i64> {
    let ExprKind::Int(s) = &a.expr(e).kind else { return None };
    let texto: String = fonte.get(s.start..s.end)?.chars().filter(|c| *c != '_').collect();
    match texto.strip_prefix("0x").or_else(|| texto.strip_prefix("0X")) {
        Some(hex) => i64::from_str_radix(hex, 16).ok(),
        None => texto.parse::<i64>().ok(),
    }
}

/// `getIntValue` pela forma: o literal, com `-` na frente.
fn valor_inteiro(a: &ast::Ast, fonte: &str, e: ExprId) -> Option<i64> {
    match &a.expr(e).kind {
        ExprKind::Unary { op: UnaryOp::Neg, operand } => literal_inteiro(a, fonte, *operand).map(|v| -v),
        _ => literal_inteiro(a, fonte, e),
    }
}

/// Os elementos `...x` de uma coleção, por dentro de `if` e `for`.
fn espalhados(el: &CollectionElement, saida: &mut Vec<ExprId>) {
    match el {
        CollectionElement::Spread { value, .. } => saida.push(*value),
        CollectionElement::If { then, else_, .. } => {
            espalhados(then, saida);
            if let Some(x) = else_ {
                espalhados(x, saida);
            }
        }
        CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => espalhados(body, saida),
        _ => {}
    }
}

/// Os achados das regras tipadas deste módulo na unidade `u`.
pub fn achados(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpo: &UnitBodyTypes,
    u: UnitId,
) -> Vec<Achado> {
    let mut out: Vec<Achado> = Vec::new();
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let fonte = unidade.source.as_str();

    let da_classe = |t: TypeId, alvo: Option<ClassId>| matches!(table.get(t), Type::Interface { class, .. } if Some(*class) == alvo);
    // `bool` e `String` não anuláveis.
    let nao_nula = |t: TypeId, alvo: Option<ClassId>| {
        matches!(table.get(t), Type::Interface { class, nullable: false, .. } if Some(*class) == alvo)
    };
    // A classe do tipo é `alvo` ou o tem entre os supertipos.
    let implementa = |t: TypeId, alvo: Option<ClassId>| match table.get(t) {
        Type::Interface { class, .. } => alvo.is_some_and(|k| *class == k || outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.contains_key(&k))),
        _ => false,
    };
    let tipo = |e: ExprId| corpo.get_type(e).filter(|t| !core.is_unknown(table, *t));

    // Os trechos em contexto constante (aproximado).
    let mut constantes: Vec<Span> = Vec::new();
    let mut de_lista = |l: &ast::VariableList| {
        if l.const_ {
            constantes.extend(l.variables.iter().filter_map(|v| v.initializer).map(|e| a.expr(e).span));
        }
    };
    for d in a.decls.iter() {
        if let DeclKind::Variables(l) = &d.kind {
            de_lista(l);
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Field(l) = &m.kind {
            de_lista(l);
        }
    }
    for s in a.stmts.iter() {
        if let StmtKind::Variables(l) = &s.kind {
            de_lista(l);
        }
    }
    constantes.extend(a.decls.iter().flat_map(|d| d.metadata.iter()).chain(a.members.iter().flat_map(|m| m.metadata.iter())).map(|m| m.span));
    // Os `assert` da lista de inicializadores de construtor `const`.
    let mut asserts_const: Vec<Span> = Vec::new();
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind
            && k.const_
        {
            asserts_const.extend(k.initializers.iter().filter_map(|i| match i {
                Initializer::Assert { span, .. } => Some(*span),
                _ => None,
            }));
        }
    }
    for e in a.exprs.iter() {
        let constante = match &e.kind {
            ExprKind::List { const_, .. } | ExprKind::SetOrMap { const_, .. } | ExprKind::Record { const_, .. } => *const_,
            ExprKind::InstanceCreation { keyword, .. } => *keyword == Some(CreationKeyword::Const),
            _ => false,
        };
        if constante {
            constantes.push(e.span);
        }
    }
    let em_constante = |s: Span| constantes.iter().any(|k| k.start <= s.start && s.end <= k.end);

    // `_isUnassignedIndexOf`: `x.indexOf(v)` ou `x.indexOf(v, 0)` com `x`
    // um `Iterable` ou uma `String` (por dentro de parênteses e de `as`).
    let de_indice = |e: ExprId| {
        let mut e = sem_parenteses(a, e);
        while let ExprKind::As { value, .. } = &a.expr(e).kind {
            e = *value;
        }
        let e = sem_parenteses(a, e);
        let ExprKind::Call { target, arguments } = &a.expr(e).kind else { return false };
        let ExprKind::Property { target: receptor, name, .. } = &a.expr(*target).kind else { return false };
        if interner.resolve(name.sym) != "indexOf" {
            return false;
        }
        let Some(t) = tipo(*receptor) else { return false };
        if !implementa(t, core.iterable_class) && !implementa(t, core.string_class) {
            return false;
        }
        match &arguments.args[..] {
            [_, inicio] => valor_inteiro(a, fonte, inicio.value) == Some(0),
            _ => true,
        }
    };
    let nao_vazio = interner.lookup("isNotEmpty");

    for e in a.exprs.iter() {
        match &e.kind {
            // `avoid_bool_literals_in_conditional_expressions`.
            ExprKind::Conditional { then, else_, .. } => {
                let (x, y) = (sem_parenteses(a, *then), sem_parenteses(a, *else_));
                let booleana = |z: ExprId| tipo(z).is_some_and(|t| nao_nula(t, core.bool_class));
                if booleana(x) && booleana(y) && (matches!(a.expr(x).kind, ExprKind::Bool(_)) || matches!(a.expr(y).kind, ExprKind::Bool(_))) {
                    out.push((e.span, "avoid_bool_literals_in_conditional_expressions", Vec::new()));
                }
            }
            ExprKind::Binary { op, left, right } => {
                // `no_literal_bool_comparisons`.
                if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
                    let literal = |z: ExprId| matches!(a.expr(z).kind, ExprKind::Bool(_));
                    let booleana = |z: ExprId| tipo(z).is_some_and(|t| nao_nula(t, core.bool_class));
                    if literal(*right) && booleana(*left) {
                        out.push((a.expr(*right).span, "no_literal_bool_comparisons", Vec::new()));
                    } else if literal(*left) && booleana(*right) {
                        out.push((a.expr(*left).span, "no_literal_bool_comparisons", Vec::new()));
                    }
                }
                // `prefer_contains`.
                if matches!(op, BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::Gt | BinaryOp::GtEq | BinaryOp::Lt | BinaryOp::LtEq) {
                    // A comparação com a constante à direita.
                    let comparacao = match valor_inteiro(a, fonte, *right) {
                        Some(v) => (v <= 0 && de_indice(*left)).then_some((v, *op)),
                        None => valor_inteiro(a, fonte, *left).filter(|v| *v <= 0 && de_indice(*right)).map(|v| {
                            let invertida = match op {
                                BinaryOp::Gt => BinaryOp::Lt,
                                BinaryOp::GtEq => BinaryOp::LtEq,
                                BinaryOp::Lt => BinaryOp::Gt,
                                BinaryOp::LtEq => BinaryOp::GtEq,
                                outra => *outra,
                            };
                            (v, invertida)
                        }),
                    };
                    let codigo = comparacao.and_then(|(v, o)| match v {
                        -1 => match o {
                            BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::LtEq | BinaryOp::Gt => Some("prefer_contains_use_contains"),
                            BinaryOp::Lt => Some("prefer_contains_always_false"),
                            BinaryOp::GtEq => Some("prefer_contains_always_true"),
                            _ => None,
                        },
                        0 => matches!(o, BinaryOp::GtEq | BinaryOp::Lt).then_some("prefer_contains_use_contains"),
                        _ => match o {
                            BinaryOp::Eq | BinaryOp::LtEq | BinaryOp::Lt => Some("prefer_contains_always_false"),
                            BinaryOp::NotEq | BinaryOp::GtEq | BinaryOp::Gt => Some("prefer_contains_always_true"),
                            _ => None,
                        },
                    });
                    if let Some(c) = codigo {
                        out.push((e.span, c, Vec::new()));
                    }
                }
                // `use_is_even_rather_than_modulo`: `x % 2 == n`, inteiros.
                if *op == BinaryOp::Eq
                    && let Some(valor) = literal_inteiro(a, fonte, *right)
                    && let ExprKind::Binary { op: BinaryOp::Rem, right: divisor, .. } = &a.expr(*left).kind
                    && literal_inteiro(a, fonte, *divisor) == Some(2)
                    && tipo(*left).is_some_and(|t| da_classe(t, core.int_class))
                    && tipo(*right).is_some_and(|t| da_classe(t, core.int_class))
                    && tipo(*divisor).is_some_and(|t| da_classe(t, core.int_class))
                    && !em_constante(e.span)
                    && !asserts_const.iter().any(|k| k.start <= e.span.start && e.span.end <= k.end)
                {
                    out.push((e.span, "use_is_even_rather_than_modulo", vec![(if valor == 0 { "isEven" } else { "isOdd" }).to_string()]));
                }
            }
            // `unnecessary_string_interpolations`: `'$x'` com `x` uma `String`.
            ExprKind::String(lit) => {
                let mut interpolada: Option<ExprId> = None;
                let mut so_uma = true;
                for parte in lit.parts.iter() {
                    match parte {
                        StringPart::Interpolation(x) if interpolada.is_none() => interpolada = Some(*x),
                        StringPart::Interpolation(_) => so_uma = false,
                        StringPart::Text(_) => {}
                    }
                }
                let Some(x) = interpolada.filter(|_| so_uma) else { continue };
                let Some(texto) = fonte.get(e.span.start..e.span.end) else { continue };
                let aspas = ["'''", "\"\"\"", "'", "\""].into_iter().find(|q| texto.starts_with(q));
                let Some(aspas) = aspas else { continue };
                let dentro = a.expr(x).span;
                // Só `$`/`${` entre a aspa e a expressão, e só `}` e a aspa
                // depois dela: um literal único de início e fim vazios.
                let antes = fonte.get(e.span.start + aspas.len()..dentro.start).map(|t| t.trim());
                let depois = fonte.get(dentro.end..e.span.end).map(|t| t.trim());
                let (Some(antes), Some(depois)) = (antes, depois) else { continue };
                let unico = (antes == "$" && depois == aspas) || (antes == "${" && depois.strip_prefix('}').is_some_and(|r| r == aspas));
                if unico && tipo(x).is_some_and(|t| nao_nula(t, core.string_class)) {
                    out.push((e.span, "unnecessary_string_interpolations", Vec::new()));
                }
            }
            // `prefer_is_not_empty`: `!x.isEmpty`.
            ExprKind::Unary { op: UnaryOp::Not, operand } => {
                let alvo = sem_parenteses(a, *operand);
                if let ExprKind::Property { target, name, .. } = &a.expr(alvo).kind
                    && interner.resolve(name.sym) == "isEmpty"
                    && let Some(simbolo) = nao_vazio
                    && let Some(t) = tipo(*target)
                    && let Type::Interface { class, .. } = table.get(t)
                    && !matches!(corpo.get_resolved(alvo), Some(Resolved::ExtensionMember { .. }))
                {
                    let declara = |k: ClassId| program.class(k).instance_members.contains_key(&simbolo);
                    if declara(*class) || outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.keys().any(|k| declara(*k))) {
                        out.push((e.span, "prefer_is_not_empty", Vec::new()));
                    }
                }
            }
            // `unnecessary_to_list_in_spreads`.
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                let mut valores = Vec::new();
                for el in elements.iter() {
                    espalhados(el, &mut valores);
                }
                for v in valores {
                    if let ExprKind::Call { target, .. } = &a.expr(v).kind
                        && let ExprKind::Property { target: receptor, name, .. } = &a.expr(*target).kind
                        && interner.resolve(name.sym) == "toList"
                        && tipo(*receptor).is_some_and(|t| implementa(t, core.iterable_class))
                    {
                        out.push((name.span, "unnecessary_to_list_in_spreads", Vec::new()));
                    }
                }
            }
            // `unnecessary_null_aware_assignments`: `x ??= null`, salvo
            // quando a escrita vai para um setter (campo, variável de topo).
            ExprKind::Assign { op: AssignOp::Compound(BinaryOp::IfNull), target, value } => {
                let para_setter = matches!(
                    corpo.get_resolved(*target),
                    Some(Resolved::Member { .. } | Resolved::Element(_) | Resolved::ExtensionMember { .. })
                );
                if matches!(a.expr(sem_parenteses(a, *value)).kind, ExprKind::Null) && !para_setter {
                    out.push((e.span, "unnecessary_null_aware_assignments", Vec::new()));
                }
            }
            // `await_only_futures`.
            ExprKind::Await(operando) => {
                if matches!(a.expr(*operando).kind, ExprKind::Null) || corpo.tipos_invalidos.contains(operando) {
                    continue;
                }
                let Some(t) = tipo(*operando) else { continue };
                let aceito = match table.get(t) {
                    Type::Dynamic | Type::FutureOr { .. } | Type::ExtensionType { .. } => true,
                    Type::Interface { .. } => implementa(t, core.future_class),
                    _ => false,
                };
                if !aceito && fonte.get(e.span.start..).is_some_and(|x| x.starts_with("await")) {
                    out.push((Span { start: e.span.start, end: e.span.start + 5 }, "await_only_futures", vec![table.format(t, interner, program)]));
                }
            }
            _ => {}
        }
    }
    out
}
