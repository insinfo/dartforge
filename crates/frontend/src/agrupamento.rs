//! A pilha de agrupamento do scanner do fasta (`abstract_scanner.dart`,
//! `appendBeginGroup`, `appendEndGroup`, `discardBeginGroupUntil`,
//! `insertSyntheticClosers`, `unmatchedBeginGroup`, `appendEofToken`),
//! refeita sobre os tokens do nosso lexer: o fecho que falta vira um token
//! sintético de comprimento zero (o `SyntheticToken`) com o erro
//! `EXPECTED_TOKEN` do scanner (o `UnmatchedToken`, `errors.dart:71`), e o
//! fecho que sobra fica como token comum.
//!
//! Na recuperação, o fasta compara duas opções (inserir os fechos ou ignorar
//! o fecho lido) reescaneando no máximo 100 chamadas do `bigSwitch` à
//! frente e contando as recuperações (`recoveryOptionTokenizer`); aqui a
//! contagem anda pelos mesmos tokens, com o peso de chamadas de cada um
//! (os espaços em branco e os comentários entre eles também são chamadas).
//!
//! Diferença conhecida: o nosso lexer fecha a interpolação `${…}` na `}`
//! que a termina e não sabe reabrir a string; quando a recuperação do fasta
//! descartaria um grupo de interpolação, aqui o fecho lido é ignorado.

use crate::token::{Interp, Keyword, Kind, Op, Token};
use dartforge_diagnostics::{Diagnostic, Span, codigos};

/// O `kind` de um `BeginToken`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grupo {
    Parenteses,
    Colchetes,
    Chaves,
    Menor,
    Interpolacao,
}

impl Grupo {
    /// `closeBraceInfoFor`.
    fn fecho(self) -> (Op, &'static str) {
        match self {
            Grupo::Parenteses => (Op::RParen, ")"),
            Grupo::Colchetes => (Op::RBracket, "]"),
            Grupo::Chaves | Grupo::Interpolacao => (Op::RBrace, "}"),
            Grupo::Menor => (Op::Gt, ">"),
        }
    }
}

/// O que uma chamada do `bigSwitch` faz com a pilha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Efeito {
    Nada,
    Abre(Grupo),
    /// `appendEndGroup` com o `openKind` (para `}`, também a interpolação).
    Fecha(Grupo),
    /// `appendGt`, `appendGtGt`, `appendGtGtGt`.
    Maior(u8),
    /// `discardOpenLt`: `;`, os tokens que começam com `=`, `this`.
    DescartaMenor,
}

/// Um token nosso visto como chamadas do `bigSwitch`.
#[derive(Debug, Clone, Copy)]
struct Passo {
    /// Chamadas de espaço em branco e comentário antes do token.
    trivia: u32,
    /// Chamadas do próprio token (0 quando é a continuação de outro).
    chamadas: u32,
    primeiro: Efeito,
    /// O efeito da segunda chamada (a continuação da string depois da `}`
    /// de uma interpolação, que pode abrir outra).
    segundo: Efeito,
}

/// Chamadas do `bigSwitch` no trecho entre dois tokens: cada caractere de
/// espaço em branco (seguido dos espaços que vierem) e cada comentário.
fn chamadas_de_trivia(texto: &str) -> u32 {
    let b = texto.as_bytes();
    let mut i = 0;
    let mut n = 0;
    while i < b.len() {
        match b[i] {
            b' ' | b'\t' | b'\n' | b'\r' => {
                n += 1;
                i += 1;
                while i < b.len() && b[i] == b' ' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                n += 1;
                while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                n += 1;
                let mut nivel = 0usize;
                while i < b.len() {
                    if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                        nivel += 1;
                        i += 2;
                    } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                        nivel -= 1;
                        i += 2;
                        if nivel == 0 {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            _ => i += 1,
        }
    }
    n
}

fn passos(source: &str, tokens: &[Token]) -> Vec<Passo> {
    let mut saida = Vec::with_capacity(tokens.len());
    // A interpolação do último trecho de cada string aberta.
    let mut trechos: Vec<Interp> = Vec::new();
    let mut fim_anterior = 0usize;
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i];
        let trivia = chamadas_de_trivia(source.get(fim_anterior..t.span.start).unwrap_or(""));
        fim_anterior = t.span.end;
        let texto = &source[t.span.start..t.span.end];
        let mut p = Passo { trivia, chamadas: 1, primeiro: Efeito::Nada, segundo: Efeito::Nada };
        match t.kind {
            Kind::StrBegin(_, interp) => {
                trechos.push(interp);
                if interp == Interp::Brace {
                    p.primeiro = Efeito::Abre(Grupo::Interpolacao);
                }
            }
            Kind::StrMid(..) | Kind::StrEnd(_) => {
                let anterior = trechos.pop();
                let nova = match t.kind {
                    Kind::StrMid(_, interp) => Some(interp),
                    _ => None,
                };
                let abre = if nova == Some(Interp::Brace) { Efeito::Abre(Grupo::Interpolacao) } else { Efeito::Nada };
                if anterior == Some(Interp::Brace) {
                    // A `}` e a continuação da string: duas chamadas.
                    p.primeiro = Efeito::Fecha(Grupo::Interpolacao);
                    p.chamadas = 2;
                    p.segundo = abre;
                } else {
                    // Depois de `$nome`, a mesma chamada do `tokenizeString`.
                    p.chamadas = 0;
                    p.primeiro = abre;
                }
                if let Some(interp) = nova {
                    trechos.push(interp);
                }
            }
            Kind::Ident
                if i > 0
                    && matches!(tokens[i - 1].kind, Kind::StrBegin(_, Interp::Ident) | Kind::StrMid(_, Interp::Ident))
                    && tokens[i - 1].span.end == t.span.start =>
            {
                p.chamadas = 0;
            }
            Kind::Keyword(Keyword::This) => p.primeiro = Efeito::DescartaMenor,
            Kind::Op(op) => match op {
                Op::LParen => p.primeiro = Efeito::Abre(Grupo::Parenteses),
                Op::LBrace => p.primeiro = Efeito::Abre(Grupo::Chaves),
                Op::Lt => p.primeiro = Efeito::Abre(Grupo::Menor),
                Op::RParen => p.primeiro = Efeito::Fecha(Grupo::Parenteses),
                Op::RBracket => p.primeiro = Efeito::Fecha(Grupo::Colchetes),
                Op::RBrace => p.primeiro = Efeito::Fecha(Grupo::Chaves),
                Op::Semicolon => p.primeiro = Efeito::DescartaMenor,
                Op::LBracket => {
                    // `[]` e `[]=` são o token de índice, sem grupo.
                    if t.glued && tokens.get(i + 1).is_some_and(|n| n.kind == Kind::Op(Op::RBracket)) {
                        saida.push(p);
                        let mut j = i + 1;
                        let colado_igual = tokens[j].glued && tokens.get(j + 1).is_some_and(|n| n.kind == Kind::Op(Op::Assign));
                        let ate = if colado_igual { j + 1 } else { j };
                        while j <= ate {
                            saida.push(Passo { trivia: 0, chamadas: 0, primeiro: Efeito::Nada, segundo: Efeito::Nada });
                            fim_anterior = tokens[j].span.end;
                            j += 1;
                        }
                        i = ate + 1;
                        continue;
                    }
                    p.primeiro = Efeito::Abre(Grupo::Colchetes);
                }
                Op::Gt => {
                    // `>`, `>>`, `>>>` e, colados a um `=`, `>=`, `>>=`,
                    // `>>>=` (sem efeito na pilha).
                    let mut n = 1usize;
                    while n < 3 && tokens[i + n - 1].glued && tokens.get(i + n).is_some_and(|x| x.kind == Kind::Op(Op::Gt)) {
                        n += 1;
                    }
                    let ultimo = i + n - 1;
                    let seguinte = tokens.get(ultimo + 1).filter(|_| tokens[ultimo].glued);
                    let igual = seguinte.filter(|x| source[x.span.start..x.span.end].starts_with('='));
                    p.primeiro = if igual.is_some() { Efeito::Nada } else { Efeito::Maior(n as u8) };
                    saida.push(p);
                    for k in i + 1..=ultimo {
                        saida.push(Passo { trivia: 0, chamadas: 0, primeiro: Efeito::Nada, segundo: Efeito::Nada });
                        fim_anterior = tokens[k].span.end;
                    }
                    i = ultimo + 1;
                    if let Some(x) = igual {
                        // O resto do token depois do `=` que o `>` levou.
                        let resto = &source[x.span.start + 1..x.span.end];
                        let efeito = if resto.starts_with('=') {
                            Efeito::DescartaMenor
                        } else if resto.starts_with('>') {
                            Efeito::Maior(1)
                        } else {
                            Efeito::Nada
                        };
                        saida.push(Passo { trivia: 0, chamadas: u32::from(!resto.is_empty()), primeiro: efeito, segundo: Efeito::Nada });
                        fim_anterior = x.span.end;
                        i += 1;
                    }
                    continue;
                }
                _ if texto.starts_with('=') => p.primeiro = Efeito::DescartaMenor,
                _ => {}
            },
            _ => {}
        }
        saida.push(p);
        i += 1;
    }
    saida
}

/// O resultado de um `discardBeginGroupUntil`.
enum Busca {
    /// O grupo do topo é o esperado (depois de descartar os `<`).
    Direto,
    /// Achado mais abaixo: a altura da pilha com ele no topo.
    Abaixo(usize),
    /// Não há abridor da espécie: a pilha volta ao que era.
    Nenhum,
}

fn descartar_menor(pilha: &mut Vec<(Grupo, usize)>) {
    while pilha.last().is_some_and(|g| g.0 == Grupo::Menor) {
        pilha.pop();
    }
}

fn casa(fecho: Grupo, abridor: Grupo) -> bool {
    // A `}` de uma interpolação é uma `}` comum para o scanner (`openKind`
    // `{`), que aceita o `{` e o `${`.
    match fecho {
        Grupo::Chaves | Grupo::Interpolacao => matches!(abridor, Grupo::Chaves | Grupo::Interpolacao),
        _ => fecho == abridor,
    }
}

/// O laço do `discardBeginGroupUntil`, sem decidir a recuperação. Deixa a
/// pilha como o fasta a deixa: sem os `<` do topo no caso direto.
fn buscar(pilha: &mut Vec<(Grupo, usize)>, fecho: Grupo) -> Busca {
    let original = pilha.clone();
    let mut primeiro = true;
    loop {
        descartar_menor(pilha);
        let Some(&(topo, _)) = pilha.last() else { break };
        if casa(fecho, topo) {
            if primeiro {
                return Busca::Direto;
            }
            return Busca::Abaixo(pilha.len());
        }
        primeiro = false;
        pilha.pop();
        if pilha.is_empty() {
            break;
        }
    }
    *pilha = original;
    Busca::Nenhum
}

fn aplicar_maior(pilha: &mut Vec<(Grupo, usize)>, n: u8) {
    for _ in 0..n {
        if pilha.last().is_some_and(|g| g.0 == Grupo::Menor) {
            pilha.pop();
        } else {
            break;
        }
    }
}

/// O `recoveryOptionTokenizer` de uma opção, a partir do passo `desde`:
/// as recuperações contadas em no máximo 101 chamadas do `bigSwitch`.
fn simular(passos: &[Passo], desde: usize, pilha: &mut Vec<(Grupo, usize)>, mut recuperacoes: usize) -> usize {
    let mut iteracoes = 0u32;
    // O último passo é o `Eof`, que o laço não lê.
    for p in passos.iter().take(passos.len().saturating_sub(1)).skip(desde) {
        iteracoes += p.trivia;
        if iteracoes > 100 {
            return recuperacoes;
        }
        if p.chamadas == 0 {
            efeito_simulado(p.primeiro, pilha, &mut recuperacoes);
            continue;
        }
        iteracoes += 1;
        efeito_simulado(p.primeiro, pilha, &mut recuperacoes);
        if iteracoes > 100 {
            return recuperacoes;
        }
        if p.chamadas == 2 {
            iteracoes += 1;
            efeito_simulado(p.segundo, pilha, &mut recuperacoes);
            if iteracoes > 100 {
                return recuperacoes;
            }
        }
    }
    recuperacoes
}

fn efeito_simulado(efeito: Efeito, pilha: &mut Vec<(Grupo, usize)>, recuperacoes: &mut usize) {
    match efeito {
        Efeito::Nada => {}
        Efeito::Abre(g) => {
            if !matches!(g, Grupo::Menor | Grupo::Parenteses) {
                descartar_menor(pilha);
            }
            pilha.push((g, 0));
        }
        Efeito::DescartaMenor => descartar_menor(pilha),
        Efeito::Maior(n) => aplicar_maior(pilha, n),
        Efeito::Fecha(g) => {
            let altura = pilha.len();
            match buscar(pilha, g) {
                Busca::Direto => {
                    pilha.pop();
                }
                Busca::Abaixo(h) => {
                    *recuperacoes += 1;
                    // `insertSyntheticClosers`: um `unmatchedBeginGroup` por
                    // entrada acima da achada, os `<` inclusive.
                    *recuperacoes += altura - h;
                    pilha.pop();
                }
                Busca::Nenhum => *recuperacoes += 1,
            }
        }
    }
}

/// Um fecho sintético: o `endGroup` de um abridor que o scanner não casou.
#[derive(Debug, Clone, Copy)]
pub struct FechoSintetico {
    /// O início do abridor.
    pub abre: usize,
    /// Onde o fecho está (o `charOffset` do `SyntheticToken`).
    pub em: usize,
    /// O índice do erro dele entre os erros devolvidos.
    pub erro: usize,
}

/// Os tokens com os fechos sintéticos inseridos, os erros do scanner e os
/// fechos (para o `moveSynthetic` do parser).
pub fn agrupar(source: &str, tokens: Vec<Token>) -> (Vec<Token>, Vec<Diagnostic>, Vec<FechoSintetico>) {
    let passos = passos(source, &tokens);
    debug_assert_eq!(passos.len(), tokens.len());
    let mut pilha: Vec<(Grupo, usize)> = Vec::new();
    // (antes do token de índice, token sintético)
    let mut insercoes: Vec<(usize, Token)> = Vec::new();
    let mut erros = Vec::new();
    let mut fechos: Vec<FechoSintetico> = Vec::new();
    let sintetico = |op: Op, em: usize| Token { kind: Kind::Op(op), span: Span { start: em, end: em }, glued: false };
    let nao_casado = |g: Grupo, abridor: usize, em: usize, antes: usize, insercoes: &mut Vec<(usize, Token)>, erros: &mut Vec<Diagnostic>, fechos: &mut Vec<FechoSintetico>| {
        let (op, texto) = g.fecho();
        insercoes.push((antes, sintetico(op, em)));
        fechos.push(FechoSintetico { abre: tokens[abridor].span.start, em, erro: erros.len() });
        erros.push(Diagnostic::com_codigo(codigos::scanner::EXPECTED_TOKEN, Span { start: em, end: em + 1 }, [texto]));
    };
    for (i, (t, p)) in tokens.iter().zip(&passos).enumerate() {
        if t.kind == Kind::Eof {
            // `appendEofToken`.
            descartar_menor(&mut pilha);
            while let Some((g, abridor)) = pilha.pop() {
                nao_casado(g, abridor, t.span.start, i, &mut insercoes, &mut erros, &mut fechos);
            }
            break;
        }
        for (k, efeito) in [p.primeiro, p.segundo].into_iter().enumerate() {
            if k == 1 && p.chamadas < 2 {
                break;
            }
            match efeito {
                Efeito::Nada => {}
                Efeito::Abre(g) => {
                    if !matches!(g, Grupo::Menor | Grupo::Parenteses) {
                        descartar_menor(&mut pilha);
                    }
                    pilha.push((g, i));
                }
                Efeito::DescartaMenor => descartar_menor(&mut pilha),
                Efeito::Maior(n) => aplicar_maior(&mut pilha, n),
                Efeito::Fecha(g) => {
                    let original = pilha.clone();
                    match buscar(&mut pilha, g) {
                        Busca::Direto => {
                            pilha.pop();
                        }
                        Busca::Nenhum => {}
                        Busca::Abaixo(h) => {
                            let interpolacao_no_meio = original[h..].iter().any(|e| e.0 == Grupo::Interpolacao);
                            let opcao1 = {
                                let mut s = pilha.clone();
                                s.pop();
                                let r = simular(&passos, i + 1, &mut s, original.len() - h);
                                r + s.len()
                            };
                            let opcao2 = {
                                let mut s = original.clone();
                                let r = simular(&passos, i + 1, &mut s, 0);
                                r + s.len() + 1
                            };
                            let fecho_de_interpolacao = g == Grupo::Interpolacao;
                            if !fecho_de_interpolacao && (opcao2 < opcao1 || interpolacao_no_meio) {
                                // Opção 2: o fecho fica como token comum.
                                pilha = original;
                            } else {
                                for &(e, abridor) in original[h..].iter().rev() {
                                    nao_casado(e, abridor, t.span.start, i, &mut insercoes, &mut erros, &mut fechos);
                                }
                                pilha.pop();
                            }
                        }
                    }
                }
            }
        }
    }
    if insercoes.is_empty() {
        return (tokens, erros, fechos);
    }
    let mut saida = Vec::with_capacity(tokens.len() + insercoes.len());
    let mut proxima = insercoes.into_iter().peekable();
    for (i, t) in tokens.into_iter().enumerate() {
        while let Some((_, s)) = proxima.next_if(|(antes, _)| *antes == i) {
            saida.push(s);
        }
        saida.push(t);
    }
    (saida, erros, fechos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fechos(src: &str) -> (Vec<String>, Vec<(usize, String)>) {
        let tokens = crate::lexer::lex(src).unwrap();
        let (t, e, _) = agrupar(src, tokens);
        let sinteticos = t
            .iter()
            .filter(|t| t.span.start == t.span.end && t.kind != Kind::Eof)
            .map(|t| format!("{:?}@{}", t.kind, t.span.start))
            .collect();
        let erros = e.iter().map(|d| (d.span.start, d.message.clone())).collect();
        (sinteticos, erros)
    }

    #[test]
    fn parenteses_aberto_antes_da_chave() {
        let src = "void m() {\n  f(1, \n}\n";
        let (s, e) = fechos(src);
        let chave = src.rfind('}').unwrap();
        assert_eq!(s, vec![format!("Op(RParen)@{chave}")]);
        assert_eq!(e, vec![(chave, "Expected to find ')'.".to_string())]);
    }

    #[test]
    fn menor_aberto_tambem_ganha_fecho() {
        // O `insertSyntheticClosers` olha o abridor achado, não o `<`.
        let src = "void m() {\n  f(a < b, 1\n}\n";
        let (s, _) = fechos(src);
        let chave = src.rfind('}').unwrap();
        assert_eq!(s, vec![format!("Op(Gt)@{chave}"), format!("Op(RParen)@{chave}")]);
    }

    #[test]
    fn colchete_com_parenteses_dentro() {
        let src = "void n() {\n  [1, (2]\n}\n";
        let (s, _) = fechos(src);
        let colchete = src.find(']').unwrap();
        assert_eq!(s, vec![format!("Op(RParen)@{colchete}")]);
    }

    #[test]
    fn fim_do_arquivo_fecha_tudo() {
        let src = "void m() {\n  f(1";
        let (s, e) = fechos(src);
        assert_eq!(s, vec![format!("Op(RParen)@{}", src.len()), format!("Op(RBrace)@{}", src.len())]);
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn indice_e_genericos_nao_abrem_grupo() {
        let (s, e) = fechos("operator []=(i, v) {}\nList<List<int>> x = [];\nvar y = a < b;\n");
        assert!(s.is_empty() && e.is_empty(), "{s:?} {e:?}");
    }
}
