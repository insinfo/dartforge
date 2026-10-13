//! The view emitter: the Dart that ngdart expects to find in the
//! `.template.dart`.
//!
//! The ABI is the contract, and it is fixed: for a component `X` with selector
//! `s`, the file has `ViewX0 extends ComponentView<X>`, the constant factory
//! `_XNgFactory`, the getter `XNgFactory`, the function `createXFactory()` and
//! the host view `_ViewXHost0 extends HostView<X>` with its
//! `viewFactory_XHost0()`. These are the names through which the application
//! and `ngrouter` reach the component.
//!
//! The `importN` come out numbered in the order the official emitter allocates
//! them — `dart:html` in the view constructor, `dom_helpers` on its first use
//! inside `build()`, and `package:ngdart/angular.dart` taking a number but
//! written without a prefix, because `ComponentFactory` appears unqualified.
//! That is the only reason the output can be compared byte for byte with the
//! official one.
use crate::component::Component;
use crate::dom;
use crate::html::Node;
use crate::resolucao::{Resolution, asset_of_uri, import_path};
use std::fmt::Write;
use std::path::Path;

/// Import table of the generated file.
#[derive(Default, Clone)]
pub struct Imports {
    /// (key, URI, with prefix). The key is the URI, except when the official
    /// compiler imports the same library twice (see [`Self::q_keyed`]).
    items: Vec<(String, String, bool)>,
}

impl Imports {
    /// Allocates (or reuses) the number of a URI and returns the prefix — empty
    /// for the libraries the official compiler imports without a prefix.
    pub fn alias(&mut self, uri: &str) -> String {
        if UNPREFIXED.contains(&uri) {
            self.index(uri, false);
            return String::new();
        }
        let n = self.index(uri, true);
        format!("import{n}")
    }

    /// `alias` followed by a dot, or empty: `import3.NgFor` and `NgIf`.
    pub fn q(&mut self, uri: &str) -> String {
        let a = self.alias(uri);
        if a.is_empty() { a } else { format!("{a}.") }
    }

    /// A compiler DOM identifier (`document`, `HtmlElement`, `DivElement`…,
    /// by its `dart:html` name), qualified by the import of its module in the
    /// generation dialect ([`crate::dialect::dom`]): `import8.DivElement` in 8,
    /// `import5.HTMLDivElement` in 9.
    pub fn dom(&mut self, name: &str) -> String {
        let (uri, n) = crate::dialect::dom(name);
        format!("{}{n}", self.q(uri))
    }

    /// Allocates a number without a prefix — the URI is imported unqualified.
    pub fn without_alias(&mut self, uri: &str) {
        self.index(uri, false);
    }

    /// Like [`Self::q`], but with a key of its own: the same URI gets another
    /// number. This is what the official compiler does with the type argument
    /// of a `MultiToken` (`MultiToken<import20.ControlValueAccessor<dynamic>>`
    /// next to the field's `import7.ControlValueAccessor`): the type comes from
    /// another analyzer path and the import is a different one, even with the
    /// same text.
    pub fn q_keyed(&mut self, key: &str, uri: &str) -> String {
        let n = match self.items.iter().position(|(c, _, _)| c == key) {
            Some(i) => i,
            None => {
                self.items.push((key.to_string(), uri.to_string(), true));
                self.items.len() - 1
            }
        };
        format!("import{n}.")
    }

    fn index(&mut self, uri: &str, with_alias: bool) -> usize {
        if let Some(i) = self.items.iter().position(|(c, _, _)| c == uri) {
            return i;
        }
        self.items
            .push((uri.to_string(), uri.to_string(), with_alias));
        self.items.len() - 1
    }

    fn write_to(&self, out: &mut String) {
        for (i, (_, uri, with_alias)) in self.items.iter().enumerate() {
            // Internally the URIs are those of `ngdart` 8; the output is in the
            // dialect.
            let uri = crate::dialect::writing(uri);
            if *with_alias {
                let _ = writeln!(out, "import '{uri}' as import{i};");
            } else {
                let _ = writeln!(out, "import '{uri}';");
            }
        }
    }
}

const COMPONENT_VIEW: &str = "package:ngdart/src/core/linker/views/component_view.dart";
const STYLE_ENCAPSULATION: &str = "package:ngdart/src/core/linker/style_encapsulation.dart";
const VIEW: &str = "package:ngdart/src/core/linker/views/view.dart";
const CHANGE_DETECTION: &str = "package:ngdart/src/meta/change_detection_constants.dart";
const UTILITIES: &str = "package:ngdart/src/utilities.dart";
const DOM_HELPERS: &str = "package:ngdart/src/runtime/dom_helpers.dart";
const HOST_VIEW: &str = "package:ngdart/src/core/linker/views/host_view.dart";
const ANGULAR: &str = "package:ngdart/angular.dart";
const DI_ERRORS: &str = "package:ngdart/src/di/errors.dart";
const TEXT_BINDING: &str = "package:ngdart/src/runtime/text_binding.dart";
const CHECK_BINDING: &str = "package:ngdart/src/runtime/check_binding.dart";
const DEVTOOLS: &str = "package:ngdart/src/devtools.dart";
const VIEW_CONTAINER: &str = "package:ngdart/src/core/linker/view_container.dart";
const TEMPLATE_REF: &str = "package:ngdart/src/core/linker/template_ref.dart";
const ELEMENT_REF: &str = "package:ngdart/src/core/linker/element_ref.dart";
const NG_IF: &str = "package:ngdart/src/common/directives/ng_if.dart";
const NG_FOR: &str = "package:ngdart/src/common/directives/ng_for.dart";
const NG_SWITCH: &str = "package:ngdart/src/common/directives/ng_switch.dart";
const NG_TEMPLATE_OUTLET: &str = "package:ngdart/src/common/directives/ng_template_outlet.dart";
/// The `star` name put on a hand-written `<template>`
/// ([`template_as_container`]): it marks the embedded view boundary for
/// whoever walks the template, without being any directive.
const TEMPLATE_MARK: &str = "\u{0}template";
const EMBEDDED_VIEW: &str = "package:ngdart/src/core/linker/views/embedded_view.dart";
const PROXIES: &str = "package:ngdart/src/runtime/proxies.dart";
const RENDER_VIEW: &str = "package:ngdart/src/core/linker/views/render_view.dart";
const DIRECTIVE_CHANGE_DETECTOR: &str =
    "package:ngdart/src/core/change_detection/directive_change_detector.dart";

/// Libraries that the official emitter imports **without a prefix**, because
/// they are ngdart's public API (`_allowListedImports` in
/// `output/dart_emitter.dart`).
const UNPREFIXED: &[&str] = &[
    ANGULAR,
    "dart:core",
    ELEMENT_REF,
    VIEW_CONTAINER,
    TEMPLATE_REF,
    "package:ngdart/src/core/change_detection/change_detection.dart",
    NG_IF,
    "package:ngdart/src/core/linker/app_view.dart",
    "package:ngdart/src/core/render/api.dart",
];
const INTERPOLATE: &str = "package:ngdart/src/runtime/interpolate.dart";
const INTL: &str = "package:intl/intl.dart";
const QUERIES: &str = "package:ngdart/src/runtime/queries.dart";
const SAFE_HTML: &str = "package:ngdart/src/security/safe_html_adapter.dart";
const APP_VIEW_UTILS: &str = "package:ngdart/src/core/linker/app_view_utils.dart";

/// Why a file is not generated by us yet. The scoreboard counts by reason:
/// that is what tells which shape is worth learning next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reason {
    /// `@Directive` or `@Pipe` in the same file.
    DirectiveOrPipe,
    /// `@GenerateInjector`.
    Injector,
    /// `@HostBinding`/`@HostListener` on a directive in a shape that the
    /// generated `DirectiveChangeDetector` does not cover yet (inheritance, a
    /// binding that is not `class.x`, several directives in the file).
    HostBindingOnDirective,
    /// More than one component in the file.
    SeveralComponents,
    /// `styleUrls`/`styles`: they touch `styles$X` and turn on the style shim.
    Styles,
    /// Annotated parameter (`@Optional`, `@Inject(…)`, `@Attribute`…).
    AnnotatedInjection,
    /// Named parameter in the constructor.
    NamedInjection,
    /// Generic token (`List<X>`, `OpaqueToken<String>`).
    GenericInjection,
    /// Parameter without a written type.
    UntypedInjection,
    /// The semantic database did not find the library that declares the type.
    UnresolvedInjection,
    /// `[x]`, `(x)`, `[(x)]`, `#ref` or `*ngIf` in the template.
    Binding,
    /// `{{ … }}` in the template.
    Interpolation,
    /// Tag that is not HTML: a component or directive of the application.
    ComponentInTemplate,
    /// `<ng-content>`.
    Projection,
    /// Inline `style="..."`.
    InlineStyle,
    /// `.html` file of the `templateUrl` not found.
    MissingTemplate,
    /// `@Input`/`@Output` on a child component.
    BindingOnChild,
    /// Attribute or binding that belongs to an ecosystem directive
    /// (`ngClass`, `ngModel`…), not to the DOM.
    Directive,
    /// `providers:` with providers: the host element's injection.
    Providers,
    /// `@ViewChild` whose target is in an embedded view (`*ngIf`, `*ngFor`),
    /// `@ViewChildren`, or a reference that the template does not have.
    DynamicViewChild,
    /// `@ViewChild` that queries a component or directive (type selector,
    /// `read:`, `#ref` on a child component, a field that is not `Element`).
    ViewChildOnChild,
    /// Pipe used in the template (`x | name`, `$pipe.name(x)`).
    UsedPipes,
    /// `encapsulation:` — changes the style shim.
    Encapsulation,
    /// `@HostListener` on a component.
    HostListenerOnComponent,
    /// `@HostBinding` on a component: the view's `detectHostChanges`.
    HostBindingOnComponent,
    /// `@ContentChild`/`@ContentChildren`.
    ContentChild,
    /// Another shape of the component that the generator cannot translate and
    /// therefore refuses — an unknown argument of `@Component`, `@ViewChild`
    /// with options.
    NotUnderstood,
    /// An element matches the selector of a directive from `directives:` that
    /// the emitter does not instantiate (`<form>` with `NgForm`, `<option>`,
    /// `[routerLink]`…). Emitting the bare element would compile and do
    /// something else.
    DirectiveBySelector,
    /// `(event)` in the template outside what the emitter can bind.
    Event,
    /// `@i18n` outside the shape the emitter translates (message with HTML,
    /// annotation on a child component or on `*`, another annotation).
    I18n,
    /// Template expression outside what the converter translates. It is the
    /// converter's provisional category: its caller swaps it for the one of
    /// the context (interpolation, binding, event) with [`Refusal::em`].
    Expression,
}

/// A refusal: the scoreboard category and the concrete sub-shape that the
/// emitter cannot translate (`event: handler with assignment`, `interpolation:
/// kind of ternary`). The scoreboard counts both; it is the sub-shape that
/// tells what to tackle next.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Refusal {
    pub reason: Reason,
    pub shape: String,
}

impl Refusal {
    pub fn new(reason: Reason, shape: impl Into<String>) -> Self {
        Refusal {
            reason,
            shape: shape.into(),
        }
    }

    /// The same sub-shape in the context of whoever called the expression
    /// converter. A refusal that already has its own category (pipe) stays as
    /// it is.
    pub fn em(self, reason: Reason) -> Self {
        if self.reason == Reason::Expression {
            Refusal { reason, ..self }
        } else {
            self
        }
    }

    pub fn text(&self) -> String {
        format!("{}: {}", self.reason.text(), self.shape)
    }
}

/// Shortcut: `Err(refusal(Reason::Event, "handler with assignment"))`.
pub fn refusal(reason: Reason, shape: impl Into<String>) -> Refusal {
    Refusal::new(reason, shape)
}

impl Reason {
    pub fn text(self) -> &'static str {
        match self {
            Reason::DirectiveOrPipe => "directive or pipe",
            Reason::Injector => "@GenerateInjector",
            Reason::HostBindingOnDirective => "@HostBinding/@HostListener on a directive",
            Reason::SeveralComponents => "several components in the file",
            Reason::Styles => "stylesheet",
            Reason::AnnotatedInjection => "injection: annotated parameter",
            Reason::NamedInjection => "injection: named parameter",
            Reason::GenericInjection => "injection: generic token",
            Reason::UntypedInjection => "injection: untyped parameter",
            Reason::UnresolvedInjection => "injection: unresolved type",
            Reason::Binding => "binding in the template",
            Reason::Interpolation => "interpolation",
            Reason::ComponentInTemplate => "component in the template",
            Reason::Projection => "<ng-content>",
            Reason::InlineStyle => "inline style",
            Reason::MissingTemplate => "template not found",
            Reason::BindingOnChild => "binding on a child component",
            Reason::Directive => "directive binding",
            Reason::Providers => "providers: [..]",
            Reason::DynamicViewChild => "@ViewChild in an embedded view / @ViewChildren",
            Reason::ViewChildOnChild => "@ViewChild of a component or directive",
            Reason::UsedPipes => "pipe used in the template",
            Reason::Encapsulation => "encapsulation:",
            Reason::HostListenerOnComponent => "@HostListener on a component",
            Reason::HostBindingOnComponent => "@HostBinding on a component",
            Reason::ContentChild => "@ContentChild",
            Reason::NotUnderstood => "other component shape not understood",
            Reason::DirectiveBySelector => "directive matched by selector",
            Reason::Event => "event",
            Reason::I18n => "@i18n",
            Reason::Expression => "expression",
        }
    }
}

/// The `#ref`s the emitter accepts on an HTML element: without a value
/// (`#f="ngForm"` points to a directive) and not used in a template expression
/// — once used, the name becomes a view local and the read changes. Such a
/// `#ref` is just a name for the node, and whoever reads it is `@ViewChild`.
fn free_refs(nodes: &[Node]) -> std::collections::HashSet<String> {
    fn collect_all(nodes: &[Node], out: &mut Vec<(String, String)>) {
        for n in nodes {
            if let Node::Element(e) = n {
                out.extend(
                    e.references
                        .iter()
                        .map(|r| (r.name.clone(), r.value.clone())),
                );
                collect_all(&e.children, out);
            }
        }
    }
    let mut refs = Vec::new();
    collect_all(nodes, &mut refs);
    refs.iter()
        .filter(|(name, value)| value.is_empty() && !local_referenced(nodes, name))
        .map(|(name, _)| name.clone())
        .collect()
}

/// The `#ref`s of an embedded view that are just a name for the node: without
/// a value, declared in it, not read by any expression of its scope
/// ([`referenced_in_scope`]) and with no query looking for them — the official
/// compiler creates the element as if the `#ref` did not exist.
fn free_refs_of_view(
    nodes: &[Node],
    children: &std::collections::HashMap<String, Child>,
    queried: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    let mut own = Vec::new();
    fn walk(nodes: &[Node], out: &mut Vec<(String, String)>) {
        for n in nodes {
            if let Node::Element(e) = n {
                // What is in a `*` (or in a `<template>`) belongs to another view.
                if e.star.is_some() {
                    if e.star.as_ref().is_some_and(|l| l.name == TEMPLATE_MARK) {
                        out.extend(
                            e.references
                                .iter()
                                .map(|r| (r.name.clone(), r.value.clone())),
                        );
                    }
                    continue;
                }
                out.extend(
                    e.references
                        .iter()
                        .map(|r| (r.name.clone(), r.value.clone())),
                );
                walk(&e.children, out);
            }
        }
    }
    walk(nodes, &mut own);
    own.into_iter()
        .filter(|(name, value)| {
            value.is_empty()
                && !queried.contains(name)
                && !referenced_in_scope(nodes, name, children)
        })
        .map(|(name, _)| name)
        .collect()
}

/// The metadata of an `@i18n` message (`I18nMetadata`), with the values
/// already normalized (`_normalizeWhitespace`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MetaI18n {
    description: Option<String>,
    locale: Option<String>,
    meaning: Option<String>,
    skip: bool,
}

/// `parseI18nMetadata`: the element's annotations grouped by the attribute
/// they internationalize (`None`: the children). The name matches
/// `i18n(.locale|.meaning|.skip)?(:attribute)?`; an annotation that is not
/// `@i18n`, a parameter without a description and an empty attribute are
/// refused.
fn i18n_metadata(e: &crate::html::Element) -> Result<Vec<(Option<String>, MetaI18n)>, Refusal> {
    fn normalize(v: &str) -> String {
        v.split_whitespace().collect::<Vec<_>>().join(" ")
    }
    let mut out: Vec<(Option<String>, MetaI18n)> = Vec::new();
    for a in &e.annotations {
        let Some(rest) = a.name.strip_prefix("i18n") else {
            return Err(refusal(Reason::I18n, format!("annotation @{}", a.name)));
        };
        let (param, rest) = match rest.strip_prefix('.') {
            Some(r) => {
                let end = r.find(':').unwrap_or(r.len());
                (Some(&r[..end]), &r[end..])
            }
            None => (None, rest),
        };
        let attribute = match rest.strip_prefix(':') {
            Some("") => return Err(refusal(Reason::I18n, "@i18n: no attribute")),
            Some(x) => Some(x.to_string()),
            None if rest.is_empty() => None,
            None => return Err(refusal(Reason::I18n, format!("annotation @{}", a.name))),
        };
        let i = match out.iter().position(|(k, _)| *k == attribute) {
            Some(i) => i,
            None => {
                out.push((attribute, MetaI18n::default()));
                out.len() - 1
            }
        };
        let m = &mut out[i].1;
        match param {
            None => m.description = Some(normalize(&a.value)),
            Some("locale") => m.locale = Some(normalize(&a.value)),
            Some("meaning") => m.meaning = Some(normalize(&a.value)),
            Some("skip") => m.skip = true,
            Some(_) => return Err(refusal(Reason::I18n, format!("annotation @{}", a.name))),
        }
    }
    if out.iter().any(|(_, m)| m.description.is_none()) {
        return Err(refusal(
            Reason::I18n,
            "@i18n parameter without the description",
        ));
    }
    Ok(out)
}

/// In the component's view (outside `*`), does the first `@i18n` annotation
/// come before the first interpolation? That is what decides whether
/// `package:intl` (of the `_message_N` field) is imported before or after
/// `text_binding.dart`: both fields are allocated by `build()`, in document
/// order. `None`: there is no annotation in the component's view.
fn i18n_before_interpolation(nodes: &[Node]) -> Option<bool> {
    fn walk(nodes: &[Node], interpolated: &mut bool) -> Option<bool> {
        for n in nodes {
            match n {
                Node::Interpolation { .. } => *interpolated = true,
                Node::Element(e) if e.star.is_some() => {}
                Node::Element(e) => {
                    if !e.annotations.is_empty() {
                        // A message with HTML is a method, not a field: the
                        // import comes out with the method, after `build()`.
                        if message_in_field(e) {
                            return Some(!*interpolated);
                        }
                        continue;
                    }
                    if let Some(r) = walk(&e.children, interpolated) {
                        return Some(r);
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(nodes, &mut false)
}

/// Is there any element with an annotation (`@i18n`) in this view (outside
/// `*`)?
fn has_annotation(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Element(e) if e.star.is_some() => false,
        Node::Element(e) => !e.annotations.is_empty() || has_annotation(&e.children),
        _ => false,
    })
}

/// Does the element's `@i18n` generate any `static final String _message_N`
/// field? The attribute ones (`@i18n:x`) and the children's one when they are
/// a single text (`_isText` of `i18n.dart`); with HTML, the children's
/// message is a static method.
fn message_in_field(e: &crate::html::Element) -> bool {
    let from_attribute = e.annotations.iter().any(|a| a.name.contains(':'));
    from_attribute || matches!(e.children.as_slice(), [Node::Str(_)])
}

/// The `@i18n` message of children with HTML, as `I18nBuilder` builds it: the
/// text (escaped by hand: `\n`, `\r`, `'`, `$`, `\`) with `${startTagK}`,
/// `${endTagK}` and `${voidElementK}` in place of the tags, and the arguments
/// in the order they appear. Only an HTML element without an attribute,
/// binding, event, `#ref`, annotation or `*`, and text without `&` (the entity
/// would be decoded and escaped again); interpolation and `<ng-content>` are
/// an error in the official compiler.
///
/// # Errors
///
/// The shape that cannot be written yet.
fn message_with_html(
    nodes: &[Node],
    children: &std::collections::HashMap<String, Child>,
    text: &mut String,
    args: &mut Vec<(String, String)>,
    tags: &mut usize,
) -> Result<bool, &'static str> {
    let mut has_text = false;
    for n in nodes {
        match n {
            Node::Comment(_) => {}
            Node::Str(t) => {
                if t.contains('&') || t.contains(crate::html::NGSP) {
                    return Err("@i18n message with an HTML entity");
                }
                has_text |= !t.trim().is_empty();
                for c in t.chars() {
                    match c {
                        '\n' => text.push_str("\\n"),
                        '\r' => text.push_str("\\r"),
                        '\'' | '$' | '\\' => {
                            text.push('\\');
                            text.push(c);
                        }
                        c => text.push(c),
                    }
                }
            }
            Node::Element(e) => {
                let simple = e.attributes.is_empty()
                    && e.properties.is_empty()
                    && e.events.is_empty()
                    && e.bananas.is_empty()
                    && e.references.is_empty()
                    && e.annotations.is_empty()
                    && e.star.is_none()
                    && !children.contains_key(&e.name)
                    && dom::tag_html(&e.name)
                    && !matches!(e.name.as_str(), "template" | "ng-container");
                if !simple {
                    return Err("@i18n message with an element that is not plain HTML");
                }
                let k = *tags;
                *tags += 1;
                let mut arg = |name: String, value: String, text: &mut String| {
                    let _ = write!(text, "${{{name}}}");
                    args.push((name, value));
                };
                if crate::html::EMPTY.contains(&e.name.as_str()) {
                    if !e.children.is_empty() {
                        return Err("@i18n message with a void element with children");
                    }
                    arg(format!("voidElement{k}"), format!("<{}>", e.name), text);
                } else {
                    arg(format!("startTag{k}"), format!("<{}>", e.name), text);
                    has_text |= message_with_html(&e.children, children, text, args, tags)?;
                    let _ = write!(text, "${{endTag{k}}}");
                    args.push((format!("endTag{k}"), format!("</{}>", e.name)));
                }
            }
            Node::Interpolation { .. } | Node::Content { .. } => {
                return Err("@i18n message with interpolation or <ng-content>");
            }
        }
    }
    Ok(has_text)
}

/// Mark of the node of a `#ref` read as a local: `\u{5}name\u{6}`. The
/// declaration (`final local_x = this._el_n;`) is requested by whoever reads
/// the local, sometimes before the element is created; the mark is replaced by
/// the node when the view is complete ([`resolver_refs`]).
const REF_MARK: char = '\u{5}';
const REF_END: char = '\u{6}';

/// Prefix, in the key of [`resolver_refs`], of the view of an `onPush` child
/// found by the key of a query (`\u{5}.\u{8}f\u{6}` → `._compView_1`), read
/// from a nested view ([`QueryAssembly::map`]).
const DETECTOR_MARK: char = '\u{8}';

/// The template's `#ref` names that may become a local of some view: no
/// component method with the same name, and no member whose type is not known
/// from here (with the same name as a member, the local is the node, but the
/// official compiler's `_TypeResolver` types the read by the member:
/// [`ViewBody::declare_refs`]). Uniqueness and shadowing by `let` are per view
/// ([`local_refs`]).
fn candidate_refs(nodes: &[Node], c: &Component) -> std::collections::HashSet<String> {
    fn collect_all(nodes: &[Node], refs: &mut Vec<String>) {
        for n in nodes {
            if let Node::Element(e) = n {
                refs.extend(e.references.iter().map(|r| r.name.clone()));
                collect_all(&e.children, refs);
            }
        }
    }
    let mut refs = Vec::new();
    collect_all(nodes, &mut refs);
    refs.into_iter()
        .filter(|name| {
            !c.methods.contains_key(name.as_str())
                && c.members
                    .get(name.as_str())
                    .is_none_or(|m| !m.ty.trim().is_empty())
        })
        .collect()
}

/// The `#ref`s (from `candidates`) that become locals of this view: declared
/// only once on an HTML element or child component of it (not in a child's
/// content; those of views embedded in it belong to them), without a `let` of
/// this view with the same name — the node or the child's instance — and read
/// by some expression of it or of the embedded views in which the name is not
/// shadowed.
///
/// It is `CompileElement`'s `nameResolver.addLocal(name, renderNode)`: each
/// view has its own resolver, a child of the outer view's, and the closest
/// name wins (`let` and `#ref` of the embedded view hide the outer one).
/// Whoever reads the local outside `build()` (detection, handler, another
/// view) promotes the node to a field (`NodeReferenceStorageVisitor`), and the
/// type of the read is `dynamic` (the reference does not go into the
/// `AnalyzedClass`'s `locals`).
fn local_refs(
    nodes: &[Node],
    children: &std::collections::HashMap<String, Child>,
    candidates: &std::collections::HashSet<String>,
    lets: &[String],
) -> std::collections::HashSet<String> {
    candidates
        .iter()
        .filter(|name| {
            let mut places = Vec::new();
            where_is(nodes, name, children, false, &mut places);
            let own: Vec<&Place> = places.iter().filter(|l| **l != Place::Embedded).collect();
            matches!(
                own.as_slice(),
                [Place::Root | Place::OnChild | Place::Projected | Place::OnProjectedChild]
            ) && !lets.contains(name)
                && referenced_in_scope(nodes, name, children)
        })
        .cloned()
        .collect()
}

/// `name` is read by some expression of this view or of an embedded one in it
/// that does not shadow it (a `let` of it, or a `#ref` declared in it). The
/// bindings of the `*` are evaluated in the outer view; its `let`s are not
/// reads.
fn referenced_in_scope(
    nodes: &[Node],
    name: &str,
    children: &std::collections::HashMap<String, Child>,
) -> bool {
    let refers = |text: &str| references_at_root(text, name);
    nodes.iter().any(|n| match n {
        Node::Interpolation { expr, .. } => refers(expr),
        Node::Element(e) => match &e.star {
            Some(star) if star.name != TEMPLATE_MARK => {
                let micro = e.star_micro().unwrap_or_default();
                micro.properties.iter().any(|(_, expr)| refers(expr))
                    || referenced_in_embedded(e, name, children)
            }
            Some(_) => element_references(e, name) || referenced_in_embedded(e, name, children),
            None => element_references(e, name) || referenced_in_scope(&e.children, name, children),
        },
        _ => false,
    })
}

/// `name` is read in this view's detection — a binding, an interpolation, the
/// `*` of one of its elements —, not counting events nor the embedded views.
fn read_in_detection(nodes: &[Node], name: &str) -> bool {
    let refers = |text: &str| references_at_root(text, name);
    let on_element = |e: &crate::html::Element| {
        e.properties
            .iter()
            .chain(e.bananas.iter())
            .any(|l| refers(&l.value))
            || e.attributes
                .iter()
                .any(|a| a.value.contains("{{") && refers(&a.value))
    };
    nodes.iter().any(|n| match n {
        Node::Interpolation { expr, .. } => refers(expr),
        Node::Element(e) => match &e.star {
            Some(star) if star.name != TEMPLATE_MARK => {
                let micro = e.star_micro().unwrap_or_default();
                micro.properties.iter().any(|(_, expr)| refers(expr))
            }
            Some(_) => on_element(e),
            None => on_element(e) || read_in_detection(&e.children, name),
        },
        _ => false,
    })
}

/// Does any binding of the element itself read `name` with the implicit
/// receiver?
fn element_references(e: &crate::html::Element, name: &str) -> bool {
    let refers = |text: &str| references_at_root(text, name);
    e.properties
        .iter()
        .chain(e.events.iter())
        .chain(e.bananas.iter())
        .any(|l| refers(&l.value))
        || e.attributes
            .iter()
            .any(|a| a.value.contains("{{") && refers(&a.value))
}

/// Does the embedded view of the `*` (or of the written `<template>`) `e` read
/// the outer `name`? Not if it shadows it: a `let` of it or a `#ref` declared
/// in it.
fn referenced_in_embedded(
    e: &crate::html::Element,
    name: &str,
    children: &std::collections::HashMap<String, Child>,
) -> bool {
    let has_ref = |x: &crate::html::Element| x.references.iter().any(|r| r.name == name);
    let mut places = Vec::new();
    match &e.star {
        Some(star) if star.name != TEMPLATE_MARK => {
            let micro = e.star_micro().unwrap_or_default();
            // The element of the `*` and what is below it belong to the
            // embedded view.
            where_matches(
                &[Node::Element(crate::html::Element {
                    star: None,
                    ..e.clone()
                })],
                &has_ref,
                children,
                false,
                &mut places,
            );
            let shadowed = micro.locals.iter().any(|(l, _)| l == name)
                || places.iter().any(|l| *l != Place::Embedded);
            !shadowed
                && (element_references(e, name) || referenced_in_scope(&e.children, name, children))
        }
        _ => {
            // Written `<template>`: the content is the embedded view; the
            // `let-x` stay in the element's attributes.
            where_matches(&e.children, &has_ref, children, false, &mut places);
            let lets = e
                .attributes
                .iter()
                .any(|a| a.name.strip_prefix("let-") == Some(name));
            let shadowed = lets || places.iter().any(|l| *l != Place::Embedded);
            !shadowed && referenced_in_scope(&e.children, name, children)
        }
    }
}

/// The fields created by the binder, in its order: the `_expr_k` of the
/// bindings and, among them, the node that an embedded view reads, at the
/// position where it was bound ([`ViewBody::embedded_position`]; without a
/// position, after all of them).
fn detection_fields(
    exprs: &[String],
    from_embedded: &[(String, String)],
    positions: &std::collections::HashMap<String, usize>,
) -> Vec<String> {
    let position = |name: &str| positions.get(name).copied().unwrap_or(exprs.len());
    let mut out = Vec::with_capacity(exprs.len() + from_embedded.len());
    for k in 0..=exprs.len() {
        out.extend(
            from_embedded
                .iter()
                .filter(|(n, _)| position(n) == k)
                .map(|(_, c)| c.clone()),
        );
        if let Some(e) = exprs.get(k) {
            out.push(e.clone());
        }
    }
    out
}

/// `name` is read only from inside a view embedded in this one (not counting
/// the expressions of the view itself): the official compiler promotes the
/// node when it compiles the embedded view, before this one's bindings (the
/// field comes before the `_expr_k`).
fn referenced_in_embedded_views(
    nodes: &[Node],
    name: &str,
    children: &std::collections::HashMap<String, Child>,
) -> bool {
    nodes.iter().any(|n| match n {
        Node::Element(e) if e.star.is_some() => referenced_in_embedded(e, name, children),
        Node::Element(e) => referenced_in_embedded_views(&e.children, name, children),
        _ => false,
    })
}

/// The key of a `#ref` read as a local in the mark and in the map of resolved
/// ones: the name and the view that declares it — the same name may belong to
/// different views (each one resolves its own, [`local_refs`]).
fn ref_key(name: &str, view_class: &str) -> String {
    format!("{name}@{view_class}")
}

/// The `k`-th result of `key` in a view (repeated `#ref`s, children of the
/// same type): the mark that [`export_refs`] records in the order the nodes
/// were seen.
fn ordinal_key(key: &str, k: usize) -> String {
    format!("{key}\u{3}{k}")
}

/// The `#ref`s of a finished view in the map of resolved ones: by the key with
/// the view ([`ref_key`]) and by the name (that of the queries, which
/// read the node of a nested view by name).
fn export_refs(
    resolved: &std::cell::RefCell<std::collections::HashMap<String, String>>,
    refs: &std::collections::HashMap<String, String>,
    detectors: &std::collections::HashMap<String, String>,
    in_order: &[(String, String)],
    detectors_in_order: &[(String, String)],
    view_class: &str,
) {
    let mut r = resolved.borrow_mut();
    for (name, read) in refs {
        r.insert(ref_key(name, view_class), read.clone());
        r.insert(name.clone(), read.clone());
    }
    let mut seen: std::collections::HashMap<&str, usize> = Default::default();
    for (name, read) in in_order {
        let k = seen.entry(name.as_str()).or_default();
        r.insert(ref_key(&ordinal_key(name, *k), view_class), read.clone());
        *k += 1;
    }
    let mut seen: std::collections::HashMap<&str, usize> = Default::default();
    for (key, view) in detectors_in_order {
        let k = seen.entry(key.as_str()).or_default();
        r.insert(
            format!(
                "{DETECTOR_MARK}{}",
                ref_key(&ordinal_key(key, *k), view_class)
            ),
            format!("this.{view}"),
        );
        *k += 1;
    }
    for (key, view) in detectors {
        r.insert(format!("{DETECTOR_MARK}{key}"), format!("this.{view}"));
        r.insert(
            format!("{DETECTOR_MARK}{}", ref_key(key, view_class)),
            format!("this.{view}"),
        );
    }
}

/// Replaces the `#ref` marks ([`REF_MARK`]) with each one's node:
/// `\u{5}x\u{6}` with the read in the view itself (`this._el_3`), and
/// `\u{5}.x\u{6}` with just the field (`_el_3`), read from a nested view. The
/// mark of a node not seen yet stays (the view that creates it comes later).
fn resolver_refs(text: &str, refs: &std::collections::HashMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(REF_MARK) {
        out.push_str(&rest[..i]);
        let after = &rest[i + REF_MARK.len_utf8()..];
        let f = after.find(REF_END).unwrap_or(after.len());
        let name = &after[..f];
        match (
            name.strip_prefix('.'),
            refs.get(name.trim_start_matches('.')),
        ) {
            (Some(_), Some(read)) => {
                out.push('.');
                out.push_str(read.strip_prefix("this.").unwrap_or(read));
            }
            (None, Some(read)) => out.push_str(read),
            (_, None) => {
                out.push(REF_MARK);
                out.push_str(name);
                out.push(REF_END);
            }
        }
        rest = after.get(f + REF_END.len_utf8()..).unwrap_or("");
    }
    out.push_str(rest);
    out
}

/// The component shapes that can only be decided by looking at the template:
/// where the `#ref` of each `@ViewChild` is. Returns what
/// still prevents generation, in the order it appears.
fn shapes_against_template(
    c: &Component,
    local: &Local,
    nodes: &[Node],
    resolver: Option<&dyn Resolution>,
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
) -> Vec<Refusal> {
    let mut refused = Vec::new();
    let templates = template_refs(nodes);
    for query in &c.queries {
        let mut places = Vec::new();
        // `read:` in the static query of an element of the view (below) or in
        // that of an element in `*` ([`query_in_embedded`]): by type, a
        // static list or `<template>` not yet.
        let from_provider = !query.by_type
            && !templates.contains(&query.reference)
            && read_token(query, local, resolver).is_some();
        // `read: ViewContainerRef` on a `<template #x>`: the value is the
        // node's `ViewContainer`, assigned in `build()` — the static shape (a
        // single `#x`, in the view itself, `@ViewChild`).
        if !query.by_type
            && templates.contains(&query.reference)
            && reads_container(query, local, resolver)
        {
            where_is(nodes, &query.reference, children, false, &mut places);
            if query.list || places.as_slice() != [Place::Root] {
                refused.push(refusal(
                    Reason::DynamicViewChild,
                    "@ViewChild(.., read: ViewContainerRef) of <template> outside the static shape",
                ));
            }
            continue;
        }
        if query.read.is_some()
            && !from_provider
            && !query_in_embedded(nodes, query, children, local, resolver)
            && (query.by_type || query.list || templates.contains(&query.reference))
        {
            refused.push(refusal(
                Reason::ViewChildOnChild,
                "@ViewChild(.., read: T) outside a static element",
            ));
            continue;
        }
        if query.by_type {
            // `@ViewChild(Type)`: the instances of the children of that class.
            // The attribute directive of the same type would also be a result:
            // not yet.
            let Some(uri) = query_uri(query, local, resolver) else {
                refused.push(refusal(
                    Reason::ViewChildOnChild,
                    "@ViewChild(Type) without resolution",
                ));
                continue;
            };
            if query_in_embedded(nodes, query, children, local, resolver) {
                continue;
            }
            if dynamic_token_query(nodes, query, children, used, local, resolver).is_some() {
                continue;
            }
            // `@ViewChild(Directive)`: its token is in the
            // `_resolvedProvidersArray` of the element it matches, and the
            // value is its field (`_providers.get(type).build()`, with the
            // `.instance` of an `XNgCd`), assigned at the end of `build()`
            // when the result is in the view itself (case j117).
            if let Some(u) = used
                .iter()
                .find(|u| u.child.is_none() && u.uri == uri && u.class == query.reference)
            {
                let matcher = |e: &crate::html::Element| {
                    !children.contains_key(&e.name)
                        && crate::selector::matches_any(
                            &u.selectors,
                            &crate::selector::Element::of_template(e),
                        )
                };
                where_matches(nodes, &matcher, children, false, &mut places);
                let in_view = !places.is_empty()
                    && places
                        .iter()
                        .all(|l| matches!(l, Place::Root | Place::Projected));
                if query.list || !in_view || u.directive.is_none() {
                    refused.push(refusal(
                        Reason::ViewChildOnChild,
                        "@ViewChild(Directive) outside the static shape",
                    ));
                }
                continue;
            }
            let is_child = |e: &crate::html::Element| {
                children
                    .get(&e.name)
                    .is_some_and(|f| f.uri_dart == uri && f.class == query.reference)
            };
            where_matches(nodes, &is_child, children, false, &mut places);
            let only_children = places
                .iter()
                .all(|l| matches!(l, Place::OnChild | Place::OnProjectedChild));
            let element = is_element_type(&query.ty, local, resolver);
            // The type has to be a component from `directives:`: a directive,
            // service or `ElementRef` would give other results.
            let is_component = children
                .values()
                .any(|f| f.uri_dart == uri && f.class == query.reference);
            if !is_component || !only_children || element || (places.is_empty() && !query.list) {
                refused.push(refusal(
                    Reason::ViewChildOnChild,
                    "@ViewChild(Type) outside the static shape",
                ));
            }
            continue;
        }
        where_is(nodes, &query.reference, children, false, &mut places);
        // `#ref="x"`: the value is the instance of the exported directive (what
        // the `#ref` reads in the refs map, `this._X_n_m`), assigned like that
        // of an element — only in the static shape: a single `#ref`, in the
        // view itself (also in a child's projected content, case j69),
        // `@ViewChild` (not a list).
        if ref_with_value(nodes, &query.reference) {
            // Inside `*`: the exported provider is the result recorded in the
            // nested view (`compile_element.dart:254-262`), read by
            // `mapNestedViews` like the node of an element (§A5).
            if query_in_embedded(nodes, query, children, local, resolver) {
                continue;
            }
            // On a child's element, the instance of the exported directive is
            // read the same way (case j109).
            if !(matches!(
                places.as_slice(),
                [Place::Root | Place::Projected | Place::OnChild | Place::OnProjectedChild]
            ) && !query.list)
            {
                refused.push(refusal(
                    Reason::ViewChildOnChild,
                    "@ViewChild of #ref with a value (exportAs) outside the static shape",
                ));
            }
            continue;
        }
        // `@ViewChild('t')` of a written `<template #t>` ([`ViewBody::hand_template`]): the
        // value read is the node's `TemplateRef` (the implicit `read` of a
        // `<template>`), assigned in `build()` like that of an element
        // (`_ctx.x = this._TemplateRef_n_7;`). Only the static shape — a
        // single `#t`, in the view itself, an ngdart `TemplateRef` field; a
        // list, a result in `*` or in projected content not yet.
        if templates.contains(&query.reference) {
            match places.as_slice() {
                [Place::Root] if !query.list && is_template_ref(&query.ty, local, resolver) => {}
                [Place::Root] if !query.list => refused.push(refusal(
                    Reason::ViewChildOnChild,
                    "@ViewChild of <template> in a field that is not TemplateRef",
                )),
                _ => refused.push(refusal(
                    Reason::DynamicViewChild,
                    "@ViewChild of <template> outside the static shape",
                )),
            }
            continue;
        }
        // Static `@ViewChildren`, or `@ViewChild` of a repeated `#ref` (the
        // first result): all results in this view, and of the same kind (nodes
        // with an `Element` field, or child instances with a field of its
        // type). Without a result, the list receives `[]`. A result in `*` not
        // yet.
        if query_in_embedded(nodes, query, children, local, resolver) {
            continue;
        }
        // The single query whose first result (in pre-order) is in this view
        // is static and keeps it (`_isStatic`, `_buildQueryResults` stops at
        // the first): those inside `*` that come later do not count (j17).
        if !query.list && places.len() > 1 && places[0] != Place::Embedded {
            places.truncate(1);
        }
        if query.list || places.len() > 1 {
            let element = is_element_type(&query.ty, local, resolver);
            let all_nodes = places
                .iter()
                .all(|l| matches!(l, Place::Root | Place::Projected));
            let all_children = places
                .iter()
                .all(|l| matches!(l, Place::OnChild | Place::OnProjectedChild));
            let ok = (from_provider && all_nodes)
                || (!from_provider && element && all_nodes)
                || (!from_provider && !element && all_children);
            if !ok {
                refused.push(refusal(
                    Reason::DynamicViewChild,
                    "@ViewChildren outside the static shape",
                ));
            }
            continue;
        }
        let r = match places.as_slice() {
            // The node or `ElementRef(node)`, according to the read
            // ([`element_value`]): the static query, which comes out in
            // `build()`.
            [Place::Root | Place::Projected]
                if element_value(query, local, resolver).is_some() || from_provider =>
            {
                continue;
            }
            [Place::Root | Place::Projected] => refusal(
                Reason::ViewChildOnChild,
                "@ViewChild(.., read: T) of a token the element does not provide",
            ),
            // The child's instance: the field has to be of its type (not an
            // `Element`) and the child cannot be `onPush`, which registers the
            // query's `ChangeDetectorRef` (`queryChangeDetectorRefs`).
            [Place::OnChild | Place::OnProjectedChild] => {
                if query.read.is_some() {
                    refusal(
                        Reason::ViewChildOnChild,
                        "@ViewChild(.., read: T) of a child's #ref",
                    )
                } else if is_element_type(&query.ty, local, resolver) {
                    refusal(
                        Reason::ViewChildOnChild,
                        "@ViewChild of type Element on a child's #ref",
                    )
                } else {
                    continue;
                }
            }
            [Place::Child] => refusal(
                Reason::ViewChildOnChild,
                "@ViewChild of #ref in a child component",
            ),
            // Inside `*` the query goes through `mapNestedViews`; without a
            // result, or with two, the rule is different — none of that yet.
            // Without a result, the single query receives nothing
            // (`_createUpdates`, case j82).
            [] => continue,
            [_] => refusal(
                Reason::DynamicViewChild,
                "@ViewChild of #ref in an embedded view",
            ),
            _ => refusal(Reason::DynamicViewChild, "@ViewChild of a repeated #ref"),
        };
        refused.push(r);
    }
    refused
}
/// A view query result in the tree of `compile_query.dart`
/// (`_NestedQueryValues`): a node of this view, or the results from inside a
/// `*` of it — in pre-order, which is the order in which `CompileElement`
/// calls `addQueryResult` (in `beforeChildren`, with the embedded view built
/// on the spot).
#[derive(Debug, Clone)]
enum QueryItem {
    /// The node of the `#ref` (HTML element) or the child's instance;
    /// `on_push`: the child registers the `ChangeDetectorRef`
    /// (`buildChangeDetectorRef`).
    Value {
        place: Place,
        on_push: bool,
        /// How many results with the same key come before it in the same view
        /// (the node is read by [`ordinal_key`]).
        ordinal: usize,
    },
    /// The results of the embedded view of the `*` that starts at `star` (the
    /// start of its binding, which identifies the `*` in the template).
    Nested { star: usize, items: Vec<QueryItem> },
}

/// The tree of the results of `key` in `nodes` ([`QueryItem`]). The `*` is
/// followed through the content of HTML elements and of `<ng-container>`,
/// also in a child's projected content; a result on a tag directive or inside
/// a written `<template>` is not translated yet.
fn query_tree(
    nodes: &[Node],
    key: &str,
    children: &std::collections::HashMap<String, Child>,
) -> Result<Vec<QueryItem>, &'static str> {
    fn walk(
        nodes: &[Node],
        key: &str,
        children: &std::collections::HashMap<String, Child>,
        in_child: bool,
        out: &mut Vec<QueryItem>,
        seen: &mut usize,
    ) -> Result<(), &'static str> {
        for n in nodes {
            let Node::Element(e) = n else { continue };
            if let Some(star) = &e.star {
                let mut inside = Vec::new();
                if star.name == TEMPLATE_MARK {
                    // The written `<template>` belongs to this view; its content,
                    // to its embedded view, mapped like that of a `*`
                    // (`_appEl_n.mapNestedViews`, case j86). The `<template>`
                    // itself as a result is not translated yet.
                    if matches_key(e, key, children) {
                        return Err("query result on a written <template>");
                    }
                    walk(&e.children, key, children, false, &mut inside, &mut 0)?;
                    if !inside.is_empty() {
                        out.push(QueryItem::Nested {
                            star: star.start,
                            items: inside,
                        });
                    }
                    continue;
                }
                let mut without = e.clone();
                without.star = None;
                walk(
                    std::slice::from_ref(&Node::Element(without)),
                    key,
                    children,
                    false,
                    &mut inside,
                    &mut 0,
                )?;
                // The `*` in a child's projected content belongs to this view
                // (the anchor is from here; only the node is projected): the
                // query maps the embedded view the same way (case j77).
                if !inside.is_empty() {
                    out.push(QueryItem::Nested {
                        star: star.start,
                        items: inside,
                    });
                }
                continue;
            }
            let (place, below) = if let Some(f) = children.get(&e.name) {
                let place = if in_child {
                    Place::OnProjectedChild
                } else {
                    Place::OnChild
                };
                if matches_key(e, key, children) {
                    out.push(QueryItem::Value {
                        place,
                        on_push: f.on_push,
                        ordinal: *seen,
                    });
                    *seen += 1;
                }
                (place, true)
            } else if e.name == "ng-container" {
                (Place::Root, in_child)
            } else if !dom::tag_html(&e.name) {
                (Place::Child, true)
            } else if in_child {
                (Place::Projected, true)
            } else {
                (Place::Root, false)
            };
            if !children.contains_key(&e.name) && matches_key(e, key, children) {
                out.push(QueryItem::Value {
                    place,
                    on_push: false,
                    ordinal: *seen,
                });
                *seen += 1;
            }
            walk(&e.children, key, children, below, out, seen)?;
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(nodes, key, children, false, &mut out, &mut 0)?;
    Ok(out)
}

/// The query needs `mapNestedViews` (`_shouldMapNestedViews`): the single
/// query when the first result is inside a `*`; the list when any is.
/// Otherwise it is static (`createImmediateUpdates`).
fn query_is_dynamic(tree: &[QueryItem], list: bool) -> bool {
    if list {
        tree.iter().any(|i| matches!(i, QueryItem::Nested { .. }))
    } else {
        matches!(tree.first(), Some(QueryItem::Nested { .. }))
    }
}

/// Is there a result in this view, outside `*` (`hasStaticValues`)?
fn has_static_result(items: &[QueryItem]) -> bool {
    items.iter().any(|i| matches!(i, QueryItem::Value { .. }))
}

/// Is any result an `onPush` component?
fn has_detector(items: &[QueryItem]) -> bool {
    items.iter().any(|i| match i {
        QueryItem::Value { on_push, .. } => *on_push,
        QueryItem::Nested { items, .. } => has_detector(items),
    })
}

/// The view query with results inside `*` that is translated through the tree
/// ([`QueryItem`]): each result is an HTML element (an `Element` field, or
/// read with `read: ElementRef`/`Element`) or a child's instance, and each
/// view has at most one (the node is read by the name of the `#ref` in that
/// view). It is the shape of `mapNestedViews` and
/// `mapNestedViewsWithSingleResult`, at any depth, with several `*` and with
/// static results alongside (i47, i96, i97, j06, j09, j10, j14, j16, j17,
/// j34).
fn query_in_embedded(
    nodes: &[Node],
    query: &crate::component::Query,
    children: &std::collections::HashMap<String, Child>,
    local: &Local,
    resolver: Option<&dyn Resolution>,
) -> bool {
    let Some(key) = query_key(query, local, resolver) else {
        return false;
    };
    let Ok(tree) = query_tree(nodes, &key, children) else {
        return false;
    };
    if !query_is_dynamic(&tree, query.list) {
        return false;
    }
    let element = is_element_type(&query.ty, local, resolver);
    let read = query.read.is_some();
    // `#ref="exportAs"`: the value is the exported instance (the `#ref` reads
    // it in the refs map), in the single query without `read:` (§A5).
    let exported = !query.by_type && ref_with_value(nodes, &key);
    let terminal = |l: Place| match l {
        _ if exported => !read && !query.list && matches!(l, Place::Root | Place::Projected),
        Place::Root if read => {
            !query.by_type
                && (element_value(query, local, resolver).is_some()
                    || read_token(query, local, resolver).is_some())
        }
        Place::Root => element && !query.by_type,
        // The child in the content projected into another is also created in
        // this view (the instance, like that of the child on the node itself).
        Place::OnChild | Place::OnProjectedChild => !read && !element,
        _ => false,
    };
    fn validate(items: &[QueryItem], terminal: &dyn Fn(Place) -> bool) -> bool {
        items.iter().all(|i| match i {
            QueryItem::Value { place, .. } => terminal(*place),
            QueryItem::Nested { items, .. } => validate(items, terminal),
        })
    }
    validate(&tree, &terminal)
}

/// The anchors of each `*` that contains a dynamic query result, by the start
/// of the star's binding: (`_appEl_n`, class `_ViewX1` of the embedded view),
/// recorded when the `*` is emitted.
type Anchors = std::collections::HashMap<usize, (String, String)>;

/// Per embedded view (the start of the star), the content queries whose result
/// is in it: (position of the first result, dirty field, how many
/// `parentView` up to the query's view, class of that view).
type DirtyContentQueries = std::collections::HashMap<usize, Vec<(usize, u8, String, u32, String)>>;

/// A content query result: the node that provides the token (by position,
/// [`node_key`]) or an embedded view with results.
#[derive(Debug, Clone)]
enum ContentItem {
    Value {
        start: usize,
        /// The node's `onPush` component is the result: its
        /// `ChangeDetectorRef` goes to `View.queryChangeDetectorRefs`
        /// (`buildChangeDetectorRef`, `compile_element.dart:390-418`).
        on_push: bool,
    },
    Nested {
        star: usize,
        items: Vec<ContentItem>,
    },
}

/// A content query updated during detection (`createDynamicUpdates`), built
/// when the anchors of all views are known.
#[derive(Debug, Clone)]
struct DynamicContent {
    id: usize,
    tree: Vec<ContentItem>,
    /// [`type_key`] of the token.
    key: String,
    list: bool,
    /// `this._FocusListDirective_0_5.instance.listItems`.
    target: String,
    /// The class of the query's view.
    class: String,
}

/// `@ViewChild(ren)(Token)` whose token a directive (or its `providers:`)
/// provides, with a result in `*` (`_shouldMapNestedViews`): the tree of the
/// results ([`token_tree`]), for the content-query machinery with the
/// root in the component's view. `None` for the rest (the child of the class
/// itself goes the usual way, [`query_in_embedded`]).
fn dynamic_token_query(
    nodes: &[Node],
    query: &crate::component::Query,
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
    local: &Local,
    resolver: Option<&dyn Resolution>,
) -> Option<Vec<ContentItem>> {
    if !query.by_type || query.read.is_some() {
        return None;
    }
    let uri = query_uri(query, local, resolver)?;
    let class = query.reference.rsplit('.').next()?.to_string();
    if children
        .values()
        .any(|f| f.uri_dart == uri && f.class == class)
    {
        return None;
    }
    let token = crate::directives::Token::Class {
        uri: uri.clone(),
        class: class.clone(),
    };
    let provides = |d: &crate::directives::Directive| {
        (d.uri == uri && d.class == class) || d.providers.iter().any(|p| p.token == token)
    };
    let anyone = used
        .iter()
        .any(|u| u.directive.as_deref().is_some_and(provides))
        || children
            .values()
            .any(|f| f.metadata.as_deref().is_some_and(provides));
    if !anyone {
        return None;
    }
    let tree = token_tree(nodes, &uri, &class, children, used).ok()?;
    let dynamic = if query.list {
        tree.iter().any(|i| matches!(i, ContentItem::Nested { .. }))
    } else {
        matches!(tree.first(), Some(ContentItem::Nested { .. }))
    };
    dynamic.then_some(tree)
}

fn token_tree(
    nodes: &[Node],
    uri: &str,
    class: &str,
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
) -> Result<Vec<ContentItem>, Refusal> {
    let token = crate::directives::Token::Class {
        uri: uri.to_string(),
        class: class.to_string(),
    };
    let provides = |d: &crate::directives::Directive| {
        (d.uri == uri && d.class == class) || d.providers.iter().any(|p| p.token == token)
    };
    let mut out = Vec::new();
    for node in nodes {
        let Node::Element(x) = node else { continue };
        if let Some(star) = &x.star {
            let mut without = x.clone();
            if star.name != TEMPLATE_MARK {
                without.star = None;
                let inside = token_tree(
                    std::slice::from_ref(&Node::Element(without)),
                    uri,
                    class,
                    children,
                    used,
                )?;
                if !inside.is_empty() {
                    out.push(ContentItem::Nested {
                        star: star.start,
                        items: inside,
                    });
                }
                continue;
            }
            // Written `<template>`: the node belongs to this view; the
            // content, to its embedded view.
            without.star = None;
            if matched_directives(used, &without)
                .iter()
                .any(|d| provides(d))
            {
                return Err(refusal(
                    Reason::BindingOnChild,
                    "@ContentChildren with a result on the <template> itself",
                ));
            }
            let inside = token_tree(&x.children, uri, class, children, used)?;
            if !inside.is_empty() {
                out.push(ContentItem::Nested {
                    star: star.start,
                    items: inside,
                });
            }
            continue;
        }
        let child = children.get(&x.name);
        let matched = child.is_some_and(|f| {
            (f.uri_dart == uri && f.class == class)
                || f.metadata.as_deref().is_some_and(|m| provides(m))
        }) || matched_directives(used, x).iter().any(|d| provides(d));
        if matched {
            let on_push = child.is_some_and(|f| f.on_push && f.uri_dart == uri && f.class == class);
            out.push(ContentItem::Value {
                start: x.start,
                on_push,
            });
        }
        out.extend(token_tree(&x.children, uri, class, children, used)?);
    }
    Ok(out)
}

/// The key of the node at `start` that provides the token of `key`: the result
/// of a content query read from another view.
fn node_key(key: &str, start: usize) -> String {
    format!("{key}\u{3}@{start}")
}

const CONTENT_MARK: char = '\u{12}';
const CONTENT_END: char = '\u{13}';

/// Mark of the update of a dynamic view query during detection:
/// `\u{e}field|q..|d..|e..\u{f}`, replaced by [`resolve_queries`] when the
/// anchors of all views are known. The qualifiers (`firstOrNull` of
/// `queries.dart`, `View`, `ElementRef`) go in the mark so that they are
/// allocated in text order by [`resolve_deferred`].
const QUERY_MARK: char = '\u{e}';
const QUERY_END: char = '\u{f}';

/// Builds the value of a dynamic query like `compile_query.dart`:
/// `_buildQueryResults` (the single query stops at the first static result of
/// each view; with more than one item, `mapNestedViews` is spread with
/// `...`), `_mapNestedViews` (`mapNestedViewsWithSingleResult` when the view
/// gives a single result; the `ChangeDetectorRef`s of the `onPush` ones before
/// the `return`) and `_createUpdatesNested` (a list when there is more than
/// one value; the single query takes `.first` if there is a static result,
/// otherwise `firstOrNull`). The text comes out as `DartFormatter` leaves it:
/// each closure with its body two spaces in from the line where it opens, each
/// list item on its own line.
struct QueryAssembly<'a> {
    anchors: &'a Anchors,
    key: &'a str,
    list: bool,
    element_ref: Option<&'a str>,
    view: Option<&'a str>,
}

impl QueryAssembly<'_> {
    /// The values and the `ChangeDetectorRef` registrations of a view:
    /// `receiver` is `this` in the component's view and `nestedView` inside a
    /// closure; `class`, that of the view (the node's mark is `name@class`).
    fn results(
        &self,
        items: &[QueryItem],
        receiver: &str,
        class: &str,
    ) -> Option<(Vec<String>, Vec<String>)> {
        let (mut values, mut registrations) = (Vec::new(), Vec::new());
        let read = |mark: &str| {
            if receiver == "this" {
                format!("{REF_MARK}{mark}{REF_END}")
            } else {
                format!("{receiver}{REF_MARK}.{mark}{REF_END}")
            }
        };
        for item in items {
            match item {
                QueryItem::Value {
                    on_push, ordinal, ..
                } => {
                    let key = ordinal_key(self.key, *ordinal);
                    let node = read(&ref_key(&key, class));
                    let value = match self.element_ref {
                        Some(q) => format!("{q}ElementRef({node})"),
                        None => node,
                    };
                    if *on_push {
                        let v = self.view?;
                        let view = read(&format!("{DETECTOR_MARK}{}", ref_key(&key, class)));
                        registrations.push(format!(
                            "{v}View.queryChangeDetectorRefs[{value}] = {view};"
                        ));
                    }
                    values.push(value);
                    if !self.list {
                        break;
                    }
                }
                QueryItem::Nested {
                    star,
                    items: inside,
                } => {
                    let map = self.map(*star, inside, receiver)?;
                    values.push(if items.len() > 1 {
                        format!("...{map}")
                    } else {
                        map
                    });
                }
            }
        }
        Some((values, registrations))
    }

    fn map(&self, star: usize, inside: &[QueryItem], receiver: &str) -> Option<String> {
        let (anchor, class) = self.anchors.get(&star)?;
        let (values, mut lines) = self.results(inside, "nestedView", class)?;
        let several = if self.list {
            inside.len() > 1 || inside.iter().any(|i| matches!(i, QueryItem::Nested { .. }))
        } else {
            matches!(inside.first(), Some(QueryItem::Nested { .. }))
        };
        let method = if several {
            "mapNestedViews"
        } else {
            "mapNestedViewsWithSingleResult"
        };
        let body = if values.len() > 1 {
            list_literal(&values)
        } else {
            values.into_iter().next()?
        };
        lines.push(format!("return {body};"));
        Some(format!(
            "{receiver}.{anchor}.{method}(({class} nestedView) {{\n{}\n}})",
            indent_lines(&lines.join("\n"), 2)
        ))
    }
}

/// `[a, b]`: on one line, as `DartFormatter` (an "infinite" page) leaves it,
/// unless some item has a line break (a closure) — then one item per line.
fn list_literal(values: &[String]) -> String {
    if !values.iter().any(|v| v.contains('\n')) {
        return format!("[{}]", values.join(", "));
    }
    let items: Vec<String> = values.iter().map(|v| indent_lines(v, 2)).collect();
    format!("[\n{}\n]", items.join(",\n"))
}

/// Replaces each [`QUERY_MARK`] with the statements of the query
/// update, with the indentation of the line the mark is on.
fn resolve_queries(
    text: &str,
    dynamic_queries: &[DynamicQuery],
    anchors: &Anchors,
    root_class: &str,
) -> String {
    let mut out = text.to_string();
    for q in dynamic_queries {
        let start = format!("{QUERY_MARK}{}", q.field);
        while let Some(i) = out.find(&start) {
            let after = &out[i + start.len()..];
            let Some(f) = after.find(QUERY_END) else {
                break;
            };
            // The field ends in `_isDirty`: no other one has it as a prefix.
            let mut qualifiers = std::collections::HashMap::new();
            for part in after[..f].split('|').skip(1) {
                let (k, v) = part.split_at(1);
                qualifiers.insert(k.to_string(), v.to_string());
            }
            let assembly = QueryAssembly {
                anchors,
                key: &q.read,
                list: q.list,
                element_ref: qualifiers.get("e").map(String::as_str),
                view: qualifiers.get("d").map(String::as_str),
            };
            let Some((values, mut lines)) = assembly.results(&q.tree, "this", root_class) else {
                break;
            };
            let value = if values.len() == 1 {
                values[0].clone()
            } else {
                list_literal(&values)
            };
            let value = match (q.list, qualifiers.get("q")) {
                (true, _) => value,
                (false, Some(queries)) => format!("{queries}firstOrNull({value})"),
                (false, None) => format!("{value}.first"),
            };
            lines.push(format!("_ctx.{} = {value};", q.property));
            let line_start = out[..i].rfind('\n').map_or(0, |k| k + 1);
            let indent = out[line_start..i].len() - out[line_start..i].trim_start().len();
            let text = lines.join("\n");
            let text = indent_lines(&text, indent);
            let end = i + start.len() + f + QUERY_END.len_utf8();
            out.replace_range(i..end, text.trim_start());
        }
    }
    out
}

/// Replaces each [`CONTENT_MARK`] with the assignment of the content
/// query: `_buildQueryResults`/`_mapNestedViews`/`_createUpdatesNested`
/// (`compile_query.dart:200-530`) over its tree, with the indentation of the
/// mark's line. The nodes come out as `#ref` marks ([`node_key`]),
/// replaced later by [`resolver_refs`].
fn resolve_dynamic_content(
    text: &str,
    dynamic_queries: &[DynamicContent],
    anchors: &Anchors,
) -> String {
    /// The values and the `ChangeDetectorRef` registrations (`view`: the
    /// qualified `View`) of a view, like [`QueryAssembly::results`].
    fn results(
        q: &DynamicContent,
        items: &[ContentItem],
        receiver: &str,
        class: &str,
        anchors: &Anchors,
        view: Option<&str>,
    ) -> Option<(Vec<String>, Vec<String>)> {
        let (mut values, mut registrations) = (Vec::new(), Vec::new());
        let read = |mark: &str| {
            if receiver == "this" {
                format!("{REF_MARK}{mark}{REF_END}")
            } else {
                format!("{receiver}{REF_MARK}.{mark}{REF_END}")
            }
        };
        for i in items {
            match i {
                ContentItem::Value { start, on_push } => {
                    let key = ref_key(&node_key(&q.key, *start), class);
                    let value = read(&key);
                    if *on_push {
                        let detector_view = read(&format!("{DETECTOR_MARK}{key}"));
                        registrations.push(format!(
                            "{}View.queryChangeDetectorRefs[{value}] = {detector_view};",
                            view?
                        ));
                    }
                    values.push(value);
                    // The single query stops at the view's first static result.
                    if !q.list {
                        break;
                    }
                }
                ContentItem::Nested {
                    star,
                    items: inside,
                } => {
                    let (anchor, class_w) = anchors.get(star)?;
                    let (vals, mut lines) =
                        results(q, inside, "nestedView", class_w, anchors, view)?;
                    let several = if q.list {
                        inside.len() > 1
                            || inside
                                .iter()
                                .any(|x| matches!(x, ContentItem::Nested { .. }))
                    } else {
                        matches!(inside.first(), Some(ContentItem::Nested { .. }))
                    };
                    let method = if several {
                        "mapNestedViews"
                    } else {
                        "mapNestedViewsWithSingleResult"
                    };
                    let body = if vals.len() > 1 {
                        list_literal(&vals)
                    } else {
                        vals.into_iter().next()?
                    };
                    lines.push(format!("return {body};"));
                    let map = format!(
                        "{receiver}.{anchor}.{method}(({class_w} nestedView) {{\n{}\n}})",
                        indent_lines(&lines.join("\n"), 2)
                    );
                    values.push(if items.len() > 1 {
                        format!("...{map}")
                    } else {
                        map
                    });
                }
            }
        }
        Some((values, registrations))
    }
    let mut out = text.to_string();
    for q in dynamic_queries {
        let start = format!("{CONTENT_MARK}{}", q.id);
        while let Some(i) = out.find(&start) {
            let after = &out[i + start.len()..];
            let Some(f) = after.find(CONTENT_END) else {
                break;
            };
            // The id ends at the mark or at the `|`: no other one has it as a
            // prefix.
            let rest = &after[..f];
            if !(rest.is_empty() || rest.starts_with('|')) {
                break;
            }
            // `|q<queries>` and `|d<View>`, each one only when used.
            let mut qualifiers = std::collections::HashMap::new();
            for part in rest.split('|').skip(1) {
                if !part.is_empty() {
                    let (k, v) = part.split_at(1);
                    qualifiers.insert(k, v);
                }
            }
            let queries = qualifiers.get("q").copied();
            let view = qualifiers.get("d").copied();
            let Some((values, registrations)) =
                results(q, &q.tree, "this", &q.class, anchors, view)
            else {
                break;
            };
            let value = if values.len() == 1 {
                values[0].clone()
            } else {
                list_literal(&values)
            };
            let value = match (q.list, queries) {
                (true, _) => value,
                (false, Some(queries)) => format!("{queries}firstOrNull({value})"),
                (false, None) => format!("{value}.first"),
            };
            let mut text = registrations.join("\n");
            if !text.is_empty() {
                text.push('\n');
            }
            text += &format!("{} = {value};", q.target);
            let line_start = out[..i].rfind('\n').map_or(0, |k| k + 1);
            let indent = out[line_start..i].len() - out[line_start..i].trim_start().len();
            let text = indent_lines(&text, indent);
            let end = i + start.len() + f + CONTENT_END.len_utf8();
            out.replace_range(i..end, text.trim_start());
        }
    }
    out
}

/// A view query updated during detection (`createDynamicUpdates`): the
/// "dirty" field, the anchor and the class of the result's embedded view.
#[derive(Debug, Clone)]
struct DynamicQuery {
    index: usize,
    property: String,
    list: bool,
    /// What to look for ([`query_key`]).
    key: String,
    /// How the result is read on the node: the key itself, or that of the
    /// provider that `read:` asks for ([`read_key`]).
    read: String,
    /// `_viewQuery_ref_N_isDirty`.
    field: String,
    /// `read: ElementRef`: the result is `ElementRef(node)`.
    element_ref: bool,
    /// The results in pre-order ([`query_tree`]).
    tree: Vec<QueryItem>,
    /// Some `*` with a result was emitted (without it, the view was refused
    /// and the query is not written).
    view: bool,
}

/// A hand-written `<template [ngIf]="x">…</template>` is the same
/// `EmbeddedTemplateAst` as `<ng-container *ngIf="x">…</ng-container>`: the
/// children are the roots of the embedded view. Only the shape with a binding
/// to a known single-input structural directive (`ngIf`, `ngSwitchCase`,
/// `ngSwitchWhen`), or only with the `ngSwitchDefault` attribute, and nothing
/// else is rewritten — with the
/// binding's range, which is that of the `REF`; the rest stays `<template>`
/// and is refused.
fn template_as_container(nodes: &[Node]) -> Vec<Node> {
    nodes
        .iter()
        .map(|n| match n {
            Node::Element(e)
                if e.star.as_ref().is_some_and(|l| {
                    l.name != TEMPLATE_MARK && Structural::known(&l.name).is_none()
                }) =>
            {
                // `*dir` of another directive: the `<template>` that the
                // microsyntax produces, with the element inside, through the
                // path of the written `<template>`.
                template_as_container(std::slice::from_ref(&Node::Element(star_template(e))))
                    .remove(0)
            }
            Node::Element(e) => {
                let mut e = e.clone();
                e.children = template_as_container(&e.children);
                let clean = e.events.is_empty()
                    && e.bananas.is_empty()
                    && e.references.is_empty()
                    && e.annotations.is_empty()
                    && e.star.is_none();
                let binding = match (e.attributes.as_slice(), e.properties.as_slice()) {
                    ([], [p])
                        if matches!(p.name.as_str(), "ngIf" | "ngSwitchCase" | "ngSwitchWhen")
                            && !crate::micro::e_micro(p.value.trim()) =>
                    {
                        Some(p.clone())
                    }
                    // `<template ngSwitchDefault>`: the attribute only matches
                    // the selector, with no input.
                    ([a], []) if a.name == "ngSwitchDefault" && a.value.is_empty() => {
                        Some(a.clone())
                    }
                    _ => None,
                };
                if let (true, "template", Some(l)) = (clean, e.name.as_str(), binding) {
                    e.star = Some(l);
                    e.properties.clear();
                    e.attributes.clear();
                    e.name = "ng-container".into();
                } else if let (true, "template", Some((star, bindings, micro))) =
                    (clean, e.name.as_str(), template_with_directive(&e))
                {
                    e.star = Some(star);
                    e.template_bindings = bindings;
                    e.template_micro = Some(micro);
                    e.properties.clear();
                    e.attributes.clear();
                    e.name = "ng-container".into();
                } else if e.name == "template"
                    && e.star.is_none()
                    && e.bananas.is_empty()
                    && e.annotations.is_empty()
                    && e.references.len() <= 1
                    && e.references.iter().all(|r| r.value.is_empty())
                {
                    // `<template>` with (at most) one `#ref`, `let-x`, and the
                    // attributes, `[x]` and `(x)` of its directives: anchor,
                    // `ViewContainer` and `TemplateRef`, with the content in an
                    // embedded view ([`ViewBody::hand_template`], which resolves the
                    // directives and refuses what none of them receives).
                    // The start is that of the `<template>`: it identifies its
                    // anchor in the view queries, like that of a `*`.
                    e.star = Some(crate::html::Binding {
                        name: TEMPLATE_MARK.into(),
                        value: String::new(),
                        start: e.start,
                        end: e.start,
                        without_value: true,
                    });
                }
                Node::Element(e)
            }
            other => other.clone(),
        })
        .collect()
}

/// The `<template>` of a `*dir="..."` (ngast's `micro/parser.dart`):
/// `[dir]="expr"` and `[dirKey]="expr"` for the bindings, `let-x="key"` for
/// the locals, and the empty `dir` attribute when the first binding is not
/// the directive's own (`*dir` alone, `*dir="let x of xs"`). Every binding has
/// the range of the whole `*dir` attribute, which is that of the `REF` (case
/// j94). The element goes inside, without the star; the `<template>` starts
/// where the star starts (its anchor in the queries).
fn star_template(e: &crate::html::Element) -> crate::html::Element {
    let star = e.star.clone().unwrap_or_default();
    let micro = e.star_micro().unwrap_or_default();
    let binding = |name: &str, value: &str| crate::html::Binding {
        name: name.to_string(),
        value: value.to_string(),
        start: star.start,
        end: star.end,
        without_value: false,
    };
    let mut t = crate::html::Element {
        name: "template".into(),
        start: star.start,
        ..Default::default()
    };
    if micro
        .properties
        .first()
        .is_none_or(|(p, _)| *p != star.name)
    {
        // The desugared `dir` has no value (`EmptyExpr`: `true` in a `bool`
        // input, the `*deferredContent="forceContent: x"`).
        t.attributes.push(crate::html::Binding {
            without_value: true,
            ..binding(&star.name, "")
        });
    }
    for (name, key) in &micro.locals {
        let value = if key == "$implicit" { "" } else { key.as_str() };
        t.attributes.push(binding(&format!("let-{name}"), value));
    }
    for (name, expr) in &micro.properties {
        t.properties.push(binding(name, expr));
    }
    let mut inside = e.clone();
    inside.star = None;
    inside.template_micro = None;
    t.children = vec![Node::Element(inside)];
    t
}

/// A hand-written `<template dir let-x let-y="key" [dirA]="a" [dirB]="b">` is
/// exactly the `<template>` that the microsyntax of
/// `*dir="let x; let y = key; a: a; b: b"` produces (ngast's
/// `micro/parser.dart` unfolds the `*` into these attributes): the same
/// `EmbeddedTemplateAst`, with the directive matched by the selector
/// (`[ngFor][ngForOf]`). Rewritten as the equivalent `star`, it follows
/// the path of the `*`; the only thing the `*` does not keep is the range of
/// each written binding — the `REF` of each input —, returned separately.
///
/// Only the unambiguous shape: the known structural directive matched by an
/// attribute without a value with its name (`<template ngFor [ngForOf]="a">`,
/// which is `*ngFor="of: a"`) or by the `[dir]` binding itself (`<template
/// [ngTemplateOutlet]="t" [ngTemplateOutletValue]="v">`, which is
/// `*ngTemplateOutlet="t; value: v"`: the directive's expression comes
/// first); `let-x` (with or without a value); and the other `[dirX]` bindings
/// with the directive's prefix and without `;` in the expression (which the
/// microsyntax would split). The rest stays `<template>` and is refused.
fn template_with_directive(
    e: &crate::html::Element,
) -> Option<(
    crate::html::Binding,
    Vec<crate::html::Binding>,
    crate::micro::Micro,
)> {
    let (lets, others): (Vec<_>, Vec<_>) = e
        .attributes
        .iter()
        .partition(|a| a.name.starts_with("let-"));
    let mut parts = Vec::new();
    // The equivalent microsyntax, already decomposed: the text (`parts`) only
    // serves as a description, because `a ? b : c; value: x` would not even
    // pass `isMicroExpression` (case j75).
    let mut micro = crate::micro::Micro::default();
    let (dir, properties) = match others.as_slice() {
        [dir] if dir.value.is_empty() && Structural::known(&dir.name).is_some() => {
            if e.properties.is_empty() {
                return None;
            }
            ((*dir).clone(), e.properties.iter().collect::<Vec<_>>())
        }
        [] => {
            let (matched, rest): (Vec<_>, Vec<_>) = e
                .properties
                .iter()
                .partition(|p| Structural::known(&p.name).is_some());
            let [dir] = matched.as_slice() else {
                return None;
            };
            if dir.value.trim().is_empty() {
                return None;
            }
            parts.push(dir.value.trim().to_string());
            micro
                .properties
                .push((dir.name.clone(), dir.value.trim().to_string()));
            ((*dir).clone(), rest)
        }
        _ => return None,
    };
    for l in &lets {
        let name = &l.name["let-".len()..];
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        {
            return None;
        }
        let key = l.value.trim();
        if key.contains([';', '=']) {
            return None;
        }
        parts.push(if key.is_empty() {
            format!("let {name}")
        } else {
            format!("let {name} = {key}")
        });
        micro.locals.push((
            name.to_string(),
            if key.is_empty() {
                "$implicit".to_string()
            } else {
                key.to_string()
            },
        ));
    }
    for p in &properties {
        let suffix = p.name.strip_prefix(dir.name.as_str())?;
        let mut cs = suffix.chars();
        let first = cs.next()?;
        if !first.is_ascii_uppercase() || p.value.trim().is_empty() {
            return None;
        }
        parts.push(format!(
            "{}{}: {}",
            first.to_ascii_lowercase(),
            cs.as_str(),
            p.value.trim()
        ));
        micro
            .properties
            .push((p.name.clone(), p.value.trim().to_string()));
    }
    Some((
        crate::html::Binding {
            name: dir.name.clone(),
            value: parts.join("; "),
            start: dir.start,
            end: dir.end,
            without_value: false,
        },
        e.properties.clone(),
        micro,
    ))
}

/// The `#ref` names declared more than once in the template, or with the name
/// of a `let` of some `*`: each view resolves the name by its own scope (the
/// closest wins), which is not translated yet.
fn ambiguous_refs(nodes: &[Node]) -> std::collections::HashSet<String> {
    fn walk(nodes: &[Node], refs: &mut Vec<String>, lets: &mut Vec<String>) {
        for n in nodes {
            if let Node::Element(e) = n {
                refs.extend(e.references.iter().map(|r| r.name.clone()));
                if let Some(micro) = e.star_micro() {
                    lets.extend(micro.locals.into_iter().map(|(name, _)| name));
                }
                walk(&e.children, refs, lets);
            }
        }
    }
    let (mut refs, mut lets) = (Vec::new(), Vec::new());
    walk(nodes, &mut refs, &mut lets);
    refs.iter()
        .filter(|n| refs.iter().filter(|x| x == n).count() > 1 || lets.contains(n))
        .cloned()
        .collect()
}

/// The node fields in the order in which `NodeReferenceStorageVisitor`
/// promotes them: by the first read outside `build()`. In
/// `detectChangesInternal` the declarations of the locals (`final local_x =
/// this._el_n;`) come at the top, so the nodes read by them come before those
/// that the bindings read (in document order).
fn el_fields_in_order(
    el_fields: &[String],
    field_refs: &std::collections::HashMap<String, Vec<String>>,
    root_locals: &[String],
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in root_locals {
        for c in el_fields {
            let read = field_refs.get(c).is_some_and(|r| r.contains(name));
            if read && !out.contains(c) {
                out.push(c.clone());
            }
        }
    }
    for c in el_fields {
        if !out.contains(c) {
            out.push(c.clone());
        }
    }
    out
}

/// The `#ref`s read as locals whose node becomes a field: those without a
/// value. With a value (`#d="x"`), the local is the exported directive, and the
/// element stays as it is.
fn promoted_nodes(
    nodes: &[Node],
    refs: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    refs.iter()
        .filter(|n| !ref_with_value(nodes, n))
        .cloned()
        .collect()
}

/// Is there any `#name="value"` (with a value) in the template, at any depth?
fn ref_with_value(nodes: &[Node], name: &str) -> bool {
    nodes.iter().any(|n| match n {
        Node::Element(e) => {
            e.references
                .iter()
                .any(|r| r.name == name && !r.value.is_empty())
                || ref_with_value(&e.children, name)
        }
        _ => false,
    })
}

/// The `#ref`s of the written `<template>`s ([`TEMPLATE_MARK`]), at any
/// depth.
fn template_refs(nodes: &[Node]) -> Vec<String> {
    let mut out = Vec::new();
    for n in nodes {
        if let Node::Element(e) = n {
            if e.star.as_ref().is_some_and(|l| l.name == TEMPLATE_MARK) {
                out.extend(e.references.iter().map(|r| r.name.clone()));
            }
            out.extend(template_refs(&e.children));
        }
    }
    out
}

/// Where a `#ref` appears in the template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// On an HTML element of the view itself.
    Root,
    /// Inside an embedded view (`*ngIf`, `*ngFor`).
    Embedded,
    /// On a tag that is neither HTML nor a component (that of a directive), or
    /// below it: a shape still refused for `@ViewChild`.
    Child,
    /// On the element itself of a child component of the view: the instance
    /// counts.
    OnChild,
    /// On an HTML element of the content projected into a child: it is still a
    /// node of this view (created detached and handed over in
    /// `createAndProject`).
    Projected,
    /// On a child component inside the content projected into another: the
    /// instance, also of this view.
    OnProjectedChild,
}

fn where_is(
    nodes: &[Node],
    name: &str,
    children: &std::collections::HashMap<String, Child>,
    in_child: bool,
    out: &mut Vec<Place>,
) {
    let has_ref = |e: &crate::html::Element| matches_key(e, name, children);
    where_matches(nodes, &has_ref, children, in_child, out);
}

/// What a query looks for, as a key of [`where_is`]: the `#ref`, or
/// [`type_key`] of the component of `@ViewChild(Type)` (`None` without
/// resolution).
fn query_key(
    query: &crate::component::Query,
    local: &Local,
    resolver: Option<&dyn Resolution>,
) -> Option<String> {
    if query.by_type {
        query_uri(query, local, resolver).map(|u| type_key(&u, &query.reference))
    } else {
        Some(query.reference.clone())
    }
}

/// The element matches a query's key: it has the `#ref`, or it is a child
/// component of the type of [`type_key`].
fn matches_key(
    e: &crate::html::Element,
    key: &str,
    children: &std::collections::HashMap<String, Child>,
) -> bool {
    match key.strip_prefix('\u{7}').and_then(|c| c.rsplit_once('#')) {
        Some((uri, class)) => children
            .get(&e.name)
            .is_some_and(|f| f.uri_dart == uri && f.class == class),
        None => e.references.iter().any(|r| r.name == key),
    }
}

/// Where `ViewBuilder` finds the first result of `key` inside a `*`: that is
/// where the query gets the `_isDirty` field (`_setParentQueryAsDirty`,
/// allocated the first time), and `ViewStorage` writes the fields in
/// allocation order. `ViewBuilder` descends into the embedded views on the
/// spot (`visitEmbeddedTemplate`) and the result goes into the element's
/// `beforeChildren`: pre-order of the whole template. On the same element,
/// the providers (query by type) come before the references, and the
/// references in the order they are written. `None`: no dynamic result.
fn first_dynamic_result(
    nodes: &[Node],
    key: &str,
    children: &std::collections::HashMap<String, Child>,
) -> Option<(usize, usize)> {
    fn walk(
        nodes: &[Node],
        key: &str,
        children: &std::collections::HashMap<String, Child>,
        dynamic: bool,
        ordinal: &mut usize,
    ) -> Option<(usize, usize)> {
        for n in nodes {
            let Node::Element(e) = n else { continue };
            let here = *ordinal;
            *ordinal += 1;
            let dynamic = dynamic || e.star.is_some();
            if dynamic && matches_key(e, key, children) {
                let place = if key.starts_with('\u{7}') {
                    0
                } else {
                    1 + e.references.iter().position(|r| r.name == key).unwrap_or(0)
                };
                return Some((here, place));
            }
            if let Some(r) = walk(&e.children, key, children, dynamic, ordinal) {
                return Some(r);
            }
        }
        None
    }
    walk(nodes, key, children, false, &mut 0)
}

/// Like [`first_dynamic_result`], with the position of the node
/// ([`crate::html::Element::start`]) of the first result inside `*`.
fn first_dynamic_result_start(
    nodes: &[Node],
    key: &str,
    children: &std::collections::HashMap<String, Child>,
) -> Option<usize> {
    fn walk(
        nodes: &[Node],
        key: &str,
        children: &std::collections::HashMap<String, Child>,
        dynamic: bool,
    ) -> Option<usize> {
        for n in nodes {
            let Node::Element(e) = n else { continue };
            let dynamic = dynamic || e.star.is_some();
            if dynamic && matches_key(e, key, children) {
                return Some(e.start);
            }
            if let Some(r) = walk(&e.children, key, children, dynamic) {
                return Some(r);
            }
        }
        None
    }
    walk(nodes, key, children, false)
}

/// The position of the first node of this view (outside `*`) that matches
/// `key`.
fn first_result_start_in_view(
    nodes: &[Node],
    key: &str,
    children: &std::collections::HashMap<String, Child>,
) -> Option<usize> {
    nodes.iter().find_map(|n| {
        let Node::Element(e) = n else { return None };
        if e.star.is_some() {
            return None;
        }
        if matches_key(e, key, children) {
            return Some(e.start);
        }
        first_result_start_in_view(&e.children, key, children)
    })
}

/// The fields with an initializer of a view, which open the class
/// (`dart_emitter.dart:198-205`), in the order `ViewStorage` allocates them:
/// the lazy provider when its node is created, the dirty field of a query when
/// its first result in `*` is visited (pre-order of the whole template, the
/// embedded views in place). On the same node, the provider comes first, and
/// the content query before the view query (`_getQueriesFor`).
fn initialized_fields(body: &ViewBody) -> Vec<String> {
    let mut all: Vec<(usize, u8, usize, String)> = Vec::new();
    for (k, c) in body.lazy_fields.iter().enumerate() {
        let p = body.lazy_positions.get(k).copied().unwrap_or(0);
        all.push((p, 0, k, c.clone()));
    }
    for (k, (p, c)) in body.content_dirty_fields.iter().enumerate() {
        all.push((*p, 1, k, c.clone()));
    }
    for (k, (p, c)) in body.view_dirty_fields.iter().enumerate() {
        all.push((*p, 2, k, c.clone()));
    }
    all.sort_by_key(|(p, kind, k, _)| (*p, *kind, *k));
    all.into_iter().map(|(_, _, _, c)| c).collect()
}

/// Mark, at the start of a root or projection item, that it is already a list
/// (`this.projectedNodes[i]`, `o.ArrayType`).
const LIST_ROOT: char = '\u{11}';

/// A text with `{{ }}` converted ([`ViewBody::interpolated_value`]).
struct Interpolated {
    converted: Vec<crate::expr::Converted>,
    immutable: bool,
    /// The binding's number (`_expr_k`, `currVal_k`).
    k: u32,
    /// The value computed once, when immutable.
    constant: String,
    /// The checked `currVal_k`.
    check: String,
    /// The value where it is used (`currVal_k`, or interpolated there).
    in_action: String,
}

/// A list of nodes as `createFlatArrayForProjectNodes` writes it.
enum FlatList {
    /// `const <Object>[]`.
    Empty,
    /// `<Object>[a, b]`, with the items.
    Literal(Vec<String>),
    /// Any other expression (a projected list, or the concatenation).
    Expression(String),
    /// Two or more `..addAll(x)` sections: `dart format` puts each one on its
    /// own line, two spaces past the start of the expression.
    Cascade { head: String, sections: Vec<String> },
}

impl FlatList {
    fn text(&self) -> String {
        self.text_at(0)
    }

    /// The text with the expression starting at column `column` (that of the
    /// cascade's sections is `column + 2`).
    fn text_at(&self, column: usize) -> String {
        match self {
            FlatList::Empty => "const <Object>[]".to_string(),
            FlatList::Literal(items) => format!("<Object>[{}]", items.join(", ")),
            FlatList::Expression(e) => e.clone(),
            FlatList::Cascade { head, sections } => {
                let indent = " ".repeat(column + 2);
                let mut t = head.clone();
                for x in sections {
                    t.push_str(&format!("\n{indent}..addAll({x})"));
                }
                t
            }
        }
    }
}

/// `createFlatArrayForProjectNodes` (`view_compiler_utils.dart`): the items
/// that are not lists go together in an `<Object>[..]`; each projected list
/// ([`LIST_ROOT`]) is concatenated with `..addAll(..)` — the one that does
/// not open the expression, inside `unsafeCast`. A list alone is itself. With
/// more than one concatenation, the builder's `dart format` breaks the cascade
/// into lines, a shape with no case yet.
fn flat_list(items: &[String], util: &str) -> Result<FlatList, Refusal> {
    let list = |x: &String| x.strip_prefix(LIST_ROOT).map(str::to_string);
    match items {
        [] => return Ok(FlatList::Empty),
        [x] => {
            return Ok(match list(x) {
                Some(l) => FlatList::Expression(l),
                None => FlatList::Literal(vec![x.clone()]),
            });
        }
        _ => {}
    }
    if !items.iter().any(|x| x.starts_with(LIST_ROOT)) {
        return Ok(FlatList::Literal(items.to_vec()));
    }
    let mut loose: Vec<String> = Vec::new();
    // The head (the first literal) and each `..addAll(..)` section.
    let mut result: Option<String> = None;
    let mut sections: Vec<String> = Vec::new();
    let push_part =
        |result: &mut Option<String>, part: String, sections: &mut Vec<String>| match result {
            None => *result = Some(part),
            Some(_) => sections.push(part),
        };
    for x in items {
        match list(x) {
            Some(l) => {
                if !loose.is_empty() {
                    let part = format!("<Object>[{}]", loose.join(", "));
                    push_part(&mut result, part, &mut sections);
                    loose.clear();
                }
                let part = if result.is_none() {
                    format!("<Object>[{l}]")
                } else {
                    format!("{util}.unsafeCast({l})")
                };
                push_part(&mut result, part, &mut sections);
            }
            None => loose.push(x.clone()),
        }
    }
    if !loose.is_empty() {
        let part = format!("<Object>[{}]", loose.join(", "));
        push_part(&mut result, part, &mut sections);
    }
    let head = result.unwrap_or_default();
    // One section stays on the line (`a..addAll(b)`); with two or more,
    // `dart format` breaks it (`icon_tooltip`, `modal`).
    Ok(match sections.as_slice() {
        [] => FlatList::Expression(head),
        [x] => FlatList::Expression(format!("{head}..addAll({x})")),
        _ => FlatList::Cascade { head, sections },
    })
}

/// How many `<ng-content>` there are in `nodes`, at any depth.
fn ng_content_count(nodes: &[Node]) -> u32 {
    nodes
        .iter()
        .map(|n| match n {
            Node::Content { .. } => 1,
            Node::Element(e) => ng_content_count(&e.children),
            _ => 0,
        })
        .sum()
}

/// The key of the results of a `@ViewChild(Type)` among the `#ref`s seen
/// (`ViewBody::refs_in_order`): it does not collide with a reference name.
fn type_key(uri: &str, class: &str) -> String {
    format!("\u{7}{uri}#{class}")
}

/// The library that declares the type of a `@ViewChild(Type)`, in the
/// component's scope.
fn query_uri(
    query: &crate::component::Query,
    local: &Local,
    resolver: Option<&dyn Resolution>,
) -> Option<String> {
    resolver?.type_uri(local.path, &query.reference)
}

/// Where, in document order, the elements that match `predicate` are.
/// Where, in pre-order, the first result of the query `key` that belongs to
/// this view (outside `*` and the content of `<template>`) is: the element's
/// ordinal and, inside it, the `_QueryWithRead` order — the queries by type
/// (the node's providers) before those by `#ref`, these in the order of the
/// `#ref`s. `None` when there is none (it goes to the end).
fn first_result(
    nodes: &[Node],
    key: &str,
    children: &std::collections::HashMap<String, Child>,
) -> Option<(usize, usize)> {
    fn walk(
        nodes: &[Node],
        key: &str,
        children: &std::collections::HashMap<String, Child>,
        ordinal: &mut usize,
    ) -> Option<(usize, usize)> {
        for node in nodes {
            let Node::Element(e) = node else { continue };
            *ordinal += 1;
            let template = e.star.as_ref().is_some_and(|l| l.name == TEMPLATE_MARK);
            if e.star.is_some() && !template {
                continue;
            }
            if matches_key(e, key, children) {
                let inside = if key.starts_with('\u{7}') {
                    0
                } else {
                    1 + e.references.iter().position(|r| r.name == key).unwrap_or(0)
                };
                return Some((*ordinal, inside));
            }
            if !template && let Some(p) = walk(&e.children, key, children, ordinal) {
                return Some(p);
            }
        }
        None
    }
    let mut ordinal = 0;
    walk(nodes, key, children, &mut ordinal)
}

fn where_matches(
    nodes: &[Node],
    predicate: &dyn Fn(&crate::html::Element) -> bool,
    children: &std::collections::HashMap<String, Child>,
    in_child: bool,
    out: &mut Vec<Place>,
) {
    for node in nodes {
        let Node::Element(e) = node else { continue };
        // The written `<template>` belongs to this view; its content does not.
        let template = e.star.as_ref().is_some_and(|l| l.name == TEMPLATE_MARK);
        let place = if e.star.is_some() && !template {
            Place::Embedded
        } else if children.contains_key(&e.name) {
            if in_child {
                Place::OnProjectedChild
            } else {
                Place::OnChild
            }
        } else if !dom::tag_html(&e.name) && e.name != "ng-container" {
            // `<ng-container>` (also the rewritten `<template [ngIf]>`) is not
            // a node: its children stay where it is (case j78).
            Place::Child
        } else if in_child {
            Place::Projected
        } else {
            Place::Root
        };
        if predicate(e) {
            out.push(place);
        }
        let mut inside = Vec::new();
        let below_child = in_child
            || matches!(
                place,
                Place::Child | Place::OnChild | Place::OnProjectedChild | Place::Projected
            );
        where_matches(&e.children, predicate, children, below_child, &mut inside);
        // Everything below a `*` belongs to the embedded view.
        if place == Place::Embedded || template {
            inside.iter_mut().for_each(|l| *l = Place::Embedded);
        }
        out.extend(inside);
    }
}

/// Is the field's type `dart:html`'s `Element` or a subtype of it? Every
/// `…Element` of `dart:html` descends from `Element`, except `NoncedElement`
/// (checked in the `html_dart2js.dart` of SDK 3.6.2).
fn is_element_type(ty: &str, local: &Local, resolver: Option<&dyn Resolution>) -> bool {
    let ty = ty.trim().trim_end_matches('?');
    let simple = crate::resolucao::without_anchor(ty.rsplit('.').next().unwrap_or(ty));
    // In 9, `Element` is the one from `package:web` (`find_components.dart:533`).
    resolver
        .and_then(|r| r.type_uri(local.path, ty))
        .is_some_and(|uri| crate::dialect::is_element(&uri, simple))
}

/// The value of `[style.x.unit]` when the source is null: `null` in 8, `''` in
/// 9 (`package:web`'s `setProperty` does not accept null,
/// `NX:compiler/view_compiler/update_statement_visitor.dart:145`).
fn null_style() -> &'static str {
    match crate::dialect::current() {
        crate::dialect::Dialect::Ngdart => "null",
        crate::dialect::Dialect::Ngx => "''",
    }
}

/// The value of `[style.x]` without a unit (`visitStyleBinding`): the value
/// itself if it is a `String`, otherwise `v.toString()` (`v?.toString()` when
/// it may be null); in 9, wrapped in `?? ''` when it may be null
/// (`update_statement_visitor.dart:148-159`).
fn unitless_style(value: &str, is_string: bool, nullable: bool) -> String {
    let v = if is_string {
        value.to_string()
    } else if nullable {
        format!("{value}?.toString()")
    } else {
        format!("{value}.toString()")
    };
    if crate::dialect::current() == crate::dialect::Dialect::Ngx && nullable {
        format!("({v} ?? '')")
    } else {
        v
    }
}

/// The value of a static query in 9: with an `Element` field (or a list of
/// `Element`, `isElementType`), each result goes into an `unsafeCast(..)`
/// (`NX:compiler/view_compiler/compile_query.dart:473-486`); in 8, as it is.
fn element_with_cast(
    value: String,
    query: &crate::component::Query,
    local: &Local,
    resolver: Option<&dyn Resolution>,
) -> String {
    if crate::dialect::current() == crate::dialect::Dialect::Ngdart {
        return value;
    }
    let ty = if query.list {
        element_type(&query.ty)
    } else {
        Some(query.ty.clone())
    };
    if ty.is_some_and(|t| is_element_type(&t, local, resolver)) {
        format!("{}unsafeCast({value})", deferred_q(UTILITIES))
    } else {
        value
    }
}

/// The value of a view query whose result is an HTML element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ElementValue {
    /// The node itself (`_el_n`).
    Node,
    /// `ElementRef(_el_n)`.
    ElementRef,
}

/// What an element gives to a query (`compile_element.dart`, the
/// `queriesWithReads` loop): with `read: ElementRef`, the node's `ElementRef`;
/// with `read: Element`/`HtmlElement` (from `dart:html`), the node; without
/// `read:`, the node if the field is `Element` (`isElementType`), otherwise
/// the `ElementRef`. `None` for `read:` of another token (directive,
/// `ViewContainerRef`…), which cannot be written yet.
fn element_value(
    query: &crate::component::Query,
    local: &Local,
    resolver: Option<&dyn Resolution>,
) -> Option<ElementValue> {
    let Some(t) = query.read.as_deref() else {
        return Some(if is_element_type(&query.ty, local, resolver) {
            ElementValue::Node
        } else {
            ElementValue::ElementRef
        });
    };
    let uri = resolver.and_then(|r| r.type_uri(local.path, t))?;
    match (t, uri.as_str()) {
        ("ElementRef", ELEMENT_REF) => Some(ElementValue::ElementRef),
        (t, u) if crate::dialect::embedded_node(u, t.rsplit('.').next().unwrap_or(t)).is_some() => {
            Some(ElementValue::Node)
        }
        _ => None,
    }
}

/// The token that a query's `read:` asks of a provider of the node (not the
/// node nor `ElementRef`, which [`element_value`] reads, nor a built-in):
/// the class, by the library that declares it. The node creates it eagerly
/// (`queriedTokens`) and the query receives the instance.
fn read_token(
    query: &crate::component::Query,
    local: &Local,
    resolver: Option<&dyn Resolution>,
) -> Option<crate::directives::Token> {
    let t = query.read.as_deref()?;
    if element_value(query, local, resolver).is_some() {
        return None;
    }
    let uri = resolver?.type_uri(local.path, t)?;
    let token = crate::directives::Token::Class {
        uri,
        class: t.to_string(),
    };
    (!token.builtin()).then_some(token)
}

/// The mark of the result of `key` read through the node's provider `t`.
fn read_key(key: &str, t: &crate::directives::Token) -> String {
    match t {
        crate::directives::Token::Class { uri, class } => format!("{key}\u{4}{uri}#{class}"),
        other => format!("{key}\u{4}{}", other.name()),
    }
}

/// `detectChangesInCheckAlwaysViews` of a (non-host) view of a
/// `@changeDetectionLink` component: each public `ViewContainer`, then each
/// child that is also linked (`compile_view.dart:1383-1395`). When empty, the
/// method is not emitted.
fn link_method(link: bool, anchors: &[String], linked: &[String]) -> String {
    if !link || (anchors.is_empty() && linked.is_empty()) {
        return String::new();
    }
    let lines: Vec<String> = anchors
        .iter()
        .chain(linked)
        .map(|a| format!("    this.{a}.detectChangesInCheckAlwaysViews();"))
        .collect();
    format!(
        "\n  @override\n  void detectChangesInCheckAlwaysViews() {{\n{}\n  }}\n",
        lines.join("\n")
    )
}

/// Does the query read ngdart's `ViewContainerRef` (`read: ViewContainerRef`)?
/// On a node's `#ref`, this turns on its `_requiresViewContainer`
/// (`provider_parser.dart:95-104`).
fn reads_container(
    query: &crate::component::Query,
    local: &Local,
    resolver: Option<&dyn Resolution>,
) -> bool {
    let Some(t) = query.read.as_deref().map(str::trim) else {
        return false;
    };
    t.rsplit('.').next() == Some("ViewContainerRef")
        && resolver
            .and_then(|r| r.type_uri(local.path, t))
            .is_some_and(|u| u.starts_with("package:ngdart/"))
}

/// The key of the result of `#name` on a `<template>` read as
/// `ViewContainerRef`: the node's `ViewContainer` (`this._appEl_n`).
fn container_key(name: &str) -> String {
    format!("{name}\u{4}ViewContainerRef")
}

/// Is the field's type ngdart's `TemplateRef` (`TemplateRef` or
/// `TemplateRef?`, without a prefix, resolved to `template_ref.dart`)?
fn is_template_ref(ty: &str, local: &Local, resolver: Option<&dyn Resolution>) -> bool {
    let ty = ty.trim().trim_end_matches('?');
    ty == "TemplateRef"
        && resolver.and_then(|r| r.type_uri(local.path, ty)).as_deref() == Some(TEMPLATE_REF)
}

/// A pipe from `pipes:`, with the URI of the library that declares it.
#[derive(Debug, Clone)]
pub struct UsedPipe {
    pub uri: String,
    pub pipe: crate::component::Pipe,
}

/// The instance of a pipe, a field of a view (`_pipe_date_0`): that of a pure
/// pipe is one per name, in the component's view, created on first use
/// (`compView.purePipes`); that of an impure one is one per call, in the
/// call's view. The number is the `pipeCount` of the view that holds it.
#[derive(Debug, Clone)]
struct PipeInstance {
    name: String,
    field: String,
    class: String,
    /// Import path of the pipe's library (`getImportModulePath`).
    path: String,
    /// The view that holds the instance (0 for every pure pipe).
    view: u32,
    impure: bool,
    /// The constructor receives the `ChangeDetectorRef` (the view itself).
    detector: bool,
    /// `OnDestroy`: `ngOnDestroy()` in the view's `destroyInternal`.
    destroys: bool,
}

/// A `$pipe.name(..)` call: the view where it is and what replaces it — the
/// proxy (`_pipe_date_0_1`, `_PurePipeProxy`, a field of that view) of a pure
/// pipe, or `_pipe_async_1.transform` of an impure one.
#[derive(Debug, Clone)]
struct PipeCall {
    view: u32,
    instance: usize,
    proxy: String,
    arguments: usize,
    /// `o.FunctionType(returnType, paramTypes.sublist(0, argCount))`; empty
    /// for the impure one, which has no proxy.
    ty: String,
    /// The type mentions `dart:core` (everything that is not `dynamic`).
    core: bool,
}

/// The pipes of a template: the instances and each call, in the order in which
/// the official compiler converts them — that of `bindView`, which descends
/// into the embedded views where they are. It is this order that numbers the
/// proxies, and it is only known by looking at the whole template before
/// emitting: the embedded views here are emitted after the component's.
#[derive(Debug, Default)]
struct TemplatePipes {
    instances: Vec<PipeInstance>,
    calls: Vec<PipeCall>,
}

impl TemplatePipes {
    fn of_view(&self, view: u32) -> impl Iterator<Item = &PipeCall> {
        self.calls.iter().filter(move |c| c.view == view)
    }

    /// The pure-pipe proxies of this view, of one instance.
    fn proxies(&self, view: u32, k: usize) -> impl Iterator<Item = &PipeCall> {
        self.of_view(view)
            .filter(move |c| c.instance == k && !self.instances[k].impure)
    }

    /// The imports of the pipe fields of this view, in declaration order: the
    /// class of each instance held here and the `dart:core` of a proxy's type.
    fn field_imports(&self, view: u32) -> Vec<String> {
        let mut out = Vec::new();
        for (k, inst) in self.instances.iter().enumerate() {
            if inst.view == view {
                out.push(inst.path.clone());
            }
            if self.proxies(view, k).any(|c| c.core) {
                out.push("dart:core".to_string());
            }
        }
        out
    }

    /// The pipe fields of this view, in the order in which each
    /// `CompilePipe`'s `create()` allocates them: the instance and, right
    /// after, its proxies. They come after the `_expr_` (allocated in
    /// `bindView`) and before the `_el_` (promoted to fields at the end).
    fn fields(&self, view: u32, imp: &mut Imports) -> Vec<String> {
        let mut out = Vec::new();
        for (k, inst) in self.instances.iter().enumerate() {
            if inst.view == view {
                let q = imp.q(&inst.path);
                out.push(format!("  late final {q}{} {};", inst.class, inst.field));
            }
            for c in self.proxies(view, k) {
                out.push(format!("  late final {} {};", c.ty, c.proxy));
            }
        }
        out
    }

    /// The creation in `build()` (`afterNodes`, after the listeners): the
    /// instance (`createPipeInstance`) and the proxies (`createPureProxy`),
    /// which read the instance through the `parentView` chain (`base`).
    fn creation(&self, view: u32, base: &str, imp: &mut Imports) -> Vec<String> {
        let mut out = Vec::new();
        for (k, inst) in self.instances.iter().enumerate() {
            if inst.view == view {
                let q = imp.q(&inst.path);
                let arg = if inst.detector { "this" } else { "" };
                out.push(format!(
                    "    this.{} = {q}{}({arg});",
                    inst.field, inst.class
                ));
            }
            for c in self.proxies(view, k) {
                out.push(format!(
                    "    this.{} = {}.pureProxy{}({base}.{}.transform);",
                    c.proxy,
                    deferred(PROXIES),
                    c.arguments,
                    inst.field
                ));
            }
        }
        out
    }

    /// The `ngOnDestroy()` of the instances of this view
    /// (`bindPipeDestroyLifecycleCallbacks`), in their order.
    fn destruction(&self, view: u32) -> Vec<String> {
        self.instances
            .iter()
            .filter(|i| i.view == view && i.destroys)
            .map(|i| format!("    this.{}.ngOnDestroy();", i.field))
            .collect()
    }
}

/// A type that `fromDartType` writes without an import of its own: `dynamic`
/// or a `dart:core` type, with or without `?`.
fn is_core_type(t: &str) -> bool {
    t == "dynamic"
        || matches!(
            t.strip_suffix('?').unwrap_or(t),
            "String" | "int" | "double" | "num" | "bool" | "Object"
        )
}

/// The template's pipe table (see [`TemplatePipes`]). The pipe of each call
/// is the last one in `pipes:` with that name (`_findPipeMeta`).
fn template_pipes(
    nodes: &[Node],
    children: &std::collections::HashMap<String, Child>,
    pipes: &Result<Vec<UsedPipe>, Refusal>,
    asset: &str,
) -> Result<TemplatePipes, Refusal> {
    let mut raw = Vec::new();
    let mut next = 1;
    raw_calls(nodes, 0, &mut next, children, &mut raw)?;
    let mut t = TemplatePipes::default();
    if raw.is_empty() {
        return Ok(t);
    }
    let pipes = pipes.as_ref().map_err(Clone::clone)?;
    let refuse = |f: &str| refusal(Reason::UsedPipes, f);
    // The `pipeCount` of each view.
    let mut counters: std::collections::HashMap<u32, usize> = Default::default();
    for (view, name, args) in raw {
        let used = pipes
            .iter()
            .rev()
            .find(|p| p.pipe.name == name)
            .ok_or_else(|| refuse("pipe that is not in pipes:"))?;
        let p = &used.pipe;
        if let Some(f) = p.unsupported {
            return Err(refuse(f));
        }
        // More arguments than parameters is a compile error in the official
        // compiler.
        if args > p.params.len() {
            return Err(refuse("pipe with too many arguments"));
        }
        let path = || {
            asset_of_uri(&used.uri, "", Path::new(""))
                .and_then(|target| import_path(asset, &target))
                .ok_or_else(|| refuse("pipe without an import path"))
        };
        let new_instance = |t: &mut TemplatePipes,
                            counters: &mut std::collections::HashMap<u32, usize>,
                            view: u32,
                            impure: bool|
         -> Result<usize, Refusal> {
            let n = counters.entry(view).or_default();
            t.instances.push(PipeInstance {
                field: format!("_pipe_{name}_{n}"),
                name: name.clone(),
                class: p.class.clone(),
                path: path()?,
                view,
                impure,
                detector: p.detector,
                destroys: p.destroys,
            });
            *n += 1;
            Ok(t.instances.len() - 1)
        };
        // Impure: one instance per call, in its view, called directly
        // (`_call`: `instance.transform(..)`).
        if !p.pure {
            let instance = new_instance(&mut t, &mut counters, view, true)?;
            let proxy = format!("{}.transform", t.instances[instance].field);
            t.calls.push(PipeCall {
                view,
                instance,
                proxy,
                arguments: args,
                ty: String::new(),
                core: false,
            });
            continue;
        }
        // `Identifiers.pureProxies`: `pureProxy1` to `pureProxy6` checked in
        // `proxies.dart`.
        if args > 6 {
            return Err(refuse("pipe with more than 6 arguments"));
        }
        let params: Option<Vec<String>> = p.params[..args]
            .iter()
            .map(|x| x.clone().filter(|x| is_core_type(x)))
            .collect();
        let (Some(params), true) = (params, is_core_type(&p.return_type)) else {
            return Err(refuse("transform with a type outside dart:core or untyped"));
        };
        let core = std::iter::once(&p.return_type)
            .chain(&params)
            .any(|x| x != "dynamic");
        let instance = match t.instances.iter().position(|i| i.name == name && !i.impure) {
            Some(k) => k,
            None => new_instance(&mut t, &mut counters, 0, false)?,
        };
        let already = t.calls.iter().filter(|c| c.instance == instance).count();
        t.calls.push(PipeCall {
            view,
            instance,
            proxy: format!("{}_{already}", t.instances[instance].field),
            arguments: args,
            ty: format!("{} Function({})", p.return_type, params.join(", ")),
            core,
        });
    }
    // In an embedded view, the order between an impure pipe's instance and a
    // pure pipe's proxies (created by the instance's `create()`, in the
    // component's view) has no case yet.
    for c in &t.calls {
        let impure_here = t.instances.iter().any(|i| i.impure && i.view == c.view);
        if c.view != 0 && impure_here && !t.instances[c.instance].impure {
            return Err(refuse(
                "impure pipe and pure pipe in the same embedded view",
            ));
        }
    }
    Ok(t)
}

/// The template's `$pipe.name(..)` calls (view, name, arguments), in
/// `bindView` order: on each element its bindings before the children, and
/// the content of a `*` in its place, as a new view. The views are numbered
/// like the embedded ones (depth-first, starting at 1). A pipe where the
/// order has not been checked yet (event, `*`, child input) is
/// refused.
fn raw_calls(
    nodes: &[Node],
    view: u32,
    next: &mut u32,
    children: &std::collections::HashMap<String, Child>,
    out: &mut Vec<(u32, String, usize)>,
) -> Result<(), Refusal> {
    let has = |t: &str| t.contains("$pipe");
    for node in nodes {
        match node {
            Node::Interpolation { expr, .. } => pipes_in_expression(expr, view, out)?,
            Node::Element(e) => {
                if let Some(star) = &e.star {
                    if has(&star.value) {
                        return Err(refusal(Reason::UsedPipes, "pipe in the input of `*`"));
                    }
                    let v = *next;
                    *next += 1;
                    let mut without_star = e.clone();
                    without_star.star = None;
                    let inside = Node::Element(without_star);
                    raw_calls(std::slice::from_ref(&inside), v, next, children, out)?;
                    continue;
                }
                if e.events.iter().any(|l| has(&l.value)) {
                    return Err(refusal(Reason::UsedPipes, "pipe in an event"));
                }
                if e.bananas.iter().any(|l| has(&l.value)) {
                    return Err(refusal(Reason::UsedPipes, "pipe in [(x)]"));
                }
                let of_child = children.contains_key(&e.name) || !dom::tag_html(&e.name);
                // `[x]` and then the interpolated attributes, as
                // `html_element` converts them.
                for l in &e.properties {
                    if has(&l.value) {
                        if of_child {
                            return Err(refusal(
                                Reason::UsedPipes,
                                "pipe in a child component binding",
                            ));
                        }
                        pipes_in_expression(&l.value, view, out)?;
                    }
                }
                for a in e.attributes.iter().filter(|a| a.value.contains("{{")) {
                    if !has(&a.value) {
                        continue;
                    }
                    if of_child {
                        return Err(refusal(
                            Reason::UsedPipes,
                            "pipe in a child component binding",
                        ));
                    }
                    let Some((_, exprs)) = interpolation_parts(&a.value) else {
                        return Err(refusal(
                            Reason::UsedPipes,
                            "pipe in an unreadable attribute",
                        ));
                    };
                    for x in &exprs {
                        pipes_in_expression(x, view, out)?;
                    }
                }
                raw_calls(&e.children, view, next, children, out)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// The `$pipe.name(..)` calls of an expression, with the number of arguments,
/// in conversion order: the pipes of the arguments before the outer one
/// (`visitPipe` converts the input and the arguments first). Text between
/// quotes does not count.
fn pipes_in_expression(
    text: &str,
    view: u32,
    out: &mut Vec<(u32, String, usize)>,
) -> Result<(), Refusal> {
    let shape = || {
        refusal(
            Reason::UsedPipes,
            "`$pipe` outside the `$pipe.name(..)` shape",
        )
    };
    let is_name = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let b = text.as_bytes();
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == b'\'' || c == b'"' {
            quote = Some(c);
            i += 1;
            continue;
        }
        if !text[i..].starts_with("$pipe") {
            i += 1;
            continue;
        }
        if i > 0 && (is_name(b[i - 1]) || b[i - 1] == b'.') {
            return Err(shape());
        }
        if b.get(i + 5) != Some(&b'.') {
            return Err(shape());
        }
        let start = i + 6;
        let mut j = start;
        while j < b.len() && is_name(b[j]) {
            j += 1;
        }
        let mut k = j;
        while k < b.len() && b[k].is_ascii_whitespace() {
            k += 1;
        }
        if j == start || b.get(k) != Some(&b'(') {
            return Err(shape());
        }
        let (end, args) = call_arguments(text, k).ok_or_else(shape)?;
        pipes_in_expression(&text[k + 1..end - 1], view, out)?;
        out.push((view, text[start..j].to_string(), args));
        i = end;
    }
    Ok(())
}

/// From the `(` at `open`: the index after the closing `)` and how many
/// arguments there are (commas at the first level, plus one).
fn call_arguments(text: &str, open: usize) -> Option<(usize, usize)> {
    let b = text.as_bytes();
    let mut depth = 0u32;
    let mut commas = 0;
    let mut any = false;
    let mut quote: Option<u8> = None;
    let mut i = open;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'\'' | b'"' => quote = Some(c),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    let n = if any { commas + 1 } else { 0 };
                    return Some((i + 1, n));
                }
            }
            b',' if depth == 1 => commas += 1,
            _ => {}
        }
        if depth >= 1 && i > open && !c.is_ascii_whitespace() {
            any = true;
        }
        i += 1;
    }
    None
}

/// A component that this template may use, coming from the package index.
#[derive(Debug, Clone)]
pub struct Child {
    pub class: String,
    pub selector: String,
    /// `package:` URI of the `.dart` that declares the class.
    pub uri_dart: String,
    /// `package:` URI of its `.template.dart`.
    pub uri_template: String,
    /// `ngContentSelectors` of the child's template: the `select` of each
    /// `<ng-content>`, in order, `*` without it. With any, whoever uses the
    /// child calls `createAndProject` with one list of nodes per projection.
    pub projections: Vec<String>,
    /// The child's `@Input`s, in the order the official compiler writes them.
    pub inputs: Vec<crate::component::Input>,
    /// The child's lifecycle hooks: whoever uses it calls them
    /// (`bindDirectiveDetectChangesLifecycleCallbacks`,
    /// `bindDirectiveAfterChildrenCallbacks`).
    pub hooks: crate::component::Hooks,
    /// `onPush`: whoever changes an input marks the child for check.
    pub on_push: bool,
    /// `@changeDetectionLink`: in a view of a component that is also linked,
    /// `detectChangesInCheckAlwaysViews` descends into its view.
    pub detection_link: bool,
    /// `@HostBinding`: whoever uses it calls `detectHostChanges(firstCheck)`
    /// before detecting its view (`bindDirectiveHostProps`).
    pub host: bool,
    /// `@Output`s (name in the template, member), in the order of the
    /// `outputs` map.
    pub outputs: Vec<(String, String)>,
    /// The child's `hostAttributes`: `@HostBinding` on an immutable static
    /// member outside `class.x`/`style.x` (`_computeHostBindingImmutability`),
    /// by the attribute name (without `attr.`) and the member. Whoever uses it
    /// merges them with the written attribute of the same name
    /// (`_mergeHtmlAndDirectiveAttrs`).
    pub host_attributes: Vec<(String, String)>,
    /// What the child's constructor receives, in order.
    pub params: Vec<Injected>,
    /// The child's `@ContentChild`/`@ContentChildren`, with the target
    /// resolved: (field, list, target). Whoever projects content into it
    /// updates the query.
    pub queries: Vec<ChildQuery>,
    /// What in the child changes the code of whoever uses it and the emitter
    /// does not write yet: constructor injection, `@HostBinding`, content
    /// query, providers, projection with a selector. Any of them refuses the
    /// use.
    pub pending: Vec<Refusal>,
    /// The component's metadata read from the program (`metadata.rs`): its
    /// `providers:` go into the node of whoever uses it.
    pub metadata: Option<std::sync::Arc<crate::directives::Directive>>,
}

/// A parameter of a child's constructor, as the official compiler resolves it
/// on its node (`provider_resolver.dart`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Injected {
    /// `Element`/`HtmlElement`: the node itself.
    Element,
    /// `ChangeDetectorRef`: the child's view.
    Detector,
    /// `ViewContainerRef`: the `ViewContainer` of the child's node
    /// (`_appEl_n`).
    Container,
    /// A service from outside the view: `injectorGet` through the view above
    /// (`injectFromViewParentInjector`), `injectorGetOptional` with
    /// `@Optional()`.
    Service {
        /// The class, or the `OpaqueToken`/`MultiToken` of `@Inject(..)`.
        token: crate::directives::Token,
        optional: bool,
        /// `@Self()`, `@Host()`, `@SkipSelf()` (`_getDependency`).
        self_only: bool,
        host: bool,
        skip_self: bool,
    },
    /// `@Attribute('name')`: the literal value of the attribute on the child's
    /// element, or `null` (`_getLocalDependency`).
    Attribute(String),
}

/// Where a service that the child injects comes from (`_getDependency`).
enum ServiceOrigin {
    /// A provider of the node itself, created before the child.
    Local(String),
    /// An element above provides it: its read.
    Above(String),
    /// An `@Optional()` that is not found (`@Self`, or `@Host` outside the
    /// host view).
    Null,
    /// The injector from outside the view.
    Injector,
}

/// An `@ContentChild`/`@ContentChildren` of a child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildQuery {
    pub field: String,
    /// `@ContentChildren`.
    pub list: bool,
    pub target: QueryTarget,
    pub descendants: bool,
    /// `read:`: what is read from the node found, instead of the instance that
    /// matched.
    pub read: Option<QueryRead>,
}

/// The `read:` of a child's content query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryRead {
    /// `HtmlElement`/`Element` of `dart:html`: the node itself.
    Element,
    /// Another provider of the node found, by class (URI, name).
    Class(String, String),
}

/// What a child's content query looks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryTarget {
    /// A class (URI, name).
    Class(String, String),
    /// A `#ref` of the content.
    Reference(String),
}

impl Child {
    /// The member that the `@Output` `name` exposes.
    pub fn output(&self, name: &str) -> Option<&str> {
        self.outputs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, m)| m.as_str())
    }

    /// The `@Input` that receives the binding `name`.
    pub fn input(&self, name: &str) -> Option<&crate::component::Input> {
        self.inputs.iter().find(|e| e.name == name)
    }
}

/// A directive or component from `directives:`, already resolved, with the
/// selector read. A component's list follows the official compiler's order
/// (the depth-first expansion of the constant lists, without repetition): it
/// is the order in which the directives of a node are instantiated.
#[derive(Debug, Clone)]
pub struct UsedDirective {
    pub class: String,
    /// URI of the library that declares the class.
    pub uri: String,
    pub selectors: Vec<crate::selector::Selector>,
    /// `Some` when it is a component: what the emitter knows about it.
    pub child: Option<Child>,
    /// The metadata read from the program (`metadata.rs`), when it is a
    /// directive and a program is loaded.
    pub directive: Option<std::sync::Arc<crate::directives::Directive>>,
}

impl UsedDirective {
    /// Is it ngdart's own structural directive that the emitter knows
    /// (`NgIf`, `NgFor`)?
    fn is_structural(&self, e: &Structural) -> bool {
        self.class == e.class && self.uri == e.uri
    }
}

/// What the emitter needs to know about where the component lives.
pub struct Local<'a> {
    /// Package name (`new_sali_frontend`).
    pub package: &'a str,
    /// Path of the `.dart` inside the package, with slashes:
    /// `lib/src/x/foo.dart`.
    pub relative: &'a str,
    /// File name for the `import` of itself: `foo.dart`.
    pub file: &'a str,
    /// Path on disk, to ask the semantic database in which scope the
    /// constructor's names are resolved.
    pub path: &'a Path,
    /// Package root, to map `file:` URIs of the project itself.
    pub root: &'a Path,
    /// `package:` URI of the template's `.html` file, when there is one. It is
    /// where the `/* REF:url:start:end */` comment that the official compiler
    /// writes on each binding points to.
    pub template_url: Option<String>,
    /// The component's own metadata read from the program (`metadata.rs`): its
    /// `providers:` go to the host view.
    pub metadata: Option<std::sync::Arc<crate::directives::Directive>>,
}

impl Local<'_> {
    /// `package:` URI of the compiled stylesheet of a `styleUrls`: the
    /// `.css.shim.dart` (with shim) or, with `ViewEncapsulation.none`, the
    /// `.css.dart` (ngcompiler's `stylesModuleUrl(url, shim)`).
    pub(crate) fn style_uri(&self, url: &str, shim: bool) -> Option<String> {
        let suffix = if shim { ".shim.dart" } else { ".dart" };
        // A written `package:` URI: it is already the stylesheet's
        // (`button_decorator.scss.css` of `dropdown_button`).
        if url.starts_with("package:") {
            return Some(format!("{url}{suffix}"));
        }
        let inside = self.relative.strip_prefix("lib/")?;
        let dir = inside.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let path = if dir.is_empty() {
            url.to_string()
        } else {
            format!("{dir}/{url}")
        };
        // Resolved as a URI: `.` and `..` go away.
        let mut parts: Vec<&str> = Vec::new();
        for p in path.split('/') {
            match p {
                "." | "" => {}
                ".." => {
                    parts.pop()?;
                }
                _ => parts.push(p),
            }
        }
        Some(format!(
            "package:{}/{}{suffix}",
            self.package,
            parts.join("/")
        ))
    }

    /// `asset:` URI of this file — the space in which the official emitter
    /// computes the import paths.
    fn asset(&self) -> String {
        format!("asset:{}/{}", self.package, self.relative)
    }
}

/// A property binding already translated: the action of a constant one (it
/// goes into `if (firstCheck)`) or the whole block of a dynamic one.
enum BoundProperty {
    Constant(String),
    Dynamic(String),
}

/// Body of a view's `build()`, assembled while walking the template.
struct ViewBody<'a> {
    lines: Vec<String>,
    /// `addEventListener` of each event. The official compiler creates all the
    /// nodes first and binds afterwards (`_buildView` and only then
    /// `bindView`, in `view_compiler.dart`), so the listeners come out
    /// together, after the last node, in document order.
    listeners: Vec<String>,
    /// `#ref`s accepted in this template ([`free_refs`]); empty in
    /// the embedded view.
    free_refs: std::collections::HashSet<String>,
    /// Element `#ref`s of this view read by its expressions
    /// ([`local_refs`]): the node becomes a field and the name, a
    /// local (`final local_x = this._el_n;`).
    local_refs: std::collections::HashSet<String>,
    /// The `#ref`s repeated or shadowed by `let`
    /// ([`ambiguous_refs`]): their per-view scope is not translated
    /// yet, and the refusal says so.
    ambiguous_refs: std::collections::HashSet<String>,
    /// The local `#ref`s of the ancestor views (see [`EmbeddedSpec`]).
    ancestor_refs: Vec<(String, String, u32)>,
    /// In the component's view: the view queries updated during detection
    /// ([`query_in_embedded`]).
    dynamic_queries: Vec<DynamicQuery>,
    /// The dynamic queries whose result is below this embedded view: (`#ref`,
    /// dirty field, levels up to the component's view).
    queries_in_transit: Vec<(String, String, u32)>,
    /// The anchors of the dynamic queries, from the [`Context`].
    query_anchors: &'a std::cell::RefCell<Anchors>,
    /// [`Context::dynamic_content`].
    dynamic_content: &'a std::cell::RefCell<Vec<DynamicContent>>,
    /// [`Context::dirty_content_queries`].
    dirty_content_queries: &'a std::cell::RefCell<DirtyContentQueries>,
    /// The updates of this view's dynamic content queries
    /// (`updateContentQuery`), in the block of the content hooks, after the
    /// view ones and before the hooks, in the order of the nodes'
    /// `afterChildren`.
    content_updates: Vec<String>,
    /// Those of the view queries by token ([`dynamic_token_query`]),
    /// with the query's index (the order in `afterNodes`).
    view_updates: Vec<(usize, String)>,
    /// The indices of these queries (outside the static assignment).
    token_queries: std::collections::HashSet<usize>,
    /// The `bool _query_X_n_i_isDirty = true;` fields of these queries, with
    /// the position of the first result in `*` (when the field is allocated).
    content_dirty_fields: Vec<(usize, String)>,
    /// How many content queries each node has already registered
    /// (`_queryCount`).
    queries_per_node: std::collections::HashMap<u32, usize>,
    /// In the embedded view: the `#ref`s that are a result of a query of the
    /// component's view (the node becomes a field) and the "dirty" field of
    /// each one.
    queried_refs: Vec<(String, String, u32)>,
    /// Each `#ref` seen, with the node's expression (`_el_3` or `this._el_3`)
    /// — it is the value that `@ViewChild` receives.
    refs: std::collections::HashMap<String, String>,
    /// The same, in document order and with repetition: the results of a
    /// `@ViewChildren` (`addQueryResult`).
    /// [`Context::reads`].
    reads: &'a [(String, crate::directives::Token)],
    /// [`Context::provider_only_refs`].
    provider_only_refs: &'a std::collections::HashSet<String>,
    /// [`Context::templates_with_container`].
    templates_with_container: &'a std::collections::HashSet<String>,
    /// [`Context::directive_types`].
    directive_types: &'a [ResolvedDirectiveType],
    refs_in_order: Vec<(String, String)>,
    /// The tag of the element whose bindings are being written: the security
    /// context of a property depends on it ([`sanitizer`]).
    current_tag: String,
    /// The inherited namespace prefix (`svg`, `math`): `_NamespaceVisitor`
    /// passes the parent's to the descendants
    /// (`ast_template_parser.dart:1331-1352`).
    current_ns: Option<String>,
    /// The `appViewInstance` of the node whose bindings are being written:
    /// `this`, or the component's view on its element (`this._compView_n`, the
    /// `[class]` of a child's element, case j74).
    view_instance: Option<String>,
    /// The view passed to the `detectHostChanges` of a directive with
    /// `@HostBinding` on the node: that of the child component, on its node
    /// (case j92); `None` is the view itself (`this`).
    host_view: Option<String>,
    /// `TextBinding` and `_message_N` fields, which come first in the class,
    /// in the order `build()` allocates them.
    fields: Vec<String>,
    /// Prefix of `package:intl` when the view has `@i18n` that the emitter
    /// translates (only the component's); `None` refuses the annotation.
    intl: Option<String>,
    /// The messages already created (`createI18nMessage` reuses an equal
    /// one): the `Intl.message(..)` call and the field.
    messages: Vec<(String, String)>,
    /// `Object? _expr_k` fields of the bindings, in the order they appear.
    expr_fields: Vec<String>,
    /// `late final T _el_n` fields of the elements with a binding.
    el_fields: Vec<String>,
    /// The `#ref`s (without a value) of the node of each field of
    /// [`ViewBody::el_fields`].
    el_field_refs: std::collections::HashMap<String, Vec<String>>,
    /// The `#ref`s of this view read only in an event handler: the node
    /// becomes a field when `NodeReferenceStorageVisitor` reaches the
    /// `_handleEvent_N` that reads it — after those promoted in detection
    /// (case j60).
    event_only_refs: std::collections::HashSet<String>,
    /// The fields of these nodes waiting for the first handler that reads
    /// them.
    event_el_fields: Vec<(String, String)>,
    /// The same, in the order the handlers read them.
    el_fields_by_event: Vec<String>,
    /// Fields of the nodes of this embedded view that are a result of a
    /// dynamic query of the component's view, with the query's position in
    /// [`ViewBody::queried_refs`]: the official compiler promotes them to
    /// fields when the component's view writes the `mapNestedViews`
    /// (`replaceReadClassMemberInExpression`), before promoting the nodes that
    /// this view's bindings read — they come before [`ViewBody::el_fields`], in
    /// query order.
    queried_el_fields: Vec<(usize, String)>,
    /// Fields of the nodes that an embedded view reads (a `#ref` of this view
    /// read from inside a `*`), with the name: they go among the `_expr_k`, at
    /// the position of [`ViewBody::embedded_position`]
    /// ([`referenced_in_embedded_views`]).
    embedded_el_fields: Vec<(String, String)>,
    /// The local `#ref`s of this view read from an embedded view.
    embedded_refs: std::collections::HashSet<String>,
    /// How many `_expr_k` existed when the first embedded view that reads each
    /// of them was bound.
    embedded_position: std::collections::HashMap<String, usize>,
    /// Next binding index (`_expr_k`, `currVal_k`).
    next_binding: u32,
    /// Body of `detectChangesInternal`.
    detection: Vec<String>,
    /// Prefix of `text_binding.dart`, allocated before the rest when the
    /// template has interpolation (the order of the imports follows the order
    /// in which the official compiler writes the file, and the fields come
    /// first).
    /// Some `TextBinding` field was created in this view.
    tb_used: bool,
    tb: Option<String>,
    /// `package:` URI of the template file, for the `REF` comment.
    template_url: Option<String>,
    /// Types of the component's members, to choose `interpolateString`.
    members: &'a std::collections::HashMap<String, crate::component::Member>,
    exports: &'a Exports,
    /// [`Context::generic_names`].
    generic_names: &'a [String],
    /// [`Context::view_tokens`].
    view_tokens: &'a [crate::directives::Token],
    /// Methods of the class, valid only as a call target.
    methods: &'a std::collections::HashMap<String, String>,
    /// Positional parameters of each method, for the event tear-off.
    arities: &'a std::collections::HashMap<String, usize>,
    /// The `_handleEvent_N` methods of this view, in the order they were
    /// created (`createEventHandler`); they come after `destroyInternal`.
    event_methods: Vec<String>,
    /// The type of the `$event` of the next `_handleEvent_N` (in 9: that of
    /// the native event or of the output, qualified by a deferred mark);
    /// `None`, raw.
    event_type: Option<String>,
    /// Why the type of `$event` is not known: only the complex handler (the
    /// `_handleEvent_N`) needs it, and it is the one that refuses.
    event_type_failure: Option<Refusal>,
    /// The `static String _message_N(..)` methods of the `@i18n` messages
    /// with HTML, which open the view's methods.
    i18n_methods: Vec<String>,
    /// Declaration (`final local_x = …;`) of each local of **this** view, or
    /// why it cannot be declared. A local of an ancestor view is not here: it
    /// is read through the `parentView` chain, a shape still refused.
    local_decls: std::collections::HashMap<String, Result<String, Refusal>>,
    /// The locals read by the detection bindings, in order of first use (the
    /// `_localsInScope` of the view's `ViewNameResolver`): it is the order of
    /// the declarations at the top of `detectChangesInternal`.
    root_locals: Vec<String>,
    /// Class of this view (`_ViewX2`) and the locals it declares itself
    /// (name, key), for the nested views to read through `parentView`.
    this_class: String,
    own_locals: Vec<(String, String)>,
    /// The locals of the ancestor views, with where they come from.
    ancestors: std::collections::HashMap<String, Origin>,
    /// The injectable providers of the elements above the node, the farthest
    /// first: (token, field, and the ancestor view that has it — class and how
    /// many `parentView` up to it — or nothing, if it is this one's).
    above: Vec<(crate::directives::Token, String, Option<(String, u32)>)>,
    /// The fields of `above` that are lazy providers of the owner element
    /// (field, class of its view): a node below that requests them would
    /// transform them before the owner's `afterElement` (gap L1 of section
    /// 05).
    lazy_above: std::collections::HashSet<(String, String)>,
    /// Those whose owner modelled the content's requests (a child's node):
    /// from another view, read through the lazy field.
    lazy_with_requests: std::collections::HashSet<(String, String)>,
    /// How many component elements there are above (providers that the
    /// emitter does not model).
    components_above: u32,
    /// Components above whose providers are not known (without program
    /// metadata): what the node does not find above may be in them.
    uncertain_above: u32,
    /// Components that this template may use, by selector.
    children: &'a std::collections::HashMap<String, Child>,
    /// All the directives and components of `directives:`, to know what
    /// matches each element.
    used: &'a [UsedDirective],
    /// Collection mode (the scoreboard diagnostic): each refusal is noted here
    /// and the emission goes on, to find the others. `None`: the first
    /// refusal stops it, and the file is left to `build_runner`.
    collected: Option<Vec<Refusal>>,
    /// Fields of the lazy providers (`late T _X_n_m = value;`): `ViewStorage`
    /// allocates them when building the view, before promoting nodes and text
    /// bindings to fields, so they open the class (case j38).
    lazy_fields: Vec<String>,
    /// The position of each one's node ([`crate::html::Element::start`]):
    /// the allocation order among the fields with an initializer.
    lazy_positions: Vec<usize>,
    /// The dirty fields of the dynamic view queries, with the position of
    /// their first result (see [`initialized_fields`]).
    view_dirty_fields: Vec<(usize, String)>,
    /// Fields of the child views (`_compView_n` and the instance), which come
    /// out in the class after the text bindings.
    child_fields: Vec<String>,
    /// `_compView_n` of each child, for detection and destruction.
    child_views: Vec<String>,
    /// Those of the `@changeDetectionLink` children ([`link_method`]).
    linked_views: Vec<String>,
    /// Asset of this file, to compute the import paths of the children.
    asset: String,
    /// Semantic database and the file, to type chains like `item.name`.
    types: Option<(&'a dyn Resolution, &'a Path)>,
    /// The component's class with its import prefix (`import1.Class`): the
    /// receiver of the static members.
    qualified_class: String,
    /// The component has a stylesheet: each element gets `addShimC`.
    with_style: bool,
    /// Embedded views to emit. Only the specification: the text is generated
    /// after `angular.dart`, which is where the official compiler writes these
    /// classes — and it is the writing order that numbers the imports.
    embedded_views: Vec<EmbeddedSpec>,
    /// Name of the view class (`ViewX`), to number the embedded views.
    view_class: String,
    /// The type arguments of a generic component (`<T, U>`), empty for the
    /// rest: the factory of each embedded view is called with them.
    generics: String,
    /// `preserveWhitespace: true`: the ends of the interpolations are not
    /// compressed (`_compressWhitespace*`).
    preserve_whitespace: bool,
    /// `_appEl_n` of each `ViewContainer`, for detection and destruction.
    anchors: Vec<String>,
    /// This view is embedded: the root is registered with `initRootNode`.
    embedded: bool,
    /// Locals of the scope (`let item of items`), which shadow the members.
    locals: std::collections::HashMap<String, crate::expr::Local>,
    /// Next embedded view number. The official compiler uses a single
    /// depth-first counter (`_view.viewIndex + _nestedViewCount` in
    /// `view_builder.dart`): the first top-level `*` is 1, the one nested
    /// inside it is 2, and the next sibling is 3.
    next_embedded: u32,
    /// To parse the template's expressions, which are Dart expressions.
    names: &'a mut dartforge_intern::Interner,
    /// `bool firstCheck = this.firstCheck;` in `detectChangesInternal`, when
    /// some immutable binding is written only on the first check.
    uses_first_check: bool,
    /// Inputs of child directives and components (`@Input`, `ngIf`,
    /// `ngForOf`) and their `ngDoCheck`: the `detectChangesInInputsMethod`,
    /// which the official compiler writes **before** the nested views and the
    /// property and text bindings (`writeChangeDetectionStatements`).
    inputs: Vec<String>,
    /// Next node index. It counts elements and texts together, in document
    /// order; a comment does not consume an index because it disappears
    /// earlier.
    next_node: u32,
    /// `final doc = …` comes out once, on the first element.
    has_doc: bool,
    /// `final _ctx = this.ctx;` at the top of `build()`, when some immutable
    /// expression is computed there.
    uses_ctx_in_build: bool,
    /// Ordinal of the next `<ng-content>` — it is the second argument of
    /// `project`, and does not consume a node index.
    next_projection: u32,
    imp: &'a mut Imports,
    /// The template's pipes, with each view's call.
    pipes: &'a TemplatePipes,
    /// Number of this view (0 the component's, the `index` in the embedded
    /// one) and how many `parentView` separate it from the component's view.
    view: u32,
    depth: u32,
    /// How many pipe calls of this view have already been converted.
    cursor_pipe: usize,
    /// `changed` is read or written: `bool changed = false;` at the top of
    /// detection.
    uses_changed: bool,
    /// The children's `ngAfterContent*` and `ngAfterView*` hooks, in the
    /// official compiler's order (bottom-up), already at method level: they go
    /// inside `if ((!debugThrowIfChanged))`.
    after_content: Vec<String>,
    after_view: Vec<String>,
    /// The children's `ngOnDestroy`, at the end of `destroyInternal`.
    destroy: Vec<String>,
    /// How many `subscription_N` (a child's `@Output`) this view has.
    subscriptions: usize,
    /// The children whose projected content we are in (URI, class): one of
    /// them, if injected, would be resolved right here, not by the injector
    /// above.
    children_above: Vec<(String, String)>,
    /// `#ref` of an `onPush` child, with its view: `@ViewChild` registers the
    /// `ChangeDetectorRef` (`queryChangeDetectorRefs`).
    /// The same registrations as [`ViewBody::detectors`], in node order (one
    /// per result, also with a repeated `#ref`).
    detectors_in_order: Vec<(String, String)>,
    detectors: std::collections::HashMap<String, String>,
    /// The nodes created without a parent (root of the embedded view,
    /// projected content), as they are read later: `_el_3`, `this._el_3`,
    /// `this._appEl_4`.
    roots: Vec<String>,
    /// The index of the child whose projected content is being created: it is
    /// the parent of a detached anchor there (`parent.nodeIndex`).
    projected_parent: Option<u32>,
    /// The depth of the view where the topmost node of the injector chain of
    /// a node of this view is (`ProviderResolver._getDependency` goes up to
    /// the child of the root of the component's view; the root of an embedded
    /// view has as its parent the parent of its anchor).
    top_level: u32,
    /// The nodes with injectable providers (`ProviderNode`), in pre-order:
    /// (first index, last index of the subtree, [(tokens, field)]).
    injectors: Vec<InjectorNode>,
    /// The elements we are in (index, whether it has a directive or
    /// component): the `parent` chain of `_getQueriesFor`.
    stack: Vec<(u32, bool)>,
    /// The providers created in this view, in pre-order, with the chain of
    /// elements above each one: it is from them that the static result of a
    /// content query comes (`addQueryResult`).
    registrations: Vec<Registration>,
}

/// A node with providers, for the content queries: the tokens by which each
/// instance matches a query and its field.
#[derive(Debug, Clone)]
struct Registration {
    above: Vec<(u32, bool)>,
    /// (token, field, and the child's view when it is an `onPush` component:
    /// the provider's `buildChangeDetectorRef`).
    providers: Vec<(crate::directives::Token, String, Option<String>)>,
    /// The node as `build()` reads it (`_el_3` or `this._el_3`), for
    /// `read: HtmlElement`.
    element: String,
}

impl ViewBody<'_> {
    /// The class of this view, for the `unsafeCast` of whoever reads it from
    /// below.
    /// The type arguments from `directiveTypes:` for the directive `class`
    /// (from `uri`) on node `e` (`lookupTypeArgumentsOf`, `compile_view.dart:
    /// 1523-1569`): the `Typed` whose `on:` equals a `#ref` of the node wins;
    /// otherwise the first one without `on:`. Empty when there is none.
    fn type_arguments(&mut self, uri: &str, class: &str, e: &crate::html::Element) -> String {
        let text = self.written_type_arguments(uri, class, e);
        resolve_deferred(self.imp, &text)
    }

    fn written_type_arguments(&self, uri: &str, class: &str, e: &crate::html::Element) -> String {
        let mut first = None;
        for t in self.directive_types {
            if t.uri != uri || t.class != class {
                continue;
            }
            match &t.em {
                Some(r) => {
                    if e.references.iter().any(|x| x.name == *r) {
                        return t.arguments.clone();
                    }
                }
                None if first.is_none() => {
                    if e.references.is_empty() {
                        return t.arguments.clone();
                    }
                    first = Some(t.arguments.clone());
                }
                None => {}
            }
        }
        first.unwrap_or_default()
    }

    /// Registers, by the position of the node ([`node_key`]), each class
    /// token that its instances provide (the instance itself and the
    /// `ExistingProvider`s): the value that a content query reads from another
    /// view (`_providers.get(token).build()`).
    fn register_node_by_position(
        &mut self,
        start: usize,
        instances: &[crate::directives::Instance],
    ) {
        for i in instances {
            for t in std::iter::once(&i.token).chain(&i.aliases) {
                if let crate::directives::Token::Class { uri, class } = t {
                    self.refs
                        .entry(node_key(&type_key(uri, class), start))
                        .or_insert_with(|| format!("this.{}", i.read));
                }
            }
        }
    }

    /// The results of a content query by token (`uri`, `class`) in `nodes` (the
    /// content of the query's node, `descendants: true`): each node that
    /// provides it (a component by its tag, a matched directive, or the
    /// `providers:` of one of them) and each embedded view with results, in
    /// pre-order. `Err` for a result that is not translated yet (an `onPush`
    /// component, which would register the `ChangeDetectorRef`).
    fn content_tree(
        &self,
        nodes: &[Node],
        uri: &str,
        class: &str,
    ) -> Result<Vec<ContentItem>, Refusal> {
        token_tree(nodes, uri, class, self.children, self.used)
    }

    /// Registers the dynamic content query ([`DynamicContent`]): the dirty
    /// field, the update in change detection (the marker, assembled at the end)
    /// and the `dirtyParentQueriesInternal` of each embedded view with a
    /// result.
    #[allow(clippy::too_many_arguments)]
    fn dynamic_content_query(
        &mut self,
        tree: Vec<ContentItem>,
        key: String,
        selector: &str,
        n: u32,
        index: usize,
        list: bool,
        target: String,
        view_query: bool,
    ) {
        let class = self.this_view_class();
        // The view one (`@ViewChild(ren)` by token, rooted in the component
        // view): `_viewQuery_X_i_isDirty`, updated in `afterNodes`.
        let field = if view_query {
            format!("_viewQuery_{selector}_{index}_isDirty")
        } else {
            format!("_query_{selector}_{n}_{index}_isDirty")
        };
        let kind: u8 = if view_query { 1 } else { 0 };
        fn mark_dirty(
            items: &[ContentItem],
            level: u32,
            field: &str,
            class: &str,
            kind: u8,
            map: &mut DirtyContentQueries,
        ) {
            for i in items {
                if let ContentItem::Nested { star, items } = i {
                    let first = items.iter().find_map(|x| match x {
                        ContentItem::Value { start, .. } => Some(*start),
                        ContentItem::Nested { .. } => None,
                    });
                    if let Some(p) = first {
                        map.entry(*star).or_default().push((
                            p,
                            kind,
                            field.to_string(),
                            level,
                            class.to_string(),
                        ));
                    }
                    mark_dirty(items, level + 1, field, class, kind, map);
                }
            }
        }
        mark_dirty(
            &tree,
            1,
            &field,
            &class,
            kind,
            &mut self.dirty_content_queries.borrow_mut(),
        );
        let has_static = tree.iter().any(|i| matches!(i, ContentItem::Value { .. }));
        fn first_nested(items: &[ContentItem], nested: bool) -> Option<usize> {
            items.iter().find_map(|i| match i {
                ContentItem::Value { start, .. } => nested.then_some(*start),
                ContentItem::Nested { items, .. } => first_nested(items, true),
            })
        }
        let position = first_nested(&tree, false).unwrap_or(usize::MAX);
        fn has_on_push(items: &[ContentItem]) -> bool {
            items.iter().any(|i| match i {
                ContentItem::Value { on_push, .. } => *on_push,
                ContentItem::Nested { items, .. } => has_on_push(items),
            })
        }
        let any_on_push = has_on_push(&tree);
        let mut queries = self.dynamic_content.borrow_mut();
        let id = queries.len();
        queries.push(DynamicContent {
            id,
            tree,
            key,
            list,
            target,
            class,
        });
        let mut mark = format!("{CONTENT_MARK}{id}");
        if !list && !has_static {
            mark += &format!("|q{}", deferred_q(QUERIES));
        }
        if any_on_push {
            mark += &format!("|d{}", deferred_q(VIEW));
        }
        mark.push(CONTENT_END);
        drop(queries);
        let update =
            format!("    if (this.{field}) {{\n      {mark}\n      this.{field} = false;\n    }}");
        if view_query {
            self.view_dirty_fields
                .push((position, format!("  bool {field} = true;")));
            self.view_updates.push((index, update));
        } else {
            self.content_dirty_fields
                .push((position, format!("  bool {field} = true;")));
            self.content_updates.push(update);
        }
    }

    /// The namespace of the element and the name without prefix: an explicit
    /// `ns:tag`, the tag's implicit one (`svg`, `math`, `html_tags.dart`) or
    /// the inherited one.
    fn namespace_of(&self, e: &crate::html::Element) -> Option<(String, String)> {
        if let Some((ns, name)) = e.name.split_once(':') {
            return Some((ns.to_string(), name.to_string()));
        }
        let implicit = match e.name.to_ascii_lowercase().as_str() {
            "svg" => Some("svg"),
            "math" => Some("math"),
            _ => None,
        };
        implicit
            .map(str::to_string)
            .or_else(|| self.current_ns.clone())
            .map(|ns| (ns, e.name.clone()))
    }

    fn this_view_class(&self) -> String {
        if self.this_class.is_empty() {
            format!("{}0", self.view_class)
        } else {
            self.this_class.clone()
        }
    }

    /// The providers above, from the nearest to the farthest, with the read as
    /// seen from this view (`getPropertyInView`).
    fn providers_above(&mut self) -> Vec<crate::directives::ProviderAbove> {
        let util = self.imp.alias(UTILITIES);
        self.providers_above_with(&util)
    }

    /// [`Self::providers_above`] with the given `unsafeCast` prefix: a
    /// preliminary resolution (the position of the node's fields) does not
    /// allocate an import.
    fn providers_above_with(&self, util: &str) -> Vec<crate::directives::ProviderAbove> {
        self.above
            .iter()
            .rev()
            .map(|(t, field, v)| crate::directives::ProviderAbove {
                lazy: self.lazy_above.contains(&(
                    field.clone(),
                    match v {
                        None => self.this_view_class(),
                        Some((class, _)) => class.clone(),
                    },
                )),
                lazy_read: match v {
                    None => false,
                    Some((class, _)) => self
                        .lazy_with_requests
                        .contains(&(field.clone(), class.clone())),
                },
                token: t.clone(),
                read: match v {
                    None => format!("this.{field}"),
                    Some((class, n)) => {
                        let mut view = "(this.parentView!)".to_string();
                        for _ in 1..*n {
                            view = format!("({view}.parentView!)");
                        }
                        format!("{util}.unsafeCast<{class}>({view}).{field}")
                    }
                },
            })
            .collect()
    }

    fn dom(&mut self) -> String {
        self.imp.alias(DOM_HELPERS)
    }

    /// The `#ref`s of this view read as a local: the local (`dynamic`) and the
    /// declaration with the node's marker, replaced when the view is finished.
    fn declare_refs(&mut self, refs: std::collections::HashSet<String>) {
        let class = self.this_view_class();
        for name in &refs {
            // The type of the read is `dynamic` (the reference does not go into
            // the `locals` of the `AnalyzedClass`), or that of the member with
            // the same name, which the `_TypeResolver` finds first.
            let ty = self
                .members
                .get(name.as_str())
                .map(|m| m.ty.trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "dynamic".into());
            self.locals.insert(
                name.clone(),
                crate::expr::Local {
                    dart: format!("local_{name}"),
                    ty,
                    scope: None,
                },
            );
            self.local_decls.insert(
                name.clone(),
                Ok(format!(
                    "final local_{name} = {REF_MARK}{}{REF_END};",
                    ref_key(name, &class)
                )),
            );
        }
        self.local_refs = refs;
    }

    /// The field of an `@i18n` message without HTML (`createI18nMessage`): a
    /// `static final String _message_N = Intl.message(text, desc: ..)`, reused
    /// when the text and the metadata repeat.
    fn message(&mut self, text: &str, m: &MetaI18n) -> Result<String, Refusal> {
        let Some(intl) = self.intl.clone().filter(|i| !i.is_empty()) else {
            return Err(refusal(Reason::I18n, "@i18n outside the component view"));
        };
        let mut call = format!(
            "{intl}.Intl.message({}, desc: {}",
            literal(text),
            literal(m.description.as_deref().unwrap_or_default())
        );
        if let Some(l) = &m.locale {
            let _ = write!(call, ", locale: {}", literal(l));
        }
        if let Some(s) = &m.meaning {
            let _ = write!(call, ", meaning: {}", literal(s));
        }
        if m.skip {
            call.push_str(", skip: true");
        }
        call.push(')');
        if let Some((_, field)) = self.messages.iter().find(|(c, _)| *c == call) {
            return Ok(field.clone());
        }
        let field = format!("_message_{}", self.messages.len());
        self.fields
            .push(format!("  static final String {field} = {call};"));
        self.messages.push((call, field.clone()));
        Ok(field)
    }

    /// The children with HTML of an element with `@i18n`
    /// ([`ViewBody::i18n_children`]).
    fn i18n_html_children(
        &mut self,
        e: &crate::html::Element,
        m: &MetaI18n,
        target: &str,
    ) -> Result<(), Refusal> {
        if self.intl.is_none() {
            return self.record_refusal(refusal(Reason::I18n, "@i18n outside the component view"));
        }
        let mut text = String::new();
        let mut args = Vec::new();
        match message_with_html(&e.children, self.children, &mut text, &mut args, &mut 0) {
            Ok(true) if !args.is_empty() => {}
            Ok(_) => {
                return self.record_refusal(refusal(Reason::I18n, "@i18n message without text"));
            }
            Err(f) => return self.record_refusal(refusal(Reason::I18n, f)),
        }
        // `HtmlEscape(HtmlEscapeMode.element)` of the already escaped text.
        let text = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let intl = deferred(INTL);
        let mut call = format!(
            "{intl}.Intl.message('{text}', desc: {}",
            literal(m.description.as_deref().unwrap_or_default())
        );
        if let Some(l) = &m.locale {
            let _ = write!(call, ", locale: {}", literal(l));
        }
        if let Some(s) = &m.meaning {
            let _ = write!(call, ", meaning: {}", literal(s));
        }
        if m.skip {
            call.push_str(", skip: true");
        }
        let values: Vec<String> = args.iter().map(|(_, v)| literal(v)).collect();
        let invocation = |field: &str| format!("{field}({})", values.join(", "));
        let field = match self.messages.iter().find(|(c, _)| *c == call) {
            Some((_, field)) => field.clone(),
            None => {
                let field = format!("_message_{}", self.messages.len());
                let names: Vec<&str> = args.iter().map(|(n, _)| n.as_str()).collect();
                let examples: Vec<String> = args
                    .iter()
                    .map(|(n, v)| format!("{}: {}", literal(n), literal(v)))
                    .collect();
                let parameters: Vec<String> = names.iter().map(|n| format!("String {n}")).collect();
                self.i18n_methods.push(format!(
                    "\n  static String {field}({}) {{\n    return {call}, name: '{}_{field}', args: [{}], examples: const {{{}}});\n  }}\n",
                    parameters.join(", "),
                    self.this_view_class(),
                    names.join(", "),
                    examples.join(", ")
                ));
                self.messages.push((call, field.clone()));
                field
            }
        };
        let n = self.next_node;
        self.next_node += 1;
        let avu = self.imp.q(APP_VIEW_UTILS);
        self.lines.push(format!(
            "    final _html_{n} = {avu}createTrustedHtml({});\n    {target}.append(_html_{n});",
            invocation(&field)
        ));
        Ok(())
    }

    /// Records a refusal. Outside collection mode it stops the emission
    /// (`Err`); in collection mode, it is recorded and the emission goes on.
    fn record_refusal(&mut self, r: Refusal) -> Result<(), Refusal> {
        match &mut self.collected {
            Some(v) => {
                v.push(r);
                Ok(())
            }
            None => Err(r),
        }
    }

    fn collecting(&self) -> bool {
        self.collected.is_some()
    }

    /// The template file, for the `REF` comment. With the template written in
    /// the annotation the reference is a different one (URI of the `.dart` and
    /// offset), and that has no case in the corpus yet.
    fn url(&self, reason: Reason) -> Result<String, Refusal> {
        self.template_url
            .clone()
            .ok_or_else(|| refusal(reason, "binding in a template written in the annotation"))
    }

    /// Converts an expression of the change-detection bindings; the locals it
    /// reads go into the root list, in order of first use. A call with a named
    /// argument, which the formatter breaks into lines
    /// ([`crate::expr::BREAK`]), is left to whoever knows where it lands
    /// ([`Self::convert_breakable`]).
    fn convert(&mut self, text: &str, reason: Reason) -> Result<crate::expr::Converted, Refusal> {
        let c = self.convert_breakable(text, reason)?;
        if c.text.contains(crate::expr::BREAK) {
            return Err(refusal(
                reason,
                "call with a named argument outside `final currVal`, an event or an interpolation",
            ));
        }
        Ok(c)
    }

    /// Like [`Self::convert`], but accepts the call that breaks: for whoever
    /// writes it alone in a statement (`final currVal_k = …;`) or handles its
    /// indentation.
    fn convert_breakable(
        &mut self,
        text: &str,
        reason: Reason,
    ) -> Result<crate::expr::Converted, Refusal> {
        let locals = self.locals.clone();
        let scope = crate::expr::Scope {
            members: self.members,
            methods: self.methods,
            arities: self.arities,
            locals: &locals,
            types: self.types,
            class: Some(&self.qualified_class),
            exports: Some(self.exports),
            generics: self.generic_names,
        };
        let mut c =
            crate::expr::convert_in_scope(text, &scope, self.names).map_err(|r| r.em(reason))?;
        if c.text.contains(crate::expr::PIPE_MARK) {
            c.text = self.replace_pipes(&c.text)?;
        }
        for l in &c.locals {
            self.local_declaration(l, reason)?;
            if !self.root_locals.contains(l) {
                self.root_locals.push(l.clone());
            }
        }
        Ok(c)
    }

    /// Replaces each pipe marker of the converted text with the proxy of the
    /// next call of this view (`this._pipe_date_0_1`), checking the name and
    /// the number of arguments against the table — which has the official
    /// compiler's order. The order is that of the conversion, which visits the
    /// arguments before the pipe: with a pipe inside a pipe, it is the order in
    /// which the calls **close** in the text.
    fn replace_pipes(&mut self, text: &str) -> Result<String, Refusal> {
        use crate::expr::{PIPE_END, PIPE_MARK};
        let calls: Vec<(String, usize, String)> = self
            .pipes
            .of_view(self.view)
            .map(|c| {
                (
                    self.pipes.instances[c.instance].name.clone(),
                    c.arguments,
                    c.proxy.clone(),
                )
            })
            .collect();
        // Each marker: where its call starts and where it closes.
        let mut marks = Vec::new();
        let mut from = 0;
        while let Some(i) = text[from..].find(PIPE_MARK).map(|i| i + from) {
            let after = i + PIPE_MARK.len_utf8();
            let f = text[after..]
                .find(PIPE_END)
                .map_or(text.len(), |f| f + after);
            let open = f + PIPE_END.len_utf8();
            let close = call_arguments(text, open).map_or(text.len(), |(end, _)| end);
            marks.push((i, close));
            from = after;
        }
        let mut by_end: Vec<usize> = (0..marks.len()).collect();
        by_end.sort_by_key(|&k| marks[k].1);
        let mut order = vec![0; marks.len()];
        for (position, &k) in by_end.iter().enumerate() {
            order[k] = position;
        }
        let base = self.cursor_pipe;
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        let mut k = 0;
        while let Some(i) = rest.find(PIPE_MARK) {
            out.push_str(&rest[..i]);
            let after = &rest[i + PIPE_MARK.len_utf8()..];
            let f = after.find(PIPE_END).unwrap_or(after.len());
            let (name, n) = after[..f].rsplit_once('/').unwrap_or((&after[..f], ""));
            let position = order.get(k).copied().unwrap_or(k);
            k += 1;
            match calls.get(base + position) {
                Some((expected, arg_count, proxy))
                    if expected == name && arg_count.to_string() == n =>
                {
                    out.push_str("this.");
                    out.push_str(proxy);
                    self.cursor_pipe += 1;
                }
                // In collection mode the output is discarded and the order
                // does not matter (the refused content is reviewed elsewhere).
                _ if self.collecting() => out.push_str("this._pipe"),
                _ => {
                    return Err(refusal(
                        Reason::UsedPipes,
                        "pipe call out of the official compiler's order",
                    ));
                }
            }
            rest = after.get(f + PIPE_END.len_utf8()..).unwrap_or("");
        }
        out.push_str(rest);
        Ok(out)
    }

    /// Every pipe call of this view was converted, and in order.
    fn check_pipes(&self) -> Result<(), Refusal> {
        if !self.collecting() && self.cursor_pipe != self.pipes.of_view(self.view).count() {
            return Err(refusal(
                Reason::UsedPipes,
                "pipe call out of the official compiler's order",
            ));
        }
        Ok(())
    }

    /// The declaration of a local of this view, or the refusal: a local of an
    /// ancestor view (read through `parentView`) and a local without a known
    /// type.
    fn local_declaration(&self, name: &str, reason: Reason) -> Result<String, Refusal> {
        match self.local_decls.get(name) {
            Some(Ok(d)) => Ok(d.clone()),
            Some(Err(r)) => Err(r.clone().em(reason)),
            None => Err(refusal(
                reason,
                "local of an ancestor `*` read in the nested view",
            )),
        }
    }

    /// `[x]="e"`: new value, `checkBinding` against the previous one and the
    /// action on the element. The binding name and the template URI go into the
    /// check for the "expression changed after it was checked" message.
    fn property(
        &mut self,
        l: &crate::html::Binding,
        target: &str,
    ) -> Result<BoundProperty, Refusal> {
        let url = self.url(Reason::Binding)?;
        // The call (which may break) is never immutable: it goes alone into
        // `final currVal_k = …;`.
        let converted = self.convert_breakable(&l.value, Reason::Binding)?;
        // The text that goes into `checkBinding` is the source of the binding
        // as written (`ASTWithSource.source`), in a Dart literal.
        let expr = literal(&l.value);
        let (start, end) = (l.start, l.end);
        // Every binding consumes an index (`createUniqueBindIndex` in
        // `_checkBinding`), even the immutable one, which gets no field.
        let k = self.next_binding;
        self.next_binding += 1;
        // A value that does not change is written once, on the first check,
        // without `checkBinding` and without a previous-value field
        // (`isImmutable`, `_bindLiteral`). What can be null (`canBeNull`:
        // everything but a literal) gets an `if (x != null)` around it; the
        // `null` literal generates nothing, a shape with no case in the
        // corpus yet.
        if converted.immutable {
            if converted.text == "null" {
                return Err(refusal(Reason::Binding, "constant `null` binding"));
            }
            let action = self.action(l, target, &converted.text, &converted)?;
            if converted.can_be_null {
                let value = &converted.text;
                return Ok(BoundProperty::Constant(format!(
                    "      if (({value} != null)) {{\n        {action} /* REF:{url}:{start}:{end} */;\n      }}"
                )));
            }
            return Ok(BoundProperty::Constant(format!(
                "      {action} /* REF:{url}:{start}:{end} */;"
            )));
        }
        self.expr_fields.push(format!("  Object? _expr_{k};"));
        let action = self.action(l, target, &format!("currVal_{k}"), &converted)?;
        let chk = deferred(CHECK_BINDING);
        let value = converted.text;
        Ok(BoundProperty::Dynamic(format!(
            "    final currVal_{k} = {value};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, {expr}, '{url}')) {{\n      {action} /* REF:{url}:{start}:{end} */;\n      this._expr_{k} = currVal_{k};\n    }}"
        )))
    }

    /// The bindings of an element, as `bindAndWriteToRenderer` writes them: the
    /// constant ones first, in an `if (firstCheck)` — reusing the one of the
    /// previous element if it is the last thing written
    /// (`addStmtsIfFirstCheck`) —, then the dynamic ones, in order.
    fn write_bindings(&mut self, bindings: Vec<BoundProperty>) {
        let mut constants = Vec::new();
        let mut dynamic = Vec::new();
        for l in bindings {
            match l {
                BoundProperty::Constant(s) => constants.push(s),
                BoundProperty::Dynamic(s) => dynamic.push(s),
            }
        }
        if !constants.is_empty() {
            self.uses_first_check = true;
            in_first_check(&mut self.detection, &constants);
        }
        self.detection.extend(dynamic);
    }

    /// What the binding does with the element, by the prefix of the name.
    ///
    /// Rules of `_UpdateStatementsVisitor` (`update_statement_visitor.dart`)
    /// and of `createElementPropertyAst` (`template_parser.dart`). What changes
    /// the shape and has no case in the corpus yet is refused:
    /// `[style.x.unit]`, a style whose value is not a `String`
    /// (it would emit `.toString()`), a property renamed by the schema
    /// (`readonly` → `readOnly`, `tabindex`) and a property or attribute with a
    /// security context (`href`, `src`, `innerHtml`…), which get the
    /// `sanitize*` around them.
    fn action(
        &mut self,
        l: &crate::html::Binding,
        target: &str,
        value: &str,
        c: &crate::expr::Converted,
    ) -> Result<String, Refusal> {
        // The action goes into `detectChangesInternal`: the import is allocated
        // when change detection is written, not now.
        let dom = deferred(DOM_HELPERS);
        // `[class]`, `[className]` and `[attr.class]` are the same unnamed
        // `ClassBinding` (`_propertyToIr` of `binding_converter`):
        // `updateChildClass`, which keeps the classes of the stylesheet scope.
        Ok(
            // An element that is not HTML (a component, an unknown tag with a
            // directive): the `NonHtml` variants (`isHtmlElement`).
            if matches!(l.name.as_str(), "class" | "className" | "attr.class") {
                let method = if dom::tag_html(&self.current_tag) {
                    "updateChildClass"
                } else {
                    "updateChildClassNonHtml"
                };
                let instance = self.view_instance.as_deref().unwrap_or("this");
                format!("{instance}.{method}({target}, {value})")
            } else if let Some(class) = l.name.strip_prefix("class.") {
                // `[class.x.y]`: the class is `x` (`parts[1]`), the rest is
                // dropped.
                let class = class.split('.').next().unwrap_or(class);
                let method = if dom::tag_html(&self.current_tag) {
                    "updateClassBinding"
                } else {
                    "updateClassBindingNonHtml"
                };
                format!("{dom}.{method}({target}, '{class}', {value})")
            } else if let Some(attr) = l.name.strip_prefix("attr.") {
                // `attr.x.if`: conditional attribute; `attr.ns:x`: with a
                // namespace (`template_parser.dart:73-99`). Any other unit is
                // an error in the official compiler.
                let (attr, unit) = match attr.split_once('.') {
                    Some((a, u)) => (a, Some(u.split('.').next().unwrap_or(u))),
                    None => (attr, None),
                };
                let conditional = match unit {
                    None => false,
                    Some("if") => true,
                    Some(_) => {
                        return Err(refusal(Reason::Binding, "invalid [attr.x.unit]"));
                    }
                };
                if conditional && attr == "class" {
                    return Err(refusal(
                        Reason::Binding,
                        "class.if (official compiler error)",
                    ));
                }
                // The prefix becomes the URI from `namespaceUris`
                // (`view_compiler_utils.dart:21-25`); outside it, `null`, and
                // the attribute is emitted without a namespace
                // (`ir/model.dart:396`).
                let (ns_uri, attr) = match attr.split_once(':') {
                    Some((ns, a)) => (
                        match ns {
                            "xlink" => Some("http://www.w3.org/1999/xlink"),
                            "svg" => Some("http://www.w3.org/2000/svg"),
                            "xhtml" => Some("http://www.w3.org/1999/xhtml"),
                            _ => None,
                        },
                        a,
                    ),
                    None => (None, attr),
                };
                // With a security context the value goes through the context's
                // sanitizer, found by the mapped property name
                // (`securityContext(tag, getMappedPropName(x))` in
                // `createElementPropertyAst`); the written attribute stays as
                // it is.
                let attr_sanitizer = sanitizer(
                    &self.current_tag.to_ascii_lowercase(),
                    mapped_property(attr),
                );
                if attr_sanitizer.is_some() && (conditional || ns_uri.is_some()) {
                    return Err(refusal(
                        Reason::Binding,
                        "sanitized [attr.x.if] or namespaced attribute",
                    ));
                }
                let sanitized = match attr_sanitizer {
                    Some(f) => {
                        let s = deferred(SAFE_HTML);
                        format!("{s}.{f}({value})")
                    }
                    None => value.to_string(),
                };
                // Conditional: `(v ? '' : null)`, always `updateAttribute`
                // (`visitAttributeBinding`, b/171226440).
                let sanitized = if conditional {
                    format!("({sanitized} ? '' : null)")
                } else {
                    sanitized
                };
                let value = sanitized.as_str();
                // `visitAttributeBinding`: `setAttribute` when the source
                // cannot be null (`isNullable` is the `canBeNull` of
                // `analyzed_class.dart`: a primitive literal, and `a ?? b` with
                // one of the sides like that — case j64).
                if let Some(ns) = ns_uri {
                    format!("{dom}.updateAttributeNS({target}, '{ns}', '{attr}', {value})")
                } else {
                    let f = if !c.can_be_null && !conditional {
                        "setAttribute"
                    } else {
                        "updateAttribute"
                    };
                    format!("{dom}.{f}({target}, '{attr}', {value})")
                }
            } else if let Some(style) = l.name.strip_prefix("style.") {
                // `visitStyleBinding`: with a unit, `v == null ? null : v +
                // 'px'` (`v.toString()` if it is not a `String`); without one,
                // the value itself if it is a `String`, otherwise
                // `v.toString()` — `v?.toString()` when `canBeNull`. `isString`
                // is the `_TypeResolver` (`String?` counts).
                let Some(ty) = c.ty.as_deref() else {
                    return Err(refusal(Reason::Binding, "[style.x] of unknown type"));
                };
                let is_string = ty.trim_end_matches('?') == "String";
                let (name, unit) = match style.split_once('.') {
                    Some((_, u)) if u.contains('.') => {
                        return Err(refusal(Reason::Binding, "[style.x.y.z]"));
                    }
                    Some((n, u)) => (n, Some(u)),
                    None => (style, None),
                };
                let value = match unit {
                    Some(u) => {
                        let text = if is_string {
                            value.to_string()
                        } else {
                            format!("{value}.toString()")
                        };
                        format!(
                            "(({value} == null) ? {} : ({text} + {}))",
                            null_style(),
                            literal(u)
                        )
                    }
                    None => unitless_style(value, is_string, c.can_be_null),
                };
                format!("{target}.style.setProperty('{name}', {value})")
            } else if l.name.contains('.') {
                return Err(refusal(Reason::Binding, "binding with an unknown prefix"));
            } else {
                // `getMappedPropName` (`_attrToPropMap`,
                // `dom_element_schema_registry.dart`): only four names change;
                // `class` was already handled above as a `ClassBinding`. A
                // bound `[tabindex]` is `PropertyBinding('tabIndex')`, not the
                // `TabIndexBinding` of the literal
                // (`binding_converter.dart:131`).
                let prop = mapped_property(&l.name);
                // `_sanitizedValue`: the value goes through the context's
                // sanitizer (`[style]` is `*|style`: `sanitizeStyle`, case
                // i92).
                match sanitizer(&self.current_tag.to_ascii_lowercase(), prop) {
                    Some(f) => {
                        let s = deferred(SAFE_HTML);
                        format!("{dom}.setProperty({target}, '{prop}', {s}.{f}({value}))")
                    }
                    None => format!("{dom}.setProperty({target}, '{prop}', {value})"),
                }
            },
        )
    }

    /// The events of an element, as `bindRenderOutputs` binds them: those with
    /// the same name merged into a single handler (`mergeEvents`, in order of
    /// first occurrence), each with its listener at the end of `build()`.
    fn events(&mut self, e: &crate::html::Element, target: &str) -> Result<(), Refusal> {
        let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
        for l in &e.events {
            match groups.iter_mut().find(|(n, _)| *n == l.name) {
                // Two `(click)` written on the same element are an error in the
                // official compiler ("Found multiple events with the same
                // name"); merging only happens with the listeners that come
                // from directives.
                Some(_) => {
                    return Err(refusal(
                        Reason::Event,
                        "two handlers of the same event in the template",
                    ));
                }
                None => groups.push((&l.name, vec![&l.value])),
            }
        }
        for (name, handlers) in groups {
            self.event_type_failure = None;
            self.event_type = native_event_type_q(name);
            match self.handler(&handlers) {
                Ok(h) => self.listener(name, target, &h),
                Err(r) => self.record_refusal(r)?,
            }
        }
        Ok(())
    }

    /// The listener of an event: the element's `addEventListener` for a DOM
    /// event, ngdart's `eventManager` for the rest (`keyup.enter`, a custom
    /// event) — `visitNativeEvent` × `visitCustomEvent`.
    fn listener(&mut self, name: &str, target: &str, handler: &str) {
        let line = if native_event(name) {
            format!(
                "    {target}.addEventListener('{name}', {});",
                native_listener(name, handler)
            )
        } else {
            let utils = deferred_q(APP_VIEW_UTILS);
            format!(
                "    {utils}appViewUtils.eventManager.addEventListener({target}, '{name}', {handler});"
            )
        };
        self.listeners.push(line);
    }

    /// The handler expression of an event (`BoundValueConverter`): a single
    /// simple handler becomes a tear-off (`this.eventHandler0(_ctx.m)`); the
    /// rest — an assignment, arguments, several merged handlers — becomes the
    /// `_handleEvent_N` method of this view, with `eventHandler1`.
    fn handler(&mut self, texts: &[&str]) -> Result<String, Refusal> {
        self.handler_with(texts, &[])
    }

    /// [`Self::handler`] with `extras`: the calls of the nodes' directives'
    /// `@HostListener`s for the same event, after the template actions
    /// (`mergeEvents`) — the handler is always the method (case j70).
    fn handler_with(&mut self, texts: &[&str], extras: &[String]) -> Result<String, Refusal> {
        let scope_locals = self.locals.clone();
        let scope = crate::expr::Scope {
            members: self.members,
            methods: self.methods,
            arities: self.arities,
            locals: &scope_locals,
            types: self.types,
            class: Some(&self.qualified_class),
            exports: Some(self.exports),
            generics: self.generic_names,
        };
        let mut actions = Vec::new();
        for t in texts {
            actions.push(
                crate::expr::convert_action(t, &scope, self.names)
                    .map_err(|r| r.em(Reason::Event))?,
            );
        }
        if let ([crate::expr::Action::Simple { method, arity, .. }], []) =
            (actions.as_slice(), extras)
        {
            self.uses_ctx_in_build = true;
            return Ok(format!("this.eventHandler{arity}(_ctx.{method})"));
        }
        // Complex: the method takes the type of `$event` (in 9).
        if let Some(r) = self.event_type_failure.clone() {
            return Err(r);
        }
        // The statements, with the locals they read declared at the top of the
        // method (the scope of `scopeNamespace`).
        let mut statements = Vec::new();
        let mut locals: Vec<String> = Vec::new();
        for a in &actions {
            match a {
                crate::expr::Action::Simple { statement, .. } => statements.push(statement.clone()),
                crate::expr::Action::Complexa(c) => {
                    statements.push(c.text.clone());
                    for l in &c.locals {
                        if !locals.contains(l) {
                            locals.push(l.clone());
                        }
                    }
                }
            }
        }
        let mut body = Vec::new();
        // The node of a `#ref` read only in an event becomes a field now (case
        // j60).
        for l in &locals {
            if let Some(k) = self.event_el_fields.iter().position(|(n, _)| n == l) {
                let (_, field) = self.event_el_fields.remove(k);
                self.el_fields_by_event.push(field);
            }
        }
        for l in &locals {
            body.push(format!("    {}", self.local_declaration(l, Reason::Event)?));
        }
        if references_ctx(&statements) {
            body.push("    final _ctx = this.ctx;".to_string());
        }
        body.extend(statements.iter().map(|i| format!("    {i};")));
        body.extend(extras.iter().map(|i| format!("    {i};")));
        let n = self.event_methods.len();
        self.event_methods.push(format!(
            "\n  void _handleEvent_{n}({}) {{\n{}\n  }}\n",
            event_parameter(&self.event_type),
            body.join("\n")
        ));
        Ok(format!("this.eventHandler1(this._handleEvent_{n})"))
    }

    /// A component inside the template: the child view is a field, the instance
    /// is another one, and `build()` creates both and binds them.
    ///
    /// The `_5` suffix of the instance field is the `uniqueId` of the provider
    /// on the node (`_instances.length` in `provider_resolver.dart`): on a node
    /// that has only the component, the element's five built-in providers
    /// already take 0..4.
    fn child_component(
        &mut self,
        e: &crate::html::Element,
        child: &Child,
        parent: &str,
    ) -> Result<(), Refusal> {
        let refuse = |f: &str| refusal(Reason::BindingOnChild, f);
        if let Some(r) = child.pending.first() {
            return Err(r.clone());
        }
        // A `[(x)]` that the child itself receives (`@Input x`, `@Output
        // xChange`), desugared like the `DesugarVisitor` does: the input at the
        // end of the properties and the event `x = $event` at the end of the
        // events.
        let mut desugared = e.clone();
        desugared.bananas.clear();
        for b in &e.bananas {
            let change = format!("{}Change", b.name);
            if child.input(&b.name).is_some() && child.output(&change).is_some() {
                desugared.properties.push(b.clone());
                desugared.events.push(crate::html::Binding {
                    name: change,
                    value: format!("{} = $event", b.value),
                    ..b.clone()
                });
            } else {
                desugared.bananas.push(b.clone());
            }
        }
        let e = &desugared;
        // The directives that match the child's element (`NgModel`), and their
        // `[(x)]` desugared as on an HTML element.
        let extras = matched_directives(self.used, e);
        let mut dir_props = Vec::new();
        let mut dir_events = Vec::new();
        for b in &e.bananas {
            let change = format!("{}Change", b.name);
            if !consumes_input(&extras, &b.name) || !consumes_output(&extras, &change) {
                return Err(refuse("[(x)] on the child"));
            }
            dir_props.push(b.clone());
            dir_events.push(crate::html::Binding {
                name: change,
                value: format!("{} = $event", b.value),
                ..b.clone()
            });
        }
        dir_props.extend(
            e.properties
                .iter()
                .filter(|l| consumes_input(&extras, &l.name))
                .cloned(),
        );
        dir_events.extend(
            e.events
                .iter()
                .filter(|l| consumes_output(&extras, &l.name))
                .cloned(),
        );
        // The child's `viewProviders:`: private providers of the node. Without
        // child nodes, they go into the same `ProviderNode` as the others
        // (`createProviderNode`, `childNodeCount == 0`); with them, into a
        // separate one (`[n, n]`), still without a case.
        let no_child_nodes = e.children.iter().all(|x| match x {
            Node::Str(t) => !self.preserve_whitespace && t.trim().is_empty(),
            Node::Comment(_) => true,
            _ => false,
        });
        if !no_child_nodes
            && child
                .metadata
                .as_ref()
                .is_some_and(|m| !m.view_providers.is_empty())
        {
            return Err(refuse("child with viewProviders and child nodes"));
        }
        // The providers of the node: the component first, then the directives.
        // `Visibility.all` puts the child into `injectorGetInternal`: it is the
        // resolved path that writes it.
        let has_providers = !extras.is_empty()
            || child.metadata.as_ref().is_some_and(|m| {
                !m.providers.is_empty() || !m.view_providers.is_empty() || m.visible
            });
        let resolved_node = if has_providers {
            let meta = child
                .metadata
                .clone()
                .ok_or_else(|| refuse("child with providers/directive without metadata"))?;
            let matched = child_node_matched(&meta, &extras, self.used);
            if let Err(f) = writable_providers(&meta) {
                return Err(refuse(&format!("child with providers: {f}")));
            }
            Some(matched)
        } else {
            None
        };
        // A `#ref` on the child is the component instance; with a value, the
        // instance of the node's directive with that `exportAs`
        // (`identifierForReference`, `ast_template_parser.dart:854-868`),
        // chosen further on. With a value, a name that is not read is just a
        // name, as on an HTML element.
        for r in &e.references {
            if !r.value.is_empty() {
                continue;
            }
            if !self.free_refs.contains(&r.name)
                && !self.local_refs.contains(&r.name)
                && !self.queried_refs.iter().any(|(n, _, _)| *n == r.name)
            {
                return Err(refuse(if self.embedded {
                    "#ref on the child in an embedded view"
                } else {
                    "#ref on the child used in an expression"
                }));
            }
        }
        for a in &e.attributes {
            // The literal is the element's `TabIndexBinding` (written with the
            // attributes); the interpolated one would be a property of the
            // child's element, a shape still without a case.
            if a.name.eq_ignore_ascii_case("tabindex") && a.value.contains("{{") {
                return Err(refuse("interpolated tabindex on the child"));
            }
        }
        // What reaches an `@Input`: a static attribute (literal) and `[x]`. A
        // name bound twice is dropped by the official compiler
        // (`_removeExisting`).
        let mut bound: Vec<(&crate::html::Binding, bool)> = e
            .attributes
            .iter()
            .filter(|a| child.input(&a.name).is_some())
            .map(|a| (a, true))
            .collect();
        // A `[x]` that only a directive of the node receives does not belong to
        // the child; one that nobody receives is a property of the child's
        // element (`_visitProperties`: `[class.x]`, `[attr.x]`, `[id]`…; the
        // schema accepts any property on a custom element), written in
        // `bindRenderInputs` (cases j74, j135).
        bound.extend(
            e.properties
                .iter()
                .filter(|l| child.input(&l.name).is_some())
                .map(|l| (l, false)),
        );
        let element_props: Vec<crate::html::Binding> = e
            .properties
            .iter()
            .filter(|l| child.input(&l.name).is_none() && !consumes_input(&extras, &l.name))
            .cloned()
            .collect();
        // An interpolated attribute that neither the child nor a directive of
        // the node receives is a property of the child's element, after its
        // `[x]`s (`_visitProperties`, case j92).
        let element_interpolations: Vec<crate::html::Binding> = e
            .attributes
            .iter()
            .filter(|a| {
                a.value.contains("{{")
                    && child.input(&a.name).is_none()
                    && !consumes_input(&extras, &a.name)
            })
            .cloned()
            .collect();
        for (i, (l, _)) in bound.iter().enumerate() {
            if bound[..i].iter().any(|(x, _)| x.name == l.name) {
                return Err(refuse("child @Input bound twice"));
            }
        }
        let n = self.next_node;
        self.next_node += 1;
        // With a binding of its own, the child's node becomes a field (change
        // detection reads it). The `detectHostChanges(view, el)` of a directive
        // with `@HostBinding` reads the node during change detection.
        let has_host = extras.iter().any(|d| !d.host_bindings.is_empty());
        let el = if element_props.is_empty() && element_interpolations.is_empty() && !has_host {
            format!("_el_{n}")
        } else {
            let ty = self.imp.dom("HtmlElement");
            self.el_fields.push(format!("  late final {ty} _el_{n};"));
            format!("this._el_{n}")
        };
        let outside_lib = || refusal(Reason::ComponentInTemplate, "child without an import path");
        let template_path = import_path(
            &self.asset,
            &asset_of_uri(&child.uri_template, "", Path::new("")).ok_or_else(outside_lib)?,
        )
        .ok_or_else(outside_lib)?;
        let dart_path = import_path(
            &self.asset,
            &asset_of_uri(&child.uri_dart, "", Path::new("")).ok_or_else(outside_lib)?,
        )
        .ok_or_else(outside_lib)?;
        // A child from the same file: its view is right beside it, without an
        // import.
        let vt = if is_own_template(&self.asset, &template_path) {
            String::new()
        } else {
            self.imp.q(&template_path)
        };
        let vd = self.imp.alias(&dart_path);
        let class = &child.class;
        let view_field = format!("_compView_{n}");
        // A child that injects `ViewContainerRef`: the node gets a
        // `ViewContainer` (a field between the view and the instance) and three
        // more built-ins, which take the instance to 8 (case j47).
        let container = child
            .params
            .iter()
            .any(|p| matches!(p, Injected::Container))
            || extras
                .iter()
                .any(|d| crate::directives::requests_container(d));
        // The providers of the node, in the official compiler's order: the
        // directives in `directives:` order with the child in its place
        // (`_matchDirectives`), each eager provider after its dependencies
        // (`_getOrCreateLocalProvider`) — what the child injects from its own
        // node and the directives that come before it are emitted before the
        // instance (cases j61, j67). The reads from above take the import of
        // the deferred `unsafeCast`, allocated where the text is emitted.
        let resolved = match &resolved_node {
            Some((index, matched)) => {
                let providers = self.providers_above_with(&deferred(UTILITIES));
                let above = crate::directives::Above {
                    providers: &providers,
                    uncertain: self.uncertain_above > 0,
                    of_view: self.view_tokens,
                };
                let requests = requests_to_child_node(e, matched, self.children, self.used)?;
                let r = crate::directives::resolve_child_node(
                    matched,
                    *index,
                    n,
                    Some(above),
                    container,
                    &requests,
                )
                .map_err(|f| refusal(Reason::DirectiveBySelector, f))?;
                let token = matched[*index].token();
                let pos = r
                    .instances
                    .iter()
                    .position(|i| i.token == token)
                    .ok_or_else(|| refuse("child node without the child instance"))?;
                Some((r, pos))
            }
            None => None,
        };
        let inst_field = match &resolved {
            Some((r, pos)) => r.instances[*pos].field.clone(),
            None => format!(
                "_{class}_{n}_{}",
                crate::dialect::extra_builtins() + if container { 8 } else { 5 }
            ),
        };
        // The directives of the node before and after the child, in the order
        // of the bindings and of the hooks (`transformedDirectiveAsts`).
        let (dirs_before, dirs_after) = match &resolved {
            Some((r, _)) => {
                let k = r
                    .directives
                    .iter()
                    .position(|(_, c)| *c == inst_field)
                    .ok_or_else(|| refuse("child node without the child directive"))?;
                (r.directives[..k].to_vec(), r.directives[k + 1..].to_vec())
            }
            None => (Vec::new(), Vec::new()),
        };
        // `directiveTypes:` types the fields (the view and the instance), not
        // the creation (`lookupTypeArgumentsOf`, `compile_view.dart:891-905`).
        let type_args = self.type_arguments(&child.uri_dart, &child.class, e);
        self.child_fields.push(format!(
            "  late final {vt}View{class}0{type_args} {view_field};"
        ));
        if container {
            let vc = self.imp.q(VIEW_CONTAINER);
            self.child_fields
                .push(format!("  late final {vc}ViewContainer _appEl_{n};"));
        }
        self.child_views.push(view_field.clone());
        if child.detection_link {
            self.linked_views.push(view_field.clone());
        }
        if child.host {
            self.uses_first_check = true;
        }
        self.lines.push(format!(
            "    this.{view_field} = {vt}View{class}0(this, {n});"
        ));
        self.lines.push(format!(
            "    {} = this.{view_field}.rootElement;",
            if el.starts_with("this.") {
                el.clone()
            } else {
                format!("final {el}")
            }
        ));
        // At the root of the embedded view, or when projected, the node has no
        // parent here (with a `ViewContainer`, it is the root: `vcAppEl ??
        // renderNode`).
        if parent.is_empty() {
            self.roots.push(if container {
                format!("this._appEl_{n}")
            } else {
                el.clone()
            });
        } else {
            self.lines.push(format!("    {parent}.append({el});"));
        }
        // The written attributes, in alphabetical order (`_toSortedBindings`),
        // all of them — also the ones that feed an `@Input` —, and `class`
        // through `updateChildClassNonHtml`: the child's element is not HTML
        // (`writeLiteralAttributeValues`).
        // The interpolated one is a binding, not a written attribute (case
        // j58).
        let mut attributes: Vec<_> = e
            .attributes
            .iter()
            .filter(|a| !a.value.contains("{{"))
            .cloned()
            .collect();
        attributes.sort_by(|a, b| a.name.cmp(&b.name));
        for a in &attributes {
            let mut value = literal(&a.value);
            // `_mergeHtmlAndDirectiveAttrs`: the component's static `class` is
            // only written here when the element also has a `class`; the two
            // become an interpolation (the component's one stays in the
            // constructor of its view, and this one overwrites it at run time).
            // A merged `style` has no case yet; the others keep the written
            // attribute.
            if let Some((_, member)) = child
                .host_attributes
                .iter()
                .find(|(name, _)| *name == a.name)
            {
                match a.name.as_str() {
                    "class" => {
                        let interp = self.imp.q(INTERPOLATE);
                        value = format!(
                            "{interp}interpolate2('', {value}, ' ', {vd}.{class}.{member}, '')"
                        );
                    }
                    "style" => {
                        return Err(refuse("static child style merged with the written one"));
                    }
                    _ => {}
                }
            }
            if a.name == "class" {
                self.lines
                    .push(format!("    this.updateChildClassNonHtml({el}, {value});"));
            } else if a.name == "tabindex" || a.name == "tabIndex" {
                // `TabIndexBinding`, as on an HTML element (case j102).
                match a.value.trim().parse::<i64>() {
                    Ok(n) if a.value.trim() == a.value => {
                        self.lines.push(format!("    {el}.tabIndex = {n};"));
                    }
                    _ => return Err(refuse("non-integer tabindex on the child")),
                }
            } else {
                let dom = self.dom();
                self.lines.push(format!(
                    "    {dom}.setAttribute({el}, '{}', {value});",
                    a.name
                ));
            }
        }
        if self.with_style {
            self.lines.push(format!("    this.addShimC({el});"));
        }
        // The `ViewContainer` is born with the `CompileElement`, before the
        // providers; the second argument is the parent (`null` at the root).
        if container {
            let vc = self.imp.q(VIEW_CONTAINER);
            let parent_index = if parent.is_empty() {
                self.projected_parent
                    .map_or_else(|| "null".to_string(), |k| k.to_string())
            } else {
                element_index(parent)
            };
            self.lines.push(format!(
                "    this._appEl_{n} = {vc}ViewContainer({n}, {parent_index}, this, {el});"
            ));
            self.anchors.push(format!("_appEl_{n}"));
        }
        let before_child = resolved
            .as_ref()
            .map_or(&[][..], |(r, pos)| &r.instances[..*pos]);
        self.create_instances(e, before_child, &el.clone(), &format!("this.{view_field}"))?;
        self.child_fields.push(format!(
            "  late final {vd}.{class}{type_args} {inst_field};"
        ));
        let from_node: Vec<(crate::directives::Token, String)> = before_child
            .iter()
            .flat_map(|i| {
                std::iter::once(&i.token)
                    .chain(&i.aliases)
                    .map(|t| (t.clone(), i.read.clone()))
            })
            .collect();
        let creation = self.child_construction(e, child, n, &el, &vd, &view_field, &from_node)?;
        self.lines
            .push(format!("    this.{inst_field} = {creation};"));
        let mut above = self.stack.clone();
        above.push((n, true));
        self.registrations.push(Registration {
            above,
            providers: vec![(
                crate::directives::Token::Class {
                    uri: child.uri_dart.clone(),
                    class: child.class.clone(),
                },
                inst_field.clone(),
                child.on_push.then(|| view_field.clone()),
            )],
            element: el.clone(),
        });
        // The target of each `#ref`: without a value, the component; with a
        // value, the only directive of the node (the component included) with
        // an equal `exportAs` — its field already carries the `.instance` of an
        // `XNgCd`.
        let mut component_refs = Vec::new();
        for r in &e.references {
            let read = if r.value.is_empty() {
                component_refs.push(r.name.clone());
                format!("this.{inst_field}")
            } else {
                let found: Vec<&String> = match &resolved {
                    Some((res, _)) => res
                        .directives
                        .iter()
                        .filter(|(d, _)| d.export_as.as_deref() == Some(r.value.as_str()))
                        .map(|(_, c)| c)
                        .collect(),
                    None => child
                        .metadata
                        .iter()
                        .filter(|m| m.export_as.as_deref() == Some(r.value.as_str()))
                        .map(|_| &inst_field)
                        .collect(),
                };
                match found.as_slice() {
                    [c] => {
                        if **c == inst_field {
                            component_refs.push(r.name.clone());
                        }
                        format!("this.{c}")
                    }
                    [] => return Err(refuse("#ref with a value no directive exports")),
                    _ => {
                        return Err(refuse(
                            "#ref with a value exported by more than one directive",
                        ));
                    }
                }
            };
            self.refs.insert(r.name.clone(), read.clone());
            self.refs_in_order.push((r.name.clone(), read));
        }
        // The result of a `@ViewChild(Type)` ([`type_key`]), also read
        // from a view above (dynamic query).
        let key = type_key(&child.uri_dart, &child.class);
        self.refs
            .entry(node_key(&key, e.start))
            .or_insert_with(|| format!("this.{inst_field}"));
        if let Some((r, _)) = &resolved {
            let instances = r.instances.clone();
            self.register_node_by_position(e.start, &instances);
        }
        self.refs.insert(key.clone(), format!("this.{inst_field}"));
        self.refs_in_order
            .push((key.clone(), format!("this.{inst_field}")));
        if child.on_push {
            // The result of a dynamic content query by position.
            self.detectors
                .insert(node_key(&key, e.start), view_field.clone());
            self.detectors_in_order
                .push((key.clone(), view_field.clone()));
            self.detectors.insert(key, view_field.clone());
        }
        // `buildChangeDetectorRef()` only exists for an `onPush` component: a
        // `#ref` whose value is a directive does not register a detector.
        for name in &component_refs {
            if child.on_push {
                self.detectors_in_order
                    .push((name.clone(), view_field.clone()));
                self.detectors.insert(name.clone(), view_field.clone());
            }
        }
        // `bindRenderInputs`: the bindings of the child's own element, before
        // the directives' ones (case j74).
        if !element_props.is_empty() || !element_interpolations.is_empty() {
            self.current_tag = e.name.clone();
            self.view_instance = Some(format!("this.{view_field}"));
            let mut el_bindings = Vec::new();
            let mut failure = None;
            for l in &element_props {
                match self.property(l, &el) {
                    Ok(x) => el_bindings.push(x),
                    Err(r) => {
                        failure = Some(r);
                        break;
                    }
                }
            }
            if failure.is_none() {
                for a in &element_interpolations {
                    match self.interpolated_attribute(a, &el) {
                        Ok(x) => el_bindings.push(x),
                        Err(r) => {
                            failure = Some(r);
                            break;
                        }
                    }
                }
            }
            self.view_instance = None;
            if let Some(r) = failure {
                self.record_refusal(r)?;
            }
            self.write_bindings(el_bindings);
        }
        self.host_view = Some(format!("this.{view_field}"));
        if let Some((r, _)) = &resolved {
            self.bind_directives(
                e,
                &r.instances,
                &dirs_before,
                &el.clone(),
                &dir_props,
                &dir_events,
                true,
                false,
            )?;
        }
        self.host_view = None;
        self.child_inputs(&bound, child, &inst_field, &view_field)?;
        // The component's `bindDirectiveHostProps`: with the property bindings,
        // in document order, right after its inputs — not together with the
        // `detectChanges` of its view.
        if child.host {
            self.detection.push(format!(
                "    this.{view_field}.detectHostChanges(firstCheck);"
            ));
        }
        let mut without_dir_outputs = e.clone();
        without_dir_outputs
            .events
            .retain(|l| !consumes_output(&extras, &l.name));
        // `bindRenderOutputs` (the element's events) before the outputs of the
        // directives, which follow their order.
        self.child_element_events(
            &without_dir_outputs,
            child,
            &el,
            resolved.as_ref().map(|(r, _)| (extras.as_slice(), r)),
        )?;
        if let Some((r, _)) = &resolved {
            self.bind_directives(
                e,
                &r.instances,
                &dirs_before,
                &el.clone(),
                &dir_props,
                &dir_events,
                false,
                true,
            )?;
        }
        self.child_outputs(&without_dir_outputs, child, &inst_field)?;
        // The other providers of the node (a value accessor from the child's
        // `providers:`, `NgModel`): fields after the instance, inputs and
        // outputs after the child's ones (`transformedDirectiveAsts`).
        let mut child_injector = None;
        let above_len = self.above.len();
        if let Some((r, pos)) = &resolved {
            let pos = *pos;
            // The lazy ones that the content asks for already come eager and in
            // the right position from the resolver
            // ([`requests_to_child_node`]).
            let after = r.instances[pos + 1..].to_vec();
            // The providers after the child, the `registerDirective` of all the
            // directives of the node (except the component) and the bindings of
            // the ones that come after it.
            let target = el.clone();
            self.create_instances(e, &after, &target, &format!("this.{view_field}"))?;
            let all: Vec<_> = dirs_before.iter().chain(&dirs_after).cloned().collect();
            self.register_directives(&all, &target);
            self.host_view = Some(format!("this.{view_field}"));
            let bound = self.bind_directives(
                e,
                &r.instances,
                &dirs_after,
                &target,
                &dir_props,
                &dir_events,
                true,
                true,
            );
            self.host_view = None;
            bound?;
            // The child goes first into `injectorGetInternal`, by its aliases
            // (`ExistingProvider(X, TheChild)`) and, when visible, by its
            // class.
            let injectables: Vec<(Vec<crate::directives::Token>, String)> = r
                .instances
                .iter()
                .filter(|i| !i.injectable_by.is_empty())
                .map(|i| (i.injectable_by.clone(), i.read.clone()))
                .collect();
            if !injectables.is_empty() {
                child_injector = Some(self.injectors.len());
                self.injectors.push((n, n, injectables));
            }
            for i in &r.instances {
                // Visibility only decides `injectorGetInternal` (other views);
                // in the same template a node below finds even a local provider
                // by its token (case j41).
                if i.lazy {
                    self.lazy_above
                        .insert((i.read.clone(), self.this_view_class()));
                    self.lazy_with_requests
                        .insert((i.read.clone(), self.this_view_class()));
                }
                if i.injectable_by.is_empty() {
                    self.above.push((i.token.clone(), i.read.clone(), None));
                }
                for t in &i.injectable_by {
                    self.above.push((t.clone(), i.read.clone(), None));
                }
            }
            let mut reg_above = self.stack.clone();
            reg_above.push((n, true));
            // The child's token is already in its record; its aliases, here.
            let providers = r
                .instances
                .iter()
                .flat_map(|i| {
                    (i.field != inst_field)
                        .then_some(&i.token)
                        .into_iter()
                        .chain(&i.aliases)
                        .map(|t| (t.clone(), i.read.clone(), None))
                })
                .collect();
            self.registrations.push(Registration {
                above: reg_above,
                providers,
                element: el.clone(),
            });
        } else if let Some(meta) = &child.metadata {
            // Without `providers:`, another directive or `Visibility.all`, the
            // child's node only provides its class to the content of the same
            // template.
            self.above.push((meta.token(), inst_field.clone(), None));
        }
        if child.projections.is_empty() && e.children.is_empty() {
            self.directive_queries(e, &dirs_before, n)?;
            self.child_queries(e, child, &inst_field, n)?;
            self.directive_queries(e, &dirs_after, n)?;
            self.hooks_after_children(&dirs_before);
            self.after_children(child, &inst_field);
            self.hooks_after_children(&dirs_after);
            self.lines
                .push(format!("    this.{view_field}.create(this.{inst_field});"));
            self.above.truncate(above_len);
            return Ok(());
        }
        // Projected content: the nodes are created detached and each one goes
        // to the projection whose selector matches it (`findNgContentIndex`:
        // the lowest index that matches, otherwise the `*`).
        let selectors: Vec<Option<Vec<crate::selector::Selector>>> = child
            .projections
            .iter()
            .map(|s| (s != "*").then(|| crate::selector::Selector::parse(s)))
            .collect();
        let wildcard = child.projections.iter().position(|s| s == "*");
        let mut lists: Vec<Vec<String>> = vec![Vec::new(); child.projections.len()];
        self.children_above
            .push((child.uri_dart.clone(), child.class.clone()));
        let parent_before = self.projected_parent.replace(n);
        self.stack.push((n, true));
        self.components_above += 1;
        let uncertain = child.metadata.is_none();
        if uncertain {
            self.uncertain_above += 1;
        }
        let mut r = Ok(());
        for node in &e.children {
            let index = match node {
                Node::Comment(_) => continue,
                Node::Element(x) => {
                    let mut without_star = x.clone();
                    without_star.star = None;
                    let desc = crate::selector::Element::of_template(&without_star);
                    selectors
                        .iter()
                        .position(|s| {
                            s.as_ref()
                                .is_some_and(|s| crate::selector::matches_any(s, &desc))
                        })
                        .or(wildcard)
                }
                _ => wildcard,
            };
            // Content that no projection receives (null `ngContentIndex`; a
            // child without `<ng-content>` receives nothing): text, bound or
            // not, is skipped and only consumes the node index
            // (`_maybeSkipNode`, the official compiler warns "Dead code in
            // template"); the element and the anchor of a `<template>` are
            // created and discarded (`addContentNode` is not called) — their
            // directives exist and take part in the queries.
            let discarded = match (index, node) {
                (Some(_), _) => false,
                (None, Node::Str(_) | Node::Interpolation { .. }) => {
                    self.next_node += 1;
                    continue;
                }
                (None, Node::Element(x)) if x.name != "ng-container" || x.star.is_some() => true,
                (None, _) => {
                    r = Err(refusal(
                        Reason::Projection,
                        "<ng-container> that no projection receives",
                    ));
                    break;
                }
            };
            let before = self.roots.len();
            if let Err(x) = self.nodes(std::slice::from_ref(node), "") {
                r = Err(x);
                break;
            }
            let created: Vec<String> = self.roots.drain(before..).collect();
            if let (false, Some(i)) = (discarded, index) {
                lists[i].extend(created);
            }
        }
        self.projected_parent = parent_before;
        self.children_above.pop();
        self.stack.pop();
        self.components_above -= 1;
        if uncertain {
            self.uncertain_above -= 1;
        }
        r?;
        self.directive_queries(e, &dirs_before, n)?;
        self.child_queries(e, child, &inst_field, n)?;
        self.directive_queries(e, &dirs_after, n)?;
        self.hooks_after_children(&dirs_before);
        self.after_children(child, &inst_field);
        self.hooks_after_children(&dirs_after);
        // Without projections: `create`, like the child without content. All
        // empty: a single line, constants. Otherwise, one list per line (`dart
        // format` breaks the list that has another non-empty one).
        let text = if lists.is_empty() {
            format!("    this.{view_field}.create(this.{inst_field});")
        } else if lists.iter().all(Vec::is_empty) {
            let empties = vec!["const <Object>[]"; lists.len()];
            format!(
                "    this.{view_field}.createAndProject(this.{inst_field}, [{}]);",
                empties.join(", ")
            )
        } else {
            // Each list through `createFlatArrayForProjectNodes` (a reprojected
            // `<ng-content>` is a whole list, case j57).
            let reprojects = lists.iter().flatten().any(|x| x.starts_with(LIST_ROOT));
            let util = if reprojects {
                self.imp.alias(UTILITIES)
            } else {
                String::new()
            };
            // In the broken list, each item starts at column 6.
            let mut items: Vec<String> = Vec::new();
            for l in &lists {
                items.push(flat_list(l, &util)?.text_at(6));
            }
            // `dart format` breaks the outer list only when an item is a
            // non-empty collection literal (the reprojected list alone is not).
            if items
                .iter()
                .any(|t| t.starts_with("<Object>[") && t != "<Object>[]")
            {
                let lines: Vec<String> = items.iter().map(|t| format!("      {t}")).collect();
                format!(
                    "    this.{view_field}.createAndProject(this.{inst_field}, [\n{}\n    ]);",
                    lines.join(",\n")
                )
            } else {
                format!(
                    "    this.{view_field}.createAndProject(this.{inst_field}, [{}]);",
                    items.join(", ")
                )
            }
        };
        self.lines.push(text);
        self.above.truncate(above_len);
        // `ProviderNode(nodeIndex, nodeIndex + childNodeCount)`.
        if let Some(i) = child_injector {
            self.injectors[i].1 = self.next_node - 1;
        }
        Ok(())
    }

    /// The view from which the outer injector is read
    /// (`injectFromViewParentInjector` written in the view of the topmost node
    /// of the injector chain and brought to this one by `getPropertyInView`):
    /// `this`, or the `parentView` chain up to it (cases i76, j40).
    fn injector_view(&self) -> String {
        let hops = self.depth - self.top_level;
        let mut v = "this".to_string();
        for k in 0..hops {
            v = if k == 0 {
                "(this.parentView!)".to_string()
            } else {
                format!("({v}.parentView!)")
            };
        }
        v
    }

    /// The construction of the child instance, the text after `this._X_n_5 = `
    /// (without the `;`): the node, the child's view and the services through
    /// the view above. With a service, the official compiler wraps it in
    /// `debugInjectorWrap` under `isDevMode`, as in the host view.
    fn child_construction(
        &mut self,
        e: &crate::html::Element,
        child: &Child,
        n: u32,
        el: &str,
        vd: &str,
        view_field: &str,
        from_node: &[(crate::directives::Token, String)],
    ) -> Result<String, Refusal> {
        let refuse = |f: &str| refusal(Reason::BindingOnChild, f);
        let class = &child.class;
        let from_own_node = |token: &crate::directives::Token| {
            from_node
                .iter()
                .find_map(|(t, c)| (t == token).then_some(c))
        };
        // The providers of the elements above (`_getDependency` walks up
        // through them before the outer injector), with the import of the
        // deferred `unsafeCast`.
        let providers_above = self.providers_above_with(&deferred(UTILITIES));
        let from_above =
            |token: &crate::directives::Token| providers_above.iter().find(|p| p.token == *token);
        // The class of this view's component: `@Host()` finds it in the
        // injector (`identifierToken(component.type).equalsTo(dep.token)`).
        let is_own_component = |token: &crate::directives::Token| match token {
            crate::directives::Token::Class { uri, class: ty } => {
                self.qualified_class.rsplit('.').next() == Some(ty.as_str())
                    && asset_of_uri(uri, "", Path::new("")).as_deref() == Some(self.asset.as_str())
            }
            _ => false,
        };
        // `_getDependency`: the node itself (without `@SkipSelf`); with
        // `@Self`, only it; otherwise the elements above; and, if not found,
        // the outer injector — except with `@Host` in a component view (unless
        // the token is the component itself), where it is `null` if
        // `@Optional`.
        let mut sources = Vec::new();
        for p in &child.params {
            let Injected::Service {
                token,
                optional,
                self_only,
                host,
                skip_self,
            } = p
            else {
                sources.push(None);
                continue;
            };
            if !skip_self && let Some(c) = from_own_node(token) {
                sources.push(Some(ServiceOrigin::Local(c.clone())));
                continue;
            }
            if *self_only {
                if !optional {
                    return Err(refuse("@Self() without a provider on the child node"));
                }
                sources.push(Some(ServiceOrigin::Null));
                continue;
            }
            // An element built-in: the official compiler finds it on the node,
            // never in the outer injector (still without a case on the child
            // node).
            if crate::directives::element_builtin(token) {
                return Err(refuse(
                    "child that injects an element built-in (ViewContainerRef…)",
                ));
            }
            // An element above provides the service (also a component above,
            // whose content contains the child): the official compiler reads it
            // from there — the field, or the `.instance` of the `XNgCd` (cases
            // j68, j79).
            if let Some(p) = from_above(token) {
                // A lazy provider of the element above: the official compiler
                // would transform it now, changing the index and the shape
                // there (L1).
                if p.lazy {
                    return Err(refuse(
                        "lazy provider of an element above requested below (L1)",
                    ));
                }
                sources.push(Some(ServiceOrigin::Above(p.read.clone())));
                continue;
            }
            // A component above without metadata could provide the service: not
            // finding it does not prove that it comes from outside.
            if self.uncertain_above > 0 {
                return Err(refuse(
                    "child that injects a service under a component without metadata",
                ));
            }
            if *host && !is_own_component(token) && !self.view_tokens.contains(token) {
                if !optional {
                    return Err(refuse("@Host() without a provider in the child view"));
                }
                sources.push(Some(ServiceOrigin::Null));
                continue;
            }
            sources.push(Some(ServiceOrigin::Injector));
        }
        // Only the injector dependency is dynamic (`hasDynamicDependencies`):
        // one that the node itself or an element above provides does not wrap
        // the creation.
        let injects = sources
            .iter()
            .any(|o| matches!(o, Some(ServiceOrigin::Injector)));
        // The order of the imports is the writing order: `isDevMode`,
        // `errors.dart`, the class and the injected types.
        let prefix = if injects {
            let util = self.imp.alias(UTILITIES);
            let errors = self.imp.alias(DI_ERRORS);
            Some((util, errors))
        } else {
            None
        };
        // `injectFromViewParentInjector` written in the node's view and brought
        // (`getPropertyInView`) to the view of the topmost node of the injector
        // chain: `parentView.injectorGet(T, parentIndex)` as seen from there.
        let component_view = self.injector_view();
        let mut args = Vec::new();
        for (p, source) in child.params.iter().zip(&sources) {
            match p {
                Injected::Element => args.push(el.to_string()),
                Injected::Detector => args.push(format!("this.{view_field}")),
                Injected::Container => args.push(format!("this._appEl_{n}")),
                // The literal value of the attribute on the element
                // (`_attrs[name]`), or `null`.
                Injected::Attribute(name) => args.push(
                    e.attributes
                        .iter()
                        .find(|a| a.name == *name && !a.value.contains("{{"))
                        .map_or_else(|| "null".to_string(), |a| literal(&a.value)),
                ),
                Injected::Service {
                    token, optional, ..
                } => {
                    match source {
                        Some(ServiceOrigin::Local(c)) => {
                            args.push(format!("this.{c}"));
                            continue;
                        }
                        Some(ServiceOrigin::Above(l)) => {
                            args.push(l.clone());
                            continue;
                        }
                        Some(ServiceOrigin::Null) => {
                            args.push("null".to_string());
                            continue;
                        }
                        _ => {}
                    }
                    let expr = match token {
                        crate::directives::Token::Class { uri, class: ty } => {
                            let path = asset_of_uri(uri, "", Path::new(""))
                                .and_then(|target| import_path(&self.asset, &target))
                                .ok_or_else(|| {
                                    refuse("type injected into the child without an import path")
                                })?;
                            format!("{}{ty}", self.imp.q(&path))
                        }
                        // `const OpaqueToken<T>('x')`, with the imports in text
                        // order.
                        other => resolve_deferred(self.imp, &token_expr(other)),
                    };
                    let method = if *optional {
                        "injectorGetOptional"
                    } else {
                        "injectorGet"
                    };
                    let v = &component_view;
                    args.push(format!(
                        "({v}.parentView!).{method}({expr}, {v}.parentIndex)"
                    ));
                }
            }
        }
        let call = resolve_deferred(self.imp, &format!("{vd}.{class}({})", args.join(", ")));
        Ok(match prefix {
            None => call,
            Some((util, errors)) => format!(
                "({util}.isDevMode\n        ? {errors}.debugInjectorWrap({vd}.{class}, () {{\n            return {call};\n          }})\n        : {call})"
            ),
        })
    }

    /// The child's content queries, in the node's `afterChildren`
    /// (`updateQueryAtStartup`). The static result is that of the instances
    /// created in the content that match the searched type, in pre-order
    /// (`addQueryResult`), filtered by the distance of `_getQueriesFor` when
    /// the query is not `descendants`: the list gets `[a, b]` (or `[]`), the
    /// single one nothing when empty. A result inside a `*` (`mapNestedViews`),
    /// a found `#ref` and a single query with a result are still refused.
    fn child_queries(
        &mut self,
        e: &crate::html::Element,
        child: &Child,
        inst_field: &str,
        n: u32,
    ) -> Result<(), Refusal> {
        self.content_queries_on_node(e, &child.queries, inst_field, n)
    }

    /// `updateQueryAtStartup` of the content queries of the `directives` of
    /// node `n`, in its `afterChildren`, in their order.
    fn directive_queries(
        &mut self,
        e: &crate::html::Element,
        directives: &[(std::sync::Arc<crate::directives::Directive>, String)],
        n: u32,
    ) -> Result<(), Refusal> {
        for (d, field) in directives {
            if !d.content_queries.is_empty() {
                self.content_queries_on_node(e, &d.content_queries, field, n)?;
            }
        }
        Ok(())
    }

    /// The content queries of a directive or of a child on node `n`, with the
    /// instance read through `inst_field` ([`Self::child_queries`]; the
    /// directive, case j72).
    fn content_queries_on_node(
        &mut self,
        e: &crate::html::Element,
        queries: &[ChildQuery],
        inst_field: &str,
        n: u32,
    ) -> Result<(), Refusal> {
        let refuse = |f: &str| refusal(Reason::BindingOnChild, f);
        for q in queries {
            // The node's `_queryCount`: every content query of it counts.
            let index = {
                let k = self.queries_per_node.entry(n).or_default();
                *k += 1;
                *k - 1
            };
            let (uri, class) = match &q.target {
                QueryTarget::Reference(r) => {
                    if ref_in_content(&e.children, r) {
                        return Err(refuse("child @ContentChild with a #ref in the content"));
                    }
                    if q.list {
                        self.lines
                            .push(format!("    this.{inst_field}.{} = [];", q.field));
                    }
                    continue;
                }
                QueryTarget::Class(uri, class) => (uri, class),
            };
            if self.class_in_embedded(&e.children, uri, class, false) {
                // Without `descendants`, the `<template>` (with a directive)
                // between the result and the node already puts the distance
                // above 1.
                if !q.descendants || q.read.is_some() {
                    return Err(refuse(
                        "child @ContentChild with a result in an embedded view",
                    ));
                }
                let tree = self.content_tree(&e.children, uri, class)?;
                // The single one with the first result in this view is static
                // (`_shouldMapNestedViews`): it goes on below, through the
                // first one.
                let dynamic = if q.list {
                    tree.iter().any(|i| matches!(i, ContentItem::Nested { .. }))
                } else {
                    matches!(tree.first(), Some(ContentItem::Nested { .. }))
                };
                if dynamic {
                    self.dynamic_content_query(
                        tree,
                        type_key(uri, class),
                        class,
                        n,
                        index,
                        q.list,
                        format!("this.{inst_field}.{}", q.field),
                        false,
                    );
                    continue;
                }
            }
            let token = crate::directives::Token::Class {
                uri: uri.clone(),
                class: class.clone(),
            };
            let mut values = Vec::new();
            // The results that are an `onPush` component: the
            // `ChangeDetectorRef` of each one is registered before the
            // assignment (`_createAddQueryChangeDetectorRefs` in
            // `_createUpdates`).
            let mut detectors: Vec<(String, String)> = Vec::new();
            for r in &self.registrations {
                let Some(pos) = r.above.iter().position(|(k, _)| *k == n) else {
                    continue;
                };
                let distance = r.above[pos + 1..].iter().filter(|(_, d)| *d).count();
                if !q.descendants && distance > 1 {
                    continue;
                }
                for (t, field, on_push_view) in &r.providers {
                    if *t != token {
                        continue;
                    }
                    // With `read:`, the value comes from another provider,
                    // without a `ChangeDetectorRef`.
                    if let (Some(cv), None) = (on_push_view, &q.read) {
                        detectors.push((format!("this.{field}"), cv.clone()));
                    }
                    values.push(match &q.read {
                        None => format!("this.{field}"),
                        // `read:` reads another token of the same node.
                        Some(QueryRead::Element) => r.element.clone(),
                        Some(QueryRead::Class(u, c)) => {
                            let read = crate::directives::Token::Class {
                                uri: u.clone(),
                                class: c.clone(),
                            };
                            let node = r.above.last().map(|(k, _)| *k);
                            self.registrations
                                .iter()
                                .filter(|x| x.above.last().map(|(k, _)| *k) == node)
                                .flat_map(|x| &x.providers)
                                .find(|(t, _, _)| *t == read)
                                .map(|(_, c, _)| format!("this.{c}"))
                                .ok_or_else(|| {
                                    refuse(
                                        "@ContentChild(.., read:) token absent from the found node",
                                    )
                                })?
                        }
                    });
                }
            }
            // The single one keeps only the first value (`_buildQueryResults`
            // stops at the first static one), and only its registration.
            if !q.list {
                detectors.retain(|(v, _)| Some(v) == values.first());
            }
            if !detectors.is_empty() {
                let v = self.imp.alias(VIEW);
                for (value, cv) in &detectors {
                    self.lines.push(format!(
                        "    {v}.View.queryChangeDetectorRefs[{value}] = this.{cv};"
                    ));
                }
            }
            if q.list {
                self.lines.push(format!(
                    "    this.{inst_field}.{} = [{}];",
                    q.field,
                    values.join(", ")
                ));
            } else if let Some(first) = values.first() {
                // The single one gets the first, in pre-order; without a
                // result, nothing.
                self.lines
                    .push(format!("    this.{inst_field}.{} = {first};", q.field));
            }
        }
        Ok(())
    }

    /// Does some element inside a `*` of the content match the searched class
    /// (a component by its tag or a directive by its selector)?
    fn class_in_embedded(&self, nodes: &[Node], uri: &str, class: &str, inside: bool) -> bool {
        nodes.iter().any(|node| {
            let Node::Element(x) = node else { return false };
            // The element of a `*` goes into the embedded view; a written
            // `<template>` stays in this one, and only its content belongs to
            // the embedded one.
            let is_template = x.star.as_ref().is_some_and(|l| l.name == TEMPLATE_MARK);
            let below = inside || x.star.is_some();
            let inside = inside || (x.star.is_some() && !is_template);
            let mut stripped = x.clone();
            stripped.star = None;
            // The token also matches through the node's `providers:` (the
            // `ExistingProvider` of a directive, the `FocusableItem` of
            // `FocusItemDirective`): `_getQueriesFor` looks at all the resolved
            // providers of the node.
            let token = crate::directives::Token::Class {
                uri: uri.to_string(),
                class: class.to_string(),
            };
            let provides = |d: &crate::directives::Directive| {
                (d.uri == uri && d.class == class) || d.providers.iter().any(|p| p.token == token)
            };
            let matches = self.children.get(&x.name).is_some_and(|f| {
                (f.uri_dart == uri && f.class == class)
                    || f.metadata.as_deref().is_some_and(provides)
            }) || matched_directives(self.used, &stripped)
                .iter()
                .any(|d| (d.uri == uri && d.class == class) || provides(d));
            (inside && matches) || self.class_in_embedded(&x.children, uri, class, below)
        })
    }

    /// `bindDirectiveAfterChildrenCallbacks` of each directive of the node, in
    /// their order, after the children (bottom-up): `ngAfterContent*` and
    /// `ngAfterView*` (`Init` in an `if (firstCheck)`, `addStmtsIfFirstCheck`)
    /// and `ngOnDestroy` (case j65).
    fn hooks_after_children(
        &mut self,
        directives: &[(std::sync::Arc<crate::directives::Directive>, String)],
    ) {
        for (d, field) in directives {
            let g = &d.hooks;
            if g.after_content_init {
                self.uses_first_check = true;
                in_first_check(
                    &mut self.after_content,
                    &[format!("      this.{field}.ngAfterContentInit();")],
                );
            }
            if g.after_content_checked {
                self.after_content
                    .push(format!("    this.{field}.ngAfterContentChecked();"));
            }
            if g.after_view_init {
                self.uses_first_check = true;
                in_first_check(
                    &mut self.after_view,
                    &[format!("      this.{field}.ngAfterViewInit();")],
                );
            }
            if g.after_view_checked {
                self.after_view
                    .push(format!("    this.{field}.ngAfterViewChecked();"));
            }
            if g.on_destroy {
                self.destroy
                    .push(format!("    this.{field}.ngOnDestroy();"));
            }
        }
    }

    /// The hooks that the official compiler binds after visiting the child's
    /// content (`bindDirectiveAfterChildrenCallbacks`): `ngAfterContent*`,
    /// `ngAfterView*` and `ngOnDestroy`.
    fn after_children(&mut self, child: &Child, inst_field: &str) {
        let g = &child.hooks;
        if g.after_content_init {
            self.uses_first_check = true;
            in_first_check(
                &mut self.after_content,
                &[format!("      this.{inst_field}.ngAfterContentInit();")],
            );
        }
        if g.after_content_checked {
            self.after_content
                .push(format!("    this.{inst_field}.ngAfterContentChecked();"));
        }
        if g.after_view_init {
            self.uses_first_check = true;
            in_first_check(
                &mut self.after_view,
                &[format!("      this.{inst_field}.ngAfterViewInit();")],
            );
        }
        if g.after_view_checked {
            self.after_view
                .push(format!("    this.{inst_field}.ngAfterViewChecked();"));
        }
        if g.on_destroy {
            self.destroy
                .push(format!("    this.{inst_field}.ngOnDestroy();"));
        }
    }

    /// The inputs of a child (`bindDirectiveInputs`) and the hooks of its
    /// change detection (`bindDirectiveDetectChangesLifecycleCallbacks`), in
    /// `detectChangesInInputsMethod`.
    ///
    /// The bindings are emitted in the order in which the child declares its
    /// `@Input`s (`_SortInputsVisitor`); the constant ones (a static attribute,
    /// an immutable expression) together in an `if (firstCheck)` before the
    /// others (`bindAndWriteToRenderer`), each one consuming its index. An
    /// `onPush` child (or one with `AfterChanges` and some bound input)
    /// computes `changed`.
    fn child_inputs(
        &mut self,
        bound: &[(&crate::html::Binding, bool)],
        child: &Child,
        inst_field: &str,
        view_field: &str,
    ) -> Result<(), Refusal> {
        // The `bool` type of the input (`_isBoolType`), from the child's
        // metadata: an attribute without a value binds `true` on it, `''` on
        // the others (case j73).
        let is_bool = |name: &str| {
            child
                .metadata
                .as_ref()
                .and_then(|m| m.input(name))
                .and_then(|x| x.boolean)
        };
        let spec: Vec<(String, String, Option<bool>)> = child
            .inputs
            .iter()
            .map(|e| (e.name.clone(), e.field.clone(), is_bool(&e.name)))
            .collect();
        let view = child.on_push.then_some(view_field);
        self.inputs_of(
            bound,
            &spec,
            inst_field,
            view,
            &child.hooks,
            Reason::BindingOnChild,
        )
    }

    /// `bindDirectiveInputs` and `bindDirectiveDetectChangesLifecycleCallbacks`
    /// of a directive or component: `spec` are the declared `@Input`s (name,
    /// member, whether it is `bool` when known), `on_push` the view of the
    /// `onPush` component that marks the check.
    fn inputs_of(
        &mut self,
        bound: &[(&crate::html::Binding, bool)],
        spec: &[(String, String, Option<bool>)],
        inst_field: &str,
        on_push: Option<&str>,
        g: &crate::component::Hooks,
        reason: Reason,
    ) -> Result<(), Refusal> {
        let dbg = deferred(CHECK_BINDING);
        // `optimizeLifecycles`: without a bound input, `ngAfterChanges` is dropped.
        let after_changes = g.after_changes && !bound.is_empty();
        // `if (!directive.hasInputs) return;`: without a declared `@Input`, no
        // inputs and no `changed`.
        if !spec.is_empty() {
            let url = if bound.is_empty() {
                String::new()
            } else {
                self.url(reason)?
            };
            let computes_changed = on_push.is_some() || after_changes;
            if computes_changed {
                self.uses_changed = true;
                self.inputs.push("    changed = false;".to_string());
            }
            let mut order: Vec<&(&crate::html::Binding, bool)> = bound.iter().collect();
            order.sort_by_key(|(l, _)| {
                spec.iter()
                    .position(|x| x.0 == l.name)
                    .unwrap_or(usize::MAX)
            });
            let dev = deferred(DEVTOOLS);
            let mut constants = Vec::new();
            let mut dynamic = Vec::new();
            for (l, is_static) in order {
                let Some((_, field, is_bool)) = spec.iter().find(|x| x.0 == l.name).cloned() else {
                    // A name that the child does not declare as an `@Input`: it
                    // may be a directive.
                    return Err(refusal(reason, "[x] that is not a child @Input"));
                };
                let name = &l.name;
                let (start, end) = (l.start, l.end);
                let changed_stmt = if computes_changed {
                    "\nchanged = true;"
                } else {
                    ""
                };
                // An attribute with `{{ }}` on an `@Input` (`x="a {{b}}"`): the
                // interpolation goes into the input — mutable, checked and
                // assigned (the primitive alone, raw and interpolated in the
                // assignment); immutable, on the first check (case j58).
                if *is_static && l.value.contains("{{") {
                    let v = self.interpolated_value(&l.value, reason)?;
                    if v.immutable {
                        let value = v.constant;
                        let block = format!(
                            "if ({dev}.isDevToolsEnabled) {{\n  {dev}.Inspector.instance.recordInput(this.{inst_field}, '{name}', {value});\n}}\nthis.{inst_field}.{field} = {value} /* REF:{url}:{start}:{end} */;{changed_stmt}"
                        );
                        constants.push(indent_lines(&block, 6));
                        continue;
                    }
                    let k = v.k;
                    let expr = literal(&l.value);
                    let chk = deferred(CHECK_BINDING);
                    let (check, in_action) = (v.check, v.in_action);
                    let changed_stmt = if computes_changed {
                        "\n      changed = true;"
                    } else {
                        ""
                    };
                    self.expr_fields.push(format!("  Object? _expr_{k};"));
                    dynamic.push(format!(
                        "    final currVal_{k} = {check};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, {expr}, '{url}')) {{\n      if ({dev}.isDevToolsEnabled) {{\n        {dev}.Inspector.instance.recordInput(this.{inst_field}, '{name}', {in_action});\n      }}\n      this.{inst_field}.{field} = {in_action} /* REF:{url}:{start}:{end} */;{changed_stmt}\n      this._expr_{k} = currVal_{k};\n    }}"
                    ));
                    continue;
                }
                let k = self.next_binding;
                self.next_binding += 1;
                // A static attribute: the `LiteralPrimitive` of the text;
                // without a value, `EmptyExpr` (`true` on a `bool` input,
                // otherwise `''`).
                // An immutable expression: the same shape (`_bindLiteral`).
                let (value, nullable) = if *is_static {
                    if without_value(l) {
                        match is_bool {
                            Some(true) => ("true".to_string(), false),
                            Some(false) => ("''".to_string(), false),
                            None => {
                                return Err(refusal(
                                    reason,
                                    "attribute without a value on a child @Input",
                                ));
                            }
                        }
                    } else {
                        (literal(&l.value), false)
                    }
                } else {
                    // Only the dynamic one (`final currVal_k = …;`) can break.
                    let c = self.convert_breakable(&l.value, reason)?;
                    if c.immutable && c.text.contains(crate::expr::BREAK) {
                        return Err(refusal(reason, "immutable call with a named argument"));
                    }
                    if !c.immutable {
                        let expr = literal(&l.value);
                        let chk = deferred(CHECK_BINDING);
                        let value = c.text;
                        let changed_stmt = if computes_changed {
                            "\n      changed = true;"
                        } else {
                            ""
                        };
                        self.expr_fields.push(format!("  Object? _expr_{k};"));
                        dynamic.push(format!(
                            "    final currVal_{k} = {value};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, {expr}, '{url}')) {{\n      if ({dev}.isDevToolsEnabled) {{\n        {dev}.Inspector.instance.recordInput(this.{inst_field}, '{name}', currVal_{k});\n      }}\n      this.{inst_field}.{field} = currVal_{k} /* REF:{url}:{start}:{end} */;{changed_stmt}\n      this._expr_{k} = currVal_{k};\n    }}"
                        ));
                        continue;
                    }
                    (c.text, c.can_be_null)
                };
                if value == "null" {
                    continue;
                }
                let mut block = format!(
                    "if ({dev}.isDevToolsEnabled) {{\n  {dev}.Inspector.instance.recordInput(this.{inst_field}, '{name}', {value});\n}}\nthis.{inst_field}.{field} = {value} /* REF:{url}:{start}:{end} */;{changed_stmt}"
                );
                if nullable {
                    block = format!("if (({value} != null)) {{\n{}\n}}", indent_lines(&block, 2));
                }
                constants.push(indent_lines(&block, 6));
            }
            if !constants.is_empty() {
                self.uses_first_check = true;
                in_first_check(&mut self.inputs, &constants);
            }
            self.inputs.extend(dynamic);
            if let Some(view) = on_push {
                self.inputs.push(format!(
                    "    if (changed) {{\n      this.{view}.markAsCheckOnce();\n    }}"
                ));
            }
        }
        if after_changes {
            self.uses_changed = true;
            self.inputs.push(format!(
                "    if (changed) {{\n      this.{inst_field}.ngAfterChanges();\n    }}"
            ));
        }
        if g.on_init {
            self.uses_first_check = true;
            self.inputs.push(format!(
                "    if (((!{dbg}.debugThrowIfChanged) && firstCheck)) {{\n      this.{inst_field}.ngOnInit();\n    }}"
            ));
        }
        if g.do_check {
            self.inputs.push(format!(
                "    if ((!{dbg}.debugThrowIfChanged)) {{\n      this.{inst_field}.ngDoCheck();\n    }}"
            ));
        }
        Ok(())
    }

    /// The listeners of a node with directives: the template events, with the
    /// `@HostListener` of the same event in their handler (`mergeEvents`), and
    /// then the `@HostListener`s of the directives (`_visitHostListeners`),
    /// grouped by event.
    fn listeners_with_host(
        &mut self,
        node: &crate::html::Element,
        matched: &[std::sync::Arc<crate::directives::Directive>],
        r: &crate::directives::ResolvedNode,
        target: &str,
    ) -> Result<(), Refusal> {
        // The `@HostListener`s in the order of the directives in `directives:`
        // (`_collectHostListeners`), grouped by event in the order in which
        // each one first appears: two listeners of the same event become one
        // `_handleEvent_N` that calls both.
        let mut groups: Vec<(String, Vec<(String, crate::directives::Listener)>)> = Vec::new();
        for d in matched {
            let Some((_, field)) = r
                .directives
                .iter()
                .find(|(x, _)| std::sync::Arc::ptr_eq(x, d))
            else {
                continue;
            };
            for o in &d.listeners {
                match groups.iter_mut().find(|(ev, _)| *ev == o.event) {
                    Some((_, l)) => l.push((field.clone(), o.clone())),
                    None => groups.push((o.event.clone(), vec![(field.clone(), o.clone())])),
                }
            }
        }
        // The template events come first, and the `@HostListener` of the same
        // event goes into their handler, after the written action
        // (`mergeEvents` over the element's outputs, case j70).
        let mut from_template: Vec<&str> = Vec::new();
        for l in &node.events {
            if from_template.contains(&l.name.as_str()) {
                return Err(refusal(
                    Reason::Event,
                    "two handlers of the same event in the template",
                ));
            }
            from_template.push(&l.name);
        }
        for l in &node.events {
            let extras: Vec<String> = groups
                .iter()
                .filter(|(ev, _)| *ev == l.name)
                .flat_map(|(_, list)| list)
                .map(|(field, o)| format!("this.{field}.{}({})", o.method, o.args))
                .collect();
            self.event_type_failure = None;
            self.event_type = native_event_type_q(&l.name);
            match self.handler_with(&[&l.value], &extras) {
                Ok(h) => self.listener(&l.name, target, &h),
                Err(r) => self.record_refusal(r)?,
            }
        }
        for (event, list) in &groups {
            if from_template.contains(&event.as_str()) {
                continue;
            }
            self.event_type_failure = None;
            self.event_type = native_event_type_q(event);
            let h = match list.as_slice() {
                [(field, o)] => self.host_handler(field, o),
                several => self.group_handler(several),
            };
            self.listener(event, target, &h);
        }
        Ok(())
    }

    /// The events written on the child's element that do not match one of its
    /// `@Output`s: element events (`bindRenderOutputs`), before the outputs of
    /// the node's directives ([`Self::child_outputs`]).
    fn child_element_events(
        &mut self,
        e: &crate::html::Element,
        child: &Child,
        el: &str,
        directives: Option<(
            &[std::sync::Arc<crate::directives::Directive>],
            &crate::directives::ResolvedNode,
        )>,
    ) -> Result<(), Refusal> {
        let mut element_only = e.clone();
        element_only
            .events
            .retain(|l| child.output(&l.name).is_none());
        // With directives on the node, their `@HostListener`s (case j93).
        match directives {
            Some((matched, r)) if matched.iter().any(|d| !d.listeners.is_empty()) => {
                self.listeners_with_host(&element_only, matched, r, el)
            }
            _ => self.events(&element_only, el),
        }
    }

    /// The type of `$event` of the handler of an `@Output` (`member` of class
    /// `class` from `uri`): in 9, the `T` of its `Stream<T>` through
    /// `fromDartType` (§B4); in 8, none.
    fn output_type(&self, uri: &str, class: &str, member: &str) -> Result<Option<String>, Refusal> {
        if crate::dialect::current() == crate::dialect::Dialect::Ngdart {
            return Ok(None);
        }
        let failure = || refusal(Reason::Event, "@Output without the Stream type");
        let (r, _) = self.types.ok_or_else(failure)?;
        let (t, scope) = r.output_type(uri, class, member).ok_or_else(failure)?;
        if t == "dynamic" {
            return Ok(Some(t));
        }
        let full = instantiate_raw(&t, &scope, r).ok_or_else(failure)?;
        qualified_type(&full, &scope, r, &self.asset, &[])
            .map(Some)
            .ok_or_else(failure)
    }

    /// The type of `$event` for the handler of an `@Output`; without it, the
    /// failure is kept for the complex handler, the only one that writes it.
    fn set_output_type(&mut self, uri: &str, class: &str, member: &str) {
        match self.output_type(uri, class, member) {
            Ok(t) => {
                self.event_type = t;
                self.event_type_failure = None;
            }
            Err(r) => {
                self.event_type = None;
                self.event_type_failure = Some(r);
            }
        }
    }

    /// The child's `@Output`s bound in the template: `subscription_N`
    /// (`bindDirectiveOutputs`), at the child's turn among the node's
    /// directives.
    fn child_outputs(
        &mut self,
        e: &crate::html::Element,
        child: &Child,
        inst_field: &str,
    ) -> Result<(), Refusal> {
        let mut seen: Vec<&str> = Vec::new();
        for l in e.events.iter().filter(|l| child.output(&l.name).is_some()) {
            if seen.contains(&l.name.as_str()) {
                return Err(refusal(
                    Reason::Event,
                    "two handlers of the same event in the template",
                ));
            }
            seen.push(&l.name);
            let member = child.output(&l.name).unwrap_or_default().to_string();
            self.set_output_type(&child.uri_dart, &child.class, &member);
            match self.handler(&[&l.value]) {
                Ok(h) => {
                    let k = self.subscriptions;
                    self.subscriptions += 1;
                    self.listeners.push(format!(
                        "    final subscription_{k} = this.{inst_field}.{member}.listen({h});"
                    ));
                }
                Err(r) => self.record_refusal(r)?,
            }
        }
        Ok(())
    }

    /// `*dir="..."`: anchor, `ViewContainer`, `TemplateRef` and the structural
    /// directive, with the content in a separate embedded view.
    ///
    /// Only ngdart's own two directives. Generalizing requires the directive
    /// index (attribute selector, constructor, inputs, lifecycle) — and the
    /// numbering of the providers changes with what the directive injects, so
    /// it cannot be guessed.
    fn structural(
        &mut self,
        e: &crate::html::Element,
        star: &crate::html::Binding,
        parent: &str,
    ) -> Result<(), Refusal> {
        let url = self.url(Reason::Binding)?;
        let Some(dir) = Structural::known(&star.name) else {
            return Err(refusal(
                Reason::Binding,
                "`*` of a directive that is not ngIf/ngFor/ngSwitch",
            ));
        };
        // `@Host()`: the provider on an element above, in this view or in an
        // ancestor one (the read already comes through the `parentView` chain).
        // With a component in the way, the injector may be a different one:
        // refusal.
        let host_read = match dir.host {
            None => None,
            Some(class) => {
                let token = crate::directives::Token::Class {
                    uri: dir.uri.to_string(),
                    class: class.to_string(),
                };
                let found = self
                    .providers_above()
                    .into_iter()
                    .find(|p| p.token == token)
                    .map(|p| p.read);
                match found {
                    Some(l) if self.components_above == 0 => Some(l),
                    _ => {
                        return Err(refusal(
                            Reason::DirectiveBySelector,
                            format!("{} without the {class} above", dir.class),
                        ));
                    }
                }
            }
        };
        let mut micro = e.star_micro().unwrap_or_default();
        // The bindings are emitted in the order of the directive's inputs, not
        // in the written one (`_orderingOf(directive.inputs)`). An input that
        // the directive does not declare is an error in the official compiler
        // ("Can't bind to ..."): there is no output.
        if let Some((p, _)) = micro
            .properties
            .iter()
            .find(|(p, _)| !dir.inputs.contains(&p.as_str()))
        {
            return Err(refusal(
                Reason::Binding,
                format!("`{p}` is not an input of {}", dir.class),
            ));
        }
        micro
            .properties
            .sort_by_key(|(p, _)| dir.inputs.iter().position(|x| x == p));
        self.template_guard(&star.name, &micro, Some(&dir))?;
        // The `REF` of each input: the span of the binding written in a
        // `<template dir [dirX]>` ([`template_with_directive`]); for `*`, the span
        // of the whole attribute.
        let span = |prop: &str| {
            e.template_bindings
                .iter()
                .find(|l| l.name == prop)
                .map_or((star.start, star.end), |l| (l.start, l.end))
        };
        let n = self.next_node;
        self.next_node += 1;
        let dom = self.dom();
        let vc = self.imp.q(VIEW_CONTAINER);
        let tr = self.imp.q(TEMPLATE_REF);
        let qd = self.imp.q(dir.uri);
        let dev = self.imp.alias(DEVTOOLS);
        let dir_class = dir.class;
        // The number of this view, and the jump its subtree consumes.
        let index = self.next_embedded;
        self.next_embedded += 1 + count_structural(&e.children);
        let factory_name = format!("viewFactory_{}{index}", &self.view_class[4..]);
        // In 9 there is the extra `JSObject` among the built-ins (§B3).
        let x = crate::dialect::extra_builtins();
        let (k_tr, k_right) = (8 + x, 9 + x);
        let field = format!("_{dir_class}_{n}_{k_right}");
        // In a `<template>` the built-in providers take 0..7 and the
        // `TemplateRef` is number 8 — see docs/GERADOR-NG.md §2.
        self.child_fields
            .push(format!("  late final {vc}ViewContainer _appEl_{n};"));
        self.child_fields
            .push(format!("  late final {qd}{dir_class} {field};"));
        self.anchors.push(format!("_appEl_{n}"));
        // The second argument is the index of the parent element, and `null`
        // when the anchor is at the root of the view (`isRootElement ? null :
        // parent.nodeIndex`, in `compile_element.dart`); loose in projected
        // content, the parent is the child that receives it.
        let parent_index = if parent.is_empty() {
            self.roots.push(format!("this._appEl_{n}"));
            self.lines
                .push(format!("    final _anchor_{n} = {dom}.createAnchor();"));
            self.projected_parent
                .map_or_else(|| "null".to_string(), |k| k.to_string())
        } else {
            self.lines.push(format!(
                "    final _anchor_{n} = {dom}.appendAnchor({parent});"
            ));
            element_index(parent)
        };
        self.lines.push(format!(
            "    this._appEl_{n} = {vc}ViewContainer({n}, {parent_index}, this, _anchor_{n});"
        ));
        self.lines.push(format!(
            "    var _TemplateRef_{n}_{k_tr} = {tr}TemplateRef(this._appEl_{n}, {});",
            self.template_factory(&factory_name)
        ));
        let extra = host_read.map(|h| format!(", {h}")).unwrap_or_default();
        let template_arg = if dir.with_template {
            format!(", _TemplateRef_{n}_{k_tr}")
        } else {
            String::new()
        };
        self.lines.push(format!(
            "    this.{field} = {qd}{dir_class}(this._appEl_{n}{template_arg}{extra});"
        ));
        self.lines.push(format!(
            "    if ({dev}.isDevToolsEnabled) {{\n      {dev}.Inspector.instance.registerDirective(_anchor_{n}, this.{field});\n    }}"
        ));

        // The inputs of the directive, in the order in which the microsyntax
        // declares them (it is the order of the binding indices).
        // `bindAndWriteToRenderer` writes the immutable ones (`if
        // (firstCheck)`) before the others.
        let mut collection_type = None;
        let mut dynamic = Vec::new();
        for (prop, expr) in &micro.properties {
            let c = self.convert(expr, Reason::Binding)?;
            if prop.ends_with("Of") {
                collection_type = c.ty.clone().map(|t| (t, c.scope.clone()));
                // `_typeNgForLocals` types the collection with the
                // `analyzedClass`, whose locals are only the ones that have a
                // type: a local without a type (a `let-x` of a `<template>`, an
                // item of a `dynamic` collection) is not included, and the name
                // falls back to the component member (case j83). Without a
                // member with the same name, the `_lookupGetterReturnType` of
                // the implicit receiver gives `dynamic`, the same as the
                // untyped local: only a local that a member replaces leaves the
                // scope (and the arguments of a call do not weigh on its type).
                let is_member = |name: &str| {
                    self.members.contains_key(name)
                        || self.methods.contains_key(name)
                        || self.types.is_some_and(|(r, file)| {
                            let class = self.qualified_class.rsplit('.').next().unwrap_or("");
                            r.member_type(file, class, name).is_some()
                                || r.method(file, class, name).is_some()
                        })
                };
                let untyped: Vec<String> = self
                    .locals
                    .iter()
                    .filter(|(name, l)| {
                        l.ty == "dynamic" && references_at_root(expr, name) && is_member(name)
                    })
                    .map(|(name, _)| name.clone())
                    .collect();
                if !untyped.is_empty() {
                    let saved = self.locals.clone();
                    for name in &untyped {
                        self.locals.remove(name);
                    }
                    let typed_only = self.convert(expr, Reason::Binding);
                    self.locals = saved;
                    collection_type = typed_only
                        .ok()
                        .and_then(|x| x.ty.map(|t| (t, x.scope)))
                        .or_else(|| Some(("dynamic".to_string(), None)));
                }
            }
            // An immutable input (`final List<X> items`, `*ngIf="fixed"`) is
            // written once, in the inputs' `if (firstCheck)`: directly on
            // `NgIf` (`_directBinding` in the constants method); through
            // `_bindLiteral` on `NgFor`, which consumes the binding index and
            // puts the `if (x != null)` around what can be null.
            if c.immutable {
                let (start, end) = span(prop);
                let value = &c.text;
                let dev_rec = format!(
                    "if ({dev}.isDevToolsEnabled) {{\n  {dev}.Inspector.instance.recordInput(this.{field}, '{prop}', {value});\n}}"
                );
                let set = format!("this.{field}.{prop} = {value} /* REF:{url}:{start}:{end} */;");
                let mut statements = vec![dev_rec, set];
                if !dir.direct {
                    self.next_binding += 1;
                    if value == "null" {
                        continue;
                    }
                    if c.can_be_null {
                        let inner: Vec<String> =
                            statements.iter().map(|i| indent_lines(i, 2)).collect();
                        statements = vec![format!(
                            "if (({value} != null)) {{\n{}\n}}",
                            inner.join("\n")
                        )];
                    }
                }
                let blocks: Vec<String> = statements.iter().map(|i| indent_lines(i, 6)).collect();
                self.uses_first_check = true;
                in_first_check(&mut self.inputs, &blocks);
                continue;
            }
            let value = c.text;
            let (start, end) = span(prop);
            if dir.direct {
                // The ngcompiler's `_isDirectBinding`: `NgIf` already compares
                // the value before acting, so there is no `checkBinding`
                // outside.
                dynamic.push(format!(
                    "    if ({dev}.isDevToolsEnabled) {{\n      {dev}.Inspector.instance.recordInput(this.{field}, '{prop}', {value});\n    }}\n    this.{field}.{prop} = {value} /* REF:{url}:{start}:{end} */;"
                ));
            } else {
                let k = self.next_binding;
                self.next_binding += 1;
                self.expr_fields.push(format!("  Object? _expr_{k};"));
                let chk = deferred(CHECK_BINDING);
                // The name that goes into the check is the expression of this
                // property (`items`), not the whole value of the `*`.
                let text = expr.trim();
                dynamic.push(format!(
                    "    final currVal_{k} = {value};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, '{text}', '{url}')) {{\n      if ({dev}.isDevToolsEnabled) {{\n        {dev}.Inspector.instance.recordInput(this.{field}, '{prop}', currVal_{k});\n      }}\n      this.{field}.{prop} = currVal_{k} /* REF:{url}:{start}:{end} */;\n      this._expr_{k} = currVal_{k};\n    }}"
                ));
            }
        }
        self.inputs.extend(dynamic);
        if dir.do_check {
            let chk = deferred(CHECK_BINDING);
            self.inputs.push(format!(
                "    if ((!{chk}.debugThrowIfChanged)) {{\n      this.{field}.ngDoCheck();\n    }}"
            ));
        }

        // The loop locals, typed by the element of the collection.
        let locals = self.micro_locals(&micro, collection_type.as_ref())?;
        let mut without_star = e.clone();
        without_star.star = None;
        self.push_embedded(
            e,
            vec![Node::Element(without_star)],
            n,
            index,
            factory_name,
            parent,
            locals,
            micro,
        );
        Ok(())
    }

    /// A hand-written `<template>`: anchor, `ViewContainer` and `TemplateRef`
    /// (number 7 of the node's table), with the content in an embedded view
    /// whose locals are the `let-x`s (key `$implicit` when without a value).
    /// With a `#ref` the `TemplateRef` is a field and the name reads it;
    /// without one, a local. Nobody creates the view here: the `ViewContainer`
    /// stays out of change detection and destruction.
    ///
    /// A `<template>` directive (`template[x]`, which receives the
    /// `TemplateRef` in its constructor: limitless_ui's
    /// `LiSelectTriggerDirective`) is provider 8, created right after,
    /// registered in devtools and findable by the content queries of the child
    /// that receives the projected `<template>` (case j28). Without a directive
    /// that asks for `ViewContainerRef`, the node has no `hasViewContainer`:
    /// what goes to the root and to the projection is the anchor
    /// (`_addRootNodeAndProject`).
    fn hand_template(&mut self, e: &crate::html::Element, parent: &str) -> Result<(), Refusal> {
        let mut template_locals = Vec::new();
        let mut attributes = Vec::new();
        for a in &e.attributes {
            match a.name.strip_prefix("let-") {
                Some(name) => template_locals.push((
                    name.to_string(),
                    if a.value.is_empty() {
                        "$implicit".to_string()
                    } else {
                        a.value.clone()
                    },
                )),
                None => attributes.push(a),
            }
        }
        let mut stripped = e.clone();
        stripped.star = None;
        let desc = crate::selector::Element::of_template(&stripped);
        if let Some(u) = self.used.iter().find(|u| {
            (u.child.is_some() || u.directive.is_none())
                && crate::selector::matches_any(&u.selectors, &desc)
        }) {
            return Err(refusal(
                Reason::DirectiveBySelector,
                format!("directive {} on <template> without metadata", u.class),
            ));
        }
        // The directives of the `<template>` in `directives:` order
        // (`_matchTemplateDirectives`), resolved like those of an element: the
        // node's `TemplateRef` and `ViewContainerRef`, dependencies from above,
        // inputs, outputs and hooks (case j85). The `@HostBinding` of one of
        // them is not written on the `<template>` (`visitEmbeddedTemplate` does
        // not call `bindDirectiveHostProps`).
        let matched = matched_directives(self.used, &stripped);
        if matched.is_empty() && !attributes.is_empty() {
            return Err(refusal(
                Reason::Binding,
                "attribute on <template> without a directive",
            ));
        }
        // `[x]` and `(x)` on a `<template>` only exist as an input or output of
        // one of its directives (the rest is an error in the official
        // compiler).
        if let Some(l) = stripped
            .properties
            .iter()
            .find(|l| !consumes_input(&matched, &l.name))
            .or_else(|| {
                stripped
                    .events
                    .iter()
                    .find(|l| !consumes_output(&matched, &l.name))
            })
        {
            return Err(refusal(
                Reason::Binding,
                format!("[{}] on <template> that no directive receives", l.name),
            ));
        }
        for d in &matched {
            if !d.listeners.is_empty() || d.queries || !d.content_queries.is_empty() {
                return Err(refusal(
                    Reason::DirectiveBySelector,
                    format!(
                        "directive {} on <template> with @HostListener or a query",
                        d.class
                    ),
                ));
            }
        }
        // A `@ViewChild` with the result on the `<template>` itself: the value
        // would be its `TemplateRef`. Inside, the query maps the embedded view
        // ([`query_tree`]).
        if self
            .dynamic_queries
            .iter()
            .any(|q| matches_key(e, &q.key, self.children))
        {
            return Err(refusal(Reason::Binding, "@ViewChild on <template>"));
        }
        let n = self.next_node;
        self.next_node += 1;
        let dom = self.dom();
        let vc = self.imp.q(VIEW_CONTAINER);
        let tr = self.imp.q(TEMPLATE_REF);
        let index = self.next_embedded;
        self.next_embedded += 1 + count_structural(&e.children);
        let factory_name = format!("viewFactory_{}{index}", &self.view_class[4..]);
        self.child_fields
            .push(format!("  late final {vc}ViewContainer _appEl_{n};"));
        let reference = e.references.first().map(|r| r.name.clone());
        // The `TemplateRef` is a field when read outside `build()` (`#ref`);
        // the read goes through the index the resolver gives it.
        let has_ref = reference.is_some();
        let tr_read = |k: u32| {
            if has_ref {
                format!("this._TemplateRef_{n}_{k}")
            } else {
                format!("_TemplateRef_{n}_{k}")
            }
        };
        // A query reads the `#ref` as a `ViewContainerRef`: the `ViewContainer`
        // is not private and the `TemplateRef` goes to index 8.
        let force_container = reference
            .as_ref()
            .is_some_and(|r| self.templates_with_container.contains(r));
        let (resolved, k_tr) = if matched.is_empty() {
            (
                None,
                crate::dialect::extra_builtins() + if force_container { 8 } else { 7 },
            )
        } else {
            let providers = self.providers_above_with(&deferred(UTILITIES));
            let above = crate::directives::Above {
                providers: &providers,
                uncertain: self.uncertain_above > 0,
                of_view: self.view_tokens,
            };
            let (r, k) = crate::directives::resolve_template(
                &matched,
                n,
                Some(above),
                force_container,
                &tr_read,
            )
            .map_err(|f| refusal(Reason::DirectiveBySelector, f))?;
            (Some(r), k)
        };
        if has_ref {
            self.child_fields.push(format!(
                "  late final {tr}TemplateRef _TemplateRef_{n}_{k_tr};"
            ));
        }
        let parent_index = if parent.is_empty() {
            // With a non-private `ViewContainer`, it is the root (`vcAppEl ??
            // renderNode`, case j85).
            let container = force_container || resolved.as_ref().is_some_and(|r| r.container);
            self.roots.push(if container {
                format!("this._appEl_{n}")
            } else {
                format!("_anchor_{n}")
            });
            self.lines
                .push(format!("    final _anchor_{n} = {dom}.createAnchor();"));
            self.projected_parent
                .map_or_else(|| "null".to_string(), |k| k.to_string())
        } else {
            self.lines.push(format!(
                "    final _anchor_{n} = {dom}.appendAnchor({parent});"
            ));
            element_index(parent)
        };
        self.lines.push(format!(
            "    this._appEl_{n} = {vc}ViewContainer({n}, {parent_index}, this, _anchor_{n});"
        ));
        match &reference {
            Some(name) => {
                self.lines.push(format!(
                    "    this._TemplateRef_{n}_{k_tr} = {tr}TemplateRef(this._appEl_{n}, {});",
                    self.template_factory(&factory_name)
                ));
                let read = format!("this._TemplateRef_{n}_{k_tr}");
                self.refs.insert(name.clone(), read.clone());
                self.refs_in_order.push((name.clone(), read.clone()));
                if force_container {
                    let vc = format!("this._appEl_{n}");
                    self.refs.insert(container_key(name), vc.clone());
                    self.refs_in_order.push((container_key(name), vc));
                }
            }
            None => {
                self.lines.push(format!(
                    "    var _TemplateRef_{n}_{k_tr} = {tr}TemplateRef(this._appEl_{n}, {});",
                    self.template_factory(&factory_name)
                ));
            }
        }
        // The directives: creation, `registerDirective` on the anchor, inputs
        // and outputs (without `@HostBinding`: no instance for
        // `detectHostChanges`) and the hooks, which on a `<template>` are
        // emitted right away (`bindDirectiveAfterChildrenCallbacks` in
        // `visitEmbeddedTemplate`).
        let above_len = self.above.len();
        // With `ViewContainerRef` requested, the `ViewContainer` is not private
        // (`createViewContainer(.., !hasViewContainer, ..)`): its views are
        // detected and destroyed by this one (case j85).
        if force_container || resolved.as_ref().is_some_and(|r| r.container) {
            self.anchors.push(format!("_appEl_{n}"));
        }
        if let Some(r) = &resolved {
            let target = format!("_anchor_{n}");
            self.create_instances(&stripped, &r.instances, &target, "this")?;
            self.register_directives(&r.directives, &target);
            self.bind_directives(
                &stripped,
                &[],
                &r.directives,
                &target,
                &stripped.properties,
                &stripped.events,
                true,
                true,
            )?;
            self.hooks_after_children(&r.directives);
            let injectables: Vec<(Vec<crate::directives::Token>, String)> = r
                .instances
                .iter()
                .filter(|i| !i.injectable_by.is_empty())
                .map(|i| (i.injectable_by.clone(), i.read.clone()))
                .collect();
            if !injectables.is_empty() {
                self.injectors.push((n, n, injectables));
            }
            let mut above = self.stack.clone();
            above.push((n, true));
            self.registrations.push(Registration {
                above,
                providers: r
                    .instances
                    .iter()
                    .flat_map(|i| {
                        std::iter::once(&i.token)
                            .chain(&i.aliases)
                            .map(|t| (t.clone(), i.read.clone(), None))
                    })
                    .collect(),
                element: target,
            });
            // The providers of the `<template>` stay above its content.
            for i in &r.instances {
                if i.lazy {
                    self.lazy_above
                        .insert((i.read.clone(), self.this_view_class()));
                }
                if i.injectable_by.is_empty() {
                    self.above.push((i.token.clone(), i.read.clone(), None));
                }
                for t in &i.injectable_by {
                    self.above.push((t.clone(), i.read.clone(), None));
                }
            }
        }
        // Whoever creates the view (the child's directive, with
        // `ngTemplateOutlet`) is the one that gives the context: the local has
        // no type (`dynamic`, without a cast).
        let mut locals = self.locals.clone();
        for (name, _) in &template_locals {
            locals.insert(
                name.clone(),
                crate::expr::Local {
                    dart: format!("local_{name}"),
                    ty: "dynamic".to_string(),
                    scope: None,
                },
            );
        }
        self.push_embedded(
            e,
            e.children.clone(),
            n,
            index,
            factory_name,
            parent,
            locals,
            crate::micro::Micro {
                properties: Vec::new(),
                locals: template_locals,
            },
        );
        self.above.truncate(above_len);
        Ok(())
    }

    /// The second argument of `TemplateRef`: the factory of the embedded view.
    /// In a generic component the official compiler wraps it in a closure that
    /// passes the type arguments (`viewFactory_X1<T>(parentView,
    /// parentIndex)`).
    fn template_factory(&self, name: &str) -> String {
        if self.generics.is_empty() {
            return name.to_string();
        }
        format!(
            "(parentView, parentIndex) {{\n      return {name}{}(parentView, parentIndex);\n    }}",
            self.generics
        )
    }

    /// Stores the specification of the embedded view of a `*` or `<template>`
    /// (`e`, at anchor `n`), with what it inherits from this one: the locals,
    /// the `#ref`s and the providers above, one `parentView` farther away.
    #[allow(clippy::too_many_arguments)]
    fn push_embedded(
        &mut self,
        e: &crate::html::Element,
        nodes: Vec<Node>,
        n: u32,
        index: u32,
        factory_name: String,
        parent: &str,
        locals: std::collections::HashMap<String, crate::expr::Local>,
        micro: crate::micro::Micro,
    ) {
        // The official compiler binds the embedded view when the binder reaches
        // the template, after its inputs: the node of this view that it reads
        // is promoted there, between the `_expr_k`s already created and the
        // following ones.
        let pending: Vec<String> = self
            .embedded_refs
            .iter()
            .filter(|name| !self.embedded_position.contains_key(*name))
            .cloned()
            .collect();
        for name in pending {
            if referenced_in_embedded(e, &name, self.children) {
                self.embedded_position.insert(name, self.expr_fields.len());
            }
        }
        // For the new view, the locals of this view and of the ancestor ones
        // end up one `parentView` farther away.
        let mut ancestors: std::collections::HashMap<String, Origin> = self
            .ancestors
            .iter()
            .map(|(n, o)| {
                (
                    n.clone(),
                    Origin {
                        levels: o.levels + 1,
                        ..o.clone()
                    },
                )
            })
            .collect();
        for (name, key) in &self.own_locals {
            ancestors.insert(
                name.clone(),
                Origin {
                    key: key.clone(),
                    class: self.this_class.clone(),
                    levels: 1,
                },
            );
        }
        // An anchor at the root of this view has as parent the same node as the
        // root of this view; when nested, a node of this view.
        let at_top =
            parent == "parentRenderNode" || (parent.is_empty() && self.projected_parent.is_none());
        let top_level = if !at_top || self.top_level < self.depth {
            self.top_level
        } else {
            self.depth + 1
        };
        // A view query with its result in here: the anchor and the class go
        // into its chain; if the result is in the new view, it marks the dirty
        // field in `dirtyParentQueriesInternal`; if it is further below, the
        // query stays in transit to the nested ones.
        let new_class = format!("_{}{index}", self.view_class);
        let mut pending: Vec<(String, String, u32)> = Vec::new();
        for q in &mut self.dynamic_queries {
            let mut l = Vec::new();
            where_is(
                std::slice::from_ref(&Node::Element(e.clone())),
                &q.key,
                self.children,
                false,
                &mut l,
            );
            if l.is_empty() {
                continue;
            }
            q.view = true;
            pending.push((q.key.clone(), q.field.clone(), 1));
        }
        for (r, field, levels) in &self.queries_in_transit {
            let mut l = Vec::new();
            where_is(
                std::slice::from_ref(&Node::Element(e.clone())),
                r,
                self.children,
                false,
                &mut l,
            );
            if !l.is_empty() {
                pending.push((r.clone(), field.clone(), levels + 1));
            }
        }
        // The anchor of the `*` goes into the map by the start of the star; in
        // the new view, the query with a result in it marks the dirty field,
        // and the one with a result further below stays in transit (both things
        // can hold at the same time).
        let star = e.star.as_ref().map(|l| l.start);
        // Every anchor goes into the map: the content queries also read the
        // chain through it ([`resolve_dynamic_content`]).
        if let Some(k) = star {
            self.query_anchors
                .borrow_mut()
                .insert(k, (format!("_appEl_{n}"), new_class.clone()));
        }
        let mut queried_refs = Vec::new();
        let mut queries_in_transit = Vec::new();
        for (r, field, levels) in pending {
            if let Some(k) = star {
                self.query_anchors
                    .borrow_mut()
                    .insert(k, (format!("_appEl_{n}"), new_class.clone()));
            }
            let mut l = Vec::new();
            where_is(&nodes, &r, self.children, false, &mut l);
            if l.iter()
                .any(|x| matches!(x, Place::Root | Place::OnChild | Place::OnProjectedChild))
            {
                queried_refs.push((r.clone(), field.clone(), levels));
            }
            if l.contains(&Place::Embedded) {
                queries_in_transit.push((r, field, levels));
            }
        }
        // `dirtyParentQueriesInternal` marks each query when its first result
        // in the new view is registered (`_setParentQueryAsDirty`, in the
        // `addQueryResult` of each element, in pre-order): the order is that of
        // the results, not that of the queries.
        let mut dirty = queried_refs.clone();
        dirty.sort_by_key(|(r, _, _)| first_result(&nodes, r, self.children));
        // The index of the `<ng-content>` is its ordinal in the whole template
        // (`ngContentSelectors`, in pre-order): the embedded view continues the
        // count from here, and this view skips the ones that stay in there.
        let next_projection = self.next_projection;
        self.next_projection += ng_content_count(&nodes);
        self.embedded_views.push(EmbeddedSpec {
            index,
            star,
            ns: self.current_ns.clone(),
            next_projection,
            queried_refs,
            dirty,
            queries_in_transit,
            depth: self.depth + 1,
            top_level,
            class: format!("_{}{index}", self.view_class),
            factory: factory_name,
            nodes,
            locals,
            micro,
            ancestors,
            ancestor_refs: self
                .ancestor_refs
                .iter()
                .map(|(n, c, k)| (n.clone(), c.clone(), k + 1))
                .chain(
                    self.local_refs
                        .iter()
                        .map(|n| (n.clone(), self.this_view_class(), 1)),
                )
                .collect(),
            above: self
                .above
                .iter()
                .map(|(t, c, v)| {
                    let v = match v {
                        Some((class, n)) => (class.clone(), n + 1),
                        None => (self.this_view_class(), 1),
                    };
                    (t.clone(), c.clone(), Some(v))
                })
                .collect(),
            lazy_above: self.lazy_above.clone(),
            lazy_with_requests: self.lazy_with_requests.clone(),
            components_above: self.components_above,
            uncertain_above: self.uncertain_above,
        });
    }

    /// The locals that the `*` puts in the scope of the embedded view, with
    /// their type: `$implicit` of the element of the collection, integer
    /// `index`/`count` and the four booleans of `NgFor`.
    fn micro_locals(
        &self,
        micro: &crate::micro::Micro,
        collection_type: Option<&(String, Option<std::path::PathBuf>)>,
    ) -> Result<std::collections::HashMap<String, crate::expr::Local>, Refusal> {
        let mut locals = self.locals.clone();
        for (name, key) in &micro.locals {
            let (ty, scope) = match key.as_str() {
                // A `dynamic` collection (an index, `??`, a ternary, a
                // `dynamic` field): `getIterableElementType` finds no type, and
                // the local has no cast (`this.locals['\$implicit']`).
                "$implicit" if collection_type.is_some_and(|(t, _)| t == "dynamic") => {
                    ("dynamic".to_string(), None)
                }
                "$implicit" => {
                    // `List`/`Iterable`/`Set` by the text; otherwise the return
                    // type of the `single` getter on the collection type
                    // (`getIterableElementType`, `analyzed_class.dart:39-42`).
                    let via_single = |t: &str, e: &Option<std::path::PathBuf>| {
                        let (r, file) = self.types?;
                        let (x, scope) = r.member_type_with_free(
                            e.as_deref().unwrap_or(file),
                            t,
                            "single",
                            self.generic_names,
                        )?;
                        Some((x, Some(scope)))
                    };
                    let Some((t, scope)) = collection_type.and_then(|(t, e)| {
                        element_type(t)
                            .map(|x| (x, e.clone()))
                            .or_else(|| via_single(t, e))
                    }) else {
                        return Err(refusal(
                            Reason::Binding,
                            "`*ngFor` local without the element type",
                        ));
                    };
                    (t, scope)
                }
                "index" | "count" => ("int".to_string(), None),
                "first" | "last" | "even" | "odd" => ("bool".to_string(), None),
                // A key that `_typeNgForLocals` does not type (`$index`): the
                // local has no type, read without a cast
                // (`template_optimize.dart:76-95`,
                // `view_name_resolver.dart:55`).
                _ => ("dynamic".to_string(), None),
            };
            locals.insert(
                name.clone(),
                crate::expr::Local {
                    dart: format!("local_{name}"),
                    ty,
                    scope,
                },
            );
        }
        Ok(locals)
    }

    /// Guard for the directives of the `<template>` that the `*` creates: it
    /// has to match exactly the structural directive that the emitter knows,
    /// and no other (`_templateSelector`: `template`, the attributes and the
    /// properties of the microsyntax).
    fn template_guard(
        &self,
        name: &str,
        micro: &crate::micro::Micro,
        dir: Option<&Structural>,
    ) -> Result<(), Refusal> {
        let mut desc = crate::selector::Element::novo("template");
        if !micro.properties.iter().any(|(p, _)| p == name) {
            desc.attribute(name, None);
        }
        for (p, v) in &micro.properties {
            desc.attribute(p, Some(v));
        }
        let matched: Vec<&UsedDirective> = self
            .used
            .iter()
            .filter(|u| crate::selector::matches_any(&u.selectors, &desc))
            .collect();
        match (dir, matched.as_slice()) {
            (Some(d), [u]) if u.is_structural(d) => Ok(()),
            (_, []) => Err(refusal(
                Reason::DirectiveBySelector,
                "`*` without a matching directive in directives:",
            )),
            _ => {
                let other = matched
                    .iter()
                    .find(|u| dir.is_none_or(|d| !u.is_structural(d)))
                    .map(|u| u.class.clone())
                    .unwrap_or_default();
                Err(refusal(
                    Reason::DirectiveBySelector,
                    format!("directive {other} on <template>"),
                ))
            }
        }
    }

    /// Guard for the directives of an element: no directive from `directives:`
    /// may match it, except the component that the emitter is going to
    /// instantiate by its tag.
    fn element_guard(&self, e: &crate::html::Element) -> Result<(), Refusal> {
        if self.used.is_empty() {
            return Ok(());
        }
        let desc = crate::selector::Element::of_template(e);
        let child = self.children.get(&e.name);
        for u in self.used {
            if !crate::selector::matches_any(&u.selectors, &desc) {
                continue;
            }
            let is_the_child = match (&u.child, child) {
                (Some(f), Some(g)) => f.class == g.class && f.uri_dart == g.uri_dart,
                _ => false,
            };
            if is_the_child {
                continue;
            }
            if u.child.is_some() {
                return Err(refusal(
                    Reason::DirectiveBySelector,
                    format!("component {} by a compound selector", u.class),
                ));
            }
            // A directive on an HTML element: the emitter instantiates it if
            // the metadata read from the program has nothing beyond what it
            // writes (`Directive::blocker`).
            // On a component's element (`<li-select [(ngModel)]>`), without
            // `@HostListener`: directive listeners there are not emitted yet.
            let pending = match &u.directive {
                None => Some("without metadata".to_string()),
                Some(d) => d.blocker(),
            };
            if let Some(p) = pending {
                return Err(refusal(
                    Reason::DirectiveBySelector,
                    format!("directive {} ({p})", u.class),
                ));
            }
        }
        Ok(())
    }

    /// Emits the text binding of `{{ … }}`: the `TextBinding` field, the
    /// `append` in `build()` and the update in `detectChangesInternal`.
    fn interpolation(
        &mut self,
        expr: &str,
        start: usize,
        end: usize,
        parent: &str,
    ) -> Result<(), Refusal> {
        let url = self.url(Reason::Interpolation)?;
        // Without a parent: the root of an embedded view (the `*` on an
        // `<ng-container>`) or projected content — the node is the
        // `TextBinding.element` itself, or the `createText(value)` when
        // immutable (`createTextBinding` without a `parentNode`), and goes into
        // the roots or into the projection list.
        let detached = parent.is_empty();
        let converted = self.convert_breakable(expr, Reason::Interpolation)?;
        let breaks = converted.text.contains(crate::expr::BREAK);
        // Without the static type it is not possible to choose between
        // `interpolateString`, `interpolate` and `updateTextWithPrimitive` —
        // and choosing wrong changes what the program does.
        let Some(ty) = converted.ty.clone() else {
            return Err(refusal(
                Reason::Interpolation,
                format!("unknown type of {}", converted.shape),
            ));
        };
        let bare = ty.trim_end_matches('?').to_string();
        let n = self.next_node;
        self.next_node += 1;
        // `visitInterpolation` with empty ends: a mutable primitive is read
        // directly (`_isPrimitiveCheck`), a literal becomes the text itself
        // (`o.literal('${value}')`), the rest goes through `interpolateString0`
        // (all `String`) or `interpolate0`. The type is the one from
        // `_TypeResolver`: a ternary, `!x`, `x!`, `??`, an index and a binary
        // that does not add two `String`s are `dynamic`.
        let mutable_primitive = !converted.immutable && primitive(&bare);
        let f = if bare == "String" {
            "interpolateString0"
        } else {
            "interpolate0"
        };
        if converted.immutable {
            // A value that does not change has no binding: the text is computed
            // once, in `build()`, as the official compiler does
            // (`isImmutable`).
            // The emitter writes `appendText(` before the value: the
            // `dom_helpers` import comes first.
            let dom = self.dom();
            let value = if converted.literal {
                literal_value(&converted.text)
            } else {
                let alias = self.imp.alias(INTERPOLATE);
                format!("{alias}.{f}({})", converted.text)
            };
            // In `build()` the import of a name from `exports:` is added now,
            // after `interpolate` (case j51).
            let value = resolve_deferred(self.imp, &value);
            if references_ctx(std::slice::from_ref(&value)) {
                self.uses_ctx_in_build = true;
            }
            if detached {
                self.lines
                    .push(format!("    final _text_{n} = {dom}.createText({value});"));
                self.roots.push(format!("_text_{n}"));
            } else {
                self.lines.push(format!(
                    "    final _text_{n} = {dom}.appendText({parent}, {value});"
                ));
            }
            return Ok(());
        }
        let Some(tb) = self.tb.clone() else {
            return Err(refusal(
                Reason::Interpolation,
                "interpolation without a text binding",
            ));
        };
        self.tb_used = true;
        self.fields.push(format!(
            "  final {tb}.TextBinding _textBinding_{n} = {tb}.TextBinding();"
        ));
        if detached {
            self.roots.push(format!("this._textBinding_{n}.element"));
        } else {
            self.lines.push(format!(
                "    {parent}.append(this._textBinding_{n}.element);"
            ));
        }
        let access = converted.text;
        // A call that breaks inside `interpolateString0(..)` gets the
        // formatter's continuation indentation (+4, case j21); inside
        // `updateTextWithPrimitive(..)`, no case.
        if breaks && (mutable_primitive || converted.immutable) {
            return Err(refusal(
                Reason::Interpolation,
                "call with a named argument in a primitive interpolation",
            ));
        }
        let access = crate::expr::indent_breaks(&access, 4);
        let update = if mutable_primitive {
            format!("updateTextWithPrimitive({access})")
        } else {
            // In change detection the import belongs to the moment it is
            // written.
            format!("updateText({}.{f}({access}))", deferred(INTERPOLATE))
        };
        self.detection.push(format!(
            "    this._textBinding_{n}.{update} /* REF:{url}:{start}:{end} */;"
        ));
        Ok(())
    }

    /// The value of a text with `{{ }}` (`title="a {{b}}"`, `x="{{y}}"`), as
    /// `_createPropertyForAttribute`/`convertInterpolation` write it:
    /// `interpolateStringN` when everything is a `String`, `interpolateN`
    /// otherwise; a pure literal is the text itself; immutable, computed once.
    /// A mutable primitive expression alone (`_maybeOptimizeInterpolation`) is
    /// checked raw and interpolated only where the value is used (`in_action`).
    /// Allocates the binding number (`k`).
    ///
    /// The attribute value is the raw one: the ngast tokenizer does not decode
    /// entities in it (`02-parser-entry-template.md` R1.5/R1.6), and the `&&` of
    /// `placeholder="{{ a && b }}"` reaches the expression parser like that
    /// (§A6).
    fn interpolated_value(&mut self, text: &str, reason: Reason) -> Result<Interpolated, Refusal> {
        let (texts, exprs) = interpolation_parts(text)
            .ok_or_else(|| refusal(reason, "malformed interpolated attribute"))?;
        let mut converted = Vec::new();
        for e in &exprs {
            converted.push(self.convert(e, reason)?);
        }
        // The type only chooses the family with 1 or 2 expressions; with 3 or
        // more it is always `interpolateN` (`interpolateFallback`).
        let mut types = Vec::new();
        for c in &converted {
            match &c.ty {
                Some(t) => types.push(t.trim_end_matches('?').to_string()),
                None if converted.len() > 2 => types.push("dynamic".into()),
                None => {
                    return Err(refusal(reason, format!("unknown type of {}", c.shape)));
                }
            }
        }
        // `_compressWhitespacePreceding`/`Following`: only the ends, and only
        // when there is a line break.
        let text_count = texts.len();
        let texts: Vec<String> = texts
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let t = if self.preserve_whitespace {
                    t.replace(NGSP, " ")
                } else if i == 0 {
                    compress_before(t)
                } else if i == text_count - 1 {
                    compress_after(t)
                } else {
                    t.replace(NGSP, " ")
                };
                literal(&t)
            })
            .collect();
        let all_string = types.iter().all(|t| t == "String");
        let family = if all_string {
            "interpolateString"
        } else {
            "interpolate"
        };
        let immutable = converted.iter().all(|c| c.immutable);
        let k = self.next_binding;
        self.next_binding += 1;
        let interp = deferred(INTERPOLATE);
        // The value of the interpolation with each expression in place.
        let value_with = |exprs: &[String], family: &str| -> String {
            match exprs {
                [e] if texts[0] == "''" && texts[1] == "''" => {
                    format!("{interp}.{family}0({e})")
                }
                [e] => format!("{interp}.{family}1({}, {e}, {})", texts[0], texts[1]),
                [e0, e1] => format!(
                    "{interp}.{family}2({}, {e0}, {}, {e1}, {})",
                    texts[0], texts[1], texts[2]
                ),
                // `interpolateFallback` (`expression_converter.dart:219-227`):
                // the texts and the expressions interleaved in a list.
                _ => {
                    let mut items = Vec::new();
                    for (t, e) in texts.iter().zip(exprs) {
                        items.push(t.clone());
                        items.push(e.clone());
                    }
                    items.extend(texts.last().cloned());
                    format!("{interp}.interpolateN([{}])", items.join(", "))
                }
            }
        };
        let expr_texts: Vec<String> = converted.iter().map(|c| c.text.clone()).collect();
        // A literal expression with empty ends is the text itself.
        let pure_literal =
            converted.len() == 1 && converted[0].literal && texts[0] == "''" && texts[1] == "''";
        let constant = if pure_literal {
            literal_value(&converted[0].text)
        } else {
            value_with(&expr_texts, family)
        };
        let primitive = converted.len() == 1 && !converted[0].immutable && primitive(&types[0]);
        let (check, in_action) = if primitive {
            (
                converted[0].text.clone(),
                value_with(&[format!("currVal_{k}")], "interpolate"),
            )
        } else {
            (value_with(&expr_texts, family), format!("currVal_{k}"))
        };
        Ok(Interpolated {
            converted,
            immutable,
            k,
            constant,
            check,
            in_action,
        })
    }

    /// An attribute with `{{ }}` (`title="a {{b}}"`): the official compiler
    /// treats it as a property binding (`_createPropertyForAttribute`), with
    /// the name passed through the schema (`class` → `className`, which becomes
    /// `updateChildClass`) and the value `Interpolation(strings, exprs)`. It is
    /// emitted after the element's `[x]` bindings.
    fn interpolated_attribute(
        &mut self,
        a: &crate::html::Binding,
        target: &str,
    ) -> Result<BoundProperty, Refusal> {
        let url = self.url(Reason::Interpolation)?;
        let name = a.name.as_str();
        // `_createPropertyForAttribute` passes the name to the same
        // `createElementPropertyAst` as `[x]`: `attr.x`, `class.x` and
        // `style.x` are the usual bindings (case j107). Without a prefix, a
        // name that the schema renames (`readonly`, `tabindex`, `for`…) is not
        // emitted yet; one that is not even a DOM property (`data-x`, `aria-x`)
        // is an error in the official compiler.
        let has_prefix = ["attr.", "class.", "style."]
            .iter()
            .any(|p| name.starts_with(p));
        if name != "class"
            && !has_prefix
            && (!name.chars().all(|c| c.is_ascii_lowercase())
                // `for` and `formaction` are not properties in the schema
                // (`_attrToPropMap` does not map them): an error in the
                // official compiler.
                || matches!(name, "for" | "formaction"))
        {
            return Err(refusal(
                Reason::Interpolation,
                "interpolated attribute renamed, protected or outside the schema",
            ));
        }
        // The value is the raw one (without decoding entities, §A6).
        let v = self.interpolated_value(&a.value, Reason::Interpolation)?;
        // The value of the binding is the `Interpolation`, not the expression
        // inside: it is never null (`canBeNull` gives `false`, hence the
        // `setAttribute` of `visitAttributeBinding`) and `_TypeResolver` types
        // it as `String`. In the primitive shortcut
        // (`_shouldInterpolateAfterCheck`) the check is the raw expression, and
        // the type that counts is its own (`[style.x]` with an `int` gets the
        // `.toString()`, case j107; `calc({{a - b}}%)` does not, the
        // `material_slider`).
        let primitive = v.converted.len() == 1 && v.check == v.converted[0].text;
        let mut converted = v.converted;
        if let Some(c) = converted.first_mut() {
            c.can_be_null = false;
            if !primitive {
                c.ty = Some("String".into());
            }
            c.scope = None;
        }
        let k = v.k;
        let synthetic = crate::html::Binding {
            name: a.name.clone(),
            value: a.value.clone(),
            start: a.start,
            end: a.end,
            without_value: a.without_value,
        };
        let (start, end) = (a.start, a.end);
        if v.immutable {
            let action = self.action(&synthetic, target, &v.constant, &converted[0])?;
            return Ok(BoundProperty::Constant(format!(
                "      {action} /* REF:{url}:{start}:{end} */;"
            )));
        }
        let (check, in_action) = (v.check, v.in_action);
        self.expr_fields.push(format!("  Object? _expr_{k};"));
        let action = self.action(&synthetic, target, &in_action, &converted[0])?;
        let chk = deferred(CHECK_BINDING);
        let source = literal(&a.value);
        Ok(BoundProperty::Dynamic(format!(
            "    final currVal_{k} = {check};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, {source}, '{url}')) {{\n      {action} /* REF:{url}:{start}:{end} */;\n      this._expr_{k} = currVal_{k};\n    }}"
        )))
    }

    /// Emits the child nodes of `parent`. Outside collection mode, the first
    /// shape that the emitter does not cover returns `Err` and the whole file
    /// goes back to `build_runner`; in collection mode, each refusal is
    /// recorded and the walk goes on.
    fn nodes(&mut self, nodes: &[Node], parent: &str) -> Result<(), Refusal> {
        for node in nodes {
            if let Err(r) = self.node(node, parent) {
                self.record_refusal(r)?;
                // In collection mode, what is below a refused node also counts:
                // descend with any parent (the output is discarded).
                if let Node::Element(e) = node {
                    self.collect_below(e);
                }
            }
        }
        Ok(())
    }

    /// Collects the refusals of the subtree of an element that was not emitted.
    /// The locals of a `*` go in without a type, so that the expressions that
    /// mention them are examined for what they are.
    fn collect_below(&mut self, e: &crate::html::Element) {
        if !self.collecting() || e.children.is_empty() {
            return;
        }
        let saved = (
            self.locals.clone(),
            self.embedded,
            self.tb.clone(),
            self.local_decls.clone(),
            self.root_locals.clone(),
            self.own_locals.clone(),
        );
        // The text binding is allocated per view; here the output is discarded.
        self.tb.get_or_insert_with(|| "_coleta".into());
        if let Some(micro) = e.star_micro() {
            // The type of the collection, if its expression converts; otherwise
            // the locals stay `dynamic`.
            let collection = micro
                .properties
                .iter()
                .find(|(p, _)| p.ends_with("Of"))
                .and_then(|(_, x)| self.convert(x, Reason::Binding).ok())
                .and_then(|c| c.ty.map(|t| (t, c.scope)));
            let locals = self
                .micro_locals(&micro, collection.as_ref())
                .unwrap_or_else(|_| {
                    let mut l = self.locals.clone();
                    for (name, _) in &micro.locals {
                        l.insert(
                            name.clone(),
                            crate::expr::Local {
                                dart: format!("local_{name}"),
                                ty: "dynamic".into(),
                                scope: None,
                            },
                        );
                    }
                    l
                });
            self.locals = locals;
            for (name, _) in &micro.locals {
                self.local_decls.insert(name.clone(), Ok(String::new()));
            }
            // An embedded view emitted further below inherits these locals as
            // ancestor ones (collection only gathers refusals: the origin does
            // not matter).
            self.own_locals.extend(micro.locals.iter().cloned());
            self.embedded = true;
        }
        let _ = self.nodes(&e.children, "_el_coleta");
        (
            self.locals,
            self.embedded,
            self.tb,
            self.local_decls,
            self.root_locals,
            self.own_locals,
        ) = saved;
    }

    /// A template node.
    fn node(&mut self, node: &Node, parent: &str) -> Result<(), Refusal> {
        match node {
            Node::Comment(_) => {}
            Node::Str(t) => {
                // Without a parent: the root of an embedded view or projected
                // content (`createText` from `dom_helpers`, and the node goes
                // into the roots or into the projection list).
                let n = self.next_node;
                self.next_node += 1;
                let dom = self.dom();
                let text = literal(t);
                if parent.is_empty() {
                    self.lines
                        .push(format!("    final _text_{n} = {dom}.createText({text});"));
                    self.roots.push(format!("_text_{n}"));
                } else {
                    self.lines.push(format!(
                        "    final _text_{n} = {dom}.appendText({parent}, {text});"
                    ));
                }
            }
            Node::Element(e) => {
                // A hand-written `<template>` is an embedded view, not an HTML
                // element (`EmbeddedTemplateAst`).
                // (What [`template_as_container`] did not rewrite or mark as
                // [`TEMPLATE_MARK`] is not translated yet.)
                if e.name.eq_ignore_ascii_case("template")
                    && e.star.as_ref().is_none_or(|l| l.name != TEMPLATE_MARK)
                {
                    return Err(refusal(
                        Reason::Binding,
                        "<template> written in the template",
                    ));
                }
                // `<ng-container>` without `*` (`visitNgContainer`): the
                // children go straight to the parent, without a node or an
                // index.
                if e.name == "ng-container" && e.star.is_none() {
                    return self.container(e, parent);
                }
                if e.star.is_none() {
                    self.element_guard(e)?;
                }
                // `@i18n` only on an HTML element: on a child it changes the
                // projected content. With `*` the annotation stays on the
                // element, which goes to the embedded view (case j01).
                if !e.annotations.is_empty() && self.children.contains_key(&e.name) {
                    return Err(refusal(Reason::I18n, "@i18n on a child component"));
                }
                // `*` on a child: the child goes to the embedded view, like any
                // element.
                if let (Some(star), true) = (&e.star, self.children.contains_key(&e.name)) {
                    return self.structural(e, star, parent);
                }
                if let Some(child) = self.children.get(&e.name).cloned() {
                    return self.child_component(e, &child, parent);
                }
                // A tag that is not HTML, with only a directive
                // (`<pg-breadcrumb-item>`): it is an element typed `Element`
                // (`identifierFromTagName`).
                if !dom::tag_html(&e.name)
                    && e.name != "ng-container"
                    && self.namespace_of(e).is_none()
                    && matched_directives(self.used, e).is_empty()
                {
                    return Err(refusal(
                        Reason::ComponentInTemplate,
                        "tag that is neither HTML nor a known component",
                    ));
                }
                // The name of an ecosystem directive (`ngClass`, `ngModel`…) in
                // a binding belongs to a directive, it is not a DOM property:
                // emitting `setProperty` there would do something else — unless
                // a directive from the catalog receives it.
                let matched = matched_directives(self.used, e);
                if let Some(l) = e
                    .properties
                    .iter()
                    .chain(e.events.iter())
                    .chain(e.attributes.iter())
                    .chain(e.bananas.iter())
                    .find(|l| {
                        is_directive_name(&l.name)
                            && !consumes_input(&matched, &l.name)
                            && !consumes_output(&matched, &l.name)
                    })
                {
                    return Err(refusal(
                        Reason::Directive,
                        format!(
                            "`{}` without a directive that receives it",
                            l.name.split('.').next().unwrap_or(&l.name)
                        ),
                    ));
                }
                if let Some(star) = &e.star {
                    if star.name == TEMPLATE_MARK {
                        return self.hand_template(e, parent);
                    }
                    return self.structural(e, star, parent);
                }
                self.html_element(e, parent)?;
            }
            Node::Interpolation { expr, start, end } => {
                self.interpolation(expr, *start, *end, parent)?
            }
            Node::Content { selector } => {
                // `<ng-content>` does not consume a node index; the number is
                // the ordinal of the projection in the template
                // (`ngContentSelectors`), with or without `select`.
                let _ = selector;
                let i = self.next_projection;
                self.next_projection += 1;
                // Without a parent (the root of an embedded view, or content
                // that goes to another child): the projected nodes go whole
                // into the list of roots (`ProjectedNodes`, a list), case j56.
                if parent.is_empty() {
                    self.roots
                        .push(format!("{LIST_ROOT}this.projectedNodes[{i}]"));
                } else {
                    self.lines.push(format!("    this.project({parent}, {i});"));
                }
            }
        }
        Ok(())
    }

    /// `<ng-container>`: it only groups. The children are created in its parent
    /// (in the embedded view of a `*`, detached: each one is a root). The ngast
    /// only accepts `*`, `#ref` and annotations on it; `#ref` and `@i18n` not
    /// yet, nor the container in content projected into a child (its
    /// `ngContentIndex` is computed from the container's empty selector).
    fn container(&mut self, e: &crate::html::Element, parent: &str) -> Result<(), Refusal> {
        let has_any = !(e.attributes.is_empty()
            && e.properties.is_empty()
            && e.events.is_empty()
            && e.bananas.is_empty()
            && e.references.is_empty()
            && e.annotations.is_empty());
        if has_any {
            return Err(refusal(
                Reason::Binding,
                "<ng-container> with an attribute, binding, #ref or annotation",
            ));
        }
        if parent.is_empty() && self.projected_parent.is_some() {
            return Err(refusal(
                Reason::Projection,
                "<ng-container> in projected content",
            ));
        }
        self.nodes(&e.children, parent)
    }

    /// An HTML element: creation, attributes, bindings, events, style and the
    /// children.
    fn html_element(&mut self, e: &crate::html::Element, parent: &str) -> Result<(), Refusal> {
        // The catalog directives that match the node, and their `[(x)]`
        // desugared like the `DesugarVisitor` does: the input `x` and the event
        // `xChange` with `value = $event`.
        let matched = matched_directives(self.used, e);
        let mut properties = e.properties.clone();
        let mut events = e.events.clone();
        for b in &e.bananas {
            let change = format!("{}Change", b.name);
            if !consumes_input(&matched, &b.name) || !consumes_output(&matched, &change) {
                self.record_refusal(refusal(Reason::Binding, "[(x)] on an HTML element"))?;
                continue;
            }
            properties.push(b.clone());
            events.push(crate::html::Binding {
                name: change,
                value: format!("{} = $event", b.value),
                ..b.clone()
            });
        }
        // `#ref="x"`: the node's directive with `exportAs: 'x'` (only one; more
        // than one is an error in the official compiler). The local is its
        // instance (`referenceTokens` in `compile_element.dart`).
        let exported = |r: &crate::html::Binding| {
            let found: Vec<_> = matched
                .iter()
                .filter(|d| d.export_as.as_deref() == Some(r.value.as_str()))
                .collect();
            match found.as_slice() {
                [d] => Some(std::sync::Arc::clone(d)),
                _ => None,
            }
        };
        // `#ref` only in the shape that changes nothing on the node; its value
        // is registered further on, for `@ViewChild`. With a value and the
        // exported directive on the node, a name not read by an expression is
        // also just a name (for its instance).
        if let Some(r) = e.references.iter().find(|r| {
            (!r.value.is_empty() && exported(r).is_none())
                || !(self.free_refs.contains(&r.name)
                    || self.local_refs.contains(&r.name)
                    || self.queried_refs.iter().any(|(n, _, _)| *n == r.name)
                    || !r.value.is_empty())
        }) {
            self.record_refusal(refusal(
                Reason::Binding,
                if !r.value.is_empty() {
                    "#ref with a value and no node directive exporting it"
                } else if self.ambiguous_refs.contains(&r.name) {
                    "#ref repeated or shadowed by `let`"
                } else if self.embedded {
                    "#ref in an embedded view"
                } else {
                    "#ref used in an expression"
                },
            ))?;
        }
        let n = self.next_node;
        self.next_node += 1;
        let resolved = if matched.is_empty() {
            None
        } else {
            let providers = self.providers_above();
            let above = crate::directives::Above {
                providers: &providers,
                uncertain: self.uncertain_above > 0,
                of_view: self.view_tokens,
            };
            // What queries with `read:` read from this node is eager.
            let queried: Vec<crate::directives::Token> = self
                .reads
                .iter()
                .filter(|(key, _)| matches_key(e, key, self.children))
                .map(|(_, t)| t.clone())
                .collect();
            match crate::directives::resolve_queried(&matched, n, Some(above), &queried) {
                Ok(r) => Some(r),
                Err(f) => {
                    self.record_refusal(refusal(Reason::DirectiveBySelector, f))?;
                    None
                }
            }
        };
        // The result of each query with a provider `read:`: the instance of the
        // token on the node ([`read_key`]).
        for (key, t) in self.reads {
            if !matches_key(e, key, self.children) {
                continue;
            }
            let instance = resolved.as_ref().and_then(|r| {
                r.instances
                    .iter()
                    .find(|i| i.token == *t || i.aliases.contains(t))
            });
            match instance {
                Some(i) => {
                    let read = format!("this.{}", i.read);
                    let k = read_key(key, t);
                    self.refs.insert(k.clone(), read.clone());
                    self.refs_in_order.push((k, read));
                }
                None => self.record_refusal(refusal(
                    Reason::ViewChildOnChild,
                    "@ViewChild(.., read: T) of a token the element does not provide",
                ))?,
            }
        }
        if !self.has_doc {
            self.has_doc = true;
            let document = self.imp.dom("document");
            self.lines.push(format!("    final doc = {document};"));
        }
        let dom = self.dom();
        // With a namespace, the name becomes `@ns:tag` (`mergeNsAndName`): it
        // is not HTML (`Element`, the `NonHtml` variants) and is emitted with
        // `createElementNS` and a separate `append` (`createElementNs`,
        // `_initializeAndAppendNode`).
        let namespace = self.namespace_of(e);
        if let Some((ns, _)) = &namespace {
            if !matched.is_empty() || self.children.contains_key(&e.name) {
                return Err(refusal(
                    Reason::ComponentInTemplate,
                    "directive on an element with a namespace",
                ));
            }
            if e.attributes.iter().any(|a| a.name.contains(':')) {
                return Err(refusal(
                    Reason::Binding,
                    "namespaced attribute on an SVG element",
                ));
            }
            if properties.iter().any(|l| {
                !["attr.", "class.", "style."]
                    .iter()
                    .any(|p| l.name.starts_with(p))
            }) {
                return Err(refusal(
                    Reason::Binding,
                    "[property] on an element with a namespace",
                ));
            }
            let _ = ns;
        }
        let tag = match &namespace {
            Some((ns, name)) => format!("@{ns}:{name}"),
            None => e.name.to_ascii_lowercase(),
        };
        let ns_uri = namespace.as_ref().map(|(ns, _)| match ns.as_str() {
            "xlink" => "'http://www.w3.org/1999/xlink'".to_string(),
            "svg" => "'http://www.w3.org/2000/svg'".to_string(),
            "xhtml" => "'http://www.w3.org/1999/xhtml'".to_string(),
            _ => "null".to_string(),
        });
        // A projected node has no parent: the official compiler creates it
        // detached, with `document.createElement`, and hands it to the child
        // (`_createElementAndAppend`, with `parent == null`).
        let creation = if let (Some((_, name)), Some(uri)) = (&namespace, &ns_uri) {
            format!("doc.createElementNS({uri}, '{name}')")
        } else if parent.is_empty() {
            let util = self.imp.alias(UTILITIES);
            // In 9 the detached node carries its type (`unsafeCast<T>`, for the
            // static dispatch of the *extension types* of `package:web`,
            // `NX:compiler/view_compiler/compile_view.dart:822-834`).
            let node_type = if crate::dialect::current() == crate::dialect::Dialect::Ngx {
                format!("<{}>", self.imp.dom(dom::tag_type(&tag)))
            } else {
                String::new()
            };
            format!("{util}.unsafeCast{node_type}(doc.createElement('{tag}'))")
        } else {
            match tag.as_str() {
                "div" => format!("{dom}.appendDiv(doc, {parent})"),
                "span" => format!("{dom}.appendSpan(doc, {parent})"),
                _ => {
                    let ty = self.imp.dom(dom::tag_type(&tag));
                    format!("{dom}.appendElement<{ty}>(doc, {parent}, '{tag}')")
                }
            }
        };
        // An element with a property binding becomes a field of the view:
        // `detectChangesInternal` needs it after `build()`. An event alone does
        // not require a field.
        let ty = if namespace.is_some() {
            "Element"
        } else {
            dom::tag_type(&tag)
        };
        // A `#ref` that only queries with a provider `read:` look for does not
        // read the node.
        let queries_node = |name: &String| {
            self.queried_refs.iter().any(|(n, _, _)| n == name)
                && !self.provider_only_refs.contains(name)
        };
        let read_as_local = e.references.iter().any(|r| {
            r.value.is_empty() && (self.local_refs.contains(&r.name) || queries_node(&r.name))
        });
        let read_from_embedded = e
            .references
            .iter()
            .find(|r| r.value.is_empty() && self.embedded_refs.contains(&r.name))
            .map(|r| r.name.clone());
        let target = if !binds_on_element(e, &matched) && !read_as_local {
            self.lines.push(format!("    final _el_{n} = {creation};"));
            format!("_el_{n}")
        } else {
            let field_type = self.imp.dom(ty);
            let field = format!("  late final {field_type} _el_{n};");
            let query_pos = e
                .references
                .iter()
                .filter(|r| r.value.is_empty() && !self.provider_only_refs.contains(&r.name))
                .filter_map(|r| self.queried_refs.iter().position(|(n, _, _)| *n == r.name))
                .min();
            let only_in_event = (!binds_on_element(e, &matched))
                .then(|| {
                    e.references
                        .iter()
                        .find(|r| r.value.is_empty() && self.event_only_refs.contains(&r.name))
                        .map(|r| r.name.clone())
                })
                .flatten();
            match (read_from_embedded, query_pos, only_in_event) {
                (Some(name), _, _) => self.embedded_el_fields.push((name, field)),
                (None, Some(k), _) => self.queried_el_fields.push((k, field)),
                (None, None, Some(name)) => self.event_el_fields.push((name, field)),
                (None, None, None) => {
                    self.el_field_refs.insert(
                        field.clone(),
                        e.references
                            .iter()
                            .filter(|r| r.value.is_empty())
                            .map(|r| r.name.clone())
                            .collect(),
                    );
                    self.el_fields.push(field);
                }
            }
            self.lines.push(format!("    this._el_{n} = {creation};"));
            format!("this._el_{n}")
        };
        if namespace.is_some() && !parent.is_empty() {
            self.lines.push(format!("    {parent}.append({target});"));
        }
        // With a `ViewContainer` (a directive that injects `ViewContainerRef`),
        // it is the root (`vcAppEl ?? renderNode`, case j49).
        let container = resolved.as_ref().is_some_and(|r| r.container);
        if parent.is_empty() {
            self.roots.push(if container {
                format!("this._appEl_{n}")
            } else {
                target.clone()
            });
        }
        if let Some(res) = &resolved {
            self.register_node_by_position(e.start, &res.instances);
        }
        // The result of a `@ViewChild(Directive)` ([`type_key`]): the
        // field of each directive of the node, before the `#ref`s (the order of
        // `_resolvedProvidersArray` in `beforeChildren`, case j117).
        if let Some(res) = &resolved {
            for (d, field) in &res.directives {
                if d.is_component {
                    continue;
                }
                let key = type_key(&d.uri, &d.class);
                let read = format!("this.{field}");
                self.refs.insert(key.clone(), read.clone());
                self.refs_in_order.push((key, read));
            }
        }
        // `renderNode.toReadExpr()`: the local or the field, depending on how
        // the node was declared.
        for r in &e.references {
            // With a value, the instance of the exported directive.
            let read = match (r.value.is_empty(), exported(r), &resolved) {
                (true, _, _) => target.clone(),
                (false, Some(d), Some(res)) => match res
                    .directives
                    .iter()
                    .find(|(x, _)| std::sync::Arc::ptr_eq(x, &d))
                {
                    Some((_, field)) => format!("this.{field}"),
                    None => continue,
                },
                _ => continue,
            };
            self.refs.insert(r.name.clone(), read.clone());
            self.refs_in_order.push((r.name.clone(), read));
        }
        // The `@i18n` messages of the node: the attributes' one replaces the
        // literal, the children's one replaces the text.
        let mut i18n = if e.annotations.is_empty() {
            Vec::new()
        } else {
            match i18n_metadata(e) {
                Ok(m) => m,
                Err(r) => {
                    self.record_refusal(r)?;
                    Vec::new()
                }
            }
        };
        let i18n_children = i18n
            .iter()
            .position(|(k, _)| k.is_none())
            .map(|i| i18n.remove(i).1);
        for (attribute, _) in &i18n {
            // `I18nMessage(astNode.value!, ..)`: the written value, without
            // interpolation; an entity, a special attribute shape or a missing
            // attribute (or a bound one: `[x]`) not yet.
            let eligible = e.attributes.iter().any(|a| {
                Some(&a.name) == attribute.as_ref()
                    && !a.value.contains("{{")
                    && !a.value.contains('&')
                    && !matches!(a.name.as_str(), "class" | "tabindex" | "tabIndex" | "style")
            });
            if !eligible {
                self.record_refusal(refusal(
                    Reason::I18n,
                    "@i18n:x without a simple written attribute x",
                ))?;
            }
        }
        // Attributes are emitted in alphabetical order (`_toSortedBindings`).
        let mut attributes = e.attributes.clone();
        attributes.sort_by(|a, b| a.name.cmp(&b.name));
        for a in attributes.iter().filter(|a| !a.value.contains("{{")) {
            let value = match i18n.iter().find(|(k, _)| k.as_ref() == Some(&a.name)) {
                Some((_, m)) => {
                    let m = m.clone();
                    match self.message(&a.value, &m) {
                        Ok(field) => field,
                        Err(r) => {
                            self.record_refusal(r)?;
                            literal(&a.value)
                        }
                    }
                }
                None => literal(&a.value),
            };
            if a.name == "class" {
                // `writeLiteralAttributeValues`: `NonHtml` outside HTML.
                let method = if dom::tag_html(&tag) {
                    "updateChildClass"
                } else {
                    "updateChildClassNonHtml"
                };
                self.lines
                    .push(format!("    this.{method}({target}, {value});"));
            } else if a.name == "tabindex" || a.name == "tabIndex" {
                // `TabIndexBinding` (`binding_converter.dart`): the literal
                // becomes `el.tabIndex = N` (`visitTabIndexBinding`); a
                // non-integer is a compile error in the official compiler.
                match a.value.trim().parse::<i64>() {
                    Ok(n) if a.value.trim() == a.value => {
                        self.lines.push(format!("    {target}.tabIndex = {n};"));
                    }
                    _ => self.record_refusal(refusal(Reason::Binding, "non-integer tabindex"))?,
                }
            } else {
                // A written `style` is an attribute like any other
                // (`setAttribute` in `build()`), also together with
                // `[style.x]`, which only writes during change detection (case
                // j74).
                let dom = self.dom();
                let name = &a.name;
                self.lines.push(format!(
                    "    {dom}.setAttribute({target}, '{name}', {value});"
                ));
            }
        }
        self.current_tag = tag.clone();
        // Bindings are numbered in document order — the parent's before the
        // children's —, so they are registered before descending. Events too:
        // the parent's listener comes before the children's, and all of them
        // are emitted together at the end of `build()` (see `listeners`).
        let mut bindings = Vec::new();
        // A `[x]` that a directive receives is an input of that directive, not
        // of the node.
        for l in properties
            .iter()
            .filter(|l| !consumes_input(&matched, &l.name))
        {
            match self.property(l, &target) {
                Ok(x) => bindings.push(x),
                Err(r) => self.record_refusal(r)?,
            }
        }
        // Interpolated attributes come after the `[x]`s, in written order
        // (`_visitProperties`).
        // What a directive of the node receives as an input belongs to it (case
        // j59).
        for a in e
            .attributes
            .iter()
            .filter(|a| a.value.contains("{{") && !consumes_input(&matched, &a.name))
        {
            match self.interpolated_attribute(a, &target) {
                Ok(x) => bindings.push(x),
                Err(r) => self.record_refusal(r)?,
            }
        }
        self.write_bindings(bindings);
        // The listeners are emitted together at the end of `build()`, in the
        // order in which the elements appear (`bindView` after `_buildView`):
        // the template ones that are not a directive output, then the
        // `@HostListener`s of the directives (`_visitHostListeners`).
        let mut node_el = e.clone();
        node_el.events = events
            .iter()
            .filter(|l| !consumes_output(&matched, &l.name))
            .cloned()
            .collect();
        if let Some(r) = &resolved {
            self.listeners_with_host(&node_el, &matched, r, &target)?;
        } else {
            self.events(&node_el, &target)?;
        }
        if self.with_style {
            // Style isolation by attribute: the element goes into the
            // component's scope (`shimCssForNode`: `addShimE` on a node typed
            // `Element`, a tag that is not HTML).
            let method = if ty == "Element" {
                "addShimE"
            } else {
                "addShimC"
            };
            self.lines.push(format!("    this.{method}({target});"));
        }
        let mut injector = None;
        if let Some(r) = &resolved {
            // The `ViewContainer` is born with the `CompileElement`, before the
            // providers (its field before the directives' ones).
            if r.container {
                let vc = self.imp.q(VIEW_CONTAINER);
                let parent_index = if parent.is_empty() {
                    self.projected_parent
                        .map_or_else(|| "null".to_string(), |k| k.to_string())
                } else {
                    element_index(parent)
                };
                self.child_fields
                    .push(format!("  late final {vc}ViewContainer _appEl_{n};"));
                self.lines.push(format!(
                    "    this._appEl_{n} = {vc}ViewContainer({n}, {parent_index}, this, {target});"
                ));
                self.anchors.push(format!("_appEl_{n}"));
            }
            self.node_directives(e, r, &target, &properties, &events)?;
            let injectables: Vec<(Vec<crate::directives::Token>, String)> = r
                .instances
                .iter()
                .filter(|i| !i.injectable_by.is_empty())
                .map(|i| (i.injectable_by.clone(), i.read.clone()))
                .collect();
            if !injectables.is_empty() {
                injector = Some(self.injectors.len());
                self.injectors.push((n, n, injectables));
            }
            let mut above = self.stack.clone();
            above.push((n, true));
            let providers = r
                .instances
                .iter()
                .flat_map(|i| {
                    std::iter::once(&i.token)
                        .chain(&i.aliases)
                        .map(|t| (t.clone(), i.read.clone(), None))
                })
                .collect();
            self.registrations.push(Registration {
                above,
                providers,
                element: target.clone(),
            });
        }
        // The injectable providers of this node stay above the children
        // (`injectable_by`: the visible ones and the aliases).
        let above_len = self.above.len();
        if let Some(r) = &resolved {
            for i in &r.instances {
                // Visibility only decides `injectorGetInternal` (other views);
                // in the same template a node below finds even a local provider
                // by its token (case j41).
                if i.lazy {
                    self.lazy_above
                        .insert((i.read.clone(), self.this_view_class()));
                }
                if i.injectable_by.is_empty() {
                    self.above.push((i.token.clone(), i.read.clone(), None));
                }
                for t in &i.injectable_by {
                    self.above.push((t.clone(), i.read.clone(), None));
                }
            }
        }
        self.stack.push((n, resolved.is_some()));
        let r = match &i18n_children {
            Some(m) => self.i18n_children(e, m, &target),
            None => {
                // The descendants inherit the namespace.
                let saved = std::mem::replace(
                    &mut self.current_ns,
                    namespace.as_ref().map(|(ns, _)| ns.clone()),
                );
                let r = self.nodes(&e.children, &target);
                self.current_ns = saved;
                r
            }
        };
        self.stack.pop();
        self.above.truncate(above_len);
        if let Some(res) = &resolved {
            self.directive_queries(e, &res.directives, n)?;
            self.hooks_after_children(&res.directives);
        }
        // `ProviderNode(nodeIndex, nodeIndex + childNodeCount)`.
        if let Some(i) = injector {
            self.injectors[i].1 = self.next_node - 1;
        }
        r
    }

    /// The children of an element with `@i18n`: a single text becomes a message
    /// (`internationalize`, `_textMessage`) and a text node with it. With HTML
    /// inside ([`message_with_html`]) the message is a static method with one
    /// parameter per tag (`createI18nMessage`), and the node is a
    /// `DocumentFragment` (`createHtml`): `final _html_n =
    /// createTrustedHtml(_message_K('<b>', '</b>'));` and
    /// `parent.append(_html_n)`.
    fn i18n_children(
        &mut self,
        e: &crate::html::Element,
        m: &MetaI18n,
        target: &str,
    ) -> Result<(), Refusal> {
        let [Node::Str(t)] = e.children.as_slice() else {
            return self.i18n_html_children(e, m, target);
        };
        if t.trim().is_empty() {
            return self.record_refusal(refusal(Reason::I18n, "@i18n message with HTML or empty"));
        }
        let field = self.message(t, m)?;
        let n = self.next_node;
        self.next_node += 1;
        let dom = self.dom();
        self.lines.push(format!(
            "    final _text_{n} = {dom}.appendText({target}, {field});"
        ));
        Ok(())
    }

    /// The `@HostListener`s of several directives of the node for the same
    /// event: a `_handleEvent_N` that calls each one, in order.
    fn group_handler(&mut self, list: &[(String, crate::directives::Listener)]) -> String {
        let n = self.event_methods.len();
        let body: Vec<String> = list
            .iter()
            .map(|(field, o)| format!("    this.{field}.{}({});", o.method, o.args))
            .collect();
        self.event_methods.push(format!(
            "\n  void _handleEvent_{n}({}) {{\n{}\n  }}\n",
            event_parameter(&self.event_type),
            body.join("\n")
        ));
        format!("this.eventHandler1(this._handleEvent_{n})")
    }

    /// The handler of a directive's `@HostListener`: a method without a
    /// parameter or with `$event` is a tear-off of the instance; with another
    /// argument, a `_handleEvent_N` that calls the method.
    fn host_handler(&mut self, field: &str, o: &crate::directives::Listener) -> String {
        let m = &o.method;
        match o.args.as_str() {
            "" => format!("this.eventHandler0(this.{field}.{m})"),
            "$event" => format!("this.eventHandler1(this.{field}.{m})"),
            args => {
                let n = self.event_methods.len();
                self.event_methods.push(format!(
                    "\n  void _handleEvent_{n}({}) {{\n    this.{field}.{m}({args});\n  }}\n",
                    event_parameter(&self.event_type)
                ));
                format!("this.eventHandler1(this._handleEvent_{n})")
            }
        }
    }

    /// The providers of the node's directives: fields, creation in `build()`
    /// (and the devtools `registerDirective`), inputs and hooks in change
    /// detection, and the outputs.
    fn node_directives(
        &mut self,
        e: &crate::html::Element,
        r: &crate::directives::ResolvedNode,
        target: &str,
        properties: &[crate::html::Binding],
        events: &[crate::html::Binding],
    ) -> Result<(), Refusal> {
        self.create_instances(e, &r.instances, target, "this")?;
        self.register_directives(&r.directives, target);
        self.bind_directives(
            e,
            &r.instances,
            &r.directives,
            target,
            properties,
            events,
            true,
            true,
        )
    }

    fn create_instances(
        &mut self,
        e: &crate::html::Element,
        instances: &[crate::directives::Instance],
        target: &str,
        // `ChangeDetectorRef` on the node: the child's `componentView`, or the
        // view itself (`compile_element.dart:199`).
        detector: &str,
    ) -> Result<(), Refusal> {
        use crate::directives::{Argument, Creation, Token};
        for inst in instances {
            // A child's `providers:` entry that is not an alias: written as in
            // the host view (`ProviderSource.build`). A lazy one is a field
            // with an initializer; what the child or a directive of the node
            // asks for is emitted in `build()`, in provider order (cases j61,
            // j67).
            if let Creation::Expression(_) | Creation::Multi(_) = &inst.creation {
                let view = self.injector_view();
                if inst.lazy {
                    let text = lazy_provider_text(inst, &self.asset, Some(&view))?;
                    let text = resolve_deferred(self.imp, &text);
                    self.lazy_fields.push(text);
                    self.lazy_positions.push(e.start);
                    continue;
                }
                let ty = resolve_deferred(self.imp, &provider_type(inst, &self.asset)?);
                // `isDevMode` and `debugInjectorWrap` with deferred imports,
                // in text order.
                let util = deferred(UTILITIES);
                let v = Some(view.as_str());
                let value = match &inst.creation {
                    Creation::Expression(x) => expr_text(x, &inst.token, &self.asset, &util, 4, v),
                    Creation::Multi(items) => format!(
                        "[{}]",
                        items
                            .iter()
                            .map(|x| expr_text(x, &inst.token, &self.asset, &util, 4, v))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    _ => unreachable!(),
                };
                let value = resolve_deferred(self.imp, &value);
                self.child_fields
                    .push(format!("  late final {ty} {};", inst.field));
                self.lines
                    .push(format!("    this.{} = {value};", inst.field));
                continue;
            }
            let ty = match (&inst.creation, &inst.token) {
                (Creation::Directive { directive, .. }, _)
                    if !directive.host_bindings.is_empty() =>
                {
                    let prefix = ngcd_prefix(self.imp, &directive.uri, &self.asset);
                    let args = self.type_arguments(&directive.uri, &directive.class, e);
                    format!("{prefix}{}NgCd{args}", directive.class)
                }
                (Creation::Directive { directive, .. }, _) => {
                    let prefix = self.imp.q(&import_of(&directive.uri, &self.asset));
                    let args = self.type_arguments(&directive.uri, &directive.class, e);
                    format!("{prefix}{}{args}", directive.class)
                }
                (Creation::List(_), Token::Multi { ty, .. }) if ty.e_object() => {
                    format!("List<{}Object>", self.imp.q("dart:core"))
                }
                (Creation::List(_), Token::Multi { ty, .. }) if ty.generics > 0 => {
                    let args = vec!["dynamic"; ty.generics].join(", ");
                    format!(
                        "List<{}{}<{args}>>",
                        self.imp.q(&import_of(&ty.uri, &self.asset)),
                        ty.class
                    )
                }
                _ => {
                    return Err(refusal(
                        Reason::DirectiveBySelector,
                        "provider without a type",
                    ));
                }
            };
            let value = match &inst.creation {
                Creation::Directive { directive, args } => {
                    // With `@HostBinding`, the `XNgCd` of the directive's
                    // `.template.dart` wraps the instance.
                    let cd = if directive.host_bindings.is_empty() {
                        None
                    } else {
                        Some(ngcd_prefix(self.imp, &directive.uri, &self.asset))
                    };
                    // A dependency on the outer injector: the creation is
                    // wrapped in `debugInjectorWrap` under `isDevMode`, and the
                    // emitter writes `isDevMode` and `debugInjectorWrap` before
                    // the class and the tokens (the order of the imports).
                    let injects = args
                        .iter()
                        .any(|a| matches!(a, Argument::FromOutside { .. }));
                    let wrapped =
                        injects.then(|| (self.imp.alias(UTILITIES), self.imp.alias(DI_ERRORS)));
                    let class = format!(
                        "{}{}",
                        self.imp.q(&import_of(&directive.uri, &self.asset)),
                        directive.class
                    );
                    let v = self.injector_view();
                    let mut texts = Vec::new();
                    for a in args {
                        texts.push(match a {
                            Argument::Element => target.to_string(),
                            Argument::Detector => detector.to_string(),
                            Argument::Null => "null".to_string(),
                            Argument::Field(c) => format!("this.{c}"),
                            Argument::Above(read) => read.clone(),
                            Argument::Injector(n) => format!("this.injector({n})"),
                            Argument::Container => {
                                format!("this._appEl_{}", element_index(target))
                            }
                            // The attribute written on the element (`attrs`),
                            // without a binding; when absent, `null`.
                            Argument::Attribute(name) => e
                                .attributes
                                .iter()
                                .find(|a| a.name == *name)
                                .map_or_else(|| "null".to_string(), |a| literal(&a.value)),
                            Argument::FromOutside { token, optional } => {
                                let method = if *optional {
                                    "injectorGetOptional"
                                } else {
                                    "injectorGet"
                                };
                                let t = token_expr(&token_local(token, &self.asset));
                                let t = resolve_deferred(self.imp, &t);
                                format!("({v}.parentView!).{method}({t}, {v}.parentIndex)")
                            }
                        });
                    }
                    let call = format!("{class}({})", texts.join(", "));
                    let creation = match wrapped {
                        None => call,
                        Some((util, errors)) => format!(
                            "({util}.isDevMode\n        ? {errors}.debugInjectorWrap({class}, () {{\n            return {call};\n          }})\n        : {call})"
                        ),
                    };
                    match cd {
                        Some(q) => format!("{q}{}NgCd({creation})", directive.class),
                        None => creation,
                    }
                }
                Creation::List(items) => {
                    let items: Vec<String> = items.iter().map(|c| format!("this.{c}")).collect();
                    format!("[{}]", items.join(", "))
                }
                // Only the host view creates providers like this.
                Creation::Expression(_) | Creation::Multi(_) => {
                    return Err(refusal(
                        Reason::DirectiveBySelector,
                        "provider that is neither a directive nor an alias on the node",
                    ));
                }
            };
            // The reads from above may bring the import of the deferred
            // `unsafeCast` (a child's node): allocated here, where the text is
            // emitted.
            let value = resolve_deferred(self.imp, &value);
            if inst.lazy {
                // `late` without `final`, with the value in the initializer.
                self.lazy_fields
                    .push(format!("  late {ty} {} = {value};", inst.field));
                self.lazy_positions.push(e.start);
            } else {
                self.child_fields
                    .push(format!("  late final {ty} {};", inst.field));
                self.lines
                    .push(format!("    this.{} = {value};", inst.field));
            }
        }
        Ok(())
    }

    /// `registerDirectives`: the instances of the directives, in order.
    /// (Only the component on the node, without a directive: nothing to
    /// register.)
    fn register_directives(
        &mut self,
        directives: &[(std::sync::Arc<crate::directives::Directive>, String)],
        target: &str,
    ) {
        if !directives.is_empty() {
            let dev = self.imp.alias(DEVTOOLS);
            let registrations: Vec<String> = directives
                .iter()
                .map(|(_, c)| {
                    format!("      {dev}.Inspector.instance.registerDirective({target}, this.{c});")
                })
                .collect();
            self.lines.push(format!(
                "    if ({dev}.isDevToolsEnabled) {{\n{}\n    }}",
                registrations.join("\n")
            ));
        }
    }

    /// The bindings of the node's `directives`: the `detectHostChanges` of the
    /// ones that have `@HostBinding` and the inputs (`inputs`), and the outputs
    /// (`outputs`), directive by directive, in the given order
    /// (`transformedDirectiveAsts`).
    #[allow(clippy::too_many_arguments)]
    fn bind_directives(
        &mut self,
        e: &crate::html::Element,
        instances: &[crate::directives::Instance],
        directives: &[(std::sync::Arc<crate::directives::Directive>, String)],
        target: &str,
        properties: &[crate::html::Binding],
        events: &[crate::html::Binding],
        inputs: bool,
        outputs: bool,
    ) -> Result<(), Refusal> {
        use crate::directives::Creation;
        // `detectHostChanges` of the directives with `@HostBinding`, with the
        // node's property bindings (`bindDirectiveHostProps`).
        for inst in instances.iter().filter(|_| inputs) {
            if let Creation::Directive { directive, .. } = &inst.creation
                && !directive.host_bindings.is_empty()
                && directives.iter().any(|(_, c)| *c == inst.read)
            {
                let view = self.host_view.as_deref().unwrap_or("this");
                self.detection.push(format!(
                    "    this.{}.detectHostChanges({view}, {target});",
                    inst.field
                ));
            }
        }
        // Inputs and outputs, directive by directive, in provider order
        // (`transformedDirectiveAsts`).
        for (d, field) in directives {
            if !inputs {
                self.directive_outputs(d, field, events)?;
                continue;
            }
            let mut bound: Vec<(&crate::html::Binding, bool)> = e
                .attributes
                .iter()
                .filter(|a| d.input(&a.name).is_some())
                .map(|a| (a, true))
                .collect();
            bound.extend(
                properties
                    .iter()
                    .filter(|l| d.input(&l.name).is_some())
                    .map(|l| (l, false)),
            );
            let spec: Vec<(String, String, Option<bool>)> = d
                .inputs
                .iter()
                .map(|x| (x.name.clone(), x.member.clone(), x.boolean))
                .collect();
            // Only `AfterChanges`, `OnInit` and `DoCheck` get here (`OnDestroy`
            // is emitted in `destroyInternal`): the other hooks are refused by
            // the guard (`Directive::blocker`).
            let hooks = crate::component::Hooks {
                after_changes: d.hooks.after_changes,
                on_init: d.hooks.on_init,
                do_check: d.hooks.do_check,
                ..Default::default()
            };
            self.inputs_of(
                &bound,
                &spec,
                field,
                None,
                &hooks,
                Reason::DirectiveBySelector,
            )?;
            if outputs {
                self.directive_outputs(d, field, events)?;
            }
        }
        Ok(())
    }

    /// The `@Output`s of a directive of the node bound in the template
    /// (`bindDirectiveOutputs`).
    fn directive_outputs(
        &mut self,
        d: &crate::directives::Directive,
        field: &str,
        events: &[crate::html::Binding],
    ) -> Result<(), Refusal> {
        {
            let mut seen: Vec<&str> = Vec::new();
            for l in events.iter().filter(|l| d.output(&l.name).is_some()) {
                if seen.contains(&l.name.as_str()) {
                    return Err(refusal(
                        Reason::Event,
                        "two handlers of the same event in the template",
                    ));
                }
                seen.push(&l.name);
                let member = d.output(&l.name).unwrap_or_default();
                self.set_output_type(&d.uri, &d.class, member);
                match self.handler(&[&l.value]) {
                    Ok(h) => {
                        let k = self.subscriptions;
                        self.subscriptions += 1;
                        self.listeners.push(format!(
                            "    final subscription_{k} = this.{field}.{member}.listen({h});"
                        ));
                    }
                    Err(r) => self.record_refusal(r)?,
                }
            }
        }
        Ok(())
    }
}

/// `detectChangesInternal` and `destroyInternal` of the host view, in the exact
/// official shape: the content hooks before detecting the view, the view hooks
/// after, all under `!debugThrowIfChanged`, and `firstCheck` declared only when
/// someone uses it.
///
/// An `onPush` component with `@Input` gets, before the hooks, the `if
/// (changed) markAsCheckOnce()` of `bindDirectiveInputs` — which in the host
/// view never sets `changed`, but declares it. The `check_binding` import only
/// goes in if the text uses it (`deferred`).
///
/// With `@HostBinding` (`host`), the `detectHostChanges(firstCheck)` of
/// the component's view comes right before its `detectChanges`, after the
/// content hooks (case j32).
fn lifecycle(g: &crate::component::Hooks, mark: bool, host: bool, nested: bool) -> String {
    let dbg = deferred(CHECK_BINDING);
    let mut s = String::new();
    if g.has_detection() || mark || host || nested {
        s.push_str(
            "
  @override
  void detectChangesInternal() {
",
        );
        if mark {
            s.push_str("    bool changed = false;\n");
        }
        if g.uses_first_check() || host {
            s.push_str(
                "    bool firstCheck = this.firstCheck;
",
            );
        }
        if mark {
            s.push_str("    if (changed) {\n      this.componentView.markAsCheckOnce();\n    }\n");
        }
        if g.on_init {
            s.push_str(&format!(
                "    if (((!{dbg}.debugThrowIfChanged) && firstCheck)) {{
      this.component.ngOnInit();
    }}
"
            ));
        }
        if g.do_check {
            s.push_str(&format!(
                "    if ((!{dbg}.debugThrowIfChanged)) {{
      this.component.ngDoCheck();
    }}
"
            ));
        }
        // The `ViewContainer` of the host element (a component that injects
        // `ViewContainerRef`): after `ngOnInit`/`ngDoCheck`, before the content
        // hooks (case j47).
        if nested {
            s.push_str("    this._appEl_0.detectChangesInNestedViews();\n");
        }
        if g.after_content_init || g.after_content_checked {
            s.push_str(&format!(
                "    if ((!{dbg}.debugThrowIfChanged)) {{
"
            ));
            if g.after_content_init {
                s.push_str(
                    "      if (firstCheck) {
        this.component.ngAfterContentInit();
      }
",
                );
            }
            if g.after_content_checked {
                s.push_str(
                    "      this.component.ngAfterContentChecked();
",
                );
            }
            s.push_str(
                "    }
",
            );
        }
        if host {
            s.push_str("    this.componentView.detectHostChanges(firstCheck);\n");
        }
        s.push_str(
            "    this.componentView.detectChanges();
",
        );
        if g.after_view_init || g.after_view_checked {
            s.push_str(&format!(
                "    if ((!{dbg}.debugThrowIfChanged)) {{
"
            ));
            if g.after_view_init {
                s.push_str(
                    "      if (firstCheck) {
        this.component.ngAfterViewInit();
      }
",
                );
            }
            if g.after_view_checked {
                s.push_str(
                    "      this.component.ngAfterViewChecked();
",
                );
            }
            s.push_str(
                "    }
",
            );
        }
        s.push_str(
            "  }
",
        );
    }
    if g.on_destroy || nested {
        s.push_str("\n  @override\n  void destroyInternal() {\n");
        if nested {
            s.push_str("    this._appEl_0.destroyNestedViews();\n");
        }
        if g.on_destroy {
            s.push_str("    this.component.ngOnDestroy();\n");
        }
        s.push_str("  }\n");
    }
    s
}

/// Types that ngcompiler treats as primitive in interpolation (`isBool`,
/// `isNumber`, `isDouble`, `isInt`).
fn primitive(ty: &str) -> bool {
    matches!(ty, "bool" | "num" | "double" | "int")
}

/// Does some element have a property binding (or a `#ref` read as a local, from
/// `refs`) and therefore become a field?
fn has_bound_element(
    nodes: &[Node],
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
    refs: &std::collections::HashSet<String>,
) -> bool {
    nodes.iter().any(|n| match n {
        // The subtree of a `*` belongs to the embedded view.
        Node::Element(e) if e.star.is_some() => false,
        Node::Element(e) => match children.get(&e.name) {
            None => {
                binds_on_element(e, &matched_directives(used, e))
                    || e.references.iter().any(|r| refs.contains(&r.name))
                    || has_bound_element(&e.children, children, used, refs)
            }
            // A child's node becomes a field with the element's own binding
            // ([`ViewBody::child`]).
            Some(f) => {
                bound_child_element(e, f, &matched_directives(used, e))
                    || has_bound_element(&e.children, children, used, refs)
            }
        },
        _ => false,
    })
}

/// The element of a child component becomes a field of the view: a binding of
/// the element itself (`[class.x]`, `[style.x]`, `[attr.x]`, `[class]`) or an
/// interpolated attribute that neither the child nor a directive of the node
/// receives, or a directive of the node with `@HostBinding` (the
/// `detectHostChanges` reads the node).
fn bound_child_element(
    e: &crate::html::Element,
    child: &Child,
    extras: &[std::sync::Arc<crate::directives::Directive>],
) -> bool {
    e.properties
        .iter()
        .any(|l| child.input(&l.name).is_none() && !consumes_input(extras, &l.name))
        || e.attributes.iter().any(|a| {
            a.value.contains("{{")
                && child.input(&a.name).is_none()
                && !consumes_input(extras, &a.name)
        })
        || extras.iter().any(|d| !d.host_bindings.is_empty())
}

/// The directives that match an HTML element, with the metadata read from the
/// program, in `directives:` order (`_matchDirectives`). The element guard has
/// already refused those the emitter does not instantiate.
fn matched_directives(
    used: &[UsedDirective],
    e: &crate::html::Element,
) -> Vec<std::sync::Arc<crate::directives::Directive>> {
    if used.is_empty() {
        return Vec::new();
    }
    let desc = crate::selector::Element::of_template(e);
    used.iter()
        .filter(|u| u.child.is_none() && crate::selector::matches_any(&u.selectors, &desc))
        .filter_map(|u| u.directive.clone())
        .collect()
}

/// The tokens that the content of the child's node asks of its providers, eager
/// and in visiting order ([`content_requests`]): the resolver transforms
/// them right after the node's eager ones, and the field numbering comes out in
/// the right order (case i76; the `PopupRef_0_9` of `paper_tooltip`).
fn requests_to_child_node(
    e: &crate::html::Element,
    matched: &[std::sync::Arc<crate::directives::Directive>],
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
) -> Result<Vec<(crate::directives::Token, bool)>, Refusal> {
    let tokens: Vec<crate::directives::Token> = matched
        .iter()
        .flat_map(|d| d.providers.iter().map(|p| p.token.clone()))
        .collect();
    let mut requests = Vec::new();
    content_requests(&e.children, children, used, &tokens, true, &mut requests)?;
    Ok(requests)
}

/// The fields of the node's providers and their creation in `build()`, in the
/// given order (`addDirectiveProviders`).
/// The tokens of `tokens` that the content `nodes` asks of the node above
/// (`_getDependency`: the node below looks in itself first, except with
/// `@SkipSelf`, and walks up; with `@Self` it does not walk up), in the order
/// of the requests: in pre-order, at each node the child and then the
/// directives. A `*` on the way makes the request lazy (`_isViewRoot`), and a
/// node that provides the token serves those below. A node with requests for
/// more than one token, whose order depends on its resolution, is refused.
fn content_requests(
    nodes: &[Node],
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
    tokens: &[crate::directives::Token],
    eager: bool,
    output: &mut Vec<(crate::directives::Token, bool)>,
) -> Result<(), Refusal> {
    use crate::directives::Token;
    for n in nodes {
        let Node::Element(e) = n else { continue };
        // The `*` takes the node to the embedded view: the request that comes
        // out of it crosses its root (`_isViewRoot`) and is lazy. A written
        // `<template>` stays here; only the content crosses.
        if let Some(star) = &e.star {
            let mut without = e.clone();
            without.star = None;
            if star.name == TEMPLATE_MARK {
                // The directives of the `<template>` ask from here; the
                // content, from inside the embedded view, with what they
                // provide already served.
                let mut only_the_node = without.clone();
                only_the_node.children.clear();
                content_requests(
                    std::slice::from_ref(&Node::Element(only_the_node)),
                    children,
                    used,
                    tokens,
                    eager,
                    output,
                )?;
                let own: Vec<Token> = matched_directives(used, &without)
                    .iter()
                    .flat_map(|d| {
                        std::iter::once(d.token())
                            .chain(d.providers.iter().map(|p| p.token.clone()))
                    })
                    .collect();
                let below: Vec<Token> = tokens
                    .iter()
                    .filter(|t| !own.contains(t))
                    .cloned()
                    .collect();
                content_requests(&e.children, children, used, &below, false, output)?;
            } else {
                content_requests(
                    std::slice::from_ref(&Node::Element(without)),
                    children,
                    used,
                    tokens,
                    false,
                    output,
                )?;
            }
            continue;
        }
        let child = children.get(&e.name);
        let matched = matched_directives(used, e);
        // What the node itself provides.
        let mut own: Vec<Token> = Vec::new();
        if let Some(m) = child.and_then(|f| f.metadata.as_ref()) {
            own.push(m.token());
            own.extend(m.providers.iter().map(|p| p.token.clone()));
        }
        for d in &matched {
            own.push(d.token());
            own.extend(d.providers.iter().map(|p| p.token.clone()));
        }
        let mut deps: Vec<(Token, bool, bool)> = Vec::new();
        if let Some(f) = child {
            for p in &f.params {
                if let Injected::Service {
                    token,
                    self_only,
                    skip_self,
                    ..
                } = p
                {
                    deps.push((token.clone(), *self_only, *skip_self));
                }
            }
        }
        for d in &matched {
            for x in &d.dependencies {
                deps.push((x.token.clone(), x.self_only, x.skip_self));
            }
        }
        let mut this_node: Vec<Token> = Vec::new();
        for (t, self_only, skip_self) in deps {
            if self_only || !tokens.contains(&t) || (!skip_self && own.contains(&t)) {
                continue;
            }
            if !this_node.contains(&t) {
                this_node.push(t);
            }
        }
        let ja = |t: &Token, output: &Vec<(Token, bool)>| output.iter().any(|(x, _)| x == t);
        // Several tokens: the order is that of the creation of this node's
        // providers (each one asks for its dependencies in order,
        // `_getOrCreateLocalProvider`).
        if this_node.iter().filter(|t| !ja(t, output)).count() > 1 {
            let Some(order) = node_request_order(e, child, &matched, used) else {
                return Err(refusal(
                    Reason::Binding,
                    "content node requesting several lazy providers of the child",
                ));
            };
            let mut sorted: Vec<Token> = order
                .into_iter()
                .filter(|t| this_node.contains(t))
                .collect();
            sorted.dedup();
            if sorted.len() != this_node.len() {
                return Err(refusal(
                    Reason::Binding,
                    "content node requesting several lazy providers of the child",
                ));
            }
            this_node = sorted;
        }
        for t in this_node {
            if !ja(&t, output) {
                output.push((t, eager));
            }
        }
        let below: Vec<Token> = tokens
            .iter()
            .filter(|t| !own.contains(t))
            .cloned()
            .collect();
        content_requests(&e.children, children, used, &below, eager, output)?;
    }
    Ok(())
}

/// The tokens that the providers of a content node ask for, in the order its
/// `ProviderElementContext` creates them (each eager directive after its
/// dependencies, and its dependencies in order). `None` when the node does not
/// resolve or has a provider that is not a directive.
fn node_request_order(
    e: &crate::html::Element,
    child: Option<&Child>,
    matched: &[std::sync::Arc<crate::directives::Directive>],
    used: &[UsedDirective],
) -> Option<Vec<crate::directives::Token>> {
    use crate::directives::Creation;
    let _ = e;
    let r = match child {
        Some(f) => {
            let meta = f.metadata.as_ref()?;
            let (index, cas) = child_node_matched(meta, matched, used);
            let container = f.params.iter().any(|p| matches!(p, Injected::Container))
                || matched
                    .iter()
                    .any(|d| crate::directives::requests_container(d));
            crate::directives::resolve_child_node(&cas, index, 0, None, container, &[]).ok()?
        }
        None => crate::directives::resolver(matched, 0, None).ok()?,
    };
    // The component enters the resolution only with the dependencies of the
    // node itself ([`child_node_matched`]); here all of them count, in
    // constructor order.
    let component = child.and_then(|f| f.metadata.clone());
    let mut output = Vec::new();
    for i in &r.instances {
        match &i.creation {
            Creation::Directive { directive, .. } => {
                let deps = match &component {
                    Some(m) if m.uri == directive.uri && m.class == directive.class => {
                        &m.dependencies
                    }
                    _ => &directive.dependencies,
                };
                output.extend(deps.iter().map(|d| d.token.clone()));
            }
            // The lazy one only asks in `afterElement`, after the content.
            _ if i.lazy => {}
            _ => return None,
        }
    }
    Some(output)
}

/// Does some `@Input` of the directives have this name?
fn consumes_input(matched: &[std::sync::Arc<crate::directives::Directive>], name: &str) -> bool {
    matched.iter().any(|d| d.input(name).is_some())
}

/// Does some `@Output` of the directives have this name?
fn consumes_output(matched: &[std::sync::Arc<crate::directives::Directive>], name: &str) -> bool {
    matched.iter().any(|d| d.output(name).is_some())
}

/// The node has a binding of its own — `[x]` that no directive receives, or an
/// attribute with `{{ }}` —, and becomes a field of the view.
fn binds_on_element(
    e: &crate::html::Element,
    matched: &[std::sync::Arc<crate::directives::Directive>],
) -> bool {
    e.properties
        .iter()
        .any(|p| !consumes_input(matched, &p.name))
        || e
            .attributes
            .iter()
            .any(|a| a.value.contains("{{") && !consumes_input(matched, &a.name))
        // `detectHostChanges(this, el)` reads the node during detection.
        || matched.iter().any(|d| !d.host_bindings.is_empty())
}

/// An attribute written without a value (`<input required>`): its range is just
/// the name.
fn without_value(a: &crate::html::Binding) -> bool {
    a.without_value
}

/// The `injectorGetInternal` of a view (`writeInjectorGetMethod`,
/// `ProviderForest.build`): the nodes with an injectable provider, each with
/// the index range it serves; empty if there are none.
/// A node with injectable providers: (first index, last index of the subtree,
/// [(tokens, field)]).
type InjectorNode = (u32, u32, Vec<(Vec<crate::directives::Token>, String)>);

fn injector_method(injectors: &[InjectorNode], asset: &str) -> String {
    use crate::directives::Token;
    if injectors.is_empty() {
        return String::new();
    }
    // The URIs of the tokens become the import path seen from this file.
    let converted = |t: &Token| match t {
        Token::Class { uri, class } => Token::Class {
            uri: import_of(uri, asset),
            class: class.clone(),
        },
        Token::Multi { name, ty } => Token::Multi {
            name: name.clone(),
            ty: crate::directives::TokenType {
                uri: import_of(&ty.uri, asset),
                ..ty.clone()
            },
        },
        Token::Opaque { name, ty } => Token::Opaque {
            name: name.clone(),
            ty: crate::directives::TokenType {
                uri: import_of(&ty.uri, asset),
                ..ty.clone()
            },
        },
        other => other.clone(),
    };
    let converted: Vec<InjectorNode> = injectors
        .iter()
        .map(|(a, b, ps)| {
            let ps = ps
                .iter()
                .map(|(ts, c)| (ts.iter().map(converted).collect(), c.clone()))
                .collect();
            (*a, *b, ps)
        })
        .collect();
    let injectors = &converted[..];
    // The forest: each node is a child of the closest one before it that
    // contains it.
    let mut pais: Vec<Option<usize>> = Vec::new();
    for (i, (begin, end, _)) in injectors.iter().enumerate() {
        let mut p = if i == 0 { None } else { Some(i - 1) };
        while let Some(j) = p {
            if injectors[j].0 <= *begin && *end <= injectors[j].1 {
                break;
            }
            p = pais[j];
        }
        pais.push(p);
    }
    fn tokens(ts: &[Token]) -> String {
        let parts: Vec<String> = ts
            .iter()
            .map(|t| format!("identical(token, {})", token_expr(t)))
            .collect();
        // `||` associates to the left: `((a || b) || c)`.
        let mut it = parts.into_iter();
        let first = it.next().unwrap_or_default();
        it.fold(first, |acc, t| format!("({acc} || {t})"))
    }
    fn index(begin: u32, end: u32, below: i64, alto: i64) -> String {
        if begin == end {
            format!("({begin} == nodeIndex)")
        } else if i64::from(begin) == below {
            format!("(nodeIndex <= {end})")
        } else if i64::from(end) == alto {
            format!("({begin} <= nodeIndex)")
        } else {
            format!("(({begin} <= nodeIndex) && (nodeIndex <= {end}))")
        }
    }
    fn level(
        injectors: &[InjectorNode],
        pais: &[Option<usize>],
        parent: Option<usize>,
        below: i64,
        alto: i64,
        output: &mut Vec<String>,
    ) {
        for (i, (begin, end, providers)) in injectors.iter().enumerate() {
            if pais[i] != parent {
                continue;
            }
            let cond = index(*begin, *end, below, alto);
            let has_children = pais.contains(&Some(i));
            if !has_children && providers.len() == 1 {
                let (ts, field) = &providers[0];
                output.push(format!(
                    "if (({} && {cond})) {{\n  return this.{field};\n}}",
                    tokens(ts)
                ));
            } else {
                let mut inside = Vec::new();
                level(
                    injectors,
                    pais,
                    Some(i),
                    i64::from(*begin),
                    i64::from(*end),
                    &mut inside,
                );
                for (ts, field) in providers {
                    inside.push(format!(
                        "if ({}) {{\n  return this.{field};\n}}",
                        tokens(ts)
                    ));
                }
                output.push(format!(
                    "if ({cond}) {{\n{}\n}}",
                    indent_lines(&inside.join("\n"), 2)
                ));
            }
        }
    }
    let mut body = Vec::new();
    level(injectors, &pais, None, 0, -1, &mut body);
    format!(
        "\n  @override\n  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {{\n{}\n    return notFoundResult;\n  }}\n",
        indent_lines(&body.join("\n"), 4)
    )
}

/// The expression of a token (`createDiTokenExpression`), with the URI already
/// seen from this file and the deferred import: the class, or the `const
/// MultiToken<T>('x')` / `const OpaqueToken<T>('x')`. A `T` from `dart:core`
/// comes out without a prefix; one from another library gets its own import
/// (see [`Imports::q_keyed`]).
fn token_expr(t: &crate::directives::Token) -> String {
    use crate::directives::Token;
    let (token_class, name, ty) = match t {
        Token::Class { uri, class } => return format!("{}{class}", deferred_q(uri)),
        Token::Multi { name, ty } => ("MultiToken", name, ty),
        Token::Opaque { name, ty } => ("OpaqueToken", name, ty),
        Token::Element | Token::Detector => return String::new(),
    };
    let arg = token_type_text(ty);
    format!(
        "const {}{token_class}<{arg}>({})",
        deferred_q(crate::directives::DI_TOKENS),
        literal(name)
    )
}

/// The `T` of a token as `fromDartType` writes it: one from `dart:core` without
/// a prefix (but with the import allocated), one from another library with its
/// own import by URI (`q_keyed`), and the concrete arguments recursively
/// (`List<import29.RelativePosition>`, by the `package:` URI of the type).
fn token_type_text(ty: &crate::directives::TokenType) -> String {
    if ty.is_dynamic() {
        return "dynamic".into();
    }
    let args = if !ty.args.is_empty() {
        let inside: Vec<String> = ty.args.iter().map(token_type_text).collect();
        format!("<{}>", inside.join(", "))
    } else if ty.generics > 0 {
        format!("<{}>", vec!["dynamic"; ty.generics].join(", "))
    } else {
        String::new()
    };
    let (uri, class) = (&ty.uri, &ty.class);
    if uri == "dart:core" {
        format!("{}{class}{args}", deferred_q("dart:core"))
    } else {
        format!("\u{1}k:{uri}#token|{uri}\u{2}{class}{args}")
    }
}

/// What generates a field in the view class, in order of appearance.
enum ViewField<'a> {
    /// The child, if its node has a `ViewContainer`, and the fields of the
    /// node's providers created before it ([`field_uri`]).
    Child(&'a Child, bool, Vec<String>),
    /// The `ViewContainer` of an ordinary element (a directive that injects
    /// `ViewContainerRef`), before the directives.
    Container,
    /// Structural directive, with the URI of its class.
    Structural(&'static str),
    /// A written `<template>`: the `ViewContainer`, with `#ref` the field of
    /// the `TemplateRef`, and the fields of its directives (the text with the
    /// deferred import, [`field_uri`]), in order.
    Template(bool, Vec<String>),
    /// Directive providers on a node: the text with the deferred imports of
    /// each one's field ([`field_uri`]), in order.
    Directives(Vec<String>),
    /// Lazy providers (a field with an initializer): they come before the other
    /// fields in the class, and so do their imports.
    Lazy(Vec<String>),
}

/// The fields the template will generate, in document order. The order of the
/// imports follows this list, and it is what makes the numbering match the
/// official one.
fn fields_in_order<'a>(
    nodes: &[Node],
    children: &'a std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
    asset: &str,
) -> Vec<ViewField<'a>> {
    let mut output = Vec::new();
    let split = |instances: &[crate::directives::Instance], output: &mut Vec<ViewField>| {
        let uris = instances
            .iter()
            .filter(|i| !i.lazy)
            .map(|i| field_uri(i, asset))
            .collect();
        let lazy = instances
            .iter()
            .filter(|i| i.lazy)
            .map(|i| field_uri(i, asset))
            .collect();
        output.push(ViewField::Directives(uris));
        output.push(ViewField::Lazy(lazy));
    };
    for node in nodes {
        let Node::Element(e) = node else { continue };
        if let Some(star) = &e.star {
            // The content goes to the embedded view; its fields belong there.
            if star.name == TEMPLATE_MARK {
                let mut without = e.clone();
                without.star = None;
                let matched = matched_directives(used, &without);
                let fields = if matched.is_empty() {
                    Vec::new()
                } else {
                    crate::directives::resolve_template(&matched, 0, None, false, &|_| {
                        String::new()
                    })
                    .map(|(r, _)| r.instances.iter().map(|i| field_uri(i, asset)).collect())
                    .unwrap_or_default()
                };
                output.push(ViewField::Template(!e.references.is_empty(), fields));
            } else if let Some(d) = Structural::known(&star.name) {
                output.push(ViewField::Structural(d.uri));
            }
            continue;
        }
        if let Some(f) = children.get(&e.name) {
            // The other providers of the child's node, after the instance.
            let extras = matched_directives(used, e);
            let container = f.params.iter().any(|p| matches!(p, Injected::Container))
                || extras
                    .iter()
                    .any(|d| crate::directives::requests_container(d));
            let resolved = f
                .metadata
                .as_ref()
                .filter(|meta| !extras.is_empty() || !meta.providers.is_empty())
                .and_then(|meta| {
                    let (index, matched) = child_node_matched(meta, &extras, used);
                    let requests = requests_to_child_node(e, &matched, children, used).ok()?;
                    let r = crate::directives::resolve_child_node(
                        &matched, index, 0, None, container, &requests,
                    )
                    .ok()?;
                    let token = matched[index].token();
                    let pos = r.instances.iter().position(|i| i.token == token)?;
                    Some((r, pos))
                });
            match resolved {
                Some((r, pos)) => {
                    let before = r.instances[..pos]
                        .iter()
                        .map(|i| field_uri(i, asset))
                        .collect();
                    output.push(ViewField::Child(f, container, before));
                    // The lazy ones the content asks for already come eager and
                    // in the right position (case i76).
                    split(&r.instances[pos + 1..], &mut output);
                }
                None => output.push(ViewField::Child(f, container, Vec::new())),
            }
        } else {
            let matched = matched_directives(used, e);
            if let (false, Ok(r)) = (
                matched.is_empty(),
                crate::directives::resolver(&matched, 0, None),
            ) {
                if r.container {
                    output.push(ViewField::Container);
                }
                split(&r.instances, &mut output);
            }
        }
        output.extend(fields_in_order(&e.children, children, used, asset));
    }
    output
}

/// The directives of a child component's node, for the resolver: the child with
/// its `providers:` and only the dependencies the node itself provides (the
/// rest is resolved separately, in [`ViewBody::child_construction`]) — they
/// decide the order of the fields, because the official
/// `_getOrCreateLocalProvider` creates the dependencies of an eager provider
/// before it (case j61) —, and the node's directives. The order is that of
/// `directives:`, with the child in its place (`_matchDirectives`); it comes
/// back with its index.
fn child_node_matched(
    meta: &crate::directives::Directive,
    extras: &[std::sync::Arc<crate::directives::Directive>],
    used: &[UsedDirective],
) -> (usize, Vec<std::sync::Arc<crate::directives::Directive>>) {
    let provided: Vec<crate::directives::Token> = std::iter::once(meta)
        .chain(extras.iter().map(|d| &**d))
        .flat_map(|d| d.providers.iter().map(|p| p.token.clone()))
        .chain(extras.iter().map(|d| d.token()))
        .collect();
    let mut only_providers = meta.clone();
    only_providers
        .dependencies
        .retain(|d| !d.skip_self && d.attribute.is_none() && provided.contains(&d.token));
    let position =
        |class: &str, uri: &str| used.iter().position(|u| u.class == class && u.uri == uri);
    let of_child = position(&meta.class, &meta.uri);
    let index = extras
        .iter()
        .filter(|d| position(&d.class, &d.uri) < of_child)
        .count();
    let mut matched: Vec<_> = extras.to_vec();
    matched.insert(index, std::sync::Arc::new(only_providers));
    (index, matched)
}

/// The field of a node provider, with the deferred imports in the order the
/// official compiler writes them: the type and, for a lazy one from
/// `providers:`, the value.
fn field_uri(i: &crate::directives::Instance, asset: &str) -> String {
    use crate::directives::{Creation, Token};
    let uri = match (&i.creation, &i.token) {
        // The lazy one has the value in the field; the eager one, only the type
        // (the value comes out in `build()`).
        (Creation::Expression(_) | Creation::Multi(_), _) => {
            let text = if i.lazy {
                lazy_provider_text(i, asset, Some("this"))
            } else {
                Err(refusal(Reason::Providers, ""))
            };
            return text
                .or_else(|_| provider_type(i, asset))
                .unwrap_or_default();
        }
        (Creation::Directive { directive, .. }, _)
            if !directive.host_bindings.is_empty() && !directive.is_component =>
        {
            let tpl = import_of(&directive.uri.replace(".dart", ".template.dart"), asset);
            // An `XNgCd` declared in this same file has no import (j45).
            if is_own_template(asset, &tpl) {
                return String::new();
            }
            return deferred(&tpl);
        }
        (Creation::Directive { directive, .. }, _) => directive.uri.clone(),
        (_, Token::Multi { ty, .. }) => ty.uri.clone(),
        _ => "dart:core".to_string(),
    };
    deferred(&import_of(&uri, asset))
}

/// What is needed to emit an embedded view, kept during the scan and used
/// later.
struct EmbeddedSpec {
    /// Number of the view (`_ViewX3`), assigned during the scan.
    index: u32,
    /// The start of the star that creates it (key of
    /// [`Context::dirty_content_queries`]).
    star: Option<usize>,
    /// O namespace herdado ([`ViewBody::current_ns`]).
    ns: Option<String>,
    /// How many `parentView`s up to the component's view.
    depth: u32,
    /// See [`ViewBody::top_level`] (in `EmbeddedSpec`).
    top_level: u32,
    class: String,
    factory: String,
    nodes: Vec<Node>,
    locals: std::collections::HashMap<String, crate::expr::Local>,
    micro: crate::micro::Micro,
    /// The locals declared in ancestor views, with where they come from.
    ancestors: std::collections::HashMap<String, Origin>,
    /// The `#ref`s read as locals declared in ancestor views: (name, class of
    /// the view that declares it, how many `parentView`s up to it).
    ancestor_refs: Vec<(String, String, u32)>,
    /// The view query results in this view: (`#ref`, dirty field, how many
    /// `parentView`s up to the component's view).
    queried_refs: Vec<(String, String, u32)>,
    /// The same, in the order of the lines of `dirtyParentQueriesInternal`
    /// (that of the first result of each one, [`first_result`]).
    dirty: Vec<(String, String, u32)>,
    /// As consultas com resultado mais abaixo ([`ViewBody::queries_in_transit`]).
    queries_in_transit: Vec<(String, String, u32)>,
    /// The providers above the anchor, seen from the new view.
    above: Vec<(crate::directives::Token, String, Option<(String, u32)>)>,
    /// The fields of `above` that are lazy providers of the owning element
    /// (field, class of its view): a node below that asks for them would
    /// transform them before the owner's `afterElement` (gap L1 of section 05).
    lazy_above: std::collections::HashSet<(String, String)>,
    /// Those of them whose owner modeled the content's requests (a child's
    /// node): from another view, read through the lazy field.
    lazy_with_requests: std::collections::HashSet<(String, String)>,
    components_above: u32,
    uncertain_above: u32,
    /// The index of the embedded view's first `<ng-content>` in the template.
    next_projection: u32,
}

/// Where a local of an ancestor view comes from: the key in `locals`, the class
/// of the view that declares it and how many `parentView`s separate the two
/// (`getPropertyInView`, in `view_compiler_utils.dart`).
#[derive(Debug, Clone)]
struct Origin {
    key: String,
    class: String,
    levels: u32,
}

/// The names of the component's `exports:`: the qualifier (deferred import) and
/// what each one designates.
pub(crate) type Exports = std::collections::HashMap<String, (String, crate::resolucao::Exported)>;

/// What every view of the file shares: the component, the directives it uses
/// and where the file lives.
/// A `Typed` of `directiveTypes:` with the class resolved and the arguments
/// already written (`<T>`, `<import3.X>`).
#[derive(Debug, Clone)]
struct ResolvedDirectiveType {
    uri: String,
    class: String,
    arguments: String,
    em: Option<String>,
}

struct Context<'a> {
    members: &'a std::collections::HashMap<String, crate::component::Member>,
    /// The resolved `exports:` names ([`Exports`]).
    exports: &'a Exports,
    /// The component's type parameters (`T`, `U`), free in the views.
    generic_names: Vec<String>,
    /// The tokens of the component's `viewProviders:`
    /// (`ProviderViewContext.viewProviders`): an `@Host()` of its view that
    /// asks for one of them goes to the injector instead of being `null`.
    view_tokens: Vec<crate::directives::Token>,
    methods: &'a std::collections::HashMap<String, String>,
    arities: &'a std::collections::HashMap<String, usize>,
    children: &'a std::collections::HashMap<String, Child>,
    used: &'a [UsedDirective],
    asset: String,
    types: Option<(&'a dyn Resolution, &'a Path)>,
    qualified_class: String,
    with_style: bool,
    template_url: Option<String>,
    view_class: String,
    context_type: String,
    /// The type arguments of a generic component (`<T, U>`) and their
    /// declaration (`<T extends num, U>`); empty for the rest.
    generics: String,
    generics_decl: String,
    preserve_whitespace: bool,
    pipes: &'a TemplatePipes,
    /// The `#ref` names that may become locals ([`candidate_refs`]).
    candidate_refs: std::collections::HashSet<String>,
    /// (key, token) of the queries with `read:` of a node provider
    /// ([`read_token`]).
    reads: Vec<(String, crate::directives::Token)>,
    /// The `#ref`s that only queries with a provider `read:` look for: the node
    /// does not need to become a field.
    provider_only_refs: std::collections::HashSet<String>,
    /// The `<template #x>`s that a query reads as `ViewContainerRef`: the
    /// node's `ViewContainer` stops being private ([`reads_container`]).
    templates_with_container: std::collections::HashSet<String>,
    /// `@changeDetectionLink` no componente.
    detection_link: bool,
    /// `directiveTypes:` resolvido ([`ResolvedDirectiveType`]).
    directive_types: Vec<ResolvedDirectiveType>,
    /// The `#ref` names that are repeated or shadowed by `let`.
    ambiguous_refs: std::collections::HashSet<String>,
    /// The node of each `#ref` seen, of all views already walked: a nested view
    /// is emitted after the one that contains it, and reads the field from it.
    resolved_refs: std::cell::RefCell<std::collections::HashMap<String, String>>,
    /// The chain of each dynamic view query (by the dirty field): the
    /// `_appEl_n` anchor and the class of the embedded view at each level, from
    /// the component's down. The lower levels are only known when the nested
    /// views are emitted.
    query_anchors: std::cell::RefCell<Anchors>,
    /// The content queries with a result in a `*`, assembled at the end
    /// ([`resolve_dynamic_content`]).
    dynamic_content: std::cell::RefCell<Vec<DynamicContent>>,
    /// Per embedded view (the start of its star), the content queries it marks
    /// in `dirtyParentQueriesInternal`.
    dirty_content_queries: std::cell::RefCell<DirtyContentQueries>,
}

impl<'a> Context<'a> {
    /// An empty body for a view of this file.
    fn body<'b>(
        &'b self,
        imp: &'b mut Imports,
        names: &'b mut dartforge_intern::Interner,
        collected: Option<Vec<Refusal>>,
        embedded: bool,
    ) -> ViewBody<'b>
    where
        'a: 'b,
    {
        ViewBody {
            lines: Vec::new(),
            listeners: Vec::new(),
            free_refs: Default::default(),
            local_refs: Default::default(),
            ambiguous_refs: self.ambiguous_refs.clone(),
            reads: &self.reads,
            provider_only_refs: &self.provider_only_refs,
            templates_with_container: &self.templates_with_container,
            directive_types: &self.directive_types,
            ancestor_refs: Vec::new(),
            dynamic_queries: Vec::new(),
            queries_in_transit: Vec::new(),
            query_anchors: &self.query_anchors,
            dynamic_content: &self.dynamic_content,
            dirty_content_queries: &self.dirty_content_queries,
            content_updates: Vec::new(),
            view_updates: Vec::new(),
            token_queries: Default::default(),
            content_dirty_fields: Vec::new(),
            queries_per_node: Default::default(),
            queried_refs: Vec::new(),
            refs: Default::default(),
            refs_in_order: Vec::new(),
            fields: Vec::new(),
            intl: None,
            messages: Vec::new(),
            current_tag: String::new(),
            current_ns: None,
            view_instance: None,
            host_view: None,
            lazy_fields: Vec::new(),
            lazy_positions: Vec::new(),
            view_dirty_fields: Vec::new(),
            child_fields: Vec::new(),
            child_views: Vec::new(),
            linked_views: Vec::new(),
            expr_fields: Vec::new(),
            el_fields: Vec::new(),
            el_field_refs: Default::default(),
            event_only_refs: Default::default(),
            event_el_fields: Vec::new(),
            el_fields_by_event: Vec::new(),
            queried_el_fields: Vec::new(),
            embedded_el_fields: Vec::new(),
            embedded_refs: Default::default(),
            embedded_position: Default::default(),
            next_binding: 0,
            detection: Vec::new(),
            names,
            uses_first_check: false,
            inputs: Vec::new(),
            tb: None,
            tb_used: false,
            template_url: self.template_url.clone(),
            members: self.members,
            exports: self.exports,
            generic_names: &self.generic_names,
            view_tokens: &self.view_tokens,
            methods: self.methods,
            arities: self.arities,
            event_methods: Vec::new(),
            event_type: None,
            event_type_failure: None,
            i18n_methods: Vec::new(),
            local_decls: Default::default(),
            root_locals: Vec::new(),
            this_class: String::new(),
            own_locals: Vec::new(),
            ancestors: Default::default(),
            above: Vec::new(),
            lazy_above: Default::default(),
            lazy_with_requests: Default::default(),
            components_above: 0,
            uncertain_above: 0,
            children: self.children,
            used: self.used,
            collected,
            asset: self.asset.clone(),
            types: self.types,
            qualified_class: self.qualified_class.clone(),
            with_style: self.with_style,
            embedded_views: Vec::new(),
            view_class: self.view_class.clone(),
            generics: self.generics.clone(),
            preserve_whitespace: self.preserve_whitespace,
            anchors: Vec::new(),
            embedded,
            locals: Default::default(),
            next_embedded: 1,
            next_node: 0,
            has_doc: false,
            uses_ctx_in_build: false,
            next_projection: 0,
            imp,
            pipes: self.pipes,
            view: 0,
            depth: 0,
            cursor_pipe: 0,
            uses_changed: false,
            after_content: Vec::new(),
            after_view: Vec::new(),
            destroy: Vec::new(),
            subscriptions: 0,
            children_above: Vec::new(),
            detectors: Default::default(),
            detectors_in_order: Vec::new(),
            roots: Vec::new(),
            projected_parent: None,
            stack: Vec::new(),
            registrations: Vec::new(),
            top_level: 0,
            injectors: Vec::new(),
        }
    }
}

/// Emits the class of an embedded view, its factory and — recursively — the
/// embedded views inside it.
///
/// Runs **after** `angular.dart`, because that is where the official compiler
/// writes these classes; the imports it allocates (`embedded_view`, the
/// `text_binding` of the fields, `render_view` of the constructor, `dart:core`
/// of the type argument, `interpolate`) come out in that order.
///
/// In collection mode (`collected` with `Some`), each refusal is recorded and the
/// emission goes on.
fn emit_embedded(
    spec: EmbeddedSpec,
    ctx: &Context,
    imp: &mut Imports,
    names: &mut dartforge_intern::Interner,
    collected: &mut Option<Vec<Refusal>>,
) -> Result<String, Refusal> {
    let ev = imp.alias(EMBEDDED_VIEW);
    let class = spec.class.clone();
    let factory = spec.factory.clone();
    // As in the component's view (case j91): the `text_binding` import is
    // allocated before the body, at the position of the first `TextBinding`
    // field; when no interpolation is mutable (all written in `build()`, like
    // that of an `exports:` name), there is no field and the emission is redone
    // without it, from the same import table (case j112).
    let (imp_before, collected_before) = (imp.clone(), collected.clone());
    let mut with_tb = true;
    let (mut text, nested) = loop {
        let mut inside = ctx.body(imp, names, collected.take(), true);
        inside.locals = spec.locals.clone();
        inside.view = spec.index;
        inside.depth = spec.depth;
        inside.top_level = spec.top_level;
        // The numbering continues where the parent stopped.
        inside.next_embedded = spec.index + 1;
        let r = embedded_body(&mut inside, &spec, ctx, &ev, &class, &factory, with_tb);
        let leftover = inside.tb.is_some() && !inside.tb_used && !inside.collecting();
        *collected = inside.collected.take();
        drop(inside);
        if with_tb && leftover && r.is_ok() {
            *imp = imp_before.clone();
            *collected = collected_before.clone();
            with_tb = false;
            continue;
        }
        break r?;
    };
    for a in nested {
        text.push_str(&emit_embedded(a, ctx, imp, names, collected)?);
    }
    Ok(text)
}

/// The text of an embedded view and the specs of the views nested in it.
fn embedded_body(
    inside: &mut ViewBody,
    spec: &EmbeddedSpec,
    ctx: &Context,
    ev: &str,
    class: &str,
    factory: &str,
    with_tb: bool,
) -> Result<(String, Vec<EmbeddedSpec>), Refusal> {
    // The text binding field is declared before the constructor, so its import
    // goes in here — and that of `package:intl`, before or after it according
    // to document order, when the view has `@i18n` (case j01).
    allocate_lazy_imports(inside.imp, &spec.nodes, ctx.children, ctx.used, &ctx.asset);
    let i18n_order = i18n_before_interpolation(&spec.nodes);
    if i18n_order == Some(true) {
        inside.intl = Some(inside.imp.alias(INTL));
    }
    if with_tb && has_interpolation(&spec.nodes) {
        inside.tb = Some(inside.imp.alias(TEXT_BINDING));
    }
    if i18n_order == Some(false) {
        inside.intl = Some(inside.imp.alias(INTL));
    }
    // With a child, directive or `*` in the view, the order of the fields among
    // them has no case yet; without a message in a field, the HTML one uses
    // `intl` only in the method.
    if has_annotation(&spec.nodes) {
        if i18n_order.is_some()
            && !fields_in_order(&spec.nodes, ctx.children, ctx.used, &ctx.asset).is_empty()
        {
            inside.record_refusal(refusal(
                Reason::I18n,
                "@i18n with a child, directive or `*` in the view",
            ))?;
        }
        inside.intl.get_or_insert_with(String::new);
    }
    let lets: Vec<String> = spec.micro.locals.iter().map(|(n, _)| n.clone()).collect();
    let local_refs = local_refs(&spec.nodes, ctx.children, &ctx.candidate_refs, &lets);
    let mut promoted = local_refs.clone();
    promoted.extend(spec.queried_refs.iter().map(|(n, _, _)| n.clone()));
    inside.queried_refs = spec.queried_refs.clone();
    inside.queries_in_transit = spec.queries_in_transit.clone();
    if let Err(r) = allocate_field_imports(
        inside.imp,
        &spec.nodes,
        ctx.children,
        ctx.used,
        &ctx.asset,
        &ctx.pipes.field_imports(spec.index),
        &promoted_nodes(&spec.nodes, &promoted),
        &ctx.directive_types,
    ) {
        inside.record_refusal(r)?;
    }
    // The constructor comes after the fields and before `build()`, and it is
    // what names the `RenderView`.
    let rv = inside.imp.alias(RENDER_VIEW);
    let util = inside.imp.alias(UTILITIES);
    // The declaration of each local of this view, ready for whoever asks for it
    // (the detection or a `_handleEvent_N`). A local of an *ancestor* view is
    // read through the `parentView` chain
    // (`unsafeCast<_ViewX2>((this.parentView!)) .locals['$implicit']`,
    // `getLocal` of `ViewNameResolver`, case j125).
    inside.this_class = spec.class.clone();
    inside.own_locals = spec.micro.locals.clone();
    inside.ancestors = spec.ancestors.clone();
    inside.above = spec.above.clone();
    inside.lazy_above = spec.lazy_above.clone();
    inside.current_ns = spec.ns.clone();
    inside.lazy_with_requests = spec.lazy_with_requests.clone();
    inside.components_above = spec.components_above;
    inside.uncertain_above = spec.uncertain_above;
    inside.next_projection = spec.next_projection;
    // The `#ref`s of the ancestor views: the field of the node in the view that
    // declares it (`getPropertyInView`, without casting the value: the
    // reference has no type).
    for (name, class, levels) in &spec.ancestor_refs {
        let mut chain = "(this.parentView!)".to_string();
        for _ in 1..*levels {
            chain = format!("({chain}.parentView!)");
        }
        inside.local_decls.insert(
            name.clone(),
            Ok(format!(
                "final local_{name} = {util}.unsafeCast<{class}>({chain}){REF_MARK}.{}{REF_END};",
                ref_key(name, class)
            )),
        );
    }
    inside.ancestor_refs = spec.ancestor_refs.clone();
    // A `#ref` of this view only matters for the queries that reach it (the
    // dynamic ones: `queried_refs` and those in transit); the static one,
    // with the first result outside a `*`, never reaches it
    // (`compile_query.dart: 128-150`, case j109).
    let queried_here: std::collections::HashSet<String> = spec
        .queried_refs
        .iter()
        .chain(&spec.queries_in_transit)
        .map(|(r, _, _)| r.clone())
        .collect();
    inside.free_refs = free_refs_of_view(&spec.nodes, ctx.children, &queried_here);
    inside.embedded_refs = local_refs
        .iter()
        .filter(|n| referenced_in_embedded_views(&spec.nodes, n, ctx.children))
        .cloned()
        .collect();
    inside.event_only_refs = local_refs
        .iter()
        .filter(|n| !read_in_detection(&spec.nodes, n))
        .cloned()
        .collect();
    inside.declare_refs(local_refs);
    for (name, origin) in &spec.ancestors {
        let Some(l) = spec.locals.get(name.as_str()) else {
            continue;
        };
        let decl = local_declaration(l, origin, ctx, &util);
        inside.local_decls.insert(name.clone(), decl);
    }
    for (name, key) in &spec.micro.locals {
        let Some(l) = spec.locals.get(name.as_str()) else {
            continue;
        };
        let origin = Origin {
            key: key.clone(),
            class: spec.class.clone(),
            levels: 0,
        };
        let decl = local_declaration(l, &origin, ctx, &util);
        inside.local_decls.insert(name.clone(), decl);
    }
    let annotated = inside.collected.as_ref().map_or(0, Vec::len);
    inside.nodes(&spec.nodes, "")?;
    inside.check_pipes()?;
    export_refs(
        &ctx.resolved_refs,
        &inside.refs,
        &inside.detectors,
        &inside.refs_in_order,
        &inside.detectors_in_order,
        &spec.class,
    );
    inside.destroy.extend(ctx.pipes.destruction(spec.index));
    // In collection mode, a refused node does not consume an index: the view
    // looks empty without being so. An empty `<ng-container *x>` and a
    // `<template>` without content (only comments, or nothing) are really
    // empty: the view has no root at all (`const <Object>[]`, cases i82, j54).
    let empty = spec.nodes.iter().all(|n| matches!(n, Node::Comment(_)))
        || matches!(spec.nodes.as_slice(), [Node::Element(x)]
        if x.name == "ng-container"
            && x.children.is_empty()
            && x.star.is_none()
            && x.attributes.is_empty()
            && x.properties.is_empty()
            && x.events.is_empty()
            && x.bananas.is_empty()
            && x.references.is_empty());
    if !empty
        && inside.next_node == 0
        && inside.roots.is_empty()
        && inside.collected.as_ref().map_or(0, Vec::len) == annotated
    {
        inside.record_refusal(refusal(Reason::Binding, "embedded view without a node"))?;
    }
    // The locals read by the detection, in order of first use.
    let mut declarations = Vec::new();
    for name in inside.root_locals.clone() {
        if let Some(Ok(d)) = inside.local_decls.get(&name) {
            declarations.push(format!("    {d}"));
        }
    }
    // The listeners are written after the nodes: their imports come now.
    let listeners: Vec<String> = inside
        .listeners
        .iter()
        .map(|o| resolve_deferred(inside.imp, o))
        .collect();
    // The pipe proxies of this view (`afterNodes` of the component's view,
    // which runs after the `bindView` of all of them): they read the instance
    // in the component's view through the `parentView` chain
    // (`getPropertyInView`).
    let mut chain = "(this.parentView!)".to_string();
    for _ in 1..spec.depth {
        chain = format!("({chain}.parentView!)");
    }
    let base = format!("{util}.unsafeCast<{}0>({chain})", ctx.view_class);
    let pipe_creation: Vec<String> = ctx
        .pipes
        .creation(spec.index, &base, inside.imp)
        .iter()
        .map(|l| resolve_deferred(inside.imp, l))
        .collect();
    // `_ctx` in `build()` only if someone reads it there (an event tear-off,
    // immutable text): `maybeCachedCtxDeclarationStatement`.
    let ctx_build = if references_ctx(&inside.lines) || references_ctx(&listeners) {
        "    final _ctx = this.ctx;\n"
    } else {
        ""
    };
    // The nodes, then the listeners — and only then the `initRootNode`, which
    // is the closing declaration (`_generateInitStatement`).
    let body = inside
        .lines
        .iter()
        .chain(&listeners)
        .chain(&pipe_creation)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    // Same order as the top-level view: text bindings, child views and anchors,
    // previous values, elements (the query results before the bound ones).
    let mut all = initialized_fields(&inside);
    all.extend(inside.fields.clone());
    all.extend(inside.child_fields.clone());
    all.extend(detection_fields(
        &inside.expr_fields,
        &inside.embedded_el_fields,
        &inside.embedded_position,
    ));
    all.extend(ctx.pipes.fields(spec.index, inside.imp));
    let mut queried = inside.queried_el_fields.clone();
    queried.sort_by_key(|(k, _)| *k);
    all.extend(queried.into_iter().map(|(_, c)| c));
    all.extend(el_fields_in_order(
        &inside.el_fields,
        &inside.el_field_refs,
        &inside.root_locals,
    ));
    all.extend(inside.el_fields_by_event.clone());
    all.extend(inside.event_el_fields.iter().map(|(_, c)| c.clone()));
    let fields = if all.is_empty() {
        String::new()
    } else {
        format!("{}\n", all.join("\n"))
    };
    let mut det_lines = Vec::new();
    // `_ctx` and `firstCheck` are only declared if the body actually uses them
    // (`writeChangeDetectionStatements`): in an `*ngFor` view the interpolation
    // usually uses the loop local, not the context. Both come before the local
    // declarations.
    if references_ctx(&inside.inputs) || references_ctx(&inside.detection) {
        det_lines.push("    final _ctx = this.ctx;".to_string());
    }
    if inside.uses_changed {
        det_lines.push("    bool changed = false;".to_string());
    }
    if inside.uses_first_check {
        det_lines.push("    bool firstCheck = this.firstCheck;".to_string());
    }
    // Same order as the top-level view, with the locals before everything.
    det_lines.extend(declarations);
    det_lines.extend(inside.inputs.clone());
    for a in &inside.anchors {
        det_lines.push(format!("    this.{a}.detectChangesInNestedViews();"));
    }
    let mut after_content = inside.content_updates.clone();
    after_content.extend(inside.after_content.iter().cloned());
    det_lines.extend(without_throw(&after_content));
    det_lines.extend(inside.detection.clone());
    for v in &inside.child_views {
        det_lines.push(format!("    this.{v}.detectChanges();"));
    }
    det_lines.extend(without_throw(&inside.after_view));
    let detection = if det_lines.is_empty() {
        String::new()
    } else {
        format!(
            "\n  @override\n  void detectChangesInternal() {{\n{}\n  }}\n",
            det_lines.join("\n")
        )
    };
    // `dirtyParentQueriesInternal`, between the detection and
    // `destroyInternal`: marks each query of the component's view with a result
    // here, and each content query of a view above (`_setParentQueryAsDirty`,
    // in the order of the first result).
    let mut of_content = spec
        .star
        .and_then(|k| ctx.dirty_content_queries.borrow().get(&k).cloned())
        .unwrap_or_default();
    of_content.sort_by_key(|(p, kind, _, _, _)| (*p, *kind));
    let dirty = if spec.queried_refs.is_empty() && of_content.is_empty() {
        String::new()
    } else {
        let chain = |levels: u32| {
            let mut view = "this".to_string();
            for _ in 0..levels {
                view = format!("({view}.parentView!)");
            }
            view
        };
        // The order is that of the first result of each query in this view; on
        // the same node, the content one before the view one
        // (`_getQueriesFor`).
        let mut marked: Vec<(usize, u8, usize, String)> = Vec::new();
        for (k, (r, field, levels)) in spec.dirty.iter().enumerate() {
            let position =
                first_result_start_in_view(&spec.nodes, r, ctx.children).unwrap_or(usize::MAX);
            marked.push((
                position,
                1,
                k,
                format!(
                    "    {util}.unsafeCast<{}0>({}).{field} = true;",
                    ctx.view_class,
                    chain(*levels)
                ),
            ));
        }
        for (k, (position, kind, field, levels, class)) in of_content.iter().enumerate() {
            marked.push((
                *position,
                *kind,
                k,
                format!(
                    "    {util}.unsafeCast<{class}>({}).{field} = true;",
                    chain(*levels)
                ),
            ));
        }
        // Without a known position, the previous order (the view ones already
        // come sorted by the first result).
        marked.sort_by_key(|(p, ty, k, _)| (*p, *ty, *k));
        let lines: Vec<String> = marked.into_iter().map(|(_, _, _, l)| l).collect();
        format!(
            "\n  @override\n  void dirtyParentQueriesInternal() {{\n{}\n  }}\n",
            lines.join("\n")
        )
    };
    // `injectorGetInternal` comes after `build()` and before the detection.
    let injector = resolve_deferred(
        inside.imp,
        &injector_method(&inside.injectors, &inside.asset),
    );
    let detection = resolver_refs(
        &resolve_deferred(inside.imp, &detection),
        &ctx.resolved_refs.borrow(),
    );
    let detection = format!(
        "{}{detection}",
        link_method(ctx.detection_link, &inside.anchors, &inside.linked_views)
    );
    // An embedded view also destroys what it hung on itself.
    let destruction = if inside.anchors.is_empty()
        && inside.child_views.is_empty()
        && inside.destroy.is_empty()
    {
        String::new()
    } else {
        let mut lines: Vec<String> = inside
            .anchors
            .iter()
            .map(|a| format!("    this.{a}.destroyNestedViews();"))
            .collect();
        lines.extend(
            inside
                .child_views
                .iter()
                .map(|v| format!("    this.{v}.destroyInternalState();")),
        );
        lines.extend(inside.destroy.iter().cloned());
        format!(
            "\n  @override\n  void destroyInternal() {{\n{}\n  }}\n",
            lines.join("\n")
        )
    };
    let nested = std::mem::take(&mut inside.embedded_views);
    // The `_handleEvent_N`s, after `destroyInternal`.
    if !inside.i18n_methods.is_empty() && !inside.event_methods.is_empty() {
        return Err(refusal(
            Reason::I18n,
            "@i18n with HTML and an event handler in the same view",
        ));
    }
    let methods: String = inside
        .i18n_methods
        .iter()
        .chain(&inside.event_methods)
        .map(|m| {
            resolver_refs(
                &resolve_deferred(inside.imp, m),
                &ctx.resolved_refs.borrow(),
            )
        })
        .collect();
    // The roots (`rootNodesOrViewContainers`): each node created without a
    // parent, the local or the field (`renderNode.toReadExpr()`) — more than
    // one when the `*` is on an `<ng-container>`. In collection mode, with a
    // refused node, the list may come back empty; the text is discarded.
    let roots = if inside.roots.is_empty() {
        vec!["_el_0".to_string()]
    } else {
        inside.roots.clone()
    };
    // `_generateInitStatement`: a single root and no `subscription_N` is
    // `initRootNode`; the rest goes in a list.
    let flat = flat_list(&roots, &util)?;
    let start = match (&flat, inside.subscriptions) {
        _ if empty && inside.roots.is_empty() && inside.subscriptions == 0 => format!(
            "this.initRootNodesAndSubscriptions({util}.unsafeCast(const <Object>[]), null);"
        ),
        (FlatList::Literal(items), 0) if items.len() == 1 => {
            format!("this.initRootNode({});", items[0])
        }
        (_, subscriptions) => {
            let subs = if subscriptions == 0 {
                "null".to_string()
            } else {
                let list: Vec<String> = (0..subscriptions)
                    .map(|k| format!("subscription_{k}"))
                    .collect();
                format!("[{}]", list.join(", "))
            };
            match &flat {
                // With the cascade broken, the arguments go one per line
                // (continuation indentation, column 8).
                FlatList::Cascade { .. } => format!(
                    "this.initRootNodesAndSubscriptions(\n        {util}.unsafeCast({}),\n        {subs});",
                    flat.text_at(8)
                ),
                _ => format!(
                    "this.initRootNodesAndSubscriptions({util}.unsafeCast({}), {subs});",
                    flat.text()
                ),
            }
        }
    };
    // Without a created node (the root is just a `TextBinding`), `build()` is
    // only the `initRootNode`.
    let body = if body.is_empty() {
        body
    } else {
        format!("{body}\n")
    };
    let context_type = &ctx.context_type;
    let (args, decl) = (&ctx.generics, &ctx.generics_decl);
    let text = format!(
        "\nclass {class}{decl} extends {ev}.EmbeddedView<{context_type}> {{\n{fields}  {class}({rv}.RenderView parentView, int parentIndex) : super(parentView, parentIndex);\n  @override\n  void build() {{\n{ctx_build}{body}    {start}\n  }}\n{injector}{detection}{dirty}{destruction}{methods}}}\n\n{ev}.EmbeddedView<void> {factory}{decl}({rv}.RenderView parentView, int parentIndex) {{\n  return {class}{args}(parentView, parentIndex);\n}}\n"
    );
    Ok((text, nested))
}

/// `final local_x = unsafeCast<T>(this.locals['key']);` — the declaration that
/// `ViewNameResolver` keeps for the local, with the type qualified by the
/// import of the library that declares it. The import stays marked: it is
/// allocated where the declaration is written first.
fn local_declaration(
    l: &crate::expr::Local,
    origin: &Origin,
    ctx: &Context,
    util: &str,
) -> Result<String, Refusal> {
    let d = &l.dart;
    // The key comes out as an escaped literal (`'\$implicit'`): without the
    // escape, the `$` would become an interpolation in Dart.
    let key = literal(&origin.key);
    // A local of this view: `this.locals`. Of an ancestor view: the
    // `parentView` chain, each step with `!`, and the cast to the class of the
    // view that declares it.
    let locals = if origin.levels == 0 {
        "this.locals".to_string()
    } else {
        let mut view = "(this.parentView!)".to_string();
        for _ in 1..origin.levels {
            view = format!("({view}.parentView!)");
        }
        format!("{util}.unsafeCast<{}>({view}).locals", origin.class)
    };
    // Without a type, without a cast (`getLocal`: `type != null && type !=
    // dynamic`).
    if l.ty == "dynamic" {
        return Ok(format!("final {d} = {locals}[{key}];"));
    }
    let ty = if matches!(
        l.ty.as_str(),
        "String" | "int" | "double" | "bool" | "num" | "Object"
    ) {
        format!("{}{}", deferred_q("dart:core"), l.ty)
    } else {
        let without_import = || {
            refusal(
                Reason::Binding,
                "type of the `*ngFor` local without an import",
            )
        };
        let (r, file) = ctx.types.ok_or_else(without_import)?;
        let scope = l.scope.as_deref().unwrap_or(file);
        let full = instantiate_raw(&l.ty, scope, r).ok_or_else(without_import)?;
        let free: Vec<&str> = ctx.generic_names.iter().map(String::as_str).collect();
        qualified_type(&full, scope, r, &ctx.asset, &free).ok_or_else(without_import)?
    };
    // A private type becomes `dynamic` in `fromDartType`, and `getLocal` only
    // adds the cast when the type is not `dynamic`
    // (`view_name_resolver.dart:55`).
    if ty == "dynamic" {
        return Ok(format!("final {d} = {locals}[{key}];"));
    }
    Ok(format!(
        "final {d} = {util}.unsafeCast<{ty}>({locals}[{key}]);"
    ))
}

/// `if ((!debugThrowIfChanged)) { .. }` around the content or view hooks
/// (`notThrowOnChanges` in `writeChangeDetectionStatements`); nothing if there
/// are no hooks.
fn without_throw(lines: &[String]) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let dbg = deferred(CHECK_BINDING);
    Some(format!(
        "    if ((!{dbg}.debugThrowIfChanged)) {{\n{}\n    }}",
        indent_lines(&lines.join("\n"), 2)
    ))
}

/// Reindents a block of statements: each line gets `n` spaces.
fn indent_lines(text: &str, n: usize) -> String {
    let prefix = " ".repeat(n);
    text.lines()
        .map(|l| format!("{prefix}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `addStmtsIfFirstCheck`: if the last statement of the method is an `if
/// (firstCheck)`, the new ones go into it; otherwise, they open another.
fn in_first_check(method: &mut Vec<String>, statements: &[String]) {
    const OPENS: &str = "    if (firstCheck) {\n";
    const CLOSES: &str = "\n    }";
    match method.last_mut() {
        Some(last) if last.starts_with(OPENS) && last.ends_with(CLOSES) => {
            last.truncate(last.len() - CLOSES.len());
            last.push('\n');
            last.push_str(&statements.join("\n"));
            last.push_str(CLOSES);
        }
        _ => method.push(format!("{OPENS}{}{CLOSES}", statements.join("\n"))),
    }
}

/// Allocates the imports that the class **fields** use, in the order they are
/// declared — text bindings, child views and structural directives, and finally
/// `dart:html` of the elements that become fields.
///
/// The order of the imports is the order in which the official compiler writes
/// the file, and the fields come before everything in the class. It holds
/// equally for the top-level view and for each embedded view.
/// The imports of the view's lazy providers. `ViewStorage` writes the fields in
/// the order they are allocated: the lazy provider is allocated when the view
/// is built, and the text binding and the nodes only later, in
/// `NodeReferenceStorageVisitor` — so their fields (and imports) come before
/// all the others, including `text_binding.dart` (case j38).
fn allocate_lazy_imports(
    imp: &mut Imports,
    nodes: &[Node],
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
    asset: &str,
) {
    for field in fields_in_order(nodes, children, used, asset) {
        if let ViewField::Lazy(texts) = field {
            for t in &texts {
                resolve_deferred(imp, t);
            }
        }
    }
}

fn allocate_field_imports(
    imp: &mut Imports,
    nodes: &[Node],
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
    asset: &str,
    pipes: &[String],
    refs: &std::collections::HashSet<String>,
    types: &[ResolvedDirectiveType],
) -> Result<(), Refusal> {
    let without_path = || refusal(Reason::ComponentInTemplate, "child without an import path");
    // The `directiveTypes:` arguments come out in the field type, right after
    // the class name: their imports come right after the class's.
    let arguments_of = |imp: &mut Imports, import_text: &str| {
        for t in types {
            let of_class = deferred(&import_of(&t.uri, asset));
            let of_ngcd = deferred(&import_of(&t.uri.replace(".dart", ".template.dart"), asset));
            if import_text == of_class || import_text == of_ngcd {
                resolve_deferred(imp, &t.arguments);
            }
        }
    };
    let fields = fields_in_order(nodes, children, used, asset);
    for field in fields {
        match field {
            ViewField::Lazy(_) => {}
            ViewField::Container => {
                imp.alias(VIEW_CONTAINER);
            }
            ViewField::Child(f, container, before) => {
                for (k, uri) in [&f.uri_template, &f.uri_dart].into_iter().enumerate() {
                    // The node's `ViewContainer` (a child or one of its
                    // directives that injects `ViewContainerRef`) and the
                    // providers the child injects between the view and the
                    // instance.
                    if k == 1 {
                        if container {
                            imp.alias(VIEW_CONTAINER);
                        }
                        for t in &before {
                            resolve_deferred(imp, t);
                        }
                    }
                    let target = asset_of_uri(uri, "", Path::new("")).ok_or_else(without_path)?;
                    let path = import_path(asset, &target).ok_or_else(without_path)?;
                    if !is_own_template(asset, &path) {
                        imp.alias(&path);
                    }
                    for t in types {
                        if t.uri == f.uri_dart && t.class == f.class {
                            resolve_deferred(imp, &t.arguments);
                        }
                    }
                }
            }
            ViewField::Structural(uri) => {
                imp.alias(VIEW_CONTAINER);
                imp.alias(uri);
            }
            ViewField::Template(with_ref, fields) => {
                imp.alias(VIEW_CONTAINER);
                if with_ref {
                    imp.alias(TEMPLATE_REF);
                }
                for t in fields {
                    resolve_deferred(imp, &t);
                    arguments_of(imp, &t);
                }
            }
            ViewField::Directives(texts) => {
                for t in texts {
                    resolve_deferred(imp, &t);
                    arguments_of(imp, &t);
                }
            }
        }
    }
    // The pipe fields come after the `_expr_`s and before the `_el_`s.
    for uri in pipes {
        imp.alias(uri);
    }
    // The `dart:html` of the element fields. In 9 each type has its own
    // `package:web` module: each one goes in at the position of its first
    // field.
    match crate::dialect::current() {
        crate::dialect::Dialect::Ngdart => {
            if has_bound_element(nodes, children, used, refs) {
                imp.alias("dart:html");
            }
        }
        crate::dialect::Dialect::Ngx => {
            let mut types = Vec::new();
            bound_element_types(nodes, children, used, refs, false, &mut types);
            for t in types {
                imp.alias(crate::dialect::dom(t).0);
            }
        }
    }
    Ok(())
}

/// The types (by the `dart:html` name) of the nodes of this view that become
/// fields, in document order: what [`has_bound_element`] finds, with the type
/// of each one (`Element` with a namespace, `HtmlElement` on a child's node).
fn bound_element_types(
    nodes: &[Node],
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
    refs: &std::collections::HashSet<String>,
    in_ns: bool,
    output: &mut Vec<&'static str>,
) {
    for n in nodes {
        let Node::Element(e) = n else { continue };
        if e.star.is_some() {
            continue;
        }
        let ns = in_ns
            || e.name.contains(':')
            || matches!(e.name.to_ascii_lowercase().as_str(), "svg" | "math");
        match children.get(&e.name) {
            None => {
                if binds_on_element(e, &matched_directives(used, e))
                    || e.references.iter().any(|r| refs.contains(&r.name))
                {
                    output.push(if ns {
                        "Element"
                    } else {
                        dom::tag_type(&e.name)
                    });
                }
            }
            Some(f) => {
                if bound_child_element(e, f, &matched_directives(used, e)) {
                    output.push("HtmlElement");
                }
            }
        }
        bound_element_types(&e.children, children, used, refs, ns, output);
    }
}

/// Alguma linha usa `_ctx`?
fn references_ctx(lines: &[String]) -> bool {
    lines.iter().any(|l| {
        l.split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|t| t == "_ctx")
    })
}

/// Index of the element that serves as the parent, or `null` if it is the root
/// of the view.
fn element_index(parent: &str) -> String {
    parent
        .rsplit_once("_el_")
        .or_else(|| parent.rsplit_once("_anchor_"))
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| "null".to_string())
}

/// How many structural directives there are in the subtree — it is the jump the
/// numbering of the embedded views makes before the next sibling.
fn count_structural(nodes: &[Node]) -> u32 {
    nodes
        .iter()
        .map(|n| match n {
            Node::Element(e) => u32::from(e.star.is_some()) + count_structural(&e.children),
            _ => 0,
        })
        .sum()
}

/// Does the local appear in some expression of the subtree?
///
/// Its declaration opens `detectChangesInternal` and brings `dart:core` along,
/// so the decision must be made before walking the body.
fn local_referenced(nodes: &[Node], name: &str) -> bool {
    let references = |text: &str| references_at_root(text, name);
    nodes.iter().any(|n| match n {
        Node::Interpolation { expr, .. } => references(expr),
        Node::Element(e) => {
            e.properties
                .iter()
                .chain(e.events.iter())
                .chain(e.bananas.iter())
                .any(|l| references(&l.value))
                || e.attributes
                    .iter()
                    .any(|a| a.value.contains("{{") && references(&a.value))
                || e.star.as_ref().is_some_and(|l| references(&l.value))
                || local_referenced(&e.children, name)
        }
        _ => false,
    })
}

/// Does the expression read `name` with the implicit receiver — which is when
/// `ViewNameResolver.getLocal` is called? A name after `.`/`?.` is a member of
/// something else, before `:` after `(`/`,` it is a named argument, and inside
/// quotes it is text.
fn references_at_root(text: &str, name: &str) -> bool {
    let cs: Vec<char> = text.chars().collect();
    let part = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '\'' || c == '"' {
            i += 1;
            while i < cs.len() && cs[i] != c {
                if cs[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if !part(c) {
            i += 1;
            continue;
        }
        let begin = i;
        while i < cs.len() && part(cs[i]) {
            i += 1;
        }
        if cs[begin..i].iter().copied().eq(name.chars()) {
            let before = cs[..begin].iter().rev().find(|c| !c.is_whitespace());
            let after = cs[i..].iter().find(|c| !c.is_whitespace());
            let member = before == Some(&'.');
            let named = after == Some(&':') && matches!(before, Some('(' | ','));
            if !member && !named {
                return true;
            }
        }
    }
    false
}

/// The structural directives the generator knows, with what changes in each
/// one. What makes `NgIf` and `NgFor` differ is declared in ngcompiler:
/// `_isDirectBinding` (in `semantic_analysis/binding_converter.dart`) exempts
/// `NgIf` from `checkBinding`, and `NgFor` implements `DoCheck`.
struct Structural {
    class: &'static str,
    uri: &'static str,
    /// The input is written directly, without `checkBinding`.
    direct: bool,
    /// Implements `DoCheck`: the view calls `ngDoCheck()` during detection.
    do_check: bool,
    /// Third constructor argument: `@Host()` of a directive of the same library
    /// on an element above (the `NgSwitch` of `NgSwitchWhen`).
    host: Option<&'static str>,
    /// The constructor receives the `TemplateRef` (`NgTemplateOutlet` only the
    /// `ViewContainerRef`; the `TemplateRef` of the `*` stays in an unused
    /// local).
    with_template: bool,
    /// The inputs (`@Input`), in declaration order in the class: it is the
    /// order of the bindings (`ast.inputs.sort(_orderingOf(directive.inputs))`
    /// in `ast_template_parser.dart`), whatever the written order.
    inputs: &'static [&'static str],
}

impl Structural {
    fn known(name: &str) -> Option<Structural> {
        match name {
            "ngIf" => Some(Structural {
                class: "NgIf",
                uri: NG_IF,
                direct: true,
                do_check: false,
                host: None,
                with_template: true,
                inputs: &["ngIf"],
            }),
            "ngFor" => Some(Structural {
                class: "NgFor",
                uri: NG_FOR,
                direct: false,
                do_check: true,
                host: None,
                with_template: true,
                inputs: &["ngForOf", "ngForTemplate", "ngForTrackBy"],
            }),
            "ngTemplateOutlet" => Some(Structural {
                class: "NgTemplateOutlet",
                uri: NG_TEMPLATE_OUTLET,
                direct: false,
                do_check: true,
                host: None,
                with_template: false,
                inputs: &[
                    "ngTemplateOutlet",
                    "ngTemplateOutletContext",
                    "ngTemplateOutletValue",
                ],
            }),
            // Both inputs are setters without their own comparison: they go
            // through `checkBinding` (or, if immutable, through
            // `_bindLiteral`).
            "ngSwitchCase" | "ngSwitchWhen" => Some(Structural {
                class: "NgSwitchWhen",
                uri: NG_SWITCH,
                direct: false,
                do_check: false,
                host: Some("NgSwitch"),
                with_template: true,
                inputs: &["ngSwitchCase", "ngSwitchWhen"],
            }),
            "ngSwitchDefault" => Some(Structural {
                class: "NgSwitchDefault",
                uri: NG_SWITCH,
                direct: false,
                do_check: false,
                host: Some("NgSwitch"),
                with_template: true,
                inputs: &[],
            }),
            _ => None,
        }
    }
}

/// `List<String>` -> `String`. Without a type argument the loop local cannot be
/// typed, and the official compiler writes `unsafeCast<T>` with an explicit T.
fn element_type(ty: &str) -> Option<String> {
    let t = ty.trim().trim_end_matches('?');
    let opens = t.find('<')?;
    let base = t[..opens].trim();
    if !matches!(base, "List" | "Iterable" | "Set") {
        return None;
    }
    let inside = t[opens + 1..].strip_suffix('>')?;
    // A single argument at the top level; the nested ones (`List<List<X>>`,
    // `List<Map<K, V>>`) go whole.
    let mut level = 0i32;
    for c in inside.chars() {
        match c {
            '<' => level += 1,
            '>' => level -= 1,
            ',' if level == 0 => return None,
            _ => {}
        }
        if level < 0 {
            return None;
        }
    }
    (level == 0).then(|| inside.trim().to_string())
}

/// The type the analyzer gives to the name of a generic class written raw
/// (`Grupo` in `List<Grupo>`): the instantiation to bounds, which
/// `fromDartType` writes with the arguments (`Grupo<dynamic>`). Without a bound
/// the argument is `dynamic`; with a bound there is no case yet (`None`).
fn instantiate_raw(text: &str, scope: &Path, r: &dyn Resolution) -> Option<String> {
    let mut output = String::new();
    let chars: Vec<char> = text.trim().chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_alphanumeric() || c == '_' || c == '$' {
            let mut name = String::new();
            while i < chars.len()
                && (chars[i].is_alphanumeric() || matches!(chars[i], '_' | '$' | '.'))
            {
                name.push(chars[i]);
                i += 1;
            }
            output.push_str(&name);
            let followed_by_args = chars[i..].iter().find(|c| !c.is_whitespace()) == Some(&'<');
            if !followed_by_args && !matches!(name.as_str(), "dynamic" | "void" | "Function") {
                if let Some(bounds) = r.type_bounds(scope, &name) {
                    if !bounds.is_empty() {
                        if bounds.iter().any(|&l| l) {
                            return None;
                        }
                        output.push('<');
                        output.push_str(&vec!["dynamic"; bounds.len()].join(", "));
                        output.push('>');
                    }
                }
            }
            continue;
        }
        output.push(c);
        i += 1;
    }
    Some(output)
}

/// The text of a type with each name qualified by the import of the library
/// that declares it, as the official compiler writes a `DartType`
/// (`List<import2.X>`: `dart:core` without a prefix, but with the import
/// allocated). The imports stay marked ([`deferred_q`]). `None` for what is not a
/// named type (function, record) or a name not found in `scope`.
/// The names in `free` (type parameters in scope) come out as they are.
fn qualified_type(
    text: &str,
    scope: &Path,
    r: &dyn Resolution,
    asset: &str,
    free: &[&str],
) -> Option<String> {
    let mut output = String::new();
    let mut chars = text.trim().chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_alphanumeric() || c == '_' || c == '$' {
            let mut name = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || matches!(c, '_' | '$' | '.') {
                    name.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            if matches!(name.as_str(), "dynamic" | "void") || free.contains(&name.as_str()) {
                output.push_str(&name);
                continue;
            }
            if name == "Function" {
                return None;
            }
            let simple = crate::resolucao::without_anchor(name.rsplit('.').next().unwrap_or(&name));
            // A private type is not visible in the generated file:
            // `fromDartType` replaces it with `dynamic`, with its arguments and
            // `?` (`convert.dart:35-37`).
            if simple.starts_with('_') {
                output.push_str("dynamic");
                skip_arguments_and_nullable(&mut chars);
                continue;
            }
            let uri = r.type_uri(scope, &name)?;
            let path = if uri.starts_with("dart:") {
                uri
            } else {
                import_path(asset, &asset_of_uri(&uri, "", Path::new(""))?)?
            };
            output.push_str(&deferred_q(&path));
            output.push_str(simple);
            continue;
        }
        chars.next();
        match c {
            '<' | '>' | '?' => output.push(c),
            ',' => output.push_str(", "),
            c if c.is_whitespace() => {}
            _ => return None,
        }
    }
    Some(output)
}

/// The second argument of `addEventListener` for a native event: the handler in
/// 8; in 9, the function exported to JS with the event type
/// (`FunctionToJSExportedDartFunction((h as void Function(MouseEvent))).toJS`,
/// `NX:compiler/view_compiler/update_statement_visitor.dart:254-261`). The
/// imports go as deferred marks, allocated in text order.
fn native_listener(name: &str, handler: &str) -> String {
    if crate::dialect::current() == crate::dialect::Dialect::Ngdart {
        return handler.to_string();
    }
    let js = deferred_q("dart:js_interop");
    let ty = native_event_type(name);
    format!("{js}FunctionToJSExportedDartFunction(({handler} as void Function({ty}))).toJS")
}

/// The type of a native event in 9, qualified by a deferred mark
/// (`nativeHtmlEventType`, `NX:compiler/html_events.dart:116-177`).
fn native_event_type(name: &str) -> String {
    let (uri, n) = crate::dialect::dom(crate::dialect::native_event_type(name));
    format!("{}{n}", deferred_q(uri))
}

/// The type of the `$event` of an event written on the element: in 9, that of
/// the native event (`NativeEvent`); a custom event (`keyup.enter`,
/// `CustomEvent`) and 8 have none (`binding_converter.dart:75,228`).
fn native_event_type_q(name: &str) -> Option<String> {
    (crate::dialect::current() == crate::dialect::Dialect::Ngx && native_event(name))
        .then(|| native_event_type(name))
}

/// The parameter of a `_handleEvent_N`: a raw `$event` in 8 (and in 9 without a
/// type: a custom event, an output that is not a `Stream`), `T $event` with the
/// event type in 9 (`NX:compiler/view_compiler/compile_view.dart:726-760`).
fn event_parameter(ty: &Option<String>) -> String {
    match ty {
        Some(t) => format!("{t} $event"),
        None => "$event".to_string(),
    }
}

/// Consumes, after a type name, its arguments (`<..>`, nested) and its `?`.
fn skip_arguments_and_nullable(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while chars.next_if(|c| c.is_whitespace()).is_some() {}
    if chars.next_if_eq(&'<').is_some() {
        let mut level = 1usize;
        for c in chars.by_ref() {
            match c {
                '<' => level += 1,
                '>' => {
                    level -= 1;
                    if level == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    while chars.next_if(|c| c.is_whitespace()).is_some() {}
    chars.next_if_eq(&'?');
}

/// Import prefix allocated later. The official compiler numbers the imports in
/// the order it **writes** the file, and `detectChangesInternal` is written
/// after the whole `build()`; we build both at the same time, walking the
/// template. The mark keeps the URI until the detection is written
/// ([`resolve_deferred`]).
fn deferred(uri: &str) -> String {
    format!("\u{1}{uri}\u{2}")
}

/// Like [`deferred`], but resolves to the qualifier (`import3.` or nothing, for
/// the libraries imported without a prefix).
fn deferred_q(uri: &str) -> String {
    format!("\u{1}q:{uri}\u{2}")
}

/// Replaces each mark of [`deferred`] and [`deferred_q`] with the prefix,
/// allocating in text order.
fn resolve_deferred(imp: &mut Imports, text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('\u{1}') {
        output.push_str(&rest[..i]);
        let Some(f) = rest[i..].find('\u{2}') else {
            break;
        };
        let mark = &rest[i + 1..i + f];
        if let Some((key, uri)) = mark.strip_prefix("k:").and_then(|m| m.split_once('|')) {
            output.push_str(&imp.q_keyed(key, uri));
        } else {
            match mark.strip_prefix("q:") {
                Some(uri) => output.push_str(&imp.q(uri)),
                None => output.push_str(&imp.alias(mark)),
            }
        }
        rest = &rest[i + f + 1..];
    }
    output.push_str(rest);
    output
}

/// Property or attribute name with a security context in some tag
/// (`_initializeSecuritySchema`, in `dom_element_schema_registry.dart`): the
/// value comes out wrapped in a `sanitize*`. Without looking at the tag,
/// refuses the name on any element.
/// The sanitizer of a DOM property (`securityContext` of
/// `DomElementSchemaRegistry`, with its table): `sanitizeHtml`,
/// `sanitizeStyle`, `sanitizeUrl` or `sanitizeResourceUrl`, or none. The key is
/// `tag|property`, then `*|property`.
fn sanitizer(tag: &str, prop: &str) -> Option<&'static str> {
    const HTML: &[&str] = &["iframe|srcdoc", "*|innerHTML", "*|outerHTML"];
    const URL: &[&str] = &[
        "*|formAction",
        "area|href",
        "area|ping",
        "audio|src",
        "a|href",
        "a|ping",
        "blockquote|cite",
        "body|background",
        "del|cite",
        "form|action",
        "img|src",
        "img|srcset",
        "input|src",
        "ins|cite",
        "q|cite",
        "source|src",
        "source|srcset",
        "video|poster",
        "video|src",
    ];
    const RESOURCE: &[&str] = &[
        "applet|code",
        "applet|codebase",
        "base|href",
        "embed|src",
        "frame|src",
        "head|profile",
        "html|manifest",
        "iframe|src",
        "link|href",
        "media|src",
        "object|codebase",
        "object|data",
        "script|src",
        "track|src",
    ];
    let context = |key: &str| {
        if HTML.contains(&key) {
            Some("sanitizeHtml")
        } else if key == "*|style" {
            Some("sanitizeStyle")
        } else if URL.contains(&key) {
            Some("sanitizeUrl")
        } else if RESOURCE.contains(&key) {
            Some("sanitizeResourceUrl")
        } else {
            None
        }
    };
    context(&format!("{tag}|{prop}")).or_else(|| context(&format!("*|{prop}")))
}

/// `getMappedPropName` of `DomElementSchemaRegistry`: the property name of an
/// attribute, used to find the security context.
fn mapped_property(attribute: &str) -> &str {
    match attribute {
        "class" => "className",
        "innerHtml" => "innerHTML",
        "readonly" => "readOnly",
        "tabindex" => "tabIndex",
        other => other,
    }
}

/// ngcompiler's `isNativeHtmlEvent` (`html_events.dart`): only these go
/// directly to `addEventListener`.
pub(crate) fn native_event(name: &str) -> bool {
    const NATIVE_EVENTS: &[&str] = &[
        "abort",
        "afterprint",
        "animationend",
        "animationiteration",
        "animationstart",
        "appinstalled",
        "audioend",
        "audiostart",
        "beforeprint",
        "beforeunload",
        "blur",
        "canplay",
        "canplaythrough",
        "change",
        "click",
        "compositionend",
        "compositionstart",
        "compositionupdate",
        "contextmenu",
        "copy",
        "cut",
        "dblclick",
        "drag",
        "dragend",
        "dragenter",
        "dragleave",
        "dragover",
        "dragstart",
        "drop",
        "durationchange",
        "ended",
        "error",
        "focus",
        "focusin",
        "focusout",
        "fullscreenchange",
        "fullscreenerror",
        "gotpointercapture",
        "hashchange",
        "input",
        "invalid",
        "keydown",
        "keypress",
        "keyup",
        "languagechange",
        "load",
        "loadeddata",
        "loadedmetadata",
        "loadstart",
        "lostpointercapture",
        "message",
        "mousedown",
        "mouseenter",
        "mouseleave",
        "mousemove",
        "mouseout",
        "mouseover",
        "mouseup",
        "notificationclick",
        "offline",
        "online",
        "open",
        "orientationchange",
        "pagehide",
        "pageshow",
        "paste",
        "pause",
        "play",
        "playing",
        "progress",
        "pointercancel",
        "pointerdown",
        "pointerenter",
        "pointerleave",
        "pointerlockchange",
        "pointerlockerror",
        "pointermove",
        "pointerout",
        "pointerover",
        "pointerup",
        "ratechange",
        "reset",
        "resize",
        "scroll",
        "search",
        "seeked",
        "seeking",
        "select",
        "show",
        "stalled",
        "storage",
        "submit",
        "suspend",
        "timeupdate",
        "toggle",
        "touchcancel",
        "touchend",
        "touchmove",
        "touchstart",
        "transitionend",
        "unload",
        "volumechange",
        "waiting",
        "wheel",
    ];
    NATIVE_EVENTS.contains(&name)
}

/// A name that belongs to an ecosystem directive, not to the DOM.
fn is_directive_name(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or(name);
    base.starts_with("ng") || base.starts_with("form") && base != "form"
}

/// Does the template have `{{ … }}` in this view?
///
/// It does not descend into the subtree of a `*`: that content belongs to the
/// embedded view, and its fields are declared there.
fn has_interpolation(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Interpolation { .. } => true,
        Node::Element(e) if e.star.is_none() => has_interpolation(&e.children),
        _ => false,
    })
}

/// `o.literal('${value}')` of a primitive literal: the text of the value,
/// between single quotes. A string literal already arrives written like that;
/// `null` becomes empty text.
fn literal_value(text: &str) -> String {
    if text.starts_with('\'') {
        text.to_string()
    } else if text == "null" {
        "''".to_string()
    } else {
        literal(text)
    }
}

/// `&ngsp;`, which `ngast` replaces with this character; it turns back into a
/// space.
const NGSP: char = '\u{E500}';

/// `_compressWhitespacePreceding`: with a line break (and without `&nbsp;` or
/// `&ngsp;`), the line breaks disappear and the start is trimmed.
fn compress_before(t: &str) -> String {
    if t.contains('\u{00A0}') || t.contains(NGSP) || !t.contains('\n') {
        return t.replace(NGSP, " ");
    }
    t.replace('\n', "").trim_start().replace(NGSP, " ")
}

/// `_compressWhitespaceFollowing`: the same, trimming the end.
fn compress_after(t: &str) -> String {
    if t.contains('\u{00A0}') || t.contains(NGSP) || !t.contains('\n') {
        return t.replace(NGSP, " ");
    }
    t.replace('\n', "").trim_end().replace(NGSP, " ")
}

/// The texts and expressions of a value with `{{ }}` (`splitInterpolation`,
/// with the regular expression `{{([\s\S]*?)}}`): always one more text than
/// expressions. An empty expression is an error in the official compiler.
fn interpolation_parts(value: &str) -> Option<(Vec<String>, Vec<String>)> {
    let mut texts = Vec::new();
    let mut exprs = Vec::new();
    let mut rest = value;
    while let Some(i) = rest.find("{{") {
        let after = &rest[i + 2..];
        let f = after.find("}}")?;
        let e = &after[..f];
        if e.trim().is_empty() {
            return None;
        }
        texts.push(rest[..i].to_string());
        exprs.push(e.to_string());
        rest = &after[f + 2..];
    }
    texts.push(rest.to_string());
    (!exprs.is_empty()).then_some((texts, exprs))
}

/// Dart literal of a string, with single quotes — the official emitter's
/// `escapeSingleQuoteString`.
pub(crate) fn literal(t: &str) -> String {
    let mut s = String::with_capacity(t.len() + 2);
    s.push('\'');
    for c in t.chars() {
        match c {
            '\'' => s.push_str("\\'"),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '$' => s.push_str("\\$"),
            c => s.push(c),
        }
    }
    s.push('\'');
    s
}

/// The `.template.dart` of a file with only directives with `@HostBinding`: the
/// `XNgCd` classes in source order, with the shared import table
/// ([`ngcd_class`]).
pub fn directive_change_detector(hs: &[&crate::Host], file: &str) -> String {
    let mut imp = Imports::default();
    let classes: Vec<String> = hs.iter().map(|h| ngcd_class(h, file, &mut imp)).collect();
    let mut s = String::with_capacity(1024 * classes.len().max(1));
    s.push_str(crate::HEADER);
    let _ = writeln!(s, "import '{file}';");
    imp.write_to(&mut s);
    for c in &classes {
        s.push_str(c);
    }
    s
}

/// The `XNgCd` class of a directive with `@HostBinding`
/// (`DirectiveChangeDetector`), with the imports allocated in `imp` in writing
/// order: the superclass, the `instance` field, the parameters of
/// `detectHostChanges` and, in the body, `checkBinding` and, through each
/// binding's action, `dom_helpers` (and the sanitizer, when there is one). In a
/// file with a component, it comes out after the components' fragments (case
/// j45).
pub fn ngcd_class(h: &crate::Host, file: &str, imp: &mut Imports) -> String {
    let cd = imp.alias(DIRECTIVE_CHANGE_DETECTOR);
    let own_prefix = imp.alias(file);
    let rv = imp.alias(RENDER_VIEW);
    let element = imp.dom("Element");
    // The imports of the body are allocated in text order, which is only known
    // once assembled (the immutable ones come first): deferred marks, resolved
    // at the end.
    let chk = deferred(CHECK_BINDING);
    let x = &h.class;
    let mut fields = String::new();
    let mut body = String::new();
    // The immutable ones (`isImmutable`: a `final` field) come out first, in
    // `if (firstCheck)`, without a field, but with their index (case j103).
    let mut constants = String::new();
    for (k, (name, member)) in h.bindings.iter().enumerate() {
        // `host` (lib.rs) only lets the known shapes through.
        let Ok(mut shape) = host_shape(name) else {
            continue;
        };
        let is_static = h.statics.contains(member);
        if let HostShape::Estilo { text, null, .. } = &mut shape {
            // The static is `dynamic` for `_TypeResolver`: `?.toString()`.
            let ty = if is_static {
                "dynamic"
            } else {
                h.style_types
                    .get(member)
                    .map(String::as_str)
                    .unwrap_or_default()
            };
            *text = ty.trim_end_matches('?') == "String";
            // `isNullable` is the expression's `canBeNull`
            // (`ir/model.dart:609`): outside a literal, always
            // (`analyzed_class.dart:210-221`).
            *null = true;
        }
        if h.immutables.contains(member) || is_static {
            let value = if is_static {
                format!("{own_prefix}.{x}.{member}")
            } else {
                format!("this.instance.{member}")
            };
            let action = shape.action("el", &value);
            let _ = write!(
                constants,
                "      if (({value} != null)) {{\n        {action};\n      }}\n"
            );
            continue;
        }
        let action = shape.action("el", &format!("currVal_{k}"));
        let _ = writeln!(fields, "  Object? _expr_{k};");
        let _ = write!(
            body,
            "    final currVal_{k} = this.instance.{member};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, null, null)) {{\n      {action};\n      this._expr_{k} = currVal_{k};\n    }}\n"
        );
    }
    if !constants.is_empty() {
        body = format!(
            "    bool firstCheck = view.firstCheck;\n    if (firstCheck) {{\n{constants}    }}\n{body}"
        );
    }
    let body = resolve_deferred(imp, &body);
    format!(
        "\nclass {x}NgCd extends {cd}.DirectiveChangeDetector {{\n  final {own_prefix}.{x} instance;\n{fields}  {x}NgCd(this.instance);\n  void detectHostChanges({rv}.RenderView view, {element} el) {{\n{body}  }}\n}}\n"
    )
}

/// Generates a component's fragment in the `.template.dart` — from `styles$X`
/// to the host view factory —, allocating the imports in the file's table. With
/// several components in the file, there is a single table and the fragments
/// come out in source order ([`assemble_file`]), as the official compiler
/// writes them.
///
/// Whatever does not fit comes back as `Err` with the first refusal, and the
/// file keeps coming from `build_runner`.
#[allow(clippy::too_many_arguments)]
pub fn component_fragment(
    c: &Component,
    local: &Local,
    nodes: &[Node],
    resolver: Option<&dyn Resolution>,
    names: &mut dartforge_intern::Interner,
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
    pipes: &Result<Vec<UsedPipe>, Refusal>,
    imp: &mut Imports,
) -> Result<String, Refusal> {
    let mut collected = None;
    let text = generate_component(
        c,
        local,
        nodes,
        resolver,
        names,
        children,
        used,
        pipes,
        &mut collected,
        imp,
    )?;
    // A deferred import mark that no step resolved would end up in the file: it
    // is a shape the emitter does not write yet.
    if text.contains(['\u{1}', '\u{2}']) {
        return Err(refusal(
            Reason::NotUnderstood,
            "unresolved deferred import in the output",
        ));
    }
    Ok(text)
}

/// The `.template.dart` of a file with a single component:
/// [`component_fragment`] with its own import table.
#[allow(clippy::too_many_arguments)]
pub fn component_template(
    c: &Component,
    local: &Local,
    nodes: &[Node],
    resolver: Option<&dyn Resolution>,
    names: &mut dartforge_intern::Interner,
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
    pipes: &Result<Vec<UsedPipe>, Refusal>,
) -> Result<String, Refusal> {
    let mut imp = Imports::default();
    let fragment = component_fragment(
        c, local, nodes, resolver, names, children, used, pipes, &mut imp,
    )?;
    Ok(assemble_file(local.file, &imp, &[fragment]))
}

/// The whole `.template.dart`: the header, the import of itself, the import
/// table and the components' fragments.
pub fn assemble_file(file: &str, imp: &Imports, fragments: &[String]) -> String {
    let mut s = String::with_capacity(4096 * fragments.len().max(1));
    s.push_str(crate::HEADER);
    let _ = writeln!(s, "import '{file}';");
    imp.write_to(&mut s);
    for t in fragments {
        s.push_str(t);
    }
    crate::expr::resolve_breaks(&s)
}

/// All the refusals of this component, not only the first: runs the same
/// emission in collection mode. Without this the scoreboard is misleading — a
/// file that gets stuck on a stylesheet may also get stuck on an event and an
/// interpolation, and counting only the first makes it look as if learning one
/// shape unlocks the file.
#[allow(clippy::too_many_arguments)]
pub fn collect(
    c: &Component,
    local: &Local,
    nodes: &[Node],
    resolver: Option<&dyn Resolution>,
    names: &mut dartforge_intern::Interner,
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
    pipes: &Result<Vec<UsedPipe>, Refusal>,
) -> std::collections::BTreeSet<Refusal> {
    let mut collected = Some(Vec::new());
    let r = generate_component(
        c,
        local,
        nodes,
        resolver,
        names,
        children,
        used,
        pipes,
        &mut collected,
        &mut Imports::default(),
    );
    let mut unsupported: std::collections::BTreeSet<Refusal> =
        collected.unwrap_or_default().into_iter().collect();
    if let Err(r) = r {
        unsupported.insert(r);
    }
    // The stylesheet is another output (`gerar_folha`): the template only
    // imports it.
    if c.style_urls
        .iter()
        .any(|u| local.style_uri(u, !c.without_encapsulation).is_none())
    {
        unsupported.insert(refusal(Reason::Styles, "stylesheet outside lib/"));
    }
    unsupported
}

/// What a component's `@HostBinding` writes on the host element: the bindings
/// of `createElementPropertyAst` with the `div` element
/// (`_securityContextElementName` of `DirectiveConverter`), written by
/// `bindAndWriteToRenderer` with `isHtmlElement` false.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HostShape {
    /// `class.x`: `updateClassBindingNonHtml`.
    Class(String),
    /// `class`, `className`, `attr.class` on a component: the whole class,
    /// `this.updateChildClassNonHtml(this.rootElement, v)` (the unnamed
    /// `ClassBinding`, case j80).
    WholeClass,
    /// `attr.x`, with the sanitizer of the security context: `updateAttribute`.
    Attribute(String, Option<&'static str>),
    /// A static `attr.tabindex`: the element's `tabIndex` property (the
    /// `TabIndexBinding` of `binding_converter.dart`, which only the view
    /// constructor uses; the dynamic binding is still `updateAttribute`, case
    /// j96).
    TabIndex,
    /// `style.x` and `style.x.unit`, with the text of the value decided by the
    /// member type (`visitStyleBinding`: `isString`, `isNullable`).
    Estilo {
        name: String,
        unit: Option<String>,
        text: bool,
        null: bool,
    },
    /// Property (name mapped by `getMappedPropName`), with the sanitizer:
    /// `setProperty`.
    Property(String, Option<&'static str>),
}

/// A component's `@HostBinding`, resolved: the member read, whether it is
/// immutable (`isImmutable`: a `final` field) and the shape.
#[derive(Debug, Clone)]
struct HostBinding {
    member: String,
    immutable: bool,
    /// Static member: written in the view constructor, with the value read from
    /// the class (`X.member`), and outside `detectHostChanges` (case j96).
    is_static: bool,
    shape: HostShape,
}

impl HostBinding {
    /// The statement that writes the value `v` on the host element.
    fn action(&self, v: &str) -> String {
        self.shape.action("this.rootElement", v)
    }
}

impl HostShape {
    /// The statement that writes the value `v` on the element `el` (the
    /// component's `this.rootElement`, the `el` of the directive's `XNgCd`).
    /// The imports go as [`deferred`] marks, in text order.
    pub(crate) fn action(&self, el: &str, v: &str) -> String {
        let dom = deferred(DOM_HELPERS);
        let sanitized = |s: &Option<&'static str>| match s {
            Some(f) => format!("{}.{f}({v})", deferred(SAFE_HTML)),
            None => v.to_string(),
        };
        match self {
            HostShape::Class(x) => {
                format!("{dom}.updateClassBindingNonHtml({el}, '{x}', {v})")
            }
            // Only the component gets here
            // ([`component_host_shape`]): the receiver is its
            // view.
            HostShape::WholeClass => {
                format!("this.updateChildClassNonHtml({el}, {v})")
            }
            HostShape::Attribute(x, s) => {
                format!("{dom}.updateAttribute({el}, '{x}', {})", sanitized(s))
            }
            HostShape::TabIndex => format!("{el}.tabIndex = {v}"),
            HostShape::Property(x, s) => {
                format!("{dom}.setProperty({el}, '{x}', {})", sanitized(s))
            }
            HostShape::Estilo {
                name,
                unit,
                text,
                null,
            } => {
                let value = match unit {
                    Some(u) => {
                        let t = if *text {
                            v.to_string()
                        } else {
                            format!("{v}.toString()")
                        };
                        format!(
                            "(({v} == null) ? {} : ({t} + {}))",
                            null_style(),
                            literal(u)
                        )
                    }
                    None => unitless_style(v, *text, *null),
                };
                format!("{el}.style.setProperty('{name}', {value})")
            }
        }
    }
}

/// The shape of a `@HostBinding` name (`createElementPropertyAst`): whatever is
/// not `class.x`, `attr.x`, `style.x[.unit]` or a simple property is refused —
/// `class`/`className` (the whole class), `attr.x.if`, a namespace and an
/// unknown prefix have no case yet.
pub(crate) fn host_shape(name: &str) -> Result<HostShape, String> {
    let simple = |n: &str| !n.is_empty() && !n.contains(['.', ':']);
    let unsupported =
        || format!("@HostBinding('{name}') outside class.x, attr.x, style.x and property");
    let parts: Vec<&str> = name.split('.').collect();
    Ok(match parts.as_slice() {
        // `class.x.y`: only the second part counts (`boundPropertyName =
        // parts[1]`, `template_parser.dart:100-102`), the rest is ignored (the
        // `class.basic-icon.if` of `material_icon_toggle`).
        ["class", x, ..] if simple(x) => HostShape::Class(x.to_string()),
        ["attr", x] if simple(x) => {
            HostShape::Attribute(x.to_string(), sanitizer("div", mapped_property(x)))
        }
        ["style", x] | ["style", x, _] if simple(x) => HostShape::Estilo {
            name: x.to_string(),
            unit: parts.get(2).map(|u| u.to_string()),
            text: false,
            null: false,
        },
        [p] if simple(p) => {
            let mapped = mapped_property(p);
            if mapped == "className" {
                return Err(unsupported());
            }
            HostShape::Property(mapped.to_string(), sanitizer("div", mapped))
        }
        _ => return Err(unsupported()),
    })
}

/// [`host_shape`] for the component, which also writes the whole class
/// (`class`, `className`, `attr.class`) through its view.
fn component_host_shape(name: &str) -> Result<HostShape, String> {
    if matches!(name, "class" | "className" | "attr.class") {
        return Ok(HostShape::WholeClass);
    }
    host_shape(name)
}

/// The component's `@HostBinding`s in the order of the official
/// `hostProperties` map. Without inheritance, those of the class itself
/// ([`Component`]); with inheritance, those of the metadata read from the
/// program (`metadata.rs`: supertypes first, the key is the name and the last
/// one wins in the position of the first), with the immutability of the
/// inherited members asked of the resolver. `style.x` needs the member type
/// (`isString`, `isNullable`).
///
/// # Errors
///
/// The refusal of the shape that cannot be written yet: a name outside the
/// known shapes, inheritance without the program metadata, an unreadable
/// inherited member, `style.x` on a `final` field or of an unknown type.
fn component_bindings(
    c: &Component,
    local: &Local,
    resolver: Option<&dyn Resolution>,
) -> Result<Vec<HostBinding>, Refusal> {
    let unsupported = |f: &str| refusal(Reason::HostBindingOnComponent, f.to_string());
    let list: Vec<(String, String)> = if c.inherits {
        match local.metadata.as_deref() {
            Some(m) if m.unsupported.is_empty() => m.host_bindings.clone(),
            _ if c.binds_host => {
                return Err(unsupported("@HostBinding on an inheriting component"));
            }
            _ => Vec::new(),
        }
    } else {
        c.host_bindings
            .iter()
            .map(|l| (l.name.clone(), l.member.clone()))
            .collect()
    };
    let mut output = Vec::new();
    for (name, member) in list {
        let mut shape = component_host_shape(&name).map_err(|f| unsupported(&f))?;
        let own_binding = c.host_bindings.iter().find(|l| l.member == member);
        let immutable = match own_binding {
            Some(l) => l.immutable,
            None => resolver
                .and_then(|r| r.final_member(local.path, &c.class, &member))
                .ok_or_else(|| {
                    unsupported("inherited @HostBinding without a readable declaration")
                })?,
        };
        if let HostShape::Estilo { text, null, .. } = &mut shape {
            if immutable {
                return Err(unsupported("@HostBinding('style.x') on a final field"));
            }
            let ty = resolver
                .and_then(|r| r.member_type(local.path, &c.class, &member))
                .map(|(t, _)| t)
                .or_else(|| c.members.get(&member).map(|m| m.ty.clone()))
                .unwrap_or_default();
            let base = ty.trim().trim_end_matches('?');
            if base.is_empty() || matches!(base, "dynamic" | "var" | "Object" | "Never") {
                return Err(unsupported("@HostBinding('style.x') of unknown type"));
            }
            *text = base == "String";
            // `canBeNull` de `_ctx.x`: sempre (`analyzed_class.dart:210-221`).
            *null = true;
        }
        output.push(HostBinding {
            member,
            immutable,
            is_static: own_binding.is_some_and(|l| l.is_static),
            shape: match shape {
                HostShape::Attribute(x, _)
                    if own_binding.is_some_and(|l| l.is_static)
                        && matches!(x.as_str(), "tabindex" | "tabIndex") =>
                {
                    HostShape::TabIndex
                }
                f => f,
            },
        });
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
/// The emission of a component. The `text_binding.dart` import is allocated
/// before the body, at the position of the class's first `TextBinding` field;
/// when no interpolation of the root view is mutable (all in `build()`,
/// `isImmutable`), the field does not exist and the emission is redone without
/// it, from the same import table (case j91).
#[allow(clippy::too_many_arguments)]
fn generate_component(
    c: &Component,
    local: &Local,
    nodes: &[Node],
    resolver: Option<&dyn Resolution>,
    names: &mut dartforge_intern::Interner,
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
    pipes: &Result<Vec<UsedPipe>, Refusal>,
    collected: &mut Option<Vec<Refusal>>,
    imp: &mut Imports,
) -> Result<String, Refusal> {
    let (imp_before, collected_before) = (imp.clone(), collected.clone());
    let (text, tb_leftover) = generate_component_with(
        c, local, nodes, resolver, names, children, used, pipes, collected, imp, true,
    )?;
    if !tb_leftover {
        return Ok(text);
    }
    *imp = imp_before;
    *collected = collected_before;
    generate_component_with(
        c, local, nodes, resolver, names, children, used, pipes, collected, imp, false,
    )
    .map(|(t, _)| t)
}

#[allow(clippy::too_many_arguments)]
fn generate_component_with(
    c: &Component,
    local: &Local,
    nodes: &[Node],
    resolver: Option<&dyn Resolution>,
    names: &mut dartforge_intern::Interner,
    children: &std::collections::HashMap<String, Child>,
    used: &[UsedDirective],
    pipes: &Result<Vec<UsedPipe>, Refusal>,
    collected: &mut Option<Vec<Refusal>>,
    imp: &mut Imports,
    with_tb: bool,
) -> Result<(String, bool), Refusal> {
    let nodes = &template_as_container(nodes);
    // Records in collection mode or stops.
    fn record_refusal(collected: &mut Option<Vec<Refusal>>, r: Refusal) -> Result<(), Refusal> {
        match collected {
            Some(v) => {
                v.push(r);
                Ok(())
            }
            None => Err(r),
        }
    }
    for r in &c.not_understood {
        record_refusal(collected, r.clone())?;
    }
    for r in shapes_against_template(c, local, nodes, resolver, children, used) {
        record_refusal(collected, r)?;
    }
    // Unused `pipes:` do not change the view (case b19).
    let table = match template_pipes(nodes, children, pipes, &local.asset()) {
        Ok(t) => t,
        Err(r) => {
            record_refusal(collected, r)?;
            TemplatePipes::default()
        }
    };
    if c.unreadable_styles {
        record_refusal(
            collected,
            refusal(
                Reason::Styles,
                "styleUrls/styles with an item that is not a literal string",
            ),
        )?;
    }
    // `styles: ['…']` written in the annotation: each text goes into the
    // `styles$X` list after the `styleUrls` stylesheets (`_compileStyles`),
    // shimmed under emulated encapsulation and as is under `none` (cases j07,
    // j08).
    let mut inline = Vec::new();
    for text in &c.styles {
        if text.contains("@import") {
            record_refusal(
                collected,
                refusal(Reason::Styles, "styles: [..] with @import"),
            )?;
            continue;
        }
        if c.without_encapsulation {
            inline.push(literal(text));
            continue;
        }
        match crate::css::shim(text) {
            Ok(t) => inline.push(literal(&t)),
            Err(_) => record_refusal(
                collected,
                refusal(
                    Reason::Styles,
                    "CSS the ngdart shim refuses (the official compiler throws)",
                ),
            )?,
        }
    }
    // `ViewEncapsulation.none` with a stylesheet: the template imports the
    // `.css.dart` (the stylesheet without the shim, from `gerar_folha`),
    // `ComponentStyles.unscoped` and no `addShimC` (case i88).
    // The component's `@HostBinding`s, with the inherited ones.
    let of_host = match component_bindings(c, local, resolver) {
        Ok(l) => l,
        Err(r) => {
            record_refusal(collected, r)?;
            Vec::new()
        }
    };
    // The construction comes out after the fixed imports, because injection
    // allocates its own (`errors.dart` and that of each injected type) at the
    // end of the table.
    if let Some(r) = missing_for_construction(c, local, resolver) {
        record_refusal(collected, r)?;
    }
    // The component's `providers:`: the host view creates them; with
    // `Visibility.all`, it also hands the component out through
    // `injectorGetInternal`.
    if c.written_visibility && local.metadata.is_none() {
        record_refusal(
            collected,
            refusal(
                Reason::Providers,
                "visibility: without the program metadata",
            ),
        )?;
    }
    let visible = local.metadata.as_ref().is_some_and(|m| m.visible);
    let on_host = if c.with_providers || visible {
        match host_providers(local) {
            Ok(r) => Some(r),
            Err(r) => {
                record_refusal(collected, r)?;
                None
            }
        }
    } else {
        None
    };
    let node_tokens: Vec<crate::directives::Token> = on_host
        .iter()
        .flat_map(|r| &r.instances)
        .flat_map(|i| std::iter::once(i.token.clone()).chain(i.aliases.iter().cloned()))
        .collect();
    let host_queries_text = match host_queries(c, local, resolver, &node_tokens) {
        Ok(s) => s,
        Err(r) => {
            record_refusal(collected, r)?;
            String::new()
        }
    };

    // The compiled stylesheets, in `styleUrls` order: each one allocates its
    // import when the `styles$X` list is written — the file's first one is the
    // first import; in a file with several components, that of a later
    // component goes into the middle of the table, and a repeated one reuses
    // the import (case j44). The stylesheet goes in by the `package:` URI even
    // when it is next to the file: that is how the official compiler writes it
    // (the `styleUrls` resolver is a different one, and does not go through the
    // relative path).
    let mut style = Vec::new();
    for url in &c.style_urls {
        match local.style_uri(url, !c.without_encapsulation) {
            Some(uri) => style.push(imp.alias(&uri)),
            None => record_refusal(
                collected,
                refusal(Reason::Styles, "stylesheet outside lib/"),
            )?,
        }
    }
    // Generic component: the view, the host view, the embedded views and the
    // factories take the class's type parameters. The written bound (`T extends
    // num`) is the first type in the header of the view class: its import is
    // allocated before `ComponentView`.
    let (generics, generics_decl) = match type_params(c, local, resolver, imp) {
        Ok(g) => g,
        Err(r) => {
            record_refusal(collected, r)?;
            Default::default()
        }
    };
    let component_view_prefix = imp.alias(COMPONENT_VIEW);
    let own_prefix = imp.alias(local.file);
    // The view's fields come before everything in the class — text bindings,
    // child views, previous values, elements —, and the imports are allocated
    // in that same order. That is what makes the numbering match the official
    // one; out of order, the byte-for-byte comparison is worthless.
    // `@i18n`: `package:intl` goes in with the first `_message_N` field, before
    // or after `text_binding.dart` according to document order. With a child,
    // directive or `*` in the view, the order of the fields (and the imports)
    // among them has no case yet.
    allocate_lazy_imports(imp, nodes, children, used, &local.asset());
    let i18n_order = i18n_before_interpolation(nodes);
    let mut intl = None;
    if i18n_order == Some(true) {
        intl = Some(imp.alias(INTL));
    }
    let tb = (with_tb && has_interpolation(nodes)).then(|| imp.alias(TEXT_BINDING));
    if i18n_order == Some(false) {
        intl = Some(imp.alias(INTL));
    }
    if i18n_order.is_some() && !fields_in_order(nodes, children, used, &local.asset()).is_empty() {
        record_refusal(
            collected,
            refusal(
                Reason::I18n,
                "@i18n with a child, directive or `*` in the view",
            ),
        )?;
    }
    let candidate_refs = candidate_refs(nodes, c);
    let local_refs = local_refs(nodes, children, &candidate_refs, &[]);
    // The node of this view that is the result of a dynamic query is read
    // during detection (the list `[this._el_0, ...]`): it becomes a field, like
    // the one an expression reads (case j17).
    let root_queried: Vec<(String, String, u32)> = c
        .queries
        .iter()
        .enumerate()
        .filter(|(_, q)| !q.by_type && query_in_embedded(nodes, q, children, local, resolver))
        .filter(|(_, q)| {
            query_tree(nodes, &q.reference, children).is_ok_and(|a| has_static_result(&a))
        })
        .map(|(i, q)| {
            (
                q.reference.clone(),
                format!("_viewQuery_{}_{i}_isDirty", q.reference),
                0,
            )
        })
        .collect();
    // `directiveTypes:`: the class in the component's scope and the arguments
    // written like `fromTypeLink` (`#T` is the component's parameter).
    let directive_types = {
        let free: Vec<&str> = c.type_params.iter().map(|(n, _)| n.as_str()).collect();
        let mut v = Vec::new();
        for t in &c.directive_types {
            let fails = || {
                refusal(
                    Reason::NotUnderstood,
                    format!("directiveTypes: Typed<{}> without resolution", t.class),
                )
            };
            let r = resolver.ok_or_else(fails)?;
            let uri = r.type_uri(local.path, &t.class).ok_or_else(fails)?;
            let simple = t.class.rsplit('.').next().unwrap_or(&t.class).to_string();
            if !used.iter().any(|u| u.uri == uri && u.class == simple) {
                return Err(refusal(
                    Reason::NotUnderstood,
                    "directiveTypes: with a directive outside directives: (error in the official compiler)",
                ));
            }
            let mut written = Vec::new();
            for a in &t.arguments {
                written.push(
                    qualified_type(a, local.path, r, &local.asset(), &free).ok_or_else(fails)?,
                );
            }
            v.push(ResolvedDirectiveType {
                uri,
                class: simple,
                arguments: if written.is_empty() {
                    String::new()
                } else {
                    format!("<{}>", written.join(", "))
                },
                em: t.em.clone(),
            });
        }
        v
    };
    let mut promoted = local_refs.clone();
    promoted.extend(root_queried.iter().map(|(n, _, _)| n.clone()));
    if let Err(r) = allocate_field_imports(
        imp,
        nodes,
        children,
        used,
        &local.asset(),
        &table.field_imports(0),
        &promoted_nodes(nodes, &promoted),
        &directive_types,
    ) {
        record_refusal(collected, r)?;
    }
    let styles = imp.alias(STYLE_ENCAPSULATION);
    let view = imp.alias(VIEW);
    let cd = imp.alias(CHANGE_DETECTION);
    let util = imp.alias(UTILITIES);
    // The view constructor uses `document.createElement`, so `dart:html` (in 9,
    // the `dom.dart` of `package:web`) always goes in before the body of
    // `build()`.
    let document = imp.dom("document");
    // The static `@HostBinding`s come out in the constructor, right after
    // `rootElement`: their imports come before those of `build()`.
    let statics_in_constructor: String = of_host
        .iter()
        .filter(|l| l.is_static)
        .map(|l| {
            let value = format!("{own_prefix}.{}.{}", c.class, l.member);
            format!("\n    {};", resolve_deferred(imp, &l.action(&value)))
        })
        .collect();

    // `exports:`: each name through the import of the library that declares it,
    // allocated when the expression is written (case j51).
    let mut exports = Exports::new();
    if let Some(r) = resolver {
        for name in c.exports.iter().filter(|n| !n.contains('.')) {
            let Some((uri, what)) = r.exported(local.path, name) else {
                continue;
            };
            let Some(path) = asset_of_uri(&uri, local.package, local.root)
                .and_then(|a| import_path(&local.asset(), &a))
            else {
                continue;
            };
            exports.insert(name.clone(), (deferred_q(&path), what));
        }
    }
    let ctx = Context {
        members: &c.members,
        exports: &exports,
        generic_names: c.type_params.iter().map(|(n, _)| n.clone()).collect(),
        view_tokens: local
            .metadata
            .as_deref()
            .map(|m| m.view_providers.iter().map(|p| p.token.clone()).collect())
            .unwrap_or_default(),
        methods: &c.methods,
        arities: &c.arities,
        children,
        used,
        asset: local.asset(),
        types: resolver.map(|r| (r, local.path)),
        qualified_class: format!("{own_prefix}.{}", c.class),
        with_style: (!c.style_urls.is_empty() || !c.styles.is_empty()) && !c.without_encapsulation,
        template_url: local.template_url.clone(),
        view_class: format!("View{}", c.class),
        context_type: format!("{own_prefix}.{}{generics}", c.class),
        generics,
        generics_decl,
        preserve_whitespace: c.preserve_whitespace,
        pipes: &table,
        ambiguous_refs: ambiguous_refs(nodes),
        candidate_refs,
        reads: c
            .queries
            .iter()
            .filter_map(|q| {
                Some((
                    query_key(q, local, resolver)?,
                    read_token(q, local, resolver)?,
                ))
            })
            .collect(),
        provider_only_refs: c
            .queries
            .iter()
            .filter(|q| !q.by_type)
            .map(|q| q.reference.clone())
            .filter(|r| {
                c.queries
                    .iter()
                    .filter(|q| !q.by_type && q.reference == *r)
                    .all(|q| read_token(q, local, resolver).is_some())
            })
            .collect(),
        templates_with_container: {
            let templates = template_refs(nodes);
            c.queries
                .iter()
                .filter(|q| {
                    !q.by_type
                        && templates.contains(&q.reference)
                        && reads_container(q, local, resolver)
                })
                .map(|q| q.reference.clone())
                .collect()
        },
        detection_link: c.detection_link,
        directive_types,
        resolved_refs: Default::default(),
        query_anchors: Default::default(),
        dynamic_content: Default::default(),
        dirty_content_queries: Default::default(),
    };
    let mut body = ctx.body(imp, names, collected.take(), false);
    body.free_refs = free_refs(nodes);
    body.embedded_refs = local_refs
        .iter()
        .filter(|n| referenced_in_embedded_views(nodes, n, children))
        .cloned()
        .collect();
    body.event_only_refs = local_refs
        .iter()
        .filter(|n| !read_in_detection(nodes, n))
        .cloned()
        .collect();
    body.declare_refs(local_refs);
    body.queried_refs = root_queried;
    body.dynamic_queries = c
        .queries
        .iter()
        .enumerate()
        .filter(|(_, q)| query_in_embedded(nodes, q, children, local, resolver))
        .map(|(i, q)| DynamicQuery {
            index: i,
            property: q.property.clone(),
            list: q.list,
            key: query_key(q, local, resolver).unwrap_or_default(),
            read: {
                let key = query_key(q, local, resolver).unwrap_or_default();
                match read_token(q, local, resolver) {
                    Some(t) => read_key(&key, &t),
                    None => key,
                }
            },
            field: format!("_viewQuery_{}_{i}_isDirty", q.reference),
            element_ref: q.read.is_some()
                && element_value(q, local, resolver) == Some(ElementValue::ElementRef),
            tree: query_key(q, local, resolver)
                .and_then(|k| query_tree(nodes, &k, children).ok())
                .unwrap_or_default(),
            view: false,
        })
        .collect();
    // The "dirty" fields open the class, in the order the official compiler
    // allocates them ([`first_dynamic_result`]; on a tie, the order of
    // the queries).
    let mut dirty: Vec<(Option<(usize, usize)>, usize, &str)> = body
        .dynamic_queries
        .iter()
        .map(|d| {
            (
                first_dynamic_result(nodes, &d.key, children),
                d.index,
                d.field.as_str(),
            )
        })
        .collect();
    dirty.sort_by_key(|(position, index, _)| (position.is_none(), *position, *index));
    let dirty: Vec<(usize, String)> = dirty
        .into_iter()
        .map(|(_, _, field)| {
            let position = body
                .dynamic_queries
                .iter()
                .find(|d| d.field == field)
                .and_then(|d| first_dynamic_result_start(nodes, &d.key, children))
                .unwrap_or(usize::MAX);
            (position, format!("  bool {field} = true;"))
        })
        .collect();
    body.view_dirty_fields = dirty;
    // `@ViewChild(ren)(Token)` provided by a directive, with a result in a `*`:
    // through the content query machinery, with the root in this view.
    for (i, q) in c.queries.iter().enumerate() {
        let Some(tree) = dynamic_token_query(nodes, q, children, used, local, resolver) else {
            continue;
        };
        let (Some(uri), Some(class)) = (
            query_uri(q, local, resolver),
            q.reference.rsplit('.').next().map(str::to_string),
        ) else {
            continue;
        };
        body.dynamic_content_query(
            tree,
            type_key(&uri, &class),
            &class,
            0,
            i,
            q.list,
            format!("_ctx.{}", q.property),
            true,
        );
        body.token_queries.insert(i);
    }
    // Only messages with HTML: `intl` is requested by the method, later.
    body.intl = intl.or_else(|| has_annotation(nodes).then(String::new));
    body.tb = tb;
    let r = body
        .nodes(nodes, "parentRenderNode")
        .and_then(|()| body.check_pipes());
    if let Err(r) = r {
        *collected = body.collected.take();
        return Err(r);
    }
    export_refs(
        &ctx.resolved_refs,
        &body.refs,
        &body.detectors,
        &body.refs_in_order,
        &body.detectors_in_order,
        &body.this_view_class(),
    );
    // The pre-allocated `text_binding.dart` import without a field that uses
    // it.
    let tb_leftover = body.tb.is_some() && !body.tb_used && !body.collecting();
    // The pipes' `ngOnDestroy` comes after those of the directives.
    body.destroy.extend(table.destruction(0));
    // A static `@ViewChild`: immediate assignment, in `afterNodes` — after the
    // listeners, in the declaration order of the queries
    // (`updateQueryAtStartup`, `createImmediateUpdates` in
    // `compile_query.dart`). `shapes_against_template` has already ensured
    // that each `#ref` appears only once, on an HTML element of the view
    // itself.
    let templates = template_refs(nodes);
    let mut queries = Vec::new();
    for (i, q) in c.queries.iter().enumerate() {
        // The dynamic one comes out in the detection.
        if body.dynamic_queries.iter().any(|d| d.index == i) || body.token_queries.contains(&i) {
            continue;
        }
        // What to look for among the results: the `#ref`, or the type key.
        let key = if q.by_type {
            query_uri(q, local, resolver)
                .map(|u| type_key(&u, &q.reference))
                .unwrap_or_default()
        } else {
            q.reference.clone()
        };
        // With a provider `read:`, its instance on each node; with `read:
        // ViewContainerRef` on a `<template>`, its `ViewContainer`.
        let key = match read_token(q, local, resolver) {
            Some(t) => read_key(&key, &t),
            None if !q.by_type
                && templates.contains(&q.reference)
                && reads_container(q, local, resolver) =>
            {
                container_key(&key)
            }
            None => key,
        };
        if q.list {
            let values: Vec<String> = body
                .refs_in_order
                .iter()
                .filter(|(n, _)| *n == key)
                .map(|(_, v)| v.clone())
                .collect();
            // An `onPush` child in the result would register the
            // `ChangeDetectorRef` of each one: not yet.
            if body.detectors.contains_key(&key) {
                let r = refusal(Reason::ViewChildOnChild, "@ViewChildren of an onPush child");
                if body.collecting() {
                    body.record_refusal(r)?;
                } else {
                    *collected = body.collected.take();
                    return Err(r);
                }
            }
            let values: Vec<String> = values
                .into_iter()
                .map(|v| element_with_cast(v, q, local, resolver))
                .collect();
            queries.push(format!(
                "    _ctx.{} = [{}];",
                q.property,
                values.join(", ")
            ));
            body.uses_ctx_in_build = true;
            continue;
        }
        // The first result in document order (repeated `#ref`).
        let first = body
            .refs_in_order
            .iter()
            .find(|(n, _)| *n == key)
            .map(|(_, v)| v.clone());
        match first {
            Some(target) => {
                // An `onPush` child: its `ChangeDetectorRef` is registered
                // (`_createAddQueryChangeDetectorRefs`). With a repeated
                // `#ref`, the registration kept may not be that of the first:
                // not yet.
                let repeated = body.refs_in_order.iter().filter(|(n, _)| *n == key).count() > 1;
                if repeated && body.detectors.contains_key(&key) {
                    let r = refusal(
                        Reason::ViewChildOnChild,
                        "@ViewChild of a repeated #ref on an onPush child",
                    );
                    if body.collecting() {
                        body.record_refusal(r)?;
                    } else {
                        *collected = body.collected.take();
                        return Err(r);
                    }
                }
                if let Some(cv) = body.detectors.get(&key).cloned() {
                    let v = body.imp.alias(VIEW);
                    queries.push(format!(
                        "    {v}.View.queryChangeDetectorRefs[{target}] = this.{cv};"
                    ));
                }
                // `ElementRef(node)` for the element read like this; the import
                // comes out in text order, after the listeners and the pipes.
                let mut places = Vec::new();
                where_is(nodes, &q.reference, children, false, &mut places);
                let element = matches!(places.first(), Some(Place::Root | Place::Projected));
                // With `#ref="x"`, the value is the exported instance, not the
                // node.
                let target = if !q.by_type
                    && element
                    && !templates.contains(&q.reference)
                    && !ref_with_value(nodes, &q.reference)
                    && element_value(q, local, resolver) == Some(ElementValue::ElementRef)
                {
                    format!("{}ElementRef({target})", deferred_q(ELEMENT_REF))
                } else {
                    element_with_cast(target, q, local, resolver)
                };
                queries.push(format!("    _ctx.{} = {target};", q.property));
            }
            // In collection mode the refusal already came from
            // `shapes_against_template`.
            None if body.collecting() => {}
            // No `#ref` with that name in the template: the single one receives
            // nothing (case j82).
            // Without an assignment, `build()` does not read `_ctx` for it.
            None if !q.by_type && {
                let mut l = Vec::new();
                where_is(nodes, &q.reference, children, false, &mut l);
                l.is_empty()
            } =>
            {
                continue;
            }
            None => {
                return Err(refusal(
                    Reason::DynamicViewChild,
                    "@ViewChild without a #ref in the template",
                ));
            }
        }
        body.uses_ctx_in_build = true;
    }
    // The element listeners are written after the nodes: their imports go in
    // now.
    let listeners: Vec<String> = body
        .listeners
        .iter()
        .map(|o| resolve_deferred(body.imp, o))
        .collect();
    // The pipes: instance and proxies, in `afterNodes`, before the queries.
    let pipe_creation: Vec<String> = table
        .creation(0, "this", body.imp)
        .iter()
        .map(|l| resolve_deferred(body.imp, l))
        .collect();
    let queries: Vec<String> = queries
        .iter()
        .map(|l| resolve_deferred(body.imp, l))
        .collect();
    // The component's `@HostListener`s close `build()`, bound to the root node
    // (`_writeComponentHostEventListeners`, after `writeBuildStatements` in
    // `_generateBuildMethod`). The handler goes through the same converter as
    // the template events, and a complex one gets the next `_handleEvent_N`.
    // For an inheriting component: those of the supertypes too, from the
    // metadata (supertypes first; the same event keeps the last one, in the
    // place of the first).
    let host_listeners: Vec<crate::component::Listener> = if c.inherits {
        match local.metadata.as_deref() {
            Some(m) if m.unsupported.is_empty() => {
                if m.listeners.iter().any(|o| !native_event(&o.event)) {
                    body.record_refusal(refusal(
                        Reason::HostListenerOnComponent,
                        "inherited @HostListener of a non-native event",
                    ))?;
                }
                m.listeners
                    .iter()
                    .map(|o| crate::component::Listener {
                        event: o.event.clone(),
                        handler: format!("{}({})", o.method, o.args),
                    })
                    .collect()
            }
            _ => {
                body.record_refusal(refusal(
                    Reason::HostListenerOnComponent,
                    "@HostListener of an inheriting component without the metadata",
                ))?;
                c.listeners.clone()
            }
        }
    } else {
        c.listeners.clone()
    };
    let mut host = Vec::new();
    for o in &host_listeners {
        body.event_type_failure = None;
        body.event_type = native_event_type_q(&o.event);
        match body.handler(&[&o.handler]) {
            // The `package:web` marks in 9, in text order.
            Ok(h) => host.push(resolve_deferred(
                body.imp,
                &format!(
                    "    parentRenderNode.addEventListener('{}', {});",
                    o.event,
                    if native_event(&o.event) {
                        native_listener(&o.event, &h)
                    } else {
                        h.clone()
                    }
                ),
            )),
            Err(r) => {
                let r = r.em(Reason::HostListenerOnComponent);
                if body.collecting() {
                    body.record_refusal(r)?;
                } else {
                    *collected = body.collected.take();
                    return Err(r);
                }
            }
        }
    }
    let ctx_in_build = if body.uses_ctx_in_build {
        "\n    final _ctx = this.ctx;"
    } else {
        ""
    };
    // Order of the official `build()` (`_generateBuildMethod`): the nodes (the
    // `_buildView` phase), the listeners (`bindView`) and, finally, what
    // `afterNodes` adds.
    let lines = body
        .lines
        .iter()
        .chain(&listeners)
        .chain(&pipe_creation)
        .chain(&queries)
        .cloned()
        .chain((body.subscriptions > 0).then(|| {
            let subs: Vec<String> = (0..body.subscriptions)
                .map(|k| format!("subscription_{k}"))
                .collect();
            format!("    this.initSubscriptions([{}]);", subs.join(", "))
        }))
        // The component's `@HostListener`s after `initSubscriptions`
        // (`_writeComponentHostEventListeners`, `view_builder.dart:840-852`).
        .chain(host.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n");
    let build_body = if lines.is_empty() {
        String::new()
    } else {
        format!("\n{lines}")
    };
    // The component's `@HostBinding`: the view's
    // `detectHostChanges(firstCheck)` (`bindAndWriteToRenderer` with
    // `isHtmlElement` false — hence `updateClassBindingNonHtml`), with the
    // binding indices after those of the template and the immutable ones first,
    // in `if (firstCheck)`. `checkBinding` takes `null, null`: the binding has
    // no template text.
    let host_changes = if !of_host.iter().any(|l| !l.is_static) {
        String::new()
    } else {
        let chk = deferred(CHECK_BINDING);
        let mut constants = Vec::new();
        let mut dynamic = Vec::new();
        for l in of_host.iter().filter(|l| !l.is_static) {
            let k = body.next_binding;
            body.next_binding += 1;
            let m = &l.member;
            if l.immutable {
                constants.push(format!(
                    "      if ((_ctx.{m} != null)) {{\n        {};\n      }}",
                    l.action(&format!("_ctx.{m}"))
                ));
            } else {
                body.expr_fields.push(format!("  Object? _expr_{k};"));
                dynamic.push(format!(
                    "    final currVal_{k} = _ctx.{m};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, null, null)) {{\n      {};\n      this._expr_{k} = currVal_{k};\n    }}",
                    l.action(&format!("currVal_{k}"))
                ));
            }
        }
        let mut lines = vec!["    final _ctx = this.ctx;".to_string()];
        if !constants.is_empty() {
            lines.push(format!(
                "    if (firstCheck) {{\n{}\n    }}",
                constants.join("\n")
            ));
        }
        lines.extend(dynamic);
        format!(
            "\n  void detectHostChanges(bool firstCheck) {{\n{}\n  }}\n",
            lines.join("\n")
        )
    };
    // Order of the fields in the class, as the official compiler writes it:
    // text bindings, then the previous values of the bindings, then the
    // elements.
    let specs = std::mem::take(&mut body.embedded_views);
    let mut all = initialized_fields(&body);
    all.extend(body.fields.clone());
    all.extend(body.child_fields.clone());
    all.extend(detection_fields(
        &body.expr_fields,
        &body.embedded_el_fields,
        &body.embedded_position,
    ));
    all.extend(table.fields(0, body.imp));
    let mut queried = body.queried_el_fields.clone();
    queried.sort_by_key(|(k, _)| *k);
    all.extend(queried.into_iter().map(|(_, c)| c));
    all.extend(el_fields_in_order(
        &body.el_fields,
        &body.el_field_refs,
        &body.root_locals,
    ));
    all.extend(body.el_fields_by_event.clone());
    all.extend(body.event_el_fields.iter().map(|(_, c)| c.clone()));
    let fields = if all.is_empty() {
        String::new()
    } else {
        format!("{}\n", all.join("\n"))
    };
    // The block of the content hooks (`_updateContentQueriesMethod`): the
    // dynamic content queries, in each node's `afterChildren`; the view ones,
    // in `afterNodes` (`compile_view.dart:502-511`), in query order; then the
    // hooks.
    let mut after_content = body.content_updates.clone();
    let mut of_view: Vec<(usize, String)> = body.view_updates.clone();
    for d in &body.dynamic_queries {
        if !d.view {
            continue;
        }
        // The value can only be assembled with the anchors of all views, after
        // the nested ones are emitted ([`resolve_queries`]); the qualifiers
        // go in the mark, in the order they appear in the text.
        let mut mark = format!("{QUERY_MARK}{}", d.field);
        if !d.list && !has_static_result(&d.tree) {
            mark += &format!("|q{}", deferred_q(QUERIES));
        }
        if has_detector(&d.tree) {
            mark += &format!("|d{}", deferred_q(VIEW));
        }
        if d.element_ref {
            mark += &format!("|e{}", deferred_q(ELEMENT_REF));
        }
        mark.push(QUERY_END);
        of_view.push((
            d.index,
            format!(
                "    if (this.{field}) {{\n      {mark}\n      this.{field} = false;\n    }}",
                field = d.field
            ),
        ));
    }
    of_view.sort_by_key(|(i, _)| *i);
    after_content.extend(of_view.into_iter().map(|(_, l)| l));
    after_content.extend(body.after_content.iter().cloned());
    let queries_body = body.dynamic_queries.clone();
    let root_class = body.this_view_class();
    // The detection in the order of `writeChangeDetectionStatements`: inputs of
    // directives and children, nested views, property and text bindings, child
    // views. `_ctx` and `firstCheck` are only declared if someone reads them
    // (`maybeCachedCtxDeclarationStatement`).
    let mut detection_lines = body.inputs.clone();
    for a in &body.anchors {
        detection_lines.push(format!("    this.{a}.detectChangesInNestedViews();"));
    }
    detection_lines.extend(without_throw(&after_content));
    detection_lines.extend(body.detection.iter().cloned());
    for v in &body.child_views {
        detection_lines.push(format!("    this.{v}.detectChanges();"));
    }
    detection_lines.extend(without_throw(&body.after_view));
    let detection = if detection_lines.is_empty() {
        String::new()
    } else {
        // The dynamic query writes `_ctx.x = ...` when it is resolved
        // ([`resolve_queries`]), after this computation.
        let ctx_det = if references_ctx(&detection_lines)
            || (body.dynamic_queries.iter().any(|d| d.view) || !body.view_updates.is_empty())
        {
            "    final _ctx = this.ctx;\n"
        } else {
            ""
        };
        let first = if body.uses_first_check {
            "    bool firstCheck = this.firstCheck;\n"
        } else {
            ""
        };
        let changed = if body.uses_changed {
            "    bool changed = false;\n"
        } else {
            ""
        };
        // The locals read by the detection (the `#ref`s), in order of first
        // use, after `_ctx`, as in the embedded view.
        let locals: String = body
            .root_locals
            .iter()
            .filter_map(|name| match body.local_decls.get(name) {
                Some(Ok(d)) => Some(format!("    {d}\n")),
                _ => None,
            })
            .collect();
        format!(
            "\n  @override\n  void detectChangesInternal() {{\n{ctx_det}{changed}{first}{locals}{}\n  }}\n",
            detection_lines.join("\n")
        )
    };
    let detection = resolver_refs(&detection, &ctx.resolved_refs.borrow());
    // `injectorGetInternal` comes after `build()` and before the detection.
    let injector = resolve_deferred(body.imp, &injector_method(&body.injectors, &body.asset));
    // The imports of the detection go in now, after those of `build()`.
    let detection = resolve_deferred(body.imp, &detection);
    let detection = format!(
        "{}{detection}",
        link_method(ctx.detection_link, &body.anchors, &body.linked_views)
    );
    // A child view must be destroyed with the view that created it.
    let destruction =
        if body.child_views.is_empty() && body.anchors.is_empty() && body.destroy.is_empty() {
            String::new()
        } else {
            let mut lines: Vec<String> = body
                .anchors
                .iter()
                .map(|a| format!("    this.{a}.destroyNestedViews();"))
                .collect();
            lines.extend(
                body.child_views
                    .iter()
                    .map(|v| format!("    this.{v}.destroyInternalState();")),
            );
            lines.extend(body.destroy.iter().cloned());
            format!(
                "\n  @override\n  void destroyInternal() {{\n{}\n  }}\n",
                lines.join("\n")
            )
        };
    // The `_handleEvent_N`s, after `destroyInternal`.
    if !body.i18n_methods.is_empty() && !body.event_methods.is_empty() {
        let r = refusal(
            Reason::I18n,
            "@i18n with HTML and an event handler in the same view",
        );
        *collected = body.collected.take();
        return Err(r);
    }
    let methods: String = body
        .i18n_methods
        .iter()
        .chain(&body.event_methods)
        .map(|m| resolver_refs(&resolve_deferred(body.imp, m), &ctx.resolved_refs.borrow()))
        .collect();
    // `detectHostChanges` comes after the handlers (`view.methods`).
    let methods = methods + &resolve_deferred(body.imp, &host_changes);
    // `body` borrows the interner and the import table; emitting the embedded
    // views needs both.
    *collected = body.collected.take();
    drop(body);

    imp.without_alias(ANGULAR);
    // The embedded views are written here, after the factories — and that is
    // why their imports come after `angular.dart`.
    let mut embedded_views = String::new();
    for spec in specs {
        embedded_views.push_str(&emit_embedded(spec, &ctx, imp, names, collected)?);
    }
    let host_view_prefix = imp.alias(HOST_VIEW);
    // The providers: fields, statements before the component and
    // `injectorGetInternal`, with the imports in the order they are written.
    let asset_local = local.asset();
    let providers = match &on_host {
        Some(r) => match write_host_providers(r, &asset_local, &util) {
            Ok(p) => Some(p),
            Err(r) => {
                if collected.is_none() {
                    return Err(r);
                }
                None
            }
        },
        None => None,
    };
    // `ViewContainerRef` in the constructor: the host element gets a
    // `ViewContainer`, created in `CompileElement` before the providers (the
    // field and the import come first) and passed to the component (case j47).
    let container = c.params.iter().any(|p| e_container(p, local, resolver));
    let mut container_field = String::new();
    let mut before_component = String::new();
    if container {
        let vc = imp.q(VIEW_CONTAINER);
        container_field = format!("  late final {vc}ViewContainer _appEl_0;\n");
        before_component.push_str(&format!(
            "    this._appEl_0 = {vc}ViewContainer(0, null, this, _el_0);\n"
        ));
    }
    // The emitter writes the fields with an initializer (the lazy ones) before
    // the others (`dart_emitter.dart:198-205`).
    let mut host_fields = providers
        .as_ref()
        .map(|p| resolve_deferred(imp, &p.fields))
        .unwrap_or_default();
    host_fields.push_str(&container_field);
    before_component.push_str(
        &providers
            .as_ref()
            .map(|p| resolve_deferred(imp, &p.before_component))
            .unwrap_or_default(),
    );
    let host_root = if container { "this._appEl_0" } else { "_el_0" };
    let locals: Vec<(crate::directives::Token, String)> = on_host
        .iter()
        .flat_map(|r| &r.instances)
        .flat_map(|i| {
            std::iter::once(i.token.clone())
                .chain(i.aliases.iter().cloned())
                .map(|t| (t, i.read.clone()))
        })
        .collect();
    let construction =
        match component_construction(c, local, resolver, imp, &own_prefix, &util, &locals) {
            Some(x) => x,
            None => {
                // In collection mode the refusal already came from
                // `missing_for_construction`.
                if collected.is_none() {
                    return Err(refusal(
                        Reason::UnresolvedInjection,
                        "component construction",
                    ));
                }
                String::new()
            }
        };
    // The lifecycle hooks come out in the host view, after `build()`, and
    // `check_binding.dart` goes in there.
    // `onPush` with `@Input`: the host view marks the check
    // (`bindDirectiveInputs`, `hasInputs` counts the inherited ones — which
    // cannot be seen from here: refuse).
    // `hasInputs` (`matched_directive_converter.dart:72`) counts the inherited
    // `@Input`s, which the metadata collects like
    // `_collectInheritableMetadata`.
    let inheritor_metadata = local
        .metadata
        .as_deref()
        .filter(|m| m.unsupported.is_empty());
    let inherited_inputs = c.inherits && inheritor_metadata.is_some_and(|m| !m.inputs.is_empty());
    let mark = c.on_push && (!c.inputs.is_empty() || inherited_inputs);
    if c.on_push && c.inputs.is_empty() && c.inherits && inheritor_metadata.is_none() {
        let r = refusal(
            Reason::NotUnderstood,
            "inheriting onPush component, without the metadata",
        );
        if collected.is_none() {
            return Err(r);
        }
    }
    let host_injector = match &providers {
        Some(p) if !p.injectables.is_empty() => resolve_deferred(
            imp,
            &injector_method(&[(0, 0, p.injectables.clone())], &asset_local),
        ),
        _ => String::new(),
    };
    // The hooks of an inheriting component walk up the supertypes (the class
    // that implements `OnInit` may be the base): they come from the program
    // metadata.
    // Also without inheritance: `implements X` with `X extends OnInit` can only
    // be seen through the program (`allSupertypes`); the class text is not
    // enough.
    let hooks = if let Some(m) = local
        .metadata
        .as_deref()
        .filter(|m| m.unsupported.is_empty())
    {
        m.hooks
    } else if c.inherits {
        match local.metadata.as_deref() {
            Some(m) if m.unsupported.is_empty() => m.hooks,
            _ => {
                record_refusal(
                    collected,
                    refusal(
                        Reason::NotUnderstood,
                        "hooks of an inheriting component without the metadata",
                    ),
                )?;
                c.hooks
            }
        }
    } else {
        c.hooks
    };
    // With `@HostBinding`, the host view calls `detectHostChanges` before
    // detecting the component's view (`lifecycle`).
    let cycle = resolve_deferred(
        imp,
        &lifecycle(
            &hooks,
            mark,
            of_host.iter().any(|l| !l.is_static),
            container,
        ),
    );

    let x = &c.class;
    let (args, decl) = (&ctx.generics, &ctx.generics_decl);
    let selector = &c.selector;
    // `_tagNameFromComponentSelector`: the first selector with an element.
    let tag = crate::selector::Selector::parse(selector)
        .into_iter()
        .find_map(|s| s.element)
        .unwrap_or_else(|| selector.trim().to_string());
    // `_getChangeDetectionCheckMode`: an `onPush` component starts in
    // `checkOnce`.
    let estado = if c.on_push {
        "checkOnce"
    } else {
        "checkAlways"
    };
    // Without a stylesheet, the list is constant and the style is not
    // encapsulated.
    let items: Vec<String> = style
        .iter()
        .map(|a| format!("{a}.styles"))
        .chain(inline)
        .collect();
    let (style_list, encapsulation) = match (items.is_empty(), c.without_encapsulation) {
        (true, _) => ("const []".to_string(), "unscoped"),
        (false, true) => (format!("[{}]", items.join(", ")), "unscoped"),
        (false, false) => (format!("[{}]", items.join(", ")), "scoped"),
    };
    let asset = format!("asset:{}/{}", local.package, local.relative);

    let mut s = String::with_capacity(4096);
    let _ = write!(
        s,
        "
final List<Object> styles${x} = {style_list};

class View{x}0{decl} extends {component_view_prefix}.ComponentView<{own_prefix}.{x}{args}> {{
{fields}  static {styles}.ComponentStyles? _componentStyles;
  View{x}0({view}.View parentView, int parentIndex) : super(parentView, parentIndex, {cd}.ChangeDetectionCheckedState.{estado}) {{
    this.initComponentStyles();
    this.rootElement = {util}.unsafeCast({document}.createElement('{tag}'));{statics_in_constructor}
  }}
  static String? get _debugComponentUrl {{
    return ({util}.isDevMode ? '{asset}' : null);
  }}

  @override
  void build() {{{ctx_in_build}
    final parentRenderNode = this.initViewRoot();{build_body}
  }}
{injector}{detection}{destruction}{methods}
  static void _debugClearComponentStyles() {{
    _componentStyles = null;
  }}

  void initComponentStyles() {{
    var styles = _componentStyles;
    if ((styles == null)) {{
      _componentStyles = (styles = {styles}.ComponentStyles.{encapsulation}(styles${x}, _debugComponentUrl));
      if ({util}.isDevMode) {{
        {styles}.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }}
    }}
    this.componentStyles = styles;
  }}
}}

const _{x}NgFactory = ComponentFactory<{own_prefix}.{x}>('{selector}', viewFactory_{x}Host0);
ComponentFactory<{own_prefix}.{x}> get {x}NgFactory {{
  return _{x}NgFactory;
}}

ComponentFactory<{own_prefix}.{x}{args}> create{x}Factory{decl}() {{
  return ComponentFactory('{selector}', viewFactory_{x}Host0);
}}
{embedded_views}
final List<Object> styles${x}Host = const [];

class _View{x}Host0{decl} extends {host_view_prefix}.HostView<{own_prefix}.{x}{args}> {{
{host_fields}  @override
  void build() {{
    this.componentView = View{x}0(this, 0);
    final _el_0 = this.componentView.rootElement;
{before_component}    this.component = {construction}
{host_queries_text}    this.initRootNode({host_root});
  }}
{host_injector}{cycle}}}

{host_view_prefix}.HostView<{own_prefix}.{x}{args}> viewFactory_{x}Host0{decl}() {{
  return _View{x}Host0();
}}
"
    );
    // The chains of the dynamic queries and the node that a view reads from a
    // nested one (the query result) are only known after the nested ones are
    // emitted.
    let s = resolve_queries(&s, &queries_body, &ctx.query_anchors.borrow(), &root_class);
    let s = resolve_dynamic_content(
        &s,
        &ctx.dynamic_content.borrow(),
        &ctx.query_anchors.borrow(),
    );
    let s = resolver_refs(&s, &ctx.resolved_refs.borrow());
    if s.contains(REF_MARK) && collected.is_none() {
        return Err(refusal(
            Reason::Binding,
            "#ref without a node in the template",
        ));
    }
    Ok((s, tb_leftover))
}

/// The arguments (`<T, U>`) and the declaration (`<T extends num, U>`) of the
/// type parameters of a generic component, with the bound qualified by the
/// import of whoever declares it; empty for a non-generic component.
fn type_params(
    c: &Component,
    local: &Local,
    resolver: Option<&dyn Resolution>,
    imp: &mut Imports,
) -> Result<(String, String), Refusal> {
    if c.type_params.is_empty() {
        return Ok(Default::default());
    }
    let free: Vec<&str> = c.type_params.iter().map(|(n, _)| n.as_str()).collect();
    let mut decl = Vec::new();
    for (name, bound) in &c.type_params {
        match bound {
            None => decl.push(name.clone()),
            Some(l) => {
                let q = resolver
                    .and_then(|r| qualified_type(l, local.path, r, &local.asset(), &free))
                    .ok_or_else(|| {
                        refusal(
                            Reason::NotUnderstood,
                            format!("type parameter bound `{l}` without an import"),
                        )
                    })?;
                decl.push(format!("{name} extends {}", resolve_deferred(imp, &q)));
            }
        }
    }
    Ok((
        format!("<{}>", free.join(", ")),
        format!("<{}>", decl.join(", ")),
    ))
}

/// Are a component's `providers:` in the part the emitter writes (in its host
/// view or on its user's node)? Otherwise, the reason.
pub(crate) fn writable_providers(meta: &crate::directives::Directive) -> Result<(), String> {
    use crate::directives::{Token, TokenType};
    if let Some(f) = meta.unsupported.first() {
        return Err(f.clone());
    }
    // The `T` of a token from outside `dart:core` and without type arguments
    // would get another import: no case yet.
    let known_type = |t: &TokenType| t.uri == "dart:core" || t.generics > 0;
    let known_token = |t: &Token| match t {
        Token::Multi { ty, .. } => known_type(ty),
        // Generic `OpaqueToken<T>`: no case yet.
        Token::Opaque { ty, .. } => ty.uri == "dart:core",
        _ => true,
    };
    let with_arguments = |t: &Token| match t {
        Token::Multi { ty, .. } | Token::Opaque { ty, .. } => !ty.args.is_empty(),
        _ => false,
    };
    for p in meta.providers.iter().chain(&meta.view_providers) {
        if with_arguments(&p.token) || p.ty.as_ref().is_some_and(|t| !t.args.is_empty()) {
            return Err("type token with concrete arguments".into());
        }
        let target = match &p.source {
            crate::directives::Provides::Existing(t) => Some(t),
            _ => None,
        };
        if !known_token(&p.token) || target.is_some_and(|t| !known_token(t)) {
            return Err("type token outside dart:core".into());
        }
    }
    Ok(())
}

/// The host view's providers: node 0, with the component and its `providers:`
/// (the `ProviderElementContext` of the host element). Refuses what the emitter
/// does not write yet.
fn host_providers(local: &Local) -> Result<crate::directives::ResolvedNode, Refusal> {
    use crate::directives::{Creation, Expr};
    let prov = |f: &str| refusal(Reason::Providers, format!("providers: {f}"));
    let meta = local
        .metadata
        .clone()
        .ok_or_else(|| prov("without the program metadata"))?;
    writable_providers(&meta).map_err(|f| prov(&f))?;
    let r = crate::directives::resolve_host(meta).map_err(prov)?;
    // The component is the last eager one: those it asks for come before, and
    // nothing after it is created in `build()` (a content query that finds a
    // provider would make it eager: refused separately).
    let last_eager = r.instances.iter().rposition(|i| !i.lazy);
    if last_eager.map(|k| r.instances[k].field.as_str()) != Some("component") {
        return Err(prov("eager provider after the component"));
    }
    for i in &r.instances {
        if let Creation::Multi(items) = &i.creation
            && items.iter().any(Expr::dynamic)
        {
            return Err(prov("multi-provider item with an outside dependency"));
        }
    }
    Ok(r)
}

/// The type of an inferred `T` (`o.importType`): one from `dart:core` without a
/// prefix, with its import counted.
fn type_text(t: &crate::directives::TokenType, asset: &str) -> String {
    let args = if t.generics > 0 {
        format!("<{}>", vec!["dynamic"; t.generics].join(", "))
    } else {
        String::new()
    };
    let uri = if t.uri == "dart:core" {
        t.uri.clone()
    } else {
        import_of(&t.uri, asset)
    };
    format!("{}{}{args}", deferred_q(&uri), t.class)
}

/// The token of a provider seen from this file, ready for [`token_expr`].
fn token_local(t: &crate::directives::Token, asset: &str) -> crate::directives::Token {
    use crate::directives::{Token, TokenType};
    let ty = |x: &TokenType| TokenType {
        uri: import_of(&x.uri, asset),
        ..x.clone()
    };
    match t {
        Token::Class { uri, class } => Token::Class {
            uri: import_of(uri, asset),
            class: class.clone(),
        },
        Token::Multi { name, ty: x } => Token::Multi {
            name: name.clone(),
            ty: ty(x),
        },
        Token::Opaque { name, ty: x } => Token::Opaque {
            name: name.clone(),
            ty: ty(x),
        },
        other => other.clone(),
    }
}

/// A `useValue:` as the emitter writes it.
fn value_text(v: &crate::directives::ConstValue, asset: &str) -> String {
    use crate::directives::ConstValue;
    match v {
        ConstValue::Str(s) => literal(s),
        ConstValue::Integer(i) => i.to_string(),
        ConstValue::Boolean(b) => b.to_string(),
        ConstValue::Object {
            uri,
            class,
            constructor,
            positional,
            named,
        } => {
            let mut args: Vec<String> = positional.iter().map(|x| value_text(x, asset)).collect();
            args.extend(
                named
                    .iter()
                    .map(|(n, x)| format!("{n}: {}", value_text(x, asset))),
            );
            let ctor = constructor
                .as_ref()
                .map(|c| format!(".{c}"))
                .unwrap_or_default();
            format!(
                "const {}{class}{ctor}({})",
                deferred_q(&import_of(uri, asset)),
                args.join(", ")
            )
        }
    }
}

/// The value of a provider (`ProviderSource.build`). With an injector
/// dependency, the creation goes wrapped in `debugInjectorWrap` under
/// `isDevMode` (`indent`: the column of the statement or field). `view`: in the
/// host view (`None`), the injector is its own (`this.injectorGet(T,
/// this.parentIndex)`); in a template node, the one from outside the view that
/// reads it (`injectFromViewParentInjector`: `(v.parentView!).injectorGet(T,
/// v.parentIndex)`).
fn expr_text(
    e: &crate::directives::Expr,
    token: &crate::directives::Token,
    asset: &str,
    util: &str,
    indent: usize,
    view: Option<&str>,
) -> String {
    use crate::directives::Expr;
    let simple = |e: &Expr| -> String {
        let args = |a: &[Expr]| -> String {
            a.iter()
                .map(|x| expr_text(x, token, asset, util, indent, view))
                .collect::<Vec<_>>()
                .join(", ")
        };
        match e {
            Expr::Field(c) => format!("this.{c}"),
            Expr::Read(l) => l.clone(),
            Expr::Injector { token, optional } => {
                let method = if *optional {
                    "injectorGetOptional"
                } else {
                    "injectorGet"
                };
                let t = token_expr(&token_local(token, asset));
                match view {
                    None => format!("this.{method}({t}, this.parentIndex)"),
                    Some(v) => format!("({v}.parentView!).{method}({t}, {v}.parentIndex)"),
                }
            }
            Expr::Class {
                uri,
                class,
                args: a,
            } => {
                format!("{}{class}({})", deferred_q(&import_of(uri, asset)), args(a))
            }
            Expr::Factory { uri, name, args: a } => {
                format!("{}{name}({})", deferred_q(&import_of(uri, asset)), args(a))
            }
            Expr::Value(v) => value_text(v, asset),
        }
    };
    let x = simple(e);
    if !e.dynamic() {
        return x;
    }
    let (a, b, c) = (
        " ".repeat(indent + 4),
        " ".repeat(indent + 8),
        " ".repeat(indent + 6),
    );
    format!(
        "({util}.isDevMode\n{a}? {}.debugInjectorWrap({}, () {{\n{b}return {x};\n{c}}})\n{a}: {x})",
        deferred(DI_ERRORS),
        token_expr(&token_local(token, asset))
    )
}

/// The field type of a host view provider (`createProvider`): the inferred `T`;
/// without it, the type of the expression (`dynamic` when it has none).
fn provider_type(i: &crate::directives::Instance, asset: &str) -> Result<String, Refusal> {
    use crate::directives::{ConstValue, Creation, Expr};
    if let Creation::Multi(_) = &i.creation {
        return Ok(match &i.ty {
            Some(t) => format!("List<{}>", type_text(t, asset)),
            None => "List<dynamic>".to_string(),
        });
    }
    if let Some(t) = &i.ty {
        return Ok(type_text(t, asset));
    }
    let Creation::Expression(e) = &i.creation else {
        return Err(refusal(
            Reason::Providers,
            "providers: provider without an expression",
        ));
    };
    Ok(match e {
        Expr::Class { uri, class, .. } if !e.dynamic() => {
            format!("{}{class}", deferred_q(&import_of(uri, asset)))
        }
        Expr::Value(ConstValue::Str(_)) => "String".into(),
        Expr::Value(ConstValue::Integer(_)) => "int".into(),
        Expr::Value(ConstValue::Boolean(_)) => "bool".into(),
        Expr::Value(ConstValue::Object { uri, class, .. }) => {
            format!("{}{class}", deferred_q(&import_of(uri, asset)))
        }
        Expr::Class { .. } | Expr::Factory { .. } | Expr::Injector { .. } => "dynamic".into(),
        Expr::Field(_) | Expr::Read(_) => {
            return Err(refusal(
                Reason::Providers,
                "providers: provider that reads another field",
            ));
        }
    })
}

/// The field of a lazy `providers:` provider (`late T _X_n_m = value;`), with
/// deferred imports: the type and then the value, the order in which the
/// official compiler writes (and numbers the imports).
fn lazy_provider_text(
    i: &crate::directives::Instance,
    asset: &str,
    view: Option<&str>,
) -> Result<String, Refusal> {
    use crate::directives::Creation;
    let ty = provider_type(i, asset)?;
    // The `isDevMode` of `debugInjectorWrap` (only with an injector dependency)
    // with a deferred import, in text order (case i78).
    let util = deferred(UTILITIES);
    let value = match &i.creation {
        Creation::Expression(e) => expr_text(e, &i.token, asset, &util, 2, view),
        Creation::Multi(items) => format!(
            "[{}]",
            items
                .iter()
                .map(|x| expr_text(x, &i.token, asset, &util, 2, view))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => {
            return Err(refusal(
                Reason::BindingOnChild,
                "lazy provider that is not an expression",
            ));
        }
    };
    Ok(format!("  late {ty} {} = {value};", i.field))
}

/// What the providers put in the host view: the fields (the lazy ones, with an
/// initializer, first — `visitDeclareClassStmt` groups them like that), the
/// `build()` statements before the component, and the injectables of
/// `injectorGetInternal`. All with deferred imports, in text order.
struct HostProviders {
    fields: String,
    before_component: String,
    injectables: Vec<(Vec<crate::directives::Token>, String)>,
}

fn write_host_providers(
    r: &crate::directives::ResolvedNode,
    asset: &str,
    util: &str,
) -> Result<HostProviders, Refusal> {
    use crate::directives::Creation;
    let mut lazy = Vec::new();
    let mut eager = Vec::new();
    let mut before = String::new();
    for i in &r.instances {
        if i.field == "component" {
            continue;
        }
        let ty = provider_type(i, asset)?;
        let value = |indent: usize| match &i.creation {
            Creation::Expression(e) => expr_text(e, &i.token, asset, util, indent, None),
            Creation::Multi(items) => format!(
                "[{}]",
                items
                    .iter()
                    .map(|x| expr_text(x, &i.token, asset, util, indent, None))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            _ => String::new(),
        };
        if i.lazy {
            lazy.push(format!("  late {ty} {} = {};\n", i.field, value(2)));
        } else {
            eager.push(format!("  late final {ty} {};\n", i.field));
            let _ = writeln!(before, "    this.{} = {};", i.field, value(4));
        }
    }
    let injectables = r
        .instances
        .iter()
        .filter(|i| !i.injectable_by.is_empty())
        .map(|i| {
            (
                i.injectable_by
                    .iter()
                    .map(|t| token_local(t, asset))
                    .collect(),
                i.read.clone(),
            )
        })
        .collect();
    lazy.extend(eager);
    Ok(HostProviders {
        fields: lazy.concat(),
        before_component: before,
        injectables,
    })
}

/// The component's own `@ContentChildren` in the host view: without content,
/// the list receives `[]` right after the construction
/// (`updateQueryAtStartup`), in query order; the single `@ContentChild`
/// receives nothing. A query that would find the node itself (the component,
/// one of its providers or an ngdart type) is refused.
fn host_queries(
    c: &Component,
    local: &Local,
    resolver: Option<&dyn Resolution>,
    tokens: &[crate::directives::Token],
) -> Result<String, Refusal> {
    let Some(queries) = &c.content_queries else {
        return Ok(String::new());
    };
    let mut s = String::new();
    for q in queries {
        if !q.reference {
            let uri = resolver
                .and_then(|r| r.type_uri(local.path, &q.target))
                .ok_or_else(|| {
                    refusal(Reason::ContentChild, "@ContentChild of an unresolved type")
                })?;
            let simple = q.target.rsplit('.').next().unwrap_or(&q.target).to_string();
            let token = crate::directives::Token::Class {
                uri: uri.clone(),
                class: simple.clone(),
            };
            let is_own = import_of(&uri, &local.asset()) == local.file && simple == c.class;
            if is_own
                || tokens.contains(&token)
                || uri.starts_with("package:ngdart/")
                || uri.starts_with("dart:")
            {
                return Err(refusal(
                    Reason::ContentChild,
                    "@ContentChild that would find the node itself",
                ));
            }
        }
        if q.list {
            let _ = writeln!(s, "    this.component.{} = [];", q.field);
        }
    }
    Ok(s)
}

/// The construction of the component in the host view, with injection — the
/// whole text after `this.component = `, semicolon included.
///
/// Each constructor parameter becomes `this.injectorGet(T, this.parentIndex)`,
/// with `T` qualified by the import of the library that **declares** the type —
/// that is why injection needed the semantic database. A parameter of type
/// `Element` is the root element and does not go through the injector.
///
/// When there is injection, the official compiler wraps the call in
/// `debugInjectorWrap` under `isDevMode`, so that a missing token points at the
/// component; without injection the call comes out clean.
fn component_construction(
    c: &Component,
    local: &Local,
    resolver: Option<&dyn Resolution>,
    imp: &mut Imports,
    own_prefix: &str,
    util: &str,
    locals: &[(crate::directives::Token, String)],
) -> Option<String> {
    let x = &c.class;
    // A parameter that a provider of the node itself satisfies reads its field
    // (`this._Servico_0_5`) and does not count as injection.
    let of_node = |p: &crate::component::Parameter| -> Option<String> {
        let ty = p.ty.as_deref()?.trim_end_matches('?');
        let simple = ty.rsplit('.').next()?;
        let uri = resolver?.type_uri(local.path, ty)?;
        let token = crate::directives::Token::Class {
            uri,
            class: simple.to_string(),
        };
        locals
            .iter()
            .find(|(t, _)| *t == token)
            .map(|(_, c)| format!("this.{c}"))
    };
    // In the host view (`component.type.isHost`), what the node does not
    // provide comes from the injector, with or without `@Host()`; `@Self()`
    // stays on the node (or `null`, optional); `@SkipSelf()` skips the node
    // (`_getDependency`, case j79).
    let injects = c.params.iter().any(|p| {
        if let Some(d) = annotated_dependency(c, local, p) {
            return d.attribute.is_none() && !locals.iter().any(|(t, _)| *t == d.token);
        }
        !is_element(p.ty.as_deref())
            && !e_detector(p, local, resolver)
            && !e_container(p, local, resolver)
            && !p.self_only
            && (p.skip_self || of_node(p).is_none())
    });
    // `errors.dart` goes in before the injected types, as in the official
    // output.
    let errors = injects.then(|| imp.alias(DI_ERRORS));
    let mut args = Vec::new();
    for p in &c.params {
        // `@Attribute`: the host element has no attributes (`null`).
        // `@Inject(token)`: the node's provider or the injector, by token.
        if let Some(d) = annotated_dependency(c, local, p) {
            if d.attribute.is_some() {
                args.push("null".to_string());
                continue;
            }
            if let Some((_, field)) = locals.iter().find(|(t, _)| *t == d.token) {
                args.push(format!("this.{field}"));
                continue;
            }
            let expr = match &d.token {
                crate::directives::Token::Class { uri, class } => {
                    let asset = asset_of_uri(uri, local.package, local.root)?;
                    let path = import_path(&local.asset(), &asset)?;
                    format!("{}.{class}", imp.alias(&path))
                }
                t => resolve_deferred(imp, &token_expr(t)),
            };
            let method = if d.optional {
                "injectorGetOptional"
            } else {
                "injectorGet"
            };
            args.push(format!("this.{method}({expr}, this.parentIndex)"));
            continue;
        }
        if is_element(p.ty.as_deref()) {
            args.push("_el_0".to_string());
            continue;
        }
        // `ChangeDetectorRef`: the component's view.
        if e_detector(p, local, resolver) {
            args.push("this.componentView".to_string());
            continue;
        }
        // `ViewContainerRef`: o `ViewContainer` do elemento hospedeiro.
        if e_container(p, local, resolver) {
            args.push("this._appEl_0".to_string());
            continue;
        }
        if !p.named
            && !p.skip_self
            && let Some(field) = of_node(p)
        {
            args.push(field);
            continue;
        }
        if p.self_only && p.optional && !p.named && !p.other_annotation {
            args.push("null".to_string());
            continue;
        }
        if p.other_annotation || p.self_only || p.named {
            return None; // `@Inject(...)`, `@Self`, named: not yet
        }
        let ty = token_ty(p.ty.as_deref()?);
        let simple = ty.rsplit('.').next()?;
        let uri = resolver?.type_uri(local.path, ty)?;
        // Another built-in of the host element (`ElementRef`, `TemplateRef`…):
        // does not come from the injector; no case yet.
        if crate::directives::element_builtin(&crate::directives::Token::Class {
            uri: uri.clone(),
            class: simple.to_string(),
        }) {
            return None;
        }
        let asset = asset_of_uri(&uri, local.package, local.root)?;
        let path = import_path(&local.asset(), &asset)?;
        let alias = imp.alias(&path);
        // `@Optional()`: `injectorGetOptional` (`injectFromViewParentInjector`).
        let method = if p.optional {
            "injectorGetOptional"
        } else {
            "injectorGet"
        };
        args.push(format!("this.{method}({alias}.{simple}, this.parentIndex)"));
    }
    let call = format!("{own_prefix}.{x}({})", args.join(", "));
    let Some(errors) = errors else {
        return Some(format!("{call};"));
    };
    Some(format!(
        "({util}.isDevMode
        ? {errors}.debugInjectorWrap({own_prefix}.{x}, () {{
            return {call};
          }})
        : {call});"
    ))
}

/// The dependency (from the program metadata) of a parameter with `@Inject` or
/// `@Attribute`, by its position among the unnamed ones; `None` without the
/// metadata, and also with `@Self`/`@Host`/`@SkipSelf` alongside (no case yet)
/// or on an element built-in.
fn annotated_dependency<'m>(
    c: &Component,
    local: &'m Local,
    p: &crate::component::Parameter,
) -> Option<&'m crate::directives::Dependency> {
    if !p.other_annotation || p.named || p.self_only || p.host || p.skip_self {
        return None;
    }
    let k = c
        .params
        .iter()
        .filter(|q| !q.named)
        .position(|q| std::ptr::eq(q, p))?;
    let meta = local.metadata.as_deref()?;
    if !meta.dependencies_read
        || meta.dependencies.len() != c.params.iter().filter(|q| !q.named).count()
    {
        return None;
    }
    let d = meta.dependencies.get(k)?;
    match &d.token {
        _ if d.attribute.is_some() => Some(d),
        crate::directives::Token::Element | crate::directives::Token::Detector => None,
        t if crate::directives::element_builtin(t) => None,
        _ => Some(d),
    }
}

/// Is the parameter the component's root element?
/// Is the parameter ngdart's `ChangeDetectorRef` (without an annotation)?
fn e_detector(
    p: &crate::component::Parameter,
    local: &Local,
    resolver: Option<&dyn Resolution>,
) -> bool {
    let Some(ty) = p.ty.as_deref() else {
        return false;
    };
    !p.annotated
        && ty.rsplit('.').next() == Some("ChangeDetectorRef")
        && resolver
            .and_then(|r| r.type_uri(local.path, ty))
            .is_some_and(|u| u.starts_with("package:ngdart/"))
}

/// Is the parameter ngdart's `ViewContainerRef` (without an annotation)?
fn e_container(
    p: &crate::component::Parameter,
    local: &Local,
    resolver: Option<&dyn Resolution>,
) -> bool {
    let Some(ty) = p.ty.as_deref() else {
        return false;
    };
    !p.annotated
        && matches!(
            ty.rsplit('.').next(),
            Some("ViewContainerRef" | "ComponentLoader")
        )
        && resolver
            .and_then(|r| r.type_uri(local.path, ty))
            .is_some_and(|u| u.starts_with("package:ngdart/"))
}

/// Does the parameter ask for the element node: `Element`/`HtmlElement` of
/// `dart:html`, or `Element`/`HTMLElement` of `package:web` in 9?
fn is_element(ty: Option<&str>) -> bool {
    let ngx = crate::dialect::current() == crate::dialect::Dialect::Ngx;
    match ty.map(|t| t.rsplit('.').next().unwrap_or(t)) {
        Some("Element") => true,
        Some("HtmlElement") => !ngx,
        Some("HTMLElement") => ngx,
        _ => false,
    }
}

/// The token of a parameter by its written type (`_tokenForType`/`_idFor`): the
/// class, without the `?` (`@Optional() X? x`) and without the type arguments
/// (`MaterialTreeRoot<T>` asks for `MaterialTreeRoot`).
pub(crate) fn token_ty(ty: &str) -> &str {
    ty.split('<')
        .next()
        .unwrap_or(ty)
        .trim()
        .trim_end_matches('?')
}

/// What prevents the construction, if anything. The import table is not touched
/// here — this only looks.
fn missing_for_construction(
    c: &Component,
    local: &Local,
    resolver: Option<&dyn Resolution>,
) -> Option<Refusal> {
    for p in &c.params {
        if is_element(p.ty.as_deref()) {
            continue;
        }
        if annotated_dependency(c, local, p).is_some() {
            continue;
        }
        if p.other_annotation || (p.self_only && !p.optional) {
            return Some(refusal(
                Reason::AnnotatedInjection,
                "@Inject/@Attribute/@Self in the constructor",
            ));
        }
        if p.named {
            return Some(refusal(
                Reason::NamedInjection,
                "named parameter in the constructor",
            ));
        }
        let Some(ty) = p.ty.as_deref() else {
            return Some(refusal(
                Reason::UntypedInjection,
                "untyped parameter in the constructor",
            ));
        };
        let ty = token_ty(ty);
        if !resolver.is_some_and(|r| r.type_uri(local.path, ty).is_some()) {
            return Some(refusal(
                Reason::UnresolvedInjection,
                "unresolved injected type",
            ));
        }
    }
    None
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::component::Parameter;
    use dartforge_intern::Interner;

    /// `<template ngFor let-x [ngForOf]>` becomes the `star` of the
    /// equivalent `*ngFor`, keeping the range of each written binding (i84,
    /// i86).
    #[test]
    fn template_with_directive_becomes_star() {
        let nodes = crate::html::parse(
            r#"<template ngFor [ngForTrackBy]="f" let-item [ngForOf]="xs" let-i="index"><p></p></template>"#,
        );
        let nodes = template_as_container(&nodes);
        let [Node::Element(e)] = nodes.as_slice() else {
            panic!("a single element");
        };
        assert_eq!(e.name, "ng-container");
        let star = e.star.as_ref().expect("estrela");
        assert_eq!(star.name, "ngFor");
        assert_eq!(star.value, "let item; let i = index; trackBy: f; of: xs");
        let micro = e.star_micro().unwrap_or_default();
        assert_eq!(
            micro.properties,
            vec![
                ("ngForTrackBy".to_string(), "f".to_string()),
                ("ngForOf".to_string(), "xs".to_string())
            ]
        );
        let spans: Vec<(&str, usize, usize)> = e
            .template_bindings
            .iter()
            .map(|l| (l.name.as_str(), l.start, l.end))
            .collect();
        assert_eq!(spans, vec![("ngForTrackBy", 16, 34), ("ngForOf", 44, 58)]);
    }

    /// What is not the exact shape stays a `<template>` (and is refused):
    /// unknown directive, two attributes, `;` in the expression, event.
    #[test]
    fn template_with_directive_outside_shape() {
        for t in [
            r#"<template foo [fooOf]="xs"></template>"#,
            r#"<template ngFor ngIf [ngForOf]="xs"></template>"#,
            r#"<template ngFor [ngForOf]="xs" (x)="y()"></template>"#,
            r#"<template ngFor let-x></template>"#,
        ] {
            let nodes = template_as_container(&crate::html::parse(t));
            let [Node::Element(e)] = nodes.as_slice() else {
                panic!("a single element");
            };
            assert_eq!(e.name, "template", "{t}");
            assert!(e.template_bindings.is_empty(), "{t}");
        }
        // A `;` inside a string does not get in the way: the microsyntax goes
        // decomposed, without going back to the text (case j75).
        let nodes = template_as_container(&crate::html::parse(
            r#"<template ngFor [ngForOf]="f(';')"></template>"#,
        ));
        let [Node::Element(e)] = nodes.as_slice() else {
            panic!("a single element");
        };
        assert_eq!(e.name, "ng-container");
        assert_eq!(
            e.star_micro().expect("micro").properties,
            vec![("ngForOf".to_string(), "f(';')".to_string())]
        );
    }

    fn local() -> Local<'static> {
        Local {
            package: "new_sali_frontend",
            relative: "lib/src/shared/components/form_feedback/form_feedback_component.dart",
            file: "form_feedback_component.dart",
            path: Path::new("x.dart"),
            root: Path::new(""),
            template_url: None,
            metadata: None,
        }
    }

    /// Answers what the semantic database would answer, without loading a
    /// program: the pair (type name, URI of the library that declares it).
    struct Table(&'static [(&'static str, &'static str)]);

    impl Resolution for Table {
        fn type_uri(&self, _file: &Path, name: &str) -> Option<String> {
            self.0
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, u)| u.to_string())
        }
    }

    fn param(ty: &str) -> Parameter {
        Parameter {
            ty: Some(ty.into()),
            name: "p".into(),
            named: false,
            annotated: false,
            optional: false,
            self_only: false,
            host: false,
            skip_self: false,
            other_annotation: false,
        }
    }

    /// Only a read with the implicit receiver calls `getLocal` (case i45).
    #[test]
    fn local_reference_only_at_root() {
        assert!(references_at_root("usar(campo.value)", "campo"));
        assert!(references_at_root("a ? campo : b", "campo"));
        assert!(!references_at_root("p.campo", "campo"));
        assert!(!references_at_root("p?.campo", "campo"));
        assert!(!references_at_root("f(campo: 1)", "campo"));
        assert!(!references_at_root("'campo' + x", "campo"));
        assert!(!references_at_root("campos", "campo"));
    }

    /// Exact bytes of the file the official compiler generated for
    /// `FormFeedbackComponent` (a template with only a comment).
    #[test]
    fn skeleton_equal_to_official() {
        let c = Component {
            class: "FormFeedbackComponent".into(),
            selector: "form-feedback-comp".into(),
            ..Default::default()
        };
        let output = component_template(
            &c,
            &local(),
            &[Node::Comment("{{message}}".into())],
            None,
            &mut Interner::new(),
            &Default::default(),
            &[],
            &Ok(Vec::new()),
        )
        .expect("gera");
        let expected = include_str!("../testdata/form_feedback_component.template.dart");
        assert_eq!(output, expected.replace("\r\n", "\n"));
    }

    /// Exact bytes of `CallbackComponent`, whose template is `<div>Processando
    /// login...</div>` — the first one with real nodes. The constructor
    /// injection is replaced with a constructor without parameters, which is
    /// what this step covers; the rest of the file is the official one.
    #[test]
    fn element_and_text_equal_to_official() {
        let c = Component {
            class: "CallbackComponent".into(),
            selector: "callback-page".into(),
            ..Default::default()
        };
        let local = Local {
            package: "new_sali_frontend",
            relative: "lib/src/modules/auth/pages/callback/callback_component.dart",
            file: "callback_component.dart",
            path: Path::new("callback_component.dart"),
            root: Path::new(""),
            template_url: None,
            metadata: None,
        };
        let nodes = crate::html::parse("<div>Processando login...</div>");
        let output = component_template(
            &c,
            &local,
            &nodes,
            None,
            &mut Interner::new(),
            &Default::default(),
            &[],
            &Ok(Vec::new()),
        )
        .expect("gera");
        let expected = include_str!("../testdata/callback_component.template.dart");
        assert_eq!(output, expected.replace("\r\n", "\n"));
    }

    #[test]
    fn binding_not_generated_yet() {
        let c = Component {
            class: "X".into(),
            selector: "x".into(),
            ..Default::default()
        };
        let nodes = crate::html::parse("<div [hidden]=\"a\"></div>");
        assert_eq!(
            component_template(
                &c,
                &local(),
                &nodes,
                None,
                &mut Interner::new(),
                &Default::default(),
                &[],
                &Ok(Vec::new()),
            )
            .map_err(|r| r.reason),
            Err(Reason::Binding)
        );
    }

    #[test]
    fn component_inside_template_not_generated_yet() {
        let c = Component {
            class: "X".into(),
            selector: "x".into(),
            ..Default::default()
        };
        let nodes = crate::html::parse("<outro-comp></outro-comp>");
        assert_eq!(
            component_template(
                &c,
                &local(),
                &nodes,
                None,
                &mut Interner::new(),
                &Default::default(),
                &[],
                &Ok(Vec::new()),
            )
            .map_err(|r| r.reason),
            Err(Reason::ComponentInTemplate)
        );
    }

    #[test]
    fn injected_parameter_not_generated_yet() {
        let c = Component {
            class: "X".into(),
            selector: "x".into(),
            params: vec![param("RestConfig")],
            ..Default::default()
        };
        // Without a semantic database there is no way to know which library
        // declares the type.
        assert_eq!(
            component_template(
                &c,
                &local(),
                &[],
                None,
                &mut Interner::new(),
                &Default::default(),
                &[],
                &Ok(Vec::new()),
            )
            .map_err(|r| r.reason),
            Err(Reason::UnresolvedInjection)
        );
    }

    /// Exact bytes of the official `CallbackComponent`, now **with** injection:
    /// two tokens, one from the package itself (relative import) and one from
    /// ngrouter (the `package:` import of the library that declares the type).
    #[test]
    fn injection_equal_to_official() {
        let c = Component {
            class: "CallbackComponent".into(),
            selector: "callback-page".into(),
            params: vec![param("OidcService"), param("Router")],
            ..Default::default()
        };
        let local = Local {
            package: "new_sali_frontend",
            relative: "lib/src/modules/auth/pages/callback/callback_component.dart",
            file: "callback_component.dart",
            path: Path::new("callback_component.dart"),
            root: Path::new(""),
            template_url: None,
            metadata: None,
        };
        let table = Table(&[
            (
                "OidcService",
                "package:new_sali_frontend/src/shared/services/oidc_service.dart",
            ),
            ("Router", "package:ngrouter/src/router/router.dart"),
        ]);
        let nodes = crate::html::parse("<div>Processando login...</div>");
        let output = component_template(
            &c,
            &local,
            &nodes,
            Some(&table),
            &mut Interner::new(),
            &Default::default(),
            &[],
            &Ok(Vec::new()),
        )
        .expect("gera");
        let expected = include_str!("../testdata/callback_with_injection.template.dart");
        assert_eq!(output, expected.replace("\r\n", "\n"));
    }
}

/// Whether `path` (seen from `asset`) is the `.template.dart` of the file
/// itself — that of a child declared next to it, whose view takes no prefix.
/// The prefix of the `XNgCd` of a directive with `@HostBinding`: the import of
/// its `.template.dart`, or nothing when the class is in this same file (case
/// j45).
fn ngcd_prefix(imp: &mut Imports, uri: &str, asset: &str) -> String {
    let tpl = import_of(&uri.replace(".dart", ".template.dart"), asset);
    if is_own_template(asset, &tpl) {
        String::new()
    } else {
        imp.q(&tpl)
    }
}

fn is_own_template(asset: &str, path: &str) -> bool {
    let name = asset.rsplit('/').next().unwrap_or(asset);
    name.strip_suffix(".dart")
        .is_some_and(|base| path == format!("{base}.template.dart"))
}

/// How the file generated in `asset` imports library `uri`
/// (`getImportModulePath`): relative in the same package and folder, `package:`
/// outside; `dart:` as is.
fn import_of(uri: &str, asset: &str) -> String {
    asset_of_uri(uri, "", Path::new(""))
        .and_then(|target| import_path(asset, &target))
        .unwrap_or_else(|| uri.to_string())
}

/// Does some element of the content have the `#ref`?
fn ref_in_content(nodes: &[Node], r: &str) -> bool {
    nodes.iter().any(|n| {
        let Node::Element(x) = n else { return false };
        x.references.iter().any(|y| y.name == r) || ref_in_content(&x.children, r)
    })
}
