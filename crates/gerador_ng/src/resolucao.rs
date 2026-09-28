//! A ponte do gerador para o banco semântico.
//!
//! O compilador oficial pergunta ao `package:analyzer` onde um tipo é
//! declarado; nós perguntamos ao nosso `Program`. É a mesma pergunta e a
//! resposta tem de ser a mesma: para `OidcService` o import gerado é o da
//! biblioteca que **declara** o tipo, não a que o reexporta — por isso sai
//! `package:ngrouter/src/router/router.dart` e não
//! `package:ngrouter/ngrouter.dart`.
//!
//! O caminho do import é montado pela regra do `getImportModulePath` do
//! `ngcompiler` (`output/path_util.dart`): mesmo pacote e mesma pasta de
//! primeiro nível viram caminho relativo; o resto vira `package:`.
use dartforge_elements::model::{ClassId, Element, LibraryId, Program};
use dartforge_intern::Interner;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// O que o emissor precisa perguntar sobre nomes. É um traço para que o
/// teste possa responder sem carregar um programa inteiro.
/// Um método de instância achado na hierarquia ([`Resolucao::metodo`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metodo {
    pub posicionais: usize,
    /// O retorno escrito e o arquivo do escopo dele; `None` sem tipo escrito
    /// ou com [`Metodo::incerto`].
    pub retorno: Option<(String, PathBuf)>,
    /// O retorno cita um parâmetro de tipo que a substituição não resolveu:
    /// o tipo existe, mas não se sabe escrever (nunca `dynamic`).
    pub incerto: bool,
}

pub trait Resolucao {
    /// URI da biblioteca que declara `nome` no escopo de `arquivo`.
    fn uri_do_tipo(&self, arquivo: &Path, nome: &str) -> Option<String>;

    /// Tipo declarado de `membro` na classe `tipo` (nomeada no escopo de
    /// `arquivo`), com o arquivo em cujo escopo esse tipo foi escrito — o da
    /// classe que declara o membro, não o do componente.
    ///
    /// É o que permite interpolar `{{ item.nome }}`: sem saber que `nome` é
    /// `String`, não dá para escolher entre `interpolateString`,
    /// `interpolate` e `updateTextWithPrimitive`.
    fn tipo_do_membro(
        &self,
        _arquivo: &Path,
        _tipo: &str,
        _membro: &str,
    ) -> Option<(String, PathBuf)> {
        None
    }

    /// O método de instância `nome` da classe `tipo` (nomeada no escopo de
    /// `arquivo`, subindo pelas superclasses): quantos parâmetros
    /// posicionais ele tem (`rewriteTearOff`) e o retorno escrito, com o
    /// arquivo em cujo escopo ele se resolve. `None` quando não é método
    /// (campo, getter, estático) ou quando o retorno cita parâmetro de tipo.
    fn metodo(&self, _arquivo: &Path, _tipo: &str, _nome: &str) -> Option<Metodo> {
        None
    }

    /// [`Self::tipo_do_membro`] com `livres`: os parâmetros de tipo do
    /// componente, em escopo em toda visão dele (as visões são genéricas).
    /// Um argumento do receptor que é um deles se substitui como está.
    fn tipo_do_membro_livre(
        &self,
        arquivo: &Path,
        tipo: &str,
        membro: &str,
        _livres: &[String],
    ) -> Option<(String, PathBuf)> {
        self.tipo_do_membro(arquivo, tipo, membro)
    }

    /// [`Self::metodo`] com `livres` e os argumentos do receptor em `tipo`.
    fn metodo_livre(
        &self,
        arquivo: &Path,
        tipo: &str,
        nome: &str,
        _livres: &[String],
    ) -> Option<Metodo> {
        self.metodo(arquivo, tipo, nome)
    }

    /// O membro de instância `membro` da classe `tipo` (nomeada no escopo de
    /// `arquivo`, subindo pelas superclasses) é um campo `final`/`const`?
    /// `Some(false)` para campo mutável ou getter; `None` quando não se acha
    /// a declaração. É o `isImmutable` do ngcompiler para um membro herdado.
    fn membro_final(&self, _arquivo: &Path, _tipo: &str, _membro: &str) -> Option<bool> {
        None
    }

    /// A classe `tipo` (ou algo acima dela, na ordem do Dart) tem um setter
    /// de instância `membro` — de um campo não final ou um `set membro(..)`?
    /// É o que a escrita `membro = x` de um evento precisa: o `PropertyWrite`
    /// de receptor implícito vira `_ctx.membro`. `false` quando não se sabe.
    fn tem_setter(&self, _arquivo: &Path, _tipo: &str, _membro: &str) -> bool {
        false
    }

    /// Os parâmetros de tipo da classe `tipo` (no escopo de `arquivo`): para
    /// cada um, se tem limite escrito. `None` quando não se acha a classe.
    fn limites_de_tipo(&self, _arquivo: &Path, _tipo: &str) -> Option<Vec<bool>> {
        None
    }

    /// O nome-base do tipo `tipo` (sem `?` e sem argumentos de tipo) não
    /// está declarado no escopo de `arquivo` — nem importado, nem no
    /// `dart:core`. Para o analyzer é um `InvalidType`, e o `_TypeResolver`
    /// do oficial dá `dynamic` a qualquer membro lido dele (caso típico: a
    /// classe vem de um arquivo gerado por uma fase posterior, que o
    /// resolvedor do builder não enxerga). `false` quando não se sabe.
    fn tipo_inexistente(&self, _arquivo: &Path, _tipo: &str) -> bool {
        false
    }

    /// O que um nome de `directives:` designa no escopo de `arquivo`: uma
    /// classe ou uma lista constante de outros nomes.
    fn designado(&self, arquivo: &Path, nome: &str) -> Option<Designado> {
        self.uri_do_tipo(arquivo, nome)
            .map(|uri| Designado::Classe { uri })
    }

    /// Um nome de `exports:` no escopo de `arquivo`: a URI da biblioteca que
    /// o declara e o que ele é.
    fn exportado(&self, _arquivo: &Path, _nome: &str) -> Option<(String, Exportado)> {
        None
    }

    /// O membro estático `membro` da classe `tipo` (no escopo de
    /// `arquivo`).
    fn membro_estatico(&self, _arquivo: &Path, _tipo: &str, _membro: &str) -> Option<Estatico> {
        None
    }
}

/// O que um nome de `exports:` designa no escopo do componente
/// (`_matchExport`: o oficial o escreve pelo import da biblioteca que o
/// declara, com tipo `dynamic`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exportado {
    /// Classe, enum ou mixin: vale como receptor de membro estático.
    Classe,
    /// Variável de topo: imutável quando `const`/`final` (`isImmutable`).
    Variavel { imutavel: bool },
    /// Getter de topo.
    Getter,
    /// Função de topo: vale como alvo de chamada.
    Funcao,
}

/// Um membro estático lido por `Classe.nome`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estatico {
    /// Campo (ou valor de enum): imutável quando `const`/`final`.
    Campo {
        imutavel: bool,
    },
    Getter,
    Metodo,
}

/// Um item de `directives: [...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Designado {
    /// Uma classe, pela URI da biblioteca que a declara.
    Classe { uri: String },
    /// Uma lista constante (`coreDirectives`, `li.limitlessFormDirectives`):
    /// os nomes dos itens, a resolver no escopo do arquivo que a declara.
    Lista { itens: Vec<String>, escopo: PathBuf },
}

pub struct Resolvedor<'a> {
    program: &'a Program,
    interner: &'a Interner,
    /// Caminho de cada unidade principal para a sua biblioteca.
    por_caminho: HashMap<PathBuf, LibraryId>,
}

impl<'a> Resolvedor<'a> {
    pub fn novo(program: &'a Program, interner: &'a Interner) -> Self {
        let mut por_caminho = HashMap::new();
        for unidade in &program.units {
            let Some(caminho) = &unidade.path else {
                continue;
            };
            por_caminho
                .entry(dartforge_elements::gerado::chave(caminho))
                .or_insert(unidade.library);
        }
        Self {
            program,
            interner,
            por_caminho,
        }
    }

    /// Cada biblioteca carregada: URI, árvore, unidade, fonte e caminho.
    ///
    /// É daqui que sai o índice de componentes de **todos** os pacotes — sem
    /// isto, um `<li-select>` do limitless_ui seria só uma tag desconhecida.
    pub fn bibliotecas(
        &self,
    ) -> impl Iterator<
        Item = (
            &'a str,
            &'a dartforge_frontend::ast::Ast,
            &'a dartforge_frontend::ast::CompilationUnit,
            &'a str,
            Option<&'a Path>,
        ),
    > + '_ {
        self.program.units.iter().filter_map(move |u| {
            let lib = self.program.library(u.library);
            if lib.is_sdk {
                return None;
            }
            Some((
                lib.uri.as_str(),
                &u.ast,
                &u.unit,
                u.source.as_str(),
                u.path.as_deref(),
            ))
        })
    }

    /// O interner da carga, para ler os nomes das árvores acima.
    pub fn interner(&self) -> &'a Interner {
        self.interner
    }

    /// O programa carregado: é dele que saem os metadados das diretivas
    /// (`metadados.rs`), como o oficial os tira do `analyzer`.
    pub fn programa(&self) -> &'a Program {
        self.program
    }

    /// A classe `classe` declarada na biblioteca de URI `uri`.
    pub fn classe_por_uri(&self, uri: &str, classe: &str) -> Option<ClassId> {
        let lib = self.program.libraries.iter().position(|l| l.uri == uri)?;
        let sym = self.interner.lookup(classe)?;
        match self.program.libraries[lib].declared.get(&sym)?.getter? {
            Element::Class(id) => Some(id),
            _ => None,
        }
    }

    /// O que `nome` (sem prefixo) ou `prefixo.nome` designa no escopo da
    /// biblioteca `lib`. Nome ambíguo não designa nada.
    pub fn elemento_em(
        &self,
        lib: LibraryId,
        prefixo: Option<&str>,
        nome: &str,
    ) -> Option<Element> {
        let biblioteca = self.program.library(lib);
        let sym = self.interner.lookup(nome)?;
        let espaco = match prefixo {
            None => &biblioteca.scope,
            Some(p) => biblioteca.prefixes.get(&self.interner.lookup(p)?)?,
        };
        let ligacao = espaco.get(&sym)?;
        if ligacao.ambiguous {
            return None;
        }
        ligacao.getter
    }

    /// `nome` é um prefixo de import no escopo de `lib`?
    pub fn e_prefixo(&self, lib: LibraryId, nome: &str) -> bool {
        self.interner
            .lookup(nome)
            .is_some_and(|s| self.program.library(lib).prefixes.contains_key(&s))
    }

    /// Biblioteca do arquivo, se ele foi carregado.
    pub fn biblioteca(&self, arquivo: &Path) -> Option<LibraryId> {
        self.por_caminho
            .get(&dartforge_elements::gerado::chave(arquivo))
            .copied()
    }

    fn procurar(&self, arquivo: &Path, nome: &str) -> Option<&'a str> {
        let lib = self.biblioteca(arquivo)?;
        let biblioteca = self.program.library(lib);
        let (prefixo, simples) = match nome.split_once('.') {
            Some((p, t)) => (Some(p), t),
            None => (None, nome),
        };
        let sym = self.interner.lookup(simples)?;
        let espaco = match prefixo {
            None => &biblioteca.scope,
            Some(p) => {
                let psym = self.interner.lookup(p)?;
                biblioteca.prefixes.get(&psym)?
            }
        };
        let ligacao = espaco.get(&sym)?;
        if ligacao.ambiguous {
            return None;
        }
        match ligacao.getter? {
            Element::Class(id) => {
                let dona = self.program.class(id).library;
                Some(self.program.library(dona).uri.as_str())
            }
            Element::Typedef(id) => {
                let dona = self.program.typedef(id).library;
                Some(self.program.library(dona).uri.as_str())
            }
            _ => None,
        }
    }
}

/// URI `asset:<pacote>/<pasta>/<resto>` de uma URI de biblioteca.
///
/// O emissor oficial trabalha nesse espaço; `package:x/y` é
/// `asset:x/lib/y`.
pub fn asset_de_uri(uri: &str, pacote_do_projeto: &str, raiz: &Path) -> Option<String> {
    if let Some(resto) = uri.strip_prefix("package:") {
        let (pkg, caminho) = resto.split_once('/')?;
        return Some(format!("asset:{pkg}/lib/{caminho}"));
    }
    // `file:///…/web/main.dart` do próprio projeto.
    let caminho = uri.strip_prefix("file:///").map(|c| c.replace('/', "\\"))?;
    let rel = Path::new(&caminho).strip_prefix(raiz).ok()?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    Some(format!("asset:{pacote_do_projeto}/{rel}"))
}

/// `getImportModulePath` do `ngcompiler`: como um arquivo gerado em
/// `modulo` escreve o import de `importado`, ambos em URIs `asset:`.
pub fn caminho_do_import(modulo: &str, importado: &str) -> Option<String> {
    let m = Asset::analisar(modulo)?;
    let i = Asset::analisar(importado)?;
    if modulo == importado {
        return i.caminho.rsplit('/').next().map(str::to_string);
    }
    if m.pasta == i.pasta && m.pacote == i.pacote {
        return Some(relativo(m.caminho, i.caminho));
    }
    if i.pasta == "lib" {
        return Some(format!("package:{}/{}", i.pacote, i.caminho));
    }
    None
}

struct Asset<'a> {
    pacote: &'a str,
    /// `lib`, `web`, `test`.
    pasta: &'a str,
    /// O que vem depois da pasta.
    caminho: &'a str,
}

impl<'a> Asset<'a> {
    fn analisar(uri: &'a str) -> Option<Self> {
        let resto = uri.strip_prefix("asset:")?;
        let (pacote, resto) = resto.split_once('/')?;
        let (pasta, caminho) = resto.split_once('/')?;
        Some(Asset {
            pacote,
            pasta,
            caminho,
        })
    }
}

/// Caminho relativo de `modulo` para `importado`, contando segmentos.
fn relativo(modulo: &str, importado: &str) -> String {
    let m: Vec<&str> = modulo.split('/').collect();
    let i: Vec<&str> = importado.split('/').collect();
    let mut prefixo = 0;
    while prefixo < m.len().min(i.len()) && m[prefixo] == i[prefixo] {
        prefixo += 1;
    }
    let subir = m.len().saturating_sub(1).saturating_sub(prefixo);
    let mut partes: Vec<&str> = vec![".."; subir];
    partes.extend_from_slice(&i[prefixo..]);
    partes.join("/")
}

/// Aceita `T` e `p.T`: o prefixo é procurado no namespace do próprio
/// prefixo, como manda a resolução do Dart.
impl Resolucao for Resolvedor<'_> {
    fn uri_do_tipo(&self, arquivo: &Path, nome: &str) -> Option<String> {
        self.procurar(arquivo, nome).map(str::to_string)
    }

    fn tipo_do_membro(
        &self,
        arquivo: &Path,
        tipo: &str,
        membro: &str,
    ) -> Option<(String, PathBuf)> {
        self.tipo_do_membro_livre(arquivo, tipo, membro, &[])
    }

    fn tipo_do_membro_livre(
        &self,
        arquivo: &Path,
        tipo: &str,
        membro: &str,
        livres: &[String],
    ) -> Option<(String, PathBuf)> {
        let classe = self.classe(arquivo, tipo)?;
        let (_, args) = separar_argumentos(tipo.trim().trim_end_matches('?'));
        self.membro_da_classe_com(classe, membro, Some(&args), livres)
    }

    fn limites_de_tipo(&self, arquivo: &Path, tipo: &str) -> Option<Vec<bool>> {
        let c = self.program.class(self.classe(arquivo, tipo)?);
        Some(c.type_params.iter().map(|p| p.bound.is_some()).collect())
    }

    fn tem_setter(&self, arquivo: &Path, tipo: &str, membro: &str) -> bool {
        let Some(sym) = self.interner.lookup(&format!("{membro}_=")) else {
            return false;
        };
        self.classe(arquivo, tipo)
            .and_then(|c| self.membro_de_instancia(c, sym, 0))
            .is_some()
    }

    fn metodo(&self, arquivo: &Path, tipo: &str, nome: &str) -> Option<Metodo> {
        self.metodo_livre(arquivo, tipo, nome, &[])
    }

    fn metodo_livre(
        &self,
        arquivo: &Path,
        tipo: &str,
        nome: &str,
        livres: &[String],
    ) -> Option<Metodo> {
        let sym = self.interner.lookup(nome)?;
        let classe = self.classe(arquivo, tipo)?;
        let (_, escritos) = separar_argumentos(tipo.trim().trim_end_matches('?'));
        // Sem argumentos escritos e sem livres, a busca sem substituição; com
        // eles, pela hierarquia com os argumentos.
        let (id, fid, args) = if escritos.is_empty() && livres.is_empty() {
            let (id, fid) = self.membro_de_instancia(classe, sym, 0)?;
            (id, fid, None)
        } else {
            let iniciais = self.instanciar(classe, &escritos, livres)?;
            let (id, fid, a) = self.membro_com_argumentos(classe, iniciais, sym, 0, livres)?;
            (id, fid, Some(a))
        };
        let c = self.program.class(id);
        let dartforge_elements::model::FunctionRef::Function { unit, function } =
            self.program.function(fid).node
        else {
            return None;
        };
        let u = self.program.unit(unit);
        let f = u.ast.function(function);
        if f.static_ || !matches!(f.kind, dartforge_frontend::ast::FunctionKind::Function) {
            return None;
        }
        let posicionais = f.parameters.as_ref().map_or(0, |ps| {
            ps.iter()
                .filter(|p| !matches!(p.kind, dartforge_frontend::ast::ParameterKind::Named))
                .count()
        });
        let mut incerto = false;
        let retorno = match f.return_type {
            None => None,
            Some(t) => {
                let sp = u.ast.ty(t).span;
                let texto = u.source.get(sp.start..sp.end)?.to_string();
                let proprios: Vec<&str> = c
                    .type_params
                    .iter()
                    .map(|p| self.interner.resolve(p.name))
                    .collect();
                let do_metodo: Vec<&str> = f
                    .type_params
                    .iter()
                    .map(|p| self.interner.resolve(p.name.sym))
                    .collect();
                let palavras: Vec<&str> = texto
                    .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '$')
                    .collect();
                let cita_proprio = palavras.iter().any(|t| proprios.contains(t));
                let cita_do_metodo = palavras.iter().any(|t| do_metodo.contains(t));
                // Os parâmetros da classe que declara, trocados pelos
                // argumentos que a busca trouxe; faltando algum, incerto.
                let troca: Option<Vec<(&str, String)>> = match (&args, cita_proprio) {
                    (_, false) => Some(Vec::new()),
                    (None, true) => None,
                    (Some(a), true) => {
                        let pares: Vec<(&str, String)> = proprios
                            .iter()
                            .zip(a)
                            .filter_map(|(n, v)| v.clone().map(|v| (*n, v)))
                            .collect();
                        let falta = palavras
                            .iter()
                            .any(|t| proprios.contains(t) && !pares.iter().any(|(n, _)| n == t));
                        (!falta).then_some(pares)
                    }
                };
                match troca {
                    Some(pares) if !cita_do_metodo => {
                        Some((substituir_palavras(&texto, &pares), u.path.clone()?))
                    }
                    _ => {
                        incerto = true;
                        None
                    }
                }
            }
        };
        Some(Metodo {
            posicionais,
            retorno,
            incerto,
        })
    }

    fn tipo_inexistente(&self, arquivo: &Path, tipo: &str) -> bool {
        let Some(lib) = self.biblioteca(arquivo) else {
            return false;
        };
        let base = tipo.trim().trim_end_matches('?');
        let base = base.split('<').next().unwrap_or(base).trim();
        // Tipo de função, registro ou palavra reservada: não é nome a achar.
        if base.is_empty()
            || !base
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '.'))
            || matches!(base, "dynamic" | "void" | "Never" | "Function" | "Null")
        {
            return false;
        }
        let biblioteca = self.program.library(lib);
        let (prefixo, simples) = match base.split_once('.') {
            Some((p, t)) => (Some(p), t),
            None => (None, base),
        };
        let espaco = match prefixo {
            None => &biblioteca.scope,
            Some(p) => match self
                .interner
                .lookup(p)
                .and_then(|ps| biblioteca.prefixes.get(&ps))
            {
                Some(e) => e,
                None => return true,
            },
        };
        match self.interner.lookup(simples) {
            None => true,
            Some(sym) => espaco.get(&sym).is_none(),
        }
    }

    fn membro_final(&self, arquivo: &Path, tipo: &str, membro: &str) -> Option<bool> {
        let sym = self.interner.lookup(membro)?;
        let (_, fid) = self.membro_de_instancia(self.classe(arquivo, tipo)?, sym, 0)?;
        let f = self.program.function(fid);
        let Some(vid) = f.variable else {
            // Getter escrito: nunca imutável.
            return matches!(
                f.node,
                dartforge_elements::model::FunctionRef::Function { .. }
            )
            .then_some(false);
        };
        let dartforge_elements::model::VariableRef::Field { unit, member, .. } =
            self.program.variable(vid).node
        else {
            return None;
        };
        let u = self.program.unit(unit);
        let dartforge_frontend::ast::MemberKind::Field(lista) = &u.ast.member(member).kind else {
            return None;
        };
        Some(lista.final_ || lista.const_)
    }

    fn exportado(&self, arquivo: &Path, nome: &str) -> Option<(String, Exportado)> {
        let lib = self.biblioteca(arquivo)?;
        let sym = self.interner.lookup(nome)?;
        let ligacao = self.program.library(lib).scope.get(&sym)?;
        if ligacao.ambiguous {
            return None;
        }
        let (dona, o_que) = match ligacao.getter? {
            Element::Class(id) => (self.program.class(id).library, Exportado::Classe),
            Element::Variable(vid) => {
                let v = self.program.variable(vid);
                (
                    v.library,
                    Exportado::Variavel {
                        imutavel: v.const_ || v.final_,
                    },
                )
            }
            Element::Function(fid) => {
                let f = self.program.function(fid);
                let o_que = match f.kind {
                    dartforge_elements::model::FunctionKind::Function => Exportado::Funcao,
                    dartforge_elements::model::FunctionKind::Getter => Exportado::Getter,
                    _ => return None,
                };
                (f.library, o_que)
            }
            _ => return None,
        };
        Some((self.program.library(dona).uri.clone(), o_que))
    }

    fn membro_estatico(&self, arquivo: &Path, tipo: &str, membro: &str) -> Option<Estatico> {
        let id = self.classe(arquivo, tipo)?;
        let sym = self.interner.lookup(membro)?;
        let c = self.program.class(id);
        // Valor de enum: constante (fica em `enum_constants`, não entre os
        // membros estáticos).
        if c.enum_constants
            .iter()
            .any(|&v| self.program.variable(v).name == sym)
        {
            return Some(Estatico::Campo { imutavel: true });
        }
        let fid = *c.static_members.get(&sym)?;
        let f = self.program.function(fid);
        match (f.variable, f.kind) {
            (Some(vid), _) => {
                let v = self.program.variable(vid);
                Some(Estatico::Campo {
                    imutavel: v.const_ || v.final_,
                })
            }
            (None, dartforge_elements::model::FunctionKind::Getter) => Some(Estatico::Getter),
            (None, dartforge_elements::model::FunctionKind::Function) => Some(Estatico::Metodo),
            _ => None,
        }
    }

    fn designado(&self, arquivo: &Path, nome: &str) -> Option<Designado> {
        let lib = self.biblioteca(arquivo)?;
        let biblioteca = self.program.library(lib);
        let (prefixo, simples) = match nome.split_once('.') {
            Some((p, t)) => (Some(p), t),
            None => (None, nome),
        };
        let sym = self.interner.lookup(simples)?;
        let espaco = match prefixo {
            None => &biblioteca.scope,
            Some(p) => biblioteca.prefixes.get(&self.interner.lookup(p)?)?,
        };
        let ligacao = espaco.get(&sym)?;
        if ligacao.ambiguous {
            return None;
        }
        match ligacao.getter? {
            Element::Class(id) => {
                let dona = self.program.class(id).library;
                Some(Designado::Classe {
                    uri: self.program.library(dona).uri.clone(),
                })
            }
            Element::Variable(vid) => self.lista_constante(vid),
            _ => None,
        }
    }
}

impl<'a> Resolvedor<'a> {
    /// A classe que o nome `tipo` designa no escopo de `arquivo`.
    fn classe(&self, arquivo: &Path, tipo: &str) -> Option<ClassId> {
        let lib = self.biblioteca(arquivo)?;
        let biblioteca = self.program.library(lib);
        let simples = separar_argumentos(tipo.trim().trim_end_matches('?')).0;
        let (prefixo, simples) = match simples.split_once('.') {
            Some((p, t)) => (Some(p), t),
            None => (None, simples),
        };
        let sym = self.interner.lookup(simples)?;
        let espaco = match prefixo {
            None => &biblioteca.scope,
            Some(p) => biblioteca.prefixes.get(&self.interner.lookup(p)?)?,
        };
        match espaco.get(&sym)?.getter? {
            Element::Class(id) => Some(id),
            _ => None,
        }
    }

    /// Os itens de uma variável de topo `const` cuja inicialização é uma
    /// lista literal de nomes (`const coreDirectives = [NgClass, NgFor]`),
    /// também espalhados (`...outra`). Qualquer outra forma (`if`, `for`,
    /// expressão) fica sem resposta — e quem pergunta recusa.
    fn lista_constante(&self, vid: dartforge_elements::model::VariableId) -> Option<Designado> {
        use dartforge_frontend::ast;
        let v = self.program.variable(vid);
        if !v.const_ {
            return None;
        }
        let dartforge_elements::model::VariableRef::TopLevel { unit, decl, index } = v.node else {
            return None;
        };
        let u = self.program.unit(unit);
        let ast::DeclKind::Variables(lista) = &u.ast.decl(decl).kind else {
            return None;
        };
        let inicial = lista.variables.get(index)?.initializer?;
        let ast::ExprKind::List { elements, .. } = &u.ast.expr(inicial).kind else {
            return None;
        };
        let mut itens = Vec::new();
        for e in elements.iter() {
            // `...outraLista`: o valor constante é a lista achatada, na
            // ordem — o mesmo que o nome dela no lugar.
            let (ast::CollectionElement::Expression(x)
            | ast::CollectionElement::Spread { value: x, .. }) = e
            else {
                return None;
            };
            itens.push(nome_qualificado(&u.ast, self.interner, *x)?);
        }
        Some(Designado::Lista {
            itens,
            escopo: u.path.clone()?,
        })
    }

    /// Tipo declarado de um membro de instância, subindo pela superclasse
    /// quando a classe não o declara — que é onde ficam os campos herdados.
    /// O membro de instância `sym` visto de `classe` e a classe que o
    /// declara, na ordem de busca do Dart: a própria classe, os mixins do
    /// último ao primeiro (`with A, B` põe `B` por cima de `A`) e, por fim,
    /// a superclasse, recursivamente.
    fn membro_de_instancia(
        &self,
        classe: ClassId,
        sym: dartforge_intern::SymbolId,
        profundidade: u32,
    ) -> Option<(ClassId, dartforge_elements::model::FunctionElementId)> {
        if profundidade > 64 {
            return None;
        }
        let c = self.program.class(classe);
        if let Some(&fid) = c.instance_members.get(&sym) {
            return Some((classe, fid));
        }
        for &m in c.mixin_classes.iter().rev() {
            if let Some(achado) = self.membro_de_instancia(m, sym, profundidade + 1) {
                return Some(achado);
            }
        }
        self.membro_de_instancia(c.supertype_class?, sym, profundidade + 1)
    }

    /// O tipo do membro `membro` de `classe` visto de um receptor com os
    /// argumentos de tipo `args` (`Grupo<dynamic>` dá `["dynamic"]`; `Grupo`
    /// cru, nenhum). O analyzer instancia o receptor e sobe pela hierarquia
    /// substituindo os parâmetros de cada supertipo (`MenuItemGroup<T>` →
    /// `LabeledList<T>` → `DelegatingList<T>`, onde mora o `single`), e o
    /// `fromDartType` escreve o resultado. Aqui a troca é textual: um
    /// argumento só é conhecido quando se escreve igual em qualquer escopo
    /// (`dynamic` e os tipos do `dart:core`), e o tipo que precisa de um
    /// desconhecido fica sem resposta. Sem `args` (receptor implícito, a
    /// própria classe), tipo que cita parâmetro também fica sem resposta.
    fn membro_da_classe_com(
        &self,
        classe: ClassId,
        membro: &str,
        args: Option<&[String]>,
        livres: &[String],
    ) -> Option<(String, PathBuf)> {
        let sym = self.interner.lookup(membro)?;
        let (id, fid, args_de_quem_declara) = match args {
            None => {
                let (id, fid) = self.membro_de_instancia(classe, sym, 0)?;
                (id, fid, None)
            }
            Some(a) => {
                let iniciais = self.instanciar(classe, a, livres)?;
                let (id, fid, v) = self.membro_com_argumentos(classe, iniciais, sym, 0, livres)?;
                (id, fid, Some(v))
            }
        };
        let c = self.program.class(id);
        let (tipo, escopo) = self.tipo_da_funcao(fid)?;
        let proprios: Vec<&str> = c
            .type_params
            .iter()
            .map(|p| self.interner.resolve(p.name))
            .collect();
        let mut do_metodo: Vec<&str> = Vec::new();
        if let dartforge_elements::model::FunctionRef::Function { unit, function } =
            self.program.function(fid).node
        {
            let f = self.program.unit(unit).ast.function(function);
            do_metodo.extend(
                f.type_params
                    .iter()
                    .map(|p| self.interner.resolve(p.name.sym)),
            );
        }
        let palavras: Vec<&str> = tipo
            .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '$')
            .collect();
        // Parâmetro do método: o analyzer o infere na chamada; aqui não.
        if palavras.iter().any(|t| do_metodo.contains(t)) {
            return None;
        }
        if !palavras.iter().any(|t| proprios.contains(t)) {
            return Some((tipo, escopo));
        }
        let args = args_de_quem_declara?;
        let troca: Vec<(&str, String)> = proprios
            .iter()
            .zip(&args)
            .filter_map(|(n, a)| a.clone().map(|a| (*n, a)))
            .collect();
        // Um parâmetro citado cujo argumento não se conhece: sem resposta.
        if palavras
            .iter()
            .any(|t| proprios.contains(t) && !troca.iter().any(|(n, _)| n == t))
        {
            return None;
        }
        Some((substituir_palavras(&tipo, &troca), escopo))
    }

    /// Os argumentos de `classe` a partir dos escritos no receptor: cada um
    /// conhecido se se escreve igual em qualquer escopo; cru, a instanciação
    /// pelos limites (sem limite, `dynamic`).
    fn instanciar(
        &self,
        classe: ClassId,
        args: &[String],
        livres: &[String],
    ) -> Option<Vec<Option<String>>> {
        let c = self.program.class(classe);
        if args.is_empty() {
            return Some(
                c.type_params
                    .iter()
                    .map(|p| p.bound.is_none().then(|| "dynamic".to_string()))
                    .collect(),
            );
        }
        if args.len() != c.type_params.len() {
            return None;
        }
        Some(
            args.iter()
                .map(|a| escrito_igual_em_todo_escopo(a, livres).then(|| a.trim().to_string()))
                .collect(),
        )
    }

    /// Os argumentos de um supertipo escrito (`extends LabeledList<T>`),
    /// com os parâmetros da classe de baixo trocados pelos argumentos dela.
    fn argumentos_do_supertipo(
        &self,
        escrito: (
            dartforge_elements::model::UnitId,
            dartforge_frontend::ast::TypeId,
        ),
        superclasse: ClassId,
        troca: &[(&str, Option<String>)],
        livres: &[String],
    ) -> Vec<Option<String>> {
        let n = self.program.class(superclasse).type_params.len();
        let u = self.program.unit(escrito.0);
        let sp = u.ast.ty(escrito.1).span;
        let Some(texto) = u.source.get(sp.start..sp.end) else {
            return vec![None; n];
        };
        let (_, escritos) = separar_argumentos(texto.trim().trim_end_matches('?'));
        if escritos.is_empty() {
            return self
                .instanciar(superclasse, &[], livres)
                .unwrap_or_else(|| vec![None; n]);
        }
        if escritos.len() != n {
            return vec![None; n];
        }
        escritos
            .iter()
            .map(|a| {
                let palavras: Vec<&str> = a
                    .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '$')
                    .filter(|p| !p.is_empty())
                    .collect();
                let mut pares = Vec::new();
                for p in &palavras {
                    if let Some((n, v)) = troca.iter().find(|(n, _)| n == p) {
                        pares.push((*n, v.clone()?));
                    }
                }
                // Um nome livre que o supertipo cita sem ser parâmetro desta
                // classe é outro tipo (do escopo de quem escreveu): não vale.
                let so_parametros = palavras
                    .iter()
                    .all(|p| troca.iter().any(|(n, _)| n == p) || !livres.iter().any(|l| l == p));
                let novo = substituir_palavras(a, &pares);
                (so_parametros && escrito_igual_em_todo_escopo(&novo, livres)).then_some(novo)
            })
            .collect()
    }

    /// O membro `sym` visto de `classe` com os argumentos `args`, na ordem
    /// de busca do Dart (a classe, os mixins do último ao primeiro, a
    /// superclasse e, por fim, as interfaces — o `lookUpGetter2` do
    /// analyzer também acha membro abstrato), com os argumentos da classe
    /// que o declara.
    #[allow(clippy::type_complexity)]
    fn membro_com_argumentos(
        &self,
        classe: ClassId,
        args: Vec<Option<String>>,
        sym: dartforge_intern::SymbolId,
        profundidade: u32,
        livres: &[String],
    ) -> Option<(
        ClassId,
        dartforge_elements::model::FunctionElementId,
        Vec<Option<String>>,
    )> {
        if profundidade > 64 {
            return None;
        }
        let c = self.program.class(classe);
        if let Some(&fid) = c.instance_members.get(&sym) {
            return Some((classe, fid, args));
        }
        let troca: Vec<(&str, Option<String>)> = c
            .type_params
            .iter()
            .map(|p| self.interner.resolve(p.name))
            .zip(args.iter().cloned())
            .collect();
        // Um supertipo escrito que não resolveu deixa as listas de tamanhos
        // diferentes: sem o par, os argumentos ficam desconhecidos.
        let pareados = |classes: &[ClassId], escritos: &[_]| -> Vec<(ClassId, Option<_>)> {
            if classes.len() == escritos.len() {
                classes
                    .iter()
                    .copied()
                    .zip(escritos.iter().copied().map(Some))
                    .collect()
            } else {
                classes.iter().map(|&k| (k, None)).collect()
            }
        };
        let argumentos = |k: ClassId, escrito: Option<_>| match escrito {
            Some(e) => self.argumentos_do_supertipo(e, k, &troca, livres),
            None => vec![None; self.program.class(k).type_params.len()],
        };
        for (m, escrito) in pareados(&c.mixin_classes, &c.mixins).into_iter().rev() {
            let a = argumentos(m, escrito);
            if let Some(achado) = self.membro_com_argumentos(m, a, sym, profundidade + 1, livres) {
                return Some(achado);
            }
        }
        if let Some(sc) = c.supertype_class {
            let a = match c.supertype {
                Some(escrito) => self.argumentos_do_supertipo(escrito, sc, &troca, livres),
                None => Vec::new(),
            };
            if let Some(achado) = self.membro_com_argumentos(sc, a, sym, profundidade + 1, livres) {
                return Some(achado);
            }
        }
        for (i, escrito) in pareados(&c.interface_classes, &c.interfaces) {
            let a = argumentos(i, escrito);
            if let Some(achado) = self.membro_com_argumentos(i, a, sym, profundidade + 1, livres) {
                return Some(achado);
            }
        }
        None
    }

    /// O tipo que um acessor devolve: de um campo, o tipo escrito no campo;
    /// de um getter, o retorno declarado. Junto, o arquivo em que esse texto
    /// foi escrito, que é o escopo em que ele se resolve.
    fn tipo_da_funcao(
        &self,
        fid: dartforge_elements::model::FunctionElementId,
    ) -> Option<(String, PathBuf)> {
        let f = self.program.function(fid);
        if let Some(vid) = f.variable {
            let v = self.program.variable(vid);
            if let dartforge_elements::model::VariableRef::Field { unit, member, .. } = v.node {
                let u = self.program.unit(unit);
                let dartforge_frontend::ast::MemberKind::Field(lista) = &u.ast.member(member).kind
                else {
                    return None;
                };
                let t = lista.ty?;
                let s = u.ast.ty(t).span;
                let texto = u.source.get(s.start..s.end)?;
                return Some((texto.to_string(), u.path.clone()?));
            }
            return None;
        }
        let dartforge_elements::model::FunctionRef::Function { unit, function } = f.node else {
            return None;
        };
        let u = self.program.unit(unit);
        let funcao = u.ast.function(function);
        let t = funcao.return_type?;
        let s = u.ast.ty(t).span;
        Some((u.source.get(s.start..s.end)?.to_string(), u.path.clone()?))
    }
}

/// `A` ou `p.A`, como escrito numa lista de `directives:`.
pub(crate) fn nome_qualificado(
    arvore: &dartforge_frontend::ast::Ast,
    interner: &Interner,
    id: dartforge_frontend::ast::ExprId,
) -> Option<String> {
    use dartforge_frontend::ast::ExprKind;
    match &arvore.expr(id).kind {
        ExprKind::Identifier(n) => Some(interner.resolve(n.sym).to_string()),
        ExprKind::Property {
            target,
            name,
            null_aware: false,
        } => match &arvore.expr(*target).kind {
            ExprKind::Identifier(p) => Some(format!(
                "{}.{}",
                interner.resolve(p.sym),
                interner.resolve(name.sym)
            )),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O caso do `CallbackComponent`: o oficial escreveu
    /// `'../../../../shared/services/oidc_service.dart'` e
    /// `'package:ngrouter/src/router/router.dart'`.
    #[test]
    fn caminhos_como_no_oficial() {
        let modulo =
            "asset:new_sali_frontend/lib/src/modules/auth/pages/callback/callback_component.dart";
        assert_eq!(
            caminho_do_import(
                modulo,
                "asset:new_sali_frontend/lib/src/shared/services/oidc_service.dart"
            )
            .unwrap(),
            "../../../../shared/services/oidc_service.dart"
        );
        assert_eq!(
            caminho_do_import(modulo, "asset:ngrouter/lib/src/router/router.dart").unwrap(),
            "package:ngrouter/src/router/router.dart"
        );
        assert_eq!(
            caminho_do_import(modulo, modulo).unwrap(),
            "callback_component.dart"
        );
    }

    #[test]
    fn asset_de_package() {
        assert_eq!(
            asset_de_uri(
                "package:ngrouter/src/router/router.dart",
                "x",
                Path::new("/p")
            )
            .unwrap(),
            "asset:ngrouter/lib/src/router/router.dart"
        );
    }
}

/// `Grupo<A, B<C>>` em `("Grupo", ["A", "B<C>"])`; sem `<`, lista vazia.
pub(crate) fn separar_argumentos(tipo: &str) -> (&str, Vec<String>) {
    let Some(i) = tipo.find('<') else {
        return (tipo, Vec::new());
    };
    let dentro = tipo[i + 1..]
        .trim_end()
        .strip_suffix('>')
        .unwrap_or(&tipo[i + 1..]);
    let mut args = Vec::new();
    let mut nivel = 0i32;
    let mut atual = String::new();
    for ch in dentro.chars() {
        match ch {
            '<' | '(' => nivel += 1,
            '>' | ')' => nivel -= 1,
            ',' if nivel == 0 => {
                args.push(atual.trim().to_string());
                atual.clear();
                continue;
            }
            _ => {}
        }
        atual.push(ch);
    }
    if !atual.trim().is_empty() {
        args.push(atual.trim().to_string());
    }
    (tipo[..i].trim(), args)
}

/// Um argumento de tipo que se escreve igual em qualquer arquivo:
/// `dynamic`, `void` e os tipos do `dart:core` sem prefixo (com `?` ou
/// argumentos também assim).
fn escrito_igual_em_todo_escopo(tipo: &str, livres: &[String]) -> bool {
    let t = tipo.trim().trim_end_matches('?');
    let (base, args) = separar_argumentos(t);
    let do_core = matches!(
        base,
        "dynamic"
            | "void"
            | "Object"
            | "int"
            | "double"
            | "num"
            | "String"
            | "bool"
            | "List"
            | "Map"
            | "Set"
            | "Iterable"
            | "Null"
            | "Never"
    );
    (do_core || (args.is_empty() && livres.iter().any(|l| l == base)))
        && args.iter().all(|a| escrito_igual_em_todo_escopo(a, livres))
}

/// Troca cada palavra de `texto` que é um dos nomes de `troca` pelo valor.
fn substituir_palavras(texto: &str, troca: &[(&str, String)]) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut palavra = String::new();
    let fechar = |palavra: &mut String, saida: &mut String| {
        match troca.iter().find(|(n, _)| *n == palavra.as_str()) {
            Some((_, v)) => saida.push_str(v),
            None => saida.push_str(palavra),
        }
        palavra.clear();
    };
    for ch in texto.chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '$' {
            palavra.push(ch);
        } else {
            fechar(&mut palavra, &mut saida);
            saida.push(ch);
        }
    }
    fechar(&mut palavra, &mut saida);
    saida
}
