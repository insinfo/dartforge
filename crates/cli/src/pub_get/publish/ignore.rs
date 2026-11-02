//! A port of Pub's `ignore.dart`: `.gitignore`-style rules and the walk of
//! a directory tree that applies the ignore files of every directory.
//!
//! Each pattern becomes the same regular expression Pub builds (anchored
//! when it contains a slash, `/$` when it matches directories only, `**`
//! as "nothing or a path followed by `/`"), so the same paths match. Dart's
//! `RegExp` is translated for the `regex` crate: `.` excludes the line
//! terminators of JavaScript, and characters that are special in a `regex`
//! class but literal in JavaScript are escaped.
use regex::{Regex, RegexBuilder};
use std::rc::Rc;

/// One rule: the expression and whether it starts with `!`.
#[derive(Clone)]
struct Rule {
    pattern: Regex,
    negative: bool,
}

/// The rules of one ignore file (Pub's `Ignore`).
#[derive(Clone)]
pub(crate) struct Ignore {
    rules: Vec<Rule>,
}

/// Appends `c` escaped like `RegExp.escape` (as a `regex` literal).
fn push_literal(expression: &mut String, c: char) {
    if c.is_ascii_punctuation() {
        expression.push('\\');
    }
    expression.push(c);
}

/// `_parseIgnorePattern`: `Ok(None)` for comments and empty patterns,
/// `Err(message)` for invalid ones.
fn parse_pattern(pattern: &str, ignore_case: bool) -> Result<Option<Rule>, String> {
    if pattern.starts_with('#') {
        return Ok(None);
    }
    // Indices are in UTF-16 code units in Dart; the pattern is walked by
    // characters here, which only changes the offset of the exception.
    let chars: Vec<char> = pattern.chars().collect();
    let mut first = 0;
    let mut end = chars.len();
    let negative = pattern.starts_with('!');
    if negative {
        first += 1;
    }
    while end != 0 && chars[end - 1] == ' ' && (end == 1 || chars[end - 2] != '\\') {
        end -= 1;
    }
    if first == end {
        return Ok(None);
    }
    let mut current = first;
    let mut expression = String::new();
    let mut relative_to_path = false;
    let mut directories_only = false;
    let peek = |current: usize| {
        if current >= end {
            None
        } else {
            Some(chars[current])
        }
    };
    while let Some(next) = peek(current) {
        current += 1;
        match next {
            '*' => {
                if peek(current) == Some('*') {
                    current += 1;
                    if peek(current) == Some('/') {
                        current += 1;
                        if current == end {
                            expression.push_str(ANY);
                        } else {
                            expression.push_str("(?:(?:)|(?:");
                            expression.push_str(ANY);
                            expression.push_str("/))");
                        }
                        if current == end {
                            directories_only = true;
                        } else {
                            relative_to_path = true;
                        }
                    } else {
                        expression.push_str(ANY);
                    }
                } else if peek(current) == Some('/') || peek(current).is_none() {
                    expression.push_str("[^/]+");
                } else {
                    expression.push_str("[^/]*");
                }
            }
            '?' => expression.push_str("[^/]"),
            '[' => {
                let mut range = String::new();
                let mut first_in_range = true;
                let closed = loop {
                    let Some(c) = peek(current) else { break false };
                    current += 1;
                    if c == '\\' {
                        let Some(escaped) = peek(current) else {
                            break false;
                        };
                        current += 1;
                        if escaped == '-' {
                            range.push_str("\\-");
                        } else {
                            push_literal(&mut range, escaped);
                        }
                    } else if c == '!' && first_in_range {
                        range.push('^');
                    } else if c == ']' && first_in_range {
                        range.push_str("\\]");
                    } else if c == ']' {
                        break true;
                    } else if matches!(c, '[' | '&' | '~') {
                        // Literal in a JavaScript class, but nested classes
                        // and set operations in `regex`.
                        range.push('\\');
                        range.push(c);
                    } else {
                        range.push(c);
                    }
                    first_in_range = false;
                };
                if !closed {
                    return Err(format!(
                        "Pattern \"{pattern}\" had an invalid `[a-b]` style character range"
                    ));
                }
                expression.push('[');
                expression.push_str(&range);
                expression.push(']');
            }
            '\\' => {
                let Some(escaped) = peek(current) else {
                    return Err(format!(
                        "Pattern \"{pattern}\" end of pattern inside character escape."
                    ));
                };
                push_literal(&mut expression, escaped);
                current += 1;
            }
            '/' => {
                if current - 1 != first && current != end {
                    expression.push('/');
                }
                if current == end {
                    directories_only = true;
                } else {
                    relative_to_path = true;
                }
            }
            c => push_literal(&mut expression, c),
        }
    }
    let mut expression = if relative_to_path {
        format!("^{expression}")
    } else {
        format!("(?:^|/){expression}")
    };
    if directories_only {
        expression.push_str("/$");
    } else {
        expression.push_str("/?$");
    }
    // A range the JavaScript engine accepts can still be rejected here
    // (`[z-a]`); Dart throws an `AssertionError` for those, which is a
    // crash, so they are reported as invalid instead.
    let compiled = RegexBuilder::new(&expression)
        .case_insensitive(ignore_case)
        .build()
        .map_err(|_| {
            format!("Pattern \"{pattern}\" had an invalid `[a-b]` style character range")
        })?;
    Ok(Some(Rule {
        pattern: compiled,
        negative,
    }))
}

/// `.` of a JavaScript `RegExp`: anything but a line terminator.
const ANY: &str = "[^\\n\\r\\u{2028}\\u{2029}]*";

impl Ignore {
    /// `Ignore(patterns, ignoreCase:, onInvalidPattern:)`: every element may
    /// hold several lines (split at `\r?\n`). `on_invalid` receives the
    /// pattern and the exception's message.
    pub fn new(
        patterns: &[String],
        ignore_case: bool,
        mut on_invalid: impl FnMut(&str, &str),
    ) -> Self {
        let mut rules = Vec::new();
        for text in patterns {
            for line in text.split('\n') {
                let line = line.strip_suffix('\r').unwrap_or(line);
                match parse_pattern(line, ignore_case) {
                    Ok(Some(rule)) => rules.push(rule),
                    Ok(None) => {}
                    Err(message) => on_invalid(line, &message),
                }
            }
        }
        Self { rules }
    }

    /// `Ignore.ignores`: whether the relative `path` (ending in `/` for a
    /// directory) is ignored by these rules placed at the root.
    pub fn ignores(&self, path: &str) -> bool {
        if self.rules.is_empty() || path.is_empty() {
            return false;
        }
        let without_slash = path.strip_suffix('/').unwrap_or(path);
        // `listFiles` below `without_slash` that lists only the next part of
        // `path`: the path is ignored when nothing is left.
        list_files(
            without_slash,
            &mut |dir: &str| -> Result<Vec<String>, ()> {
                if dir == without_slash {
                    return Ok(Vec::new());
                }
                let start = if dir.is_empty() || dir == "." {
                    0
                } else {
                    dir.len() + 1
                };
                let next = path[start..].find('/').map_or(path.len(), |i| start + i);
                Ok(vec![path[..next].to_owned()])
            },
            &mut |dir: &str| (dir == "." || dir.is_empty()).then(|| Rc::new(self.clone())),
            &mut |candidate: &str| {
                candidate == "."
                    || candidate.is_empty()
                    || (path.len() > candidate.len() && path.as_bytes()[candidate.len()] == b'/')
            },
            true,
        )
        .map(|files| files.is_empty())
        .unwrap_or(false)
    }
}

/// `_matchesStack`: whether a rule of the innermost ignore file that matches
/// `path` (starting with `/`, ending with `/` for a directory) is not
/// negated.
fn matches_stack(stack: &[Option<(Rc<Ignore>, String)>], path: &str) -> bool {
    for (ignore, prefix) in stack.iter().rev().flatten() {
        let s = if prefix.is_empty() {
            path
        } else {
            &path[prefix.len()..]
        };
        for rule in ignore.rules.iter().rev() {
            if rule.pattern.is_match(s) {
                return !rule.negative;
            }
        }
    }
    false
}

/// `Ignore.listFiles`: the files (and directories, with `include_dirs`)
/// under `beneath` not ignored by the ignore files of the directories from
/// the root down, in Pub's depth-first order. Paths use `/`; the root is
/// `.`. `list_dir` lists the paths inside a directory (relative to the
/// root), `ignore_for_dir` returns a directory's rules and `is_dir` tells
/// directories apart.
pub(crate) fn list_files<E>(
    beneath: &str,
    list_dir: &mut dyn FnMut(&str) -> Result<Vec<String>, E>,
    ignore_for_dir: &mut dyn FnMut(&str) -> Option<Rc<Ignore>>,
    is_dir: &mut dyn FnMut(&str) -> bool,
    include_dirs: bool,
) -> Result<Vec<String>, E> {
    let beneath = if beneath == "." { "" } else { beneath };
    let beneath = format!("/{beneath}");
    let mut result = Vec::new();
    let mut stack: Vec<Option<(Rc<Ignore>, String)>> = vec![None];
    // The ignore files between the root and `beneath` (not inclusive).
    let mut index = 0;
    while let Some(found) = beneath[index..].find('/') {
        let at = index + found;
        let partial = &beneath[..=at];
        if matches_stack(&stack, partial) {
            return Ok(Vec::new());
        }
        let dir = if partial == "/" {
            ".".to_owned()
        } else {
            partial[1..partial.len() - 1].to_owned()
        };
        stack.push(ignore_for_dir(&dir).map(|ignore| (ignore, partial.to_owned())));
        index = at + 1;
    }
    let mut to_visit: Vec<Vec<String>> = vec![vec![beneath]];
    while let Some(top) = to_visit.last_mut() {
        let Some(current) = top.pop() else {
            to_visit.pop();
            stack.pop();
            continue;
        };
        let normalized = if current == "/" {
            ".".to_owned()
        } else {
            current[1..].to_owned()
        };
        let current_is_dir = is_dir(&normalized);
        let probe = if current_is_dir {
            format!("{current}/")
        } else {
            current.clone()
        };
        if matches_stack(&stack, &probe) {
            continue;
        }
        if current_is_dir {
            let prefix = if current == "/" {
                current.clone()
            } else {
                format!("{current}/")
            };
            stack.push(ignore_for_dir(&normalized).map(|ignore| (ignore, prefix)));
            let children = list_dir(&normalized)?
                .into_iter()
                .map(|x| format!("/{x}"))
                .collect();
            to_visit.push(children);
            if include_dirs {
                result.push(normalized);
            }
        } else {
            result.push(normalized);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ignore(patterns: &[&str]) -> Ignore {
        let patterns: Vec<String> = patterns.iter().map(|p| (*p).to_owned()).collect();
        Ignore::new(&patterns, false, |_, _| panic!("invalid pattern"))
    }

    #[test]
    fn simple_patterns() {
        let i = ignore(&["*.o"]);
        assert!(i.ignores("main.o"));
        assert!(!i.ignores("main.c"));
        assert!(!i.ignores("lib/"));
        assert!(i.ignores("lib/helper.o"));
        assert!(!i.ignores("lib/helper.c"));
    }

    #[test]
    fn negation_comments_and_directories() {
        let i = ignore(&["# Comment\nobj/\n*.o\n!main.o\n"]);
        assert!(i.ignores("obj/README.md"));
        assert!(i.ignores("lib.o"));
        assert!(!i.ignores("main.o"));
        assert!(!i.ignores("obj"));
        assert!(i.ignores("obj/"));
    }

    #[test]
    fn anchoring_and_double_stars() {
        let i = ignore(&["/build", "a/**/b", "**/c", "d/**"]);
        assert!(i.ignores("build"));
        assert!(!i.ignores("x/build"));
        assert!(i.ignores("a/b"));
        assert!(i.ignores("a/x/y/b"));
        assert!(i.ignores("c"));
        assert!(i.ignores("x/c"));
        assert!(i.ignores("d/e"));
        assert!(i.ignores("d/"));
    }

    #[test]
    fn the_base_rules_of_pub() {
        let i = ignore(&[
            ".*",
            "!.htaccess",
            "pubspec.lock",
            "!pubspec.lock/",
            "/pubspec_overrides.yaml",
        ]);
        assert!(i.ignores(".git/"));
        assert!(i.ignores(".dart_tool/x"));
        assert!(!i.ignores(".htaccess"));
        assert!(i.ignores("pubspec.lock"));
        assert!(i.ignores("lib/pubspec.lock"));
        assert!(!i.ignores("pubspec.lock/"));
        assert!(i.ignores("pubspec_overrides.yaml"));
        assert!(!i.ignores("lib/pubspec_overrides.yaml"));
    }

    #[test]
    fn character_ranges_and_escapes() {
        let i = ignore(&["[!a]x", "\\#y", "z\\ ", "[a-c]q", "[&~]w"]);
        assert!(i.ignores("bx"));
        assert!(!i.ignores("ax"));
        assert!(i.ignores("#y"));
        assert!(i.ignores("z "));
        assert!(i.ignores("bq"));
        assert!(i.ignores("&w"));
        assert!(i.ignores("~w"));
    }

    #[test]
    fn trailing_spaces_are_removed_unless_escaped() {
        let i = ignore(&["a  "]);
        assert!(i.ignores("a"));
    }

    #[test]
    fn invalid_patterns_are_reported() {
        let mut problems = Vec::new();
        let patterns = vec!["[abc".to_owned(), "x\\".to_owned(), "ok".to_owned()];
        let i = Ignore::new(&patterns, false, |p, m| problems.push(format!("{p}: {m}")));
        assert_eq!(
            problems,
            [
                "[abc: Pattern \"[abc\" had an invalid `[a-b]` style character range",
                "x\\: Pattern \"x\\\" end of pattern inside character escape."
            ]
        );
        assert!(i.ignores("ok"));
    }

    #[test]
    fn case_insensitive_rules() {
        let patterns = vec!["*.pdb".to_owned()];
        let i = Ignore::new(&patterns, true, |_, _| {});
        assert!(i.ignores("A.PDB"));
    }

    /// The walk over a small in-memory tree, with an ignore file per
    /// directory: the inner rules win, and order is Pub's (last listed
    /// entry first).
    #[test]
    fn walk_applies_the_ignore_files_of_each_directory() {
        let root = Rc::new(ignore(&["*.g.dart", "out/"]));
        let lib = Rc::new(ignore(&["!keep.g.dart"]));
        let tree: &[(&str, &[&str])] = &[
            (".", &["a.dart", "b.g.dart", "lib", "out"]),
            ("lib", &["lib/keep.g.dart", "lib/x.g.dart", "lib/y.dart"]),
            ("out", &["out/z"]),
        ];
        let files = list_files::<()>(
            "",
            &mut |dir: &str| {
                Ok(tree
                    .iter()
                    .find(|(d, _)| *d == dir)
                    .map(|(_, c)| c.iter().map(|s| (*s).to_owned()).collect())
                    .unwrap_or_default())
            },
            &mut |dir: &str| match dir {
                "." => Some(root.clone()),
                "lib" => Some(lib.clone()),
                _ => None,
            },
            &mut |path: &str| tree.iter().any(|(d, _)| *d == path),
            true,
        )
        .unwrap();
        assert_eq!(
            files,
            [".", "lib", "lib/y.dart", "lib/keep.g.dart", "a.dart"]
        );
    }

    #[test]
    fn walk_below_an_ignored_directory_is_empty() {
        let root = Rc::new(ignore(&["sub/"]));
        let files = list_files::<()>(
            "sub/pkg",
            &mut |_| Ok(vec!["sub/pkg/a".to_owned()]),
            &mut |dir: &str| (dir == ".").then(|| root.clone()),
            &mut |path: &str| path != "sub/pkg/a",
            false,
        )
        .unwrap();
        assert!(files.is_empty());
    }
}
