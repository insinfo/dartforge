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
//! Cada regra lê o mesmo dado do emissor:
//! - `getIntValue(e, context)` (`prefer_contains`): o literal inteiro ou o
//!   identificador simples avaliado como constante (o `Motor` de
//!   constantes, o `computeConstantValue`), com `-` na frente;
//! - o contexto constante é o `inConstantContext` (`dartforge_frontend::pais`);
//! - o `implementsAnyInterface` desce pelos limites do parâmetro de tipo;
//! - `prefer_is_not_empty` pergunta ao elemento que declara o `isEmpty`
//!   (classe, mixin, enum, tipo de extensão ou extensão) se ele tem um filho
//!   `isNotEmpty` (`getChildren`);
//! - `unnecessary_string_interpolations` é o literal único (não adjacente)
//!   com exatamente uma interpolação e os trechos de antes e de depois
//!   vazios;
//! - `unnecessary_null_aware_assignments` só cala quando a escrita vai para
//!   um setter (o `[]=` não é setter).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::constantes::avaliador::{Constante, Ctx, Motor};
use crate::lints_tipados::Achado;
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved, UnitBodyTypes};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, CollectionElement, ExprId, ExprKind, Initializer, MemberKind, StringPart, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

fn sem_parenteses(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

/// `IntegerLiteral.value`: o literal inteiro (decimal ou `0x`, até 64 bits
/// no hexadecimal). Um literal que o parser tenha juntado com o `-` vale
/// negativo.
fn literal_inteiro(a: &ast::Ast, fonte: &str, e: ExprId) -> Option<i64> {
    let ExprKind::Int(s) = &a.expr(e).kind else { return None };
    let texto: String = fonte.get(s.start..s.end)?.chars().filter(|c| *c != '_' && !c.is_whitespace()).collect();
    let (negativo, texto) = match texto.strip_prefix('-') {
        Some(r) => (true, r.to_string()),
        None => (false, texto),
    };
    let v = match texto.strip_prefix("0x").or_else(|| texto.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16).ok().map(|v| v as i64),
        None => texto.parse::<i64>().ok(),
    }?;
    Some(if negativo { v.wrapping_neg() } else { v })
}

/// O literal escrito sem sinal (`right is IntegerLiteral`).
fn literal_sem_sinal(a: &ast::Ast, fonte: &str, e: ExprId) -> Option<i64> {
    let ExprKind::Int(s) = &a.expr(e).kind else { return None };
    if fonte.get(s.start..s.end).is_some_and(|t| t.starts_with('-')) {
        return None;
    }
    literal_inteiro(a, fonte, e)
}

/// `getIntValue(e, context)`: o literal ou o identificador constante, com
/// `-` na frente (só o `-`).
fn valor_inteiro(a: &ast::Ast, fonte: &str, avaliados: &HashMap<ExprId, i64>, e: ExprId) -> Option<i64> {
    let base = |x: ExprId| match &a.expr(x).kind {
        ExprKind::Int(_) => literal_inteiro(a, fonte, x),
        ExprKind::Identifier(_) => avaliados.get(&x).copied(),
        _ => None,
    };
    match &a.expr(e).kind {
        ExprKind::Unary { op: UnaryOp::Neg, operand } => base(*operand).map(i64::wrapping_neg),
        _ => base(e),
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

/// Os identificadores simples que o `getIntValue(…, context)` do
/// `prefer_contains` pode consultar: os operandos das comparações e o
/// segundo argumento de `indexOf`, também atrás de `-`.
fn identificadores_consultados(interner: &Interner, a: &ast::Ast) -> Vec<ExprId> {
    let mut v = Vec::new();
    let mut considerar = |e: ExprId| {
        let x = match &a.expr(e).kind {
            ExprKind::Unary { op: UnaryOp::Neg, operand } => *operand,
            _ => e,
        };
        if matches!(a.expr(x).kind, ExprKind::Identifier(_)) {
            v.push(x);
        }
    };
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Binary { op: BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::Gt | BinaryOp::GtEq | BinaryOp::Lt | BinaryOp::LtEq, left, right } => {
                considerar(*left);
                considerar(*right);
            }
            ExprKind::Call { target, arguments } if arguments.args.len() == 2 => {
                if let ExprKind::Property { name, .. } = &a.expr(*target).kind
                    && interner.resolve(name.sym) == "indexOf"
                {
                    considerar(arguments.args[1].value);
                }
            }
            _ => {}
        }
    }
    v
}

/// Os filhos do elemento `getChildren(elemento, nome)` (pelo `displayName`:
/// o setter `nome=` também conta).
fn tem_filho(program: &Program, interner: &Interner, dono: Dono, nome: &str) -> bool {
    let simbolos: Vec<SymbolId> = [interner.lookup(nome), interner.lookup(&format!("{nome}_="))].into_iter().flatten().collect();
    let tem = |m: &HashMap<SymbolId, dartforge_elements::model::FunctionElementId>| simbolos.iter().any(|s| m.contains_key(s));
    match dono {
        Dono::Classe(c) => {
            let e = program.class(c);
            tem(&e.instance_members)
                || tem(&e.static_members)
                || e.fields.iter().chain(e.enum_constants.iter()).any(|v| interner.resolve(program.variable(*v).name) == nome)
        }
        Dono::Extensao(x) => {
            let e = program.extension(x);
            tem(&e.instance_members) || tem(&e.static_members) || e.fields.iter().any(|v| interner.resolve(program.variable(*v).name) == nome)
        }
    }
}

#[derive(Clone, Copy)]
enum Dono {
    Classe(ClassId),
    Extensao(dartforge_elements::model::ExtensionId),
}

/// O elemento que declara o membro resolvido (`enclosingElement3`).
fn dono_do_membro(program: &Program, r: &Resolved) -> Option<Dono> {
    let (classe, extensao) = match r {
        Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => {
            let g = program.function(*f);
            (g.class, g.extension)
        }
        Resolved::Member { member: MemberRef::Variable(v), .. } => {
            let x = program.variable(*v);
            (x.class, x.extension)
        }
        _ => return None,
    };
    classe.map(Dono::Classe).or(extensao.map(Dono::Extensao))
}

/// Os achados das regras tipadas deste módulo na unidade `u`.
#[allow(clippy::too_many_arguments)]
pub fn achados(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpos: &BodyTypes,
    inferidas: &HashSet<LibraryId>,
    u: UnitId,
) -> Vec<Achado> {
    let mut out: Vec<Achado> = Vec::new();
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let fonte = unidade.source.as_str();
    let Some(corpo) = corpos.units.get(u.0 as usize) else { return out };

    // `computeConstantValue` dos identificadores consultados, com o motor de
    // constantes (só se houver algum).
    let mut avaliados: HashMap<ExprId, i64> = HashMap::new();
    let consultados = identificadores_consultados(interner, a);
    if !consultados.is_empty() {
        let mut motor = Motor::novo(program, interner, table, core, outline, corpos, inferidas);
        let cx = Ctx::simples(u, unidade.library);
        for e in consultados {
            if let Constante::Valor(v) = motor.avaliar(&cx, e, false)
                && let crate::constantes::valor::Estado::Int(Some(i)) = v.estado
            {
                avaliados.insert(e, i);
            }
        }
    }
    let table: &TypeTable = table;

    let nao_nula = |t: TypeId, alvo: Option<ClassId>| matches!(table.get(t), Type::Interface { class, nullable: false, .. } if Some(*class) == alvo);
    // `implementsInterface`: a interface é `alvo` ou o tem entre os
    // supertipos.
    let implementa = |t: TypeId, alvo: Option<ClassId>| match table.get(t) {
        Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => alvo.is_some_and(|k| {
            *class == k || (program.class(*class).decl.is_some() && outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.contains_key(&k)))
        }),
        _ => false,
    };
    // `implementsAnyInterface`: pelo `typeForInterfaceCheck`.
    let implementa_algum = |t: TypeId, alvos: &[Option<ClassId>]| {
        let t = crate::lints_tipados3::para_interface(table, core, t);
        alvos.iter().any(|k| implementa(t, *k))
    };
    let tipo = |e: ExprId| corpo.get_type(e).filter(|t| !core.is_unknown(table, *t));
    let pais = crate::lints_tipados::pais_da_unidade(program, u);

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

    // `_isUnassignedIndexOf`: `x.indexOf(v)` ou `x.indexOf(v, 0)` com `x`
    // um `Iterable` ou uma `String` (por dentro de parênteses e de `as`).
    let de_indice = |e: ExprId| {
        let mut e = sem_parenteses(a, e);
        while let ExprKind::As { value, .. } = &a.expr(e).kind {
            e = *value;
        }
        let e = sem_parenteses(a, e);
        let ExprKind::Call { target, arguments } = &a.expr(e).kind else { return false };
        if matches!(corpo.get_resolved(e), Some(Resolved::Constructor(_))) {
            return false;
        }
        let ExprKind::Property { target: receptor, name, .. } = &a.expr(*target).kind else { return false };
        if interner.resolve(name.sym) != "indexOf" {
            return false;
        }
        let Some(t) = tipo(*receptor) else { return false };
        if !implementa_algum(t, &[core.iterable_class, core.string_class]) {
            return false;
        }
        match &arguments.args[..] {
            [_, inicio] => valor_inteiro(a, fonte, &avaliados, inicio.value) == Some(0),
            _ => true,
        }
    };

    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
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
                    let comparacao = match valor_inteiro(a, fonte, &avaliados, *right) {
                        Some(v) => (v <= 0 && de_indice(*left)).then_some((v, *op)),
                        None => valor_inteiro(a, fonte, &avaliados, *left).filter(|v| *v <= 0 && de_indice(*right)).map(|v| {
                            // `TokenType.inverted`.
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
                // `use_is_even_rather_than_modulo`: `x % 2 == n`, inteiros,
                // com `n` literal sem sinal.
                if *op == BinaryOp::Eq
                    && !pais.em_contexto_constante(a, id)
                    && let Some(valor) = literal_sem_sinal(a, fonte, *right)
                    && let ExprKind::Binary { op: BinaryOp::Rem, right: divisor, .. } = &a.expr(*left).kind
                    && literal_sem_sinal(a, fonte, *divisor) == Some(2)
                    && tipo(*left).is_some_and(|t| nao_nula(t, core.int_class))
                    && tipo(*right).is_some_and(|t| nao_nula(t, core.int_class))
                    && tipo(*divisor).is_some_and(|t| nao_nula(t, core.int_class))
                    && !asserts_const.iter().any(|k| k.start <= e.span.start && e.span.end <= k.end)
                {
                    out.push((e.span, "use_is_even_rather_than_modulo", vec![(if valor == 0 { "isEven" } else { "isOdd" }).to_string()]));
                }
            }
            // `unnecessary_string_interpolations`: o literal único com uma
            // interpolação só e os trechos de antes e de depois vazios.
            ExprKind::String(lit) => {
                let interpolacoes: Vec<ExprId> = lit
                    .parts
                    .iter()
                    .filter_map(|p| match p {
                        StringPart::Interpolation(x) => Some(*x),
                        StringPart::Text(_) => None,
                    })
                    .collect();
                let [x] = interpolacoes.as_slice() else { continue };
                let x = *x;
                let Some(texto) = fonte.get(e.span.start..e.span.end) else { continue };
                let Some(aspas) = ["'''", "\"\"\"", "'", "\""].into_iter().find(|q| texto.starts_with(q)) else { continue };
                let dentro = a.expr(x).span;
                let (Some(antes), Some(depois)) = (fonte.get(e.span.start + aspas.len()..dentro.start), fonte.get(dentro.end..e.span.end)) else {
                    continue;
                };
                let unico = if antes == "$" {
                    depois == aspas
                } else if let Some(resto) = antes.strip_prefix("${") {
                    // Só brancos e comentários entre `${` e a expressão, e
                    // entre ela e o `}`; depois do `}`, só a aspa.
                    let b = resto.as_bytes();
                    let d = depois.as_bytes();
                    let k = dartforge_frontend::fonte::pular_brancos(d, 0);
                    dartforge_frontend::fonte::pular_brancos(b, 0) == b.len() && d.get(k) == Some(&b'}') && &depois[k + 1..] == aspas
                } else {
                    false
                };
                if unico && tipo(x).is_some_and(|t| nao_nula(t, core.string_class)) {
                    out.push((e.span, "unnecessary_string_interpolations", Vec::new()));
                }
            }
            // `prefer_is_not_empty`: `!x.isEmpty`, com o elemento que declara
            // o `isEmpty` tendo um filho `isNotEmpty`.
            ExprKind::Unary { op: UnaryOp::Not, operand } => {
                let alvo = sem_parenteses(a, *operand);
                if let ExprKind::Property { target, name, .. } = &a.expr(alvo).kind
                    && !matches!(a.expr(*target).kind, ExprKind::CascadeTarget)
                    && interner.resolve(name.sym) == "isEmpty"
                    && let Some(r) = corpo.get_resolved(alvo)
                    && let Some(dono) = dono_do_membro(program, r)
                    && tem_filho(program, interner, dono, "isNotEmpty")
                {
                    out.push((e.span, "prefer_is_not_empty", Vec::new()));
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
            // quando a escrita vai para um setter (campo, variável de topo,
            // setter de extensão; o `[]=` não é setter).
            ExprKind::Assign { op: AssignOp::Compound(BinaryOp::IfNull), target, value } => {
                let alvo = a.expr(*target);
                let para_setter = matches!(alvo.kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                    && matches!(
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
