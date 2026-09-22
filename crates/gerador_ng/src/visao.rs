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
const DI_ERRORS: &str = "package:ngdart/src/di/errors.dart";
const TEXT_BINDING: &str = "package:ngdart/src/runtime/text_binding.dart";
const CHECK_BINDING: &str = "package:ngdart/src/runtime/check_binding.dart";
const DEVTOOLS: &str = "package:ngdart/src/devtools.dart";
const INTERPOLATE: &str = "package:ngdart/src/runtime/interpolate.dart";

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
    /// Forma do componente que o gerador não sabe traduzir e, por isso,
    /// recusa — `@HostListener`, `@ViewChild`, ciclo de vida, argumento
    /// desconhecido de `@Component`.
    NaoEntendido,
}

impl Motivo {
    pub fn texto(self) -> &'static str {
        match self {
            Motivo::DiretivaOuPipe => "diretiva ou pipe",
            Motivo::Injetor => "@GenerateInjector",
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
            Motivo::NaoEntendido => "forma do componente não entendida",
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
    if c.nao_entendido.is_some() {
        fora.insert(Motivo::NaoEntendido);
    }
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
    motivos_dos_nos(nos, filhos, &mut fora);
    fora
}

fn motivos_dos_nos(
    nos: &[No],
    filhos: &std::collections::HashMap<String, Filho>,
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
                        || e.propriedades.iter().any(|l| !f.entradas.contains_key(&l.nome))
                    {
                        fora.insert(Motivo::LigacaoEmFilho);
                    }
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
                motivos_dos_nos(&e.filhos, filhos, fora);
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
        let caminho =
            if dir.is_empty() { url.to_string() } else { format!("{dir}/{url}") };
        Some(format!("package:{}/{caminho}.shim.dart", self.pacote))
    }

    /// URI `asset:` deste arquivo — o espaço em que o emissor oficial calcula
    /// os caminhos de import.
    fn asset(&self) -> String {
        format!("asset:{}/{}", self.pacote, self.relativo)
    }
}

/// Corpo do `build()` de uma visão, montado enquanto se anda pelo template.
struct Corpo<'a> {
    linhas: Vec<String>,
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
    /// Para analisar as expressões do template, que são expressões Dart.
    nomes: &'a mut dartforge_intern::Interner,
    /// `bool firstCheck = this.firstCheck;` no `detectChangesInternal`, quando
    /// alguma ligação imutável é escrita só na primeira checagem.
    usa_primeira_checagem: bool,
    /// `final _ctx = this.ctx;` no `detectChangesInternal` — uma visão que só
    /// repassa a detecção para as filhas não precisa dele.
    usa_ctx_na_deteccao: bool,
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
    fn propriedade(&mut self, l: &crate::html::Ligacao, alvo: &str) -> Result<(), Motivo> {
        let Some(url) = self.url_do_template.clone() else { return Err(Motivo::Ligacao) };
        let convertida = crate::expr::converter_com_metodos(&l.valor, self.membros, self.metodos, self.nomes, self.tipos)?;
        let expr = l.valor.trim();
        let (ini, fim) = (l.inicio, l.fim);
        // Valor que não muda é escrito uma vez, na primeira checagem, sem
        // `checkBinding` e sem campo de valor anterior (`isImmutable`).
        if convertida.imutavel {
            let acao = self.acao(l, alvo, &convertida.texto)?;
            self.usa_primeira_checagem = true;
            self.usa_ctx_na_deteccao = true;
        self.deteccao.push(format!(
                "    if (firstCheck) {{\n      {acao} /* REF:{url}:{ini}:{fim} */;\n    }}"
            ));
            return Ok(());
        }
        let k = self.proxima_ligacao;
        self.proxima_ligacao += 1;
        self.campos_expr.push(format!("  Object? _expr_{k};"));
        let acao = self.acao(l, alvo, &format!("currVal_{k}"))?;
        let chk = self.imp.alias(CHECK_BINDING);
        let valor = convertida.texto;
        self.usa_ctx_na_deteccao = true;
        self.deteccao.push(format!(
            "    final currVal_{k} = {valor};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, '{expr}', '{url}')) {{\n      {acao} /* REF:{url}:{ini}:{fim} */;\n      this._expr_{k} = currVal_{k};\n    }}"
        ));
        Ok(())
    }

    /// O que a ligação faz com o elemento, pelo prefixo do nome.
    fn acao(
        &mut self,
        l: &crate::html::Ligacao,
        alvo: &str,
        valor: &str,
    ) -> Result<String, Motivo> {
        let dom = self.dom();
        Ok(if l.nome == "class" {
            format!("this.updateChildClass({alvo}, {valor})")
        } else if let Some(classe) = l.nome.strip_prefix("class.") {
            format!("{dom}.updateClassBinding({alvo}, '{classe}', {valor})")
        } else if let Some(attr) = l.nome.strip_prefix("attr.") {
            format!("{dom}.updateAttribute({alvo}, '{attr}', {valor})")
        } else if let Some(estilo) = l.nome.strip_prefix("style.") {
            format!("{alvo}.style.setProperty('{estilo}', {valor})")
        } else if l.nome.contains('.') {
            return Err(Motivo::Ligacao);
        } else {
            let prop = &l.nome;
            format!("{dom}.setProperty({alvo}, '{prop}', {valor})")
        })
    }

    /// `(e)="metodo()"` ou `(e)="metodo($event)"`: o oficial passa o método
    /// por referência a `eventHandlerN`, onde N é quantos argumentos o
    /// template escreveu.
    fn evento(&mut self, l: &crate::html::Ligacao, alvo: &str) -> Result<(), Motivo> {
        let texto = l.valor.trim();
        let Some((nome, resto)) = texto.split_once('(') else { return Err(Motivo::Ligacao) };
        let Some(args) = resto.strip_suffix(')') else { return Err(Motivo::Ligacao) };
        let aridade = match args.trim() {
            "" => 0,
            "$event" => 1,
            _ => return Err(Motivo::Ligacao),
        };
        let metodo = crate::expr::converter_com_metodos(nome.trim(), self.membros, self.metodos, self.nomes, self.tipos)?
            .texto;
        self.usa_ctx_no_build = true;
        let evento = &l.nome;
        self.linhas.push(format!(
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
        let n = self.proximo;
        self.proximo += 1;
        let cam_template = caminho_do_import(&self.asset, &asset_de_uri(&filho.uri_template, "", Path::new(""))
            .ok_or(Motivo::ComponenteNoTemplate)?)
            .ok_or(Motivo::ComponenteNoTemplate)?;
        let cam_dart = caminho_do_import(&self.asset, &asset_de_uri(&filho.uri_dart, "", Path::new(""))
            .ok_or(Motivo::ComponenteNoTemplate)?)
            .ok_or(Motivo::ComponenteNoTemplate)?;
        let vt = self.imp.alias(&cam_template);
        let vd = self.imp.alias(&cam_dart);
        let classe = &filho.classe;
        let campo_vista = format!("_compView_{n}");
        let campo_inst = format!("_{classe}_{n}_5");
        self.campos_filho.push(format!("  late final {vt}.View{classe}0 {campo_vista};"));
        self.campos_filho.push(format!("  late final {vd}.{classe} {campo_inst};"));
        self.vistas_filhas.push(campo_vista.clone());
        self.linhas.push(format!("    this.{campo_vista} = {vt}.View{classe}0(this, {n});"));
        self.linhas.push(format!("    final _el_{n} = this.{campo_vista}.rootElement;"));
        self.linhas.push(format!("    {pai}.append(_el_{n});"));
        self.linhas.push(format!("    this.{campo_inst} = {vd}.{classe}();"));
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
            let raiz: Vec<String> =
                criados.iter().filter(|n| n.starts_with("_el_")).cloned().collect();
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
            self.linhas.push(format!("    this.{campo_vista}.create(this.{campo_inst});"));
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
        let Some(url) = self.url_do_template.clone() else { return Err(Motivo::LigacaoEmFilho) };
        let Some(campo) = filho.entradas.get(&l.nome).cloned() else {
            // Nome que o filho não declara como `@Input`: pode ser diretiva.
            return Err(Motivo::LigacaoEmFilho);
        };
        let convertida =
            crate::expr::converter_com_metodos(&l.valor, self.membros, self.metodos, self.nomes, self.tipos)
                .map_err(|_| Motivo::LigacaoEmFilho)?;
        let k = self.proxima_ligacao;
        self.proxima_ligacao += 1;
        self.campos_expr.push(format!("  Object? _expr_{k};"));
        let chk = self.imp.alias(CHECK_BINDING);
        let dev = self.imp.alias(DEVTOOLS);
        let expr = l.valor.trim();
        let (ini, fim) = (l.inicio, l.fim);
        let valor = convertida.texto;
        let nome = &l.nome;
        self.usa_ctx_na_deteccao = true;
        self.deteccao.push(format!(
            "    final currVal_{k} = {valor};\n    if ({chk}.checkBinding(this._expr_{k}, currVal_{k}, '{expr}', '{url}')) {{\n      if ({dev}.isDevToolsEnabled) {{\n        {dev}.Inspector.instance.recordInput(this.{campo_inst}, '{nome}', currVal_{k});\n      }}\n      this.{campo_inst}.{campo} = currVal_{k} /* REF:{url}:{ini}:{fim} */;\n      this._expr_{k} = currVal_{k};\n    }}"
        ));
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
        let convertida = crate::expr::converter_com_metodos(expr, self.membros, self.metodos, self.nomes, self.tipos)
            .map_err(|_| Motivo::Interpolacao)?;
        // Sem o tipo estático não dá para escolher entre `interpolateString`,
        // `interpolate` e `updateTextWithPrimitive` — e escolher errado muda o
        // que o programa faz.
        let Some(tipo) = convertida.tipo.clone() else { return Err(Motivo::Interpolacao) };
        let membro = crate::componente::Membro { tipo, imutavel: convertida.imutavel };
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
            let f = if nu == "String" { "interpolateString0" } else { "interpolate0" };
            format!("{alias}.{f}({acesso})")
        };
        if membro.imutavel {
            // Valor que não muda não tem ligação: o texto é calculado uma vez,
            // no `build()`, como o oficial faz (`isImmutable`).
            self.usa_ctx_no_build = true;
            let valor = interpolar(self.imp);
            let dom = self.dom();
            self.linhas
                .push(format!("    final _text_{n} = {dom}.appendText({pai}, {valor});"));
            return Ok(());
        }
        self.campos
            .push(format!("  final {tb}.TextBinding _textBinding_{n} = {tb}.TextBinding();"));
        self.linhas.push(format!("    {pai}.append(this._textBinding_{n}.element);"));
        let atualizacao = if primitivo(&nu) {
            format!("updateTextWithPrimitive({acesso})")
        } else {
            format!("updateText({})", interpolar(self.imp))
        };
        self.usa_ctx_na_deteccao = true;
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
                    if e.propriedades.iter().chain(e.eventos.iter()).any(|l| e_de_diretiva(&l.nome))
                        || e.atributos.iter().any(|a| e_de_diretiva(&a.nome))
                    {
                        return Err(Motivo::Diretiva);
                    }
                    if !e.bananas.is_empty() || !e.referencias.is_empty() || e.estrela.is_some() {
                        // `[(x)]`, `#ref` e `*ngIf` ainda não.
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
                        self.linhas.push(format!("    final doc = {html}.document;"));
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
                            format!(
                                "{dom}.appendElement<{html}.{tipo}>(doc, {pai}, '{tag}')"
                            )
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
                    // de descer. Os eventos saem no `build()` depois dos
                    // filhos, que é onde o oficial os escreve.
                    for l in &e.propriedades {
                        self.propriedade(l, &alvo)?;
                    }
                    if self.com_estilo {
                        // Isolamento de estilo por atributo: o elemento entra
                        // no escopo do componente.
                        self.linhas.push(format!("    this.addShimC({alvo});"));
                    }
                    self.nos(&e.filhos, &alvo)?;
                    for l in &e.eventos {
                        self.evento(l, &alvo)?;
                    }
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
        s.push_str("
  @override
  void detectChangesInternal() {
");
        if g.usa_primeira_checagem() {
            s.push_str("    bool firstCheck = this.firstCheck;
");
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
            s.push_str(&format!("    if ((!{dbg}.debugThrowIfChanged)) {{
"));
            if g.after_content_init {
                s.push_str("      if (firstCheck) {
        this.component.ngAfterContentInit();
      }
");
            }
            if g.after_content_checked {
                s.push_str("      this.component.ngAfterContentChecked();
");
            }
            s.push_str("    }
");
        }
        s.push_str("    this.componentView.detectChanges();
");
        if g.after_view_init || g.after_view_checked {
            s.push_str(&format!("    if ((!{dbg}.debugThrowIfChanged)) {{
"));
            if g.after_view_init {
                s.push_str("      if (firstCheck) {
        this.component.ngAfterViewInit();
      }
");
            }
            if g.after_view_checked {
                s.push_str("      this.component.ngAfterViewChecked();
");
            }
            s.push_str("    }
");
        }
        s.push_str("  }
");
    }
    if g.on_destroy {
        s.push_str("
  @override
  void destroyInternal() {
    this.component.ngOnDestroy();
  }
");
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
        No::Elemento(e) if !filhos.contains_key(&e.nome) => {
            !e.propriedades.is_empty() || tem_elemento_ligado(&e.filhos, filhos)
        }
        No::Elemento(e) => tem_elemento_ligado(&e.filhos, filhos),
        _ => false,
    })
}

/// Os componentes filhos usados no template, em ordem de documento.
fn filhos_em_ordem<'a>(
    nos: &[No],
    filhos: &'a std::collections::HashMap<String, Filho>,
) -> Vec<&'a Filho> {
    let mut saida = Vec::new();
    for no in nos {
        let No::Elemento(e) = no else { continue };
        match filhos.get(&e.nome) {
            Some(f) => {
                saida.push(f);
                // O conteúdo projetado é criado no mesmo `build()`, mas os
                // seus campos vêm depois — o percurso continua.
                saida.extend(filhos_em_ordem(&e.filhos, filhos));
            }
            None => saida.extend(filhos_em_ordem(&e.filhos, filhos)),
        }
    }
    saida
}

/// Nome que pertence a uma diretiva do ecossistema, não ao DOM.
fn e_de_diretiva(nome: &str) -> bool {
    let base = nome.split('.').next().unwrap_or(nome);
    base.starts_with("ng") || base.starts_with("form") && base != "form"
}

/// O template tem `{{ … }}` em algum lugar?
fn tem_interpolacao(nos: &[No]) -> bool {
    nos.iter().any(|n| match n {
        No::Interpolacao { .. } => true,
        No::Elemento(e) => tem_interpolacao(&e.filhos),
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
            let uri = local.uri_do_estilo(&c.style_urls[0]).ok_or(Motivo::Estilos)?;
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
    for f in filhos_em_ordem(nos, filhos) {
        for uri in [&f.uri_template, &f.uri_dart] {
            let asset = asset_de_uri(uri, "", Path::new("")).ok_or(Motivo::ComponenteNoTemplate)?;
            let caminho =
                caminho_do_import(&local.asset(), &asset).ok_or(Motivo::ComponenteNoTemplate)?;
            imp.alias(&caminho);
        }
    }
    if tem_elemento_ligado(nos, filhos) {
        imp.alias("dart:html");
    }
    let estilos = imp.alias(STYLE_ENCAPSULATION);
    let view = imp.alias(VIEW);
    let cd = imp.alias(CHANGE_DETECTION);
    let util = imp.alias(UTILITIES);
    // O construtor da visão usa `document.createElement`, então `dart:html`
    // sempre entra antes do corpo do `build()`.
    let html = imp.alias("dart:html");

    let mut corpo = Corpo {
        linhas: Vec::new(),
        campos: Vec::new(),
        campos_filho: Vec::new(),
        vistas_filhas: Vec::new(),
        filhos,
        asset: local.asset(),
        tipos: resolvedor.map(|r| (r, local.caminho)),
        com_estilo: !c.style_urls.is_empty(),
        campos_expr: Vec::new(),
        campos_el: Vec::new(),
        proxima_ligacao: 0,
        deteccao: Vec::new(),
        nomes,
        usa_primeira_checagem: false,
        usa_ctx_na_deteccao: false,
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
    let ctx_no_build =
        if corpo.usa_ctx_no_build { "
    final _ctx = this.ctx;" } else { "" };
    let linhas = corpo.linhas.join("\n");
    let corpo_build =
        if linhas.is_empty() { String::new() } else { format!("\n{linhas}") };
    // Ordem dos campos na classe, como o oficial escreve: ligações de texto,
    // depois os valores anteriores das ligações, depois os elementos.
    let mut todos = corpo.campos.clone();
    todos.extend(corpo.campos_filho.clone());
    todos.extend(corpo.campos_expr.clone());
    todos.extend(corpo.campos_el.clone());
    let campos =
        if todos.is_empty() { String::new() } else { format!("{}
", todos.join("
")) };
    // A detecção junta as ligações do próprio template e o repasse para as
    // visões-filhas. `_ctx` e `firstCheck` só são declarados se alguém os usa.
    let mut linhas_deteccao = corpo.deteccao.clone();
    for v in &corpo.vistas_filhas {
        linhas_deteccao.push(format!("    this.{v}.detectChanges();"));
    }
    let deteccao = if linhas_deteccao.is_empty() {
        String::new()
    } else {
        let ctx = if corpo.usa_ctx_na_deteccao { "    final _ctx = this.ctx;
" } else { "" };
        let primeira =
            if corpo.usa_primeira_checagem { "    bool firstCheck = this.firstCheck;
" } else { "" };
        format!(
            "
  @override
  void detectChangesInternal() {{
{ctx}{primeira}{}
  }}
",
            linhas_deteccao.join("
")
        )
    };
    // Visão-filha precisa ser destruída com a visão que a criou.
    let destruicao = if corpo.vistas_filhas.is_empty() {
        String::new()
    } else {
        let linhas: Vec<String> = corpo
            .vistas_filhas
            .iter()
            .map(|v| format!("    this.{v}.destroyInternalState();"))
            .collect();
        format!("
  @override
  void destroyInternal() {{
{}
  }}
", linhas.join("
"))
    };

    imp.sem_alias(ANGULAR);
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
    let estado = if c.on_push { "waitingToBeChecked" } else { "checkAlways" };
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
        args.push(format!("this.injectorGet({alias}.{simples}, this.parentIndex)"));
    }
    let chamada = format!("{proprio}.{x}({})", args.join(", "));
    let Some(erros) = erros else { return Some(format!("{chamada};")) };
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
    matches!(tipo.map(|t| t.rsplit('.').next().unwrap_or(t)), Some("Element" | "HtmlElement"))
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
            self.0.iter().find(|(n, _)| *n == nome).map(|(_, u)| u.to_string())
        }
    }

    fn param(tipo: &str) -> Parametro {
        Parametro { tipo: Some(tipo.into()), nome: "p".into(), nomeado: false, anotado: false }
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
        let saida = template_de_componente(&c, &local, &nos, None, &mut Interner::new(), &Default::default()).expect("gera");
        let esperado = include_str!("../testes/callback_component.template.dart");
        assert_eq!(saida, esperado.replace("\r\n", "\n"));
    }

    #[test]
    fn ligacao_ainda_nao_gera() {
        let c = Componente { classe: "X".into(), seletor: "x".into(), ..Default::default() };
        let nos = crate::html::analisar("<div [hidden]=\"a\"></div>");
        assert_eq!(template_de_componente(&c, &local(), &nos, None, &mut Interner::new(), &Default::default()), Err(Motivo::Ligacao));
    }

    #[test]
    fn componente_dentro_do_template_ainda_nao_gera() {
        let c = Componente { classe: "X".into(), seletor: "x".into(), ..Default::default() };
        let nos = crate::html::analisar("<outro-comp></outro-comp>");
        assert_eq!(
            template_de_componente(&c, &local(), &nos, None, &mut Interner::new(), &Default::default()),
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
            template_de_componente(&c, &local(), &[], None, &mut Interner::new(), &Default::default()),
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
            ("OidcService", "package:new_sali_frontend/src/shared/services/oidc_service.dart"),
            ("Router", "package:ngrouter/src/router/router.dart"),
        ]);
        let nos = crate::html::analisar("<div>Processando login...</div>");
        let saida =
            template_de_componente(&c, &local, &nos, Some(&tabela), &mut Interner::new(), &Default::default()).expect("gera");
        let esperado = include_str!("../testes/callback_com_injecao.template.dart");
        assert_eq!(saida, esperado.replace("
", "
"));
    }
}
