//! Hierarquia de chamadas (`textDocument/prepareCallHierarchy`,
//! `callHierarchy/incomingCalls`, `callHierarchy/outgoingCalls`) como o
//! `DartCallHierarchyComputer` do Dart 3.6.2
//! (`AS:src/computer/computer_call_hierarchy.dart`, lido inteiro;
//! docs/LSP-ESPECIFICACAO.md §12) sobre a árvore do analyzer:
//!
//! * `_findTargetNode`: o nó do `NodeLocator` no offset; um
//!   `SimpleIdentifier` cujo pai não é `VariableDeclaration` nem atribuição
//!   sobe para o pai (`x` em `x.foo()` dá `foo`; `a` em `a == b`, o `==`);
//!   o nome do tipo antes do nome do construtor não é alvo;
//! * `_getElementOfNode`: o construtor de um `NamedType`/`ConstructorName`,
//!   a propriedade de um `PropertyAccess`, senão o `ElementLocator`; o
//!   acessor sintético não é alvo;
//! * o item de um elemento (`CallHierarchyItem.forElement`): o nome exibido
//!   (`get x`, `set x`, o nome do arquivo), a espécie, o contêiner, o
//!   código (com o comentário e as anotações) e o nome;
//! * recebidas: as referências do elemento agrupadas pelo contêiner;
//! * feitas: o `_OutboundCallVisitor` no corpo do alvo, agrupado pelo
//!   elemento chamado.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::arvore_analyzer::Marca;
use crate::projeto::{Alvo, Projeto};
use crate::refatoracoes::Contexto;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, UnitId};
use dartforge_frontend::ast;
use dartforge_types::{MemberRef, Resolved};

/// `CallHierarchyKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EspecieDeChamada {
    Classe,
    Construtor,
    Extensao,
    Arquivo,
    Funcao,
    Metodo,
    Mixin,
    Propriedade,
    Desconhecida,
}

impl EspecieDeChamada {
    /// `toSymbolKindMapping` (sem o recuo do cliente).
    pub fn simbolo(self) -> Option<u8> {
        Some(match self {
            EspecieDeChamada::Classe | EspecieDeChamada::Extensao | EspecieDeChamada::Mixin => 5,
            EspecieDeChamada::Construtor => 9,
            EspecieDeChamada::Arquivo => 1,
            EspecieDeChamada::Funcao => 12,
            EspecieDeChamada::Metodo => 6,
            EspecieDeChamada::Propriedade => 7,
            EspecieDeChamada::Desconhecida => return None,
        })
    }
}

/// Um item da hierarquia de chamadas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDeChamada {
    pub nome: String,
    pub especie: EspecieDeChamada,
    pub detalhe: Option<String>,
    pub uri: String,
    /// O código (com o comentário de documentação e as anotações).
    pub intervalo: Span,
    /// O nome.
    pub selecao: Span,
}

/// Um elemento da hierarquia (o que tem item).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ElemDeChamada {
    Funcao(FunctionElementId),
    /// Função local, pela unidade e a função do parser.
    Local(UnitId, u32),
    Classe(ClassId),
    Extensao(ExtensionId),
    Arquivo(UnitId),
}

impl Projeto {
    /// O `ElementKind` de um elemento, na forma de `CallHierarchyKind`.
    fn especie_de_chamada(&self, e: ElemDeChamada) -> EspecieDeChamada {
        let p = self.programa();
        match e {
            ElemDeChamada::Funcao(f) => match p.function(f).kind {
                FunctionKind::Constructor | FunctionKind::SyntheticConstructor => EspecieDeChamada::Construtor,
                FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => EspecieDeChamada::Propriedade,
                _ if p.function(f).class.is_none() && p.function(f).extension.is_none() => EspecieDeChamada::Funcao,
                _ => EspecieDeChamada::Metodo,
            },
            ElemDeChamada::Local(..) => EspecieDeChamada::Funcao,
            ElemDeChamada::Classe(c) => match p.class(c).kind {
                ClassKind::Mixin => EspecieDeChamada::Mixin,
                ClassKind::Enum | ClassKind::ExtensionType => EspecieDeChamada::Desconhecida,
                _ => EspecieDeChamada::Classe,
            },
            ElemDeChamada::Extensao(_) => EspecieDeChamada::Extensao,
            ElemDeChamada::Arquivo(_) => EspecieDeChamada::Arquivo,
        }
    }

    /// `_getDisplayName`.
    fn nome_exibido(&self, e: ElemDeChamada) -> String {
        let p = self.programa();
        match e {
            ElemDeChamada::Funcao(f) => {
                let fe = p.function(f);
                let base = crate::projeto::nome_base(self.nome(fe.name)).to_string();
                match fe.kind {
                    FunctionKind::Getter => format!("get {base}"),
                    FunctionKind::Setter => format!("set {base}"),
                    FunctionKind::ImplicitAccessor => {
                        if self.nome(fe.name).ends_with('=') { format!("set {base}") } else { format!("get {base}") }
                    }
                    FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
                        let classe = fe.class.map_or(String::new(), |c| self.nome(p.class(c).name).to_string());
                        if base.is_empty() { classe } else { format!("{classe}.{base}") }
                    }
                    _ => base,
                }
            }
            ElemDeChamada::Local(u, f) => {
                let a = &p.unit(u).ast;
                a.function(ast::FunctionId(f)).name.map(|n| self.nome(n.sym).to_string()).unwrap_or_default()
            }
            ElemDeChamada::Classe(c) => self.nome(p.class(c).name).to_string(),
            ElemDeChamada::Extensao(x) => p.extension(x).name.map(|n| self.nome(n).to_string()).unwrap_or_default(),
            ElemDeChamada::Arquivo(u) => p.unit(u).path.as_ref().and_then(|c| c.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        }
    }

    /// `_getContainer(element.enclosingElement)`: a classe, extensão ou
    /// mixin dono; senão o arquivo.
    fn conteiner_do_elemento(&self, e: ElemDeChamada) -> Option<ElemDeChamada> {
        let p = self.programa();
        match e {
            ElemDeChamada::Funcao(f) => {
                let fe = p.function(f);
                if let Some(c) = fe.class {
                    return Some(ElemDeChamada::Classe(c));
                }
                if let Some(x) = fe.extension {
                    return Some(ElemDeChamada::Extensao(x));
                }
                self.nome_da_funcao(f).map(|(u, _)| ElemDeChamada::Arquivo(u))
            }
            ElemDeChamada::Local(u, f) => {
                // A função ou o executável que contém a função local.
                let s = p.unit(u).ast.function(ast::FunctionId(f)).span;
                Some(self.conteiner_no_offset(u, s.start, Some(f)))
            }
            ElemDeChamada::Classe(c) => p.class(c).decl.map(|d| ElemDeChamada::Arquivo(d.unit)),
            ElemDeChamada::Extensao(x) => Some(ElemDeChamada::Arquivo(p.extension(x).decl.unit)),
            ElemDeChamada::Arquivo(_) => None,
        }
    }

    /// O nome e o código de um elemento (`nonSynthetic`).
    fn intervalos(&self, e: ElemDeChamada) -> Option<(UnitId, Span, Span)> {
        let p = self.programa();
        match e {
            ElemDeChamada::Funcao(f) => {
                let fe = p.function(f);
                if fe.kind == FunctionKind::SyntheticConstructor {
                    return self.intervalos(ElemDeChamada::Classe(fe.class?));
                }
                let (u, nome) = self.nome_da_funcao(f)?;
                let cx = Contexto::novo(self, u);
                let no = cx.arvore.localizar(nome.start, nome.end)?;
                let decl = cx.arvore.cadeia(no).into_iter().find(|&k| matches!(cx.especie(k), "FunctionDeclaration" | "MethodDeclaration" | "ConstructorDeclaration" | "VariableDeclaration"))?;
                Some((u, nome, cx.arvore.span(decl)))
            }
            ElemDeChamada::Local(u, f) => {
                let nome = p.unit(u).ast.function(ast::FunctionId(f)).name?.span;
                let cx = Contexto::novo(self, u);
                let no = cx.arvore.localizar(nome.start, nome.end)?;
                let decl = cx.arvore.cadeia(no).into_iter().find(|&k| cx.especie(k) == "FunctionDeclaration")?;
                Some((u, nome, cx.arvore.span(decl)))
            }
            ElemDeChamada::Classe(c) => {
                let (u, nome) = self.nome_do_elemento_de_topo(Element::Class(c))?;
                let d = p.class(c).decl?;
                let cx = Contexto::novo(self, u);
                let no = cx.arvore.nos.iter().position(|n| matches!(n.marca, Marca::Decl(x) if x == d.decl) && n.especie.ends_with("Declaration"))?;
                Some((u, nome, cx.arvore.span(no)))
            }
            ElemDeChamada::Extensao(x) => {
                let d = p.extension(x).decl;
                let cx = Contexto::novo(self, d.unit);
                let no = cx.arvore.nos.iter().position(|n| matches!(n.marca, Marca::Decl(k) if k == d.decl) && n.especie == "ExtensionDeclaration")?;
                let nome = match &p.unit(d.unit).ast.decl(d.decl).kind {
                    ast::DeclKind::Extension(e) => e.name.map_or(Span { start: cx.arvore.nos[no].inicio, end: cx.arvore.nos[no].inicio }, |n| n.span),
                    _ => return None,
                };
                Some((d.unit, nome, cx.arvore.span(no)))
            }
            ElemDeChamada::Arquivo(u) => Some((u, Span { start: 0, end: 0 }, Span { start: 0, end: p.unit(u).source.len() })),
        }
    }

    /// `CallHierarchyItem.forElement`.
    pub(crate) fn item_de_chamada(&self, e: ElemDeChamada) -> Option<ItemDeChamada> {
        let (u, selecao, intervalo) = self.intervalos(e)?;
        let detalhe = self.conteiner_do_elemento(e).map(|c| self.nome_exibido(c));
        Some(ItemDeChamada { nome: self.nome_exibido(e), especie: self.especie_de_chamada(e), detalhe, uri: self.uri_da_unidade(u)?, intervalo, selecao })
    }

    /// O elemento resolvido de um `SimpleIdentifier` da árvore.
    fn elemento_do_identificador_de_chamada(&self, cx: &Contexto<'_>, n: usize) -> Option<ElemDeChamada> {
        let x = match cx.arvore.nos[n].marca {
            Marca::Expr(x) => x,
            _ => return None,
        };
        if let Some(&f) = cx.construtores_por_alvo.get(&x) {
            return Some(ElemDeChamada::Funcao(f));
        }
        if let Some(d) = cx.corpos.declaracao_local(x)
            && let Some(i) = cx.ast.functions.iter().position(|f| f.name.is_some_and(|nm| nm.span.start == d))
        {
            return Some(ElemDeChamada::Local(cx.unidade, i as u32));
        }
        self.de_resolucao(cx.corpos.get_resolved(x)?)
    }

    fn de_resolucao(&self, r: &Resolved) -> Option<ElemDeChamada> {
        Some(match r {
            Resolved::Element(Element::Function(f)) | Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } | Resolved::Constructor(f) => {
                ElemDeChamada::Funcao(*f)
            }
            Resolved::Element(Element::Class(c)) => ElemDeChamada::Classe(*c),
            Resolved::Element(Element::Extension(x)) => ElemDeChamada::Extensao(*x),
            _ => return None,
        })
    }

    /// `ElementLocator.locate(node)` nas espécies que a hierarquia usa.
    fn localizar_elemento(&self, cx: &Contexto<'_>, n: usize) -> Option<ElemDeChamada> {
        match cx.especie(n) {
            "SimpleIdentifier" => self.elemento_do_identificador_de_chamada(cx, n),
            "MethodInvocation" => self.elemento_do_identificador_de_chamada(cx, cx.nome_do_metodo(n)?),
            "PrefixedIdentifier" => self.elemento_do_identificador_de_chamada(cx, *cx.filhos(n).get(1)?),
            "FunctionDeclaration" | "MethodDeclaration" => match cx.arvore.nos[n].marca {
                Marca::Funcao(fid) => {
                    if cx.pai(n).is_some_and(|p| cx.especie(p) == "FunctionDeclarationStatement") {
                        return Some(ElemDeChamada::Local(cx.unidade, fid.0));
                    }
                    self.funcao_do_no(cx.unidade, fid).map(ElemDeChamada::Funcao)
                }
                _ => None,
            },
            "ConstructorDeclaration" => {
                let s = cx.arvore.span(n);
                let (mi, _) = cx.ast.members.iter().enumerate().find(|(_, m)| matches!(m.kind, ast::MemberKind::Constructor(_)) && m.span.start >= s.start && m.span.end <= s.end)?;
                self.construtor_do_no(cx.unidade, ast::MemberId(mi as u32)).map(ElemDeChamada::Funcao)
            }
            "ClassDeclaration" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration" => match cx.arvore.nos[n].marca {
                Marca::Decl(d) => cx.classe_da_declaracao(cx.unidade, d).map(ElemDeChamada::Classe),
                _ => None,
            },
            "ExtensionDeclaration" => match cx.arvore.nos[n].marca {
                Marca::Decl(d) => {
                    let p = self.programa();
                    (0..p.extensions.len()).map(|i| ExtensionId(i as u32)).find(|&x| p.extension(x).decl.unit == cx.unidade && p.extension(x).decl.decl == d).map(ElemDeChamada::Extensao)
                }
                _ => None,
            },
            "InstanceCreationExpression" | "BinaryExpression" | "PrefixExpression" | "PostfixExpression" | "IndexExpression" | "AssignmentExpression" | "FunctionExpressionInvocation" => {
                let x = match cx.arvore.nos[n].marca {
                    Marca::Operador(x) => x,
                    _ => cx.expr_do_no(n)?,
                };
                self.de_resolucao(cx.corpos.get_resolved(x)?)
            }
            _ => None,
        }
    }

    /// `_findTargetNode(offset)`.
    fn no_alvo(&self, cx: &Contexto<'_>, offset: usize) -> Option<usize> {
        let mut n = cx.arvore.localizar(offset, offset)?;
        if cx.especie(n) == "SimpleIdentifier"
            && let Some(p) = cx.pai(n)
            && !matches!(cx.especie(p), "VariableDeclaration" | "AssignmentExpression")
        {
            n = p;
        }
        match cx.especie(n) {
            "NamedType" => {
                if let Some(p) = cx.pai(n)
                    && cx.especie(p) == "ConstructorName"
                    && let Some(&nome) = cx.filhos(p).get(1)
                    && offset < cx.arvore.nos[nome].inicio
                {
                    return None;
                }
            }
            "ConstructorDeclaration" => {
                let s = cx.arvore.span(n);
                let nome = cx.ast.members.iter().find_map(|m| match &m.kind {
                    ast::MemberKind::Constructor(k) if m.span.start >= s.start && m.span.end <= s.end => k.name,
                    _ => None,
                });
                if nome.is_some_and(|nm| offset < nm.span.start) {
                    return None;
                }
            }
            _ => {}
        }
        Some(n)
    }

    /// `_getElementOfNode(node)`: sem o acessor sintético.
    fn elemento_do_no_de_chamada(&self, cx: &Contexto<'_>, n: usize) -> Option<ElemDeChamada> {
        let pai = cx.pai(n);
        let e = match cx.especie(n) {
            "NamedType" if pai.is_some_and(|p| cx.especie(p) == "ConstructorName") => {
                let criacao = cx.pai(pai?)?;
                self.localizar_elemento(cx, criacao)
            }
            "ConstructorName" => self.localizar_elemento(cx, pai?),
            "PropertyAccess" => self.localizar_elemento(cx, *cx.filhos(n).last()?),
            _ => self.localizar_elemento(cx, n),
        }?;
        if let ElemDeChamada::Funcao(f) = e
            && self.programa().function(f).kind == FunctionKind::ImplicitAccessor
        {
            return None;
        }
        Some(e)
    }

    /// `findTarget(offset)`: só executáveis.
    pub(crate) fn alvo_de_chamada(&self, unidade: UnitId, offset: usize) -> Option<ElemDeChamada> {
        let cx = Contexto::novo(self, unidade);
        let n = self.no_alvo(&cx, offset)?;
        let e = self.elemento_do_no_de_chamada(&cx, n)?;
        matches!(e, ElemDeChamada::Funcao(_) | ElemDeChamada::Local(..)).then_some(e)
    }

    /// O alvo do item do cliente (`toServerItem` + o nó no nome + o
    /// `_isMatchingElement` pelo nome exibido); a classe com espécie de
    /// construtor é o construtor sem nome implícito. Com o nó, para as
    /// feitas.
    pub(crate) fn alvo_do_item(&self, unidade: UnitId, offset: usize, nome: &str, construtor: bool) -> Option<(ElemDeChamada, &'static str)> {
        let cx = Contexto::novo(self, unidade);
        let n = self.no_alvo(&cx, offset)?;
        let especie = cx.especie(n);
        let mut e = self.elemento_do_no_de_chamada(&cx, n)?;
        if self.nome_exibido(e) != nome {
            return None;
        }
        if let ElemDeChamada::Classe(c) = e
            && construtor
        {
            let p = self.programa();
            let sem_nome = p.class(c).constructors.iter().find(|(s, _)| self.nome(**s).is_empty()).map(|(_, f)| *f)?;
            e = ElemDeChamada::Funcao(sem_nome);
        }
        Some((e, especie))
    }

    /// O contêiner do índice para uma referência em `offset` de `u`: o
    /// executável nomeado mais interno (função, método, construtor, função
    /// local), a classe, a extensão, ou o arquivo.
    fn conteiner_no_offset(&self, u: UnitId, offset: usize, excluir: Option<u32>) -> ElemDeChamada {
        let p = self.programa();
        let cx = Contexto::novo(self, u);
        let Some(n) = cx.arvore.localizar_exclusivo(offset) else { return ElemDeChamada::Arquivo(u) };
        for k in cx.arvore.cadeia(n) {
            match (cx.especie(k), cx.arvore.nos[k].marca) {
                ("FunctionDeclaration" | "MethodDeclaration", Marca::Funcao(fid)) if Some(fid.0) != excluir => {
                    if cx.pai(k).is_some_and(|x| cx.especie(x) == "FunctionDeclarationStatement") {
                        return ElemDeChamada::Local(u, fid.0);
                    }
                    if let Some(f) = self.funcao_do_no(u, fid) {
                        return ElemDeChamada::Funcao(f);
                    }
                }
                ("ConstructorDeclaration", _) => {
                    if let Some(ElemDeChamada::Funcao(f)) = self.localizar_elemento(&cx, k) {
                        return ElemDeChamada::Funcao(f);
                    }
                }
                ("ClassDeclaration" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration", Marca::Decl(d)) => {
                    if let Some(c) = cx.classe_da_declaracao(u, d) {
                        return ElemDeChamada::Classe(c);
                    }
                }
                ("ExtensionDeclaration", Marca::Decl(d)) => {
                    if let Some(x) = (0..p.extensions.len()).map(|i| ExtensionId(i as u32)).find(|&x| p.extension(x).decl.unit == u && p.extension(x).decl.decl == d) {
                        return ElemDeChamada::Extensao(x);
                    }
                }
                _ => {}
            }
        }
        ElemDeChamada::Arquivo(u)
    }

    /// `findIncomingCalls`: as referências do executável, agrupadas pelo
    /// contêiner na ordem da primeira, com o intervalo de cada uma
    /// (`_rangeForSearchMatch`).
    pub(crate) fn chamadas_recebidas(&self, e: ElemDeChamada) -> Vec<(ItemDeChamada, Vec<Span>)> {
        let p = self.programa();
        let (alvo, declaracoes) = match e {
            ElemDeChamada::Funcao(f) => {
                let alvo = match p.function(f).kind {
                    FunctionKind::Constructor | FunctionKind::SyntheticConstructor => Alvo::Construtor(f),
                    _ if p.function(f).class.is_none() && p.function(f).extension.is_none() => Alvo::Topo(Element::Function(f)),
                    _ => self.membro_de_funcao(f),
                };
                let d = self.declaracoes(&alvo);
                (alvo, d)
            }
            ElemDeChamada::Local(u, fid) => {
                let Some(nome) = p.unit(u).ast.function(ast::FunctionId(fid)).name else { return Vec::new() };
                let alvo = Alvo::Local { unidade: u, declaracao: nome.span.start };
                let d = self.declaracoes(&alvo);
                (alvo, d)
            }
            _ => return Vec::new(),
        };
        let Ok(ocorrencias) = self.ocorrencias(&alvo, false) else { return Vec::new() };
        let mut grupos: Vec<(ElemDeChamada, Vec<Span>)> = Vec::new();
        for (u, de, ate) in ocorrencias {
            if declaracoes.contains(&(u, de, ate)) {
                continue;
            }
            let conteiner = self.conteiner_no_offset(u, de, None);
            // O intervalo: o nome do método numa `MethodInvocation` (o
            // construtor nomeado vem com o ponto no índice); o do nó num
            // intervalo vazio.
            let cx = Contexto::novo(self, u);
            let mut s = Span { start: de, end: ate };
            if let Some(n) = cx.arvore.localizar(de, de) {
                if cx.especie(n) == "SimpleIdentifier"
                    && let Some(pai) = cx.pai(n)
                    && cx.especie(pai) == "MethodInvocation"
                    && let Some(m) = cx.nome_do_metodo(pai)
                {
                    s = cx.arvore.span(m);
                } else if de == ate {
                    s = cx.arvore.span(n);
                }
            }
            match grupos.iter_mut().find(|(c, _)| *c == conteiner) {
                Some((_, l)) => l.push(s),
                None => grupos.push((conteiner, vec![s])),
            }
        }
        grupos.into_iter().filter_map(|(c, spans)| Some((self.item_de_chamada(c)?, spans))).collect()
    }

    /// `findOutgoingCalls`: o `_OutboundCallVisitor` no nó do alvo (só
    /// função, construtor ou método), agrupado pelo elemento chamado na
    /// ordem do primeiro.
    pub(crate) fn chamadas_feitas(&self, unidade: UnitId, offset: usize) -> Vec<(ItemDeChamada, Vec<Span>)> {
        let cx = Contexto::novo(self, unidade);
        let Some(raiz) = self.no_alvo(&cx, offset) else { return Vec::new() };
        if !matches!(cx.especie(raiz), "FunctionDeclaration" | "ConstructorDeclaration" | "MethodDeclaration") {
            return Vec::new();
        }
        // Os nós coletados, em ordem de visita (pré-ordem).
        let mut coletados: Vec<usize> = Vec::new();
        let mut pilha = vec![raiz];
        while let Some(k) = pilha.pop() {
            match cx.especie(k) {
                "FunctionDeclaration" if k != raiz => continue,
                "ConstructorName" => coletados.push(cx.filhos(k).get(1).copied().unwrap_or(k)),
                "FunctionReference" => coletados.push(k),
                "MethodInvocation" => {
                    if let Some(m) = cx.nome_do_metodo(k) {
                        coletados.push(m);
                    }
                }
                "PrefixedIdentifier" => {
                    if !cx.pai(k).is_some_and(|p| cx.especie(p) == "NamedType")
                        && let Some(&i) = cx.filhos(k).get(1)
                    {
                        coletados.push(i);
                    }
                }
                "PropertyAccess" => {
                    if let Some(&i) = cx.filhos(k).last() {
                        coletados.push(i);
                    }
                }
                "SimpleIdentifier" => {
                    // `staticElement is FunctionElement` (de topo ou local)
                    // fora de contexto de declaração.
                    let e = self.elemento_do_identificador_de_chamada(&cx, k);
                    let funcao = match e {
                        Some(ElemDeChamada::Local(..)) => true,
                        Some(ElemDeChamada::Funcao(f)) => {
                            let fe = self.programa().function(f);
                            fe.kind == FunctionKind::Function && fe.class.is_none() && fe.extension.is_none()
                        }
                        _ => false,
                    };
                    if funcao && !matches!(cx.arvore.nos[k].marca, Marca::ContextoDeDeclaracao) {
                        coletados.push(k);
                    }
                }
                _ => {}
            }
            for &f in cx.filhos(k).iter().rev() {
                pilha.push(f);
            }
        }
        // Um conjunto (`Set<AstNode>`): cada nó uma vez, na ordem de inserção.
        let mut vistos = std::collections::HashSet::new();
        coletados.retain(|n| vistos.insert(*n));
        let mut grupos: Vec<(ElemDeChamada, Vec<Span>)> = Vec::new();
        for n in coletados {
            let Some(e) = self.elemento_do_no_de_chamada(&cx, n) else { continue };
            let s = cx.arvore.span(n);
            match grupos.iter_mut().find(|(c, _)| *c == e) {
                Some((_, l)) => l.push(s),
                None => grupos.push((e, vec![s])),
            }
        }
        grupos.into_iter().filter_map(|(c, spans)| Some((self.item_de_chamada(c)?, spans))).collect()
    }
}
