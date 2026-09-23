//! Emissor das visões: o Dart que o ngdart espera encontrar no
//! `.template.dart`.
//!
//! A ABI é o contrato, e ela é fixa: para um componente `X` com seletor `s`, o
//! arquivo tem `ViewX0 extends ComponentView<X>`, a fábrica constante
//! `_XNgFactory`, o getter `XNgFactory`, a função `createXFactory()` e a
//! visão-hospedeira `_ViewXHost0 extends HostView<X>` com sua
//! `viewFactory_XHost0()`. É por esses nomes que a aplicação e o `ngrouter`
//! chegam ao componente.
//!
//! Os `importN` saem numerados na ordem em que o emissor oficial os aloca — o
//! `dart:html` no construtor da visão, o `dom_helpers` no primeiro uso dentro
//! do `build()`, e `package:ngdart/angular.dart` ocupando um número mas
//! escrito sem prefixo, porque `ComponentFactory` aparece sem qualificar. É só
//! por isso que a saída pode ser comparada byte a byte com a do oficial.
use crate::componente::Componente;
use crate::dom;
use crate::html::No;
use crate::resolucao::{Resolucao, asset_de_uri, caminho_do_import};
use std::fmt::Write;
use std::path::Path;

/// Tabela de imports do arquivo gerado.
#[derive(Default)]
pub struct Importacoes {
    itens: Vec<(String, bool)>,
}

impl Importacoes {
    /// Aloca (ou reaproveita) o número de uma URI e devolve o prefixo — vazio
    /// para as bibliotecas que o oficial importa sem prefixo.
    pub fn alias(&mut self, uri: &str) -> String {
        if SEM_PREFIXO.contains(&uri) {
            self.indice(uri, false);
            return String::new();
        }
        let n = self.indice(uri, true);
        format!("import{n}")
    }

    /// `alias` seguido de ponto, ou vazio: `import3.NgFor` e `NgIf`.
    pub fn q(&mut self, uri: &str) -> String {
        let a = self.alias(uri);
        if a.is_empty() { a } else { format!("{a}.") }
    }

    /// Aloca um número sem prefixo — a URI é importada aberta.
    pub fn sem_alias(&mut self, uri: &str) {
        self.indice(uri, false);
    }

    fn indice(&mut self, uri: &str, com_alias: bool) -> usize {
        if let Some(i) = self.itens.iter().position(|(u, _)| u == uri) {
            return i;
        }
        self.itens.push((uri.to_string(), com_alias));
        self.itens.len() - 1
    }

    fn escrever(&self, saida: &mut String) {
        for (i, (uri, com_alias)) in self.itens.iter().enumerate() {
            if *com_alias {
                let _ = writeln!(saida, "import '{uri}' as import{i};");
            } else {
                let _ = writeln!(saida, "import '{uri}';");
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
const NG_IF: &str = "package:ngdart/src/common/directives/ng_if.dart";
const NG_FOR: &str = "package:ngdart/src/common/directives/ng_for.dart";
const EMBEDDED_VIEW: &str = "package:ngdart/src/core/linker/views/embedded_view.dart";
const RENDER_VIEW: &str = "package:ngdart/src/core/linker/views/render_view.dart";
const DIRECTIVE_CHANGE_DETECTOR: &str =
    "package:ngdart/src/core/change_detection/directive_change_detector.dart";

/// Bibliotecas que o emissor oficial importa **sem prefixo**, por serem a API
/// pública do ngdart (`_allowListedImports` em `output/dart_emitter.dart`).
const SEM_PREFIXO: &[&str] = &[
    ANGULAR,
    "dart:core",
    "package:ngdart/src/core/linker/element_ref.dart",
    VIEW_CONTAINER,
    TEMPLATE_REF,
    "package:ngdart/src/core/change_detection/change_detection.dart",
    NG_IF,
    "package:ngdart/src/core/linker/app_view.dart",
    "package:ngdart/src/core/render/api.dart",
];
const INTERPOLATE: &str = "package:ngdart/src/runtime/interpolate.dart";

/// Por que um arquivo ainda não é gerado por nós. O placar conta por motivo:
/// é isso que diz qual forma vale a pena aprender em seguida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Motivo {
    /// `@Directive` ou `@Pipe` no mesmo arquivo.
    DiretivaOuPipe,
    /// `@GenerateInjector`.
    Injetor,
    /// `@HostBinding`/`@HostListener` em diretiva numa forma que o
    /// `DirectiveChangeDetector` gerado ainda não cobre (herança, ligação que
    /// não é `class.x`, várias diretivas no arquivo).
    HostBindingEmDiretiva,
    /// Mais de um componente no arquivo.
    VariosComponentes,
    /// `styleUrls`/`styles`: mexem em `styles$X` e ligam o shim de estilo.
    Estilos,
    /// Parâmetro anotado (`@Optional`, `@Inject(…)`, `@Attribute`…).
    InjecaoAnotada,
    /// Parâmetro nomeado no construtor.
    InjecaoNomeada,
    /// Token genérico (`List<X>`, `OpaqueToken<String>`).
    InjecaoGenerica,
    /// Parâmetro sem tipo escrito.
    InjecaoSemTipo,
    /// O banco semântico não achou a biblioteca que declara o tipo.
    InjecaoNaoResolvida,
    /// `[x]`, `(x)`, `[(x)]`, `#ref` ou `*ngIf` no template.
    Ligacao,
    /// `{{ … }}` no template.
    Interpolacao,
    /// Tag que não é HTML: componente ou diretiva da aplicação.
    ComponenteNoTemplate,
    /// `<ng-content>`.
    Projecao,
    /// `style="..."` em linha.
    EstiloEmLinha,
    /// Arquivo `.html` do `templateUrl` não encontrado.
    TemplateAusente,
    /// `@Input`/`@Output` num componente filho.
    LigacaoEmFilho,
    /// Atributo ou ligação que pertence a uma diretiva do ecossistema
    /// (`ngClass`, `ngModel`…), não ao DOM.
    Diretiva,
    /// `providers:` com provedores: a injeção do elemento hospedeiro.
    Providers,
    /// `@ViewChild` cujo alvo está numa visão embutida (`*ngIf`, `*ngFor`),
    /// `@ViewChildren`, ou referência que o template não tem.
    ViewChildDinamico,
    /// `@ViewChild` que consulta um componente ou diretiva (seletor de tipo,
    /// `read:`, `#ref` em componente filho, campo que não é `Element`).
    ViewChildEmFilho,
    /// Pipe usado no template (`x | nome`, `$pipe.nome(x)`).
    PipesUsados,
    /// `encapsulation:` — muda o shim de estilo.
    Encapsulamento,
    /// `@HostListener` num componente.
    HostListenerEmComponente,
    /// `@HostBinding` num componente: o `detectHostChanges` da visão.
    HostBindingEmComponente,
    /// `@ContentChild`/`@ContentChildren`.
    ContentChild,
    /// Outra forma do componente que o gerador não sabe traduzir e, por
    /// isso, recusa — argumento desconhecido de `@Component`, `@ViewChild`
    /// com opções.
    NaoEntendido,
}

impl Motivo {
    pub fn texto(self) -> &'static str {
        match self {
            Motivo::DiretivaOuPipe => "diretiva ou pipe",
            Motivo::Injetor => "@GenerateInjector",
            Motivo::HostBindingEmDiretiva => "@HostBinding/@HostListener em diretiva",
            Motivo::VariosComponentes => "vários componentes no arquivo",
            Motivo::Estilos => "folha de estilo",
            Motivo::InjecaoAnotada => "injeção: parâmetro anotado",
            Motivo::InjecaoNomeada => "injeção: parâmetro nomeado",
            Motivo::InjecaoGenerica => "injeção: token genérico",
            Motivo::InjecaoSemTipo => "injeção: parâmetro sem tipo",
            Motivo::InjecaoNaoResolvida => "injeção: tipo não resolvido",
            Motivo::Ligacao => "ligação no template",
            Motivo::Interpolacao => "interpolação",
            Motivo::ComponenteNoTemplate => "componente no template",
            Motivo::Projecao => "<ng-content>",
            Motivo::EstiloEmLinha => "style em linha",
            Motivo::TemplateAusente => "template não encontrado",
            Motivo::LigacaoEmFilho => "ligação em componente filho",
            Motivo::Diretiva => "ligação de diretiva",
            Motivo::Providers => "providers: [..]",
            Motivo::ViewChildDinamico => "@ViewChild em visão embutida / @ViewChildren",
            Motivo::ViewChildEmFilho => "@ViewChild de componente ou diretiva",
            Motivo::PipesUsados => "pipe usado no template",
            Motivo::Encapsulamento => "encapsulation:",
            Motivo::HostListenerEmComponente => "@HostListener em componente",
            Motivo::HostBindingEmComponente => "@HostBinding em componente",
            Motivo::ContentChild => "@ContentChild",
            Motivo::NaoEntendido => "outra forma do componente não entendida",
        }
    }
}

/// Todos os motivos que impedem a geração deste componente, não só o
/// primeiro. Sem isto o placar engana: um arquivo que trava em folha de
/// estilo pode travar também em ligação e interpolação, e contar só o
/// primeiro faz parecer que aprender uma forma destrava o arquivo.
pub fn motivos(
    c: &Componente,
    local: &Local,
    nos: &[No],
    resolvedor: Option<&dyn Resolucao>,
    filhos: &std::collections::HashMap<String, Filho>,
) -> std::collections::BTreeSet<Motivo> {
    let mut fora = std::collections::BTreeSet::new();
    fora.extend(c.nao_entendidos.iter().map(|(m, _)| *m));
    fora.extend(formas_contra_o_template(c, local, nos, resolvedor, filhos));
    // O diagnóstico roda a mesma conta do gerador: marcar toda folha como
    // pendente escondia o que já funciona.
    if !c.styles.is_empty() || c.style_urls.len() > 1 {
        fora.insert(Motivo::Estilos);
    } else if let Some(url) = c.style_urls.first() {
        if local.uri_do_estilo(url).is_none() || !crate::estilo_compila(local.caminho, url) {
            fora.insert(Motivo::Estilos);
        }
    }
    if let Some(m) = falta_para_construir(c, local, resolvedor) {
        fora.insert(m);
    }
    let livres = referencias_livres(nos);
    motivos_dos_nos(nos, filhos, &livres, false, &mut fora);
    fora
}

/// Os `#ref` que o emissor aceita num elemento HTML: sem valor (`#f="ngForm"`
/// aponta para uma diretiva) e sem uso em expressão do template — usado, o
/// nome vira local da visão e a leitura muda. Um `#ref` assim é só um nome
/// para o nó, e quem o lê é o `@ViewChild`.
fn referencias_livres(nos: &[No]) -> std::collections::HashSet<String> {
    fn todas(nos: &[No], saida: &mut Vec<(String, String)>) {
        for n in nos {
            if let No::Elemento(e) = n {
                saida.extend(
                    e.referencias
                        .iter()
                        .map(|r| (r.nome.clone(), r.valor.clone())),
                );
                todas(&e.filhos, saida);
            }
        }
    }
    let mut refs = Vec::new();
    todas(nos, &mut refs);
    refs.iter()
        .filter(|(nome, valor)| valor.is_empty() && !local_citado(nos, nome))
        .map(|(nome, _)| nome.clone())
        .collect()
}

/// As formas do componente que só se decidem olhando o template: se o
/// `pipes:` é usado e onde está o `#ref` de cada `@ViewChild`. Devolve o que
/// ainda impede a geração, na ordem em que aparece.
fn formas_contra_o_template(
    c: &Componente,
    local: &Local,
    nos: &[No],
    resolvedor: Option<&dyn Resolucao>,
    filhos: &std::collections::HashMap<String, Filho>,
) -> Vec<Motivo> {
    let mut fora = Vec::new();
    // `pipes:` sem uso não muda a visão (caso b19); usado, cria o pipe e o
    // `pureProxy` no `build()` — ainda não.
    if c.pipes && usa_pipe(nos) {
        fora.push(Motivo::PipesUsados);
    }
    for consulta in &c.consultas {
        let mut lugares = Vec::new();
        onde_esta(nos, &consulta.referencia, filhos, false, &mut lugares);
        let motivo = match lugares.as_slice() {
            // `isElementType`: campo `Element` (ou subtipo) recebe o nó;
            // qualquer outro, um `ElementRef`. O primeiro caso é o estático,
            // que sai no `build()`.
            [Lugar::Raiz] if e_tipo_de_elemento(&consulta.tipo, local, resolvedor) => continue,
            [Lugar::Raiz] | [Lugar::Filho] => Motivo::ViewChildEmFilho,
            // Dentro de `*` a consulta passa por `mapNestedViews`; sem
            // resultado, ou com dois, a regra é outra — nada disso ainda.
            _ => Motivo::ViewChildDinamico,
        };
        fora.push(motivo);
    }
    fora
}

/// Onde um `#ref` aparece no template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lugar {
    /// Num elemento HTML da própria visão.
    Raiz,
    /// Dentro de uma visão embutida (`*ngIf`, `*ngFor`).
    Embutida,
    /// Num componente filho ou no conteúdo projetado nele.
    Filho,
}

fn onde_esta(
    nos: &[No],
    nome: &str,
    filhos: &std::collections::HashMap<String, Filho>,
    em_filho: bool,
    saida: &mut Vec<Lugar>,
) {
    for no in nos {
        let No::Elemento(e) = no else { continue };
        let lugar = if e.estrela.is_some() {
            Lugar::Embutida
        } else if em_filho || filhos.contains_key(&e.nome) || !dom::tag_html(&e.nome) {
            Lugar::Filho
        } else {
            Lugar::Raiz
        };
        if e.referencias.iter().any(|r| r.nome == nome) {
            saida.push(lugar);
        }
        let mut dentro = Vec::new();
        onde_esta(&e.filhos, nome, filhos, lugar == Lugar::Filho, &mut dentro);
        // Tudo abaixo de um `*` é da visão embutida.
        if lugar == Lugar::Embutida {
            dentro.iter_mut().for_each(|l| *l = Lugar::Embutida);
        }
        saida.extend(dentro);
    }
}

/// O tipo do campo é `Element` do `dart:html` ou um subtipo dele? Todo
/// `…Element` do `dart:html` desce de `Element`, menos `NoncedElement`
/// (conferido no `html_dart2js.dart` do SDK 3.6.2).
fn e_tipo_de_elemento(tipo: &str, local: &Local, resolvedor: Option<&dyn Resolucao>) -> bool {
    let tipo = tipo.trim().trim_end_matches('?');
    let simples = tipo.rsplit('.').next().unwrap_or(tipo);
    simples.ends_with("Element")
        && simples != "NoncedElement"
        && resolvedor
            .and_then(|r| r.uri_do_tipo(local.caminho, tipo))
            .as_deref()
            == Some("dart:html")
}

/// Algum pipe no template? `|` sozinho (o `||` é OU lógico) ou `$pipe`, em
/// qualquer expressão: interpolação, ligação, evento, `*`.
fn usa_pipe(nos: &[No]) -> bool {
    let tem = |t: &str| {
        let b = t.as_bytes();
        t.contains("$pipe")
            || (0..b.len()).any(|i| {
                b[i] == b'|' && b.get(i + 1) != Some(&b'|') && (i == 0 || b[i - 1] != b'|')
            })
    };
    nos.iter().any(|n| match n {
        No::Interpolacao { expr, .. } => tem(expr),
        No::Elemento(e) => {
            e.propriedades
                .iter()
                .chain(&e.eventos)
                .chain(&e.bananas)
                .chain(e.estrela.iter())
                .any(|l| tem(&l.valor))
                || e.atributos
                    .iter()
                    .any(|a| a.valor.contains("{{") && tem(&a.valor))
                || usa_pipe(&e.filhos)
        }
        _ => false,
    })
}

/// `livres` são os `#ref` aceitos ([`referencias_livres`]); `embutida` diz
/// se os nós estão dentro de um `*`, onde `#ref` ainda não é aceito.
fn motivos_dos_nos(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    livres: &std::collections::HashSet<String>,
    embutida: bool,
    fora: &mut std::collections::BTreeSet<Motivo>,
) {
    for no in nos {
        match no {
            No::Comentario(_) | No::Texto(_) => {}
            No::Interpolacao { .. } => {
                fora.insert(Motivo::Interpolacao);
            }
            No::Conteudo { .. } => {
                fora.insert(Motivo::Projecao);
            }
            No::Elemento(e) => {
                let e_filho = filhos.contains_key(&e.nome);
                if !dom::tag_html(&e.nome) && !e_filho {
                    fora.insert(Motivo::ComponenteNoTemplate);
                } else if e_filho {
                    // Componente conhecido: falta o que não for `@Input`
                    // declarado por ele.
                    let f = &filhos[&e.nome];
                    if !e.eventos.is_empty()
                        || !e.bananas.is_empty()
                        || !e.referencias.is_empty()
                        || e.estrela.is_some()
                        || !e.atributos.is_empty()
                        || e.propriedades
                            .iter()
                            .any(|l| !f.entradas.contains_key(&l.nome))
                    {
                        fora.insert(Motivo::LigacaoEmFilho);
                    }
                } else if !e.bananas.is_empty()
                    || e.referencias
                        .iter()
                        .any(|r| embutida || e.estrela.is_some() || !livres.contains(&r.nome))
                    || e.estrela.as_ref().is_some_and(|x| x.nome != "ngIf")
                {
                    fora.insert(Motivo::Ligacao);
                }
                if e.atributos.iter().any(|a| a.valor.contains("{{")) {
                    fora.insert(Motivo::Interpolacao);
                }
                if e.atributos.iter().any(|a| a.nome == "style") {
                    fora.insert(Motivo::EstiloEmLinha);
                }
                let dentro = embutida || e.estrela.is_some();
                motivos_dos_nos(&e.filhos, filhos, livres, dentro, fora);
            }
        }
    }
}

/// Um componente que este template pode usar, vindo do índice do pacote.
#[derive(Debug, Clone)]
pub struct Filho {
    pub classe: String,
    pub seletor: String,
    /// URI `package:` do `.dart` que declara a classe.
    pub uri_dart: String,
    /// URI `package:` do `.template.dart` dele.
    pub uri_template: String,
    /// Tem `<ng-content>`: muda `create` para `createAndProject`.
    pub projeta: bool,
    /// `@Input`s do filho: nome no template -> campo que recebe o valor.
    pub entradas: std::collections::HashMap<String, String>,
    /// O filho tem ciclo de vida, `AfterChanges` ou é `OnPush`: quem o usa
    /// chama os ganchos dele (`bindDirectiveDetectChangesLifecycleCallbacks`)
    /// e marca a checagem ao mudar uma entrada — nada disso ainda.
    pub muda_o_pai: bool,
}

/// O que o emissor precisa saber de onde o componente mora.
pub struct Local<'a> {
    /// Nome do pacote (`new_sali_frontend`).
    pub pacote: &'a str,
    /// Caminho do `.dart` dentro do pacote, com barras: `lib/src/x/foo.dart`.
    pub relativo: &'a str,
    /// Nome do arquivo para o `import` de si mesmo: `foo.dart`.
    pub arquivo: &'a str,
    /// Caminho no disco, para perguntar ao banco semântico em que escopo os
    /// nomes do construtor são resolvidos.
    pub caminho: &'a Path,
    /// Raiz do pacote, para mapear URIs `file:` do próprio projeto.
    pub raiz: &'a Path,
    /// URI `package:` do arquivo `.html` do template, quando há um. É para
    /// onde aponta o comentário `/* REF:url:inicio:fim */` que o oficial
    /// escreve em cada ligação.
    pub url_do_template: Option<String>,
}

impl Local<'_> {
    /// URI `package:` do `.css.shim.dart` de uma folha do `styleUrls`.
    pub(crate) fn uri_do_estilo(&self, url: &str) -> Option<String> {
        let dentro = self.relativo.strip_prefix("lib/")?;
        let dir = dentro.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let caminho = if dir.is_empty() {
            url.to_string()
        } else {
            format!("{dir}/{url}")
        };
        Some(format!("package:{}/{caminho}.shim.dart", self.pacote))
    }

    /// URI `asset:` deste arquivo — o espaço em que o emissor oficial calcula
    /// os caminhos de import.
    fn asset(&self) -> String {
        format!("asset:{}/{}", self.pacote, self.relativo)
    }
}

/// Uma ligação de propriedade já traduzida: a ação de uma constante (vai
/// para o `if (firstCheck)`) ou o bloco inteiro de uma dinâmica.
enum Ligada {
    Constante(String),
    Dinamica(String),
}

/// Corpo do `build()` de uma visão, montado enquanto se anda pelo template.
struct Corpo<'a> {
    linhas: Vec<String>,
    /// `addEventListener` de cada evento. O oficial cria todos os nós
    /// primeiro e liga depois (`_buildView` e só então `bindView`, em
    /// `view_compiler.dart`), então os ouvintes saem juntos, depois do último
    /// nó, em ordem de documento.
    ouvintes: Vec<String>,
    /// `#ref` aceitos neste template ([`referencias_livres`]); vazio na
    /// visão embutida.
    refs_livres: std::collections::HashSet<String>,
    /// Cada `#ref` visto, com a expressão do nó (`_el_3` ou `this._el_3`) —
    /// é o valor que o `@ViewChild` recebe.
    refs: std::collections::HashMap<String, String>,
    /// Campos `TextBinding`, que saem primeiro na classe.
    campos: Vec<String>,
    /// Campos `Object? _expr_k` das ligações, na ordem em que aparecem.
    campos_expr: Vec<String>,
    /// Campos `late final T _el_n` dos elementos com ligação.
    campos_el: Vec<String>,
    /// Próximo índice de ligação (`_expr_k`, `currVal_k`).
    proxima_ligacao: u32,
    /// Corpo do `detectChangesInternal`.
    deteccao: Vec<String>,
    /// Prefixo do `text_binding.dart`, alocado antes do resto quando o
    /// template tem interpolação (a ordem dos imports segue a ordem em que o
    /// oficial escreve o arquivo, e os campos vêm primeiro).
    tb: Option<String>,
    /// URI `package:` do arquivo do template, para o comentário `REF`.
    url_do_template: Option<String>,
    /// Tipos dos membros do componente, para escolher `interpolateString`.
    membros: &'a std::collections::HashMap<String, crate::componente::Membro>,
    /// Métodos da classe, válidos só como alvo de chamada.
    metodos: &'a std::collections::HashMap<String, String>,
    /// Componentes que este template pode usar, por seletor.
    filhos: &'a std::collections::HashMap<String, Filho>,
    /// Campos das visões-filhas (`_compView_n` e a instância), que saem na
    /// classe depois das ligações de texto.
    campos_filho: Vec<String>,
    /// `_compView_n` de cada filho, para a detecção e a destruição.
    vistas_filhas: Vec<String>,
    /// Asset deste arquivo, para calcular os caminhos de import dos filhos.
    asset: String,
    /// Banco semântico e o arquivo, para tipar cadeias como `item.nome`.
    tipos: Option<(&'a dyn Resolucao, &'a Path)>,
    /// O componente tem folha de estilo: cada elemento ganha `addShimC`.
    com_estilo: bool,
    /// Visões embutidas a emitir. Só a especificação: o texto é gerado
    /// depois do `angular.dart`, que é onde o oficial escreve essas classes —
    /// e é a ordem de escrita que numera os imports.
    embutidas: Vec<EspecEmbutida>,
    /// Nome da classe da visão (`ViewX`), para numerar as embutidas.
    classe_da_visao: String,
    /// `_appEl_n` de cada `ViewContainer`, para a detecção e a destruição.
    ancoras: Vec<String>,
    /// Esta visão é embutida: a raiz é registrada com `initRootNode`.
    embutida: bool,
    /// Locais do escopo (`let item of itens`), que sombreiam os membros.
    locais: std::collections::HashMap<String, crate::expr::Local>,
    /// Próximo número de visão embutida. O oficial usa um contador único em
    /// profundidade (`_view.viewIndex + _nestedViewCount` em
    /// `view_builder.dart`): o primeiro `*` do topo é 1, o aninhado dentro
    /// dele é 2, e o irmão seguinte é 3.
    proxima_embutida: u32,
    /// Tipo do contexto da visão (`import1.X`), para a embutida declarar
    /// `EmbeddedView<X>`.
    tipo_do_contexto: String,
    /// Para analisar as expressões do template, que são expressões Dart.
    nomes: &'a mut dartforge_intern::Interner,
    /// `bool firstCheck = this.firstCheck;` no `detectChangesInternal`, quando
    /// alguma ligação imutável é escrita só na primeira checagem.
    usa_primeira_checagem: bool,
    /// Entradas de diretivas e componentes filhos (`@Input`, `ngIf`,
    /// `ngForOf`) e o `ngDoCheck` delas: o `detectChangesInInputsMethod`,
    /// que o oficial escreve **antes** das visões aninhadas e das ligações
    /// de propriedade e texto (`writeChangeDetectionStatements`).
    entradas: Vec<String>,
    /// Próximo índice de nó. Vale para elementos e textos juntos, em ordem de
    /// documento; comentário não consome índice porque some antes.
    proximo: u32,
    /// `final doc = …` sai uma vez, no primeiro elemento.
    tem_doc: bool,
    /// `final _ctx = this.ctx;` no topo do `build()`, quando alguma expressão
    /// imutável é calculada ali.
    usa_ctx_no_build: bool,
    /// Ordinal do próximo `<ng-content>` — é o segundo argumento do
    /// `project`, e não consome índice de nó.
    proxima_projecao: u32,
    imp: &'a mut Importacoes,
    html: String,
}

impl Corpo<'_> {
    fn dom(&mut self) -> String {
        self.imp.alias(DOM_HELPERS)
    }

    /// `[x]="e"`: valor novo, `checkBinding` contra o anterior e a ação sobre
    /// o elemento. O nome da ligação e a URI do template vão na verificação
    /// para a mensagem de "expressão mudou depois da checagem".
    fn propriedade(&mut self, l: &crate::html::Ligacao, alvo: &str) -> Result<Ligada, Motivo> {
        let Some(url) = self.url_do_template.clone() else {
            return Err(Motivo::Ligacao);
        };
        let convertida = crate::expr::converter_com_locais(
            &l.valor,
            self.membros,
            self.metodos,
            &self.locais,
            self.nomes,
            self.tipos,
        )?;
        let expr = l.valor.trim();
        let (ini, fim) = (l.inicio, l.fim);
        // Toda ligação consome um índice (`createUniqueBindIndex` em
        // `_checkBinding`), mesmo a imutável, que não ganha campo.
        let k = self.proxima_ligacao;
        self.proxima_ligacao += 1;
        // Valor que não muda é escrito uma vez, na primeira checagem, sem
        // `checkBinding` e sem campo de valor anterior (`isImmutable`,
        // `_bindLiteral`). Valor que pode ser nulo ganharia um
        // `if (x != null)` em volta, e `null` não gera nada — os dois ainda
        // não.
        if convertida.imutavel {
            let tipo_certo = convertida
                .tipo
                .as_deref()
                .is_some_and(|t| !t.ends_with('?') && t != "dynamic" && t != "Object");
            if !tipo_certo {
                return Err(Motivo::Ligacao);
            }
            let acao = self.acao(l, alvo, &convertida.texto, &convertida)?;
            return Ok(Ligada::Constante(format!(
                "      {acao} /* REF:{url}:{ini}:{fim} */;"
            )));
        }
        self.campos_expr.push(format!("  Object? _expr_{k};"));
        let acao = self.acao(l, alvo, &format!("currVal_{k}"), &convertida)?;
        let chk = tardio(CHECK_BINDING);
        let valor = convertida.texto;
        Ok(Ligada::Dinamica(format!(
            "    final currVal_{k} = {valor};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, '{expr}', '{url}')) {{\n      {acao} /* REF:{url}:{ini}:{fim} */;\n      this._expr_{k} = currVal_{k};\n    }}"
        )))
    }

    /// As ligações de um elemento, como `bindAndWriteToRenderer` as escreve:
    /// as constantes primeiro, num `if (firstCheck)` — reaproveitando o do
    /// elemento anterior se ele for a última coisa escrita
    /// (`addStmtsIfFirstCheck`) —, depois as dinâmicas, na ordem.
    fn escrever_ligacoes(&mut self, ligadas: Vec<Ligada>) {
        let mut constantes = Vec::new();
        let mut dinamicas = Vec::new();
        for l in ligadas {
            match l {
                Ligada::Constante(s) => constantes.push(s),
                Ligada::Dinamica(s) => dinamicas.push(s),
            }
        }
        if !constantes.is_empty() {
            self.usa_primeira_checagem = true;
            const ABRE: &str = "    if (firstCheck) {\n";
            const FECHA: &str = "\n    }";
            match self.deteccao.last_mut() {
                Some(ultimo) if ultimo.starts_with(ABRE) && ultimo.ends_with(FECHA) => {
                    ultimo.truncate(ultimo.len() - FECHA.len());
                    ultimo.push('\n');
                    ultimo.push_str(&constantes.join("\n"));
                    ultimo.push_str(FECHA);
                }
                _ => self
                    .deteccao
                    .push(format!("{ABRE}{}{FECHA}", constantes.join("\n"))),
            }
        }
        self.deteccao.extend(dinamicas);
    }

    /// O que a ligação faz com o elemento, pelo prefixo do nome.
    ///
    /// Regras de `_UpdateStatementsVisitor` (`update_statement_visitor.dart`)
    /// e de `createElementPropertyAst` (`template_parser.dart`). O que muda a
    /// forma e ainda não tem caso no corpus é recusado: `[attr.x.if]`,
    /// namespace, `[style.x.unidade]`, estilo de valor que não é `String`
    /// (sairia `.toString()`), propriedade renomeada pelo esquema
    /// (`readonly` → `readOnly`, `tabindex`) e propriedade ou atributo com
    /// contexto de segurança (`href`, `src`, `innerHtml`…), que ganham o
    /// `sanitize*` em volta.
    fn acao(
        &mut self,
        l: &crate::html::Ligacao,
        alvo: &str,
        valor: &str,
        c: &crate::expr::Convertida,
    ) -> Result<String, Motivo> {
        // A ação vai para o `detectChangesInternal`: o import é alocado
        // quando a detecção é escrita, não agora.
        let dom = tardio(DOM_HELPERS);
        Ok(if l.nome == "class" {
            format!("this.updateChildClass({alvo}, {valor})")
        } else if let Some(classe) = l.nome.strip_prefix("class.") {
            if classe.contains('.') {
                return Err(Motivo::Ligacao);
            }
            format!("{dom}.updateClassBinding({alvo}, '{classe}', {valor})")
        } else if let Some(attr) = l.nome.strip_prefix("attr.") {
            if attr.contains('.') || attr.contains(':') || com_seguranca(attr) {
                return Err(Motivo::Ligacao);
            }
            // `canBeNull` (`analyzed_class.dart`): só o literal não pode ser
            // nulo, e só ele vai por `setAttribute`. `a ?? b` tem regra
            // própria; ainda não.
            if l.valor.contains("??") {
                return Err(Motivo::Ligacao);
            }
            let literal =
                c.texto.starts_with(['\'', '"']) || matches!(c.texto.as_str(), "true" | "false");
            let f = if literal {
                "setAttribute"
            } else {
                "updateAttribute"
            };
            format!("{dom}.{f}({alvo}, '{attr}', {valor})")
        } else if let Some(estilo) = l.nome.strip_prefix("style.") {
            if estilo.contains('.') || c.tipo.as_deref() != Some("String") {
                return Err(Motivo::Ligacao);
            }
            format!("{alvo}.style.setProperty('{estilo}', {valor})")
        } else if l.nome.contains('.') {
            return Err(Motivo::Ligacao);
        } else {
            let prop = &l.nome;
            if com_seguranca(prop)
                || matches!(
                    prop.as_str(),
                    "readonly" | "tabindex" | "tabIndex" | "innerHtml" | "style"
                )
            {
                return Err(Motivo::Ligacao);
            }
            format!("{dom}.setProperty({alvo}, '{prop}', {valor})")
        })
    }

    /// `(e)="metodo()"` ou `(e)="metodo($event)"`: o oficial passa o método
    /// por referência a `eventHandlerN`, onde N é quantos argumentos o
    /// template escreveu.
    fn evento(&mut self, l: &crate::html::Ligacao, alvo: &str) -> Result<(), Motivo> {
        // Evento fora da lista do DOM (`keyup.enter`, `document:click`,
        // evento próprio) vai pelo `eventManager` do ngdart, outra forma.
        if !evento_nativo(&l.nome) {
            return Err(Motivo::Ligacao);
        }
        let texto = l.valor.trim();
        let Some((nome, resto)) = texto.split_once('(') else {
            return Err(Motivo::Ligacao);
        };
        let Some(args) = resto.strip_suffix(')') else {
            return Err(Motivo::Ligacao);
        };
        let aridade = match args.trim() {
            "" => 0,
            "$event" => 1,
            _ => return Err(Motivo::Ligacao),
        };
        // O alvo é um método do componente, passado por referência
        // (`_tearOffSimpleHandler`): `_ctx.metodo`. Pelo conversor de
        // expressões não dá — método solto não é valor lá, e o evento nunca
        // saía. Local de visão com o mesmo nome sombrearia o método.
        let nome = nome.trim();
        if !self.metodos.contains_key(nome) || self.locais.contains_key(nome) {
            return Err(Motivo::Ligacao);
        }
        // Na visão embutida o `build()` não declara `_ctx`; o que o oficial
        // escreve ali ainda não tem caso no corpus.
        if self.embutida {
            return Err(Motivo::Ligacao);
        }
        let metodo = format!("_ctx.{nome}");
        self.usa_ctx_no_build = true;
        let evento = &l.nome;
        self.ouvintes.push(format!(
            "    {alvo}.addEventListener('{evento}', this.eventHandler{aridade}({metodo}));"
        ));
        Ok(())
    }

    /// Um componente dentro do template: a visão-filha é um campo, a
    /// instância é outro, e o `build()` cria as duas e as liga.
    ///
    /// O sufixo `_5` do campo da instância é o `uniqueId` do provedor no nó
    /// (`_instances.length` em `provider_resolver.dart`): num nó que só tem o
    /// componente, os cinco provedores embutidos do elemento já ocupam 0..4.
    fn componente_filho(
        &mut self,
        e: &crate::html::Elemento,
        filho: &Filho,
        pai: &str,
    ) -> Result<(), Motivo> {
        if !e.eventos.is_empty()
            || !e.bananas.is_empty()
            || !e.referencias.is_empty()
            || e.estrela.is_some()
            || !e.atributos.is_empty()
        {
            // `@Output`, `[(x)]`, `#ref` e atributo estático em filho: ainda
            // não.
            return Err(Motivo::LigacaoEmFilho);
        }
        if filho.muda_o_pai {
            return Err(Motivo::LigacaoEmFilho);
        }
        let n = self.proximo;
        self.proximo += 1;
        let cam_template = caminho_do_import(
            &self.asset,
            &asset_de_uri(&filho.uri_template, "", Path::new(""))
                .ok_or(Motivo::ComponenteNoTemplate)?,
        )
        .ok_or(Motivo::ComponenteNoTemplate)?;
        let cam_dart = caminho_do_import(
            &self.asset,
            &asset_de_uri(&filho.uri_dart, "", Path::new(""))
                .ok_or(Motivo::ComponenteNoTemplate)?,
        )
        .ok_or(Motivo::ComponenteNoTemplate)?;
        let vt = self.imp.alias(&cam_template);
        let vd = self.imp.alias(&cam_dart);
        let classe = &filho.classe;
        let campo_vista = format!("_compView_{n}");
        let campo_inst = format!("_{classe}_{n}_5");
        self.campos_filho
            .push(format!("  late final {vt}.View{classe}0 {campo_vista};"));
        self.campos_filho
            .push(format!("  late final {vd}.{classe} {campo_inst};"));
        self.vistas_filhas.push(campo_vista.clone());
        self.linhas.push(format!(
            "    this.{campo_vista} = {vt}.View{classe}0(this, {n});"
        ));
        self.linhas.push(format!(
            "    final _el_{n} = this.{campo_vista}.rootElement;"
        ));
        self.linhas.push(format!("    {pai}.append(_el_{n});"));
        self.linhas
            .push(format!("    this.{campo_inst} = {vd}.{classe}();"));
        for l in &e.propriedades {
            self.entrada_do_filho(l, filho, &campo_inst)?;
        }
        if filho.projeta {
            // Conteúdo projetado: os nós são criados soltos e entregues ao
            // filho, que decide onde encaixá-los.
            let marca = self.linhas.len();
            self.nos(&e.filhos, "")?;
            let criados: Vec<String> = self.linhas[marca..]
                .iter()
                .filter_map(|l| l.split_once("final ").map(|(_, r)| r))
                .filter_map(|r| r.split_once(' ').map(|(nome, _)| nome.to_string()))
                .collect();
            let raiz: Vec<String> = criados
                .iter()
                .filter(|n| n.starts_with("_el_"))
                .cloned()
                .collect();
            // Sem conteúdo projetado a lista sai constante e numa linha
            // só, como o oficial escreve.
            self.linhas.push(if raiz.is_empty() {
                format!(
                    "    this.{campo_vista}.createAndProject(this.{campo_inst}, [const <Object>[]]);"
                )
            } else {
                format!(
                    "    this.{campo_vista}.createAndProject(this.{campo_inst}, [\n      <Object>[{}]\n    ]);",
                    raiz.join(", ")
                )
            });
        } else {
            if !e.filhos.is_empty() {
                return Err(Motivo::Projecao);
            }
            self.linhas
                .push(format!("    this.{campo_vista}.create(this.{campo_inst});"));
        }
        Ok(())
    }

    /// `[titulo]="valor"` num componente filho: o valor entra no campo que o
    /// `@Input` aponta, e o devtools registra a entrada em modo de
    /// desenvolvimento.
    fn entrada_do_filho(
        &mut self,
        l: &crate::html::Ligacao,
        filho: &Filho,
        campo_inst: &str,
    ) -> Result<(), Motivo> {
        let Some(url) = self.url_do_template.clone() else {
            return Err(Motivo::LigacaoEmFilho);
        };
        let Some(campo) = filho.entradas.get(&l.nome).cloned() else {
            // Nome que o filho não declara como `@Input`: pode ser diretiva.
            return Err(Motivo::LigacaoEmFilho);
        };
        let convertida = crate::expr::converter_com_locais(
            &l.valor,
            self.membros,
            self.metodos,
            &self.locais,
            self.nomes,
            self.tipos,
        )
        .map_err(|_| Motivo::LigacaoEmFilho)?;
        // Entrada imutável vai para o `if (firstCheck)` (`_bindLiteral`),
        // outra forma; ainda não.
        if convertida.imutavel {
            return Err(Motivo::LigacaoEmFilho);
        }
        let k = self.proxima_ligacao;
        self.proxima_ligacao += 1;
        self.campos_expr.push(format!("  Object? _expr_{k};"));
        let chk = tardio(CHECK_BINDING);
        let dev = tardio(DEVTOOLS);
        let expr = l.valor.trim();
        let (ini, fim) = (l.inicio, l.fim);
        let valor = convertida.texto;
        let nome = &l.nome;
        self.entradas.push(format!(
            "    final currVal_{k} = {valor};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, '{expr}', '{url}')) {{\n      if ({dev}.isDevToolsEnabled) {{\n        {dev}.Inspector.instance.recordInput(this.{campo_inst}, '{nome}', currVal_{k});\n      }}\n      this.{campo_inst}.{campo} = currVal_{k} /* REF:{url}:{ini}:{fim} */;\n      this._expr_{k} = currVal_{k};\n    }}"
        ));
        Ok(())
    }

    /// `*dir="..."`: âncora, `ViewContainer`, `TemplateRef` e a diretiva
    /// estrutural, com o conteúdo numa visão embutida à parte.
    ///
    /// Só as duas diretivas do próprio ngdart. Generalizar exige o índice de
    /// diretivas (seletor de atributo, construtor, entradas, ciclo de vida) —
    /// e a numeração dos provedores muda com o que a diretiva injeta, então
    /// não dá para chutar.
    fn estrutural(
        &mut self,
        e: &crate::html::Elemento,
        estrela: &crate::html::Ligacao,
        pai: &str,
    ) -> Result<(), Motivo> {
        let Some(url) = self.url_do_template.clone() else {
            return Err(Motivo::Ligacao);
        };
        let Some(dir) = Estrutural::conhecida(&estrela.nome) else {
            return Err(Motivo::Ligacao);
        };
        let micro = crate::micro::analisar(&estrela.nome, &estrela.valor);
        let n = self.proximo;
        self.proximo += 1;
        let dom = self.dom();
        let vc = self.imp.q(VIEW_CONTAINER);
        let tr = self.imp.q(TEMPLATE_REF);
        let qd = self.imp.q(dir.uri);
        let dev = self.imp.alias(DEVTOOLS);
        let classe_dir = dir.classe;
        // O número desta visão, e o salto que o subconjunto dela consome.
        let indice = self.proxima_embutida;
        self.proxima_embutida += 1 + contar_estruturais(&e.filhos);
        let nome_fabrica = format!("viewFactory_{}{indice}", &self.classe_da_visao[4..]);
        let campo = format!("_{classe_dir}_{n}_9");
        // Num `<template>` os provedores embutidos ocupam 0..7 e o
        // `TemplateRef` é o 8 — ver docs/GERADOR-NG.md §2.
        self.campos_filho
            .push(format!("  late final {vc}ViewContainer _appEl_{n};"));
        self.campos_filho
            .push(format!("  late final {qd}{classe_dir} {campo};"));
        self.ancoras.push(format!("_appEl_{n}"));
        self.linhas.push(format!(
            "    final _anchor_{n} = {dom}.appendAnchor({pai});"
        ));
        // O segundo argumento é o índice do elemento pai, e `null` quando a
        // âncora está na raiz da visão (`isRootElement ? null :
        // parent.nodeIndex`, em `compile_element.dart`).
        let pai_indice = indice_do_elemento(pai);
        self.linhas.push(format!(
            "    this._appEl_{n} = {vc}ViewContainer({n}, {pai_indice}, this, _anchor_{n});"
        ));
        self.linhas.push(format!(
            "    var _TemplateRef_{n}_8 = {tr}TemplateRef(this._appEl_{n}, {nome_fabrica});"
        ));
        self.linhas.push(format!(
            "    this.{campo} = {qd}{classe_dir}(this._appEl_{n}, _TemplateRef_{n}_8);"
        ));
        self.linhas.push(format!(
            "    if ({dev}.isDevToolsEnabled) {{\n      {dev}.Inspector.instance.registerDirective(_anchor_{n}, this.{campo});\n    }}"
        ));

        // As entradas da diretiva, na ordem em que a microssintaxe as declara.
        let mut tipo_da_colecao = None;
        for (prop, expr) in &micro.propriedades {
            let c = crate::expr::converter_com_locais(
                expr,
                self.membros,
                self.metodos,
                &self.locais,
                self.nomes,
                self.tipos,
            )?;
            // Entrada imutável (`final List<X> itens`) vai para o
            // `if (firstCheck)`, outra forma; ainda não.
            if c.imutavel {
                return Err(Motivo::Ligacao);
            }
            if prop.ends_with("Of") {
                tipo_da_colecao = c.tipo.clone();
            }
            let valor = c.texto;
            let (ini, fim) = (estrela.inicio, estrela.fim);
            if dir.direta {
                // `_isDirectBinding` do ngcompiler: o `NgIf` já compara o
                // valor antes de agir, então não há `checkBinding` fora.
                self.entradas.push(format!(
                    "    if ({dev}.isDevToolsEnabled) {{\n      {dev}.Inspector.instance.recordInput(this.{campo}, '{prop}', {valor});\n    }}\n    this.{campo}.{prop} = {valor} /* REF:{url}:{ini}:{fim} */;"
                ));
            } else {
                let k = self.proxima_ligacao;
                self.proxima_ligacao += 1;
                self.campos_expr.push(format!("  Object? _expr_{k};"));
                let chk = tardio(CHECK_BINDING);
                // O nome que vai na verificação é a expressão desta
                // propriedade (`itens`), não o valor inteiro do `*`.
                let texto = expr.trim();
                self.entradas.push(format!(
                    "    final currVal_{k} = {valor};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, '{texto}', '{url}')) {{\n      if ({dev}.isDevToolsEnabled) {{\n        {dev}.Inspector.instance.recordInput(this.{campo}, '{prop}', currVal_{k});\n      }}\n      this.{campo}.{prop} = currVal_{k} /* REF:{url}:{ini}:{fim} */;\n      this._expr_{k} = currVal_{k};\n    }}"
                ));
            }
        }
        if dir.do_check {
            let chk = tardio(CHECK_BINDING);
            self.entradas.push(format!(
                "    if ((!{chk}.debugThrowIfChanged)) {{\n      this.{campo}.ngDoCheck();\n    }}"
            ));
        }

        // Os locais do laço, tipados pelo elemento da coleção.
        let mut locais = self.locais.clone();
        for (nome, chave) in &micro.locais {
            let tipo = match chave.as_str() {
                "$implicit" => {
                    let Some(t) = tipo_da_colecao.as_deref().and_then(tipo_do_elemento) else {
                        return Err(Motivo::Ligacao);
                    };
                    t
                }
                "index" | "count" => "int".to_string(),
                "first" | "last" | "even" | "odd" => "bool".to_string(),
                _ => return Err(Motivo::Ligacao),
            };
            locais.insert(
                nome.clone(),
                crate::expr::Local {
                    dart: format!("local_{nome}"),
                    tipo,
                },
            );
        }

        let mut sem_estrela = e.clone();
        sem_estrela.estrela = None;
        self.embutidas.push(EspecEmbutida {
            indice,
            classe: format!("_{}{indice}", self.classe_da_visao),
            fabrica: nome_fabrica,
            nos: vec![No::Elemento(sem_estrela)],
            locais,
            micro,
        });
        Ok(())
    }

    /// Emite a ligação de texto de `{{ … }}`: o campo `TextBinding`, o
    /// `append` no `build()` e a atualização no `detectChangesInternal`.
    fn interpolacao(
        &mut self,
        expr: &str,
        inicio: usize,
        fim: usize,
        pai: &str,
    ) -> Result<(), Motivo> {
        let (Some(tb), Some(url)) = (self.tb.clone(), self.url_do_template.clone()) else {
            return Err(Motivo::Interpolacao);
        };
        let convertida = crate::expr::converter_com_locais(
            expr,
            self.membros,
            self.metodos,
            &self.locais,
            self.nomes,
            self.tipos,
        )
        .map_err(|_| Motivo::Interpolacao)?;
        // Sem o tipo estático não dá para escolher entre `interpolateString`,
        // `interpolate` e `updateTextWithPrimitive` — e escolher errado muda o
        // que o programa faz.
        let Some(tipo) = convertida.tipo.clone() else {
            return Err(Motivo::Interpolacao);
        };
        let membro = crate::componente::Membro {
            tipo,
            imutavel: convertida.imutavel,
        };
        let membro = &membro;
        let acesso = convertida.texto;
        let n = self.proximo;
        self.proximo += 1;
        let nu = membro.tipo.trim_end_matches('?').to_string();
        // `expressionsAreString` decide entre `interpolateString` e
        // `interpolate`; `_isPrimitiveCheck` tira o primitivo mutável do
        // caminho da interpolação.
        let interpolar = |imp: &mut Importacoes| {
            let alias = imp.alias(INTERPOLATE);
            let f = if nu == "String" {
                "interpolateString0"
            } else {
                "interpolate0"
            };
            format!("{alias}.{f}({acesso})")
        };
        if membro.imutavel {
            // Valor que não muda não tem ligação: o texto é calculado uma vez,
            // no `build()`, como o oficial faz (`isImmutable`).
            self.usa_ctx_no_build = true;
            let valor = interpolar(self.imp);
            let dom = self.dom();
            self.linhas.push(format!(
                "    final _text_{n} = {dom}.appendText({pai}, {valor});"
            ));
            return Ok(());
        }
        self.campos.push(format!(
            "  final {tb}.TextBinding _textBinding_{n} = {tb}.TextBinding();"
        ));
        self.linhas
            .push(format!("    {pai}.append(this._textBinding_{n}.element);"));
        let atualizacao = if primitivo(&nu) {
            format!("updateTextWithPrimitive({acesso})")
        } else {
            // Na detecção o import é do momento em que ela é escrita.
            let f = if nu == "String" {
                "interpolateString0"
            } else {
                "interpolate0"
            };
            format!("updateText({}.{f}({acesso}))", tardio(INTERPOLATE))
        };
        self.deteccao.push(format!(
            "    this._textBinding_{n}.{atualizacao} /* REF:{url}:{inicio}:{fim} */;"
        ));
        Ok(())
    }

    /// Emite os nós filhos de `pai`. Devolve `None` na primeira forma que o
    /// emissor ainda não cobre — o arquivo inteiro volta para o build_runner.
    fn nos(&mut self, nos: &[No], pai: &str) -> Result<(), Motivo> {
        for no in nos {
            match no {
                No::Comentario(_) => {}
                No::Texto(t) => {
                    if pai.is_empty() {
                        return Err(Motivo::Projecao); // texto projetado solto
                    }
                    let n = self.proximo;
                    self.proximo += 1;
                    let dom = self.dom();
                    let texto = literal(t);
                    self.linhas.push(format!(
                        "    final _text_{n} = {dom}.appendText({pai}, {texto});"
                    ));
                }
                No::Elemento(e) => {
                    if let Some(filho) = self.filhos.get(&e.nome).cloned() {
                        self.componente_filho(e, &filho, pai)?;
                        continue;
                    }
                    if !dom::tag_html(&e.nome) {
                        return Err(Motivo::ComponenteNoTemplate);
                    }
                    // Nome de diretiva do ecossistema (`ngClass`, `ngModel`…)
                    // numa ligação é coisa de diretiva, não propriedade do
                    // DOM: emitir `setProperty` ali faria outra coisa.
                    if e.propriedades
                        .iter()
                        .chain(e.eventos.iter())
                        .any(|l| e_de_diretiva(&l.nome))
                        || e.atributos.iter().any(|a| e_de_diretiva(&a.nome))
                    {
                        return Err(Motivo::Diretiva);
                    }
                    if let Some(estrela) = &e.estrela {
                        self.estrutural(e, estrela, pai)?;
                        continue;
                    }
                    if !e.bananas.is_empty() {
                        // `[(x)]` ainda não.
                        return Err(Motivo::Ligacao);
                    }
                    // `#ref` só na forma que não muda nada no nó; o valor
                    // dele é registrado adiante, para o `@ViewChild`.
                    if e.referencias
                        .iter()
                        .any(|r| !r.valor.is_empty() || !self.refs_livres.contains(&r.nome))
                    {
                        return Err(Motivo::Ligacao);
                    }
                    if e.atributos.iter().any(|a| a.valor.contains("{{")) {
                        return Err(Motivo::Interpolacao);
                    }
                    let n = self.proximo;
                    self.proximo += 1;
                    if !self.tem_doc {
                        self.tem_doc = true;
                        let html = self.html.clone();
                        self.linhas
                            .push(format!("    final doc = {html}.document;"));
                    }
                    let dom = self.dom();
                    let tag = e.nome.to_ascii_lowercase();
                    // Nó projetado não tem pai: o oficial cria solto, com
                    // `document.createElement`, e entrega ao filho
                    // (`_createElementAndAppend`, com `parent == null`).
                    let criacao = if pai.is_empty() {
                        let util = self.imp.alias(UTILITIES);
                        format!("{util}.unsafeCast(doc.createElement('{tag}'))")
                    } else {
                        match tag.as_str() {
                            "div" => format!("{dom}.appendDiv(doc, {pai})"),
                            "span" => format!("{dom}.appendSpan(doc, {pai})"),
                            _ => {
                                let tipo = dom::tipo_da_tag(&tag);
                                let html = &self.html;
                                format!("{dom}.appendElement<{html}.{tipo}>(doc, {pai}, '{tag}')")
                            }
                        }
                    };
                    // Elemento com ligação de propriedade vira campo da
                    // visão: o `detectChangesInternal` precisa dele depois do
                    // `build()`. Evento sozinho não exige campo.
                    let tipo = dom::tipo_da_tag(&tag);
                    let alvo = if e.propriedades.is_empty() {
                        self.linhas.push(format!("    final _el_{n} = {criacao};"));
                        format!("_el_{n}")
                    } else {
                        let html = self.html.clone();
                        self.campos_el
                            .push(format!("  late final {html}.{tipo} _el_{n};"));
                        self.linhas.push(format!("    this._el_{n} = {criacao};"));
                        format!("this._el_{n}")
                    };
                    // `renderNode.toReadExpr()`: o local ou o campo, como o
                    // nó tiver sido declarado.
                    for r in &e.referencias {
                        self.refs.insert(r.nome.clone(), alvo.clone());
                    }
                    // Atributos saem em ordem alfabética (`_toSortedBindings`).
                    let mut atributos = e.atributos.clone();
                    atributos.sort_by(|a, b| a.nome.cmp(&b.nome));
                    for a in &atributos {
                        let valor = literal(&a.valor);
                        if a.nome == "class" {
                            self.linhas
                                .push(format!("    this.updateChildClass({alvo}, {valor});"));
                        } else if a.nome == "style" {
                            return Err(Motivo::EstiloEmLinha);
                        } else {
                            let dom = self.dom();
                            let nome = &a.nome;
                            self.linhas.push(format!(
                                "    {dom}.setAttribute({alvo}, '{nome}', {valor});"
                            ));
                        }
                    }
                    // As ligações são numeradas em ordem de documento — a do
                    // pai antes das dos filhos —, então são registradas antes
                    // de descer. Os eventos também: o ouvinte do pai vem
                    // antes do dos filhos, e todos saem juntos no fim do
                    // `build()` (ver `ouvintes`).
                    let mut ligadas = Vec::new();
                    for l in &e.propriedades {
                        ligadas.push(self.propriedade(l, &alvo)?);
                    }
                    self.escrever_ligacoes(ligadas);
                    // Dois `(click)` no mesmo elemento viram um método só
                    // (`mergeEvents`); ainda não.
                    // Evento em nó projetado num filho ainda não tem caso no
                    // corpus.
                    if pai.is_empty() && !e.eventos.is_empty() {
                        return Err(Motivo::Ligacao);
                    }
                    let mut vistos = std::collections::HashSet::new();
                    for l in &e.eventos {
                        if !vistos.insert(l.nome.as_str()) {
                            return Err(Motivo::Ligacao);
                        }
                        self.evento(l, &alvo)?;
                    }
                    if self.com_estilo {
                        // Isolamento de estilo por atributo: o elemento entra
                        // no escopo do componente.
                        self.linhas.push(format!("    this.addShimC({alvo});"));
                    }
                    self.nos(&e.filhos, &alvo)?;
                }
                No::Interpolacao { expr, inicio, fim } => {
                    self.interpolacao(expr, *inicio, *fim, pai)?
                }
                No::Conteudo { seletor } => {
                    // `<ng-content>` não consome índice de nó; o número é o
                    // ordinal da projeção no template.
                    if seletor.is_some() {
                        return Err(Motivo::Projecao); // seletor ainda não
                    }
                    let i = self.proxima_projecao;
                    self.proxima_projecao += 1;
                    self.linhas.push(format!("    this.project({pai}, {i});"));
                }
            }
        }
        Ok(())
    }
}

/// `detectChangesInternal` e `destroyInternal` da visão-hospedeira, na forma
/// exata do oficial: os ganchos de conteúdo antes de detectar a visão, os de
/// visão depois, todos sob `!debugThrowIfChanged`, e `firstCheck` declarado só
/// quando alguém o usa.
fn ciclo_de_vida(g: &crate::componente::Ganchos, dbg: &str) -> String {
    let mut s = String::new();
    if g.tem_deteccao() {
        s.push_str(
            "
  @override
  void detectChangesInternal() {
",
        );
        if g.usa_primeira_checagem() {
            s.push_str(
                "    bool firstCheck = this.firstCheck;
",
            );
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
    if g.on_destroy {
        s.push_str(
            "
  @override
  void destroyInternal() {
    this.component.ngOnDestroy();
  }
",
        );
    }
    s
}

/// Tipos que o ngcompiler trata como primitivos na interpolação
/// (`isBool`, `isNumber`, `isDouble`, `isInt`).
fn primitivo(tipo: &str) -> bool {
    matches!(tipo, "bool" | "num" | "double" | "int")
}

/// Algum elemento tem ligação de propriedade e, portanto, vira campo?
fn tem_elemento_ligado(nos: &[No], filhos: &std::collections::HashMap<String, Filho>) -> bool {
    nos.iter().any(|n| match n {
        // Componente filho não vira campo de elemento: quem guarda a raiz
        // dele é a visão-filha.
        // Subárvore de `*` é da visão embutida.
        No::Elemento(e) if e.estrela.is_some() => false,
        No::Elemento(e) if !filhos.contains_key(&e.nome) => {
            !e.propriedades.is_empty() || tem_elemento_ligado(&e.filhos, filhos)
        }
        No::Elemento(e) => tem_elemento_ligado(&e.filhos, filhos),
        _ => false,
    })
}

/// O que gera campo na classe da visão, na ordem em que aparece.
enum CampoDaVisao<'a> {
    Filho(&'a Filho),
    /// Diretiva estrutural, com a URI da classe dela.
    Estrutural(&'static str),
}

/// Os campos que o template vai gerar, em ordem de documento. A ordem dos
/// imports segue esta lista, e é ela que faz a numeração bater com a do
/// oficial.
fn campos_em_ordem<'a>(
    nos: &[No],
    filhos: &'a std::collections::HashMap<String, Filho>,
) -> Vec<CampoDaVisao<'a>> {
    let mut saida = Vec::new();
    for no in nos {
        let No::Elemento(e) = no else { continue };
        if let Some(estrela) = &e.estrela {
            // O conteúdo vai para a visão embutida; os campos dele são de lá.
            if let Some(d) = Estrutural::conhecida(&estrela.nome) {
                saida.push(CampoDaVisao::Estrutural(d.uri));
            }
            continue;
        }
        if let Some(f) = filhos.get(&e.nome) {
            saida.push(CampoDaVisao::Filho(f));
        }
        saida.extend(campos_em_ordem(&e.filhos, filhos));
    }
    saida
}

/// O que é preciso para emitir uma visão embutida, guardado durante a
/// varredura e usado depois.
struct EspecEmbutida {
    /// Número da visão (`_ViewX3`), atribuído na varredura.
    indice: u32,
    classe: String,
    fabrica: String,
    nos: Vec<No>,
    locais: std::collections::HashMap<String, crate::expr::Local>,
    micro: crate::micro::Micro,
}

/// Emite a classe de uma visão embutida, a sua fábrica e — recursivamente —
/// as visões embutidas dentro dela.
///
/// Roda **depois** do `angular.dart`, porque é aí que o oficial escreve
/// essas classes; os imports que ela aloca (`embedded_view`, o
/// `text_binding` dos campos, `render_view` do construtor, `dart:core` do
/// argumento de tipo, `interpolate`) saem nessa ordem.
#[allow(clippy::too_many_arguments)]
fn emitir_embutida(
    espec: EspecEmbutida,
    imp: &mut Importacoes,
    membros: &std::collections::HashMap<String, crate::componente::Membro>,
    metodos: &std::collections::HashMap<String, String>,
    filhos: &std::collections::HashMap<String, Filho>,
    asset: &str,
    tipos: Option<(&dyn Resolucao, &Path)>,
    com_estilo: bool,
    url_do_template: Option<String>,
    classe_da_visao: &str,
    tipo_do_contexto: &str,
    html: &str,
    nomes: &mut dartforge_intern::Interner,
) -> Result<String, Motivo> {
    let ev = imp.alias(EMBEDDED_VIEW);
    let classe = espec.classe.clone();
    let fabrica = espec.fabrica.clone();
    let mut dentro = Corpo {
        linhas: Vec::new(),
        ouvintes: Vec::new(),
        refs_livres: Default::default(),
        refs: Default::default(),
        campos: Vec::new(),
        campos_filho: Vec::new(),
        vistas_filhas: Vec::new(),
        campos_expr: Vec::new(),
        campos_el: Vec::new(),
        proxima_ligacao: 0,
        deteccao: Vec::new(),
        nomes,
        usa_primeira_checagem: false,
        entradas: Vec::new(),
        tb: None,
        url_do_template: url_do_template.clone(),
        membros,
        metodos,
        filhos,
        asset: asset.to_string(),
        tipos,
        com_estilo,
        embutidas: Vec::new(),
        classe_da_visao: classe_da_visao.to_string(),
        ancoras: Vec::new(),
        embutida: true,
        locais: espec.locais.clone(),
        // A numeração continua de onde o pai parou.
        proxima_embutida: espec.indice + 1,
        tipo_do_contexto: tipo_do_contexto.to_string(),
        proximo: 0,
        tem_doc: false,
        usa_ctx_no_build: false,
        proxima_projecao: 0,
        imp,
        html: html.to_string(),
    };
    // O campo de ligação de texto é declarado antes do construtor, então o
    // import dele entra aqui.
    if tem_interpolacao(&espec.nos) {
        dentro.tb = Some(dentro.imp.alias(TEXT_BINDING));
    }
    // Local declarado numa visão *ancestral* é lido pela cadeia de
    // `parentView`, com cast para a classe daquela visão:
    // `unsafeCast<_ViewX2>((this.parentView!)).locals['$implicit']`.
    // É mecanismo próprio e ainda não está verificado por caso de corpus.
    for nome in espec.locais.keys() {
        let meu = espec.micro.locais.iter().any(|(n, _)| n == nome);
        if !meu && local_citado(&espec.nos, nome) {
            return Err(Motivo::Ligacao);
        }
    }
    alocar_imports_dos_campos(dentro.imp, &espec.nos, filhos, asset)?;
    // O construtor vem depois dos campos e antes do `build()`, e é ele que
    // nomeia a `RenderView`.
    let rv = dentro.imp.alias(RENDER_VIEW);
    // As declarações de local abrem o `detectChangesInternal`, antes de
    // qualquer ligação; o argumento de tipo do `unsafeCast` é que traz o
    // `dart:core`.
    let util = dentro.imp.alias(UTILITIES);
    let usados: Vec<&(String, String)> = espec
        .micro
        .locais
        .iter()
        .filter(|(nome, _)| local_citado(&espec.nos, nome))
        .collect();
    // O argumento de tipo do `unsafeCast` precisa estar importado e
    // qualificado: do `dart:core` sai sem prefixo, de outra biblioteca sai
    // com o prefixo dela.
    let mut tipos_locais: std::collections::HashMap<String, String> = Default::default();
    for (nome, _) in &usados {
        let Some(l) = espec.locais.get(nome.as_str()) else {
            continue;
        };
        if matches!(
            l.tipo.as_str(),
            "String" | "int" | "double" | "bool" | "num" | "Object"
        ) {
            dentro.imp.alias("dart:core");
            tipos_locais.insert(nome.to_string(), l.tipo.clone());
            continue;
        }
        let Some((r, arquivo)) = tipos else {
            return Err(Motivo::Ligacao);
        };
        let Some(uri) = r.uri_do_tipo(arquivo, &l.tipo) else {
            return Err(Motivo::Ligacao);
        };
        let alvo = asset_de_uri(&uri, "", Path::new("")).ok_or(Motivo::Ligacao)?;
        let caminho = caminho_do_import(asset, &alvo).ok_or(Motivo::Ligacao)?;
        let q = dentro.imp.q(&caminho);
        tipos_locais.insert(nome.to_string(), format!("{q}{}", l.tipo));
    }
    dentro.nos(&espec.nos, "")?;
    if dentro.proximo == 0 {
        return Err(Motivo::Ligacao);
    }
    // O local só é declarado se for usado, como no oficial.
    let mut declaracoes = Vec::new();
    for (nome, chave) in &usados {
        let Some(l) = espec.locais.get(nome.as_str()) else {
            continue;
        };
        let d = &l.dart;
        let t = tipos_locais.get(nome.as_str()).unwrap_or(&l.tipo);
        // A chave sai como literal escapado (`'\$implicit'`): sem o escape,
        // o `$` viraria interpolação em Dart.
        let chave = literal(chave);
        declaracoes.push(format!(
            "    final {d} = {util}.unsafeCast<{t}>(this.locals[{chave}]);"
        ));
    }
    // Os nós, depois os ouvintes — e só então o `initRootNode`, que é a
    // declaração de fechamento (`_generateInitStatement`).
    let corpo = dentro
        .linhas
        .iter()
        .chain(&dentro.ouvintes)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    // Mesma ordem da visão de topo: ligações de texto, visões-filhas e
    // âncoras, valores anteriores, elementos.
    let mut todos = dentro.campos.clone();
    todos.extend(dentro.campos_filho.clone());
    todos.extend(dentro.campos_expr.clone());
    todos.extend(dentro.campos_el.clone());
    let campos = if todos.is_empty() {
        String::new()
    } else {
        format!(
            "{}
",
            todos.join(
                "
"
            )
        )
    };
    // Ligação escrita só na primeira checagem precisaria de `firstCheck`
    // declarado aqui também; sem caso no corpus, fica de fora.
    if dentro.usa_primeira_checagem {
        return Err(Motivo::Ligacao);
    }
    let mut linhas_det = Vec::new();
    // `_ctx` só é declarado se o corpo o usar de fato: numa visão de `*ngFor`
    // a interpolação costuma usar o local do laço, não o contexto.
    if cita_ctx(&dentro.entradas) || cita_ctx(&dentro.deteccao) {
        linhas_det.push("    final _ctx = this.ctx;".to_string());
    }
    // Mesma ordem da visão de topo, com os locais antes de tudo.
    linhas_det.extend(declaracoes);
    linhas_det.extend(dentro.entradas.clone());
    for a in &dentro.ancoras {
        linhas_det.push(format!("    this.{a}.detectChangesInNestedViews();"));
    }
    linhas_det.extend(dentro.deteccao.clone());
    for v in &dentro.vistas_filhas {
        linhas_det.push(format!("    this.{v}.detectChanges();"));
    }
    let deteccao = if linhas_det.is_empty() {
        String::new()
    } else {
        format!(
            "
  @override
  void detectChangesInternal() {{
{}
  }}
",
            linhas_det.join(
                "
"
            )
        )
    };
    let deteccao = resolver_tardios(dentro.imp, &deteccao);
    // Visão embutida também destrói o que pendurou nela.
    let destruicao = if dentro.ancoras.is_empty() && dentro.vistas_filhas.is_empty() {
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
        format!(
            "
  @override
  void destroyInternal() {{
{}
  }}
",
            linhas.join(
                "
"
            )
        )
    };
    let aninhadas = std::mem::take(&mut dentro.embutidas);
    // Tira os dois emprestados de dentro da estrutura: a recursão precisa
    // deles, e `dentro` não vive além daqui.
    let imp = dentro.imp;
    let nomes = dentro.nomes;
    let mut texto = format!(
        "\nclass {classe} extends {ev}.EmbeddedView<{tipo_do_contexto}> {{\n{campos}  {classe}({rv}.RenderView parentView, int parentIndex) : super(parentView, parentIndex);\n  @override\n  void build() {{\n{corpo}\n    this.initRootNode(_el_0);\n  }}\n{deteccao}{destruicao}}}\n\n{ev}.EmbeddedView<void> {fabrica}({rv}.RenderView parentView, int parentIndex) {{\n  return {classe}(parentView, parentIndex);\n}}\n"
    );
    for a in aninhadas {
        texto.push_str(&emitir_embutida(
            a,
            imp,
            membros,
            metodos,
            filhos,
            asset,
            tipos,
            com_estilo,
            url_do_template.clone(),
            classe_da_visao,
            tipo_do_contexto,
            html,
            nomes,
        )?);
    }
    Ok(texto)
}

/// Aloca os imports que os **campos** da classe usam, na ordem em que eles
/// são declarados — ligações de texto, visões-filhas e diretivas
/// estruturais, e por fim `dart:html` dos elementos que viram campo.
///
/// A ordem dos imports é a ordem em que o oficial escreve o arquivo, e os
/// campos vêm antes de tudo na classe. Vale igual para a visão de topo e
/// para cada visão embutida.
fn alocar_imports_dos_campos(
    imp: &mut Importacoes,
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    asset: &str,
) -> Result<(), Motivo> {
    for campo in campos_em_ordem(nos, filhos) {
        match campo {
            CampoDaVisao::Filho(f) => {
                for uri in [&f.uri_template, &f.uri_dart] {
                    let alvo =
                        asset_de_uri(uri, "", Path::new("")).ok_or(Motivo::ComponenteNoTemplate)?;
                    let caminho =
                        caminho_do_import(asset, &alvo).ok_or(Motivo::ComponenteNoTemplate)?;
                    imp.alias(&caminho);
                }
            }
            CampoDaVisao::Estrutural(uri) => {
                imp.alias(VIEW_CONTAINER);
                imp.alias(uri);
            }
        }
    }
    if tem_elemento_ligado(nos, filhos) {
        imp.alias("dart:html");
    }
    Ok(())
}

/// Alguma linha usa `_ctx`?
fn cita_ctx(linhas: &[String]) -> bool {
    linhas.iter().any(|l| {
        l.split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|t| t == "_ctx")
    })
}

/// Índice do elemento que serve de pai, ou `null` se for a raiz da visão.
fn indice_do_elemento(pai: &str) -> String {
    pai.rsplit_once("_el_")
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| "null".to_string())
}

/// Quantas diretivas estruturais há na subárvore — é o salto que a
/// numeração das visões embutidas dá antes do próximo irmão.
fn contar_estruturais(nos: &[No]) -> u32 {
    nos.iter()
        .map(|n| match n {
            No::Elemento(e) => u32::from(e.estrela.is_some()) + contar_estruturais(&e.filhos),
            _ => 0,
        })
        .sum()
}

/// O local aparece em alguma expressão da subárvore?
///
/// A declaração dele abre o `detectChangesInternal` e traz o `dart:core`
/// junto, então a decisão precisa ser tomada antes de percorrer o corpo.
fn local_citado(nos: &[No], nome: &str) -> bool {
    let cita = |texto: &str| {
        texto
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')
            .any(|t| t == nome)
    };
    nos.iter().any(|n| match n {
        No::Interpolacao { expr, .. } => cita(expr),
        No::Elemento(e) => {
            e.propriedades
                .iter()
                .chain(e.eventos.iter())
                .any(|l| cita(&l.valor))
                || e.estrela.as_ref().is_some_and(|l| cita(&l.valor))
                || local_citado(&e.filhos, nome)
        }
        _ => false,
    })
}

/// As diretivas estruturais que o gerador conhece, com o que muda em cada
/// uma. O que faz `NgIf` e `NgFor` diferirem está declarado no ngcompiler:
/// `_isDirectBinding` (em `semantic_analysis/binding_converter.dart`) isenta
/// o `NgIf` do `checkBinding`, e o `NgFor` implementa `DoCheck`.
struct Estrutural {
    classe: &'static str,
    uri: &'static str,
    /// A entrada é escrita direto, sem `checkBinding`.
    direta: bool,
    /// Implementa `DoCheck`: a visão chama `ngDoCheck()` na detecção.
    do_check: bool,
}

impl Estrutural {
    fn conhecida(nome: &str) -> Option<Estrutural> {
        match nome {
            "ngIf" => Some(Estrutural {
                classe: "NgIf",
                uri: NG_IF,
                direta: true,
                do_check: false,
            }),
            "ngFor" => Some(Estrutural {
                classe: "NgFor",
                uri: NG_FOR,
                direta: false,
                do_check: true,
            }),
            _ => None,
        }
    }
}

/// `List<String>` -> `String`. Sem argumento de tipo não dá para tipar o
/// local do laço, e o oficial escreve `unsafeCast<T>` com T explícito.
fn tipo_do_elemento(tipo: &str) -> Option<String> {
    let t = tipo.trim().trim_end_matches('?');
    let abre = t.find('<')?;
    let base = &t[..abre];
    if !matches!(base, "List" | "Iterable" | "Set") {
        return None;
    }
    let dentro = t[abre + 1..].strip_suffix('>')?;
    if dentro.contains(',') || dentro.contains('<') {
        return None;
    }
    Some(dentro.trim().to_string())
}

/// Prefixo de import alocado mais tarde. O oficial numera os imports na
/// ordem em que **escreve** o arquivo, e o `detectChangesInternal` é escrito
/// depois do `build()` inteiro; nós montamos os dois ao mesmo tempo, andando
/// pelo template. A marca guarda a URI até a detecção ser escrita
/// ([`resolver_tardios`]).
fn tardio(uri: &str) -> String {
    format!("\u{1}{uri}\u{2}")
}

/// Troca cada marca de [`tardio`] pelo prefixo, alocando na ordem do texto.
fn resolver_tardios(imp: &mut Importacoes, texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(i) = resto.find('\u{1}') {
        saida.push_str(&resto[..i]);
        let Some(f) = resto[i..].find('\u{2}') else {
            break;
        };
        saida.push_str(&imp.alias(&resto[i + 1..i + f]));
        resto = &resto[i + f + 1..];
    }
    saida.push_str(resto);
    saida
}

/// Nome de propriedade ou atributo com contexto de segurança em alguma tag
/// (`_initializeSecuritySchema`, em `dom_element_schema_registry.dart`): o
/// valor sai embrulhado num `sanitize*`. Sem olhar a tag, recusa o nome em
/// qualquer elemento.
fn com_seguranca(nome: &str) -> bool {
    matches!(
        nome,
        "srcdoc"
            | "innerHTML"
            | "outerHTML"
            | "style"
            | "formAction"
            | "href"
            | "ping"
            | "src"
            | "cite"
            | "background"
            | "action"
            | "srcset"
            | "poster"
            | "code"
            | "codebase"
            | "profile"
            | "manifest"
            | "data"
    )
}

/// `isNativeHtmlEvent` do ngcompiler (`html_events.dart`): só estes vão
/// direto para `addEventListener`.
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

/// Nome que pertence a uma diretiva do ecossistema, não ao DOM.
fn e_de_diretiva(nome: &str) -> bool {
    let base = nome.split('.').next().unwrap_or(nome);
    base.starts_with("ng") || base.starts_with("form") && base != "form"
}

/// O template tem `{{ … }}` nesta visão?
///
/// Não desce na subárvore de um `*`: aquele conteúdo é da visão embutida, e
/// os campos dele são declarados lá.
fn tem_interpolacao(nos: &[No]) -> bool {
    nos.iter().any(|n| match n {
        No::Interpolacao { .. } => true,
        No::Elemento(e) if e.estrela.is_none() => tem_interpolacao(&e.filhos),
        _ => false,
    })
}

/// Literal Dart de uma string, com aspas simples.
fn literal(t: &str) -> String {
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

/// O arquivo de uma `@Directive` com `@HostBinding`: a classe `XNgCd`, que o
/// oficial gera para tirar de cada ponto de uso a detecção das ligações do
/// hospedeiro (emissão em `directive_compiler.dart`, ligações por
/// `bindAndWriteToRenderer` com `isHtmlElement` falso — daí o
/// `updateClassBindingNonHtml`). O `checkBinding` leva `null, null` porque a
/// ligação de hospedeiro não tem texto de template.
pub fn detector_de_diretiva(h: &crate::Hospedeira, arquivo: &str) -> String {
    let mut imp = Importacoes::default();
    // A ordem dos imports é a da escrita da classe: a superclasse, o campo
    // `instance`, os parâmetros de `detectHostChanges` e, no corpo,
    // `checkBinding` e o `dom_helpers`.
    let cd = imp.alias(DIRECTIVE_CHANGE_DETECTOR);
    let proprio = imp.alias(arquivo);
    let rv = imp.alias(RENDER_VIEW);
    let html = imp.alias("dart:html");
    let chk = imp.alias(CHECK_BINDING);
    let dom = imp.alias(DOM_HELPERS);
    let x = &h.classe;
    let mut campos = String::new();
    let mut corpo = String::new();
    for (k, (classe_css, membro)) in h.classes.iter().enumerate() {
        let _ = writeln!(campos, "  Object? _expr_{k};");
        let _ = write!(
            corpo,
            "    final currVal_{k} = this.instance.{membro};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, null, null)) {{\n      {dom}.updateClassBindingNonHtml(el, '{classe_css}', currVal_{k});\n      this._expr_{k} = currVal_{k};\n    }}\n"
        );
    }
    let mut s = String::with_capacity(1024);
    s.push_str(crate::CABECALHO);
    let _ = writeln!(s, "import '{arquivo}';");
    imp.escrever(&mut s);
    let _ = write!(
        s,
        "\nclass {x}NgCd extends {cd}.DirectiveChangeDetector {{\n  final {proprio}.{x} instance;\n{campos}  {x}NgCd(this.instance);\n  void detectHostChanges({rv}.RenderView view, {html}.Element el) {{\n{corpo}  }}\n}}\n"
    );
    s
}

/// Gera o `.template.dart` de um arquivo com um componente só, cujo template
/// tem apenas elementos HTML e texto — sem ligação, diretiva, projeção,
/// folha de estilo nem injeção no construtor.
///
/// O que não couber volta `None` e continua vindo do `build_runner`.
pub fn template_de_componente(
    c: &Componente,
    local: &Local,
    nos: &[No],
    resolvedor: Option<&dyn Resolucao>,
    nomes: &mut dartforge_intern::Interner,
    filhos: &std::collections::HashMap<String, Filho>,
) -> Result<String, Motivo> {
    if let Some((m, _)) = c.nao_entendidos.first() {
        return Err(*m);
    }
    if let Some(m) = formas_contra_o_template(c, local, nos, resolvedor, filhos).first() {
        return Err(*m);
    }
    if !c.styles.is_empty() {
        // `styles: ['…']` escrito na anotação ainda não.
        return Err(Motivo::Estilos);
    }
    // A construção sai depois dos imports fixos, porque a injeção aloca os
    // seus (o `errors.dart` e o de cada tipo injetado) no fim da tabela.
    if let Some(m) = falta_para_construir(c, local, resolvedor) {
        return Err(m);
    }

    let mut imp = Importacoes::default();
    // A folha compilada é o primeiro import do arquivo, antes de tudo.
    let estilo = match c.style_urls.len() {
        0 => None,
        1 => {
            // A folha entra pela URI `package:` mesmo estando ao lado: é
            // assim que o oficial escreve (o resolvedor de `styleUrls` é
            // outro, e não passa pelo caminho relativo).
            let uri = local
                .uri_do_estilo(&c.style_urls[0])
                .ok_or(Motivo::Estilos)?;
            Some(imp.alias(&uri))
        }
        // Mais de uma folha muda a lista de `styles$X`; uma de cada vez.
        _ => return Err(Motivo::Estilos),
    };
    let vista = imp.alias(COMPONENT_VIEW);
    let proprio = imp.alias(local.arquivo);
    // Os campos da visão saem antes de tudo na classe, então os seus imports
    // são alocados antes: é o que faz a numeração bater com a do oficial.
    // Os campos da visão saem antes de tudo na classe — ligações de texto,
    // visões-filhas, valores anteriores, elementos —, e os imports são
    // alocados nessa mesma ordem. É isso que faz a numeração bater com a do
    // oficial; fora de ordem, a comparação byte a byte não vale nada.
    let tb = tem_interpolacao(nos).then(|| imp.alias(TEXT_BINDING));
    alocar_imports_dos_campos(&mut imp, nos, filhos, &local.asset())?;
    let estilos = imp.alias(STYLE_ENCAPSULATION);
    let view = imp.alias(VIEW);
    let cd = imp.alias(CHANGE_DETECTION);
    let util = imp.alias(UTILITIES);
    // O construtor da visão usa `document.createElement`, então `dart:html`
    // sempre entra antes do corpo do `build()`.
    let html = imp.alias("dart:html");

    let mut corpo = Corpo {
        linhas: Vec::new(),
        ouvintes: Vec::new(),
        refs_livres: referencias_livres(nos),
        refs: Default::default(),
        campos: Vec::new(),
        campos_filho: Vec::new(),
        vistas_filhas: Vec::new(),
        filhos,
        asset: local.asset(),
        tipos: resolvedor.map(|r| (r, local.caminho)),
        com_estilo: !c.style_urls.is_empty(),
        embutidas: Vec::new(),
        classe_da_visao: format!("View{}", c.classe),
        ancoras: Vec::new(),
        embutida: false,
        locais: Default::default(),
        proxima_embutida: 1,
        tipo_do_contexto: format!("{proprio}.{}", c.classe),
        campos_expr: Vec::new(),
        campos_el: Vec::new(),
        proxima_ligacao: 0,
        deteccao: Vec::new(),
        nomes,
        usa_primeira_checagem: false,
        entradas: Vec::new(),
        tb,
        url_do_template: local.url_do_template.clone(),
        membros: &c.membros,
        metodos: &c.metodos,
        proximo: 0,
        tem_doc: false,
        usa_ctx_no_build: false,
        proxima_projecao: 0,
        imp: &mut imp,
        html: html.clone(),
    };
    corpo.nos(nos, "parentRenderNode")?;
    // `@ViewChild` estático: atribuição imediata, no `afterNodes` — depois
    // dos ouvintes, na ordem de declaração das consultas
    // (`updateQueryAtStartup`, `createImmediateUpdates` em
    // `compile_query.dart`). `formas_contra_o_template` já garantiu que cada
    // `#ref` está uma vez só, num elemento HTML da própria visão.
    let mut consultas = Vec::new();
    for q in &c.consultas {
        let Some(alvo) = corpo.refs.get(&q.referencia) else {
            return Err(Motivo::ViewChildDinamico);
        };
        consultas.push(format!("    _ctx.{} = {alvo};", q.propriedade));
        corpo.usa_ctx_no_build = true;
    }
    // Os `@HostListener` do componente fecham o `build()`, ligados ao nó
    // raiz (`_writeComponentHostEventListeners`, depois do
    // `writeBuildStatements` em `_generateBuildMethod`).
    let mut hospedeiro = Vec::new();
    for o in &c.ouvintes {
        corpo.usa_ctx_no_build = true;
        hospedeiro.push(format!(
            "    parentRenderNode.addEventListener('{}', this.eventHandler{}(_ctx.{}));",
            o.evento, o.aridade, o.metodo
        ));
    }
    let ctx_no_build = if corpo.usa_ctx_no_build {
        "
    final _ctx = this.ctx;"
    } else {
        ""
    };
    // Ordem do `build()` oficial (`_generateBuildMethod`): os nós (a fase
    // `_buildView`), os ouvintes (`bindView`) e, por fim, o que o `afterNodes`
    // acrescenta.
    let linhas = corpo
        .linhas
        .iter()
        .chain(&corpo.ouvintes)
        .chain(&consultas)
        .chain(&hospedeiro)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let corpo_build = if linhas.is_empty() {
        String::new()
    } else {
        format!("\n{linhas}")
    };
    // Ordem dos campos na classe, como o oficial escreve: ligações de texto,
    // depois os valores anteriores das ligações, depois os elementos.
    let especs = std::mem::take(&mut corpo.embutidas);
    let mut todos = corpo.campos.clone();
    todos.extend(corpo.campos_filho.clone());
    todos.extend(corpo.campos_expr.clone());
    todos.extend(corpo.campos_el.clone());
    let campos = if todos.is_empty() {
        String::new()
    } else {
        format!(
            "{}
",
            todos.join(
                "
"
            )
        )
    };
    // A detecção na ordem de `writeChangeDetectionStatements`: entradas de
    // diretivas e filhos, visões aninhadas, ligações de propriedade e texto,
    // visões-filhas. `_ctx` e `firstCheck` só são declarados se alguém os
    // lê (`maybeCachedCtxDeclarationStatement`).
    let mut linhas_deteccao = corpo.entradas.clone();
    for a in &corpo.ancoras {
        linhas_deteccao.push(format!("    this.{a}.detectChangesInNestedViews();"));
    }
    linhas_deteccao.extend(corpo.deteccao.iter().cloned());
    for v in &corpo.vistas_filhas {
        linhas_deteccao.push(format!("    this.{v}.detectChanges();"));
    }
    let deteccao = if linhas_deteccao.is_empty() {
        String::new()
    } else {
        let ctx = if cita_ctx(&linhas_deteccao) {
            "    final _ctx = this.ctx;
"
        } else {
            ""
        };
        let primeira = if corpo.usa_primeira_checagem {
            "    bool firstCheck = this.firstCheck;
"
        } else {
            ""
        };
        format!(
            "
  @override
  void detectChangesInternal() {{
{ctx}{primeira}{}
  }}
",
            linhas_deteccao.join(
                "
"
            )
        )
    };
    // Os imports da detecção entram agora, depois dos do `build()`.
    let deteccao = resolver_tardios(corpo.imp, &deteccao);
    // Visão-filha precisa ser destruída com a visão que a criou.
    let destruicao = if corpo.vistas_filhas.is_empty() && corpo.ancoras.is_empty() {
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
        format!(
            "
  @override
  void destroyInternal() {{
{}
  }}
",
            linhas.join(
                "
"
            )
        )
    };
    // `corpo` empresta o interner e a tabela de imports; a emissão das
    // visões embutidas precisa dos dois.
    drop(corpo);

    imp.sem_alias(ANGULAR);
    // As visões embutidas são escritas aqui, depois das fábricas — e é por
    // isso que os imports delas vêm depois do `angular.dart`.
    let mut embutidas = String::new();
    for espec in especs {
        embutidas.push_str(&emitir_embutida(
            espec,
            &mut imp,
            &c.membros,
            &c.metodos,
            filhos,
            &local.asset(),
            resolvedor.map(|r| (r, local.caminho)),
            !c.style_urls.is_empty(),
            local.url_do_template.clone(),
            &format!("View{}", c.classe),
            &format!("{proprio}.{}", c.classe),
            &html,
            nomes,
        )?);
    }
    let hosp = imp.alias(HOST_VIEW);
    let construcao = construcao_do_componente(c, local, resolvedor, &mut imp, &proprio, &util)
        .ok_or(Motivo::InjecaoNaoResolvida)?;
    // Os ganchos de ciclo de vida saem na visão-hospedeira, depois do
    // `build()`, e o `check_binding.dart` entra aí.
    let ciclo = if c.ganchos.algum() {
        ciclo_de_vida(&c.ganchos, &imp.alias(CHECK_BINDING))
    } else {
        String::new()
    };

    let x = &c.classe;
    let seletor = &c.seletor;
    let estado = if c.on_push {
        "waitingToBeChecked"
    } else {
        "checkAlways"
    };
    // Sem folha, a lista é constante e o estilo não é encapsulado.
    let (lista_de_estilos, encapsulamento) = match &estilo {
        Some(a) => (format!("[{a}.styles]"), "scoped"),
        None => ("const []".to_string(), "unscoped"),
    };
    let asset = format!("asset:{}/{}", local.pacote, local.relativo);

    let mut s = String::with_capacity(4096);
    s.push_str(crate::CABECALHO);
    let _ = writeln!(s, "import '{}';", local.arquivo);
    imp.escrever(&mut s);
    let _ = write!(
        s,
        "
final List<Object> styles${x} = {lista_de_estilos};

class View{x}0 extends {vista}.ComponentView<{proprio}.{x}> {{
{campos}  static {estilos}.ComponentStyles? _componentStyles;
  View{x}0({view}.View parentView, int parentIndex) : super(parentView, parentIndex, {cd}.ChangeDetectionCheckedState.{estado}) {{
    this.initComponentStyles();
    this.rootElement = {util}.unsafeCast({html}.document.createElement('{seletor}'));
  }}
  static String? get _debugComponentUrl {{
    return ({util}.isDevMode ? '{asset}' : null);
  }}

  @override
  void build() {{{ctx_no_build}
    final parentRenderNode = this.initViewRoot();{corpo_build}
  }}
{deteccao}{destruicao}
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

ComponentFactory<{proprio}.{x}> create{x}Factory() {{
  return ComponentFactory('{seletor}', viewFactory_{x}Host0);
}}
{embutidas}
final List<Object> styles${x}Host = const [];

class _View{x}Host0 extends {hosp}.HostView<{proprio}.{x}> {{
  @override
  void build() {{
    this.componentView = View{x}0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = {construcao}
    this.initRootNode(_el_0);
  }}
{ciclo}}}

{hosp}.HostView<{proprio}.{x}> viewFactory_{x}Host0() {{
  return _View{x}Host0();
}}
"
    );
    Ok(s)
}

/// A construção do componente na visão-hospedeira, com injeção — o texto
/// inteiro depois de `this.component = `, ponto e vírgula incluído.
///
/// Cada parâmetro do construtor vira `this.injectorGet(T, this.parentIndex)`,
/// com `T` qualificado pelo import da biblioteca que **declara** o tipo — é
/// por isso que a injeção precisou do banco semântico. Um parâmetro do tipo
/// `Element` é o elemento raiz e não passa pelo injetor.
///
/// Havendo injeção, o oficial embrulha a chamada em `debugInjectorWrap` sob
/// `isDevMode`, para que um token faltando aponte o componente; sem injeção a
/// chamada sai limpa.
fn construcao_do_componente(
    c: &Componente,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
    imp: &mut Importacoes,
    proprio: &str,
    util: &str,
) -> Option<String> {
    let x = &c.classe;
    let injeta = c.parametros.iter().any(|p| !e_elemento(p.tipo.as_deref()));
    // O `errors.dart` entra antes dos tipos injetados, como no oficial.
    let erros = injeta.then(|| imp.alias(DI_ERRORS));
    let mut args = Vec::new();
    for p in &c.parametros {
        if e_elemento(p.tipo.as_deref()) {
            args.push("_el_0".to_string());
            continue;
        }
        if p.anotado || p.nomeado {
            return None; // `@Optional`, `@Inject(...)`, nomeado: ainda não
        }
        let tipo = p.tipo.as_deref()?;
        if tipo.contains('<') {
            return None; // token genérico ainda não
        }
        let simples = tipo.rsplit('.').next()?;
        let uri = resolvedor?.uri_do_tipo(local.caminho, tipo)?;
        let asset = asset_de_uri(&uri, local.pacote, local.raiz)?;
        let caminho = caminho_do_import(&local.asset(), &asset)?;
        let alias = imp.alias(&caminho);
        args.push(format!(
            "this.injectorGet({alias}.{simples}, this.parentIndex)"
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

/// O parâmetro é o elemento raiz do componente?
fn e_elemento(tipo: Option<&str>) -> bool {
    matches!(
        tipo.map(|t| t.rsplit('.').next().unwrap_or(t)),
        Some("Element" | "HtmlElement")
    )
}

/// O que impede a construção, se algo impede. A tabela de imports não é
/// tocada aqui — isto só olha.
fn falta_para_construir(
    c: &Componente,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> Option<Motivo> {
    for p in &c.parametros {
        if e_elemento(p.tipo.as_deref()) {
            continue;
        }
        if p.anotado {
            return Some(Motivo::InjecaoAnotada);
        }
        if p.nomeado {
            return Some(Motivo::InjecaoNomeada);
        }
        let Some(tipo) = p.tipo.as_deref() else {
            return Some(Motivo::InjecaoSemTipo);
        };
        if tipo.contains('<') {
            return Some(Motivo::InjecaoGenerica);
        }
        if !resolvedor.is_some_and(|r| r.uri_do_tipo(local.caminho, tipo).is_some()) {
            return Some(Motivo::InjecaoNaoResolvida);
        }
    }
    None
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::componente::Parametro;
    use dartforge_intern::Interner;

    fn local() -> Local<'static> {
        Local {
            pacote: "new_sali_frontend",
            relativo: "lib/src/shared/components/form_feedback/form_feedback_component.dart",
            arquivo: "form_feedback_component.dart",
            caminho: Path::new("x.dart"),
            raiz: Path::new(""),
            url_do_template: None,
        }
    }

    /// Responde o que o banco semântico responderia, sem carregar um
    /// programa: o par (nome do tipo, URI da biblioteca que o declara).
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
        }
    }

    /// Bytes exatos do arquivo que o compilador oficial gerou para
    /// `FormFeedbackComponent` (template só com um comentário).
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
        )
        .expect("gera");
        let esperado = include_str!("../testes/form_feedback_component.template.dart");
        assert_eq!(saida, esperado.replace("\r\n", "\n"));
    }

    /// Bytes exatos do `CallbackComponent`, cujo template é
    /// `<div>Processando login...</div>` — o primeiro com nós de verdade.
    /// A injeção no construtor é trocada por um construtor sem parâmetros, que
    /// é o que este passo cobre; o resto do arquivo é o do oficial.
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
        };
        let nos = crate::html::analisar("<div>Processando login...</div>");
        let saida = template_de_componente(
            &c,
            &local,
            &nos,
            None,
            &mut Interner::new(),
            &Default::default(),
        )
        .expect("gera");
        let esperado = include_str!("../testes/callback_component.template.dart");
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
                &Default::default()
            ),
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
                &Default::default()
            ),
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
        // Sem banco semântico não há como saber que biblioteca declara o tipo.
        assert_eq!(
            template_de_componente(
                &c,
                &local(),
                &[],
                None,
                &mut Interner::new(),
                &Default::default()
            ),
            Err(Motivo::InjecaoNaoResolvida)
        );
    }

    /// Bytes exatos do `CallbackComponent` oficial, agora **com** a injeção:
    /// dois tokens, um do próprio pacote (import relativo) e um do ngrouter
    /// (import `package:` da biblioteca que declara o tipo).
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
        )
        .expect("gera");
        let esperado = include_str!("../testes/callback_com_injecao.template.dart");
        assert_eq!(
            saida,
            esperado.replace(
                "
", "
"
            )
        );
    }
}
