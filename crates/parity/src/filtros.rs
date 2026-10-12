//! Os filtros que o analyzer aplica por último (plano §2.2, item 4):
//! `analyzer: exclude:` e `analyzer: errors:` do `analysis_options.yaml`, e os
//! comentários `// ignore:` e `// ignore_for_file:`.
//!
//! Referências (analyzer 6.11.0): `IgnoreInfo` (`src/ignore_comments/
//! ignore_info.dart`: o comentário numa linha só vale para a linha seguinte;
//! no fim de uma linha de código, para ela mesma), `ErrorCode.isIgnorable`
//! (erro de severidade `ERROR` não se ignora por comentário) e o
//! `ErrorProcessor` das opções (`ignore` remove; `info`/`warning`/`error`
//! trocam a severidade).
//!
//! O `analysis_options.yaml` é lido com um leitor de YAML de verdade, com os
//! `include:` fundidos (docs/ANALYZER-ESPECIFICACAO-INFRA.md, III.2.4;
//! reescrito em 2026-10-04 sem compilar nem executar).

use dartforge_diagnostics::{Diagnostic, Severidade};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use yaml_rust2::{Yaml, YamlLoader};

/// O que o `analysis_options.yaml` configura para os diagnósticos
/// (docs/ANALYZER-ESPECIFICACAO-INFRA.md §4.4 a §4.6 e III.2.4): lido com um
/// leitor de YAML de verdade, com os `include:` fundidos.
#[derive(Debug, Default, Clone)]
pub struct Opcoes {
    /// A pasta do arquivo de opções: os globs de `exclude` são relativos a
    /// ela. `None` nas opções feitas de um texto solto.
    pub pasta: Option<PathBuf>,
    /// Globs de `analyzer: exclude:`, relativos ao diretório das opções.
    pub exclude: Vec<String>,
    /// `analyzer: errors:` — código → `None` (ignore) ou a nova severidade.
    pub errors: BTreeMap<String, Option<Severidade>>,
    /// `analyzer: cannot-ignore:` — códigos (minúsculos) e severidades
    /// (`error`, `warning`, `info`) que um `// ignore:` não cala.
    pub nao_ignoraveis: BTreeSet<String>,
    /// `analyzer: language: strict-casts`.
    pub strict_casts: bool,
    /// `analyzer: language: strict-inference`.
    pub strict_inference: bool,
    /// `analyzer: language: strict-raw-types`.
    pub strict_raw_types: bool,
    /// `analyzer: enable-experiment:`.
    pub experimentos: Vec<String>,
    /// `linter: rules:` — regra → ligada (a forma de lista liga cada item).
    pub regras: BTreeMap<String, bool>,
    /// `analyzer: optional-checks: chrome-os-manifest-checks`: liga o
    /// validador do `AndroidManifest.xml`.
    pub manifesto_do_chrome_os: bool,
}

/// Um item `String` de uma lista YAML.
fn textos(no: &Yaml) -> Vec<&str> {
    no.as_vec().map(|l| l.iter().filter_map(Yaml::as_str).collect()).unwrap_or_default()
}

/// A lista só tem strings (`Merger`: candidata a virar mapa de booleanos).
fn lista_de_textos(no: &Yaml) -> bool {
    no.as_vec().is_some_and(|l| l.iter().all(|x| x.as_str().is_some()))
}

/// O mapa só tem valores booleanos.
fn mapa_de_booleanos(no: &Yaml) -> bool {
    no.as_hash().is_some_and(|m| m.iter().all(|(_, v)| v.as_bool().is_some()))
}

/// A lista de strings como mapa `{item: true}`.
fn promover(no: Yaml) -> Yaml {
    match no {
        Yaml::Array(l) => Yaml::Hash(l.into_iter().map(|x| (x, Yaml::Boolean(true))).collect()),
        outro => outro,
    }
}

/// `Merger.merge` (`analyzer/lib/src/util/yaml.dart:23-100`): `base` é o
/// arquivo incluído, `cima` o que inclui (e vence). Mapas fundem-se chave a
/// chave; listas concatenam sem repetir; lista de strings contra mapa de
/// booleanos vira mapa; um escalar de cima vence, salvo se nulo.
fn fundir(base: Yaml, cima: Yaml) -> Yaml {
    let (base, cima) = if lista_de_textos(&base) && mapa_de_booleanos(&cima) {
        (promover(base), cima)
    } else if mapa_de_booleanos(&base) && lista_de_textos(&cima) {
        (base, promover(cima))
    } else {
        (base, cima)
    };
    match (base, cima) {
        (Yaml::Hash(mut b), Yaml::Hash(c)) => {
            for (k, v) in c {
                let novo = match b.remove(&k) {
                    Some(antigo) => fundir(antigo, v),
                    None => v,
                };
                b.insert(k, novo);
            }
            Yaml::Hash(b)
        }
        (Yaml::Array(mut b), Yaml::Array(c)) => {
            for x in c {
                if !b.contains(&x) {
                    b.push(x);
                }
            }
            Yaml::Array(b)
        }
        (b, Yaml::Null | Yaml::BadValue) => b,
        (_, c) => c,
    }
}

/// O mapa YAML de um texto de opções: não-mapa ou erro de YAML → mapa vazio
/// (`getOptionsFromSource`, `analysis_options_provider.dart:81-83`).
fn mapa_do_texto(texto: &str) -> Yaml {
    let vazio = || Yaml::Hash(Default::default());
    match YamlLoader::load_from_str(texto) {
        Ok(docs) => match docs.into_iter().next() {
            Some(m @ Yaml::Hash(_)) => m,
            _ => vazio(),
        },
        Err(_) => vazio(),
    }
}

/// `getOptionsFromSource`: o mapa do arquivo com o `include:` (um escalar,
/// relativo ao arquivo ou `package:`) fundido por baixo, recursivamente.
/// `visitados` corta o ciclo (o original não o detecta na carga).
fn mapa_do_arquivo(arquivo: &Path, pacotes: &dyn Fn(&str) -> Option<PathBuf>, visitados: &mut Vec<PathBuf>) -> Yaml {
    let Ok(texto) = std::fs::read_to_string(arquivo) else { return Yaml::Hash(Default::default()) };
    visitados.push(arquivo.to_path_buf());
    let opcoes = mapa_do_texto(&texto);
    let incluido = opcoes["include"].as_str().and_then(|uri| {
        if uri.starts_with("package:") {
            pacotes(uri)
        } else {
            arquivo.parent().map(|p| p.join(uri))
        }
    });
    match incluido {
        Some(pai) if pai.is_file() && !visitados.contains(&pai) => fundir(mapa_do_arquivo(&pai, pacotes, visitados), opcoes),
        _ => opcoes,
    }
}

impl Opcoes {
    /// Lê `<raiz>/analysis_options.yaml`, se existir, com os `include:`
    /// (os `package:` pelo `package_config.json` acima da raiz).
    pub fn ler(raiz: &Path) -> Opcoes {
        Opcoes::ler_arquivo(&raiz.join("analysis_options.yaml"))
    }

    /// Lê um arquivo de opções com os `include:` fundidos.
    pub fn ler_arquivo(arquivo: &Path) -> Opcoes {
        if !arquivo.is_file() {
            return Opcoes::default();
        }
        let config = dartforge_elements::config::PackageConfig::discover(arquivo)
            .and_then(|p| dartforge_elements::config::PackageConfig::load(&p).ok());
        let pacotes = |uri: &str| config.as_ref().and_then(|c| c.resolve_package_uri(uri).ok());
        let mapa = mapa_do_arquivo(arquivo, &pacotes, &mut Vec::new());
        let mut o = Opcoes::do_mapa(&mapa);
        o.pasta = arquivo.parent().map(Path::to_path_buf);
        o
    }

    /// As opções de um texto solto (sem seguir `include:`).
    pub fn de_texto(texto: &str) -> Opcoes {
        Opcoes::do_mapa(&mapa_do_texto(texto))
    }

    /// `applyOptions` (`apply_options.dart:179-237`) sobre o mapa já fundido.
    fn do_mapa(mapa: &Yaml) -> Opcoes {
        let mut o = Opcoes::default();
        let analyzer = &mapa["analyzer"];
        if let Some(erros) = analyzer["errors"].as_hash() {
            for (k, v) in erros.iter() {
                let (Some(k), Some(v)) = (k.as_str(), v.as_str()) else { continue };
                let sev = match v.to_lowercase().as_str() {
                    "ignore" => None,
                    "info" => Some(Severidade::Info),
                    "warning" => Some(Severidade::Warning),
                    "error" => Some(Severidade::Error),
                    _ => continue,
                };
                o.errors.insert(k.to_lowercase(), sev);
            }
        }
        for g in textos(&analyzer["exclude"]) {
            o.exclude.push(g.to_string());
            // `x/**` também exclui a própria pasta `x`.
            if let Some(pasta) = g.strip_suffix("/**") {
                o.exclude.push(pasta.to_string());
            }
        }
        for n in textos(&analyzer["cannot-ignore"]) {
            o.nao_ignoraveis.insert(n.to_lowercase());
        }
        let linguagem = &analyzer["language"];
        o.strict_casts = linguagem["strict-casts"].as_bool().unwrap_or(false);
        o.strict_inference = linguagem["strict-inference"].as_bool().unwrap_or(false);
        o.strict_raw_types = linguagem["strict-raw-types"].as_bool().unwrap_or(false);
        o.experimentos = textos(&analyzer["enable-experiment"]).into_iter().map(str::to_string).collect();
        let checagens = &analyzer["optional-checks"];
        o.manifesto_do_chrome_os = checagens.as_str() == Some("chrome-os-manifest-checks")
            || checagens["chrome-os-manifest-checks"].as_bool().unwrap_or(false);
        let regras = &mapa["linter"]["rules"];
        for r in textos(regras) {
            o.regras.insert(r.to_string(), true);
        }
        if let Some(m) = regras.as_hash() {
            for (k, v) in m.iter() {
                if let (Some(k), Some(v)) = (k.as_str(), v.as_bool()) {
                    o.regras.insert(k.to_string(), v);
                }
            }
        }
        o
    }

    /// O arquivo de opções mais próximo de `arquivo`, subindo, estritamente
    /// abaixo de `raiz` (§4.5: as opções de subpasta). `None` quando o que
    /// vale é o da raiz.
    pub fn de_subpasta(arquivo: &Path, raiz: &Path) -> Option<PathBuf> {
        let mut pasta = arquivo.parent();
        while let Some(p) = pasta {
            if p == raiz || !p.starts_with(raiz) {
                return None;
            }
            let candidato = p.join("analysis_options.yaml");
            if candidato.is_file() {
                return Some(candidato);
            }
            pasta = p.parent();
        }
        None
    }

    /// O arquivo (caminho absoluto) é excluído pelos globs destas opções,
    /// relativos à pasta delas.
    pub fn exclui_arquivo(&self, arquivo: &Path) -> bool {
        let Some(pasta) = &self.pasta else { return false };
        let Ok(rel) = arquivo.strip_prefix(pasta) else { return false };
        let rel = rel.to_string_lossy().replace('\\', "/");
        self.exclude.iter().any(|g| glob(g, &rel))
    }

    /// Um `// ignore:` pode calar `d`? Não, se o código estiver em
    /// `cannot-ignore`, ou a severidade dele (a de `errors:`, se mudada ali;
    /// senão a padrão) estiver (`AnalysisOptionsImpl.unignorableNames`).
    pub fn ignoravel(&self, d: &Diagnostic) -> bool {
        if self.nao_ignoraveis.is_empty() {
            return true;
        }
        let Some(c) = d.code else { return true };
        let info = c.info();
        if self.nao_ignoraveis.contains(info.nome) {
            return false;
        }
        let nome_da = |s: Severidade| match s {
            Severidade::Error => "error",
            Severidade::Warning => "warning",
            Severidade::Info => "info",
        };
        // `applyUnignorables` (docs/ANALYZER-ESPECIFICACAO-INFRA.md §4.3,
        // fato c): o código entra pela severidade que `errors:` lhe deu e
        // também pela padrão (o segundo `if` roda sempre que o primeiro não
        // deu `continue`): o erro rebaixado a `warning` continua não
        // ignorável com `cannot-ignore: [error]`.
        if let Some(Some(s)) = self.errors.get(info.nome)
            && self.nao_ignoraveis.contains(nome_da(*s))
        {
            return false;
        }
        !self.nao_ignoraveis.contains(nome_da(info.severidade))
    }

    /// O lint `nome` (de nome único `unico`) pode ser calado por
    /// `// ignore:`: os lints só entram no `cannot-ignore` pelo nome, nunca
    /// pela severidade (§4.3, fato d), e a comparação é sem caixa
    /// (`code.name.toUpperCase()`).
    pub fn lint_ignoravel(&self, nome: &str, unico: &str) -> bool {
        !(self.nao_ignoraveis.contains(&nome.to_lowercase()) || self.nao_ignoraveis.contains(&unico.to_lowercase()))
    }

    /// `rel` (relativo à raiz, com `/`) está fora da análise: casa algum
    /// `exclude`, ou algum componente do caminho começa com `.`
    /// (`ContextRootImpl._isExcluded`, passos 1 e 3).
    pub fn excluido(&self, rel: &str) -> bool {
        rel.split('/').any(|c| c.starts_with('.') && c != "." && c != "..") || self.exclude.iter().any(|g| glob(g, rel))
    }

    /// Aplica `errors:` a um diagnóstico: `None` se ignorado.
    pub fn processar(&self, mut d: Diagnostic) -> Option<Diagnostic> {
        let Some(c) = d.code else { return Some(d) };
        match self.errors.get(c.info().nome) {
            None => Some(d),
            Some(None) => None,
            Some(Some(s)) => {
                d.severity = *s;
                Some(d)
            }
        }
    }
}

/// O fecho de uma construção do glob que começa em `p[0]` (`[` ou `{`):
/// o índice do delimitador que fecha, pulando escapes e aninhamento.
fn fecho(p: &[u8], abre: u8, fecha: u8) -> Option<usize> {
    let mut nivel = 0usize;
    let mut i = 0;
    while i < p.len() {
        match p[i] {
            b'\\' => i += 1,
            c if c == abre => nivel += 1,
            c if c == fecha => {
                nivel -= 1;
                if nivel == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Glob do `package:glob`: `**` casa qualquer sequência (inclusive `/`),
/// `*` qualquer sequência sem `/`, `?` um caractere, `[abc]`, `[a-z]` e
/// `[!abc]`/`[^abc]` um caractere da classe (nunca `/`), `{a,b}` uma das
/// alternativas, e `\` tira o significado do caractere seguinte.
pub fn glob(padrao: &str, texto: &str) -> bool {
    fn casa(p: &[u8], t: &[u8]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        if p.starts_with(b"**") {
            let resto = p[2..].strip_prefix(b"/").unwrap_or(&p[2..]);
            return (0..=t.len()).any(|i| casa(resto, &t[i..])) || casa(&p[2..], t);
        }
        match p[0] {
            b'*' => (0..=t.len()).take_while(|&i| i == 0 || t[i - 1] != b'/').any(|i| casa(&p[1..], &t[i..])),
            b'?' => !t.is_empty() && t[0] != b'/' && casa(&p[1..], &t[1..]),
            b'\\' if p.len() > 1 => !t.is_empty() && t[0] == p[1] && casa(&p[2..], &t[1..]),
            b'[' => {
                let Some(fim) = fecho(p, b'[', b']') else { return !t.is_empty() && t[0] == b'[' && casa(&p[1..], &t[1..]) };
                if t.is_empty() || t[0] == b'/' {
                    return false;
                }
                let mut classe = &p[1..fim];
                let negada = matches!(classe.first(), Some(b'!' | b'^'));
                if negada {
                    classe = &classe[1..];
                }
                let mut dentro = false;
                let mut i = 0;
                while i < classe.len() {
                    let mut c = classe[i];
                    if c == b'\\' && i + 1 < classe.len() {
                        i += 1;
                        c = classe[i];
                    }
                    if i + 2 < classe.len() && classe[i + 1] == b'-' {
                        dentro |= c <= t[0] && t[0] <= classe[i + 2];
                        i += 3;
                    } else {
                        dentro |= c == t[0];
                        i += 1;
                    }
                }
                dentro != negada && casa(&p[fim + 1..], &t[1..])
            }
            b'{' => {
                let Some(fim) = fecho(p, b'{', b'}') else { return !t.is_empty() && t[0] == b'{' && casa(&p[1..], &t[1..]) };
                // As alternativas: separadas pelas vírgulas do nível de fora.
                let miolo = &p[1..fim];
                let mut alternativas: Vec<&[u8]> = Vec::new();
                let (mut nivel, mut inicio, mut i) = (0usize, 0usize, 0usize);
                while i < miolo.len() {
                    match miolo[i] {
                        b'\\' => i += 1,
                        b'{' => nivel += 1,
                        b'}' => nivel = nivel.saturating_sub(1),
                        b',' if nivel == 0 => {
                            alternativas.push(&miolo[inicio..i]);
                            inicio = i + 1;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                alternativas.push(&miolo[inicio..]);
                alternativas.iter().any(|a| {
                    let mut junto = a.to_vec();
                    junto.extend_from_slice(&p[fim + 1..]);
                    casa(&junto, t)
                })
            }
            c => !t.is_empty() && t[0] == c && casa(&p[1..], &t[1..]),
        }
    }
    casa(padrao.as_bytes(), texto.as_bytes())
}

/// Um elemento da lista de um comentário `ignore` (`IgnoredElement`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ignorado {
    /// Um nome de diagnóstico, em minúsculas, e o offset dele na fonte.
    Nome { nome: String, offset: usize },
    /// `type=…`, em minúsculas.
    Tipo { tipo: String },
}

/// Os `// ignore:` de um arquivo, como o `IgnoreInfo` do analyzer
/// (`analyzer/lib/src/ignore_comments/ignore_info.dart`;
/// docs/ANALYZER-ESPECIFICACAO-INFRA.md §4.1): por linha (1-based) e para o
/// arquivo todo. Os comentários vêm de uma varredura da fonte que conhece
/// strings e comentários de bloco — um `//` dentro de uma string não é
/// comentário.
#[derive(Debug, Default)]
pub struct Ignorados {
    por_linha: BTreeMap<usize, Vec<Ignorado>>,
    arquivo: Vec<Ignorado>,
    /// Os nomes repetidos (`IgnoreValidator`, `duplicate_ignore`): o nome e
    /// o intervalo da repetição.
    repetidos: Vec<(String, dartforge_diagnostics::Span)>,
}

/// Os comentários de linha (`//…`) da fonte: o offset do `//` e o texto até
/// o fim da linha. Pula strings (simples, de três aspas e cruas, com as
/// interpolações `${…}`) e comentários de bloco aninhados.
fn comentarios_de_linha(fonte: &str) -> Vec<(usize, &str)> {
    let b = fonte.as_bytes();
    let n = b.len();
    let mut saida = Vec::new();
    // A pilha de contextos: `None` é código (o `usize` das strings é a
    // profundidade de chaves do código dentro de uma interpolação).
    #[derive(Clone, Copy)]
    enum Ctx {
        /// Código; `chaves` conta os `{` abertos desde a interpolação que o
        /// abriu (no topo do arquivo, nunca fecha).
        Codigo { chaves: u32, de_interpolacao: bool },
        /// Dentro de uma string: o delimitador, se é de três, se é crua.
        Texto { aspa: u8, tripla: bool, crua: bool },
    }
    let mut pilha: Vec<Ctx> = vec![Ctx::Codigo { chaves: 0, de_interpolacao: false }];
    let mut i = 0;
    while i < n {
        let Some(topo) = pilha.last().copied() else { break };
        match topo {
            Ctx::Codigo { chaves, de_interpolacao } => {
                let ch = b[i];
                if ch == b'/' && b.get(i + 1) == Some(&b'/') {
                    let fim = fonte[i..].find('\n').map_or(n, |k| i + k);
                    saida.push((i, fonte[i..fim].trim_end_matches('\r')));
                    i = fim;
                } else if ch == b'/' && b.get(i + 1) == Some(&b'*') {
                    let mut nivel = 1;
                    i += 2;
                    while i < n && nivel > 0 {
                        if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                            nivel += 1;
                            i += 2;
                        } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                            nivel -= 1;
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                } else if ch == b'\'' || ch == b'"' {
                    let crua = i > 0 && b[i - 1] == b'r' && (i < 2 || !(b[i - 2].is_ascii_alphanumeric() || b[i - 2] == b'_' || b[i - 2] == b'$'));
                    let tripla = b.get(i + 1) == Some(&ch) && b.get(i + 2) == Some(&ch);
                    pilha.push(Ctx::Texto { aspa: ch, tripla, crua });
                    i += if tripla { 3 } else { 1 };
                } else if ch == b'{' {
                    if let Some(Ctx::Codigo { chaves, .. }) = pilha.last_mut() {
                        *chaves += 1;
                    }
                    i += 1;
                } else if ch == b'}' {
                    if de_interpolacao && chaves == 0 {
                        // Fecha a interpolação: volta à string.
                        pilha.pop();
                    } else if let Some(Ctx::Codigo { chaves, .. }) = pilha.last_mut() {
                        *chaves = chaves.saturating_sub(1);
                    }
                    i += 1;
                } else {
                    i += 1;
                }
            }
            Ctx::Texto { aspa, tripla, crua } => {
                let ch = b[i];
                if !crua && ch == b'\\' {
                    i += 2;
                } else if !crua && ch == b'$' && b.get(i + 1) == Some(&b'{') {
                    pilha.push(Ctx::Codigo { chaves: 0, de_interpolacao: true });
                    i += 2;
                } else if ch == aspa && (!tripla || (b.get(i + 1) == Some(&aspa) && b.get(i + 2) == Some(&aspa))) {
                    pilha.pop();
                    i += if tripla { 3 } else { 1 };
                } else if !tripla && ch == b'\n' {
                    // String de uma linha sem fecho: o erro é do lexer; o
                    // resto do arquivo volta a ser código.
                    pilha.pop();
                    i += 1;
                } else {
                    i += 1;
                }
            }
        }
    }
    saida
}

/// `CommentTokenExtension.ignoredElements` (`ignore_info.dart:202-330`): os
/// elementos da lista depois do PRIMEIRO `:` do comentário. `base` é o
/// offset do comentário na fonte.
fn elementos_do_comentario(lexema: &str, base: usize) -> Vec<Ignorado> {
    let b = lexema.as_bytes();
    let n = b.len();
    let mut saida = Vec::new();
    let Some(dois_pontos) = lexema.find(':') else { return saida };
    let mut i = dois_pontos + 1;
    let pular_brancos = |i: &mut usize| {
        while *i < n && (b[*i] == b' ' || b[*i] == b'\t') {
            *i += 1;
        }
    };
    // Uma palavra: começa com letra, segue com letra, dígito ou `_`.
    let palavra = |i: &mut usize| -> Option<(usize, usize)> {
        let ini = *i;
        if *i < n && b[*i].is_ascii_alphabetic() {
            *i += 1;
            while *i < n && (b[*i].is_ascii_alphanumeric() || b[*i] == b'_') {
                *i += 1;
            }
            Some((ini, *i))
        } else {
            None
        }
    };
    loop {
        pular_brancos(&mut i);
        if i >= n {
            return saida;
        }
        let Some((ini, fim)) = palavra(&mut i) else { return saida };
        let texto = lexema[ini..fim].to_lowercase();
        if texto == "type" {
            pular_brancos(&mut i);
            if i >= n || b[i] != b'=' {
                return saida;
            }
            i += 1;
            pular_brancos(&mut i);
            let Some((tini, tfim)) = palavra(&mut i) else { return saida };
            if i < n && !(b[i] == b' ' || b[i] == b'\t' || b[i] == b',') {
                return saida;
            }
            saida.push(Ignorado::Tipo { tipo: lexema[tini..tfim].to_lowercase() });
        } else {
            // Um nome seguido de caractere estranho (`ignore: http://x`,
            // `nome.`) encerra a lista sem incluí-lo.
            if i < n && !(b[i] == b' ' || b[i] == b'\t' || b[i] == b',') {
                return saida;
            }
            saida.push(Ignorado::Nome { nome: texto, offset: base + ini });
        }
        pular_brancos(&mut i);
        if i >= n || b[i] != b',' {
            // Fim, ou texto livre depois do último nome.
            return saida;
        }
        i += 1;
    }
}

impl Ignorados {
    pub fn de_texto(texto: &str) -> Ignorados {
        let mut ig = Ignorados::default();
        // O começo de cada linha, para achar a linha de um offset.
        let mut inicios: Vec<usize> = vec![0];
        inicios.extend(texto.bytes().enumerate().filter(|(_, c)| *c == b'\n').map(|(i, _)| i + 1));
        let mut do_arquivo_bruto: Vec<Ignorado> = Vec::new();
        let mut de_linha_bruto: Vec<(usize, Vec<Ignorado>)> = Vec::new();
        for (offset, lexema) in comentarios_de_linha(texto) {
            // `//+[ ]*ignore:` (duas ou mais barras) ou `//[ ]*ignore_for_file:`
            // (exatamente duas): só o COMEÇO do comentário é testado.
            let depois_das_barras = lexema.trim_start_matches('/');
            let barras = lexema.len() - depois_das_barras.len();
            let corpo = depois_das_barras.trim_start_matches(' ');
            let e_ignore = corpo.starts_with("ignore:");
            let e_do_arquivo = barras == 2 && corpo.starts_with("ignore_for_file:");
            if !e_ignore && !e_do_arquivo {
                continue;
            }
            let elementos = elementos_do_comentario(lexema, offset);
            // `forDart`: o teste de `ignore:` vem antes — um `ignore_for_file`
            // cujo texto contém `ignore:` adiante vira ignore de linha.
            if lexema.contains("ignore:") {
                let linha0 = inicios.partition_point(|&p| p <= offset) - 1;
                let antes = &texto[inicios[linha0]..offset];
                // Sozinho na linha: vale para a seguinte; depois de código,
                // para a própria.
                let alvo = if antes.trim().is_empty() { linha0 + 2 } else { linha0 + 1 };
                de_linha_bruto.push((alvo, elementos));
            } else if lexema.contains("ignore_for_file:") {
                do_arquivo_bruto.extend(elementos);
            }
        }
        // `IgnoreValidator`: um nome repetido no próprio comentário de
        // arquivo, e depois um nome de linha repetido ou já coberto pelo
        // arquivo. O relato é no nome repetido.
        let nome_de = |e: &Ignorado| match e {
            Ignorado::Nome { nome, offset } => Some((nome.clone(), *offset)),
            Ignorado::Tipo { .. } => None,
        };
        let mut vistos_no_arquivo: BTreeSet<String> = BTreeSet::new();
        for e in &do_arquivo_bruto {
            if let Some((nome, offset)) = nome_de(e) {
                if !vistos_no_arquivo.insert(nome.clone()) {
                    ig.repetidos.push((nome.clone(), dartforge_diagnostics::Span { start: offset, end: offset + nome.len() }));
                }
            }
        }
        for (_, elementos) in &de_linha_bruto {
            let mut vistos_na_linha: BTreeSet<String> = BTreeSet::new();
            for e in elementos {
                if let Some((nome, offset)) = nome_de(e) {
                    if vistos_no_arquivo.contains(&nome) || !vistos_na_linha.insert(nome.clone()) {
                        ig.repetidos.push((nome.clone(), dartforge_diagnostics::Span { start: offset, end: offset + nome.len() }));
                    }
                }
            }
        }
        ig.arquivo = do_arquivo_bruto;
        for (linha, elementos) in de_linha_bruto {
            ig.por_linha.entry(linha).or_default().extend(elementos);
        }
        ig
    }

    /// Os `duplicate_ignore` do arquivo (`IgnoreValidator`: no 3.6.2 é o
    /// único relato dele).
    pub fn duplicados(&self) -> Vec<Diagnostic> {
        self.repetidos
            .iter()
            .map(|(nome, span)| Diagnostic::com_codigo(dartforge_diagnostics::codigos::warning::DUPLICATE_IGNORE, *span, [nome.as_str()]))
            .collect()
    }

    /// O diagnóstico na `linha` é ignorado por comentário?
    ///
    /// Qualquer severidade: o `LibraryAnalyzer._filterIgnoredErrors` do
    /// analyzer 6.11.0 não consulta `ErrorCode.isIgnorable` (que nega
    /// `ERROR`); só o `cannot-ignore` das opções impede
    /// ([`Opcoes::ignoravel`]). Conferido no `dart analyze` 3.6.2 e 3.13.4:
    /// `// ignore_for_file: uri_has_not_been_generated` cala o erro.
    ///
    /// Um nome casa com o nome emitido do código ou com o nome único sem o
    /// prefixo da classe (`IgnoredDiagnosticName.matches`); `type=` só
    /// conhece `hint`, `lint` e `warning` (este, o `STATIC_WARNING`).
    pub fn ignora(&self, d: &Diagnostic, linha: usize) -> bool {
        let Some(c) = d.code else { return false };
        let info = c.info();
        // As formas 3.13.4 do catálogo levam o sufixo `_3_13` só para
        // distinguir a entrada; o nome único do analyzer não o tem.
        let unico = info
            .unico
            .split_once('.')
            .map_or(String::new(), |(_, r)| r.strip_suffix("_3_13").unwrap_or(r).to_lowercase());
        let tipo_do_codigo = match info.tipo {
            dartforge_diagnostics::TipoErro::Hint => "hint",
            dartforge_diagnostics::TipoErro::Lint => "lint",
            dartforge_diagnostics::TipoErro::StaticWarning => "warning",
            _ => "",
        };
        let casa = |lista: &Vec<Ignorado>| {
            lista.iter().any(|e| match e {
                Ignorado::Nome { nome, .. } => nome == info.nome || *nome == unico,
                Ignorado::Tipo { tipo } => !tipo_do_codigo.is_empty() && tipo == tipo_do_codigo,
            })
        };
        casa(&self.arquivo) || self.por_linha.get(&linha).is_some_and(casa)
    }

    /// O comentário `ignore` cala o lint de nome `nome` (o da regra) e nome
    /// único `unico` na linha `linha`? Vale também `type=lint`.
    pub fn ignora_lint(&self, nome: &str, unico: &str, linha: usize) -> bool {
        let casa = |lista: &Vec<Ignorado>| {
            lista.iter().any(|e| match e {
                Ignorado::Nome { nome: n, .. } => n == nome || n == unico,
                Ignorado::Tipo { tipo } => tipo == "lint",
            })
        };
        casa(&self.arquivo) || self.por_linha.get(&linha).is_some_and(casa)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn opcoes_do_new_sali() {
        let o = Opcoes::de_texto(
            "analyzer:\n  exclude:\n    - build/**\n    - 'lib/**.g.dart'\n  errors:\n    uri_has_not_been_generated: ignore\n    deprecated_member_use: ignore # x\n    dead_code: info\n",
        );
        assert_eq!(o.exclude, vec!["build/**", "build", "lib/**.g.dart"]);
        assert_eq!(o.errors.get("uri_has_not_been_generated"), Some(&None));
        assert_eq!(o.errors.get("dead_code"), Some(&Some(Severidade::Info)));
        assert!(o.excluido("build/x/y.dart"));
        assert!(o.excluido("lib/a/b.g.dart"));
        assert!(!o.excluido("lib/a/b.dart"));
    }

    /// YAML de verdade: fluxo, aspas, comentário, `language:`, lints nas
    /// duas formas, e o que `errors:` aceita.
    #[test]
    fn yaml_de_verdade() {
        let o = Opcoes::de_texto(
            "analyzer: {exclude: ['gen/**', \"x # y.dart\"], language: {strict-casts: true}}
linter:
  rules:
    - a_regra
",
        );
        assert_eq!(o.exclude, vec!["gen/**", "gen", "x # y.dart"]);
        assert!(o.strict_casts && !o.strict_inference);
        assert_eq!(o.regras.get("a_regra"), Some(&true));
        let mapa = Opcoes::de_texto("linter:
  rules:
    a_regra: false
");
        assert_eq!(mapa.regras.get("a_regra"), Some(&false));
        // Um YAML inválido deixa as opções padrão.
        assert!(Opcoes::de_texto("analyzer: [
").exclude.is_empty());
        // Pastas e arquivos ocultos ficam fora.
        assert!(Opcoes::default().excluido(".dart_tool/x.dart"));
        assert!(Opcoes::default().excluido("lib/.oculto.dart"));
    }

    /// `Merger`: o incluidor vence; listas concatenam sem repetir; lista de
    /// lints contra mapa de booleanos vira mapa.
    #[test]
    fn fusao_do_include() {
        let base = mapa_do_texto("analyzer:
  exclude: [a, b]
  errors:
    dead_code: info
    todo: ignore
linter:
  rules: [r1, r2]
");
        let cima = mapa_do_texto("analyzer:
  exclude: [b, c]
  errors:
    dead_code: error
linter:
  rules:
    r2: false
");
        let o = Opcoes::do_mapa(&fundir(base, cima));
        assert_eq!(o.exclude, vec!["a", "b", "c"]);
        assert_eq!(o.errors.get("dead_code"), Some(&Some(Severidade::Error)));
        assert_eq!(o.errors.get("todo"), Some(&None));
        assert_eq!(o.regras.get("r1"), Some(&true));
        assert_eq!(o.regras.get("r2"), Some(&false));
    }

    /// `// ignore:` e `// ignore_for_file:` valem para erro também
    /// (`v01/min/ignora`: o `dart analyze` 3.6.2 e o 3.13.4 dão "No issues
    /// found!"), salvo o que `cannot-ignore` lista.
    #[test]
    fn ignore_vale_para_erro_salvo_cannot_ignore() {
        use dartforge_diagnostics::codigos::compile_time_error as c;
        let d = Diagnostic::com_codigo(c::URI_HAS_NOT_BEEN_GENERATED, dartforge_diagnostics::Span { start: 0, end: 1 }, ["a.template.dart"]);
        let arquivo = Ignorados::de_texto("// ignore_for_file: uri_has_not_been_generated\nimport 'a.template.dart';\n");
        assert!(arquivo.ignora(&d, 2));
        let linha = Ignorados::de_texto("// ignore: uri_has_not_been_generated\nimport 'a.template.dart';\n");
        assert!(linha.ignora(&d, 2));
        assert!(!linha.ignora(&d, 3));
        assert!(Opcoes::default().ignoravel(&d));
        let por_nome = Opcoes::de_texto("analyzer:\n  cannot-ignore:\n    - uri_has_not_been_generated\n");
        assert!(!por_nome.ignoravel(&d));
        let por_severidade = Opcoes::de_texto("analyzer:\n  cannot-ignore:\n    - error\n");
        assert!(!por_severidade.ignoravel(&d));
        // O rebaixado continua não ignorável pela severidade padrão e passa a
        // sê-lo também pela nova (conferido no binário 3.6.2, §4.3 c).
        let rebaixado = Opcoes::de_texto("analyzer:\n  errors:\n    uri_has_not_been_generated: warning\n  cannot-ignore:\n    - error\n");
        assert!(!rebaixado.ignoravel(&d));
        let pela_nova = Opcoes::de_texto("analyzer:\n  errors:\n    uri_has_not_been_generated: warning\n  cannot-ignore:\n    - warning\n");
        assert!(!pela_nova.ignoravel(&d));
        let outra = Opcoes::de_texto("analyzer:\n  cannot-ignore:\n    - info\n");
        assert!(outra.ignoravel(&d));
        // Os lints, só pelo nome.
        let lint = Opcoes::de_texto("analyzer:\n  cannot-ignore:\n    - avoid_print\n    - info\n");
        assert!(!lint.lint_ignoravel("avoid_print", "avoid_print"));
        assert!(lint.lint_ignoravel("empty_catches", "empty_catches"));
    }

    /// A tabela conferida no binário 3.6.2 (docs/ANALYZER-ESPECIFICACAO-INFRA.md
    /// §4.1): cada comentário e se ele cala o `unused_local_variable` da
    /// linha 2.
    #[test]
    fn tabela_do_oraculo() {
        use dartforge_diagnostics::codigos::warning as w;
        let d = Diagnostic::com_codigo(w::UNUSED_LOCAL_VARIABLE, dartforge_diagnostics::Span { start: 0, end: 1 }, ["x"]);
        let cala = |texto: &str, linha: usize| Ignorados::de_texto(texto).ignora(&d, linha);
        assert!(cala("// ignore: type=warning
var x = 1;
", 2));
        assert!(!cala("// ignore: type=static_warning
var x = 1;
", 2));
        assert!(cala("///ignore: unused_local_variable
var x = 1;
", 2));
        assert!(!cala("//	ignore: unused_local_variable
var x = 1;
", 2));
        assert!(!cala("// ignore_for_file: dead_code // ignore: unused_local_variable
var x = 1;
", 2));
        assert!(!cala("var f = 1; /* ignore: unused_local_variable */
", 1));
        assert!(cala("var s = 'http://x'; // ignore: unused_local_variable
", 1));
        assert!(!cala("// ignore: unused_local_variable porque sim

var h = 1;
", 3));
        assert!(!cala("// ignore: unused_local_variable.
var x = 1;
", 2));
        assert!(cala("// ignore: UNUSED_LOCAL_VARIABLE , dead_code
var x = 1;
", 2));
    }

    #[test]
    fn nomes_repetidos() {
        // 0         1         2         3         4
        // 01234567890123456789012345678901234567890123456789
        // // ignore: dead_code, dead_code
        let ig = Ignorados::de_texto("// ignore: dead_code, dead_code
var x = 1;
");
        let ds = ig.duplicados();
        assert_eq!(ds.len(), 1);
        assert_eq!((ds[0].span.start, ds[0].span.end), (22, 31));
        let ig = Ignorados::de_texto("// ignore_for_file: dead_code
// ignore: dead_code
var x = 1;
");
        assert_eq!(ig.duplicados().len(), 1);
        assert!(Ignorados::de_texto("// ignore: a, dead_code
var x = 1;
").duplicados().is_empty());
    }

    #[test]
    fn glob_basico() {
        assert!(glob("*.dart", "a.dart"));
        // Chaves, classes de caractere e escape (`package:glob`).
        assert!(glob("lib/{a,b}/*.dart", "lib/b/x.dart"));
        assert!(!glob("lib/{a,b}/*.dart", "lib/c/x.dart"));
        assert!(glob("lib/[a-c]?.dart", "lib/bz.dart"));
        assert!(!glob("lib/[!a-c]?.dart", "lib/bz.dart"));
        assert!(glob("a\\*b", "a*b"));
        assert!(!glob("a\\*b", "axb"));
        assert!(!glob("*.dart", "x/a.dart"));
        assert!(glob("**/*.dart", "x/y/a.dart"));
        assert!(glob("**", "x/y"));
    }
}
