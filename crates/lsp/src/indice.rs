//! Índices de nomes públicos de topo, para o completar com importação
//! automática e para a ação "importar biblioteca".
//!
//! * [`IndiceSdk`]: as bibliotecas públicas do SDK (sem `_`) com as suas
//!   partes, montado uma vez por SDK; o tamanho é o do SDK.
//! * [`IndiceProjeto`]: os arquivos `.dart` de **um** projeto (o último
//!   consultado), por arquivo, com a impressão de cada um (versão do texto
//!   aberto ou data e tamanho no disco). Só o arquivo que mudou é analisado
//!   de novo; arquivo que sumiu sai do índice; trocar de projeto descarta o
//!   anterior. Guarda só nomes, espécies e offsets, nunca árvores.

use crate::DocumentStore;
use crate::completar::especie;
use crate::projeto::{arquivos_do_projeto, eh_parte};
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use url::Url;

/// Um nome público de topo declarado num arquivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Declarado {
    pub nome: String,
    /// `CompletionItemKind`.
    pub especie: u32,
    /// Nomeia um tipo (classe, mixin, enum, extension type, typedef).
    pub tipo: bool,
    /// Função sem parâmetros (o rótulo é `f()`, não `f(…)`).
    pub sem_parametros: bool,
    /// Arquivo e início da declaração (com metadados), para a documentação.
    pub arquivo: PathBuf,
    pub inicio: usize,
}

/// Os nomes públicos de topo de uma unidade analisada.
pub(crate) fn declarados(
    ast: &ast::Ast,
    unit: &ast::CompilationUnit,
    nomes: &dartforge_intern::Interner,
    arquivo: &Path,
) -> Vec<Declarado> {
    let mut saida = Vec::new();
    for &d in &unit.declarations {
        let decl = ast.decl(d);
        let mut empurrar =
            |n: Option<ast::Name>, especie: u32, tipo: bool, sem_parametros: bool| {
                if let Some(n) = n {
                    let s = nomes.resolve(n.sym);
                    if !s.starts_with('_') {
                        saida.push(Declarado {
                            nome: s.to_string(),
                            especie,
                            tipo,
                            sem_parametros,
                            arquivo: arquivo.to_path_buf(),
                            inicio: decl.span.start,
                        });
                    }
                }
            };
        match &decl.kind {
            DeclKind::Class(c) => empurrar(Some(c.name), especie::CLASSE, true, false),
            DeclKind::Mixin(m) => empurrar(Some(m.name), especie::CLASSE, true, false),
            DeclKind::Enum(e) => empurrar(Some(e.name), especie::ENUM, true, false),
            DeclKind::ExtensionType(e) => empurrar(Some(e.name), especie::CLASSE, true, false),
            DeclKind::Typedef(t) => empurrar(Some(t.name), especie::CLASSE, true, false),
            DeclKind::Extension(x) => empurrar(x.name, especie::CLASSE, false, false),
            DeclKind::Function(f) => {
                let func = ast.function(*f);
                let (esp, sem) = match func.kind {
                    ast::FunctionKind::Getter | ast::FunctionKind::Setter => {
                        (especie::VARIAVEL, false)
                    }
                    _ => (
                        especie::FUNCAO,
                        func.parameters.as_ref().is_none_or(|p| p.is_empty()),
                    ),
                };
                empurrar(func.name, esp, false, sem)
            }
            DeclKind::Variables(vl) => {
                for v in vl.variables.iter() {
                    empurrar(Some(v.name), especie::VARIAVEL, false, false);
                }
            }
        }
    }
    saida
}

/// Nomes públicos de topo do SDK: nome → (URI `dart:` → declaração).
#[derive(Debug, Default)]
pub(crate) struct IndiceSdk {
    pub por_nome: HashMap<String, BTreeMap<String, Declarado>>,
}

/// Monta o índice do SDK: cada biblioteca pública (sem `_`) com as suas
/// partes.
pub(crate) fn indexar_sdk(sdk: &SdkLayout) -> IndiceSdk {
    let mut indice = IndiceSdk::default();
    let mut bibliotecas: Vec<_> = sdk
        .libraries
        .values()
        .filter(|l| !l.name.starts_with('_') && l.supported)
        .collect();
    bibliotecas.sort_by(|a, b| a.name.cmp(&b.name));
    for lib in bibliotecas {
        let uri = format!("dart:{}", lib.name);
        let mut pendentes = vec![lib.path.clone()];
        let mut vistos = std::collections::BTreeSet::new();
        while let Some(caminho) = pendentes.pop() {
            if !vistos.insert(caminho.clone()) {
                continue;
            }
            let Ok(texto) = std::fs::read_to_string(&caminho) else {
                continue;
            };
            let mut nomes = dartforge_intern::Interner::new();
            let analisado = dartforge_frontend::parser::parse(&texto, &mut nomes);
            for d in declarados(&analisado.ast, &analisado.unit, &nomes, &caminho) {
                indice
                    .por_nome
                    .entry(d.nome.clone())
                    .or_default()
                    .entry(uri.clone())
                    .or_insert(d);
            }
            for d in &analisado.unit.directives {
                if let DirectiveKind::Part { uri } = &d.kind
                    && let Some(relativo) = dartforge_elements::load::string_lit_value(uri)
                    && let Some(dir) = caminho.parent()
                {
                    pendentes.push(dir.join(relativo));
                }
            }
        }
    }
    indice
}

/// Impressão de um arquivo: versão e tamanho do texto aberto, ou data e
/// tamanho no disco.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Impressao {
    Aberto(i32, usize),
    Disco(Option<(SystemTime, u64)>),
}

#[derive(Debug)]
struct EntradaDeArquivo {
    impressao: Impressao,
    /// O arquivo é uma parte (os nomes dele são da biblioteca dona e não se
    /// importam por ele).
    parte: bool,
    nomes: Vec<Declarado>,
}

/// Índice incremental dos nomes públicos de topo de um projeto.
#[derive(Debug, Default)]
pub(crate) struct IndiceProjeto {
    raiz: Option<PathBuf>,
    arquivos: HashMap<PathBuf, EntradaDeArquivo>,
    /// Arquivos analisados na última atualização (medição e testes).
    pub(crate) reanalisados: usize,
}

impl IndiceProjeto {
    /// Atualiza o índice do projeto `raiz` com os textos vigentes e devolve
    /// as bibliotecas (arquivos que não são partes) com os nomes delas.
    pub(crate) fn atualizar(
        &mut self,
        raiz: &Path,
        documentos: &DocumentStore,
    ) -> Vec<(&Path, &[Declarado])> {
        if self.raiz.as_deref() != Some(raiz) {
            self.raiz = Some(raiz.to_path_buf());
            self.arquivos.clear();
        }
        self.reanalisados = 0;
        let arquivos = arquivos_do_projeto(raiz);
        self.arquivos
            .retain(|p, _| arquivos.binary_search(p).is_ok());
        for arquivo in &arquivos {
            let uri = Url::from_file_path(arquivo).ok();
            let aberto = uri
                .as_ref()
                .and_then(|u| Some((documentos.version(u.as_str())?, documentos.get(u.as_str())?)));
            let impressao = match aberto {
                Some((v, t)) => Impressao::Aberto(v, t.len()),
                None => Impressao::Disco(
                    std::fs::metadata(arquivo)
                        .ok()
                        .and_then(|m| Some((m.modified().ok()?, m.len()))),
                ),
            };
            if self
                .arquivos
                .get(arquivo)
                .is_some_and(|e| e.impressao == impressao)
            {
                continue;
            }
            let texto = match aberto {
                Some((_, t)) => Some(t.to_string()),
                None => std::fs::read_to_string(arquivo).ok(),
            };
            let Some(texto) = texto else {
                self.arquivos.remove(arquivo);
                continue;
            };
            self.reanalisados += 1;
            let parte = eh_parte(&texto);
            let mut nomes = dartforge_intern::Interner::new();
            let analisado = dartforge_frontend::parser::parse(&texto, &mut nomes);
            let declarados = declarados(&analisado.ast, &analisado.unit, &nomes, arquivo);
            self.arquivos.insert(
                arquivo.clone(),
                EntradaDeArquivo {
                    impressao,
                    parte,
                    nomes: declarados,
                },
            );
        }
        let mut saida: Vec<(&Path, &[Declarado])> = self
            .arquivos
            .iter()
            .filter(|(_, e)| !e.parte)
            .map(|(p, e)| (p.as_path(), e.nomes.as_slice()))
            .collect();
        saida.sort_by(|a, b| a.0.cmp(b.0));
        saida
    }
}

/// Caminho de `destino` relativo ao diretório `base`, com `/`.
pub(crate) fn relativo(destino: &Path, base: &Path) -> Option<String> {
    use std::path::Component;
    let d: Vec<Component> = destino.components().collect();
    let b: Vec<Component> = base.components().collect();
    let comum = d.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if comum == 0 {
        return None;
    }
    let mut partes: Vec<String> = b[comum..].iter().map(|_| "..".to_string()).collect();
    partes.extend(
        d[comum..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    Some(partes.join("/"))
}

/// Nome do pacote no `pubspec.yaml` da raiz.
pub(crate) fn nome_do_pacote(raiz: &Path) -> Option<String> {
    let texto = std::fs::read_to_string(raiz.join("pubspec.yaml")).ok()?;
    texto.lines().find_map(|l| {
        l.strip_prefix("name:")
            .map(|n| n.trim().trim_matches(['\'', '"']).to_string())
    })
}

/// A URI com que `arquivo` importa `destino` do mesmo projeto: `package:`
/// quando `arquivo` está fora de `lib/` e `destino` dentro; senão relativa.
pub(crate) fn uri_de_import(
    arquivo: &Path,
    destino: &Path,
    raiz: &Path,
    pacote: Option<&str>,
) -> Option<String> {
    let lib = raiz.join("lib");
    match pacote {
        Some(p) if destino.starts_with(&lib) && !arquivo.starts_with(&lib) => {
            relativo(destino, &lib).map(|r| format!("package:{p}/{r}"))
        }
        _ => arquivo.parent().and_then(|dir| relativo(destino, dir)),
    }
}
