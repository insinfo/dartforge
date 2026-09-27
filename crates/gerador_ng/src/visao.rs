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
    "package:ngdart/src/core/linker/element_ref.dart",
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
                        return Some(!*interpolou);
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

/// Marca do nó de um `#ref` lido como local: `\u{5}nome\u{6}`. A
/// declaração (`final local_x = this._el_n;`) é pedida por quem lê o local,
/// às vezes antes de o elemento ser criado; a marca é trocada pelo nó quando
/// a visão está completa ([`resolver_refs`]).
const MARCA_DE_REF: char = '\u{5}';
const FIM_DE_REF: char = '\u{6}';

/// Os nomes de `#ref` do template que podem virar local: sem valor,
/// declarados uma vez só, sem membro do componente com o mesmo nome (o
/// `_TypeResolver` do oficial tiparia a leitura pelo membro) e sem `let`
/// que o sombreie.
fn referencias_unicas(nos: &[No], c: &Componente) -> std::collections::HashSet<String> {
    fn todas(nos: &[No], refs: &mut Vec<(String, String)>, lets: &mut Vec<String>) {
        for n in nos {
            if let No::Elemento(e) = n {
                refs.extend(
                    e.referencias
                        .iter()
                        .map(|r| (r.nome.clone(), r.valor.clone())),
                );
                if let Some(estrela) = &e.estrela {
                    let micro = crate::micro::analisar(&estrela.nome, &estrela.valor);
                    lets.extend(micro.locais.into_iter().map(|(nome, _)| nome));
                }
                todas(&e.filhos, refs, lets);
            }
        }
    }
    let (mut refs, mut lets) = (Vec::new(), Vec::new());
    todas(nos, &mut refs, &mut lets);
    refs.iter()
        .filter(|(nome, valor)| {
            valor.is_empty()
                && refs.iter().filter(|(n, _)| n == nome).count() == 1
                && !lets.contains(nome)
                && !c.membros.contains_key(nome.as_str())
                && !c.metodos.contains_key(nome.as_str())
        })
        .map(|(nome, _)| nome.clone())
        .collect()
}

/// Os `#ref` (de `unicos`) que viram local desta visão: declarados num
/// elemento HTML ou de componente filho dela (não dentro de outro `*`, não
/// no conteúdo de um filho) — o nó ou a instância do filho — e lidos por
/// alguma expressão dela ou das visões embutidas nela.
///
/// É o `nameResolver.addLocal(nome, renderNode)` do `CompileElement`: quem
/// lê o local fora do `build()` (detecção, handler, outra visão) promove o
/// nó a campo (`NodeReferenceStorageVisitor`), e o tipo da leitura é
/// `dynamic` (a referência não entra nos `locals` do `AnalyzedClass`).
fn referencias_locais(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    unicos: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    unicos
        .iter()
        .filter(|nome| {
            let mut lugares = Vec::new();
            onde_esta(nos, nome, filhos, false, &mut lugares);
            matches!(
                lugares.as_slice(),
                [Lugar::Raiz | Lugar::NoFilho | Lugar::Projetado | Lugar::NoFilhoProjetado]
            ) && local_citado(nos, nome)
        })
        .cloned()
        .collect()
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
) -> Vec<Recusa> {
    let mut fora = Vec::new();
    let moldes = referencias_de_moldes(nos);
    for consulta in &c.consultas {
        let mut lugares = Vec::new();
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
        if consulta.lista || lugares.len() > 1 {
            let elemento = e_tipo_de_elemento(&consulta.tipo, local, resolvedor);
            let todos_nos = lugares
                .iter()
                .all(|l| matches!(l, Lugar::Raiz | Lugar::Projetado));
            let todos_filhos = lugares
                .iter()
                .all(|l| matches!(l, Lugar::NoFilho | Lugar::NoFilhoProjetado));
            let ok = (elemento && todos_nos) || (!elemento && todos_filhos);
            if !ok {
                fora.push(recusa(
                    Motivo::ViewChildDinamico,
                    "@ViewChildren fora da forma estática",
                ));
            }
            continue;
        }
        let r = match lugares.as_slice() {
            // `isElementType`: campo `Element` (ou subtipo) recebe o nó;
            // qualquer outro, um `ElementRef`. O primeiro caso é o estático,
            // que sai no `build()`.
            [Lugar::Raiz | Lugar::Projetado]
                if e_tipo_de_elemento(&consulta.tipo, local, resolvedor) =>
            {
                continue;
            }
            [Lugar::Raiz | Lugar::Projetado] => recusa(
                Motivo::ViewChildEmFilho,
                "@ViewChild de elemento com tipo que não é Element",
            ),
            // A instância do filho: o campo tem de ser do tipo dele (não um
            // `Element`) e o filho não pode ser `onPush`, que registra o
            // `ChangeDetectorRef` da consulta (`queryChangeDetectorRefs`).
            [Lugar::NoFilho | Lugar::NoFilhoProjetado] => {
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

/// A consulta de visão cujo único resultado está numa visão embutida filha
/// direta da do componente (`*` na raiz, fora do conteúdo projetado): um
/// elemento HTML com o `#ref` (uma vez só no template), na raiz dessa
/// visão, e campo `Element`. É a forma de `mapNestedViewsWithSingleResult`
/// com um nível, a mais comum (`@ViewChild` dentro de `*ngIf`); as outras
/// (vários resultados, dois níveis, filho, mistura com estáticos) ainda
/// não.
fn consulta_em_embutida(
    nos: &[No],
    consulta: &crate::componente::Consulta,
    filhos: &std::collections::HashMap<String, Filho>,
    local: &Local,
    resolvedor: Option<&dyn Resolucao>,
) -> bool {
    fn estrelas_da_raiz<'n>(
        nos: &'n [No],
        filhos: &std::collections::HashMap<String, Filho>,
        saida: &mut Vec<&'n crate::html::Elemento>,
    ) {
        for n in nos {
            let No::Elemento(e) = n else { continue };
            if e.estrela.is_some() {
                saida.push(e);
            } else if (!filhos.contains_key(&e.nome) && dom::tag_html(&e.nome))
                || e.nome == "ng-container"
            {
                estrelas_da_raiz(&e.filhos, filhos, saida);
            }
        }
    }
    if consulta.por_tipo || !e_tipo_de_elemento(&consulta.tipo, local, resolvedor) {
        return false;
    }
    let nome = consulta.referencia.as_str();
    let mut todos = Vec::new();
    onde_esta(nos, nome, filhos, false, &mut todos);
    if todos.len() != 1 {
        return false;
    }
    let mut estrelas = Vec::new();
    estrelas_da_raiz(nos, filhos, &mut estrelas);
    let com_ref: Vec<&&crate::html::Elemento> = estrelas
        .iter()
        .filter(|e| {
            let mut l = Vec::new();
            onde_esta(
                std::slice::from_ref(&No::Elemento((**e).clone())),
                nome,
                filhos,
                false,
                &mut l,
            );
            !l.is_empty()
        })
        .collect();
    let [e] = com_ref.as_slice() else {
        return false;
    };
    let mut sem = (**e).clone();
    sem.estrela = None;
    let mut l = Vec::new();
    onde_esta(&[No::Elemento(sem)], nome, filhos, false, &mut l);
    l == [Lugar::Raiz]
}

/// Uma consulta de visão atualizada na detecção (`createDynamicUpdates`):
/// o campo "sujo", a âncora e a classe da visão embutida do resultado.
#[derive(Debug, Clone)]
struct ConsultaDinamica {
    indice: usize,
    propriedade: String,
    referencia: String,
    lista: bool,
    /// `_viewQuery_ref_N_isDirty`.
    campo: String,
    /// (âncora `_appEl_n`, classe `_ViewX1`), quando o `*` foi visto.
    origem: Option<(String, String)>,
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
                } else if let (true, "template", Some((estrela, ligacoes))) =
                    (limpo, e.nome.as_str(), molde_com_diretiva(&e))
                {
                    e.estrela = Some(estrela);
                    e.ligacoes_do_molde = ligacoes;
                    e.propriedades.clear();
                    e.atributos.clear();
                    e.nome = "ng-container".into();
                } else if e.nome == "template"
                    && e.estrela.is_none()
                    && e.atributos.is_empty()
                    && e.propriedades.is_empty()
                    && e.eventos.is_empty()
                    && e.bananas.is_empty()
                    && e.anotacoes.is_empty()
                    && e.referencias.len() <= 1
                    && e.referencias.iter().all(|r| r.valor.is_empty())
                {
                    // `<template>` só com (no máximo) um `#ref` e sem
                    // diretiva: âncora, `ViewContainer` e `TemplateRef`, com o
                    // conteúdo numa visão embutida ([`Corpo::molde`]).
                    e.estrela = Some(crate::html::Ligacao {
                        nome: MARCA_DE_MOLDE.into(),
                        valor: String::new(),
                        inicio: 0,
                        fim: 0,
                    });
                }
                No::Elemento(e)
            }
            outro => outro.clone(),
        })
        .collect()
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
/// Só a forma sem ambiguidade: exatamente um atributo sem valor com o nome
/// de uma diretiva estrutural conhecida, `let-x` (com ou sem valor) e ao
/// menos uma ligação `[dirX]` com o prefixo da diretiva e sem `;` na
/// expressão (que a microssintaxe separaria). O resto continua `<template>`
/// e é recusado.
fn molde_com_diretiva(
    e: &crate::html::Elemento,
) -> Option<(crate::html::Ligacao, Vec<crate::html::Ligacao>)> {
    let (lets, outros): (Vec<_>, Vec<_>) =
        e.atributos.iter().partition(|a| a.nome.starts_with("let-"));
    let [dir] = outros.as_slice() else {
        return None;
    };
    if !dir.valor.is_empty() || Estrutural::conhecida(&dir.nome).is_none() {
        return None;
    }
    if e.propriedades.is_empty() {
        return None;
    }
    let mut partes = Vec::new();
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
    }
    for p in &e.propriedades {
        let sufixo = p.nome.strip_prefix(dir.nome.as_str())?;
        let mut cs = sufixo.chars();
        let primeira = cs.next()?;
        if !primeira.is_ascii_uppercase() || p.valor.contains(';') || p.valor.trim().is_empty() {
            return None;
        }
        partes.push(format!(
            "{}{}: {}",
            primeira.to_ascii_lowercase(),
            cs.as_str(),
            p.valor.trim()
        ));
    }
    Some((
        crate::html::Ligacao {
            nome: dir.nome.clone(),
            valor: partes.join("; "),
            inicio: dir.inicio,
            fim: dir.fim,
        },
        e.propriedades.clone(),
    ))
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
    let tem_ref = |e: &crate::html::Elemento| e.referencias.iter().any(|r| r.nome == nome);
    onde_casa(nos, &tem_ref, filhos, em_filho, saida);
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
        } else if !dom::tag_html(&e.nome) {
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
    /// `@HostBinding`: quem o usa chama `detectHostChanges(firstCheck)`
    /// antes de detectar a visão dele (`bindDirectiveHostProps`).
    pub hospedeiro: bool,
    /// `@Output`s (nome no template, membro), na ordem do mapa `outputs`.
    pub saidas: Vec<(String, String)>,
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
    /// Serviço de fora da visão: `injectorGet` pela visão de cima
    /// (`injectFromViewParentInjector`), `injectorGetOptional` com
    /// `@Optional()`.
    Servico {
        uri: String,
        classe: String,
        opcional: bool,
    },
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
        let dentro = self.relativo.strip_prefix("lib/")?;
        let dir = dentro.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let caminho = if dir.is_empty() {
            url.to_string()
        } else {
            format!("{dir}/{url}")
        };
        let sufixo = if shim { ".shim.dart" } else { ".dart" };
        Some(format!("package:{}/{caminho}{sufixo}", self.pacote))
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
    /// Os `#ref` locais das visões ancestrais (ver [`EspecEmbutida`]).
    refs_ancestrais: Vec<(String, String, u32)>,
    /// Na visão do componente: as consultas de visão atualizadas na
    /// detecção ([`consulta_em_embutida`]).
    consultas_dinamicas: Vec<ConsultaDinamica>,
    /// Na embutida: os `#ref` resultado de consulta da visão do componente
    /// (o nó vira campo) e o campo "sujo" de cada uma.
    refs_consultados: Vec<(String, String)>,
    /// Cada `#ref` visto, com a expressão do nó (`_el_3` ou `this._el_3`) —
    /// é o valor que o `@ViewChild` recebe.
    refs: std::collections::HashMap<String, String>,
    /// Os mesmos, em ordem de documento e com repetição: os resultados de
    /// um `@ViewChildren` (`addQueryResult`).
    refs_em_ordem: Vec<(String, String)>,
    /// A tag do elemento cujas ligações estão sendo escritas: o contexto de
    /// segurança de uma propriedade depende dela ([`saneador`]).
    tag_atual: String,
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
    /// Os provedores injetáveis dos elementos acima do nó, o mais longe
    /// primeiro: (token, campo, e a visão ancestral que o tem — classe e
    /// quantos `parentView` até ela — ou nada, se é desta).
    acima: Vec<(crate::diretivas::Token, String, Option<(String, u32)>)>,
    /// Quantos elementos de componente há acima (provedores que o emissor
    /// não modela).
    componentes_acima: u32,
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
    /// As visões-filhas de componente com `@HostBinding`.
    vistas_hospedeiras: std::collections::HashSet<String>,
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
    /// (token, campo, componente `onPush`).
    provedores: Vec<(crate::diretivas::Token, String, bool)>,
    /// O nó como o `build()` o lê (`_el_3` ou `this._el_3`), para o
    /// `read: HtmlElement`.
    elemento: String,
}

impl Corpo<'_> {
    /// A classe desta visão, para o `unsafeCast` de quem a lê de baixo.
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
        self.acima
            .iter()
            .rev()
            .map(|(t, campo, v)| crate::diretivas::ProvedorAcima {
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
        for nome in &refs {
            self.locais.insert(
                nome.clone(),
                crate::expr::Local {
                    dart: format!("local_{nome}"),
                    tipo: "dynamic".into(),
                    escopo: None,
                },
            );
            self.decl_locais.insert(
                nome.clone(),
                Ok(format!(
                    "final local_{nome} = {MARCA_DE_REF}{nome}{FIM_DE_REF};"
                )),
            );
        }
        self.refs_locais = refs;
    }

    /// O campo de uma mensagem `@i18n` sem HTML (`createI18nMessage`): um
    /// `static final String _message_N = Intl.message(texto, desc: ..)`,
    /// reaproveitado quando texto e metadados se repetem.
    fn mensagem(&mut self, texto: &str, m: &MetaI18n) -> Result<String, Recusa> {
        let Some(intl) = self.intl.clone() else {
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
            // Com contexto de segurança o valor passa pelo saneador do
            // contexto, achado pelo nome de propriedade mapeado
            // (`securityContext(tag, getMappedPropName(x))` em
            // `createElementPropertyAst`); o atributo escrito fica como está.
            let saneado = match saneador(
                &self.tag_atual.to_ascii_lowercase(),
                propriedade_mapeada(attr),
            ) {
                Some(f) => {
                    let s = tardio(SAFE_HTML);
                    format!("{s}.{f}({valor})")
                }
                None => valor.to_string(),
            };
            let valor = saneado.as_str();
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
            // `getMappedPropName`: `innerHtml` é `innerHTML`; os outros
            // nomes do mapa têm forma própria (`class`, `tabIndex`).
            let prop = if l.nome == "innerHtml" {
                "innerHTML"
            } else {
                l.nome.as_str()
            };
            if matches!(prop, "readonly" | "tabindex" | "tabIndex") {
                return Err(recusa(
                    Motivo::Ligacao,
                    "[propriedade] renomeada pelo esquema",
                ));
            }
            // `_sanitizedValue`: o valor passa pelo saneador do contexto
            // (`[style]` é `*|style`: `sanitizeStyle`, caso i92).
            match saneador(&self.tag_atual.to_ascii_lowercase(), prop) {
                Some(f) => {
                    let s = tardio(SAFE_HTML);
                    format!("{dom}.setProperty({alvo}, '{prop}', {s}.{f}({valor}))")
                }
                None => format!("{dom}.setProperty({alvo}, '{prop}', {valor})"),
            }
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
        // Os provedores do nó: o componente primeiro, depois as diretivas.
        let com_provedores = !extras.is_empty()
            || filho
                .metadados
                .as_ref()
                .is_some_and(|m| !m.provedores.is_empty());
        let no_resolvido =
            if com_provedores {
                let meta = filho
                    .metadados
                    .clone()
                    .ok_or_else(|| em_filho("filho com providers ou diretiva sem metadados"))?;
                // As dependências do filho são resolvidas à parte
                // (`construcao_do_filho`); aqui só pesa a posição dele.
                let mut so_provedores = (*meta).clone();
                so_provedores.dependencias.clear();
                let mut casadas = vec![std::sync::Arc::new(so_provedores)];
                casadas.extend(extras.iter().cloned());
                // Serviço que o próprio nó provê: o filho seria criado depois
                // dele, lendo o campo, e não pela visão de cima
                // ([`Self::construcao_do_filho`]).
                let do_no = |uri: &str, classe: &str| {
                    casadas.iter().flat_map(|d| &d.provedores).any(|p| {
                        matches!(&p.token, crate::diretivas::Token::Classe { uri: u, classe: c }
                        if u == uri && c == classe)
                    })
                };
                if filho.parametros.iter().any(
                    |p| matches!(p, Injetado::Servico { uri, classe, .. } if do_no(uri, classe)),
                ) {
                    return Err(em_filho("filho que injeta um provedor do próprio nó"));
                }
                if let Err(f) = provedores_escreviveis(&meta) {
                    return Err(em_filho(&format!("filho com providers: {f}")));
                }
                Some(casadas)
            } else {
                None
            };
        // `#ref` no filho vale a instância (o campo dela): só sem valor, e
        // lido por expressão só da própria visão ([`referencias_locais`]).
        for r in &e.referencias {
            if !r.valor.is_empty() {
                return Err(em_filho("#ref com valor no filho"));
            }
            if !self.refs_livres.contains(&r.nome) && !self.refs_locais.contains(&r.nome) {
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
            if a.nome.eq_ignore_ascii_case("tabindex") {
                return Err(em_filho("tabindex no filho"));
            }
            if a.nome == "style" && e.propriedades.iter().any(|p| p.nome.starts_with("style")) {
                return Err(recusa(
                    Motivo::EstiloEmLinha,
                    "style=\"...\" com [style.x] no mesmo nó",
                ));
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
        // A `[x]` que só uma diretiva do nó recebe não é do filho.
        ligadas.extend(
            e.propriedades
                .iter()
                .filter(|l| filho.entrada(&l.nome).is_some() || !consome_entrada(&extras, &l.nome))
                .map(|l| (l, false)),
        );
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
        // Filho do mesmo arquivo: a visão dele está ao lado, sem import.
        let vt = if e_o_proprio_template(&self.asset, &cam_template) {
            String::new()
        } else {
            self.imp.q(&cam_template)
        };
        let vd = self.imp.alias(&cam_dart);
        let classe = &filho.classe;
        let campo_vista = format!("_compView_{n}");
        let campo_inst = format!("_{classe}_{n}_5");
        self.campos_filho
            .push(format!("  late final {vt}View{classe}0 {campo_vista};"));
        self.campos_filho
            .push(format!("  late final {vd}.{classe} {campo_inst};"));
        self.vistas_filhas.push(campo_vista.clone());
        if filho.hospedeiro {
            self.vistas_hospedeiras.insert(campo_vista.clone());
            self.usa_primeira_checagem = true;
        }
        self.linhas.push(format!(
            "    this.{campo_vista} = {vt}View{classe}0(this, {n});"
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
                filho.on_push,
            )],
            elemento: format!("_el_{n}"),
        });
        for r in &e.referencias {
            self.refs
                .insert(r.nome.clone(), format!("this.{campo_inst}"));
            self.refs_em_ordem
                .push((r.nome.clone(), format!("this.{campo_inst}")));
        }
        // O resultado de um `@ViewChild(Tipo)` ([`chave_de_tipo`]).
        let chave = chave_de_tipo(&filho.uri_dart, &filho.classe);
        self.refs_em_ordem
            .push((chave.clone(), format!("this.{campo_inst}")));
        if filho.on_push {
            self.detectores.insert(chave, campo_vista.clone());
        }
        for r in &e.referencias {
            if filho.on_push {
                self.detectores.insert(r.nome.clone(), campo_vista.clone());
            }
        }
        self.entradas_do_filho(&ligadas, filho, &campo_inst, &campo_vista)?;
        let mut sem_as_da_diretiva = e.clone();
        sem_as_da_diretiva
            .eventos
            .retain(|l| !consome_saida(&extras, &l.nome));
        self.saidas_do_filho(&sem_as_da_diretiva, filho, n, &campo_inst)?;
        // Os outros provedores do nó (acessor de valor por `providers:` do
        // filho, `NgModel`): campos depois da instância, entradas e saídas
        // depois das do filho (`transformedDirectiveAsts`).
        let mut injetor_do_filho = None;
        let antes_acima = self.acima.len();
        if let Some(casadas) = &no_resolvido {
            let provedores = self.provedores_acima();
            let acima = crate::diretivas::Acima {
                provedores: &provedores,
                incerto: self.componentes_acima > 0,
            };
            let r = crate::diretivas::resolver_no_do_filho(casadas, n, Some(acima))
                .map_err(|f| recusa(Motivo::DiretivaPorSeletor, f))?;
            let Some(primeira) = r.instancias.first() else {
                return Err(em_filho("nó do filho sem a instância do filho"));
            };
            if primeira.campo != campo_inst {
                return Err(em_filho("provedor do nó antes do filho"));
            }
            // Provedor preguiçoso do filho pedido por um nó do conteúdo: o
            // oficial o cria no `build()`, logo depois do filho (caso i76).
            // Ainda sem tradução.
            let preguicosos: Vec<crate::diretivas::Token> = r.instancias[1..]
                .iter()
                .filter(|i| {
                    i.preguicosa
                        && matches!(
                            i.criacao,
                            crate::diretivas::Criacao::Expressao(_)
                                | crate::diretivas::Criacao::Multi(_)
                        )
                })
                .flat_map(|i| std::iter::once(i.token.clone()).chain(i.apelidos.iter().cloned()))
                .collect();
            if !preguicosos.is_empty()
                && pede_algum(&e.filhos, self.filhos, self.usadas, &preguicosos)
            {
                return Err(em_filho("provedor do filho pedido por um nó do conteúdo"));
            }
            let resto = crate::diretivas::NoResolvido {
                instancias: r.instancias[1..].to_vec(),
                diretivas: r.diretivas[1..].to_vec(),
            };
            self.diretivas_do_no(e, &resto, &format!("_el_{n}"), &props_dir, &eventos_dir)?;
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
                for t in &i.injetavel_por {
                    self.acima.push((t.clone(), i.leitura.clone(), None));
                }
            }
            for (d, c) in &resto.diretivas {
                if d.ganchos.on_destroy {
                    self.destruir.push(format!("    this.{c}.ngOnDestroy();"));
                }
            }
            let mut acima_reg = self.pilha.clone();
            acima_reg.push((n, true));
            // O token do filho já está no registro dele; os apelidos dele,
            // aqui.
            let provedores = r
                .instancias
                .iter()
                .enumerate()
                .flat_map(|(k, i)| {
                    (k > 0)
                        .then_some(&i.token)
                        .into_iter()
                        .chain(&i.apelidos)
                        .map(|t| (t.clone(), i.leitura.clone(), false))
                })
                .collect();
            self.registros.push(Registro {
                acima: acima_reg,
                provedores,
                elemento: format!("_el_{n}"),
            });
        }
        if filho.projecoes.is_empty() {
            if !e.filhos.is_empty() {
                return Err(recusa(
                    Motivo::Projecao,
                    "conteúdo em filho que não projeta",
                ));
            }
            self.consultas_do_filho(e, filho, &campo_inst, n)?;
            self.depois_dos_filhos(filho, &campo_inst);
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
            // Elemento que nenhuma projeção recebe é criado e descartado
            // (`ngContentIndex` nulo: `addContentNode` não é chamado) — as
            // diretivas dele existem e entram nas consultas.
            let descartado = match (indice, no) {
                (Some(_), _) => false,
                (None, No::Elemento(x)) if x.estrela.is_none() => true,
                (None, _) => {
                    r = Err(recusa(
                        Motivo::Projecao,
                        "conteúdo que nenhuma projeção recebe",
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
        r?;
        self.consultas_do_filho(e, filho, &campo_inst, n)?;
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
        self.acima.truncate(antes_acima);
        // `ProviderNode(nodeIndex, nodeIndex + childNodeCount)`.
        if let Some(i) = injetor_do_filho {
            self.injetores[i].1 = self.proximo - 1;
        }
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
                    // Um elemento acima provê o serviço: o oficial o leria
                    // de lá (`_getDependency`), não do injetor de fora.
                    let token = crate::diretivas::Token::Classe {
                        uri: uri.clone(),
                        classe: tipo.clone(),
                    };
                    if self.acima.iter().any(|(t, _, _)| *t == token) {
                        return Err(em_filho("filho que injeta um provedor de um nó acima"));
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
        let fora = |f: &str| recusa(Motivo::LigacaoEmFilho, f);
        for q in &filho.consultas {
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
                return Err(fora(
                    "@ContentChild do filho com resultado em visão embutida",
                ));
            }
            let token = crate::diretivas::Token::Classe {
                uri: uri.clone(),
                classe: classe.clone(),
            };
            let mut valores = Vec::new();
            for r in &self.registros {
                let Some(pos) = r.acima.iter().position(|(k, _)| *k == n) else {
                    continue;
                };
                let distancia = r.acima[pos + 1..].iter().filter(|(_, d)| *d).count();
                if !q.descendentes && distancia > 1 {
                    continue;
                }
                for (t, campo, on_push) in &r.provedores {
                    if *t != token {
                        continue;
                    }
                    if *on_push {
                        return Err(fora("@ContentChild do filho que acha componente onPush"));
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
            let dentro = dentro || x.estrela.is_some();
            let mut sem = x.clone();
            sem.estrela = None;
            let casa = self
                .filhos
                .get(&x.nome)
                .is_some_and(|f| f.uri_dart == uri && f.classe == classe)
                || diretivas_casadas(self.usadas, &sem)
                    .iter()
                    .any(|d| d.uri == uri && d.classe == classe);
            (dentro && casa) || self.classe_em_embutida(&x.filhos, uri, classe, dentro)
        })
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
        let mut micro = crate::micro::analisar(&estrela.nome, &estrela.valor);
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
            "    var _TemplateRef_{n}_8 = {tr}TemplateRef(this._appEl_{n}, {nome_fabrica});"
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

    /// `<template>` escrito à mão, sem diretiva: âncora, `ViewContainer` e
    /// `TemplateRef` (o 7 da tabela do nó: não há diretiva), com o conteúdo
    /// numa visão embutida. Com `#ref` o `TemplateRef` é campo e o nome o
    /// lê; sem, um local sem uso. Ninguém cria a visão: o `ViewContainer`
    /// fica fora da detecção e da destruição.
    fn molde(&mut self, e: &crate::html::Elemento, pai: &str) -> Result<(), Recusa> {
        if self.pai_projetado.is_some() {
            return Err(recusa(Motivo::Projecao, "<template> no conteúdo projetado"));
        }
        let desc = crate::seletor::Elemento::do_template(e);
        if let Some(u) = self
            .usadas
            .iter()
            .find(|u| crate::seletor::casa_algum(&u.seletores, &desc))
        {
            return Err(recusa(
                Motivo::DiretivaPorSeletor,
                format!("diretiva {} em <template>", u.classe),
            ));
        }
        // `@ViewChild` com o resultado no `<template>` ou dentro dele: o
        // valor seria o `TemplateRef` ou uma visão que ninguém cria.
        for q in &self.consultas_dinamicas {
            let mut l = Vec::new();
            onde_esta(
                std::slice::from_ref(&No::Elemento(e.clone())),
                &q.referencia,
                self.filhos,
                false,
                &mut l,
            );
            if !l.is_empty() {
                return Err(recusa(Motivo::Ligacao, "@ViewChild em <template>"));
            }
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
        if referencia.is_some() {
            self.campos_filho
                .push(format!("  late final {tr}TemplateRef _TemplateRef_{n}_7;"));
        }
        let pai_indice = if pai.is_empty() {
            self.raizes.push(format!("this._appEl_{n}"));
            self.linhas
                .push(format!("    final _anchor_{n} = {dom}.createAnchor();"));
            "null".to_string()
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
                    "    this._TemplateRef_{n}_7 = {tr}TemplateRef(this._appEl_{n}, {nome_fabrica});"
                ));
                let leitura = format!("this._TemplateRef_{n}_7");
                self.refs.insert(nome.clone(), leitura.clone());
                self.refs_em_ordem.push((nome.clone(), leitura));
            }
            None => self.linhas.push(format!(
                "    var _TemplateRef_{n}_7 = {tr}TemplateRef(this._appEl_{n}, {nome_fabrica});"
            )),
        }
        let locais = self.locais.clone();
        self.empilhar_embutida(
            e,
            e.filhos.clone(),
            n,
            indice,
            nome_fabrica,
            pai,
            locais,
            crate::micro::Micro::default(),
        );
        Ok(())
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
        // Consulta de visão com o resultado aqui dentro: a embutida marca o
        // campo sujo no `dirtyParentQueriesInternal`.
        let mut refs_consultados = Vec::new();
        for q in &mut self.consultas_dinamicas {
            let mut l = Vec::new();
            onde_esta(
                std::slice::from_ref(&No::Elemento(e.clone())),
                &q.referencia,
                self.filhos,
                false,
                &mut l,
            );
            if l.is_empty() || q.origem.is_some() {
                continue;
            }
            q.origem = Some((
                format!("_appEl_{n}"),
                format!("_{}{indice}", self.classe_da_visao),
            ));
            refs_consultados.push((q.referencia.clone(), q.campo.clone()));
        }
        self.embutidas.push(EspecEmbutida {
            indice,
            refs_consultados,
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
            componentes_acima: self.componentes_acima,
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
            // No elemento de um componente (`<li-select [(ngModel)]>`), sem
            // `@HostListener`: os ouvintes de diretiva ali ainda não saem.
            let pendencia = match &u.diretiva {
                None => Some("sem metadados".to_string()),
                Some(d) => d.pendencia().or_else(|| {
                    (filho.is_some()
                        && (!d.ouvintes.is_empty() || !d.ligacoes_do_hospedeiro.is_empty()))
                    .then(|| "@HostListener/@HostBinding no elemento de um componente".to_string())
                }),
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
        // Sem pai: raiz de uma visão embutida (o `*` num `<ng-container>`),
        // que é o próprio `TextBinding.element`; texto solto entregue a um
        // filho (`createText`) ainda não tem caso no corpus.
        let raiz_embutida = pai.is_empty() && self.embutida && self.pai_projetado.is_none();
        if pai.is_empty() && !raiz_embutida {
            return Err(recusa(Motivo::Projecao, "interpolação projetada solta"));
        }
        let convertida = self.converter(expr, Motivo::Interpolacao)?;
        if raiz_embutida && convertida.imutavel {
            return Err(recusa(
                Motivo::Interpolacao,
                "interpolação imutável na raiz de visão embutida",
            ));
        }
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
        if raiz_embutida {
            self.raizes.push(format!("this._textBinding_{n}.element"));
        } else {
            self.linhas
                .push(format!("    {pai}.append(this._textBinding_{n}.element);"));
        }
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
                // Sem pai: raiz de visão embutida (`createText`, e o nó
                // entra nas raízes); no conteúdo projetado, ainda não.
                let raiz_embutida = pai.is_empty() && self.embutida && self.pai_projetado.is_none();
                if pai.is_empty() && !raiz_embutida {
                    return Err(recusa(Motivo::Projecao, "texto projetado solto"));
                }
                let n = self.proximo;
                self.proximo += 1;
                let dom = self.dom();
                let texto = literal(t);
                if raiz_embutida {
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
                // projetado, e no `*` vai para o `<template>`.
                if !e.anotacoes.is_empty()
                    && (e.estrela.is_some() || self.filhos.contains_key(&e.nome))
                {
                    return Err(recusa(Motivo::I18n, "@i18n em componente filho ou em `*`"));
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
                self.linhas.push(format!("    this.project({pai}, {i});"));
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
        if let Some(r) = e.referencias.iter().find(|r| {
            !r.valor.is_empty()
                || !(self.refs_livres.contains(&r.nome)
                    || self.refs_locais.contains(&r.nome)
                    || self.refs_consultados.iter().any(|(n, _)| *n == r.nome))
        }) {
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
            let provedores = self.provedores_acima();
            let acima = crate::diretivas::Acima {
                provedores: &provedores,
                incerto: self.componentes_acima > 0,
            };
            match crate::diretivas::resolver(&casadas, n, Some(acima)) {
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
        let lido_como_local = e.referencias.iter().any(|r| {
            self.refs_locais.contains(&r.nome)
                || self.refs_consultados.iter().any(|(n, _)| *n == r.nome)
        });
        let alvo = if !liga_no_elemento(e, &casadas) && !lido_como_local {
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
            self.refs_em_ordem.push((r.nome.clone(), alvo.clone()));
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
                self.linhas
                    .push(format!("    this.updateChildClass({alvo}, {valor});"));
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
            } else if a.nome == "style"
                && e.propriedades.iter().any(|p| p.nome.starts_with("style"))
            {
                // `style` escrito é um atributo como outro qualquer
                // (`setAttribute`); junto de `[style.x]` a ordem das duas
                // escritas ainda não tem caso.
                self.anotar(recusa(
                    Motivo::EstiloEmLinha,
                    "style=\"...\" com [style.x] no mesmo nó",
                ))?;
            } else {
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
            // (`_collectHostListeners`), agrupados por evento na ordem em
            // que cada um aparece primeiro: dois ouvintes do mesmo evento
            // viram um `_handleEvent_N` que chama os dois.
            let mut grupos: Vec<(String, Vec<(String, crate::diretivas::Ouvinte)>)> = Vec::new();
            for d in &casadas {
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
            for (evento, lista) in &grupos {
                let h = match lista.as_slice() {
                    [(campo, o)] => self.handler_de_hospedeiro(campo, o),
                    varios => self.handler_de_grupo(varios),
                };
                self.ouvinte(evento, &alvo, &h);
            }
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
                        .map(|t| (t.clone(), i.leitura.clone(), false))
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
                for t in &i.injetavel_por {
                    self.acima.push((t.clone(), i.leitura.clone(), None));
                }
            }
        }
        self.pilha.push((n, resolvido.is_some()));
        let r = match &i18n_filhos {
            Some(m) => self.filhos_i18n(e, m, &alvo),
            None => self.nos(&e.filhos, &alvo),
        };
        self.pilha.pop();
        self.acima.truncate(antes_acima);
        // `ngOnDestroy` das diretivas do nó, depois dos filhos
        // (`bindDirectiveAfterChildrenCallbacks`), na ordem delas.
        if let Some(res) = &resolvido {
            for (d, campo) in &res.diretivas {
                if d.ganchos.on_destroy {
                    self.destruir
                        .push(format!("    this.{campo}.ngOnDestroy();"));
                }
            }
        }
        // `ProviderNode(nodeIndex, nodeIndex + childNodeCount)`.
        if let Some(i) = injetor {
            self.injetores[i].1 = self.proximo - 1;
        }
        r
    }

    /// Os filhos de um elemento com `@i18n`: um texto só vira uma mensagem
    /// (`internationalize`, `_textMessage`) e um nó de texto com ela. Com
    /// HTML dentro a mensagem é um método estático com argumentos, forma
    /// ainda recusada.
    fn filhos_i18n(
        &mut self,
        e: &crate::html::Elemento,
        m: &MetaI18n,
        alvo: &str,
    ) -> Result<(), Recusa> {
        let [No::Texto(t)] = e.filhos.as_slice() else {
            return self.anotar(recusa(Motivo::I18n, "mensagem @i18n com HTML ou vazia"));
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
        use crate::diretivas::{Argumento, Criacao, Token};
        for inst in &r.instancias {
            if inst.preguicosa && self.tb.is_some() {
                return Err(recusa(
                    Motivo::DiretivaPorSeletor,
                    "provedor preguiçoso com ligação de texto na visão",
                ));
            }
            // Um `providers:` do filho que não é apelido: escrito como na
            // hospedeira (`ProviderSource.build`), só preguiçoso — o que o
            // filho ou uma diretiva do nó pede sairia no `build()`, forma
            // ainda sem caso.
            if let Criacao::Expressao(_) | Criacao::Multi(_) = &inst.criacao {
                if !inst.preguicosa {
                    return Err(recusa(
                        Motivo::LigacaoEmFilho,
                        "provedor do filho pedido no próprio nó",
                    ));
                }
                let texto = texto_de_provedor_preguicoso(inst, &self.asset)?;
                let texto = resolver_tardios(self.imp, &texto);
                self.campos.push(texto);
                continue;
            }
            let tipo = match (&inst.criacao, &inst.token) {
                (Criacao::Diretiva { diretiva, .. }, _)
                    if !diretiva.ligacoes_do_hospedeiro.is_empty() =>
                {
                    let tpl = diretiva.uri.replace(".dart", ".template.dart");
                    format!(
                        "{}{}NgCd",
                        self.imp.q(&import_de(&tpl, &self.asset)),
                        diretiva.classe
                    )
                }
                (Criacao::Diretiva { diretiva, .. }, _) => {
                    format!(
                        "{}{}",
                        self.imp.q(&import_de(&diretiva.uri, &self.asset)),
                        diretiva.classe
                    )
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
                    let args: Vec<String> = args
                        .iter()
                        .map(|a| match a {
                            Argumento::Elemento => alvo.to_string(),
                            Argumento::Detector => "this".to_string(),
                            Argumento::Nulo => "null".to_string(),
                            Argumento::Campo(c) => format!("this.{c}"),
                            Argumento::Acima(leitura) => leitura.clone(),
                        })
                        .collect();
                    // Com `@HostBinding`, o `XNgCd` do `.template.dart` da
                    // diretiva embrulha a instância.
                    let cd = if diretiva.ligacoes_do_hospedeiro.is_empty() {
                        None
                    } else {
                        let tpl = diretiva.uri.replace(".dart", ".template.dart");
                        Some(self.imp.q(&import_de(&tpl, &self.asset)))
                    };
                    let criacao = format!(
                        "{}{}({})",
                        self.imp.q(&import_de(&diretiva.uri, &self.asset)),
                        diretiva.classe,
                        args.join(", ")
                    );
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
            if inst.preguicosa {
                // `late` sem `final`, com o valor no inicializador.
                self.campos
                    .push(format!("  late {tipo} {} = {valor};", inst.campo));
            } else {
                self.campos_filho
                    .push(format!("  late final {tipo} {};", inst.campo));
                self.linhas
                    .push(format!("    this.{} = {valor};", inst.campo));
            }
        }
        // `registerDirectives`: as instâncias das diretivas, na ordem.
        // (Só o componente no nó, sem diretiva: nada a registrar.)
        if !r.diretivas.is_empty() {
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
        }
        // `detectHostChanges` das diretivas com `@HostBinding`, com as
        // ligações de propriedade do nó (`bindDirectiveHostProps`).
        for inst in &r.instancias {
            if let Criacao::Diretiva { diretiva, .. } = &inst.criacao
                && !diretiva.ligacoes_do_hospedeiro.is_empty()
            {
                self.deteccao.push(format!(
                    "    this.{}.detectHostChanges(this, {alvo});",
                    inst.campo
                ));
            }
        }
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

/// Algum elemento tem ligação de propriedade (ou um `#ref` lido como
/// local, de `refs`) e, portanto, vira campo?
fn tem_elemento_ligado(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    refs: &std::collections::HashSet<String>,
) -> bool {
    nos.iter().any(|n| match n {
        // Componente filho não vira campo de elemento: quem guarda a raiz
        // dele é a visão-filha.
        // Subárvore de `*` é da visão embutida.
        No::Elemento(e) if e.estrela.is_some() => false,
        No::Elemento(e) if !filhos.contains_key(&e.nome) => {
            liga_no_elemento(e, &diretivas_casadas(usadas, e))
                || e.referencias.iter().any(|r| refs.contains(&r.nome))
                || tem_elemento_ligado(&e.filhos, filhos, usadas, refs)
        }
        No::Elemento(e) => tem_elemento_ligado(&e.filhos, filhos, usadas, refs),
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

/// Alguma diretiva ou componente filho em `nos` (também dentro de `*`)
/// depende de um destes tokens?
fn pede_algum(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
    usadas: &[Usada],
    tokens: &[crate::diretivas::Token],
) -> bool {
    nos.iter().any(|n| {
        let No::Elemento(e) = n else { return false };
        let das_diretivas = diretivas_casadas(usadas, e)
            .iter()
            .any(|d| d.dependencias.iter().any(|x| tokens.contains(&x.token)));
        let do_filho = filhos.get(&e.nome).is_some_and(|f| {
            f.parametros.iter().any(|p| {
                matches!(p, Injetado::Servico { uri, classe, .. }
                if tokens.contains(&crate::diretivas::Token::Classe {
                    uri: uri.clone(),
                    classe: classe.clone(),
                }))
            })
        });
        das_diretivas || do_filho || pede_algum(&e.filhos, filhos, usadas, tokens)
    })
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
        // `detectHostChanges(this, el)` lê o nó na detecção.
        || casadas.iter().any(|d| !d.ligacoes_do_hospedeiro.is_empty())
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
    let arg = if tipo.uri == "dart:core" {
        format!("{}{}", tardio_q("dart:core"), tipo.classe)
    } else {
        let (uri, classe) = (&tipo.uri, &tipo.classe);
        let args = if tipo.genericos > 0 {
            format!("<{}>", vec!["dynamic"; tipo.genericos].join(", "))
        } else {
            String::new()
        };
        format!("\u{1}k:{uri}#token|{uri}\u{2}{classe}{args}")
    };
    format!(
        "const {}{classe_do_token}<{arg}>({})",
        tardio_q(crate::diretivas::DI_TOKENS),
        literal(nome)
    )
}

/// O que gera campo na classe da visão, na ordem em que aparece.
enum CampoDaVisao<'a> {
    Filho(&'a Filho),
    /// Diretiva estrutural, com a URI da classe dela.
    Estrutural(&'static str),
    /// `<template>` escrito: o `ViewContainer` e, com `#ref`, o campo do
    /// `TemplateRef`.
    Molde(bool),
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
                saida.push(CampoDaVisao::Molde(!e.referencias.is_empty()));
            } else if let Some(d) = Estrutural::conhecida(&estrela.nome) {
                saida.push(CampoDaVisao::Estrutural(d.uri));
            }
            continue;
        }
        if let Some(f) = filhos.get(&e.nome) {
            saida.push(CampoDaVisao::Filho(f));
            // Os outros provedores do nó do filho, depois da instância.
            let extras = diretivas_casadas(usadas, e);
            if let Some(meta) = &f.metadados
                && (!extras.is_empty() || !meta.provedores.is_empty())
            {
                let mut so_provedores = (**meta).clone();
                so_provedores.dependencias.clear();
                let mut casadas = vec![std::sync::Arc::new(so_provedores)];
                casadas.extend(extras);
                if let Ok(r) = crate::diretivas::resolver_no_do_filho(&casadas, 0, None) {
                    separar(&r.instancias[1..], &mut saida);
                }
            }
        } else {
            let casadas = diretivas_casadas(usadas, e);
            if let (false, Ok(r)) = (
                casadas.is_empty(),
                crate::diretivas::resolver(&casadas, 0, None),
            ) {
                separar(&r.instancias, &mut saida);
            }
        }
        saida.extend(campos_em_ordem(&e.filhos, filhos, usadas, asset));
    }
    saida
}

/// O campo de um provedor do nó, com os imports tardios na ordem em que o
/// oficial os escreve: o tipo e, no preguiçoso de `providers:`, o valor.
fn uri_do_campo(i: &crate::diretivas::Instancia, asset: &str) -> String {
    use crate::diretivas::{Criacao, Token};
    let uri = match (&i.criacao, &i.token) {
        (Criacao::Expressao(_) | Criacao::Multi(_), _) => {
            return texto_de_provedor_preguicoso(i, asset)
                .or_else(|_| tipo_do_provedor(i, asset))
                .unwrap_or_default();
        }
        (Criacao::Diretiva { diretiva, .. }, _) if !diretiva.ligacoes_do_hospedeiro.is_empty() => {
            diretiva.uri.replace(".dart", ".template.dart")
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
    /// Os resultados de consulta de visão nesta visão: (`#ref`, campo sujo).
    refs_consultados: Vec<(String, String)>,
    /// Os provedores acima da âncora, vistos da visão nova.
    acima: Vec<(crate::diretivas::Token, String, Option<(String, u32)>)>,
    componentes_acima: u32,
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
    /// Os nomes de `#ref` que podem virar local ([`referencias_unicas`]).
    refs_unicos: std::collections::HashSet<String>,
    /// O nó de cada `#ref` visto, de todas as visões já percorridas: a visão
    /// aninhada é emitida depois da que a contém, e lê dela o campo.
    refs_resolvidos: std::cell::RefCell<std::collections::HashMap<String, String>>,
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
            refs_ancestrais: Vec::new(),
            consultas_dinamicas: Vec::new(),
            refs_consultados: Vec::new(),
            refs: Default::default(),
            refs_em_ordem: Vec::new(),
            campos: Vec::new(),
            intl: None,
            mensagens: Vec::new(),
            tag_atual: String::new(),
            campos_filho: Vec::new(),
            vistas_filhas: Vec::new(),
            vistas_hospedeiras: Default::default(),
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
            acima: Vec::new(),
            componentes_acima: 0,
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
    let refs_locais = referencias_locais(&espec.nos, ctx.filhos, &ctx.refs_unicos);
    let mut promovidos = refs_locais.clone();
    promovidos.extend(espec.refs_consultados.iter().map(|(n, _)| n.clone()));
    dentro.refs_consultados = espec.refs_consultados.clone();
    if let Err(r) = alocar_imports_dos_campos(
        dentro.imp,
        &espec.nos,
        ctx.filhos,
        ctx.usadas,
        &ctx.asset,
        &ctx.pipes.imports_dos_campos(espec.indice),
        &promovidos,
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
    dentro.acima = espec.acima.clone();
    dentro.componentes_acima = espec.componentes_acima;
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
                "final local_{nome} = {util}.unsafeCast<{classe}>({cadeia}){MARCA_DE_REF}.{nome}{FIM_DE_REF};"
            )),
        );
    }
    dentro.refs_ancestrais = espec.refs_ancestrais.clone();
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
    ctx.refs_resolvidos
        .borrow_mut()
        .extend(dentro.refs.iter().map(|(k, v)| (k.clone(), v.clone())));
    dentro.destruir.extend(ctx.pipes.destruicao(espec.indice));
    // Na coleta, um nó recusado não consome índice: a visão parece vazia
    // sem estar. O `<ng-container *x>` vazio é vazio de fato: a visão não
    // tem raiz nenhuma (`const <Object>[]`).
    let vazia = matches!(espec.nos.as_slice(), [No::Elemento(x)]
        if x.nome == "ng-container"
            && x.filhos.is_empty()
            && x.estrela.is_none()
            && x.atributos.is_empty()
            && x.propriedades.is_empty()
            && x.eventos.is_empty()
            && x.bananas.is_empty()
            && x.referencias.is_empty());
    if !vazia && dentro.proximo == 0 && dentro.coleta.as_ref().map_or(0, Vec::len) == anotadas {
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
        if dentro.vistas_hospedeiras.contains(v) {
            linhas_det.push(format!("    this.{v}.detectHostChanges(firstCheck);"));
        }
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
    // marca cada consulta da visão do componente com resultado aqui.
    let sujas = if espec.refs_consultados.is_empty() {
        String::new()
    } else {
        let linhas: Vec<String> = espec
            .refs_consultados
            .iter()
            .map(|(_, campo)| {
                format!(
                    "    {util}.unsafeCast<{}0>((this.parentView!)).{campo} = true;",
                    ctx.classe_da_visao
                )
            })
            .collect();
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
    let inicio = match (raizes.as_slice(), dentro.subscricoes) {
        _ if vazia && dentro.raizes.is_empty() && dentro.subscricoes == 0 => format!(
            "this.initRootNodesAndSubscriptions({util}.unsafeCast(const <Object>[]), null);"
        ),
        ([raiz], 0) => format!("this.initRootNode({raiz});"),
        (_, subscricoes) => {
            let subs = if subscricoes == 0 {
                "null".to_string()
            } else {
                let lista: Vec<String> = (0..subscricoes)
                    .map(|k| format!("subscription_{k}"))
                    .collect();
                format!("[{}]", lista.join(", "))
            };
            format!(
                "this.initRootNodesAndSubscriptions({util}.unsafeCast(<Object>[{}]), {subs});",
                raizes.join(", ")
            )
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
    let texto = format!(
        "\nclass {classe} extends {ev}.EmbeddedView<{tipo_do_contexto}> {{\n{campos}  {classe}({rv}.RenderView parentView, int parentIndex) : super(parentView, parentIndex);\n  @override\n  void build() {{\n{ctx_build}{corpo}    {inicio}\n  }}\n{injetor}{deteccao}{sujas}{destruicao}{metodos}}}\n\n{ev}.EmbeddedView<void> {fabrica}({rv}.RenderView parentView, int parentIndex) {{\n  return {classe}(parentView, parentIndex);\n}}\n"
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
    refs: &std::collections::HashSet<String>,
) -> Result<(), Recusa> {
    let sem_caminho = || recusa(Motivo::ComponenteNoTemplate, "filho sem caminho de import");
    let campos = campos_em_ordem(nos, filhos, usadas, asset);
    // Os campos com inicializador (provedores preguiçosos) vêm primeiro.
    for campo in &campos {
        if let CampoDaVisao::Preguicosos(textos) = campo {
            for t in textos {
                resolver_tardios(imp, t);
            }
        }
    }
    for campo in campos {
        match campo {
            CampoDaVisao::Preguicosos(_) => {}
            CampoDaVisao::Filho(f) => {
                for uri in [&f.uri_template, &f.uri_dart] {
                    let alvo = asset_de_uri(uri, "", Path::new("")).ok_or_else(sem_caminho)?;
                    let caminho = caminho_do_import(asset, &alvo).ok_or_else(sem_caminho)?;
                    if !e_o_proprio_template(asset, &caminho) {
                        imp.alias(&caminho);
                    }
                }
            }
            CampoDaVisao::Estrutural(uri) => {
                imp.alias(VIEW_CONTAINER);
                imp.alias(uri);
            }
            CampoDaVisao::Molde(com_ref) => {
                imp.alias(VIEW_CONTAINER);
                if com_ref {
                    imp.alias(TEMPLATE_REF);
                }
            }
            CampoDaVisao::Diretivas(textos) => {
                for t in textos {
                    resolver_tardios(imp, &t);
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
        imp,
    )
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
    s
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
    // A folha de estilo é compilada por quem chama (`gerar_interno`); o
    // diagnóstico roda a mesma conta aqui.
    if c.style_urls.len() == 1 {
        let url = &c.style_urls[0];
        if local.uri_do_estilo(url, !c.sem_encapsulamento).is_none() {
            fora.insert(recusa(Motivo::Estilos, "folha fora de lib/"));
        } else if c.sem_encapsulamento {
            if let Err(f) = crate::folha_sem_shim(local.caminho, url) {
                fora.insert(recusa(Motivo::Encapsulamento, f));
            }
        } else if !crate::estilo_compila(local.caminho, url) {
            fora.insert(recusa(Motivo::Estilos, "Sass ou CSS fora do subconjunto"));
        }
    }
    fora
}

/// O que um `@HostBinding` de componente escreve no elemento hospedeiro:
/// as ligações de `createElementPropertyAst` com o elemento `div`
/// (`_securityContextElementName` do `DirectiveConverter`), escritas por
/// `bindAndWriteToRenderer` com `isHtmlElement` falso.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FormaDoHospedeiro {
    /// `class.x`: `updateClassBindingNonHtml`.
    Classe(String),
    /// `attr.x`, com o saneador do contexto de segurança: `updateAttribute`.
    Atributo(String, Option<&'static str>),
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
    forma: FormaDoHospedeiro,
}

impl LigacaoDoHospedeiro {
    /// A instrução que escreve o valor `v` no elemento hospedeiro.
    fn acao(&self, v: &str) -> String {
        let dom = tardio(DOM_HELPERS);
        let saneado = |s: &Option<&'static str>| match s {
            Some(f) => format!("{}.{f}({v})", tardio(SAFE_HTML)),
            None => v.to_string(),
        };
        match &self.forma {
            FormaDoHospedeiro::Classe(x) => {
                format!("{dom}.updateClassBindingNonHtml(this.rootElement, '{x}', {v})")
            }
            FormaDoHospedeiro::Atributo(x, s) => format!(
                "{dom}.updateAttribute(this.rootElement, '{x}', {})",
                saneado(s)
            ),
            FormaDoHospedeiro::Propriedade(x, s) => {
                format!("{dom}.setProperty(this.rootElement, '{x}', {})", saneado(s))
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
                format!("this.rootElement.style.setProperty('{nome}', {valor})")
            }
        }
    }
}

/// A forma de um nome de `@HostBinding` (`createElementPropertyAst`): o que
/// não é `class.x`, `attr.x`, `style.x[.unidade]` ou propriedade simples é
/// recusado — `class`/`className` (a classe inteira), `attr.x.if`,
/// namespace e prefixo desconhecido ainda não têm caso.
fn forma_do_hospedeiro(nome: &str) -> Result<FormaDoHospedeiro, String> {
    let simples = |n: &str| !n.is_empty() && !n.contains(['.', ':']);
    let fora = || format!("@HostBinding('{nome}') fora de class.x, attr.x, style.x e propriedade");
    let partes: Vec<&str> = nome.split('.').collect();
    Ok(match partes.as_slice() {
        ["class", x] if simples(x) => FormaDoHospedeiro::Classe(x.to_string()),
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
        let mut forma = forma_do_hospedeiro(&nome).map_err(|f| fora(&f))?;
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
            forma,
        });
    }
    Ok(saida)
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
    imp: &mut Importacoes,
) -> Result<String, Recusa> {
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
    // `ViewEncapsulation.none` com folha: a folha sem shim (`.css.dart`,
    // escrita por `gerar_interno`), `ComponentStyles.unscoped` e nenhum
    // `addShimC` (caso i88). Só a folha `.css` escrita, sem `@import`: a
    // saída do `sass_builder` e as folhas importadas mudam o texto.
    if c.sem_encapsulamento
        && let [url] = c.style_urls.as_slice()
        && let Err(f) = crate::folha_sem_shim(local.caminho, url)
    {
        anotar(coleta, recusa(Motivo::Encapsulamento, f))?;
    }
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
    // `providers:` do componente: a visão-hospedeira os cria.
    let no_hospedeiro = if c.com_provedores {
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

    // A folha compilada é o primeiro import do arquivo, antes de tudo.
    let estilo = match c.style_urls.len() {
        0 => None,
        1 => {
            // A folha entra pela URI `package:` mesmo estando ao lado: é
            // assim que o oficial escreve (o resolvedor de `styleUrls` é
            // outro, e não passa pelo caminho relativo).
            match local.uri_do_estilo(&c.style_urls[0], !c.sem_encapsulamento) {
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
    // `@i18n`: o `package:intl` entra com o primeiro campo `_message_N`,
    // antes ou depois do `text_binding.dart` conforme a ordem de documento.
    // Com filho, diretiva ou `*` na visão, a ordem dos campos (e dos
    // imports) entre eles ainda não tem caso.
    let ordem_i18n = i18n_antes_da_interpolacao(nos);
    let mut intl = None;
    if ordem_i18n == Some(true) {
        intl = Some(imp.alias(INTL));
    }
    let tb = tem_interpolacao(nos).then(|| imp.alias(TEXT_BINDING));
    if ordem_i18n == Some(false) {
        intl = Some(imp.alias(INTL));
    }
    if ordem_i18n.is_some() && !campos_em_ordem(nos, filhos, usadas, &local.asset()).is_empty() {
        anotar(
            coleta,
            recusa(Motivo::I18n, "@i18n com filho, diretiva ou `*` na visão"),
        )?;
    }
    let refs_unicos = referencias_unicas(nos, c);
    let refs_locais = referencias_locais(nos, filhos, &refs_unicos);
    if let Err(r) = alocar_imports_dos_campos(
        imp,
        nos,
        filhos,
        usadas,
        &local.asset(),
        &tabela.imports_dos_campos(0),
        &refs_locais,
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
        com_estilo: !c.style_urls.is_empty() && !c.sem_encapsulamento,
        url_do_template: local.url_do_template.clone(),
        classe_da_visao: format!("View{}", c.classe),
        tipo_do_contexto: format!("{proprio}.{}", c.classe),
        html: html.clone(),
        pipes: &tabela,
        refs_unicos,
        refs_resolvidos: Default::default(),
    };
    let mut corpo = ctx.corpo(imp, nomes, coleta.take(), false);
    corpo.refs_livres = referencias_livres(nos);
    corpo.declarar_refs(refs_locais);
    corpo.consultas_dinamicas = c
        .consultas
        .iter()
        .enumerate()
        .filter(|(_, q)| consulta_em_embutida(nos, q, filhos, local, resolvedor))
        .map(|(i, q)| ConsultaDinamica {
            indice: i,
            propriedade: q.propriedade.clone(),
            referencia: q.referencia.clone(),
            lista: q.lista,
            campo: format!("_viewQuery_{}_{i}_isDirty", q.referencia),
            origem: None,
        })
        .collect();
    // Os campos "sujos" abrem a classe, na ordem das consultas.
    for d in &corpo.consultas_dinamicas {
        corpo.campos.push(format!("  bool {} = true;", d.campo));
    }
    corpo.intl = intl;
    corpo.tb = tb;
    let r = corpo
        .nos(nos, "parentRenderNode")
        .and_then(|()| corpo.conferir_pipes());
    if let Err(r) = r {
        *coleta = corpo.coleta.take();
        return Err(r);
    }
    ctx.refs_resolvidos
        .borrow_mut()
        .extend(corpo.refs.iter().map(|(k, v)| (k.clone(), v.clone())));
    // O `ngOnDestroy` dos pipes vem depois dos das diretivas.
    corpo.destruir.extend(tabela.destruicao(0));
    // `@ViewChild` estático: atribuição imediata, no `afterNodes` — depois
    // dos ouvintes, na ordem de declaração das consultas
    // (`updateQueryAtStartup`, `createImmediateUpdates` em
    // `compile_query.dart`). `formas_contra_o_template` já garantiu que cada
    // `#ref` está uma vez só, num elemento HTML da própria visão.
    let mut consultas = Vec::new();
    for (i, q) in c.consultas.iter().enumerate() {
        // A dinâmica sai na detecção.
        if corpo.consultas_dinamicas.iter().any(|d| d.indice == i) {
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
    // `@HostBinding` do componente: o `detectHostChanges(firstCheck)` da
    // visão (`bindAndWriteToRenderer` com `isHtmlElement` falso — daí o
    // `updateClassBindingNonHtml`), com os índices de ligação depois dos do
    // template e as imutáveis antes, no `if (firstCheck)`. O `checkBinding`
    // leva `null, null`: a ligação não tem texto de template.
    let host_changes = if do_hospedeiro.is_empty() {
        String::new()
    } else {
        let chk = tardio(CHECK_BINDING);
        let mut constantes = Vec::new();
        let mut dinamicas = Vec::new();
        for l in &do_hospedeiro {
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
    // As consultas de visão dinâmicas abrem o bloco dos ganchos de
    // conteúdo (`updateContentQuery` escreve no `_updateContentQueriesMethod`),
    // na ordem das consultas.
    let mut apos_conteudo = Vec::new();
    for d in &corpo.consultas_dinamicas {
        let Some((ancora, classe)) = &d.origem else {
            continue;
        };
        let q = tardio(QUERIES);
        let mapa = format!(
            "this.{ancora}.mapNestedViewsWithSingleResult(({classe} nestedView) {{\n      return nestedView{MARCA_DE_REF}.{}{FIM_DE_REF};\n    }})",
            d.referencia
        );
        let valor = if d.lista {
            mapa
        } else {
            format!("{q}.firstOrNull({mapa})")
        };
        apos_conteudo.push(format!(
            "    if (this.{campo}) {{\n      _ctx.{} = {};\n      this.{campo} = false;\n    }}",
            d.propriedade,
            indentar(&valor, 2).trim_start(),
            campo = d.campo
        ));
    }
    apos_conteudo.extend(corpo.apos_conteudo.iter().cloned());
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
        if corpo.vistas_hospedeiras.contains(v) {
            linhas_deteccao.push(format!("    this.{v}.detectHostChanges(firstCheck);"));
        }
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
    let campos_hosp = provedores
        .as_ref()
        .map(|p| resolver_tardios(imp, &p.campos))
        .unwrap_or_default();
    let antes_do_componente = provedores
        .as_ref()
        .map(|p| resolver_tardios(imp, &p.antes_do_componente))
        .unwrap_or_default();
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
    let marca = c.on_push && !c.entradas.is_empty();
    if c.on_push && c.entradas.is_empty() && c.herda {
        let r = recusa(Motivo::NaoEntendido, "componente onPush que herda @Input");
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
    let mut ciclo = resolver_tardios(imp, &ciclo_de_vida(&c.ganchos, marca));
    // Com `@HostBinding`, a hospedeira chama o `detectHostChanges` antes de
    // detectar a visão do componente. Junto de ganchos de ciclo de vida,
    // a ordem ainda não tem caso.
    if !do_hospedeiro.is_empty() {
        if ciclo.is_empty() {
            ciclo = "\n  @override\n  void detectChangesInternal() {\n    bool firstCheck = this.firstCheck;\n    this.componentView.detectHostChanges(firstCheck);\n    this.componentView.detectChanges();\n  }\n".to_string();
        } else if coleta.is_none() {
            return Err(recusa(
                Motivo::HostBindingEmComponente,
                "@HostBinding com gancho de ciclo de vida",
            ));
        }
    }

    let x = &c.classe;
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
    let (lista_de_estilos, encapsulamento) = match &estilo {
        Some(a) if c.sem_encapsulamento => (format!("[{a}.styles]"), "unscoped"),
        Some(a) => (format!("[{a}.styles]"), "scoped"),
        None => ("const []".to_string(), "unscoped"),
    };
    let asset = format!("asset:{}/{}", local.pacote, local.relativo);

    let mut s = String::with_capacity(4096);
    let _ = write!(
        s,
        "
final List<Object> styles${x} = {lista_de_estilos};

class View{x}0 extends {vista}.ComponentView<{proprio}.{x}> {{
{campos}  static {estilos}.ComponentStyles? _componentStyles;
  View{x}0({view}.View parentView, int parentIndex) : super(parentView, parentIndex, {cd}.ChangeDetectionCheckedState.{estado}) {{
    this.initComponentStyles();
    this.rootElement = {util}.unsafeCast({html}.document.createElement('{tag}'));
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
{campos_hosp}  @override
  void build() {{
    this.componentView = View{x}0(this, 0);
    final _el_0 = this.componentView.rootElement;
{antes_do_componente}    this.component = {construcao}
{consultas_hosp}    this.initRootNode(_el_0);
  }}
{injetor_hosp}{ciclo}}}

{hosp}.HostView<{proprio}.{x}> viewFactory_{x}Host0() {{
  return _View{x}Host0();
}}
"
    );
    // O nó que uma visão lê de uma aninhada (o resultado de consulta) só é
    // conhecido depois de emitida a aninhada.
    let s = resolver_refs(&s, &ctx.refs_resolvidos.borrow());
    if s.contains(MARCA_DE_REF) && coleta.is_none() {
        return Err(recusa(Motivo::Ligacao, "#ref sem nó no template"));
    }
    Ok(s)
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
    for p in &meta.provedores {
        let alvo = match &p.fonte {
            crate::diretivas::Fornece::Existente(t) => Some(t),
            _ => None,
        };
        if !token_conhecido(&p.token) || alvo.is_some_and(|t| !token_conhecido(t)) {
            return Err("token de tipo fora do dart:core".into());
        }
        // O campo tipado pelo `T` com argumentos só tem caso no multi.
        if !p.multi && p.tipo.as_ref().is_some_and(|t| t.genericos > 0) {
            return Err("tipo de provedor com argumentos".into());
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

/// O valor de um provedor da hospedeira (`ProviderSource.build`). Com
/// dependência do injetor, a criação vai embrulhada em `debugInjectorWrap`
/// sob `isDevMode` (`recuo`: a coluna da instrução ou do campo).
fn texto_da_expr(
    e: &crate::diretivas::Expr,
    token: &crate::diretivas::Token,
    asset: &str,
    util: &str,
    recuo: usize,
) -> String {
    use crate::diretivas::Expr;
    let simples = |e: &Expr| -> String {
        let args = |a: &[Expr]| -> String {
            a.iter()
                .map(|x| texto_da_expr(x, token, asset, util, recuo))
                .collect::<Vec<_>>()
                .join(", ")
        };
        match e {
            Expr::Campo(c) => format!("this.{c}"),
            Expr::Injetor { token, opcional } => format!(
                "this.{}({}, this.parentIndex)",
                if *opcional {
                    "injectorGetOptional"
                } else {
                    "injectorGet"
                },
                expr_do_token(&token_local(token, asset))
            ),
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
        Expr::Campo(_) => {
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
) -> Result<String, Recusa> {
    use crate::diretivas::{Criacao, Expr};
    let tipo = tipo_do_provedor(i, asset)?;
    // Sem dependência do injetor, o `util` do `debugInjectorWrap` não entra.
    let valor = match &i.criacao {
        Criacao::Expressao(e) if !e.dinamica() => texto_da_expr(e, &i.token, asset, "", 2),
        Criacao::Multi(itens) if !itens.iter().any(Expr::dinamica) => format!(
            "[{}]",
            itens
                .iter()
                .map(|x| texto_da_expr(x, &i.token, asset, "", 2))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => {
            return Err(recusa(
                Motivo::LigacaoEmFilho,
                "provedor do filho com dependência de fora do nó",
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
            Criacao::Expressao(e) => texto_da_expr(e, &i.token, asset, util, recuo),
            Criacao::Multi(itens) => format!(
                "[{}]",
                itens
                    .iter()
                    .map(|x| texto_da_expr(x, &i.token, asset, util, recuo))
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
    let injeta = c.parametros.iter().any(|p| {
        !e_elemento(p.tipo.as_deref()) && !e_detector(p, local, resolvedor) && do_no(p).is_none()
    });
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
        if !p.nomeado
            && let Some(campo) = do_no(p)
        {
            args.push(campo);
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
        let micro = crate::micro::analisar(&estrela.nome, &estrela.valor);
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
            r#"<template ngFor [ngForOf]="f(';')"></template>"#,
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
