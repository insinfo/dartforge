//! ngdart generator: produces the `.template.dart` files that `build_runner`
//! produces, without `build_runner`.
//!
//! The ngdart compiler turns component + HTML template into imperative Dart
//! (`ViewXComp0.build()`, factories, injection). `build_runner` only
//! orchestrates this — and it is `build_runner`, with `build_web_compilers`,
//! that costs the minute and a half of compiling new_sali. Generating here, the
//! whole cycle is ours.
//!
//! Rule: **do not touch ngdart or the application**. What matters is not
//! producing identical text, but the same ABI — the same public symbols
//! (`XNgFactory`, `createXFactory`, `ViewX0`) with the same semantics. Where
//! the output is equal to the official one, even better: it can be compared
//! byte for byte, and the 284 files that build_runner already wrote in new_sali
//! serve as an oracle, as `dartdevc` does for the emitter.
//!
//! Progress is gradual and measured: what we cannot generate yet is left out
//! and keeps coming from `build_runner`. The application works at every step
//! and the scoreboard says exactly where we are.
pub mod component;
pub mod css;
pub mod csslib;
pub mod dialect;
pub mod directives;
pub mod dom;
mod entities;
pub mod expr;
pub mod html;
pub mod incremental;
mod injector;
pub mod metadata;
pub mod micro;
pub mod resolution;
pub mod sass;
pub mod selector;
pub mod shadow_css;
pub mod view;

use dartforge_elements::generated::{Generation, GenerationBuilder};
use dartforge_frontend::ast;
use dartforge_intern::Interner;
pub use incremental::{FileOutput, NgQuery, analyze_file, generate_file};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use view::{Reason, Refusal, refusal};

/// Header that the official compiler writes in every generated file.
pub const HEADER: &str = "// **************************************************************************\n// Generator: AngularDart Compiler\n// **************************************************************************\n\n";

/// What a library has of interest to the generator.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Findings {
    /// `@Component`s read in full — the view comes from them.
    pub components: Vec<component::Component>,
    /// `@Directive`s, read as the official compiler sees them from their users
    /// (selector, inputs, outputs, constructor).
    pub directives: Vec<component::Component>,
    /// `@Pipe`s, with what their users need to know about them.
    pub pipes: Vec<component::Pipe>,
    /// The top-level variables with `@GenerateInjector`, in source order.
    pub injectors: Vec<String>,
    /// `@Directive`s with `@HostBinding`: each one gets a
    /// `DirectiveChangeDetector` in the generated file.
    pub hosts: Vec<Host>,
    /// `@Directive`s with `@HostBinding`/`@HostListener` that inherit from
    /// something (`extends`, `with`): the official compiler also collects the
    /// inherited members, which cannot be seen from here.
    pub inherited_host: bool,
}

/// A `@Directive` with `@HostBinding`: the official compiler generates the
/// `XNgCd` class for it (`requiresDirectiveChangeDetector`, in
/// `compile_metadata.dart`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Host {
    pub class: String,
    /// `(binding name, member)` of each `@HostBinding` — `class.x`, `attr.x`,
    /// property or, without an argument, the member name —, in the order the
    /// official compiler collects them (`DirectiveVisitor`, in
    /// `angular_compiler/analyzer/view/directive.dart`): accessors, then
    /// methods, then fields — each group in declaration order.
    pub bindings: Vec<(String, String)>,
    /// `style.x`: the member type as written (`String`, `int?`…), which decides
    /// between `toString()`, `?.toString()` and the direct value
    /// (`visitStyleBinding`).
    pub style_types: std::collections::HashMap<String, String>,
    /// Some binding outside what we know how to translate: the reason.
    pub refused: Option<String>,
    /// The `final` members (`isImmutable`): written once, in `if (firstCheck)`,
    /// without an `_expr_N` field.
    pub immutables: std::collections::HashSet<String>,
    /// The static members of the class itself: read through the class
    /// (`StaticRead`), always immutable and `dynamic` (case j134).
    pub statics: std::collections::HashSet<String>,
}

/// Reads the `@HostBinding`s of a `@Directive` class; `None` if there are none.
fn host(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    class: &ast::ClassDecl,
) -> Option<Host> {
    let members = component::member_types(tree, source, interner, class);
    let mut style_types = std::collections::HashMap::new();
    let mut accessors = Vec::new();
    let mut fields = Vec::new();
    let mut immutables = std::collections::HashSet::new();
    let mut refused: Option<String> = None;
    let refuse = |reason: String, refused: &mut Option<String>| {
        refused.get_or_insert(reason);
    };
    let mut some = false;
    for &m in &class.members {
        let member = tree.member(m);
        for a in member.metadata.iter() {
            if annotation_name(a, interner) != "HostBinding" {
                continue;
            }
            some = true;
            // Without an argument (`Some(None)`), the binding name is that of
            // the member itself (`bindingName ?? memberName`).
            let name = match a.arguments.as_ref().map(|args| &args.args[..]) {
                None | Some([]) => Some(None),
                Some([x]) if x.name.is_none() => match &tree.expr(x.value).kind {
                    ast::ExprKind::String(lit) => {
                        lit.constant_value().map(|s| Some(s.to_string_lossy()))
                    }
                    _ => None,
                },
                _ => None,
            };
            let Some(name) = name else {
                refuse(
                    "@HostBinding with a name that is not a string".into(),
                    &mut refused,
                );
                continue;
            };
            // `class.x`, `attr.x`, property and `style.x`
            // ([`view::host_shape`]); `style.x` by the written type of
            // the field (not `final`: the immutable one would be written once),
            // as in the component.
            let accepts = |n: &str, member: &str, field: bool| match view::host_shape(n) {
                Ok(view::HostShape::Style { .. }) => {
                    let ty = members.get(member).map(|m| m.ty.trim()).unwrap_or_default();
                    let base = ty.trim_end_matches('?');
                    field
                        && !base.is_empty()
                        && !matches!(base, "dynamic" | "var" | "Object" | "Never")
                }
                Ok(_) => true,
                Err(_) => false,
            };
            match &member.kind {
                // A `final` field is immutable and is written once, on the
                // first check (`isImmutable`); a static one reads through the
                // class, not yet.
                ast::MemberKind::Field(l) if !l.static_ && l.variables.len() == 1 => {
                    let member = interner.resolve(l.variables[0].name.sym).to_string();
                    let name = name.unwrap_or_else(|| member.clone());
                    let immutable = l.final_ || l.const_;
                    if immutable {
                        immutables.insert(member.clone());
                    }
                    if !accepts(&name, &member, !immutable) {
                        refuse(
                            format!("@HostBinding('{name}') outside the known shapes"),
                            &mut refused,
                        );
                    }
                    if let Some(m) = members.get(&member) {
                        style_types.insert(member.clone(), m.ty.trim().to_string());
                    }
                    fields.push((name, member));
                }
                ast::MemberKind::Method(f) => {
                    let function = tree.function(*f);
                    match (function.kind, function.name) {
                        (ast::FunctionKind::Getter, Some(n)) if !function.static_ => {
                            let member = interner.resolve(n.sym).to_string();
                            let name = name.unwrap_or_else(|| member.clone());
                            if !accepts(&name, &member, false) {
                                refuse(
                                    format!("@HostBinding('{name}') outside the known shapes"),
                                    &mut refused,
                                );
                            }
                            accessors.push((name, member));
                        }
                        _ => refuse(
                            "@HostBinding on a method, setter or static getter".into(),
                            &mut refused,
                        ),
                    }
                }
                // An immutable static outside `class.x`/`style.x`:
                // `hostAttribute`, which does not go into the `XNgCd`
                // (`_computeHostBindingImmutability`).
                ast::MemberKind::Field(l)
                    if l.static_
                        && (l.final_ || l.const_)
                        && !name.as_deref().is_some_and(|n| {
                            n.starts_with("class.") || n.starts_with("style.")
                        }) => {}
                ast::MemberKind::Field(l) if l.static_ => {
                    refuse("@HostBinding on a static field".into(), &mut refused)
                }
                _ => refuse(
                    "@HostBinding on a member outside the shape".into(),
                    &mut refused,
                ),
            }
        }
    }
    accessors.extend(fields);
    // Without `hostProperties` (only `hostAttributes`, or nothing) there is no
    // `XNgCd` (`requiresDirectiveChangeDetector`).
    if !some || (accessors.is_empty() && refused.is_none()) {
        return None;
    }
    // The official map is keyed by binding name: repeating the name overwrites
    // the value and keeps the position of the first.
    let mut seen = std::collections::HashSet::new();
    if !accessors.iter().all(|(c, _)| seen.insert(c.clone())) {
        refuse("@HostBinding with a repeated name".into(), &mut refused);
    }
    // A generic type changes the class declaration (`XNgCd<T>`); not yet.
    if !class.type_params.is_empty() {
        refuse("@HostBinding on a generic directive".into(), &mut refused);
    }
    Some(Host {
        class: interner.resolve(class.name.sym).to_string(),
        bindings: accessors,
        style_types,
        refused,
        immutables,
        statics: Default::default(),
    })
}

impl Findings {
    /// Without any of this the generated file is just the header and the
    /// `import` of itself — 120 of the 284 files of new_sali/frontend.
    pub fn trivial(&self) -> bool {
        self.components.is_empty()
            && self.directives.is_empty()
            && self.pipes.is_empty()
            && self.injectors.is_empty()
    }
}

/// Effective name of an annotation: `@Component`, `@ng.Component` and
/// `@Component.new` all count as `Component`. The library prefix is lowercase
/// by convention and the class uppercase; that is what tells `p.Component` from
/// `Component.new`.
pub(crate) fn annotation_name(a: &ast::Annotation, interner: &Interner) -> String {
    for part in &a.name {
        let text = interner.resolve(part.sym);
        if text.starts_with(char::is_uppercase) {
            return text.to_string();
        }
    }
    a.name
        .first()
        .map(|n| interner.resolve(n.sym).to_string())
        .unwrap_or_default()
}

/// Scans the top-level declarations of an already parsed unit.
pub fn find(
    tree: &ast::Ast,
    unit: &ast::CompilationUnit,
    source: &str,
    interner: &Interner,
) -> Findings {
    let mut findings = Findings::default();
    for &id in &unit.declarations {
        let decl = tree.decl(id);
        // `@Component` and `@Directive` only exist on classes, but
        // `@GenerateInjector` sits on a top-level variable
        // (`@GenerateInjector([...]) final InjectorFactory injector = …`),
        // which is how new_sali sets up injection. That is why the scan does
        // not filter by declaration kind.
        let target = match &decl.kind {
            ast::DeclKind::Class(c) => interner.resolve(c.name.sym).to_string(),
            _ => String::new(),
        };
        for a in decl.metadata.iter() {
            match annotation_name(a, interner).as_str() {
                "Component" => {
                    if let ast::DeclKind::Class(class) = &decl.kind {
                        let mut c = component::read_component(tree, source, interner, class, a);
                        c.detection_link = decl.metadata.iter().any(|m| {
                            m.name.last().map(|n| interner.resolve(n.sym))
                                == Some("changeDetectionLink")
                        });
                        findings.components.push(c);
                    }
                }
                "Directive" => {
                    if let ast::DeclKind::Class(class) = &decl.kind {
                        findings
                            .directives
                            .push(component::read_directive(tree, source, interner, class, a));
                    }
                    // Only `@HostBinding` changes the directive's file;
                    // `@HostListener` goes to its users. But the official
                    // compiler collects both in the superclasses too.
                    if let ast::DeclKind::Class(class) = &decl.kind {
                        let inherits = class.extends.is_some() || !class.with.is_empty();
                        let annotated = class.members.iter().any(|&m| {
                            tree.member(m).metadata.iter().any(|a| {
                                matches!(
                                    annotation_name(a, interner).as_str(),
                                    "HostBinding" | "HostListener"
                                )
                            })
                        });
                        findings.inherited_host |= inherits && annotated;
                        findings.hosts.extend(host(tree, source, interner, class));
                    }
                }
                "Pipe" => {
                    if let ast::DeclKind::Class(class) = &decl.kind {
                        findings
                            .pipes
                            .push(component::read_pipe(tree, source, interner, class, a));
                    }
                }
                "GenerateInjector" => {
                    if let ast::DeclKind::Variables(l) = &decl.kind {
                        findings.injectors.extend(
                            l.variables
                                .iter()
                                .map(|v| interner.resolve(v.name.sym).to_string()),
                        );
                    } else {
                        findings.injectors.push(target.clone());
                    }
                }
                _ => {}
            }
        }
    }
    findings
}

/// Generated file of a library with nothing Angular.
pub fn template_trivial(file_name: &str) -> String {
    format!("{HEADER}import '{file_name}';\n")
}

/// Where we are: how many files the generator already covers and which ones are
/// missing.
#[derive(Debug, Default)]
pub struct Scoreboard {
    /// `.dart` files examined.
    pub examined: usize,
    /// Generated by us.
    pub generated: usize,
    /// Left to whoever knows how to generate them (today, `build_runner`).
    pub pending: Vec<PathBuf>,
    /// How many pending files per reason of the first refusal.
    pub reasons: std::collections::BTreeMap<Reason, usize>,
    /// Complete set of refusals (reason and sub-shape) of each pending file,
    /// collected by the same emission that generates. It is how one knows how
    /// many files a new shape really unlocks.
    pub refusal_sets: Vec<std::collections::BTreeSet<Refusal>>,
    /// How many times each shape not understood appears (`@HostBinding`,
    /// `providers: [..]`…), counting all of those of each pending component.
    pub not_understood: std::collections::BTreeMap<String, usize>,
}

impl Scoreboard {
    pub fn summary(&self) -> String {
        format!(
            "ngdart: {}/{} generated by us",
            self.generated, self.examined
        )
    }
}

/// Path of the generated file next to the source: `foo.dart` ->
/// `foo.template.dart`.
pub fn template_path(source: &Path) -> PathBuf {
    let name = source.file_name().unwrap_or_default().to_string_lossy();
    let base = name.strip_suffix(".dart").unwrap_or(&name);
    source.with_file_name(format!("{base}.template.dart"))
}

/// The package being generated: the name is used for the `asset:` URIs that
/// ngdart uses in development-mode messages.
#[derive(Default)]
pub struct Package {
    pub name: String,
    pub root: PathBuf,
    /// The `.css` stylesheets that `sass_builder` generated in this build (the
    /// engine reads them from its output), by path: the ngdart shim starts from
    /// them, as in the official build — in the style (`outputStyle`) the
    /// project asked for. Without it, the shim compiles the `.scss` next to it
    /// on its own.
    pub generated_sheets: std::collections::HashMap<PathBuf, String>,
}

impl Package {
    /// Path of a file inside the package, with slashes: `lib/src/x/f.dart`.
    fn relative(&self, file: &Path) -> String {
        file.strip_prefix(&self.root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
    }
}

/// Generates what it can for the `.dart` files of `directories`, accumulating
/// into `c`.
pub fn generate_in(
    package: &Package,
    directories: &[PathBuf],
    interner: &mut Interner,
    c: &mut GenerationBuilder,
    scoreboard: &mut Scoreboard,
    program: Option<&resolution::Resolver>,
) {
    let resolver = program.map(|r| r as &dyn resolution::Resolution);
    // Two passes: the first reads and parses everything, the second generates.
    // It is the first that builds the package's component index — without it
    // there is no way to know that `<a02-texto-estatico>` is a component and
    // which class implements it.
    let mut files: Vec<(PathBuf, String, Findings)> = Vec::new();
    for dir in directories {
        let mut stack = vec![dir.clone()];
        while let Some(d) = stack.pop() {
            let Ok(inputs) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in inputs.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let name = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if !name.ends_with(".dart") || name.ends_with(".template.dart") {
                    continue;
                }
                scoreboard.examined += 1;
                let Ok(source) = std::fs::read_to_string(&p) else {
                    scoreboard.pending.push(p);
                    continue;
                };
                let tokens = dartforge_frontend::lexer::lex(&source);
                let parsed = dartforge_frontend::parser::parse_lexed(&source, tokens, interner);
                // A part (`part of`) is not a library: the official compiler
                // generates nothing for it (`resolver.isLibrary`), and what it
                // declares goes into the `.template.dart` of the owning
                // library.
                if is_part(&parsed.unit) {
                    scoreboard.examined -= 1;
                    continue;
                }
                let findings = find(&parsed.ast, &parsed.unit, &source, interner);
                files.push((p, name, findings));
            }
        }
    }

    let index = NgIndex::assemble(package, &files, program);

    for (p, name, findings) in &files {
        let (text, inputs, extras) =
            match generate_internal(package, p, name, findings, resolver, interner, &index) {
                Ok(x) => x,
                Err(first) => {
                    // A shape the generator does not cover yet: it stays with
                    // build_runner, and the application compiles all the same.
                    *scoreboard.reasons.entry(first.reason).or_default() += 1;
                    // Every shape present counts, not only the one that
                    // refused.
                    for c in &findings.components {
                        for r in &c.not_understood {
                            *scoreboard
                                .not_understood
                                .entry(r.shape.clone())
                                .or_default() += 1;
                        }
                    }
                    scoreboard.refusal_sets.push(file_reasons(
                        package, p, findings, first, resolver, &index, interner,
                    ));
                    scoreboard.pending.push(p.clone());
                    continue;
                }
            };
        c.put(template_path(p), text, "ngdart", inputs.clone());
        for (destination, content) in extras {
            c.put(destination, content, "ngdart", inputs.clone());
        }
        scoreboard.generated += 1;
    }

    // The stylesheets, one by one, like the `StylesheetCompiler`: every `.css`
    // of the directories — the written one, the one `sass_builder` generated in
    // this build and, outside the engine, the one of each `.scss` that is not a
    // partial.
    for css in sheets_of(package, directories) {
        let input = if css.is_file() {
            css.clone()
        } else {
            css.with_extension("scss")
        };
        for (destination, r) in generate_sheet(package, &css) {
            match r {
                Ok(content) => c.put(destination, content, "ngdart", vec![input.clone()]),
                Err(refusal) => {
                    *scoreboard.reasons.entry(refusal.reason).or_default() += 1;
                    scoreboard
                        .refusal_sets
                        .push(std::iter::once(refusal).collect());
                    scoreboard.pending.push(destination);
                }
            }
        }
    }
}

/// The `.css` files for which ngdart generates stylesheets, in order: those on
/// disk, those `sass_builder` generated in this build and those of the
/// non-partial `.scss` files without a `.css`.
fn sheets_of(package: &Package, directories: &[PathBuf]) -> Vec<PathBuf> {
    let mut v = std::collections::BTreeSet::new();
    for dir in directories {
        let mut stack = vec![dir.clone()];
        while let Some(d) = stack.pop() {
            let Ok(inputs) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in inputs.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let name = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if name.ends_with(".css") {
                    v.insert(p);
                } else if name.ends_with(".scss") && !name.starts_with('_') {
                    v.insert(p.with_extension("css"));
                }
            }
        }
        v.extend(
            package
                .generated_sheets
                .keys()
                .filter(|k| k.starts_with(dir))
                .cloned(),
        );
    }
    v.into_iter().collect()
}

/// The unit is a part (`part of`), not a library.
pub fn is_part(unit: &dartforge_frontend::ast::CompilationUnit) -> bool {
    unit.directives.iter().any(|d| {
        matches!(
            d.kind,
            dartforge_frontend::ast::DirectiveKind::PartOf { .. }
        )
    })
}

/// The components and directives of the package, by library and class.
///
/// The official compiler asks the `build` asset graph for this; here it comes
/// from the first pass. It is what allows matching `<a02-texto-estatico>` with
/// the class that declares that selector and emitting the child view — and
/// knowing that a `<form>` gets `NgForm`.
///
/// It is incremental: [`NgIndex::update`] and [`NgIndex::remove`] replace
/// what one file put in, without rereading the rest — the build engine keeps it
/// alive during the session.
#[derive(Default)]
pub struct NgIndex {
    by_class: std::collections::HashMap<(String, String), view::Child>,
    /// `@Directive` by (library URI, class): the selector and what the emitter
    /// needs to instantiate it.
    directives: std::collections::HashMap<(String, String), component::Component>,
    /// `@Pipe` by (library URI, class).
    pipes: std::collections::HashMap<(String, String), component::Pipe>,
    /// The metadata of each `@Directive`, read from the program
    /// (`metadata.rs`): it is through them that the emitter instantiates the
    /// directive on a node. Without a program, empty — and every matched
    /// directive is refused.
    metadata: std::collections::HashMap<(String, String), std::sync::Arc<directives::Directive>>,
    /// The `@GenerateInjector`s by (library URI, variable), read from the
    /// program (`metadata::read_injectors`), or the reason they could not be
    /// read.
    injectors: std::collections::HashMap<
        (String, String),
        Result<std::sync::Arc<metadata::Injector>, String>,
    >,
    /// The keys each file put in, for `remove`/`update`.
    by_file: std::collections::HashMap<PathBuf, Vec<(String, String)>>,
}

impl NgIndex {
    /// An empty index.
    pub fn new() -> NgIndex {
        NgIndex::default()
    }

    fn assemble(
        package: &Package,
        files: &[(PathBuf, String, Findings)],
        program: Option<&resolution::Resolver>,
    ) -> Self {
        // With the program loaded, the index covers all packages: an
        // `<li-select>` from limitless_ui is as much a component as one of the
        // project itself. Without it, only what the file scan saw.
        let mut index = match program {
            Some(r) => NgIndex::from_program(r),
            None => NgIndex::new(),
        };
        for (path, _name, findings) in files {
            let Some(uri) = library_uri(package, path) else {
                continue;
            };
            index.merge_unique(&uri, Some(path), findings, program, false);
        }
        index
    }

    /// The index of all libraries loaded in the program (dependencies
    /// included), each one registered by its file: afterwards,
    /// [`NgIndex::update`] replaces only what changes.
    pub fn from_program(r: &resolution::Resolver) -> NgIndex {
        let mut index = NgIndex::new();
        let interner = r.interner();
        for (uri, tree, unit, source, path) in r.libraries() {
            let findings = find(tree, unit, source, interner);
            index.merge_unique(&uri, path, &findings, Some(r), true);
        }
        index
    }

    /// The findings of `file` go into the index, replacing the previous
    /// ones. With the program, the directive metadata is also reread from it
    /// (and the components' injection resolved); without it, the components
    /// keep unresolved injection and the directives no metadata — which refuses
    /// whoever uses them.
    pub fn update(
        &mut self,
        package: &Package,
        file: &Path,
        findings: &Findings,
        program: Option<&resolution::Resolver>,
    ) {
        self.remove(file);
        let Some(uri) = library_uri(package, file) else {
            return;
        };
        self.merge_unique(&uri, Some(file), findings, program, true);
    }

    /// Removes from the index everything `file` had put in.
    pub fn remove(&mut self, file: &Path) {
        let Some(keys) = self
            .by_file
            .remove(&dartforge_elements::generated::key(file))
        else {
            return;
        };
        for k in keys {
            self.by_class.remove(&k);
            self.directives.remove(&k);
            self.pipes.remove(&k);
            self.metadata.remove(&k);
            self.injectors.remove(&k);
        }
    }

    /// The components `comp` can use in its template, in the order of the
    /// expanded `directives:` (the directives are left out).
    pub fn children_of(
        &self,
        comp: &component::Component,
        source: &Path,
        resolver: Option<&dyn resolution::Resolution>,
    ) -> Vec<view::Child> {
        self.directives_of(comp, source, resolver)
            .0
            .into_iter()
            .filter_map(|u| u.child)
            .collect()
    }

    /// Who declares exactly this selector: (library, class). With more than
    /// one, the one with the smallest key — the answer does not depend on the
    /// map order.
    pub fn declarer(&self, selector: &str) -> Option<(String, String)> {
        self.by_class
            .iter()
            .filter(|(_, f)| f.selector == selector)
            .map(|(k, _)| k)
            .chain(
                self.directives
                    .iter()
                    .filter(|(_, d)| d.selector == selector)
                    .map(|(k, _)| k),
            )
            .min()
            .cloned()
    }

    /// Adds the findings of a library. `replaces`: directives and pipes
    /// replace whatever is there (otherwise, what is already there stays — the
    /// file pass does not overwrite what the program read).
    fn merge_unique(
        &mut self,
        uri: &str,
        path: Option<&Path>,
        findings: &Findings,
        program: Option<&resolution::Resolver>,
        replaces: bool,
    ) {
        let mut keys = Vec::new();
        for comp in &findings.components {
            if comp.selector.is_empty() {
                continue;
            }
            index_findings(
                &mut self.by_class,
                uri,
                comp,
                path,
                program.map(|r| r as &dyn resolution::Resolution),
            );
            let k = (uri.to_string(), comp.class.clone());
            // The component's metadata: with it, its `providers:`
            // (`ExistingProvider`, like those of directives) go into the node
            // of whoever uses it, and "child with providers" stops being a
            // refusal.
            if let Some(r) = program
                && let Some(m) = r
                    .class_by_uri(uri, &comp.class)
                    .and_then(|id| crate::metadata::read_directive(r, id))
                && let Some(f) = self.by_class.get_mut(&k)
            {
                // The template node writes the child's `providers:` as the host
                // view does; what is not emitted yet (a dependency from outside
                // the node, a provider requested on the node itself) is refused
                // by the view, with the reason.
                if crate::view::writable_providers(&m).is_ok() {
                    f.pending.retain(|p| p.shape != "child with providers");
                }
                // The constructor parameters from the metadata (`@Inject`,
                // `@Attribute`, opaque tokens): they replace the ones read from
                // the text and the refusal they caused.
                let text_refusal = comp
                    .params
                    .iter()
                    .find_map(|p| injected(p, path, Some(r as &dyn resolution::Resolution)).err());
                match metadata_params(comp, &m) {
                    Ok(ps) => {
                        if let Some(shape) = text_refusal {
                            f.pending.retain(|p| p.shape != shape);
                        }
                        f.params = ps;
                    }
                    // The metadata says better what is missing: its reason
                    // replaces that of the text.
                    Err(reason) => {
                        if let Some(shape) = text_refusal {
                            for p in f.pending.iter_mut().filter(|p| p.shape == shape) {
                                p.shape = reason.to_string();
                            }
                        }
                    }
                }
                // The inheriting child: `@Input`, `@Output`, `@HostBinding` and
                // hooks from the metadata, which walks up the supertypes like
                // the official `_collectInheritableMetadata`. The inherited
                // content query is not written yet.
                // The hooks always come from the program (indirect interface:
                // `implements X`, `X extends OnInit`).
                if m.unsupported.is_empty() {
                    f.hooks = m.hooks;
                }
                if comp.inherits && m.unsupported.is_empty() {
                    f.inputs = m
                        .inputs
                        .iter()
                        .map(|e| component::Input {
                            name: e.name.clone(),
                            field: e.member.clone(),
                        })
                        .collect();
                    f.outputs = m.outputs.clone();
                    f.hooks = m.hooks;
                    f.host = m.host_bindings.iter().any(|(_, member)| {
                        !comp
                            .host_bindings
                            .iter()
                            .any(|l| l.is_static && l.member == *member)
                    });
                    if m.content_queries.is_empty() {
                        f.pending
                            .retain(|p| p.shape != "child that inherits (extends/with)");
                    }
                }
                f.metadata = Some(std::sync::Arc::new(m));
            }
            keys.push(k);
        }
        for d in &findings.directives {
            let k = (uri.to_string(), d.class.clone());
            if let Some(r) = program
                && let Some(m) = r
                    .class_by_uri(uri, &d.class)
                    .and_then(|id| crate::metadata::read_directive(r, id))
            {
                self.metadata.insert(k.clone(), std::sync::Arc::new(m));
            }
            if replaces {
                self.directives.insert(k.clone(), d.clone());
            } else {
                self.directives
                    .entry(k.clone())
                    .or_insert_with(|| d.clone());
            }
            keys.push(k);
        }
        if !findings.injectors.is_empty()
            && let Some(r) = program
        {
            for (name, read) in crate::metadata::read_injectors(r, uri) {
                let k = (uri.to_string(), name);
                self.injectors
                    .insert(k.clone(), read.map(std::sync::Arc::new));
                keys.push(k);
            }
        }
        for p in &findings.pipes {
            let k = (uri.to_string(), p.class.clone());
            if replaces {
                self.pipes.insert(k.clone(), p.clone());
            } else {
                self.pipes.entry(k.clone()).or_insert_with(|| p.clone());
            }
            keys.push(k);
        }
        if let Some(c) = path {
            self.by_file
                .entry(dartforge_elements::generated::key(c))
                .or_default()
                .extend(keys);
        }
    }
    /// The pipes this component uses, in the order of `pipes:` expanded
    /// depth-first (the constant lists, like `commonPipes`, through the
    /// semantic database). The official compiler looks the name up from the end
    /// to the start (`_findPipeMeta`), so the order matters. A class that is
    /// not a pipe is left out; a name that does not resolve comes back as a
    /// refusal.
    fn pipes_of(
        &self,
        comp: &component::Component,
        file: &Path,
        resolver: Option<&dyn resolution::Resolution>,
    ) -> (Vec<view::UsedPipe>, Vec<Refusal>) {
        let mut output = Vec::new();
        let mut unsupported = Vec::new();
        if comp.unreadable_pipes {
            unsupported.push(refusal(
                Reason::UsedPipes,
                "pipes: with an item that is not a name",
            ));
        }
        let mut stack: Vec<(String, PathBuf, u32)> = comp
            .pipes
            .iter()
            .rev()
            .map(|n| (n.clone(), file.to_path_buf(), 0))
            .collect();
        while let Some((name, scope, depth)) = stack.pop() {
            if depth > 32 {
                unsupported.push(refusal(Reason::UsedPipes, "pipes: circular list"));
                break;
            }
            let simple = name.rsplit('.').next().unwrap_or(&name).to_string();
            match resolver.and_then(|r| r.designated(&scope, &name)) {
                Some(resolution::Designated::Class { uri }) => {
                    if let Some(p) = self.pipes.get(&(uri.clone(), simple)) {
                        output.push(view::UsedPipe {
                            uri,
                            pipe: p.clone(),
                        });
                    }
                }
                Some(resolution::Designated::List { items, scope }) => {
                    for item in items.iter().rev() {
                        stack.push((item.clone(), scope.clone(), depth + 1));
                    }
                }
                None => unsupported.push(refusal(Reason::UsedPipes, "pipes: unresolved name")),
            }
        }
        (output, unsupported)
    }

    /// The directives and components this component uses, in the official
    /// order: `directives:` expanded depth-first (the constant lists through
    /// the semantic database), without repetition (`removeDuplicates`). A class
    /// that is neither a directive nor a component is ignored, as in the
    /// official compiler (`typeDeclarationOf(value)?.accept(...)` returns
    /// `null`). What cannot be resolved comes back as a refusal: without the
    /// whole list one cannot know what matches each element.
    fn directives_of(
        &self,
        comp: &component::Component,
        file: &Path,
        resolver: Option<&dyn resolution::Resolution>,
    ) -> (Vec<view::UsedDirective>, Vec<Refusal>) {
        let mut output = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut unsupported = Vec::new();
        if comp.unreadable_directive_types {
            unsupported.push(refusal(
                Reason::NotUnderstood,
                "directiveTypes: in an unreadable shape",
            ));
        }
        if comp.unreadable_directives {
            unsupported.push(refusal(
                Reason::DirectiveBySelector,
                "directives: with an item that is not a name",
            ));
        }
        for name in &comp.directives {
            self.expand(
                name,
                file,
                resolver,
                &mut output,
                &mut seen,
                &mut unsupported,
                0,
            );
        }
        (output, unsupported)
    }

    #[allow(clippy::too_many_arguments)]
    fn expand(
        &self,
        name: &str,
        scope: &Path,
        resolver: Option<&dyn resolution::Resolution>,
        output: &mut Vec<view::UsedDirective>,
        seen: &mut std::collections::HashSet<(String, String)>,
        unsupported: &mut Vec<Refusal>,
        depth: u32,
    ) {
        if depth > 32 {
            unsupported.push(refusal(
                Reason::DirectiveBySelector,
                "directives: circular list",
            ));
            return;
        }
        let simple = name.rsplit('.').next().unwrap_or(name).to_string();
        match resolver.and_then(|r| r.designated(scope, name)) {
            Some(resolution::Designated::Class { uri }) => {
                let key = (uri.clone(), simple.clone());
                if !seen.insert(key.clone()) {
                    return;
                }
                if let Some(f) = self.by_class.get(&key) {
                    output.push(view::UsedDirective {
                        class: simple,
                        uri,
                        selectors: selector::Selector::parse(&f.selector),
                        child: Some(f.clone()),
                        directive: None,
                    });
                } else if let Some(d) = self.directives.get(&key) {
                    output.push(view::UsedDirective {
                        class: simple,
                        uri,
                        selectors: selector::Selector::parse(&d.selector),
                        child: None,
                        directive: self.metadata.get(&key).cloned(),
                    });
                }
            }
            Some(resolution::Designated::List { items, scope }) => {
                for item in &items {
                    self.expand(item, &scope, resolver, output, seen, unsupported, depth + 1);
                }
            }
            None => {
                // No answer from the semantic database: only a component with a
                // unique name in the package (the earlier path, without a
                // program).
                let by_name: Vec<&view::Child> = self
                    .by_class
                    .values()
                    .filter(|f| f.class == simple)
                    .collect();
                match by_name.as_slice() {
                    [f] if !name.contains('.') => {
                        let key = (f.uri_dart.clone(), f.class.clone());
                        if seen.insert(key) {
                            output.push(view::UsedDirective {
                                class: f.class.clone(),
                                uri: f.uri_dart.clone(),
                                selectors: selector::Selector::parse(&f.selector),
                                child: Some((*f).clone()),
                                directive: None,
                            });
                        }
                    }
                    _ => unsupported.push(refusal(
                        Reason::DirectiveBySelector,
                        "directives: unresolved name",
                    )),
                }
            }
        }
    }
}

/// The components the emitter matches by tag: those whose selector is only an
/// element name. The rest of the list only serves the directive guard.
fn children_by_tag(used: &[view::UsedDirective]) -> std::collections::HashMap<String, view::Child> {
    let mut output = std::collections::HashMap::new();
    // Compound selector (`page-header-comp,pg-header`): each alternative that
    // is just the tag matches the element with that tag. The other alternatives
    // are left to the directive guard.
    for u in used {
        let Some(f) = &u.child else {
            continue;
        };
        for s in &u.selectors {
            if let Some(tag) = s.tag_only() {
                output.entry(tag.to_string()).or_insert_with(|| f.clone());
            }
        }
    }
    output
}

/// The constructor parameters of a child from the metadata read from the
/// program (`_getCompileDiDependencyMetadata`: the named ones are left out;
/// `@Inject(token)` and `@Attribute('name')` resolved), as the official
/// compiler resolves them on its node. A generic type without `@Inject` and the
/// annotated element built-ins are not translated yet.
fn metadata_params(
    comp: &component::Component,
    meta: &directives::Directive,
) -> Result<Vec<view::Injected>, String> {
    use directives::Token;
    if let Some(f) = meta.unsupported.first() {
        return Err(format!("incomplete child metadata: {f}"));
    }
    let textual: Vec<&component::Parameter> = comp.params.iter().filter(|p| !p.named).collect();
    if textual.len() != meta.dependencies.len() {
        return Err("child parameters without the metadata".into());
    }
    let mut output = Vec::new();
    for d in &meta.dependencies {
        if let Some(name) = &d.attribute {
            output.push(view::Injected::Attribute(name.clone()));
            continue;
        }
        let annotated = d.optional || d.self_only || d.host || d.skip_self;
        let builtin = match &d.token {
            Token::Element | Token::Detector => true,
            Token::Class { uri, class } => ngdart_builtin(uri, class),
            _ => false,
        };
        if builtin && annotated {
            return Err("annotated element built-in in the child constructor".into());
        }
        output.push(match &d.token {
            Token::Element => view::Injected::Element,
            Token::Detector => view::Injected::Detector,
            Token::Class { uri, class } if ngdart_builtin(uri, class) => match class.as_str() {
                "ViewContainerRef" | "ComponentLoader" => view::Injected::Container,
                _ => return Err("ngdart token in the child constructor".into()),
            },
            Token::Class { uri, .. } if uri.starts_with("dart:") => {
                return Err("SDK type in the child constructor".into());
            }
            t => view::Injected::Service {
                token: t.clone(),
                optional: d.optional,
                self_only: d.self_only,
                host: d.host,
                skip_self: d.skip_self,
            },
        });
    }
    Ok(output)
}

/// The node built-ins that the official `_getLocalDependency` returns without
/// looking for a provider (`ElementRef`, `ChangeDetectorRef`, `NgContentRef`,
/// `TemplateRef`, `ViewContainerRef`, `ComponentLoader`, `Injector`); another
/// ngdart class (`NgZone`, …) is a service like any other.
fn ngdart_builtin(uri: &str, class: &str) -> bool {
    uri.starts_with("package:ngdart/")
        && matches!(
            class,
            "ElementRef"
                | "ChangeDetectorRef"
                | "NgContentRef"
                | "TemplateRef"
                | "ViewContainerRef"
                | "ComponentLoader"
                | "Injector"
        )
}

/// How the official compiler resolves a constructor parameter of a child on its
/// node: the node, the child's view or a service from outside the view. The
/// rest (another ngdart token, `@Inject`, `@Self`, generic, named) not yet.
fn injected(
    p: &component::Parameter,
    path: Option<&Path>,
    resolver: Option<&dyn resolution::Resolution>,
) -> Result<view::Injected, &'static str> {
    let ty =
        p.ty.as_deref()
            .ok_or("untyped parameter in the child constructor")?;
    if p.named {
        return Err("named parameter in the child constructor");
    }
    if p.other_annotation {
        return Err("@Inject/@Attribute in the child constructor");
    }
    let ty = view::token_ty(ty);
    let uri = match (path, resolver) {
        (Some(c), Some(r)) => r.type_uri(c, ty),
        _ => None,
    }
    .ok_or("unresolved type injected into the child")?;
    let simple = ty.rsplit('.').next().unwrap_or(ty).to_string();
    // The element built-ins (the node, the view, the `ViewContainer`) with
    // `@Self`/`@Host`/`@SkipSelf`/`@Optional`: no case yet.
    let builtin =
        crate::dialect::embedded_node(&uri, &simple).is_some() || ngdart_builtin(&uri, &simple);
    if builtin && p.annotated {
        return Err("annotated element built-in in the child constructor");
    }
    if crate::dialect::embedded_node(&uri, &simple).is_some() {
        return Ok(view::Injected::Element);
    }
    if ngdart_builtin(&uri, &simple) {
        return match simple.as_str() {
            "ChangeDetectorRef" if !p.optional => Ok(view::Injected::Detector),
            // The node gets a `ViewContainer` (`requiresViewContainer`), which
            // the child receives (case j47).
            "ViewContainerRef" | "ComponentLoader" => Ok(view::Injected::Container),
            _ => Err("ngdart token in the child constructor"),
        };
    }
    if uri.starts_with("dart:") {
        return Err("SDK type in the child constructor");
    }
    Ok(view::Injected::Service {
        token: directives::Token::Class { uri, class: simple },
        optional: p.optional,
        self_only: p.self_only,
        host: p.host,
        skip_self: p.skip_self,
    })
}

/// Puts a component in the index, with what the emitter needs from it.
fn index_findings(
    by_class: &mut std::collections::HashMap<(String, String), view::Child>,
    uri: &str,
    comp: &component::Component,
    path: Option<&Path>,
    resolver: Option<&dyn resolution::Resolution>,
) {
    if comp.selector.is_empty() {
        return;
    }
    // If the child projects content, its user calls `createAndProject`.
    let nodes = match (&comp.template, &comp.template_url) {
        (Some(t), _) => Some(html::parse(t)),
        (None, Some(u)) => path
            .and_then(|c| c.parent().map(|d| d.join(u)))
            .and_then(|c| std::fs::read_to_string(c).ok())
            .map(|t| html::parse(&t)),
        (None, None) => None,
    };
    // `ngContentSelectors`: the `select` of each `<ng-content>`, `*` without
    // it.
    let projections: Vec<String> = nodes
        .as_deref()
        .map(html::projections)
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.unwrap_or_else(|| "*".to_string()))
        .collect();
    // What in the child changes the code of its user and is not written yet.
    let on_child = |f: &str| refusal(Reason::BindingOnChild, f);
    let mut pending = Vec::new();
    let mut params = Vec::new();
    for p in &comp.params {
        match injected(p, path, resolver) {
            Ok(x) => params.push(x),
            Err(f) => {
                pending.push(on_child(f));
                break;
            }
        }
    }
    // The content queries, with the target resolved in the child's file. An
    // ngdart type (`TemplateRef`, core directives) would match what the emitter
    // puts in the content (`*ngIf`): not yet.
    let mut queries = Vec::new();
    match &comp.content_queries {
        None => pending.push(on_child("child with @ContentChild outside the shape")),
        Some(found_reads) => {
            for q in found_reads {
                // `read:` changes the value read: the node
                // (`HtmlElement`/`Element`) or another provider of the node, of
                // a package class. The rest (`ViewContainerRef`, `TemplateRef`,
                // tokens) not yet.
                let read = match &q.read {
                    None => None,
                    Some(name) => {
                        let uri = match (path, resolver) {
                            (Some(c), Some(r)) => r.type_uri(c, name),
                            _ => None,
                        };
                        let simple = name.rsplit('.').next().unwrap_or(name);
                        match uri {
                            Some(u) if crate::dialect::embedded_node(&u, simple).is_some() => {
                                Some(view::QueryRead::Element)
                            }
                            Some(u)
                                if !u.starts_with("package:ngdart/") && !u.starts_with("dart:") =>
                            {
                                Some(view::QueryRead::Class(u, simple.to_string()))
                            }
                            _ => {
                                pending.push(on_child("child with @ContentChild(.., read:)"));
                                break;
                            }
                        }
                    }
                };
                if q.reference {
                    queries.push(view::ChildQuery {
                        field: q.field.clone(),
                        list: q.list,
                        target: view::QueryTarget::Reference(q.target.clone()),
                        descendants: q.descendants,
                        read,
                    });
                    continue;
                }
                let uri = match (path, resolver) {
                    (Some(c), Some(r)) => r.type_uri(c, &q.target),
                    _ => None,
                };
                match uri {
                    Some(u) if !u.starts_with("package:ngdart/") && !u.starts_with("dart:") => {
                        let simple = q.target.rsplit('.').next().unwrap_or(&q.target);
                        queries.push(view::ChildQuery {
                            field: q.field.clone(),
                            list: q.list,
                            target: view::QueryTarget::Class(u, simple.to_string()),
                            descendants: q.descendants,
                            read,
                        });
                    }
                    _ => {
                        pending.push(on_child("child with @ContentChild of an unresolved type"));
                        break;
                    }
                }
            }
        }
    }
    if comp.with_providers {
        pending.push(on_child("child with providers"));
    }
    // Inherited hooks, `@Input` and `@Output` cannot be seen from here (the
    // official compiler collects them through the supertypes).
    if comp.inherits {
        pending.push(on_child("child that inherits (extends/with)"));
    }
    if comp.template.is_none() && comp.template_url.is_some() && nodes.is_none() {
        pending.push(on_child("child template not found"));
    }
    by_class.insert(
        (uri.to_string(), comp.class.clone()),
        view::Child {
            class: comp.class.clone(),
            selector: comp.selector.clone(),
            uri_dart: uri.to_string(),
            uri_template: uri.replace(".dart", ".template.dart"),
            projections,
            inputs: comp.inputs.clone(),
            hooks: comp.hooks,
            on_push: comp.on_push,
            detection_link: comp.detection_link,
            // The static one goes into the constructor of the child's view:
            // without another one, the child has no `detectHostChanges` (case
            // j101).
            host: comp.binds_host
                && (comp.host_bindings.is_empty()
                    || comp.host_bindings.iter().any(|l| !l.is_static)),
            outputs: comp.outputs.clone(),
            host_attributes: comp
                .host_bindings
                .iter()
                .filter(|l| l.is_static && l.immutable)
                .filter(|l| !l.name.starts_with("class.") && !l.name.starts_with("style."))
                .map(|l| {
                    let name = l.name.strip_prefix("attr.").unwrap_or(&l.name);
                    (name.to_string(), l.member.clone())
                })
                .collect(),
            params,
            queries,
            pending,
            metadata: None,
        },
    );
}

/// Package name, read from the root `pubspec.yaml`.
///
/// It is the authoritative source: the folder name does not work
/// (`limitless_ui/example` declares `name: limitless_ui_example`) and comparing
/// the `rootUri` of `package_config.json` with the root fails because of
/// trailing slashes and canonicalization.
pub fn read_package_name(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("pubspec.yaml")).ok()?;
    for line in text.lines() {
        // `name:` at the top level, without indentation.
        let Some(value) = line.strip_prefix("name:") else {
            continue;
        };
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let name = value.trim().trim_matches(['\'', '"']).trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

/// `package:` URI of a file of the package, when it is in `lib/`.
fn library_uri(package: &Package, path: &Path) -> Option<String> {
    let rel = package.relative(path);
    let inside = rel.strip_prefix("lib/")?;
    // In canonical form, that of the index (`package:ngx_forms/` → `ngforms`).
    Some(dialect::canonical_string(format!(
        "package:{}/{inside}",
        package.name
    )))
}

/// What comes out of a file: the `.template.dart`, the files that feed it and
/// those generated separately (the `.css.shim.dart` of each stylesheet).
pub(crate) type Generated = (String, Vec<PathBuf>, Vec<(PathBuf, String)>);

/// The directives of the file that get an `XNgCd`, in source order: the
/// bindings are those of the metadata read from the program (the official
/// `hostProperties`, supertypes first), including inherited ones — and a
/// directive that only inherits bindings gets one too (case j84). For an
/// inherited member, the immutability (a `final` field would be written once)
/// and, for `style.x`, the type are asked of the resolver. Without the
/// metadata, only the bindings of the class itself, and an inheriting directive
/// is refused.
fn effective_hosts(
    package: &Package,
    source: &Path,
    findings: &Findings,
    resolver: Option<&dyn resolution::Resolution>,
    index: &NgIndex,
) -> Result<Vec<Host>, Refusal> {
    let uri = library_uri(package, source);
    let metadata = |class: &str| {
        uri.as_ref()
            .and_then(|u| index.metadata.get(&(u.clone(), class.to_string())))
            .filter(|m| m.unsupported.is_empty())
    };
    let has_all = findings
        .directives
        .iter()
        .all(|d| metadata(&d.class).is_some());
    if !has_all {
        if findings.inherited_host {
            return Err(refusal(
                Reason::HostBindingOnDirective,
                "@HostBinding/@HostListener on an inheriting directive",
            ));
        }
        return Ok(findings.hosts.clone());
    }
    let mut output = Vec::new();
    for d in &findings.directives {
        let Some(m) = metadata(&d.class) else {
            continue;
        };
        if m.host_bindings.is_empty() {
            continue;
        }
        let statics: std::collections::HashSet<String> = m.host_statics.iter().cloned().collect();
        let own = findings.hosts.iter().find(|h| h.class == d.class);
        if let Some(h) = own
            && h.bindings == m.host_bindings
            && statics.is_empty()
        {
            output.push(h.clone());
            continue;
        }
        let mut h = Host {
            class: d.class.clone(),
            bindings: m.host_bindings.clone(),
            style_types: std::collections::HashMap::new(),
            refused: None,
            immutables: std::collections::HashSet::new(),
            statics: statics.clone(),
        };
        for (name, member) in &m.host_bindings {
            let shape = view::host_shape(name);
            if statics.contains(member) {
                if let Err(f) = shape {
                    h.refused.get_or_insert(f);
                }
                continue;
            }
            let final_ = resolver.and_then(|r| r.final_member(source, &d.class, member));
            if final_ == Some(true) {
                h.immutables.insert(member.clone());
            }
            match shape {
                Err(f) => {
                    h.refused.get_or_insert(f);
                }
                Ok(_) if final_.is_none() => {
                    h.refused.get_or_insert(format!(
                        "inherited @HostBinding('{name}') without a readable declaration"
                    ));
                }
                Ok(view::HostShape::Style { .. }) if final_ == Some(true) => {
                    h.refused
                        .get_or_insert(format!("@HostBinding('{name}') on a final field"));
                }
                Ok(view::HostShape::Style { .. }) => {
                    // The written type, from the program (which walks up to the
                    // class that declares the member), otherwise that of the
                    // class itself.
                    let ty = resolver
                        .and_then(|r| r.member_type(source, &d.class, member))
                        .map(|(t, _)| t)
                        .or_else(|| own.and_then(|x| x.style_types.get(member).cloned()));
                    let base = ty.as_deref().map(|t| t.trim().trim_end_matches('?'));
                    match (ty.clone(), base) {
                        (Some(t), Some(b))
                            if !b.is_empty()
                                && !matches!(b, "dynamic" | "var" | "Object" | "Never") =>
                        {
                            h.style_types.insert(member.clone(), t);
                        }
                        _ => {
                            h.refused
                                .get_or_insert(format!("@HostBinding('{name}') of unknown type"));
                        }
                    }
                }
                Ok(_) => {}
            }
        }
        output.push(h);
    }
    Ok(output)
}

/// Contents of the `.template.dart` of a file, when we know how to generate it,
/// with the files that feed it (the `.dart` and the template's `.html`).
pub(crate) fn generate_internal(
    package: &Package,
    source: &Path,
    file_name: &str,
    findings: &Findings,
    resolver: Option<&dyn resolution::Resolution>,
    names: &mut Interner,
    index: &NgIndex,
) -> Result<Generated, Refusal> {
    // An annotation of the file with a name that does not resolve is not
    // constant: the official compiler fails to compile it ("Arguments of a
    // constant creation must be constant expressions") and generates nothing.
    if let Some(uri) = library_uri(package, source) {
        let metadata = findings
            .directives
            .iter()
            .filter_map(|d| index.metadata.get(&(uri.clone(), d.class.clone())).cloned())
            .chain(findings.components.iter().filter_map(|c| {
                index
                    .by_class
                    .get(&(uri.clone(), c.class.clone()))
                    .and_then(|f| f.metadata.clone())
            }));
        for m in metadata {
            if let Some(f) = m.unsupported.iter().find(|f| f.contains("unresolved name")) {
                return Err(refusal(
                    Reason::NotUnderstood,
                    format!(
                        "annotation of {} with an unresolved name ({f}): the official compiler fails",
                        m.class
                    ),
                ));
            }
        }
    }
    // The same holds for whoever lists that class in `directives:`: the
    // official compiler reads the metadata of each directive used when
    // compiling the component, and its failure is the component's (the
    // `material_date_time_picker` with the `MaterialInputComponent` of a
    // nonexistent `NG_VALIDATORS`).
    for comp in &findings.components {
        let (used, _) = index.directives_of(comp, source, resolver);
        for u in &used {
            let m = u
                .child
                .as_ref()
                .and_then(|f| f.metadata.clone())
                .or_else(|| u.directive.clone());
            if let Some(m) = m
                && let Some(f) = m.unsupported.iter().find(|f| f.contains("unresolved name"))
            {
                return Err(refusal(
                    Reason::NotUnderstood,
                    format!(
                        "directives: {} has an annotation with an unresolved name ({f}): the official compiler fails",
                        m.class
                    ),
                ));
            }
        }
    }
    if findings.injectors.is_empty() {
        return generate_views(package, source, file_name, findings, resolver, names, index);
    }
    // `buildGeneratedCode`: the injectors' imports right after the import of
    // the file itself, before those of the views; their code at the end.
    let uri = library_uri(package, source)
        .ok_or_else(|| refusal(Reason::Injector, "@GenerateInjector outside a package"))?;
    let mut injectors = Vec::new();
    for name in &findings.injectors {
        match index.injectors.get(&(uri.clone(), name.clone())) {
            Some(Ok(i)) => injectors.push(i.clone()),
            Some(Err(f)) => return Err(refusal(Reason::Injector, f.clone())),
            None => {
                return Err(refusal(
                    Reason::Injector,
                    "@GenerateInjector without a program",
                ));
            }
        }
    }
    let (mut text, inputs, extras) =
        generate_views(package, source, file_name, findings, resolver, names, index)?;
    let refs: Vec<&metadata::Injector> = injectors.iter().map(|i| i.as_ref()).collect();
    let (imports, body) = injector::emit(&refs).map_err(|f| refusal(Reason::Injector, f))?;
    let mark = format!("import '{file_name}';\n");
    let after = text.find(&mark).map(|i| i + mark.len()).ok_or_else(|| {
        refusal(
            Reason::Injector,
            "generated file without the import of itself",
        )
    })?;
    text.insert_str(after, &imports);
    text.push('\n');
    text.push_str(&body);
    Ok((text, inputs, extras))
}

/// The `.template.dart` of the views (and of the `XNgCd`s) of a file, without
/// the injectors.
fn generate_views(
    package: &Package,
    source: &Path,
    file_name: &str,
    findings: &Findings,
    resolver: Option<&dyn resolution::Resolution>,
    names: &mut Interner,
    index: &NgIndex,
) -> Result<Generated, Refusal> {
    if findings.components.is_empty() && findings.directives.is_empty() && findings.pipes.is_empty()
    {
        return Ok((
            template_trivial(file_name),
            vec![source.to_path_buf()],
            Vec::new(),
        ));
    }
    // Directives and pipes do not generate a view: their file is the trivial
    // one, unless a directive has `@HostBinding` — then the official compiler
    // generates its `DirectiveChangeDetector`.
    // The `XNgCd` of each directive of the file, with the inherited bindings
    // (case j84).
    let hosts = effective_hosts(package, source, findings, resolver, index)?;
    if findings.components.is_empty() {
        if hosts.is_empty() {
            return Ok((
                template_trivial(file_name),
                vec![source.to_path_buf()],
                Vec::new(),
            ));
        }
        // The `XNgCd` classes in source order, with the shared import table; a
        // directive without `@HostBinding` and a pipe generate nothing.
        if let Some(m) = hosts.iter().find_map(|h| h.refused.clone()) {
            return Err(refusal(Reason::HostBindingOnDirective, m));
        }
        let hs: Vec<&Host> = hosts.iter().collect();
        return Ok((
            view::directive_change_detector(&hs, file_name),
            vec![source.to_path_buf()],
            Vec::new(),
        ));
    }
    // A directive without `@HostBinding`/`@HostListener` and a pipe generate
    // nothing in the file (case j29); one with `@HostBinding` gets the `XNgCd`
    // class after the components (case j45).
    if let Some(m) = hosts.iter().find_map(|h| h.refused.clone()) {
        return Err(refusal(Reason::HostBindingOnDirective, m));
    }
    // Several components: there is a single import table and the fragments come
    // out in source order (case i24); the import of each one's stylesheet is
    // allocated when its `styles$X` list is written (case j44).
    let mut imp = view::Imports::default();
    let mut fragments = Vec::new();
    let mut inputs = vec![source.to_path_buf()];
    for comp in &findings.components {
        let (fragment, html) = component_fragment(
            package, source, file_name, comp, resolver, names, index, &mut imp,
        )?;
        fragments.push(fragment);
        inputs.extend(html);
    }
    for h in &hosts {
        fragments.push(view::ngcd_class(h, file_name, &mut imp));
    }
    Ok((
        view::assemble_file(file_name, &imp, &fragments),
        inputs,
        Vec::new(),
    ))
}

/// The `Local` of a component: where it lives and its metadata.
fn component_local<'a>(
    package: &'a Package,
    source: &'a Path,
    relative: &'a str,
    file_name: &'a str,
    comp: &component::Component,
    index: &NgIndex,
) -> view::Local<'a> {
    view::Local {
        package: &package.name,
        relative,
        file: file_name,
        path: source,
        root: &package.root,
        template_url: template_url(package, source, comp),
        metadata: library_uri(package, source).and_then(|uri| {
            index
                .by_class
                .get(&(uri, comp.class.clone()))
                .and_then(|f| f.metadata.clone())
        }),
    }
}

/// The template of a component, already parsed: the text of the `.html` (or
/// that of the annotation, with the positions shifted to those of the `.dart`)
/// and the file that feeds it.
fn component_template(
    source: &Path,
    comp: &component::Component,
) -> Result<(Vec<html::Node>, Option<PathBuf>), Refusal> {
    let missing = || refusal(Reason::MissingTemplate, "templateUrl not found");
    Ok(match (&comp.template, &comp.template_url) {
        (Some(t), _) => {
            let (mut nodes, error) = html::parse_in_mode(t, comp.preserve_whitespace);
            if let Some(e) = error {
                return Err(refusal(
                    Reason::NotUnderstood,
                    format!("invalid template: {e}"),
                ));
            }
            if let Some(k) = comp.template_offset {
                html::shift_positions(&mut nodes, k);
            }
            (nodes, None)
        }
        (None, Some(url)) => {
            let path = source.parent().ok_or_else(missing)?.join(url);
            let text = std::fs::read_to_string(&path).map_err(|_| missing())?;
            let (nodes, error) = html::parse_in_mode(&text, comp.preserve_whitespace);
            if let Some(e) = error {
                return Err(refusal(
                    Reason::NotUnderstood,
                    format!("invalid template: {e}"),
                ));
            }
            (nodes, Some(path))
        }
        (None, None) => (html::parse(""), None),
    })
}

/// The fragment of a component in the `.template.dart`, with its `.html`.
#[allow(clippy::too_many_arguments)]
fn component_fragment(
    package: &Package,
    source: &Path,
    file_name: &str,
    comp: &component::Component,
    resolver: Option<&dyn resolution::Resolution>,
    names: &mut Interner,
    index: &NgIndex,
    imp: &mut view::Imports,
) -> Result<(String, Option<PathBuf>), Refusal> {
    if let Some(r) = comp.not_understood.first() {
        return Err(r.clone());
    }
    let (nodes, html_file) = component_template(source, comp)?;
    let relative = package.relative(source);
    let local = component_local(package, source, &relative, file_name, comp, index);
    let (used, unsupported) = index.directives_of(comp, source, resolver);
    if let Some(r) = unsupported.into_iter().next() {
        return Err(r);
    }
    let children = children_by_tag(&used);
    // The `pipes:` list only matters if the template uses a pipe (case b19):
    // its refusal goes along and the view decides.
    let (pipes, unsupported) = index.pipes_of(comp, source, resolver);
    let pipes = match unsupported.into_iter().next() {
        Some(r) => Err(r),
        None => Ok(pipes),
    };
    let text = view::component_fragment(
        comp, &local, &nodes, resolver, names, &children, &used, &pipes, imp,
    )?;
    // The stylesheet is another output ([`generate_sheet`], per `.css`, like the
    // official `StylesheetCompiler`): the template only imports its module, and
    // the contents do not change it. Here we only check that it exists.
    for url in &comp.style_urls {
        // `package:this/x.css` is `lib/x.css`; one from another package is an
        // output of that package's build, which the template only imports by
        // URI.
        let css = match url.strip_prefix("package:").and_then(|r| r.split_once('/')) {
            Some((p, rest)) if p == package.name => package.root.join("lib").join(rest),
            Some(_) => continue,
            None => source
                .parent()
                .ok_or_else(|| refusal(Reason::Styles, "stylesheet outside lib/"))?
                .join(url),
        };
        if sheet_text(package, &css).is_none() {
            return Err(refusal(Reason::Styles, "stylesheet not found"));
        }
    }
    Ok((text, html_file))
}

/// `package:` URI of the template file — what the official compiler writes in
/// the `REF` comment of each binding. Only for components in `lib/` with
/// `templateUrl`; with the template written in the annotation the reference is
/// different.
fn template_url(package: &Package, source: &Path, comp: &component::Component) -> Option<String> {
    // Template in the annotation: the reference is the `.dart` (`asset:`), with
    // its positions — only when the literal allows counting them.
    if comp.template.is_some() {
        comp.template_offset?;
        return Some(format!(
            "asset:{}/{}",
            package.name,
            package.relative(source)
        ));
    }
    let url = comp.template_url.as_ref()?;
    let html = source.parent()?.join(url);
    let rel = package.relative(&html);
    let inside_lib = rel.strip_prefix("lib/")?;
    Some(format!("package:{}/{inside_lib}", package.name))
}

/// Does a component's stylesheet compile (Sass and shim)? It is the same
/// computation the generator does; the scoreboard uses it so as not to mark as
/// pending what already comes out.
/// The stylesheet text of a component with `ViewEncapsulation.none`, which goes
/// without a shim into the `.css.dart`: the written `.css`, as is —
/// ngcompiler's `compileStylesheet` only removes the `@import`s
/// (`extractStyleUrls`) and escapes the string.
///
/// # Errors
///
/// The refused shape, when the text is not known byte for byte: the stylesheet
/// comes from `sass_builder` (the `.css` does not exist; the formatting of its
/// output is not ours), has `@import` (it becomes another entry in the list) or
/// cannot be read.
/// The text of a `.css` stylesheet, as the official `StylesheetCompiler` reads
/// it: the output of `sass_builder` in this build (the engine passes it in
/// [`Package::generated_sheets`]), the written `.css`, or — outside the engine,
/// without `sass_builder` — the `.scss` next to it compiled here. `None`
/// without any.
pub(crate) fn sheet_text(package: &Package, css: &Path) -> Option<Result<String, view::Reason>> {
    // `buildStep.readAsString` decodes with `utf8.decode`, which strips the
    // leading BOM (the `compressed` output of dart-sass with a non-ASCII
    // character starts with it).
    let without_bom = |t: String| match t.strip_prefix('\u{FEFF}') {
        Some(rest) => rest.to_string(),
        None => t,
    };
    if let Some(t) = package.generated_sheets.get(css) {
        return Some(Ok(without_bom(t.clone())));
    }
    if let Ok(t) = std::fs::read_to_string(css) {
        return Some(Ok(without_bom(t)));
    }
    let scss = css.with_extension("scss");
    let source = std::fs::read_to_string(&scss).ok()?;
    Some(sass::compile_in(&source, scss.parent()).map(without_bom))
}

/// The outputs of the ngdart `StylesheetCompiler` for a `.css` stylesheet
/// (`compileStylesheet`, `stylesheet_compiler/builder.dart`): the
/// `.css.shim.dart`, with the CSS rewritten for emulated encapsulation, and the
/// `.css.dart`, with it as is. Each resolvable `@import` leaves the text and
/// becomes the module of the imported stylesheet, before it
/// (`extractStyleUrls`, `_compileStyles`); the text goes through the emitter's
/// `escapeSingleQuoteString`. Each output comes with the contents or the reason
/// for not generating it.
pub fn generate_sheet(package: &Package, css: &Path) -> Vec<(PathBuf, Result<String, Refusal>)> {
    let destination = |suffix: &str| {
        css.with_file_name(format!(
            "{}{suffix}",
            css.file_name().unwrap_or_default().to_string_lossy()
        ))
    };
    let text = match sheet_text(package, css) {
        Some(Ok(t)) => Ok(t),
        Some(Err(_)) => Err(refusal(
            Reason::Styles,
            "Sass the native compiler does not translate",
        )),
        None => Err(refusal(Reason::Styles, "stylesheet not found")),
    };
    let asset = format!("asset:{}/{}", package.name, package.relative(css));
    let text = text.map(|t| extract_imports(&asset, &t));
    let output = |suffix: &str, content: &dyn Fn(&str) -> Result<String, Refusal>| {
        text.clone().and_then(|(style, urls)| {
            let module = format!("{asset}{suffix}");
            let mut head = String::new();
            let mut items = Vec::new();
            for (k, url) in urls.iter().enumerate() {
                let imported = format!("{url}{suffix}");
                let path = resolution::asset_of_uri(&imported, "", Path::new(""))
                    .or_else(|| imported.starts_with("asset:").then(|| imported.clone()))
                    .and_then(|target| resolution::import_path(&module, &target))
                    .ok_or_else(|| {
                        refusal(Reason::Styles, "stylesheet @import without an import path")
                    })?;
                head.push_str(&format!("import '{path}' as import{k};\n"));
                items.push(format!("import{k}.styles"));
            }
            items.push(view::literal(&content(&style)?));
            Ok(format!("{head}{}", style_list(&items)))
        })
    };
    vec![
        (
            destination(".shim.dart"),
            output(".shim.dart", &|t| {
                css::shim(t).map_err(|_| {
                    refusal(
                        Reason::Styles,
                        "CSS the ngdart shim refuses (the official compiler throws)",
                    )
                })
            }),
        ),
        (
            destination(".dart"),
            output(".dart", &|t| Ok(t.to_string())),
        ),
    ]
}

/// `final List<Object> styles = [..];` as the emitter writes it (the
/// `.css.dart` does not go through `DartFormatter`): with one item, on one
/// line; with more, `[` and the items on the next line, indented two spaces,
/// separated by `,` and wrapping when the line goes past 80 (`visitAllObjects`
/// with `keepOnSameLine`, `currentLineLength` = indentation level +
/// characters), and `]` and `;` on their own lines.
fn style_list(items: &[String]) -> String {
    if let [item] = items {
        return format!("final List<Object> styles = [{item}];");
    }
    let mut lines = vec!["final List<Object> styles = [".to_string()];
    let mut line = String::new();
    for (k, item) in items.iter().enumerate() {
        line.push_str(item);
        if k + 1 < items.len() {
            // The test looks at the line before the separator (in UTF-16 code
            // units, like Dart's `String.length`); the comma goes in anyway.
            let line_break = 1 + line.encode_utf16().count() > 80;
            line.push(',');
            if line_break {
                lines.push(format!("  {line}"));
                line.clear();
            }
        }
    }
    lines.push(format!("  {line}"));
    lines.push("]".to_string());
    lines.push(";".to_string());
    lines.join("\n")
}

/// `extractStyleUrls`: each `@import` (the official `_cssImportRe`,
/// `@import\s+(?:url\()?\s*(?:(?:['"]([^'"]*))|([^;\)\s]*))[^;]*;?`) with a
/// resolvable URL — relative, `package:` or `asset:` — leaves the text, and the
/// URL resolved against that of the stylesheet goes into the list; one with
/// another scheme or absolute stays.
fn extract_imports(asset: &str, css: &str) -> (String, Vec<String>) {
    let mut output = String::with_capacity(css.len());
    let mut urls = Vec::new();
    let b = css.as_bytes();
    let mut i = 0;
    let mut copied = 0;
    while let Some(k) = css[i..].find("@import") {
        let begin = i + k;
        let mut j = begin + "@import".len();
        // `\s+`
        let spaces = css[j..].len() - css[j..].trim_start().len();
        if spaces == 0 {
            i = j;
            continue;
        }
        j += spaces;
        if css[j..].starts_with("url(") {
            j += 4;
        }
        j += css[j..].len() - css[j..].trim_start().len();
        let url = if j < b.len() && (b[j] == b'\'' || b[j] == b'"') {
            j += 1;
            let f = css[j..].find(['\'', '"']).map_or(css.len(), |f| j + f);
            let u = css[j..f].to_string();
            j = f;
            u
        } else {
            let f = css[j..]
                .find(|c: char| c == ';' || c == ')' || c.is_whitespace())
                .map_or(css.len(), |f| j + f);
            let u = css[j..f].to_string();
            j = f;
            u
        };
        // `[^;]*;?`
        let end = match css[j..].find(';') {
            Some(f) => j + f + 1,
            None => css.len(),
        };
        if resolvable(&url) {
            urls.push(resolver_url(asset, &url));
            output.push_str(&css[copied..begin]);
            copied = end;
        }
        i = end.max(begin + 1);
    }
    output.push_str(&css[copied..]);
    (output, urls)
}

/// `isStyleUrlResolvable`.
fn resolvable(url: &str) -> bool {
    if url.is_empty() || url.starts_with('/') {
        return false;
    }
    match url.find(':') {
        Some(k) if !url[..k].contains(['/', '?', '#']) && k > 0 => {
            matches!(&url[..k], "package" | "asset")
        }
        _ => true,
    }
}

/// `Uri.parse(base).resolve(url)` for stylesheet URLs: with a scheme, the URL
/// itself; relative, against the base's folder, without `.` and `..`.
fn resolver_url(base: &str, url: &str) -> String {
    if url.starts_with("package:") || url.starts_with("asset:") {
        return url.to_string();
    }
    let (scheme, path) = base.split_once(':').unwrap_or(("", base));
    let folder = path.rsplit_once('/').map_or("", |(p, _)| p);
    let mut parts: Vec<&str> = folder.split('/').filter(|p| !p.is_empty()).collect();
    for p in url.split('/') {
        match p {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    format!("{scheme}:{}", parts.join("/"))
}

/// Set of refusals of a pending file, for the scoreboard: the first one, and
/// all those the emission in collection mode finds.
fn file_reasons(
    package: &Package,
    source: &Path,
    findings: &Findings,
    first: Refusal,
    resolver: Option<&dyn resolution::Resolution>,
    index: &NgIndex,
    names: &mut Interner,
) -> std::collections::BTreeSet<Refusal> {
    let mut unsupported = std::collections::BTreeSet::new();
    unsupported.insert(first);
    let relative = package.relative(source);
    let name = source
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    for comp in &findings.components {
        let Ok((nodes, _)) = component_template(source, comp) else {
            continue;
        };
        let local = component_local(package, source, &relative, &name, comp, index);
        let (used, outside_list) = index.directives_of(comp, source, resolver);
        unsupported.extend(outside_list);
        let children = children_by_tag(&used);
        let (pipes, outside_pipes) = index.pipes_of(comp, source, resolver);
        let pipes = match outside_pipes.into_iter().next() {
            Some(r) => Err(r),
            None => Ok(pipes),
        };
        unsupported.extend(view::collect(
            comp, &local, &nodes, resolver, names, &children, &used, &pipes,
        ));
    }
    unsupported
}

/// Generates for a whole package (`lib/`, `web/`, `test/`), completing what it
/// cannot generate yet with a fallback generation — today, what `build_runner`
/// left on disk.
///
/// It is this mix that lets the migration happen in steps: with each new shape
/// the generator learns, a file leaves the fallback and enters ours, and the
/// application keeps compiling the whole time.
pub fn generate_with_fallback(
    package: &Package,
    interner: &mut Interner,
    fallback: Option<&Generation>,
    program: Option<&resolution::Resolver>,
) -> (Arc<Generation>, Scoreboard) {
    let mut c = GenerationBuilder::new();
    let mut scoreboard = Scoreboard::default();
    let dirs: Vec<PathBuf> = ["lib", "web", "test"]
        .iter()
        .map(|d| package.root.join(d))
        .filter(|d| d.is_dir())
        .collect();
    generate_in(package, &dirs, interner, &mut c, &mut scoreboard, program);
    if let Some(fallback) = fallback {
        // The fallback only enters where we do not generate: pending files of
        // this package and everything outside it.
        for (path, f) in fallback.iter() {
            if c.contains(path) {
                continue;
            }
            c.put(
                path.clone(),
                f.content.to_string(),
                f.generator,
                f.inputs.to_vec(),
            );
        }
    }
    let g = c.finish(1).unwrap_or_else(|errors| {
        for m in errors {
            eprintln!("ngdart generator error: {m}");
        }
        Arc::new(Generation::default())
    });
    (g, scoreboard)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each `.css` gives the `.css.shim.dart` and the `.css.dart` (the text as
    /// is); the stylesheet from the `sass_builder` output wins over the disk,
    /// and `@import` is refused in both.
    #[test]
    fn generate_sheet_gives_both_outputs() {
        let dir = tempfile::tempdir().expect("temporary dir");
        let a = dir.path().join("a.css");
        std::fs::write(&a, ".a { content: '\\e939'; }\n").unwrap();
        let b = dir.path().join("b.css");
        std::fs::write(&b, "@import 'c.css';\n").unwrap();
        let mut package = Package::default();
        let outputs = generate_sheet(&package, &a);
        assert_eq!(outputs[0].0, dir.path().join("a.css.shim.dart"));
        assert_eq!(outputs[1].0, dir.path().join("a.css.dart"));
        assert_eq!(
            outputs[1].1.as_deref(),
            Ok("final List<Object> styles = ['.a { content: \\'\\\\e939\\'; }\\n'];")
        );
        assert!(generate_sheet(&package, &b).iter().all(|(_, r)| r.is_err()));
        package
            .generated_sheets
            .insert(a.clone(), ".x{}".to_string());
        assert_eq!(
            generate_sheet(&package, &a)[1].1.as_deref(),
            Ok("final List<Object> styles = ['.x{}'];")
        );
        let z = dir.path().join("z.css");
        assert!(generate_sheet(&package, &z).iter().all(|(_, r)| r.is_err()));
    }

    fn findings_of(source: &str) -> Findings {
        let mut i = Interner::new();
        let t = dartforge_frontend::lexer::lex(source);
        let u = dartforge_frontend::parser::parse_lexed(source, t, &mut i);
        find(&u.ast, &u.unit, source, &i)
    }

    #[test]
    fn library_without_angular_is_trivial() {
        assert!(findings_of("class A {}\nint x = 1;\n").trivial());
    }

    #[test]
    fn finds_component_with_and_without_prefix() {
        let a = findings_of("@Component(selector: 'x')\nclass XComp {}\n");
        assert_eq!(a.components[0].class, "XComp");
        assert_eq!(a.components[0].selector, "x");
        let b = findings_of("@ng.Component(selector: 'x')\nclass YComp {}\n");
        assert_eq!(b.components[0].class, "YComp");
    }

    #[test]
    fn finds_directive_and_pipe() {
        let a = findings_of("@Directive(selector: '[d]')\nclass D {}\n@Pipe('p')\nclass P {}\n");
        assert_eq!(a.directives.len(), 1);
        assert_eq!(a.directives[0].class, "D");
        assert_eq!(a.directives[0].selector, "[d]");
        assert_eq!(a.pipes.len(), 1);
        assert_eq!(a.pipes[0].class, "P");
        assert_eq!(a.pipes[0].name, "p");
        assert!(a.pipes[0].pure);
        assert!(!a.trivial());
    }

    /// Exact bytes of the official compiler, checked against
    /// `loading.template.dart` of new_sali/frontend.
    #[test]
    fn trivial_byte_for_byte() {
        let expected = "// **************************************************************************\n// Generator: AngularDart Compiler\n// **************************************************************************\n\nimport 'loading.dart';\n";
        assert_eq!(template_trivial("loading.dart"), expected);
    }

    /// new_sali sets up injection like this; if this slips, we generate an
    /// empty file in place of a whole injector.
    #[test]
    fn finds_injector_in_top_level_variable() {
        let a = findings_of(
            "@GenerateInjector([])
final InjectorFactory injector = self.injector$Injector;
",
        );
        assert_eq!(a.injectors.len(), 1);
        assert!(!a.trivial());
    }

    /// The build engine index: a file goes in, is found by its selector, is
    /// replaced and goes out, without touching the others.
    #[test]
    fn incremental_index() {
        let package = Package {
            generated_sheets: Default::default(),
            name: "p".into(),
            root: PathBuf::from("/r"),
        };
        let a = Path::new("/r/lib/a.dart");
        let b = Path::new("/r/lib/src/b.dart");
        let mut index = NgIndex::new();
        index.update(
            &package,
            a,
            &findings_of("@Component(selector: 'x-a', template: '')\nclass A {}\n"),
            None,
        );
        index.update(
            &package,
            b,
            &findings_of("@Directive(selector: '[b]')\nclass B {}\n"),
            None,
        );
        assert_eq!(
            index.declarer("x-a"),
            Some(("package:p/a.dart".into(), "A".into()))
        );
        assert_eq!(
            index.declarer("[b]"),
            Some(("package:p/src/b.dart".into(), "B".into()))
        );
        // The file changed its selector: the old one disappears.
        index.update(
            &package,
            a,
            &findings_of("@Component(selector: 'x-novo', template: '')\nclass A {}\n"),
            None,
        );
        assert_eq!(index.declarer("x-a"), None);
        assert!(index.declarer("x-novo").is_some());
        index.remove(a);
        assert_eq!(index.declarer("x-novo"), None);
        assert!(index.declarer("[b]").is_some());
    }

    #[test]
    fn path_next_to_source() {
        assert_eq!(
            template_path(Path::new("/p/lib/a/foo.dart")),
            PathBuf::from("/p/lib/a/foo.template.dart")
        );
    }
}
