//! As dependências do executor de builders que o projeto não resolve
//! (DF-BUILD-004): o hospedeiro não pode depender de o projeto trazer
//! `build_runner` ou `build_resolvers`. O lado Dart do executor
//! (`pacotes/build_executor`) importa [`DEPENDENCIAS`]; quando o
//! `package_config.json` do projeto não resolve algum deles, o hospedeiro os
//! procura no cache do pub (`PUB_CACHE`, ou o padrão da plataforma), com as
//! restrições do `pubspec.yaml` do executor, e depois as dependências deles
//! que também faltam, com as restrições de quem as pede — sempre a maior
//! versão do cache que satisfaz, como o pub escolheria entre as que já tem.
//! Um pacote que o projeto já resolve fica na versão do projeto.
//!
//! Não há rede: o que o cache não tem é diagnosticado pelo nome, pela
//! restrição e por quem o pediu.
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Os pacotes que o lado Dart do executor importa, com as restrições do
/// `pubspec.yaml` de `pacotes/build_executor`.
pub const DEPENDENCIAS: &[(&str, &str)] = &[
    ("analyzer", ">=6.9.0 <8.0.0"),
    ("build", "^2.4.2"),
    ("build_resolvers", "^2.4.4"),
    ("glob", "^2.1.0"),
    ("logging", "^1.2.0"),
    ("package_config", "^2.1.0"),
];

/// Uma versão semântica `maior.menor.patch[-pre][+build]` (o `+build` não
/// conta na ordem).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versao {
    pub numeros: [u64; 3],
    pub pre: Option<String>,
}

impl Versao {
    /// Lê `1.2.3`, `1.2.3-dev.1`, `1.2.3+4`.
    ///
    /// ```
    /// use dartforge_build::dependencias_executor::Versao;
    /// assert!(Versao::ler("2.4.2").unwrap() < Versao::ler("2.10.0").unwrap());
    /// assert!(Versao::ler("3.0.0-dev.1").unwrap() < Versao::ler("3.0.0").unwrap());
    /// ```
    pub fn ler(texto: &str) -> Option<Versao> {
        let texto = texto.trim();
        let sem_build = texto.split('+').next()?;
        let (nucleo, pre) = match sem_build.split_once('-') {
            Some((n, p)) => (n, Some(p.to_string())),
            None => (sem_build, None),
        };
        let mut partes = nucleo.split('.');
        let mut numeros = [0u64; 3];
        for n in &mut numeros {
            *n = partes.next()?.parse().ok()?;
        }
        if partes.next().is_some() {
            return None;
        }
        Some(Versao { numeros, pre })
    }
}

impl PartialOrd for Versao {
    fn partial_cmp(&self, outro: &Versao) -> Option<Ordering> {
        Some(self.cmp(outro))
    }
}

impl Ord for Versao {
    fn cmp(&self, outro: &Versao) -> Ordering {
        self.numeros
            .cmp(&outro.numeros)
            .then_with(|| match (&self.pre, &outro.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

/// Uma restrição de versão do pub: `any`, `1.2.3`, `^1.2.3`, e intervalos
/// com `>=`, `>`, `<=`, `<` separados por espaço.
///
/// ```
/// use dartforge_build::dependencias_executor::{Versao, satisfaz};
/// let v = |t| Versao::ler(t).unwrap();
/// assert!(satisfaz("^2.4.2", &v("2.9.0")) && !satisfaz("^2.4.2", &v("3.0.0")));
/// assert!(satisfaz("^0.2.1", &v("0.2.9")) && !satisfaz("^0.2.1", &v("0.3.0")));
/// assert!(satisfaz(">=6.9.0 <8.0.0", &v("7.7.1")));
/// assert!(!satisfaz("^1.0.0", &v("1.1.0-dev")));
/// ```
pub fn satisfaz(restricao: &str, v: &Versao) -> bool {
    let restricao = restricao.trim().trim_matches(|c| c == '\'' || c == '"');
    // Pré-lançamentos só quando a própria restrição cita um.
    if v.pre.is_some() && !restricao.contains('-') {
        return false;
    }
    if restricao.is_empty() || restricao == "any" {
        return true;
    }
    if let Some(base) = restricao.strip_prefix('^') {
        let Some(b) = Versao::ler(base) else {
            return false;
        };
        let teto = match b.numeros {
            [0, 0, p] => [0, 0, p + 1],
            [0, m, _] => [0, m + 1, 0],
            [x, _, _] => [x + 1, 0, 0],
        };
        return *v >= b && v.numeros < teto;
    }
    let mut resto = restricao;
    let mut algum = false;
    while !resto.trim_start().is_empty() {
        resto = resto.trim_start();
        let (op, depois) = [">=", "<=", ">", "<"]
            .iter()
            .find_map(|op| resto.strip_prefix(op).map(|d| (*op, d)))
            .unwrap_or(("=", resto));
        let depois = depois.trim_start();
        let fim = depois.find(char::is_whitespace).unwrap_or(depois.len());
        let Some(b) = Versao::ler(&depois[..fim]) else {
            return false;
        };
        let ok = match op {
            ">=" => *v >= b,
            ">" => *v > b,
            "<=" => *v <= b,
            "<" => *v < b,
            _ => *v == b,
        };
        if !ok {
            return false;
        }
        algum = true;
        resto = &depois[fim..];
    }
    algum
}

/// O diretório `hosted/pub.dev` do cache do pub: `PUB_CACHE`, senão
/// `%LOCALAPPDATA%\Pub\Cache` (Windows) ou `~/.pub-cache`.
pub fn cache_do_pub() -> Option<PathBuf> {
    let raiz = std::env::var_os("PUB_CACHE")
        .map(PathBuf::from)
        .or_else(|| {
            if cfg!(windows) {
                std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Pub").join("Cache"))
            } else {
                std::env::var_os("HOME").map(|d| PathBuf::from(d).join(".pub-cache"))
            }
        })?;
    Some(raiz.join("hosted").join("pub.dev"))
}

/// A maior versão de `nome` no cache que satisfaz `restricao`.
fn melhor_do_cache(cache: &Path, nome: &str, restricao: &str) -> Option<(Versao, PathBuf)> {
    let prefixo = format!("{nome}-");
    std::fs::read_dir(cache)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            let v = Versao::ler(n.strip_prefix(&prefixo)?)?;
            (satisfaz(restricao, &v) && e.path().join("pubspec.yaml").is_file())
                .then(|| (v, e.path()))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
}

/// As `dependencies` de um `pubspec.yaml`: nome → restrição (as de outra
/// fonte que não o pub.dev ficam com `any`).
fn dependencias_do_pubspec(texto: &str) -> (Vec<(String, String)>, Option<String>) {
    use yaml_rust2::{Yaml, YamlLoader};
    let Ok(docs) = YamlLoader::load_from_str(texto) else {
        return (Vec::new(), None);
    };
    let Some(doc) = docs.first() else {
        return (Vec::new(), None);
    };
    let mut deps = Vec::new();
    if let Yaml::Hash(h) = &doc["dependencies"] {
        for (k, v) in h {
            let Some(nome) = k.as_str() else { continue };
            let r = match v {
                Yaml::String(s) => s.clone(),
                Yaml::Real(s) => s.clone(),
                Yaml::Hash(_) => v["version"].as_str().unwrap_or("any").to_string(),
                _ => "any".to_string(),
            };
            deps.push((nome.to_string(), r));
        }
    }
    let sdk = doc["environment"]["sdk"].as_str().map(str::to_string);
    (deps, sdk)
}

/// A versão de linguagem de um pacote pela restrição do SDK (a menor versão
/// aceita, `maior.menor`), como o pub grava no `package_config.json`.
fn versao_de_linguagem(sdk: Option<&str>) -> String {
    let base = sdk
        .and_then(|s| {
            let s = s.trim();
            let s = s
                .strip_prefix('^')
                .or_else(|| s.strip_prefix(">="))
                .unwrap_or(s);
            Versao::ler(s.split_whitespace().next()?)
        })
        .map_or([2, 12, 0], |v| v.numeros);
    format!("{}.{}", base[0], base[1])
}

/// O `package_config.json` do projeto (`original`) completado com os pacotes
/// do executor que ele não resolve, tirados de `cache` (o `hosted/pub.dev`
/// do cache do pub). Devolve o JSON e os nomes acrescentados.
///
/// # Erros
///
/// O `package_config` não tem `packages`, ou o cache não tem uma versão que
/// satisfaça algum pacote que falta (a mensagem diz qual, com a restrição e
/// quem o pediu).
pub fn completar(original: &Value, cache: &Path) -> Result<(Value, Vec<String>), String> {
    let mut v = original.clone();
    let pacotes = v
        .get_mut("packages")
        .and_then(Value::as_array_mut)
        .ok_or("package_config sem 'packages'")?;
    let presentes: Vec<String> = pacotes
        .iter()
        .filter_map(|p| p["name"].as_str().map(str::to_string))
        .collect();
    let mut pendentes: Vec<(String, String, String)> = DEPENDENCIAS
        .iter()
        .map(|(n, r)| {
            (
                n.to_string(),
                r.to_string(),
                "dartforge_build_executor".to_string(),
            )
        })
        .collect();
    let mut escolhidos: BTreeMap<String, (Versao, PathBuf, Option<String>)> = BTreeMap::new();
    let mut faltam = Vec::new();
    while let Some((nome, restricao, quem)) = pendentes.pop() {
        if presentes.contains(&nome) || escolhidos.contains_key(&nome) {
            continue;
        }
        let Some((versao, dir)) = melhor_do_cache(cache, &nome, &restricao) else {
            faltam.push(format!("{nome} {restricao} (pedido por {quem})"));
            continue;
        };
        let texto = std::fs::read_to_string(dir.join("pubspec.yaml")).unwrap_or_default();
        let (deps, sdk) = dependencias_do_pubspec(&texto);
        for (d, r) in deps {
            pendentes.push((d, r, nome.clone()));
        }
        escolhidos.insert(nome, (versao, dir, sdk));
    }
    if !faltam.is_empty() {
        faltam.sort();
        return Err(format!(
            "o executor de builders precisa de pacotes que o projeto não resolve e que o cache do pub \
             ({}) não tem: {}. Declare-os nas dev_dependencies do projeto e resolva as dependências",
            cache.display(),
            faltam.join(", ")
        ));
    }
    let mut acrescentados = Vec::new();
    for (nome, (_, dir, sdk)) in escolhidos {
        let uri = url::Url::from_directory_path(&dir)
            .map(|u| u.to_string())
            .unwrap_or_default();
        pacotes.push(json!({
            "name": nome,
            "rootUri": uri,
            "packageUri": "lib/",
            "languageVersion": versao_de_linguagem(sdk.as_deref()),
        }));
        acrescentados.push(nome);
    }
    Ok((v, acrescentados))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pacote(cache: &Path, nome: &str, versao: &str, deps: &str) {
        let d = cache.join(format!("{nome}-{versao}"));
        std::fs::create_dir_all(d.join("lib")).unwrap();
        std::fs::write(
            d.join("pubspec.yaml"),
            format!("name: {nome}\nversion: {versao}\nenvironment:\n  sdk: '>=3.4.0 <4.0.0'\ndependencies:\n{deps}"),
        )
        .unwrap();
    }

    #[test]
    fn completa_pelo_cache_com_as_dependencias_transitivas() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path();
        pacote(
            cache,
            "build_resolvers",
            "2.4.4",
            "  analyzer: '>=6.9.0 <8.0.0'\n  build: ^2.0.0\n",
        );
        pacote(cache, "build_resolvers", "3.0.0", "");
        pacote(cache, "analyzer", "6.11.0", "");
        pacote(cache, "analyzer", "7.7.1", "");
        pacote(cache, "analyzer", "8.0.0", "");
        pacote(cache, "logging", "1.3.0", "");
        pacote(cache, "package_config", "2.2.0", "");
        let original = json!({"configVersion": 2, "packages": [
            {"name": "build", "rootUri": "file:///b/", "packageUri": "lib/"},
            {"name": "glob", "rootUri": "file:///g/", "packageUri": "lib/"},
        ]});
        let (v, novos) = completar(&original, cache).unwrap();
        assert_eq!(
            novos,
            vec!["analyzer", "build_resolvers", "logging", "package_config"]
        );
        let ps = v["packages"].as_array().unwrap();
        let raiz = |n: &str| {
            ps.iter().find(|p| p["name"] == n).unwrap()["rootUri"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert!(raiz("build_resolvers").ends_with("build_resolvers-2.4.4/"));
        assert!(raiz("analyzer").ends_with("analyzer-7.7.1/"));
        // O `build` do projeto fica.
        assert_eq!(raiz("build"), "file:///b/");
        assert_eq!(
            ps.iter().find(|p| p["name"] == "logging").unwrap()["languageVersion"],
            "3.4"
        );
    }

    #[test]
    fn diagnostica_o_que_o_cache_nao_tem() {
        let dir = tempfile::tempdir().unwrap();
        let original = json!({"configVersion": 2, "packages": []});
        let e = completar(&original, dir.path()).unwrap_err();
        assert!(
            e.contains("build_resolvers ^2.4.4 (pedido por dartforge_build_executor)"),
            "{e}"
        );
    }
}
