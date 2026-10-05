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
use crate::projeto::{Alvo, Projeto};
use crate::refatoracoes::Contexto;
use dartforge_elements::model::{Element, UnitId, UnitRole};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind};
use std::collections::HashSet;
use std::sync::Arc;

/// O identificador que começa em `pos`.
fn identificador_em(fonte: &str, pos: usize) -> &str {
    let resto = &fonte[pos.min(fonte.len())..];
    let fim = resto.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(resto.len());
    &resto[..fim]
}

struct Computador<'c, 'p> {
    cx: &'c Contexto<'p>,
    nomes: HashSet<String>,
    prefixos: HashSet<String>,
    escopos: Vec<HashSet<String>>,
}

impl Computador<'_, '_> {
    fn nome(&self, n: ast::Name) -> String {
        self.cx.p.nome(n.sym).to_string()
    }

    fn contem(&self, nome: &str) -> bool {
        self.escopos.iter().any(|e| e.contains(nome))
    }

    /// `addTypeParameters`.
    fn parametros_de_tipo(&self, e: &mut HashSet<String>, tps: &[ast::TypeParameter]) {
        for t in tps {
            e.insert(self.nome(t.name));
        }
    }

    /// `addFormalParameters`: só os `NormalFormalParameter` (o opcional e o
    /// nomeado vêm embrulhados num `DefaultFormalParameter` e ficam de fora).
    fn parametros(&self, e: &mut HashSet<String>, ps: &[ast::Parameter]) {
        for p in ps {
            if p.kind == ParameterKind::Required
                && let Some(n) = p.name
            {
                e.insert(self.nome(n));
            }
        }
    }

    /// `forClass`/`forExtensionType`: parâmetros de tipo, campos e métodos.
    fn membros(&self, e: &mut HashSet<String>, tps: &[ast::TypeParameter], membros: &[ast::MemberId]) {
        self.parametros_de_tipo(e, tps);
        let ast = self.cx.ast;
        for &mid in membros {
            match &ast.member(mid).kind {
                MemberKind::Field(l) => {
                    for v in l.variables.iter() {
                        e.insert(self.nome(v.name));
                    }
                }
                MemberKind::Method(fid) => {
                    if let Some(n) = ast.function(*fid).name {
                        e.insert(self.nome(n));
                    }
                }
                MemberKind::Constructor(_) => {}
            }
        }
    }

    /// `forUnit`: os `NamedCompilationUnitMember` (extensão não é) e as
    /// variáveis de topo do próprio arquivo.
    fn escopo_da_unidade(&self) -> HashSet<String> {
        let mut e = HashSet::new();
        let ast = self.cx.ast;
        for &did in &self.cx.p.programa().unit(self.cx.unidade).unit.declarations {
            match &ast.decl(did).kind {
                DeclKind::Class(c) => {
                    e.insert(self.nome(c.name));
                }
                DeclKind::Mixin(m) => {
                    e.insert(self.nome(m.name));
                }
                DeclKind::Enum(x) => {
                    e.insert(self.nome(x.name));
                }
                DeclKind::ExtensionType(x) => {
                    e.insert(self.nome(x.name));
                }
                DeclKind::Typedef(t) => {
                    e.insert(self.nome(t.name));
                }
                DeclKind::Function(fid) => {
                    if let Some(n) = ast.function(*fid).name {
                        e.insert(self.nome(n));
                    }
                }
                DeclKind::Variables(l) => {
                    for v in l.variables.iter() {
                        e.insert(self.nome(v.name));
                    }
                }
                DeclKind::Extension(_) => {}
            }
        }
        e
    }

    /// A função marcada no nó ou no `FunctionExpression` filho.
    fn funcao(&self, k: usize) -> Option<ast::FunctionId> {
        let cx = self.cx;
        std::iter::once(k).chain(cx.filhos(k).iter().copied()).find_map(|n| match cx.arvore.nos[n].marca {
            Marca::Funcao(f) => Some(f),
            _ => None,
        })
    }

    /// O escopo que o nó `k` abre, se abre.
    fn escopo_do_no(&self, k: usize) -> Option<HashSet<String>> {
        let cx = self.cx;
        let ast = cx.ast;
        let mut e = HashSet::new();
        match cx.especie(k) {
            // `forBlock`: funções e variáveis declaradas como instruções
            // diretas do bloco.
            "Block" => {
                for &s in cx.filhos(k) {
                    match cx.especie(s) {
                        "FunctionDeclarationStatement" => {
                            if let Some(f) = cx.filhos(s).first().and_then(|&d| self.funcao(d))
                                && let Some(n) = ast.function(f).name
                            {
                                e.insert(self.nome(n));
                            }
                        }
                        "VariableDeclarationStatement" => {
                            for &lista in cx.filhos(s) {
                                for &v in cx.filhos(lista) {
                                    if cx.especie(v) == "VariableDeclaration" {
                                        e.insert(identificador_em(cx.fonte, cx.arvore.nos[v].inicio).to_string());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "ClassDeclaration" | "ClassTypeAlias" | "ExtensionTypeDeclaration" | "FunctionTypeAlias" => {
                let Marca::Decl(did) = cx.arvore.nos[k].marca else { return Some(e) };
                match &ast.decl(did).kind {
                    DeclKind::Class(c) if c.mixin_application => self.parametros_de_tipo(&mut e, &c.type_params),
                    DeclKind::Class(c) => self.membros(&mut e, &c.type_params, &c.members),
                    DeclKind::ExtensionType(x) => self.membros(&mut e, &x.type_params, &x.members),
                    DeclKind::Typedef(t) => self.parametros_de_tipo(&mut e, &t.type_params),
                    _ => {}
                }
            }
            // `forConstructor`: só os parâmetros.
            "ConstructorDeclaration" => {
                let fim = cx.arvore.nos[k].fim;
                let dono = cx.pai(k).and_then(|p| match cx.arvore.nos[p].marca {
                    Marca::Decl(d) => Some(d),
                    _ => None,
                });
                let membros: &[ast::MemberId] = match dono.map(|d| &ast.decl(d).kind) {
                    Some(DeclKind::Class(c)) => &c.members,
                    Some(DeclKind::Enum(x)) => &x.members,
                    Some(DeclKind::Mixin(x)) => &x.members,
                    Some(DeclKind::ExtensionType(x)) => &x.members,
                    Some(DeclKind::Extension(x)) => &x.members,
                    _ => &[],
                };
                for &mid in membros {
                    let m = ast.member(mid);
                    if let MemberKind::Constructor(c) = &m.kind
                        && m.span.end == fim
                    {
                        self.parametros(&mut e, &c.parameters);
                        break;
                    }
                }
            }
            // `forFunction`/`forMethod`: parâmetros de tipo e parâmetros.
            "FunctionDeclaration" | "MethodDeclaration" => {
                if let Some(f) = self.funcao(k) {
                    let f = ast.function(f);
                    self.parametros_de_tipo(&mut e, &f.type_params);
                    if let Some(ps) = &f.parameters {
                        self.parametros(&mut e, ps);
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
    fn qualificado(&self, k: usize, pai: usize) -> bool {
        let cx = self.cx;
        let filhos = cx.filhos(pai);
        match cx.especie(pai) {
            "PrefixedIdentifier" => filhos.get(1) == Some(&k),
            "PropertyAccess" | "ConstructorName" => filhos.last() == Some(&k),
            "MethodInvocation" => {
                let antes: Vec<usize> = filhos.iter().copied().filter(|&f| !matches!(cx.especie(f), "ArgumentList" | "TypeArgumentList")).collect();
                if antes.last() != Some(&k) {
                    return false;
                }
                if antes.len() >= 2 {
                    return true;
                }
                // Sem alvo escrito: o `realTarget` da seção de cascata.
                let mut atual = pai;
                while let Some(p) = cx.pai(atual) {
                    match cx.especie(p) {
                        "CascadeExpression" => return cx.filhos(p).first() != Some(&atual),
                        "MethodInvocation" | "PropertyAccess" | "IndexExpression" | "FunctionExpressionInvocation" if cx.filhos(p).first() == Some(&atual) => atual = p,
                        _ => return false,
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn visitar(&mut self, k: usize) {
        let cx = self.cx;
        let especie = cx.especie(k);
        match especie {
            // O redirecionamento de fábrica não cita o alvo.
            "ConstructorName" if cx.pai(k).is_some_and(|p| cx.especie(p) == "ConstructorDeclaration") => return,
            "ImportDirective" => {
                for &f in cx.filhos(k) {
                    if cx.especie(f) == "SimpleIdentifier" && matches!(cx.arvore.nos[f].marca, Marca::ContextoDeDeclaracao) {
                        self.prefixos.insert(cx.texto_do_no(f).to_string());
                    }
                }
            }
            "NamedType" => {
                let prefixo = cx.filhos(k).iter().copied().find(|&f| cx.especie(f) == "ImportPrefixReference");
                let inicio = match prefixo {
                    Some(p) => {
                        let fim = cx.arvore.nos[p].fim;
                        fim + cx.fonte[fim..].len() - cx.fonte[fim..].trim_start().len()
                    }
                    None => cx.arvore.nos[k].inicio,
                };
                let nome = identificador_em(cx.fonte, inicio).to_string();
                // `_addIfNotShadowed`.
                if !(self.contem(&nome) && prefixo.is_none()) && !self.prefixos.contains(&nome) && !nome.is_empty() {
                    self.nomes.insert(nome);
                }
            }
            "SimpleIdentifier" => {
                self.identificador(k);
                return;
            }
            _ => {}
        }
        let escopo = self.escopo_do_no(k);
        let abriu = escopo.is_some();
        if let Some(e) = escopo {
            self.escopos.push(e);
        }
        for &f in cx.filhos(k) {
            self.visitar(f);
        }
        if abriu {
            self.escopos.pop();
        }
    }

    fn identificador(&mut self, k: usize) {
        let cx = self.cx;
        if matches!(cx.arvore.nos[k].marca, Marca::ContextoDeDeclaracao) {
            return;
        }
        let Some(pai) = cx.pai(k) else { return };
        // O nome da classe no construtor declarado.
        if cx.especie(pai) == "ConstructorDeclaration" && cx.filhos(pai).iter().copied().find(|&f| cx.especie(f) == "SimpleIdentifier") == Some(k) {
            return;
        }
        let nome = cx.texto_do_no(k).to_string();
        let rotulo_de_argumento = cx.especie(pai) == "Label" && cx.pai(pai).is_some_and(|a| cx.especie(a) == "NamedExpression" && cx.filhos(a).first() == Some(&pai));
        if !(self.qualificado(k, pai) || rotulo_de_argumento) && (self.contem(&nome) || self.prefixos.contains(&nome)) {
            return;
        }
        self.nomes.insert(nome);
    }
}

impl Contexto<'_> {
    /// `computeReferencedNames` da unidade.
    pub(crate) fn nomes_referenciados(&self) -> HashSet<String> {
        let mut c = Computador { cx: self, nomes: HashSet::new(), prefixos: HashSet::new(), escopos: Vec::new() };
        let unidade = c.escopo_da_unidade();
        c.escopos.push(unidade);
        for &f in self.filhos(0) {
            c.visitar(f);
        }
        c.nomes
    }
}

impl Projeto {
    /// O `referencedNames` de `u`, calculado uma vez por sessão.
    pub(crate) fn nomes_referenciados_da_unidade(&self, u: UnitId) -> Arc<HashSet<String>> {
        if let Some(n) = self.nomes_referenciados.lock().ok().and_then(|m| m.get(&u).cloned()) {
            return n;
        }
        let n = Arc::new(Contexto::novo(self, u).nomes_referenciados());
        if let Ok(mut m) = self.nomes_referenciados.lock() {
            m.insert(u, n.clone());
        }
        n
    }

    /// O nome que `Search._addResults` procura para o alvo (o
    /// `displayName`; num construtor, o da classe); `None` nas buscas locais
    /// (local, parâmetro de tipo, prefixo), que não passam por ele.
    pub(crate) fn nome_da_busca(&self, alvo: &Alvo) -> Option<String> {
        let p = self.programa();
        Some(match alvo {
            Alvo::Topo(el) => match *el {
                Element::Class(c) => self.nome(p.class(c).name).to_string(),
                Element::Typedef(t) => self.nome(p.typedef(t).name).to_string(),
                Element::Extension(x) => p.extension(x).name.map(|n| self.nome(n).to_string()).unwrap_or_default(),
                Element::Function(f) => self.nome(p.function(f).name).trim_end_matches('=').to_string(),
                Element::Variable(v) => self.nome(p.variable(v).name).to_string(),
                Element::Prefix(..) => return None,
            },
            Alvo::Membro { nome, .. } => nome.clone(),
            Alvo::Construtor(f) => self.nome(p.class(p.function(*f).class?).name).to_string(),
            Alvo::Local { .. } | Alvo::Prefixo { .. } | Alvo::ParametroDeTipo { .. } => return None,
        })
    }

    /// Os arquivos candidatos de `Search._addResults` para o nome `nome` de
    /// elementos declarados nas unidades `declaracoes`: num nome privado,
    /// os arquivos da biblioteca de cada um que são o do elemento ou citam
    /// o nome; senão, os arquivos que citam o nome mais todos os da
    /// biblioteca de cada elemento.
    pub(crate) fn arquivos_candidatos(&self, nome: &str, declaracoes: &[UnitId]) -> HashSet<UnitId> {
        let p = self.programa();
        let privado = nome.starts_with('_');
        let mut saida = HashSet::new();
        for &u in declaracoes {
            let lib = p.unit(u).library;
            for &f in &p.library(lib).units {
                if p.unit(f).role == UnitRole::Patch {
                    continue;
                }
                if !privado || f == u || self.nomes_referenciados_da_unidade(f).contains(nome) {
                    saida.insert(f);
                }
            }
        }
        if !privado {
            for u in self.unidades() {
                // O analyzer não vê os patches do SDK.
                if p.unit(u).role == UnitRole::Patch {
                    continue;
                }
                if self.nomes_referenciados_da_unidade(u).contains(nome) {
                    saida.insert(u);
                }
            }
        }
        saida
    }
}
