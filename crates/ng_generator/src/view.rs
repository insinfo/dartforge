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
use crate::component::Componente;
use crate::dom;
use crate::html::No;
use crate::resolucao::{Resolucao, asset_de_uri, caminho_do_import};
use std::fmt::Write;
use std::path::Path;

/// Import table of the generated file.
#[derive(Default, Clone)]
pub struct Importacoes {
    /// (key, URI, with prefix). The key is the URI, except when the official
    /// compiler imports the same library twice (see [`Self::q_chave`]).
    items: Vec<(String, String, bool)>,
}

impl Importacoes {
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
    pub fn sem_alias(&mut self, uri: &str) {
        self.index(uri, false);
    }

    /// Like [`Self::q`], but with a key of its own: the same URI gets another
    /// number. This is what the official compiler does with the type argument
    /// of a `MultiToken` (`MultiToken<import20.ControlValueAccessor<dynamic>>`
    /// next to the field's `import7.ControlValueAccessor`): the type comes from
    /// another analyzer path and the import is a different one, even with the
    /// same text.
    pub fn q_chave(&mut self, key: &str, uri: &str) -> String {
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

    fn escrever(&self, out: &mut String) {
        for (i, (_, uri, with_alias)) in self.items.iter().enumerate() {
            // Internally the URIs are those of `ngdart` 8; the output is in the
            // dialect.
            let uri = crate::dialect::escrita(uri);
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
/// The `estrela` name put on a hand-written `<template>`
/// ([`template_como_container`]): it marks the embedded view boundary for
/// whoever walks the template, without being any directive.
const MARCA_DE_MOLDE: &str = "\u{0}template";
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
pub enum Motivo {
    /// `@Directive` or `@Pipe` in the same file.
    DiretivaOuPipe,
    /// `@GenerateInjector`.
    Injetor,
    /// `@HostBinding`/`@HostListener` on a directive in a shape that the
    /// generated `DirectiveChangeDetector` does not cover yet (inheritance, a
    /// binding that is not `class.x`, several directives in the file).
    HostBindingEmDiretiva,
    /// More than one component in the file.
    VariosComponentes,
    /// `styleUrls`/`styles`: they touch `styles$X` and turn on the style shim.
    Estilos,
    /// Annotated parameter (`@Optional`, `@Inject(…)`, `@Attribute`…).
    InjecaoAnotada,
    /// Named parameter in the constructor.
    InjecaoNomeada,
    /// Generic token (`List<X>`, `OpaqueToken<String>`).
    InjecaoGenerica,
    /// Parameter without a written type.
    InjecaoSemTipo,
    /// The semantic database did not find the library that declares the type.
    InjecaoNaoResolvida,
    /// `[x]`, `(x)`, `[(x)]`, `#ref` or `*ngIf` in the template.
    Ligacao,
    /// `{{ … }}` in the template.
    Interpolacao,
    /// Tag that is not HTML: a component or directive of the application.
    ComponenteNoTemplate,
    /// `<ng-content>`.
    Projecao,
    /// Inline `style="..."`.
    EstiloEmLinha,
    /// `.html` file of the `templateUrl` not found.
    TemplateAusente,
    /// `@Input`/`@Output` on a child component.
    LigacaoEmFilho,
    /// Attribute or binding that belongs to an ecosystem directive
    /// (`ngClass`, `ngModel`…), not to the DOM.
    Diretiva,
    /// `providers:` with providers: the host element's injection.
    Providers,
    /// `@ViewChild` whose target is in an embedded view (`*ngIf`, `*ngFor`),
    /// `@ViewChildren`, or a reference that the template does not have.
    ViewChildDinamico,
    /// `@ViewChild` that queries a component or directive (type selector,
    /// `read:`, `#ref` on a child component, a field that is not `Element`).
    ViewChildEmFilho,
    /// Pipe used in the template (`x | name`, `$pipe.name(x)`).
    PipesUsados,
    /// `encapsulation:` — changes the style shim.
    Encapsulamento,
    /// `@HostListener` on a component.
    HostListenerEmComponente,
    /// `@HostBinding` on a component: the view's `detectHostChanges`.
    HostBindingEmComponente,
    /// `@ContentChild`/`@ContentChildren`.
    ContentChild,
    /// Another shape of the component that the generator cannot translate and
    /// therefore refuses — an unknown argument of `@Component`, `@ViewChild`
    /// with options.
    NaoEntendido,
    /// An element matches the selector of a directive from `directives:` that
    /// the emitter does not instantiate (`<form>` with `NgForm`, `<option>`,
    /// `[routerLink]`…). Emitting the bare element would compile and do
    /// something else.
    DiretivaPorSeletor,
    /// `(event)` in the template outside what the emitter can bind.
    Evento,
    /// `@i18n` outside the shape the emitter translates (message with HTML,
    /// annotation on a child component or on `*`, another annotation).
    I18n,
    /// Template expression outside what the converter translates. It is the
    /// converter's provisional category: its caller swaps it for the one of
    /// the context (interpolation, binding, event) with [`Recusa::em`].
    Expressao,
}

/// A refusal: the scoreboard category and the concrete sub-shape that the
/// emitter cannot translate (`event: handler with assignment`, `interpolation:
/// kind of ternary`). The scoreboard counts both; it is the sub-shape that
/// tells what to tackle next.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Recusa {
    pub motivo: Motivo,
    pub forma: String,
}

impl Recusa {
    pub fn new(reason: Motivo, shape: impl Into<String>) -> Self {
        Recusa {
            motivo: reason,
            forma: shape.into(),
        }
    }

    /// The same sub-shape in the context of whoever called the expression
    /// converter. A refusal that already has its own category (pipe) stays as
    /// it is.
    pub fn em(self, reason: Motivo) -> Self {
        if self.motivo == Motivo::Expressao {
            Recusa { motivo: reason, ..self }
        } else {
            self
        }
    }

    pub fn texto(&self) -> String {
        format!("{}: {}", self.motivo.texto(), self.forma)
    }
}

/// Shortcut: `Err(recusa(Motivo::Evento, "handler with assignment"))`.
pub fn recusa(reason: Motivo, shape: impl Into<String>) -> Recusa {
    Recusa::new(reason, shape)
}

impl Motivo {
    pub fn texto(self) -> &'static str {
        match self {
            Motivo::DiretivaOuPipe => "directive or pipe",
            Motivo::Injetor => "@GenerateInjector",
            Motivo::HostBindingEmDiretiva => "@HostBinding/@HostListener on a directive",
            Motivo::VariosComponentes => "several components in the file",
            Motivo::Estilos => "stylesheet",
            Motivo::InjecaoAnotada => "injection: annotated parameter",
            Motivo::InjecaoNomeada => "injection: named parameter",
            Motivo::InjecaoGenerica => "injection: generic token",
            Motivo::InjecaoSemTipo => "injection: untyped parameter",
            Motivo::InjecaoNaoResolvida => "injection: unresolved type",
            Motivo::Ligacao => "binding in the template",
            Motivo::Interpolacao => "interpolation",
            Motivo::ComponenteNoTemplate => "component in the template",
            Motivo::Projecao => "<ng-content>",
            Motivo::EstiloEmLinha => "inline style",
            Motivo::TemplateAusente => "template not found",
            Motivo::LigacaoEmFilho => "binding on a child component",
            Motivo::Diretiva => "directive binding",
            Motivo::Providers => "providers: [..]",
            Motivo::ViewChildDinamico => "@ViewChild in an embedded view / @ViewChildren",
            Motivo::ViewChildEmFilho => "@ViewChild of a component or directive",
            Motivo::PipesUsados => "pipe used in the template",
            Motivo::Encapsulamento => "encapsulation:",
            Motivo::HostListenerEmComponente => "@HostListener on a component",
            Motivo::HostBindingEmComponente => "@HostBinding on a component",
            Motivo::ContentChild => "@ContentChild",
            Motivo::NaoEntendido => "other component shape not understood",
            Motivo::DiretivaPorSeletor => "directive matched by selector",
            Motivo::Evento => "event",
            Motivo::I18n => "@i18n",
            Motivo::Expressao => "expression",
        }
    }
}

/// The `#ref`s the emitter accepts on an HTML element: without a value
/// (`#f="ngForm"` points to a directive) and not used in a template expression
/// — once used, the name becomes a view local and the read changes. Such a
/// `#ref` is just a name for the node, and whoever reads it is `@ViewChild`.
fn referencias_livres(nodes: &[No]) -> std::collections::HashSet<String> {
    fn collect_all(nodes: &[No], out: &mut Vec<(String, String)>) {
        for n in nodes {
            if let No::Elemento(e) = n {
                out.extend(
                    e.referencias
                        .iter()
                        .map(|r| (r.nome.clone(), r.valor.clone())),
                );
                collect_all(&e.filhos, out);
            }
        }
    }
    let mut refs = Vec::new();
    collect_all(nodes, &mut refs);
    refs.iter()
        .filter(|(name, value)| value.is_empty() && !local_citado(nodes, name))
        .map(|(name, _)| name.clone())
        .collect()
}

/// The `#ref`s of an embedded view that are just a name for the node: without
/// a value, declared in it, not read by any expression of its scope
/// ([`referenced_in_scope`]) and with no query looking for them — the official
/// compiler creates the element as if the `#ref` did not exist.
fn referencias_livres_da_visao(
    nodes: &[No],
    children: &std::collections::HashMap<String, Filho>,
    queried: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    let mut own = Vec::new();
    fn walk(nodes: &[No], out: &mut Vec<(String, String)>) {
        for n in nodes {
            if let No::Elemento(e) = n {
                // What is in a `*` (or in a `<template>`) belongs to another view.
                if e.estrela.is_some() {
                    if e.estrela.as_ref().is_some_and(|l| l.nome == MARCA_DE_MOLDE) {
                        out.extend(
                            e.referencias
                                .iter()
                                .map(|r| (r.nome.clone(), r.valor.clone())),
                        );
                    }
                    continue;
                }
                out.extend(
                    e.referencias
                        .iter()
                        .map(|r| (r.nome.clone(), r.valor.clone())),
                );
                walk(&e.filhos, out);
            }
        }
    }
    walk(nodes, &mut own);
    own
        .into_iter()
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
    descricao: Option<String>,
    locale: Option<String>,
    meaning: Option<String>,
    skip: bool,
}

/// `parseI18nMetadata`: the element's annotations grouped by the attribute
/// they internationalize (`None`: the children). The name matches
/// `i18n(.locale|.meaning|.skip)?(:attribute)?`; an annotation that is not
/// `@i18n`, a parameter without a description and an empty attribute are
/// refused.
fn metadados_i18n(e: &crate::html::Elemento) -> Result<Vec<(Option<String>, MetaI18n)>, Recusa> {
    fn normalize(v: &str) -> String {
        v.split_whitespace().collect::<Vec<_>>().join(" ")
    }
    let mut out: Vec<(Option<String>, MetaI18n)> = Vec::new();
    for a in &e.anotacoes {
        let Some(rest) = a.nome.strip_prefix("i18n") else {
            return Err(recusa(Motivo::I18n, format!("annotation @{}", a.nome)));
        };
        let (param, rest) = match rest.strip_prefix('.') {
            Some(r) => {
                let end = r.find(':').unwrap_or(r.len());
                (Some(&r[..end]), &r[end..])
            }
            None => (None, rest),
        };
        let attribute = match rest.strip_prefix(':') {
            Some("") => return Err(recusa(Motivo::I18n, "@i18n: no attribute")),
            Some(x) => Some(x.to_string()),
            None if rest.is_empty() => None,
            None => return Err(recusa(Motivo::I18n, format!("annotation @{}", a.nome))),
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
            None => m.descricao = Some(normalize(&a.valor)),
            Some("locale") => m.locale = Some(normalize(&a.valor)),
            Some("meaning") => m.meaning = Some(normalize(&a.valor)),
            Some("skip") => m.skip = true,
            Some(_) => return Err(recusa(Motivo::I18n, format!("annotation @{}", a.nome))),
        }
    }
    if out.iter().any(|(_, m)| m.descricao.is_none()) {
        return Err(recusa(Motivo::I18n, "@i18n parameter without the description"));
    }
    Ok(out)
}

/// In the component's view (outside `*`), does the first `@i18n` annotation
/// come before the first interpolation? That is what decides whether
/// `package:intl` (of the `_message_N` field) is imported before or after
/// `text_binding.dart`: both fields are allocated by `build()`, in document
/// order. `None`: there is no annotation in the component's view.
fn i18n_antes_da_interpolacao(nodes: &[No]) -> Option<bool> {
    fn walk(nodes: &[No], interpolated: &mut bool) -> Option<bool> {
        for n in nodes {
            match n {
                No::Interpolacao { .. } => *interpolated = true,
                No::Elemento(e) if e.estrela.is_some() => {}
                No::Elemento(e) => {
                    if !e.anotacoes.is_empty() {
                        // A message with HTML is a method, not a field: the
                        // import comes out with the method, after `build()`.
                        if message_in_field(e) {
                            return Some(!*interpolated);
                        }
                        continue;
                    }
                    if let Some(r) = walk(&e.filhos, interpolated) {
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
fn contem_anotacao(nodes: &[No]) -> bool {
    nodes.iter().any(|n| match n {
        No::Elemento(e) if e.estrela.is_some() => false,
        No::Elemento(e) => !e.anotacoes.is_empty() || contem_anotacao(&e.filhos),
        _ => false,
    })
}

/// Does the element's `@i18n` generate any `static final String _message_N`
/// field? The attribute ones (`@i18n:x`) and the children's one when they are
/// a single text (`_isText` of `i18n.dart`); with HTML, the children's
/// message is a static method.
fn message_in_field(e: &crate::html::Elemento) -> bool {
    let from_attribute = e.anotacoes.iter().any(|a| a.nome.contains(':'));
    from_attribute || matches!(e.filhos.as_slice(), [No::Texto(_)])
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
fn mensagem_com_html(
    nodes: &[No],
    children: &std::collections::HashMap<String, Filho>,
    text: &mut String,
    args: &mut Vec<(String, String)>,
    tags: &mut usize,
) -> Result<bool, &'static str> {
    let mut has_text = false;
    for n in nodes {
        match n {
            No::Comentario(_) => {}
            No::Texto(t) => {
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
            No::Elemento(e) => {
                let simple = e.atributos.is_empty()
                    && e.propriedades.is_empty()
                    && e.eventos.is_empty()
                    && e.bananas.is_empty()
                    && e.referencias.is_empty()
                    && e.anotacoes.is_empty()
                    && e.estrela.is_none()
                    && !children.contains_key(&e.nome)
                    && dom::tag_html(&e.nome)
                    && !matches!(e.nome.as_str(), "template" | "ng-container");
                if !simple {
                    return Err("@i18n message with an element that is not plain HTML");
                }
                let k = *tags;
                *tags += 1;
                let mut arg = |name: String, value: String, text: &mut String| {
                    let _ = write!(text, "${{{name}}}");
                    args.push((name, value));
                };
                if crate::html::VAZIOS.contains(&e.nome.as_str()) {
                    if !e.filhos.is_empty() {
                        return Err("@i18n message with a void element with children");
                    }
                    arg(format!("voidElement{k}"), format!("<{}>", e.nome), text);
                } else {
                    arg(format!("startTag{k}"), format!("<{}>", e.nome), text);
                    has_text |= mensagem_com_html(&e.filhos, children, text, args, tags)?;
                    let _ = write!(text, "${{endTag{k}}}");
                    args.push((format!("endTag{k}"), format!("</{}>", e.nome)));
                }
            }
            No::Interpolacao { .. } | No::Conteudo { .. } => {
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
const MARCA_DE_REF: char = '\u{5}';
const FIM_DE_REF: char = '\u{6}';

/// Prefix, in the key of [`resolver_refs`], of the view of an `onPush` child
/// found by the key of a query (`\u{5}.\u{8}f\u{6}` → `._compView_1`), read
/// from a nested view ([`QueryAssembly::map`]).
const DETECTOR_MARK: char = '\u{8}';

/// The template's `#ref` names that may become a local of some view: no
/// component method with the same name, and no member whose type is not known
/// from here (with the same name as a member, the local is the node, but the
/// official compiler's `_TypeResolver` types the read by the member:
/// [`Corpo::declarar_refs`]). Uniqueness and shadowing by `let` are per view
/// ([`referencias_locais`]).
fn referencias_candidatas(nodes: &[No], c: &Componente) -> std::collections::HashSet<String> {
    fn collect_all(nodes: &[No], refs: &mut Vec<String>) {
        for n in nodes {
            if let No::Elemento(e) = n {
                refs.extend(e.referencias.iter().map(|r| r.nome.clone()));
                collect_all(&e.filhos, refs);
            }
        }
    }
    let mut refs = Vec::new();
    collect_all(nodes, &mut refs);
    refs.into_iter()
        .filter(|name| {
            !c.metodos.contains_key(name.as_str())
                && c.membros
                    .get(name.as_str())
                    .is_none_or(|m| !m.tipo.trim().is_empty())
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
fn referencias_locais(
    nodes: &[No],
    children: &std::collections::HashMap<String, Filho>,
    candidates: &std::collections::HashSet<String>,
    lets: &[String],
) -> std::collections::HashSet<String> {
    candidates
        .iter()
        .filter(|name| {
            let mut places = Vec::new();
            onde_esta(nodes, name, children, false, &mut places);
            let own: Vec<&Lugar> = places.iter().filter(|l| **l != Lugar::Embutida).collect();
            matches!(
                own.as_slice(),
                [Lugar::Raiz | Lugar::NoFilho | Lugar::Projetado | Lugar::NoFilhoProjetado]
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
    nodes: &[No],
    name: &str,
    children: &std::collections::HashMap<String, Filho>,
) -> bool {
    let refers = |text: &str| cita_na_raiz(text, name);
    nodes.iter().any(|n| match n {
        No::Interpolacao { expr, .. } => refers(expr),
        No::Elemento(e) => match &e.estrela {
            Some(star) if star.nome != MARCA_DE_MOLDE => {
                let micro = e.micro_da_estrela().unwrap_or_default();
                micro.propriedades.iter().any(|(_, expr)| refers(expr))
                    || citado_na_embutida(e, name, children)
            }
            Some(_) => element_references(e, name) || citado_na_embutida(e, name, children),
            None => element_references(e, name) || referenced_in_scope(&e.filhos, name, children),
        },
        _ => false,
    })
}

/// `name` is read in this view's detection — a binding, an interpolation, the
/// `*` of one of its elements —, not counting events nor the embedded views.
fn lido_na_deteccao(nodes: &[No], name: &str) -> bool {
    let refers = |text: &str| cita_na_raiz(text, name);
    let on_element = |e: &crate::html::Elemento| {
        e.propriedades
            .iter()
            .chain(e.bananas.iter())
            .any(|l| refers(&l.valor))
            || e.atributos
                .iter()
                .any(|a| a.valor.contains("{{") && refers(&a.valor))
    };
    nodes.iter().any(|n| match n {
        No::Interpolacao { expr, .. } => refers(expr),
        No::Elemento(e) => match &e.estrela {
            Some(star) if star.nome != MARCA_DE_MOLDE => {
                let micro = e.micro_da_estrela().unwrap_or_default();
                micro.propriedades.iter().any(|(_, expr)| refers(expr))
            }
            Some(_) => on_element(e),
            None => on_element(e) || lido_na_deteccao(&e.filhos, name),
        },
        _ => false,
    })
}

/// Does any binding of the element itself read `name` with the implicit
/// receiver?
fn element_references(e: &crate::html::Elemento, name: &str) -> bool {
    let refers = |text: &str| cita_na_raiz(text, name);
    e.propriedades
        .iter()
        .chain(e.eventos.iter())
        .chain(e.bananas.iter())
        .any(|l| refers(&l.valor))
        || e.atributos
            .iter()
            .any(|a| a.valor.contains("{{") && refers(&a.valor))
}

/// Does the embedded view of the `*` (or of the written `<template>`) `e` read
/// the outer `name`? Not if it shadows it: a `let` of it or a `#ref` declared
/// in it.
fn citado_na_embutida(
    e: &crate::html::Elemento,
    name: &str,
    children: &std::collections::HashMap<String, Filho>,
) -> bool {
    let has_ref = |x: &crate::html::Elemento| x.referencias.iter().any(|r| r.nome == name);
    let mut places = Vec::new();
    match &e.estrela {
        Some(star) if star.nome != MARCA_DE_MOLDE => {
            let micro = e.micro_da_estrela().unwrap_or_default();
            // The element of the `*` and what is below it belong to the
            // embedded view.
            where_matches(
                &[No::Elemento(crate::html::Elemento {
                    estrela: None,
                    ..e.clone()
                })],
                &has_ref,
                children,
                false,
                &mut places,
            );
            let shadowed = micro.locais.iter().any(|(l, _)| l == name)
                || places.iter().any(|l| *l != Lugar::Embutida);
            !shadowed
                && (element_references(e, name) || referenced_in_scope(&e.filhos, name, children))
        }
        _ => {
            // Written `<template>`: the content is the embedded view; the
            // `let-x` stay in the element's attributes.
            where_matches(&e.filhos, &has_ref, children, false, &mut places);
            let lets = e
                .atributos
                .iter()
                .any(|a| a.nome.strip_prefix("let-") == Some(name));
            let shadowed = lets || places.iter().any(|l| *l != Lugar::Embutida);
            !shadowed && referenced_in_scope(&e.filhos, name, children)
        }
    }
}

/// The fields created by the binder, in its order: the `_expr_k` of the
/// bindings and, among them, the node that an embedded view reads, at the
/// position where it was bound ([`Corpo::posicao_de_embutida`]; without a
/// position, after all of them).
fn campos_da_deteccao(
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
fn citado_em_embutidas(
    nodes: &[No],
    name: &str,
    children: &std::collections::HashMap<String, Filho>,
) -> bool {
    nodes.iter().any(|n| match n {
        No::Elemento(e) if e.estrela.is_some() => citado_na_embutida(e, name, children),
        No::Elemento(e) => citado_em_embutidas(&e.filhos, name, children),
        _ => false,
    })
}

/// The key of a `#ref` read as a local in the mark and in the map of resolved
/// ones: the name and the view that declares it — the same name may belong to
/// different views (each one resolves its own, [`referencias_locais`]).
fn chave_de_ref(name: &str, view_class: &str) -> String {
    format!("{name}@{view_class}")
}

/// The `k`-th result of `key` in a view (repeated `#ref`s, children of the
/// same type): the mark that [`exportar_refs`] records in the order the nodes
/// were seen.
fn ordinal_key(key: &str, k: usize) -> String {
    format!("{key}\u{3}{k}")
}

/// The `#ref`s of a finished view in the map of resolved ones: by the key with
/// the view ([`chave_de_ref`]) and by the name (that of the queries, which
/// read the node of a nested view by name).
fn exportar_refs(
    resolved: &std::cell::RefCell<std::collections::HashMap<String, String>>,
    refs: &std::collections::HashMap<String, String>,
    detectors: &std::collections::HashMap<String, String>,
    in_order: &[(String, String)],
    detectors_in_order: &[(String, String)],
    view_class: &str,
) {
    let mut r = resolved.borrow_mut();
    for (name, read) in refs {
        r.insert(chave_de_ref(name, view_class), read.clone());
        r.insert(name.clone(), read.clone());
    }
    let mut seen: std::collections::HashMap<&str, usize> = Default::default();
    for (name, read) in in_order {
        let k = seen.entry(name.as_str()).or_default();
        r.insert(
            chave_de_ref(&ordinal_key(name, *k), view_class),
            read.clone(),
        );
        *k += 1;
    }
    let mut seen: std::collections::HashMap<&str, usize> = Default::default();
    for (key, view) in detectors_in_order {
        let k = seen.entry(key.as_str()).or_default();
        r.insert(
            format!(
                "{DETECTOR_MARK}{}",
                chave_de_ref(&ordinal_key(key, *k), view_class)
            ),
            format!("this.{view}"),
        );
        *k += 1;
    }
    for (key, view) in detectors {
        r.insert(
            format!("{DETECTOR_MARK}{key}"),
            format!("this.{view}"),
        );
        r.insert(
            format!(
                "{DETECTOR_MARK}{}",
                chave_de_ref(key, view_class)
            ),
            format!("this.{view}"),
        );
    }
}

/// Replaces the `#ref` marks ([`MARCA_DE_REF`]) with each one's node:
/// `\u{5}x\u{6}` with the read in the view itself (`this._el_3`), and
/// `\u{5}.x\u{6}` with just the field (`_el_3`), read from a nested view. The
/// mark of a node not seen yet stays (the view that creates it comes later).
fn resolver_refs(text: &str, refs: &std::collections::HashMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(MARCA_DE_REF) {
        out.push_str(&rest[..i]);
        let after = &rest[i + MARCA_DE_REF.len_utf8()..];
        let f = after.find(FIM_DE_REF).unwrap_or(after.len());
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
                out.push(MARCA_DE_REF);
                out.push_str(name);
                out.push(FIM_DE_REF);
            }
        }
        rest = after.get(f + FIM_DE_REF.len_utf8()..).unwrap_or("");
    }
    out.push_str(rest);
    out
}

/// The component shapes that can only be decided by looking at the template:
/// where the `#ref` of each `@ViewChild` is. Returns what
/// still prevents generation, in the order it appears.
fn formas_contra_o_template(
    c: &Componente,
    local: &Local,
    nodes: &[No],
    resolver: Option<&dyn Resolucao>,
    children: &std::collections::HashMap<String, Filho>,
    used: &[Usada],
) -> Vec<Recusa> {
    let mut refused = Vec::new();
    let templates = referencias_de_moldes(nodes);
    for query in &c.consultas {
        let mut places = Vec::new();
        // `read:` in the static query of an element of the view (below) or in
        // that of an element in `*` ([`consulta_em_embutida`]): by type, a
        // static list or `<template>` not yet.
        let from_provider = !query.por_tipo
            && !templates.contains(&query.referencia)
            && token_de_leitura(query, local, resolver).is_some();
        // `read: ViewContainerRef` on a `<template #x>`: the value is the
        // node's `ViewContainer`, assigned in `build()` — the static shape (a
        // single `#x`, in the view itself, `@ViewChild`).
        if !query.por_tipo
            && templates.contains(&query.referencia)
            && le_container(query, local, resolver)
        {
            onde_esta(nodes, &query.referencia, children, false, &mut places);
            if query.lista || places.as_slice() != [Lugar::Raiz] {
                refused.push(recusa(
                    Motivo::ViewChildDinamico,
                    "@ViewChild(.., read: ViewContainerRef) of <template> outside the static shape",
                ));
            }
            continue;
        }
        if query.leitura.is_some()
            && !from_provider
            && !consulta_em_embutida(nodes, query, children, local, resolver)
            && (query.por_tipo || query.lista || templates.contains(&query.referencia))
        {
            refused.push(recusa(
                Motivo::ViewChildEmFilho,
                "@ViewChild(.., read: T) outside a static element",
            ));
            continue;
        }
        if query.por_tipo {
            // `@ViewChild(Type)`: the instances of the children of that class.
            // The attribute directive of the same type would also be a result:
            // not yet.
            let Some(uri) = uri_da_consulta(query, local, resolver) else {
                refused.push(recusa(
                    Motivo::ViewChildEmFilho,
                    "@ViewChild(Type) without resolution",
                ));
                continue;
            };
            if consulta_em_embutida(nodes, query, children, local, resolver) {
                continue;
            }
            if consulta_de_token_dinamica(nodes, query, children, used, local, resolver)
                .is_some()
            {
                continue;
            }
            // `@ViewChild(Directive)`: its token is in the
            // `_resolvedProvidersArray` of the element it matches, and the
            // value is its field (`_providers.get(type).build()`, with the
            // `.instance` of an `XNgCd`), assigned at the end of `build()`
            // when the result is in the view itself (case j117).
            if let Some(u) = used
                .iter()
                .find(|u| u.filho.is_none() && u.uri == uri && u.classe == query.referencia)
            {
                let matcher = |e: &crate::html::Elemento| {
                    !children.contains_key(&e.nome)
                        && crate::selector::casa_algum(
                            &u.seletores,
                            &crate::selector::Elemento::do_template(e),
                        )
                };
                where_matches(nodes, &matcher, children, false, &mut places);
                let in_view = !places.is_empty()
                    && places
                        .iter()
                        .all(|l| matches!(l, Lugar::Raiz | Lugar::Projetado));
                if query.lista || !in_view || u.diretiva.is_none() {
                    refused.push(recusa(
                        Motivo::ViewChildEmFilho,
                        "@ViewChild(Directive) outside the static shape",
                    ));
                }
                continue;
            }
            let is_child = |e: &crate::html::Elemento| {
                children
                    .get(&e.nome)
                    .is_some_and(|f| f.uri_dart == uri && f.classe == query.referencia)
            };
            where_matches(nodes, &is_child, children, false, &mut places);
            let only_children = places
                .iter()
                .all(|l| matches!(l, Lugar::NoFilho | Lugar::NoFilhoProjetado));
            let element = is_element_type(&query.tipo, local, resolver);
            // The type has to be a component from `directives:`: a directive,
            // service or `ElementRef` would give other results.
            let is_component = children
                .values()
                .any(|f| f.uri_dart == uri && f.classe == query.referencia);
            if !is_component || !only_children || element || (places.is_empty() && !query.lista) {
                refused.push(recusa(
                    Motivo::ViewChildEmFilho,
                    "@ViewChild(Type) outside the static shape",
                ));
            }
            continue;
        }
        onde_esta(nodes, &query.referencia, children, false, &mut places);
        // `#ref="x"`: the value is the instance of the exported directive (what
        // the `#ref` reads in the refs map, `this._X_n_m`), assigned like that
        // of an element — only in the static shape: a single `#ref`, in the
        // view itself (also in a child's projected content, case j69),
        // `@ViewChild` (not a list).
        if referencia_com_valor(nodes, &query.referencia) {
            // Inside `*`: the exported provider is the result recorded in the
            // nested view (`compile_element.dart:254-262`), read by
            // `mapNestedViews` like the node of an element (§A5).
            if consulta_em_embutida(nodes, query, children, local, resolver) {
                continue;
            }
            // On a child's element, the instance of the exported directive is
            // read the same way (case j109).
            if !(matches!(
                places.as_slice(),
                [Lugar::Raiz | Lugar::Projetado | Lugar::NoFilho | Lugar::NoFilhoProjetado]
            ) && !query.lista)
            {
                refused.push(recusa(
                    Motivo::ViewChildEmFilho,
                    "@ViewChild of #ref with a value (exportAs) outside the static shape",
                ));
            }
            continue;
        }
        // `@ViewChild('t')` of a written `<template #t>` ([`Corpo::molde`]): the
        // value read is the node's `TemplateRef` (the implicit `read` of a
        // `<template>`), assigned in `build()` like that of an element
        // (`_ctx.x = this._TemplateRef_n_7;`). Only the static shape — a
        // single `#t`, in the view itself, an ngdart `TemplateRef` field; a
        // list, a result in `*` or in projected content not yet.
        if templates.contains(&query.referencia) {
            match places.as_slice() {
                [Lugar::Raiz]
                    if !query.lista && is_template_ref(&query.tipo, local, resolver) => {}
                [Lugar::Raiz] if !query.lista => refused.push(recusa(
                    Motivo::ViewChildEmFilho,
                    "@ViewChild of <template> in a field that is not TemplateRef",
                )),
                _ => refused.push(recusa(
                    Motivo::ViewChildDinamico,
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
        if consulta_em_embutida(nodes, query, children, local, resolver) {
            continue;
        }
        // The single query whose first result (in pre-order) is in this view
        // is static and keeps it (`_isStatic`, `_buildQueryResults` stops at
        // the first): those inside `*` that come later do not count (j17).
        if !query.lista && places.len() > 1 && places[0] != Lugar::Embutida {
            places.truncate(1);
        }
        if query.lista || places.len() > 1 {
            let element = is_element_type(&query.tipo, local, resolver);
            let all_nodes = places
                .iter()
                .all(|l| matches!(l, Lugar::Raiz | Lugar::Projetado));
            let all_children = places
                .iter()
                .all(|l| matches!(l, Lugar::NoFilho | Lugar::NoFilhoProjetado));
            let ok = (from_provider && all_nodes)
                || (!from_provider && element && all_nodes)
                || (!from_provider && !element && all_children);
            if !ok {
                refused.push(recusa(
                    Motivo::ViewChildDinamico,
                    "@ViewChildren outside the static shape",
                ));
            }
            continue;
        }
        let r = match places.as_slice() {
            // The node or `ElementRef(node)`, according to the read
            // ([`valor_de_elemento`]): the static query, which comes out in
            // `build()`.
            [Lugar::Raiz | Lugar::Projetado]
                if valor_de_elemento(query, local, resolver).is_some() || from_provider =>
            {
                continue;
            }
            [Lugar::Raiz | Lugar::Projetado] => recusa(
                Motivo::ViewChildEmFilho,
                "@ViewChild(.., read: T) of a token the element does not provide",
            ),
            // The child's instance: the field has to be of its type (not an
            // `Element`) and the child cannot be `onPush`, which registers the
            // query's `ChangeDetectorRef` (`queryChangeDetectorRefs`).
            [Lugar::NoFilho | Lugar::NoFilhoProjetado] => {
                if query.leitura.is_some() {
                    recusa(
                        Motivo::ViewChildEmFilho,
                        "@ViewChild(.., read: T) of a child's #ref",
                    )
                } else if is_element_type(&query.tipo, local, resolver) {
                    recusa(
                        Motivo::ViewChildEmFilho,
                        "@ViewChild of type Element on a child's #ref",
                    )
                } else {
                    continue;
                }
            }
            [Lugar::Filho] => recusa(
                Motivo::ViewChildEmFilho,
                "@ViewChild of #ref in a child component",
            ),
            // Inside `*` the query goes through `mapNestedViews`; without a
            // result, or with two, the rule is different — none of that yet.
            // Without a result, the single query receives nothing
            // (`_createUpdates`, case j82).
            [] => continue,
            [_] => recusa(
                Motivo::ViewChildDinamico,
                "@ViewChild of #ref in an embedded view",
            ),
            _ => recusa(Motivo::ViewChildDinamico, "@ViewChild of a repeated #ref"),
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
        place: Lugar,
        on_push: bool,
        /// How many results with the same key come before it in the same view
        /// (the node is read by [`ordinal_key`]).
        ordinal: usize,
    },
    /// The results of the embedded view of the `*` that starts at `star` (the
    /// start of its binding, which identifies the `*` in the template).
    Nested {
        star: usize,
        items: Vec<QueryItem>,
    },
}

/// The tree of the results of `key` in `nodes` ([`QueryItem`]). The `*` is
/// followed through the content of HTML elements and of `<ng-container>`,
/// also in a child's projected content; a result on a tag directive or inside
/// a written `<template>` is not translated yet.
fn arvore_da_consulta(
    nodes: &[No],
    key: &str,
    children: &std::collections::HashMap<String, Filho>,
) -> Result<Vec<QueryItem>, &'static str> {
    fn walk(
        nodes: &[No],
        key: &str,
        children: &std::collections::HashMap<String, Filho>,
        in_child: bool,
        out: &mut Vec<QueryItem>,
        seen: &mut usize,
    ) -> Result<(), &'static str> {
        for n in nodes {
            let No::Elemento(e) = n else { continue };
            if let Some(star) = &e.estrela {
                let mut inside = Vec::new();
                if star.nome == MARCA_DE_MOLDE {
                    // The written `<template>` belongs to this view; its content,
                    // to its embedded view, mapped like that of a `*`
                    // (`_appEl_n.mapNestedViews`, case j86). The `<template>`
                    // itself as a result is not translated yet.
                    if casa_a_chave(e, key, children) {
                        return Err("query result on a written <template>");
                    }
                    walk(&e.filhos, key, children, false, &mut inside, &mut 0)?;
                    if !inside.is_empty() {
                        out.push(QueryItem::Nested {
                            star: star.inicio,
                            items: inside,
                        });
                    }
                    continue;
                }
                let mut without = e.clone();
                without.estrela = None;
                walk(
                    std::slice::from_ref(&No::Elemento(without)),
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
                        star: star.inicio,
                        items: inside,
                    });
                }
                continue;
            }
            let (place, below) = if let Some(f) = children.get(&e.nome) {
                let place = if in_child {
                    Lugar::NoFilhoProjetado
                } else {
                    Lugar::NoFilho
                };
                if casa_a_chave(e, key, children) {
                    out.push(QueryItem::Value {
                        place,
                        on_push: f.on_push,
                        ordinal: *seen,
                    });
                    *seen += 1;
                }
                (place, true)
            } else if e.nome == "ng-container" {
                (Lugar::Raiz, in_child)
            } else if !dom::tag_html(&e.nome) {
                (Lugar::Filho, true)
            } else if in_child {
                (Lugar::Projetado, true)
            } else {
                (Lugar::Raiz, false)
            };
            if !children.contains_key(&e.nome) && casa_a_chave(e, key, children) {
                out.push(QueryItem::Value {
                    place,
                    on_push: false,
                    ordinal: *seen,
                });
                *seen += 1;
            }
            walk(&e.filhos, key, children, below, out, seen)?;
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
        tree
            .iter()
            .any(|i| matches!(i, QueryItem::Nested { .. }))
    } else {
        matches!(tree.first(), Some(QueryItem::Nested { .. }))
    }
}

/// Is there a result in this view, outside `*` (`hasStaticValues`)?
fn tem_resultado_estatico(items: &[QueryItem]) -> bool {
    items
        .iter()
        .any(|i| matches!(i, QueryItem::Value { .. }))
}

/// Is any result an `onPush` component?
fn tem_detector(items: &[QueryItem]) -> bool {
    items.iter().any(|i| match i {
        QueryItem::Value { on_push, .. } => *on_push,
        QueryItem::Nested { items, .. } => tem_detector(items),
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
fn consulta_em_embutida(
    nodes: &[No],
    query: &crate::component::Consulta,
    children: &std::collections::HashMap<String, Filho>,
    local: &Local,
    resolver: Option<&dyn Resolucao>,
) -> bool {
    let Some(key) = chave_da_consulta(query, local, resolver) else {
        return false;
    };
    let Ok(tree) = arvore_da_consulta(nodes, &key, children) else {
        return false;
    };
    if !query_is_dynamic(&tree, query.lista) {
        return false;
    }
    let element = is_element_type(&query.tipo, local, resolver);
    let read = query.leitura.is_some();
    // `#ref="exportAs"`: the value is the exported instance (the `#ref` reads
    // it in the refs map), in the single query without `read:` (§A5).
    let exported = !query.por_tipo && referencia_com_valor(nodes, &key);
    let terminal = |l: Lugar| match l {
        _ if exported => {
            !read && !query.lista && matches!(l, Lugar::Raiz | Lugar::Projetado)
        }
        Lugar::Raiz if read => {
            !query.por_tipo
                && (valor_de_elemento(query, local, resolver).is_some()
                    || token_de_leitura(query, local, resolver).is_some())
        }
        Lugar::Raiz => element && !query.por_tipo,
        // The child in the content projected into another is also created in
        // this view (the instance, like that of the child on the node itself).
        Lugar::NoFilho | Lugar::NoFilhoProjetado => !read && !element,
        _ => false,
    };
    fn validate(items: &[QueryItem], terminal: &dyn Fn(Lugar) -> bool) -> bool {
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
type Ancoras = std::collections::HashMap<usize, (String, String)>;

/// Per embedded view (the start of the star), the content queries whose result
/// is in it: (position of the first result, dirty field, how many
/// `parentView` up to the query's view, class of that view).
type SujasDeConteudo = std::collections::HashMap<usize, Vec<(usize, u8, String, u32, String)>>;

/// A content query result: the node that provides the token (by position,
/// [`chave_de_no`]) or an embedded view with results.
#[derive(Debug, Clone)]
enum ItemDeConteudo {
    Valor {
        inicio: usize,
        /// The node's `onPush` component is the result: its
        /// `ChangeDetectorRef` goes to `View.queryChangeDetectorRefs`
        /// (`buildChangeDetectorRef`, `compile_element.dart:390-418`).
        on_push: bool,
    },
    Aninhada {
        estrela: usize,
        itens: Vec<ItemDeConteudo>,
    },
}

/// A content query updated during detection (`createDynamicUpdates`), built
/// when the anchors of all views are known.
#[derive(Debug, Clone)]
struct ConteudoDinamico {
    id: usize,
    arvore: Vec<ItemDeConteudo>,
    /// [`chave_de_tipo`] of the token.
    chave: String,
    lista: bool,
    /// `this._FocusListDirective_0_5.instance.listItems`.
    alvo: String,
    /// The class of the query's view.
    classe: String,
}

/// `@ViewChild(ren)(Token)` whose token a directive (or its `providers:`)
/// provides, with a result in `*` (`_shouldMapNestedViews`): the tree of the
/// results ([`arvore_de_token`]), for the content-query machinery with the
/// root in the component's view. `None` for the rest (the child of the class
/// itself goes the usual way, [`consulta_em_embutida`]).
fn consulta_de_token_dinamica(
    nodes: &[No],
    query: &crate::component::Consulta,
    children: &std::collections::HashMap<String, Filho>,
    used: &[Usada],
    local: &Local,
    resolver: Option<&dyn Resolucao>,
) -> Option<Vec<ItemDeConteudo>> {
    if !query.por_tipo || query.leitura.is_some() {
        return None;
    }
    let uri = uri_da_consulta(query, local, resolver)?;
    let class = query.referencia.rsplit('.').next()?.to_string();
    if children
        .values()
        .any(|f| f.uri_dart == uri && f.classe == class)
    {
        return None;
    }
    let token = crate::directives::Token::Classe {
        uri: uri.clone(),
        classe: class.clone(),
    };
    let provides = |d: &crate::directives::Diretiva| {
        (d.uri == uri && d.classe == class) || d.provedores.iter().any(|p| p.token == token)
    };
    let anyone = used
        .iter()
        .any(|u| u.diretiva.as_deref().is_some_and(provides))
        || children
            .values()
            .any(|f| f.metadados.as_deref().is_some_and(provides));
    if !anyone {
        return None;
    }
    let tree = arvore_de_token(nodes, &uri, &class, children, used).ok()?;
    let dynamic = if query.lista {
        tree
            .iter()
            .any(|i| matches!(i, ItemDeConteudo::Aninhada { .. }))
    } else {
        matches!(tree.first(), Some(ItemDeConteudo::Aninhada { .. }))
    };
    dynamic.then_some(tree)
}

fn arvore_de_token(
    nodes: &[No],
    uri: &str,
    class: &str,
    children: &std::collections::HashMap<String, Filho>,
    used: &[Usada],
) -> Result<Vec<ItemDeConteudo>, Recusa> {
    let token = crate::directives::Token::Classe {
        uri: uri.to_string(),
        classe: class.to_string(),
    };
    let provides = |d: &crate::directives::Diretiva| {
        (d.uri == uri && d.classe == class) || d.provedores.iter().any(|p| p.token == token)
    };
    let mut out = Vec::new();
    for node in nodes {
        let No::Elemento(x) = node else { continue };
        if let Some(star) = &x.estrela {
            let mut without = x.clone();
            if star.nome != MARCA_DE_MOLDE {
                without.estrela = None;
                let inside = arvore_de_token(
                    std::slice::from_ref(&No::Elemento(without)),
                    uri,
                    class,
                    children,
                    used,
                )?;
                if !inside.is_empty() {
                    out.push(ItemDeConteudo::Aninhada {
                        estrela: star.inicio,
                        itens: inside,
                    });
                }
                continue;
            }
            // Written `<template>`: the node belongs to this view; the
            // content, to its embedded view.
            without.estrela = None;
            if diretivas_casadas(used, &without).iter().any(|d| provides(d)) {
                return Err(recusa(
                    Motivo::LigacaoEmFilho,
                    "@ContentChildren with a result on the <template> itself",
                ));
            }
            let inside = arvore_de_token(&x.filhos, uri, class, children, used)?;
            if !inside.is_empty() {
                out.push(ItemDeConteudo::Aninhada {
                    estrela: star.inicio,
                    itens: inside,
                });
            }
            continue;
        }
        let child = children.get(&x.nome);
        let matched = child.is_some_and(|f| {
            (f.uri_dart == uri && f.classe == class)
                || f.metadados.as_deref().is_some_and(|m| provides(m))
        }) || diretivas_casadas(used, x).iter().any(|d| provides(d));
        if matched {
            let on_push =
                child.is_some_and(|f| f.on_push && f.uri_dart == uri && f.classe == class);
            out.push(ItemDeConteudo::Valor {
                inicio: x.inicio,
                on_push,
            });
        }
        out.extend(arvore_de_token(&x.filhos, uri, class, children, used)?);
    }
    Ok(out)
}

/// The key of the node at `start` that provides the token of `key`: the result
/// of a content query read from another view.
fn chave_de_no(key: &str, start: usize) -> String {
    format!("{key}\u{3}@{start}")
}

const MARCA_DE_CONTEUDO: char = '\u{12}';
const FIM_DE_CONTEUDO: char = '\u{13}';

/// Mark of the update of a dynamic view query during detection:
/// `\u{e}field|q..|d..|e..\u{f}`, replaced by [`resolver_consultas`] when the
/// anchors of all views are known. The qualifiers (`firstOrNull` of
/// `queries.dart`, `View`, `ElementRef`) go in the mark so that they are
/// allocated in text order by [`resolver_tardios`].
const MARCA_DE_CONSULTA: char = '\u{e}';
const FIM_DE_CONSULTA: char = '\u{f}';

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
    anchors: &'a Ancoras,
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
                format!("{MARCA_DE_REF}{mark}{FIM_DE_REF}")
            } else {
                format!("{receiver}{MARCA_DE_REF}.{mark}{FIM_DE_REF}")
            }
        };
        for item in items {
            match item {
                QueryItem::Value {
                    on_push, ordinal, ..
                } => {
                    let key = ordinal_key(self.key, *ordinal);
                    let node = read(&chave_de_ref(&key, class));
                    let value = match self.element_ref {
                        Some(q) => format!("{q}ElementRef({node})"),
                        None => node,
                    };
                    if *on_push {
                        let v = self.view?;
                        let view = read(&format!(
                            "{DETECTOR_MARK}{}",
                            chave_de_ref(&key, class)
                        ));
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
            inside.len() > 1
                || inside
                    .iter()
                    .any(|i| matches!(i, QueryItem::Nested { .. }))
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
            indentar(&lines.join("\n"), 2)
        ))
    }
}

/// `[a, b]`: on one line, as `DartFormatter` (an "infinite" page) leaves it,
/// unless some item has a line break (a closure) — then one item per line.
fn list_literal(values: &[String]) -> String {
    if !values.iter().any(|v| v.contains('\n')) {
        return format!("[{}]", values.join(", "));
    }
    let items: Vec<String> = values.iter().map(|v| indentar(v, 2)).collect();
    format!("[\n{}\n]", items.join(",\n"))
}

/// Replaces each [`MARCA_DE_CONSULTA`] with the statements of the query
/// update, with the indentation of the line the mark is on.
fn resolver_consultas(
    text: &str,
    dynamic_queries: &[ConsultaDinamica],
    anchors: &Ancoras,
    root_class: &str,
) -> String {
    let mut out = text.to_string();
    for q in dynamic_queries {
        let start = format!("{MARCA_DE_CONSULTA}{}", q.campo);
        while let Some(i) = out.find(&start) {
            let after = &out[i + start.len()..];
            let Some(f) = after.find(FIM_DE_CONSULTA) else {
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
                key: &q.leitura,
                list: q.lista,
                element_ref: qualifiers.get("e").map(String::as_str),
                view: qualifiers.get("d").map(String::as_str),
            };
            let Some((values, mut lines)) = assembly.results(&q.arvore, "this", root_class)
            else {
                break;
            };
            let value = if values.len() == 1 {
                values[0].clone()
            } else {
                list_literal(&values)
            };
            let value = match (q.lista, qualifiers.get("q")) {
                (true, _) => value,
                (false, Some(queries)) => format!("{queries}firstOrNull({value})"),
                (false, None) => format!("{value}.first"),
            };
            lines.push(format!("_ctx.{} = {value};", q.propriedade));
            let line_start = out[..i].rfind('\n').map_or(0, |k| k + 1);
            let indent =
                out[line_start..i].len() - out[line_start..i].trim_start().len();
            let text = lines.join("\n");
            let text = indentar(&text, indent);
            let end = i + start.len() + f + FIM_DE_CONSULTA.len_utf8();
            out.replace_range(i..end, text.trim_start());
        }
    }
    out
}

/// Replaces each [`MARCA_DE_CONTEUDO`] with the assignment of the content
/// query: `_buildQueryResults`/`_mapNestedViews`/`_createUpdatesNested`
/// (`compile_query.dart:200-530`) over its tree, with the indentation of the
/// mark's line. The nodes come out as `#ref` marks ([`chave_de_no`]),
/// replaced later by [`resolver_refs`].
fn resolver_conteudo_dinamico(
    text: &str,
    dynamic_queries: &[ConteudoDinamico],
    anchors: &Ancoras,
) -> String {
    /// The values and the `ChangeDetectorRef` registrations (`view`: the
    /// qualified `View`) of a view, like [`QueryAssembly::results`].
    fn results(
        q: &ConteudoDinamico,
        items: &[ItemDeConteudo],
        receiver: &str,
        class: &str,
        anchors: &Ancoras,
        view: Option<&str>,
    ) -> Option<(Vec<String>, Vec<String>)> {
        let (mut values, mut registrations) = (Vec::new(), Vec::new());
        let read = |mark: &str| {
            if receiver == "this" {
                format!("{MARCA_DE_REF}{mark}{FIM_DE_REF}")
            } else {
                format!("{receiver}{MARCA_DE_REF}.{mark}{FIM_DE_REF}")
            }
        };
        for i in items {
            match i {
                ItemDeConteudo::Valor { inicio, on_push } => {
                    let key = chave_de_ref(&chave_de_no(&q.chave, *inicio), class);
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
                    if !q.lista {
                        break;
                    }
                }
                ItemDeConteudo::Aninhada {
                    estrela,
                    itens: inside,
                } => {
                    let (anchor, class_w) = anchors.get(estrela)?;
                    let (vals, mut lines) =
                        results(q, inside, "nestedView", class_w, anchors, view)?;
                    let several = if q.lista {
                        inside.len() > 1
                            || inside
                                .iter()
                                .any(|x| matches!(x, ItemDeConteudo::Aninhada { .. }))
                    } else {
                        matches!(inside.first(), Some(ItemDeConteudo::Aninhada { .. }))
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
                        indentar(&lines.join("\n"), 2)
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
        let start = format!("{MARCA_DE_CONTEUDO}{}", q.id);
        while let Some(i) = out.find(&start) {
            let after = &out[i + start.len()..];
            let Some(f) = after.find(FIM_DE_CONTEUDO) else {
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
                results(q, &q.arvore, "this", &q.classe, anchors, view)
            else {
                break;
            };
            let value = if values.len() == 1 {
                values[0].clone()
            } else {
                list_literal(&values)
            };
            let value = match (q.lista, queries) {
                (true, _) => value,
                (false, Some(queries)) => format!("{queries}firstOrNull({value})"),
                (false, None) => format!("{value}.first"),
            };
            let mut text = registrations.join("\n");
            if !text.is_empty() {
                text.push('\n');
            }
            text += &format!("{} = {value};", q.alvo);
            let line_start = out[..i].rfind('\n').map_or(0, |k| k + 1);
            let indent =
                out[line_start..i].len() - out[line_start..i].trim_start().len();
            let text = indentar(&text, indent);
            let end = i + start.len() + f + FIM_DE_CONTEUDO.len_utf8();
            out.replace_range(i..end, text.trim_start());
        }
    }
    out
}

/// A view query updated during detection (`createDynamicUpdates`): the
/// "dirty" field, the anchor and the class of the result's embedded view.
#[derive(Debug, Clone)]
struct ConsultaDinamica {
    indice: usize,
    propriedade: String,
    lista: bool,
    /// What to look for ([`chave_da_consulta`]).
    chave: String,
    /// How the result is read on the node: the key itself, or that of the
    /// provider that `read:` asks for ([`chave_de_leitura`]).
    leitura: String,
    /// `_viewQuery_ref_N_isDirty`.
    campo: String,
    /// `read: ElementRef`: the result is `ElementRef(node)`.
    element_ref: bool,
    /// The results in pre-order ([`arvore_da_consulta`]).
    arvore: Vec<QueryItem>,
    /// Some `*` with a result was emitted (without it, the view was refused
    /// and the query is not written).
    vista: bool,
}

/// A hand-written `<template [ngIf]="x">…</template>` is the same
/// `EmbeddedTemplateAst` as `<ng-container *ngIf="x">…</ng-container>`: the
/// children are the roots of the embedded view. Only the shape with a binding
/// to a known single-input structural directive (`ngIf`, `ngSwitchCase`,
/// `ngSwitchWhen`), or only with the `ngSwitchDefault` attribute, and nothing
/// else is rewritten — with the
/// binding's range, which is that of the `REF`; the rest stays `<template>`
/// and is refused.
fn template_como_container(nodes: &[No]) -> Vec<No> {
    nodes.iter()
        .map(|n| match n {
            No::Elemento(e)
                if e.estrela.as_ref().is_some_and(|l| {
                    l.nome != MARCA_DE_MOLDE && Estrutural::conhecida(&l.nome).is_none()
                }) =>
            {
                // `*dir` of another directive: the `<template>` that the
                // microsyntax produces, with the element inside, through the
                // path of the written `<template>`.
                template_como_container(std::slice::from_ref(&No::Elemento(star_template(e))))
                    .remove(0)
            }
            No::Elemento(e) => {
                let mut e = e.clone();
                e.filhos = template_como_container(&e.filhos);
                let clean = e.eventos.is_empty()
                    && e.bananas.is_empty()
                    && e.referencias.is_empty()
                    && e.anotacoes.is_empty()
                    && e.estrela.is_none();
                let binding = match (e.atributos.as_slice(), e.propriedades.as_slice()) {
                    ([], [p])
                        if matches!(p.nome.as_str(), "ngIf" | "ngSwitchCase" | "ngSwitchWhen")
                            && !crate::micro::e_micro(p.valor.trim()) =>
                    {
                        Some(p.clone())
                    }
                    // `<template ngSwitchDefault>`: the attribute only matches
                    // the selector, with no input.
                    ([a], []) if a.nome == "ngSwitchDefault" && a.valor.is_empty() => {
                        Some(a.clone())
                    }
                    _ => None,
                };
                if let (true, "template", Some(l)) = (clean, e.nome.as_str(), binding) {
                    e.estrela = Some(l);
                    e.propriedades.clear();
                    e.atributos.clear();
                    e.nome = "ng-container".into();
                } else if let (true, "template", Some((star, bindings, micro))) =
                    (clean, e.nome.as_str(), molde_com_diretiva(&e))
                {
                    e.estrela = Some(star);
                    e.ligacoes_do_molde = bindings;
                    e.micro_do_molde = Some(micro);
                    e.propriedades.clear();
                    e.atributos.clear();
                    e.nome = "ng-container".into();
                } else if e.nome == "template"
                    && e.estrela.is_none()
                    && e.bananas.is_empty()
                    && e.anotacoes.is_empty()
                    && e.referencias.len() <= 1
                    && e.referencias.iter().all(|r| r.valor.is_empty())
                {
                    // `<template>` with (at most) one `#ref`, `let-x`, and the
                    // attributes, `[x]` and `(x)` of its directives: anchor,
                    // `ViewContainer` and `TemplateRef`, with the content in an
                    // embedded view ([`Corpo::molde`], which resolves the
                    // directives and refuses what none of them receives).
                    // The start is that of the `<template>`: it identifies its
                    // anchor in the view queries, like that of a `*`.
                    e.estrela = Some(crate::html::Ligacao {
                        nome: MARCA_DE_MOLDE.into(),
                        valor: String::new(),
                        inicio: e.inicio,
                        fim: e.inicio,
                        sem_valor: true,
                    });
                }
                No::Elemento(e)
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
fn star_template(e: &crate::html::Elemento) -> crate::html::Elemento {
    let star = e.estrela.clone().unwrap_or_default();
    let micro = e.micro_da_estrela().unwrap_or_default();
    let binding = |name: &str, value: &str| crate::html::Ligacao {
        nome: name.to_string(),
        valor: value.to_string(),
        inicio: star.inicio,
        fim: star.fim,
        sem_valor: false,
    };
    let mut t = crate::html::Elemento {
        nome: "template".into(),
        inicio: star.inicio,
        ..Default::default()
    };
    if micro
        .propriedades
        .first()
        .is_none_or(|(p, _)| *p != star.nome)
    {
        // The desugared `dir` has no value (`EmptyExpr`: `true` in a `bool`
        // input, the `*deferredContent="forceContent: x"`).
        t.atributos.push(crate::html::Ligacao {
            sem_valor: true,
            ..binding(&star.nome, "")
        });
    }
    for (name, key) in &micro.locais {
        let value = if key == "$implicit" {
            ""
        } else {
            key.as_str()
        };
        t.atributos.push(binding(&format!("let-{name}"), value));
    }
    for (name, expr) in &micro.propriedades {
        t.propriedades.push(binding(name, expr));
    }
    let mut inside = e.clone();
    inside.estrela = None;
    inside.micro_do_molde = None;
    t.filhos = vec![No::Elemento(inside)];
    t
}

/// A hand-written `<template dir let-x let-y="key" [dirA]="a" [dirB]="b">` is
/// exactly the `<template>` that the microsyntax of
/// `*dir="let x; let y = key; a: a; b: b"` produces (ngast's
/// `micro/parser.dart` unfolds the `*` into these attributes): the same
/// `EmbeddedTemplateAst`, with the directive matched by the selector
/// (`[ngFor][ngForOf]`). Rewritten as the equivalent `estrela`, it follows
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
fn molde_com_diretiva(
    e: &crate::html::Elemento,
) -> Option<(
    crate::html::Ligacao,
    Vec<crate::html::Ligacao>,
    crate::micro::Micro,
)> {
    let (lets, others): (Vec<_>, Vec<_>) =
        e.atributos.iter().partition(|a| a.nome.starts_with("let-"));
    let mut parts = Vec::new();
    // The equivalent microsyntax, already decomposed: the text (`parts`) only
    // serves as a description, because `a ? b : c; value: x` would not even
    // pass `isMicroExpression` (case j75).
    let mut micro = crate::micro::Micro::default();
    let (dir, properties) = match others.as_slice() {
        [dir] if dir.valor.is_empty() && Estrutural::conhecida(&dir.nome).is_some() => {
            if e.propriedades.is_empty() {
                return None;
            }
            ((*dir).clone(), e.propriedades.iter().collect::<Vec<_>>())
        }
        [] => {
            let (matched, rest): (Vec<_>, Vec<_>) = e
                .propriedades
                .iter()
                .partition(|p| Estrutural::conhecida(&p.nome).is_some());
            let [dir] = matched.as_slice() else {
                return None;
            };
            if dir.valor.trim().is_empty() {
                return None;
            }
            parts.push(dir.valor.trim().to_string());
            micro
                .propriedades
                .push((dir.nome.clone(), dir.valor.trim().to_string()));
            ((*dir).clone(), rest)
        }
        _ => return None,
    };
    for l in &lets {
        let name = &l.nome["let-".len()..];
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        {
            return None;
        }
        let key = l.valor.trim();
        if key.contains([';', '=']) {
            return None;
        }
        parts.push(if key.is_empty() {
            format!("let {name}")
        } else {
            format!("let {name} = {key}")
        });
        micro.locais.push((
            name.to_string(),
            if key.is_empty() {
                "$implicit".to_string()
            } else {
                key.to_string()
            },
        ));
    }
    for p in &properties {
        let suffix = p.nome.strip_prefix(dir.nome.as_str())?;
        let mut cs = suffix.chars();
        let first = cs.next()?;
        if !first.is_ascii_uppercase() || p.valor.trim().is_empty() {
            return None;
        }
        parts.push(format!(
            "{}{}: {}",
            first.to_ascii_lowercase(),
            cs.as_str(),
            p.valor.trim()
        ));
        micro
            .propriedades
            .push((p.nome.clone(), p.valor.trim().to_string()));
    }
    Some((
        crate::html::Ligacao {
            nome: dir.nome.clone(),
            valor: parts.join("; "),
            inicio: dir.inicio,
            fim: dir.fim,
            sem_valor: false,
        },
        e.propriedades.clone(),
        micro,
    ))
}

/// The `#ref` names declared more than once in the template, or with the name
/// of a `let` of some `*`: each view resolves the name by its own scope (the
/// closest wins), which is not translated yet.
fn referencias_ambiguas(nodes: &[No]) -> std::collections::HashSet<String> {
    fn walk(nodes: &[No], refs: &mut Vec<String>, lets: &mut Vec<String>) {
        for n in nodes {
            if let No::Elemento(e) = n {
                refs.extend(e.referencias.iter().map(|r| r.nome.clone()));
                if let Some(micro) = e.micro_da_estrela() {
                    lets.extend(micro.locais.into_iter().map(|(name, _)| name));
                }
                walk(&e.filhos, refs, lets);
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
fn campos_el_em_ordem(
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
fn nos_promovidos(
    nodes: &[No],
    refs: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    refs.iter()
        .filter(|n| !referencia_com_valor(nodes, n))
        .cloned()
        .collect()
}

/// Is there any `#name="value"` (with a value) in the template, at any depth?
fn referencia_com_valor(nodes: &[No], name: &str) -> bool {
    nodes.iter().any(|n| match n {
        No::Elemento(e) => {
            e.referencias
                .iter()
                .any(|r| r.nome == name && !r.valor.is_empty())
                || referencia_com_valor(&e.filhos, name)
        }
        _ => false,
    })
}

/// The `#ref`s of the written `<template>`s ([`MARCA_DE_MOLDE`]), at any
/// depth.
fn referencias_de_moldes(nodes: &[No]) -> Vec<String> {
    let mut out = Vec::new();
    for n in nodes {
        if let No::Elemento(e) = n {
            if e.estrela.as_ref().is_some_and(|l| l.nome == MARCA_DE_MOLDE) {
                out.extend(e.referencias.iter().map(|r| r.nome.clone()));
            }
            out.extend(referencias_de_moldes(&e.filhos));
        }
    }
    out
}

/// Where a `#ref` appears in the template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lugar {
    /// On an HTML element of the view itself.
    Raiz,
    /// Inside an embedded view (`*ngIf`, `*ngFor`).
    Embutida,
    /// On a tag that is neither HTML nor a component (that of a directive), or
    /// below it: a shape still refused for `@ViewChild`.
    Filho,
    /// On the element itself of a child component of the view: the instance
    /// counts.
    NoFilho,
    /// On an HTML element of the content projected into a child: it is still a
    /// node of this view (created detached and handed over in
    /// `createAndProject`).
    Projetado,
    /// On a child component inside the content projected into another: the
    /// instance, also of this view.
    NoFilhoProjetado,
}

fn onde_esta(
    nodes: &[No],
    name: &str,
    children: &std::collections::HashMap<String, Filho>,
    in_child: bool,
    out: &mut Vec<Lugar>,
) {
    let has_ref = |e: &crate::html::Elemento| casa_a_chave(e, name, children);
    where_matches(nodes, &has_ref, children, in_child, out);
}

/// What a query looks for, as a key of [`onde_esta`]: the `#ref`, or
/// [`chave_de_tipo`] of the component of `@ViewChild(Type)` (`None` without
/// resolution).
fn chave_da_consulta(
    query: &crate::component::Consulta,
    local: &Local,
    resolver: Option<&dyn Resolucao>,
) -> Option<String> {
    if query.por_tipo {
        uri_da_consulta(query, local, resolver)
            .map(|u| chave_de_tipo(&u, &query.referencia))
    } else {
        Some(query.referencia.clone())
    }
}

/// The element matches a query's key: it has the `#ref`, or it is a child
/// component of the type of [`chave_de_tipo`].
fn casa_a_chave(
    e: &crate::html::Elemento,
    key: &str,
    children: &std::collections::HashMap<String, Filho>,
) -> bool {
    match key.strip_prefix('\u{7}').and_then(|c| c.rsplit_once('#')) {
        Some((uri, class)) => children
            .get(&e.nome)
            .is_some_and(|f| f.uri_dart == uri && f.classe == class),
        None => e.referencias.iter().any(|r| r.nome == key),
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
fn primeiro_resultado_dinamico(
    nodes: &[No],
    key: &str,
    children: &std::collections::HashMap<String, Filho>,
) -> Option<(usize, usize)> {
    fn walk(
        nodes: &[No],
        key: &str,
        children: &std::collections::HashMap<String, Filho>,
        dynamic: bool,
        ordinal: &mut usize,
    ) -> Option<(usize, usize)> {
        for n in nodes {
            let No::Elemento(e) = n else { continue };
            let here = *ordinal;
            *ordinal += 1;
            let dynamic = dynamic || e.estrela.is_some();
            if dynamic && casa_a_chave(e, key, children) {
                let place = if key.starts_with('\u{7}') {
                    0
                } else {
                    1 + e
                        .referencias
                        .iter()
                        .position(|r| r.nome == key)
                        .unwrap_or(0)
                };
                return Some((here, place));
            }
            if let Some(r) = walk(&e.filhos, key, children, dynamic, ordinal) {
                return Some(r);
            }
        }
        None
    }
    walk(nodes, key, children, false, &mut 0)
}

/// Like [`primeiro_resultado_dinamico`], with the position of the node
/// ([`crate::html::Elemento::inicio`]) of the first result inside `*`.
fn inicio_do_primeiro_resultado_dinamico(
    nodes: &[No],
    key: &str,
    children: &std::collections::HashMap<String, Filho>,
) -> Option<usize> {
    fn walk(
        nodes: &[No],
        key: &str,
        children: &std::collections::HashMap<String, Filho>,
        dynamic: bool,
    ) -> Option<usize> {
        for n in nodes {
            let No::Elemento(e) = n else { continue };
            let dynamic = dynamic || e.estrela.is_some();
            if dynamic && casa_a_chave(e, key, children) {
                return Some(e.inicio);
            }
            if let Some(r) = walk(&e.filhos, key, children, dynamic) {
                return Some(r);
            }
        }
        None
    }
    walk(nodes, key, children, false)
}

/// The position of the first node of this view (outside `*`) that matches
/// `key`.
fn inicio_do_primeiro_resultado_na_visao(
    nodes: &[No],
    key: &str,
    children: &std::collections::HashMap<String, Filho>,
) -> Option<usize> {
    nodes.iter().find_map(|n| {
        let No::Elemento(e) = n else { return None };
        if e.estrela.is_some() {
            return None;
        }
        if casa_a_chave(e, key, children) {
            return Some(e.inicio);
        }
        inicio_do_primeiro_resultado_na_visao(&e.filhos, key, children)
    })
}

/// The fields with an initializer of a view, which open the class
/// (`dart_emitter.dart:198-205`), in the order `ViewStorage` allocates them:
/// the lazy provider when its node is created, the dirty field of a query when
/// its first result in `*` is visited (pre-order of the whole template, the
/// embedded views in place). On the same node, the provider comes first, and
/// the content query before the view query (`_getQueriesFor`).
fn campos_com_inicializador(body: &Corpo) -> Vec<String> {
    let mut all: Vec<(usize, u8, usize, String)> = Vec::new();
    for (k, c) in body.campos_preguicosos.iter().enumerate() {
        let p = body.posicoes_preguicosas.get(k).copied().unwrap_or(0);
        all.push((p, 0, k, c.clone()));
    }
    for (k, (p, c)) in body.sujos_de_conteudo.iter().enumerate() {
        all.push((*p, 1, k, c.clone()));
    }
    for (k, (p, c)) in body.sujos_de_visao.iter().enumerate() {
        all.push((*p, 2, k, c.clone()));
    }
    all.sort_by_key(|(p, kind, k, _)| (*p, *kind, *k));
    all.into_iter().map(|(_, _, _, c)| c).collect()
}

/// Mark, at the start of a root or projection item, that it is already a list
/// (`this.projectedNodes[i]`, `o.ArrayType`).
const RAIZ_LISTA: char = '\u{11}';

/// A text with `{{ }}` converted ([`Corpo::valor_interpolado`]).
struct Interpolada {
    convertidas: Vec<crate::expr::Convertida>,
    imutavel: bool,
    /// The binding's number (`_expr_k`, `currVal_k`).
    k: u32,
    /// The value computed once, when immutable.
    constante: String,
    /// The checked `currVal_k`.
    checagem: String,
    /// The value where it is used (`currVal_k`, or interpolated there).
    na_acao: String,
}

/// A list of nodes as `createFlatArrayForProjectNodes` writes it.
enum ListaPlana {
    /// `const <Object>[]`.
    Vazia,
    /// `<Object>[a, b]`, with the items.
    Literal(Vec<String>),
    /// Any other expression (a projected list, or the concatenation).
    Expressao(String),
    /// Two or more `..addAll(x)` sections: `dart format` puts each one on its
    /// own line, two spaces past the start of the expression.
    Cascata { cabeca: String, secoes: Vec<String> },
}

impl ListaPlana {
    fn texto(&self) -> String {
        self.texto_em(0)
    }

    /// The text with the expression starting at column `column` (that of the
    /// cascade's sections is `column + 2`).
    fn texto_em(&self, column: usize) -> String {
        match self {
            ListaPlana::Vazia => "const <Object>[]".to_string(),
            ListaPlana::Literal(items) => format!("<Object>[{}]", items.join(", ")),
            ListaPlana::Expressao(e) => e.clone(),
            ListaPlana::Cascata { cabeca, secoes } => {
                let indent = " ".repeat(column + 2);
                let mut t = cabeca.clone();
                for x in secoes {
                    t.push_str(&format!("\n{indent}..addAll({x})"));
                }
                t
            }
        }
    }
}

/// `createFlatArrayForProjectNodes` (`view_compiler_utils.dart`): the items
/// that are not lists go together in an `<Object>[..]`; each projected list
/// ([`RAIZ_LISTA`]) is concatenated with `..addAll(..)` — the one that does
/// not open the expression, inside `unsafeCast`. A list alone is itself. With
/// more than one concatenation, the builder's `dart format` breaks the cascade
/// into lines, a shape with no case yet.
fn lista_plana(items: &[String], util: &str) -> Result<ListaPlana, Recusa> {
    let list = |x: &String| x.strip_prefix(RAIZ_LISTA).map(str::to_string);
    match items {
        [] => return Ok(ListaPlana::Vazia),
        [x] => {
            return Ok(match list(x) {
                Some(l) => ListaPlana::Expressao(l),
                None => ListaPlana::Literal(vec![x.clone()]),
            });
        }
        _ => {}
    }
    if !items.iter().any(|x| x.starts_with(RAIZ_LISTA)) {
        return Ok(ListaPlana::Literal(items.to_vec()));
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
        [] => ListaPlana::Expressao(head),
        [x] => ListaPlana::Expressao(format!("{head}..addAll({x})")),
        _ => ListaPlana::Cascata { cabeca: head, secoes: sections },
    })
}

/// How many `<ng-content>` there are in `nodes`, at any depth.
fn conteudos_em(nodes: &[No]) -> u32 {
    nodes.iter()
        .map(|n| match n {
            No::Conteudo { .. } => 1,
            No::Elemento(e) => conteudos_em(&e.filhos),
            _ => 0,
        })
        .sum()
}

/// The key of the results of a `@ViewChild(Type)` among the `#ref`s seen
/// (`Corpo::refs_em_ordem`): it does not collide with a reference name.
fn chave_de_tipo(uri: &str, class: &str) -> String {
    format!("\u{7}{uri}#{class}")
}

/// The library that declares the type of a `@ViewChild(Type)`, in the
/// component's scope.
fn uri_da_consulta(
    query: &crate::component::Consulta,
    local: &Local,
    resolver: Option<&dyn Resolucao>,
) -> Option<String> {
    resolver?.uri_do_tipo(local.caminho, &query.referencia)
}

/// Where, in document order, the elements that match `predicate` are.
/// Where, in pre-order, the first result of the query `key` that belongs to
/// this view (outside `*` and the content of `<template>`) is: the element's
/// ordinal and, inside it, the `_QueryWithRead` order — the queries by type
/// (the node's providers) before those by `#ref`, these in the order of the
/// `#ref`s. `None` when there is none (it goes to the end).
fn primeiro_resultado(
    nodes: &[No],
    key: &str,
    children: &std::collections::HashMap<String, Filho>,
) -> Option<(usize, usize)> {
    fn walk(
        nodes: &[No],
        key: &str,
        children: &std::collections::HashMap<String, Filho>,
        ordinal: &mut usize,
    ) -> Option<(usize, usize)> {
        for node in nodes {
            let No::Elemento(e) = node else { continue };
            *ordinal += 1;
            let template = e.estrela.as_ref().is_some_and(|l| l.nome == MARCA_DE_MOLDE);
            if e.estrela.is_some() && !template {
                continue;
            }
            if casa_a_chave(e, key, children) {
                let inside = if key.starts_with('\u{7}') {
                    0
                } else {
                    1 + e
                        .referencias
                        .iter()
                        .position(|r| r.nome == key)
                        .unwrap_or(0)
                };
                return Some((*ordinal, inside));
            }
            if !template && let Some(p) = walk(&e.filhos, key, children, ordinal) {
                return Some(p);
            }
        }
        None
    }
    let mut ordinal = 0;
    walk(nodes, key, children, &mut ordinal)
}

fn where_matches(
    nodes: &[No],
    predicate: &dyn Fn(&crate::html::Elemento) -> bool,
    children: &std::collections::HashMap<String, Filho>,
    in_child: bool,
    out: &mut Vec<Lugar>,
) {
    for node in nodes {
        let No::Elemento(e) = node else { continue };
        // The written `<template>` belongs to this view; its content does not.
        let template = e.estrela.as_ref().is_some_and(|l| l.nome == MARCA_DE_MOLDE);
        let place = if e.estrela.is_some() && !template {
            Lugar::Embutida
        } else if children.contains_key(&e.nome) {
            if in_child {
                Lugar::NoFilhoProjetado
            } else {
                Lugar::NoFilho
            }
        } else if !dom::tag_html(&e.nome) && e.nome != "ng-container" {
            // `<ng-container>` (also the rewritten `<template [ngIf]>`) is not
            // a node: its children stay where it is (case j78).
            Lugar::Filho
        } else if in_child {
            Lugar::Projetado
        } else {
            Lugar::Raiz
        };
        if predicate(e) {
            out.push(place);
        }
        let mut inside = Vec::new();
        let below_child = in_child
            || matches!(
                place,
                Lugar::Filho | Lugar::NoFilho | Lugar::NoFilhoProjetado | Lugar::Projetado
            );
        where_matches(&e.filhos, predicate, children, below_child, &mut inside);
        // Everything below a `*` belongs to the embedded view.
        if place == Lugar::Embutida || template {
            inside.iter_mut().for_each(|l| *l = Lugar::Embutida);
        }
        out.extend(inside);
    }
}

/// Is the field's type `dart:html`'s `Element` or a subtype of it? Every
/// `…Element` of `dart:html` descends from `Element`, except `NoncedElement`
/// (checked in the `html_dart2js.dart` of SDK 3.6.2).
fn is_element_type(ty: &str, local: &Local, resolver: Option<&dyn Resolucao>) -> bool {
    let ty = ty.trim().trim_end_matches('?');
    let simple = crate::resolucao::sem_ancora(ty.rsplit('.').next().unwrap_or(ty));
    // In 9, `Element` is the one from `package:web` (`find_components.dart:533`).
    resolver
        .and_then(|r| r.uri_do_tipo(local.caminho, ty))
        .is_some_and(|uri| crate::dialect::e_elemento(&uri, simple))
}

/// The value of `[style.x.unit]` when the source is null: `null` in 8, `''` in
/// 9 (`package:web`'s `setProperty` does not accept null,
/// `NX:compiler/view_compiler/update_statement_visitor.dart:145`).
fn estilo_nulo() -> &'static str {
    match crate::dialect::atual() {
        crate::dialect::Dialeto::Ngdart => "null",
        crate::dialect::Dialeto::Ngx => "''",
    }
}

/// The value of `[style.x]` without a unit (`visitStyleBinding`): the value
/// itself if it is a `String`, otherwise `v.toString()` (`v?.toString()` when
/// it may be null); in 9, wrapped in `?? ''` when it may be null
/// (`update_statement_visitor.dart:148-159`).
fn estilo_sem_unidade(value: &str, is_string: bool, nullable: bool) -> String {
    let v = if is_string {
        value.to_string()
    } else if nullable {
        format!("{value}?.toString()")
    } else {
        format!("{value}.toString()")
    };
    if crate::dialect::atual() == crate::dialect::Dialeto::Ngx && nullable {
        format!("({v} ?? '')")
    } else {
        v
    }
}

/// The value of a static query in 9: with an `Element` field (or a list of
/// `Element`, `isElementType`), each result goes into an `unsafeCast(..)`
/// (`NX:compiler/view_compiler/compile_query.dart:473-486`); in 8, as it is.
fn elemento_com_cast(
    value: String,
    query: &crate::component::Consulta,
    local: &Local,
    resolver: Option<&dyn Resolucao>,
) -> String {
    if crate::dialect::atual() == crate::dialect::Dialeto::Ngdart {
        return value;
    }
    let ty = if query.lista {
        tipo_do_elemento(&query.tipo)
    } else {
        Some(query.tipo.clone())
    };
    if ty.is_some_and(|t| is_element_type(&t, local, resolver)) {
        format!("{}unsafeCast({value})", tardio_q(UTILITIES))
    } else {
        value
    }
}

/// The value of a view query whose result is an HTML element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValorDeElemento {
    /// The node itself (`_el_n`).
    No,
    /// `ElementRef(_el_n)`.
    ElementRef,
}

/// What an element gives to a query (`compile_element.dart`, the
/// `queriesWithReads` loop): with `read: ElementRef`, the node's `ElementRef`;
/// with `read: Element`/`HtmlElement` (from `dart:html`), the node; without
/// `read:`, the node if the field is `Element` (`isElementType`), otherwise
/// the `ElementRef`. `None` for `read:` of another token (directive,
/// `ViewContainerRef`…), which cannot be written yet.
fn valor_de_elemento(
    query: &crate::component::Consulta,
    local: &Local,
    resolver: Option<&dyn Resolucao>,
) -> Option<ValorDeElemento> {
    let Some(t) = query.leitura.as_deref() else {
        return Some(if is_element_type(&query.tipo, local, resolver) {
            ValorDeElemento::No
        } else {
            ValorDeElemento::ElementRef
        });
    };
    let uri = resolver.and_then(|r| r.uri_do_tipo(local.caminho, t))?;
    match (t, uri.as_str()) {
        ("ElementRef", ELEMENT_REF) => Some(ValorDeElemento::ElementRef),
        (t, u) if crate::dialect::no_embutido(u, t.rsplit('.').next().unwrap_or(t)).is_some() => {
            Some(ValorDeElemento::No)
        }
        _ => None,
    }
}

/// The token that a query's `read:` asks of a provider of the node (not the
/// node nor `ElementRef`, which [`valor_de_elemento`] reads, nor a built-in):
/// the class, by the library that declares it. The node creates it eagerly
/// (`queriedTokens`) and the query receives the instance.
fn token_de_leitura(
    query: &crate::component::Consulta,
    local: &Local,
    resolver: Option<&dyn Resolucao>,
) -> Option<crate::directives::Token> {
    let t = query.leitura.as_deref()?;
    if valor_de_elemento(query, local, resolver).is_some() {
        return None;
    }
    let uri = resolver?.uri_do_tipo(local.caminho, t)?;
    let token = crate::directives::Token::Classe {
        uri,
        classe: t.to_string(),
    };
    (!token.embutido()).then_some(token)
}

/// The mark of the result of `key` read through the node's provider `t`.
fn chave_de_leitura(key: &str, t: &crate::directives::Token) -> String {
    match t {
        crate::directives::Token::Classe { uri, classe } => format!("{key}\u{4}{uri}#{classe}"),
        other => format!("{key}\u{4}{}", other.nome()),
    }
}

/// `detectChangesInCheckAlwaysViews` of a (non-host) view of a
/// `@changeDetectionLink` component: each public `ViewContainer`, then each
/// child that is also linked (`compile_view.dart:1383-1395`). When empty, the
/// method is not emitted.
fn metodo_de_link(link: bool, anchors: &[String], linked: &[String]) -> String {
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
fn le_container(
    query: &crate::component::Consulta,
    local: &Local,
    resolver: Option<&dyn Resolucao>,
) -> bool {
    let Some(t) = query.leitura.as_deref().map(str::trim) else {
        return false;
    };
    t.rsplit('.').next() == Some("ViewContainerRef")
        && resolver
            .and_then(|r| r.uri_do_tipo(local.caminho, t))
            .is_some_and(|u| u.starts_with("package:ngdart/"))
}

/// The key of the result of `#name` on a `<template>` read as
/// `ViewContainerRef`: the node's `ViewContainer` (`this._appEl_n`).
fn chave_de_container(name: &str) -> String {
    format!("{name}\u{4}ViewContainerRef")
}

/// Is the field's type ngdart's `TemplateRef` (`TemplateRef` or
/// `TemplateRef?`, without a prefix, resolved to `template_ref.dart`)?
fn is_template_ref(ty: &str, local: &Local, resolver: Option<&dyn Resolucao>) -> bool {
    let ty = ty.trim().trim_end_matches('?');
    ty == "TemplateRef"
        && resolver
            .and_then(|r| r.uri_do_tipo(local.caminho, ty))
            .as_deref()
            == Some(TEMPLATE_REF)
}

/// A pipe from `pipes:`, with the URI of the library that declares it.
#[derive(Debug, Clone)]
pub struct PipeUsado {
    pub uri: String,
    pub pipe: crate::component::Pipe,
}

/// The instance of a pipe, a field of a view (`_pipe_date_0`): that of a pure
/// pipe is one per name, in the component's view, created on first use
/// (`compView.purePipes`); that of an impure one is one per call, in the
/// call's view. The number is the `pipeCount` of the view that holds it.
#[derive(Debug, Clone)]
struct PipeInstance {
    nome: String,
    campo: String,
    classe: String,
    /// Import path of the pipe's library (`getImportModulePath`).
    caminho: String,
    /// The view that holds the instance (0 for every pure pipe).
    vista: u32,
    impura: bool,
    /// The constructor receives the `ChangeDetectorRef` (the view itself).
    detector: bool,
    /// `OnDestroy`: `ngOnDestroy()` in the view's `destroyInternal`.
    destroi: bool,
}

/// A `$pipe.name(..)` call: the view where it is and what replaces it — the
/// proxy (`_pipe_date_0_1`, `_PurePipeProxy`, a field of that view) of a pure
/// pipe, or `_pipe_async_1.transform` of an impure one.
#[derive(Debug, Clone)]
struct PipeCall {
    vista: u32,
    instancia: usize,
    proxy: String,
    argumentos: usize,
    /// `o.FunctionType(returnType, paramTypes.sublist(0, argCount))`; empty
    /// for the impure one, which has no proxy.
    tipo: String,
    /// The type mentions `dart:core` (everything that is not `dynamic`).
    core: bool,
}

/// The pipes of a template: the instances and each call, in the order in which
/// the official compiler converts them — that of `bindView`, which descends
/// into the embedded views where they are. It is this order that numbers the
/// proxies, and it is only known by looking at the whole template before
/// emitting: the embedded views here are emitted after the component's.
#[derive(Debug, Default)]
struct PipesDoTemplate {
    instancias: Vec<PipeInstance>,
    chamadas: Vec<PipeCall>,
}

impl PipesDoTemplate {
    fn da_vista(&self, view: u32) -> impl Iterator<Item = &PipeCall> {
        self.chamadas.iter().filter(move |c| c.vista == view)
    }

    /// The pure-pipe proxies of this view, of one instance.
    fn proxies(&self, view: u32, k: usize) -> impl Iterator<Item = &PipeCall> {
        self.da_vista(view)
            .filter(move |c| c.instancia == k && !self.instancias[k].impura)
    }

    /// The imports of the pipe fields of this view, in declaration order: the
    /// class of each instance held here and the `dart:core` of a proxy's type.
    fn imports_dos_campos(&self, view: u32) -> Vec<String> {
        let mut out = Vec::new();
        for (k, inst) in self.instancias.iter().enumerate() {
            if inst.vista == view {
                out.push(inst.caminho.clone());
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
    fn campos(&self, view: u32, imp: &mut Importacoes) -> Vec<String> {
        let mut out = Vec::new();
        for (k, inst) in self.instancias.iter().enumerate() {
            if inst.vista == view {
                let q = imp.q(&inst.caminho);
                out.push(format!("  late final {q}{} {};", inst.classe, inst.campo));
            }
            for c in self.proxies(view, k) {
                out.push(format!("  late final {} {};", c.tipo, c.proxy));
            }
        }
        out
    }

    /// The creation in `build()` (`afterNodes`, after the listeners): the
    /// instance (`createPipeInstance`) and the proxies (`createPureProxy`),
    /// which read the instance through the `parentView` chain (`base`).
    fn criacao(&self, view: u32, base: &str, imp: &mut Importacoes) -> Vec<String> {
        let mut out = Vec::new();
        for (k, inst) in self.instancias.iter().enumerate() {
            if inst.vista == view {
                let q = imp.q(&inst.caminho);
                let arg = if inst.detector { "this" } else { "" };
                out.push(format!(
                    "    this.{} = {q}{}({arg});",
                    inst.campo, inst.classe
                ));
            }
            for c in self.proxies(view, k) {
                out.push(format!(
                    "    this.{} = {}.pureProxy{}({base}.{}.transform);",
                    c.proxy,
                    tardio(PROXIES),
                    c.argumentos,
                    inst.campo
                ));
            }
        }
        out
    }

    /// The `ngOnDestroy()` of the instances of this view
    /// (`bindPipeDestroyLifecycleCallbacks`), in their order.
    fn destruicao(&self, view: u32) -> Vec<String> {
        self.instancias
            .iter()
            .filter(|i| i.vista == view && i.destroi)
            .map(|i| format!("    this.{}.ngOnDestroy();", i.campo))
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

/// The template's pipe table (see [`PipesDoTemplate`]). The pipe of each call
/// is the last one in `pipes:` with that name (`_findPipeMeta`).
fn pipes_do_template(
    nodes: &[No],
    children: &std::collections::HashMap<String, Filho>,
    pipes: &Result<Vec<PipeUsado>, Recusa>,
    asset: &str,
) -> Result<PipesDoTemplate, Recusa> {
    let mut raw = Vec::new();
    let mut next = 1;
    raw_calls(nodes, 0, &mut next, children, &mut raw)?;
    let mut t = PipesDoTemplate::default();
    if raw.is_empty() {
        return Ok(t);
    }
    let pipes = pipes.as_ref().map_err(Clone::clone)?;
    let refuse = |f: &str| recusa(Motivo::PipesUsados, f);
    // The `pipeCount` of each view.
    let mut counters: std::collections::HashMap<u32, usize> = Default::default();
    for (view, name, args) in raw {
        let used = pipes
            .iter()
            .rev()
            .find(|p| p.pipe.nome == name)
            .ok_or_else(|| refuse("pipe that is not in pipes:"))?;
        let p = &used.pipe;
        if let Some(f) = p.fora {
            return Err(refuse(f));
        }
        // More arguments than parameters is a compile error in the official
        // compiler.
        if args > p.parametros.len() {
            return Err(refuse("pipe with too many arguments"));
        }
        let path = || {
            asset_de_uri(&used.uri, "", Path::new(""))
                .and_then(|target| caminho_do_import(asset, &target))
                .ok_or_else(|| refuse("pipe without an import path"))
        };
        let new_instance = |t: &mut PipesDoTemplate,
                            counters: &mut std::collections::HashMap<u32, usize>,
                            view: u32,
                            impure: bool|
         -> Result<usize, Recusa> {
            let n = counters.entry(view).or_default();
            t.instancias.push(PipeInstance {
                campo: format!("_pipe_{name}_{n}"),
                nome: name.clone(),
                classe: p.classe.clone(),
                caminho: path()?,
                vista: view,
                impura: impure,
                detector: p.detector,
                destroi: p.destroi,
            });
            *n += 1;
            Ok(t.instancias.len() - 1)
        };
        // Impure: one instance per call, in its view, called directly
        // (`_call`: `instance.transform(..)`).
        if !p.puro {
            let instance = new_instance(&mut t, &mut counters, view, true)?;
            let proxy = format!("{}.transform", t.instancias[instance].campo);
            t.chamadas.push(PipeCall {
                vista: view,
                instancia: instance,
                proxy,
                argumentos: args,
                tipo: String::new(),
                core: false,
            });
            continue;
        }
        // `Identifiers.pureProxies`: `pureProxy1` to `pureProxy6` checked in
        // `proxies.dart`.
        if args > 6 {
            return Err(refuse("pipe with more than 6 arguments"));
        }
        let params: Option<Vec<String>> = p.parametros[..args]
            .iter()
            .map(|x| x.clone().filter(|x| is_core_type(x)))
            .collect();
        let (Some(params), true) = (params, is_core_type(&p.retorno)) else {
            return Err(refuse("transform with a type outside dart:core or untyped"));
        };
        let core = std::iter::once(&p.retorno)
            .chain(&params)
            .any(|x| x != "dynamic");
        let instance = match t
            .instancias
            .iter()
            .position(|i| i.nome == name && !i.impura)
        {
            Some(k) => k,
            None => new_instance(&mut t, &mut counters, 0, false)?,
        };
        let already = t
            .chamadas
            .iter()
            .filter(|c| c.instancia == instance)
            .count();
        t.chamadas.push(PipeCall {
            vista: view,
            instancia: instance,
            proxy: format!("{}_{already}", t.instancias[instance].campo),
            argumentos: args,
            tipo: format!("{} Function({})", p.retorno, params.join(", ")),
            core,
        });
    }
    // In an embedded view, the order between an impure pipe's instance and a
    // pure pipe's proxies (created by the instance's `create()`, in the
    // component's view) has no case yet.
    for c in &t.chamadas {
        let impure_here = t.instancias.iter().any(|i| i.impura && i.vista == c.vista);
        if c.vista != 0 && impure_here && !t.instancias[c.instancia].impura {
            return Err(refuse("impure pipe and pure pipe in the same embedded view"));
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
    nodes: &[No],
    view: u32,
    next: &mut u32,
    children: &std::collections::HashMap<String, Filho>,
    out: &mut Vec<(u32, String, usize)>,
) -> Result<(), Recusa> {
    let has = |t: &str| t.contains("$pipe");
    for node in nodes {
        match node {
            No::Interpolacao { expr, .. } => pipes_in_expression(expr, view, out)?,
            No::Elemento(e) => {
                if let Some(star) = &e.estrela {
                    if has(&star.valor) {
                        return Err(recusa(Motivo::PipesUsados, "pipe in the input of `*`"));
                    }
                    let v = *next;
                    *next += 1;
                    let mut without_star = e.clone();
                    without_star.estrela = None;
                    let inside = No::Elemento(without_star);
                    raw_calls(std::slice::from_ref(&inside), v, next, children, out)?;
                    continue;
                }
                if e.eventos.iter().any(|l| has(&l.valor)) {
                    return Err(recusa(Motivo::PipesUsados, "pipe in an event"));
                }
                if e.bananas.iter().any(|l| has(&l.valor)) {
                    return Err(recusa(Motivo::PipesUsados, "pipe in [(x)]"));
                }
                let of_child = children.contains_key(&e.nome) || !dom::tag_html(&e.nome);
                // `[x]` and then the interpolated attributes, as
                // `elemento_html` converts them.
                for l in &e.propriedades {
                    if has(&l.valor) {
                        if of_child {
                            return Err(recusa(
                                Motivo::PipesUsados,
                                "pipe in a child component binding",
                            ));
                        }
                        pipes_in_expression(&l.valor, view, out)?;
                    }
                }
                for a in e.atributos.iter().filter(|a| a.valor.contains("{{")) {
                    if !has(&a.valor) {
                        continue;
                    }
                    if of_child {
                        return Err(recusa(
                            Motivo::PipesUsados,
                            "pipe in a child component binding",
                        ));
                    }
                    let Some((_, exprs)) = partes_da_interpolacao(&a.valor) else {
                        return Err(recusa(Motivo::PipesUsados, "pipe in an unreadable attribute"));
                    };
                    for x in &exprs {
                        pipes_in_expression(x, view, out)?;
                    }
                }
                raw_calls(&e.filhos, view, next, children, out)?;
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
) -> Result<(), Recusa> {
    let shape = || {
        recusa(
            Motivo::PipesUsados,
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
        let (end, args) = argumentos_da_chamada(text, k).ok_or_else(shape)?;
        pipes_in_expression(&text[k + 1..end - 1], view, out)?;
        out.push((view, text[start..j].to_string(), args));
        i = end;
    }
    Ok(())
}

/// From the `(` at `open`: the index after the closing `)` and how many
/// arguments there are (commas at the first level, plus one).
fn argumentos_da_chamada(text: &str, open: usize) -> Option<(usize, usize)> {
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
pub struct Filho {
    pub classe: String,
    pub seletor: String,
    /// `package:` URI of the `.dart` that declares the class.
    pub uri_dart: String,
    /// `package:` URI of its `.template.dart`.
    pub uri_template: String,
    /// `ngContentSelectors` of the child's template: the `select` of each
    /// `<ng-content>`, in order, `*` without it. With any, whoever uses the
    /// child calls `createAndProject` with one list of nodes per projection.
    pub projecoes: Vec<String>,
    /// The child's `@Input`s, in the order the official compiler writes them.
    pub entradas: Vec<crate::component::Entrada>,
    /// The child's lifecycle hooks: whoever uses it calls them
    /// (`bindDirectiveDetectChangesLifecycleCallbacks`,
    /// `bindDirectiveAfterChildrenCallbacks`).
    pub ganchos: crate::component::Ganchos,
    /// `onPush`: whoever changes an input marks the child for check.
    pub on_push: bool,
    /// `@changeDetectionLink`: in a view of a component that is also linked,
    /// `detectChangesInCheckAlwaysViews` descends into its view.
    pub link_de_deteccao: bool,
    /// `@HostBinding`: whoever uses it calls `detectHostChanges(firstCheck)`
    /// before detecting its view (`bindDirectiveHostProps`).
    pub hospedeiro: bool,
    /// `@Output`s (name in the template, member), in the order of the
    /// `outputs` map.
    pub saidas: Vec<(String, String)>,
    /// The child's `hostAttributes`: `@HostBinding` on an immutable static
    /// member outside `class.x`/`style.x` (`_computeHostBindingImmutability`),
    /// by the attribute name (without `attr.`) and the member. Whoever uses it
    /// merges them with the written attribute of the same name
    /// (`_mergeHtmlAndDirectiveAttrs`).
    pub atributos_do_hospedeiro: Vec<(String, String)>,
    /// What the child's constructor receives, in order.
    pub parametros: Vec<Injetado>,
    /// The child's `@ContentChild`/`@ContentChildren`, with the target
    /// resolved: (field, list, target). Whoever projects content into it
    /// updates the query.
    pub consultas: Vec<ConsultaDoFilho>,
    /// What in the child changes the code of whoever uses it and the emitter
    /// does not write yet: constructor injection, `@HostBinding`, content
    /// query, providers, projection with a selector. Any of them refuses the
    /// use.
    pub pendencias: Vec<Recusa>,
    /// The component's metadata read from the program (`metadata.rs`): its
    /// `providers:` go into the node of whoever uses it.
    pub metadados: Option<std::sync::Arc<crate::directives::Diretiva>>,
}

/// A parameter of a child's constructor, as the official compiler resolves it
/// on its node (`provider_resolver.dart`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Injetado {
    /// `Element`/`HtmlElement`: the node itself.
    Elemento,
    /// `ChangeDetectorRef`: the child's view.
    Detector,
    /// `ViewContainerRef`: the `ViewContainer` of the child's node
    /// (`_appEl_n`).
    Container,
    /// A service from outside the view: `injectorGet` through the view above
    /// (`injectFromViewParentInjector`), `injectorGetOptional` with
    /// `@Optional()`.
    Servico {
        /// The class, or the `OpaqueToken`/`MultiToken` of `@Inject(..)`.
        token: crate::directives::Token,
        opcional: bool,
        /// `@Self()`, `@Host()`, `@SkipSelf()` (`_getDependency`).
        proprio: bool,
        hospedeiro: bool,
        pular: bool,
    },
    /// `@Attribute('name')`: the literal value of the attribute on the child's
    /// element, or `null` (`_getLocalDependency`).
    Atributo(String),
}

/// Where a service that the child injects comes from (`_getDependency`).
enum OrigemDoServico {
    /// A provider of the node itself, created before the child.
    Local(String),
    /// An element above provides it: its read.
    Cima(String),
    /// An `@Optional()` that is not found (`@Self`, or `@Host` outside the
    /// host view).
    Nulo,
    /// The injector from outside the view.
    Injetor,
}

/// An `@ContentChild`/`@ContentChildren` of a child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultaDoFilho {
    pub campo: String,
    /// `@ContentChildren`.
    pub lista: bool,
    pub alvo: AlvoDeConsulta,
    pub descendentes: bool,
    /// `read:`: what is read from the node found, instead of the instance that
    /// matched.
    pub leitura: Option<LeituraDaConsulta>,
}

/// The `read:` of a child's content query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeituraDaConsulta {
    /// `HtmlElement`/`Element` of `dart:html`: the node itself.
    Elemento,
    /// Another provider of the node found, by class (URI, name).
    Classe(String, String),
}

/// What a child's content query looks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlvoDeConsulta {
    /// A class (URI, name).
    Classe(String, String),
    /// A `#ref` of the content.
    Referencia(String),
}

impl Filho {
    /// The member that the `@Output` `name` exposes.
    pub fn saida(&self, name: &str) -> Option<&str> {
        self.saidas
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, m)| m.as_str())
    }

    /// The `@Input` that receives the binding `name`.
    pub fn entrada(&self, name: &str) -> Option<&crate::component::Entrada> {
        self.entradas.iter().find(|e| e.nome == name)
    }
}

/// A directive or component from `directives:`, already resolved, with the
/// selector read. A component's list follows the official compiler's order
/// (the depth-first expansion of the constant lists, without repetition): it
/// is the order in which the directives of a node are instantiated.
#[derive(Debug, Clone)]
pub struct Usada {
    pub classe: String,
    /// URI of the library that declares the class.
    pub uri: String,
    pub seletores: Vec<crate::selector::Seletor>,
    /// `Some` when it is a component: what the emitter knows about it.
    pub filho: Option<Filho>,
    /// The metadata read from the program (`metadata.rs`), when it is a
    /// directive and a program is loaded.
    pub diretiva: Option<std::sync::Arc<crate::directives::Diretiva>>,
}

impl Usada {
    /// Is it ngdart's own structural directive that the emitter knows
    /// (`NgIf`, `NgFor`)?
    fn e_estrutural(&self, e: &Estrutural) -> bool {
        self.classe == e.classe && self.uri == e.uri
    }
}

/// What the emitter needs to know about where the component lives.
pub struct Local<'a> {
    /// Package name (`new_sali_frontend`).
    pub pacote: &'a str,
    /// Path of the `.dart` inside the package, with slashes:
    /// `lib/src/x/foo.dart`.
    pub relativo: &'a str,
    /// File name for the `import` of itself: `foo.dart`.
    pub arquivo: &'a str,
    /// Path on disk, to ask the semantic database in which scope the
    /// constructor's names are resolved.
    pub caminho: &'a Path,
    /// Package root, to map `file:` URIs of the project itself.
    pub raiz: &'a Path,
    /// `package:` URI of the template's `.html` file, when there is one. It is
    /// where the `/* REF:url:inicio:fim */` comment that the official compiler
    /// writes on each binding points to.
    pub url_do_template: Option<String>,
    /// The component's own metadata read from the program (`metadata.rs`): its
    /// `providers:` go to the host view.
    pub metadados: Option<std::sync::Arc<crate::directives::Diretiva>>,
}

impl Local<'_> {
    /// `package:` URI of the compiled stylesheet of a `styleUrls`: the
    /// `.css.shim.dart` (with shim) or, with `ViewEncapsulation.none`, the
    /// `.css.dart` (ngcompiler's `stylesModuleUrl(url, shim)`).
    pub(crate) fn uri_do_estilo(&self, url: &str, shim: bool) -> Option<String> {
        let suffix = if shim { ".shim.dart" } else { ".dart" };
        // A written `package:` URI: it is already the stylesheet's
        // (`button_decorator.scss.css` of `dropdown_button`).
        if url.starts_with("package:") {
            return Some(format!("{url}{suffix}"));
        }
        let inside = self.relativo.strip_prefix("lib/")?;
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
            self.pacote,
            parts.join("/")
        ))
    }

    /// `asset:` URI of this file — the space in which the official emitter
    /// computes the import paths.
    fn asset(&self) -> String {
        format!("asset:{}/{}", self.pacote, self.relativo)
    }
}

/// A property binding already translated: the action of a constant one (it
/// goes into `if (firstCheck)`) or the whole block of a dynamic one.
enum Ligada {
    Constante(String),
    Dinamica(String),
}

/// Body of a view's `build()`, assembled while walking the template.
struct Corpo<'a> {
    linhas: Vec<String>,
    /// `addEventListener` of each event. The official compiler creates all the
    /// nodes first and binds afterwards (`_buildView` and only then
    /// `bindView`, in `view_compiler.dart`), so the listeners come out
    /// together, after the last node, in document order.
    ouvintes: Vec<String>,
    /// `#ref`s accepted in this template ([`referencias_livres`]); empty in
    /// the embedded view.
    refs_livres: std::collections::HashSet<String>,
    /// Element `#ref`s of this view read by its expressions
    /// ([`referencias_locais`]): the node becomes a field and the name, a
    /// local (`final local_x = this._el_n;`).
    refs_locais: std::collections::HashSet<String>,
    /// The `#ref`s repeated or shadowed by `let`
    /// ([`referencias_ambiguas`]): their per-view scope is not translated
    /// yet, and the refusal says so.
    refs_ambiguos: std::collections::HashSet<String>,
    /// The local `#ref`s of the ancestor views (see [`EspecEmbutida`]).
    refs_ancestrais: Vec<(String, String, u32)>,
    /// In the component's view: the view queries updated during detection
    /// ([`consulta_em_embutida`]).
    consultas_dinamicas: Vec<ConsultaDinamica>,
    /// The dynamic queries whose result is below this embedded view: (`#ref`,
    /// dirty field, levels up to the component's view).
    consultas_em_transito: Vec<(String, String, u32)>,
    /// The anchors of the dynamic queries, from the [`Contexto`].
    ancoras_de_consulta: &'a std::cell::RefCell<Ancoras>,
    /// [`Contexto::conteudo_dinamico`].
    conteudo_dinamico: &'a std::cell::RefCell<Vec<ConteudoDinamico>>,
    /// [`Contexto::sujas_de_conteudo`].
    sujas_de_conteudo: &'a std::cell::RefCell<SujasDeConteudo>,
    /// The updates of this view's dynamic content queries
    /// (`updateContentQuery`), in the block of the content hooks, after the
    /// view ones and before the hooks, in the order of the nodes'
    /// `afterChildren`.
    atualizacoes_de_conteudo: Vec<String>,
    /// Those of the view queries by token ([`consulta_de_token_dinamica`]),
    /// with the query's index (the order in `afterNodes`).
    atualizacoes_de_visao: Vec<(usize, String)>,
    /// The indices of these queries (outside the static assignment).
    consultas_por_token: std::collections::HashSet<usize>,
    /// The `bool _query_X_n_i_isDirty = true;` fields of these queries, with
    /// the position of the first result in `*` (when the field is allocated).
    sujos_de_conteudo: Vec<(usize, String)>,
    /// How many content queries each node has already registered
    /// (`_queryCount`).
    consultas_por_no: std::collections::HashMap<u32, usize>,
    /// In the embedded view: the `#ref`s that are a result of a query of the
    /// component's view (the node becomes a field) and the "dirty" field of
    /// each one.
    refs_consultados: Vec<(String, String, u32)>,
    /// Each `#ref` seen, with the node's expression (`_el_3` or `this._el_3`)
    /// — it is the value that `@ViewChild` receives.
    refs: std::collections::HashMap<String, String>,
    /// The same, in document order and with repetition: the results of a
    /// `@ViewChildren` (`addQueryResult`).
    /// [`Contexto::leituras`].
    leituras: &'a [(String, crate::directives::Token)],
    /// [`Contexto::refs_so_por_provedor`].
    refs_so_por_provedor: &'a std::collections::HashSet<String>,
    /// [`Contexto::moldes_com_container`].
    moldes_com_container: &'a std::collections::HashSet<String>,
    /// [`Contexto::tipos_de_diretiva`].
    tipos_de_diretiva: &'a [TipoDeDiretivaResolvido],
    refs_em_ordem: Vec<(String, String)>,
    /// The tag of the element whose bindings are being written: the security
    /// context of a property depends on it ([`saneador`]).
    tag_atual: String,
    /// The inherited namespace prefix (`svg`, `math`): `_NamespaceVisitor`
    /// passes the parent's to the descendants
    /// (`ast_template_parser.dart:1331-1352`).
    ns_atual: Option<String>,
    /// The `appViewInstance` of the node whose bindings are being written:
    /// `this`, or the component's view on its element (`this._compView_n`, the
    /// `[class]` of a child's element, case j74).
    instancia_da_visao: Option<String>,
    /// The view passed to the `detectHostChanges` of a directive with
    /// `@HostBinding` on the node: that of the child component, on its node
    /// (case j92); `None` is the view itself (`this`).
    vista_do_hospedeiro: Option<String>,
    /// `TextBinding` and `_message_N` fields, which come first in the class,
    /// in the order `build()` allocates them.
    campos: Vec<String>,
    /// Prefix of `package:intl` when the view has `@i18n` that the emitter
    /// translates (only the component's); `None` refuses the annotation.
    intl: Option<String>,
    /// The messages already created (`createI18nMessage` reuses an equal
    /// one): the `Intl.message(..)` call and the field.
    mensagens: Vec<(String, String)>,
    /// `Object? _expr_k` fields of the bindings, in the order they appear.
    campos_expr: Vec<String>,
    /// `late final T _el_n` fields of the elements with a binding.
    campos_el: Vec<String>,
    /// The `#ref`s (without a value) of the node of each field of
    /// [`Corpo::campos_el`].
    refs_dos_campos_el: std::collections::HashMap<String, Vec<String>>,
    /// The `#ref`s of this view read only in an event handler: the node
    /// becomes a field when `NodeReferenceStorageVisitor` reaches the
    /// `_handleEvent_N` that reads it — after those promoted in detection
    /// (case j60).
    refs_so_em_eventos: std::collections::HashSet<String>,
    /// The fields of these nodes waiting for the first handler that reads
    /// them.
    campos_el_de_eventos: Vec<(String, String)>,
    /// The same, in the order the handlers read them.
    campos_el_por_evento: Vec<String>,
    /// Fields of the nodes of this embedded view that are a result of a
    /// dynamic query of the component's view, with the query's position in
    /// [`Corpo::refs_consultados`]: the official compiler promotes them to
    /// fields when the component's view writes the `mapNestedViews`
    /// (`replaceReadClassMemberInExpression`), before promoting the nodes that
    /// this view's bindings read — they come before [`Corpo::campos_el`], in
    /// query order.
    campos_el_consultados: Vec<(usize, String)>,
    /// Fields of the nodes that an embedded view reads (a `#ref` of this view
    /// read from inside a `*`), with the name: they go among the `_expr_k`, at
    /// the position of [`Corpo::posicao_de_embutida`]
    /// ([`citado_em_embutidas`]).
    campos_el_de_embutidas: Vec<(String, String)>,
    /// The local `#ref`s of this view read from an embedded view.
    refs_de_embutidas: std::collections::HashSet<String>,
    /// How many `_expr_k` existed when the first embedded view that reads each
    /// of them was bound.
    posicao_de_embutida: std::collections::HashMap<String, usize>,
    /// Next binding index (`_expr_k`, `currVal_k`).
    proxima_ligacao: u32,
    /// Body of `detectChangesInternal`.
    deteccao: Vec<String>,
    /// Prefix of `text_binding.dart`, allocated before the rest when the
    /// template has interpolation (the order of the imports follows the order
    /// in which the official compiler writes the file, and the fields come
    /// first).
    /// Some `TextBinding` field was created in this view.
    tb_usado: bool,
    tb: Option<String>,
    /// `package:` URI of the template file, for the `REF` comment.
    url_do_template: Option<String>,
    /// Types of the component's members, to choose `interpolateString`.
    membros: &'a std::collections::HashMap<String, crate::component::Membro>,
    exportados: &'a Exportados,
    /// [`Contexto::nomes_genericos`].
    nomes_genericos: &'a [String],
    /// [`Contexto::tokens_de_visao`].
    tokens_de_visao: &'a [crate::directives::Token],
    /// Methods of the class, valid only as a call target.
    metodos: &'a std::collections::HashMap<String, String>,
    /// Positional parameters of each method, for the event tear-off.
    aridades: &'a std::collections::HashMap<String, usize>,
    /// The `_handleEvent_N` methods of this view, in the order they were
    /// created (`createEventHandler`); they come after `destroyInternal`.
    metodos_evento: Vec<String>,
    /// The type of the `$event` of the next `_handleEvent_N` (in 9: that of
    /// the native event or of the output, qualified by a deferred mark);
    /// `None`, raw.
    tipo_do_evento: Option<String>,
    /// Why the type of `$event` is not known: only the complex handler (the
    /// `_handleEvent_N`) needs it, and it is the one that refuses.
    tipo_do_evento_falha: Option<Recusa>,
    /// The `static String _message_N(..)` methods of the `@i18n` messages
    /// with HTML, which open the view's methods.
    metodos_i18n: Vec<String>,
    /// Declaration (`final local_x = …;`) of each local of **this** view, or
    /// why it cannot be declared. A local of an ancestor view is not here: it
    /// is read through the `parentView` chain, a shape still refused.
    decl_locais: std::collections::HashMap<String, Result<String, Recusa>>,
    /// The locals read by the detection bindings, in order of first use (the
    /// `_localsInScope` of the view's `ViewNameResolver`): it is the order of
    /// the declarations at the top of `detectChangesInternal`.
    locais_raiz: Vec<String>,
    /// Class of this view (`_ViewX2`) and the locals it declares itself
    /// (name, key), for the nested views to read through `parentView`.
    classe_desta: String,
    locais_proprios: Vec<(String, String)>,
    /// The locals of the ancestor views, with where they come from.
    ancestrais: std::collections::HashMap<String, Origem>,
    /// The injectable providers of the elements above the node, the farthest
    /// first: (token, field, and the ancestor view that has it — class and how
    /// many `parentView` up to it — or nothing, if it is this one's).
    acima: Vec<(crate::directives::Token, String, Option<(String, u32)>)>,
    /// The fields of `acima` that are lazy providers of the owner element
    /// (field, class of its view): a node below that requests them would
    /// transform them before the owner's `afterElement` (gap L1 of section
    /// 05).
    preguicosos_acima: std::collections::HashSet<(String, String)>,
    /// Those whose owner modelled the content's requests (a child's node):
    /// from another view, read through the lazy field.
    preguicosos_com_pedidos: std::collections::HashSet<(String, String)>,
    /// How many component elements there are above (providers that the
    /// emitter does not model).
    componentes_acima: u32,
    /// Components above whose providers are not known (without program
    /// metadata): what the node does not find above may be in them.
    incertos_acima: u32,
    /// Components that this template may use, by selector.
    filhos: &'a std::collections::HashMap<String, Filho>,
    /// All the directives and components of `directives:`, to know what
    /// matches each element.
    usadas: &'a [Usada],
    /// Collection mode (the scoreboard diagnostic): each refusal is noted here
    /// and the emission goes on, to find the others. `None`: the first
    /// refusal stops it, and the file is left to `build_runner`.
    coleta: Option<Vec<Recusa>>,
    /// Fields of the lazy providers (`late T _X_n_m = value;`): `ViewStorage`
    /// allocates them when building the view, before promoting nodes and text
    /// bindings to fields, so they open the class (case j38).
    campos_preguicosos: Vec<String>,
    /// The position of each one's node ([`crate::html::Elemento::inicio`]):
    /// the allocation order among the fields with an initializer.
    posicoes_preguicosas: Vec<usize>,
    /// The dirty fields of the dynamic view queries, with the position of
    /// their first result (see [`campos_com_inicializador`]).
    sujos_de_visao: Vec<(usize, String)>,
    /// Fields of the child views (`_compView_n` and the instance), which come
    /// out in the class after the text bindings.
    campos_filho: Vec<String>,
    /// `_compView_n` of each child, for detection and destruction.
    vistas_filhas: Vec<String>,
    /// Those of the `@changeDetectionLink` children ([`metodo_de_link`]).
    vistas_ligadas: Vec<String>,
    /// Asset of this file, to compute the import paths of the children.
    asset: String,
    /// Semantic database and the file, to type chains like `item.nome`.
    tipos: Option<(&'a dyn Resolucao, &'a Path)>,
    /// The component's class with its import prefix (`import1.Classe`): the
    /// receiver of the static members.
    classe_qualificada: String,
    /// The component has a stylesheet: each element gets `addShimC`.
    com_estilo: bool,
    /// Embedded views to emit. Only the specification: the text is generated
    /// after `angular.dart`, which is where the official compiler writes these
    /// classes — and it is the writing order that numbers the imports.
    embutidas: Vec<EspecEmbutida>,
    /// Name of the view class (`ViewX`), to number the embedded views.
    classe_da_visao: String,
    /// The type arguments of a generic component (`<T, U>`), empty for the
    /// rest: the factory of each embedded view is called with them.
    genericos: String,
    /// `preserveWhitespace: true`: the ends of the interpolations are not
    /// compressed (`_compressWhitespace*`).
    preservar_espacos: bool,
    /// `_appEl_n` of each `ViewContainer`, for detection and destruction.
    ancoras: Vec<String>,
    /// This view is embedded: the root is registered with `initRootNode`.
    embutida: bool,
    /// Locals of the scope (`let item of itens`), which shadow the members.
    locais: std::collections::HashMap<String, crate::expr::Local>,
    /// Next embedded view number. The official compiler uses a single
    /// depth-first counter (`_view.viewIndex + _nestedViewCount` in
    /// `view_builder.dart`): the first top-level `*` is 1, the one nested
    /// inside it is 2, and the next sibling is 3.
    proxima_embutida: u32,
    /// To parse the template's expressions, which are Dart expressions.
    nomes: &'a mut dartforge_intern::Interner,
    /// `bool firstCheck = this.firstCheck;` in `detectChangesInternal`, when
    /// some immutable binding is written only on the first check.
    usa_primeira_checagem: bool,
    /// Inputs of child directives and components (`@Input`, `ngIf`,
    /// `ngForOf`) and their `ngDoCheck`: the `detectChangesInInputsMethod`,
    /// which the official compiler writes **before** the nested views and the
    /// property and text bindings (`writeChangeDetectionStatements`).
    entradas: Vec<String>,
    /// Next node index. It counts elements and texts together, in document
    /// order; a comment does not consume an index because it disappears
    /// earlier.
    proximo: u32,
    /// `final doc = …` comes out once, on the first element.
    tem_doc: bool,
    /// `final _ctx = this.ctx;` at the top of `build()`, when some immutable
    /// expression is computed there.
    usa_ctx_no_build: bool,
    /// Ordinal of the next `<ng-content>` — it is the second argument of
    /// `project`, and does not consume a node index.
    proxima_projecao: u32,
    imp: &'a mut Importacoes,
    /// The template's pipes, with each view's call.
    pipes: &'a PipesDoTemplate,
    /// Number of this view (0 the component's, the `indice` in the embedded
    /// one) and how many `parentView` separate it from the component's view.
    vista: u32,
    profundidade: u32,
    /// How many pipe calls of this view have already been converted.
    cursor_pipe: usize,
    /// `changed` is read or written: `bool changed = false;` at the top of
    /// detection.
    usa_changed: bool,
    /// The children's `ngAfterContent*` and `ngAfterView*` hooks, in the
    /// official compiler's order (bottom-up), already at method level: they go
    /// inside `if ((!debugThrowIfChanged))`.
    apos_conteudo: Vec<String>,
    apos_visao: Vec<String>,
    /// The children's `ngOnDestroy`, at the end of `destroyInternal`.
    destruir: Vec<String>,
    /// How many `subscription_N` (a child's `@Output`) this view has.
    subscricoes: usize,
    /// The children whose projected content we are in (URI, class): one of
    /// them, if injected, would be resolved right here, not by the injector
    /// above.
    filhos_acima: Vec<(String, String)>,
    /// `#ref` of an `onPush` child, with its view: `@ViewChild` registers the
    /// `ChangeDetectorRef` (`queryChangeDetectorRefs`).
    /// The same registrations as [`Corpo::detectores`], in node order (one
    /// per result, also with a repeated `#ref`).
    detectores_em_ordem: Vec<(String, String)>,
    detectores: std::collections::HashMap<String, String>,
    /// The nodes created without a parent (root of the embedded view,
    /// projected content), as they are read later: `_el_3`, `this._el_3`,
    /// `this._appEl_4`.
    raizes: Vec<String>,
    /// The index of the child whose projected content is being created: it is
    /// the parent of a detached anchor there (`parent.nodeIndex`).
    pai_projetado: Option<u32>,
    /// The depth of the view where the topmost node of the injector chain of
    /// a node of this view is (`ProviderResolver._getDependency` goes up to
    /// the child of the root of the component's view; the root of an embedded
    /// view has as its parent the parent of its anchor).
    nivel_do_topo: u32,
    /// The nodes with injectable providers (`ProviderNode`), in pre-order:
    /// (first index, last index of the subtree, [(tokens, field)]).
    injetores: Vec<NoInjetor>,
    /// The elements we are in (index, whether it has a directive or
    /// component): the `parent` chain of `_getQueriesFor`.
    pilha: Vec<(u32, bool)>,
    /// The providers created in this view, in pre-order, with the chain of
    /// elements above each one: it is from them that the static result of a
    /// content query comes (`addQueryResult`).
    registros: Vec<Registro>,
}

/// A node with providers, for the content queries: the tokens by which each
/// instance matches a query and its field.
#[derive(Debug, Clone)]
struct Registro {
    acima: Vec<(u32, bool)>,
    /// (token, field, and the child's view when it is an `onPush` component:
    /// the provider's `buildChangeDetectorRef`).
    provedores: Vec<(crate::directives::Token, String, Option<String>)>,
    /// The node as `build()` reads it (`_el_3` or `this._el_3`), for
    /// `read: HtmlElement`.
    elemento: String,
}

impl Corpo<'_> {
    /// The class of this view, for the `unsafeCast` of whoever reads it from
    /// below.
    /// The type arguments from `directiveTypes:` for the directive `class`
    /// (from `uri`) on node `e` (`lookupTypeArgumentsOf`, `compile_view.dart:
    /// 1523-1569`): the `Typed` whose `on:` equals a `#ref` of the node wins;
    /// otherwise the first one without `on:`. Empty when there is none.
    fn type_arguments(&mut self, uri: &str, class: &str, e: &crate::html::Elemento) -> String {
        let text = self.written_type_arguments(uri, class, e);
        resolver_tardios(self.imp, &text)
    }

    fn written_type_arguments(&self, uri: &str, class: &str, e: &crate::html::Elemento) -> String {
        let mut first = None;
        for t in self.tipos_de_diretiva {
            if t.uri != uri || t.classe != class {
                continue;
            }
            match &t.em {
                Some(r) => {
                    if e.referencias.iter().any(|x| x.nome == *r) {
                        return t.argumentos.clone();
                    }
                }
                None if first.is_none() => {
                    if e.referencias.is_empty() {
                        return t.argumentos.clone();
                    }
                    first = Some(t.argumentos.clone());
                }
                None => {}
            }
        }
        first.unwrap_or_default()
    }

    /// Registers, by the position of the node ([`chave_de_no`]), each class
    /// token that its instances provide (the instance itself and the
    /// `ExistingProvider`s): the value that a content query reads from another
    /// view (`_providers.get(token).build()`).
    fn register_node_by_position(
        &mut self,
        start: usize,
        instances: &[crate::directives::Instancia],
    ) {
        for i in instances {
            for t in std::iter::once(&i.token).chain(&i.apelidos) {
                if let crate::directives::Token::Classe { uri, classe } = t {
                    self.refs
                        .entry(chave_de_no(&chave_de_tipo(uri, classe), start))
                        .or_insert_with(|| format!("this.{}", i.leitura));
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
        nodes: &[No],
        uri: &str,
        class: &str,
    ) -> Result<Vec<ItemDeConteudo>, Recusa> {
        arvore_de_token(nodes, uri, class, self.filhos, self.usadas)
    }

    /// Registers the dynamic content query ([`ConteudoDinamico`]): the dirty
    /// field, the update in change detection (the marker, assembled at the end)
    /// and the `dirtyParentQueriesInternal` of each embedded view with a
    /// result.
    #[allow(clippy::too_many_arguments)]
    fn conteudo_dinamico_no(
        &mut self,
        arvore: Vec<ItemDeConteudo>,
        chave: String,
        selector: &str,
        n: u32,
        index: usize,
        lista: bool,
        alvo: String,
        view_query: bool,
    ) {
        let classe = self.classe_desta_visao();
        // The view one (`@ViewChild(ren)` by token, rooted in the component
        // view): `_viewQuery_X_i_isDirty`, updated in `afterNodes`.
        let field = if view_query {
            format!("_viewQuery_{selector}_{index}_isDirty")
        } else {
            format!("_query_{selector}_{n}_{index}_isDirty")
        };
        let kind: u8 = if view_query { 1 } else { 0 };
        fn mark_dirty(
            itens: &[ItemDeConteudo],
            level: u32,
            field: &str,
            classe: &str,
            kind: u8,
            map: &mut SujasDeConteudo,
        ) {
            for i in itens {
                if let ItemDeConteudo::Aninhada { estrela, itens } = i {
                    let first = itens.iter().find_map(|x| match x {
                        ItemDeConteudo::Valor { inicio, .. } => Some(*inicio),
                        ItemDeConteudo::Aninhada { .. } => None,
                    });
                    if let Some(p) = first {
                        map.entry(*estrela).or_default().push((
                            p,
                            kind,
                            field.to_string(),
                            level,
                            classe.to_string(),
                        ));
                    }
                    mark_dirty(itens, level + 1, field, classe, kind, map);
                }
            }
        }
        mark_dirty(
            &arvore,
            1,
            &field,
            &classe,
            kind,
            &mut self.sujas_de_conteudo.borrow_mut(),
        );
        let has_static = arvore
            .iter()
            .any(|i| matches!(i, ItemDeConteudo::Valor { .. }));
        fn first_nested(itens: &[ItemDeConteudo], nested: bool) -> Option<usize> {
            itens.iter().find_map(|i| match i {
                ItemDeConteudo::Valor { inicio, .. } => nested.then_some(*inicio),
                ItemDeConteudo::Aninhada { itens, .. } => first_nested(itens, true),
            })
        }
        let position = first_nested(&arvore, false).unwrap_or(usize::MAX);
        fn has_on_push(itens: &[ItemDeConteudo]) -> bool {
            itens.iter().any(|i| match i {
                ItemDeConteudo::Valor { on_push, .. } => *on_push,
                ItemDeConteudo::Aninhada { itens, .. } => has_on_push(itens),
            })
        }
        let any_on_push = has_on_push(&arvore);
        let mut queries = self.conteudo_dinamico.borrow_mut();
        let id = queries.len();
        queries.push(ConteudoDinamico {
            id,
            arvore,
            chave,
            lista,
            alvo,
            classe,
        });
        let mut mark = format!("{MARCA_DE_CONTEUDO}{id}");
        if !lista && !has_static {
            mark += &format!("|q{}", tardio_q(QUERIES));
        }
        if any_on_push {
            mark += &format!("|d{}", tardio_q(VIEW));
        }
        mark.push(FIM_DE_CONTEUDO);
        drop(queries);
        let update =
            format!("    if (this.{field}) {{\n      {mark}\n      this.{field} = false;\n    }}");
        if view_query {
            self.sujos_de_visao
                .push((position, format!("  bool {field} = true;")));
            self.atualizacoes_de_visao.push((index, update));
        } else {
            self.sujos_de_conteudo
                .push((position, format!("  bool {field} = true;")));
            self.atualizacoes_de_conteudo.push(update);
        }
    }

    /// The namespace of the element and the name without prefix: an explicit
    /// `ns:tag`, the tag's implicit one (`svg`, `math`, `html_tags.dart`) or
    /// the inherited one.
    fn namespace_of(&self, e: &crate::html::Elemento) -> Option<(String, String)> {
        if let Some((ns, name)) = e.nome.split_once(':') {
            return Some((ns.to_string(), name.to_string()));
        }
        let implicit = match e.nome.to_ascii_lowercase().as_str() {
            "svg" => Some("svg"),
            "math" => Some("math"),
            _ => None,
        };
        implicit
            .map(str::to_string)
            .or_else(|| self.ns_atual.clone())
            .map(|ns| (ns, e.nome.clone()))
    }

    fn classe_desta_visao(&self) -> String {
        if self.classe_desta.is_empty() {
            format!("{}0", self.classe_da_visao)
        } else {
            self.classe_desta.clone()
        }
    }

    /// The providers above, from the nearest to the farthest, with the read as
    /// seen from this view (`getPropertyInView`).
    fn providers_above(&mut self) -> Vec<crate::directives::ProvedorAcima> {
        let util = self.imp.alias(UTILITIES);
        self.providers_above_with(&util)
    }

    /// [`Self::providers_above`] with the given `unsafeCast` prefix: a
    /// preliminary resolution (the position of the node's fields) does not
    /// allocate an import.
    fn providers_above_with(&self, util: &str) -> Vec<crate::directives::ProvedorAcima> {
        self.acima
            .iter()
            .rev()
            .map(|(t, field, v)| crate::directives::ProvedorAcima {
                preguicoso: self.preguicosos_acima.contains(&(
                    field.clone(),
                    match v {
                        None => self.classe_desta_visao(),
                        Some((class, _)) => class.clone(),
                    },
                )),
                leitura_preguicosa: match v {
                    None => false,
                    Some((class, _)) => self
                        .preguicosos_com_pedidos
                        .contains(&(field.clone(), class.clone())),
                },
                token: t.clone(),
                leitura: match v {
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
    fn declarar_refs(&mut self, refs: std::collections::HashSet<String>) {
        let class = self.classe_desta_visao();
        for name in &refs {
            // The type of the read is `dynamic` (the reference does not go into
            // the `locals` of the `AnalyzedClass`), or that of the member with
            // the same name, which the `_TypeResolver` finds first.
            let tipo = self
                .membros
                .get(name.as_str())
                .map(|m| m.tipo.trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "dynamic".into());
            self.locais.insert(
                name.clone(),
                crate::expr::Local {
                    dart: format!("local_{name}"),
                    tipo,
                    escopo: None,
                },
            );
            self.decl_locais.insert(
                name.clone(),
                Ok(format!(
                    "final local_{name} = {MARCA_DE_REF}{}{FIM_DE_REF};",
                    chave_de_ref(name, &class)
                )),
            );
        }
        self.refs_locais = refs;
    }

    /// The field of an `@i18n` message without HTML (`createI18nMessage`): a
    /// `static final String _message_N = Intl.message(text, desc: ..)`, reused
    /// when the text and the metadata repeat.
    fn message(&mut self, text: &str, m: &MetaI18n) -> Result<String, Recusa> {
        let Some(intl) = self.intl.clone().filter(|i| !i.is_empty()) else {
            return Err(recusa(Motivo::I18n, "@i18n outside the component view"));
        };
        let mut call = format!(
            "{intl}.Intl.message({}, desc: {}",
            literal(text),
            literal(m.descricao.as_deref().unwrap_or_default())
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
        if let Some((_, field)) = self.mensagens.iter().find(|(c, _)| *c == call) {
            return Ok(field.clone());
        }
        let field = format!("_message_{}", self.mensagens.len());
        self.campos
            .push(format!("  static final String {field} = {call};"));
        self.mensagens.push((call, field.clone()));
        Ok(field)
    }

    /// The children with HTML of an element with `@i18n`
    /// ([`Corpo::i18n_children`]).
    fn i18n_html_children(
        &mut self,
        e: &crate::html::Elemento,
        m: &MetaI18n,
        target: &str,
    ) -> Result<(), Recusa> {
        if self.intl.is_none() {
            return self.anotar(recusa(Motivo::I18n, "@i18n outside the component view"));
        }
        let mut text = String::new();
        let mut args = Vec::new();
        match mensagem_com_html(&e.filhos, self.filhos, &mut text, &mut args, &mut 0) {
            Ok(true) if !args.is_empty() => {}
            Ok(_) => {
                return self.anotar(recusa(Motivo::I18n, "@i18n message without text"));
            }
            Err(f) => return self.anotar(recusa(Motivo::I18n, f)),
        }
        // `HtmlEscape(HtmlEscapeMode.element)` of the already escaped text.
        let text = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let intl = tardio(INTL);
        let mut call = format!(
            "{intl}.Intl.message('{text}', desc: {}",
            literal(m.descricao.as_deref().unwrap_or_default())
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
        let field = match self.mensagens.iter().find(|(c, _)| *c == call) {
            Some((_, field)) => field.clone(),
            None => {
                let field = format!("_message_{}", self.mensagens.len());
                let names: Vec<&str> = args.iter().map(|(n, _)| n.as_str()).collect();
                let examples: Vec<String> = args
                    .iter()
                    .map(|(n, v)| format!("{}: {}", literal(n), literal(v)))
                    .collect();
                let parameters: Vec<String> = names.iter().map(|n| format!("String {n}")).collect();
                self.metodos_i18n.push(format!(
                    "\n  static String {field}({}) {{\n    return {call}, name: '{}_{field}', args: [{}], examples: const {{{}}});\n  }}\n",
                    parameters.join(", "),
                    self.classe_desta_visao(),
                    names.join(", "),
                    examples.join(", ")
                ));
                self.mensagens.push((call, field.clone()));
                field
            }
        };
        let n = self.proximo;
        self.proximo += 1;
        let avu = self.imp.q(APP_VIEW_UTILS);
        self.linhas.push(format!(
            "    final _html_{n} = {avu}createTrustedHtml({});\n    {target}.append(_html_{n});",
            invocation(&field)
        ));
        Ok(())
    }

    /// Records a refusal. Outside collection mode it stops the emission
    /// (`Err`); in collection mode, it is recorded and the emission goes on.
    fn anotar(&mut self, r: Recusa) -> Result<(), Recusa> {
        match &mut self.coleta {
            Some(v) => {
                v.push(r);
                Ok(())
            }
            None => Err(r),
        }
    }

    fn coletando(&self) -> bool {
        self.coleta.is_some()
    }

    /// The template file, for the `REF` comment. With the template written in
    /// the annotation the reference is a different one (URI of the `.dart` and
    /// offset), and that has no case in the corpus yet.
    fn url(&self, reason: Motivo) -> Result<String, Recusa> {
        self.url_do_template
            .clone()
            .ok_or_else(|| recusa(reason, "binding in a template written in the annotation"))
    }

    /// Converts an expression of the change-detection bindings; the locals it
    /// reads go into the root list, in order of first use. A call with a named
    /// argument, which the formatter breaks into lines
    /// ([`crate::expr::QUEBRA`]), is left to whoever knows where it lands
    /// ([`Self::convert_breakable`]).
    fn convert(
        &mut self,
        text: &str,
        reason: Motivo,
    ) -> Result<crate::expr::Convertida, Recusa> {
        let c = self.convert_breakable(text, reason)?;
        if c.texto.contains(crate::expr::QUEBRA) {
            return Err(recusa(
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
        reason: Motivo,
    ) -> Result<crate::expr::Convertida, Recusa> {
        let locals = self.locais.clone();
        let scope = crate::expr::Escopo {
            membros: self.membros,
            metodos: self.metodos,
            aridades: self.aridades,
            locais: &locals,
            tipos: self.tipos,
            classe: Some(&self.classe_qualificada),
            exportados: Some(self.exportados),
            genericos: self.nomes_genericos,
        };
        let mut c = crate::expr::converter_no_escopo(text, &scope, self.nomes)
            .map_err(|r| r.em(reason))?;
        if c.texto.contains(crate::expr::MARCA_DE_PIPE) {
            c.texto = self.replace_pipes(&c.texto)?;
        }
        for l in &c.locais {
            self.local_declaration(l, reason)?;
            if !self.locais_raiz.contains(l) {
                self.locais_raiz.push(l.clone());
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
    fn replace_pipes(&mut self, text: &str) -> Result<String, Recusa> {
        use crate::expr::{FIM_DE_PIPE, MARCA_DE_PIPE};
        let calls: Vec<(String, usize, String)> = self
            .pipes
            .da_vista(self.vista)
            .map(|c| {
                (
                    self.pipes.instancias[c.instancia].nome.clone(),
                    c.argumentos,
                    c.proxy.clone(),
                )
            })
            .collect();
        // Each marker: where its call starts and where it closes.
        let mut marks = Vec::new();
        let mut from = 0;
        while let Some(i) = text[from..].find(MARCA_DE_PIPE).map(|i| i + from) {
            let after = i + MARCA_DE_PIPE.len_utf8();
            let f = text[after..]
                .find(FIM_DE_PIPE)
                .map_or(text.len(), |f| f + after);
            let open = f + FIM_DE_PIPE.len_utf8();
            let close = argumentos_da_chamada(text, open).map_or(text.len(), |(end, _)| end);
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
        while let Some(i) = rest.find(MARCA_DE_PIPE) {
            out.push_str(&rest[..i]);
            let after = &rest[i + MARCA_DE_PIPE.len_utf8()..];
            let f = after.find(FIM_DE_PIPE).unwrap_or(after.len());
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
                _ if self.coletando() => out.push_str("this._pipe"),
                _ => {
                    return Err(recusa(
                        Motivo::PipesUsados,
                        "pipe call out of the official compiler's order",
                    ));
                }
            }
            rest = after.get(f + FIM_DE_PIPE.len_utf8()..).unwrap_or("");
        }
        out.push_str(rest);
        Ok(out)
    }

    /// Every pipe call of this view was converted, and in order.
    fn conferir_pipes(&self) -> Result<(), Recusa> {
        if !self.coletando() && self.cursor_pipe != self.pipes.da_vista(self.vista).count() {
            return Err(recusa(
                Motivo::PipesUsados,
                "pipe call out of the official compiler's order",
            ));
        }
        Ok(())
    }

    /// The declaration of a local of this view, or the refusal: a local of an
    /// ancestor view (read through `parentView`) and a local without a known
    /// type.
    fn local_declaration(&self, name: &str, reason: Motivo) -> Result<String, Recusa> {
        match self.decl_locais.get(name) {
            Some(Ok(d)) => Ok(d.clone()),
            Some(Err(r)) => Err(r.clone().em(reason)),
            None => Err(recusa(
                reason,
                "local of an ancestor `*` read in the nested view",
            )),
        }
    }

    /// `[x]="e"`: new value, `checkBinding` against the previous one and the
    /// action on the element. The binding name and the template URI go into the
    /// check for the "expression changed after it was checked" message.
    fn property(&mut self, l: &crate::html::Ligacao, target: &str) -> Result<Ligada, Recusa> {
        let url = self.url(Motivo::Ligacao)?;
        // The call (which may break) is never immutable: it goes alone into
        // `final currVal_k = …;`.
        let converted = self.convert_breakable(&l.valor, Motivo::Ligacao)?;
        // The text that goes into `checkBinding` is the source of the binding
        // as written (`ASTWithSource.source`), in a Dart literal.
        let expr = literal(&l.valor);
        let (start, end) = (l.inicio, l.fim);
        // Every binding consumes an index (`createUniqueBindIndex` in
        // `_checkBinding`), even the immutable one, which gets no field.
        let k = self.proxima_ligacao;
        self.proxima_ligacao += 1;
        // A value that does not change is written once, on the first check,
        // without `checkBinding` and without a previous-value field
        // (`isImmutable`, `_bindLiteral`). What can be null (`canBeNull`:
        // everything but a literal) gets an `if (x != null)` around it; the
        // `null` literal generates nothing, a shape with no case in the
        // corpus yet.
        if converted.imutavel {
            if converted.texto == "null" {
                return Err(recusa(Motivo::Ligacao, "constant `null` binding"));
            }
            let action = self.action(l, target, &converted.texto, &converted)?;
            if converted.pode_ser_nulo {
                let value = &converted.texto;
                return Ok(Ligada::Constante(format!(
                    "      if (({value} != null)) {{\n        {action} /* REF:{url}:{start}:{end} */;\n      }}"
                )));
            }
            return Ok(Ligada::Constante(format!(
                "      {action} /* REF:{url}:{start}:{end} */;"
            )));
        }
        self.campos_expr.push(format!("  Object? _expr_{k};"));
        let action = self.action(l, target, &format!("currVal_{k}"), &converted)?;
        let chk = tardio(CHECK_BINDING);
        let value = converted.texto;
        Ok(Ligada::Dinamica(format!(
            "    final currVal_{k} = {value};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, {expr}, '{url}')) {{\n      {action} /* REF:{url}:{start}:{end} */;\n      this._expr_{k} = currVal_{k};\n    }}"
        )))
    }

    /// The bindings of an element, as `bindAndWriteToRenderer` writes them: the
    /// constant ones first, in an `if (firstCheck)` — reusing the one of the
    /// previous element if it is the last thing written
    /// (`addStmtsIfFirstCheck`) —, then the dynamic ones, in order.
    fn write_bindings(&mut self, bindings: Vec<Ligada>) {
        let mut constants = Vec::new();
        let mut dynamic = Vec::new();
        for l in bindings {
            match l {
                Ligada::Constante(s) => constants.push(s),
                Ligada::Dinamica(s) => dynamic.push(s),
            }
        }
        if !constants.is_empty() {
            self.usa_primeira_checagem = true;
            na_primeira_checagem(&mut self.deteccao, &constants);
        }
        self.deteccao.extend(dynamic);
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
        l: &crate::html::Ligacao,
        target: &str,
        value: &str,
        c: &crate::expr::Convertida,
    ) -> Result<String, Recusa> {
        // The action goes into `detectChangesInternal`: the import is allocated
        // when change detection is written, not now.
        let dom = tardio(DOM_HELPERS);
        // `[class]`, `[className]` and `[attr.class]` are the same unnamed
        // `ClassBinding` (`_propertyToIr` of `binding_converter`):
        // `updateChildClass`, which keeps the classes of the stylesheet scope.
        Ok(
            // An element that is not HTML (a component, an unknown tag with a
            // directive): the `NonHtml` variants (`isHtmlElement`).
            if matches!(l.nome.as_str(), "class" | "className" | "attr.class") {
                let method = if dom::tag_html(&self.tag_atual) {
                    "updateChildClass"
                } else {
                    "updateChildClassNonHtml"
                };
                let instance = self.instancia_da_visao.as_deref().unwrap_or("this");
                format!("{instance}.{method}({target}, {value})")
            } else if let Some(class) = l.nome.strip_prefix("class.") {
                // `[class.x.y]`: the class is `x` (`parts[1]`), the rest is
                // dropped.
                let class = class.split('.').next().unwrap_or(class);
                let method = if dom::tag_html(&self.tag_atual) {
                    "updateClassBinding"
                } else {
                    "updateClassBindingNonHtml"
                };
                format!("{dom}.{method}({target}, '{class}', {value})")
            } else if let Some(attr) = l.nome.strip_prefix("attr.") {
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
                        return Err(recusa(Motivo::Ligacao, "invalid [attr.x.unit]"));
                    }
                };
                if conditional && attr == "class" {
                    return Err(recusa(Motivo::Ligacao, "class.if (official compiler error)"));
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
                let attr_sanitizer = saneador(
                    &self.tag_atual.to_ascii_lowercase(),
                    propriedade_mapeada(attr),
                );
                if attr_sanitizer.is_some() && (conditional || ns_uri.is_some()) {
                    return Err(recusa(
                        Motivo::Ligacao,
                        "sanitized [attr.x.if] or namespaced attribute",
                    ));
                }
                let sanitized = match attr_sanitizer {
                    Some(f) => {
                        let s = tardio(SAFE_HTML);
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
                    let f = if !c.pode_ser_nulo && !conditional {
                        "setAttribute"
                    } else {
                        "updateAttribute"
                    };
                    format!("{dom}.{f}({target}, '{attr}', {value})")
                }
            } else if let Some(style) = l.nome.strip_prefix("style.") {
                // `visitStyleBinding`: with a unit, `v == null ? null : v +
                // 'px'` (`v.toString()` if it is not a `String`); without one,
                // the value itself if it is a `String`, otherwise
                // `v.toString()` — `v?.toString()` when `canBeNull`. `isString`
                // is the `_TypeResolver` (`String?` counts).
                let Some(ty) = c.tipo.as_deref() else {
                    return Err(recusa(Motivo::Ligacao, "[style.x] of unknown type"));
                };
                let is_string = ty.trim_end_matches('?') == "String";
                let (name, unit) = match style.split_once('.') {
                    Some((_, u)) if u.contains('.') => {
                        return Err(recusa(Motivo::Ligacao, "[style.x.y.z]"));
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
                            estilo_nulo(),
                            literal(u)
                        )
                    }
                    None => estilo_sem_unidade(value, is_string, c.pode_ser_nulo),
                };
                format!("{target}.style.setProperty('{name}', {value})")
            } else if l.nome.contains('.') {
                return Err(recusa(Motivo::Ligacao, "binding with an unknown prefix"));
            } else {
                // `getMappedPropName` (`_attrToPropMap`,
                // `dom_element_schema_registry.dart`): only four names change;
                // `class` was already handled above as a `ClassBinding`. A
                // bound `[tabindex]` is `PropertyBinding('tabIndex')`, not the
                // `TabIndexBinding` of the literal
                // (`binding_converter.dart:131`).
                let prop = propriedade_mapeada(&l.nome);
                // `_sanitizedValue`: the value goes through the context's
                // sanitizer (`[style]` is `*|style`: `sanitizeStyle`, case
                // i92).
                match saneador(&self.tag_atual.to_ascii_lowercase(), prop) {
                    Some(f) => {
                        let s = tardio(SAFE_HTML);
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
    fn events(&mut self, e: &crate::html::Elemento, target: &str) -> Result<(), Recusa> {
        let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
        for l in &e.eventos {
            match groups.iter_mut().find(|(n, _)| *n == l.nome) {
                // Two `(click)` written on the same element are an error in the
                // official compiler ("Found multiple events with the same
                // name"); merging only happens with the listeners that come
                // from directives.
                Some(_) => {
                    return Err(recusa(
                        Motivo::Evento,
                        "two handlers of the same event in the template",
                    ));
                }
                None => groups.push((&l.nome, vec![&l.valor])),
            }
        }
        for (name, handlers) in groups {
            self.tipo_do_evento_falha = None;
            self.tipo_do_evento = tipo_de_evento_nativo(name);
            match self.handler(&handlers) {
                Ok(h) => self.listener(name, target, &h),
                Err(r) => self.anotar(r)?,
            }
        }
        Ok(())
    }

    /// The listener of an event: the element's `addEventListener` for a DOM
    /// event, ngdart's `eventManager` for the rest (`keyup.enter`, a custom
    /// event) — `visitNativeEvent` × `visitCustomEvent`.
    fn listener(&mut self, name: &str, target: &str, handler: &str) {
        let line = if evento_nativo(name) {
            format!("    {target}.addEventListener('{name}', {});", ouvinte_nativo(name, handler))
        } else {
            let utils = tardio_q(APP_VIEW_UTILS);
            format!(
                "    {utils}appViewUtils.eventManager.addEventListener({target}, '{name}', {handler});"
            )
        };
        self.ouvintes.push(line);
    }

    /// The handler expression of an event (`BoundValueConverter`): a single
    /// simple handler becomes a tear-off (`this.eventHandler0(_ctx.m)`); the
    /// rest — an assignment, arguments, several merged handlers — becomes the
    /// `_handleEvent_N` method of this view, with `eventHandler1`.
    fn handler(&mut self, texts: &[&str]) -> Result<String, Recusa> {
        self.handler_with(texts, &[])
    }

    /// [`Self::handler`] with `extras`: the calls of the nodes' directives'
    /// `@HostListener`s for the same event, after the template actions
    /// (`mergeEvents`) — the handler is always the method (case j70).
    fn handler_with(&mut self, texts: &[&str], extras: &[String]) -> Result<String, Recusa> {
        let scope_locals = self.locais.clone();
        let scope = crate::expr::Escopo {
            membros: self.membros,
            metodos: self.metodos,
            aridades: self.aridades,
            locais: &scope_locals,
            tipos: self.tipos,
            classe: Some(&self.classe_qualificada),
            exportados: Some(self.exportados),
            genericos: self.nomes_genericos,
        };
        let mut actions = Vec::new();
        for t in texts {
            actions.push(
                crate::expr::converter_acao(t, &scope, self.nomes)
                    .map_err(|r| r.em(Motivo::Evento))?,
            );
        }
        if let (
            [
                crate::expr::Acao::Simples {
                    metodo, aridade, ..
                },
            ],
            [],
        ) = (actions.as_slice(), extras)
        {
            self.usa_ctx_no_build = true;
            return Ok(format!("this.eventHandler{aridade}(_ctx.{metodo})"));
        }
        // Complex: the method takes the type of `$event` (in 9).
        if let Some(r) = self.tipo_do_evento_falha.clone() {
            return Err(r);
        }
        // The statements, with the locals they read declared at the top of the
        // method (the scope of `scopeNamespace`).
        let mut statements = Vec::new();
        let mut locals: Vec<String> = Vec::new();
        for a in &actions {
            match a {
                crate::expr::Acao::Simples { instrucao, .. } => statements.push(instrucao.clone()),
                crate::expr::Acao::Complexa(c) => {
                    statements.push(c.texto.clone());
                    for l in &c.locais {
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
            if let Some(k) = self.campos_el_de_eventos.iter().position(|(n, _)| n == l) {
                let (_, field) = self.campos_el_de_eventos.remove(k);
                self.campos_el_por_evento.push(field);
            }
        }
        for l in &locals {
            body.push(format!(
                "    {}",
                self.local_declaration(l, Motivo::Evento)?
            ));
        }
        if cita_ctx(&statements) {
            body.push("    final _ctx = this.ctx;".to_string());
        }
        body.extend(statements.iter().map(|i| format!("    {i};")));
        body.extend(extras.iter().map(|i| format!("    {i};")));
        let n = self.metodos_evento.len();
        self.metodos_evento.push(format!(
            "\n  void _handleEvent_{n}({}) {{\n{}\n  }}\n",
            parametro_do_evento(&self.tipo_do_evento),
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
        e: &crate::html::Elemento,
        child: &Filho,
        parent: &str,
    ) -> Result<(), Recusa> {
        let refuse = |f: &str| recusa(Motivo::LigacaoEmFilho, f);
        if let Some(r) = child.pendencias.first() {
            return Err(r.clone());
        }
        // A `[(x)]` that the child itself receives (`@Input x`, `@Output
        // xChange`), desugared like the `DesugarVisitor` does: the input at the
        // end of the properties and the event `x = $event` at the end of the
        // events.
        let mut desugared = e.clone();
        desugared.bananas.clear();
        for b in &e.bananas {
            let change = format!("{}Change", b.nome);
            if child.entrada(&b.nome).is_some() && child.saida(&change).is_some() {
                desugared.propriedades.push(b.clone());
                desugared.eventos.push(crate::html::Ligacao {
                    nome: change,
                    valor: format!("{} = $event", b.valor),
                    ..b.clone()
                });
            } else {
                desugared.bananas.push(b.clone());
            }
        }
        let e = &desugared;
        // The directives that match the child's element (`NgModel`), and their
        // `[(x)]` desugared as on an HTML element.
        let extras = diretivas_casadas(self.usadas, e);
        let mut dir_props = Vec::new();
        let mut dir_events = Vec::new();
        for b in &e.bananas {
            let change = format!("{}Change", b.nome);
            if !consome_entrada(&extras, &b.nome) || !consome_saida(&extras, &change) {
                return Err(refuse("[(x)] on the child"));
            }
            dir_props.push(b.clone());
            dir_events.push(crate::html::Ligacao {
                nome: change,
                valor: format!("{} = $event", b.valor),
                ..b.clone()
            });
        }
        dir_props.extend(
            e.propriedades
                .iter()
                .filter(|l| consome_entrada(&extras, &l.nome))
                .cloned(),
        );
        dir_events.extend(
            e.eventos
                .iter()
                .filter(|l| consome_saida(&extras, &l.nome))
                .cloned(),
        );
        // The child's `viewProviders:`: private providers of the node. Without
        // child nodes, they go into the same `ProviderNode` as the others
        // (`createProviderNode`, `childNodeCount == 0`); with them, into a
        // separate one (`[n, n]`), still without a case.
        let no_child_nodes = e.filhos.iter().all(|x| match x {
            No::Texto(t) => !self.preservar_espacos && t.trim().is_empty(),
            No::Comentario(_) => true,
            _ => false,
        });
        if !no_child_nodes
            && child
                .metadados
                .as_ref()
                .is_some_and(|m| !m.provedores_de_visao.is_empty())
        {
            return Err(refuse("child with viewProviders and child nodes"));
        }
        // The providers of the node: the component first, then the directives.
        // `Visibility.all` puts the child into `injectorGetInternal`: it is the
        // resolved path that writes it.
        let has_providers = !extras.is_empty()
            || child.metadados.as_ref().is_some_and(|m| {
                !m.provedores.is_empty() || !m.provedores_de_visao.is_empty() || m.visivel
            });
        let resolved_node = if has_providers {
            let meta = child
                .metadados
                .clone()
                .ok_or_else(|| refuse("child with providers/directive without metadata"))?;
            let matched = casadas_do_no_do_filho(&meta, &extras, self.usadas);
            if let Err(f) = provedores_escreviveis(&meta) {
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
        for r in &e.referencias {
            if !r.valor.is_empty() {
                continue;
            }
            if !self.refs_livres.contains(&r.nome)
                && !self.refs_locais.contains(&r.nome)
                && !self.refs_consultados.iter().any(|(n, _, _)| *n == r.nome)
            {
                return Err(refuse(if self.embutida {
                    "#ref on the child in an embedded view"
                } else {
                    "#ref on the child used in an expression"
                }));
            }
        }
        for a in &e.atributos {
            // The literal is the element's `TabIndexBinding` (written with the
            // attributes); the interpolated one would be a property of the
            // child's element, a shape still without a case.
            if a.nome.eq_ignore_ascii_case("tabindex") && a.valor.contains("{{") {
                return Err(refuse("interpolated tabindex on the child"));
            }
        }
        // What reaches an `@Input`: a static attribute (literal) and `[x]`. A
        // name bound twice is dropped by the official compiler
        // (`_removeExisting`).
        let mut bound: Vec<(&crate::html::Ligacao, bool)> = e
            .atributos
            .iter()
            .filter(|a| child.entrada(&a.nome).is_some())
            .map(|a| (a, true))
            .collect();
        // A `[x]` that only a directive of the node receives does not belong to
        // the child; one that nobody receives is a property of the child's
        // element (`_visitProperties`: `[class.x]`, `[attr.x]`, `[id]`…; the
        // schema accepts any property on a custom element), written in
        // `bindRenderInputs` (cases j74, j135).
        bound.extend(
            e.propriedades
                .iter()
                .filter(|l| child.entrada(&l.nome).is_some())
                .map(|l| (l, false)),
        );
        let element_props: Vec<crate::html::Ligacao> = e
            .propriedades
            .iter()
            .filter(|l| child.entrada(&l.nome).is_none() && !consome_entrada(&extras, &l.nome))
            .cloned()
            .collect();
        // An interpolated attribute that neither the child nor a directive of
        // the node receives is a property of the child's element, after its
        // `[x]`s (`_visitProperties`, case j92).
        let element_interpolations: Vec<crate::html::Ligacao> = e
            .atributos
            .iter()
            .filter(|a| {
                a.valor.contains("{{")
                    && child.entrada(&a.nome).is_none()
                    && !consome_entrada(&extras, &a.nome)
            })
            .cloned()
            .collect();
        for (i, (l, _)) in bound.iter().enumerate() {
            if bound[..i].iter().any(|(x, _)| x.nome == l.nome) {
                return Err(refuse("child @Input bound twice"));
            }
        }
        let n = self.proximo;
        self.proximo += 1;
        // With a binding of its own, the child's node becomes a field (change
        // detection reads it). The `detectHostChanges(view, el)` of a directive
        // with `@HostBinding` reads the node during change detection.
        let has_host = extras.iter().any(|d| !d.ligacoes_do_hospedeiro.is_empty());
        let el = if element_props.is_empty()
            && element_interpolations.is_empty()
            && !has_host
        {
            format!("_el_{n}")
        } else {
            let ty = self.imp.dom("HtmlElement");
            self.campos_el.push(format!("  late final {ty} _el_{n};"));
            format!("this._el_{n}")
        };
        let outside_lib = || recusa(Motivo::ComponenteNoTemplate, "child without an import path");
        let template_path = caminho_do_import(
            &self.asset,
            &asset_de_uri(&child.uri_template, "", Path::new("")).ok_or_else(outside_lib)?,
        )
        .ok_or_else(outside_lib)?;
        let dart_path = caminho_do_import(
            &self.asset,
            &asset_de_uri(&child.uri_dart, "", Path::new("")).ok_or_else(outside_lib)?,
        )
        .ok_or_else(outside_lib)?;
        // A child from the same file: its view is right beside it, without an
        // import.
        let vt = if e_o_proprio_template(&self.asset, &template_path) {
            String::new()
        } else {
            self.imp.q(&template_path)
        };
        let vd = self.imp.alias(&dart_path);
        let class = &child.classe;
        let view_field = format!("_compView_{n}");
        // A child that injects `ViewContainerRef`: the node gets a
        // `ViewContainer` (a field between the view and the instance) and three
        // more built-ins, which take the instance to 8 (case j47).
        let container = child
            .parametros
            .iter()
            .any(|p| matches!(p, Injetado::Container))
            || extras.iter().any(|d| crate::directives::pede_container(d));
        // The providers of the node, in the official compiler's order: the
        // directives in `directives:` order with the child in its place
        // (`_matchDirectives`), each eager provider after its dependencies
        // (`_getOrCreateLocalProvider`) — what the child injects from its own
        // node and the directives that come before it are emitted before the
        // instance (cases j61, j67). The reads from above take the import of
        // the deferred `unsafeCast`, allocated where the text is emitted.
        let resolved = match &resolved_node {
            Some((index, matched)) => {
                let provedores = self.providers_above_with(&tardio(UTILITIES));
                let acima = crate::directives::Acima {
                    provedores: &provedores,
                    incerto: self.incertos_acima > 0,
                    de_visao: self.tokens_de_visao,
                };
                let requests = pedidos_ao_no_do_filho(e, matched, self.filhos, self.usadas)?;
                let r = crate::directives::resolver_no_do_filho(
                    matched,
                    *index,
                    n,
                    Some(acima),
                    container,
                    &requests,
                )
                .map_err(|f| recusa(Motivo::DiretivaPorSeletor, f))?;
                let token = matched[*index].token();
                let pos = r
                    .instancias
                    .iter()
                    .position(|i| i.token == token)
                    .ok_or_else(|| refuse("child node without the child instance"))?;
                Some((r, pos))
            }
            None => None,
        };
        let inst_field = match &resolved {
            Some((r, pos)) => r.instancias[*pos].campo.clone(),
            None => format!(
                "_{class}_{n}_{}",
                crate::dialect::embutidos_extra() + if container { 8 } else { 5 }
            ),
        };
        // The directives of the node before and after the child, in the order
        // of the bindings and of the hooks (`transformedDirectiveAsts`).
        let (dirs_before, dirs_after) = match &resolved {
            Some((r, _)) => {
                let k = r
                    .diretivas
                    .iter()
                    .position(|(_, c)| *c == inst_field)
                    .ok_or_else(|| refuse("child node without the child directive"))?;
                (r.diretivas[..k].to_vec(), r.diretivas[k + 1..].to_vec())
            }
            None => (Vec::new(), Vec::new()),
        };
        // `directiveTypes:` types the fields (the view and the instance), not
        // the creation (`lookupTypeArgumentsOf`, `compile_view.dart:891-905`).
        let type_args = self.type_arguments(&child.uri_dart, &child.classe, e);
        self.campos_filho.push(format!(
            "  late final {vt}View{class}0{type_args} {view_field};"
        ));
        if container {
            let vc = self.imp.q(VIEW_CONTAINER);
            self.campos_filho
                .push(format!("  late final {vc}ViewContainer _appEl_{n};"));
        }
        self.vistas_filhas.push(view_field.clone());
        if child.link_de_deteccao {
            self.vistas_ligadas.push(view_field.clone());
        }
        if child.hospedeiro {
            self.usa_primeira_checagem = true;
        }
        self.linhas.push(format!(
            "    this.{view_field} = {vt}View{class}0(this, {n});"
        ));
        self.linhas.push(format!(
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
            self.raizes.push(if container {
                format!("this._appEl_{n}")
            } else {
                el.clone()
            });
        } else {
            self.linhas.push(format!("    {parent}.append({el});"));
        }
        // The written attributes, in alphabetical order (`_toSortedBindings`),
        // all of them — also the ones that feed an `@Input` —, and `class`
        // through `updateChildClassNonHtml`: the child's element is not HTML
        // (`writeLiteralAttributeValues`).
        // The interpolated one is a binding, not a written attribute (case
        // j58).
        let mut attributes: Vec<_> = e
            .atributos
            .iter()
            .filter(|a| !a.valor.contains("{{"))
            .cloned()
            .collect();
        attributes.sort_by(|a, b| a.nome.cmp(&b.nome));
        for a in &attributes {
            let mut value = literal(&a.valor);
            // `_mergeHtmlAndDirectiveAttrs`: the component's static `class` is
            // only written here when the element also has a `class`; the two
            // become an interpolation (the component's one stays in the
            // constructor of its view, and this one overwrites it at run time).
            // A merged `style` has no case yet; the others keep the written
            // attribute.
            if let Some((_, member)) = child
                .atributos_do_hospedeiro
                .iter()
                .find(|(name, _)| *name == a.nome)
            {
                match a.nome.as_str() {
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
            if a.nome == "class" {
                self.linhas
                    .push(format!("    this.updateChildClassNonHtml({el}, {value});"));
            } else if a.nome == "tabindex" || a.nome == "tabIndex" {
                // `TabIndexBinding`, as on an HTML element (case j102).
                match a.valor.trim().parse::<i64>() {
                    Ok(n) if a.valor.trim() == a.valor => {
                        self.linhas.push(format!("    {el}.tabIndex = {n};"));
                    }
                    _ => return Err(refuse("non-integer tabindex on the child")),
                }
            } else {
                let dom = self.dom();
                self.linhas.push(format!(
                    "    {dom}.setAttribute({el}, '{}', {value});",
                    a.nome
                ));
            }
        }
        if self.com_estilo {
            self.linhas.push(format!("    this.addShimC({el});"));
        }
        // The `ViewContainer` is born with the `CompileElement`, before the
        // providers; the second argument is the parent (`null` at the root).
        if container {
            let vc = self.imp.q(VIEW_CONTAINER);
            let parent_index = if parent.is_empty() {
                self.pai_projetado
                    .map_or_else(|| "null".to_string(), |k| k.to_string())
            } else {
                indice_do_elemento(parent)
            };
            self.linhas.push(format!(
                "    this._appEl_{n} = {vc}ViewContainer({n}, {parent_index}, this, {el});"
            ));
            self.ancoras.push(format!("_appEl_{n}"));
        }
        let before_child = resolved
            .as_ref()
            .map_or(&[][..], |(r, pos)| &r.instancias[..*pos]);
        self.create_instances(
            e,
            before_child,
            &el.clone(),
            &format!("this.{view_field}"),
        )?;
        self.campos_filho.push(format!(
            "  late final {vd}.{class}{type_args} {inst_field};"
        ));
        let from_node: Vec<(crate::directives::Token, String)> = before_child
            .iter()
            .flat_map(|i| {
                std::iter::once(&i.token)
                    .chain(&i.apelidos)
                    .map(|t| (t.clone(), i.leitura.clone()))
            })
            .collect();
        let creation = self.construcao_do_filho(e, child, n, &el, &vd, &view_field, &from_node)?;
        self.linhas
            .push(format!("    this.{inst_field} = {creation};"));
        let mut acima = self.pilha.clone();
        acima.push((n, true));
        self.registros.push(Registro {
            acima,
            provedores: vec![(
                crate::directives::Token::Classe {
                    uri: child.uri_dart.clone(),
                    classe: child.classe.clone(),
                },
                inst_field.clone(),
                child.on_push.then(|| view_field.clone()),
            )],
            elemento: el.clone(),
        });
        // The target of each `#ref`: without a value, the component; with a
        // value, the only directive of the node (the component included) with
        // an equal `exportAs` — its field already carries the `.instance` of an
        // `XNgCd`.
        let mut component_refs = Vec::new();
        for r in &e.referencias {
            let read = if r.valor.is_empty() {
                component_refs.push(r.nome.clone());
                format!("this.{inst_field}")
            } else {
                let found: Vec<&String> = match &resolved {
                    Some((res, _)) => res
                        .diretivas
                        .iter()
                        .filter(|(d, _)| d.export_as.as_deref() == Some(r.valor.as_str()))
                        .map(|(_, c)| c)
                        .collect(),
                    None => child
                        .metadados
                        .iter()
                        .filter(|m| m.export_as.as_deref() == Some(r.valor.as_str()))
                        .map(|_| &inst_field)
                        .collect(),
                };
                match found.as_slice() {
                    [c] => {
                        if **c == inst_field {
                            component_refs.push(r.nome.clone());
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
            self.refs.insert(r.nome.clone(), read.clone());
            self.refs_em_ordem.push((r.nome.clone(), read));
        }
        // The result of a `@ViewChild(Type)` ([`chave_de_tipo`]), also read
        // from a view above (dynamic query).
        let key = chave_de_tipo(&child.uri_dart, &child.classe);
        self.refs
            .entry(chave_de_no(&key, e.inicio))
            .or_insert_with(|| format!("this.{inst_field}"));
        if let Some((r, _)) = &resolved {
            let instances = r.instancias.clone();
            self.register_node_by_position(e.inicio, &instances);
        }
        self.refs
            .insert(key.clone(), format!("this.{inst_field}"));
        self.refs_em_ordem
            .push((key.clone(), format!("this.{inst_field}")));
        if child.on_push {
            // The result of a dynamic content query by position.
            self.detectores
                .insert(chave_de_no(&key, e.inicio), view_field.clone());
            self.detectores_em_ordem
                .push((key.clone(), view_field.clone()));
            self.detectores.insert(key, view_field.clone());
        }
        // `buildChangeDetectorRef()` only exists for an `onPush` component: a
        // `#ref` whose value is a directive does not register a detector.
        for name in &component_refs {
            if child.on_push {
                self.detectores_em_ordem
                    .push((name.clone(), view_field.clone()));
                self.detectores.insert(name.clone(), view_field.clone());
            }
        }
        // `bindRenderInputs`: the bindings of the child's own element, before
        // the directives' ones (case j74).
        if !element_props.is_empty() || !element_interpolations.is_empty() {
            self.tag_atual = e.nome.clone();
            self.instancia_da_visao = Some(format!("this.{view_field}"));
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
            self.instancia_da_visao = None;
            if let Some(r) = failure {
                self.anotar(r)?;
            }
            self.write_bindings(el_bindings);
        }
        self.vista_do_hospedeiro = Some(format!("this.{view_field}"));
        if let Some((r, _)) = &resolved {
            self.bind_directives(
                e,
                &r.instancias,
                &dirs_before,
                &el.clone(),
                &dir_props,
                &dir_events,
                true,
                false,
            )?;
        }
        self.vista_do_hospedeiro = None;
        self.child_inputs(&bound, child, &inst_field, &view_field)?;
        // The component's `bindDirectiveHostProps`: with the property bindings,
        // in document order, right after its inputs — not together with the
        // `detectChanges` of its view.
        if child.hospedeiro {
            self.deteccao.push(format!(
                "    this.{view_field}.detectHostChanges(firstCheck);"
            ));
        }
        let mut without_dir_outputs = e.clone();
        without_dir_outputs
            .eventos
            .retain(|l| !consome_saida(&extras, &l.nome));
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
                &r.instancias,
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
        let above_len = self.acima.len();
        if let Some((r, pos)) = &resolved {
            let pos = *pos;
            // The lazy ones that the content asks for already come eager and in
            // the right position from the resolver
            // ([`pedidos_ao_no_do_filho`]).
            let after = r.instancias[pos + 1..].to_vec();
            // The providers after the child, the `registerDirective` of all the
            // directives of the node (except the component) and the bindings of
            // the ones that come after it.
            let target = el.clone();
            self.create_instances(e, &after, &target, &format!("this.{view_field}"))?;
            let all: Vec<_> = dirs_before.iter().chain(&dirs_after).cloned().collect();
            self.register_directives(&all, &target);
            self.vista_do_hospedeiro = Some(format!("this.{view_field}"));
            let bound = self.bind_directives(
                e,
                &r.instancias,
                &dirs_after,
                &target,
                &dir_props,
                &dir_events,
                true,
                true,
            );
            self.vista_do_hospedeiro = None;
            bound?;
            // The child goes first into `injectorGetInternal`, by its aliases
            // (`ExistingProvider(X, TheChild)`) and, when visible, by its
            // class.
            let injectables: Vec<(Vec<crate::directives::Token>, String)> = r
                .instancias
                .iter()
                .filter(|i| !i.injetavel_por.is_empty())
                .map(|i| (i.injetavel_por.clone(), i.leitura.clone()))
                .collect();
            if !injectables.is_empty() {
                child_injector = Some(self.injetores.len());
                self.injetores.push((n, n, injectables));
            }
            for i in &r.instancias {
                // Visibility only decides `injectorGetInternal` (other views);
                // in the same template a node below finds even a local provider
                // by its token (case j41).
                if i.preguicosa {
                    self.preguicosos_acima
                        .insert((i.leitura.clone(), self.classe_desta_visao()));
                    self.preguicosos_com_pedidos
                        .insert((i.leitura.clone(), self.classe_desta_visao()));
                }
                if i.injetavel_por.is_empty() {
                    self.acima.push((i.token.clone(), i.leitura.clone(), None));
                }
                for t in &i.injetavel_por {
                    self.acima.push((t.clone(), i.leitura.clone(), None));
                }
            }
            let mut reg_above = self.pilha.clone();
            reg_above.push((n, true));
            // The child's token is already in its record; its aliases, here.
            let provedores = r
                .instancias
                .iter()
                .flat_map(|i| {
                    (i.campo != inst_field)
                        .then_some(&i.token)
                        .into_iter()
                        .chain(&i.apelidos)
                        .map(|t| (t.clone(), i.leitura.clone(), None))
                })
                .collect();
            self.registros.push(Registro {
                acima: reg_above,
                provedores,
                elemento: el.clone(),
            });
        } else if let Some(meta) = &child.metadados {
            // Without `providers:`, another directive or `Visibility.all`, the
            // child's node only provides its class to the content of the same
            // template.
            self.acima.push((meta.token(), inst_field.clone(), None));
        }
        if child.projecoes.is_empty() && e.filhos.is_empty() {
            self.directive_queries(e, &dirs_before, n)?;
            self.child_queries(e, child, &inst_field, n)?;
            self.directive_queries(e, &dirs_after, n)?;
            self.hooks_after_children(&dirs_before);
            self.after_children(child, &inst_field);
            self.hooks_after_children(&dirs_after);
            self.linhas
                .push(format!("    this.{view_field}.create(this.{inst_field});"));
            self.acima.truncate(above_len);
            return Ok(());
        }
        // Projected content: the nodes are created detached and each one goes
        // to the projection whose selector matches it (`findNgContentIndex`:
        // the lowest index that matches, otherwise the `*`).
        let selectors: Vec<Option<Vec<crate::selector::Seletor>>> = child
            .projecoes
            .iter()
            .map(|s| (s != "*").then(|| crate::selector::Seletor::analisar(s)))
            .collect();
        let wildcard = child.projecoes.iter().position(|s| s == "*");
        let mut lists: Vec<Vec<String>> = vec![Vec::new(); child.projecoes.len()];
        self.filhos_acima
            .push((child.uri_dart.clone(), child.classe.clone()));
        let parent_before = self.pai_projetado.replace(n);
        self.pilha.push((n, true));
        self.componentes_acima += 1;
        let uncertain = child.metadados.is_none();
        if uncertain {
            self.incertos_acima += 1;
        }
        let mut r = Ok(());
        for node in &e.filhos {
            let index = match node {
                No::Comentario(_) => continue,
                No::Elemento(x) => {
                    let mut without_star = x.clone();
                    without_star.estrela = None;
                    let desc = crate::selector::Elemento::do_template(&without_star);
                    selectors
                        .iter()
                        .position(|s| {
                            s.as_ref()
                                .is_some_and(|s| crate::selector::casa_algum(s, &desc))
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
                (None, No::Texto(_) | No::Interpolacao { .. }) => {
                    self.proximo += 1;
                    continue;
                }
                (None, No::Elemento(x)) if x.nome != "ng-container" || x.estrela.is_some() => true,
                (None, _) => {
                    r = Err(recusa(
                        Motivo::Projecao,
                        "<ng-container> that no projection receives",
                    ));
                    break;
                }
            };
            let before = self.raizes.len();
            if let Err(x) = self.nos(std::slice::from_ref(node), "") {
                r = Err(x);
                break;
            }
            let created: Vec<String> = self.raizes.drain(before..).collect();
            if let (false, Some(i)) = (discarded, index) {
                lists[i].extend(created);
            }
        }
        self.pai_projetado = parent_before;
        self.filhos_acima.pop();
        self.pilha.pop();
        self.componentes_acima -= 1;
        if uncertain {
            self.incertos_acima -= 1;
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
            let reprojects = lists.iter().flatten().any(|x| x.starts_with(RAIZ_LISTA));
            let util = if reprojects {
                self.imp.alias(UTILITIES)
            } else {
                String::new()
            };
            // In the broken list, each item starts at column 6.
            let mut items: Vec<String> = Vec::new();
            for l in &lists {
                items.push(lista_plana(l, &util)?.texto_em(6));
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
        self.linhas.push(text);
        self.acima.truncate(above_len);
        // `ProviderNode(nodeIndex, nodeIndex + childNodeCount)`.
        if let Some(i) = child_injector {
            self.injetores[i].1 = self.proximo - 1;
        }
        Ok(())
    }

    /// The view from which the outer injector is read
    /// (`injectFromViewParentInjector` written in the view of the topmost node
    /// of the injector chain and brought to this one by `getPropertyInView`):
    /// `this`, or the `parentView` chain up to it (cases i76, j40).
    fn injector_view(&self) -> String {
        let hops = self.profundidade - self.nivel_do_topo;
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
    fn construcao_do_filho(
        &mut self,
        e: &crate::html::Elemento,
        child: &Filho,
        n: u32,
        el: &str,
        vd: &str,
        view_field: &str,
        from_node: &[(crate::directives::Token, String)],
    ) -> Result<String, Recusa> {
        let refuse = |f: &str| recusa(Motivo::LigacaoEmFilho, f);
        let class = &child.classe;
        let from_own_node = |token: &crate::directives::Token| {
            from_node.iter().find_map(|(t, c)| (t == token).then_some(c))
        };
        // The providers of the elements above (`_getDependency` walks up
        // through them before the outer injector), with the import of the
        // deferred `unsafeCast`.
        let providers_above = self.providers_above_with(&tardio(UTILITIES));
        let from_above =
            |token: &crate::directives::Token| providers_above.iter().find(|p| p.token == *token);
        // The class of this view's component: `@Host()` finds it in the
        // injector (`identifierToken(component.type).equalsTo(dep.token)`).
        let is_own_component = |token: &crate::directives::Token| match token {
            crate::directives::Token::Classe { uri, classe: ty } => {
                self.classe_qualificada.rsplit('.').next() == Some(ty.as_str())
                    && asset_de_uri(uri, "", Path::new("")).as_deref() == Some(self.asset.as_str())
            }
            _ => false,
        };
        // `_getDependency`: the node itself (without `@SkipSelf`); with
        // `@Self`, only it; otherwise the elements above; and, if not found,
        // the outer injector — except with `@Host` in a component view (unless
        // the token is the component itself), where it is `null` if
        // `@Optional`.
        let mut sources = Vec::new();
        for p in &child.parametros {
            let Injetado::Servico {
                token,
                opcional,
                proprio,
                hospedeiro,
                pular,
            } = p
            else {
                sources.push(None);
                continue;
            };
            if !pular && let Some(c) = from_own_node(token) {
                sources.push(Some(OrigemDoServico::Local(c.clone())));
                continue;
            }
            if *proprio {
                if !opcional {
                    return Err(refuse("@Self() without a provider on the child node"));
                }
                sources.push(Some(OrigemDoServico::Nulo));
                continue;
            }
            // An element built-in: the official compiler finds it on the node,
            // never in the outer injector (still without a case on the child
            // node).
            if crate::directives::embutido_do_elemento(token) {
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
                if p.preguicoso {
                    return Err(refuse(
                        "lazy provider of an element above requested below (L1)",
                    ));
                }
                sources.push(Some(OrigemDoServico::Cima(p.leitura.clone())));
                continue;
            }
            // A component above without metadata could provide the service: not
            // finding it does not prove that it comes from outside.
            if self.incertos_acima > 0 {
                return Err(refuse(
                    "child that injects a service under a component without metadata",
                ));
            }
            if *hospedeiro && !is_own_component(token) && !self.tokens_de_visao.contains(token) {
                if !opcional {
                    return Err(refuse("@Host() without a provider in the child view"));
                }
                sources.push(Some(OrigemDoServico::Nulo));
                continue;
            }
            sources.push(Some(OrigemDoServico::Injetor));
        }
        // Only the injector dependency is dynamic (`hasDynamicDependencies`):
        // one that the node itself or an element above provides does not wrap
        // the creation.
        let injects = sources
            .iter()
            .any(|o| matches!(o, Some(OrigemDoServico::Injetor)));
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
        for (p, source) in child.parametros.iter().zip(&sources) {
            match p {
                Injetado::Elemento => args.push(el.to_string()),
                Injetado::Detector => args.push(format!("this.{view_field}")),
                Injetado::Container => args.push(format!("this._appEl_{n}")),
                // The literal value of the attribute on the element
                // (`_attrs[name]`), or `null`.
                Injetado::Atributo(name) => args.push(
                    e.atributos
                        .iter()
                        .find(|a| a.nome == *name && !a.valor.contains("{{"))
                        .map_or_else(|| "null".to_string(), |a| literal(&a.valor)),
                ),
                Injetado::Servico {
                    token, opcional, ..
                } => {
                    match source {
                        Some(OrigemDoServico::Local(c)) => {
                            args.push(format!("this.{c}"));
                            continue;
                        }
                        Some(OrigemDoServico::Cima(l)) => {
                            args.push(l.clone());
                            continue;
                        }
                        Some(OrigemDoServico::Nulo) => {
                            args.push("null".to_string());
                            continue;
                        }
                        _ => {}
                    }
                    let expr = match token {
                        crate::directives::Token::Classe { uri, classe: ty } => {
                            let path = asset_de_uri(uri, "", Path::new(""))
                                .and_then(|target| caminho_do_import(&self.asset, &target))
                                .ok_or_else(|| {
                                    refuse("type injected into the child without an import path")
                                })?;
                            format!("{}{ty}", self.imp.q(&path))
                        }
                        // `const OpaqueToken<T>('x')`, with the imports in text
                        // order.
                        other => resolver_tardios(self.imp, &expr_do_token(other)),
                    };
                    let method = if *opcional {
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
        let call = resolver_tardios(self.imp, &format!("{vd}.{class}({})", args.join(", ")));
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
        e: &crate::html::Elemento,
        child: &Filho,
        inst_field: &str,
        n: u32,
    ) -> Result<(), Recusa> {
        self.content_queries_on_node(e, &child.consultas, inst_field, n)
    }

    /// `updateQueryAtStartup` of the content queries of the `directives` of
    /// node `n`, in its `afterChildren`, in their order.
    fn directive_queries(
        &mut self,
        e: &crate::html::Elemento,
        directives: &[(std::sync::Arc<crate::directives::Diretiva>, String)],
        n: u32,
    ) -> Result<(), Recusa> {
        for (d, field) in directives {
            if !d.consultas_de_conteudo.is_empty() {
                self.content_queries_on_node(e, &d.consultas_de_conteudo, field, n)?;
            }
        }
        Ok(())
    }

    /// The content queries of a directive or of a child on node `n`, with the
    /// instance read through `inst_field` ([`Self::child_queries`]; the
    /// directive, case j72).
    fn content_queries_on_node(
        &mut self,
        e: &crate::html::Elemento,
        queries: &[ConsultaDoFilho],
        inst_field: &str,
        n: u32,
    ) -> Result<(), Recusa> {
        let refuse = |f: &str| recusa(Motivo::LigacaoEmFilho, f);
        for q in queries {
            // The node's `_queryCount`: every content query of it counts.
            let index = {
                let k = self.consultas_por_no.entry(n).or_default();
                *k += 1;
                *k - 1
            };
            let (uri, class) = match &q.alvo {
                AlvoDeConsulta::Referencia(r) => {
                    if referencia_no_conteudo(&e.filhos, r) {
                        return Err(refuse("child @ContentChild with a #ref in the content"));
                    }
                    if q.lista {
                        self.linhas
                            .push(format!("    this.{inst_field}.{} = [];", q.campo));
                    }
                    continue;
                }
                AlvoDeConsulta::Classe(uri, class) => (uri, class),
            };
            if self.class_in_embedded(&e.filhos, uri, class, false) {
                // Without `descendants`, the `<template>` (with a directive)
                // between the result and the node already puts the distance
                // above 1.
                if !q.descendentes || q.leitura.is_some() {
                    return Err(refuse(
                        "child @ContentChild with a result in an embedded view",
                    ));
                }
                let tree = self.content_tree(&e.filhos, uri, class)?;
                // The single one with the first result in this view is static
                // (`_shouldMapNestedViews`): it goes on below, through the
                // first one.
                let dynamic = if q.lista {
                    tree
                        .iter()
                        .any(|i| matches!(i, ItemDeConteudo::Aninhada { .. }))
                } else {
                    matches!(tree.first(), Some(ItemDeConteudo::Aninhada { .. }))
                };
                if dynamic {
                    self.conteudo_dinamico_no(
                        tree,
                        chave_de_tipo(uri, class),
                        class,
                        n,
                        index,
                        q.lista,
                        format!("this.{inst_field}.{}", q.campo),
                        false,
                    );
                    continue;
                }
            }
            let token = crate::directives::Token::Classe {
                uri: uri.clone(),
                classe: class.clone(),
            };
            let mut values = Vec::new();
            // The results that are an `onPush` component: the
            // `ChangeDetectorRef` of each one is registered before the
            // assignment (`_createAddQueryChangeDetectorRefs` in
            // `_createUpdates`).
            let mut detectors: Vec<(String, String)> = Vec::new();
            for r in &self.registros {
                let Some(pos) = r.acima.iter().position(|(k, _)| *k == n) else {
                    continue;
                };
                let distance = r.acima[pos + 1..].iter().filter(|(_, d)| *d).count();
                if !q.descendentes && distance > 1 {
                    continue;
                }
                for (t, field, on_push_view) in &r.provedores {
                    if *t != token {
                        continue;
                    }
                    // With `read:`, the value comes from another provider,
                    // without a `ChangeDetectorRef`.
                    if let (Some(cv), None) = (on_push_view, &q.leitura) {
                        detectors.push((format!("this.{field}"), cv.clone()));
                    }
                    values.push(match &q.leitura {
                        None => format!("this.{field}"),
                        // `read:` reads another token of the same node.
                        Some(LeituraDaConsulta::Elemento) => r.elemento.clone(),
                        Some(LeituraDaConsulta::Classe(u, c)) => {
                            let read = crate::directives::Token::Classe {
                                uri: u.clone(),
                                classe: c.clone(),
                            };
                            let node = r.acima.last().map(|(k, _)| *k);
                            self.registros
                                .iter()
                                .filter(|x| x.acima.last().map(|(k, _)| *k) == node)
                                .flat_map(|x| &x.provedores)
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
            if !q.lista {
                detectors.retain(|(v, _)| Some(v) == values.first());
            }
            if !detectors.is_empty() {
                let v = self.imp.alias(VIEW);
                for (value, cv) in &detectors {
                    self.linhas.push(format!(
                        "    {v}.View.queryChangeDetectorRefs[{value}] = this.{cv};"
                    ));
                }
            }
            if q.lista {
                self.linhas.push(format!(
                    "    this.{inst_field}.{} = [{}];",
                    q.campo,
                    values.join(", ")
                ));
            } else if let Some(first) = values.first() {
                // The single one gets the first, in pre-order; without a
                // result, nothing.
                self.linhas
                    .push(format!("    this.{inst_field}.{} = {first};", q.campo));
            }
        }
        Ok(())
    }

    /// Does some element inside a `*` of the content match the searched class
    /// (a component by its tag or a directive by its selector)?
    fn class_in_embedded(&self, nodes: &[No], uri: &str, class: &str, inside: bool) -> bool {
        nodes.iter().any(|node| {
            let No::Elemento(x) = node else { return false };
            // The element of a `*` goes into the embedded view; a written
            // `<template>` stays in this one, and only its content belongs to
            // the embedded one.
            let is_template = x.estrela.as_ref().is_some_and(|l| l.nome == MARCA_DE_MOLDE);
            let below = inside || x.estrela.is_some();
            let inside = inside || (x.estrela.is_some() && !is_template);
            let mut stripped = x.clone();
            stripped.estrela = None;
            // The token also matches through the node's `providers:` (the
            // `ExistingProvider` of a directive, the `FocusableItem` of
            // `FocusItemDirective`): `_getQueriesFor` looks at all the resolved
            // providers of the node.
            let token = crate::directives::Token::Classe {
                uri: uri.to_string(),
                classe: class.to_string(),
            };
            let provides = |d: &crate::directives::Diretiva| {
                (d.uri == uri && d.classe == class)
                    || d.provedores.iter().any(|p| p.token == token)
            };
            let matches = self.filhos.get(&x.nome).is_some_and(|f| {
                (f.uri_dart == uri && f.classe == class)
                    || f.metadados.as_deref().is_some_and(provides)
            }) || diretivas_casadas(self.usadas, &stripped)
                .iter()
                .any(|d| (d.uri == uri && d.classe == class) || provides(d));
            (inside && matches) || self.class_in_embedded(&x.filhos, uri, class, below)
        })
    }

    /// `bindDirectiveAfterChildrenCallbacks` of each directive of the node, in
    /// their order, after the children (bottom-up): `ngAfterContent*` and
    /// `ngAfterView*` (`Init` in an `if (firstCheck)`, `addStmtsIfFirstCheck`)
    /// and `ngOnDestroy` (case j65).
    fn hooks_after_children(
        &mut self,
        directives: &[(std::sync::Arc<crate::directives::Diretiva>, String)],
    ) {
        for (d, field) in directives {
            let g = &d.ganchos;
            if g.after_content_init {
                self.usa_primeira_checagem = true;
                na_primeira_checagem(
                    &mut self.apos_conteudo,
                    &[format!("      this.{field}.ngAfterContentInit();")],
                );
            }
            if g.after_content_checked {
                self.apos_conteudo
                    .push(format!("    this.{field}.ngAfterContentChecked();"));
            }
            if g.after_view_init {
                self.usa_primeira_checagem = true;
                na_primeira_checagem(
                    &mut self.apos_visao,
                    &[format!("      this.{field}.ngAfterViewInit();")],
                );
            }
            if g.after_view_checked {
                self.apos_visao
                    .push(format!("    this.{field}.ngAfterViewChecked();"));
            }
            if g.on_destroy {
                self.destruir
                    .push(format!("    this.{field}.ngOnDestroy();"));
            }
        }
    }

    /// The hooks that the official compiler binds after visiting the child's
    /// content (`bindDirectiveAfterChildrenCallbacks`): `ngAfterContent*`,
    /// `ngAfterView*` and `ngOnDestroy`.
    fn after_children(&mut self, child: &Filho, inst_field: &str) {
        let g = &child.ganchos;
        if g.after_content_init {
            self.usa_primeira_checagem = true;
            na_primeira_checagem(
                &mut self.apos_conteudo,
                &[format!("      this.{inst_field}.ngAfterContentInit();")],
            );
        }
        if g.after_content_checked {
            self.apos_conteudo
                .push(format!("    this.{inst_field}.ngAfterContentChecked();"));
        }
        if g.after_view_init {
            self.usa_primeira_checagem = true;
            na_primeira_checagem(
                &mut self.apos_visao,
                &[format!("      this.{inst_field}.ngAfterViewInit();")],
            );
        }
        if g.after_view_checked {
            self.apos_visao
                .push(format!("    this.{inst_field}.ngAfterViewChecked();"));
        }
        if g.on_destroy {
            self.destruir
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
        bound: &[(&crate::html::Ligacao, bool)],
        child: &Filho,
        inst_field: &str,
        view_field: &str,
    ) -> Result<(), Recusa> {
        // The `bool` type of the input (`_isBoolType`), from the child's
        // metadata: an attribute without a value binds `true` on it, `''` on
        // the others (case j73).
        let is_bool = |name: &str| {
            child
                .metadados
                .as_ref()
                .and_then(|m| m.entrada(name))
                .and_then(|x| x.booleana)
        };
        let spec: Vec<(String, String, Option<bool>)> = child
            .entradas
            .iter()
            .map(|e| (e.nome.clone(), e.campo.clone(), is_bool(&e.nome)))
            .collect();
        let view = child.on_push.then_some(view_field);
        self.inputs_of(
            bound,
            &spec,
            inst_field,
            view,
            &child.ganchos,
            Motivo::LigacaoEmFilho,
        )
    }

    /// `bindDirectiveInputs` and `bindDirectiveDetectChangesLifecycleCallbacks`
    /// of a directive or component: `spec` are the declared `@Input`s (name,
    /// member, whether it is `bool` when known), `on_push` the view of the
    /// `onPush` component that marks the check.
    fn inputs_of(
        &mut self,
        bound: &[(&crate::html::Ligacao, bool)],
        spec: &[(String, String, Option<bool>)],
        inst_field: &str,
        on_push: Option<&str>,
        g: &crate::component::Ganchos,
        reason: Motivo,
    ) -> Result<(), Recusa> {
        let dbg = tardio(CHECK_BINDING);
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
                self.usa_changed = true;
                self.entradas.push("    changed = false;".to_string());
            }
            let mut order: Vec<&(&crate::html::Ligacao, bool)> = bound.iter().collect();
            order.sort_by_key(|(l, _)| {
                spec.iter()
                    .position(|x| x.0 == l.nome)
                    .unwrap_or(usize::MAX)
            });
            let dev = tardio(DEVTOOLS);
            let mut constants = Vec::new();
            let mut dynamic = Vec::new();
            for (l, is_static) in order {
                let Some((_, field, is_bool)) = spec.iter().find(|x| x.0 == l.nome).cloned()
                else {
                    // A name that the child does not declare as an `@Input`: it
                    // may be a directive.
                    return Err(recusa(reason, "[x] that is not a child @Input"));
                };
                let name = &l.nome;
                let (start, end) = (l.inicio, l.fim);
                let changed_stmt = if computes_changed { "\nchanged = true;" } else { "" };
                // An attribute with `{{ }}` on an `@Input` (`x="a {{b}}"`): the
                // interpolation goes into the input — mutable, checked and
                // assigned (the primitive alone, raw and interpolated in the
                // assignment); immutable, on the first check (case j58).
                if *is_static && l.valor.contains("{{") {
                    let v = self.valor_interpolado(&l.valor, reason)?;
                    if v.imutavel {
                        let value = v.constante;
                        let block = format!(
                            "if ({dev}.isDevToolsEnabled) {{\n  {dev}.Inspector.instance.recordInput(this.{inst_field}, '{name}', {value});\n}}\nthis.{inst_field}.{field} = {value} /* REF:{url}:{start}:{end} */;{changed_stmt}"
                        );
                        constants.push(indentar(&block, 6));
                        continue;
                    }
                    let k = v.k;
                    let expr = literal(&l.valor);
                    let chk = tardio(CHECK_BINDING);
                    let (check, in_action) = (v.checagem, v.na_acao);
                    let changed_stmt = if computes_changed {
                        "\n      changed = true;"
                    } else {
                        ""
                    };
                    self.campos_expr.push(format!("  Object? _expr_{k};"));
                    dynamic.push(format!(
                        "    final currVal_{k} = {check};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, {expr}, '{url}')) {{\n      if ({dev}.isDevToolsEnabled) {{\n        {dev}.Inspector.instance.recordInput(this.{inst_field}, '{name}', {in_action});\n      }}\n      this.{inst_field}.{field} = {in_action} /* REF:{url}:{start}:{end} */;{changed_stmt}\n      this._expr_{k} = currVal_{k};\n    }}"
                    ));
                    continue;
                }
                let k = self.proxima_ligacao;
                self.proxima_ligacao += 1;
                // A static attribute: the `LiteralPrimitive` of the text;
                // without a value, `EmptyExpr` (`true` on a `bool` input,
                // otherwise `''`).
                // An immutable expression: the same shape (`_bindLiteral`).
                let (value, nullable) = if *is_static {
                    if sem_valor(l) {
                        match is_bool {
                            Some(true) => ("true".to_string(), false),
                            Some(false) => ("''".to_string(), false),
                            None => {
                                return Err(recusa(
                                    reason,
                                    "attribute without a value on a child @Input",
                                ));
                            }
                        }
                    } else {
                        (literal(&l.valor), false)
                    }
                } else {
                    // Only the dynamic one (`final currVal_k = …;`) can break.
                    let c = self.convert_breakable(&l.valor, reason)?;
                    if c.imutavel && c.texto.contains(crate::expr::QUEBRA) {
                        return Err(recusa(reason, "immutable call with a named argument"));
                    }
                    if !c.imutavel {
                        let expr = literal(&l.valor);
                        let chk = tardio(CHECK_BINDING);
                        let value = c.texto;
                        let changed_stmt = if computes_changed {
                            "\n      changed = true;"
                        } else {
                            ""
                        };
                        self.campos_expr.push(format!("  Object? _expr_{k};"));
                        dynamic.push(format!(
                            "    final currVal_{k} = {value};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, {expr}, '{url}')) {{\n      if ({dev}.isDevToolsEnabled) {{\n        {dev}.Inspector.instance.recordInput(this.{inst_field}, '{name}', currVal_{k});\n      }}\n      this.{inst_field}.{field} = currVal_{k} /* REF:{url}:{start}:{end} */;{changed_stmt}\n      this._expr_{k} = currVal_{k};\n    }}"
                        ));
                        continue;
                    }
                    (c.texto, c.pode_ser_nulo)
                };
                if value == "null" {
                    continue;
                }
                let mut block = format!(
                    "if ({dev}.isDevToolsEnabled) {{\n  {dev}.Inspector.instance.recordInput(this.{inst_field}, '{name}', {value});\n}}\nthis.{inst_field}.{field} = {value} /* REF:{url}:{start}:{end} */;{changed_stmt}"
                );
                if nullable {
                    block = format!("if (({value} != null)) {{\n{}\n}}", indentar(&block, 2));
                }
                constants.push(indentar(&block, 6));
            }
            if !constants.is_empty() {
                self.usa_primeira_checagem = true;
                na_primeira_checagem(&mut self.entradas, &constants);
            }
            self.entradas.extend(dynamic);
            if let Some(view) = on_push {
                self.entradas.push(format!(
                    "    if (changed) {{\n      this.{view}.markAsCheckOnce();\n    }}"
                ));
            }
        }
        if after_changes {
            self.usa_changed = true;
            self.entradas.push(format!(
                "    if (changed) {{\n      this.{inst_field}.ngAfterChanges();\n    }}"
            ));
        }
        if g.on_init {
            self.usa_primeira_checagem = true;
            self.entradas.push(format!(
                "    if (((!{dbg}.debugThrowIfChanged) && firstCheck)) {{\n      this.{inst_field}.ngOnInit();\n    }}"
            ));
        }
        if g.do_check {
            self.entradas.push(format!(
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
        node: &crate::html::Elemento,
        matched: &[std::sync::Arc<crate::directives::Diretiva>],
        r: &crate::directives::NoResolvido,
        target: &str,
    ) -> Result<(), Recusa> {
        // The `@HostListener`s in the order of the directives in `directives:`
        // (`_collectHostListeners`), grouped by event in the order in which
        // each one first appears: two listeners of the same event become one
        // `_handleEvent_N` that calls both.
        let mut groups: Vec<(String, Vec<(String, crate::directives::Ouvinte)>)> = Vec::new();
        for d in matched {
            let Some((_, field)) = r
                .diretivas
                .iter()
                .find(|(x, _)| std::sync::Arc::ptr_eq(x, d))
            else {
                continue;
            };
            for o in &d.ouvintes {
                match groups.iter_mut().find(|(ev, _)| *ev == o.evento) {
                    Some((_, l)) => l.push((field.clone(), o.clone())),
                    None => groups.push((o.evento.clone(), vec![(field.clone(), o.clone())])),
                }
            }
        }
        // The template events come first, and the `@HostListener` of the same
        // event goes into their handler, after the written action
        // (`mergeEvents` over the element's outputs, case j70).
        let mut from_template: Vec<&str> = Vec::new();
        for l in &node.eventos {
            if from_template.contains(&l.nome.as_str()) {
                return Err(recusa(
                    Motivo::Evento,
                    "two handlers of the same event in the template",
                ));
            }
            from_template.push(&l.nome);
        }
        for l in &node.eventos {
            let extras: Vec<String> = groups
                .iter()
                .filter(|(ev, _)| *ev == l.nome)
                .flat_map(|(_, list)| list)
                .map(|(field, o)| format!("this.{field}.{}({})", o.metodo, o.args))
                .collect();
            self.tipo_do_evento_falha = None;
            self.tipo_do_evento = tipo_de_evento_nativo(&l.nome);
            match self.handler_with(&[&l.valor], &extras) {
                Ok(h) => self.listener(&l.nome, target, &h),
                Err(r) => self.anotar(r)?,
            }
        }
        for (event, list) in &groups {
            if from_template.contains(&event.as_str()) {
                continue;
            }
            self.tipo_do_evento_falha = None;
            self.tipo_do_evento = tipo_de_evento_nativo(event);
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
        e: &crate::html::Elemento,
        child: &Filho,
        el: &str,
        directives: Option<(
            &[std::sync::Arc<crate::directives::Diretiva>],
            &crate::directives::NoResolvido,
        )>,
    ) -> Result<(), Recusa> {
        let mut element_only = e.clone();
        element_only
            .eventos
            .retain(|l| child.saida(&l.nome).is_none());
        // With directives on the node, their `@HostListener`s (case j93).
        match directives {
            Some((matched, r)) if matched.iter().any(|d| !d.ouvintes.is_empty()) => {
                self.listeners_with_host(&element_only, matched, r, el)
            }
            _ => self.events(&element_only, el),
        }
    }

    /// The type of `$event` of the handler of an `@Output` (`member` of class
    /// `class` from `uri`): in 9, the `T` of its `Stream<T>` through
    /// `fromDartType` (§B4); in 8, none.
    fn output_type(&self, uri: &str, class: &str, member: &str) -> Result<Option<String>, Recusa> {
        if crate::dialect::atual() == crate::dialect::Dialeto::Ngdart {
            return Ok(None);
        }
        let failure = || recusa(Motivo::Evento, "@Output without the Stream type");
        let (r, _) = self.tipos.ok_or_else(failure)?;
        let (t, scope) = r.tipo_da_saida(uri, class, member).ok_or_else(failure)?;
        if t == "dynamic" {
            return Ok(Some(t));
        }
        let full = instanciar_crus(&t, &scope, r).ok_or_else(failure)?;
        tipo_qualificado(&full, &scope, r, &self.asset, &[])
            .map(Some)
            .ok_or_else(failure)
    }

    /// The type of `$event` for the handler of an `@Output`; without it, the
    /// failure is kept for the complex handler, the only one that writes it.
    fn set_output_type(&mut self, uri: &str, class: &str, member: &str) {
        match self.output_type(uri, class, member) {
            Ok(t) => {
                self.tipo_do_evento = t;
                self.tipo_do_evento_falha = None;
            }
            Err(r) => {
                self.tipo_do_evento = None;
                self.tipo_do_evento_falha = Some(r);
            }
        }
    }

    /// The child's `@Output`s bound in the template: `subscription_N`
    /// (`bindDirectiveOutputs`), at the child's turn among the node's
    /// directives.
    fn child_outputs(
        &mut self,
        e: &crate::html::Elemento,
        child: &Filho,
        inst_field: &str,
    ) -> Result<(), Recusa> {
        let mut seen: Vec<&str> = Vec::new();
        for l in e.eventos.iter().filter(|l| child.saida(&l.nome).is_some()) {
            if seen.contains(&l.nome.as_str()) {
                return Err(recusa(
                    Motivo::Evento,
                    "two handlers of the same event in the template",
                ));
            }
            seen.push(&l.nome);
            let member = child.saida(&l.nome).unwrap_or_default().to_string();
            self.set_output_type(&child.uri_dart, &child.classe, &member);
            match self.handler(&[&l.valor]) {
                Ok(h) => {
                    let k = self.subscricoes;
                    self.subscricoes += 1;
                    self.ouvintes.push(format!(
                        "    final subscription_{k} = this.{inst_field}.{member}.listen({h});"
                    ));
                }
                Err(r) => self.anotar(r)?,
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
        e: &crate::html::Elemento,
        star: &crate::html::Ligacao,
        parent: &str,
    ) -> Result<(), Recusa> {
        let url = self.url(Motivo::Ligacao)?;
        let Some(dir) = Estrutural::conhecida(&star.nome) else {
            return Err(recusa(
                Motivo::Ligacao,
                "`*` of a directive that is not ngIf/ngFor/ngSwitch",
            ));
        };
        // `@Host()`: the provider on an element above, in this view or in an
        // ancestor one (the read already comes through the `parentView` chain).
        // With a component in the way, the injector may be a different one:
        // refusal.
        let host_read = match dir.hospedeiro {
            None => None,
            Some(class) => {
                let token = crate::directives::Token::Classe {
                    uri: dir.uri.to_string(),
                    classe: class.to_string(),
                };
                let found = self
                    .providers_above()
                    .into_iter()
                    .find(|p| p.token == token)
                    .map(|p| p.leitura);
                match found {
                    Some(l) if self.componentes_acima == 0 => Some(l),
                    _ => {
                        return Err(recusa(
                            Motivo::DiretivaPorSeletor,
                            format!("{} without the {class} above", dir.classe),
                        ));
                    }
                }
            }
        };
        let mut micro = e.micro_da_estrela().unwrap_or_default();
        // The bindings are emitted in the order of the directive's inputs, not
        // in the written one (`_orderingOf(directive.inputs)`). An input that
        // the directive does not declare is an error in the official compiler
        // ("Can't bind to ..."): there is no output.
        if let Some((p, _)) = micro
            .propriedades
            .iter()
            .find(|(p, _)| !dir.entradas.contains(&p.as_str()))
        {
            return Err(recusa(
                Motivo::Ligacao,
                format!("`{p}` is not an input of {}", dir.classe),
            ));
        }
        micro
            .propriedades
            .sort_by_key(|(p, _)| dir.entradas.iter().position(|x| x == p));
        self.template_guard(&star.nome, &micro, Some(&dir))?;
        // The `REF` of each input: the span of the binding written in a
        // `<template dir [dirX]>` ([`molde_com_diretiva`]); for `*`, the span
        // of the whole attribute.
        let span = |prop: &str| {
            e.ligacoes_do_molde
                .iter()
                .find(|l| l.nome == prop)
                .map_or((star.inicio, star.fim), |l| (l.inicio, l.fim))
        };
        let n = self.proximo;
        self.proximo += 1;
        let dom = self.dom();
        let vc = self.imp.q(VIEW_CONTAINER);
        let tr = self.imp.q(TEMPLATE_REF);
        let qd = self.imp.q(dir.uri);
        let dev = self.imp.alias(DEVTOOLS);
        let dir_class = dir.classe;
        // The number of this view, and the jump its subtree consumes.
        let index = self.proxima_embutida;
        self.proxima_embutida += 1 + contar_estruturais(&e.filhos);
        let factory_name = format!("viewFactory_{}{index}", &self.classe_da_visao[4..]);
        // In 9 there is the extra `JSObject` among the built-ins (§B3).
        let x = crate::dialect::embutidos_extra();
        let (k_tr, k_dir) = (8 + x, 9 + x);
        let field = format!("_{dir_class}_{n}_{k_dir}");
        // In a `<template>` the built-in providers take 0..7 and the
        // `TemplateRef` is number 8 — see docs/GERADOR-NG.md §2.
        self.campos_filho
            .push(format!("  late final {vc}ViewContainer _appEl_{n};"));
        self.campos_filho
            .push(format!("  late final {qd}{dir_class} {field};"));
        self.ancoras.push(format!("_appEl_{n}"));
        // The second argument is the index of the parent element, and `null`
        // when the anchor is at the root of the view (`isRootElement ? null :
        // parent.nodeIndex`, in `compile_element.dart`); loose in projected
        // content, the parent is the child that receives it.
        let parent_index = if parent.is_empty() {
            self.raizes.push(format!("this._appEl_{n}"));
            self.linhas
                .push(format!("    final _anchor_{n} = {dom}.createAnchor();"));
            self.pai_projetado
                .map_or_else(|| "null".to_string(), |k| k.to_string())
        } else {
            self.linhas.push(format!(
                "    final _anchor_{n} = {dom}.appendAnchor({parent});"
            ));
            indice_do_elemento(parent)
        };
        self.linhas.push(format!(
            "    this._appEl_{n} = {vc}ViewContainer({n}, {parent_index}, this, _anchor_{n});"
        ));
        self.linhas.push(format!(
            "    var _TemplateRef_{n}_{k_tr} = {tr}TemplateRef(this._appEl_{n}, {});",
            self.template_factory(&factory_name)
        ));
        let extra = host_read.map(|h| format!(", {h}")).unwrap_or_default();
        let template_arg = if dir.com_template {
            format!(", _TemplateRef_{n}_{k_tr}")
        } else {
            String::new()
        };
        self.linhas.push(format!(
            "    this.{field} = {qd}{dir_class}(this._appEl_{n}{template_arg}{extra});"
        ));
        self.linhas.push(format!(
            "    if ({dev}.isDevToolsEnabled) {{\n      {dev}.Inspector.instance.registerDirective(_anchor_{n}, this.{field});\n    }}"
        ));

        // The inputs of the directive, in the order in which the microsyntax
        // declares them (it is the order of the binding indices).
        // `bindAndWriteToRenderer` writes the immutable ones (`if
        // (firstCheck)`) before the others.
        let mut collection_type = None;
        let mut dynamic = Vec::new();
        for (prop, expr) in &micro.propriedades {
            let c = self.convert(expr, Motivo::Ligacao)?;
            if prop.ends_with("Of") {
                collection_type = c.tipo.clone().map(|t| (t, c.escopo.clone()));
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
                    self.membros.contains_key(name)
                        || self.metodos.contains_key(name)
                        || self.tipos.is_some_and(|(r, file)| {
                            let class = self.classe_qualificada.rsplit('.').next().unwrap_or("");
                            r.tipo_do_membro(file, class, name).is_some()
                                || r.metodo(file, class, name).is_some()
                        })
                };
                let untyped: Vec<String> = self
                    .locais
                    .iter()
                    .filter(|(name, l)| {
                        l.tipo == "dynamic" && cita_na_raiz(expr, name) && is_member(name)
                    })
                    .map(|(name, _)| name.clone())
                    .collect();
                if !untyped.is_empty() {
                    let saved = self.locais.clone();
                    for name in &untyped {
                        self.locais.remove(name);
                    }
                    let typed_only = self.convert(expr, Motivo::Ligacao);
                    self.locais = saved;
                    collection_type = typed_only
                        .ok()
                        .and_then(|x| x.tipo.map(|t| (t, x.escopo)))
                        .or_else(|| Some(("dynamic".to_string(), None)));
                }
            }
            // An immutable input (`final List<X> items`, `*ngIf="fixed"`) is
            // written once, in the inputs' `if (firstCheck)`: directly on
            // `NgIf` (`_directBinding` in the constants method); through
            // `_bindLiteral` on `NgFor`, which consumes the binding index and
            // puts the `if (x != null)` around what can be null.
            if c.imutavel {
                let (start, end) = span(prop);
                let value = &c.texto;
                let dev_rec = format!(
                    "if ({dev}.isDevToolsEnabled) {{\n  {dev}.Inspector.instance.recordInput(this.{field}, '{prop}', {value});\n}}"
                );
                let set = format!("this.{field}.{prop} = {value} /* REF:{url}:{start}:{end} */;");
                let mut statements = vec![dev_rec, set];
                if !dir.direta {
                    self.proxima_ligacao += 1;
                    if value == "null" {
                        continue;
                    }
                    if c.pode_ser_nulo {
                        let inner: Vec<String> =
                            statements.iter().map(|i| indentar(i, 2)).collect();
                        statements = vec![format!(
                            "if (({value} != null)) {{\n{}\n}}",
                            inner.join("\n")
                        )];
                    }
                }
                let blocks: Vec<String> = statements.iter().map(|i| indentar(i, 6)).collect();
                self.usa_primeira_checagem = true;
                na_primeira_checagem(&mut self.entradas, &blocks);
                continue;
            }
            let value = c.texto;
            let (start, end) = span(prop);
            if dir.direta {
                // The ngcompiler's `_isDirectBinding`: `NgIf` already compares
                // the value before acting, so there is no `checkBinding`
                // outside.
                dynamic.push(format!(
                    "    if ({dev}.isDevToolsEnabled) {{\n      {dev}.Inspector.instance.recordInput(this.{field}, '{prop}', {value});\n    }}\n    this.{field}.{prop} = {value} /* REF:{url}:{start}:{end} */;"
                ));
            } else {
                let k = self.proxima_ligacao;
                self.proxima_ligacao += 1;
                self.campos_expr.push(format!("  Object? _expr_{k};"));
                let chk = tardio(CHECK_BINDING);
                // The name that goes into the check is the expression of this
                // property (`items`), not the whole value of the `*`.
                let text = expr.trim();
                dynamic.push(format!(
                    "    final currVal_{k} = {value};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, '{text}', '{url}')) {{\n      if ({dev}.isDevToolsEnabled) {{\n        {dev}.Inspector.instance.recordInput(this.{field}, '{prop}', currVal_{k});\n      }}\n      this.{field}.{prop} = currVal_{k} /* REF:{url}:{start}:{end} */;\n      this._expr_{k} = currVal_{k};\n    }}"
                ));
            }
        }
        self.entradas.extend(dynamic);
        if dir.do_check {
            let chk = tardio(CHECK_BINDING);
            self.entradas.push(format!(
                "    if ((!{chk}.debugThrowIfChanged)) {{\n      this.{field}.ngDoCheck();\n    }}"
            ));
        }

        // The loop locals, typed by the element of the collection.
        let locals = self.micro_locals(&micro, collection_type.as_ref())?;
        let mut without_star = e.clone();
        without_star.estrela = None;
        self.push_embedded(
            e,
            vec![No::Elemento(without_star)],
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
    fn molde(&mut self, e: &crate::html::Elemento, parent: &str) -> Result<(), Recusa> {
        let mut template_locals = Vec::new();
        let mut attributes = Vec::new();
        for a in &e.atributos {
            match a.nome.strip_prefix("let-") {
                Some(name) => template_locals.push((
                    name.to_string(),
                    if a.valor.is_empty() {
                        "$implicit".to_string()
                    } else {
                        a.valor.clone()
                    },
                )),
                None => attributes.push(a),
            }
        }
        let mut stripped = e.clone();
        stripped.estrela = None;
        let desc = crate::selector::Elemento::do_template(&stripped);
        if let Some(u) = self.usadas.iter().find(|u| {
            (u.filho.is_some() || u.diretiva.is_none())
                && crate::selector::casa_algum(&u.seletores, &desc)
        }) {
            return Err(recusa(
                Motivo::DiretivaPorSeletor,
                format!("directive {} on <template> without metadata", u.classe),
            ));
        }
        // The directives of the `<template>` in `directives:` order
        // (`_matchTemplateDirectives`), resolved like those of an element: the
        // node's `TemplateRef` and `ViewContainerRef`, dependencies from above,
        // inputs, outputs and hooks (case j85). The `@HostBinding` of one of
        // them is not written on the `<template>` (`visitEmbeddedTemplate` does
        // not call `bindDirectiveHostProps`).
        let matched = diretivas_casadas(self.usadas, &stripped);
        if matched.is_empty() && !attributes.is_empty() {
            return Err(recusa(
                Motivo::Ligacao,
                "attribute on <template> without a directive",
            ));
        }
        // `[x]` and `(x)` on a `<template>` only exist as an input or output of
        // one of its directives (the rest is an error in the official
        // compiler).
        if let Some(l) = stripped
            .propriedades
            .iter()
            .find(|l| !consome_entrada(&matched, &l.nome))
            .or_else(|| {
                stripped.eventos
                    .iter()
                    .find(|l| !consome_saida(&matched, &l.nome))
            })
        {
            return Err(recusa(
                Motivo::Ligacao,
                format!("[{}] on <template> that no directive receives", l.nome),
            ));
        }
        for d in &matched {
            if !d.ouvintes.is_empty() || d.consultas || !d.consultas_de_conteudo.is_empty() {
                return Err(recusa(
                    Motivo::DiretivaPorSeletor,
                    format!(
                        "directive {} on <template> with @HostListener or a query",
                        d.classe
                    ),
                ));
            }
        }
        // A `@ViewChild` with the result on the `<template>` itself: the value
        // would be its `TemplateRef`. Inside, the query maps the embedded view
        // ([`arvore_da_consulta`]).
        if self
            .consultas_dinamicas
            .iter()
            .any(|q| casa_a_chave(e, &q.chave, self.filhos))
        {
            return Err(recusa(Motivo::Ligacao, "@ViewChild on <template>"));
        }
        let n = self.proximo;
        self.proximo += 1;
        let dom = self.dom();
        let vc = self.imp.q(VIEW_CONTAINER);
        let tr = self.imp.q(TEMPLATE_REF);
        let index = self.proxima_embutida;
        self.proxima_embutida += 1 + contar_estruturais(&e.filhos);
        let factory_name = format!("viewFactory_{}{index}", &self.classe_da_visao[4..]);
        self.campos_filho
            .push(format!("  late final {vc}ViewContainer _appEl_{n};"));
        let reference = e.referencias.first().map(|r| r.nome.clone());
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
            .is_some_and(|r| self.moldes_com_container.contains(r));
        let (resolved, k_tr) = if matched.is_empty() {
            (
                None,
                crate::dialect::embutidos_extra() + if force_container { 8 } else { 7 },
            )
        } else {
            let provedores = self.providers_above_with(&tardio(UTILITIES));
            let acima = crate::directives::Acima {
                provedores: &provedores,
                incerto: self.incertos_acima > 0,
                de_visao: self.tokens_de_visao,
            };
            let (r, k) = crate::directives::resolver_de_molde(
                &matched,
                n,
                Some(acima),
                force_container,
                &tr_read,
            )
            .map_err(|f| recusa(Motivo::DiretivaPorSeletor, f))?;
            (Some(r), k)
        };
        if has_ref {
            self.campos_filho.push(format!(
                "  late final {tr}TemplateRef _TemplateRef_{n}_{k_tr};"
            ));
        }
        let parent_index = if parent.is_empty() {
            // With a non-private `ViewContainer`, it is the root (`vcAppEl ??
            // renderNode`, case j85).
            let container = force_container || resolved.as_ref().is_some_and(|r| r.container);
            self.raizes.push(if container {
                format!("this._appEl_{n}")
            } else {
                format!("_anchor_{n}")
            });
            self.linhas
                .push(format!("    final _anchor_{n} = {dom}.createAnchor();"));
            self.pai_projetado
                .map_or_else(|| "null".to_string(), |k| k.to_string())
        } else {
            self.linhas.push(format!(
                "    final _anchor_{n} = {dom}.appendAnchor({parent});"
            ));
            indice_do_elemento(parent)
        };
        self.linhas.push(format!(
            "    this._appEl_{n} = {vc}ViewContainer({n}, {parent_index}, this, _anchor_{n});"
        ));
        match &reference {
            Some(name) => {
                self.linhas.push(format!(
                    "    this._TemplateRef_{n}_{k_tr} = {tr}TemplateRef(this._appEl_{n}, {});",
                    self.template_factory(&factory_name)
                ));
                let read = format!("this._TemplateRef_{n}_{k_tr}");
                self.refs.insert(name.clone(), read.clone());
                self.refs_em_ordem.push((name.clone(), read.clone()));
                if force_container {
                    let vc = format!("this._appEl_{n}");
                    self.refs.insert(chave_de_container(name), vc.clone());
                    self.refs_em_ordem.push((chave_de_container(name), vc));
                }
            }
            None => {
                self.linhas.push(format!(
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
        let above_len = self.acima.len();
        // With `ViewContainerRef` requested, the `ViewContainer` is not private
        // (`createViewContainer(.., !hasViewContainer, ..)`): its views are
        // detected and destroyed by this one (case j85).
        if force_container || resolved.as_ref().is_some_and(|r| r.container) {
            self.ancoras.push(format!("_appEl_{n}"));
        }
        if let Some(r) = &resolved {
            let target = format!("_anchor_{n}");
            self.create_instances(&stripped, &r.instancias, &target, "this")?;
            self.register_directives(&r.diretivas, &target);
            self.bind_directives(
                &stripped,
                &[],
                &r.diretivas,
                &target,
                &stripped.propriedades,
                &stripped.eventos,
                true,
                true,
            )?;
            self.hooks_after_children(&r.diretivas);
            let injectables: Vec<(Vec<crate::directives::Token>, String)> = r
                .instancias
                .iter()
                .filter(|i| !i.injetavel_por.is_empty())
                .map(|i| (i.injetavel_por.clone(), i.leitura.clone()))
                .collect();
            if !injectables.is_empty() {
                self.injetores.push((n, n, injectables));
            }
            let mut acima = self.pilha.clone();
            acima.push((n, true));
            self.registros.push(Registro {
                acima,
                provedores: r
                    .instancias
                    .iter()
                    .flat_map(|i| {
                        std::iter::once(&i.token)
                            .chain(&i.apelidos)
                            .map(|t| (t.clone(), i.leitura.clone(), None))
                    })
                    .collect(),
                elemento: target,
            });
            // The providers of the `<template>` stay above its content.
            for i in &r.instancias {
                if i.preguicosa {
                    self.preguicosos_acima
                        .insert((i.leitura.clone(), self.classe_desta_visao()));
                }
                if i.injetavel_por.is_empty() {
                    self.acima.push((i.token.clone(), i.leitura.clone(), None));
                }
                for t in &i.injetavel_por {
                    self.acima.push((t.clone(), i.leitura.clone(), None));
                }
            }
        }
        // Whoever creates the view (the child's directive, with
        // `ngTemplateOutlet`) is the one that gives the context: the local has
        // no type (`dynamic`, without a cast).
        let mut locals = self.locais.clone();
        for (name, _) in &template_locals {
            locals.insert(
                name.clone(),
                crate::expr::Local {
                    dart: format!("local_{name}"),
                    tipo: "dynamic".to_string(),
                    escopo: None,
                },
            );
        }
        self.push_embedded(
            e,
            e.filhos.clone(),
            n,
            index,
            factory_name,
            parent,
            locals,
            crate::micro::Micro {
                propriedades: Vec::new(),
                locais: template_locals,
            },
        );
        self.acima.truncate(above_len);
        Ok(())
    }

    /// The second argument of `TemplateRef`: the factory of the embedded view.
    /// In a generic component the official compiler wraps it in a closure that
    /// passes the type arguments (`viewFactory_X1<T>(parentView,
    /// parentIndex)`).
    fn template_factory(&self, name: &str) -> String {
        if self.genericos.is_empty() {
            return name.to_string();
        }
        format!(
            "(parentView, parentIndex) {{\n      return {name}{}(parentView, parentIndex);\n    }}",
            self.genericos
        )
    }

    /// Stores the specification of the embedded view of a `*` or `<template>`
    /// (`e`, at anchor `n`), with what it inherits from this one: the locals,
    /// the `#ref`s and the providers above, one `parentView` farther away.
    #[allow(clippy::too_many_arguments)]
    fn push_embedded(
        &mut self,
        e: &crate::html::Elemento,
        nos: Vec<No>,
        n: u32,
        indice: u32,
        factory_name: String,
        parent: &str,
        locais: std::collections::HashMap<String, crate::expr::Local>,
        micro: crate::micro::Micro,
    ) {
        // The official compiler binds the embedded view when the binder reaches
        // the template, after its inputs: the node of this view that it reads
        // is promoted there, between the `_expr_k`s already created and the
        // following ones.
        let pending: Vec<String> = self
            .refs_de_embutidas
            .iter()
            .filter(|name| !self.posicao_de_embutida.contains_key(*name))
            .cloned()
            .collect();
        for name in pending {
            if citado_na_embutida(e, &name, self.filhos) {
                self.posicao_de_embutida
                    .insert(name, self.campos_expr.len());
            }
        }
        // For the new view, the locals of this view and of the ancestor ones
        // end up one `parentView` farther away.
        let mut ancestrais: std::collections::HashMap<String, Origem> = self
            .ancestrais
            .iter()
            .map(|(n, o)| {
                (
                    n.clone(),
                    Origem {
                        niveis: o.niveis + 1,
                        ..o.clone()
                    },
                )
            })
            .collect();
        for (name, key) in &self.locais_proprios {
            ancestrais.insert(
                name.clone(),
                Origem {
                    chave: key.clone(),
                    classe: self.classe_desta.clone(),
                    niveis: 1,
                },
            );
        }
        // An anchor at the root of this view has as parent the same node as the
        // root of this view; when nested, a node of this view.
        let at_top =
            parent == "parentRenderNode" || (parent.is_empty() && self.pai_projetado.is_none());
        let nivel_do_topo = if !at_top || self.nivel_do_topo < self.profundidade {
            self.nivel_do_topo
        } else {
            self.profundidade + 1
        };
        // A view query with its result in here: the anchor and the class go
        // into its chain; if the result is in the new view, it marks the dirty
        // field in `dirtyParentQueriesInternal`; if it is further below, the
        // query stays in transit to the nested ones.
        let new_class = format!("_{}{indice}", self.classe_da_visao);
        let mut pending: Vec<(String, String, u32)> = Vec::new();
        for q in &mut self.consultas_dinamicas {
            let mut l = Vec::new();
            onde_esta(
                std::slice::from_ref(&No::Elemento(e.clone())),
                &q.chave,
                self.filhos,
                false,
                &mut l,
            );
            if l.is_empty() {
                continue;
            }
            q.vista = true;
            pending.push((q.chave.clone(), q.campo.clone(), 1));
        }
        for (r, field, levels) in &self.consultas_em_transito {
            let mut l = Vec::new();
            onde_esta(
                std::slice::from_ref(&No::Elemento(e.clone())),
                r,
                self.filhos,
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
        let estrela = e.estrela.as_ref().map(|l| l.inicio);
        // Every anchor goes into the map: the content queries also read the
        // chain through it ([`resolver_conteudo_dinamico`]).
        if let Some(k) = estrela {
            self.ancoras_de_consulta
                .borrow_mut()
                .insert(k, (format!("_appEl_{n}"), new_class.clone()));
        }
        let mut refs_consultados = Vec::new();
        let mut consultas_em_transito = Vec::new();
        for (r, field, levels) in pending {
            if let Some(k) = estrela {
                self.ancoras_de_consulta
                    .borrow_mut()
                    .insert(k, (format!("_appEl_{n}"), new_class.clone()));
            }
            let mut l = Vec::new();
            onde_esta(&nos, &r, self.filhos, false, &mut l);
            if l.iter()
                .any(|x| matches!(x, Lugar::Raiz | Lugar::NoFilho | Lugar::NoFilhoProjetado))
            {
                refs_consultados.push((r.clone(), field.clone(), levels));
            }
            if l.contains(&Lugar::Embutida) {
                consultas_em_transito.push((r, field, levels));
            }
        }
        // `dirtyParentQueriesInternal` marks each query when its first result
        // in the new view is registered (`_setParentQueryAsDirty`, in the
        // `addQueryResult` of each element, in pre-order): the order is that of
        // the results, not that of the queries.
        let mut sujas = refs_consultados.clone();
        sujas.sort_by_key(|(r, _, _)| primeiro_resultado(&nos, r, self.filhos));
        // The index of the `<ng-content>` is its ordinal in the whole template
        // (`ngContentSelectors`, in pre-order): the embedded view continues the
        // count from here, and this view skips the ones that stay in there.
        let proxima_projecao = self.proxima_projecao;
        self.proxima_projecao += conteudos_em(&nos);
        self.embutidas.push(EspecEmbutida {
            indice,
            estrela,
            ns: self.ns_atual.clone(),
            proxima_projecao,
            refs_consultados,
            sujas,
            consultas_em_transito,
            profundidade: self.profundidade + 1,
            nivel_do_topo,
            classe: format!("_{}{indice}", self.classe_da_visao),
            fabrica: factory_name,
            nos,
            locais,
            micro,
            ancestrais,
            refs_ancestrais: self
                .refs_ancestrais
                .iter()
                .map(|(n, c, k)| (n.clone(), c.clone(), k + 1))
                .chain(
                    self.refs_locais
                        .iter()
                        .map(|n| (n.clone(), self.classe_desta_visao(), 1)),
                )
                .collect(),
            acima: self
                .acima
                .iter()
                .map(|(t, c, v)| {
                    let v = match v {
                        Some((class, n)) => (class.clone(), n + 1),
                        None => (self.classe_desta_visao(), 1),
                    };
                    (t.clone(), c.clone(), Some(v))
                })
                .collect(),
            preguicosos_acima: self.preguicosos_acima.clone(),
            preguicosos_com_pedidos: self.preguicosos_com_pedidos.clone(),
            componentes_acima: self.componentes_acima,
            incertos_acima: self.incertos_acima,
        });
    }

    /// The locals that the `*` puts in the scope of the embedded view, with
    /// their type: `$implicit` of the element of the collection, integer
    /// `index`/`count` and the four booleans of `NgFor`.
    fn micro_locals(
        &self,
        micro: &crate::micro::Micro,
        collection_type: Option<&(String, Option<std::path::PathBuf>)>,
    ) -> Result<std::collections::HashMap<String, crate::expr::Local>, Recusa> {
        let mut locals = self.locais.clone();
        for (name, key) in &micro.locais {
            let (tipo, escopo) = match key.as_str() {
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
                        let (r, file) = self.tipos?;
                        let (x, escopo) = r.tipo_do_membro_livre(
                            e.as_deref().unwrap_or(file),
                            t,
                            "single",
                            self.nomes_genericos,
                        )?;
                        Some((x, Some(escopo)))
                    };
                    let Some((t, escopo)) = collection_type.and_then(|(t, e)| {
                        tipo_do_elemento(t)
                            .map(|x| (x, e.clone()))
                            .or_else(|| via_single(t, e))
                    }) else {
                        return Err(recusa(
                            Motivo::Ligacao,
                            "`*ngFor` local without the element type",
                        ));
                    };
                    (t, escopo)
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
                    tipo,
                    escopo,
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
        dir: Option<&Estrutural>,
    ) -> Result<(), Recusa> {
        let mut desc = crate::selector::Elemento::novo("template");
        if !micro.propriedades.iter().any(|(p, _)| p == name) {
            desc.atributo(name, None);
        }
        for (p, v) in &micro.propriedades {
            desc.atributo(p, Some(v));
        }
        let matched: Vec<&Usada> = self
            .usadas
            .iter()
            .filter(|u| crate::selector::casa_algum(&u.seletores, &desc))
            .collect();
        match (dir, matched.as_slice()) {
            (Some(d), [u]) if u.e_estrutural(d) => Ok(()),
            (_, []) => Err(recusa(
                Motivo::DiretivaPorSeletor,
                "`*` without a matching directive in directives:",
            )),
            _ => {
                let other = matched
                    .iter()
                    .find(|u| dir.is_none_or(|d| !u.e_estrutural(d)))
                    .map(|u| u.classe.clone())
                    .unwrap_or_default();
                Err(recusa(
                    Motivo::DiretivaPorSeletor,
                    format!("directive {other} on <template>"),
                ))
            }
        }
    }

    /// Guard for the directives of an element: no directive from `directives:`
    /// may match it, except the component that the emitter is going to
    /// instantiate by its tag.
    fn element_guard(&self, e: &crate::html::Elemento) -> Result<(), Recusa> {
        if self.usadas.is_empty() {
            return Ok(());
        }
        let desc = crate::selector::Elemento::do_template(e);
        let child = self.filhos.get(&e.nome);
        for u in self.usadas {
            if !crate::selector::casa_algum(&u.seletores, &desc) {
                continue;
            }
            let is_the_child = match (&u.filho, child) {
                (Some(f), Some(g)) => f.classe == g.classe && f.uri_dart == g.uri_dart,
                _ => false,
            };
            if is_the_child {
                continue;
            }
            if u.filho.is_some() {
                return Err(recusa(
                    Motivo::DiretivaPorSeletor,
                    format!("component {} by a compound selector", u.classe),
                ));
            }
            // A directive on an HTML element: the emitter instantiates it if
            // the metadata read from the program has nothing beyond what it
            // writes (`Diretiva::pendencia`).
            // On a component's element (`<li-select [(ngModel)]>`), without
            // `@HostListener`: directive listeners there are not emitted yet.
            let pending = match &u.diretiva {
                None => Some("without metadata".to_string()),
                Some(d) => d.pendencia(),
            };
            if let Some(p) = pending {
                return Err(recusa(
                    Motivo::DiretivaPorSeletor,
                    format!("directive {} ({p})", u.classe),
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
    ) -> Result<(), Recusa> {
        let url = self.url(Motivo::Interpolacao)?;
        // Without a parent: the root of an embedded view (the `*` on an
        // `<ng-container>`) or projected content — the node is the
        // `TextBinding.element` itself, or the `createText(value)` when
        // immutable (`createTextBinding` without a `parentNode`), and goes into
        // the roots or into the projection list.
        let detached = parent.is_empty();
        let converted = self.convert_breakable(expr, Motivo::Interpolacao)?;
        let breaks = converted.texto.contains(crate::expr::QUEBRA);
        // Without the static type it is not possible to choose between
        // `interpolateString`, `interpolate` and `updateTextWithPrimitive` —
        // and choosing wrong changes what the program does.
        let Some(ty) = converted.tipo.clone() else {
            return Err(recusa(
                Motivo::Interpolacao,
                format!("unknown type of {}", converted.forma),
            ));
        };
        let bare = ty.trim_end_matches('?').to_string();
        let n = self.proximo;
        self.proximo += 1;
        // `visitInterpolation` with empty ends: a mutable primitive is read
        // directly (`_isPrimitiveCheck`), a literal becomes the text itself
        // (`o.literal('${value}')`), the rest goes through `interpolateString0`
        // (all `String`) or `interpolate0`. The type is the one from
        // `_TypeResolver`: a ternary, `!x`, `x!`, `??`, an index and a binary
        // that does not add two `String`s are `dynamic`.
        let mutable_primitive = !converted.imutavel && primitivo(&bare);
        let f = if bare == "String" {
            "interpolateString0"
        } else {
            "interpolate0"
        };
        if converted.imutavel {
            // A value that does not change has no binding: the text is computed
            // once, in `build()`, as the official compiler does
            // (`isImmutable`).
            // The emitter writes `appendText(` before the value: the
            // `dom_helpers` import comes first.
            let dom = self.dom();
            let value = if converted.literal {
                valor_de_literal(&converted.texto)
            } else {
                let alias = self.imp.alias(INTERPOLATE);
                format!("{alias}.{f}({})", converted.texto)
            };
            // In `build()` the import of a name from `exports:` is added now,
            // after `interpolate` (case j51).
            let value = resolver_tardios(self.imp, &value);
            if cita_ctx(std::slice::from_ref(&value)) {
                self.usa_ctx_no_build = true;
            }
            if detached {
                self.linhas
                    .push(format!("    final _text_{n} = {dom}.createText({value});"));
                self.raizes.push(format!("_text_{n}"));
            } else {
                self.linhas.push(format!(
                    "    final _text_{n} = {dom}.appendText({parent}, {value});"
                ));
            }
            return Ok(());
        }
        let Some(tb) = self.tb.clone() else {
            return Err(recusa(
                Motivo::Interpolacao,
                "interpolation without a text binding",
            ));
        };
        self.tb_usado = true;
        self.campos.push(format!(
            "  final {tb}.TextBinding _textBinding_{n} = {tb}.TextBinding();"
        ));
        if detached {
            self.raizes.push(format!("this._textBinding_{n}.element"));
        } else {
            self.linhas
                .push(format!("    {parent}.append(this._textBinding_{n}.element);"));
        }
        let access = converted.texto;
        // A call that breaks inside `interpolateString0(..)` gets the
        // formatter's continuation indentation (+4, case j21); inside
        // `updateTextWithPrimitive(..)`, no case.
        if breaks && (mutable_primitive || converted.imutavel) {
            return Err(recusa(
                Motivo::Interpolacao,
                "call with a named argument in a primitive interpolation",
            ));
        }
        let access = crate::expr::recuar(&access, 4);
        let update = if mutable_primitive {
            format!("updateTextWithPrimitive({access})")
        } else {
            // In change detection the import belongs to the moment it is
            // written.
            format!("updateText({}.{f}({access}))", tardio(INTERPOLATE))
        };
        self.deteccao.push(format!(
            "    this._textBinding_{n}.{update} /* REF:{url}:{start}:{end} */;"
        ));
        Ok(())
    }

    /// The value of a text with `{{ }}` (`title="a {{b}}"`, `x="{{y}}"`), as
    /// `_createPropertyForAttribute`/`convertInterpolation` write it:
    /// `interpolateStringN` when everything is a `String`, `interpolateN`
    /// otherwise; a pure literal is the text itself; immutable, computed once.
    /// A mutable primitive expression alone (`_maybeOptimizeInterpolation`) is
    /// checked raw and interpolated only where the value is used (`na_acao`).
    /// Allocates the binding number (`k`).
    ///
    /// The attribute value is the raw one: the ngast tokenizer does not decode
    /// entities in it (`02-parser-de-template.md` R1.5/R1.6), and the `&&` of
    /// `placeholder="{{ a && b }}"` reaches the expression parser like that
    /// (§A6).
    fn valor_interpolado(&mut self, text: &str, reason: Motivo) -> Result<Interpolada, Recusa> {
        let (texts, exprs) = partes_da_interpolacao(text)
            .ok_or_else(|| recusa(reason, "malformed interpolated attribute"))?;
        let mut convertidas = Vec::new();
        for e in &exprs {
            convertidas.push(self.convert(e, reason)?);
        }
        // The type only chooses the family with 1 or 2 expressions; with 3 or
        // more it is always `interpolateN` (`interpolateFallback`).
        let mut types = Vec::new();
        for c in &convertidas {
            match &c.tipo {
                Some(t) => types.push(t.trim_end_matches('?').to_string()),
                None if convertidas.len() > 2 => types.push("dynamic".into()),
                None => {
                    return Err(recusa(reason, format!("unknown type of {}", c.forma)));
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
                let t = if self.preservar_espacos {
                    t.replace(NGSP, " ")
                } else if i == 0 {
                    comprimir_antes(t)
                } else if i == text_count - 1 {
                    comprimir_depois(t)
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
        let imutavel = convertidas.iter().all(|c| c.imutavel);
        let k = self.proxima_ligacao;
        self.proxima_ligacao += 1;
        let interp = tardio(INTERPOLATE);
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
        let expr_texts: Vec<String> = convertidas.iter().map(|c| c.texto.clone()).collect();
        // A literal expression with empty ends is the text itself.
        let pure_literal = convertidas.len() == 1
            && convertidas[0].literal
            && texts[0] == "''"
            && texts[1] == "''";
        let constante = if pure_literal {
            valor_de_literal(&convertidas[0].texto)
        } else {
            value_with(&expr_texts, family)
        };
        let primitive = convertidas.len() == 1 && !convertidas[0].imutavel && primitivo(&types[0]);
        let (checagem, na_acao) = if primitive {
            (
                convertidas[0].texto.clone(),
                value_with(&[format!("currVal_{k}")], "interpolate"),
            )
        } else {
            (value_with(&expr_texts, family), format!("currVal_{k}"))
        };
        Ok(Interpolada {
            convertidas,
            imutavel,
            k,
            constante,
            checagem,
            na_acao,
        })
    }

    /// An attribute with `{{ }}` (`title="a {{b}}"`): the official compiler
    /// treats it as a property binding (`_createPropertyForAttribute`), with
    /// the name passed through the schema (`class` → `className`, which becomes
    /// `updateChildClass`) and the value `Interpolation(strings, exprs)`. It is
    /// emitted after the element's `[x]` bindings.
    fn interpolated_attribute(
        &mut self,
        a: &crate::html::Ligacao,
        target: &str,
    ) -> Result<Ligada, Recusa> {
        let url = self.url(Motivo::Interpolacao)?;
        let name = a.nome.as_str();
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
            return Err(recusa(
                Motivo::Interpolacao,
                "interpolated attribute renamed, protected or outside the schema",
            ));
        }
        // The value is the raw one (without decoding entities, §A6).
        let v = self.valor_interpolado(&a.valor, Motivo::Interpolacao)?;
        // The value of the binding is the `Interpolation`, not the expression
        // inside: it is never null (`canBeNull` gives `false`, hence the
        // `setAttribute` of `visitAttributeBinding`) and `_TypeResolver` types
        // it as `String`. In the primitive shortcut
        // (`_shouldInterpolateAfterCheck`) the check is the raw expression, and
        // the type that counts is its own (`[style.x]` with an `int` gets the
        // `.toString()`, case j107; `calc({{a - b}}%)` does not, the
        // `material_slider`).
        let primitive = v.convertidas.len() == 1 && v.checagem == v.convertidas[0].texto;
        let mut converted = v.convertidas;
        if let Some(c) = converted.first_mut() {
            c.pode_ser_nulo = false;
            if !primitive {
                c.tipo = Some("String".into());
            }
            c.escopo = None;
        }
        let k = v.k;
        let synthetic = crate::html::Ligacao {
            nome: a.nome.clone(),
            valor: a.valor.clone(),
            inicio: a.inicio,
            fim: a.fim,
            sem_valor: a.sem_valor,
        };
        let (start, end) = (a.inicio, a.fim);
        if v.imutavel {
            let action = self.action(&synthetic, target, &v.constante, &converted[0])?;
            return Ok(Ligada::Constante(format!(
                "      {action} /* REF:{url}:{start}:{end} */;"
            )));
        }
        let (check, in_action) = (v.checagem, v.na_acao);
        self.campos_expr.push(format!("  Object? _expr_{k};"));
        let action = self.action(&synthetic, target, &in_action, &converted[0])?;
        let chk = tardio(CHECK_BINDING);
        let source = literal(&a.valor);
        Ok(Ligada::Dinamica(format!(
            "    final currVal_{k} = {check};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, {source}, '{url}')) {{\n      {action} /* REF:{url}:{start}:{end} */;\n      this._expr_{k} = currVal_{k};\n    }}"
        )))
    }

    /// Emits the child nodes of `parent`. Outside collection mode, the first
    /// shape that the emitter does not cover returns `Err` and the whole file
    /// goes back to `build_runner`; in collection mode, each refusal is
    /// recorded and the walk goes on.
    fn nos(&mut self, nodes: &[No], parent: &str) -> Result<(), Recusa> {
        for node in nodes {
            if let Err(r) = self.node(node, parent) {
                self.anotar(r)?;
                // In collection mode, what is below a refused node also counts:
                // descend with any parent (the output is discarded).
                if let No::Elemento(e) = node {
                    self.collect_below(e);
                }
            }
        }
        Ok(())
    }

    /// Collects the refusals of the subtree of an element that was not emitted.
    /// The locals of a `*` go in without a type, so that the expressions that
    /// mention them are examined for what they are.
    fn collect_below(&mut self, e: &crate::html::Elemento) {
        if !self.coletando() || e.filhos.is_empty() {
            return;
        }
        let saved = (
            self.locais.clone(),
            self.embutida,
            self.tb.clone(),
            self.decl_locais.clone(),
            self.locais_raiz.clone(),
            self.locais_proprios.clone(),
        );
        // The text binding is allocated per view; here the output is discarded.
        self.tb.get_or_insert_with(|| "_coleta".into());
        if let Some(micro) = e.micro_da_estrela() {
            // The type of the collection, if its expression converts; otherwise
            // the locals stay `dynamic`.
            let collection = micro
                .propriedades
                .iter()
                .find(|(p, _)| p.ends_with("Of"))
                .and_then(|(_, x)| self.convert(x, Motivo::Ligacao).ok())
                .and_then(|c| c.tipo.map(|t| (t, c.escopo)));
            let locals = self
                .micro_locals(&micro, collection.as_ref())
                .unwrap_or_else(|_| {
                    let mut l = self.locais.clone();
                    for (name, _) in &micro.locais {
                        l.insert(
                            name.clone(),
                            crate::expr::Local {
                                dart: format!("local_{name}"),
                                tipo: "dynamic".into(),
                                escopo: None,
                            },
                        );
                    }
                    l
                });
            self.locais = locals;
            for (name, _) in &micro.locais {
                self.decl_locais.insert(name.clone(), Ok(String::new()));
            }
            // An embedded view emitted further below inherits these locals as
            // ancestor ones (collection only gathers refusals: the origin does
            // not matter).
            self.locais_proprios.extend(micro.locais.iter().cloned());
            self.embutida = true;
        }
        let _ = self.nos(&e.filhos, "_el_coleta");
        (
            self.locais,
            self.embutida,
            self.tb,
            self.decl_locais,
            self.locais_raiz,
            self.locais_proprios,
        ) = saved;
    }

    /// A template node.
    fn node(&mut self, node: &No, parent: &str) -> Result<(), Recusa> {
        match node {
            No::Comentario(_) => {}
            No::Texto(t) => {
                // Without a parent: the root of an embedded view or projected
                // content (`createText` from `dom_helpers`, and the node goes
                // into the roots or into the projection list).
                let n = self.proximo;
                self.proximo += 1;
                let dom = self.dom();
                let text = literal(t);
                if parent.is_empty() {
                    self.linhas
                        .push(format!("    final _text_{n} = {dom}.createText({text});"));
                    self.raizes.push(format!("_text_{n}"));
                } else {
                    self.linhas.push(format!(
                        "    final _text_{n} = {dom}.appendText({parent}, {text});"
                    ));
                }
            }
            No::Elemento(e) => {
                // A hand-written `<template>` is an embedded view, not an HTML
                // element (`EmbeddedTemplateAst`).
                // (What [`template_como_container`] did not rewrite or mark as
                // [`MARCA_DE_MOLDE`] is not translated yet.)
                if e.nome.eq_ignore_ascii_case("template")
                    && e.estrela.as_ref().is_none_or(|l| l.nome != MARCA_DE_MOLDE)
                {
                    return Err(recusa(Motivo::Ligacao, "<template> written in the template"));
                }
                // `<ng-container>` without `*` (`visitNgContainer`): the
                // children go straight to the parent, without a node or an
                // index.
                if e.nome == "ng-container" && e.estrela.is_none() {
                    return self.container(e, parent);
                }
                if e.estrela.is_none() {
                    self.element_guard(e)?;
                }
                // `@i18n` only on an HTML element: on a child it changes the
                // projected content. With `*` the annotation stays on the
                // element, which goes to the embedded view (case j01).
                if !e.anotacoes.is_empty() && self.filhos.contains_key(&e.nome) {
                    return Err(recusa(Motivo::I18n, "@i18n on a child component"));
                }
                // `*` on a child: the child goes to the embedded view, like any
                // element.
                if let (Some(star), true) = (&e.estrela, self.filhos.contains_key(&e.nome)) {
                    return self.structural(e, star, parent);
                }
                if let Some(child) = self.filhos.get(&e.nome).cloned() {
                    return self.child_component(e, &child, parent);
                }
                // A tag that is not HTML, with only a directive
                // (`<pg-breadcrumb-item>`): it is an element typed `Element`
                // (`identifierFromTagName`).
                if !dom::tag_html(&e.nome)
                    && e.nome != "ng-container"
                    && self.namespace_of(e).is_none()
                    && diretivas_casadas(self.usadas, e).is_empty()
                {
                    return Err(recusa(
                        Motivo::ComponenteNoTemplate,
                        "tag that is neither HTML nor a known component",
                    ));
                }
                // The name of an ecosystem directive (`ngClass`, `ngModel`…) in
                // a binding belongs to a directive, it is not a DOM property:
                // emitting `setProperty` there would do something else — unless
                // a directive from the catalog receives it.
                let matched = diretivas_casadas(self.usadas, e);
                if let Some(l) = e
                    .propriedades
                    .iter()
                    .chain(e.eventos.iter())
                    .chain(e.atributos.iter())
                    .chain(e.bananas.iter())
                    .find(|l| {
                        e_de_diretiva(&l.nome)
                            && !consome_entrada(&matched, &l.nome)
                            && !consome_saida(&matched, &l.nome)
                    })
                {
                    return Err(recusa(
                        Motivo::Diretiva,
                        format!(
                            "`{}` without a directive that receives it",
                            l.nome.split('.').next().unwrap_or(&l.nome)
                        ),
                    ));
                }
                if let Some(star) = &e.estrela {
                    if star.nome == MARCA_DE_MOLDE {
                        return self.molde(e, parent);
                    }
                    return self.structural(e, star, parent);
                }
                self.elemento_html(e, parent)?;
            }
            No::Interpolacao { expr, inicio, fim } => {
                self.interpolation(expr, *inicio, *fim, parent)?
            }
            No::Conteudo { seletor } => {
                // `<ng-content>` does not consume a node index; the number is
                // the ordinal of the projection in the template
                // (`ngContentSelectors`), with or without `select`.
                let _ = seletor;
                let i = self.proxima_projecao;
                self.proxima_projecao += 1;
                // Without a parent (the root of an embedded view, or content
                // that goes to another child): the projected nodes go whole
                // into the list of roots (`ProjectedNodes`, a list), case j56.
                if parent.is_empty() {
                    self.raizes
                        .push(format!("{RAIZ_LISTA}this.projectedNodes[{i}]"));
                } else {
                    self.linhas.push(format!("    this.project({parent}, {i});"));
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
    fn container(&mut self, e: &crate::html::Elemento, parent: &str) -> Result<(), Recusa> {
        let has_any = !(e.atributos.is_empty()
            && e.propriedades.is_empty()
            && e.eventos.is_empty()
            && e.bananas.is_empty()
            && e.referencias.is_empty()
            && e.anotacoes.is_empty());
        if has_any {
            return Err(recusa(
                Motivo::Ligacao,
                "<ng-container> with an attribute, binding, #ref or annotation",
            ));
        }
        if parent.is_empty() && self.pai_projetado.is_some() {
            return Err(recusa(
                Motivo::Projecao,
                "<ng-container> in projected content",
            ));
        }
        self.nos(&e.filhos, parent)
    }

    /// An HTML element: creation, attributes, bindings, events, style and the
    /// children.
    fn elemento_html(&mut self, e: &crate::html::Elemento, parent: &str) -> Result<(), Recusa> {
        // The catalog directives that match the node, and their `[(x)]`
        // desugared like the `DesugarVisitor` does: the input `x` and the event
        // `xChange` with `value = $event`.
        let matched = diretivas_casadas(self.usadas, e);
        let mut properties = e.propriedades.clone();
        let mut events = e.eventos.clone();
        for b in &e.bananas {
            let change = format!("{}Change", b.nome);
            if !consome_entrada(&matched, &b.nome) || !consome_saida(&matched, &change) {
                self.anotar(recusa(Motivo::Ligacao, "[(x)] on an HTML element"))?;
                continue;
            }
            properties.push(b.clone());
            events.push(crate::html::Ligacao {
                nome: change,
                valor: format!("{} = $event", b.valor),
                ..b.clone()
            });
        }
        // `#ref="x"`: the node's directive with `exportAs: 'x'` (only one; more
        // than one is an error in the official compiler). The local is its
        // instance (`referenceTokens` in `compile_element.dart`).
        let exported = |r: &crate::html::Ligacao| {
            let found: Vec<_> = matched
                .iter()
                .filter(|d| d.export_as.as_deref() == Some(r.valor.as_str()))
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
        if let Some(r) = e.referencias.iter().find(|r| {
            (!r.valor.is_empty() && exported(r).is_none())
                || !(self.refs_livres.contains(&r.nome)
                    || self.refs_locais.contains(&r.nome)
                    || self.refs_consultados.iter().any(|(n, _, _)| *n == r.nome)
                    || !r.valor.is_empty())
        }) {
            self.anotar(recusa(
                Motivo::Ligacao,
                if !r.valor.is_empty() {
                    "#ref with a value and no node directive exporting it"
                } else if self.refs_ambiguos.contains(&r.nome) {
                    "#ref repeated or shadowed by `let`"
                } else if self.embutida {
                    "#ref in an embedded view"
                } else {
                    "#ref used in an expression"
                },
            ))?;
        }
        let n = self.proximo;
        self.proximo += 1;
        let resolved = if matched.is_empty() {
            None
        } else {
            let provedores = self.providers_above();
            let acima = crate::directives::Acima {
                provedores: &provedores,
                incerto: self.incertos_acima > 0,
                de_visao: self.tokens_de_visao,
            };
            // What queries with `read:` read from this node is eager.
            let queried: Vec<crate::directives::Token> = self
                .leituras
                .iter()
                .filter(|(key, _)| casa_a_chave(e, key, self.filhos))
                .map(|(_, t)| t.clone())
                .collect();
            match crate::directives::resolver_consultado(&matched, n, Some(acima), &queried) {
                Ok(r) => Some(r),
                Err(f) => {
                    self.anotar(recusa(Motivo::DiretivaPorSeletor, f))?;
                    None
                }
            }
        };
        // The result of each query with a provider `read:`: the instance of the
        // token on the node ([`chave_de_leitura`]).
        for (key, t) in self.leituras {
            if !casa_a_chave(e, key, self.filhos) {
                continue;
            }
            let instance = resolved.as_ref().and_then(|r| {
                r.instancias
                    .iter()
                    .find(|i| i.token == *t || i.apelidos.contains(t))
            });
            match instance {
                Some(i) => {
                    let read = format!("this.{}", i.leitura);
                    let k = chave_de_leitura(key, t);
                    self.refs.insert(k.clone(), read.clone());
                    self.refs_em_ordem.push((k, read));
                }
                None => self.anotar(recusa(
                    Motivo::ViewChildEmFilho,
                    "@ViewChild(.., read: T) of a token the element does not provide",
                ))?,
            }
        }
        if !self.tem_doc {
            self.tem_doc = true;
            let document = self.imp.dom("document");
            self.linhas.push(format!("    final doc = {document};"));
        }
        let dom = self.dom();
        // With a namespace, the name becomes `@ns:tag` (`mergeNsAndName`): it
        // is not HTML (`Element`, the `NonHtml` variants) and is emitted with
        // `createElementNS` and a separate `append` (`createElementNs`,
        // `_initializeAndAppendNode`).
        let namespace = self.namespace_of(e);
        if let Some((ns, _)) = &namespace {
            if !matched.is_empty() || self.filhos.contains_key(&e.nome) {
                return Err(recusa(
                    Motivo::ComponenteNoTemplate,
                    "directive on an element with a namespace",
                ));
            }
            if e.atributos.iter().any(|a| a.nome.contains(':')) {
                return Err(recusa(
                    Motivo::Ligacao,
                    "namespaced attribute on an SVG element",
                ));
            }
            if properties.iter().any(|l| {
                !["attr.", "class.", "style."]
                    .iter()
                    .any(|p| l.nome.starts_with(p))
            }) {
                return Err(recusa(
                    Motivo::Ligacao,
                    "[property] on an element with a namespace",
                ));
            }
            let _ = ns;
        }
        let tag = match &namespace {
            Some((ns, name)) => format!("@{ns}:{name}"),
            None => e.nome.to_ascii_lowercase(),
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
            let node_type = if crate::dialect::atual() == crate::dialect::Dialeto::Ngx {
                format!("<{}>", self.imp.dom(dom::tipo_da_tag(&tag)))
            } else {
                String::new()
            };
            format!("{util}.unsafeCast{node_type}(doc.createElement('{tag}'))")
        } else {
            match tag.as_str() {
                "div" => format!("{dom}.appendDiv(doc, {parent})"),
                "span" => format!("{dom}.appendSpan(doc, {parent})"),
                _ => {
                    let ty = self.imp.dom(dom::tipo_da_tag(&tag));
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
            dom::tipo_da_tag(&tag)
        };
        // A `#ref` that only queries with a provider `read:` look for does not
        // read the node.
        let queries_node = |name: &String| {
            self.refs_consultados.iter().any(|(n, _, _)| n == name)
                && !self.refs_so_por_provedor.contains(name)
        };
        let read_as_local = e.referencias.iter().any(|r| {
            r.valor.is_empty() && (self.refs_locais.contains(&r.nome) || queries_node(&r.nome))
        });
        let read_from_embedded = e
            .referencias
            .iter()
            .find(|r| r.valor.is_empty() && self.refs_de_embutidas.contains(&r.nome))
            .map(|r| r.nome.clone());
        let target = if !liga_no_elemento(e, &matched) && !read_as_local {
            self.linhas.push(format!("    final _el_{n} = {creation};"));
            format!("_el_{n}")
        } else {
            let field_type = self.imp.dom(ty);
            let field = format!("  late final {field_type} _el_{n};");
            let query_pos = e
                .referencias
                .iter()
                .filter(|r| r.valor.is_empty() && !self.refs_so_por_provedor.contains(&r.nome))
                .filter_map(|r| {
                    self.refs_consultados
                        .iter()
                        .position(|(n, _, _)| *n == r.nome)
                })
                .min();
            let only_in_event = (!liga_no_elemento(e, &matched))
                .then(|| {
                    e.referencias
                        .iter()
                        .find(|r| r.valor.is_empty() && self.refs_so_em_eventos.contains(&r.nome))
                        .map(|r| r.nome.clone())
                })
                .flatten();
            match (read_from_embedded, query_pos, only_in_event) {
                (Some(name), _, _) => self.campos_el_de_embutidas.push((name, field)),
                (None, Some(k), _) => self.campos_el_consultados.push((k, field)),
                (None, None, Some(name)) => self.campos_el_de_eventos.push((name, field)),
                (None, None, None) => {
                    self.refs_dos_campos_el.insert(
                        field.clone(),
                        e.referencias
                            .iter()
                            .filter(|r| r.valor.is_empty())
                            .map(|r| r.nome.clone())
                            .collect(),
                    );
                    self.campos_el.push(field);
                }
            }
            self.linhas.push(format!("    this._el_{n} = {creation};"));
            format!("this._el_{n}")
        };
        if namespace.is_some() && !parent.is_empty() {
            self.linhas.push(format!("    {parent}.append({target});"));
        }
        // With a `ViewContainer` (a directive that injects `ViewContainerRef`),
        // it is the root (`vcAppEl ?? renderNode`, case j49).
        let container = resolved.as_ref().is_some_and(|r| r.container);
        if parent.is_empty() {
            self.raizes.push(if container {
                format!("this._appEl_{n}")
            } else {
                target.clone()
            });
        }
        if let Some(res) = &resolved {
            self.register_node_by_position(e.inicio, &res.instancias);
        }
        // The result of a `@ViewChild(Directive)` ([`chave_de_tipo`]): the
        // field of each directive of the node, before the `#ref`s (the order of
        // `_resolvedProvidersArray` in `beforeChildren`, case j117).
        if let Some(res) = &resolved {
            for (d, field) in &res.diretivas {
                if d.e_componente {
                    continue;
                }
                let key = chave_de_tipo(&d.uri, &d.classe);
                let read = format!("this.{field}");
                self.refs.insert(key.clone(), read.clone());
                self.refs_em_ordem.push((key, read));
            }
        }
        // `renderNode.toReadExpr()`: the local or the field, depending on how
        // the node was declared.
        for r in &e.referencias {
            // With a value, the instance of the exported directive.
            let read = match (r.valor.is_empty(), exported(r), &resolved) {
                (true, _, _) => target.clone(),
                (false, Some(d), Some(res)) => match res
                    .diretivas
                    .iter()
                    .find(|(x, _)| std::sync::Arc::ptr_eq(x, &d))
                {
                    Some((_, field)) => format!("this.{field}"),
                    None => continue,
                },
                _ => continue,
            };
            self.refs.insert(r.nome.clone(), read.clone());
            self.refs_em_ordem.push((r.nome.clone(), read));
        }
        // The `@i18n` messages of the node: the attributes' one replaces the
        // literal, the children's one replaces the text.
        let mut i18n = if e.anotacoes.is_empty() {
            Vec::new()
        } else {
            match metadados_i18n(e) {
                Ok(m) => m,
                Err(r) => {
                    self.anotar(r)?;
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
            let eligible = e.atributos.iter().any(|a| {
                Some(&a.nome) == attribute.as_ref()
                    && !a.valor.contains("{{")
                    && !a.valor.contains('&')
                    && !matches!(a.nome.as_str(), "class" | "tabindex" | "tabIndex" | "style")
            });
            if !eligible {
                self.anotar(recusa(
                    Motivo::I18n,
                    "@i18n:x without a simple written attribute x",
                ))?;
            }
        }
        // Attributes are emitted in alphabetical order (`_toSortedBindings`).
        let mut attributes = e.atributos.clone();
        attributes.sort_by(|a, b| a.nome.cmp(&b.nome));
        for a in attributes.iter().filter(|a| !a.valor.contains("{{")) {
            let value = match i18n.iter().find(|(k, _)| k.as_ref() == Some(&a.nome)) {
                Some((_, m)) => {
                    let m = m.clone();
                    match self.message(&a.valor, &m) {
                        Ok(field) => field,
                        Err(r) => {
                            self.anotar(r)?;
                            literal(&a.valor)
                        }
                    }
                }
                None => literal(&a.valor),
            };
            if a.nome == "class" {
                // `writeLiteralAttributeValues`: `NonHtml` outside HTML.
                let method = if dom::tag_html(&tag) {
                    "updateChildClass"
                } else {
                    "updateChildClassNonHtml"
                };
                self.linhas
                    .push(format!("    this.{method}({target}, {value});"));
            } else if a.nome == "tabindex" || a.nome == "tabIndex" {
                // `TabIndexBinding` (`binding_converter.dart`): the literal
                // becomes `el.tabIndex = N` (`visitTabIndexBinding`); a
                // non-integer is a compile error in the official compiler.
                match a.valor.trim().parse::<i64>() {
                    Ok(n) if a.valor.trim() == a.valor => {
                        self.linhas.push(format!("    {target}.tabIndex = {n};"));
                    }
                    _ => self.anotar(recusa(Motivo::Ligacao, "non-integer tabindex"))?,
                }
            } else {
                // A written `style` is an attribute like any other
                // (`setAttribute` in `build()`), also together with
                // `[style.x]`, which only writes during change detection (case
                // j74).
                let dom = self.dom();
                let name = &a.nome;
                self.linhas.push(format!(
                    "    {dom}.setAttribute({target}, '{name}', {value});"
                ));
            }
        }
        self.tag_atual = tag.clone();
        // Bindings are numbered in document order — the parent's before the
        // children's —, so they are registered before descending. Events too:
        // the parent's listener comes before the children's, and all of them
        // are emitted together at the end of `build()` (see `ouvintes`).
        let mut bindings = Vec::new();
        // A `[x]` that a directive receives is an input of that directive, not
        // of the node.
        for l in properties
            .iter()
            .filter(|l| !consome_entrada(&matched, &l.nome))
        {
            match self.property(l, &target) {
                Ok(x) => bindings.push(x),
                Err(r) => self.anotar(r)?,
            }
        }
        // Interpolated attributes come after the `[x]`s, in written order
        // (`_visitProperties`).
        // What a directive of the node receives as an input belongs to it (case
        // j59).
        for a in e
            .atributos
            .iter()
            .filter(|a| a.valor.contains("{{") && !consome_entrada(&matched, &a.nome))
        {
            match self.interpolated_attribute(a, &target) {
                Ok(x) => bindings.push(x),
                Err(r) => self.anotar(r)?,
            }
        }
        self.write_bindings(bindings);
        // The listeners are emitted together at the end of `build()`, in the
        // order in which the elements appear (`bindView` after `_buildView`):
        // the template ones that are not a directive output, then the
        // `@HostListener`s of the directives (`_visitHostListeners`).
        let mut node_el = e.clone();
        node_el.eventos = events
            .iter()
            .filter(|l| !consome_saida(&matched, &l.nome))
            .cloned()
            .collect();
        if let Some(r) = &resolved {
            self.listeners_with_host(&node_el, &matched, r, &target)?;
        } else {
            self.events(&node_el, &target)?;
        }
        if self.com_estilo {
            // Style isolation by attribute: the element goes into the
            // component's scope (`shimCssForNode`: `addShimE` on a node typed
            // `Element`, a tag that is not HTML).
            let method = if ty == "Element" {
                "addShimE"
            } else {
                "addShimC"
            };
            self.linhas.push(format!("    this.{method}({target});"));
        }
        let mut injector = None;
        if let Some(r) = &resolved {
            // The `ViewContainer` is born with the `CompileElement`, before the
            // providers (its field before the directives' ones).
            if r.container {
                let vc = self.imp.q(VIEW_CONTAINER);
                let parent_index = if parent.is_empty() {
                    self.pai_projetado
                        .map_or_else(|| "null".to_string(), |k| k.to_string())
                } else {
                    indice_do_elemento(parent)
                };
                self.campos_filho
                    .push(format!("  late final {vc}ViewContainer _appEl_{n};"));
                self.linhas.push(format!(
                    "    this._appEl_{n} = {vc}ViewContainer({n}, {parent_index}, this, {target});"
                ));
                self.ancoras.push(format!("_appEl_{n}"));
            }
            self.node_directives(e, r, &target, &properties, &events)?;
            let injectables: Vec<(Vec<crate::directives::Token>, String)> = r
                .instancias
                .iter()
                .filter(|i| !i.injetavel_por.is_empty())
                .map(|i| (i.injetavel_por.clone(), i.leitura.clone()))
                .collect();
            if !injectables.is_empty() {
                injector = Some(self.injetores.len());
                self.injetores.push((n, n, injectables));
            }
            let mut acima = self.pilha.clone();
            acima.push((n, true));
            let provedores = r
                .instancias
                .iter()
                .flat_map(|i| {
                    std::iter::once(&i.token)
                        .chain(&i.apelidos)
                        .map(|t| (t.clone(), i.leitura.clone(), None))
                })
                .collect();
            self.registros.push(Registro {
                acima,
                provedores,
                elemento: target.clone(),
            });
        }
        // The injectable providers of this node stay above the children
        // (`injetavel_por`: the visible ones and the aliases).
        let above_len = self.acima.len();
        if let Some(r) = &resolved {
            for i in &r.instancias {
                // Visibility only decides `injectorGetInternal` (other views);
                // in the same template a node below finds even a local provider
                // by its token (case j41).
                if i.preguicosa {
                    self.preguicosos_acima
                        .insert((i.leitura.clone(), self.classe_desta_visao()));
                }
                if i.injetavel_por.is_empty() {
                    self.acima.push((i.token.clone(), i.leitura.clone(), None));
                }
                for t in &i.injetavel_por {
                    self.acima.push((t.clone(), i.leitura.clone(), None));
                }
            }
        }
        self.pilha.push((n, resolved.is_some()));
        let r = match &i18n_children {
            Some(m) => self.i18n_children(e, m, &target),
            None => {
                // The descendants inherit the namespace.
                let saved = std::mem::replace(
                    &mut self.ns_atual,
                    namespace.as_ref().map(|(ns, _)| ns.clone()),
                );
                let r = self.nos(&e.filhos, &target);
                self.ns_atual = saved;
                r
            }
        };
        self.pilha.pop();
        self.acima.truncate(above_len);
        if let Some(res) = &resolved {
            self.directive_queries(e, &res.diretivas, n)?;
            self.hooks_after_children(&res.diretivas);
        }
        // `ProviderNode(nodeIndex, nodeIndex + childNodeCount)`.
        if let Some(i) = injector {
            self.injetores[i].1 = self.proximo - 1;
        }
        r
    }

    /// The children of an element with `@i18n`: a single text becomes a message
    /// (`internationalize`, `_textMessage`) and a text node with it. With HTML
    /// inside ([`mensagem_com_html`]) the message is a static method with one
    /// parameter per tag (`createI18nMessage`), and the node is a
    /// `DocumentFragment` (`createHtml`): `final _html_n =
    /// createTrustedHtml(_message_K('<b>', '</b>'));` and
    /// `parent.append(_html_n)`.
    fn i18n_children(
        &mut self,
        e: &crate::html::Elemento,
        m: &MetaI18n,
        target: &str,
    ) -> Result<(), Recusa> {
        let [No::Texto(t)] = e.filhos.as_slice() else {
            return self.i18n_html_children(e, m, target);
        };
        if t.trim().is_empty() {
            return self.anotar(recusa(Motivo::I18n, "@i18n message with HTML or empty"));
        }
        let field = self.message(t, m)?;
        let n = self.proximo;
        self.proximo += 1;
        let dom = self.dom();
        self.linhas.push(format!(
            "    final _text_{n} = {dom}.appendText({target}, {field});"
        ));
        Ok(())
    }

    /// The `@HostListener`s of several directives of the node for the same
    /// event: a `_handleEvent_N` that calls each one, in order.
    fn group_handler(&mut self, list: &[(String, crate::directives::Ouvinte)]) -> String {
        let n = self.metodos_evento.len();
        let body: Vec<String> = list
            .iter()
            .map(|(field, o)| format!("    this.{field}.{}({});", o.metodo, o.args))
            .collect();
        self.metodos_evento.push(format!(
            "\n  void _handleEvent_{n}({}) {{\n{}\n  }}\n",
            parametro_do_evento(&self.tipo_do_evento),
            body.join("\n")
        ));
        format!("this.eventHandler1(this._handleEvent_{n})")
    }

    /// The handler of a directive's `@HostListener`: a method without a
    /// parameter or with `$event` is a tear-off of the instance; with another
    /// argument, a `_handleEvent_N` that calls the method.
    fn host_handler(&mut self, field: &str, o: &crate::directives::Ouvinte) -> String {
        let m = &o.metodo;
        match o.args.as_str() {
            "" => format!("this.eventHandler0(this.{field}.{m})"),
            "$event" => format!("this.eventHandler1(this.{field}.{m})"),
            args => {
                let n = self.metodos_evento.len();
                self.metodos_evento.push(format!(
                    "\n  void _handleEvent_{n}({}) {{\n    this.{field}.{m}({args});\n  }}\n",
                    parametro_do_evento(&self.tipo_do_evento)
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
        e: &crate::html::Elemento,
        r: &crate::directives::NoResolvido,
        target: &str,
        properties: &[crate::html::Ligacao],
        events: &[crate::html::Ligacao],
    ) -> Result<(), Recusa> {
        self.create_instances(e, &r.instancias, target, "this")?;
        self.register_directives(&r.diretivas, target);
        self.bind_directives(
            e,
            &r.instancias,
            &r.diretivas,
            target,
            properties,
            events,
            true,
            true,
        )
    }

    fn create_instances(
        &mut self,
        e: &crate::html::Elemento,
        instances: &[crate::directives::Instancia],
        target: &str,
        // `ChangeDetectorRef` on the node: the child's `componentView`, or the
        // view itself (`compile_element.dart:199`).
        detector: &str,
    ) -> Result<(), Recusa> {
        use crate::directives::{Argumento, Criacao, Token};
        for inst in instances {
            // A child's `providers:` entry that is not an alias: written as in
            // the host view (`ProviderSource.build`). A lazy one is a field
            // with an initializer; what the child or a directive of the node
            // asks for is emitted in `build()`, in provider order (cases j61,
            // j67).
            if let Criacao::Expressao(_) | Criacao::Multi(_) = &inst.criacao {
                let view = self.injector_view();
                if inst.preguicosa {
                    let text = texto_de_provedor_preguicoso(inst, &self.asset, Some(&view))?;
                    let text = resolver_tardios(self.imp, &text);
                    self.campos_preguicosos.push(text);
                    self.posicoes_preguicosas.push(e.inicio);
                    continue;
                }
                let ty = resolver_tardios(self.imp, &tipo_do_provedor(inst, &self.asset)?);
                // `isDevMode` and `debugInjectorWrap` with deferred imports,
                // in text order.
                let util = tardio(UTILITIES);
                let v = Some(view.as_str());
                let value = match &inst.criacao {
                    Criacao::Expressao(x) => {
                        texto_da_expr(x, &inst.token, &self.asset, &util, 4, v)
                    }
                    Criacao::Multi(items) => format!(
                        "[{}]",
                        items
                            .iter()
                            .map(|x| texto_da_expr(x, &inst.token, &self.asset, &util, 4, v))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    _ => unreachable!(),
                };
                let value = resolver_tardios(self.imp, &value);
                self.campos_filho
                    .push(format!("  late final {ty} {};", inst.campo));
                self.linhas
                    .push(format!("    this.{} = {value};", inst.campo));
                continue;
            }
            let ty = match (&inst.criacao, &inst.token) {
                (Criacao::Diretiva { diretiva, .. }, _)
                    if !diretiva.ligacoes_do_hospedeiro.is_empty() =>
                {
                    let prefix = prefixo_do_ngcd(self.imp, &diretiva.uri, &self.asset);
                    let args = self.type_arguments(&diretiva.uri, &diretiva.classe, e);
                    format!("{prefix}{}NgCd{args}", diretiva.classe)
                }
                (Criacao::Diretiva { diretiva, .. }, _) => {
                    let prefix = self.imp.q(&import_de(&diretiva.uri, &self.asset));
                    let args = self.type_arguments(&diretiva.uri, &diretiva.classe, e);
                    format!("{prefix}{}{args}", diretiva.classe)
                }
                (Criacao::Lista(_), Token::Multi { tipo, .. }) if tipo.e_object() => {
                    format!("List<{}Object>", self.imp.q("dart:core"))
                }
                (Criacao::Lista(_), Token::Multi { tipo, .. }) if tipo.genericos > 0 => {
                    let args = vec!["dynamic"; tipo.genericos].join(", ");
                    format!(
                        "List<{}{}<{args}>>",
                        self.imp.q(&import_de(&tipo.uri, &self.asset)),
                        tipo.classe
                    )
                }
                _ => return Err(recusa(Motivo::DiretivaPorSeletor, "provider without a type")),
            };
            let value = match &inst.criacao {
                Criacao::Diretiva { diretiva, args } => {
                    // With `@HostBinding`, the `XNgCd` of the directive's
                    // `.template.dart` wraps the instance.
                    let cd = if diretiva.ligacoes_do_hospedeiro.is_empty() {
                        None
                    } else {
                        Some(prefixo_do_ngcd(self.imp, &diretiva.uri, &self.asset))
                    };
                    // A dependency on the outer injector: the creation is
                    // wrapped in `debugInjectorWrap` under `isDevMode`, and the
                    // emitter writes `isDevMode` and `debugInjectorWrap` before
                    // the class and the tokens (the order of the imports).
                    let injects = args.iter().any(|a| matches!(a, Argumento::DeFora { .. }));
                    let wrapped =
                        injects.then(|| (self.imp.alias(UTILITIES), self.imp.alias(DI_ERRORS)));
                    let class = format!(
                        "{}{}",
                        self.imp.q(&import_de(&diretiva.uri, &self.asset)),
                        diretiva.classe
                    );
                    let v = self.injector_view();
                    let mut texts = Vec::new();
                    for a in args {
                        texts.push(match a {
                            Argumento::Elemento => target.to_string(),
                            Argumento::Detector => detector.to_string(),
                            Argumento::Nulo => "null".to_string(),
                            Argumento::Campo(c) => format!("this.{c}"),
                            Argumento::Acima(read) => read.clone(),
                            Argumento::Injetor(n) => format!("this.injector({n})"),
                            Argumento::Container => {
                                format!("this._appEl_{}", indice_do_elemento(target))
                            }
                            // The attribute written on the element (`attrs`),
                            // without a binding; when absent, `null`.
                            Argumento::Atributo(name) => e
                                .atributos
                                .iter()
                                .find(|a| a.nome == *name)
                                .map_or_else(|| "null".to_string(), |a| literal(&a.valor)),
                            Argumento::DeFora { token, opcional } => {
                                let method = if *opcional {
                                    "injectorGetOptional"
                                } else {
                                    "injectorGet"
                                };
                                let t = expr_do_token(&token_local(token, &self.asset));
                                let t = resolver_tardios(self.imp, &t);
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
                        Some(q) => format!("{q}{}NgCd({creation})", diretiva.classe),
                        None => creation,
                    }
                }
                Criacao::Lista(items) => {
                    let items: Vec<String> = items.iter().map(|c| format!("this.{c}")).collect();
                    format!("[{}]", items.join(", "))
                }
                // Only the host view creates providers like this.
                Criacao::Expressao(_) | Criacao::Multi(_) => {
                    return Err(recusa(
                        Motivo::DiretivaPorSeletor,
                        "provider that is neither a directive nor an alias on the node",
                    ));
                }
            };
            // The reads from above may bring the import of the deferred
            // `unsafeCast` (a child's node): allocated here, where the text is
            // emitted.
            let value = resolver_tardios(self.imp, &value);
            if inst.preguicosa {
                // `late` without `final`, with the value in the initializer.
                self.campos_preguicosos
                    .push(format!("  late {ty} {} = {value};", inst.campo));
                self.posicoes_preguicosas.push(e.inicio);
            } else {
                self.campos_filho
                    .push(format!("  late final {ty} {};", inst.campo));
                self.linhas
                    .push(format!("    this.{} = {value};", inst.campo));
            }
        }
        Ok(())
    }

    /// `registerDirectives`: the instances of the directives, in order.
    /// (Only the component on the node, without a directive: nothing to
    /// register.)
    fn register_directives(
        &mut self,
        directives: &[(std::sync::Arc<crate::directives::Diretiva>, String)],
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
            self.linhas.push(format!(
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
        e: &crate::html::Elemento,
        instances: &[crate::directives::Instancia],
        directives: &[(std::sync::Arc<crate::directives::Diretiva>, String)],
        target: &str,
        properties: &[crate::html::Ligacao],
        events: &[crate::html::Ligacao],
        inputs: bool,
        outputs: bool,
    ) -> Result<(), Recusa> {
        use crate::directives::Criacao;
        // `detectHostChanges` of the directives with `@HostBinding`, with the
        // node's property bindings (`bindDirectiveHostProps`).
        for inst in instances.iter().filter(|_| inputs) {
            if let Criacao::Diretiva { diretiva, .. } = &inst.criacao
                && !diretiva.ligacoes_do_hospedeiro.is_empty()
                && directives.iter().any(|(_, c)| *c == inst.leitura)
            {
                let view = self.vista_do_hospedeiro.as_deref().unwrap_or("this");
                self.deteccao.push(format!(
                    "    this.{}.detectHostChanges({view}, {target});",
                    inst.campo
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
            let mut bound: Vec<(&crate::html::Ligacao, bool)> = e
                .atributos
                .iter()
                .filter(|a| d.entrada(&a.nome).is_some())
                .map(|a| (a, true))
                .collect();
            bound.extend(
                properties
                    .iter()
                    .filter(|l| d.entrada(&l.nome).is_some())
                    .map(|l| (l, false)),
            );
            let spec: Vec<(String, String, Option<bool>)> = d
                .entradas
                .iter()
                .map(|x| (x.nome.clone(), x.membro.clone(), x.booleana))
                .collect();
            // Only `AfterChanges`, `OnInit` and `DoCheck` get here (`OnDestroy`
            // is emitted in `destroyInternal`): the other hooks are refused by
            // the guard (`Diretiva::pendencia`).
            let hooks = crate::component::Ganchos {
                after_changes: d.ganchos.after_changes,
                on_init: d.ganchos.on_init,
                do_check: d.ganchos.do_check,
                ..Default::default()
            };
            self.inputs_of(
                &bound,
                &spec,
                field,
                None,
                &hooks,
                Motivo::DiretivaPorSeletor,
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
        d: &crate::directives::Diretiva,
        field: &str,
        events: &[crate::html::Ligacao],
    ) -> Result<(), Recusa> {
        {
            let mut seen: Vec<&str> = Vec::new();
            for l in events.iter().filter(|l| d.saida(&l.nome).is_some()) {
                if seen.contains(&l.nome.as_str()) {
                    return Err(recusa(
                        Motivo::Evento,
                        "two handlers of the same event in the template",
                    ));
                }
                seen.push(&l.nome);
                let member = d.saida(&l.nome).unwrap_or_default();
                self.set_output_type(&d.uri, &d.classe, member);
                match self.handler(&[&l.valor]) {
                    Ok(h) => {
                        let k = self.subscricoes;
                        self.subscricoes += 1;
                        self.ouvintes.push(format!(
                            "    final subscription_{k} = this.{field}.{member}.listen({h});"
                        ));
                    }
                    Err(r) => self.anotar(r)?,
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
/// goes in if the text uses it (`tardio`).
///
/// With `@HostBinding` (`hospedeiro`), the `detectHostChanges(firstCheck)` of
/// the component's view comes right before its `detectChanges`, after the
/// content hooks (case j32).
fn ciclo_de_vida(
    g: &crate::component::Ganchos,
    marca: bool,
    hospedeiro: bool,
    aninhadas: bool,
) -> String {
    let dbg = tardio(CHECK_BINDING);
    let mut s = String::new();
    if g.tem_deteccao() || marca || hospedeiro || aninhadas {
        s.push_str(
            "
  @override
  void detectChangesInternal() {
",
        );
        if marca {
            s.push_str("    bool changed = false;\n");
        }
        if g.usa_primeira_checagem() || hospedeiro {
            s.push_str(
                "    bool firstCheck = this.firstCheck;
",
            );
        }
        if marca {
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
        if aninhadas {
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
        if hospedeiro {
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
    if g.on_destroy || aninhadas {
        s.push_str("\n  @override\n  void destroyInternal() {\n");
        if aninhadas {
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
fn primitivo(tipo: &str) -> bool {
    matches!(tipo, "bool" | "num" | "double" | "int")
}

/// Does some element have a property binding (or a `#ref` read as a local, from
/// `refs`) and therefore become a field?
fn tem_elemento_ligado(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    refs: &std::collections::HashSet<String>,
) -> bool {
    nos.iter().any(|n| match n {
        // The subtree of a `*` belongs to the embedded view.
        No::Elemento(e) if e.estrela.is_some() => false,
        No::Elemento(e) => match filhos.get(&e.nome) {
            None => {
                liga_no_elemento(e, &diretivas_casadas(usadas, e))
                    || e.referencias.iter().any(|r| refs.contains(&r.nome))
                    || tem_elemento_ligado(&e.filhos, filhos, usadas, refs)
            }
            // A child's node becomes a field with the element's own binding
            // ([`Corpo::filho`]).
            Some(f) => {
                elemento_do_filho_ligado(e, f, &diretivas_casadas(usadas, e))
                    || tem_elemento_ligado(&e.filhos, filhos, usadas, refs)
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
fn elemento_do_filho_ligado(
    e: &crate::html::Elemento,
    filho: &Filho,
    extras: &[std::sync::Arc<crate::directives::Diretiva>],
) -> bool {
    e.propriedades
        .iter()
        .any(|l| filho.entrada(&l.nome).is_none() && !consome_entrada(extras, &l.nome))
        || e.atributos.iter().any(|a| {
            a.valor.contains("{{")
                && filho.entrada(&a.nome).is_none()
                && !consome_entrada(extras, &a.nome)
        })
        || extras.iter().any(|d| !d.ligacoes_do_hospedeiro.is_empty())
}

/// The directives that match an HTML element, with the metadata read from the
/// program, in `directives:` order (`_matchDirectives`). The element guard has
/// already refused those the emitter does not instantiate.
fn diretivas_casadas(
    usadas: &[Usada],
    e: &crate::html::Elemento,
) -> Vec<std::sync::Arc<crate::directives::Diretiva>> {
    if usadas.is_empty() {
        return Vec::new();
    }
    let desc = crate::selector::Elemento::do_template(e);
    usadas
        .iter()
        .filter(|u| u.filho.is_none() && crate::selector::casa_algum(&u.seletores, &desc))
        .filter_map(|u| u.diretiva.clone())
        .collect()
}

/// The tokens that the content of the child's node asks of its providers, eager
/// and in visiting order ([`pedidos_do_conteudo`]): the resolver transforms
/// them right after the node's eager ones, and the field numbering comes out in
/// the right order (case i76; the `PopupRef_0_9` of `paper_tooltip`).
fn pedidos_ao_no_do_filho(
    e: &crate::html::Elemento,
    casadas: &[std::sync::Arc<crate::directives::Diretiva>],
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
) -> Result<Vec<(crate::directives::Token, bool)>, Recusa> {
    let tokens: Vec<crate::directives::Token> = casadas
        .iter()
        .flat_map(|d| d.provedores.iter().map(|p| p.token.clone()))
        .collect();
    let mut pedidos = Vec::new();
    pedidos_do_conteudo(&e.filhos, filhos, usadas, &tokens, true, &mut pedidos)?;
    Ok(pedidos)
}

/// The fields of the node's providers and their creation in `build()`, in the
/// given order (`addDirectiveProviders`).
/// The tokens of `tokens` that the content `nos` asks of the node above
/// (`_getDependency`: the node below looks in itself first, except with
/// `@SkipSelf`, and walks up; with `@Self` it does not walk up), in the order
/// of the requests: in pre-order, at each node the child and then the
/// directives. A `*` on the way makes the request lazy (`_isViewRoot`), and a
/// node that provides the token serves those below. A node with requests for
/// more than one token, whose order depends on its resolution, is refused.
fn pedidos_do_conteudo(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    tokens: &[crate::directives::Token],
    ansioso: bool,
    saida: &mut Vec<(crate::directives::Token, bool)>,
) -> Result<(), Recusa> {
    use crate::directives::Token;
    for n in nos {
        let No::Elemento(e) = n else { continue };
        // The `*` takes the node to the embedded view: the request that comes
        // out of it crosses its root (`_isViewRoot`) and is lazy. A written
        // `<template>` stays here; only the content crosses.
        if let Some(estrela) = &e.estrela {
            let mut sem = e.clone();
            sem.estrela = None;
            if estrela.nome == MARCA_DE_MOLDE {
                // The directives of the `<template>` ask from here; the
                // content, from inside the embedded view, with what they
                // provide already served.
                let mut so_o_no = sem.clone();
                so_o_no.filhos.clear();
                pedidos_do_conteudo(
                    std::slice::from_ref(&No::Elemento(so_o_no)),
                    filhos,
                    usadas,
                    tokens,
                    ansioso,
                    saida,
                )?;
                let proprios: Vec<Token> = diretivas_casadas(usadas, &sem)
                    .iter()
                    .flat_map(|d| {
                        std::iter::once(d.token())
                            .chain(d.provedores.iter().map(|p| p.token.clone()))
                    })
                    .collect();
                let abaixo: Vec<Token> = tokens
                    .iter()
                    .filter(|t| !proprios.contains(t))
                    .cloned()
                    .collect();
                pedidos_do_conteudo(&e.filhos, filhos, usadas, &abaixo, false, saida)?;
            } else {
                pedidos_do_conteudo(
                    std::slice::from_ref(&No::Elemento(sem)),
                    filhos,
                    usadas,
                    tokens,
                    false,
                    saida,
                )?;
            }
            continue;
        }
        let filho = filhos.get(&e.nome);
        let casadas = diretivas_casadas(usadas, e);
        // What the node itself provides.
        let mut proprios: Vec<Token> = Vec::new();
        if let Some(m) = filho.and_then(|f| f.metadados.as_ref()) {
            proprios.push(m.token());
            proprios.extend(m.provedores.iter().map(|p| p.token.clone()));
        }
        for d in &casadas {
            proprios.push(d.token());
            proprios.extend(d.provedores.iter().map(|p| p.token.clone()));
        }
        let mut deps: Vec<(Token, bool, bool)> = Vec::new();
        if let Some(f) = filho {
            for p in &f.parametros {
                if let Injetado::Servico {
                    token,
                    proprio,
                    pular,
                    ..
                } = p
                {
                    deps.push((token.clone(), *proprio, *pular));
                }
            }
        }
        for d in &casadas {
            for x in &d.dependencias {
                deps.push((x.token.clone(), x.proprio, x.pular));
            }
        }
        let mut deste_no: Vec<Token> = Vec::new();
        for (t, proprio, pular) in deps {
            if proprio || !tokens.contains(&t) || (!pular && proprios.contains(&t)) {
                continue;
            }
            if !deste_no.contains(&t) {
                deste_no.push(t);
            }
        }
        let ja = |t: &Token, saida: &Vec<(Token, bool)>| saida.iter().any(|(x, _)| x == t);
        // Several tokens: the order is that of the creation of this node's
        // providers (each one asks for its dependencies in order,
        // `_getOrCreateLocalProvider`).
        if deste_no.iter().filter(|t| !ja(t, saida)).count() > 1 {
            let Some(ordem) = ordem_dos_pedidos_do_no(e, filho, &casadas, usadas) else {
                return Err(recusa(
                    Motivo::Ligacao,
                    "content node requesting several lazy providers of the child",
                ));
            };
            let mut ordenados: Vec<Token> =
                ordem.into_iter().filter(|t| deste_no.contains(t)).collect();
            ordenados.dedup();
            if ordenados.len() != deste_no.len() {
                return Err(recusa(
                    Motivo::Ligacao,
                    "content node requesting several lazy providers of the child",
                ));
            }
            deste_no = ordenados;
        }
        for t in deste_no {
            if !ja(&t, saida) {
                saida.push((t, ansioso));
            }
        }
        let abaixo: Vec<Token> = tokens
            .iter()
            .filter(|t| !proprios.contains(t))
            .cloned()
            .collect();
        pedidos_do_conteudo(&e.filhos, filhos, usadas, &abaixo, ansioso, saida)?;
    }
    Ok(())
}

/// The tokens that the providers of a content node ask for, in the order its
/// `ProviderElementContext` creates them (each eager directive after its
/// dependencies, and its dependencies in order). `None` when the node does not
/// resolve or has a provider that is not a directive.
fn ordem_dos_pedidos_do_no(
    e: &crate::html::Elemento,
    filho: Option<&Filho>,
    casadas: &[std::sync::Arc<crate::directives::Diretiva>],
    usadas: &[Usada],
) -> Option<Vec<crate::directives::Token>> {
    use crate::directives::Criacao;
    let _ = e;
    let r = match filho {
        Some(f) => {
            let meta = f.metadados.as_ref()?;
            let (indice, cas) = casadas_do_no_do_filho(meta, casadas, usadas);
            let container = f
                .parametros
                .iter()
                .any(|p| matches!(p, Injetado::Container))
                || casadas.iter().any(|d| crate::directives::pede_container(d));
            crate::directives::resolver_no_do_filho(&cas, indice, 0, None, container, &[]).ok()?
        }
        None => crate::directives::resolver(casadas, 0, None).ok()?,
    };
    // The component enters the resolution only with the dependencies of the
    // node itself ([`casadas_do_no_do_filho`]); here all of them count, in
    // constructor order.
    let componente = filho.and_then(|f| f.metadados.clone());
    let mut saida = Vec::new();
    for i in &r.instancias {
        match &i.criacao {
            Criacao::Diretiva { diretiva, .. } => {
                let deps = match &componente {
                    Some(m) if m.uri == diretiva.uri && m.classe == diretiva.classe => {
                        &m.dependencias
                    }
                    _ => &diretiva.dependencias,
                };
                saida.extend(deps.iter().map(|d| d.token.clone()));
            }
            // The lazy one only asks in `afterElement`, after the content.
            _ if i.preguicosa => {}
            _ => return None,
        }
    }
    Some(saida)
}

/// Does some `@Input` of the directives have this name?
fn consome_entrada(casadas: &[std::sync::Arc<crate::directives::Diretiva>], nome: &str) -> bool {
    casadas.iter().any(|d| d.entrada(nome).is_some())
}

/// Does some `@Output` of the directives have this name?
fn consome_saida(casadas: &[std::sync::Arc<crate::directives::Diretiva>], nome: &str) -> bool {
    casadas.iter().any(|d| d.saida(nome).is_some())
}

/// The node has a binding of its own — `[x]` that no directive receives, or an
/// attribute with `{{ }}` —, and becomes a field of the view.
fn liga_no_elemento(
    e: &crate::html::Elemento,
    casadas: &[std::sync::Arc<crate::directives::Diretiva>],
) -> bool {
    e.propriedades
        .iter()
        .any(|p| !consome_entrada(casadas, &p.nome))
        || e
            .atributos
            .iter()
            .any(|a| a.valor.contains("{{") && !consome_entrada(casadas, &a.nome))
        // `detectHostChanges(this, el)` reads the node during detection.
        || casadas.iter().any(|d| !d.ligacoes_do_hospedeiro.is_empty())
}

/// An attribute written without a value (`<input required>`): its range is just
/// the name.
fn sem_valor(a: &crate::html::Ligacao) -> bool {
    a.sem_valor
}

/// The `injectorGetInternal` of a view (`writeInjectorGetMethod`,
/// `ProviderForest.build`): the nodes with an injectable provider, each with
/// the index range it serves; empty if there are none.
/// A node with injectable providers: (first index, last index of the subtree,
/// [(tokens, field)]).
type NoInjetor = (u32, u32, Vec<(Vec<crate::directives::Token>, String)>);

fn metodo_injetor(injetores: &[NoInjetor], asset: &str) -> String {
    use crate::directives::Token;
    if injetores.is_empty() {
        return String::new();
    }
    // As URIs dos tokens viram o caminho do import visto deste arquivo.
    let convertido = |t: &Token| match t {
        Token::Classe { uri, classe } => Token::Classe {
            uri: import_de(uri, asset),
            classe: classe.clone(),
        },
        Token::Multi { nome, tipo } => Token::Multi {
            nome: nome.clone(),
            tipo: crate::directives::TipoDeToken {
                uri: import_de(&tipo.uri, asset),
                ..tipo.clone()
            },
        },
        Token::Opaco { nome, tipo } => Token::Opaco {
            nome: nome.clone(),
            tipo: crate::directives::TipoDeToken {
                uri: import_de(&tipo.uri, asset),
                ..tipo.clone()
            },
        },
        outro => outro.clone(),
    };
    let convertidos: Vec<NoInjetor> = injetores
        .iter()
        .map(|(a, b, ps)| {
            let ps = ps
                .iter()
                .map(|(ts, c)| (ts.iter().map(convertido).collect(), c.clone()))
                .collect();
            (*a, *b, ps)
        })
        .collect();
    let injetores = &convertidos[..];
    // The forest: each node is a child of the closest one before it that
    // contains it.
    let mut pais: Vec<Option<usize>> = Vec::new();
    for (i, (ini, fim, _)) in injetores.iter().enumerate() {
        let mut p = if i == 0 { None } else { Some(i - 1) };
        while let Some(j) = p {
            if injetores[j].0 <= *ini && *fim <= injetores[j].1 {
                break;
            }
            p = pais[j];
        }
        pais.push(p);
    }
    fn tokens(ts: &[Token]) -> String {
        let partes: Vec<String> = ts
            .iter()
            .map(|t| format!("identical(token, {})", expr_do_token(t)))
            .collect();
        // `||` associates to the left: `((a || b) || c)`.
        let mut it = partes.into_iter();
        let primeiro = it.next().unwrap_or_default();
        it.fold(primeiro, |acc, t| format!("({acc} || {t})"))
    }
    fn indice(ini: u32, fim: u32, baixo: i64, alto: i64) -> String {
        if ini == fim {
            format!("({ini} == nodeIndex)")
        } else if i64::from(ini) == baixo {
            format!("(nodeIndex <= {fim})")
        } else if i64::from(fim) == alto {
            format!("({ini} <= nodeIndex)")
        } else {
            format!("(({ini} <= nodeIndex) && (nodeIndex <= {fim}))")
        }
    }
    fn nivel(
        injetores: &[NoInjetor],
        pais: &[Option<usize>],
        pai: Option<usize>,
        baixo: i64,
        alto: i64,
        saida: &mut Vec<String>,
    ) {
        for (i, (ini, fim, provedores)) in injetores.iter().enumerate() {
            if pais[i] != pai {
                continue;
            }
            let cond = indice(*ini, *fim, baixo, alto);
            let tem_filhos = pais.contains(&Some(i));
            if !tem_filhos && provedores.len() == 1 {
                let (ts, campo) = &provedores[0];
                saida.push(format!(
                    "if (({} && {cond})) {{\n  return this.{campo};\n}}",
                    tokens(ts)
                ));
            } else {
                let mut dentro = Vec::new();
                nivel(
                    injetores,
                    pais,
                    Some(i),
                    i64::from(*ini),
                    i64::from(*fim),
                    &mut dentro,
                );
                for (ts, campo) in provedores {
                    dentro.push(format!(
                        "if ({}) {{\n  return this.{campo};\n}}",
                        tokens(ts)
                    ));
                }
                saida.push(format!(
                    "if ({cond}) {{\n{}\n}}",
                    indentar(&dentro.join("\n"), 2)
                ));
            }
        }
    }
    let mut corpo = Vec::new();
    nivel(injetores, &pais, None, 0, -1, &mut corpo);
    format!(
        "\n  @override\n  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {{\n{}\n    return notFoundResult;\n  }}\n",
        indentar(&corpo.join("\n"), 4)
    )
}

/// The expression of a token (`createDiTokenExpression`), with the URI already
/// seen from this file and the deferred import: the class, or the `const
/// MultiToken<T>('x')` / `const OpaqueToken<T>('x')`. A `T` from `dart:core`
/// comes out without a prefix; one from another library gets its own import
/// (see [`Importacoes::q_chave`]).
fn expr_do_token(t: &crate::directives::Token) -> String {
    use crate::directives::Token;
    let (classe_do_token, nome, tipo) = match t {
        Token::Classe { uri, classe } => return format!("{}{classe}", tardio_q(uri)),
        Token::Multi { nome, tipo } => ("MultiToken", nome, tipo),
        Token::Opaco { nome, tipo } => ("OpaqueToken", nome, tipo),
        Token::Elemento | Token::Detector => return String::new(),
    };
    let arg = texto_do_tipo_de_token(tipo);
    format!(
        "const {}{classe_do_token}<{arg}>({})",
        tardio_q(crate::directives::DI_TOKENS),
        literal(nome)
    )
}

/// The `T` of a token as `fromDartType` writes it: one from `dart:core` without
/// a prefix (but with the import allocated), one from another library with its
/// own import by URI (`q_chave`), and the concrete arguments recursively
/// (`List<import29.RelativePosition>`, by the `package:` URI of the type).
fn texto_do_tipo_de_token(tipo: &crate::directives::TipoDeToken) -> String {
    if tipo.e_dinamico() {
        return "dynamic".into();
    }
    let args = if !tipo.args.is_empty() {
        let dentro: Vec<String> = tipo.args.iter().map(texto_do_tipo_de_token).collect();
        format!("<{}>", dentro.join(", "))
    } else if tipo.genericos > 0 {
        format!("<{}>", vec!["dynamic"; tipo.genericos].join(", "))
    } else {
        String::new()
    };
    let (uri, classe) = (&tipo.uri, &tipo.classe);
    if uri == "dart:core" {
        format!("{}{classe}{args}", tardio_q("dart:core"))
    } else {
        format!("\u{1}k:{uri}#token|{uri}\u{2}{classe}{args}")
    }
}

/// What generates a field in the view class, in order of appearance.
enum CampoDaVisao<'a> {
    /// The child, if its node has a `ViewContainer`, and the fields of the
    /// node's providers created before it ([`uri_do_campo`]).
    Filho(&'a Filho, bool, Vec<String>),
    /// The `ViewContainer` of an ordinary element (a directive that injects
    /// `ViewContainerRef`), before the directives.
    Container,
    /// Diretiva estrutural, com a URI da classe dela.
    Estrutural(&'static str),
    /// A written `<template>`: the `ViewContainer`, with `#ref` the field of
    /// the `TemplateRef`, and the fields of its directives (the text with the
    /// deferred import, [`uri_do_campo`]), in order.
    Molde(bool, Vec<String>),
    /// Directive providers on a node: the text with the deferred imports of
    /// each one's field ([`uri_do_campo`]), in order.
    Diretivas(Vec<String>),
    /// Lazy providers (a field with an initializer): they come before the other
    /// fields in the class, and so do their imports.
    Preguicosos(Vec<String>),
}

/// The fields the template will generate, in document order. The order of the
/// imports follows this list, and it is what makes the numbering match the
/// official one.
fn campos_em_ordem<'a>(
    nos: &[No],
    filhos: &'a std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    asset: &str,
) -> Vec<CampoDaVisao<'a>> {
    let mut saida = Vec::new();
    let separar = |instancias: &[crate::directives::Instancia], saida: &mut Vec<CampoDaVisao>| {
        let uris = instancias
            .iter()
            .filter(|i| !i.preguicosa)
            .map(|i| uri_do_campo(i, asset))
            .collect();
        let preguicosos = instancias
            .iter()
            .filter(|i| i.preguicosa)
            .map(|i| uri_do_campo(i, asset))
            .collect();
        saida.push(CampoDaVisao::Diretivas(uris));
        saida.push(CampoDaVisao::Preguicosos(preguicosos));
    };
    for no in nos {
        let No::Elemento(e) = no else { continue };
        if let Some(estrela) = &e.estrela {
            // The content goes to the embedded view; its fields belong there.
            if estrela.nome == MARCA_DE_MOLDE {
                let mut sem = e.clone();
                sem.estrela = None;
                let casadas = diretivas_casadas(usadas, &sem);
                let campos = if casadas.is_empty() {
                    Vec::new()
                } else {
                    crate::directives::resolver_de_molde(&casadas, 0, None, false, &|_| {
                        String::new()
                    })
                    .map(|(r, _)| {
                        r.instancias
                            .iter()
                            .map(|i| uri_do_campo(i, asset))
                            .collect()
                    })
                    .unwrap_or_default()
                };
                saida.push(CampoDaVisao::Molde(!e.referencias.is_empty(), campos));
            } else if let Some(d) = Estrutural::conhecida(&estrela.nome) {
                saida.push(CampoDaVisao::Estrutural(d.uri));
            }
            continue;
        }
        if let Some(f) = filhos.get(&e.nome) {
            // The other providers of the child's node, after the instance.
            let extras = diretivas_casadas(usadas, e);
            let container = f
                .parametros
                .iter()
                .any(|p| matches!(p, Injetado::Container))
                || extras.iter().any(|d| crate::directives::pede_container(d));
            let resolvido = f
                .metadados
                .as_ref()
                .filter(|meta| !extras.is_empty() || !meta.provedores.is_empty())
                .and_then(|meta| {
                    let (indice, casadas) = casadas_do_no_do_filho(meta, &extras, usadas);
                    let pedidos = pedidos_ao_no_do_filho(e, &casadas, filhos, usadas).ok()?;
                    let r = crate::directives::resolver_no_do_filho(
                        &casadas, indice, 0, None, container, &pedidos,
                    )
                    .ok()?;
                    let token = casadas[indice].token();
                    let pos = r.instancias.iter().position(|i| i.token == token)?;
                    Some((r, pos))
                });
            match resolvido {
                Some((r, pos)) => {
                    let antes = r.instancias[..pos]
                        .iter()
                        .map(|i| uri_do_campo(i, asset))
                        .collect();
                    saida.push(CampoDaVisao::Filho(f, container, antes));
                    // The lazy ones the content asks for already come eager and
                    // in the right position (case i76).
                    separar(&r.instancias[pos + 1..], &mut saida);
                }
                None => saida.push(CampoDaVisao::Filho(f, container, Vec::new())),
            }
        } else {
            let casadas = diretivas_casadas(usadas, e);
            if let (false, Ok(r)) = (
                casadas.is_empty(),
                crate::directives::resolver(&casadas, 0, None),
            ) {
                if r.container {
                    saida.push(CampoDaVisao::Container);
                }
                separar(&r.instancias, &mut saida);
            }
        }
        saida.extend(campos_em_ordem(&e.filhos, filhos, usadas, asset));
    }
    saida
}

/// The directives of a child component's node, for the resolver: the child with
/// its `providers:` and only the dependencies the node itself provides (the
/// rest is resolved separately, in [`Corpo::construcao_do_filho`]) — they
/// decide the order of the fields, because the official
/// `_getOrCreateLocalProvider` creates the dependencies of an eager provider
/// before it (case j61) —, and the node's directives. The order is that of
/// `directives:`, with the child in its place (`_matchDirectives`); it comes
/// back with its index.
fn casadas_do_no_do_filho(
    meta: &crate::directives::Diretiva,
    extras: &[std::sync::Arc<crate::directives::Diretiva>],
    usadas: &[Usada],
) -> (usize, Vec<std::sync::Arc<crate::directives::Diretiva>>) {
    let providos: Vec<crate::directives::Token> = std::iter::once(meta)
        .chain(extras.iter().map(|d| &**d))
        .flat_map(|d| d.provedores.iter().map(|p| p.token.clone()))
        .chain(extras.iter().map(|d| d.token()))
        .collect();
    let mut so_provedores = meta.clone();
    so_provedores
        .dependencias
        .retain(|d| !d.pular && d.atributo.is_none() && providos.contains(&d.token));
    let posicao = |classe: &str, uri: &str| {
        usadas
            .iter()
            .position(|u| u.classe == classe && u.uri == uri)
    };
    let do_filho = posicao(&meta.classe, &meta.uri);
    let indice = extras
        .iter()
        .filter(|d| posicao(&d.classe, &d.uri) < do_filho)
        .count();
    let mut casadas: Vec<_> = extras.to_vec();
    casadas.insert(indice, std::sync::Arc::new(so_provedores));
    (indice, casadas)
}

/// The field of a node provider, with the deferred imports in the order the
/// official compiler writes them: the type and, for a lazy one from
/// `providers:`, the value.
fn uri_do_campo(i: &crate::directives::Instancia, asset: &str) -> String {
    use crate::directives::{Criacao, Token};
    let uri = match (&i.criacao, &i.token) {
        // The lazy one has the value in the field; the eager one, only the type
        // (the value comes out in `build()`).
        (Criacao::Expressao(_) | Criacao::Multi(_), _) => {
            let texto = if i.preguicosa {
                texto_de_provedor_preguicoso(i, asset, Some("this"))
            } else {
                Err(recusa(Motivo::Providers, ""))
            };
            return texto
                .or_else(|_| tipo_do_provedor(i, asset))
                .unwrap_or_default();
        }
        (Criacao::Diretiva { diretiva, .. }, _)
            if !diretiva.ligacoes_do_hospedeiro.is_empty() && !diretiva.e_componente =>
        {
            let tpl = import_de(&diretiva.uri.replace(".dart", ".template.dart"), asset);
            // An `XNgCd` declared in this same file has no import (j45).
            if e_o_proprio_template(asset, &tpl) {
                return String::new();
            }
            return tardio(&tpl);
        }
        (Criacao::Diretiva { diretiva, .. }, _) => diretiva.uri.clone(),
        (_, Token::Multi { tipo, .. }) => tipo.uri.clone(),
        _ => "dart:core".to_string(),
    };
    tardio(&import_de(&uri, asset))
}

/// What is needed to emit an embedded view, kept during the scan and used
/// later.
struct EspecEmbutida {
    /// Number of the view (`_ViewX3`), assigned during the scan.
    indice: u32,
    /// The start of the star that creates it (key of
    /// [`Contexto::sujas_de_conteudo`]).
    estrela: Option<usize>,
    /// O namespace herdado ([`Corpo::ns_atual`]).
    ns: Option<String>,
    /// How many `parentView`s up to the component's view.
    profundidade: u32,
    /// Ver [`Corpo::nivel_do_topo`] (em `EspecEmbutida`).
    nivel_do_topo: u32,
    classe: String,
    fabrica: String,
    nos: Vec<No>,
    locais: std::collections::HashMap<String, crate::expr::Local>,
    micro: crate::micro::Micro,
    /// The locals declared in ancestor views, with where they come from.
    ancestrais: std::collections::HashMap<String, Origem>,
    /// The `#ref`s read as locals declared in ancestor views: (name, class of
    /// the view that declares it, how many `parentView`s up to it).
    refs_ancestrais: Vec<(String, String, u32)>,
    /// The view query results in this view: (`#ref`, dirty field, how many
    /// `parentView`s up to the component's view).
    refs_consultados: Vec<(String, String, u32)>,
    /// The same, in the order of the lines of `dirtyParentQueriesInternal`
    /// (that of the first result of each one, [`primeiro_resultado`]).
    sujas: Vec<(String, String, u32)>,
    /// As consultas com resultado mais abaixo ([`Corpo::consultas_em_transito`]).
    consultas_em_transito: Vec<(String, String, u32)>,
    /// The providers above the anchor, seen from the new view.
    acima: Vec<(crate::directives::Token, String, Option<(String, u32)>)>,
    /// The fields of `acima` that are lazy providers of the owning element
    /// (field, class of its view): a node below that asks for them would
    /// transform them before the owner's `afterElement` (gap L1 of section 05).
    preguicosos_acima: std::collections::HashSet<(String, String)>,
    /// Those of them whose owner modeled the content's requests (a child's
    /// node): from another view, read through the lazy field.
    preguicosos_com_pedidos: std::collections::HashSet<(String, String)>,
    componentes_acima: u32,
    incertos_acima: u32,
    /// The index of the embedded view's first `<ng-content>` in the template.
    proxima_projecao: u32,
}

/// Where a local of an ancestor view comes from: the key in `locals`, the class
/// of the view that declares it and how many `parentView`s separate the two
/// (`getPropertyInView`, in `view_compiler_utils.dart`).
#[derive(Debug, Clone)]
struct Origem {
    chave: String,
    classe: String,
    niveis: u32,
}

/// The names of the component's `exports:`: the qualifier (deferred import) and
/// what each one designates.
pub(crate) type Exportados =
    std::collections::HashMap<String, (String, crate::resolucao::Exportado)>;

/// What every view of the file shares: the component, the directives it uses
/// and where the file lives.
/// A `Typed` of `directiveTypes:` with the class resolved and the arguments
/// already written (`<T>`, `<import3.X>`).
#[derive(Debug, Clone)]
struct TipoDeDiretivaResolvido {
    uri: String,
    classe: String,
    argumentos: String,
    em: Option<String>,
}

struct Contexto<'a> {
    membros: &'a std::collections::HashMap<String, crate::component::Membro>,
    /// The resolved `exports:` names ([`Exportados`]).
    exportados: &'a Exportados,
    /// The component's type parameters (`T`, `U`), free in the views.
    nomes_genericos: Vec<String>,
    /// The tokens of the component's `viewProviders:`
    /// (`ProviderViewContext.viewProviders`): an `@Host()` of its view that
    /// asks for one of them goes to the injector instead of being `null`.
    tokens_de_visao: Vec<crate::directives::Token>,
    metodos: &'a std::collections::HashMap<String, String>,
    aridades: &'a std::collections::HashMap<String, usize>,
    filhos: &'a std::collections::HashMap<String, Filho>,
    usadas: &'a [Usada],
    asset: String,
    tipos: Option<(&'a dyn Resolucao, &'a Path)>,
    classe_qualificada: String,
    com_estilo: bool,
    url_do_template: Option<String>,
    classe_da_visao: String,
    tipo_do_contexto: String,
    /// The type arguments of a generic component (`<T, U>`) and their
    /// declaration (`<T extends num, U>`); empty for the rest.
    genericos: String,
    genericos_decl: String,
    preservar_espacos: bool,
    pipes: &'a PipesDoTemplate,
    /// The `#ref` names that may become locals ([`referencias_candidatas`]).
    refs_candidatos: std::collections::HashSet<String>,
    /// (key, token) of the queries with `read:` of a node provider
    /// ([`token_de_leitura`]).
    leituras: Vec<(String, crate::directives::Token)>,
    /// The `#ref`s that only queries with a provider `read:` look for: the node
    /// does not need to become a field.
    refs_so_por_provedor: std::collections::HashSet<String>,
    /// The `<template #x>`s that a query reads as `ViewContainerRef`: the
    /// node's `ViewContainer` stops being private ([`le_container`]).
    moldes_com_container: std::collections::HashSet<String>,
    /// `@changeDetectionLink` no componente.
    link_de_deteccao: bool,
    /// `directiveTypes:` resolvido ([`TipoDeDiretivaResolvido`]).
    tipos_de_diretiva: Vec<TipoDeDiretivaResolvido>,
    /// The `#ref` names that are repeated or shadowed by `let`.
    refs_ambiguos: std::collections::HashSet<String>,
    /// The node of each `#ref` seen, of all views already walked: a nested view
    /// is emitted after the one that contains it, and reads the field from it.
    refs_resolvidos: std::cell::RefCell<std::collections::HashMap<String, String>>,
    /// The chain of each dynamic view query (by the dirty field): the
    /// `_appEl_n` anchor and the class of the embedded view at each level, from
    /// the component's down. The lower levels are only known when the nested
    /// views are emitted.
    ancoras_de_consulta: std::cell::RefCell<Ancoras>,
    /// The content queries with a result in a `*`, assembled at the end
    /// ([`resolver_conteudo_dinamico`]).
    conteudo_dinamico: std::cell::RefCell<Vec<ConteudoDinamico>>,
    /// Per embedded view (the start of its star), the content queries it marks
    /// in `dirtyParentQueriesInternal`.
    sujas_de_conteudo: std::cell::RefCell<SujasDeConteudo>,
}

impl<'a> Contexto<'a> {
    /// An empty body for a view of this file.
    fn corpo<'b>(
        &'b self,
        imp: &'b mut Importacoes,
        nomes: &'b mut dartforge_intern::Interner,
        coleta: Option<Vec<Recusa>>,
        embutida: bool,
    ) -> Corpo<'b>
    where
        'a: 'b,
    {
        Corpo {
            linhas: Vec::new(),
            ouvintes: Vec::new(),
            refs_livres: Default::default(),
            refs_locais: Default::default(),
            refs_ambiguos: self.refs_ambiguos.clone(),
            leituras: &self.leituras,
            refs_so_por_provedor: &self.refs_so_por_provedor,
            moldes_com_container: &self.moldes_com_container,
            tipos_de_diretiva: &self.tipos_de_diretiva,
            refs_ancestrais: Vec::new(),
            consultas_dinamicas: Vec::new(),
            consultas_em_transito: Vec::new(),
            ancoras_de_consulta: &self.ancoras_de_consulta,
            conteudo_dinamico: &self.conteudo_dinamico,
            sujas_de_conteudo: &self.sujas_de_conteudo,
            atualizacoes_de_conteudo: Vec::new(),
            atualizacoes_de_visao: Vec::new(),
            consultas_por_token: Default::default(),
            sujos_de_conteudo: Vec::new(),
            consultas_por_no: Default::default(),
            refs_consultados: Vec::new(),
            refs: Default::default(),
            refs_em_ordem: Vec::new(),
            campos: Vec::new(),
            intl: None,
            mensagens: Vec::new(),
            tag_atual: String::new(),
            ns_atual: None,
            instancia_da_visao: None,
            vista_do_hospedeiro: None,
            campos_preguicosos: Vec::new(),
            posicoes_preguicosas: Vec::new(),
            sujos_de_visao: Vec::new(),
            campos_filho: Vec::new(),
            vistas_filhas: Vec::new(),
            vistas_ligadas: Vec::new(),
            campos_expr: Vec::new(),
            campos_el: Vec::new(),
            refs_dos_campos_el: Default::default(),
            refs_so_em_eventos: Default::default(),
            campos_el_de_eventos: Vec::new(),
            campos_el_por_evento: Vec::new(),
            campos_el_consultados: Vec::new(),
            campos_el_de_embutidas: Vec::new(),
            refs_de_embutidas: Default::default(),
            posicao_de_embutida: Default::default(),
            proxima_ligacao: 0,
            deteccao: Vec::new(),
            nomes,
            usa_primeira_checagem: false,
            entradas: Vec::new(),
            tb: None,
            tb_usado: false,
            url_do_template: self.url_do_template.clone(),
            membros: self.membros,
            exportados: self.exportados,
            nomes_genericos: &self.nomes_genericos,
            tokens_de_visao: &self.tokens_de_visao,
            metodos: self.metodos,
            aridades: self.aridades,
            metodos_evento: Vec::new(),
            tipo_do_evento: None,
            tipo_do_evento_falha: None,
            metodos_i18n: Vec::new(),
            decl_locais: Default::default(),
            locais_raiz: Vec::new(),
            classe_desta: String::new(),
            locais_proprios: Vec::new(),
            ancestrais: Default::default(),
            acima: Vec::new(),
            preguicosos_acima: Default::default(),
            preguicosos_com_pedidos: Default::default(),
            componentes_acima: 0,
            incertos_acima: 0,
            filhos: self.filhos,
            usadas: self.usadas,
            coleta,
            asset: self.asset.clone(),
            tipos: self.tipos,
            classe_qualificada: self.classe_qualificada.clone(),
            com_estilo: self.com_estilo,
            embutidas: Vec::new(),
            classe_da_visao: self.classe_da_visao.clone(),
            genericos: self.genericos.clone(),
            preservar_espacos: self.preservar_espacos,
            ancoras: Vec::new(),
            embutida,
            locais: Default::default(),
            proxima_embutida: 1,
            proximo: 0,
            tem_doc: false,
            usa_ctx_no_build: false,
            proxima_projecao: 0,
            imp,
            pipes: self.pipes,
            vista: 0,
            profundidade: 0,
            cursor_pipe: 0,
            usa_changed: false,
            apos_conteudo: Vec::new(),
            apos_visao: Vec::new(),
            destruir: Vec::new(),
            subscricoes: 0,
            filhos_acima: Vec::new(),
            detectores: Default::default(),
            detectores_em_ordem: Vec::new(),
            raizes: Vec::new(),
            pai_projetado: None,
            pilha: Vec::new(),
            registros: Vec::new(),
            nivel_do_topo: 0,
            injetores: Vec::new(),
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
/// In collection mode (`coleta` with `Some`), each refusal is recorded and the
/// emission goes on.
fn emitir_embutida(
    espec: EspecEmbutida,
    ctx: &Contexto,
    imp: &mut Importacoes,
    nomes: &mut dartforge_intern::Interner,
    coleta: &mut Option<Vec<Recusa>>,
) -> Result<String, Recusa> {
    let ev = imp.alias(EMBEDDED_VIEW);
    let classe = espec.classe.clone();
    let fabrica = espec.fabrica.clone();
    // As in the component's view (case j91): the `text_binding` import is
    // allocated before the body, at the position of the first `TextBinding`
    // field; when no interpolation is mutable (all written in `build()`, like
    // that of an `exports:` name), there is no field and the emission is redone
    // without it, from the same import table (case j112).
    let (imp_antes, coleta_antes) = (imp.clone(), coleta.clone());
    let mut com_tb = true;
    let (mut texto, aninhadas) = loop {
        let mut dentro = ctx.corpo(imp, nomes, coleta.take(), true);
        dentro.locais = espec.locais.clone();
        dentro.vista = espec.indice;
        dentro.profundidade = espec.profundidade;
        dentro.nivel_do_topo = espec.nivel_do_topo;
        // The numbering continues where the parent stopped.
        dentro.proxima_embutida = espec.indice + 1;
        let r = corpo_da_embutida(&mut dentro, &espec, ctx, &ev, &classe, &fabrica, com_tb);
        let sobrando = dentro.tb.is_some() && !dentro.tb_usado && !dentro.coletando();
        *coleta = dentro.coleta.take();
        drop(dentro);
        if com_tb && sobrando && r.is_ok() {
            *imp = imp_antes.clone();
            *coleta = coleta_antes.clone();
            com_tb = false;
            continue;
        }
        break r?;
    };
    for a in aninhadas {
        texto.push_str(&emitir_embutida(a, ctx, imp, nomes, coleta)?);
    }
    Ok(texto)
}

/// The text of an embedded view and the specs of the views nested in it.
fn corpo_da_embutida(
    dentro: &mut Corpo,
    espec: &EspecEmbutida,
    ctx: &Contexto,
    ev: &str,
    classe: &str,
    fabrica: &str,
    com_tb: bool,
) -> Result<(String, Vec<EspecEmbutida>), Recusa> {
    // The text binding field is declared before the constructor, so its import
    // goes in here — and that of `package:intl`, before or after it according
    // to document order, when the view has `@i18n` (case j01).
    alocar_imports_preguicosos(dentro.imp, &espec.nos, ctx.filhos, ctx.usadas, &ctx.asset);
    let ordem_i18n = i18n_antes_da_interpolacao(&espec.nos);
    if ordem_i18n == Some(true) {
        dentro.intl = Some(dentro.imp.alias(INTL));
    }
    if com_tb && tem_interpolacao(&espec.nos) {
        dentro.tb = Some(dentro.imp.alias(TEXT_BINDING));
    }
    if ordem_i18n == Some(false) {
        dentro.intl = Some(dentro.imp.alias(INTL));
    }
    // With a child, directive or `*` in the view, the order of the fields among
    // them has no case yet; without a message in a field, the HTML one uses
    // `intl` only in the method.
    if contem_anotacao(&espec.nos) {
        if ordem_i18n.is_some()
            && !campos_em_ordem(&espec.nos, ctx.filhos, ctx.usadas, &ctx.asset).is_empty()
        {
            dentro.anotar(recusa(
                Motivo::I18n,
                "@i18n with a child, directive or `*` in the view",
            ))?;
        }
        dentro.intl.get_or_insert_with(String::new);
    }
    let lets: Vec<String> = espec.micro.locais.iter().map(|(n, _)| n.clone()).collect();
    let refs_locais = referencias_locais(&espec.nos, ctx.filhos, &ctx.refs_candidatos, &lets);
    let mut promovidos = refs_locais.clone();
    promovidos.extend(espec.refs_consultados.iter().map(|(n, _, _)| n.clone()));
    dentro.refs_consultados = espec.refs_consultados.clone();
    dentro.consultas_em_transito = espec.consultas_em_transito.clone();
    if let Err(r) = alocar_imports_dos_campos(
        dentro.imp,
        &espec.nos,
        ctx.filhos,
        ctx.usadas,
        &ctx.asset,
        &ctx.pipes.imports_dos_campos(espec.indice),
        &nos_promovidos(&espec.nos, &promovidos),
        &ctx.tipos_de_diretiva,
    ) {
        dentro.anotar(r)?;
    }
    // The constructor comes after the fields and before `build()`, and it is
    // what names the `RenderView`.
    let rv = dentro.imp.alias(RENDER_VIEW);
    let util = dentro.imp.alias(UTILITIES);
    // The declaration of each local of this view, ready for whoever asks for it
    // (the detection or a `_handleEvent_N`). A local of an *ancestor* view is
    // read through the `parentView` chain
    // (`unsafeCast<_ViewX2>((this.parentView!)) .locals['$implicit']`,
    // `getLocal` of `ViewNameResolver`, case j125).
    dentro.classe_desta = espec.classe.clone();
    dentro.locais_proprios = espec.micro.locais.clone();
    dentro.ancestrais = espec.ancestrais.clone();
    dentro.acima = espec.acima.clone();
    dentro.preguicosos_acima = espec.preguicosos_acima.clone();
    dentro.ns_atual = espec.ns.clone();
    dentro.preguicosos_com_pedidos = espec.preguicosos_com_pedidos.clone();
    dentro.componentes_acima = espec.componentes_acima;
    dentro.incertos_acima = espec.incertos_acima;
    dentro.proxima_projecao = espec.proxima_projecao;
    // The `#ref`s of the ancestor views: the field of the node in the view that
    // declares it (`getPropertyInView`, without casting the value: the
    // reference has no type).
    for (nome, classe, niveis) in &espec.refs_ancestrais {
        let mut cadeia = "(this.parentView!)".to_string();
        for _ in 1..*niveis {
            cadeia = format!("({cadeia}.parentView!)");
        }
        dentro.decl_locais.insert(
            nome.clone(),
            Ok(format!(
                "final local_{nome} = {util}.unsafeCast<{classe}>({cadeia}){MARCA_DE_REF}.{}{FIM_DE_REF};",
                chave_de_ref(nome, classe)
            )),
        );
    }
    dentro.refs_ancestrais = espec.refs_ancestrais.clone();
    // A `#ref` of this view only matters for the queries that reach it (the
    // dynamic ones: `refs_consultados` and those in transit); the static one,
    // with the first result outside a `*`, never reaches it
    // (`compile_query.dart: 128-150`, case j109).
    let consultados_aqui: std::collections::HashSet<String> = espec
        .refs_consultados
        .iter()
        .chain(&espec.consultas_em_transito)
        .map(|(r, _, _)| r.clone())
        .collect();
    dentro.refs_livres = referencias_livres_da_visao(&espec.nos, ctx.filhos, &consultados_aqui);
    dentro.refs_de_embutidas = refs_locais
        .iter()
        .filter(|n| citado_em_embutidas(&espec.nos, n, ctx.filhos))
        .cloned()
        .collect();
    dentro.refs_so_em_eventos = refs_locais
        .iter()
        .filter(|n| !lido_na_deteccao(&espec.nos, n))
        .cloned()
        .collect();
    dentro.declarar_refs(refs_locais);
    for (nome, origem) in &espec.ancestrais {
        let Some(l) = espec.locais.get(nome.as_str()) else {
            continue;
        };
        let decl = declaracao_de_local(l, origem, ctx, &util);
        dentro.decl_locais.insert(nome.clone(), decl);
    }
    for (nome, chave) in &espec.micro.locais {
        let Some(l) = espec.locais.get(nome.as_str()) else {
            continue;
        };
        let origem = Origem {
            chave: chave.clone(),
            classe: espec.classe.clone(),
            niveis: 0,
        };
        let decl = declaracao_de_local(l, &origem, ctx, &util);
        dentro.decl_locais.insert(nome.clone(), decl);
    }
    let anotadas = dentro.coleta.as_ref().map_or(0, Vec::len);
    dentro.nos(&espec.nos, "")?;
    dentro.conferir_pipes()?;
    exportar_refs(
        &ctx.refs_resolvidos,
        &dentro.refs,
        &dentro.detectores,
        &dentro.refs_em_ordem,
        &dentro.detectores_em_ordem,
        &espec.classe,
    );
    dentro.destruir.extend(ctx.pipes.destruicao(espec.indice));
    // In collection mode, a refused node does not consume an index: the view
    // looks empty without being so. An empty `<ng-container *x>` and a
    // `<template>` without content (only comments, or nothing) are really
    // empty: the view has no root at all (`const <Object>[]`, cases i82, j54).
    let vazia = espec.nos.iter().all(|n| matches!(n, No::Comentario(_)))
        || matches!(espec.nos.as_slice(), [No::Elemento(x)]
        if x.nome == "ng-container"
            && x.filhos.is_empty()
            && x.estrela.is_none()
            && x.atributos.is_empty()
            && x.propriedades.is_empty()
            && x.eventos.is_empty()
            && x.bananas.is_empty()
            && x.referencias.is_empty());
    if !vazia
        && dentro.proximo == 0
        && dentro.raizes.is_empty()
        && dentro.coleta.as_ref().map_or(0, Vec::len) == anotadas
    {
        dentro.anotar(recusa(Motivo::Ligacao, "embedded view without a node"))?;
    }
    // The locals read by the detection, in order of first use.
    let mut declaracoes = Vec::new();
    for nome in dentro.locais_raiz.clone() {
        if let Some(Ok(d)) = dentro.decl_locais.get(&nome) {
            declaracoes.push(format!("    {d}"));
        }
    }
    // The listeners are written after the nodes: their imports come now.
    let ouvintes: Vec<String> = dentro
        .ouvintes
        .iter()
        .map(|o| resolver_tardios(dentro.imp, o))
        .collect();
    // The pipe proxies of this view (`afterNodes` of the component's view,
    // which runs after the `bindView` of all of them): they read the instance
    // in the component's view through the `parentView` chain
    // (`getPropertyInView`).
    let mut cadeia = "(this.parentView!)".to_string();
    for _ in 1..espec.profundidade {
        cadeia = format!("({cadeia}.parentView!)");
    }
    let base = format!("{util}.unsafeCast<{}0>({cadeia})", ctx.classe_da_visao);
    let criacao_pipes: Vec<String> = ctx
        .pipes
        .criacao(espec.indice, &base, dentro.imp)
        .iter()
        .map(|l| resolver_tardios(dentro.imp, l))
        .collect();
    // `_ctx` in `build()` only if someone reads it there (an event tear-off,
    // immutable text): `maybeCachedCtxDeclarationStatement`.
    let ctx_build = if cita_ctx(&dentro.linhas) || cita_ctx(&ouvintes) {
        "    final _ctx = this.ctx;\n"
    } else {
        ""
    };
    // The nodes, then the listeners — and only then the `initRootNode`, which
    // is the closing declaration (`_generateInitStatement`).
    let corpo = dentro
        .linhas
        .iter()
        .chain(&ouvintes)
        .chain(&criacao_pipes)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    // Same order as the top-level view: text bindings, child views and anchors,
    // previous values, elements (the query results before the bound ones).
    let mut todos = campos_com_inicializador(&dentro);
    todos.extend(dentro.campos.clone());
    todos.extend(dentro.campos_filho.clone());
    todos.extend(campos_da_deteccao(
        &dentro.campos_expr,
        &dentro.campos_el_de_embutidas,
        &dentro.posicao_de_embutida,
    ));
    todos.extend(ctx.pipes.campos(espec.indice, dentro.imp));
    let mut consultados = dentro.campos_el_consultados.clone();
    consultados.sort_by_key(|(k, _)| *k);
    todos.extend(consultados.into_iter().map(|(_, c)| c));
    todos.extend(campos_el_em_ordem(
        &dentro.campos_el,
        &dentro.refs_dos_campos_el,
        &dentro.locais_raiz,
    ));
    todos.extend(dentro.campos_el_por_evento.clone());
    todos.extend(dentro.campos_el_de_eventos.iter().map(|(_, c)| c.clone()));
    let campos = if todos.is_empty() {
        String::new()
    } else {
        format!("{}\n", todos.join("\n"))
    };
    let mut linhas_det = Vec::new();
    // `_ctx` and `firstCheck` are only declared if the body actually uses them
    // (`writeChangeDetectionStatements`): in an `*ngFor` view the interpolation
    // usually uses the loop local, not the context. Both come before the local
    // declarations.
    if cita_ctx(&dentro.entradas) || cita_ctx(&dentro.deteccao) {
        linhas_det.push("    final _ctx = this.ctx;".to_string());
    }
    if dentro.usa_changed {
        linhas_det.push("    bool changed = false;".to_string());
    }
    if dentro.usa_primeira_checagem {
        linhas_det.push("    bool firstCheck = this.firstCheck;".to_string());
    }
    // Same order as the top-level view, with the locals before everything.
    linhas_det.extend(declaracoes);
    linhas_det.extend(dentro.entradas.clone());
    for a in &dentro.ancoras {
        linhas_det.push(format!("    this.{a}.detectChangesInNestedViews();"));
    }
    let mut apos_conteudo = dentro.atualizacoes_de_conteudo.clone();
    apos_conteudo.extend(dentro.apos_conteudo.iter().cloned());
    linhas_det.extend(sem_lancar(&apos_conteudo));
    linhas_det.extend(dentro.deteccao.clone());
    for v in &dentro.vistas_filhas {
        linhas_det.push(format!("    this.{v}.detectChanges();"));
    }
    linhas_det.extend(sem_lancar(&dentro.apos_visao));
    let deteccao = if linhas_det.is_empty() {
        String::new()
    } else {
        format!(
            "\n  @override\n  void detectChangesInternal() {{\n{}\n  }}\n",
            linhas_det.join("\n")
        )
    };
    // `dirtyParentQueriesInternal`, between the detection and
    // `destroyInternal`: marks each query of the component's view with a result
    // here, and each content query of a view above (`_setParentQueryAsDirty`,
    // in the order of the first result).
    let mut de_conteudo = espec
        .estrela
        .and_then(|k| ctx.sujas_de_conteudo.borrow().get(&k).cloned())
        .unwrap_or_default();
    de_conteudo.sort_by_key(|(p, especie, _, _, _)| (*p, *especie));
    let sujas = if espec.refs_consultados.is_empty() && de_conteudo.is_empty() {
        String::new()
    } else {
        let cadeia = |niveis: u32| {
            let mut vista = "this".to_string();
            for _ in 0..niveis {
                vista = format!("({vista}.parentView!)");
            }
            vista
        };
        // The order is that of the first result of each query in this view; on
        // the same node, the content one before the view one
        // (`_getQueriesFor`).
        let mut marcadas: Vec<(usize, u8, usize, String)> = Vec::new();
        for (k, (r, campo, niveis)) in espec.sujas.iter().enumerate() {
            let posicao = inicio_do_primeiro_resultado_na_visao(&espec.nos, r, ctx.filhos)
                .unwrap_or(usize::MAX);
            marcadas.push((
                posicao,
                1,
                k,
                format!(
                    "    {util}.unsafeCast<{}0>({}).{campo} = true;",
                    ctx.classe_da_visao,
                    cadeia(*niveis)
                ),
            ));
        }
        for (k, (posicao, especie, campo, niveis, classe)) in de_conteudo.iter().enumerate() {
            marcadas.push((
                *posicao,
                *especie,
                k,
                format!(
                    "    {util}.unsafeCast<{classe}>({}).{campo} = true;",
                    cadeia(*niveis)
                ),
            ));
        }
        // Without a known position, the previous order (the view ones already
        // come sorted by the first result).
        marcadas.sort_by_key(|(p, tipo, k, _)| (*p, *tipo, *k));
        let linhas: Vec<String> = marcadas.into_iter().map(|(_, _, _, l)| l).collect();
        format!(
            "\n  @override\n  void dirtyParentQueriesInternal() {{\n{}\n  }}\n",
            linhas.join("\n")
        )
    };
    // `injectorGetInternal` comes after `build()` and before the detection.
    let injetor = resolver_tardios(
        dentro.imp,
        &metodo_injetor(&dentro.injetores, &dentro.asset),
    );
    let deteccao = resolver_refs(
        &resolver_tardios(dentro.imp, &deteccao),
        &ctx.refs_resolvidos.borrow(),
    );
    let deteccao = format!(
        "{}{deteccao}",
        metodo_de_link(
            ctx.link_de_deteccao,
            &dentro.ancoras,
            &dentro.vistas_ligadas
        )
    );
    // An embedded view also destroys what it hung on itself.
    let destruicao = if dentro.ancoras.is_empty()
        && dentro.vistas_filhas.is_empty()
        && dentro.destruir.is_empty()
    {
        String::new()
    } else {
        let mut linhas: Vec<String> = dentro
            .ancoras
            .iter()
            .map(|a| format!("    this.{a}.destroyNestedViews();"))
            .collect();
        linhas.extend(
            dentro
                .vistas_filhas
                .iter()
                .map(|v| format!("    this.{v}.destroyInternalState();")),
        );
        linhas.extend(dentro.destruir.iter().cloned());
        format!(
            "\n  @override\n  void destroyInternal() {{\n{}\n  }}\n",
            linhas.join("\n")
        )
    };
    let aninhadas = std::mem::take(&mut dentro.embutidas);
    // The `_handleEvent_N`s, after `destroyInternal`.
    if !dentro.metodos_i18n.is_empty() && !dentro.metodos_evento.is_empty() {
        return Err(recusa(
            Motivo::I18n,
            "@i18n with HTML and an event handler in the same view",
        ));
    }
    let metodos: String = dentro
        .metodos_i18n
        .iter()
        .chain(&dentro.metodos_evento)
        .map(|m| {
            resolver_refs(
                &resolver_tardios(dentro.imp, m),
                &ctx.refs_resolvidos.borrow(),
            )
        })
        .collect();
    // The roots (`rootNodesOrViewContainers`): each node created without a
    // parent, the local or the field (`renderNode.toReadExpr()`) — more than
    // one when the `*` is on an `<ng-container>`. In collection mode, with a
    // refused node, the list may come back empty; the text is discarded.
    let raizes = if dentro.raizes.is_empty() {
        vec!["_el_0".to_string()]
    } else {
        dentro.raizes.clone()
    };
    // `_generateInitStatement`: a single root and no `subscription_N` is
    // `initRootNode`; the rest goes in a list.
    let plana = lista_plana(&raizes, &util)?;
    let inicio = match (&plana, dentro.subscricoes) {
        _ if vazia && dentro.raizes.is_empty() && dentro.subscricoes == 0 => format!(
            "this.initRootNodesAndSubscriptions({util}.unsafeCast(const <Object>[]), null);"
        ),
        (ListaPlana::Literal(itens), 0) if itens.len() == 1 => {
            format!("this.initRootNode({});", itens[0])
        }
        (_, subscricoes) => {
            let subs = if subscricoes == 0 {
                "null".to_string()
            } else {
                let lista: Vec<String> = (0..subscricoes)
                    .map(|k| format!("subscription_{k}"))
                    .collect();
                format!("[{}]", lista.join(", "))
            };
            match &plana {
                // With the cascade broken, the arguments go one per line
                // (continuation indentation, column 8).
                ListaPlana::Cascata { .. } => format!(
                    "this.initRootNodesAndSubscriptions(\n        {util}.unsafeCast({}),\n        {subs});",
                    plana.texto_em(8)
                ),
                _ => format!(
                    "this.initRootNodesAndSubscriptions({util}.unsafeCast({}), {subs});",
                    plana.texto()
                ),
            }
        }
    };
    // Without a created node (the root is just a `TextBinding`), `build()` is
    // only the `initRootNode`.
    let corpo = if corpo.is_empty() {
        corpo
    } else {
        format!("{corpo}\n")
    };
    let tipo_do_contexto = &ctx.tipo_do_contexto;
    let (args, decl) = (&ctx.genericos, &ctx.genericos_decl);
    let texto = format!(
        "\nclass {classe}{decl} extends {ev}.EmbeddedView<{tipo_do_contexto}> {{\n{campos}  {classe}({rv}.RenderView parentView, int parentIndex) : super(parentView, parentIndex);\n  @override\n  void build() {{\n{ctx_build}{corpo}    {inicio}\n  }}\n{injetor}{deteccao}{sujas}{destruicao}{metodos}}}\n\n{ev}.EmbeddedView<void> {fabrica}{decl}({rv}.RenderView parentView, int parentIndex) {{\n  return {classe}{args}(parentView, parentIndex);\n}}\n"
    );
    Ok((texto, aninhadas))
}

/// `final local_x = unsafeCast<T>(this.locals['key']);` — the declaration that
/// `ViewNameResolver` keeps for the local, with the type qualified by the
/// import of the library that declares it. The import stays marked: it is
/// allocated where the declaration is written first.
fn declaracao_de_local(
    l: &crate::expr::Local,
    origem: &Origem,
    ctx: &Contexto,
    util: &str,
) -> Result<String, Recusa> {
    let d = &l.dart;
    // The key comes out as an escaped literal (`'\$implicit'`): without the
    // escape, the `$` would become an interpolation in Dart.
    let chave = literal(&origem.chave);
    // A local of this view: `this.locals`. Of an ancestor view: the
    // `parentView` chain, each step with `!`, and the cast to the class of the
    // view that declares it.
    let locals = if origem.niveis == 0 {
        "this.locals".to_string()
    } else {
        let mut vista = "(this.parentView!)".to_string();
        for _ in 1..origem.niveis {
            vista = format!("({vista}.parentView!)");
        }
        format!("{util}.unsafeCast<{}>({vista}).locals", origem.classe)
    };
    // Without a type, without a cast (`getLocal`: `type != null && type !=
    // dynamic`).
    if l.tipo == "dynamic" {
        return Ok(format!("final {d} = {locals}[{chave}];"));
    }
    let tipo = if matches!(
        l.tipo.as_str(),
        "String" | "int" | "double" | "bool" | "num" | "Object"
    ) {
        format!("{}{}", tardio_q("dart:core"), l.tipo)
    } else {
        let sem_import = || recusa(Motivo::Ligacao, "type of the `*ngFor` local without an import");
        let (r, arquivo) = ctx.tipos.ok_or_else(sem_import)?;
        let escopo = l.escopo.as_deref().unwrap_or(arquivo);
        let cheio = instanciar_crus(&l.tipo, escopo, r).ok_or_else(sem_import)?;
        let livres: Vec<&str> = ctx.nomes_genericos.iter().map(String::as_str).collect();
        tipo_qualificado(&cheio, escopo, r, &ctx.asset, &livres).ok_or_else(sem_import)?
    };
    // A private type becomes `dynamic` in `fromDartType`, and `getLocal` only
    // adds the cast when the type is not `dynamic`
    // (`view_name_resolver.dart:55`).
    if tipo == "dynamic" {
        return Ok(format!("final {d} = {locals}[{chave}];"));
    }
    Ok(format!(
        "final {d} = {util}.unsafeCast<{tipo}>({locals}[{chave}]);"
    ))
}

/// `if ((!debugThrowIfChanged)) { .. }` around the content or view hooks
/// (`notThrowOnChanges` in `writeChangeDetectionStatements`); nothing if there
/// are no hooks.
fn sem_lancar(linhas: &[String]) -> Option<String> {
    if linhas.is_empty() {
        return None;
    }
    let dbg = tardio(CHECK_BINDING);
    Some(format!(
        "    if ((!{dbg}.debugThrowIfChanged)) {{\n{}\n    }}",
        indentar(&linhas.join("\n"), 2)
    ))
}

/// Reindents a block of statements: each line gets `n` spaces.
fn indentar(texto: &str, n: usize) -> String {
    let prefixo = " ".repeat(n);
    texto
        .lines()
        .map(|l| format!("{prefixo}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `addStmtsIfFirstCheck`: if the last statement of the method is an `if
/// (firstCheck)`, the new ones go into it; otherwise, they open another.
fn na_primeira_checagem(metodo: &mut Vec<String>, instrucoes: &[String]) {
    const ABRE: &str = "    if (firstCheck) {\n";
    const FECHA: &str = "\n    }";
    match metodo.last_mut() {
        Some(ultimo) if ultimo.starts_with(ABRE) && ultimo.ends_with(FECHA) => {
            ultimo.truncate(ultimo.len() - FECHA.len());
            ultimo.push('\n');
            ultimo.push_str(&instrucoes.join("\n"));
            ultimo.push_str(FECHA);
        }
        _ => metodo.push(format!("{ABRE}{}{FECHA}", instrucoes.join("\n"))),
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
fn alocar_imports_preguicosos(
    imp: &mut Importacoes,
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    asset: &str,
) {
    for campo in campos_em_ordem(nos, filhos, usadas, asset) {
        if let CampoDaVisao::Preguicosos(textos) = campo {
            for t in &textos {
                resolver_tardios(imp, t);
            }
        }
    }
}

fn alocar_imports_dos_campos(
    imp: &mut Importacoes,
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    asset: &str,
    pipes: &[String],
    refs: &std::collections::HashSet<String>,
    tipos: &[TipoDeDiretivaResolvido],
) -> Result<(), Recusa> {
    let sem_caminho = || recusa(Motivo::ComponenteNoTemplate, "child without an import path");
    // The `directiveTypes:` arguments come out in the field type, right after
    // the class name: their imports come right after the class's.
    let argumentos_de = |imp: &mut Importacoes, texto_do_import: &str| {
        for t in tipos {
            let da_classe = tardio(&import_de(&t.uri, asset));
            let do_ngcd = tardio(&import_de(&t.uri.replace(".dart", ".template.dart"), asset));
            if texto_do_import == da_classe || texto_do_import == do_ngcd {
                resolver_tardios(imp, &t.argumentos);
            }
        }
    };
    let campos = campos_em_ordem(nos, filhos, usadas, asset);
    for campo in campos {
        match campo {
            CampoDaVisao::Preguicosos(_) => {}
            CampoDaVisao::Container => {
                imp.alias(VIEW_CONTAINER);
            }
            CampoDaVisao::Filho(f, container, antes) => {
                for (k, uri) in [&f.uri_template, &f.uri_dart].into_iter().enumerate() {
                    // The node's `ViewContainer` (a child or one of its
                    // directives that injects `ViewContainerRef`) and the
                    // providers the child injects between the view and the
                    // instance.
                    if k == 1 {
                        if container {
                            imp.alias(VIEW_CONTAINER);
                        }
                        for t in &antes {
                            resolver_tardios(imp, t);
                        }
                    }
                    let alvo = asset_de_uri(uri, "", Path::new("")).ok_or_else(sem_caminho)?;
                    let caminho = caminho_do_import(asset, &alvo).ok_or_else(sem_caminho)?;
                    if !e_o_proprio_template(asset, &caminho) {
                        imp.alias(&caminho);
                    }
                    for t in tipos {
                        if t.uri == f.uri_dart && t.classe == f.classe {
                            resolver_tardios(imp, &t.argumentos);
                        }
                    }
                }
            }
            CampoDaVisao::Estrutural(uri) => {
                imp.alias(VIEW_CONTAINER);
                imp.alias(uri);
            }
            CampoDaVisao::Molde(com_ref, campos) => {
                imp.alias(VIEW_CONTAINER);
                if com_ref {
                    imp.alias(TEMPLATE_REF);
                }
                for t in campos {
                    resolver_tardios(imp, &t);
                    argumentos_de(imp, &t);
                }
            }
            CampoDaVisao::Diretivas(textos) => {
                for t in textos {
                    resolver_tardios(imp, &t);
                    argumentos_de(imp, &t);
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
    match crate::dialect::atual() {
        crate::dialect::Dialeto::Ngdart => {
            if tem_elemento_ligado(nos, filhos, usadas, refs) {
                imp.alias("dart:html");
            }
        }
        crate::dialect::Dialeto::Ngx => {
            let mut tipos = Vec::new();
            tipos_dos_elementos_ligados(nos, filhos, usadas, refs, false, &mut tipos);
            for t in tipos {
                imp.alias(crate::dialect::dom(t).0);
            }
        }
    }
    Ok(())
}

/// The types (by the `dart:html` name) of the nodes of this view that become
/// fields, in document order: what [`tem_elemento_ligado`] finds, with the type
/// of each one (`Element` with a namespace, `HtmlElement` on a child's node).
fn tipos_dos_elementos_ligados(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    refs: &std::collections::HashSet<String>,
    em_ns: bool,
    saida: &mut Vec<&'static str>,
) {
    for n in nos {
        let No::Elemento(e) = n else { continue };
        if e.estrela.is_some() {
            continue;
        }
        let ns = em_ns
            || e.nome.contains(':')
            || matches!(e.nome.to_ascii_lowercase().as_str(), "svg" | "math");
        match filhos.get(&e.nome) {
            None => {
                if liga_no_elemento(e, &diretivas_casadas(usadas, e))
                    || e.referencias.iter().any(|r| refs.contains(&r.nome))
                {
                    saida.push(if ns {
                        "Element"
                    } else {
                        dom::tipo_da_tag(&e.nome)
                    });
                }
            }
            Some(f) => {
                if elemento_do_filho_ligado(e, f, &diretivas_casadas(usadas, e)) {
                    saida.push("HtmlElement");
                }
            }
        }
        tipos_dos_elementos_ligados(&e.filhos, filhos, usadas, refs, ns, saida);
    }
}

/// Alguma linha usa `_ctx`?
fn cita_ctx(linhas: &[String]) -> bool {
    linhas.iter().any(|l| {
        l.split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|t| t == "_ctx")
    })
}

/// Index of the element that serves as the parent, or `null` if it is the root
/// of the view.
fn indice_do_elemento(pai: &str) -> String {
    pai.rsplit_once("_el_")
        .or_else(|| pai.rsplit_once("_anchor_"))
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| "null".to_string())
}

/// How many structural directives there are in the subtree — it is the jump the
/// numbering of the embedded views makes before the next sibling.
fn contar_estruturais(nos: &[No]) -> u32 {
    nos.iter()
        .map(|n| match n {
            No::Elemento(e) => u32::from(e.estrela.is_some()) + contar_estruturais(&e.filhos),
            _ => 0,
        })
        .sum()
}

/// Does the local appear in some expression of the subtree?
///
/// Its declaration opens `detectChangesInternal` and brings `dart:core` along,
/// so the decision must be made before walking the body.
fn local_citado(nos: &[No], nome: &str) -> bool {
    let cita = |texto: &str| cita_na_raiz(texto, nome);
    nos.iter().any(|n| match n {
        No::Interpolacao { expr, .. } => cita(expr),
        No::Elemento(e) => {
            e.propriedades
                .iter()
                .chain(e.eventos.iter())
                .chain(e.bananas.iter())
                .any(|l| cita(&l.valor))
                || e.atributos
                    .iter()
                    .any(|a| a.valor.contains("{{") && cita(&a.valor))
                || e.estrela.as_ref().is_some_and(|l| cita(&l.valor))
                || local_citado(&e.filhos, nome)
        }
        _ => false,
    })
}

/// Does the expression read `nome` with the implicit receiver — which is when
/// `ViewNameResolver.getLocal` is called? A name after `.`/`?.` is a member of
/// something else, before `:` after `(`/`,` it is a named argument, and inside
/// quotes it is text.
fn cita_na_raiz(texto: &str, nome: &str) -> bool {
    let cs: Vec<char> = texto.chars().collect();
    let parte = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
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
        if !parte(c) {
            i += 1;
            continue;
        }
        let ini = i;
        while i < cs.len() && parte(cs[i]) {
            i += 1;
        }
        if cs[ini..i].iter().copied().eq(nome.chars()) {
            let antes = cs[..ini].iter().rev().find(|c| !c.is_whitespace());
            let depois = cs[i..].iter().find(|c| !c.is_whitespace());
            let membro = antes == Some(&'.');
            let nomeado = depois == Some(&':') && matches!(antes, Some('(' | ','));
            if !membro && !nomeado {
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
struct Estrutural {
    classe: &'static str,
    uri: &'static str,
    /// The input is written directly, without `checkBinding`.
    direta: bool,
    /// Implements `DoCheck`: the view calls `ngDoCheck()` during detection.
    do_check: bool,
    /// Third constructor argument: `@Host()` of a directive of the same library
    /// on an element above (the `NgSwitch` of `NgSwitchWhen`).
    hospedeiro: Option<&'static str>,
    /// The constructor receives the `TemplateRef` (`NgTemplateOutlet` only the
    /// `ViewContainerRef`; the `TemplateRef` of the `*` stays in an unused
    /// local).
    com_template: bool,
    /// The inputs (`@Input`), in declaration order in the class: it is the
    /// order of the bindings (`ast.inputs.sort(_orderingOf(directive.inputs))`
    /// in `ast_template_parser.dart`), whatever the written order.
    entradas: &'static [&'static str],
}

impl Estrutural {
    fn conhecida(nome: &str) -> Option<Estrutural> {
        match nome {
            "ngIf" => Some(Estrutural {
                classe: "NgIf",
                uri: NG_IF,
                direta: true,
                do_check: false,
                hospedeiro: None,
                com_template: true,
                entradas: &["ngIf"],
            }),
            "ngFor" => Some(Estrutural {
                classe: "NgFor",
                uri: NG_FOR,
                direta: false,
                do_check: true,
                hospedeiro: None,
                com_template: true,
                entradas: &["ngForOf", "ngForTemplate", "ngForTrackBy"],
            }),
            "ngTemplateOutlet" => Some(Estrutural {
                classe: "NgTemplateOutlet",
                uri: NG_TEMPLATE_OUTLET,
                direta: false,
                do_check: true,
                hospedeiro: None,
                com_template: false,
                entradas: &[
                    "ngTemplateOutlet",
                    "ngTemplateOutletContext",
                    "ngTemplateOutletValue",
                ],
            }),
            // Both inputs are setters without their own comparison: they go
            // through `checkBinding` (or, if immutable, through
            // `_bindLiteral`).
            "ngSwitchCase" | "ngSwitchWhen" => Some(Estrutural {
                classe: "NgSwitchWhen",
                uri: NG_SWITCH,
                direta: false,
                do_check: false,
                hospedeiro: Some("NgSwitch"),
                com_template: true,
                entradas: &["ngSwitchCase", "ngSwitchWhen"],
            }),
            "ngSwitchDefault" => Some(Estrutural {
                classe: "NgSwitchDefault",
                uri: NG_SWITCH,
                direta: false,
                do_check: false,
                hospedeiro: Some("NgSwitch"),
                com_template: true,
                entradas: &[],
            }),
            _ => None,
        }
    }
}

/// `List<String>` -> `String`. Without a type argument the loop local cannot be
/// typed, and the official compiler writes `unsafeCast<T>` with an explicit T.
fn tipo_do_elemento(tipo: &str) -> Option<String> {
    let t = tipo.trim().trim_end_matches('?');
    let abre = t.find('<')?;
    let base = t[..abre].trim();
    if !matches!(base, "List" | "Iterable" | "Set") {
        return None;
    }
    let dentro = t[abre + 1..].strip_suffix('>')?;
    // A single argument at the top level; the nested ones (`List<List<X>>`,
    // `List<Map<K, V>>`) go whole.
    let mut nivel = 0i32;
    for c in dentro.chars() {
        match c {
            '<' => nivel += 1,
            '>' => nivel -= 1,
            ',' if nivel == 0 => return None,
            _ => {}
        }
        if nivel < 0 {
            return None;
        }
    }
    (nivel == 0).then(|| dentro.trim().to_string())
}

/// The type the analyzer gives to the name of a generic class written raw
/// (`Grupo` in `List<Grupo>`): the instantiation to bounds, which
/// `fromDartType` writes with the arguments (`Grupo<dynamic>`). Without a bound
/// the argument is `dynamic`; with a bound there is no case yet (`None`).
fn instanciar_crus(texto: &str, escopo: &Path, r: &dyn Resolucao) -> Option<String> {
    let mut saida = String::new();
    let chars: Vec<char> = texto.trim().chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_alphanumeric() || c == '_' || c == '$' {
            let mut nome = String::new();
            while i < chars.len()
                && (chars[i].is_alphanumeric() || matches!(chars[i], '_' | '$' | '.'))
            {
                nome.push(chars[i]);
                i += 1;
            }
            saida.push_str(&nome);
            let seguido_de_args = chars[i..].iter().find(|c| !c.is_whitespace()) == Some(&'<');
            if !seguido_de_args && !matches!(nome.as_str(), "dynamic" | "void" | "Function") {
                if let Some(limites) = r.limites_de_tipo(escopo, &nome) {
                    if !limites.is_empty() {
                        if limites.iter().any(|&l| l) {
                            return None;
                        }
                        saida.push('<');
                        saida.push_str(&vec!["dynamic"; limites.len()].join(", "));
                        saida.push('>');
                    }
                }
            }
            continue;
        }
        saida.push(c);
        i += 1;
    }
    Some(saida)
}

/// The text of a type with each name qualified by the import of the library
/// that declares it, as the official compiler writes a `DartType`
/// (`List<import2.X>`: `dart:core` without a prefix, but with the import
/// allocated). The imports stay marked ([`tardio_q`]). `None` for what is not a
/// named type (function, record) or a name not found in `escopo`.
/// The names in `livres` (type parameters in scope) come out as they are.
fn tipo_qualificado(
    texto: &str,
    escopo: &Path,
    r: &dyn Resolucao,
    asset: &str,
    livres: &[&str],
) -> Option<String> {
    let mut saida = String::new();
    let mut chars = texto.trim().chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_alphanumeric() || c == '_' || c == '$' {
            let mut nome = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || matches!(c, '_' | '$' | '.') {
                    nome.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            if matches!(nome.as_str(), "dynamic" | "void") || livres.contains(&nome.as_str()) {
                saida.push_str(&nome);
                continue;
            }
            if nome == "Function" {
                return None;
            }
            let simples = crate::resolucao::sem_ancora(nome.rsplit('.').next().unwrap_or(&nome));
            // A private type is not visible in the generated file:
            // `fromDartType` replaces it with `dynamic`, with its arguments and
            // `?` (`convert.dart:35-37`).
            if simples.starts_with('_') {
                saida.push_str("dynamic");
                pular_argumentos_e_anulavel(&mut chars);
                continue;
            }
            let uri = r.uri_do_tipo(escopo, &nome)?;
            let caminho = if uri.starts_with("dart:") {
                uri
            } else {
                caminho_do_import(asset, &asset_de_uri(&uri, "", Path::new(""))?)?
            };
            saida.push_str(&tardio_q(&caminho));
            saida.push_str(simples);
            continue;
        }
        chars.next();
        match c {
            '<' | '>' | '?' => saida.push(c),
            ',' => saida.push_str(", "),
            c if c.is_whitespace() => {}
            _ => return None,
        }
    }
    Some(saida)
}

/// The second argument of `addEventListener` for a native event: the handler in
/// 8; in 9, the function exported to JS with the event type
/// (`FunctionToJSExportedDartFunction((h as void Function(MouseEvent))).toJS`,
/// `NX:compiler/view_compiler/update_statement_visitor.dart:254-261`). The
/// imports go as deferred marks, allocated in text order.
fn ouvinte_nativo(nome: &str, handler: &str) -> String {
    if crate::dialect::atual() == crate::dialect::Dialeto::Ngdart {
        return handler.to_string();
    }
    let js = tardio_q("dart:js_interop");
    let tipo = tipo_do_evento_nativo(nome);
    format!("{js}FunctionToJSExportedDartFunction(({handler} as void Function({tipo}))).toJS")
}

/// The type of a native event in 9, qualified by a deferred mark
/// (`nativeHtmlEventType`, `NX:compiler/html_events.dart:116-177`).
fn tipo_do_evento_nativo(nome: &str) -> String {
    let (uri, n) = crate::dialect::dom(crate::dialect::tipo_do_evento_nativo(nome));
    format!("{}{n}", tardio_q(uri))
}

/// The type of the `$event` of an event written on the element: in 9, that of
/// the native event (`NativeEvent`); a custom event (`keyup.enter`,
/// `CustomEvent`) and 8 have none (`binding_converter.dart:75,228`).
fn tipo_de_evento_nativo(nome: &str) -> Option<String> {
    (crate::dialect::atual() == crate::dialect::Dialeto::Ngx && evento_nativo(nome))
        .then(|| tipo_do_evento_nativo(nome))
}

/// The parameter of a `_handleEvent_N`: a raw `$event` in 8 (and in 9 without a
/// type: a custom event, an output that is not a `Stream`), `T $event` with the
/// event type in 9 (`NX:compiler/view_compiler/compile_view.dart:726-760`).
fn parametro_do_evento(tipo: &Option<String>) -> String {
    match tipo {
        Some(t) => format!("{t} $event"),
        None => "$event".to_string(),
    }
}

/// Consumes, after a type name, its arguments (`<..>`, nested) and its `?`.
fn pular_argumentos_e_anulavel(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while chars.next_if(|c| c.is_whitespace()).is_some() {}
    if chars.next_if_eq(&'<').is_some() {
        let mut nivel = 1usize;
        for c in chars.by_ref() {
            match c {
                '<' => nivel += 1,
                '>' => {
                    nivel -= 1;
                    if nivel == 0 {
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
/// ([`resolver_tardios`]).
fn tardio(uri: &str) -> String {
    format!("\u{1}{uri}\u{2}")
}

/// Like [`tardio`], but resolves to the qualifier (`import3.` or nothing, for
/// the libraries imported without a prefix).
fn tardio_q(uri: &str) -> String {
    format!("\u{1}q:{uri}\u{2}")
}

/// Replaces each mark of [`tardio`] and [`tardio_q`] with the prefix,
/// allocating in text order.
fn resolver_tardios(imp: &mut Importacoes, texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(i) = resto.find('\u{1}') {
        saida.push_str(&resto[..i]);
        let Some(f) = resto[i..].find('\u{2}') else {
            break;
        };
        let marca = &resto[i + 1..i + f];
        if let Some((chave, uri)) = marca.strip_prefix("k:").and_then(|m| m.split_once('|')) {
            saida.push_str(&imp.q_chave(chave, uri));
        } else {
            match marca.strip_prefix("q:") {
                Some(uri) => saida.push_str(&imp.q(uri)),
                None => saida.push_str(&imp.alias(marca)),
            }
        }
        resto = &resto[i + f + 1..];
    }
    saida.push_str(resto);
    saida
}

/// Property or attribute name with a security context in some tag
/// (`_initializeSecuritySchema`, in `dom_element_schema_registry.dart`): the
/// value comes out wrapped in a `sanitize*`. Without looking at the tag,
/// refuses the name on any element.
/// The sanitizer of a DOM property (`securityContext` of
/// `DomElementSchemaRegistry`, with its table): `sanitizeHtml`,
/// `sanitizeStyle`, `sanitizeUrl` or `sanitizeResourceUrl`, or none. The key is
/// `tag|property`, then `*|property`.
fn saneador(tag: &str, prop: &str) -> Option<&'static str> {
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
    const RECURSO: &[&str] = &[
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
    let contexto = |chave: &str| {
        if HTML.contains(&chave) {
            Some("sanitizeHtml")
        } else if chave == "*|style" {
            Some("sanitizeStyle")
        } else if URL.contains(&chave) {
            Some("sanitizeUrl")
        } else if RECURSO.contains(&chave) {
            Some("sanitizeResourceUrl")
        } else {
            None
        }
    };
    contexto(&format!("{tag}|{prop}")).or_else(|| contexto(&format!("*|{prop}")))
}

/// `getMappedPropName` of `DomElementSchemaRegistry`: the property name of an
/// attribute, used to find the security context.
fn propriedade_mapeada(atributo: &str) -> &str {
    match atributo {
        "class" => "className",
        "innerHtml" => "innerHTML",
        "readonly" => "readOnly",
        "tabindex" => "tabIndex",
        outro => outro,
    }
}

/// ngcompiler's `isNativeHtmlEvent` (`html_events.dart`): only these go
/// directly to `addEventListener`.
pub(crate) fn evento_nativo(nome: &str) -> bool {
    const NATIVOS: &[&str] = &[
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
    NATIVOS.contains(&nome)
}

/// A name that belongs to an ecosystem directive, not to the DOM.
fn e_de_diretiva(nome: &str) -> bool {
    let base = nome.split('.').next().unwrap_or(nome);
    base.starts_with("ng") || base.starts_with("form") && base != "form"
}

/// Does the template have `{{ … }}` in this view?
///
/// It does not descend into the subtree of a `*`: that content belongs to the
/// embedded view, and its fields are declared there.
fn tem_interpolacao(nos: &[No]) -> bool {
    nos.iter().any(|n| match n {
        No::Interpolacao { .. } => true,
        No::Elemento(e) if e.estrela.is_none() => tem_interpolacao(&e.filhos),
        _ => false,
    })
}

/// `o.literal('${value}')` of a primitive literal: the text of the value,
/// between single quotes. A string literal already arrives written like that;
/// `null` becomes empty text.
fn valor_de_literal(texto: &str) -> String {
    if texto.starts_with('\'') {
        texto.to_string()
    } else if texto == "null" {
        "''".to_string()
    } else {
        literal(texto)
    }
}

/// `&ngsp;`, which `ngast` replaces with this character; it turns back into a
/// space.
const NGSP: char = '\u{E500}';

/// `_compressWhitespacePreceding`: with a line break (and without `&nbsp;` or
/// `&ngsp;`), the line breaks disappear and the start is trimmed.
fn comprimir_antes(t: &str) -> String {
    if t.contains('\u{00A0}') || t.contains(NGSP) || !t.contains('\n') {
        return t.replace(NGSP, " ");
    }
    t.replace('\n', "").trim_start().replace(NGSP, " ")
}

/// `_compressWhitespaceFollowing`: o mesmo, aparando o fim.
fn comprimir_depois(t: &str) -> String {
    if t.contains('\u{00A0}') || t.contains(NGSP) || !t.contains('\n') {
        return t.replace(NGSP, " ");
    }
    t.replace('\n', "").trim_end().replace(NGSP, " ")
}

/// The texts and expressions of a value with `{{ }}` (`splitInterpolation`,
/// with the regular expression `{{([\s\S]*?)}}`): always one more text than
/// expressions. An empty expression is an error in the official compiler.
fn partes_da_interpolacao(valor: &str) -> Option<(Vec<String>, Vec<String>)> {
    let mut textos = Vec::new();
    let mut exprs = Vec::new();
    let mut resto = valor;
    while let Some(i) = resto.find("{{") {
        let depois = &resto[i + 2..];
        let f = depois.find("}}")?;
        let e = &depois[..f];
        if e.trim().is_empty() {
            return None;
        }
        textos.push(resto[..i].to_string());
        exprs.push(e.to_string());
        resto = &depois[f + 2..];
    }
    textos.push(resto.to_string());
    (!exprs.is_empty()).then_some((textos, exprs))
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
/// ([`classe_ngcd`]).
pub fn detector_de_diretivas(hs: &[&crate::Hospedeira], arquivo: &str) -> String {
    let mut imp = Importacoes::default();
    let classes: Vec<String> = hs
        .iter()
        .map(|h| classe_ngcd(h, arquivo, &mut imp))
        .collect();
    let mut s = String::with_capacity(1024 * classes.len().max(1));
    s.push_str(crate::CABECALHO);
    let _ = writeln!(s, "import '{arquivo}';");
    imp.escrever(&mut s);
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
pub fn classe_ngcd(h: &crate::Hospedeira, arquivo: &str, imp: &mut Importacoes) -> String {
    let cd = imp.alias(DIRECTIVE_CHANGE_DETECTOR);
    let proprio = imp.alias(arquivo);
    let rv = imp.alias(RENDER_VIEW);
    let elemento = imp.dom("Element");
    // The imports of the body are allocated in text order, which is only known
    // once assembled (the immutable ones come first): deferred marks, resolved
    // at the end.
    let chk = tardio(CHECK_BINDING);
    let x = &h.classe;
    let mut campos = String::new();
    let mut corpo = String::new();
    // The immutable ones (`isImmutable`: a `final` field) come out first, in
    // `if (firstCheck)`, without a field, but with their index (case j103).
    let mut constantes = String::new();
    for (k, (nome, membro)) in h.ligacoes.iter().enumerate() {
        // `hospedeira` (lib.rs) only lets the known shapes through.
        let Ok(mut forma) = forma_do_hospedeiro(nome) else {
            continue;
        };
        let estatico = h.estaticos.contains(membro);
        if let FormaDoHospedeiro::Estilo { texto, nulo, .. } = &mut forma {
            // The static is `dynamic` for `_TypeResolver`: `?.toString()`.
            let tipo = if estatico {
                "dynamic"
            } else {
                h.tipos_de_estilo
                    .get(membro)
                    .map(String::as_str)
                    .unwrap_or_default()
            };
            *texto = tipo.trim_end_matches('?') == "String";
            // `isNullable` is the expression's `canBeNull`
            // (`ir/model.dart:609`): outside a literal, always
            // (`analyzed_class.dart:210-221`).
            *nulo = true;
        }
        if h.imutaveis.contains(membro) || estatico {
            let valor = if estatico {
                format!("{proprio}.{x}.{membro}")
            } else {
                format!("this.instance.{membro}")
            };
            let acao = forma.acao("el", &valor);
            let _ = write!(
                constantes,
                "      if (({valor} != null)) {{\n        {acao};\n      }}\n"
            );
            continue;
        }
        let acao = forma.acao("el", &format!("currVal_{k}"));
        let _ = writeln!(campos, "  Object? _expr_{k};");
        let _ = write!(
            corpo,
            "    final currVal_{k} = this.instance.{membro};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, null, null)) {{\n      {acao};\n      this._expr_{k} = currVal_{k};\n    }}\n"
        );
    }
    if !constantes.is_empty() {
        corpo = format!(
            "    bool firstCheck = view.firstCheck;\n    if (firstCheck) {{\n{constantes}    }}\n{corpo}"
        );
    }
    let corpo = resolver_tardios(imp, &corpo);
    format!(
        "\nclass {x}NgCd extends {cd}.DirectiveChangeDetector {{\n  final {proprio}.{x} instance;\n{campos}  {x}NgCd(this.instance);\n  void detectHostChanges({rv}.RenderView view, {elemento} el) {{\n{corpo}  }}\n}}\n"
    )
}

/// Generates a component's fragment in the `.template.dart` — from `styles$X`
/// to the host view factory —, allocating the imports in the file's table. With
/// several components in the file, there is a single table and the fragments
/// come out in source order ([`montar_arquivo`]), as the official compiler
/// writes them.
///
/// Whatever does not fit comes back as `Err` with the first refusal, and the
/// file keeps coming from `build_runner`.
#[allow(clippy::too_many_arguments)]
pub fn trecho_de_componente(
    c: &Componente,
    local: &Local,
    nos: &[No],
    resolvedor: Option<&dyn Resolucao>,
    nomes: &mut dartforge_intern::Interner,
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    pipes: &Result<Vec<PipeUsado>, Recusa>,
    imp: &mut Importacoes,
) -> Result<String, Recusa> {
    let mut coleta = None;
    let texto = gerar_componente(
        c,
        local,
        nos,
        resolvedor,
        nomes,
        filhos,
        usadas,
        pipes,
        &mut coleta,
        imp,
    )?;
    // A deferred import mark that no step resolved would end up in the file: it
    // is a shape the emitter does not write yet.
    if texto.contains(['\u{1}', '\u{2}']) {
        return Err(recusa(
            Motivo::NaoEntendido,
            "unresolved deferred import in the output",
        ));
    }
    Ok(texto)
}

/// The `.template.dart` of a file with a single component:
/// [`trecho_de_componente`] with its own import table.
#[allow(clippy::too_many_arguments)]
pub fn template_de_componente(
    c: &Componente,
    local: &Local,
    nos: &[No],
    resolvedor: Option<&dyn Resolucao>,
    nomes: &mut dartforge_intern::Interner,
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    pipes: &Result<Vec<PipeUsado>, Recusa>,
) -> Result<String, Recusa> {
    let mut imp = Importacoes::default();
    let trecho = trecho_de_componente(
        c, local, nos, resolvedor, nomes, filhos, usadas, pipes, &mut imp,
    )?;
    Ok(montar_arquivo(local.arquivo, &imp, &[trecho]))
}

/// The whole `.template.dart`: the header, the import of itself, the import
/// table and the components' fragments.
pub fn montar_arquivo(arquivo: &str, imp: &Importacoes, trechos: &[String]) -> String {
    let mut s = String::with_capacity(4096 * trechos.len().max(1));
    s.push_str(crate::CABECALHO);
    let _ = writeln!(s, "import '{arquivo}';");
    imp.escrever(&mut s);
    for t in trechos {
        s.push_str(t);
    }
    crate::expr::resolver_quebras(&s)
}

/// All the refusals of this component, not only the first: runs the same
/// emission in collection mode. Without this the scoreboard is misleading — a
/// file that gets stuck on a stylesheet may also get stuck on an event and an
/// interpolation, and counting only the first makes it look as if learning one
/// shape unlocks the file.
#[allow(clippy::too_many_arguments)]
pub fn coletar(
    c: &Componente,
    local: &Local,
    nos: &[No],
    resolvedor: Option<&dyn Resolucao>,
    nomes: &mut dartforge_intern::Interner,
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    pipes: &Result<Vec<PipeUsado>, Recusa>,
) -> std::collections::BTreeSet<Recusa> {
    let mut coleta = Some(Vec::new());
    let r = gerar_componente(
        c,
        local,
        nos,
        resolvedor,
        nomes,
        filhos,
        usadas,
        pipes,
        &mut coleta,
        &mut Importacoes::default(),
    );
    let mut fora: std::collections::BTreeSet<Recusa> =
        coleta.unwrap_or_default().into_iter().collect();
    if let Err(r) = r {
        fora.insert(r);
    }
    // The stylesheet is another output (`gerar_folha`): the template only
    // imports it.
    if c.style_urls
        .iter()
        .any(|u| local.uri_do_estilo(u, !c.sem_encapsulamento).is_none())
    {
        fora.insert(recusa(Motivo::Estilos, "stylesheet outside lib/"));
    }
    fora
}

/// What a component's `@HostBinding` writes on the host element: the bindings
/// of `createElementPropertyAst` with the `div` element
/// (`_securityContextElementName` of `DirectiveConverter`), written by
/// `bindAndWriteToRenderer` with `isHtmlElement` false.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FormaDoHospedeiro {
    /// `class.x`: `updateClassBindingNonHtml`.
    Classe(String),
    /// `class`, `className`, `attr.class` on a component: the whole class,
    /// `this.updateChildClassNonHtml(this.rootElement, v)` (the unnamed
    /// `ClassBinding`, case j80).
    ClasseInteira,
    /// `attr.x`, with the sanitizer of the security context: `updateAttribute`.
    Atributo(String, Option<&'static str>),
    /// A static `attr.tabindex`: the element's `tabIndex` property (the
    /// `TabIndexBinding` of `binding_converter.dart`, which only the view
    /// constructor uses; the dynamic binding is still `updateAttribute`, case
    /// j96).
    TabIndex,
    /// `style.x` and `style.x.unit`, with the text of the value decided by the
    /// member type (`visitStyleBinding`: `isString`, `isNullable`).
    Estilo {
        nome: String,
        unidade: Option<String>,
        texto: bool,
        nulo: bool,
    },
    /// Propriedade (nome mapeado por `getMappedPropName`), com o saneador:
    /// `setProperty`.
    Propriedade(String, Option<&'static str>),
}

/// A component's `@HostBinding`, resolved: the member read, whether it is
/// immutable (`isImmutable`: a `final` field) and the shape.
#[derive(Debug, Clone)]
struct LigacaoDoHospedeiro {
    membro: String,
    imutavel: bool,
    /// Static member: written in the view constructor, with the value read from
    /// the class (`X.member`), and outside `detectHostChanges` (case j96).
    estatico: bool,
    forma: FormaDoHospedeiro,
}

impl LigacaoDoHospedeiro {
    /// The statement that writes the value `v` on the host element.
    fn acao(&self, v: &str) -> String {
        self.forma.acao("this.rootElement", v)
    }
}

impl FormaDoHospedeiro {
    /// The statement that writes the value `v` on the element `el` (the
    /// component's `this.rootElement`, the `el` of the directive's `XNgCd`).
    /// The imports go as [`tardio`] marks, in text order.
    pub(crate) fn acao(&self, el: &str, v: &str) -> String {
        let dom = tardio(DOM_HELPERS);
        let saneado = |s: &Option<&'static str>| match s {
            Some(f) => format!("{}.{f}({v})", tardio(SAFE_HTML)),
            None => v.to_string(),
        };
        match self {
            FormaDoHospedeiro::Classe(x) => {
                format!("{dom}.updateClassBindingNonHtml({el}, '{x}', {v})")
            }
            // Only the component gets here
            // ([`forma_do_hospedeiro_de_componente`]): the receiver is its
            // view.
            FormaDoHospedeiro::ClasseInteira => {
                format!("this.updateChildClassNonHtml({el}, {v})")
            }
            FormaDoHospedeiro::Atributo(x, s) => {
                format!("{dom}.updateAttribute({el}, '{x}', {})", saneado(s))
            }
            FormaDoHospedeiro::TabIndex => format!("{el}.tabIndex = {v}"),
            FormaDoHospedeiro::Propriedade(x, s) => {
                format!("{dom}.setProperty({el}, '{x}', {})", saneado(s))
            }
            FormaDoHospedeiro::Estilo {
                nome,
                unidade,
                texto,
                nulo,
            } => {
                let valor = match unidade {
                    Some(u) => {
                        let t = if *texto {
                            v.to_string()
                        } else {
                            format!("{v}.toString()")
                        };
                        format!("(({v} == null) ? {} : ({t} + {}))", estilo_nulo(), literal(u))
                    }
                    None => estilo_sem_unidade(v, *texto, *nulo),
                };
                format!("{el}.style.setProperty('{nome}', {valor})")
            }
        }
    }
}

/// The shape of a `@HostBinding` name (`createElementPropertyAst`): whatever is
/// not `class.x`, `attr.x`, `style.x[.unit]` or a simple property is refused —
/// `class`/`className` (the whole class), `attr.x.if`, a namespace and an
/// unknown prefix have no case yet.
pub(crate) fn forma_do_hospedeiro(nome: &str) -> Result<FormaDoHospedeiro, String> {
    let simples = |n: &str| !n.is_empty() && !n.contains(['.', ':']);
    let fora = || format!("@HostBinding('{nome}') outside class.x, attr.x, style.x and property");
    let partes: Vec<&str> = nome.split('.').collect();
    Ok(match partes.as_slice() {
        // `class.x.y`: only the second part counts (`boundPropertyName =
        // parts[1]`, `template_parser.dart:100-102`), the rest is ignored (the
        // `class.basic-icon.if` of `material_icon_toggle`).
        ["class", x, ..] if simples(x) => FormaDoHospedeiro::Classe(x.to_string()),
        ["attr", x] if simples(x) => {
            FormaDoHospedeiro::Atributo(x.to_string(), saneador("div", propriedade_mapeada(x)))
        }
        ["style", x] | ["style", x, _] if simples(x) => FormaDoHospedeiro::Estilo {
            nome: x.to_string(),
            unidade: partes.get(2).map(|u| u.to_string()),
            texto: false,
            nulo: false,
        },
        [p] if simples(p) => {
            let mapeada = propriedade_mapeada(p);
            if mapeada == "className" {
                return Err(fora());
            }
            FormaDoHospedeiro::Propriedade(mapeada.to_string(), saneador("div", mapeada))
        }
        _ => return Err(fora()),
    })
}

/// [`forma_do_hospedeiro`] for the component, which also writes the whole class
/// (`class`, `className`, `attr.class`) through its view.
fn forma_do_hospedeiro_de_componente(nome: &str) -> Result<FormaDoHospedeiro, String> {
    if matches!(nome, "class" | "className" | "attr.class") {
        return Ok(FormaDoHospedeiro::ClasseInteira);
    }
    forma_do_hospedeiro(nome)
}

/// The component's `@HostBinding`s in the order of the official
/// `hostProperties` map. Without inheritance, those of the class itself
/// ([`Componente`]); with inheritance, those of the metadata read from the
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
fn ligacoes_do_componente(
    c: &Componente,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> Result<Vec<LigacaoDoHospedeiro>, Recusa> {
    let fora = |f: &str| recusa(Motivo::HostBindingEmComponente, f.to_string());
    let lista: Vec<(String, String)> = if c.herda {
        match local.metadados.as_deref() {
            Some(m) if m.fora.is_empty() => m.ligacoes_do_hospedeiro.clone(),
            _ if c.liga_hospedeiro => {
                return Err(fora("@HostBinding on an inheriting component"));
            }
            _ => Vec::new(),
        }
    } else {
        c.ligacoes_do_hospedeiro
            .iter()
            .map(|l| (l.nome.clone(), l.membro.clone()))
            .collect()
    };
    let mut saida = Vec::new();
    for (nome, membro) in lista {
        let mut forma = forma_do_hospedeiro_de_componente(&nome).map_err(|f| fora(&f))?;
        let proprio = c.ligacoes_do_hospedeiro.iter().find(|l| l.membro == membro);
        let imutavel = match proprio {
            Some(l) => l.imutavel,
            None => resolvedor
                .and_then(|r| r.membro_final(local.caminho, &c.classe, &membro))
                .ok_or_else(|| fora("inherited @HostBinding without a readable declaration"))?,
        };
        if let FormaDoHospedeiro::Estilo { texto, nulo, .. } = &mut forma {
            if imutavel {
                return Err(fora("@HostBinding('style.x') on a final field"));
            }
            let tipo = resolvedor
                .and_then(|r| r.tipo_do_membro(local.caminho, &c.classe, &membro))
                .map(|(t, _)| t)
                .or_else(|| c.membros.get(&membro).map(|m| m.tipo.clone()))
                .unwrap_or_default();
            let base = tipo.trim().trim_end_matches('?');
            if base.is_empty() || matches!(base, "dynamic" | "var" | "Object" | "Never") {
                return Err(fora("@HostBinding('style.x') of unknown type"));
            }
            *texto = base == "String";
            // `canBeNull` de `_ctx.x`: sempre (`analyzed_class.dart:210-221`).
            *nulo = true;
        }
        saida.push(LigacaoDoHospedeiro {
            membro,
            imutavel,
            estatico: proprio.is_some_and(|l| l.estatico),
            forma: match forma {
                FormaDoHospedeiro::Atributo(x, _)
                    if proprio.is_some_and(|l| l.estatico)
                        && matches!(x.as_str(), "tabindex" | "tabIndex") =>
                {
                    FormaDoHospedeiro::TabIndex
                }
                f => f,
            },
        });
    }
    Ok(saida)
}

#[allow(clippy::too_many_arguments)]
/// The emission of a component. The `text_binding.dart` import is allocated
/// before the body, at the position of the class's first `TextBinding` field;
/// when no interpolation of the root view is mutable (all in `build()`,
/// `isImmutable`), the field does not exist and the emission is redone without
/// it, from the same import table (case j91).
#[allow(clippy::too_many_arguments)]
fn gerar_componente(
    c: &Componente,
    local: &Local,
    nos: &[No],
    resolvedor: Option<&dyn Resolucao>,
    nomes: &mut dartforge_intern::Interner,
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    pipes: &Result<Vec<PipeUsado>, Recusa>,
    coleta: &mut Option<Vec<Recusa>>,
    imp: &mut Importacoes,
) -> Result<String, Recusa> {
    let (imp_antes, coleta_antes) = (imp.clone(), coleta.clone());
    let (texto, tb_sobrando) = gerar_componente_com(
        c, local, nos, resolvedor, nomes, filhos, usadas, pipes, coleta, imp, true,
    )?;
    if !tb_sobrando {
        return Ok(texto);
    }
    *imp = imp_antes;
    *coleta = coleta_antes;
    gerar_componente_com(
        c, local, nos, resolvedor, nomes, filhos, usadas, pipes, coleta, imp, false,
    )
    .map(|(t, _)| t)
}

#[allow(clippy::too_many_arguments)]
fn gerar_componente_com(
    c: &Componente,
    local: &Local,
    nos: &[No],
    resolvedor: Option<&dyn Resolucao>,
    nomes: &mut dartforge_intern::Interner,
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    pipes: &Result<Vec<PipeUsado>, Recusa>,
    coleta: &mut Option<Vec<Recusa>>,
    imp: &mut Importacoes,
    com_tb: bool,
) -> Result<(String, bool), Recusa> {
    let nos = &template_como_container(nos);
    // Anota na coleta ou interrompe.
    fn anotar(coleta: &mut Option<Vec<Recusa>>, r: Recusa) -> Result<(), Recusa> {
        match coleta {
            Some(v) => {
                v.push(r);
                Ok(())
            }
            None => Err(r),
        }
    }
    for r in &c.nao_entendidos {
        anotar(coleta, r.clone())?;
    }
    for r in formas_contra_o_template(c, local, nos, resolvedor, filhos, usadas) {
        anotar(coleta, r)?;
    }
    // Unused `pipes:` do not change the view (case b19).
    let tabela = match pipes_do_template(nos, filhos, pipes, &local.asset()) {
        Ok(t) => t,
        Err(r) => {
            anotar(coleta, r)?;
            PipesDoTemplate::default()
        }
    };
    if c.estilos_ilegiveis {
        anotar(
            coleta,
            recusa(
                Motivo::Estilos,
                "styleUrls/styles with an item that is not a literal string",
            ),
        )?;
    }
    // `styles: ['…']` written in the annotation: each text goes into the
    // `styles$X` list after the `styleUrls` stylesheets (`_compileStyles`),
    // shimmed under emulated encapsulation and as is under `none` (cases j07,
    // j08).
    let mut em_linha = Vec::new();
    for texto in &c.styles {
        if texto.contains("@import") {
            anotar(coleta, recusa(Motivo::Estilos, "styles: [..] com @import"))?;
            continue;
        }
        if c.sem_encapsulamento {
            em_linha.push(literal(texto));
            continue;
        }
        match crate::css::shim(texto) {
            Ok(t) => em_linha.push(literal(&t)),
            Err(_) => anotar(
                coleta,
                recusa(
                    Motivo::Estilos,
                    "CSS the ngdart shim refuses (the official compiler throws)",
                ),
            )?,
        }
    }
    // `ViewEncapsulation.none` with a stylesheet: the template imports the
    // `.css.dart` (the stylesheet without the shim, from `gerar_folha`),
    // `ComponentStyles.unscoped` and no `addShimC` (case i88).
    // The component's `@HostBinding`s, with the inherited ones.
    let do_hospedeiro = match ligacoes_do_componente(c, local, resolvedor) {
        Ok(l) => l,
        Err(r) => {
            anotar(coleta, r)?;
            Vec::new()
        }
    };
    // The construction comes out after the fixed imports, because injection
    // allocates its own (`errors.dart` and that of each injected type) at the
    // end of the table.
    if let Some(r) = falta_para_construir(c, local, resolvedor) {
        anotar(coleta, r)?;
    }
    // The component's `providers:`: the host view creates them; with
    // `Visibility.all`, it also hands the component out through
    // `injectorGetInternal`.
    if c.visibilidade_escrita && local.metadados.is_none() {
        anotar(
            coleta,
            recusa(
                Motivo::Providers,
                "visibility: without the program metadata",
            ),
        )?;
    }
    let visivel = local.metadados.as_ref().is_some_and(|m| m.visivel);
    let no_hospedeiro = if c.com_provedores || visivel {
        match provedores_da_hospedeira(local) {
            Ok(r) => Some(r),
            Err(r) => {
                anotar(coleta, r)?;
                None
            }
        }
    } else {
        None
    };
    let tokens_do_no: Vec<crate::directives::Token> = no_hospedeiro
        .iter()
        .flat_map(|r| &r.instancias)
        .flat_map(|i| std::iter::once(i.token.clone()).chain(i.apelidos.iter().cloned()))
        .collect();
    let consultas_hosp = match consultas_da_hospedeira(c, local, resolvedor, &tokens_do_no) {
        Ok(s) => s,
        Err(r) => {
            anotar(coleta, r)?;
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
    let mut estilo = Vec::new();
    for url in &c.style_urls {
        match local.uri_do_estilo(url, !c.sem_encapsulamento) {
            Some(uri) => estilo.push(imp.alias(&uri)),
            None => anotar(coleta, recusa(Motivo::Estilos, "stylesheet outside lib/"))?,
        }
    }
    // Generic component: the view, the host view, the embedded views and the
    // factories take the class's type parameters. The written bound (`T extends
    // num`) is the first type in the header of the view class: its import is
    // allocated before `ComponentView`.
    let (genericos, genericos_decl) = match parametros_de_tipo(c, local, resolvedor, imp) {
        Ok(g) => g,
        Err(r) => {
            anotar(coleta, r)?;
            Default::default()
        }
    };
    let vista = imp.alias(COMPONENT_VIEW);
    let proprio = imp.alias(local.arquivo);
    // The view's fields come before everything in the class — text bindings,
    // child views, previous values, elements —, and the imports are allocated
    // in that same order. That is what makes the numbering match the official
    // one; out of order, the byte-for-byte comparison is worthless.
    // `@i18n`: `package:intl` goes in with the first `_message_N` field, before
    // or after `text_binding.dart` according to document order. With a child,
    // directive or `*` in the view, the order of the fields (and the imports)
    // among them has no case yet.
    alocar_imports_preguicosos(imp, nos, filhos, usadas, &local.asset());
    let ordem_i18n = i18n_antes_da_interpolacao(nos);
    let mut intl = None;
    if ordem_i18n == Some(true) {
        intl = Some(imp.alias(INTL));
    }
    let tb = (com_tb && tem_interpolacao(nos)).then(|| imp.alias(TEXT_BINDING));
    if ordem_i18n == Some(false) {
        intl = Some(imp.alias(INTL));
    }
    if ordem_i18n.is_some() && !campos_em_ordem(nos, filhos, usadas, &local.asset()).is_empty() {
        anotar(
            coleta,
            recusa(Motivo::I18n, "@i18n with a child, directive or `*` in the view"),
        )?;
    }
    let refs_candidatos = referencias_candidatas(nos, c);
    let refs_locais = referencias_locais(nos, filhos, &refs_candidatos, &[]);
    // The node of this view that is the result of a dynamic query is read
    // during detection (the list `[this._el_0, ...]`): it becomes a field, like
    // the one an expression reads (case j17).
    let consultados_da_raiz: Vec<(String, String, u32)> = c
        .consultas
        .iter()
        .enumerate()
        .filter(|(_, q)| !q.por_tipo && consulta_em_embutida(nos, q, filhos, local, resolvedor))
        .filter(|(_, q)| {
            arvore_da_consulta(nos, &q.referencia, filhos).is_ok_and(|a| tem_resultado_estatico(&a))
        })
        .map(|(i, q)| {
            (
                q.referencia.clone(),
                format!("_viewQuery_{}_{i}_isDirty", q.referencia),
                0,
            )
        })
        .collect();
    // `directiveTypes:`: the class in the component's scope and the arguments
    // written like `fromTypeLink` (`#T` is the component's parameter).
    let tipos_de_diretiva = {
        let livres: Vec<&str> = c
            .parametros_de_tipo
            .iter()
            .map(|(n, _)| n.as_str())
            .collect();
        let mut v = Vec::new();
        for t in &c.tipos_de_diretiva {
            let falha = || {
                recusa(
                    Motivo::NaoEntendido,
                    format!("directiveTypes: Typed<{}> without resolution", t.classe),
                )
            };
            let r = resolvedor.ok_or_else(falha)?;
            let uri = r.uri_do_tipo(local.caminho, &t.classe).ok_or_else(falha)?;
            let simples = t.classe.rsplit('.').next().unwrap_or(&t.classe).to_string();
            if !usadas.iter().any(|u| u.uri == uri && u.classe == simples) {
                return Err(recusa(
                    Motivo::NaoEntendido,
                    "directiveTypes: with a directive outside directives: (error in the official compiler)",
                ));
            }
            let mut escritos = Vec::new();
            for a in &t.argumentos {
                escritos.push(
                    tipo_qualificado(a, local.caminho, r, &local.asset(), &livres)
                        .ok_or_else(falha)?,
                );
            }
            v.push(TipoDeDiretivaResolvido {
                uri,
                classe: simples,
                argumentos: if escritos.is_empty() {
                    String::new()
                } else {
                    format!("<{}>", escritos.join(", "))
                },
                em: t.em.clone(),
            });
        }
        v
    };
    let mut promovidos = refs_locais.clone();
    promovidos.extend(consultados_da_raiz.iter().map(|(n, _, _)| n.clone()));
    if let Err(r) = alocar_imports_dos_campos(
        imp,
        nos,
        filhos,
        usadas,
        &local.asset(),
        &tabela.imports_dos_campos(0),
        &nos_promovidos(nos, &promovidos),
        &tipos_de_diretiva,
    ) {
        anotar(coleta, r)?;
    }
    let estilos = imp.alias(STYLE_ENCAPSULATION);
    let view = imp.alias(VIEW);
    let cd = imp.alias(CHANGE_DETECTION);
    let util = imp.alias(UTILITIES);
    // The view constructor uses `document.createElement`, so `dart:html` (in 9,
    // the `dom.dart` of `package:web`) always goes in before the body of
    // `build()`.
    let documento = imp.dom("document");
    // The static `@HostBinding`s come out in the constructor, right after
    // `rootElement`: their imports come before those of `build()`.
    let estaticos_no_construtor: String = do_hospedeiro
        .iter()
        .filter(|l| l.estatico)
        .map(|l| {
            let valor = format!("{proprio}.{}.{}", c.classe, l.membro);
            format!("\n    {};", resolver_tardios(imp, &l.acao(&valor)))
        })
        .collect();

    // `exports:`: each name through the import of the library that declares it,
    // allocated when the expression is written (case j51).
    let mut exportados = Exportados::new();
    if let Some(r) = resolvedor {
        for nome in c.exportados.iter().filter(|n| !n.contains('.')) {
            let Some((uri, o_que)) = r.exportado(local.caminho, nome) else {
                continue;
            };
            let Some(caminho) = asset_de_uri(&uri, local.pacote, local.raiz)
                .and_then(|a| caminho_do_import(&local.asset(), &a))
            else {
                continue;
            };
            exportados.insert(nome.clone(), (tardio_q(&caminho), o_que));
        }
    }
    let ctx = Contexto {
        membros: &c.membros,
        exportados: &exportados,
        nomes_genericos: c
            .parametros_de_tipo
            .iter()
            .map(|(n, _)| n.clone())
            .collect(),
        tokens_de_visao: local
            .metadados
            .as_deref()
            .map(|m| {
                m.provedores_de_visao
                    .iter()
                    .map(|p| p.token.clone())
                    .collect()
            })
            .unwrap_or_default(),
        metodos: &c.metodos,
        aridades: &c.aridades,
        filhos,
        usadas,
        asset: local.asset(),
        tipos: resolvedor.map(|r| (r, local.caminho)),
        classe_qualificada: format!("{proprio}.{}", c.classe),
        com_estilo: (!c.style_urls.is_empty() || !c.styles.is_empty()) && !c.sem_encapsulamento,
        url_do_template: local.url_do_template.clone(),
        classe_da_visao: format!("View{}", c.classe),
        tipo_do_contexto: format!("{proprio}.{}{genericos}", c.classe),
        genericos,
        genericos_decl,
        preservar_espacos: c.preservar_espacos,
        pipes: &tabela,
        refs_ambiguos: referencias_ambiguas(nos),
        refs_candidatos,
        leituras: c
            .consultas
            .iter()
            .filter_map(|q| {
                Some((
                    chave_da_consulta(q, local, resolvedor)?,
                    token_de_leitura(q, local, resolvedor)?,
                ))
            })
            .collect(),
        refs_so_por_provedor: c
            .consultas
            .iter()
            .filter(|q| !q.por_tipo)
            .map(|q| q.referencia.clone())
            .filter(|r| {
                c.consultas
                    .iter()
                    .filter(|q| !q.por_tipo && q.referencia == *r)
                    .all(|q| token_de_leitura(q, local, resolvedor).is_some())
            })
            .collect(),
        moldes_com_container: {
            let moldes = referencias_de_moldes(nos);
            c.consultas
                .iter()
                .filter(|q| {
                    !q.por_tipo
                        && moldes.contains(&q.referencia)
                        && le_container(q, local, resolvedor)
                })
                .map(|q| q.referencia.clone())
                .collect()
        },
        link_de_deteccao: c.link_de_deteccao,
        tipos_de_diretiva,
        refs_resolvidos: Default::default(),
        ancoras_de_consulta: Default::default(),
        conteudo_dinamico: Default::default(),
        sujas_de_conteudo: Default::default(),
    };
    let mut corpo = ctx.corpo(imp, nomes, coleta.take(), false);
    corpo.refs_livres = referencias_livres(nos);
    corpo.refs_de_embutidas = refs_locais
        .iter()
        .filter(|n| citado_em_embutidas(nos, n, filhos))
        .cloned()
        .collect();
    corpo.refs_so_em_eventos = refs_locais
        .iter()
        .filter(|n| !lido_na_deteccao(nos, n))
        .cloned()
        .collect();
    corpo.declarar_refs(refs_locais);
    corpo.refs_consultados = consultados_da_raiz;
    corpo.consultas_dinamicas = c
        .consultas
        .iter()
        .enumerate()
        .filter(|(_, q)| consulta_em_embutida(nos, q, filhos, local, resolvedor))
        .map(|(i, q)| ConsultaDinamica {
            indice: i,
            propriedade: q.propriedade.clone(),
            lista: q.lista,
            chave: chave_da_consulta(q, local, resolvedor).unwrap_or_default(),
            leitura: {
                let chave = chave_da_consulta(q, local, resolvedor).unwrap_or_default();
                match token_de_leitura(q, local, resolvedor) {
                    Some(t) => chave_de_leitura(&chave, &t),
                    None => chave,
                }
            },
            campo: format!("_viewQuery_{}_{i}_isDirty", q.referencia),
            element_ref: q.leitura.is_some()
                && valor_de_elemento(q, local, resolvedor) == Some(ValorDeElemento::ElementRef),
            arvore: chave_da_consulta(q, local, resolvedor)
                .and_then(|k| arvore_da_consulta(nos, &k, filhos).ok())
                .unwrap_or_default(),
            vista: false,
        })
        .collect();
    // The "dirty" fields open the class, in the order the official compiler
    // allocates them ([`primeiro_resultado_dinamico`]; on a tie, the order of
    // the queries).
    let mut sujos: Vec<(Option<(usize, usize)>, usize, &str)> = corpo
        .consultas_dinamicas
        .iter()
        .map(|d| {
            (
                primeiro_resultado_dinamico(nos, &d.chave, filhos),
                d.indice,
                d.campo.as_str(),
            )
        })
        .collect();
    sujos.sort_by_key(|(posicao, indice, _)| (posicao.is_none(), *posicao, *indice));
    let sujos: Vec<(usize, String)> = sujos
        .into_iter()
        .map(|(_, _, campo)| {
            let posicao = corpo
                .consultas_dinamicas
                .iter()
                .find(|d| d.campo == campo)
                .and_then(|d| inicio_do_primeiro_resultado_dinamico(nos, &d.chave, filhos))
                .unwrap_or(usize::MAX);
            (posicao, format!("  bool {campo} = true;"))
        })
        .collect();
    corpo.sujos_de_visao = sujos;
    // `@ViewChild(ren)(Token)` provided by a directive, with a result in a `*`:
    // through the content query machinery, with the root in this view.
    for (i, q) in c.consultas.iter().enumerate() {
        let Some(arvore) = consulta_de_token_dinamica(nos, q, filhos, usadas, local, resolvedor)
        else {
            continue;
        };
        let (Some(uri), Some(classe)) = (
            uri_da_consulta(q, local, resolvedor),
            q.referencia.rsplit('.').next().map(str::to_string),
        ) else {
            continue;
        };
        corpo.conteudo_dinamico_no(
            arvore,
            chave_de_tipo(&uri, &classe),
            &classe,
            0,
            i,
            q.lista,
            format!("_ctx.{}", q.propriedade),
            true,
        );
        corpo.consultas_por_token.insert(i);
    }
    // Only messages with HTML: `intl` is requested by the method, later.
    corpo.intl = intl.or_else(|| contem_anotacao(nos).then(String::new));
    corpo.tb = tb;
    let r = corpo
        .nos(nos, "parentRenderNode")
        .and_then(|()| corpo.conferir_pipes());
    if let Err(r) = r {
        *coleta = corpo.coleta.take();
        return Err(r);
    }
    exportar_refs(
        &ctx.refs_resolvidos,
        &corpo.refs,
        &corpo.detectores,
        &corpo.refs_em_ordem,
        &corpo.detectores_em_ordem,
        &corpo.classe_desta_visao(),
    );
    // The pre-allocated `text_binding.dart` import without a field that uses
    // it.
    let tb_sobrando = corpo.tb.is_some() && !corpo.tb_usado && !corpo.coletando();
    // The pipes' `ngOnDestroy` comes after those of the directives.
    corpo.destruir.extend(tabela.destruicao(0));
    // A static `@ViewChild`: immediate assignment, in `afterNodes` — after the
    // listeners, in the declaration order of the queries
    // (`updateQueryAtStartup`, `createImmediateUpdates` in
    // `compile_query.dart`). `formas_contra_o_template` has already ensured
    // that each `#ref` appears only once, on an HTML element of the view
    // itself.
    let moldes = referencias_de_moldes(nos);
    let mut consultas = Vec::new();
    for (i, q) in c.consultas.iter().enumerate() {
        // The dynamic one comes out in the detection.
        if corpo.consultas_dinamicas.iter().any(|d| d.indice == i)
            || corpo.consultas_por_token.contains(&i)
        {
            continue;
        }
        // What to look for among the results: the `#ref`, or the type key.
        let chave = if q.por_tipo {
            uri_da_consulta(q, local, resolvedor)
                .map(|u| chave_de_tipo(&u, &q.referencia))
                .unwrap_or_default()
        } else {
            q.referencia.clone()
        };
        // With a provider `read:`, its instance on each node; with `read:
        // ViewContainerRef` on a `<template>`, its `ViewContainer`.
        let chave = match token_de_leitura(q, local, resolvedor) {
            Some(t) => chave_de_leitura(&chave, &t),
            None if !q.por_tipo
                && moldes.contains(&q.referencia)
                && le_container(q, local, resolvedor) =>
            {
                chave_de_container(&chave)
            }
            None => chave,
        };
        if q.lista {
            let valores: Vec<String> = corpo
                .refs_em_ordem
                .iter()
                .filter(|(n, _)| *n == chave)
                .map(|(_, v)| v.clone())
                .collect();
            // An `onPush` child in the result would register the
            // `ChangeDetectorRef` of each one: not yet.
            if corpo.detectores.contains_key(&chave) {
                let r = recusa(Motivo::ViewChildEmFilho, "@ViewChildren of an onPush child");
                if corpo.coletando() {
                    corpo.anotar(r)?;
                } else {
                    *coleta = corpo.coleta.take();
                    return Err(r);
                }
            }
            let valores: Vec<String> = valores
                .into_iter()
                .map(|v| elemento_com_cast(v, q, local, resolvedor))
                .collect();
            consultas.push(format!(
                "    _ctx.{} = [{}];",
                q.propriedade,
                valores.join(", ")
            ));
            corpo.usa_ctx_no_build = true;
            continue;
        }
        // O primeiro resultado em ordem de documento (`#ref` repetido).
        let primeiro = corpo
            .refs_em_ordem
            .iter()
            .find(|(n, _)| *n == chave)
            .map(|(_, v)| v.clone());
        match primeiro {
            Some(alvo) => {
                // An `onPush` child: its `ChangeDetectorRef` is registered
                // (`_createAddQueryChangeDetectorRefs`). With a repeated
                // `#ref`, the registration kept may not be that of the first:
                // not yet.
                let repetido = corpo
                    .refs_em_ordem
                    .iter()
                    .filter(|(n, _)| *n == chave)
                    .count()
                    > 1;
                if repetido && corpo.detectores.contains_key(&chave) {
                    let r = recusa(
                        Motivo::ViewChildEmFilho,
                        "@ViewChild of a repeated #ref on an onPush child",
                    );
                    if corpo.coletando() {
                        corpo.anotar(r)?;
                    } else {
                        *coleta = corpo.coleta.take();
                        return Err(r);
                    }
                }
                if let Some(cv) = corpo.detectores.get(&chave).cloned() {
                    let v = corpo.imp.alias(VIEW);
                    consultas.push(format!(
                        "    {v}.View.queryChangeDetectorRefs[{alvo}] = this.{cv};"
                    ));
                }
                // `ElementRef(node)` for the element read like this; the import
                // comes out in text order, after the listeners and the pipes.
                let mut lugares = Vec::new();
                onde_esta(nos, &q.referencia, filhos, false, &mut lugares);
                let elemento = matches!(lugares.first(), Some(Lugar::Raiz | Lugar::Projetado));
                // With `#ref="x"`, the value is the exported instance, not the
                // node.
                let alvo = if !q.por_tipo
                    && elemento
                    && !moldes.contains(&q.referencia)
                    && !referencia_com_valor(nos, &q.referencia)
                    && valor_de_elemento(q, local, resolvedor) == Some(ValorDeElemento::ElementRef)
                {
                    format!("{}ElementRef({alvo})", tardio_q(ELEMENT_REF))
                } else {
                    elemento_com_cast(alvo, q, local, resolvedor)
                };
                consultas.push(format!("    _ctx.{} = {alvo};", q.propriedade));
            }
            // In collection mode the refusal already came from
            // `formas_contra_o_template`.
            None if corpo.coletando() => {}
            // No `#ref` with that name in the template: the single one receives
            // nothing (case j82).
            // Without an assignment, `build()` does not read `_ctx` for it.
            None if !q.por_tipo && {
                let mut l = Vec::new();
                onde_esta(nos, &q.referencia, filhos, false, &mut l);
                l.is_empty()
            } => continue,
            None => {
                return Err(recusa(
                    Motivo::ViewChildDinamico,
                    "@ViewChild without a #ref in the template",
                ));
            }
        }
        corpo.usa_ctx_no_build = true;
    }
    // The element listeners are written after the nodes: their imports go in
    // now.
    let ouvintes: Vec<String> = corpo
        .ouvintes
        .iter()
        .map(|o| resolver_tardios(corpo.imp, o))
        .collect();
    // The pipes: instance and proxies, in `afterNodes`, before the queries.
    let criacao_pipes: Vec<String> = tabela
        .criacao(0, "this", corpo.imp)
        .iter()
        .map(|l| resolver_tardios(corpo.imp, l))
        .collect();
    let consultas: Vec<String> = consultas
        .iter()
        .map(|l| resolver_tardios(corpo.imp, l))
        .collect();
    // The component's `@HostListener`s close `build()`, bound to the root node
    // (`_writeComponentHostEventListeners`, after `writeBuildStatements` in
    // `_generateBuildMethod`). The handler goes through the same converter as
    // the template events, and a complex one gets the next `_handleEvent_N`.
    // For an inheriting component: those of the supertypes too, from the
    // metadata (supertypes first; the same event keeps the last one, in the
    // place of the first).
    let ouvintes_do_hospedeiro: Vec<crate::component::Ouvinte> = if c.herda {
        match local.metadados.as_deref() {
            Some(m) if m.fora.is_empty() => {
                if m.ouvintes.iter().any(|o| !evento_nativo(&o.evento)) {
                    corpo.anotar(recusa(
                        Motivo::HostListenerEmComponente,
                        "inherited @HostListener of a non-native event",
                    ))?;
                }
                m.ouvintes
                    .iter()
                    .map(|o| crate::component::Ouvinte {
                        evento: o.evento.clone(),
                        handler: format!("{}({})", o.metodo, o.args),
                    })
                    .collect()
            }
            _ => {
                corpo.anotar(recusa(
                    Motivo::HostListenerEmComponente,
                    "@HostListener of an inheriting component without the metadata",
                ))?;
                c.ouvintes.clone()
            }
        }
    } else {
        c.ouvintes.clone()
    };
    let mut hospedeiro = Vec::new();
    for o in &ouvintes_do_hospedeiro {
        corpo.tipo_do_evento_falha = None;
        corpo.tipo_do_evento = tipo_de_evento_nativo(&o.evento);
        match corpo.handler(&[&o.handler]) {
            // As marcas do `package:web` no 9, na ordem do texto.
            Ok(h) => hospedeiro.push(resolver_tardios(
                corpo.imp,
                &format!(
                    "    parentRenderNode.addEventListener('{}', {});",
                    o.evento,
                    if evento_nativo(&o.evento) {
                        ouvinte_nativo(&o.evento, &h)
                    } else {
                        h.clone()
                    }
                ),
            )),
            Err(r) => {
                let r = r.em(Motivo::HostListenerEmComponente);
                if corpo.coletando() {
                    corpo.anotar(r)?;
                } else {
                    *coleta = corpo.coleta.take();
                    return Err(r);
                }
            }
        }
    }
    let ctx_no_build = if corpo.usa_ctx_no_build {
        "\n    final _ctx = this.ctx;"
    } else {
        ""
    };
    // Order of the official `build()` (`_generateBuildMethod`): the nodes (the
    // `_buildView` phase), the listeners (`bindView`) and, finally, what
    // `afterNodes` adds.
    let linhas = corpo
        .linhas
        .iter()
        .chain(&ouvintes)
        .chain(&criacao_pipes)
        .chain(&consultas)
        .cloned()
        .chain((corpo.subscricoes > 0).then(|| {
            let subs: Vec<String> = (0..corpo.subscricoes)
                .map(|k| format!("subscription_{k}"))
                .collect();
            format!("    this.initSubscriptions([{}]);", subs.join(", "))
        }))
        // The component's `@HostListener`s after `initSubscriptions`
        // (`_writeComponentHostEventListeners`, `view_builder.dart:840-852`).
        .chain(hospedeiro.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n");
    let corpo_build = if linhas.is_empty() {
        String::new()
    } else {
        format!("\n{linhas}")
    };
    // The component's `@HostBinding`: the view's
    // `detectHostChanges(firstCheck)` (`bindAndWriteToRenderer` with
    // `isHtmlElement` false — hence `updateClassBindingNonHtml`), with the
    // binding indices after those of the template and the immutable ones first,
    // in `if (firstCheck)`. `checkBinding` takes `null, null`: the binding has
    // no template text.
    let host_changes = if !do_hospedeiro.iter().any(|l| !l.estatico) {
        String::new()
    } else {
        let chk = tardio(CHECK_BINDING);
        let mut constantes = Vec::new();
        let mut dinamicas = Vec::new();
        for l in do_hospedeiro.iter().filter(|l| !l.estatico) {
            let k = corpo.proxima_ligacao;
            corpo.proxima_ligacao += 1;
            let m = &l.membro;
            if l.imutavel {
                constantes.push(format!(
                    "      if ((_ctx.{m} != null)) {{\n        {};\n      }}",
                    l.acao(&format!("_ctx.{m}"))
                ));
            } else {
                corpo.campos_expr.push(format!("  Object? _expr_{k};"));
                dinamicas.push(format!(
                    "    final currVal_{k} = _ctx.{m};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, null, null)) {{\n      {};\n      this._expr_{k} = currVal_{k};\n    }}",
                    l.acao(&format!("currVal_{k}"))
                ));
            }
        }
        let mut linhas = vec!["    final _ctx = this.ctx;".to_string()];
        if !constantes.is_empty() {
            linhas.push(format!(
                "    if (firstCheck) {{\n{}\n    }}",
                constantes.join("\n")
            ));
        }
        linhas.extend(dinamicas);
        format!(
            "\n  void detectHostChanges(bool firstCheck) {{\n{}\n  }}\n",
            linhas.join("\n")
        )
    };
    // Order of the fields in the class, as the official compiler writes it:
    // text bindings, then the previous values of the bindings, then the
    // elements.
    let especs = std::mem::take(&mut corpo.embutidas);
    let mut todos = campos_com_inicializador(&corpo);
    todos.extend(corpo.campos.clone());
    todos.extend(corpo.campos_filho.clone());
    todos.extend(campos_da_deteccao(
        &corpo.campos_expr,
        &corpo.campos_el_de_embutidas,
        &corpo.posicao_de_embutida,
    ));
    todos.extend(tabela.campos(0, corpo.imp));
    let mut consultados = corpo.campos_el_consultados.clone();
    consultados.sort_by_key(|(k, _)| *k);
    todos.extend(consultados.into_iter().map(|(_, c)| c));
    todos.extend(campos_el_em_ordem(
        &corpo.campos_el,
        &corpo.refs_dos_campos_el,
        &corpo.locais_raiz,
    ));
    todos.extend(corpo.campos_el_por_evento.clone());
    todos.extend(corpo.campos_el_de_eventos.iter().map(|(_, c)| c.clone()));
    let campos = if todos.is_empty() {
        String::new()
    } else {
        format!("{}\n", todos.join("\n"))
    };
    // The block of the content hooks (`_updateContentQueriesMethod`): the
    // dynamic content queries, in each node's `afterChildren`; the view ones,
    // in `afterNodes` (`compile_view.dart:502-511`), in query order; then the
    // hooks.
    let mut apos_conteudo = corpo.atualizacoes_de_conteudo.clone();
    let mut de_visao: Vec<(usize, String)> = corpo.atualizacoes_de_visao.clone();
    for d in &corpo.consultas_dinamicas {
        if !d.vista {
            continue;
        }
        // The value can only be assembled with the anchors of all views, after
        // the nested ones are emitted ([`resolver_consultas`]); the qualifiers
        // go in the mark, in the order they appear in the text.
        let mut marca = format!("{MARCA_DE_CONSULTA}{}", d.campo);
        if !d.lista && !tem_resultado_estatico(&d.arvore) {
            marca += &format!("|q{}", tardio_q(QUERIES));
        }
        if tem_detector(&d.arvore) {
            marca += &format!("|d{}", tardio_q(VIEW));
        }
        if d.element_ref {
            marca += &format!("|e{}", tardio_q(ELEMENT_REF));
        }
        marca.push(FIM_DE_CONSULTA);
        de_visao.push((
            d.indice,
            format!(
                "    if (this.{campo}) {{\n      {marca}\n      this.{campo} = false;\n    }}",
                campo = d.campo
            ),
        ));
    }
    de_visao.sort_by_key(|(i, _)| *i);
    apos_conteudo.extend(de_visao.into_iter().map(|(_, l)| l));
    apos_conteudo.extend(corpo.apos_conteudo.iter().cloned());
    let corpo_consultas = corpo.consultas_dinamicas.clone();
    let classe_raiz = corpo.classe_desta_visao();
    // The detection in the order of `writeChangeDetectionStatements`: inputs of
    // directives and children, nested views, property and text bindings, child
    // views. `_ctx` and `firstCheck` are only declared if someone reads them
    // (`maybeCachedCtxDeclarationStatement`).
    let mut linhas_deteccao = corpo.entradas.clone();
    for a in &corpo.ancoras {
        linhas_deteccao.push(format!("    this.{a}.detectChangesInNestedViews();"));
    }
    linhas_deteccao.extend(sem_lancar(&apos_conteudo));
    linhas_deteccao.extend(corpo.deteccao.iter().cloned());
    for v in &corpo.vistas_filhas {
        linhas_deteccao.push(format!("    this.{v}.detectChanges();"));
    }
    linhas_deteccao.extend(sem_lancar(&corpo.apos_visao));
    let deteccao = if linhas_deteccao.is_empty() {
        String::new()
    } else {
        // The dynamic query writes `_ctx.x = ...` when it is resolved
        // ([`resolver_consultas`]), after this computation.
        let ctx_det = if cita_ctx(&linhas_deteccao)
            || (corpo.consultas_dinamicas.iter().any(|d| d.vista)
                || !corpo.atualizacoes_de_visao.is_empty())
        {
            "    final _ctx = this.ctx;\n"
        } else {
            ""
        };
        let primeira = if corpo.usa_primeira_checagem {
            "    bool firstCheck = this.firstCheck;\n"
        } else {
            ""
        };
        let mudou = if corpo.usa_changed {
            "    bool changed = false;\n"
        } else {
            ""
        };
        // The locals read by the detection (the `#ref`s), in order of first
        // use, after `_ctx`, as in the embedded view.
        let locais: String = corpo
            .locais_raiz
            .iter()
            .filter_map(|nome| match corpo.decl_locais.get(nome) {
                Some(Ok(d)) => Some(format!("    {d}\n")),
                _ => None,
            })
            .collect();
        format!(
            "\n  @override\n  void detectChangesInternal() {{\n{ctx_det}{mudou}{primeira}{locais}{}\n  }}\n",
            linhas_deteccao.join("\n")
        )
    };
    let deteccao = resolver_refs(&deteccao, &ctx.refs_resolvidos.borrow());
    // `injectorGetInternal` comes after `build()` and before the detection.
    let injetor = resolver_tardios(corpo.imp, &metodo_injetor(&corpo.injetores, &corpo.asset));
    // The imports of the detection go in now, after those of `build()`.
    let deteccao = resolver_tardios(corpo.imp, &deteccao);
    let deteccao = format!(
        "{}{deteccao}",
        metodo_de_link(ctx.link_de_deteccao, &corpo.ancoras, &corpo.vistas_ligadas)
    );
    // A child view must be destroyed with the view that created it.
    let destruicao = if corpo.vistas_filhas.is_empty()
        && corpo.ancoras.is_empty()
        && corpo.destruir.is_empty()
    {
        String::new()
    } else {
        let mut linhas: Vec<String> = corpo
            .ancoras
            .iter()
            .map(|a| format!("    this.{a}.destroyNestedViews();"))
            .collect();
        linhas.extend(
            corpo
                .vistas_filhas
                .iter()
                .map(|v| format!("    this.{v}.destroyInternalState();")),
        );
        linhas.extend(corpo.destruir.iter().cloned());
        format!(
            "\n  @override\n  void destroyInternal() {{\n{}\n  }}\n",
            linhas.join("\n")
        )
    };
    // The `_handleEvent_N`s, after `destroyInternal`.
    if !corpo.metodos_i18n.is_empty() && !corpo.metodos_evento.is_empty() {
        let r = recusa(
            Motivo::I18n,
            "@i18n with HTML and an event handler in the same view",
        );
        *coleta = corpo.coleta.take();
        return Err(r);
    }
    let metodos: String = corpo
        .metodos_i18n
        .iter()
        .chain(&corpo.metodos_evento)
        .map(|m| {
            resolver_refs(
                &resolver_tardios(corpo.imp, m),
                &ctx.refs_resolvidos.borrow(),
            )
        })
        .collect();
    // `detectHostChanges` comes after the handlers (`view.methods`).
    let metodos = metodos + &resolver_tardios(corpo.imp, &host_changes);
    // `corpo` borrows the interner and the import table; emitting the embedded
    // views needs both.
    *coleta = corpo.coleta.take();
    drop(corpo);

    imp.sem_alias(ANGULAR);
    // The embedded views are written here, after the factories — and that is
    // why their imports come after `angular.dart`.
    let mut embutidas = String::new();
    for espec in especs {
        embutidas.push_str(&emitir_embutida(espec, &ctx, imp, nomes, coleta)?);
    }
    let hosp = imp.alias(HOST_VIEW);
    // The providers: fields, statements before the component and
    // `injectorGetInternal`, with the imports in the order they are written.
    let asset_local = local.asset();
    let provedores = match &no_hospedeiro {
        Some(r) => match escrever_provedores_da_hospedeira(r, &asset_local, &util) {
            Ok(p) => Some(p),
            Err(r) => {
                if coleta.is_none() {
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
    let container = c
        .parametros
        .iter()
        .any(|p| e_container(p, local, resolvedor));
    let mut campo_do_container = String::new();
    let mut antes_do_componente = String::new();
    if container {
        let vc = imp.q(VIEW_CONTAINER);
        campo_do_container = format!("  late final {vc}ViewContainer _appEl_0;\n");
        antes_do_componente.push_str(&format!(
            "    this._appEl_0 = {vc}ViewContainer(0, null, this, _el_0);\n"
        ));
    }
    // The emitter writes the fields with an initializer (the lazy ones) before
    // the others (`dart_emitter.dart:198-205`).
    let mut campos_hosp = provedores
        .as_ref()
        .map(|p| resolver_tardios(imp, &p.campos))
        .unwrap_or_default();
    campos_hosp.push_str(&campo_do_container);
    antes_do_componente.push_str(
        &provedores
            .as_ref()
            .map(|p| resolver_tardios(imp, &p.antes_do_componente))
            .unwrap_or_default(),
    );
    let raiz_hosp = if container { "this._appEl_0" } else { "_el_0" };
    let locais: Vec<(crate::directives::Token, String)> = no_hospedeiro
        .iter()
        .flat_map(|r| &r.instancias)
        .flat_map(|i| {
            std::iter::once(i.token.clone())
                .chain(i.apelidos.iter().cloned())
                .map(|t| (t, i.leitura.clone()))
        })
        .collect();
    let construcao =
        match construcao_do_componente(c, local, resolvedor, imp, &proprio, &util, &locais) {
            Some(x) => x,
            None => {
                // In collection mode the refusal already came from
                // `falta_para_construir`.
                if coleta.is_none() {
                    return Err(recusa(
                        Motivo::InjecaoNaoResolvida,
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
    let metadados_de_quem_herda = local.metadados.as_deref().filter(|m| m.fora.is_empty());
    let entradas_herdadas =
        c.herda && metadados_de_quem_herda.is_some_and(|m| !m.entradas.is_empty());
    let marca = c.on_push && (!c.entradas.is_empty() || entradas_herdadas);
    if c.on_push && c.entradas.is_empty() && c.herda && metadados_de_quem_herda.is_none() {
        let r = recusa(
            Motivo::NaoEntendido,
            "inheriting onPush component, without the metadata",
        );
        if coleta.is_none() {
            return Err(r);
        }
    }
    let injetor_hosp = match &provedores {
        Some(p) if !p.injetaveis.is_empty() => resolver_tardios(
            imp,
            &metodo_injetor(&[(0, 0, p.injetaveis.clone())], &asset_local),
        ),
        _ => String::new(),
    };
    // The hooks of an inheriting component walk up the supertypes (the class
    // that implements `OnInit` may be the base): they come from the program
    // metadata.
    // Also without inheritance: `implements X` with `X extends OnInit` can only
    // be seen through the program (`allSupertypes`); the class text is not
    // enough.
    let ganchos = if let Some(m) = local.metadados.as_deref().filter(|m| m.fora.is_empty()) {
        m.ganchos
    } else if c.herda {
        match local.metadados.as_deref() {
            Some(m) if m.fora.is_empty() => m.ganchos,
            _ => {
                anotar(
                    coleta,
                    recusa(
                        Motivo::NaoEntendido,
                        "hooks of an inheriting component without the metadata",
                    ),
                )?;
                c.ganchos
            }
        }
    } else {
        c.ganchos
    };
    // With `@HostBinding`, the host view calls `detectHostChanges` before
    // detecting the component's view (`ciclo_de_vida`).
    let ciclo = resolver_tardios(
        imp,
        &ciclo_de_vida(
            &ganchos,
            marca,
            do_hospedeiro.iter().any(|l| !l.estatico),
            container,
        ),
    );

    let x = &c.classe;
    let (args, decl) = (&ctx.genericos, &ctx.genericos_decl);
    let seletor = &c.seletor;
    // `_tagNameFromComponentSelector`: o primeiro seletor com elemento.
    let tag = crate::selector::Seletor::analisar(seletor)
        .into_iter()
        .find_map(|s| s.elemento)
        .unwrap_or_else(|| seletor.trim().to_string());
    // `_getChangeDetectionCheckMode`: an `onPush` component starts in
    // `checkOnce`.
    let estado = if c.on_push {
        "checkOnce"
    } else {
        "checkAlways"
    };
    // Without a stylesheet, the list is constant and the style is not
    // encapsulated.
    let itens: Vec<String> = estilo
        .iter()
        .map(|a| format!("{a}.styles"))
        .chain(em_linha)
        .collect();
    let (lista_de_estilos, encapsulamento) = match (itens.is_empty(), c.sem_encapsulamento) {
        (true, _) => ("const []".to_string(), "unscoped"),
        (false, true) => (format!("[{}]", itens.join(", ")), "unscoped"),
        (false, false) => (format!("[{}]", itens.join(", ")), "scoped"),
    };
    let asset = format!("asset:{}/{}", local.pacote, local.relativo);

    let mut s = String::with_capacity(4096);
    let _ = write!(
        s,
        "
final List<Object> styles${x} = {lista_de_estilos};

class View{x}0{decl} extends {vista}.ComponentView<{proprio}.{x}{args}> {{
{campos}  static {estilos}.ComponentStyles? _componentStyles;
  View{x}0({view}.View parentView, int parentIndex) : super(parentView, parentIndex, {cd}.ChangeDetectionCheckedState.{estado}) {{
    this.initComponentStyles();
    this.rootElement = {util}.unsafeCast({documento}.createElement('{tag}'));{estaticos_no_construtor}
  }}
  static String? get _debugComponentUrl {{
    return ({util}.isDevMode ? '{asset}' : null);
  }}

  @override
  void build() {{{ctx_no_build}
    final parentRenderNode = this.initViewRoot();{corpo_build}
  }}
{injetor}{deteccao}{destruicao}{metodos}
  static void _debugClearComponentStyles() {{
    _componentStyles = null;
  }}

  void initComponentStyles() {{
    var styles = _componentStyles;
    if ((styles == null)) {{
      _componentStyles = (styles = {estilos}.ComponentStyles.{encapsulamento}(styles${x}, _debugComponentUrl));
      if ({util}.isDevMode) {{
        {estilos}.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }}
    }}
    this.componentStyles = styles;
  }}
}}

const _{x}NgFactory = ComponentFactory<{proprio}.{x}>('{seletor}', viewFactory_{x}Host0);
ComponentFactory<{proprio}.{x}> get {x}NgFactory {{
  return _{x}NgFactory;
}}

ComponentFactory<{proprio}.{x}{args}> create{x}Factory{decl}() {{
  return ComponentFactory('{seletor}', viewFactory_{x}Host0);
}}
{embutidas}
final List<Object> styles${x}Host = const [];

class _View{x}Host0{decl} extends {hosp}.HostView<{proprio}.{x}{args}> {{
{campos_hosp}  @override
  void build() {{
    this.componentView = View{x}0(this, 0);
    final _el_0 = this.componentView.rootElement;
{antes_do_componente}    this.component = {construcao}
{consultas_hosp}    this.initRootNode({raiz_hosp});
  }}
{injetor_hosp}{ciclo}}}

{hosp}.HostView<{proprio}.{x}{args}> viewFactory_{x}Host0{decl}() {{
  return _View{x}Host0();
}}
"
    );
    // The chains of the dynamic queries and the node that a view reads from a
    // nested one (the query result) are only known after the nested ones are
    // emitted.
    let s = resolver_consultas(
        &s,
        &corpo_consultas,
        &ctx.ancoras_de_consulta.borrow(),
        &classe_raiz,
    );
    let s = resolver_conteudo_dinamico(
        &s,
        &ctx.conteudo_dinamico.borrow(),
        &ctx.ancoras_de_consulta.borrow(),
    );
    let s = resolver_refs(&s, &ctx.refs_resolvidos.borrow());
    if s.contains(MARCA_DE_REF) && coleta.is_none() {
        return Err(recusa(Motivo::Ligacao, "#ref without a node in the template"));
    }
    Ok((s, tb_sobrando))
}

/// The arguments (`<T, U>`) and the declaration (`<T extends num, U>`) of the
/// type parameters of a generic component, with the bound qualified by the
/// import of whoever declares it; empty for a non-generic component.
fn parametros_de_tipo(
    c: &Componente,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
    imp: &mut Importacoes,
) -> Result<(String, String), Recusa> {
    if c.parametros_de_tipo.is_empty() {
        return Ok(Default::default());
    }
    let livres: Vec<&str> = c
        .parametros_de_tipo
        .iter()
        .map(|(n, _)| n.as_str())
        .collect();
    let mut decl = Vec::new();
    for (nome, limite) in &c.parametros_de_tipo {
        match limite {
            None => decl.push(nome.clone()),
            Some(l) => {
                let q = resolvedor
                    .and_then(|r| tipo_qualificado(l, local.caminho, r, &local.asset(), &livres))
                    .ok_or_else(|| {
                        recusa(
                            Motivo::NaoEntendido,
                            format!("type parameter bound `{l}` without an import"),
                        )
                    })?;
                decl.push(format!("{nome} extends {}", resolver_tardios(imp, &q)));
            }
        }
    }
    Ok((
        format!("<{}>", livres.join(", ")),
        format!("<{}>", decl.join(", ")),
    ))
}

/// Are a component's `providers:` in the part the emitter writes (in its host
/// view or on its user's node)? Otherwise, the reason.
pub(crate) fn provedores_escreviveis(meta: &crate::directives::Diretiva) -> Result<(), String> {
    use crate::directives::{TipoDeToken, Token};
    if let Some(f) = meta.fora.first() {
        return Err(f.clone());
    }
    // The `T` of a token from outside `dart:core` and without type arguments
    // would get another import: no case yet.
    let tipo_conhecido = |t: &TipoDeToken| t.uri == "dart:core" || t.genericos > 0;
    let token_conhecido = |t: &Token| match t {
        Token::Multi { tipo, .. } => tipo_conhecido(tipo),
        // Generic `OpaqueToken<T>`: no case yet.
        Token::Opaco { tipo, .. } => tipo.uri == "dart:core",
        _ => true,
    };
    let com_argumentos = |t: &Token| match t {
        Token::Multi { tipo, .. } | Token::Opaco { tipo, .. } => !tipo.args.is_empty(),
        _ => false,
    };
    for p in meta.provedores.iter().chain(&meta.provedores_de_visao) {
        if com_argumentos(&p.token) || p.tipo.as_ref().is_some_and(|t| !t.args.is_empty()) {
            return Err("type token with concrete arguments".into());
        }
        let alvo = match &p.fonte {
            crate::directives::Fornece::Existente(t) => Some(t),
            _ => None,
        };
        if !token_conhecido(&p.token) || alvo.is_some_and(|t| !token_conhecido(t)) {
            return Err("type token outside dart:core".into());
        }
    }
    Ok(())
}

/// The host view's providers: node 0, with the component and its `providers:`
/// (the `ProviderElementContext` of the host element). Refuses what the emitter
/// does not write yet.
fn provedores_da_hospedeira(local: &Local) -> Result<crate::directives::NoResolvido, Recusa> {
    use crate::directives::{Criacao, Expr};
    let prov = |f: &str| recusa(Motivo::Providers, format!("providers: {f}"));
    let meta = local
        .metadados
        .clone()
        .ok_or_else(|| prov("without the program metadata"))?;
    provedores_escreviveis(&meta).map_err(|f| prov(&f))?;
    let r = crate::directives::resolver_hospedeira(meta).map_err(prov)?;
    // The component is the last eager one: those it asks for come before, and
    // nothing after it is created in `build()` (a content query that finds a
    // provider would make it eager: refused separately).
    let ultimo_ansioso = r.instancias.iter().rposition(|i| !i.preguicosa);
    if ultimo_ansioso.map(|k| r.instancias[k].campo.as_str()) != Some("component") {
        return Err(prov("provedor ansioso depois do componente"));
    }
    for i in &r.instancias {
        if let Criacao::Multi(itens) = &i.criacao
            && itens.iter().any(Expr::dinamica)
        {
            return Err(prov("multi-provider item with an outside dependency"));
        }
    }
    Ok(r)
}

/// The type of an inferred `T` (`o.importType`): one from `dart:core` without a
/// prefix, with its import counted.
fn texto_do_tipo(t: &crate::directives::TipoDeToken, asset: &str) -> String {
    let args = if t.genericos > 0 {
        format!("<{}>", vec!["dynamic"; t.genericos].join(", "))
    } else {
        String::new()
    };
    let uri = if t.uri == "dart:core" {
        t.uri.clone()
    } else {
        import_de(&t.uri, asset)
    };
    format!("{}{}{args}", tardio_q(&uri), t.classe)
}

/// The token of a provider seen from this file, ready for [`expr_do_token`].
fn token_local(t: &crate::directives::Token, asset: &str) -> crate::directives::Token {
    use crate::directives::{TipoDeToken, Token};
    let tipo = |x: &TipoDeToken| TipoDeToken {
        uri: import_de(&x.uri, asset),
        ..x.clone()
    };
    match t {
        Token::Classe { uri, classe } => Token::Classe {
            uri: import_de(uri, asset),
            classe: classe.clone(),
        },
        Token::Multi { nome, tipo: x } => Token::Multi {
            nome: nome.clone(),
            tipo: tipo(x),
        },
        Token::Opaco { nome, tipo: x } => Token::Opaco {
            nome: nome.clone(),
            tipo: tipo(x),
        },
        outro => outro.clone(),
    }
}

/// A `useValue:` as the emitter writes it.
fn texto_do_valor(v: &crate::directives::ValorConst, asset: &str) -> String {
    use crate::directives::ValorConst;
    match v {
        ValorConst::Texto(s) => literal(s),
        ValorConst::Inteiro(i) => i.to_string(),
        ValorConst::Booleano(b) => b.to_string(),
        ValorConst::Objeto {
            uri,
            classe,
            construtor,
            posicionais,
            nomeados,
        } => {
            let mut args: Vec<String> = posicionais
                .iter()
                .map(|x| texto_do_valor(x, asset))
                .collect();
            args.extend(
                nomeados
                    .iter()
                    .map(|(n, x)| format!("{n}: {}", texto_do_valor(x, asset))),
            );
            let ctor = construtor
                .as_ref()
                .map(|c| format!(".{c}"))
                .unwrap_or_default();
            format!(
                "const {}{classe}{ctor}({})",
                tardio_q(&import_de(uri, asset)),
                args.join(", ")
            )
        }
    }
}

/// The value of a provider (`ProviderSource.build`). With an injector
/// dependency, the creation goes wrapped in `debugInjectorWrap` under
/// `isDevMode` (`recuo`: the column of the statement or field). `vista`: in the
/// host view (`None`), the injector is its own (`this.injectorGet(T,
/// this.parentIndex)`); in a template node, the one from outside the view that
/// reads it (`injectFromViewParentInjector`: `(v.parentView!).injectorGet(T,
/// v.parentIndex)`).
fn texto_da_expr(
    e: &crate::directives::Expr,
    token: &crate::directives::Token,
    asset: &str,
    util: &str,
    recuo: usize,
    vista: Option<&str>,
) -> String {
    use crate::directives::Expr;
    let simples = |e: &Expr| -> String {
        let args = |a: &[Expr]| -> String {
            a.iter()
                .map(|x| texto_da_expr(x, token, asset, util, recuo, vista))
                .collect::<Vec<_>>()
                .join(", ")
        };
        match e {
            Expr::Campo(c) => format!("this.{c}"),
            Expr::Leitura(l) => l.clone(),
            Expr::Injetor { token, opcional } => {
                let metodo = if *opcional {
                    "injectorGetOptional"
                } else {
                    "injectorGet"
                };
                let t = expr_do_token(&token_local(token, asset));
                match vista {
                    None => format!("this.{metodo}({t}, this.parentIndex)"),
                    Some(v) => format!("({v}.parentView!).{metodo}({t}, {v}.parentIndex)"),
                }
            }
            Expr::Classe {
                uri,
                classe,
                args: a,
            } => {
                format!("{}{classe}({})", tardio_q(&import_de(uri, asset)), args(a))
            }
            Expr::Fabrica { uri, nome, args: a } => {
                format!("{}{nome}({})", tardio_q(&import_de(uri, asset)), args(a))
            }
            Expr::Valor(v) => texto_do_valor(v, asset),
        }
    };
    let x = simples(e);
    if !e.dinamica() {
        return x;
    }
    let (a, b, c) = (
        " ".repeat(recuo + 4),
        " ".repeat(recuo + 8),
        " ".repeat(recuo + 6),
    );
    format!(
        "({util}.isDevMode\n{a}? {}.debugInjectorWrap({}, () {{\n{b}return {x};\n{c}}})\n{a}: {x})",
        tardio(DI_ERRORS),
        expr_do_token(&token_local(token, asset))
    )
}

/// The field type of a host view provider (`createProvider`): the inferred `T`;
/// without it, the type of the expression (`dynamic` when it has none).
fn tipo_do_provedor(i: &crate::directives::Instancia, asset: &str) -> Result<String, Recusa> {
    use crate::directives::{Criacao, Expr, ValorConst};
    if let Criacao::Multi(_) = &i.criacao {
        return Ok(match &i.tipo {
            Some(t) => format!("List<{}>", texto_do_tipo(t, asset)),
            None => "List<dynamic>".to_string(),
        });
    }
    if let Some(t) = &i.tipo {
        return Ok(texto_do_tipo(t, asset));
    }
    let Criacao::Expressao(e) = &i.criacao else {
        return Err(recusa(
            Motivo::Providers,
            "providers: provider without an expression",
        ));
    };
    Ok(match e {
        Expr::Classe { uri, classe, .. } if !e.dinamica() => {
            format!("{}{classe}", tardio_q(&import_de(uri, asset)))
        }
        Expr::Valor(ValorConst::Texto(_)) => "String".into(),
        Expr::Valor(ValorConst::Inteiro(_)) => "int".into(),
        Expr::Valor(ValorConst::Booleano(_)) => "bool".into(),
        Expr::Valor(ValorConst::Objeto { uri, classe, .. }) => {
            format!("{}{classe}", tardio_q(&import_de(uri, asset)))
        }
        Expr::Classe { .. } | Expr::Fabrica { .. } | Expr::Injetor { .. } => "dynamic".into(),
        Expr::Campo(_) | Expr::Leitura(_) => {
            return Err(recusa(
                Motivo::Providers,
                "providers: provider that reads another field",
            ));
        }
    })
}

/// The field of a lazy `providers:` provider (`late T _X_n_m = value;`), with
/// deferred imports: the type and then the value, the order in which the
/// official compiler writes (and numbers the imports).
fn texto_de_provedor_preguicoso(
    i: &crate::directives::Instancia,
    asset: &str,
    vista: Option<&str>,
) -> Result<String, Recusa> {
    use crate::directives::Criacao;
    let tipo = tipo_do_provedor(i, asset)?;
    // The `isDevMode` of `debugInjectorWrap` (only with an injector dependency)
    // with a deferred import, in text order (case i78).
    let util = tardio(UTILITIES);
    let valor = match &i.criacao {
        Criacao::Expressao(e) => texto_da_expr(e, &i.token, asset, &util, 2, vista),
        Criacao::Multi(itens) => format!(
            "[{}]",
            itens
                .iter()
                .map(|x| texto_da_expr(x, &i.token, asset, &util, 2, vista))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => {
            return Err(recusa(
                Motivo::LigacaoEmFilho,
                "lazy provider that is not an expression",
            ));
        }
    };
    Ok(format!("  late {tipo} {} = {valor};", i.campo))
}

/// What the providers put in the host view: the fields (the lazy ones, with an
/// initializer, first — `visitDeclareClassStmt` groups them like that), the
/// `build()` statements before the component, and the injectables of
/// `injectorGetInternal`. All with deferred imports, in text order.
struct ProvedoresNaHospedeira {
    campos: String,
    antes_do_componente: String,
    injetaveis: Vec<(Vec<crate::directives::Token>, String)>,
}

fn escrever_provedores_da_hospedeira(
    r: &crate::directives::NoResolvido,
    asset: &str,
    util: &str,
) -> Result<ProvedoresNaHospedeira, Recusa> {
    use crate::directives::Criacao;
    let mut preguicosos = Vec::new();
    let mut ansiosos = Vec::new();
    let mut antes = String::new();
    for i in &r.instancias {
        if i.campo == "component" {
            continue;
        }
        let tipo = tipo_do_provedor(i, asset)?;
        let valor = |recuo: usize| match &i.criacao {
            Criacao::Expressao(e) => texto_da_expr(e, &i.token, asset, util, recuo, None),
            Criacao::Multi(itens) => format!(
                "[{}]",
                itens
                    .iter()
                    .map(|x| texto_da_expr(x, &i.token, asset, util, recuo, None))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            _ => String::new(),
        };
        if i.preguicosa {
            preguicosos.push(format!("  late {tipo} {} = {};\n", i.campo, valor(2)));
        } else {
            ansiosos.push(format!("  late final {tipo} {};\n", i.campo));
            let _ = writeln!(antes, "    this.{} = {};", i.campo, valor(4));
        }
    }
    let injetaveis = r
        .instancias
        .iter()
        .filter(|i| !i.injetavel_por.is_empty())
        .map(|i| {
            (
                i.injetavel_por
                    .iter()
                    .map(|t| token_local(t, asset))
                    .collect(),
                i.leitura.clone(),
            )
        })
        .collect();
    preguicosos.extend(ansiosos);
    Ok(ProvedoresNaHospedeira {
        campos: preguicosos.concat(),
        antes_do_componente: antes,
        injetaveis,
    })
}

/// The component's own `@ContentChildren` in the host view: without content,
/// the list receives `[]` right after the construction
/// (`updateQueryAtStartup`), in query order; the single `@ContentChild`
/// receives nothing. A query that would find the node itself (the component,
/// one of its providers or an ngdart type) is refused.
fn consultas_da_hospedeira(
    c: &Componente,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
    tokens: &[crate::directives::Token],
) -> Result<String, Recusa> {
    let Some(consultas) = &c.consultas_de_conteudo else {
        return Ok(String::new());
    };
    let mut s = String::new();
    for q in consultas {
        if !q.referencia {
            let uri = resolvedor
                .and_then(|r| r.uri_do_tipo(local.caminho, &q.alvo))
                .ok_or_else(|| {
                    recusa(Motivo::ContentChild, "@ContentChild of an unresolved type")
                })?;
            let simples = q.alvo.rsplit('.').next().unwrap_or(&q.alvo).to_string();
            let token = crate::directives::Token::Classe {
                uri: uri.clone(),
                classe: simples.clone(),
            };
            let proprio = import_de(&uri, &local.asset()) == local.arquivo && simples == c.classe;
            if proprio
                || tokens.contains(&token)
                || uri.starts_with("package:ngdart/")
                || uri.starts_with("dart:")
            {
                return Err(recusa(
                    Motivo::ContentChild,
                    "@ContentChild that would find the node itself",
                ));
            }
        }
        if q.lista {
            let _ = writeln!(s, "    this.component.{} = [];", q.campo);
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
fn construcao_do_componente(
    c: &Componente,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
    imp: &mut Importacoes,
    proprio: &str,
    util: &str,
    locais: &[(crate::directives::Token, String)],
) -> Option<String> {
    let x = &c.classe;
    // A parameter that a provider of the node itself satisfies reads its field
    // (`this._Servico_0_5`) and does not count as injection.
    let do_no = |p: &crate::component::Parametro| -> Option<String> {
        let tipo = p.tipo.as_deref()?.trim_end_matches('?');
        let simples = tipo.rsplit('.').next()?;
        let uri = resolvedor?.uri_do_tipo(local.caminho, tipo)?;
        let token = crate::directives::Token::Classe {
            uri,
            classe: simples.to_string(),
        };
        locais
            .iter()
            .find(|(t, _)| *t == token)
            .map(|(_, c)| format!("this.{c}"))
    };
    // In the host view (`component.type.isHost`), what the node does not
    // provide comes from the injector, with or without `@Host()`; `@Self()`
    // stays on the node (or `null`, optional); `@SkipSelf()` skips the node
    // (`_getDependency`, case j79).
    let injeta = c.parametros.iter().any(|p| {
        if let Some(d) = dependencia_anotada(c, local, p) {
            return d.atributo.is_none() && !locais.iter().any(|(t, _)| *t == d.token);
        }
        !e_elemento(p.tipo.as_deref())
            && !e_detector(p, local, resolvedor)
            && !e_container(p, local, resolvedor)
            && !p.proprio
            && (p.pular || do_no(p).is_none())
    });
    // `errors.dart` goes in before the injected types, as in the official
    // output.
    let erros = injeta.then(|| imp.alias(DI_ERRORS));
    let mut args = Vec::new();
    for p in &c.parametros {
        // `@Attribute`: the host element has no attributes (`null`).
        // `@Inject(token)`: the node's provider or the injector, by token.
        if let Some(d) = dependencia_anotada(c, local, p) {
            if d.atributo.is_some() {
                args.push("null".to_string());
                continue;
            }
            if let Some((_, campo)) = locais.iter().find(|(t, _)| *t == d.token) {
                args.push(format!("this.{campo}"));
                continue;
            }
            let expr = match &d.token {
                crate::directives::Token::Classe { uri, classe } => {
                    let asset = asset_de_uri(uri, local.pacote, local.raiz)?;
                    let caminho = caminho_do_import(&local.asset(), &asset)?;
                    format!("{}.{classe}", imp.alias(&caminho))
                }
                t => resolver_tardios(imp, &expr_do_token(t)),
            };
            let metodo = if d.opcional {
                "injectorGetOptional"
            } else {
                "injectorGet"
            };
            args.push(format!("this.{metodo}({expr}, this.parentIndex)"));
            continue;
        }
        if e_elemento(p.tipo.as_deref()) {
            args.push("_el_0".to_string());
            continue;
        }
        // `ChangeDetectorRef`: the component's view.
        if e_detector(p, local, resolvedor) {
            args.push("this.componentView".to_string());
            continue;
        }
        // `ViewContainerRef`: o `ViewContainer` do elemento hospedeiro.
        if e_container(p, local, resolvedor) {
            args.push("this._appEl_0".to_string());
            continue;
        }
        if !p.nomeado
            && !p.pular
            && let Some(campo) = do_no(p)
        {
            args.push(campo);
            continue;
        }
        if p.proprio && p.opcional && !p.nomeado && !p.outra_anotacao {
            args.push("null".to_string());
            continue;
        }
        if p.outra_anotacao || p.proprio || p.nomeado {
            return None; // `@Inject(...)`, `@Self`, named: not yet
        }
        let tipo = tipo_do_token(p.tipo.as_deref()?);
        let simples = tipo.rsplit('.').next()?;
        let uri = resolvedor?.uri_do_tipo(local.caminho, tipo)?;
        // Another built-in of the host element (`ElementRef`, `TemplateRef`…):
        // does not come from the injector; no case yet.
        if crate::directives::embutido_do_elemento(&crate::directives::Token::Classe {
            uri: uri.clone(),
            classe: simples.to_string(),
        }) {
            return None;
        }
        let asset = asset_de_uri(&uri, local.pacote, local.raiz)?;
        let caminho = caminho_do_import(&local.asset(), &asset)?;
        let alias = imp.alias(&caminho);
        // `@Optional()`: `injectorGetOptional` (`injectFromViewParentInjector`).
        let metodo = if p.opcional {
            "injectorGetOptional"
        } else {
            "injectorGet"
        };
        args.push(format!(
            "this.{metodo}({alias}.{simples}, this.parentIndex)"
        ));
    }
    let chamada = format!("{proprio}.{x}({})", args.join(", "));
    let Some(erros) = erros else {
        return Some(format!("{chamada};"));
    };
    Some(format!(
        "({util}.isDevMode
        ? {erros}.debugInjectorWrap({proprio}.{x}, () {{
            return {chamada};
          }})
        : {chamada});"
    ))
}

/// The dependency (from the program metadata) of a parameter with `@Inject` or
/// `@Attribute`, by its position among the unnamed ones; `None` without the
/// metadata, and also with `@Self`/`@Host`/`@SkipSelf` alongside (no case yet)
/// or on an element built-in.
fn dependencia_anotada<'m>(
    c: &Componente,
    local: &'m Local,
    p: &crate::component::Parametro,
) -> Option<&'m crate::directives::Dependencia> {
    if !p.outra_anotacao || p.nomeado || p.proprio || p.hospedeiro || p.pular {
        return None;
    }
    let k = c
        .parametros
        .iter()
        .filter(|q| !q.nomeado)
        .position(|q| std::ptr::eq(q, p))?;
    let meta = local.metadados.as_deref()?;
    if !meta.dependencias_lidas
        || meta.dependencias.len() != c.parametros.iter().filter(|q| !q.nomeado).count()
    {
        return None;
    }
    let d = meta.dependencias.get(k)?;
    match &d.token {
        _ if d.atributo.is_some() => Some(d),
        crate::directives::Token::Elemento | crate::directives::Token::Detector => None,
        t if crate::directives::embutido_do_elemento(t) => None,
        _ => Some(d),
    }
}

/// Is the parameter the component's root element?
/// Is the parameter ngdart's `ChangeDetectorRef` (without an annotation)?
fn e_detector(
    p: &crate::component::Parametro,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> bool {
    let Some(tipo) = p.tipo.as_deref() else {
        return false;
    };
    !p.anotado
        && tipo.rsplit('.').next() == Some("ChangeDetectorRef")
        && resolvedor
            .and_then(|r| r.uri_do_tipo(local.caminho, tipo))
            .is_some_and(|u| u.starts_with("package:ngdart/"))
}

/// Is the parameter ngdart's `ViewContainerRef` (without an annotation)?
fn e_container(
    p: &crate::component::Parametro,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> bool {
    let Some(tipo) = p.tipo.as_deref() else {
        return false;
    };
    !p.anotado
        && matches!(
            tipo.rsplit('.').next(),
            Some("ViewContainerRef" | "ComponentLoader")
        )
        && resolvedor
            .and_then(|r| r.uri_do_tipo(local.caminho, tipo))
            .is_some_and(|u| u.starts_with("package:ngdart/"))
}

/// Does the parameter ask for the element node: `Element`/`HtmlElement` of
/// `dart:html`, or `Element`/`HTMLElement` of `package:web` in 9?
fn e_elemento(tipo: Option<&str>) -> bool {
    let ngx = crate::dialect::atual() == crate::dialect::Dialeto::Ngx;
    match tipo.map(|t| t.rsplit('.').next().unwrap_or(t)) {
        Some("Element") => true,
        Some("HtmlElement") => !ngx,
        Some("HTMLElement") => ngx,
        _ => false,
    }
}

/// The token of a parameter by its written type (`_tokenForType`/`_idFor`): the
/// class, without the `?` (`@Optional() X? x`) and without the type arguments
/// (`MaterialTreeRoot<T>` asks for `MaterialTreeRoot`).
pub(crate) fn tipo_do_token(tipo: &str) -> &str {
    tipo.split('<')
        .next()
        .unwrap_or(tipo)
        .trim()
        .trim_end_matches('?')
}

/// What prevents the construction, if anything. The import table is not touched
/// here — this only looks.
fn falta_para_construir(
    c: &Componente,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> Option<Recusa> {
    for p in &c.parametros {
        if e_elemento(p.tipo.as_deref()) {
            continue;
        }
        if dependencia_anotada(c, local, p).is_some() {
            continue;
        }
        if p.outra_anotacao || (p.proprio && !p.opcional) {
            return Some(recusa(
                Motivo::InjecaoAnotada,
                "@Inject/@Attribute/@Self in the constructor",
            ));
        }
        if p.nomeado {
            return Some(recusa(
                Motivo::InjecaoNomeada,
                "named parameter in the constructor",
            ));
        }
        let Some(tipo) = p.tipo.as_deref() else {
            return Some(recusa(
                Motivo::InjecaoSemTipo,
                "untyped parameter in the constructor",
            ));
        };
        let tipo = tipo_do_token(tipo);
        if !resolvedor.is_some_and(|r| r.uri_do_tipo(local.caminho, tipo).is_some()) {
            return Some(recusa(
                Motivo::InjecaoNaoResolvida,
                "unresolved injected type",
            ));
        }
    }
    None
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::component::Parametro;
    use dartforge_intern::Interner;

    /// `<template ngFor let-x [ngForOf]>` becomes the `estrela` of the
    /// equivalent `*ngFor`, keeping the range of each written binding (i84,
    /// i86).
    #[test]
    fn template_com_diretiva_vira_estrela() {
        let nos = crate::html::analisar(
            r#"<template ngFor [ngForTrackBy]="f" let-item [ngForOf]="xs" let-i="index"><p></p></template>"#,
        );
        let nos = template_como_container(&nos);
        let [No::Elemento(e)] = nos.as_slice() else {
            panic!("a single element");
        };
        assert_eq!(e.nome, "ng-container");
        let estrela = e.estrela.as_ref().expect("estrela");
        assert_eq!(estrela.nome, "ngFor");
        assert_eq!(estrela.valor, "let item; let i = index; trackBy: f; of: xs");
        let micro = e.micro_da_estrela().unwrap_or_default();
        assert_eq!(
            micro.propriedades,
            vec![
                ("ngForTrackBy".to_string(), "f".to_string()),
                ("ngForOf".to_string(), "xs".to_string())
            ]
        );
        let spans: Vec<(&str, usize, usize)> = e
            .ligacoes_do_molde
            .iter()
            .map(|l| (l.nome.as_str(), l.inicio, l.fim))
            .collect();
        assert_eq!(spans, vec![("ngForTrackBy", 16, 34), ("ngForOf", 44, 58)]);
    }

    /// What is not the exact shape stays a `<template>` (and is refused):
    /// unknown directive, two attributes, `;` in the expression, event.
    #[test]
    fn template_com_diretiva_fora_da_forma() {
        for t in [
            r#"<template foo [fooOf]="xs"></template>"#,
            r#"<template ngFor ngIf [ngForOf]="xs"></template>"#,
            r#"<template ngFor [ngForOf]="xs" (x)="y()"></template>"#,
            r#"<template ngFor let-x></template>"#,
        ] {
            let nos = template_como_container(&crate::html::analisar(t));
            let [No::Elemento(e)] = nos.as_slice() else {
                panic!("a single element");
            };
            assert_eq!(e.nome, "template", "{t}");
            assert!(e.ligacoes_do_molde.is_empty(), "{t}");
        }
        // A `;` inside a string does not get in the way: the microsyntax goes
        // decomposed, without going back to the text (case j75).
        let nos = template_como_container(&crate::html::analisar(
            r#"<template ngFor [ngForOf]="f(';')"></template>"#,
        ));
        let [No::Elemento(e)] = nos.as_slice() else {
            panic!("a single element");
        };
        assert_eq!(e.nome, "ng-container");
        assert_eq!(
            e.micro_da_estrela().expect("micro").propriedades,
            vec![("ngForOf".to_string(), "f(';')".to_string())]
        );
    }

    fn local() -> Local<'static> {
        Local {
            pacote: "new_sali_frontend",
            relativo: "lib/src/shared/components/form_feedback/form_feedback_component.dart",
            arquivo: "form_feedback_component.dart",
            caminho: Path::new("x.dart"),
            raiz: Path::new(""),
            url_do_template: None,
            metadados: None,
        }
    }

    /// Answers what the semantic database would answer, without loading a
    /// program: the pair (type name, URI of the library that declares it).
    struct Tabela(&'static [(&'static str, &'static str)]);

    impl Resolucao for Tabela {
        fn uri_do_tipo(&self, _arquivo: &Path, nome: &str) -> Option<String> {
            self.0
                .iter()
                .find(|(n, _)| *n == nome)
                .map(|(_, u)| u.to_string())
        }
    }

    fn param(tipo: &str) -> Parametro {
        Parametro {
            tipo: Some(tipo.into()),
            nome: "p".into(),
            nomeado: false,
            anotado: false,
            opcional: false,
            proprio: false,
            hospedeiro: false,
            pular: false,
            outra_anotacao: false,
        }
    }

    /// Only a read with the implicit receiver calls `getLocal` (case i45).
    #[test]
    fn citacao_de_local_so_na_raiz() {
        assert!(cita_na_raiz("usar(campo.value)", "campo"));
        assert!(cita_na_raiz("a ? campo : b", "campo"));
        assert!(!cita_na_raiz("p.campo", "campo"));
        assert!(!cita_na_raiz("p?.campo", "campo"));
        assert!(!cita_na_raiz("f(campo: 1)", "campo"));
        assert!(!cita_na_raiz("'campo' + x", "campo"));
        assert!(!cita_na_raiz("campos", "campo"));
    }

    /// Exact bytes of the file the official compiler generated for
    /// `FormFeedbackComponent` (a template with only a comment).
    #[test]
    fn esqueleto_igual_ao_oficial() {
        let c = Componente {
            classe: "FormFeedbackComponent".into(),
            seletor: "form-feedback-comp".into(),
            ..Default::default()
        };
        let saida = template_de_componente(
            &c,
            &local(),
            &[No::Comentario("{{message}}".into())],
            None,
            &mut Interner::new(),
            &Default::default(),
            &[],
            &Ok(Vec::new()),
        )
        .expect("gera");
        let esperado = include_str!("../testdata/form_feedback_component.template.dart");
        assert_eq!(saida, esperado.replace("\r\n", "\n"));
    }

    /// Exact bytes of `CallbackComponent`, whose template is `<div>Processando
    /// login...</div>` — the first one with real nodes. The constructor
    /// injection is replaced with a constructor without parameters, which is
    /// what this step covers; the rest of the file is the official one.
    #[test]
    fn elemento_e_texto_iguais_ao_oficial() {
        let c = Componente {
            classe: "CallbackComponent".into(),
            seletor: "callback-page".into(),
            ..Default::default()
        };
        let local = Local {
            pacote: "new_sali_frontend",
            relativo: "lib/src/modules/auth/pages/callback/callback_component.dart",
            arquivo: "callback_component.dart",
            caminho: Path::new("callback_component.dart"),
            raiz: Path::new(""),
            url_do_template: None,
            metadados: None,
        };
        let nos = crate::html::analisar("<div>Processando login...</div>");
        let saida = template_de_componente(
            &c,
            &local,
            &nos,
            None,
            &mut Interner::new(),
            &Default::default(),
            &[],
            &Ok(Vec::new()),
        )
        .expect("gera");
        let esperado = include_str!("../testdata/callback_component.template.dart");
        assert_eq!(saida, esperado.replace("\r\n", "\n"));
    }

    #[test]
    fn ligacao_ainda_nao_gera() {
        let c = Componente {
            classe: "X".into(),
            seletor: "x".into(),
            ..Default::default()
        };
        let nos = crate::html::analisar("<div [hidden]=\"a\"></div>");
        assert_eq!(
            template_de_componente(
                &c,
                &local(),
                &nos,
                None,
                &mut Interner::new(),
                &Default::default(),
                &[],
                &Ok(Vec::new()),
            )
            .map_err(|r| r.motivo),
            Err(Motivo::Ligacao)
        );
    }

    #[test]
    fn componente_dentro_do_template_ainda_nao_gera() {
        let c = Componente {
            classe: "X".into(),
            seletor: "x".into(),
            ..Default::default()
        };
        let nos = crate::html::analisar("<outro-comp></outro-comp>");
        assert_eq!(
            template_de_componente(
                &c,
                &local(),
                &nos,
                None,
                &mut Interner::new(),
                &Default::default(),
                &[],
                &Ok(Vec::new()),
            )
            .map_err(|r| r.motivo),
            Err(Motivo::ComponenteNoTemplate)
        );
    }

    #[test]
    fn parametro_injetado_ainda_nao_gera() {
        let c = Componente {
            classe: "X".into(),
            seletor: "x".into(),
            parametros: vec![param("RestConfig")],
            ..Default::default()
        };
        // Without a semantic database there is no way to know which library
        // declares the type.
        assert_eq!(
            template_de_componente(
                &c,
                &local(),
                &[],
                None,
                &mut Interner::new(),
                &Default::default(),
                &[],
                &Ok(Vec::new()),
            )
            .map_err(|r| r.motivo),
            Err(Motivo::InjecaoNaoResolvida)
        );
    }

    /// Exact bytes of the official `CallbackComponent`, now **with** injection:
    /// two tokens, one from the package itself (relative import) and one from
    /// ngrouter (the `package:` import of the library that declares the type).
    #[test]
    fn injecao_igual_ao_oficial() {
        let c = Componente {
            classe: "CallbackComponent".into(),
            seletor: "callback-page".into(),
            parametros: vec![param("OidcService"), param("Router")],
            ..Default::default()
        };
        let local = Local {
            pacote: "new_sali_frontend",
            relativo: "lib/src/modules/auth/pages/callback/callback_component.dart",
            arquivo: "callback_component.dart",
            caminho: Path::new("callback_component.dart"),
            raiz: Path::new(""),
            url_do_template: None,
            metadados: None,
        };
        let tabela = Tabela(&[
            (
                "OidcService",
                "package:new_sali_frontend/src/shared/services/oidc_service.dart",
            ),
            ("Router", "package:ngrouter/src/router/router.dart"),
        ]);
        let nos = crate::html::analisar("<div>Processando login...</div>");
        let saida = template_de_componente(
            &c,
            &local,
            &nos,
            Some(&tabela),
            &mut Interner::new(),
            &Default::default(),
            &[],
            &Ok(Vec::new()),
        )
        .expect("gera");
        let esperado = include_str!("../testdata/callback_with_injection.template.dart");
        assert_eq!(saida, esperado.replace("\r\n", "\n"));
    }
}

/// Whether `caminho` (seen from `asset`) is the `.template.dart` of the file
/// itself — that of a child declared next to it, whose view takes no prefix.
/// The prefix of the `XNgCd` of a directive with `@HostBinding`: the import of
/// its `.template.dart`, or nothing when the class is in this same file (case
/// j45).
fn prefixo_do_ngcd(imp: &mut Importacoes, uri: &str, asset: &str) -> String {
    let tpl = import_de(&uri.replace(".dart", ".template.dart"), asset);
    if e_o_proprio_template(asset, &tpl) {
        String::new()
    } else {
        imp.q(&tpl)
    }
}

fn e_o_proprio_template(asset: &str, caminho: &str) -> bool {
    let nome = asset.rsplit('/').next().unwrap_or(asset);
    nome.strip_suffix(".dart")
        .is_some_and(|base| caminho == format!("{base}.template.dart"))
}

/// How the file generated in `asset` imports library `uri`
/// (`getImportModulePath`): relative in the same package and folder, `package:`
/// outside; `dart:` as is.
fn import_de(uri: &str, asset: &str) -> String {
    asset_de_uri(uri, "", Path::new(""))
        .and_then(|alvo| caminho_do_import(asset, &alvo))
        .unwrap_or_else(|| uri.to_string())
}

/// Does some element of the content have the `#ref`?
fn referencia_no_conteudo(nos: &[No], r: &str) -> bool {
    nos.iter().any(|n| {
        let No::Elemento(x) = n else { return false };
        x.referencias.iter().any(|y| y.nome == r) || referencia_no_conteudo(&x.filhos, r)
    })
}
