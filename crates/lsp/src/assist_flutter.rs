//! Assistências do Flutter do `analysis_server` 3.6.2 sobre a árvore no
//! formato do analyzer, com as utilidades de
//! `utilities/extensions/flutter.dart` (o widget é reconhecido pela classe
//! `Widget` de `package:flutter/src/widgets/framework.dart` nos supertipos).
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to children:` | `refactor.flutter.convert.childToChildren` | `FlutterConvertToChildren` |
//! | `Move widget down` / `Move widget up` | `refactor.flutter.move.down` / `.up` | `FlutterMoveDown`, `FlutterMoveUp` |
//! | `Remove this widget` | `refactor.flutter.removeWidget` | `FlutterRemoveWidget` |
//! | `Swap with child` / `Swap with parent` | `refactor.flutter.swap.withChild` / `.withParent` | `FlutterSwapWithChild`, `FlutterSwapWithParent` |
//! | `Wrap with Builder` | `refactor.flutter.wrap.builder` | `FlutterWrapBuilder` |
//! | `Wrap with StreamBuilder` | `refactor.flutter.wrap.streamBuilder` | `FlutterWrapStreamBuilder` |
//! | `Wrap with widget...` | `refactor.flutter.wrap.generic` | `FlutterWrapGeneric`, `FlutterWrap` |
//! | `Wrap with Center`/`Container`/`Padding`/`SizedBox`/`Column`/`Row` | `refactor.flutter.wrap.*` | `FlutterWrap` |
//! | `Move child property to end of arguments` | `refactor.sort.child.properties.last` | `SortChildPropertyLast` |

use crate::acoes::AcaoDeCodigo;
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::{Mudanca, Texto, UM_RECUO};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element};
use dartforge_types::{Resolved, Type, TypeId};

pub(crate) const URI_FRAMEWORK: &str = "package:flutter/src/widgets/framework.dart";
const URI_BASIC: &str = "package:flutter/src/widgets/basic.dart";
const URI_CONTAINER: &str = "package:flutter/src/widgets/container.dart";
const URI_ASYNC: &str = "package:flutter/src/widgets/async.dart";
const URI_WIDGETS: &str = "package:flutter/widgets.dart";

impl Contexto<'_> {
    /// `_isExactly`: a `ClassElement` de nome `nome` declarada no arquivo
    /// `uri`.
    pub(crate) fn flutter_exatamente(&self, c: ClassId, nome: &str, uri: &str) -> bool {
        let p = self.p.programa();
        let k = p.class(c);
        matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication) && self.p.nome(k.name) == nome && k.decl.is_some_and(|d| p.unit(d.unit).uri == uri)
    }

    /// `isWidget`: a classe `Widget` ou uma subclasse dela.
    pub(crate) fn classe_e_widget(&self, c: ClassId) -> bool {
        let p = self.p.programa();
        if !matches!(p.class(c).kind, ClassKind::Class | ClassKind::MixinApplication) {
            return false;
        }
        self.flutter_exatamente(c, "Widget", URI_FRAMEWORK) || self.p.supertipos(c).into_iter().any(|s| self.flutter_exatamente(s, "Widget", URI_FRAMEWORK))
    }

    /// A classe de um tipo de interface.
    pub(crate) fn classe_do_tipo_flutter(&self, t: TypeId) -> Option<ClassId> {
        match self.p.consulta.tabela.get(t) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        }
    }

    /// `isWidgetType`.
    fn tipo_widget(&self, t: TypeId) -> bool {
        self.classe_do_tipo_flutter(t).is_some_and(|c| self.classe_e_widget(c))
    }

    /// O `staticType` do nó de expressão.
    pub(crate) fn tipo_do_no_flutter(&self, n: usize) -> Option<TypeId> {
        self.expr_do_no(n).and_then(|x| self.corpos.get_type(x))
    }

    /// `isWidgetExpression`.
    pub(crate) fn e_expressao_widget(&self, n: usize) -> bool {
        if let Some(pai) = self.pai(n) {
            if self.especie(pai) == "NamedType" || self.pai(pai).is_some_and(|a| self.especie(a) == "NamedType") {
                return false;
            }
            if self.especie(pai) == "ConstructorName" {
                return false;
            }
        }
        if self.especie(n) == "NamedExpression" || !self.e_expressao(n) {
            return false;
        }
        self.tipo_do_no_flutter(n).is_some_and(|t| self.tipo_widget(t))
    }

    /// `findWidgetExpression`.
    pub(crate) fn achar_expressao_widget(&self, n: usize) -> Option<usize> {
        let mut atual = Some(n);
        while let Some(k) = atual {
            if !self.e_expressao_widget(k) {
                let e = self.especie(k);
                if e == "ArgumentList" || self.e_comando(k) || e.ends_with("FunctionBody") {
                    return None;
                }
                atual = self.pai(k);
                continue;
            }
            if self.especie(k) == "AssignmentExpression" {
                return None;
            }
            let Some(pai) = self.pai(k) else { return None };
            let filhos = self.filhos(pai);
            let ultimo = filhos.last() == Some(&k);
            match self.especie(pai) {
                "AssignmentExpression" => return if ultimo { Some(k) } else { None },
                "ArgumentList" | "ListLiteral" | "VariableDeclaration" => return Some(k),
                "ConditionalExpression" if filhos.first() != Some(&k) => return Some(k),
                "ExpressionFunctionBody" | "NamedExpression" | "SwitchExpressionCase" if ultimo => return Some(k),
                "ForElement" if ultimo => return Some(k),
                "IfElement" => {
                    // `thenElement`/`elseElement`: não a condição (nem o
                    // padrão do `case`).
                    let condicao = filhos.first() == Some(&k) || self.especie(k) == "CaseClause";
                    if !condicao {
                        return Some(k);
                    }
                }
                _ if self.e_comando(pai) => return Some(k),
                _ => {}
            }
            atual = Some(pai);
        }
        None
    }

    /// `findInstanceCreationExpression`.
    pub(crate) fn achar_criacao(&self, n: usize) -> Option<usize> {
        let mut no = n;
        for especie in ["ImportPrefixReference", "SimpleIdentifier", "PrefixedIdentifier", "NamedType", "ConstructorName"] {
            if self.especie(no) == especie {
                no = self.pai(no)?;
            }
        }
        (self.especie(no) == "InstanceCreationExpression").then_some(no)
    }

    /// `isWidgetCreation`.
    pub(crate) fn criacao_de_widget(&self, n: usize) -> bool {
        if self.especie(n) != "InstanceCreationExpression" {
            return false;
        }
        match self.expr_do_no(n).and_then(|x| self.corpos.get_resolved(x)) {
            Some(Resolved::Constructor(f)) => self.p.programa().function(*f).class.is_some_and(|c| self.classe_e_widget(c)),
            _ => false,
        }
    }

    /// A `ArgumentList` de uma invocação.
    fn lista_de_argumentos_flutter(&self, invocacao: usize) -> Option<usize> {
        self.filhos(invocacao).iter().copied().find(|&k| self.especie(k) == "ArgumentList")
    }

    /// O nome do rótulo de uma `NamedExpression`.
    pub(crate) fn rotulo_do_nomeado(&self, nomeado: usize) -> Option<&str> {
        let rotulo = *self.filhos(nomeado).first()?;
        if self.especie(rotulo) != "Label" {
            return None;
        }
        Some(self.texto_do_no(*self.filhos(rotulo).first()?))
    }

    /// `childArgument`, `childrenArgument`, `builderArgument`…: o argumento
    /// nomeado `nome` da criação.
    pub(crate) fn argumento_nomeado_flutter(&self, criacao: usize, nome: &str) -> Option<usize> {
        let lista = self.lista_de_argumentos_flutter(criacao)?;
        self.filhos(lista).iter().copied().find(|&a| self.especie(a) == "NamedExpression" && self.rotulo_do_nomeado(a) == Some(nome))
    }

    /// A expressão de uma `NamedExpression`.
    pub(crate) fn expressao_do_nomeado(&self, nomeado: usize) -> Option<usize> {
        self.filhos(nomeado).last().copied().filter(|&e| self.especie(e) != "Label")
    }

    /// `findArgumentNamed`: o identificador é o rótulo `nome:` de um
    /// argumento de uma criação de widget.
    fn argumento_nomeado_no_rotulo(&self, n: usize, nome: &str) -> Option<usize> {
        if self.especie(n) != "SimpleIdentifier" || self.texto_do_no(n) != nome {
            return None;
        }
        let rotulo = self.pai(n).filter(|&p| self.especie(p) == "Label")?;
        let nomeado = self.pai(rotulo).filter(|&p| self.especie(p) == "NamedExpression")?;
        let invocacao = self.pai(nomeado).and_then(|l| self.pai(l))?;
        (self.especie(invocacao) == "InstanceCreationExpression" && self.criacao_de_widget(invocacao)).then_some(nomeado)
    }

    /// `sessionHelper.getClass(uri, nome)`: a classe exportada pela
    /// biblioteca `uri`.
    pub(crate) fn classe_flutter(&self, uri: &str, nome: &str) -> Option<ClassId> {
        let p = self.p.programa();
        let lib = p.libraries.iter().position(|l| l.uri == uri)?;
        let s = self.p.consulta.nomes.lookup(nome)?;
        match p.library(dartforge_elements::model::LibraryId(lib as u32)).exported.get(&s)?.getter? {
            Element::Class(c) => Some(c),
            _ => None,
        }
    }

    fn acao_com_mudanca(&self, titulo: &str, especie: &str, m: Mudanca) -> Option<AcaoDeCodigo> {
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: titulo.into(),
            especie: especie.into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }

    /// As assistências do Flutter no cursor.
    pub(crate) fn assistencias_flutter(&self, uri: &str, inicio: usize, fim: usize) -> Vec<AcaoDeCodigo> {
        let Some(no) = self.arvore.localizar(inicio, fim) else { return Vec::new() };
        let mut v = Vec::new();
        v.extend(self.flutter_para_children(uri, no));
        v.extend(self.flutter_mover(uri, no, false));
        v.extend(self.flutter_mover(uri, no, true));
        v.extend(self.flutter_remover(uri, no));
        v.extend(self.flutter_trocar_com_filho(uri, no));
        v.extend(self.flutter_trocar_com_pai(uri, no));
        v.extend(self.flutter_embrulhar_builder(uri, no, false));
        v.extend(self.flutter_embrulhar_lista(uri, no));
        v.extend(self.flutter_embrulhar_builder(uri, no, true));
        v.extend(self.flutter_filho_por_ultimo(uri, no));
        v.extend(self.flutter_embrulhar(uri, no, inicio, fim));
        v
    }

    /// `FlutterConvertToChildren`.
    fn flutter_para_children(&self, uri: &str, n: usize) -> Option<AcaoDeCodigo> {
        if self.especie(n) != "SimpleIdentifier" || self.texto_do_no(n) != "child" {
            return None;
        }
        let rotulo = self.pai(n).filter(|&p| self.especie(p) == "Label")?;
        let nomeado = self.pai(rotulo).filter(|&p| self.especie(p) == "NamedExpression")?;
        // `node.element != null`: o rótulo resolve para o parâmetro.
        let invocacao = self.pai(nomeado).and_then(|l| self.pai(l))?;
        let f = match self.expr_do_no(invocacao).and_then(|x| self.corpos.get_resolved(x)) {
            Some(Resolved::Constructor(f)) | Some(Resolved::Element(Element::Function(f))) => *f,
            Some(Resolved::Member { member: dartforge_types::MemberRef::Function(f), .. }) => *f,
            _ => return None,
        };
        let child = self.p.consulta.nomes.lookup("child")?;
        if !self.p.consulta.outline.functions.get(f.0 as usize)?.parameters.iter().any(|q| q.kind == dartforge_frontend::ast::ParameterKind::Named && q.name == Some(child)) {
            return None;
        }
        let arg = self.expressao_do_nomeado(nomeado)?;
        if !self.e_expressao_widget(arg) {
            return None;
        }
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let mut m = Mudanca::default();
        let ini_nomeado = self.arvore.nos[nomeado].inicio;
        let (ini_arg, fim_arg) = (self.arvore.nos[arg].inicio, self.arvore.nos[arg].fim);
        let vazio = |o: usize| Span { start: o, end: o };
        m.adicionar(uri, vazio(ini_nomeado + "child".len()), "ren");
        let fonte_arg = self.texto_do_no(arg);
        if !fonte_arg.contains(eol) {
            m.adicionar(uri, vazio(ini_arg), "[");
            m.adicionar(uri, vazio(fim_arg), "]");
        } else {
            let mut nova_linha = fonte_arg.rfind(eol).unwrap_or(0);
            if nova_linha == fonte_arg.len() {
                nova_linha -= 1;
            }
            let recuo_velho = tx.prefixo_da_linha(ini_arg + eol.len() + nova_linha).to_string();
            let recuo_novo = format!("{recuo_velho}{UM_RECUO}");
            let separador = &self.fonte[ini_nomeado..ini_arg];
            let prefixo = if separador.contains(eol) { String::new() } else { format!("{eol}{recuo_novo}") };
            if prefixo.is_empty() {
                m.adicionar(uri, vazio(ini_nomeado + "child:".len()), " [");
                m.adicionar(uri, Span { start: ini_arg - 2, end: ini_arg }, "");
            } else {
                m.adicionar(uri, vazio(ini_arg), "[");
            }
            let novo = tx.trocar_recuo(fonte_arg, &recuo_velho, &recuo_novo, false, false);
            m.adicionar(uri, Span { start: ini_arg, end: fim_arg }, format!("{prefixo}{novo},{eol}{recuo_velho}]"));
        }
        self.acao_com_mudanca("Convert to children:", "refactor.flutter.convert.childToChildren", m)
    }

    /// `FlutterMoveUp`/`FlutterMoveDown`.
    fn flutter_mover(&self, uri: &str, n: usize, para_baixo: bool) -> Option<AcaoDeCodigo> {
        let widget = self.achar_expressao_widget(n)?;
        let lista = self.pai(widget).filter(|&p| self.especie(p) == "ListLiteral")?;
        let elementos: Vec<usize> = self.filhos(lista).iter().copied().filter(|&k| self.especie(k) != "TypeArgumentList").collect();
        let i = elementos.iter().position(|&k| k == widget)?;
        let outro = if para_baixo {
            if i + 1 == elementos.len() {
                return None;
            }
            elementos[i + 1]
        } else {
            if i == 0 {
                return None;
            }
            elementos[i - 1]
        };
        let mut m = Mudanca::default();
        m.adicionar(uri, self.arvore.span(outro), self.texto_do_no(widget));
        m.adicionar(uri, self.arvore.span(widget), self.texto_do_no(outro));
        let (titulo, especie) = if para_baixo { ("Move widget down", "refactor.flutter.move.down") } else { ("Move widget up", "refactor.flutter.move.up") };
        self.acao_com_mudanca(titulo, especie, m)
    }

    /// `FlutterRemoveWidget`.
    fn flutter_remover(&self, uri: &str, n: usize) -> Option<AcaoDeCodigo> {
        let criacao = self.achar_criacao(n)?;
        if !self.criacao_de_widget(criacao) {
            return None;
        }
        let elementos_da_lista = |lista: usize| -> Vec<usize> { self.filhos(lista).iter().copied().filter(|&k| self.especie(k) != "TypeArgumentList").collect() };
        let m = if let Some(a) = self.argumento_nomeado_flutter(criacao, "children") {
            let e = self.expressao_do_nomeado(a)?;
            if self.especie(e) != "ListLiteral" || elementos_da_lista(e).is_empty() {
                return None;
            }
            self.flutter_remover_filhos(uri, criacao, &elementos_da_lista(e))?
        } else if let Some(a) = self.argumento_nomeado_flutter(criacao, "child") {
            self.flutter_remover_um(uri, criacao, self.expressao_do_nomeado(a)?)
        } else if let Some(a) = self.argumento_nomeado_flutter(criacao, "builder") {
            let e = self.expressao_do_nomeado(a)?;
            if self.especie(e) != "FunctionExpression" {
                return None;
            }
            // O primeiro parâmetro do builder sem uso no corpo.
            let lista = self.filhos(e).iter().copied().find(|&k| self.especie(k) == "FormalParameterList")?;
            let parametro = *self.filhos(lista).first()?;
            let nome = self.token_anterior(self.arvore.nos[parametro].fim)?;
            let corpo = *self.filhos(e).last()?;
            // `_UsageFinder`: um identificador do corpo que lê o parâmetro.
            let usado = self.arvore.nos.iter().any(|no| {
                no.especie == "SimpleIdentifier"
                    && no.inicio >= self.arvore.nos[corpo].inicio
                    && no.fim <= self.arvore.nos[corpo].fim
                    && matches!(no.marca, crate::arvore_analyzer::Marca::Expr(x) if self.corpos.declaracao_local(x) == Some(nome.start))
            });
            if usado {
                return None;
            }
            let expressao = match self.especie(corpo) {
                "BlockFunctionBody" => {
                    let bloco = *self.filhos(corpo).first()?;
                    let [r] = self.filhos(bloco)[..] else { return None };
                    if self.especie(r) != "ReturnStatement" {
                        return None;
                    }
                    *self.filhos(r).first()?
                }
                "ExpressionFunctionBody" => *self.filhos(corpo).first()?,
                _ => return None,
            };
            self.flutter_remover_um(uri, criacao, expressao)
        } else if let Some(a) = self.argumento_nomeado_flutter(criacao, "slivers") {
            let e = self.expressao_do_nomeado(a)?;
            if self.especie(e) != "ListLiteral" || elementos_da_lista(e).is_empty() {
                return None;
            }
            self.flutter_remover_filhos(uri, criacao, &elementos_da_lista(e))?
        } else if let Some(a) = self.argumento_nomeado_flutter(criacao, "sliver") {
            self.flutter_remover_um(uri, criacao, self.expressao_do_nomeado(a)?)
        } else {
            // `_removeSingleWhenInList`.
            let lista = self.pai(criacao).filter(|&p| self.especie(p) == "ListLiteral")?;
            let mut m = Mudanca::default();
            m.adicionar(uri, self.no_em_lista(&elementos_da_lista(lista), criacao), "");
            m
        };
        self.acao_com_mudanca("Remove this widget", "refactor.flutter.removeWidget", m)
    }

    fn flutter_remover_filhos(&self, uri: &str, criacao: usize, filhos: &[usize]) -> Option<Mudanca> {
        if filhos.len() > 1 && self.pai(criacao).is_none_or(|p| self.especie(p) != "ListLiteral") {
            return None;
        }
        let tx = Texto::novo(self.fonte);
        let (primeiro, ultimo) = (*filhos.first()?, *filhos.last()?);
        let texto = &self.fonte[self.arvore.nos[primeiro].inicio..self.arvore.nos[ultimo].fim];
        let velho = tx.prefixo_da_linha(self.arvore.nos[primeiro].inicio);
        let novo = tx.prefixo_da_linha(self.arvore.nos[criacao].inicio);
        let mut m = Mudanca::default();
        m.adicionar(uri, self.arvore.span(criacao), tx.trocar_recuo(texto, velho, novo, false, false));
        Some(m)
    }

    fn flutter_remover_um(&self, uri: &str, criacao: usize, expressao: usize) -> Mudanca {
        let tx = Texto::novo(self.fonte);
        let velho = tx.prefixo_da_linha(self.arvore.nos[expressao].inicio);
        let novo = tx.prefixo_da_linha(self.arvore.nos[criacao].inicio);
        let mut m = Mudanca::default();
        m.adicionar(uri, self.arvore.span(criacao), tx.trocar_recuo(self.texto_do_no(expressao), velho, novo, false, false));
        m
    }

    /// `_singleChildInChildren`.
    fn filho_unico_em_children(&self, criacao: usize) -> Option<usize> {
        let a = self.argumento_nomeado_flutter(criacao, "children")?;
        let lista = self.expressao_do_nomeado(a).filter(|&e| self.especie(e) == "ListLiteral")?;
        let elementos: Vec<usize> = self.filhos(lista).iter().copied().filter(|&k| self.especie(k) != "TypeArgumentList").collect();
        let [unico] = elementos[..] else { return None };
        (self.especie(unico) == "InstanceCreationExpression").then_some(unico)
    }

    /// `FlutterSwapWithChild`.
    fn flutter_trocar_com_filho(&self, uri: &str, n: usize) -> Option<AcaoDeCodigo> {
        let pai = self.achar_criacao(n)?;
        if !self.criacao_de_widget(pai) {
            return None;
        }
        let mut pai_com_um_filho = true;
        let mut filho = None;
        if let Some(primeiro) = self.filho_unico_em_children(pai) {
            filho = Some(primeiro);
            pai_com_um_filho = false;
        }
        let filho = filho.or_else(|| self.argumento_nomeado_flutter(pai, "child").and_then(|a| self.expressao_do_nomeado(a)))?;
        if self.especie(filho) != "InstanceCreationExpression" || !self.criacao_de_widget(filho) {
            return None;
        }
        self.trocar_pai_e_filho(uri, pai, filho, pai_com_um_filho, "Swap with child", "refactor.flutter.swap.withChild")
    }

    /// `FlutterSwapWithParent`.
    fn flutter_trocar_com_pai(&self, uri: &str, n: usize) -> Option<AcaoDeCodigo> {
        let filho = self.achar_criacao(n)?;
        if !self.criacao_de_widget(filho) {
            return None;
        }
        let mut pai_com_um_filho = true;
        let mut nomeado = None;
        if let Some(lista) = self.pai(filho).filter(|&p| self.especie(p) == "ListLiteral") {
            let elementos = self.filhos(lista).iter().filter(|&&k| self.especie(k) != "TypeArgumentList").count();
            if elementos != 1 {
                return None;
            }
            if let Some(p) = self.pai(lista).filter(|&p| self.especie(p) == "NamedExpression") {
                nomeado = Some(p);
                pai_com_um_filho = false;
            }
        }
        let base = nomeado.or_else(|| self.pai(filho))?;
        let expr = self.pai(base).and_then(|x| self.pai(x))?;
        if self.especie(expr) != "InstanceCreationExpression" {
            return None;
        }
        self.trocar_pai_e_filho(uri, expr, filho, pai_com_um_filho, "Swap with parent", "refactor.flutter.swap.withParent")
    }

    /// `swapParentAndChild`.
    fn trocar_pai_e_filho(&self, uri: &str, pai: usize, filho: usize, pai_com_um_filho: bool, titulo: &str, especie: &str) -> Option<AcaoDeCodigo> {
        let estavel = match self.filho_unico_em_children(filho) {
            Some(primeiro) => primeiro,
            None => self.argumento_nomeado_flutter(filho, "child")?,
        };
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let args_filho = self.lista_de_argumentos_flutter(filho)?;
        let args_pai = self.lista_de_argumentos_flutter(pai)?;
        let texto_filho = &self.fonte[self.arvore.nos[filho].inicio..self.arvore.nos[args_filho].inicio];
        let texto_pai = &self.fonte[self.arvore.nos[pai].inicio..self.arvore.nos[args_pai].inicio];
        let recuo_pai = tx.prefixo_da_linha(self.arvore.nos[pai].inicio).to_string();
        let recuo_filho = format!("{recuo_pai}  ");
        let mut s = String::new();
        s.push_str(texto_filho);
        s.push('(');
        s.push_str(eol);
        for &a in self.filhos(args_filho) {
            if a != estavel {
                let texto = tx.trocar_recuo(self.texto_do_no(a), &recuo_filho, &recuo_pai, false, false);
                s.push_str(&format!("{recuo_pai}  {texto},{eol}"));
            }
        }
        s.push_str(&format!("{recuo_pai}  child: {texto_pai}({eol}"));
        for &a in self.filhos(args_pai) {
            let rotulo = if self.especie(a) == "NamedExpression" { self.rotulo_do_nomeado(a) } else { None };
            if !matches!(rotulo, Some("child") | Some("children")) {
                let texto = tx.trocar_recuo(self.texto_do_no(a), &recuo_pai, &recuo_filho, false, false);
                s.push_str(&format!("{recuo_filho}  {texto},{eol}"));
            }
        }
        {
            let mut texto = self.texto_do_no(estavel).to_string();
            if texto.trim().starts_with("child: ") {
                texto = texto["child: ".len()..].to_string();
            } else if texto.trim().starts_with("children: ") {
                texto = texto["children: ".len()..].to_string();
            }
            s.push_str(&recuo_filho);
            s.push_str("  ");
            if pai_com_um_filho {
                s.push_str("child: ");
            } else {
                s.push_str("children: [");
                s.push_str(eol);
                s.push_str(&recuo_filho);
                s.push_str("    ");
            }
            s.push_str(&texto);
            s.push(',');
            s.push_str(eol);
        }
        s.push_str(&recuo_filho);
        if !pai_com_um_filho {
            s.push_str("  ]");
            s.push(',');
            s.push_str(eol);
            s.push_str(&recuo_filho);
        }
        s.push_str("),");
        s.push_str(eol);
        s.push_str(&recuo_pai);
        s.push(')');
        let mut m = Mudanca::default();
        m.adicionar(uri, self.arvore.span(pai), s);
        self.acao_com_mudanca(titulo, especie, m)
    }

    /// `FlutterWrapBuilder` e `FlutterWrapStreamBuilder`.
    fn flutter_embrulhar_builder(&self, uri: &str, n: usize, stream: bool) -> Option<AcaoDeCodigo> {
        let widget = self.achar_expressao_widget(n)?;
        let t = self.tipo_do_no_flutter(widget)?;
        let (nome, uri_classe) = if stream { ("StreamBuilder", URI_ASYNC) } else { ("Builder", URI_BASIC) };
        if self.classe_do_tipo_flutter(t).is_some_and(|c| self.flutter_exatamente(c, nome, uri_classe)) {
            return None;
        }
        let classe = self.classe_flutter(URI_WIDGETS, nome)?;
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let mut escritor = crate::escrever_tipo::Escritor::novo(self, self.arvore.nos[widget].inicio);
        let referencia = escritor.referencia_de(Element::Class(classe), nome);
        let recuo = tx.prefixo_da_linha(self.arvore.nos[widget].inicio).to_string();
        let recuo1 = format!("{recuo}{UM_RECUO}");
        let recuo2 = format!("{recuo}{UM_RECUO}{UM_RECUO}");
        let virgulas = crate::refatoracoes_exec::regra_ligada(self.p, self.unidade, "require_trailing_commas");
        let fonte_widget = tx.trocar_recuo(self.texto_do_no(widget), &recuo, &recuo2, false, false);
        let mut s = referencia;
        if stream {
            s.push_str("<Object>(");
        } else {
            s.push('(');
        }
        s.push_str(eol);
        if stream {
            s.push_str(&format!("{recuo1}stream: null,{eol}"));
            s.push_str(&format!("{recuo1}builder: (context, snapshot) {{{eol}"));
        } else {
            s.push_str(&format!("{recuo1}builder: (context) {{{eol}"));
        }
        s.push_str(&format!("{recuo2}return {fonte_widget};{eol}"));
        s.push_str(&format!("{recuo1}}}{}{eol}", if virgulas { "," } else { "" }));
        s.push_str(&recuo);
        s.push(')');
        let mut m = Mudanca::default();
        m.adicionar(uri, self.arvore.span(widget), s);
        crate::refatoracoes_mover::imports_do_builder(self, &mut m, &escritor.importar);
        let (titulo, especie) = if stream { ("Wrap with StreamBuilder", "refactor.flutter.wrap.streamBuilder") } else { ("Wrap with Builder", "refactor.flutter.wrap.builder") };
        self.acao_com_mudanca(titulo, especie, m)
    }

    /// `FlutterWrapGeneric` (o produtor da lista de widgets).
    fn flutter_embrulhar_lista(&self, uri: &str, n: usize) -> Option<AcaoDeCodigo> {
        if self.especie(n) != "ListLiteral" {
            return None;
        }
        let elementos: Vec<usize> = self.filhos(n).iter().copied().filter(|&k| self.especie(k) != "TypeArgumentList").collect();
        if elementos.iter().any(|&e| !(self.especie(e) == "InstanceCreationExpression" && self.criacao_de_widget(e))) {
            return None;
        }
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let fonte = self.texto_do_no(n);
        let nova_linha = fonte.rfind(eol)?;
        if nova_linha == fonte.len() - 1 {
            return None;
        }
        let recuo = tx.prefixo_da_linha(self.arvore.nos[n].inicio + eol.len() + nova_linha).to_string();
        let recuo_arg = format!("{recuo}{UM_RECUO}");
        let recuo_lista = format!("{recuo}{UM_RECUO}{UM_RECUO}");
        let lista = tx.trocar_recuo(fonte, &recuo, &recuo_lista, false, false);
        let s = format!("[{eol}{recuo_arg}widget({eol}{recuo_lista}children: {lista},{eol}{recuo_arg}),{eol}{recuo}]");
        let mut m = Mudanca::default();
        m.adicionar(uri, self.arvore.span(n), s);
        self.acao_com_mudanca("Wrap with widget...", "refactor.flutter.wrap.generic", m)
    }

    /// `SortChildPropertyLast`.
    fn flutter_filho_por_ultimo(&self, uri: &str, n: usize) -> Option<AcaoDeCodigo> {
        let filho = if self.especie(n) == "NamedExpression" && matches!(self.rotulo_do_nomeado(n), Some("child") | Some("children")) {
            n
        } else {
            self.argumento_nomeado_no_rotulo(n, "child").or_else(|| self.argumento_nomeado_no_rotulo(n, "children"))?
        };
        let lista = self.pai(filho)?;
        let criacao = self.pai(lista)?;
        if self.especie(criacao) != "InstanceCreationExpression" || !self.criacao_de_widget(criacao) {
            return None;
        }
        let argumentos = self.filhos(lista).to_vec();
        let ultimo = *argumentos.last()?;
        if ultimo == filho {
            return None;
        }
        let depois = |k: usize| self.token_seguinte(self.arvore.nos[k].fim);
        let virgula_final = depois(ultimo).is_some_and(|t| &self.fonte[t.start..t.end] == ",");
        let ini_filho = self.token_anterior(self.arvore.nos[filho].inicio)?.end;
        let mut fim_filho = depois(filho)?.end;
        let mut exclusao = Span { start: ini_filho, end: fim_filho };
        if argumentos.first() == Some(&filho) {
            exclusao = Span { start: self.arvore.nos[filho].inicio, end: self.arvore.nos[*argumentos.get(1)?].inicio };
        }
        if !virgula_final {
            fim_filho = self.arvore.nos[filho].fim;
        }
        let mut texto = self.fonte[ini_filho..fim_filho].to_string();
        let mut insercao = self.arvore.nos[ultimo].fim;
        if virgula_final {
            insercao = depois(ultimo)?.end;
        } else if ini_filho == self.arvore.nos[filho].inicio {
            texto = format!(", {texto}");
        } else {
            texto = format!(",{texto}");
        }
        let mut m = Mudanca::default();
        m.adicionar(uri, exclusao, "");
        m.adicionar(uri, Span { start: insercao, end: insercao }, texto);
        self.acao_com_mudanca("Move child property to end of arguments", "refactor.sort.child.properties.last", m)
    }

    /// `FlutterWrap`: o embrulho genérico e os de `Center`, `Container`,
    /// `Padding`, `SizedBox` de um widget; `Column` e `Row` de um ou mais
    /// widgets selecionados.
    fn flutter_embrulhar(&self, uri: &str, n: usize, inicio: usize, fim: usize) -> Vec<AcaoDeCodigo> {
        let mut v = Vec::new();
        if let Some(widget) = self.achar_expressao_widget(n)
            && let Some(t) = self.tipo_do_no_flutter(widget)
        {
            let classe = self.classe_do_tipo_flutter(t);
            let exato = |nome: &str, uri_c: &str| classe.is_some_and(|c| self.flutter_exatamente(c, nome, uri_c));
            v.extend(self.embrulhar_um(uri, widget, None));
            if !exato("Center", URI_BASIC) {
                v.extend(self.embrulhar_um(uri, widget, Some("Center")));
            }
            if !exato("Container", URI_CONTAINER) {
                v.extend(self.embrulhar_um(uri, widget, Some("Container")));
            }
            if !exato("Padding", URI_BASIC) {
                v.extend(self.embrulhar_um(uri, widget, Some("Padding")));
            }
            if !exato("SizedBox", URI_BASIC) {
                v.extend(self.embrulhar_um(uri, widget, Some("SizedBox")));
            }
        }
        // `_wrapMultipleWidgets`.
        let analise = self.analisar(inicio, fim);
        let mut widgets: Vec<usize> = Vec::new();
        if !analise.selecionados.is_empty() {
            for &s in &analise.selecionados {
                let mut s = s;
                if self.especie(s) == "ConstructorName"
                    && let Some(p) = self.pai(s).filter(|&p| self.especie(p) == "InstanceCreationExpression")
                {
                    s = p;
                }
                if !self.e_expressao(s) || !self.e_expressao_widget(s) {
                    return v;
                }
                widgets.push(s);
            }
        } else if let Some(mut cobertura) = analise.cobertura {
            if self.especie(cobertura) == "ArgumentList"
                && self.arvore.nos[cobertura].inicio == inicio
                && let Some(p) = self.pai(cobertura)
            {
                cobertura = p;
            }
            if let Some(w) = self.achar_expressao_widget(cobertura) {
                widgets.push(w);
            }
        }
        let (Some(&primeiro), Some(&ultimo)) = (widgets.first(), widgets.last()) else { return v };
        for nome in ["Column", "Row"] {
            v.extend(self.embrulhar_varios(uri, primeiro, ultimo, nome));
        }
        v
    }

    /// `_WrapSingleWidget`.
    fn embrulhar_um(&self, uri: &str, widget: usize, nome: Option<&str>) -> Option<AcaoDeCodigo> {
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let mut fonte = self.texto_do_no(widget).to_string();
        let classe = match nome {
            Some(x) => Some(self.classe_flutter(URI_WIDGETS, x)?),
            None => None,
        };
        let mut escritor = crate::escrever_tipo::Escritor::novo(self, self.arvore.nos[widget].inicio);
        let mut s = match (classe, nome) {
            (Some(c), Some(x)) => escritor.referencia_de(Element::Class(c), x),
            _ => "widget".to_string(),
        };
        s.push('(');
        let linhas: Vec<String> = if nome == Some("Padding") {
            let palavra = if crate::criar::em_contexto_constante(self, widget) { "" } else { " const" };
            let valor = if crate::refatoracoes_exec::regra_ligada(self.p, self.unidade, "prefer_int_literals") { "8" } else { "8.0" };
            vec![format!("padding:{palavra} EdgeInsets.all({valor}),")]
        } else {
            Vec::new()
        };
        if fonte.contains(eol) || !linhas.is_empty() {
            let recuo = tx.prefixo_da_linha(self.arvore.nos[widget].inicio).to_string();
            let recuo_novo = format!("{recuo}{UM_RECUO}");
            for l in &linhas {
                s.push_str(eol);
                s.push_str(&recuo_novo);
                s.push_str(l);
            }
            s.push_str(eol);
            s.push_str(&recuo_novo);
            fonte = tx.trocar_recuo(&fonte, &recuo, &recuo_novo, false, false);
            fonte = format!("{fonte},{eol}{recuo}");
        }
        s.push_str("child: ");
        s.push_str(&fonte);
        s.push(')');
        let mut m = Mudanca::default();
        m.adicionar(uri, self.arvore.span(widget), s);
        crate::refatoracoes_mover::imports_do_builder(self, &mut m, &escritor.importar);
        let (titulo, especie) = match nome {
            None => ("Wrap with widget...".to_string(), "refactor.flutter.wrap.generic".to_string()),
            Some("SizedBox") => ("Wrap with SizedBox".to_string(), "refactor.flutter.wrap.sizedBox".to_string()),
            Some(x) => (format!("Wrap with {x}"), format!("refactor.flutter.wrap.{}", x.to_lowercase())),
        };
        self.acao_com_mudanca(&titulo, &especie, m)
    }

    /// `_WrapMultipleWidgets`.
    fn embrulhar_varios(&self, uri: &str, primeiro: usize, ultimo: usize, nome: &str) -> Option<AcaoDeCodigo> {
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let faixa = Span { start: self.arvore.nos[primeiro].inicio, end: self.arvore.nos[ultimo].fim };
        let fonte = &self.fonte[faixa.start..faixa.end];
        let classe = self.classe_flutter(URI_WIDGETS, nome)?;
        self.classe_flutter(URI_WIDGETS, "Widget")?;
        let mut escritor = crate::escrever_tipo::Escritor::novo(self, faixa.start);
        let mut s = escritor.referencia_de(Element::Class(classe), nome);
        s.push('(');
        let recuo = tx.prefixo_da_linha(faixa.start).to_string();
        let recuo1 = format!("{recuo}{UM_RECUO}");
        let recuo2 = format!("{recuo}{UM_RECUO}{UM_RECUO}");
        s.push_str(eol);
        s.push_str(&recuo1);
        s.push_str("children: [");
        s.push_str(eol);
        s.push_str(&recuo2);
        s.push_str(&tx.trocar_recuo(fonte, &recuo, &recuo2, false, false));
        s.push(',');
        s.push_str(eol);
        s.push_str(&recuo1);
        s.push_str("],");
        s.push_str(eol);
        s.push_str(&recuo);
        s.push(')');
        let mut m = Mudanca::default();
        m.adicionar(uri, faixa, s);
        crate::refatoracoes_mover::imports_do_builder(self, &mut m, &escritor.importar);
        self.acao_com_mudanca(&format!("Wrap with {nome}"), &format!("refactor.flutter.wrap.{}", nome.to_lowercase()), m)
    }
}
