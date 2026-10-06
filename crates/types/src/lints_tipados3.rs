//! Mais regras de lint que pedem os tipos estáticos e a resolução da
//! inferência (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos
//! emissores da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`).
//! Como em [`crate::lints_tipados`], devolvem achados neutros (a posição, o
//! nome único do código e os argumentos), que quem chama só emite com a
//! regra ligada.
//!
//! Aqui: `use_truncating_division`, `avoid_double_and_int_checks`,
//! `only_throw_errors`, `no_runtimeType_toString`, `avoid_dynamic_calls` e
//! `unrelated_type_equality_checks` (expressão e padrão relacional).
//!
//! Cada regra lê o mesmo dado do emissor:
//! - o tipo escrito de um `is` é o resolvido
//!   (`UnitBodyTypes::tipos_de_anotacoes`, o `TypeAnnotation.type`);
//! - o `realTarget` de uma seção de cascata é o alvo da cascata;
//! - o operando de `++`/`--` (posição de escrita) não tem tipo estático no
//!   analyzer: conta o tipo lido da expressão inteira;
//! - `typesAreUnrelated` usa a subtipagem normativa (`subtyping::is_subtype`),
//!   o `promoteToNonNull`, os limites dos parâmetros de tipo e o
//!   `lookUpConcreteMethod('call')`;
//! - o `typeForInterfaceCheck` desce pelo limite promovido e pelo declarado;
//! - a extensão só é pulada pelo `no_runtimeType_toString` quando o tipo
//!   estendido é uma interface que não é classe concreta.
//!
//! O tipo que a inferência não determinou (`unknown`) não é relatado: é falta
//! de dado da inferência, não um `InvalidType`.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::lints_tipados::Achado;
use crate::resolve::OutlineTypes;
use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use crate::subtyping::{SubtypeEnv, is_subtype};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionKind, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, DeclKind, ExprId, ExprKind, Initializer, MemberKind, PatternId, PatternKind, StmtKind, StringPart, UnaryOp};
use dartforge_intern::Interner;
use std::collections::HashMap;

fn sem_parenteses(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// A classe `nome` declarada na biblioteca `lib`.
fn classe_de(program: &Program, interner: &Interner, lib: Option<LibraryId>, nome: &str) -> Option<ClassId> {
    let sym = interner.lookup(nome)?;
    match program.library(lib?).declared.get(&sym)?.getter? {
        Element::Class(c) => Some(c),
        _ => None,
    }
}

/// O tipo estático da expressão, sem o que a inferência não determinou.
fn tipo(corpo: &UnitBodyTypes, core: &CoreTypes, table: &TypeTable, e: ExprId) -> Option<TypeId> {
    corpo.get_type(e).filter(|t| !core.is_unknown(table, *t))
}

/// O tipo é a interface `c`, não anulável (`isDartCoreInt`, …).
fn e_da_classe(table: &TypeTable, t: TypeId, c: Option<ClassId>) -> bool {
    matches!(table.get(t), Type::Interface { class, nullable: false, .. } if Some(*class) == c)
}

/// O `realTarget` de cada seção de cascata: o `CascadeTarget` de cada seção
/// aponta para o alvo da cascata.
fn alvos_de_cascata(a: &ast::Ast) -> HashMap<ExprId, ExprId> {
    let mut m = HashMap::new();
    for e in a.exprs.iter() {
        let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
        for &s in sections.iter() {
            // Desce pela cadeia de receptores da seção até o marcador.
            let mut x = s;
            loop {
                let proximo = match &a.expr(x).kind {
                    ExprKind::Property { target, .. }
                    | ExprKind::Index { target, .. }
                    | ExprKind::Call { target, .. }
                    | ExprKind::TypeArguments { target, .. }
                    | ExprKind::Assign { target, .. } => *target,
                    ExprKind::Unary { operand, .. } => *operand,
                    ExprKind::CascadeTarget => {
                        m.insert(x, *target);
                        break;
                    }
                    _ => break,
                };
                x = proximo;
            }
        }
    }
    m
}

/// Os achados das regras tipadas deste módulo na unidade `u`.
pub fn achados(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpo: &UnitBodyTypes,
    u: UnitId,
) -> Vec<Achado> {
    let mut out: Vec<Achado> = Vec::new();
    let a = &program.unit(u).ast;
    let cascatas = alvos_de_cascata(a);
    let real = |x: ExprId| if matches!(a.expr(x).kind, ExprKind::CascadeTarget) { cascatas.get(&x).copied() } else { Some(x) };

    truncar_divisao(interner, table, core, corpo, a, &mut out);
    double_e_int(table, core, corpo, a, &mut out);
    so_lancar_erros(program, interner, table, core, outline, corpo, a, &mut out);
    runtime_type_to_string(program, interner, table, outline, corpo, u, a, &real, &mut out);
    chamadas_dinamicas(interner, table, core, corpo, a, &real, &mut out);
    igualdade_sem_relacao(program, interner, table, core, outline, corpo, u, &mut out);
    out
}

/// `use_truncating_division`: `(a / b).toInt()` com `a` e `b` `int` e o `/`
/// do `dart:core`.
fn truncar_divisao(interner: &Interner, table: &TypeTable, core: &CoreTypes, corpo: &UnitBodyTypes, a: &ast::Ast, out: &mut Vec<Achado>) {
    for e in a.exprs.iter() {
        let ExprKind::Call { target, arguments } = &e.kind else { continue };
        if !arguments.args.is_empty() {
            continue;
        }
        let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind else { continue };
        if interner.resolve(name.sym) != "toInt" || !matches!(a.expr(*alvo).kind, ExprKind::Parenthesized(_)) {
            continue;
        }
        let ExprKind::Binary { op: BinaryOp::Div, left, right } = &a.expr(sem_parenteses(a, *alvo)).kind else { continue };
        let int = |x: ExprId| tipo(corpo, core, table, x).is_some_and(|t| e_da_classe(table, t, core.int_class));
        if int(*left) && int(*right) {
            out.push((e.span, "use_truncating_division", Vec::new()));
        }
    }
}

/// `avoid_double_and_int_checks`: `if (x is double) … else if (x is int)`,
/// com `x` parâmetro ou variável local, os tipos resolvidos iguais a
/// `double` e `int` (o `is!` também conta: o emissor não olha o `!`).
fn double_e_int(table: &mut TypeTable, core: &CoreTypes, corpo: &UnitBodyTypes, a: &ast::Ast, out: &mut Vec<Achado>) {
    for s in a.stmts.iter() {
        let StmtKind::If { condition, else_: Some(senao), .. } = &s.kind else { continue };
        let StmtKind::If { condition: c2, .. } = &a.stmt(*senao).kind else { continue };
        let (ExprKind::Is { value: v1, ty: t1, .. }, ExprKind::Is { value: v2, ty: t2, .. }) = (&a.expr(*condition).kind, &a.expr(*c2).kind) else {
            continue;
        };
        let (ExprKind::Identifier(n1), ExprKind::Identifier(n2)) = (&a.expr(*v1).kind, &a.expr(*v2).kind) else { continue };
        if n1.sym != n2.sym {
            continue;
        }
        let local = matches!(corpo.get_resolved(*v1), Some(Resolved::Local(_) | Resolved::Parameter { .. }));
        let mut resolvido = |t: ast::TypeId| corpo.tipos_de_anotacoes.get(&t).map(|&x| crate::ops::sem_exibicao(x, table));
        if local && resolvido(*t1) == Some(core.double) && resolvido(*t2) == Some(core.int) {
            out.push((a.expr(*c2).span, "avoid_double_and_int_checks", Vec::new()));
        }
    }
}

/// O `typeForInterfaceCheck`: o parâmetro de tipo vira o limite promovido
/// ou o declarado, até não ser mais parâmetro.
pub(crate) fn para_interface(table: &TypeTable, core: &CoreTypes, mut t: TypeId) -> TypeId {
    for _ in 0..64 {
        match table.get(t) {
            Type::Intersection { bound, .. } => t = *bound,
            Type::TypeParameter { param, .. } => {
                let p = table.param(*param);
                // Sem limite escrito, o limite é `Object?`.
                t = if p.explicito { p.bound } else { core.object_nullable };
            }
            _ => return t,
        }
    }
    t
}

/// `only_throw_errors`: o literal lançado, ou o tipo que não é `dynamic`,
/// `Never` nem implementa `Exception`/`Error` do `dart:core`.
#[allow(clippy::too_many_arguments)]
fn so_lancar_erros(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpo: &UnitBodyTypes,
    a: &ast::Ast,
    out: &mut Vec<Achado>,
) {
    let excecao = classe_de(program, interner, core.core_library, "Exception");
    let erro = classe_de(program, interner, core.core_library, "Error");
    // `implementsAnyInterface`: a própria ou algum supertipo.
    let implementa = |c: ClassId| {
        [excecao, erro]
            .into_iter()
            .flatten()
            .any(|k| c == k || (program.class(c).decl.is_some() && outline.hierarchy.get(c).is_some_and(|d| d.supertypes.contains_key(&k))))
    };
    for e in a.exprs.iter() {
        let ExprKind::Throw(x) = &e.kind else { continue };
        let lancado = a.expr(*x);
        // `Literal`: booleano, número, `null`, string, símbolo, coleção, record.
        let literal = matches!(
            lancado.kind,
            ExprKind::Int(_)
                | ExprKind::Double(_)
                | ExprKind::Bool(_)
                | ExprKind::Null
                | ExprKind::String(_)
                | ExprKind::Symbol(_)
                | ExprKind::List { .. }
                | ExprKind::SetOrMap { .. }
                | ExprKind::Record { .. }
        );
        let lancavel = match tipo(corpo, core, table, *x) {
            None => true,
            Some(t) => {
                if matches!(table.get(t), Type::Never) {
                    true
                } else {
                    match table.get(para_interface(table, core, t)) {
                        Type::Dynamic => true,
                        Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => implementa(*class),
                        _ => false,
                    }
                }
            }
        };
        if literal || !lancavel {
            out.push((lancado.span, "only_throw_errors", Vec::new()));
        }
    }
}

/// `no_runtimeType_toString`.
#[allow(clippy::too_many_arguments)]
fn runtime_type_to_string(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    outline: &OutlineTypes,
    corpo: &UnitBodyTypes,
    u: UnitId,
    a: &ast::Ast,
    real: &dyn Fn(ExprId) -> Option<ExprId>,
    out: &mut Vec<Achado>,
) {
    let nome = |s: dartforge_intern::SymbolId| interner.resolve(s);
    // `_canSkip`: as regiões em que a regra não age.
    let mut fora: Vec<Span> = Vec::new();
    for s in a.stmts.iter() {
        if matches!(s.kind, StmtKind::Assert { .. }) {
            fora.push(s.span);
        }
        if let StmtKind::Try { catches, .. } = &s.kind {
            fora.extend(catches.iter().map(|c| c.span));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            fora.extend(k.initializers.iter().filter_map(|i| match i {
                Initializer::Assert { span, .. } => Some(*span),
                _ => None,
            }));
        }
    }
    for e in a.exprs.iter() {
        if matches!(e.kind, ExprKind::Throw(_)) {
            fora.push(e.span);
        }
    }
    for (di, d) in a.decls.iter().enumerate() {
        match &d.kind {
            DeclKind::Mixin(_) => fora.push(d.span),
            DeclKind::Class(x) if x.modifiers.abstract_ => fora.push(d.span),
            // A extensão cujo tipo estendido é uma interface que não é
            // classe concreta.
            DeclKind::Extension(_) => {
                let Some(i) = program.extensions.iter().position(|x| x.decl.unit == u && x.decl.decl == ast::DeclId(di as u32)) else { continue };
                let Some(dados) = outline.extensions.get(i) else { continue };
                let pula = match table.get(dados.on) {
                    Type::Interface { class, .. } => {
                        let c = program.class(*class);
                        let concreta = matches!(c.kind, ClassKind::Class | ClassKind::MixinApplication) && !c.modifiers.abstract_ && !c.modifiers.sealed;
                        !concreta
                    }
                    // `Null` é classe concreta; `FutureOr` é abstrata; o tipo
                    // de extensão não é classe.
                    Type::Null => false,
                    Type::FutureOr { .. } | Type::ExtensionType { .. } => true,
                    _ => false,
                };
                if pula {
                    fora.push(d.span);
                }
            }
            _ => {}
        }
    }
    let pula = |s: Span| fora.iter().any(|&r| dentro(s, r));
    // `_isRuntimeTypeAccess`: `this.runtimeType`, `super.runtimeType`, ou o
    // identificador `runtimeType` cujo elemento é um getter.
    let e_getter = |r: Option<&Resolved>| match r {
        Some(Resolved::Member { member: MemberRef::Variable(_), .. }) | Some(Resolved::Element(Element::Variable(_))) => true,
        Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) => {
            program.function(*f).kind == FunctionKind::Getter || program.function(*f).variable.is_some()
        }
        Some(Resolved::ExtensionMember { member, .. }) => program.function(*member).kind == FunctionKind::Getter,
        _ => false,
    };
    let e_runtime_type = |x: ExprId| match &a.expr(x).kind {
        ExprKind::Property { target, name, .. } => nome(name.sym) == "runtimeType" && matches!(a.expr(*target).kind, ExprKind::This | ExprKind::Super),
        ExprKind::Identifier(n) => nome(n.sym) == "runtimeType" && e_getter(corpo.get_resolved(x)),
        _ => false,
    };
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::String(lit) => {
                for parte in lit.parts.iter() {
                    if let StringPart::Interpolation(x) = parte
                        && e_runtime_type(*x)
                        && !pula(a.expr(*x).span)
                    {
                        out.push((a.expr(*x).span, "no_runtimeType_toString", Vec::new()));
                    }
                }
            }
            ExprKind::Call { target, .. } => {
                if let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind
                    && nome(name.sym) == "toString"
                    && real(*alvo).is_some_and(&e_runtime_type)
                    && !pula(e.span)
                {
                    out.push((name.span, "no_runtimeType_toString", Vec::new()));
                }
            }
            _ => {}
        }
    }
}

/// `avoid_dynamic_calls`.
fn chamadas_dinamicas(
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    corpo: &UnitBodyTypes,
    a: &ast::Ast,
    real: &dyn Fn(ExprId) -> Option<ExprId>,
    out: &mut Vec<Achado>,
) {
    let nome = |s: dartforge_intern::SymbolId| interner.resolve(s);
    let tipo = |e: ExprId| tipo(corpo, core, table, e);
    let dinamico = |e: ExprId| tipo(e).is_some_and(|t| matches!(table.get(t), Type::Dynamic));
    let como = |x: ExprId| matches!(a.expr(sem_parenteses(a, x)).kind, ExprKind::As { .. });
    let mut achados: Vec<Span> = Vec::new();
    // `_reportIfDynamic`.
    let se_dinamico = |x: Option<ExprId>, achados: &mut Vec<Span>| -> bool {
        let Some(x) = x else { return false };
        if dinamico(x) && !como(x) {
            achados.push(a.expr(x).span);
            return true;
        }
        false
    };
    // `_reportIfDynamicOrFunction`.
    let dinamico_ou_funcao = |no: ExprId, tipo_estatico: Option<TypeId>, achados: &mut Vec<Span>| {
        let Some(t) = tipo_estatico else { return };
        if como(no) {
            return;
        }
        if matches!(table.get(t), Type::Dynamic) || e_da_classe(table, t, core.function_class) {
            achados.push(a.expr(no).span);
        }
    };
    let permitido = |n: &str| matches!(n, "hashCode" | "noSuchMethod" | "runtimeType" | "toString");
    // Os alvos de chamada não são acesso a propriedade.
    let mut chamados: std::collections::HashSet<ExprId> = std::collections::HashSet::new();
    for e in a.exprs.iter() {
        if let ExprKind::Call { target, .. } = &e.kind {
            chamados.insert(*target);
        }
    }
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        match &e.kind {
            // A criação de instância sem `new` não é chamada.
            ExprKind::Call { .. } if matches!(corpo.get_resolved(id), Some(Resolved::Constructor(_))) => {}
            ExprKind::Call { target, arguments } => match &a.expr(*target).kind {
                // `MethodInvocation` com alvo (ou em cascata).
                ExprKind::Property { target: alvo, name, .. } => {
                    let em_cascata = matches!(a.expr(*alvo).kind, ExprKind::CascadeTarget);
                    let n = nome(name.sym);
                    if !em_cascata {
                        let so_um_posicional = arguments.args.len() == 1 && arguments.args[0].name.is_none();
                        if (n == "noSuchMethod" && so_um_posicional) || (n == "toString" && arguments.args.is_empty()) {
                            continue;
                        }
                    }
                    let alvo_real = real(*alvo);
                    if !se_dinamico(alvo_real, &mut achados) {
                        // O `methodName`: o tipo do membro, salvo `call`
                        // sobre um tipo de função.
                        let tipo_do_alvo = alvo_real.and_then(tipo);
                        let estatico = match tipo_do_alvo {
                            Some(t) if n == "call" && matches!(table.get(t), Type::Function { .. }) => Some(t),
                            _ => tipo(*target),
                        };
                        if let Some(t) = estatico
                            && (matches!(table.get(t), Type::Dynamic) || e_da_classe(table, t, core.function_class))
                        {
                            achados.push(name.span);
                        }
                    }
                }
                // `MethodInvocation` sem alvo: o `methodName`.
                ExprKind::Identifier(_) => dinamico_ou_funcao(*target, tipo(*target), &mut achados),
                // `FunctionExpressionInvocation`: `node.function`.
                _ => dinamico_ou_funcao(*target, tipo(*target), &mut achados),
            },
            // `PropertyAccess`/`PrefixedIdentifier` (não alvo de chamada).
            ExprKind::Property { target, name, .. } if !chamados.contains(&id) => {
                if !permitido(nome(name.sym)) {
                    se_dinamico(real(*target), &mut achados);
                }
            }
            ExprKind::Index { target, .. } => {
                se_dinamico(real(*target), &mut achados);
            }
            ExprKind::Binary { op, left, .. } => {
                let definivel = !matches!(op, BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull);
                if definivel {
                    se_dinamico(Some(*left), &mut achados);
                }
            }
            ExprKind::Unary { op, operand } => match op {
                UnaryOp::NullAssert => {}
                // `++`/`--`: o operando é posição de escrita (sem tipo
                // estático no analyzer); conta o tipo lido.
                UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec => {
                    if dinamico(*operand) {
                        achados.push(e.span);
                    }
                }
                _ => {
                    se_dinamico(Some(*operand), &mut achados);
                }
            },
            // A atribuição com leitura `dynamic` (não `=`, não `??=`).
            ExprKind::Assign { op, target, .. } => {
                if !matches!(op, AssignOp::Assign | AssignOp::Compound(BinaryOp::IfNull)) && dinamico(*target) {
                    achados.push(e.span);
                }
            }
            _ => {}
        }
    }
    achados.sort_by_key(|s| (s.start, s.end));
    achados.dedup();
    out.extend(achados.into_iter().map(|s| (s, "avoid_dynamic_calls", Vec::new())));
}

/// `unrelated_type_equality_checks`: a expressão `a == b`/`a != b` de tipo
/// `bool` e o padrão relacional `== x`/`!= x`.
#[allow(clippy::too_many_arguments)]
fn igualdade_sem_relacao(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpo: &UnitBodyTypes,
    u: UnitId,
    out: &mut Vec<Achado>,
) {
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let future_or = classe_de(program, interner, core.async_library, "FutureOr");
    // `isFixnumIntX` contra `isCoreInt`.
    let fixnum = |table: &TypeTable, l: TypeId, r: TypeId| {
        let l_fixnum = match table.get(l) {
            Type::Interface { class, .. } => {
                let c = program.class(*class);
                matches!(interner.resolve(c.name), "Int32" | "Int64") && program.library(c.library).uri.starts_with("package:fixnum/")
            }
            _ => false,
        };
        l_fixnum && e_da_classe(table, r, core.int_class)
    };
    let mut casos: Vec<(Span, &'static str, TypeId, TypeId)> = Vec::new();
    for (k, e) in a.exprs.iter().enumerate() {
        let ExprKind::Binary { op: BinaryOp::Eq | BinaryOp::NotEq, left, right } = &e.kind else { continue };
        if matches!(a.expr(*left).kind, ExprKind::Null) || matches!(a.expr(*right).kind, ExprKind::Null) {
            continue;
        }
        // `node.staticType?.isDartCoreBool`.
        let Some(tb) = tipo(corpo, core, table, ExprId(k as u32)) else { continue };
        if !e_da_classe(table, tb, core.bool_class) {
            continue;
        }
        let (Some(te), Some(td)) = (tipo(corpo, core, table, *left), tipo(corpo, core, table, *right)) else { continue };
        // O token do operador: o primeiro depois do operando esquerdo.
        let i = dartforge_frontend::fonte::pular_brancos(unidade.source.as_bytes(), a.expr(*left).span.end);
        casos.push((Span { start: i, end: i + 2 }, "unrelated_type_equality_checks_in_expression", te, td));
    }
    for (k, p) in a.patterns.iter().enumerate() {
        let PatternKind::Relational { op: BinaryOp::Eq | BinaryOp::NotEq, value } = &p.kind else { continue };
        let Some(&valor) = corpo.tipos_casados.get(&PatternId(k as u32)) else { continue };
        let Some(operando) = tipo(corpo, core, table, *value) else { continue };
        casos.push((p.span, "unrelated_type_equality_checks_in_pattern", valor, operando));
    }
    for (span, codigo, l, r) in casos {
        let mut rel = Relacao { program, table: &mut *table, core, outline, future_or, call: interner.lookup("call") };
        if !rel.sem_relacao(l, r) || fixnum(table, l, r) {
            continue;
        }
        // `[rightType, leftType]` (na forma do padrão: `[operando, valor]`).
        let args = vec![crate::despejo::formatar(table, r, interner, program), crate::despejo::formatar(table, l, interner, program)];
        out.push((span, codigo, args));
    }
}

/// `typesAreUnrelated` do linter (`util/dart_type_utilities.dart:150`).
pub(crate) struct Relacao<'a> {
    pub(crate) program: &'a Program,
    pub(crate) table: &'a mut TypeTable,
    pub(crate) core: &'a CoreTypes,
    pub(crate) outline: &'a OutlineTypes,
    pub(crate) future_or: Option<ClassId>,
    /// O símbolo `call` (sem ele, nenhuma classe tem `call`).
    pub(crate) call: Option<dartforge_intern::SymbolId>,
}

impl Relacao<'_> {
    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    /// `isBottom`: `Never`, ou o parâmetro de tipo não anulável cujo limite
    /// (promovido, ou o escrito) é fundo.
    fn fundo(&self, t: TypeId) -> bool {
        match self.table.get(t) {
            Type::Never => true,
            Type::Intersection { bound, .. } => self.fundo(*bound),
            Type::TypeParameter { param, nullable: false } => {
                let p = self.table.param(*param);
                p.explicito && self.fundo(p.bound)
            }
            _ => false,
        }
    }

    /// O elemento e os argumentos de um tipo que o analyzer tem por
    /// `InterfaceType` (classe, enum, mixin, tipo de extensão, `FutureOr`).
    fn interface(&self, t: TypeId) -> Option<(ClassId, Vec<TypeId>)> {
        match self.table.get(t) {
            Type::Interface { class, args, .. } => Some((*class, args.to_vec())),
            Type::ExtensionType { decl, args, .. } => Some((*decl, args.to_vec())),
            Type::FutureOr { arg, .. } => self.future_or.map(|c| (c, vec![*arg])),
            _ => None,
        }
    }

    /// O parâmetro de tipo (também promovido) e o limite escrito dele.
    fn parametro(&self, t: TypeId) -> Option<Option<TypeId>> {
        let param = match self.table.get(t) {
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. } => *param,
            _ => return None,
        };
        let p = self.table.param(param);
        Some(if p.explicito { Some(p.bound) } else { None })
    }

    /// O `supertype` do elemento (nulo para `Object`, mixin e tipo de
    /// extensão).
    fn supertipo(&mut self, c: ClassId) -> Option<TypeId> {
        if matches!(self.program.class(c).kind, ClassKind::Mixin | ClassKind::ExtensionType) {
            return None;
        }
        let s = self.outline.classes.get(c.0 as usize).and_then(|x| x.supertype)?;
        Some(crate::ops::sem_exibicao(s, self.table))
    }

    pub(crate) fn sem_relacao(&mut self, l: TypeId, r: TypeId) -> bool {
        let dinamico = |t: &Type| matches!(t, Type::Dynamic);
        if self.fundo(l) || dinamico(self.table.get(l)) || self.fundo(r) || dinamico(self.table.get(r)) {
            return false;
        }
        let pl = crate::ops::non_nullable(l, self.table);
        let pr = crate::ops::non_nullable(r, self.table);
        let (cl, cr) = (crate::ops::sem_exibicao(pl, self.table), crate::ops::sem_exibicao(pr, self.table));
        if cl == cr || self.sub(pl, pr) || self.sub(pr, pl) {
            return false;
        }
        if let (Some((el, al)), Some((er, ar))) = (self.interface(pl), self.interface(pr)) {
            return self.interfaces_sem_relacao(el, &al, er, &ar);
        }
        if let (Some(bl), Some(br)) = (self.parametro(pl), self.parametro(pr)) {
            return match (bl, br) {
                (Some(x), Some(y)) => self.sem_relacao(x, y),
                _ => false,
            };
        }
        if matches!(self.table.get(pl), Type::Function { .. }) {
            return self.funcao_sem_relacao(pr);
        }
        if matches!(self.table.get(pr), Type::Function { .. }) {
            return self.funcao_sem_relacao(pl);
        }
        if matches!(self.table.get(pl), Type::Record { .. }) || matches!(self.table.get(pr), Type::Record { .. }) {
            // `isAssignableTo` entre tipos não `dynamic`: a subtipagem.
            return !self.sub(pl, pr) && !self.sub(pr, pl);
        }
        false
    }

    /// `interfaceTypesAreUnrelated`.
    fn interfaces_sem_relacao(&mut self, el: ClassId, al: &[TypeId], er: ClassId, ar: &[TypeId]) -> bool {
        if el == er {
            if al.len() != ar.len() {
                return false;
            }
            return al.iter().zip(ar.iter()).any(|(x, y)| self.sem_relacao(*x, *y));
        }
        let (sl, sr) = (self.supertipo(el), self.supertipo(er));
        let mesmos = sl == sr;
        if mesmos && self.program.class(el).kind == ClassKind::Enum {
            return true;
        }
        let de_object = sl.is_some_and(|s| matches!(self.table.get(s), Type::Interface { class, nullable: false, .. } if Some(*class) == self.core.object_class));
        de_object || !mesmos
    }

    /// `_isFunctionTypeUnrelatedToType`: só a classe com `call` concreto
    /// (`lookUpConcreteMethod`) e o tipo de função se relacionam.
    fn funcao_sem_relacao(&mut self, outro: TypeId) -> bool {
        match self.table.get(outro) {
            Type::Function { .. } => false,
            Type::Interface { class, .. } => {
                let c = self.program.class(*class);
                if matches!(c.kind, ClassKind::Class | ClassKind::MixinApplication)
                    && let Some(call) = self.call
                    && crate::fase_super::concreto(self.program, *class, true, call, c.library, false, true)
                {
                    return false;
                }
                true
            }
            _ => true,
        }
    }
}
