//! `workspace/willRenameFiles` (docs/LSP-ESPECIFICACAO.md §8.5): o
//! `MoveFileRefactoringImpl.multi` do 3.6.2
//! (`AS:src/services/refactoring/legacy/move_file.dart`). Renomear ou mover
//! arquivos (ou pastas, resolvidas nos arquivos de dentro) só reescreve as
//! URIs de diretivas:
//!
//! * as que chegam ao arquivo movido (`import`, `export`, `part` e, para uma
//!   biblioteca, o `part of` relativo das partes dela), de qualquer arquivo
//!   da raiz de análise;
//! * as relativas que saem dele, quando a pasta muda.
//!
//! A URI nova mantém `package:` quando o alvo continua num `lib/` de pacote;
//! senão é o caminho relativo de onde o arquivo de origem vai ficar até onde
//! o alvo vai ficar, com `/`. As aspas do literal ficam. Uma condição fatal
//! (renomear a raiz, arquivo fora de uma raiz ou de mais de uma, arquivo que
//! não existe) não muda nada.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::DocumentStore;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast;
use std::path::{Component, Path, PathBuf};

/// Uma referência a atualizar: o literal da URI, o arquivo em que está, o
/// arquivo a que aponta e o lexema com as aspas.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Referencia {
    span: (usize, usize),
    origem: PathBuf,
    alvo: PathBuf,
    lexema: String,
}

/// O caminho normalizado (`.` e `..` resolvidos), sem tocar no disco.
fn normalizar(p: &Path) -> PathBuf {
    let mut saida = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                saida.pop();
            }
            outro => saida.push(outro.as_os_str()),
        }
    }
    saida
}

/// A chave de comparação de caminhos (a do carregamento).
fn chave(p: &Path) -> PathBuf {
    dartforge_elements::gerado::chave(&normalizar(p))
}

/// `_isRelativeUri`: nem URI absoluta (com esquema) nem caminho absoluto.
fn uri_relativa(uri: &str) -> bool {
    let esquema = uri.find(':').is_some_and(|i| i > 1 && uri[..i].chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')));
    !esquema && !Path::new(uri).is_absolute() && !uri.starts_with('/') && !uri.starts_with('\\')
}

/// A URI de cada diretiva com literal simples (sem interpolação nem
/// adjacência): o literal, o valor e se é `part of`.
fn uris_das_diretivas(unit: &ast::CompilationUnit) -> Vec<(&ast::StringLit, String, bool)> {
    let mut v = Vec::new();
    for d in &unit.directives {
        let (lit, parte_de) = match &d.kind {
            ast::DirectiveKind::Import { uri, .. } | ast::DirectiveKind::Export { uri, .. } | ast::DirectiveKind::Part { uri } => (Some(uri), false),
            ast::DirectiveKind::PartOf { uri, .. } => (uri.as_ref(), true),
            _ => (None, false),
        };
        let Some(lit) = lit else { continue };
        if lit.parts.len() != 1 {
            continue;
        }
        let Some(valor) = lit.constant_value() else { continue };
        v.push((lit, String::from_utf8_lossy(valor.as_bytes()).into_owned(), parte_de));
    }
    v
}

/// O caminho de uma URI de diretiva escrita em `origem`: relativa, ou
/// `package:` pelos pacotes da raiz.
fn caminho_da_uri(uri: &str, origem: &Path, pacotes: &[(String, PathBuf)]) -> Option<PathBuf> {
    if let Some(resto) = uri.strip_prefix("package:") {
        let (nome, rel) = resto.split_once('/')?;
        let (_, lib) = pacotes.iter().find(|(n, _)| n == nome)?;
        return Some(normalizar(&rel.split('/').fold(lib.clone(), |p, x| p.join(x))));
    }
    if !uri_relativa(uri) {
        return None;
    }
    let pasta = origem.parent()?;
    Some(normalizar(&uri.split('/').fold(pasta.to_path_buf(), |p, x| p.join(x))))
}

/// `pathToUri`: `package:` quando o arquivo está no `lib/` de um pacote.
fn uri_de_pacote(alvo: &Path, pacotes: &[(String, PathBuf)]) -> Option<String> {
    let alvo = normalizar(alvo);
    for (nome, lib) in pacotes {
        if let Ok(rel) = alvo.strip_prefix(normalizar(lib)) {
            let partes: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            return Some(format!("package:{nome}/{}", partes.join("/")));
        }
    }
    None
}

/// O caminho relativo de `pasta` até `alvo`, com `/` (o `pathContext.relative`
/// e o `posix.joinAll` do Dart).
fn relativo(alvo: &Path, pasta: &Path) -> String {
    let a: Vec<Component> = normalizar(alvo).components().collect();
    let b: Vec<Component> = normalizar(pasta).components().collect();
    // O `pathContext` do Windows compara sem caixa.
    let igual = |x: &Component, y: &Component| {
        if cfg!(windows) {
            x.as_os_str().to_string_lossy().to_lowercase() == y.as_os_str().to_string_lossy().to_lowercase()
        } else {
            x == y
        }
    };
    let comum = a.iter().zip(b.iter()).take_while(|(x, y)| igual(x, y)).count();
    let mut partes: Vec<String> = vec!["..".to_string(); b.len() - comum];
    partes.extend(a[comum..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    partes.join("/")
}

/// As aspas do lexema (`analyzeQuote`): o começo (com o `r`) e o fim.
fn aspas(lexema: &str) -> (usize, usize) {
    let cru = usize::from(lexema.starts_with('r'));
    let resto = &lexema[cru..];
    let n = if resto.starts_with("'''") || resto.starts_with("\"\"\"") { 3 } else { 1 };
    (cru + n, n)
}

/// `_resolveMapping`: uma pasta vira os arquivos de dentro, recursivamente.
fn resolver(antigo: &Path, novo: &Path, saida: &mut Vec<(PathBuf, PathBuf)>) {
    if antigo.is_dir() {
        let Ok(r) = std::fs::read_dir(antigo) else { return };
        let mut filhos: Vec<PathBuf> = r.flatten().map(|e| e.path()).collect();
        filhos.sort();
        for f in filhos {
            let Some(nome) = f.file_name() else { continue };
            resolver(&f, &novo.join(nome), saida);
        }
    } else {
        saida.push((antigo.to_path_buf(), novo.to_path_buf()));
    }
}

/// As edições do `willRenameFiles` para `mapa` (antigo → novo), nas raízes
/// de análise `raizes`. `None`: condição fatal (o Dart responde `null`).
pub(crate) fn edicoes(raizes: &[PathBuf], documentos: &DocumentStore, mapa: &[(PathBuf, PathBuf)]) -> Option<Vec<crate::Edicao>> {
    // `checkFinalConditions`.
    let mut raiz_comum: Option<PathBuf> = None;
    for (antigo, _) in mapa {
        if raizes.iter().any(|r| chave(r) == chave(antigo)) {
            return None;
        }
        let contem: Vec<&PathBuf> = raizes.iter().filter(|r| chave(antigo).starts_with(chave(r))).collect();
        if contem.len() != 1 || !antigo.exists() {
            return None;
        }
        match &raiz_comum {
            Some(r) if chave(r) != chave(contem[0]) => return None,
            _ => raiz_comum = Some(contem[0].clone()),
        }
    }
    let raiz = raiz_comum?;
    let pacotes = crate::projeto::pacotes_com_nome(&raiz);
    let mut resolvido: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (antigo, novo) in mapa {
        resolver(antigo, novo, &mut resolvido);
    }
    let novo_de = |p: &Path| resolvido.iter().find(|(a, _)| chave(a) == chave(p)).map(|(_, n)| n.clone());
    // Os textos: o aberto vale mais que o disco.
    let texto_de = |p: &Path| -> Option<String> {
        if let Ok(u) = url::Url::from_file_path(p)
            && let Some(t) = documentos.get(u.as_str())
        {
            return Some(t.to_string());
        }
        std::fs::read_to_string(p).ok()
    };
    // Os arquivos da raiz de análise e os abertos sob ela.
    let mut arquivos = crate::projeto::arquivos_do_projeto(&raiz);
    for u in documentos.uris() {
        if let Some(p) = url::Url::parse(u).ok().and_then(|x| x.to_file_path().ok())
            && chave(&p).starts_with(chave(&raiz))
            && p.extension().is_some_and(|e| e == "dart")
            && !arquivos.iter().any(|a| chave(a) == chave(&p))
        {
            arquivos.push(p);
        }
    }
    let movidos: Vec<PathBuf> = resolvido.iter().filter(|(a, _)| a.extension().is_some_and(|e| e == "dart")).map(|(a, _)| chave(a)).collect();
    let mut referencias: std::collections::BTreeSet<Referencia> = std::collections::BTreeSet::new();
    for arquivo in &arquivos {
        let Some(texto) = texto_de(arquivo) else { continue };
        let mut nomes = dartforge_intern::Interner::new();
        let p = dartforge_frontend::parser::parse_com(&texto, &mut nomes, dartforge_frontend::LibraryFeatures::default());
        let movido = movidos.contains(&chave(arquivo));
        let pasta_muda = movido
            && novo_de(arquivo).is_some_and(|n| n.parent().map(chave) != arquivo.parent().map(chave));
        for (lit, uri, parte_de) in uris_das_diretivas(&p.unit) {
            let Some(alvo) = caminho_da_uri(&uri, arquivo, &pacotes) else { continue };
            let lexema = texto.get(lit.span.start..lit.span.end).unwrap_or_default().to_string();
            // O `part of` que chega só é achado relativo (a busca de
            // referências da unidade não o inclui; o passo das partes da
            // biblioteca só lê os relativos).
            let entrada = movidos.contains(&chave(&alvo)) && (!parte_de || uri_relativa(&uri));
            // Que sai de um arquivo cuja pasta muda: só as relativas.
            let saindo = pasta_muda && uri_relativa(&uri);
            if entrada || saindo {
                referencias.insert(Referencia { span: (lit.span.start, lit.span.end), origem: arquivo.clone(), alvo, lexema });
            }
        }
    }
    let mut saida = Vec::new();
    for r in referencias {
        let nova_origem = novo_de(&r.origem).unwrap_or_else(|| r.origem.clone());
        let novo_alvo = novo_de(&r.alvo).unwrap_or_else(|| r.alvo.clone());
        let (comeco, fim) = aspas(&r.lexema);
        if r.lexema.len() < comeco + fim {
            continue;
        }
        let atual = &r.lexema[comeco..r.lexema.len() - fim];
        let nova = match atual.starts_with("package:").then(|| uri_de_pacote(&novo_alvo, &pacotes)).flatten() {
            Some(u) => u,
            None => relativo(&novo_alvo, nova_origem.parent().unwrap_or(Path::new(""))),
        };
        if nova == atual {
            continue;
        }
        let Ok(uri) = url::Url::from_file_path(&r.origem) else { continue };
        saida.push(crate::Edicao {
            uri: uri.to_string(),
            span: Span { start: r.span.0, end: r.span.1 },
            texto: format!("{}{nova}{}", &r.lexema[..comeco], &r.lexema[r.lexema.len() - fim..]),
        });
    }
    Some(saida)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn caminhos_e_aspas() {
        assert!(uri_relativa("a/b.dart"));
        assert!(uri_relativa("../b.dart"));
        assert!(!uri_relativa("package:x/y.dart"));
        assert!(!uri_relativa("dart:core"));
        assert_eq!(relativo(Path::new("/p/lib/src/b.dart"), Path::new("/p/lib")), "src/b.dart");
        assert_eq!(relativo(Path::new("/p/lib/b.dart"), Path::new("/p/lib/src")), "../b.dart");
        assert_eq!(aspas("'a.dart'"), (1, 1));
        assert_eq!(aspas("r\"a.dart\""), (2, 1));
        assert_eq!(aspas("'''a.dart'''"), (3, 3));
        let pacotes = vec![("x".to_string(), PathBuf::from("/p/lib"))];
        assert_eq!(uri_de_pacote(Path::new("/p/lib/src/c.dart"), &pacotes).as_deref(), Some("package:x/src/c.dart"));
        assert_eq!(caminho_da_uri("package:x/a.dart", Path::new("/q/z.dart"), &pacotes), Some(PathBuf::from("/p/lib/a.dart")));
    }
}
