//! Reads the arguments of `@Component` and `@Directive` directly from the tree.
//!
//! The official compiler does this through `package:analyzer`, resolving each
//! argument as a constant. Here the arguments that matter to the generator are
//! literals written in the annotation itself (`selector: 'x'`, `templateUrl:
//! 'x.html'`), and those that are not — the `directives` list, the `providers`
//! — are kept as source text, for the stages that know how to use them.
use crate::view::{Reason, Refusal, refusal};
use dartforge_frontend::ast;
use dartforge_intern::Interner;

/// `@Component(...)` or `@Directive(...)` of a class, as written — the official
/// `CompileDirectiveMetadata`, in the part the generator uses.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Component {
    pub class: String,
    pub selector: String,
    /// `@Component` (and not `@Directive`).
    pub is_component: bool,
    /// The class extends another one or uses a mixin: inherited `@Input`,
    /// `@Output` and `@HostBinding` cannot be seen from here.
    pub inherits: bool,
    /// The type parameters of the class (`class X<T extends num, U>`): the name
    /// and the bound as written. The component's view is generic over the same
    /// parameters (`ViewX0<T> extends ComponentView<X<T>>`).
    pub type_params: Vec<(String, Option<String>)>,
    /// `exportAs:` — the name through which `#ref="name"` reaches the
    /// directive.
    pub export_as: Option<String>,
    /// `@Output`s: name in the template -> member (a `Stream` getter), in the
    /// order the official compiler collects them.
    pub outputs: Vec<(String, String)>,
    /// Some `@HostBinding`: whoever uses the directive calls its
    /// `detectHostChanges`.
    pub binds_host: bool,
    /// `@ContentChild`/`@ContentChildren`: the query is updated by the view of
    /// whoever projects the content.
    pub content_query: bool,
    /// `providers:`/`viewProviders:` with providers: they go into the node of
    /// the user and change the numbering of its providers.
    pub with_providers: bool,
    /// The names of `exports:`, in written order: they enter the expression
    /// scope (`_matchExport`) before the members.
    pub exports: Vec<String>,
    /// `visibility:` written: the value comes from the program metadata
    /// (`metadata.rs`); without it the host is unknown.
    pub written_visibility: bool,
    /// `template: '...'` when the template is in the annotation itself.
    pub template: Option<String>,
    /// Where the contents of `template:` start in the `.dart`, in UTF-16 code
    /// units — what the official compiler adds to the positions of the `REF`
    /// comment. Only when the literal is a simple string, without escapes or
    /// interpolation (the template text is that of the source, and the
    /// positions inside it hold in both).
    pub template_offset: Option<usize>,
    /// `templateUrl: 'x.html'`.
    pub template_url: Option<String>,
    pub style_urls: Vec<String>,
    /// `styles: ['...']` — stylesheets written in the annotation.
    pub styles: Vec<String>,
    /// `styleUrls:`/`styles:` with an item that is not a literal string (or
    /// that is not a list): what the official compiler reads cannot be seen
    /// from here, and the view refuses.
    pub unreadable_styles: bool,
    /// `changeDetection: ChangeDetectionStrategy.OnPush`.
    pub on_push: bool,
    /// `directiveTypes: [Typed<X>.of([#T]), ..]`: the type arguments of the
    /// generic directives of this view (`CompileTypedMetadata`).
    pub directive_types: Vec<DirectiveType>,
    /// `directiveTypes:` in a shape that cannot be read from here.
    pub unreadable_directive_types: bool,
    /// `@changeDetectionLink` (`package:ngdart/experimental.dart`): the views
    /// get `detectChangesInCheckAlwaysViews` (`view_builder.dart:522`).
    pub detection_link: bool,
    /// `encapsulation: ViewEncapsulation.none`. Without a stylesheet the
    /// official compiler already turns encapsulation off
    /// (`ast_directive_normalizer.dart`), so it only matters with
    /// `styleUrls`/`styles` — a shape still refused by the view.
    pub without_encapsulation: bool,
    /// `preserveWhitespace: true`: the template does not go through the
    /// `MinimizeWhitespaceVisitor` (only `&ngsp;` becomes a space) and the ends
    /// of interpolations are not compressed
    /// (`ast_template_parser.dart:155-164`,
    /// `expression_converter.dart:151,161`).
    pub preserve_whitespace: bool,
    /// Names written in `directives:`, in order, as written (`A`, `li.B`).
    /// Constant lists (`coreDirectives`) go in by name and are expanded by the
    /// semantic database.
    pub directives: Vec<String>,
    /// Some item of `directives:` is not a name (spread, `if`, expression): the
    /// list is not known.
    pub unreadable_directives: bool,
    /// Constructor parameters, in order — what the host view needs to
    /// instantiate the component.
    pub params: Vec<Parameter>,
    /// Lifecycle hooks the class implements.
    pub hooks: Hooks,
    /// Every shape of the component the generator does not translate yet —
    /// member annotation (`@HostBinding`, `@ContentChild`…) or `@Component`
    /// argument outside what it understands —, with the text the scoreboard
    /// shows. All of them, not only the first: that is how the scoreboard knows
    /// how many files a new shape unlocks. While there is one, the file is not
    /// ours: generating while ignoring it gives **wrong** output, not
    /// incomplete output.
    pub not_understood: Vec<Refusal>,
    /// Names written in `pipes:`, in order, as written. As in `directives:`, a
    /// constant list (`commonPipes`) goes in by name and is expanded by the
    /// semantic database. The list itself does not change the view; using a
    /// pipe in the template does.
    pub pipes: Vec<String>,
    /// Some item of `pipes:` is not a name: the list is not known.
    pub unreadable_pipes: bool,
    /// `@ViewChild('ref')` on a field, in declaration order — which is the
    /// order in which the official compiler writes the assignments
    /// (`queryIndex`). Whether the query is static or not depends on where
    /// `#ref` is in the template.
    pub queries: Vec<Query>,
    /// The class's `@HostListener`s, in the simple shape, in declaration order.
    pub listeners: Vec<Listener>,
    /// `@HostBinding`s of a component, in the order the official compiler
    /// collects them (accessors, then fields, each group in declaration order):
    /// the view's `detectHostChanges`.
    pub host_bindings: Vec<HostBinding>,
    /// `@ContentChild`/`@ContentChildren`, in the order of `directive.queries`
    /// (setters before fields, as the official visitor visits them). `None`:
    /// some outside the known shape.
    pub content_queries: Option<Vec<ContentQuery>>,
    /// Each instance field and getter of the class. The emitter needs this to
    /// choose between `interpolateString`, `interpolate` and
    /// `updateTextWithPrimitive`, which the official compiler decides by the
    /// static type of the template expression and by whether it is mutable. A
    /// static member is left out: the official compiler reads it through the
    /// class (`import1.X.name`), a shape the converter does not write.
    pub members: std::collections::HashMap<String, Member>,
    /// `@Input`s, in the order of the official `inputs` map — the order in
    /// which the inputs of a node are written (`_SortInputsVisitor`).
    pub inputs: Vec<Input>,
    /// Methods of the class, kept apart from the fields: a method can only
    /// appear as a call target (`titulo()`), and its type is the return type.
    /// If they were in the same map, `{{ titulo }}` (tear-off) would be
    /// interpolated as if it were the return value.
    pub methods: std::collections::HashMap<String, String>,
    /// Positional parameters of each instance method: that is what decides, in
    /// a tear-off (`(click)="salvar"`), between `salvar()` and `salvar($event)`
    /// (`rewriteTearOff`).
    pub arities: std::collections::HashMap<String, usize>,
}

/// Um `@Input`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    /// Binding name in the template (the alias, if any).
    pub name: String,
    /// Member that receives the value.
    pub field: String,
}

impl Component {
    /// The member that receives the input `name` of the template.
    pub fn input(&self, name: &str) -> Option<&Input> {
        self.inputs.iter().find(|e| e.name == name)
    }
}

/// ngdart lifecycle hooks implemented by the component. Each one has its fixed
/// place in the host view's `detectChangesInternal`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Hooks {
    pub on_init: bool,
    pub on_destroy: bool,
    pub do_check: bool,
    pub after_content_init: bool,
    pub after_content_checked: bool,
    pub after_view_init: bool,
    pub after_view_checked: bool,
    /// `AfterChanges`: does not appear in the component's own file, but changes
    /// the change detection of whoever uses it with `@Input`.
    pub after_changes: bool,
}

impl Hooks {
    /// Does any hook put code in the host view?
    pub fn any(&self) -> bool {
        self.on_init
            || self.on_destroy
            || self.do_check
            || self.after_content_init
            || self.after_content_checked
            || self.after_view_init
            || self.after_view_checked
    }

    /// `firstCheck` is only declared when some hook uses it.
    pub fn uses_first_check(&self) -> bool {
        self.on_init || self.after_content_init || self.after_view_init
    }

    /// Is there change detection in the host view? (`ngOnDestroy` alone only
    /// generates `destroyInternal`.)
    pub fn has_detection(&self) -> bool {
        self.on_init
            || self.do_check
            || self.after_content_init
            || self.after_content_checked
            || self.after_view_init
            || self.after_view_checked
    }
}

/// A field or getter of the component class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// Type as written, or inferred from the initializer; empty when unknown
    /// (`final x = A.b;`).
    pub ty: String,
    /// `final` (or getter): counts as immutable under ngcompiler's
    /// `isImmutable` rule, which decides whether a primitive value takes the
    /// fast path.
    pub immutable: bool,
    /// `static` member: the template reads it unqualified, and the official
    /// compiler writes it through the class (`import1.Class.name`), with type
    /// `dynamic` (the `_TypeResolver` does not resolve statics).
    pub is_static: bool,
}

/// A component's `@HostBinding('class.x')`/`@HostBinding('attr.x')`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostBinding {
    /// `class.ativo`, `attr.role`.
    pub name: String,
    /// The field or getter read.
    pub member: String,
    /// `final` field: written once, on the first check (`isImmutable`).
    pub immutable: bool,
    /// Static member: written once, in the view constructor (case j96).
    pub is_static: bool,
}

/// Um `@ViewChild('ref') T? field;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// Field that receives the result.
    pub property: String,
    /// The reference looked up in the template (`#ref`).
    pub reference: String,
    /// Field type, as written (`html.DivElement?`). It decides the value: an
    /// `Element` receives the node itself, any other type an `ElementRef`
    /// (`isElementType` in `find_components.dart`). In `@ViewChildren`, the
    /// type argument of the list.
    pub ty: String,
    /// `@ViewChildren`: the field receives the list of all results.
    pub list: bool,
    /// `@ViewChild(Type)`: `reference` is the name of the type looked up.
    pub by_type: bool,
    /// `read: T`, as written: the token of the provider of the found node that
    /// becomes the value (`queryWithRead.read` in `compile_element.dart`).
    pub read: Option<String>,
}

/// A `@ContentChild`/`@ContentChildren`, as the user of the component needs it:
/// with no result in the content, the list receives `[]` and the single one
/// receives nothing (`_createUpdates`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentQuery {
    /// Field or setter that receives the result.
    pub field: String,
    /// `@ContentChildren`.
    pub list: bool,
    /// The type looked up, as written (`LiTabComponent`, `li.X`), or the
    /// reference (`'name'`, with `name` here and `reference` set).
    pub target: String,
    pub reference: bool,
    /// `descendants:` — without it, only content one directive away matches
    /// (`_getQueriesFor`).
    pub descendants: bool,
    /// `read:` — the value read is another token of the found node: the type
    /// name, as written.
    pub read: Option<String>,
}

/// A component's `@HostListener`: the event and the text of the handler the
/// official compiler builds (`_addHostListener`: `method(args)`, with `$event`
/// deduced when there are no `args` and the method has one parameter). The text
/// goes through the same handler converter as the template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    pub event: String,
    pub handler: String,
}

/// A parameter of the component constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Tipo como escrito (`Element`, `RestConfig`, `List<int>`).
    pub ty: Option<String>,
    pub name: String,
    pub named: bool,
    /// `@Optional`, `@Inject(...)`, `@Self`… change the injection.
    pub annotated: bool,
    /// `@Optional()`: `injectorGetOptional`, or `null` where there is no
    /// provider.
    pub optional: bool,
    /// `@Self()`: only the node itself.
    pub self_only: bool,
    /// `@Host()`: does not leave the component's view (`_getDependency`).
    pub host: bool,
    /// `@SkipSelf()`: starts at the node above.
    pub skip_self: bool,
    /// Another annotation (`@Inject`, `@Attribute`, unknown): no translation
    /// yet.
    pub other_annotation: bool,
}

/// Value of a named argument, when it is a string literal without
/// interpolation.
///
/// In a multi-line string, each source line ending (`\r\n`, `\r`) is `\n` in
/// the value (Dart specification, *Strings*); it is the value the analyzer
/// hands to ngcompiler (`li_highlight_component.dart`, with CRLF).
fn argument_text(tree: &ast::Ast, id: ast::ExprId) -> Option<String> {
    match &tree.expr(id).kind {
        ast::ExprKind::String(lit) => lit
            .constant_value()
            // Only the CRLF pair: the value is already decoded, and a lone `\r`
            // may be the source's `\r` escape, which stays.
            .map(|s| s.to_string_lossy().replace("\r\n", "\n")),
        ast::ExprKind::Parenthesized(inner) => argument_text(tree, *inner),
        _ => None,
    }
}

/// Where the contents of a `template:` start in the source (in UTF-16 code
/// units, as the analyzer counts): the `REF` positions are those of the string
/// value plus this. A single string (simple, raw or triple-quoted, with escapes
/// or not) counts from the contents (`contentsOffset`); adjacent strings count
/// from the start of the node (`offset`) — cases j53 and j63.
fn template_offset(tree: &ast::Ast, source: &str, id: ast::ExprId) -> Option<usize> {
    let span = tree.expr(id).span;
    let raw_text = source.get(span.start..span.end)?;
    let (start, end) = literal_contents(raw_text)?;
    let offset = if end == raw_text.len() {
        span.start + start
    } else {
        span.start
    };
    Some(source.get(..offset)?.encode_utf16().count())
}

/// The first string literal of `raw_text`: where the contents start (after the `r`
/// and the quotes; with triple quotes, after the first line if it only has
/// whitespace — Dart's rule for multi-line strings, case j53) and where the
/// literal ends.
fn literal_contents(raw_text: &str) -> Option<(usize, usize)> {
    let raw = raw_text.starts_with(['r', 'R']);
    let after_r = usize::from(raw);
    let rest = &raw_text[after_r..];
    let quote = ["'''", "\"\"\"", "'", "\""]
        .into_iter()
        .find(|q| rest.starts_with(q))?;
    let mut start = after_r + quote.len();
    let body = &raw_text[start..];
    // The end: the next equal quote that is not escaped (in a raw string, the
    // backslash escapes nothing).
    let mut i = 0;
    let bytes = body.as_bytes();
    let end = loop {
        if i >= body.len() {
            return None;
        }
        if !raw && bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i..].starts_with(quote.as_bytes()) {
            break start + i + quote.len();
        }
        i += 1;
    };
    if quote.len() == 3 {
        let without_blanks = body.trim_start_matches([' ', '\t']);
        if let Some(after) = without_blanks
            .strip_prefix("\r\n")
            .or_else(|| without_blanks.strip_prefix('\n'))
        {
            start += body.len() - after.len();
        }
    }
    Some((start, end))
}

/// List of literal strings (`styleUrls: ['a.css', 'b.css']`).
fn text_list(tree: &ast::Ast, id: ast::ExprId) -> Vec<String> {
    let ast::ExprKind::List { elements, .. } = &tree.expr(id).kind else {
        return Vec::new();
    };
    elements
        .iter()
        .filter_map(|e| match e {
            ast::CollectionElement::Expression(x) => argument_text(tree, *x),
            _ => None,
        })
        .collect()
}

/// Names written in a list literal (`directives: [A, li.B]`), and whether some
/// item is not a name.
fn list_names(tree: &ast::Ast, interner: &Interner, id: ast::ExprId) -> (Vec<String>, bool) {
    let ast::ExprKind::List { elements, .. } = &tree.expr(id).kind else {
        return (Vec::new(), true);
    };
    let mut names = Vec::new();
    let mut unreadable = false;
    for e in elements.iter() {
        match e {
            // `...list`: the official constant value is the flattened list; its
            // name is expanded in place (`Designated::List`).
            ast::CollectionElement::Expression(x)
            | ast::CollectionElement::Spread { value: x, .. } => {
                match crate::resolution::qualified_name(tree, interner, *x) {
                    Some(n) => names.push(n),
                    None => unreadable = true,
                }
            }
            _ => unreadable = true,
        }
    }
    (names, unreadable)
}

/// `ChangeDetectionStrategy.onPush` (the name in ngdart 8) — what changes the
/// initial state of the view.
fn e_on_push(tree: &ast::Ast, source: &str, id: ast::ExprId) -> bool {
    let span = tree.expr(id).span;
    source
        .get(span.start..span.end)
        .is_some_and(|t| t.rsplit('.').next().map(str::trim) == Some("onPush"))
}

/// The value of `encapsulation:` when written as `ViewEncapsulation.x` (with or
/// without an import prefix): `emulated` or `none`, both from ngdart 8. Any
/// other shape (constant, expression) gives `None`.
fn encapsulation_value<'a>(tree: &ast::Ast, source: &'a str, id: ast::ExprId) -> Option<&'a str> {
    let span = tree.expr(id).span;
    let text = source.get(span.start..span.end)?.trim();
    let (before, value) = text.rsplit_once('.')?;
    let ty = before.rsplit('.').next()?.trim();
    (ty == "ViewEncapsulation" && matches!(value.trim(), "emulated" | "none")).then(|| value.trim())
}

/// Extracts the `@Component` of an annotated class.
pub fn read_component(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    class: &ast::ClassDecl,
    annotation: &ast::Annotation,
) -> Component {
    read_annotated(tree, source, interner, class, annotation, true)
}

/// Extracts the `@Directive` of an annotated class: what its users need to know
/// about it (selector, inputs, outputs, constructor, lifecycle).
pub fn read_directive(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    class: &ast::ClassDecl,
    annotation: &ast::Annotation,
) -> Component {
    read_annotated(tree, source, interner, class, annotation, false)
}

/// A `@Pipe`, as the official `PipeVisitor` reads it: the name, whether it is
/// pure and the type of `transform` (`fromFunctionType`) — it is where the type
/// of each call's `pureProxyN` comes from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pipe {
    pub class: String,
    /// `@Pipe('name')`.
    pub name: String,
    /// `pure:`, `true` when omitted (`coerceBool(.., defaultTo: true)`).
    pub pure: bool,
    /// Retorno do `transform`, como escrito (`String?`).
    pub return_type: String,
    /// Type of each positional parameter of `transform`, in order; without a
    /// written type, `None` (the inferred type of an override cannot be seen
    /// from here).
    pub params: Vec<Option<String>>,
    /// Why this pipe is not instantiated by the generator yet: constructor
    /// injection (`createPipeInstance` asks the injector), `OnDestroy`
    /// (`bindPipeDestroyLifecycleCallbacks`), inherited `transform`, no return
    /// type or a named parameter.
    pub unsupported: Option<&'static str>,
    /// The constructor receives only the `ChangeDetectorRef`: the view that
    /// creates it passes itself (`o.thisExpr` of `createPipeInstance`).
    pub detector: bool,
    /// `OnDestroy`: the view that creates it calls `ngOnDestroy()` when
    /// destroyed.
    pub destroys: bool,
}

/// Extracts the `@Pipe` of an annotated class.
pub fn read_pipe(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    class: &ast::ClassDecl,
    annotation: &ast::Annotation,
) -> Pipe {
    let mut p = Pipe {
        class: interner.resolve(class.name.sym).to_string(),
        pure: true,
        ..Default::default()
    };
    let mut read_name = false;
    if let Some(args) = &annotation.arguments {
        for a in args.args.iter() {
            match a.name.as_ref().map(|n| interner.resolve(n.sym)) {
                None if !read_name => {
                    read_name = true;
                    match argument_text(tree, a.value) {
                        Some(n) => p.name = n,
                        None => p.unsupported = Some("@Pipe name that is not a literal"),
                    }
                }
                Some("name") if !read_name => {
                    read_name = true;
                    match argument_text(tree, a.value) {
                        Some(n) => p.name = n,
                        None => p.unsupported = Some("@Pipe name that is not a literal"),
                    }
                }
                Some("pure") => match &tree.expr(a.value).kind {
                    ast::ExprKind::Bool(b) => p.pure = *b,
                    _ => p.unsupported = Some("pure: that is not a literal"),
                },
                _ => p.unsupported = Some("@Pipe argument outside the known ones"),
            }
        }
    }
    let params = constructor_params(tree, source, interner, class);
    match params.as_slice() {
        [] => {}
        [x] if x.ty.as_deref().map(|t| t.rsplit('.').next().unwrap_or(t))
            == Some("ChangeDetectorRef")
            && !x.annotated
            && !x.named =>
        {
            p.detector = true;
        }
        _ => p.unsupported = Some("pipe with constructor injection"),
    }
    p.destroys = class_hooks(tree, source, class).on_destroy;
    let mut found = false;
    for &id in &class.members {
        let ast::MemberKind::Method(f) = &tree.member(id).kind else {
            continue;
        };
        let function = tree.function(*f);
        if !matches!(function.kind, ast::FunctionKind::Function) || function.static_ {
            continue;
        }
        if function.name.map(|n| interner.resolve(n.sym)) != Some("transform") {
            continue;
        }
        found = true;
        match function.return_type {
            Some(t) => p.return_type = type_text(tree, source, t),
            None => p.unsupported = Some("transform without a return type"),
        }
        for param in function.parameters.as_deref().unwrap_or(&[]) {
            if matches!(param.kind, ast::ParameterKind::Named) {
                p.unsupported = Some("transform with a named parameter");
                continue;
            }
            p.params.push(param.ty.map(|t| type_text(tree, source, t)));
        }
    }
    if !found {
        p.unsupported = Some("inherited or missing transform");
    }
    p
}

fn read_annotated(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    class: &ast::ClassDecl,
    annotation: &ast::Annotation,
    is_component: bool,
) -> Component {
    let mut c = Component {
        class: interner.resolve(class.name.sym).to_string(),
        is_component,
        inherits: class.extends.is_some() || !class.with.is_empty(),
        type_params: class
            .type_params
            .iter()
            .map(|t| {
                (
                    interner.resolve(t.name.sym).to_string(),
                    t.bound.map(|b| type_text(tree, source, b)),
                )
            })
            .collect(),
        ..Default::default()
    };
    let mut exports = Vec::new();
    if let Some(args) = &annotation.arguments {
        for a in args.args.iter() {
            let Some(name) = a.name.as_ref().map(|n| interner.resolve(n.sym)) else {
                continue;
            };
            match name {
                "selector" => c.selector = argument_text(tree, a.value).unwrap_or_default(),
                "exportAs" => c.export_as = argument_text(tree, a.value),
                "template" => {
                    c.template = argument_text(tree, a.value);
                    c.template_offset = c
                        .template
                        .as_deref()
                        .and_then(|_| template_offset(tree, source, a.value));
                }
                "templateUrl" => c.template_url = argument_text(tree, a.value),
                "styleUrls" | "styles" => {
                    let texts = text_list(tree, a.value);
                    let items = match &tree.expr(a.value).kind {
                        ast::ExprKind::List { elements, .. } => Some(elements.len()),
                        _ => None,
                    };
                    c.unreadable_styles |= items != Some(texts.len());
                    if name == "styles" {
                        c.styles = texts;
                    } else {
                        c.style_urls = texts;
                    }
                }
                "changeDetection" => c.on_push = e_on_push(tree, source, a.value),
                "encapsulation" => {
                    c.without_encapsulation =
                        encapsulation_value(tree, source, a.value) == Some("none")
                }
                "preserveWhitespace" => {
                    c.preserve_whitespace =
                        matches!(tree.expr(a.value).kind, ast::ExprKind::Bool(true))
                }
                "directives" => {
                    (c.directives, c.unreadable_directives) = list_names(tree, interner, a.value)
                }
                "directiveTypes" => {
                    let sp = tree.expr(a.value).span;
                    match source.get(sp.start..sp.end).and_then(read_directive_types) {
                        Some(v) => c.directive_types = v,
                        None => c.unreadable_directive_types = true,
                    }
                }
                "pipes" => (c.pipes, c.unreadable_pipes) = list_names(tree, interner, a.value),
                "exports" => exports = list_names(tree, interner, a.value).0,
                "providers" | "viewProviders" => c.with_providers |= !empty_list(tree, a.value),
                "visibility" => c.written_visibility = true,
                _ => {}
            }
        }
    }
    c.params = constructor_params(tree, source, interner, class);
    c.members = member_types(tree, source, interner, class);
    c.methods = method_types(tree, source, interner, class);
    c.arities = method_arities(tree, interner, class);
    // `exports:` puts static names in the expression scope, and they win over
    // the member with the same name (`_matchExport` before the implicit
    // receiver). A member shadowed like this leaves the map, and using it is
    // refused.
    for n in &exports {
        let simple = n.split('.').next().unwrap_or(n);
        c.members.remove(simple);
        c.methods.remove(simple);
        c.arities.remove(simple);
    }
    c.exports = exports;
    c.inputs = class_inputs(tree, interner, class);
    c.outputs = class_outputs(tree, interner, class);
    c.hooks = class_hooks(tree, source, class);
    c.not_understood =
        what_we_do_not_understand(tree, source, interner, class, annotation, is_component);
    c.binds_host = has_annotation(tree, interner, class, &["HostBinding"]);
    c.content_query = has_annotation(tree, interner, class, &["ContentChild", "ContentChildren"]);
    c.queries = class_queries(tree, source, interner, class, &mut c.not_understood);
    c.listeners = class_listeners(tree, interner, class, &mut c.not_understood);
    // The inherited ones go into the official list and cannot be seen from
    // here: the view completes the list from the metadata read from the
    // program, or refuses.
    if is_component && c.binds_host {
        c.host_bindings = host_bindings(tree, interner, class, &mut c.not_understood);
    }
    c.content_queries = content_queries(tree, interner, class);
    if is_component && c.content_query && c.content_queries.is_none() {
        c.not_understood.push(refusal(
            Reason::ContentChild,
            "@ContentChild outside the shape",
        ));
    }
    c
}

/// The class's `@ContentChild`/`@ContentChildren`: setters, then fields, each
/// group in declaration order. `None` if any is outside the shape
/// `@ContentChild(Type)`/`@ContentChild('ref')` on a field or setter.
fn content_queries(
    tree: &ast::Ast,
    interner: &Interner,
    class: &ast::ClassDecl,
) -> Option<Vec<ContentQuery>> {
    let mut setters = Vec::new();
    let mut fields = Vec::new();
    for &id in &class.members {
        let member = tree.member(id);
        for a in member.metadata.iter() {
            let list = match crate::annotation_name(a, interner).as_str() {
                "ContentChild" => false,
                "ContentChildren" => true,
                _ => continue,
            };
            let first = a
                .arguments
                .as_ref()
                .and_then(|args| args.args.iter().find(|x| x.name.is_none()))?;
            let (target, reference) = match argument_text(tree, first.value) {
                Some(t) => (t, true),
                None => (
                    crate::resolution::qualified_name(tree, interner, first.value)?,
                    false,
                ),
            };
            // `ContentChildren(descendants: true)` by default in ngdart 8;
            // `ContentChild` always. `read:` changes the value read.
            let mut descendants = true;
            let mut read = None;
            for x in a.arguments.as_ref().map(|g| &g.args[..]).unwrap_or(&[]) {
                match x.name.map(|n| interner.resolve(n.sym)) {
                    None => {}
                    Some("descendants") if list => match &tree.expr(x.value).kind {
                        ast::ExprKind::Bool(b) => descendants = *b,
                        _ => return None,
                    },
                    Some("read") => {
                        read = Some(crate::resolution::qualified_name(tree, interner, x.value)?)
                    }
                    _ => return None,
                }
            }
            match &member.kind {
                ast::MemberKind::Field(l) if !l.static_ && l.variables.len() == 1 => {
                    fields.push(ContentQuery {
                        field: interner.resolve(l.variables[0].name.sym).to_string(),
                        list,
                        target,
                        reference,
                        descendants,
                        read: read.clone(),
                    });
                }
                ast::MemberKind::Method(f) => {
                    let function = tree.function(*f);
                    match (function.kind, function.name, function.static_) {
                        (ast::FunctionKind::Setter, Some(n), false) => setters.push(ContentQuery {
                            field: interner.resolve(n.sym).to_string(),
                            list,
                            target,
                            reference,
                            descendants,
                            read: read.clone(),
                        }),
                        _ => return None,
                    }
                }
                _ => return None,
            }
        }
    }
    setters.extend(fields);
    Some(setters)
}

/// Does any member of the class have one of these annotations?
fn has_annotation(
    tree: &ast::Ast,
    interner: &Interner,
    class: &ast::ClassDecl,
    names: &[&str],
) -> bool {
    class.members.iter().any(|&m| {
        tree.member(m)
            .metadata
            .iter()
            .any(|a| names.contains(&crate::annotation_name(a, interner).as_str()))
    })
}

/// Arguments of `@Component` whose effect the generator knows. `pipes:` is here
/// because the list does not change the view; using a pipe in the template
/// does, and that is checked against the template. `exports:` only puts names
/// in the expression scope (see [`ler`]).
const KNOWN_ARGUMENTS: &[&str] = &[
    "selector",
    // Only decides which class a user's `#ref="name"` points to
    // (`identifierForReference`); read in [`Component::export_as`].
    "exportAs",
    "template",
    "templateUrl",
    "styleUrls",
    "styles",
    "changeDetection",
    "directives",
    "directiveTypes",
    "exports",
    "pipes",
    // Only decides `injectorGetInternal`; read from the program metadata
    // (`metadata.rs`), case j41.
    "visibility",
    // Private providers of the component's element: read from the program
    // metadata (`metadata.rs`) and written in the host view; on the node of the
    // child's user, still refused (`view.rs`).
    "viewProviders",
];

/// Arguments of `@Directive` whose effect the generator knows.
const DIRECTIVE_ARGUMENTS: &[&str] = &["selector", "exportAs"];

/// Lifecycle interfaces implemented, through the `implements` and `with`
/// clauses. (Those of ngrouter — `OnActivate`, `CanDeactivate` — do not touch
/// the view; nor does `AfterChanges` appear in the component's own file.)
fn class_hooks(tree: &ast::Ast, source: &str, class: &ast::ClassDecl) -> Hooks {
    let mut g = Hooks::default();
    for t in class.implements.iter().chain(class.with.iter()) {
        let s = tree.ty(*t).span;
        let text = source.get(s.start..s.end).unwrap_or("");
        match text.split(['<', '.']).next_back().unwrap_or(text).trim() {
            "OnInit" => g.on_init = true,
            "OnDestroy" => g.on_destroy = true,
            "DoCheck" => g.do_check = true,
            "AfterContentInit" => g.after_content_init = true,
            "AfterContentChecked" => g.after_content_checked = true,
            "AfterViewInit" => g.after_view_init = true,
            "AfterViewChecked" => g.after_view_checked = true,
            "AfterChanges" => g.after_changes = true,
            _ => {}
        }
    }
    g
}

/// Everything in this class the generator does not know how to translate yet,
/// without stopping at the first. Refusing is mandatory: generating without it
/// produces a **wrong** file.
///
/// `@ViewChild` and `@HostListener` are not here: each has its own reading
/// ([`class_queries`], [`class_listeners`]), which refuses what it
/// does not know.
fn what_we_do_not_understand(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    class: &ast::ClassDecl,
    annotation: &ast::Annotation,
    is_component: bool,
) -> Vec<Refusal> {
    let mut unsupported = Vec::new();
    let ann = if is_component {
        "@Component"
    } else {
        "@Directive"
    };
    if let Some(args) = &annotation.arguments {
        for a in args.args.iter() {
            let Some(name) = a.name.as_ref().map(|n| interner.resolve(n.sym)) else {
                unsupported.push(refusal(
                    Reason::NotUnderstood,
                    format!("positional argument in {ann}"),
                ));
                continue;
            };
            match name {
                // The empty list does not change the view (corpus case b18) —
                // and it is 57 of the 70 `providers:` of new_sali.
                "providers" if empty_list(tree, a.value) => {}
                // Those of the component go to the host view, which resolves
                // them through the metadata read from the program (`view.rs`).
                "providers" if is_component => {}
                "providers" => unsupported.push(refusal(
                    Reason::Providers,
                    format!("{ann}(.., providers: [..])"),
                )),
                // `ViewEncapsulation.emulated`/`.none` written: the effect is
                // known (see [`Component::without_encapsulation`]).
                "encapsulation"
                    if is_component && encapsulation_value(tree, source, a.value).is_some() => {}
                "encapsulation" => unsupported.push(refusal(
                    Reason::Encapsulation,
                    "@Component(.., encapsulation: ..)",
                )),
                // Literal `true`/`false`: the whitespace mode of the template.
                "preserveWhitespace"
                    if is_component
                        && matches!(tree.expr(a.value).kind, ast::ExprKind::Bool(_)) => {}
                n if is_component && KNOWN_ARGUMENTS.contains(&n) => {}
                n if !is_component && DIRECTIVE_ARGUMENTS.contains(&n) => {}
                n => unsupported.push(refusal(
                    Reason::NotUnderstood,
                    format!("{ann}(.., {n}: ..)"),
                )),
            }
        }
    }
    for &id in &class.members {
        let member = tree.member(id);
        for a in member.metadata.iter() {
            let name = crate::annotation_name(a, interner);
            let reason = match name.as_str() {
                "HostBinding" if !is_component => Reason::HostBindingOnComponent,
                // On a component, the host assigns the empty result (see
                // [`content_queries`]).
                "ContentChild" | "ContentChildren" if !is_component => Reason::ContentChild,
                _ => continue,
            };
            unsupported.push(refusal(reason, format!("@{name}")));
        }
    }
    unsupported
}

/// Literal `[]`, without any element (a comment inside does not count).
fn empty_list(tree: &ast::Ast, id: ast::ExprId) -> bool {
    matches!(&tree.expr(id).kind, ast::ExprKind::List { elements, .. } if elements.is_empty())
}

/// The class's `@ViewChild`s. Only the shape `@ViewChild('ref') T? field;` — a
/// string selector, without `read:`, on a field with a written type — becomes a
/// query. The rest is refused right here, with the reason.
fn class_queries(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    class: &ast::ClassDecl,
    unsupported: &mut Vec<Refusal>,
) -> Vec<Query> {
    // Like the official `_queries`: the accessors (setters), then the fields,
    // each group in declaration order.
    let mut setters = Vec::new();
    let mut fields = Vec::new();
    for &id in &class.members {
        let member = tree.member(id);
        for a in member.metadata.iter() {
            let list = match crate::annotation_name(a, interner).as_str() {
                "ViewChild" => false,
                "ViewChildren" => true,
                _ => continue,
            };
            match simple_query(tree, source, interner, member, a, list) {
                Ok(c) if matches!(member.kind, ast::MemberKind::Method(_)) => setters.push(c),
                Ok(c) => fields.push(c),
                Err(r) => unsupported.push(r),
            }
        }
    }
    setters.extend(fields);
    setters
}

/// The `@HostBinding`s declared in the component's own class, in the order of
/// `DirectiveVisitor`: accessors, then fields. The name is the literal text of
/// the argument or, without an argument, that of the member
/// (`hostProperties[bindingName ?? memberName]`); getter or field, instance or
/// static. The shape of each name (`class.x`, `attr.x`, `style.x`, property) is
/// checked by the view, which also adds the inherited ones.
fn host_bindings(
    tree: &ast::Ast,
    interner: &Interner,
    class: &ast::ClassDecl,
    unsupported: &mut Vec<Refusal>,
) -> Vec<HostBinding> {
    let mut accessors = Vec::new();
    let mut fields = Vec::new();
    let outside_shape = || {
        refusal(
            Reason::HostBindingOnComponent,
            "@HostBinding outside a field or getter, or with an unreadable name",
        )
    };
    for &id in &class.members {
        let member = tree.member(id);
        for a in member.metadata.iter() {
            if crate::annotation_name(a, interner) != "HostBinding" {
                continue;
            }
            // `Some(None)`: without an argument — the name is that of the
            // member.
            let name = match a.arguments.as_ref().map(|args| &args.args[..]) {
                None | Some([]) => Some(None),
                Some([x]) if x.name.is_none() => argument_text(tree, x.value).map(Some),
                _ => None,
            };
            let Some(name) = name else {
                unsupported.push(outside_shape());
                continue;
            };
            match &member.kind {
                ast::MemberKind::Field(l) if l.variables.len() == 1 && (l.static_ || !l.const_) => {
                    let m = interner.resolve(l.variables[0].name.sym).to_string();
                    fields.push(HostBinding {
                        name: name.unwrap_or_else(|| m.clone()),
                        member: m,
                        immutable: l.final_ || l.const_,
                        is_static: l.static_,
                    });
                }
                ast::MemberKind::Method(f) => {
                    let function = tree.function(*f);
                    match (function.kind, function.name) {
                        (ast::FunctionKind::Getter, Some(n)) => {
                            let m = interner.resolve(n.sym).to_string();
                            accessors.push(HostBinding {
                                name: name.unwrap_or_else(|| m.clone()),
                                member: m,
                                immutable: false,
                                is_static: function.static_,
                            });
                        }
                        _ => unsupported.push(outside_shape()),
                    }
                }
                _ => unsupported.push(outside_shape()),
            }
        }
    }
    accessors.extend(fields);
    accessors
}

/// The class's `@HostListener`s, in the declaration order of the methods
/// (`DirectiveVisitor` visits the methods in order). The handler is the text of
/// `_addHostListener` (`find_components.dart`): without `args`, a method with
/// one parameter receives `$event`.
fn class_listeners(
    tree: &ast::Ast,
    interner: &Interner,
    class: &ast::ClassDecl,
    unsupported: &mut Vec<Refusal>,
) -> Vec<Listener> {
    let mut output: Vec<Listener> = Vec::new();
    for &id in &class.members {
        let member = tree.member(id);
        for a in member.metadata.iter() {
            if crate::annotation_name(a, interner) != "HostListener" {
                continue;
            }
            match listener(tree, interner, member, a) {
                Some(o) if !output.iter().any(|x| x.event == o.event) => output.push(o),
                // Two listeners of the same event: the official map keeps the
                // last one, in place of the first. Not yet.
                Some(_) => unsupported.push(refusal(
                    Reason::HostListenerOnComponent,
                    "two @HostListeners of the same event",
                )),
                None => unsupported.push(refusal(
                    Reason::HostListenerOnComponent,
                    "@HostListener of a non-native event or outside the shape `m(args)`",
                )),
            }
        }
    }
    output
}

/// Reads a `@HostListener`, or `None` if it is outside the known shape.
fn listener(
    tree: &ast::Ast,
    interner: &Interner,
    member: &ast::Member,
    annotation: &ast::Annotation,
) -> Option<Listener> {
    let ast::MemberKind::Method(f) = &member.kind else {
        return None;
    };
    let function = tree.function(*f);
    if function.static_ || !matches!(function.kind, ast::FunctionKind::Function) {
        return None;
    }
    let method = interner.resolve(function.name?.sym).to_string();
    // `element.parameters.length`: all of them, optional and named included.
    let params = function.parameters.as_ref().map_or(0, |p| p.len());
    let args = annotation.arguments.as_ref()?;
    let (event, list) = match &args.args[..] {
        [e] if e.name.is_none() => (e, None),
        [e, l] if e.name.is_none() && l.name.is_none() => (e, Some(l.value)),
        _ => return None,
    };
    let event = argument_text(tree, event.value)?;
    // An event that is not a DOM one (`document:click`, `keyup.enter`) goes
    // through `eventManager` and, with `:`, is not even accepted by the
    // official compiler; not yet.
    if !crate::view::native_event(&event) {
        return None;
    }
    let mut arguments = match list {
        None => Vec::new(),
        Some(l) => {
            let ast::ExprKind::List { elements, .. } = &tree.expr(l).kind else {
                return None;
            };
            let texts = text_list(tree, l);
            if texts.len() != elements.len() {
                return None;
            }
            texts
        }
    };
    if arguments.is_empty() && params == 1 {
        arguments.push("$event".into());
    }
    Some(Listener {
        event,
        handler: format!("{method}({})", arguments.join(", ")),
    })
}

/// Reads a `@ViewChild` in the shape the generator knows, or says why not.
fn simple_query(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    member: &ast::Member,
    annotation: &ast::Annotation,
    is_list: bool,
) -> Result<Query, Refusal> {
    let args = annotation
        .arguments
        .as_ref()
        .ok_or_else(|| refusal(Reason::NotUnderstood, "@ViewChild without an argument"))?;
    // `read: T` replaces the value with a provider of the node: kept as the
    // written name, which the view resolves (`ElementRef`,
    // `Element`/`HtmlElement`). `first:`, `descendants:` and a `read:` that is
    // not a simple name, not yet.
    let mut read = None;
    let mut positional = Vec::new();
    for x in args.args.iter() {
        match x.name.map(|n| interner.resolve(n.sym)) {
            None => positional.push(x),
            Some("read") => match &tree.expr(x.value).kind {
                ast::ExprKind::Identifier(n) if read.is_none() => {
                    read = Some(interner.resolve(n.sym).to_string());
                }
                _ => return Err(refusal(Reason::ViewChildOnChild, "@ViewChild(.., read: T)")),
            },
            Some(_) => return Err(refusal(Reason::NotUnderstood, "@ViewChild with options")),
        }
    }
    let [single] = positional[..] else {
        return Err(refusal(
            Reason::NotUnderstood,
            "@ViewChild with only a named argument",
        ));
    };
    // A type selector (`@ViewChild(OtherComp)`) queries a component or
    // directive, not an element: only the simple name, resolved later.
    let (reference, by_type) = match argument_text(tree, single.value) {
        Some(r) => (r, false),
        None => match &tree.expr(single.value).kind {
            ast::ExprKind::Identifier(n) => (interner.resolve(n.sym).to_string(), true),
            _ => {
                return Err(refusal(
                    Reason::ViewChildOnChild,
                    "@ViewChild(prefixo.Tipo)",
                ));
            }
        },
    };
    // `'a,b'` are two selectors in a single query.
    if reference.contains(',') || reference.trim() != reference || reference.is_empty() {
        return Err(refusal(Reason::NotUnderstood, "@ViewChild('a,b')"));
    }
    // An instance field with a written type, or an instance setter with the
    // type written on the parameter (the `_ctx.x = v` is the same, case j81).
    let (property, t) = match &member.kind {
        ast::MemberKind::Field(list) => {
            if list.static_ || list.final_ || list.const_ || list.late || list.variables.len() != 1
            {
                return Err(refusal(
                    Reason::NotUnderstood,
                    "@ViewChild on a final/late/static field",
                ));
            }
            let Some(t) = list.ty else {
                return Err(refusal(
                    Reason::NotUnderstood,
                    "@ViewChild on an untyped field",
                ));
            };
            (interner.resolve(list.variables[0].name.sym).to_string(), t)
        }
        ast::MemberKind::Method(f) => {
            let function = tree.function(*f);
            let (ast::FunctionKind::Setter, Some(name), false) =
                (function.kind, function.name, function.static_)
            else {
                return Err(refusal(
                    Reason::NotUnderstood,
                    "@ViewChild on a method that is not a setter",
                ));
            };
            let Some(t) = function
                .parameters
                .as_deref()
                .and_then(|ps| ps.first())
                .and_then(|p| p.ty)
            else {
                return Err(refusal(
                    Reason::NotUnderstood,
                    "@ViewChild on an untyped setter",
                ));
            };
            (interner.resolve(name.sym).to_string(), t)
        }
        _ => {
            return Err(refusal(
                Reason::NotUnderstood,
                "@ViewChild outside a field or setter",
            ));
        }
    };
    let ty = type_text(tree, source, t);
    // `@ViewChildren`: `List<T>` (or `List<T>?`); `isElementType` looks at `T`.
    // Another collection, not yet.
    let ty = if is_list {
        let t = ty.trim().trim_end_matches('?');
        match t.strip_prefix("List<").and_then(|x| x.strip_suffix('>')) {
            Some(inside) if !inside.contains(['<', ',']) => inside.trim().to_string(),
            _ => {
                return Err(refusal(
                    Reason::DynamicViewChild,
                    "@ViewChildren on a field that is not List<T>",
                ));
            }
        }
    } else {
        ty
    };
    Ok(Query {
        property,
        reference,
        ty,
        list: is_list,
        by_type,
        read,
    })
}

/// Parameters of the generative constructor (the unnamed one). Without a
/// declared constructor, the class has the implicit constructor without
/// parameters.
fn constructor_params(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    class: &ast::ClassDecl,
) -> Vec<Parameter> {
    for &id in &class.members {
        let member = tree.member(id);
        let ast::MemberKind::Constructor(ctor) = &member.kind else {
            continue;
        };
        if ctor.name.is_some() {
            continue; // a named constructor is not the one the view uses
        }
        let fields = field_types(tree, source, interner, class);
        // A named parameter does not take part in injection
        // (`_getCompileDiDependencyMetadata`, `compile_metadata.dart:298-302`).
        return ctor
            .parameters
            .iter()
            .filter(|p| !matches!(p.kind, ast::ParameterKind::Named))
            .map(|p| {
                let name = p
                    .name
                    .map(|n| interner.resolve(n.sym).to_string())
                    .unwrap_or_default();
                // `C(this.x)` writes no type at all: the parameter type is that
                // of the field. That is how almost every ngdart component
                // receives its dependencies, and without this the injection
                // does not come out.
                let ty = match p.ty {
                    Some(t) => Some(type_text(tree, source, t)),
                    None if p.this_ || p.super_ => fields.get(&name).cloned(),
                    None => None,
                };
                let names: Vec<String> = p
                    .metadata
                    .iter()
                    .map(|a| crate::annotation_name(a, interner))
                    .collect();
                let has = |n: &str| names.iter().any(|x| x == n);
                Parameter {
                    ty,
                    name,
                    named: matches!(p.kind, ast::ParameterKind::Named),
                    annotated: !p.metadata.is_empty(),
                    optional: has("Optional"),
                    self_only: has("Self"),
                    host: has("Host"),
                    skip_self: has("SkipSelf"),
                    other_annotation: names
                        .iter()
                        .any(|n| !matches!(n.as_str(), "Optional" | "Self" | "Host" | "SkipSelf")),
                }
            })
            .collect();
    }
    Vec::new()
}

/// Source text of a type, as written.
fn type_text(tree: &ast::Ast, source: &str, t: ast::TypeId) -> String {
    let s = tree.ty(t).span;
    source.get(s.start..s.end).unwrap_or("").to_string()
}

/// Type of each instance field and getter of the class, so the emitter knows
/// the static type of `{{ name }}`.
///
/// The immutability is that of the official `isImmutable`: the value comes from
/// `lookUpGetter(n).variable`, and only a real field (`!isSynthetic`) that is
/// `final`/`const` is immutable. In a hand-written getter the `variable` is
/// synthetic: **mutable**.
pub(crate) fn member_types(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    class: &ast::ClassDecl,
) -> std::collections::HashMap<String, Member> {
    let mut output = std::collections::HashMap::new();
    for &id in &class.members {
        match &tree.member(id).kind {
            ast::MemberKind::Field(list) => {
                let immutable = list.final_ || list.const_;
                for v in list.variables.iter() {
                    // Without a written type (`var x = Foo()`), the type is
                    // what the analyzer infers from the initializer — only in
                    // the shapes where it is certain.
                    let ty = match list.ty {
                        Some(t) => type_text(tree, source, t),
                        // Outside those shapes the member exists, but without a
                        // type (empty text): whoever needs the type refuses.
                        None => v
                            .initializer
                            .and_then(|e| inferred_type(tree, source, interner, e))
                            .unwrap_or_default(),
                    };
                    output.insert(
                        interner.resolve(v.name.sym).to_string(),
                        Member {
                            ty,
                            immutable,
                            is_static: list.static_,
                        },
                    );
                }
            }
            ast::MemberKind::Method(f) => {
                let function = tree.function(*f);
                if !matches!(function.kind, ast::FunctionKind::Getter) {
                    continue;
                }
                let (Some(name), Some(t)) = (function.name, function.return_type) else {
                    continue;
                };
                output.insert(
                    interner.resolve(name.sym).to_string(),
                    Member {
                        ty: type_text(tree, source, t),
                        immutable: false,
                        is_static: function.static_,
                    },
                );
            }
            _ => {}
        }
    }
    output
}

/// The binding name of `@Input('alias')`/`@Output('alias')`, or that of the
/// member.
fn alias(tree: &ast::Ast, a: &ast::Annotation) -> Option<String> {
    a.arguments.as_ref().and_then(|args| {
        args.args
            .first()
            .and_then(|arg| match &tree.expr(arg.value).kind {
                ast::ExprKind::String(lit) => lit.constant_value().map(|s| s.to_string_lossy()),
                _ => None,
            })
    })
}

/// `@Input()` of each field or setter, in the order of the official `inputs`
/// map (`_collectInheritableMetadataOn`, in `find_components.dart`): the fields
/// first, in declaration order, then the setters. Inherited ones are not
/// included — a binding to them is refused because the input is not found.
fn class_inputs(tree: &ast::Ast, interner: &Interner, class: &ast::ClassDecl) -> Vec<Input> {
    let mut fields = Vec::new();
    let mut setters = Vec::new();
    for &id in &class.members {
        let member = tree.member(id);
        let Some(a) = member
            .metadata
            .iter()
            .find(|a| crate::annotation_name(a, interner) == "Input")
        else {
            continue;
        };
        let alias = alias(tree, a);
        match &member.kind {
            ast::MemberKind::Field(list) if !list.static_ => {
                for v in list.variables.iter() {
                    let name = interner.resolve(v.name.sym).to_string();
                    fields.push(Input {
                        name: alias.clone().unwrap_or_else(|| name.clone()),
                        field: name,
                    });
                }
            }
            ast::MemberKind::Method(f) => {
                let function = tree.function(*f);
                if let (ast::FunctionKind::Setter, Some(n), false) =
                    (function.kind, function.name, function.static_)
                {
                    let name = interner.resolve(n.sym).to_string();
                    setters.push(Input {
                        name: alias.clone().unwrap_or_else(|| name.clone()),
                        field: name,
                    });
                }
            }
            _ => {}
        }
    }
    fields.extend(setters);
    fields
}

/// `@Output()` of each field or getter: name in the template -> member.
fn class_outputs(
    tree: &ast::Ast,
    interner: &Interner,
    class: &ast::ClassDecl,
) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let mut getters = Vec::new();
    for &id in &class.members {
        let member = tree.member(id);
        let Some(a) = member
            .metadata
            .iter()
            .find(|a| crate::annotation_name(a, interner) == "Output")
        else {
            continue;
        };
        let alias = alias(tree, a);
        match &member.kind {
            ast::MemberKind::Field(list) if !list.static_ => {
                for v in list.variables.iter() {
                    let name = interner.resolve(v.name.sym).to_string();
                    fields.push((alias.clone().unwrap_or_else(|| name.clone()), name));
                }
            }
            ast::MemberKind::Method(f) => {
                let function = tree.function(*f);
                if let (ast::FunctionKind::Getter, Some(n), false) =
                    (function.kind, function.name, function.static_)
                {
                    let name = interner.resolve(n.sym).to_string();
                    getters.push((alias.clone().unwrap_or_else(|| name.clone()), name));
                }
            }
            _ => {}
        }
    }
    // The order of the `outputs` map: getters (the accessors are visited before
    // the fields by the class's `visitChildren`), then fields.
    getters.extend(fields);
    getters
}

/// Positional parameters (required and optional) of each instance method.
fn method_arities(
    tree: &ast::Ast,
    interner: &Interner,
    class: &ast::ClassDecl,
) -> std::collections::HashMap<String, usize> {
    let mut output = std::collections::HashMap::new();
    for &id in &class.members {
        let ast::MemberKind::Method(f) = &tree.member(id).kind else {
            continue;
        };
        let function = tree.function(*f);
        if !matches!(function.kind, ast::FunctionKind::Function) || function.static_ {
            continue;
        }
        let Some(name) = function.name else { continue };
        let positional = function.parameters.as_ref().map_or(0, |ps| {
            ps.iter()
                .filter(|p| !matches!(p.kind, ast::ParameterKind::Named))
                .count()
        });
        output.insert(interner.resolve(name.sym).to_string(), positional);
    }
    output
}

/// Return type of each instance method of the class.
fn method_types(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    class: &ast::ClassDecl,
) -> std::collections::HashMap<String, String> {
    let mut output = std::collections::HashMap::new();
    for &id in &class.members {
        let ast::MemberKind::Method(f) = &tree.member(id).kind else {
            continue;
        };
        let function = tree.function(*f);
        if !matches!(function.kind, ast::FunctionKind::Function) || function.static_ {
            continue;
        }
        let Some(name) = function.name else { continue };
        let ty = match function.return_type {
            Some(t) => type_text(tree, source, t),
            None => "dynamic".to_string(),
        };
        output.insert(interner.resolve(name.sym).to_string(), ty);
    }
    output
}

/// The type the analyzer infers for an initializer, in the shapes where it
/// depends on nothing outside the expression: a primitive literal without
/// interpolation, `Class(..)`, `prefix.Class(..)` and `new`/`const T(..)` (with
/// `T` as written). `Class.name(..)` may be a static method, and is left out.
pub(crate) fn inferred_type(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    e: ast::ExprId,
) -> Option<String> {
    let uppercase = |n: &ast::Name| interner.resolve(n.sym).starts_with(char::is_uppercase);
    match &tree.expr(e).kind {
        ast::ExprKind::String(lit) => lit.constant_value().map(|_| "String".to_string()),
        ast::ExprKind::Int(_) => Some("int".to_string()),
        ast::ExprKind::Double(_) => Some("double".to_string()),
        ast::ExprKind::Bool(_) => Some("bool".to_string()),
        ast::ExprKind::InstanceCreation { ty, .. } => Some(type_text(tree, source, *ty)),
        ast::ExprKind::Call { target, arguments } if arguments.type_args.is_empty() => {
            match &tree.expr(*target).kind {
                ast::ExprKind::Identifier(n) if uppercase(n) => {
                    Some(interner.resolve(n.sym).to_string())
                }
                ast::ExprKind::Property {
                    target: p,
                    name,
                    null_aware: false,
                } if uppercase(name) => match &tree.expr(*p).kind {
                    ast::ExprKind::Identifier(prefix) if !uppercase(prefix) => Some(format!(
                        "{}.{}",
                        interner.resolve(prefix.sym),
                        interner.resolve(name.sym)
                    )),
                    _ => None,
                },
                // `Class<A>.named(..)`: with the class's type arguments before
                // `.name(`, it can only be a constructor (a static method does
                // not take those of the class); the type is `Class<A>` as
                // written.
                ast::ExprKind::Property {
                    target: p,
                    null_aware: false,
                    ..
                } => match &tree.expr(*p).kind {
                    ast::ExprKind::TypeArguments { target: class, .. }
                        if matches!(&tree.expr(*class).kind,
                            ast::ExprKind::Identifier(n) if uppercase(n)) =>
                    {
                        let sp = tree.expr(*p).span;
                        source.get(sp.start..sp.end).map(|t| t.trim().to_string())
                    }
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}

/// Type of each field of the class, to resolve the `this.x` parameters.
fn field_types(
    tree: &ast::Ast,
    source: &str,
    interner: &Interner,
    class: &ast::ClassDecl,
) -> std::collections::HashMap<String, String> {
    let mut output = std::collections::HashMap::new();
    for &id in &class.members {
        let ast::MemberKind::Field(list) = &tree.member(id).kind else {
            continue;
        };
        let Some(t) = list.ty else { continue };
        let ty = type_text(tree, source, t);
        for v in list.variables.iter() {
            output.insert(interner.resolve(v.name.sym).to_string(), ty.clone());
        }
    }
    output
}

/// A `Typed<X>.of([..], on: 'ref')` of `directiveTypes:`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectiveType {
    /// The class as written (`MaterialSelectItemComponent`, `p.X`).
    pub class: String,
    /// The arguments: the text of each type, and `#T` as `T` (the component's
    /// type parameter, `fromTypeLink`).
    pub arguments: Vec<String>,
    /// `on:`: only on the node with that `#ref`.
    pub em: Option<String>,
}

/// Reads `[Typed<X>.of([#T, String], on: 'a'), Typed<Y<int>>()]` from the text
/// (`TypedReader`). `None` for whatever does not have this shape.
fn read_directive_types(text: &str) -> Option<Vec<DirectiveType>> {
    let t = text.trim();
    let t = t.strip_prefix("const").unwrap_or(t).trim();
    let t = t.strip_prefix('[')?.strip_suffix(']')?;
    let mut output = Vec::new();
    for item in split_at_top_level(t, ',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let item = item.strip_prefix("const").unwrap_or(item).trim();
        let rest = item.strip_prefix("Typed")?.trim_start();
        let rest = rest.strip_prefix('<')?;
        // The argument of `Typed<..>`, up to the closing `>`.
        let mut level = 1i32;
        let mut end = None;
        for (i, ch) in rest.char_indices() {
            match ch {
                '<' => level += 1,
                '>' => {
                    level -= 1;
                    if level == 0 {
                        end = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end?;
        let ty = rest[..end].trim();
        let after = rest[end + 1..].trim();
        let (class, mut arguments) = match ty.find('<') {
            Some(i) => (
                ty[..i].trim().to_string(),
                split_at_top_level(ty[i + 1..].strip_suffix('>')?, ',')
                    .into_iter()
                    .map(|a| a.trim().to_string())
                    .collect::<Vec<_>>(),
            ),
            None => (ty.to_string(), Vec::new()),
        };
        let (is_of, call) = match after.strip_prefix(".of") {
            Some(r) => (true, r.trim()),
            None => (false, after),
        };
        let inside = call.strip_prefix('(')?.strip_suffix(')')?;
        let mut em = None;
        for (k, part) in split_at_top_level(inside, ',').into_iter().enumerate() {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some(v) = part.strip_prefix("on:") {
                let v = v.trim();
                let v = v
                    .strip_prefix('\'')
                    .and_then(|v| v.strip_suffix('\''))
                    .or_else(|| v.strip_prefix('"').and_then(|v| v.strip_suffix('"')))?;
                em = Some(v.to_string());
            } else if is_of && k == 0 {
                if !arguments.is_empty() {
                    return None;
                }
                let list = part.strip_prefix('[')?.strip_suffix(']')?;
                for a in split_at_top_level(list, ',') {
                    let a = a.trim();
                    if a.is_empty() {
                        continue;
                    }
                    arguments.push(match a.strip_prefix('#') {
                        Some(symbol) => symbol.trim().to_string(),
                        // `Typed<..>` nested as an argument: not yet.
                        None if a.starts_with("Typed") => return None,
                        None => a.to_string(),
                    });
                }
            } else {
                return None;
            }
        }
        output.push(DirectiveType {
            class,
            arguments,
            em,
        });
    }
    Some(output)
}

/// Splits `text` at `sep` outside `<>`, `()`, `[]` and `{}`.
fn split_at_top_level(text: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut level = 0i32;
    let mut start = 0;
    for (i, ch) in text.char_indices() {
        match ch {
            '<' | '(' | '[' | '{' => level += 1,
            '>' | ')' | ']' | '}' => level -= 1,
            c if c == sep && level == 0 => {
                parts.push(&text[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}
