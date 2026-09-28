//! A compilação como o `sass_builder` 2.2.1 a faz (`lib/sass_builder.dart`
//! e `lib/src/build_importer.dart`): a entrada é um ativo do `build_runner`
//! (`package:pkg/x.scss` para o que está em `lib/`, `asset:pkg/web/y.scss`
//! para o resto), e as folhas que ela carrega são resolvidas pelo
//! `BuildImporter`, isto é, pelo `AssetId.resolve` do pacote `build`
//! contra os pacotes do `package_config.json`.
//!
//! Opções que o `sass_builder` passa ao dart-sass
//! (`compileStringToResultAsync`): `syntax: Syntax.forPath(entrada)`,
//! `importers: [BuildImporter]`, `style` (`expanded` por padrão,
//! `compressed`), `url: inputId.uri` e `sourceMap`. Nada de `loadPaths`,
//! `quietDeps`, `charset` (padrão: ligado) nem funções próprias; o
//! importador base da entrada é o `FilesystemImporter.cwd`, que não resolve
//! URL `package:`/`asset:` — na prática só o `BuildImporter` resolve.
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::importer::{Importer, ImporterResult};
use crate::{InputSyntax, Options, OutputStyle};

/// Um ativo do `build_runner`: pacote e caminho dentro dele (`lib/a.scss`,
/// `web/b.scss`), sempre com `/`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ativo {
    pub pacote: String,
    pub caminho: String,
}

impl Ativo {
    /// `AssetId.uri`: `package:` dentro de `lib/`, `asset:` fora.
    pub fn uri(&self) -> String {
        match self.caminho.strip_prefix("lib/") {
            Some(resto) => format!("package:{}/{resto}", self.pacote),
            None => format!("asset:{}/{}", self.pacote, self.caminho),
        }
    }

    /// `AssetId.resolve(uri, from: de)`: URL relativa resolve contra a URI
    /// de `de` (`Uri.resolveUri`); só `package:` e `asset:` são aceitas.
    pub fn resolver(uri: &str, de: &Ativo) -> Result<Ativo, String> {
        let resolvida = match esquema(uri) {
            Some(_) => uri.to_owned(),
            None => resolver_relativa(&de.uri(), uri),
        };
        let erro =
            || format!("Cannot resolve {uri}; only \"package\" and \"asset\" schemes supported");
        let (esq, resto) = resolvida.split_once(':').ok_or_else(erro)?;
        // `pathSegments` do `Uri` do Dart: o caminho sem consulta/fragmento,
        // dividido em `/` (um `/` inicial não gera segmento vazio).
        let resto = resto.split(['?', '#']).next().unwrap_or("");
        let resto = resto.strip_prefix('/').unwrap_or(resto);
        let mut segs = resto.split('/');
        let pacote = segs.next().unwrap_or("").to_owned();
        let demais: Vec<&str> = segs.collect();
        match esq {
            "package" => Ok(Ativo {
                pacote,
                caminho: juntar("lib", &demais),
            }),
            "asset" => Ok(Ativo {
                pacote,
                caminho: juntar("", &demais),
            }),
            _ => Err(erro()),
        }
    }
}

/// `p.url.join(base, p.url.joinAll(segs))` com a normalização que o
/// construtor de `AssetId` faz (`p.url.normalize`).
fn juntar(base: &str, segs: &[&str]) -> String {
    let mut partes: Vec<&str> = Vec::new();
    for s in std::iter::once(base).chain(segs.iter().copied()) {
        match s {
            "" | "." => {}
            ".." => {
                if partes.last().is_some_and(|u| *u != "..") {
                    partes.pop();
                } else {
                    partes.push("..");
                }
            }
            s => partes.push(s),
        }
    }
    partes.join("/")
}

/// O esquema de uma URI (`[a-zA-Z][a-zA-Z0-9+.-]*:`), se houver.
fn esquema(uri: &str) -> Option<&str> {
    let (e, _) = uri.split_once(':')?;
    let mut cs = e.chars();
    let primeiro = cs.next()?;
    (primeiro.is_ascii_alphabetic() && cs.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)))
        .then_some(e)
}

/// `Uri.resolveUri` do Dart para uma referência sem esquema contra uma base
/// sem autoridade (`package:`/`asset:`): RFC 3986 §5.2, com a remoção de
/// segmentos de ponto do Dart (`_removeDotSegments`, que para caminho sem
/// `/` inicial mantém `..` que sobram — `_normalizeRelativePath`).
fn resolver_relativa(base: &str, rel: &str) -> String {
    let (esq, caminho_base) = base.split_once(':').unwrap_or(("", base));
    let caminho_base = caminho_base.split(['?', '#']).next().unwrap_or("");
    let (rel_caminho, sufixo) = match rel.find(['?', '#']) {
        Some(i) => (&rel[..i], &rel[i..]),
        None => (rel, ""),
    };
    if rel_caminho.is_empty() {
        return format!("{esq}:{caminho_base}{sufixo}");
    }
    let juntado = if rel_caminho.starts_with('/') {
        rel_caminho.to_owned()
    } else {
        match caminho_base.rfind('/') {
            Some(i) => format!("{}{rel_caminho}", &caminho_base[..=i]),
            None => rel_caminho.to_owned(),
        }
    };
    let absoluto = juntado.starts_with('/');
    let mut saida: Vec<&str> = Vec::new();
    let segs: Vec<&str> = juntado.split('/').collect();
    let n = segs.len();
    let mut fim_com_barra = false;
    for (i, s) in segs.iter().enumerate() {
        let ultimo = i + 1 == n;
        match *s {
            "." => fim_com_barra = ultimo,
            ".." => {
                if saida.last().is_some_and(|u| *u != ".." && !u.is_empty()) {
                    saida.pop();
                } else if !absoluto {
                    saida.push("..");
                }
                fim_com_barra = ultimo;
            }
            "" if i == 0 && absoluto => {}
            s => {
                saida.push(s);
                fim_com_barra = false;
            }
        }
    }
    let mut c = saida.join("/");
    if absoluto {
        c.insert(0, '/');
    }
    if fim_com_barra {
        c.push('/');
    }
    format!("{esq}:{c}{sufixo}")
}

/// As raízes dos pacotes, por nome, como no `package_config.json`.
#[derive(Debug, Clone, Default)]
pub struct Pacotes {
    raizes: HashMap<String, PathBuf>,
}

impl Pacotes {
    /// Pacotes vazios; acrescente com [`Pacotes::inserir`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Acrescenta (ou troca) a raiz de um pacote.
    pub fn inserir(&mut self, nome: impl Into<String>, raiz: impl Into<PathBuf>) {
        self.raizes.insert(nome.into(), raiz.into());
    }

    /// A raiz de `nome`, se conhecida.
    pub fn raiz(&self, nome: &str) -> Option<&Path> {
        self.raizes.get(nome).map(PathBuf::as_path)
    }

    /// Lê um `package_config.json` (versão 2): `rootUri` absoluta
    /// (`file:///…`) ou relativa ao diretório do arquivo.
    ///
    /// # Erros
    ///
    /// O arquivo não se lê ou não tem a forma esperada.
    pub fn de_package_config(caminho: &Path) -> io::Result<Self> {
        let texto = std::fs::read_to_string(caminho)?;
        let dir = caminho.parent().unwrap_or(Path::new(""));
        let mut p = Pacotes::new();
        let invalido =
            || io::Error::new(io::ErrorKind::InvalidData, "package_config.json inválido");
        // Leitura mínima de JSON: os objetos de `packages` com `name` e
        // `rootUri` (o crate não depende de um leitor de JSON).
        let lista = texto.find("\"packages\"").ok_or_else(invalido)?;
        let mut resto = &texto[lista..];
        while let Some(i) = resto.find('{') {
            let fim = resto[i..].find('}').ok_or_else(invalido)? + i;
            let obj = &resto[i..fim];
            if let (Some(nome), Some(raiz)) = (campo_json(obj, "name"), campo_json(obj, "rootUri"))
            {
                let raiz = match raiz.strip_prefix("file://") {
                    Some(abs) => PathBuf::from(decodificar_pct(abs)),
                    None => dir.join(decodificar_pct(&raiz)),
                };
                p.inserir(nome, normalizar(&raiz));
            }
            resto = &resto[fim + 1..];
        }
        Ok(p)
    }
}

fn campo_json(obj: &str, nome: &str) -> Option<String> {
    let i = obj.find(&format!("\"{nome}\""))? + nome.len() + 2;
    let depois = obj[i..]
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?;
    let mut s = String::new();
    let mut cs = depois.chars();
    while let Some(c) = cs.next() {
        match c {
            '"' => return Some(s),
            '\\' => match cs.next()? {
                'n' => s.push('\n'),
                't' => s.push('\t'),
                'u' => {
                    let h: String = cs.by_ref().take(4).collect();
                    s.push(char::from_u32(u32::from_str_radix(&h, 16).ok()?)?);
                }
                o => s.push(o),
            },
            c => s.push(c),
        }
    }
    None
}

fn decodificar_pct(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn normalizar(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// Como o importador lê os ativos: o `BuildStep.canRead`/`readAsString`.
/// O padrão é o disco; o motor de build passa o dele (que vê as saídas de
/// builders anteriores e registra as dependências).
pub trait Leitor {
    /// O arquivo existe e pode ser lido?
    fn existe(&self, caminho: &Path) -> bool;
    /// O texto do arquivo.
    ///
    /// # Erros
    ///
    /// O arquivo não se lê ou não é UTF-8.
    fn ler(&self, caminho: &Path) -> io::Result<String>;
}

/// O disco, sem mais nada.
#[derive(Debug, Clone, Copy, Default)]
pub struct Disco;

impl Leitor for Disco {
    fn existe(&self, caminho: &Path) -> bool {
        caminho.is_file()
    }
    fn ler(&self, caminho: &Path) -> io::Result<String> {
        std::fs::read_to_string(caminho)
    }
}

/// O `BuildImporter` do `sass_builder` sobre um [`Leitor`], guardando o
/// que foi lido e o que foi sondado sem existir (as dependências da ação).
struct BuildImporter<'a> {
    entrada: &'a Ativo,
    pacotes: &'a Pacotes,
    leitor: &'a dyn Leitor,
    lidos: RefCell<Vec<PathBuf>>,
    sondados: RefCell<Vec<PathBuf>>,
}

impl std::fmt::Debug for BuildImporter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildImporter")
            .field("entrada", self.entrada)
            .finish()
    }
}

impl BuildImporter<'_> {
    fn arquivo(&self, a: &Ativo) -> Option<PathBuf> {
        self.pacotes.raiz(&a.pacote).map(|r| r.join(&a.caminho))
    }

    /// `buildStep.canRead(id)`.
    fn pode_ler(&self, a: &Ativo) -> bool {
        let Some(f) = self.arquivo(a) else {
            return false;
        };
        let sim = self.leitor.existe(&f);
        let mut lista = if sim {
            self.lidos.borrow_mut()
        } else {
            self.sondados.borrow_mut()
        };
        if !lista.contains(&f) {
            lista.push(f);
        }
        sim
    }

    /// `_resolveImport`, resolvendo relativas contra `de`.
    fn resolver(&self, import: &str, de: &Ativo) -> Result<Option<Ativo>, String> {
        let ext = extensao(import);
        if ext == ".sass" || ext == ".scss" {
            return exatamente_um(self.tentar(import, de)?);
        }
        if let Some(a) = exatamente_um(self.tentar_com_extensoes(import, de)?)? {
            return Ok(Some(a));
        }
        // `_tryImportAsDirectory`: `p.url.join(import, 'index')`.
        let indice = if import.is_empty() || import.ends_with('/') {
            format!("{import}index")
        } else {
            format!("{import}/index")
        };
        exatamente_um(self.tentar_com_extensoes(&indice, de)?)
    }

    fn tentar_com_extensoes(&self, import: &str, de: &Ativo) -> Result<Vec<Ativo>, String> {
        let mut v = self.tentar(&format!("{import}.sass"), de)?;
        v.extend(self.tentar(&format!("{import}.scss"), de)?);
        Ok(v)
    }

    /// `_tryImport`: o parcial `_nome` e o próprio, nessa ordem.
    fn tentar(&self, import: &str, de: &Ativo) -> Result<Vec<Ativo>, String> {
        let mut v = Vec::new();
        let (dir, base) = dirname_basename(import);
        let parcial = if dir == "." {
            format!("_{base}")
        } else if dir.ends_with('/') {
            format!("{dir}_{base}")
        } else {
            format!("{dir}/_{base}")
        };
        let parcial = Ativo::resolver(&parcial, de)?;
        if self.pode_ler(&parcial) {
            v.push(parcial);
        }
        let proprio = Ativo::resolver(import, de)?;
        if self.pode_ler(&proprio) {
            v.push(proprio);
        }
        Ok(v)
    }
}

/// `p.extension` (estilo URL): do último `.` do último segmento, se não for
/// o primeiro caractere dele.
fn extensao(s: &str) -> &str {
    let base = s.rsplit('/').next().unwrap_or(s);
    match base.rfind('.') {
        Some(i) if i > 0 => &base[i..],
        _ => "",
    }
}

/// `p.dirname`/`p.basename` do pacote `path` (estilo POSIX, como o
/// `sass_builder` chama no Dart da VM em Linux), sem barras finais.
fn dirname_basename(s: &str) -> (String, String) {
    let t = s.trim_end_matches('/');
    if t.is_empty() {
        return (
            if s.starts_with('/') {
                "/".into()
            } else {
                ".".into()
            },
            if s.starts_with('/') {
                "/".into()
            } else {
                String::new()
            },
        );
    }
    match t.rfind('/') {
        Some(0) => ("/".into(), t[1..].into()),
        Some(i) => {
            let d = t[..i].trim_end_matches('/');
            (
                if d.is_empty() { "/".into() } else { d.into() },
                t[i + 1..].into(),
            )
        }
        None => (".".into(), t.into()),
    }
}

fn exatamente_um(v: Vec<Ativo>) -> Result<Option<Ativo>, String> {
    match v.len() {
        0 => Ok(None),
        1 => Ok(v.into_iter().next()),
        _ => Err("It is not clear which file to import.".into()),
    }
}

impl Importer for BuildImporter<'_> {
    fn canonicalize(
        &self,
        url: &str,
        base: &str,
        _for_import: bool,
    ) -> Result<Option<String>, String> {
        // `ImportCache.canonicalize`: URL relativa vai primeiro ao importador
        // da folha que a contém, resolvida contra a URL canônica dela; depois
        // à lista de importadores com a URL como veio — e o `BuildImporter`
        // a resolve contra a **entrada** (`AssetId.resolve(…, from:
        // inputId)`).
        if esquema(url).is_none() {
            if let Ok(de) = Ativo::resolver(base, self.entrada) {
                if de != *self.entrada {
                    let resolvida = resolver_relativa(base, url);
                    if let Some(a) = self.resolver(&resolvida, &de)? {
                        return Ok(Some(a.uri()));
                    }
                }
            }
        }
        Ok(self.resolver(url, self.entrada)?.map(|a| a.uri()))
    }

    fn load(&self, canonical: &str) -> Result<ImporterResult, String> {
        let a = Ativo::resolver(canonical, self.entrada)?;
        let f = self
            .arquivo(&a)
            .ok_or_else(|| format!("pacote desconhecido: {}", a.pacote))?;
        let contents = self.leitor.ler(&f).map_err(|e| format!("{e}"))?;
        let mut l = self.lidos.borrow_mut();
        if !l.contains(&f) {
            l.push(f);
        }
        Ok(ImporterResult {
            contents,
            syntax: InputSyntax::for_path(Path::new(&a.caminho)),
        })
    }
}

/// O resultado de [`compilar`].
#[derive(Debug, Clone)]
pub struct Compilado {
    /// O `compileResult.css` (sem `\n` final nem comentário de mapa).
    pub css: String,
    /// O `.css.map` como o `sass_builder` o escreve (`json.encode`, com as
    /// `sources` reescritas), quando pedido.
    pub mapa: Option<String>,
    /// Os arquivos que a compilação leu (dependências).
    pub lidos: Vec<PathBuf>,
    /// Os arquivos que o importador procurou e não achou (também
    /// dependências: criar um deles muda o resultado).
    pub sondados: Vec<PathBuf>,
}

impl Compilado {
    /// O `.css` que o `sass_builder` grava: o CSS, o comentário
    /// `sourceMappingURL` quando há mapa, e `\n`.
    pub fn arquivo_css(&self, entrada: &Ativo) -> String {
        match &self.mapa {
            Some(_) => {
                let nome = entrada.caminho.rsplit('/').next().unwrap_or("");
                let base = match nome.rfind('.') {
                    Some(i) if i > 0 => &nome[..i],
                    _ => nome,
                };
                format!("{}\n\n/*# sourceMappingURL={base}.css.map */\n", self.css)
            }
            None => format!("{}\n", self.css),
        }
    }
}

/// Compila `fonte`, o texto do ativo `entrada`, como o `SassBuilder` com o
/// dart-sass `versao`.
///
/// # Erros
///
/// O erro de compilação do Sass (a mensagem do grass/dart-sass).
pub fn compilar(
    fonte: &str,
    entrada: &Ativo,
    pacotes: &Pacotes,
    leitor: &dyn Leitor,
    estilo: OutputStyle,
    mapa: bool,
    versao: crate::VersaoDartSass,
) -> Result<Compilado, Box<crate::Error>> {
    let importer = BuildImporter {
        entrada,
        pacotes,
        leitor,
        lidos: RefCell::new(Vec::new()),
        sondados: RefCell::new(Vec::new()),
    };
    let opcoes = Options::default()
        .style(estilo)
        .versao(versao)
        .quiet(true)
        .importer(&importer)
        .input_syntax(InputSyntax::for_path(Path::new(&entrada.caminho)));
    let (css, mapa_bruto) =
        crate::compilar_com_mapa(fonte.to_owned(), &entrada.uri(), &opcoes, mapa)?;
    let mapa = mapa_bruto.map(|m| m.json_sass_builder(entrada));
    Ok(Compilado {
        css,
        mapa,
        lidos: importer.lidos.into_inner(),
        sondados: importer.sondados.into_inner(),
    })
}

/// A reescrita das `sources` do mapa que o `sass_builder` faz
/// (`writeSourceMaps`): o que está em `lib/` vira
/// `packages/<pacote>/<resto>`; o que está em `web/` do próprio pacote,
/// relativo a `web/`; o resto fica como veio.
pub fn fonte_do_mapa(url: &str, entrada: &Ativo) -> String {
    let Ok(a) = Ativo::resolver(url, entrada) else {
        return url.to_owned();
    };
    if let Some(resto) = a.caminho.strip_prefix("lib/") {
        return format!("packages/{}/{resto}", a.pacote);
    }
    if a.pacote == entrada.pacote {
        if let Some(resto) = a.caminho.strip_prefix("web/") {
            return resto.to_owned();
        }
    }
    url.to_owned()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn resolve_como_o_asset_id() {
        let e = Ativo {
            pacote: "app".into(),
            caminho: "web/css/main.scss".into(),
        };
        assert_eq!(e.uri(), "asset:app/web/css/main.scss");
        let a = Ativo::resolver("../x/_y.scss", &e).unwrap();
        assert_eq!(a.caminho, "web/x/_y.scss");
        let b = Ativo::resolver("package:ui/src/a.scss", &e).unwrap();
        assert_eq!(
            (b.pacote.as_str(), b.caminho.as_str()),
            ("ui", "lib/src/a.scss")
        );
        assert_eq!(
            fonte_do_mapa("package:ui/src/a.scss", &e),
            "packages/ui/src/a.scss"
        );
        assert_eq!(
            fonte_do_mapa("asset:app/web/css/main.scss", &e),
            "css/main.scss"
        );
        assert!(Ativo::resolver("http://x/y", &e).is_err());
    }
}
