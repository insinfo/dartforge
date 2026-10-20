//! `Sort Members` (`dart.edit.sortMembers`): the Dart 3.6.2 server's
//! `MemberSorter` (`AS:src/services/correction/sort_members.dart:14-278`;
//! docs/LSP-ESPECIFICACAO.md §13.12.5).
//!
//! Sorts the members of each class, enum, mixin, extension and extension
//! type and then the top-level declarations, by the original's priority
//! table; within a group, by lowercase name (class fields keep the file
//! order). Each member carries the comments that precede it and the
//! end-of-line comment; what lies between members (blank lines,
//! indentation) does not move. The result is **one** edit, from the first
//! to the last differing character (`computeSimpleDiff`).
//!
//! The final step is the original's: the directives reordered by the
//! `ImportOrganizer` without removal (`crate::organize_imports`, §13.12.6).
//! Written without compiling or running (2026-10-04).

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    Ast, CompilationUnit, DeclKind, FunctionKind, MemberId, MemberKind, TypedefKind,
};
use dartforge_intern::Interner;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    UnitFunctionMain,
    UnitVariableConst,
    UnitVariable,
    UnitAccessor,
    UnitFunction,
    UnitGenericTypeAlias,
    UnitFunctionType,
    UnitClass,
    UnitExtensionType,
    UnitExtension,
    ClassField,
    ClassAccessor,
    ClassConstructor,
    ClassMethod,
}

/// `_PRIORITY_ITEMS` (`sort_members.dart:15-47`): `(static, kind, private)`;
/// for class fields the third one does not take part in equality.
const PRIORITIES: &[(bool, Kind, bool)] = &[
    (false, Kind::UnitFunctionMain, false),
    (false, Kind::UnitVariableConst, false),
    (false, Kind::UnitVariableConst, true),
    (false, Kind::UnitVariable, false),
    (false, Kind::UnitVariable, true),
    (false, Kind::UnitAccessor, false),
    (false, Kind::UnitAccessor, true),
    (false, Kind::UnitFunction, false),
    (false, Kind::UnitFunction, true),
    (false, Kind::UnitGenericTypeAlias, false),
    (false, Kind::UnitGenericTypeAlias, true),
    (false, Kind::UnitFunctionType, false),
    (false, Kind::UnitFunctionType, true),
    (false, Kind::UnitClass, false),
    (false, Kind::UnitClass, true),
    (false, Kind::UnitExtensionType, false),
    (false, Kind::UnitExtensionType, true),
    (false, Kind::UnitExtension, false),
    (false, Kind::UnitExtension, true),
    (true, Kind::ClassField, false),
    (true, Kind::ClassAccessor, false),
    (true, Kind::ClassAccessor, true),
    (false, Kind::ClassField, false),
    (false, Kind::ClassConstructor, false),
    (false, Kind::ClassConstructor, true),
    (false, Kind::ClassAccessor, false),
    (false, Kind::ClassAccessor, true),
    (false, Kind::ClassMethod, false),
    (false, Kind::ClassMethod, true),
    (true, Kind::ClassMethod, false),
    (true, Kind::ClassMethod, true),
];

/// A member to sort: the key and the range of its text (with the comments
/// that go with it) in the ORIGINAL source.
struct Item {
    kind: Kind,
    is_static: bool,
    name: String,
    start: usize,
    end: usize,
}

impl Item {
    /// `_getPriority`: the index of the first equal item of the table.
    fn priority(&self) -> usize {
        let private = self.name.starts_with('_');
        PRIORITIES
            .iter()
            .position(|&(is_static, kind, p)| {
                kind == self.kind
                    && is_static == self.is_static
                    && (kind == Kind::ClassField || p == private)
            })
            .unwrap_or(0)
    }
}

/// A comment in a gap between two members.
pub(crate) struct Comment {
    pub(crate) start: usize,
    pub(crate) end: usize,
    /// `///` or `/** */`.
    pub(crate) is_documentation: bool,
}

/// The comments of `text[from..until]`, which has only whitespace and comments.
pub(crate) fn comments(text: &str, from: usize, until: usize) -> Vec<Comment> {
    let b = text.as_bytes();
    let mut output = Vec::new();
    let mut i = from;
    while i + 1 < until {
        if b[i] == b'/' && b[i + 1] == b'/' {
            let end = text[i..until].find('\n').map_or(until, |k| i + k);
            let end_without_cr = if end > i && b[end - 1] == b'\r' {
                end - 1
            } else {
                end
            };
            let is_documentation = b.get(i + 2) == Some(&b'/') && b.get(i + 3) != Some(&b'/');
            output.push(Comment {
                start: i,
                end: end_without_cr,
                is_documentation,
            });
            i = end;
        } else if b[i] == b'/' && b[i + 1] == b'*' {
            let mut level = 1;
            let mut j = i + 2;
            while j < until && level > 0 {
                if j + 1 < until && b[j] == b'/' && b[j + 1] == b'*' {
                    level += 1;
                    j += 2;
                } else if j + 1 < until && b[j] == b'*' && b[j + 1] == b'/' {
                    level -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            let is_documentation = b.get(i + 2) == Some(&b'*') && b.get(i + 3) != Some(&b'/');
            output.push(Comment {
                start: i,
                end: j.min(until),
                is_documentation,
            });
            i = j;
        } else {
            i += 1;
        }
    }
    output
}

/// The line (0-based) of the offset.
pub(crate) fn line_of(text: &str, offset: usize) -> usize {
    text.as_bytes()[..offset.min(text.len())]
        .iter()
        .filter(|c| **c == b'\n')
        .count()
}

/// `range.nodeWithComments`: the range of a node's text, with the leading
/// comments and the end-of-line one. `previous`: the end of the token
/// before the node (`None` when the node is the first thing in the file).
fn with_comments(text: &str, node: Span, previous: Option<usize>) -> (usize, usize) {
    let start = match previous {
        // The first thing in the file: the comments before it are a header, except
        // the node's own documentation comment.
        None => {
            let cs = comments(text, 0, node.start);
            documentation_start(&cs).unwrap_or(node.start)
        }
        Some(previous_end) => {
            let cs = comments(text, previous_end, node.start);
            match documentation_start(&cs) {
                // With a documentation comment, the node starts at it: the ordinary
                // comments above stay where they are.
                Some(i) => i,
                None => {
                    // Those that start on the previous token's line are its end-of-line comment.
                    let previous_line = line_of(text, previous_end.saturating_sub(1));
                    cs.iter()
                        .find(|c| line_of(text, c.start) != previous_line)
                        .map_or(node.start, |c| c.start)
                }
            }
        }
    };
    // The end-of-line comment: those that start on the line where the node's
    // last token starts (here, the line of its last character).
    let end_line = line_of(text, node.end.saturating_sub(1));
    let line_end = text[node.end.min(text.len())..]
        .find('\n')
        .map_or(text.len(), |k| node.end + k);
    let mut end = node.end;
    for c in comments(text, node.end.min(text.len()), line_end) {
        // Only comments separated from the node by whitespace (no code in between).
        if line_of(text, c.start) == end_line && text[end..c.start].trim().is_empty() {
            end = c.end;
        } else {
            break;
        }
    }
    (start, end)
}

/// The start of a node's documentation comment: the last sequence of
/// consecutive `///` lines, or the last `/** */` block, of the gap.
fn documentation_start(cs: &[Comment]) -> Option<usize> {
    let last = cs.iter().rposition(|c| c.is_documentation)?;
    let mut first = last;
    while first > 0 && cs[first - 1].is_documentation {
        first -= 1;
    }
    Some(cs[first].start)
}

/// `_getSortedMembers`: priority; class fields by position; the rest by
/// lowercase name, in UTF-16 units. Stable.
fn sort(items: &[Item]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&x, &y| {
        let (a, b) = (&items[x], &items[y]);
        a.priority().cmp(&b.priority()).then_with(|| {
            if a.kind == Kind::ClassField {
                a.start.cmp(&b.start)
            } else {
                a.name
                    .to_lowercase()
                    .encode_utf16()
                    .cmp(b.name.to_lowercase().encode_utf16())
            }
        })
    });
    order
}

/// `_sortAndReorderMembers`: the k-th slot receives the text of the k-th
/// sorted member, back to front. The ranges are those of the original
/// source; since each swap preserves the container's total length, they stay
/// valid in `code` for the earlier slots.
fn reorder(code: &mut String, items: &[Item]) {
    let order = sort(items);
    if order.iter().enumerate().all(|(i, &j)| i == j) {
        return;
    }
    let texts: Vec<String> = items
        .iter()
        .map(|it| code[it.start..it.end].to_string())
        .collect();
    // The accumulated shift of the slots on the left changes the offsets of
    // those on the right: the whole span is built at once.
    let (first, last) = (items[0].start, items[items.len() - 1].end);
    let mut new = String::with_capacity(last - first);
    let mut cursor = first;
    for (i, it) in items.iter().enumerate() {
        new.push_str(&code[cursor..it.start]);
        new.push_str(&texts[order[i]]);
        cursor = it.end;
    }
    code.replace_range(first..last, &new);
}

fn members_of(decl: &DeclKind) -> Option<&[MemberId]> {
    match decl {
        DeclKind::Class(x) => Some(&x.members),
        DeclKind::Enum(x) => Some(&x.members),
        DeclKind::Mixin(x) => Some(&x.members),
        DeclKind::Extension(x) => Some(&x.members),
        DeclKind::ExtensionType(x) => Some(&x.members),
        _ => None,
    }
}

/// The member items of a class; `None` abandons sorting it (a field without
/// a variable).
fn class_items(
    text: &str,
    ast: &Ast,
    names: &Interner,
    opening: usize,
    members: &[MemberId],
) -> Option<Vec<Item>> {
    let mut items = Vec::with_capacity(members.len());
    let mut previous: Option<usize> = None;
    for &mid in members {
        let member = ast.member(mid);
        let (kind, is_static, name) = match &member.kind {
            MemberKind::Constructor(k) => (
                Kind::ClassConstructor,
                false,
                k.name
                    .map_or(String::new(), |n| names.resolve(n.sym).to_string()),
            ),
            MemberKind::Field(l) => {
                let first = l.variables.first()?;
                (
                    Kind::ClassField,
                    l.static_,
                    names.resolve(first.name.sym).to_string(),
                )
            }
            MemberKind::Method(f) => {
                let func = ast.function(*f);
                let base = func
                    .name
                    .map_or(String::new(), |n| names.resolve(n.sym).to_string());
                match func.kind {
                    FunctionKind::Getter => {
                        (Kind::ClassAccessor, func.static_, format!("{base} getter"))
                    }
                    FunctionKind::Setter => {
                        (Kind::ClassAccessor, func.static_, format!("{base} setter"))
                    }
                    FunctionKind::Function | FunctionKind::Operator => {
                        (Kind::ClassMethod, func.static_, base)
                    }
                }
            }
        };
        let previous_end = previous.unwrap_or(opening.min(member.span.start));
        let (start, end) = with_comments(text, member.span, Some(previous_end));
        items.push(Item {
            kind,
            is_static,
            name,
            start,
            end,
        });
        previous = Some(member.span.end);
    }
    Some(items)
}

/// `MemberSorter.sort`: the single edit `(offset, length, new text)` in
/// source bytes, or `None` when nothing changes.
pub(crate) fn sort_members(
    text: &str,
    unit: &CompilationUnit,
    ast: &Ast,
    names: &Interner,
) -> Option<(usize, usize, String)> {
    let mut code = text.to_string();
    // 1. The members of each class, in file order.
    for &d in unit.declarations.iter() {
        let decl = ast.decl(d);
        let Some(members) = members_of(&decl.kind) else {
            continue;
        };
        if members.len() < 2 {
            continue;
        }
        // The end of the token before the first member: the enum constants' `;`,
        // or the body's `{`.
        let first = ast.member(members[0]).span.start;
        let after_constants = match &decl.kind {
            DeclKind::Enum(x) => x.constants.last().and_then(|k| {
                let from = k.span.end.min(first);
                text[from..first].find(';').map(|i| from + i + 1)
            }),
            _ => None,
        };
        let opening = after_constants
            .or_else(|| {
                text.get(decl.span.start..first)
                    .and_then(|t| t.rfind('{'))
                    .map(|i| decl.span.start + i + 1)
            })
            .unwrap_or(first);
        if let Some(items) = class_items(text, ast, names, opening, members) {
            reorder(&mut code, &items);
        }
    }
    // 2. The top-level declarations; the texts come from the code with the
    // members already sorted.
    let mut items: Vec<Item> = Vec::new();
    let mut previous: Option<usize> = unit.directives.last().map(|x| x.span.end);
    let mut abandon = false;
    for &d in unit.declarations.iter() {
        let decl = ast.decl(d);
        let text_of = |n: dartforge_frontend::ast::Name| names.resolve(n.sym).to_string();
        let (kind, name) = match &decl.kind {
            DeclKind::Class(x) => (Kind::UnitClass, text_of(x.name)),
            DeclKind::Enum(x) => (Kind::UnitClass, text_of(x.name)),
            DeclKind::Mixin(x) => (Kind::UnitClass, text_of(x.name)),
            DeclKind::ExtensionType(x) => (Kind::UnitExtensionType, text_of(x.name)),
            DeclKind::Extension(x) => (Kind::UnitExtension, x.name.map_or(String::new(), text_of)),
            DeclKind::Function(f) => {
                let func = ast.function(*f);
                let base = func.name.map_or(String::new(), text_of);
                match func.kind {
                    FunctionKind::Getter => (Kind::UnitAccessor, format!("{base} getter")),
                    FunctionKind::Setter => (Kind::UnitAccessor, format!("{base} setter")),
                    _ if base == "main" => (Kind::UnitFunctionMain, base),
                    _ => (Kind::UnitFunction, base),
                }
            }
            DeclKind::Typedef(x) => match x.kind {
                TypedefKind::Legacy { .. } => (Kind::UnitFunctionType, text_of(x.name)),
                TypedefKind::Alias(_) => (Kind::UnitGenericTypeAlias, text_of(x.name)),
            },
            DeclKind::Variables(l) => match l.variables.first() {
                Some(first_) => (
                    if l.const_ {
                        Kind::UnitVariableConst
                    } else {
                        Kind::UnitVariable
                    },
                    text_of(first_.name),
                ),
                None => {
                    abandon = true;
                    break;
                }
            },
        };
        let (start, end) = with_comments(text, decl.span, previous);
        items.push(Item {
            kind,
            is_static: false,
            name,
            start,
            end,
        });
        previous = Some(decl.span.end);
    }
    if !abandon && items.len() >= 2 {
        reorder(&mut code, &items);
    }
    // 3. The directives, last: only reorders and regroups (no errors and no
    // removal). Their offsets are the original ones: the swaps above only touch
    // from the first declaration slot on.
    if let Some(new) = crate::organize_imports::organize(&code, unit, &[], false, false) {
        code = new;
    }
    simple_difference(text, &code)
}

/// `computeSimpleDiff` (`AS:src/utilities/strings.dart:36-49`): the longest
/// common prefix and suffix; the edit is the middle.
fn simple_difference(old: &str, new: &str) -> Option<(usize, usize, String)> {
    if old == new {
        return None;
    }
    let (a, n) = (old.as_bytes(), new.as_bytes());
    let mut prefix = a.iter().zip(n.iter()).take_while(|(x, y)| x == y).count();
    while !old.is_char_boundary(prefix) || !new.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let max = a.len().min(n.len()) - prefix;
    let mut suffix = a
        .iter()
        .rev()
        .zip(n.iter().rev())
        .take(max)
        .take_while(|(x, y)| x == y)
        .count();
    while !old.is_char_boundary(a.len() - suffix) || !new.is_char_boundary(n.len() - suffix) {
        suffix -= 1;
    }
    Some((
        prefix,
        a.len() - prefix - suffix,
        new[prefix..n.len() - suffix].to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(source: &str) -> String {
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        match sort_members(source, &p.unit, &p.ast, &names) {
            Some((offset, length, new)) => {
                format!("{}{}{}", &source[..offset], new, &source[offset + length..])
            }
            None => source.to_string(),
        }
    }

    /// Fields in file order, the comment above and the end-of-line one go with
    /// the member, the unnamed constructor before the named one.
    #[test]
    fn spec_example() {
        let before = "class A {\n  void b() {}\n  // sobre o campo\n  final int z = 1; // fim\n  A.named();\n  static int s = 0;\n  int a = 2;\n  A();\n  int get x => 0;\n}\n";
        let after = "class A {\n  static int s = 0;\n  // sobre o campo\n  final int z = 1; // fim\n  int a = 2;\n  A();\n  A.named();\n  int get x => 0;\n  void b() {}\n}\n";
        assert_eq!(sorted(before), after);
    }

    #[test]
    fn top_level_with_main_first_and_classes_after_functions() {
        let before = "class B {}\nvoid z() {}\nvoid main() {}\nconst k = 1;\n";
        let after = "void main() {}\nconst k = 1;\nvoid z() {}\nclass B {}\n";
        assert_eq!(sorted(before), after);
    }

    #[test]
    fn enum_with_comment_between_constants() {
        let before = "enum E {\n  a, // primeira\n  b;\n\n  void z() {}\n  void y() {}\n}\n";
        let after = "enum E {\n  a, // primeira\n  b;\n\n  void y() {}\n  void z() {}\n}\n";
        assert_eq!(sorted(before), after);
    }

    #[test]
    fn directives_reordered_without_removing_duplicates() {
        let before = "import 'b.dart';\nimport 'dart:io';\nimport 'b.dart';\n\nvoid main() {}\n";
        let after = "import 'dart:io';\n\nimport 'b.dart';\nimport 'b.dart';\n\nvoid main() {}\n";
        assert_eq!(sorted(before), after);
    }

    #[test]
    fn already_sorted_has_no_edit() {
        let mut names = Interner::new();
        let source = "void main() {}\nclass A {}\n";
        let p = dartforge_frontend::parser::parse(source, &mut names);
        assert!(sort_members(source, &p.unit, &p.ast, &names).is_none());
    }
}
