//! `switch` assists, ported from the `analysis_server` 3.6.2 producers over
//! the analyzer-shaped tree.
//!
//! | Title | Kind | Producer |
//! |---|---|---|
//! | `Convert to switch statement` | `refactor.convert.switchStatement` | `ConvertIfStatementToSwitchStatement`, `ConvertSwitchExpressionToSwitchStatement` |

use crate::actions::CodeAction;
use crate::refactorings::Context;
use crate::refactorings_exec::{Change, Text, ONE_INDENT};
use dartforge_diagnostics::Span;
use dartforge_frontend::ast;
use dartforge_types::{Type, TypeId};

/// A branch of the `if`: the pattern (`None` for the `else`) and the statement.
struct Case {
    pattern: Option<String>,
    expression: Option<String>,
    command: usize,
}

const LITERALS: &[&str] = &[
    "IntegerLiteral",
    "DoubleLiteral",
    "BooleanLiteral",
    "NullLiteral",
    "SimpleStringLiteral",
    "StringInterpolation",
    "AdjacentStrings",
    "SymbolLiteral",
    "ListLiteral",
    "SetOrMapLiteral",
    "RecordLiteral",
];

impl Context<'_> {
    /// The `type` of a type annotation (`NamedType.type`).
    pub(crate) fn annotation_type_at(&self, k: usize) -> Option<TypeId> {
        let s = self.tree.span(k);
        self.ast.types.iter().enumerate().filter(|(_, t)| t.span == s).find_map(|(i, _)| {
            let id = ast::TypeId(i as u32);
            self.bodies.tipos_de_anotacoes.get(&id).or_else(|| self.p.query.outline.tipos_escritos.get(&(self.unit, id))).copied()
        })
    }

    /// `_patternOfBoolCondition`.
    fn condition_pattern(&self, c: usize) -> Option<(String, String)> {
        match self.kind(c) {
            "BinaryExpression" => {
                let f = self.children(c);
                let (e, d) = (*f.first()?, *f.get(1)?);
                let op = self.binary_operator(c)?;
                if op == "!=" && self.kind(d) == "NullLiteral" {
                    return Some((self.node_text(e).to_string(), "_?".into()));
                }
                if matches!(op, "<" | ">" | "<=" | ">=") && LITERALS.contains(&self.kind(d)) {
                    let start_op = self.next_token(self.tree.nos[e].fim)?.start;
                    return Some((self.node_text(e).to_string(), self.source[start_op..self.tree.nos[d].fim].to_string()));
                }
                None
            }
            "IsExpression" => {
                let f = self.children(c);
                let (e, t) = (*f.first()?, *f.get(1)?);
                let ty = self.node_text(t);
                let interface = self.annotation_type_at(t).is_some_and(|x| {
                    matches!(self.p.query.table.get(x), Type::Interface { .. } | Type::FutureOr { .. } | Type::ExtensionType { .. } | Type::Null)
                });
                Some((self.node_text(e).to_string(), if interface { format!("{ty}()") } else { format!("{ty} _") }))
            }
            _ => None,
        }
    }

    /// `_buildCases`.
    fn if_cases(&self, if_node: usize) -> Option<Vec<Case>> {
        let children = self.children(if_node);
        let condition = *children.first()?;
        let branches: Vec<usize> = children.iter().copied().filter(|&k| self.is_command(k)).collect();
        let then = *branches.first()?;
        // `_buildThenCase`.
        let first = match children.iter().copied().find(|&k| self.kind(k) == "CaseClause") {
            Some(clause) => {
                if self.kind(condition) != "SimpleIdentifier" {
                    return None;
                }
                let saved = *self.children(clause).first()?;
                Case { pattern: Some(self.node_text(saved).to_string()), expression: Some(self.node_text(condition).to_string()), command: then }
            }
            None => {
                let (e, p) = self.condition_pattern(condition)?;
                Case { pattern: Some(p), expression: Some(e), command: then }
            }
        };
        let expression = first.expression.clone();
        let mut cases = vec![first];
        match branches.get(1) {
            Some(&else_) if self.kind(else_) == "IfStatement" => {
                for c in self.if_cases(else_)? {
                    if c.pattern.is_some() && c.expression != expression {
                        return None;
                    }
                    cases.push(c);
                }
            }
            Some(&else_) => cases.push(Case { pattern: None, expression: None, command: else_ }),
            None => {}
        }
        Some(cases)
    }

    /// `selfOrBlockStatements`.
    fn commands_of(&self, c: usize) -> Vec<usize> {
        if self.kind(c) == "Block" { self.children(c).to_vec() } else { vec![c] }
    }

    /// `ConvertIfStatementToSwitchStatement`.
    pub(crate) fn if_to_switch(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        if !self.with_patterns() {
            return None;
        }
        let if_node = self.tree.localizar(start, end)?;
        if self.kind(if_node) != "IfStatement" {
            return None;
        }
        let cases = self.if_cases(if_node)?;
        let expression = cases.first()?.expression.clone()?;
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let indent = tx.line_prefix(self.tree.nos[if_node].inicio).to_string();
        let case_indent = format!("{indent}{ONE_INDENT}");
        let mut s = format!("switch ({expression}) {{{eol}");
        for (i, c) in cases.iter().enumerate() {
            match &c.pattern {
                Some(p) => s.push_str(&format!("{case_indent}case {p}:{eol}")),
                None => s.push_str(&format!("{case_indent}default:{eol}")),
            }
            // `_writeStatement`: `getLinesRangeStatements` fails without statements.
            let commands = self.commands_of(c.command);
            let (&first, &last) = (commands.first()?, commands.last()?);
            let _ = i;
            let range = tx.line_range(self.tree.nos[first].inicio, self.tree.nos[last].fim);
            s.push_str(&tx.swap_indent(&self.source[range.start..range.end], &indent, &case_indent, true, true));
        }
        s.push_str(&format!("{indent}}}"));
        Some(CodeAction {
            title: "Convert to switch statement".into(),
            kind: "refactor.convert.switchStatement".into(),
            edits: vec![crate::Edit { uri: uri.to_string(), span: self.tree.span(if_node), text: s }],
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `_rewriteCases`.
    fn rewrite_cases(&self, uri: &str, m: &mut Change, switch: usize, indent: &str, before: &str, semicolon: Span) {
        let eol = Text::new(self.source).eol();
        for &case in self.children(switch).iter().filter(|&&k| self.kind(k) == "SwitchExpressionCase") {
            let f = self.children(case);
            let (Some(&saved), Some(&expression)) = (f.first(), f.get(1)) else { continue };
            let g = self.children(saved);
            let wildcard = g.len() == 1 && self.kind(g[0]) == "WildcardPattern" && self.children(g[0]).is_empty();
            if wildcard {
                m.add(uri, self.tree.span(saved), "default");
            } else {
                let i = self.tree.nos[saved].inicio;
                m.add(uri, Span { start: i, end: i }, "case ");
            }
            m.add(uri, Span { start: self.tree.nos[saved].fim, end: self.tree.nos[expression].inicio }, format!(":{eol}{indent}    {before}"));
            match self.next_token(self.tree.nos[expression].fim) {
                Some(v) if &self.source[v.start..v.end] == "," => m.add(uri, v, ";"),
                _ => {
                    let e = self.tree.nos[expression].fim;
                    m.add(uri, Span { start: e, end: e }, ";");
                }
            }
        }
        m.add(uri, semicolon, String::new());
    }

    /// `ConvertSwitchExpressionToSwitchStatement`: the `switch` expression of a
    /// declaration, an assignment or a `return` becomes a statement.
    pub(crate) fn switch_expression_to_statement(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let switch = self.tree.localizar(start, end)?;
        if self.kind(switch) != "SwitchExpression" {
            return None;
        }
        let parent = self.parent(switch)?;
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let mut m = Change::default();
        let mut import = std::collections::BTreeSet::new();
        match self.kind(parent) {
            "VariableDeclaration" => {
                let list = self.parent(parent)?;
                let command = self.parent(list)?;
                if self.kind(list) != "VariableDeclarationList"
                    || self.children(list).iter().filter(|&&k| self.kind(k) == "VariableDeclaration").count() != 1
                    || self.kind(command) != "VariableDeclarationStatement"
                {
                    return None;
                }
                let indent = tx.line_prefix(self.tree.nos[command].inicio).to_string();
                let name = self.next_token(self.tree.nos[parent].inicio)?;
                let has_type = self.children(list).iter().any(|&k| matches!(self.kind(k), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation"));
                if !has_type {
                    let mut k = self.next_token(self.tree.nos[list].inicio)?;
                    if &self.source[k.start..k.end] == "late" {
                        k = self.next_token(k.end)?;
                    }
                    let word = &self.source[k.start..k.end];
                    if matches!(word, "final" | "var" | "const") {
                        let mut writer = crate::write_type::Writer::new(self, k.start);
                        let ty = self.node_expr(switch).and_then(|x| self.bodies.get_type(x));
                        let written = writer.write_type(ty, false).unwrap_or_default();
                        import = writer.import;
                        if word == "final" {
                            m.add(uri, Span { start: name.start, end: name.start }, format!("{written} "));
                        } else {
                            m.add(uri, k, written);
                        }
                    }
                }
                m.add(uri, Span { start: name.end, end: self.tree.nos[switch].inicio }, format!(";{eol}{indent}"));
                let dot = self.previous_token(self.tree.nos[command].fim)?;
                self.rewrite_cases(uri, &mut m, switch, &indent, &format!("{} = ", &self.source[name.start..name.end]), dot);
            }
            "AssignmentExpression" => {
                let f = self.children(parent);
                if f.get(1) != Some(&switch) {
                    return None;
                }
                let variable = *f.first()?;
                let command = self.parent(parent)?;
                if self.kind(variable) != "SimpleIdentifier" || self.kind(command) != "ExpressionStatement" {
                    return None;
                }
                let dot = self.previous_token(self.tree.nos[command].fim).filter(|s| &self.source[s.start..s.end] == ";")?;
                let indent = tx.line_prefix(self.tree.nos[command].inicio).to_string();
                m.add(uri, Span { start: self.tree.nos[variable].inicio, end: self.tree.nos[switch].inicio }, String::new());
                self.rewrite_cases(uri, &mut m, switch, &indent, &format!("{} = ", self.node_text(variable)), dot);
            }
            "ReturnStatement" => {
                let indent = tx.line_prefix(self.tree.nos[parent].inicio).to_string();
                m.add(uri, Span { start: self.tree.nos[parent].inicio, end: self.tree.nos[switch].inicio }, String::new());
                let dot = self.previous_token(self.tree.nos[parent].fim)?;
                self.rewrite_cases(uri, &mut m, switch, &indent, "return ", dot);
            }
            _ => return None,
        }
        crate::refactorings_move::builder_imports(self, &mut m, &import);
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Convert to switch statement".into(),
            kind: "refactor.convert.switchStatement".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }
}
