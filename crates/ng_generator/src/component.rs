//! Reads the arguments of `@Component` and `@Directive` directly from the tree.
//!
//! The official compiler does this through `package:analyzer`, resolving each
//! argument as a constant. Here the arguments that matter to the generator are
//! literals written in the annotation itself (`selector: 'x'`, `templateUrl:
//! 'x.html'`), and those that are not — the `directives` list, the `providers`
//! — are kept as source text, for the stages that know how to use them.
use crate::view::{Motivo, Recusa, recusa};
use dartforge_frontend::ast;
use dartforge_intern::Interner;

/// `@Component(...)` or `@Directive(...)` of a class, as written — the official
/// `CompileDirectiveMetadata`, in the part the generator uses.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Componente {
    pub classe: String,
    pub seletor: String,
    /// `@Component` (and not `@Directive`).
    pub e_componente: bool,
    /// The class extends another one or uses a mixin: inherited `@Input`,
    /// `@Output` and `@HostBinding` cannot be seen from here.
    pub herda: bool,
    /// The type parameters of the class (`class X<T extends num, U>`): the name
    /// and the bound as written. The component's view is generic over the same
    /// parameters (`ViewX0<T> extends ComponentView<X<T>>`).
    pub parametros_de_tipo: Vec<(String, Option<String>)>,
    /// `exportAs:` — the name through which `#ref="name"` reaches the
    /// directive.
    pub export_as: Option<String>,
    /// `@Output`s: name in the template -> member (a `Stream` getter), in the
    /// order the official compiler collects them.
    pub saidas: Vec<(String, String)>,
    /// Algum `@HostBinding`: quem usa a diretiva chama o `detectHostChanges`
    /// dela.
    pub liga_hospedeiro: bool,
    /// `@ContentChild`/`@ContentChildren`: the query is updated by the view of
    /// whoever projects the content.
    pub consulta_conteudo: bool,
    /// `providers:`/`viewProviders:` with providers: they go into the node of
    /// the user and change the numbering of its providers.
    pub com_provedores: bool,
    /// The names of `exports:`, in written order: they enter the expression
    /// scope (`_matchExport`) before the members.
    pub exportados: Vec<String>,
    /// `visibility:` written: the value comes from the program metadata
    /// (`metadata.rs`); without it the host is unknown.
    pub visibilidade_escrita: bool,
    /// `template: '...'` when the template is in the annotation itself.
    pub template: Option<String>,
    /// Where the contents of `template:` start in the `.dart`, in UTF-16 code
    /// units — what the official compiler adds to the positions of the `REF`
    /// comment. Only when the literal is a simple string, without escapes or
    /// interpolation (the template text is that of the source, and the
    /// positions inside it hold in both).
    pub deslocamento_do_template: Option<usize>,
    /// `templateUrl: 'x.html'`.
    pub template_url: Option<String>,
    pub style_urls: Vec<String>,
    /// `styles: ['...']` — stylesheets written in the annotation.
    pub styles: Vec<String>,
    /// `styleUrls:`/`styles:` with an item that is not a literal string (or
    /// that is not a list): what the official compiler reads cannot be seen
    /// from here, and the view refuses.
    pub estilos_ilegiveis: bool,
    /// `changeDetection: ChangeDetectionStrategy.OnPush`.
    pub on_push: bool,
    /// `directiveTypes: [Typed<X>.of([#T]), ..]`: the type arguments of the
    /// generic directives of this view (`CompileTypedMetadata`).
    pub tipos_de_diretiva: Vec<TipoDeDiretiva>,
    /// `directiveTypes:` in a shape that cannot be read from here.
    pub tipos_de_diretiva_ilegiveis: bool,
    /// `@changeDetectionLink` (`package:ngdart/experimental.dart`): the views
    /// get `detectChangesInCheckAlwaysViews` (`view_builder.dart:522`).
    pub link_de_deteccao: bool,
    /// `encapsulation: ViewEncapsulation.none`. Without a stylesheet the
    /// official compiler already turns encapsulation off
    /// (`ast_directive_normalizer.dart`), so it only matters with
    /// `styleUrls`/`styles` — a shape still refused by the view.
    pub sem_encapsulamento: bool,
    /// `preserveWhitespace: true`: the template does not go through the
    /// `MinimizeWhitespaceVisitor` (only `&ngsp;` becomes a space) and the ends
    /// of interpolations are not compressed
    /// (`ast_template_parser.dart:155-164`,
    /// `expression_converter.dart:151,161`).
    pub preservar_espacos: bool,
    /// Names written in `directives:`, in order, as written (`A`, `li.B`).
    /// Constant lists (`coreDirectives`) go in by name and are expanded by the
    /// semantic database.
    pub diretivas: Vec<String>,
    /// Some item of `directives:` is not a name (spread, `if`, expression): the
    /// list is not known.
    pub diretivas_ilegiveis: bool,
    /// Constructor parameters, in order — what the host view needs to
    /// instantiate the component.
    pub parametros: Vec<Parametro>,
    /// Lifecycle hooks the class implements.
    pub ganchos: Ganchos,
    /// Every shape of the component the generator does not translate yet —
    /// member annotation (`@HostBinding`, `@ContentChild`…) or `@Component`
    /// argument outside what it understands —, with the text the scoreboard
    /// shows. All of them, not only the first: that is how the scoreboard knows
    /// how many files a new shape unlocks. While there is one, the file is not
    /// ours: generating while ignoring it gives **wrong** output, not
    /// incomplete output.
    pub nao_entendidos: Vec<Recusa>,
    /// Names written in `pipes:`, in order, as written. As in `directives:`, a
    /// constant list (`commonPipes`) goes in by name and is expanded by the
    /// semantic database. The list itself does not change the view; using a
    /// pipe in the template does.
    pub pipes: Vec<String>,
    /// Some item of `pipes:` is not a name: the list is not known.
    pub pipes_ilegiveis: bool,
    /// `@ViewChild('ref')` on a field, in declaration order — which is the
    /// order in which the official compiler writes the assignments
    /// (`queryIndex`). Whether the query is static or not depends on where
    /// `#ref` is in the template.
    pub consultas: Vec<Consulta>,
    /// The class's `@HostListener`s, in the simple shape, in declaration order.
    pub ouvintes: Vec<Ouvinte>,
    /// `@HostBinding`s of a component, in the order the official compiler
    /// collects them (accessors, then fields, each group in declaration order):
    /// the view's `detectHostChanges`.
    pub ligacoes_do_hospedeiro: Vec<LigacaoDoHospedeiro>,
    /// `@ContentChild`/`@ContentChildren`, in the order of `directive.queries`
    /// (setters before fields, as the official visitor visits them). `None`:
    /// some outside the known shape.
    pub consultas_de_conteudo: Option<Vec<ConsultaDeConteudo>>,
    /// Each instance field and getter of the class. The emitter needs this to
    /// choose between `interpolateString`, `interpolate` and
    /// `updateTextWithPrimitive`, which the official compiler decides by the
    /// static type of the template expression and by whether it is mutable. A
    /// static member is left out: the official compiler reads it through the
    /// class (`import1.X.name`), a shape the converter does not write.
    pub membros: std::collections::HashMap<String, Membro>,
    /// `@Input`s, in the order of the official `inputs` map — the order in
    /// which the inputs of a node are written (`_SortInputsVisitor`).
    pub entradas: Vec<Entrada>,
    /// Methods of the class, kept apart from the fields: a method can only
    /// appear as a call target (`titulo()`), and its type is the return type.
    /// If they were in the same map, `{{ titulo }}` (tear-off) would be
    /// interpolated as if it were the return value.
    pub metodos: std::collections::HashMap<String, String>,
    /// Positional parameters of each instance method: that is what decides, in
    /// a tear-off (`(click)="salvar"`), between `salvar()` and `salvar($event)`
    /// (`rewriteTearOff`).
    pub aridades: std::collections::HashMap<String, usize>,
}

/// Um `@Input`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// Binding name in the template (the alias, if any).
    pub nome: String,
    /// Membro que recebe o valor.
    pub campo: String,
}

impl Componente {
    /// O membro que recebe a entrada `nome` do template.
    pub fn entrada(&self, nome: &str) -> Option<&Entrada> {
        self.entradas.iter().find(|e| e.nome == nome)
    }
}

/// ngdart lifecycle hooks implemented by the component. Each one has its fixed
/// place in the host view's `detectChangesInternal`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Ganchos {
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

impl Ganchos {
    /// Does any hook put code in the host view?
    pub fn algum(&self) -> bool {
        self.on_init
            || self.on_destroy
            || self.do_check
            || self.after_content_init
            || self.after_content_checked
            || self.after_view_init
            || self.after_view_checked
    }

    /// `firstCheck` is only declared when some hook uses it.
    pub fn usa_primeira_checagem(&self) -> bool {
        self.on_init || self.after_content_init || self.after_view_init
    }

    /// Is there change detection in the host view? (`ngOnDestroy` alone only
    /// generates `destroyInternal`.)
    pub fn tem_deteccao(&self) -> bool {
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
pub struct Membro {
    /// Type as written, or inferred from the initializer; empty when unknown
    /// (`final x = A.b;`).
    pub tipo: String,
    /// `final` (or getter): counts as immutable under ngcompiler's
    /// `isImmutable` rule, which decides whether a primitive value takes the
    /// fast path.
    pub imutavel: bool,
    /// `static` member: the template reads it unqualified, and the official
    /// compiler writes it through the class (`import1.Class.name`), with type
    /// `dynamic` (the `_TypeResolver` does not resolve statics).
    pub estatico: bool,
}

/// A component's `@HostBinding('class.x')`/`@HostBinding('attr.x')`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigacaoDoHospedeiro {
    /// `class.ativo`, `attr.role`.
    pub nome: String,
    /// O campo ou getter lido.
    pub membro: String,
    /// Campo `final`: escrito uma vez, na primeira checagem (`isImmutable`).
    pub imutavel: bool,
    /// Static member: written once, in the view constructor (case j96).
    pub estatico: bool,
}

/// Um `@ViewChild('ref') T? campo;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consulta {
    /// Campo que recebe o resultado.
    pub propriedade: String,
    /// The reference looked up in the template (`#ref`).
    pub referencia: String,
    /// Field type, as written (`html.DivElement?`). It decides the value: an
    /// `Element` receives the node itself, any other type an `ElementRef`
    /// (`isElementType` in `find_components.dart`). In `@ViewChildren`, the
    /// type argument of the list.
    pub tipo: String,
    /// `@ViewChildren`: the field receives the list of all results.
    pub lista: bool,
    /// `@ViewChild(Type)`: `referencia` is the name of the type looked up.
    pub por_tipo: bool,
    /// `read: T`, as written: the token of the provider of the found node that
    /// becomes the value (`queryWithRead.read` in `compile_element.dart`).
    pub leitura: Option<String>,
}

/// A `@ContentChild`/`@ContentChildren`, as the user of the component needs it:
/// with no result in the content, the list receives `[]` and the single one
/// receives nothing (`_createUpdates`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultaDeConteudo {
    /// Field or setter that receives the result.
    pub campo: String,
    /// `@ContentChildren`.
    pub lista: bool,
    /// The type looked up, as written (`LiTabComponent`, `li.X`), or the
    /// reference (`'name'`, with `nome` here and `referencia` set).
    pub alvo: String,
    pub referencia: bool,
    /// `descendants:` — without it, only content one directive away matches
    /// (`_getQueriesFor`).
    pub descendentes: bool,
    /// `read:` — the value read is another token of the found node: the type
    /// name, as written.
    pub leitura: Option<String>,
}

/// A component's `@HostListener`: the event and the text of the handler the
/// official compiler builds (`_addHostListener`: `method(args)`, with `$event`
/// deduced when there are no `args` and the method has one parameter). The text
/// goes through the same handler converter as the template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ouvinte {
    pub evento: String,
    pub handler: String,
}

/// A parameter of the component constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parametro {
    /// Tipo como escrito (`Element`, `RestConfig`, `List<int>`).
    pub tipo: Option<String>,
    pub nome: String,
    pub nomeado: bool,
    /// `@Optional`, `@Inject(...)`, `@Self`… change the injection.
    pub anotado: bool,
    /// `@Optional()`: `injectorGetOptional`, or `null` where there is no
    /// provider.
    pub opcional: bool,
    /// `@Self()`: only the node itself.
    pub proprio: bool,
    /// `@Host()`: does not leave the component's view (`_getDependency`).
    pub hospedeiro: bool,
    /// `@SkipSelf()`: starts at the node above.
    pub pular: bool,
    /// Another annotation (`@Inject`, `@Attribute`, unknown): no translation
    /// yet.
    pub outra_anotacao: bool,
}

/// Value of a named argument, when it is a string literal without
/// interpolation.
///
/// In a multi-line string, each source line ending (`\r\n`, `\r`) is `\n` in
/// the value (Dart specification, *Strings*); it is the value the analyzer
/// hands to ngcompiler (`li_highlight_component.dart`, with CRLF).
fn texto_do_argumento(arvore: &ast::Ast, id: ast::ExprId) -> Option<String> {
    match &arvore.expr(id).kind {
        ast::ExprKind::String(lit) => lit
            .constant_value()
            // Only the CRLF pair: the value is already decoded, and a lone `\r`
            // may be the source's `\r` escape, which stays.
            .map(|s| s.to_string_lossy().replace("\r\n", "\n")),
        ast::ExprKind::Parenthesized(inner) => texto_do_argumento(arvore, *inner),
        _ => None,
    }
}

/// Where the contents of a `template:` start in the source (in UTF-16 code
/// units, as the analyzer counts): the `REF` positions are those of the string
/// value plus this. A single string (simple, raw or triple-quoted, with escapes
/// or not) counts from the contents (`contentsOffset`); adjacent strings count
/// from the start of the node (`offset`) — cases j53 and j63.
fn deslocamento_do_template(arvore: &ast::Ast, fonte: &str, id: ast::ExprId) -> Option<usize> {
    let span = arvore.expr(id).span;
    let bruto = fonte.get(span.start..span.end)?;
    let (inicio, fim) = conteudo_do_literal(bruto)?;
    let deslocamento = if fim == bruto.len() {
        span.start + inicio
    } else {
        span.start
    };
    Some(fonte.get(..deslocamento)?.encode_utf16().count())
}

/// The first string literal of `bruto`: where the contents start (after the `r`
/// and the quotes; with triple quotes, after the first line if it only has
/// whitespace — Dart's rule for multi-line strings, case j53) and where the
/// literal ends.
fn conteudo_do_literal(bruto: &str) -> Option<(usize, usize)> {
    let cru = bruto.starts_with(['r', 'R']);
    let depois_do_r = usize::from(cru);
    let resto = &bruto[depois_do_r..];
    let aspa = ["'''", "\"\"\"", "'", "\""]
        .into_iter()
        .find(|q| resto.starts_with(q))?;
    let mut inicio = depois_do_r + aspa.len();
    let corpo = &bruto[inicio..];
    // The end: the next equal quote that is not escaped (in a raw string, the
    // backslash escapes nothing).
    let mut i = 0;
    let bytes = corpo.as_bytes();
    let fim = loop {
        if i >= corpo.len() {
            return None;
        }
        if !cru && bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i..].starts_with(aspa.as_bytes()) {
            break inicio + i + aspa.len();
        }
        i += 1;
    };
    if aspa.len() == 3 {
        let sem_brancos = corpo.trim_start_matches([' ', '\t']);
        if let Some(depois) = sem_brancos
            .strip_prefix("\r\n")
            .or_else(|| sem_brancos.strip_prefix('\n'))
        {
            inicio += corpo.len() - depois.len();
        }
    }
    Some((inicio, fim))
}

/// Lista de strings literais (`styleUrls: ['a.css', 'b.css']`).
fn lista_de_textos(arvore: &ast::Ast, id: ast::ExprId) -> Vec<String> {
    let ast::ExprKind::List { elements, .. } = &arvore.expr(id).kind else {
        return Vec::new();
    };
    elements
        .iter()
        .filter_map(|e| match e {
            ast::CollectionElement::Expression(x) => texto_do_argumento(arvore, *x),
            _ => None,
        })
        .collect()
}

/// Names written in a list literal (`directives: [A, li.B]`), and whether some
/// item is not a name.
fn nomes_da_lista(arvore: &ast::Ast, interner: &Interner, id: ast::ExprId) -> (Vec<String>, bool) {
    let ast::ExprKind::List { elements, .. } = &arvore.expr(id).kind else {
        return (Vec::new(), true);
    };
    let mut nomes = Vec::new();
    let mut ilegivel = false;
    for e in elements.iter() {
        match e {
            // `...list`: the official constant value is the flattened list; its
            // name is expanded in place (`Designado::Lista`).
            ast::CollectionElement::Expression(x)
            | ast::CollectionElement::Spread { value: x, .. } => {
                match crate::resolucao::nome_qualificado(arvore, interner, *x) {
                    Some(n) => nomes.push(n),
                    None => ilegivel = true,
                }
            }
            _ => ilegivel = true,
        }
    }
    (nomes, ilegivel)
}

/// `ChangeDetectionStrategy.onPush` (the name in ngdart 8) — what changes the
/// initial state of the view.
fn e_on_push(arvore: &ast::Ast, fonte: &str, id: ast::ExprId) -> bool {
    let span = arvore.expr(id).span;
    fonte
        .get(span.start..span.end)
        .is_some_and(|t| t.rsplit('.').next().map(str::trim) == Some("onPush"))
}

/// The value of `encapsulation:` when written as `ViewEncapsulation.x` (with or
/// without an import prefix): `emulated` or `none`, both from ngdart 8. Any
/// other shape (constant, expression) gives `None`.
fn valor_de_encapsulamento<'a>(
    arvore: &ast::Ast,
    fonte: &'a str,
    id: ast::ExprId,
) -> Option<&'a str> {
    let span = arvore.expr(id).span;
    let texto = fonte.get(span.start..span.end)?.trim();
    let (antes, valor) = texto.rsplit_once('.')?;
    let tipo = antes.rsplit('.').next()?.trim();
    (tipo == "ViewEncapsulation" && matches!(valor.trim(), "emulated" | "none"))
        .then(|| valor.trim())
}

/// Extracts the `@Component` of an annotated class.
pub fn ler_componente(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
    anotacao: &ast::Annotation,
) -> Componente {
    ler(arvore, fonte, interner, classe, anotacao, true)
}

/// Extracts the `@Directive` of an annotated class: what its users need to know
/// about it (selector, inputs, outputs, constructor, lifecycle).
pub fn ler_diretiva(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
    anotacao: &ast::Annotation,
) -> Componente {
    ler(arvore, fonte, interner, classe, anotacao, false)
}

/// A `@Pipe`, as the official `PipeVisitor` reads it: the name, whether it is
/// pure and the type of `transform` (`fromFunctionType`) — it is where the type
/// of each call's `pureProxyN` comes from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pipe {
    pub classe: String,
    /// `@Pipe('nome')`.
    pub nome: String,
    /// `pure:`, `true` quando omitido (`coerceBool(.., defaultTo: true)`).
    pub puro: bool,
    /// Retorno do `transform`, como escrito (`String?`).
    pub retorno: String,
    /// Type of each positional parameter of `transform`, in order; without a
    /// written type, `None` (the inferred type of an override cannot be seen
    /// from here).
    pub parametros: Vec<Option<String>>,
    /// Why this pipe is not instantiated by the generator yet: constructor
    /// injection (`createPipeInstance` asks the injector), `OnDestroy`
    /// (`bindPipeDestroyLifecycleCallbacks`), inherited `transform`, no return
    /// type or a named parameter.
    pub fora: Option<&'static str>,
    /// The constructor receives only the `ChangeDetectorRef`: the view that
    /// creates it passes itself (`o.thisExpr` of `createPipeInstance`).
    pub detector: bool,
    /// `OnDestroy`: the view that creates it calls `ngOnDestroy()` when
    /// destroyed.
    pub destroi: bool,
}

/// Extracts the `@Pipe` of an annotated class.
pub fn ler_pipe(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
    anotacao: &ast::Annotation,
) -> Pipe {
    let mut p = Pipe {
        classe: interner.resolve(classe.name.sym).to_string(),
        puro: true,
        ..Default::default()
    };
    let mut nome_lido = false;
    if let Some(args) = &anotacao.arguments {
        for a in args.args.iter() {
            match a.name.as_ref().map(|n| interner.resolve(n.sym)) {
                None if !nome_lido => {
                    nome_lido = true;
                    match texto_do_argumento(arvore, a.value) {
                        Some(n) => p.nome = n,
                        None => p.fora = Some("@Pipe name that is not a literal"),
                    }
                }
                Some("name") if !nome_lido => {
                    nome_lido = true;
                    match texto_do_argumento(arvore, a.value) {
                        Some(n) => p.nome = n,
                        None => p.fora = Some("@Pipe name that is not a literal"),
                    }
                }
                Some("pure") => match &arvore.expr(a.value).kind {
                    ast::ExprKind::Bool(b) => p.puro = *b,
                    _ => p.fora = Some("pure: that is not a literal"),
                },
                _ => p.fora = Some("@Pipe argument outside the known ones"),
            }
        }
    }
    let parametros = parametros_do_construtor(arvore, fonte, interner, classe);
    match parametros.as_slice() {
        [] => {}
        [x] if x.tipo.as_deref().map(|t| t.rsplit('.').next().unwrap_or(t))
            == Some("ChangeDetectorRef")
            && !x.anotado
            && !x.nomeado =>
        {
            p.detector = true;
        }
        _ => p.fora = Some("pipe with constructor injection"),
    }
    p.destroi = ganchos_da_classe(arvore, fonte, classe).on_destroy;
    let mut achou = false;
    for &id in &classe.members {
        let ast::MemberKind::Method(f) = &arvore.member(id).kind else {
            continue;
        };
        let funcao = arvore.function(*f);
        if !matches!(funcao.kind, ast::FunctionKind::Function) || funcao.static_ {
            continue;
        }
        if funcao.name.map(|n| interner.resolve(n.sym)) != Some("transform") {
            continue;
        }
        achou = true;
        match funcao.return_type {
            Some(t) => p.retorno = texto_do_tipo(arvore, fonte, t),
            None => p.fora = Some("transform without a return type"),
        }
        for par in funcao.parameters.as_deref().unwrap_or(&[]) {
            if matches!(par.kind, ast::ParameterKind::Named) {
                p.fora = Some("transform with a named parameter");
                continue;
            }
            p.parametros
                .push(par.ty.map(|t| texto_do_tipo(arvore, fonte, t)));
        }
    }
    if !achou {
        p.fora = Some("inherited or missing transform");
    }
    p
}

fn ler(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
    anotacao: &ast::Annotation,
    e_componente: bool,
) -> Componente {
    let mut c = Componente {
        classe: interner.resolve(classe.name.sym).to_string(),
        e_componente,
        herda: classe.extends.is_some() || !classe.with.is_empty(),
        parametros_de_tipo: classe
            .type_params
            .iter()
            .map(|t| {
                (
                    interner.resolve(t.name.sym).to_string(),
                    t.bound.map(|b| texto_do_tipo(arvore, fonte, b)),
                )
            })
            .collect(),
        ..Default::default()
    };
    let mut exportados = Vec::new();
    if let Some(args) = &anotacao.arguments {
        for a in args.args.iter() {
            let Some(nome) = a.name.as_ref().map(|n| interner.resolve(n.sym)) else {
                continue;
            };
            match nome {
                "selector" => c.seletor = texto_do_argumento(arvore, a.value).unwrap_or_default(),
                "exportAs" => c.export_as = texto_do_argumento(arvore, a.value),
                "template" => {
                    c.template = texto_do_argumento(arvore, a.value);
                    c.deslocamento_do_template = c
                        .template
                        .as_deref()
                        .and_then(|_| deslocamento_do_template(arvore, fonte, a.value));
                }
                "templateUrl" => c.template_url = texto_do_argumento(arvore, a.value),
                "styleUrls" | "styles" => {
                    let textos = lista_de_textos(arvore, a.value);
                    let itens = match &arvore.expr(a.value).kind {
                        ast::ExprKind::List { elements, .. } => Some(elements.len()),
                        _ => None,
                    };
                    c.estilos_ilegiveis |= itens != Some(textos.len());
                    if nome == "styles" {
                        c.styles = textos;
                    } else {
                        c.style_urls = textos;
                    }
                }
                "changeDetection" => c.on_push = e_on_push(arvore, fonte, a.value),
                "encapsulation" => {
                    c.sem_encapsulamento =
                        valor_de_encapsulamento(arvore, fonte, a.value) == Some("none")
                }
                "preserveWhitespace" => {
                    c.preservar_espacos =
                        matches!(arvore.expr(a.value).kind, ast::ExprKind::Bool(true))
                }
                "directives" => {
                    (c.diretivas, c.diretivas_ilegiveis) = nomes_da_lista(arvore, interner, a.value)
                }
                "directiveTypes" => {
                    let sp = arvore.expr(a.value).span;
                    match fonte.get(sp.start..sp.end).and_then(ler_tipos_de_diretiva) {
                        Some(v) => c.tipos_de_diretiva = v,
                        None => c.tipos_de_diretiva_ilegiveis = true,
                    }
                }
                "pipes" => (c.pipes, c.pipes_ilegiveis) = nomes_da_lista(arvore, interner, a.value),
                "exports" => exportados = nomes_da_lista(arvore, interner, a.value).0,
                "providers" | "viewProviders" => c.com_provedores |= !lista_vazia(arvore, a.value),
                "visibility" => c.visibilidade_escrita = true,
                _ => {}
            }
        }
    }
    c.parametros = parametros_do_construtor(arvore, fonte, interner, classe);
    c.membros = tipos_dos_membros(arvore, fonte, interner, classe);
    c.metodos = tipos_dos_metodos(arvore, fonte, interner, classe);
    c.aridades = aridades_dos_metodos(arvore, interner, classe);
    // `exports:` puts static names in the expression scope, and they win over
    // the member with the same name (`_matchExport` before the implicit
    // receiver). A member shadowed like this leaves the map, and using it is
    // refused.
    for n in &exportados {
        let simples = n.split('.').next().unwrap_or(n);
        c.membros.remove(simples);
        c.metodos.remove(simples);
        c.aridades.remove(simples);
    }
    c.exportados = exportados;
    c.entradas = entradas_da_classe(arvore, interner, classe);
    c.saidas = saidas_da_classe(arvore, interner, classe);
    c.ganchos = ganchos_da_classe(arvore, fonte, classe);
    c.nao_entendidos =
        o_que_nao_entendemos(arvore, fonte, interner, classe, anotacao, e_componente);
    c.liga_hospedeiro = tem_anotacao(arvore, interner, classe, &["HostBinding"]);
    c.consulta_conteudo = tem_anotacao(
        arvore,
        interner,
        classe,
        &["ContentChild", "ContentChildren"],
    );
    c.consultas = consultas_da_classe(arvore, fonte, interner, classe, &mut c.nao_entendidos);
    c.ouvintes = ouvintes_da_classe(arvore, interner, classe, &mut c.nao_entendidos);
    // The inherited ones go into the official list and cannot be seen from
    // here: the view completes the list from the metadata read from the
    // program, or refuses.
    if e_componente && c.liga_hospedeiro {
        c.ligacoes_do_hospedeiro =
            ligacoes_do_hospedeiro(arvore, interner, classe, &mut c.nao_entendidos);
    }
    c.consultas_de_conteudo = consultas_de_conteudo(arvore, interner, classe);
    if e_componente && c.consulta_conteudo && c.consultas_de_conteudo.is_none() {
        c.nao_entendidos
            .push(recusa(Motivo::ContentChild, "@ContentChild outside the shape"));
    }
    c
}

/// The class's `@ContentChild`/`@ContentChildren`: setters, then fields, each
/// group in declaration order. `None` if any is outside the shape
/// `@ContentChild(Type)`/`@ContentChild('ref')` on a field or setter.
fn consultas_de_conteudo(
    arvore: &ast::Ast,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> Option<Vec<ConsultaDeConteudo>> {
    let mut setters = Vec::new();
    let mut campos = Vec::new();
    for &id in &classe.members {
        let membro = arvore.member(id);
        for a in membro.metadata.iter() {
            let lista = match crate::nome_da_anotacao(a, interner).as_str() {
                "ContentChild" => false,
                "ContentChildren" => true,
                _ => continue,
            };
            let primeiro = a
                .arguments
                .as_ref()
                .and_then(|args| args.args.iter().find(|x| x.name.is_none()))?;
            let (alvo, referencia) = match texto_do_argumento(arvore, primeiro.value) {
                Some(t) => (t, true),
                None => (
                    crate::resolucao::nome_qualificado(arvore, interner, primeiro.value)?,
                    false,
                ),
            };
            // `ContentChildren(descendants: true)` by default in ngdart 8;
            // `ContentChild` always. `read:` changes the value read.
            let mut descendentes = true;
            let mut leitura = None;
            for x in a.arguments.as_ref().map(|g| &g.args[..]).unwrap_or(&[]) {
                match x.name.map(|n| interner.resolve(n.sym)) {
                    None => {}
                    Some("descendants") if lista => match &arvore.expr(x.value).kind {
                        ast::ExprKind::Bool(b) => descendentes = *b,
                        _ => return None,
                    },
                    Some("read") => {
                        leitura = Some(crate::resolucao::nome_qualificado(
                            arvore, interner, x.value,
                        )?)
                    }
                    _ => return None,
                }
            }
            match &membro.kind {
                ast::MemberKind::Field(l) if !l.static_ && l.variables.len() == 1 => {
                    campos.push(ConsultaDeConteudo {
                        campo: interner.resolve(l.variables[0].name.sym).to_string(),
                        lista,
                        alvo,
                        referencia,
                        descendentes,
                        leitura: leitura.clone(),
                    });
                }
                ast::MemberKind::Method(f) => {
                    let funcao = arvore.function(*f);
                    match (funcao.kind, funcao.name, funcao.static_) {
                        (ast::FunctionKind::Setter, Some(n), false) => {
                            setters.push(ConsultaDeConteudo {
                                campo: interner.resolve(n.sym).to_string(),
                                lista,
                                alvo,
                                referencia,
                                descendentes,
                                leitura: leitura.clone(),
                            })
                        }
                        _ => return None,
                    }
                }
                _ => return None,
            }
        }
    }
    setters.extend(campos);
    Some(setters)
}

/// Does any member of the class have one of these annotations?
fn tem_anotacao(
    arvore: &ast::Ast,
    interner: &Interner,
    classe: &ast::ClassDecl,
    nomes: &[&str],
) -> bool {
    classe.members.iter().any(|&m| {
        arvore
            .member(m)
            .metadata
            .iter()
            .any(|a| nomes.contains(&crate::nome_da_anotacao(a, interner).as_str()))
    })
}

/// Arguments of `@Component` whose effect the generator knows. `pipes:` is here
/// because the list does not change the view; using a pipe in the template
/// does, and that is checked against the template. `exports:` only puts names
/// in the expression scope (see [`ler`]).
const ARGUMENTOS_CONHECIDOS: &[&str] = &[
    "selector",
    // Only decides which class a user's `#ref="name"` points to
    // (`identifierForReference`); read in [`Componente::export_as`].
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

/// Argumentos de `@Directive` cujo efeito o gerador conhece.
const ARGUMENTOS_DE_DIRETIVA: &[&str] = &["selector", "exportAs"];

/// Lifecycle interfaces implemented, through the `implements` and `with`
/// clauses. (Those of ngrouter — `OnActivate`, `CanDeactivate` — do not touch
/// the view; nor does `AfterChanges` appear in the component's own file.)
fn ganchos_da_classe(arvore: &ast::Ast, fonte: &str, classe: &ast::ClassDecl) -> Ganchos {
    let mut g = Ganchos::default();
    for t in classe.implements.iter().chain(classe.with.iter()) {
        let s = arvore.ty(*t).span;
        let texto = fonte.get(s.start..s.end).unwrap_or("");
        match texto.split(['<', '.']).next_back().unwrap_or(texto).trim() {
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
/// ([`consultas_da_classe`], [`ouvintes_da_classe`]), which refuses what it
/// does not know.
fn o_que_nao_entendemos(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
    anotacao: &ast::Annotation,
    e_componente: bool,
) -> Vec<Recusa> {
    let mut fora = Vec::new();
    let anot = if e_componente {
        "@Component"
    } else {
        "@Directive"
    };
    if let Some(args) = &anotacao.arguments {
        for a in args.args.iter() {
            let Some(nome) = a.name.as_ref().map(|n| interner.resolve(n.sym)) else {
                fora.push(recusa(
                    Motivo::NaoEntendido,
                    format!("argumento posicional em {anot}"),
                ));
                continue;
            };
            match nome {
                // The empty list does not change the view (corpus case b18) —
                // and it is 57 of the 70 `providers:` of new_sali.
                "providers" if lista_vazia(arvore, a.value) => {}
                // Those of the component go to the host view, which resolves
                // them through the metadata read from the program (`view.rs`).
                "providers" if e_componente => {}
                "providers" => fora.push(recusa(
                    Motivo::Providers,
                    format!("{anot}(.., providers: [..])"),
                )),
                // `ViewEncapsulation.emulated`/`.none` written: the effect is
                // known (see [`Componente::sem_encapsulamento`]).
                "encapsulation"
                    if e_componente
                        && valor_de_encapsulamento(arvore, fonte, a.value).is_some() => {}
                "encapsulation" => fora.push(recusa(
                    Motivo::Encapsulamento,
                    "@Component(.., encapsulation: ..)",
                )),
                // Literal `true`/`false`: the whitespace mode of the template.
                "preserveWhitespace"
                    if e_componente
                        && matches!(arvore.expr(a.value).kind, ast::ExprKind::Bool(_)) => {}
                n if e_componente && ARGUMENTOS_CONHECIDOS.contains(&n) => {}
                n if !e_componente && ARGUMENTOS_DE_DIRETIVA.contains(&n) => {}
                n => fora.push(recusa(Motivo::NaoEntendido, format!("{anot}(.., {n}: ..)"))),
            }
        }
    }
    for &id in &classe.members {
        let membro = arvore.member(id);
        for a in membro.metadata.iter() {
            let nome = crate::nome_da_anotacao(a, interner);
            let motivo = match nome.as_str() {
                "HostBinding" if !e_componente => Motivo::HostBindingEmComponente,
                // No componente, a hospedeira atribui o resultado vazio (ver
                // [`consultas_de_conteudo`]).
                "ContentChild" | "ContentChildren" if !e_componente => Motivo::ContentChild,
                _ => continue,
            };
            fora.push(recusa(motivo, format!("@{nome}")));
        }
    }
    fora
}

/// Literal `[]`, without any element (a comment inside does not count).
fn lista_vazia(arvore: &ast::Ast, id: ast::ExprId) -> bool {
    matches!(&arvore.expr(id).kind, ast::ExprKind::List { elements, .. } if elements.is_empty())
}

/// The class's `@ViewChild`s. Only the shape `@ViewChild('ref') T? field;` — a
/// string selector, without `read:`, on a field with a written type — becomes a
/// query. The rest is refused right here, with the reason.
fn consultas_da_classe(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
    fora: &mut Vec<Recusa>,
) -> Vec<Consulta> {
    // Like the official `_queries`: the accessors (setters), then the fields,
    // each group in declaration order.
    let mut setters = Vec::new();
    let mut campos = Vec::new();
    for &id in &classe.members {
        let membro = arvore.member(id);
        for a in membro.metadata.iter() {
            let lista = match crate::nome_da_anotacao(a, interner).as_str() {
                "ViewChild" => false,
                "ViewChildren" => true,
                _ => continue,
            };
            match consulta_simples(arvore, fonte, interner, membro, a, lista) {
                Ok(c) if matches!(membro.kind, ast::MemberKind::Method(_)) => setters.push(c),
                Ok(c) => campos.push(c),
                Err(r) => fora.push(r),
            }
        }
    }
    setters.extend(campos);
    setters
}

/// The `@HostBinding`s declared in the component's own class, in the order of
/// `DirectiveVisitor`: accessors, then fields. The name is the literal text of
/// the argument or, without an argument, that of the member
/// (`hostProperties[bindingName ?? memberName]`); getter or field, instance or
/// static. The shape of each name (`class.x`, `attr.x`, `style.x`, property) is
/// checked by the view, which also adds the inherited ones.
fn ligacoes_do_hospedeiro(
    arvore: &ast::Ast,
    interner: &Interner,
    classe: &ast::ClassDecl,
    fora: &mut Vec<Recusa>,
) -> Vec<LigacaoDoHospedeiro> {
    let mut acessores = Vec::new();
    let mut campos = Vec::new();
    let fora_da_forma = || {
        recusa(
            Motivo::HostBindingEmComponente,
            "@HostBinding outside a field or getter, or with an unreadable name",
        )
    };
    for &id in &classe.members {
        let membro = arvore.member(id);
        for a in membro.metadata.iter() {
            if crate::nome_da_anotacao(a, interner) != "HostBinding" {
                continue;
            }
            // `Some(None)`: without an argument — the name is that of the
            // member.
            let nome = match a.arguments.as_ref().map(|args| &args.args[..]) {
                None | Some([]) => Some(None),
                Some([x]) if x.name.is_none() => texto_do_argumento(arvore, x.value).map(Some),
                _ => None,
            };
            let Some(nome) = nome else {
                fora.push(fora_da_forma());
                continue;
            };
            match &membro.kind {
                ast::MemberKind::Field(l) if l.variables.len() == 1 && (l.static_ || !l.const_) => {
                    let m = interner.resolve(l.variables[0].name.sym).to_string();
                    campos.push(LigacaoDoHospedeiro {
                        nome: nome.unwrap_or_else(|| m.clone()),
                        membro: m,
                        imutavel: l.final_ || l.const_,
                        estatico: l.static_,
                    });
                }
                ast::MemberKind::Method(f) => {
                    let funcao = arvore.function(*f);
                    match (funcao.kind, funcao.name) {
                        (ast::FunctionKind::Getter, Some(n)) => {
                            let m = interner.resolve(n.sym).to_string();
                            acessores.push(LigacaoDoHospedeiro {
                                nome: nome.unwrap_or_else(|| m.clone()),
                                membro: m,
                                imutavel: false,
                                estatico: funcao.static_,
                            });
                        }
                        _ => fora.push(fora_da_forma()),
                    }
                }
                _ => fora.push(fora_da_forma()),
            }
        }
    }
    acessores.extend(campos);
    acessores
}

/// The class's `@HostListener`s, in the declaration order of the methods
/// (`DirectiveVisitor` visits the methods in order). The handler is the text of
/// `_addHostListener` (`find_components.dart`): without `args`, a method with
/// one parameter receives `$event`.
fn ouvintes_da_classe(
    arvore: &ast::Ast,
    interner: &Interner,
    classe: &ast::ClassDecl,
    fora: &mut Vec<Recusa>,
) -> Vec<Ouvinte> {
    let mut saida: Vec<Ouvinte> = Vec::new();
    for &id in &classe.members {
        let membro = arvore.member(id);
        for a in membro.metadata.iter() {
            if crate::nome_da_anotacao(a, interner) != "HostListener" {
                continue;
            }
            match ouvinte(arvore, interner, membro, a) {
                Some(o) if !saida.iter().any(|x| x.evento == o.evento) => saida.push(o),
                // Two listeners of the same event: the official map keeps the
                // last one, in place of the first. Not yet.
                Some(_) => fora.push(recusa(
                    Motivo::HostListenerEmComponente,
                    "two @HostListeners of the same event",
                )),
                None => fora.push(recusa(
                    Motivo::HostListenerEmComponente,
                    "@HostListener of a non-native event or outside the shape `m(args)`",
                )),
            }
        }
    }
    saida
}

/// Reads a `@HostListener`, or `None` if it is outside the known shape.
fn ouvinte(
    arvore: &ast::Ast,
    interner: &Interner,
    membro: &ast::Member,
    anotacao: &ast::Annotation,
) -> Option<Ouvinte> {
    let ast::MemberKind::Method(f) = &membro.kind else {
        return None;
    };
    let funcao = arvore.function(*f);
    if funcao.static_ || !matches!(funcao.kind, ast::FunctionKind::Function) {
        return None;
    }
    let metodo = interner.resolve(funcao.name?.sym).to_string();
    // `element.parameters.length`: todos, opcionais e nomeados inclusive.
    let parametros = funcao.parameters.as_ref().map_or(0, |p| p.len());
    let args = anotacao.arguments.as_ref()?;
    let (evento, lista) = match &args.args[..] {
        [e] if e.name.is_none() => (e, None),
        [e, l] if e.name.is_none() && l.name.is_none() => (e, Some(l.value)),
        _ => return None,
    };
    let evento = texto_do_argumento(arvore, evento.value)?;
    // An event that is not a DOM one (`document:click`, `keyup.enter`) goes
    // through `eventManager` and, with `:`, is not even accepted by the
    // official compiler; not yet.
    if !crate::view::evento_nativo(&evento) {
        return None;
    }
    let mut argumentos = match lista {
        None => Vec::new(),
        Some(l) => {
            let ast::ExprKind::List { elements, .. } = &arvore.expr(l).kind else {
                return None;
            };
            let textos = lista_de_textos(arvore, l);
            if textos.len() != elements.len() {
                return None;
            }
            textos
        }
    };
    if argumentos.is_empty() && parametros == 1 {
        argumentos.push("$event".into());
    }
    Some(Ouvinte {
        evento,
        handler: format!("{metodo}({})", argumentos.join(", ")),
    })
}

/// Reads a `@ViewChild` in the shape the generator knows, or says why not.
fn consulta_simples(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    membro: &ast::Member,
    anotacao: &ast::Annotation,
    eh_lista: bool,
) -> Result<Consulta, Recusa> {
    let args = anotacao
        .arguments
        .as_ref()
        .ok_or_else(|| recusa(Motivo::NaoEntendido, "@ViewChild sem argumento"))?;
    // `read: T` replaces the value with a provider of the node: kept as the
    // written name, which the view resolves (`ElementRef`,
    // `Element`/`HtmlElement`). `first:`, `descendants:` and a `read:` that is
    // not a simple name, not yet.
    let mut leitura = None;
    let mut posicionais = Vec::new();
    for x in args.args.iter() {
        match x.name.map(|n| interner.resolve(n.sym)) {
            None => posicionais.push(x),
            Some("read") => match &arvore.expr(x.value).kind {
                ast::ExprKind::Identifier(n) if leitura.is_none() => {
                    leitura = Some(interner.resolve(n.sym).to_string());
                }
                _ => return Err(recusa(Motivo::ViewChildEmFilho, "@ViewChild(.., read: T)")),
            },
            Some(_) => return Err(recusa(Motivo::NaoEntendido, "@ViewChild with options")),
        }
    }
    let [unico] = posicionais[..] else {
        return Err(recusa(
            Motivo::NaoEntendido,
            "@ViewChild with only a named argument",
        ));
    };
    // A type selector (`@ViewChild(OtherComp)`) queries a component or
    // directive, not an element: only the simple name, resolved later.
    let (referencia, por_tipo) = match texto_do_argumento(arvore, unico.value) {
        Some(r) => (r, false),
        None => match &arvore.expr(unico.value).kind {
            ast::ExprKind::Identifier(n) => (interner.resolve(n.sym).to_string(), true),
            _ => return Err(recusa(Motivo::ViewChildEmFilho, "@ViewChild(prefixo.Tipo)")),
        },
    };
    // `'a,b'` are two selectors in a single query.
    if referencia.contains(',') || referencia.trim() != referencia || referencia.is_empty() {
        return Err(recusa(Motivo::NaoEntendido, "@ViewChild('a,b')"));
    }
    // An instance field with a written type, or an instance setter with the
    // type written on the parameter (the `_ctx.x = v` is the same, case j81).
    let (propriedade, t) = match &membro.kind {
        ast::MemberKind::Field(lista) => {
            if lista.static_
                || lista.final_
                || lista.const_
                || lista.late
                || lista.variables.len() != 1
            {
                return Err(recusa(
                    Motivo::NaoEntendido,
                    "@ViewChild on a final/late/static field",
                ));
            }
            let Some(t) = lista.ty else {
                return Err(recusa(Motivo::NaoEntendido, "@ViewChild on an untyped field"));
            };
            (interner.resolve(lista.variables[0].name.sym).to_string(), t)
        }
        ast::MemberKind::Method(f) => {
            let funcao = arvore.function(*f);
            let (ast::FunctionKind::Setter, Some(nome), false) =
                (funcao.kind, funcao.name, funcao.static_)
            else {
                return Err(recusa(
                    Motivo::NaoEntendido,
                    "@ViewChild on a method that is not a setter",
                ));
            };
            let Some(t) = funcao
                .parameters
                .as_deref()
                .and_then(|ps| ps.first())
                .and_then(|p| p.ty)
            else {
                return Err(recusa(
                    Motivo::NaoEntendido,
                    "@ViewChild on an untyped setter",
                ));
            };
            (interner.resolve(nome.sym).to_string(), t)
        }
        _ => {
            return Err(recusa(
                Motivo::NaoEntendido,
                "@ViewChild outside a field or setter",
            ));
        }
    };
    let tipo = texto_do_tipo(arvore, fonte, t);
    // `@ViewChildren`: `List<T>` (or `List<T>?`); `isElementType` looks at `T`.
    // Another collection, not yet.
    let tipo = if eh_lista {
        let t = tipo.trim().trim_end_matches('?');
        match t.strip_prefix("List<").and_then(|x| x.strip_suffix('>')) {
            Some(dentro) if !dentro.contains(['<', ',']) => dentro.trim().to_string(),
            _ => {
                return Err(recusa(
                    Motivo::ViewChildDinamico,
                    "@ViewChildren on a field that is not List<T>",
                ));
            }
        }
    } else {
        tipo
    };
    Ok(Consulta {
        propriedade,
        referencia,
        tipo,
        lista: eh_lista,
        por_tipo,
        leitura,
    })
}

/// Parameters of the generative constructor (the unnamed one). Without a
/// declared constructor, the class has the implicit constructor without
/// parameters.
fn parametros_do_construtor(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> Vec<Parametro> {
    for &id in &classe.members {
        let membro = arvore.member(id);
        let ast::MemberKind::Constructor(ctor) = &membro.kind else {
            continue;
        };
        if ctor.name.is_some() {
            continue; // a named constructor is not the one the view uses
        }
        let campos = tipos_dos_campos(arvore, fonte, interner, classe);
        // A named parameter does not take part in injection
        // (`_getCompileDiDependencyMetadata`, `compile_metadata.dart:298-302`).
        return ctor
            .parameters
            .iter()
            .filter(|p| !matches!(p.kind, ast::ParameterKind::Named))
            .map(|p| {
                let nome = p
                    .name
                    .map(|n| interner.resolve(n.sym).to_string())
                    .unwrap_or_default();
                // `C(this.x)` writes no type at all: the parameter type is that
                // of the field. That is how almost every ngdart component
                // receives its dependencies, and without this the injection
                // does not come out.
                let tipo = match p.ty {
                    Some(t) => Some(texto_do_tipo(arvore, fonte, t)),
                    None if p.this_ || p.super_ => campos.get(&nome).cloned(),
                    None => None,
                };
                let nomes: Vec<String> = p
                    .metadata
                    .iter()
                    .map(|a| crate::nome_da_anotacao(a, interner))
                    .collect();
                let tem = |n: &str| nomes.iter().any(|x| x == n);
                Parametro {
                    tipo,
                    nome,
                    nomeado: matches!(p.kind, ast::ParameterKind::Named),
                    anotado: !p.metadata.is_empty(),
                    opcional: tem("Optional"),
                    proprio: tem("Self"),
                    hospedeiro: tem("Host"),
                    pular: tem("SkipSelf"),
                    outra_anotacao: nomes
                        .iter()
                        .any(|n| !matches!(n.as_str(), "Optional" | "Self" | "Host" | "SkipSelf")),
                }
            })
            .collect();
    }
    Vec::new()
}

/// Source text of a type, as written.
fn texto_do_tipo(arvore: &ast::Ast, fonte: &str, t: ast::TypeId) -> String {
    let s = arvore.ty(t).span;
    fonte.get(s.start..s.end).unwrap_or("").to_string()
}

/// Type of each instance field and getter of the class, so the emitter knows
/// the static type of `{{ name }}`.
///
/// The immutability is that of the official `isImmutable`: the value comes from
/// `lookUpGetter(n).variable`, and only a real field (`!isSynthetic`) that is
/// `final`/`const` is immutable. In a hand-written getter the `variable` is
/// synthetic: **mutable**.
pub(crate) fn tipos_dos_membros(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> std::collections::HashMap<String, Membro> {
    let mut saida = std::collections::HashMap::new();
    for &id in &classe.members {
        match &arvore.member(id).kind {
            ast::MemberKind::Field(lista) => {
                let imutavel = lista.final_ || lista.const_;
                for v in lista.variables.iter() {
                    // Without a written type (`var x = Foo()`), the type is
                    // what the analyzer infers from the initializer — only in
                    // the shapes where it is certain.
                    let tipo = match lista.ty {
                        Some(t) => texto_do_tipo(arvore, fonte, t),
                        // Outside those shapes the member exists, but without a
                        // type (empty text): whoever needs the type refuses.
                        None => v
                            .initializer
                            .and_then(|e| tipo_inferido(arvore, fonte, interner, e))
                            .unwrap_or_default(),
                    };
                    saida.insert(
                        interner.resolve(v.name.sym).to_string(),
                        Membro {
                            tipo,
                            imutavel,
                            estatico: lista.static_,
                        },
                    );
                }
            }
            ast::MemberKind::Method(f) => {
                let funcao = arvore.function(*f);
                if !matches!(funcao.kind, ast::FunctionKind::Getter) {
                    continue;
                }
                let (Some(nome), Some(t)) = (funcao.name, funcao.return_type) else {
                    continue;
                };
                saida.insert(
                    interner.resolve(nome.sym).to_string(),
                    Membro {
                        tipo: texto_do_tipo(arvore, fonte, t),
                        imutavel: false,
                        estatico: funcao.static_,
                    },
                );
            }
            _ => {}
        }
    }
    saida
}

/// The binding name of `@Input('alias')`/`@Output('alias')`, or that of the
/// member.
fn apelido(arvore: &ast::Ast, a: &ast::Annotation) -> Option<String> {
    a.arguments.as_ref().and_then(|args| {
        args.args
            .first()
            .and_then(|arg| match &arvore.expr(arg.value).kind {
                ast::ExprKind::String(lit) => lit.constant_value().map(|s| s.to_string_lossy()),
                _ => None,
            })
    })
}

/// `@Input()` of each field or setter, in the order of the official `inputs`
/// map (`_collectInheritableMetadataOn`, in `find_components.dart`): the fields
/// first, in declaration order, then the setters. Inherited ones are not
/// included — a binding to them is refused because the input is not found.
fn entradas_da_classe(
    arvore: &ast::Ast,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> Vec<Entrada> {
    let mut campos = Vec::new();
    let mut setters = Vec::new();
    for &id in &classe.members {
        let membro = arvore.member(id);
        let Some(a) = membro
            .metadata
            .iter()
            .find(|a| crate::nome_da_anotacao(a, interner) == "Input")
        else {
            continue;
        };
        let apelido = apelido(arvore, a);
        match &membro.kind {
            ast::MemberKind::Field(lista) if !lista.static_ => {
                for v in lista.variables.iter() {
                    let nome = interner.resolve(v.name.sym).to_string();
                    campos.push(Entrada {
                        nome: apelido.clone().unwrap_or_else(|| nome.clone()),
                        campo: nome,
                    });
                }
            }
            ast::MemberKind::Method(f) => {
                let funcao = arvore.function(*f);
                if let (ast::FunctionKind::Setter, Some(n), false) =
                    (funcao.kind, funcao.name, funcao.static_)
                {
                    let nome = interner.resolve(n.sym).to_string();
                    setters.push(Entrada {
                        nome: apelido.clone().unwrap_or_else(|| nome.clone()),
                        campo: nome,
                    });
                }
            }
            _ => {}
        }
    }
    campos.extend(setters);
    campos
}

/// `@Output()` of each field or getter: name in the template -> member.
fn saidas_da_classe(
    arvore: &ast::Ast,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> Vec<(String, String)> {
    let mut campos = Vec::new();
    let mut getters = Vec::new();
    for &id in &classe.members {
        let membro = arvore.member(id);
        let Some(a) = membro
            .metadata
            .iter()
            .find(|a| crate::nome_da_anotacao(a, interner) == "Output")
        else {
            continue;
        };
        let apelido = apelido(arvore, a);
        match &membro.kind {
            ast::MemberKind::Field(lista) if !lista.static_ => {
                for v in lista.variables.iter() {
                    let nome = interner.resolve(v.name.sym).to_string();
                    campos.push((apelido.clone().unwrap_or_else(|| nome.clone()), nome));
                }
            }
            ast::MemberKind::Method(f) => {
                let funcao = arvore.function(*f);
                if let (ast::FunctionKind::Getter, Some(n), false) =
                    (funcao.kind, funcao.name, funcao.static_)
                {
                    let nome = interner.resolve(n.sym).to_string();
                    getters.push((apelido.clone().unwrap_or_else(|| nome.clone()), nome));
                }
            }
            _ => {}
        }
    }
    // The order of the `outputs` map: getters (the accessors are visited before
    // the fields by the class's `visitChildren`), then fields.
    getters.extend(campos);
    getters
}

/// Positional parameters (required and optional) of each instance method.
fn aridades_dos_metodos(
    arvore: &ast::Ast,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> std::collections::HashMap<String, usize> {
    let mut saida = std::collections::HashMap::new();
    for &id in &classe.members {
        let ast::MemberKind::Method(f) = &arvore.member(id).kind else {
            continue;
        };
        let funcao = arvore.function(*f);
        if !matches!(funcao.kind, ast::FunctionKind::Function) || funcao.static_ {
            continue;
        }
        let Some(nome) = funcao.name else { continue };
        let posicionais = funcao.parameters.as_ref().map_or(0, |ps| {
            ps.iter()
                .filter(|p| !matches!(p.kind, ast::ParameterKind::Named))
                .count()
        });
        saida.insert(interner.resolve(nome.sym).to_string(), posicionais);
    }
    saida
}

/// Return type of each instance method of the class.
fn tipos_dos_metodos(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> std::collections::HashMap<String, String> {
    let mut saida = std::collections::HashMap::new();
    for &id in &classe.members {
        let ast::MemberKind::Method(f) = &arvore.member(id).kind else {
            continue;
        };
        let funcao = arvore.function(*f);
        if !matches!(funcao.kind, ast::FunctionKind::Function) || funcao.static_ {
            continue;
        }
        let Some(nome) = funcao.name else { continue };
        let tipo = match funcao.return_type {
            Some(t) => texto_do_tipo(arvore, fonte, t),
            None => "dynamic".to_string(),
        };
        saida.insert(interner.resolve(nome.sym).to_string(), tipo);
    }
    saida
}

/// The type the analyzer infers for an initializer, in the shapes where it
/// depends on nothing outside the expression: a primitive literal without
/// interpolation, `Class(..)`, `prefix.Class(..)` and `new`/`const T(..)` (with
/// `T` as written). `Class.name(..)` may be a static method, and is left out.
pub(crate) fn tipo_inferido(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    e: ast::ExprId,
) -> Option<String> {
    let maiuscula = |n: &ast::Name| interner.resolve(n.sym).starts_with(char::is_uppercase);
    match &arvore.expr(e).kind {
        ast::ExprKind::String(lit) => lit.constant_value().map(|_| "String".to_string()),
        ast::ExprKind::Int(_) => Some("int".to_string()),
        ast::ExprKind::Double(_) => Some("double".to_string()),
        ast::ExprKind::Bool(_) => Some("bool".to_string()),
        ast::ExprKind::InstanceCreation { ty, .. } => Some(texto_do_tipo(arvore, fonte, *ty)),
        ast::ExprKind::Call { target, arguments } if arguments.type_args.is_empty() => {
            match &arvore.expr(*target).kind {
                ast::ExprKind::Identifier(n) if maiuscula(n) => {
                    Some(interner.resolve(n.sym).to_string())
                }
                ast::ExprKind::Property {
                    target: p,
                    name,
                    null_aware: false,
                } if maiuscula(name) => match &arvore.expr(*p).kind {
                    ast::ExprKind::Identifier(prefixo) if !maiuscula(prefixo) => Some(format!(
                        "{}.{}",
                        interner.resolve(prefixo.sym),
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
                } => match &arvore.expr(*p).kind {
                    ast::ExprKind::TypeArguments { target: classe, .. }
                        if matches!(&arvore.expr(*classe).kind,
                            ast::ExprKind::Identifier(n) if maiuscula(n)) =>
                    {
                        let sp = arvore.expr(*p).span;
                        fonte.get(sp.start..sp.end).map(|t| t.trim().to_string())
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
fn tipos_dos_campos(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> std::collections::HashMap<String, String> {
    let mut saida = std::collections::HashMap::new();
    for &id in &classe.members {
        let ast::MemberKind::Field(lista) = &arvore.member(id).kind else {
            continue;
        };
        let Some(t) = lista.ty else { continue };
        let tipo = texto_do_tipo(arvore, fonte, t);
        for v in lista.variables.iter() {
            saida.insert(interner.resolve(v.name.sym).to_string(), tipo.clone());
        }
    }
    saida
}

/// A `Typed<X>.of([..], on: 'ref')` of `directiveTypes:`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TipoDeDiretiva {
    /// A classe como escrita (`MaterialSelectItemComponent`, `p.X`).
    pub classe: String,
    /// The arguments: the text of each type, and `#T` as `T` (the component's
    /// type parameter, `fromTypeLink`).
    pub argumentos: Vec<String>,
    /// `on:`: only on the node with that `#ref`.
    pub em: Option<String>,
}

/// Reads `[Typed<X>.of([#T, String], on: 'a'), Typed<Y<int>>()]` from the text
/// (`TypedReader`). `None` for whatever does not have this shape.
fn ler_tipos_de_diretiva(texto: &str) -> Option<Vec<TipoDeDiretiva>> {
    let t = texto.trim();
    let t = t.strip_prefix("const").unwrap_or(t).trim();
    let t = t.strip_prefix('[')?.strip_suffix(']')?;
    let mut saida = Vec::new();
    for item in partir_no_topo(t, ',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let item = item.strip_prefix("const").unwrap_or(item).trim();
        let resto = item.strip_prefix("Typed")?.trim_start();
        let resto = resto.strip_prefix('<')?;
        // The argument of `Typed<..>`, up to the closing `>`.
        let mut nivel = 1i32;
        let mut fim = None;
        for (i, ch) in resto.char_indices() {
            match ch {
                '<' => nivel += 1,
                '>' => {
                    nivel -= 1;
                    if nivel == 0 {
                        fim = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let fim = fim?;
        let tipo = resto[..fim].trim();
        let depois = resto[fim + 1..].trim();
        let (classe, mut argumentos) = match tipo.find('<') {
            Some(i) => (
                tipo[..i].trim().to_string(),
                partir_no_topo(tipo[i + 1..].strip_suffix('>')?, ',')
                    .into_iter()
                    .map(|a| a.trim().to_string())
                    .collect::<Vec<_>>(),
            ),
            None => (tipo.to_string(), Vec::new()),
        };
        let (eh_of, chamada) = match depois.strip_prefix(".of") {
            Some(r) => (true, r.trim()),
            None => (false, depois),
        };
        let dentro = chamada.strip_prefix('(')?.strip_suffix(')')?;
        let mut em = None;
        for (k, parte) in partir_no_topo(dentro, ',').into_iter().enumerate() {
            let parte = parte.trim();
            if parte.is_empty() {
                continue;
            }
            if let Some(v) = parte.strip_prefix("on:") {
                let v = v.trim();
                let v = v
                    .strip_prefix('\'')
                    .and_then(|v| v.strip_suffix('\''))
                    .or_else(|| v.strip_prefix('"').and_then(|v| v.strip_suffix('"')))?;
                em = Some(v.to_string());
            } else if eh_of && k == 0 {
                if !argumentos.is_empty() {
                    return None;
                }
                let lista = parte.strip_prefix('[')?.strip_suffix(']')?;
                for a in partir_no_topo(lista, ',') {
                    let a = a.trim();
                    if a.is_empty() {
                        continue;
                    }
                    argumentos.push(match a.strip_prefix('#') {
                        Some(simbolo) => simbolo.trim().to_string(),
                        // `Typed<..>` nested as an argument: not yet.
                        None if a.starts_with("Typed") => return None,
                        None => a.to_string(),
                    });
                }
            } else {
                return None;
            }
        }
        saida.push(TipoDeDiretiva {
            classe,
            argumentos,
            em,
        });
    }
    Some(saida)
}

/// Parte `texto` em `sep` fora de `<>`, `()`, `[]` e `{}`.
fn partir_no_topo(texto: &str, sep: char) -> Vec<&str> {
    let mut partes = Vec::new();
    let mut nivel = 0i32;
    let mut inicio = 0;
    for (i, ch) in texto.char_indices() {
        match ch {
            '<' | '(' | '[' | '{' => nivel += 1,
            '>' | ')' | ']' | '}' => nivel -= 1,
            c if c == sep && nivel == 0 => {
                partes.push(&texto[inicio..i]);
                inicio = i + c.len_utf8();
            }
            _ => {}
        }
    }
    partes.push(&texto[inicio..]);
    partes
}
