//! Conditional expression assists, ported from the `analysis_server` 3.6.2
//! producers over the analyzer-shaped tree.
//!
//! | Title | Kind | Producer |
//! |---|---|---|
//! | `Replace conditional with 'if-else'` | `refactor.convert.conditionalToIfElse` | `ReplaceConditionalWithIfElse` |
//! | `Split && condition` | `refactor.splitIfConjunction` | `SplitAndCondition` |
//! | `Convert to use '?.'` | `refactor.convert.toNullAware` | `ConvertToNullAware` |
//! | `Convert to 'if-case' statement` | `refactor.convert.ifCaseStatement` | `ConvertToIfCaseStatement` |
//! | `Convert to 'if-case' statement chain` | `refactor.convert.ifCaseStatementChain` | `ConvertToIfCaseStatementChain` |

use crate::actions::CodeAction;
use crate::Edit;
use crate::refactorings::Context;
use crate::refactorings_exec::{Change, Text, ONE_INDENT};
use dartforge_diagnostics::Span;

impl Context<'_> {
    /// `ReplaceConditionalWithIfElse` (replace_conditional_with_if_else.dart):
    /// in the statement enclosing the selection node, `T v = c ? a : b;`,
    /// `v = c ? a : b;` or `return c ? a : b;` becomes an `if`/`else`.
    pub(crate) fn conditional_to_if_else(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let command = self.with_parents(no).find(|&k| self.is_command(k))?;
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let prefix = self.node_prefix(command);
        let mut m = Change::default();
        let parts = |cx: &Self, conditional: usize| -> Option<(String, String, String)> {
            let f = cx.children(conditional);
            Some((cx.node_text(*f.first()?).to_string(), cx.node_text(*f.get(1)?).to_string(), cx.node_text(*f.get(2)?).to_string()))
        };
        match self.kind(command) {
            "VariableDeclarationStatement" => {
                let list = *self.children(command).first()?;
                let has_type = self.children(list).iter().any(|&k| self.kind(k) != "VariableDeclaration" && self.kind(k) != "Annotation");
                let mut import = std::collections::BTreeSet::new();
                for &variable in self.children(list).iter().filter(|&&k| self.kind(k) == "VariableDeclaration") {
                    let Some(&conditional) = self.children(variable).first() else { continue };
                    if self.kind(conditional) != "ConditionalExpression" {
                        continue;
                    }
                    let name = self.next_token(self.tree.nos[variable].inicio)?;
                    if !has_type {
                        let mut writer = crate::write_type::Writer::new(self, name.start);
                        let ty = self.node_expr(conditional).and_then(|x| self.bodies.get_type(x));
                        let written = writer.write_type(ty, false).unwrap_or_default();
                        import.extend(writer.import.iter().copied());
                        // The list's keyword (`late` apart).
                        let mut k = self.next_token(self.tree.nos[list].inicio)?;
                        if &self.source[k.start..k.end] == "late" {
                            k = self.next_token(k.end)?;
                        }
                        if &self.source[k.start..k.end] == "var" {
                            m.add(uri, k, written);
                        } else {
                            m.add(uri, Span { start: name.start, end: name.start }, format!("{written} "));
                        }
                    }
                    // `range.endEnd(variable.name, conditional)`.
                    m.add(uri, Span { start: name.end, end: self.tree.nos[conditional].fim }, String::new());
                    let (c, a, b) = parts(self, conditional)?;
                    let n = &self.source[name.start..name.end];
                    let src = format!(
                        "{eol}{prefix}if ({c}) {{{eol}{prefix}{ONE_INDENT}{n} = {a};{eol}{prefix}}} else {{{eol}{prefix}{ONE_INDENT}{n} = {b};{eol}{prefix}}}"
                    );
                    let command_end = self.tree.nos[command].fim;
                    m.add(uri, Span { start: command_end, end: command_end }, src);
                }
                crate::refactorings_move::builder_imports(self, &mut m, &import);
            }
            "ExpressionStatement" => {
                let assignment = *self.children(command).first()?;
                if self.kind(assignment) != "AssignmentExpression" {
                    return None;
                }
                let f = self.children(assignment);
                let (&left, &conditional) = (f.first()?, f.get(1)?);
                let operator = self.source[self.tree.nos[left].fim..self.tree.nos[conditional].inicio].trim();
                if operator != "=" || self.kind(conditional) != "ConditionalExpression" {
                    return None;
                }
                let (c, a, b) = parts(self, conditional)?;
                let n = self.node_text(left);
                let src = format!(
                    "if ({c}) {{{eol}{prefix}{ONE_INDENT}{n} = {a};{eol}{prefix}}} else {{{eol}{prefix}{ONE_INDENT}{n} = {b};{eol}{prefix}}}"
                );
                m.add(uri, self.tree.span(command), src);
            }
            "ReturnStatement" => {
                let &conditional = self.children(command).first()?;
                if self.kind(conditional) != "ConditionalExpression" {
                    return None;
                }
                let (c, a, b) = parts(self, conditional)?;
                let src = format!(
                    "if ({c}) {{{eol}{prefix}{ONE_INDENT}return {a};{eol}{prefix}}} else {{{eol}{prefix}{ONE_INDENT}return {b};{eol}{prefix}}}"
                );
                m.add(uri, self.tree.span(command), src);
            }
            _ => return None,
        }
        if m.conflict.is_some() || m.files.is_empty() {
            return None;
        }
        Some(CodeAction {
            title: "Replace conditional with 'if-else'".into(),
            kind: "refactor.convert.conditionalToIfElse".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `isOperatorSelected` (`CorrectionProducer`): the selection lies between
    /// the operands, or is exactly the expression when no operand is binary.
    pub(crate) fn selected_operator(&self, binary: usize, start: usize, end: usize) -> bool {
        let f = self.children(binary);
        let (Some(&left), Some(&right)) = (f.first(), f.get(1)) else { return false };
        let (e, d) = (&self.tree.nos[left], &self.tree.nos[right]);
        if start >= e.fim && end <= d.inicio {
            return true;
        }
        if start == e.inicio && end == d.fim {
            return self.kind(left) != "BinaryExpression" && self.kind(right) != "BinaryExpression";
        }
        false
    }

    /// The operator of a binary expression (the text between the operands).
    pub(crate) fn binary_operator(&self, binary: usize) -> Option<&str> {
        let f = self.children(binary);
        let (e, d) = (*f.first()?, *f.get(1)?);
        Some(self.source[self.tree.nos[e].fim..self.tree.nos[d].inicio].trim())
    }

    /// `SplitAndCondition` (split_and_condition.dart): with the top-level `&&`
    /// of the condition of an `if` without `else` selected, the right part
    /// becomes an `if` nested around the `then`, which gets one more level of
    /// indentation.
    pub(crate) fn split_and_condition(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let binary = self.tree.localizar(start, end)?;
        if self.kind(binary) != "BinaryExpression" || !self.selected_operator(binary, start, end) || self.binary_operator(binary)? != "&&" {
            return None;
        }
        let command = self.with_parents(binary).find(|&k| self.is_command(k))?;
        if self.kind(command) != "IfStatement" {
            return None;
        }
        let if_commands: Vec<usize> = self.children(command).iter().copied().filter(|&k| self.is_command(k)).collect();
        if if_commands.len() != 1 {
            // No `else` support.
            return None;
        }
        let then = if_commands[0];
        // The top-level condition: walks up through the `&&`s.
        let mut condition = binary;
        while let Some(p) = self.parent(condition) {
            if self.kind(p) == "BinaryExpression" && self.binary_operator(p) == Some("&&") {
                condition = p;
            } else {
                break;
            }
        }
        if self.children(command).first() != Some(&condition) {
            return None;
        }
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let prefix = self.node_prefix(command);
        let f = self.children(binary);
        let (left, right) = (f[0], f[1]);
        let right_source = &self.source[self.tree.nos[right].inicio..self.tree.nos[condition].fim];
        let mut m = Change::default();
        // Removes "&& right".
        m.add(uri, Span { start: self.tree.nos[left].fim, end: self.tree.nos[condition].fim }, String::new());
        let commands: Vec<usize> = if self.kind(then) == "Block" {
            let block = self.tree.span(then);
            m.add(uri, Span { start: block.start + 1, end: block.start + 1 }, format!("{eol}{prefix}{ONE_INDENT}if ({right_source}) {{"));
            m.add(uri, Span { start: block.end - 1, end: block.end - 1 }, format!("{ONE_INDENT}}}{eol}{prefix}"));
            self.children(then).to_vec()
        } else {
            let closes = self.previous_token(self.tree.nos[then].inicio)?;
            m.add(uri, Span { start: closes.start + 1, end: closes.start + 1 }, format!("{eol}{prefix}{ONE_INDENT}if ({right_source})"));
            vec![then]
        };
        // The indentation of the `then` statements (`getLinesRangeStatements`,
        // which fails without statements).
        let (&first, &last) = (commands.first()?, commands.last()?);
        let lines = tx.line_range(self.tree.nos[first].inicio, self.tree.nos[last].fim);
        let old = format!("{prefix}{ONE_INDENT}");
        let new = format!("{old}{ONE_INDENT}");
        m.add(uri, lines, tx.swap_indent(&self.source[lines.start..lines.end], &old, &new, true, true));
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Split && condition".into(),
            kind: "refactor.splitIfConjunction".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `unParenthesized`.
    pub(crate) fn without_parens(&self, mut n: usize) -> usize {
        while self.kind(n) == "ParenthesizedExpression" {
            match self.children(n).first() {
                Some(&f) => n = f,
                None => break,
            }
        }
        n
    }

    /// The `toString` of an expression (the `toSource`).
    fn as_text(&self, n: usize) -> String {
        crate::assist_loops::as_source(self.node_text(n))
    }

    /// The access operator (`.`, `?.`, `..`) between the target `target` and the
    /// following name.
    fn access_operator(&self, target: usize) -> Option<Span> {
        let s = self.next_token(self.tree.nos[target].fim)?;
        matches!(&self.source[s.start..s.end], "." | "?." | ".." | "?..").then_some(s)
    }

    /// `ConvertToNullAware` (convert_to_null_aware.dart): `x == null ? null :
    /// x.y` (or with `!=`) becomes `x?.y`.
    pub(crate) fn convert_to_null_aware(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let mut target = no;
        if let Some(p) = self.parent(no)
            && self.kind(p) == "BinaryExpression"
            && let Some(a) = self.parent(p)
            && self.kind(a) == "ConditionalExpression"
        {
            target = a;
        }
        if self.kind(target) != "ConditionalExpression" {
            return None;
        }
        let f = self.children(target);
        let (condition, then, else_) = (self.without_parens(*f.first()?), *f.get(1)?, *f.get(2)?);
        if self.kind(condition) != "BinaryExpression" {
            return None;
        }
        let fc = self.children(condition);
        let (left, right) = (*fc.first()?, *fc.get(1)?);
        let null = |k: usize| self.kind(k) == "NullLiteral";
        let condition_text = if null(left) && !null(right) {
            self.as_text(right)
        } else if null(right) && !null(left) {
            self.as_text(left)
        } else {
            return None;
        };
        let (null_expression, non_null) = match self.binary_operator(condition)? {
            "==" => (then, else_),
            "!=" => (else_, then),
            _ => return None,
        };
        if !null(self.without_parens(null_expression)) {
            return None;
        }
        let mut result = self.without_parens(non_null);
        let mut operator: Option<Span> = None;
        loop {
            match self.kind(result) {
                "PrefixedIdentifier" => {
                    let prefix = *self.children(result).first()?;
                    operator = Some(self.access_operator(prefix)?);
                    result = prefix;
                }
                "MethodInvocation" | "PropertyAccess" => {
                    // Without a target, the producer's `default`.
                    let first = *self.children(result).first()?;
                    let op = self.access_operator(first)?;
                    if op.start < self.tree.nos[first].fim {
                        return None;
                    }
                    if self.kind(result) == "MethodInvocation" && self.method_name(result) == Some(first) {
                        return None;
                    }
                    operator = Some(op);
                    result = first;
                }
                "PostfixExpression" if self.source[self.tree.nos[result].inicio..self.tree.nos[result].fim].ends_with('!') => {
                    result = *self.children(result).first()?;
                }
                _ => return None,
            }
            if self.as_text(result) == condition_text {
                break;
            }
        }
        let (range, question) = match operator {
            Some(op) => {
                let q = if &self.source[op.start..op.end] == "." { "?" } else { "" };
                (Span { start: self.tree.nos[result].fim, end: op.start }, q)
            }
            None if self.parent(result).is_some_and(|p| self.kind(p) == "PostfixExpression") => {
                (Span { start: self.tree.nos[result].fim, end: self.tree.nos[self.parent(result)?].fim }, "")
            }
            None => return None,
        };
        let mut m = Change::default();
        m.add(uri, Span { start: self.tree.nos[target].inicio, end: self.tree.nos[non_null].inicio }, String::new());
        m.add(uri, range, question.to_string());
        m.add(uri, Span { start: self.tree.nos[non_null].fim, end: self.tree.nos[target].fim }, String::new());
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Convert to use '?.'".into(),
            kind: "refactor.convert.toNullAware".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// The library has patterns (Dart 3.0).
    pub(crate) fn with_patterns(&self) -> bool {
        let prog = self.p.program();
        prog.library(prog.unit(self.unit).library).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0)
    }

    /// Some identifier of `n` reads the local declared at `name`.
    pub(crate) fn cites_local(&self, n: usize, name: usize) -> bool {
        let mut stack = vec![n];
        while let Some(k) = stack.pop() {
            if self.kind(k) == "SimpleIdentifier"
                && let crate::arvore_analyzer::Marca::Expr(x) = self.tree.nos[k].marca
                && matches!(self.bodies.get_resolved(x), Some(dartforge_types::Resolved::Local(_)))
                && self.bodies.declaracao_local(x) == Some(name)
            {
                return true;
            }
            stack.extend(self.children(k).iter().copied());
        }
        false
    }

    /// `asSingleVariableDeclaration`: the statement declares a single local with
    /// an initializer; the name, the initializer and whether it is `final`.
    pub(crate) fn single_declaration(&self, command: usize) -> Option<(Span, usize, bool)> {
        if self.kind(command) != "VariableDeclarationStatement" {
            return None;
        }
        let list = *self.children(command).first()?;
        let variables: Vec<usize> = self.children(list).iter().copied().filter(|&k| self.kind(k) == "VariableDeclaration").collect();
        let [variable] = variables[..] else { return None };
        let name = self.next_token(self.tree.nos[variable].inicio)?;
        self.bodies.tipos_de_locais.get(&name.start)?;
        let initializer = *self.children(variable).first()?;
        let final_ = self.tokens.iter().any(|t| {
            t.span.start >= self.tree.nos[list].inicio && t.span.end <= name.start && matches!(&self.source[t.span.start..t.span.end], "final" | "const")
        });
        Some((name, initializer, final_))
    }

    /// `ConvertToIfCaseStatement` (convert_to_if_case_statement.dart): the local
    /// declared in the statement before the `if` and tested with `is T` or
    /// `!= null`, with no read after the `then`, moves into the `if-case`.
    pub(crate) fn convert_to_if_case(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        if !self.with_patterns() {
            return None;
        }
        let command = self.tree.localizar(start, end)?;
        if self.kind(command) != "IfStatement" {
            return None;
        }
        let block = self.parent(command)?;
        if self.kind(block) != "Block" {
            return None;
        }
        let commands = self.children(block);
        let index = commands.iter().position(|&c| c == command)?;
        let previous = *commands.get(index.checked_sub(1)?)?;
        let (name, initializer, final_) = self.single_declaration(previous)?;
        let name_text = &self.source[name.start..name.end];
        let if_children = self.children(command);
        let condition = *if_children.first()?;
        let branches: Vec<usize> = if_children.iter().copied().filter(|&k| self.is_command(k)).collect();
        let references_after = || branches.get(1).is_some_and(|&e| self.cites_local(e, name.start)) || commands[index + 1..].iter().any(|&c| self.cites_local(c, name.start));
        let pattern = match self.kind(condition) {
            "IsExpression" => {
                let f = self.children(condition);
                let (left, ty) = (*f.first()?, *f.get(1)?);
                if self.kind(left) != "SimpleIdentifier" || self.node_text(left) != name_text || references_after() {
                    return None;
                }
                format!("{}{} {name_text}", if final_ { "final " } else { "" }, self.node_text(ty))
            }
            "BinaryExpression" => {
                let f = self.children(condition);
                let (left, right) = (*f.first()?, *f.get(1)?);
                if self.binary_operator(condition)? != "!="
                    || self.kind(right) != "NullLiteral"
                    || self.kind(left) != "SimpleIdentifier"
                    || self.node_text(left) != name_text
                    || references_after()
                {
                    return None;
                }
                format!("{} {name_text}?", if final_ { "final" } else { "var" })
            }
            _ => return None,
        };
        let edits = vec![
            Edit { uri: uri.to_string(), span: self.tree.span(condition), text: format!("{} case {pattern}", self.node_text(initializer)) },
            Edit { uri: uri.to_string(), span: Span { start: self.tree.nos[previous].inicio, end: self.tree.nos[command].inicio }, text: String::new() },
        ];
        Some(CodeAction {
            title: "Convert to 'if-case' statement".into(),
            kind: "refactor.convert.ifCaseStatement".into(),
            edits,
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `SwitchStatement.memberGroups`: the members joined up to the first one
    /// that has statements; each group with its statements.
    pub(crate) fn member_groups(&self, switch: usize) -> Vec<(Vec<usize>, Vec<usize>)> {
        let mut groups = Vec::new();
        let mut members = Vec::new();
        for &m in self.children(switch).iter().filter(|&&k| matches!(self.kind(k), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase")) {
            members.push(m);
            let commands: Vec<usize> = self.children(m).iter().copied().filter(|&k| self.is_command(k)).collect();
            if !commands.is_empty() {
                groups.push((std::mem::take(&mut members), commands));
            }
        }
        if !members.is_empty() {
            groups.push((members, Vec::new()));
        }
        groups
    }

    /// `_writeStatements`: the statements (without the final `break`), indented
    /// one level inside `indent`.
    pub(crate) fn indented_commands(&self, commands: &[usize], indent: &str) -> String {
        let mut commands = commands;
        if commands.last().is_some_and(|&c| self.kind(c) == "BreakStatement") {
            commands = &commands[..commands.len() - 1];
        }
        let (Some(&first), Some(&last)) = (commands.first(), commands.last()) else { return String::new() };
        let tx = Text::new(self.source);
        let range = tx.line_range(self.tree.nos[first].inicio, self.tree.nos[last].fim);
        let first_indent = tx.line_prefix(self.tree.nos[first].inicio);
        tx.swap_indent(&self.source[range.start..range.end], first_indent, &format!("{indent}{ONE_INDENT}"), true, true)
    }

    /// `ConvertToIfCaseStatementChain`: a `switch` of pattern cases becomes an
    /// `if-case` chain, with the `default` as the `else`.
    pub(crate) fn switch_to_if_case_chain(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let switch = self.tree.localizar(start, end)?;
        if self.kind(switch) != "SwitchStatement" {
            return None;
        }
        let groups = self.member_groups(switch);
        if groups.is_empty() {
            return None;
        }
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let indent = tx.line_prefix(self.tree.nos[switch].inicio).to_string();
        let expression = self.node_text(*self.children(switch).first()?).to_string();
        let mut s = String::new();
        for (i, (members, commands)) in groups.iter().enumerate() {
            if i > 0 {
                s.push_str(" else ");
            }
            if members.iter().any(|&m| self.kind(m) == "SwitchDefault") {
                if members.len() != 1 {
                    return None;
                }
                s.push_str(&format!("{{{eol}"));
            } else {
                let saved: Vec<usize> = members
                    .iter()
                    .filter(|&&m| self.kind(m) == "SwitchPatternCase")
                    .filter_map(|&m| self.children(m).iter().copied().find(|&k| self.kind(k) == "GuardedPattern"))
                    .collect();
                if saved.len() != members.len() {
                    return None;
                }
                let pattern = if let [g] = saved[..] {
                    self.node_text(g).to_string()
                } else {
                    if saved.iter().any(|&g| self.children(g).iter().any(|&k| self.kind(k) == "WhenClause")) {
                        return None;
                    }
                    saved.iter().filter_map(|&g| self.children(g).first().map(|&p| self.node_text(p).to_string())).collect::<Vec<_>>().join(" || ")
                };
                s.push_str(&format!("if ({expression} case {pattern}) {{{eol}"));
            }
            s.push_str(&self.indented_commands(commands, &indent));
            s.push_str(&format!("{indent}}}"));
        }
        Some(CodeAction {
            title: "Convert to 'if-case' statement chain".into(),
            kind: "refactor.convert.ifCaseStatementChain".into(),
            edits: vec![Edit { uri: uri.to_string(), span: self.tree.span(switch), text: s }],
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }
}
