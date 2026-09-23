//! O grafo de assets: fontes de cada pacote, nós sintéticos e as saídas
//! **esperadas** de cada fase — calcular não é executar.
//!
//! Reproduz `asset_graph/graph.dart` (`_addOutputsForSources`,
//! `_addInBuildPhaseOutputs`, `_actionMatches`, `_addGeneratedOutputs`) e a
//! listagem de fontes do `build_definition.dart` (`_listAssetIds`) com a
//! visibilidade do `target_graph.dart`.
use crate::glob::Glob;
use crate::pacotes::{GrafoPacotes, SDK};
use crate::plano::{Casador, Configs, Fase, NoAlvo};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// `AssetId`: pacote e caminho POSIX relativo ao pacote.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetId {
    pub pacote: Arc<str>,
    pub caminho: Arc<str>,
}

impl AssetId {
    pub fn novo(pacote: &str, caminho: &str) -> AssetId {
        AssetId { pacote: pacote.into(), caminho: caminho.into() }
    }

    /// `pacote|caminho`, como o `build` escreve.
    pub fn texto(&self) -> String {
        format!("{}|{}", self.pacote, self.caminho)
    }

    pub fn de_texto(s: &str) -> Option<AssetId> {
        let (p, c) = s.split_once('|')?;
        Some(AssetId::novo(p, c))
    }
}

/// Os padrões visíveis de um pacote que não é a raiz
/// (`options.dart`, `defaultNonRootVisibleAssets`).
pub const VISIVEIS_FORA_DA_RAIZ: &[&str] = &["CHANGELOG*", "lib/**", "bin/**", "LICENSE*", "pubspec.yaml", "README*"];

/// Fontes padrão do pacote raiz (`options.dart`, `defaultRootPackageSources`).
pub const FONTES_DA_RAIZ: &[&str] = &[
    "assets/**",
    "benchmark/**",
    "bin/**",
    "CHANGELOG*",
    "example/**",
    "lib/**",
    "test/**",
    "integration_test/**",
    "tool/**",
    "web/**",
    "node/**",
    "LICENSE*",
    "pubspec.yaml",
    "pubspec.lock",
    "README*",
    "$package$",
];

/// Nós sintéticos de todo pacote (`graph.dart`, `placeholderIdsFor`).
pub const SINTETICOS: &[&str] = &["lib/$lib$", "test/$test$", "web/$web$", "$package$"];

/// Uma ação: uma fase aplicada a uma entrada primária. Sem saídas = ação
/// removida (a entrada deixou de ser fonte).
#[derive(Debug, Clone)]
pub struct Acao {
    pub fase: usize,
    pub entrada: AssetId,
    pub saidas: Vec<AssetId>,
}

#[derive(Debug, Clone)]
pub struct NoGerado {
    pub acao: usize,
    pub fase: usize,
    pub oculto: bool,
}

#[derive(Debug, Default)]
pub struct Grafo {
    /// Fontes (inclui os sintéticos), por pacote.
    pub fontes: BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
    pub gerados: BTreeMap<AssetId, NoGerado>,
    pub acoes: Vec<Acao>,
    /// Pacotes cujas fontes foram listadas (os que têm alguma fase que gera).
    pub listados: BTreeSet<Arc<str>>,
}

/// Lista, sob `raiz`, os arquivos que algum `globs` casa, podando os
/// diretórios que nenhum pode alcançar.
pub fn listar(raiz: &Path, globs: &[Glob]) -> BTreeSet<String> {
    let mut v = BTreeSet::new();
    let mut pilha: Vec<(PathBuf, String)> = vec![(raiz.to_path_buf(), String::new())];
    while let Some((dir, rel)) = pilha.pop() {
        let Ok(ls) = std::fs::read_dir(&dir) else { continue };
        for e in ls.flatten() {
            let nome = e.file_name().to_string_lossy().to_string();
            let r = if rel.is_empty() { nome.clone() } else { format!("{rel}/{nome}") };
            let Ok(tipo) = e.file_type() else { continue };
            let eh_dir = tipo.is_dir() || (tipo.is_symlink() && e.path().is_dir());
            if eh_dir {
                if globs.iter().any(|g| g.pode_descer(&r)) {
                    pilha.push((e.path(), r));
                }
            } else if globs.iter().any(|g| g.casa(&r)) {
                v.insert(r);
            }
        }
    }
    v
}

fn globs(padroes: &[String]) -> Result<Vec<Glob>, String> {
    padroes.iter().map(|p| Glob::novo(p)).collect()
}

impl Grafo {
    pub fn tem_fonte(&self, id: &AssetId) -> bool {
        self.fontes.get(&id.pacote).is_some_and(|s| s.contains(&id.caminho))
    }

    pub fn existe(&self, id: &AssetId) -> bool {
        self.tem_fonte(id) || self.gerados.contains_key(id)
    }

    /// Fonte original de uma cadeia de saídas.
    pub fn origem<'a>(&'a self, id: &'a AssetId) -> &'a AssetId {
        let mut atual = id;
        while let Some(g) = self.gerados.get(atual) {
            atual = &self.acoes[g.acao].entrada;
        }
        atual
    }

    /// Fontes de um pacote, pelos alvos (`_listAssetIds`).
    fn fontes_do_pacote(grafo: &GrafoPacotes, configs: &Configs, pacote: usize) -> Result<BTreeSet<String>, String> {
        let no = &grafo.nos[pacote];
        let cfg = &configs.por_pacote[pacote];
        let mut v = BTreeSet::new();
        if no.nome == SDK || no.raiz.as_os_str().is_empty() {
            return Ok(v);
        }
        let padrao: Vec<String> = if no.e_raiz {
            FONTES_DA_RAIZ.iter().map(|s| s.to_string()).chain(cfg.publicos_adicionais.iter().cloned()).collect()
        } else {
            VISIVEIS_FORA_DA_RAIZ.iter().map(|s| s.to_string()).chain(cfg.publicos_adicionais.iter().cloned()).collect()
        };
        let visiveis = if no.e_raiz {
            None
        } else {
            let pub_: Vec<String> =
                VISIVEIS_FORA_DA_RAIZ.iter().map(|s| s.to_string()).chain(cfg.publicos_adicionais.iter().cloned()).collect();
            Some(globs(&pub_)?)
        };
        for alvo in &cfg.alvos {
            let casador = Casador::novo(&alvo.sources, Some(&padrao))?;
            let inc = casador.include.clone().unwrap_or_default();
            if inc.is_empty() {
                continue;
            }
            for c in listar(&no.raiz, &inc) {
                if visiveis.as_ref().is_some_and(|g| !g.iter().any(|x| x.casa(&c))) {
                    continue;
                }
                if casador.exclui(&c) {
                    continue;
                }
                v.insert(c);
            }
        }
        Ok(v)
    }

    /// Monta o grafo: fontes dos pacotes com fases que geram, sintéticos de
    /// todos, e as saídas esperadas fase a fase.
    pub fn montar(grafo: &GrafoPacotes, configs: &Configs, fases: &[Fase], _alvos: &[NoAlvo]) -> Result<Grafo, String> {
        let mut g = Grafo::default();
        let pacotes_com_fase: BTreeSet<usize> =
            fases.iter().filter(|f| f.extensoes.is_some()).map(|f| f.pacote).collect();
        for (i, no) in grafo.nos.iter().enumerate() {
            let conj = g.fontes.entry(no.nome.as_str().into()).or_default();
            for s in SINTETICOS {
                conj.insert((*s).into());
            }
            if pacotes_com_fase.contains(&i) {
                for c in Self::fontes_do_pacote(grafo, configs, i)? {
                    conj.insert(c.into());
                }
                g.listados.insert(no.nome.as_str().into());
            }
        }
        // `allInputs`: fontes + sintéticos, e as saídas acumuladas.
        let mut entradas: BTreeMap<Arc<str>, BTreeSet<Arc<str>>> = g.fontes.clone();
        for (fi, fase) in fases.iter().enumerate() {
            let Some(ext) = &fase.extensoes else { continue };
            let pacote: Arc<str> = grafo.nos[fase.pacote].nome.as_str().into();
            let candidatas: Vec<Arc<str>> = entradas
                .get(&pacote)
                .map(|s| {
                    s.iter()
                        .filter(|c| fase.generate_for.casa(c) && ext.tem_saida(c))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            let mut novas: Vec<Arc<str>> = Vec::new();
            for c in candidatas {
                // Pode ter deixado de ser entrada nesta fase (virou saída).
                if !entradas.get(&pacote).is_some_and(|s| s.contains(&c)) {
                    continue;
                }
                let id = AssetId { pacote: pacote.clone(), caminho: c.clone() };
                if !fase.fontes_alvo.casa(&g.origem(&id).caminho) {
                    continue;
                }
                let saidas: Vec<AssetId> = ext
                    .saidas(&c)?
                    .into_iter()
                    .map(|s| AssetId { pacote: pacote.clone(), caminho: s.into() })
                    .collect();
                let ai = g.acoes.len();
                for s in &saidas {
                    if let Some(outro) = g.gerados.get(s) {
                        return Err(format!(
                            "Both {} and {} may output {}, which is not allowed.",
                            fases[outro.fase].fabrica,
                            fase.fabrica,
                            s.texto()
                        ));
                    }
                    // Fonte no lugar de uma saída (saída `source` de um build
                    // anterior): a saída a substitui, e o que tinha sido
                    // esperado dela sai junto (`_removeRecursive`).
                    if g.tem_fonte(s) {
                        g.remover_recursivo(s, &mut entradas, &mut novas);
                    }
                    g.gerados.insert(s.clone(), NoGerado { acao: ai, fase: fi, oculto: fase.oculta });
                    novas.push(s.caminho.clone());
                }
                g.acoes.push(Acao { fase: fi, entrada: id, saidas });
            }
            let e = entradas.entry(pacote).or_default();
            e.extend(novas);
        }
        Ok(g)
    }

    /// `_removeRecursive`: tira o nó e, recursivamente, as saídas das ações
    /// que o tinham como entrada primária.
    fn remover_recursivo(
        &mut self,
        id: &AssetId,
        entradas: &mut BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
        novas: &mut Vec<Arc<str>>,
    ) {
        if let Some(f) = self.fontes.get_mut(&id.pacote) {
            f.remove(&id.caminho);
        }
        if let Some(e) = entradas.get_mut(&id.pacote) {
            e.remove(&id.caminho);
        }
        // Saídas são sempre do pacote da entrada: `novas` é da mesma fase.
        novas.retain(|c| *c != id.caminho);
        self.gerados.remove(id);
        let dependentes: Vec<usize> =
            (0..self.acoes.len()).filter(|&a| self.acoes[a].entrada == *id && !self.acoes[a].saidas.is_empty()).collect();
        for a in dependentes {
            let saidas = std::mem::take(&mut self.acoes[a].saidas);
            for s in saidas {
                self.remover_recursivo(&s, entradas, novas);
            }
        }
    }

    /// `build_impl.dart:443-463`: a saída é legível por uma ação da fase
    /// `leitora`? Fase posterior não; mesma fase só a própria saída.
    pub fn legivel(&self, id: &AssetId, leitora: usize, propria: Option<usize>) -> bool {
        match self.gerados.get(id) {
            None => self.tem_fonte(id),
            Some(g) => g.fase < leitora || (g.fase == leitora && Some(g.acao) == propria),
        }
    }
}
