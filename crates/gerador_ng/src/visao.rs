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
    /// (chave, URI, com prefixo). A chave é a URI, a não ser quando o
    /// oficial importa a mesma biblioteca duas vezes (ver [`Self::q_chave`]).
    itens: Vec<(String, String, bool)>,
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

    /// Como [`Self::q`], mas com uma chave própria: a mesma URI ganha outro
    /// número. É o que o oficial faz com o argumento de tipo de um
    /// `MultiToken` (`MultiToken<import20.ControlValueAccessor<dynamic>>` ao
    /// lado do `import7.ControlValueAccessor` do campo): o tipo vem de outro
    /// caminho do analyzer e o import é outro, mesmo com o mesmo texto.
    pub fn q_chave(&mut self, chave: &str, uri: &str) -> String {
        let n = match self.itens.iter().position(|(c, _, _)| c == chave) {
            Some(i) => i,
            None => {
                self.itens.push((chave.to_string(), uri.to_string(), true));
                self.itens.len() - 1
            }
        };
        format!("import{n}.")
    }

    fn indice(&mut self, uri: &str, com_alias: bool) -> usize {
        if let Some(i) = self.itens.iter().position(|(c, _, _)| c == uri) {
            return i;
        }
        self.itens
            .push((uri.to_string(), uri.to_string(), com_alias));
        self.itens.len() - 1
    }

    fn escrever(&self, saida: &mut String) {
        for (i, (_, uri, com_alias)) in self.itens.iter().enumerate() {
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
const PROXIES: &str = "package:ngdart/src/runtime/proxies.dart";
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
const APP_VIEW_UTILS: &str = "package:ngdart/src/core/linker/app_view_utils.dart";

/// Por que um arquivo ainda não é gerado por nós. O placar conta por motivo:
/// é isso que diz qual forma vale a pena aprender em seguida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    /// Um elemento casa com o seletor de uma diretiva de `directives:` que o
    /// emissor não instancia (`<form>` com `NgForm`, `<option>`,
    /// `[routerLink]`…). Emitir o elemento puro compilaria e faria outra
    /// coisa.
    DiretivaPorSeletor,
    /// `(evento)` no template fora do que o emissor sabe ligar.
    Evento,
    /// Expressão do template fora do que o conversor traduz. É a categoria
    /// provisória do conversor: quem o chama troca pela do contexto
    /// (interpolação, ligação, evento) com [`Recusa::em`].
    Expressao,
}

/// Uma recusa: a categoria do placar e a sub-forma concreta que o emissor
/// não sabe traduzir (`evento: handler com atribuição`, `interpolação: tipo
/// de ternário`). O placar conta as duas; é a sub-forma que diz o que
/// atacar em seguida.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Recusa {
    pub motivo: Motivo,
    pub forma: String,
}

impl Recusa {
    pub fn new(motivo: Motivo, forma: impl Into<String>) -> Self {
        Recusa {
            motivo,
            forma: forma.into(),
        }
    }

    /// A mesma sub-forma no contexto de quem chamou o conversor de
    /// expressões. Recusa que já tem categoria própria (pipe) fica como está.
    pub fn em(self, motivo: Motivo) -> Self {
        if self.motivo == Motivo::Expressao {
            Recusa { motivo, ..self }
        } else {
            self
        }
    }

    pub fn texto(&self) -> String {
        format!("{}: {}", self.motivo.texto(), self.forma)
    }
}

/// Atalho: `Err(recusa(Motivo::Evento, "handler com atribuição"))`.
pub fn recusa(motivo: Motivo, forma: impl Into<String>) -> Recusa {
    Recusa::new(motivo, forma)
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
            Motivo::DiretivaPorSeletor => "diretiva casada por seletor",
            Motivo::Evento => "evento",
            Motivo::Expressao => "expressão",
        }
    }
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

/// As formas do componente que só se decidem olhando o template: onde está
/// o `#ref` de cada `@ViewChild`. Devolve o que
/// ainda impede a geração, na ordem em que aparece.
fn formas_contra_o_template(
    c: &Componente,
    local: &Local,
    nos: &[No],
    resolvedor: Option<&dyn Resolucao>,
    filhos: &std::collections::HashMap<String, Filho>,
) -> Vec<Recusa> {
    let mut fora = Vec::new();
    for consulta in &c.consultas {
        let mut lugares = Vec::new();
        onde_esta(nos, &consulta.referencia, filhos, false, &mut lugares);
        let r = match lugares.as_slice() {
            // `isElementType`: campo `Element` (ou subtipo) recebe o nó;
            // qualquer outro, um `ElementRef`. O primeiro caso é o estático,
            // que sai no `build()`.
            [Lugar::Raiz] if e_tipo_de_elemento(&consulta.tipo, local, resolvedor) => continue,
            [Lugar::Raiz] => recusa(
                Motivo::ViewChildEmFilho,
                "@ViewChild de elemento com tipo que não é Element",
            ),
            // A instância do filho: o campo tem de ser do tipo dele (não um
            // `Element`) e o filho não pode ser `onPush`, que registra o
            // `ChangeDetectorRef` da consulta (`queryChangeDetectorRefs`).
            [Lugar::NoFilho] => {
                if e_tipo_de_elemento(&consulta.tipo, local, resolvedor) {
                    recusa(
                        Motivo::ViewChildEmFilho,
                        "@ViewChild de tipo Element em #ref de filho",
                    )
                } else {
                    continue;
                }
            }
            [Lugar::Filho] => recusa(
                Motivo::ViewChildEmFilho,
                "@ViewChild de #ref em componente filho",
            ),
            // Dentro de `*` a consulta passa por `mapNestedViews`; sem
            // resultado, ou com dois, a regra é outra — nada disso ainda.
            [] => recusa(Motivo::ViewChildDinamico, "@ViewChild sem #ref no template"),
            [_] => recusa(
                Motivo::ViewChildDinamico,
                "@ViewChild de #ref em visão embutida",
            ),
            _ => recusa(Motivo::ViewChildDinamico, "@ViewChild de #ref repetido"),
        };
        fora.push(r);
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
    /// No conteúdo projetado num componente filho.
    Filho,
    /// No próprio elemento de um componente filho da visão: vale a
    /// instância.
    NoFilho,
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
        } else if !em_filho && filhos.contains_key(&e.nome) {
            Lugar::NoFilho
        } else if em_filho || !dom::tag_html(&e.nome) {
            Lugar::Filho
        } else {
            Lugar::Raiz
        };
        if e.referencias.iter().any(|r| r.nome == nome) {
            saida.push(lugar);
        }
        let mut dentro = Vec::new();
        let abaixo_de_filho = matches!(lugar, Lugar::Filho | Lugar::NoFilho);
        onde_esta(&e.filhos, nome, filhos, abaixo_de_filho, &mut dentro);
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

/// Um pipe de `pipes:`, com a URI da biblioteca que o declara.
#[derive(Debug, Clone)]
pub struct PipeUsado {
    pub uri: String,
    pub pipe: crate::componente::Pipe,
}

/// A instância de um pipe puro, campo da visão do componente
/// (`_pipe_date_0`): uma por nome, na ordem do primeiro uso
/// (`compView.purePipes`, `pipeCount`).
#[derive(Debug, Clone)]
struct InstanciaDePipe {
    nome: String,
    campo: String,
    classe: String,
    /// Caminho do import da biblioteca do pipe (`getImportModulePath`).
    caminho: String,
}

/// Uma chamada `$pipe.nome(..)`: a visão onde está e o proxy dela
/// (`_pipe_date_0_1`, `_PurePipeProxy`), que é campo dessa visão.
#[derive(Debug, Clone)]
struct ChamadaDePipe {
    vista: u32,
    instancia: usize,
    proxy: String,
    argumentos: usize,
    /// `o.FunctionType(retorno, paramTypes.sublist(0, argCount))`.
    tipo: String,
    /// O tipo cita o `dart:core` (tudo o que não é `dynamic`).
    core: bool,
}

/// Os pipes de um template: as instâncias e cada chamada, na ordem em que o
/// oficial as converte — a do `bindView`, que desce nas visões embutidas
/// onde elas estão. É essa ordem que numera os proxies, e ela só se sabe
/// olhando o template inteiro antes de emitir: as visões embutidas daqui
/// são emitidas depois da do componente.
#[derive(Debug, Default)]
struct PipesDoTemplate {
    instancias: Vec<InstanciaDePipe>,
    chamadas: Vec<ChamadaDePipe>,
}

impl PipesDoTemplate {
    fn da_vista(&self, vista: u32) -> impl Iterator<Item = &ChamadaDePipe> {
        self.chamadas.iter().filter(move |c| c.vista == vista)
    }

    /// Os imports dos campos de pipe desta visão, na ordem da declaração: a
    /// classe de cada instância (só na visão do componente) e o `dart:core`
    /// do tipo de um proxy.
    fn imports_dos_campos(&self, vista: u32) -> Vec<String> {
        let mut saida = Vec::new();
        for (k, inst) in self.instancias.iter().enumerate() {
            if vista == 0 {
                saida.push(inst.caminho.clone());
            }
            if self.da_vista(vista).any(|c| c.instancia == k && c.core) {
                saida.push("dart:core".to_string());
            }
        }
        saida
    }

    /// Os campos de pipe desta visão, na ordem em que o `create()` de cada
    /// `CompilePipe` os aloca: a instância e, logo depois, os proxies dela.
    /// Saem depois dos `_expr_` (alocados no `bindView`) e antes dos `_el_`
    /// (promovidos a campo no fim).
    fn campos(&self, vista: u32, imp: &mut Importacoes) -> Vec<String> {
        let mut saida = Vec::new();
        for (k, inst) in self.instancias.iter().enumerate() {
            if vista == 0 {
                let q = imp.q(&inst.caminho);
                saida.push(format!("  late final {q}{} {};", inst.classe, inst.campo));
            }
            for c in self.da_vista(vista).filter(|c| c.instancia == k) {
                saida.push(format!("  late final {} {};", c.tipo, c.proxy));
            }
        }
        saida
    }

    /// A criação no `build()` (`afterNodes`, depois dos ouvintes): a
    /// instância (`createPipeInstance`) e os proxies (`createPureProxy`),
    /// que leem a instância pela cadeia de `parentView` (`base`).
    fn criacao(&self, vista: u32, base: &str, imp: &mut Importacoes) -> Vec<String> {
        let mut saida = Vec::new();
        for (k, inst) in self.instancias.iter().enumerate() {
            if vista == 0 {
                let q = imp.q(&inst.caminho);
                saida.push(format!("    this.{} = {q}{}();", inst.campo, inst.classe));
            }
            for c in self.da_vista(vista).filter(|c| c.instancia == k) {
                saida.push(format!(
                    "    this.{} = {}.pureProxy{}({base}.{}.transform);",
                    c.proxy,
                    tardio(PROXIES),
                    c.argumentos,
                    inst.campo
                ));
            }
        }
        saida
    }
}

/// Tipo que `fromDartType` escreve sem import próprio: `dynamic` ou um tipo
/// do `dart:core`, com ou sem `?`.
fn tipo_do_core(t: &str) -> bool {
    t == "dynamic"
        || matches!(
            t.strip_suffix('?').unwrap_or(t),
            "String" | "int" | "double" | "num" | "bool" | "Object"
        )
}

/// A tabela de pipes do template (ver [`PipesDoTemplate`]). O pipe de cada
/// chamada é o último de `pipes:` com aquele nome (`_findPipeMeta`).
fn pipes_do_template(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    pipes: &Result<Vec<PipeUsado>, Recusa>,
    asset: &str,
) -> Result<PipesDoTemplate, Recusa> {
    let mut brutas = Vec::new();
    let mut proxima = 1;
    chamadas_brutas(nos, 0, &mut proxima, filhos, &mut brutas)?;
    let mut t = PipesDoTemplate::default();
    if brutas.is_empty() {
        return Ok(t);
    }
    let pipes = pipes.as_ref().map_err(Clone::clone)?;
    let fora = |f: &str| recusa(Motivo::PipesUsados, f);
    for (vista, nome, argumentos) in brutas {
        let usado = pipes
            .iter()
            .rev()
            .find(|p| p.pipe.nome == nome)
            .ok_or_else(|| fora("pipe que não está em pipes:"))?;
        let p = &usado.pipe;
        if let Some(f) = p.fora {
            return Err(fora(f));
        }
        if !p.puro {
            return Err(fora("pipe impuro (`pure: false`)"));
        }
        // Mais argumentos que parâmetros é erro de compilação no oficial.
        if argumentos > p.parametros.len() {
            return Err(fora("pipe com argumentos demais"));
        }
        // `Identifiers.pureProxies`: `pureProxy1` a `pureProxy6` conferidos
        // no `proxies.dart`.
        if argumentos > 6 {
            return Err(fora("pipe com mais de 6 argumentos"));
        }
        let parametros: Option<Vec<String>> = p.parametros[..argumentos]
            .iter()
            .map(|x| x.clone().filter(|x| tipo_do_core(x)))
            .collect();
        let (Some(parametros), true) = (parametros, tipo_do_core(&p.retorno)) else {
            return Err(fora("transform com tipo fora do dart:core ou sem tipo"));
        };
        let core = std::iter::once(&p.retorno)
            .chain(&parametros)
            .any(|x| x != "dynamic");
        let instancia = match t.instancias.iter().position(|i| i.nome == nome) {
            Some(k) => k,
            None => {
                let caminho = asset_de_uri(&usado.uri, "", Path::new(""))
                    .and_then(|alvo| caminho_do_import(asset, &alvo))
                    .ok_or_else(|| fora("pipe sem caminho de import"))?;
                t.instancias.push(InstanciaDePipe {
                    campo: format!("_pipe_{nome}_{}", t.instancias.len()),
                    nome: nome.clone(),
                    classe: p.classe.clone(),
                    caminho,
                });
                t.instancias.len() - 1
            }
        };
        let ja = t
            .chamadas
            .iter()
            .filter(|c| c.instancia == instancia)
            .count();
        t.chamadas.push(ChamadaDePipe {
            vista,
            instancia,
            proxy: format!("{}_{ja}", t.instancias[instancia].campo),
            argumentos,
            tipo: format!("{} Function({})", p.retorno, parametros.join(", ")),
            core,
        });
    }
    Ok(t)
}

/// As chamadas `$pipe.nome(..)` do template (visão, nome, argumentos), na
/// ordem do `bindView`: em cada elemento as ligações dele antes dos filhos,
/// e o conteúdo de um `*` no lugar dele, como visão nova. As visões são
/// numeradas como as embutidas (em profundidade, a partir de 1). Pipe onde
/// a ordem ainda não foi conferida (evento, `*`, entrada de filho) é
/// recusado.
fn chamadas_brutas(
    nos: &[No],
    vista: u32,
    proxima: &mut u32,
    filhos: &std::collections::HashMap<String, Filho>,
    saida: &mut Vec<(u32, String, usize)>,
) -> Result<(), Recusa> {
    let tem = |t: &str| t.contains("$pipe");
    for no in nos {
        match no {
            No::Interpolacao { expr, .. } => pipes_na_expressao(expr, vista, saida)?,
            No::Elemento(e) => {
                if let Some(estrela) = &e.estrela {
                    if tem(&estrela.valor) {
                        return Err(recusa(Motivo::PipesUsados, "pipe na entrada de `*`"));
                    }
                    let v = *proxima;
                    *proxima += 1;
                    let mut sem_estrela = e.clone();
                    sem_estrela.estrela = None;
                    let dentro = No::Elemento(sem_estrela);
                    chamadas_brutas(std::slice::from_ref(&dentro), v, proxima, filhos, saida)?;
                    continue;
                }
                if e.eventos.iter().any(|l| tem(&l.valor)) {
                    return Err(recusa(Motivo::PipesUsados, "pipe em evento"));
                }
                if e.bananas.iter().any(|l| tem(&l.valor)) {
                    return Err(recusa(Motivo::PipesUsados, "pipe em [(x)]"));
                }
                let de_filho = filhos.contains_key(&e.nome) || !dom::tag_html(&e.nome);
                // `[x]` e depois os atributos interpolados, como o
                // `elemento_html` converte.
                for l in &e.propriedades {
                    if tem(&l.valor) {
                        if de_filho {
                            return Err(recusa(
                                Motivo::PipesUsados,
                                "pipe em ligação de componente filho",
                            ));
                        }
                        pipes_na_expressao(&l.valor, vista, saida)?;
                    }
                }
                for a in e.atributos.iter().filter(|a| a.valor.contains("{{")) {
                    if !tem(&a.valor) {
                        continue;
                    }
                    if de_filho {
                        return Err(recusa(
                            Motivo::PipesUsados,
                            "pipe em ligação de componente filho",
                        ));
                    }
                    let Some((_, exprs)) = partes_da_interpolacao(&a.valor) else {
                        return Err(recusa(Motivo::PipesUsados, "pipe em atributo ilegível"));
                    };
                    for x in &exprs {
                        pipes_na_expressao(x, vista, saida)?;
                    }
                }
                chamadas_brutas(&e.filhos, vista, proxima, filhos, saida)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// As chamadas `$pipe.nome(..)` de uma expressão, em ordem de texto, com o
/// número de argumentos. Texto entre aspas não conta; pipe dentro de pipe é
/// recusado (o de dentro seria convertido primeiro).
fn pipes_na_expressao(
    texto: &str,
    vista: u32,
    saida: &mut Vec<(u32, String, usize)>,
) -> Result<(), Recusa> {
    let forma = || {
        recusa(
            Motivo::PipesUsados,
            "`$pipe` fora da forma `$pipe.nome(..)`",
        )
    };
    let e_nome = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let b = texto.as_bytes();
    let mut i = 0;
    let mut aspas: Option<u8> = None;
    let mut fim_do_ultimo = 0;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = aspas {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                aspas = None;
            }
            i += 1;
            continue;
        }
        if c == b'\'' || c == b'"' {
            aspas = Some(c);
            i += 1;
            continue;
        }
        if !texto[i..].starts_with("$pipe") {
            i += 1;
            continue;
        }
        if i > 0 && (e_nome(b[i - 1]) || b[i - 1] == b'.') {
            return Err(forma());
        }
        if i < fim_do_ultimo {
            return Err(recusa(Motivo::PipesUsados, "pipe dentro de pipe"));
        }
        if b.get(i + 5) != Some(&b'.') {
            return Err(forma());
        }
        let ini = i + 6;
        let mut j = ini;
        while j < b.len() && e_nome(b[j]) {
            j += 1;
        }
        let mut k = j;
        while k < b.len() && b[k].is_ascii_whitespace() {
            k += 1;
        }
        if j == ini || b.get(k) != Some(&b'(') {
            return Err(forma());
        }
        let (fim, argumentos) = argumentos_da_chamada(texto, k).ok_or_else(forma)?;
        saida.push((vista, texto[ini..j].to_string(), argumentos));
        fim_do_ultimo = fim;
        i = j;
    }
    Ok(())
}

/// Do `(` em `abre`: o índice depois do `)` que fecha e quantos argumentos
/// há (vírgulas no primeiro nível, mais um).
fn argumentos_da_chamada(texto: &str, abre: usize) -> Option<(usize, usize)> {
    let b = texto.as_bytes();
    let mut nivel = 0u32;
    let mut virgulas = 0;
    let mut algum = false;
    let mut aspas: Option<u8> = None;
    let mut i = abre;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = aspas {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                aspas = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'\'' | b'"' => aspas = Some(c),
            b'(' | b'[' | b'{' => nivel += 1,
            b')' | b']' | b'}' => {
                nivel -= 1;
                if nivel == 0 {
                    let n = if algum { virgulas + 1 } else { 0 };
                    return Some((i + 1, n));
                }
            }
            b',' if nivel == 1 => virgulas += 1,
            _ => {}
        }
        if nivel >= 1 && i > abre && !c.is_ascii_whitespace() {
            algum = true;
        }
        i += 1;
    }
    None
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
    /// `ngContentSelectors` do template do filho: o `select` de cada
    /// `<ng-content>`, em ordem, `*` sem ele. Com algum, quem usa o filho
    /// chama `createAndProject` com uma lista de nós por projeção.
    pub projecoes: Vec<String>,
    /// `@Input`s do filho, na ordem em que o oficial os escreve.
    pub entradas: Vec<crate::componente::Entrada>,
    /// Ganchos de ciclo de vida do filho: quem o usa os chama
    /// (`bindDirectiveDetectChangesLifecycleCallbacks`,
    /// `bindDirectiveAfterChildrenCallbacks`).
    pub ganchos: crate::componente::Ganchos,
    /// `onPush`: quem muda uma entrada marca a checagem do filho.
    pub on_push: bool,
    /// `@Output`s (nome no template, membro), na ordem do mapa `outputs`.
    pub saidas: Vec<(String, String)>,
    /// O que o construtor do filho recebe, na ordem.
    pub parametros: Vec<Injetado>,
    /// `@ContentChild`/`@ContentChildren` do filho, com o alvo resolvido:
    /// (campo, lista, alvo). Quem projeta conteúdo nele atualiza a consulta.
    pub consultas: Vec<(String, bool, AlvoDeConsulta)>,
    /// O que no filho muda o código de quem o usa e o emissor ainda não
    /// escreve: injeção no construtor, `@HostBinding`, consulta de conteúdo,
    /// provedores, projeção com seletor. Qualquer uma recusa o uso.
    pub pendencias: Vec<Recusa>,
}

/// Um parâmetro do construtor de um filho, como o oficial o resolve no nó
/// dele (`provider_resolver.dart`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Injetado {
    /// `Element`/`HtmlElement`: o próprio nó.
    Elemento,
    /// `ChangeDetectorRef`: a visão do filho.
    Detector,
    /// Serviço de fora da visão: `injectorGet` pela visão de cima
    /// (`injectFromViewParentInjector`), `injectorGetOptional` com
    /// `@Optional()`.
    Servico {
        uri: String,
        classe: String,
        opcional: bool,
    },
}

/// O que uma consulta de conteúdo de um filho procura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlvoDeConsulta {
    /// Uma classe (URI, nome).
    Classe(String, String),
    /// Um `#ref` do conteúdo.
    Referencia(String),
}

impl Filho {
    /// O membro que o `@Output` `nome` expõe.
    pub fn saida(&self, nome: &str) -> Option<&str> {
        self.saidas
            .iter()
            .find(|(n, _)| n == nome)
            .map(|(_, m)| m.as_str())
    }

    /// O `@Input` que recebe a ligação `nome`.
    pub fn entrada(&self, nome: &str) -> Option<&crate::componente::Entrada> {
        self.entradas.iter().find(|e| e.nome == nome)
    }
}

/// Uma diretiva ou componente de `directives:`, já resolvida, com o
/// seletor lido. A lista de um componente segue a ordem do oficial (a
/// expansão das listas constantes em profundidade, sem repetição): é a
/// ordem em que as diretivas de um nó são instanciadas.
#[derive(Debug, Clone)]
pub struct Usada {
    pub classe: String,
    /// URI da biblioteca que declara a classe.
    pub uri: String,
    pub seletores: Vec<crate::seletor::Seletor>,
    /// `Some` quando é componente: o que o emissor sabe dele.
    pub filho: Option<Filho>,
    /// Os metadados lidos do programa (`metadados.rs`), quando é diretiva e
    /// há programa carregado.
    pub diretiva: Option<std::sync::Arc<crate::diretivas::Diretiva>>,
}

impl Usada {
    /// É a diretiva estrutural do próprio ngdart que o emissor conhece
    /// (`NgIf`, `NgFor`)?
    fn e_estrutural(&self, e: &Estrutural) -> bool {
        self.classe == e.classe && self.uri == e.uri
    }
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
    /// Parâmetros posicionais de cada método, para o tear-off de evento.
    aridades: &'a std::collections::HashMap<String, usize>,
    /// Os métodos `_handleEvent_N` desta visão, na ordem em que foram
    /// criados (`createEventHandler`); saem depois do `destroyInternal`.
    metodos_evento: Vec<String>,
    /// Declaração (`final local_x = …;`) de cada local **desta** visão, ou
    /// por que ele não pode ser declarado. Local de visão ancestral não está
    /// aqui: ele é lido pela cadeia de `parentView`, forma ainda recusada.
    decl_locais: std::collections::HashMap<String, Result<String, Recusa>>,
    /// Os locais lidos pelas ligações da detecção, na ordem do primeiro uso
    /// (`_localsInScope` do `ViewNameResolver` da visão): é a ordem das
    /// declarações no topo do `detectChangesInternal`.
    locais_raiz: Vec<String>,
    /// Classe desta visão (`_ViewX2`) e os locais que ela mesma declara
    /// (nome, chave), para as visões aninhadas lerem pela `parentView`.
    classe_desta: String,
    locais_proprios: Vec<(String, String)>,
    /// Os locais das visões ancestrais, com de onde vêm.
    ancestrais: std::collections::HashMap<String, Origem>,
    /// Componentes que este template pode usar, por seletor.
    filhos: &'a std::collections::HashMap<String, Filho>,
    /// Todas as diretivas e componentes de `directives:`, para saber o que
    /// casa com cada elemento.
    usadas: &'a [Usada],
    /// Modo de coleta (o diagnóstico do placar): cada recusa é anotada aqui
    /// e a emissão segue, para achar as outras. `None`: a primeira recusa
    /// interrompe, e o arquivo fica com o `build_runner`.
    coleta: Option<Vec<Recusa>>,
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
    /// Os pipes do template, com a chamada de cada visão.
    pipes: &'a PipesDoTemplate,
    /// Número desta visão (0 a do componente, o `indice` na embutida) e
    /// quantas `parentView` a separam da visão do componente.
    vista: u32,
    profundidade: u32,
    /// Quantas chamadas de pipe desta visão já foram convertidas.
    cursor_pipe: usize,
    /// `changed` é lido ou escrito: `bool changed = false;` no topo da
    /// detecção.
    usa_changed: bool,
    /// Ganchos `ngAfterContent*` e `ngAfterView*` dos filhos, na ordem do
    /// oficial (de baixo para cima), já no nível do método: saem dentro do
    /// `if ((!debugThrowIfChanged))`.
    apos_conteudo: Vec<String>,
    apos_visao: Vec<String>,
    /// `ngOnDestroy` dos filhos, no fim do `destroyInternal`.
    destruir: Vec<String>,
    /// Quantas `subscription_N` (`@Output` de filho) esta visão tem.
    subscricoes: usize,
    /// Os filhos em cujo conteúdo projetado estamos (URI, classe): um deles
    /// injetado seria resolvido aqui mesmo, não pelo injetor de cima.
    filhos_acima: Vec<(String, String)>,
    /// `#ref` de filho `onPush`, com a visão dele: o `@ViewChild` registra
    /// o `ChangeDetectorRef` (`queryChangeDetectorRefs`).
    detectores: std::collections::HashMap<String, String>,
    /// Os nós criados sem pai (raiz da visão embutida, conteúdo projetado),
    /// como são lidos depois: `_el_3`, `this._el_3`, `this._appEl_4`.
    raizes: Vec<String>,
    /// O índice do filho cujo conteúdo projetado está sendo criado: é o pai
    /// de uma âncora solta ali (`parent.nodeIndex`).
    pai_projetado: Option<u32>,
    /// A profundidade da visão onde está o nó mais alto da cadeia de
    /// injetores de um nó desta visão (`ProviderResolver._getDependency`
    /// sobe até o filho da raiz da visão do componente; a raiz de uma visão
    /// embutida tem por pai o pai da âncora dela).
    nivel_do_topo: u32,
    /// Os nós com provedores injetáveis (`ProviderNode`), em pré-ordem:
    /// (primeiro índice, último índice da subárvore, [(tokens, campo)]).
    injetores: Vec<NoInjetor>,
}

impl Corpo<'_> {
    fn dom(&mut self) -> String {
        self.imp.alias(DOM_HELPERS)
    }

    /// Anota uma recusa. Fora do modo de coleta ela interrompe a emissão
    /// (`Err`); na coleta, fica anotada e a emissão segue.
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

    /// O arquivo do template, para o comentário `REF`. Com o template escrito
    /// na anotação a referência é outra (URI do `.dart` e deslocamento), e
    /// isso ainda não tem caso no corpus.
    fn url(&self, motivo: Motivo) -> Result<String, Recusa> {
        self.url_do_template
            .clone()
            .ok_or_else(|| recusa(motivo, "ligação em template escrito na anotação"))
    }

    /// Converte uma expressão das ligações da detecção; os locais que ela lê
    /// entram na lista da raiz, na ordem do primeiro uso.
    fn converter(
        &mut self,
        texto: &str,
        motivo: Motivo,
    ) -> Result<crate::expr::Convertida, Recusa> {
        let locais = self.locais.clone();
        let escopo = crate::expr::Escopo {
            membros: self.membros,
            metodos: self.metodos,
            aridades: self.aridades,
            locais: &locais,
            tipos: self.tipos,
        };
        let mut c = crate::expr::converter_no_escopo(texto, &escopo, self.nomes)
            .map_err(|r| r.em(motivo))?;
        if c.texto.contains(crate::expr::MARCA_DE_PIPE) {
            c.texto = self.trocar_pipes(&c.texto)?;
        }
        for l in &c.locais {
            self.declaracao_do_local(l, motivo)?;
            if !self.locais_raiz.contains(l) {
                self.locais_raiz.push(l.clone());
            }
        }
        Ok(c)
    }

    /// Troca cada marca de pipe do texto convertido pelo proxy da próxima
    /// chamada desta visão (`this._pipe_date_0_1`), conferindo nome e número
    /// de argumentos com a tabela — que tem a ordem do oficial.
    fn trocar_pipes(&mut self, texto: &str) -> Result<String, Recusa> {
        use crate::expr::{FIM_DE_PIPE, MARCA_DE_PIPE};
        let chamadas: Vec<(String, usize, String)> = self
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
        let mut saida = String::with_capacity(texto.len());
        let mut resto = texto;
        while let Some(i) = resto.find(MARCA_DE_PIPE) {
            saida.push_str(&resto[..i]);
            let depois = &resto[i + MARCA_DE_PIPE.len_utf8()..];
            let f = depois.find(FIM_DE_PIPE).unwrap_or(depois.len());
            let (nome, n) = depois[..f].rsplit_once('/').unwrap_or((&depois[..f], ""));
            match chamadas.get(self.cursor_pipe) {
                Some((esperado, argumentos, proxy))
                    if esperado == nome && argumentos.to_string() == n =>
                {
                    saida.push_str("this.");
                    saida.push_str(proxy);
                    self.cursor_pipe += 1;
                }
                // Na coleta a saída é descartada e a ordem não vale (o
                // conteúdo recusado é revisto noutro lugar).
                _ if self.coletando() => saida.push_str("this._pipe"),
                _ => {
                    return Err(recusa(
                        Motivo::PipesUsados,
                        "chamada de pipe fora da ordem do oficial",
                    ));
                }
            }
            resto = depois.get(f + FIM_DE_PIPE.len_utf8()..).unwrap_or("");
        }
        saida.push_str(resto);
        Ok(saida)
    }

    /// Toda chamada de pipe desta visão foi convertida, e na ordem.
    fn conferir_pipes(&self) -> Result<(), Recusa> {
        if !self.coletando() && self.cursor_pipe != self.pipes.da_vista(self.vista).count() {
            return Err(recusa(
                Motivo::PipesUsados,
                "chamada de pipe fora da ordem do oficial",
            ));
        }
        Ok(())
    }

    /// A declaração de um local desta visão, ou a recusa: local de visão
    /// ancestral (lido por `parentView`) e local sem tipo conhecido.
    fn declaracao_do_local(&self, nome: &str, motivo: Motivo) -> Result<String, Recusa> {
        match self.decl_locais.get(nome) {
            Some(Ok(d)) => Ok(d.clone()),
            Some(Err(r)) => Err(r.clone().em(motivo)),
            None => Err(recusa(
                motivo,
                "local de `*` ancestral lido na visão aninhada",
            )),
        }
    }

    /// `[x]="e"`: valor novo, `checkBinding` contra o anterior e a ação sobre
    /// o elemento. O nome da ligação e a URI do template vão na verificação
    /// para a mensagem de "expressão mudou depois da checagem".
    fn propriedade(&mut self, l: &crate::html::Ligacao, alvo: &str) -> Result<Ligada, Recusa> {
        let url = self.url(Motivo::Ligacao)?;
        let convertida = self.converter(&l.valor, Motivo::Ligacao)?;
        // O texto que vai no `checkBinding` é a fonte da ligação como
        // escrita (`ASTWithSource.source`), num literal Dart.
        let expr = literal(&l.valor);
        let (ini, fim) = (l.inicio, l.fim);
        // Toda ligação consome um índice (`createUniqueBindIndex` em
        // `_checkBinding`), mesmo a imutável, que não ganha campo.
        let k = self.proxima_ligacao;
        self.proxima_ligacao += 1;
        // Valor que não muda é escrito uma vez, na primeira checagem, sem
        // `checkBinding` e sem campo de valor anterior (`isImmutable`,
        // `_bindLiteral`). O que pode ser nulo (`canBeNull`: tudo menos o
        // literal) ganha um `if (x != null)` em volta; o literal `null` não
        // gera nada, forma ainda sem caso no corpus.
        if convertida.imutavel {
            if convertida.texto == "null" {
                return Err(recusa(Motivo::Ligacao, "ligação constante `null`"));
            }
            let acao = self.acao(l, alvo, &convertida.texto, &convertida)?;
            if convertida.pode_ser_nulo {
                let valor = &convertida.texto;
                return Ok(Ligada::Constante(format!(
                    "      if (({valor} != null)) {{\n        {acao} /* REF:{url}:{ini}:{fim} */;\n      }}"
                )));
            }
            return Ok(Ligada::Constante(format!(
                "      {acao} /* REF:{url}:{ini}:{fim} */;"
            )));
        }
        self.campos_expr.push(format!("  Object? _expr_{k};"));
        let acao = self.acao(l, alvo, &format!("currVal_{k}"), &convertida)?;
        let chk = tardio(CHECK_BINDING);
        let valor = convertida.texto;
        Ok(Ligada::Dinamica(format!(
            "    final currVal_{k} = {valor};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, {expr}, '{url}')) {{\n      {acao} /* REF:{url}:{ini}:{fim} */;\n      this._expr_{k} = currVal_{k};\n    }}"
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
            na_primeira_checagem(&mut self.deteccao, &constantes);
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
    ) -> Result<String, Recusa> {
        // A ação vai para o `detectChangesInternal`: o import é alocado
        // quando a detecção é escrita, não agora.
        let dom = tardio(DOM_HELPERS);
        Ok(if l.nome == "class" {
            format!("this.updateChildClass({alvo}, {valor})")
        } else if let Some(classe) = l.nome.strip_prefix("class.") {
            if classe.contains('.') {
                return Err(recusa(Motivo::Ligacao, "[class.x.y]"));
            }
            format!("{dom}.updateClassBinding({alvo}, '{classe}', {valor})")
        } else if let Some(attr) = l.nome.strip_prefix("attr.") {
            if attr.contains('.') || attr.contains(':') {
                return Err(recusa(
                    Motivo::Ligacao,
                    "[attr.x.if] ou atributo com namespace",
                ));
            }
            if com_seguranca(attr) {
                return Err(recusa(
                    Motivo::Ligacao,
                    "[attr.x] com contexto de segurança",
                ));
            }
            // `canBeNull` (`analyzed_class.dart`): só o literal não pode ser
            // nulo, e só ele vai por `setAttribute`. `a ?? b` tem regra
            // própria, ainda sem caso no corpus.
            if l.valor.contains("??") {
                return Err(recusa(Motivo::Ligacao, "[attr.x] com `??`"));
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
            if estilo.contains('.') {
                return Err(recusa(Motivo::Ligacao, "[style.x.unidade]"));
            }
            if c.tipo.as_deref() != Some("String") {
                return Err(recusa(
                    Motivo::Ligacao,
                    "[style.x] com valor que não é String",
                ));
            }
            format!("{alvo}.style.setProperty('{estilo}', {valor})")
        } else if l.nome.contains('.') {
            return Err(recusa(Motivo::Ligacao, "ligação com prefixo desconhecido"));
        } else {
            let prop = &l.nome;
            if com_seguranca(prop) || matches!(prop.as_str(), "innerHtml" | "style") {
                return Err(recusa(
                    Motivo::Ligacao,
                    "[propriedade] com contexto de segurança",
                ));
            }
            if matches!(prop.as_str(), "readonly" | "tabindex" | "tabIndex") {
                return Err(recusa(
                    Motivo::Ligacao,
                    "[propriedade] renomeada pelo esquema",
                ));
            }
            format!("{dom}.setProperty({alvo}, '{prop}', {valor})")
        })
    }

    /// Os eventos de um elemento, como `bindRenderOutputs` os liga: os do
    /// mesmo nome fundidos num handler só (`mergeEvents`, na ordem da
    /// primeira ocorrência), cada um com o seu ouvinte no fim do `build()`.
    fn eventos(&mut self, e: &crate::html::Elemento, alvo: &str) -> Result<(), Recusa> {
        let mut grupos: Vec<(&str, Vec<&str>)> = Vec::new();
        for l in &e.eventos {
            match grupos.iter_mut().find(|(n, _)| *n == l.nome) {
                // Dois `(click)` escritos no mesmo elemento são erro no
                // oficial ("Found multiple events with the same name"); a
                // fusão só acontece com os ouvintes que vêm de diretivas.
                Some(_) => {
                    return Err(recusa(
                        Motivo::Evento,
                        "dois handlers do mesmo evento no template",
                    ));
                }
                None => grupos.push((&l.nome, vec![&l.valor])),
            }
        }
        for (nome, handlers) in grupos {
            match self.handler(&handlers) {
                Ok(h) => self.ouvinte(nome, alvo, &h),
                Err(r) => self.anotar(r)?,
            }
        }
        Ok(())
    }

    /// O ouvinte de um evento: `addEventListener` do elemento para evento do
    /// DOM, o `eventManager` do ngdart para o resto (`keyup.enter`, evento
    /// próprio) — `visitNativeEvent` × `visitCustomEvent`.
    fn ouvinte(&mut self, nome: &str, alvo: &str, handler: &str) {
        let linha = if evento_nativo(nome) {
            format!("    {alvo}.addEventListener('{nome}', {handler});")
        } else {
            let utils = tardio_q(APP_VIEW_UTILS);
            format!(
                "    {utils}appViewUtils.eventManager.addEventListener({alvo}, '{nome}', {handler});"
            )
        };
        self.ouvintes.push(linha);
    }

    /// A expressão do handler de um evento (`BoundValueConverter`): um só
    /// handler simples vira tear-off (`this.eventHandler0(_ctx.m)`); o
    /// resto — atribuição, argumentos, vários handlers fundidos — vira o
    /// método `_handleEvent_N` desta visão, com `eventHandler1`.
    fn handler(&mut self, textos: &[&str]) -> Result<String, Recusa> {
        let escopo_locais = self.locais.clone();
        let escopo = crate::expr::Escopo {
            membros: self.membros,
            metodos: self.metodos,
            aridades: self.aridades,
            locais: &escopo_locais,
            tipos: self.tipos,
        };
        let mut acoes = Vec::new();
        for t in textos {
            acoes.push(
                crate::expr::converter_acao(t, &escopo, self.nomes)
                    .map_err(|r| r.em(Motivo::Evento))?,
            );
        }
        if let [
            crate::expr::Acao::Simples {
                metodo, aridade, ..
            },
        ] = acoes.as_slice()
        {
            self.usa_ctx_no_build = true;
            return Ok(format!("this.eventHandler{aridade}(_ctx.{metodo})"));
        }
        // Complexo: as instruções, com os locais que elas leem declarados
        // no topo do método (o escopo do `scopeNamespace`).
        let mut instrucoes = Vec::new();
        let mut locais: Vec<String> = Vec::new();
        for a in &acoes {
            match a {
                crate::expr::Acao::Simples { instrucao, .. } => instrucoes.push(instrucao.clone()),
                crate::expr::Acao::Complexa(c) => {
                    instrucoes.push(c.texto.clone());
                    for l in &c.locais {
                        if !locais.contains(l) {
                            locais.push(l.clone());
                        }
                    }
                }
            }
        }
        let mut corpo = Vec::new();
        for l in &locais {
            corpo.push(format!(
                "    {}",
                self.declaracao_do_local(l, Motivo::Evento)?
            ));
        }
        if cita_ctx(&instrucoes) {
            corpo.push("    final _ctx = this.ctx;".to_string());
        }
        corpo.extend(instrucoes.iter().map(|i| format!("    {i};")));
        let n = self.metodos_evento.len();
        self.metodos_evento.push(format!(
            "\n  void _handleEvent_{n}($event) {{\n{}\n  }}\n",
            corpo.join("\n")
        ));
        Ok(format!("this.eventHandler1(this._handleEvent_{n})"))
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
    ) -> Result<(), Recusa> {
        let em_filho = |f: &str| recusa(Motivo::LigacaoEmFilho, f);
        if !e.bananas.is_empty() {
            return Err(em_filho("[(x)] no filho"));
        }
        if let Some(r) = filho.pendencias.first() {
            return Err(r.clone());
        }
        // `#ref` no filho vale a instância; só na forma que não muda nada
        // no nó e que nenhuma expressão lê (o `@ViewChild` a recebe).
        for r in &e.referencias {
            if !r.valor.is_empty() {
                return Err(em_filho("#ref com valor no filho"));
            }
            if !self.refs_livres.contains(&r.nome) {
                return Err(em_filho(if self.embutida {
                    "#ref no filho em visão embutida"
                } else {
                    "#ref no filho usado em expressão"
                }));
            }
        }
        for a in &e.atributos {
            if a.valor.contains("{{") {
                return Err(em_filho("atributo interpolado no filho"));
            }
            if a.nome == "style" {
                return Err(recusa(Motivo::EstiloEmLinha, "style=\"...\" em linha"));
            }
            // Sem valor (`<x disabled>`) o oficial liga um `EmptyExpr`, e
            // `x=""` não se distingue daqui: nenhum dos dois ainda.
            if filho.entrada(&a.nome).is_some() && a.valor.is_empty() {
                return Err(em_filho("atributo sem valor em @Input do filho"));
            }
        }
        // O que chega a um `@Input`: atributo estático (literal) e `[x]`. Um
        // nome ligado duas vezes some no oficial (`_removeExisting`).
        let mut ligadas: Vec<(&crate::html::Ligacao, bool)> = e
            .atributos
            .iter()
            .filter(|a| filho.entrada(&a.nome).is_some())
            .map(|a| (a, true))
            .collect();
        ligadas.extend(e.propriedades.iter().map(|l| (l, false)));
        for (i, (l, _)) in ligadas.iter().enumerate() {
            if ligadas[..i].iter().any(|(x, _)| x.nome == l.nome) {
                return Err(em_filho("@Input do filho ligado duas vezes"));
            }
        }
        let n = self.proximo;
        self.proximo += 1;
        let fora_de_lib = || recusa(Motivo::ComponenteNoTemplate, "filho sem caminho de import");
        let cam_template = caminho_do_import(
            &self.asset,
            &asset_de_uri(&filho.uri_template, "", Path::new("")).ok_or_else(fora_de_lib)?,
        )
        .ok_or_else(fora_de_lib)?;
        let cam_dart = caminho_do_import(
            &self.asset,
            &asset_de_uri(&filho.uri_dart, "", Path::new("")).ok_or_else(fora_de_lib)?,
        )
        .ok_or_else(fora_de_lib)?;
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
        // Na raiz da visão embutida, ou projetado, o nó não tem pai aqui.
        if pai.is_empty() {
            self.raizes.push(format!("_el_{n}"));
        } else {
            self.linhas.push(format!("    {pai}.append(_el_{n});"));
        }
        // Os atributos escritos, em ordem alfabética (`_toSortedBindings`),
        // todos — também os que alimentam um `@Input` —, e o `class` pelo
        // `updateChildClassNonHtml`: o elemento do filho não é HTML
        // (`writeLiteralAttributeValues`).
        let mut atributos = e.atributos.clone();
        atributos.sort_by(|a, b| a.nome.cmp(&b.nome));
        for a in &atributos {
            let valor = literal(&a.valor);
            if a.nome == "class" {
                self.linhas.push(format!(
                    "    this.updateChildClassNonHtml(_el_{n}, {valor});"
                ));
            } else {
                let dom = self.dom();
                self.linhas.push(format!(
                    "    {dom}.setAttribute(_el_{n}, '{}', {valor});",
                    a.nome
                ));
            }
        }
        if self.com_estilo {
            self.linhas.push(format!("    this.addShimC(_el_{n});"));
        }
        let construcao = self.construcao_do_filho(filho, n, &vd, &campo_vista)?;
        self.linhas
            .push(format!("    this.{campo_inst} = {construcao};"));
        for r in &e.referencias {
            self.refs
                .insert(r.nome.clone(), format!("this.{campo_inst}"));
            if filho.on_push {
                self.detectores.insert(r.nome.clone(), campo_vista.clone());
            }
        }
        self.entradas_do_filho(&ligadas, filho, &campo_inst, &campo_vista)?;
        self.saidas_do_filho(e, filho, n, &campo_inst)?;
        if filho.projecoes.is_empty() {
            if !e.filhos.is_empty() {
                return Err(recusa(
                    Motivo::Projecao,
                    "conteúdo em filho que não projeta",
                ));
            }
            self.consultas_do_filho(e, filho, &campo_inst)?;
            self.depois_dos_filhos(filho, &campo_inst);
            self.linhas
                .push(format!("    this.{campo_vista}.create(this.{campo_inst});"));
            return Ok(());
        }
        // Conteúdo projetado: os nós são criados soltos e cada um vai para a
        // projeção cujo seletor casa com ele (`findNgContentIndex`: o menor
        // índice que casa, senão o `*`).
        let seletores: Vec<Option<Vec<crate::seletor::Seletor>>> = filho
            .projecoes
            .iter()
            .map(|s| (s != "*").then(|| crate::seletor::Seletor::analisar(s)))
            .collect();
        let curinga = filho.projecoes.iter().position(|s| s == "*");
        let mut listas: Vec<Vec<String>> = vec![Vec::new(); filho.projecoes.len()];
        self.filhos_acima
            .push((filho.uri_dart.clone(), filho.classe.clone()));
        let pai_antes = self.pai_projetado.replace(n);
        let mut r = Ok(());
        for no in &e.filhos {
            let indice = match no {
                No::Comentario(_) => continue,
                No::Elemento(x) => {
                    let mut sem_estrela = x.clone();
                    sem_estrela.estrela = None;
                    let el = crate::seletor::Elemento::do_template(&sem_estrela);
                    seletores
                        .iter()
                        .position(|s| {
                            s.as_ref()
                                .is_some_and(|s| crate::seletor::casa_algum(s, &el))
                        })
                        .or(curinga)
                }
                _ => curinga,
            };
            let Some(indice) = indice else {
                r = Err(recusa(
                    Motivo::Projecao,
                    "conteúdo que nenhuma projeção recebe",
                ));
                break;
            };
            let antes = self.raizes.len();
            if let Err(x) = self.nos(std::slice::from_ref(no), "") {
                r = Err(x);
                break;
            }
            listas[indice].extend(self.raizes.drain(antes..));
        }
        self.pai_projetado = pai_antes;
        self.filhos_acima.pop();
        r?;
        self.consultas_do_filho(e, filho, &campo_inst)?;
        self.depois_dos_filhos(filho, &campo_inst);
        // Todas vazias: uma linha só, constantes. Senão, uma lista por
        // linha (o `dart format` quebra a lista que tem outra não vazia).
        let texto = if listas.iter().all(Vec::is_empty) {
            let vazias = vec!["const <Object>[]"; listas.len()];
            format!(
                "    this.{campo_vista}.createAndProject(this.{campo_inst}, [{}]);",
                vazias.join(", ")
            )
        } else {
            let linhas: Vec<String> = listas
                .iter()
                .map(|l| {
                    if l.is_empty() {
                        "      const <Object>[]".to_string()
                    } else {
                        format!("      <Object>[{}]", l.join(", "))
                    }
                })
                .collect();
            format!(
                "    this.{campo_vista}.createAndProject(this.{campo_inst}, [\n{}\n    ]);",
                linhas.join(",\n")
            )
        };
        self.linhas.push(texto);
        Ok(())
    }

    /// A construção da instância do filho, texto depois de `this._X_n_5 = `
    /// (sem o `;`): o nó, a visão do filho e os serviços pela visão de cima.
    /// Com serviço, o oficial embrulha em `debugInjectorWrap` sob
    /// `isDevMode`, como na hospedeira.
    fn construcao_do_filho(
        &mut self,
        filho: &Filho,
        n: u32,
        vd: &str,
        campo_vista: &str,
    ) -> Result<String, Recusa> {
        let em_filho = |f: &str| recusa(Motivo::LigacaoEmFilho, f);
        let classe = &filho.classe;
        let injeta = filho
            .parametros
            .iter()
            .any(|p| matches!(p, Injetado::Servico { .. }));
        // A ordem dos imports é a da escrita: `isDevMode`, `errors.dart`, a
        // classe e os tipos injetados.
        let prefixo = if injeta {
            let util = self.imp.alias(UTILITIES);
            let erros = self.imp.alias(DI_ERRORS);
            Some((util, erros))
        } else {
            None
        };
        // `injectFromViewParentInjector` escrito na visão do nó e levado
        // (`getPropertyInView`) à visão do nó mais alto da cadeia de
        // injetores: `parentView.injectorGet(T, parentIndex)` visto de lá.
        let saltos = self.profundidade - self.nivel_do_topo;
        let visao_do_componente = if saltos == 0 {
            "this".to_string()
        } else {
            let mut v = "(this.parentView!)".to_string();
            for _ in 1..saltos {
                v = format!("({v}.parentView!)");
            }
            v
        };
        let mut args = Vec::new();
        for p in &filho.parametros {
            match p {
                Injetado::Elemento => args.push(format!("_el_{n}")),
                Injetado::Detector => args.push(format!("this.{campo_vista}")),
                Injetado::Servico {
                    uri,
                    classe: tipo,
                    opcional,
                } => {
                    if self.filhos_acima.iter().any(|(u, c)| u == uri && c == tipo) {
                        return Err(em_filho("filho que injeta um componente acima dele"));
                    }
                    let caminho = asset_de_uri(uri, "", Path::new(""))
                        .and_then(|alvo| caminho_do_import(&self.asset, &alvo))
                        .ok_or_else(|| em_filho("tipo injetado no filho sem caminho de import"))?;
                    let q = self.imp.q(&caminho);
                    let metodo = if *opcional {
                        "injectorGetOptional"
                    } else {
                        "injectorGet"
                    };
                    let v = &visao_do_componente;
                    args.push(format!(
                        "({v}.parentView!).{metodo}({q}{tipo}, {v}.parentIndex)"
                    ));
                }
            }
        }
        let chamada = format!("{vd}.{classe}({})", args.join(", "));
        Ok(match prefixo {
            None => chamada,
            Some((util, erros)) => format!(
                "({util}.isDevMode\n        ? {erros}.debugInjectorWrap({vd}.{classe}, () {{\n            return {chamada};\n          }})\n        : {chamada})"
            ),
        })
    }

    /// As consultas de conteúdo do filho, no `afterChildren` do nó
    /// (`updateQueryAtStartup`): sem resultado, a lista recebe `[]` e a
    /// única nada. Com resultado no conteúdo — um filho da classe procurada,
    /// o `#ref` procurado —, a forma é outra, ainda recusada.
    fn consultas_do_filho(
        &mut self,
        e: &crate::html::Elemento,
        filho: &Filho,
        campo_inst: &str,
    ) -> Result<(), Recusa> {
        fn acha(
            nos: &[No],
            alvo: &AlvoDeConsulta,
            filhos: &std::collections::HashMap<String, Filho>,
        ) -> bool {
            nos.iter().any(|n| {
                let No::Elemento(x) = n else { return false };
                let aqui = match alvo {
                    AlvoDeConsulta::Referencia(r) => x.referencias.iter().any(|y| &y.nome == r),
                    AlvoDeConsulta::Classe(uri, classe) => filhos
                        .get(&x.nome)
                        .is_some_and(|f| &f.uri_dart == uri && &f.classe == classe),
                };
                aqui || acha(&x.filhos, alvo, filhos)
            })
        }
        for (campo, lista, alvo) in &filho.consultas {
            if acha(&e.filhos, alvo, self.filhos) {
                return Err(recusa(
                    Motivo::LigacaoEmFilho,
                    "@ContentChild do filho com resultado no conteúdo",
                ));
            }
            if *lista {
                self.linhas
                    .push(format!("    this.{campo_inst}.{campo} = [];"));
            }
        }
        Ok(())
    }

    /// Os ganchos que o oficial liga depois de visitar o conteúdo do filho
    /// (`bindDirectiveAfterChildrenCallbacks`): `ngAfterContent*`,
    /// `ngAfterView*` e `ngOnDestroy`.
    fn depois_dos_filhos(&mut self, filho: &Filho, campo_inst: &str) {
        let g = &filho.ganchos;
        if g.after_content_init {
            self.usa_primeira_checagem = true;
            na_primeira_checagem(
                &mut self.apos_conteudo,
                &[format!("      this.{campo_inst}.ngAfterContentInit();")],
            );
        }
        if g.after_content_checked {
            self.apos_conteudo
                .push(format!("    this.{campo_inst}.ngAfterContentChecked();"));
        }
        if g.after_view_init {
            self.usa_primeira_checagem = true;
            na_primeira_checagem(
                &mut self.apos_visao,
                &[format!("      this.{campo_inst}.ngAfterViewInit();")],
            );
        }
        if g.after_view_checked {
            self.apos_visao
                .push(format!("    this.{campo_inst}.ngAfterViewChecked();"));
        }
        if g.on_destroy {
            self.destruir
                .push(format!("    this.{campo_inst}.ngOnDestroy();"));
        }
    }

    /// As entradas de um filho (`bindDirectiveInputs`) e os ganchos da
    /// detecção dele (`bindDirectiveDetectChangesLifecycleCallbacks`), no
    /// `detectChangesInInputsMethod`.
    ///
    /// As ligações saem na ordem em que o filho declara os `@Input`
    /// (`_SortInputsVisitor`); as constantes (atributo estático, expressão
    /// imutável) juntas num `if (firstCheck)` antes das outras
    /// (`bindAndWriteToRenderer`), cada uma consumindo o seu índice. Filho
    /// `onPush` (ou com `AfterChanges` e alguma entrada ligada) calcula
    /// `changed`.
    fn entradas_do_filho(
        &mut self,
        ligadas: &[(&crate::html::Ligacao, bool)],
        filho: &Filho,
        campo_inst: &str,
        campo_vista: &str,
    ) -> Result<(), Recusa> {
        let spec: Vec<(String, String, Option<bool>)> = filho
            .entradas
            .iter()
            .map(|e| (e.nome.clone(), e.campo.clone(), None))
            .collect();
        let vista = filho.on_push.then_some(campo_vista);
        self.entradas_de(
            ligadas,
            &spec,
            campo_inst,
            vista,
            &filho.ganchos,
            Motivo::LigacaoEmFilho,
        )
    }

    /// `bindDirectiveInputs` e `bindDirectiveDetectChangesLifecycleCallbacks`
    /// de uma diretiva ou componente: `spec` são os `@Input` declarados
    /// (nome, membro, se é `bool` quando se sabe), `on_push` a visão do
    /// componente `onPush` que marca a checagem.
    fn entradas_de(
        &mut self,
        ligadas: &[(&crate::html::Ligacao, bool)],
        spec: &[(String, String, Option<bool>)],
        campo_inst: &str,
        on_push: Option<&str>,
        g: &crate::componente::Ganchos,
        motivo: Motivo,
    ) -> Result<(), Recusa> {
        let dbg = tardio(CHECK_BINDING);
        // `optimizeLifecycles`: sem entrada ligada, o `ngAfterChanges` some.
        let after_changes = g.after_changes && !ligadas.is_empty();
        // `if (!directive.hasInputs) return;`: sem `@Input` declarado, nada
        // de entradas nem de `changed`.
        if !spec.is_empty() {
            let url = if ligadas.is_empty() {
                String::new()
            } else {
                self.url(motivo)?
            };
            let calcula = on_push.is_some() || after_changes;
            if calcula {
                self.usa_changed = true;
                self.entradas.push("    changed = false;".to_string());
            }
            let mut ordem: Vec<&(&crate::html::Ligacao, bool)> = ligadas.iter().collect();
            ordem.sort_by_key(|(l, _)| {
                spec.iter()
                    .position(|x| x.0 == l.nome)
                    .unwrap_or(usize::MAX)
            });
            let dev = tardio(DEVTOOLS);
            let mut constantes = Vec::new();
            let mut dinamicas = Vec::new();
            for (l, estatico) in ordem {
                let Some((_, campo, booleana)) = spec.iter().find(|x| x.0 == l.nome).cloned()
                else {
                    // Nome que o filho não declara como `@Input`: pode ser
                    // diretiva.
                    return Err(recusa(motivo, "[x] que não é @Input do filho"));
                };
                let nome = &l.nome;
                let (ini, fim) = (l.inicio, l.fim);
                let k = self.proxima_ligacao;
                self.proxima_ligacao += 1;
                let mudou = if calcula { "\nchanged = true;" } else { "" };
                // Atributo estático: `LiteralPrimitive` do texto; sem valor,
                // `EmptyExpr` (`true` numa entrada `bool`, senão `''`).
                // Expressão imutável: a mesma forma (`_bindLiteral`).
                let (valor, nulo) = if *estatico {
                    if sem_valor(l) {
                        match booleana {
                            Some(true) => ("true".to_string(), false),
                            Some(false) => ("''".to_string(), false),
                            None => {
                                return Err(recusa(
                                    motivo,
                                    "atributo sem valor em @Input do filho",
                                ));
                            }
                        }
                    } else {
                        (literal(&l.valor), false)
                    }
                } else {
                    let c = self.converter(&l.valor, motivo)?;
                    if !c.imutavel {
                        let expr = literal(&l.valor);
                        let chk = tardio(CHECK_BINDING);
                        let valor = c.texto;
                        let mudou = if calcula {
                            "\n      changed = true;"
                        } else {
                            ""
                        };
                        self.campos_expr.push(format!("  Object? _expr_{k};"));
                        dinamicas.push(format!(
                            "    final currVal_{k} = {valor};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, {expr}, '{url}')) {{\n      if ({dev}.isDevToolsEnabled) {{\n        {dev}.Inspector.instance.recordInput(this.{campo_inst}, '{nome}', currVal_{k});\n      }}\n      this.{campo_inst}.{campo} = currVal_{k} /* REF:{url}:{ini}:{fim} */;{mudou}\n      this._expr_{k} = currVal_{k};\n    }}"
                        ));
                        continue;
                    }
                    (c.texto, c.pode_ser_nulo)
                };
                if valor == "null" {
                    continue;
                }
                let mut bloco = format!(
                    "if ({dev}.isDevToolsEnabled) {{\n  {dev}.Inspector.instance.recordInput(this.{campo_inst}, '{nome}', {valor});\n}}\nthis.{campo_inst}.{campo} = {valor} /* REF:{url}:{ini}:{fim} */;{mudou}"
                );
                if nulo {
                    bloco = format!("if (({valor} != null)) {{\n{}\n}}", indentar(&bloco, 2));
                }
                constantes.push(indentar(&bloco, 6));
            }
            if !constantes.is_empty() {
                self.usa_primeira_checagem = true;
                na_primeira_checagem(&mut self.entradas, &constantes);
            }
            self.entradas.extend(dinamicas);
            if let Some(vista) = on_push {
                self.entradas.push(format!(
                    "    if (changed) {{\n      this.{vista}.markAsCheckOnce();\n    }}"
                ));
            }
        }
        if after_changes {
            self.usa_changed = true;
            self.entradas.push(format!(
                "    if (changed) {{\n      this.{campo_inst}.ngAfterChanges();\n    }}"
            ));
        }
        if g.on_init {
            self.usa_primeira_checagem = true;
            self.entradas.push(format!(
                "    if (((!{dbg}.debugThrowIfChanged) && firstCheck)) {{\n      this.{campo_inst}.ngOnInit();\n    }}"
            ));
        }
        if g.do_check {
            self.entradas.push(format!(
                "    if ((!{dbg}.debugThrowIfChanged)) {{\n      this.{campo_inst}.ngDoCheck();\n    }}"
            ));
        }
        Ok(())
    }

    /// Os eventos escritos no elemento do filho: os que casam um `@Output`
    /// viram `subscription_N` (`bindDirectiveOutputs`, depois dos do
    /// elemento); os outros são eventos do elemento (`bindRenderOutputs`).
    fn saidas_do_filho(
        &mut self,
        e: &crate::html::Elemento,
        filho: &Filho,
        n: u32,
        campo_inst: &str,
    ) -> Result<(), Recusa> {
        let mut do_elemento = e.clone();
        do_elemento
            .eventos
            .retain(|l| filho.saida(&l.nome).is_none());
        self.eventos(&do_elemento, &format!("_el_{n}"))?;
        let mut vistos: Vec<&str> = Vec::new();
        for l in e.eventos.iter().filter(|l| filho.saida(&l.nome).is_some()) {
            if vistos.contains(&l.nome.as_str()) {
                return Err(recusa(
                    Motivo::Evento,
                    "dois handlers do mesmo evento no template",
                ));
            }
            vistos.push(&l.nome);
            let membro = filho.saida(&l.nome).unwrap_or_default().to_string();
            match self.handler(&[&l.valor]) {
                Ok(h) => {
                    let k = self.subscricoes;
                    self.subscricoes += 1;
                    self.ouvintes.push(format!(
                        "    final subscription_{k} = this.{campo_inst}.{membro}.listen({h});"
                    ));
                }
                Err(r) => self.anotar(r)?,
            }
        }
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
    ) -> Result<(), Recusa> {
        let url = self.url(Motivo::Ligacao)?;
        let Some(dir) = Estrutural::conhecida(&estrela.nome) else {
            return Err(recusa(
                Motivo::Ligacao,
                "`*` de diretiva que não é ngIf/ngFor",
            ));
        };
        let micro = crate::micro::analisar(&estrela.nome, &estrela.valor);
        self.guarda_do_template(&estrela.nome, &micro, Some(&dir))?;
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
        // O segundo argumento é o índice do elemento pai, e `null` quando a
        // âncora está na raiz da visão (`isRootElement ? null :
        // parent.nodeIndex`, em `compile_element.dart`); solta no conteúdo
        // projetado, o pai é o filho que a recebe.
        let pai_indice = if pai.is_empty() {
            self.raizes.push(format!("this._appEl_{n}"));
            self.linhas
                .push(format!("    final _anchor_{n} = {dom}.createAnchor();"));
            self.pai_projetado
                .map_or_else(|| "null".to_string(), |k| k.to_string())
        } else {
            self.linhas.push(format!(
                "    final _anchor_{n} = {dom}.appendAnchor({pai});"
            ));
            indice_do_elemento(pai)
        };
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
            let c = self.converter(expr, Motivo::Ligacao)?;
            if prop.ends_with("Of") {
                tipo_da_colecao = c.tipo.clone().map(|t| (t, c.escopo.clone()));
            }
            // Entrada imutável (`final List<X> itens`, `*ngIf="fixo"`) é
            // escrita uma vez, no `if (firstCheck)` das entradas: direto no
            // `NgIf` (`_directBinding` no método das constantes); pelo
            // `_bindLiteral` no `NgFor`, que consome o índice da ligação e
            // põe o `if (x != null)` em volta do que pode ser nulo.
            if c.imutavel {
                let (ini, fim) = (estrela.inicio, estrela.fim);
                let valor = &c.texto;
                let dev_rec = format!(
                    "if ({dev}.isDevToolsEnabled) {{\n  {dev}.Inspector.instance.recordInput(this.{campo}, '{prop}', {valor});\n}}"
                );
                let atrib = format!("this.{campo}.{prop} = {valor} /* REF:{url}:{ini}:{fim} */;");
                let mut instrucoes = vec![dev_rec, atrib];
                if !dir.direta {
                    self.proxima_ligacao += 1;
                    if valor == "null" {
                        continue;
                    }
                    if c.pode_ser_nulo {
                        let dentro: Vec<String> =
                            instrucoes.iter().map(|i| indentar(i, 2)).collect();
                        instrucoes = vec![format!(
                            "if (({valor} != null)) {{\n{}\n}}",
                            dentro.join("\n")
                        )];
                    }
                }
                let blocos: Vec<String> = instrucoes.iter().map(|i| indentar(i, 6)).collect();
                self.usa_primeira_checagem = true;
                na_primeira_checagem(&mut self.entradas, &blocos);
                continue;
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
        let locais = self.locais_da_micro(&micro, tipo_da_colecao.as_ref())?;
        let mut sem_estrela = e.clone();
        sem_estrela.estrela = None;
        // Para a visão nova, os locais desta visão e das ancestrais ficam um
        // `parentView` mais longe.
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
        for (nome, chave) in &self.locais_proprios {
            ancestrais.insert(
                nome.clone(),
                Origem {
                    chave: chave.clone(),
                    classe: self.classe_desta.clone(),
                    niveis: 1,
                },
            );
        }
        // A âncora na raiz desta visão tem por pai o mesmo nó que a raiz
        // desta visão; aninhada, um nó desta visão.
        let no_topo = pai == "parentRenderNode" || (pai.is_empty() && self.pai_projetado.is_none());
        let nivel_do_topo = if !no_topo || self.nivel_do_topo < self.profundidade {
            self.nivel_do_topo
        } else {
            self.profundidade + 1
        };
        self.embutidas.push(EspecEmbutida {
            indice,
            profundidade: self.profundidade + 1,
            nivel_do_topo,
            classe: format!("_{}{indice}", self.classe_da_visao),
            fabrica: nome_fabrica,
            nos: vec![No::Elemento(sem_estrela)],
            locais,
            micro,
            ancestrais,
        });
        Ok(())
    }

    /// Os locais que o `*` põe no escopo da visão embutida, com o tipo:
    /// `$implicit` do elemento da coleção, `index`/`count` inteiros e os
    /// quatro booleanos do `NgFor`.
    fn locais_da_micro(
        &self,
        micro: &crate::micro::Micro,
        tipo_da_colecao: Option<&(String, Option<std::path::PathBuf>)>,
    ) -> Result<std::collections::HashMap<String, crate::expr::Local>, Recusa> {
        let mut locais = self.locais.clone();
        for (nome, chave) in &micro.locais {
            let (tipo, escopo) = match chave.as_str() {
                "$implicit" => {
                    let Some((t, escopo)) = tipo_da_colecao
                        .and_then(|(t, e)| tipo_do_elemento(t).map(|x| (x, e.clone())))
                    else {
                        return Err(recusa(
                            Motivo::Ligacao,
                            "local de `*ngFor` sem o tipo do elemento",
                        ));
                    };
                    (t, escopo)
                }
                "index" | "count" => ("int".to_string(), None),
                "first" | "last" | "even" | "odd" => ("bool".to_string(), None),
                _ => {
                    return Err(recusa(
                        Motivo::Ligacao,
                        "local de `*` com chave desconhecida",
                    ));
                }
            };
            locais.insert(
                nome.clone(),
                crate::expr::Local {
                    dart: format!("local_{nome}"),
                    tipo,
                    escopo,
                },
            );
        }
        Ok(locais)
    }

    /// Guarda das diretivas do `<template>` que o `*` cria: tem de casar
    /// exatamente a diretiva estrutural que o emissor conhece, e nenhuma
    /// outra (`_templateSelector`: `template`, os atributos e as
    /// propriedades da microssintaxe).
    fn guarda_do_template(
        &self,
        nome: &str,
        micro: &crate::micro::Micro,
        dir: Option<&Estrutural>,
    ) -> Result<(), Recusa> {
        let mut desc = crate::seletor::Elemento::novo("template");
        if !micro.propriedades.iter().any(|(p, _)| p == nome) {
            desc.atributo(nome, None);
        }
        for (p, v) in &micro.propriedades {
            desc.atributo(p, Some(v));
        }
        let casadas: Vec<&Usada> = self
            .usadas
            .iter()
            .filter(|u| crate::seletor::casa_algum(&u.seletores, &desc))
            .collect();
        match (dir, casadas.as_slice()) {
            (Some(d), [u]) if u.e_estrutural(d) => Ok(()),
            (_, []) => Err(recusa(
                Motivo::DiretivaPorSeletor,
                "`*` sem diretiva de directives: que case",
            )),
            _ => {
                let outra = casadas
                    .iter()
                    .find(|u| dir.is_none_or(|d| !u.e_estrutural(d)))
                    .map(|u| u.classe.clone())
                    .unwrap_or_default();
                Err(recusa(
                    Motivo::DiretivaPorSeletor,
                    format!("diretiva {outra} em <template>"),
                ))
            }
        }
    }

    /// Guarda das diretivas de um elemento: nenhuma diretiva de
    /// `directives:` pode casar com ele, a não ser o componente que o emissor
    /// vai instanciar pela tag.
    fn guarda_do_elemento(&self, e: &crate::html::Elemento) -> Result<(), Recusa> {
        if self.usadas.is_empty() {
            return Ok(());
        }
        let desc = crate::seletor::Elemento::do_template(e);
        let filho = self.filhos.get(&e.nome);
        for u in self.usadas {
            if !crate::seletor::casa_algum(&u.seletores, &desc) {
                continue;
            }
            let e_o_filho = match (&u.filho, filho) {
                (Some(f), Some(g)) => f.classe == g.classe && f.uri_dart == g.uri_dart,
                _ => false,
            };
            if e_o_filho {
                continue;
            }
            if u.filho.is_some() {
                return Err(recusa(
                    Motivo::DiretivaPorSeletor,
                    format!("componente {} por seletor composto", u.classe),
                ));
            }
            // Diretiva num elemento HTML: o emissor a instancia se os
            // metadados lidos do programa não têm nada fora do que ele
            // escreve (`Diretiva::pendencia`).
            let pendencia = if filho.is_some() || !dom::tag_html(&e.nome) {
                Some("em elemento que não é HTML".to_string())
            } else {
                match &u.diretiva {
                    None => Some("sem metadados".to_string()),
                    Some(d) => d.pendencia(),
                }
            };
            if let Some(p) = pendencia {
                return Err(recusa(
                    Motivo::DiretivaPorSeletor,
                    format!("diretiva {} ({p})", u.classe),
                ));
            }
        }
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
    ) -> Result<(), Recusa> {
        let url = self.url(Motivo::Interpolacao)?;
        let Some(tb) = self.tb.clone() else {
            return Err(recusa(
                Motivo::Interpolacao,
                "interpolação sem ligação de texto",
            ));
        };
        // Texto solto entregue a um filho (`createText`, sem pai): forma
        // ainda sem caso no corpus.
        if pai.is_empty() {
            return Err(recusa(Motivo::Projecao, "interpolação projetada solta"));
        }
        let convertida = self.converter(expr, Motivo::Interpolacao)?;
        // Sem o tipo estático não dá para escolher entre `interpolateString`,
        // `interpolate` e `updateTextWithPrimitive` — e escolher errado muda o
        // que o programa faz.
        let Some(tipo) = convertida.tipo.clone() else {
            return Err(recusa(
                Motivo::Interpolacao,
                format!("tipo desconhecido de {}", convertida.forma),
            ));
        };
        let nu = tipo.trim_end_matches('?').to_string();
        let n = self.proximo;
        self.proximo += 1;
        // `visitInterpolation` com as pontas vazias: o primitivo mutável é
        // lido direto (`_isPrimitiveCheck`), o literal vira o próprio texto
        // (`o.literal('${valor}')`), o resto passa por `interpolateString0`
        // (tudo `String`) ou `interpolate0`. O tipo é o do `_TypeResolver`:
        // ternário, `!x`, `x!`, `??`, índice e binário que não soma dois
        // `String` são `dynamic`.
        let primitivo_mutavel = !convertida.imutavel && primitivo(&nu);
        let f = if nu == "String" {
            "interpolateString0"
        } else {
            "interpolate0"
        };
        if convertida.imutavel {
            // Valor que não muda não tem ligação: o texto é calculado uma vez,
            // no `build()`, como o oficial faz (`isImmutable`).
            // O emissor escreve `appendText(` antes do valor: o import do
            // `dom_helpers` vem primeiro.
            let dom = self.dom();
            let valor = if convertida.literal {
                valor_de_literal(&convertida.texto)
            } else {
                let alias = self.imp.alias(INTERPOLATE);
                format!("{alias}.{f}({})", convertida.texto)
            };
            if cita_ctx(std::slice::from_ref(&valor)) {
                self.usa_ctx_no_build = true;
            }
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
        let acesso = convertida.texto;
        let atualizacao = if primitivo_mutavel {
            format!("updateTextWithPrimitive({acesso})")
        } else {
            // Na detecção o import é do momento em que ela é escrita.
            format!("updateText({}.{f}({acesso}))", tardio(INTERPOLATE))
        };
        self.deteccao.push(format!(
            "    this._textBinding_{n}.{atualizacao} /* REF:{url}:{inicio}:{fim} */;"
        ));
        Ok(())
    }

    /// Um atributo com `{{ }}` (`title="a {{b}}"`): o oficial o trata como
    /// ligação de propriedade (`_createPropertyForAttribute`), com o nome
    /// passado pelo esquema (`class` → `className`, que vira
    /// `updateChildClass`) e o valor `Interpolation(strings, exprs)`. Sai
    /// depois das ligações `[x]` do elemento.
    fn atributo_interpolado(
        &mut self,
        a: &crate::html::Ligacao,
        alvo: &str,
    ) -> Result<Ligada, Recusa> {
        let url = self.url(Motivo::Interpolacao)?;
        let nome = a.nome.as_str();
        // Nome que o esquema renomeia ou protege (`readonly`, `href`…), ou
        // que nem é propriedade do DOM (`data-x`, `aria-x`: o oficial acusa
        // erro), fica de fora.
        if nome != "class"
            && (!nome.chars().all(|c| c.is_ascii_lowercase())
                || com_seguranca(nome)
                || matches!(nome, "style" | "readonly" | "tabindex" | "for"))
        {
            return Err(recusa(
                Motivo::Interpolacao,
                "atributo interpolado renomeado, protegido ou fora do esquema",
            ));
        }
        if a.valor.contains('&') {
            return Err(recusa(
                Motivo::Interpolacao,
                "atributo interpolado com entidade HTML",
            ));
        }
        let (textos, exprs) = partes_da_interpolacao(&a.valor)
            .ok_or_else(|| recusa(Motivo::Interpolacao, "atributo interpolado mal formado"))?;
        if exprs.len() > 2 {
            return Err(recusa(
                Motivo::Interpolacao,
                "atributo com 3+ interpolações (`interpolateFallback`)",
            ));
        }
        let mut convertidas = Vec::new();
        for e in &exprs {
            convertidas.push(self.converter(e, Motivo::Interpolacao)?);
        }
        let mut tipos = Vec::new();
        for c in &convertidas {
            let Some(t) = &c.tipo else {
                return Err(recusa(
                    Motivo::Interpolacao,
                    format!("tipo desconhecido de {}", c.forma),
                ));
            };
            tipos.push(t.trim_end_matches('?').to_string());
        }
        // `_compressWhitespacePreceding`/`Following`: só as pontas, e só
        // quando há quebra de linha.
        let n_textos = textos.len();
        let textos: Vec<String> = textos
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let t = if i == 0 {
                    comprimir_antes(t)
                } else if i == n_textos - 1 {
                    comprimir_depois(t)
                } else {
                    t.replace(NGSP, " ")
                };
                literal(&t)
            })
            .collect();
        let so_string = tipos.iter().all(|t| t == "String");
        let familia = if so_string {
            "interpolateString"
        } else {
            "interpolate"
        };
        let imutavel = convertidas.iter().all(|c| c.imutavel);
        let k = self.proxima_ligacao;
        self.proxima_ligacao += 1;
        let interp = tardio(INTERPOLATE);
        // O valor da interpolação com cada expressão no lugar.
        let valor_com = |exprs: &[String], familia: &str| -> String {
            match exprs {
                [e] if textos[0] == "''" && textos[1] == "''" => {
                    format!("{interp}.{familia}0({e})")
                }
                [e] => format!("{interp}.{familia}1({}, {e}, {})", textos[0], textos[1]),
                [e0, e1] => format!(
                    "{interp}.{familia}2({}, {e0}, {}, {e1}, {})",
                    textos[0], textos[1], textos[2]
                ),
                _ => String::new(),
            }
        };
        let textos_expr: Vec<String> = convertidas.iter().map(|c| c.texto.clone()).collect();
        let simulada = crate::html::Ligacao {
            nome: a.nome.clone(),
            valor: a.valor.clone(),
            inicio: a.inicio,
            fim: a.fim,
        };
        let (ini, fim) = (a.inicio, a.fim);
        // Uma expressão literal com as pontas vazias é o próprio texto.
        let literal_puro = convertidas.len() == 1
            && convertidas[0].literal
            && textos[0] == "''"
            && textos[1] == "''";
        if imutavel {
            let valor = if literal_puro {
                valor_de_literal(&convertidas[0].texto)
            } else {
                valor_com(&textos_expr, familia)
            };
            let acao = self.acao(&simulada, alvo, &valor, &convertidas[0])?;
            return Ok(Ligada::Constante(format!(
                "      {acao} /* REF:{url}:{ini}:{fim} */;"
            )));
        }
        // `_maybeOptimizeInterpolation`: uma expressão primitiva mutável é
        // conferida crua e interpolada só na ação (a variável tem tipo
        // `dynamic`, daí `interpolate`).
        let primitiva = convertidas.len() == 1 && !convertidas[0].imutavel && primitivo(&tipos[0]);
        let (checagem, na_acao) = if primitiva {
            (
                convertidas[0].texto.clone(),
                valor_com(&[format!("currVal_{k}")], "interpolate"),
            )
        } else {
            (valor_com(&textos_expr, familia), format!("currVal_{k}"))
        };
        self.campos_expr.push(format!("  Object? _expr_{k};"));
        let acao = self.acao(&simulada, alvo, &na_acao, &convertidas[0])?;
        let chk = tardio(CHECK_BINDING);
        let fonte = literal(&a.valor);
        Ok(Ligada::Dinamica(format!(
            "    final currVal_{k} = {checagem};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, {fonte}, '{url}')) {{\n      {acao} /* REF:{url}:{ini}:{fim} */;\n      this._expr_{k} = currVal_{k};\n    }}"
        )))
    }

    /// Emite os nós filhos de `pai`. Fora da coleta, a primeira forma que o
    /// emissor não cobre devolve `Err` e o arquivo inteiro volta para o
    /// `build_runner`; na coleta, cada recusa é anotada e a varredura segue.
    fn nos(&mut self, nos: &[No], pai: &str) -> Result<(), Recusa> {
        for no in nos {
            if let Err(r) = self.no(no, pai) {
                self.anotar(r)?;
                // Na coleta, o que está abaixo de um nó recusado também
                // conta: desce com um pai qualquer (a saída é descartada).
                if let No::Elemento(e) = no {
                    self.coletar_abaixo(e);
                }
            }
        }
        Ok(())
    }

    /// Coleta as recusas da subárvore de um elemento que não saiu. Os locais
    /// de um `*` entram sem tipo, para as expressões que os citam serem
    /// examinadas pelo que são.
    fn coletar_abaixo(&mut self, e: &crate::html::Elemento) {
        if !self.coletando() || e.filhos.is_empty() {
            return;
        }
        let guardados = (
            self.locais.clone(),
            self.embutida,
            self.tb.clone(),
            self.decl_locais.clone(),
            self.locais_raiz.clone(),
        );
        // A ligação de texto é alocada por visão; aqui a saída é descartada.
        self.tb.get_or_insert_with(|| "_coleta".into());
        if let Some(estrela) = &e.estrela {
            let micro = crate::micro::analisar(&estrela.nome, &estrela.valor);
            // O tipo da coleção, se a expressão dela converte; senão os
            // locais ficam `dynamic`.
            let colecao = micro
                .propriedades
                .iter()
                .find(|(p, _)| p.ends_with("Of"))
                .and_then(|(_, x)| self.converter(x, Motivo::Ligacao).ok())
                .and_then(|c| c.tipo.map(|t| (t, c.escopo)));
            let locais = self
                .locais_da_micro(&micro, colecao.as_ref())
                .unwrap_or_else(|_| {
                    let mut l = self.locais.clone();
                    for (nome, _) in &micro.locais {
                        l.insert(
                            nome.clone(),
                            crate::expr::Local {
                                dart: format!("local_{nome}"),
                                tipo: "dynamic".into(),
                                escopo: None,
                            },
                        );
                    }
                    l
                });
            self.locais = locais;
            for (nome, _) in &micro.locais {
                self.decl_locais.insert(nome.clone(), Ok(String::new()));
            }
            self.embutida = true;
        }
        let _ = self.nos(&e.filhos, "_el_coleta");
        (
            self.locais,
            self.embutida,
            self.tb,
            self.decl_locais,
            self.locais_raiz,
        ) = guardados;
    }

    /// Um nó do template.
    fn no(&mut self, no: &No, pai: &str) -> Result<(), Recusa> {
        match no {
            No::Comentario(_) => {}
            No::Texto(t) => {
                if pai.is_empty() {
                    return Err(recusa(Motivo::Projecao, "texto projetado solto"));
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
                // `<template>` escrito à mão é uma visão embutida, não um
                // elemento HTML (`EmbeddedTemplateAst`).
                if e.nome.eq_ignore_ascii_case("template") {
                    return Err(recusa(Motivo::Ligacao, "<template> escrito no template"));
                }
                if e.estrela.is_none() {
                    self.guarda_do_elemento(e)?;
                }
                // `*` num filho: o filho vai para a visão embutida, como
                // qualquer elemento.
                if let (Some(estrela), true) = (&e.estrela, self.filhos.contains_key(&e.nome)) {
                    return self.estrutural(e, estrela, pai);
                }
                if let Some(filho) = self.filhos.get(&e.nome).cloned() {
                    return self.componente_filho(e, &filho, pai);
                }
                if !dom::tag_html(&e.nome) {
                    return Err(recusa(
                        Motivo::ComponenteNoTemplate,
                        "tag que não é HTML nem componente conhecido",
                    ));
                }
                // Nome de diretiva do ecossistema (`ngClass`, `ngModel`…)
                // numa ligação é coisa de diretiva, não propriedade do
                // DOM: emitir `setProperty` ali faria outra coisa — a não
                // ser que uma diretiva do catálogo o receba.
                let casadas = diretivas_casadas(self.usadas, e);
                if let Some(l) = e
                    .propriedades
                    .iter()
                    .chain(e.eventos.iter())
                    .chain(e.atributos.iter())
                    .chain(e.bananas.iter())
                    .find(|l| {
                        e_de_diretiva(&l.nome)
                            && !consome_entrada(&casadas, &l.nome)
                            && !consome_saida(&casadas, &l.nome)
                    })
                {
                    return Err(recusa(
                        Motivo::Diretiva,
                        format!(
                            "`{}` sem diretiva que o receba",
                            l.nome.split('.').next().unwrap_or(&l.nome)
                        ),
                    ));
                }
                if let Some(estrela) = &e.estrela {
                    return self.estrutural(e, estrela, pai);
                }
                self.elemento_html(e, pai)?;
            }
            No::Interpolacao { expr, inicio, fim } => {
                self.interpolacao(expr, *inicio, *fim, pai)?
            }
            No::Conteudo { seletor } => {
                // `<ng-content>` não consome índice de nó; o número é o
                // ordinal da projeção no template (`ngContentSelectors`),
                // com ou sem `select`.
                let _ = seletor;
                let i = self.proxima_projecao;
                self.proxima_projecao += 1;
                self.linhas.push(format!("    this.project({pai}, {i});"));
            }
        }
        Ok(())
    }

    /// Um elemento HTML: criação, atributos, ligações, eventos, estilo e os
    /// filhos.
    fn elemento_html(&mut self, e: &crate::html::Elemento, pai: &str) -> Result<(), Recusa> {
        // As diretivas do catálogo que casam o nó, e o `[(x)]` delas
        // desfeito como o `DesugarVisitor`: a entrada `x` e o evento
        // `xChange` com `valor = $event`.
        let casadas = diretivas_casadas(self.usadas, e);
        let mut propriedades = e.propriedades.clone();
        let mut eventos = e.eventos.clone();
        for b in &e.bananas {
            let mudanca = format!("{}Change", b.nome);
            if !consome_entrada(&casadas, &b.nome) || !consome_saida(&casadas, &mudanca) {
                self.anotar(recusa(Motivo::Ligacao, "[(x)] em elemento HTML"))?;
                continue;
            }
            propriedades.push(b.clone());
            eventos.push(crate::html::Ligacao {
                nome: mudanca,
                valor: format!("{} = $event", b.valor),
                ..b.clone()
            });
        }
        if let Some(a) = e
            .atributos
            .iter()
            .find(|a| a.valor.contains("{{") && consome_entrada(&casadas, &a.nome))
        {
            self.anotar(recusa(
                Motivo::DiretivaPorSeletor,
                format!("atributo interpolado em entrada de diretiva ({})", a.nome),
            ))?;
        }
        // `#ref` só na forma que não muda nada no nó; o valor dele é
        // registrado adiante, para o `@ViewChild`.
        if let Some(r) = e
            .referencias
            .iter()
            .find(|r| !r.valor.is_empty() || !self.refs_livres.contains(&r.nome))
        {
            self.anotar(recusa(
                Motivo::Ligacao,
                if !r.valor.is_empty() {
                    "#ref com valor (`#f=\"ngForm\"`)"
                } else if self.embutida {
                    "#ref em visão embutida"
                } else {
                    "#ref usado em expressão"
                },
            ))?;
        }
        let n = self.proximo;
        self.proximo += 1;
        let resolvido = if casadas.is_empty() {
            None
        } else {
            match crate::diretivas::resolver(&casadas, n) {
                Ok(r) => Some(r),
                Err(f) => {
                    self.anotar(recusa(Motivo::DiretivaPorSeletor, f))?;
                    None
                }
            }
        };
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
        let alvo = if !liga_no_elemento(e, &casadas) {
            self.linhas.push(format!("    final _el_{n} = {criacao};"));
            format!("_el_{n}")
        } else {
            let html = self.html.clone();
            self.campos_el
                .push(format!("  late final {html}.{tipo} _el_{n};"));
            self.linhas.push(format!("    this._el_{n} = {criacao};"));
            format!("this._el_{n}")
        };
        if pai.is_empty() {
            self.raizes.push(alvo.clone());
        }
        // `renderNode.toReadExpr()`: o local ou o campo, como o
        // nó tiver sido declarado.
        for r in &e.referencias {
            self.refs.insert(r.nome.clone(), alvo.clone());
        }
        // Atributos saem em ordem alfabética (`_toSortedBindings`).
        let mut atributos = e.atributos.clone();
        atributos.sort_by(|a, b| a.nome.cmp(&b.nome));
        for a in atributos.iter().filter(|a| !a.valor.contains("{{")) {
            let valor = literal(&a.valor);
            if a.nome == "class" {
                self.linhas
                    .push(format!("    this.updateChildClass({alvo}, {valor});"));
            } else if a.nome == "style" {
                self.anotar(recusa(Motivo::EstiloEmLinha, "style=\"...\" em linha"))?;
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
        // A `[x]` que uma diretiva recebe é entrada dela, não do nó.
        for l in propriedades
            .iter()
            .filter(|l| !consome_entrada(&casadas, &l.nome))
        {
            match self.propriedade(l, &alvo) {
                Ok(x) => ligadas.push(x),
                Err(r) => self.anotar(r)?,
            }
        }
        // Os atributos interpolados vêm depois das `[x]`, na ordem escrita
        // (`_visitProperties`).
        for a in e.atributos.iter().filter(|a| a.valor.contains("{{")) {
            match self.atributo_interpolado(a, &alvo) {
                Ok(x) => ligadas.push(x),
                Err(r) => self.anotar(r)?,
            }
        }
        self.escrever_ligacoes(ligadas);
        // Os ouvintes saem juntos no fim do `build()`, na ordem em que os
        // elementos aparecem (`bindView` depois de `_buildView`): os do
        // template que não são saída de diretiva, depois os
        // `@HostListener` das diretivas (`_visitHostListeners`).
        let mut do_no = e.clone();
        do_no.eventos = eventos
            .iter()
            .filter(|l| !consome_saida(&casadas, &l.nome))
            .cloned()
            .collect();
        if let Some(r) = &resolvido {
            for d in &casadas {
                for o in &d.ouvintes {
                    if do_no.eventos.iter().any(|l| l.nome == o.evento) {
                        self.anotar(recusa(
                            Motivo::Evento,
                            "evento do template e @HostListener de diretiva no mesmo nó",
                        ))?;
                    }
                }
            }
            self.eventos(&do_no, &alvo)?;
            // Os `@HostListener` na ordem das diretivas em `directives:`
            // (`_collectHostListeners`).
            for d in &casadas {
                let Some((_, campo)) = r
                    .diretivas
                    .iter()
                    .find(|(x, _)| std::sync::Arc::ptr_eq(x, d))
                else {
                    continue;
                };
                for o in &d.ouvintes {
                    let h = self.handler_de_hospedeiro(campo, o);
                    self.ouvinte(&o.evento, &alvo, &h);
                }
            }
        } else {
            self.eventos(&do_no, &alvo)?;
        }
        if self.com_estilo {
            // Isolamento de estilo por atributo: o elemento entra
            // no escopo do componente.
            self.linhas.push(format!("    this.addShimC({alvo});"));
        }
        let mut injetor = None;
        if let Some(r) = &resolvido {
            self.diretivas_do_no(e, r, &alvo, &propriedades, &eventos)?;
            let injetaveis: Vec<(Vec<crate::diretivas::Token>, String)> = r
                .instancias
                .iter()
                .filter(|i| !i.injetavel_por.is_empty())
                .map(|i| (i.injetavel_por.clone(), i.campo.clone()))
                .collect();
            if !injetaveis.is_empty() {
                injetor = Some(self.injetores.len());
                self.injetores.push((n, n, injetaveis));
            }
        }
        let r = self.nos(&e.filhos, &alvo);
        // `ProviderNode(nodeIndex, nodeIndex + childNodeCount)`.
        if let Some(i) = injetor {
            self.injetores[i].1 = self.proximo - 1;
        }
        r
    }

    /// O handler de um `@HostListener` de diretiva: método sem parâmetro ou
    /// com `$event` é tear-off da instância; com outro argumento, um
    /// `_handleEvent_N` que chama o método.
    fn handler_de_hospedeiro(&mut self, campo: &str, o: &crate::diretivas::Ouvinte) -> String {
        let m = &o.metodo;
        match o.args.as_str() {
            "" => format!("this.eventHandler0(this.{campo}.{m})"),
            "$event" => format!("this.eventHandler1(this.{campo}.{m})"),
            args => {
                let n = self.metodos_evento.len();
                self.metodos_evento.push(format!(
                    "\n  void _handleEvent_{n}($event) {{\n    this.{campo}.{m}({args});\n  }}\n"
                ));
                format!("this.eventHandler1(this._handleEvent_{n})")
            }
        }
    }

    /// Os provedores das diretivas do nó: campos, criação no `build()` (e o
    /// `registerDirective` do devtools), entradas e ganchos na detecção, e
    /// as saídas.
    fn diretivas_do_no(
        &mut self,
        e: &crate::html::Elemento,
        r: &crate::diretivas::NoResolvido,
        alvo: &str,
        propriedades: &[crate::html::Ligacao],
        eventos: &[crate::html::Ligacao],
    ) -> Result<(), Recusa> {
        use crate::diretivas::{Argumento, Criacao, Token};
        for inst in &r.instancias {
            let tipo = match (&inst.criacao, &inst.token) {
                (Criacao::Diretiva { diretiva, .. }, _) => {
                    format!("{}{}", self.imp.q(&diretiva.uri), diretiva.classe)
                }
                (Criacao::Lista(_), Token::Multi { tipo, .. }) if tipo.e_object() => {
                    format!("List<{}Object>", self.imp.q("dart:core"))
                }
                (Criacao::Lista(_), Token::Multi { tipo, .. }) if tipo.genericos > 0 => {
                    let args = vec!["dynamic"; tipo.genericos].join(", ");
                    format!("List<{}{}<{args}>>", self.imp.q(&tipo.uri), tipo.classe)
                }
                _ => return Err(recusa(Motivo::DiretivaPorSeletor, "provedor sem tipo")),
            };
            self.campos_filho
                .push(format!("  late final {tipo} {};", inst.campo));
            let valor = match &inst.criacao {
                Criacao::Diretiva { diretiva, args } => {
                    let args: Vec<String> = args
                        .iter()
                        .map(|a| match a {
                            Argumento::Elemento => alvo.to_string(),
                            Argumento::Detector => "this".to_string(),
                            Argumento::Nulo => "null".to_string(),
                            Argumento::Campo(c) => format!("this.{c}"),
                        })
                        .collect();
                    format!(
                        "{}{}({})",
                        self.imp.q(&diretiva.uri),
                        diretiva.classe,
                        args.join(", ")
                    )
                }
                Criacao::Lista(itens) => {
                    let itens: Vec<String> = itens.iter().map(|c| format!("this.{c}")).collect();
                    format!("[{}]", itens.join(", "))
                }
            };
            self.linhas
                .push(format!("    this.{} = {valor};", inst.campo));
        }
        // `registerDirectives`: as instâncias das diretivas, na ordem.
        let dev = self.imp.alias(DEVTOOLS);
        let registros: Vec<String> = r
            .diretivas
            .iter()
            .map(|(_, c)| {
                format!("      {dev}.Inspector.instance.registerDirective({alvo}, this.{c});")
            })
            .collect();
        self.linhas.push(format!(
            "    if ({dev}.isDevToolsEnabled) {{\n{}\n    }}",
            registros.join("\n")
        ));
        // Entradas e saídas, diretiva por diretiva, na ordem dos provedores
        // (`transformedDirectiveAsts`).
        for (d, campo) in &r.diretivas {
            let mut ligadas: Vec<(&crate::html::Ligacao, bool)> = e
                .atributos
                .iter()
                .filter(|a| !a.valor.contains("{{") && d.entrada(&a.nome).is_some())
                .map(|a| (a, true))
                .collect();
            ligadas.extend(
                propriedades
                    .iter()
                    .filter(|l| d.entrada(&l.nome).is_some())
                    .map(|l| (l, false)),
            );
            let spec: Vec<(String, String, Option<bool>)> = d
                .entradas
                .iter()
                .map(|x| (x.nome.clone(), x.membro.clone(), x.booleana))
                .collect();
            // Só `AfterChanges` e `OnInit` chegam aqui: os outros ganchos são
            // recusados pela guarda (`Diretiva::pendencia`).
            let ganchos = crate::componente::Ganchos {
                after_changes: d.ganchos.after_changes,
                on_init: d.ganchos.on_init,
                ..Default::default()
            };
            self.entradas_de(
                &ligadas,
                &spec,
                campo,
                None,
                &ganchos,
                Motivo::DiretivaPorSeletor,
            )?;
            let mut vistos: Vec<&str> = Vec::new();
            for l in eventos.iter().filter(|l| d.saida(&l.nome).is_some()) {
                if vistos.contains(&l.nome.as_str()) {
                    return Err(recusa(
                        Motivo::Evento,
                        "dois handlers do mesmo evento no template",
                    ));
                }
                vistos.push(&l.nome);
                let membro = d.saida(&l.nome).unwrap_or_default();
                match self.handler(&[&l.valor]) {
                    Ok(h) => {
                        let k = self.subscricoes;
                        self.subscricoes += 1;
                        self.ouvintes.push(format!(
                            "    final subscription_{k} = this.{campo}.{membro}.listen({h});"
                        ));
                    }
                    Err(r) => self.anotar(r)?,
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
///
/// Componente `onPush` com `@Input` ganha, antes dos ganchos, o
/// `if (changed) markAsCheckOnce()` de `bindDirectiveInputs` — que na
/// hospedeira nunca liga `changed`, mas o declara. O import do
/// `check_binding` só entra se o texto o usa (`tardio`).
fn ciclo_de_vida(g: &crate::componente::Ganchos, marca: bool) -> String {
    let dbg = tardio(CHECK_BINDING);
    let mut s = String::new();
    if g.tem_deteccao() || marca {
        s.push_str(
            "
  @override
  void detectChangesInternal() {
",
        );
        if marca {
            s.push_str("    bool changed = false;\n");
        }
        if g.usa_primeira_checagem() {
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
fn tem_elemento_ligado(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
) -> bool {
    nos.iter().any(|n| match n {
        // Componente filho não vira campo de elemento: quem guarda a raiz
        // dele é a visão-filha.
        // Subárvore de `*` é da visão embutida.
        No::Elemento(e) if e.estrela.is_some() => false,
        No::Elemento(e) if !filhos.contains_key(&e.nome) => {
            liga_no_elemento(e, &diretivas_casadas(usadas, e))
                || tem_elemento_ligado(&e.filhos, filhos, usadas)
        }
        No::Elemento(e) => tem_elemento_ligado(&e.filhos, filhos, usadas),
        _ => false,
    })
}

/// As diretivas que casam um elemento HTML, com os metadados lidos do
/// programa, na ordem de `directives:` (`_matchDirectives`). A guarda do
/// elemento já recusou as que o emissor não instancia.
fn diretivas_casadas(
    usadas: &[Usada],
    e: &crate::html::Elemento,
) -> Vec<std::sync::Arc<crate::diretivas::Diretiva>> {
    if usadas.is_empty() || !dom::tag_html(&e.nome) {
        return Vec::new();
    }
    let desc = crate::seletor::Elemento::do_template(e);
    usadas
        .iter()
        .filter(|u| u.filho.is_none() && crate::seletor::casa_algum(&u.seletores, &desc))
        .filter_map(|u| u.diretiva.clone())
        .collect()
}

/// Algum `@Input` das diretivas tem este nome?
fn consome_entrada(casadas: &[std::sync::Arc<crate::diretivas::Diretiva>], nome: &str) -> bool {
    casadas.iter().any(|d| d.entrada(nome).is_some())
}

/// Algum `@Output` das diretivas tem este nome?
fn consome_saida(casadas: &[std::sync::Arc<crate::diretivas::Diretiva>], nome: &str) -> bool {
    casadas.iter().any(|d| d.saida(nome).is_some())
}

/// O nó tem ligação dele mesmo — `[x]` que nenhuma diretiva recebe, ou
/// atributo com `{{ }}` —, e vira campo da visão.
fn liga_no_elemento(
    e: &crate::html::Elemento,
    casadas: &[std::sync::Arc<crate::diretivas::Diretiva>],
) -> bool {
    e.propriedades
        .iter()
        .any(|p| !consome_entrada(casadas, &p.nome))
        || e.atributos.iter().any(|a| a.valor.contains("{{"))
}

/// Atributo escrito sem valor (`<input required>`): o intervalo dele é só o
/// nome.
fn sem_valor(a: &crate::html::Ligacao) -> bool {
    a.valor.is_empty() && a.fim - a.inicio == a.nome.encode_utf16().count()
}

/// O `injectorGetInternal` de uma visão (`writeInjectorGetMethod`,
/// `ProviderForest.build`): os nós com provedor injetável, cada um com o
/// intervalo de índices que ele serve; vazio se não há nenhum.
/// Um nó com provedores injetáveis: (primeiro índice, último índice da
/// subárvore, [(tokens, campo)]).
type NoInjetor = (u32, u32, Vec<(Vec<crate::diretivas::Token>, String)>);

fn metodo_injetor(injetores: &[NoInjetor]) -> String {
    use crate::diretivas::Token;
    if injetores.is_empty() {
        return String::new();
    }
    // A floresta: cada nó é filho do mais próximo antes dele que o contém.
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
    fn token(t: &Token) -> String {
        match t {
            Token::Classe { uri, classe } => format!("{}{classe}", tardio_q(uri)),
            Token::Multi { nome, tipo } => {
                let arg = if tipo.e_object() {
                    format!("{}Object", tardio_q("dart:core"))
                } else {
                    let (uri, classe) = (&tipo.uri, &tipo.classe);
                    let args = vec!["dynamic"; tipo.genericos].join(", ");
                    format!("\u{1}k:{uri}#token|{uri}\u{2}{classe}<{args}>")
                };
                format!(
                    "const {}MultiToken<{arg}>('{nome}')",
                    tardio_q(crate::diretivas::DI_TOKENS)
                )
            }
            Token::Elemento | Token::Detector => String::new(),
        }
    }
    fn tokens(ts: &[Token]) -> String {
        let partes: Vec<String> = ts
            .iter()
            .map(|t| format!("identical(token, {})", token(t)))
            .collect();
        if partes.len() == 1 {
            partes[0].clone()
        } else {
            format!("({})", partes.join(" || "))
        }
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

/// O que gera campo na classe da visão, na ordem em que aparece.
enum CampoDaVisao<'a> {
    Filho(&'a Filho),
    /// Diretiva estrutural, com a URI da classe dela.
    Estrutural(&'static str),
    /// Provedores de diretivas num nó: a URI do tipo de cada campo, na
    /// ordem.
    Diretivas(Vec<String>),
}

/// Os campos que o template vai gerar, em ordem de documento. A ordem dos
/// imports segue esta lista, e é ela que faz a numeração bater com a do
/// oficial.
fn campos_em_ordem<'a>(
    nos: &[No],
    filhos: &'a std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
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
        } else {
            let casadas = diretivas_casadas(usadas, e);
            if let (false, Ok(r)) = (casadas.is_empty(), crate::diretivas::resolver(&casadas, 0)) {
                let uris = r
                    .instancias
                    .iter()
                    .map(|i| match (&i.criacao, &i.token) {
                        (crate::diretivas::Criacao::Diretiva { diretiva, .. }, _) => {
                            diretiva.uri.clone()
                        }
                        (_, crate::diretivas::Token::Multi { tipo, .. }) => tipo.uri.clone(),
                        _ => "dart:core".to_string(),
                    })
                    .collect();
                saida.push(CampoDaVisao::Diretivas(uris));
            }
        }
        saida.extend(campos_em_ordem(&e.filhos, filhos, usadas));
    }
    saida
}

/// O que é preciso para emitir uma visão embutida, guardado durante a
/// varredura e usado depois.
struct EspecEmbutida {
    /// Número da visão (`_ViewX3`), atribuído na varredura.
    indice: u32,
    /// Quantas `parentView` até a visão do componente.
    profundidade: u32,
    /// Ver [`Corpo::nivel_do_topo`] (em `EspecEmbutida`).
    nivel_do_topo: u32,
    classe: String,
    fabrica: String,
    nos: Vec<No>,
    locais: std::collections::HashMap<String, crate::expr::Local>,
    micro: crate::micro::Micro,
    /// Os locais declarados em visões ancestrais, com de onde vêm.
    ancestrais: std::collections::HashMap<String, Origem>,
}

/// De onde vem um local de visão ancestral: a chave em `locals`, a classe
/// da visão que o declara e quantos `parentView` separam as duas
/// (`getPropertyInView`, em `view_compiler_utils.dart`).
#[derive(Debug, Clone)]
struct Origem {
    chave: String,
    classe: String,
    niveis: u32,
}

/// O que toda visão do arquivo compartilha: o componente, as diretivas que
/// ele usa e onde o arquivo mora.
struct Contexto<'a> {
    membros: &'a std::collections::HashMap<String, crate::componente::Membro>,
    metodos: &'a std::collections::HashMap<String, String>,
    aridades: &'a std::collections::HashMap<String, usize>,
    filhos: &'a std::collections::HashMap<String, Filho>,
    usadas: &'a [Usada],
    asset: String,
    tipos: Option<(&'a dyn Resolucao, &'a Path)>,
    com_estilo: bool,
    url_do_template: Option<String>,
    classe_da_visao: String,
    tipo_do_contexto: String,
    html: String,
    pipes: &'a PipesDoTemplate,
}

impl<'a> Contexto<'a> {
    /// Um corpo vazio para uma visão deste arquivo.
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
            url_do_template: self.url_do_template.clone(),
            membros: self.membros,
            metodos: self.metodos,
            aridades: self.aridades,
            metodos_evento: Vec::new(),
            decl_locais: Default::default(),
            locais_raiz: Vec::new(),
            classe_desta: String::new(),
            locais_proprios: Vec::new(),
            ancestrais: Default::default(),
            filhos: self.filhos,
            usadas: self.usadas,
            coleta,
            asset: self.asset.clone(),
            tipos: self.tipos,
            com_estilo: self.com_estilo,
            embutidas: Vec::new(),
            classe_da_visao: self.classe_da_visao.clone(),
            ancoras: Vec::new(),
            embutida,
            locais: Default::default(),
            proxima_embutida: 1,
            proximo: 0,
            tem_doc: false,
            usa_ctx_no_build: false,
            proxima_projecao: 0,
            imp,
            html: self.html.clone(),
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
            raizes: Vec::new(),
            pai_projetado: None,
            nivel_do_topo: 0,
            injetores: Vec::new(),
        }
    }
}

/// Emite a classe de uma visão embutida, a sua fábrica e — recursivamente —
/// as visões embutidas dentro dela.
///
/// Roda **depois** do `angular.dart`, porque é aí que o oficial escreve
/// essas classes; os imports que ela aloca (`embedded_view`, o
/// `text_binding` dos campos, `render_view` do construtor, `dart:core` do
/// argumento de tipo, `interpolate`) saem nessa ordem.
///
/// Na coleta (`coleta` com `Some`), cada recusa é anotada e a emissão segue.
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
    let mut dentro = ctx.corpo(imp, nomes, coleta.take(), true);
    dentro.locais = espec.locais.clone();
    dentro.vista = espec.indice;
    dentro.profundidade = espec.profundidade;
    dentro.nivel_do_topo = espec.nivel_do_topo;
    // A numeração continua de onde o pai parou.
    dentro.proxima_embutida = espec.indice + 1;
    let r = corpo_da_embutida(&mut dentro, &espec, ctx, &ev, &classe, &fabrica);
    *coleta = dentro.coleta.take();
    let (mut texto, aninhadas) = r?;
    for a in aninhadas {
        texto.push_str(&emitir_embutida(a, ctx, imp, nomes, coleta)?);
    }
    Ok(texto)
}

/// O texto de uma visão embutida e as especificações das aninhadas nela.
fn corpo_da_embutida(
    dentro: &mut Corpo,
    espec: &EspecEmbutida,
    ctx: &Contexto,
    ev: &str,
    classe: &str,
    fabrica: &str,
) -> Result<(String, Vec<EspecEmbutida>), Recusa> {
    // O campo de ligação de texto é declarado antes do construtor, então o
    // import dele entra aqui.
    if tem_interpolacao(&espec.nos) {
        dentro.tb = Some(dentro.imp.alias(TEXT_BINDING));
    }
    if let Err(r) = alocar_imports_dos_campos(
        dentro.imp,
        &espec.nos,
        ctx.filhos,
        ctx.usadas,
        &ctx.asset,
        &ctx.pipes.imports_dos_campos(espec.indice),
    ) {
        dentro.anotar(r)?;
    }
    // O construtor vem depois dos campos e antes do `build()`, e é ele que
    // nomeia a `RenderView`.
    let rv = dentro.imp.alias(RENDER_VIEW);
    let util = dentro.imp.alias(UTILITIES);
    // A declaração de cada local desta visão, pronta para quem a pedir (a
    // detecção ou um `_handleEvent_N`). Local de visão *ancestral* é lido
    // pela cadeia de `parentView` (`unsafeCast<_ViewX2>((this.parentView!))
    // .locals['$implicit']`), mecanismo ainda sem caso no corpus: fica fora
    // do mapa e quem o lê recusa.
    dentro.classe_desta = espec.classe.clone();
    dentro.locais_proprios = espec.micro.locais.clone();
    dentro.ancestrais = espec.ancestrais.clone();
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
    // Na coleta, um nó recusado não consome índice: a visão parece vazia
    // sem estar.
    if dentro.proximo == 0 && dentro.coleta.as_ref().map_or(0, Vec::len) == anotadas {
        dentro.anotar(recusa(Motivo::Ligacao, "visão embutida sem nó"))?;
    }
    // Os locais lidos pela detecção, na ordem do primeiro uso.
    let mut declaracoes = Vec::new();
    for nome in dentro.locais_raiz.clone() {
        if let Some(Ok(d)) = dentro.decl_locais.get(&nome) {
            declaracoes.push(format!("    {d}"));
        }
    }
    // Os ouvintes são escritos depois dos nós: os imports deles vêm agora.
    let ouvintes: Vec<String> = dentro
        .ouvintes
        .iter()
        .map(|o| resolver_tardios(dentro.imp, o))
        .collect();
    // Os proxies de pipe desta visão (`afterNodes` da visão do componente,
    // que roda depois do `bindView` de todas): leem a instância na visão do
    // componente pela cadeia de `parentView` (`getPropertyInView`).
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
    // `_ctx` no `build()` só se alguém o lê ali (tear-off de evento, texto
    // imutável): `maybeCachedCtxDeclarationStatement`.
    let ctx_build = if cita_ctx(&dentro.linhas) || cita_ctx(&ouvintes) {
        "    final _ctx = this.ctx;\n"
    } else {
        ""
    };
    // Os nós, depois os ouvintes — e só então o `initRootNode`, que é a
    // declaração de fechamento (`_generateInitStatement`).
    let corpo = dentro
        .linhas
        .iter()
        .chain(&ouvintes)
        .chain(&criacao_pipes)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    // Mesma ordem da visão de topo: ligações de texto, visões-filhas e
    // âncoras, valores anteriores, elementos.
    let mut todos = dentro.campos.clone();
    todos.extend(dentro.campos_filho.clone());
    todos.extend(dentro.campos_expr.clone());
    todos.extend(ctx.pipes.campos(espec.indice, dentro.imp));
    todos.extend(dentro.campos_el.clone());
    let campos = if todos.is_empty() {
        String::new()
    } else {
        format!("{}\n", todos.join("\n"))
    };
    let mut linhas_det = Vec::new();
    // `_ctx` e `firstCheck` só são declarados se o corpo os usar de fato
    // (`writeChangeDetectionStatements`): numa visão de `*ngFor` a
    // interpolação costuma usar o local do laço, não o contexto. Os dois
    // vêm antes das declarações de local.
    if cita_ctx(&dentro.entradas) || cita_ctx(&dentro.deteccao) {
        linhas_det.push("    final _ctx = this.ctx;".to_string());
    }
    if dentro.usa_changed {
        linhas_det.push("    bool changed = false;".to_string());
    }
    if dentro.usa_primeira_checagem {
        linhas_det.push("    bool firstCheck = this.firstCheck;".to_string());
    }
    // Mesma ordem da visão de topo, com os locais antes de tudo.
    linhas_det.extend(declaracoes);
    linhas_det.extend(dentro.entradas.clone());
    for a in &dentro.ancoras {
        linhas_det.push(format!("    this.{a}.detectChangesInNestedViews();"));
    }
    linhas_det.extend(sem_lancar(&dentro.apos_conteudo));
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
    // O `injectorGetInternal` vem depois do `build()` e antes da detecção.
    let injetor = resolver_tardios(dentro.imp, &metodo_injetor(&dentro.injetores));
    let deteccao = resolver_tardios(dentro.imp, &deteccao);
    // Visão embutida também destrói o que pendurou nela.
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
    // Os `_handleEvent_N`, depois do `destroyInternal`.
    let metodos: String = dentro
        .metodos_evento
        .iter()
        .map(|m| resolver_tardios(dentro.imp, m))
        .collect();
    // A raiz é o nó 0: o local, ou o campo quando ele tem ligação
    // (`renderNode.toReadExpr()`).
    let raiz = if dentro.campos_el.iter().any(|c| c.ends_with(" _el_0;")) {
        "this._el_0"
    } else {
        "_el_0"
    };
    // `_generateInitStatement`: com `subscription_N`, a raiz vai numa lista.
    let inicio = if dentro.subscricoes == 0 {
        format!("this.initRootNode({raiz});")
    } else {
        let subs: Vec<String> = (0..dentro.subscricoes)
            .map(|k| format!("subscription_{k}"))
            .collect();
        format!(
            "this.initRootNodesAndSubscriptions({util}.unsafeCast(<Object>[{raiz}]), [{}]);",
            subs.join(", ")
        )
    };
    let tipo_do_contexto = &ctx.tipo_do_contexto;
    let texto = format!(
        "\nclass {classe} extends {ev}.EmbeddedView<{tipo_do_contexto}> {{\n{campos}  {classe}({rv}.RenderView parentView, int parentIndex) : super(parentView, parentIndex);\n  @override\n  void build() {{\n{ctx_build}{corpo}\n    {inicio}\n  }}\n{injetor}{deteccao}{destruicao}{metodos}}}\n\n{ev}.EmbeddedView<void> {fabrica}({rv}.RenderView parentView, int parentIndex) {{\n  return {classe}(parentView, parentIndex);\n}}\n"
    );
    Ok((texto, aninhadas))
}

/// `final local_x = unsafeCast<T>(this.locals['chave']);` — a declaração que
/// o `ViewNameResolver` guarda para o local, com o tipo qualificado pelo
/// import da biblioteca que o declara. O import fica marcado: é alocado onde
/// a declaração for escrita primeiro.
fn declaracao_de_local(
    l: &crate::expr::Local,
    origem: &Origem,
    ctx: &Contexto,
    util: &str,
) -> Result<String, Recusa> {
    let d = &l.dart;
    // A chave sai como literal escapado (`'\$implicit'`): sem o escape, o
    // `$` viraria interpolação em Dart.
    let chave = literal(&origem.chave);
    // Local desta visão: `this.locals`. De visão ancestral: a cadeia de
    // `parentView`, cada passo com `!`, e o cast para a classe da visão que
    // o declara.
    let locals = if origem.niveis == 0 {
        "this.locals".to_string()
    } else {
        let mut vista = "(this.parentView!)".to_string();
        for _ in 1..origem.niveis {
            vista = format!("({vista}.parentView!)");
        }
        format!("{util}.unsafeCast<{}>({vista}).locals", origem.classe)
    };
    let tipo = if matches!(
        l.tipo.as_str(),
        "String" | "int" | "double" | "bool" | "num" | "Object"
    ) {
        format!("{}{}", tardio_q("dart:core"), l.tipo)
    } else {
        let sem_import = || recusa(Motivo::Ligacao, "tipo do local de `*ngFor` sem import");
        let (r, arquivo) = ctx.tipos.ok_or_else(sem_import)?;
        let escopo = l.escopo.as_deref().unwrap_or(arquivo);
        let caminho = r
            .uri_do_tipo(escopo, &l.tipo)
            .and_then(|uri| asset_de_uri(&uri, "", Path::new("")))
            .and_then(|alvo| caminho_do_import(&ctx.asset, &alvo))
            .ok_or_else(sem_import)?;
        let simples = l.tipo.rsplit('.').next().unwrap_or(&l.tipo);
        format!("{}{simples}", tardio_q(&caminho))
    };
    Ok(format!(
        "final {d} = {util}.unsafeCast<{tipo}>({locals}[{chave}]);"
    ))
}

/// `if ((!debugThrowIfChanged)) { .. }` em volta dos ganchos de conteúdo ou
/// de visão (`notThrowOnChanges` em `writeChangeDetectionStatements`);
/// nada se não há ganchos.
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

/// Reindenta um bloco de instruções: cada linha ganha `n` espaços.
fn indentar(texto: &str, n: usize) -> String {
    let prefixo = " ".repeat(n);
    texto
        .lines()
        .map(|l| format!("{prefixo}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `addStmtsIfFirstCheck`: se a última instrução do método é um
/// `if (firstCheck)`, as novas entram nele; senão, abrem outro.
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
    usadas: &[Usada],
    asset: &str,
    pipes: &[String],
) -> Result<(), Recusa> {
    let sem_caminho = || recusa(Motivo::ComponenteNoTemplate, "filho sem caminho de import");
    for campo in campos_em_ordem(nos, filhos, usadas) {
        match campo {
            CampoDaVisao::Filho(f) => {
                for uri in [&f.uri_template, &f.uri_dart] {
                    let alvo = asset_de_uri(uri, "", Path::new("")).ok_or_else(sem_caminho)?;
                    let caminho = caminho_do_import(asset, &alvo).ok_or_else(sem_caminho)?;
                    imp.alias(&caminho);
                }
            }
            CampoDaVisao::Estrutural(uri) => {
                imp.alias(VIEW_CONTAINER);
                imp.alias(uri);
            }
            CampoDaVisao::Diretivas(uris) => {
                for uri in uris {
                    imp.alias(&uri);
                }
            }
        }
    }
    // Os campos de pipe vêm depois dos `_expr_` e antes dos `_el_`.
    for uri in pipes {
        imp.alias(uri);
    }
    if tem_elemento_ligado(nos, filhos, usadas) {
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

/// Como [`tardio`], mas resolve para o qualificador (`import3.` ou nada,
/// para as bibliotecas importadas sem prefixo).
fn tardio_q(uri: &str) -> String {
    format!("\u{1}q:{uri}\u{2}")
}

/// Troca cada marca de [`tardio`] e [`tardio_q`] pelo prefixo, alocando na
/// ordem do texto.
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

/// `o.literal('${valor}')` de um literal primitivo: o texto do valor, entre
/// aspas simples. O literal de texto já chega escrito assim; `null` vira
/// texto vazio.
fn valor_de_literal(texto: &str) -> String {
    if texto.starts_with('\'') {
        texto.to_string()
    } else if texto == "null" {
        "''".to_string()
    } else {
        literal(texto)
    }
}

/// `&ngsp;`, que o `ngast` troca por este caractere; volta a ser espaço.
const NGSP: char = '\u{E500}';

/// `_compressWhitespacePreceding`: com quebra de linha (e sem `&nbsp;` nem
/// `&ngsp;`), as quebras somem e o começo é aparado.
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

/// Os textos e as expressões de um valor com `{{ }}` (`splitInterpolation`,
/// com a expressão regular `{{([\s\S]*?)}}`): sempre um texto a mais que
/// expressões. Expressão vazia é erro no oficial.
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

/// Literal Dart de uma string, com aspas simples — o `escapeSingleQuoteString`
/// do emissor oficial.
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

/// Gera o `.template.dart` de um arquivo com um componente só.
///
/// O que não couber volta `Err` com a primeira recusa, e o arquivo continua
/// vindo do `build_runner`.
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
    let mut coleta = None;
    gerar_componente(
        c,
        local,
        nos,
        resolvedor,
        nomes,
        filhos,
        usadas,
        pipes,
        &mut coleta,
    )
}

/// Todas as recusas deste componente, não só a primeira: roda a mesma
/// emissão em modo de coleta. Sem isto o placar engana — um arquivo que
/// trava em folha de estilo pode travar também em evento e interpolação, e
/// contar só o primeiro faz parecer que aprender uma forma destrava o
/// arquivo.
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
    );
    let mut fora: std::collections::BTreeSet<Recusa> =
        coleta.unwrap_or_default().into_iter().collect();
    if let Err(r) = r {
        fora.insert(r);
    }
    // A folha de estilo é compilada por quem chama (`gerar_arquivo`); o
    // diagnóstico roda a mesma conta aqui.
    if c.style_urls.len() == 1 {
        let url = &c.style_urls[0];
        if local.uri_do_estilo(url).is_none() {
            fora.insert(recusa(Motivo::Estilos, "folha fora de lib/"));
        } else if !crate::estilo_compila(local.caminho, url) {
            fora.insert(recusa(Motivo::Estilos, "Sass ou CSS fora do subconjunto"));
        }
    }
    fora
}

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
) -> Result<String, Recusa> {
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
    for r in formas_contra_o_template(c, local, nos, resolvedor, filhos) {
        anotar(coleta, r)?;
    }
    // `pipes:` sem uso não muda a visão (caso b19).
    let tabela = match pipes_do_template(nos, filhos, pipes, &local.asset()) {
        Ok(t) => t,
        Err(r) => {
            anotar(coleta, r)?;
            PipesDoTemplate::default()
        }
    };
    if !c.styles.is_empty() {
        // `styles: ['…']` escrito na anotação ainda não.
        anotar(coleta, recusa(Motivo::Estilos, "styles: [..] na anotação"))?;
    }
    // A construção sai depois dos imports fixos, porque a injeção aloca os
    // seus (o `errors.dart` e o de cada tipo injetado) no fim da tabela.
    if let Some(r) = falta_para_construir(c, local, resolvedor) {
        anotar(coleta, r)?;
    }

    let mut imp = Importacoes::default();
    // A folha compilada é o primeiro import do arquivo, antes de tudo.
    let estilo = match c.style_urls.len() {
        0 => None,
        1 => {
            // A folha entra pela URI `package:` mesmo estando ao lado: é
            // assim que o oficial escreve (o resolvedor de `styleUrls` é
            // outro, e não passa pelo caminho relativo).
            match local.uri_do_estilo(&c.style_urls[0]) {
                Some(uri) => Some(imp.alias(&uri)),
                None => {
                    anotar(coleta, recusa(Motivo::Estilos, "folha fora de lib/"))?;
                    None
                }
            }
        }
        // Mais de uma folha muda a lista de `styles$X`; uma de cada vez.
        _ => {
            anotar(
                coleta,
                recusa(Motivo::Estilos, "mais de uma folha em styleUrls"),
            )?;
            None
        }
    };
    let vista = imp.alias(COMPONENT_VIEW);
    let proprio = imp.alias(local.arquivo);
    // Os campos da visão saem antes de tudo na classe — ligações de texto,
    // visões-filhas, valores anteriores, elementos —, e os imports são
    // alocados nessa mesma ordem. É isso que faz a numeração bater com a do
    // oficial; fora de ordem, a comparação byte a byte não vale nada.
    let tb = tem_interpolacao(nos).then(|| imp.alias(TEXT_BINDING));
    if let Err(r) = alocar_imports_dos_campos(
        &mut imp,
        nos,
        filhos,
        usadas,
        &local.asset(),
        &tabela.imports_dos_campos(0),
    ) {
        anotar(coleta, r)?;
    }
    let estilos = imp.alias(STYLE_ENCAPSULATION);
    let view = imp.alias(VIEW);
    let cd = imp.alias(CHANGE_DETECTION);
    let util = imp.alias(UTILITIES);
    // O construtor da visão usa `document.createElement`, então `dart:html`
    // sempre entra antes do corpo do `build()`.
    let html = imp.alias("dart:html");

    let ctx = Contexto {
        membros: &c.membros,
        metodos: &c.metodos,
        aridades: &c.aridades,
        filhos,
        usadas,
        asset: local.asset(),
        tipos: resolvedor.map(|r| (r, local.caminho)),
        com_estilo: !c.style_urls.is_empty(),
        url_do_template: local.url_do_template.clone(),
        classe_da_visao: format!("View{}", c.classe),
        tipo_do_contexto: format!("{proprio}.{}", c.classe),
        html: html.clone(),
        pipes: &tabela,
    };
    let mut corpo = ctx.corpo(&mut imp, nomes, coleta.take(), false);
    corpo.refs_livres = referencias_livres(nos);
    corpo.tb = tb;
    let r = corpo
        .nos(nos, "parentRenderNode")
        .and_then(|()| corpo.conferir_pipes());
    if let Err(r) = r {
        *coleta = corpo.coleta.take();
        return Err(r);
    }
    // `@ViewChild` estático: atribuição imediata, no `afterNodes` — depois
    // dos ouvintes, na ordem de declaração das consultas
    // (`updateQueryAtStartup`, `createImmediateUpdates` em
    // `compile_query.dart`). `formas_contra_o_template` já garantiu que cada
    // `#ref` está uma vez só, num elemento HTML da própria visão.
    let mut consultas = Vec::new();
    for q in &c.consultas {
        match corpo.refs.get(&q.referencia).cloned() {
            Some(alvo) => {
                // Filho `onPush`: o `ChangeDetectorRef` dele fica registrado
                // (`_createAddQueryChangeDetectorRefs`).
                if let Some(cv) = corpo.detectores.get(&q.referencia).cloned() {
                    let v = corpo.imp.alias(VIEW);
                    consultas.push(format!(
                        "    {v}.View.queryChangeDetectorRefs[{alvo}] = this.{cv};"
                    ));
                }
                consultas.push(format!("    _ctx.{} = {alvo};", q.propriedade));
            }
            // Na coleta a recusa já veio de `formas_contra_o_template`.
            None if corpo.coletando() => {}
            None => {
                return Err(recusa(
                    Motivo::ViewChildDinamico,
                    "@ViewChild sem #ref no template",
                ));
            }
        }
        corpo.usa_ctx_no_build = true;
    }
    // Os ouvintes dos elementos são escritos depois dos nós: os imports
    // deles entram agora.
    let ouvintes: Vec<String> = corpo
        .ouvintes
        .iter()
        .map(|o| resolver_tardios(corpo.imp, o))
        .collect();
    // Os pipes: instância e proxies, no `afterNodes`, antes das consultas.
    let criacao_pipes: Vec<String> = tabela
        .criacao(0, "this", corpo.imp)
        .iter()
        .map(|l| resolver_tardios(corpo.imp, l))
        .collect();
    // Os `@HostListener` do componente fecham o `build()`, ligados ao nó
    // raiz (`_writeComponentHostEventListeners`, depois do
    // `writeBuildStatements` em `_generateBuildMethod`). O handler passa
    // pelo mesmo conversor dos eventos do template, e um complexo ganha o
    // próximo `_handleEvent_N`.
    let mut hospedeiro = Vec::new();
    for o in &c.ouvintes {
        match corpo.handler(&[&o.handler]) {
            Ok(h) => hospedeiro.push(format!(
                "    parentRenderNode.addEventListener('{}', {h});",
                o.evento
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
    // Ordem do `build()` oficial (`_generateBuildMethod`): os nós (a fase
    // `_buildView`), os ouvintes (`bindView`) e, por fim, o que o `afterNodes`
    // acrescenta.
    let linhas = corpo
        .linhas
        .iter()
        .chain(&ouvintes)
        .chain(&criacao_pipes)
        .chain(&consultas)
        .chain(&hospedeiro)
        .cloned()
        .chain((corpo.subscricoes > 0).then(|| {
            let subs: Vec<String> = (0..corpo.subscricoes)
                .map(|k| format!("subscription_{k}"))
                .collect();
            format!("    this.initSubscriptions([{}]);", subs.join(", "))
        }))
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
    todos.extend(tabela.campos(0, corpo.imp));
    todos.extend(corpo.campos_el.clone());
    let campos = if todos.is_empty() {
        String::new()
    } else {
        format!("{}\n", todos.join("\n"))
    };
    // A detecção na ordem de `writeChangeDetectionStatements`: entradas de
    // diretivas e filhos, visões aninhadas, ligações de propriedade e texto,
    // visões-filhas. `_ctx` e `firstCheck` só são declarados se alguém os
    // lê (`maybeCachedCtxDeclarationStatement`).
    let mut linhas_deteccao = corpo.entradas.clone();
    for a in &corpo.ancoras {
        linhas_deteccao.push(format!("    this.{a}.detectChangesInNestedViews();"));
    }
    linhas_deteccao.extend(sem_lancar(&corpo.apos_conteudo));
    linhas_deteccao.extend(corpo.deteccao.iter().cloned());
    for v in &corpo.vistas_filhas {
        linhas_deteccao.push(format!("    this.{v}.detectChanges();"));
    }
    linhas_deteccao.extend(sem_lancar(&corpo.apos_visao));
    let deteccao = if linhas_deteccao.is_empty() {
        String::new()
    } else {
        let ctx_det = if cita_ctx(&linhas_deteccao) {
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
        format!(
            "\n  @override\n  void detectChangesInternal() {{\n{ctx_det}{mudou}{primeira}{}\n  }}\n",
            linhas_deteccao.join("\n")
        )
    };
    // O `injectorGetInternal` vem depois do `build()` e antes da detecção.
    let injetor = resolver_tardios(corpo.imp, &metodo_injetor(&corpo.injetores));
    // Os imports da detecção entram agora, depois dos do `build()`.
    let deteccao = resolver_tardios(corpo.imp, &deteccao);
    // Visão-filha precisa ser destruída com a visão que a criou.
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
    // Os `_handleEvent_N`, depois do `destroyInternal`.
    let metodos: String = corpo
        .metodos_evento
        .iter()
        .map(|m| resolver_tardios(corpo.imp, m))
        .collect();
    // `corpo` empresta o interner e a tabela de imports; a emissão das
    // visões embutidas precisa dos dois.
    *coleta = corpo.coleta.take();
    drop(corpo);

    imp.sem_alias(ANGULAR);
    // As visões embutidas são escritas aqui, depois das fábricas — e é por
    // isso que os imports delas vêm depois do `angular.dart`.
    let mut embutidas = String::new();
    for espec in especs {
        embutidas.push_str(&emitir_embutida(espec, &ctx, &mut imp, nomes, coleta)?);
    }
    let hosp = imp.alias(HOST_VIEW);
    let construcao = match construcao_do_componente(c, local, resolvedor, &mut imp, &proprio, &util)
    {
        Some(x) => x,
        None => {
            // Na coleta a recusa já veio de `falta_para_construir`.
            if coleta.is_none() {
                return Err(recusa(
                    Motivo::InjecaoNaoResolvida,
                    "construção do componente",
                ));
            }
            String::new()
        }
    };
    // Os ganchos de ciclo de vida saem na visão-hospedeira, depois do
    // `build()`, e o `check_binding.dart` entra aí.
    // `onPush` com `@Input`: a hospedeira marca a checagem
    // (`bindDirectiveInputs`, `hasInputs` conta os herdados — que daqui não
    // se veem: recusa).
    let marca = c.on_push && !c.entradas.is_empty();
    if c.on_push && c.entradas.is_empty() && c.herda {
        let r = recusa(Motivo::NaoEntendido, "componente onPush que herda @Input");
        if coleta.is_none() {
            return Err(r);
        }
    }
    let ciclo = resolver_tardios(&mut imp, &ciclo_de_vida(&c.ganchos, marca));

    let x = &c.classe;
    let seletor = &c.seletor;
    // `_getChangeDetectionCheckMode`: componente `onPush` começa em
    // `checkOnce`.
    let estado = if c.on_push {
        "checkOnce"
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
    let injeta = c
        .parametros
        .iter()
        .any(|p| !e_elemento(p.tipo.as_deref()) && !e_detector(p, local, resolvedor));
    // O `errors.dart` entra antes dos tipos injetados, como no oficial.
    let erros = injeta.then(|| imp.alias(DI_ERRORS));
    let mut args = Vec::new();
    for p in &c.parametros {
        if e_elemento(p.tipo.as_deref()) {
            args.push("_el_0".to_string());
            continue;
        }
        // `ChangeDetectorRef`: a visão do componente.
        if e_detector(p, local, resolvedor) {
            args.push("this.componentView".to_string());
            continue;
        }
        if (p.anotado && !p.opcional) || p.nomeado {
            return None; // `@Inject(...)`, `@Self`, nomeado: ainda não
        }
        // O token é o tipo sem o `?` (`@Optional() X? x`).
        let tipo = p.tipo.as_deref()?.trim_end_matches('?');
        if tipo.contains('<') {
            return None; // token genérico ainda não
        }
        let simples = tipo.rsplit('.').next()?;
        let uri = resolvedor?.uri_do_tipo(local.caminho, tipo)?;
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

/// O parâmetro é o elemento raiz do componente?
/// O parâmetro é o `ChangeDetectorRef` do ngdart (sem anotação)?
fn e_detector(
    p: &crate::componente::Parametro,
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
) -> Option<Recusa> {
    for p in &c.parametros {
        if e_elemento(p.tipo.as_deref()) {
            continue;
        }
        if p.anotado && !p.opcional {
            return Some(recusa(
                Motivo::InjecaoAnotada,
                "@Optional/@Inject/@Attribute no construtor",
            ));
        }
        if p.nomeado {
            return Some(recusa(
                Motivo::InjecaoNomeada,
                "parâmetro nomeado no construtor",
            ));
        }
        let Some(tipo) = p.tipo.as_deref() else {
            return Some(recusa(
                Motivo::InjecaoSemTipo,
                "parâmetro sem tipo no construtor",
            ));
        };
        if tipo.contains('<') {
            return Some(recusa(
                Motivo::InjecaoGenerica,
                "token genérico no construtor",
            ));
        }
        let tipo = tipo.trim_end_matches('?');
        if !resolvedor.is_some_and(|r| r.uri_do_tipo(local.caminho, tipo).is_some()) {
            return Some(recusa(
                Motivo::InjecaoNaoResolvida,
                "tipo injetado sem resolução",
            ));
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
            opcional: false,
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
            &[],
            &Ok(Vec::new()),
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
            &[],
            &Ok(Vec::new()),
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
        // Sem banco semântico não há como saber que biblioteca declara o tipo.
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
            &[],
            &Ok(Vec::new()),
        )
        .expect("gera");
        let esperado = include_str!("../testes/callback_com_injecao.template.dart");
        assert_eq!(saida, esperado.replace("\r\n", "\n"));
    }
}
