//! O sétimo lote de regras de lint (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), as que olham o lexema das strings e as linhas, escritas direto dos
//! emissores da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `prefer_single_quotes`, `prefer_double_quotes`,
//! `avoid_escaping_inner_quotes`, `unnecessary_string_escapes`,
//! `use_raw_strings`, `leading_newlines_in_multiline_strings` e
//! `lines_longer_than_80_chars`.
//!
//! Os literais vêm de [`super::cordas`], que relê o texto de cada expressão
//! de string (a árvore junta as adjacentes); entram também as URIs das
//! diretivas.
//!
//! Diferenças conhecidas: onde o original pergunta pelo valor da string
//! ("contém aspa", "parece URI"), aqui a pergunta é feita ao texto como
//! escrito, e um caractere produzido só por escape (`\x27`, `\u002f`) não é
//! visto. Em `lines_longer_than_80_chars` só `\n` e `\r\n` terminam linha.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::cordas::{comentarios, literais, Literal};
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{DirectiveKind, ExprKind, StringLit};
use dartforge_intern::Interner;

/// Os caracteres que podem vir depois de `\` (`allowedEscapedChars`).
const ESCAPAVEIS: [char; 12] = ['"', '\'', '$', '\\', 'n', 'r', 'f', 'b', 't', 'v', 'x', 'u'];

/// `_looksLikeUriOrPath` sobre o texto escrito: tem `/`, ou tem `\` (numa
/// string comum, o `\\` que produz uma).
fn parece_uri(texto: &str, crua: bool) -> bool {
    texto.contains('/') || if crua { texto.contains('\\') } else { texto.contains("\\\\") }
}

/// `unnecessary_string_escapes` num trecho de texto (`visitLexeme`): os
/// lugares das barras desnecessárias. `ultimo`: o trecho encosta na aspa de
/// fecho.
fn escapes_desnecessarios(fonte: &str, l: &Literal, trecho: Span, ultimo: bool, saida: &mut Vec<usize>) {
    let Some(texto) = fonte.get(trecho.start..trecho.end) else { return };
    let (propria, outra) = if l.aspas_simples { ('\'', '"') } else { ('"', '\'') };
    // As aspas próprias seguidas: (lugar, escapada). Numa string de três
    // aspas, até duas seguidas dispensam o escape.
    let mut pendentes: Vec<(usize, bool)> = Vec::new();
    let conferir = |pendentes: &[(usize, bool)], saida: &mut Vec<usize>| {
        if l.multilinha && pendentes.len() < 3 {
            for &(lugar, escapada) in pendentes {
                // `'''…\''''`: sem a barra, as aspas se confundem com o fecho.
                if escapada && !(ultimo && lugar + 2 == trecho.end) {
                    saida.push(lugar);
                }
            }
        }
    };
    let cs: Vec<(usize, char)> = texto.char_indices().collect();
    let mut k = 0;
    while k < cs.len() {
        let (mut lugar, mut atual) = cs[k];
        let mut escapado = false;
        if atual == '\\' && k + 1 < cs.len() {
            escapado = true;
            let barra = lugar;
            k += 1;
            (lugar, atual) = cs[k];
            if atual == outra || !ESCAPAVEIS.contains(&atual) {
                saida.push(trecho.start + barra);
            }
        }
        if atual == propria {
            pendentes.push((trecho.start + lugar - usize::from(escapado), escapado));
        } else {
            conferir(&pendentes[..], &mut *saida);
            pendentes.clear();
        }
        k += 1;
    }
    conferir(&pendentes[..], &mut *saida);
}

/// `use_raw_strings`: o conteúdo tem escape, e todos são `\\` ou `\$`.
fn so_escapa_barra_e_cifrao(conteudo: &str) -> bool {
    let cs: Vec<char> = conteudo.chars().collect();
    let mut tem = false;
    let mut i = 0;
    while i + 1 < cs.len() {
        if cs[i] == '\\' {
            tem = true;
            i += 1;
            if cs[i] != '\\' && cs[i] != '$' {
                return false;
            }
        }
        i += 1;
    }
    tem
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, _interner: &Interner, ligada: &dyn Fn(&str) -> bool) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let regras = [
        "prefer_single_quotes",
        "prefer_double_quotes",
        "avoid_escaping_inner_quotes",
        "unnecessary_string_escapes",
        "use_raw_strings",
        "leading_newlines_in_multiline_strings",
        "lines_longer_than_80_chars",
    ];
    if !regras.iter().any(|r| ligada(r)) {
        return out;
    }

    // Todos os literais da unidade: os das expressões e os das diretivas.
    let mut todos: Vec<Literal> = Vec::new();
    for e in a.exprs.iter() {
        if matches!(e.kind, ExprKind::String(_)) {
            todos.extend(literais(fonte, e.span));
        }
    }
    let mut da_diretiva = |lit: &StringLit| todos.extend(literais(fonte, lit.span));
    for d in u.unit.directives.iter() {
        match &d.kind {
            DirectiveKind::Import { uri, configurations, .. } | DirectiveKind::Export { uri, configurations, .. } => {
                da_diretiva(uri);
                for k in configurations.iter() {
                    da_diretiva(&k.uri);
                    if let Some(v) = &k.value {
                        da_diretiva(v);
                    }
                }
            }
            DirectiveKind::Part { uri } | DirectiveKind::ImportAugment { uri } | DirectiveKind::AugmentLibrary { uri } => da_diretiva(uri),
            DirectiveKind::PartOf { uri: Some(uri), .. } => da_diretiva(uri),
            DirectiveKind::PartOf { uri: None, .. } | DirectiveKind::Library { .. } => {}
        }
    }
    todos.sort_by_key(|l| (l.span.start, l.span.end));
    todos.dedup();
    let interpolacoes: Vec<Span> = todos.iter().flat_map(|l| l.interpolacoes.iter().copied()).collect();
    let dentro = |fora: Span, x: Span| fora.start < x.start && x.end <= fora.end;
    // Dentro da interpolação de outra string.
    let aninhado = |l: &Literal| interpolacoes.iter().any(|s| dentro(*s, l.span));
    // Tem outra string numa interpolação sua.
    let contem_string = |l: &Literal| todos.iter().any(|o| l.interpolacoes.iter().any(|s| dentro(*s, o.span)));

    // `prefer_single_quotes` e `prefer_double_quotes` (o `QuoteVisitor`).
    for (regra, simples, codigo) in
        [("prefer_single_quotes", true, &c::PREFER_SINGLE_QUOTES), ("prefer_double_quotes", false, &c::PREFER_DOUBLE_QUOTES)]
    {
        if !ligada(regra) {
            continue;
        }
        let pedida = if simples { '\'' } else { '"' };
        for l in todos.iter() {
            if l.aspas_simples == simples || l.texto(fonte).contains(pedida) || aninhado(l) {
                continue;
            }
            if l.interpolado() && contem_string(l) {
                continue;
            }
            relatar(codigo, l.span, &[]);
        }
    }
    // `avoid_escaping_inner_quotes`: a string comum de uma linha que escapa
    // a própria aspa e não tem a outra.
    if ligada("avoid_escaping_inner_quotes") {
        for l in todos.iter().filter(|l| !l.crua && !l.multilinha) {
            let texto = l.texto(fonte);
            let (propria, outra) = if l.aspas_simples { ("'", "\"") } else { ("\"", "'") };
            if texto.contains(propria) && !texto.contains(outra) {
                relatar(&c::AVOID_ESCAPING_INNER_QUOTES, l.span, &[propria, outra]);
            }
        }
    }
    // `unnecessary_string_escapes`.
    if ligada("unnecessary_string_escapes") {
        let mut lugares: Vec<usize> = Vec::new();
        for l in todos.iter().filter(|l| !l.crua) {
            let n = l.trechos.len();
            for (i, t) in l.trechos.iter().enumerate() {
                escapes_desnecessarios(fonte, l, *t, i + 1 == n, &mut lugares);
            }
        }
        for lugar in lugares {
            relatar(&c::UNNECESSARY_STRING_ESCAPES, Span { start: lugar, end: lugar + 1 }, &[]);
        }
    }
    // `use_raw_strings`: só o literal sem interpolação.
    if ligada("use_raw_strings") {
        for l in todos.iter().filter(|l| !l.crua && !l.interpolado()) {
            if so_escapa_barra_e_cifrao(&l.texto(fonte)) {
                relatar(&c::USE_RAW_STRINGS, l.span, &[]);
            }
        }
    }
    // `leading_newlines_in_multiline_strings`: a string de três aspas que
    // ocupa mais de uma linha começa com quebra de linha.
    if ligada("leading_newlines_in_multiline_strings") {
        for l in todos.iter().filter(|l| l.multilinha) {
            let Some(lexema) = fonte.get(l.span.start..l.span.end) else { continue };
            let Some(primeiro) = l.trechos.first() else { continue };
            let comeco = fonte.get(primeiro.start..).unwrap_or("");
            if lexema.contains('\n') && !comeco.starts_with('\n') && !comeco.starts_with('\r') {
                relatar(&c::LEADING_NEWLINES_IN_MULTILINE_STRINGS, l.span, &[]);
            }
        }
    }
    // `lines_longer_than_80_chars`.
    if ligada("lines_longer_than_80_chars") {
        // Os começos de linha, para o número (a partir de 1) de um lugar.
        let mut comecos: Vec<usize> = vec![0];
        comecos.extend(fonte.bytes().enumerate().filter(|(_, b)| *b == b'\n').map(|(i, _)| i + 1));
        let linha_de = |lugar: usize| comecos.partition_point(|&x| x <= lugar);
        let mut longas: Vec<(usize, Span)> = Vec::new();
        for (i, &inicio) in comecos.iter().enumerate() {
            let mut fim = match comecos.get(i + 1) {
                Some(&seguinte) => seguinte - 1,
                None => fonte.len(),
            };
            let Some(texto) = fonte.get(inicio..fim) else { continue };
            if texto.encode_utf16().count() <= 80 {
                continue;
            }
            // A quebra `\r\n` não conta (só a última linha fica como está).
            if i + 1 < comecos.len() && texto.ends_with('\r') {
                fim -= 1;
            }
            let Some(texto) = fonte.get(inicio..fim) else { continue };
            if texto.encode_utf16().count() <= 80 {
                continue;
            }
            // O relato começa na coluna 80 (em unidades UTF-16).
            let mut unidades = 0;
            let mut corte = fim;
            for (k, ch) in texto.char_indices() {
                if unidades >= 80 {
                    corte = inicio + k;
                    break;
                }
                unidades += ch.len_utf16();
            }
            longas.push((i + 1, Span { start: corte, end: fim }));
        }
        if !longas.is_empty() {
            let mut permitidas: Vec<usize> = Vec::new();
            // Strings: a de três aspas libera todas as suas linhas; a de uma
            // linha, a sua, se parece URI ou caminho.
            for l in todos.iter() {
                if l.multilinha {
                    permitidas.extend(linha_de(l.span.start)..=linha_de(l.span.end));
                } else if parece_uri(&l.texto(fonte), l.crua) {
                    permitidas.push(linha_de(l.span.start));
                }
            }
            // Comentários: `// ignore:` libera a linha; qualquer linha de
            // comentário que pareça URI ou caminho libera a sua.
            for s in comentarios(fonte) {
                let Some(texto) = fonte.get(s.start..s.end) else { continue };
                let primeira = linha_de(s.start);
                let mut linhas: Vec<&str> = Vec::new();
                if let Some(resto) = texto.strip_prefix("///") {
                    linhas.push(resto);
                } else if let Some(resto) = texto.strip_prefix("//") {
                    if resto.trim_start().starts_with("ignore:") {
                        permitidas.push(primeira);
                    } else {
                        linhas.push(resto);
                    }
                } else if texto.len() >= 4 {
                    let miolo = &texto[2..texto.len() - 2];
                    linhas.extend(miolo.split('\n').map(|x| x.strip_suffix('\r').unwrap_or(x)));
                }
                for (i, valor) in linhas.iter().enumerate() {
                    if valor.contains('/') || valor.contains('\\') {
                        permitidas.push(primeira + i);
                    }
                }
            }
            for (linha, span) in longas {
                if !permitidas.contains(&linha) {
                    relatar(&c::LINES_LONGER_THAN_80_CHARS, span, &[]);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn so(regra: &str, fonte: &str) -> Vec<String> {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        let mut relatos = executar(u, &nomes, &|r| r == regra);
        relatos.sort_by_key(|r| (r.span.start, r.span.end));
        relatos.into_iter().map(|r| fonte[r.span.start..r.span.end].to_string()).collect()
    }

    #[test]
    fn aspas() {
        let fonte = "import \"dart:io\";\nvar a = \"x\";\nvar b = \"it's\";\nvar c = 'y';\nvar d = \"p$a\";\nvar e = \"q${a == 'x' ? 'y' : \"z\"}\";\n";
        assert_eq!(so("prefer_single_quotes", fonte), vec!["\"dart:io\"".to_string(), "\"x\"".to_string(), "\"p$a\"".to_string()]);
        assert_eq!(so("prefer_double_quotes", "var a = 'x';\nvar b = 'say \"hi\"';\nvar c = \"y\";\n"), vec!["'x'".to_string()]);
        assert_eq!(
            so("avoid_escaping_inner_quotes", "var a = 'it\\'s';\nvar b = 'it\\'s \"x\"';\nvar c = \"it's\";\n"),
            vec!["'it\\'s'".to_string()]
        );
    }

    #[test]
    fn escapes() {
        // `\"` em aspas simples, `\a`; `\n`, `\'` e `\$` ficam.
        assert_eq!(
            so("unnecessary_string_escapes", "var a = 'x\\\"y\\az\\n\\'\\$';\n"),
            vec!["\\".to_string(), "\\".to_string()]
        );
        // Em três aspas, a aspa própria sozinha não pede escape.
        assert_eq!(so("unnecessary_string_escapes", "var a = '''x\\'y''';\n").len(), 1);
        assert!(so("unnecessary_string_escapes", "var a = r'x\\ay';\n").is_empty());
        assert_eq!(so("use_raw_strings", "var a = 'a\\\\b\\$c';\nvar b = 'a\\\\b\\n';\nvar c = 'abc';\n"), vec!["'a\\\\b\\$c'".to_string()]);
    }

    #[test]
    fn linhas() {
        assert_eq!(
            so("leading_newlines_in_multiline_strings", "var a = '''x\ny''';\nvar b = '''\nx''';\nvar c = '''x''';\n"),
            vec!["'''x\ny'''".to_string()]
        );
        let longa = format!("var a = {};\n", "1 + ".repeat(20) + "1");
        let relatos = so("lines_longer_than_80_chars", &longa);
        assert_eq!(relatos.len(), 1);
        assert_eq!(relatos[0].len(), longa.len() - 1 - 80);
        // A linha com URI numa string ou num comentário é permitida.
        let com_uri = format!("var a = 'https://exemplo/{}';\n// ver http://{}\n", "x".repeat(80), "y".repeat(80));
        assert!(so("lines_longer_than_80_chars", &com_uri).is_empty());
    }
}
