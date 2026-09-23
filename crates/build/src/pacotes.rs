//! O grafo de pacotes do `build_runner_core` 8.0.0
//! (`package_graph/package_graph.dart`, `PackageGraph.forPath`) e os SCCs do
//! `graphs` 2.3.2, que decidem a ordem dos pacotes e dos alvos.
use crate::config::{ler_lock, Pubspec, TipoDependencia, Travado};
use dartforge_elements::config::PackageConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct No {
    pub nome: String,
    pub raiz: PathBuf,
    pub tipo: TipoDependencia,
    pub e_raiz: bool,
    /// Dependências diretas, na ordem do oficial (nomes ordenados).
    pub deps: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct GrafoPacotes {
    /// `allPackages`: os do `package_config.json` em ordem de nome e o
    /// `$sdk` no fim.
    pub nos: Vec<No>,
    pub raiz: usize,
    pub por_nome: HashMap<String, usize>,
    /// Versões do `pubspec.lock`.
    pub lock: HashMap<String, Travado>,
    /// Diretório do `package_config.json` e do `pubspec.lock`.
    pub dir_raiz: PathBuf,
}

/// Nome do pseudo-pacote do SDK.
pub const SDK: &str = "$sdk";

impl GrafoPacotes {
    pub fn no(&self, nome: &str) -> Option<&No> {
        self.por_nome.get(nome).map(|&i| &self.nos[i])
    }

    /// `PackageGraph.forPath(dir)`: `dir` tem o `pubspec.yaml` da raiz; o
    /// `package_config.json` é o que `cfg` já leu.
    pub fn montar(dir: &Path, cfg: &PackageConfig, sdk: Option<&Path>) -> Result<GrafoPacotes, String> {
        let pubspec = Pubspec::ler(dir)?;
        let nome_raiz = pubspec
            .nome
            .clone()
            .ok_or("The current package has no name, please add one to the pubspec.yaml.")?;
        let dir_raiz = cfg
            .origin
            .as_ref()
            .and_then(|o| o.parent())
            .and_then(|d| d.parent())
            .map(Path::to_path_buf)
            .ok_or("package_config.json sem origem")?;
        let caminho_lock = dir_raiz.join("pubspec.lock");
        let texto_lock = std::fs::read_to_string(&caminho_lock).map_err(|_| {
            "Unable to generate package graph, no `pubspec.lock` found. This program must be ran from the root directory of your package.".to_string()
        })?;
        let lock: HashMap<String, Travado> =
            ler_lock(&texto_lock, &caminho_lock.display().to_string())?.into_iter().collect();

        let mut nomes: Vec<&String> = cfg.packages.keys().collect();
        nomes.sort();
        let mut nos = Vec::with_capacity(nomes.len() + 1);
        let mut por_nome = HashMap::new();
        for nome in nomes {
            let info = &cfg.packages[nome];
            let raiz = info
                .root_uri
                .to_file_path()
                .map(dartforge_elements::config::sem_verbatim)
                .map_err(|_| format!("rootUri inválido para {nome}"))?;
            por_nome.insert(nome.clone(), nos.len());
            nos.push(No {
                nome: nome.clone(),
                raiz,
                tipo: lock.get(nome).map(|t| t.tipo).unwrap_or(TipoDependencia::Path),
                e_raiz: *nome == nome_raiz,
                deps: Vec::new(),
            });
        }
        let raiz = *por_nome.get(&nome_raiz).ok_or_else(|| {
            format!("Dependency {nome_raiz} not present, please run `dart pub get` or `flutter pub get` to fetch dependencies.")
        })?;
        let achar = |n: &str, pai: &str, por_nome: &HashMap<String, usize>| {
            por_nome.get(n).copied().ok_or_else(|| {
                format!("Dependency {n} of {pai} not present, please run `dart pub get` or `flutter pub get` to fetch dependencies.")
            })
        };
        // Raiz: dependencies ∪ dev_dependencies, ordenadas.
        let mut d: Vec<String> = pubspec.dependencias.clone();
        for x in &pubspec.dev_dependencias {
            if !d.contains(x) {
                d.push(x.clone());
            }
        }
        d.sort();
        nos[raiz].deps = d.iter().map(|n| achar(n, &nome_raiz, &por_nome)).collect::<Result<_, _>>()?;
        for i in 0..nos.len() {
            if i == raiz {
                continue;
            }
            let p = Pubspec::ler(&nos[i].raiz)?;
            let mut d = p.dependencias.clone();
            d.sort();
            d.dedup();
            let nome = nos[i].nome.clone();
            nos[i].deps = d.iter().map(|n| achar(n, &nome, &por_nome)).collect::<Result<_, _>>()?;
        }
        por_nome.insert(SDK.to_string(), nos.len());
        nos.push(No {
            nome: SDK.to_string(),
            raiz: sdk.map(Path::to_path_buf).unwrap_or_default(),
            tipo: TipoDependencia::Hosted,
            e_raiz: false,
            deps: Vec::new(),
        });
        Ok(GrafoPacotes { nos, raiz, por_nome, lock, dir_raiz })
    }

    /// Ordem dos pacotes para o script de build: SCCs a partir da raiz,
    /// dependências primeiro (`build_script_generate.dart:86-92`).
    pub fn ordem_do_script(&self) -> Vec<usize> {
        scc(&[self.raiz], |n| self.nos[n].deps.clone()).into_iter().flatten().collect()
    }
}

/// `stronglyConnectedComponents` do `graphs` 2.3.2, na mesma ordem de
/// visita (pilha explícita), para que componentes e empates saiam iguais.
pub fn scc(inicio: &[usize], arestas: impl Fn(usize) -> Vec<usize>) -> Vec<Vec<usize>> {
    let mut resultado = Vec::new();
    let mut baixo: HashMap<usize, usize> = HashMap::new();
    let mut indice: HashMap<usize, usize> = HashMap::new();
    let mut na_pilha = std::collections::HashSet::new();
    let mut proximo = 0usize;
    let mut visitados: Vec<usize> = Vec::new();
    // (nó, iterador: arestas e posição corrente), `None` = ainda não iniciado.
    // O Dart monta `[for (node in nodes) _StackState(node)]` e tira do fim:
    // o último nó é visitado primeiro. Reproduz exatamente isso.
    let mut pilha: Vec<(usize, Option<(Vec<usize>, usize)>)> = inicio.iter().map(|&n| (n, None)).collect();
    'externo: while let Some((no, it)) = pilha.pop() {
        let (mut it, mut low) = match it {
            None => {
                if indice.contains_key(&no) {
                    continue;
                }
                indice.insert(no, proximo);
                baixo.insert(no, proximo);
                let low = proximo;
                proximo += 1;
                let arestas = arestas(no);
                if arestas.is_empty() {
                    resultado.push(vec![no]);
                    continue;
                }
                visitados.push(no);
                na_pilha.insert(no);
                ((arestas, 0usize), low)
            }
            Some((ar, pos)) => {
                let atual = ar[pos];
                let low = baixo[&no].min(baixo[&atual]);
                ((ar, pos), low)
            }
        };
        loop {
            let prox = it.0[it.1];
            if !indice.contains_key(&prox) {
                pilha.push((no, Some((it.0.clone(), it.1))));
                pilha.push((prox, None));
                continue 'externo;
            } else if na_pilha.contains(&prox) {
                low = low.min(indice[&prox]);
                baixo.insert(no, low);
            }
            it.1 += 1;
            if it.1 >= it.0.len() {
                break;
            }
        }
        if low == indice[&no] {
            let mut comp = Vec::new();
            loop {
                let n = visitados.pop().expect("pilha de Tarjan vazia");
                na_pilha.remove(&n);
                comp.push(n);
                if n == no {
                    break;
                }
            }
            resultado.push(comp);
        }
    }
    resultado
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn scc_dependencias_primeiro() {
        // 0 → 1 → 2, 0 → 2
        let g = [vec![1, 2], vec![2], vec![]];
        let r = scc(&[0], |n| g[n].clone());
        assert_eq!(r, vec![vec![2], vec![1], vec![0]]);
    }

    #[test]
    fn scc_ciclo() {
        // 0 → 1 → 2 → 1
        let g = [vec![1], vec![2], vec![1]];
        let r = scc(&[0], |n| g[n].clone());
        assert_eq!(r, vec![vec![2, 1], vec![0]]);
    }

    #[test]
    fn scc_varios_inicios_na_ordem_do_dart() {
        // Sem arestas: o Dart tira do fim da lista inicial.
        let g: [Vec<usize>; 3] = [vec![], vec![], vec![]];
        let r = scc(&[0, 1, 2], |n| g[n].clone());
        assert_eq!(r, vec![vec![2], vec![1], vec![0]]);
    }
}
