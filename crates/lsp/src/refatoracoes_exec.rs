//! A execução das refatorações (`refactor.perform`, `refactor.validate` e
//! `dart.refactor.move_top_level_to_file`; docs/LSP-ESPECIFICACAO.md §13.11.2
//! a §13.11.9): o `RefactoringStatus`, os utilitários de texto do
//! `CorrectionUtils`, a validação e a sugestão de nomes, e as refatorações
//! Extract Local Variable, Inline Local Variable, Convert Getter to Method e
//! Convert Method to Getter. Extract Method, Inline Method e Move ficam em
//! `refatoracoes_metodo.rs`, `refatoracoes_embutir.rs` e
//! `refatoracoes_mover.rs`.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::Edicao;
use crate::projeto::{Alvo, Concreto, Projeto};
use crate::refatoracoes::{Contexto, Elem, PedidoDeRefatoracao, ResultadoDeRefatoracao, SelecaoLocal};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionElementId, FunctionKind, FunctionRef, UnitId};
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use dartforge_frontend::token::Kind;
use dartforge_types::{Resolved, Type, TypeId};
use std::collections::{BTreeSet, HashMap, HashSet};

type R = ResultadoDeRefatoracao;

// -- RefactoringStatus ---------------------------------------------------------

/// A severidade de um problema (`RefactoringProblemSeverity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Severidade {
    Aviso,
    Erro,
    Fatal,
}

/// `RefactoringStatus`: os problemas na ordem de inserção.
#[derive(Debug, Clone, Default)]
pub(crate) struct Estado {
    pub(crate) problemas: Vec<(Severidade, String)>,
}

impl Estado {
    pub(crate) fn fatal(m: impl Into<String>) -> Estado {
        Estado { problemas: vec![(Severidade::Fatal, m.into())] }
    }

    pub(crate) fn erro(m: impl Into<String>) -> Estado {
        Estado { problemas: vec![(Severidade::Erro, m.into())] }
    }

    pub(crate) fn aviso(m: impl Into<String>) -> Estado {
        Estado { problemas: vec![(Severidade::Aviso, m.into())] }
    }

    pub(crate) fn adicionar(&mut self, s: Severidade, m: impl Into<String>) {
        self.problemas.push((s, m.into()));
    }

    /// `addStatus`.
    pub(crate) fn somar(&mut self, outro: Estado) {
        self.problemas.extend(outro.problemas);
    }

    pub(crate) fn maxima(&self) -> Option<Severidade> {
        self.problemas.iter().map(|(s, _)| *s).max()
    }

    pub(crate) fn tem_fatal(&self) -> bool {
        self.maxima() == Some(Severidade::Fatal)
    }

    /// `hasError`: `ERROR` ou `FATAL`.
    pub(crate) fn tem_erro(&self) -> bool {
        self.maxima().is_some_and(|s| s >= Severidade::Erro)
    }

    pub(crate) fn ok(&self) -> bool {
        self.problemas.is_empty()
    }

    /// `message`: a do primeiro problema de severidade máxima.
    pub(crate) fn mensagem(&self) -> Option<String> {
        let max = self.maxima()?;
        self.problemas.iter().find(|(s, _)| *s == max).map(|(_, m)| m.clone())
    }
}

/// O resultado de uma refatoração legada: `refactor.validate` olha só as
/// condições iniciais; `refactor.perform`, as iniciais (fatal interrompe) e
/// as finais, depois a mudança.
pub(crate) fn concluir(
    pedido: &PedidoDeRefatoracao,
    iniciais: Estado,
    finais: impl FnOnce() -> Estado,
    mudanca: impl FnOnce() -> Mudanca,
) -> ResultadoDeRefatoracao {
    if pedido.so_validar {
        return match (iniciais.tem_erro(), iniciais.mensagem()) {
            (true, Some(m)) => R::Erro(m),
            _ => R::Valido,
        };
    }
    let mut todas = iniciais;
    if !todas.tem_fatal() {
        todas.somar(finais());
    }
    if todas.tem_erro() {
        return R::Erro(todas.mensagem().unwrap_or_default());
    }
    mudanca().resultado()
}

// -- SourceChange ----------------------------------------------------------------

/// `SourceChange`: as edições por arquivo, na ordem do `SourceFileEdit.add`
/// (decrescente por offset; uma nova com o mesmo offset entra antes das
/// existentes), e um arquivo novo.
#[derive(Debug, Clone, Default)]
pub(crate) struct Mudanca {
    pub(crate) arquivos: Vec<(String, Vec<Edicao>)>,
    pub(crate) criar: Option<(String, String)>,
    /// A primeira `ConflictingEditException` (edições que se sobrepõem).
    pub(crate) conflito: Option<String>,
}

impl Mudanca {
    /// `addEdit` / `addEditForSource`.
    pub(crate) fn adicionar(&mut self, uri: &str, span: Span, texto: impl Into<String>) {
        let e = Edicao { uri: uri.to_string(), span, texto: texto.into() };
        let lista = match self.arquivos.iter().position(|(u, _)| u == uri) {
            Some(i) => &mut self.arquivos[i].1,
            None => {
                self.arquivos.push((uri.to_string(), Vec::new()));
                &mut self.arquivos.last_mut().unwrap().1
            }
        };
        let mut i = 0;
        while i < lista.len() && lista[i].span.start > e.span.start {
            i += 1;
        }
        // As verificações de sobreposição do `addEditForSource`.
        let conflita = (i > 0 && e.span.end > lista[i - 1].span.start)
            || (i < lista.len() && {
                let proxima = &lista[i];
                (e.span.start == proxima.span.start && e.span.end > e.span.start && proxima.span.end > proxima.span.start)
                    || proxima.span.end > e.span.start
            });
        if conflita {
            if self.conflito.is_none() {
                self.conflito = Some(format!("ConflictingEditException: {}:{} conflicts with an existing edit", e.span.start, e.span.end - e.span.start));
            }
            return;
        }
        lista.insert(i, e);
    }

    pub(crate) fn vazia(&self) -> bool {
        self.criar.is_none() && self.arquivos.iter().all(|(_, l)| l.is_empty())
    }

    /// A mudança na forma do LSP: `sortSourceEditsForLsp` inverte a lista
    /// de cada arquivo (crescente por offset; inserções no mesmo offset na
    /// ordem em que ficam no texto).
    pub(crate) fn resultado(self) -> ResultadoDeRefatoracao {
        if let Some(c) = self.conflito {
            return R::ErroInterno(c);
        }
        R::Mudanca { edicoes: self.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(), criar: self.criar }
    }
}

// -- CorrectionUtils ---------------------------------------------------------------

/// Os utilitários de texto do `CorrectionUtils`
/// (`analysis_server_plugin/lib/edit/correction_utils.dart`).
#[derive(Clone, Copy)]
pub(crate) struct Texto<'a> {
    pub(crate) t: &'a str,
}

/// Dois espaços (`oneIndent`).
pub(crate) const UM_RECUO: &str = "  ";

impl<'a> Texto<'a> {
    pub(crate) fn novo(t: &'a str) -> Texto<'a> {
        Texto { t }
    }

    fn b(&self, i: usize) -> u8 {
        self.t.as_bytes()[i]
    }

    /// `endOfLine`.
    pub(crate) fn eol(&self) -> &'static str {
        if self.t.contains("\r\n") {
            "\r\n"
        } else if self.t.contains('\n') {
            "\n"
        } else if cfg!(windows) {
            "\r\n"
        } else {
            "\n"
        }
    }

    /// `getLineContentEnd`.
    pub(crate) fn fim_do_conteudo(&self, mut i: usize) -> usize {
        let n = self.t.len();
        while i < n && matches!(self.b(i), b' ' | b'\t') {
            i += 1;
        }
        if i < n && self.b(i) == b'\r' {
            i += 1;
        }
        if i < n && self.b(i) == b'\n' {
            i += 1;
        }
        i
    }

    /// `getLineContentStart`.
    pub(crate) fn inicio_do_conteudo(&self, mut i: usize) -> usize {
        while i > 0 && matches!(self.b(i - 1), b' ' | b'\t') {
            i -= 1;
        }
        i
    }

    /// `getLineNext`.
    pub(crate) fn proxima_linha(&self, mut i: usize) -> usize {
        let n = self.t.len();
        while i < n && !matches!(self.b(i), b'\r' | b'\n') {
            i += 1;
        }
        if i < n && self.b(i) == b'\r' {
            i += 1;
        }
        if i < n && self.b(i) == b'\n' {
            i += 1;
        }
        i
    }

    /// `getLineThis`.
    pub(crate) fn inicio_da_linha(&self, mut i: usize) -> usize {
        while i > 0 && !matches!(self.b(i - 1), b'\r' | b'\n') {
            i -= 1;
        }
        i
    }

    /// `getLinePrefix`.
    pub(crate) fn prefixo_da_linha(&self, i: usize) -> &'a str {
        let ini = self.inicio_da_linha(i);
        let mut j = ini;
        let n = self.t.len();
        while j < n && matches!(self.b(j), b' ' | b'\t') {
            j += 1;
        }
        &self.t[ini..j]
    }

    /// `getPrefix`.
    pub(crate) fn prefixo(&self, i: usize) -> &'a str {
        &self.t[self.inicio_do_conteudo(i)..i]
    }

    /// `getLinesRange`.
    pub(crate) fn faixa_de_linhas(&self, ini: usize, fim: usize) -> Span {
        let inicio = self.inicio_do_conteudo(ini);
        let mut depois = fim;
        if self.inicio_da_linha(inicio) == inicio {
            depois = self.fim_do_conteudo(fim);
        }
        Span { start: inicio, end: depois }
    }

    /// `indentSourceLeftRight(indentLeft: true)`.
    pub(crate) fn recuar_a_esquerda(&self, fonte: &str) -> String {
        let eol = self.eol();
        let linhas: Vec<&str> = fonte.split(eol).collect();
        let mut s = String::new();
        for (i, linha) in linhas.iter().enumerate() {
            if i == linhas.len() - 1 && linha.is_empty() {
                break;
            }
            s.push_str(linha.strip_prefix(UM_RECUO).unwrap_or(linha));
            s.push_str(eol);
        }
        s
    }

    /// `replaceSourceIndent`.
    pub(crate) fn trocar_recuo(&self, fonte: &str, velho: &str, novo: &str, incluir_inicio: bool, garantir_eol: bool) -> String {
        // As faixas dos tokens de string (uma linha que começa dentro de
        // uma string de várias linhas não muda).
        let strings: Vec<Span> = dartforge_frontend::lexer::lex(fonte)
            .map(|v| {
                v.iter()
                    .filter(|t| matches!(t.kind, Kind::Str(_) | Kind::StrBegin(..) | Kind::StrMid(..) | Kind::StrEnd(_)))
                    .map(|t| t.span)
                    .collect()
            })
            .unwrap_or_default();
        let eol = self.eol();
        let linhas: Vec<&str> = fonte.split(eol).collect();
        let mut s = String::new();
        let mut offset_da_linha = 0usize;
        for (i, &linha) in linhas.iter().enumerate() {
            if i == linhas.len() - 1 && linha.is_empty() {
                break;
            }
            let trocar = i != 0 || incluir_inicio;
            let eol_depois = i != linhas.len() - 1 || garantir_eol;
            let mut em_string = false;
            for r in &strings {
                if offset_da_linha > r.start && offset_da_linha < r.end {
                    em_string = true;
                }
                if offset_da_linha > r.end {
                    break;
                }
            }
            offset_da_linha += linha.len() + eol.len();
            if !em_string && trocar {
                s.push_str(novo);
                s.push_str(linha.strip_prefix(velho).unwrap_or(linha));
            } else {
                s.push_str(linha);
            }
            if eol_depois {
                s.push_str(eol);
            }
        }
        s
    }
}

impl Contexto<'_> {
    /// `getNodePrefix`.
    pub(crate) fn prefixo_do_no(&self, n: usize) -> String {
        let tx = Texto::novo(self.fonte);
        let o = self.arvore.nos[n].inicio;
        if self.especie(n) == "FunctionExpression" { tx.prefixo_da_linha(o).to_string() } else { tx.prefixo(o).to_string() }
    }

    /// O texto de um nó.
    pub(crate) fn texto_do_no(&self, n: usize) -> &str {
        let no = &self.arvore.nos[n];
        &self.fonte[no.inicio..no.fim]
    }

    /// A expressão da árvore do parser do nó: a da marca de um
    /// `SimpleIdentifier`, senão a mais externa com o mesmo intervalo (os pais
    /// entram na arena depois dos filhos).
    pub(crate) fn expr_do_no(&self, n: usize) -> Option<ExprId> {
        let no = &self.arvore.nos[n];
        if let crate::arvore_analyzer::Marca::Expr(x) = no.marca
            && self.especie(n) == "SimpleIdentifier"
        {
            return Some(x);
        }
        self.ast
            .exprs
            .iter()
            .enumerate()
            .filter(|(_, e)| e.span.start == no.inicio && e.span.end == no.fim)
            .map(|(i, _)| ExprId(i as u32))
            .max_by_key(|x| x.0)
    }

    /// O tipo estático (`staticType`) do nó de expressão.
    pub(crate) fn tipo_do_no(&self, n: usize) -> Option<TypeId> {
        let x = self.expr_do_no(n)?;
        self.corpos.get_type(x)
    }
}

/// A regra de lint `nome` está ligada para o arquivo da unidade
/// (`analysis_options.yaml` da subpasta ou da raiz).
pub(crate) fn regra_ligada(p: &Projeto, unidade: UnitId, nome: &str) -> bool {
    let Some(caminho) = p.programa().unit(unidade).path.as_deref() else { return false };
    regra_ligada_em(caminho, nome)
}

/// Como [`regra_ligada`], pelo caminho do arquivo.
pub(crate) fn regra_ligada_em(caminho: &std::path::Path, nome: &str) -> bool {
    let raiz = crate::projeto::raiz_do_projeto(caminho);
    let arquivo = dartforge_paridade::filtros::Opcoes::de_subpasta(caminho, &raiz).unwrap_or_else(|| raiz.join("analysis_options.yaml"));
    dartforge_paridade::filtros::Opcoes::ler_arquivo(&arquivo).regras.get(nome).copied().unwrap_or(false)
}

/// `options['name'] as String` com `options != null`: o nome, ou o erro do
/// `as` que vira `UnhandledError`.
pub(crate) fn nome_das_opcoes(pedido: &PedidoDeRefatoracao) -> Result<Option<String>, ResultadoDeRefatoracao> {
    match &pedido.opcoes {
        None => Ok(None),
        Some(m) => match m.get("name") {
            Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
            Some(v) => Err(R::ErroInterno(format!("type '{}' is not a subtype of type 'String' in type cast", tipo_json(v)))),
            None => Err(R::ErroInterno("type 'Null' is not a subtype of type 'String' in type cast".to_string())),
        },
    }
}

fn tipo_json(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "Null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(n) if n.is_i64() || n.is_u64() => "int",
        serde_json::Value::Number(_) => "double",
        serde_json::Value::String(_) => "String",
        serde_json::Value::Array(_) => "List<dynamic>",
        serde_json::Value::Object(_) => "_Map<String, dynamic>",
    }
}

// -- Nomes -------------------------------------------------------------------------

/// As palavras reservadas (`KeywordStyle.reserved`); as outras palavras-chave
/// são embutidas ou pseudo.
const RESERVADAS: &[&str] = &[
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else", "enum", "extends", "false", "final", "finally", "for", "if",
    "in", "is", "new", "null", "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while", "with",
];

/// As embutidas e as pseudo (`isBuiltInOrPseudo`).
const EMBUTIDAS: &[&str] = &[
    "abstract", "as", "async", "augment", "await", "base", "covariant", "deferred", "dynamic", "export", "extension", "external", "factory", "Function",
    "get", "hide", "implements", "import", "inout", "interface", "late", "library", "mixin", "native", "of", "on", "operator", "out", "part", "patch",
    "required", "sealed", "set", "show", "source", "static", "sync", "typedef", "when", "yield",
];

/// `_validateLowerCamelCase(nome, desc, allowBuiltIn: true)`
/// (`legacy/naming_conventions.dart:144-220`).
pub(crate) fn validar_nome(nome: &str, desc: &str) -> Estado {
    let desc = format!("{desc} name");
    if nome != nome.trim() {
        return Estado::fatal(format!("{desc} must not start or end with a blank."));
    }
    if nome.is_empty() {
        return Estado::fatal(format!("{desc} must not be empty."));
    }
    if EMBUTIDAS.contains(&nome) {
        return Estado::aviso("Avoid using built-in identifiers as names.");
    }
    if RESERVADAS.contains(&nome) {
        return Estado::fatal(format!("{desc} must not be a keyword."));
    }
    for c in nome.chars() {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '$') {
            return Estado::fatal(format!("{desc} must not contain '{c}'."));
        }
    }
    let primeiro = nome.as_bytes()[0];
    if !(primeiro.is_ascii_alphabetic() || primeiro == b'_' || primeiro == b'$') {
        return Estado::fatal(format!("{desc} must begin with a lowercase letter or underscore."));
    }
    if primeiro == b'_' || primeiro == b'$' {
        return Estado::default();
    }
    if !primeiro.is_ascii_lowercase() {
        return Estado::aviso(format!("{desc} should start with a lowercase letter."));
    }
    Estado::default()
}

/// `getCamelWords`.
pub(crate) fn palavras_camel(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    let b = s.as_bytes();
    let mut partes = Vec::new();
    let (mut era_min, mut era_mai) = (false, false);
    let mut inicio = 0;
    for i in 0..b.len() {
        let c = b[i];
        let min = c.is_ascii_lowercase();
        let mai = c.is_ascii_uppercase();
        if era_min && mai {
            partes.push(s[inicio..i].to_string());
            inicio = i;
        }
        if era_mai && mai && i + 1 < b.len() && b[i + 1].is_ascii_lowercase() {
            partes.push(s[inicio..i].to_string());
            inicio = i;
        }
        era_min = min;
        era_mai = mai;
    }
    partes.push(s[inicio..].to_string());
    partes
}

/// `getCamelWordCombinations`.
pub(crate) fn combinacoes_camel(nome: &str) -> Vec<String> {
    let partes = palavras_camel(nome);
    (0..partes.len()).map(|i| format!("{}{}", partes[i].to_lowercase(), partes[i + 1..].concat())).collect()
}

/// `capitalize`.
fn capitalizar(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `_addAll`: o primeiro de `item`, `item2`, `item3`… não excluído.
pub(crate) fn adicionar_todos(excluidos: &HashSet<String>, saida: &mut Vec<String>, itens: Vec<String>, prefixo: Option<&str>) {
    for item in itens {
        let mut sufixo = 1;
        loop {
            let nome = if sufixo > 1 { format!("{item}{sufixo}") } else { item.clone() };
            if !excluidos.contains(&nome) {
                let final_ = match prefixo {
                    Some(p) => format!("{p}{}", capitalizar(&nome)),
                    None => nome,
                };
                if !saida.contains(&final_) {
                    saida.push(final_);
                }
                break;
            }
            sufixo += 1;
        }
    }
}

/// `_addSingleCharacterName`.
pub(crate) fn adicionar_letra(excluidos: &HashSet<String>, saida: &mut Vec<String>, mut c: u8) {
    while c < b'z' {
        let nome = (c as char).to_string();
        if !excluidos.contains(&nome) {
            if !saida.contains(&nome) {
                saida.push(nome);
            }
            break;
        }
        c += 1;
    }
}

/// `getVariableNameSuggestionsForText`.
pub(crate) fn nomes_para_texto(texto: &str, excluidos: &HashSet<String>) -> Vec<String> {
    let filtrado: String = texto.chars().filter(|c| c.is_alphabetic() || matches!(c, ' ' | '\t' | '\r' | '\n')).collect();
    let mut camel = String::new();
    for (i, palavra) in filtrado.split(' ').enumerate() {
        if i > 0 {
            camel.push_str(&capitalizar(palavra));
        } else {
            camel.push_str(palavra);
        }
    }
    let mut saida = Vec::new();
    adicionar_todos(excluidos, &mut saida, combinacoes_camel(&camel), None);
    saida
}

impl Contexto<'_> {
    /// `getVariableNameSuggestionsForExpression(tipo, n, excluidos,
    /// isMethod)` (`name_suggestion.dart:29-76`).
    pub(crate) fn nomes_para_expressao(&self, tipo: Option<TypeId>, n: usize, excluidos: &HashSet<String>, metodo: bool) -> Vec<String> {
        let mut prefixo: Option<&str> = None;
        if metodo
            && let Some(m) = self.com_pais(n).find(|&k| self.especie(k) == "MethodDeclaration")
            && let crate::arvore_analyzer::Marca::Funcao(fid) = self.arvore.nos[m].marca
            && let Some(nome) = self.ast.function(fid).name
            && self.fonte[nome.span.start..nome.span.end].starts_with("build")
        {
            prefixo = Some("build");
        }
        let mut saida = Vec::new();
        if let Some(mut nome) = self.nome_base_da_expressao(n) {
            if let Some(r) = nome.strip_prefix('_') {
                nome = r.to_string();
            }
            adicionar_todos(excluidos, &mut saida, combinacoes_camel(&nome), prefixo);
        }
        if let Some(nome) = self.nome_pela_posicao(n) {
            adicionar_todos(excluidos, &mut saida, combinacoes_camel(&nome), None);
        }
        if let Some(t) = tipo {
            let tabela = &self.p.consulta.tabela;
            let core = &self.p.consulta.core;
            match tabela.get(t) {
                Type::Dynamic => {}
                _ if tabela.canonico(t) == tabela.canonico(core.int) => adicionar_letra(excluidos, &mut saida, b'i'),
                _ if tabela.canonico(t) == tabela.canonico(core.double) => adicionar_letra(excluidos, &mut saida, b'd'),
                _ if tabela.canonico(t) == tabela.canonico(core.string) => adicionar_letra(excluidos, &mut saida, b's'),
                Type::Interface { class, .. } => {
                    let nome = self.p.nome(self.p.programa().class(*class).name).to_string();
                    adicionar_todos(excluidos, &mut saida, combinacoes_camel(&nome), None);
                }
                _ => {}
            }
        }
        saida
    }

    /// `_getBaseNameFromExpression`.
    fn nome_base_da_expressao(&self, mut n: usize) -> Option<String> {
        while matches!(self.especie(n), "AsExpression" | "ParenthesizedExpression") {
            n = *self.filhos(n).first()?;
        }
        let texto = |k: usize| self.texto_do_no(k).to_string();
        let nome: String = match self.especie(n) {
            "SimpleIdentifier" => return Some(texto(n)),
            "PrefixedIdentifier" => return self.filhos(n).get(1).map(|&k| texto(k)),
            "PropertyAccess" => return self.filhos(n).last().map(|&k| texto(k)),
            "MethodInvocation" => texto(self.nome_do_metodo(n)?),
            "InstanceCreationExpression" => {
                let cn = *self.filhos(n).iter().find(|&&k| self.especie(k) == "ConstructorName")?;
                let tipo = *self.filhos(cn).first()?;
                // O nome do `NamedType` (depois do prefixo de import).
                let no = &self.arvore.nos[tipo];
                let inicio = self.filhos(tipo).iter().find(|&&k| self.especie(k) == "ImportPrefixReference").map_or(no.inicio, |&k| self.arvore.nos[k].fim);
                let resto = &self.fonte[inicio..no.fim];
                let fim = resto.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(resto.len());
                return Some(resto[..fim].trim().to_string());
            }
            "IndexExpression" => {
                let alvo = self.alvo_real_do_indice(n)?;
                let mut nome = self.nome_base_da_expressao(alvo)?;
                if nome.ends_with('s') {
                    nome.pop();
                }
                nome
            }
            _ => return None,
        };
        for p in ["get", "is", "to"] {
            if let Some(r) = nome.strip_prefix(p) {
                if r.is_empty() {
                    return None;
                }
                if r.as_bytes()[0].is_ascii_uppercase() {
                    return Some(r.to_string());
                }
            }
        }
        Some(nome)
    }

    /// `IndexExpression.realTarget`: o alvo, ou o da cascata.
    fn alvo_real_do_indice(&self, n: usize) -> Option<usize> {
        let filhos = self.filhos(n);
        if filhos.len() >= 2 {
            return filhos.first().copied();
        }
        // `..[i]`: o alvo da cascata.
        let mut k = n;
        while let Some(p) = self.pai(k) {
            if self.especie(p) == "CascadeExpression" {
                return self.filhos(p).first().copied();
            }
            k = p;
        }
        None
    }

    /// `_getBaseNameFromLocationInParent`: o rótulo do argumento nomeado, ou
    /// o nome do parâmetro posicional correspondente.
    fn nome_pela_posicao(&self, n: usize) -> Option<String> {
        let p = self.pai(n)?;
        if self.especie(p) == "NamedExpression" && self.filhos(p).get(1) == Some(&n) {
            let rotulo = *self.filhos(p).first()?;
            let id = *self.filhos(rotulo).first()?;
            return Some(self.texto_do_no(id).to_string());
        }
        if self.especie(p) != "ArgumentList" {
            return None;
        }
        // O índice entre os posicionais.
        let posicionais: Vec<usize> = self.filhos(p).iter().copied().filter(|&k| self.especie(k) != "NamedExpression").collect();
        let indice = posicionais.iter().position(|&k| k == n)?;
        let dono = self.pai(p)?;
        let f = match self.especie(dono) {
            "MethodInvocation" => match self.elemento_do_identificador(self.nome_do_metodo(dono)?, true) {
                Elem::Funcao(f) => f,
                _ => return None,
            },
            "InstanceCreationExpression" => {
                let x = self.expr_do_no(dono)?;
                match self.corpos.get_resolved(x) {
                    Some(Resolved::Constructor(f)) => *f,
                    _ => {
                        let cn = *self.filhos(dono).iter().find(|&&k| self.especie(k) == "ConstructorName")?;
                        let _ = cn;
                        let alvo = self.construtores_por_alvo.iter().find(|(a, _)| {
                            let s = self.ast.expr(**a).span;
                            s.start >= self.arvore.nos[dono].inicio && s.end <= self.arvore.nos[dono].fim
                        })?;
                        *alvo.1
                    }
                }
            }
            _ => return None,
        };
        let dados = self.p.consulta.outline.functions.get(f.0 as usize)?;
        let parametro = dados.parameters.iter().filter(|q| q.kind != ast::ParameterKind::Named).nth(indice)?;
        Some(self.p.nome(parametro.name?).to_string())
    }
}

// -- Despacho ----------------------------------------------------------------------

/// `refactor.perform`/`refactor.validate` de uma refatoração legada.
pub(crate) fn executar(p: &Projeto, uri: &str, pedido: &PedidoDeRefatoracao) -> ResultadoDeRefatoracao {
    let Some(unidade) = p.unidade_do_uri(uri) else { return R::NaoAnalisado };
    let cx = Contexto::novo(p, unidade);
    let comando = if pedido.so_validar { "Validate Refactor" } else { "Perform Refactor" };
    match pedido.kind.as_str() {
        "EXTRACT_METHOD" => crate::refatoracoes_metodo::executar(&cx, uri, pedido),
        "EXTRACT_LOCAL_VARIABLE" => extrair_local(&cx, uri, pedido),
        "EXTRACT_WIDGET" => extrair_widget(&cx, pedido),
        "INLINE_LOCAL_VARIABLE" => embutir_local(&cx, uri, pedido),
        "INLINE_METHOD" => crate::refatoracoes_embutir::executar(&cx, uri, pedido),
        "CONVERT_GETTER_TO_METHOD" => getter_para_metodo(&cx, pedido, comando),
        "CONVERT_METHOD_TO_GETTER" => metodo_para_getter(&cx, pedido, comando),
        k => R::ArgumentosInvalidos(format!("Unknown RefactoringKind RefactoringKind.{k} was supplied to {comando}")),
    }
}

/// `dart.refactor.move_top_level_to_file`.
pub(crate) fn mover(p: &Projeto, uri: &str, offset: usize, comprimento: usize, destino: &str) -> ResultadoDeRefatoracao {
    let Some(unidade) = p.unidade_do_uri(uri) else { return R::NaoAnalisado };
    let cx = Contexto::novo(p, unidade);
    crate::refatoracoes_mover::executar(&cx, uri, offset, comprimento, destino)
}

/// `EXTRACT_WIDGET`: sem o Flutter, nenhuma expressão cria `Widget` nem
/// método o devolve (§13.11.10).
fn extrair_widget(cx: &Contexto<'_>, pedido: &PedidoDeRefatoracao) -> ResultadoDeRefatoracao {
    if let Err(e) = nome_das_opcoes(pedido) {
        return e;
    }
    let fim = pedido.offset + pedido.comprimento;
    let no = cx.arvore.localizar(pedido.offset, fim);
    let mensagem = match no.map(|n| cx.especie(n)) {
        Some("Block") => "The last selected statement must return a widget.",
        _ => "Can only extract a widget expression or a method returning widget.",
    };
    let iniciais = Estado::fatal(mensagem);
    concluir(pedido, iniciais, Estado::default, Mudanca::default)
}

// -- Extract Local Variable ----------------------------------------------------------

/// `ExtractLocalRefactoringImpl` (§13.11.4).
fn extrair_local(cx: &Contexto<'_>, uri: &str, pedido: &PedidoDeRefatoracao) -> ResultadoDeRefatoracao {
    let preferido = match nome_das_opcoes(pedido) {
        Ok(n) => n,
        Err(e) => return e,
    };
    let selecao = cx.selecao_local(pedido.offset, pedido.comprimento);
    let (iniciais, sel) = match selecao {
        Err(m) => (Estado::fatal(m), None),
        Ok(s) => (Estado::default(), Some(s)),
    };
    // `findPossibleLocalVariableConflicts(selectionOffset)` e os nomes.
    let excluidos = cx.conflitos_de_local(pedido.offset);
    let nomes = match &sel {
        Some(s) => match (&s.parte_de_string, s.expressao) {
            (Some(parte), _) => nomes_para_texto(parte, &excluidos),
            (None, Some(e)) => cx.nomes_para_expressao(cx.tipo_do_no(e), e, &excluidos, false),
            _ => Vec::new(),
        },
        None => Vec::new(),
    };
    let nome = preferido.or_else(|| nomes.first().cloned()).unwrap_or_else(|| "newVariable".to_string());
    let finais = || {
        let mut e = validar_nome(&nome, "Variable");
        if excluidos.contains(&nome) {
            e.adicionar(Severidade::Erro, format!("The name '{nome}' is already used in the scope."));
        }
        e
    };
    let sel2 = sel.clone();
    concluir(pedido, iniciais, finais, || match sel2 {
        Some(s) => cx.mudanca_de_extrair_local(uri, &s, &nome),
        None => Mudanca::default(),
    })
}

impl Contexto<'_> {
    /// `findPossibleLocalVariableConflicts(offset)`: no `Block` mais interno
    /// do nó do offset, os nomes referidos sem prefixo e os declarados por
    /// `VariableDeclaration`.
    pub(crate) fn conflitos_de_local(&self, offset: usize) -> HashSet<String> {
        let mut nomes = HashSet::new();
        let Some(n) = self.arvore.localizar(offset, offset) else { return nomes };
        let Some(bloco) = self.com_pais(n).find(|&k| self.especie(k) == "Block") else { return nomes };
        let mut pilha = vec![bloco];
        while let Some(k) = pilha.pop() {
            match self.especie(k) {
                "NamedType" => {
                    let tem_prefixo = self.filhos(k).iter().any(|&f| self.especie(f) == "ImportPrefixReference");
                    if !tem_prefixo {
                        let no = &self.arvore.nos[k];
                        let resto = &self.fonte[no.inicio..no.fim];
                        let fim = resto.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(resto.len());
                        nomes.insert(resto[..fim].to_string());
                    }
                }
                "SimpleIdentifier" => {
                    if !self.identificador_prefixado(k) && !self.pai(k).is_some_and(|p| self.especie(p) == "Label") {
                        nomes.insert(self.texto_do_no(k).to_string());
                    }
                }
                "VariableDeclaration" => {
                    let no = &self.arvore.nos[k];
                    let resto = &self.fonte[no.inicio..no.fim];
                    let fim = resto.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(resto.len());
                    nomes.insert(resto[..fim].to_string());
                }
                _ => {}
            }
            pilha.extend(self.filhos(k).iter().copied());
        }
        nomes
    }

    /// `_ReferencedUnprefixedNamesCollector._isPrefixed`.
    fn identificador_prefixado(&self, n: usize) -> bool {
        let Some(p) = self.pai(n) else { return false };
        match self.especie(p) {
            "ConstructorName" => self.filhos(p).get(1) == Some(&n),
            "MethodInvocation" => {
                self.nome_do_metodo(p) == Some(n)
                    && (self.filhos(p).first() != Some(&n) || self.pai(p).is_some_and(|c| self.especie(c) == "CascadeExpression" && self.filhos(c).first() != Some(&p)))
            }
            "PrefixedIdentifier" => self.filhos(p).get(1) == Some(&n),
            "PropertyAccess" => self.filhos(p).len() > 1 && self.filhos(p).first() == Some(&n),
            _ => false,
        }
    }

    /// `_isPartOfConstantExpression`.
    fn parte_de_constante(&self, n: Option<usize>) -> bool {
        let Some(n) = n else { return false };
        match self.especie(n) {
            "ListLiteral" | "SetOrMapLiteral" | "InstanceCreationExpression" => self.constante(n),
            "ArgumentList" | "ConditionalExpression" | "BinaryExpression" | "ParenthesizedExpression" | "PrefixExpression" | "MapLiteralEntry"
            | "IntegerLiteral" | "DoubleLiteral" | "BooleanLiteral" | "NullLiteral" | "SimpleStringLiteral" | "StringInterpolation" | "AdjacentStrings"
            | "SymbolLiteral" | "RecordLiteral" => self.parte_de_constante(self.pai(n)),
            _ => false,
        }
    }

    /// `TypedLiteral.isConst`/`InstanceCreationExpression.isConst`: `const`
    /// escrito, ou (sem palavra-chave) o contexto constante.
    fn constante(&self, n: usize) -> bool {
        let no = &self.arvore.nos[n];
        if self.fonte[no.inicio..no.fim].starts_with("const") && !self.fonte[no.inicio + 5..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            return true;
        }
        if self.fonte[no.inicio..no.fim].starts_with("new") && !self.fonte[no.inicio + 3..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            return false;
        }
        let Some(x) = self.expr_do_no(n) else { return false };
        let u = self.p.programa().unit(self.unidade);
        let antes_de_3 = self.p.programa().library(u.library).features.versao().major < 3;
        let pais = dartforge_frontend::pais::Pais::novo(self.ast, &u.unit, self.fonte, antes_de_3);
        pais.em_contexto_constante(self.ast, x)
    }

    /// `_declarationKeywordAndType`.
    fn palavra_e_tipo(&self, s: &SelecaoLocal) -> String {
        let usar_const = s.parte_de_string.is_none() && self.parte_de_constante(s.expressao);
        let usar_final = regra_ligada(self.p, self.unidade, "prefer_final_locals");
        let tipo: Option<String> = if regra_ligada(self.p, self.unidade, "always_specify_types") {
            match (s.expressao, &s.parte_de_string) {
                (Some(e), _) => self.tipo_do_no(e).map(|t| self.p.consulta.formatar(t)),
                (None, Some(_)) => Some("String".to_string()),
                _ => None,
            }
        } else {
            None
        };
        match (usar_const, usar_final, tipo) {
            (true, _, Some(t)) => format!("const {t}"),
            (true, _, None) => "const".to_string(),
            (false, true, Some(t)) => format!("final {t}"),
            (false, true, None) => "final".to_string(),
            (false, false, Some(t)) => t,
            (false, false, None) => "var".to_string(),
        }
    }

    /// `ExtractLocalRefactoringImpl.createChange` com `extractAll = false`.
    fn mudanca_de_extrair_local(&self, uri: &str, s: &SelecaoLocal, nome: &str) -> Mudanca {
        let mut m = Mudanca::default();
        let tx = Texto::novo(self.fonte);
        let (ini, fim) = s.faixa;
        let palavra = self.palavra_e_tipo(s);
        // Atalho: a expressão inteira de um `ExpressionStatement`.
        if let Some(e) = s.expressao
            && self.pai(e).is_some_and(|p| self.especie(p) == "ExpressionStatement")
        {
            let o = self.arvore.nos[e].inicio;
            m.adicionar(uri, Span { start: o, end: o }, format!("{palavra} {nome} = "));
            return m;
        }
        let mut declaracao = format!("{palavra} ");
        match &s.parte_de_string {
            Some(parte) => declaracao.push_str(&format!("{nome} = '{parte}';")),
            None => declaracao.push_str(&format!("{nome} = {};", &self.fonte[ini..fim])),
        }
        let eol = tx.eol();
        match self.alvo_da_declaracao(ini) {
            Some(Alvo2::Comando(alvo)) => {
                let prefixo = self.prefixo_do_no(alvo);
                let o = self.arvore.nos[alvo].inicio;
                m.adicionar(uri, Span { start: o, end: o }, format!("{declaracao}{eol}{prefixo}"));
            }
            Some(Alvo2::CorpoDeExpressao(corpo)) => {
                let prefixo = self.pai(corpo).map(|p| self.prefixo_do_no(p)).unwrap_or_default();
                let expr = *self.filhos(corpo).first().unwrap_or(&corpo);
                let (ci, cf) = (self.arvore.nos[corpo].inicio, self.arvore.nos[corpo].fim);
                let (ei, ef) = (self.arvore.nos[expr].inicio, self.arvore.nos[expr].fim);
                let codigo = format!("{{{eol}{prefixo}{UM_RECUO}{declaracao}{eol}{prefixo}{UM_RECUO}return ");
                m.adicionar(uri, Span { start: ci, end: ei }, codigo);
                m.adicionar(uri, Span { start: ef, end: cf }, format!(";{eol}{prefixo}}}"));
            }
            None => {}
        }
        let troca = if s.parte_de_string.is_some() { format!("${{{nome}}}") } else { nome.to_string() };
        m.adicionar(uri, Span { start: ini, end: fim }, troca);
        m
    }

    /// `_findDeclarationTarget([selectionRange])`.
    fn alvo_da_declaracao(&self, offset: usize) -> Option<Alvo2> {
        let comum = self.arvore.localizar(offset, offset)?;
        let caminho: Vec<usize> = self.com_pais(comum).collect();
        let _ = &caminho;
        // `Block`, `SwitchCase`, `SwitchPatternCase`: o filho no caminho (com
        // uma ocorrência, o comum é o próprio nó, que não é um deles na
        // prática; a regra vale igual).
        if matches!(self.especie(comum), "Block" | "SwitchCase" | "SwitchPatternCase") {
            return None;
        }
        // `ExpressionFunctionBody` antes de qualquer comando.
        for k in self.com_pais(comum) {
            if crate::refatoracoes::e_comando_especie(self.especie(k)) {
                break;
            }
            if self.especie(k) == "ExpressionFunctionBody" {
                return Some(Alvo2::CorpoDeExpressao(k));
            }
        }
        // O comando de nível de bloco.
        let mut alvo = self.com_pais(comum).find(|&k| crate::refatoracoes::e_comando_especie(self.especie(k)))?;
        loop {
            let pai = self.pai(alvo)?;
            if matches!(self.especie(pai), "Block" | "SwitchCase" | "SwitchPatternCase") {
                return Some(Alvo2::Comando(alvo));
            }
            alvo = pai;
        }
    }
}

/// O alvo da declaração do novo local.
enum Alvo2 {
    Comando(usize),
    CorpoDeExpressao(usize),
}

// -- Inline Local Variable -----------------------------------------------------------

/// A espécie de uma referência a um local (`search.dart:1572-1601`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EspecieDeReferencia {
    Leitura,
    Escrita,
    LeituraEscrita,
    Invocacao,
}

/// `InlineLocalRefactoringImpl` (§13.11.5).
fn embutir_local(cx: &Contexto<'_>, uri: &str, pedido: &PedidoDeRefatoracao) -> ResultadoDeRefatoracao {
    let o = pedido.offset;
    let mensagem = "Local variable declaration or reference must be selected to activate this refactoring.";
    let preparar = || -> Result<(usize, usize, usize, usize), Estado> {
        // O elemento e a declaração dele.
        let n = cx.arvore.localizar(o, o).ok_or_else(|| Estado::fatal(mensagem))?;
        let decl = match cx.especie(n) {
            "SimpleIdentifier" => {
                if cx.elemento_do_identificador(n, true) != Elem::VariavelLocal {
                    return Err(Estado::fatal(mensagem));
                }
                let crate::arvore_analyzer::Marca::Expr(x) = cx.arvore.nos[n].marca else { return Err(Estado::fatal(mensagem)) };
                cx.corpos.declaracao_local(x).ok_or_else(|| Estado::fatal(mensagem))?
            }
            "VariableDeclaration" if cx.arvore.nos[n].marca == crate::arvore_analyzer::Marca::VariavelLocal => cx.arvore.nos[n].inicio,
            _ => return Err(Estado::fatal(mensagem)),
        };
        // `getElementDeclaration`: o nó da declaração é `VariableDeclaration`.
        let vd = cx
            .arvore
            .nos
            .iter()
            .position(|k| k.especie == "VariableDeclaration" && k.inicio == decl)
            .ok_or_else(|| Estado::fatal(mensagem))?;
        // `_declarationStatement`.
        let lista = cx.pai(vd).filter(|&l| cx.especie(l) == "VariableDeclarationList").ok_or_else(|| Estado::fatal(mensagem))?;
        let comando = cx.pai(lista).filter(|&s| cx.especie(s) == "VariableDeclarationStatement").ok_or_else(|| Estado::fatal(mensagem))?;
        if !cx.pai(comando).is_some_and(|b| matches!(cx.especie(b), "Block" | "SwitchCase" | "SwitchPatternCase")) {
            return Err(Estado::fatal(mensagem));
        }
        let nome = {
            let no = &cx.arvore.nos[vd];
            let resto = &cx.fonte[no.inicio..no.fim];
            let fim = resto.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(resto.len());
            resto[..fim].to_string()
        };
        let Some(&inicializador) = cx.filhos(vd).first() else {
            return Err(Estado::fatal(format!("Local variable '{nome}' is not initialized at declaration.")));
        };
        // Toda referência é leitura.
        if cx.referencias_do_local(decl).iter().any(|(_, e)| *e != EspecieDeReferencia::Leitura) {
            return Err(Estado::fatal(format!("Local variable '[{nome}]' is assigned more than once.")));
        }
        Ok((vd, comando, inicializador, decl))
    };
    match preparar() {
        Err(e) => concluir(pedido, e, Estado::default, Mudanca::default),
        Ok((_vd, comando, inicializador, decl)) => concluir(pedido, Estado::default(), Estado::default, || cx.mudanca_de_embutir_local(uri, comando, inicializador, decl)),
    }
}

impl Contexto<'_> {
    /// As referências ao local declarado em `decl` (o offset do nome) na
    /// unidade, com a espécie, em ordem de offset.
    fn referencias_do_local(&self, decl: usize) -> Vec<(Span, EspecieDeReferencia)> {
        let mut v = Vec::new();
        let invocados: HashSet<ExprId> = self
            .ast
            .exprs
            .iter()
            .filter_map(|e| match &e.kind {
                ExprKind::Call { target, .. } => Some(*target),
                _ => None,
            })
            .collect();
        let compostas: HashSet<ExprId> = self
            .ast
            .exprs
            .iter()
            .filter_map(|e| match &e.kind {
                ExprKind::Assign { op: ast::AssignOp::Compound(_), target, .. } => Some(*target),
                ExprKind::Unary {
                    op: ast::UnaryOp::PrefixInc | ast::UnaryOp::PrefixDec | ast::UnaryOp::PostfixInc | ast::UnaryOp::PostfixDec,
                    operand,
                } => Some(*operand),
                _ => None,
            })
            .collect();
        for (i, e) in self.ast.exprs.iter().enumerate() {
            let x = ExprId(i as u32);
            let ExprKind::Identifier(n) = &e.kind else { continue };
            if self.corpos.declaracao_local(x) != Some(decl) {
                continue;
            }
            let especie = if compostas.contains(&x) {
                EspecieDeReferencia::LeituraEscrita
            } else if self.escritas.contains(&x) {
                EspecieDeReferencia::Escrita
            } else if invocados.contains(&x) {
                EspecieDeReferencia::Invocacao
            } else {
                EspecieDeReferencia::Leitura
            };
            v.push((n.span, especie));
        }
        // `AssignedVariablePattern`: escrita.
        for (pid, d) in self.corpos.declaracoes_de_padroes.iter() {
            if *d == decl {
                v.push((self.ast.pattern(*pid).span, EspecieDeReferencia::Escrita));
            }
        }
        v.sort_by_key(|(s, _)| s.start);
        v
    }

    /// `InlineLocalRefactoringImpl.createChange`.
    fn mudanca_de_embutir_local(&self, uri: &str, comando: usize, inicializador: usize, decl: usize) -> Mudanca {
        let mut m = Mudanca::default();
        let tx = Texto::novo(self.fonte);
        let (ci, cf) = (self.arvore.nos[comando].inicio, self.arvore.nos[comando].fim);
        m.adicionar(uri, tx.faixa_de_linhas(ci, cf), "");
        let codigo = self.texto_do_no(inicializador).to_string();
        let init_no = &self.arvore.nos[inicializador];
        for (s, _) in self.referencias_do_local(decl) {
            let Some(n) = self.arvore.localizar(s.start, s.start) else { continue };
            let pai = self.pai(n);
            let interpolacao = pai.filter(|&p| self.especie(p) == "InterpolationExpression");
            let (faixa, texto) = if let Some(ip) = interpolacao {
                let alvo = self.pai(ip);
                let simples_ou_interp = matches!(init_no.especie, "SimpleStringLiteral" | "StringInterpolation");
                let fundir = alvo.is_some_and(|a| self.especie(a) == "StringInterpolation")
                    && simples_ou_interp
                    && !string_crua(&codigo)
                    && aspas_simples(&codigo) == aspas_simples(self.texto_do_no(alvo.unwrap()))
                    && (!string_multilinha(&codigo) || string_multilinha(self.texto_do_no(alvo.unwrap())));
                let ipn = &self.arvore.nos[ip];
                if fundir {
                    (Span { start: ipn.inicio, end: ipn.fim }, conteudo_da_string(&codigo).to_string())
                } else if !self.fonte[ipn.inicio..].starts_with("${") && init_no.especie != "SimpleIdentifier" {
                    (s, format!("{{{codigo}}}"))
                } else {
                    (s, codigo.clone())
                }
            } else if self.precisa_de_parenteses(inicializador, n) {
                (s, format!("({codigo})"))
            } else {
                (s, codigo.clone())
            };
            m.adicionar(uri, faixa, texto);
        }
        m
    }

    /// `_shouldUseParenthesis(init, node)`.
    pub(crate) fn precisa_de_parenteses(&self, init: usize, n: usize) -> bool {
        if let Some(p) = self.pai(n)
            && self.especie(p) == "SwitchExpression"
            && self.filhos(p).first() == Some(&n)
        {
            return false;
        }
        if self.precedencia(init) < self.precedencia_do_pai(n) {
            return true;
        }
        if self.especie(init) == "PrefixExpression"
            && let Some(p) = self.pai(n)
            && self.especie(p) == "PrefixExpression"
            && self.texto_do_no(p).starts_with('-')
            && !self.texto_do_no(p).starts_with("--")
        {
            let t = self.texto_do_no(init);
            return t.starts_with('-');
        }
        false
    }

    /// `Expression.precedence` (`AN:dart/ast/precedence.dart`), em números
    /// crescentes de `none` (0) a `primary` (17).
    pub(crate) fn precedencia(&self, n: usize) -> u8 {
        match self.especie(n) {
            "IntegerLiteral" | "DoubleLiteral" | "BooleanLiteral" | "NullLiteral" | "SimpleStringLiteral" | "StringInterpolation" | "AdjacentStrings"
            | "SymbolLiteral" | "ListLiteral" | "SetOrMapLiteral" | "RecordLiteral" | "SimpleIdentifier" | "ParenthesizedExpression"
            | "InstanceCreationExpression" | "FunctionExpression" | "ThisExpression" | "SuperExpression" | "SwitchExpression" => 17,
            "MethodInvocation" | "PropertyAccess" | "PrefixedIdentifier" | "IndexExpression" | "PostfixExpression" | "FunctionExpressionInvocation"
            | "FunctionReference" | "DotShorthand" => 16,
            "PrefixExpression" | "AwaitExpression" => 15,
            "BinaryExpression" => self.precedencia_binaria(n),
            "AsExpression" | "IsExpression" => 8,
            "ConditionalExpression" => 3,
            "CascadeExpression" => 2,
            "AssignmentExpression" | "ThrowExpression" | "RethrowExpression" | "PatternAssignment" => 1,
            _ => 0,
        }
    }

    /// `Precedence.forTokenType` do operador da `BinaryExpression`.
    fn precedencia_binaria(&self, n: usize) -> u8 {
        let filhos = self.filhos(n);
        let (Some(&a), Some(&b)) = (filhos.first(), filhos.get(1)) else { return 0 };
        let entre = &self.fonte[self.arvore.nos[a].fim..self.arvore.nos[b].inicio];
        let operador = dartforge_frontend::lexer::lex(entre)
            .ok()
            .and_then(|v| {
                let ops: Vec<String> = v.iter().filter(|t| t.kind != Kind::Eof).map(|t| entre[t.span.start..t.span.end].to_string()).collect();
                Some(ops.concat())
            })
            .unwrap_or_default();
        match operador.as_str() {
            "??" => 4,
            "||" => 5,
            "&&" => 6,
            "==" | "!=" => 7,
            "<" | ">" | "<=" | ">=" => 8,
            "|" => 9,
            "^" => 10,
            "&" => 11,
            "<<" | ">>" | ">>>" => 12,
            "+" | "-" => 13,
            "*" | "/" | "%" | "~/" => 14,
            _ => 0,
        }
    }

    /// `getExpressionParentPrecedence` (`util.dart:162-178`).
    pub(crate) fn precedencia_do_pai(&self, n: usize) -> u8 {
        let Some(p) = self.pai(n) else { return 0 };
        match self.especie(p) {
            "ParenthesizedExpression" => 1,
            "IndexExpression" if self.filhos(p).get(1) == Some(&n) => 1,
            "AssignmentExpression" if self.filhos(p).get(1) == Some(&n) && self.pai(p).is_some_and(|g| self.especie(g) == "CascadeExpression") => 3,
            _ if self.e_expressao(p) => self.precedencia(p),
            _ => 0,
        }
    }
}

/// O nome no começo do texto de uma `VariableDeclaration`.
pub(crate) fn nome_no_inicio_pub(t: &str) -> String {
    let fim = t.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(t.len());
    t[..fim].to_string()
}

/// A string literal é crua (`r'…'`).
fn string_crua(codigo: &str) -> bool {
    codigo.starts_with('r') || codigo.starts_with('R')
}

/// A string literal usa aspas simples.
fn aspas_simples(codigo: &str) -> bool {
    codigo.trim_start_matches(['r', 'R']).starts_with('\'')
}

/// A string literal é de três aspas.
fn string_multilinha(codigo: &str) -> bool {
    let s = codigo.trim_start_matches(['r', 'R']);
    s.starts_with("'''") || s.starts_with("\"\"\"")
}

/// `contentsOffset…contentsEnd`: o texto sem as aspas.
fn conteudo_da_string(codigo: &str) -> &str {
    let s = codigo.trim_start_matches(['r', 'R']);
    let n = if string_multilinha(codigo) { 3 } else { 1 };
    if s.len() < 2 * n {
        return "";
    }
    &s[n..s.len() - n]
}

// -- Convert Getter to Method / Method to Getter -----------------------------------

/// As referências de `f` no projeto (sem as declarações), deduplicadas.
pub(crate) fn referencias_de_funcao(p: &Projeto, f: FunctionElementId, getter: bool) -> Vec<(UnitId, Span)> {
    let alvo = p.membro_de_funcao(f);
    let declaracoes = p.declaracoes(&alvo);
    let Ok(ocorrencias) = p.ocorrencias(&alvo, false) else { return Vec::new() };
    let mut v = Vec::new();
    let mut vistos: BTreeSet<(UnitId, usize, usize)> = BTreeSet::new();
    for (u, ini, fim) in ocorrencias {
        if declaracoes.contains(&(u, ini, fim)) || !vistos.insert((u, ini, fim)) {
            continue;
        }
        let Ok(Some(d)) = p.identificar(u, ini) else { continue };
        if d.concreto != Some(Concreto::Funcao(f)) {
            continue;
        }
        // Um getter só é referido onde é lido: a escrita simples é do setter.
        if getter
            && let Some(x) = d.expr
            && p.programa().unit(u).ast.exprs.iter().any(|e| matches!(&e.kind, ExprKind::Assign { op: ast::AssignOp::Assign, target, .. } if *target == x))
        {
            continue;
        }
        v.push((u, Span { start: ini, end: fim }));
    }
    v
}

/// A unidade e o `ast::FunctionId` da declaração de `f`.
fn declaracao_de_funcao(p: &Projeto, f: FunctionElementId) -> Option<(UnitId, ast::FunctionId)> {
    match p.programa().function(f).node {
        FunctionRef::Function { unit, function } => Some((unit, function)),
        _ => None,
    }
}

/// O elemento sob o cursor para os dois `Convert` (`getElementOfNode`).
fn elemento_para_converter(cx: &Contexto<'_>, o: usize) -> Elem {
    cx.elemento_no_cursor(o)
}

/// `ConvertGetterToMethodRefactoringImpl`.
fn getter_para_metodo(cx: &Contexto<'_>, pedido: &PedidoDeRefatoracao, comando: &str) -> ResultadoDeRefatoracao {
    let p = cx.p;
    let e = elemento_para_converter(cx, pedido.offset);
    // `element is PropertyAccessorElement`: getter, setter ou acessor
    // implícito.
    let acessor = match e {
        Elem::Funcao(f) => matches!(p.programa().function(f).kind, FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor),
        Elem::Variavel => true,
        _ => false,
    };
    if !acessor {
        return R::ArgumentosInvalidos(format!("Location supplied to {comando} RefactoringKind.CONVERT_GETTER_TO_METHOD is not longer valid"));
    }
    let iniciais = if !cx.no_workspace(e) {
        Estado::fatal("Only getters in your workspace can be converted.")
    } else if !matches!(e, Elem::Funcao(f) if p.programa().function(f).kind == FunctionKind::Getter) {
        Estado::fatal("Only explicit getters can be converted to methods.")
    } else {
        Estado::default()
    };
    concluir(pedido, iniciais, Estado::default, || {
        let Elem::Funcao(g) = e else { return Mudanca::default() };
        let mut m = Mudanca::default();
        let fe = p.programa().function(g);
        let membros: Vec<FunctionElementId> = if fe.class.is_none() && fe.extension.is_none() {
            vec![g]
        } else {
            match p.membro_de_funcao(g) {
                Alvo::Membro { dono, nome, estatico } => match p.familia(dono, &nome, estatico, false) {
                    Ok(fam) => {
                        let mut v: Vec<FunctionElementId> =
                            fam.funcoes.into_iter().filter(|&x| p.programa().function(x).kind == FunctionKind::Getter).collect();
                        v.sort_by_key(|x| x.0);
                        v
                    }
                    Err(_) => vec![g],
                },
                _ => vec![g],
            }
        };
        for f in membros {
            if let Some((u, fid)) = declaracao_de_funcao(p, f)
                && let Some(uri) = p.uri_da_unidade(u)
            {
                let unidade = p.programa().unit(u);
                let funcao = unidade.ast.function(fid);
                if let Some(nome) = funcao.name {
                    // O `get` antes do nome.
                    let antes = unidade.source[..nome.span.start].trim_end();
                    if antes.ends_with("get") {
                        let get = antes.len() - 3;
                        m.adicionar(&uri, Span { start: get, end: nome.span.start }, "");
                    }
                    m.adicionar(&uri, Span { start: nome.span.end, end: nome.span.end }, "()");
                }
            }
            for (u, s) in referencias_de_funcao(p, f, true) {
                if let Some(uri) = p.uri_da_unidade(u) {
                    m.adicionar(&uri, Span { start: s.end, end: s.end }, "()");
                }
            }
        }
        m
    })
}

/// `ConvertMethodToGetterRefactoringImpl`.
fn metodo_para_getter(cx: &Contexto<'_>, pedido: &PedidoDeRefatoracao, comando: &str) -> ResultadoDeRefatoracao {
    let p = cx.p;
    let e = elemento_para_converter(cx, pedido.offset);
    if !cx.executavel(e) {
        return R::ArgumentosInvalidos(format!("Location supplied to {comando} RefactoringKind.CONVERT_METHOD_TO_GETTER is not longer valid"));
    }
    let iniciais = match cx.erro_de_convert_method(e) {
        Some(m) => Estado::fatal(m),
        None => Estado::default(),
    };
    concluir(pedido, iniciais, Estado::default, || {
        let mut m = Mudanca::default();
        let membros: Vec<FunctionElementId> = match e {
            Elem::Funcao(f) if cx.e_funcao(e) => vec![f],
            Elem::Funcao(f) => match p.membro_de_funcao(f) {
                Alvo::Membro { dono, nome, estatico } => match p.familia(dono, &nome, estatico, false) {
                    Ok(fam) => {
                        let mut v: Vec<FunctionElementId> = fam
                            .funcoes
                            .into_iter()
                            .filter(|&x| matches!(p.programa().function(x).kind, FunctionKind::Function | FunctionKind::Operator))
                            .collect();
                        v.sort_by_key(|x| x.0);
                        v
                    }
                    Err(_) => vec![f],
                },
                _ => vec![f],
            },
            _ => Vec::new(),
        };
        let mut contextos: HashMap<UnitId, Contexto<'_>> = HashMap::new();
        for f in membros {
            // A declaração: `get ` no nome e sem a lista de parâmetros.
            if let Some((u, fid)) = declaracao_de_funcao(p, f)
                && let Some(uri) = p.uri_da_unidade(u)
            {
                let unidade = p.programa().unit(u);
                let funcao = unidade.ast.function(fid);
                if let (Some(nome), Some(_)) = (funcao.name, &funcao.parameters) {
                    let cxu = contextos.entry(u).or_insert_with(|| Contexto::novo(p, u));
                    let lista = cxu
                        .arvore
                        .nos
                        .iter()
                        .enumerate()
                        .find(|(_, k)| k.marca == crate::arvore_analyzer::Marca::Funcao(fid) && matches!(k.especie, "MethodDeclaration" | "FunctionDeclaration"))
                        .and_then(|(i, k)| {
                            let alvo = if k.especie == "FunctionDeclaration" {
                                cxu.filhos(i).iter().copied().find(|&c| cxu.especie(c) == "FunctionExpression").unwrap_or(i)
                            } else {
                                i
                            };
                            cxu.filhos(alvo).iter().copied().find(|&c| cxu.especie(c) == "FormalParameterList")
                        })
                        .map(|l| (cxu.arvore.nos[l].inicio, cxu.arvore.nos[l].fim));
                    m.adicionar(&uri, Span { start: nome.span.start, end: nome.span.start }, "get ");
                    if let Some((li, lf)) = lista {
                        m.adicionar(&uri, Span { start: li, end: lf }, "");
                    }
                }
            }
            // As referências: sem os argumentos da invocação.
            for (u, s) in referencias_de_funcao(p, f, false) {
                let Some(uri) = p.uri_da_unidade(u) else { continue };
                let cxu = contextos.entry(u).or_insert_with(|| Contexto::novo(p, u));
                let Some(n) = cxu.arvore.localizar(s.start, s.start) else { continue };
                if let Some(inv) = cxu.com_pais(n).find(|&k| cxu.especie(k) == "MethodInvocation") {
                    let fim = cxu.arvore.nos[inv].fim;
                    m.adicionar(&uri, Span { start: s.end, end: fim }, "");
                }
            }
        }
        m
    })
}

/// Tipos de retorno não escritos de uma declaração (`Type::Void`).
pub(crate) fn e_void(p: &Projeto, t: TypeId) -> bool {
    matches!(p.consulta.tabela.get(t), Type::Void)
}
