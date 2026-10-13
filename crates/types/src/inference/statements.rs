//! Instruções: blocos, declarações, controle de fluxo (com os modelos de
//! `flow-analysis.md` para `if`, laços, `switch`, `try`, saltos), `return` e
//! `yield` (coletados para a inferência do retorno de closures).

use super::body::{JumpTarget, Body, Local, Nome};
use super::expr::{self, declarar_local, inferir, infer_free};
use super::flow::Fluxo;
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

/// Infere uma instrução.
pub(crate) fn infer_statement(inf: &mut BodyInferrer<'_>, cx: &mut Body, s: StmtId) {
    validate_local_annotations(inf, cx, s);
    let a = &inf.program.unit(cx.unit).ast;
    let st = a.stmt(s);
    let span = st.span;
    match &st.kind {
        StmtKind::Block(stmts) => {
            cx.push_scope();
            // O escopo do bloco contém todas as suas declarações locais.
            declare_forwards(inf, cx, stmts);
            // Sem bloco básico em curso, o trecho vai até a última instrução
            // deste bloco.
            let block_end = stmts.last().map(|&x| inf.program.unit(cx.unit).ast.stmt(x).span.end).unwrap_or(span.end);
            cx.block_ends.push(block_end);
            for &x in stmts.iter() {
                if !cx.flow.alcancavel && cx.dead_code.is_none() {
                    let sx = inf.program.unit(cx.unit).ast.stmt(x).span;
                    let end = cx.flow_ends.last().copied().unwrap_or(block_end);
                    inf.aviso(DEAD_CODE.template.to_string(), Span { start: sx.start, end: end.max(sx.end) });
                    cx.dead_code = Some(cx.flow_ends.len());
                    cx.dead_origin = Some(s);
                }
                infer_statement(inf, cx, x);
            }
            cx.block_ends.pop();
            cx.pop_scope();
        }
        StmtKind::Variables(vl) => variable_declaration(inf, cx, vl),
        StmtKind::PatternVariables { final_, pattern, value } => patterns::declaracao(inf, cx, *final_, *pattern, *value),
        StmtKind::Function(f) => functions::funcao_local(inf, cx, *f),
        StmtKind::Expression(e) => {
            infer_free(inf, cx, *e);
        }
        StmtKind::If { condition, case_pattern, guard, then, else_ } => {
            let before = cx.flow.clone();
            let (vf, ff) = match case_pattern {
                Some(p) => {
                    let t = infer_free(inf, cx, *condition);
                    cx.push_scope();
                    patterns::caso(inf, cx, *p, t, *guard, Some(*condition))
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
            cx.flow = inf.juntar(&after_then, &after_else);
        }
        StmtKind::While { condition, body } => {
            let the_labels = pending_labels(cx);
            let (writes, captured) = writes_in(inf, cx, &[Parte::Expr(*condition), Parte::Stmt(*body)]);
            cx.flow.conservative_join(&writes, &captured);
            let (vf, ff) = expr::checked_condition(inf, cx, *condition);
            cx.saltos.push(JumpTarget { rotulos: the_labels, laco: true, e_switch: false, breaks: Vec::new(), continues: Vec::new() });
            cx.flow = vf;
            flow_branch(inf, cx, *body);
            let tgt = cx.saltos.pop().unwrap();
            let mut exits = vec![ff];
            exits.extend(tgt.breaks);
            let base = cx.flow.clone();
            cx.flow = inf.join_all(&base, &exits);
        }
        StmtKind::DoWhile { body, condition } => {
            let the_labels = pending_labels(cx);
            let (writes, captured) = writes_in(inf, cx, &[Parte::Expr(*condition), Parte::Stmt(*body)]);
            cx.flow.conservative_join(&writes, &captured);
            cx.saltos.push(JumpTarget { rotulos: the_labels, laco: true, e_switch: false, breaks: Vec::new(), continues: Vec::new() });
            branch(inf, cx, *body);
            let tgt = cx.saltos.pop().unwrap();
            let mut the_conts = vec![cx.flow.clone()];
            the_conts.extend(tgt.continues);
            let base = cx.flow.clone();
            cx.flow = inf.join_all(&base, &the_conts);
            // A condição é o primeiro nó morto (o corpo não chega ao fim nem
            // continua): o `flowEnd` com pai `DoStatement`
            // (`dead_code_verifier.dart:281-301`) relata o `do` (até o `{` do
            // corpo bloco) e do `while` (do `}` do corpo bloco) ao `;`; o resto
            // começa no token depois do `;`, e não sai se o laço tem `break`
            // (`hasBreakStatement`).
            if !cx.flow.alcancavel && cx.dead_code.is_none() {
                let ast_ = &inf.program.unit(cx.unit).ast;
                let the_body = ast_.stmt(*body);
                let block = matches!(the_body.kind, StmtKind::Block(_));
                let body_span = the_body.span;
                let source = inf.program.unit(cx.unit).source.as_bytes();
                let of_end = if block { body_span.start + 1 } else { span.start + 2 };
                let while_start = if block { body_span.end.saturating_sub(1) } else { next_token(source, body_span.end) };
                inf.aviso(DEAD_CODE.template.to_string(), Span { start: span.start, end: of_end });
                inf.aviso(DEAD_CODE.template.to_string(), Span { start: while_start, end: span.end });
                if tgt.breaks.is_empty() {
                    let begin = next_token(source, span.end);
                    let end = cx.flow_ends.last().copied().or(cx.block_ends.last().copied()).unwrap_or(span.end);
                    if end > begin {
                        inf.aviso(DEAD_CODE.template.to_string(), Span { start: begin, end: end });
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
            // Inicialização que não completa (`for (var i = throw 0; c; u)`):
            // o primeiro nó morto é a condição, e o `flowEnd` das partes vai
            // dela ao fim da última atualização (`node = parent.updaters.last`,
            // `dead_code_verifier.dart:309-310`); sem atualizações o
            // `updaters.last` lança e nada sai. O resto do laço fica no mesmo
            // trecho.
            let dead_at_start = !cx.flow.alcancavel && cx.dead_code.is_none();
            if dead_at_start {
                let a = &inf.program.unit(cx.unit).ast;
                if let (Some(c), Some(u)) = (condition, updates.last()) {
                    let sp = Span { start: a.expr(*c).span.start, end: a.expr(*u).span.end };
                    inf.aviso(DEAD_CODE.template.to_string(), sp);
                }
                cx.dead_code = Some(usize::MAX);
            }
            let mut parts = vec![Parte::Stmt(*body)];
            if let Some(c) = condition {
                parts.push(Parte::Expr(*c));
            }
            for u in updates.iter() {
                parts.push(Parte::Expr(*u));
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
            cx.saltos.push(JumpTarget { rotulos: the_labels, laco: true, e_switch: false, breaks: Vec::new(), continues: Vec::new() });
            cx.flow = vf;
            let dead_before = cx.dead_code.is_some();
            cx.dead_origin = None;
            flow_branch(inf, cx, *body);
            // `_reportForUpdaters`: o trecho morto começou no corpo (ou numa
            // instrução direta dele) e terminou nele — as atualizações também
            // são código morto.
            if !dead_before && cx.dead_code.is_none() && cx.dead_origin == Some(*body) {
                let a = &inf.program.unit(cx.unit).ast;
                if let (Some(p), Some(u)) = (updates.first(), updates.last()) {
                    let sp = Span { start: a.expr(*p).span.start, end: a.expr(*u).span.end };
                    inf.aviso(DEAD_CODE.template.to_string(), sp);
                }
            }
            let tgt = cx.saltos.pop().unwrap();
            let mut the_conts = vec![cx.flow.clone()];
            the_conts.extend(tgt.continues);
            let base = cx.flow.clone();
            cx.flow = inf.join_all(&base, &the_conts);
            // Atualizações inalcançáveis: o trecho delas é o do
            // `_reportForUpdaters` (ou nenhum); nada de dentro abre outro.
            let close = !cx.flow.alcancavel && cx.dead_code.is_none();
            if close {
                // Na profundidade corrente: o ramo de um `?:` ou de um `&&`
                // dentro das atualizações entra um nível abaixo, e a saída
                // dele não fecha este trecho (o `usize::MAX` fechava, e o
                // ramo morto de dentro abria outro relato).
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
            // Sem atualizações, o `updaters.last` do analyzer lança e o
            // verificador de código morto não segue no corpo: o trecho fica
            // aberto até o fim dele.
            if dead_at_start {
                // Profundidade 0: nenhum `sair_fluxo` o fecha.
                cx.dead_code = if condition.is_some() && updates.is_empty() { Some(0) } else { None };
            }
            cx.pop_scope();
        }
        StmtKind::ForIn { await_, target, iterable, body } => {
            let the_labels = pending_labels(cx);
            cx.push_scope();
            let before_body = {
                let reachable_before = cx.flow.alcancavel;
                let write = for_in_header(inf, cx, target, *iterable, *await_);
                // Iterável que não completa (`Never`): o primeiro nó morto é a
                // variável do laço (o `DeclaredIdentifier`, depois do
                // iterável na ordem do fluxo), e o trecho vai dela ao fim do
                // bloco básico.
                if reachable_before && !cx.flow.alcancavel && cx.dead_code.is_none() {
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
                    inf.aviso(DEAD_CODE.template.to_string(), Span { start: i, end: end });
                    cx.dead_code = Some(cx.flow_ends.len());
                }
                let (writes, captured) = writes_in(inf, cx, &[Parte::Stmt(*body)]);
                cx.flow.conservative_join(&writes, &captured);
                let before = cx.flow.clone();
                if let Some(x) = write {
                    x.aplicar(inf, cx);
                }
                before
            };
            cx.saltos.push(JumpTarget { rotulos: the_labels, laco: true, e_switch: false, breaks: Vec::new(), continues: Vec::new() });
            flow_branch(inf, cx, *body);
            let tgt = cx.saltos.pop().unwrap();
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
            // Todos os casos casam a mesma referência do valor (T6).
            let outer_scrutinee = cx.switch_scrutinee.replace(None);
            cx.saltos.push(JumpTarget { rotulos: the_labels, laco: false, e_switch: true, breaks: Vec::new(), continues: Vec::new() });
            let mut has_default_value = false;
            let mut i = 0;
            // O índice do grupo de casos (os que dividem o corpo contam um).
            let mut group = 0usize;
            let n = cases.len();
            let mut exits: Vec<Fluxo> = Vec::new();
            while i < n {
                // Casos agrupados: os vazios seguem para o próximo corpo.
                let mut j = i;
                let mut the_entries: Vec<Fluxo> = Vec::new();
                cx.push_scope();
                let first_local = cx.locais.len();
                // As variáveis de cada membro do grupo (o `addAll` do
                // `_SharedCaseScope`), e se há rótulo ou `default`.
                let mut members: Vec<Vec<(SymbolId, LocalId)>> = Vec::new();
                let mut with_label = false;
                let mut with_default = false;
                loop {
                    let c = &cases[j];
                    if !c.labels.is_empty() {
                        with_label = true;
                    }
                    cx.flow = not_matched.clone();
                    // A cabeça de cada `case`/`default` é um bloco básico
                    // (`handleSwitchBeforeAlternative` e o `flowEnd` de
                    // `resolver.dart:1083`/`:1098`): se nenhum valor chega a
                    // ela, o trecho morto é só a palavra-chave, e o padrão e
                    // a guarda não abrem outro (Apêndice B do §B).
                    let pattern_flow = patterns::pattern_flow_enabled(inf);
                    if pattern_flow {
                        enter_flow(cx, c.span.end);
                        if !cx.flow.alcancavel && cx.dead_code.is_none() {
                            let keyword = case_keyword(inf, cx, c);
                            inf.aviso(DEAD_CODE.template.to_string(), keyword);
                            cx.dead_code = Some(cx.flow_ends.len());
                        }
                    }
                    match c.pattern {
                        Some(p) => {
                            let (vf, ff) = patterns::caso(inf, cx, p, t, c.guard, Some(*value));
                            the_entries.push(vf);
                            not_matched = ff;
                            let range = cx.last_pattern_locals.clone();
                            members.push(
                                range
                                    .filter(|&k| cx.locais[k].offset != 0)
                                    .map(|k| (cx.locais[k].nome, LocalId(k as u32)))
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
                // A variável de junção (a que o corpo enxerga, a cópia do
                // último caso) tem, em cada alternativa, o estado da cópia
                // daquela alternativa: a junção dos fluxos guarda a promoção
                // comum a todas (`switchStatement_endAlternatives` junta as
                // variáveis de junção, `case int? x?: case int? x?:` → `int`).
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
                                if let Some(m) = the_entries[k].modelo(idk).cloned() {
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
                // Grupo com rótulo (`switchStatement_endAlternatives` com
                // `hasLabels`, 3.6.2 `flow_analysis.dart:5421-5425`): um
                // `continue L` pode chegar de qualquer ponto do `switch`; o
                // corpo começa do estado depois do escrutínio, com a junção
                // conservadora das variáveis escritas ou capturadas no
                // `switch` inteiro (a escrita depois do rótulo despromove).
                if with_label {
                    let (writes, captured) = writes_in(inf, cx, &[Parte::Stmt(s)]);
                    let mut f = after_value.clone();
                    f.conservative_join(&writes, &captured);
                    cx.flow = f;
                }
                // Casos que dividem o corpo: as variáveis dos padrões são
                // variáveis de junção, atribuídas em qualquer caminho que
                // chegue ao corpo (cada `case` declarou a sua cópia; a
                // junção dos fluxos, sozinha, as via como não atribuídas).
                if j > i {
                    for id in first_local..cx.locais.len() {
                        let id = LocalId(id as u32);
                        if cx.flow.modelo(id).is_none() {
                            cx.flow.declarar(id);
                        }
                        cx.flow.inicializar(id);
                    }
                }
                if with_label {
                    members.push(Vec::new());
                }
                group_joins(inf, cx, &members, with_label || with_default);
                let the_body: Vec<StmtId> = cases[j].body.to_vec();
                // O corpo do grupo é um bloco básico (`resolver.dart:1126`):
                // o trecho morto vai da primeira instrução inalcançável ao
                // fim da última instrução do corpo.
                let pattern_flow = patterns::pattern_flow_enabled(inf);
                let body_end = the_body.last().map(|&x| inf.program.unit(cx.unit).ast.stmt(x).span.end);
                if pattern_flow {
                    if let Some(end) = body_end {
                        enter_flow(cx, end);
                    }
                }
                // As instruções do membro formam um escopo local com as
                // declarações delas (o `LocalScope` das `statements` do
                // `SwitchMember`): usar antes de declarar é
                // `REFERENCED_BEFORE_DECLARATION`.
                declare_forwards(inf, cx, &the_body);
                for s in the_body {
                    if pattern_flow && !cx.flow.alcancavel && cx.dead_code.is_none() {
                        if let Some(end) = body_end {
                            let start = inf.program.unit(cx.unit).ast.stmt(s).span.start;
                            inf.aviso(DEAD_CODE.template.to_string(), Span { start: start, end: end });
                            cx.dead_code = Some(cx.flow_ends.len());
                        }
                    }
                    infer_statement(inf, cx, s);
                }
                if pattern_flow && body_end.is_some() {
                    exit_flow(cx);
                }
                // `switchCaseCompletesNormally` (`shared_type_analyzer.dart:232-239`,
                // `analyzeSwitchStatement`): sem padrões (< 3.0), o corpo de um
                // grupo que não é o último não pode chegar ao fim; na palavra
                // do primeiro membro do grupo.
                // O analisador do 3.6.2 recebe o índice do GRUPO e o usa na lista
                // de membros (`node.members[caseIndex].keyword`,
                // `shared_type_analyzer.dart:232-239`): com casos que dividem o
                // corpo antes, o relato cai num membro anterior.
                let version = inf.program.library(cx.lib).features.versao();
                if cx.flow.alcancavel && j + 1 < n && version < dartforge_frontend::features::LanguageVersion::new(3, 0) {
                    let keyword = case_keyword(inf, cx, &cases[group]);
                    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::SWITCH_CASE_COMPLETES_NORMALLY, keyword, &[]);
                }
                if cx.flow.alcancavel {
                    exits.push(cx.flow.clone());
                }
                cx.pop_scope();
                i = j + 1;
                group += 1;
            }
            let tgt = cx.saltos.pop().unwrap();
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
            cx.flow.alcancavel = false;
        }
        StmtKind::Continue(label) => {
            let f = cx.flow.clone();
            if let Some(tgt) = jump_target(cx, label.map(|n| n.sym), true) {
                tgt.continues.push(f);
            }
            cx.flow.alcancavel = false;
        }
        StmtKind::Return(e) => {
            retorno(inf, cx, *e, span);
            cx.flow.alcancavel = false;
        }
        StmtKind::Yield { star, value } => {
            let (m, k) = match cx.funcoes.last() {
                Some(f) => (f.modificador, f.return_context),
                None => (AsyncModifier::None, inf.core.unknown),
            };
            // O tipo de retorno declarado do gerador (`imposedType`).
            let declared = cx.funcoes.last().and_then(|f| f.retorno.filter(|_| f.executavel.is_some()).or(f.closure_imposed));
            if *star {
                let ctx = if m == AsyncModifier::AsyncStar { inf.flow_of(k) } else { inf.iterable(k) };
                let t = inferir(inf, cx, *value, ctx);
                expr::void_use(inf, cx, *value, t);
                // `YieldStatementResolver._resolve_generator` (`:171-175`).
                if matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
                    expr::nullable_dereference(inf, cx, *value, t, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_USE_OF_NULLABLE_VALUE_IN_YIELD_EACH);
                }
                invalid_yield(inf, cx, *value, t, declared, true, m);
                let cls = if m == AsyncModifier::AsyncStar { inf.core.stream_class } else { inf.core.iterable_class };
                let el = if inf.e_dynamic(t) { inf.core.dynamic_ } else { inf.as_instance_of(t, cls).map(|a| a[0]).unwrap_or(inf.core.dynamic_) };
                if let Some(f) = cx.funcoes.last_mut() {
                    f.returned.push(el);
                }
            } else {
                let t = inferir(inf, cx, *value, k);
                expr::void_use(inf, cx, *value, t);
                invalid_yield(inf, cx, *value, t, declared, false, m);
                if let Some(f) = cx.funcoes.last_mut() {
                    f.returned.push(t);
                }
            }
        }
        StmtKind::Try { body, catches, finally_ } => {
            let before = cx.flow.clone();
            let (writes, captured) = writes_in(inf, cx, &[Parte::Stmt(*body)]);
            flow_branch(inf, cx, *body);
            let after_try = cx.flow.clone();
            let mut exits = vec![after_try];
            // `_CatchClausesVerifier` (`dead_code_verifier.dart:495-545`): os
            // tipos `on` já vistos, e se algum `catch` já foi dado por morto.
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
                    use dartforge_diagnostics::codigos::warning as w;
                    // `on T` com `T` potencialmente anulável (e não inválido):
                    // não se lança um valor anulável.
                    if let Some(t) = c.on_type {
                        if !inf.table.e_invalido(ex_type) && !inf.is_non_nullable(ex_type) {
                            let sp = inf.program.unit(cx.unit).ast.ty(t).span;
                            inf.warning_with_code(w::NULLABLE_TYPE_IN_CATCH_CLAUSE, sp, &[]);
                        }
                    }
                    if !dead_catch {
                        let last = catches.len() - 1;
                        let end = catches[last].span.end;
                        if c.on_type.is_none() || ex_type == inf.core.object {
                            // Um `catch` que pega tudo: os seguintes são mortos.
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
                                        &[crate::exibicao::Arg::Tipo(ex_type), crate::exibicao::Arg::Tipo(s)],
                                    );
                                    dead_catch = true;
                                }
                                None => catch_types.push(ex_type),
                            }
                        }
                    }
                }
                if let Some(n) = &c.exception {
                    declarar_local(inf, cx, Local { nome: n.sym, tipo: ex_type, final_: true, late: false, const_: false, offset: n.span.start, funcao_local: false }, true);
                }
                if let Some(n) = &c.stack_trace {
                    let st = stack_trace(inf);
                    declarar_local(inf, cx, Local { nome: n.sym, tipo: st, final_: true, late: false, const_: false, offset: n.span.start, funcao_local: false }, true);
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
                let mut f = inf.juntar(&after_catches, &before);
                f.conservative_join(&writes, &captured);
                let finally_start = f.clone();
                cx.flow = f;
                branch(inf, cx, *fin);
                if cx.flow.alcancavel {
                    // O `finally` completa (`attachFinally`): vale o estado
                    // depois do `try`/`catch`, com as promoções do `finally`
                    // reaplicadas (`o as int;` no `finally` promove depois),
                    // e o estado do `finally` para o que ele escreveu.
                    let end = cx.flow.clone();
                    let reaches = after_catches.alcancavel;
                    let mut r = inf.reapply(&after_catches, &end);
                    for (i, m) in end.vars.iter().enumerate() {
                        let v0 = finally_start.vars.get(i).and_then(|m| m.as_ref()).map(|m| m.versao);
                        if m.is_some() && m.as_ref().map(|m| m.versao) != v0 {
                            if r.vars.len() <= i {
                                r.vars.resize(i + 1, None);
                            }
                            r.vars[i] = m.clone();
                        }
                    }
                    r.alcancavel = reaches;
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
                cx.saltos.push(JumpTarget { rotulos: ls, laco: false, e_switch: false, breaks: Vec::new(), continues: Vec::new() });
                infer_statement(inf, cx, *body);
                let tgt = cx.saltos.pop().unwrap();
                let mut exits = vec![cx.flow.clone()];
                exits.extend(tgt.breaks);
                let base = cx.flow.clone();
                cx.flow = inf.join_all(&base, &exits);
            }
        }
        StmtKind::Assert { condition, message } => {
            let before = cx.flow.clone();
            expr::assert_condition(inf, cx, *condition);
            if let Some(m) = message {
                infer_free(inf, cx, *m);
            }
            cx.flow = before;
        }
        StmtKind::Empty => {}
    }
}

/// Pré-declara (como "adiante") os locais de uma lista de instruções:
/// variáveis, funções locais e as variáveis das declarações de padrão.
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
                let refutable = std::mem::replace(&mut cx.padrao_refutavel, false);
                let mut names = Vec::new();
                patterns::declared_variables(inf, cx, *pattern, &mut names);
                cx.padrao_refutavel = refutable;
                for n in names {
                    cx.declare_forward(n.sym, n.span);
                }
            }
            _ => {}
        }
    }
}

/// Um ramo (`then`, `else`, corpo de laço) em escopo próprio.
fn branch(inf: &mut BodyInferrer<'_>, cx: &mut Body, s: StmtId) {
    cx.push_scope();
    infer_statement(inf, cx, s);
    cx.pop_scope();
}

/// Um ramo que fecha um bloco básico (`flowEnd` do analyzer: `then`/`else`,
/// corpo de `while`/`for`/`for-in`, corpo do `try`): se ele mesmo é
/// inalcançável, o trecho morto é a instrução inteira (`if (false) { … }`
/// relata do `{` ao `}`).
fn flow_branch(inf: &mut BodyInferrer<'_>, cx: &mut Body, s: StmtId) {
    let span = inf.program.unit(cx.unit).ast.stmt(s).span;
    if !cx.flow.alcancavel && cx.dead_code.is_none() {
        inf.aviso(DEAD_CODE.template.to_string(), span);
        cx.dead_code = Some(cx.flow_ends.len() + 1);
        cx.dead_origin = Some(s);
    }
    let end = flow_end(inf, cx, s);
    enter_flow(cx, end);
    branch(inf, cx, s);
    exit_flow(cx);
}

/// O fim de um bloco básico que termina em `s`, aparado como o analyzer:
/// num bloco com instruções, o fim da última.
pub(crate) fn flow_end(inf: &BodyInferrer<'_>, cx: &Body, s: StmtId) -> usize {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.stmt(s).kind {
        StmtKind::Block(stmts) if !stmts.is_empty() => a.stmt(*stmts.last().unwrap()).span.end,
        _ => a.stmt(s).span.end,
    }
}

/// Entra num bloco básico que termina em `fim`.
pub(crate) fn enter_flow(cx: &mut Body, end: usize) {
    cx.flow_ends.push(end);
}

/// Sai do bloco básico: um trecho morto começado nele termina aqui.
pub(crate) fn exit_flow(cx: &mut Body) {
    let depth = cx.flow_ends.len();
    cx.flow_ends.pop();
    if cx.dead_code.is_some_and(|p| p >= depth) {
        cx.dead_code = None;
    }
}

/// A palavra `case` ou `default` de um membro de `switch`, depois dos
/// rótulos (`rotulo: case 1:`).
/// `switchStatementSharedCaseScopeFinish` (`variable_bindings.dart:177-202`) e
/// `finishJoinedPatternVariable` (`resolver.dart:878-915`): as variáveis de
/// um grupo de casos que divide o corpo. Uma variável que não está em todos
/// os membros (um `default` ou um rótulo conta como membro vazio) é
/// `…_HAS_LABEL` com rótulo ou `default`, senão `…_NOT_ALL_CASES`; presente em
/// todos, mas com tipos ou `final` diferentes, `…_DIFFERENT_FINALITY_OR_TYPE`.
/// O código vale para cada referência no corpo.
fn group_joins(inf: &mut BodyInferrer<'_>, cx: &mut Body, members: &[Vec<(SymbolId, LocalId)>], with_label: bool) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
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
            let typ = inf.table.canonico(first.tipo);
            let different = ids[1..].iter().any(|&id| {
                let l = cx.local(id);
                inf.table.canonico(l.tipo) != typ || l.final_ != first.final_
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

/// O início do primeiro token em `fonte` a partir de `de` (pula espaços e
/// comentários).
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
    for tgt in cx.saltos.iter_mut().rev() {
        match label {
            Some(r) => {
                if tgt.rotulos.contains(&r) {
                    return Some(tgt);
                }
            }
            None => {
                if tgt.laco || (!continue_ && tgt.e_switch) {
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

/// O `isExhaustive` do `analyzeSwitchStatement`
/// (`_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart:2017-2027`)
/// de um `switch` sem `default`: com padrões (linguagem 3.0 ou mais), o
/// `isAlwaysExhaustiveType` do tipo do escrutínio; antes, o
/// `SwitchExhaustiveness` legado (`resolver.dart:5469-5557`): um enum cujas
/// constantes aparecem todas em `case` sem guarda (constante, entre
/// parênteses ou não), com o `null` coberto pelo tipo não anulável ou por um
/// `case null`.
fn exhaustive_switch(inf: &mut BodyInferrer<'_>, cx: &Body, t: TypeId, the_cases: &[ast::SwitchCase]) -> bool {
    if inf.program.library(cx.lib).features.versao().major >= 3 {
        return always_exhaustive(inf, t, 0);
    }
    let Type::Interface { class, nullable, .. } = inf.table.get(t).clone() else { return false };
    let k = inf.program.class(class);
    if k.kind != dartforge_elements::model::ClassKind::Enum {
        return false;
    }
    let mut constantes: Vec<dartforge_elements::model::VariableId> = k.enum_constants.clone();
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
        // `_referencedElement`: o getter da constante (a variável dele).
        let variable = match the_body.get_resolved(e) {
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => Some(*v),
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) => inf.program.function(*f).variable,
            Some(Resolved::Element(dartforge_elements::model::Element::Variable(v))) => Some(*v),
            _ => None,
        };
        if let Some(v) = variable {
            constantes.retain(|x| *x != v);
        }
        if matches!(a.expr(e).kind, ast::ExprKind::Null) {
            covered_null = true;
        }
        if constantes.is_empty() && covered_null {
            exhaustive = true;
        }
    }
    exhaustive
}

/// `TypeSystemImpl.isAlwaysExhaustive` (analyzer 7.7.1,
/// `type_system.dart:819`): o `?` não conta (o `S?` de uma classe `sealed`
/// é sempre exaustivo — `case var y?` e `case null` o cobrem); `Null`,
/// `bool`, enum e classe `sealed`; tipo de extensão pela *erasure*;
/// `FutureOr<T>` pelo `T`; parâmetro de tipo pelo limite promovido ou pelo
/// declarado; record quando todos os campos são.
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
/// 72-140`): o valor de `yield` não cabe no elemento do retorno declarado
/// (`YIELD_OF_INVALID_TYPE`), ou o de `yield*` não cabe no retorno
/// (`YIELD_EACH_OF_INVALID_TYPE`), na expressão.
fn invalid_yield(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, t: TypeId, r: Option<TypeId>, the_star: bool, m: AsyncModifier) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    if !matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
        return;
    }
    let sp = inf.span_expr(cx.unit, e);
    let cls = if m == AsyncModifier::AsyncStar { inf.core.stream_class } else { inf.core.iterable_class };
    if the_star {
        if let Some(r) = r.filter(|&r| !inf.atribuivel(t, r)) {
            inf.warning_with_args(c::YIELD_EACH_OF_INVALID_TYPE, sp, &[crate::exibicao::Arg::Tipo(t), crate::exibicao::Arg::Tipo(r)]);
            return;
        }
        let d = inf.core.dynamic_;
        let req = if m == AsyncModifier::AsyncStar { inf.flow_of(d) } else { inf.iterable(d) };
        if !inf.atribuivel(t, req) {
            inf.warning_with_args(c::YIELD_EACH_OF_INVALID_TYPE, sp, &[crate::exibicao::Arg::Tipo(t), crate::exibicao::Arg::Tipo(req)]);
        }
    } else if let Some(args) = r.and_then(|r| inf.as_instance_of(r, cls)) {
        let v = args[0];
        if !inf.atribuivel(t, v) {
            inf.warning_with_args(c::YIELD_OF_INVALID_TYPE, sp, &[crate::exibicao::Arg::Tipo(t), crate::exibicao::Arg::Tipo(v)]);
        }
    }
}

/// O inicializador de uma local `late`: o fluxo o trata como um literal de
/// função (`lateInitializer_begin`/`_end` são os de `functionExpression`):
/// lá dentro, as variáveis escritas em qualquer lugar do membro não estão
/// definitivamente não atribuídas; depois dele, o fluxo é o de antes, e o
/// que ele escreve conta como escrita capturada.
fn late_initializer(inf: &mut BodyInferrer<'_>, cx: &mut Body, init: ExprId, ctx: TypeId) -> TypeId {
    let before = cx.flow.clone();
    let mut inside = before.clone();
    if cx.written_in_body.is_none() {
        let a = &inf.program.unit(cx.unit).ast;
        cx.written_in_body = Some(match cx.raiz {
            super::body::Raiz::Funcao(f) => separately_written_names(inf, cx.unit, &a.function(f).body),
            super::body::Raiz::Construtor(m) => match &a.member(m).kind {
                ast::MemberKind::Constructor(c) => separately_written_names(inf, cx.unit, &c.body),
                _ => Default::default(),
            },
            super::body::Raiz::Nada => Default::default(),
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
    let t = inferir(inf, cx, init, ctx);
    // O que o inicializador escreve, sintaticamente.
    let mut v = Sweep::nova(&inf.program.unit(cx.unit).ast);
    v.expr(init, false);
    let writes: Vec<Escrita> = v.outside.into_iter().chain(v.dentro).collect();
    cx.flow = before;
    for e in writes {
        if let Some(id) = write_local(cx, e) {
            cx.flow.capture(id);
        }
    }
    t
}

/// Declaração de variáveis locais (`var`, `final`, tipadas, `late`, `const`).
pub(crate) fn variable_declaration(inf: &mut BodyInferrer<'_>, cx: &mut Body, vl: &ast::VariableList) {
    let declared = vl.ty.map(|t| inf.type_from_annotation(cx, t));
    let u = inf.core.unknown;
    for v in vl.variables.iter() {
        let mut typ = declared;
        let mut written = None;
        let mut saved_condition = None;
        if let Some(init) = v.initializer {
            // Inicializador que é condição (`x != null && …`): os modelos
            // verdadeiro/falso ficam guardados na variável (§7.10); `late`
            // nunca guarda.
            let t = if vl.late {
                late_initializer(inf, cx, init, declared.unwrap_or(u))
            } else if expr::is_condition_form(inf, cx, init) {
                let (yes, not_) = expr::condicao(inf, cx, init);
                cx.flow = inf.juntar(&yes, &not_);
                saved_condition = Some((yes, not_));
                inf.body_types.units[cx.unit.0 as usize].get_type(init).unwrap_or(inf.core.bool_)
            } else {
                inferir(inf, cx, init, declared.unwrap_or(u))
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
            // (`dead_code_verifier.dart:114-128`): `late` local curinga (3.7)
            // com inicializador, que nunca é avaliado.
            if vl.late && cx.curinga == Some(v.name.sym) {
                let sp = inf.span_expr(cx.unit, init);
                inf.warning_with_code(dartforge_diagnostics::codigos::warning::DEAD_CODE_LATE_WILDCARD_VARIABLE_INITIALIZER, sp, &[]);
            }
        }
        let typ = typ.unwrap_or(inf.core.dynamic_);
        let id = declarar_local(
            inf,
            cx,
            Local { nome: v.name.sym, tipo: typ, final_: vl.final_ || vl.const_, late: vl.late, const_: vl.const_, offset: v.name.span.start, funcao_local: false },
            false,
        );
        match written {
            Some(t) if !vl.late => {
                let toi = declared.is_some() && !vl.final_;
                let mut f = std::mem::replace(&mut cx.flow, Fluxo::alcancavel());
                inf.write_flow(&mut f, id, typ, t, toi, None);
                cx.flow = f;
            }
            Some(_) => cx.flow.inicializar(id),
            None => {}
        }
        // `_initialize` (`flow_analysis.dart:5957-5962`): antes da 2.15
        // (`respectImplicitlyTypedVarInitializers`, o `constructor-tearoffs`),
        // a variável sem tipo escrito ignora a informação do inicializador
        // (o comportamento de dart-lang/language#1785).
        let library_version = inf.program.library(cx.lib).features.versao();
        let respects_initializer = declared.is_some() || (library_version.major, library_version.minor) >= (2, 15);
        if respects_initializer
            && let (Some((yes, not_)), Some(version)) = (saved_condition, cx.flow.versao(id))
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
            Some(e) => inf.aviso(format!("{}: {}", CONST_EVAL_THROWS_EXCEPTION.template, e), sp),
            None if !inf.is_constant(cx, init) => inf.aviso(CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE.template.to_string(), sp),
            None => {}
        }
    }
}

/// Inicialização de um `for` clássico.
pub(crate) fn for_initialization(inf: &mut BodyInferrer<'_>, cx: &mut Body, init: &ast::ForInit) {
    match init {
        ast::ForInit::Variables(vl) => variable_declaration(inf, cx, vl),
        ast::ForInit::Expression(e) => {
            infer_free(inf, cx, *e);
        }
        ast::ForInit::Pattern { final_, pattern, value } => patterns::declaracao(inf, cx, *final_, *pattern, *value),
    }
}

/// A escrita da variável existente de um `for (x in e)`, adiada para depois
/// do `forEach_bodyBegin` (a junção conservadora do laço e o estado
/// anterior ao corpo, que entra no `forEach_end`).
pub(crate) struct LoopWrite {
    pub(crate) local: LocalId,
    declarado: TypeId,
    written: TypeId,
    motivo: super::flow::NonPromotionReason,
}

impl LoopWrite {
    /// O `flow.write` do identificador (`for_resolver.dart:187-191`).
    pub(crate) fn aplicar(self, inf: &mut BodyInferrer<'_>, cx: &mut Body) {
        let mut f = std::mem::replace(&mut cx.flow, Fluxo::alcancavel());
        inf.assign_flow(&mut f, self.local, self.declarado, self.written, Some(self.motivo));
        cx.flow = f;
    }
}

/// Cabeçalho de `for-in` (instrução ou elemento de coleção): infere o
/// iterável e declara a variável do laço. A atribuição a uma variável
/// existente volta para quem chama, que a aplica depois da junção
/// conservadora (`forEach_bodyBegin`).
pub(crate) fn for_in_header(inf: &mut BodyInferrer<'_>, cx: &mut Body, target: &ast::ForInTarget, iterable: ExprId, await_: bool) -> Option<LoopWrite> {
    let cls = if await_ { inf.core.stream_class } else { inf.core.iterable_class };
    // Contexto do iterável: `Iterable<T>` com o tipo escrito da variável.
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
    let t = inferir(inf, cx, iterable, ctx);
    expr::void_use(inf, cx, iterable, t);
    // `ForResolver._forEachParts` (`for_resolver.dart:164-169`).
    expr::nullable_dereference(inf, cx, iterable, t, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_USE_OF_NULLABLE_VALUE_AS_ITERATOR);
    let valid_parts = for_in_invalid_type(inf, cx, target, iterable, t, written, await_);
    // `visitForEachPartsWithDeclaration` (`error_verifier.dart:912-921`): com
    // o `_checkForEachParts` sem erro, o `const` da variável.
    if valid_parts
        && let ast::ForInTarget::Declared { const_: Some(keyword), .. } = target
    {
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::FOR_IN_WITH_CONST_VARIABLE, *keyword, &[]);
    }
    // `_computeForEachElementType` (`for_resolver.dart:92-116`): iterável
    // `dynamic` dá `dynamic`; o que não é `Iterable`/`Stream` (inclusive o
    // próprio `InvalidType`) dá `InvalidType`, e os usos do elemento não
    // relatam mais nada.
    let el = if inf.table.e_invalido(t) {
        t
    } else if inf.e_dynamic(t) {
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
            declarar_local(inf, cx, Local { nome: name.sym, tipo: typ, final_: *final_, late: false, const_: false, offset: name.span.start, funcao_local: false }, true);
        }
        ast::ForInTarget::Pattern { final_, pattern } => {
            patterns::declare_by_type(inf, cx, *final_, *pattern, el);
        }
        ast::ForInTarget::Expression(e) => {
            // `for (x in e)`: atribuição a uma variável existente.
            let a = &inf.program.unit(cx.unit).ast;
            if let ExprKind::Identifier(n) = &a.expr(*e).kind {
                if let Some(Nome::Local(id)) = cx.buscar(n.sym) {
                    expr::resolver(inf, cx, *e, crate::resolved::Resolved::Local(id));
                    // `checkFinalAlreadyAssigned(identifier, isForEachIdentifier:
                    // true)` (3.6.2 `for_resolver.dart:131-136`,
                    // `assignment_expression_resolver.dart:354-388`): o local
                    // `final` relata sempre, sem olhar a atribuição definida.
                    let l = cx.local(id);
                    // `visitForEachPartsWithIdentifier` → `_checkForAssignmentToFinal`
                    // (3.6.2 `error_verifier.dart:926-930`, `:2155-2161`): a
                    // constante local, com as partes do laço válidas.
                    if l.const_ && valid_parts {
                        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::ASSIGNMENT_TO_CONST, n.span, &[]);
                    }
                    if l.final_ && !l.const_ && !cx.funcoes_locais.contains(&id) {
                        if l.late {
                            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::LATE_FINAL_LOCAL_ALREADY_ASSIGNED, n.span, &[]);
                        } else {
                            let msg = format!("{}: '{}'", crate::codes::ASSIGNMENT_TO_FINAL_LOCAL.template, inf.interner.resolve(l.nome));
                            inf.aviso(msg, n.span);
                        }
                    }
                    let decl = cx.local(id).tipo;
                    expr::registrar(inf, cx, *e, decl);
                    // `DemoteViaExplicitWrite` no identificador
                    // (`ForEachPartsWithIdentifier.identifier`).
                    let reason = super::flow::NonPromotionReason::Escrita { nome: n.sym, span: n.span };
                    return Some(LoopWrite { local: id, declarado: decl, written: el, motivo: reason });
                }
            }
            infer_free(inf, cx, *e);
        }
    }
    None
}

/// As anotações de uma declaração local (`Ast::metadados_locais`), pelo
/// `AnnotationResolver` no escopo local: o nome que resolve para uma local
/// (`_localVariable`, 3.6.2 `annotation_resolver.dart:170-184`) é
/// `INVALID_ANNOTATION` se ela não é `const` ou se há argumentos; a função
/// local também (não é variável nem acessor); o resto pelo escopo da
/// biblioteca, como as anotações de declaração.
fn validate_local_annotations(inf: &mut BodyInferrer<'_>, cx: &mut Body, s: StmtId) {
    let a = &inf.program.unit(cx.unit).ast;
    let Some((_, ms)) = a.metadados_locais.iter().find(|(x, _)| *x == s) else { return };
    for m in ms.iter() {
        validate_local_annotation(inf, cx, m);
    }
}

/// Uma anotação no escopo local (ver [`validar_anotacoes_locais`]).
pub(crate) fn validate_local_annotation(inf: &mut BodyInferrer<'_>, cx: &Body, m: &ast::Annotation) {
    let Some(n1) = m.name.first() else { return };
    match cx.buscar(n1.sym) {
        Some(Nome::Local(id)) => {
            let l = cx.local(id);
            if cx.funcoes_locais.contains(&id) || !l.const_ || m.arguments.is_some() || m.name.len() > 1 {
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INVALID_ANNOTATION, m.span, &[]);
            }
        }
        Some(_) => {}
        None => super::functions::validate_annotation(inf, cx.unit, cx.classe, m),
    }
}

/// `_checkForEachParts` (an611:src/generated/error_verifier.dart:3103-3233):
/// `FOR_IN_OF_INVALID_TYPE` quando o iterável (resolvido ao limite) não é
/// `Iterable`/`Stream`, e `FOR_IN_OF_INVALID_ELEMENT_TYPE` quando o tipo dos
/// elementos não é atribuível à variável do laço. `void` (relatado antes),
/// `dynamic` e tipos anuláveis (o erro é o de nulo) não relatam.
/// Devolve o `_checkForEachParts` (`error_verifier.dart:3104-3210`): sem
/// erro, com o iterável não `void` nem anulável e a variável declarada.
fn for_in_invalid_type(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    target: &ast::ForInTarget,
    iterable: ExprId,
    t: TypeId,
    written: Option<TypeId>,
    await_: bool,
) -> bool {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let Some(cls) = (if await_ { inf.core.stream_class } else { inf.core.iterable_class }) else { return false };
    // `for (var (p) in e)`: o `_checkForEachParts` não roda (só as partes com
    // declaração e com identificador); o `analyzePatternForIn` compartilhado
    // (3.6.2 `type_analyzer.dart:1445-1463`) relata se `asInstanceOf` do
    // tipo (sem olhar o `?`, parâmetro de tipo pelo limite) não acha
    // `Iterable`/`Stream`, fora `dynamic` e o tipo inválido; os argumentos
    // são o tipo e sempre `'Iterable'` (`shared_type_analyzer.dart:161-171`).
    if matches!(target, ast::ForInTarget::Pattern { .. }) {
        if inf.e_dynamic(t) || inf.table.e_invalido(t) || inf.the_is_unknown(t) {
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
    if inf.e_dynamic(t) || inf.the_is_unknown(t) {
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
    if inf.e_dynamic(base) || matches!(inf.table.get(base), Type::Void) || base == inf.core.object_nullable {
        return !matches!(target, ast::ForInTarget::Pattern { .. });
    }
    let sp = inf.span_expr(cx.unit, iterable);
    if !inf.atribuivel(base, the_required) {
        let tt = inf.table.format(base, inf.interner, inf.program);
        inf.warning_with_code(ce::FOR_IN_OF_INVALID_TYPE, sp, &[&tt, the_name]);
        return false;
    }
    // Tipo da variável: o escrito, ou o do local existente em `for (x in …)`.
    let var = match target {
        ast::ForInTarget::Declared { .. } => written,
        ast::ForInTarget::Expression(e) => match &inf.program.unit(cx.unit).ast.expr(*e).kind {
            ExprKind::Identifier(n) => match cx.buscar(n.sym) {
                Some(Nome::Local(id)) => Some(cx.local(id).tipo),
                _ => None,
            },
            _ => None,
        },
        ast::ForInTarget::Pattern { .. } => return false,
    };
    // A variável declarada sem tipo escrito tem o tipo do elemento: cabe.
    let Some(var) = var else { return matches!(target, ast::ForInTarget::Declared { .. }) };
    let Some(el) = inf.as_instance_of(base, Some(cls)).map(|a| a[0]) else { return true };
    let tearoff = matches!(inf.table.get(var), Type::Function { .. } | Type::FutureOr { .. })
        && matches!(inf.table.get(el), Type::Interface { .. } | Type::ExtensionType { .. } | Type::TypeParameter { .. } | Type::Intersection { .. });
    if !tearoff && !inf.atribuivel(el, var) {
        inf.warning_with_args(ce::FOR_IN_OF_INVALID_ELEMENT_TYPE, sp, &[crate::exibicao::Arg::Tipo(base), crate::exibicao::Arg::from(the_name), crate::exibicao::Arg::Tipo(var)]);
        return false;
    }
    true
}

/// `return e;` / `return;`.
fn retorno(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: Option<ExprId>, span: dartforge_diagnostics::Span) {
    let Some(fc) = cx.funcoes.last().cloned() else {
        if let Some(e) = e {
            infer_free(inf, cx, e);
        }
        return;
    };
    match e {
        Some(e) => {
            let t = inferir(inf, cx, e, fc.return_context);
            match fc.retorno {
                Some(_) => {
                    let _ = span;
                    super::functions::verify_return(inf, cx, &fc, e, t);
                }
                None => {
                    if let Some(f) = cx.funcoes.last_mut() {
                        f.returned_expressions.push((e, t));
                    }
                    let t = if fc.modificador == AsyncModifier::Async { inf.flatten(t) } else { t };
                    if let Some(f) = cx.funcoes.last_mut() {
                        f.returned.push(t);
                    }
                }
            }
        }
        None => {
            // `RETURN_WITHOUT_VALUE` (`_checkReturnWithoutValue`,
            // `an611:src/error/return_type_verifier.dart:277-295`): num
            // executável declarado não gerador, com retorno (o valor futuro,
            // em `async`) que não é `void`, `dynamic` nem `Null`.
            if let (Some(t), Some(_)) = (fc.retorno, fc.executavel.as_ref()) {
                let generator = matches!(fc.modificador, AsyncModifier::SyncStar | AsyncModifier::AsyncStar);
                if !generator {
                    let tv = if fc.modificador == AsyncModifier::Async { inf.future_value_type(t) } else { t };
                    if !matches!(inf.table.get(tv), Type::Void | Type::Dynamic | Type::Null) {
                        let sp = dartforge_diagnostics::Span { start: span.start, end: span.start + "return".len() };
                        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::RETURN_WITHOUT_VALUE, sp, &[]);
                    }
                }
            }
            if let Some(f) = cx.funcoes.last_mut() {
                f.empty_return = true;
                if f.executavel.is_none() {
                    f.valueless_returns.push(dartforge_diagnostics::Span { start: span.start, end: span.start + "return".len() });
                }
            }
        }
    }
}

// -------------------------------------------------------------------
// Varredura sintática: variáveis escritas (assignedIn/capturedIn)
// -------------------------------------------------------------------

pub(crate) enum Parte<'a> {
    Expr(ExprId),
    Stmt(StmtId),
    /// O corpo de um `for`/`for-in` de coleção.
    Elemento(&'a ast::CollectionElement),
}

/// Uma escrita: o nome e o offset do nome na declaração a que ela se refere,
/// quando essa declaração está dentro da região varrida (a mais interna com
/// esse nome, pelo escopo léxico); `None` quando é de fora (parâmetro do
/// membro, variável externa). Sem isso, `x = …` numa closure `test(…)`
/// contaminava um `x` homônimo de outra closure do mesmo `main`.
pub(crate) type Escrita = (SymbolId, Option<usize>);

/// O local de `cx` a que a escrita se refere, visto do ponto corrente.
pub(crate) fn write_local(cx: &Body, e: Escrita) -> Option<LocalId> {
    let Some(Nome::Local(id)) = cx.buscar(e.0) else { return None };
    match e.1 {
        Some(o) if cx.local(id).offset != o => None,
        _ => Some(id),
    }
}

/// Locais escritos nas partes (e os escritos dentro de closures nelas).
pub(crate) fn writes_in(inf: &BodyInferrer<'_>, cx: &Body, parts: &[Parte<'_>]) -> (Vec<LocalId>, Vec<LocalId>) {
    let mut v = Sweep::nova(&inf.program.unit(cx.unit).ast);
    for p in parts {
        match p {
            Parte::Expr(e) => v.expr(*e, false),
            Parte::Stmt(s) => v.stmt(*s, false),
            Parte::Elemento(el) => v.elemento(el, false),
        }
    }
    let ids = |es: &[Escrita]| -> Vec<LocalId> {
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
    (ids(&v.outside), ids(&v.dentro))
}

/// Escritas dentro de uma função (para a captura de escrita): os parâmetros
/// dela sombreiam os de fora no corpo inteiro (resolvem para eles mesmos).
pub(crate) fn names_written_in_function(inf: &BodyInferrer<'_>, unit: UnitId, f: &ast::Function) -> Vec<Escrita> {
    let mut v = Sweep::nova(&inf.program.unit(unit).ast);
    v.escopos.push(Vec::new());
    v.declarar_parametros(f);
    v.corpo(&f.body, false);
    let mut out = v.outside;
    for e in v.dentro {
        if !out.contains(&e) {
            out.push(e);
        }
    }
    out
}

/// Escritas num corpo, separadas: `(fora de literais de função, dentro de
/// literais)` — `assignedVariables.anywhere.written` e `.captured` do
/// analyzer.
pub(crate) fn separately_written_names(inf: &BodyInferrer<'_>, unit: UnitId, body: &ast::FunctionBody) -> (Vec<Escrita>, Vec<Escrita>) {
    let mut v = Sweep::nova(&inf.program.unit(unit).ast);
    v.corpo(body, false);
    (v.outside, v.dentro)
}

/// Varredura sintática das escritas, com os escopos léxicos declarados
/// dentro da região (blocos, `for`, `catch`, `case`, closures e funções
/// locais).
struct Sweep<'a> {
    a: &'a ast::Ast,
    escopos: Vec<Vec<(SymbolId, usize)>>,
    outside: Vec<Escrita>,
    dentro: Vec<Escrita>,
    /// O índice do escopo em que começa cada closure ou função local em
    /// curso: a escrita de uma variável declarada nela mesma não é captura.
    closures: Vec<usize>,
}

impl<'a> Sweep<'a> {
    fn nova(a: &'a ast::Ast) -> Self {
        Sweep { a, escopos: vec![Vec::new()], outside: Vec::new(), dentro: Vec::new(), closures: Vec::new() }
    }

    fn declarar(&mut self, n: ast::Name) {
        if let Some(e) = self.escopos.last_mut() {
            e.push((n.sym, n.span.start));
        }
    }

    fn declarar_parametros(&mut self, f: &ast::Function) {
        for p in f.parameters.iter().flat_map(|ps| ps.iter()) {
            if let Some(n) = p.name {
                self.declarar(n);
            }
        }
    }

    fn escrever(&mut self, n: ast::Name, inside: bool) {
        let finding = self
            .escopos
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, e)| e.iter().rev().find(|(s, _)| *s == n.sym).map(|(_, o)| (i, *o)));
        let decl = finding.map(|(_, o)| o);
        // `AssignedVariables.captured`: só a escrita numa closure de variável
        // declarada fora dela (o parâmetro de uma função local escrito nela
        // mesma é escrita comum).
        let inside = inside && !finding.is_some_and(|(i, _)| self.closures.last().is_some_and(|&c| i >= c));
        let e = (n.sym, decl);
        let v = if inside { &mut self.dentro } else { &mut self.outside };
        if !v.contains(&e) {
            v.push(e);
        }
    }

    fn with_scope(&mut self, f: impl FnOnce(&mut Self)) {
        self.escopos.push(Vec::new());
        f(self);
        self.escopos.pop();
    }

    fn corpo(&mut self, body: &ast::FunctionBody, inside: bool) {
        match body {
            ast::FunctionBody::Block(b) => self.stmt(*b, inside),
            ast::FunctionBody::Expression(x) => self.expr(*x, inside),
            _ => {}
        }
    }

    /// Closure ou função local: o que ela escreve conta como escrito em
    /// closure; os parâmetros dela são declarações próprias.
    fn funcao(&mut self, f: &ast::Function) {
        self.with_scope(|v| {
            v.closures.push(v.escopos.len() - 1);
            v.declarar_parametros(f);
            v.corpo(&f.body, true);
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
                    self.escrever(n, inside);
                    return;
                }
                _ => return,
            }
        }
    }

    /// Variáveis de um padrão: escritas (`PatternAssign`) ou declarações.
    fn padrao(&mut self, p: ast::PatternId, inside: bool, declares: bool) {
        use ast::PatternKind as P;
        let a = self.a;
        match &a.pattern(p).kind {
            P::Variable { name, var_, final_, ty } => {
                // `var`/`final`/tipo numa atribuição também declara
                // (`DeclaredVariablePattern`).
                if declares || *var_ || *final_ || ty.is_some() {
                    self.declarar(*name);
                } else {
                    self.escrever(*name, inside);
                }
            }
            P::Or(x, y) | P::And(x, y) => {
                self.padrao(*x, inside, declares);
                self.padrao(*y, inside, declares);
            }
            P::NullCheck(x) | P::NullAssert(x) | P::Parenthesized(x) => self.padrao(*x, inside, declares),
            P::Cast { pattern, .. } => self.padrao(*pattern, inside, declares),
            P::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => {
                            self.padrao(*x, inside, declares)
                        }
                        _ => {}
                    }
                }
            }
            P::Map { entries, .. } => {
                for en in entries.iter() {
                    self.padrao(en.value, inside, declares);
                }
            }
            P::Record { fields } | P::Object { fields, .. } => {
                for f in fields.iter() {
                    self.padrao(f.pattern, inside, declares);
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
                self.padrao(*pattern, inside, false);
                self.expr(*value, inside);
            }
            ExprKind::FunctionExpression(f) => self.funcao(a.function(*f)),
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
                    self.elemento(el, inside);
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
                        v.padrao(c.pattern, inside, true);
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

    fn elemento(&mut self, el: &ast::CollectionElement, inside: bool) {
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
                        v.padrao(*p, inside, true);
                    }
                    if let Some(g) = guard {
                        v.expr(*g, inside);
                    }
                    v.elemento(then, inside);
                });
                if let Some(e) = else_ {
                    self.elemento(e, inside);
                }
            }
            C::For { init, condition, updates, body, .. } => self.with_scope(|v| {
                match init {
                    Some(ast::ForInit::Expression(x)) => v.expr(*x, inside),
                    Some(ast::ForInit::Variables(vl)) => v.variaveis(vl, inside),
                    Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                        v.expr(*value, inside);
                        v.padrao(*pattern, inside, true);
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    v.expr(*c, inside);
                }
                for u in updates.iter() {
                    v.expr(*u, inside);
                }
                v.elemento(body, inside);
            }),
            C::ForIn { target, iterable, body, .. } => {
                self.expr(*iterable, inside);
                self.with_scope(|v| {
                    v.alvo_for_in(target, inside);
                    v.elemento(body, inside);
                });
            }
        }
    }

    fn variaveis(&mut self, vl: &ast::VariableList, inside: bool) {
        for x in vl.variables.iter() {
            if let Some(i) = x.initializer {
                self.expr(i, inside);
            }
            self.declarar(x.name);
        }
    }

    fn alvo_for_in(&mut self, target: &ast::ForInTarget, inside: bool) {
        match target {
            ast::ForInTarget::Declared { name, .. } => self.declarar(*name),
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
            StmtKind::Variables(vl) => self.variaveis(vl, inside),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.expr(*value, inside);
                self.padrao(*pattern, inside, true);
            }
            StmtKind::Function(f) => {
                let f = a.function(*f);
                if let Some(n) = f.name {
                    self.declarar(n);
                }
                self.funcao(f);
            }
            StmtKind::Expression(e) => self.expr(*e, inside),
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition, inside);
                self.with_scope(|v| {
                    if let Some(p) = case_pattern {
                        v.padrao(*p, inside, true);
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
                    Some(ast::ForInit::Variables(vl)) => v.variaveis(vl, inside),
                    Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                        v.expr(*value, inside);
                        v.padrao(*pattern, inside, true);
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
                    v.alvo_for_in(target, inside);
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
                            v.declarar(n);
                        }
                        if let Some(n) = c.stack_trace {
                            v.declarar(n);
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
