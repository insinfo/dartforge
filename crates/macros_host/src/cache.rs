//! O cache de expansões (docs/MACROS-PROTOCOLO.md §6): reaproveita o
//! resultado de uma aplicação de macro numa fase sem falar com o executor,
//! quando tudo o que a execução anterior **observou** continua igual.
//!
//! Para cada `(aplicação, fase)` o cache guarda um registro:
//!
//! * a impressão digital do **pedido** `macro.executar` — a identidade da
//!   implementação da macro (o fonte do fecho de imports da biblioteca dela),
//!   o construtor e os argumentos, a fase, o alvo e o modelo com os membros
//!   pré-carregados —, mais a identidade do executor e a versão do
//!   hospedeiro;
//! * cada `macro.consulta`, **na ordem**, com a impressão digital da
//!   resposta — inclusive as negativas (erro "não existe") e as coleções na
//!   ordem em que o executor as recebeu;
//! * o resultado.
//!
//! Numa nova aplicação o hospedeiro monta o pedido de novo; se a impressão
//! bate, refaz as consultas gravadas contra o programa corrente (a
//! *revalidação*, como a do motor de build, docs/BUILD-MOTOR.md §4). Tudo
//! igual, o resultado gravado vale — a macro é uma função determinística do
//! que recebeu (a spec proíbe E/S fora de `Resource`, e o estado da instância
//! entre fases não é garantido). Qualquer diferença, a macro roda de novo e o
//! registro é trocado. A primeira consulta que difere interrompe a
//! revalidação: as seguintes dependiam da resposta antiga.
//!
//! **Os ids.** O pedido e as respostas carregam os ids da [`Tabela`] e as
//! chaves de `StaticType` ([`Estaticos`]). O cache guarda os dois e os usa
//! em todas as compilações que servir: um id nunca muda de significado, e um
//! identificador novo (um campo acrescentado) ganha id novo sem deslocar os
//! outros. Por isso a edição que uma aplicação não observa não muda o pedido
//! nem as respostas dela. Uma compilação sem cache começa de tabela vazia,
//! com os ids de sempre (os da sessão gravada do CFE).
//!
//! **Onde vive.** Em memória, com quem compila várias vezes no mesmo processo
//! (o `dartforge dev`, o LSP, os testes): o motor de build também não grava
//! registro em disco (docs/BUILD-MOTOR.md §4, D-B1), e a regra governante 6
//! das macros proíbe contabilidade em disco.
use crate::aplicacoes::{Alvo, Aplicacao};
use crate::consultas::Estaticos;
use crate::executor::{ErroDeConsulta, Fase, ServicoDeConsultas};
use crate::modelo::{Chave, Tabela};
use crate::montagem::Resultado;
use crate::protocolo::VERSAO_DO_HOSPEDEIRO;
use dartforge_elements::model::{LibraryId, Program};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Impressão digital (blake3).
pub type Digest = [u8; 32];

/// A identidade **estável** de uma aplicação: o que ela é, não onde está na
/// AST. Sobrevive a edições em outras declarações e a recargas; duas
/// anotações iguais no mesmo alvo se distinguem pelo `ordinal`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentidadeDaAplicacao {
    pub biblioteca: String,
    pub alvo: Alvo,
    /// `uri#Classe` da macro.
    pub macro_: String,
    pub construtor: String,
    /// Os argumentos no formato do protocolo, em JSON.
    pub argumentos: String,
    /// Quantas aplicações idênticas vêm antes desta no mesmo alvo.
    pub ordinal: usize,
}

impl IdentidadeDaAplicacao {
    /// As identidades de `apps`, na mesma ordem.
    pub fn de_todas(apps: &[Aplicacao]) -> Vec<IdentidadeDaAplicacao> {
        let mut out: Vec<IdentidadeDaAplicacao> = Vec::with_capacity(apps.len());
        for a in apps {
            let mut id = IdentidadeDaAplicacao {
                biblioteca: a.biblioteca.clone(),
                alvo: a.alvo.clone(),
                macro_: a.macro_.clone(),
                construtor: a.construtor.clone(),
                argumentos: a.argumentos.to_string(),
                ordinal: 0,
            };
            id.ordinal = out.iter().filter(|x| **x == id).count();
            out.push(id);
        }
        out
    }
}

impl std::fmt::Display for IdentidadeDaAplicacao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let alvo = match &self.alvo {
            Alvo::Biblioteca(_) => "library".to_string(),
            Alvo::Declaracao(
                Chave::Metodo { dono, nome, .. } | Chave::Campo { dono, nome, .. } | Chave::Construtor { dono, nome, .. },
            ) => format!("{dono}.{nome}"),
            Alvo::Declaracao(c) => c.nome().to_string(),
        };
        let classe = self.macro_.rsplit('#').next().unwrap_or("");
        let construtor = if self.construtor.is_empty() { String::new() } else { format!(".{}", self.construtor) };
        write!(f, "{} {alvo} @{classe}{construtor}", self.biblioteca)?;
        if self.ordinal > 0 {
            write!(f, " #{}", self.ordinal)?;
        }
        Ok(())
    }
}

/// Uma instância de macro: a implementação, o construtor e os argumentos.
/// A implementação entra pela impressão digital do fonte (não pelo nome):
/// editar a macro invalida as instâncias dela.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ChaveDeInstancia {
    pub implementacao: Digest,
    pub macro_: String,
    pub construtor: String,
    pub argumentos: String,
}

/// Uma consulta feita pela execução e a impressão digital da resposta.
#[derive(Debug, Clone)]
pub(crate) struct ConsultaGravada {
    tipo: String,
    args: Value,
    resposta: Digest,
}

/// O que uma execução leu e produziu.
#[derive(Debug, Clone)]
pub(crate) struct Registro {
    pedido: Digest,
    consultas: Vec<ConsultaGravada>,
    resultado: Resultado,
}

/// O cache de expansões de um executor. Um valor por configuração de
/// executor: a identidade dada em [`CacheDeMacros::novo`] entra em todas as
/// impressões digitais.
///
/// ```
/// use dartforge_macros_host::cache::CacheDeMacros;
/// let cache = CacheDeMacros::novo("vm dart-3.6.2");
/// assert!(cache.is_empty());
/// ```
#[derive(Debug)]
pub struct CacheDeMacros {
    /// Sem cache (compilação única): nada é gravado nem revalidado.
    pub(crate) ativo: bool,
    pub(crate) identidade_do_executor: String,
    pub(crate) tabela: Tabela,
    pub(crate) estaticos: Estaticos,
    /// As interfaces de macro de cada instância (as fases que ela roda):
    /// com elas o hospedeiro decide as fases sem instanciar no executor.
    pub(crate) interfaces: HashMap<ChaveDeInstancia, Vec<String>>,
    pub(crate) registros: Registros,
}

impl CacheDeMacros {
    /// Um cache vazio para o executor identificado por `identidade_do_executor`
    /// (o executável e a versão da API de macros, por exemplo).
    pub fn novo(identidade_do_executor: impl Into<String>) -> Self {
        CacheDeMacros {
            ativo: true,
            identidade_do_executor: identidade_do_executor.into(),
            tabela: Tabela::default(),
            estaticos: Estaticos::default(),
            interfaces: HashMap::new(),
            registros: HashMap::new(),
        }
    }

    /// O estado de uma compilação sem cache: tabela nova e nada gravado.
    pub(crate) fn desligado() -> Self {
        CacheDeMacros { ativo: false, ..CacheDeMacros::novo("") }
    }

    /// Quantas expansões `(aplicação, fase)` estão gravadas.
    pub fn len(&self) -> usize {
        self.registros.len()
    }

    pub fn is_empty(&self) -> bool {
        self.registros.is_empty()
    }

}

/// Os registros por `(aplicação, fase)`.
pub(crate) type Registros = HashMap<(IdentidadeDaAplicacao, Fase), Registro>;

/// O resultado gravado de `chave`, se ainda vale para `pedido` e as consultas
/// gravadas respondem igual em `consultas` (o programa corrente).
pub(crate) fn revalidar(
    registros: &Registros,
    chave: &(IdentidadeDaAplicacao, Fase),
    pedido: &Digest,
    consultas: &mut dyn ServicoDeConsultas,
) -> Revalidacao {
    let Some(r) = registros.get(chave) else { return Revalidacao::Ausente };
    if r.pedido != *pedido {
        return Revalidacao::PedidoMudou;
    }
    for (i, c) in r.consultas.iter().enumerate() {
        if digest_da_resposta(&consultas.consultar(&c.tipo, &c.args)) != c.resposta {
            return Revalidacao::ConsultaMudou { refeitas: i + 1 };
        }
    }
    Revalidacao::Valido { resultado: Box::new(r.resultado.clone()), refeitas: r.consultas.len() }
}

/// O desfecho de uma revalidação.
pub(crate) enum Revalidacao {
    Ausente,
    PedidoMudou,
    /// A consulta de número `refeitas` respondeu diferente.
    ConsultaMudou { refeitas: usize },
    Valido { resultado: Box<Resultado>, refeitas: usize },
}

/// Grava um [`Registro`] novo (depois de executar) e diz se o resultado é o
/// mesmo do registro anterior — "saída igual": as aplicações seguintes não
/// perdem o cache por causa desta, porque o programa que elas veem não muda.
pub(crate) fn gravar(
    registros: &mut Registros,
    chave: (IdentidadeDaAplicacao, Fase),
    pedido: Digest,
    consultas: Vec<ConsultaGravada>,
    resultado: &Resultado,
) -> bool {
    let igual = registros.get(&chave).is_some_and(|r| r.resultado == *resultado);
    registros.insert(chave, Registro { pedido, consultas, resultado: resultado.clone() });
    igual
}

/// Esquece o que esta compilação não usou (aplicações removidas): o cache
/// não cresce com o histórico de edições.
pub(crate) fn podar(
    registros: &mut Registros,
    interfaces: &mut HashMap<ChaveDeInstancia, Vec<String>>,
    usadas: &HashSet<(IdentidadeDaAplicacao, Fase)>,
    instancias: &HashSet<ChaveDeInstancia>,
) {
    registros.retain(|k, _| usadas.contains(k));
    interfaces.retain(|k, _| instancias.contains(k));
}

/// Registra, em ordem, as consultas que passam para `interno` (só com o
/// cache ligado: sem ele, não há o que revalidar depois).
pub(crate) struct Gravador<'s> {
    interno: &'s mut dyn ServicoDeConsultas,
    ligado: bool,
    pub(crate) consultas: Vec<ConsultaGravada>,
}

impl<'s> Gravador<'s> {
    pub fn novo(interno: &'s mut dyn ServicoDeConsultas, ligado: bool) -> Self {
        Gravador { interno, ligado, consultas: Vec::new() }
    }
}

impl ServicoDeConsultas for Gravador<'_> {
    fn consultar(&mut self, tipo: &str, args: &Value) -> Result<Value, ErroDeConsulta> {
        let r = self.interno.consultar(tipo, args);
        if self.ligado {
            self.consultas.push(ConsultaGravada { tipo: tipo.to_string(), args: args.clone(), resposta: digest_da_resposta(&r) });
        }
        r
    }
}

fn campo(h: &mut blake3::Hasher, s: &str) {
    h.update(&(s.len() as u64).to_le_bytes());
    h.update(s.as_bytes());
}

/// A impressão digital de uma resposta, inclusive de erro (a resposta
/// negativa também é dependência).
fn digest_da_resposta(r: &Result<Value, ErroDeConsulta>) -> Digest {
    let mut h = blake3::Hasher::new();
    match r {
        Ok(v) => {
            campo(&mut h, "ok");
            campo(&mut h, &v.to_string());
        }
        Err(e) => {
            campo(&mut h, "erro");
            campo(&mut h, e.tipo);
            campo(&mut h, &e.mensagem);
        }
    }
    *h.finalize().as_bytes()
}

/// A impressão digital do pedido `macro.executar`, sem o número da
/// instância (que é do processo executor, não da macro).
pub(crate) fn digest_do_pedido(executor: &str, instancia: &ChaveDeInstancia, fase: Fase, alvo: &Value, modelo: &Value) -> Digest {
    let mut h = blake3::Hasher::new();
    campo(&mut h, VERSAO_DO_HOSPEDEIRO);
    campo(&mut h, executor);
    h.update(&instancia.implementacao);
    campo(&mut h, &instancia.macro_);
    campo(&mut h, &instancia.construtor);
    campo(&mut h, &instancia.argumentos);
    campo(&mut h, fase.nome());
    campo(&mut h, &alvo.to_string());
    campo(&mut h, &modelo.to_string());
    *h.finalize().as_bytes()
}

/// A identidade da implementação da macro declarada na biblioteca `uri`:
/// o fonte de todas as unidades do fecho de imports e exports dela (as do
/// SDK entram pela URI; a versão do SDK é da identidade do executor).
pub(crate) fn identidade_da_implementacao(program: &Program, uri: &str) -> Digest {
    let mut h = blake3::Hasher::new();
    campo(&mut h, VERSAO_DO_HOSPEDEIRO);
    campo(&mut h, uri);
    let Some(inicio) = program.libraries.iter().position(|l| l.uri == uri) else {
        return *h.finalize().as_bytes();
    };
    let mut vistas: HashSet<LibraryId> = HashSet::new();
    let mut pilha = vec![LibraryId(inicio as u32)];
    while let Some(l) = pilha.pop() {
        if !vistas.insert(l) {
            continue;
        }
        let lib = program.library(l);
        pilha.extend(lib.imports.iter().map(|i| i.library));
        pilha.extend(lib.exports.iter().map(|e| e.library));
    }
    let mut libs: Vec<LibraryId> = vistas.into_iter().collect();
    libs.sort_by(|a, b| program.library(*a).uri.cmp(&program.library(*b).uri));
    for l in libs {
        let lib = program.library(l);
        campo(&mut h, &lib.uri);
        if lib.is_sdk {
            continue;
        }
        for &u in &lib.units {
            let unit = program.unit(u);
            campo(&mut h, &unit.uri);
            campo(&mut h, &unit.source);
        }
    }
    *h.finalize().as_bytes()
}
