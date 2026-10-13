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
use crate::project::{Target, Project};
use crate::refactorings::Context;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, UnitId};
use dartforge_frontend::ast;
use dartforge_types::{MemberRef, Resolved};

/// `CallHierarchyKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    Class,
    Constructor,
    Extension,
    File,
    Function,
    Method,
    Mixin,
    Property,
    Unknown,
}

impl CallKind {
    /// `toSymbolKindMapping` (sem o recuo do cliente).
    pub fn symbol(self) -> Option<u8> {
        Some(match self {
            CallKind::Class | CallKind::Extension | CallKind::Mixin => 5,
            CallKind::Constructor => 9,
            CallKind::File => 1,
            CallKind::Function => 12,
            CallKind::Method => 6,
            CallKind::Property => 7,
            CallKind::Unknown => return None,
        })
    }
}

/// Um item da hierarquia de chamadas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallItem {
    pub name: String,
    pub kind: CallKind,
    pub detail: Option<String>,
    pub uri: String,
    /// O código (com o comentário de documentação e as anotações).
    pub range: Span,
    /// O nome.
    pub selection: Span,
}

/// Um elemento da hierarquia (o que tem item).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum CallElem {
    Function(FunctionElementId),
    /// Função local, pela unidade e a função do parser.
    Local(UnitId, u32),
    Class(ClassId),
    Extension(ExtensionId),
    File(UnitId),
}

impl Project {
    /// O `ElementKind` de um elemento, na forma de `CallHierarchyKind`.
    fn call_kind(&self, e: CallElem) -> CallKind {
        let p = self.program();
        match e {
            CallElem::Function(f) => match p.function(f).kind {
                FunctionKind::Constructor | FunctionKind::SyntheticConstructor => CallKind::Constructor,
                FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => CallKind::Property,
                _ if p.function(f).class.is_none() && p.function(f).extension.is_none() => CallKind::Function,
                _ => CallKind::Method,
            },
            CallElem::Local(..) => CallKind::Function,
            CallElem::Class(c) => match p.class(c).kind {
                ClassKind::Mixin => CallKind::Mixin,
                ClassKind::Enum | ClassKind::ExtensionType => CallKind::Unknown,
                _ => CallKind::Class,
            },
            CallElem::Extension(_) => CallKind::Extension,
            CallElem::File(_) => CallKind::File,
        }
    }

    /// `_getDisplayName`.
    fn displayed_name(&self, e: CallElem) -> String {
        let p = self.program();
        match e {
            CallElem::Function(f) => {
                let fe = p.function(f);
                let base = crate::project::base_name(self.name(fe.name)).to_string();
                match fe.kind {
                    FunctionKind::Getter => format!("get {base}"),
                    FunctionKind::Setter => format!("set {base}"),
                    FunctionKind::ImplicitAccessor => {
                        if self.name(fe.name).ends_with('=') { format!("set {base}") } else { format!("get {base}") }
                    }
                    FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
                        let class = fe.class.map_or(String::new(), |c| self.name(p.class(c).name).to_string());
                        if base.is_empty() { class } else { format!("{class}.{base}") }
                    }
                    _ => base,
                }
            }
            CallElem::Local(u, f) => {
                let a = &p.unit(u).ast;
                a.function(ast::FunctionId(f)).name.map(|n| self.name(n.sym).to_string()).unwrap_or_default()
            }
            CallElem::Class(c) => self.name(p.class(c).name).to_string(),
            CallElem::Extension(x) => p.extension(x).name.map(|n| self.name(n).to_string()).unwrap_or_default(),
            CallElem::File(u) => p.unit(u).path.as_ref().and_then(|c| c.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        }
    }

    /// `_getContainer(element.enclosingElement)`: a classe, extensão ou
    /// mixin dono; senão o arquivo.
    fn element_container(&self, e: CallElem) -> Option<CallElem> {
        let p = self.program();
        match e {
            CallElem::Function(f) => {
                let fe = p.function(f);
                if let Some(c) = fe.class {
                    return Some(CallElem::Class(c));
                }
                if let Some(x) = fe.extension {
                    return Some(CallElem::Extension(x));
                }
                self.function_name(f).map(|(u, _)| CallElem::File(u))
            }
            CallElem::Local(u, f) => {
                // A função ou o executável que contém a função local.
                let s = p.unit(u).ast.function(ast::FunctionId(f)).span;
                Some(self.container_at_offset(u, s.start, Some(f)))
            }
            CallElem::Class(c) => p.class(c).decl.map(|d| CallElem::File(d.unit)),
            CallElem::Extension(x) => Some(CallElem::File(p.extension(x).decl.unit)),
            CallElem::File(_) => None,
        }
    }

    /// O nome e o código de um elemento (`nonSynthetic`).
    fn ranges(&self, e: CallElem) -> Option<(UnitId, Span, Span)> {
        let p = self.program();
        match e {
            CallElem::Function(f) => {
                let fe = p.function(f);
                if fe.kind == FunctionKind::SyntheticConstructor {
                    return self.ranges(CallElem::Class(fe.class?));
                }
                let (u, name) = self.function_name(f)?;
                let cx = Context::new(self, u);
                let no = cx.tree.localizar(name.start, name.end)?;
                let decl = cx.tree.cadeia(no).into_iter().find(|&k| matches!(cx.kind(k), "FunctionDeclaration" | "MethodDeclaration" | "ConstructorDeclaration" | "VariableDeclaration"))?;
                Some((u, name, cx.tree.span(decl)))
            }
            CallElem::Local(u, f) => {
                let name = p.unit(u).ast.function(ast::FunctionId(f)).name?.span;
                let cx = Context::new(self, u);
                let no = cx.tree.localizar(name.start, name.end)?;
                let decl = cx.tree.cadeia(no).into_iter().find(|&k| cx.kind(k) == "FunctionDeclaration")?;
                Some((u, name, cx.tree.span(decl)))
            }
            CallElem::Class(c) => {
                let (u, name) = self.top_element_name(Element::Class(c))?;
                let d = p.class(c).decl?;
                let cx = Context::new(self, u);
                let no = cx.tree.nos.iter().position(|n| matches!(n.marca, Marca::Decl(x) if x == d.decl) && n.especie.ends_with("Declaration"))?;
                Some((u, name, cx.tree.span(no)))
            }
            CallElem::Extension(x) => {
                let d = p.extension(x).decl;
                let cx = Context::new(self, d.unit);
                let no = cx.tree.nos.iter().position(|n| matches!(n.marca, Marca::Decl(k) if k == d.decl) && n.especie == "ExtensionDeclaration")?;
                let name = match &p.unit(d.unit).ast.decl(d.decl).kind {
                    ast::DeclKind::Extension(e) => e.name.map_or(Span { start: cx.tree.nos[no].inicio, end: cx.tree.nos[no].inicio }, |n| n.span),
                    _ => return None,
                };
                Some((d.unit, name, cx.tree.span(no)))
            }
            CallElem::File(u) => Some((u, Span { start: 0, end: 0 }, Span { start: 0, end: p.unit(u).source.len() })),
        }
    }

    /// `CallHierarchyItem.forElement`.
    pub(crate) fn call_item(&self, e: CallElem) -> Option<CallItem> {
        let (u, selection, range) = self.ranges(e)?;
        let detail = self.element_container(e).map(|c| self.displayed_name(c));
        Some(CallItem { name: self.displayed_name(e), kind: self.call_kind(e), detail, uri: self.unit_uri(u)?, range, selection })
    }

    /// O elemento resolvido de um `SimpleIdentifier` da árvore.
    fn call_identifier_element(&self, cx: &Context<'_>, n: usize) -> Option<CallElem> {
        let x = match cx.tree.nos[n].marca {
            Marca::Expr(x) => x,
            _ => return None,
        };
        if let Some(&f) = cx.constructors_by_target.get(&x) {
            return Some(CallElem::Function(f));
        }
        if let Some(d) = cx.bodies.declaracao_local(x)
            && let Some(i) = cx.ast.functions.iter().position(|f| f.name.is_some_and(|nm| nm.span.start == d))
        {
            return Some(CallElem::Local(cx.unit, i as u32));
        }
        self.of_resolution(cx.bodies.get_resolved(x)?)
    }

    fn of_resolution(&self, r: &Resolved) -> Option<CallElem> {
        Some(match r {
            Resolved::Element(Element::Function(f)) | Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } | Resolved::Constructor(f) => {
                CallElem::Function(*f)
            }
            Resolved::Element(Element::Class(c)) => CallElem::Class(*c),
            Resolved::Element(Element::Extension(x)) => CallElem::Extension(*x),
            _ => return None,
        })
    }

    /// `ElementLocator.locate(node)` nas espécies que a hierarquia usa.
    fn locate_element(&self, cx: &Context<'_>, n: usize) -> Option<CallElem> {
        match cx.kind(n) {
            "SimpleIdentifier" => self.call_identifier_element(cx, n),
            "MethodInvocation" => self.call_identifier_element(cx, cx.method_name(n)?),
            "PrefixedIdentifier" => self.call_identifier_element(cx, *cx.children(n).get(1)?),
            "FunctionDeclaration" | "MethodDeclaration" => match cx.tree.nos[n].marca {
                Marca::Funcao(fid) => {
                    if cx.parent(n).is_some_and(|p| cx.kind(p) == "FunctionDeclarationStatement") {
                        return Some(CallElem::Local(cx.unit, fid.0));
                    }
                    self.node_function(cx.unit, fid).map(CallElem::Function)
                }
                _ => None,
            },
            "ConstructorDeclaration" => {
                let s = cx.tree.span(n);
                let (mi, _) = cx.ast.members.iter().enumerate().find(|(_, m)| matches!(m.kind, ast::MemberKind::Constructor(_)) && m.span.start >= s.start && m.span.end <= s.end)?;
                self.node_constructor(cx.unit, ast::MemberId(mi as u32)).map(CallElem::Function)
            }
            "ClassDeclaration" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration" => match cx.tree.nos[n].marca {
                Marca::Decl(d) => cx.declaration_class(cx.unit, d).map(CallElem::Class),
                _ => None,
            },
            "ExtensionDeclaration" => match cx.tree.nos[n].marca {
                Marca::Decl(d) => {
                    let p = self.program();
                    (0..p.extensions.len()).map(|i| ExtensionId(i as u32)).find(|&x| p.extension(x).decl.unit == cx.unit && p.extension(x).decl.decl == d).map(CallElem::Extension)
                }
                _ => None,
            },
            "InstanceCreationExpression" | "BinaryExpression" | "PrefixExpression" | "PostfixExpression" | "IndexExpression" | "AssignmentExpression" | "FunctionExpressionInvocation" => {
                let x = match cx.tree.nos[n].marca {
                    Marca::Operador(x) => x,
                    _ => cx.node_expr(n)?,
                };
                self.of_resolution(cx.bodies.get_resolved(x)?)
            }
            _ => None,
        }
    }

    /// `_findTargetNode(offset)`.
    fn at_target(&self, cx: &Context<'_>, offset: usize) -> Option<usize> {
        let mut n = cx.tree.localizar(offset, offset)?;
        if cx.kind(n) == "SimpleIdentifier"
            && let Some(p) = cx.parent(n)
            && !matches!(cx.kind(p), "VariableDeclaration" | "AssignmentExpression")
        {
            n = p;
        }
        match cx.kind(n) {
            "NamedType" => {
                if let Some(p) = cx.parent(n)
                    && cx.kind(p) == "ConstructorName"
                    && let Some(&name) = cx.children(p).get(1)
                    && offset < cx.tree.nos[name].inicio
                {
                    return None;
                }
            }
            "ConstructorDeclaration" => {
                let s = cx.tree.span(n);
                let name = cx.ast.members.iter().find_map(|m| match &m.kind {
                    ast::MemberKind::Constructor(k) if m.span.start >= s.start && m.span.end <= s.end => k.name,
                    _ => None,
                });
                if name.is_some_and(|nm| offset < nm.span.start) {
                    return None;
                }
            }
            _ => {}
        }
        Some(n)
    }

    /// `_getElementOfNode(node)`: sem o acessor sintético.
    fn call_node_element(&self, cx: &Context<'_>, n: usize) -> Option<CallElem> {
        let parent = cx.parent(n);
        let e = match cx.kind(n) {
            "NamedType" if parent.is_some_and(|p| cx.kind(p) == "ConstructorName") => {
                let creation = cx.parent(parent?)?;
                self.locate_element(cx, creation)
            }
            "ConstructorName" => self.locate_element(cx, parent?),
            "PropertyAccess" => self.locate_element(cx, *cx.children(n).last()?),
            _ => self.locate_element(cx, n),
        }?;
        if let CallElem::Function(f) = e
            && self.program().function(f).kind == FunctionKind::ImplicitAccessor
        {
            return None;
        }
        Some(e)
    }

    /// `findTarget(offset)`: só executáveis.
    pub(crate) fn call_target(&self, unit: UnitId, offset: usize) -> Option<CallElem> {
        let cx = Context::new(self, unit);
        let n = self.at_target(&cx, offset)?;
        let e = self.call_node_element(&cx, n)?;
        matches!(e, CallElem::Function(_) | CallElem::Local(..)).then_some(e)
    }

    /// O alvo do item do cliente (`toServerItem` + o nó no nome + o
    /// `_isMatchingElement` pelo nome exibido); a classe com espécie de
    /// construtor é o construtor sem nome implícito. Com o nó, para as
    /// feitas.
    pub(crate) fn item_target(&self, unit: UnitId, offset: usize, name: &str, constructor: bool) -> Option<(CallElem, &'static str)> {
        let cx = Context::new(self, unit);
        let n = self.at_target(&cx, offset)?;
        let kind = cx.kind(n);
        let mut e = self.call_node_element(&cx, n)?;
        if self.displayed_name(e) != name {
            return None;
        }
        if let CallElem::Class(c) = e
            && constructor
        {
            let p = self.program();
            let unnamed = p.class(c).constructors.iter().find(|(s, _)| self.name(**s).is_empty()).map(|(_, f)| *f)?;
            e = CallElem::Function(unnamed);
        }
        Some((e, kind))
    }

    /// O contêiner do índice para uma referência em `offset` de `u`: o
    /// executável nomeado mais interno (função, método, construtor, função
    /// local), a classe, a extensão, ou o arquivo.
    fn container_at_offset(&self, u: UnitId, offset: usize, exclude: Option<u32>) -> CallElem {
        let p = self.program();
        let cx = Context::new(self, u);
        let Some(n) = cx.tree.localizar_exclusivo(offset) else { return CallElem::File(u) };
        for k in cx.tree.cadeia(n) {
            match (cx.kind(k), cx.tree.nos[k].marca) {
                ("FunctionDeclaration" | "MethodDeclaration", Marca::Funcao(fid)) if Some(fid.0) != exclude => {
                    if cx.parent(k).is_some_and(|x| cx.kind(x) == "FunctionDeclarationStatement") {
                        return CallElem::Local(u, fid.0);
                    }
                    if let Some(f) = self.node_function(u, fid) {
                        return CallElem::Function(f);
                    }
                }
                ("ConstructorDeclaration", _) => {
                    if let Some(CallElem::Function(f)) = self.locate_element(&cx, k) {
                        return CallElem::Function(f);
                    }
                }
                ("ClassDeclaration" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration", Marca::Decl(d)) => {
                    if let Some(c) = cx.declaration_class(u, d) {
                        return CallElem::Class(c);
                    }
                }
                ("ExtensionDeclaration", Marca::Decl(d)) => {
                    if let Some(x) = (0..p.extensions.len()).map(|i| ExtensionId(i as u32)).find(|&x| p.extension(x).decl.unit == u && p.extension(x).decl.decl == d) {
                        return CallElem::Extension(x);
                    }
                }
                _ => {}
            }
        }
        CallElem::File(u)
    }

    /// `findIncomingCalls`: as referências do executável, agrupadas pelo
    /// contêiner na ordem da primeira, com o intervalo de cada uma
    /// (`_rangeForSearchMatch`).
    pub(crate) fn incoming_calls(&self, e: CallElem) -> Vec<(CallItem, Vec<Span>)> {
        let p = self.program();
        let (target, declarations) = match e {
            CallElem::Function(f) => {
                let target = match p.function(f).kind {
                    FunctionKind::Constructor | FunctionKind::SyntheticConstructor => Target::Constructor(f),
                    _ if p.function(f).class.is_none() && p.function(f).extension.is_none() => Target::Top(Element::Function(f)),
                    _ => self.function_member(f),
                };
                let d = self.declarations(&target);
                (target, d)
            }
            CallElem::Local(u, fid) => {
                let Some(name) = p.unit(u).ast.function(ast::FunctionId(fid)).name else { return Vec::new() };
                let target = Target::Local { unit: u, declaration: name.span.start };
                let d = self.declarations(&target);
                (target, d)
            }
            _ => return Vec::new(),
        };
        let Ok(occurrences) = self.occurrences(&target, false) else { return Vec::new() };
        let mut groups: Vec<(CallElem, Vec<Span>)> = Vec::new();
        for (u, de, until) in occurrences {
            if declarations.contains(&(u, de, until)) {
                continue;
            }
            let container = self.container_at_offset(u, de, None);
            // O intervalo: o nome do método numa `MethodInvocation` (o
            // construtor nomeado vem com o ponto no índice); o do nó num
            // intervalo vazio.
            let cx = Context::new(self, u);
            let mut s = Span { start: de, end: until };
            if let Some(n) = cx.tree.localizar(de, de) {
                if cx.kind(n) == "SimpleIdentifier"
                    && let Some(parent) = cx.parent(n)
                    && cx.kind(parent) == "MethodInvocation"
                    && let Some(m) = cx.method_name(parent)
                {
                    s = cx.tree.span(m);
                } else if de == until {
                    s = cx.tree.span(n);
                }
            }
            match groups.iter_mut().find(|(c, _)| *c == container) {
                Some((_, l)) => l.push(s),
                None => groups.push((container, vec![s])),
            }
        }
        groups.into_iter().filter_map(|(c, spans)| Some((self.call_item(c)?, spans))).collect()
    }

    /// `findOutgoingCalls`: o `_OutboundCallVisitor` no nó do alvo (só
    /// função, construtor ou método), agrupado pelo elemento chamado na
    /// ordem do primeiro.
    pub(crate) fn outgoing_calls(&self, unit: UnitId, offset: usize) -> Vec<(CallItem, Vec<Span>)> {
        let cx = Context::new(self, unit);
        let Some(root) = self.at_target(&cx, offset) else { return Vec::new() };
        if !matches!(cx.kind(root), "FunctionDeclaration" | "ConstructorDeclaration" | "MethodDeclaration") {
            return Vec::new();
        }
        // Os nós coletados, em ordem de visita (pré-ordem).
        let mut collected: Vec<usize> = Vec::new();
        let mut stack = vec![root];
        while let Some(k) = stack.pop() {
            match cx.kind(k) {
                "FunctionDeclaration" if k != root => continue,
                "ConstructorName" => collected.push(cx.children(k).get(1).copied().unwrap_or(k)),
                "FunctionReference" => collected.push(k),
                "MethodInvocation" => {
                    if let Some(m) = cx.method_name(k) {
                        collected.push(m);
                    }
                }
                "PrefixedIdentifier" => {
                    if !cx.parent(k).is_some_and(|p| cx.kind(p) == "NamedType")
                        && let Some(&i) = cx.children(k).get(1)
                    {
                        collected.push(i);
                    }
                }
                "PropertyAccess" => {
                    if let Some(&i) = cx.children(k).last() {
                        collected.push(i);
                    }
                }
                "SimpleIdentifier" => {
                    // `staticElement is FunctionElement` (de topo ou local)
                    // fora de contexto de declaração.
                    let e = self.call_identifier_element(&cx, k);
                    let function = match e {
                        Some(CallElem::Local(..)) => true,
                        Some(CallElem::Function(f)) => {
                            let fe = self.program().function(f);
                            fe.kind == FunctionKind::Function && fe.class.is_none() && fe.extension.is_none()
                        }
                        _ => false,
                    };
                    if function && !matches!(cx.tree.nos[k].marca, Marca::ContextoDeDeclaracao) {
                        collected.push(k);
                    }
                }
                _ => {}
            }
            for &f in cx.children(k).iter().rev() {
                stack.push(f);
            }
        }
        // Um conjunto (`Set<AstNode>`): cada nó uma vez, na ordem de inserção.
        let mut seen = std::collections::HashSet::new();
        collected.retain(|n| seen.insert(*n));
        let mut groups: Vec<(CallElem, Vec<Span>)> = Vec::new();
        for n in collected {
            let Some(e) = self.call_node_element(&cx, n) else { continue };
            let s = cx.tree.span(n);
            match groups.iter_mut().find(|(c, _)| *c == e) {
                Some((_, l)) => l.push(s),
                None => groups.push((e, vec![s])),
            }
        }
        groups.into_iter().filter_map(|(c, spans)| Some((self.call_item(c)?, spans))).collect()
    }
}
