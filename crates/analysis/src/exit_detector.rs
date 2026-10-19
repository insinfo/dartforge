//! The `ExitDetector` of analyzer 3.6.2 (`analyzer/lib/src/dart/resolver/exit_detector.dart`)
//! over the tree in the analyzer's shape ([`crate::analyzer_tree`]): used by
//! the LSP's extract-method refactoring and by `use_build_context_synchronously`
//! (`terminatesControl`). `_elementExits` comes from outside, through the name node.
//! Moved from the LSP on 2026-10-05 (written without compiling or running).

use crate::analyzer_tree::AnalyzerTree;
use std::collections::HashSet;

fn is_stmt_kind(e: &str) -> bool {
    crate::analyzer_tree::is_statement(e)
}

/// The tree and its text.
#[derive(Clone, Copy)]
struct Cx<'a> {
    tree: &'a AnalyzerTree,
    source: &'a str,
}

impl<'a> Cx<'a> {
    fn kind(&self, n: usize) -> &'static str {
        self.tree.nodes[n].kind
    }

    fn parent(&self, n: usize) -> Option<usize> {
        self.tree.nodes[n].parent
    }

    fn children(&self, n: usize) -> &'a [usize] {
        &self.tree.nodes[n].children
    }

    fn with_parents(&self, n: usize) -> impl Iterator<Item = usize> + 'a {
        let tree = self.tree;
        std::iter::successors(Some(n), move |&k| tree.nodes[k].parent)
    }

    fn node_text(&self, n: usize) -> &'a str {
        let node = &self.tree.nodes[n];
        &self.source[node.start..node.end]
    }

    /// The `methodName` of a `MethodInvocation`: the last `SimpleIdentifier`
    /// child.
    fn method_name(&self, n: usize) -> Option<usize> {
        self.children(n).iter().rev().copied().find(|&f| self.kind(f) == "SimpleIdentifier")
    }
}

/// The analyzer's `ExitDetector` over the tree in the analyzer's shape.
pub struct ExitDetector<'a> {
    cx: Cx<'a>,
    /// `_elementExits` of the identifier (the `methodName`) with the given index.
    element: &'a dyn Fn(usize) -> bool,
    /// `_enclosingBlockContainsBreak`.
    breaks: bool,
    /// `_enclosingBlockContainsContinue`.
    continues: bool,
    /// `_enclosingBlockBreaksLabel`: the statements targeted by `break label`.
    broken_labels: HashSet<usize>,
}

impl<'a> ExitDetector<'a> {
    /// `element(n)`: the `_elementExits` of the element of identifier `n` (the
    /// executable with `@alwaysThrows` or with a `Never` return).
    pub fn new(tree: &'a AnalyzerTree, source: &'a str, element: &'a dyn Fn(usize) -> bool) -> ExitDetector<'a> {
        ExitDetector { cx: Cx { tree, source }, element, breaks: false, continues: false, broken_labels: HashSet::new() }
    }

    fn text(&self, n: usize) -> &str {
        self.cx.node_text(n)
    }

    fn children(&self, n: usize) -> Vec<usize> {
        self.cx.children(n).to_vec()
    }

    /// `_nodeExits`.
    fn maybe(&mut self, n: Option<usize>) -> Result<bool, String> {
        match n {
            Some(n) => self.exits(n),
            None => Ok(false),
        }
    }

    fn bool_literal(&self, n: usize) -> Option<bool> {
        (self.cx.kind(n) == "BooleanLiteral").then(|| self.text(n) == "true")
    }

    /// `_visitExpressions`: back to front.
    fn exprs(&mut self, ns: &[usize]) -> Result<bool, String> {
        for &n in ns.iter().rev() {
            if self.exits(n)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn stmts(&mut self, ns: &[usize]) -> Result<bool, String> {
        for &n in ns {
            if self.exits(n)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `node.accept(ExitDetector)`.
    pub fn exits(&mut self, n: usize) -> Result<bool, String> {
        let cx = self.cx;
        let f = self.children(n);
        match cx.kind(n) {
            "ArgumentList" => self.exprs(&f),
            "AsExpression" | "AwaitExpression" | "ParenthesizedExpression" | "IsExpression" | "SpreadElement" | "ExpressionStatement" => {
                self.maybe(f.first().copied())
            }
            "AssertInitializer" | "AssertStatement" | "ConstructorReference" | "EmptyStatement" | "ExtensionOverride"
            | "FunctionDeclarationStatement" | "FunctionExpression" | "GenericFunctionType" | "SimpleIdentifier" | "PrefixedIdentifier"
            | "LibraryIdentifier" | "Label" | "NamedType" | "PostfixExpression" | "PrefixExpression" | "SuperExpression" | "ThisExpression"
            | "IntegerLiteral" | "DoubleLiteral" | "BooleanLiteral" | "NullLiteral" | "SimpleStringLiteral" | "StringInterpolation"
            | "AdjacentStrings" | "SymbolLiteral" | "RecordLiteral" => Ok(false),
            "AssignmentExpression" => {
                let (Some(&l), Some(&r)) = (f.first(), f.get(1)) else { return Ok(false) };
                if self.exits(l)? {
                    return Ok(true);
                }
                let between = &cx.source[cx.tree.nodes[l].end..cx.tree.nodes[r].start];
                let op = between.trim();
                if op.starts_with("&&=") || op.starts_with("||=") || op.starts_with("??=") {
                    return Ok(false);
                }
                if cx.kind(l) == "PropertyAccess" && self.text(l).contains("?.") {
                    let target = cx.children(l).first().copied();
                    let name = cx.children(l).last().copied();
                    if let (Some(a), Some(b)) = (target, name)
                        && cx.source[cx.tree.nodes[a].end..cx.tree.nodes[b].start].trim() == "?."
                    {
                        return Ok(false);
                    }
                }
                self.exits(r)
            }
            "BinaryExpression" => {
                let (Some(&l), Some(&r)) = (f.first(), f.get(1)) else { return Ok(false) };
                let op = cx.source[cx.tree.nodes[l].end..cx.tree.nodes[r].start].trim().to_string();
                if op == "||" {
                    if self.bool_literal(l) == Some(false) {
                        return self.exits(r);
                    }
                    return self.exits(l);
                }
                if op == "&&" {
                    if self.bool_literal(l) == Some(true) {
                        return self.exits(r);
                    }
                    return self.exits(l);
                }
                if op == "??" {
                    return self.exits(l);
                }
                Ok(self.exits(l)? || self.exits(r)?)
            }
            "Block" => self.stmts(&f),
            "BlockFunctionBody" => self.maybe(f.first().copied()),
            "BreakStatement" => {
                self.breaks = true;
                if let Some(&label) = f.first() {
                    let name = self.text(label).to_string();
                    if let Some(target) = self.label_target(n, &name) {
                        self.broken_labels.insert(target);
                    }
                }
                Ok(false)
            }
            "CascadeExpression" => {
                let Some((&target, sections)) = f.split_first() else { return Ok(false) };
                Ok(self.exits(target)? || self.exprs(sections)?)
            }
            "ConditionalExpression" => {
                let (Some(&c), Some(&t), Some(&e)) = (f.first(), f.get(1), f.get(2)) else { return Ok(false) };
                if self.exits(c)? {
                    return Ok(true);
                }
                Ok(self.exits(t)? && self.exits(e)?)
            }
            "ContinueStatement" => {
                self.continues = true;
                Ok(false)
            }
            "DoStatement" => {
                let (outside_q, outside_c) = (self.breaks, self.continues);
                self.breaks = false;
                self.continues = false;
                let r = (|| -> Result<bool, String> {
                    let body = self.maybe(f.first().copied())?;
                    let break_or_continue = self.breaks || self.continues;
                    if body && !break_or_continue {
                        return Ok(true);
                    }
                    let Some(&c) = f.get(1) else { return Ok(false) };
                    if self.exits(c)? {
                        return Ok(true);
                    }
                    if self.bool_literal(c) == Some(true) && !self.breaks {
                        return Ok(true);
                    }
                    Ok(false)
                })();
                self.breaks = outside_q;
                self.continues = outside_c;
                r
            }
            "ForElement" | "ForStatement" => {
                let outside = self.breaks;
                self.breaks = false;
                let r = self.for_exits(n, &f);
                self.breaks = outside;
                r
            }
            "FunctionExpressionInvocation" => {
                if self.maybe(f.first().copied())? {
                    return Ok(true);
                }
                let list = f.iter().copied().find(|&k| cx.kind(k) == "ArgumentList");
                self.maybe(list)
            }
            "FunctionReference" => self.maybe(f.first().copied()),
            "IfElement" | "IfStatement" => {
                let Some(&c) = f.first() else { return Ok(false) };
                let then = f.get(1).copied();
                let else_ = f.get(2).copied();
                if self.exits(c)? {
                    return Ok(true);
                }
                match self.bool_literal(c) {
                    Some(true) => return self.maybe(then),
                    Some(false) if else_.is_some() => return self.maybe(else_),
                    _ => {}
                }
                let a = self.maybe(then)?;
                let b = self.maybe(else_)?;
                if else_.is_none() {
                    return Ok(false);
                }
                Ok(a && b)
            }
            "IndexExpression" => {
                let target = if f.len() >= 2 { Some(f[0]) } else { self.cascade_target(n) };
                if self.maybe(target)? {
                    return Ok(true);
                }
                self.maybe(f.last().copied())
            }
            "InstanceCreationExpression" => self.maybe(f.iter().copied().find(|&k| cx.kind(k) == "ArgumentList")),
            "LabeledStatement" => {
                let body = f.last().copied();
                let r = self.maybe(body);
                let broken = body.is_some_and(|c| self.broken_labels.contains(&c));
                if let Some(c) = body {
                    self.broken_labels.remove(&c);
                }
                Ok(r? && !broken)
            }
            "ListLiteral" | "SetOrMapLiteral" => {
                for k in f.iter().copied().filter(|&k| cx.kind(k) != "TypeArgumentList") {
                    if self.exits(k)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            "MapLiteralEntry" => Ok(self.maybe(f.first().copied())? || self.maybe(f.get(1).copied())?),
            "MethodInvocation" => {
                let name = cx.method_name(n);
                let explicit = f.first().copied().filter(|&a| Some(a) != name);
                let target = explicit.or_else(|| self.cascade_target(n));
                if let Some(a) = target {
                    if self.exits(a)? {
                        return Ok(true);
                    }
                    let null = match (explicit, name) {
                        (Some(x), Some(m)) => cx.source[cx.tree.nodes[x].end..cx.tree.nodes[m].start].trim().starts_with('?'),
                        (None, Some(m)) => cx.source[cx.tree.nodes[n].start..cx.tree.nodes[m].start].trim().starts_with('?'),
                        _ => false,
                    };
                    if null {
                        return Ok(false);
                    }
                }
                if let Some(m) = name
                    && (self.element)(m)
                {
                    return Ok(true);
                }
                self.maybe(f.iter().copied().find(|&k| cx.kind(k) == "ArgumentList"))
            }
            "NamedExpression" => self.maybe(f.get(1).copied()),
            "PropertyAccess" => {
                let target = if f.len() >= 2 { Some(f[0]) } else { self.cascade_target(n) };
                self.maybe(target)
            }
            "RethrowExpression" | "ReturnStatement" | "ThrowExpression" => Ok(true),
            "SwitchCase" | "SwitchDefault" | "SwitchPatternCase" => {
                let cmds: Vec<usize> = f.iter().copied().filter(|&k| is_stmt_kind(cx.kind(k))).collect();
                self.stmts(&cmds)
            }
            "SwitchExpression" => {
                for k in f.iter().copied().filter(|&k| cx.kind(k) == "SwitchExpressionCase") {
                    if !self.exits(k)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            "SwitchExpressionCase" => {
                // The guard (`when`) and the expression.
                let guard = f.first().and_then(|&g| cx.children(g).iter().copied().find(|&w| cx.kind(w) == "WhenClause"));
                let condition = guard.and_then(|w| cx.children(w).first().copied());
                Ok(self.maybe(condition)? || self.maybe(f.last().copied())?)
            }
            "SwitchStatement" => {
                let outside = self.breaks;
                self.breaks = false;
                let r = (|| -> Result<bool, String> {
                    let members: Vec<usize> = f.iter().copied().filter(|&k| matches!(cx.kind(k), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase")).collect();
                    let mut has_default = false;
                    let mut case_that_does_not_exit = false;
                    for (i, &m) in members.iter().enumerate() {
                        let empty = !cx.children(m).iter().any(|&k| is_stmt_kind(cx.kind(k)));
                        if cx.kind(m) == "SwitchDefault" {
                            has_default = true;
                            if empty && i + 1 == members.len() {
                                case_that_does_not_exit = true;
                                continue;
                            }
                        }
                        if !empty && !self.exits(m)? {
                            case_that_does_not_exit = true;
                        }
                    }
                    if case_that_does_not_exit {
                        return Ok(false);
                    }
                    Ok(has_default)
                })();
                self.breaks = outside;
                r
            }
            "TryStatement" => {
                let body = f.first().copied();
                let finally_ = f.last().copied().filter(|&k| f.len() > 1 && cx.kind(k) == "Block");
                if self.maybe(finally_)? {
                    return Ok(true);
                }
                if !self.maybe(body)? {
                    return Ok(false);
                }
                for c in f.iter().copied().filter(|&k| cx.kind(k) == "CatchClause") {
                    let b = cx.children(c).last().copied();
                    if !self.maybe(b)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            "VariableDeclaration" => self.maybe(f.first().copied()),
            "VariableDeclarationList" => {
                let vs: Vec<usize> = f.iter().copied().filter(|&k| cx.kind(k) == "VariableDeclaration").collect();
                for &v in vs.iter().rev() {
                    if self.exits(v)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            "VariableDeclarationStatement" => {
                let Some(list) = f.iter().copied().find(|&k| cx.kind(k) == "VariableDeclarationList") else { return Ok(false) };
                for v in cx.children(list).iter().copied().filter(|&k| cx.kind(k) == "VariableDeclaration") {
                    if self.exits(v)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            "WhileStatement" => {
                let outside = self.breaks;
                self.breaks = false;
                let r = (|| -> Result<bool, String> {
                    let Some(&c) = f.first() else { return Ok(false) };
                    if self.exits(c)? {
                        return Ok(true);
                    }
                    let _ = self.maybe(f.get(1).copied())?;
                    if self.bool_literal(c) == Some(true) && !self.breaks {
                        return Ok(true);
                    }
                    Ok(false)
                })();
                self.breaks = outside;
                r
            }
            "YieldStatement" => self.maybe(f.first().copied()),
            other => Err(format!("Bad state: Missing a visit method for a node of type {other}Impl")),
        }
    }

    /// The parts of a `for` (`visitForStatement`/`visitForElement`).
    fn for_exits(&mut self, n: usize, f: &[usize]) -> Result<bool, String> {
        let cx = self.cx;
        let Some(&parts) = f.first() else { return Ok(false) };
        let body = f.get(1).copied();
        match cx.kind(parts) {
            "ForEachPartsWithDeclaration" | "ForEachPartsWithIdentifier" | "ForEachPartsWithPattern" => {
                let iterable = cx.children(parts).last().copied();
                let r = self.maybe(iterable)?;
                let _ = self.maybe(body)?;
                Ok(r)
            }
            "ForPartsWithDeclarations" | "ForPartsWithExpression" | "ForPartsWithPattern" => {
                if cx.kind(n) == "ForStatement" && cx.kind(parts) == "ForPartsWithPattern" {
                    return Err("UnimplementedError".to_string());
                }
                let Some(p) = cx.tree.for_parts.get(&parts).cloned() else { return Ok(false) };
                if let Some(i) = p.initialization {
                    match cx.kind(parts) {
                        "ForPartsWithDeclarations" => {
                            if self.exits(i)? {
                                return Ok(true);
                            }
                        }
                        "ForPartsWithExpression"
                            if self.exits(i)? => {
                                return Ok(true);
                            }
                        _ => {}
                    }
                }
                if self.maybe(p.condition)? {
                    return Ok(true);
                }
                if self.exprs(&p.updaters)? {
                    return Ok(true);
                }
                let back = self.maybe(body)?;
                let true_ = match p.condition {
                    None => true,
                    Some(c) => self.bool_literal(c) == Some(true),
                };
                if true_ && (back || !self.breaks) {
                    return Ok(true);
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    /// `realTarget` of a cascade section without a written target: the
    /// target of the `CascadeExpression`.
    fn cascade_target(&self, n: usize) -> Option<usize> {
        let cx = self.cx;
        let mut k = n;
        while let Some(p) = cx.parent(k) {
            if cx.kind(p) == "CascadeExpression" {
                return cx.children(p).first().copied().filter(|&a| a != k);
            }
            if !matches!(cx.kind(p), "MethodInvocation" | "PropertyAccess" | "IndexExpression" | "AssignmentExpression") {
                return None;
            }
            k = p;
        }
        None
    }

    /// The statement labeled `name` that contains the `break`.
    fn label_target(&self, n: usize, name: &str) -> Option<usize> {
        let cx = self.cx;
        for k in cx.with_parents(n) {
            if cx.kind(k) == "LabeledStatement" {
                let labels: Vec<String> = cx
                    .children(k)
                    .iter()
                    .filter(|&&l| cx.kind(l) == "Label")
                    .filter_map(|&l| cx.children(l).first().map(|&i| cx.node_text(i).to_string()))
                    .collect();
                if labels.iter().any(|r| r == name) {
                    return cx.children(k).last().copied();
                }
            }
            if cx.kind(k) == "SwitchStatement" {
                // A labeled `case`.
                for m in cx.children(k).iter().copied() {
                    let has = cx
                        .children(m)
                        .iter()
                        .filter(|&&l| cx.kind(l) == "Label")
                        .any(|&l| cx.children(l).first().is_some_and(|&i| cx.node_text(i) == name));
                    if has {
                        return Some(m);
                    }
                }
            }
        }
        None
    }
}
