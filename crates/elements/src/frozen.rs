//! The frozen prefix of a program (docs/LSP.md, "Projeto: reanalisar só o
//! que mudou"): the libraries that do not change during an editing session
//! (the SDK and the dependencies) come first in every arena, closed under
//! their imports and exports. A reload of the rest keeps the prefix as it is
//! ([`Program::truncate`] and [`crate::load::load_lenient_onto`]): the ids of
//! its units, libraries and elements survive the reload, and whatever was
//! computed from them (types, hierarchy, inferred initializers) stays valid.

use crate::model::{LibraryId, Program, UnitId};

/// The length of each arena at the end of a frozen prefix: the libraries
/// before it, their units and their elements depend on nothing after it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Checkpoint {
    pub units: usize,
    pub libraries: usize,
    pub classes: usize,
    pub extensions: usize,
    pub typedefs: usize,
    pub functions: usize,
    pub variables: usize,
}

/// Which libraries are frozen (before the outline: the library and its units
/// are loaded, its elements are not).
pub type Frozen<'a> = &'a dyn Fn(&Program, LibraryId) -> bool;

impl Program {
    /// The checkpoint after the first `libraries` libraries, when their units
    /// and elements are the first of each arena; `None` otherwise.
    pub fn checkpoint_at(&self, libraries: usize) -> Option<Checkpoint> {
        fn split<T>(items: &[T], before: impl Fn(&T) -> bool) -> Option<usize> {
            let n = items.iter().take_while(|t| before(t)).count();
            items[n..].iter().all(|t| !before(t)).then_some(n)
        }
        let inside = |l: LibraryId| (l.0 as usize) < libraries;
        Some(Checkpoint {
            units: split(&self.units, |u| inside(u.library))?,
            libraries,
            classes: split(&self.classes, |c| inside(c.library))?,
            extensions: split(&self.extensions, |e| inside(e.library))?,
            typedefs: split(&self.typedefs, |t| inside(t.library))?,
            functions: split(&self.functions, |f| inside(f.library))?,
            variables: split(&self.variables, |v| inside(v.library))?,
        })
    }

    /// Drops everything after `at`: what is left is the frozen prefix, whose
    /// libraries reference nothing that was dropped.
    pub fn truncate(&mut self, at: &Checkpoint) {
        self.units.truncate(at.units);
        self.libraries.truncate(at.libraries);
        self.classes.truncate(at.classes);
        self.extensions.truncate(at.extensions);
        self.typedefs.truncate(at.typedefs);
        self.functions.truncate(at.functions);
        self.variables.truncate(at.variables);
        self.augmentations
            .retain(|c, _| (c.0 as usize) < at.classes);
        self.classes_macro.retain(|c| (c.0 as usize) < at.classes);
        self.entry = self.entry.filter(|l| (l.0 as usize) < at.libraries);
        self.element_index = std::sync::OnceLock::new();
        self.times = Default::default();
    }
}

/// Before the outline, reorders the libraries and units so that the ones
/// `frozen` accepts come first (each group in the order it had), after
/// taking out of the group any library that imports or exports one outside
/// it. Returns how many libraries the prefix has.
///
/// # Panics
///
/// When the program already has elements.
pub(crate) fn frozen_first(program: &mut Program, frozen: Frozen<'_>) -> usize {
    assert!(
        program.classes.is_empty() && program.functions.is_empty() && program.variables.is_empty(),
        "the frozen prefix is laid out before the outline"
    );
    let n = program.libraries.len();
    let mut keep: Vec<bool> = (0..n)
        .map(|l| frozen(program, LibraryId(l as u32)))
        .collect();
    loop {
        let mut changed = false;
        for l in 0..n {
            if !keep[l] {
                continue;
            }
            let lib = &program.libraries[l];
            let leaves = lib.imports.iter().map(|i| i.library);
            let leaves = leaves.chain(lib.exports.iter().map(|e| e.library));
            if leaves.into_iter().any(|t| !keep[t.0 as usize]) {
                keep[l] = false;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let order: Vec<usize> = (0..n)
        .filter(|&l| keep[l])
        .chain((0..n).filter(|&l| !keep[l]))
        .collect();
    let mut library_map = vec![LibraryId(0); n];
    for (new, &old) in order.iter().enumerate() {
        library_map[old] = LibraryId(new as u32);
    }
    let units = program.units.len();
    let unit_frozen = |u: usize| keep[program.units[u].library.0 as usize];
    let unit_order: Vec<usize> = (0..units)
        .filter(|&u| unit_frozen(u))
        .chain((0..units).filter(|&u| !unit_frozen(u)))
        .collect();
    let mut unit_map = vec![UnitId(0); units];
    for (new, &old) in unit_order.iter().enumerate() {
        unit_map[old] = UnitId(new as u32);
    }
    let lib = |l: LibraryId| library_map[l.0 as usize];
    let unit = |u: UnitId| unit_map[u.0 as usize];

    let mut old_units: Vec<Option<crate::model::Unit>> = std::mem::take(&mut program.units)
        .into_iter()
        .map(Some)
        .collect();
    program.units = unit_order
        .iter()
        .map(|&u| {
            let mut x = old_units[u].take().expect("each unit once");
            x.library = lib(x.library);
            x
        })
        .collect();
    let mut old_libraries: Vec<Option<crate::model::Library>> =
        std::mem::take(&mut program.libraries)
            .into_iter()
            .map(Some)
            .collect();
    program.libraries = order
        .iter()
        .map(|&l| {
            let mut x = old_libraries[l].take().expect("each library once");
            for u in &mut x.units {
                *u = unit(*u);
            }
            for i in &mut x.imports {
                i.unit = unit(i.unit);
                i.library = lib(i.library);
            }
            for e in &mut x.exports {
                e.unit = unit(e.unit);
                e.library = lib(e.library);
            }
            x.unit_parents = x
                .unit_parents
                .iter()
                .map(|(a, b)| (unit(*a), unit(*b)))
                .collect();
            x
        })
        .collect();
    program.core = program.core.map(lib);
    program.entry = program.entry.map(lib);
    keep.iter().filter(|k| **k).count()
}

#[cfg(test)]
mod tests {
    use crate::model::{Element, Namespace, Program};
    use dartforge_intern::Interner;
    use std::path::{Path, PathBuf};

    /// An element by what it is, not by its id: kind, library and name.
    fn describe(p: &Program, names: &Interner, e: Element) -> String {
        let uri = |l: crate::model::LibraryId| p.libraries[l.0 as usize].uri.clone();
        match e {
            Element::Class(c) => {
                let c = &p.classes[c.0 as usize];
                format!("class {}#{}", uri(c.library), names.resolve(c.name))
            }
            Element::Extension(x) => {
                let x = &p.extensions[x.0 as usize];
                let name = x.name.map(|n| names.resolve(n).to_string());
                format!("extension {}#{name:?}", uri(x.library))
            }
            Element::Typedef(t) => {
                let t = &p.typedefs[t.0 as usize];
                format!("typedef {}#{}", uri(t.library), names.resolve(t.name))
            }
            Element::Function(f) => {
                let f = &p.functions[f.0 as usize];
                format!("function {}#{}", uri(f.library), names.resolve(f.name))
            }
            Element::Variable(v) => {
                let v = &p.variables[v.0 as usize];
                format!("variable {}#{}", uri(v.library), names.resolve(v.name))
            }
            Element::Prefix(l, s) => format!("prefix {}#{}", uri(l), names.resolve(s)),
        }
    }

    fn namespace(p: &Program, names: &Interner, ns: &Namespace) -> Vec<String> {
        let mut v: Vec<String> = ns
            .iter()
            .map(|(s, b)| {
                let side = |e: Option<Element>| e.map(|e| describe(p, names, e));
                format!(
                    "{} = {:?} / {:?}{}",
                    names.resolve(*s),
                    side(b.getter),
                    side(b.setter),
                    if b.ambiguous { " ambiguous" } else { "" }
                )
            })
            .collect();
        v.sort();
        v
    }

    /// The program by URIs and names: libraries with their units, imports,
    /// namespaces, and classes with their supertypes.
    fn dump(p: &Program, names: &Interner) -> Vec<String> {
        let mut out = Vec::new();
        let mut libraries: Vec<usize> = (0..p.libraries.len()).collect();
        libraries.sort_by_key(|&l| p.libraries[l].uri.clone());
        for l in libraries {
            let lib = &p.libraries[l];
            out.push(format!("library {}", lib.uri));
            for u in &lib.units {
                out.push(format!("  unit {}", p.units[u.0 as usize].uri));
            }
            for i in &lib.imports {
                out.push(format!(
                    "  import {}",
                    p.libraries[i.library.0 as usize].uri
                ));
            }
            for e in &lib.exports {
                out.push(format!(
                    "  export {}",
                    p.libraries[e.library.0 as usize].uri
                ));
            }
            for (what, ns) in [
                ("declared", &lib.declared),
                ("exported", &lib.exported),
                ("scope", &lib.scope),
            ] {
                for line in namespace(p, names, ns) {
                    out.push(format!("  {what} {line}"));
                }
            }
        }
        let mut classes: Vec<String> = p
            .classes
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let of = |c: Option<crate::model::ClassId>| {
                    c.map(|c| describe(p, names, Element::Class(c)))
                };
                let interfaces: Vec<_> = c.interface_classes.iter().map(|&i| of(Some(i))).collect();
                format!(
                    "{} extends {:?} implements {interfaces:?}",
                    describe(p, names, Element::Class(crate::model::ClassId(i as u32))),
                    of(c.supertype_class)
                )
            })
            .collect();
        classes.sort();
        out.extend(classes);
        out
    }

    fn frozen_load(entry: &Path, outside: &Path, names: &mut Interner) -> Program {
        let sdk = crate::sdk::SdkLayout::default();
        let outside = outside.to_path_buf();
        let frozen = move |p: &Program, l: crate::model::LibraryId| {
            p.libraries[l.0 as usize].units.iter().all(|u| {
                p.units[u.0 as usize]
                    .path
                    .as_deref()
                    .is_some_and(|f| f.starts_with(&outside))
            })
        };
        crate::load::load_lenient_frozen(entry, &sdk, None, names, None, None, None, &frozen).0
    }

    /// After an API change in the package, the load over the frozen prefix
    /// keeps the prefix's ids and gives the program a full load gives.
    #[test]
    fn a_load_onto_the_frozen_prefix_is_a_full_load() {
        let root = std::env::temp_dir().join(format!("df-frozen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("ext")).unwrap();
        std::fs::create_dir_all(root.join("pkg")).unwrap();
        let root = std::fs::canonicalize(&root).unwrap();
        let ext = root.join("ext");
        let pkg = root.join("pkg");
        std::fs::write(ext.join("e.dart"), "class E {}\nint e = 0;\n").unwrap();
        std::fs::write(
            ext.join("f.dart"),
            "import 'e.dart';\nclass F extends E {}\n",
        )
        .unwrap();
        // A library outside the package that imports one of the package is
        // not frozen (the prefix is closed under imports).
        std::fs::write(
            ext.join("g.dart"),
            "import '../pkg/b.dart';\nclass G extends B {}\n",
        )
        .unwrap();
        let a: PathBuf = pkg.join("a.dart");
        std::fs::write(
            &a,
            "import '../ext/f.dart';\nimport '../ext/g.dart';\nimport 'b.dart';\n\
             class A extends F implements B {}\n",
        )
        .unwrap();
        let b = pkg.join("b.dart");
        std::fs::write(
            &b,
            "export '../ext/e.dart';\nimport '../ext/e.dart';\nclass B extends E { int x = 0; }\n",
        )
        .unwrap();
        let mut names = Interner::new();
        let first = frozen_load(&a, &ext, &mut names);
        let at = first.frozen.expect("a frozen prefix");
        let prefix: Vec<String> = first.libraries[..at.libraries]
            .iter()
            .map(|l| l.uri.clone())
            .collect();
        assert_eq!(prefix.len(), 2, "{prefix:?}");
        assert!(
            prefix
                .iter()
                .all(|u| u.ends_with("e.dart") || u.ends_with("f.dart"))
        );
        let class_e = first
            .classes
            .iter()
            .position(|c| names.resolve(c.name) == "E")
            .unwrap();

        std::fs::write(
            &b,
            "export '../ext/e.dart';\nimport '../ext/e.dart';\n\
             class B extends E { String y = ''; }\nclass C {}\n",
        )
        .unwrap();
        let sdk = crate::sdk::SdkLayout::default();
        let (onto, _) =
            crate::load::load_lenient_onto(first, &a, &sdk, None, &mut names, None, None);
        assert_eq!(onto.frozen, Some(at));
        assert_eq!(names.resolve(onto.classes[class_e].name), "E");
        let fresh = frozen_load(&a, &ext, &mut names);
        assert_eq!(dump(&onto, &names), dump(&fresh, &names));
        assert!(
            dump(&onto, &names)
                .iter()
                .any(|l| l.ends_with("#C extends None implements []") || l.contains("#C extends"))
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
