//! `pubspec.yaml`, `pubspec.lock` e `build.yaml`, lidos como o
//! `build_config` 1.1.2 e o `build_runner_core` 8.0.0 leem.
//!
//! O `build.yaml` segue o `$checkKeys` dos `*.g.dart` do `build_config`:
//! chave desconhecida é erro, campo obrigatório ausente é erro, tipo errado é
//! erro. As chaves de builder e de alvo são normalizadas como
//! `key_normalization.dart`.
use crate::valor::{Mapa, Valor};
use std::path::Path;
use yaml_rust2::{Yaml, YamlLoader};

/// Carrega um documento YAML (o primeiro; vazio = `Null`).
pub fn carregar_yaml(texto: &str, origem: &str) -> Result<Yaml, String> {
    let docs = YamlLoader::load_from_str(texto).map_err(|e| format!("{origem}: YAML inválido: {e}"))?;
    Ok(docs.into_iter().next().unwrap_or(Yaml::Null))
}

fn chaves_texto(y: &Yaml) -> Vec<String> {
    match y {
        Yaml::Hash(h) => h.keys().filter_map(|k| k.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------- pubspec

#[derive(Debug, Clone, Default)]
pub struct Pubspec {
    pub nome: Option<String>,
    /// Chaves de `dependencies`, na ordem do arquivo.
    pub dependencias: Vec<String>,
    pub dev_dependencias: Vec<String>,
}

impl Pubspec {
    pub fn ler(dir: &Path) -> Result<Pubspec, String> {
        let caminho = dir.join("pubspec.yaml");
        let texto = std::fs::read_to_string(&caminho)
            .map_err(|_| format!("Unable to generate package graph, no `{}` found.", caminho.display()))?;
        Self::de_texto(&texto, &caminho.display().to_string())
    }

    pub fn de_texto(texto: &str, origem: &str) -> Result<Pubspec, String> {
        let y = carregar_yaml(texto, origem)?;
        Ok(Pubspec {
            nome: y["name"].as_str().map(str::to_string),
            dependencias: chaves_texto(&y["dependencies"]),
            dev_dependencias: chaves_texto(&y["dev_dependencies"]),
        })
    }
}

/// Tipo de dependência (`package_graph.dart`, `DependencyType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoDependencia {
    Github,
    Path,
    Hosted,
}

/// Uma linha do `pubspec.lock`.
#[derive(Debug, Clone)]
pub struct Travado {
    pub tipo: TipoDependencia,
    pub versao: String,
}

pub fn ler_lock(texto: &str, origem: &str) -> Result<Vec<(String, Travado)>, String> {
    let y = carregar_yaml(texto, origem)?;
    let Yaml::Hash(pacotes) = &y["packages"] else {
        return Err(format!("{origem}: sem `packages`"));
    };
    let mut v = Vec::new();
    for (k, p) in pacotes {
        let Some(nome) = k.as_str() else { continue };
        let tipo = match p["source"].as_str() {
            Some("git") => TipoDependencia::Github,
            Some("hosted") => TipoDependencia::Hosted,
            Some("path") | Some("sdk") => TipoDependencia::Path,
            outro => return Err(format!("Unable to determine dependency type:\n{outro:?}")),
        };
        let versao = p["version"].as_str().unwrap_or_default().to_string();
        v.push((nome.to_string(), Travado { tipo, versao }));
    }
    Ok(v)
}

// ------------------------------------------------------ normalização de chaves

pub fn chave_builder_definicao(chave: &str, pacote: &str) -> String {
    normalizar_definicao(&chave.replacen('|', ":", 1), pacote)
}

pub fn chave_builder_uso(chave: &str, pacote: &str) -> String {
    normalizar_uso(&chave.replacen('|', ":", 1), pacote)
}

pub fn chave_alvo_definicao(chave: &str, pacote: &str) -> String {
    if chave == "$default" { format!("{pacote}:{pacote}") } else { normalizar_definicao(chave, pacote) }
}

pub fn chave_alvo_uso(chave: &str, pacote: &str) -> String {
    match chave {
        "$default" | ":$default" | "$default:$default" => format!("{pacote}:{pacote}"),
        _ => normalizar_uso(chave, pacote),
    }
}

fn normalizar_uso(nome: &str, pacote: &str) -> String {
    if nome.starts_with(':') {
        return format!("{pacote}{nome}");
    }
    if !nome.contains(':') {
        return format!("{nome}:{nome}");
    }
    nome.to_string()
}

fn normalizar_definicao(nome: &str, pacote: &str) -> String {
    if nome.starts_with(':') {
        return format!("{pacote}{nome}");
    }
    if !nome.contains(':') {
        return format!("{pacote}:{nome}");
    }
    nome.to_string()
}

// ------------------------------------------------------------- build.yaml

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoApply {
    Nenhum,
    Dependentes,
    Todos,
    Raiz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTo {
    Source,
    Cache,
}

/// `InputSet`: `None` = qualquer caminho.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InputSet {
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

impl InputSet {
    pub fn texto_canonico(&self) -> String {
        let l = |v: &Option<Vec<String>>| match v {
            None => "-".to_string(),
            Some(v) => Valor::Lista(v.iter().map(|s| Valor::Texto(s.clone())).collect()).texto_canonico(),
        };
        format!("InputSet(include: {}, exclude: {})", l(&self.include), l(&self.exclude))
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Padroes {
    /// `None` = `InputSet.anything` (idêntico): não aparece no `build.dart`.
    pub generate_for: Option<InputSet>,
    pub options: Mapa,
    pub dev_options: Mapa,
    pub release_options: Mapa,
}

#[derive(Debug, Clone)]
pub struct DefBuilder {
    pub chave: String,
    pub pacote: String,
    pub fabricas: Vec<String>,
    pub import: String,
    pub extensoes: Vec<(String, Vec<String>)>,
    pub auto_apply: AutoApply,
    pub required_inputs: Vec<String>,
    pub runs_before: Vec<String>,
    pub applies_builders: Vec<String>,
    pub opcional: bool,
    pub build_to: BuildTo,
    pub padroes: Padroes,
}

#[derive(Debug, Clone)]
pub struct DefPos {
    pub chave: String,
    pub pacote: String,
    pub fabrica: String,
    pub import: String,
    pub padroes: Padroes,
}

#[derive(Debug, Clone, Default)]
pub struct CfgBuilderAlvo {
    pub habilitado: bool,
    pub generate_for: Option<InputSet>,
    pub options: Mapa,
    pub dev_options: Mapa,
    pub release_options: Mapa,
}

#[derive(Debug, Clone)]
pub struct Alvo {
    pub chave: String,
    pub pacote: String,
    pub auto_apply_builders: bool,
    pub builders: Vec<(String, CfgBuilderAlvo)>,
    pub dependencias: Vec<String>,
    pub sources: InputSet,
}

impl Alvo {
    pub fn cfg(&self, chave: &str) -> Option<&CfgBuilderAlvo> {
        self.builders.iter().find(|(k, _)| k == chave).map(|(_, c)| c)
    }
}

#[derive(Debug, Clone, Default)]
pub struct CfgGlobal {
    pub options: Mapa,
    pub dev_options: Mapa,
    pub release_options: Mapa,
    pub runs_before: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub pacote: String,
    pub builders: Vec<DefBuilder>,
    pub pos: Vec<DefPos>,
    pub alvos: Vec<Alvo>,
    pub global: Vec<(String, CfgGlobal)>,
    pub publicos_adicionais: Vec<String>,
}

impl BuildConfig {
    /// `BuildConfig.useDefault`.
    pub fn padrao(pacote: &str, dependencias: &[String]) -> BuildConfig {
        BuildConfig {
            pacote: pacote.to_string(),
            builders: Vec::new(),
            pos: Vec::new(),
            alvos: vec![Alvo {
                chave: format!("{pacote}:{pacote}"),
                pacote: pacote.to_string(),
                auto_apply_builders: true,
                builders: Vec::new(),
                dependencias: dependencias.iter().map(|d| chave_alvo_uso(d, pacote)).collect(),
                sources: InputSet::default(),
            }],
            global: Vec::new(),
            publicos_adicionais: Vec::new(),
        }
    }

    pub fn global_de(&self, chave: &str) -> Option<&CfgGlobal> {
        self.global.iter().find(|(k, _)| k == chave).map(|(_, c)| c)
    }

    /// `BuildConfig.fromBuildConfigDir`: `build.yaml` do diretório, ou o padrão.
    pub fn do_diretorio(pacote: &str, dependencias: &[String], dir: &Path) -> Result<BuildConfig, String> {
        let caminho = dir.join("build.yaml");
        match std::fs::read_to_string(&caminho) {
            Ok(texto) => Self::de_texto(pacote, dependencias, &texto, &caminho.display().to_string()),
            Err(_) => Ok(Self::padrao(pacote, dependencias)),
        }
    }

    /// `BuildConfig.parse`.
    pub fn de_texto(pacote: &str, dependencias: &[String], texto: &str, origem: &str) -> Result<BuildConfig, String> {
        let y = carregar_yaml(texto, origem)?;
        let ctx = Ctx { pacote, deps: dependencias, origem };
        ctx.config(&y)
    }
}

struct Ctx<'a> {
    pacote: &'a str,
    deps: &'a [String],
    origem: &'a str,
}

type R<T> = Result<T, String>;

impl Ctx<'_> {
    fn erro(&self, onde: &str, msg: &str) -> String {
        format!("{}: {onde}: {msg}", self.origem)
    }

    fn mapa<'y>(&self, y: &'y Yaml, onde: &str) -> R<Option<&'y yaml_rust2::yaml::Hash>> {
        match y {
            Yaml::Null | Yaml::BadValue => Ok(None),
            Yaml::Hash(h) => Ok(Some(h)),
            _ => Err(self.erro(onde, "esperava um mapa")),
        }
    }

    fn checar(&self, y: &Yaml, onde: &str, permitidas: &[&str], obrigatorias: &[&str]) -> R<()> {
        let Some(h) = self.mapa(y, onde)? else { return Ok(()) };
        let mut desconhecidas = Vec::new();
        for k in h.keys() {
            let k = k.as_str().ok_or_else(|| self.erro(onde, "chave não é texto"))?;
            if !permitidas.contains(&k) {
                desconhecidas.push(k.to_string());
            }
        }
        if !desconhecidas.is_empty() {
            return Err(self.erro(
                onde,
                &format!("Unrecognized keys: [{}]; supported keys: [{}]", desconhecidas.join(", "), permitidas.join(", ")),
            ));
        }
        for o in obrigatorias {
            match &y[*o] {
                Yaml::BadValue => return Err(self.erro(onde, &format!("Required keys are missing: {o}."))),
                Yaml::Null => return Err(self.erro(onde, &format!("These keys had `null` values, which is not allowed: [{o}]"))),
                _ => {}
            }
        }
        Ok(())
    }

    fn texto(&self, y: &Yaml, onde: &str) -> R<Option<String>> {
        match y {
            Yaml::Null | Yaml::BadValue => Ok(None),
            Yaml::String(s) => Ok(Some(s.clone())),
            _ => Err(self.erro(onde, "esperava texto")),
        }
    }

    fn booleano(&self, y: &Yaml, onde: &str) -> R<Option<bool>> {
        match y {
            Yaml::Null | Yaml::BadValue => Ok(None),
            Yaml::Boolean(b) => Ok(Some(*b)),
            _ => Err(self.erro(onde, "esperava bool")),
        }
    }

    fn lista_textos(&self, y: &Yaml, onde: &str) -> R<Option<Vec<String>>> {
        match y {
            Yaml::Null | Yaml::BadValue => Ok(None),
            Yaml::Array(a) => a
                .iter()
                .map(|e| e.as_str().map(str::to_string).ok_or_else(|| self.erro(onde, "esperava lista de textos")))
                .collect::<R<Vec<_>>>()
                .map(Some),
            _ => Err(self.erro(onde, "esperava lista")),
        }
    }

    fn opcoes(&self, y: &Yaml, onde: &str) -> R<Mapa> {
        match self.mapa(y, onde)? {
            None => Ok(Mapa::default()),
            Some(h) => {
                let mut m = Mapa::default();
                for (k, v) in h {
                    let k = k.as_str().ok_or_else(|| self.erro(onde, "chave de opção não é texto"))?;
                    m.inserir(Valor::Texto(k.to_string()), Valor::de_yaml(v).map_err(|e| self.erro(onde, &e))?);
                }
                Ok(m)
            }
        }
    }

    /// `InputSet.fromJson`: lista = `include`; mapa com `include`/`exclude`.
    fn input_set(&self, y: &Yaml, onde: &str) -> R<Option<InputSet>> {
        let s = match y {
            Yaml::Null | Yaml::BadValue => return Ok(None),
            Yaml::Array(a) => {
                if a.iter().any(|e| matches!(e, Yaml::Null)) {
                    return Err(self.erro(onde, "Include globs must not be empty"));
                }
                InputSet { include: self.lista_textos(y, onde)?, exclude: None }
            }
            Yaml::Hash(_) => {
                self.checar(y, onde, &["include", "exclude"], &[])?;
                InputSet {
                    include: self.lista_textos(&y["include"], onde)?,
                    exclude: self.lista_textos(&y["exclude"], onde)?,
                }
            }
            _ => return Err(self.erro(onde, "Expected a Map or a List")),
        };
        if s.include.as_ref().is_some_and(|v| v.iter().any(String::is_empty)) {
            return Err(self.erro(onde, "Include globs must not be empty"));
        }
        if s.exclude.as_ref().is_some_and(|v| v.iter().any(String::is_empty)) {
            return Err(self.erro(onde, "Exclude globs must not be empty"));
        }
        Ok(Some(s))
    }

    fn padroes(&self, y: &Yaml, onde: &str) -> R<Padroes> {
        self.checar(y, onde, &["generate_for", "options", "dev_options", "release_options"], &[])?;
        Ok(Padroes {
            generate_for: self.input_set(&y["generate_for"], onde)?,
            options: self.opcoes(&y["options"], onde)?,
            dev_options: self.opcoes(&y["dev_options"], onde)?,
            release_options: self.opcoes(&y["release_options"], onde)?,
        })
    }

    fn builder(&self, chave: &str, y: &Yaml) -> R<DefBuilder> {
        let onde = &format!("builders.{chave}");
        self.checar(
            y,
            onde,
            &[
                "builder_factories",
                "import",
                "build_extensions",
                "target",
                "auto_apply",
                "required_inputs",
                "runs_before",
                "applies_builders",
                "is_optional",
                "build_to",
                "defaults",
            ],
            &["builder_factories", "import", "build_extensions"],
        )?;
        let fabricas = self.lista_textos(&y["builder_factories"], onde)?.unwrap_or_default();
        if fabricas.is_empty() {
            return Err(self.erro(onde, "builderFactories: Must have at least one value."));
        }
        let mut extensoes = Vec::new();
        let Some(h) = self.mapa(&y["build_extensions"], onde)? else {
            return Err(self.erro(onde, "build_extensions ausente"));
        };
        for (k, v) in h {
            let k = k.as_str().ok_or_else(|| self.erro(onde, "extensão não é texto"))?.to_string();
            let saidas = self.lista_textos(v, onde)?.ok_or_else(|| self.erro(onde, "extensões de saída ausentes"))?;
            if saidas.contains(&k) {
                return Err(self.erro(
                    onde,
                    "May not overwrite an input, the output extensions must not contain the input extension",
                ));
            }
            extensoes.push((k, saidas));
        }
        let auto_apply = match self.texto(&y["auto_apply"], onde)?.as_deref() {
            None | Some("none") => AutoApply::Nenhum,
            Some("dependents") => AutoApply::Dependentes,
            Some("all_packages") => AutoApply::Todos,
            Some("root_package") => AutoApply::Raiz,
            Some(o) => return Err(self.erro(onde, &format!("auto_apply desconhecido: {o}"))),
        };
        let build_to = match self.texto(&y["build_to"], onde)?.as_deref() {
            None | Some("cache") => BuildTo::Cache,
            Some("source") => BuildTo::Source,
            Some(o) => return Err(self.erro(onde, &format!("build_to desconhecido: {o}"))),
        };
        let uso = |v: Option<Vec<String>>| {
            v.unwrap_or_default().iter().map(|b| chave_builder_uso(b, self.pacote)).collect::<Vec<_>>()
        };
        Ok(DefBuilder {
            chave: chave_builder_definicao(chave, self.pacote),
            pacote: self.pacote.to_string(),
            fabricas,
            import: self.texto(&y["import"], onde)?.unwrap_or_default(),
            extensoes,
            auto_apply,
            required_inputs: self.lista_textos(&y["required_inputs"], onde)?.unwrap_or_default(),
            runs_before: uso(self.lista_textos(&y["runs_before"], onde)?),
            applies_builders: uso(self.lista_textos(&y["applies_builders"], onde)?),
            opcional: self.booleano(&y["is_optional"], onde)?.unwrap_or(false),
            build_to,
            padroes: match &y["defaults"] {
                Yaml::Null | Yaml::BadValue => Padroes::default(),
                d => self.padroes(d, onde)?,
            },
        })
    }

    fn pos(&self, chave: &str, y: &Yaml) -> R<DefPos> {
        let onde = &format!("post_process_builders.{chave}");
        self.checar(
            y,
            onde,
            &["builder_factory", "import", "input_extensions", "target", "defaults"],
            &["builder_factory", "import"],
        )?;
        Ok(DefPos {
            chave: chave_builder_definicao(chave, self.pacote),
            pacote: self.pacote.to_string(),
            fabrica: self.texto(&y["builder_factory"], onde)?.unwrap_or_default(),
            import: self.texto(&y["import"], onde)?.unwrap_or_default(),
            padroes: match &y["defaults"] {
                Yaml::Null | Yaml::BadValue => Padroes::default(),
                d => self.padroes(d, onde)?,
            },
        })
    }

    fn alvo(&self, chave: &str, y: &Yaml) -> R<Alvo> {
        let onde = &format!("targets.{chave}");
        if !matches!(y, Yaml::Hash(_)) {
            return Err(self.erro(onde, "esperava um mapa"));
        }
        self.checar(y, onde, &["auto_apply_builders", "builders", "dependencies", "sources"], &[])?;
        let mut builders = Vec::new();
        if let Some(h) = self.mapa(&y["builders"], onde)? {
            for (k, v) in h {
                let k = k.as_str().ok_or_else(|| self.erro(onde, "chave de builder não é texto"))?;
                let o = &format!("{onde}.builders.{k}");
                if !matches!(v, Yaml::Hash(_)) {
                    return Err(self.erro(o, "esperava um mapa"));
                }
                self.checar(v, o, &["enabled", "generate_for", "options", "dev_options", "release_options"], &[])?;
                let cfg = CfgBuilderAlvo {
                    habilitado: self.booleano(&v["enabled"], o)?.unwrap_or(true),
                    generate_for: self.input_set(&v["generate_for"], o)?,
                    options: self.opcoes(&v["options"], o)?,
                    dev_options: self.opcoes(&v["dev_options"], o)?,
                    release_options: self.opcoes(&v["release_options"], o)?,
                };
                let chave_b = chave_builder_uso(k, self.pacote);
                if let Some(p) = builders.iter_mut().find(|(c, _): &&mut (String, CfgBuilderAlvo)| *c == chave_b) {
                    p.1 = cfg;
                } else {
                    builders.push((chave_b, cfg));
                }
            }
        }
        let dependencias = match self.lista_textos(&y["dependencies"], onde)? {
            Some(v) => v,
            None => self.deps.to_vec(),
        };
        Ok(Alvo {
            chave: chave_alvo_definicao(chave, self.pacote),
            pacote: self.pacote.to_string(),
            auto_apply_builders: self.booleano(&y["auto_apply_builders"], onde)?.unwrap_or(true),
            builders,
            dependencias: dependencias.iter().map(|d| chave_alvo_uso(d, self.pacote)).collect(),
            sources: self.input_set(&y["sources"], onde)?.unwrap_or_default(),
        })
    }

    fn config(&self, y: &Yaml) -> R<BuildConfig> {
        let vazio = Yaml::Hash(Default::default());
        let y = if matches!(y, Yaml::Null) { &vazio } else { y };
        if !matches!(y, Yaml::Hash(_)) {
            return Err(self.erro("build.yaml", "esperava um mapa"));
        }
        self.checar(
            y,
            "build.yaml",
            &["builders", "post_process_builders", "targets", "global_options", "additional_public_assets"],
            &[],
        )?;
        let mut alvos = Vec::new();
        match self.mapa(&y["targets"], "targets")? {
            None => alvos = BuildConfig::padrao(self.pacote, self.deps).alvos,
            Some(h) => {
                for (k, v) in h {
                    let k = k.as_str().ok_or_else(|| self.erro("targets", "chave não é texto"))?;
                    alvos.push(self.alvo(k, v)?);
                }
                let padrao = format!("{0}:{0}", self.pacote);
                if !alvos.iter().any(|a| a.chave == padrao) {
                    return Err(self.erro(
                        "targets",
                        &format!("Must specify a target with the name `{}` or `$default`.", self.pacote),
                    ));
                }
            }
        }
        let mut global = Vec::new();
        if let Some(h) = self.mapa(&y["global_options"], "global_options")? {
            for (k, v) in h {
                let k = k.as_str().ok_or_else(|| self.erro("global_options", "chave não é texto"))?;
                let o = &format!("global_options.{k}");
                if !matches!(v, Yaml::Hash(_)) {
                    return Err(self.erro(o, "esperava um mapa"));
                }
                self.checar(v, o, &["options", "dev_options", "release_options", "runs_before"], &[])?;
                global.push((
                    chave_builder_uso(k, self.pacote),
                    CfgGlobal {
                        options: self.opcoes(&v["options"], o)?,
                        dev_options: self.opcoes(&v["dev_options"], o)?,
                        release_options: self.opcoes(&v["release_options"], o)?,
                        runs_before: self
                            .lista_textos(&v["runs_before"], o)?
                            .unwrap_or_default()
                            .iter()
                            .map(|b| chave_builder_uso(b, self.pacote))
                            .collect(),
                    },
                ));
            }
        }
        let mut builders = Vec::new();
        if let Some(h) = self.mapa(&y["builders"], "builders")? {
            for (k, v) in h {
                let k = k.as_str().ok_or_else(|| self.erro("builders", "chave não é texto"))?;
                let b = self.builder(k, v)?;
                builders.retain(|x: &DefBuilder| x.chave != b.chave);
                builders.push(b);
            }
        }
        let mut pos = Vec::new();
        if let Some(h) = self.mapa(&y["post_process_builders"], "post_process_builders")? {
            for (k, v) in h {
                let k = k.as_str().ok_or_else(|| self.erro("post_process_builders", "chave não é texto"))?;
                let p = self.pos(k, v)?;
                pos.retain(|x: &DefPos| x.chave != p.chave);
                pos.push(p);
            }
        }
        Ok(BuildConfig {
            pacote: self.pacote.to_string(),
            builders,
            pos,
            alvos,
            global,
            publicos_adicionais: self.lista_textos(&y["additional_public_assets"], "additional_public_assets")?.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn normalizacao_de_chaves() {
        assert_eq!(chave_builder_definicao("x", "p"), "p:x");
        assert_eq!(chave_builder_definicao("q|x", "p"), "q:x");
        assert_eq!(chave_builder_uso("ngdart", "p"), "ngdart:ngdart");
        assert_eq!(chave_builder_uso(":x", "p"), "p:x");
        assert_eq!(chave_builder_uso("ngdart|placeholder_cleanup", "p"), "ngdart:placeholder_cleanup");
        assert_eq!(chave_alvo_definicao("$default", "p"), "p:p");
        assert_eq!(chave_alvo_uso(":$default", "p"), "p:p");
        assert_eq!(chave_alvo_uso("dep", "p"), "dep:dep");
    }

    #[test]
    fn build_yaml_do_sass_builder() {
        let t = r#"
builders:
  sass_builder:
    import: "package:sass_builder/sass_builder.dart"
    builder_factories: ["sassBuilder"]
    auto_apply: dependents
    build_extensions:
      .scss: [".css", ".css.map"]
      .sass: [".css", ".css.map"]
    applies_builders:
      - sass_builder:sass_source_cleanup
    defaults:
      release_options:
        outputStyle: compressed
        sourceMaps: false
      dev_options:
        sourceMaps: true
post_process_builders:
  sass_source_cleanup:
    import: "package:sass_builder/sass_builder.dart"
    builder_factory: "sassSourceCleanup"
    defaults:
      release_options:
        enabled: true
"#;
        let c = BuildConfig::de_texto("sass_builder", &["sass".into()], t, "teste").unwrap();
        let b = &c.builders[0];
        assert_eq!(b.chave, "sass_builder:sass_builder");
        assert_eq!(b.auto_apply, AutoApply::Dependentes);
        assert_eq!(b.build_to, BuildTo::Cache);
        assert_eq!(b.padroes.release_options.texto_canonico(), r#"{"outputStyle": "compressed", "sourceMaps": false}"#);
        assert_eq!(c.pos[0].chave, "sass_builder:sass_source_cleanup");
        assert_eq!(c.alvos[0].chave, "sass_builder:sass_builder");
        assert_eq!(c.alvos[0].dependencias, vec!["sass:sass"]);
    }

    #[test]
    fn chave_desconhecida_e_erro() {
        assert!(BuildConfig::de_texto("p", &[], "targets:\n  $default:\n    fontes: []\n", "t").is_err());
        assert!(BuildConfig::de_texto("p", &[], "targets:\n  outro: {}\n", "t").is_err());
        assert!(BuildConfig::de_texto("p", &[], "", "t").is_ok());
    }
}
