//! Pub's explanation of a failed resolution (`SolveFailure` and its
//! `_Writer`): the derivation of the final incompatibility, numbered where
//! a fact is used more than once.
use super::{
    manifest::{Dependency, Description},
    solver::{Cause, Incompatibility, Term},
    version::Version,
};
use std::collections::{BTreeSet, HashMap};

/// Pub's `PackageDetail`: which parts of a package reference to show.
#[derive(Clone, Copy, Default)]
struct Detail {
    show_version: Option<bool>,
    show_source: Option<bool>,
    show_description: bool,
}

type Details = HashMap<String, Detail>;

fn format_description(description: &Description) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    description.format(&cwd)
}

fn ref_string(package: &Dependency, detail: Option<&Detail>) -> String {
    let detail = detail.copied().unwrap_or_default();
    if package.description.is_root() {
        return package.name.clone();
    }
    let mut text = package.name.clone();
    if detail
        .show_source
        .unwrap_or(!matches!(package.description, Description::Hosted(_)))
    {
        text.push_str(&format!(" from {}", package.description.source()));
        if detail.show_description {
            text.push_str(&format!(" {}", format_description(&package.description)));
        }
    }
    text
}

fn range_string(package: &Dependency, detail: Option<&Detail>) -> String {
    let detail = detail.copied().unwrap_or_default();
    let mut text = package.name.clone();
    let show_version = !package.description.is_root()
        && (!package.constraint.is_any() || package.description.has_multiple_versions());
    if detail.show_version.unwrap_or(show_version) {
        text.push_str(&format!(" {}", package.constraint.terse()));
    }
    if !package.description.is_root()
        && detail
            .show_source
            .unwrap_or(!matches!(package.description, Description::Hosted(_)))
    {
        text.push_str(&format!(" from {}", package.description.source()));
        if detail.show_description {
            text.push_str(&format!(" {}", format_description(&package.description)));
        }
    }
    text
}

fn terse_ref(term: &Term, details: &Details) -> String {
    ref_string(&term.package, details.get(&term.package.name))
}

fn terse(term: &Term, details: &Details, allow_every: bool) -> String {
    if allow_every && term.package.constraint.is_any() {
        format!("every version of {}", terse_ref(term, details))
    } else {
        range_string(&term.package, details.get(&term.package.name))
    }
}

fn sdk_title(sdk: &str) -> &'static str {
    match sdk {
        "dart" => "Dart",
        "flutter" => "Flutter",
        _ => "Fuchsia",
    }
}

fn no_null_safety(cause: &Cause) -> bool {
    match cause {
        Cause::Sdk { constraint, sdk } => {
            sdk == "dart"
                && constraint
                    .as_ref()
                    .is_none_or(|c| super::manifest::language_of(c) < (2, 12))
        }
        _ => false,
    }
}

fn sdk_requirement(cause: &Cause) -> String {
    let Cause::Sdk { constraint, sdk } = cause else {
        return String::new();
    };
    if no_null_safety(cause) {
        return "doesn't support null safety".into();
    }
    if sdk != "dart" {
        return format!("requires the {} SDK", sdk_title(sdk));
    }
    format!(
        "requires SDK version {}",
        constraint
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "null".into())
    )
}

fn single(incompatibility: &Incompatibility, positive: bool) -> Option<&Term> {
    let mut found = None;
    for term in &incompatibility.terms {
        if term.positive != positive {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(term);
    }
    found
}

fn satisfies(a: &Term, b: &Term) -> bool {
    a.satisfies(b)
}

struct Writer<'a> {
    all: &'a [Incompatibility],
    root: usize,
    derivations: HashMap<usize, usize>,
    lines: Vec<(String, Option<usize>)>,
    line_numbers: HashMap<usize, usize>,
    sdk_version: String,
}

impl<'a> Writer<'a> {
    fn count(&mut self, id: usize) {
        if let Some(count) = self.derivations.get_mut(&id) {
            *count += 1;
            return;
        }
        if let Cause::Conflict(a, b) = self.all[id].cause {
            self.count(a);
            self.count(b);
        }
        self.derivations.insert(id, 1);
    }

    fn external(&self, id: usize, result: &mut Vec<usize>) {
        match self.all[id].cause {
            Cause::Conflict(a, b) => {
                self.external(a, result);
                self.external(b, result);
            }
            _ => result.push(id),
        }
    }

    fn is_conflict(&self, id: usize) -> bool {
        matches!(self.all[id].cause, Cause::Conflict(..))
    }

    fn to_string(&self, id: usize, details: &Details) -> String {
        let incompatibility = &self.all[id];
        let terms = &incompatibility.terms;
        match &incompatibility.cause {
            Cause::Dependency => {
                return format!(
                    "{} depends on {}",
                    terse(&terms[0], details, true),
                    terse(&terms[1], details, false)
                );
            }
            cause @ Cause::Sdk { .. } => {
                return format!(
                    "{} {}",
                    terse(&terms[0], details, true),
                    sdk_requirement(cause)
                );
            }
            Cause::NoVersions => {
                return format!(
                    "no versions of {} match {}",
                    terse_ref(&terms[0], details),
                    terms[0].package.constraint.terse()
                );
            }
            Cause::NotFound(message) => {
                return format!(
                    "{} doesn't exist ({message})",
                    terse_ref(&terms[0], details)
                );
            }
            Cause::UnknownSource => {
                return format!(
                    "{} comes from unknown source \"{}\"",
                    terms[0].package.name,
                    terms[0].package.description.source()
                );
            }
            Cause::Root => {
                return format!(
                    "{} is {}",
                    terms[0].package.name,
                    terms[0].package.constraint.terse()
                );
            }
            Cause::Conflict(..) => {}
        }
        if incompatibility.is_failure() {
            return "version solving failed".into();
        }
        if let [term] = terms.as_slice() {
            let what = if term.positive {
                "forbidden"
            } else {
                "required"
            };
            return if term.package.constraint.is_any() {
                format!("{} is {what}", terse_ref(term, details))
            } else {
                format!("{} is {what}", terse(term, details, false))
            };
        }
        if let [first, second] = terms.as_slice()
            && first.positive == second.positive
        {
            if first.positive {
                let show = |t: &Term| {
                    if t.package.constraint.is_any() {
                        terse_ref(t, details)
                    } else {
                        terse(t, details, false)
                    }
                };
                return format!("{} is incompatible with {}", show(first), show(second));
            }
            return format!(
                "either {} or {}",
                terse(first, details, false),
                terse(second, details, false)
            );
        }
        let positive: Vec<String> = terms
            .iter()
            .filter(|t| t.positive)
            .map(|t| terse(t, details, false))
            .collect();
        let negative: Vec<String> = terms
            .iter()
            .filter(|t| !t.positive)
            .map(|t| terse(t, details, false))
            .collect();
        if !positive.is_empty() && !negative.is_empty() {
            if positive.len() == 1 {
                let term = terms.iter().find(|t| t.positive).expect("one positive");
                format!(
                    "{} requires {}",
                    terse(term, details, true),
                    negative.join(" or ")
                )
            } else {
                format!(
                    "if {} then {}",
                    positive.join(" and "),
                    negative.join(" or ")
                )
            }
        } else if !positive.is_empty() {
            format!("one of {} must be false", positive.join(" or "))
        } else {
            format!("one of {} must be true", negative.join(" or "))
        }
    }

    fn verb(&self, id: usize) -> &'static str {
        if matches!(self.all[id].cause, Cause::Dependency) {
            "depends on"
        } else {
            "requires"
        }
    }

    fn and_to_string(
        &self,
        this: usize,
        other: usize,
        details: &Details,
        this_line: Option<usize>,
        other_line: Option<usize>,
    ) -> String {
        if let Some(text) = self.requires_both(this, other, details, this_line, other_line) {
            return text;
        }
        if let Some(text) = self.requires_through(this, other, details, this_line, other_line) {
            return text;
        }
        if let Some(text) = self.requires_forbidden(this, other, details, this_line, other_line) {
            return text;
        }
        let mut text = self.to_string(this, details);
        if let Some(line) = this_line {
            text.push_str(&format!(" {line}"));
        }
        text.push_str(&format!(" and {}", self.to_string(other, details)));
        if other_line.is_some()
            && let Some(line) = this_line
        {
            // Pub prints `thisLine` here.
            text.push_str(&format!(" {line}"));
        }
        text
    }

    fn requires_both(
        &self,
        this: usize,
        other: usize,
        details: &Details,
        this_line: Option<usize>,
        other_line: Option<usize>,
    ) -> Option<String> {
        let (a, b) = (&self.all[this], &self.all[other]);
        if a.terms.len() == 1 || b.terms.len() == 1 {
            return None;
        }
        let this_positive = single(a, true)?;
        let other_positive = single(b, true)?;
        if this_positive.package != other_positive.package {
            return None;
        }
        let negatives = |i: &Incompatibility| {
            i.terms
                .iter()
                .filter(|t| !t.positive)
                .map(|t| terse(t, details, false))
                .collect::<Vec<_>>()
                .join(" or ")
        };
        let both_dependencies =
            matches!(a.cause, Cause::Dependency) && matches!(b.cause, Cause::Dependency);
        let mut text = format!(
            "{} {} both {}",
            terse(this_positive, details, true),
            if both_dependencies {
                "depends on"
            } else {
                "requires"
            },
            negatives(a)
        );
        if let Some(line) = this_line {
            text.push_str(&format!(" ({line})"));
        }
        text.push_str(&format!(" and {}", negatives(b)));
        if let Some(line) = other_line {
            text.push_str(&format!(" ({line})"));
        }
        Some(text)
    }

    fn requires_through(
        &self,
        this: usize,
        other: usize,
        details: &Details,
        this_line: Option<usize>,
        other_line: Option<usize>,
    ) -> Option<String> {
        let (a, b) = (&self.all[this], &self.all[other]);
        if a.terms.len() == 1 || b.terms.len() == 1 {
            return None;
        }
        let this_negative = single(a, false);
        let other_negative = single(b, false);
        if this_negative.is_none() && other_negative.is_none() {
            return None;
        }
        let this_positive = single(a, true);
        let other_positive = single(b, true);
        let (prior, prior_negative, prior_line, latter, latter_line) = if let (Some(n), Some(p)) =
            (this_negative, other_positive)
            && n.package.name == p.package.name
            && satisfies(&n.inverse(), p)
        {
            (this, n, this_line, other, other_line)
        } else if let (Some(n), Some(p)) = (other_negative, this_positive)
            && n.package.name == p.package.name
            && satisfies(&n.inverse(), p)
        {
            (other, n, other_line, this, this_line)
        } else {
            return None;
        };
        let positives: Vec<&Term> = self.all[prior]
            .terms
            .iter()
            .filter(|t| t.positive)
            .collect();
        let mut text = if positives.len() > 1 {
            format!(
                "if {} then ",
                positives
                    .iter()
                    .map(|t| terse(t, details, false))
                    .collect::<Vec<_>>()
                    .join(" or ")
            )
        } else {
            format!(
                "{} {} ",
                terse(positives[0], details, true),
                self.verb(prior)
            )
        };
        text.push_str(&terse(prior_negative, details, false));
        if let Some(line) = prior_line {
            text.push_str(&format!(" ({line})"));
        }
        text.push_str(&format!(" which {} ", self.verb(latter)));
        text.push_str(
            &self.all[latter]
                .terms
                .iter()
                .filter(|t| !t.positive)
                .map(|t| terse(t, details, false))
                .collect::<Vec<_>>()
                .join(" or "),
        );
        if let Some(line) = latter_line {
            text.push_str(&format!(" ({line})"));
        }
        Some(text)
    }

    fn requires_forbidden(
        &self,
        this: usize,
        other: usize,
        details: &Details,
        this_line: Option<usize>,
        other_line: Option<usize>,
    ) -> Option<String> {
        let (a, b) = (&self.all[this], &self.all[other]);
        if a.terms.len() != 1 && b.terms.len() != 1 {
            return None;
        }
        let (prior, latter, prior_line, latter_line) = if a.terms.len() == 1 {
            (other, this, other_line, this_line)
        } else {
            (this, other, this_line, other_line)
        };
        let negative = single(&self.all[prior], false)?;
        if !satisfies(&negative.inverse(), &self.all[latter].terms[0]) {
            return None;
        }
        let positives: Vec<&Term> = self.all[prior]
            .terms
            .iter()
            .filter(|t| t.positive)
            .collect();
        let mut text = if positives.len() > 1 {
            format!(
                "if {} then ",
                positives
                    .iter()
                    .map(|t| terse(t, details, false))
                    .collect::<Vec<_>>()
                    .join(" or ")
            )
        } else {
            format!(
                "{} {} ",
                terse(positives[0], details, true),
                self.verb(prior)
            )
        };
        let latter_incompatibility = &self.all[latter];
        if matches!(latter_incompatibility.cause, Cause::UnknownSource) {
            let package = &latter_incompatibility.terms[0].package;
            text.push_str(&format!("{} ", package.name));
            if let Some(line) = prior_line {
                text.push_str(&format!("({line}) "));
            }
            text.push_str(&format!(
                "from unknown source \"{}\"",
                package.description.source()
            ));
            if let Some(line) = latter_line {
                text.push_str(&format!(" ({line})"));
            }
            return Some(text);
        }
        text.push_str(&format!(
            "{} ",
            terse(&latter_incompatibility.terms[0], details, false)
        ));
        if let Some(line) = prior_line {
            text.push_str(&format!("({line}) "));
        }
        match &latter_incompatibility.cause {
            cause @ Cause::Sdk { .. } => {
                let requirement = sdk_requirement(cause);
                if no_null_safety(cause) {
                    text.push_str("which doesn't support null safety");
                } else {
                    text.push_str(&format!("which {requirement}"));
                }
            }
            Cause::NoVersions => text.push_str("which doesn't match any versions"),
            Cause::NotFound(message) => text.push_str(&format!("which doesn't exist ({message})")),
            _ => text.push_str("which is forbidden"),
        }
        if let Some(line) = latter_line {
            text.push_str(&format!(" ({line})"));
        }
        Some(text)
    }

    fn write(&mut self, id: usize, message: String, numbered: bool) {
        if numbered {
            let number = self.line_numbers.len() + 1;
            self.line_numbers.insert(id, number);
            self.lines.push((message, Some(number)));
        } else {
            self.lines.push((message, None));
        }
    }

    fn details_for_cause(&self, conflict: usize, other: usize) -> Details {
        let mut conflict_packages: HashMap<&str, &Dependency> = HashMap::new();
        for term in &self.all[conflict].terms {
            if !term.is_root() {
                conflict_packages.insert(&term.package.name, &term.package);
            }
        }
        let mut details = Details::new();
        for term in &self.all[other].terms {
            if term.is_root() {
                continue;
            }
            let Some(package) = conflict_packages.get(term.package.name.as_str()) else {
                continue;
            };
            if package.description.source() != term.package.description.source() {
                details.insert(
                    term.package.name.clone(),
                    Detail {
                        show_source: Some(true),
                        show_version: Some(false),
                        show_description: false,
                    },
                );
            } else if package.description != term.package.description {
                details.insert(
                    term.package.name.clone(),
                    Detail {
                        show_description: true,
                        show_version: Some(false),
                        show_source: None,
                    },
                );
            }
        }
        details
    }

    fn is_single_line(&self, id: usize) -> bool {
        let Cause::Conflict(a, b) = self.all[id].cause else {
            return false;
        };
        !self.is_conflict(a) && !self.is_conflict(b)
    }

    fn is_collapsible(&self, id: usize) -> bool {
        if self.derivations[&id] > 1 {
            return false;
        }
        let Cause::Conflict(a, b) = self.all[id].cause else {
            return false;
        };
        if self.is_conflict(a) == self.is_conflict(b) {
            return false;
        }
        let complex = if self.is_conflict(a) { a } else { b };
        !self.line_numbers.contains_key(&complex)
    }

    fn visit(&mut self, id: usize, details: &Details, conclusion: bool) {
        let numbered = conclusion || self.derivations[&id] > 1;
        let conjunction = if conclusion || id == self.root {
            "So,"
        } else {
            "And"
        };
        let incompatibility_string = self.to_string(id, details);
        let Cause::Conflict(conflict, other) = self.all[id].cause else {
            return;
        };
        let mut details_for_cause = self.details_for_cause(conflict, other);
        let cause_conflict = self.is_conflict(conflict);
        let other_conflict = self.is_conflict(other);
        if cause_conflict && other_conflict {
            let conflict_line = self.line_numbers.get(&conflict).copied();
            let other_line = self.line_numbers.get(&other).copied();
            if let (Some(cl), Some(ol)) = (conflict_line, other_line) {
                let text =
                    self.and_to_string(conflict, other, &details_for_cause, Some(cl), Some(ol));
                self.write(
                    id,
                    format!("Because {text}, {incompatibility_string}."),
                    numbered,
                );
            } else if conflict_line.is_some() || other_line.is_some() {
                let (with_line, without_line, line) = match conflict_line {
                    Some(line) => (conflict, other, line),
                    None => (other, conflict, other_line.expect("one line")),
                };
                self.visit(without_line, &details_for_cause, false);
                let text = self.to_string(with_line, &details_for_cause);
                self.write(
                    id,
                    format!("{conjunction} because {text} ({line}), {incompatibility_string}."),
                    numbered,
                );
            } else {
                let single_conflict = self.is_single_line(conflict);
                let single_other = self.is_single_line(other);
                if single_other || single_conflict {
                    let (first, second) = if single_other {
                        (conflict, other)
                    } else {
                        (other, conflict)
                    };
                    self.visit(first, &details_for_cause, false);
                    self.visit(second, &details_for_cause, false);
                    self.write(id, format!("Thus, {incompatibility_string}."), numbered);
                } else {
                    self.visit(conflict, &Details::new(), true);
                    self.lines.push((String::new(), None));
                    self.visit(other, &details_for_cause, false);
                    let text = self.to_string(conflict, &details_for_cause);
                    let line = self.line_numbers.get(&conflict).copied().unwrap_or(0);
                    self.write(
                        id,
                        format!("{conjunction} because {text} ({line}), {incompatibility_string}."),
                        numbered,
                    );
                }
            }
        } else if cause_conflict || other_conflict {
            let (derived, ext) = if cause_conflict {
                (conflict, other)
            } else {
                (other, conflict)
            };
            if let Some(line) = self.line_numbers.get(&derived).copied() {
                let text = self.and_to_string(ext, derived, &details_for_cause, None, Some(line));
                self.write(
                    id,
                    format!("Because {text}, {incompatibility_string}."),
                    numbered,
                );
            } else if self.is_collapsible(derived) {
                let Cause::Conflict(dc, dother) = self.all[derived].cause else {
                    unreachable!("collapsible incompatibilities are derived");
                };
                let (collapsed_derived, collapsed_ext) = if self.is_conflict(dc) {
                    (dc, dother)
                } else {
                    (dother, dc)
                };
                for (name, detail) in self.details_for_cause(dc, dother) {
                    details_for_cause.entry(name).or_insert(detail);
                }
                self.visit(collapsed_derived, &details_for_cause, false);
                let text = self.and_to_string(collapsed_ext, ext, &details_for_cause, None, None);
                self.write(
                    id,
                    format!("{conjunction} because {text}, {incompatibility_string}."),
                    numbered,
                );
            } else {
                self.visit(derived, &details_for_cause, false);
                let text = self.to_string(ext, &details_for_cause);
                self.write(
                    id,
                    format!("{conjunction} because {text}, {incompatibility_string}."),
                    numbered,
                );
            }
        } else {
            let text = self.and_to_string(conflict, other, &details_for_cause, None, None);
            self.write(
                id,
                format!("Because {text}, {incompatibility_string}."),
                numbered,
            );
        }
    }

    fn render(mut self) -> String {
        let mut external = Vec::new();
        self.external(self.root, &mut external);
        let mut notices = BTreeSet::new();
        let mut hints = BTreeSet::new();
        for &id in &external {
            match &self.all[id].cause {
                cause @ Cause::Sdk { sdk, .. } => {
                    if sdk == "dart" {
                        notices.insert(format!(
                            "The current Dart SDK version is {}.",
                            self.sdk_version
                        ));
                    }
                    if no_null_safety(cause) {
                        if let Cause::Sdk { constraint, .. } = cause {
                            hints.insert(format!(
                                "The lower bound of \"sdk: '{}'\" must be 2.12.0 or higher to enable null safety.\nFor details, see https://dart.dev/null-safety",
                                constraint.as_ref().map(ToString::to_string).unwrap_or_default()
                            ));
                        }
                    } else if sdk == "flutter" {
                        hints.insert(
                            "Flutter users should use `flutter pub` instead of `dart pub`."
                                .to_owned(),
                        );
                    }
                }
                Cause::NotFound(message)
                    if message.contains("--offline") || message.contains("in cache") =>
                {
                    hints.insert("Try again without --offline!".to_owned());
                }
                _ => {}
            }
        }
        let mut buffer = String::new();
        for notice in &notices {
            buffer.push_str(notice);
            buffer.push('\n');
        }
        if !notices.is_empty() {
            buffer.push('\n');
        }
        if self.is_conflict(self.root) {
            let root = self.root;
            self.visit(root, &Details::new(), false);
        } else {
            let text = self.to_string(self.root, &Details::new());
            self.write(
                self.root,
                format!("Because {text}, version solving failed."),
                false,
            );
        }
        let padding = self
            .line_numbers
            .values()
            .max()
            .map(|n| format!("({n}) ").len())
            .unwrap_or(0);
        let mut last_was_empty = false;
        for (message, number) in &self.lines {
            if message.is_empty() {
                if !last_was_empty {
                    buffer.push('\n');
                }
                last_was_empty = true;
                continue;
            }
            last_was_empty = false;
            let prefix = match number {
                Some(number) => format!("{:<padding$}", format!("({number})")),
                None => " ".repeat(padding),
            };
            buffer.push_str(&prefix);
            buffer.push_str(message);
            buffer.push('\n');
        }
        for hint in hints {
            buffer.push('\n');
            buffer.push_str(&hint);
            buffer.push('\n');
        }
        buffer
    }
}

/// Whether the failure comes from a package that does not exist (Pub then
/// exits with `UNAVAILABLE`).
pub(super) fn package_not_found(all: &[Incompatibility], root: usize) -> bool {
    fn walk(all: &[Incompatibility], id: usize) -> bool {
        match all[id].cause {
            Cause::Conflict(a, b) => walk(all, a) || walk(all, b),
            Cause::NotFound(_) => true,
            _ => false,
        }
    }
    walk(all, root)
}

/// The text Pub prints for a failed resolution.
pub(super) fn explain(all: &[Incompatibility], root: usize, sdk: &Version) -> String {
    let mut writer = Writer {
        all,
        root,
        derivations: HashMap::new(),
        lines: Vec::new(),
        line_numbers: HashMap::new(),
        sdk_version: sdk.to_string(),
    };
    writer.count(root);
    let text = writer.render();
    text.strip_suffix('\n').unwrap_or(&text).to_owned()
}
