//! `DestructureLocalVariableAssignment`
//! (destructure_local_variable_assignment.dart) from `analysis_server` 3.6.2:
//! the name of a local variable of record type becomes a record pattern;
//! one of class type, whose reads are only of properties, becomes an object
//! pattern, and each read `v.p` becomes the pattern variable. The producer's
//! linked positions come out as their text.
//!
//! | Title | Kind |
//! |---|---|
//! | `Destructure variable assignment` | `refactor.destructureLocalVariableAssignment` |

use crate::actions::CodeAction;
use crate::analyzer_tree::Mark;
use crate::refactorings::Context;
use crate::refactorings_exec::names_to_text;
use crate::Edit;
use dartforge_diagnostics::Span;
use dartforge_types::{Resolved, Type};
use std::collections::HashSet;

impl Context<'_> {
    /// The name of a formal parameter (its last token, looking inside the
    /// `DefaultFormalParameter`).
    fn parameter_name(&self, p: usize) -> Option<Span> {
        let p = if self.kind(p) == "DefaultFormalParameter" { *self.children(p).first()? } else { p };
        let after = self.children(p).iter().rev().find(|&&k| self.kind(k) == "FormalParameterList").map_or(self.tree.nodes[p].end, |&l| self.tree.nodes[l].start);
        self.previous_token(after)
    }

    /// `ScopedNameFinder(position)` applied to `n`: the names of locals and
    /// parameters visible at `n`, declared before the position.
    pub(crate) fn names_in_scope(&self, n: usize, position: usize) -> HashSet<String> {
        let mut names = HashSet::new();
        let mut add = |s: Option<Span>, names: &mut HashSet<String>| {
            if let Some(s) = s
                && s.end < position
            {
                names.insert(self.source[s.start..s.end].to_string());
            }
        };
        let variable_name = |v: usize| self.next_token(self.tree.nodes[v].start);
        let mut in_local_function = false;
        let mut child = n;
        let mut current = Some(n);
        while let Some(k) = current {
            let parameters_outside_child = |names: &mut HashSet<String>, add: &mut dyn FnMut(Option<Span>, &mut HashSet<String>)| {
                if let Some(&list) = self.children(k).iter().find(|&&f| self.kind(f) == "FormalParameterList")
                    && list != child
                {
                    for &p in self.children(list) {
                        add(self.parameter_name(p), names);
                    }
                }
            };
            match self.kind(k) {
                "Block" | "SwitchCase" | "SwitchPatternCase" | "SwitchDefault" => {
                    // `_checkStatements`: the statements before the immediate child.
                    for &c in self.children(k) {
                        if c == child {
                            break;
                        }
                        match self.kind(c) {
                            "VariableDeclarationStatement" => {
                                if let Some(&l) = self.children(c).first() {
                                    for &v in self.children(l).iter().filter(|&&v| self.kind(v) == "VariableDeclaration") {
                                        add(variable_name(v), &mut names);
                                    }
                                }
                            }
                            "FunctionDeclarationStatement" if !in_local_function => {
                                if let Some(&f) = self.children(c).first()
                                    && let Mark::Function(fid) = self.tree.nodes[f].mark
                                {
                                    add(self.ast.function(fid).name.map(|x| x.span), &mut names);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "CatchClause" => {
                    for &p in self.children(k).iter().filter(|&&p| self.kind(p) == "CatchClauseParameter") {
                        add(Some(self.tree.span(p)), &mut names);
                    }
                }
                "ConstructorDeclaration" => {
                    parameters_outside_child(&mut names, &mut add);
                    break;
                }
                "FieldDeclaration" | "TopLevelVariableDeclaration" | "FunctionTypeAlias" | "GenericTypeAlias" | "ClassTypeAlias" => break,
                "ForEachPartsWithDeclaration" => {
                    if let Some(&d) = self.children(k).iter().find(|&&f| self.kind(f) == "DeclaredIdentifier") {
                        add(self.previous_token(self.tree.nodes[d].end), &mut names);
                    }
                }
                "ForPartsWithDeclarations" => {
                    if let Some(&l) = self.children(k).iter().find(|&&f| self.kind(f) == "VariableDeclarationList") {
                        for &v in self.children(l).iter().filter(|&&v| self.kind(v) == "VariableDeclaration") {
                            add(variable_name(v), &mut names);
                        }
                    }
                }
                "FunctionDeclaration" => {
                    if self.parent(k).is_none_or(|p| self.kind(p) != "FunctionDeclarationStatement") {
                        break;
                    }
                }
                "FunctionDeclarationStatement" => in_local_function = true,
                "FunctionExpression" => parameters_outside_child(&mut names, &mut add),
                "MethodDeclaration" => {
                    parameters_outside_child(&mut names, &mut add);
                    break;
                }
                _ => {}
            }
            child = k;
            current = self.parent(k);
        }
        names
    }

    /// The reads of the local declared at `name` inside `body`, in text order:
    /// `(property, access node)` for `v.p`, and `None` for a read of the whole
    /// object.
    fn local_reads(&self, body: usize, name: usize) -> Vec<Option<(String, usize)>> {
        let mut v = Vec::new();
        let mut stack = vec![body];
        while let Some(k) = stack.pop() {
            if self.kind(k) == "SimpleIdentifier"
                && let Mark::Expr(x) = self.tree.nodes[k].mark
                && matches!(self.bodies.get_resolved(x), Some(Resolved::Local(_)))
                && self.bodies.local_declaration(x) == Some(name)
            {
                let parent = self.parent(k);
                match parent.map(|p| self.kind(p)) {
                    Some("PrefixedIdentifier" | "PropertyAccess") if self.children(parent.unwrap_or(k)).first() == Some(&k) => {
                        let p = parent.unwrap_or(k);
                        let property = self.children(p).last().map(|&q| self.node_text(q).to_string()).unwrap_or_default();
                        v.push(Some((property, p)));
                    }
                    _ => v.push(None),
                }
            }
            stack.extend(self.children(k).iter().rev().copied());
        }
        v
    }

    /// `DestructureLocalVariableAssignment`.
    pub(crate) fn destructure_local(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let node = self.tree.locate(start, end)?;
        if self.kind(node) != "VariableDeclaration" {
            return None;
        }
        let name = self.next_token(self.tree.nodes[node].start)?;
        // `declaredElement2`: only a local.
        let ty = *self.bodies.local_types.get(&name.start)?;
        let table = &self.p.query.table;
        let action = |edits: Vec<(Span, String)>| CodeAction {
            title: "Destructure variable assignment".into(),
            kind: "refactor.destructureLocalVariableAssignment".into(),
            edits: edits.into_iter().rev().map(|(span, text)| Edit { uri: uri.to_string(), span, text }).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        };
        match table.get(ty) {
            Type::Record { positional, named, .. } => {
                let mut excluded = self.names_in_scope(node, self.tree.nodes[node].start);
                let mut variables: Vec<String> = Vec::new();
                for i in 1..=positional.len() {
                    let mut name_v = format!("${i}");
                    if excluded.contains(&name_v) {
                        // `getIndexedVariableName`.
                        if let Some(n) = (b'a'..b'z').map(|c| format!("${i}{}", c as char)).find(|n| !excluded.contains(n)) {
                            name_v = n;
                        }
                    }
                    excluded.insert(name_v.clone());
                    variables.push(name_v);
                }
                // Each variable is a linked group `VAR_i` with the suggestions of
                // `RecordField.write`: (written prefix, group text, suggestions).
                let mut groups: Vec<(String, String, Vec<String>)> =
                    variables.into_iter().map(|v| (String::new(), v.clone(), vec![v, "_".to_string()])).collect();
                let mut named_: Vec<String> = named.iter().map(|(s, _)| self.p.name(*s).to_string()).collect();
                named_.sort();
                for field in named_ {
                    if !excluded.contains(&field) {
                        groups.push((String::new(), format!(":{field}"), vec![format!(":{field}"), format!("{field}: _")]));
                    } else {
                        let suggestion = names_to_text(&field, &excluded).into_iter().next()?;
                        groups.push((format!("{field}: "), suggestion.clone(), vec![suggestion, "_".to_string()]));
                    }
                }
                let mut text = String::from("(");
                let mut positions = Vec::new();
                for (i, (prefix, v, _)) in groups.iter().enumerate() {
                    if i > 0 {
                        text.push_str(", ");
                    }
                    text.push_str(prefix);
                    positions.push((text.len(), v.len()));
                    text.push_str(v);
                }
                text.push(')');
                let mut a = action(vec![(name, text)]);
                for ((k, n), (_, _, suggestions)) in positions.into_iter().zip(groups) {
                    a = a.with_group(0, &[k], n, suggestions);
                }
                Some(a)
            }
            Type::Interface { class, .. } => {
                let body = self.with_parents(node).find(|&k| matches!(self.kind(k), "BlockFunctionBody" | "ExpressionFunctionBody"))?;
                let reads = self.local_reads(body, name.start);
                if reads.iter().any(Option::is_none) {
                    return None;
                }
                let in_scope = self.names_in_scope(node, self.tree.nodes[node].start);
                // The properties, in the order of their first read.
                let mut properties: Vec<(String, Vec<usize>)> = Vec::new();
                for (p, access) in reads.into_iter().flatten() {
                    match properties.iter_mut().find(|(q, _)| *q == p) {
                        Some((_, l)) => l.push(access),
                        None => properties.push((p, vec![access])),
                    }
                }
                let mut fields: Vec<(String, String, Vec<usize>)> = Vec::new();
                for (p, accesses) in properties {
                    let mut excluded = self.local_conflicts(self.tree.nodes[node].start);
                    excluded.extend(in_scope.iter().cloned());
                    for &a in &accesses {
                        // `inSetterContext`.
                        if self.node_expr(a).is_some_and(|x| self.writes.contains(&x)) {
                            return None;
                        }
                        excluded.extend(self.local_conflicts(self.tree.nodes[a].start));
                    }
                    let variable = names_to_text(&p, &excluded).into_iter().next()?;
                    fields.push((variable, p, accesses));
                }
                let prog = self.p.program();
                let pattern = fields
                    .iter()
                    .map(|(v, p, _)| if v == p { format!(":{v}") } else { format!("{p}: {v}") })
                    .collect::<Vec<_>>()
                    .join(", ");
                let class_ = self.p.name(prog.class(*class).name);
                let mut edits = vec![(name, format!("{class_}({pattern})"))];
                for (v, _, accesses) in &fields {
                    for &a in accesses {
                        edits.push((self.tree.span(a), v.clone()));
                    }
                }
                edits.sort_by_key(|(s, _)| s.start);
                // Without properties read, `selectHere` between the parentheses; with them,
                // the replaced reads are separate edits (their groups do not reach a
                // snippet).
                let selection = fields.is_empty().then_some(class_.len() + 1);
                let a = action(edits);
                Some(match selection {
                    Some(k) => a.with_selection(0, k, 0),
                    None => a,
                })
            }
            _ => None,
        }
    }
}
