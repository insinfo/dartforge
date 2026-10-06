//! As regras de lint do `pubspec.yaml` (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), escritas direto dos emissores da 3.6.2
//! (`linter/lib/src/rules/pub/`), sobre o modelo `Pubspec` do analyzer
//! (`analyzer/lib/src/lint/pub.dart`): `package_names`,
//! `secure_pubspec_urls` e `sort_pub_dependencies`; e o que o
//! `depend_on_referenced_packages` lê do `pubspec.yaml` do pacote.
//!
//! O `Pubspec.parseYaml` só lê um documento que é mapa; as chaves são as
//! escalares, e a última ocorrência vale. O texto de um nó é o `toString` do
//! valor do escalar (números e booleanos como escritos; nulo não tem). O
//! `IgnoreInfo.forYaml` (`# ignore:` e `# ignore_for_file:`) está em
//! [`IgnoradosYaml`], que vale também para o `PubspecValidator`.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::naodart::yaml::{self, No, Valor};
use dartforge_diagnostics::Span;

/// `_PSNode.text`.
fn texto(no: &No) -> Option<String> {
    match &no.valor {
        Valor::Texto(t) | Valor::Outro(t) => Some(t.clone()),
        _ => None,
    }
}

/// O texto de uma chave escalar (`key.toString()`).
fn chave(no: &No) -> Option<String> {
    if no.escalar() { texto(no).or_else(|| no.nulo().then(|| "null".to_string())) } else { None }
}

/// `_findEntry(map, key)`: o valor escalar da última chave igual.
fn entrada<'a>(mapa: &'a No, nome: &str) -> Option<&'a No> {
    let mut achado = None;
    for (k, v) in mapa.mapa()? {
        if chave(k).as_deref() == Some(nome) {
            achado = Some(v).filter(|v| v.escalar());
        }
    }
    achado
}

/// Uma dependência (`_PSDependency`).
pub struct Dependencia<'a> {
    pub nome: &'a No,
    /// O `url` do `git` e o do `hosted`.
    pub urls: Vec<&'a No>,
}

/// O `pubspec.yaml` no modelo do linter.
#[derive(Default)]
pub struct Pubspec<'a> {
    pub nome: Option<&'a No>,
    pub documentation: Option<&'a No>,
    pub homepage: Option<&'a No>,
    pub issue_tracker: Option<&'a No>,
    pub repository: Option<&'a No>,
    pub dependencies: Option<Vec<Dependencia<'a>>>,
    pub dev_dependencies: Option<Vec<Dependencia<'a>>>,
    pub dependency_overrides: Option<Vec<Dependencia<'a>>>,
}

/// `_processDependencies`.
fn dependencias(v: &No) -> Option<Vec<Dependencia<'_>>> {
    let pares = v.mapa()?;
    let mut lista = Vec::new();
    for (k, valor) in pares {
        if !k.escalar() {
            continue;
        }
        let mut git: Option<&No> = None;
        let mut hosted: Option<&No> = None;
        if let Some(detalhes) = valor.mapa() {
            for (dk, dv) in detalhes {
                match chave(dk).as_deref() {
                    Some("hosted") => {
                        hosted = if dv.escalar() { Some(dv) } else if dv.mapa().is_some() { entrada(dv, "url") } else { None };
                    }
                    Some("git") => {
                        git = if dv.escalar() { Some(dv) } else if dv.mapa().is_some() { entrada(dv, "url") } else { None };
                    }
                    _ => {}
                }
            }
        }
        lista.push(Dependencia { nome: k, urls: git.into_iter().chain(hosted).collect() });
    }
    Some(lista)
}

/// `Pubspec.parseYaml`.
pub fn ler(raiz: &No) -> Pubspec<'_> {
    let mut p = Pubspec::default();
    let Some(pares) = raiz.mapa() else { return p };
    for (k, v) in pares {
        if !k.escalar() {
            continue;
        }
        let escalar = || Some(v).filter(|v| v.escalar());
        match chave(k).as_deref() {
            Some("homepage") => p.homepage = escalar(),
            Some("repository") => p.repository = escalar(),
            Some("issue_tracker") => p.issue_tracker = escalar(),
            Some("name") => p.nome = escalar(),
            Some("documentation") => p.documentation = escalar(),
            Some("dependencies") => p.dependencies = dependencias(v),
            Some("dev_dependencies") => p.dev_dependencies = dependencias(v),
            Some("dependency_overrides") => p.dependency_overrides = dependencias(v),
            _ => {}
        }
    }
    p
}

/// `^[_$a-z]+(\d[_a-z\d]*)?$` sem caixa.
fn e_identificador(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i] == b'_' || b[i] == b'$' || b[i].is_ascii_alphabetic()) {
        i += 1;
    }
    if i == 0 {
        return false;
    }
    if i == b.len() {
        return true;
    }
    if !b[i].is_ascii_digit() {
        return false;
    }
    b[i + 1..].iter().all(|c| *c == b'_' || c.is_ascii_alphanumeric())
}

/// As palavras reservadas do `Keyword` (`isReservedWord`).
const RESERVADAS: [&str; 33] = [
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else", "enum", "extends", "false", "final", "finally",
    "for", "if", "in", "is", "new", "null", "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while",
    "with",
];

/// `isValidPackageName`: `^_*[a-z](?:_?[a-z\d])*$`, identificador e não
/// reservada.
fn nome_de_pacote_valido(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i] == b'_' {
        i += 1;
    }
    if i >= b.len() || !b[i].is_ascii_lowercase() {
        return false;
    }
    i += 1;
    while i < b.len() {
        if b[i] == b'_' {
            i += 1;
            if i >= b.len() {
                return false;
            }
        }
        if !(b[i].is_ascii_lowercase() || b[i].is_ascii_digit()) {
            return false;
        }
        i += 1;
    }
    e_identificador(s) && !RESERVADAS.contains(&s)
}

/// O esquema de `Uri.tryParse(texto)`, em minúsculas.
fn esquema(t: &str) -> Option<String> {
    let fim = t.find(|c: char| matches!(c, ':' | '/' | '?' | '#'))?;
    if t.as_bytes()[fim] != b':' || fim == 0 {
        return None;
    }
    let s = &t[..fim];
    let b = s.as_bytes();
    if !b[0].is_ascii_alphabetic() || !b.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.')) {
        return None;
    }
    Some(s.to_ascii_lowercase())
}

/// Os lints do `pubspec.yaml` com o texto `fonte`.
pub fn executar(fonte: &str, ligada: &dyn Fn(&str) -> bool) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Ok(Some(raiz)) = yaml::ler(fonte) else { return out };
    let p = ler(&raiz);
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let listas = [&p.dependencies, &p.dev_dependencies, &p.dependency_overrides];

    // `package_names`.
    if ligada("package_names")
        && let Some(n) = p.nome
        && let Some(t) = texto(n)
        && !nome_de_pacote_valido(&t)
    {
        relatar(&c::PACKAGE_NAMES, n.span, &[t.as_str()]);
    }

    // `secure_pubspec_urls`: na ordem do `accept` (documentation,
    // homepage, issue_tracker, repository; depois as dependências).
    if ligada("secure_pubspec_urls") {
        let mut checar = |no: &No| {
            if let Some(t) = texto(no)
                && let Some(s) = esquema(&t)
                && (s == "http" || s == "git")
            {
                relatar(&c::SECURE_PUBSPEC_URLS, no.span, &[s.as_str()]);
            }
        };
        for no in [p.documentation, p.homepage, p.issue_tracker, p.repository].into_iter().flatten() {
            checar(no);
        }
        for lista in listas.iter().copied().flatten() {
            for d in lista {
                for u in &d.urls {
                    checar(u);
                }
            }
        }
    }

    // `sort_pub_dependencies`: a primeira fora de ordem de cada lista.
    if ligada("sort_pub_dependencies") {
        for lista in listas.iter().copied().flatten() {
            let mut por_posicao: Vec<&Dependencia<'_>> = lista.iter().collect();
            por_posicao.sort_by_key(|d| d.nome.span.start);
            let mut anterior: Vec<u16> = Vec::new();
            for d in por_posicao {
                let Some(t) = texto(d.nome) else { continue };
                let unidades: Vec<u16> = t.encode_utf16().collect();
                if unidades < anterior {
                    relatar(&c::SORT_PUB_DEPENDENCIES, d.nome.span, &[]);
                    break;
                }
                anterior = unidades;
            }
        }
    }
    out
}

/// O que o `depend_on_referenced_packages` lê: o nome do pacote, as
/// dependências e as de desenvolvimento.
pub fn dependencias_disponiveis(fonte: &str, com_dev: bool) -> Option<Vec<String>> {
    let raiz = yaml::ler(fonte).ok()??;
    let p = ler(&raiz);
    let mut v = vec![texto(p.nome?)?];
    for d in p.dependencies.iter().flatten() {
        v.extend(texto(d.nome));
    }
    if com_dev {
        for d in p.dev_dependencies.iter().flatten() {
            v.extend(texto(d.nome));
        }
    }
    Some(v)
}

/// `IgnoreInfo.forYaml`: os nomes calados por linha (`# ignore:`) e no
/// arquivo todo (`# ignore_for_file:`), em minúsculas.
pub struct IgnoradosYaml {
    no_arquivo: Vec<String>,
    por_linha: std::collections::HashMap<usize, Vec<String>>,
    inicios: Vec<usize>,
}

/// `_trimmedCommaSeparatedMatcher` (`[^\s,]([^,]*[^\s,])?`).
fn nomes_separados(t: &str) -> Vec<String> {
    t.split(',').map(str::trim).filter(|x| !x.is_empty()).map(str::to_lowercase).collect()
}

impl IgnoradosYaml {
    pub fn de(fonte: &str) -> IgnoradosYaml {
        let mut inicios = vec![0usize];
        let b = fonte.as_bytes();
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'\r' => {
                    if b.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    inicios.push(i + 1);
                }
                b'\n' => inicios.push(i + 1),
                _ => {}
            }
            i += 1;
        }
        let mut r = IgnoradosYaml { no_arquivo: Vec::new(), por_linha: Default::default(), inicios };
        // As linhas como o `.` do `RegExp` as vê (sem `\n`, `\r`, U+2028 e
        // U+2029).
        let linhas: Vec<(usize, &str)> = {
            let mut v = Vec::new();
            let mut ini = 0;
            for (k, ch) in fonte.char_indices() {
                if matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                    v.push((ini, &fonte[ini..k]));
                    ini = k + ch.len_utf8();
                }
            }
            v.push((ini, &fonte[ini..]));
            v
        };
        for &(ini, linha) in &linhas {
            // `#[ ]*ignore_for_file:(?<ignored>.*)`: cada ocorrência.
            let mut de = 0;
            while let Some(k) = linha[de..].find('#') {
                let pos = de + k;
                let resto = linha[pos + 1..].trim_start_matches(' ');
                if let Some(nomes) = resto.strip_prefix("ignore_for_file:") {
                    r.no_arquivo.extend(nomes_separados(nomes));
                    break;
                }
                de = pos + 1;
            }
            // `^(?<before>.*)#+[ ]*ignore:(?<ignored>.*)` (multilinha): o
            // `.*` guloso fica com a última ocorrência de `#+ *ignore:`, e
            // com todos os `#` dela menos um.
            let mut ultimo: Option<(usize, usize)> = None;
            for (k, _) in linha.match_indices('#') {
                let resto = linha[k + 1..].trim_start_matches(' ');
                if let Some(nomes) = resto.strip_prefix("ignore:") {
                    ultimo = Some((k, linha.len() - nomes.len()));
                }
            }
            if let Some((hash, depois)) = ultimo {
                // `before.trim().isEmpty`: o comentário vale para a linha
                // seguinte.
                let linha_no = linha_de(&r.inicios, ini);
                let alvo = if linha[..hash].trim().is_empty() { linha_no + 1 } else { linha_no };
                r.por_linha.entry(alvo).or_default().extend(nomes_separados(&linha[depois..]));
            }
        }
        r
    }

    /// O código `nome` (único `unico`) em `pos` está calado.
    pub fn ignora(&self, nome: &str, unico: &str, pos: usize) -> bool {
        let casa = |x: &String| *x == nome.to_lowercase() || *x == unico.to_lowercase();
        if self.no_arquivo.iter().any(casa) {
            return true;
        }
        self.por_linha.get(&linha_de(&self.inicios, pos)).is_some_and(|v| v.iter().any(casa))
    }
}

fn linha_de(inicios: &[usize], pos: usize) -> usize {
    inicios.partition_point(|&x| x <= pos)
}
