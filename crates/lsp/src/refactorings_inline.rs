//! Inline Method (`legacy/inline_method.dart`, docs/LSP-ESPECIFICACAO.md
//! §13.11.6): `_prepareMethod`, `_prepareMethodParts` (with the
//! `_VariablesVisitor`), the `_ReferenceProcessor` of each reference,
//! `_getMethodSourceForInvocation` (with the conflicting names from
//! `VisibleRangesComputer`) and the removal of the declaration.
//! Written without compiling or running (2026-10-05).

use crate::analyzer_tree::Mark;
use crate::refactorings::{Context, Elem, RefactoringRequest, RefactoringResult};
use crate::refactorings_exec::{State, Change, Severity, Text};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, FunctionKind, FunctionRef, UnitId};
use dartforge_frontend::ast::{self, AsyncModifier, ExprId};
use dartforge_types::{MemberRef, Resolved, Type};
use std::collections::HashMap;

type R = RefactoringResult;

/// `_ParameterOccurrence`.
#[derive(Debug, Clone)]
struct ParameterOccurrence {
    range: (usize, usize),
    parent_precedence: u8,
    in_interpolation: bool,
}

/// `_SourcePart`.
#[derive(Debug, Clone, Default)]
struct Part {
    base: usize,
    source: String,
    prefix: String,
    /// By parameter name, in the order of the first occurrence.
    parameters: Vec<(String, Vec<ParameterOccurrence>)>,
    /// By the local's declaration: the name and the ranges.
    variables: Vec<(usize, String, Vec<(usize, usize)>)>,
    explicit_this: Vec<usize>,
    implicit_this: Vec<usize>,
    implicit_classes: Vec<(String, Vec<usize>)>,
}

/// The inlined method.
struct Method<'c, 'p> {
    /// The tree of the declaration's unit.
    cxm: &'c Context<'p>,
    element: Elem,
    node: usize,
    parameters: Option<usize>,
    expression: Option<usize>,
    expression_part: Option<Part>,
    part_commands: Option<Part>,
    /// The declared parameters: name, required positional, named, and the text
    /// of the default value.
    declared: Vec<(String, bool, bool, Option<String>)>,
    is_async: bool,
}

/// `refactor.perform`/`refactor.validate` of `INLINE_METHOD`.
pub(crate) fn run(cx: &Context<'_>, uri: &str, request: &RefactoringRequest) -> RefactoringResult {
    let _ = uri;
    let p = cx.p;
    let prog = p.program();
    let fatal = "Method declaration or reference must be selected to activate this refactoring.";
    let fail = |e: State| crate::refactorings_exec::finish(request, e, State::default, Change::default);
    // `_prepareMethod`.
    let Some((e, _, declaration)) = cx.inline_method_element(request.offset) else { return fail(State::fatal(fatal)) };
    if !cx.executable(e) || cx.synthetic(e) {
        return fail(State::fatal(fatal));
    }
    let (unit_m, fid) = match e {
        Elem::LocalFunction(fid) => (cx.unit, fid),
        Elem::Function(f) => match prog.function(f).node {
            FunctionRef::Function { unit, function } => (unit, function),
            _ => return fail(State::fatal(fatal)),
        },
        _ => return fail(State::fatal(fatal)),
    };
    let context_m;
    let cxm: &Context<'_> = if unit_m == cx.unit {
        cx
    } else {
        context_m = Context::new(p, unit_m);
        &context_m
    };
    let Some(node) = cxm
        .tree
        .nodes
        .iter()
        .position(|k| k.mark == Mark::Function(fid) && matches!(k.kind, "FunctionDeclaration" | "MethodDeclaration"))
    else {
        return fail(State::fatal(fatal));
    };
    let exclude_source = declaration;
    let inline_all = exclude_source;
    if cx.operator(e) {
        return fail(State::fatal("Cannot inline operator."));
    }
    if cx.generator(e) {
        return fail(State::fatal("Cannot inline a generator."));
    }
    // The parameters and the body.
    let function_node = if cxm.kind(node) == "FunctionDeclaration" {
        cxm.children(node).iter().copied().find(|&c| cxm.kind(c) == "FunctionExpression").unwrap_or(node)
    } else {
        node
    };
    let parameters = cxm.children(function_node).iter().copied().find(|&c| cxm.kind(c) == "FormalParameterList");
    let body = cxm.children(function_node).iter().copied().find(|&c| matches!(cxm.kind(c), "BlockFunctionBody" | "ExpressionFunctionBody" | "EmptyFunctionBody" | "NativeFunctionBody"));
    let function_ = cxm.ast.function(fid);
    let declared: Vec<(String, bool, bool, Option<String>)> = function_
        .parameters
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|q| {
            let name = q.name.map(|n| cxm.source[n.span.start..n.span.end].to_string()).unwrap_or_default();
            let pattern = q.default_value.map(|d| {
                let s = cxm.ast.expr(d).span;
                cxm.source[s.start..s.end].to_string()
            });
            (name, q.kind == ast::ParameterKind::Required, q.kind == ast::ParameterKind::Named, pattern)
        })
        .collect();
    let mut m = Method {
        cxm,
        element: e,
        node,
        parameters,
        expression: None,
        expression_part: None,
        part_commands: None,
        declared,
        is_async: matches!(function_.modifier, AsyncModifier::Async | AsyncModifier::AsyncStar),
    };
    // `_prepareMethodParts`.
    let mut initial = State::default();
    match body.map(|c| cxm.kind(c)) {
        Some("ExpressionFunctionBody") => {
            let c = body.unwrap();
            if let Some(&x) = cxm.children(c).first() {
                m.expression = Some(x);
                let (a, b) = range(cxm, x);
                match m.part(cx, (a, b), fid) {
                    Ok(p) => m.expression_part = Some(p),
                    Err(e) => return R::InternalError(e),
                }
            }
        }
        Some("BlockFunctionBody") => {
            let c = body.unwrap();
            let Some(&block) = cxm.children(c).first() else { return fail(State::fatal("Cannot inline method without body.")) };
            let mut commands: Vec<usize> = cxm.children(block).to_vec();
            if let Some(&last) = commands.last()
                && cxm.kind(last) == "ReturnStatement"
            {
                m.expression = cxm.children(last).first().copied();
                if let Some(x) = m.expression {
                    match m.part(cx, range(cxm, x), fid) {
                        Ok(p) => m.expression_part = Some(p),
                        Err(e) => return R::InternalError(e),
                    }
                }
                commands.pop();
            }
            if !commands.is_empty() {
                let tx = Text::new(cxm.source);
                let s = tx.line_range(range(cxm, commands[0]).0, range(cxm, *commands.last().unwrap()).1);
                match m.part(cx, (s.start, s.end), fid) {
                    Ok(p) => m.part_commands = Some(p),
                    Err(e) => return R::InternalError(e),
                }
            }
            // `_ReturnsValidatorVisitor`: the second `return` (closures included).
            let mut how_many = 0;
            let mut stack = vec![block];
            let mut order = Vec::new();
            while let Some(k) = stack.pop() {
                order.push(k);
                stack.extend(cxm.children(k).iter().rev().copied());
            }
            for k in order {
                if cxm.kind(k) == "ReturnStatement" {
                    how_many += 1;
                    if how_many == 2 {
                        initial.add(Severity::Error, "Ambiguous return value.");
                    }
                }
            }
        }
        _ => return fail(State::fatal("Cannot inline method without body.")),
    }
    // The references.
    let references: Vec<(UnitId, Span)> = match e {
        Elem::LocalFunction(f) => {
            let d = cxm.ast.function(f).name.map(|n| n.span.start);
            let mut v: Vec<(UnitId, Span)> = cxm
                .ast
                .exprs
                .iter()
                .enumerate()
                .filter(|(i, x)| matches!(x.kind, ast::ExprKind::Identifier(_)) && cxm.bodies.local_declaration(ExprId(*i as u32)) == d && d.is_some())
                .map(|(_, x)| (unit_m, x.span))
                .collect();
            v.sort_by_key(|(_, s)| s.start);
            v
        }
        Elem::Function(f) => {
            let getter = prog.function(f).kind == FunctionKind::Getter;
            let mut v = crate::refactorings_exec::function_references(p, f, getter);
            v.sort_by_key(|(u, s)| (u.0, s.start));
            v
        }
        _ => Vec::new(),
    };
    if request.only_validate {
        return crate::refactorings_exec::finish(request, initial, State::default, Change::default);
    }
    if initial.has_fatal() {
        return R::Error(initial.message().unwrap_or_default());
    }
    // `checkFinalConditions`: the change.
    let mut finals = State::default();
    let mut change = Change::default();
    let mut contexts: HashMap<UnitId, Context<'_>> = HashMap::new();
    let mut already_async: Vec<(UnitId, usize)> = Vec::new();
    for (u, s) in references {
        let cxr: &Context<'_> = if u == cx.unit {
            cx
        } else if u == unit_m {
            cxm
        } else {
            contexts.entry(u).or_insert_with(|| Context::new(p, u));
            contexts.get(&u).unwrap()
        };
        let Some(uri_r) = p.unit_uri(u) else { continue };
        let Some(n) = cxr.tree.locate(s.start, s.start) else { continue };
        if cxr.kind(n) != "SimpleIdentifier" {
            return R::InternalError(format!("type '{}Impl' is not a subtype of type 'SimpleIdentifier' in type cast", cxr.kind(n)));
        }
        if let Err(e) = m.process(cxr, &uri_r, u, n, request.offset, inline_all, &mut finals, &mut change, &mut already_async) {
            return R::InternalError(e);
        }
    }
    if exclude_source && inline_all
        && let Some(uri_m) = p.unit_uri(unit_m)
    {
        let (a, b) = range(cxm, node);
        let tx = Text::new(cxm.source);
        let start = skip_empty_lines_left(cxm.source, tx.content_start(a));
        let s = {
            let mut end = b;
            if tx.line_start(start) == start {
                end = tx.content_end(b);
            }
            Span { start: start, end: end }
        };
        change.add(&uri_m, s, "");
    }
    let mut all = initial;
    all.absorb(finals);
    if all.has_error() {
        return R::Error(all.message().unwrap_or_default());
    }
    change.result()
}

/// The range of a node.
fn range(cx: &Context<'_>, n: usize) -> (usize, usize) {
    (cx.tree.nodes[n].start, cx.tree.nodes[n].end)
}

/// `_skipEmptyLinesLeft`.
fn skip_empty_lines_left(t: &str, mut i: usize) -> usize {
    let b = t.as_bytes();
    let mut last = i;
    while i > 0 {
        let c = b[i - 1];
        if !matches!(c, b' ' | b'\t' | b'\r' | b'\n') {
            return last;
        }
        if matches!(c, b'\r' | b'\n') {
            last = i;
        }
        i -= 1;
    }
    0
}

impl<'c, 'p> Method<'c, 'p> {
    /// `_createSourcePart(range)`: the text, the line prefix (sic: in the text
    /// of the cursor's unit) and the `_VariablesVisitor`.
    fn part(&self, cx: &Context<'_>, (a, b): (usize, usize), fid: ast::FunctionId) -> Result<Part, String> {
        let cxm = self.cxm;
        if a > cx.source.len() {
            return Err(format!("RangeError (offset): Invalid value: Not in inclusive range 0..{}: {a}", cx.source.len()));
        }
        let prefix = Text::new(cx.source).line_prefix(a).to_string();
        let mut part = Part { base: a, source: cxm.source[a..b].to_string(), prefix, ..Default::default() };
        // The names of the method's parameters (by name offset).
        let parameter_names: Vec<(usize, String)> = cxm
            .ast
            .function(fid)
            .parameters
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(|q| q.name.map(|n| (n.span.start, cxm.source[n.span.start..n.span.end].to_string())))
            .collect();
        let mut stack = vec![0usize];
        while let Some(k) = stack.pop() {
            let (ki, kf) = range(cxm, k);
            match cxm.kind(k) {
                "SimpleIdentifier" => {
                    if a <= ki && kf <= b {
                        self.implicit_qualifier(k, &mut part);
                        self.parameter(k, &parameter_names, &mut part);
                        let e = cxm.identifier_element(k, true);
                        if e == Elem::LocalVariable
                            && let Mark::Expr(x) = cxm.tree.nodes[k].mark
                            && let Some(d) = cxm.bodies.local_declaration(x)
                        {
                            add_variable(&mut part, d, cxm.node_text(k).to_string(), (ki - a, kf - a));
                        }
                    }
                    continue;
                }
                "ThisExpression" => {
                    if a <= ki && ki <= b {
                        part.explicit_this.push(ki - a);
                    }
                    continue;
                }
                "VariableDeclaration" => {
                    let name = crate::refactorings_exec::name_at_start_pub(cxm.node_text(k));
                    if a <= ki && ki + name.len() <= b {
                        add_variable(&mut part, ki, name.clone(), (ki - a, ki + name.len() - a));
                    }
                }
                _ => {}
            }
            // `visitNode`: only descends into what intersects the range.
            if k != 0 && (kf <= a || ki >= b) {
                continue;
            }
            stack.extend(cxm.children(k).iter().rev().copied());
        }
        Ok(part)
    }

    /// `_addMemberQualifier`.
    fn implicit_qualifier(&self, n: usize, part: &mut Part) {
        let cxm = self.cxm;
        let prog = cxm.p.program();
        // `getNodeQualifier`.
        if let Some(parent) = cxm.parent(n) {
            let has = match cxm.kind(parent) {
                "MethodInvocation" => cxm.method_name(parent) == Some(n) && cxm.children(parent).first() != Some(&n),
                "PropertyAccess" => cxm.children(parent).last() == Some(&n) && cxm.children(parent).len() > 1,
                "PrefixedIdentifier" => cxm.children(parent).get(1) == Some(&n),
                _ => false,
            };
            if has {
                return;
            }
        }
        let Mark::Expr(x) = cxm.tree.nodes[n].mark else { return };
        let (class, is_static): (Option<ClassId>, bool) = match cxm.bodies.get_resolved(x) {
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) => {
                let fe = prog.function(*f);
                if fe.extension.is_some() || !matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator | FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor) {
                    return;
                }
                (fe.class, fe.static_)
            }
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
                let ve = prog.variable(*v);
                if ve.extension.is_some() {
                    return;
                }
                (ve.class, ve.static_)
            }
            _ => return,
        };
        let Some(c) = class else { return };
        let o = cxm.tree.nodes[n].start - part.base;
        if is_static {
            let name = cxm.p.name(prog.class(c).name).to_string();
            match part.implicit_classes.iter_mut().find(|(k, _)| *k == name) {
                Some((_, v)) => v.push(o),
                None => part.implicit_classes.push((name, vec![o])),
            }
        } else {
            part.implicit_this.push(o);
        }
    }

    /// `_addParameter`.
    fn parameter(&self, n: usize, names: &[(usize, String)], part: &mut Part) {
        let cxm = self.cxm;
        if cxm.identifier_element(n, true) != Elem::Parameter {
            return;
        }
        let Mark::Expr(x) = cxm.tree.nodes[n].mark else { return };
        let Some(d) = cxm.bodies.local_declaration(x) else { return };
        let Some((_, name)) = names.iter().find(|(o, _)| *o == d) else { return };
        let (ki, kf) = range(cxm, n);
        let occurrence = ParameterOccurrence {
            range: (ki - part.base, kf - part.base),
            parent_precedence: cxm.parent_precedence(n),
            in_interpolation: cxm.parent(n).is_some_and(|p| cxm.kind(p) == "InterpolationExpression"),
        };
        match part.parameters.iter_mut().find(|(k, _)| k == name) {
            Some((_, v)) => v.push(occurrence),
            None => part.parameters.push((name.clone(), vec![occurrence])),
        }
    }

    /// `_ReferenceProcessor.init` and `_process`.
    #[allow(clippy::too_many_arguments)]
    fn process(
        &self,
        cxr: &Context<'_>,
        uri_r: &str,
        u: UnitId,
        n: usize,
        offset: usize,
        inline_all: bool,
        state: &mut State,
        m: &mut Change,
        already_async: &mut Vec<(UnitId, usize)>,
    ) -> Result<(), String> {
        let tx = Text::new(cxr.source);
        // `init`.
        let command = cxr.with_parents(n).find(|&k| crate::refactorings::is_command_kind(cxr.kind(k)));
        let (ref_lines, ref_prefix) = match command {
            Some(c) => {
                let (a, b) = range(cxr, c);
                (Some(tx.line_range(a, b)), cxr.node_prefix(c))
            }
            None => (None, tx.line_prefix(cxr.tree.nodes[n].start).to_string()),
        };
        // `_shouldProcess`.
        if !inline_all {
            let (a, b) = range(cxr, n);
            if !(a <= offset && offset <= b) {
                return Ok(());
            }
        }
        // An `async` method in a synchronous body.
        if self.is_async
            && let Some(body) = cxr.with_parents(n).find(|&k| matches!(cxr.kind(k), "BlockFunctionBody" | "ExpressionFunctionBody" | "EmptyFunctionBody" | "NativeFunctionBody"))
        {
            let text = cxr.node_text(body);
            let sync = !text.starts_with("async");
            if sync {
                if text.starts_with("sync") {
                    state.add(Severity::Fatal, "Cannot inline async into sync*.");
                    return Ok(());
                }
                let ref_element = enclosing_executable(cxr, n);
                if let Some((_, future)) = ref_element
                    && !future
                {
                    state.add(Severity::Fatal, "Cannot inline async into a function that does not return a Future.");
                    return Ok(());
                }
                let key = (u, ref_element.map_or(usize::MAX, |(k, _)| k));
                if !already_async.contains(&key) {
                    already_async.push(key);
                    let o = cxr.tree.nodes[body].start;
                    m.add(uri_r, Span { start: o, end: o }, "async ");
                }
            }
        }
        let parent = cxr.parent(n);
        if let Some(inv) = parent.filter(|&k| cxr.kind(k) == "MethodInvocation") {
            let name = cxr.method_name(inv);
            let target = cxr.children(inv).first().copied().filter(|&a| Some(a) != name);
            let in_cascade = target.is_none() && cxr.parent(inv).is_some_and(|c| cxr.kind(c) == "CascadeExpression");
            let arguments: Vec<usize> = cxr
                .children(inv)
                .iter()
                .copied()
                .find(|&k| cxr.kind(k) == "ArgumentList")
                .map(|l| cxr.children(l).to_vec())
                .unwrap_or_default();
            self.inline_invocation(cxr, uri_r, n, inv, in_cascade, target, &arguments, ref_lines, &ref_prefix, state, m);
            return Ok(());
        }
        let prog = cxr.p.program();
        let is_accessor = match self.element {
            Elem::Function(f) => matches!(prog.function(f).kind, FunctionKind::Getter | FunctionKind::Setter),
            _ => false,
        };
        if cxr.is_method(self.element) {
            state.add(Severity::Fatal, "Cannot inline class method reference.");
            return Ok(());
        }
        if is_accessor {
            let mut usage = n;
            let mut target: Option<usize> = None;
            let mut cascade = false;
            if let Some(pp) = parent {
                match cxr.kind(pp) {
                    "PrefixedIdentifier" => {
                        usage = pp;
                        target = cxr.children(pp).first().copied();
                    }
                    "PropertyAccess" => {
                        usage = pp;
                        let f = cxr.children(pp);
                        if f.len() >= 2 {
                            target = f.first().copied();
                        } else {
                            // `realTarget` of a cascade section.
                            target = cxr.with_parents(pp).find(|&k| cxr.kind(k) == "CascadeExpression").and_then(|c| cxr.children(c).first().copied());
                            cascade = true;
                        }
                    }
                    _ => {}
                }
            }
            let mut arguments = Vec::new();
            let write = match cxr.tree.nodes[n].mark {
                Mark::Expr(x) => cxr.writes.contains(&x),
                _ => false,
            };
            if write
                && let Some(assign) = cxr.with_parents(n).find(|&k| cxr.kind(k) == "AssignmentExpression")
                && let Some(&right_side) = cxr.children(assign).get(1)
            {
                arguments.push(right_side);
            }
            self.inline_invocation(cxr, uri_r, n, usage, cascade, target, &arguments, ref_lines, &ref_prefix, state, m);
            return Ok(());
        }
        // Function reference (tear-off): the closure.
        let source = self.closure_source(cxr, &ref_prefix);
        let source = source.strip_suffix(';').unwrap_or(&source).to_string();
        let (a, b) = range(cxr, n);
        m.add(uri_r, Span { start: a, end: b }, source);
        Ok(())
    }

    /// The text from the parameters' `(` to the end of the declaration,
    /// re-indented to the reference's prefix and trimmed.
    fn closure_source(&self, cxr: &Context<'_>, ref_prefix: &str) -> String {
        let cxm = self.cxm;
        let start = self.parameters.map_or(range(cxm, self.node).1, |l| range(cxm, l).0);
        let end = range(cxm, self.node).1;
        let source = &cxm.source[start.min(end)..end];
        let method_prefix = Text::new(cxm.source).line_prefix(range(cxm, self.node).0).to_string();
        Text::new(cxr.source).swap_indent(source, &method_prefix, ref_prefix, false, false).trim().to_string()
    }

    /// `_canInlineBody(usage)`.
    fn can_inline_body(&self, cxr: &Context<'_>, usage: usize) -> bool {
        if self.part_commands.is_none() {
            return self.expression_part.is_some();
        }
        let parent = cxr.parent(usage);
        let grandparent = parent.and_then(|p| cxr.parent(p));
        let e = |k: Option<usize>, s: &str| k.is_some_and(|k| cxr.kind(k) == s);
        if let Some(pp) = parent {
            if crate::refactorings::is_command_kind(cxr.kind(pp)) {
                return e(grandparent, "Block");
            }
            if cxr.kind(pp) == "AssignmentExpression" {
                if cxr.children(pp).first() == Some(&usage) {
                    return grandparent.is_some_and(|a| crate::refactorings::is_command_kind(cxr.kind(a)) && e(cxr.parent(a), "Block"));
                }
                return self.expression_part.is_some();
            }
        }
        if self.expression_part.is_some() && e(parent, "VariableDeclaration") && e(grandparent, "VariableDeclarationList") {
            let p3 = grandparent.and_then(|a| cxr.parent(a));
            return e(p3, "VariableDeclarationStatement") && e(p3.and_then(|x| cxr.parent(x)), "Block");
        }
        false
    }

    /// `_inlineMethodInvocation`.
    #[allow(clippy::too_many_arguments)]
    fn inline_invocation(
        &self,
        cxr: &Context<'_>,
        uri_r: &str,
        ref_node: usize,
        usage: usize,
        cascade: bool,
        target: Option<usize>,
        arguments: &[usize],
        ref_lines: Option<Span>,
        ref_prefix: &str,
        state: &mut State,
        m: &mut Change,
    ) {
        if cascade {
            state.add(Severity::Error, "Cannot inline cascade invocation.");
        }
        if self.can_inline_body(cxr, usage) {
            if let Some(part) = &self.part_commands {
                let source = self.source_to_invocation(cxr, part, usage, target, arguments, state);
                let source = Text::new(cxr.source).swap_indent(&source, &part.prefix, ref_prefix, true, true);
                if let Some(l) = ref_lines {
                    m.add(uri_r, Span { start: l.start, end: l.start }, source);
                }
            }
            if let Some(part) = &self.expression_part {
                let mut source = self.source_to_invocation(cxr, part, usage, target, arguments, state);
                if let Some(x) = self.expression
                    && self.cxm.precedence(x) < cxr.parent_precedence(usage)
                {
                    source = format!("({source})");
                }
                if cxr.parent(usage).is_some_and(|p| cxr.kind(p) == "AwaitExpression") && source.starts_with("await") {
                    source = source.get(6..).unwrap_or("").to_string();
                }
                let (a, b) = range(cxr, usage);
                m.add(uri_r, Span { start: a, end: b }, source);
            } else if let Some(l) = ref_lines {
                m.add(uri_r, l, "");
            }
            return;
        }
        // Immediate closure: the identifier becomes the closure.
        let source = self.closure_source(cxr, ref_prefix);
        let (a, b) = range(cxr, ref_node);
        m.add(uri_r, Span { start: a, end: b }, source);
    }

    /// `_getMethodSourceForInvocation`.
    fn source_to_invocation(&self, cxr: &Context<'_>, part: &Part, context: usize, target: Option<usize>, arguments: &[usize], state: &mut State) -> String {
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        // The parameter of each argument (`staticParameterElement.name`).
        let positional: Vec<&(String, bool, bool, Option<String>)> = self.declared.iter().filter(|d| !d.2).collect();
        let mut index = 0usize;
        let mut by_name: Vec<(String, usize)> = Vec::new();
        for &a in arguments {
            if cxr.kind(a) == "NamedExpression" {
                let label = cxr.children(a).first().and_then(|&l| cxr.children(l).first().copied());
                if let Some(r) = label {
                    by_name.push((cxr.node_text(r).to_string(), a));
                }
            } else {
                if let Some(d) = positional.get(index) {
                    by_name.push((d.0.clone(), a));
                }
                index += 1;
            }
        }
        for (name, occurrences) in &part.parameters {
            let mut argument = by_name.iter().find(|(n, _)| n == name).map(|(_, a)| *a);
            if let Some(a) = argument
                && cxr.kind(a) == "NamedExpression"
            {
                argument = cxr.children(a).get(1).copied();
            }
            let (precedence, source, is_identifier) = match argument {
                Some(a) => (cxr.precedence(a), cxr.node_text(a).to_string(), cxr.kind(a) == "SimpleIdentifier"),
                None => {
                    let declared = self.declared.iter().find(|d| &d.0 == name);
                    if declared.is_some_and(|d| d.1) {
                        state.add(Severity::Error, format!("No argument for the parameter \"{name}\"."));
                        continue;
                    }
                    (0, declared.and_then(|d| d.3.clone()).unwrap_or_else(|| "null".to_string()), false)
                }
            };
            for o in occurrences {
                let text = if o.in_interpolation && !is_identifier {
                    format!("{{{source}}}")
                } else if precedence < o.parent_precedence {
                    format!("({source})")
                } else {
                    source.clone()
                };
                edits.push((o.range.0, o.range.1, text));
            }
        }
        for (class, offsets) in &part.implicit_classes {
            for &o in offsets {
                edits.push((o, o, format!("{class}.")));
            }
        }
        if let Some(a) = target {
            let target_source = cxr.node_text(a).to_string();
            for &o in &part.explicit_this {
                edits.push((o, o + 4, target_source.clone()));
            }
            for &o in &part.implicit_this {
                edits.push((o, o, format!("{target_source}.")));
            }
        }
        let conflicts = names_in_conflict(cxr, context);
        for (_, original, ranges) in &part.variables {
            let mut single = original.clone();
            let mut k = 2;
            while conflicts.contains(&single) {
                single = format!("{original}{k}");
                k += 1;
            }
            if &single != original {
                for &(a, b) in ranges {
                    edits.push((a, b, single.clone()));
                }
            }
        }
        edits.sort_by(|a, b| b.0.cmp(&a.0));
        let mut source = part.source.clone();
        for (a, b, t) in edits {
            if a <= b && b <= source.len() {
                source.replace_range(a..b, &t);
            }
        }
        source
    }
}

/// `addVariable`.
fn add_variable(part: &mut Part, d: usize, name: String, range: (usize, usize)) {
    match part.variables.iter_mut().find(|(k, _, _)| *k == d) {
        Some((_, _, v)) => v.push(range),
        None => part.variables.push((d, name, vec![range])),
    }
}

/// The element enclosing the reference (`SearchMatch.element`): the unit or
/// class executable member that contains it, and whether its return is a
/// `Future`.
fn enclosing_executable(cx: &Context<'_>, n: usize) -> Option<(usize, bool)> {
    let p = cx.p;
    for k in cx.with_parents(n) {
        let kind = cx.kind(k);
        let local = kind == "FunctionDeclaration" && cx.parent(k).is_some_and(|x| cx.kind(x) == "FunctionDeclarationStatement");
        if matches!(kind, "MethodDeclaration" | "FunctionDeclaration") && !local {
            let Mark::Function(fid) = cx.tree.nodes[k].mark else { return None };
            let f = p.node_function(cx.unit, fid)?;
            let ret = p.query.outline.functions.get(f.0 as usize).map(|d| d.return_type);
            let future = ret.is_some_and(|t| matches!(p.query.table.get(t), Type::Interface { class, .. } if Some(*class) == p.query.core.future_class));
            return Some((k, future));
        }
        if kind == "ConstructorDeclaration" {
            return Some((k, false));
        }
        if matches!(kind, "FieldDeclaration" | "TopLevelVariableDeclaration") {
            return None;
        }
    }
    None
}

/// `_getNamesConflictingAt(node)`.
fn names_in_conflict(cx: &Context<'_>, n: usize) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    // `_getLocalsConflictingRange`.
    let (start, end) = match cx.with_parents(n).find(|&k| cx.kind(k) == "Block") {
        Some(b) => (cx.tree.nodes[n].start, cx.tree.nodes[b].end),
        None => match cx.with_parents(n).find(|&k| matches!(cx.kind(k), "FunctionDeclaration" | "ConstructorDeclaration" | "MethodDeclaration")) {
            Some(x) => range(cx, x),
            None => (0, 0),
        },
    };
    if let Some(executable) = cx.with_parents(n).find(|&k| matches!(cx.kind(k), "FunctionDeclaration" | "ConstructorDeclaration" | "MethodDeclaration")) {
        for (name, (a, b)) in visible_ranges(cx, executable) {
            if !(b <= start) && !(a >= end) && !v.contains(&name) {
                v.push(name);
            }
        }
    }
    // The members of the enclosing class and of its superclasses.
    let p = cx.p;
    let prog = p.program();
    if let Some(decl) = cx.with_parents(n).find(|&k| matches!(cx.kind(k), "ClassDeclaration" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration"))
        && let Mark::Decl(d) = cx.tree.nodes[decl].mark
        && let Some(c) = cx.declaration_class(cx.unit, d)
    {
        let mut classes: Vec<ClassId> = p.supertypes(c).into_iter().filter(|&x| x != c).collect();
        classes.sort_by_key(|x| x.0);
        classes.push(c);
        for k in classes {
            let ce = prog.class(k);
            let mut names: Vec<String> = Vec::new();
            for (s, _) in ce.instance_members.iter().chain(ce.static_members.iter()) {
                names.push(p.name(*s).trim_end_matches('=').to_string());
            }
            for f in ce.fields.iter() {
                names.push(p.name(prog.variable(*f).name).to_string());
            }
            for (s, _) in ce.constructors.iter() {
                names.push(p.name(*s).to_string());
            }
            for tp in ce.type_params.iter() {
                names.push(p.name(tp.name).to_string());
            }
            for name in names {
                if !v.contains(&name) {
                    v.push(name);
                }
            }
        }
    }
    v
}

/// `VisibleRangesComputer.forNode(executable)`: the name of each local and
/// the range where it is visible.
pub(crate) fn visible_ranges(cx: &Context<'_>, root: usize) -> Vec<(String, (usize, usize))> {
    let mut v: Vec<(String, (usize, usize))> = Vec::new();
    let name_of = |t: &str| crate::refactorings_exec::name_at_start_pub(t);
    let mut stack = vec![root];
    while let Some(k) = stack.pop() {
        match cx.kind(k) {
            "CatchClause" => {
                for &f in cx.children(k) {
                    if cx.kind(f) == "CatchClauseParameter" {
                        v.push((cx.node_text(f).to_string(), range(cx, k)));
                    }
                }
                if let Some(&b) = cx.children(k).last() {
                    stack.push(b);
                }
                continue;
            }
            "SimpleFormalParameter" | "FieldFormalParameter" | "SuperFormalParameter" | "FunctionTypedFormalParameter" | "DefaultFormalParameter" => {
                // The body of the owning function.
                let owning = cx.with_parents(k).find(|&x| matches!(cx.kind(x), "FunctionExpression" | "MethodDeclaration" | "ConstructorDeclaration"));
                let body = owning.and_then(|d| cx.children(d).iter().copied().find(|&c| matches!(cx.kind(c), "BlockFunctionBody" | "ExpressionFunctionBody")));
                if let Some(c) = body {
                    v.push((parameter_name(cx, k), range(cx, c)));
                }
                continue;
            }
            "ForPartsWithDeclarations" => {
                if let Some(loop_) = cx.parent(k)
                    && let Some(&list) = cx.children(k).first()
                {
                    for &d in cx.children(list) {
                        if cx.kind(d) == "VariableDeclaration" {
                            v.push((name_of(cx.node_text(d)), range(cx, loop_)));
                            stack.extend(cx.children(d).iter().copied());
                        }
                    }
                }
                continue;
            }
            "FunctionDeclaration" => {
                if let Some(s) = cx.parent(k)
                    && let Some(b) = cx.parent(s)
                    && cx.kind(b) == "Block"
                    && let Mark::Function(fid) = cx.tree.nodes[k].mark
                    && let Some(name) = cx.ast.function(fid).name
                {
                    v.push((cx.source[name.span.start..name.span.end].to_string(), range(cx, b)));
                }
            }
            "VariableDeclarationStatement" => {
                if let Some(block) = cx.parent(k)
                    && let Some(&list) = cx.children(k).iter().find(|&&c| cx.kind(c) == "VariableDeclarationList")
                {
                    for &d in cx.children(list) {
                        if cx.kind(d) == "VariableDeclaration" {
                            v.push((name_of(cx.node_text(d)), range(cx, block)));
                            stack.extend(cx.children(d).iter().copied());
                        }
                    }
                }
                continue;
            }
            _ => {}
        }
        stack.extend(cx.children(k).iter().rev().copied());
    }
    v
}

/// The declared name of a formal parameter (the last identifier before `=`,
/// `:` or `(`).
fn parameter_name(cx: &Context<'_>, k: usize) -> String {
    let t = cx.node_text(k);
    let cut = t.find(['=', ':']).unwrap_or(t.len());
    let before = &t[..cut];
    let before = match before.find('(') {
        Some(i) if cx.kind(k) == "FunctionTypedFormalParameter" => &before[..i],
        _ => before,
    };
    before
        .trim_end()
        .rsplit(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .next()
        .unwrap_or("")
        .to_string()
}
