//! O `CompletionItem` do servidor do Dart 3.6.2 (`toCompletionItem`,
//! `MAP:974-1179`; docs/LSP-ESPECIFICACAO.md §14.8): rótulo, `filterText`,
//! `kind` pela preferência e o `valueSet` do cliente, `detail` e
//! `labelDetails`, o texto inserido com o snippet de chamada
//! (`_buildInsertText`), `InsertReplaceEdit`/`TextEdit`, `itemDefaults`,
//! documentação no item e o `data` dos não importados; e os snippets de
//! código (§14.8.9).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::completar::{Chamada, ItemCompletar};
use serde_json::{Value, json};

/// O que o cliente anunciou para o completar (`client_capabilities.dart:155-202`).
#[derive(Debug, Clone, Default)]
pub(crate) struct Capacidades {
    pub obsoleto: bool,
    pub tag_obsoleto: bool,
    /// `documentationFormat` (`None`: o cliente não mandou o campo).
    pub formatos_de_doc: Option<Vec<String>>,
    pub snippet: bool,
    pub inserir_substituir: bool,
    pub modos_de_insercao: Vec<u64>,
    pub detalhes_do_rotulo: bool,
    pub especies: Option<Vec<u64>>,
    /// `completionList.itemDefaults` contém `editRange` / `insertTextMode`.
    pub padrao_intervalo: bool,
    pub padrao_modo: bool,
}

impl Capacidades {
    pub(crate) fn do_initialize(c: &Value) -> Capacidades {
        let item = c.pointer("/textDocument/completion/completionItem");
        let b = |p: &str| item.and_then(|i| i.pointer(p)).and_then(Value::as_bool).unwrap_or(false);
        let padroes: Vec<String> = c
            .pointer("/textDocument/completion/completionList/itemDefaults")
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        Capacidades {
            obsoleto: b("/deprecatedSupport"),
            tag_obsoleto: item.and_then(|i| i.pointer("/tagSupport/valueSet")).and_then(Value::as_array).is_some_and(|l| l.iter().any(|x| x.as_u64() == Some(1))),
            formatos_de_doc: item.and_then(|i| i.get("documentationFormat")).and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()),
            snippet: b("/snippetSupport"),
            inserir_substituir: b("/insertReplaceSupport"),
            modos_de_insercao: item.and_then(|i| i.pointer("/insertTextModeSupport/valueSet")).and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_u64).collect()).unwrap_or_default(),
            detalhes_do_rotulo: b("/labelDetailsSupport"),
            especies: c.pointer("/textDocument/completion/completionItemKind/valueSet").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_u64).collect()),
            padrao_intervalo: padroes.iter().any(|p| p == "editRange"),
            padrao_modo: padroes.iter().any(|p| p == "insertTextMode"),
        }
    }

    /// O primeiro `kind` da preferência que o cliente aceita (sem
    /// `valueSet`, o conjunto padrão do LSP: 1 a 18).
    fn especie(&self, preferencia: &[u64]) -> Option<u64> {
        preferencia.iter().copied().find(|k| match &self.especies {
            Some(l) => l.contains(k),
            None => (1..=18).contains(k),
        })
    }

    /// A documentação no formato do cliente (`MAP:1619-1634`, `:79-85`).
    fn documentacao(&self, texto: String) -> Value {
        match &self.formatos_de_doc {
            None => json!(texto),
            Some(l) => {
                let markdown = l.is_empty() || l.iter().any(|f| f == "markdown") || !l.iter().any(|f| f == "plaintext");
                json!({"kind": if markdown { "markdown" } else { "plaintext" }, "value": texto})
            }
        }
    }
}

/// O contexto de um pedido de completar.
pub(crate) struct Pedido<'a> {
    /// `replacementRange` e `insert` (já em posições LSP).
    pub substituir: Value,
    pub inserir: Value,
    pub iguais: bool,
    /// `itemDefaults.editRange` vigente.
    pub padrao: bool,
    /// O `completeFunctionCalls` efetivo (configuração e sem lista de
    /// argumentos no alvo).
    pub chamadas: bool,
    /// O caminho do arquivo em edição (`data.file`).
    pub arquivo: &'a str,
    /// A preferência `documentation` (`none`/`summary`/`full`).
    pub preferencia_de_doc: &'a str,
}

/// `escapeSnippetPlainText`.
fn escapar(t: &str) -> String {
    t.replace('\\', "\\\\").replace('$', "\\$")
}

/// `buildSnippetStringWithTabStops(texto, pares)`.
fn snippet_com_paradas(texto: &str, pares: &[(usize, usize)]) -> String {
    let unidades: Vec<u16> = texto.encode_utf16().collect();
    let validos: Vec<(usize, usize)> = pares.iter().copied().filter(|&(o, l)| o + l <= unidades.len()).collect();
    let mut s = String::new();
    let mut pos = 0usize;
    let unico = validos.len() == 1;
    for (i, &(o, l)) in validos.iter().enumerate() {
        s.push_str(&escapar(&String::from_utf16_lossy(&unidades[pos..o])));
        let n = if unico { 0 } else { i + 1 };
        let dentro = String::from_utf16_lossy(&unidades[o..o + l]);
        if dentro.is_empty() {
            s.push_str(&format!("${n}"));
        } else {
            let e = dentro.replace('\\', "\\\\").replace('$', "\\$").replace('}', "\\}");
            s.push_str(&format!("${{{n}:{e}}}"));
        }
        pos = o + l;
    }
    s.push_str(&escapar(&String::from_utf16_lossy(&unidades[pos..])));
    s
}

/// `computeCompletionDefaultArgumentList`: o texto e os pares (UTF-16).
fn lista_padrao(parametros: &[String]) -> Option<(String, Vec<(usize, usize)>)> {
    if parametros.is_empty() {
        return None;
    }
    let mut texto = String::new();
    let mut pares = Vec::new();
    for p in parametros {
        if !texto.is_empty() {
            texto.push_str(", ");
        }
        match p.strip_suffix(": ") {
            Some(n) => {
                texto.push_str(n);
                texto.push_str(": ");
                let o = texto.encode_utf16().count();
                texto.push_str(n);
                pares.push((o, n.encode_utf16().count()));
            }
            None => {
                let o = texto.encode_utf16().count();
                texto.push_str(p);
                pares.push((o, p.encode_utf16().count()));
            }
        }
    }
    Some((texto, pares))
}

/// `getCompletionDetail`: `truncatedParams` e `truncatedSignature` de um
/// `detail` já na forma `(params) → ret` (ou só o tipo).
fn assinaturas(detalhe: Option<&str>, chamavel: bool) -> (String, String) {
    let Some(d) = detalhe.filter(|d| !d.is_empty()) else { return (String::new(), String::new()) };
    if chamavel && d.starts_with('(') {
        let mut nivel = 0usize;
        let mut fim = d.len();
        for (i, c) in d.char_indices() {
            match c {
                '(' | '<' | '[' | '{' => nivel += 1,
                ')' | '>' | ']' | '}' => {
                    nivel = nivel.saturating_sub(1);
                    if nivel == 0 && c == ')' {
                        fim = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let truncados = if &d[..fim] == "()" { "()".to_string() } else { "(…)".to_string() };
        let retorno = d[fim..].trim_start().strip_prefix('→').map(str::trim).filter(|r| !r.is_empty());
        let assinatura = match retorno {
            Some(r) => format!("{truncados} → {r}"),
            None => truncados.clone(),
        };
        return (truncados, assinatura);
    }
    (String::new(), format!(" {d}"))
}

/// `docSummary`: o primeiro parágrafo.
fn resumo(doc: &str) -> String {
    let mut linhas = Vec::new();
    for l in doc.lines() {
        if l.trim().is_empty() {
            break;
        }
        linhas.push(l);
    }
    linhas.join("\n")
}

/// A preferência de espécies de um item (`elementKindToCompletionItemKind`
/// e `suggestionKindToCompletionItemKind`).
fn preferencia(k: u32) -> Vec<u64> {
    match k {
        20 => vec![20, 13],
        25 => vec![25, 6],
        17 => vec![17, 9],
        0 => Vec::new(),
        k => vec![u64::from(k)],
    }
}

/// O `CompletionItem` de um item do servidor.
pub(crate) fn item(cap: &Capacidades, p: &Pedido<'_>, i: &ItemCompletar, documentacao: Option<String>) -> Value {
    let chamavel = i.chamada.is_some();
    // `label`, `filterText`.
    let mut rotulo = i.inserir.clone();
    let comeca_com_padrao = rotulo.starts_with("=>") || rotulo.starts_with('(');
    let filtro = if comeca_com_padrao {
        rotulo.clone()
    } else {
        rotulo.split(|c| c == '(').next().unwrap_or("").split("=>").next().unwrap_or("").trim().to_string()
    };
    if cap.detalhes_do_rotulo {
        rotulo = filtro.clone();
    }
    if rotulo.ends_with(',') {
        rotulo.pop();
    }
    let (truncados, assinatura) = assinaturas(i.detalhe.as_deref(), chamavel);
    if !cap.detalhes_do_rotulo {
        rotulo.push_str(&truncados);
    }
    // `_buildInsertText`.
    let mut texto = i.inserir.clone();
    let mut snippet = false;
    let mut chamadas = p.chamadas && chamavel && !i.inserir.contains('(');
    if i.inserir.contains('(') {
        chamadas = false;
    }
    if cap.snippet && chamadas {
        snippet = true;
        let sufixo = match &i.chamada {
            Some(Chamada::Parametros(ps)) => match lista_padrao(ps) {
                Some((t, pares)) => snippet_com_paradas(&t, &pares),
                None => String::new(),
            },
            Some(Chamada::Desconhecida) => "$0".to_string(),
            None => String::new(),
        };
        texto = format!("{}({sufixo})", escapar(&i.inserir));
    }
    let mut v = json!({"label": rotulo});
    if let Some(k) = cap.especie(&preferencia(i.especie)) {
        v["kind"] = json!(k);
    }
    // `detail`: a assinatura completa.
    if let Some(d) = i.detalhe.as_ref().filter(|d| !d.is_empty()) {
        v["detail"] = json!(d);
    }
    if cap.detalhes_do_rotulo {
        let mut m = serde_json::Map::new();
        if !assinatura.is_empty() {
            m.insert("detail".into(), json!(assinatura));
        }
        if let Some(imp) = &i.importar {
            m.insert("description".into(), json!(uri_de_exibicao(&imp.uri, p.arquivo)));
        }
        if !m.is_empty() {
            v["labelDetails"] = Value::Object(m);
        }
    }
    // Documentação: só sem `data` (os não importados resolvem depois).
    if i.importar.is_none()
        && let Some(doc) = documentacao
    {
        let doc = match p.preferencia_de_doc {
            "none" => None,
            "summary" => Some(resumo(&doc)),
            _ => Some(doc),
        };
        if let Some(d) = doc.filter(|d| !d.is_empty()) {
            v["documentation"] = cap.documentacao(d);
        }
    }
    v["sortText"] = json!(i.sort_text);
    if filtro != v["label"].as_str().unwrap_or("") {
        v["filterText"] = json!(filtro);
    }
    if snippet {
        v["insertTextFormat"] = json!(2);
    }
    if !cap.padrao_modo && cap.modos_de_insercao.contains(&1) && texto.contains('\n') {
        v["insertTextMode"] = json!(1);
    }
    if p.padrao {
        if texto != v["label"].as_str().unwrap_or("") {
            v["textEditText"] = json!(texto);
        }
    } else if cap.inserir_substituir && !p.iguais {
        v["textEdit"] = json!({"insert": p.inserir, "replace": p.substituir, "newText": texto});
    } else {
        v["textEdit"] = json!({"range": p.substituir, "newText": texto});
    }
    if let Some(imp) = &i.importar {
        let nome = i.inserir.trim_end_matches([':', ' ']);
        v["data"] = json!({"file": p.arquivo, "importUris": [imp.uri], "ref": format!("{};{};{}", imp.uri, imp.uri, nome)});
    }
    v
}

/// `getCompletionDisplayUriString`: `file:` relativo à pasta do arquivo.
pub(crate) fn uri_de_exibicao(uri: &str, arquivo: &str) -> String {
    if let Ok(u) = url::Url::parse(uri)
        && u.scheme() == "file"
        && let Ok(alvo) = u.to_file_path()
        && let Some(pasta) = std::path::Path::new(arquivo).parent()
    {
        return crate::refatoracoes_metodo::caminho_relativo(&alvo, pasta);
    }
    if !uri.contains(':') {
        return uri.to_string();
    }
    uri.to_string()
}

/// O contexto do snippet (`DartSnippetRequest._getContext`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextoDeSnippet {
    Topo,
    Bloco,
    Classe,
    Expressao,
    Outro,
}

/// Os snippets de código (§14.8.9) do contexto, filtrados pelo prefixo:
/// (prefixo, rótulo, documentação, corpo).
pub(crate) fn snippets(contexto: ContextoDeSnippet, recuo: &str, eol: &str, em_testes: bool, finais: bool) -> Vec<(&'static str, &'static str, &'static str, String)> {
    let q = |s: &str| s.replace("⏎", &format!("{eol}{recuo}"));
    let var = if finais { "final" } else { "var" };
    let mut v: Vec<(&'static str, &'static str, &'static str, String)> = Vec::new();
    match contexto {
        ContextoDeSnippet::Topo => {
            v.push(("class", "class", "Insert a class definition.", q("class ${1:ClassName} {⏎  $0⏎}")));
            v.push(("fun", "fun", "Insert a function definition.", q("${1:void} ${2:name}(${3:params}) {⏎  $0⏎}")));
            let main = if em_testes { "void main() {\n  $0\n}".to_string() } else { "void main(List<String> args) {\n  $0\n}".to_string() };
            v.push(("main", "main()", "Insert a main function, used as an entry point.", main));
        }
        ContextoDeSnippet::Bloco => {
            v.push(("do", "do while", "Insert a do-while loop.", q("do {⏎  $0⏎} while (${1:condition});")));
            v.push(("forin", "for in", "Insert a for-in loop.", q(&format!("for ({var} ${{1:element}} in ${{2:collection}}) {{⏎  $0⏎}}"))));
            v.push(("for", "for", "Insert a for loop.", q("for (var i = 0; i < ${1:count}; i++) {⏎  $0⏎}")));
            v.push(("fun", "fun", "Insert a function definition.", q("${1:void} ${2:name}(${3:params}) {⏎  $0⏎}")));
            v.push(("ife", "ife", "Insert an if/else statement.", q("if (${1:condition}) {⏎  $0⏎} else {⏎  ⏎}")));
            v.push(("if", "if", "Insert an if statement.", q("if (${1:condition}) {⏎  $0⏎}")));
            v.push(("switch", "switch statement", "Insert a switch statement.", q("switch (${1:expression}) {⏎  case ${2:value}:⏎    $0⏎    break;⏎  default:⏎}")));
            if em_testes {
                v.push(("test", "test", "Insert a test block.", q("test('${1:test name}', () {⏎  $0⏎});")));
                v.push(("group", "group", "Insert a test group block.", q("group('${1:group name}', () {⏎  $0⏎});")));
            }
            v.push(("try", "try", "Insert a try/catch statement.", q("try {⏎  $0⏎} catch (${1:e}) {⏎  ⏎}")));
            v.push(("while", "while", "Insert a while loop.", q("while (${1:condition}) {⏎  $0⏎}")));
        }
        ContextoDeSnippet::Classe => {
            v.push(("fun", "fun", "Insert a function definition.", q("${1:void} ${2:name}(${3:params}) {⏎  $0⏎}")));
        }
        ContextoDeSnippet::Expressao => {
            v.push(("switch", "switch expression", "Insert a switch expression.", q("switch (${1:expression}) {⏎  ${2:pattern} => ${3:value},$0⏎}")));
        }
        ContextoDeSnippet::Outro => {}
    }
    v
}

/// O item de um snippet (`snippetToCompletionItem`, `MAP:788-878`).
pub(crate) fn item_de_snippet(cap: &Capacidades, p: &Pedido<'_>, prefixo: &str, rotulo: &str, doc: &str, corpo: &str) -> Value {
    let mut v = json!({"label": rotulo, "kind": 15, "sortText": format!("zzz{prefixo}"), "insertTextFormat": 2});
    if prefixo != rotulo {
        v["filterText"] = json!(prefixo);
    }
    v["documentation"] = cap.documentacao(doc.to_string());
    if cap.modos_de_insercao.contains(&1) {
        v["insertTextMode"] = json!(1);
    }
    if p.padrao {
        v["textEditText"] = json!(corpo);
    } else {
        v["textEdit"] = json!({"insertTextFormat": 2, "range": p.substituir, "newText": corpo});
    }
    v
}

/// O alvo do completar (`CompletionTarget.forOffset`,
/// `completion_target.dart:141-260`) sobre a árvore sintática: o nó que
/// contém e a entidade (nó ou token).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Entidade {
    No(usize),
    Token(usize),
    Nenhuma,
}

/// O contexto do snippet do texto (`DartSnippetRequest._getContext`); `None`
/// em comentário.
pub(crate) fn contexto_de_snippet(texto: &str, offset: usize) -> Option<ContextoDeSnippet> {
    use dartforge_frontend::token::Kind;
    // Comentário: `inComment`.
    let comentarios = dartforge_frontend::comentarios::Comentarios::de(texto);
    for c in comentarios.todos() {
        let linha = texto[c.start..c.end].starts_with("//");
        if c.start < offset && (offset < c.end || (linha && offset == c.end)) {
            return Some(ContextoDeSnippet::Outro);
        }
    }
    let mut nomes = dartforge_intern::Interner::new();
    let analisado = dartforge_frontend::parser::parse(texto, &mut nomes);
    let arv = crate::arvore_analyzer::construir(texto, &analisado.ast, &analisado.unit, false);
    let tokens: Vec<dartforge_frontend::token::Token> = dartforge_frontend::lexer::lex(texto).unwrap_or_default().into_iter().filter(|t| t.kind != Kind::Eof).collect();
    let palavra = |t: &dartforge_frontend::token::Token| matches!(t.kind, Kind::Ident | Kind::Keyword(_));
    let candidato = |t: &dartforge_frontend::token::Token| offset < t.span.end || (offset == t.span.end && (palavra(t) || t.span.start == t.span.end));
    let (mut cont, mut entidade) = (0usize, Entidade::Nenhuma);
    'externo: loop {
        let no = &arv.nos[cont];
        // As entidades: os filhos e os tokens fora deles, em ordem.
        let mut ents: Vec<(usize, Entidade)> = no.filhos.iter().map(|&f| (arv.nos[f].inicio, Entidade::No(f))).collect();
        let (ini, fim) = if cont == 0 { (0, texto.len()) } else { (no.inicio, no.fim) };
        for (k, t) in tokens.iter().enumerate() {
            if t.span.start < ini || t.span.end > fim {
                continue;
            }
            if no.filhos.iter().any(|&f| arv.nos[f].inicio <= t.span.start && t.span.end <= arv.nos[f].fim) {
                continue;
            }
            ents.push((t.span.start, Entidade::Token(k)));
        }
        ents.sort_by_key(|(o, _)| *o);
        for (_, e) in ents {
            match e {
                Entidade::Token(k) => {
                    if candidato(&tokens[k]) {
                        entidade = Entidade::Token(k);
                        break 'externo;
                    }
                }
                Entidade::No(f) => {
                    let nf = &arv.nos[f];
                    let ultimo = tokens.iter().rev().find(|t| t.span.end <= nf.fim && t.span.start >= nf.inicio);
                    if !ultimo.is_some_and(candidato) {
                        continue;
                    }
                    let primeiro = tokens.iter().find(|t| t.span.start >= nf.inicio);
                    let e_candidato = match primeiro {
                        Some(b) if palavra(b) => candidato(b),
                        _ => offset <= nf.inicio,
                    };
                    if e_candidato {
                        entidade = Entidade::No(f);
                        break 'externo;
                    }
                    cont = f;
                    continue 'externo;
                }
                Entidade::Nenhuma => {}
            }
        }
        break;
    }
    match entidade {
        Entidade::Token(k) if matches!(tokens[k].kind, Kind::Str(_) | Kind::StrBegin(..) | Kind::StrMid(..) | Kind::StrEnd(_)) => return Some(ContextoDeSnippet::Outro),
        Entidade::No(f) if arv.nos[f].especie == "NamedExpression" => {
            let nome = arv.nos[f].filhos.first().map(|&l| (arv.nos[l].inicio, arv.nos[l].fim));
            if nome.is_some_and(|(a, b)| a <= offset && offset <= b) {
                return Some(ContextoDeSnippet::Outro);
            }
        }
        _ => {}
    }
    let mut atual = Some(cont);
    while let Some(k) = atual {
        if k == 0 {
            break;
        }
        let e = arv.nos[k].especie;
        match e {
            "ReturnStatement" if offset > arv.nos[k].inicio + "return".len() => return Some(ContextoDeSnippet::Expressao),
            "Comment" | "SimpleStringLiteral" | "StringInterpolation" | "AdjacentStrings" => return Some(ContextoDeSnippet::Outro),
            "VariableDeclaration" => return Some(ContextoDeSnippet::Expressao),
            "VariableDeclarationList" => return Some(ContextoDeSnippet::Outro),
            "PropertyAccess" | "FieldFormalParameter" | "PrefixedIdentifier" | "ConstructorReference" | "InstanceCreationExpression" => return Some(ContextoDeSnippet::Outro),
            "Block" => return Some(ContextoDeSnippet::Bloco),
            _ if crate::refatoracoes::e_comando_especie(e) => return Some(ContextoDeSnippet::Outro),
            "SwitchExpression" => return Some(ContextoDeSnippet::Outro),
            _ if crate::refatoracoes::e_expressao_especie(e) => return Some(ContextoDeSnippet::Expressao),
            "Annotation" => return Some(ContextoDeSnippet::Outro),
            "BlockFunctionBody" => return Some(ContextoDeSnippet::Bloco),
            "ClassDeclaration" | "ExtensionDeclaration" | "MixinDeclaration" | "EnumDeclaration" => return Some(ContextoDeSnippet::Classe),
            _ => {}
        }
        atual = arv.nos[k].pai;
    }
    Some(ContextoDeSnippet::Topo)
}
