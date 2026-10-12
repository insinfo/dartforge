//! O `PubspecValidator` do analyzer (`analyzer/lib/src/pubspec/`, lido na
//! 6.11.0; docs/ANALYZER-ESPECIFICACAO-INFRA.md, lote II.10): os sete
//! sub-validadores de `validatePubspec`, na ordem do original
//! (dependências, campos, flutter, nome, capturas de tela, plataformas,
//! workspace). Todos são avisos, no intervalo do nó YAML.
//!
//! Fora: os lints de pubspec e o filtro de `# ignore` do fim de
//! `validatePubspec`. A existência de arquivos é consultada no disco.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g::pubspec as c;
use super::yaml::{self, No, Valor};
use super::Relato;
use dartforge_diagnostics::Span;
use std::path::{Path, PathBuf};

/// `joinAll(posix.split(p))` a partir da pasta do pubspec.
fn caminho(pasta: &Path, relativo: &str) -> PathBuf {
    let mut saida = pasta.to_path_buf();
    if relativo.starts_with('/') {
        saida = PathBuf::from("/");
    }
    for parte in relativo.split('/').filter(|p| !p.is_empty()) {
        saida.push(parte);
    }
    saida
}

/// O caminho sem `.` e `..` (`normalize`), sem consultar o disco.
fn normalizar(p: &Path) -> PathBuf {
    let mut saida = PathBuf::new();
    for parte in p.components() {
        match parte {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                saida.pop();
            }
            outra => saida.push(outra.as_os_str()),
        }
    }
    saida
}

struct Ctx<'a> {
    pasta: &'a Path,
    out: Vec<Relato>,
}

impl Ctx<'_> {
    fn relatar(&mut self, codigo: &'static super::CodigoNaoDart, no: &No, args: &[&str]) {
        self.out.push(Relato::novo(codigo, no.span, args));
    }

    /// `validatePathEntries`.
    fn entradas_de_caminho(&mut self, dependencia: &No, checar: bool) {
        if dependencia.mapa().is_none() {
            return;
        }
        if let Some((chave, valor)) = dependencia.par("path")
            && let Some(p) = valor.texto()
        {
            if p.contains('\\') {
                self.relatar(&c::PATH_NOT_POSIX, valor, &[p]);
                return;
            }
            let pasta = normalizar(&caminho(self.pasta, p));
            if !pasta.is_dir() {
                self.relatar(&c::PATH_DOES_NOT_EXIST, valor, &[p]);
            } else if !pasta.join("pubspec.yaml").is_file() {
                self.relatar(&c::PATH_PUBSPEC_DOES_NOT_EXIST, valor, &[p]);
            }
            if checar {
                self.relatar(&c::INVALID_DEPENDENCY, chave, &["path"]);
            }
        }
        if let Some((chave, valor)) = dependencia.par("git")
            && !valor.nulo()
            && checar
        {
            self.relatar(&c::INVALID_DEPENDENCY, chave, &["git"]);
        }
    }

    /// `getDeclaredDependencies`: ausente ou nulo, nada; mapa, os pares;
    /// outro, o aviso.
    fn dependencias_de<'n>(&mut self, raiz: &'n No, campo: &str) -> &'n [(No, No)] {
        match raiz.campo(campo) {
            None => &[],
            Some(v) if v.nulo() => &[],
            Some(v) => match v.mapa() {
                Some(m) => m,
                None => {
                    self.relatar(&c::DEPENDENCIES_FIELD_NOT_MAP, v, &[campo]);
                    &[]
                }
            },
        }
    }

    /// `dependencyValidator`.
    fn dependencias(&mut self, raiz: &No) {
        let normais = self.dependencias_de(raiz, "dependencies");
        let de_desenvolvimento = self.dependencias_de(raiz, "dev_dependencies");
        let publicavel = raiz.campo("version").is_some_and(|v| !v.nulo()) && raiz.campo("publish_to").and_then(No::texto) != Some("none");
        for (_, valor) in normais {
            self.entradas_de_caminho(valor, publicavel);
        }
        for (chave, valor) in de_desenvolvimento {
            if let Some(nome) = chave.texto()
                && normais.iter().any(|(k, _)| k.texto() == Some(nome))
            {
                self.relatar(&c::UNNECESSARY_DEV_DEPENDENCY, chave, &[nome]);
            }
            self.entradas_de_caminho(valor, false);
        }
    }

    /// `fieldValidator`: as chaves de topo que não se usam mais.
    fn campos(&mut self, raiz: &No) {
        for (chave, _) in raiz.mapa().unwrap_or(&[]) {
            if let Some(nome) = chave.texto()
                && ["author", "authors", "transformers", "web"].contains(&nome)
            {
                self.relatar(&c::DEPRECATED_FIELD, chave, &[nome]);
            }
        }
    }

    /// `_assetExistsAtPath`: pasta, arquivo, ou o arquivo numa subpasta
    /// qualquer do pai (as variantes de resolução).
    fn recurso_existe(&self, p: &Path) -> bool {
        if p.is_dir() || p.is_file() {
            return true;
        }
        let (Some(nome), Some(pai)) = (p.file_name(), p.parent()) else { return false };
        let Ok(filhos) = std::fs::read_dir(pai) else { return false };
        filhos.flatten().any(|f| f.path().is_dir() && f.path().join(nome).is_file())
    }

    /// `_validateAssetPath`.
    fn caminho_de_recurso(&mut self, valor: &str, no: &No) {
        if valor.starts_with("packages/") {
            return;
        }
        if !self.recurso_existe(&caminho(self.pasta, valor)) {
            let codigo = if valor.ends_with('/') { &c::ASSET_DIRECTORY_DOES_NOT_EXIST } else { &c::ASSET_DOES_NOT_EXIST };
            self.relatar(codigo, no, &[valor]);
        }
    }

    /// `flutterValidator`.
    fn flutter(&mut self, raiz: &No) {
        let Some(flutter) = raiz.campo("flutter") else { return };
        if flutter.mapa().is_none() {
            // `flutter:` vazio é aceito.
            if !flutter.nulo() {
                self.relatar(&c::FLUTTER_FIELD_NOT_MAP, flutter, &[]);
            }
            return;
        }
        let Some(recursos) = flutter.campo("assets") else { return };
        let Some(itens) = recursos.lista() else {
            self.relatar(&c::ASSET_FIELD_NOT_LIST, recursos, &[]);
            return;
        };
        for item in itens {
            if item.escalar() {
                // Um escalar que não é string interrompe o validador.
                let Some(entrada) = item.texto() else {
                    self.relatar(&c::ASSET_NOT_STRING_OR_MAP, item, &[]);
                    return;
                };
                self.caminho_de_recurso(entrada, item);
            } else if item.mapa().is_some() {
                match item.campo("path") {
                    None => self.relatar(&c::ASSET_MISSING_PATH, item, &[]),
                    Some(p) if !p.escalar() => self.relatar(&c::ASSET_PATH_NOT_STRING, p, &[]),
                    Some(p) => {
                        let Some(entrada) = p.texto() else {
                            self.relatar(&c::ASSET_NOT_STRING, p, &[]);
                            return;
                        };
                        self.caminho_de_recurso(entrada, p);
                    }
                }
            } else {
                self.relatar(&c::ASSET_NOT_STRING_OR_MAP, item, &[]);
            }
        }
    }

    /// `nameValidator`, para um documento que é mapa.
    fn nome(&mut self, raiz: &No) {
        match raiz.campo("name") {
            None => self.out.push(Relato::novo(&c::MISSING_NAME, Span { start: 0, end: 0 }, &[])),
            Some(n) if n.texto().is_none() => self.relatar(&c::NAME_NOT_STRING, n, &[]),
            Some(_) => {}
        }
    }

    /// `screenshotsValidator`: o arquivo de cada `path:` existe.
    fn capturas(&mut self, raiz: &No) {
        let Some(itens) = raiz.campo("screenshots").and_then(No::lista) else { return };
        for item in itens {
            if let Some(p) = item.campo("path")
                && let Some(texto) = p.texto()
                && !caminho(self.pasta, texto).is_file()
            {
                self.relatar(&c::PATH_DOES_NOT_EXIST, p, &[texto]);
            }
        }
    }

    /// `platformsValidator`.
    fn plataformas(&mut self, raiz: &No, fonte: &str) {
        let Some(plataformas) = raiz.campo("platforms") else { return };
        let Some(pares) = plataformas.mapa() else {
            self.relatar(&c::INVALID_PLATFORMS_FIELD, plataformas, &[]);
            return;
        };
        for (chave, _) in pares {
            let conhecida = chave.texto().is_some_and(|t| ["android", "ios", "linux", "macos", "web", "windows"].contains(&t));
            if !conhecida {
                // A string, o número, ou o nó como escrito.
                let mostrado = match &chave.valor {
                    Valor::Texto(t) | Valor::Outro(t) => t.clone(),
                    _ => fonte.get(chave.span.start..chave.span.end).unwrap_or("").to_string(),
                };
                self.relatar(&c::UNKNOWN_PLATFORM, chave, &[mostrado.as_str()]);
            }
        }
        for (_, valor) in pares {
            if !valor.nulo() {
                self.relatar(&c::PLATFORM_VALUE_DISALLOWED, valor, &[]);
            }
        }
    }

    /// `workspaceValidator`.
    fn workspace(&mut self, raiz: &No) {
        let Some(campo) = raiz.campo("workspace") else { return };
        let Some(itens) = campo.lista() else {
            self.relatar(&c::WORKSPACE_FIELD_NOT_LIST, campo, &[]);
            return;
        };
        for item in itens {
            if !item.escalar() {
                self.relatar(&c::WORKSPACE_VALUE_NOT_STRING, item, &[]);
                continue;
            }
            let Some(entrada) = item.texto() else {
                self.relatar(&c::WORKSPACE_VALUE_NOT_STRING, item, &[]);
                return;
            };
            // `_validateDirectoryPath`: subpasta da raiz, e existente.
            let pasta = normalizar(&caminho(self.pasta, entrada));
            let raiz_do_pacote = normalizar(self.pasta);
            if pasta == raiz_do_pacote || !pasta.starts_with(&raiz_do_pacote) {
                let texto = self.pasta.to_string_lossy().into_owned();
                self.relatar(&c::WORKSPACE_VALUE_NOT_SUBDIRECTORY, item, &[texto.as_str()]);
            } else if !pasta.is_dir() {
                self.relatar(&c::PATH_DOES_NOT_EXIST, item, &[entrada]);
            }
        }
    }
}

/// `validatePubspec`: os relatos do `pubspec.yaml` de texto `fonte` que
/// está na pasta `pasta`. Um erro de sintaxe do YAML não é relatado aqui
/// (o servidor trata o documento como mapa vazio): só falta o nome.
pub fn validar(fonte: &str, pasta: &Path) -> Vec<Relato> {
    let mut ctx = Ctx { pasta, out: Vec::new() };
    let raiz = match yaml::ler(fonte) {
        Ok(Some(no)) if no.mapa().is_some() => no,
        // Documento que não é mapa: só o `missing_name`, em 0/0.
        _ => return vec![Relato::novo(&c::MISSING_NAME, Span { start: 0, end: 0 }, &[])],
    };
    ctx.dependencias(&raiz);
    ctx.campos(&raiz);
    ctx.flutter(&raiz);
    ctx.nome(&raiz);
    ctx.capturas(&raiz);
    ctx.plataformas(&raiz, fonte);
    ctx.workspace(&raiz);
    ctx.out
}

#[cfg(test)]
mod testes {
    use super::*;

    /// `(nome do código, texto do intervalo)` de cada relato.
    fn achados(fonte: &str) -> Vec<(&'static str, String)> {
        // Uma pasta que não existe: todo caminho relativo falta.
        let pasta = Path::new("pasta-que-nao-existe-para-o-teste");
        validar(fonte, pasta).into_iter().map(|r| (r.codigo.nome, fonte[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn nome_e_campos() {
        assert_eq!(achados("version: 1.0.0\n"), vec![("missing_name", String::new())]);
        assert_eq!(achados("name: 3\n"), vec![("name_not_string", "3".to_string())]);
        assert_eq!(achados("name: a\nauthor: eu\n"), vec![("deprecated_field", "author".to_string())]);
        assert_eq!(achados("- a\n"), vec![("missing_name", String::new())]);
    }

    #[test]
    fn dependencias() {
        let fonte = "name: a\nversion: 1.0.0\ndependencies:\n  b:\n    path: ../b\n  c:\n    git: https://x\ndev_dependencies:\n  b: any\n";
        assert_eq!(
            achados(fonte),
            vec![
                ("path_does_not_exist", "../b".to_string()),
                ("invalid_dependency", "path".to_string()),
                ("invalid_dependency", "git".to_string()),
                ("unnecessary_dev_dependency", "b".to_string()),
            ]
        );
        assert_eq!(achados("name: a\ndependencies: 3\n"), vec![("dependencies_field_not_map", "3".to_string())]);
        // Com barra invertida, só o `path_not_posix`.
        assert_eq!(achados("name: a\ndependencies:\n  b:\n    path: ..\\b\n"), vec![("path_not_posix", "..\\b".to_string())]);
    }

    #[test]
    fn plataformas_e_workspace() {
        assert_eq!(
            achados("name: a\nplatforms:\n  android:\n  fuchsia: x\n"),
            vec![("unknown_platform", "fuchsia".to_string()), ("platform_value_disallowed", "x".to_string())]
        );
        assert_eq!(achados("name: a\nworkspace: x\n"), vec![("workspace_field_not_list", "x".to_string())]);
    }
}
