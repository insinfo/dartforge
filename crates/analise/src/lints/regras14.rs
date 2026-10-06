//! O décimo quarto lote de regras de lint (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), escritas direto dos emissores da 3.6.2
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`): as do conjunto
//! `core` que não pedem subtipagem.
//!
//! * `dangling_library_doc_comments` e `unintended_html_in_doc_comment` leem
//!   os comentários de documentação como o `AstBuilder` os liga
//!   (`_findComment`: o `findDartDoc` antes do token depois das anotações,
//!   senão antes de cada anotação, da última à primeira; os tokens do
//!   `Comment` são o inicial e, numa sequência de `///`, todos os `///`
//!   seguintes da cadeia) nos nós que o recebem: diretivas, declarações de
//!   topo, membros, constantes de enum, parâmetros de tipo, parâmetros
//!   formais (antes do `this`, do tipo ou do nome), listas de variáveis
//!   locais (antes do nome da primeira) e declarações por padrão. Funções
//!   locais não recebem comentário. Os blocos de código são os do
//!   `DocCommentBuilder` (cercados e indentados), e o padrão de
//!   `_markdownTokenPattern` é casado à mão, alternativa por alternativa,
//!   com a semântica da `RegExp` da VM (`caseSensitive: false`, sem
//!   `unicode`).
//! * `file_names` usa o nome do arquivo da unidade que define a biblioteca
//!   (`library2.firstFragment.source.shortName`), também nas partes.
//! * `valid_regexps` valida o literal com o porte do analisador da VM
//!   (`super::regexp_vm`).
//! * `library_annotations` lê o `targetKinds` (`crate::meta`) e o
//!   `@pragma('dart2js:late:trust')`.
//! * `no_wildcard_variable_uses`: o identificador cujo elemento é variável
//!   local ou parâmetro; o lado esquerdo de uma atribuição e o operando de
//!   `++`/`--` não têm elemento (`readElement`/`writeElement` ficam no pai).
//! * `type_literal_in_constant_pattern`: o tipo estático `Type` do
//!   `dart:core`.
//! * `avoid_types_as_parameter_names`: o `resolveNameInScope` a partir do
//!   primeiro ancestral com escopo do `ScopeResolverVisitor`, por regiões: os
//!   parâmetros de tipo de cada função valem nela inteira; os parâmetros, no
//!   corpo; os locais de um bloco (variáveis, declarações por padrão e
//!   funções locais), no bloco inteiro; os de um `case`, no `case`; os de um
//!   `for`, no comando; os de um `catch`, no corpo; os membros declarados de
//!   uma classe (o `InstanceScope`), nos métodos e nos corpos de construtor
//!   (a lista de parâmetros de um construtor e os inicializadores de campo
//!   param no escopo de parâmetros de tipo da classe); fora de tudo, o escopo
//!   da unidade. Desvio conhecido: as variáveis de `for` e de `if case` de
//!   elementos de coleção e as de casos de `switch` em expressão não entram
//!   nas regiões.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassKind, Element};
use dartforge_frontend::ast::{
    self, Annotation, Ast, DeclKind, DirectiveKind, ExprId, ExprKind, ForInit, MemberKind, Parameter, PatternKind, StmtId, StmtKind, TypeKind,
    TypedefKind, UnaryOp,
};
use dartforge_frontend::comentarios::Comentarios;
use dartforge_frontend::fonte::pular_brancos;
use dartforge_intern::{Interner, SymbolId};

/// `LineInfo.fromContent`: os inícios de linha (depois de `\n`, `\r\n` e
/// `\r` sozinho).
fn inicios_de_linha(fonte: &str) -> Vec<usize> {
    let b = fonte.as_bytes();
    let mut v = vec![0usize];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\r' => {
                if b.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                v.push(i + 1);
            }
            b'\n' => v.push(i + 1),
            _ => {}
        }
        i += 1;
    }
    v
}

fn linha(inicios: &[usize], pos: usize) -> usize {
    inicios.partition_point(|&x| x <= pos)
}

/// `DocumentationCommentToken`: `///…` ou `/**…`.
fn e_doc(fonte: &str, s: Span) -> bool {
    let t = &fonte[s.start..s.end];
    t.starts_with("///") || t.starts_with("/**")
}

/// Um `Comment` de documentação ligado a um nó.
pub(super) struct Doc {
    /// Os tokens do `Comment`.
    pub(super) tokens: Vec<Span>,
    /// A cadeia de comentários do token inicial em diante.
    pub(super) cadeia: Vec<Span>,
    /// O token antes do qual a cadeia está (o `parent` dos comentários).
    pub(super) pai: usize,
}

/// Um bloco de código de um comentário de documentação (`MdCodeBlock`).
pub(super) struct BlocoDeCodigo {
    pub(super) cercado: bool,
    pub(super) tem_info: bool,
    /// As linhas: (posição, tamanho).
    pub(super) linhas: Vec<(usize, usize)>,
}

/// As linhas do `_CharacterSequence` de um comentário: (posição, conteúdo).
pub(super) fn linhas_do_doc<'f>(fonte: &'f str, doc: &Doc) -> Vec<(usize, &'f str)> {
    let primeiro = doc.tokens[0];
    let lexema = &fonte[primeiro.start..primeiro.end];
    if lexema.starts_with("///") {
        return doc.tokens.iter().map(|s| (s.start + 3, &fonte[s.start + 3..s.end])).collect();
    }
    // `_CharacterSequenceFromMultiLineComment`.
    let b = lexema.as_bytes();
    let branco = |c: u8| matches!(c, b' ' | b'\n' | b'\r' | b'\t');
    let mut v = Vec::new();
    let fim0 = lexema.find('\n').unwrap_or(lexema.len());
    v.push((primeiro.start, &lexema[..fim0]));
    let mut fim = fim0;
    loop {
        let mut o = fim + 1;
        if o >= b.len() {
            break;
        }
        while branco(b[o]) {
            o += 1;
            if o >= b.len() {
                return v;
            }
        }
        let fim_linha = lexema[o..].find('\n').map_or(lexema.len(), |k| o + k);
        fim = fim_linha;
        if lexema[o..].starts_with("* ") {
            o += 2;
        } else if fim == o + 1 && b[o] == b'*' {
            o += 1;
        }
        v.push((primeiro.start + o, &lexema[o.min(fim_linha)..fim_linha]));
    }
    v
}

/// Os blocos de código do `_parseDocComment` (indentados e cercados).
pub(super) fn blocos_de_codigo(linhas: &[(usize, &str)]) -> Vec<BlocoDeCodigo> {
    let mut blocos = Vec::new();
    let mut anterior_vazia = true;
    let mut i = 0;
    while i < linhas.len() {
        let (pos, conteudo) = linhas[i];
        if anterior_vazia && brancos_iniciais(conteudo) >= 4 {
            let mut b = BlocoDeCodigo { cercado: false, tem_info: false, linhas: vec![(pos, conteudo.len())] };
            i += 1;
            while i < linhas.len() && brancos_iniciais(linhas[i].1) >= 4 {
                b.linhas.push((linhas[i].0, linhas[i].1.len()));
                i += 1;
            }
            blocos.push(b);
            if i < linhas.len() {
                anterior_vazia = linhas[i].1.is_empty();
            }
            continue;
        }
        if let Some(mut k) = delimitador_cercado(conteudo, 3) {
            let bs = conteudo.as_bytes();
            let mut crases = 0;
            while k < bs.len() && bs[k] == b'`' {
                crases += 1;
                k += 1;
            }
            let tem_info = k < bs.len() && !conteudo[k..].trim().is_empty();
            let mut b = BlocoDeCodigo { cercado: true, tem_info, linhas: vec![(pos, conteudo.len())] };
            i += 1;
            while i < linhas.len() {
                b.linhas.push((linhas[i].0, linhas[i].1.len()));
                if delimitador_cercado(linhas[i].1, crases).is_some() {
                    break;
                }
                i += 1;
            }
            blocos.push(b);
            anterior_vazia = false;
        } else {
            anterior_vazia = conteudo.is_empty();
        }
        i += 1;
    }
    blocos
}

/// `_findComment(metadata, tokenAfterMetadata)`.
pub(super) fn doc_de(fonte: &str, comentarios: &Comentarios, metadata: &[Annotation], depois: usize) -> Option<Doc> {
    let montar = |pai: usize| -> Option<Doc> {
        let inicio = comentarios.dart_doc(fonte, pai)?;
        let cadeia: Vec<Span> = comentarios.antes_de(fonte, pai).into_iter().filter(|s| s.start >= inicio.start).collect();
        let mut tokens = vec![inicio];
        if fonte[inicio.start..inicio.end].starts_with("///") {
            tokens.extend(cadeia.iter().skip(1).filter(|s| fonte[s.start..s.end].starts_with("///")).copied());
        }
        Some(Doc { tokens, cadeia, pai })
    };
    montar(depois).or_else(|| metadata.iter().rev().find_map(|m| montar(m.span.start)))
}

/// O token depois das anotações.
pub(super) fn depois_das_anotacoes(fonte: &str, metadata: &[Annotation], inicio: usize) -> usize {
    match metadata.last() {
        Some(m) => pular_brancos(fonte.as_bytes(), m.span.end),
        None => inicio,
    }
}

/// Pula a palavra `augment` (o `_findComment` de `enum`, `extension`,
/// `extension type`, `typedef` e `mixin` parte da palavra-chave).
pub(super) fn sem_augment(fonte: &str, pos: usize) -> usize {
    if fonte[pos..].starts_with("augment") && !fonte.as_bytes().get(pos + 7).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'$') {
        return pular_brancos(fonte.as_bytes(), pos + 7);
    }
    pos
}

/// O token que o `endFormalParameter` usa: o `this`, o começo do tipo, ou o
/// nome.
fn token_do_parametro(fonte: &str, a: &Ast, p: &Parameter) -> Option<usize> {
    if p.this_
        && let Some(n) = p.name
    {
        return fonte[..n.span.start].rfind("this");
    }
    if let Some(t) = p.ty {
        return Some(a.ty(t).span.start);
    }
    p.name.map(|n| n.span.start)
}

/// Todas as listas de parâmetros da unidade (as aninhadas também).
fn listas_de_parametros(a: &Ast) -> Vec<&[Parameter]> {
    fn com_aninhadas<'x>(ps: &'x [Parameter], v: &mut Vec<&'x [Parameter]>) {
        v.push(ps);
        for p in ps {
            if let Some(inner) = &p.function_parameters {
                com_aninhadas(inner, v);
            }
        }
    }
    let mut v: Vec<&[Parameter]> = Vec::new();
    for f in a.functions.iter() {
        if let Some(ps) = &f.parameters {
            com_aninhadas(ps, &mut v);
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            com_aninhadas(&k.parameters, &mut v);
        }
    }
    for t in a.types.iter() {
        if let TypeKind::Function { parameters, .. } = &t.kind {
            com_aninhadas(parameters, &mut v);
        }
    }
    for d in a.decls.iter() {
        if let DeclKind::Typedef(x) = &d.kind
            && let TypedefKind::Legacy { parameters, .. } = &x.kind
        {
            com_aninhadas(parameters, &mut v);
        }
    }
    v
}

/// Todos os parâmetros de tipo da unidade.
fn parametros_de_tipo(a: &Ast) -> Vec<&ast::TypeParameter> {
    let mut v: Vec<&ast::TypeParameter> = Vec::new();
    for d in a.decls.iter() {
        match &d.kind {
            DeclKind::Class(x) => v.extend(x.type_params.iter()),
            DeclKind::Mixin(x) => v.extend(x.type_params.iter()),
            DeclKind::Enum(x) => v.extend(x.type_params.iter()),
            DeclKind::Extension(x) => v.extend(x.type_params.iter()),
            DeclKind::ExtensionType(x) => v.extend(x.type_params.iter()),
            DeclKind::Typedef(x) => v.extend(x.type_params.iter()),
            _ => {}
        }
    }
    for f in a.functions.iter() {
        v.extend(f.type_params.iter());
    }
    for t in a.types.iter() {
        if let TypeKind::Function { type_params, .. } = &t.kind {
            v.extend(type_params.iter());
        }
    }
    for ps in listas_de_parametros(a) {
        for p in ps {
            v.extend(p.function_type_params.iter());
        }
    }
    v
}

/// Os comentários de documentação de todos os nós que o `AstBuilder` liga,
/// sem repetição.
pub(super) fn docs_da_unidade(u: Unidade<'_>, comentarios: &Comentarios) -> Vec<Doc> {
    let a = u.ast;
    let fonte = u.fonte;
    let mut docs: Vec<Doc> = Vec::new();
    let juntar = |d: Option<Doc>, docs: &mut Vec<Doc>| {
        if let Some(d) = d
            && !docs.iter().any(|x| x.tokens[0] == d.tokens[0])
        {
            docs.push(d);
        }
    };
    for d in &u.unit.directives {
        let pos = depois_das_anotacoes(fonte, &d.metadata, d.span.start);
        juntar(doc_de(fonte, comentarios, &d.metadata, pos), &mut docs);
    }
    for d in a.decls.iter() {
        let mut pos = depois_das_anotacoes(fonte, &d.metadata, d.span.start);
        if d.augment && matches!(d.kind, DeclKind::Enum(_) | DeclKind::Extension(_) | DeclKind::ExtensionType(_) | DeclKind::Typedef(_) | DeclKind::Mixin(_)) {
            pos = sem_augment(fonte, pos);
        }
        juntar(doc_de(fonte, comentarios, &d.metadata, pos), &mut docs);
        if let DeclKind::Enum(x) = &d.kind {
            for k in x.constants.iter() {
                juntar(doc_de(fonte, comentarios, &k.metadata, k.name.span.start), &mut docs);
            }
        }
    }
    for m in a.members.iter() {
        let pos = depois_das_anotacoes(fonte, &m.metadata, m.span.start);
        juntar(doc_de(fonte, comentarios, &m.metadata, pos), &mut docs);
    }
    for tp in parametros_de_tipo(a) {
        juntar(doc_de(fonte, comentarios, &tp.metadata, tp.name.span.start), &mut docs);
    }
    for ps in listas_de_parametros(a) {
        for p in ps {
            if let Some(pos) = token_do_parametro(fonte, a, p) {
                juntar(doc_de(fonte, comentarios, &p.metadata, pos), &mut docs);
            }
        }
    }
    let metadados = |s: StmtId| -> &[Annotation] { a.metadados_locais.iter().find(|(x, _)| *x == s).map_or(&[], |(_, m)| &m[..]) };
    for (k, s) in a.stmts.iter().enumerate() {
        let id = StmtId(k as u32);
        match &s.kind {
            StmtKind::Variables(l) => {
                if let Some(v) = l.variables.first() {
                    juntar(doc_de(fonte, comentarios, metadados(id), v.name.span.start), &mut docs);
                }
            }
            StmtKind::For { init: Some(ForInit::Variables(l)), .. } => {
                if let Some(v) = l.variables.first() {
                    juntar(doc_de(fonte, comentarios, &[], v.name.span.start), &mut docs);
                }
            }
            StmtKind::PatternVariables { .. } => {
                let meta = metadados(id);
                let pos = depois_das_anotacoes(fonte, meta, s.span.start);
                juntar(doc_de(fonte, comentarios, meta, pos), &mut docs);
            }
            _ => {}
        }
    }
    docs
}

/// As tags HTML que o `rule.reportLintForOffset` aceita (`_validHtmlTags`),
/// na ordem da alternância.
const TAGS_HTML: [&str; 98] = [
    "a", "abbr", "address", "area", "article", "aside", "audio", "b", "bdi", "bdo", "blockquote", "br", "button", "canvas", "caption",
    "cite", "code", "col", "colgroup", "data", "datalist", "dd", "del", "dfn", "div", "dl", "dt", "em", "fieldset", "figcaption",
    "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hr", "i", "iframe", "img", "input", "ins", "kbd", "keygen",
    "label", "legend", "li", "link", "main", "map", "mark", "meta", "meter", "nav", "noscript", "object", "ol", "optgroup", "option",
    "output", "p", "param", "pre", "progress", "q", "s", "samp", "script", "section", "select", "small", "source", "span", "strong",
    "style", "sub", "sup", "table", "tbody", "td", "template", "textarea", "tfoot", "th", "thead", "time", "title", "tr", "track", "u",
    "ul", "var", "video", "wbr",
];

fn igual_sem_caixa(b: &[u8], i: usize, s: &[u8]) -> bool {
    b.len() >= i + s.len() && b[i..i + s.len()].eq_ignore_ascii_case(s)
}

fn achar(b: &[u8], de: usize, s: &[u8]) -> Option<usize> {
    if de > b.len() {
        return None;
    }
    b[de..].windows(s.len()).position(|w| w == s).map(|k| de + k)
}

/// `_markdownTokenPattern` casado em `i` (a alternância na ordem):
/// `Some((fim, nh))`, com `nh` quando casou a alternativa das tags
/// indevidas.
fn casar_markdown(t: &str, i: usize) -> Option<(usize, bool)> {
    let b = t.as_bytes();
    let n = b.len();
    match b[i] {
        // `\\.`: o `.` sem `dotAll` não casa os terminadores de linha.
        b'\\' => {
            let c = t[i + 1..].chars().next()?;
            if matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                return None;
            }
            Some((i + 1 + c.len_utf8(), false))
        }
        // `(?<cq>`+)(?:[^]+?\k<cq>)?`.
        b'`' => {
            let mut k = i;
            while k < n && b[k] == b'`' {
                k += 1;
            }
            let q = k - i;
            let mut j = k + 1;
            while j + q <= n {
                if b[j..j + q].iter().all(|&x| x == b'`') {
                    return Some((j + q, false));
                }
                j += 1;
            }
            Some((k, false))
        }
        // `(?<!\])\[[^\]]*\](?![(\[])`.
        b'[' => {
            if i > 0 && b[i - 1] == b']' {
                return None;
            }
            let fecha = achar(b, i + 1, b"]")?;
            if matches!(b.get(fecha + 1), Some(b'(' | b'[')) {
                return None;
            }
            Some((fecha + 1, false))
        }
        b'<' => {
            let letra = |k: usize| b.get(k).is_some_and(u8::is_ascii_alphabetic);
            // Autolink: `<[a-z][a-z\d\-+.]+:[^\x00-\x20\x7f<>]*>`.
            if letra(i + 1) {
                let mut j = i + 2;
                while j < n && (b[j].is_ascii_alphanumeric() || matches!(b[j], b'-' | b'+' | b'.')) {
                    j += 1;
                }
                if j > i + 2 && b.get(j) == Some(&b':') {
                    let mut k = j + 1;
                    while k < n && !(b[k] <= 0x20 || b[k] == 0x7F || b[k] == b'<' || b[k] == b'>') {
                        k += 1;
                    }
                    if b.get(k) == Some(&b'>') {
                        return Some((k + 1, false));
                    }
                }
            }
            // Comentário HTML: `<!--(?:-?>|[^]*?-->)`.
            if b[i..].starts_with(b"<!--") {
                let j = i + 4;
                if b[j..].starts_with(b"->") {
                    return Some((j + 2, false));
                }
                if b.get(j) == Some(&b'>') {
                    return Some((j + 1, false));
                }
                if let Some(k) = achar(b, j, b"-->") {
                    return Some((k + 3, false));
                }
            }
            // Declaração: `<![a-z][^]*?!>`.
            if b[i..].starts_with(b"<!")
                && letra(i + 2)
                && let Some(k) = achar(b, i + 3, b"!>")
            {
                return Some((k + 2, false));
            }
            // Instrução de processamento: `<\?[^]*?\?>`.
            if b[i..].starts_with(b"<?")
                && let Some(k) = achar(b, i + 2, b"?>")
            {
                return Some((k + 2, false));
            }
            // CDATA: `<\[CDATA[^]*\]>` (guloso: o último `]>`).
            if b[i..].starts_with(b"<[") && igual_sem_caixa(b, i + 2, b"cdata") {
                let de = i + 7;
                if de <= n
                    && let Some(k) = b[de..].windows(2).rposition(|w| w == b"]>")
                {
                    return Some((de + k + 2, false));
                }
            }
            // Tag válida: `<(?<et>/?)(?:tags)(?:/(?=\k<et>)>|>|[\x20\r\n\t][^]*?>)`.
            let fecha_tag = b.get(i + 1) == Some(&b'/');
            let p = if fecha_tag { i + 2 } else { i + 1 };
            for tag in TAGS_HTML.iter() {
                if !igual_sem_caixa(b, p, tag.as_bytes()) {
                    continue;
                }
                let q = p + tag.len();
                if !fecha_tag && b.get(q) == Some(&b'/') && b.get(q + 1) == Some(&b'>') {
                    return Some((q + 2, false));
                }
                if b.get(q) == Some(&b'>') {
                    return Some((q + 1, false));
                }
                if matches!(b.get(q), Some(b' ' | b'\r' | b'\n' | b'\t'))
                    && let Some(k) = achar(b, q + 1, b">")
                {
                    return Some((k + 1, false));
                }
            }
            // As indevidas: `</?[a-z][^]*?>`.
            let q = if b.get(i + 1) == Some(&b'/') && letra(i + 2) {
                i + 3
            } else if letra(i + 1) {
                i + 2
            } else {
                return None;
            };
            let k = achar(b, q, b">")?;
            Some((k + 1, true))
        }
        _ => None,
    }
}

/// `_findUnintendedHtmlTags`: `(início, tamanho)` relativos ao texto.
fn tags_indevidas(t: &str) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut i = 0;
    while i < t.len() {
        match casar_markdown(t, i) {
            Some((fim, nh)) => {
                if nh {
                    v.push((i, fim - i));
                }
                i = fim.max(i + 1);
            }
            None => i += 1,
        }
    }
    v
}

/// `_readWhitespace` com o `isWhitespace` do parser (espaço, `\n`, `\r`,
/// `\t`).
fn brancos_iniciais(s: &str) -> usize {
    s.bytes().take_while(|c| matches!(c, b' ' | b'\n' | b'\r' | b'\t')).count()
}

/// `_fencedCodeBlockDelimiter`: o índice das crases, ou nada.
fn delimitador_cercado(s: &str, minimo: usize) -> Option<usize> {
    if s.is_empty() {
        return None;
    }
    let i = brancos_iniciais(s);
    if i + 3 > s.len() {
        return None;
    }
    if s.as_bytes()[i..i + 3] == *"`".repeat(minimo).as_bytes() { Some(i) } else { None }
}

/// As linhas de uma sequência de `///` (os conteúdos depois das barras) que
/// estão em bloco de código (`_parseDocComment`).
fn linhas_em_bloco_de_codigo(linhas: &[&str]) -> Vec<bool> {
    let mut marcadas = vec![false; linhas.len()];
    let mut anterior_vazia = true;
    let mut i = 0;
    while i < linhas.len() {
        let conteudo = linhas[i];
        if anterior_vazia && brancos_iniciais(conteudo) >= 4 {
            // `_parseIndentedCodeBlock`.
            marcadas[i] = true;
            i += 1;
            while i < linhas.len() && brancos_iniciais(linhas[i]) >= 4 {
                marcadas[i] = true;
                i += 1;
            }
            if i < linhas.len() {
                anterior_vazia = linhas[i].is_empty();
            }
            continue;
        }
        if let Some(mut k) = delimitador_cercado(conteudo, 3) {
            // `_parseFencedCodeBlock`.
            let b = conteudo.as_bytes();
            let mut crases = 0;
            while k < b.len() && b[k] == b'`' {
                crases += 1;
                k += 1;
            }
            marcadas[i] = true;
            i += 1;
            while i < linhas.len() {
                marcadas[i] = true;
                if delimitador_cercado(linhas[i], crases).is_some() {
                    break;
                }
                i += 1;
            }
            anterior_vazia = false;
        } else {
            anterior_vazia = conteudo.is_empty();
        }
        i += 1;
    }
    marcadas
}

/// `isValidDartFileName`.
fn nome_de_arquivo_valido(nome: &str) -> bool {
    let b = nome.as_bytes();
    if b.len() < 6 || !nome.ends_with(".dart") {
        return true;
    }
    let tamanho = b.len() - 5;
    if (1..tamanho.saturating_sub(1)).any(|i| b[i] == b'.') {
        return true;
    }
    for (i, &c) in b[..tamanho].iter().enumerate() {
        if !c.is_ascii_lowercase() && c != b'_' {
            if c.is_ascii_digit() {
                if i == 0 {
                    return false;
                }
                continue;
            }
            return false;
        }
    }
    true
}

/// `isJustUnderscores`.
fn so_sublinhados(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c == b'_')
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let precisa_de_comentarios = ligada("dangling_library_doc_comments") || ligada("unintended_html_in_doc_comment");
    let comentarios = if precisa_de_comentarios { Some(Comentarios::de(fonte)) } else { None };

    // `dangling_library_doc_comments`.
    if ligada("dangling_library_doc_comments")
        && let Some(comentarios) = &comentarios
    {
        let inicios = inicios_de_linha(fonte);
        let l = |pos: usize| linha(&inicios, pos);
        'regra: {
            if let Some(primeira) = u.unit.directives.first() {
                if matches!(primeira.kind, DirectiveKind::Library { .. } | DirectiveKind::PartOf { .. }) {
                    break 'regra;
                }
                let pos = depois_das_anotacoes(fonte, &primeira.metadata, primeira.span.start);
                if let Some(doc) = doc_de(fonte, comentarios, &primeira.metadata, pos) {
                    relatar(&c::DANGLING_LIBRARY_DOC_COMMENTS, doc.tokens[0], &[]);
                }
                break 'regra;
            }
            let Some(&primeira) = u.unit.declarations.first() else {
                // Sem declarações: os comentários de documentação antes do
                // fim do arquivo.
                for s in comentarios.antes_de(fonte, fonte.len()) {
                    if e_doc(fonte, s) {
                        relatar(&c::DANGLING_LIBRARY_DOC_COMMENTS, s, &[]);
                    }
                }
                break 'regra;
            };
            let d = a.decl(primeira);
            let mut pos = depois_das_anotacoes(fonte, &d.metadata, d.span.start);
            if d.augment && matches!(d.kind, DeclKind::Enum(_) | DeclKind::Extension(_) | DeclKind::ExtensionType(_) | DeclKind::Typedef(_) | DeclKind::Mixin(_)) {
                pos = sem_augment(fonte, pos);
            }
            let Some(doc) = doc_de(fonte, comentarios, &d.metadata, pos) else { break 'regra };
            let no = Span { start: doc.tokens[0].start, end: doc.tokens[doc.tokens.len() - 1].end };
            for par in doc.tokens.windows(2) {
                if l(par[1].start) > l(par[0].end) + 1 {
                    relatar(&c::DANGLING_LIBRARY_DOC_COMMENTS, par[0], &[]);
                    break 'regra;
                }
            }
            // Os comentários depois do último token do `Comment`.
            let ultimo = doc.tokens[doc.tokens.len() - 1];
            let mut atual = ultimo;
            for &seguinte in doc.cadeia.iter().filter(|s| s.start > ultimo.start) {
                if l(seguinte.start) > l(atual.end) + 1 {
                    relatar(&c::DANGLING_LIBRARY_DOC_COMMENTS, no, &[]);
                    break 'regra;
                }
                atual = seguinte;
            }
            if l(doc.pai) > l(atual.end) + 1 {
                relatar(&c::DANGLING_LIBRARY_DOC_COMMENTS, no, &[]);
            }
        }
    }

    // `unintended_html_in_doc_comment`.
    if ligada("unintended_html_in_doc_comment")
        && let Some(comentarios) = &comentarios
    {
        for doc in docs_da_unidade(u, comentarios) {
            let primeiro = doc.tokens[0];
            let de_linha = fonte[primeiro.start..primeiro.end].starts_with("///");
            // Num `/** … */` a primeira linha (com o `/**`) nunca é de bloco
            // de código; numa sequência de `///`, as linhas são os tokens.
            let em_bloco = if de_linha {
                let linhas: Vec<&str> = doc.tokens.iter().map(|s| &fonte[s.start + 3..s.end]).collect();
                linhas_em_bloco_de_codigo(&linhas)
            } else {
                vec![false]
            };
            for (k, s) in doc.tokens.iter().enumerate() {
                if em_bloco.get(k).copied().unwrap_or(false) {
                    continue;
                }
                for (ini, tam) in tags_indevidas(&fonte[s.start..s.end]) {
                    relatar(&c::UNINTENDED_HTML_IN_DOC_COMMENT, Span { start: s.start + ini, end: s.start + ini + tam }, &[]);
                }
            }
        }
    }

    let Some(s) = sem else { return out };
    let program = s.program;

    // `file_names`: o nome do arquivo que define a biblioteca.
    if ligada("file_names") {
        let lib = program.library(program.unit(s.unidade).library);
        if let Some(&definidora) = lib.units.first() {
            let un = program.unit(definidora);
            let nome = match &un.path {
                Some(p) => p.file_name().map(|x| x.to_string_lossy().into_owned()),
                None => un.uri.rsplit('/').next().map(str::to_string),
            };
            if let Some(nome) = nome
                && !nome_de_arquivo_valido(&nome)
            {
                relatar(&c::FILE_NAMES, Span { start: 0, end: 0 }, &[nome.as_str()]);
            }
        }
    }

    // `depend_on_referenced_packages`: o pacote (o `pubspec.yaml` mais
    // próximo do arquivo da biblioteca), o nome dele, as dependências e,
    // fora de `lib/`, `bin/` e dos ganchos `hook/build.dart` e
    // `hook/link.dart`, as de desenvolvimento.
    if ligada("depend_on_referenced_packages") {
        let lib = program.library(program.unit(s.unidade).library);
        let caminho_da_lib = lib.units.first().and_then(|x| program.unit(*x).path.clone());
        let raiz = caminho_da_lib.as_ref().and_then(|c| c.ancestors().skip(1).find(|d| d.join("pubspec.yaml").is_file()).map(std::path::Path::to_path_buf));
        if let (Some(arquivo), Some(raiz)) = (caminho_da_lib, raiz)
            && let Ok(texto) = std::fs::read_to_string(raiz.join("pubspec.yaml"))
        {
            let dentro_de = |d: &std::path::Path| arquivo.starts_with(d) && arquivo != d;
            let publico = dentro_de(&raiz.join("lib"))
                || dentro_de(&raiz.join("bin"))
                || arquivo == raiz.join("hook").join("build.dart")
                || arquivo == raiz.join("hook").join("link.dart");
            if let Some(disponiveis) = super::pubspec::dependencias_disponiveis(&texto, !publico) {
                for d in &u.unit.directives {
                    let uri = match &d.kind {
                        DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } => uri,
                        _ => continue,
                    };
                    let Some(valor) = uri.constant_value().map(|v| v.to_string_lossy()) else { continue };
                    if !valor.starts_with("package:") {
                        continue;
                    }
                    let Some(barra) = valor.find('/') else { continue };
                    let pacote = &valor[8..barra.max(8)];
                    if pacote == "flutter_gen" || disponiveis.iter().any(|x| x == pacote) {
                        continue;
                    }
                    relatar(&c::DEPEND_ON_REFERENCED_PACKAGES, uri.span, &[pacote]);
                }
            }
        }
    }

    // `valid_regexps`.
    if ligada("valid_regexps") {
        for (k, e) in a.exprs.iter().enumerate() {
            let argumentos = match &e.kind {
                ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => arguments,
                _ => continue,
            };
            let Some(dartforge_types::resolved::Resolved::Constructor(f)) = s.corpo.get_resolved(ExprId(k as u32)) else { continue };
            let Some(classe) = program.function(*f).class else { continue };
            let cl = program.class(classe);
            if interner.resolve(cl.name) != "RegExp" || program.library(cl.library).uri != "dart:core" {
                continue;
            }
            let Some(primeiro) = argumentos.args.first() else { continue };
            let unicode = argumentos.args.iter().any(|x| {
                x.name.is_some_and(|n| interner.resolve(n.sym) == "unicode") && matches!(a.expr(x.value).kind, ExprKind::Bool(true))
            });
            if primeiro.name.is_some() {
                continue;
            }
            let ExprKind::String(lit) = &a.expr(primeiro.value).kind else { continue };
            let Some(valor) = lit.constant_value() else { continue };
            let unidades: Vec<u16> = valor.code_units().collect();
            if !super::regexp_vm::valida(&unidades, unicode) {
                relatar(&c::VALID_REGEXPS, a.expr(primeiro.value).span, &[]);
            }
        }
    }

    // `library_annotations`.
    if ligada("library_annotations") {
        let primeira = u.unit.directives.first().map(|d| d.span);
        // `@pragma('dart2js:late:trust')`: o construtor de `pragma` do
        // `dart:core` com o nome constante.
        let pragma_late_trust = |m: &Annotation| -> bool {
            let simples = |n: ast::Name| program.lookup_na_unidade(s.unidade, n.sym).and_then(|b| b.getter);
            let elemento = match &m.name[..] {
                [n] => simples(*n),
                [p, n] => program.lookup_prefixed_na_unidade(s.unidade, p.sym, n.sym).and_then(|b| b.getter),
                _ => None,
            };
            let Some(Element::Class(cl)) = elemento else { return false };
            let classe = program.class(cl);
            if interner.resolve(classe.name) != "pragma" || program.library(classe.library).uri != "dart:core" {
                return false;
            }
            let Some(arg) = m.arguments.as_ref().and_then(|x| x.args.iter().find(|y| y.name.is_none())) else { return false };
            valor_constante_de_string(program, interner, s.unidade, a, arg.value).as_deref() == Some("dart2js:late:trust")
        };
        let mut conferir = |metadata: &[Annotation], e_primeira: bool| {
            for m in metadata {
                let especies = crate::meta::especies_da_anotacao(program, interner, s.unidade, m);
                if (especies.len() == 1 && especies[0] == "library" && e_primeira) || pragma_late_trust(m) {
                    relatar(&c::LIBRARY_ANNOTATIONS, m.span, &[]);
                }
            }
        };
        'regra: {
            for d in &u.unit.directives {
                if matches!(d.kind, DirectiveKind::PartOf { .. }) {
                    break 'regra;
                }
                if !matches!(d.kind, DirectiveKind::Library { .. }) {
                    conferir(&d.metadata, Some(d.span) == primeira);
                }
            }
            for &did in &u.unit.declarations {
                let d = a.decl(did);
                conferir(&d.metadata, false);
            }
        }
    }

    // `no_wildcard_variable_uses`.
    if ligada("no_wildcard_variable_uses") {
        let mut sem_elemento: std::collections::HashSet<ExprId> = std::collections::HashSet::new();
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Assign { target, .. } => {
                    sem_elemento.insert(*target);
                }
                ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                    sem_elemento.insert(*operand);
                }
                _ => {}
            }
        }
        // As funções locais (o elemento é `LocalFunctionElement`).
        let funcoes_locais: std::collections::HashSet<usize> = a
            .stmts
            .iter()
            .filter_map(|x| match &x.kind {
                StmtKind::Function(f) => a.function(*f).name.map(|n| n.span.start),
                _ => None,
            })
            .collect();
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let ExprKind::Identifier(n) = &e.kind else { continue };
            if sem_elemento.contains(&id) || !so_sublinhados(interner.resolve(n.sym)) {
                continue;
            }
            let local_ou_parametro = match s.corpo.get_resolved(id) {
                Some(dartforge_types::resolved::Resolved::Local(_)) => {
                    !s.corpo.declaracao_local(id).is_some_and(|d| funcoes_locais.contains(&d))
                }
                Some(dartforge_types::resolved::Resolved::Parameter { .. }) => true,
                _ => false,
            };
            if local_ou_parametro {
                relatar(&c::NO_WILDCARD_VARIABLE_USES, e.span, &[]);
            }
        }
    }

    // `type_literal_in_constant_pattern`.
    if ligada("type_literal_in_constant_pattern") {
        let classe_type = match s.table.get(s.core.type_) {
            dartforge_types::table::Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        for p in a.patterns.iter() {
            let PatternKind::Constant(e) = &p.kind else { continue };
            let texto = &fonte[p.span.start..p.span.end];
            if texto.starts_with("const") && !texto.as_bytes().get(5).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'$') {
                continue;
            }
            let Some(t) = s.corpo.get_type(*e) else { continue };
            if let dartforge_types::table::Type::Interface { class, .. } = s.table.get(t)
                && Some(*class) == classe_type
            {
                relatar(&c::TYPE_LITERAL_IN_CONSTANT_PATTERN, p.span, &[]);
            }
        }
    }

    // `avoid_types_as_parameter_names`.
    if ligada("avoid_types_as_parameter_names") {
        let escopos = super::escopo::Escopos::de(a);
        let e_tipo = |nome: SymbolId, pos: usize| -> bool {
            let achadas = escopos.procurar(nome, pos);
            if !achadas.is_empty() {
                // O getter do escopo: um parâmetro de tipo é tipo; o resto
                // (local, membro, ou só setter) não.
                return achadas.iter().any(|r| r.especie == super::escopo::Especie::ParametroDeTipo);
            }
            let Some(b) = program.lookup_na_unidade(s.unidade, nome) else { return false };
            if b.ambiguous {
                return false;
            }
            match b.getter {
                Some(Element::Class(cl)) => matches!(program.class(cl).kind, ClassKind::Class | ClassKind::MixinApplication | ClassKind::ExtensionType),
                Some(Element::Typedef(_)) => true,
                _ => false,
            }
        };
        for st in a.stmts.iter() {
            let StmtKind::Try { catches, .. } = &st.kind else { continue };
            for k in catches.iter() {
                if let Some(n) = k.exception
                    && e_tipo(n.sym, k.span.start)
                {
                    relatar(&c::AVOID_TYPES_AS_PARAMETER_NAMES, n.span, &[interner.resolve(n.sym)]);
                }
            }
        }
        for ps in listas_de_parametros(a) {
            for p in ps {
                let Some(n) = p.name else { continue };
                // `hasImplicitType`, fora o `this.x`.
                if p.this_ || p.ty.is_some() || p.function_parameters.is_some() {
                    continue;
                }
                if e_tipo(n.sym, p.span.start) {
                    relatar(&c::AVOID_TYPES_AS_PARAMETER_NAMES, n.span, &[interner.resolve(n.sym)]);
                }
            }
        }
    }

    out
}

/// O valor constante de uma expressão de string: o literal sem
/// interpolação, ou o identificador de uma constante de topo com esse
/// inicializador.
fn valor_constante_de_string(
    program: &dartforge_elements::model::Program,
    interner: &Interner,
    u: dartforge_elements::model::UnitId,
    a: &Ast,
    e: ExprId,
) -> Option<String> {
    match &a.expr(e).kind {
        ExprKind::String(lit) => lit.constant_value().map(|v| v.to_string_lossy()),
        ExprKind::Parenthesized(x) => valor_constante_de_string(program, interner, u, a, *x),
        ExprKind::Identifier(n) => {
            let b = program.lookup_na_unidade(u, n.sym)?;
            let v = match b.getter? {
                Element::Variable(v) => v,
                Element::Function(f) => program.function(f).variable?,
                _ => return None,
            };
            let dartforge_elements::model::VariableRef::TopLevel { unit, decl, index } = program.variable(v).node else { return None };
            let a2 = &program.unit(unit).ast;
            let DeclKind::Variables(l) = &a2.decl(decl).kind else { return None };
            if !l.const_ {
                return None;
            }
            let init = l.variables.get(index)?.initializer?;
            match &a2.expr(init).kind {
                ExprKind::String(lit) => lit.constant_value().map(|v| v.to_string_lossy()),
                _ => None,
            }
        }
        _ => None,
    }
}
