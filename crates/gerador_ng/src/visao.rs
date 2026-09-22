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
//! Os `importN` saem numerados na ordem em que o emissor oficial os aloca; é
//! só por isso que a saída pode ser comparada byte a byte com a dele. O
//! `package:ngdart/angular.dart` ocupa um número mas é escrito sem prefixo,
//! porque `ComponentFactory` aparece sem qualificar no código gerado.
use crate::componente::Componente;
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
const HOST_VIEW: &str = "package:ngdart/src/core/linker/views/host_view.dart";
const ANGULAR: &str = "package:ngdart/angular.dart";

/// O que o emissor precisa saber de onde o componente mora.
pub struct Local<'a> {
    /// Nome do pacote (`new_sali_frontend`).
    pub pacote: &'a str,
    /// Caminho do `.dart` dentro do pacote, com barras: `lib/src/x/foo.dart`.
    pub relativo: &'a str,
    /// Nome do arquivo para o `import` de si mesmo: `foo.dart`.
    pub arquivo: &'a str,
}

/// Gera o `.template.dart` de um arquivo com um componente só, cujo template
/// não produz nenhum nó — nem elemento, nem texto, nem projeção.
///
/// É a forma mais simples que existe e serve de esqueleto verificado: tudo o
/// que vier depois entra dentro do `build()` e na tabela de imports, sem mexer
/// no resto.
pub fn template_de_componente(c: &Componente, local: &Local, nos: &[No]) -> Option<String> {
    if !nos.iter().all(|n| matches!(n, No::Comentario(_))) {
        return None; // template com conteúdo: ainda não
    }
    if !c.style_urls.is_empty() || !c.styles.is_empty() {
        return None; // folhas de estilo mudam `styles$X` e a encapsulação
    }
    let Some(construcao) = construcao_do_componente(c) else { return None };

    let mut imp = Importacoes::default();
    let vista = imp.alias(COMPONENT_VIEW);
    let proprio = imp.alias(&format!("{}", local.arquivo));
    let estilos = imp.alias(STYLE_ENCAPSULATION);
    let view = imp.alias(VIEW);
    let cd = imp.alias(CHANGE_DETECTION);
    let util = imp.alias(UTILITIES);
    let html = imp.alias("dart:html");
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
    final parentRenderNode = this.initViewRoot();
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
    Some(s)
}

/// Argumentos do construtor do componente na visão-hospedeira.
///
/// Só as formas que não precisam de injeção: sem parâmetros, ou um parâmetro
/// do elemento raiz. Injeção de serviços entra quando o gerador souber
/// resolver tokens.
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

    #[test]
    fn elemento_no_template_ainda_nao_gera() {
        let c = Componente { classe: "X".into(), seletor: "x".into(), ..Default::default() };
        let nos = crate::html::analisar("<div></div>");
        assert!(template_de_componente(&c, &local(), &nos).is_none());
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
        assert!(template_de_componente(&c, &local(), &[]).is_none());
    }
}
