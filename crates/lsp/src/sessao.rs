//! Sessão semântica limitada: o último [`Projeto`] carregado é reaproveitado
//! pelas consultas seguintes enquanto nada do que ele leu mudou.
//!
//! Sem ela, cada `hover`, `definition`, `references`, `prepareRename` +
//! `rename` e `codeAction` recarrega o programa (as bibliotecas `dart:`
//! dominam o custo) e infere os corpos de novo, mesmo quando o editor pede
//! várias consultas sobre a mesma versão do texto (o VS Code pede
//! `codeAction` a cada movimento do cursor, e `rename` vem depois de
//! `prepareRename`). A política de memória continua explícita:
//!
//! * **uma** entrada, no máximo: o programa retido é o de uma consulta, não
//!   um cache por arquivo ou por versão;
//! * a entrada cai (o `drop` é imediato) a cada `didOpen`, `didChange` ou
//!   `didClose` — nenhuma árvore de uma versão velha fica presa;
//! * antes de reaproveitar, a chave é conferida: a versão e o tamanho de
//!   cada documento aberto, e a data e o tamanho de cada arquivo do disco
//!   que entrou na carga, mais a lista de arquivos `.dart` do projeto (um
//!   arquivo novo pode satisfazer um import);
//! * só retém o programa cuja fonte somada cabe no orçamento
//!   (`DARTFORGE_LSP_SESSAO_MIB`, em MiB de fonte; padrão
//!   [`ORCAMENTO_PADRAO_MIB`]; `0` desliga a sessão). O custo vivo medido é
//!   proporcional à fonte (`docs/LSP.md`, "Sessão semântica").

use crate::DocumentStore;
use crate::projeto::{Projeto, arquivos_do_projeto};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Orçamento padrão, em MiB de fonte carregada (SDK incluído).
pub(crate) const ORCAMENTO_PADRAO_MIB: usize = 8;

/// O que a entrada carregou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Escopo {
    /// Só a biblioteca do arquivo (definição, hover, ações).
    Biblioteca(PathBuf),
    /// Todas as bibliotecas do projeto com essa raiz (referências, renomear).
    Projeto(PathBuf),
}

/// O que precisa continuar igual para a entrada valer.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Chave {
    escopo: Escopo,
    /// (URI, versão, tamanho) de cada documento aberto, em ordem.
    abertos: Vec<(String, i32, usize)>,
    /// Arquivos lidos do disco na carga, com (data, tamanho).
    disco: Vec<(PathBuf, Option<(SystemTime, u64)>)>,
    /// Arquivos `.dart` do projeto no momento da carga.
    arquivos: Vec<PathBuf>,
}

/// Contadores da sessão (medição e testes).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EstatisticasSessao {
    /// Consultas atendidas pela entrada retida.
    pub reaproveitadas: u64,
    /// Consultas que carregaram o programa.
    pub carregadas: u64,
    /// Entradas descartadas por mudança de texto ou do disco.
    pub invalidadas: u64,
    /// Cargas não retidas por passar do orçamento.
    pub acima_do_orcamento: u64,
    /// Bytes de fonte da entrada retida agora (0 sem entrada).
    pub fonte_retida: usize,
}

/// A sessão: no máximo um programa retido.
pub(crate) struct Sessao {
    entrada: Option<(Chave, Projeto)>,
    orcamento: usize,
    pub(crate) estatisticas: EstatisticasSessao,
}

impl Sessao {
    /// Sessão com o orçamento de `DARTFORGE_LSP_SESSAO_MIB` (ou o padrão).
    pub(crate) fn nova() -> Self {
        let mib = std::env::var("DARTFORGE_LSP_SESSAO_MIB")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(ORCAMENTO_PADRAO_MIB);
        Self::com_orcamento(mib)
    }

    /// Sessão com orçamento de `mib` MiB de fonte (`0`: nunca retém).
    pub(crate) fn com_orcamento(mib: usize) -> Self {
        Self {
            entrada: None,
            orcamento: mib.saturating_mul(1024 * 1024),
            estatisticas: EstatisticasSessao::default(),
        }
    }

    /// Descarta a entrada (um texto mudou). O programa cai aqui, não depois.
    pub(crate) fn invalidar(&mut self) {
        if self.entrada.take().is_some() {
            self.estatisticas.invalidadas += 1;
            self.estatisticas.fonte_retida = 0;
        }
    }

    /// O projeto para `escopo`: o retido, se a chave ainda vale (uma entrada
    /// de projeto inteiro também serve a uma consulta de biblioteca do mesmo
    /// projeto), senão o que `carregar` devolve — retido se couber no
    /// orçamento, ou devolvido só para esta consulta.
    pub(crate) fn obter(
        &mut self,
        escopo: Escopo,
        documentos: &DocumentStore,
        carregar: impl FnOnce() -> Option<Projeto>,
    ) -> Option<Uso<'_>> {
        let abertos = abertos(documentos);
        let serve = self.entrada.as_ref().is_some_and(|(chave, _)| {
            let escopo_serve = match (&chave.escopo, &escopo) {
                (a, b) if a == b => true,
                (Escopo::Projeto(raiz), Escopo::Biblioteca(arquivo)) => arquivo.starts_with(raiz),
                _ => false,
            };
            escopo_serve && chave.abertos == abertos && disco_igual(chave)
        });
        if serve {
            self.estatisticas.reaproveitadas += 1;
            return self.entrada.as_ref().map(|(_, p)| Uso::Retido(p));
        }
        self.invalidar();
        let projeto = carregar()?;
        self.estatisticas.carregadas += 1;
        let fonte: usize = projeto
            .programa()
            .units
            .iter()
            .map(|u| u.source.len())
            .sum();
        if self.orcamento == 0 || fonte > self.orcamento {
            // Não cabe: vale só para esta consulta e cai com a resposta.
            self.estatisticas.acima_do_orcamento += 1;
            return Some(Uso::Proprio(Box::new(projeto)));
        }
        let chave = Chave {
            disco: arquivos_lidos(&projeto, documentos),
            arquivos: arquivos_do_projeto(&projeto.raiz),
            escopo,
            abertos,
        };
        self.estatisticas.fonte_retida = fonte;
        self.entrada = Some((chave, projeto));
        self.entrada.as_ref().map(|(_, p)| Uso::Retido(p))
    }
}

/// Um projeto emprestado da sessão ou próprio da consulta (acima do
/// orçamento), usado do mesmo jeito.
pub(crate) enum Uso<'a> {
    Retido(&'a Projeto),
    Proprio(Box<Projeto>),
}

impl std::ops::Deref for Uso<'_> {
    type Target = Projeto;

    fn deref(&self) -> &Projeto {
        match self {
            Uso::Retido(p) => p,
            Uso::Proprio(p) => p,
        }
    }
}

/// (URI, versão, tamanho) dos documentos abertos, em ordem.
fn abertos(documentos: &DocumentStore) -> Vec<(String, i32, usize)> {
    let mut v: Vec<(String, i32, usize)> = documentos
        .uris()
        .map(|u| {
            (
                u.to_string(),
                documentos.version(u).unwrap_or(0),
                documentos.get(u).map_or(0, str::len),
            )
        })
        .collect();
    v.sort();
    v
}

/// Data de modificação e tamanho de um arquivo.
fn metadados(caminho: &Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(caminho).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// Os arquivos que a carga leu do disco (os abertos vieram do editor).
fn arquivos_lidos(
    projeto: &Projeto,
    documentos: &DocumentStore,
) -> Vec<(PathBuf, Option<(SystemTime, u64)>)> {
    let mut v: Vec<(PathBuf, Option<(SystemTime, u64)>)> = projeto
        .programa()
        .units
        .iter()
        .filter_map(|u| u.path.clone())
        .filter(|p| {
            url::Url::from_file_path(p)
                .ok()
                .is_none_or(|u| documentos.get(u.as_str()).is_none())
        })
        .map(|p| {
            let m = metadados(&p);
            (p, m)
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

/// O disco ainda está como na carga.
fn disco_igual(chave: &Chave) -> bool {
    let raiz = match &chave.escopo {
        Escopo::Projeto(r) => r.clone(),
        Escopo::Biblioteca(a) => crate::projeto::raiz_do_projeto(a),
    };
    chave.disco.iter().all(|(p, m)| metadados(p) == *m)
        && arquivos_do_projeto(&raiz) == chave.arquivos
}
