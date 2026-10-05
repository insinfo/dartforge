//! `Organize Imports` (`dart.edit.organizeImports`): o `ImportOrganizer` do
//! servidor do Dart 3.6.2
//! (`AS:src/services/correction/organize_imports.dart:22-345` e
//! `AP:src/utilities/directive_sort.dart`; docs/LSP-ESPECIFICACAO.md
//! §13.12.6).
//!
//! As diretivas `import`, `export` e `part` são ordenadas por grupo (nove
//! prioridades) e pela URI; cada uma leva o comentário de cima e o primeiro
//! comentário de fim de linha; o bloco inteiro, da primeira à última, é
//! substituído pela concatenação dos textos, com uma linha em branco entre
//! os grupos. Com `remover`, saem os imports apontados pelos erros
//! (`unused_import`, `duplicate_import`, `unnecessary_import`) e as
//! duplicatas textuais, salvo quando há identificador não resolvido.
//!
//! As anotações de biblioteca (`TargetKind.library`) da primeira diretiva
//! não são reconhecidas: pedem a unidade resolvida. É o comportamento do
//! original quando chamado pelo `Sort Members`.
//! Escrito sem compilar nem executar (2026-10-04).

use crate::fonte_ordenar::{Comentario, comentarios, linha_de};
use dartforge_frontend::ast::{CompilationUnit, DirectiveKind};
use std::cmp::Ordering;

struct Info {
    /// O ordinal de `DirectiveSortPriority`.
    prioridade: u8,
    uri: String,
    inicio: usize,
    fim: usize,
    /// Há erro de import não usado, duplicado ou desnecessário no literal
    /// da URI.
    removivel: bool,
}

/// A ordem do `String.compareTo` do Dart: unidades UTF-16.
fn utf16(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `compareDirectiveUri` (`directive_sort.dart:10-22`).
fn comparar_uri(a: &str, b: &str) -> Ordering {
    if (!a.starts_with("package:") || !b.starts_with("package:")) && !a.starts_with('/') && !b.starts_with('/') {
        return utf16(a, b);
    }
    match (a.find('/'), b.find('/')) {
        (Some(i), Some(j)) => utf16(&a[..i], &b[..j]).then_with(|| utf16(&a[i + 1..], &b[j + 1..])),
        _ => utf16(a, b),
    }
}

/// O início da linha seguinte à do offset (`getOffsetOfLineAfter`); `None`
/// quando não há (o original lança).
fn linha_seguinte(codigo: &str, offset: usize) -> Option<usize> {
    codigo.get(offset..)?.find('\n').map(|k| offset + k + 1)
}

/// `// @dart = 2.12`: o comentário de versão da linguagem
/// (`LanguageVersionToken`).
fn versao_da_linguagem(comentario: &str) -> bool {
    let Some(resto) = comentario.strip_prefix("//") else { return false };
    let Some(resto) = resto.trim_start().strip_prefix("@dart") else { return false };
    let Some(resto) = resto.trim_start().strip_prefix('=') else { return false };
    let mut partes = resto.trim().splitn(2, '.');
    let numero = |p: Option<&str>| p.is_some_and(|x| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit()));
    numero(partes.next()) && numero(partes.next())
}

/// `IgnoreInfo.ignoreMatcher`: `^//+[ ]*ignore:`.
fn de_ignore(comentario: &str) -> bool {
    comentario.starts_with("//") && comentario.trim_start_matches('/').trim_start_matches(' ').starts_with("ignore:")
}

/// O bloco de documentação de um vão: a última sequência de comentários de
/// documentação consecutivos (início e fim).
fn bloco_de_documentacao(cs: &[Comentario]) -> Option<(usize, usize)> {
    let ultimo = cs.iter().rposition(|c| c.de_documentacao)?;
    let mut primeiro = ultimo;
    while primeiro > 0 && cs[primeiro - 1].de_documentacao {
        primeiro -= 1;
    }
    Some((cs[primeiro].inicio, cs[ultimo].fim))
}

/// `getLeadingComment` (`:227-284`): o offset do comentário de cima que
/// acompanha a diretiva. `cs`: os comentários entre o token anterior e a
/// diretiva; `fim_anterior`: o fim do token anterior (`None` quando a
/// diretiva é o primeiro token do arquivo).
fn comentario_de_cima(codigo: &str, cs: &[Comentario], pseudo: bool, fim_anterior: Option<usize>, diretiva: usize) -> Option<usize> {
    if cs.is_empty() {
        return None;
    }
    let linha = |offset: usize| linha_de(codigo, offset);
    let mut k = 0;
    if pseudo {
        // O início do último bloco sem linha em branco no meio.
        for (i, par) in cs.windows(2).enumerate() {
            if linha(par[1].inicio) > linha(par[0].inicio) + 1 {
                k = i + 1;
            }
        }
    }
    if versao_da_linguagem(&codigo[cs[k].inicio..cs[k].fim]) {
        k += 1;
    }
    let c = cs.get(k)?;
    // O primeiro comentário do arquivo é cabeçalho, salvo um `ignore:`.
    if fim_anterior.is_none() && k == 0 {
        return de_ignore(&codigo[c.inicio..c.fim]).then_some(c.inicio);
    }
    if pseudo {
        return (linha(diretiva) == linha(c.inicio) + 1).then_some(c.inicio);
    }
    // Os que começam na linha do token anterior são o comentário de fim de
    // linha dele.
    let linha_anterior = linha(fim_anterior.unwrap_or(0));
    cs[k..].iter().find(|x| linha(x.inicio) != linha_anterior).map(|x| x.inicio)
}

/// O fim do comentário que começa em `pos` ou depois de brancos na mesma
/// linha (`getTrailingComment`: só o primeiro).
fn comentario_de_fim_de_linha(codigo: &str, pos: usize) -> Option<usize> {
    let resto = codigo.get(pos..)?;
    let brancos = resto.len() - resto.trim_start_matches([' ', '\t']).len();
    let inicio = pos + brancos;
    let b = codigo.as_bytes();
    if codigo[inicio..].starts_with("//") {
        let fim = codigo[inicio..].find('\n').map_or(codigo.len(), |k| inicio + k);
        return Some(if fim > inicio && b[fim - 1] == b'\r' { fim - 1 } else { fim });
    }
    if codigo[inicio..].starts_with("/*") {
        let mut nivel = 1;
        let mut j = inicio + 2;
        while j < b.len() && nivel > 0 {
            if j + 1 < b.len() && b[j] == b'/' && b[j + 1] == b'*' {
                nivel += 1;
                j += 2;
            } else if j + 1 < b.len() && b[j] == b'*' && b[j + 1] == b'/' {
                nivel -= 1;
                j += 2;
            } else {
                j += 1;
            }
        }
        return Some(j.min(b.len()));
    }
    None
}

/// `ImportOrganizer.organize` até `code`: o código com o bloco de diretivas
/// reescrito. `None` quando não há diretiva com URI, ou num caso em que o
/// original lança (primeira diretiva documentada sem linha seguinte).
///
/// `removiveis`: os offsets dos literais de URI com erro `unused_import`,
/// `duplicate_import` ou `unnecessary_import`; `nao_resolvido`: há erro de
/// identificador não resolvido no arquivo; `remover`: o `removeUnused`.
pub(crate) fn organizar(codigo: &str, unidade: &CompilationUnit, removiveis: &[usize], nao_resolvido: bool, remover: bool) -> Option<String> {
    let fim_de_linha = if codigo.contains("\r\n") { "\r\n" } else { "\n" };
    let mut tem_library = false;
    let mut infos: Vec<Info> = Vec::new();
    // A tag de script é um token: o que vem depois dela não é o primeiro
    // comentário do arquivo.
    let mut fim_anterior: Option<usize> = unidade.script_tag.map(|s| s.end);
    for (i, d) in unidade.directives.iter().enumerate() {
        let (tipo, literal) = match &d.kind {
            DirectiveKind::Import { uri, .. } => (0u8, uri),
            DirectiveKind::Export { uri, .. } => (1u8, uri),
            DirectiveKind::Part { uri } => (2u8, uri),
            outra => {
                if matches!(outra, DirectiveKind::Library { .. }) {
                    tem_library = true;
                }
                fim_anterior = Some(d.span.end);
                continue;
            }
        };
        let conteudo = dartforge_elements::load::string_lit_value(literal).unwrap_or_default();
        let grupo = if conteudo.starts_with("dart:") {
            0
        } else if conteudo.starts_with("package:") {
            1
        } else if conteudo.contains("://") {
            2
        } else {
            3
        };
        let prioridade = if tipo == 2 { 8 } else { tipo * 4 + grupo };
        let de = fim_anterior.unwrap_or(0).min(d.span.start);
        let cs = comentarios(codigo, de, d.span.start);
        // `directive.offset` inclui o comentário de documentação.
        let doc = bloco_de_documentacao(&cs);
        let offset_da_diretiva = doc.map_or(d.span.start, |(inicio, _)| inicio);
        let pseudo = !tem_library && i == 0;
        // Na primeira diretiva sem `library`, a documentação é da biblioteca
        // e fica no topo, com uma linha em branco depois dela.
        let mut fim_da_biblioteca: Option<usize> = None;
        if pseudo && let Some((_, fim_doc)) = doc {
            let seguinte = linha_seguinte(codigo, fim_doc)?;
            let fim_da_linha = codigo[seguinte..].find('\n').map_or(codigo.len(), |k| seguinte + k);
            fim_da_biblioteca =
                Some(if codigo[seguinte..fim_da_linha].trim().is_empty() { linha_seguinte(codigo, seguinte)? } else { seguinte });
        }
        // Com documentação, o primeiro token da diretiva é o comentário de
        // documentação, que não tem comentários antes.
        let de_cima = if doc.is_some() { None } else { comentario_de_cima(codigo, &cs, pseudo, fim_anterior, d.span.start) };
        let fim = comentario_de_fim_de_linha(codigo, d.span.end).unwrap_or(d.span.end);
        let inicio = fim_da_biblioteca.or(de_cima).unwrap_or(offset_da_diretiva);
        if inicio > fim {
            return None;
        }
        infos.push(Info { prioridade, uri: conteudo, inicio, fim, removivel: removiveis.contains(&literal.span.start) });
        fim_anterior = Some(d.span.end);
    }
    let (primeiro, ultimo) = (infos.first()?.inicio, infos.last()?.fim);
    if ultimo < primeiro {
        return None;
    }
    let mut ordem: Vec<&Info> = infos.iter().collect();
    ordem.sort_by(|a, b| {
        a.prioridade
            .cmp(&b.prioridade)
            .then_with(|| comparar_uri(&a.uri, &b.uri))
            .then_with(|| utf16(&codigo[a.inicio..a.fim], &codigo[b.inicio..b.fim]))
    });
    let mut saida = String::new();
    let mut atual: Option<u8> = None;
    let mut anterior: &str = "";
    for d in ordem {
        let texto = &codigo[d.inicio..d.fim];
        if !nao_resolvido && remover && (d.removivel || anterior == texto) {
            continue;
        }
        if atual != Some(d.prioridade) {
            if atual.is_some() {
                saida.push_str(fim_de_linha);
            }
            atual = Some(d.prioridade);
        }
        saida.push_str(texto);
        saida.push_str(fim_de_linha);
        anterior = texto;
    }
    Some(format!("{}{}{}", &codigo[..primeiro], saida.trim_end(), &codigo[ultimo..]))
}

/// A edição do `organize` (`:47-58`): do offset 0 até o fim do bloco
/// alterado (só o sufixo comum é cortado). `(offset, comprimento, texto)`.
pub(crate) fn edicao(inicial: &str, novo: &str) -> Option<(usize, usize, String)> {
    if inicial == novo {
        return None;
    }
    let (a, n) = (inicial.as_bytes(), novo.as_bytes());
    let mut sufixo = a.iter().rev().zip(n.iter().rev()).take_while(|(x, y)| x == y).count();
    while !inicial.is_char_boundary(a.len() - sufixo) || !novo.is_char_boundary(n.len() - sufixo) {
        sufixo -= 1;
    }
    Some((0, a.len() - sufixo, novo[..n.len() - sufixo].to_string()))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn organizado(fonte: &str, removiveis: &[&str]) -> String {
        let mut nomes = dartforge_intern::Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        // Cada removível é dado pelo texto do literal; vale a 1ª ocorrência.
        let offsets: Vec<usize> = removiveis.iter().map(|r| fonte.find(r).expect("literal presente")).collect();
        organizar(fonte, &p.unit, &offsets, false, true).unwrap_or_else(|| fonte.to_string())
    }

    /// O exemplo 1 da §13.12.6.
    #[test]
    fn exemplo_um_da_especificacao() {
        let antes = "// Copyright.\n\nimport 'package:b/b.dart';\nimport 'dart:io'; // io\nimport 'src/z.dart';\nimport 'dart:async';\n// sobre a\nimport 'package:a/a.dart';\nexport 'src/y.dart';\nimport 'dart:io'; // io\n\nvoid main() {}\n";
        let depois = "// Copyright.\n\nimport 'dart:io'; // io\n\n// sobre a\nimport 'package:a/a.dart';\nimport 'package:b/b.dart';\n\nimport 'src/z.dart';\n\nexport 'src/y.dart';\n\nvoid main() {}\n";
        assert_eq!(organizado(antes, &["'dart:async'"]), depois);
    }

    #[test]
    fn a_edicao_comeca_no_offset_zero() {
        let antes = "import 'b.dart';\nimport 'a.dart';\n\nvoid main() {}\n";
        let depois = organizado(antes, &[]);
        assert_eq!(depois, "import 'a.dart';\nimport 'b.dart';\n\nvoid main() {}\n");
        let (offset, comprimento, texto) = edicao(antes, &depois).expect("mudou");
        assert_eq!(offset, 0);
        assert_eq!(format!("{}{}", texto, &antes[comprimento..]), depois);
    }

    #[test]
    fn pacote_prefixo_de_outro_vem_antes() {
        assert_eq!(comparar_uri("package:a/x.dart", "package:a.b/x.dart"), Ordering::Less);
        assert_eq!(comparar_uri("dart:io", "dart:async"), Ordering::Greater);
    }

    #[test]
    fn comentarios_especiais() {
        assert!(versao_da_linguagem("// @dart = 2.12"));
        assert!(!versao_da_linguagem("// @dart"));
        assert!(de_ignore("// ignore: unused_import"));
        assert!(!de_ignore("// ignore_for_file: unused_import"));
    }
}
