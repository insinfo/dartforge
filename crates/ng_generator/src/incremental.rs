//! Generation of **one** file, for the build engine (`crates/build`,
//! `docs/BUILD-PEDIDOS-GERADOR-NG.md`): the engine has one action per `.dart`
//! and reruns only that of the edited component. Besides the text, the
//! generation returns what it asked other libraries ([`NgQuery`]) — it is
//! through these queries that the engine knows whom to wake up when a child's
//! `@Input` changes or a selector appears.
//!
//! The output is the same as [`crate::generate_in`]: both go through
//! `generate_internal`.
use crate::resolution::{Designated, Resolution};
use crate::view::Refusal;
use crate::{Findings, NgIndex, Package};
use dartforge_intern::Interner;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// What generating one file produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOutput {
    /// Texto do `<name>.template.dart`.
    pub template: String,
    /// Files read: the `.dart`, the `.html` of `templateUrl`, each `styleUrls`
    /// stylesheet (the `.css`, or the `.scss` it comes from).
    pub inputs: Vec<PathBuf>,
    /// Extra outputs (`<name>.css.shim.dart`), by natural path.
    pub extras: Vec<(PathBuf, String)>,
    /// What the generation asked other libraries, without repetition, in the
    /// order it asked.
    pub queries: Vec<NgQuery>,
}

/// A question the generation asked outside its own file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NgQuery {
    /// A component or directive of `directives:` resolved: the selector and who
    /// declares it.
    Child {
        selector: String,
        library: String,
        class: String,
    },
    /// A type resolved by the semantic database (injection, member type for
    /// `interpolate*`, item of `directives:`/`pipes:`): the library that
    /// declares it and the name.
    Type { library: String, name: String },
    /// A template tag that is not HTML and that no component of `directives:`
    /// declares: if one appears, the generation changes.
    MissingSelector(String),
}

/// Reads and analyzes a `.dart`: the findings the generation and the index use.
pub fn analyze_file(source: &Path, text: &str, names: &mut Interner) -> Findings {
    let _ = source;
    let tokens = dartforge_frontend::lexer::lex(text);
    let parsed = dartforge_frontend::parser::parse_lexed(text, tokens, names);
    crate::find(&parsed.ast, &parsed.unit, text, names)
}

/// Generates the `.template.dart` of `source` — the same text
/// [`crate::generate_in`] would write for it —, or says why not. The refusal
/// carries the [`crate::view::Reason`] (`refusal.reason`) and the sub-shape.
pub fn generate_file(
    package: &Package,
    source: &Path,
    findings: &Findings,
    resolver: Option<&dyn Resolution>,
    names: &mut Interner,
    index: &NgIndex,
) -> Result<FileOutput, Refusal> {
    let name = source
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let recorder = resolver.map(Recorder::new);
    let generated = crate::generate_internal(
        package,
        source,
        &name,
        findings,
        recorder.as_ref().map(|g| g as &dyn Resolution),
        names,
        index,
    );
    let mut queries: Vec<NgQuery> = recorder.map(|g| g.queries.into_inner()).unwrap_or_default();
    queries.extend(queried_children(source, findings, resolver, index));
    let (template, inputs, extras) = generated?;
    let mut unique = Vec::new();
    for c in queries {
        if !unique.contains(&c) {
            unique.push(c);
        }
    }
    Ok(FileOutput {
        template,
        inputs,
        extras,
        queries: unique,
    })
}

/// The children each component of the file resolved, and the tags that found no
/// owner.
fn queried_children(
    source: &Path,
    findings: &Findings,
    resolver: Option<&dyn Resolution>,
    index: &NgIndex,
) -> Vec<NgQuery> {
    let mut output = Vec::new();
    for comp in &findings.components {
        let (used, _) = index.directives_of(comp, source, resolver);
        for u in &used {
            let selector = match (&u.child, &u.directive) {
                (Some(f), _) => f.selector.clone(),
                (None, Some(d)) => d.selector.clone(),
                (None, None) => index
                    .directives
                    .get(&(u.uri.clone(), u.class.clone()))
                    .map(|d| d.selector.clone())
                    .unwrap_or_default(),
            };
            output.push(NgQuery::Child {
                selector,
                library: u.uri.clone(),
                class: u.class.clone(),
            });
        }
        let children = crate::children_by_tag(&used);
        let template = match (&comp.template, &comp.template_url) {
            (Some(t), _) => t.clone(),
            (None, Some(u)) => source
                .parent()
                .and_then(|d| std::fs::read_to_string(d.join(u)).ok())
                .unwrap_or_default(),
            (None, None) => String::new(),
        };
        let mut tags = Vec::new();
        tags_of(&crate::html::parse(&template), &mut tags);
        for t in tags {
            if !crate::dom::tag_html(&t) && !children.contains_key(&t) && t != "ng-container" {
                output.push(NgQuery::MissingSelector(t));
            }
        }
    }
    output
}

fn tags_of(nodes: &[crate::html::Node], output: &mut Vec<String>) {
    for n in nodes {
        if let crate::html::Node::Element(e) = n {
            if !output.contains(&e.name) {
                output.push(e.name.clone());
            }
            tags_of(&e.children, output);
        }
    }
}

/// A [`Resolution`] that answers for the inner one and records each type found.
struct Recorder<'a> {
    inside: &'a dyn Resolution,
    queries: RefCell<Vec<NgQuery>>,
}

impl<'a> Recorder<'a> {
    fn new(inside: &'a dyn Resolution) -> Self {
        Recorder {
            inside,
            queries: RefCell::new(Vec::new()),
        }
    }

    fn record_refusal(&self, library: &str, name: &str) {
        let simple = name.trim_end_matches('?');
        let simple = simple.rsplit('.').next().unwrap_or(simple);
        let c = NgQuery::Type {
            library: library.to_string(),
            name: simple.to_string(),
        };
        let mut v = self.queries.borrow_mut();
        if !v.contains(&c) {
            v.push(c);
        }
    }
}

impl Resolution for Recorder<'_> {
    fn type_uri(&self, file: &Path, name: &str) -> Option<String> {
        let r = self.inside.type_uri(file, name);
        if let Some(uri) = &r {
            self.record_refusal(uri, name);
        }
        r
    }

    fn member_type(&self, file: &Path, ty: &str, member: &str) -> Option<(String, PathBuf)> {
        if let Some(uri) = self.inside.type_uri(file, ty) {
            self.record_refusal(&uri, ty);
        }
        self.inside.member_type(file, ty, member)
    }

    fn method(&self, file: &Path, ty: &str, name: &str) -> Option<crate::resolution::Method> {
        if let Some(uri) = self.inside.type_uri(file, ty) {
            self.record_refusal(&uri, ty);
        }
        self.inside.method(file, ty, name)
    }

    fn type_bounds(&self, file: &Path, ty: &str) -> Option<Vec<bool>> {
        if let Some(uri) = self.inside.type_uri(file, ty) {
            self.record_refusal(&uri, ty);
        }
        self.inside.type_bounds(file, ty)
    }

    fn has_setter(&self, file: &Path, ty: &str, member: &str) -> bool {
        if let Some(uri) = self.inside.type_uri(file, ty) {
            self.record_refusal(&uri, ty);
        }
        self.inside.has_setter(file, ty, member)
    }

    fn member_type_with_free(
        &self,
        file: &Path,
        ty: &str,
        member: &str,
        free: &[String],
    ) -> Option<(String, PathBuf)> {
        if let Some(uri) = self.inside.type_uri(file, ty) {
            self.record_refusal(&uri, ty);
        }
        self.inside.member_type_with_free(file, ty, member, free)
    }

    fn free_method(
        &self,
        file: &Path,
        ty: &str,
        name: &str,
        free: &[String],
    ) -> Option<crate::resolution::Method> {
        if let Some(uri) = self.inside.type_uri(file, ty) {
            self.record_refusal(&uri, ty);
        }
        self.inside.free_method(file, ty, name, free)
    }

    fn final_member(&self, file: &Path, ty: &str, member: &str) -> Option<bool> {
        if let Some(uri) = self.inside.type_uri(file, ty) {
            self.record_refusal(&uri, ty);
        }
        self.inside.final_member(file, ty, member)
    }

    fn designated(&self, file: &Path, name: &str) -> Option<Designated> {
        let r = self.inside.designated(file, name);
        if let Some(Designated::Class { uri }) = &r {
            self.record_refusal(uri, name);
        }
        r
    }

    // The type missing from the scope depends on the libraries the file
    // imports, which the import closure of the invalidation (`NgEstagioA`)
    // already covers; there is no declaring library to record.
    fn nonexistent_type(&self, file: &Path, ty: &str) -> bool {
        self.inside.nonexistent_type(file, ty)
    }

    fn exported(&self, file: &Path, name: &str) -> Option<(String, crate::resolution::Exported)> {
        let r = self.inside.exported(file, name);
        if let Some((uri, _)) = &r {
            self.record_refusal(uri, name);
        }
        r
    }

    fn static_member(
        &self,
        file: &Path,
        ty: &str,
        member: &str,
    ) -> Option<crate::resolution::Static> {
        if let Some(uri) = self.inside.type_uri(file, ty) {
            self.record_refusal(&uri, ty);
        }
        self.inside.static_member(file, ty, member)
    }

    fn output_type(&self, uri: &str, class: &str, member: &str) -> Option<(String, PathBuf)> {
        self.record_refusal(uri, class);
        self.inside.output_type(uri, class, member)
    }
}
