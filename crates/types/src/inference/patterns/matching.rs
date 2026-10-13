//! O estado do casamento de padrões no fluxo (docs/ANALYZER-ESPECIFICACAO.md
//! §G, T6): o porte de `_pushScrutinee`/`_pushPattern`/`_popPattern`,
//! `promoteForPattern`, `_nullCheckPattern`, `_handleEqualityCheckPattern` e
//! dos `logicalOrPattern_*` do `_FlowAnalysisImpl`
//! (`_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart`), com os
//! chamadores do `TypeAnalyzer` (`type_inference/type_analyzer.dart`).
//!
//! O valor casado é uma **referência temporária** com chave de promoção
//! própria: aqui, um local sintético do corpo, cujo modelo vive no
//! [`Fluxo`] corrente como o de qualquer variável. O tipo casado de um
//! padrão é o tipo promovido dessa chave na entrada do padrão; promover a
//! chave promove também o escrutínio, quando ele é uma referência que a
//! construção deixa promover. O estado "não casou" é acumulado à parte
//! ([`Casamento::nao_casou`]) e começa inalcançável: ele só fica alcançável
//! quando algum padrão pode falhar.
//!
//! Só o analisador usa este caminho (ver [`ligado`]); os compiladores ficam
//! com a tipagem antiga de `super::tipar`.

use super::super::body::{Body, Local, Nome};
use super::super::expr::{self, declarar_local, inferir};
use super::super::flow::Fluxo;
use super::super::members::Busca;
use super::super::BodyInferrer;
use super::{
    invalid_annotation, implicit_named_field, nome_implicito, never_matches, record_pattern_type,
    object_pattern_type,
};
use crate::resolved::LocalId;
use crate::table::{Type, TypeId};
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, ExprId, ListPatternElement, PatternId, PatternKind};

/// A referência do valor casado de um padrão (ou subpadrão).
#[derive(Debug, Clone)]
pub(crate) struct MatchedRef {
    /// O local sintético que carrega as promoções do valor casado.
    pub chave: LocalId,
    /// O tipo com que a referência foi criada.
    pub tipo: TypeId,
    /// O escrutínio, quando é uma referência promovível.
    pub alvo: Option<LocalId>,
    /// O escrutínio é uma propriedade promovível (promove em qualquer
    /// construção); senão é variável e só promove enquanto não for reescrita.
    pub propriedade: bool,
    /// A versão de escrita da variável escrutinada no começo do casamento.
    pub versao: Option<u32>,
}

/// O casamento em curso de um padrão de topo.
#[derive(Debug, Clone)]
pub(crate) struct Matching {
    /// `_unmatched`.
    pub not_matched: Fluxo,
    /// A referência do padrão de topo e as dos subpadrões abertos.
    pub pilha: Vec<MatchedRef>,
}

/// O fluxo de padrões do analyzer vale só no modo do analisador (o mesmo
/// que preserva a exibição dos tipos): ele muda o tipo promovido visto por
/// todo corpo com `switch` ou `if-case`, e os emissores JS e nativo
/// consomem as promoções (T6.7).
pub(crate) fn ligado(inf: &BodyInferrer<'_>) -> bool {
    inf.table.preservar_exibicao
}

fn language_patterns(inf: &BodyInferrer<'_>, cx: &Body) -> bool {
    inf.program.library(cx.lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0)
}

/// Um local sintético para a chave de promoção de um valor casado.
pub(crate) fn new_key(inf: &BodyInferrer<'_>, cx: &mut Body, typ: TypeId) -> LocalId {
    let the_name = match inf.sym.vazio.or(inf.sym.new_) {
        Some(s) => s,
        None => inf.program.classes[0].name,
    };
    cx.declare_synthetic(Local { nome: the_name, tipo: typ, final_: true, late: false, const_: false, offset: 0, funcao_local: false })
}

/// Casa com a referência `r` o que `f` tipa e devolve o estado "não casou".
pub(crate) fn casar(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    r: MatchedRef,
    f: impl FnOnce(&mut BodyInferrer<'_>, &mut Body),
) -> Fluxo {
    let not_matched = cx.flow.unreachable();
    let saved = cx.matching.replace(Matching { not_matched, pilha: vec![r] });
    f(inf, cx);
    let end = std::mem::replace(&mut cx.matching, saved);
    match end {
        Some(c) => c.not_matched,
        None => cx.flow.unreachable(),
    }
}

fn topo(cx: &Body) -> Option<MatchedRef> {
    cx.matching.as_ref().and_then(|c| c.pilha.last().cloned())
}

/// `_getMatchedValueType`: o tipo promovido da chave do topo.
fn matched(cx: &Body, r: &MatchedRef) -> TypeId {
    cx.flow.current_type(r.chave, r.tipo)
}

/// `_unmatched = _join(_unmatched, extra)`.
fn join_not_matched(inf: &mut BodyInferrer<'_>, cx: &mut Body, extra: &Fluxo) {
    let Some(old) = cx.matching.as_ref().map(|c| c.not_matched.clone()) else { return };
    let new_ = inf.juntar(&old, extra);
    if let Some(c) = cx.matching.as_mut() {
        c.not_matched = new_;
    }
}

/// O escrutínio a promover junto com a chave: a propriedade, sempre; a
/// variável, só se a construção deixa e ela não foi reescrita desde o
/// começo do casamento (`promoteForPattern`, `:5227-5235`).
fn valid_target(cx: &Body, r: &MatchedRef) -> Option<LocalId> {
    let id = r.alvo?;
    if r.propriedade || cx.flow.versao(id) == r.versao {
        Some(id)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Classe {
    NonNull,
    Nulo,
    Talvez,
}

/// `classifyType` (`flow_analysis_visitor.dart:446-456`).
fn classificar(inf: &mut BodyInferrer<'_>, t: TypeId) -> Classe {
    let (object, null) = (inf.core.object, inf.core.null);
    if inf.sub(t, object) {
        Classe::NonNull
    } else if inf.sub(t, null) {
        Classe::Nulo
    } else {
        Classe::Talvez
    }
}

/// O ramo verdadeiro de `tryPromoteForTypeCheck`. Promover nunca torna o
/// fluxo inalcançável, nem para `Never`.
fn promote_yes(inf: &mut BodyInferrer<'_>, f: &mut Fluxo, id: LocalId, declared: TypeId, k: TypeId) {
    let reachable = f.alcancavel;
    inf.promote_tested(f, id, declared, k, k);
    f.alcancavel = reachable;
}

/// O ramo falso: promove para `factor(atual, K)`, salvo quando o fator é
/// `Never` ou o próprio tipo atual (aí só o tipo de interesse fica).
fn promote_not(inf: &mut BodyInferrer<'_>, f: &mut Fluxo, id: LocalId, declared: TypeId, k: TypeId) {
    let reachable = f.alcancavel;
    let s = f.current_type(id, declared);
    let fat = expr::factor(inf, s, k);
    let target = if fat == s || matches!(inf.table.get(fat), Type::Never) { s } else { fat };
    inf.promote_tested(f, id, declared, target, k);
    f.alcancavel = reachable;
}

/// `promoteForPattern` (`:5191-5258`): devolve se `K` cobre o tipo casado.
fn promote_for_pattern(inf: &mut BodyInferrer<'_>, cx: &mut Body, k: TypeId, failure: bool, can: bool) -> bool {
    // O tipo conhecido inválido (`Unresolved()`, `:5196-5199`): o estado
    // atual vai para o "não casou", sem promoção, e não cobre.
    if inf.table.e_invalido(k) {
        let current = cx.flow.clone();
        join_not_matched(inf, cx, &current);
        return false;
    }
    let Some(r) = topo(cx) else { return true };
    let t = matched(cx, &r);
    // Tipo casado não anulável: o conhecido vira o não anulável dele
    // (`:5201-5207`), e `case int? x?` promove a `int`.
    let k = if classificar(inf, t) == Classe::NonNull { inf.non_null_promotion(k) } else { k };
    let covers = inf.sub(t, k);
    let mut yes = cx.flow.clone();
    let mut not_ = cx.flow.clone();
    promote_yes(inf, &mut yes, r.chave, r.tipo, k);
    promote_not(inf, &mut not_, r.chave, r.tipo, k);
    if let Some(id) = valid_target(cx, &r) {
        let d = cx.local(id).tipo;
        promote_yes(inf, &mut yes, id, d, k);
        promote_not(inf, &mut not_, id, d, k);
    }
    cx.flow = yes;
    if can || (failure && !covers) {
        let extra = if covers { cx.flow.clone() } else { not_ };
        join_not_matched(inf, cx, &extra);
    }
    covers
}

/// `_nullCheckPattern` (`:6019-6032`): o estado "não é nulo", ou `None`
/// quando o tipo casado já é não anulável (nada a checar).
fn check_null(inf: &mut BodyInferrer<'_>, cx: &mut Body) -> Option<Fluxo> {
    let r = topo(cx)?;
    let t = matched(cx, &r);
    let cls = classificar(inf, t);
    if cls == Classe::NonNull {
        return None;
    }
    let nn = inf.non_null_promotion(t);
    let mut yes = cx.flow.clone();
    promote_yes(inf, &mut yes, r.chave, r.tipo, nn);
    if let Some(id) = valid_target(cx, &r) {
        let d = cx.local(id).tipo;
        let current = yes.current_type(id, d);
        let nn_target = inf.non_null_promotion(current);
        promote_yes(inf, &mut yes, id, d, nn_target);
    }
    if cls == Classe::Nulo {
        yes.alcancavel = false;
    }
    Some(yes)
}

/// `_handleEqualityCheckPattern` (`:5832-5903`): padrão constante (`==`) e
/// relacional `==`/`!=` contra `operando`, de tipo `tipo_operando`.
fn equality(inf: &mut BodyInferrer<'_>, cx: &mut Body, the_operand: ExprId, operand_type: TypeId, different: bool) {
    let Some(r) = topo(cx) else { return };
    let t = matched(cx, &r);
    let a = &inf.program.unit(cx.unit).ast;
    let mut x = the_operand;
    while let ast::ExprKind::Parenthesized(i) = &a.expr(x).kind {
        x = *i;
    }
    let null_literal = matches!(a.expr(x).kind, ast::ExprKind::Null);
    let (ct, co) = (classificar(inf, t), classificar(inf, operand_type));
    let current = cx.flow.clone();
    if ct == Classe::Nulo && co == Classe::Nulo {
        // Iguais com certeza: `==` sempre casa, `!=` nunca.
        if different {
            join_not_matched(inf, cx, &current);
            cx.flow.alcancavel = false;
        }
    } else if (ct == Classe::Nulo && co == Classe::NonNull) || (co == Classe::Nulo && ct == Classe::NonNull) {
        // Sem informação (no modo fraco poderiam ser iguais).
        join_not_matched(inf, cx, &current);
    } else if null_literal {
        // A comparação com `null` é uma checagem de nulo do valor casado.
        let non_null = check_null(inf, cx).unwrap_or_else(|| current.clone());
        if different {
            join_not_matched(inf, cx, &current);
            cx.flow = non_null;
        } else {
            join_not_matched(inf, cx, &non_null);
        }
    } else {
        join_not_matched(inf, cx, &current);
    }
}

/// `pushSubpattern(tipo)` … `popSubpattern`: `f` tipa o subpadrão contra uma
/// referência nova de tipo `tipo`; devolve o tipo demonstrado (o promovido
/// da referência depois do subpadrão).
fn with_subpattern(inf: &mut BodyInferrer<'_>, cx: &mut Body, typ: TypeId, f: impl FnOnce(&mut BodyInferrer<'_>, &mut Body)) -> TypeId {
    let the_key = new_key(inf, cx, typ);
    if let Some(c) = cx.matching.as_mut() {
        c.pilha.push(MatchedRef { chave: the_key, tipo: typ, alvo: None, propriedade: false, versao: None });
    }
    f(inf, cx);
    let shown = cx.flow.current_type(the_key, typ);
    if let Some(c) = cx.matching.as_mut() {
        c.pilha.pop();
    }
    shown
}

/// A palavra `as` de um padrão de cast, entre o subpadrão e o tipo.
fn as_keyword(inf: &BodyInferrer<'_>, cx: &Body, internal: PatternId, ty: ast::TypeId) -> Option<Span> {
    let a = &inf.program.unit(cx.unit).ast;
    let (start, end) = (a.pattern(internal).span.end, a.ty(ty).span.start);
    let source = inf.program.unit(cx.unit).source.as_bytes();
    let snippet = source.get(start..end)?;
    let mut i = 0;
    while i + 2 <= snippet.len() {
        let before_ok = i == 0 || !(snippet[i - 1].is_ascii_alphanumeric() || snippet[i - 1] == b'_');
        let after_ok = snippet.get(i + 2).is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_'));
        if &snippet[i..i + 2] == b"as" && before_ok && after_ok {
            return Some(Span { start: start + i, end: start + i + 2 });
        }
        i += 1;
    }
    None
}

/// Tipa o padrão `p` contra o valor casado do topo da pilha, declarando as
/// variáveis. `de_e`: `p` é operando direto de `&&` (o único lugar em que um
/// curinga que sempre casa é relatado como desnecessário).
/// O padrão está num contexto irrefutável (declaração, atribuição, `for-in`)
/// que nenhum padrão refutável de fora tornou refutável.
fn irrefutable_now(cx: &Body) -> bool {
    !cx.padrao_refutavel && !cx.forced_refutable
}

/// `refutablePatternInIrrefutableContext` (`shared_type_analyzer.dart:187-194`):
/// relata o padrão refutável e torna o resto refutável. Devolve se relatou
/// (quem chamou desfaz o `refutavel_forcado` no fim do padrão).
fn refutable_in_irrefutable(inf: &mut BodyInferrer<'_>, cx: &mut Body, sp: Span) -> bool {
    if !irrefutable_now(cx) {
        return false;
    }
    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::REFUTABLE_PATTERN_IN_IRREFUTABLE_CONTEXT, sp, &[]);
    cx.forced_refutable = true;
    true
}

/// `patternTypeMismatchInIrrefutableContext` (`shared_type_analyzer.dart:174-185`),
/// no padrão inteiro, com o tipo casado e o requerido.
fn wrong_type_in_irrefutable(inf: &mut BodyInferrer<'_>, matched: TypeId, the_required: TypeId, sp: Span) {
    let a1 = inf.table.format(matched, inf.interner, inf.program);
    let a2 = inf.table.format(the_required, inf.interner, inf.program);
    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::PATTERN_TYPE_MISMATCH_IN_IRREFUTABLE_CONTEXT, sp, &[&a1, &a2]);
}

/// As variáveis que o padrão declara, na ordem do `VariableBinder` (um `||`
/// junta as dos dois lados, a esquerda primeiro).
pub(crate) fn declared_variables(inf: &BodyInferrer<'_>, cx: &Body, p: PatternId, output: &mut Vec<ast::Name>) {
    let a = &inf.program.unit(cx.unit).ast;
    let by = |n: ast::Name, output: &mut Vec<ast::Name>| {
        if !output.iter().any(|x| x.sym == n.sym) {
            output.push(n);
        }
    };
    match &a.pattern(p).kind {
        PatternKind::Variable { final_, var_, ty, name } => {
            if !(cx.padrao_refutavel && !*var_ && !*final_ && ty.is_none()) {
                by(*name, output);
            }
        }
        PatternKind::Wildcard { .. } | PatternKind::Constant(_) | PatternKind::Relational { .. } => {}
        PatternKind::Or(x, y) | PatternKind::And(x, y) => {
            let mut l = Vec::new();
            declared_variables(inf, cx, *x, &mut l);
            declared_variables(inf, cx, *y, &mut l);
            for n in l {
                by(n, output);
            }
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => declared_variables(inf, cx, *x, output),
        PatternKind::Cast { pattern, .. } => declared_variables(inf, cx, *pattern, output),
        PatternKind::List { elements, .. } => {
            for e in elements.iter() {
                match e {
                    ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) => declared_variables(inf, cx, *x, output),
                    ListPatternElement::Rest(None) => {}
                }
            }
        }
        PatternKind::Map { entries, .. } => {
            for en in entries.iter() {
                declared_variables(inf, cx, en.value, output);
            }
        }
        PatternKind::Record { fields } => {
            for f in fields.iter() {
                declared_variables(inf, cx, f.pattern, output);
            }
        }
        PatternKind::Object { fields, .. } => {
            for f in fields.iter() {
                declared_variables(inf, cx, f.pattern, output);
            }
        }
    }
}

/// `logicalOrPatternBranchMissingVariable` (`resolution_visitor.dart:1992-2004`,
/// `VariableBinder.logicalOrPatternFinish`): cada variável só da esquerda
/// relata no operando direito; cada uma só da direita, no esquerdo.
fn variables_missing_in_or(inf: &mut BodyInferrer<'_>, cx: &Body, x: PatternId, y: PatternId) {
    let mut left = Vec::new();
    declared_variables(inf, cx, x, &mut left);
    let mut right = Vec::new();
    declared_variables(inf, cx, y, &mut right);
    let (sx, sy) = {
        let a = &inf.program.unit(cx.unit).ast;
        (a.pattern(x).span, a.pattern(y).span)
    };
    let code = dartforge_diagnostics::codigos::compile_time_error::MISSING_VARIABLE_PATTERN;
    for n in &left {
        if !right.iter().any(|d| d.sym == n.sym) {
            let the_name = inf.interner.resolve(n.sym).to_string();
            inf.warning_with_code(code, sy, &[&the_name]);
        }
    }
    for n in &right {
        if !left.iter().any(|e| e.sym == n.sym) {
            let the_name = inf.interner.resolve(n.sym).to_string();
            inf.warning_with_code(code, sx, &[&the_name]);
        }
    }
}

/// `duplicateRestPattern` (`shared_type_analyzer.dart:83-96`): cada `...`
/// depois do primeiro numa lista, no elemento (o `...` e o subpadrão).
fn duplicate_rests(inf: &mut BodyInferrer<'_>, cx: &Body, p: PatternId) {
    let a = &inf.program.unit(cx.unit).ast;
    let PatternKind::List { type_args, elements } = &a.pattern(p).kind else { return };
    let source = inf.program.unit(cx.unit).source.as_str();
    let sp = a.pattern(p).span;
    let after_types = type_args.last().map_or(sp.start, |x| a.ty(*x).span.end);
    let Some(open) = source.get(after_types..sp.end).and_then(|s| s.find('[')).map(|i| after_types + i) else { return };
    let mut cursor = open + 1;
    let mut rests: Vec<Span> = Vec::new();
    for (i, e) in elements.iter().enumerate() {
        match e {
            ListPatternElement::Pattern(x) => cursor = a.pattern(*x).span.end,
            ListPatternElement::Rest(sub) => {
                let bound = match sub {
                    Some(x) => a.pattern(*x).span.start,
                    None => elements.get(i + 1).map_or(sp.end, |next| match next {
                        ListPatternElement::Pattern(y) | ListPatternElement::Rest(Some(y)) => a.pattern(*y).span.start,
                        ListPatternElement::Rest(None) => sp.end,
                    }),
                };
                let Some(start) = source.get(cursor..bound.max(cursor)).and_then(|s| s.find("...")).map(|k| cursor + k) else { continue };
                let end = sub.map_or(start + 3, |x| a.pattern(x).span.end);
                rests.push(Span { start: start, end: end });
                cursor = end;
            }
        }
    }
    // `duplicateRestElementInPattern`: o contexto é o primeiro.
    let Some(&first) = rests.first() else { return };
    for s in rests.into_iter().skip(1) {
        inf.warning_with_context(dartforge_diagnostics::codigos::compile_time_error::DUPLICATE_REST_ELEMENT_IN_PATTERN, s, &[], vec![(None, first, "The first rest element.".to_string())]);
    }
}

pub(crate) fn type_check(inf: &mut BodyInferrer<'_>, cx: &mut Body, p: PatternId, final_: bool, assignment: bool, of_e: bool) {
    use dartforge_diagnostics::codigos::{static_warning as sw, warning as w};
    let a = &inf.program.unit(cx.unit).ast;
    let Some(r) = topo(cx) else { return };
    // O tipo casado é o da ENTRADA do padrão, antes de ele mesmo promover.
    let t = matched(cx, &r);
    inf.body_types.units[cx.unit.0 as usize].tipos_casados.insert(p, t);
    let pattern_span = a.pattern(p).span;
    match &a.pattern(p).kind {
        PatternKind::Wildcard { ty: Some(x) } => {
            let x = *x;
            let k = inf.type_from_annotation(cx, x);
            let inv = invalid_annotation(inf, cx, x, k);
            record_pattern_type(inf, cx, p, k, inv);
            let sp = inf.program.unit(cx.unit).ast.ty(x).span;
            never_matches(inf, cx, t, k, sp);
            if irrefutable_now(cx) && !inf.atribuivel(t, k) {
                wrong_type_in_irrefutable(inf, t, k, pattern_span);
            }
            let covers = promote_for_pattern(inf, cx, k, true, false);
            if covers && of_e {
                inf.warning_with_code(w::UNNECESSARY_WILDCARD_PATTERN, pattern_span, &[]);
            }
        }
        PatternKind::Wildcard { ty: None } => {
            if of_e {
                inf.warning_with_code(w::UNNECESSARY_WILDCARD_PATTERN, pattern_span, &[]);
            }
        }
        PatternKind::Variable { final_: f2, var_, ty, name } => {
            let (f2, ty, name) = (*f2, *ty, *name);
            if cx.padrao_refutavel && !*var_ && !f2 && ty.is_none() {
                // `case limite:` é o padrão constante de uma `const`: pode
                // casar ou não, sem informação. O nome é um
                // `SimpleIdentifier` (`ConstantPattern`): sem elemento,
                // `UNDEFINED_IDENTIFIER`.
                // O valor do nome como constante (`_getConstantValue`,
                // 3.6.2 `evaluation.dart:1790-1870`): sem `const`, o erro
                // genérico, que o `ConstantVerifier` relata no nome.
                let referenced = expr::resolve_name(inf, cx, name.sym, false);
                let resolved = match &referenced {
                    expr::NameRef::Elemento(el) => Some(crate::resolved::Resolved::Element(*el)),
                    expr::NameRef::LexicalMember(f, static_) => Some(expr::lexical_member_resolved(inf, cx, *f, *static_)),
                    expr::NameRef::ConstantEnum(v) => inf.program.variable(*v).class.map(|c| crate::resolved::Resolved::Member {
                        class: c,
                        member: crate::resolved::MemberRef::Variable(*v),
                        via_super: false,
                    }),
                    _ => None,
                };
                if let Some(r) = resolved {
                    inf.body_types.units[cx.unit.0 as usize].pattern_names.insert(p, r);
                }
                let non_constant = match referenced {
                    expr::NameRef::Local(id) => !cx.local(id).const_,
                    expr::NameRef::Elemento(dartforge_elements::model::Element::Variable(v)) => !inf.program.variable(v).const_,
                    expr::NameRef::Elemento(dartforge_elements::model::Element::Function(f)) => {
                        !matches!(inf.program.function(f).kind, dartforge_elements::model::FunctionKind::Function)
                    }
                    expr::NameRef::LexicalMember(f, static_) => {
                        let fe = inf.program.function(f);
                        match fe.kind {
                            dartforge_elements::model::FunctionKind::Function => !static_,
                            dartforge_elements::model::FunctionKind::ImplicitAccessor => {
                                !static_ || !fe.variable.is_some_and(|v| inf.program.variable(v).const_)
                            }
                            _ => true,
                        }
                    }
                    expr::NameRef::ImplicitThis => true,
                    _ => false,
                };
                if non_constant {
                    inf.body_types.units[cx.unit.0 as usize].non_constant_name_patterns.insert(p);
                }
                match expr::resolve_name(inf, cx, name.sym, false) {
                    expr::NameRef::Nenhum => expr::undefined_name_without_this(inf, cx, name),
                    // O parâmetro de tipo como literal de tipo constante: fora
                    // de um construtor `const` não há ambiente léxico, e o
                    // `_getConstantValue` dá `CONST_TYPE_PARAMETER` no nome
                    // (3.6.2 `evaluation.dart:1847-1859`).
                    expr::NameRef::TypeParam(_) => {
                        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::CONST_TYPE_PARAMETER, name.span, &[]);
                    }
                    _ => {}
                }
                let current = cx.flow.clone();
                join_not_matched(inf, cx, &current);
                return;
            }
            // Com `var`, `final` ou tipo, mesmo numa atribuição o padrão é
            // `DeclaredVariablePattern` (o parser só relata
            // `PATTERN_ASSIGNMENT_DECLARES_VARIABLE`), e o
            // `visitDeclaredVariablePattern` declara o local
            // (`resolution_visitor.dart:408-433`).
            if assignment && !*var_ && !f2 && ty.is_none() {
                // `visitAssignedVariablePattern` (`resolution_visitor.dart:189-210`):
                // o nome pelo escopo léxico; sem elemento, `UNDEFINED_IDENTIFIER`;
                // elemento que não é local nem parâmetro,
                // `PATTERN_ASSIGNMENT_NOT_LOCAL_VARIABLE`.
                match expr::resolve_name(inf, cx, name.sym, false) {
                    expr::NameRef::Local(id) => expr::write_in_primary(inf, cx, id, name.span),
                    expr::NameRef::ImplicitThis | expr::NameRef::Forward(_) | expr::NameRef::Nenhum => {
                        expr::undefined_name_without_this(inf, cx, name);
                    }
                    _ => {
                        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::PATTERN_ASSIGNMENT_NOT_LOCAL_VARIABLE, name.span, &[]);
                    }
                }
                if let Some(Nome::Local(id)) = cx.buscar(name.sym) {
                    let decl = cx.local(id).tipo;
                    if irrefutable_now(cx) && !inf.e_dynamic(t) && !inf.sub(t, decl) {
                        wrong_type_in_irrefutable(inf, t, decl, pattern_span);
                    }
                    promote_for_pattern(inf, cx, decl, true, false);
                    let written = matched(cx, &r);
                    let mut f = std::mem::replace(&mut cx.flow, Fluxo::alcancavel());
                    let reason = crate::inference::flow::NonPromotionReason::Escrita { nome: name.sym, span: pattern_span };
                    inf.assign_flow(&mut f, id, decl, written, Some(reason));
                    cx.flow = f;
                }
                return;
            }
            let k = match ty {
                Some(x) => {
                    let k = inf.type_from_annotation(cx, x);
                    never_matches(inf, cx, t, k, inf.program.unit(cx.unit).ast.ty(x).span);
                    if irrefutable_now(cx) && !inf.e_dynamic(t) && !inf.sub(t, k) {
                        wrong_type_in_irrefutable(inf, t, k, pattern_span);
                    }
                    k
                }
                // `variableTypeFromInitializerType`: um valor `Null`
                // (inclusive `Never?`) declara a variável como `dynamic`.
                None => {
                    if classificar(inf, t) == Classe::Nulo {
                        inf.core.dynamic_
                    } else {
                        t
                    }
                }
            };
            promote_for_pattern(inf, cx, k, true, false);
            let inv = ty.is_some_and(|x| invalid_annotation(inf, cx, x, k));
            record_pattern_type(inf, cx, p, k, inv);
            let id = declarar_local(
                inf,
                cx,
                Local { nome: name.sym, tipo: k, final_: final_ || f2, late: false, const_: false, offset: name.span.start, funcao_local: false },
                true,
            );
            // `declaredVariablePattern` + `assignMatchedPatternVariable`
            // (3.6.2 `flow_analysis.dart:4528-4544`, `_initialize` `:5931-5962`):
            // a variável começa com a escrita do tipo casado, que promove pelo
            // tipo de interesse (o não anulável do declarado) se ela tem tipo
            // escrito e não é `final` (`case int? x?:` deixa `x` como `int`).
            // O tipo escrito é o casado **depois** do `promoteForPattern`
            // (`type_analyzer.dart:515-530`, `promotedValueType`): com o
            // escrutínio `Object?`, `case String? a?:` casa `Object` e a
            // promoção ao `String?` declarado dá `String`.
            let promoted = matched(cx, &r);
            if ty.is_some() && !(final_ || f2) && !inf.e_dynamic(promoted) && inf.sub(promoted, k) {
                let mut f = std::mem::replace(&mut cx.flow, Fluxo::alcancavel());
                inf.write_flow(&mut f, id, k, promoted, true, None);
                cx.flow = f;
            }
        }
        PatternKind::Constant(e) => {
            let e = *e;
            // `visitConstantPattern` (`best_practices_verifier.dart:257-265`).
            if expr::e_double_nan(inf, cx, e) {
                inf.warning_with_code(w::UNNECESSARY_NAN_COMPARISON_FALSE, pattern_span, &[]);
            }
            let forced = refutable_in_irrefutable(inf, cx, pattern_span);
            let c = inferir(inf, cx, e, t);
            if forced {
                cx.forced_refutable = false;
            }
            // `caseExpressionTypeMismatch` (`shared_type_analyzer.dart:32-45`):
            // sem padrões (< 3.0), a constante de um `case` cujo tipo não é
            // subtipo do tipo do escrutínio.
            if !language_patterns(inf, cx) && cx.padrao_refutavel && !inf.e_dynamic(c) && !inf.sub(c, t) {
                let sp = inf.span_expr(cx.unit, e);
                let a1 = inf.table.format(c, inf.interner, inf.program);
                let a2 = inf.table.format(t, inf.interner, inf.program);
                inf.warning_with_code(
                    dartforge_diagnostics::codigos::compile_time_error::CASE_EXPRESSION_TYPE_IS_NOT_SWITCH_EXPRESSION_SUBTYPE,
                    sp,
                    &[&a1, &a2],
                );
            }
            if language_patterns(inf, cx) {
                equality(inf, cx, e, c, false);
            } else {
                // Sem padrões (< 3.0): todo `case` é alcançável e nada é
                // promovido (`constantPattern_end`, `:4489-4505`).
                let current = cx.flow.clone();
                join_not_matched(inf, cx, &current);
            }
        }
        PatternKind::Relational { op, value } => {
            let (op, value) = (*op, *value);
            if refutable_in_irrefutable(inf, cx, pattern_span) {
                cx.forced_refutable = false;
            }
            // `analyzeRelationalPattern`: operador, contexto do operando e as
            // conferências (operando contra o parâmetro, retorno contra `bool`).
            let tv = super::relational_pattern(inf, cx, p, op, value, t);
            match op {
                ast::BinaryOp::Eq => equality(inf, cx, value, tv, false),
                ast::BinaryOp::NotEq => equality(inf, cx, value, tv, true),
                _ => {
                    let current = cx.flow.clone();
                    join_not_matched(inf, cx, &current);
                }
            }
        }
        PatternKind::Or(x, y) => {
            let (x, y) = (*x, *y);
            if !assignment {
                variables_missing_in_or(inf, cx, x, y);
            }
            let forced = refutable_in_irrefutable(inf, cx, pattern_span);
            // `logicalOrPattern_begin`: o operando esquerdo tem o seu
            // próprio "não casou", que é a entrada do direito.
            let empty = cx.flow.unreachable();
            let previous = cx.matching.as_mut().map(|c| std::mem::replace(&mut c.not_matched, empty));
            let start_x = cx.locais.len();
            type_check(inf, cx, x, final_, assignment, false);
            let start_y = cx.locais.len();
            let mut matched_left = cx.flow.clone();
            let failed = match (cx.matching.as_mut(), previous) {
                (Some(c), Some(previous)) => std::mem::replace(&mut c.not_matched, previous),
                _ => cx.flow.clone(),
            };
            cx.flow = failed;
            // O operando direito de um `||` cujo esquerdo sempre casa é
            // código morto (Apêndice B do §B).
            // O trecho começa no operador `||` e vai ao fim do operando
            // (`dead_code_verifier.dart:321-324`).
            let (end_x, sp_y) = (inf.program.unit(cx.unit).ast.pattern(x).span.end, inf.program.unit(cx.unit).ast.pattern(y).span);
            super::super::statements::enter_flow(cx, sp_y.end);
            if !cx.flow.alcancavel && cx.dead_code.is_none() {
                let source = inf.program.unit(cx.unit).source.as_bytes();
                let operator = source
                    .get(end_x..sp_y.start)
                    .and_then(|snippet| snippet.windows(2).position(|w| w == b"||"))
                    .map_or(sp_y.start, |i| end_x + i);
                inf.aviso(crate::codes::DEAD_CODE.template.to_string(), Span { start: operator, end: sp_y.end });
                cx.dead_code = Some(cx.flow_ends.len());
            }
            type_check(inf, cx, y, final_, assignment, false);
            super::super::statements::exit_flow(cx);
            let matched_right = cx.flow.clone();
            // A variável de junção (a do operando direito, que o escopo
            // enxerga) tem, no ramo do esquerdo, o estado da cópia do esquerdo
            // (`logicalOrPattern_end` junta as variáveis pelo nome): com o
            // direito morto, fica a do esquerdo, atribuída.
            for d in start_y..cx.locais.len() {
                let the_name = cx.locais[d].nome;
                if let Some(e) = (start_x..start_y).find(|&e| cx.locais[e].nome == the_name && cx.locais[e].offset != 0)
                    && let Some(m) = matched_left.modelo(LocalId(e as u32)).cloned()
                {
                    if matched_left.vars.len() <= d {
                        matched_left.vars.resize(d + 1, None);
                    }
                    matched_left.vars[d] = Some(m);
                }
            }
            cx.flow = inf.juntar(&matched_left, &matched_right);
            if forced {
                cx.forced_refutable = false;
            }
        }
        PatternKind::And(x, y) => {
            let (x, y) = (*x, *y);
            type_check(inf, cx, x, final_, assignment, true);
            type_check(inf, cx, y, final_, assignment, true);
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) => {
            let x = *x;
            let assertion = matches!(a.pattern(p).kind, PatternKind::NullAssert(_));
            let forced = !assertion && refutable_in_irrefutable(inf, cx, pattern_span);
            match check_null(inf, cx) {
                Some(non_null) => {
                    if !assertion {
                        let current = cx.flow.clone();
                        join_not_matched(inf, cx, &current);
                    }
                    cx.flow = non_null;
                }
                None => {
                    // O valor casado não pode ser nulo: o `?`/`!` não faz
                    // nada. O `?` num contexto irrefutável já é outro erro.
                    if assertion || (!forced && !irrefutable_now(cx)) {
                        let token = Span { start: pattern_span.end.saturating_sub(1), end: pattern_span.end };
                        let code = if assertion { sw::UNNECESSARY_NULL_ASSERT_PATTERN } else { sw::UNNECESSARY_NULL_CHECK_PATTERN };
                        inf.warning_with_code(code, token, &[]);
                    }
                }
            }
            type_check(inf, cx, x, final_, assignment, false);
            if forced {
                cx.forced_refutable = false;
            }
        }
        PatternKind::Parenthesized(x) => {
            // O `analyzeParenthesizedPattern` repassa o contexto, com o
            // `unnecessaryWildcardKind` (`(_) && p`).
            let x = *x;
            type_check(inf, cx, x, final_, assignment, of_e);
        }
        PatternKind::Cast { pattern, ty } => {
            let (pattern, ty) = (*pattern, *ty);
            let c = inf.type_from_annotation(cx, ty);
            let inv = invalid_annotation(inf, cx, ty, c);
            record_pattern_type(inf, cx, p, c, inv);
            let sp = inf.program.unit(cx.unit).ast.ty(ty).span;
            never_matches(inf, cx, t, c, sp);
            if !inv && !inf.table.e_invalido(c) && inf.sub(t, c) {
                if let Some(keyword) = as_keyword(inf, cx, pattern, ty) {
                    inf.warning_with_code(w::UNNECESSARY_CAST_PATTERN, keyword, &[]);
                }
            }
            if inf.is_non_nullable(c) && classificar(inf, t) == Classe::Nulo {
                inf.warning_with_code(w::CAST_FROM_NULL_ALWAYS_FAILS, pattern_span, &[]);
            }
            // O cast não falha por tipo errado (lança): nada vai ao "não casou".
            promote_for_pattern(inf, cx, c, false, false);
            with_subpattern(inf, cx, c, |inf, cx| type_check(inf, cx, pattern, final_, assignment, false));
        }
        PatternKind::List { type_args, elements } => {
            let el = if let Some(x) = type_args.first() {
                inf.type_argument_type(cx, *x)
            } else if inf.e_dynamic(t) {
                // `dynamic` e o inválido dão elementos do mesmo tipo.
                t
            } else {
                inf.as_instance_of(t, inf.core.list_class).map(|a| a[0]).unwrap_or(inf.core.object_nullable)
            };
            let the_required = inf.lista(el);
            {
                let inv = type_args.first().is_some_and(|&x| invalid_annotation(inf, cx, x, el));
                record_pattern_type(inf, cx, p, the_required, inv);
                never_matches(inf, cx, t, the_required, pattern_span);
            }
            // Só `[...]` (um elemento, e ele é o resto) casa com toda lista.
            let only_rest = elements.len() == 1 && matches!(elements[0], ListPatternElement::Rest(_));
            promote_for_pattern(inf, cx, the_required, true, !only_rest);
            for e in elements.iter() {
                match e {
                    ListPatternElement::Pattern(x) => {
                        let x = *x;
                        with_subpattern(inf, cx, el, |inf, cx| type_check(inf, cx, x, final_, assignment, false));
                    }
                    ListPatternElement::Rest(Some(x)) => {
                        let x = *x;
                        with_subpattern(inf, cx, the_required, |inf, cx| type_check(inf, cx, x, final_, assignment, false));
                    }
                    ListPatternElement::Rest(None) => {}
                }
            }
            duplicate_rests(inf, cx, p);
            if irrefutable_now(cx) && !inf.atribuivel(t, the_required) {
                wrong_type_in_irrefutable(inf, t, the_required, pattern_span);
            }
        }
        PatternKind::Map { type_args, entries, .. } => {
            let (k, v) = if type_args.len() == 2 {
                (inf.type_argument_type(cx, type_args[0]), inf.type_argument_type(cx, type_args[1]))
            } else if inf.e_dynamic(t) {
                (t, t)
            } else {
                match inf.as_instance_of(t, inf.core.map_class) {
                    Some(a) => (a[0], a[1]),
                    None => (inf.core.object_nullable, inf.core.object_nullable),
                }
            };
            let mut map_required = None;
            match inf.core.map_class {
                Some(mc) => {
                    let the_required = inf.table.intern(Type::Interface { class: mc, args: vec![k, v].into_boxed_slice(), nullable: false });
                    let inv = type_args.len() == 2 && (invalid_annotation(inf, cx, type_args[0], k) || invalid_annotation(inf, cx, type_args[1], v));
                    record_pattern_type(inf, cx, p, the_required, inv);
                    // `visitMapPattern` (3.6.2 `resolver.dart:1549-1554`): no
                    // padrão inteiro.
                    never_matches(inf, cx, t, the_required, pattern_span);
                    // Um mapa pode não ter a chave: falha mesmo com o tipo certo.
                    promote_for_pattern(inf, cx, the_required, true, true);
                    map_required = Some(the_required);
                }
                None => {
                    let current = cx.flow.clone();
                    join_not_matched(inf, cx, &current);
                }
            }
            for en in entries.iter() {
                let (key, val) = (en.key, en.value);
                inferir(inf, cx, key, k);
                with_subpattern(inf, cx, v, |inf, cx| type_check(inf, cx, val, final_, assignment, false));
            }
            if let Some(the_required) = map_required
                && irrefutable_now(cx)
                && !inf.atribuivel(t, the_required)
            {
                wrong_type_in_irrefutable(inf, t, the_required, pattern_span);
            }
        }
        PatternKind::Record { fields } => {
            let the_fields: Vec<(Option<ast::Name>, PatternId)> = fields.iter().map(|f| (f.name, f.pattern)).collect();
            // A forma do registro: `(Object?, …, {Object? n})`.
            let names: Vec<Option<dartforge_intern::SymbolId>> = the_fields
                .iter()
                .map(|&(n, x)| n.map(|n| n.sym).or_else(|| if implicit_named_field(inf, cx, p, x) { nome_implicito(inf, cx, x) } else { None }))
                .collect();
            let fields_top = inf.core.object_nullable;
            let assemble = |inf: &mut BodyInferrer<'_>, types: &[TypeId]| -> TypeId {
                let mut pos = Vec::new();
                let mut nm = Vec::new();
                for (the_name, &typ) in names.iter().zip(types.iter()) {
                    match the_name {
                        Some(s) => nm.push((*s, typ)),
                        None => pos.push(typ),
                    }
                }
                inf.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
            };
            let the_required = assemble(inf, &vec![fields_top; the_fields.len()]);
            // `RecordPatternImpl.resolvePattern` (3.6.2 `ast.dart:14858-14865`):
            // sem campo nomeado repetido, no padrão inteiro.
            let repeated = {
                let mut seen = std::collections::HashSet::new();
                names.iter().flatten().any(|n| !seen.insert(*n))
            };
            if !repeated {
                never_matches(inf, cx, t, the_required, pattern_span);
            }
            promote_for_pattern(inf, cx, the_required, true, false);
            // Os tipos dos campos vêm do tipo casado na ENTRADA do padrão
            // (`matchedValueType`, 3.6.2 `type_analyzer.dart:1608-1625`), não
            // do promovido para `(Object?, …)`.
            let promoted = t;
            let rec = match inf.table.get(promoted).clone() {
                Type::Record { positional, named, .. } => Some((positional, named)),
                _ => None,
            };
            let mut ipos = 0;
            let mut the_shown: Vec<TypeId> = Vec::with_capacity(the_fields.len());
            for (i, &(_, x)) in the_fields.iter().enumerate() {
                let ft = match (&rec, names[i]) {
                    (Some((_, named)), Some(nm)) => named.iter().find(|(s, _)| *s == nm).map(|(_, t)| *t),
                    (Some((pos, _)), None) => {
                        let finding = pos.get(ipos).copied();
                        ipos += 1;
                        finding
                    }
                    _ => None,
                };
                let ft = ft.unwrap_or(if inf.e_dynamic(t) { t } else { fields_top });
                let d = with_subpattern(inf, cx, ft, |inf, cx| type_check(inf, cx, x, final_, assignment, false));
                the_shown.push(d);
            }
            // A segunda promoção, para o registro dos tipos demonstrados
            // pelos subpadrões; não falha por tipo.
            let shown = assemble(inf, &the_shown);
            promote_for_pattern(inf, cx, shown, false, false);
            if irrefutable_now(cx) && !inf.atribuivel(t, the_required) {
                wrong_type_in_irrefutable(inf, t, the_required, pattern_span);
            }
        }
        PatternKind::Object { ty, fields } => {
            let ty = *ty;
            let the_fields: Vec<(Option<ast::Name>, PatternId)> = fields.iter().map(|f| (f.name, f.pattern)).collect();
            let obj = object_pattern_type(inf, cx, ty, t);
            let inv = invalid_annotation(inf, cx, ty, obj);
            record_pattern_type(inf, cx, p, obj, inv);
            {
                let sp = inf.program.unit(cx.unit).ast.ty(ty).span;
                never_matches(inf, cx, t, obj, sp);
            }
            promote_for_pattern(inf, cx, obj, true, false);
            for (n, x) in the_fields {
                let the_name = n.map(|n| n.sym).or_else(|| nome_implicito(inf, cx, x));
                // `resolveObjectPatternPropertyGet` (3.6.2 `resolver.dart:1562-1580`):
                // o `TypePropertyResolver` com `propertyErrorEntity` no tipo do
                // padrão; com o tipo potencialmente anulável, o uso sem
                // checagem no tipo e a busca no tipo sem `?`.
                let obj_lookup = match the_name {
                    Some(nm) if inf.requires_null_check(cx.lib, obj, nm, false) => {
                        let sp = inf.program.unit(cx.unit).ast.ty(ty).span;
                        let text = inf.interner.resolve(nm).to_string();
                        inf.null_warning(
                            obj,
                            dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
                            sp,
                            &[&text],
                        );
                        inf.non_null(obj)
                    }
                    _ => obj,
                };
                let ft = match the_name {
                    Some(nm) => match inf.lookup_member(cx.lib, obj_lookup, nm, false) {
                        Busca::Achado(m) => {
                            let of_extension_type = matches!(
                                &m.resolved,
                                crate::resolved::Resolved::Member { class, .. }
                                    if inf.program.class(*class).kind == dartforge_elements::model::ClassKind::ExtensionType
                            );
                            if m.of_extension || of_extension_type {
                                inf.body_types.units[cx.unit.0 as usize].extension_fields.insert(x, m.tipo);
                            }
                            inf.body_types.units[cx.unit.0 as usize].field_elements.insert(x, m.resolved.clone());
                            m.tipo
                        }
                        Busca::NeverType => inf.core.never,
                        _ => inf.core.dynamic_,
                    },
                    None => inf.core.dynamic_,
                };
                // Getter `Never`: a leitura não volta (`flow.handleExit()`).
                if matches!(inf.table.get(ft), Type::Never) {
                    cx.flow.alcancavel = false;
                }
                with_subpattern(inf, cx, ft, |inf, cx| type_check(inf, cx, x, final_, assignment, false));
            }
            if irrefutable_now(cx) && !inf.atribuivel(t, obj) {
                wrong_type_in_irrefutable(inf, t, obj, pattern_span);
            }
        }
    }
}

/// `case p when g` com o fluxo de padrões: `(casou, não casou)`.
pub(crate) fn caso(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    p: PatternId,
    t: TypeId,
    guard: Option<ExprId>,
    scrutinee: Option<ExprId>,
) -> (Fluxo, Fluxo) {
    // O escrutínio é resolvido no escopo de fora, antes de o padrão
    // declarar as suas variáveis.
    let target = scrutinee.and_then(|e| expr::promotion_target(inf, cx, e));
    let property = target.is_some_and(|id| cx.is_property(id));
    // Num `switch`, todos os casos casam a MESMA referência: as promoções
    // do "não casou" de um caso chegam ao seguinte.
    let reuse = match (cx.switch_scrutinee, scrutinee) {
        (Some(Some((e0, the_key, version))), Some(e)) if e0 == e && cx.flow.modelo(the_key).is_some() => Some((the_key, version)),
        _ => None,
    };
    let (the_key, version) = match reuse {
        Some(x) => x,
        None => {
            let the_key = new_key(inf, cx, t);
            let version = target.and_then(|id| cx.flow.versao(id));
            if cx.switch_scrutinee.is_some() {
                if let Some(e) = scrutinee {
                    cx.switch_scrutinee = Some(Some((e, the_key, version)));
                }
            }
            (the_key, version)
        }
    };
    let r = MatchedRef { chave: the_key, tipo: t, alvo: target, propriedade: property, versao: version };
    let refutable_before = std::mem::replace(&mut cx.padrao_refutavel, true);
    let forced_before = std::mem::replace(&mut cx.forced_refutable, false);
    let first_local = cx.locais.len();
    // `visitGuardedPattern`: as variáveis do padrão ficam escondidas durante
    // o padrão inteiro (não na guarda).
    let mut hidden = Vec::new();
    declared_variables(inf, cx, p, &mut hidden);
    let base_hidden = cx.pattern_hidden.len();
    cx.pattern_hidden.extend(hidden.iter().map(|n| (n.sym, n.span)));
    let mut not_ = casar(inf, cx, r, |inf, cx| type_check(inf, cx, p, false, false, false));
    cx.pattern_hidden.truncate(base_hidden);
    cx.padrao_refutavel = refutable_before;
    cx.forced_refutable = forced_before;
    cx.last_pattern_locals = first_local..cx.locais.len();
    if let Some(g) = guard {
        // `visitGuardedPattern`: as variáveis do padrão, durante a guarda.
        let base = cx.variables_in_guard.len();
        cx.variables_in_guard.extend((first_local..cx.locais.len()).map(|i| LocalId(i as u32)));
        let (gv, gf) = expr::checked_condition(inf, cx, g);
        cx.variables_in_guard.truncate(base);
        cx.flow = gv;
        not_ = inf.juntar(&not_, &gf);
    }
    (cx.flow.clone(), not_)
}

/// Declaração, atribuição e `for-in` de padrão: o mesmo casamento, sem
/// promover variável escrutinada (`allowScrutineePromotion: false`) e com o
/// "não casou" descartado. `valor`: o inicializador, quando há — uma
/// propriedade promovível é promovida em qualquer construção.
pub(crate) fn irrefutable(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    p: PatternId,
    t: TypeId,
    final_: bool,
    assignment: bool,
    the_value: Option<ExprId>,
) {
    let target = the_value
        .and_then(|e| expr::promotion_target(inf, cx, e))
        .filter(|&id| cx.is_property(id));
    let the_key = new_key(inf, cx, t);
    let r = MatchedRef { chave: the_key, tipo: t, alvo: target, propriedade: target.is_some(), versao: None };
    let _ = casar(inf, cx, r, |inf, cx| type_check(inf, cx, p, final_, assignment, false));
}
