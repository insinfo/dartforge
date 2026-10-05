//! Os três "ignore" do Dart 3.6.2 (`COR:ignore_diagnostic.dart`;
//! docs/LSP-ESPECIFICACAO.md §13.7.1 item 6): `Ignore 'x' for this line`,
//! `Ignore 'x' for the whole file` e ``Ignore 'x' in `analysis_options.yaml` ``,
//! este último com o `YamlEditor.update` do `yaml_edit` 2.2.3
//! (`editor.dart`, `map_mutations.dart`, `strings.dart`, `utils.dart`)
//! portado nos caminhos que o fix usa.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::acoes::AcaoDeCodigo;
use crate::Edicao;
use dartforge_analise::naodart::yaml::{self, No, Valor};
use dartforge_diagnostics::{Diagnostic, Span};
use std::path::{Path, PathBuf};
use url::Url;

/// `LineInfo.fromContent`: os inícios de linha (`\n`, `\r\n` e `\r`).
fn inicios_de_linha(t: &str) -> Vec<usize> {
    let b = t.as_bytes();
    let mut v = vec![0];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\r' if b.get(i + 1) == Some(&b'\n') => {
                v.push(i + 2);
                i += 2;
                continue;
            }
            b'\r' | b'\n' => v.push(i + 1),
            _ => {}
        }
        i += 1;
    }
    v
}

/// `IgnoreInfo.ignoreMatcher` (`//+[ ]*ignore:`) no começo do texto.
fn comeca_com_ignore(linha: &str) -> bool {
    let barras = linha.len() - linha.trim_start_matches('/').len();
    if barras < 2 {
        return false;
    }
    linha[barras..].trim_start_matches(' ').starts_with("ignore:")
}

/// As opções aplicáveis ao arquivo e o caminho do `analysis_options.yaml`
/// (`analysisOptions.file`), quando existe.
pub(crate) fn opcoes_do_arquivo(caminho: &Path) -> (dartforge_paridade::filtros::Opcoes, Option<PathBuf>) {
    let raiz = crate::projeto::raiz_do_projeto(caminho);
    let arquivo = dartforge_paridade::filtros::Opcoes::de_subpasta(caminho, &raiz).unwrap_or_else(|| raiz.join("analysis_options.yaml"));
    let existe = arquivo.is_file();
    (dartforge_paridade::filtros::Opcoes::ler_arquivo(&arquivo), existe.then_some(arquivo))
}

/// Os três fixes de cada diagnóstico `HintCode`/`WarningCode` das linhas
/// `inicio..fim`, um conjunto por diagnóstico (o dedupe por título é do
/// ordenador).
pub(crate) fn ignorar(uri: &str, texto: &str, diagnosticos: &[Diagnostic], inicio: usize, fim: usize) -> Vec<AcaoDeCodigo> {
    let caminho = crate::projeto::arquivo_da_uri(uri);
    let (opcoes, arquivo_de_opcoes) = match caminho.as_deref() {
        Some(c) => opcoes_do_arquivo(c),
        None => (dartforge_paridade::filtros::Opcoes::default(), None),
    };
    let tx = crate::refatoracoes_exec::Texto::novo(texto);
    let eol = tx.eol();
    let linhas = inicios_de_linha(texto);
    let linha_de = |o: usize| linhas.partition_point(|&s| s <= o).saturating_sub(1);
    let mut saida = Vec::new();
    for d in diagnosticos {
        if !(d.span.start <= fim && inicio <= d.span.end) {
            continue;
        }
        let Some(c) = d.code else { continue };
        let info = c.info();
        // Só `LintCode`, `HintCode` e `WarningCode`.
        if !(info.unico.starts_with("HintCode.") || info.unico.starts_with("WarningCode.") || info.unico.starts_with("LintCode.")) {
            continue;
        }
        // `_isCodeUnignorable`.
        if !opcoes.ignoravel(d) || matches!(opcoes.errors.get(info.nome), Some(None)) {
            continue;
        }
        let codigo = info.nome;
        let inserir_em = |o: usize, antes: bool, depois: bool, prefixo: &str| -> (Span, String) {
            let recuo = tx.prefixo_da_linha(o);
            let mut s = String::new();
            if antes {
                s.push_str(eol);
            }
            s.push_str(recuo);
            s.push_str(&format!("// {prefixo}: {codigo}"));
            s.push_str(eol);
            if depois {
                s.push_str(eol);
            }
            (Span { start: o, end: o }, s)
        };
        // `IgnoreDiagnosticOnLine`.
        let n = linha_de(d.span.start.min(texto.len()));
        let edicao = if n == 0 {
            inserir_em(0, false, false, "ignore")
        } else {
            let anterior = linhas[n - 1];
            let comeco = linhas[n];
            if comeca_com_ignore(texto[anterior..comeco].trim()) {
                let o = comeco - eol.len();
                (Span { start: o, end: o }, format!(", {codigo}"))
            } else {
                inserir_em(comeco, false, false, "ignore")
            }
        };
        saida.push(acao(uri, format!("Ignore '{codigo}' for this line"), "quickfix.ignore.line", vec![edicao], d));
        // `IgnoreDiagnosticInFile`.
        let total = linhas.len();
        let edicao = if total == 1 {
            inserir_em(0, false, true, "ignore_for_file")
        } else {
            let mut em_branco = None;
            let mut comeco = 0;
            let mut existente = None;
            for k in 0..total - 1 {
                comeco = linhas[k];
                let seguinte = linhas[k + 1];
                let l = texto[comeco..seguinte].trim();
                if l.starts_with("// ignore_for_file:") {
                    existente = Some(seguinte - eol.len());
                    break;
                }
                if l.is_empty() {
                    em_branco = Some(comeco);
                    continue;
                }
                if l.starts_with("#!") || l.starts_with("//") {
                    continue;
                }
                break;
            }
            match (existente, em_branco) {
                (Some(o), _) => (Span { start: o, end: o }, format!(", {codigo}")),
                (None, Some(o)) => inserir_em(o, true, false, "ignore_for_file"),
                (None, None) => inserir_em(comeco, false, true, "ignore_for_file"),
            }
        };
        saida.push(acao(uri, format!("Ignore '{codigo}' for the whole file"), "quickfix.ignore.file", vec![edicao], d));
        // `IgnoreDiagnosticInAnalysisOptionsFile`.
        if let Some(arquivo) = &arquivo_de_opcoes
            && let Ok(conteudo) = std::fs::read_to_string(arquivo)
            && let Some((offset, texto_novo)) = ignorar_no_yaml(&conteudo, codigo)
            && let Ok(uri_yaml) = Url::from_file_path(arquivo)
        {
            saida.push(AcaoDeCodigo {
                titulo: format!("Ignore '{codigo}' in `analysis_options.yaml`"),
                especie: "quickfix.ignore.analysis".into(),
                edicoes: vec![Edicao { uri: uri_yaml.to_string(), span: Span { start: offset, end: offset }, texto: texto_novo }],
                diagnostico: Some(d.clone()),
                criar_arquivo: None,
            });
        }
    }
    saida
}

fn acao(uri: &str, titulo: String, especie: &str, edicoes: Vec<(Span, String)>, d: &Diagnostic) -> AcaoDeCodigo {
    AcaoDeCodigo {
        titulo,
        especie: especie.into(),
        edicoes: edicoes.into_iter().map(|(span, texto)| Edicao { uri: uri.to_string(), span, texto }).collect(),
        diagnostico: Some(d.clone()),
        criar_arquivo: None,
    }
}

// -- yaml_edit ------------------------------------------------------------------

/// Um valor novo: escalar simples ou mapa (`wrapAsYamlNode` de um `Map`).
enum Novo {
    Escalar(String),
    Mapa(Vec<(String, Novo)>),
}

/// `getLineEnding`: `\r\n` se mais da metade das quebras o usam.
fn fim_de_linha(t: &str) -> &'static str {
    let b = t.as_bytes();
    let (mut unix, mut windows) = (0, 0);
    for (i, &c) in b.iter().enumerate() {
        if c == b'\n' {
            if i != 0 && b[i - 1] == b'\r' {
                windows += 1;
            } else {
                unix += 1;
            }
        }
    }
    if windows > unix { "\r\n" } else { "\n" }
}

/// `String.lastIndexOf(c, inicio)` (inclusivo).
fn ultimo_indice(t: &str, c: u8, inicio: usize) -> Option<usize> {
    let b = t.as_bytes();
    let fim = inicio.min(b.len().saturating_sub(1));
    if b.is_empty() {
        return None;
    }
    (0..=fim).rev().find(|&i| b[i] == c)
}

fn em_fluxo(t: &str, no: &No) -> bool {
    t[no.span.start.min(t.len())..].starts_with(['{', '['])
}

/// `getContentSensitiveEnd`.
fn fim_sensivel(t: &str, no: &No) -> usize {
    match &no.valor {
        Valor::Lista(l) if !em_fluxo(t, no) => l.last().map_or(no.span.end, |x| fim_sensivel(t, x)),
        Valor::Mapa(m) if !em_fluxo(t, no) => m.last().map_or(no.span.end, |(_, v)| fim_sensivel(t, v)),
        _ => no.span.end,
    }
}

/// `getMapIndentation` (`None`: o mapa de bloco vazio, `UnsupportedError`).
fn recuo_do_mapa(t: &str, mapa: &No) -> Option<usize> {
    if em_fluxo(t, mapa) {
        return Some(0);
    }
    let (ultima, _) = mapa.mapa()?.last()?;
    let o = ultima.span.start;
    let nl = ultimo_indice(t, b'\n', o);
    let q = ultimo_indice(t, b'?', o);
    Some(match (q, nl) {
        (None, None) => o,
        (None, Some(n)) => o - n - 1,
        (Some(q), None) => q,
        (Some(q), Some(n)) if q > n => q - n - 1,
        (Some(_), Some(n)) => o - n - 1,
    })
}

/// `getListIndentation`.
fn recuo_da_lista(t: &str, lista: &No) -> Option<usize> {
    if em_fluxo(t, lista) {
        return Some(0);
    }
    let ultimo = lista.lista()?.last()?;
    let hifen = ultimo_indice(t, b'-', ultimo.span.start.checked_sub(1)?)?;
    if hifen == 0 {
        return Some(0);
    }
    let nl = hifen.checked_sub(1).and_then(|h| ultimo_indice(t, b'\n', h));
    Some(match nl {
        Some(n) => hifen - n - 1,
        None => hifen,
    })
}

/// `getIndentation(editor)`: o recuo da última coleção de bloco não nula
/// no segundo nível; 2 sem nenhuma.
fn recuo_do_editor(t: &str, raiz: &No) -> usize {
    let mut recuo = 2;
    let filhos: Vec<&No> = match &raiz.valor {
        Valor::Mapa(m) if !em_fluxo(t, raiz) => m.iter().map(|(_, v)| v).collect(),
        Valor::Lista(l) if !em_fluxo(t, raiz) => l.iter().collect(),
        _ => Vec::new(),
    };
    for f in filhos {
        let r = match &f.valor {
            Valor::Lista(_) => recuo_da_lista(t, f).unwrap_or(0),
            Valor::Mapa(_) => recuo_do_mapa(t, f).unwrap_or(0),
            _ => 0,
        };
        if r != 0 {
            recuo = r;
        }
    }
    recuo
}

/// O `toString` de uma chave escalar.
fn texto_da_chave(k: &No) -> String {
    match &k.valor {
        Valor::Texto(s) | Valor::Outro(s) => s.clone(),
        Valor::Nulo => "null".to_string(),
        _ => String::new(),
    }
}

/// `getMapInsertionIndex`.
fn indice_de_insercao(mapa: &[(No, No)], chave: &str) -> usize {
    let chaves: Vec<String> = mapa.iter().map(|(k, _)| texto_da_chave(k)).collect();
    for i in 1..chaves.len() {
        if chaves[i] < chaves[i - 1] {
            return mapa.len();
        }
    }
    chaves.iter().position(|k| k.as_str() > chave).unwrap_or(mapa.len())
}

/// `yamlEncodeFlow`.
fn em_fluxo_texto(v: &Novo) -> String {
    match v {
        Novo::Escalar(s) => s.clone(),
        Novo::Mapa(m) => format!("{{{}}}", m.iter().map(|(k, x)| format!("{k}: {}", em_fluxo_texto(x))).collect::<Vec<_>>().join(", ")),
    }
}

/// `yamlEncodeBlock`.
fn em_bloco(v: &Novo, recuo: usize, eol: &str) -> String {
    match v {
        Novo::Escalar(s) => s.clone(),
        Novo::Mapa(m) if m.is_empty() => format!("{}{{}}", " ".repeat(recuo)),
        Novo::Mapa(m) => m
            .iter()
            .map(|(k, x)| {
                let chave = format!("{}{k}", " ".repeat(recuo));
                let valor = em_bloco(x, recuo + 2, eol);
                match x {
                    Novo::Mapa(n) if !n.is_empty() => format!("{chave}:{eol}{valor}"),
                    _ => format!("{chave}: {valor}"),
                }
            })
            .collect::<Vec<_>>()
            .join(eol),
    }
}

fn colecao_nao_vazia(v: &Novo) -> bool {
    matches!(v, Novo::Mapa(m) if !m.is_empty())
}

/// `updateInMap(editor, mapa, chave, valor)`: o edit (offset, texto).
fn atualizar_no_mapa(t: &str, raiz: &No, mapa: &No, chave: &str, valor: &Novo) -> Option<(usize, String)> {
    let pares = mapa.mapa()?;
    let existente = pares.iter().rev().find(|(k, _)| texto_da_chave(k) == chave);
    let fluxo = em_fluxo(t, mapa);
    let eol = fim_de_linha(t);
    match (existente, fluxo) {
        (None, true) => {
            // `_addToFlowMap`.
            let par = format!("{chave}: {}", em_fluxo_texto(valor));
            if pares.is_empty() {
                return Some((mapa.span.end - 1, par));
            }
            let i = indice_de_insercao(pares, chave);
            if i == pares.len() {
                return Some((mapa.span.end - 1, format!(", {par}")));
            }
            Some((pares[i].0.span.start, format!("{par}, ")))
        }
        (None, false) => {
            // `_addToBlockMap`.
            let recuo_mapa = recuo_do_mapa(t, mapa)?;
            let novo_recuo = recuo_mapa + recuo_do_editor(t, raiz);
            let mut formatado = " ".repeat(recuo_mapa);
            let mut offset = mapa.span.end;
            let i = indice_de_insercao(pares, chave);
            if !pares.is_empty() {
                if i == pares.len() {
                    let fim = fim_sensivel(t, &pares[pares.len() - 1].1);
                    match t[fim.min(t.len())..].find('\n') {
                        Some(k) => offset = fim + k + 1,
                        None => formatado = format!("{eol}{formatado}"),
                    }
                } else {
                    offset = ultimo_indice(t, b'\n', pares[i].0.span.start).map_or(0, |n| n + 1);
                }
            }
            let valor_texto = em_bloco(valor, novo_recuo, eol);
            if colecao_nao_vazia(valor) {
                formatado.push_str(&format!("{chave}:{eol}{valor_texto}{eol}"));
            } else {
                formatado.push_str(&format!("{chave}: {valor_texto}{eol}"));
            }
            Some((offset, formatado))
        }
        (Some((_, v)), true) => {
            // `_replaceInFlowMap`.
            Some((v.span.start, em_fluxo_texto(valor)))
        }
        (Some((k, _)), false) => {
            // `_replaceInBlockMap`: do fim da chave + 1 (o `:`).
            let recuo_mapa = recuo_do_mapa(t, mapa)?;
            let novo_recuo = recuo_mapa + recuo_do_editor(t, raiz);
            let mut texto = em_bloco(valor, novo_recuo, eol);
            if colecao_nao_vazia(valor) {
                texto = format!("{eol}{texto}");
            }
            if !texto.starts_with(eol) {
                texto = format!(" {texto}");
            }
            Some((k.span.end + 1, texto))
        }
    }
}

/// O edit do `IgnoreDiagnosticInAnalysisOptionsFile` (o offset e o
/// `replacement`, emitido como inserção); `None` no YAML inválido.
pub(crate) fn ignorar_no_yaml(conteudo: &str, codigo: &str) -> Option<(usize, String)> {
    let raiz = yaml::ler(conteudo).ok()?;
    let erros = || Novo::Mapa(vec![(codigo.to_string(), Novo::Escalar("ignore".to_string()))]);
    let Some(r) = raiz.as_ref().filter(|r| r.mapa().is_some()) else {
        // `path = []`: o documento inteiro (a inserção no começo dele).
        let eol = fim_de_linha(conteudo);
        let inicio = raiz.as_ref().map_or(conteudo.len(), |r| r.span.start);
        let valor = Novo::Mapa(vec![("analyzer".to_string(), Novo::Mapa(vec![("errors".to_string(), erros())]))]);
        return Some((inicio, em_bloco(&valor, 0, eol)));
    };
    let analyzer = r.campo("analyzer").filter(|a| a.mapa().is_some() && a.par("errors").is_some());
    match analyzer {
        None => atualizar_no_mapa(conteudo, r, r, "analyzer", &Novo::Mapa(vec![("errors".to_string(), erros())])),
        Some(a) => {
            let e = a.campo("errors")?;
            e.mapa()?;
            atualizar_no_mapa(conteudo, r, e, codigo, &Novo::Escalar("ignore".to_string()))
        }
    }
}

#[cfg(test)]
mod testes {
    use super::ignorar_no_yaml;

    #[test]
    fn yaml_do_ignore() {
        assert_eq!(ignorar_no_yaml("", "x"), Some((0, "analyzer:\n  errors:\n    x: ignore".to_string())));
        assert_eq!(
            ignorar_no_yaml("analyzer:\n  errors:\n    a: ignore\n    z: ignore\n", "m"),
            Some((34, "    m: ignore\n".to_string()))
        );
        assert_eq!(
            ignorar_no_yaml("include: package:lints/recommended.yaml\n", "x"),
            Some((0, "analyzer:\n  errors:\n    x: ignore\n".to_string()))
        );
    }
}
