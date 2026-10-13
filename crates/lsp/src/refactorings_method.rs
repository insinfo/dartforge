//! Extract Method (`legacy/extract_method.dart`, docs/LSP-ESPECIFICACAO.md
//! §13.11.3): the initial conditions (`_checkSelection`,
//! `_initializeParameters`, `_initializeReturnType`, occurrences, getter,
//! names), the final ones (name, parameters, `validateCreateFunction`/
//! `validateCreateMethod`) and the change with `extractAll = false`, with
//! the imports from `addLibraryImports`. The analyzer's `ExitDetector`
//! (`AN:src/dart/resolve/exit_detector.dart`) is ported here over the
//! analyzer-shaped tree.
//! Written without compiling or running (2026-10-05).

use crate::analyzer_tree::Mark;
use crate::refactorings::{Context, Elem, RefactoringRequest, RefactoringResult, MethodSelection};
use crate::refactorings_exec::{State, Change, Severity, Text, ONE_INDENT, finish, options_name, validate_name};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind, LibraryId};
use dartforge_frontend::ast::{self, DeclKind, ExprId};
use dartforge_types::table::DisplayDecoration;
use dartforge_types::{SubtypeEnv, Type, TypeId, TypeTable};
use std::collections::{BTreeSet, HashMap, HashSet};

type R = RefactoringResult;

/// A parameter of the new method (`RefactoringMethodParameter`).
#[derive(Debug, Clone)]
struct Parameter {
    name: String,
    ty: String,
    /// The parameters of a function-typed parameter (`(int a)`).
    parameters: Option<String>,
}

/// An occurrence of the selection (`_Occurrence`).
#[derive(Debug, Clone)]
struct Occurrence {
    range: (usize, usize),
    is_selection: bool,
    /// Name in the selection → name in this occurrence.
    names: HashMap<String, String>,
}

/// The pattern of a snippet (`_SourcePattern`).
#[derive(Debug, Clone, Default)]
struct Pattern {
    types: Vec<Option<TypeId>>,
    normalized: String,
    /// Original name → `__refVar<i>`.
    names: Vec<(String, String)>,
}

impl Pattern {
    fn compatible(&self, other: &Pattern, table: &TypeTable) -> bool {
        other.normalized == self.normalized
            && other.types.len() == self.types.len()
            && self.types.iter().zip(other.types.iter()).all(|(a, b)| match (a, b) {
                (Some(a), Some(b)) => table.canonical(*a) == table.canonical(*b),
                (None, None) => true,
                _ => false,
            })
    }
}

/// The state of `ExtractMethodRefactoringImpl`.
struct Extract<'c, 'p> {
    cx: &'c Context<'p>,
    /// A copy of the type table (the LUB and `Future<T>` create types).
    table: TypeTable,
    sel: MethodSelection,
    /// `_selectionRange`.
    range: (usize, usize),
    /// `_parentMember`.
    member: Option<usize>,
    parameters: Vec<Parameter>,
    /// By the parameter's id (its name).
    references: HashMap<String, Vec<(usize, usize)>>,
    local_names: HashMap<String, Vec<(usize, usize)>>,
    not_qualified: HashSet<String>,
    return_type_: Option<TypeId>,
    return_variable: Option<String>,
    has_await: bool,
    variable_type: Option<String>,
    return_type: String,
    occurrences: Vec<Occurrence>,
    static_context: bool,
    create_getter: bool,
    names: Vec<String>,
    import: BTreeSet<LibraryId>,
    name: String,
    /// A Dart exception (becomes `UnhandledError`).
    exception: Option<String>,
    /// `_getExpectedClosureReturnTypeCode`.
    closure_return: String,
}

/// `refactor.perform`/`refactor.validate` of `EXTRACT_METHOD`.
pub(crate) fn run(cx: &Context<'_>, uri: &str, request: &RefactoringRequest) -> RefactoringResult {
    let preferred = match options_name(request) {
        Ok(n) => n,
        Err(e) => return e,
    };
    let mut at = Extract {
        cx,
        table: cx.p.query.table.clone(),
        sel: MethodSelection { range: (request.offset, request.offset + request.length), expression: None, closure: None, commands: Vec::new() },
        range: (request.offset, request.offset + request.length),
        member: None,
        parameters: Vec::new(),
        references: HashMap::new(),
        local_names: HashMap::new(),
        not_qualified: HashSet::new(),
        return_type_: None,
        return_variable: None,
        has_await: false,
        variable_type: None,
        return_type: String::new(),
        occurrences: Vec::new(),
        static_context: false,
        create_getter: false,
        names: Vec::new(),
        import: BTreeSet::new(),
        name: String::new(),
        exception: None,
        closure_return: String::new(),
    };
    let initial = at.initial_conditions(request.offset, request.length);
    if let Some(e) = at.exception.take() {
        return R::InternalError(e);
    }
    at.name = preferred.or_else(|| at.names.first().cloned()).unwrap_or_else(|| "newMethod".to_string());
    let at = &at;
    finish(request, initial, || at.final_conditions(), || at.change(uri))
}

/// `getEnclosingClassOrUnitMember`: the node right below the first class,
/// enum, extension, extension type, mixin or unit, walking up from `n`.
pub(crate) fn enclosing_member(cx: &Context<'_>, n: usize) -> Option<usize> {
    let mut member = n;
    for k in cx.with_parents(n) {
        if matches!(
            cx.kind(k),
            "ClassDeclaration" | "CompilationUnit" | "EnumDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration" | "MixinDeclaration"
        ) {
            return Some(member);
        }
        member = k;
    }
    None
}

impl<'c, 'p> Extract<'c, 'p> {
    fn node(&self, n: usize) -> (usize, usize) {
        let node = &self.cx.tree.nodes[n];
        (node.start, node.end)
    }

    fn covers(&self, n: usize) -> bool {
        let (i, f) = self.node(n);
        self.range.0 <= i && f <= self.range.1
    }

    /// The nodes in pre-order from `root` (the visitors' order).
    fn pre_order(&self, root: usize) -> Vec<usize> {
        let mut v = Vec::new();
        let mut stack = vec![root];
        while let Some(k) = stack.pop() {
            v.push(k);
            stack.extend(self.cx.children(k).iter().rev().copied());
        }
        v
    }

    /// The offset of the declared name of an identifier's local element.
    fn local_declaration(&self, n: usize, e: Elem) -> Option<usize> {
        match e {
            Elem::LocalFunction(fid) => self.cx.ast.function(fid).name.map(|x| x.span.start),
            Elem::LocalVariable | Elem::Parameter => match self.cx.tree.nodes[n].mark {
                Mark::Expr(x) => self.cx.bodies.local_declaration(x),
                _ => None,
            },
            _ => None,
        }
    }

    /// The identifier is in a write context (`inSetterContext`).
    fn in_write(&self, n: usize) -> bool {
        match self.cx.tree.nodes[n].mark {
            Mark::Expr(x) => self.cx.writes.contains(&x),
            _ => false,
        }
    }

    /// `checkInitialConditions`.
    fn initial_conditions(&mut self, o: usize, l: usize) -> State {
        let sel = match self.cx.method_selection(o, l) {
            Err(m) => return State::fatal(m),
            Ok(s) => s,
        };
        self.range = sel.range;
        let base = sel.closure.or(sel.expression).or(sel.commands.first().copied());
        self.member = base.and_then(|n| enclosing_member(self.cx, n));
        self.sel = sel;
        let r = self.initialize_parameters();
        self.has_await = self.compute_await();
        self.initialize_return_type();
        self.initialize_occurrences();
        self.create_getter = self.can_create_getter() && self.sel.expression.is_some_and(|e| self.getter_expression(e));
        // The names: the locals declared in the member are left out.
        let excluded: HashSet<String> = match self.member {
            Some(m) => self
                .pre_order(m)
                .into_iter()
                .filter(|&k| self.cx.kind(k) == "VariableDeclaration" && self.cx.tree.nodes[k].mark == Mark::LocalVariable)
                .map(|k| name_at_start(self.cx.node_text(k)))
                .collect(),
            None => HashSet::new(),
        };
        if let Some(e) = self.sel.expression {
            self.names = self.cx.names_to_expression(self.cx.node_type(e), e, &excluded, true);
        }
        if self.sel.closure.is_some() {
            self.closure_return = self.closure_expected_return();
        }
        if self.sel.closure.is_some() && !self.parameters.is_empty() {
            let names: Vec<String> = self.parameters.iter().map(|p| p.name.clone()).collect();
            let plural = if names.len() == 1 { "variable" } else { "variables" };
            return State::fatal(format!("Cannot extract the closure as a method,it references the external {plural} {}.", quoted_with_and(&names)));
        }
        r
    }

    /// `_initializeParameters`.
    fn initialize_parameters(&mut self) -> State {
        let mut r = State::default();
        let mut assigned: Vec<(usize, String)> = Vec::new();
        let order = self.pre_order(0);
        for &n in &order {
            match self.cx.kind(n) {
                "SimpleIdentifier" if self.covers(n) => self.visit_identifier(n, &mut assigned),
                "VariableDeclaration" if self.covers(n) && self.cx.tree.nodes[n].mark == Mark::LocalVariable => {
                    let d = self.cx.tree.nodes[n].start;
                    let name = name_at_start(self.cx.node_text(n));
                    if self.used_after(d) && !assigned.iter().any(|(x, _)| *x == d) {
                        assigned.push((d, name.clone()));
                    }
                    if let Some(range) = self.visible_range(n) {
                        self.local_names.entry(name).or_default().push(range);
                    }
                }
                _ => {}
            }
        }
        if let Some(e) = self.sel.expression {
            self.return_type_ = self.cx.node_type(e);
        }
        let commands = self.sel.commands.clone();
        if !commands.is_empty() {
            let has_return = commands.iter().any(|&c| self.can_end_with_return(c));
            if has_return {
                match Output::new(self.cx).exits(*commands.last().unwrap()) {
                    Ok(true) => {}
                    Ok(false) => r.add(
                        Severity::Error,
                        "Selected statements contain a return statement, but not all possible execution flows exit. Semantics may not be preserved.",
                    ),
                    Err(e) => {
                        self.exception = Some(e);
                        return r;
                    }
                }
            }
            self.return_type_ = self.returns_type(&commands);
        }
        if assigned.len() == 1 {
            if self.return_type_.is_some() {
                r.add(Severity::Fatal, "Ambiguous return value: Selected block contains assignment(s) to local variables and return statement.");
                return r;
            }
            let (d, name) = assigned[0].clone();
            self.return_type_ = self.cx.bodies.local_type(d);
            self.return_variable = Some(name);
        }
        if assigned.len() > 1 {
            let list: String = assigned.iter().map(|(_, n)| format!("{n}\n")).collect();
            r.add(
                Severity::Fatal,
                format!(
                    "Ambiguous return value: Selected block contains more than one assignment to local variables. Affected variables are:\n\n{}",
                    list.trim()
                ),
            );
        }
        r
    }

    /// `_InitializeParametersVisitor.visitSimpleIdentifier`.
    fn visit_identifier(&mut self, n: usize, assigned: &mut Vec<(usize, String)>) {
        let cx = self.cx;
        let name = cx.node_text(n).to_string();
        // The label of a named argument: the element is the parameter (local for
        // `isLocalElement`), and the visitor returns.
        if let Some(p) = cx.parent(n)
            && cx.kind(p) == "Label"
            && cx.parent(p).is_some_and(|g| cx.kind(g) == "NamedExpression")
        {
            return;
        }
        let e = cx.identifier_element(n, false);
        let local = matches!(e, Elem::LocalVariable | Elem::Parameter | Elem::LocalFunction(_));
        if local {
            let Some(d) = self.local_declaration(n, e) else { return };
            if !(self.range.0 <= d && d <= self.range.1) {
                if !self.parameters.iter().any(|p| p.name == name) {
                    let ty = if self.in_write(n) {
                        cx.bodies.local_type(d)
                    } else {
                        match cx.tree.nodes[n].mark {
                            Mark::Expr(x) => cx.bodies.get_type(x),
                            _ => None,
                        }
                    };
                    let mut buffer = String::new();
                    let code = match ty {
                        Some(t) => {
                            let names = self.local_parameter_names(e, d);
                            self.type_source(t, Some((&mut buffer, names)))
                        }
                        None => Some("dynamic".to_string()),
                    };
                    let Some(code) = code else { return };
                    self.parameters.push(Parameter { name: name.clone(), ty: code, parameters: (!buffer.is_empty()).then_some(buffer) });
                }
                let range = self.node(n);
                self.references.entry(name.clone()).or_default().push(range);
            }
            if self.in_write(n) && self.used_after(d) && matches!(e, Elem::LocalVariable | Elem::Parameter) && !assigned.iter().any(|(x, _)| *x == d) {
                assigned.push((d, name.clone()));
            }
        } else if !self.qualified(n) {
            self.not_qualified.insert(name);
        }
    }

    /// `SimpleIdentifier.isQualified`.
    fn qualified(&self, n: usize) -> bool {
        let cx = self.cx;
        let Some(p) = cx.parent(n) else { return false };
        match cx.kind(p) {
            "PrefixedIdentifier" => cx.children(p).get(1) == Some(&n),
            "PropertyAccess" => cx.children(p).last() == Some(&n),
            "ConstructorName" => cx.children(p).get(1) == Some(&n),
            "MethodInvocation" => {
                cx.method_name(p) == Some(n)
                    && (cx.children(p).first() != Some(&n) || cx.parent(p).is_some_and(|c| cx.kind(c) == "CascadeExpression" && cx.children(c).first() != Some(&p)))
            }
            _ => false,
        }
    }

    /// `_isUsedAfterSelection`: an identifier of the member with the same
    /// element after the selection.
    fn used_after(&self, d: usize) -> bool {
        let Some(m) = self.member else { return false };
        let (mi, mf) = self.node(m);
        self.cx.ast.exprs.iter().enumerate().any(|(i, e)| {
            matches!(e.kind, ast::ExprKind::Identifier(_))
                && e.span.start > self.range.1
                && mi <= e.span.start
                && e.span.end <= mf
                && self.cx.bodies.local_declaration(ExprId(i as u32)) == Some(d)
        })
    }

    /// `VisibleRangesComputer`: the block that contains the declaration.
    fn visible_range(&self, n: usize) -> Option<(usize, usize)> {
        let b = self.cx.with_parents(n).find(|&k| matches!(self.cx.kind(k), "Block" | "SwitchCase" | "SwitchPatternCase" | "SwitchDefault"))?;
        Some(self.node(b))
    }

    /// `_mayEndWithReturnStatement`: some `return` outside nested block bodies.
    fn can_end_with_return(&self, c: usize) -> bool {
        let mut stack = vec![c];
        while let Some(k) = stack.pop() {
            match self.cx.kind(k) {
                "ReturnStatement" => return true,
                "BlockFunctionBody" => {}
                _ => stack.extend(self.cx.children(k).iter().copied()),
            }
        }
        false
    }

    /// `_ReturnTypeComputer`: the LUB of the types of the `return e;`s
    /// (excluding bottom ones), outside nested block bodies.
    fn returns_type(&mut self, commands: &[usize]) -> Option<TypeId> {
        let mut ty: Option<TypeId> = None;
        for &c in commands {
            let mut order = Vec::new();
            let mut stack = vec![c];
            while let Some(k) = stack.pop() {
                if self.cx.kind(k) == "BlockFunctionBody" {
                    continue;
                }
                order.push(k);
                stack.extend(self.cx.children(k).iter().rev().copied());
            }
            for k in order {
                if self.cx.kind(k) != "ReturnStatement" {
                    continue;
                }
                let Some(&e) = self.cx.children(k).first() else { continue };
                let Some(t) = self.cx.node_type(e) else { continue };
                if self.is_bottom(t) {
                    continue;
                }
                ty = Some(match ty {
                    None => t,
                    Some(a) => {
                        let mut env = SubtypeEnv::new(&mut self.table, &self.cx.p.query.outline.hierarchy, &self.cx.p.query.core);
                        dartforge_types::lub(a, t, &mut env)
                    }
                });
            }
        }
        ty
    }

    /// `DartType.isBottom`: `Never` or a type parameter bounded by it.
    fn is_bottom(&self, t: TypeId) -> bool {
        match self.table.get(t) {
            Type::Never => true,
            Type::Intersection { bound, .. } => self.is_bottom(*bound),
            _ => false,
        }
    }

    /// `_HasAwaitVisitor`.
    fn compute_await(&self) -> bool {
        let roots: Vec<usize> = match (self.sel.expression, self.sel.commands.is_empty()) {
            (Some(e), _) => vec![e],
            (None, false) => self.sel.commands.clone(),
            _ => return false,
        };
        let mut stack = roots;
        while let Some(k) = stack.pop() {
            match self.cx.kind(k) {
                "AwaitExpression" => return true,
                "FunctionExpression" => continue,
                "ForStatement" | "ForElement" if self.cx.node_text(k).starts_with("await") => return true,
                _ => {}
            }
            stack.extend(self.cx.children(k).iter().copied());
        }
        false
    }

    /// `_initializeReturnType`.
    fn initialize_return_type(&mut self) {
        let core = &self.cx.p.query.core;
        if self.sel.closure.is_some() {
            self.variable_type = Some(String::new());
            self.return_type = String::new();
            return;
        }
        let Some(t) = self.return_type_ else {
            self.variable_type = None;
            self.return_type = if self.has_await {
                let fv = self.future(core.void_);
                self.type_source(fv, None).unwrap_or_default()
            } else {
                "void".to_string()
            };
            return;
        };
        if matches!(self.table.get(t), Type::Dynamic) && !matches!(self.table.decoration(t), Some(DisplayDecoration::Invalid)) {
            self.variable_type = Some(String::new());
            self.return_type = if self.has_await {
                let fd = self.future(core.dynamic_);
                self.type_source(fd, None).unwrap_or_default()
            } else {
                String::new()
            };
            return;
        }
        let code = self.type_source(t, None).unwrap_or_default();
        self.variable_type = Some(code.clone());
        if self.has_await {
            let interface_not_future = match self.table.get(t) {
                Type::Interface { class, .. } => Some(*class) != core.future_class,
                Type::FutureOr { .. } | Type::ExtensionType { .. } => true,
                _ => false,
            };
            if interface_not_future {
                let f = self.future(t);
                self.return_type = self.type_source(f, None).unwrap_or_default();
            }
        } else {
            self.return_type = code;
        }
    }

    /// `Future<t>`.
    fn future(&mut self, t: TypeId) -> TypeId {
        match self.cx.p.query.core.future_class {
            Some(c) => self.table.intern(Type::Interface { class: c, args: vec![t].into_boxed_slice(), nullable: false }),
            None => t,
        }
    }

    /// `_computeCanCreateGetter`.
    fn can_create_getter(&self) -> bool {
        if self.sel.closure.is_some() || !self.parameters.is_empty() {
            return false;
        }
        if let Some(e) = self.sel.expression
            && self.cx.kind(e) == "AssignmentExpression"
        {
            return false;
        }
        if !self.sel.commands.is_empty() {
            return self.return_type != "void";
        }
        true
    }

    /// `_isExpressionForGetter`.
    fn getter_expression(&self, e: usize) -> bool {
        let cx = self.cx;
        match cx.kind(e) {
            "BinaryExpression" => cx.children(e).iter().all(|&f| self.getter_expression(f)),
            "IntegerLiteral" | "DoubleLiteral" | "BooleanLiteral" | "NullLiteral" | "SimpleStringLiteral" | "StringInterpolation" | "AdjacentStrings"
            | "SymbolLiteral" | "ListLiteral" | "SetOrMapLiteral" | "RecordLiteral" => true,
            "PrefixExpression" => cx.children(e).first().is_some_and(|&f| self.getter_expression(f)),
            "PrefixedIdentifier" | "PropertyAccess" => cx.children(e).first().is_some_and(|&f| self.getter_expression(f)),
            "SimpleIdentifier" => true,
            _ => false,
        }
    }

    // -- Occurrences -------------------------------------------------------------

    /// `_getSourcePattern(range)`.
    fn pattern(&self, (start, end): (usize, usize)) -> Pattern {
        let cx = self.cx;
        let mut p = Pattern::default();
        let mut swaps: Vec<(usize, usize, String)> = Vec::new();
        let mut add = |p: &mut Pattern, ni: usize, nf: usize, name: String, ty: Option<TypeId>| {
            if start <= ni && nf <= end {
                let pattern = match p.names.iter().find(|(o, _)| *o == name) {
                    Some((_, x)) => x.clone(),
                    None => {
                        p.types.push(ty);
                        let x = format!("__refVar{}", p.names.len());
                        p.names.push((name.clone(), x.clone()));
                        x
                    }
                };
                swaps.push((ni - start, nf - start, pattern));
            }
        };
        // `_GetSourcePatternVisitor` (a `NamedExpression` only visits the expression).
        let mut stack = vec![0usize];
        while let Some(k) = stack.pop() {
            let (ki, kf) = self.node(k);
            if kf < start || ki > end {
                continue;
            }
            match cx.kind(k) {
                "SimpleIdentifier" => {
                    let e = cx.identifier_element(k, true);
                    if matches!(e, Elem::LocalVariable | Elem::Parameter | Elem::LocalFunction(_))
                        && let Some(d) = self.local_declaration(k, e)
                    {
                        add(&mut p, ki, kf, cx.node_text(k).to_string(), self.element_type(e, d));
                    }
                }
                "VariableDeclaration" if cx.tree.nodes[k].mark == Mark::LocalVariable => {
                    let name = name_at_start(cx.node_text(k));
                    add(&mut p, ki, ki + name.len(), name, cx.bodies.local_type(ki));
                }
                _ => {}
            }
            let children = cx.children(k);
            if cx.kind(k) == "NamedExpression" {
                if let Some(&x) = children.get(1) {
                    stack.push(x);
                }
            } else {
                stack.extend(children.iter().rev().copied());
            }
        }
        let mut text = cx.source[start..end].to_string();
        swaps.sort_by(|a, b| b.0.cmp(&a.0));
        for (a, b, x) in swaps {
            text.replace_range(a..b, &x);
        }
        p.normalized = match dartforge_frontend::lexer::lex(&text) {
            Ok(v) => v
                .iter()
                .filter(|t| t.kind != dartforge_frontend::token::Kind::Eof)
                .map(|t| text[t.span.start..t.span.end].to_string())
                .collect::<Vec<_>>()
                .join("\u{FFFF}"),
            Err(_) => String::new(),
        };
        p
    }

    /// The declared type of a local element (`VariableElement.type` or
    /// `FunctionElement.type`).
    fn element_type(&self, e: Elem, d: usize) -> Option<TypeId> {
        match e {
            Elem::LocalFunction(fid) => self.cx.bodies.local_type(d).or_else(|| self.cx.bodies.function_runtime_type(fid)),
            _ => self.cx.bodies.local_type(d),
        }
    }

    /// `_initializeOccurrences` with the `_InitializeOccurrencesVisitor`.
    fn initialize_occurrences(&mut self) {
        self.occurrences.clear();
        let Some(m) = self.member else { return };
        let Some(parent) = self.cx.parent(m) else { return };
        let selection_pattern = self.pattern(self.range);
        let mut is_static = false;
        self.visit_occurrences(parent, &selection_pattern, &mut is_static);
    }

    fn visit_occurrences(&mut self, n: usize, pattern: &Pattern, force: &mut bool) {
        let cx = self.cx;
        let kind = cx.kind(n);
        let previous = *force;
        match kind {
            "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer" => *force = true,
            "FieldDeclaration" | "MethodDeclaration" => *force = self.static_declaration(n),
            _ => {}
        }
        if kind == "Block" && !self.sel.commands.is_empty() {
            let cmds: Vec<usize> = cx.children(n).to_vec();
            self.windows(&cmds, pattern, *force);
        }
        if matches!(kind, "SwitchCase" | "SwitchDefault" | "SwitchPatternCase") && !self.sel.commands.is_empty() {
            let cmds: Vec<usize> = cx.children(n).iter().copied().filter(|&k| crate::refactorings::is_command_kind(cx.kind(k))).collect();
            self.windows(&cmds, pattern, *force);
        }
        if crate::refactorings::is_expression_kind(kind)
            && (self.sel.closure.is_some() || self.sel.expression.is_some_and(|e| cx.kind(e) == kind))
        {
            let range = self.node(n);
            self.try_occurrence(range, pattern, *force);
        }
        for f in cx.children(n).to_vec() {
            self.visit_occurrences(f, pattern, force);
        }
        // The visitor's `finally` clears (does not restore) the mark.
        if matches!(
            kind,
            "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer" | "FieldDeclaration" | "MethodDeclaration"
        ) {
            *force = false;
        } else {
            let _ = previous;
        }
    }

    /// The member is `static`.
    fn static_declaration(&self, n: usize) -> bool {
        let t = self.cx.node_text(n);
        // After the comment and the annotations: is the first token `static`?
        let (start, _) = self.node(n);
        let real_start = self
            .cx
            .children(n)
            .iter()
            .filter(|&&f| matches!(self.cx.kind(f), "Comment" | "Annotation"))
            .map(|&f| self.node(f).1)
            .max()
            .unwrap_or(start);
        let rest = self.cx.source[real_start..start + t.len()].trim_start();
        rest.starts_with("static") && !rest[6..].starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '$')
    }

    /// `_visitStatements`: windows of `n` statements.
    fn windows(&mut self, commands: &[usize], pattern: &Pattern, force: bool) {
        let n = self.sel.commands.len();
        let mut i = 0;
        while i + n <= commands.len() {
            let range = (self.node(commands[i]).0, self.node(commands[i + n - 1]).1);
            if self.try_occurrence(range, pattern, force) {
                i += n;
            } else {
                i += 1;
            }
        }
    }

    /// `_tryToFindOccurrence`.
    fn try_occurrence(&mut self, range: (usize, usize), pattern: &Pattern, force: bool) -> bool {
        if self.cx.analyze(range.0, range.1).fatal.is_some() {
            return false;
        }
        let p = self.pattern(range);
        if !pattern.compatible(&p, &self.table) {
            return false;
        }
        let is_selection = !(range.1 <= self.range.0) && !(range.0 >= self.range.1);
        let mut names = HashMap::new();
        for (original, x) in &p.names {
            if let Some((of_selection, _)) = pattern.names.iter().find(|(_, y)| y == x) {
                names.insert(of_selection.clone(), original.clone());
            }
        }
        self.occurrences.push(Occurrence { range, is_selection, names });
        if force {
            self.static_context = true;
        }
        true
    }

    // -- Types -------------------------------------------------------------------

    /// The parameter names of a local's function type (those of the analyzer's
    /// `FunctionType`): those of the local function, of the closure that
    /// initializes it, or of the written type.
    fn local_parameter_names(&self, e: Elem, d: usize) -> Vec<String> {
        let ast = self.cx.ast;
        let source = self.cx.source;
        let names_of = |ps: &[ast::Parameter]| ps.iter().map(|p| p.name.map(|n| source[n.span.start..n.span.end].to_string()).unwrap_or_default()).collect();
        if let Elem::LocalFunction(fid) = e {
            return ast.function(fid).parameters.as_deref().map(names_of).unwrap_or_default();
        }
        // The declaration: a parameter with a written type, or a variable.
        if let Some((p, _)) = crate::project::parameter_in(ast, d) {
            if let Some(t) = p.ty
                && let ast::TypeKind::Function { parameters, .. } = &ast.ty(t).kind
            {
                return names_of(parameters);
            }
            if let Some(fs) = &p.function_parameters {
                return names_of(fs);
            }
            return Vec::new();
        }
        for s in ast.stmts.iter() {
            if let ast::StmtKind::Variables(l) = &s.kind
                && let Some(v) = l.variables.iter().find(|v| v.name.span.start == d)
            {
                if let Some(t) = l.ty
                    && let ast::TypeKind::Function { parameters, .. } = &ast.ty(t).kind
                {
                    return names_of(parameters);
                }
                if l.ty.is_none()
                    && let Some(i) = v.initializer
                    && let ast::ExprKind::FunctionExpression(f) = ast.expr(i).kind
                {
                    return ast.function(f).parameters.as_deref().map(names_of).unwrap_or_default();
                }
            }
        }
        Vec::new()
    }

    /// `LibraryElement.getTypeSource(type, librariesToImport, parametersBuffer)`.
    fn type_source(&mut self, t: TypeId, buffer: Option<(&mut String, Vec<String>)>) -> Option<String> {
        if let Some(DisplayDecoration::Alias { typedef, args }) = self.table.decoration(t).cloned() {
            let null = self.table.get(t).is_declared_nullable();
            return self.source_with_arguments(Element::Typedef(typedef), null, &args);
        }
        if matches!(self.table.decoration(t), Some(DisplayDecoration::Invalid)) {
            return Some("dynamic".to_string());
        }
        if matches!(self.table.decoration(t), Some(DisplayDecoration::NeverNullable)) {
            return Some("Never".to_string());
        }
        match self.table.get(t).clone() {
            Type::Dynamic => Some("dynamic".to_string()),
            Type::Function { ret, positional, optional, named, .. } => {
                let Some((buf, names)) = buffer else { return Some("Function".to_string()) };
                buf.push('(');
                let mut i = 0usize;
                let write = |this: &mut Self, buf: &mut String, ty_: TypeId, name: &str| {
                    let source = this.type_source(ty_, None);
                    if buf.len() != 1 {
                        buf.push_str(", ");
                    }
                    buf.push_str(&source.unwrap_or_else(|| "null".to_string()));
                    buf.push(' ');
                    buf.push_str(name);
                };
                for &p in positional.iter().chain(optional.iter()) {
                    let name = names.get(i).cloned().unwrap_or_default();
                    write(self, buf, p, &name);
                    i += 1;
                }
                let mut named_: Vec<(String, TypeId)> = named.iter().map(|(n, ty, _)| (self.cx.p.name(*n).to_string(), *ty)).collect();
                named_.sort_by(|a, b| a.0.cmp(&b.0));
                for (n, ty) in named_ {
                    write(self, buf, ty, &n);
                }
                buf.push(')');
                self.type_source(ret, None)
            }
            Type::Interface { class, args, nullable } | Type::ExtensionType { decl: class, args, nullable } => {
                self.source_with_arguments(Element::Class(class), nullable, &args)
            }
            Type::FutureOr { arg, nullable } => match self.future_or_class() {
                Some(c) => self.source_with_arguments(Element::Class(c), nullable, &[arg]),
                None => Some("FutureOr".to_string()),
            },
            Type::Null => {
                let core = &self.cx.p.query.core;
                match core.null_class {
                    Some(c) => self.source_with_arguments(Element::Class(c), false, &[]),
                    None => Some("Null".to_string()),
                }
            }
            Type::Never => Some("Never".to_string()),
            Type::Record { positional, named, nullable } => {
                let mut s = String::from("(");
                let total = positional.len() + named.len();
                let mut i = 0usize;
                for &p in positional.iter() {
                    s.push_str(&self.type_source(p, None).unwrap_or_else(|| "null".to_string()));
                    if i + 1 < total {
                        s.push_str(", ");
                    }
                    i += 1;
                }
                if !named.is_empty() {
                    s.push('{');
                    let mut named_: Vec<(String, TypeId)> = named.iter().map(|(n, ty)| (self.cx.p.name(*n).to_string(), *ty)).collect();
                    named_.sort_by(|a, b| a.0.cmp(&b.0));
                    for (n, ty) in named_ {
                        s.push_str(&self.type_source(ty, None).unwrap_or_else(|| "null".to_string()));
                        s.push(' ');
                        s.push_str(&n);
                        if i + 1 < total {
                            s.push_str(", ");
                        }
                        i += 1;
                    }
                    s.push('}');
                }
                s.push(')');
                if nullable {
                    s.push('?');
                }
                Some(s)
            }
            Type::TypeParameter { .. } | Type::Intersection { .. } => Some("dynamic".to_string()),
            Type::Void => Some("void".to_string()),
        }
    }

    /// The `FutureOr` class of `dart:async`.
    fn future_or_class(&self) -> Option<ClassId> {
        let p = self.cx.p;
        let prog = p.program();
        let lib = p.query.core.async_library?;
        prog.library(lib).exported.iter().find(|(s, _)| p.name(**s) == "FutureOr").and_then(|(_, b)| match b.getter {
            Some(Element::Class(c)) => Some(c),
            _ => None,
        })
    }

    /// `_getTypeCodeElementArguments`.
    fn source_with_arguments(&mut self, el: Element, null: bool, args: &[TypeId]) -> Option<String> {
        let p = self.cx.p;
        let prog = p.program();
        let current = prog.unit(self.cx.unit).library;
        let (library, name) = match el {
            Element::Class(c) => (prog.class(c).library, p.name(prog.class(c).name).to_string()),
            Element::Typedef(t) => (prog.typedef(t).library, p.name(prog.typedef(t).name).to_string()),
            _ => return None,
        };
        let mut s = String::new();
        if library != current {
            if name.starts_with('_') {
                return None;
            }
            match self.element_import(current, el, &name) {
                Some(Some(prefix)) => {
                    s.push_str(&prefix);
                    s.push('.');
                }
                Some(None) => {}
                None => {
                    self.import.insert(library);
                }
            }
        }
        s.push_str(&name);
        if !args.is_empty() {
            s.push('<');
            for (i, &a) in args.iter().enumerate() {
                if i != 0 {
                    s.push_str(", ");
                }
                s.push_str(&self.type_source(a, None)?);
            }
            s.push('>');
        }
        if null {
            s.push('?');
        }
        Some(s)
    }

    /// `_getImportElement`: the first import of the defining unit whose
    /// namespace has the element: `Some(prefix)`; `None` without an import.
    fn element_import(&self, current: LibraryId, el: Element, name: &str) -> Option<Option<String>> {
        let p = self.cx.p;
        let prog = p.program();
        let lib = prog.library(current);
        let defining = lib.units.first().copied();
        let symbol = prog.library(match el {
            Element::Class(c) => prog.class(c).library,
            Element::Typedef(t) => prog.typedef(t).library,
            _ => return None,
        });
        let _ = symbol;
        let mut explicit_core = false;
        for imp in lib.imports.iter().filter(|i| Some(i.unit) == defining) {
            let target = prog.library(imp.library);
            if target.uri == "dart:core" {
                explicit_core = true;
            }
            let Some(b) = target.exported.iter().find(|(s, _)| p.name(**s) == name).map(|(_, b)| *b) else { continue };
            if b.getter != Some(el) {
                continue;
            }
            // The combinators.
            let visible = imp.combinators.iter().all(|c| match c {
                ast::Combinator::Show(ns) => ns.iter().any(|n| p.name(n.sym) == name),
                ast::Combinator::Hide(ns) => !ns.iter().any(|n| p.name(n.sym) == name),
            });
            if !visible {
                continue;
            }
            return Some(imp.prefix.map(|x| p.name(x).to_string()));
        }
        // The implicit `dart:core` import.
        let library = match el {
            Element::Class(c) => prog.class(c).library,
            Element::Typedef(t) => prog.typedef(t).library,
            _ => return None,
        };
        if !explicit_core && prog.library(library).uri == "dart:core" {
            return Some(None);
        }
        None
    }

    // -- Final conditions ----------------------------------------------------------

    /// `checkFinalConditions`.
    fn final_conditions(&self) -> State {
        let mut r = validate_name(&self.name, "Method");
        r.absorb(self.verify_parameter_names());
        r.absorb(self.possible_conflicts());
        r
    }

    /// `_checkParameterNames`.
    fn verify_parameter_names(&self) -> State {
        let mut r = State::default();
        for (i, p) in self.parameters.iter().enumerate() {
            r.absorb(validate_name(&p.name, "Parameter"));
            if self.parameters.iter().enumerate().any(|(j, o)| j != i && o.name == p.name) {
                r.add(Severity::Error, format!("Parameter '{}' already exists", p.name));
                return r;
            }
            let refs = self.references.get(&p.name).map(|v| &v[..]).unwrap_or(&[]);
            let others = self.local_names.get(&p.name).map(|v| &v[..]).unwrap_or(&[]);
            let crosses = refs.iter().any(|a| others.iter().any(|b| !(a.1 <= b.0) && !(a.0 >= b.1)));
            if crosses || self.not_qualified.contains(&p.name) {
                r.add(Severity::Error, format!("'{}' is already used as a name in the selected code", p.name));
                return r;
            }
        }
        r
    }

    /// `_checkPossibleConflicts`.
    fn possible_conflicts(&self) -> State {
        let cx = self.cx;
        let Some(m) = self.member else { return State::default() };
        let Some(parent) = cx.parent(m) else { return State::default() };
        match cx.kind(parent) {
            "CompilationUnit" => self.validate_create_function(),
            "ClassDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration" | "MixinDeclaration" => {
                let Mark::Decl(d) = cx.tree.nodes[parent].mark else { return State::default() };
                match cx.declaration_class(cx.unit, d) {
                    Some(c) => self.validate_create_method(c, cx.kind(parent)),
                    None => State::default(),
                }
            }
            _ => State::default(),
        }
    }

    /// `validateCreateFunction`: `_validateWillConflict` and
    /// `_validateWillShadow(null)`.
    fn validate_create_function(&self) -> State {
        let p = self.cx.p;
        let prog = p.program();
        let lib = prog.unit(self.cx.unit).library;
        let mut r = State::default();
        // The top-level elements of each unit, in `children` order: accessors,
        // classes, enums, extensions, extension types, functions, mixins, aliases
        // and variables.
        for &u in prog.library(lib).units.iter() {
            for (kind, name) in top_elements(self.cx, u) {
                if name == self.name {
                    r.add(Severity::Error, format!("Library already declares {kind} with name '{}'.", self.name));
                }
            }
        }
        r.absorb(self.shadowings("function"));
        r
    }

    /// `_validateWillShadow(null)`: each class member with the name, with an
    /// unqualified reference from another class.
    fn shadowings(&self, created_kind: &str) -> State {
        let p = self.cx.p;
        let prog = p.program();
        let mut r = State::default();
        for (i, c) in prog.classes.iter().enumerate() {
            let class = ClassId(i as u32);
            for (&s, &f) in c.instance_members.iter().chain(c.static_members.iter()) {
                let Some((kind, qualified, functions)) = self.member_declaration(c, s, f) else { continue };
                if self.unqualified_reference_from_other_class(&functions, class) {
                    r.add(Severity::Error, format!("Created {created_kind} will shadow {kind} '{qualified}'."));
                }
            }
        }
        r
    }

    /// Member `s` → `f` of class `c` is a declaration with the searched name
    /// (`searchMemberDeclarations`: explicit accessors, fields and methods):
    /// the kind, the qualified name and the functions whose references count.
    fn member_declaration(
        &self,
        c: &dartforge_elements::model::ClassElement,
        s: dartforge_intern::SymbolId,
        f: dartforge_elements::model::FunctionElementId,
    ) -> Option<(&'static str, String, Vec<dartforge_elements::model::FunctionElementId>)> {
        let p = self.cx.p;
        let prog = p.program();
        let key = p.name(s);
        if key.trim_end_matches('=') != self.name {
            return None;
        }
        let fe = prog.function(f);
        let qualified = format!("{}.{}", p.name(c.name), self.name);
        match fe.kind {
            FunctionKind::Function | FunctionKind::Operator => Some(("method", qualified, vec![f])),
            FunctionKind::ImplicitAccessor if !key.ends_with('=') => {
                // The field: the references of the getter and of the setter.
                let mut fs = vec![f];
                for (&s2, &g) in c.instance_members.iter().chain(c.static_members.iter()) {
                    if p.name(s2) == format!("{key}=") {
                        fs.push(g);
                    }
                }
                Some(("field", qualified, fs))
            }
            FunctionKind::Getter => Some(("getter", self.name.clone(), vec![f])),
            FunctionKind::Setter => Some(("setter", self.name.clone(), vec![f])),
            _ => None,
        }
    }

    /// Some unqualified reference to one of the functions, made outside the
    /// owning class.
    fn unqualified_reference_from_other_class(&self, functions: &[dartforge_elements::model::FunctionElementId], owning: ClassId) -> bool {
        let p = self.cx.p;
        let prog = p.program();
        for &lib in p.libraries.iter() {
            for &u in prog.library(lib).units.iter() {
                let unit = prog.unit(u);
                let bodies = &p.query.bodies.units[u.0 as usize];
                for (i, e) in unit.ast.exprs.iter().enumerate() {
                    let ast::ExprKind::Identifier(n) = &e.kind else { continue };
                    let target = match bodies.get_resolved(ExprId(i as u32)) {
                        Some(dartforge_types::Resolved::Member { member: dartforge_types::MemberRef::Function(g), .. }) => *g,
                        _ => continue,
                    };
                    if !functions.contains(&target) {
                        continue;
                    }
                    // The class that contains the reference.
                    let inside = prog.classes.iter().enumerate().find(|(_, c)| {
                        c.decl.is_some_and(|d| d.unit == u && {
                            let s = unit.ast.decl(d.decl).span;
                            s.start <= n.span.start && n.span.end <= s.end
                        })
                    });
                    if inside.map(|(j, _)| ClassId(j as u32)) != Some(owning) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// `validateCreateMethod`.
    fn validate_create_method(&self, c: ClassId, node_kind_: &str) -> State {
        let p = self.cx.p;
        let prog = p.program();
        let class = prog.class(c);
        let mut r = State::default();
        let class_kind = match node_kind_ {
            "EnumDeclaration" => "enum",
            "MixinDeclaration" => "mixin",
            "ExtensionTypeDeclaration" => "extension type",
            _ => "class",
        };
        let class_name = p.name(class.name).to_string();
        // `_checkClassAlreadyDeclares`: accessors, fields, constructors, methods,
        // type parameters.
        let mut children: Vec<&'static str> = Vec::new();
        let mut members: Vec<(&dartforge_intern::SymbolId, &dartforge_elements::model::FunctionElementId)> =
            class.instance_members.iter().chain(class.static_members.iter()).collect();
        members.sort_by_key(|(_, f)| f.0);
        for (s, f) in &members {
            let fe = prog.function(**f);
            if p.name(**s).trim_end_matches('=') == self.name && matches!(fe.kind, FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor) {
                children.push(if fe.kind == FunctionKind::Setter || p.name(**s).ends_with('=') { "setter" } else { "getter" });
            }
        }
        for v in class.fields.iter() {
            if p.name(prog.variable(*v).name) == self.name {
                children.push("field");
            }
        }
        for (s, _) in class.constructors.iter() {
            if p.name(*s) == self.name {
                children.push("constructor");
            }
        }
        for (s, f) in &members {
            let fe = prog.function(**f);
            if p.name(**s) == self.name && matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator) {
                children.push("method");
            }
        }
        for tp in class.type_params.iter() {
            if p.name(tp.name) == self.name {
                children.push("type parameter");
            }
        }
        let capital = {
            let mut c = class_kind.chars();
            c.next().map(|x| x.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        };
        for f in children {
            r.add(Severity::Error, format!("{capital} '{class_name}' already declares {f} with name '{}'.", self.name));
        }
        if class_name == self.name {
            r.add(Severity::Error, format!("Created method has the same name as the declaring {class_kind} '{}'.", self.name));
        }
        // `_checkHierarchy(isRename: false)`: a superclass member with the name.
        let supers = self.cx.p.supertypes(c);
        for (i, k) in prog.classes.iter().enumerate() {
            let id = ClassId(i as u32);
            if !supers.contains(&id) || id == c {
                continue;
            }
            for (&s, &f) in k.instance_members.iter().chain(k.static_members.iter()) {
                let Some((kind, qualified, _)) = self.member_declaration(k, s, f) else { continue };
                r.add(Severity::Error, format!("Created method will shadow {kind} '{qualified}'."));
            }
        }
        r
    }

    // -- The change -------------------------------------------------------------------

    /// The signature (`signature`).
    fn signature(&self) -> String {
        if self.create_getter {
            return format!("get {}", self.name);
        }
        let mut s = format!("{}(", self.name);
        for (i, p) in self.parameters.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            if p.ty != "dynamic" && !p.ty.is_empty() {
                s.push_str(&p.ty);
                s.push(' ');
            }
            s.push_str(&p.name);
            if let Some(ps) = &p.parameters {
                s.push_str(ps);
            }
        }
        s.push(')');
        s
    }

    /// `_getMethodBodySource`.
    fn body(&self) -> String {
        let cx = self.cx;
        let tx = Text::new(cx.source);
        let (start, end) = self.range;
        let mut source = cx.source[start..end].to_string();
        let mut swaps: Vec<(usize, usize, String)> = Vec::new();
        for p in &self.parameters {
            for &(a, b) in self.references.get(&p.name).map(|v| &v[..]).unwrap_or(&[]) {
                swaps.push((a - start, b - start, p.name.clone()));
            }
        }
        swaps.sort_by(|a, b| b.0.cmp(&a.0));
        for (a, b, t) in swaps {
            source.replace_range(a..b, &t);
        }
        if let Some(f) = self.sel.closure
            && let Some(base) = cx.with_parents(f).find(|&k| crate::refactorings::is_command_kind(cx.kind(k)))
        {
            let indent_base = cx.node_prefix(base);
            let indent_target = self.member.map(|m| cx.node_prefix(m)).unwrap_or_default();
            source = tx.swap_indent(&source, &indent_base, &indent_target, false, false).trim().to_string();
        }
        if let Some(&first) = self.sel.commands.first() {
            let indent = cx.node_prefix(first);
            let target = format!("{}{ONE_INDENT}", self.member.map(|m| cx.node_prefix(m)).unwrap_or_default());
            source = tx.swap_indent(&source, &indent, &target, true, true);
        }
        source
    }

    /// `_getExpectedClosureReturnTypeCode`: the return of the type of the
    /// parameter that receives the closure, if not `dynamic`.
    fn closure_expected_return(&mut self) -> String {
        let cx = self.cx;
        let Some(f) = self.sel.closure else { return String::new() };
        let argument = match cx.parent(f) {
            Some(p) if cx.kind(p) == "NamedExpression" => p,
            _ => f,
        };
        let Some(list) = cx.parent(argument).filter(|&l| cx.kind(l) == "ArgumentList") else { return String::new() };
        let Some(owner) = cx.parent(list) else { return String::new() };
        let function = match cx.kind(owner) {
            "MethodInvocation" => match cx.method_name(owner).map(|m| cx.identifier_element(m, true)) {
                Some(Elem::Function(f)) => f,
                _ => return String::new(),
            },
            "InstanceCreationExpression" => match cx.node_expr(owner).and_then(|x| cx.bodies.get_resolved(x)) {
                Some(dartforge_types::Resolved::Constructor(f)) => *f,
                _ => return String::new(),
            },
            _ => return String::new(),
        };
        let Some(data) = cx.p.query.outline.functions.get(function.0 as usize) else { return String::new() };
        let parameter = if cx.kind(argument) == "NamedExpression" {
            let label = cx.children(argument).first().and_then(|&l| cx.children(l).first().copied());
            let name = label.map(|r| cx.node_text(r).to_string()).unwrap_or_default();
            data.parameters.iter().find(|p| p.kind == ast::ParameterKind::Named && p.name.is_some_and(|n| cx.p.name(n) == name))
        } else {
            let index = cx.children(list).iter().filter(|&&k| cx.kind(k) != "NamedExpression").position(|&k| k == argument);
            index.and_then(|i| data.parameters.iter().filter(|p| p.kind != ast::ParameterKind::Named).nth(i))
        };
        let Some(parameter) = parameter else { return String::new() };
        let table = &cx.p.query.table;
        let Type::Function { ret, .. } = table.get(parameter.ty) else { return String::new() };
        let ret = *ret;
        match self.type_source(ret, None) {
            Some(t) if t != "dynamic" => format!("{t} "),
            _ => String::new(),
        }
    }

    /// `createChange`.
    fn change(&self, uri: &str) -> Change {
        let cx = self.cx;
        let tx = Text::new(cx.source);
        let mut m = Change::default();
        for o in self.occurrences.iter().filter(|o| o.is_selection) {
            let invocation = if self.sel.closure.is_some() {
                self.name.clone()
            } else {
                let mut s = String::new();
                if !self.sel.commands.is_empty()
                    && let Some(ty) = &self.variable_type
                {
                    match &self.return_variable {
                        Some(v) => {
                            let name_in_occurrence = o.names.get(v).cloned().unwrap_or_else(|| "null".to_string());
                            if !self.parameters.iter().any(|p| &p.name == v) {
                                if ty.is_empty() {
                                    s.push_str("var ");
                                } else {
                                    s.push_str(ty);
                                    s.push(' ');
                                }
                            }
                            s.push_str(&name_in_occurrence);
                            s.push_str(" = ");
                        }
                        None => s.push_str("return "),
                    }
                }
                if self.has_await {
                    s.push_str("await ");
                }
                s.push_str(&self.name);
                if !self.create_getter {
                    s.push('(');
                    for (i, p) in self.parameters.iter().enumerate() {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        s.push_str(&o.names.get(&p.name).cloned().unwrap_or_else(|| "null".to_string()));
                    }
                    s.push(')');
                }
                if !self.sel.commands.is_empty() {
                    s.push(';');
                }
                s
            };
            m.add(uri, Span { start: o.range.0, end: o.range.1 }, invocation);
        }
        // The declaration.
        if let Some(member) = self.member {
            let prefix = cx.node_prefix(member);
            let eol = tx.eol();
            let mut annotations = if self.static_context { "static ".to_string() } else { String::new() };
            let mut body = self.body();
            let mut declaration: Option<String> = None;
            if let Some(f) = self.sel.closure {
                let ty = &self.closure_return;
                let mut d = format!("{annotations}{ty}{}{body}", self.name);
                if cx.children(f).iter().any(|&c| cx.kind(c) == "ExpressionFunctionBody") {
                    d.push(';');
                }
                declaration = Some(d);
            }
            let is_async = if self.has_await { " async" } else { "" };
            if self.sel.expression.is_some() {
                let several_lines = body.contains(eol);
                if !several_lines {
                    if !self.return_type.is_empty() {
                        annotations.push_str(&format!("{} ", self.return_type));
                    }
                    declaration = Some(format!("{annotations}{}{is_async} => {body};", self.signature()));
                } else {
                    body = tx.dedent(&format!("{};", body.trim())).trim().to_string();
                    if !self.return_type.is_empty() {
                        annotations.push_str(&format!("{} ", self.return_type));
                    }
                    let mut d = format!("{annotations}{}{is_async} {{{eol}{prefix}  ", self.signature());
                    if !self.return_type.is_empty() {
                        d.push_str("return ");
                    }
                    d.push_str(&format!("{body}{eol}{prefix}}}"));
                    declaration = Some(d);
                }
            }
            if !self.sel.commands.is_empty() {
                if !self.return_type.is_empty() {
                    annotations.push_str(&format!("{} ", self.return_type));
                }
                let mut d = format!("{annotations}{}{is_async} {{{eol}{body}", self.signature());
                if let Some(v) = &self.return_variable {
                    d.push_str(&format!("{prefix}  return {v};{eol}"));
                }
                d.push_str(&format!("{prefix}}}"));
                declaration = Some(d);
            }
            if let Some(d) = declaration {
                let end = cx.tree.nodes[member].end;
                m.add(uri, Span { start: end, end: end }, format!("{eol}{eol}{prefix}{d}"));
            }
        }
        add_imports(cx, &mut m, &self.import);
        m
    }
}

/// `addLibraryImports(change, targetLibrary, libraries)`: the imports
/// sorted, among the existing ones, after the `library` or at the top.
pub(crate) fn add_imports(cx: &Context<'_>, m: &mut Change, libraries: &BTreeSet<LibraryId>) {
    if libraries.is_empty() {
        return;
    }
    let p = cx.p;
    let prog = p.program();
    let lib = prog.unit(cx.unit).library;
    let Some(&defining) = prog.library(lib).units.first() else { return };
    let Some(uri_lib) = p.unit_uri(defining) else { return };
    let unit = prog.unit(defining);
    let source = unit.source.as_str();
    let tx = Text::new(source);
    let eol = tx.eol();
    // The URIs to import (relative for `file:`), sorted.
    let folder = unit.path.as_deref().and_then(|c| c.parent()).map(|c| c.to_path_buf());
    let mut uris: Vec<String> = libraries
        .iter()
        .map(|&b| {
            let u = &prog.library(b).uri;
            if let (Some(folder), Ok(url)) = (&folder, url::Url::parse(u))
                && url.scheme() == "file"
                && let Ok(path) = url.to_file_path()
            {
                return relative_path(&path, folder);
            }
            u.clone()
        })
        .collect();
    uris.sort();
    // The preferred quotes.
    let quotes = if crate::refactorings_exec::enabled_rule(p, defining, "prefer_single_quotes") {
        '\''
    } else if crate::refactorings_exec::enabled_rule(p, defining, "prefer_double_quotes") {
        '"'
    } else {
        let (mut simple, mut double) = (0, 0);
        for d in unit.unit.directives.iter() {
            let uri = match &d.kind {
                ast::DirectiveKind::Import { uri, .. } | ast::DirectiveKind::Export { uri, .. } => Some(uri.span),
                _ => None,
            };
            if let Some(s) = uri {
                if source[s.start..s.end].starts_with('"') {
                    double += 1;
                } else {
                    simple += 1;
                }
            }
        }
        if double > simple { '"' } else { '\'' }
    };
    let imports: Vec<(String, usize, usize)> = unit
        .unit
        .directives
        .iter()
        .filter_map(|d| match &d.kind {
            ast::DirectiveKind::Import { uri, .. } => {
                let t = &source[uri.span.start..uri.span.end];
                let value = t.trim_matches(|c| c == '\'' || c == '"').to_string();
                Some((value, d.span.start, d.span.end))
            }
            _ => None,
        })
        .collect();
    if !imports.is_empty() {
        let mut first_package = true;
        for u in &uris {
            let mut inserted = false;
            let package = u.starts_with("package:");
            let mut after_dart = false;
            for (existing, start, _) in &imports {
                if existing.starts_with("dart:") {
                    after_dart = true;
                }
                if existing.starts_with("package:") {
                    first_package = false;
                }
                if u.as_str() < existing.as_str() {
                    m.add(&uri_lib, Span { start: *start, end: *start }, format!("import {quotes}{u}{quotes};{eol}"));
                    inserted = true;
                    break;
                }
            }
            if !inserted {
                let mut code = format!("{eol}import {quotes}{u}{quotes};");
                if package && first_package && after_dart {
                    code = format!("{eol}{code}");
                }
                let end = imports.last().unwrap().2;
                m.add(&uri_lib, Span { start: end, end: end }, code);
            }
            if package {
                first_package = false;
            }
        }
        return;
    }
    if let Some(d) = unit.unit.directives.iter().find(|d| matches!(d.kind, ast::DirectiveKind::Library { .. })) {
        let mut prefix = format!("{eol}{eol}");
        for u in &uris {
            m.add(&uri_lib, Span { start: d.span.end, end: d.span.end }, format!("{prefix}import {quotes}{u}{quotes};"));
            prefix = eol.to_string();
        }
        return;
    }
    // At the top: after the `#!` and the leading `//` comments.
    let mut offset = 0usize;
    let mut line_before = false;
    let n = source.len();
    if n >= 2 && offset < n - 2 && &source[0..2] == "#!" {
        line_before = true;
        offset = tx.next_line(offset);
        let mut empty = offset;
        while n >= 2 && empty < n - 2 {
            let next = tx.next_line(empty);
            let line = &source[empty..next];
            if line.trim().is_empty() {
                empty = next;
                continue;
            } else if line.starts_with("//") {
                offset = empty;
                break;
            } else {
                break;
            }
        }
    }
    while n >= 2 && offset < n - 2 && &source[offset..offset + 2] == "//" {
        line_before = true;
        offset = tx.next_line(offset);
    }
    let next = tx.next_line(offset);
    let line_after = !source[offset..next].trim().is_empty();
    for (i, u) in uris.iter().enumerate() {
        let mut code = format!("import {quotes}{u}{quotes};{eol}");
        if i == 0 && line_before {
            code = format!("{eol}{code}");
        }
        if i == uris.len() - 1 && line_after {
            code = format!("{code}{eol}");
        }
        m.add(&uri_lib, Span { start: offset, end: offset }, code);
    }
}

/// `pathContext.relative(what, from: folder)` with `/`.
pub(crate) fn relative_path(target: &std::path::Path, folder: &std::path::Path) -> String {
    let a: Vec<_> = target.components().collect();
    let b: Vec<_> = folder.components().collect();
    let common = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = Vec::new();
    for _ in common..b.len() {
        parts.push("..".to_string());
    }
    for c in &a[common..] {
        parts.push(c.as_os_str().to_string_lossy().into_owned());
    }
    parts.join("/")
}

/// The top-level elements of a unit with their kind (`kind.displayName`),
/// in `CompilationUnitElementImpl.children` order.
pub(crate) fn top_elements(cx: &Context<'_>, u: dartforge_elements::model::UnitId) -> Vec<(&'static str, String)> {
    let prog = cx.p.program();
    let unit = prog.unit(u);
    let source = unit.source.as_str();
    let ast = &unit.ast;
    let text = |n: ast::Name| source[n.span.start..n.span.end].to_string();
    let decls: Vec<&ast::Decl> = unit.unit.declarations.iter().map(|&d| ast.decl(d)).collect();
    let mut v: Vec<(&'static str, String)> = Vec::new();
    // Accessors: the explicit ones and the variables' synthetic ones, in order.
    for d in &decls {
        match &d.kind {
            DeclKind::Function(f) => {
                let fun = ast.function(*f);
                let Some(n) = fun.name else { continue };
                match fun.kind {
                    ast::FunctionKind::Getter => v.push(("getter", text(n))),
                    ast::FunctionKind::Setter => v.push(("setter", text(n))),
                    _ => {}
                }
            }
            DeclKind::Variables(l) => {
                for var in l.variables.iter() {
                    v.push(("getter", text(var.name)));
                    if !(l.final_ || l.const_) || (l.late && var.initializer.is_none() && l.final_) {
                        v.push(("setter", text(var.name)));
                    }
                }
            }
            _ => {}
        }
    }
    let kinds: [(&'static str, fn(&ast::DeclKind) -> Option<ast::Name>); 4] = [
        ("class", |k| match k {
            DeclKind::Class(c) => Some(c.name),
            _ => None,
        }),
        ("enum", |k| match k {
            DeclKind::Enum(e) => Some(e.name),
            _ => None,
        }),
        ("extension", |k| match k {
            DeclKind::Extension(e) => e.name,
            _ => None,
        }),
        ("extension type", |k| match k {
            DeclKind::ExtensionType(e) => Some(e.name),
            _ => None,
        }),
    ];
    for (kind, f) in kinds {
        for d in &decls {
            if let Some(n) = f(&d.kind) {
                v.push((kind, text(n)));
            }
        }
    }
    for d in &decls {
        if let DeclKind::Function(f) = &d.kind {
            let fun = ast.function(*f);
            let Some(n) = fun.name else { continue };
            if matches!(fun.kind, ast::FunctionKind::Function | ast::FunctionKind::Operator) {
                v.push(("function", text(n)));
            }
        }
    }
    for d in &decls {
        if let DeclKind::Mixin(x) = &d.kind {
            v.push(("mixin", text(x.name)));
        }
    }
    for d in &decls {
        if let DeclKind::Typedef(x) = &d.kind {
            v.push(("type alias", text(x.name)));
        }
        if let DeclKind::Class(c) = &d.kind
            && c.mixin_application
        {
            // `class C = S with M;` is a `ClassElement` (already counted among classes).
        }
    }
    for d in &decls {
        if let DeclKind::Variables(l) = &d.kind {
            for var in l.variables.iter() {
                v.push(("top level variable", text(var.name)));
            }
        }
    }
    v
}

/// The name at the start of the text of a `VariableDeclaration`.
fn name_at_start(t: &str) -> String {
    let end = t.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(t.len());
    t[..end].to_string()
}

/// `quotedAndCommaSeparatedWithAnd`.
fn quoted_with_and(names: &[String]) -> String {
    let q: Vec<String> = names.iter().map(|n| format!("'{n}'")).collect();
    match q.len() {
        0 => String::new(),
        1 => q[0].clone(),
        2 => format!("{} and {}", q[0], q[1]),
        n => format!("{}, and {}", q[..n - 1].join(", "), q[n - 1]),
    }
}

// -- ExitDetector -----------------------------------------------------------------

/// The `ExitDetector` ([`dartforge_analysis::exit_detector::ExitDetector`]) with
/// `_elementExits` through the context's elements.
pub(crate) struct Output<'c, 'p> {
    cx: &'c Context<'p>,
}

impl<'c, 'p> Output<'c, 'p> {
    pub(crate) fn new(cx: &'c Context<'p>) -> Output<'c, 'p> {
        Output { cx }
    }

    /// `node.accept(ExitDetector)`.
    pub(crate) fn exits(&mut self, n: usize) -> Result<bool, String> {
        let cx = self.cx;
        let element = |m: usize| element_exits(cx, m);
        dartforge_analysis::exit_detector::ExitDetector::new(&cx.tree, &cx.source, &element).exits(n)
    }
}

/// `_elementExits`: the executable has `@alwaysThrows` or returns `Never`.
fn element_exits(cx: &Context<'_>, m: usize) -> bool {
    match cx.identifier_element(m, true) {
        Elem::Function(f) => {
            let p = cx.p;
            let declared = p.program().function(f).public_declaration.unwrap_or(f);
            let never = p
                .query
                .outline
                .functions
                .get(declared.0 as usize)
                .is_some_and(|d| matches!(p.query.table.get(d.return_type), Type::Never));
            never || has_always_throws(cx, declared)
        }
        Elem::LocalFunction(fid) => {
            let t = cx.ast.function(fid).name.and_then(|n| cx.bodies.local_type(n.span.start));
            matches!(t.map(|t| cx.p.query.table.get(t)), Some(Type::Function { ret, .. }) if matches!(cx.p.query.table.get(*ret), Type::Never))
        }
        _ => false,
    }
}

/// The declaration has the `@alwaysThrows` annotation (from `package:meta`).
fn has_always_throws(cx: &Context<'_>, f: dartforge_elements::model::FunctionElementId) -> bool {
    let prog = cx.p.program();
    let dartforge_elements::model::FunctionRef::Function { unit, function } = prog.function(f).node else { return false };
    let unit_ = prog.unit(unit);
    let target = unit_.ast.function(function).span;
    let decls = unit_.ast.decls.iter().map(|d| (&d.metadata, d.span)).chain(unit_.ast.members.iter().map(|m| (&m.metadata, m.span)));
    for (meta, span) in decls {
        if span.start <= target.start && target.end <= span.end {
            return meta.iter().any(|a| {
                let t = &unit_.source[a.span.start..a.span.end];
                t == "@alwaysThrows" || t.ends_with(".alwaysThrows")
            });
        }
    }
    false
}
