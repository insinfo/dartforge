//! `insertIntoUnitMember` with the `_InsertionPreparer` and
//! `writeParametersMatchingArguments` of the Dart 3.6.2
//! `DartFileEditBuilder` (`change_builder_dart.dart:2202-2296`,
//! `:2982-3131`, `:827-940`, `:1221-1390`; docs/LSP-ESPECIFICACAO.md
//! §13.7.4.0), and `CreateConstructor` (`create_constructor.dart`).
//! Written without compiling or running (2026-10-05).

use crate::arvore_analyzer::Marca;
use crate::write_type::Writer;
use crate::refactorings::Context;
use crate::refactorings_exec::{Text, ONE_INDENT, add_letter, add_all, camel_combinations};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassKind, LibraryId, UnitId};
use dartforge_frontend::ast;
use dartforge_types::Type;
use std::collections::{BTreeSet, HashSet};

/// The filter for the last compatible member (`lastMemberFilter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Filter {
    /// `insertField`.
    Field,
    /// `insertConstructor` (with `sort_constructors_first`, constructors only).
    Constructor { first_constructors: bool },
    /// `insertGetter`.
    Getter,
    /// `insertMethod`.
    Method,
    /// No filter: the last member.
    None,
}

/// The method name prefixes that the suggestion strips (`get`, `is`, `to`).
const KNOWN_PREFIXES: [&str; 3] = ["get", "is", "to"];

impl Context<'_> {
    /// The members (`ClassMember`) of a declaration.
    pub(crate) fn container_members(&self, c: usize) -> Vec<usize> {
        self.children(c).iter().copied().filter(|&f| matches!(self.kind(f), "ConstructorDeclaration" | "MethodDeclaration" | "FieldDeclaration")).collect()
    }

    fn is_getter(&self, m: usize) -> bool {
        match self.tree.nos[m].marca {
            Marca::Funcao(fid) => self.ast.function(fid).kind == ast::FunctionKind::Getter,
            _ => false,
        }
    }

    /// `insertIntoUnitMember(container, filter)`: the offset and the full text
    /// (prefix, member, suffix) of the insertion of `member`.
    pub(crate) fn insert_in_member(&self, container: usize, filter: Filter, member: &str) -> Option<(usize, String)> {
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let members = self.container_members(container);
        let passes = |m: usize| match filter {
            Filter::Field => self.kind(m) == "FieldDeclaration",
            Filter::Constructor { first_constructors: true } => self.kind(m) == "ConstructorDeclaration",
            Filter::Constructor { first_constructors: false } => matches!(self.kind(m), "ConstructorDeclaration" | "FieldDeclaration"),
            Filter::Getter => matches!(self.kind(m), "FieldDeclaration" | "ConstructorDeclaration") || (self.kind(m) == "MethodDeclaration" && self.is_getter(m)),
            Filter::Method => matches!(self.kind(m), "FieldDeclaration" | "ConstructorDeclaration" | "MethodDeclaration"),
            Filter::None => true,
        };
        let target = members.iter().rev().copied().find(|&m| passes(m));
        let enum_ = self.kind(container) == "EnumDeclaration";
        let constants = self.kind_children(container, "EnumConstantDeclaration");
        // The constants' `;`.
        let semicolon = if enum_ {
            constants.last().and_then(|&c| {
                let mut t = self.next_token(self.tree.nos[c].fim)?;
                if &self.source[t.start..t.end] == "," {
                    t = self.next_token(t.end)?;
                }
                (&self.source[t.start..t.end] == ";").then_some(t)
            })
        } else {
            None
        };
        // The declaration's `{` (the first one after the header).
        let key = {
            let mut pos = self.tree.nos[container].inicio;
            let limit = members.first().map_or(self.tree.nos[container].fim, |&m| self.tree.nos[m].inicio).min(constants.first().map_or(usize::MAX, |&c| self.tree.nos[c].inicio));
            let mut found = None;
            while let Some(t) = self.next_token(pos) {
                if t.start >= limit {
                    break;
                }
                if &self.source[t.start..t.end] == "{" {
                    found = Some(t);
                }
                pos = t.end;
            }
            found
        };
        let offset = match target {
            Some(m) => self.tree.nos[m].fim,
            None if enum_ => match semicolon {
                Some(p) => p.end,
                None => self.tree.nos[*constants.last()?].fim,
            },
            None => key?.end,
        };
        let mut s = String::new();
        if enum_ && semicolon.is_none() && target.is_none() {
            s.push(';');
        }
        if target.is_some() || (enum_ && !constants.is_empty()) {
            s.push_str(eol);
            s.push_str(eol);
        } else {
            s.push_str(eol);
        }
        s.push_str(ONE_INDENT);
        s.push_str(member);
        // The suffix.
        if target.is_none() && !(enum_ && !constants.is_empty()) {
            if !members.is_empty() {
                s.push_str(eol);
            } else {
                let start = self.first_token_after_comment_and_metadata(container).map_or(self.tree.nos[container].inicio, |t| t.start);
                let line_begin = self.source[..start].matches('\n').count();
                let line_end = self.source[..self.tree.nos[container].fim].matches('\n').count();
                if line_begin == line_end {
                    s.push_str(eol);
                }
            }
        }
        Some((offset, s))
    }

    /// `_getBaseNameFromExpression`.
    fn ins_expression_base_name(&self, e: usize) -> Option<String> {
        match self.kind(e) {
            "AsExpression" | "ParenthesizedExpression" => self.ins_expression_base_name(*self.children(e).first()?),
            _ => self.unwrapped_base_name(e),
        }
    }

    /// `_getBaseNameFromUnwrappedExpression`.
    fn unwrapped_base_name(&self, e: usize) -> Option<String> {
        let mut name: Option<String> = None;
        match self.kind(e) {
            "SimpleIdentifier" => return Some(self.node_text(e).to_string()),
            "PrefixedIdentifier" => return Some(self.node_text(*self.children(e).get(1)?).to_string()),
            "PropertyAccess" => return Some(self.node_text(*self.children(e).last()?).to_string()),
            "MethodInvocation" => name = self.method_name(e).map(|m| self.node_text(m).to_string()),
            "InstanceCreationExpression" => {
                let cn = self.kind_children(e, "ConstructorName").first().copied()?;
                let ty = self.kind_children(cn, "NamedType").first().copied()?;
                // The type's `name2`: the identifier after the prefix.
                let prefix_end = self.kind_children(ty, "ImportPrefixReference").first().map_or(self.tree.nos[ty].inicio, |&p| self.tree.nos[p].fim);
                let s = crate::project::word(self.source, self.next_token(prefix_end)?.start)?;
                return Some(self.source[s.start..s.end].to_string());
            }
            "IndexExpression" => {
                name = self.ins_expression_base_name(*self.children(e).first()?);
                if let Some(n) = &name {
                    if let Some(x) = n.strip_suffix("es") {
                        name = Some(x.to_string());
                    } else if let Some(x) = n.strip_suffix('s') {
                        name = Some(x.to_string());
                    }
                }
            }
            _ => {}
        }
        if let Some(n) = &name {
            for p in KNOWN_PREFIXES {
                if n.starts_with(p) {
                    if n == p {
                        return None;
                    } else if n.as_bytes().get(p.len()).is_some_and(|c| c.is_ascii_uppercase()) {
                        return Some(n[p.len()..].to_string());
                    }
                }
            }
        }
        name
    }

    /// `_getParameterNameSuggestions(usedNames, type, expression, index)`.
    fn parameter_suggestions(&self, used: &HashSet<String>, ty: Option<&Type>, arg: usize, index: usize) -> Vec<String> {
        let mut res: Vec<String> = Vec::new();
        if let Some(n) = self.ins_expression_base_name(arg) {
            let n = n.strip_prefix('_').unwrap_or(&n).to_string();
            add_all(used, &mut res, camel_combinations(&n), None);
        }
        // `_getBaseNameFromLocationInParent`: the label of a named argument (the
        // parameter of a positional one does not exist: the call does not resolve).
        if let Some(p) = self.parent(arg)
            && self.kind(p) == "NamedExpression"
            && self.children(p).get(1) == Some(&arg)
            && let Some(&label) = self.children(p).first()
            && let Some(&id) = self.children(label).first()
        {
            add_all(used, &mut res, camel_combinations(self.node_text(id)), None);
        }
        let core = &self.p.query.core;
        match ty {
            Some(Type::Interface { class, .. }) if Some(*class) == core.int_class => add_letter(used, &mut res, b'i'),
            Some(Type::Interface { class, .. }) if Some(*class) == core.double_class => add_letter(used, &mut res, b'd'),
            Some(Type::Interface { class, .. }) if Some(*class) == core.string_class => add_letter(used, &mut res, b's'),
            Some(Type::Interface { class, .. }) => {
                let name = self.p.name(self.p.program().class(*class).name).to_string();
                add_all(used, &mut res, camel_combinations(&name), None);
            }
            _ => {}
        }
        if res.is_empty() {
            return vec![format!("param{index}")];
        }
        res
    }

    /// `writeParametersMatchingArguments(argumentList)`: the types come from
    /// this context and are written by `writer` (that of the destination file).
    pub(crate) fn parameters_to_arguments(&self, list: usize, writer: &mut Writer<'_, '_>) -> String {
        let mut e = crate::snippets::Write::default();
        self.parameters_to_arguments_in(list, writer, &mut e);
        e.text
    }

    /// [`Self::parameters_to_arguments`] written into `e`, with the linked
    /// groups of `writeParameterMatchingArgument`: `TYPE<i>` on the type (with
    /// the supertype suggestions) and `PARAM<i>` on the name of a positional
    /// argument (with all the name suggestions).
    pub(crate) fn parameters_to_arguments_in(&self, list: usize, writer: &mut Writer<'_, '_>, e: &mut crate::snippets::Write) {
        let mut copy: Option<dartforge_types::TypeTable> = None;
        let mut used: HashSet<String> = HashSet::new();
        let mut s = String::new();
        let mut named_ = false;
        let arguments: Vec<usize> = self.children(list).to_vec();
        for (i, &arg) in arguments.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            let named = self.kind(arg) == "NamedExpression";
            if named && !named_ {
                named_ = true;
                s.push('{');
            }
            // `writeParameterMatchingArgument`.
            let value = if named { self.children(arg).get(1).copied().unwrap_or(arg) } else { arg };
            let table = &self.p.query.table;
            let ty = self.node_type(value).filter(|t| !matches!(table.get(*t), Type::Never | Type::Null) && !matches!(table.exibicao(*t), Some(dartforge_types::table::Exibicao::NeverAnulavel)));
            let without_suffix = ty.is_none_or(|t| !table.get(t).is_declared_nullable());
            if named && without_suffix {
                s.push_str("required ");
            }
            if let Some(t) = writer.write_type(ty, false) {
                e.write(&s);
                s.clear();
                let suggestions = ty.map(|x| crate::create::supertype_suggestions_at(self.p, &mut copy, x)).unwrap_or_default();
                e.linked(&format!("TYPE{i}"), &t, &suggestions);
                s.push(' ');
            }
            if named {
                let label = self.children(arg).first().and_then(|&r| self.children(r).first().copied());
                s.push_str(label.map_or("", |id| self.node_text(id)));
            } else {
                let suggestions = self.parameter_suggestions(&used, ty.map(|t| table.get(t)), value, i);
                let favorite = suggestions[0].clone();
                used.insert(favorite.clone());
                e.write(&s);
                s.clear();
                e.linked(&format!("PARAM{i}"), &favorite, &suggestions);
            }
        }
        if named_ {
            s.push('}');
        }
        e.write(&s);
    }
}

/// The result of `CreateConstructor`: the name (`{0}`), the edits per unit
/// and the libraries to import in the destination unit.
pub(crate) struct Constructor {
    pub(crate) name: String,
    pub(crate) unit: UnitId,
    pub(crate) edit: (Span, String),
    pub(crate) import: BTreeSet<LibraryId>,
    /// The linked groups of the declaration and its offset in the edit's text.
    pub(crate) marks: Option<(usize, crate::snippets::Write)>,
}

/// `CreateConstructor._proposeFromConstructorName` (the
/// `new_with_undefined_constructor` path): the `node` is the constructor
/// name in an `InstanceCreationExpression` (`new A.b()`); the class (only a
/// `ClassDeclaration`) gets `A.b(<parameters>);` through `insertConstructor`.
pub(crate) fn create_constructor(cx: &Context<'_>, error: Span) -> Option<Constructor> {
    let node = cx.tree.localizar(error.start, error.end)?;
    if cx.kind(node) != "SimpleIdentifier" {
        return None;
    }
    let cn = cx.parent(node).filter(|&c| cx.kind(c) == "ConstructorName")?;
    if cx.children(cn).get(1) != Some(&node) {
        return None;
    }
    let creation = cx.parent(cn).filter(|&c| cx.kind(c) == "InstanceCreationExpression")?;
    let name = cx.node_text(cn).split_whitespace().collect::<String>();
    // The type's class.
    let ty = cx.ast.types.iter().enumerate().find(|(_, t)| t.span.start == cx.tree.nos[cn].inicio).map(|(i, _)| ast::TypeId(i as u32))?;
    let class = cx.p.type_class(cx.unit, ty)?;
    let prog = cx.p.program();
    let c = prog.class(class);
    if c.kind != ClassKind::Class {
        return None;
    }
    let decl = c.decl?;
    let target = crate::refactorings::Context::new(cx.p, decl.unit);
    let container = target.tree.nos.iter().position(|n| n.especie == "ClassDeclaration" && matches!(n.marca, Marca::Decl(d) if d == decl.decl))?;
    let first_constructors = crate::refactorings_exec::enabled_rule(cx.p, decl.unit, "sort_constructors_first");
    let list = cx.kind_children(creation, "ArgumentList").first().copied()?;
    // The insertion point (for the type writer) before writing.
    let (offset, _) = target.insert_in_member(container, Filter::Constructor { first_constructors }, "")?;
    let mut writer = Writer::new(&target, offset);
    // The `NAME` group on the constructor name (`_write`,
    // `constructorNameGroupName`) and those of the parameters
    // (`writeParametersMatchingArguments`).
    let class_name = cx.p.name(c.name).to_string();
    let mut e = crate::snippets::Write::default();
    e.write(&format!("{class_name}.")).linked("NAME", cx.node_text(node), &[]).write("(");
    cx.parameters_to_arguments_in(list, &mut writer, &mut e);
    e.write(");");
    let (offset, text) = target.insert_in_member(container, Filter::Constructor { first_constructors }, &e.text)?;
    let marks = text.find(&e.text).map(|k| (k, e));
    Some(Constructor { name, unit: decl.unit, edit: (Span { start: offset, end: offset }, text), import: writer.import, marks })
}
