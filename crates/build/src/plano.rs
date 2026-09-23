//! O plano: a lista de aplicações de builder que o `build_runner` escreve no
//! `.dart_tool/build/entrypoint/build.dart` (`build_script_generate.dart`) e
//! as fases que ele expande em tempo de execução (`apply_builders.dart`).
use crate::config::{AutoApply, BuildConfig, BuildTo, InputSet};
use crate::extensoes::Extensoes;
use crate::glob::Glob;
use crate::pacotes::{scc, GrafoPacotes, SDK};
use crate::valor::Mapa;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filtro {
    Nenhum,
    DependentesDe(String),
    Todos,
    Raiz,
}

impl Filtro {
    fn texto(&self) -> String {
        match self {
            Filtro::Nenhum => "toNoneByDefault()".into(),
            Filtro::DependentesDe(p) => format!("toDependentsOf({})", serde_json::Value::String(p.clone())),
            Filtro::Todos => "toAllPackages()".into(),
            Filtro::Raiz => "toRoot()".into(),
        }
    }
}

/// Uma aplicação do `build.dart`: `apply(...)` ou `applyPostProcess(...)`.
#[derive(Debug, Clone)]
pub struct Aplicacao {
    pub chave: String,
    pub pos: bool,
    /// Import como o script o escreve (`package:…` ou relativo ao
    /// `.dart_tool/build/entrypoint`).
    pub import: String,
    pub fabricas: Vec<String>,
    pub filtro: Filtro,
    pub opcional: bool,
    pub oculta: bool,
    pub generate_for_padrao: Option<InputSet>,
    pub opcoes_padrao: Mapa,
    pub opcoes_dev: Mapa,
    pub opcoes_release: Mapa,
    pub aplica: Vec<String>,
    /// Pacote que define o builder.
    pub pacote: String,
    /// Extensões do `build.yaml` (as de execução vêm do descritor).
    pub extensoes_declaradas: Vec<(String, Vec<String>)>,
}

impl Aplicacao {
    /// Forma canônica, comparável com a que se lê do `build.dart`.
    pub fn texto_canonico(&self) -> String {
        let l = |v: &[String]| {
            crate::valor::Valor::Lista(v.iter().map(|s| crate::valor::Valor::Texto(s.clone())).collect()).texto_canonico()
        };
        let fabricas: Vec<String> = self.fabricas.iter().map(|f| format!("{}#{f}", self.import)).collect();
        let gf = self.generate_for_padrao.as_ref().map(|g| g.texto_canonico()).unwrap_or_else(|| "-".into());
        if self.pos {
            format!(
                "applyPostProcess {} {} defaultGenerateFor={gf} defaultOptions={} defaultDevOptions={} defaultReleaseOptions={}",
                serde_json::Value::String(self.chave.clone()),
                l(&fabricas),
                self.opcoes_padrao.texto_canonico(),
                self.opcoes_dev.texto_canonico(),
                self.opcoes_release.texto_canonico(),
            )
        } else {
            format!(
                "apply {} {} {} isOptional={} hideOutput={} defaultGenerateFor={gf} defaultOptions={} defaultDevOptions={} defaultReleaseOptions={} appliesBuilders={}",
                serde_json::Value::String(self.chave.clone()),
                l(&fabricas),
                self.filtro.texto(),
                self.opcional,
                self.oculta,
                self.opcoes_padrao.texto_canonico(),
                self.opcoes_dev.texto_canonico(),
                self.opcoes_release.texto_canonico(),
                l(&self.aplica),
            )
        }
    }

    fn aplica_ao_pacote(&self, grafo: &GrafoPacotes, pacote: usize) -> bool {
        let no = &grafo.nos[pacote];
        match &self.filtro {
            Filtro::Nenhum => false,
            Filtro::Todos => true,
            Filtro::Raiz => no.e_raiz,
            Filtro::DependentesDe(p) => no.deps.iter().any(|&d| grafo.nos[d].nome == *p),
        }
    }
}

/// `_buildScriptImport` (`build_script_generate.dart:252-263`).
fn import_do_script(import: &str) -> String {
    if import.starts_with("package:") || import.starts_with("../") || import.starts_with('/') {
        return import.to_string();
    }
    // `p.url.relative(import, from: '.dart_tool/build/entrypoint')`.
    let mut partes: Vec<&str> = Vec::new();
    for p in import.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                partes.pop();
            }
            x => partes.push(x),
        }
    }
    let alvo = partes.join("/");
    if let Some(dentro) = alvo.strip_prefix(".dart_tool/build/entrypoint/") {
        return dentro.to_string();
    }
    if alvo.starts_with(".dart_tool/build/") {
        return format!("../{}", &alvo[".dart_tool/build/".len()..]);
    }
    if alvo.starts_with(".dart_tool/") {
        return format!("../../{}", &alvo[".dart_tool/".len()..]);
    }
    format!("../../../{alvo}")
}

/// Configuração de cada pacote, como o `build_runner` a obtém.
pub struct Configs {
    pub por_pacote: Vec<BuildConfig>,
    pub avisos: Vec<String>,
}

/// `<pacote>.build.yaml` na raiz (`build_config_overrides.dart`).
fn overrides(grafo: &GrafoPacotes) -> Result<HashMap<String, BuildConfig>, String> {
    let raiz = &grafo.nos[grafo.raiz];
    let mut m = HashMap::new();
    let Ok(ls) = std::fs::read_dir(&raiz.raiz) else { return Ok(m) };
    let glob = Glob::novo("*.build.yaml")?;
    let mut nomes: Vec<String> =
        ls.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    nomes.sort();
    for nome in nomes {
        if !glob.casa(&nome) {
            continue;
        }
        let pacote = nome.split('.').next().unwrap_or_default().to_string();
        let Some(no) = grafo.no(&pacote) else { continue };
        let deps: Vec<String> = no.deps.iter().map(|&d| grafo.nos[d].nome.clone()).collect();
        let caminho = raiz.raiz.join(&nome);
        let texto = std::fs::read_to_string(&caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
        m.insert(pacote.clone(), BuildConfig::de_texto(&pacote, &deps, &texto, &caminho.display().to_string())?);
    }
    Ok(m)
}

impl Configs {
    /// `estrito`: erro de `build.yaml` é fatal (o `TargetGraph` em tempo de
    /// execução); senão vira o padrão com aviso (a geração do script).
    pub fn ler(grafo: &GrafoPacotes, estrito: bool) -> Result<Configs, String> {
        let ov = overrides(grafo)?;
        let mut por_pacote = Vec::with_capacity(grafo.nos.len());
        let mut avisos = Vec::new();
        for no in &grafo.nos {
            let deps: Vec<String> = no.deps.iter().map(|&d| grafo.nos[d].nome.clone()).collect();
            if let Some(c) = ov.get(&no.nome) {
                por_pacote.push(c.clone());
                continue;
            }
            if no.nome == SDK {
                por_pacote.push(BuildConfig::padrao(SDK, &deps));
                continue;
            }
            match BuildConfig::do_diretorio(&no.nome, &deps, &no.raiz) {
                Ok(c) => por_pacote.push(c),
                Err(e) if estrito => return Err(format!("Failed to parse `build.yaml` for {}: {e}", no.nome)),
                Err(e) => {
                    avisos.push(e);
                    por_pacote.push(BuildConfig::padrao(&no.nome, &deps));
                }
            }
        }
        Ok(Configs { por_pacote, avisos })
    }
}

/// O conteúdo do `build.dart`: as aplicações na ordem final.
#[derive(Debug, Clone)]
pub struct Plano {
    pub aplicacoes: Vec<Aplicacao>,
    pub avisos: Vec<String>,
}

impl Plano {
    pub fn texto_canonico(&self) -> String {
        self.aplicacoes.iter().map(|a| a.texto_canonico() + "\n").collect()
    }

    pub fn aplicacao(&self, chave: &str) -> Option<&Aplicacao> {
        self.aplicacoes.iter().find(|a| a.chave == chave)
    }

    /// `findBuildScriptOptions` (`build_script_generate.dart:72-160`).
    pub fn do_script(grafo: &GrafoPacotes, configs: &Configs) -> Result<Plano, String> {
        let ordem = grafo.ordem_do_script();
        let raiz = &grafo.nos[grafo.raiz];
        let valido = |import: &str, pacote: &str| -> bool {
            if let Some(resto) = import.strip_prefix("package:") {
                let p = resto.split('/').next().unwrap_or_default();
                return grafo.no(p).is_some();
            }
            pacote == raiz.nome
        };
        let mut avisos = configs.avisos.clone();
        let mut defs = Vec::new();
        let mut pos = Vec::new();
        for &i in &ordem {
            let c = &configs.por_pacote[i];
            for b in &c.builders {
                if valido(&b.import, &b.pacote) {
                    defs.push(b.clone());
                } else {
                    avisos.push(format!("Could not load imported package for definition \"{}\".", b.chave));
                }
            }
            for p in &c.pos {
                if valido(&p.import, &p.pacote) {
                    pos.push(p.clone());
                }
            }
        }
        let cfg_raiz = &configs.por_pacote[*ordem.last().ok_or("grafo de pacotes vazio")?];
        // `findBuilderOrder` (`builder_ordering.dart`).
        defs.sort_by(|a, b| a.chave.cmp(&b.chave));
        let n = defs.len();
        let saidas: Vec<Vec<&String>> = defs.iter().map(|d| d.extensoes.iter().flat_map(|(_, s)| s).collect()).collect();
        let deve_antes = |filho: usize, pai: usize| -> bool {
            defs[filho].runs_before.contains(&defs[pai].chave)
                || cfg_raiz.global_de(&defs[filho].chave).is_some_and(|g| g.runs_before.contains(&defs[pai].chave))
        };
        let arestas: Vec<Vec<usize>> = (0..n)
            .map(|pai| {
                (0..n)
                    .filter(|&filho| {
                        pai != filho
                            && (defs[pai].required_inputs.iter().any(|ri| saidas[filho].iter().any(|s| s.ends_with(ri.as_str())))
                                || deve_antes(filho, pai))
                    })
                    .collect()
            })
            .collect();
        // Kahn com fila de prioridade pela chave (`_topologicalSortWithSecondary`).
        let mut entrada = vec![0usize; n];
        for a in &arestas {
            for &f in a {
                entrada[f] += 1;
            }
        }
        let mut prontos: std::collections::BTreeSet<(String, usize)> =
            (0..n).filter(|&i| entrada[i] == 0).map(|i| (defs[i].chave.clone(), i)).collect();
        let mut kahn = Vec::with_capacity(n);
        while let Some(primeiro) = prontos.iter().next().cloned() {
            prontos.remove(&primeiro);
            kahn.push(primeiro.1);
            for &f in &arestas[primeiro.1] {
                entrada[f] -= 1;
                if entrada[f] == 0 {
                    prontos.insert((defs[f].chave.clone(), f));
                }
            }
        }
        if kahn.len() < n {
            let ciclo: Vec<&str> = (0..n).filter(|&i| entrada[i] > 0).map(|i| defs[i].chave.as_str()).collect();
            return Err(format!("Required input cycle for [{}]", ciclo.join(", ")));
        }
        kahn.reverse();
        for (chave, _) in &cfg_raiz.global {
            let conhecida = |k: &str| defs.iter().any(|d| d.chave == k);
            let g = cfg_raiz.global_de(chave).expect("chave global");
            for k in std::iter::once(chave).chain(&g.runs_before) {
                if !conhecida(k) {
                    avisos.push(format!(
                        "Invalid builder key `{k}` found in global_options config of build.yaml. This configuration will have no effect."
                    ));
                }
            }
        }
        let mut aplicacoes = Vec::with_capacity(n + pos.len());
        for i in kahn {
            let d = &defs[i];
            aplicacoes.push(Aplicacao {
                chave: d.chave.clone(),
                pos: false,
                import: import_do_script(&d.import),
                fabricas: d.fabricas.clone(),
                filtro: match d.auto_apply {
                    AutoApply::Nenhum => Filtro::Nenhum,
                    AutoApply::Dependentes => Filtro::DependentesDe(d.pacote.clone()),
                    AutoApply::Todos => Filtro::Todos,
                    AutoApply::Raiz => Filtro::Raiz,
                },
                opcional: d.opcional,
                oculta: d.build_to == BuildTo::Cache,
                generate_for_padrao: d.padroes.generate_for.clone(),
                opcoes_padrao: d.padroes.options.clone(),
                opcoes_dev: d.padroes.dev_options.clone(),
                opcoes_release: d.padroes.release_options.clone(),
                aplica: d.applies_builders.clone(),
                pacote: d.pacote.clone(),
                extensoes_declaradas: d.extensoes.clone(),
            });
        }
        for p in pos {
            aplicacoes.push(Aplicacao {
                chave: p.chave.clone(),
                pos: true,
                import: import_do_script(&p.import),
                fabricas: vec![p.fabrica.clone()],
                filtro: Filtro::Nenhum,
                opcional: false,
                oculta: true,
                generate_for_padrao: p.padroes.generate_for.clone(),
                opcoes_padrao: p.padroes.options.clone(),
                opcoes_dev: p.padroes.dev_options.clone(),
                opcoes_release: p.padroes.release_options.clone(),
                aplica: Vec::new(),
                pacote: p.pacote.clone(),
                extensoes_declaradas: Vec::new(),
            });
        }
        Ok(Plano { aplicacoes, avisos })
    }
}

/// `InputMatcher`: `include` vazio ou ausente = tudo.
#[derive(Debug, Clone)]
pub struct Casador {
    pub include: Option<Vec<Glob>>,
    pub exclude: Option<Vec<Glob>>,
}

impl Casador {
    pub fn novo(s: &InputSet, padrao_include: Option<&[String]>) -> Result<Casador, String> {
        let inc = s.include.as_deref().or(padrao_include);
        Ok(Casador {
            include: inc.map(|v| v.iter().map(|g| Glob::novo(g)).collect::<Result<_, _>>()).transpose()?,
            exclude: s.exclude.as_ref().map(|v| v.iter().map(|g| Glob::novo(g)).collect::<Result<_, _>>()).transpose()?,
        })
    }

    pub fn inclui(&self, caminho: &str) -> bool {
        match &self.include {
            None => true,
            Some(v) => v.is_empty() || v.iter().any(|g| g.casa(caminho)),
        }
    }

    pub fn exclui(&self, caminho: &str) -> bool {
        self.exclude.as_ref().is_some_and(|v| !v.is_empty() && v.iter().any(|g| g.casa(caminho)))
    }

    pub fn casa(&self, caminho: &str) -> bool {
        self.inclui(caminho) && !self.exclui(caminho)
    }

    pub fn texto(&self) -> String {
        let l = |v: &Option<Vec<Glob>>| match v {
            None => "-".to_string(),
            Some(v) => v.iter().map(|g| g.padrao.as_str()).collect::<Vec<_>>().join(","),
        };
        format!("[{}]-[{}]", l(&self.include), l(&self.exclude))
    }
}

/// Uma fase (`InBuildPhase`, ou uma ação do `PostBuildPhase` final).
#[derive(Debug, Clone)]
pub struct Fase {
    pub aplicacao: usize,
    pub fabrica: String,
    pub pacote: usize,
    pub alvo: String,
    pub fontes_alvo: Casador,
    pub generate_for: Casador,
    pub opcoes: Mapa,
    pub raiz: bool,
    pub opcional: bool,
    pub oculta: bool,
    pub pos: bool,
    /// `None` para pós-processadores e builders substituídos.
    pub extensoes: Option<Extensoes>,
    pub substituido: Option<&'static str>,
}

impl Fase {
    /// `InBuildPhase.identity`: o que muda a fase sem ser opção.
    pub fn identidade(&self, plano: &Plano, grafo: &GrafoPacotes) -> String {
        format!(
            "{}#{} {} {} gen={} fontes={} opcional={} oculta={} pos={} ext={:?}",
            plano.aplicacoes[self.aplicacao].chave,
            self.fabrica,
            grafo.nos[self.pacote].nome,
            self.alvo,
            self.generate_for.texto(),
            self.fontes_alvo.texto(),
            self.opcional,
            self.oculta,
            self.pos,
            self.extensoes.as_ref().map(|e| &e.declaradas),
        )
    }
}

/// Um alvo do grafo de alvos (`TargetNode`).
#[derive(Debug, Clone)]
pub struct NoAlvo {
    pub pacote: usize,
    pub config: usize,
    pub alvo: usize,
}

/// As fases, como `createBuildPhases` (`apply_builders.dart:196-349`).
pub fn fases(
    grafo: &GrafoPacotes,
    configs: &Configs,
    plano: &Plano,
    release: bool,
) -> Result<(Vec<Fase>, Vec<NoAlvo>), String> {
    // `allModules`: por pacote (ordem de `allPackages`), alvos na ordem.
    let mut nos: Vec<NoAlvo> = Vec::new();
    let mut por_chave: HashMap<&str, usize> = HashMap::new();
    for (p, c) in configs.por_pacote.iter().enumerate() {
        for (a, alvo) in c.alvos.iter().enumerate() {
            por_chave.insert(alvo.chave.as_str(), nos.len());
            nos.push(NoAlvo { pacote: p, config: p, alvo: a });
        }
    }
    let alvo = |n: &NoAlvo| &configs.por_pacote[n.config].alvos[n.alvo];
    let mut deps_alvo: Vec<Vec<usize>> = Vec::with_capacity(nos.len());
    for n in &nos {
        let a = alvo(n);
        let mut v = Vec::new();
        for d in &a.dependencias {
            let Some(&i) = por_chave.get(d.as_str()) else {
                return Err(format!("{} declares a dependency on {d} but it does not exist", a.chave));
            };
            v.push(i);
        }
        deps_alvo.push(v);
    }
    let inicio: Vec<usize> = (0..nos.len()).collect();
    let ciclos = scc(&inicio, |i| deps_alvo[i].clone());

    let cfg_raiz = &configs.por_pacote[grafo.raiz];
    let global: HashMap<&str, Mapa> = cfg_raiz
        .global
        .iter()
        .map(|(k, g)| (k.as_str(), g.options.sobrepor(if release { &g.release_options } else { &g.dev_options })))
        .collect();
    let por_aplicacao: HashMap<&str, usize> =
        plano.aplicacoes.iter().enumerate().map(|(i, a)| (a.chave.as_str(), i)).collect();
    let mut aplica_com: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, a) in plano.aplicacoes.iter().enumerate() {
        for b in &a.aplica {
            aplica_com.entry(b.as_str()).or_default().push(i);
        }
    }

    // `_shouldApply` com memória (a recursão por `applies_builders` repete).
    fn deve_aplicar(
        a: usize,
        n: usize,
        grafo: &GrafoPacotes,
        plano: &Plano,
        nos: &[NoAlvo],
        configs: &Configs,
        por_aplicacao: &HashMap<&str, usize>,
        aplica_com: &HashMap<&str, Vec<usize>>,
        profundidade: usize,
    ) -> bool {
        let ap = &plano.aplicacoes[a];
        let no = &nos[n];
        let pacote = &grafo.nos[no.pacote];
        let todas_ocultas = ap.aplica.iter().all(|b| por_aplicacao.get(b.as_str()).is_none_or(|&i| plano.aplicacoes[i].oculta));
        if !(ap.oculta && todas_ocultas) && !pacote.e_raiz {
            return false;
        }
        let alvo = &configs.por_pacote[no.config].alvos[no.alvo];
        if let Some(c) = alvo.cfg(&ap.chave) {
            return c.habilitado;
        }
        if alvo.auto_apply_builders && ap.aplica_ao_pacote(grafo, no.pacote) {
            return true;
        }
        if profundidade > 64 {
            return false;
        }
        aplica_com.get(ap.chave.as_str()).is_some_and(|ancoras| {
            ancoras.iter().any(|&anc| {
                deve_aplicar(anc, n, grafo, plano, nos, configs, por_aplicacao, aplica_com, profundidade + 1)
            })
        })
    }

    let mut em_build = Vec::new();
    let mut pos = Vec::new();
    for ciclo in &ciclos {
        for (ia, ap) in plano.aplicacoes.iter().enumerate() {
            for fab in &ap.fabricas {
                for &n in ciclo {
                    if !deve_aplicar(ia, n, grafo, plano, &nos, configs, &por_aplicacao, &aplica_com, 0) {
                        continue;
                    }
                    let no = &nos[n];
                    let a = alvo(no);
                    let cfg = a.cfg(&ap.chave);
                    let vazio = Mapa::default();
                    let opcoes_alvo = cfg
                        .map(|c| c.options.sobrepor(if release { &c.release_options } else { &c.dev_options }))
                        .unwrap_or_default()
                        .sobrepor(global.get(ap.chave.as_str()).unwrap_or(&vazio));
                    let opcoes = ap
                        .opcoes_padrao
                        .sobrepor(if release { &ap.opcoes_release } else { &ap.opcoes_dev })
                        .sobrepor(&opcoes_alvo);
                    let generate_for =
                        cfg.and_then(|c| c.generate_for.clone()).or_else(|| ap.generate_for_padrao.clone()).unwrap_or_default();
                    let substituido = crate::descritor::substituido(&ap.chave);
                    let extensoes = if ap.pos || substituido.is_some() {
                        None
                    } else {
                        let decl =
                            crate::descritor::extensoes_de_execucao(&ap.chave, fab, &ap.fabricas, &opcoes, &ap.extensoes_declaradas)?;
                        Some(Extensoes::novas(&decl, &ap.chave)?)
                    };
                    let f = Fase {
                        aplicacao: ia,
                        fabrica: fab.clone(),
                        pacote: no.pacote,
                        alvo: a.chave.clone(),
                        fontes_alvo: Casador::novo(&a.sources, None)?,
                        generate_for: Casador::novo(&generate_for, None)?,
                        opcoes,
                        raiz: grafo.nos[no.pacote].e_raiz,
                        opcional: ap.opcional,
                        oculta: ap.oculta,
                        pos: ap.pos,
                        extensoes,
                        substituido,
                    };
                    if ap.pos {
                        pos.push(f);
                    } else {
                        em_build.push(f);
                    }
                }
            }
        }
    }
    em_build.extend(pos);
    Ok((em_build, nos))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn import_relativo_do_script() {
        assert_eq!(import_do_script("package:a/b.dart"), "package:a/b.dart");
        assert_eq!(import_do_script("tool/builders.dart"), "../../../tool/builders.dart");
        assert_eq!(import_do_script("./tool/builders.dart"), "../../../tool/builders.dart");
    }
}
