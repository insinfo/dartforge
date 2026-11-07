//! Proofs for retaining reference records across declaration position and API edits.
//!
//! The dependency graph is still authoritative. Retention requires an unchanged
//! API or an isolated unused top-level function delta, and every foreign
//! declaration range must translate without overlapping the source replacement.

use crate::DocumentStore;
use crate::reference_index::{FileRecord, ReferenceIndex, text_stamp};
use dartforge_elements::incremental::{ApiDeclaration, api_signature_and_digest};
use dartforge_frontend::ast::{DeclKind, FunctionKind};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One previous source per recorded open file, bounded independently of edits.
#[derive(Default)]
pub(crate) struct Sources {
    texts: HashMap<PathBuf, (u64, Arc<str>)>,
    bytes: usize,
}

const SOURCE_BUDGET: usize = 1024 * 1024;

impl Sources {
    pub(crate) fn clear(&mut self) {
        self.texts.clear();
        self.bytes = 0;
    }

    pub(crate) fn remove(&mut self, path: &Path) {
        if let Some((_, text)) = self.texts.remove(path) {
            self.bytes -= text.len();
        }
    }

    pub(crate) fn remember(&mut self, record: &FileRecord, documents: &DocumentStore) {
        let Some(text) = documents.get(&record.uri) else {
            return;
        };
        if text.len() > SOURCE_BUDGET || text_stamp(text) != record.stamp {
            self.remove(&record.path);
            return;
        }
        if self
            .texts
            .get(&record.path)
            .is_some_and(|(stamp, _)| *stamp == record.stamp)
        {
            return;
        }
        self.remove(&record.path);
        if self.bytes + text.len() > SOURCE_BUDGET {
            self.clear();
        }
        self.bytes += text.len();
        self.texts
            .insert(record.path.clone(), (record.stamp, Arc::from(text)));
    }

    pub(crate) fn change(&self, record: &FileRecord, current: &str) -> Option<DeclarationChange> {
        // Foreign path tables must identify this path without a lossy spelling.
        record.path.to_str()?;
        if current.len() > SOURCE_BUDGET || record.length as usize > SOURCE_BUDGET {
            return None;
        }
        let old = match self.texts.get(&record.path) {
            Some((stamp, text)) if *stamp == record.stamp => Arc::clone(text),
            _ => {
                let text = std::fs::read_to_string(&record.path).ok()?;
                if text_stamp(&text) != record.stamp {
                    return None;
                }
                Arc::from(text)
            }
        };
        DeclarationChange::new(record, &old, current)
    }
}

/// A single contiguous source replacement; ranges crossing it are unproved.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Edit {
    start: u32,
    old_end: u32,
    new_end: u32,
    before_len: u32,
    after_len: u32,
}

impl Edit {
    fn between(old: &str, new: &str) -> Option<Self> {
        if old == new {
            return None;
        }
        let mut start = old
            .bytes()
            .zip(new.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        while !old.is_char_boundary(start) || !new.is_char_boundary(start) {
            start -= 1;
        }
        let mut suffix = old.as_bytes()[start..]
            .iter()
            .rev()
            .zip(new.as_bytes()[start..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        while !old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix)
        {
            suffix -= 1;
        }
        Some(Self {
            start: u32::try_from(start).ok()?,
            old_end: u32::try_from(old.len() - suffix).ok()?,
            new_end: u32::try_from(new.len() - suffix).ok()?,
            before_len: u32::try_from(old.len()).ok()?,
            after_len: u32::try_from(new.len()).ok()?,
        })
    }

    pub(crate) fn span(self, start: u32, end: u32) -> Option<(u32, u32)> {
        if start > end || end > self.before_len || (start == end && start == self.start) {
            return None;
        }
        if end <= self.start {
            return Some((start, end));
        }
        if start < self.old_end {
            return None;
        }
        let delta = i64::from(self.new_end) - i64::from(self.old_end);
        let start = u32::try_from(i64::from(start) + delta).ok()?;
        let end = u32::try_from(i64::from(end) + delta).ok()?;
        (end <= self.after_len).then_some((start, end))
    }
}

/// Why one library's source delta cannot change its dependents' lookup.
pub(crate) struct DeclarationChange {
    pub(crate) path: PathBuf,
    pub(crate) edit: Edit,
    stamp: u64,
    api: u64,
    names: Vec<String>,
}

impl DeclarationChange {
    fn new(record: &FileRecord, old: &str, new: &str) -> Option<Self> {
        if dartforge_frontend::features::version_marker(old).map(|(version, _)| version)
            != dartforge_frontend::features::version_marker(new).map(|(version, _)| version)
        {
            return None;
        }
        let mut names = dartforge_intern::Interner::new();
        let a = dartforge_frontend::parser::parse(old, &mut names);
        let b = dartforge_frontend::parser::parse(new, &mut names);
        if !a.diagnostics.is_empty() || !b.diagnostics.is_empty() {
            return None;
        }
        let (old_api, old_digest) = api_signature_and_digest(old, &a.ast, &a.unit);
        let (new_api, new_digest) = api_signature_and_digest(new, &b.ast, &b.unit);
        if old_api != record.api || old_api.is_none() {
            return None;
        }
        let (old_digest, new_digest) = (old_digest?, new_digest?);
        if old_digest.directives != new_digest.directives {
            return None;
        }
        let declarations = |items: Vec<ApiDeclaration>| -> Option<BTreeMap<(Option<String>,String),ApiDeclaration>> {
            let mut map = BTreeMap::new();
            for item in items {
                if map.insert((item.owner.clone(), item.name.clone()), item).is_some() { return None; }
            }
            Some(map)
        };
        let old_map = declarations(old_digest.declarations)?;
        let new_map = declarations(new_digest.declarations)?;
        let mut changed = Vec::new();
        for (key, declaration) in &old_map {
            match new_map.get(key) {
                Some(new) if new == declaration => {}
                Some(_) => return None,
                None if key.0.is_none() => changed.push(key.1.clone()),
                None => return None,
            }
        }
        for key in new_map.keys().filter(|key| !old_map.contains_key(*key)) {
            if key.0.is_some() {
                return None;
            }
            changed.push(key.1.clone());
        }
        changed.sort();
        changed.dedup();
        if changed.is_empty() && old_api != new_api {
            return None;
        }
        let retained = |source: &str,
                        parsed: &dartforge_frontend::parser::Parsed,
                        declarations: &BTreeMap<(Option<String>, String), ApiDeclaration>|
         -> Option<bool> {
            let mut spans = Vec::new();
            let mut functions = HashSet::new();
            for id in &parsed.unit.declarations {
                let declaration = parsed.ast.decl(*id);
                let DeclKind::Function(id) = &declaration.kind else {
                    continue;
                };
                if declaration.augment || !declaration.metadata.is_empty() {
                    continue;
                }
                let function = parsed.ast.function(*id);
                if function.kind != FunctionKind::Function {
                    continue;
                }
                let name = function.name?;
                let word = source.get(name.span.start..name.span.end)?;
                if changed.iter().any(|name| name == word) {
                    functions.insert(word.to_string());
                    spans.push(declaration.span);
                }
            }
            for name in &changed {
                if declarations.contains_key(&(None, name.clone()))
                    && !functions.contains(name.as_str())
                {
                    return None;
                }
            }
            spans.sort_by_key(|span| span.start);
            let mut at = 0;
            for span in spans {
                if span.start < at
                    || changed
                        .iter()
                        .any(|name| source[at..span.start].contains(name.as_str()))
                {
                    return Some(false);
                }
                at = span.end;
            }
            Some(
                !changed
                    .iter()
                    .any(|name| source[at..].contains(name.as_str())),
            )
        };
        if !retained(old, &a, &old_map)? || !retained(new, &b, &new_map)? {
            return None;
        }
        Some(Self {
            path: record.path.clone(),
            edit: Edit::between(old, new)?,
            stamp: text_stamp(new),
            api: new_api?,
            names: changed,
        })
    }

    pub(crate) fn proves_unaffected(
        &self,
        index: &ReferenceIndex,
        documents: &DocumentStore,
    ) -> bool {
        let Some(owner) = index.file(&self.path) else {
            return false;
        };
        let mut paths: HashSet<_> = index
            .dependents(std::slice::from_ref(&self.path))
            .into_iter()
            .collect();
        paths.extend(
            index
                .records()
                .filter(|r| r.library == owner.library)
                .map(|r| r.path.clone()),
        );
        // The proof is local to one unchanged package-map context. A nested
        // package or another driver's catalog still takes the broad path.
        let mut contexts = HashSet::new();
        let mut checked_directories = HashSet::new();
        for path in std::iter::once(&self.path).chain(paths.iter()) {
            let Some(directory) = path.parent() else {
                return false;
            };
            if !checked_directories.insert(directory.to_path_buf()) {
                continue;
            }
            if crate::project::project_root(path) != index.root {
                return false;
            }
            let configuration = dartforge_elements::PackageConfig::discover(path);
            if contexts.insert(configuration) && !index.package_configuration_current(path) {
                return false;
            }
        }
        paths
            .into_iter()
            .filter(|path| path != &self.path)
            .all(|path| {
                let Some(record) = index.file(&path) else {
                    return false;
                };
                let text = documents
                    .get(&record.uri)
                    .map(str::to_string)
                    .or_else(|| std::fs::read_to_string(&path).ok());
                text.is_some_and(|text| {
                    text_stamp(&text) == record.stamp
                        && !self.names.iter().any(|name| text.contains(name.as_str()))
                })
            })
    }

    pub(crate) fn owner_is_fresh(
        &self,
        index: &ReferenceIndex,
        refreshed: &HashSet<PathBuf>,
    ) -> bool {
        let Some(owner) = index.file(&self.path) else {
            return false;
        };
        owner.stamp == self.stamp
            && owner.api == Some(self.api)
            && index
                .records()
                .filter(|r| r.library == owner.library)
                .all(|r| refreshed.contains(&r.path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(source: &str) -> FileRecord {
        FileRecord {
            stamp: text_stamp(source),
            length: source.len() as u32,
            api: dartforge_elements::incremental::api_signature(source),
            ..Default::default()
        }
    }

    #[test]
    fn changed_language_or_generated_api_does_not_prove_retained_resolution() {
        let old = "// @dart=3.6\nvoid target() {}";
        let new = "// @dart=3.13\nvoid unused() {}\nvoid target() {}";
        assert!(DeclarationChange::new(&record(old), old, new).is_none());
        let old = "void target() {}";
        for new in [
            "@deprecated\nvoid unused() {}\nvoid target() {}",
            "class Added {}\nvoid target() {}",
            "void target(int changed) {}\nvoid unused() {}",
            "import 'other.dart';\nvoid unused() {}\nvoid target() {}",
        ] {
            assert!(
                DeclarationChange::new(&record(old), old, new).is_none(),
                "{new}"
            );
        }
    }

    #[test]
    fn old_sources_replace_versions_and_stay_within_a_fixed_budget() {
        let mut sources = Sources::default();
        let mut documents = DocumentStore::new();
        let root = std::env::temp_dir().join("dartforge-source-budget");
        for i in 0..12 {
            let path = root.join(format!("{i}.dart"));
            let uri = url::Url::from_file_path(&path).unwrap().to_string();
            for version in 1..=3 {
                let text = format!("// {version}\n{}", " ".repeat(SOURCE_BUDGET / 5));
                documents.open(uri.clone(), version, text.clone());
                let mut record = record(&text);
                record.path = path.clone();
                record.uri = uri.clone();
                sources.remember(&record, &documents);
                assert!(sources.bytes <= SOURCE_BUDGET);
                assert_eq!(
                    sources.bytes,
                    sources
                        .texts
                        .values()
                        .map(|(_, text)| text.len())
                        .sum::<usize>()
                );
                assert_eq!(sources.texts[&path].0, text_stamp(&text));
            }
        }
        let last = root.join("11.dart");
        let before = sources.bytes;
        let removed = sources.texts[&last].1.len();
        sources.remove(&last);
        assert_eq!(sources.bytes, before - removed);
        assert!(!sources.texts.contains_key(&last));
        sources.clear();
        assert_eq!(sources.bytes, 0);
        assert!(sources.texts.is_empty());
    }

    #[test]
    fn relocation_rejects_crossing_ranges_without_partly_changing_the_index() {
        use crate::reference_index::{IdentFact, IndexTarget, TargetKey};
        let root = std::env::temp_dir().join("dartforge-relocation-proof");
        let source = root.join("source.dart");
        let caller = root.join("caller.dart");
        let mut index = ReferenceIndex::new(root);
        let file = index.paths.id(&source.to_string_lossy());
        let target = IndexTarget {
            kind: TargetKey::Top(0),
            concrete: None,
            name: 0,
            word: None,
            group: Vec::new(),
            declarations: vec![(file, 0, 2, 0), (file, 2, 6, 0)],
            declaration: None,
            answer: Some((file, 6, 8)),
            pivot: None,
        };
        let expected = IdentFact::Target(target);
        index.put(vec![FileRecord {
            path: caller.clone(),
            facts: vec![expected.clone()],
            ..Default::default()
        }]);
        let edit = Edit::between("abcdefgh", "abcXYZdefgh").unwrap();
        assert!(!index.relocate_foreign_declarations(&source, edit, &HashSet::new()));
        assert_eq!(index.file(&caller).unwrap().facts[0], expected);
        let fresh = HashSet::from([caller.clone()]);
        assert!(index.relocate_foreign_declarations(&source, edit, &fresh));
        assert_eq!(index.file(&caller).unwrap().facts[0], expected);
    }
}
