//! `ConvertToSwitchExpression` (convert_to_switch_expression.dart) from
//! `analysis_server` 3.6.2: a `switch` statement whose cases only return,
//! only assign to the same local variable or only pass one argument to the
//! same top-level function becomes a `switch` expression.
//!
//! | Title | Kind |
//! |---|---|
//! | `Convert to switch expression` | `refactor.convert.switchExpression` |

use crate::actions::CodeAction;
use crate::refactorings::{Context, Elem};
use crate::refactorings_exec::{Text, ONE_INDENT};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassKind, FunctionKind};
use dartforge_types::{Type, TypeId, TypeTable};

/// `_Group`: the lone `default` or the pattern cases together.
enum Group {
    Pattern { member: usize, commands: Vec<usize> },
    Cases { cases: Vec<usize>, commands: Vec<usize> },
}

impl Group {
    fn commands(&self) -> &[usize] {
        match self {
            Group::Pattern { commands, .. } | Group::Cases { commands, .. } => commands,
        }
    }
}

/// `_SwitchType`.
enum SwitchType {
    Return,
    /// The variable name and the operator.
    Assignment(String, String),
    /// The function name.
    Argument(String),
}

impl Context<'_> {
    /// `TypeSystemImpl.isAlwaysExhaustive` from analyzer 6.11 (over a copy of
    /// the table: the *erasure* of an extension type may create types).
    fn always_exhaustive_in_lsp(&self, table: &mut TypeTable, t: TypeId, prof: u32) -> bool {
        if prof > 32 {
            return false;
        }
        let query = &self.p.query;
        let prog = self.p.program();
        match table.get(t).clone() {
            Type::Null => true,
            Type::Interface { class, .. } => {
                let c = prog.class(class);
                Some(class) == query.core.bool_class || c.kind == ClassKind::Enum || (c.kind == ClassKind::Class && c.modifiers.sealed)
            }
            Type::ExtensionType { .. } => {
                let deleted = {
                    let mut engine = dartforge_types::constantes::avaliador::Motor::novo(prog, &query.names, table, &query.core, &query.outline, &query.bodies, &self.p.libraries);
                    engine.apagar(t)
                };
                deleted != t && self.always_exhaustive_in_lsp(table, deleted, prof + 1)
            }
            Type::FutureOr { arg, .. } => self.always_exhaustive_in_lsp(table, arg, prof + 1),
            Type::Intersection { param, bound } => {
                self.always_exhaustive_in_lsp(table, bound, prof + 1) || {
                    let (explicit, limit) = (table.param(param).explicito, table.param(param).bound);
                    explicit && self.always_exhaustive_in_lsp(table, limit, prof + 1)
                }
            }
            Type::TypeParameter { param, .. } => {
                let (explicit, limit) = (table.param(param).explicito, table.param(param).bound);
                explicit && self.always_exhaustive_in_lsp(table, limit, prof + 1)
            }
            Type::Record { positional, named, .. } => {
                let fields: Vec<TypeId> = positional.iter().copied().chain(named.iter().map(|(_, c)| *c)).collect();
                fields.into_iter().all(|c| self.always_exhaustive_in_lsp(table, c, prof + 1))
            }
            _ => false,
        }
    }

    /// The `:` of a `switch` member.
    fn member_colon(&self, member: usize) -> Option<Span> {
        let after = match self.kind(member) {
            "SwitchDefault" => self.next_token(self.tree.nos[member].inicio)?.end,
            _ => {
                let saved = self.children(member).iter().copied().find(|&f| self.kind(f) == "GuardedPattern")?;
                self.tree.nos[saved].fim
            }
        };
        self.next_token(after).filter(|t| &self.source[t.start..t.end] == ":")
    }

    /// The node's last token (`endToken`).
    fn last_token(&self, n: usize) -> Option<Span> {
        self.previous_token(self.tree.nos[n].fim)
    }

    /// `statement.beginToken.precedingComments != null`.
    fn has_comment_before(&self, n: usize) -> bool {
        !self.comments_before(Span { start: self.tree.nos[n].inicio, end: self.tree.nos[n].inicio }).is_empty()
    }

    /// `_getBreakRange`: from the end of the first comment before the `break`
    /// (or of the previous token) to the end of the `break`.
    fn break_range(&self, b: usize) -> Option<Span> {
        let s = self.tree.nos[b].inicio;
        let before = match self.comments_before(Span { start: s, end: s }).first() {
            Some(c) => c.end,
            None => self.previous_token(s)?.end,
        };
        Some(Span { start: before, end: self.tree.nos[b].fim })
    }

    /// The pattern (the text) of the joined cases.
    fn pattern_text(&self, cases: &[usize]) -> Option<String> {
        let mut parts = Vec::new();
        for &c in cases {
            let saved = self.children(c).iter().copied().find(|&f| self.kind(f) == "GuardedPattern")?;
            parts.push(self.node_text(*self.children(saved).first()?));
        }
        Some(parts.join(" || "))
    }

    /// `_getSupportedSwitchType`.
    fn switch_type(&self, switch: usize) -> Option<(SwitchType, Vec<Group>)> {
        let member_groups = self.member_groups(switch);
        if member_groups.is_empty() {
            return None;
        }
        let prog = self.p.program();
        let mut can_return = true;
        let mut can_assign = true;
        let mut can_argument = true;
        let mut function: Option<(dartforge_elements::model::FunctionElementId, String)> = None;
        let mut write: Option<(usize, String, String)> = None;
        let mut groups = Vec::new();
        let how_many = member_groups.len();
        for (members, commands) in member_groups {
            if members.iter().any(|&m| self.children(m).iter().any(|&f| self.kind(f) == "Label")) {
                return None;
            }
            if let [m] = members[..]
                && self.kind(m) == "SwitchDefault"
            {
                groups.push(Group::Pattern { member: m, commands: commands.clone() });
            } else {
                if members.iter().any(|&m| self.kind(m) != "SwitchPatternCase") {
                    return None;
                }
                if members.len() != 1
                    && members.iter().any(|&m| {
                        self.children(m)
                            .iter()
                            .any(|&g| self.kind(g) == "GuardedPattern" && self.children(g).iter().any(|&w| self.kind(w) == "WhenClause"))
                    })
                {
                    return None;
                }
                groups.push(Group::Cases { cases: members.clone(), commands: commands.clone() });
            }
            if commands.is_empty() || commands.len() > 2 {
                return None;
            }
            if let [_, second] = commands[..] {
                if self.kind(second) != "BreakStatement" {
                    return None;
                }
                can_return = false;
            }
            let command = commands[0];
            if self.kind(command) == "ExpressionStatement" {
                let expression = *self.children(command).first()?;
                if self.kind(expression) == "ThrowExpression" {
                    if how_many == 1 {
                        can_assign = false;
                        can_argument = false;
                    }
                    continue;
                }
                can_return = false;
                if can_argument && self.kind(expression) == "MethodInvocation" {
                    can_assign = false;
                    let name = self.method_name(expression)?;
                    let Elem::Function(f) = self.identifier_element(name, true) else { return None };
                    let fe = prog.function(f);
                    if fe.class.is_some() || fe.extension.is_some() || fe.kind != FunctionKind::Function {
                        return None;
                    }
                    match &function {
                        None => function = Some((f, self.p.name(fe.name).to_string())),
                        Some((g, _)) if *g != f => return None,
                        _ => {}
                    }
                } else if can_assign && self.kind(expression) == "AssignmentExpression" {
                    can_argument = false;
                    let left = *self.children(expression).first()?;
                    if self.kind(left) != "SimpleIdentifier" {
                        return None;
                    }
                    let operator = self.next_token(self.tree.nos[left].fim)?;
                    let operator = self.source[operator.start..operator.end].to_string();
                    let declaration = match self.tree.nos[left].marca {
                        crate::arvore_analyzer::Marca::Expr(x) => self.bodies.declaracao_local(x),
                        _ => None,
                    };
                    match &write {
                        None => {
                            let Some(d) = declaration else { return None };
                            if !matches!(self.local_element(d), Elem::LocalVariable) {
                                return None;
                            }
                            write = Some((d, self.node_text(left).to_string(), operator));
                        }
                        Some((d, _, o)) => {
                            if declaration != Some(*d) || *o != operator {
                                return None;
                            }
                        }
                    }
                } else {
                    return None;
                }
            } else {
                if !can_return || self.kind(command) != "ReturnStatement" || self.children(command).is_empty() {
                    return None;
                }
                can_assign = false;
                can_argument = false;
            }
            if !can_return && !can_assign && !can_argument {
                return None;
            }
        }
        let ty = if can_return {
            SwitchType::Return
        } else if can_assign {
            let (_, name, operator) = write?;
            SwitchType::Assignment(name, operator)
        } else if can_argument {
            SwitchType::Argument(function?.1)
        } else {
            return None;
        };
        Some((ty, groups))
    }

    /// `ConvertToSwitchExpression`.
    pub(crate) fn switch_to_expression(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let switch = self.tree.localizar(start, end)?;
        if self.kind(switch) != "SwitchStatement" {
            return None;
        }
        let scrutinee = *self.children(switch).first()?;
        let scrutinee_type = self.node_expr(scrutinee).and_then(|x| self.bodies.get_type(x));
        // `_isEffectivelyExhaustive`.
        let exhaustive = scrutinee_type.is_some_and(|t| {
            self.always_exhaustive_in_lsp(&mut self.p.query.table.clone(), t, 0) || {
                let members: Vec<usize> = self.children(switch).iter().copied().filter(|&k| matches!(self.kind(k), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase")).collect();
                match members.last() {
                    Some(&u) if self.kind(u) == "SwitchPatternCase" => self
                        .children(u)
                        .iter()
                        .copied()
                        .find(|&g| self.kind(g) == "GuardedPattern")
                        .and_then(|g| self.children(g).first().copied())
                        .is_some_and(|p| self.kind(p) == "WildcardPattern"),
                    Some(&u) => self.kind(u) == "SwitchDefault",
                    None => false,
                }
            }
        });
        // `followingThrow`: the next statement in the block is `throw …;`.
        let mut next_throw: Option<(usize, usize)> = None;
        if !exhaustive {
            let block = self.parent(switch).filter(|&b| self.kind(b) == "Block")?;
            let siblings = self.children(block);
            let i = siblings.iter().position(|&k| k == switch)?;
            let next = *siblings.get(i + 1)?;
            if self.kind(next) != "ExpressionStatement" {
                return None;
            }
            let t = *self.children(next).first()?;
            if self.kind(t) != "ThrowExpression" {
                return None;
            }
            next_throw = Some((next, t));
        }
        let (ty, groups) = self.switch_type(switch)?;
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let (start_s, end_s) = (self.tree.nos[switch].inicio, self.tree.nos[switch].fim);
        let mut edits: Vec<(Span, String)> = Vec::new();
        let empty = |p: usize| Span { start: p, end: p };
        let how_many = groups.len();
        match &ty {
            SwitchType::Return => {
                edits.push((empty(start_s), "return ".to_string()));
                edits.push((empty(end_s), ";".to_string()));
                for (i, g) in groups.iter().enumerate() {
                    let (colon, comma) = match g {
                        Group::Pattern { member, .. } => {
                            let dp = self.member_colon(*member)?;
                            edits.push((Span { start: self.tree.nos[*member].inicio, end: dp.end }, "_ =>".to_string()));
                            (dp, false)
                        }
                        Group::Cases { cases, .. } => {
                            let dp = self.member_colon(*cases.last()?)?;
                            let text = self.pattern_text(cases)?;
                            edits.push((Span { start: self.tree.nos[cases[0]].inicio, end: dp.end }, format!("{text} =>")));
                            (dp, i < how_many - 1 || next_throw.is_some())
                        }
                    };
                    let command = *g.commands().first()?;
                    let comment = self.has_comment_before(command);
                    match self.kind(command) {
                        "ReturnStatement" => {
                            let e = *self.children(command).first()?;
                            if !comment {
                                edits.push((Span { start: colon.end, end: self.tree.nos[e].inicio }, " ".to_string()));
                            } else {
                                edits.push((Span { start: self.tree.nos[command].inicio, end: self.tree.nos[e].inicio }, String::new()));
                            }
                        }
                        "ExpressionStatement" => {
                            let e = *self.children(command).first()?;
                            if self.kind(e) == "ThrowExpression" && !comment {
                                edits.push((Span { start: colon.end, end: self.tree.nos[e].inicio }, " ".to_string()));
                            }
                        }
                        _ => {}
                    }
                    edits.push((self.last_token(command)?, if comma { ",".to_string() } else { String::new() }));
                }
                if let Some((command, throw)) = next_throw {
                    let closes = self.last_token(switch)?;
                    let text = format!("{}_ => {},", tx.line_prefix(closes.start), self.node_text(throw));
                    let indented: Vec<String> = text.split(eol).map(|l| format!("{ONE_INDENT}{l}")).collect();
                    edits.push((empty(tx.content_start(closes.start)), format!("{}{eol}", indented.join(eol))));
                    edits.push((tx.line_range(self.tree.nos[command].inicio, self.tree.nos[command].fim), String::new()));
                }
            }
            SwitchType::Assignment(name, operator) => {
                edits.push((empty(start_s), format!("{name} {operator} ")));
                for (i, g) in groups.iter().enumerate() {
                    let (colon, comma) = match g {
                        Group::Pattern { member, .. } => {
                            let dp = self.member_colon(*member)?;
                            edits.push((Span { start: self.tree.nos[*member].inicio, end: dp.end }, "_ =>".to_string()));
                            (dp, false)
                        }
                        Group::Cases { cases, .. } => {
                            let dp = self.member_colon(*cases.last()?)?;
                            let text = self.pattern_text(cases)?;
                            edits.push((Span { start: self.tree.nos[cases[0]].inicio, end: dp.end }, format!("{text} =>")));
                            (dp, i < how_many - 1)
                        }
                    };
                    for &command in g.commands() {
                        if self.kind(command) == "ExpressionStatement" {
                            let e = *self.children(command).first()?;
                            if self.kind(e) == "AssignmentExpression" {
                                let left = *self.children(e).first()?;
                                let right = *self.children(e).last()?;
                                let op = self.next_token(self.tree.nos[left].fim)?;
                                if !self.has_comment_before(command) {
                                    edits.push((Span { start: colon.end, end: op.end }, String::new()));
                                } else {
                                    edits.push((Span { start: self.tree.nos[e].inicio, end: self.tree.nos[right].inicio }, String::new()));
                                }
                            } else if self.kind(e) == "ThrowExpression" {
                                edits.push((Span { start: colon.end, end: self.tree.nos[command].inicio - 1 }, String::new()));
                            }
                        }
                        if self.kind(command) == "BreakStatement" {
                            edits.push((self.break_range(command)?, String::new()));
                        } else {
                            edits.push((self.last_token(command)?, if comma { ",".to_string() } else { String::new() }));
                        }
                    }
                }
                edits.push((empty(end_s), ";".to_string()));
            }
            SwitchType::Argument(name) => {
                edits.push((empty(start_s), format!("{name}(")));
                for (i, g) in groups.iter().enumerate() {
                    let (colon, comma) = match g {
                        Group::Pattern { member, .. } => {
                            let dp = self.member_colon(*member)?;
                            edits.push((Span { start: self.tree.nos[*member].inicio, end: dp.end }, "_ =>".to_string()));
                            (dp, false)
                        }
                        Group::Cases { cases, .. } => {
                            let dp = self.member_colon(*cases.last()?)?;
                            let text = self.pattern_text(cases)?;
                            edits.push((Span { start: self.tree.nos[cases[0]].inicio, end: dp.end }, format!("{text} => ")));
                            (dp, i < how_many - 1)
                        }
                    };
                    for &command in g.commands() {
                        let comment = self.has_comment_before(command);
                        if self.kind(command) == "ExpressionStatement" {
                            let e = *self.children(command).first()?;
                            if self.kind(e) == "MethodInvocation" {
                                let list = self.children(e).iter().copied().find(|&f| self.kind(f) == "ArgumentList")?;
                                let opens = self.tree.nos[list].inicio + 1;
                                let closes = self.tree.nos[list].fim;
                                let de = if !comment { colon.end } else { self.tree.nos[e].inicio };
                                edits.push((Span { start: de, end: opens }, String::new()));
                                edits.push((Span { start: closes - 1, end: closes }, String::new()));
                            }
                            if !comment && self.kind(e) == "ThrowExpression" {
                                edits.push((Span { start: colon.end, end: self.tree.nos[command].inicio - 1 }, String::new()));
                            }
                        }
                        if self.kind(command) == "BreakStatement" {
                            edits.push((self.break_range(command)?, String::new()));
                        } else {
                            edits.push((self.last_token(command)?, if comma { ",".to_string() } else { String::new() }));
                        }
                    }
                }
                edits.push((empty(end_s), ");".to_string()));
            }
        }
        Some(self.simple_action(uri, "Convert to switch expression", "refactor.convert.switchExpression", edits))
    }
}
