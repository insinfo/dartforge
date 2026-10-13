//! `computeReferencedNames` (`AN:src/dart/analysis/referenced_names.dart`,
//! docs/LSP-ESPECIFICACAO.md §11.2): os nomes externos que um arquivo cita,
//! sobre a árvore no formato do analyzer. Decide os arquivos candidatos de
//! uma busca de referências (`Search._addResults`).
//!
//! Entram o nome de todo `NamedType` e de todo `SimpleIdentifier`, menos:
//! identificador em contexto de declaração; o `returnType` de um
//! construtor; nome não qualificado sombreado pelo escopo local sintático
//! (`_LocalNameScope`: topo do arquivo, classe, extension type, construtor,
//! função, método, `typedef` e bloco); nome igual a um prefixo de import já
//! visto; e tudo dentro do `ConstructorName` de um redirecionamento de
//! fábrica.

use crate::arvore_analyzer::Marca;
use crate::project::{Target, Project};
use crate::refactorings::Context;
use dartforge_elements::model::{Element, UnitId, UnitRole};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind};
use std::collections::HashSet;
use std::sync::Arc;

/// O índice reverso do `referencedNames`: para cada nome, as unidades que o
/// citam. Construído sob demanda, unidade por unidade (as inferidas que
/// ainda não estão nele), e atualizado por unidade quando o texto dela muda
/// ([`Project::forget_unit`]): a busca de referências acha os arquivos
/// candidatos sem percorrer todos.
#[derive(Debug, Default)]
pub(crate) struct ReverseIndex {
    pub(crate) indexed: HashSet<UnitId>,
    pub(crate) by_name: std::collections::HashMap<String, std::collections::BTreeSet<UnitId>>,
}

/// O identificador que começa em `pos`.
fn identifier_in(source: &str, pos: usize) -> &str {
    let rest = &source[pos.min(source.len())..];
    let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(rest.len());
    &rest[..end]
}

struct Computer<'c, 'p> {
    cx: &'c Context<'p>,
    names: HashSet<String>,
    prefixes: HashSet<String>,
    scopes: Vec<HashSet<String>>,
}

impl Computer<'_, '_> {
    fn name(&self, n: ast::Name) -> String {
        self.cx.p.name(n.sym).to_string()
    }

    fn contains(&self, name: &str) -> bool {
        self.scopes.iter().any(|e| e.contains(name))
    }

    /// `addTypeParameters`.
    fn type_parameters(&self, e: &mut HashSet<String>, tps: &[ast::TypeParameter]) {
        for t in tps {
            e.insert(self.name(t.name));
        }
    }

    /// `addFormalParameters`: só os `NormalFormalParameter` (o opcional e o
    /// nomeado vêm embrulhados num `DefaultFormalParameter` e ficam de fora).
    fn parameters(&self, e: &mut HashSet<String>, ps: &[ast::Parameter]) {
        for p in ps {
            if p.kind == ParameterKind::Required
                && let Some(n) = p.name
            {
                e.insert(self.name(n));
            }
        }
    }

    /// `forClass`/`forExtensionType`: parâmetros de tipo, campos e métodos.
    fn members(&self, e: &mut HashSet<String>, tps: &[ast::TypeParameter], members: &[ast::MemberId]) {
        self.type_parameters(e, tps);
        let ast = self.cx.ast;
        for &mid in members {
            match &ast.member(mid).kind {
                MemberKind::Field(l) => {
                    for v in l.variables.iter() {
                        e.insert(self.name(v.name));
                    }
                }
                MemberKind::Method(fid) => {
                    if let Some(n) = ast.function(*fid).name {
                        e.insert(self.name(n));
                    }
                }
                MemberKind::Constructor(_) => {}
            }
        }
    }

    /// `forUnit`: os `NamedCompilationUnitMember` (extensão não é) e as
    /// variáveis de topo do próprio arquivo.
    fn unit_scope(&self) -> HashSet<String> {
        let mut e = HashSet::new();
        let ast = self.cx.ast;
        for &did in &self.cx.p.program().unit(self.cx.unit).unit.declarations {
            match &ast.decl(did).kind {
                DeclKind::Class(c) => {
                    e.insert(self.name(c.name));
                }
                DeclKind::Mixin(m) => {
                    e.insert(self.name(m.name));
                }
                DeclKind::Enum(x) => {
                    e.insert(self.name(x.name));
                }
                DeclKind::ExtensionType(x) => {
                    e.insert(self.name(x.name));
                }
                DeclKind::Typedef(t) => {
                    e.insert(self.name(t.name));
                }
                DeclKind::Function(fid) => {
                    if let Some(n) = ast.function(*fid).name {
                        e.insert(self.name(n));
                    }
                }
                DeclKind::Variables(l) => {
                    for v in l.variables.iter() {
                        e.insert(self.name(v.name));
                    }
                }
                DeclKind::Extension(_) => {}
            }
        }
        e
    }

    /// A função marcada no nó ou no `FunctionExpression` filho.
    fn function(&self, k: usize) -> Option<ast::FunctionId> {
        let cx = self.cx;
        std::iter::once(k).chain(cx.children(k).iter().copied()).find_map(|n| match cx.tree.nos[n].marca {
            Marca::Funcao(f) => Some(f),
            _ => None,
        })
    }

    /// O escopo que o nó `k` abre, se abre.
    fn node_scope(&self, k: usize) -> Option<HashSet<String>> {
        let cx = self.cx;
        let ast = cx.ast;
        let mut e = HashSet::new();
        match cx.kind(k) {
            // `forBlock`: funções e variáveis declaradas como instruções
            // diretas do bloco.
            "Block" => {
                for &s in cx.children(k) {
                    match cx.kind(s) {
                        "FunctionDeclarationStatement" => {
                            if let Some(f) = cx.children(s).first().and_then(|&d| self.function(d))
                                && let Some(n) = ast.function(f).name
                            {
                                e.insert(self.name(n));
                            }
                        }
                        "VariableDeclarationStatement" => {
                            for &list in cx.children(s) {
                                for &v in cx.children(list) {
                                    if cx.kind(v) == "VariableDeclaration" {
                                        e.insert(identifier_in(cx.source, cx.tree.nos[v].inicio).to_string());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "ClassDeclaration" | "ClassTypeAlias" | "ExtensionTypeDeclaration" | "FunctionTypeAlias" => {
                let Marca::Decl(did) = cx.tree.nos[k].marca else { return Some(e) };
                match &ast.decl(did).kind {
                    DeclKind::Class(c) if c.mixin_application => self.type_parameters(&mut e, &c.type_params),
                    DeclKind::Class(c) => self.members(&mut e, &c.type_params, &c.members),
                    DeclKind::ExtensionType(x) => self.members(&mut e, &x.type_params, &x.members),
                    DeclKind::Typedef(t) => self.type_parameters(&mut e, &t.type_params),
                    _ => {}
                }
            }
            // `forConstructor`: só os parâmetros.
            "ConstructorDeclaration" => {
                let end = cx.tree.nos[k].fim;
                let owner = cx.parent(k).and_then(|p| match cx.tree.nos[p].marca {
                    Marca::Decl(d) => Some(d),
                    _ => None,
                });
                let members: &[ast::MemberId] = match owner.map(|d| &ast.decl(d).kind) {
                    Some(DeclKind::Class(c)) => &c.members,
                    Some(DeclKind::Enum(x)) => &x.members,
                    Some(DeclKind::Mixin(x)) => &x.members,
                    Some(DeclKind::ExtensionType(x)) => &x.members,
                    Some(DeclKind::Extension(x)) => &x.members,
                    _ => &[],
                };
                for &mid in members {
                    let m = ast.member(mid);
                    if let MemberKind::Constructor(c) = &m.kind
                        && m.span.end == end
                    {
                        self.parameters(&mut e, &c.parameters);
                        break;
                    }
                }
            }
            // `forFunction`/`forMethod`: parâmetros de tipo e parâmetros.
            "FunctionDeclaration" | "MethodDeclaration" => {
                if let Some(f) = self.function(k) {
                    let f = ast.function(f);
                    self.type_parameters(&mut e, &f.type_params);
                    if let Some(ps) = &f.parameters {
                        self.parameters(&mut e, ps);
                    }
                }
            }
            _ => return None,
        }
        Some(e)
    }

    /// `SimpleIdentifier.isQualified`: o identificador de um
    /// `PrefixedIdentifier`, o nome de um `PropertyAccess` ou de um
    /// `ConstructorName`, o `methodName` de uma invocação com alvo (também o
    /// de uma seção de cascata).
    fn qualified(&self, k: usize, parent: usize) -> bool {
        let cx = self.cx;
        let children = cx.children(parent);
        match cx.kind(parent) {
            "PrefixedIdentifier" => children.get(1) == Some(&k),
            "PropertyAccess" | "ConstructorName" => children.last() == Some(&k),
            "MethodInvocation" => {
                let before: Vec<usize> = children.iter().copied().filter(|&f| !matches!(cx.kind(f), "ArgumentList" | "TypeArgumentList")).collect();
                if before.last() != Some(&k) {
                    return false;
                }
                if before.len() >= 2 {
                    return true;
                }
                // Sem alvo escrito: o `realTarget` da seção de cascata.
                let mut current = parent;
                while let Some(p) = cx.parent(current) {
                    match cx.kind(p) {
                        "CascadeExpression" => return cx.children(p).first() != Some(&current),
                        "MethodInvocation" | "PropertyAccess" | "IndexExpression" | "FunctionExpressionInvocation" if cx.children(p).first() == Some(&current) => current = p,
                        _ => return false,
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn visit(&mut self, k: usize) {
        let cx = self.cx;
        let kind = cx.kind(k);
        match kind {
            // O redirecionamento de fábrica não cita o alvo.
            "ConstructorName" if cx.parent(k).is_some_and(|p| cx.kind(p) == "ConstructorDeclaration") => return,
            "ImportDirective" => {
                for &f in cx.children(k) {
                    if cx.kind(f) == "SimpleIdentifier" && matches!(cx.tree.nos[f].marca, Marca::ContextoDeDeclaracao) {
                        self.prefixes.insert(cx.node_text(f).to_string());
                    }
                }
            }
            "NamedType" => {
                let prefix = cx.children(k).iter().copied().find(|&f| cx.kind(f) == "ImportPrefixReference");
                let start = match prefix {
                    Some(p) => {
                        let end = cx.tree.nos[p].fim;
                        end + cx.source[end..].len() - cx.source[end..].trim_start().len()
                    }
                    None => cx.tree.nos[k].inicio,
                };
                let name = identifier_in(cx.source, start).to_string();
                // `_addIfNotShadowed`.
                if !(self.contains(&name) && prefix.is_none()) && !self.prefixes.contains(&name) && !name.is_empty() {
                    self.names.insert(name);
                }
            }
            "SimpleIdentifier" => {
                self.identifier(k);
                return;
            }
            _ => {}
        }
        let scope = self.node_scope(k);
        let opened = scope.is_some();
        if let Some(e) = scope {
            self.scopes.push(e);
        }
        for &f in cx.children(k) {
            self.visit(f);
        }
        if opened {
            self.scopes.pop();
        }
    }

    fn identifier(&mut self, k: usize) {
        let cx = self.cx;
        if matches!(cx.tree.nos[k].marca, Marca::ContextoDeDeclaracao) {
            return;
        }
        let Some(parent) = cx.parent(k) else { return };
        // O nome da classe no construtor declarado.
        if cx.kind(parent) == "ConstructorDeclaration" && cx.children(parent).iter().copied().find(|&f| cx.kind(f) == "SimpleIdentifier") == Some(k) {
            return;
        }
        let name = cx.node_text(k).to_string();
        let argument_label = cx.kind(parent) == "Label" && cx.parent(parent).is_some_and(|a| cx.kind(a) == "NamedExpression" && cx.children(a).first() == Some(&parent));
        if !(self.qualified(k, parent) || argument_label) && (self.contains(&name) || self.prefixes.contains(&name)) {
            return;
        }
        self.names.insert(name);
    }
}

impl Context<'_> {
    /// `computeReferencedNames` da unidade.
    pub(crate) fn referenced_names(&self) -> HashSet<String> {
        let mut c = Computer { cx: self, names: HashSet::new(), prefixes: HashSet::new(), scopes: Vec::new() };
        let unit = c.unit_scope();
        c.scopes.push(unit);
        for &f in self.children(0) {
            c.visit(f);
        }
        c.names
    }
}

impl Project {
    /// O `referencedNames` de `u`, calculado uma vez por sessão.
    /// Esquece o que foi calculado para `u` (o texto dela mudou): os nomes
    /// citados e as entradas dela no índice reverso.
    pub(crate) fn forget_unit(&self, u: UnitId) {
        let old = self.referenced_names.lock().ok().and_then(|mut m| m.remove(&u));
        if let Ok(mut m) = self.comments.lock() {
            m.remove(&u);
        }
        if let Ok(mut m) = self.occurrences_by_unit.lock() {
            m.remove(&u);
        }
        if let Ok(mut i) = self.reverse_index.lock()
            && i.indexed.remove(&u)
            && let Some(old) = old
        {
            for name in old.iter() {
                if let Some(l) = i.by_name.get_mut(name.as_str()) {
                    l.remove(&u);
                }
            }
        }
    }

    /// As unidades inferidas (fora os patches) cujo `referencedNames` contém
    /// `name`, pelo índice reverso: as unidades que ainda não estão nele
    /// (as de bibliotecas recém-inferidas ou de texto novo) entram antes.
    pub(crate) fn units_that_cite(&self, name: &str) -> Vec<UnitId> {
        let units = self.units();
        let missing: Vec<UnitId> = match self.reverse_index.lock() {
            Ok(i) => units.iter().copied().filter(|u| !i.indexed.contains(u)).collect(),
            Err(_) => Vec::new(),
        };
        for u in missing {
            // Uma unidade cujo texto não tem `name` não o cita: fica para
            // quando outro nome a pedir (o `referencedNames` sai do texto).
            if self.program().unit(u).role == UnitRole::Patch || !self.program().unit(u).source.contains(name) {
                continue;
            }
            let names = self.unit_referenced_names(u);
            if let Ok(mut i) = self.reverse_index.lock()
                && i.indexed.insert(u)
            {
                for n in names.iter() {
                    i.by_name.entry(n.clone()).or_default().insert(u);
                }
            }
        }
        let Ok(i) = self.reverse_index.lock() else { return Vec::new() };
        i.by_name
            .get(name)
            .map(|l| l.iter().copied().filter(|u| self.libraries.contains(&self.program().unit(*u).library)).collect())
            .unwrap_or_default()
    }

    pub(crate) fn unit_referenced_names(&self, u: UnitId) -> Arc<HashSet<String>> {
        if let Some(n) = self.referenced_names.lock().ok().and_then(|m| m.get(&u).cloned()) {
            return n;
        }
        let n = Arc::new(Context::new(self, u).referenced_names());
        if let Ok(mut m) = self.referenced_names.lock() {
            m.insert(u, n.clone());
        }
        n
    }

    /// O nome que `Search._addResults` procura para o alvo (o
    /// `displayName`; num construtor, o da classe); `None` nas buscas locais
    /// (local, parâmetro de tipo, prefixo), que não passam por ele.
    pub(crate) fn search_name(&self, target: &Target) -> Option<String> {
        let p = self.program();
        Some(match target {
            Target::Top(el) => match *el {
                Element::Class(c) => self.name(p.class(c).name).to_string(),
                Element::Typedef(t) => self.name(p.typedef(t).name).to_string(),
                Element::Extension(x) => p.extension(x).name.map(|n| self.name(n).to_string()).unwrap_or_default(),
                Element::Function(f) => self.name(p.function(f).name).trim_end_matches('=').to_string(),
                Element::Variable(v) => self.name(p.variable(v).name).to_string(),
                Element::Prefix(..) => return None,
            },
            Target::Member { name, .. } => name.clone(),
            Target::Constructor(f) => self.name(p.class(p.function(*f).class?).name).to_string(),
            Target::Local { .. } | Target::Prefix { .. } | Target::TypeParameter { .. } => return None,
        })
    }

    /// Os arquivos candidatos de `Search._addResults` para o nome `name` de
    /// elementos declarados nas unidades `declarations`: num nome privado,
    /// os arquivos da biblioteca de cada um que são o do elemento ou citam
    /// o nome; senão, os arquivos que citam o nome mais todos os da
    /// biblioteca de cada elemento.
    pub(crate) fn candidate_files(&self, name: &str, declarations: &[UnitId]) -> HashSet<UnitId> {
        let p = self.program();
        let private = name.starts_with('_');
        let mut output = HashSet::new();
        for &u in declarations {
            let lib = p.unit(u).library;
            for &f in &p.library(lib).units {
                if p.unit(f).role == UnitRole::Patch {
                    continue;
                }
                if !private || f == u || self.unit_referenced_names(f).contains(name) {
                    output.insert(f);
                }
            }
        }
        if !private {
            // O analyzer não vê os patches do SDK (o índice os deixa de fora).
            output.extend(self.units_that_cite(name));
        }
        output
    }
}
