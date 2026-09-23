//! Lê os argumentos de `@Component` e `@Directive` direto da árvore.
//!
//! O compilador oficial faz isso pelo `package:analyzer`, resolvendo cada
//! argumento como constante. Aqui os argumentos que interessam ao gerador são
//! literais escritos na própria anotação (`selector: 'x'`, `templateUrl:
//! 'x.html'`), e os que não são — a lista de `directives`, os `providers` —
//! ficam guardados como texto da fonte, para as etapas que souberem usá-los.
use crate::visao::{Motivo, Recusa, recusa};
use dartforge_frontend::ast;
use dartforge_intern::Interner;

/// `@Component(...)` ou `@Directive(...)` de uma classe, como está escrito —
/// o `CompileDirectiveMetadata` do oficial, na parte que o gerador usa.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Componente {
    pub classe: String,
    pub seletor: String,
    /// `@Component` (e não `@Directive`).
    pub e_componente: bool,
    /// A classe estende outra ou usa mixin: `@Input`, `@Output` e
    /// `@HostBinding` herdados não se veem daqui.
    pub herda: bool,
    /// `exportAs:` — o nome pelo qual `#ref="nome"` chega à diretiva.
    pub export_as: Option<String>,
    /// `@Output`s: nome no template -> membro (um getter de `Stream`), na
    /// ordem em que o oficial os coleta.
    pub saidas: Vec<(String, String)>,
    /// Algum `@HostBinding`: quem usa a diretiva chama o `detectHostChanges`
    /// dela.
    pub liga_hospedeiro: bool,
    /// `@ContentChild`/`@ContentChildren`: a consulta é atualizada pela visão
    /// de quem projeta o conteúdo.
    pub consulta_conteudo: bool,
    /// `providers:`/`viewProviders:` com provedores: entram no nó de quem usa
    /// e mudam a numeração dos provedores dele.
    pub com_provedores: bool,
    /// `template: '...'` quando o template está na própria anotação.
    pub template: Option<String>,
    /// `templateUrl: 'x.html'`.
    pub template_url: Option<String>,
    pub style_urls: Vec<String>,
    /// `styles: ['...']` — folhas escritas na anotação.
    pub styles: Vec<String>,
    /// `changeDetection: ChangeDetectionStrategy.OnPush`.
    pub on_push: bool,
    /// Nomes escritos em `directives:`, na ordem, como escritos (`A`,
    /// `li.B`). Listas constantes (`coreDirectives`) entram pelo nome e são
    /// expandidas pelo banco semântico.
    pub diretivas: Vec<String>,
    /// Algum item de `directives:` não é um nome (espalhamento, `if`,
    /// expressão): a lista não é conhecida.
    pub diretivas_ilegiveis: bool,
    /// Parâmetros do construtor, na ordem — o que a visão-hospedeira precisa
    /// para instanciar o componente.
    pub parametros: Vec<Parametro>,
    /// Ganchos de ciclo de vida que a classe implementa.
    pub ganchos: Ganchos,
    /// Toda forma do componente que o gerador ainda não traduz — anotação de
    /// membro (`@HostBinding`, `@ContentChild`…) ou argumento de
    /// `@Component` fora do que ele entende —, com o texto que o placar
    /// mostra. Todas, não só a primeira: é assim que o placar sabe quantos
    /// arquivos uma forma nova destrava. Enquanto houver uma, o arquivo não é
    /// nosso: gerar ignorando isso dá saída **errada**, não incompleta.
    pub nao_entendidos: Vec<Recusa>,
    /// Nomes escritos em `pipes:`, na ordem, como escritos. Como em
    /// `directives:`, uma lista constante (`commonPipes`) entra pelo nome e é
    /// expandida pelo banco semântico. A lista em si não muda a visão; o que
    /// muda é usar um pipe no template.
    pub pipes: Vec<String>,
    /// Algum item de `pipes:` não é um nome: a lista não é conhecida.
    pub pipes_ilegiveis: bool,
    /// `@ViewChild('ref')` em campo, na ordem de declaração — que é a ordem
    /// em que o oficial escreve as atribuições (`queryIndex`). Se a consulta
    /// sai estática ou não depende de onde `#ref` está no template.
    pub consultas: Vec<Consulta>,
    /// `@HostListener` da classe, na forma simples, na ordem de declaração.
    pub ouvintes: Vec<Ouvinte>,
    /// `@ContentChild`/`@ContentChildren`, na ordem do `directive.queries`
    /// (setters antes dos campos, como o visitante do oficial os visita).
    /// `None`: alguma fora da forma conhecida.
    pub consultas_de_conteudo: Option<Vec<ConsultaDeConteudo>>,
    /// Cada campo e getter de instância da classe. O emissor precisa disto
    /// para escolher entre `interpolateString`, `interpolate` e
    /// `updateTextWithPrimitive`, que o oficial decide pelo tipo estático da
    /// expressão do template e por ela ser mutável ou não. Membro estático
    /// fica de fora: o oficial o lê pela classe (`import1.X.nome`), forma que
    /// o conversor não escreve.
    pub membros: std::collections::HashMap<String, Membro>,
    /// `@Input`s, na ordem do mapa `inputs` do oficial — é a ordem em que as
    /// entradas de um nó são escritas (`_SortInputsVisitor`).
    pub entradas: Vec<Entrada>,
    /// Métodos da classe, separados dos campos: um método só pode aparecer
    /// como alvo de chamada (`titulo()`), e o seu tipo é o do retorno. Se
    /// entrassem no mesmo mapa, `{{ titulo }}` (tearoff) seria interpolado
    /// como se fosse o valor de retorno.
    pub metodos: std::collections::HashMap<String, String>,
    /// Parâmetros posicionais de cada método de instância: é o que decide,
    /// num tear-off (`(click)="salvar"`), entre `salvar()` e
    /// `salvar($event)` (`rewriteTearOff`).
    pub aridades: std::collections::HashMap<String, usize>,
}

/// Um `@Input`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// Nome da ligação no template (o apelido, se houver).
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

/// Ganchos de ciclo de vida do ngdart implementados pelo componente. Cada um
/// tem o seu lugar fixo no `detectChangesInternal` da visão-hospedeira.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Ganchos {
    pub on_init: bool,
    pub on_destroy: bool,
    pub do_check: bool,
    pub after_content_init: bool,
    pub after_content_checked: bool,
    pub after_view_init: bool,
    pub after_view_checked: bool,
    /// `AfterChanges`: não aparece no arquivo do próprio componente, mas
    /// muda a detecção de quem o usa com `@Input`.
    pub after_changes: bool,
}

impl Ganchos {
    /// Algum gancho põe código na visão-hospedeira?
    pub fn algum(&self) -> bool {
        self.on_init
            || self.on_destroy
            || self.do_check
            || self.after_content_init
            || self.after_content_checked
            || self.after_view_init
            || self.after_view_checked
    }

    /// `firstCheck` só é declarado quando algum gancho o usa.
    pub fn usa_primeira_checagem(&self) -> bool {
        self.on_init || self.after_content_init || self.after_view_init
    }

    /// Há detecção de mudança na hospedeira? (`ngOnDestroy` sozinho só gera
    /// `destroyInternal`.)
    pub fn tem_deteccao(&self) -> bool {
        self.on_init
            || self.do_check
            || self.after_content_init
            || self.after_content_checked
            || self.after_view_init
            || self.after_view_checked
    }
}

/// Um campo ou getter da classe do componente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membro {
    /// Tipo como escrito, ou inferido do inicializador; vazio quando não se
    /// sabe (`final x = A.b;`).
    pub tipo: String,
    /// `final` (ou getter): conta como imutável na regra `isImmutable` do
    /// ngcompiler, que decide se o valor primitivo vai pelo caminho rápido.
    pub imutavel: bool,
}

/// Um `@ViewChild('ref') T? campo;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consulta {
    /// Campo que recebe o resultado.
    pub propriedade: String,
    /// A referência procurada no template (`#ref`).
    pub referencia: String,
    /// Tipo do campo, como escrito (`html.DivElement?`). Decide o valor: um
    /// `Element` recebe o próprio nó, qualquer outro tipo um `ElementRef`
    /// (`isElementType` em `find_components.dart`).
    pub tipo: String,
}

/// Um `@ContentChild`/`@ContentChildren`, como quem usa o componente precisa
/// dele: sem resultado no conteúdo, a lista recebe `[]` e o único não recebe
/// nada (`_createUpdates`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultaDeConteudo {
    /// Campo ou setter que recebe o resultado.
    pub campo: String,
    /// `@ContentChildren`.
    pub lista: bool,
    /// O tipo procurado, como escrito (`LiTabComponent`, `li.X`), ou a
    /// referência (`'nome'`, com `nome` aqui e `referencia` ligado).
    pub alvo: String,
    pub referencia: bool,
    /// `descendants:` — sem ele, só casa o conteúdo a uma diretiva de
    /// distância (`_getQueriesFor`).
    pub descendentes: bool,
}

/// Um `@HostListener` do componente: o evento e o texto do handler que o
/// oficial monta (`_addHostListener`: `metodo(args)`, com `$event`
/// deduzido quando não há `args` e o método tem um parâmetro). O texto passa
/// pelo mesmo conversor de handlers do template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ouvinte {
    pub evento: String,
    pub handler: String,
}

/// Um parâmetro do construtor do componente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parametro {
    /// Tipo como escrito (`Element`, `RestConfig`, `List<int>`).
    pub tipo: Option<String>,
    pub nome: String,
    pub nomeado: bool,
    /// `@Optional`, `@Inject(...)`, `@Self`… mudam a injeção.
    pub anotado: bool,
    /// A única anotação é `@Optional()`: `injectorGetOptional`.
    pub opcional: bool,
}

/// Valor de um argumento nomeado, quando é uma string literal sem
/// interpolação.
fn texto_do_argumento(arvore: &ast::Ast, id: ast::ExprId) -> Option<String> {
    match &arvore.expr(id).kind {
        ast::ExprKind::String(lit) => lit.constant_value().map(|s| s.to_string_lossy()),
        ast::ExprKind::Parenthesized(inner) => texto_do_argumento(arvore, *inner),
        _ => None,
    }
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

/// Nomes escritos numa lista literal (`directives: [A, li.B]`), e se algum
/// item não é um nome.
fn nomes_da_lista(arvore: &ast::Ast, interner: &Interner, id: ast::ExprId) -> (Vec<String>, bool) {
    let ast::ExprKind::List { elements, .. } = &arvore.expr(id).kind else {
        return (Vec::new(), true);
    };
    let mut nomes = Vec::new();
    let mut ilegivel = false;
    for e in elements.iter() {
        match e {
            ast::CollectionElement::Expression(x) => {
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

/// `ChangeDetectionStrategy.onPush` (o nome no ngdart 8) — o que muda o
/// estado inicial da visão.
fn e_on_push(arvore: &ast::Ast, fonte: &str, id: ast::ExprId) -> bool {
    let span = arvore.expr(id).span;
    fonte
        .get(span.start..span.end)
        .is_some_and(|t| t.rsplit('.').next().map(str::trim) == Some("onPush"))
}

/// Extrai o `@Component` de uma classe anotada.
pub fn ler_componente(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
    anotacao: &ast::Annotation,
) -> Componente {
    ler(arvore, fonte, interner, classe, anotacao, true)
}

/// Extrai a `@Directive` de uma classe anotada: o que quem a usa precisa
/// saber dela (seletor, entradas, saídas, construtor, ciclo de vida).
pub fn ler_diretiva(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
    anotacao: &ast::Annotation,
) -> Componente {
    ler(arvore, fonte, interner, classe, anotacao, false)
}

/// Um `@Pipe`, como o `PipeVisitor` do oficial o lê: o nome, se é puro e o
/// tipo do `transform` (`fromFunctionType`) — é dele que sai o tipo do
/// `pureProxyN` de cada chamada.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pipe {
    pub classe: String,
    /// `@Pipe('nome')`.
    pub nome: String,
    /// `pure:`, `true` quando omitido (`coerceBool(.., defaultTo: true)`).
    pub puro: bool,
    /// Retorno do `transform`, como escrito (`String?`).
    pub retorno: String,
    /// Tipo de cada parâmetro posicional do `transform`, na ordem; sem tipo
    /// escrito, `None` (o tipo inferido de uma sobrescrita daqui não se vê).
    pub parametros: Vec<Option<String>>,
    /// Por que este pipe ainda não é instanciado pelo gerador: injeção no
    /// construtor (`createPipeInstance` pede ao injetor), `OnDestroy`
    /// (`bindPipeDestroyLifecycleCallbacks`), `transform` herdado, sem tipo
    /// de retorno ou com parâmetro nomeado.
    pub fora: Option<&'static str>,
}

/// Extrai o `@Pipe` de uma classe anotada.
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
                        None => p.fora = Some("nome do @Pipe que não é literal"),
                    }
                }
                Some("name") if !nome_lido => {
                    nome_lido = true;
                    match texto_do_argumento(arvore, a.value) {
                        Some(n) => p.nome = n,
                        None => p.fora = Some("nome do @Pipe que não é literal"),
                    }
                }
                Some("pure") => match &arvore.expr(a.value).kind {
                    ast::ExprKind::Bool(b) => p.puro = *b,
                    _ => p.fora = Some("pure: que não é literal"),
                },
                _ => p.fora = Some("argumento do @Pipe fora do conhecido"),
            }
        }
    }
    if !parametros_do_construtor(arvore, fonte, interner, classe).is_empty() {
        p.fora = Some("pipe com injeção no construtor");
    }
    if ganchos_da_classe(arvore, fonte, classe).on_destroy {
        p.fora = Some("pipe com OnDestroy");
    }
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
            None => p.fora = Some("transform sem tipo de retorno"),
        }
        for par in funcao.parameters.as_deref().unwrap_or(&[]) {
            if matches!(par.kind, ast::ParameterKind::Named) {
                p.fora = Some("transform com parâmetro nomeado");
                continue;
            }
            p.parametros
                .push(par.ty.map(|t| texto_do_tipo(arvore, fonte, t)));
        }
    }
    if !achou {
        p.fora = Some("transform herdado ou ausente");
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
                "template" => c.template = texto_do_argumento(arvore, a.value),
                "templateUrl" => c.template_url = texto_do_argumento(arvore, a.value),
                "styleUrls" => c.style_urls = lista_de_textos(arvore, a.value),
                "styles" => c.styles = lista_de_textos(arvore, a.value),
                "changeDetection" => c.on_push = e_on_push(arvore, fonte, a.value),
                "directives" => {
                    (c.diretivas, c.diretivas_ilegiveis) = nomes_da_lista(arvore, interner, a.value)
                }
                "pipes" => (c.pipes, c.pipes_ilegiveis) = nomes_da_lista(arvore, interner, a.value),
                "exports" => exportados = nomes_da_lista(arvore, interner, a.value).0,
                "providers" | "viewProviders" => c.com_provedores |= !lista_vazia(arvore, a.value),
                _ => {}
            }
        }
    }
    c.parametros = parametros_do_construtor(arvore, fonte, interner, classe);
    c.membros = tipos_dos_membros(arvore, fonte, interner, classe);
    c.metodos = tipos_dos_metodos(arvore, fonte, interner, classe);
    c.aridades = aridades_dos_metodos(arvore, interner, classe);
    // `exports:` põe nomes estáticos no escopo das expressões, e eles vencem
    // o membro de mesmo nome (`_matchExport` antes do receptor implícito).
    // Um membro sombreado assim sai do mapa, e usá-lo é recusado.
    for n in &exportados {
        let simples = n.split('.').next().unwrap_or(n);
        c.membros.remove(simples);
        c.metodos.remove(simples);
        c.aridades.remove(simples);
    }
    c.entradas = entradas_da_classe(arvore, interner, classe);
    c.saidas = saidas_da_classe(arvore, interner, classe);
    c.ganchos = ganchos_da_classe(arvore, fonte, classe);
    c.nao_entendidos = o_que_nao_entendemos(arvore, interner, classe, anotacao, e_componente);
    c.liga_hospedeiro = tem_anotacao(arvore, interner, classe, &["HostBinding"]);
    c.consulta_conteudo = tem_anotacao(
        arvore,
        interner,
        classe,
        &["ContentChild", "ContentChildren"],
    );
    c.consultas = consultas_da_classe(arvore, fonte, interner, classe, &mut c.nao_entendidos);
    c.ouvintes = ouvintes_da_classe(arvore, interner, classe, &mut c.nao_entendidos);
    c.consultas_de_conteudo = consultas_de_conteudo(arvore, interner, classe);
    c
}

/// Os `@ContentChild`/`@ContentChildren` da classe: setters, depois campos,
/// cada grupo em ordem de declaração. `None` se algum sai da forma
/// `@ContentChild(Tipo)`/`@ContentChild('ref')` num campo ou setter.
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
            // `ContentChildren(descendants: true)` por omissão no ngdart 8;
            // `ContentChild` sempre. `read:` troca o valor: ainda não.
            let mut descendentes = true;
            for x in a.arguments.as_ref().map(|g| &g.args[..]).unwrap_or(&[]) {
                match x.name.map(|n| interner.resolve(n.sym)) {
                    None => {}
                    Some("descendants") if lista => match &arvore.expr(x.value).kind {
                        ast::ExprKind::Bool(b) => descendentes = *b,
                        _ => return None,
                    },
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

/// Algum membro da classe tem uma destas anotações?
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

/// Argumentos de `@Component` cujo efeito o gerador conhece. `pipes:` entra
/// aqui porque a lista não muda a visão; usar um pipe no template, sim, e
/// isso é conferido contra o template. `exports:` só põe nomes no escopo
/// das expressões (ver [`ler`]).
const ARGUMENTOS_CONHECIDOS: &[&str] = &[
    "selector",
    "template",
    "templateUrl",
    "styleUrls",
    "styles",
    "changeDetection",
    "directives",
    "exports",
    "pipes",
];

/// Argumentos de `@Directive` cujo efeito o gerador conhece.
const ARGUMENTOS_DE_DIRETIVA: &[&str] = &["selector", "exportAs"];

/// Interfaces de ciclo de vida implementadas, pelas cláusulas `implements` e
/// `with`. (As do ngrouter — `OnActivate`, `CanDeactivate` — não mexem na
/// visão; `AfterChanges` tampouco aparece no arquivo do próprio componente.)
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

/// Tudo o que nesta classe o gerador ainda não sabe traduzir, sem parar no
/// primeiro. Recusar é obrigatório: gerar sem isso produz um arquivo
/// **errado**.
///
/// `@ViewChild` e `@HostListener` não entram aqui: cada um tem a sua
/// leitura ([`consultas_da_classe`], [`ouvintes_da_classe`]), que recusa o
/// que não souber.
fn o_que_nao_entendemos(
    arvore: &ast::Ast,
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
                // A lista vazia não muda a visão (caso b18 do corpus) — e é
                // 57 dos 70 `providers:` do new_sali.
                "providers" if lista_vazia(arvore, a.value) => {}
                "providers" => fora.push(recusa(
                    Motivo::Providers,
                    format!("{anot}(.., providers: [..])"),
                )),
                "encapsulation" => fora.push(recusa(
                    Motivo::Encapsulamento,
                    "@Component(.., encapsulation: ..)",
                )),
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
                "HostBinding" => Motivo::HostBindingEmComponente,
                "ContentChild" | "ContentChildren" => Motivo::ContentChild,
                // Lista de resultados: a atribuição é outra, e com `*ngIf`
                // no caminho vira `mapNestedViews`.
                "ViewChildren" => Motivo::ViewChildDinamico,
                _ => continue,
            };
            fora.push(recusa(motivo, format!("@{nome}")));
        }
    }
    fora
}

/// `[]` literal, sem elemento nenhum (comentário dentro não conta).
fn lista_vazia(arvore: &ast::Ast, id: ast::ExprId) -> bool {
    matches!(&arvore.expr(id).kind, ast::ExprKind::List { elements, .. } if elements.is_empty())
}

/// Os `@ViewChild` da classe. Só entra como consulta a forma
/// `@ViewChild('ref') T? campo;` — seletor de texto, sem `read:`, num campo
/// com tipo escrito. O resto é recusado aqui mesmo, com o motivo.
fn consultas_da_classe(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
    fora: &mut Vec<Recusa>,
) -> Vec<Consulta> {
    let mut saida = Vec::new();
    for &id in &classe.members {
        let membro = arvore.member(id);
        for a in membro.metadata.iter() {
            if crate::nome_da_anotacao(a, interner) != "ViewChild" {
                continue;
            }
            match consulta_simples(arvore, fonte, interner, membro, a) {
                Ok(c) => saida.push(c),
                Err(r) => fora.push(r),
            }
        }
    }
    saida
}

/// Os `@HostListener` da classe, na ordem de declaração dos métodos (o
/// `DirectiveVisitor` visita os métodos em ordem). O handler é o texto de
/// `_addHostListener` (`find_components.dart`): sem `args`, um método de um
/// parâmetro recebe `$event`.
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
                // Dois ouvintes do mesmo evento: o mapa do oficial fica com o
                // último, no lugar do primeiro. Ainda não.
                Some(_) => fora.push(recusa(
                    Motivo::HostListenerEmComponente,
                    "dois @HostListener do mesmo evento",
                )),
                None => fora.push(recusa(
                    Motivo::HostListenerEmComponente,
                    "@HostListener de evento não nativo ou fora da forma `m(args)`",
                )),
            }
        }
    }
    saida
}

/// Lê um `@HostListener`, ou `None` se ele sai da forma conhecida.
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
    // Evento que não é do DOM (`document:click`, `keyup.enter`) vai pelo
    // `eventManager` e, com `:`, nem é aceito pelo oficial; ainda não.
    if !crate::visao::evento_nativo(&evento) {
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

/// Lê um `@ViewChild` na forma que o gerador conhece, ou diz por que não.
fn consulta_simples(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    membro: &ast::Member,
    anotacao: &ast::Annotation,
) -> Result<Consulta, Recusa> {
    let args = anotacao
        .arguments
        .as_ref()
        .ok_or_else(|| recusa(Motivo::NaoEntendido, "@ViewChild sem argumento"))?;
    let [unico] = &args.args[..] else {
        // `read:` troca o valor por um provedor do nó; `first:`,
        // `descendants:` não fazem sentido aqui. Todos ainda não.
        let tem_read = args
            .args
            .iter()
            .any(|x| x.name.is_some_and(|n| interner.resolve(n.sym) == "read"));
        return Err(if tem_read {
            recusa(Motivo::ViewChildEmFilho, "@ViewChild(.., read: T)")
        } else {
            recusa(Motivo::NaoEntendido, "@ViewChild com opções")
        });
    };
    if unico.name.is_some() {
        return Err(recusa(
            Motivo::NaoEntendido,
            "@ViewChild só com argumento nomeado",
        ));
    }
    // Seletor de tipo (`@ViewChild(OutroComp)`) consulta um componente ou
    // diretiva, não um elemento.
    let Some(referencia) = texto_do_argumento(arvore, unico.value) else {
        return Err(recusa(Motivo::ViewChildEmFilho, "@ViewChild(Tipo)"));
    };
    // `'a,b'` são dois seletores numa consulta só.
    if referencia.contains(',') || referencia.trim() != referencia || referencia.is_empty() {
        return Err(recusa(Motivo::NaoEntendido, "@ViewChild('a,b')"));
    }
    // Só campo de instância com tipo escrito; setter tem outra regra de tipo
    // (o do parâmetro) e não aparece nos projetos.
    let ast::MemberKind::Field(lista) = &membro.kind else {
        return Err(recusa(Motivo::NaoEntendido, "@ViewChild em setter"));
    };
    if lista.static_ || lista.final_ || lista.const_ || lista.late || lista.variables.len() != 1 {
        return Err(recusa(
            Motivo::NaoEntendido,
            "@ViewChild em campo final/late/estático",
        ));
    }
    let Some(t) = lista.ty else {
        return Err(recusa(Motivo::NaoEntendido, "@ViewChild em campo sem tipo"));
    };
    Ok(Consulta {
        propriedade: interner.resolve(lista.variables[0].name.sym).to_string(),
        referencia,
        tipo: texto_do_tipo(arvore, fonte, t),
    })
}

/// Parâmetros do construtor gerador (o sem nome). Sem construtor declarado, a
/// classe tem o construtor implícito sem parâmetros.
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
            continue; // construtor nomeado não é o que a visão usa
        }
        let campos = tipos_dos_campos(arvore, fonte, interner, classe);
        return ctor
            .parameters
            .iter()
            .map(|p| {
                let nome = p
                    .name
                    .map(|n| interner.resolve(n.sym).to_string())
                    .unwrap_or_default();
                // `C(this.x)` não escreve tipo nenhum: o tipo do parâmetro é o
                // do campo. É assim que quase todo componente ngdart recebe as
                // suas dependências, e sem isto a injeção não sai.
                let tipo = match p.ty {
                    Some(t) => Some(texto_do_tipo(arvore, fonte, t)),
                    None if p.this_ || p.super_ => campos.get(&nome).cloned(),
                    None => None,
                };
                Parametro {
                    tipo,
                    nome,
                    nomeado: matches!(p.kind, ast::ParameterKind::Named),
                    anotado: !p.metadata.is_empty(),
                    opcional: p.metadata.len() == 1
                        && crate::nome_da_anotacao(&p.metadata[0], interner) == "Optional",
                }
            })
            .collect();
    }
    Vec::new()
}

/// Texto-fonte de um tipo, como escrito.
fn texto_do_tipo(arvore: &ast::Ast, fonte: &str, t: ast::TypeId) -> String {
    let s = arvore.ty(t).span;
    fonte.get(s.start..s.end).unwrap_or("").to_string()
}

/// Tipo de cada campo e getter de instância da classe, para o emissor saber
/// o tipo estático de `{{ nome }}`.
///
/// A imutabilidade é a do `isImmutable` do oficial: o valor vem de
/// `lookUpGetter(n).variable`, e só um campo de verdade (`!isSynthetic`)
/// `final`/`const` é imutável. Num getter escrito à mão a `variable` é
/// sintética: **mutável**.
fn tipos_dos_membros(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> std::collections::HashMap<String, Membro> {
    let mut saida = std::collections::HashMap::new();
    for &id in &classe.members {
        match &arvore.member(id).kind {
            ast::MemberKind::Field(lista) => {
                if lista.static_ {
                    continue;
                }
                let imutavel = lista.final_ || lista.const_;
                for v in lista.variables.iter() {
                    // Sem tipo escrito (`var x = Foo()`), o tipo é o que o
                    // analyzer infere do inicializador — só nas formas em que
                    // ele é certo.
                    let tipo = match lista.ty {
                        Some(t) => texto_do_tipo(arvore, fonte, t),
                        // Fora dessas formas o membro existe, mas sem tipo
                        // (texto vazio): quem precisa do tipo recusa.
                        None => v
                            .initializer
                            .and_then(|e| tipo_inferido(arvore, fonte, interner, e))
                            .unwrap_or_default(),
                    };
                    saida.insert(
                        interner.resolve(v.name.sym).to_string(),
                        Membro { tipo, imutavel },
                    );
                }
            }
            ast::MemberKind::Method(f) => {
                let funcao = arvore.function(*f);
                if !matches!(funcao.kind, ast::FunctionKind::Getter) || funcao.static_ {
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
                    },
                );
            }
            _ => {}
        }
    }
    saida
}

/// O nome de ligação de `@Input('apelido')`/`@Output('apelido')`, ou o do
/// membro.
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

/// `@Input()` de cada campo ou setter, na ordem do mapa `inputs` do oficial
/// (`_collectInheritableMetadataOn`, em `find_components.dart`): os campos
/// primeiro, em ordem de declaração, depois os setters. Herdados não entram
/// — ligação a eles é recusada por não achar a entrada.
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

/// `@Output()` de cada campo ou getter: nome no template -> membro.
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
    // A ordem do mapa `outputs`: getters (os acessores são visitados antes
    // dos campos pelo `visitChildren` da classe), depois campos.
    getters.extend(campos);
    getters
}

/// Parâmetros posicionais (obrigatórios e opcionais) de cada método de
/// instância.
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

/// Retorno de cada método de instância da classe.
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

/// O tipo que o analyzer infere para um inicializador, nas formas em que ele
/// não depende de nada fora da expressão: literal primitivo sem
/// interpolação, `Classe(..)`, `prefixo.Classe(..)` e `new`/`const T(..)`
/// (com `T` como escrito). `Classe.nome(..)` pode ser método estático, e
/// fica de fora.
fn tipo_inferido(
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
                _ => None,
            }
        }
        _ => None,
    }
}

/// Tipo de cada campo da classe, para resolver os parâmetros `this.x`.
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
