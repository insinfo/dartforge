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
#[derive(Default, Clone)]
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
const ELEMENT_REF: &str = "package:ngdart/src/core/linker/element_ref.dart";
const NG_IF: &str = "package:ngdart/src/common/directives/ng_if.dart";
const NG_FOR: &str = "package:ngdart/src/common/directives/ng_for.dart";
const NG_SWITCH: &str = "package:ngdart/src/common/directives/ng_switch.dart";
const NG_TEMPLATE_OUTLET: &str = "package:ngdart/src/common/directives/ng_template_outlet.dart";
/// O nome da `estrela` posta num `<template>` escrito à mão
/// ([`template_como_container`]): marca a fronteira da visão embutida para
/// quem anda pelo template, sem ser diretiva nenhuma.
const MARCA_DE_MOLDE: &str = "\u{0}template";
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
    /// `@i18n` fora da forma que o emissor traduz (mensagem com HTML,
    /// anotação em componente filho ou em `*`, outra anotação).
    I18n,
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
            Motivo::I18n => "@i18n",
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

/// Os `#ref` de uma visão embutida que são só um nome para o nó: sem valor,
/// declarados nela, não lidos por nenhuma expressão do escopo dela
/// ([`citado_no_escopo`]) e sem consulta que os procure — o oficial cria o
/// elemento como se o `#ref` não existisse.
fn referencias_livres_da_visao(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    consultados: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    let mut proprios = Vec::new();
    fn andar(nos: &[No], saida: &mut Vec<(String, String)>) {
        for n in nos {
            if let No::Elemento(e) = n {
                // O que está num `*` (ou num `<template>`) é de outra visão.
                if e.estrela.is_some() {
                    if e.estrela.as_ref().is_some_and(|l| l.nome == MARCA_DE_MOLDE) {
                        saida.extend(
                            e.referencias
                                .iter()
                                .map(|r| (r.nome.clone(), r.valor.clone())),
                        );
                    }
                    continue;
                }
                saida.extend(
                    e.referencias
                        .iter()
                        .map(|r| (r.nome.clone(), r.valor.clone())),
                );
                andar(&e.filhos, saida);
            }
        }
    }
    andar(nos, &mut proprios);
    proprios
        .into_iter()
        .filter(|(nome, valor)| {
            valor.is_empty() && !consultados.contains(nome) && !citado_no_escopo(nos, nome, filhos)
        })
        .map(|(nome, _)| nome)
        .collect()
}

/// Os metadados de uma mensagem `@i18n` (`I18nMetadata`), com os valores
/// já normalizados (`_normalizeWhitespace`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MetaI18n {
    descricao: Option<String>,
    locale: Option<String>,
    meaning: Option<String>,
    skip: bool,
}

/// `parseI18nMetadata`: as anotações do elemento agrupadas pelo atributo
/// que internacionalizam (`None`: os filhos). O nome casa com
/// `i18n(.locale|.meaning|.skip)?(:atributo)?`; anotação que não é de
/// `@i18n`, parâmetro sem descrição e atributo vazio são recusados.
fn metadados_i18n(e: &crate::html::Elemento) -> Result<Vec<(Option<String>, MetaI18n)>, Recusa> {
    fn normalizar(v: &str) -> String {
        v.split_whitespace().collect::<Vec<_>>().join(" ")
    }
    let mut saida: Vec<(Option<String>, MetaI18n)> = Vec::new();
    for a in &e.anotacoes {
        let Some(resto) = a.nome.strip_prefix("i18n") else {
            return Err(recusa(Motivo::I18n, format!("anotação @{}", a.nome)));
        };
        let (parametro, resto) = match resto.strip_prefix('.') {
            Some(r) => {
                let fim = r.find(':').unwrap_or(r.len());
                (Some(&r[..fim]), &r[fim..])
            }
            None => (None, resto),
        };
        let atributo = match resto.strip_prefix(':') {
            Some("") => return Err(recusa(Motivo::I18n, "@i18n: sem atributo")),
            Some(x) => Some(x.to_string()),
            None if resto.is_empty() => None,
            None => return Err(recusa(Motivo::I18n, format!("anotação @{}", a.nome))),
        };
        let i = match saida.iter().position(|(k, _)| *k == atributo) {
            Some(i) => i,
            None => {
                saida.push((atributo, MetaI18n::default()));
                saida.len() - 1
            }
        };
        let m = &mut saida[i].1;
        match parametro {
            None => m.descricao = Some(normalizar(&a.valor)),
            Some("locale") => m.locale = Some(normalizar(&a.valor)),
            Some("meaning") => m.meaning = Some(normalizar(&a.valor)),
            Some("skip") => m.skip = true,
            Some(_) => return Err(recusa(Motivo::I18n, format!("anotação @{}", a.nome))),
        }
    }
    if saida.iter().any(|(_, m)| m.descricao.is_none()) {
        return Err(recusa(Motivo::I18n, "parâmetro de @i18n sem a descrição"));
    }
    Ok(saida)
}

/// Na visão do componente (fora de `*`), a primeira anotação `@i18n` vem
/// antes da primeira interpolação? É o que decide se o `package:intl` (do
/// campo `_message_N`) é importado antes ou depois do `text_binding.dart`:
/// os dois campos são alocados pelo `build()`, em ordem de documento.
/// `None`: não há anotação na visão do componente.
fn i18n_antes_da_interpolacao(nos: &[No]) -> Option<bool> {
    fn andar(nos: &[No], interpolou: &mut bool) -> Option<bool> {
        for n in nos {
            match n {
                No::Interpolacao { .. } => *interpolou = true,
                No::Elemento(e) if e.estrela.is_some() => {}
                No::Elemento(e) => {
                    if !e.anotacoes.is_empty() {
                        // Mensagem com HTML é método, não campo: o import
                        // sai com o método, depois do `build()`.
                        if mensagem_em_campo(e) {
                            return Some(!*interpolou);
                        }
                        continue;
                    }
                    if let Some(r) = andar(&e.filhos, interpolou) {
                        return Some(r);
                    }
                }
                _ => {}
            }
        }
        None
    }
    andar(nos, &mut false)
}

/// Algum elemento com anotação (`@i18n`) nesta visão (fora de `*`)?
fn contem_anotacao(nos: &[No]) -> bool {
    nos.iter().any(|n| match n {
        No::Elemento(e) if e.estrela.is_some() => false,
        No::Elemento(e) => !e.anotacoes.is_empty() || contem_anotacao(&e.filhos),
        _ => false,
    })
}

/// O `@i18n` do elemento gera algum campo `static final String
/// _message_N`? Os de atributo (`@i18n:x`) e o dos filhos quando eles são
/// um texto só (`_isText` do `i18n.dart`); com HTML, a mensagem dos filhos é
/// um método estático.
fn mensagem_em_campo(e: &crate::html::Elemento) -> bool {
    let de_atributo = e.anotacoes.iter().any(|a| a.nome.contains(':'));
    de_atributo || matches!(e.filhos.as_slice(), [No::Texto(_)])
}

/// A mensagem `@i18n` de filhos com HTML, como o `I18nBuilder` a monta: o
/// texto (escapado à mão: `\n`, `\r`, `'`, `$`, `\`) com `${startTagK}`,
/// `${endTagK}` e `${voidElementK}` no lugar das tags, e os argumentos na
/// ordem em que aparecem. Só elemento HTML sem atributo, ligação, evento,
/// `#ref`, anotação ou `*`, e texto sem `&` (a entidade seria decodificada e
/// escapada de novo); interpolação e `<ng-content>` são erro no oficial.
///
/// # Erros
///
/// A forma que ainda não se escreve.
fn mensagem_com_html(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    texto: &mut String,
    args: &mut Vec<(String, String)>,
    tags: &mut usize,
) -> Result<bool, &'static str> {
    let mut tem_texto = false;
    for n in nos {
        match n {
            No::Comentario(_) => {}
            No::Texto(t) => {
                if t.contains('&') || t.contains(crate::html::NGSP) {
                    return Err("mensagem @i18n com entidade HTML");
                }
                tem_texto |= !t.trim().is_empty();
                for c in t.chars() {
                    match c {
                        '\n' => texto.push_str("\\n"),
                        '\r' => texto.push_str("\\r"),
                        '\'' | '$' | '\\' => {
                            texto.push('\\');
                            texto.push(c);
                        }
                        c => texto.push(c),
                    }
                }
            }
            No::Elemento(e) => {
                let simples = e.atributos.is_empty()
                    && e.propriedades.is_empty()
                    && e.eventos.is_empty()
                    && e.bananas.is_empty()
                    && e.referencias.is_empty()
                    && e.anotacoes.is_empty()
                    && e.estrela.is_none()
                    && !filhos.contains_key(&e.nome)
                    && dom::tag_html(&e.nome)
                    && !matches!(e.nome.as_str(), "template" | "ng-container");
                if !simples {
                    return Err("mensagem @i18n com elemento que não é HTML simples");
                }
                let k = *tags;
                *tags += 1;
                let mut arg = |nome: String, valor: String, texto: &mut String| {
                    let _ = write!(texto, "${{{nome}}}");
                    args.push((nome, valor));
                };
                if crate::html::VAZIOS.contains(&e.nome.as_str()) {
                    if !e.filhos.is_empty() {
                        return Err("mensagem @i18n com elemento vazio com filhos");
                    }
                    arg(format!("voidElement{k}"), format!("<{}>", e.nome), texto);
                } else {
                    arg(format!("startTag{k}"), format!("<{}>", e.nome), texto);
                    tem_texto |= mensagem_com_html(&e.filhos, filhos, texto, args, tags)?;
                    let _ = write!(texto, "${{endTag{k}}}");
                    args.push((format!("endTag{k}"), format!("</{}>", e.nome)));
                }
            }
            No::Interpolacao { .. } | No::Conteudo { .. } => {
                return Err("mensagem @i18n com interpolação ou <ng-content>");
            }
        }
    }
    Ok(tem_texto)
}

/// Marca do nó de um `#ref` lido como local: `\u{5}nome\u{6}`. A
/// declaração (`final local_x = this._el_n;`) é pedida por quem lê o local,
/// às vezes antes de o elemento ser criado; a marca é trocada pelo nó quando
/// a visão está completa ([`resolver_refs`]).
const MARCA_DE_REF: char = '\u{5}';
const FIM_DE_REF: char = '\u{6}';

/// Prefixo, na chave de [`resolver_refs`], da visão de um filho `onPush`
/// achado pela chave de uma consulta (`\u{5}.\u{8}f\u{6}` →
/// `._compView_1`), lido de uma visão aninhada ([`MontagemDaConsulta::mapa`]).
const MARCA_DE_DETECTOR: char = '\u{8}';

/// Os nomes de `#ref` do template que podem virar local de alguma visão:
/// sem método do componente com o mesmo nome, e sem membro cujo tipo não se
/// sabe daqui (com o mesmo nome de um membro, o local é o nó, mas o
/// `_TypeResolver` do oficial tipa a leitura pelo membro:
/// [`Corpo::declarar_refs`]). A unicidade e o sombreamento por `let` são de
/// cada visão ([`referencias_locais`]).
fn referencias_candidatas(nos: &[No], c: &Componente) -> std::collections::HashSet<String> {
    fn todas(nos: &[No], refs: &mut Vec<String>) {
        for n in nos {
            if let No::Elemento(e) = n {
                refs.extend(e.referencias.iter().map(|r| r.nome.clone()));
                todas(&e.filhos, refs);
            }
        }
    }
    let mut refs = Vec::new();
    todas(nos, &mut refs);
    refs.into_iter()
        .filter(|nome| {
            !c.metodos.contains_key(nome.as_str())
                && c.membros
                    .get(nome.as_str())
                    .is_none_or(|m| !m.tipo.trim().is_empty())
        })
        .collect()
}

/// Os `#ref` (de `candidatos`) que viram local desta visão: declarados uma
/// vez só num elemento HTML ou de componente filho dela (não no conteúdo de
/// um filho; os de visões embutidas nela são delas), sem um `let` desta
/// visão com o mesmo nome — o nó ou a instância do filho — e lidos por
/// alguma expressão dela ou das visões embutidas em que o nome não é
/// sombreado.
///
/// É o `nameResolver.addLocal(nome, renderNode)` do `CompileElement`: cada
/// visão tem o seu resolvedor, filho do da visão de fora, e o nome mais
/// próximo vence (`let` e `#ref` da visão embutida escondem o de fora). Quem
/// lê o local fora do `build()` (detecção, handler, outra visão) promove o
/// nó a campo (`NodeReferenceStorageVisitor`), e o tipo da leitura é
/// `dynamic` (a referência não entra nos `locals` do `AnalyzedClass`).
fn referencias_locais(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    candidatos: &std::collections::HashSet<String>,
    lets: &[String],
) -> std::collections::HashSet<String> {
    candidatos
        .iter()
        .filter(|nome| {
            let mut lugares = Vec::new();
            onde_esta(nos, nome, filhos, false, &mut lugares);
            let proprios: Vec<&Lugar> = lugares.iter().filter(|l| **l != Lugar::Embutida).collect();
            matches!(
                proprios.as_slice(),
                [Lugar::Raiz | Lugar::NoFilho | Lugar::Projetado | Lugar::NoFilhoProjetado]
            ) && !lets.contains(nome)
                && citado_no_escopo(nos, nome, filhos)
        })
        .cloned()
        .collect()
}

/// `nome` é lido por alguma expressão desta visão ou de uma embutida nela
/// que não o sombreie (um `let` dela, ou um `#ref` declarado nela). As
/// ligações do `*` são avaliadas na visão de fora; os `let` dele, não são
/// leituras.
fn citado_no_escopo(
    nos: &[No],
    nome: &str,
    filhos: &std::collections::HashMap<String, Filho>,
) -> bool {
    let cita = |texto: &str| cita_na_raiz(texto, nome);
    nos.iter().any(|n| match n {
        No::Interpolacao { expr, .. } => cita(expr),
        No::Elemento(e) => match &e.estrela {
            Some(estrela) if estrela.nome != MARCA_DE_MOLDE => {
                let micro = e.micro_da_estrela().unwrap_or_default();
                micro.propriedades.iter().any(|(_, expr)| cita(expr))
                    || citado_na_embutida(e, nome, filhos)
            }
            Some(_) => elemento_cita(e, nome) || citado_na_embutida(e, nome, filhos),
            None => elemento_cita(e, nome) || citado_no_escopo(&e.filhos, nome, filhos),
        },
        _ => false,
    })
}

/// `nome` é lido na detecção desta visão — ligação, interpolação, o `*`
/// de um elemento dela —, sem contar eventos nem as visões embutidas.
fn lido_na_deteccao(nos: &[No], nome: &str) -> bool {
    let cita = |texto: &str| cita_na_raiz(texto, nome);
    let no_elemento = |e: &crate::html::Elemento| {
        e.propriedades
            .iter()
            .chain(e.bananas.iter())
            .any(|l| cita(&l.valor))
            || e.atributos
                .iter()
                .any(|a| a.valor.contains("{{") && cita(&a.valor))
    };
    nos.iter().any(|n| match n {
        No::Interpolacao { expr, .. } => cita(expr),
        No::Elemento(e) => match &e.estrela {
            Some(estrela) if estrela.nome != MARCA_DE_MOLDE => {
                let micro = e.micro_da_estrela().unwrap_or_default();
                micro.propriedades.iter().any(|(_, expr)| cita(expr))
            }
            Some(_) => no_elemento(e),
            None => no_elemento(e) || lido_na_deteccao(&e.filhos, nome),
        },
        _ => false,
    })
}

/// Alguma ligação do próprio elemento lê `nome` com o receptor implícito?
fn elemento_cita(e: &crate::html::Elemento, nome: &str) -> bool {
    let cita = |texto: &str| cita_na_raiz(texto, nome);
    e.propriedades
        .iter()
        .chain(e.eventos.iter())
        .chain(e.bananas.iter())
        .any(|l| cita(&l.valor))
        || e.atributos
            .iter()
            .any(|a| a.valor.contains("{{") && cita(&a.valor))
}

/// A visão embutida do `*` (ou do `<template>` escrito) `e` lê o `nome` de
/// fora? Não, se ela o sombreia: um `let` dela ou um `#ref` declarado nela.
fn citado_na_embutida(
    e: &crate::html::Elemento,
    nome: &str,
    filhos: &std::collections::HashMap<String, Filho>,
) -> bool {
    let tem_ref = |x: &crate::html::Elemento| x.referencias.iter().any(|r| r.nome == nome);
    let mut lugares = Vec::new();
    match &e.estrela {
        Some(estrela) if estrela.nome != MARCA_DE_MOLDE => {
            let micro = e.micro_da_estrela().unwrap_or_default();
            // O elemento do `*` e o que está abaixo dele são da visão
            // embutida.
            onde_casa(
                &[No::Elemento(crate::html::Elemento {
                    estrela: None,
                    ..e.clone()
                })],
                &tem_ref,
                filhos,
                false,
                &mut lugares,
            );
            let sombreado = micro.locais.iter().any(|(l, _)| l == nome)
                || lugares.iter().any(|l| *l != Lugar::Embutida);
            !sombreado && (elemento_cita(e, nome) || citado_no_escopo(&e.filhos, nome, filhos))
        }
        _ => {
            // `<template>` escrito: o conteúdo é a visão embutida; os
            // `let-x` ficam nos atributos do elemento.
            onde_casa(&e.filhos, &tem_ref, filhos, false, &mut lugares);
            let lets = e
                .atributos
                .iter()
                .any(|a| a.nome.strip_prefix("let-") == Some(nome));
            let sombreado = lets || lugares.iter().any(|l| *l != Lugar::Embutida);
            !sombreado && citado_no_escopo(&e.filhos, nome, filhos)
        }
    }
}

/// Os campos criados pelo binder, na ordem dele: os `_expr_k` das ligações
/// e, entre eles, o nó que uma embutida lê, na posição em que ela foi ligada
/// ([`Corpo::posicao_de_embutida`]; sem posição, depois de todos).
fn campos_da_deteccao(
    exprs: &[String],
    de_embutidas: &[(String, String)],
    posicoes: &std::collections::HashMap<String, usize>,
) -> Vec<String> {
    let posicao = |nome: &str| posicoes.get(nome).copied().unwrap_or(exprs.len());
    let mut saida = Vec::with_capacity(exprs.len() + de_embutidas.len());
    for k in 0..=exprs.len() {
        saida.extend(
            de_embutidas
                .iter()
                .filter(|(n, _)| posicao(n) == k)
                .map(|(_, c)| c.clone()),
        );
        if let Some(e) = exprs.get(k) {
            saida.push(e.clone());
        }
    }
    saida
}

/// `nome` é lido só de dentro de uma visão embutida nesta (sem contar as
/// expressões da própria visão): o oficial promove o nó quando compila a
/// embutida, antes das ligações desta (o campo vem antes dos `_expr_k`).
fn citado_em_embutidas(
    nos: &[No],
    nome: &str,
    filhos: &std::collections::HashMap<String, Filho>,
) -> bool {
    nos.iter().any(|n| match n {
        No::Elemento(e) if e.estrela.is_some() => citado_na_embutida(e, nome, filhos),
        No::Elemento(e) => citado_em_embutidas(&e.filhos, nome, filhos),
        _ => false,
    })
}

/// A chave de um `#ref` lido como local na marca e no mapa dos resolvidos:
/// o nome e a visão que o declara — o mesmo nome pode ser de visões
/// diferentes (cada uma resolve o seu, [`referencias_locais`]).
fn chave_de_ref(nome: &str, classe_da_visao: &str) -> String {
    format!("{nome}@{classe_da_visao}")
}

/// O `k`-ésimo resultado de `chave` numa visão (os `#ref` repetidos, os
/// filhos do mesmo tipo): a marca que [`exportar_refs`] registra pela ordem
/// em que os nós foram vistos.
fn chave_de_ordinal(chave: &str, k: usize) -> String {
    format!("{chave}\u{3}{k}")
}

/// Os `#ref` de uma visão pronta no mapa dos resolvidos: pela chave com a
/// visão ([`chave_de_ref`]) e pelo nome (o das consultas, que leem o nó de
/// uma aninhada pelo nome).
fn exportar_refs(
    resolvidos: &std::cell::RefCell<std::collections::HashMap<String, String>>,
    refs: &std::collections::HashMap<String, String>,
    detectores: &std::collections::HashMap<String, String>,
    em_ordem: &[(String, String)],
    detectores_em_ordem: &[(String, String)],
    classe_da_visao: &str,
) {
    let mut r = resolvidos.borrow_mut();
    for (nome, leitura) in refs {
        r.insert(chave_de_ref(nome, classe_da_visao), leitura.clone());
        r.insert(nome.clone(), leitura.clone());
    }
    let mut vistos: std::collections::HashMap<&str, usize> = Default::default();
    for (nome, leitura) in em_ordem {
        let k = vistos.entry(nome.as_str()).or_default();
        r.insert(
            chave_de_ref(&chave_de_ordinal(nome, *k), classe_da_visao),
            leitura.clone(),
        );
        *k += 1;
    }
    let mut vistos: std::collections::HashMap<&str, usize> = Default::default();
    for (chave, visao) in detectores_em_ordem {
        let k = vistos.entry(chave.as_str()).or_default();
        r.insert(
            format!(
                "{MARCA_DE_DETECTOR}{}",
                chave_de_ref(&chave_de_ordinal(chave, *k), classe_da_visao)
            ),
            format!("this.{visao}"),
        );
        *k += 1;
    }
    for (chave, visao) in detectores {
        r.insert(
            format!("{MARCA_DE_DETECTOR}{chave}"),
            format!("this.{visao}"),
        );
        r.insert(
            format!(
                "{MARCA_DE_DETECTOR}{}",
                chave_de_ref(chave, classe_da_visao)
            ),
            format!("this.{visao}"),
        );
    }
}

/// Troca as marcas de `#ref` ([`MARCA_DE_REF`]) pelo nó de cada um:
/// `\u{5}x\u{6}` pela leitura na própria visão (`this._el_3`), e
/// `\u{5}.x\u{6}` só pelo campo (`_el_3`), lido de uma visão aninhada. A
/// marca de um nó ainda não visto fica (a visão que o cria vem depois).
fn resolver_refs(texto: &str, refs: &std::collections::HashMap<String, String>) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(i) = resto.find(MARCA_DE_REF) {
        saida.push_str(&resto[..i]);
        let depois = &resto[i + MARCA_DE_REF.len_utf8()..];
        let f = depois.find(FIM_DE_REF).unwrap_or(depois.len());
        let nome = &depois[..f];
        match (
            nome.strip_prefix('.'),
            refs.get(nome.trim_start_matches('.')),
        ) {
            (Some(_), Some(leitura)) => {
                saida.push('.');
                saida.push_str(leitura.strip_prefix("this.").unwrap_or(leitura));
            }
            (None, Some(leitura)) => saida.push_str(leitura),
            (_, None) => {
                saida.push(MARCA_DE_REF);
                saida.push_str(nome);
                saida.push(FIM_DE_REF);
            }
        }
        resto = depois.get(f + FIM_DE_REF.len_utf8()..).unwrap_or("");
    }
    saida.push_str(resto);
    saida
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
    usadas: &[Usada],
) -> Vec<Recusa> {
    let mut fora = Vec::new();
    let moldes = referencias_de_moldes(nos);
    for consulta in &c.consultas {
        let mut lugares = Vec::new();
        // `read:` na consulta estática de um elemento da visão (abaixo) ou
        // na de um elemento em `*` ([`consulta_em_embutida`]): por tipo,
        // lista estática ou `<template>` ainda não.
        let de_provedor = !consulta.por_tipo
            && !moldes.contains(&consulta.referencia)
            && token_de_leitura(consulta, local, resolvedor).is_some();
        // `read: ViewContainerRef` num `<template #x>`: o valor é o
        // `ViewContainer` do nó, atribuído no `build()` — a forma estática
        // (um `#x` só, na própria visão, `@ViewChild`).
        if !consulta.por_tipo
            && moldes.contains(&consulta.referencia)
            && le_container(consulta, local, resolvedor)
        {
            onde_esta(nos, &consulta.referencia, filhos, false, &mut lugares);
            if consulta.lista || lugares.as_slice() != [Lugar::Raiz] {
                fora.push(recusa(
                    Motivo::ViewChildDinamico,
                    "@ViewChild(.., read: ViewContainerRef) de <template> fora da forma estática",
                ));
            }
            continue;
        }
        if consulta.leitura.is_some()
            && !de_provedor
            && !consulta_em_embutida(nos, consulta, filhos, local, resolvedor)
            && (consulta.por_tipo || consulta.lista || moldes.contains(&consulta.referencia))
        {
            fora.push(recusa(
                Motivo::ViewChildEmFilho,
                "@ViewChild(.., read: T) fora de elemento estático",
            ));
            continue;
        }
        if consulta.por_tipo {
            // `@ViewChild(Tipo)`: as instâncias dos filhos dessa classe. A
            // diretiva de atributo do mesmo tipo também seria resultado:
            // ainda não.
            let Some(uri) = uri_da_consulta(consulta, local, resolvedor) else {
                fora.push(recusa(
                    Motivo::ViewChildEmFilho,
                    "@ViewChild(Tipo) sem resolução",
                ));
                continue;
            };
            if consulta_em_embutida(nos, consulta, filhos, local, resolvedor) {
                continue;
            }
            if consulta_de_token_dinamica(nos, consulta, filhos, usadas, local, resolvedor)
                .is_some()
            {
                continue;
            }
            // `@ViewChild(Diretiva)`: o token dela está no
            // `_resolvedProvidersArray` do elemento que ela casa, e o valor é
            // o campo dela (`_providers.get(tipo).build()`, com o
            // `.instance` de uma `XNgCd`), atribuído no fim do `build()`
            // quando o resultado está na própria visão (caso j117).
            if let Some(u) = usadas
                .iter()
                .find(|u| u.filho.is_none() && u.uri == uri && u.classe == consulta.referencia)
            {
                let casa = |e: &crate::html::Elemento| {
                    !filhos.contains_key(&e.nome)
                        && crate::seletor::casa_algum(
                            &u.seletores,
                            &crate::seletor::Elemento::do_template(e),
                        )
                };
                onde_casa(nos, &casa, filhos, false, &mut lugares);
                let na_visao = !lugares.is_empty()
                    && lugares
                        .iter()
                        .all(|l| matches!(l, Lugar::Raiz | Lugar::Projetado));
                if consulta.lista || !na_visao || u.diretiva.is_none() {
                    fora.push(recusa(
                        Motivo::ViewChildEmFilho,
                        "@ViewChild(Diretiva) fora da forma estática",
                    ));
                }
                continue;
            }
            let e_filho = |e: &crate::html::Elemento| {
                filhos
                    .get(&e.nome)
                    .is_some_and(|f| f.uri_dart == uri && f.classe == consulta.referencia)
            };
            onde_casa(nos, &e_filho, filhos, false, &mut lugares);
            let so_filhos = lugares
                .iter()
                .all(|l| matches!(l, Lugar::NoFilho | Lugar::NoFilhoProjetado));
            let elemento = e_tipo_de_elemento(&consulta.tipo, local, resolvedor);
            // O tipo tem de ser um componente de `directives:`: diretiva,
            // serviço ou `ElementRef` dariam outros resultados.
            let e_componente = filhos
                .values()
                .any(|f| f.uri_dart == uri && f.classe == consulta.referencia);
            if !e_componente || !so_filhos || elemento || (lugares.is_empty() && !consulta.lista) {
                fora.push(recusa(
                    Motivo::ViewChildEmFilho,
                    "@ViewChild(Tipo) fora da forma estática",
                ));
            }
            continue;
        }
        onde_esta(nos, &consulta.referencia, filhos, false, &mut lugares);
        // `#ref="x"`: o valor é a instância da diretiva exportada (o que o
        // `#ref` lê no mapa dos refs, `this._X_n_m`), atribuída como a de
        // um elemento — só na forma estática: um `#ref` só, na própria
        // visão (também no conteúdo projetado de um filho, caso j69),
        // `@ViewChild` (não lista).
        if referencia_com_valor(nos, &consulta.referencia) {
            // No elemento de um filho, a instância da diretiva exportada é
            // lida do mesmo jeito (caso j109).
            if !(matches!(
                lugares.as_slice(),
                [Lugar::Raiz | Lugar::Projetado | Lugar::NoFilho | Lugar::NoFilhoProjetado]
            ) && !consulta.lista)
            {
                fora.push(recusa(
                    Motivo::ViewChildEmFilho,
                    "@ViewChild de #ref com valor (exportAs) fora da forma estática",
                ));
            }
            continue;
        }
        // `@ViewChild('t')` de um `<template #t>` escrito ([`Corpo::molde`]):
        // o valor lido é o `TemplateRef` do nó (o `read` implícito de um
        // `<template>`), atribuído no `build()` como o de um elemento
        // (`_ctx.x = this._TemplateRef_n_7;`). Só a forma estática — um
        // `#t` só, na própria visão, campo `TemplateRef` do ngdart; lista,
        // resultado em `*` ou no conteúdo projetado ainda não.
        if moldes.contains(&consulta.referencia) {
            match lugares.as_slice() {
                [Lugar::Raiz]
                    if !consulta.lista && e_template_ref(&consulta.tipo, local, resolvedor) => {}
                [Lugar::Raiz] if !consulta.lista => fora.push(recusa(
                    Motivo::ViewChildEmFilho,
                    "@ViewChild de <template> em campo que não é TemplateRef",
                )),
                _ => fora.push(recusa(
                    Motivo::ViewChildDinamico,
                    "@ViewChild de <template> fora da forma estática",
                )),
            }
            continue;
        }
        // `@ViewChildren` estático, ou `@ViewChild` de `#ref` repetido (o
        // primeiro resultado): todos os resultados nesta visão, e do mesmo
        // jeito (nós com campo `Element`, ou instâncias de filho com campo
        // do tipo dele). Sem resultado, a lista recebe `[]`. Resultado em
        // `*` ainda não.
        if consulta_em_embutida(nos, consulta, filhos, local, resolvedor) {
            continue;
        }
        // A única cujo primeiro resultado (em pré-ordem) está nesta visão é
        // estática e fica com ele (`_isStatic`, `_buildQueryResults` para no
        // primeiro): os de dentro de `*` que vêm depois não contam (j17).
        if !consulta.lista && lugares.len() > 1 && lugares[0] != Lugar::Embutida {
            lugares.truncate(1);
        }
        if consulta.lista || lugares.len() > 1 {
            let elemento = e_tipo_de_elemento(&consulta.tipo, local, resolvedor);
            let todos_nos = lugares
                .iter()
                .all(|l| matches!(l, Lugar::Raiz | Lugar::Projetado));
            let todos_filhos = lugares
                .iter()
                .all(|l| matches!(l, Lugar::NoFilho | Lugar::NoFilhoProjetado));
            let ok = (de_provedor && todos_nos)
                || (!de_provedor && elemento && todos_nos)
                || (!de_provedor && !elemento && todos_filhos);
            if !ok {
                fora.push(recusa(
                    Motivo::ViewChildDinamico,
                    "@ViewChildren fora da forma estática",
                ));
            }
            continue;
        }
        let r = match lugares.as_slice() {
            // O nó ou `ElementRef(nó)`, pela leitura ([`valor_de_elemento`]):
            // a consulta estática, que sai no `build()`.
            [Lugar::Raiz | Lugar::Projetado]
                if valor_de_elemento(consulta, local, resolvedor).is_some() || de_provedor =>
            {
                continue;
            }
            [Lugar::Raiz | Lugar::Projetado] => recusa(
                Motivo::ViewChildEmFilho,
                "@ViewChild(.., read: T) de token que o elemento não provê",
            ),
            // A instância do filho: o campo tem de ser do tipo dele (não um
            // `Element`) e o filho não pode ser `onPush`, que registra o
            // `ChangeDetectorRef` da consulta (`queryChangeDetectorRefs`).
            [Lugar::NoFilho | Lugar::NoFilhoProjetado] => {
                if consulta.leitura.is_some() {
                    recusa(
                        Motivo::ViewChildEmFilho,
                        "@ViewChild(.., read: T) de #ref de filho",
                    )
                } else if e_tipo_de_elemento(&consulta.tipo, local, resolvedor) {
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
            // Sem resultado, a única não recebe nada (`_createUpdates`,
            // caso j82).
            [] => continue,
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

/// Um resultado de consulta de visão na árvore do `compile_query.dart`
/// (`_NestedQueryValues`): um nó desta visão, ou os resultados de dentro de
/// um `*` dela — em pré-ordem, que é a ordem em que o `CompileElement`
/// chama `addQueryResult` (no `beforeChildren`, com a visão embutida
/// construída na hora).
#[derive(Debug, Clone)]
enum ItemDeConsulta {
    /// O nó do `#ref` (elemento HTML) ou a instância do filho; `on_push`: o
    /// filho registra o `ChangeDetectorRef` (`buildChangeDetectorRef`).
    Valor {
        lugar: Lugar,
        on_push: bool,
        /// Quantos resultados da mesma chave vêm antes dele na mesma visão
        /// (o nó é lido por [`chave_de_ordinal`]).
        ordinal: usize,
    },
    /// Os resultados da visão embutida do `*` que começa em `estrela` (o
    /// início da ligação dele, que identifica o `*` no template).
    Aninhada {
        estrela: usize,
        itens: Vec<ItemDeConsulta>,
    },
}

/// A árvore dos resultados de `chave` em `nos` ([`ItemDeConsulta`]). O
/// `*` é seguido pelo conteúdo de elementos HTML e de `<ng-container>`,
/// também no conteúdo projetado de um filho; resultado numa diretiva de tag
/// ou dentro de `<template>` escrito ainda não se traduz.
fn arvore_da_consulta(
    nos: &[No],
    chave: &str,
    filhos: &std::collections::HashMap<String, Filho>,
) -> Result<Vec<ItemDeConsulta>, &'static str> {
    fn andar(
        nos: &[No],
        chave: &str,
        filhos: &std::collections::HashMap<String, Filho>,
        em_filho: bool,
        saida: &mut Vec<ItemDeConsulta>,
        vistos: &mut usize,
    ) -> Result<(), &'static str> {
        for n in nos {
            let No::Elemento(e) = n else { continue };
            if let Some(estrela) = &e.estrela {
                let mut dentro = Vec::new();
                if estrela.nome == MARCA_DE_MOLDE {
                    // O `<template>` escrito é desta visão; o conteúdo, da
                    // visão embutida dele, mapeada como a de um `*`
                    // (`_appEl_n.mapNestedViews`, caso j86). O próprio
                    // `<template>` como resultado ainda não se traduz.
                    if casa_a_chave(e, chave, filhos) {
                        return Err("resultado de consulta em <template> escrito");
                    }
                    andar(&e.filhos, chave, filhos, false, &mut dentro, &mut 0)?;
                    if !dentro.is_empty() {
                        saida.push(ItemDeConsulta::Aninhada {
                            estrela: estrela.inicio,
                            itens: dentro,
                        });
                    }
                    continue;
                }
                let mut sem = e.clone();
                sem.estrela = None;
                andar(
                    std::slice::from_ref(&No::Elemento(sem)),
                    chave,
                    filhos,
                    false,
                    &mut dentro,
                    &mut 0,
                )?;
                // O `*` no conteúdo projetado de um filho é desta visão (a
                // âncora é daqui; só o nó vai projetado): a consulta mapeia
                // a visão embutida do mesmo jeito (caso j77).
                if !dentro.is_empty() {
                    saida.push(ItemDeConsulta::Aninhada {
                        estrela: estrela.inicio,
                        itens: dentro,
                    });
                }
                continue;
            }
            let (lugar, abaixo) = if let Some(f) = filhos.get(&e.nome) {
                let lugar = if em_filho {
                    Lugar::NoFilhoProjetado
                } else {
                    Lugar::NoFilho
                };
                if casa_a_chave(e, chave, filhos) {
                    saida.push(ItemDeConsulta::Valor {
                        lugar,
                        on_push: f.on_push,
                        ordinal: *vistos,
                    });
                    *vistos += 1;
                }
                (lugar, true)
            } else if e.nome == "ng-container" {
                (Lugar::Raiz, em_filho)
            } else if !dom::tag_html(&e.nome) {
                (Lugar::Filho, true)
            } else if em_filho {
                (Lugar::Projetado, true)
            } else {
                (Lugar::Raiz, false)
            };
            if !filhos.contains_key(&e.nome) && casa_a_chave(e, chave, filhos) {
                saida.push(ItemDeConsulta::Valor {
                    lugar,
                    on_push: false,
                    ordinal: *vistos,
                });
                *vistos += 1;
            }
            andar(&e.filhos, chave, filhos, abaixo, saida, vistos)?;
        }
        Ok(())
    }
    let mut saida = Vec::new();
    andar(nos, chave, filhos, false, &mut saida, &mut 0)?;
    Ok(saida)
}

/// A consulta precisa do `mapNestedViews` (`_shouldMapNestedViews`): a
/// única quando o primeiro resultado está dentro de um `*`; a lista quando
/// algum está. Senão é estática (`createImmediateUpdates`).
fn consulta_e_dinamica(arvore: &[ItemDeConsulta], lista: bool) -> bool {
    if lista {
        arvore
            .iter()
            .any(|i| matches!(i, ItemDeConsulta::Aninhada { .. }))
    } else {
        matches!(arvore.first(), Some(ItemDeConsulta::Aninhada { .. }))
    }
}

/// Há resultado nesta visão, fora de `*` (`hasStaticValues`)?
fn tem_resultado_estatico(itens: &[ItemDeConsulta]) -> bool {
    itens
        .iter()
        .any(|i| matches!(i, ItemDeConsulta::Valor { .. }))
}

/// Algum resultado é componente `onPush`?
fn tem_detector(itens: &[ItemDeConsulta]) -> bool {
    itens.iter().any(|i| match i {
        ItemDeConsulta::Valor { on_push, .. } => *on_push,
        ItemDeConsulta::Aninhada { itens, .. } => tem_detector(itens),
    })
}

/// A consulta de visão com resultados dentro de `*` que se traduz pela
/// árvore ([`ItemDeConsulta`]): cada resultado é um elemento HTML (campo
/// `Element`, ou lido com `read: ElementRef`/`Element`) ou a instância de
/// um filho, e cada visão tem no máximo um (o nó é lido pelo nome do
/// `#ref` naquela visão). É a forma de `mapNestedViews` e
/// `mapNestedViewsWithSingleResult`, em qualquer profundidade, com vários
/// `*` e com resultados estáticos junto (i47, i96, i97, j06, j09, j10,
/// j14, j16, j17, j34).
fn consulta_em_embutida(
    nos: &[No],
    consulta: &crate::componente::Consulta,
    filhos: &std::collections::HashMap<String, Filho>,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> bool {
    let Some(chave) = chave_da_consulta(consulta, local, resolvedor) else {
        return false;
    };
    let Ok(arvore) = arvore_da_consulta(nos, &chave, filhos) else {
        return false;
    };
    if !consulta_e_dinamica(&arvore, consulta.lista) {
        return false;
    }
    let elemento = e_tipo_de_elemento(&consulta.tipo, local, resolvedor);
    let leitura = consulta.leitura.is_some();
    let terminal = |l: Lugar| match l {
        Lugar::Raiz if leitura => {
            !consulta.por_tipo
                && (valor_de_elemento(consulta, local, resolvedor).is_some()
                    || token_de_leitura(consulta, local, resolvedor).is_some())
        }
        Lugar::Raiz => elemento && !consulta.por_tipo,
        // O filho no conteúdo projetado noutro também é criado nesta visão
        // (a instância, como a do filho no próprio nó).
        Lugar::NoFilho | Lugar::NoFilhoProjetado => !leitura && !elemento,
        _ => false,
    };
    fn valida(itens: &[ItemDeConsulta], terminal: &dyn Fn(Lugar) -> bool) -> bool {
        itens.iter().all(|i| match i {
            ItemDeConsulta::Valor { lugar, .. } => terminal(*lugar),
            ItemDeConsulta::Aninhada { itens, .. } => valida(itens, terminal),
        })
    }
    valida(&arvore, &terminal)
}

/// As âncoras de cada `*` que contém resultado de consulta dinâmica, pelo
/// início da ligação da estrela: (`_appEl_n`, classe `_ViewX1` da visão
/// embutida), registradas quando o `*` é emitido.
type Ancoras = std::collections::HashMap<usize, (String, String)>;

/// Por visão embutida (o início da estrela), as consultas de conteúdo cujo
/// resultado está nela: (posição do primeiro resultado, campo sujo, quantas
/// `parentView` até a visão da consulta, classe dessa visão).
type SujasDeConteudo = std::collections::HashMap<usize, Vec<(usize, u8, String, u32, String)>>;

/// Um resultado de consulta de conteúdo: o nó que fornece o token (pela
/// posição, [`chave_de_no`]) ou uma visão embutida com resultados.
#[derive(Debug, Clone)]
enum ItemDeConteudo {
    Valor {
        inicio: usize,
        /// O componente `onPush` do nó é o resultado: o `ChangeDetectorRef`
        /// dele vai para o `View.queryChangeDetectorRefs`
        /// (`buildChangeDetectorRef`, `compile_element.dart:390-418`).
        on_push: bool,
    },
    Aninhada {
        estrela: usize,
        itens: Vec<ItemDeConteudo>,
    },
}

/// Uma consulta de conteúdo atualizada na detecção (`createDynamicUpdates`),
/// montada quando as âncoras de todas as visões são conhecidas.
#[derive(Debug, Clone)]
struct ConteudoDinamico {
    id: usize,
    arvore: Vec<ItemDeConteudo>,
    /// [`chave_de_tipo`] do token.
    chave: String,
    lista: bool,
    /// `this._FocusListDirective_0_5.instance.listItems`.
    alvo: String,
    /// A classe da visão da consulta.
    classe: String,
}

/// `@ViewChild(ren)(Token)` cujo token uma diretiva (ou os `providers:` dela)
/// fornece, com resultado em `*` (`_shouldMapNestedViews`): a árvore dos
/// resultados ([`arvore_de_token`]), para a máquina das consultas de
/// conteúdo com a raiz na visão do componente. `None` para o resto (o filho
/// da própria classe vai pela de sempre, [`consulta_em_embutida`]).
fn consulta_de_token_dinamica(
    nos: &[No],
    consulta: &crate::componente::Consulta,
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> Option<Vec<ItemDeConteudo>> {
    if !consulta.por_tipo || consulta.leitura.is_some() {
        return None;
    }
    let uri = uri_da_consulta(consulta, local, resolvedor)?;
    let classe = consulta.referencia.rsplit('.').next()?.to_string();
    if filhos
        .values()
        .any(|f| f.uri_dart == uri && f.classe == classe)
    {
        return None;
    }
    let token = crate::diretivas::Token::Classe {
        uri: uri.clone(),
        classe: classe.clone(),
    };
    let fornece = |d: &crate::diretivas::Diretiva| {
        (d.uri == uri && d.classe == classe) || d.provedores.iter().any(|p| p.token == token)
    };
    let alguem = usadas
        .iter()
        .any(|u| u.diretiva.as_deref().is_some_and(fornece))
        || filhos
            .values()
            .any(|f| f.metadados.as_deref().is_some_and(fornece));
    if !alguem {
        return None;
    }
    let arvore = arvore_de_token(nos, &uri, &classe, filhos, usadas).ok()?;
    let dinamica = if consulta.lista {
        arvore
            .iter()
            .any(|i| matches!(i, ItemDeConteudo::Aninhada { .. }))
    } else {
        matches!(arvore.first(), Some(ItemDeConteudo::Aninhada { .. }))
    };
    dinamica.then_some(arvore)
}

fn arvore_de_token(
    nos: &[No],
    uri: &str,
    classe: &str,
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
) -> Result<Vec<ItemDeConteudo>, Recusa> {
    let token = crate::diretivas::Token::Classe {
        uri: uri.to_string(),
        classe: classe.to_string(),
    };
    let fornece = |d: &crate::diretivas::Diretiva| {
        (d.uri == uri && d.classe == classe) || d.provedores.iter().any(|p| p.token == token)
    };
    let mut saida = Vec::new();
    for no in nos {
        let No::Elemento(x) = no else { continue };
        if let Some(estrela) = &x.estrela {
            let mut sem = x.clone();
            if estrela.nome != MARCA_DE_MOLDE {
                sem.estrela = None;
                let dentro = arvore_de_token(
                    std::slice::from_ref(&No::Elemento(sem)),
                    uri,
                    classe,
                    filhos,
                    usadas,
                )?;
                if !dentro.is_empty() {
                    saida.push(ItemDeConteudo::Aninhada {
                        estrela: estrela.inicio,
                        itens: dentro,
                    });
                }
                continue;
            }
            // `<template>` escrito: o nó é desta visão; o conteúdo, da
            // embutida dele.
            sem.estrela = None;
            if diretivas_casadas(usadas, &sem).iter().any(|d| fornece(d)) {
                return Err(recusa(
                    Motivo::LigacaoEmFilho,
                    "@ContentChildren com resultado no próprio <template>",
                ));
            }
            let dentro = arvore_de_token(&x.filhos, uri, classe, filhos, usadas)?;
            if !dentro.is_empty() {
                saida.push(ItemDeConteudo::Aninhada {
                    estrela: estrela.inicio,
                    itens: dentro,
                });
            }
            continue;
        }
        let filho = filhos.get(&x.nome);
        let casa = filho.is_some_and(|f| {
            (f.uri_dart == uri && f.classe == classe)
                || f.metadados.as_deref().is_some_and(|m| fornece(m))
        }) || diretivas_casadas(usadas, x).iter().any(|d| fornece(d));
        if casa {
            let on_push =
                filho.is_some_and(|f| f.on_push && f.uri_dart == uri && f.classe == classe);
            saida.push(ItemDeConteudo::Valor {
                inicio: x.inicio,
                on_push,
            });
        }
        saida.extend(arvore_de_token(&x.filhos, uri, classe, filhos, usadas)?);
    }
    Ok(saida)
}

/// A chave do nó em `inicio` que fornece o token de `chave`: o resultado de
/// uma consulta de conteúdo lido de outra visão.
fn chave_de_no(chave: &str, inicio: usize) -> String {
    format!("{chave}\u{3}@{inicio}")
}

const MARCA_DE_CONTEUDO: char = '\u{12}';
const FIM_DE_CONTEUDO: char = '\u{13}';

/// Marca da atualização de uma consulta de visão dinâmica na detecção:
/// `\u{e}campo|q..|d..|e..\u{f}`, trocada por [`resolver_consultas`] quando
/// as âncoras de todas as visões são conhecidas. Os qualificadores
/// (`firstOrNull` do `queries.dart`, `View`, `ElementRef`) vão na marca
/// para serem alocados na ordem do texto por [`resolver_tardios`].
const MARCA_DE_CONSULTA: char = '\u{e}';
const FIM_DE_CONSULTA: char = '\u{f}';

/// Monta o valor de uma consulta dinâmica como o `compile_query.dart`:
/// `_buildQueryResults` (a única para no primeiro resultado estático de
/// cada visão; com mais de um item, o `mapNestedViews` é espalhado com
/// `...`), `_mapNestedViews` (`mapNestedViewsWithSingleResult` quando a
/// visão dá um resultado só; os `ChangeDetectorRef` dos `onPush` antes do
/// `return`) e `_createUpdatesNested` (lista quando há mais de um valor; a
/// única pega `.first` se há resultado estático, senão `firstOrNull`). O
/// texto sai como o `DartFormatter` o deixa: cada fecho com o corpo dois
/// espaços para dentro da linha em que abre, cada item de lista numa linha.
struct MontagemDaConsulta<'a> {
    ancoras: &'a Ancoras,
    chave: &'a str,
    lista: bool,
    element_ref: Option<&'a str>,
    view: Option<&'a str>,
}

impl MontagemDaConsulta<'_> {
    /// Os valores e os registros de `ChangeDetectorRef` de uma visão:
    /// `receptor` é `this` na visão do componente e `nestedView` dentro de
    /// um fecho; `classe`, a da visão (a marca do nó é `nome@classe`).
    fn resultados(
        &self,
        itens: &[ItemDeConsulta],
        receptor: &str,
        classe: &str,
    ) -> Option<(Vec<String>, Vec<String>)> {
        let (mut valores, mut registros) = (Vec::new(), Vec::new());
        let ler = |marca: &str| {
            if receptor == "this" {
                format!("{MARCA_DE_REF}{marca}{FIM_DE_REF}")
            } else {
                format!("{receptor}{MARCA_DE_REF}.{marca}{FIM_DE_REF}")
            }
        };
        for item in itens {
            match item {
                ItemDeConsulta::Valor {
                    on_push, ordinal, ..
                } => {
                    let chave = chave_de_ordinal(self.chave, *ordinal);
                    let no = ler(&chave_de_ref(&chave, classe));
                    let valor = match self.element_ref {
                        Some(q) => format!("{q}ElementRef({no})"),
                        None => no,
                    };
                    if *on_push {
                        let v = self.view?;
                        let visao = ler(&format!(
                            "{MARCA_DE_DETECTOR}{}",
                            chave_de_ref(&chave, classe)
                        ));
                        registros.push(format!(
                            "{v}View.queryChangeDetectorRefs[{valor}] = {visao};"
                        ));
                    }
                    valores.push(valor);
                    if !self.lista {
                        break;
                    }
                }
                ItemDeConsulta::Aninhada {
                    estrela,
                    itens: dentro,
                } => {
                    let mapa = self.mapa(*estrela, dentro, receptor)?;
                    valores.push(if itens.len() > 1 {
                        format!("...{mapa}")
                    } else {
                        mapa
                    });
                }
            }
        }
        Some((valores, registros))
    }

    fn mapa(&self, estrela: usize, dentro: &[ItemDeConsulta], receptor: &str) -> Option<String> {
        let (ancora, classe) = self.ancoras.get(&estrela)?;
        let (valores, mut linhas) = self.resultados(dentro, "nestedView", classe)?;
        let varios = if self.lista {
            dentro.len() > 1
                || dentro
                    .iter()
                    .any(|i| matches!(i, ItemDeConsulta::Aninhada { .. }))
        } else {
            matches!(dentro.first(), Some(ItemDeConsulta::Aninhada { .. }))
        };
        let metodo = if varios {
            "mapNestedViews"
        } else {
            "mapNestedViewsWithSingleResult"
        };
        let corpo = if valores.len() > 1 {
            lista_literal(&valores)
        } else {
            valores.into_iter().next()?
        };
        linhas.push(format!("return {corpo};"));
        Some(format!(
            "{receptor}.{ancora}.{metodo}(({classe} nestedView) {{\n{}\n}})",
            indentar(&linhas.join("\n"), 2)
        ))
    }
}

/// `[a, b]`: numa linha, como o `DartFormatter` (página "infinita") a deixa,
/// a não ser que algum item tenha quebra (um fecho) — aí um item por linha.
fn lista_literal(valores: &[String]) -> String {
    if !valores.iter().any(|v| v.contains('\n')) {
        return format!("[{}]", valores.join(", "));
    }
    let itens: Vec<String> = valores.iter().map(|v| indentar(v, 2)).collect();
    format!("[\n{}\n]", itens.join(",\n"))
}

/// Troca cada [`MARCA_DE_CONSULTA`] pelas instruções da atualização da
/// consulta, com a indentação da linha em que a marca está.
fn resolver_consultas(
    texto: &str,
    consultas: &[ConsultaDinamica],
    ancoras: &Ancoras,
    classe_raiz: &str,
) -> String {
    let mut saida = texto.to_string();
    for q in consultas {
        let inicio = format!("{MARCA_DE_CONSULTA}{}", q.campo);
        while let Some(i) = saida.find(&inicio) {
            let depois = &saida[i + inicio.len()..];
            let Some(f) = depois.find(FIM_DE_CONSULTA) else {
                break;
            };
            // O campo termina em `_isDirty`: nenhum outro o tem de prefixo.
            let mut qualificadores = std::collections::HashMap::new();
            for parte in depois[..f].split('|').skip(1) {
                let (k, v) = parte.split_at(1);
                qualificadores.insert(k.to_string(), v.to_string());
            }
            let montagem = MontagemDaConsulta {
                ancoras,
                chave: &q.leitura,
                lista: q.lista,
                element_ref: qualificadores.get("e").map(String::as_str),
                view: qualificadores.get("d").map(String::as_str),
            };
            let Some((valores, mut linhas)) = montagem.resultados(&q.arvore, "this", classe_raiz)
            else {
                break;
            };
            let valor = if valores.len() == 1 {
                valores[0].clone()
            } else {
                lista_literal(&valores)
            };
            let valor = match (q.lista, qualificadores.get("q")) {
                (true, _) => valor,
                (false, Some(queries)) => format!("{queries}firstOrNull({valor})"),
                (false, None) => format!("{valor}.first"),
            };
            linhas.push(format!("_ctx.{} = {valor};", q.propriedade));
            let inicio_da_linha = saida[..i].rfind('\n').map_or(0, |k| k + 1);
            let recuo =
                saida[inicio_da_linha..i].len() - saida[inicio_da_linha..i].trim_start().len();
            let texto = linhas.join("\n");
            let texto = indentar(&texto, recuo);
            let fim = i + inicio.len() + f + FIM_DE_CONSULTA.len_utf8();
            saida.replace_range(i..fim, texto.trim_start());
        }
    }
    saida
}

/// Troca cada [`MARCA_DE_CONTEUDO`] pela atribuição da consulta de
/// conteúdo: `_buildQueryResults`/`_mapNestedViews`/`_createUpdatesNested`
/// (`compile_query.dart:200-530`) sobre a árvore dela, com a indentação da
/// linha da marca. Os nós saem como marcas de `#ref` ([`chave_de_no`]),
/// trocadas depois por [`resolver_refs`].
fn resolver_conteudo_dinamico(
    texto: &str,
    consultas: &[ConteudoDinamico],
    ancoras: &Ancoras,
) -> String {
    /// Os valores e os registros de `ChangeDetectorRef` (`view`: o `View`
    /// qualificado) de uma visão, como [`MontagemDaConsulta::resultados`].
    fn resultados(
        q: &ConteudoDinamico,
        itens: &[ItemDeConteudo],
        receptor: &str,
        classe: &str,
        ancoras: &Ancoras,
        view: Option<&str>,
    ) -> Option<(Vec<String>, Vec<String>)> {
        let (mut valores, mut registros) = (Vec::new(), Vec::new());
        let ler = |marca: &str| {
            if receptor == "this" {
                format!("{MARCA_DE_REF}{marca}{FIM_DE_REF}")
            } else {
                format!("{receptor}{MARCA_DE_REF}.{marca}{FIM_DE_REF}")
            }
        };
        for i in itens {
            match i {
                ItemDeConteudo::Valor { inicio, on_push } => {
                    let chave = chave_de_ref(&chave_de_no(&q.chave, *inicio), classe);
                    let valor = ler(&chave);
                    if *on_push {
                        let visao = ler(&format!("{MARCA_DE_DETECTOR}{chave}"));
                        registros.push(format!(
                            "{}View.queryChangeDetectorRefs[{valor}] = {visao};",
                            view?
                        ));
                    }
                    valores.push(valor);
                    // A única para no primeiro resultado estático da visão.
                    if !q.lista {
                        break;
                    }
                }
                ItemDeConteudo::Aninhada {
                    estrela,
                    itens: dentro,
                } => {
                    let (ancora, classe_w) = ancoras.get(estrela)?;
                    let (vals, mut linhas) =
                        resultados(q, dentro, "nestedView", classe_w, ancoras, view)?;
                    let varios = if q.lista {
                        dentro.len() > 1
                            || dentro
                                .iter()
                                .any(|x| matches!(x, ItemDeConteudo::Aninhada { .. }))
                    } else {
                        matches!(dentro.first(), Some(ItemDeConteudo::Aninhada { .. }))
                    };
                    let metodo = if varios {
                        "mapNestedViews"
                    } else {
                        "mapNestedViewsWithSingleResult"
                    };
                    let corpo = if vals.len() > 1 {
                        lista_literal(&vals)
                    } else {
                        vals.into_iter().next()?
                    };
                    linhas.push(format!("return {corpo};"));
                    let mapa = format!(
                        "{receptor}.{ancora}.{metodo}(({classe_w} nestedView) {{\n{}\n}})",
                        indentar(&linhas.join("\n"), 2)
                    );
                    valores.push(if itens.len() > 1 {
                        format!("...{mapa}")
                    } else {
                        mapa
                    });
                }
            }
        }
        Some((valores, registros))
    }
    let mut saida = texto.to_string();
    for q in consultas {
        let inicio = format!("{MARCA_DE_CONTEUDO}{}", q.id);
        while let Some(i) = saida.find(&inicio) {
            let depois = &saida[i + inicio.len()..];
            let Some(f) = depois.find(FIM_DE_CONTEUDO) else {
                break;
            };
            // O id termina na marca ou no `|`: nenhum outro o tem de prefixo.
            let resto = &depois[..f];
            if !(resto.is_empty() || resto.starts_with('|')) {
                break;
            }
            // `|q<queries>` e `|d<View>`, cada um só quando usado.
            let mut qualificadores = std::collections::HashMap::new();
            for parte in resto.split('|').skip(1) {
                if !parte.is_empty() {
                    let (k, v) = parte.split_at(1);
                    qualificadores.insert(k, v);
                }
            }
            let queries = qualificadores.get("q").copied();
            let view = qualificadores.get("d").copied();
            let Some((valores, registros)) =
                resultados(q, &q.arvore, "this", &q.classe, ancoras, view)
            else {
                break;
            };
            let valor = if valores.len() == 1 {
                valores[0].clone()
            } else {
                lista_literal(&valores)
            };
            let valor = match (q.lista, queries) {
                (true, _) => valor,
                (false, Some(queries)) => format!("{queries}firstOrNull({valor})"),
                (false, None) => format!("{valor}.first"),
            };
            let mut texto = registros.join("\n");
            if !texto.is_empty() {
                texto.push('\n');
            }
            texto += &format!("{} = {valor};", q.alvo);
            let inicio_da_linha = saida[..i].rfind('\n').map_or(0, |k| k + 1);
            let recuo =
                saida[inicio_da_linha..i].len() - saida[inicio_da_linha..i].trim_start().len();
            let texto = indentar(&texto, recuo);
            let fim = i + inicio.len() + f + FIM_DE_CONTEUDO.len_utf8();
            saida.replace_range(i..fim, texto.trim_start());
        }
    }
    saida
}

/// Uma consulta de visão atualizada na detecção (`createDynamicUpdates`):
/// o campo "sujo", a âncora e a classe da visão embutida do resultado.
#[derive(Debug, Clone)]
struct ConsultaDinamica {
    indice: usize,
    propriedade: String,
    lista: bool,
    /// O que procurar ([`chave_da_consulta`]).
    chave: String,
    /// Como o resultado é lido no nó: a própria chave, ou a do provedor que
    /// o `read:` pede ([`chave_de_leitura`]).
    leitura: String,
    /// `_viewQuery_ref_N_isDirty`.
    campo: String,
    /// `read: ElementRef`: o resultado é `ElementRef(nó)`.
    element_ref: bool,
    /// Os resultados em pré-ordem ([`arvore_da_consulta`]).
    arvore: Vec<ItemDeConsulta>,
    /// Algum `*` com resultado foi emitido (sem ele, a visão foi recusada
    /// e a consulta não se escreve).
    vista: bool,
}

/// `<template [ngIf]="x">…</template>` escrito à mão é o mesmo
/// `EmbeddedTemplateAst` que `<ng-container *ngIf="x">…</ng-container>`:
/// os filhos são as raízes da visão embutida. Só a forma com uma ligação
/// a uma diretiva estrutural conhecida de uma entrada só (`ngIf`,
/// `ngSwitchCase`, `ngSwitchWhen`), ou só com o atributo `ngSwitchDefault`,
/// e nada mais é reescrita — com o
/// intervalo da ligação, que é o do `REF`; o resto continua `<template>`
/// e é recusado.
fn template_como_container(nos: &[No]) -> Vec<No> {
    nos.iter()
        .map(|n| match n {
            No::Elemento(e)
                if e.estrela.as_ref().is_some_and(|l| {
                    l.nome != MARCA_DE_MOLDE && Estrutural::conhecida(&l.nome).is_none()
                }) =>
            {
                // `*dir` de outra diretiva: o `<template>` que a
                // microssintaxe produz, com o elemento dentro, pelo caminho
                // do `<template>` escrito.
                template_como_container(std::slice::from_ref(&No::Elemento(template_da_estrela(e))))
                    .remove(0)
            }
            No::Elemento(e) => {
                let mut e = e.clone();
                e.filhos = template_como_container(&e.filhos);
                let limpo = e.eventos.is_empty()
                    && e.bananas.is_empty()
                    && e.referencias.is_empty()
                    && e.anotacoes.is_empty()
                    && e.estrela.is_none();
                let ligacao = match (e.atributos.as_slice(), e.propriedades.as_slice()) {
                    ([], [p])
                        if matches!(p.nome.as_str(), "ngIf" | "ngSwitchCase" | "ngSwitchWhen")
                            && !crate::micro::e_micro(p.valor.trim()) =>
                    {
                        Some(p.clone())
                    }
                    // `<template ngSwitchDefault>`: o atributo só casa o
                    // seletor, sem entrada.
                    ([a], []) if a.nome == "ngSwitchDefault" && a.valor.is_empty() => {
                        Some(a.clone())
                    }
                    _ => None,
                };
                if let (true, "template", Some(l)) = (limpo, e.nome.as_str(), ligacao) {
                    e.estrela = Some(l);
                    e.propriedades.clear();
                    e.atributos.clear();
                    e.nome = "ng-container".into();
                } else if let (true, "template", Some((estrela, ligacoes, micro))) =
                    (limpo, e.nome.as_str(), molde_com_diretiva(&e))
                {
                    e.estrela = Some(estrela);
                    e.ligacoes_do_molde = ligacoes;
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
                    // `<template>` com (no máximo) um `#ref`, `let-x`, e os
                    // atributos, `[x]` e `(x)` das diretivas dele: âncora,
                    // `ViewContainer` e `TemplateRef`, com o conteúdo numa
                    // visão embutida ([`Corpo::molde`], que resolve as
                    // diretivas e recusa o que nenhuma recebe).
                    // O início é o do `<template>`: identifica a âncora
                    // dele nas consultas de visão, como o de um `*`.
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
            outro => outro.clone(),
        })
        .collect()
}

/// O `<template>` de um `*dir="..."` (`micro/parser.dart` do ngast):
/// `[dir]="expr"` e `[dirChave]="expr"` para as ligações, `let-x="chave"`
/// para os locais, e o atributo `dir` vazio quando a primeira ligação não é
/// a da própria diretiva (`*dir` sozinho, `*dir="let x of xs"`). Toda ligação
/// tem o intervalo do atributo `*dir` inteiro, que é o do `REF` (caso j94).
/// O elemento vai dentro, sem a estrela; o `<template>` começa onde a
/// estrela começa (a âncora dele nas consultas).
fn template_da_estrela(e: &crate::html::Elemento) -> crate::html::Elemento {
    let estrela = e.estrela.clone().unwrap_or_default();
    let micro = e.micro_da_estrela().unwrap_or_default();
    let ligacao = |nome: &str, valor: &str| crate::html::Ligacao {
        nome: nome.to_string(),
        valor: valor.to_string(),
        inicio: estrela.inicio,
        fim: estrela.fim,
        sem_valor: false,
    };
    let mut t = crate::html::Elemento {
        nome: "template".into(),
        inicio: estrela.inicio,
        ..Default::default()
    };
    if micro
        .propriedades
        .first()
        .is_none_or(|(p, _)| *p != estrela.nome)
    {
        // O `dir` do desaçúcar não tem valor (`EmptyExpr`: `true` numa
        // entrada `bool`, o `*deferredContent="forceContent: x"`).
        t.atributos.push(crate::html::Ligacao {
            sem_valor: true,
            ..ligacao(&estrela.nome, "")
        });
    }
    for (nome, chave) in &micro.locais {
        let valor = if chave == "$implicit" {
            ""
        } else {
            chave.as_str()
        };
        t.atributos.push(ligacao(&format!("let-{nome}"), valor));
    }
    for (nome, expr) in &micro.propriedades {
        t.propriedades.push(ligacao(nome, expr));
    }
    let mut dentro = e.clone();
    dentro.estrela = None;
    dentro.micro_do_molde = None;
    t.filhos = vec![No::Elemento(dentro)];
    t
}

/// `<template dir let-x let-y="chave" [dirA]="a" [dirB]="b">` escrito à
/// mão é exatamente o `<template>` que a microssintaxe de
/// `*dir="let x; let y = chave; a: a; b: b"` produz (`micro/parser.dart` do
/// ngast desfaz o `*` nesses atributos): o mesmo `EmbeddedTemplateAst`, com
/// a diretiva casada pelo seletor (`[ngFor][ngForOf]`). Reescrito como a
/// `estrela` equivalente, ele segue o caminho do `*`; a única coisa que o
/// `*` não guarda é o intervalo de cada ligação escrita — o `REF` de cada
/// entrada —, devolvido à parte.
///
/// Só a forma sem ambiguidade: a diretiva estrutural conhecida casada por
/// um atributo sem valor com o nome dela (`<template ngFor [ngForOf]="a">`,
/// que é `*ngFor="of: a"`) ou pela própria ligação `[dir]` (`<template
/// [ngTemplateOutlet]="t" [ngTemplateOutletValue]="v">`, que é
/// `*ngTemplateOutlet="t; value: v"`: a expressão da diretiva vem primeiro);
/// `let-x` (com ou sem valor); e as outras ligações `[dirX]` com o prefixo
/// da diretiva e sem `;` na expressão (que a microssintaxe separaria). O
/// resto continua `<template>` e é recusado.
fn molde_com_diretiva(
    e: &crate::html::Elemento,
) -> Option<(
    crate::html::Ligacao,
    Vec<crate::html::Ligacao>,
    crate::micro::Micro,
)> {
    let (lets, outros): (Vec<_>, Vec<_>) =
        e.atributos.iter().partition(|a| a.nome.starts_with("let-"));
    let mut partes = Vec::new();
    // A microssintaxe equivalente, já decomposta: o texto (`partes`) só
    // serve de descrição, porque `a ? b : c; value: x` nem passaria no
    // `isMicroExpression` (caso j75).
    let mut micro = crate::micro::Micro::default();
    let (dir, propriedades) = match outros.as_slice() {
        [dir] if dir.valor.is_empty() && Estrutural::conhecida(&dir.nome).is_some() => {
            if e.propriedades.is_empty() {
                return None;
            }
            ((*dir).clone(), e.propriedades.iter().collect::<Vec<_>>())
        }
        [] => {
            let (casadas, resto): (Vec<_>, Vec<_>) = e
                .propriedades
                .iter()
                .partition(|p| Estrutural::conhecida(&p.nome).is_some());
            let [dir] = casadas.as_slice() else {
                return None;
            };
            if dir.valor.trim().is_empty() {
                return None;
            }
            partes.push(dir.valor.trim().to_string());
            micro
                .propriedades
                .push((dir.nome.clone(), dir.valor.trim().to_string()));
            ((*dir).clone(), resto)
        }
        _ => return None,
    };
    for l in &lets {
        let nome = &l.nome["let-".len()..];
        if nome.is_empty()
            || !nome
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        {
            return None;
        }
        let chave = l.valor.trim();
        if chave.contains([';', '=']) {
            return None;
        }
        partes.push(if chave.is_empty() {
            format!("let {nome}")
        } else {
            format!("let {nome} = {chave}")
        });
        micro.locais.push((
            nome.to_string(),
            if chave.is_empty() {
                "$implicit".to_string()
            } else {
                chave.to_string()
            },
        ));
    }
    for p in &propriedades {
        let sufixo = p.nome.strip_prefix(dir.nome.as_str())?;
        let mut cs = sufixo.chars();
        let primeira = cs.next()?;
        if !primeira.is_ascii_uppercase() || p.valor.trim().is_empty() {
            return None;
        }
        partes.push(format!(
            "{}{}: {}",
            primeira.to_ascii_lowercase(),
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
            valor: partes.join("; "),
            inicio: dir.inicio,
            fim: dir.fim,
            sem_valor: false,
        },
        e.propriedades.clone(),
        micro,
    ))
}

/// Os nomes de `#ref` declarados mais de uma vez no template, ou com o
/// nome de um `let` de algum `*`: cada visão resolve o nome pelo escopo
/// dela (o mais próximo vence), o que ainda não é traduzido.
fn referencias_ambiguas(nos: &[No]) -> std::collections::HashSet<String> {
    fn andar(nos: &[No], refs: &mut Vec<String>, lets: &mut Vec<String>) {
        for n in nos {
            if let No::Elemento(e) = n {
                refs.extend(e.referencias.iter().map(|r| r.nome.clone()));
                if let Some(micro) = e.micro_da_estrela() {
                    lets.extend(micro.locais.into_iter().map(|(nome, _)| nome));
                }
                andar(&e.filhos, refs, lets);
            }
        }
    }
    let (mut refs, mut lets) = (Vec::new(), Vec::new());
    andar(nos, &mut refs, &mut lets);
    refs.iter()
        .filter(|n| refs.iter().filter(|x| x == n).count() > 1 || lets.contains(n))
        .cloned()
        .collect()
}

/// Os campos dos nós na ordem em que o `NodeReferenceStorageVisitor` os
/// promove: pela primeira leitura fora do `build()`. No
/// `detectChangesInternal` as declarações dos locais (`final local_x =
/// this._el_n;`) vêm no topo, então os nós lidos por elas vêm antes dos que
/// as ligações leem (em ordem de documento).
fn campos_el_em_ordem(
    campos_el: &[String],
    refs_dos_campos: &std::collections::HashMap<String, Vec<String>>,
    locais_raiz: &[String],
) -> Vec<String> {
    let mut saida: Vec<String> = Vec::new();
    for nome in locais_raiz {
        for c in campos_el {
            let lido = refs_dos_campos.get(c).is_some_and(|r| r.contains(nome));
            if lido && !saida.contains(c) {
                saida.push(c.clone());
            }
        }
    }
    for c in campos_el {
        if !saida.contains(c) {
            saida.push(c.clone());
        }
    }
    saida
}

/// Os `#ref` lidos como local cujo nó vira campo: os sem valor. Com valor
/// (`#d="x"`), o local é a diretiva exportada, e o elemento fica como está.
fn nos_promovidos(
    nos: &[No],
    refs: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    refs.iter()
        .filter(|n| !referencia_com_valor(nos, n))
        .cloned()
        .collect()
}

/// Algum `#nome="valor"` (com valor) no template, em qualquer profundidade?
fn referencia_com_valor(nos: &[No], nome: &str) -> bool {
    nos.iter().any(|n| match n {
        No::Elemento(e) => {
            e.referencias
                .iter()
                .any(|r| r.nome == nome && !r.valor.is_empty())
                || referencia_com_valor(&e.filhos, nome)
        }
        _ => false,
    })
}

/// Os `#ref` dos `<template>` escritos ([`MARCA_DE_MOLDE`]), em qualquer
/// profundidade.
fn referencias_de_moldes(nos: &[No]) -> Vec<String> {
    let mut saida = Vec::new();
    for n in nos {
        if let No::Elemento(e) = n {
            if e.estrela.as_ref().is_some_and(|l| l.nome == MARCA_DE_MOLDE) {
                saida.extend(e.referencias.iter().map(|r| r.nome.clone()));
            }
            saida.extend(referencias_de_moldes(&e.filhos));
        }
    }
    saida
}

/// Onde um `#ref` aparece no template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lugar {
    /// Num elemento HTML da própria visão.
    Raiz,
    /// Dentro de uma visão embutida (`*ngIf`, `*ngFor`).
    Embutida,
    /// Numa tag que não é HTML nem componente (a de uma diretiva), ou
    /// abaixo dela: forma ainda recusada para o `@ViewChild`.
    Filho,
    /// No próprio elemento de um componente filho da visão: vale a
    /// instância.
    NoFilho,
    /// Num elemento HTML do conteúdo projetado num filho: ainda é um nó
    /// desta visão (criado solto e entregue no `createAndProject`).
    Projetado,
    /// Num componente filho dentro do conteúdo projetado noutro: a
    /// instância, também desta visão.
    NoFilhoProjetado,
}

fn onde_esta(
    nos: &[No],
    nome: &str,
    filhos: &std::collections::HashMap<String, Filho>,
    em_filho: bool,
    saida: &mut Vec<Lugar>,
) {
    let tem_ref = |e: &crate::html::Elemento| casa_a_chave(e, nome, filhos);
    onde_casa(nos, &tem_ref, filhos, em_filho, saida);
}

/// O que uma consulta procura, como chave de [`onde_esta`]: o `#ref`, ou
/// [`chave_de_tipo`] do componente de `@ViewChild(Tipo)` (`None` sem
/// resolução).
fn chave_da_consulta(
    consulta: &crate::componente::Consulta,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> Option<String> {
    if consulta.por_tipo {
        uri_da_consulta(consulta, local, resolvedor)
            .map(|u| chave_de_tipo(&u, &consulta.referencia))
    } else {
        Some(consulta.referencia.clone())
    }
}

/// O elemento casa a chave de uma consulta: tem o `#ref`, ou é um
/// componente filho do tipo de [`chave_de_tipo`].
fn casa_a_chave(
    e: &crate::html::Elemento,
    chave: &str,
    filhos: &std::collections::HashMap<String, Filho>,
) -> bool {
    match chave.strip_prefix('\u{7}').and_then(|c| c.rsplit_once('#')) {
        Some((uri, classe)) => filhos
            .get(&e.nome)
            .is_some_and(|f| f.uri_dart == uri && f.classe == classe),
        None => e.referencias.iter().any(|r| r.nome == chave),
    }
}

/// Onde o `ViewBuilder` acha o primeiro resultado de `chave` dentro de um
/// `*`: é aí que a consulta ganha o campo `_isDirty`
/// (`_setParentQueryAsDirty`, alocado na primeira vez), e o `ViewStorage`
/// escreve os campos na ordem de alocação. O `ViewBuilder` desce nas visões
/// embutidas na hora (`visitEmbeddedTemplate`) e o resultado entra no
/// `beforeChildren` do elemento: pré-ordem do template inteiro. No mesmo
/// elemento, os provedores (consulta por tipo) vêm antes das referências,
/// e as referências na ordem em que estão escritas. `None`: nenhum
/// resultado dinâmico.
fn primeiro_resultado_dinamico(
    nos: &[No],
    chave: &str,
    filhos: &std::collections::HashMap<String, Filho>,
) -> Option<(usize, usize)> {
    fn andar(
        nos: &[No],
        chave: &str,
        filhos: &std::collections::HashMap<String, Filho>,
        dinamico: bool,
        ordinal: &mut usize,
    ) -> Option<(usize, usize)> {
        for n in nos {
            let No::Elemento(e) = n else { continue };
            let aqui = *ordinal;
            *ordinal += 1;
            let dinamico = dinamico || e.estrela.is_some();
            if dinamico && casa_a_chave(e, chave, filhos) {
                let lugar = if chave.starts_with('\u{7}') {
                    0
                } else {
                    1 + e
                        .referencias
                        .iter()
                        .position(|r| r.nome == chave)
                        .unwrap_or(0)
                };
                return Some((aqui, lugar));
            }
            if let Some(r) = andar(&e.filhos, chave, filhos, dinamico, ordinal) {
                return Some(r);
            }
        }
        None
    }
    andar(nos, chave, filhos, false, &mut 0)
}

/// Como [`primeiro_resultado_dinamico`], com a posição do nó
/// ([`crate::html::Elemento::inicio`]) do primeiro resultado dentro de `*`.
fn inicio_do_primeiro_resultado_dinamico(
    nos: &[No],
    chave: &str,
    filhos: &std::collections::HashMap<String, Filho>,
) -> Option<usize> {
    fn andar(
        nos: &[No],
        chave: &str,
        filhos: &std::collections::HashMap<String, Filho>,
        dinamico: bool,
    ) -> Option<usize> {
        for n in nos {
            let No::Elemento(e) = n else { continue };
            let dinamico = dinamico || e.estrela.is_some();
            if dinamico && casa_a_chave(e, chave, filhos) {
                return Some(e.inicio);
            }
            if let Some(r) = andar(&e.filhos, chave, filhos, dinamico) {
                return Some(r);
            }
        }
        None
    }
    andar(nos, chave, filhos, false)
}

/// A posição do primeiro nó desta visão (fora de `*`) que casa `chave`.
fn inicio_do_primeiro_resultado_na_visao(
    nos: &[No],
    chave: &str,
    filhos: &std::collections::HashMap<String, Filho>,
) -> Option<usize> {
    nos.iter().find_map(|n| {
        let No::Elemento(e) = n else { return None };
        if e.estrela.is_some() {
            return None;
        }
        if casa_a_chave(e, chave, filhos) {
            return Some(e.inicio);
        }
        inicio_do_primeiro_resultado_na_visao(&e.filhos, chave, filhos)
    })
}

/// Os campos com inicializador de uma visão, que abrem a classe
/// (`dart_emitter.dart:198-205`), na ordem em que o `ViewStorage` os aloca:
/// o provedor preguiçoso quando o nó dele é criado, o campo sujo de uma
/// consulta quando o primeiro resultado dela em `*` é visitado (pré-ordem
/// do template inteiro, as visões embutidas no lugar). No mesmo nó, o
/// provedor vem antes, e a consulta de conteúdo antes da de visão
/// (`_getQueriesFor`).
fn campos_com_inicializador(corpo: &Corpo) -> Vec<String> {
    let mut todos: Vec<(usize, u8, usize, String)> = Vec::new();
    for (k, c) in corpo.campos_preguicosos.iter().enumerate() {
        let p = corpo.posicoes_preguicosas.get(k).copied().unwrap_or(0);
        todos.push((p, 0, k, c.clone()));
    }
    for (k, (p, c)) in corpo.sujos_de_conteudo.iter().enumerate() {
        todos.push((*p, 1, k, c.clone()));
    }
    for (k, (p, c)) in corpo.sujos_de_visao.iter().enumerate() {
        todos.push((*p, 2, k, c.clone()));
    }
    todos.sort_by_key(|(p, tipo, k, _)| (*p, *tipo, *k));
    todos.into_iter().map(|(_, _, _, c)| c).collect()
}

/// Marca, no início de uma raiz ou item de projeção, de que ele já é uma
/// lista (`this.projectedNodes[i]`, `o.ArrayType`).
const RAIZ_LISTA: char = '\u{11}';

/// Um texto com `{{ }}` convertido ([`Corpo::valor_interpolado`]).
struct Interpolada {
    convertidas: Vec<crate::expr::Convertida>,
    imutavel: bool,
    /// O número da ligação (`_expr_k`, `currVal_k`).
    k: u32,
    /// O valor calculado uma vez, quando imutável.
    constante: String,
    /// O `currVal_k` conferido.
    checagem: String,
    /// O valor onde ele é usado (`currVal_k`, ou interpolado ali).
    na_acao: String,
}

/// Uma lista de nós como o `createFlatArrayForProjectNodes` a escreve.
enum ListaPlana {
    /// `const <Object>[]`.
    Vazia,
    /// `<Object>[a, b]`, com os itens.
    Literal(Vec<String>),
    /// Qualquer outra expressão (uma lista projetada, ou a concatenação).
    Expressao(String),
    /// Duas ou mais seções `..addAll(x)`: o `dart format` põe cada uma na
    /// sua linha, dois espaços além do início da expressão.
    Cascata { cabeca: String, secoes: Vec<String> },
}

impl ListaPlana {
    fn texto(&self) -> String {
        self.texto_em(0)
    }

    /// O texto com a expressão começando na coluna `coluna` (a das seções
    /// da cascata é `coluna + 2`).
    fn texto_em(&self, coluna: usize) -> String {
        match self {
            ListaPlana::Vazia => "const <Object>[]".to_string(),
            ListaPlana::Literal(itens) => format!("<Object>[{}]", itens.join(", ")),
            ListaPlana::Expressao(e) => e.clone(),
            ListaPlana::Cascata { cabeca, secoes } => {
                let recuo = " ".repeat(coluna + 2);
                let mut t = cabeca.clone();
                for x in secoes {
                    t.push_str(&format!("\n{recuo}..addAll({x})"));
                }
                t
            }
        }
    }
}

/// `createFlatArrayForProjectNodes` (`view_compiler_utils.dart`): os itens
/// que não são lista vão juntos num `<Object>[..]`; cada lista projetada
/// ([`RAIZ_LISTA`]) é concatenada com `..addAll(..)` — a que não abre a
/// expressão, dentro de `unsafeCast`. Uma lista sozinha é ela mesma. Com
/// mais de uma concatenação, o `dart format` do builder quebra a cascata em
/// linhas, forma ainda sem caso.
fn lista_plana(itens: &[String], util: &str) -> Result<ListaPlana, Recusa> {
    let lista = |x: &String| x.strip_prefix(RAIZ_LISTA).map(str::to_string);
    match itens {
        [] => return Ok(ListaPlana::Vazia),
        [x] => {
            return Ok(match lista(x) {
                Some(l) => ListaPlana::Expressao(l),
                None => ListaPlana::Literal(vec![x.clone()]),
            });
        }
        _ => {}
    }
    if !itens.iter().any(|x| x.starts_with(RAIZ_LISTA)) {
        return Ok(ListaPlana::Literal(itens.to_vec()));
    }
    let mut soltos: Vec<String> = Vec::new();
    // A cabeça (o primeiro literal) e cada seção `..addAll(..)`.
    let mut resultado: Option<String> = None;
    let mut secoes: Vec<String> = Vec::new();
    let juntar =
        |resultado: &mut Option<String>, parte: String, secoes: &mut Vec<String>| match resultado {
            None => *resultado = Some(parte),
            Some(_) => secoes.push(parte),
        };
    for x in itens {
        match lista(x) {
            Some(l) => {
                if !soltos.is_empty() {
                    let parte = format!("<Object>[{}]", soltos.join(", "));
                    juntar(&mut resultado, parte, &mut secoes);
                    soltos.clear();
                }
                let parte = if resultado.is_none() {
                    format!("<Object>[{l}]")
                } else {
                    format!("{util}.unsafeCast({l})")
                };
                juntar(&mut resultado, parte, &mut secoes);
            }
            None => soltos.push(x.clone()),
        }
    }
    if !soltos.is_empty() {
        let parte = format!("<Object>[{}]", soltos.join(", "));
        juntar(&mut resultado, parte, &mut secoes);
    }
    let cabeca = resultado.unwrap_or_default();
    // Uma seção fica na linha (`a..addAll(b)`); duas ou mais, o `dart
    // format` quebra (o `icon_tooltip`, o `modal`).
    Ok(match secoes.as_slice() {
        [] => ListaPlana::Expressao(cabeca),
        [x] => ListaPlana::Expressao(format!("{cabeca}..addAll({x})")),
        _ => ListaPlana::Cascata { cabeca, secoes },
    })
}

/// Quantos `<ng-content>` há em `nos`, em qualquer profundidade.
fn conteudos_em(nos: &[No]) -> u32 {
    nos.iter()
        .map(|n| match n {
            No::Conteudo { .. } => 1,
            No::Elemento(e) => conteudos_em(&e.filhos),
            _ => 0,
        })
        .sum()
}

/// A chave dos resultados de um `@ViewChild(Tipo)` entre os `#ref` vistos
/// (`Corpo::refs_em_ordem`): não colide com nome de referência.
fn chave_de_tipo(uri: &str, classe: &str) -> String {
    format!("\u{7}{uri}#{classe}")
}

/// A biblioteca que declara o tipo de um `@ViewChild(Tipo)`, no escopo do
/// componente.
fn uri_da_consulta(
    consulta: &crate::componente::Consulta,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> Option<String> {
    resolvedor?.uri_do_tipo(local.caminho, &consulta.referencia)
}

/// Onde estão, em ordem de documento, os elementos que casam `casa`.
/// Onde está, em pré-ordem, o primeiro resultado da consulta `chave` que é
/// desta visão (fora de `*` e do conteúdo de `<template>`): o ordinal do
/// elemento e, dentro dele, a ordem do `_QueryWithRead` — as consultas por
/// tipo (provedores do nó) antes das de `#ref`, estas na ordem dos `#ref`.
/// `None` quando não há (vai para o fim).
fn primeiro_resultado(
    nos: &[No],
    chave: &str,
    filhos: &std::collections::HashMap<String, Filho>,
) -> Option<(usize, usize)> {
    fn andar(
        nos: &[No],
        chave: &str,
        filhos: &std::collections::HashMap<String, Filho>,
        ordinal: &mut usize,
    ) -> Option<(usize, usize)> {
        for no in nos {
            let No::Elemento(e) = no else { continue };
            *ordinal += 1;
            let molde = e.estrela.as_ref().is_some_and(|l| l.nome == MARCA_DE_MOLDE);
            if e.estrela.is_some() && !molde {
                continue;
            }
            if casa_a_chave(e, chave, filhos) {
                let dentro = if chave.starts_with('\u{7}') {
                    0
                } else {
                    1 + e
                        .referencias
                        .iter()
                        .position(|r| r.nome == chave)
                        .unwrap_or(0)
                };
                return Some((*ordinal, dentro));
            }
            if !molde && let Some(p) = andar(&e.filhos, chave, filhos, ordinal) {
                return Some(p);
            }
        }
        None
    }
    let mut ordinal = 0;
    andar(nos, chave, filhos, &mut ordinal)
}

fn onde_casa(
    nos: &[No],
    casa: &dyn Fn(&crate::html::Elemento) -> bool,
    filhos: &std::collections::HashMap<String, Filho>,
    em_filho: bool,
    saida: &mut Vec<Lugar>,
) {
    for no in nos {
        let No::Elemento(e) = no else { continue };
        // O `<template>` escrito é desta visão; o conteúdo dele, não.
        let molde = e.estrela.as_ref().is_some_and(|l| l.nome == MARCA_DE_MOLDE);
        let lugar = if e.estrela.is_some() && !molde {
            Lugar::Embutida
        } else if filhos.contains_key(&e.nome) {
            if em_filho {
                Lugar::NoFilhoProjetado
            } else {
                Lugar::NoFilho
            }
        } else if !dom::tag_html(&e.nome) && e.nome != "ng-container" {
            // `<ng-container>` (também o `<template [ngIf]>` reescrito) não
            // é nó: os filhos dele ficam onde ele está (caso j78).
            Lugar::Filho
        } else if em_filho {
            Lugar::Projetado
        } else {
            Lugar::Raiz
        };
        if casa(e) {
            saida.push(lugar);
        }
        let mut dentro = Vec::new();
        let abaixo_de_filho = em_filho
            || matches!(
                lugar,
                Lugar::Filho | Lugar::NoFilho | Lugar::NoFilhoProjetado | Lugar::Projetado
            );
        onde_casa(&e.filhos, casa, filhos, abaixo_de_filho, &mut dentro);
        // Tudo abaixo de um `*` é da visão embutida.
        if lugar == Lugar::Embutida || molde {
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

/// O valor de uma consulta de visão cujo resultado é um elemento HTML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValorDeElemento {
    /// O próprio nó (`_el_n`).
    No,
    /// `ElementRef(_el_n)`.
    ElementRef,
}

/// O que um elemento dá a uma consulta (`compile_element.dart`, laço de
/// `queriesWithReads`): com `read: ElementRef`, o `ElementRef` do nó; com
/// `read: Element`/`HtmlElement` (do `dart:html`), o nó; sem `read:`, o nó
/// se o campo é `Element` (`isElementType`), senão o `ElementRef`. `None`
/// para `read:` de outro token (diretiva, `ViewContainerRef`…), que ainda
/// não se escreve.
fn valor_de_elemento(
    consulta: &crate::componente::Consulta,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> Option<ValorDeElemento> {
    let Some(t) = consulta.leitura.as_deref() else {
        return Some(if e_tipo_de_elemento(&consulta.tipo, local, resolvedor) {
            ValorDeElemento::No
        } else {
            ValorDeElemento::ElementRef
        });
    };
    let uri = resolvedor.and_then(|r| r.uri_do_tipo(local.caminho, t))?;
    match (t, uri.as_str()) {
        ("ElementRef", ELEMENT_REF) => Some(ValorDeElemento::ElementRef),
        ("Element" | "HtmlElement", "dart:html") => Some(ValorDeElemento::No),
        _ => None,
    }
}

/// O token que o `read:` de uma consulta pede a um provedor do nó (não o
/// nó nem `ElementRef`, que [`valor_de_elemento`] lê, nem um embutido): a
/// classe, pela biblioteca que a declara. O nó a cria ansiosa
/// (`queriedTokens`) e a consulta recebe a instância.
fn token_de_leitura(
    consulta: &crate::componente::Consulta,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> Option<crate::diretivas::Token> {
    let t = consulta.leitura.as_deref()?;
    if valor_de_elemento(consulta, local, resolvedor).is_some() {
        return None;
    }
    let uri = resolvedor?.uri_do_tipo(local.caminho, t)?;
    let token = crate::diretivas::Token::Classe {
        uri,
        classe: t.to_string(),
    };
    (!token.embutido()).then_some(token)
}

/// A marca do resultado de `chave` lido pelo provedor `t` do nó.
fn chave_de_leitura(chave: &str, t: &crate::diretivas::Token) -> String {
    match t {
        crate::diretivas::Token::Classe { uri, classe } => format!("{chave}\u{4}{uri}#{classe}"),
        outro => format!("{chave}\u{4}{}", outro.nome()),
    }
}

/// `detectChangesInCheckAlwaysViews` de uma visão (não hospedeira) de
/// componente `@changeDetectionLink`: cada `ViewContainer` público, depois
/// cada filho também ligado (`compile_view.dart:1383-1395`). Vazio, o método
/// não sai.
fn metodo_de_link(link: bool, ancoras: &[String], ligadas: &[String]) -> String {
    if !link || (ancoras.is_empty() && ligadas.is_empty()) {
        return String::new();
    }
    let linhas: Vec<String> = ancoras
        .iter()
        .chain(ligadas)
        .map(|a| format!("    this.{a}.detectChangesInCheckAlwaysViews();"))
        .collect();
    format!(
        "\n  @override\n  void detectChangesInCheckAlwaysViews() {{\n{}\n  }}\n",
        linhas.join("\n")
    )
}

/// A consulta lê o `ViewContainerRef` do ngdart (`read: ViewContainerRef`)?
/// Num `#ref` do nó, isso liga o `_requiresViewContainer` dele
/// (`provider_parser.dart:95-104`).
fn le_container(
    consulta: &crate::componente::Consulta,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> bool {
    let Some(t) = consulta.leitura.as_deref().map(str::trim) else {
        return false;
    };
    t.rsplit('.').next() == Some("ViewContainerRef")
        && resolvedor
            .and_then(|r| r.uri_do_tipo(local.caminho, t))
            .is_some_and(|u| u.starts_with("package:ngdart/"))
}

/// A chave do resultado de `#nome` num `<template>` lido como
/// `ViewContainerRef`: o `ViewContainer` do nó (`this._appEl_n`).
fn chave_de_container(nome: &str) -> String {
    format!("{nome}\u{4}ViewContainerRef")
}

/// O tipo do campo é o `TemplateRef` do ngdart (`TemplateRef` ou
/// `TemplateRef?`, sem prefixo, resolvido para `template_ref.dart`)?
fn e_template_ref(tipo: &str, local: &Local, resolvedor: Option<&dyn Resolucao>) -> bool {
    let tipo = tipo.trim().trim_end_matches('?');
    tipo == "TemplateRef"
        && resolvedor
            .and_then(|r| r.uri_do_tipo(local.caminho, tipo))
            .as_deref()
            == Some(TEMPLATE_REF)
}

/// Um pipe de `pipes:`, com a URI da biblioteca que o declara.
#[derive(Debug, Clone)]
pub struct PipeUsado {
    pub uri: String,
    pub pipe: crate::componente::Pipe,
}

/// A instância de um pipe, campo de uma visão (`_pipe_date_0`): a de um
/// pipe puro é uma por nome, na visão do componente, criada no primeiro uso
/// (`compView.purePipes`); a de um impuro é uma por chamada, na visão da
/// chamada. O número é o `pipeCount` da visão que a guarda.
#[derive(Debug, Clone)]
struct InstanciaDePipe {
    nome: String,
    campo: String,
    classe: String,
    /// Caminho do import da biblioteca do pipe (`getImportModulePath`).
    caminho: String,
    /// A visão que guarda a instância (0 para todo pipe puro).
    vista: u32,
    impura: bool,
    /// O construtor recebe o `ChangeDetectorRef` (a própria visão).
    detector: bool,
    /// `OnDestroy`: `ngOnDestroy()` no `destroyInternal` da visão.
    destroi: bool,
}

/// Uma chamada `$pipe.nome(..)`: a visão onde está e o que a substitui —
/// o proxy (`_pipe_date_0_1`, `_PurePipeProxy`, campo dessa visão) de um
/// pipe puro, ou `_pipe_async_1.transform` de um impuro.
#[derive(Debug, Clone)]
struct ChamadaDePipe {
    vista: u32,
    instancia: usize,
    proxy: String,
    argumentos: usize,
    /// `o.FunctionType(retorno, paramTypes.sublist(0, argCount))`; vazio
    /// no impuro, que não tem proxy.
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

    /// Os proxies de pipe puro desta visão, de uma instância.
    fn proxies(&self, vista: u32, k: usize) -> impl Iterator<Item = &ChamadaDePipe> {
        self.da_vista(vista)
            .filter(move |c| c.instancia == k && !self.instancias[k].impura)
    }

    /// Os imports dos campos de pipe desta visão, na ordem da declaração: a
    /// classe de cada instância guardada aqui e o `dart:core` do tipo de um
    /// proxy.
    fn imports_dos_campos(&self, vista: u32) -> Vec<String> {
        let mut saida = Vec::new();
        for (k, inst) in self.instancias.iter().enumerate() {
            if inst.vista == vista {
                saida.push(inst.caminho.clone());
            }
            if self.proxies(vista, k).any(|c| c.core) {
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
            if inst.vista == vista {
                let q = imp.q(&inst.caminho);
                saida.push(format!("  late final {q}{} {};", inst.classe, inst.campo));
            }
            for c in self.proxies(vista, k) {
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
            if inst.vista == vista {
                let q = imp.q(&inst.caminho);
                let arg = if inst.detector { "this" } else { "" };
                saida.push(format!(
                    "    this.{} = {q}{}({arg});",
                    inst.campo, inst.classe
                ));
            }
            for c in self.proxies(vista, k) {
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

    /// O `ngOnDestroy()` das instâncias desta visão
    /// (`bindPipeDestroyLifecycleCallbacks`), na ordem delas.
    fn destruicao(&self, vista: u32) -> Vec<String> {
        self.instancias
            .iter()
            .filter(|i| i.vista == vista && i.destroi)
            .map(|i| format!("    this.{}.ngOnDestroy();", i.campo))
            .collect()
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
    // O `pipeCount` de cada visão.
    let mut contadores: std::collections::HashMap<u32, usize> = Default::default();
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
        // Mais argumentos que parâmetros é erro de compilação no oficial.
        if argumentos > p.parametros.len() {
            return Err(fora("pipe com argumentos demais"));
        }
        let caminho = || {
            asset_de_uri(&usado.uri, "", Path::new(""))
                .and_then(|alvo| caminho_do_import(asset, &alvo))
                .ok_or_else(|| fora("pipe sem caminho de import"))
        };
        let nova = |t: &mut PipesDoTemplate,
                    contadores: &mut std::collections::HashMap<u32, usize>,
                    vista: u32,
                    impura: bool|
         -> Result<usize, Recusa> {
            let n = contadores.entry(vista).or_default();
            t.instancias.push(InstanciaDePipe {
                campo: format!("_pipe_{nome}_{n}"),
                nome: nome.clone(),
                classe: p.classe.clone(),
                caminho: caminho()?,
                vista,
                impura,
                detector: p.detector,
                destroi: p.destroi,
            });
            *n += 1;
            Ok(t.instancias.len() - 1)
        };
        // Impuro: uma instância por chamada, na visão dela, chamada direto
        // (`_call`: `instance.transform(..)`).
        if !p.puro {
            let instancia = nova(&mut t, &mut contadores, vista, true)?;
            let proxy = format!("{}.transform", t.instancias[instancia].campo);
            t.chamadas.push(ChamadaDePipe {
                vista,
                instancia,
                proxy,
                argumentos,
                tipo: String::new(),
                core: false,
            });
            continue;
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
        let instancia = match t
            .instancias
            .iter()
            .position(|i| i.nome == nome && !i.impura)
        {
            Some(k) => k,
            None => nova(&mut t, &mut contadores, 0, false)?,
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
    // Numa visão embutida, a ordem entre a instância de um impuro e os
    // proxies de um puro (criados pelo `create()` da instância, na visão do
    // componente) ainda não tem caso.
    for c in &t.chamadas {
        let impura_aqui = t.instancias.iter().any(|i| i.impura && i.vista == c.vista);
        if c.vista != 0 && impura_aqui && !t.instancias[c.instancia].impura {
            return Err(fora("pipe impuro e pipe puro na mesma visão embutida"));
        }
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

/// As chamadas `$pipe.nome(..)` de uma expressão, com o número de
/// argumentos, na ordem da conversão: os pipes dos argumentos antes do de
/// fora (`visitPipe` converte a entrada e os argumentos primeiro). Texto
/// entre aspas não conta.
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
        pipes_na_expressao(&texto[k + 1..fim - 1], vista, saida)?;
        saida.push((vista, texto[ini..j].to_string(), argumentos));
        i = fim;
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
    /// `@changeDetectionLink`: numa visão de componente também ligado, o
    /// `detectChangesInCheckAlwaysViews` desce até a visão dele.
    pub link_de_deteccao: bool,
    /// `@HostBinding`: quem o usa chama `detectHostChanges(firstCheck)`
    /// antes de detectar a visão dele (`bindDirectiveHostProps`).
    pub hospedeiro: bool,
    /// `@Output`s (nome no template, membro), na ordem do mapa `outputs`.
    pub saidas: Vec<(String, String)>,
    /// Os `hostAttributes` do filho: `@HostBinding` em membro estático
    /// imutável fora de `class.x`/`style.x` (`_computeHostBindingImmutability`),
    /// pelo nome do atributo (sem `attr.`) e o membro. Quem o usa os mescla
    /// com o atributo escrito de mesmo nome (`_mergeHtmlAndDirectiveAttrs`).
    pub atributos_do_hospedeiro: Vec<(String, String)>,
    /// O que o construtor do filho recebe, na ordem.
    pub parametros: Vec<Injetado>,
    /// `@ContentChild`/`@ContentChildren` do filho, com o alvo resolvido:
    /// (campo, lista, alvo). Quem projeta conteúdo nele atualiza a consulta.
    pub consultas: Vec<ConsultaDoFilho>,
    /// O que no filho muda o código de quem o usa e o emissor ainda não
    /// escreve: injeção no construtor, `@HostBinding`, consulta de conteúdo,
    /// provedores, projeção com seletor. Qualquer uma recusa o uso.
    pub pendencias: Vec<Recusa>,
    /// Os metadados do componente lidos do programa (`metadados.rs`): os
    /// `providers:` dele entram no nó de quem o usa.
    pub metadados: Option<std::sync::Arc<crate::diretivas::Diretiva>>,
}

/// Um parâmetro do construtor de um filho, como o oficial o resolve no nó
/// dele (`provider_resolver.dart`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Injetado {
    /// `Element`/`HtmlElement`: o próprio nó.
    Elemento,
    /// `ChangeDetectorRef`: a visão do filho.
    Detector,
    /// `ViewContainerRef`: o `ViewContainer` do nó do filho (`_appEl_n`).
    Container,
    /// Serviço de fora da visão: `injectorGet` pela visão de cima
    /// (`injectFromViewParentInjector`), `injectorGetOptional` com
    /// `@Optional()`.
    Servico {
        /// A classe, ou o `OpaqueToken`/`MultiToken` do `@Inject(..)`.
        token: crate::diretivas::Token,
        opcional: bool,
        /// `@Self()`, `@Host()`, `@SkipSelf()` (`_getDependency`).
        proprio: bool,
        hospedeiro: bool,
        pular: bool,
    },
    /// `@Attribute('nome')`: o valor literal do atributo no elemento do
    /// filho, ou `null` (`_getLocalDependency`).
    Atributo(String),
}

/// De onde vem um serviço que o filho injeta (`_getDependency`).
enum OrigemDoServico {
    /// Um provedor do próprio nó, criado antes do filho.
    Local(String),
    /// Um elemento acima provê: a leitura dele.
    Cima(String),
    /// `@Optional()` que não acha (`@Self`, ou `@Host` fora da hospedeira).
    Nulo,
    /// O injetor de fora da visão.
    Injetor,
}

/// Um `@ContentChild`/`@ContentChildren` de um filho.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultaDoFilho {
    pub campo: String,
    /// `@ContentChildren`.
    pub lista: bool,
    pub alvo: AlvoDeConsulta,
    pub descendentes: bool,
    /// `read:`: o que se lê do nó achado, em vez da instância que casou.
    pub leitura: Option<LeituraDaConsulta>,
}

/// O `read:` de uma consulta de conteúdo de um filho.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeituraDaConsulta {
    /// `HtmlElement`/`Element` do `dart:html`: o próprio nó.
    Elemento,
    /// Outro provedor do nó achado, pela classe (URI, nome).
    Classe(String, String),
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
    /// Os metadados do próprio componente lidos do programa
    /// (`metadados.rs`): os `providers:` dele vão para a visão-hospedeira.
    pub metadados: Option<std::sync::Arc<crate::diretivas::Diretiva>>,
}

impl Local<'_> {
    /// URI `package:` da folha compilada de um `styleUrls`: o
    /// `.css.shim.dart` (com shim) ou, com `ViewEncapsulation.none`, o
    /// `.css.dart` (`stylesModuleUrl(url, shim)` do ngcompiler).
    pub(crate) fn uri_do_estilo(&self, url: &str, shim: bool) -> Option<String> {
        let sufixo = if shim { ".shim.dart" } else { ".dart" };
        // URI `package:` escrita: já é a da folha (`button_decorator.scss.css`
        // do `dropdown_button`).
        if url.starts_with("package:") {
            return Some(format!("{url}{sufixo}"));
        }
        let dentro = self.relativo.strip_prefix("lib/")?;
        let dir = dentro.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let caminho = if dir.is_empty() {
            url.to_string()
        } else {
            format!("{dir}/{url}")
        };
        // Resolvido como URI: `.` e `..` somem.
        let mut partes: Vec<&str> = Vec::new();
        for p in caminho.split('/') {
            match p {
                "." | "" => {}
                ".." => {
                    partes.pop()?;
                }
                _ => partes.push(p),
            }
        }
        Some(format!(
            "package:{}/{}{sufixo}",
            self.pacote,
            partes.join("/")
        ))
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
    /// `#ref` de elemento desta visão lidos por expressões dela
    /// ([`referencias_locais`]): o nó vira campo e o nome, local
    /// (`final local_x = this._el_n;`).
    refs_locais: std::collections::HashSet<String>,
    /// Os `#ref` repetidos ou sombreados por `let`
    /// ([`referencias_ambiguas`]): o escopo por visão deles ainda não é
    /// traduzido, e a recusa diz isso.
    refs_ambiguos: std::collections::HashSet<String>,
    /// Os `#ref` locais das visões ancestrais (ver [`EspecEmbutida`]).
    refs_ancestrais: Vec<(String, String, u32)>,
    /// Na visão do componente: as consultas de visão atualizadas na
    /// detecção ([`consulta_em_embutida`]).
    consultas_dinamicas: Vec<ConsultaDinamica>,
    /// As consultas dinâmicas cujo resultado está abaixo desta visão
    /// embutida: (`#ref`, campo sujo, níveis até a visão do componente).
    consultas_em_transito: Vec<(String, String, u32)>,
    /// As âncoras das consultas dinâmicas, do [`Contexto`].
    ancoras_de_consulta: &'a std::cell::RefCell<Ancoras>,
    /// [`Contexto::conteudo_dinamico`].
    conteudo_dinamico: &'a std::cell::RefCell<Vec<ConteudoDinamico>>,
    /// [`Contexto::sujas_de_conteudo`].
    sujas_de_conteudo: &'a std::cell::RefCell<SujasDeConteudo>,
    /// As atualizações das consultas de conteúdo dinâmicas desta visão
    /// (`updateContentQuery`), no bloco dos ganchos de conteúdo, depois das
    /// de visão e antes dos ganchos, na ordem do `afterChildren` dos nós.
    atualizacoes_de_conteudo: Vec<String>,
    /// As das consultas de visão por token ([`consulta_de_token_dinamica`]),
    /// com o índice da consulta (a ordem no `afterNodes`).
    atualizacoes_de_visao: Vec<(usize, String)>,
    /// Os índices dessas consultas (fora da atribuição estática).
    consultas_por_token: std::collections::HashSet<usize>,
    /// Os campos `bool _query_X_n_i_isDirty = true;` dessas consultas, com a
    /// posição do primeiro resultado em `*` (quando o campo é alocado).
    sujos_de_conteudo: Vec<(usize, String)>,
    /// Quantas consultas de conteúdo cada nó já registrou (`_queryCount`).
    consultas_por_no: std::collections::HashMap<u32, usize>,
    /// Na embutida: os `#ref` resultado de consulta da visão do componente
    /// (o nó vira campo) e o campo "sujo" de cada uma.
    refs_consultados: Vec<(String, String, u32)>,
    /// Cada `#ref` visto, com a expressão do nó (`_el_3` ou `this._el_3`) —
    /// é o valor que o `@ViewChild` recebe.
    refs: std::collections::HashMap<String, String>,
    /// Os mesmos, em ordem de documento e com repetição: os resultados de
    /// um `@ViewChildren` (`addQueryResult`).
    /// [`Contexto::leituras`].
    leituras: &'a [(String, crate::diretivas::Token)],
    /// [`Contexto::refs_so_por_provedor`].
    refs_so_por_provedor: &'a std::collections::HashSet<String>,
    /// [`Contexto::moldes_com_container`].
    moldes_com_container: &'a std::collections::HashSet<String>,
    /// [`Contexto::tipos_de_diretiva`].
    tipos_de_diretiva: &'a [TipoDeDiretivaResolvido],
    refs_em_ordem: Vec<(String, String)>,
    /// A tag do elemento cujas ligações estão sendo escritas: o contexto de
    /// segurança de uma propriedade depende dela ([`saneador`]).
    tag_atual: String,
    /// O prefixo de namespace herdado (`svg`, `math`): o `_NamespaceVisitor`
    /// passa o do pai aos descendentes (`ast_template_parser.dart:1331-1352`).
    ns_atual: Option<String>,
    /// O `appViewInstance` do nó cujas ligações se escrevem: `this`, ou a
    /// visão do componente no elemento dele (`this._compView_n`, o
    /// `[class]` do elemento de um filho, caso j74).
    instancia_da_visao: Option<String>,
    /// A visão passada ao `detectHostChanges` de uma diretiva com
    /// `@HostBinding` no nó: a do componente filho, no nó dele (caso j92);
    /// `None` é a própria visão (`this`).
    vista_do_hospedeiro: Option<String>,
    /// Campos `TextBinding` e `_message_N`, que saem primeiro na classe, na
    /// ordem em que o `build()` os aloca.
    campos: Vec<String>,
    /// Prefixo do `package:intl` quando a visão tem `@i18n` que o emissor
    /// traduz (só a do componente); `None` recusa a anotação.
    intl: Option<String>,
    /// As mensagens já criadas (`createI18nMessage` reaproveita a igual):
    /// a chamada `Intl.message(..)` e o campo.
    mensagens: Vec<(String, String)>,
    /// Campos `Object? _expr_k` das ligações, na ordem em que aparecem.
    campos_expr: Vec<String>,
    /// Campos `late final T _el_n` dos elementos com ligação.
    campos_el: Vec<String>,
    /// Os `#ref` (sem valor) do nó de cada campo de [`Corpo::campos_el`].
    refs_dos_campos_el: std::collections::HashMap<String, Vec<String>>,
    /// Os `#ref` desta visão lidos só em handler de evento: o nó vira campo
    /// quando o `NodeReferenceStorageVisitor` chega ao `_handleEvent_N` que o
    /// lê — depois dos promovidos na detecção (caso j60).
    refs_so_em_eventos: std::collections::HashSet<String>,
    /// Os campos desses nós à espera do primeiro handler que os lê.
    campos_el_de_eventos: Vec<(String, String)>,
    /// Os mesmos, na ordem em que os handlers os leem.
    campos_el_por_evento: Vec<String>,
    /// Campos dos nós desta visão embutida que são resultado de consulta
    /// dinâmica da visão do componente, com a posição da consulta em
    /// [`Corpo::refs_consultados`]: o oficial os promove a campo quando a
    /// visão do componente escreve o `mapNestedViews`
    /// (`replaceReadClassMemberInExpression`), antes de promover os nós que
    /// as ligações desta visão leem — vêm antes de [`Corpo::campos_el`],
    /// na ordem das consultas.
    campos_el_consultados: Vec<(usize, String)>,
    /// Campos dos nós que uma visão embutida lê (`#ref` desta visão lido de
    /// dentro de um `*`), com o nome: vão entre os `_expr_k`, na posição
    /// de [`Corpo::posicao_de_embutida`] ([`citado_em_embutidas`]).
    campos_el_de_embutidas: Vec<(String, String)>,
    /// Os `#ref` locais desta visão lidos de uma embutida.
    refs_de_embutidas: std::collections::HashSet<String>,
    /// Quantos `_expr_k` existiam quando a primeira embutida que lê cada um
    /// deles foi ligada.
    posicao_de_embutida: std::collections::HashMap<String, usize>,
    /// Próximo índice de ligação (`_expr_k`, `currVal_k`).
    proxima_ligacao: u32,
    /// Corpo do `detectChangesInternal`.
    deteccao: Vec<String>,
    /// Prefixo do `text_binding.dart`, alocado antes do resto quando o
    /// template tem interpolação (a ordem dos imports segue a ordem em que o
    /// oficial escreve o arquivo, e os campos vêm primeiro).
    /// Algum campo `TextBinding` foi criado nesta visão.
    tb_usado: bool,
    tb: Option<String>,
    /// URI `package:` do arquivo do template, para o comentário `REF`.
    url_do_template: Option<String>,
    /// Tipos dos membros do componente, para escolher `interpolateString`.
    membros: &'a std::collections::HashMap<String, crate::componente::Membro>,
    exportados: &'a Exportados,
    /// [`Contexto::nomes_genericos`].
    nomes_genericos: &'a [String],
    /// [`Contexto::tokens_de_visao`].
    tokens_de_visao: &'a [crate::diretivas::Token],
    /// Métodos da classe, válidos só como alvo de chamada.
    metodos: &'a std::collections::HashMap<String, String>,
    /// Parâmetros posicionais de cada método, para o tear-off de evento.
    aridades: &'a std::collections::HashMap<String, usize>,
    /// Os métodos `_handleEvent_N` desta visão, na ordem em que foram
    /// criados (`createEventHandler`); saem depois do `destroyInternal`.
    metodos_evento: Vec<String>,
    /// Os métodos `static String _message_N(..)` das mensagens `@i18n` com
    /// HTML, que abrem os métodos da visão.
    metodos_i18n: Vec<String>,
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
    /// Os provedores injetáveis dos elementos acima do nó, o mais longe
    /// primeiro: (token, campo, e a visão ancestral que o tem — classe e
    /// quantos `parentView` até ela — ou nada, se é desta).
    acima: Vec<(crate::diretivas::Token, String, Option<(String, u32)>)>,
    /// Os campos de `acima` que são provedores preguiçosos do elemento
    /// dono (campo, classe da visão dele): um nó abaixo que os pede os
    /// transformaria antes do `afterElement` do dono (lacuna L1 da seção 05).
    preguicosos_acima: std::collections::HashSet<(String, String)>,
    /// Os deles cujo dono modelou os pedidos do conteúdo (nó de filho): de
    /// outra visão, lidos pelo campo preguiçoso.
    preguicosos_com_pedidos: std::collections::HashSet<(String, String)>,
    /// Quantos elementos de componente há acima (provedores que o emissor
    /// não modela).
    componentes_acima: u32,
    /// Componentes acima cujos provedores não se sabem (sem metadados do
    /// programa): o que o nó não acha acima pode estar neles.
    incertos_acima: u32,
    /// Componentes que este template pode usar, por seletor.
    filhos: &'a std::collections::HashMap<String, Filho>,
    /// Todas as diretivas e componentes de `directives:`, para saber o que
    /// casa com cada elemento.
    usadas: &'a [Usada],
    /// Modo de coleta (o diagnóstico do placar): cada recusa é anotada aqui
    /// e a emissão segue, para achar as outras. `None`: a primeira recusa
    /// interrompe, e o arquivo fica com o `build_runner`.
    coleta: Option<Vec<Recusa>>,
    /// Campos dos provedores preguiçosos (`late T _X_n_m = valor;`): o
    /// `ViewStorage` os aloca ao construir a visão, antes de promover nós e
    /// ligações de texto a campo, então abrem a classe (caso j38).
    campos_preguicosos: Vec<String>,
    /// A posição do nó de cada um ([`crate::html::Elemento::inicio`]): a
    /// ordem de alocação entre os campos com inicializador.
    posicoes_preguicosas: Vec<usize>,
    /// Os campos sujos das consultas de visão dinâmicas, com a posição do
    /// primeiro resultado delas (ver [`campos_com_inicializador`]).
    sujos_de_visao: Vec<(usize, String)>,
    /// Campos das visões-filhas (`_compView_n` e a instância), que saem na
    /// classe depois das ligações de texto.
    campos_filho: Vec<String>,
    /// `_compView_n` de cada filho, para a detecção e a destruição.
    vistas_filhas: Vec<String>,
    /// As dos filhos `@changeDetectionLink` ([`metodo_de_link`]).
    vistas_ligadas: Vec<String>,
    /// Asset deste arquivo, para calcular os caminhos de import dos filhos.
    asset: String,
    /// Banco semântico e o arquivo, para tipar cadeias como `item.nome`.
    tipos: Option<(&'a dyn Resolucao, &'a Path)>,
    /// A classe do componente com o prefixo do import dela
    /// (`import1.Classe`): o receptor dos membros estáticos.
    classe_qualificada: String,
    /// O componente tem folha de estilo: cada elemento ganha `addShimC`.
    com_estilo: bool,
    /// Visões embutidas a emitir. Só a especificação: o texto é gerado
    /// depois do `angular.dart`, que é onde o oficial escreve essas classes —
    /// e é a ordem de escrita que numera os imports.
    embutidas: Vec<EspecEmbutida>,
    /// Nome da classe da visão (`ViewX`), para numerar as embutidas.
    classe_da_visao: String,
    /// Os argumentos de tipo de um componente genérico (`<T, U>`), vazio
    /// no resto: a fábrica de cada embutida é chamada com eles.
    genericos: String,
    /// `preserveWhitespace: true`: as pontas das interpolações não são
    /// comprimidas (`_compressWhitespace*`).
    preservar_espacos: bool,
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
    /// Os mesmos registros de [`Corpo::detectores`], na ordem dos nós (um
    /// por resultado, também com `#ref` repetido).
    detectores_em_ordem: Vec<(String, String)>,
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
    /// Os elementos em que estamos (índice, se tem diretiva ou componente):
    /// a cadeia de `parent` do `_getQueriesFor`.
    pilha: Vec<(u32, bool)>,
    /// Os provedores criados nesta visão, em pré-ordem, com a cadeia de
    /// elementos acima de cada um: é deles que sai o resultado estático de
    /// uma consulta de conteúdo (`addQueryResult`).
    registros: Vec<Registro>,
}

/// Um nó com provedores, para as consultas de conteúdo: os tokens pelos
/// quais cada instância casa uma consulta e o campo dela.
#[derive(Debug, Clone)]
struct Registro {
    acima: Vec<(u32, bool)>,
    /// (token, campo, e a visão do filho quando ele é um componente
    /// `onPush`: o `buildChangeDetectorRef` do provedor).
    provedores: Vec<(crate::diretivas::Token, String, Option<String>)>,
    /// O nó como o `build()` o lê (`_el_3` ou `this._el_3`), para o
    /// `read: HtmlElement`.
    elemento: String,
}

impl Corpo<'_> {
    /// A classe desta visão, para o `unsafeCast` de quem a lê de baixo.
    /// Os argumentos de tipo de `directiveTypes:` para a diretiva `classe`
    /// (de `uri`) no nó `e` (`lookupTypeArgumentsOf`, `compile_view.dart:
    /// 1523-1569`): o `Typed` com `on:` igual a um `#ref` do nó vence; senão o
    /// primeiro sem `on:`. Vazio sem nenhum.
    fn argumentos_de_tipo(&mut self, uri: &str, classe: &str, e: &crate::html::Elemento) -> String {
        let texto = self.argumentos_escritos(uri, classe, e);
        resolver_tardios(self.imp, &texto)
    }

    fn argumentos_escritos(&self, uri: &str, classe: &str, e: &crate::html::Elemento) -> String {
        let mut primeiro = None;
        for t in self.tipos_de_diretiva {
            if t.uri != uri || t.classe != classe {
                continue;
            }
            match &t.em {
                Some(r) => {
                    if e.referencias.iter().any(|x| x.nome == *r) {
                        return t.argumentos.clone();
                    }
                }
                None if primeiro.is_none() => {
                    if e.referencias.is_empty() {
                        return t.argumentos.clone();
                    }
                    primeiro = Some(t.argumentos.clone());
                }
                None => {}
            }
        }
        primeiro.unwrap_or_default()
    }

    /// Registra, pela posição do nó ([`chave_de_no`]), cada token de classe
    /// que as instâncias dele fornecem (o próprio e os `ExistingProvider`):
    /// o valor que uma consulta de conteúdo lê de outra visão
    /// (`_providers.get(token).build()`).
    fn registrar_no_por_posicao(
        &mut self,
        inicio: usize,
        instancias: &[crate::diretivas::Instancia],
    ) {
        for i in instancias {
            for t in std::iter::once(&i.token).chain(&i.apelidos) {
                if let crate::diretivas::Token::Classe { uri, classe } = t {
                    self.refs
                        .entry(chave_de_no(&chave_de_tipo(uri, classe), inicio))
                        .or_insert_with(|| format!("this.{}", i.leitura));
                }
            }
        }
    }

    /// Os resultados de uma consulta de conteúdo pelo token (`uri`, `classe`)
    /// em `nos` (o conteúdo do nó da consulta, `descendants: true`): cada nó
    /// que o fornece (componente pela tag, diretiva casada, ou `providers:`
    /// de uma delas) e cada visão embutida com resultados, em pré-ordem.
    /// `Err` para o resultado que ainda não se traduz (componente `onPush`,
    /// que registraria o `ChangeDetectorRef`).
    fn arvore_de_conteudo(
        &self,
        nos: &[No],
        uri: &str,
        classe: &str,
    ) -> Result<Vec<ItemDeConteudo>, Recusa> {
        arvore_de_token(nos, uri, classe, self.filhos, self.usadas)
    }

    /// Registra a consulta de conteúdo dinâmica ([`ConteudoDinamico`]): o
    /// campo sujo, a atualização na detecção (a marca, montada no fim) e o
    /// `dirtyParentQueriesInternal` de cada visão embutida com resultado.
    #[allow(clippy::too_many_arguments)]
    fn conteudo_dinamico_no(
        &mut self,
        arvore: Vec<ItemDeConteudo>,
        chave: String,
        seletor: &str,
        n: u32,
        indice: usize,
        lista: bool,
        alvo: String,
        de_visao: bool,
    ) {
        let classe = self.classe_desta_visao();
        // A de visão (`@ViewChild(ren)` por token, raiz na visão do
        // componente): `_viewQuery_X_i_isDirty`, atualizada no `afterNodes`.
        let campo = if de_visao {
            format!("_viewQuery_{seletor}_{indice}_isDirty")
        } else {
            format!("_query_{seletor}_{n}_{indice}_isDirty")
        };
        let especie: u8 = if de_visao { 1 } else { 0 };
        fn sujas(
            itens: &[ItemDeConteudo],
            nivel: u32,
            campo: &str,
            classe: &str,
            especie: u8,
            mapa: &mut SujasDeConteudo,
        ) {
            for i in itens {
                if let ItemDeConteudo::Aninhada { estrela, itens } = i {
                    let primeiro = itens.iter().find_map(|x| match x {
                        ItemDeConteudo::Valor { inicio, .. } => Some(*inicio),
                        ItemDeConteudo::Aninhada { .. } => None,
                    });
                    if let Some(p) = primeiro {
                        mapa.entry(*estrela).or_default().push((
                            p,
                            especie,
                            campo.to_string(),
                            nivel,
                            classe.to_string(),
                        ));
                    }
                    sujas(itens, nivel + 1, campo, classe, especie, mapa);
                }
            }
        }
        sujas(
            &arvore,
            1,
            &campo,
            &classe,
            especie,
            &mut self.sujas_de_conteudo.borrow_mut(),
        );
        let estatico = arvore
            .iter()
            .any(|i| matches!(i, ItemDeConteudo::Valor { .. }));
        fn primeiro_aninhado(itens: &[ItemDeConteudo], dentro: bool) -> Option<usize> {
            itens.iter().find_map(|i| match i {
                ItemDeConteudo::Valor { inicio, .. } => dentro.then_some(*inicio),
                ItemDeConteudo::Aninhada { itens, .. } => primeiro_aninhado(itens, true),
            })
        }
        let posicao = primeiro_aninhado(&arvore, false).unwrap_or(usize::MAX);
        fn tem_on_push(itens: &[ItemDeConteudo]) -> bool {
            itens.iter().any(|i| match i {
                ItemDeConteudo::Valor { on_push, .. } => *on_push,
                ItemDeConteudo::Aninhada { itens, .. } => tem_on_push(itens),
            })
        }
        let com_on_push = tem_on_push(&arvore);
        let mut consultas = self.conteudo_dinamico.borrow_mut();
        let id = consultas.len();
        consultas.push(ConteudoDinamico {
            id,
            arvore,
            chave,
            lista,
            alvo,
            classe,
        });
        let mut marca = format!("{MARCA_DE_CONTEUDO}{id}");
        if !lista && !estatico {
            marca += &format!("|q{}", tardio_q(QUERIES));
        }
        if com_on_push {
            marca += &format!("|d{}", tardio_q(VIEW));
        }
        marca.push(FIM_DE_CONTEUDO);
        drop(consultas);
        let atualizacao =
            format!("    if (this.{campo}) {{\n      {marca}\n      this.{campo} = false;\n    }}");
        if de_visao {
            self.sujos_de_visao
                .push((posicao, format!("  bool {campo} = true;")));
            self.atualizacoes_de_visao.push((indice, atualizacao));
        } else {
            self.sujos_de_conteudo
                .push((posicao, format!("  bool {campo} = true;")));
            self.atualizacoes_de_conteudo.push(atualizacao);
        }
    }

    /// O namespace do elemento e o nome sem prefixo: `ns:tag` escrito, o
    /// implícito da tag (`svg`, `math`, `html_tags.dart`) ou o herdado.
    fn namespace_de(&self, e: &crate::html::Elemento) -> Option<(String, String)> {
        if let Some((ns, nome)) = e.nome.split_once(':') {
            return Some((ns.to_string(), nome.to_string()));
        }
        let implicito = match e.nome.to_ascii_lowercase().as_str() {
            "svg" => Some("svg"),
            "math" => Some("math"),
            _ => None,
        };
        implicito
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

    /// Os provedores acima, do mais próximo ao mais longe, com a leitura
    /// vista desta visão (`getPropertyInView`).
    fn provedores_acima(&mut self) -> Vec<crate::diretivas::ProvedorAcima> {
        let util = self.imp.alias(UTILITIES);
        self.provedores_acima_com(&util)
    }

    /// [`Self::provedores_acima`] com o prefixo do `unsafeCast` dado: uma
    /// resolução prévia (a posição dos campos do nó) não aloca import.
    fn provedores_acima_com(&self, util: &str) -> Vec<crate::diretivas::ProvedorAcima> {
        self.acima
            .iter()
            .rev()
            .map(|(t, campo, v)| crate::diretivas::ProvedorAcima {
                preguicoso: self.preguicosos_acima.contains(&(
                    campo.clone(),
                    match v {
                        None => self.classe_desta_visao(),
                        Some((classe, _)) => classe.clone(),
                    },
                )),
                leitura_preguicosa: match v {
                    None => false,
                    Some((classe, _)) => self
                        .preguicosos_com_pedidos
                        .contains(&(campo.clone(), classe.clone())),
                },
                token: t.clone(),
                leitura: match v {
                    None => format!("this.{campo}"),
                    Some((classe, n)) => {
                        let mut vista = "(this.parentView!)".to_string();
                        for _ in 1..*n {
                            vista = format!("({vista}.parentView!)");
                        }
                        format!("{util}.unsafeCast<{classe}>({vista}).{campo}")
                    }
                },
            })
            .collect()
    }

    fn dom(&mut self) -> String {
        self.imp.alias(DOM_HELPERS)
    }

    /// Os `#ref` desta visão lidos como local: o local (`dynamic`) e a
    /// declaração com a marca do nó, trocada quando a visão termina.
    fn declarar_refs(&mut self, refs: std::collections::HashSet<String>) {
        let classe = self.classe_desta_visao();
        for nome in &refs {
            // O tipo da leitura é `dynamic` (a referência não entra nos
            // `locals` do `AnalyzedClass`), ou o do membro de mesmo nome, que
            // o `_TypeResolver` acha antes.
            let tipo = self
                .membros
                .get(nome.as_str())
                .map(|m| m.tipo.trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "dynamic".into());
            self.locais.insert(
                nome.clone(),
                crate::expr::Local {
                    dart: format!("local_{nome}"),
                    tipo,
                    escopo: None,
                },
            );
            self.decl_locais.insert(
                nome.clone(),
                Ok(format!(
                    "final local_{nome} = {MARCA_DE_REF}{}{FIM_DE_REF};",
                    chave_de_ref(nome, &classe)
                )),
            );
        }
        self.refs_locais = refs;
    }

    /// O campo de uma mensagem `@i18n` sem HTML (`createI18nMessage`): um
    /// `static final String _message_N = Intl.message(texto, desc: ..)`,
    /// reaproveitado quando texto e metadados se repetem.
    fn mensagem(&mut self, texto: &str, m: &MetaI18n) -> Result<String, Recusa> {
        let Some(intl) = self.intl.clone().filter(|i| !i.is_empty()) else {
            return Err(recusa(Motivo::I18n, "@i18n fora da visão do componente"));
        };
        let mut chamada = format!(
            "{intl}.Intl.message({}, desc: {}",
            literal(texto),
            literal(m.descricao.as_deref().unwrap_or_default())
        );
        if let Some(l) = &m.locale {
            let _ = write!(chamada, ", locale: {}", literal(l));
        }
        if let Some(s) = &m.meaning {
            let _ = write!(chamada, ", meaning: {}", literal(s));
        }
        if m.skip {
            chamada.push_str(", skip: true");
        }
        chamada.push(')');
        if let Some((_, campo)) = self.mensagens.iter().find(|(c, _)| *c == chamada) {
            return Ok(campo.clone());
        }
        let campo = format!("_message_{}", self.mensagens.len());
        self.campos
            .push(format!("  static final String {campo} = {chamada};"));
        self.mensagens.push((chamada, campo.clone()));
        Ok(campo)
    }

    /// Os filhos com HTML de um elemento com `@i18n` ([`Corpo::filhos_i18n`]).
    fn filhos_i18n_html(
        &mut self,
        e: &crate::html::Elemento,
        m: &MetaI18n,
        alvo: &str,
    ) -> Result<(), Recusa> {
        if self.intl.is_none() {
            return self.anotar(recusa(Motivo::I18n, "@i18n fora da visão do componente"));
        }
        let mut texto = String::new();
        let mut args = Vec::new();
        match mensagem_com_html(&e.filhos, self.filhos, &mut texto, &mut args, &mut 0) {
            Ok(true) if !args.is_empty() => {}
            Ok(_) => {
                return self.anotar(recusa(Motivo::I18n, "mensagem @i18n sem texto"));
            }
            Err(f) => return self.anotar(recusa(Motivo::I18n, f)),
        }
        // `HtmlEscape(HtmlEscapeMode.element)` do texto já escapado.
        let texto = texto
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let intl = tardio(INTL);
        let mut chamada = format!(
            "{intl}.Intl.message('{texto}', desc: {}",
            literal(m.descricao.as_deref().unwrap_or_default())
        );
        if let Some(l) = &m.locale {
            let _ = write!(chamada, ", locale: {}", literal(l));
        }
        if let Some(s) = &m.meaning {
            let _ = write!(chamada, ", meaning: {}", literal(s));
        }
        if m.skip {
            chamada.push_str(", skip: true");
        }
        let valores: Vec<String> = args.iter().map(|(_, v)| literal(v)).collect();
        let invocacao = |campo: &str| format!("{campo}({})", valores.join(", "));
        let campo = match self.mensagens.iter().find(|(c, _)| *c == chamada) {
            Some((_, campo)) => campo.clone(),
            None => {
                let campo = format!("_message_{}", self.mensagens.len());
                let nomes: Vec<&str> = args.iter().map(|(n, _)| n.as_str()).collect();
                let exemplos: Vec<String> = args
                    .iter()
                    .map(|(n, v)| format!("{}: {}", literal(n), literal(v)))
                    .collect();
                let parametros: Vec<String> = nomes.iter().map(|n| format!("String {n}")).collect();
                self.metodos_i18n.push(format!(
                    "\n  static String {campo}({}) {{\n    return {chamada}, name: '{}_{campo}', args: [{}], examples: const {{{}}});\n  }}\n",
                    parametros.join(", "),
                    self.classe_desta_visao(),
                    nomes.join(", "),
                    exemplos.join(", ")
                ));
                self.mensagens.push((chamada, campo.clone()));
                campo
            }
        };
        let n = self.proximo;
        self.proximo += 1;
        let avu = self.imp.q(APP_VIEW_UTILS);
        self.linhas.push(format!(
            "    final _html_{n} = {avu}createTrustedHtml({});\n    {alvo}.append(_html_{n});",
            invocacao(&campo)
        ));
        Ok(())
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
    /// entram na lista da raiz, na ordem do primeiro uso. A chamada com
    /// argumento nomeado, que o formatador quebra em linhas
    /// ([`crate::expr::QUEBRA`]), fica para quem sabe onde ela cai
    /// ([`Self::converter_quebravel`]).
    fn converter(
        &mut self,
        texto: &str,
        motivo: Motivo,
    ) -> Result<crate::expr::Convertida, Recusa> {
        let c = self.converter_quebravel(texto, motivo)?;
        if c.texto.contains(crate::expr::QUEBRA) {
            return Err(recusa(
                motivo,
                "chamada com argumento nomeado fora de `final currVal`, evento ou interpolação",
            ));
        }
        Ok(c)
    }

    /// Como [`Self::converter`], mas aceita a chamada que quebra: para
    /// quem a escreve sozinha numa instrução (`final currVal_k = …;`) ou
    /// trata a indentação dela.
    fn converter_quebravel(
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
            classe: Some(&self.classe_qualificada),
            exportados: Some(self.exportados),
            genericos: self.nomes_genericos,
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
    /// de argumentos com a tabela — que tem a ordem do oficial. A ordem é a
    /// da conversão, que visita os argumentos antes do pipe: com pipe dentro
    /// de pipe, é a ordem em que as chamadas **fecham** no texto.
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
        // Cada marca: onde começa e onde fecha a chamada dela.
        let mut marcas = Vec::new();
        let mut desde = 0;
        while let Some(i) = texto[desde..].find(MARCA_DE_PIPE).map(|i| i + desde) {
            let depois = i + MARCA_DE_PIPE.len_utf8();
            let f = texto[depois..]
                .find(FIM_DE_PIPE)
                .map_or(texto.len(), |f| f + depois);
            let abre = f + FIM_DE_PIPE.len_utf8();
            let fecha = argumentos_da_chamada(texto, abre).map_or(texto.len(), |(fim, _)| fim);
            marcas.push((i, fecha));
            desde = depois;
        }
        let mut por_fim: Vec<usize> = (0..marcas.len()).collect();
        por_fim.sort_by_key(|&k| marcas[k].1);
        let mut ordem = vec![0; marcas.len()];
        for (posicao, &k) in por_fim.iter().enumerate() {
            ordem[k] = posicao;
        }
        let base = self.cursor_pipe;
        let mut saida = String::with_capacity(texto.len());
        let mut resto = texto;
        let mut k = 0;
        while let Some(i) = resto.find(MARCA_DE_PIPE) {
            saida.push_str(&resto[..i]);
            let depois = &resto[i + MARCA_DE_PIPE.len_utf8()..];
            let f = depois.find(FIM_DE_PIPE).unwrap_or(depois.len());
            let (nome, n) = depois[..f].rsplit_once('/').unwrap_or((&depois[..f], ""));
            let posicao = ordem.get(k).copied().unwrap_or(k);
            k += 1;
            match chamadas.get(base + posicao) {
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
        // A chamada (que pode quebrar) nunca é imutável: sai sozinha no
        // `final currVal_k = …;`.
        let convertida = self.converter_quebravel(&l.valor, Motivo::Ligacao)?;
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
    /// forma e ainda não tem caso no corpus é recusado: `[style.x.unidade]`, estilo de valor que não é `String`
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
        // `[class]`, `[className]` e `[attr.class]` são o mesmo
        // `ClassBinding` sem nome (`_propertyToIr` do `binding_converter`):
        // o `updateChildClass`, que mantém as classes do escopo da folha.
        Ok(
            // Elemento que não é HTML (componente, tag desconhecida com
            // diretiva): as variantes `NonHtml` (`isHtmlElement`).
            if matches!(l.nome.as_str(), "class" | "className" | "attr.class") {
                let metodo = if dom::tag_html(&self.tag_atual) {
                    "updateChildClass"
                } else {
                    "updateChildClassNonHtml"
                };
                let instancia = self.instancia_da_visao.as_deref().unwrap_or("this");
                format!("{instancia}.{metodo}({alvo}, {valor})")
            } else if let Some(classe) = l.nome.strip_prefix("class.") {
                // `[class.x.y]`: a classe é `x` (`parts[1]`), o resto some.
                let classe = classe.split('.').next().unwrap_or(classe);
                let metodo = if dom::tag_html(&self.tag_atual) {
                    "updateClassBinding"
                } else {
                    "updateClassBindingNonHtml"
                };
                format!("{dom}.{metodo}({alvo}, '{classe}', {valor})")
            } else if let Some(attr) = l.nome.strip_prefix("attr.") {
                // `attr.x.if`: atributo condicional; `attr.ns:x`: com
                // namespace (`template_parser.dart:73-99`). Outra unidade é
                // erro no oficial.
                let (attr, unidade) = match attr.split_once('.') {
                    Some((a, u)) => (a, Some(u.split('.').next().unwrap_or(u))),
                    None => (attr, None),
                };
                let condicional = match unidade {
                    None => false,
                    Some("if") => true,
                    Some(_) => {
                        return Err(recusa(Motivo::Ligacao, "[attr.x.unidade] inválido"));
                    }
                };
                if condicional && attr == "class" {
                    return Err(recusa(Motivo::Ligacao, "class.if (erro no oficial)"));
                }
                // O prefixo vira a URI de `namespaceUris`
                // (`view_compiler_utils.dart:21-25`); fora dela, `null`, e o
                // atributo sai sem namespace (`ir/model.dart:396`).
                let (espaco, attr) = match attr.split_once(':') {
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
                // Com contexto de segurança o valor passa pelo saneador do
                // contexto, achado pelo nome de propriedade mapeado
                // (`securityContext(tag, getMappedPropName(x))` em
                // `createElementPropertyAst`); o atributo escrito fica como está.
                let saneador_do_attr = saneador(
                    &self.tag_atual.to_ascii_lowercase(),
                    propriedade_mapeada(attr),
                );
                if saneador_do_attr.is_some() && (condicional || espaco.is_some()) {
                    return Err(recusa(
                        Motivo::Ligacao,
                        "[attr.x.if] ou atributo com namespace saneado",
                    ));
                }
                let saneado = match saneador_do_attr {
                    Some(f) => {
                        let s = tardio(SAFE_HTML);
                        format!("{s}.{f}({valor})")
                    }
                    None => valor.to_string(),
                };
                // Condicional: `(v ? '' : null)`, sempre `updateAttribute`
                // (`visitAttributeBinding`, b/171226440).
                let saneado = if condicional {
                    format!("({saneado} ? '' : null)")
                } else {
                    saneado
                };
                let valor = saneado.as_str();
                // `visitAttributeBinding`: `setAttribute` quando a fonte não
                // pode ser nula (`isNullable` é o `canBeNull` de
                // `analyzed_class.dart`: literal primitivo, e `a ?? b` com um
                // dos lados assim — caso j64).
                if let Some(ns) = espaco {
                    format!("{dom}.updateAttributeNS({alvo}, '{ns}', '{attr}', {valor})")
                } else {
                    let f = if !c.pode_ser_nulo && !condicional {
                        "setAttribute"
                    } else {
                        "updateAttribute"
                    };
                    format!("{dom}.{f}({alvo}, '{attr}', {valor})")
                }
            } else if let Some(estilo) = l.nome.strip_prefix("style.") {
                // `visitStyleBinding`: com unidade, `v == null ? null : v +
                // 'px'` (`v.toString()` se não é `String`); sem, o próprio valor
                // se é `String`, senão `v.toString()` — `v?.toString()` quando
                // `canBeNull`. `isString` é o `_TypeResolver` (`String?` conta).
                let Some(tipo) = c.tipo.as_deref() else {
                    return Err(recusa(Motivo::Ligacao, "[style.x] de tipo desconhecido"));
                };
                let e_texto = tipo.trim_end_matches('?') == "String";
                let (nome, unidade) = match estilo.split_once('.') {
                    Some((_, u)) if u.contains('.') => {
                        return Err(recusa(Motivo::Ligacao, "[style.x.y.z]"));
                    }
                    Some((n, u)) => (n, Some(u)),
                    None => (estilo, None),
                };
                let valor = match unidade {
                    Some(u) => {
                        let texto = if e_texto {
                            valor.to_string()
                        } else {
                            format!("{valor}.toString()")
                        };
                        format!("(({valor} == null) ? null : ({texto} + {}))", literal(u))
                    }
                    None if e_texto => valor.to_string(),
                    None if c.pode_ser_nulo => format!("{valor}?.toString()"),
                    None => format!("{valor}.toString()"),
                };
                format!("{alvo}.style.setProperty('{nome}', {valor})")
            } else if l.nome.contains('.') {
                return Err(recusa(Motivo::Ligacao, "ligação com prefixo desconhecido"));
            } else {
                // `getMappedPropName` (`_attrToPropMap`,
                // `dom_element_schema_registry.dart`): só quatro nomes mudam;
                // `class` já saiu acima como `ClassBinding`. `[tabindex]`
                // ligado é `PropertyBinding('tabIndex')`, não o
                // `TabIndexBinding` do literal (`binding_converter.dart:131`).
                let prop = propriedade_mapeada(&l.nome);
                // `_sanitizedValue`: o valor passa pelo saneador do contexto
                // (`[style]` é `*|style`: `sanitizeStyle`, caso i92).
                match saneador(&self.tag_atual.to_ascii_lowercase(), prop) {
                    Some(f) => {
                        let s = tardio(SAFE_HTML);
                        format!("{dom}.setProperty({alvo}, '{prop}', {s}.{f}({valor}))")
                    }
                    None => format!("{dom}.setProperty({alvo}, '{prop}', {valor})"),
                }
            },
        )
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
        self.handler_com(textos, &[])
    }

    /// [`Self::handler`] com `extras`: as chamadas dos `@HostListener` das
    /// diretivas do nó para o mesmo evento, depois das ações do template
    /// (`mergeEvents`) — o handler é sempre o método (caso j70).
    fn handler_com(&mut self, textos: &[&str], extras: &[String]) -> Result<String, Recusa> {
        let escopo_locais = self.locais.clone();
        let escopo = crate::expr::Escopo {
            membros: self.membros,
            metodos: self.metodos,
            aridades: self.aridades,
            locais: &escopo_locais,
            tipos: self.tipos,
            classe: Some(&self.classe_qualificada),
            exportados: Some(self.exportados),
            genericos: self.nomes_genericos,
        };
        let mut acoes = Vec::new();
        for t in textos {
            acoes.push(
                crate::expr::converter_acao(t, &escopo, self.nomes)
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
        ) = (acoes.as_slice(), extras)
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
        // O nó de um `#ref` lido só em evento vira campo agora (caso j60).
        for l in &locais {
            if let Some(k) = self.campos_el_de_eventos.iter().position(|(n, _)| n == l) {
                let (_, campo) = self.campos_el_de_eventos.remove(k);
                self.campos_el_por_evento.push(campo);
            }
        }
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
        corpo.extend(extras.iter().map(|i| format!("    {i};")));
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
        if let Some(r) = filho.pendencias.first() {
            return Err(r.clone());
        }
        // `[(x)]` que o próprio filho recebe (`@Input x`, `@Output
        // xChange`), desfeito como o `DesugarVisitor`: a entrada no fim das
        // propriedades e o evento `x = $event` no fim dos eventos.
        let mut desfeito = e.clone();
        desfeito.bananas.clear();
        for b in &e.bananas {
            let mudanca = format!("{}Change", b.nome);
            if filho.entrada(&b.nome).is_some() && filho.saida(&mudanca).is_some() {
                desfeito.propriedades.push(b.clone());
                desfeito.eventos.push(crate::html::Ligacao {
                    nome: mudanca,
                    valor: format!("{} = $event", b.valor),
                    ..b.clone()
                });
            } else {
                desfeito.bananas.push(b.clone());
            }
        }
        let e = &desfeito;
        // As diretivas que casam o elemento do filho (`NgModel`), e o
        // `[(x)]` delas desfeito como no elemento HTML.
        let extras = diretivas_casadas(self.usadas, e);
        let mut props_dir = Vec::new();
        let mut eventos_dir = Vec::new();
        for b in &e.bananas {
            let mudanca = format!("{}Change", b.nome);
            if !consome_entrada(&extras, &b.nome) || !consome_saida(&extras, &mudanca) {
                return Err(em_filho("[(x)] no filho"));
            }
            props_dir.push(b.clone());
            eventos_dir.push(crate::html::Ligacao {
                nome: mudanca,
                valor: format!("{} = $event", b.valor),
                ..b.clone()
            });
        }
        props_dir.extend(
            e.propriedades
                .iter()
                .filter(|l| consome_entrada(&extras, &l.nome))
                .cloned(),
        );
        eventos_dir.extend(
            e.eventos
                .iter()
                .filter(|l| consome_saida(&extras, &l.nome))
                .cloned(),
        );
        // `viewProviders:` do filho: provedores privados do nó. Sem nós
        // filhos, entram no mesmo `ProviderNode` que os outros
        // (`createProviderNode`, `childNodeCount == 0`); com eles, num à
        // parte (`[n, n]`), ainda sem caso.
        let sem_nos_filhos = e.filhos.iter().all(|x| match x {
            No::Texto(t) => !self.preservar_espacos && t.trim().is_empty(),
            No::Comentario(_) => true,
            _ => false,
        });
        if !sem_nos_filhos
            && filho
                .metadados
                .as_ref()
                .is_some_and(|m| !m.provedores_de_visao.is_empty())
        {
            return Err(em_filho("filho com viewProviders e nós filhos"));
        }
        // Os provedores do nó: o componente primeiro, depois as diretivas.
        // `Visibility.all` põe o filho no `injectorGetInternal`: é o
        // caminho resolvido que o escreve.
        let com_provedores = !extras.is_empty()
            || filho.metadados.as_ref().is_some_and(|m| {
                !m.provedores.is_empty() || !m.provedores_de_visao.is_empty() || m.visivel
            });
        let no_resolvido = if com_provedores {
            let meta = filho
                .metadados
                .clone()
                .ok_or_else(|| em_filho("filho com providers ou diretiva sem metadados"))?;
            let casadas = casadas_do_no_do_filho(&meta, &extras, self.usadas);
            if let Err(f) = provedores_escreviveis(&meta) {
                return Err(em_filho(&format!("filho com providers: {f}")));
            }
            Some(casadas)
        } else {
            None
        };
        // `#ref` no filho vale a instância do componente; com valor, a da
        // diretiva do nó com aquele `exportAs` (`identifierForReference`,
        // `ast_template_parser.dart:854-868`), escolhida adiante. Com valor,
        // o nome não lido é só um nome, como no elemento HTML.
        for r in &e.referencias {
            if !r.valor.is_empty() {
                continue;
            }
            if !self.refs_livres.contains(&r.nome)
                && !self.refs_locais.contains(&r.nome)
                && !self.refs_consultados.iter().any(|(n, _, _)| *n == r.nome)
            {
                return Err(em_filho(if self.embutida {
                    "#ref no filho em visão embutida"
                } else {
                    "#ref no filho usado em expressão"
                }));
            }
        }
        for a in &e.atributos {
            // O literal é o `TabIndexBinding` do elemento (escrito com os
            // atributos); o interpolado seria propriedade do elemento do
            // filho, forma ainda sem caso.
            if a.nome.eq_ignore_ascii_case("tabindex") && a.valor.contains("{{") {
                return Err(em_filho("tabindex interpolado no filho"));
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
        // A `[x]` que só uma diretiva do nó recebe não é do filho; a que
        // ninguém recebe é propriedade do elemento do filho
        // (`_visitProperties`: `[class.x]`, `[attr.x]`, `[id]`…; o esquema
        // aceita qualquer propriedade num elemento customizado), escrita em
        // `bindRenderInputs` (casos j74, j135).
        ligadas.extend(
            e.propriedades
                .iter()
                .filter(|l| filho.entrada(&l.nome).is_some())
                .map(|l| (l, false)),
        );
        let props_elemento: Vec<crate::html::Ligacao> = e
            .propriedades
            .iter()
            .filter(|l| filho.entrada(&l.nome).is_none() && !consome_entrada(&extras, &l.nome))
            .cloned()
            .collect();
        // O atributo interpolado que nem o filho nem uma diretiva do nó
        // recebe é propriedade do elemento do filho, depois das `[x]` dele
        // (`_visitProperties`, caso j92).
        let interpolados_do_elemento: Vec<crate::html::Ligacao> = e
            .atributos
            .iter()
            .filter(|a| {
                a.valor.contains("{{")
                    && filho.entrada(&a.nome).is_none()
                    && !consome_entrada(&extras, &a.nome)
            })
            .cloned()
            .collect();
        for (i, (l, _)) in ligadas.iter().enumerate() {
            if ligadas[..i].iter().any(|(x, _)| x.nome == l.nome) {
                return Err(em_filho("@Input do filho ligado duas vezes"));
            }
        }
        let n = self.proximo;
        self.proximo += 1;
        // Com ligação própria, o nó do filho vira campo (a detecção o lê).
        // `detectHostChanges(visão, el)` de diretiva com `@HostBinding` lê o
        // nó na detecção.
        let com_hospedeiro = extras.iter().any(|d| !d.ligacoes_do_hospedeiro.is_empty());
        let el = if props_elemento.is_empty()
            && interpolados_do_elemento.is_empty()
            && !com_hospedeiro
        {
            format!("_el_{n}")
        } else {
            let html = self.html.clone();
            self.campos_el
                .push(format!("  late final {html}.HtmlElement _el_{n};"));
            format!("this._el_{n}")
        };
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
        // Filho do mesmo arquivo: a visão dele está ao lado, sem import.
        let vt = if e_o_proprio_template(&self.asset, &cam_template) {
            String::new()
        } else {
            self.imp.q(&cam_template)
        };
        let vd = self.imp.alias(&cam_dart);
        let classe = &filho.classe;
        let campo_vista = format!("_compView_{n}");
        // Filho que injeta `ViewContainerRef`: o nó ganha um `ViewContainer`
        // (campo entre a visão e a instância) e três embutidos a mais, que
        // levam a instância ao 8 (caso j47).
        let container = filho
            .parametros
            .iter()
            .any(|p| matches!(p, Injetado::Container))
            || extras.iter().any(|d| crate::diretivas::pede_container(d));
        // Os provedores do nó, na ordem do oficial: as diretivas na ordem de
        // `directives:` com o filho no lugar dele (`_matchDirectives`), cada
        // provedor ansioso depois das dependências (`_getOrCreateLocalProvider`)
        // — o que o filho injeta do próprio nó e as diretivas que vêm antes
        // dele saem antes da instância (casos j61, j67). As leituras de cima
        // levam o import do `unsafeCast` tardio, alocado onde o texto sai.
        let resolvido = match &no_resolvido {
            Some((indice, casadas)) => {
                let provedores = self.provedores_acima_com(&tardio(UTILITIES));
                let acima = crate::diretivas::Acima {
                    provedores: &provedores,
                    incerto: self.incertos_acima > 0,
                    de_visao: self.tokens_de_visao,
                };
                let pedidos = pedidos_ao_no_do_filho(e, casadas, self.filhos, self.usadas)?;
                let r = crate::diretivas::resolver_no_do_filho(
                    casadas,
                    *indice,
                    n,
                    Some(acima),
                    container,
                    &pedidos,
                )
                .map_err(|f| recusa(Motivo::DiretivaPorSeletor, f))?;
                let token = casadas[*indice].token();
                let pos = r
                    .instancias
                    .iter()
                    .position(|i| i.token == token)
                    .ok_or_else(|| em_filho("nó do filho sem a instância do filho"))?;
                Some((r, pos))
            }
            None => None,
        };
        let campo_inst = match &resolvido {
            Some((r, pos)) => r.instancias[*pos].campo.clone(),
            None => format!("_{classe}_{n}_{}", if container { 8 } else { 5 }),
        };
        // As diretivas do nó antes e depois do filho, na ordem das ligações
        // e dos ganchos (`transformedDirectiveAsts`).
        let (dir_antes, dir_depois) = match &resolvido {
            Some((r, _)) => {
                let k = r
                    .diretivas
                    .iter()
                    .position(|(_, c)| *c == campo_inst)
                    .ok_or_else(|| em_filho("nó do filho sem a diretiva do filho"))?;
                (r.diretivas[..k].to_vec(), r.diretivas[k + 1..].to_vec())
            }
            None => (Vec::new(), Vec::new()),
        };
        // `directiveTypes:` tipa os campos (a visão e a instância), não a
        // criação (`lookupTypeArgumentsOf`, `compile_view.dart:891-905`).
        let argumentos = self.argumentos_de_tipo(&filho.uri_dart, &filho.classe, e);
        self.campos_filho.push(format!(
            "  late final {vt}View{classe}0{argumentos} {campo_vista};"
        ));
        if container {
            let vc = self.imp.q(VIEW_CONTAINER);
            self.campos_filho
                .push(format!("  late final {vc}ViewContainer _appEl_{n};"));
        }
        self.vistas_filhas.push(campo_vista.clone());
        if filho.link_de_deteccao {
            self.vistas_ligadas.push(campo_vista.clone());
        }
        if filho.hospedeiro {
            self.usa_primeira_checagem = true;
        }
        self.linhas.push(format!(
            "    this.{campo_vista} = {vt}View{classe}0(this, {n});"
        ));
        self.linhas.push(format!(
            "    {} = this.{campo_vista}.rootElement;",
            if el.starts_with("this.") {
                el.clone()
            } else {
                format!("final {el}")
            }
        ));
        // Na raiz da visão embutida, ou projetado, o nó não tem pai aqui (com
        // `ViewContainer`, a raiz é ele: `vcAppEl ?? renderNode`).
        if pai.is_empty() {
            self.raizes.push(if container {
                format!("this._appEl_{n}")
            } else {
                el.clone()
            });
        } else {
            self.linhas.push(format!("    {pai}.append({el});"));
        }
        // Os atributos escritos, em ordem alfabética (`_toSortedBindings`),
        // todos — também os que alimentam um `@Input` —, e o `class` pelo
        // `updateChildClassNonHtml`: o elemento do filho não é HTML
        // (`writeLiteralAttributeValues`).
        // O interpolado é ligação, não atributo escrito (caso j58).
        let mut atributos: Vec<_> = e
            .atributos
            .iter()
            .filter(|a| !a.valor.contains("{{"))
            .cloned()
            .collect();
        atributos.sort_by(|a, b| a.nome.cmp(&b.nome));
        for a in &atributos {
            let mut valor = literal(&a.valor);
            // `_mergeHtmlAndDirectiveAttrs`: o `class` estático do componente
            // só é escrito aqui quando o elemento também tem `class`; os dois
            // viram uma interpolação (o do componente continua no construtor
            // da visão dele, e este sobrescreve em tempo de execução). O
            // `style` mesclado ainda não tem caso; os demais ficam com o
            // atributo escrito.
            if let Some((_, membro)) = filho
                .atributos_do_hospedeiro
                .iter()
                .find(|(nome, _)| *nome == a.nome)
            {
                match a.nome.as_str() {
                    "class" => {
                        let interp = self.imp.q(INTERPOLATE);
                        valor = format!(
                            "{interp}interpolate2('', {valor}, ' ', {vd}.{classe}.{membro}, '')"
                        );
                    }
                    "style" => {
                        return Err(em_filho("style estático do filho mesclado com o escrito"));
                    }
                    _ => {}
                }
            }
            if a.nome == "class" {
                self.linhas
                    .push(format!("    this.updateChildClassNonHtml({el}, {valor});"));
            } else if a.nome == "tabindex" || a.nome == "tabIndex" {
                // `TabIndexBinding`, como no elemento HTML (caso j102).
                match a.valor.trim().parse::<i64>() {
                    Ok(n) if a.valor.trim() == a.valor => {
                        self.linhas.push(format!("    {el}.tabIndex = {n};"));
                    }
                    _ => return Err(em_filho("tabindex que não é inteiro no filho")),
                }
            } else {
                let dom = self.dom();
                self.linhas.push(format!(
                    "    {dom}.setAttribute({el}, '{}', {valor});",
                    a.nome
                ));
            }
        }
        if self.com_estilo {
            self.linhas.push(format!("    this.addShimC({el});"));
        }
        // O `ViewContainer` nasce com o `CompileElement`, antes dos
        // provedores; o segundo argumento é o pai (`null` na raiz).
        if container {
            let vc = self.imp.q(VIEW_CONTAINER);
            let pai_indice = if pai.is_empty() {
                self.pai_projetado
                    .map_or_else(|| "null".to_string(), |k| k.to_string())
            } else {
                indice_do_elemento(pai)
            };
            self.linhas.push(format!(
                "    this._appEl_{n} = {vc}ViewContainer({n}, {pai_indice}, this, {el});"
            ));
            self.ancoras.push(format!("_appEl_{n}"));
        }
        let antes_do_filho = resolvido
            .as_ref()
            .map_or(&[][..], |(r, pos)| &r.instancias[..*pos]);
        self.criar_instancias(
            e,
            antes_do_filho,
            &el.clone(),
            &format!("this.{campo_vista}"),
        )?;
        self.campos_filho.push(format!(
            "  late final {vd}.{classe}{argumentos} {campo_inst};"
        ));
        let do_no: Vec<(crate::diretivas::Token, String)> = antes_do_filho
            .iter()
            .flat_map(|i| {
                std::iter::once(&i.token)
                    .chain(&i.apelidos)
                    .map(|t| (t.clone(), i.leitura.clone()))
            })
            .collect();
        let construcao = self.construcao_do_filho(e, filho, n, &el, &vd, &campo_vista, &do_no)?;
        self.linhas
            .push(format!("    this.{campo_inst} = {construcao};"));
        let mut acima = self.pilha.clone();
        acima.push((n, true));
        self.registros.push(Registro {
            acima,
            provedores: vec![(
                crate::diretivas::Token::Classe {
                    uri: filho.uri_dart.clone(),
                    classe: filho.classe.clone(),
                },
                campo_inst.clone(),
                filho.on_push.then(|| campo_vista.clone()),
            )],
            elemento: el.clone(),
        });
        // O alvo de cada `#ref`: sem valor, o componente; com valor, a única
        // diretiva do nó (o componente inclusive) com `exportAs` igual —
        // o campo dela já traz o `.instance` de uma `XNgCd`.
        let mut refs_ao_componente = Vec::new();
        for r in &e.referencias {
            let leitura = if r.valor.is_empty() {
                refs_ao_componente.push(r.nome.clone());
                format!("this.{campo_inst}")
            } else {
                let achadas: Vec<&String> = match &resolvido {
                    Some((res, _)) => res
                        .diretivas
                        .iter()
                        .filter(|(d, _)| d.export_as.as_deref() == Some(r.valor.as_str()))
                        .map(|(_, c)| c)
                        .collect(),
                    None => filho
                        .metadados
                        .iter()
                        .filter(|m| m.export_as.as_deref() == Some(r.valor.as_str()))
                        .map(|_| &campo_inst)
                        .collect(),
                };
                match achadas.as_slice() {
                    [c] => {
                        if **c == campo_inst {
                            refs_ao_componente.push(r.nome.clone());
                        }
                        format!("this.{c}")
                    }
                    [] => return Err(em_filho("#ref com valor sem diretiva que o exporte")),
                    _ => {
                        return Err(em_filho(
                            "#ref com valor exportado por mais de uma diretiva",
                        ));
                    }
                }
            };
            self.refs.insert(r.nome.clone(), leitura.clone());
            self.refs_em_ordem.push((r.nome.clone(), leitura));
        }
        // O resultado de um `@ViewChild(Tipo)` ([`chave_de_tipo`]), também
        // lido de uma visão de cima (consulta dinâmica).
        let chave = chave_de_tipo(&filho.uri_dart, &filho.classe);
        self.refs
            .entry(chave_de_no(&chave, e.inicio))
            .or_insert_with(|| format!("this.{campo_inst}"));
        if let Some((r, _)) = &resolvido {
            let instancias = r.instancias.clone();
            self.registrar_no_por_posicao(e.inicio, &instancias);
        }
        self.refs
            .insert(chave.clone(), format!("this.{campo_inst}"));
        self.refs_em_ordem
            .push((chave.clone(), format!("this.{campo_inst}")));
        if filho.on_push {
            // O resultado de consulta de conteúdo dinâmica pela posição.
            self.detectores
                .insert(chave_de_no(&chave, e.inicio), campo_vista.clone());
            self.detectores_em_ordem
                .push((chave.clone(), campo_vista.clone()));
            self.detectores.insert(chave, campo_vista.clone());
        }
        // `buildChangeDetectorRef()` só existe para o componente `onPush`:
        // o `#ref` que vale uma diretiva não registra detector.
        for nome in &refs_ao_componente {
            if filho.on_push {
                self.detectores_em_ordem
                    .push((nome.clone(), campo_vista.clone()));
                self.detectores.insert(nome.clone(), campo_vista.clone());
            }
        }
        // `bindRenderInputs`: as ligações do próprio elemento do filho,
        // antes das das diretivas (caso j74).
        if !props_elemento.is_empty() || !interpolados_do_elemento.is_empty() {
            self.tag_atual = e.nome.clone();
            self.instancia_da_visao = Some(format!("this.{campo_vista}"));
            let mut ligadas_el = Vec::new();
            let mut falha = None;
            for l in &props_elemento {
                match self.propriedade(l, &el) {
                    Ok(x) => ligadas_el.push(x),
                    Err(r) => {
                        falha = Some(r);
                        break;
                    }
                }
            }
            if falha.is_none() {
                for a in &interpolados_do_elemento {
                    match self.atributo_interpolado(a, &el) {
                        Ok(x) => ligadas_el.push(x),
                        Err(r) => {
                            falha = Some(r);
                            break;
                        }
                    }
                }
            }
            self.instancia_da_visao = None;
            if let Some(r) = falha {
                self.anotar(r)?;
            }
            self.escrever_ligacoes(ligadas_el);
        }
        self.vista_do_hospedeiro = Some(format!("this.{campo_vista}"));
        if let Some((r, _)) = &resolvido {
            self.ligar_diretivas(
                e,
                &r.instancias,
                &dir_antes,
                &el.clone(),
                &props_dir,
                &eventos_dir,
                true,
                false,
            )?;
        }
        self.vista_do_hospedeiro = None;
        self.entradas_do_filho(&ligadas, filho, &campo_inst, &campo_vista)?;
        // `bindDirectiveHostProps` do componente: com as ligações de
        // propriedade, na ordem de documento, logo depois das entradas dele
        // — não junto do `detectChanges` da visão dele.
        if filho.hospedeiro {
            self.deteccao.push(format!(
                "    this.{campo_vista}.detectHostChanges(firstCheck);"
            ));
        }
        let mut sem_as_da_diretiva = e.clone();
        sem_as_da_diretiva
            .eventos
            .retain(|l| !consome_saida(&extras, &l.nome));
        // `bindRenderOutputs` (os eventos do elemento) antes das saídas das
        // diretivas, que seguem a ordem delas.
        self.eventos_do_elemento_do_filho(
            &sem_as_da_diretiva,
            filho,
            &el,
            resolvido.as_ref().map(|(r, _)| (extras.as_slice(), r)),
        )?;
        if let Some((r, _)) = &resolvido {
            self.ligar_diretivas(
                e,
                &r.instancias,
                &dir_antes,
                &el.clone(),
                &props_dir,
                &eventos_dir,
                false,
                true,
            )?;
        }
        self.saidas_do_filho(&sem_as_da_diretiva, filho, &campo_inst)?;
        // Os outros provedores do nó (acessor de valor por `providers:` do
        // filho, `NgModel`): campos depois da instância, entradas e saídas
        // depois das do filho (`transformedDirectiveAsts`).
        let mut injetor_do_filho = None;
        let antes_acima = self.acima.len();
        if let Some((r, pos)) = &resolvido {
            let pos = *pos;
            // Os preguiçosos que o conteúdo pede já vêm ansiosos e na
            // posição certa do resolvedor ([`pedidos_ao_no_do_filho`]).
            let depois = r.instancias[pos + 1..].to_vec();
            // Os provedores depois do filho, o `registerDirective` de todas
            // as diretivas do nó (menos o componente) e as ligações das que
            // vêm depois dele.
            let alvo = el.clone();
            self.criar_instancias(e, &depois, &alvo, &format!("this.{campo_vista}"))?;
            let todas: Vec<_> = dir_antes.iter().chain(&dir_depois).cloned().collect();
            self.registrar_diretivas(&todas, &alvo);
            self.vista_do_hospedeiro = Some(format!("this.{campo_vista}"));
            let ligadas = self.ligar_diretivas(
                e,
                &r.instancias,
                &dir_depois,
                &alvo,
                &props_dir,
                &eventos_dir,
                true,
                true,
            );
            self.vista_do_hospedeiro = None;
            ligadas?;
            // O filho entra primeiro no `injectorGetInternal`, pelos
            // apelidos dele (`ExistingProvider(X, OFilho)`) e, visível, pela
            // classe.
            let injetaveis: Vec<(Vec<crate::diretivas::Token>, String)> = r
                .instancias
                .iter()
                .filter(|i| !i.injetavel_por.is_empty())
                .map(|i| (i.injetavel_por.clone(), i.leitura.clone()))
                .collect();
            if !injetaveis.is_empty() {
                injetor_do_filho = Some(self.injetores.len());
                self.injetores.push((n, n, injetaveis));
            }
            for i in &r.instancias {
                // A visibilidade só decide o `injectorGetInternal` (outras
                // visões); no mesmo template o nó de baixo acha até o
                // provedor local pelo token dele (caso j41).
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
            let mut acima_reg = self.pilha.clone();
            acima_reg.push((n, true));
            // O token do filho já está no registro dele; os apelidos dele,
            // aqui.
            let provedores = r
                .instancias
                .iter()
                .flat_map(|i| {
                    (i.campo != campo_inst)
                        .then_some(&i.token)
                        .into_iter()
                        .chain(&i.apelidos)
                        .map(|t| (t.clone(), i.leitura.clone(), None))
                })
                .collect();
            self.registros.push(Registro {
                acima: acima_reg,
                provedores,
                elemento: el.clone(),
            });
        } else if let Some(meta) = &filho.metadados {
            // Sem `providers:`, outra diretiva nem `Visibility.all`, o nó do
            // filho só provê a classe dele ao conteúdo do mesmo template.
            self.acima.push((meta.token(), campo_inst.clone(), None));
        }
        if filho.projecoes.is_empty() && e.filhos.is_empty() {
            self.consultas_das_diretivas(e, &dir_antes, n)?;
            self.consultas_do_filho(e, filho, &campo_inst, n)?;
            self.consultas_das_diretivas(e, &dir_depois, n)?;
            self.ganchos_depois_dos_filhos(&dir_antes);
            self.depois_dos_filhos(filho, &campo_inst);
            self.ganchos_depois_dos_filhos(&dir_depois);
            self.linhas
                .push(format!("    this.{campo_vista}.create(this.{campo_inst});"));
            self.acima.truncate(antes_acima);
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
        self.pilha.push((n, true));
        self.componentes_acima += 1;
        let incerto = filho.metadados.is_none();
        if incerto {
            self.incertos_acima += 1;
        }
        let mut r = Ok(());
        for no in &e.filhos {
            let indice = match no {
                No::Comentario(_) => continue,
                No::Elemento(x) => {
                    let mut sem_estrela = x.clone();
                    sem_estrela.estrela = None;
                    let desc = crate::seletor::Elemento::do_template(&sem_estrela);
                    seletores
                        .iter()
                        .position(|s| {
                            s.as_ref()
                                .is_some_and(|s| crate::seletor::casa_algum(s, &desc))
                        })
                        .or(curinga)
                }
                _ => curinga,
            };
            // Conteúdo que nenhuma projeção recebe (`ngContentIndex` nulo; o
            // filho sem `<ng-content>` não recebe nada): o texto, ligado ou
            // não, é pulado e só consome o índice do nó (`_maybeSkipNode`, o
            // oficial avisa "Dead code in template"); o elemento e a âncora
            // do `<template>` são criados e descartados (`addContentNode`
            // não é chamado) — as diretivas deles existem e entram nas
            // consultas.
            let descartado = match (indice, no) {
                (Some(_), _) => false,
                (None, No::Texto(_) | No::Interpolacao { .. }) => {
                    self.proximo += 1;
                    continue;
                }
                (None, No::Elemento(x)) if x.nome != "ng-container" || x.estrela.is_some() => true,
                (None, _) => {
                    r = Err(recusa(
                        Motivo::Projecao,
                        "<ng-container> que nenhuma projeção recebe",
                    ));
                    break;
                }
            };
            let antes = self.raizes.len();
            if let Err(x) = self.nos(std::slice::from_ref(no), "") {
                r = Err(x);
                break;
            }
            let criados: Vec<String> = self.raizes.drain(antes..).collect();
            if let (false, Some(i)) = (descartado, indice) {
                listas[i].extend(criados);
            }
        }
        self.pai_projetado = pai_antes;
        self.filhos_acima.pop();
        self.pilha.pop();
        self.componentes_acima -= 1;
        if incerto {
            self.incertos_acima -= 1;
        }
        r?;
        self.consultas_das_diretivas(e, &dir_antes, n)?;
        self.consultas_do_filho(e, filho, &campo_inst, n)?;
        self.consultas_das_diretivas(e, &dir_depois, n)?;
        self.ganchos_depois_dos_filhos(&dir_antes);
        self.depois_dos_filhos(filho, &campo_inst);
        self.ganchos_depois_dos_filhos(&dir_depois);
        // Sem projeções: `create`, como o filho sem conteúdo. Todas vazias:
        // uma linha só, constantes. Senão, uma lista por linha (o `dart
        // format` quebra a lista que tem outra não vazia).
        let texto = if listas.is_empty() {
            format!("    this.{campo_vista}.create(this.{campo_inst});")
        } else if listas.iter().all(Vec::is_empty) {
            let vazias = vec!["const <Object>[]"; listas.len()];
            format!(
                "    this.{campo_vista}.createAndProject(this.{campo_inst}, [{}]);",
                vazias.join(", ")
            )
        } else {
            // Cada lista pelo `createFlatArrayForProjectNodes` (um
            // `<ng-content>` reprojetado é uma lista inteira, caso j57).
            let reprojeta = listas.iter().flatten().any(|x| x.starts_with(RAIZ_LISTA));
            let util = if reprojeta {
                self.imp.alias(UTILITIES)
            } else {
                String::new()
            };
            // Na lista quebrada, cada item começa na coluna 6.
            let mut itens: Vec<String> = Vec::new();
            for l in &listas {
                itens.push(lista_plana(l, &util)?.texto_em(6));
            }
            // O `dart format` quebra a lista de fora só quando um item é uma
            // coleção literal não vazia (a lista reprojetada sozinha não é).
            if itens
                .iter()
                .any(|t| t.starts_with("<Object>[") && t != "<Object>[]")
            {
                let linhas: Vec<String> = itens.iter().map(|t| format!("      {t}")).collect();
                format!(
                    "    this.{campo_vista}.createAndProject(this.{campo_inst}, [\n{}\n    ]);",
                    linhas.join(",\n")
                )
            } else {
                format!(
                    "    this.{campo_vista}.createAndProject(this.{campo_inst}, [{}]);",
                    itens.join(", ")
                )
            }
        };
        self.linhas.push(texto);
        self.acima.truncate(antes_acima);
        // `ProviderNode(nodeIndex, nodeIndex + childNodeCount)`.
        if let Some(i) = injetor_do_filho {
            self.injetores[i].1 = self.proximo - 1;
        }
        Ok(())
    }

    /// A visão de onde o injetor de fora é lido (`injectFromViewParentInjector`
    /// escrito na visão do nó mais alto da cadeia de injetores e levado a
    /// esta por `getPropertyInView`): `this`, ou a cadeia de `parentView`
    /// até ela (casos i76, j40).
    fn visao_do_injetor(&self) -> String {
        let saltos = self.profundidade - self.nivel_do_topo;
        let mut v = "this".to_string();
        for k in 0..saltos {
            v = if k == 0 {
                "(this.parentView!)".to_string()
            } else {
                format!("({v}.parentView!)")
            };
        }
        v
    }

    /// A construção da instância do filho, texto depois de `this._X_n_5 = `
    /// (sem o `;`): o nó, a visão do filho e os serviços pela visão de cima.
    /// Com serviço, o oficial embrulha em `debugInjectorWrap` sob
    /// `isDevMode`, como na hospedeira.
    fn construcao_do_filho(
        &mut self,
        e: &crate::html::Elemento,
        filho: &Filho,
        n: u32,
        el: &str,
        vd: &str,
        campo_vista: &str,
        do_no: &[(crate::diretivas::Token, String)],
    ) -> Result<String, Recusa> {
        let em_filho = |f: &str| recusa(Motivo::LigacaoEmFilho, f);
        let classe = &filho.classe;
        let do_proprio_no = |token: &crate::diretivas::Token| {
            do_no.iter().find_map(|(t, c)| (t == token).then_some(c))
        };
        // Os provedores dos elementos acima (`_getDependency` sobe por eles
        // antes do injetor de fora), com o import do `unsafeCast` tardio.
        let provedores_acima = self.provedores_acima_com(&tardio(UTILITIES));
        let de_cima =
            |token: &crate::diretivas::Token| provedores_acima.iter().find(|p| p.token == *token);
        // A classe do componente desta visão: o `@Host()` a acha no injetor
        // (`identifierToken(component.type).equalsTo(dep.token)`).
        let proprio_componente = |token: &crate::diretivas::Token| match token {
            crate::diretivas::Token::Classe { uri, classe: tipo } => {
                self.classe_qualificada.rsplit('.').next() == Some(tipo.as_str())
                    && asset_de_uri(uri, "", Path::new("")).as_deref() == Some(self.asset.as_str())
            }
            _ => false,
        };
        // `_getDependency`: o próprio nó (sem `@SkipSelf`); com `@Self`, só
        // ele; senão os elementos acima; e, não achando, o injetor de fora —
        // menos com `@Host` numa visão de componente (a não ser que o token
        // seja o próprio componente), onde fica `null` se `@Optional`.
        let mut origens = Vec::new();
        for p in &filho.parametros {
            let Injetado::Servico {
                token,
                opcional,
                proprio,
                hospedeiro,
                pular,
            } = p
            else {
                origens.push(None);
                continue;
            };
            if !pular && let Some(c) = do_proprio_no(token) {
                origens.push(Some(OrigemDoServico::Local(c.clone())));
                continue;
            }
            if *proprio {
                if !opcional {
                    return Err(em_filho("@Self() sem provedor no nó do filho"));
                }
                origens.push(Some(OrigemDoServico::Nulo));
                continue;
            }
            // Embutido do elemento: o oficial o acha no nó, nunca no
            // injetor de fora (ainda sem caso no nó do filho).
            if crate::diretivas::embutido_do_elemento(token) {
                return Err(em_filho(
                    "filho que injeta embutido do elemento (ViewContainerRef…)",
                ));
            }
            // Um elemento acima provê o serviço (também um componente acima,
            // cujo conteúdo contém o filho): o oficial o lê de lá — o campo,
            // ou `.instance` do `XNgCd` (casos j68, j79).
            if let Some(p) = de_cima(token) {
                // Provedor preguiçoso do elemento acima: o oficial o
                // transformaria agora, mudando índice e forma lá (L1).
                if p.preguicoso {
                    return Err(em_filho(
                        "provedor preguiçoso de um elemento acima pedido abaixo (L1)",
                    ));
                }
                origens.push(Some(OrigemDoServico::Cima(p.leitura.clone())));
                continue;
            }
            // Um componente acima sem metadados poderia prover o serviço:
            // não achar não prova que vem de fora.
            if self.incertos_acima > 0 {
                return Err(em_filho(
                    "filho que injeta serviço sob componente sem metadados",
                ));
            }
            if *hospedeiro && !proprio_componente(token) && !self.tokens_de_visao.contains(token) {
                if !opcional {
                    return Err(em_filho("@Host() sem provedor na visão do filho"));
                }
                origens.push(Some(OrigemDoServico::Nulo));
                continue;
            }
            origens.push(Some(OrigemDoServico::Injetor));
        }
        // Só a dependência do injetor é dinâmica (`hasDynamicDependencies`):
        // a que o próprio nó ou um elemento acima provê não embrulha a
        // criação.
        let injeta = origens
            .iter()
            .any(|o| matches!(o, Some(OrigemDoServico::Injetor)));
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
        let visao_do_componente = self.visao_do_injetor();
        let mut args = Vec::new();
        for (p, origem) in filho.parametros.iter().zip(&origens) {
            match p {
                Injetado::Elemento => args.push(el.to_string()),
                Injetado::Detector => args.push(format!("this.{campo_vista}")),
                Injetado::Container => args.push(format!("this._appEl_{n}")),
                // O valor literal do atributo no elemento (`_attrs[nome]`),
                // ou `null`.
                Injetado::Atributo(nome) => args.push(
                    e.atributos
                        .iter()
                        .find(|a| a.nome == *nome && !a.valor.contains("{{"))
                        .map_or_else(|| "null".to_string(), |a| literal(&a.valor)),
                ),
                Injetado::Servico {
                    token, opcional, ..
                } => {
                    match origem {
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
                        crate::diretivas::Token::Classe { uri, classe: tipo } => {
                            let caminho = asset_de_uri(uri, "", Path::new(""))
                                .and_then(|alvo| caminho_do_import(&self.asset, &alvo))
                                .ok_or_else(|| {
                                    em_filho("tipo injetado no filho sem caminho de import")
                                })?;
                            format!("{}{tipo}", self.imp.q(&caminho))
                        }
                        // `const OpaqueToken<T>('x')`, com os imports na
                        // ordem do texto.
                        outro => resolver_tardios(self.imp, &expr_do_token(outro)),
                    };
                    let metodo = if *opcional {
                        "injectorGetOptional"
                    } else {
                        "injectorGet"
                    };
                    let v = &visao_do_componente;
                    args.push(format!(
                        "({v}.parentView!).{metodo}({expr}, {v}.parentIndex)"
                    ));
                }
            }
        }
        let chamada = resolver_tardios(self.imp, &format!("{vd}.{classe}({})", args.join(", ")));
        Ok(match prefixo {
            None => chamada,
            Some((util, erros)) => format!(
                "({util}.isDevMode\n        ? {erros}.debugInjectorWrap({vd}.{classe}, () {{\n            return {chamada};\n          }})\n        : {chamada})"
            ),
        })
    }

    /// As consultas de conteúdo do filho, no `afterChildren` do nó
    /// (`updateQueryAtStartup`). O resultado estático é o das instâncias
    /// criadas no conteúdo que casam o tipo procurado, em pré-ordem
    /// (`addQueryResult`), filtradas pela distância do `_getQueriesFor`
    /// quando a consulta não é `descendants`: a lista recebe `[a, b]` (ou
    /// `[]`), a única nada quando vazia. Resultado dentro de `*` (o
    /// `mapNestedViews`), `#ref` achado e consulta única com resultado ainda
    /// são recusados.
    fn consultas_do_filho(
        &mut self,
        e: &crate::html::Elemento,
        filho: &Filho,
        campo_inst: &str,
        n: u32,
    ) -> Result<(), Recusa> {
        self.consultas_de_conteudo_no(e, &filho.consultas, campo_inst, n)
    }

    /// `updateQueryAtStartup` das consultas de conteúdo das `diretivas` do
    /// nó `n`, no `afterChildren` dele, na ordem delas.
    fn consultas_das_diretivas(
        &mut self,
        e: &crate::html::Elemento,
        diretivas: &[(std::sync::Arc<crate::diretivas::Diretiva>, String)],
        n: u32,
    ) -> Result<(), Recusa> {
        for (d, campo) in diretivas {
            if !d.consultas_de_conteudo.is_empty() {
                self.consultas_de_conteudo_no(e, &d.consultas_de_conteudo, campo, n)?;
            }
        }
        Ok(())
    }

    /// As consultas de conteúdo de uma diretiva ou de um filho no nó `n`,
    /// com a instância lida por `campo_inst` ([`Self::consultas_do_filho`];
    /// a diretiva, caso j72).
    fn consultas_de_conteudo_no(
        &mut self,
        e: &crate::html::Elemento,
        consultas: &[ConsultaDoFilho],
        campo_inst: &str,
        n: u32,
    ) -> Result<(), Recusa> {
        let fora = |f: &str| recusa(Motivo::LigacaoEmFilho, f);
        for q in consultas {
            // `_queryCount` do nó: toda consulta de conteúdo dele conta.
            let indice = {
                let k = self.consultas_por_no.entry(n).or_default();
                *k += 1;
                *k - 1
            };
            let (uri, classe) = match &q.alvo {
                AlvoDeConsulta::Referencia(r) => {
                    if referencia_no_conteudo(&e.filhos, r) {
                        return Err(fora("@ContentChild do filho com #ref no conteúdo"));
                    }
                    if q.lista {
                        self.linhas
                            .push(format!("    this.{campo_inst}.{} = [];", q.campo));
                    }
                    continue;
                }
                AlvoDeConsulta::Classe(uri, classe) => (uri, classe),
            };
            if self.classe_em_embutida(&e.filhos, uri, classe, false) {
                // Sem `descendants`, o `<template>` (com diretiva) entre o
                // resultado e o nó já põe a distância acima de 1.
                if !q.descendentes || q.leitura.is_some() {
                    return Err(fora(
                        "@ContentChild do filho com resultado em visão embutida",
                    ));
                }
                let arvore = self.arvore_de_conteudo(&e.filhos, uri, classe)?;
                // A única com o primeiro resultado nesta visão é estática
                // (`_shouldMapNestedViews`): segue abaixo, pelo primeiro.
                let dinamica = if q.lista {
                    arvore
                        .iter()
                        .any(|i| matches!(i, ItemDeConteudo::Aninhada { .. }))
                } else {
                    matches!(arvore.first(), Some(ItemDeConteudo::Aninhada { .. }))
                };
                if dinamica {
                    self.conteudo_dinamico_no(
                        arvore,
                        chave_de_tipo(uri, classe),
                        classe,
                        n,
                        indice,
                        q.lista,
                        format!("this.{campo_inst}.{}", q.campo),
                        false,
                    );
                    continue;
                }
            }
            let token = crate::diretivas::Token::Classe {
                uri: uri.clone(),
                classe: classe.clone(),
            };
            let mut valores = Vec::new();
            // Os resultados que são componente `onPush`: o
            // `ChangeDetectorRef` de cada um é registrado antes da atribuição
            // (`_createAddQueryChangeDetectorRefs` em `_createUpdates`).
            let mut detectores: Vec<(String, String)> = Vec::new();
            for r in &self.registros {
                let Some(pos) = r.acima.iter().position(|(k, _)| *k == n) else {
                    continue;
                };
                let distancia = r.acima[pos + 1..].iter().filter(|(_, d)| *d).count();
                if !q.descendentes && distancia > 1 {
                    continue;
                }
                for (t, campo, visao_on_push) in &r.provedores {
                    if *t != token {
                        continue;
                    }
                    // Com `read:`, o valor é de outro provedor, sem
                    // `ChangeDetectorRef`.
                    if let (Some(cv), None) = (visao_on_push, &q.leitura) {
                        detectores.push((format!("this.{campo}"), cv.clone()));
                    }
                    valores.push(match &q.leitura {
                        None => format!("this.{campo}"),
                        // `read:` lê outro token do mesmo nó.
                        Some(LeituraDaConsulta::Elemento) => r.elemento.clone(),
                        Some(LeituraDaConsulta::Classe(u, c)) => {
                            let lido = crate::diretivas::Token::Classe {
                                uri: u.clone(),
                                classe: c.clone(),
                            };
                            let no = r.acima.last().map(|(k, _)| *k);
                            self.registros
                                .iter()
                                .filter(|x| x.acima.last().map(|(k, _)| *k) == no)
                                .flat_map(|x| &x.provedores)
                                .find(|(t, _, _)| *t == lido)
                                .map(|(_, c, _)| format!("this.{c}"))
                                .ok_or_else(|| {
                                    fora(
                                        "@ContentChild(.., read:) de token que o nó achado não tem",
                                    )
                                })?
                        }
                    });
                }
            }
            // A única fica só com o primeiro valor (`_buildQueryResults`
            // para no primeiro estático), e com o registro só dele.
            if !q.lista {
                detectores.retain(|(v, _)| Some(v) == valores.first());
            }
            if !detectores.is_empty() {
                let v = self.imp.alias(VIEW);
                for (valor, cv) in &detectores {
                    self.linhas.push(format!(
                        "    {v}.View.queryChangeDetectorRefs[{valor}] = this.{cv};"
                    ));
                }
            }
            if q.lista {
                self.linhas.push(format!(
                    "    this.{campo_inst}.{} = [{}];",
                    q.campo,
                    valores.join(", ")
                ));
            } else if let Some(primeiro) = valores.first() {
                // A única recebe o primeiro, em pré-ordem; sem resultado,
                // nada.
                self.linhas
                    .push(format!("    this.{campo_inst}.{} = {primeiro};", q.campo));
            }
        }
        Ok(())
    }

    /// Algum elemento dentro de um `*` do conteúdo casa a classe procurada
    /// (componente pela tag ou diretiva pelo seletor)?
    fn classe_em_embutida(&self, nos: &[No], uri: &str, classe: &str, dentro: bool) -> bool {
        nos.iter().any(|no| {
            let No::Elemento(x) = no else { return false };
            // O elemento de um `*` vai para a visão embutida; o `<template>`
            // escrito fica nesta, e só o conteúdo dele é da embutida.
            let molde = x.estrela.as_ref().is_some_and(|l| l.nome == MARCA_DE_MOLDE);
            let abaixo = dentro || x.estrela.is_some();
            let dentro = dentro || (x.estrela.is_some() && !molde);
            let mut sem = x.clone();
            sem.estrela = None;
            // O token também casa pelos `providers:` do nó (`ExistingProvider`
            // de uma diretiva, o `FocusableItem` do `FocusItemDirective`):
            // o `_getQueriesFor` olha todos os provedores resolvidos do nó.
            let token = crate::diretivas::Token::Classe {
                uri: uri.to_string(),
                classe: classe.to_string(),
            };
            let fornece = |d: &crate::diretivas::Diretiva| {
                (d.uri == uri && d.classe == classe)
                    || d.provedores.iter().any(|p| p.token == token)
            };
            let casa = self.filhos.get(&x.nome).is_some_and(|f| {
                (f.uri_dart == uri && f.classe == classe)
                    || f.metadados.as_deref().is_some_and(fornece)
            }) || diretivas_casadas(self.usadas, &sem)
                .iter()
                .any(|d| (d.uri == uri && d.classe == classe) || fornece(d));
            (dentro && casa) || self.classe_em_embutida(&x.filhos, uri, classe, abaixo)
        })
    }

    /// `bindDirectiveAfterChildrenCallbacks` de cada diretiva do nó, na
    /// ordem delas, depois dos filhos (de baixo para cima): `ngAfterContent*`
    /// e `ngAfterView*` (o `Init` num `if (firstCheck)`, `addStmtsIfFirstCheck`)
    /// e `ngOnDestroy` (caso j65).
    fn ganchos_depois_dos_filhos(
        &mut self,
        diretivas: &[(std::sync::Arc<crate::diretivas::Diretiva>, String)],
    ) {
        for (d, campo) in diretivas {
            let g = &d.ganchos;
            if g.after_content_init {
                self.usa_primeira_checagem = true;
                na_primeira_checagem(
                    &mut self.apos_conteudo,
                    &[format!("      this.{campo}.ngAfterContentInit();")],
                );
            }
            if g.after_content_checked {
                self.apos_conteudo
                    .push(format!("    this.{campo}.ngAfterContentChecked();"));
            }
            if g.after_view_init {
                self.usa_primeira_checagem = true;
                na_primeira_checagem(
                    &mut self.apos_visao,
                    &[format!("      this.{campo}.ngAfterViewInit();")],
                );
            }
            if g.after_view_checked {
                self.apos_visao
                    .push(format!("    this.{campo}.ngAfterViewChecked();"));
            }
            if g.on_destroy {
                self.destruir
                    .push(format!("    this.{campo}.ngOnDestroy();"));
            }
        }
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
        // O tipo `bool` da entrada (`_isBoolType`), dos metadados do filho:
        // o atributo sem valor liga `true` nele, `''` nos outros (caso j73).
        let booleana = |nome: &str| {
            filho
                .metadados
                .as_ref()
                .and_then(|m| m.entrada(nome))
                .and_then(|x| x.booleana)
        };
        let spec: Vec<(String, String, Option<bool>)> = filho
            .entradas
            .iter()
            .map(|e| (e.nome.clone(), e.campo.clone(), booleana(&e.nome)))
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
                let mudou = if calcula { "\nchanged = true;" } else { "" };
                // Atributo com `{{ }}` num `@Input` (`x="a {{b}}"`): a
                // interpolação vai para a entrada — mutável, conferida e
                // atribuída (a primitiva sozinha, crua e interpolada na
                // atribuição); imutável, na primeira checagem (caso j58).
                if *estatico && l.valor.contains("{{") {
                    let v = self.valor_interpolado(&l.valor, motivo)?;
                    if v.imutavel {
                        let valor = v.constante;
                        let bloco = format!(
                            "if ({dev}.isDevToolsEnabled) {{\n  {dev}.Inspector.instance.recordInput(this.{campo_inst}, '{nome}', {valor});\n}}\nthis.{campo_inst}.{campo} = {valor} /* REF:{url}:{ini}:{fim} */;{mudou}"
                        );
                        constantes.push(indentar(&bloco, 6));
                        continue;
                    }
                    let k = v.k;
                    let expr = literal(&l.valor);
                    let chk = tardio(CHECK_BINDING);
                    let (checagem, na_acao) = (v.checagem, v.na_acao);
                    let mudou = if calcula {
                        "\n      changed = true;"
                    } else {
                        ""
                    };
                    self.campos_expr.push(format!("  Object? _expr_{k};"));
                    dinamicas.push(format!(
                        "    final currVal_{k} = {checagem};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, {expr}, '{url}')) {{\n      if ({dev}.isDevToolsEnabled) {{\n        {dev}.Inspector.instance.recordInput(this.{campo_inst}, '{nome}', {na_acao});\n      }}\n      this.{campo_inst}.{campo} = {na_acao} /* REF:{url}:{ini}:{fim} */;{mudou}\n      this._expr_{k} = currVal_{k};\n    }}"
                    ));
                    continue;
                }
                let k = self.proxima_ligacao;
                self.proxima_ligacao += 1;
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
                    // Só a dinâmica (`final currVal_k = …;`) pode quebrar.
                    let c = self.converter_quebravel(&l.valor, motivo)?;
                    if c.imutavel && c.texto.contains(crate::expr::QUEBRA) {
                        return Err(recusa(motivo, "chamada com argumento nomeado imutável"));
                    }
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

    /// Os ouvintes de um nó com diretivas: os eventos do template, com o
    /// `@HostListener` do mesmo evento no handler deles (`mergeEvents`), e
    /// depois os `@HostListener` das diretivas (`_visitHostListeners`),
    /// agrupados por evento.
    fn ouvintes_com_hospedeiro(
        &mut self,
        do_no: &crate::html::Elemento,
        casadas: &[std::sync::Arc<crate::diretivas::Diretiva>],
        r: &crate::diretivas::NoResolvido,
        alvo: &str,
    ) -> Result<(), Recusa> {
        // Os `@HostListener` na ordem das diretivas em `directives:`
        // (`_collectHostListeners`), agrupados por evento na ordem em
        // que cada um aparece primeiro: dois ouvintes do mesmo evento
        // viram um `_handleEvent_N` que chama os dois.
        let mut grupos: Vec<(String, Vec<(String, crate::diretivas::Ouvinte)>)> = Vec::new();
        for d in casadas {
            let Some((_, campo)) = r
                .diretivas
                .iter()
                .find(|(x, _)| std::sync::Arc::ptr_eq(x, d))
            else {
                continue;
            };
            for o in &d.ouvintes {
                match grupos.iter_mut().find(|(ev, _)| *ev == o.evento) {
                    Some((_, l)) => l.push((campo.clone(), o.clone())),
                    None => grupos.push((o.evento.clone(), vec![(campo.clone(), o.clone())])),
                }
            }
        }
        // Os eventos do template vêm antes, e o `@HostListener` do mesmo
        // evento entra no handler dele, depois da ação escrita
        // (`mergeEvents` sobre as saídas do elemento, caso j70).
        let mut do_template: Vec<&str> = Vec::new();
        for l in &do_no.eventos {
            if do_template.contains(&l.nome.as_str()) {
                return Err(recusa(
                    Motivo::Evento,
                    "dois handlers do mesmo evento no template",
                ));
            }
            do_template.push(&l.nome);
        }
        for l in &do_no.eventos {
            let extras: Vec<String> = grupos
                .iter()
                .filter(|(ev, _)| *ev == l.nome)
                .flat_map(|(_, lista)| lista)
                .map(|(campo, o)| format!("this.{campo}.{}({})", o.metodo, o.args))
                .collect();
            match self.handler_com(&[&l.valor], &extras) {
                Ok(h) => self.ouvinte(&l.nome, alvo, &h),
                Err(r) => self.anotar(r)?,
            }
        }
        for (evento, lista) in &grupos {
            if do_template.contains(&evento.as_str()) {
                continue;
            }
            let h = match lista.as_slice() {
                [(campo, o)] => self.handler_de_hospedeiro(campo, o),
                varios => self.handler_de_grupo(varios),
            };
            self.ouvinte(evento, alvo, &h);
        }
        Ok(())
    }

    /// Os eventos escritos no elemento do filho que não casam um `@Output`
    /// dele: eventos do elemento (`bindRenderOutputs`), antes das saídas
    /// das diretivas do nó ([`Self::saidas_do_filho`]).
    fn eventos_do_elemento_do_filho(
        &mut self,
        e: &crate::html::Elemento,
        filho: &Filho,
        el: &str,
        diretivas: Option<(
            &[std::sync::Arc<crate::diretivas::Diretiva>],
            &crate::diretivas::NoResolvido,
        )>,
    ) -> Result<(), Recusa> {
        let mut do_elemento = e.clone();
        do_elemento
            .eventos
            .retain(|l| filho.saida(&l.nome).is_none());
        // Com diretivas no nó, os `@HostListener` delas (caso j93).
        match diretivas {
            Some((casadas, r)) if casadas.iter().any(|d| !d.ouvintes.is_empty()) => {
                self.ouvintes_com_hospedeiro(&do_elemento, casadas, r, el)
            }
            _ => self.eventos(&do_elemento, el),
        }
    }

    /// As `@Output` do filho ligadas no template: `subscription_N`
    /// (`bindDirectiveOutputs`), na vez do filho entre as diretivas do nó.
    fn saidas_do_filho(
        &mut self,
        e: &crate::html::Elemento,
        filho: &Filho,
        campo_inst: &str,
    ) -> Result<(), Recusa> {
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
                "`*` de diretiva que não é ngIf/ngFor/ngSwitch",
            ));
        };
        // `@Host()`: o provedor num elemento acima, nesta visão ou numa
        // ancestral (a leitura já vem pela cadeia de `parentView`). Com
        // componente no caminho, o injetor pode ser outro: recusa.
        let hospedeiro = match dir.hospedeiro {
            None => None,
            Some(classe) => {
                let token = crate::diretivas::Token::Classe {
                    uri: dir.uri.to_string(),
                    classe: classe.to_string(),
                };
                let achado = self
                    .provedores_acima()
                    .into_iter()
                    .find(|p| p.token == token)
                    .map(|p| p.leitura);
                match achado {
                    Some(l) if self.componentes_acima == 0 => Some(l),
                    _ => {
                        return Err(recusa(
                            Motivo::DiretivaPorSeletor,
                            format!("{} sem o {classe} acima", dir.classe),
                        ));
                    }
                }
            }
        };
        let mut micro = e.micro_da_estrela().unwrap_or_default();
        // As ligações saem na ordem das entradas da diretiva, não na escrita
        // (`_orderingOf(directive.inputs)`). Entrada que a diretiva não
        // declara é erro no oficial ("Can't bind to ..."): não há saída.
        if let Some((p, _)) = micro
            .propriedades
            .iter()
            .find(|(p, _)| !dir.entradas.contains(&p.as_str()))
        {
            return Err(recusa(
                Motivo::Ligacao,
                format!("`{p}` não é entrada de {}", dir.classe),
            ));
        }
        micro
            .propriedades
            .sort_by_key(|(p, _)| dir.entradas.iter().position(|x| x == p));
        self.guarda_do_template(&estrela.nome, &micro, Some(&dir))?;
        // O `REF` de cada entrada: o intervalo da ligação escrita num
        // `<template dir [dirX]>` ([`molde_com_diretiva`]); no `*`, o do
        // atributo inteiro.
        let intervalo = |prop: &str| {
            e.ligacoes_do_molde
                .iter()
                .find(|l| l.nome == prop)
                .map_or((estrela.inicio, estrela.fim), |l| (l.inicio, l.fim))
        };
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
            "    var _TemplateRef_{n}_8 = {tr}TemplateRef(this._appEl_{n}, {});",
            self.fabrica_do_molde(&nome_fabrica)
        ));
        let extra = hospedeiro.map(|h| format!(", {h}")).unwrap_or_default();
        let molde = if dir.com_template {
            format!(", _TemplateRef_{n}_8")
        } else {
            String::new()
        };
        self.linhas.push(format!(
            "    this.{campo} = {qd}{classe_dir}(this._appEl_{n}{molde}{extra});"
        ));
        self.linhas.push(format!(
            "    if ({dev}.isDevToolsEnabled) {{\n      {dev}.Inspector.instance.registerDirective(_anchor_{n}, this.{campo});\n    }}"
        ));

        // As entradas da diretiva, na ordem em que a microssintaxe as declara
        // (é a ordem dos índices de ligação). O `bindAndWriteToRenderer`
        // escreve as imutáveis (`if (firstCheck)`) antes das outras.
        let mut tipo_da_colecao = None;
        let mut dinamicas = Vec::new();
        for (prop, expr) in &micro.propriedades {
            let c = self.converter(expr, Motivo::Ligacao)?;
            if prop.ends_with("Of") {
                tipo_da_colecao = c.tipo.clone().map(|t| (t, c.escopo.clone()));
                // `_typeNgForLocals` tipa a coleção com o `analyzedClass`,
                // cujos locais são só os que têm tipo: um local sem tipo
                // (`let-x` de `<template>`, item de coleção `dynamic`) não
                // entra, e o nome cai no membro do componente (caso j83).
                // Sem membro de mesmo nome, o `_lookupGetterReturnType` do
                // receptor implícito dá `dynamic`, o mesmo que o local sem
                // tipo: só sai do escopo o local que um membro substitui (e
                // os argumentos de uma chamada não pesam no tipo dela).
                let e_membro = |nome: &str| {
                    self.membros.contains_key(nome)
                        || self.metodos.contains_key(nome)
                        || self.tipos.is_some_and(|(r, arquivo)| {
                            let classe = self.classe_qualificada.rsplit('.').next().unwrap_or("");
                            r.tipo_do_membro(arquivo, classe, nome).is_some()
                                || r.metodo(arquivo, classe, nome).is_some()
                        })
                };
                let sem_tipo: Vec<String> = self
                    .locais
                    .iter()
                    .filter(|(nome, l)| {
                        l.tipo == "dynamic" && cita_na_raiz(expr, nome) && e_membro(nome)
                    })
                    .map(|(nome, _)| nome.clone())
                    .collect();
                if !sem_tipo.is_empty() {
                    let salvos = self.locais.clone();
                    for nome in &sem_tipo {
                        self.locais.remove(nome);
                    }
                    let so_tipo = self.converter(expr, Motivo::Ligacao);
                    self.locais = salvos;
                    tipo_da_colecao = so_tipo
                        .ok()
                        .and_then(|x| x.tipo.map(|t| (t, x.escopo)))
                        .or_else(|| Some(("dynamic".to_string(), None)));
                }
            }
            // Entrada imutável (`final List<X> itens`, `*ngIf="fixo"`) é
            // escrita uma vez, no `if (firstCheck)` das entradas: direto no
            // `NgIf` (`_directBinding` no método das constantes); pelo
            // `_bindLiteral` no `NgFor`, que consome o índice da ligação e
            // põe o `if (x != null)` em volta do que pode ser nulo.
            if c.imutavel {
                let (ini, fim) = intervalo(prop);
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
            let (ini, fim) = intervalo(prop);
            if dir.direta {
                // `_isDirectBinding` do ngcompiler: o `NgIf` já compara o
                // valor antes de agir, então não há `checkBinding` fora.
                dinamicas.push(format!(
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
                dinamicas.push(format!(
                    "    final currVal_{k} = {valor};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, '{texto}', '{url}')) {{\n      if ({dev}.isDevToolsEnabled) {{\n        {dev}.Inspector.instance.recordInput(this.{campo}, '{prop}', currVal_{k});\n      }}\n      this.{campo}.{prop} = currVal_{k} /* REF:{url}:{ini}:{fim} */;\n      this._expr_{k} = currVal_{k};\n    }}"
                ));
            }
        }
        self.entradas.extend(dinamicas);
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
        self.empilhar_embutida(
            e,
            vec![No::Elemento(sem_estrela)],
            n,
            indice,
            nome_fabrica,
            pai,
            locais,
            micro,
        );
        Ok(())
    }

    /// `<template>` escrito à mão: âncora, `ViewContainer` e `TemplateRef`
    /// (o 7 da tabela do nó), com o conteúdo numa visão embutida cujos locais
    /// são os `let-x` (chave `$implicit` sem valor). Com `#ref` o
    /// `TemplateRef` é campo e o nome o lê; sem, um local. Ninguém cria a
    /// visão aqui: o `ViewContainer` fica fora da detecção e da destruição.
    ///
    /// Uma diretiva de `<template>` (`template[x]`, que recebe o
    /// `TemplateRef` no construtor: o `LiSelectTriggerDirective` do
    /// limitless_ui) é o provedor 8, criada logo depois, registrada no
    /// devtools e achável pelas consultas de conteúdo do filho que recebe o
    /// `<template>` projetado (caso j28). Sem diretiva que peça
    /// `ViewContainerRef`, o nó não tem `hasViewContainer`: o que vai para a
    /// raiz e para a projeção é a âncora (`_addRootNodeAndProject`).
    fn molde(&mut self, e: &crate::html::Elemento, pai: &str) -> Result<(), Recusa> {
        let mut locais_do_molde = Vec::new();
        let mut atributos = Vec::new();
        for a in &e.atributos {
            match a.nome.strip_prefix("let-") {
                Some(nome) => locais_do_molde.push((
                    nome.to_string(),
                    if a.valor.is_empty() {
                        "$implicit".to_string()
                    } else {
                        a.valor.clone()
                    },
                )),
                None => atributos.push(a),
            }
        }
        let mut sem = e.clone();
        sem.estrela = None;
        let desc = crate::seletor::Elemento::do_template(&sem);
        if let Some(u) = self.usadas.iter().find(|u| {
            (u.filho.is_some() || u.diretiva.is_none())
                && crate::seletor::casa_algum(&u.seletores, &desc)
        }) {
            return Err(recusa(
                Motivo::DiretivaPorSeletor,
                format!("diretiva {} em <template> sem metadados", u.classe),
            ));
        }
        // As diretivas do `<template>` na ordem de `directives:`
        // (`_matchTemplateDirectives`), resolvidas como as de um elemento:
        // o `TemplateRef` e o `ViewContainerRef` do nó, dependências de cima,
        // entradas, saídas e ganchos (caso j85). O `@HostBinding` de uma
        // delas não se escreve no `<template>` (`visitEmbeddedTemplate` não
        // chama `bindDirectiveHostProps`).
        let casadas = diretivas_casadas(self.usadas, &sem);
        if casadas.is_empty() && !atributos.is_empty() {
            return Err(recusa(
                Motivo::Ligacao,
                "atributo em <template> sem diretiva",
            ));
        }
        // `[x]` e `(x)` no `<template>` só existem como entrada ou saída de
        // uma diretiva dele (o resto é erro no oficial).
        if let Some(l) = sem
            .propriedades
            .iter()
            .find(|l| !consome_entrada(&casadas, &l.nome))
            .or_else(|| {
                sem.eventos
                    .iter()
                    .find(|l| !consome_saida(&casadas, &l.nome))
            })
        {
            return Err(recusa(
                Motivo::Ligacao,
                format!("[{}] em <template> que nenhuma diretiva recebe", l.nome),
            ));
        }
        for d in &casadas {
            if !d.ouvintes.is_empty() || d.consultas || !d.consultas_de_conteudo.is_empty() {
                return Err(recusa(
                    Motivo::DiretivaPorSeletor,
                    format!(
                        "diretiva {} em <template> com @HostListener ou consulta",
                        d.classe
                    ),
                ));
            }
        }
        // `@ViewChild` com o resultado no próprio `<template>`: o valor
        // seria o `TemplateRef` dele. Dentro, a consulta mapeia a visão
        // embutida ([`arvore_da_consulta`]).
        if self
            .consultas_dinamicas
            .iter()
            .any(|q| casa_a_chave(e, &q.chave, self.filhos))
        {
            return Err(recusa(Motivo::Ligacao, "@ViewChild em <template>"));
        }
        let n = self.proximo;
        self.proximo += 1;
        let dom = self.dom();
        let vc = self.imp.q(VIEW_CONTAINER);
        let tr = self.imp.q(TEMPLATE_REF);
        let indice = self.proxima_embutida;
        self.proxima_embutida += 1 + contar_estruturais(&e.filhos);
        let nome_fabrica = format!("viewFactory_{}{indice}", &self.classe_da_visao[4..]);
        self.campos_filho
            .push(format!("  late final {vc}ViewContainer _appEl_{n};"));
        let referencia = e.referencias.first().map(|r| r.nome.clone());
        // O `TemplateRef` é campo quando lido fora do `build()` (`#ref`); a
        // leitura sai pelo índice que o resolvedor dá a ele.
        let com_ref = referencia.is_some();
        let leitura_do_tr = |k: u32| {
            if com_ref {
                format!("this._TemplateRef_{n}_{k}")
            } else {
                format!("_TemplateRef_{n}_{k}")
            }
        };
        // Uma consulta lê o `#ref` como `ViewContainerRef`: o `ViewContainer`
        // não é privado e o `TemplateRef` vai para o índice 8.
        let forcar_container = referencia
            .as_ref()
            .is_some_and(|r| self.moldes_com_container.contains(r));
        let (resolvido, k_tr) = if casadas.is_empty() {
            (None, if forcar_container { 8 } else { 7 })
        } else {
            let provedores = self.provedores_acima_com(&tardio(UTILITIES));
            let acima = crate::diretivas::Acima {
                provedores: &provedores,
                incerto: self.incertos_acima > 0,
                de_visao: self.tokens_de_visao,
            };
            let (r, k) = crate::diretivas::resolver_de_molde(
                &casadas,
                n,
                Some(acima),
                forcar_container,
                &leitura_do_tr,
            )
            .map_err(|f| recusa(Motivo::DiretivaPorSeletor, f))?;
            (Some(r), k)
        };
        if com_ref {
            self.campos_filho.push(format!(
                "  late final {tr}TemplateRef _TemplateRef_{n}_{k_tr};"
            ));
        }
        let pai_indice = if pai.is_empty() {
            // Com `ViewContainer` não privado, a raiz é ele (`vcAppEl ??
            // renderNode`, caso j85).
            let container = forcar_container || resolvido.as_ref().is_some_and(|r| r.container);
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
                "    final _anchor_{n} = {dom}.appendAnchor({pai});"
            ));
            indice_do_elemento(pai)
        };
        self.linhas.push(format!(
            "    this._appEl_{n} = {vc}ViewContainer({n}, {pai_indice}, this, _anchor_{n});"
        ));
        match &referencia {
            Some(nome) => {
                self.linhas.push(format!(
                    "    this._TemplateRef_{n}_{k_tr} = {tr}TemplateRef(this._appEl_{n}, {});",
                    self.fabrica_do_molde(&nome_fabrica)
                ));
                let leitura = format!("this._TemplateRef_{n}_{k_tr}");
                self.refs.insert(nome.clone(), leitura.clone());
                self.refs_em_ordem.push((nome.clone(), leitura.clone()));
                if forcar_container {
                    let vc = format!("this._appEl_{n}");
                    self.refs.insert(chave_de_container(nome), vc.clone());
                    self.refs_em_ordem.push((chave_de_container(nome), vc));
                }
            }
            None => {
                self.linhas.push(format!(
                    "    var _TemplateRef_{n}_{k_tr} = {tr}TemplateRef(this._appEl_{n}, {});",
                    self.fabrica_do_molde(&nome_fabrica)
                ));
            }
        }
        // As diretivas: criação, `registerDirective` na âncora, entradas e
        // saídas (sem `@HostBinding`: nenhuma instância para o
        // `detectHostChanges`) e os ganchos, que no `<template>` saem logo
        // (`bindDirectiveAfterChildrenCallbacks` no `visitEmbeddedTemplate`).
        let antes_acima = self.acima.len();
        // Com `ViewContainerRef` pedido, o `ViewContainer` não é privado
        // (`createViewContainer(.., !hasViewContainer, ..)`): as visões
        // dele são detectadas e destruídas por esta (caso j85).
        if forcar_container || resolvido.as_ref().is_some_and(|r| r.container) {
            self.ancoras.push(format!("_appEl_{n}"));
        }
        if let Some(r) = &resolvido {
            let alvo = format!("_anchor_{n}");
            self.criar_instancias(&sem, &r.instancias, &alvo, "this")?;
            self.registrar_diretivas(&r.diretivas, &alvo);
            self.ligar_diretivas(
                &sem,
                &[],
                &r.diretivas,
                &alvo,
                &sem.propriedades,
                &sem.eventos,
                true,
                true,
            )?;
            self.ganchos_depois_dos_filhos(&r.diretivas);
            let injetaveis: Vec<(Vec<crate::diretivas::Token>, String)> = r
                .instancias
                .iter()
                .filter(|i| !i.injetavel_por.is_empty())
                .map(|i| (i.injetavel_por.clone(), i.leitura.clone()))
                .collect();
            if !injetaveis.is_empty() {
                self.injetores.push((n, n, injetaveis));
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
                elemento: alvo,
            });
            // Os provedores do `<template>` ficam acima do conteúdo dele.
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
        // Quem cria a visão (a diretiva do filho, com `ngTemplateOutlet`)
        // é que dá o contexto: o local não tem tipo (`dynamic`, sem cast).
        let mut locais = self.locais.clone();
        for (nome, _) in &locais_do_molde {
            locais.insert(
                nome.clone(),
                crate::expr::Local {
                    dart: format!("local_{nome}"),
                    tipo: "dynamic".to_string(),
                    escopo: None,
                },
            );
        }
        self.empilhar_embutida(
            e,
            e.filhos.clone(),
            n,
            indice,
            nome_fabrica,
            pai,
            locais,
            crate::micro::Micro {
                propriedades: Vec::new(),
                locais: locais_do_molde,
            },
        );
        self.acima.truncate(antes_acima);
        Ok(())
    }

    /// O segundo argumento do `TemplateRef`: a fábrica da embutida. Num
    /// componente genérico o oficial a embrulha num fecho que passa os
    /// argumentos de tipo (`viewFactory_X1<T>(parentView, parentIndex)`).
    fn fabrica_do_molde(&self, nome: &str) -> String {
        if self.genericos.is_empty() {
            return nome.to_string();
        }
        format!(
            "(parentView, parentIndex) {{\n      return {nome}{}(parentView, parentIndex);\n    }}",
            self.genericos
        )
    }

    /// Guarda a especificação da visão embutida de um `*` ou `<template>`
    /// (`e`, na âncora `n`), com o que ela herda desta: os locais, os
    /// `#ref` e os provedores acima, um `parentView` mais longe.
    #[allow(clippy::too_many_arguments)]
    fn empilhar_embutida(
        &mut self,
        e: &crate::html::Elemento,
        nos: Vec<No>,
        n: u32,
        indice: u32,
        nome_fabrica: String,
        pai: &str,
        locais: std::collections::HashMap<String, crate::expr::Local>,
        micro: crate::micro::Micro,
    ) {
        // O oficial liga a embutida quando o binder chega ao molde, depois
        // das entradas dele: o nó desta visão que ela lê é promovido aí,
        // entre os `_expr_k` já criados e os seguintes.
        let pendentes: Vec<String> = self
            .refs_de_embutidas
            .iter()
            .filter(|nome| !self.posicao_de_embutida.contains_key(*nome))
            .cloned()
            .collect();
        for nome in pendentes {
            if citado_na_embutida(e, &nome, self.filhos) {
                self.posicao_de_embutida
                    .insert(nome, self.campos_expr.len());
            }
        }
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
        // Consulta de visão com o resultado aqui dentro: a âncora e a classe
        // entram na cadeia dela; se o resultado está na visão nova, ela marca
        // o campo sujo no `dirtyParentQueriesInternal`; se está mais abaixo,
        // a consulta segue em trânsito para as aninhadas.
        let classe_nova = format!("_{}{indice}", self.classe_da_visao);
        let mut pendentes: Vec<(String, String, u32)> = Vec::new();
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
            pendentes.push((q.chave.clone(), q.campo.clone(), 1));
        }
        for (r, campo, niveis) in &self.consultas_em_transito {
            let mut l = Vec::new();
            onde_esta(
                std::slice::from_ref(&No::Elemento(e.clone())),
                r,
                self.filhos,
                false,
                &mut l,
            );
            if !l.is_empty() {
                pendentes.push((r.clone(), campo.clone(), niveis + 1));
            }
        }
        // A âncora do `*` entra no mapa pelo início da estrela; na visão
        // nova, a consulta com resultado nela marca o campo sujo, e a com
        // resultado mais abaixo segue em trânsito (as duas coisas podem
        // valer ao mesmo tempo).
        let estrela = e.estrela.as_ref().map(|l| l.inicio);
        // Toda âncora entra no mapa: as consultas de conteúdo também leem a
        // cadeia por ele ([`resolver_conteudo_dinamico`]).
        if let Some(k) = estrela {
            self.ancoras_de_consulta
                .borrow_mut()
                .insert(k, (format!("_appEl_{n}"), classe_nova.clone()));
        }
        let mut refs_consultados = Vec::new();
        let mut consultas_em_transito = Vec::new();
        for (r, campo, niveis) in pendentes {
            if let Some(k) = estrela {
                self.ancoras_de_consulta
                    .borrow_mut()
                    .insert(k, (format!("_appEl_{n}"), classe_nova.clone()));
            }
            let mut l = Vec::new();
            onde_esta(&nos, &r, self.filhos, false, &mut l);
            if l.iter()
                .any(|x| matches!(x, Lugar::Raiz | Lugar::NoFilho | Lugar::NoFilhoProjetado))
            {
                refs_consultados.push((r.clone(), campo.clone(), niveis));
            }
            if l.contains(&Lugar::Embutida) {
                consultas_em_transito.push((r, campo, niveis));
            }
        }
        // O `dirtyParentQueriesInternal` marca cada consulta quando o
        // primeiro resultado dela na visão nova é registrado
        // (`_setParentQueryAsDirty`, no `addQueryResult` de cada elemento, em
        // pré-ordem): a ordem é a dos resultados, não a das consultas.
        let mut sujas = refs_consultados.clone();
        sujas.sort_by_key(|(r, _, _)| primeiro_resultado(&nos, r, self.filhos));
        // O índice do `<ng-content>` é o ordinal dele no template inteiro
        // (`ngContentSelectors`, em pré-ordem): a embutida continua a
        // contagem daqui, e esta visão pula os que ficam lá dentro.
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
            fabrica: nome_fabrica,
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
                        Some((classe, n)) => (classe.clone(), n + 1),
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
                // Coleção `dynamic` (índice, `??`, ternário, campo
                // `dynamic`): o `getIterableElementType` não acha tipo, e o
                // local fica sem cast (`this.locals['\$implicit']`).
                "$implicit" if tipo_da_colecao.is_some_and(|(t, _)| t == "dynamic") => {
                    ("dynamic".to_string(), None)
                }
                "$implicit" => {
                    // `List`/`Iterable`/`Set` pelo texto; senão o retorno do
                    // getter `single` no tipo da coleção
                    // (`getIterableElementType`, `analyzed_class.dart:39-42`).
                    let pelo_single = |t: &str, e: &Option<std::path::PathBuf>| {
                        let (r, arquivo) = self.tipos?;
                        let (x, escopo) = r.tipo_do_membro_livre(
                            e.as_deref().unwrap_or(arquivo),
                            t,
                            "single",
                            self.nomes_genericos,
                        )?;
                        Some((x, Some(escopo)))
                    };
                    let Some((t, escopo)) = tipo_da_colecao.and_then(|(t, e)| {
                        tipo_do_elemento(t)
                            .map(|x| (x, e.clone()))
                            .or_else(|| pelo_single(t, e))
                    }) else {
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
            // No elemento de um componente (`<li-select [(ngModel)]>`), sem
            // `@HostListener`: os ouvintes de diretiva ali ainda não saem.
            let pendencia = match &u.diretiva {
                None => Some("sem metadados".to_string()),
                Some(d) => d.pendencia(),
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
        // Sem pai: raiz de uma visão embutida (o `*` num `<ng-container>`)
        // ou conteúdo projetado — o nó é o próprio `TextBinding.element`, ou
        // o `createText(valor)` quando imutável (`createTextBinding` sem
        // `parentNode`), e entra nas raízes ou na lista da projeção.
        let solto = pai.is_empty();
        let convertida = self.converter_quebravel(expr, Motivo::Interpolacao)?;
        let quebra = convertida.texto.contains(crate::expr::QUEBRA);
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
            // No `build()` o import de um nome de `exports:` entra agora,
            // depois do `interpolate` (caso j51).
            let valor = resolver_tardios(self.imp, &valor);
            if cita_ctx(std::slice::from_ref(&valor)) {
                self.usa_ctx_no_build = true;
            }
            if solto {
                self.linhas
                    .push(format!("    final _text_{n} = {dom}.createText({valor});"));
                self.raizes.push(format!("_text_{n}"));
            } else {
                self.linhas.push(format!(
                    "    final _text_{n} = {dom}.appendText({pai}, {valor});"
                ));
            }
            return Ok(());
        }
        let Some(tb) = self.tb.clone() else {
            return Err(recusa(
                Motivo::Interpolacao,
                "interpolação sem ligação de texto",
            ));
        };
        self.tb_usado = true;
        self.campos.push(format!(
            "  final {tb}.TextBinding _textBinding_{n} = {tb}.TextBinding();"
        ));
        if solto {
            self.raizes.push(format!("this._textBinding_{n}.element"));
        } else {
            self.linhas
                .push(format!("    {pai}.append(this._textBinding_{n}.element);"));
        }
        let acesso = convertida.texto;
        // A chamada que quebra dentro de `interpolateString0(..)` ganha a
        // indentação de continuação do formatador (+4, caso j21); dentro de
        // `updateTextWithPrimitive(..)`, sem caso.
        if quebra && (primitivo_mutavel || convertida.imutavel) {
            return Err(recusa(
                Motivo::Interpolacao,
                "chamada com argumento nomeado em interpolação primitiva",
            ));
        }
        let acesso = crate::expr::recuar(&acesso, 4);
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

    /// O valor de um texto com `{{ }}` (`title="a {{b}}"`, `x="{{y}}"`), como
    /// o `_createPropertyForAttribute`/`convertInterpolation` o escrevem:
    /// `interpolateStringN` quando tudo é `String`, `interpolateN` senão; o
    /// literal puro é o próprio texto; imutável, calculado uma vez. A
    /// expressão primitiva mutável sozinha (`_maybeOptimizeInterpolation`) é
    /// conferida crua e interpolada só onde o valor é usado (`na_acao`).
    /// Aloca o número da ligação (`k`).
    fn valor_interpolado(&mut self, texto: &str, motivo: Motivo) -> Result<Interpolada, Recusa> {
        if texto.contains('&') {
            return Err(recusa(motivo, "atributo interpolado com entidade HTML"));
        }
        let (textos, exprs) = partes_da_interpolacao(texto)
            .ok_or_else(|| recusa(motivo, "atributo interpolado mal formado"))?;
        let mut convertidas = Vec::new();
        for e in &exprs {
            convertidas.push(self.converter(e, motivo)?);
        }
        // O tipo só escolhe a família com 1 ou 2 expressões; com 3 ou mais é
        // sempre o `interpolateN` (`interpolateFallback`).
        let mut tipos = Vec::new();
        for c in &convertidas {
            match &c.tipo {
                Some(t) => tipos.push(t.trim_end_matches('?').to_string()),
                None if convertidas.len() > 2 => tipos.push("dynamic".into()),
                None => {
                    return Err(recusa(motivo, format!("tipo desconhecido de {}", c.forma)));
                }
            }
        }
        // `_compressWhitespacePreceding`/`Following`: só as pontas, e só
        // quando há quebra de linha.
        let n_textos = textos.len();
        let textos: Vec<String> = textos
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let t = if self.preservar_espacos {
                    t.replace(NGSP, " ")
                } else if i == 0 {
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
                // `interpolateFallback` (`expression_converter.dart:219-227`):
                // os textos e as expressões intercalados numa lista.
                _ => {
                    let mut itens = Vec::new();
                    for (t, e) in textos.iter().zip(exprs) {
                        itens.push(t.clone());
                        itens.push(e.clone());
                    }
                    itens.extend(textos.last().cloned());
                    format!("{interp}.interpolateN([{}])", itens.join(", "))
                }
            }
        };
        let textos_expr: Vec<String> = convertidas.iter().map(|c| c.texto.clone()).collect();
        // Uma expressão literal com as pontas vazias é o próprio texto.
        let literal_puro = convertidas.len() == 1
            && convertidas[0].literal
            && textos[0] == "''"
            && textos[1] == "''";
        let constante = if literal_puro {
            valor_de_literal(&convertidas[0].texto)
        } else {
            valor_com(&textos_expr, familia)
        };
        let primitiva = convertidas.len() == 1 && !convertidas[0].imutavel && primitivo(&tipos[0]);
        let (checagem, na_acao) = if primitiva {
            (
                convertidas[0].texto.clone(),
                valor_com(&[format!("currVal_{k}")], "interpolate"),
            )
        } else {
            (valor_com(&textos_expr, familia), format!("currVal_{k}"))
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
        // `_createPropertyForAttribute` passa o nome ao mesmo
        // `createElementPropertyAst` de `[x]`: `attr.x`, `class.x` e
        // `style.x` são as ligações de sempre (caso j107). Sem prefixo, o
        // nome que o esquema renomeia (`readonly`, `tabindex`, `for`…) ainda
        // não sai; o que nem é propriedade do DOM (`data-x`, `aria-x`) é
        // erro no oficial.
        let com_prefixo = ["attr.", "class.", "style."]
            .iter()
            .any(|p| nome.starts_with(p));
        if nome != "class"
            && !com_prefixo
            && (!nome.chars().all(|c| c.is_ascii_lowercase())
                // `for` e `formaction` não são propriedades no esquema (o
                // `_attrToPropMap` não os mapeia): erro no oficial.
                || matches!(nome, "for" | "formaction"))
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
        let v = self.valor_interpolado(&a.valor, Motivo::Interpolacao)?;
        // O valor da ligação é a `Interpolation`, não a expressão de dentro:
        // nunca é nula (`canBeNull` dá `false`, daí o `setAttribute` do
        // `visitAttributeBinding`) e o `_TypeResolver` a tipa `String`. No
        // atalho primitivo (`_shouldInterpolateAfterCheck`) a checagem é a
        // expressão crua, e o tipo que vale é o dela (`[style.x]` com um
        // `int` ganha o `.toString()`, caso j107; `calc({{a - b}}%)` não,
        // o `material_slider`).
        let primitiva = v.convertidas.len() == 1 && v.checagem == v.convertidas[0].texto;
        let mut convertidas = v.convertidas;
        if let Some(c) = convertidas.first_mut() {
            c.pode_ser_nulo = false;
            if !primitiva {
                c.tipo = Some("String".into());
            }
            c.escopo = None;
        }
        let k = v.k;
        let simulada = crate::html::Ligacao {
            nome: a.nome.clone(),
            valor: a.valor.clone(),
            inicio: a.inicio,
            fim: a.fim,
            sem_valor: a.sem_valor,
        };
        let (ini, fim) = (a.inicio, a.fim);
        if v.imutavel {
            let acao = self.acao(&simulada, alvo, &v.constante, &convertidas[0])?;
            return Ok(Ligada::Constante(format!(
                "      {acao} /* REF:{url}:{ini}:{fim} */;"
            )));
        }
        let (checagem, na_acao) = (v.checagem, v.na_acao);
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
            self.locais_proprios.clone(),
        );
        // A ligação de texto é alocada por visão; aqui a saída é descartada.
        self.tb.get_or_insert_with(|| "_coleta".into());
        if let Some(micro) = e.micro_da_estrela() {
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
            // Uma visão embutida emitida mais abaixo herda estes locais como
            // ancestrais (a coleta só junta recusas: a origem não importa).
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
        ) = guardados;
    }

    /// Um nó do template.
    fn no(&mut self, no: &No, pai: &str) -> Result<(), Recusa> {
        match no {
            No::Comentario(_) => {}
            No::Texto(t) => {
                // Sem pai: raiz de visão embutida ou conteúdo projetado
                // (`createText` do `dom_helpers`, e o nó entra nas raízes ou
                // na lista da projeção).
                let n = self.proximo;
                self.proximo += 1;
                let dom = self.dom();
                let texto = literal(t);
                if pai.is_empty() {
                    self.linhas
                        .push(format!("    final _text_{n} = {dom}.createText({texto});"));
                    self.raizes.push(format!("_text_{n}"));
                } else {
                    self.linhas.push(format!(
                        "    final _text_{n} = {dom}.appendText({pai}, {texto});"
                    ));
                }
            }
            No::Elemento(e) => {
                // `<template>` escrito à mão é uma visão embutida, não um
                // elemento HTML (`EmbeddedTemplateAst`).
                // (O que [`template_como_container`] não reescreveu nem marcou
                // como [`MARCA_DE_MOLDE`] ainda não é traduzido.)
                if e.nome.eq_ignore_ascii_case("template")
                    && e.estrela.as_ref().is_none_or(|l| l.nome != MARCA_DE_MOLDE)
                {
                    return Err(recusa(Motivo::Ligacao, "<template> escrito no template"));
                }
                // `<ng-container>` sem `*` (`visitNgContainer`): os filhos
                // vão direto para o pai, sem nó nem índice.
                if e.nome == "ng-container" && e.estrela.is_none() {
                    return self.container(e, pai);
                }
                if e.estrela.is_none() {
                    self.guarda_do_elemento(e)?;
                }
                // `@i18n` só em elemento HTML: no filho ele mexe no conteúdo
                // projetado. No `*` a anotação fica no elemento, que vai para
                // a visão embutida (caso j01).
                if !e.anotacoes.is_empty() && self.filhos.contains_key(&e.nome) {
                    return Err(recusa(Motivo::I18n, "@i18n em componente filho"));
                }
                // `*` num filho: o filho vai para a visão embutida, como
                // qualquer elemento.
                if let (Some(estrela), true) = (&e.estrela, self.filhos.contains_key(&e.nome)) {
                    return self.estrutural(e, estrela, pai);
                }
                if let Some(filho) = self.filhos.get(&e.nome).cloned() {
                    return self.componente_filho(e, &filho, pai);
                }
                // Tag que não é HTML só com diretiva (`<pg-breadcrumb-item>`): é
                // um elemento tipado `Element` (`identifierFromTagName`).
                if !dom::tag_html(&e.nome)
                    && e.nome != "ng-container"
                    && self.namespace_de(e).is_none()
                    && diretivas_casadas(self.usadas, e).is_empty()
                {
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
                    if estrela.nome == MARCA_DE_MOLDE {
                        return self.molde(e, pai);
                    }
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
                // Sem pai (raiz de visão embutida, ou conteúdo que vai para
                // outro filho): os nós projetados entram inteiros na lista
                // de raízes (`ProjectedNodes`, uma lista), caso j56.
                if pai.is_empty() {
                    self.raizes
                        .push(format!("{RAIZ_LISTA}this.projectedNodes[{i}]"));
                } else {
                    self.linhas.push(format!("    this.project({pai}, {i});"));
                }
            }
        }
        Ok(())
    }

    /// `<ng-container>`: só agrupa. Os filhos são criados no pai dele (na
    /// visão embutida de um `*`, soltos: cada um é uma raiz). O ngast só
    /// aceita nele `*`, `#ref` e anotações; `#ref` e `@i18n` ainda não, nem
    /// o contêiner no conteúdo projetado num filho (o `ngContentIndex` dele
    /// é calculado pelo seletor vazio do contêiner).
    fn container(&mut self, e: &crate::html::Elemento, pai: &str) -> Result<(), Recusa> {
        let tem_algo = !(e.atributos.is_empty()
            && e.propriedades.is_empty()
            && e.eventos.is_empty()
            && e.bananas.is_empty()
            && e.referencias.is_empty()
            && e.anotacoes.is_empty());
        if tem_algo {
            return Err(recusa(
                Motivo::Ligacao,
                "<ng-container> com atributo, ligação, #ref ou anotação",
            ));
        }
        if pai.is_empty() && self.pai_projetado.is_some() {
            return Err(recusa(
                Motivo::Projecao,
                "<ng-container> no conteúdo projetado",
            ));
        }
        self.nos(&e.filhos, pai)
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
        // `#ref="x"`: a diretiva do nó com `exportAs: 'x'` (uma só; mais de
        // uma é erro no oficial). O local vale a instância dela
        // (`referenceTokens` em `compile_element.dart`).
        let exportada = |r: &crate::html::Ligacao| {
            let achadas: Vec<_> = casadas
                .iter()
                .filter(|d| d.export_as.as_deref() == Some(r.valor.as_str()))
                .collect();
            match achadas.as_slice() {
                [d] => Some(std::sync::Arc::clone(d)),
                _ => None,
            }
        };
        // `#ref` só na forma que não muda nada no nó; o valor dele é
        // registrado adiante, para o `@ViewChild`. Com valor e a diretiva
        // exportada no nó, o nome não lido por expressão também é só um nome
        // (para a instância dela).
        if let Some(r) = e.referencias.iter().find(|r| {
            (!r.valor.is_empty() && exportada(r).is_none())
                || !(self.refs_livres.contains(&r.nome)
                    || self.refs_locais.contains(&r.nome)
                    || self.refs_consultados.iter().any(|(n, _, _)| *n == r.nome)
                    || !r.valor.is_empty())
        }) {
            self.anotar(recusa(
                Motivo::Ligacao,
                if !r.valor.is_empty() {
                    "#ref com valor sem uma diretiva do nó que o exporte"
                } else if self.refs_ambiguos.contains(&r.nome) {
                    "#ref repetido ou sombreado por `let`"
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
            let provedores = self.provedores_acima();
            let acima = crate::diretivas::Acima {
                provedores: &provedores,
                incerto: self.incertos_acima > 0,
                de_visao: self.tokens_de_visao,
            };
            // O que as consultas com `read:` leem deste nó é ansioso.
            let consultados: Vec<crate::diretivas::Token> = self
                .leituras
                .iter()
                .filter(|(chave, _)| casa_a_chave(e, chave, self.filhos))
                .map(|(_, t)| t.clone())
                .collect();
            match crate::diretivas::resolver_consultado(&casadas, n, Some(acima), &consultados) {
                Ok(r) => Some(r),
                Err(f) => {
                    self.anotar(recusa(Motivo::DiretivaPorSeletor, f))?;
                    None
                }
            }
        };
        // O resultado de cada consulta com `read:` de provedor: a instância
        // do token no nó ([`chave_de_leitura`]).
        for (chave, t) in self.leituras {
            if !casa_a_chave(e, chave, self.filhos) {
                continue;
            }
            let instancia = resolvido.as_ref().and_then(|r| {
                r.instancias
                    .iter()
                    .find(|i| i.token == *t || i.apelidos.contains(t))
            });
            match instancia {
                Some(i) => {
                    let leitura = format!("this.{}", i.leitura);
                    let k = chave_de_leitura(chave, t);
                    self.refs.insert(k.clone(), leitura.clone());
                    self.refs_em_ordem.push((k, leitura));
                }
                None => self.anotar(recusa(
                    Motivo::ViewChildEmFilho,
                    "@ViewChild(.., read: T) de token que o elemento não provê",
                ))?,
            }
        }
        if !self.tem_doc {
            self.tem_doc = true;
            let html = self.html.clone();
            self.linhas
                .push(format!("    final doc = {html}.document;"));
        }
        let dom = self.dom();
        // Com namespace, o nome vira `@ns:tag` (`mergeNsAndName`): não é
        // HTML (`Element`, as variantes `NonHtml`) e sai com
        // `createElementNS` e o `append` à parte (`createElementNs`,
        // `_initializeAndAppendNode`).
        let namespace = self.namespace_de(e);
        if let Some((ns, _)) = &namespace {
            if !casadas.is_empty() || self.filhos.contains_key(&e.nome) {
                return Err(recusa(
                    Motivo::ComponenteNoTemplate,
                    "diretiva em elemento com namespace",
                ));
            }
            if e.atributos.iter().any(|a| a.nome.contains(':')) {
                return Err(recusa(
                    Motivo::Ligacao,
                    "atributo com namespace em elemento SVG",
                ));
            }
            if propriedades.iter().any(|l| {
                !["attr.", "class.", "style."]
                    .iter()
                    .any(|p| l.nome.starts_with(p))
            }) {
                return Err(recusa(
                    Motivo::Ligacao,
                    "[propriedade] em elemento com namespace",
                ));
            }
            let _ = ns;
        }
        let tag = match &namespace {
            Some((ns, nome)) => format!("@{ns}:{nome}"),
            None => e.nome.to_ascii_lowercase(),
        };
        let uri_do_ns = namespace.as_ref().map(|(ns, _)| match ns.as_str() {
            "xlink" => "'http://www.w3.org/1999/xlink'".to_string(),
            "svg" => "'http://www.w3.org/2000/svg'".to_string(),
            "xhtml" => "'http://www.w3.org/1999/xhtml'".to_string(),
            _ => "null".to_string(),
        });
        // Nó projetado não tem pai: o oficial cria solto, com
        // `document.createElement`, e entrega ao filho
        // (`_createElementAndAppend`, com `parent == null`).
        let criacao = if let (Some((_, nome)), Some(uri)) = (&namespace, &uri_do_ns) {
            format!("doc.createElementNS({uri}, '{nome}')")
        } else if pai.is_empty() {
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
        let tipo = if namespace.is_some() {
            "Element"
        } else {
            dom::tipo_da_tag(&tag)
        };
        // O `#ref` que só consultas com `read:` de provedor procuram não
        // lê o nó.
        let consulta_o_no = |nome: &String| {
            self.refs_consultados.iter().any(|(n, _, _)| n == nome)
                && !self.refs_so_por_provedor.contains(nome)
        };
        let lido_como_local = e.referencias.iter().any(|r| {
            r.valor.is_empty() && (self.refs_locais.contains(&r.nome) || consulta_o_no(&r.nome))
        });
        let lido_de_embutida = e
            .referencias
            .iter()
            .find(|r| r.valor.is_empty() && self.refs_de_embutidas.contains(&r.nome))
            .map(|r| r.nome.clone());
        let alvo = if !liga_no_elemento(e, &casadas) && !lido_como_local {
            self.linhas.push(format!("    final _el_{n} = {criacao};"));
            format!("_el_{n}")
        } else {
            let html = self.html.clone();
            let campo = format!("  late final {html}.{tipo} _el_{n};");
            let consultado = e
                .referencias
                .iter()
                .filter(|r| r.valor.is_empty() && !self.refs_so_por_provedor.contains(&r.nome))
                .filter_map(|r| {
                    self.refs_consultados
                        .iter()
                        .position(|(n, _, _)| *n == r.nome)
                })
                .min();
            let so_em_evento = (!liga_no_elemento(e, &casadas))
                .then(|| {
                    e.referencias
                        .iter()
                        .find(|r| r.valor.is_empty() && self.refs_so_em_eventos.contains(&r.nome))
                        .map(|r| r.nome.clone())
                })
                .flatten();
            match (lido_de_embutida, consultado, so_em_evento) {
                (Some(nome), _, _) => self.campos_el_de_embutidas.push((nome, campo)),
                (None, Some(k), _) => self.campos_el_consultados.push((k, campo)),
                (None, None, Some(nome)) => self.campos_el_de_eventos.push((nome, campo)),
                (None, None, None) => {
                    self.refs_dos_campos_el.insert(
                        campo.clone(),
                        e.referencias
                            .iter()
                            .filter(|r| r.valor.is_empty())
                            .map(|r| r.nome.clone())
                            .collect(),
                    );
                    self.campos_el.push(campo);
                }
            }
            self.linhas.push(format!("    this._el_{n} = {criacao};"));
            format!("this._el_{n}")
        };
        if namespace.is_some() && !pai.is_empty() {
            self.linhas.push(format!("    {pai}.append({alvo});"));
        }
        // Com `ViewContainer` (diretiva que injeta `ViewContainerRef`), a
        // raiz é ele (`vcAppEl ?? renderNode`, caso j49).
        let container = resolvido.as_ref().is_some_and(|r| r.container);
        if pai.is_empty() {
            self.raizes.push(if container {
                format!("this._appEl_{n}")
            } else {
                alvo.clone()
            });
        }
        if let Some(res) = &resolvido {
            self.registrar_no_por_posicao(e.inicio, &res.instancias);
        }
        // O resultado de um `@ViewChild(Diretiva)` ([`chave_de_tipo`]): o
        // campo de cada diretiva do nó, antes dos `#ref` (a ordem do
        // `_resolvedProvidersArray` no `beforeChildren`, caso j117).
        if let Some(res) = &resolvido {
            for (d, campo) in &res.diretivas {
                if d.e_componente {
                    continue;
                }
                let chave = chave_de_tipo(&d.uri, &d.classe);
                let leitura = format!("this.{campo}");
                self.refs.insert(chave.clone(), leitura.clone());
                self.refs_em_ordem.push((chave, leitura));
            }
        }
        // `renderNode.toReadExpr()`: o local ou o campo, como o
        // nó tiver sido declarado.
        for r in &e.referencias {
            // Com valor, a instância da diretiva exportada.
            let leitura = match (r.valor.is_empty(), exportada(r), &resolvido) {
                (true, _, _) => alvo.clone(),
                (false, Some(d), Some(res)) => match res
                    .diretivas
                    .iter()
                    .find(|(x, _)| std::sync::Arc::ptr_eq(x, &d))
                {
                    Some((_, campo)) => format!("this.{campo}"),
                    None => continue,
                },
                _ => continue,
            };
            self.refs.insert(r.nome.clone(), leitura.clone());
            self.refs_em_ordem.push((r.nome.clone(), leitura));
        }
        // As mensagens `@i18n` do nó: a dos atributos sai no lugar do
        // literal, a dos filhos no lugar do texto.
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
        let i18n_filhos = i18n
            .iter()
            .position(|(k, _)| k.is_none())
            .map(|i| i18n.remove(i).1);
        for (atributo, _) in &i18n {
            // `I18nMessage(astNode.value!, ..)`: o valor escrito, sem
            // interpolação; entidade, forma especial do atributo ou
            // atributo ausente (ou ligado: `[x]`) ainda não.
            let apto = e.atributos.iter().any(|a| {
                Some(&a.nome) == atributo.as_ref()
                    && !a.valor.contains("{{")
                    && !a.valor.contains('&')
                    && !matches!(a.nome.as_str(), "class" | "tabindex" | "tabIndex" | "style")
            });
            if !apto {
                self.anotar(recusa(
                    Motivo::I18n,
                    "@i18n:x sem atributo x escrito e simples",
                ))?;
            }
        }
        // Atributos saem em ordem alfabética (`_toSortedBindings`).
        let mut atributos = e.atributos.clone();
        atributos.sort_by(|a, b| a.nome.cmp(&b.nome));
        for a in atributos.iter().filter(|a| !a.valor.contains("{{")) {
            let valor = match i18n.iter().find(|(k, _)| k.as_ref() == Some(&a.nome)) {
                Some((_, m)) => {
                    let m = m.clone();
                    match self.mensagem(&a.valor, &m) {
                        Ok(campo) => campo,
                        Err(r) => {
                            self.anotar(r)?;
                            literal(&a.valor)
                        }
                    }
                }
                None => literal(&a.valor),
            };
            if a.nome == "class" {
                // `writeLiteralAttributeValues`: `NonHtml` fora do HTML.
                let metodo = if dom::tag_html(&tag) {
                    "updateChildClass"
                } else {
                    "updateChildClassNonHtml"
                };
                self.linhas
                    .push(format!("    this.{metodo}({alvo}, {valor});"));
            } else if a.nome == "tabindex" || a.nome == "tabIndex" {
                // `TabIndexBinding` (`binding_converter.dart`): o literal vira
                // `el.tabIndex = N` (`visitTabIndexBinding`); não inteiro é
                // erro de compilação no oficial.
                match a.valor.trim().parse::<i64>() {
                    Ok(n) if a.valor.trim() == a.valor => {
                        self.linhas.push(format!("    {alvo}.tabIndex = {n};"));
                    }
                    _ => self.anotar(recusa(Motivo::Ligacao, "tabindex que não é inteiro"))?,
                }
            } else {
                // `style` escrito é um atributo como outro qualquer
                // (`setAttribute` no `build()`), também junto de `[style.x]`,
                // que só escreve na detecção (caso j74).
                let dom = self.dom();
                let nome = &a.nome;
                self.linhas.push(format!(
                    "    {dom}.setAttribute({alvo}, '{nome}', {valor});"
                ));
            }
        }
        self.tag_atual = tag.clone();
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
        // O que uma diretiva do nó recebe como entrada é dela (caso j59).
        for a in e
            .atributos
            .iter()
            .filter(|a| a.valor.contains("{{") && !consome_entrada(&casadas, &a.nome))
        {
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
            self.ouvintes_com_hospedeiro(&do_no, &casadas, r, &alvo)?;
        } else {
            self.eventos(&do_no, &alvo)?;
        }
        if self.com_estilo {
            // Isolamento de estilo por atributo: o elemento entra
            // no escopo do componente (`shimCssForNode`: `addShimE` no nó
            // tipado `Element`, a tag que não é HTML).
            let metodo = if tipo == "Element" {
                "addShimE"
            } else {
                "addShimC"
            };
            self.linhas.push(format!("    this.{metodo}({alvo});"));
        }
        let mut injetor = None;
        if let Some(r) = &resolvido {
            // O `ViewContainer` nasce com o `CompileElement`, antes dos
            // provedores (o campo antes dos das diretivas).
            if r.container {
                let vc = self.imp.q(VIEW_CONTAINER);
                let pai_indice = if pai.is_empty() {
                    self.pai_projetado
                        .map_or_else(|| "null".to_string(), |k| k.to_string())
                } else {
                    indice_do_elemento(pai)
                };
                self.campos_filho
                    .push(format!("  late final {vc}ViewContainer _appEl_{n};"));
                self.linhas.push(format!(
                    "    this._appEl_{n} = {vc}ViewContainer({n}, {pai_indice}, this, {alvo});"
                ));
                self.ancoras.push(format!("_appEl_{n}"));
            }
            self.diretivas_do_no(e, r, &alvo, &propriedades, &eventos)?;
            let injetaveis: Vec<(Vec<crate::diretivas::Token>, String)> = r
                .instancias
                .iter()
                .filter(|i| !i.injetavel_por.is_empty())
                .map(|i| (i.injetavel_por.clone(), i.leitura.clone()))
                .collect();
            if !injetaveis.is_empty() {
                injetor = Some(self.injetores.len());
                self.injetores.push((n, n, injetaveis));
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
                elemento: alvo.clone(),
            });
        }
        // Os provedores injetáveis deste nó ficam acima dos filhos
        // (`injetavel_por`: os visíveis e os apelidos).
        let antes_acima = self.acima.len();
        if let Some(r) = &resolvido {
            for i in &r.instancias {
                // A visibilidade só decide o `injectorGetInternal` (outras
                // visões); no mesmo template o nó de baixo acha até o
                // provedor local pelo token dele (caso j41).
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
        self.pilha.push((n, resolvido.is_some()));
        let r = match &i18n_filhos {
            Some(m) => self.filhos_i18n(e, m, &alvo),
            None => {
                // Os descendentes herdam o namespace.
                let salvo = std::mem::replace(
                    &mut self.ns_atual,
                    namespace.as_ref().map(|(ns, _)| ns.clone()),
                );
                let r = self.nos(&e.filhos, &alvo);
                self.ns_atual = salvo;
                r
            }
        };
        self.pilha.pop();
        self.acima.truncate(antes_acima);
        if let Some(res) = &resolvido {
            self.consultas_das_diretivas(e, &res.diretivas, n)?;
            self.ganchos_depois_dos_filhos(&res.diretivas);
        }
        // `ProviderNode(nodeIndex, nodeIndex + childNodeCount)`.
        if let Some(i) = injetor {
            self.injetores[i].1 = self.proximo - 1;
        }
        r
    }

    /// Os filhos de um elemento com `@i18n`: um texto só vira uma mensagem
    /// (`internationalize`, `_textMessage`) e um nó de texto com ela. Com
    /// HTML dentro ([`mensagem_com_html`]) a mensagem é um método estático
    /// com um parâmetro por tag (`createI18nMessage`), e o nó é um
    /// `DocumentFragment` (`createHtml`): `final _html_n =
    /// createTrustedHtml(_message_K('<b>', '</b>'));` e `pai.append(_html_n)`.
    fn filhos_i18n(
        &mut self,
        e: &crate::html::Elemento,
        m: &MetaI18n,
        alvo: &str,
    ) -> Result<(), Recusa> {
        let [No::Texto(t)] = e.filhos.as_slice() else {
            return self.filhos_i18n_html(e, m, alvo);
        };
        if t.trim().is_empty() {
            return self.anotar(recusa(Motivo::I18n, "mensagem @i18n com HTML ou vazia"));
        }
        let campo = self.mensagem(t, m)?;
        let n = self.proximo;
        self.proximo += 1;
        let dom = self.dom();
        self.linhas.push(format!(
            "    final _text_{n} = {dom}.appendText({alvo}, {campo});"
        ));
        Ok(())
    }

    /// Os `@HostListener` de várias diretivas do nó para o mesmo evento: um
    /// `_handleEvent_N` que chama cada um, na ordem.
    fn handler_de_grupo(&mut self, lista: &[(String, crate::diretivas::Ouvinte)]) -> String {
        let n = self.metodos_evento.len();
        let corpo: Vec<String> = lista
            .iter()
            .map(|(campo, o)| format!("    this.{campo}.{}({});", o.metodo, o.args))
            .collect();
        self.metodos_evento.push(format!(
            "\n  void _handleEvent_{n}($event) {{\n{}\n  }}\n",
            corpo.join("\n")
        ));
        format!("this.eventHandler1(this._handleEvent_{n})")
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
        self.criar_instancias(e, &r.instancias, alvo, "this")?;
        self.registrar_diretivas(&r.diretivas, alvo);
        self.ligar_diretivas(
            e,
            &r.instancias,
            &r.diretivas,
            alvo,
            propriedades,
            eventos,
            true,
            true,
        )
    }

    fn criar_instancias(
        &mut self,
        e: &crate::html::Elemento,
        instancias: &[crate::diretivas::Instancia],
        alvo: &str,
        // `ChangeDetectorRef` no nó: a `componentView` do filho, ou a
        // própria visão (`compile_element.dart:199`).
        detector: &str,
    ) -> Result<(), Recusa> {
        use crate::diretivas::{Argumento, Criacao, Token};
        for inst in instancias {
            // Um `providers:` do filho que não é apelido: escrito como na
            // hospedeira (`ProviderSource.build`). O preguiçoso é campo com
            // inicializador; o que o filho ou uma diretiva do nó pede sai no
            // `build()`, na ordem dos provedores (casos j61, j67).
            if let Criacao::Expressao(_) | Criacao::Multi(_) = &inst.criacao {
                let vista = self.visao_do_injetor();
                if inst.preguicosa {
                    let texto = texto_de_provedor_preguicoso(inst, &self.asset, Some(&vista))?;
                    let texto = resolver_tardios(self.imp, &texto);
                    self.campos_preguicosos.push(texto);
                    self.posicoes_preguicosas.push(e.inicio);
                    continue;
                }
                let tipo = resolver_tardios(self.imp, &tipo_do_provedor(inst, &self.asset)?);
                // `isDevMode` e o `debugInjectorWrap` com imports tardios,
                // na ordem do texto.
                let util = tardio(UTILITIES);
                let v = Some(vista.as_str());
                let valor = match &inst.criacao {
                    Criacao::Expressao(x) => {
                        texto_da_expr(x, &inst.token, &self.asset, &util, 4, v)
                    }
                    Criacao::Multi(itens) => format!(
                        "[{}]",
                        itens
                            .iter()
                            .map(|x| texto_da_expr(x, &inst.token, &self.asset, &util, 4, v))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    _ => unreachable!(),
                };
                let valor = resolver_tardios(self.imp, &valor);
                self.campos_filho
                    .push(format!("  late final {tipo} {};", inst.campo));
                self.linhas
                    .push(format!("    this.{} = {valor};", inst.campo));
                continue;
            }
            let tipo = match (&inst.criacao, &inst.token) {
                (Criacao::Diretiva { diretiva, .. }, _)
                    if !diretiva.ligacoes_do_hospedeiro.is_empty() =>
                {
                    let prefixo = prefixo_do_ngcd(self.imp, &diretiva.uri, &self.asset);
                    let args = self.argumentos_de_tipo(&diretiva.uri, &diretiva.classe, e);
                    format!("{prefixo}{}NgCd{args}", diretiva.classe)
                }
                (Criacao::Diretiva { diretiva, .. }, _) => {
                    let prefixo = self.imp.q(&import_de(&diretiva.uri, &self.asset));
                    let args = self.argumentos_de_tipo(&diretiva.uri, &diretiva.classe, e);
                    format!("{prefixo}{}{args}", diretiva.classe)
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
                _ => return Err(recusa(Motivo::DiretivaPorSeletor, "provedor sem tipo")),
            };
            let valor = match &inst.criacao {
                Criacao::Diretiva { diretiva, args } => {
                    // Com `@HostBinding`, o `XNgCd` do `.template.dart` da
                    // diretiva embrulha a instância.
                    let cd = if diretiva.ligacoes_do_hospedeiro.is_empty() {
                        None
                    } else {
                        Some(prefixo_do_ngcd(self.imp, &diretiva.uri, &self.asset))
                    };
                    // Dependência do injetor de fora: a criação vai
                    // embrulhada em `debugInjectorWrap` sob `isDevMode`, e o
                    // emissor escreve `isDevMode` e o `debugInjectorWrap`
                    // antes da classe e dos tokens (a ordem dos imports).
                    let injeta = args.iter().any(|a| matches!(a, Argumento::DeFora { .. }));
                    let envolto =
                        injeta.then(|| (self.imp.alias(UTILITIES), self.imp.alias(DI_ERRORS)));
                    let classe = format!(
                        "{}{}",
                        self.imp.q(&import_de(&diretiva.uri, &self.asset)),
                        diretiva.classe
                    );
                    let v = self.visao_do_injetor();
                    let mut textos = Vec::new();
                    for a in args {
                        textos.push(match a {
                            Argumento::Elemento => alvo.to_string(),
                            Argumento::Detector => detector.to_string(),
                            Argumento::Nulo => "null".to_string(),
                            Argumento::Campo(c) => format!("this.{c}"),
                            Argumento::Acima(leitura) => leitura.clone(),
                            Argumento::Injetor(n) => format!("this.injector({n})"),
                            Argumento::Container => {
                                format!("this._appEl_{}", indice_do_elemento(alvo))
                            }
                            // O atributo escrito no elemento (`attrs`),
                            // sem ligação; ausente, `null`.
                            Argumento::Atributo(nome) => e
                                .atributos
                                .iter()
                                .find(|a| a.nome == *nome)
                                .map_or_else(|| "null".to_string(), |a| literal(&a.valor)),
                            Argumento::DeFora { token, opcional } => {
                                let metodo = if *opcional {
                                    "injectorGetOptional"
                                } else {
                                    "injectorGet"
                                };
                                let t = expr_do_token(&token_local(token, &self.asset));
                                let t = resolver_tardios(self.imp, &t);
                                format!("({v}.parentView!).{metodo}({t}, {v}.parentIndex)")
                            }
                        });
                    }
                    let chamada = format!("{classe}({})", textos.join(", "));
                    let criacao = match envolto {
                        None => chamada,
                        Some((util, erros)) => format!(
                            "({util}.isDevMode\n        ? {erros}.debugInjectorWrap({classe}, () {{\n            return {chamada};\n          }})\n        : {chamada})"
                        ),
                    };
                    match cd {
                        Some(q) => format!("{q}{}NgCd({criacao})", diretiva.classe),
                        None => criacao,
                    }
                }
                Criacao::Lista(itens) => {
                    let itens: Vec<String> = itens.iter().map(|c| format!("this.{c}")).collect();
                    format!("[{}]", itens.join(", "))
                }
                // Só a visão-hospedeira cria provedores assim.
                Criacao::Expressao(_) | Criacao::Multi(_) => {
                    return Err(recusa(
                        Motivo::DiretivaPorSeletor,
                        "provedor que não é diretiva nem apelido no nó",
                    ));
                }
            };
            // As leituras de cima podem trazer o import do `unsafeCast`
            // tardio (nó de um filho): alocado aqui, onde o texto sai.
            let valor = resolver_tardios(self.imp, &valor);
            if inst.preguicosa {
                // `late` sem `final`, com o valor no inicializador.
                self.campos_preguicosos
                    .push(format!("  late {tipo} {} = {valor};", inst.campo));
                self.posicoes_preguicosas.push(e.inicio);
            } else {
                self.campos_filho
                    .push(format!("  late final {tipo} {};", inst.campo));
                self.linhas
                    .push(format!("    this.{} = {valor};", inst.campo));
            }
        }
        Ok(())
    }

    /// `registerDirectives`: as instâncias das diretivas, na ordem.
    /// (Só o componente no nó, sem diretiva: nada a registrar.)
    fn registrar_diretivas(
        &mut self,
        diretivas: &[(std::sync::Arc<crate::diretivas::Diretiva>, String)],
        alvo: &str,
    ) {
        if !diretivas.is_empty() {
            let dev = self.imp.alias(DEVTOOLS);
            let registros: Vec<String> = diretivas
                .iter()
                .map(|(_, c)| {
                    format!("      {dev}.Inspector.instance.registerDirective({alvo}, this.{c});")
                })
                .collect();
            self.linhas.push(format!(
                "    if ({dev}.isDevToolsEnabled) {{\n{}\n    }}",
                registros.join("\n")
            ));
        }
    }

    /// As ligações das `diretivas` do nó: o `detectHostChanges` das que têm
    /// `@HostBinding` e as entradas (`entradas`), e as saídas (`saidas`),
    /// diretiva por diretiva, na ordem dada (`transformedDirectiveAsts`).
    #[allow(clippy::too_many_arguments)]
    fn ligar_diretivas(
        &mut self,
        e: &crate::html::Elemento,
        instancias: &[crate::diretivas::Instancia],
        diretivas: &[(std::sync::Arc<crate::diretivas::Diretiva>, String)],
        alvo: &str,
        propriedades: &[crate::html::Ligacao],
        eventos: &[crate::html::Ligacao],
        entradas: bool,
        saidas: bool,
    ) -> Result<(), Recusa> {
        use crate::diretivas::Criacao;
        // `detectHostChanges` das diretivas com `@HostBinding`, com as
        // ligações de propriedade do nó (`bindDirectiveHostProps`).
        for inst in instancias.iter().filter(|_| entradas) {
            if let Criacao::Diretiva { diretiva, .. } = &inst.criacao
                && !diretiva.ligacoes_do_hospedeiro.is_empty()
                && diretivas.iter().any(|(_, c)| *c == inst.leitura)
            {
                let vista = self.vista_do_hospedeiro.as_deref().unwrap_or("this");
                self.deteccao.push(format!(
                    "    this.{}.detectHostChanges({vista}, {alvo});",
                    inst.campo
                ));
            }
        }
        // Entradas e saídas, diretiva por diretiva, na ordem dos provedores
        // (`transformedDirectiveAsts`).
        for (d, campo) in diretivas {
            if !entradas {
                self.saidas_da_diretiva(d, campo, eventos)?;
                continue;
            }
            let mut ligadas: Vec<(&crate::html::Ligacao, bool)> = e
                .atributos
                .iter()
                .filter(|a| d.entrada(&a.nome).is_some())
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
            // Só `AfterChanges`, `OnInit` e `DoCheck` chegam aqui (o
            // `OnDestroy` sai no `destroyInternal`): os outros ganchos são
            // recusados pela guarda (`Diretiva::pendencia`).
            let ganchos = crate::componente::Ganchos {
                after_changes: d.ganchos.after_changes,
                on_init: d.ganchos.on_init,
                do_check: d.ganchos.do_check,
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
            if saidas {
                self.saidas_da_diretiva(d, campo, eventos)?;
            }
        }
        Ok(())
    }

    /// As `@Output` de uma diretiva do nó ligadas no template
    /// (`bindDirectiveOutputs`).
    fn saidas_da_diretiva(
        &mut self,
        d: &crate::diretivas::Diretiva,
        campo: &str,
        eventos: &[crate::html::Ligacao],
    ) -> Result<(), Recusa> {
        {
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
///
/// Com `@HostBinding` (`hospedeiro`), o `detectHostChanges(firstCheck)` da
/// visão do componente vem logo antes do `detectChanges` dela, depois dos
/// ganchos de conteúdo (caso j32).
fn ciclo_de_vida(
    g: &crate::componente::Ganchos,
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
        // O `ViewContainer` do elemento hospedeiro (componente que injeta
        // `ViewContainerRef`): depois de `ngOnInit`/`ngDoCheck`, antes dos
        // ganchos de conteúdo (caso j47).
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

/// Tipos que o ngcompiler trata como primitivos na interpolação
/// (`isBool`, `isNumber`, `isDouble`, `isInt`).
fn primitivo(tipo: &str) -> bool {
    matches!(tipo, "bool" | "num" | "double" | "int")
}

/// Algum elemento tem ligação de propriedade (ou um `#ref` lido como
/// local, de `refs`) e, portanto, vira campo?
fn tem_elemento_ligado(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    refs: &std::collections::HashSet<String>,
) -> bool {
    nos.iter().any(|n| match n {
        // Subárvore de `*` é da visão embutida.
        No::Elemento(e) if e.estrela.is_some() => false,
        No::Elemento(e) => match filhos.get(&e.nome) {
            None => {
                liga_no_elemento(e, &diretivas_casadas(usadas, e))
                    || e.referencias.iter().any(|r| refs.contains(&r.nome))
                    || tem_elemento_ligado(&e.filhos, filhos, usadas, refs)
            }
            // O nó de um filho vira campo com ligação própria do elemento
            // ([`Corpo::filho`]).
            Some(f) => {
                elemento_do_filho_ligado(e, f, &diretivas_casadas(usadas, e))
                    || tem_elemento_ligado(&e.filhos, filhos, usadas, refs)
            }
        },
        _ => false,
    })
}

/// O elemento de um componente filho vira campo da visão: ligação do
/// próprio elemento (`[class.x]`, `[style.x]`, `[attr.x]`, `[class]`) ou
/// atributo interpolado que nem o filho nem uma diretiva do nó recebe, ou
/// diretiva do nó com `@HostBinding` (o `detectHostChanges` lê o nó).
fn elemento_do_filho_ligado(
    e: &crate::html::Elemento,
    filho: &Filho,
    extras: &[std::sync::Arc<crate::diretivas::Diretiva>],
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

/// As diretivas que casam um elemento HTML, com os metadados lidos do
/// programa, na ordem de `directives:` (`_matchDirectives`). A guarda do
/// elemento já recusou as que o emissor não instancia.
fn diretivas_casadas(
    usadas: &[Usada],
    e: &crate::html::Elemento,
) -> Vec<std::sync::Arc<crate::diretivas::Diretiva>> {
    if usadas.is_empty() {
        return Vec::new();
    }
    let desc = crate::seletor::Elemento::do_template(e);
    usadas
        .iter()
        .filter(|u| u.filho.is_none() && crate::seletor::casa_algum(&u.seletores, &desc))
        .filter_map(|u| u.diretiva.clone())
        .collect()
}

/// Os tokens que o conteúdo do nó do filho pede aos provedores dele,
/// ansiosos e na ordem da visita ([`pedidos_do_conteudo`]): o resolvedor os
/// transforma logo depois dos ansiosos do nó, e a numeração dos campos sai
/// na ordem certa (caso i76; o `PopupRef_0_9` do `paper_tooltip`).
fn pedidos_ao_no_do_filho(
    e: &crate::html::Elemento,
    casadas: &[std::sync::Arc<crate::diretivas::Diretiva>],
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
) -> Result<Vec<(crate::diretivas::Token, bool)>, Recusa> {
    let tokens: Vec<crate::diretivas::Token> = casadas
        .iter()
        .flat_map(|d| d.provedores.iter().map(|p| p.token.clone()))
        .collect();
    let mut pedidos = Vec::new();
    pedidos_do_conteudo(&e.filhos, filhos, usadas, &tokens, true, &mut pedidos)?;
    Ok(pedidos)
}

/// Os campos dos provedores do nó e a criação deles no `build()`, na
/// ordem dada (`addDirectiveProviders`).
/// Os tokens de `tokens` que o conteúdo `nos` pede ao nó de cima
/// (`_getDependency`: o nó de baixo procura primeiro em si, menos com
/// `@SkipSelf`, e sobe; com `@Self` não sobe), na ordem dos pedidos: em
/// pré-ordem, em cada nó o filho e depois as diretivas. Um `*` no caminho
/// deixa o pedido preguiçoso (`_isViewRoot`), e um nó que provê o token
/// atende os de baixo. Um nó com pedidos de mais de um token, cuja ordem
/// depende da resolução dele, é recusado.
fn pedidos_do_conteudo(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    tokens: &[crate::diretivas::Token],
    ansioso: bool,
    saida: &mut Vec<(crate::diretivas::Token, bool)>,
) -> Result<(), Recusa> {
    use crate::diretivas::Token;
    for n in nos {
        let No::Elemento(e) = n else { continue };
        // O `*` leva o nó para a visão embutida: o pedido que sai de lá
        // atravessa a raiz dela (`_isViewRoot`) e é preguiçoso. O
        // `<template>` escrito fica aqui; só o conteúdo atravessa.
        if let Some(estrela) = &e.estrela {
            let mut sem = e.clone();
            sem.estrela = None;
            if estrela.nome == MARCA_DE_MOLDE {
                // As diretivas do `<template>` pedem daqui; o conteúdo, de
                // dentro da embutida, com o que elas proveem já atendido.
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
        // O que o próprio nó provê.
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
        // Vários tokens: a ordem é a da criação dos provedores deste nó (cada
        // um pede as dependências em ordem, `_getOrCreateLocalProvider`).
        if deste_no.iter().filter(|t| !ja(t, saida)).count() > 1 {
            let Some(ordem) = ordem_dos_pedidos_do_no(e, filho, &casadas, usadas) else {
                return Err(recusa(
                    Motivo::Ligacao,
                    "nó do conteúdo pedindo vários provedores preguiçosos do filho",
                ));
            };
            let mut ordenados: Vec<Token> =
                ordem.into_iter().filter(|t| deste_no.contains(t)).collect();
            ordenados.dedup();
            if ordenados.len() != deste_no.len() {
                return Err(recusa(
                    Motivo::Ligacao,
                    "nó do conteúdo pedindo vários provedores preguiçosos do filho",
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

/// Os tokens que os provedores de um nó do conteúdo pedem, na ordem em que
/// o `ProviderElementContext` dele os cria (cada diretiva ansiosa depois das
/// dependências, e as dependências dela em ordem). `None` quando o nó não se
/// resolve ou tem provedor que não é diretiva.
fn ordem_dos_pedidos_do_no(
    e: &crate::html::Elemento,
    filho: Option<&Filho>,
    casadas: &[std::sync::Arc<crate::diretivas::Diretiva>],
    usadas: &[Usada],
) -> Option<Vec<crate::diretivas::Token>> {
    use crate::diretivas::Criacao;
    let _ = e;
    let r = match filho {
        Some(f) => {
            let meta = f.metadados.as_ref()?;
            let (indice, cas) = casadas_do_no_do_filho(meta, casadas, usadas);
            let container = f
                .parametros
                .iter()
                .any(|p| matches!(p, Injetado::Container))
                || casadas.iter().any(|d| crate::diretivas::pede_container(d));
            crate::diretivas::resolver_no_do_filho(&cas, indice, 0, None, container, &[]).ok()?
        }
        None => crate::diretivas::resolver(casadas, 0, None).ok()?,
    };
    // O componente entra na resolução só com as dependências do próprio nó
    // ([`casadas_do_no_do_filho`]); aqui valem todas, na ordem do construtor.
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
            // O preguiçoso só pede no `afterElement`, depois do conteúdo.
            _ if i.preguicosa => {}
            _ => return None,
        }
    }
    Some(saida)
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
        || e
            .atributos
            .iter()
            .any(|a| a.valor.contains("{{") && !consome_entrada(casadas, &a.nome))
        // `detectHostChanges(this, el)` lê o nó na detecção.
        || casadas.iter().any(|d| !d.ligacoes_do_hospedeiro.is_empty())
}

/// Atributo escrito sem valor (`<input required>`): o intervalo dele é só o
/// nome.
fn sem_valor(a: &crate::html::Ligacao) -> bool {
    a.sem_valor
}

/// O `injectorGetInternal` de uma visão (`writeInjectorGetMethod`,
/// `ProviderForest.build`): os nós com provedor injetável, cada um com o
/// intervalo de índices que ele serve; vazio se não há nenhum.
/// Um nó com provedores injetáveis: (primeiro índice, último índice da
/// subárvore, [(tokens, campo)]).
type NoInjetor = (u32, u32, Vec<(Vec<crate::diretivas::Token>, String)>);

fn metodo_injetor(injetores: &[NoInjetor], asset: &str) -> String {
    use crate::diretivas::Token;
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
            tipo: crate::diretivas::TipoDeToken {
                uri: import_de(&tipo.uri, asset),
                ..tipo.clone()
            },
        },
        Token::Opaco { nome, tipo } => Token::Opaco {
            nome: nome.clone(),
            tipo: crate::diretivas::TipoDeToken {
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
    fn tokens(ts: &[Token]) -> String {
        let partes: Vec<String> = ts
            .iter()
            .map(|t| format!("identical(token, {})", expr_do_token(t)))
            .collect();
        // `||` associa à esquerda: `((a || b) || c)`.
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

/// A expressão de um token (`createDiTokenExpression`), com a URI já vista
/// deste arquivo e o import tardio: a classe, ou o `const MultiToken<T>('x')`
/// / `const OpaqueToken<T>('x')`. O `T` de `dart:core` sai sem prefixo; o de
/// outra biblioteca ganha import próprio (ver [`Importacoes::q_chave`]).
fn expr_do_token(t: &crate::diretivas::Token) -> String {
    use crate::diretivas::Token;
    let (classe_do_token, nome, tipo) = match t {
        Token::Classe { uri, classe } => return format!("{}{classe}", tardio_q(uri)),
        Token::Multi { nome, tipo } => ("MultiToken", nome, tipo),
        Token::Opaco { nome, tipo } => ("OpaqueToken", nome, tipo),
        Token::Elemento | Token::Detector => return String::new(),
    };
    let arg = texto_do_tipo_de_token(tipo);
    format!(
        "const {}{classe_do_token}<{arg}>({})",
        tardio_q(crate::diretivas::DI_TOKENS),
        literal(nome)
    )
}

/// O `T` de um token como o `fromDartType` o escreve: o de `dart:core` sem
/// prefixo (mas com o import alocado), o de outra biblioteca com import
/// próprio pela URI (`q_chave`), e os argumentos concretos recursivamente
/// (`List<import29.RelativePosition>`, pelo URI `package:` do tipo).
fn texto_do_tipo_de_token(tipo: &crate::diretivas::TipoDeToken) -> String {
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

/// O que gera campo na classe da visão, na ordem em que aparece.
enum CampoDaVisao<'a> {
    /// O filho, se o nó dele tem `ViewContainer`, e os campos dos
    /// provedores do nó criados antes dele ([`uri_do_campo`]).
    Filho(&'a Filho, bool, Vec<String>),
    /// O `ViewContainer` de um elemento comum (diretiva que injeta
    /// `ViewContainerRef`), antes das diretivas.
    Container,
    /// Diretiva estrutural, com a URI da classe dela.
    Estrutural(&'static str),
    /// `<template>` escrito: o `ViewContainer`, com `#ref` o campo do
    /// `TemplateRef` e os campos das diretivas dele (o texto com o import
    /// tardio, [`uri_do_campo`]), na ordem.
    Molde(bool, Vec<String>),
    /// Provedores de diretivas num nó: o texto com os imports tardios do
    /// campo de cada um ([`uri_do_campo`]), na ordem.
    Diretivas(Vec<String>),
    /// Provedores preguiçosos (campo com inicializador): vêm antes dos
    /// outros campos na classe, e os imports deles também.
    Preguicosos(Vec<String>),
}

/// Os campos que o template vai gerar, em ordem de documento. A ordem dos
/// imports segue esta lista, e é ela que faz a numeração bater com a do
/// oficial.
fn campos_em_ordem<'a>(
    nos: &[No],
    filhos: &'a std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    asset: &str,
) -> Vec<CampoDaVisao<'a>> {
    let mut saida = Vec::new();
    let separar = |instancias: &[crate::diretivas::Instancia], saida: &mut Vec<CampoDaVisao>| {
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
            // O conteúdo vai para a visão embutida; os campos dele são de lá.
            if estrela.nome == MARCA_DE_MOLDE {
                let mut sem = e.clone();
                sem.estrela = None;
                let casadas = diretivas_casadas(usadas, &sem);
                let campos = if casadas.is_empty() {
                    Vec::new()
                } else {
                    crate::diretivas::resolver_de_molde(&casadas, 0, None, false, &|_| {
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
            // Os outros provedores do nó do filho, depois da instância.
            let extras = diretivas_casadas(usadas, e);
            let container = f
                .parametros
                .iter()
                .any(|p| matches!(p, Injetado::Container))
                || extras.iter().any(|d| crate::diretivas::pede_container(d));
            let resolvido = f
                .metadados
                .as_ref()
                .filter(|meta| !extras.is_empty() || !meta.provedores.is_empty())
                .and_then(|meta| {
                    let (indice, casadas) = casadas_do_no_do_filho(meta, &extras, usadas);
                    let pedidos = pedidos_ao_no_do_filho(e, &casadas, filhos, usadas).ok()?;
                    let r = crate::diretivas::resolver_no_do_filho(
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
                    // Os preguiçosos que o conteúdo pede já vêm ansiosos e
                    // na posição certa (caso i76).
                    separar(&r.instancias[pos + 1..], &mut saida);
                }
                None => saida.push(CampoDaVisao::Filho(f, container, Vec::new())),
            }
        } else {
            let casadas = diretivas_casadas(usadas, e);
            if let (false, Ok(r)) = (
                casadas.is_empty(),
                crate::diretivas::resolver(&casadas, 0, None),
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

/// As diretivas do nó de um componente filho, para o resolvedor: o filho
/// com os `providers:` dele e só as dependências que o próprio nó provê (o
/// resto se resolve à parte, em [`Corpo::construcao_do_filho`]) — elas
/// decidem a ordem dos campos, porque o `_getOrCreateLocalProvider` do
/// oficial cria as dependências de um provedor ansioso antes dele (caso
/// j61) —, e as diretivas do nó. A ordem é a de `directives:`, com o filho
/// no lugar dele (`_matchDirectives`); volta com o índice dele.
fn casadas_do_no_do_filho(
    meta: &crate::diretivas::Diretiva,
    extras: &[std::sync::Arc<crate::diretivas::Diretiva>],
    usadas: &[Usada],
) -> (usize, Vec<std::sync::Arc<crate::diretivas::Diretiva>>) {
    let providos: Vec<crate::diretivas::Token> = std::iter::once(meta)
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

/// O campo de um provedor do nó, com os imports tardios na ordem em que o
/// oficial os escreve: o tipo e, no preguiçoso de `providers:`, o valor.
fn uri_do_campo(i: &crate::diretivas::Instancia, asset: &str) -> String {
    use crate::diretivas::{Criacao, Token};
    let uri = match (&i.criacao, &i.token) {
        // O preguiçoso tem o valor no campo; o ansioso, só o tipo (o valor
        // sai no `build()`).
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
            // O `XNgCd` declarado neste mesmo arquivo não tem import (j45).
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

/// O que é preciso para emitir uma visão embutida, guardado durante a
/// varredura e usado depois.
struct EspecEmbutida {
    /// Número da visão (`_ViewX3`), atribuído na varredura.
    indice: u32,
    /// O início da estrela que a cria (chave de [`Contexto::sujas_de_conteudo`]).
    estrela: Option<usize>,
    /// O namespace herdado ([`Corpo::ns_atual`]).
    ns: Option<String>,
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
    /// Os `#ref` lidos como local declarados em visões ancestrais: (nome,
    /// classe da visão que o declara, quantos `parentView` até ela).
    refs_ancestrais: Vec<(String, String, u32)>,
    /// Os resultados de consulta de visão nesta visão: (`#ref`, campo sujo,
    /// quantos `parentView` até a visão do componente).
    refs_consultados: Vec<(String, String, u32)>,
    /// As mesmas, na ordem das linhas do `dirtyParentQueriesInternal` (a do
    /// primeiro resultado de cada uma, [`primeiro_resultado`]).
    sujas: Vec<(String, String, u32)>,
    /// As consultas com resultado mais abaixo ([`Corpo::consultas_em_transito`]).
    consultas_em_transito: Vec<(String, String, u32)>,
    /// Os provedores acima da âncora, vistos da visão nova.
    acima: Vec<(crate::diretivas::Token, String, Option<(String, u32)>)>,
    /// Os campos de `acima` que são provedores preguiçosos do elemento
    /// dono (campo, classe da visão dele): um nó abaixo que os pede os
    /// transformaria antes do `afterElement` do dono (lacuna L1 da seção 05).
    preguicosos_acima: std::collections::HashSet<(String, String)>,
    /// Os deles cujo dono modelou os pedidos do conteúdo (nó de filho): de
    /// outra visão, lidos pelo campo preguiçoso.
    preguicosos_com_pedidos: std::collections::HashSet<(String, String)>,
    componentes_acima: u32,
    incertos_acima: u32,
    /// O índice do primeiro `<ng-content>` da embutida no template.
    proxima_projecao: u32,
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

/// Os nomes de `exports:` do componente: o qualificador (import tardio) e o
/// que cada um designa.
pub(crate) type Exportados =
    std::collections::HashMap<String, (String, crate::resolucao::Exportado)>;

/// O que toda visão do arquivo compartilha: o componente, as diretivas que
/// ele usa e onde o arquivo mora.
/// Um `Typed` de `directiveTypes:` com a classe resolvida e os argumentos
/// já escritos (`<T>`, `<import3.X>`).
#[derive(Debug, Clone)]
struct TipoDeDiretivaResolvido {
    uri: String,
    classe: String,
    argumentos: String,
    em: Option<String>,
}

struct Contexto<'a> {
    membros: &'a std::collections::HashMap<String, crate::componente::Membro>,
    /// Os nomes de `exports:` resolvidos ([`Exportados`]).
    exportados: &'a Exportados,
    /// Os parâmetros de tipo do componente (`T`, `U`), livres nas visões.
    nomes_genericos: Vec<String>,
    /// Os tokens dos `viewProviders:` do componente
    /// (`ProviderViewContext.viewProviders`): um `@Host()` da visão dele que
    /// pede um deles vai ao injetor em vez de ficar `null`.
    tokens_de_visao: Vec<crate::diretivas::Token>,
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
    /// Os argumentos de tipo de um componente genérico (`<T, U>`) e a
    /// declaração deles (`<T extends num, U>`); vazios no resto.
    genericos: String,
    genericos_decl: String,
    preservar_espacos: bool,
    html: String,
    pipes: &'a PipesDoTemplate,
    /// Os nomes de `#ref` que podem virar local ([`referencias_candidatas`]).
    refs_candidatos: std::collections::HashSet<String>,
    /// (chave, token) das consultas com `read:` de um provedor do nó
    /// ([`token_de_leitura`]).
    leituras: Vec<(String, crate::diretivas::Token)>,
    /// Os `#ref` que só consultas com `read:` de provedor procuram: o nó
    /// não precisa virar campo.
    refs_so_por_provedor: std::collections::HashSet<String>,
    /// Os `<template #x>` que uma consulta lê como `ViewContainerRef`: o
    /// `ViewContainer` do nó deixa de ser privado ([`le_container`]).
    moldes_com_container: std::collections::HashSet<String>,
    /// `@changeDetectionLink` no componente.
    link_de_deteccao: bool,
    /// `directiveTypes:` resolvido ([`TipoDeDiretivaResolvido`]).
    tipos_de_diretiva: Vec<TipoDeDiretivaResolvido>,
    /// Os nomes de `#ref` repetidos ou sombreados por `let`.
    refs_ambiguos: std::collections::HashSet<String>,
    /// O nó de cada `#ref` visto, de todas as visões já percorridas: a visão
    /// aninhada é emitida depois da que a contém, e lê dela o campo.
    refs_resolvidos: std::cell::RefCell<std::collections::HashMap<String, String>>,
    /// A cadeia de cada consulta de visão dinâmica (pelo campo sujo): a
    /// âncora `_appEl_n` e a classe da visão embutida em cada nível, da do
    /// componente para baixo. Os níveis de baixo só se conhecem ao emitir
    /// as visões aninhadas.
    ancoras_de_consulta: std::cell::RefCell<Ancoras>,
    /// As consultas de conteúdo com resultado em `*`, montadas no fim
    /// ([`resolver_conteudo_dinamico`]).
    conteudo_dinamico: std::cell::RefCell<Vec<ConteudoDinamico>>,
    /// Por visão embutida (o início da estrela dela), as consultas de
    /// conteúdo que ela marca no `dirtyParentQueriesInternal`.
    sujas_de_conteudo: std::cell::RefCell<SujasDeConteudo>,
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
    // Como na visão do componente (caso j91): o import do `text_binding` é
    // alocado antes do corpo, na posição do primeiro campo `TextBinding`;
    // quando nenhuma interpolação é mutável (todas escritas no `build()`,
    // como a de um nome de `exports:`), não há campo e a emissão é refeita
    // sem ele, da mesma tabela de imports (caso j112).
    let (imp_antes, coleta_antes) = (imp.clone(), coleta.clone());
    let mut com_tb = true;
    let (mut texto, aninhadas) = loop {
        let mut dentro = ctx.corpo(imp, nomes, coleta.take(), true);
        dentro.locais = espec.locais.clone();
        dentro.vista = espec.indice;
        dentro.profundidade = espec.profundidade;
        dentro.nivel_do_topo = espec.nivel_do_topo;
        // A numeração continua de onde o pai parou.
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

/// O texto de uma visão embutida e as especificações das aninhadas nela.
fn corpo_da_embutida(
    dentro: &mut Corpo,
    espec: &EspecEmbutida,
    ctx: &Contexto,
    ev: &str,
    classe: &str,
    fabrica: &str,
    com_tb: bool,
) -> Result<(String, Vec<EspecEmbutida>), Recusa> {
    // O campo de ligação de texto é declarado antes do construtor, então o
    // import dele entra aqui — e o do `package:intl`, antes ou depois dele
    // conforme a ordem de documento, quando a visão tem `@i18n` (caso j01).
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
    // Com filho, diretiva ou `*` na visão, a ordem dos campos entre eles
    // ainda não tem caso; sem mensagem em campo, a de HTML usa o `intl`
    // só no método.
    if contem_anotacao(&espec.nos) {
        if ordem_i18n.is_some()
            && !campos_em_ordem(&espec.nos, ctx.filhos, ctx.usadas, &ctx.asset).is_empty()
        {
            dentro.anotar(recusa(
                Motivo::I18n,
                "@i18n com filho, diretiva ou `*` na visão",
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
    // O construtor vem depois dos campos e antes do `build()`, e é ele que
    // nomeia a `RenderView`.
    let rv = dentro.imp.alias(RENDER_VIEW);
    let util = dentro.imp.alias(UTILITIES);
    // A declaração de cada local desta visão, pronta para quem a pedir (a
    // detecção ou um `_handleEvent_N`). Local de visão *ancestral* é lido
    // pela cadeia de `parentView` (`unsafeCast<_ViewX2>((this.parentView!))
    // .locals['$implicit']`, `getLocal` do `ViewNameResolver`, caso j125).
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
    // Os `#ref` das visões ancestrais: o campo do nó na visão que o
    // declara (`getPropertyInView`, sem cast do valor: a referência não tem
    // tipo).
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
    // Um `#ref` desta visão só pesa para as consultas que chegam a ela (as
    // dinâmicas: `refs_consultados` e as em trânsito); a estática, com o
    // primeiro resultado fora de `*`, nunca chega (`compile_query.dart:
    // 128-150`, caso j109).
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
    // Na coleta, um nó recusado não consome índice: a visão parece vazia
    // sem estar. O `<ng-container *x>` vazio e o `<template>` sem conteúdo
    // (só comentários, ou nada) são vazios de fato: a visão não tem raiz
    // nenhuma (`const <Object>[]`, casos i82, j54).
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
    // âncoras, valores anteriores, elementos (os resultados de consulta
    // antes dos ligados).
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
    // `dirtyParentQueriesInternal`, entre a detecção e o `destroyInternal`:
    // marca cada consulta da visão do componente com resultado aqui, e cada
    // consulta de conteúdo de uma visão de cima (`_setParentQueryAsDirty`,
    // na ordem do primeiro resultado).
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
        // A ordem é a do primeiro resultado de cada consulta nesta visão; no
        // mesmo nó, a de conteúdo antes da de visão (`_getQueriesFor`).
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
        // Sem posição conhecida, a ordem de antes (as de visão já vêm
        // ordenadas pelo primeiro resultado).
        marcadas.sort_by_key(|(p, tipo, k, _)| (*p, *tipo, *k));
        let linhas: Vec<String> = marcadas.into_iter().map(|(_, _, _, l)| l).collect();
        format!(
            "\n  @override\n  void dirtyParentQueriesInternal() {{\n{}\n  }}\n",
            linhas.join("\n")
        )
    };
    // O `injectorGetInternal` vem depois do `build()` e antes da detecção.
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
    if !dentro.metodos_i18n.is_empty() && !dentro.metodos_evento.is_empty() {
        return Err(recusa(
            Motivo::I18n,
            "@i18n com HTML e handler de evento na mesma visão",
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
    // As raízes (`rootNodesOrViewContainers`): cada nó criado sem pai, o
    // local ou o campo (`renderNode.toReadExpr()`) — mais de uma quando o
    // `*` está num `<ng-container>`. Na coleta, com nó recusado, a lista
    // pode vir vazia; o texto é descartado.
    let raizes = if dentro.raizes.is_empty() {
        vec!["_el_0".to_string()]
    } else {
        dentro.raizes.clone()
    };
    // `_generateInitStatement`: uma raiz só e nenhuma `subscription_N` é
    // `initRootNode`; o resto vai numa lista.
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
                // Com a cascata quebrada, os argumentos vão um por linha
                // (recuo de continuação, coluna 8).
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
    // Sem nó criado (a raiz é só um `TextBinding`), o `build()` é só o
    // `initRootNode`.
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
    // Sem tipo, sem cast (`getLocal`: `type != null && type != dynamic`).
    if l.tipo == "dynamic" {
        return Ok(format!("final {d} = {locals}[{chave}];"));
    }
    let tipo = if matches!(
        l.tipo.as_str(),
        "String" | "int" | "double" | "bool" | "num" | "Object"
    ) {
        format!("{}{}", tardio_q("dart:core"), l.tipo)
    } else {
        let sem_import = || recusa(Motivo::Ligacao, "tipo do local de `*ngFor` sem import");
        let (r, arquivo) = ctx.tipos.ok_or_else(sem_import)?;
        let escopo = l.escopo.as_deref().unwrap_or(arquivo);
        let cheio = instanciar_crus(&l.tipo, escopo, r).ok_or_else(sem_import)?;
        let livres: Vec<&str> = ctx.nomes_genericos.iter().map(String::as_str).collect();
        tipo_qualificado(&cheio, escopo, r, &ctx.asset, &livres).ok_or_else(sem_import)?
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
/// Os imports dos provedores preguiçosos da visão. O `ViewStorage` escreve
/// os campos na ordem em que são alocados: o provedor preguiçoso é alocado
/// quando a visão é construída, e a ligação de texto e os nós só depois, no
/// `NodeReferenceStorageVisitor` — então os campos (e os imports) deles vêm
/// antes de todos os outros, inclusive o `text_binding.dart` (caso j38).
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
    let sem_caminho = || recusa(Motivo::ComponenteNoTemplate, "filho sem caminho de import");
    // Os argumentos de `directiveTypes:` saem no tipo do campo, logo depois
    // do nome da classe: os imports deles vêm em seguida ao dela.
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
                    // O `ViewContainer` do nó (filho ou diretiva dele que
                    // injeta `ViewContainerRef`) e os provedores que o filho
                    // injeta entre a visão e a instância.
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
    // Os campos de pipe vêm depois dos `_expr_` e antes dos `_el_`.
    for uri in pipes {
        imp.alias(uri);
    }
    if tem_elemento_ligado(nos, filhos, usadas, refs) {
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
        .or_else(|| pai.rsplit_once("_anchor_"))
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

/// A expressão lê `nome` com o receptor implícito — que é quando o
/// `ViewNameResolver.getLocal` é chamado? O nome depois de `.`/`?.` é
/// membro de outra coisa, antes de `:` depois de `(`/`,` é argumento
/// nomeado, e dentro de aspas é texto.
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
    /// Terceiro argumento do construtor: `@Host()` de uma diretiva da
    /// mesma biblioteca num elemento acima (o `NgSwitch` do `NgSwitchWhen`).
    hospedeiro: Option<&'static str>,
    /// O construtor recebe o `TemplateRef` (o `NgTemplateOutlet` só o
    /// `ViewContainerRef`; o `TemplateRef` do `*` fica num local sem uso).
    com_template: bool,
    /// As entradas (`@Input`), na ordem de declaração na classe: é a ordem
    /// das ligações (`ast.inputs.sort(_orderingOf(directive.inputs))` no
    /// `ast_template_parser.dart`), qualquer que seja a ordem escrita.
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
            // As duas entradas são setters sem comparação própria: passam
            // pelo `checkBinding` (ou, imutáveis, pelo `_bindLiteral`).
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

/// `List<String>` -> `String`. Sem argumento de tipo não dá para tipar o
/// local do laço, e o oficial escreve `unsafeCast<T>` com T explícito.
fn tipo_do_elemento(tipo: &str) -> Option<String> {
    let t = tipo.trim().trim_end_matches('?');
    let abre = t.find('<')?;
    let base = t[..abre].trim();
    if !matches!(base, "List" | "Iterable" | "Set") {
        return None;
    }
    let dentro = t[abre + 1..].strip_suffix('>')?;
    // Um só argumento no nível de cima; os aninhados (`List<List<X>>`,
    // `List<Map<K, V>>`) vão inteiros.
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

/// O tipo que o analyzer dá a um nome de classe genérica escrito cru
/// (`Grupo` em `List<Grupo>`): a instanciação pelos limites, que o
/// `fromDartType` escreve com os argumentos (`Grupo<dynamic>`). Sem limite o
/// argumento é `dynamic`; com limite ainda não há caso (`None`).
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

/// O texto de um tipo com cada nome qualificado pelo import da biblioteca
/// que o declara, como o oficial escreve um `DartType` (`List<import2.X>`:
/// o `dart:core` sem prefixo, mas com o import alocado). Os imports ficam
/// marcados ([`tardio_q`]). `None` para o que não é tipo nomeado (função,
/// registro) ou nome que não se acha no `escopo`.
/// Os nomes em `livres` (parâmetros de tipo em escopo) saem como estão.
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
            let uri = r.uri_do_tipo(escopo, &nome)?;
            let simples = nome.rsplit('.').next().unwrap_or(&nome);
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
/// O saneador de uma propriedade do DOM (`securityContext` do
/// `DomElementSchemaRegistry`, com a tabela dele): `sanitizeHtml`,
/// `sanitizeStyle`, `sanitizeUrl` ou `sanitizeResourceUrl`, ou nenhum. A
/// chave é `tag|propriedade`, depois `*|propriedade`.
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

/// `getMappedPropName` do `DomElementSchemaRegistry`: o nome de
/// propriedade de um atributo, usado para achar o contexto de segurança.
fn propriedade_mapeada(atributo: &str) -> &str {
    match atributo {
        "class" => "className",
        "innerHtml" => "innerHTML",
        "readonly" => "readOnly",
        "tabindex" => "tabIndex",
        outro => outro,
    }
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

/// O `.template.dart` de um arquivo só de diretivas com `@HostBinding`: as
/// classes `XNgCd` na ordem do fonte, com a tabela de imports compartilhada
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

/// A classe `XNgCd` de uma diretiva com `@HostBinding`
/// (`DirectiveChangeDetector`), com os imports alocados em `imp` na ordem
/// da escrita: a superclasse, o campo `instance`, os parâmetros de
/// `detectHostChanges` e, no corpo, `checkBinding` e, pela ação de cada
/// ligação, o `dom_helpers` (e o saneador, quando há). Num arquivo com
/// componente, ela sai depois dos trechos dos componentes (caso j45).
pub fn classe_ngcd(h: &crate::Hospedeira, arquivo: &str, imp: &mut Importacoes) -> String {
    let cd = imp.alias(DIRECTIVE_CHANGE_DETECTOR);
    let proprio = imp.alias(arquivo);
    let rv = imp.alias(RENDER_VIEW);
    let html = imp.alias("dart:html");
    // Os imports do corpo são alocados na ordem do texto, que só se conhece
    // montado (as imutáveis vêm antes): marcas tardias, resolvidas no fim.
    let chk = tardio(CHECK_BINDING);
    let x = &h.classe;
    let mut campos = String::new();
    let mut corpo = String::new();
    // As imutáveis (`isImmutable`: campo `final`) saem antes, no
    // `if (firstCheck)`, sem campo, mas com o índice delas (caso j103).
    let mut constantes = String::new();
    for (k, (nome, membro)) in h.ligacoes.iter().enumerate() {
        // `hospedeira` (lib.rs) só deixa passar as formas conhecidas.
        let Ok(mut forma) = forma_do_hospedeiro(nome) else {
            continue;
        };
        let estatico = h.estaticos.contains(membro);
        if let FormaDoHospedeiro::Estilo { texto, nulo, .. } = &mut forma {
            // O estático é `dynamic` para o `_TypeResolver`: `?.toString()`.
            let tipo = if estatico {
                "dynamic"
            } else {
                h.tipos_de_estilo
                    .get(membro)
                    .map(String::as_str)
                    .unwrap_or_default()
            };
            *texto = tipo.trim_end_matches('?') == "String";
            *nulo = tipo.ends_with('?') || tipo == "dynamic";
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
        "\nclass {x}NgCd extends {cd}.DirectiveChangeDetector {{\n  final {proprio}.{x} instance;\n{campos}  {x}NgCd(this.instance);\n  void detectHostChanges({rv}.RenderView view, {html}.Element el) {{\n{corpo}  }}\n}}\n"
    )
}

/// Gera o trecho de um componente no `.template.dart` — do `styles$X` à
/// fábrica da visão-hospedeira —, alocando os imports na tabela do arquivo.
/// Com vários componentes no arquivo, a tabela é uma só e os trechos saem
/// na ordem do fonte ([`montar_arquivo`]), como o oficial escreve.
///
/// O que não couber volta `Err` com a primeira recusa, e o arquivo continua
/// vindo do `build_runner`.
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
    // Uma marca de import tardio que nenhum passo resolveu sairia no arquivo:
    // é uma forma que o emissor ainda não escreve.
    if texto.contains(['\u{1}', '\u{2}']) {
        return Err(recusa(
            Motivo::NaoEntendido,
            "import tardio sem resolver na saída",
        ));
    }
    Ok(texto)
}

/// O `.template.dart` de um arquivo com um componente só:
/// [`trecho_de_componente`] com uma tabela de imports própria.
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

/// O `.template.dart` inteiro: o cabeçalho, o import de si mesmo, a tabela
/// de imports e os trechos dos componentes.
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
        &mut Importacoes::default(),
    );
    let mut fora: std::collections::BTreeSet<Recusa> =
        coleta.unwrap_or_default().into_iter().collect();
    if let Err(r) = r {
        fora.insert(r);
    }
    // A folha é outra saída (`gerar_folha`): o template só a importa.
    if c.style_urls
        .iter()
        .any(|u| local.uri_do_estilo(u, !c.sem_encapsulamento).is_none())
    {
        fora.insert(recusa(Motivo::Estilos, "folha fora de lib/"));
    }
    fora
}

/// O que um `@HostBinding` de componente escreve no elemento hospedeiro:
/// as ligações de `createElementPropertyAst` com o elemento `div`
/// (`_securityContextElementName` do `DirectiveConverter`), escritas por
/// `bindAndWriteToRenderer` com `isHtmlElement` falso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FormaDoHospedeiro {
    /// `class.x`: `updateClassBindingNonHtml`.
    Classe(String),
    /// `class`, `className`, `attr.class` num componente: a classe inteira,
    /// `this.updateChildClassNonHtml(this.rootElement, v)` (o `ClassBinding`
    /// sem nome, caso j80).
    ClasseInteira,
    /// `attr.x`, com o saneador do contexto de segurança: `updateAttribute`.
    Atributo(String, Option<&'static str>),
    /// `attr.tabindex` estático: a propriedade `tabIndex` do elemento
    /// (`TabIndexBinding` do `binding_converter.dart`, que só o construtor
    /// da visão usa; a ligação dinâmica continua `updateAttribute`, caso
    /// j96).
    TabIndex,
    /// `style.x` e `style.x.unidade`, com o texto do valor decidido pelo
    /// tipo do membro (`visitStyleBinding`: `isString`, `isNullable`).
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

/// Um `@HostBinding` do componente, resolvido: o membro lido, se é
/// imutável (`isImmutable`: campo `final`) e a forma.
#[derive(Debug, Clone)]
struct LigacaoDoHospedeiro {
    membro: String,
    imutavel: bool,
    /// Membro estático: escrito no construtor da visão, com o valor lido da
    /// classe (`X.membro`), e fora do `detectHostChanges` (caso j96).
    estatico: bool,
    forma: FormaDoHospedeiro,
}

impl LigacaoDoHospedeiro {
    /// A instrução que escreve o valor `v` no elemento hospedeiro.
    fn acao(&self, v: &str) -> String {
        self.forma.acao("this.rootElement", v)
    }
}

impl FormaDoHospedeiro {
    /// A instrução que escreve o valor `v` no elemento `el` (o
    /// `this.rootElement` do componente, o `el` do `XNgCd` da diretiva). Os
    /// imports vão como marcas de [`tardio`], na ordem do texto.
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
            // Só o componente chega aqui ([`forma_do_hospedeiro_de_componente`]):
            // o receptor é a visão dele.
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
                        format!("(({v} == null) ? null : ({t} + {}))", literal(u))
                    }
                    None if *texto => v.to_string(),
                    None if *nulo => format!("{v}?.toString()"),
                    None => format!("{v}.toString()"),
                };
                format!("{el}.style.setProperty('{nome}', {valor})")
            }
        }
    }
}

/// A forma de um nome de `@HostBinding` (`createElementPropertyAst`): o que
/// não é `class.x`, `attr.x`, `style.x[.unidade]` ou propriedade simples é
/// recusado — `class`/`className` (a classe inteira), `attr.x.if`,
/// namespace e prefixo desconhecido ainda não têm caso.
pub(crate) fn forma_do_hospedeiro(nome: &str) -> Result<FormaDoHospedeiro, String> {
    let simples = |n: &str| !n.is_empty() && !n.contains(['.', ':']);
    let fora = || format!("@HostBinding('{nome}') fora de class.x, attr.x, style.x e propriedade");
    let partes: Vec<&str> = nome.split('.').collect();
    Ok(match partes.as_slice() {
        // `class.x.y`: só a segunda parte conta (`boundPropertyName =
        // parts[1]`, `template_parser.dart:100-102`), o resto é ignorado
        // (o `class.basic-icon.if` do `material_icon_toggle`).
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

/// [`forma_do_hospedeiro`] no componente, que também escreve a classe
/// inteira (`class`, `className`, `attr.class`) pela visão dele.
fn forma_do_hospedeiro_de_componente(nome: &str) -> Result<FormaDoHospedeiro, String> {
    if matches!(nome, "class" | "className" | "attr.class") {
        return Ok(FormaDoHospedeiro::ClasseInteira);
    }
    forma_do_hospedeiro(nome)
}

/// Os `@HostBinding` do componente na ordem do mapa `hostProperties` do
/// oficial. Sem herança, os da própria classe ([`Componente`]); com
/// herança, os dos metadados lidos do programa (`metadados.rs`: supertipos
/// primeiro, a chave é o nome e o último vence na posição do primeiro), com
/// a imutabilidade dos membros herdados perguntada ao resolvedor.
/// `style.x` precisa do tipo do membro (`isString`, `isNullable`).
///
/// # Erros
///
/// A recusa da forma que ainda não se escreve: nome fora das formas
/// conhecidas, herança sem os metadados do programa, membro herdado
/// ilegível, `style.x` em campo `final` ou de tipo desconhecido.
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
                return Err(fora("@HostBinding em componente que herda"));
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
                .ok_or_else(|| fora("@HostBinding herdado sem declaração legível"))?,
        };
        if let FormaDoHospedeiro::Estilo { texto, nulo, .. } = &mut forma {
            if imutavel {
                return Err(fora("@HostBinding('style.x') em campo final"));
            }
            let tipo = resolvedor
                .and_then(|r| r.tipo_do_membro(local.caminho, &c.classe, &membro))
                .map(|(t, _)| t)
                .or_else(|| c.membros.get(&membro).map(|m| m.tipo.clone()))
                .unwrap_or_default();
            let base = tipo.trim().trim_end_matches('?');
            if base.is_empty() || matches!(base, "dynamic" | "var" | "Object" | "Never") {
                return Err(fora("@HostBinding('style.x') de tipo desconhecido"));
            }
            *texto = base == "String";
            *nulo = tipo.trim().ends_with('?');
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
/// A emissão de um componente. O import do `text_binding.dart` é alocado
/// antes do corpo, na posição do primeiro campo `TextBinding` da classe;
/// quando nenhuma interpolação da visão raiz é mutável (todas no `build()`,
/// `isImmutable`), o campo não existe e a emissão é refeita sem ele, da
/// mesma tabela de imports (caso j91).
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
    // `pipes:` sem uso não muda a visão (caso b19).
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
                "styleUrls/styles com item que não é texto literal",
            ),
        )?;
    }
    // `styles: ['…']` escrito na anotação: cada texto entra na lista
    // `styles$X` depois das folhas de `styleUrls` (`_compileStyles`), com
    // shim no encapsulamento emulado e tal qual no `none` (casos j07, j08).
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
                    "CSS que o shim do ngdart recusa (o oficial lança)",
                ),
            )?,
        }
    }
    // `ViewEncapsulation.none` com folha: o template importa o `.css.dart`
    // (a folha sem shim, de `gerar_folha`), `ComponentStyles.unscoped` e
    // nenhum `addShimC` (caso i88).
    // Os `@HostBinding` do componente, com os herdados.
    let do_hospedeiro = match ligacoes_do_componente(c, local, resolvedor) {
        Ok(l) => l,
        Err(r) => {
            anotar(coleta, r)?;
            Vec::new()
        }
    };
    // A construção sai depois dos imports fixos, porque a injeção aloca os
    // seus (o `errors.dart` e o de cada tipo injetado) no fim da tabela.
    if let Some(r) = falta_para_construir(c, local, resolvedor) {
        anotar(coleta, r)?;
    }
    // `providers:` do componente: a visão-hospedeira os cria; com
    // `Visibility.all`, ela também o entrega pelo `injectorGetInternal`.
    if c.visibilidade_escrita && local.metadados.is_none() {
        anotar(
            coleta,
            recusa(
                Motivo::Providers,
                "visibility: sem os metadados do programa",
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
    let tokens_do_no: Vec<crate::diretivas::Token> = no_hospedeiro
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

    // As folhas compiladas, na ordem de `styleUrls`: cada uma aloca o import
    // dela quando a lista `styles$X` é escrita — a primeira do arquivo é o
    // primeiro import; num arquivo com vários componentes, a de um
    // componente seguinte entra no meio da tabela, e a repetida reaproveita
    // o import (caso j44). A folha entra pela URI `package:` mesmo estando
    // ao lado: é assim que o oficial escreve (o resolvedor de `styleUrls` é
    // outro, e não passa pelo caminho relativo).
    let mut estilo = Vec::new();
    for url in &c.style_urls {
        match local.uri_do_estilo(url, !c.sem_encapsulamento) {
            Some(uri) => estilo.push(imp.alias(&uri)),
            None => anotar(coleta, recusa(Motivo::Estilos, "folha fora de lib/"))?,
        }
    }
    // Componente genérico: a visão, a hospedeira, as embutidas e as
    // fábricas levam os parâmetros de tipo da classe. O limite escrito
    // (`T extends num`) é o primeiro tipo do cabeçalho da classe da visão:
    // o import dele é alocado antes do `ComponentView`.
    let (genericos, genericos_decl) = match parametros_de_tipo(c, local, resolvedor, imp) {
        Ok(g) => g,
        Err(r) => {
            anotar(coleta, r)?;
            Default::default()
        }
    };
    let vista = imp.alias(COMPONENT_VIEW);
    let proprio = imp.alias(local.arquivo);
    // Os campos da visão saem antes de tudo na classe — ligações de texto,
    // visões-filhas, valores anteriores, elementos —, e os imports são
    // alocados nessa mesma ordem. É isso que faz a numeração bater com a do
    // oficial; fora de ordem, a comparação byte a byte não vale nada.
    // `@i18n`: o `package:intl` entra com o primeiro campo `_message_N`,
    // antes ou depois do `text_binding.dart` conforme a ordem de documento.
    // Com filho, diretiva ou `*` na visão, a ordem dos campos (e dos
    // imports) entre eles ainda não tem caso.
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
            recusa(Motivo::I18n, "@i18n com filho, diretiva ou `*` na visão"),
        )?;
    }
    let refs_candidatos = referencias_candidatas(nos, c);
    let refs_locais = referencias_locais(nos, filhos, &refs_candidatos, &[]);
    // O nó desta visão que é resultado de consulta dinâmica é lido na
    // detecção (a lista `[this._el_0, ...]`): vira campo, como o que uma
    // expressão lê (caso j17).
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
    // `directiveTypes:`: a classe no escopo do componente e os argumentos
    // escritos como o `fromTypeLink` (`#T` é o parâmetro do componente).
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
                    format!("directiveTypes: Typed<{}> sem resolução", t.classe),
                )
            };
            let r = resolvedor.ok_or_else(falha)?;
            let uri = r.uri_do_tipo(local.caminho, &t.classe).ok_or_else(falha)?;
            let simples = t.classe.rsplit('.').next().unwrap_or(&t.classe).to_string();
            if !usadas.iter().any(|u| u.uri == uri && u.classe == simples) {
                return Err(recusa(
                    Motivo::NaoEntendido,
                    "directiveTypes: com diretiva fora de directives: (erro no oficial)",
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
    // O construtor da visão usa `document.createElement`, então `dart:html`
    // sempre entra antes do corpo do `build()`.
    let html = imp.alias("dart:html");
    // Os `@HostBinding` estáticos saem no construtor, logo depois do
    // `rootElement`: os imports deles vêm antes dos do `build()`.
    let estaticos_no_construtor: String = do_hospedeiro
        .iter()
        .filter(|l| l.estatico)
        .map(|l| {
            let valor = format!("{proprio}.{}.{}", c.classe, l.membro);
            format!("\n    {};", resolver_tardios(imp, &l.acao(&valor)))
        })
        .collect();

    // `exports:`: cada nome pelo import da biblioteca que o declara, alocado
    // quando a expressão é escrita (caso j51).
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
        html: html.clone(),
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
    // Os campos "sujos" abrem a classe, na ordem em que o oficial os aloca
    // ([`primeiro_resultado_dinamico`]; empate, a ordem das consultas).
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
    // `@ViewChild(ren)(Token)` fornecido por diretiva, com resultado em `*`:
    // pela máquina das consultas de conteúdo, com a raiz nesta visão.
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
    // Só mensagens com HTML: o `intl` é pedido pelo método, mais tarde.
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
    // O import do `text_binding.dart` pré-alocado sem campo que o use.
    let tb_sobrando = corpo.tb.is_some() && !corpo.tb_usado && !corpo.coletando();
    // O `ngOnDestroy` dos pipes vem depois dos das diretivas.
    corpo.destruir.extend(tabela.destruicao(0));
    // `@ViewChild` estático: atribuição imediata, no `afterNodes` — depois
    // dos ouvintes, na ordem de declaração das consultas
    // (`updateQueryAtStartup`, `createImmediateUpdates` em
    // `compile_query.dart`). `formas_contra_o_template` já garantiu que cada
    // `#ref` está uma vez só, num elemento HTML da própria visão.
    let moldes = referencias_de_moldes(nos);
    let mut consultas = Vec::new();
    for (i, q) in c.consultas.iter().enumerate() {
        // A dinâmica sai na detecção.
        if corpo.consultas_dinamicas.iter().any(|d| d.indice == i)
            || corpo.consultas_por_token.contains(&i)
        {
            continue;
        }
        // O que procurar entre os resultados: o `#ref`, ou a chave do tipo.
        let chave = if q.por_tipo {
            uri_da_consulta(q, local, resolvedor)
                .map(|u| chave_de_tipo(&u, &q.referencia))
                .unwrap_or_default()
        } else {
            q.referencia.clone()
        };
        // Com `read:` de provedor, a instância dele em cada nó; com
        // `read: ViewContainerRef` num `<template>`, o `ViewContainer` dele.
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
            // Filho `onPush` no resultado registraria o `ChangeDetectorRef`
            // de cada um: ainda não.
            if corpo.detectores.contains_key(&chave) {
                let r = recusa(Motivo::ViewChildEmFilho, "@ViewChildren de filho onPush");
                if corpo.coletando() {
                    corpo.anotar(r)?;
                } else {
                    *coleta = corpo.coleta.take();
                    return Err(r);
                }
            }
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
                // Filho `onPush`: o `ChangeDetectorRef` dele fica registrado
                // (`_createAddQueryChangeDetectorRefs`). Com `#ref` repetido,
                // o registro guardado pode não ser o do primeiro: ainda não.
                let repetido = corpo
                    .refs_em_ordem
                    .iter()
                    .filter(|(n, _)| *n == chave)
                    .count()
                    > 1;
                if repetido && corpo.detectores.contains_key(&chave) {
                    let r = recusa(
                        Motivo::ViewChildEmFilho,
                        "@ViewChild de #ref repetido em filho onPush",
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
                // `ElementRef(nó)` para o elemento lido assim; o import sai
                // na ordem do texto, depois dos ouvintes e dos pipes.
                let mut lugares = Vec::new();
                onde_esta(nos, &q.referencia, filhos, false, &mut lugares);
                let elemento = matches!(lugares.first(), Some(Lugar::Raiz | Lugar::Projetado));
                // Com `#ref="x"`, o valor é a instância exportada, não o nó.
                let alvo = if !q.por_tipo
                    && elemento
                    && !moldes.contains(&q.referencia)
                    && !referencia_com_valor(nos, &q.referencia)
                    && valor_de_elemento(q, local, resolvedor) == Some(ValorDeElemento::ElementRef)
                {
                    format!("{}ElementRef({alvo})", tardio_q(ELEMENT_REF))
                } else {
                    alvo
                };
                consultas.push(format!("    _ctx.{} = {alvo};", q.propriedade));
            }
            // Na coleta a recusa já veio de `formas_contra_o_template`.
            None if corpo.coletando() => {}
            // Nenhum `#ref` com o nome no template: a única não recebe nada
            // (caso j82).
            None if !q.por_tipo && {
                let mut l = Vec::new();
                onde_esta(nos, &q.referencia, filhos, false, &mut l);
                l.is_empty()
            } => {}
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
    let consultas: Vec<String> = consultas
        .iter()
        .map(|l| resolver_tardios(corpo.imp, l))
        .collect();
    // Os `@HostListener` do componente fecham o `build()`, ligados ao nó
    // raiz (`_writeComponentHostEventListeners`, depois do
    // `writeBuildStatements` em `_generateBuildMethod`). O handler passa
    // pelo mesmo conversor dos eventos do template, e um complexo ganha o
    // próximo `_handleEvent_N`.
    // Quem herda: os dos supertipos também, pelos metadados (supertipos
    // primeiro; o mesmo evento fica com o último, no lugar do primeiro).
    let ouvintes_do_hospedeiro: Vec<crate::componente::Ouvinte> = if c.herda {
        match local.metadados.as_deref() {
            Some(m) if m.fora.is_empty() => {
                if m.ouvintes.iter().any(|o| !evento_nativo(&o.evento)) {
                    corpo.anotar(recusa(
                        Motivo::HostListenerEmComponente,
                        "@HostListener herdado de evento não nativo",
                    ))?;
                }
                m.ouvintes
                    .iter()
                    .map(|o| crate::componente::Ouvinte {
                        evento: o.evento.clone(),
                        handler: format!("{}({})", o.metodo, o.args),
                    })
                    .collect()
            }
            _ => {
                corpo.anotar(recusa(
                    Motivo::HostListenerEmComponente,
                    "@HostListener de componente que herda sem os metadados",
                ))?;
                c.ouvintes.clone()
            }
        }
    } else {
        c.ouvintes.clone()
    };
    let mut hospedeiro = Vec::new();
    for o in &ouvintes_do_hospedeiro {
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
        .cloned()
        .chain((corpo.subscricoes > 0).then(|| {
            let subs: Vec<String> = (0..corpo.subscricoes)
                .map(|k| format!("subscription_{k}"))
                .collect();
            format!("    this.initSubscriptions([{}]);", subs.join(", "))
        }))
        // Os `@HostListener` do componente depois do `initSubscriptions`
        // (`_writeComponentHostEventListeners`, `view_builder.dart:840-852`).
        .chain(hospedeiro.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n");
    let corpo_build = if linhas.is_empty() {
        String::new()
    } else {
        format!("\n{linhas}")
    };
    // `@HostBinding` do componente: o `detectHostChanges(firstCheck)` da
    // visão (`bindAndWriteToRenderer` com `isHtmlElement` falso — daí o
    // `updateClassBindingNonHtml`), com os índices de ligação depois dos do
    // template e as imutáveis antes, no `if (firstCheck)`. O `checkBinding`
    // leva `null, null`: a ligação não tem texto de template.
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
    // Ordem dos campos na classe, como o oficial escreve: ligações de texto,
    // depois os valores anteriores das ligações, depois os elementos.
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
    // O bloco dos ganchos de conteúdo (`_updateContentQueriesMethod`): as
    // consultas de conteúdo dinâmicas, no `afterChildren` de cada nó; as de
    // visão, no `afterNodes` (`compile_view.dart:502-511`), na ordem das
    // consultas; depois os ganchos.
    let mut apos_conteudo = corpo.atualizacoes_de_conteudo.clone();
    let mut de_visao: Vec<(usize, String)> = corpo.atualizacoes_de_visao.clone();
    for d in &corpo.consultas_dinamicas {
        if !d.vista {
            continue;
        }
        // O valor só se monta com as âncoras de todas as visões, depois de
        // emitidas as aninhadas ([`resolver_consultas`]); os qualificadores
        // vão na marca, na ordem em que aparecem no texto.
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
    // A detecção na ordem de `writeChangeDetectionStatements`: entradas de
    // diretivas e filhos, visões aninhadas, ligações de propriedade e texto,
    // visões-filhas. `_ctx` e `firstCheck` só são declarados se alguém os
    // lê (`maybeCachedCtxDeclarationStatement`).
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
        // A consulta dinâmica escreve `_ctx.x = ...` quando é resolvida
        // ([`resolver_consultas`]), depois desta conta.
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
        // Os locais lidos pela detecção (os `#ref`), na ordem do primeiro
        // uso, depois de `_ctx`, como na visão embutida.
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
    // O `injectorGetInternal` vem depois do `build()` e antes da detecção.
    let injetor = resolver_tardios(corpo.imp, &metodo_injetor(&corpo.injetores, &corpo.asset));
    // Os imports da detecção entram agora, depois dos do `build()`.
    let deteccao = resolver_tardios(corpo.imp, &deteccao);
    let deteccao = format!(
        "{}{deteccao}",
        metodo_de_link(ctx.link_de_deteccao, &corpo.ancoras, &corpo.vistas_ligadas)
    );
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
    if !corpo.metodos_i18n.is_empty() && !corpo.metodos_evento.is_empty() {
        let r = recusa(
            Motivo::I18n,
            "@i18n com HTML e handler de evento na mesma visão",
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
    // O `detectHostChanges` vem depois dos handlers (`view.methods`).
    let metodos = metodos + &resolver_tardios(corpo.imp, &host_changes);
    // `corpo` empresta o interner e a tabela de imports; a emissão das
    // visões embutidas precisa dos dois.
    *coleta = corpo.coleta.take();
    drop(corpo);

    imp.sem_alias(ANGULAR);
    // As visões embutidas são escritas aqui, depois das fábricas — e é por
    // isso que os imports delas vêm depois do `angular.dart`.
    let mut embutidas = String::new();
    for espec in especs {
        embutidas.push_str(&emitir_embutida(espec, &ctx, imp, nomes, coleta)?);
    }
    let hosp = imp.alias(HOST_VIEW);
    // Os provedores: campos, instruções antes do componente e o
    // `injectorGetInternal`, com os imports na ordem em que são escritos.
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
    // `ViewContainerRef` no construtor: o elemento hospedeiro ganha um
    // `ViewContainer`, criado no `CompileElement` antes dos provedores (o
    // campo e o import vêm primeiro) e passado ao componente (caso j47).
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
    // O emissor escreve os campos com inicializador (os preguiçosos) antes
    // dos outros (`dart_emitter.dart:198-205`).
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
    let locais: Vec<(crate::diretivas::Token, String)> = no_hospedeiro
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
    // `hasInputs` (`matched_directive_converter.dart:72`) conta os `@Input`
    // herdados, que os metadados coletam como o `_collectInheritableMetadata`.
    let metadados_de_quem_herda = local.metadados.as_deref().filter(|m| m.fora.is_empty());
    let entradas_herdadas =
        c.herda && metadados_de_quem_herda.is_some_and(|m| !m.entradas.is_empty());
    let marca = c.on_push && (!c.entradas.is_empty() || entradas_herdadas);
    if c.on_push && c.entradas.is_empty() && c.herda && metadados_de_quem_herda.is_none() {
        let r = recusa(
            Motivo::NaoEntendido,
            "componente onPush que herda, sem os metadados",
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
    // Os ganchos de quem herda sobem os supertipos (a classe que implementa
    // `OnInit` pode ser a base): vêm dos metadados do programa.
    // Também sem herança: `implements X` com `X extends OnInit` só se vê
    // pelo programa (`allSupertypes`); o texto da classe não basta.
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
                        "ganchos de componente que herda sem os metadados",
                    ),
                )?;
                c.ganchos
            }
        }
    } else {
        c.ganchos
    };
    // Com `@HostBinding`, a hospedeira chama o `detectHostChanges` antes de
    // detectar a visão do componente (`ciclo_de_vida`).
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
    let tag = crate::seletor::Seletor::analisar(seletor)
        .into_iter()
        .find_map(|s| s.elemento)
        .unwrap_or_else(|| seletor.trim().to_string());
    // `_getChangeDetectionCheckMode`: componente `onPush` começa em
    // `checkOnce`.
    let estado = if c.on_push {
        "checkOnce"
    } else {
        "checkAlways"
    };
    // Sem folha, a lista é constante e o estilo não é encapsulado.
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
    this.rootElement = {util}.unsafeCast({html}.document.createElement('{tag}'));{estaticos_no_construtor}
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
    // As cadeias das consultas dinâmicas e o nó que uma visão lê de uma
    // aninhada (o resultado de consulta) só são conhecidos depois de
    // emitidas as aninhadas.
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
        return Err(recusa(Motivo::Ligacao, "#ref sem nó no template"));
    }
    Ok((s, tb_sobrando))
}

/// Os argumentos (`<T, U>`) e a declaração (`<T extends num, U>`) dos
/// parâmetros de tipo de um componente genérico, com o limite qualificado
/// pelo import de quem o declara; vazios num componente não genérico.
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
                            format!("limite de parâmetro de tipo `{l}` sem import"),
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

/// Os `providers:` de um componente estão na parte que o emissor escreve
/// (na hospedeira dele ou no nó de quem o usa)? Senão, o motivo.
pub(crate) fn provedores_escreviveis(meta: &crate::diretivas::Diretiva) -> Result<(), String> {
    use crate::diretivas::{TipoDeToken, Token};
    if let Some(f) = meta.fora.first() {
        return Err(f.clone());
    }
    // O `T` de um token de fora do `dart:core` e sem argumentos de tipo
    // ganharia outro import: ainda sem caso.
    let tipo_conhecido = |t: &TipoDeToken| t.uri == "dart:core" || t.genericos > 0;
    let token_conhecido = |t: &Token| match t {
        Token::Multi { tipo, .. } => tipo_conhecido(tipo),
        // `OpaqueToken<T>` genérico: ainda sem caso.
        Token::Opaco { tipo, .. } => tipo.uri == "dart:core",
        _ => true,
    };
    let com_argumentos = |t: &Token| match t {
        Token::Multi { tipo, .. } | Token::Opaco { tipo, .. } => !tipo.args.is_empty(),
        _ => false,
    };
    for p in meta.provedores.iter().chain(&meta.provedores_de_visao) {
        if com_argumentos(&p.token) || p.tipo.as_ref().is_some_and(|t| !t.args.is_empty()) {
            return Err("token de tipo com argumentos concretos".into());
        }
        let alvo = match &p.fonte {
            crate::diretivas::Fornece::Existente(t) => Some(t),
            _ => None,
        };
        if !token_conhecido(&p.token) || alvo.is_some_and(|t| !token_conhecido(t)) {
            return Err("token de tipo fora do dart:core".into());
        }
    }
    Ok(())
}

/// Os provedores da visão-hospedeira: o nó 0, com o componente e os
/// `providers:` dele (`ProviderElementContext` do elemento hospedeiro).
/// Recusa o que o emissor ainda não escreve.
fn provedores_da_hospedeira(local: &Local) -> Result<crate::diretivas::NoResolvido, Recusa> {
    use crate::diretivas::{Criacao, Expr};
    let prov = |f: &str| recusa(Motivo::Providers, format!("providers: {f}"));
    let meta = local
        .metadados
        .clone()
        .ok_or_else(|| prov("sem os metadados do programa"))?;
    provedores_escreviveis(&meta).map_err(|f| prov(&f))?;
    let r = crate::diretivas::resolver_hospedeira(meta).map_err(prov)?;
    // O componente é o último ansioso: os que ele pede vêm antes, e nada
    // depois dele é criado no `build()` (consulta de conteúdo que acha um
    // provedor o tornaria ansioso: recusada à parte).
    let ultimo_ansioso = r.instancias.iter().rposition(|i| !i.preguicosa);
    if ultimo_ansioso.map(|k| r.instancias[k].campo.as_str()) != Some("component") {
        return Err(prov("provedor ansioso depois do componente"));
    }
    for i in &r.instancias {
        if let Criacao::Multi(itens) = &i.criacao
            && itens.iter().any(Expr::dinamica)
        {
            return Err(prov("item de multi-provedor com dependência de fora"));
        }
    }
    Ok(r)
}

/// O tipo de um `T` inferido (`o.importType`): o do `dart:core` sem prefixo,
/// com o import dele contado.
fn texto_do_tipo(t: &crate::diretivas::TipoDeToken, asset: &str) -> String {
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

/// O token de um provedor visto deste arquivo, pronto para [`expr_do_token`].
fn token_local(t: &crate::diretivas::Token, asset: &str) -> crate::diretivas::Token {
    use crate::diretivas::{TipoDeToken, Token};
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

/// Um `useValue:` como o emissor o escreve.
fn texto_do_valor(v: &crate::diretivas::ValorConst, asset: &str) -> String {
    use crate::diretivas::ValorConst;
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

/// O valor de um provedor (`ProviderSource.build`). Com dependência do
/// injetor, a criação vai embrulhada em `debugInjectorWrap` sob `isDevMode`
/// (`recuo`: a coluna da instrução ou do campo). `vista`: na hospedeira
/// (`None`), o injetor é o dela (`this.injectorGet(T, this.parentIndex)`);
/// num nó de template, o de fora da visão que o lê
/// (`injectFromViewParentInjector`: `(v.parentView!).injectorGet(T,
/// v.parentIndex)`).
fn texto_da_expr(
    e: &crate::diretivas::Expr,
    token: &crate::diretivas::Token,
    asset: &str,
    util: &str,
    recuo: usize,
    vista: Option<&str>,
) -> String {
    use crate::diretivas::Expr;
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

/// O tipo do campo de um provedor da hospedeira (`createProvider`): o `T`
/// inferido; sem ele, o tipo da expressão (`dynamic` quando ela não tem).
fn tipo_do_provedor(i: &crate::diretivas::Instancia, asset: &str) -> Result<String, Recusa> {
    use crate::diretivas::{Criacao, Expr, ValorConst};
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
            "providers: provedor sem expressão",
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
                "providers: provedor que lê outro campo",
            ));
        }
    })
}

/// O campo de um provedor preguiçoso de `providers:` (`late T _X_n_m =
/// valor;`), com imports tardios: o tipo e depois o valor, a ordem em que o
/// oficial escreve (e numera os imports).
fn texto_de_provedor_preguicoso(
    i: &crate::diretivas::Instancia,
    asset: &str,
    vista: Option<&str>,
) -> Result<String, Recusa> {
    use crate::diretivas::Criacao;
    let tipo = tipo_do_provedor(i, asset)?;
    // O `isDevMode` do `debugInjectorWrap` (só com dependência do injetor)
    // com import tardio, na ordem do texto (caso i78).
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
                "provedor preguiçoso que não é expressão",
            ));
        }
    };
    Ok(format!("  late {tipo} {} = {valor};", i.campo))
}

/// O que os provedores põem na hospedeira: os campos (os preguiçosos, com
/// inicializador, antes — `visitDeclareClassStmt` agrupa assim), as
/// instruções do `build()` antes do componente, e os injetáveis do
/// `injectorGetInternal`. Tudo com imports tardios, na ordem do texto.
struct ProvedoresNaHospedeira {
    campos: String,
    antes_do_componente: String,
    injetaveis: Vec<(Vec<crate::diretivas::Token>, String)>,
}

fn escrever_provedores_da_hospedeira(
    r: &crate::diretivas::NoResolvido,
    asset: &str,
    util: &str,
) -> Result<ProvedoresNaHospedeira, Recusa> {
    use crate::diretivas::Criacao;
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

/// `@ContentChildren` do próprio componente na hospedeira: sem conteúdo, a
/// lista recebe `[]` logo depois da construção (`updateQueryAtStartup`), na
/// ordem das consultas; o `@ContentChild` único não recebe nada. Uma
/// consulta que acharia o próprio nó (o componente, um provedor dele ou um
/// tipo do ngdart) é recusada.
fn consultas_da_hospedeira(
    c: &Componente,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
    tokens: &[crate::diretivas::Token],
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
                    recusa(Motivo::ContentChild, "@ContentChild de tipo não resolvido")
                })?;
            let simples = q.alvo.rsplit('.').next().unwrap_or(&q.alvo).to_string();
            let token = crate::diretivas::Token::Classe {
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
                    "@ContentChild que acharia o próprio nó",
                ));
            }
        }
        if q.lista {
            let _ = writeln!(s, "    this.component.{} = [];", q.campo);
        }
    }
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
    locais: &[(crate::diretivas::Token, String)],
) -> Option<String> {
    let x = &c.classe;
    // Um parâmetro que um provedor do próprio nó satisfaz lê o campo dele
    // (`this._Servico_0_5`) e não conta como injeção.
    let do_no = |p: &crate::componente::Parametro| -> Option<String> {
        let tipo = p.tipo.as_deref()?.trim_end_matches('?');
        let simples = tipo.rsplit('.').next()?;
        let uri = resolvedor?.uri_do_tipo(local.caminho, tipo)?;
        let token = crate::diretivas::Token::Classe {
            uri,
            classe: simples.to_string(),
        };
        locais
            .iter()
            .find(|(t, _)| *t == token)
            .map(|(_, c)| format!("this.{c}"))
    };
    // Na hospedeira (`component.type.isHost`), o que o nó não provê vem
    // do injetor, com ou sem `@Host()`; `@Self()` fica no nó (ou `null`,
    // opcional); `@SkipSelf()` pula o nó (`_getDependency`, caso j79).
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
    // O `errors.dart` entra antes dos tipos injetados, como no oficial.
    let erros = injeta.then(|| imp.alias(DI_ERRORS));
    let mut args = Vec::new();
    for p in &c.parametros {
        // `@Attribute`: o elemento hospedeiro não tem atributos (`null`).
        // `@Inject(token)`: o provedor do nó ou o injetor, pelo token.
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
                crate::diretivas::Token::Classe { uri, classe } => {
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
        // `ChangeDetectorRef`: a visão do componente.
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
            return None; // `@Inject(...)`, `@Self`, nomeado: ainda não
        }
        let tipo = tipo_do_token(p.tipo.as_deref()?);
        let simples = tipo.rsplit('.').next()?;
        let uri = resolvedor?.uri_do_tipo(local.caminho, tipo)?;
        // Outro embutido do elemento hospedeiro (`ElementRef`,
        // `TemplateRef`…): não vem do injetor; ainda sem caso.
        if crate::diretivas::embutido_do_elemento(&crate::diretivas::Token::Classe {
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

/// A dependência (dos metadados do programa) de um parâmetro com `@Inject`
/// ou `@Attribute`, pela posição dele entre os não nomeados; `None` sem os
/// metadados, e também com `@Self`/`@Host`/`@SkipSelf` junto (ainda sem
/// caso) ou num embutido do elemento.
fn dependencia_anotada<'m>(
    c: &Componente,
    local: &'m Local,
    p: &crate::componente::Parametro,
) -> Option<&'m crate::diretivas::Dependencia> {
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
        crate::diretivas::Token::Elemento | crate::diretivas::Token::Detector => None,
        t if crate::diretivas::embutido_do_elemento(t) => None,
        _ => Some(d),
    }
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

/// O parâmetro é o `ViewContainerRef` do ngdart (sem anotação)?
fn e_container(
    p: &crate::componente::Parametro,
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

fn e_elemento(tipo: Option<&str>) -> bool {
    matches!(
        tipo.map(|t| t.rsplit('.').next().unwrap_or(t)),
        Some("Element" | "HtmlElement")
    )
}

/// O token de um parâmetro pelo tipo escrito (`_tokenForType`/`_idFor`): a
/// classe, sem o `?` (`@Optional() X? x`) e sem os argumentos de tipo
/// (`MaterialTreeRoot<T>` pede `MaterialTreeRoot`).
pub(crate) fn tipo_do_token(tipo: &str) -> &str {
    tipo.split('<')
        .next()
        .unwrap_or(tipo)
        .trim()
        .trim_end_matches('?')
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
        if dependencia_anotada(c, local, p).is_some() {
            continue;
        }
        if p.outra_anotacao || (p.proprio && !p.opcional) {
            return Some(recusa(
                Motivo::InjecaoAnotada,
                "@Inject/@Attribute/@Self no construtor",
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
        let tipo = tipo_do_token(tipo);
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

    /// `<template ngFor let-x [ngForOf]>` vira a `estrela` do `*ngFor`
    /// equivalente, guardando o intervalo de cada ligação escrita (i84, i86).
    #[test]
    fn template_com_diretiva_vira_estrela() {
        let nos = crate::html::analisar(
            r#"<template ngFor [ngForTrackBy]="f" let-item [ngForOf]="xs" let-i="index"><p></p></template>"#,
        );
        let nos = template_como_container(&nos);
        let [No::Elemento(e)] = nos.as_slice() else {
            panic!("um elemento");
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

    /// O que não é a forma exata continua `<template>` (e é recusado):
    /// diretiva desconhecida, dois atributos, `;` na expressão, evento.
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
                panic!("um elemento");
            };
            assert_eq!(e.nome, "template", "{t}");
            assert!(e.ligacoes_do_molde.is_empty(), "{t}");
        }
        // `;` dentro de texto não atrapalha: a microssintaxe vai decomposta,
        // sem voltar ao texto (caso j75).
        let nos = template_como_container(&crate::html::analisar(
            r#"<template ngFor [ngForOf]="f(';')"></template>"#,
        ));
        let [No::Elemento(e)] = nos.as_slice() else {
            panic!("um elemento");
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
            proprio: false,
            hospedeiro: false,
            pular: false,
            outra_anotacao: false,
        }
    }

    /// Só a leitura com receptor implícito chama o `getLocal` (caso i45).
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
        let esperado = include_str!("../testes/callback_com_injecao.template.dart");
        assert_eq!(saida, esperado.replace("\r\n", "\n"));
    }
}

/// `caminho` (visto de `asset`) é o `.template.dart` do próprio arquivo — o
/// de um filho declarado ao lado, cuja visão não leva prefixo.
/// O prefixo do `XNgCd` de uma diretiva com `@HostBinding`: o import do
/// `.template.dart` dela, ou nada quando a classe é deste mesmo arquivo
/// (caso j45).
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

/// Como o arquivo gerado em `asset` importa a biblioteca `uri`
/// (`getImportModulePath`): relativo no mesmo pacote e pasta, `package:`
/// fora; `dart:` como está.
fn import_de(uri: &str, asset: &str) -> String {
    asset_de_uri(uri, "", Path::new(""))
        .and_then(|alvo| caminho_do_import(asset, &alvo))
        .unwrap_or_else(|| uri.to_string())
}

/// Algum elemento do conteúdo tem o `#ref`?
fn referencia_no_conteudo(nos: &[No], r: &str) -> bool {
    nos.iter().any(|n| {
        let No::Elemento(x) = n else { return false };
        x.referencias.iter().any(|y| y.nome == r) || referencia_no_conteudo(&x.filhos, r)
    })
}
