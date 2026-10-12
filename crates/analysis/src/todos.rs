//! O `TodoFinder` do analyzer (`analyzer/lib/src/error/todo_finder.dart`,
//! `analyzer/lib/src/dart/error/todo_codes.dart`;
//! docs/ANALYZER-ESPECIFICACAO-INFRA.md §7.1): `todo`, `fixme`, `hack` e
//! `undone`, um por casamento de `Todo.TODO_REGEX` nos comentários que
//! precedem um token (os pendurados no fim do arquivo não são examinados).
//! A mensagem é o próprio texto do comentário.
//!
//! Escrito sem compilar nem executar (2026-10-04).

use dartforge_diagnostics::codigos::todo as t;
use dartforge_diagnostics::{Codigo, Diagnostic, Span};
use dartforge_frontend::token::Kind;

const PALAVRAS: [(&str, Codigo); 4] = [("TODO", t::TODO), ("FIXME", t::FIXME), ("HACK", t::HACK), ("UNDONE", t::UNDONE)];

/// Um comentário da fonte.
struct Comentario {
    inicio: usize,
    fim: usize,
    /// `/* */` (senão `//`).
    de_bloco: bool,
    linha: usize,
    coluna: usize,
}

/// `\w` do Dart sem a opção `unicode`.
fn de_palavra(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// O fim de `[^\r\n]*` a partir de `i`.
fn resto_da_linha(s: &str, i: usize) -> usize {
    s[i..].find(['\r', '\n']).map_or(s.len(), |k| i + k)
}

/// O primeiro casamento de `Todo.TODO_REGEX` em `lexema` a partir de `de`:
/// o início e o fim do grupo 2 (a palavra e o texto) e o código.
fn casar(lexema: &str, de: usize) -> Option<(usize, usize, Codigo)> {
    for (p, c) in lexema[de..].char_indices() {
        // Grupo 1: um caractere de espaço, `/` ou `*`.
        if !(c.is_whitespace() || c == '/' || c == '*') {
            continue;
        }
        let q = de + p + c.len_utf8();
        let resto = &lexema[q..];
        for (palavra, codigo) in PALAVRAS {
            if !resto.starts_with(palavra) {
                continue;
            }
            let depois = q + palavra.len();
            match lexema[depois..].chars().next() {
                // Alternativa A: um caractere que não é de palavra, o resto
                // da linha e as linhas seguintes `*` mais dois espaços.
                Some(x) if !de_palavra(x) => {
                    let mut fim = resto_da_linha(lexema, depois + x.len_utf8());
                    loop {
                        let Some(apos_quebra) = lexema[fim..].strip_prefix('\n') else { break };
                        let sem_brancos = apos_quebra.trim_start();
                        let Some(apos_marca) = sem_brancos.strip_prefix("*  ") else { break };
                        fim = resto_da_linha(lexema, lexema.len() - apos_marca.len());
                    }
                    return Some((q, fim, codigo));
                }
                // Alternativa B: a palavra no fim do lexema.
                None => return Some((q, depois, codigo)),
                Some(_) => {}
            }
        }
    }
    None
}

/// Os comentários de `fonte[de..ate]`, um vão entre dois tokens.
fn comentarios_do_vao(fonte: &str, de: usize, ate: usize, inicios_de_linha: &[usize], saida: &mut Vec<Comentario>) {
    let b = fonte.as_bytes();
    let mut i = de;
    while i + 1 < ate {
        let (fim, de_bloco) = if b[i] == b'/' && b[i + 1] == b'/' {
            let fim = fonte[i..ate].find('\n').map_or(ate, |k| i + k);
            (if fim > i && b[fim - 1] == b'\r' { fim - 1 } else { fim }, false)
        } else if b[i] == b'/' && b[i + 1] == b'*' {
            let mut nivel = 1;
            let mut j = i + 2;
            while j < ate && nivel > 0 {
                if j + 1 < ate && b[j] == b'/' && b[j + 1] == b'*' {
                    nivel += 1;
                    j += 2;
                } else if j + 1 < ate && b[j] == b'*' && b[j + 1] == b'/' {
                    nivel -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            (j.min(ate), true)
        } else if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        } else {
            // Não é branco nem comentário: o vão não é o que se esperava.
            return;
        };
        let linha = inicios_de_linha.partition_point(|&x| x <= i) - 1;
        saida.push(Comentario { inicio: i, fim, de_bloco, linha, coluna: i - inicios_de_linha[linha] });
        i = fim.max(i + 2);
    }
}

/// `replaceAll(RegExp(r'\s*\n\s*\*\s*'), ' ')`.
fn desdobrar(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(k) = resto.find('\n') {
        let depois = resto[k + 1..].trim_start();
        match depois.strip_prefix('*') {
            Some(apos) => {
                saida.push_str(resto[..k].trim_end());
                saida.push(' ');
                resto = apos.trim_start();
            }
            None => {
                saida.push_str(&resto[..=k]);
                resto = &resto[k + 1..];
            }
        }
    }
    saida.push_str(resto);
    saida
}

/// `TodoFinder.findIn`: os relatos de uma unidade.
pub fn verificar(fonte: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // Sem nenhuma das palavras, nada a fazer (o caso comum).
    if !PALAVRAS.iter().any(|(p, _)| fonte.contains(p)) {
        return out;
    }
    let Ok(tokens) = dartforge_frontend::lexer::lex(fonte) else { return out };
    let mut inicios_de_linha = vec![0usize];
    inicios_de_linha.extend(fonte.bytes().enumerate().filter(|(_, c)| *c == b'\n').map(|(i, _)| i + 1));
    let mut anterior = 0usize;
    for token in &tokens {
        // Os comentários pendurados no token de fim de arquivo ficam fora.
        if matches!(token.kind, Kind::Eof) {
            break;
        }
        let mut cs = Vec::new();
        comentarios_do_vao(fonte, anterior.min(token.span.start), token.span.start, &inicios_de_linha, &mut cs);
        anterior = token.span.end;
        let mut k = 0;
        while k < cs.len() {
            let c = &cs[k];
            let lexema = &fonte[c.inicio..c.fim];
            // Quantos comentários seguintes viraram continuação deste.
            let mut consumidos = 0;
            let mut de = 0;
            while let Some((ini, fim_do_texto, codigo)) = casar(lexema, de) {
                de = fim_do_texto.max(ini + 1);
                let offset = c.inicio + ini;
                let coluna = c.coluna + ini;
                let mut texto = lexema[ini..fim_do_texto].to_string();
                let mut fim = offset + texto.len();
                if c.de_bloco {
                    if let Some(sem) = texto.strip_suffix("*/") {
                        texto = sem.trim_end().to_string();
                        fim = offset + texto.len();
                    }
                    texto = desdobrar(&texto);
                } else {
                    // As linhas de continuação: `//` na linha seguinte, na
                    // mesma coluna, com o texto recuado um espaço a mais que
                    // a palavra, e sem outro TODO.
                    let mut linha = c.linha;
                    let mut n = consumidos;
                    while let Some(prox) = cs.get(k + 1 + n) {
                        let lex = &fonte[prox.inicio..prox.fim];
                        let recuo = lex.len() - lex.trim_start_matches(['/', ' ']).len();
                        let continua = !prox.de_bloco
                            && !lex.starts_with("///")
                            && prox.linha == linha + 1
                            && prox.coluna == c.coluna
                            && prox.coluna + recuo == coluna + 1
                            && casar(lex, 0).is_none();
                        if !continua {
                            break;
                        }
                        fim = prox.fim;
                        texto.push(' ');
                        texto.push_str(lex[recuo..].trim_end());
                        linha = prox.linha;
                        n += 1;
                    }
                    consumidos = n;
                }
                out.push(Diagnostic::com_codigo(codigo, Span { start: offset, end: fim }, [texto.as_str()]));
            }
            k += 1 + consumidos;
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    /// `(código, início, comprimento, mensagem)` de cada relato.
    fn achados(fonte: &str) -> Vec<(&'static str, usize, usize, String)> {
        verificar(fonte)
            .into_iter()
            .map(|d| (d.code.map_or("", |c| c.info().nome), d.span.start, d.span.end - d.span.start, d.message))
            .collect()
    }

    /// A tabela conferida no binário 3.6.2 (§7.1).
    #[test]
    fn tabela_do_oraculo() {
        assert_eq!(achados("/// TODO: doc\nvoid a() {}\n"), vec![("todo", 4, 9, "TODO: doc".to_string())]);
        let bloco = "/** HACK bloco\n *  continua aqui\n */\nvoid b() {}\n";
        assert_eq!(achados(bloco), vec![("hack", 4, 28, "HACK bloco continua aqui".to_string())]);
        let linhas = "// FIXME(x): linha um\n//  continua\n// nao continua\nvoid c() {}\n";
        assert_eq!(achados(linhas), vec![("fixme", 3, 31, "FIXME(x): linha um continua".to_string())]);
        assert_eq!(achados("void c() {} // UNDONE\nvoid d() {}\n"), vec![("undone", 15, 6, "UNDONE".to_string())]);
    }

    #[test]
    fn o_que_nao_casa() {
        assert!(achados("// todo: minusculo TODOS nao\nvoid a() {}\n").is_empty());
        // Pendurado no fim do arquivo.
        assert!(achados("void a() {}\n// TODO: no fim do arquivo\n").is_empty());
    }

    #[test]
    fn desdobra_o_bloco() {
        assert_eq!(desdobrar("HACK bloco\n *  continua aqui"), "HACK bloco continua aqui");
    }
}
