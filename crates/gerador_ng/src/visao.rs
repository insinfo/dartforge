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
use std::fmt::Write;

/// Tabela de imports do arquivo gerado.
#[derive(Default)]
pub struct Importacoes {
    itens: Vec<(String, bool)>,
}

impl Importacoes {
    /// Aloca (ou reaproveita) o número de uma URI e devolve o prefixo.
    pub fn alias(&mut self, uri: &str) -> String {
        let n = self.indice(uri, true);
        format!("import{n}")
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

/// Por que um arquivo ainda não é gerado por nós. O placar conta por motivo:
/// é isso que diz qual forma vale a pena aprender em seguida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Motivo {
    /// `@Directive` ou `@Pipe` no mesmo arquivo.
    DiretivaOuPipe,
    /// `@GenerateInjector`.
    Injetor,
    /// Mais de um componente no arquivo.
    VariosComponentes,
    /// `styleUrls`/`styles`: mexem em `styles$X` e ligam o shim de estilo.
    Estilos,
    /// Construtor que pede injeção de dependência.
    Injecao,
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
}

impl Motivo {
    pub fn texto(self) -> &'static str {
        match self {
            Motivo::DiretivaOuPipe => "diretiva ou pipe",
            Motivo::Injetor => "@GenerateInjector",
            Motivo::VariosComponentes => "vários componentes no arquivo",
            Motivo::Estilos => "folha de estilo",
            Motivo::Injecao => "injeção no construtor",
            Motivo::Ligacao => "ligação no template",
            Motivo::Interpolacao => "interpolação",
            Motivo::ComponenteNoTemplate => "componente no template",
            Motivo::Projecao => "<ng-content>",
            Motivo::EstiloEmLinha => "style em linha",
            Motivo::TemplateAusente => "template não encontrado",
        }
    }
}

/// Todos os motivos que impedem a geração deste componente, não só o
/// primeiro. Sem isto o placar engana: um arquivo que trava em folha de
/// estilo pode travar também em ligação e interpolação, e contar só o
/// primeiro faz parecer que aprender uma forma destrava o arquivo.
pub fn motivos(c: &Componente, nos: &[No]) -> std::collections::BTreeSet<Motivo> {
    let mut fora = std::collections::BTreeSet::new();
    if !c.style_urls.is_empty() || !c.styles.is_empty() {
        fora.insert(Motivo::Estilos);
    }
    if construcao_do_componente(c).is_none() {
        fora.insert(Motivo::Injecao);
    }
    motivos_dos_nos(nos, &mut fora);
    fora
}

fn motivos_dos_nos(nos: &[No], fora: &mut std::collections::BTreeSet<Motivo>) {
    for no in nos {
        match no {
            No::Comentario(_) | No::Texto(_) => {}
            No::Interpolacao(_) => {
                fora.insert(Motivo::Interpolacao);
            }
            No::Conteudo { .. } => {
                fora.insert(Motivo::Projecao);
            }
            No::Elemento(e) => {
                if !dom::tag_html(&e.nome) {
                    fora.insert(Motivo::ComponenteNoTemplate);
                } else if !e.propriedades.is_empty()
                    || !e.eventos.is_empty()
                    || !e.bananas.is_empty()
                    || !e.referencias.is_empty()
                    || e.estrela.is_some()
                {
                    fora.insert(Motivo::Ligacao);
                }
                if e.atributos.iter().any(|a| a.valor.contains("{{")) {
                    fora.insert(Motivo::Interpolacao);
                }
                if e.atributos.iter().any(|a| a.nome == "style") {
                    fora.insert(Motivo::EstiloEmLinha);
                }
                motivos_dos_nos(&e.filhos, fora);
            }
        }
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
}

/// Corpo do `build()` de uma visão, montado enquanto se anda pelo template.
struct Corpo<'a> {
    linhas: Vec<String>,
    /// Próximo índice de nó. Vale para elementos e textos juntos, em ordem de
    /// documento; comentário não consome índice porque some antes.
    proximo: u32,
    /// `final doc = …` sai uma vez, no primeiro elemento.
    tem_doc: bool,
    imp: &'a mut Importacoes,
    html: String,
}

impl Corpo<'_> {
    fn dom(&mut self) -> String {
        self.imp.alias(DOM_HELPERS)
    }

    /// Emite os nós filhos de `pai`. Devolve `None` na primeira forma que o
    /// emissor ainda não cobre — o arquivo inteiro volta para o build_runner.
    fn nos(&mut self, nos: &[No], pai: &str) -> Result<(), Motivo> {
        for no in nos {
            match no {
                No::Comentario(_) => {}
                No::Texto(t) => {
                    let n = self.proximo;
                    self.proximo += 1;
                    let dom = self.dom();
                    let texto = literal(t);
                    self.linhas.push(format!(
                        "    final _text_{n} = {dom}.appendText({pai}, {texto});"
                    ));
                }
                No::Elemento(e) => {
                    if !e.propriedades.is_empty()
                        || !e.eventos.is_empty()
                        || !e.bananas.is_empty()
                        || !e.referencias.is_empty()
                        || e.estrela.is_some()
                        || !dom::tag_html(&e.nome)
                    {
                        // Ligação, referência, `*ngIf` ou componente: ainda não.
                        return Err(if dom::tag_html(&e.nome) {
                            Motivo::Ligacao
                        } else {
                            Motivo::ComponenteNoTemplate
                        });
                    }
                    if e.atributos.iter().any(|a| a.valor.contains("{{")) {
                        return Err(Motivo::Interpolacao);
                    }
                    let n = self.proximo;
                    self.proximo += 1;
                    if !self.tem_doc {
                        self.tem_doc = true;
                        let html = self.html.clone();
                        self.linhas.push(format!("    final doc = {html}.document;"));
                    }
                    let dom = self.dom();
                    let tag = e.nome.to_ascii_lowercase();
                    let criacao = match tag.as_str() {
                        "div" => format!("{dom}.appendDiv(doc, {pai})"),
                        "span" => format!("{dom}.appendSpan(doc, {pai})"),
                        _ => {
                            let tipo = dom::tipo_da_tag(&tag);
                            let html = &self.html;
                            format!(
                                "{dom}.appendElement<{html}.{tipo}>(doc, {pai}, '{tag}')"
                            )
                        }
                    };
                    self.linhas.push(format!("    final _el_{n} = {criacao};"));
                    // Atributos saem em ordem alfabética (`_toSortedBindings`).
                    let mut atributos = e.atributos.clone();
                    atributos.sort_by(|a, b| a.nome.cmp(&b.nome));
                    for a in &atributos {
                        let valor = literal(&a.valor);
                        if a.nome == "class" {
                            self.linhas
                                .push(format!("    this.updateChildClass(_el_{n}, {valor});"));
                        } else if a.nome == "style" {
                            return Err(Motivo::EstiloEmLinha);
                        } else {
                            let dom = self.dom();
                            let nome = &a.nome;
                            self.linhas.push(format!(
                                "    {dom}.setAttribute(_el_{n}, '{nome}', {valor});"
                            ));
                        }
                    }
                    self.nos(&e.filhos, &format!("_el_{n}"))?;
                }
                No::Interpolacao(_) => return Err(Motivo::Interpolacao),
                No::Conteudo { .. } => return Err(Motivo::Projecao),
            }
        }
        Ok(())
    }
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

/// Gera o `.template.dart` de um arquivo com um componente só, cujo template
/// tem apenas elementos HTML e texto — sem ligação, diretiva, projeção,
/// folha de estilo nem injeção no construtor.
///
/// O que não couber volta `None` e continua vindo do `build_runner`.
pub fn template_de_componente(
    c: &Componente,
    local: &Local,
    nos: &[No],
) -> Result<String, Motivo> {
    if !c.style_urls.is_empty() || !c.styles.is_empty() {
        return Err(Motivo::Estilos); // mudam `styles$X` e ligam o shim
    }
    let construcao = construcao_do_componente(c).ok_or(Motivo::Injecao)?;

    let mut imp = Importacoes::default();
    let vista = imp.alias(COMPONENT_VIEW);
    let proprio = imp.alias(local.arquivo);
    let estilos = imp.alias(STYLE_ENCAPSULATION);
    let view = imp.alias(VIEW);
    let cd = imp.alias(CHANGE_DETECTION);
    let util = imp.alias(UTILITIES);
    // O construtor da visão usa `document.createElement`, então `dart:html`
    // sempre entra antes do corpo do `build()`.
    let html = imp.alias("dart:html");

    let mut corpo = Corpo {
        linhas: Vec::new(),
        proximo: 0,
        tem_doc: false,
        imp: &mut imp,
        html: html.clone(),
    };
    corpo.nos(nos, "parentRenderNode")?;
    let linhas = corpo.linhas.join("\n");
    let corpo_build =
        if linhas.is_empty() { String::new() } else { format!("\n{linhas}") };

    imp.sem_alias(ANGULAR);
    let hosp = imp.alias(HOST_VIEW);

    let x = &c.classe;
    let seletor = &c.seletor;
    let estado = if c.on_push { "waitingToBeChecked" } else { "checkAlways" };
    let asset = format!("asset:{}/{}", local.pacote, local.relativo);

    let mut s = String::with_capacity(4096);
    s.push_str(crate::CABECALHO);
    let _ = writeln!(s, "import '{}';", local.arquivo);
    imp.escrever(&mut s);
    let _ = write!(
        s,
        "
final List<Object> styles${x} = const [];

class View{x}0 extends {vista}.ComponentView<{proprio}.{x}> {{
  static {estilos}.ComponentStyles? _componentStyles;
  View{x}0({view}.View parentView, int parentIndex) : super(parentView, parentIndex, {cd}.ChangeDetectionCheckedState.{estado}) {{
    this.initComponentStyles();
    this.rootElement = {util}.unsafeCast({html}.document.createElement('{seletor}'));
  }}
  static String? get _debugComponentUrl {{
    return ({util}.isDevMode ? '{asset}' : null);
  }}

  @override
  void build() {{
    final parentRenderNode = this.initViewRoot();{corpo_build}
  }}

  static void _debugClearComponentStyles() {{
    _componentStyles = null;
  }}

  void initComponentStyles() {{
    var styles = _componentStyles;
    if ((styles == null)) {{
      _componentStyles = (styles = {estilos}.ComponentStyles.unscoped(styles${x}, _debugComponentUrl));
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

final List<Object> styles${x}Host = const [];

class _View{x}Host0 extends {hosp}.HostView<{proprio}.{x}> {{
  @override
  void build() {{
    this.componentView = View{x}0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = {proprio}.{x}({construcao});
    this.initRootNode(_el_0);
  }}
}}

{hosp}.HostView<{proprio}.{x}> viewFactory_{x}Host0() {{
  return _View{x}Host0();
}}
"
    );
    Ok(s)
}

/// Argumentos do construtor do componente na visão-hospedeira.
///
/// Só as formas que não precisam de injeção: sem parâmetros, ou um parâmetro
/// do elemento raiz. Injeção de serviços exige resolver o token até a
/// biblioteca que o declara, e isso entra quando o gerador enxergar o banco
/// semântico.
fn construcao_do_componente(c: &Componente) -> Option<String> {
    let mut args = Vec::new();
    for p in &c.parametros {
        let tipo = p.tipo.as_deref().unwrap_or("");
        match tipo {
            "Element" | "HtmlElement" | "html.Element" => args.push("_el_0".to_string()),
            _ => return None,
        }
    }
    Some(args.join(", "))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::componente::Parametro;

    fn local() -> Local<'static> {
        Local {
            pacote: "new_sali_frontend",
            relativo: "lib/src/shared/components/form_feedback/form_feedback_component.dart",
            arquivo: "form_feedback_component.dart",
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
        let saida = template_de_componente(&c, &local(), &[No::Comentario("{{message}}".into())])
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
        };
        let nos = crate::html::analisar("<div>Processando login...</div>");
        let saida = template_de_componente(&c, &local, &nos).expect("gera");
        let esperado = include_str!("../testes/callback_component.template.dart");
        assert_eq!(saida, esperado.replace("\r\n", "\n"));
    }

    #[test]
    fn ligacao_ainda_nao_gera() {
        let c = Componente { classe: "X".into(), seletor: "x".into(), ..Default::default() };
        let nos = crate::html::analisar("<div [hidden]=\"a\"></div>");
        assert_eq!(template_de_componente(&c, &local(), &nos), Err(Motivo::Ligacao));
    }

    #[test]
    fn componente_dentro_do_template_ainda_nao_gera() {
        let c = Componente { classe: "X".into(), seletor: "x".into(), ..Default::default() };
        let nos = crate::html::analisar("<outro-comp></outro-comp>");
        assert_eq!(
            template_de_componente(&c, &local(), &nos),
            Err(Motivo::ComponenteNoTemplate)
        );
    }

    #[test]
    fn parametro_injetado_ainda_nao_gera() {
        let c = Componente {
            classe: "X".into(),
            seletor: "x".into(),
            parametros: vec![Parametro {
                tipo: Some("RestConfig".into()),
                nome: "r".into(),
                nomeado: false,
            }],
            ..Default::default()
        };
        assert_eq!(template_de_componente(&c, &local(), &[]), Err(Motivo::Injecao));
    }
}
