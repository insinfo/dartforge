//! Statements: blocks, declarations, control flow (with the
//! `flow-analysis.md` models for `if`, loops, `switch`, `try`, jumps),
//! `return` and `yield` (collected for the inference of closures' returns).

use super::body::{JumpTarget, Body, Local, MemberName};
use super::expr::{self, declare_local, infer, infer_free};
use super::flow::Flow;
use super::functions;
use super::patterns;
use super::BodyInferrer;
use crate::codes::*;
use crate::resolved::{LocalId, MemberRef, Resolved};
use crate::table::{Type, TypeId};
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, AsyncModifier, ExprId, ExprKind, StmtId, StmtKind, UnaryOp};
use dartforge_intern::SymbolId;

/// Infers a statement.
pub(crate) fn infer_statement(inf: &mut BodyInferrer<'_>, cx: &mut Body, s: StmtId) {
    validate_local_annotations(inf, cx, s);
    let a = &inf.program.unit(cx.unit).ast;
    let st = a.stmt(s);
    let span = st.span;
    match &st.kind {
        StmtKind::Block(stmts) => {
            cx.push_scope();
            // The block's scope contains all its local declarations.
            declare_forwards(inf, cx, stmts);
            // Without a basic block in progress, the fragment goes to the last
            // statement of this block.
            let block_end = stmts.last().map(|&x| inf.program.unit(cx.unit).ast.stmt(x).span.end).unwrap_or(span.end);
            cx.block_ends.push(block_end);
            for &x in stmts.iter() {
                if !cx.flow.reachable && cx.dead_code.is_none() {
                    let sx = inf.program.unit(cx.unit).ast.stmt(x).span;
                    let end = cx.flow_ends.last().copied().unwrap_or(block_end);
                    inf.warning(DEAD_CODE.template.to_string(), Span { start: sx.start, end: end.max(sx.end) });
                    cx.dead_code = Some(cx.flow_ends.len());
                    cx.dead_origin = Some(s);
                }
                infer_statement(inf, cx, x);
            }
            cx.block_ends.pop();
            cx.pop_scope();
        }
        StmtKind::Variables(vl) => variable_declaration(inf, cx, vl),
        StmtKind::PatternVariables { final_, pattern, value } => patterns::declaration(inf, cx, *final_, *pattern, *value),
        StmtKind::Function(f) => functions::the_local_function(inf, cx, *f),
        StmtKind::Expression(e) => {
            infer_free(inf, cx, *e);
        }
        StmtKind::If { condition, case_pattern, guard, then, else_ } => {
            let before = cx.flow.clone();
            let (vf, ff) = match case_pattern {
                Some(p) => {
                    let t = infer_free(inf, cx, *condition);
                    cx.push_scope();
                    patterns::case(inf, cx, *p, t, *guard, Some(*condition))
                }
                None => expr::checked_condition(inf, cx, *condition),
            };
            cx.flow = vf;
            flow_branch(inf, cx, *then);
            if case_pattern.is_some() {
                cx.pop_scope();
            }
            let after_then = std::mem::replace(&mut cx.flow, ff);
            if let Some(e) = else_ {
                flow_branch(inf, cx, *e);
            }
            let after_else = std::mem::replace(&mut cx.flow, before);
            cx.flow = inf.join(&after_then, &after_else);
        }
        StmtKind::While { condition, body } => {
            let the_labels = pending_labels(cx);
            let (writes, captured) = writes_in(inf, cx, &[Part::Expr(*condition), Part::Stmt(*body)]);
            cx.flow.conservative_join(&writes, &captured);
            let (vf, ff) = expr::checked_condition(inf, cx, *condition);
            cx.jumps.push(JumpTarget { labels: the_labels, the_loop: true, is_switch: false, breaks: Vec::new(), continues: Vec::new() });
            cx.flow = vf;
            flow_branch(inf, cx, *body);
            let tgt = cx.jumps.pop().unwrap();
            let mut exits = vec![ff];
            exits.extend(tgt.breaks);
            let base = cx.flow.clone();
            cx.flow = inf.join_all(&base, &exits);
        }
        StmtKind::DoWhile { body, condition } => {
            let the_labels = pending_labels(cx);
            let (writes, captured) = writes_in(inf, cx, &[Part::Expr(*condition), Part::Stmt(*body)]);
            cx.flow.conservative_join(&writes, &captured);
            cx.jumps.push(JumpTarget { labels: the_labels, the_loop: true, is_switch: false, breaks: Vec::new(), continues: Vec::new() });
            branch(inf, cx, *body);
            let tgt = cx.jumps.pop().unwrap();
            let mut the_conts = vec![cx.flow.clone()];
            the_conts.extend(tgt.continues);
            let base = cx.flow.clone();
            cx.flow = inf.join_all(&base, &the_conts);
            // The condition is the first dead node (the body neither reaches its end
            // nor continues): the `flowEnd` with a `DoStatement` parent
            // (`dead_code_verifier.dart:281-301`) reports the `do` (up to the `{` of
            // the block body) and from the `while` (from the `}` of the block body)
            // to the `;`; the rest starts at the token after the `;`, and does not
            // come out if the loop has a `break` (`hasBreakStatement`).
            if !cx.flow.reachable && cx.dead_code.is_none() {
                let ast_ = &inf.program.unit(cx.unit).ast;
                let the_body = ast_.stmt(*body);
                let block = matches!(the_body.kind, StmtKind::Block(_));
                let body_span = the_body.span;
                let source = inf.program.unit(cx.unit).source.as_bytes();
                let of_end = if block { body_span.start + 1 } else { span.start + 2 };
                let while_start = if block { body_span.end.saturating_sub(1) } else { next_token(source, body_span.end) };
                inf.warning(DEAD_CODE.template.to_string(), Span { start: span.start, end: of_end });
                inf.warning(DEAD_CODE.template.to_string(), Span { start: while_start, end: span.end });
                if tgt.breaks.is_empty() {
                    let begin = next_token(source, span.end);
                    let end = cx.flow_ends.last().copied().or(cx.block_ends.last().copied()).unwrap_or(span.end);
                    if end > begin {
                        inf.warning(DEAD_CODE.template.to_string(), Span { start: begin, end: end });
                    }
                }
                cx.dead_code = Some(cx.flow_ends.len());
            }
            let (_vf, ff) = expr::checked_condition(inf, cx, *condition);
            let mut exits = vec![ff];
            exits.extend(tgt.breaks);
            cx.flow = inf.join_all(&base, &exits);
        }
        StmtKind::For { init, condition, updates, body, .. } => {
            let the_labels = pending_labels(cx);
            cx.push_scope();
            if let Some(i) = init {
                for_initialization(inf, cx, i);
            }
            // An initialization that does not complete (`for (var i = throw 0; c; u)`):
            // the first dead node is the condition, and the parts' `flowEnd` goes
            // from it to the end of the last update (`node = parent.updaters.last`,
            // `dead_code_verifier.dart:309-310`); without updates `updaters.last`
            // throws and nothing comes out. The rest of the loop stays in the same
            // fragment.
            let dead_at_start = !cx.flow.reachable && cx.dead_code.is_none();
            if dead_at_start {
                let a = &inf.program.unit(cx.unit).ast;
                if let (Some(c), Some(u)) = (condition, updates.last()) {
                    let sp = Span { start: a.expr(*c).span.start, end: a.expr(*u).span.end };
                    inf.warning(DEAD_CODE.template.to_string(), sp);
                }
                cx.dead_code = Some(usize::MAX);
            }
            let mut parts = vec![Part::Stmt(*body)];
            if let Some(c) = condition {
                parts.push(Part::Expr(*c));
            }
            for u in updates.iter() {
                parts.push(Part::Expr(*u));
            }
            let (writes, captured) = writes_in(inf, cx, &parts);
            cx.flow.conservative_join(&writes, &captured);
            let (vf, ff) = match condition {
                Some(c) => expr::checked_condition(inf, cx, *c),
                None => {
                    let f = cx.flow.clone();
                    let i = f.unreachable();
                    (f, i)
                }
            };
            cx.jumps.push(JumpTarget { labels: the_labels, the_loop: true, is_switch: false, breaks: Vec::new(), continues: Vec::new() });
            cx.flow = vf;
            let dead_before = cx.dead_code.is_some();
            cx.dead_origin = None;
            flow_branch(inf, cx, *body);
            // `_reportForUpdaters`: the dead fragment started in the body (or in a
            // direct statement of it) and ended there: the updates are also dead
            // code.
            if !dead_before && cx.dead_code.is_none() && cx.dead_origin == Some(*body) {
                let a = &inf.program.unit(cx.unit).ast;
                if let (Some(p), Some(u)) = (updates.first(), updates.last()) {
                    let sp = Span { start: a.expr(*p).span.start, end: a.expr(*u).span.end };
                    inf.warning(DEAD_CODE.template.to_string(), sp);
                }
            }
            let tgt = cx.jumps.pop().unwrap();
            let mut the_conts = vec![cx.flow.clone()];
            the_conts.extend(tgt.continues);
            let base = cx.flow.clone();
            cx.flow = inf.join_all(&base, &the_conts);
            // Unreachable updates: their fragment is that of `_reportForUpdaters`
            // (or none); nothing inside opens another.
            let close = !cx.flow.reachable && cx.dead_code.is_none();
            if close {
                // At the current depth: the branch of a `?:` or of a `&&` inside the
                // updates enters one level below, and leaving it does not close this
                // fragment (`usize::MAX` closed it, and the dead branch inside opened
                // another report).
                cx.dead_code = Some(cx.flow_ends.len());
            }
            for u in updates.iter() {
                infer_free(inf, cx, *u);
            }
            if close {
                cx.dead_code = None;
            }
            let mut exits = vec![ff];
            exits.extend(tgt.breaks);
            cx.flow = inf.join_all(&base, &exits);
            // Without updates, the analyzer's `updaters.last` throws and the dead code
            // verifier does not go on into the body: the fragment stays open to its
            // end.
            if dead_at_start {
                // Depth 0: no `exit_flow` closes it.
                cx.dead_code = if condition.is_some() && updates.is_empty() { Some(0) } else { None };
            }
            cx.pop_scope();
        }
        StmtKind::ForIn { await_, target, iterable, body } => {
            let the_labels = pending_labels(cx);
            cx.push_scope();
            let before_body = {
                let reachable_before = cx.flow.reachable;
                let write = for_in_header(inf, cx, target, *iterable, *await_);
                // An iterable that does not complete (`Never`): the first dead node is
                // the loop variable (the `DeclaredIdentifier`, after the iterable in
                // flow order), and the fragment goes from it to the end of the basic
                // block.
                if reachable_before && !cx.flow.reachable && cx.dead_code.is_none() {
                    let source = inf.program.unit(cx.unit).source.as_bytes();
                    let mut i = span.start;
                    while i < span.end && source[i] != b'(' {
                        i += 1;
                    }
                    i += 1;
                    while i < span.end && source[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    let end = cx.flow_ends.last().copied().or(cx.block_ends.last().copied()).unwrap_or(span.end).max(span.end);
                    inf.warning(DEAD_CODE.template.to_string(), Span { start: i, end: end });
                    cx.dead_code = Some(cx.flow_ends.len());
                }
                let (writes, captured) = writes_in(inf, cx, &[Part::Stmt(*body)]);
                cx.flow.conservative_join(&writes, &captured);
                let before = cx.flow.clone();
                if let Some(x) = write {
                    x.apply(inf, cx);
                }
                before
            };
            cx.jumps.push(JumpTarget { labels: the_labels, the_loop: true, is_switch: false, breaks: Vec::new(), continues: Vec::new() });
            flow_branch(inf, cx, *body);
            let tgt = cx.jumps.pop().unwrap();
            let after = cx.flow.clone();
            let mut exits = vec![before_body, after];
            exits.extend(tgt.breaks);
            let base = cx.flow.clone();
            cx.flow = inf.join_all(&base, &exits);
            cx.pop_scope();
        }
        StmtKind::Switch { value, cases } => {
            let the_labels = pending_labels(cx);
            let t = infer_free(inf, cx, *value);
            expr::void_use(inf, cx, *value, t);
            let after_value = cx.flow.clone();
            let mut not_matched = after_value.clone();
            // All cases match the same reference to the value (T6).
            let outer_scrutinee = cx.switch_scrutinee.replace(None);
            cx.jumps.push(JumpTarget { labels: the_labels, the_loop: false, is_switch: true, breaks: Vec::new(), continues: Vec::new() });
            let mut has_default_value = false;
            let mut i = 0;
            // The index of the case group (those that share the body count as one).
            let mut group = 0usize;
            let n = cases.len();
            let mut exits: Vec<Flow> = Vec::new();
            while i < n {
                // Grouped cases: the empty ones fall through to the next body.
                let mut j = i;
                let mut the_entries: Vec<Flow> = Vec::new();
                cx.push_scope();
                let first_local = cx.locals.len();
                // The variables of each member of the group (the `addAll` of
                // `_SharedCaseScope`), and whether there is a label or `default`.
                let mut members: Vec<Vec<(SymbolId, LocalId)>> = Vec::new();
                let mut with_label = false;
                let mut with_default = false;
                loop {
                    let c = &cases[j];
                    if !c.labels.is_empty() {
                        with_label = true;
                    }
                    cx.flow = not_matched.clone();
                    // The head of each `case`/`default` is a basic block
                    // (`handleSwitchBeforeAlternative` and the `flowEnd` of
                    // `resolver.dart:1083`/`:1098`): if no value reaches it, the dead
                    // fragment is only the keyword, and the pattern and the guard do not
                    // open another (Appendix B of §B).
                    let pattern_flow = patterns::pattern_flow_enabled(inf);
                    if pattern_flow {
                        enter_flow(cx, c.span.end);
                        if !cx.flow.reachable && cx.dead_code.is_none() {
                            let keyword = case_keyword(inf, cx, c);
                            inf.warning(DEAD_CODE.template.to_string(), keyword);
                            cx.dead_code = Some(cx.flow_ends.len());
                        }
                    }
                    match c.pattern {
                        Some(p) => {
                            let (vf, ff) = patterns::case(inf, cx, p, t, c.guard, Some(*value));
                            the_entries.push(vf);
                            not_matched = ff;
                            let range = cx.last_pattern_locals.clone();
                            members.push(
                                range
                                    .filter(|&k| cx.locals[k].offset != 0)
                                    .map(|k| (cx.locals[k].name, LocalId(k as u32)))
                                    .collect(),
                            );
                        }
                        None => {
                            has_default_value = true;
                            with_default = true;
                            members.push(Vec::new());
                            the_entries.push(not_matched.clone());
                            not_matched = not_matched.unreachable();
                        }
                    }
                    if pattern_flow {
                        exit_flow(cx);
                    }
                    if !c.body.is_empty() || j + 1 >= n {
                        break;
                    }
                    j += 1;
                }
                // The join variable (the one the body sees, the copy from the last case)
                // has, in each alternative, the state of that alternative's copy: the
                // join of the flows keeps the promotion common to all of them
                // (`switchStatement_endAlternatives` joins the join variables,
                // `case int? x?: case int? x?:` → `int`).
                if j > i && members.len() == the_entries.len() {
                    if let Some(first) = members.first() {
                        for &(n, _) in first {
                            let ids: Option<Vec<LocalId>> = members.iter().map(|m| m.iter().find(|(x, _)| *x == n).map(|(_, id)| *id)).collect();
                            let Some(ids) = ids else { continue };
                            let Some(&rep) = ids.last() else { continue };
                            for (k, &idk) in ids.iter().enumerate() {
                                if idk == rep {
                                    continue;
                                }
                                if let Some(m) = the_entries[k].model(idk).cloned() {
                                    let r = rep.0 as usize;
                                    if the_entries[k].vars.len() <= r {
                                        the_entries[k].vars.resize(r + 1, None);
                                    }
                                    the_entries[k].vars[r] = Some(m);
                                }
                            }
                        }
                    }
                }
                let base = after_value.clone();
                cx.flow = inf.join_all(&base, &the_entries);
                // A group with a label (`switchStatement_endAlternatives` with
                // `hasLabels`, 3.6.2 `flow_analysis.dart:5421-5425`): a `continue L` may
                // come from any point of the `switch`; the body starts from the state
                // after the scrutinee, with the conservative join of the variables
                // written or captured in the whole `switch` (a write after the label
                // demotes).
                if with_label {
                    let (writes, captured) = writes_in(inf, cx, &[Part::Stmt(s)]);
                    let mut f = after_value.clone();
                    f.conservative_join(&writes, &captured);
                    cx.flow = f;
                }
                // Cases that share the body: the pattern variables are join variables,
                // assigned on any path that reaches the body (each `case` declared its
                // own copy; the join of the flows alone saw them as unassigned).
                if j > i {
                    for id in first_local..cx.locals.len() {
                        let id = LocalId(id as u32);
                        if cx.flow.model(id).is_none() {
                            cx.flow.declare(id);
                        }
                        cx.flow.initialize(id);
                    }
                }
                if with_label {
                    members.push(Vec::new());
                }
                group_joins(inf, cx, &members, with_label || with_default);
                let the_body: Vec<StmtId> = cases[j].body.to_vec();
                // The group's body is a basic block (`resolver.dart:1126`): the dead
                // fragment goes from the first unreachable statement to the end of the
                // body's last statement.
                let pattern_flow = patterns::pattern_flow_enabled(inf);
                let body_end = the_body.last().map(|&x| inf.program.unit(cx.unit).ast.stmt(x).span.end);
                if pattern_flow {
                    if let Some(end) = body_end {
                        enter_flow(cx, end);
                    }
                }
                // The member's statements form a local scope with their declarations
                // (the `LocalScope` of the `SwitchMember`'s `statements`): using before
                // declaring is `REFERENCED_BEFORE_DECLARATION`.
                declare_forwards(inf, cx, &the_body);
                for s in the_body {
                    if pattern_flow && !cx.flow.reachable && cx.dead_code.is_none() {
                        if let Some(end) = body_end {
                            let start = inf.program.unit(cx.unit).ast.stmt(s).span.start;
                            inf.warning(DEAD_CODE.template.to_string(), Span { start: start, end: end });
                            cx.dead_code = Some(cx.flow_ends.len());
                        }
                    }
                    infer_statement(inf, cx, s);
                }
                if pattern_flow && body_end.is_some() {
                    exit_flow(cx);
                }
                // `switchCaseCompletesNormally` (`shared_type_analyzer.dart:232-239`,
                // `analyzeSwitchStatement`): without patterns (< 3.0), the body of a group
                // that is not the last cannot reach its end; at the keyword of the group's
                // first member.
                // The 3.6.2 analyzer receives the GROUP's index and uses it on the member
                // list (`node.members[caseIndex].keyword`,
                // `shared_type_analyzer.dart:232-239`): with cases sharing the body
                // earlier, the report lands on an earlier member.
                let version = inf.program.library(cx.lib).features.version();
                if cx.flow.reachable && j + 1 < n && version < dartforge_frontend::features::LanguageVersion::new(3, 0) {
                    let keyword = case_keyword(inf, cx, &cases[group]);
                    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::SWITCH_CASE_COMPLETES_NORMALLY, keyword, &[]);
                }
                if cx.flow.reachable {
                    exits.push(cx.flow.clone());
                }
                cx.pop_scope();
                i = j + 1;
                group += 1;
            }
            let tgt = cx.jumps.pop().unwrap();
            cx.switch_scrutinee = outer_scrutinee;
            exits.extend(tgt.breaks);
            if !has_default_value && !exhaustive_switch(inf, cx, t, cases) {
                exits.push(not_matched);
            }
            cx.flow = inf.join_all(&after_value, &exits);
        }
        StmtKind::Break(label) => {
            let f = cx.flow.clone();
            if let Some(tgt) = jump_target(cx, label.map(|n| n.sym), false) {
                tgt.breaks.push(f);
            }
            cx.flow.reachable = false;
        }
        StmtKind::Continue(label) => {
            let f = cx.flow.clone();
            if let Some(tgt) = jump_target(cx, label.map(|n| n.sym), true) {
                tgt.continues.push(f);
            }
            cx.flow.reachable = false;
        }
        StmtKind::Return(e) => {
            return_(inf, cx, *e, span);
            cx.flow.reachable = false;
        }
        StmtKind::Yield { star, value } => {
            let (m, k) = match cx.functions.last() {
                Some(f) => (f.modifier, f.return_context),
                None => (AsyncModifier::None, inf.core.unknown),
            };
            // The generator's declared return type (`imposedType`).
            let declared = cx.functions.last().and_then(|f| f.the_return.filter(|_| f.executable.is_some()).or(f.closure_imposed));
            if *star {
                let ctx = if m == AsyncModifier::AsyncStar { inf.flow_of(k) } else { inf.iterable(k) };
                let t = infer(inf, cx, *value, ctx);
                expr::void_use(inf, cx, *value, t);
                // `YieldStatementResolver._resolve_generator` (`:171-175`).
                if matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
                    expr::nullable_dereference(inf, cx, *value, t, dartforge_diagnostics::codes::compile_time_error::UNCHECKED_USE_OF_NULLABLE_VALUE_IN_YIELD_EACH);
                }
                invalid_yield(inf, cx, *value, t, declared, true, m);
                let cls = if m == AsyncModifier::AsyncStar { inf.core.stream_class } else { inf.core.iterable_class };
                let el = if inf.is_dynamic(t) { inf.core.dynamic_ } else { inf.as_instance_of(t, cls).map(|a| a[0]).unwrap_or(inf.core.dynamic_) };
                if let Some(f) = cx.functions.last_mut() {
                    f.returned.push(el);
                }
            } else {
                let t = infer(inf, cx, *value, k);
                expr::void_use(inf, cx, *value, t);
                invalid_yield(inf, cx, *value, t, declared, false, m);
                if let Some(f) = cx.functions.last_mut() {
                    f.returned.push(t);
                }
            }
        }
        StmtKind::Try { body, catches, finally_ } => {
            let before = cx.flow.clone();
            let (writes, captured) = writes_in(inf, cx, &[Part::Stmt(*body)]);
            flow_branch(inf, cx, *body);
            let after_try = cx.flow.clone();
            let mut exits = vec![after_try];
            // `_CatchClausesVerifier` (`dead_code_verifier.dart:495-545`): the `on`
            // types already seen, and whether some `catch` was already found dead.
            let mut catch_types: Vec<TypeId> = Vec::new();
            let mut dead_catch = false;
            for (catch_index, c) in catches.iter().enumerate() {
                let mut f = before.clone();
                f.conservative_join(&writes, &captured);
                cx.flow = f;
                cx.push_scope();
                let ex_type = match c.on_type {
                    Some(t) => inf.type_in_context(cx, t, crate::resolve::TypeContext::Catch),
                    None => inf.core.object,
                };
                {
                    use dartforge_diagnostics::codes::warning as w;
                    // `on T` with `T` potentially nullable (and not invalid): a nullable
                    // value is not thrown.
                    if let Some(t) = c.on_type {
                        if !inf.table.is_invalid(ex_type) && !inf.is_non_nullable(ex_type) {
                            let sp = inf.program.unit(cx.unit).ast.ty(t).span;
                            inf.warning_with_code(w::NULLABLE_TYPE_IN_CATCH_CLAUSE, sp, &[]);
                        }
                    }
                    if !dead_catch {
                        let last = catches.len() - 1;
                        let end = catches[last].span.end;
                        if c.on_type.is_none() || ex_type == inf.core.object {
                            // A `catch` that catches everything: the following ones are dead.
                            if catch_index != last {
                                let start = catches[catch_index + 1].span.start;
                                inf.warning_with_code(w::DEAD_CODE_CATCH_FOLLOWING_CATCH, Span { start: start, end: end }, &[]);
                                dead_catch = true;
                            }
                        } else {
                            let previous = catch_types.iter().copied().find(|&s| inf.sub(ex_type, s));
                            match previous {
                                Some(s) => {
                                    inf.warning_with_args(
                                        w::DEAD_CODE_ON_CATCH_SUBTYPE,
                                        Span { start: c.span.start, end: end },
                                        &[crate::display::Arg::Type_(ex_type), crate::display::Arg::Type_(s)],
                                    );
                                    dead_catch = true;
                                }
                                None => catch_types.push(ex_type),
                            }
                        }
                    }
                }
                if let Some(n) = &c.exception {
                    declare_local(inf, cx, Local { name: n.sym, ty: ex_type, final_: true, late: false, const_: false, offset: n.span.start, local_function: false }, true);
                }
                if let Some(n) = &c.stack_trace {
                    let st = stack_trace(inf);
                    declare_local(inf, cx, Local { name: n.sym, ty: st, final_: true, late: false, const_: false, offset: n.span.start, local_function: false }, true);
                }
                let end = flow_end(inf, cx, c.body);
                enter_flow(cx, end);
                infer_statement(inf, cx, c.body);
                exit_flow(cx);
                cx.pop_scope();
                exits.push(cx.flow.clone());
            }
            let after_catches = inf.join_all(&before, &exits);
            if let Some(fin) = finally_ {
                let mut f = inf.join(&after_catches, &before);
                f.conservative_join(&writes, &captured);
                let finally_start = f.clone();
                cx.flow = f;
                branch(inf, cx, *fin);
                if cx.flow.reachable {
                    // The `finally` completes (`attachFinally`): the state after the
                    // `try`/`catch` applies, with the `finally`'s promotions reapplied
                    // (`o as int;` in the `finally` promotes afterwards), and the
                    // `finally`'s state for what it wrote.
                    let end = cx.flow.clone();
                    let reaches = after_catches.reachable;
                    let mut r = inf.reapply(&after_catches, &end);
                    for (i, m) in end.vars.iter().enumerate() {
                        let v0 = finally_start.vars.get(i).and_then(|m| m.as_ref()).map(|m| m.version);
                        if m.is_some() && m.as_ref().map(|m| m.version) != v0 {
                            if r.vars.len() <= i {
                                r.vars.resize(i + 1, None);
                            }
                            r.vars[i] = m.clone();
                        }
                    }
                    r.reachable = reaches;
                    cx.flow = r;
                }
            } else {
                cx.flow = after_catches;
            }
        }
        StmtKind::Labeled { labels, body } => {
            let ls: Vec<SymbolId> = labels.iter().map(|l| l.sym).collect();
            let body_and_loop = matches!(
                inf.program.unit(cx.unit).ast.stmt(*body).kind,
                StmtKind::For { .. } | StmtKind::ForIn { .. } | StmtKind::While { .. } | StmtKind::DoWhile { .. } | StmtKind::Switch { .. }
            );
            if body_and_loop {
                cx.pending_labels.extend(ls);
                infer_statement(inf, cx, *body);
            } else {
                cx.jumps.push(JumpTarget { labels: ls, the_loop: false, is_switch: false, breaks: Vec::new(), continues: Vec::new() });
                infer_statement(inf, cx, *body);
                let tgt = cx.jumps.pop().unwrap();
                let mut exits = vec![cx.flow.clone()];
                exits.extend(tgt.breaks);
                let base = cx.flow.clone();
                cx.flow = inf.join_all(&base, &exits);
            }
        }
        StmtKind::Assert { condition, message } => {
            let before = cx.flow.clone();
            // `assert_afterCondition`: the message runs where the condition is
            // false (`assert(x == null || …, '$x.y')` sees `x` non-null). The
            // analyzer does not report dead code there (`assert(true, 'm')`).
            let (_, if_false) = expr::assert_condition(inf, cx, *condition);
            if let Some(m) = message {
                if if_false.reachable {
                    cx.flow = if_false;
                }
                infer_free(inf, cx, *m);
            }
            cx.flow = before;
        }
        StmtKind::Empty => {}
    }
}

/// Pre-declares (as "ahead") the locals of a statement list: variables,
/// local functions and the variables of pattern declarations.
fn declare_forwards(inf: &mut BodyInferrer<'_>, cx: &mut Body, stmts: &[StmtId]) {
    for &x in stmts.iter() {
        match &inf.program.unit(cx.unit).ast.stmt(x).kind {
            StmtKind::Variables(vl) => {
                for v in vl.variables.iter() {
                    cx.declare_forward(v.name.sym, v.name.span);
                }
            }
            StmtKind::Function(f) => {
                if let Some(n) = inf.program.unit(cx.unit).ast.function(*f).name {
                    cx.declare_forward(n.sym, n.span);
                }
            }
            StmtKind::PatternVariables { pattern, .. } => {
                let refutable = std::mem::replace(&mut cx.refutable_pattern, false);
                let mut names = Vec::new();
                patterns::declared_variables(inf, cx, *pattern, &mut names);
                cx.refutable_pattern = refutable;
                for n in names {
                    cx.declare_forward(n.sym, n.span);
                }
            }
            _ => {}
        }
    }
}

/// A branch (`then`, `else`, loop body) in its own scope.
fn branch(inf: &mut BodyInferrer<'_>, cx: &mut Body, s: StmtId) {
    cx.push_scope();
    infer_statement(inf, cx, s);
    cx.pop_scope();
}

/// A branch that closes a basic block (the analyzer's `flowEnd`:
/// `then`/`else`, body of `while`/`for`/`for-in`, body of the `try`): if it
/// is itself unreachable, the dead fragment is the whole statement
/// (`if (false) { … }` reports from the `{` to the `}`).
fn flow_branch(inf: &mut BodyInferrer<'_>, cx: &mut Body, s: StmtId) {
    let span = inf.program.unit(cx.unit).ast.stmt(s).span;
    if !cx.flow.reachable && cx.dead_code.is_none() {
        inf.warning(DEAD_CODE.template.to_string(), span);
        cx.dead_code = Some(cx.flow_ends.len() + 1);
        cx.dead_origin = Some(s);
    }
    let end = flow_end(inf, cx, s);
    enter_flow(cx, end);
    branch(inf, cx, s);
    exit_flow(cx);
}

/// The end of a basic block that ends at `s`, trimmed like the analyzer: in
/// a block with statements, the end of the last one.
pub(crate) fn flow_end(inf: &BodyInferrer<'_>, cx: &Body, s: StmtId) -> usize {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.stmt(s).kind {
        StmtKind::Block(stmts) if !stmts.is_empty() => a.stmt(*stmts.last().unwrap()).span.end,
        _ => a.stmt(s).span.end,
    }
}

/// Enters a basic block that ends at `end`.
pub(crate) fn enter_flow(cx: &mut Body, end: usize) {
    cx.flow_ends.push(end);
}

/// Leaves the basic block: a dead fragment started in it ends here.
pub(crate) fn exit_flow(cx: &mut Body) {
    let depth = cx.flow_ends.len();
    cx.flow_ends.pop();
    if cx.dead_code.is_some_and(|p| p >= depth) {
        cx.dead_code = None;
    }
}

/// The `case` or `default` keyword of a `switch` member, after the labels
/// (`label: case 1:`).
/// `switchStatementSharedCaseScopeFinish` (`variable_bindings.dart:177-202`) and
/// `finishJoinedPatternVariable` (`resolver.dart:878-915`): the variables of a
/// case group that shares the body. A variable that is not in all the members
/// (a `default` or a label counts as an empty member) is `…_HAS_LABEL` with a
/// label or `default`, otherwise `…_NOT_ALL_CASES`; present in all, but with
/// different types or `final`, `…_DIFFERENT_FINALITY_OR_TYPE`.
/// The code applies to each reference in the body.
fn group_joins(inf: &mut BodyInferrer<'_>, cx: &mut Body, members: &[Vec<(SymbolId, LocalId)>], with_label: bool) {
    use dartforge_diagnostics::codes::compile_time_error as c;
    let mut names: Vec<SymbolId> = Vec::new();
    let mut components: std::collections::HashMap<SymbolId, (Vec<LocalId>, bool)> = std::collections::HashMap::new();
    for (k, m) in members.iter().enumerate() {
        if k == 0 {
            for &(n, id) in m {
                if !components.contains_key(&n) {
                    names.push(n);
                    components.insert(n, (vec![id], true));
                }
            }
            continue;
        }
        for n in names.clone() {
            let entry = components.get_mut(&n).unwrap();
            match m.iter().find(|(x, _)| *x == n) {
                Some(&(_, id)) => entry.0.push(id),
                None => entry.1 = false,
            }
        }
        for &(n, id) in m {
            if !components.contains_key(&n) {
                names.push(n);
                components.insert(n, (vec![id], false));
            }
        }
    }
    for n in names {
        let (ids, all) = components.remove(&n).unwrap();
        if all && ids.len() == 1 {
            continue;
        }
        let code = if !all {
            Some(if with_label { c::PATTERN_VARIABLE_SHARED_CASE_SCOPE_HAS_LABEL } else { c::PATTERN_VARIABLE_SHARED_CASE_SCOPE_NOT_ALL_CASES })
        } else {
            let first = cx.local(ids[0]).clone();
            let typ = inf.table.canonical(first.ty);
            let different = ids[1..].iter().any(|&id| {
                let l = cx.local(id);
                inf.table.canonical(l.ty) != typ || l.final_ != first.final_
            });
            different.then_some(c::PATTERN_VARIABLE_SHARED_CASE_SCOPE_DIFFERENT_FINALITY_OR_TYPE)
        };
        if let Some(code) = code {
            for id in ids {
                cx.inconsistent_joins.insert(id, code);
            }
        }
    }
}

/// The start of the first token in `source` starting at `of` (skips
/// whitespace and comments).
fn next_token(source: &[u8], of: usize) -> usize {
    let mut i = of;
    loop {
        while i < source.len() && source[i].is_ascii_whitespace() {
            i += 1;
        }
        if source[i..].starts_with(b"//") {
            while i < source.len() && source[i] != b'\n' {
                i += 1;
            }
        } else if source[i..].starts_with(b"/*") {
            let mut prof = 0usize;
            while i < source.len() {
                if source[i..].starts_with(b"/*") {
                    prof += 1;
                    i += 2;
                } else if source[i..].starts_with(b"*/") {
                    prof -= 1;
                    i += 2;
                    if prof == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else {
            return i.min(source.len());
        }
    }
}

fn case_keyword(inf: &BodyInferrer<'_>, cx: &Body, c: &ast::SwitchCase) -> Span {
    let keyword: &[u8] = if c.pattern.is_some() { b"case" } else { b"default" };
    let source = inf.program.unit(cx.unit).source.as_bytes();
    let of = c.labels.last().map_or(c.span.start, |l| l.span.end);
    let until = c.span.end.min(source.len());
    let mut i = of;
    while i + keyword.len() <= until {
        let before_ok = i == 0 || !(source[i - 1].is_ascii_alphanumeric() || source[i - 1] == b'_');
        if before_ok && &source[i..i + keyword.len()] == keyword {
            return Span { start: i, end: i + keyword.len() };
        }
        i += 1;
    }
    Span { start: c.span.start, end: (c.span.start + keyword.len()).min(c.span.end) }
}

fn pending_labels(cx: &mut Body) -> Vec<SymbolId> {
    std::mem::take(&mut cx.pending_labels)
}

fn jump_target(cx: &mut Body, label: Option<SymbolId>, continue_: bool) -> Option<&mut JumpTarget> {
    for tgt in cx.jumps.iter_mut().rev() {
        match label {
            Some(r) => {
                if tgt.labels.contains(&r) {
                    return Some(tgt);
                }
            }
            None => {
                if tgt.the_loop || (!continue_ && tgt.is_switch) {
                    return Some(tgt);
                }
            }
        }
    }
    None
}

fn stack_trace(inf: &mut BodyInferrer<'_>) -> TypeId {
    let core = inf.core.core_library;
    let sym = inf.interner.lookup("StackTrace");
    match (core, sym) {
        (Some(l), Some(s)) => match inf.program.lookup(l, s).and_then(|b| b.getter) {
            Some(dartforge_elements::model::Element::Class(c)) => inf.class_type_with_args(c, Vec::new()),
            _ => inf.core.dynamic_,
        },
        _ => inf.core.dynamic_,
    }
}

/// The `isExhaustive` of `analyzeSwitchStatement`
/// (`_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart:2017-2027`)
/// of a `switch` without `default`: with patterns (language 3.0 or later), the
/// `isAlwaysExhaustiveType` of the scrutinee's type; before that, the legacy
/// `SwitchExhaustiveness` (`resolver.dart:5469-5557`): an enum whose constants
/// all appear in `case`s without a guard (a constant, parenthesized or not),
/// with `null` covered by the non-nullable type or by a `case null`.
fn exhaustive_switch(inf: &mut BodyInferrer<'_>, cx: &Body, t: TypeId, the_cases: &[ast::SwitchCase]) -> bool {
    if inf.program.library(cx.lib).features.version().major >= 3 {
        return always_exhaustive(inf, t, 0);
    }
    let Type::Interface { class, nullable, .. } = inf.table.get(t).clone() else { return false };
    let k = inf.program.class(class);
    if k.kind != dartforge_elements::model::ClassKind::Enum {
        return false;
    }
    let mut constants: Vec<dartforge_elements::model::VariableId> = k.enum_constants.clone();
    let mut covered_null = !nullable;
    let a = &inf.program.unit(cx.unit).ast;
    let the_body = &inf.body_types.units[cx.unit.0 as usize];
    let mut exhaustive = false;
    for c in the_cases {
        let Some(p) = c.pattern else { continue };
        if c.guard.is_some() {
            continue;
        }
        let ast::PatternKind::Constant(mut e) = a.pattern(p).kind else { continue };
        while let ast::ExprKind::Parenthesized(i) = a.expr(e).kind {
            e = i;
        }
        // `_referencedElement`: the constant's getter (its variable).
        let variable = match the_body.get_resolved(e) {
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => Some(*v),
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) => inf.program.function(*f).variable,
            Some(Resolved::Element(dartforge_elements::model::Element::Variable(v))) => Some(*v),
            _ => None,
        };
        if let Some(v) = variable {
            constants.retain(|x| *x != v);
        }
        if matches!(a.expr(e).kind, ast::ExprKind::Null) {
            covered_null = true;
        }
        if constants.is_empty() && covered_null {
            exhaustive = true;
        }
    }
    exhaustive
}

/// `TypeSystemImpl.isAlwaysExhaustive` (analyzer 7.7.1,
/// `type_system.dart:819`): the `?` does not count (the `S?` of a `sealed`
/// class is always exhaustive: `case var y?` and `case null` cover it);
/// `Null`, `bool`, enums and `sealed` classes; an extension type through its
/// *erasure*; `FutureOr<T>` through `T`; a type parameter through its
/// promoted or declared bound; a record when all its fields are.
fn always_exhaustive(inf: &mut BodyInferrer<'_>, t: TypeId, prof: u32) -> bool {
    if prof > 32 {
        return false;
    }
    match inf.table.get(t).clone() {
        Type::Null => true,
        Type::Interface { class, .. } => {
            let c = inf.program.class(class);
            c.kind == dartforge_elements::model::ClassKind::Enum
                || c.modifiers.sealed
                || Some(class) == inf.core.bool_class
        }
        Type::ExtensionType { decl, .. } => {
            let Some(rep) = inf.program.class(decl).representation else {
                return false;
            };
            let Some(d) = inf.outline.variables[rep.0 as usize].declared_type else {
                return false;
            };
            let erased = inf.substitute_from_owner(t, decl, decl, d);
            always_exhaustive(inf, erased, prof + 1)
        }
        Type::FutureOr { arg, .. } => always_exhaustive(inf, arg, prof + 1),
        Type::Intersection { param, bound } => {
            always_exhaustive(inf, bound, prof + 1) || {
                let b = inf.table.param(param).bound;
                always_exhaustive(inf, b, prof + 1)
            }
        }
        Type::TypeParameter { param, .. } => {
            let b = inf.table.param(param).bound;
            always_exhaustive(inf, b, prof + 1)
        }
        Type::Record { positional, named, .. } => positional
            .iter()
            .copied()
            .chain(named.iter().map(|(_, c)| *c))
            .collect::<Vec<_>>()
            .into_iter()
            .all(|c| always_exhaustive(inf, c, prof + 1)),
        _ => false,
    }
}

/// `_checkForYieldOfInvalidType` (`an611:src/dart/resolver/yield_statement_resolver.dart:
/// 72-140`): the value of a `yield` does not fit the element of the declared return
/// (`YIELD_OF_INVALID_TYPE`), or that of a `yield*` does not fit the return
/// (`YIELD_EACH_OF_INVALID_TYPE`), at the expression.
fn invalid_yield(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, t: TypeId, r: Option<TypeId>, the_star: bool, m: AsyncModifier) {
    use dartforge_diagnostics::codes::compile_time_error as c;
    if !matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
        return;
    }
    let sp = inf.span_expr(cx.unit, e);
    let cls = if m == AsyncModifier::AsyncStar { inf.core.stream_class } else { inf.core.iterable_class };
    if the_star {
        if let Some(r) = r.filter(|&r| !inf.assignable(t, r)) {
            inf.warning_with_args(c::YIELD_EACH_OF_INVALID_TYPE, sp, &[crate::display::Arg::Type_(t), crate::display::Arg::Type_(r)]);
            return;
        }
        let d = inf.core.dynamic_;
        let req = if m == AsyncModifier::AsyncStar { inf.flow_of(d) } else { inf.iterable(d) };
        if !inf.assignable(t, req) {
            inf.warning_with_args(c::YIELD_EACH_OF_INVALID_TYPE, sp, &[crate::display::Arg::Type_(t), crate::display::Arg::Type_(req)]);
        }
    } else if let Some(args) = r.and_then(|r| inf.as_instance_of(r, cls)) {
        let v = args[0];
        if !inf.assignable(t, v) {
            inf.warning_with_args(c::YIELD_OF_INVALID_TYPE, sp, &[crate::display::Arg::Type_(t), crate::display::Arg::Type_(v)]);
        }
    }
}

/// The initializer of a `late` local: the flow treats it as a function
/// literal (`lateInitializer_begin`/`_end` are those of
/// `functionExpression`): inside it, the variables written anywhere in the
/// member are not definitely unassigned; after it, the flow is the one from
/// before, and what it writes counts as a captured write.
fn late_initializer(inf: &mut BodyInferrer<'_>, cx: &mut Body, init: ExprId, ctx: TypeId) -> TypeId {
    let before = cx.flow.clone();
    let mut inside = before.clone();
    if cx.written_in_body.is_none() {
        let a = &inf.program.unit(cx.unit).ast;
        cx.written_in_body = Some(match cx.root {
            super::body::Root::Function(f) => separately_written_names(inf, cx.unit, &a.function(f).body),
            super::body::Root::Constructor(m) => match &a.member(m).kind {
                ast::MemberKind::Constructor(c) => separately_written_names(inf, cx.unit, &c.body),
                _ => Default::default(),
            },
            super::body::Root::Nothing => Default::default(),
        });
    }
    let (outside, captured) = cx.written_in_body.clone().unwrap_or_default();
    for e in outside.iter() {
        if let Some(id) = write_local(cx, *e) {
            inside.conservative_join(&[id], &[]);
        }
    }
    for e in captured.iter() {
        if let Some(id) = write_local(cx, *e) {
            inside.conservative_join(&[], &[id]);
        }
    }
    cx.flow = inside;
    let t = infer(inf, cx, init, ctx);
    // What the initializer writes, syntactically.
    let mut v = Sweep::new(&inf.program.unit(cx.unit).ast);
    v.expr(init, false);
    let writes: Vec<Write> = v.outside.into_iter().chain(v.inside).collect();
    cx.flow = before;
    for e in writes {
        if let Some(id) = write_local(cx, e) {
            cx.flow.capture(id);
        }
    }
    t
}

/// Local variable declaration (`var`, `final`, typed, `late`, `const`).
pub(crate) fn variable_declaration(inf: &mut BodyInferrer<'_>, cx: &mut Body, vl: &ast::VariableList) {
    let declared = vl.ty.map(|t| inf.type_from_annotation(cx, t));
    let u = inf.core.unknown;
    for v in vl.variables.iter() {
        let mut typ = declared;
        let mut written = None;
        let mut saved_condition = None;
        if let Some(init) = v.initializer {
            // An initializer that is a condition (`x != null && …`): the true/false
            // models are kept in the variable (§7.10); `late` never keeps them.
            let t = if vl.late {
                late_initializer(inf, cx, init, declared.unwrap_or(u))
            } else if expr::is_condition_form(inf, cx, init) {
                let (yes, not_) = expr::the_condition(inf, cx, init);
                cx.flow = inf.join(&yes, &not_);
                saved_condition = Some((yes, not_));
                inf.body_types.units[cx.unit.0 as usize].get_type(init).unwrap_or(inf.core.bool_)
            } else {
                infer(inf, cx, init, declared.unwrap_or(u))
            };
            written = Some(t);
            if let Some(d) = declared {
                expr::verify_assignable_expr(inf, cx, init, t, d, INVALID_ASSIGNMENT.template);
            } else {
                typ = Some(if matches!(inf.table.get(t), Type::Null) { inf.core.dynamic_ } else { t });
            }
            if vl.const_ {
                const_collections(inf, cx, init);
            }
            // `DeadCodeVerifier.visitVariableDeclaration`
            // (`dead_code_verifier.dart:114-128`): a `late` wildcard local (3.7) with
            // an initializer, which is never evaluated.
            if vl.late && cx.wildcard == Some(v.name.sym) {
                let sp = inf.span_expr(cx.unit, init);
                inf.warning_with_code(dartforge_diagnostics::codes::warning::DEAD_CODE_LATE_WILDCARD_VARIABLE_INITIALIZER, sp, &[]);
            }
        }
        let typ = typ.unwrap_or(inf.core.dynamic_);
        let id = declare_local(
            inf,
            cx,
            Local { name: v.name.sym, ty: typ, final_: vl.final_ || vl.const_, late: vl.late, const_: vl.const_, offset: v.name.span.start, local_function: false },
            false,
        );
        match written {
            Some(t) if !vl.late => {
                let toi = declared.is_some() && !vl.final_;
                let mut f = std::mem::replace(&mut cx.flow, Flow::reachable());
                inf.write_flow(&mut f, id, typ, t, toi, None);
                cx.flow = f;
            }
            Some(_) => cx.flow.initialize(id),
            None => {}
        }
        // `_initialize` (`flow_analysis.dart:5957-5962`): before 2.15
        // (`respectImplicitlyTypedVarInitializers`, `constructor-tearoffs`), a
        // variable without a written type ignores the initializer's information
        // (the behaviour of dart-lang/language#1785).
        let library_version = inf.program.library(cx.lib).features.version();
        let respects_initializer = declared.is_some() || (library_version.major, library_version.minor) >= (2, 15);
        if respects_initializer
            && let (Some((yes, not_)), Some(version)) = (saved_condition, cx.flow.version(id))
        {
            cx.conditions.insert(id, (yes, not_, version));
        }
    }
}

fn const_collections(inf: &mut BodyInferrer<'_>, cx: &Body, init: ExprId) {
    let unit = cx.unit;
    super::collections::validate_const_collection(inf, cx, init);
    let mut av = crate::constant::ConstantEvaluator::new(inf.program, inf.interner, inf.table, inf.core);
    let r = av.evaluate_expr(unit, init);
    let error = av.error_thrown.clone();
    drop(av);
    if r.is_none() {
        let sp = inf.span_expr(unit, init);
        match error {
            Some(e) => inf.warning(format!("{}: {}", CONST_EVAL_THROWS_EXCEPTION.template, e), sp),
            None if !inf.is_constant(cx, init) => inf.warning(CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE.template.to_string(), sp),
            None => {}
        }
    }
}

/// Initialization of a classic `for`.
pub(crate) fn for_initialization(inf: &mut BodyInferrer<'_>, cx: &mut Body, init: &ast::ForInit) {
    match init {
        ast::ForInit::Variables(vl) => variable_declaration(inf, cx, vl),
        ast::ForInit::Expression(e) => {
            infer_free(inf, cx, *e);
        }
        ast::ForInit::Pattern { final_, pattern, value } => patterns::declaration(inf, cx, *final_, *pattern, *value),
    }
}

/// The write to the existing variable of a `for (x in e)`, deferred until
/// after `forEach_bodyBegin` (the loop's conservative join and the state
/// before the body, which goes into `forEach_end`).
pub(crate) struct LoopWrite {
    pub(crate) local: LocalId,
    declared: TypeId,
    written: TypeId,
    reason: super::flow::NonPromotionReason,
}

impl LoopWrite {
    /// The identifier's `flow.write` (`for_resolver.dart:187-191`).
    pub(crate) fn apply(self, inf: &mut BodyInferrer<'_>, cx: &mut Body) {
        let mut f = std::mem::replace(&mut cx.flow, Flow::reachable());
        inf.assign_flow(&mut f, self.local, self.declared, self.written, Some(self.reason));
        cx.flow = f;
    }
}

/// Header of a `for-in` (statement or collection element): infers the
/// iterable and declares the loop variable. An assignment to an existing
/// variable goes back to the caller, which applies it after the
/// conservative join (`forEach_bodyBegin`).
pub(crate) fn for_in_header(inf: &mut BodyInferrer<'_>, cx: &mut Body, target: &ast::ForInTarget, iterable: ExprId, await_: bool) -> Option<LoopWrite> {
    let cls = if await_ { inf.core.stream_class } else { inf.core.iterable_class };
    // Context of the iterable: `Iterable<T>` with the variable's written type.
    let written = match target {
        ast::ForInTarget::Declared { ty: Some(t), .. } => Some(inf.type_from_annotation(cx, *t)),
        _ => None,
    };
    let u = inf.core.unknown;
    let ctx = match cls {
        Some(c) => {
            let el = written.unwrap_or(u);
            inf.iface(Some(c), vec![el])
        }
        None => u,
    };
    let t = infer(inf, cx, iterable, ctx);
    expr::void_use(inf, cx, iterable, t);
    // `ForResolver._forEachParts` (`for_resolver.dart:164-169`).
    expr::nullable_dereference(inf, cx, iterable, t, dartforge_diagnostics::codes::compile_time_error::UNCHECKED_USE_OF_NULLABLE_VALUE_AS_ITERATOR);
    let valid_parts = for_in_invalid_type(inf, cx, target, iterable, t, written, await_);
    // `visitForEachPartsWithDeclaration` (`error_verifier.dart:912-921`): with
    // `_checkForEachParts` without error, the variable's `const`.
    if valid_parts
        && let ast::ForInTarget::Declared { const_: Some(keyword), .. } = target
    {
        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::FOR_IN_WITH_CONST_VARIABLE, *keyword, &[]);
    }
    // `_computeForEachElementType` (`for_resolver.dart:92-116`): a `dynamic`
    // iterable gives `dynamic`; what is not an `Iterable`/`Stream` (including
    // `InvalidType` itself) gives `InvalidType`, and the element's uses do
    // not report anything else.
    let el = if inf.table.is_invalid(t) {
        t
    } else if inf.is_dynamic(t) {
        inf.core.dynamic_
    } else {
        match inf.as_instance_of(t, cls) {
            Some(a) => a[0],
            None => inf.table.invalid(inf.core.dynamic_),
        }
    };
    match target {
        ast::ForInTarget::Declared { final_, name, .. } => {
            let typ = written.unwrap_or(el);
            declare_local(inf, cx, Local { name: name.sym, ty: typ, final_: *final_, late: false, const_: false, offset: name.span.start, local_function: false }, true);
        }
        ast::ForInTarget::Pattern { final_, pattern } => {
            patterns::declare_by_type(inf, cx, *final_, *pattern, el);
        }
        ast::ForInTarget::Expression(e) => {
            // `for (x in e)`: assignment to an existing variable.
            let a = &inf.program.unit(cx.unit).ast;
            if let ExprKind::Identifier(n) = &a.expr(*e).kind {
                if let Some(MemberName::Local(id)) = cx.lookup(n.sym) {
                    expr::resolver(inf, cx, *e, crate::resolved::Resolved::Local(id));
                    // `checkFinalAlreadyAssigned(identifier, isForEachIdentifier: true)`
                    // (3.6.2 `for_resolver.dart:131-136`,
                    // `assignment_expression_resolver.dart:354-388`): a `final` local always
                    // reports, without looking at definite assignment.
                    let l = cx.local(id);
                    // `visitForEachPartsWithIdentifier` → `_checkForAssignmentToFinal`
                    // (3.6.2 `error_verifier.dart:926-930`, `:2155-2161`): the local constant,
                    // with the loop's parts valid.
                    if l.const_ && valid_parts {
                        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::ASSIGNMENT_TO_CONST, n.span, &[]);
                    }
                    if l.final_ && !l.const_ && !cx.local_functions.contains(&id) {
                        if l.late {
                            inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::LATE_FINAL_LOCAL_ALREADY_ASSIGNED, n.span, &[]);
                        } else {
                            let msg = format!("{}: '{}'", crate::codes::ASSIGNMENT_TO_FINAL_LOCAL.template, inf.interner.resolve(l.name));
                            inf.warning(msg, n.span);
                        }
                    }
                    let decl = cx.local(id).ty;
                    expr::record_type(inf, cx, *e, decl);
                    // `DemoteViaExplicitWrite` at the identifier
                    // (`ForEachPartsWithIdentifier.identifier`).
                    let reason = super::flow::NonPromotionReason::Write { name: n.sym, span: n.span };
                    return Some(LoopWrite { local: id, declared: decl, written: el, reason });
                }
            }
            infer_free(inf, cx, *e);
        }
    }
    None
}

/// The annotations of a local declaration (`Ast::local_metadata`), through
/// `AnnotationResolver` in the local scope: a name that resolves to a local
/// (`_localVariable`, 3.6.2 `annotation_resolver.dart:170-184`) is
/// `INVALID_ANNOTATION` if it is not `const` or if there are arguments; a
/// local function too (it is neither a variable nor an accessor); the rest
/// through the library's scope, like declaration annotations.
fn validate_local_annotations(inf: &mut BodyInferrer<'_>, cx: &mut Body, s: StmtId) {
    let a = &inf.program.unit(cx.unit).ast;
    let Some((_, ms)) = a.local_metadata.iter().find(|(x, _)| *x == s) else { return };
    for m in ms.iter() {
        validate_local_annotation(inf, cx, m);
    }
}

/// An annotation in the local scope (see [`validate_local_annotations`]).
pub(crate) fn validate_local_annotation(inf: &mut BodyInferrer<'_>, cx: &Body, m: &ast::Annotation) {
    let Some(n1) = m.name.first() else { return };
    match cx.lookup(n1.sym) {
        Some(MemberName::Local(id)) => {
            let l = cx.local(id);
            if cx.local_functions.contains(&id) || !l.const_ || m.arguments.is_some() || m.name.len() > 1 {
                inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INVALID_ANNOTATION, m.span, &[]);
            }
        }
        Some(_) => {}
        None => super::functions::validate_annotation(inf, cx.unit, cx.class, m),
    }
}

/// `_checkForEachParts` (an611:src/generated/error_verifier.dart:3103-3233):
/// `FOR_IN_OF_INVALID_TYPE` when the iterable (resolved to the bound) is not
/// an `Iterable`/`Stream`, and `FOR_IN_OF_INVALID_ELEMENT_TYPE` when the
/// elements' type is not assignable to the loop variable. `void` (reported
/// earlier), `dynamic` and nullable types (the error is the null one) do not
/// report. Returns `_checkForEachParts`'s result
/// (`error_verifier.dart:3104-3210`): no error, with the iterable neither
/// `void` nor nullable and the variable declared.
fn for_in_invalid_type(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    target: &ast::ForInTarget,
    iterable: ExprId,
    t: TypeId,
    written: Option<TypeId>,
    await_: bool,
) -> bool {
    use dartforge_diagnostics::codes::compile_time_error as ce;
    let Some(cls) = (if await_ { inf.core.stream_class } else { inf.core.iterable_class }) else { return false };
    // `for (var (p) in e)`: `_checkForEachParts` does not run (only the parts
    // with a declaration and with an identifier); the shared
    // `analyzePatternForIn` (3.6.2 `type_analyzer.dart:1445-1463`) reports if
    // `asInstanceOf` of the type (without looking at the `?`, a type parameter
    // through its bound) does not find `Iterable`/`Stream`, except `dynamic`
    // and the invalid type; the arguments are the type and always `'Iterable'`
    // (`shared_type_analyzer.dart:161-171`).
    if matches!(target, ast::ForInTarget::Pattern { .. }) {
        if inf.is_dynamic(t) || inf.table.is_invalid(t) || inf.the_is_unknown(t) {
            return false;
        }
        let mut b = t;
        for _ in 0..16 {
            match inf.table.get(b) {
                Type::TypeParameter { param, .. } => {
                    let l = inf.table.param(*param).bound;
                    if l == b {
                        break;
                    }
                    b = l;
                }
                Type::Intersection { bound, .. } => b = *bound,
                _ => break,
            }
        }
        let finds = match inf.table.get(b) {
            Type::Interface { .. } | Type::ExtensionType { .. } => {
                let nn = inf.non_null(b);
                inf.as_instance_of(nn, Some(cls)).is_some()
            }
            _ => false,
        };
        if !finds {
            let sp = inf.span_expr(cx.unit, iterable);
            let tt = inf.table.format(t, inf.interner, inf.program);
            inf.warning_with_code(ce::FOR_IN_OF_INVALID_TYPE, sp, &[&tt, "Iterable"]);
        }
        return false;
    }
    if matches!(inf.table.get(t), Type::Void | Type::Null) || inf.table.get(t).is_declared_nullable() {
        return false;
    }
    if inf.is_dynamic(t) || inf.the_is_unknown(t) {
        return !matches!(target, ast::ForInTarget::Pattern { .. });
    }
    // `resolveToBound`.
    let mut base = t;
    for _ in 0..16 {
        match inf.table.get(base) {
            Type::TypeParameter { param, nullable: false } => {
                let b = inf.table.param(*param).bound;
                if b == base {
                    break;
                }
                base = b;
            }
            Type::Intersection { bound, .. } => base = *bound,
            _ => break,
        }
    }
    let d = inf.core.dynamic_;
    let the_required = inf.iface(Some(cls), vec![d]);
    let the_name = if await_ { "Stream" } else { "Iterable" };
    if inf.is_dynamic(base) || matches!(inf.table.get(base), Type::Void) || base == inf.core.object_nullable {
        return !matches!(target, ast::ForInTarget::Pattern { .. });
    }
    let sp = inf.span_expr(cx.unit, iterable);
    if !inf.assignable(base, the_required) {
        let tt = inf.table.format(base, inf.interner, inf.program);
        inf.warning_with_code(ce::FOR_IN_OF_INVALID_TYPE, sp, &[&tt, the_name]);
        return false;
    }
    // The variable's type: the written one, or that of the existing local in
    // `for (x in …)`.
    let var = match target {
        ast::ForInTarget::Declared { .. } => written,
        ast::ForInTarget::Expression(e) => match &inf.program.unit(cx.unit).ast.expr(*e).kind {
            ExprKind::Identifier(n) => match cx.lookup(n.sym) {
                Some(MemberName::Local(id)) => Some(cx.local(id).ty),
                _ => None,
            },
            _ => None,
        },
        ast::ForInTarget::Pattern { .. } => return false,
    };
    // A variable declared without a written type has the element's type: it
    // fits.
    let Some(var) = var else { return matches!(target, ast::ForInTarget::Declared { .. }) };
    let Some(el) = inf.as_instance_of(base, Some(cls)).map(|a| a[0]) else { return true };
    let tearoff = matches!(inf.table.get(var), Type::Function { .. } | Type::FutureOr { .. })
        && matches!(inf.table.get(el), Type::Interface { .. } | Type::ExtensionType { .. } | Type::TypeParameter { .. } | Type::Intersection { .. });
    if !tearoff && !inf.assignable(el, var) {
        inf.warning_with_args(ce::FOR_IN_OF_INVALID_ELEMENT_TYPE, sp, &[crate::display::Arg::Type_(base), crate::display::Arg::from(the_name), crate::display::Arg::Type_(var)]);
        return false;
    }
    true
}

/// `return e;` / `return;`.
fn return_(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: Option<ExprId>, span: dartforge_diagnostics::Span) {
    let Some(fc) = cx.functions.last().cloned() else {
        if let Some(e) = e {
            infer_free(inf, cx, e);
        }
        return;
    };
    match e {
        Some(e) => {
            let t = infer(inf, cx, e, fc.return_context);
            match fc.the_return {
                Some(_) => {
                    let _ = span;
                    super::functions::verify_return(inf, cx, &fc, e, t);
                }
                None => {
                    if let Some(f) = cx.functions.last_mut() {
                        f.returned_expressions.push((e, t));
                    }
                    let t = if fc.modifier == AsyncModifier::Async { inf.flatten(t) } else { t };
                    if let Some(f) = cx.functions.last_mut() {
                        f.returned.push(t);
                    }
                }
            }
        }
        None => {
            // `RETURN_WITHOUT_VALUE` (`_checkReturnWithoutValue`,
            // `an611:src/error/return_type_verifier.dart:277-295`): in a declared
            // non-generator executable, with a return (the future value, in `async`)
            // that is not `void`, `dynamic` or `Null`.
            if let (Some(t), Some(_)) = (fc.the_return, fc.executable.as_ref()) {
                let generator = matches!(fc.modifier, AsyncModifier::SyncStar | AsyncModifier::AsyncStar);
                if !generator {
                    let tv = if fc.modifier == AsyncModifier::Async { inf.future_value_type(t) } else { t };
                    if !matches!(inf.table.get(tv), Type::Void | Type::Dynamic | Type::Null) {
                        let sp = dartforge_diagnostics::Span { start: span.start, end: span.start + "return".len() };
                        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::RETURN_WITHOUT_VALUE, sp, &[]);
                    }
                }
            }
            if let Some(f) = cx.functions.last_mut() {
                f.empty_return = true;
                if f.executable.is_none() {
                    f.valueless_returns.push(dartforge_diagnostics::Span { start: span.start, end: span.start + "return".len() });
                }
            }
        }
    }
}

// -------------------------------------------------------------------
// Syntactic scan: written variables (assignedIn/capturedIn)
// -------------------------------------------------------------------

pub(crate) enum Part<'a> {
    Expr(ExprId),
    Stmt(StmtId),
    /// The body of a collection `for`/`for-in`.
    Element(&'a ast::CollectionElement),
}

/// A write: the name and the name's offset in the declaration it refers to,
/// when that declaration is inside the scanned region (the innermost one with
/// that name, by lexical scope); `None` when it is from outside (a parameter
/// of the member, an outer variable). Without this, `x = …` in a `test(…)`
/// closure contaminated a same-named `x` of another closure of the same
/// `main`.
pub(crate) type Write = (SymbolId, Option<usize>);

/// The local of `cx` that the write refers to, as seen from the current
/// point.
pub(crate) fn write_local(cx: &Body, e: Write) -> Option<LocalId> {
    let Some(MemberName::Local(id)) = cx.lookup(e.0) else { return None };
    match e.1 {
        Some(o) if cx.local(id).offset != o => None,
        _ => Some(id),
    }
}

/// Locals written in the parts (and those written inside closures in them).
pub(crate) fn writes_in(inf: &BodyInferrer<'_>, cx: &Body, parts: &[Part<'_>]) -> (Vec<LocalId>, Vec<LocalId>) {
    let mut v = Sweep::new(&inf.program.unit(cx.unit).ast);
    for p in parts {
        match p {
            Part::Expr(e) => v.expr(*e, false),
            Part::Stmt(s) => v.stmt(*s, false),
            Part::Element(el) => v.element(el, false),
        }
    }
    let ids = |es: &[Write]| -> Vec<LocalId> {
        let mut out = Vec::new();
        for e in es {
            if let Some(id) = write_local(cx, *e) {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    };
    (ids(&v.outside), ids(&v.inside))
}

/// Writes inside a function (for write capture): its parameters shadow the
/// outer ones in the whole body (they resolve to themselves).
pub(crate) fn names_written_in_function(inf: &BodyInferrer<'_>, unit: UnitId, f: &ast::Function) -> Vec<Write> {
    let mut v = Sweep::new(&inf.program.unit(unit).ast);
    v.scopes.push(Vec::new());
    v.declare_parameters(f);
    v.body(&f.body, false);
    let mut out = v.outside;
    for e in v.inside {
        if !out.contains(&e) {
            out.push(e);
        }
    }
    out
}

/// Writes in a body, separated: `(outside function literals, inside
/// literals)`, the analyzer's `assignedVariables.anywhere.written` and
/// `.captured`.
pub(crate) fn separately_written_names(inf: &BodyInferrer<'_>, unit: UnitId, body: &ast::FunctionBody) -> (Vec<Write>, Vec<Write>) {
    let mut v = Sweep::new(&inf.program.unit(unit).ast);
    v.body(body, false);
    (v.outside, v.inside)
}

/// Syntactic scan of the writes, with the lexical scopes declared inside the
/// region (blocks, `for`, `catch`, `case`, closures and local functions).
struct Sweep<'a> {
    a: &'a ast::Ast,
    scopes: Vec<Vec<(SymbolId, usize)>>,
    outside: Vec<Write>,
    inside: Vec<Write>,
    /// The index of the scope where each closure or local function in progress
    /// starts: writing a variable declared in it is not a capture.
    closures: Vec<usize>,
}

impl<'a> Sweep<'a> {
    fn new(a: &'a ast::Ast) -> Self {
        Sweep { a, scopes: vec![Vec::new()], outside: Vec::new(), inside: Vec::new(), closures: Vec::new() }
    }

    fn declare(&mut self, n: ast::Name) {
        if let Some(e) = self.scopes.last_mut() {
            e.push((n.sym, n.span.start));
        }
    }

    fn declare_parameters(&mut self, f: &ast::Function) {
        for p in f.parameters.iter().flat_map(|ps| ps.iter()) {
            if let Some(n) = p.name {
                self.declare(n);
            }
        }
    }

    fn write(&mut self, n: ast::Name, inside: bool) {
        let finding = self
            .scopes
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, e)| e.iter().rev().find(|(s, _)| *s == n.sym).map(|(_, o)| (i, *o)));
        let decl = finding.map(|(_, o)| o);
        // `AssignedVariables.captured`: only a write in a closure to a variable
        // declared outside it (a local function's parameter written in that same
        // function is an ordinary write).
        let inside = inside && !finding.is_some_and(|(i, _)| self.closures.last().is_some_and(|&c| i >= c));
        let e = (n.sym, decl);
        let v = if inside { &mut self.inside } else { &mut self.outside };
        if !v.contains(&e) {
            v.push(e);
        }
    }

    fn with_scope(&mut self, f: impl FnOnce(&mut Self)) {
        self.scopes.push(Vec::new());
        f(self);
        self.scopes.pop();
    }

    fn body(&mut self, body: &ast::FunctionBody, inside: bool) {
        match body {
            ast::FunctionBody::Block(b) => self.stmt(*b, inside),
            ast::FunctionBody::Expression(x) => self.expr(*x, inside),
            _ => {}
        }
    }

    /// Closure or local function: what it writes counts as written in a closure;
    /// its parameters are its own declarations.
    fn function(&mut self, f: &ast::Function) {
        self.with_scope(|v| {
            v.closures.push(v.scopes.len() - 1);
            v.declare_parameters(f);
            v.body(&f.body, true);
            v.closures.pop();
        });
    }

    fn write_target(&mut self, tgt: ExprId, inside: bool) {
        let mut x = tgt;
        loop {
            match &self.a.expr(x).kind {
                ExprKind::Parenthesized(i) => x = *i,
                ExprKind::Identifier(n) => {
                    let n = *n;
                    self.write(n, inside);
                    return;
                }
                _ => return,
            }
        }
    }

    /// Variables of a pattern: writes (`PatternAssign`) or declarations.
    fn pattern(&mut self, p: ast::PatternId, inside: bool, declares: bool) {
        use ast::PatternKind as P;
        let a = self.a;
        match &a.pattern(p).kind {
            P::Variable { name, var_, final_, ty } => {
                // `var`/`final`/a type in an assignment also declares
                // (`DeclaredVariablePattern`).
                if declares || *var_ || *final_ || ty.is_some() {
                    self.declare(*name);
                } else {
                    self.write(*name, inside);
                }
            }
            P::Or(x, y) | P::And(x, y) => {
                self.pattern(*x, inside, declares);
                self.pattern(*y, inside, declares);
            }
            P::NullCheck(x) | P::NullAssert(x) | P::Parenthesized(x) => self.pattern(*x, inside, declares),
            P::Cast { pattern, .. } => self.pattern(*pattern, inside, declares),
            P::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => {
                            self.pattern(*x, inside, declares)
                        }
                        _ => {}
                    }
                }
            }
            P::Map { entries, .. } => {
                for en in entries.iter() {
                    self.pattern(en.value, inside, declares);
                }
            }
            P::Record { fields } | P::Object { fields, .. } => {
                for f in fields.iter() {
                    self.pattern(f.pattern, inside, declares);
                }
            }
            _ => {}
        }
    }

    fn expr(&mut self, e: ExprId, inside: bool) {
        let a = self.a;
        match &a.expr(e).kind {
            ExprKind::Assign { target, value, .. } => {
                self.write_target(*target, inside);
                self.expr(*target, inside);
                self.expr(*value, inside);
            }
            ExprKind::Unary { op, operand } => {
                if matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec) {
                    self.write_target(*operand, inside);
                }
                self.expr(*operand, inside);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.pattern(*pattern, inside, false);
                self.expr(*value, inside);
            }
            ExprKind::FunctionExpression(f) => self.function(a.function(*f)),
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x, inside),
            ExprKind::Property { target, .. } => self.expr(*target, inside),
            ExprKind::Index { target, index, .. } => {
                self.expr(*target, inside);
                self.expr(*index, inside);
            }
            ExprKind::Call { target, arguments } => {
                self.expr(*target, inside);
                for x in arguments.args.iter() {
                    self.expr(x.value, inside);
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => {
                for x in arguments.args.iter() {
                    self.expr(x.value, inside);
                }
            }
            ExprKind::TypeArguments { target, .. } => self.expr(*target, inside),
            ExprKind::Binary { left, right, .. } => {
                self.expr(*left, inside);
                self.expr(*right, inside);
            }
            ExprKind::Conditional { condition, then, else_ } => {
                self.expr(*condition, inside);
                self.expr(*then, inside);
                self.expr(*else_, inside);
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.expr(*value, inside),
            ExprKind::Cascade { target, sections, .. } => {
                self.expr(*target, inside);
                for s in sections.iter() {
                    self.expr(*s, inside);
                }
            }
            ExprKind::String(lit) => {
                for p in lit.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(*x, inside);
                    }
                }
            }
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    self.element(el, inside);
                }
            }
            ExprKind::Record { positional, named, .. } => {
                for x in positional.iter() {
                    self.expr(*x, inside);
                }
                for (_, x) in named.iter() {
                    self.expr(*x, inside);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value, inside);
                for c in cases.iter() {
                    self.with_scope(|v| {
                        v.pattern(c.pattern, inside, true);
                        if let Some(g) = c.guard {
                            v.expr(g, inside);
                        }
                        v.expr(c.body, inside);
                    });
                }
            }
            _ => {}
        }
    }

    fn element(&mut self, el: &ast::CollectionElement, inside: bool) {
        use ast::CollectionElement as C;
        match el {
            C::Expression(x) | C::NullAwareExpression(x) => self.expr(*x, inside),
            C::MapEntry { key, value, .. } => {
                self.expr(*key, inside);
                self.expr(*value, inside);
            }
            C::Spread { value, .. } => self.expr(*value, inside),
            C::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition, inside);
                self.with_scope(|v| {
                    if let Some(p) = case_pattern {
                        v.pattern(*p, inside, true);
                    }
                    if let Some(g) = guard {
                        v.expr(*g, inside);
                    }
                    v.element(then, inside);
                });
                if let Some(e) = else_ {
                    self.element(e, inside);
                }
            }
            C::For { init, condition, updates, body, .. } => self.with_scope(|v| {
                match init {
                    Some(ast::ForInit::Expression(x)) => v.expr(*x, inside),
                    Some(ast::ForInit::Variables(vl)) => v.variables(vl, inside),
                    Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                        v.expr(*value, inside);
                        v.pattern(*pattern, inside, true);
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    v.expr(*c, inside);
                }
                for u in updates.iter() {
                    v.expr(*u, inside);
                }
                v.element(body, inside);
            }),
            C::ForIn { target, iterable, body, .. } => {
                self.expr(*iterable, inside);
                self.with_scope(|v| {
                    v.for_in_target(target, inside);
                    v.element(body, inside);
                });
            }
        }
    }

    fn variables(&mut self, vl: &ast::VariableList, inside: bool) {
        for x in vl.variables.iter() {
            if let Some(i) = x.initializer {
                self.expr(i, inside);
            }
            self.declare(x.name);
        }
    }

    fn for_in_target(&mut self, target: &ast::ForInTarget, inside: bool) {
        match target {
            ast::ForInTarget::Declared { name, .. } => self.declare(*name),
            ast::ForInTarget::Expression(x) => self.write_target(*x, inside),
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }

    fn stmt(&mut self, s: StmtId, inside: bool) {
        let a = self.a;
        match &a.stmt(s).kind {
            StmtKind::Block(ss) => self.with_scope(|v| {
                for x in ss.iter() {
                    v.stmt(*x, inside);
                }
            }),
            StmtKind::Variables(vl) => self.variables(vl, inside),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.expr(*value, inside);
                self.pattern(*pattern, inside, true);
            }
            StmtKind::Function(f) => {
                let f = a.function(*f);
                if let Some(n) = f.name {
                    self.declare(n);
                }
                self.function(f);
            }
            StmtKind::Expression(e) => self.expr(*e, inside),
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition, inside);
                self.with_scope(|v| {
                    if let Some(p) = case_pattern {
                        v.pattern(*p, inside, true);
                    }
                    if let Some(g) = guard {
                        v.expr(*g, inside);
                    }
                    v.stmt(*then, inside);
                });
                if let Some(e) = else_ {
                    self.stmt(*e, inside);
                }
            }
            StmtKind::For { init, condition, updates, body, .. } => self.with_scope(|v| {
                match init {
                    Some(ast::ForInit::Expression(x)) => v.expr(*x, inside),
                    Some(ast::ForInit::Variables(vl)) => v.variables(vl, inside),
                    Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                        v.expr(*value, inside);
                        v.pattern(*pattern, inside, true);
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    v.expr(*c, inside);
                }
                for u in updates.iter() {
                    v.expr(*u, inside);
                }
                v.stmt(*body, inside);
            }),
            StmtKind::ForIn { target, iterable, body, .. } => {
                self.expr(*iterable, inside);
                self.with_scope(|v| {
                    v.for_in_target(target, inside);
                    v.stmt(*body, inside);
                });
            }
            StmtKind::While { condition, body } | StmtKind::DoWhile { body, condition } => {
                self.expr(*condition, inside);
                self.stmt(*body, inside);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(*value, inside);
                for c in cases.iter() {
                    self.with_scope(|v| {
                        if let Some(g) = c.guard {
                            v.expr(g, inside);
                        }
                        for x in c.body.iter() {
                            v.stmt(*x, inside);
                        }
                    });
                }
            }
            StmtKind::Return(Some(e)) => self.expr(*e, inside),
            StmtKind::Yield { value, .. } => self.expr(*value, inside),
            StmtKind::Try { body, catches, finally_ } => {
                self.stmt(*body, inside);
                for c in catches.iter() {
                    self.with_scope(|v| {
                        if let Some(n) = c.exception {
                            v.declare(n);
                        }
                        if let Some(n) = c.stack_trace {
                            v.declare(n);
                        }
                        v.stmt(c.body, inside);
                    });
                }
                if let Some(f) = finally_ {
                    self.stmt(*f, inside);
                }
            }
            StmtKind::Labeled { body, .. } => self.stmt(*body, inside),
            StmtKind::Assert { condition, message } => {
                self.expr(*condition, inside);
                if let Some(m) = message {
                    self.expr(*m, inside);
                }
            }
            _ => {}
        }
    }
}
