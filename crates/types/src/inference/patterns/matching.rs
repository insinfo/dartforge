//! The state of pattern matching in the flow (docs/ANALYZER-ESPECIFICACAO.md
//! §G, T6): the port of `_pushScrutinee`/`_pushPattern`/`_popPattern`,
//! `promoteForPattern`, `_nullCheckPattern`, `_handleEqualityCheckPattern` and
//! the `logicalOrPattern_*` of `_FlowAnalysisImpl`
//! (`_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart`), with the
//! `TypeAnalyzer` callers (`type_inference/type_analyzer.dart`).
//!
//! The matched value is a **temporary reference** with its own promotion
//! key: here, a synthetic local of the body, whose model lives in the
//! current [`Fluxo`] like that of any variable. The matched type of a
//! pattern is the promoted type of that key on entry to the pattern;
//! promoting the key also promotes the scrutinee, when it is a reference
//! that the construct allows to promote. The "did not match" state is
//! accumulated separately ([`Matching::not_matched`]) and starts unreachable:
//! it only becomes reachable when some pattern can fail.
//!
//! Only the analyzer uses this path (see [`ligado`]); the compilers keep the
//! old typing of `super::type_check`.

use super::super::body::{Body, Local, MemberName};
use super::super::expr::{self, declare_local, infer};
use super::super::flow::Flow;
use super::super::members::Search;
use super::super::BodyInferrer;
use super::{
    invalid_annotation, implicit_named_field, implicit_name, never_matches, record_pattern_type,
    object_pattern_type,
};
use crate::resolved::LocalId;
use crate::table::{Type, TypeId};
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, ExprId, ListPatternElement, PatternId, PatternKind};

/// The reference to the matched value of a pattern (or subpattern).
#[derive(Debug, Clone)]
pub(crate) struct MatchedRef {
    /// The synthetic local that carries the promotions of the matched value.
    pub key: LocalId,
    /// The type with which the reference was created.
    pub ty: TypeId,
    /// The scrutinee, when it is a promotable reference.
    pub target: Option<LocalId>,
    /// The scrutinee is a promotable property (it promotes in any construct);
    /// otherwise it is a variable and only promotes while it is not rewritten.
    pub property: bool,
    /// The write version of the scrutinized variable at the start of the match.
    pub version: Option<u32>,
}

/// The match in progress of a top-level pattern.
#[derive(Debug, Clone)]
pub(crate) struct Matching {
    /// `_unmatched`.
    pub not_matched: Flow,
    /// The reference of the top-level pattern and those of the open subpatterns.
    pub stack: Vec<MatchedRef>,
}

/// The analyzer's pattern flow only applies in analyzer mode (the same one
/// that preserves the display of types): it changes the promoted type seen
/// by every body with a `switch` or `if-case`, and the JS and native emitters
/// consume the promotions (T6.7).
pub(crate) fn enabled(inf: &BodyInferrer<'_>) -> bool {
    inf.table.preserve_decoration
}

fn language_patterns(inf: &BodyInferrer<'_>, cx: &Body) -> bool {
    inf.program.library(cx.lib).features.version() >= dartforge_frontend::features::LanguageVersion::new(3, 0)
}

/// A synthetic local for the promotion key of a matched value.
pub(crate) fn new_key(inf: &BodyInferrer<'_>, cx: &mut Body, typ: TypeId) -> LocalId {
    let the_name = match inf.sym.empty.or(inf.sym.new_) {
        Some(s) => s,
        None => inf.program.classes[0].name,
    };
    cx.declare_synthetic(Local { name: the_name, ty: typ, final_: true, late: false, const_: false, offset: 0, local_function: false })
}

/// Matches against reference `r` what `f` types and returns the "did not
/// match" state.
pub(crate) fn match_(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    r: MatchedRef,
    f: impl FnOnce(&mut BodyInferrer<'_>, &mut Body),
) -> Flow {
    let not_matched = cx.flow.unreachable();
    let saved = cx.matching.replace(Matching { not_matched, stack: vec![r] });
    f(inf, cx);
    let end = std::mem::replace(&mut cx.matching, saved);
    match end {
        Some(c) => c.not_matched,
        None => cx.flow.unreachable(),
    }
}

fn top(cx: &Body) -> Option<MatchedRef> {
    cx.matching.as_ref().and_then(|c| c.stack.last().cloned())
}

/// `_getMatchedValueType`: the promoted type of the top key.
fn matched(cx: &Body, r: &MatchedRef) -> TypeId {
    cx.flow.current_type(r.key, r.ty)
}

/// `_unmatched = _join(_unmatched, extra)`.
fn join_not_matched(inf: &mut BodyInferrer<'_>, cx: &mut Body, extra: &Flow) {
    let Some(old) = cx.matching.as_ref().map(|c| c.not_matched.clone()) else { return };
    let new_ = inf.join(&old, extra);
    if let Some(c) = cx.matching.as_mut() {
        c.not_matched = new_;
    }
}

/// The scrutinee to promote along with the key: a property, always; a
/// variable, only if the construct allows it and it was not rewritten since
/// the start of the match (`promoteForPattern`, `:5227-5235`).
fn valid_target(cx: &Body, r: &MatchedRef) -> Option<LocalId> {
    let id = r.target?;
    if r.property || cx.flow.version(id) == r.version {
        Some(id)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    NonNull,
    Null,
    Maybe,
}

/// `classifyType` (`flow_analysis_visitor.dart:446-456`).
fn classify(inf: &mut BodyInferrer<'_>, t: TypeId) -> Class {
    let (object, null) = (inf.core.object, inf.core.null);
    if inf.sub(t, object) {
        Class::NonNull
    } else if inf.sub(t, null) {
        Class::Null
    } else {
        Class::Maybe
    }
}

/// The true branch of `tryPromoteForTypeCheck`. Promoting never makes the
/// flow unreachable, not even to `Never`.
fn promote_yes(inf: &mut BodyInferrer<'_>, f: &mut Flow, id: LocalId, declared: TypeId, k: TypeId) {
    let reachable = f.reachable;
    inf.promote_tested(f, id, declared, k, k);
    f.reachable = reachable;
}

/// The false branch: promotes to `factor(current, K)`, except when the factor
/// is `Never` or the current type itself (then only the type of interest
/// stays).
fn promote_not(inf: &mut BodyInferrer<'_>, f: &mut Flow, id: LocalId, declared: TypeId, k: TypeId) {
    let reachable = f.reachable;
    let s = f.current_type(id, declared);
    let fat = expr::factor(inf, s, k);
    let target = if fat == s || matches!(inf.table.get(fat), Type::Never) { s } else { fat };
    inf.promote_tested(f, id, declared, target, k);
    f.reachable = reachable;
}

/// `promoteForPattern` (`:5191-5258`): returns whether `K` covers the
/// matched type.
fn promote_for_pattern(inf: &mut BodyInferrer<'_>, cx: &mut Body, k: TypeId, failure: bool, can: bool) -> bool {
    // The invalid known type (`Unresolved()`, `:5196-5199`): the current state
    // goes to "did not match", without promotion, and does not cover.
    if inf.table.is_invalid(k) {
        let current = cx.flow.clone();
        join_not_matched(inf, cx, &current);
        return false;
    }
    let Some(r) = top(cx) else { return true };
    let t = matched(cx, &r);
    // A non-nullable matched type: the known one becomes its non-nullable
    // (`:5201-5207`), and `case int? x?` promotes to `int`.
    let k = if classify(inf, t) == Class::NonNull { inf.non_null_promotion(k) } else { k };
    let covers = inf.sub(t, k);
    let mut yes = cx.flow.clone();
    let mut not_ = cx.flow.clone();
    promote_yes(inf, &mut yes, r.key, r.ty, k);
    promote_not(inf, &mut not_, r.key, r.ty, k);
    if let Some(id) = valid_target(cx, &r) {
        let d = cx.local(id).ty;
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

/// `_nullCheckPattern` (`:6019-6032`): the "is not null" state, or `None`
/// when the matched type is already non-nullable (nothing to check).
fn check_null(inf: &mut BodyInferrer<'_>, cx: &mut Body) -> Option<Flow> {
    let r = top(cx)?;
    let t = matched(cx, &r);
    let cls = classify(inf, t);
    if cls == Class::NonNull {
        return None;
    }
    let nn = inf.non_null_promotion(t);
    let mut yes = cx.flow.clone();
    promote_yes(inf, &mut yes, r.key, r.ty, nn);
    if let Some(id) = valid_target(cx, &r) {
        let d = cx.local(id).ty;
        let current = yes.current_type(id, d);
        let nn_target = inf.non_null_promotion(current);
        promote_yes(inf, &mut yes, id, d, nn_target);
    }
    if cls == Class::Null {
        yes.reachable = false;
    }
    Some(yes)
}

/// `_handleEqualityCheckPattern` (`:5832-5903`): constant pattern (`==`) and
/// relational `==`/`!=` against `the_operand`, of type `operand_type`.
fn equality(inf: &mut BodyInferrer<'_>, cx: &mut Body, the_operand: ExprId, operand_type: TypeId, different: bool) {
    let Some(r) = top(cx) else { return };
    let t = matched(cx, &r);
    let a = &inf.program.unit(cx.unit).ast;
    let mut x = the_operand;
    while let ast::ExprKind::Parenthesized(i) = &a.expr(x).kind {
        x = *i;
    }
    let null_literal = matches!(a.expr(x).kind, ast::ExprKind::Null);
    let (ct, co) = (classify(inf, t), classify(inf, operand_type));
    let current = cx.flow.clone();
    if ct == Class::Null && co == Class::Null {
        // Certainly equal: `==` always matches, `!=` never.
        if different {
            join_not_matched(inf, cx, &current);
            cx.flow.reachable = false;
        }
    } else if (ct == Class::Null && co == Class::NonNull) || (co == Class::Null && ct == Class::NonNull) {
        // No information (in weak mode they could be equal).
        join_not_matched(inf, cx, &current);
    } else if null_literal {
        // Comparing with `null` is a null check of the matched value.
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

/// `pushSubpattern(typ)` … `popSubpattern`: `f` types the subpattern
/// against a new reference of type `typ`; returns the demonstrated type
/// (the reference's promoted one after the subpattern).
fn with_subpattern(inf: &mut BodyInferrer<'_>, cx: &mut Body, typ: TypeId, f: impl FnOnce(&mut BodyInferrer<'_>, &mut Body)) -> TypeId {
    let the_key = new_key(inf, cx, typ);
    if let Some(c) = cx.matching.as_mut() {
        c.stack.push(MatchedRef { key: the_key, ty: typ, target: None, property: false, version: None });
    }
    f(inf, cx);
    let shown = cx.flow.current_type(the_key, typ);
    if let Some(c) = cx.matching.as_mut() {
        c.stack.pop();
    }
    shown
}

/// The `as` keyword of a cast pattern, between the subpattern and the type.
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

/// Types pattern `p` against the matched value at the top of the stack,
/// declaring the variables. `of_e`: `p` is a direct operand of `&&` (the only
/// place where a wildcard that always matches is reported as unnecessary).
/// The pattern is in an irrefutable context (declaration, assignment,
/// `for-in`) that no outer refutable pattern made refutable.
fn irrefutable_now(cx: &Body) -> bool {
    !cx.refutable_pattern && !cx.forced_refutable
}

/// `refutablePatternInIrrefutableContext` (`shared_type_analyzer.dart:187-194`):
/// reports the refutable pattern and makes the rest refutable. Returns whether
/// it reported (the caller undoes `forced_refutable` at the end of the
/// pattern).
fn refutable_in_irrefutable(inf: &mut BodyInferrer<'_>, cx: &mut Body, sp: Span) -> bool {
    if !irrefutable_now(cx) {
        return false;
    }
    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::REFUTABLE_PATTERN_IN_IRREFUTABLE_CONTEXT, sp, &[]);
    cx.forced_refutable = true;
    true
}

/// `patternTypeMismatchInIrrefutableContext` (`shared_type_analyzer.dart:174-185`),
/// on the whole pattern, with the matched type and the required one.
fn wrong_type_in_irrefutable(inf: &mut BodyInferrer<'_>, matched: TypeId, the_required: TypeId, sp: Span) {
    let a1 = inf.table.format(matched, inf.interner, inf.program);
    let a2 = inf.table.format(the_required, inf.interner, inf.program);
    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::PATTERN_TYPE_MISMATCH_IN_IRREFUTABLE_CONTEXT, sp, &[&a1, &a2]);
}

/// The variables the pattern declares, in `VariableBinder` order (a `||`
/// joins those of both sides, the left first).
pub(crate) fn declared_variables(inf: &BodyInferrer<'_>, cx: &Body, p: PatternId, output: &mut Vec<ast::Name>) {
    let a = &inf.program.unit(cx.unit).ast;
    let by = |n: ast::Name, output: &mut Vec<ast::Name>| {
        if !output.iter().any(|x| x.sym == n.sym) {
            output.push(n);
        }
    };
    match &a.pattern(p).kind {
        PatternKind::Variable { final_, var_, ty, name } => {
            if !(cx.refutable_pattern && !*var_ && !*final_ && ty.is_none()) {
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
/// `VariableBinder.logicalOrPatternFinish`): each variable only on the left
/// reports at the right operand; each one only on the right, at the left.
fn variables_missing_in_or(inf: &mut BodyInferrer<'_>, cx: &Body, x: PatternId, y: PatternId) {
    let mut left = Vec::new();
    declared_variables(inf, cx, x, &mut left);
    let mut right = Vec::new();
    declared_variables(inf, cx, y, &mut right);
    let (sx, sy) = {
        let a = &inf.program.unit(cx.unit).ast;
        (a.pattern(x).span, a.pattern(y).span)
    };
    let code = dartforge_diagnostics::codes::compile_time_error::MISSING_VARIABLE_PATTERN;
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

/// `duplicateRestPattern` (`shared_type_analyzer.dart:83-96`): each `...`
/// after the first in a list, at the element (the `...` and the
/// subpattern).
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
    // `duplicateRestElementInPattern`: the context is the first one.
    let Some(&first) = rests.first() else { return };
    for s in rests.into_iter().skip(1) {
        inf.warning_with_context(dartforge_diagnostics::codes::compile_time_error::DUPLICATE_REST_ELEMENT_IN_PATTERN, s, &[], vec![(None, first, "The first rest element.".to_string())]);
    }
}

pub(crate) fn type_check(inf: &mut BodyInferrer<'_>, cx: &mut Body, p: PatternId, final_: bool, assignment: bool, of_e: bool) {
    use dartforge_diagnostics::codes::{static_warning as sw, warning as w};
    let a = &inf.program.unit(cx.unit).ast;
    let Some(r) = top(cx) else { return };
    // The matched type is that at the pattern's ENTRY, before it promotes
    // itself.
    let t = matched(cx, &r);
    inf.body_types.units[cx.unit.0 as usize].matched_types.insert(p, t);
    let pattern_span = a.pattern(p).span;
    match &a.pattern(p).kind {
        PatternKind::Wildcard { ty: Some(x) } => {
            let x = *x;
            let k = inf.type_from_annotation(cx, x);
            let inv = invalid_annotation(inf, cx, x, k);
            record_pattern_type(inf, cx, p, k, inv);
            let sp = inf.program.unit(cx.unit).ast.ty(x).span;
            never_matches(inf, cx, t, k, sp);
            if irrefutable_now(cx) && !inf.assignable(t, k) {
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
            if cx.refutable_pattern && !*var_ && !f2 && ty.is_none() {
                // `case bound:` is the constant pattern of a `const`: it may match or
                // not, without information. The name is a `SimpleIdentifier`
                // (`ConstantPattern`): without an element, `UNDEFINED_IDENTIFIER`.
                // The name's value as a constant (`_getConstantValue`, 3.6.2
                // `evaluation.dart:1790-1870`): without `const`, the generic error, which
                // `ConstantVerifier` reports at the name.
                let referenced = expr::resolve_name(inf, cx, name.sym, false);
                let resolved = match &referenced {
                    expr::NameRef::Element(el) => Some(crate::resolved::Resolved::Element(*el)),
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
                    expr::NameRef::Element(dartforge_elements::model::Element::Variable(v)) => !inf.program.variable(v).const_,
                    expr::NameRef::Element(dartforge_elements::model::Element::Function(f)) => {
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
                    expr::NameRef::None_ => expr::undefined_name_without_this(inf, cx, name),
                    // A type parameter as a constant type literal: outside a `const`
                    // constructor there is no lexical environment, and `_getConstantValue`
                    // gives `CONST_TYPE_PARAMETER` at the name (3.6.2
                    // `evaluation.dart:1847-1859`).
                    expr::NameRef::TypeParam(_) => {
                        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::CONST_TYPE_PARAMETER, name.span, &[]);
                    }
                    _ => {}
                }
                let current = cx.flow.clone();
                join_not_matched(inf, cx, &current);
                return;
            }
            // With `var`, `final` or a type, even in an assignment the pattern is a
            // `DeclaredVariablePattern` (the parser only reports
            // `PATTERN_ASSIGNMENT_DECLARES_VARIABLE`), and
            // `visitDeclaredVariablePattern` declares the local
            // (`resolution_visitor.dart:408-433`).
            if assignment && !*var_ && !f2 && ty.is_none() {
                // `visitAssignedVariablePattern` (`resolution_visitor.dart:189-210`): the
                // name through the lexical scope; without an element, `UNDEFINED_IDENTIFIER`;
                // an element that is neither a local nor a parameter,
                // `PATTERN_ASSIGNMENT_NOT_LOCAL_VARIABLE`.
                match expr::resolve_name(inf, cx, name.sym, false) {
                    expr::NameRef::Local(id) => {
                        if cx.local_functions.contains(&id) {
                            inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::PATTERN_ASSIGNMENT_NOT_LOCAL_VARIABLE, name.span, &[]);
                            return;
                        }
                        // Check the current flow before recording this write: earlier
                        // targets and the RHS may already have assigned a final local.
                        expr::check_final_local(inf, cx, id, name.span);
                        expr::write_in_primary(inf, cx, id, name.span);
                    }
                    expr::NameRef::ImplicitThis | expr::NameRef::Forward(_) | expr::NameRef::None_ => {
                        expr::undefined_name_without_this(inf, cx, name);
                    }
                    _ => {
                        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::PATTERN_ASSIGNMENT_NOT_LOCAL_VARIABLE, name.span, &[]);
                    }
                }
                if let Some(MemberName::Local(id)) = cx.lookup(name.sym) {
                    let decl = cx.local(id).ty;
                    if irrefutable_now(cx) && !inf.is_dynamic(t) && !inf.sub(t, decl) {
                        wrong_type_in_irrefutable(inf, t, decl, pattern_span);
                    }
                    promote_for_pattern(inf, cx, decl, true, false);
                    let written = matched(cx, &r);
                    let mut f = std::mem::replace(&mut cx.flow, Flow::reachable());
                    let reason = crate::inference::flow::NonPromotionReason::Write { name: name.sym, span: pattern_span };
                    inf.assign_flow(&mut f, id, decl, written, Some(reason));
                    cx.flow = f;
                }
                return;
            }
            let k = match ty {
                Some(x) => {
                    let k = inf.type_from_annotation(cx, x);
                    never_matches(inf, cx, t, k, inf.program.unit(cx.unit).ast.ty(x).span);
                    if irrefutable_now(cx) && !inf.is_dynamic(t) && !inf.sub(t, k) {
                        wrong_type_in_irrefutable(inf, t, k, pattern_span);
                    }
                    k
                }
                // `variableTypeFromInitializerType`: a `Null` value (including
                // `Never?`) declares the variable as `dynamic`.
                None => {
                    if classify(inf, t) == Class::Null {
                        inf.core.dynamic_
                    } else {
                        t
                    }
                }
            };
            promote_for_pattern(inf, cx, k, true, false);
            let inv = ty.is_some_and(|x| invalid_annotation(inf, cx, x, k));
            record_pattern_type(inf, cx, p, k, inv);
            let id = declare_local(
                inf,
                cx,
                Local { name: name.sym, ty: k, final_: final_ || f2, late: false, const_: false, offset: name.span.start, local_function: false },
                true,
            );
            // `declaredVariablePattern` + `assignMatchedPatternVariable` (3.6.2
            // `flow_analysis.dart:4528-4544`, `_initialize` `:5931-5962`): the variable
            // starts with the write of the matched type, which promotes through the type
            // of interest (the non-nullable of the declared type) if it has a written
            // type and is not `final` (`case int? x?:` leaves `x` as `int`).
            // The written type is the matched one **after** `promoteForPattern`
            // (`type_analyzer.dart:515-530`, `promotedValueType`): with an `Object?`
            // scrutinee, `case String? a?:` matches `Object` and the promotion to the
            // declared `String?` gives `String`.
            let promoted = matched(cx, &r);
            if ty.is_some() && !(final_ || f2) && !inf.is_dynamic(promoted) && inf.sub(promoted, k) {
                let mut f = std::mem::replace(&mut cx.flow, Flow::reachable());
                inf.write_flow(&mut f, id, k, promoted, true, None);
                cx.flow = f;
            }
        }
        PatternKind::Constant(e) => {
            let e = *e;
            // `visitConstantPattern` (`best_practices_verifier.dart:257-265`).
            if expr::is_double_nan(inf, cx, e) {
                inf.warning_with_code(w::UNNECESSARY_NAN_COMPARISON_FALSE, pattern_span, &[]);
            }
            let forced = refutable_in_irrefutable(inf, cx, pattern_span);
            let c = infer(inf, cx, e, t);
            if forced {
                cx.forced_refutable = false;
            }
            // `caseExpressionTypeMismatch` (`shared_type_analyzer.dart:32-45`):
            // without patterns (< 3.0), the constant of a `case` whose type is not
            // a subtype of the scrutinee's type.
            if !language_patterns(inf, cx) && cx.refutable_pattern && !inf.is_dynamic(c) && !inf.sub(c, t) {
                let sp = inf.span_expr(cx.unit, e);
                let a1 = inf.table.format(c, inf.interner, inf.program);
                let a2 = inf.table.format(t, inf.interner, inf.program);
                inf.warning_with_code(
                    dartforge_diagnostics::codes::compile_time_error::CASE_EXPRESSION_TYPE_IS_NOT_SWITCH_EXPRESSION_SUBTYPE,
                    sp,
                    &[&a1, &a2],
                );
            }
            if language_patterns(inf, cx) {
                equality(inf, cx, e, c, false);
            } else {
                // Without patterns (< 3.0): every `case` is reachable and nothing is
                // promoted (`constantPattern_end`, `:4489-4505`).
                let current = cx.flow.clone();
                join_not_matched(inf, cx, &current);
            }
        }
        PatternKind::Relational { op, value } => {
            let (op, value) = (*op, *value);
            if refutable_in_irrefutable(inf, cx, pattern_span) {
                cx.forced_refutable = false;
            }
            // `analyzeRelationalPattern`: the operator, the operand's context and the
            // checks (operand against the parameter, return against `bool`).
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
            // `logicalOrPattern_begin`: the left operand has its own "did not
            // match", which is the right one's entry.
            let empty = cx.flow.unreachable();
            let previous = cx.matching.as_mut().map(|c| std::mem::replace(&mut c.not_matched, empty));
            let start_x = cx.locals.len();
            type_check(inf, cx, x, final_, assignment, false);
            let start_y = cx.locals.len();
            let mut matched_left = cx.flow.clone();
            let failed = match (cx.matching.as_mut(), previous) {
                (Some(c), Some(previous)) => std::mem::replace(&mut c.not_matched, previous),
                _ => cx.flow.clone(),
            };
            cx.flow = failed;
            // The right operand of a `||` whose left one always matches is dead
            // code (Appendix B of §B).
            // The fragment starts at the `||` operator and goes to the end of the
            // operand (`dead_code_verifier.dart:321-324`).
            let (end_x, sp_y) = (inf.program.unit(cx.unit).ast.pattern(x).span.end, inf.program.unit(cx.unit).ast.pattern(y).span);
            super::super::statements::enter_flow(cx, sp_y.end);
            if !cx.flow.reachable && cx.dead_code.is_none() {
                let source = inf.program.unit(cx.unit).source.as_bytes();
                let operator = source
                    .get(end_x..sp_y.start)
                    .and_then(|snippet| snippet.windows(2).position(|w| w == b"||"))
                    .map_or(sp_y.start, |i| end_x + i);
                inf.warning(crate::codes::DEAD_CODE.template.to_string(), Span { start: operator, end: sp_y.end });
                cx.dead_code = Some(cx.flow_ends.len());
            }
            type_check(inf, cx, y, final_, assignment, false);
            super::super::statements::exit_flow(cx);
            let matched_right = cx.flow.clone();
            // The join variable (the right operand's, which the scope sees) has, on
            // the left's branch, the state of the left's copy (`logicalOrPattern_end`
            // joins the variables by name): with the right one dead, the left's one
            // remains, assigned.
            for d in start_y..cx.locals.len() {
                let the_name = cx.locals[d].name;
                if let Some(e) = (start_x..start_y).find(|&e| cx.locals[e].name == the_name && cx.locals[e].offset != 0)
                    && let Some(m) = matched_left.model(LocalId(e as u32)).cloned()
                {
                    if matched_left.vars.len() <= d {
                        matched_left.vars.resize(d + 1, None);
                    }
                    matched_left.vars[d] = Some(m);
                }
            }
            cx.flow = inf.join(&matched_left, &matched_right);
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
                    // `nullCheckOrAssertPattern_begin` (3.6.2): a `?` may still not
                    // match a non-nullable value (mixed mode); only sound flow
                    // analysis (3.9) skips that join.
                    if !assertion && !inf.program.library(cx.lib).features.has(dartforge_frontend::Feature::SoundFlowAnalysis) {
                        let current = cx.flow.clone();
                        join_not_matched(inf, cx, &current);
                    }
                    // The matched value cannot be null: the `?`/`!` does nothing. A `?` in
                    // an irrefutable context is already another error.
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
            // `analyzeParenthesizedPattern` passes the context on, with the
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
            if !inv && !inf.table.is_invalid(c) && inf.sub(t, c) {
                if let Some(keyword) = as_keyword(inf, cx, pattern, ty) {
                    inf.warning_with_code(w::UNNECESSARY_CAST_PATTERN, keyword, &[]);
                }
            }
            if inf.is_non_nullable(c) && classify(inf, t) == Class::Null {
                inf.warning_with_code(w::CAST_FROM_NULL_ALWAYS_FAILS, pattern_span, &[]);
            }
            // A cast does not fail on a wrong type (it throws): nothing goes to
            // "did not match".
            promote_for_pattern(inf, cx, c, false, false);
            with_subpattern(inf, cx, c, |inf, cx| type_check(inf, cx, pattern, final_, assignment, false));
        }
        PatternKind::List { type_args, elements } => {
            let el = if let Some(x) = type_args.first() {
                inf.type_argument_type(cx, *x)
            } else if inf.is_dynamic(t) {
                // `dynamic` and the invalid type give elements of the same type.
                t
            } else {
                inf.as_instance_of(t, inf.core.list_class).map(|a| a[0]).unwrap_or(inf.core.object_nullable)
            };
            let the_required = inf.list(el);
            {
                let inv = type_args.first().is_some_and(|&x| invalid_annotation(inf, cx, x, el));
                record_pattern_type(inf, cx, p, the_required, inv);
                never_matches(inf, cx, t, the_required, pattern_span);
            }
            // Only `[...]` (one element, and it is the rest) matches every list.
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
            if irrefutable_now(cx) && !inf.assignable(t, the_required) {
                wrong_type_in_irrefutable(inf, t, the_required, pattern_span);
            }
        }
        PatternKind::Map { type_args, entries, .. } => {
            let (k, v) = if type_args.len() == 2 {
                (inf.type_argument_type(cx, type_args[0]), inf.type_argument_type(cx, type_args[1]))
            } else if inf.is_dynamic(t) {
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
                    // `visitMapPattern` (3.6.2 `resolver.dart:1549-1554`): on the whole
                    // pattern.
                    never_matches(inf, cx, t, the_required, pattern_span);
                    // A map may not have the key: it fails even with the right type.
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
                infer(inf, cx, key, k);
                with_subpattern(inf, cx, v, |inf, cx| type_check(inf, cx, val, final_, assignment, false));
            }
            if let Some(the_required) = map_required
                && irrefutable_now(cx)
                && !inf.assignable(t, the_required)
            {
                wrong_type_in_irrefutable(inf, t, the_required, pattern_span);
            }
        }
        PatternKind::Record { fields } => {
            let the_fields: Vec<(Option<ast::Name>, PatternId)> = fields.iter().map(|f| (f.name, f.pattern)).collect();
            // The record's shape: `(Object?, …, {Object? n})`.
            let names: Vec<Option<dartforge_intern::SymbolId>> = the_fields
                .iter()
                .map(|&(n, x)| n.map(|n| n.sym).or_else(|| if implicit_named_field(inf, cx, p, x) { implicit_name(inf, cx, x) } else { None }))
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
            // without a repeated named field, on the whole pattern.
            let repeated = {
                let mut seen = std::collections::HashSet::new();
                names.iter().flatten().any(|n| !seen.insert(*n))
            };
            if !repeated {
                never_matches(inf, cx, t, the_required, pattern_span);
            }
            promote_for_pattern(inf, cx, the_required, true, false);
            // The field types come from the matched type at the pattern's ENTRY
            // (`matchedValueType`, 3.6.2 `type_analyzer.dart:1608-1625`), not from
            // the one promoted to `(Object?, …)`.
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
                let ft = ft.unwrap_or(if inf.is_dynamic(t) { t } else { fields_top });
                let d = with_subpattern(inf, cx, ft, |inf, cx| type_check(inf, cx, x, final_, assignment, false));
                the_shown.push(d);
            }
            // The second promotion, to the record of the types demonstrated by the
            // subpatterns; it does not fail on type.
            let shown = assemble(inf, &the_shown);
            promote_for_pattern(inf, cx, shown, false, false);
            if irrefutable_now(cx) && !inf.assignable(t, the_required) {
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
                let the_name = n.map(|n| n.sym).or_else(|| implicit_name(inf, cx, x));
                // `resolveObjectPatternPropertyGet` (3.6.2 `resolver.dart:1562-1580`):
                // `TypePropertyResolver` with `propertyErrorEntity` at the pattern's type;
                // with a potentially nullable type, the unchecked use at the type and the
                // lookup on the type without `?`.
                let obj_lookup = match the_name {
                    Some(nm) if inf.requires_null_check(cx.lib, obj, nm, false) => {
                        let sp = inf.program.unit(cx.unit).ast.ty(ty).span;
                        let text = inf.interner.resolve(nm).to_string();
                        inf.null_warning(
                            obj,
                            dartforge_diagnostics::codes::compile_time_error::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
                            sp,
                            &[&text],
                        );
                        inf.non_null(obj)
                    }
                    _ => obj,
                };
                let ft = match the_name {
                    Some(nm) => match inf.lookup_member(cx.lib, obj_lookup, nm, false) {
                        Search::Finding(m) => {
                            let of_extension_type = matches!(
                                &m.resolved,
                                crate::resolved::Resolved::Member { class, .. }
                                    if inf.program.class(*class).kind == dartforge_elements::model::ClassKind::ExtensionType
                            );
                            if m.of_extension || of_extension_type {
                                inf.body_types.units[cx.unit.0 as usize].extension_fields.insert(x, m.ty);
                            }
                            inf.body_types.units[cx.unit.0 as usize].field_elements.insert(x, m.resolved.clone());
                            m.ty
                        }
                        Search::NeverType => inf.core.never,
                        _ => inf.core.dynamic_,
                    },
                    None => inf.core.dynamic_,
                };
                // A `Never` getter: the read does not return (`flow.handleExit()`).
                if matches!(inf.table.get(ft), Type::Never) {
                    cx.flow.reachable = false;
                }
                with_subpattern(inf, cx, ft, |inf, cx| type_check(inf, cx, x, final_, assignment, false));
            }
            if irrefutable_now(cx) && !inf.assignable(t, obj) {
                wrong_type_in_irrefutable(inf, t, obj, pattern_span);
            }
        }
    }
}

/// `case p when g` with the pattern flow: `(matched, not matched)`.
pub(crate) fn the_case(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    p: PatternId,
    t: TypeId,
    guard: Option<ExprId>,
    scrutinee: Option<ExprId>,
) -> (Flow, Flow) {
    // The scrutinee is resolved in the outer scope, before the pattern
    // declares its variables.
    let target = scrutinee.and_then(|e| expr::promotion_target(inf, cx, e));
    let property = target.is_some_and(|id| cx.is_property(id));
    // In a `switch`, all cases match the SAME reference: the promotions of a
    // case's "did not match" reach the next one.
    let reuse = match (cx.switch_scrutinee, scrutinee) {
        (Some(Some((e0, the_key, version))), Some(e)) if e0 == e && cx.flow.model(the_key).is_some() => Some((the_key, version)),
        _ => None,
    };
    let (the_key, version) = match reuse {
        Some(x) => x,
        None => {
            let the_key = new_key(inf, cx, t);
            let version = target.and_then(|id| cx.flow.version(id));
            // Only the `switch`'s own cases: an `if (x case p)` in a case's body
            // matches another reference and must not take the scrutinee's place.
            let own_case = match cx.switch_scrutinee {
                Some(None) => true,
                Some(Some((e0, _, _))) => scrutinee == Some(e0),
                None => false,
            };
            if own_case && let Some(e) = scrutinee {
                cx.switch_scrutinee = Some(Some((e, the_key, version)));
            }
            (the_key, version)
        }
    };
    let r = MatchedRef { key: the_key, ty: t, target, property, version };
    let refutable_before = std::mem::replace(&mut cx.refutable_pattern, true);
    let forced_before = std::mem::replace(&mut cx.forced_refutable, false);
    let first_local = cx.locals.len();
    // `visitGuardedPattern`: the pattern's variables stay hidden during the
    // whole pattern (not in the guard).
    let mut hidden = Vec::new();
    declared_variables(inf, cx, p, &mut hidden);
    let base_hidden = cx.pattern_hidden.len();
    cx.pattern_hidden.extend(hidden.iter().map(|n| (n.sym, n.span)));
    let mut not_ = match_(inf, cx, r, |inf, cx| type_check(inf, cx, p, false, false, false));
    cx.pattern_hidden.truncate(base_hidden);
    cx.refutable_pattern = refutable_before;
    cx.forced_refutable = forced_before;
    cx.last_pattern_locals = first_local..cx.locals.len();
    if let Some(g) = guard {
        // `visitGuardedPattern`: the pattern's variables, during the guard.
        let base = cx.variables_in_guard.len();
        cx.variables_in_guard.extend((first_local..cx.locals.len()).map(|i| LocalId(i as u32)));
        let (gv, gf) = expr::checked_condition(inf, cx, g);
        cx.variables_in_guard.truncate(base);
        cx.flow = gv;
        not_ = inf.join(&not_, &gf);
    }
    (cx.flow.clone(), not_)
}

/// Pattern declaration, assignment and `for-in`: the same matching, without
/// promoting a scrutinized variable (`allowScrutineePromotion: false`) and
/// with the "did not match" discarded. `the_value`: the initializer, when there
/// is one; a promotable property is promoted in any construct.
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
    let r = MatchedRef { key: the_key, ty: t, target, property: target.is_some(), version: None };
    let _ = match_(inf, cx, r, |inf, cx| type_check(inf, cx, p, final_, assignment, false));
}
