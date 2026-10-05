//! As diretivas `{@nome …}` dos comentários de documentação
//! (docs/ANALYZER-ESPECIFICACAO.md §A, grupo 4, e A.R4-2): o leitor de
//! argumentos do `DocCommentBuilder` (`_DirectiveParser`,
//! `analyzer/lib/src/fasta/doc_comment_builder.dart:1039-1245`) e as
//! validações do `DocCommentVerifier`
//! (`analyzer/lib/src/error/doc_comment_verifier.dart:52-160`):
//! `doc_directive_missing_argument` (uma, duas ou três faltas),
//! `doc_directive_has_extra_arguments`,
//! `doc_directive_has_unexpected_named_argument` e
//! `doc_directive_argument_wrong_format`.
//!
//! Só os comentários `///` são lidos (os `/** */` não); uma diretiva vale
//! quando abre a linha do comentário, fora de bloco de código cercado
//! (` ``` `) ou indentado. Escrito sem compilar nem executar (2026-10-04).

use crate::Unidade;
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Formato {
    Qualquer,
    Inteiro,
    Uri,
    Youtube,
}

impl Formato {
    fn exibicao(self) -> &'static str {
        match self {
            Formato::Qualquer => "any",
            Formato::Inteiro => "an integer",
            Formato::Uri => "a URI",
            Formato::Youtube => "a YouTube URL, starting with 'https://www.youtube.com/watch?v='",
        }
    }
}

const PREFIXO_DO_YOUTUBE: &str = "https://www.youtube.com/watch?v=";

/// Um tipo de diretiva (`DocDirectiveType`).
struct Tipo {
    nome: &'static str,
    posicionais: &'static [(&'static str, Formato)],
    nomeados: &'static [&'static str],
    resto: bool,
}

const TIPOS: &[Tipo] = &[
    Tipo {
        nome: "animation",
        posicionais: &[("width", Formato::Inteiro), ("height", Formato::Inteiro), ("url", Formato::Uri)],
        nomeados: &["id"],
        resto: false,
    },
    Tipo { nome: "canonicalFor", posicionais: &[("element", Formato::Qualquer)], nomeados: &[], resto: false },
    Tipo { nome: "category", posicionais: &[], nomeados: &[], resto: true },
    Tipo { nome: "end-inject-html", posicionais: &[], nomeados: &[], resto: false },
    Tipo { nome: "end-tool", posicionais: &[], nomeados: &[], resto: false },
    Tipo { nome: "endtemplate", posicionais: &[], nomeados: &[], resto: false },
    Tipo { nome: "inject-html", posicionais: &[], nomeados: &[], resto: false },
    Tipo { nome: "macro", posicionais: &[("name", Formato::Qualquer)], nomeados: &[], resto: false },
    Tipo { nome: "subCategory", posicionais: &[], nomeados: &[], resto: true },
    Tipo { nome: "template", posicionais: &[("name", Formato::Qualquer)], nomeados: &[], resto: false },
    Tipo { nome: "tool", posicionais: &[("name", Formato::Qualquer)], nomeados: &[], resto: true },
    Tipo {
        nome: "youtube",
        posicionais: &[("width", Formato::Inteiro), ("height", Formato::Inteiro), ("url", Formato::Youtube)],
        nomeados: &[],
        resto: false,
    },
];

/// Um argumento lido: o intervalo absoluto e, se é nomeado, o nome.
struct Argumento<'a> {
    span: Span,
    nome: Option<&'a str>,
    valor: &'a str,
}

fn e_branco(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c)
}

/// `int.tryParse`: decimal com sinal opcional, ou hexadecimal `0x…`.
fn e_inteiro(s: &str) -> bool {
    let t = s.strip_prefix(['-', '+']).unwrap_or(s);
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return !h.is_empty() && h.bytes().all(|b| b.is_ascii_hexdigit());
    }
    !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())
}

/// `Uri.tryParse`, pelo lado seguro: só recusa o que o analisador de URI do
/// Dart recusa com certeza (um `%` que não é seguido de dois dígitos
/// hexadecimais, e colchete de IPv6 sem fecho).
fn e_uri(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && !(b.get(i + 1).is_some_and(u8::is_ascii_hexdigit) && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit)) {
            return false;
        }
        i += 1;
    }
    !(s.contains("://[") && !s.contains(']'))
}

/// Lê e valida a diretiva que abre em `indice` (o `{@`) de `conteudo`, cujo
/// primeiro byte está em `base` na fonte.
fn diretiva(conteudo: &str, base: usize, indice: usize, saida: &mut Vec<Diagnostic>) {
    let b = conteudo.as_bytes();
    let n = b.len();
    let inicio = indice;
    let mut i = indice + 2;
    if i >= n {
        return;
    }
    let nome_ini = i;
    while i < n && !e_branco(b[i]) && b[i] != b'}' {
        i += 1;
    }
    let nome = &conteudo[nome_ini..i];
    while i < n && e_branco(b[i]) {
        i += 1;
    }
    let Some(tipo) = TIPOS.iter().find(|t| t.nome == nome) else { return };
    // `_parseArguments`.
    let mut posicionais: Vec<Argumento<'_>> = Vec::new();
    let mut nomeados: Vec<Argumento<'_>> = Vec::new();
    let mut fim = n;
    while i < n {
        if b[i] == b'}' {
            i += 1;
            fim = i;
            break;
        }
        // `_parseArgument`.
        let arg_ini = i;
        let mut so_letras_e_digitos = true;
        let mut nomeado: Option<(usize, usize)> = None;
        while i < n {
            let ch = b[i];
            if e_branco(ch) || ch == b'}' {
                break;
            }
            if ch == b'=' && so_letras_e_digitos {
                let fim_do_nome = i;
                i += 1;
                let valor_ini = i;
                while i < n && !e_branco(b[i]) && b[i] != b'}' {
                    i += 1;
                }
                nomeado = Some((fim_do_nome, valor_ini));
                break;
            }
            if !ch.is_ascii_alphanumeric() {
                so_letras_e_digitos = false;
            }
            i += 1;
        }
        let span = Span { start: base + arg_ini, end: base + i };
        match nomeado {
            Some((fim_do_nome, valor_ini)) => {
                nomeados.push(Argumento { span, nome: Some(&conteudo[arg_ini..fim_do_nome]), valor: &conteudo[valor_ini..i] })
            }
            None => posicionais.push(Argumento { span, nome: None, valor: &conteudo[arg_ini..i] }),
        }
        while i < n && e_branco(b[i]) {
            i += 1;
        }
        fim = i;
    }
    let tag = Span { start: base + inicio, end: base + fim };
    // `validateArgumentCount`.
    let exigidos = tipo.posicionais.len();
    if posicionais.len() < exigidos {
        let falta = exigidos - posicionais.len();
        let nomes: Vec<&str> = tipo.posicionais[exigidos - falta.min(3)..].iter().map(|p| p.0).collect();
        let codigo = match falta {
            1 => Some(w::DOC_DIRECTIVE_MISSING_ONE_ARGUMENT),
            2 => Some(w::DOC_DIRECTIVE_MISSING_TWO_ARGUMENTS),
            3 => Some(w::DOC_DIRECTIVE_MISSING_THREE_ARGUMENTS),
            _ => None,
        };
        if let Some(codigo) = codigo {
            let mut args: Vec<&str> = vec![tipo.nome];
            args.extend(nomes);
            saida.push(Diagnostic::com_codigo(codigo, tag, args));
        }
    }
    if !tipo.resto {
        if posicionais.len() > exigidos {
            let span = Span { start: posicionais[exigidos].span.start, end: posicionais[posicionais.len() - 1].span.end };
            let (dados, esperados) = (posicionais.len().to_string(), exigidos.to_string());
            saida.push(Diagnostic::com_codigo(w::DOC_DIRECTIVE_HAS_EXTRA_ARGUMENTS, span, [tipo.nome, dados.as_str(), esperados.as_str()]));
        }
        for a in &nomeados {
            let nome_do_argumento = a.nome.unwrap_or("");
            if !tipo.nomeados.contains(&nome_do_argumento) {
                saida.push(Diagnostic::com_codigo(w::DOC_DIRECTIVE_HAS_UNEXPECTED_NAMED_ARGUMENT, a.span, [tipo.nome, nome_do_argumento]));
            }
        }
    }
    // `validateArgumentFormat`: roda sempre.
    for (a, (nome_do_parametro, formato)) in posicionais.iter().zip(tipo.posicionais.iter()) {
        let certo = match formato {
            Formato::Qualquer => true,
            Formato::Inteiro => e_inteiro(a.valor),
            Formato::Uri => e_uri(a.valor),
            Formato::Youtube => e_uri(a.valor) && a.valor.starts_with(PREFIXO_DO_YOUTUBE),
        };
        if !certo {
            saida.push(Diagnostic::com_codigo(w::DOC_DIRECTIVE_ARGUMENT_WRONG_FORMAT, a.span, [*nome_do_parametro, formato.exibicao()]));
        }
    }
}

/// Os diagnósticos das diretivas de documentação de uma unidade.
pub fn verificar(u: Unidade<'_>) -> Vec<Diagnostic> {
    let fonte = u.fonte;
    let mut saida = Vec::new();
    let mut pos = 0;
    // O estado do comentário em curso (uma sequência de linhas `///`).
    let mut em_comentario = false;
    let mut cercado = false;
    let mut indentado = false;
    let mut anterior_vazia = true;
    for linha in fonte.split_inclusive('\n') {
        let inicio_da_linha = pos;
        pos += linha.len();
        let sem_fim = linha.trim_end_matches(['\n', '\r']);
        let recuo = sem_fim.len() - sem_fim.trim_start().len();
        let resto = &sem_fim[recuo..];
        if !resto.starts_with("///") {
            em_comentario = false;
            continue;
        }
        if !em_comentario {
            em_comentario = true;
            cercado = false;
            indentado = false;
            anterior_vazia = true;
        }
        // O conteúdo da linha: depois do `///` e de um espaço, se houver.
        let mut base = inicio_da_linha + recuo + 3;
        let mut conteudo = &resto[3..];
        if let Some(c) = conteudo.strip_prefix(' ') {
            conteudo = c;
            base += 1;
        }
        let brancos = conteudo.len() - conteudo.trim_start().len();
        let texto = &conteudo[brancos..];
        if cercado {
            if texto.starts_with("```") {
                cercado = false;
            }
            anterior_vazia = false;
            continue;
        }
        // Bloco de código indentado: quatro espaços depois de uma linha vazia,
        // até a primeira linha com menos recuo.
        if (anterior_vazia || indentado) && brancos >= 4 {
            indentado = true;
            anterior_vazia = texto.is_empty();
            continue;
        }
        indentado = false;
        if texto.starts_with("```") {
            cercado = true;
            anterior_vazia = false;
            continue;
        }
        if texto.starts_with("{@") {
            diretiva(conteudo, base, brancos, &mut saida);
            anterior_vazia = false;
            continue;
        }
        anterior_vazia = conteudo.is_empty();
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    fn relatos(fonte: &str) -> Vec<(&'static str, usize, usize)> {
        let mut nomes = dartforge_intern::Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        verificar(u).into_iter().map(|d| (d.code.map_or("", |c| c.info().nome), d.span.start, d.span.end - d.span.start)).collect()
    }

    /// O exemplo do oráculo vivo em A.R4-2 (os relatos de aridade e de
    /// argumento nomeado; `doc_directive_missing_opening_tag` é de outro
    /// verificador).
    #[test]
    fn exemplo_do_oraculo() {
        let fonte = "/// {@youtube 600 400 https://www.youtube.com/watch?v=x extra}\n\
/// {@macro name extra}\n\
/// {@category a b c}\n\
/// {@animation 600 400 http://x.mp4 foo=bar}\n\
/// {@macro name foo=bar}\n\
/// {@animation 600 400 http://x.mp4 id=a}\n\
/// {@end-inject-html extra}\n\
class A {}\n";
        let r = relatos(fonte);
        assert_eq!(
            r,
            vec![
                ("doc_directive_has_extra_arguments", 56, 5),
                ("doc_directive_has_extra_arguments", 80, 5),
                ("doc_directive_has_unexpected_named_argument", 146, 7),
                ("doc_directive_has_unexpected_named_argument", 172, 7),
                ("doc_directive_has_extra_arguments", 246, 5),
            ]
        );
    }

    #[test]
    fn faltas_e_formato() {
        let r = relatos("/// {@youtube 600}\nclass A {}\n");
        assert_eq!(r, vec![("doc_directive_missing_argument", 4, 14)]);
        let r = relatos("/// {@animation x 400 http://a}\nclass A {}\n");
        assert_eq!(r, vec![("doc_directive_argument_wrong_format", 16, 1)]);
    }

    #[test]
    fn dentro_de_bloco_cercado_nao_conta() {
        assert!(relatos("/// ```\n/// {@macro}\n/// ```\nclass A {}\n").is_empty());
    }
}
