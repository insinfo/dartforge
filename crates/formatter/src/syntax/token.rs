//! Tokens do formatador, no formato que o `dart_style` enxerga: o fluxo do
//! scanner do analyzer, com os comentários pendurados no token seguinte
//! (`precedingComments`).
//!
//! A base é o lexer de `dartforge-frontend` ([`dartforge_frontend::lexer`]).
//! Duas diferenças de contrato são resolvidas aqui:
//!
//! * O lexer do frontend entrega strings interpoladas como trechos
//!   `StrBegin`/`StrMid`/`StrEnd` que incluem o `${`, o `$` e a `}`. O
//!   analyzer separa esses delimitadores em tokens próprios
//!   (`STRING_INTERPOLATION_EXPRESSION`, `STRING_INTERPOLATION_IDENTIFIER`,
//!   `CLOSE_CURLY_BRACKET`) e o pedaço de texto vira um `STRING` à parte; o
//!   `SourceVisitor` escreve cada um desses tokens, então eles são separados.
//! * O lexer do frontend nunca combina `>`; quem compõe `>>`, `>=`, `>>>`,
//!   `>>=` e `>>>=` é o parser (ver [`Tokens::merge_greater`]), só em
//!   contexto de expressão, como o parser do fasta faz ao dividir os tokens
//!   compostos do scanner em contexto de tipo.
//!
//! Os comentários são lidos aqui mesmo, no intervalo entre dois tokens (que
//! só contém brancos e comentários), com a mesma regra do scanner do fasta:
//! `//` vai até antes do `\n` ou `\r`, `/*` aninha.

use dartforge_frontend::token::{Keyword, Kind, Op};

/// Índice de um token em [`Tokens::list`].
pub type Tk = u32;

/// Classe do token, no grão de que o formatador precisa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Identificador, inclusive os embutidos e contextuais (`abstract`,
    /// `async`, `show`…).
    Identifier,
    /// Palavra reservada.
    Reserved(Keyword),
    /// Literal inteiro.
    Integer,
    /// Literal `double`.
    Double,
    /// Pedaço de string (`STRING` do analyzer): a string inteira quando não
    /// há interpolação, ou um trecho entre interpolações.
    Text,
    /// `${` que abre uma interpolação.
    ExpressionInterpolation,
    /// `$` antes de um identificador interpolado.
    IdentifierInterpolation,
    /// Pontuação ou operador simples.
    Op(Op),
    /// `>` composto pelo parser com os seguintes (`>>`, `>=`, `>>>`, `>>=`,
    /// `>>>=`); o texto vem do intervalo.
    ComposedGreater,
    /// `#!...` na primeira linha.
    ScriptTag,
    /// Fim do arquivo.
    Eof,
}

/// Um comentário entre dois tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comment {
    pub start: usize,
    pub end: usize,
    /// `//` (o `SINGLE_LINE_COMMENT` do analyzer).
    pub is_line: bool,
}

/// Token com posição em bytes e os comentários que o precedem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind_: TokenKind,
    pub start: usize,
    pub end: usize,
    /// O próximo token começa em `end` (sem branco nem comentário).
    pub glued: bool,
    /// Faixa em [`Tokens::comments`] com os comentários antes deste token.
    pub comments: (u32, u32),
}

/// Erro de sintaxe detectado ao ler os tokens ou a árvore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroSintaxe {
    /// Posição em bytes na fonte.
    pub posicao: usize,
    /// Comprimento em bytes do trecho apontado (pelo menos 1).
    pub comprimento: usize,
    pub mensagem: String,
}

/// O fluxo de tokens de uma unidade e seus comentários.
#[derive(Debug, Clone)]
pub struct Tokens {
    pub list: Vec<Token>,
    pub comments: Vec<Comment>,
}

impl Tokens {
    /// Lê `source` em tokens.
    ///
    /// ```
    /// use dartforge_formatter::syntax::token::{Tokens, TokenKind};
    /// let t = Tokens::read("a = 'x${b}y'; // end\n").unwrap();
    /// let textos: Vec<_> = t.list.iter().map(|k| &"a = 'x${b}y'; // end\n"[k.start..k.end]).collect();
    /// assert_eq!(textos, ["a", "=", "'x", "${", "b", "}", "y'", ";", ""]);
    /// assert_eq!(t.list.last().unwrap().kind_, TokenKind::Eof);
    /// assert_eq!(t.comments.len(), 1);
    /// ```
    ///
    /// # Erros
    /// Caractere inesperado, string ou comentário de bloco sem fecho, e os
    /// operadores `===`/`!==`, que o scanner do fasta aceita só para relatar
    /// erro.
    pub fn read(source: &str) -> Result<Tokens, ErroSintaxe> {
        let raw_tokens = dartforge_frontend::lexer::lex(source).map_err(|d| ErroSintaxe {
            posicao: d.span.start,
            comprimento: (d.span.end - d.span.start).max(1),
            mensagem: d.message.clone(),
        })?;
        let mut list: Vec<Token> = Vec::with_capacity(raw_tokens.len() + 8);
        // Pilha das interpolações abertas, por literal: `true` para `${…}`.
        // O trecho que segue uma interpolação `${…}` começa pela `}`.
        let mut open_groups: Vec<bool> = Vec::new();
        let new = |kind_, start, end| Token {
            kind_,
            start,
            end,
            glued: false,
            comments: (0, 0),
        };
        for b in &raw_tokens {
            let (start_, end) = (b.span.start, b.span.end);
            let text = &source[start_..end];
            match b.kind {
                Kind::Ident => {
                    if !text.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'$') {
                        return Err(ErroSintaxe {
                            posicao: start_,
                            comprimento: end - start_,
                            mensagem: format!("Unexpected character in '{text}'."),
                        });
                    }
                    list.push(new(TokenKind::Identifier, start_, end));
                }
                Kind::Keyword(k) => list.push(new(TokenKind::Reserved(k), start_, end)),
                Kind::Int => list.push(new(TokenKind::Integer, start_, end)),
                Kind::Double => list.push(new(TokenKind::Double, start_, end)),
                Kind::Str(f) => {
                    if f.aberta {
                        return Err(string_error(start_, end));
                    }
                    list.push(new(TokenKind::Text, start_, end));
                }
                Kind::StrBegin(_, interp) | Kind::StrMid(_, interp) => {
                    let mut i = start_;
                    let key = matches!(b.kind, Kind::StrMid(..)) && open_groups.pop().unwrap_or(false);
                    open_groups.push(interp == dartforge_frontend::token::Interp::Brace);
                    if key {
                        list.push(new(TokenKind::Op(Op::RBrace), i, i + 1));
                        i += 1;
                    }
                    let delim = match interp {
                        dartforge_frontend::token::Interp::Brace => 2,
                        dartforge_frontend::token::Interp::Ident => 1,
                    };
                    list.push(new(TokenKind::Text, i, end - delim));
                    let kind_ = if delim == 2 {
                        TokenKind::ExpressionInterpolation
                    } else {
                        TokenKind::IdentifierInterpolation
                    };
                    list.push(new(kind_, end - delim, end));
                }
                Kind::StrEnd(f) => {
                    if f.aberta {
                        return Err(string_error(start_, end));
                    }
                    let mut i = start_;
                    if open_groups.pop().unwrap_or(false) {
                        list.push(new(TokenKind::Op(Op::RBrace), i, i + 1));
                        i += 1;
                    }
                    list.push(new(TokenKind::Text, i, end));
                }
                Kind::Op(op) => {
                    if (op == Op::EqEq || op == Op::BangEq) && end - start_ == 3 {
                        return Err(ErroSintaxe {
                            posicao: start_,
                            comprimento: 3,
                            mensagem: format!("The '{text}' operator is not supported."),
                        });
                    }
                    list.push(new(TokenKind::Op(op), start_, end));
                }
                Kind::ScriptTag => list.push(new(TokenKind::ScriptTag, start_, end)),
                Kind::Eof => list.push(new(TokenKind::Eof, start_, end)),
            }
        }
        // Colagem e comentários a partir das posições finais.
        let mut comments = Vec::new();
        let bytes = source.as_bytes();
        let mut previous_end = 0usize;
        for i in 0..list.len() {
            let start_ = list[i].start;
            let c0 = comments.len() as u32;
            let mut p = previous_end;
            while p < start_ {
                let c = bytes[p];
                if c == b'/' && bytes.get(p + 1) == Some(&b'/') {
                    let s = p;
                    while p < start_ && bytes[p] != b'\n' && bytes[p] != b'\r' {
                        p += 1;
                    }
                    comments.push(Comment {
                        start: s,
                        end: p,
                        is_line: true,
                    });
                } else if c == b'/' && bytes.get(p + 1) == Some(&b'*') {
                    let s = p;
                    p += 2;
                    let mut level = 1usize;
                    while p < bytes.len() && level > 0 {
                        if bytes[p] == b'/' && bytes.get(p + 1) == Some(&b'*') {
                            level += 1;
                            p += 2;
                        } else if bytes[p] == b'*' && bytes.get(p + 1) == Some(&b'/') {
                            level -= 1;
                            p += 2;
                        } else {
                            p += 1;
                        }
                    }
                    if level > 0 {
                        return Err(ErroSintaxe {
                            posicao: s,
                            comprimento: 2,
                            mensagem: "Unterminated multi-line comment.".into(),
                        });
                    }
                    comments.push(Comment {
                        start: s,
                        end: p,
                        is_line: false,
                    });
                } else {
                    p += 1;
                }
            }
            list[i].comments = (c0, comments.len() as u32);
            if i > 0 {
                list[i - 1].glued = list[i - 1].end == start_;
            }
            previous_end = list[i].end.max(previous_end);
        }
        Ok(Tokens { list, comments })
    }

    /// Texto do token `t`.
    pub fn text<'a>(&self, source: &'a str, t: Tk) -> &'a str {
        let k = &self.list[t as usize];
        &source[k.start..k.end]
    }

    /// Os comentários que precedem o token `t`.
    pub fn comments_of(&self, t: Tk) -> &[Comment] {
        let (a, b) = self.list[t as usize].comments;
        &self.comments[a as usize..b as usize]
    }

    /// Junta o `>` em `t` com os `>`/`=` colados que o seguem, até formar o
    /// operador `target_` (`">>"`, `">="`, `">>>"`, `">>="` ou `">>>="`). Só é
    /// chamado pelo parser em contexto de expressão, logo depois de conferir
    /// com [`Tokens::composed_greater`]. Os tokens absorvidos saem da lista;
    /// nenhum nó aponta para eles ainda, porque o parser está exatamente em `t`.
    pub fn merge_greater(&mut self, t: Tk, target_: &str) {
        let i = t as usize;
        let n = target_.len();
        let end = self.list[i].start + n;
        let glued_last = self.list[i + n - 1].glued;
        self.list.drain(i + 1..i + n);
        self.list[i].end = end;
        self.list[i].kind_ = TokenKind::ComposedGreater;
        self.list[i].glued = glued_last;
    }

    /// Se o `>` em `t` começa um operador composto por tokens colados,
    /// devolve o maior deles (`>>>=`, `>>>`, `>>=`, `>>`, `>=`).
    pub fn composed_greater(&self, t: Tk) -> Option<&'static str> {
        let i = t as usize;
        let e = |k: usize, op: Op| self.list.get(i + k).is_some_and(|x| x.kind_ == TokenKind::Op(op));
        let col = |k: usize| self.list.get(i + k).is_some_and(|x| x.glued);
        if !e(0, Op::Gt) || !col(0) {
            return None;
        }
        if e(1, Op::Gt) {
            if col(1) && e(2, Op::Gt) {
                if col(2) && e(3, Op::Assign) {
                    return Some(">>>=");
                }
                return Some(">>>");
            }
            if col(1) && e(2, Op::Assign) {
                return Some(">>=");
            }
            return Some(">>");
        }
        if e(1, Op::Assign) {
            return Some(">=");
        }
        // `>==`, `>=>` etc. não existem; `>` seguido de `==` é `>=` e `=`.
        if self.list.get(i + 1).is_some_and(|x| x.kind_ == TokenKind::Op(Op::EqEq)) {
            return None;
        }
        None
    }
}

fn string_error(start_: usize, end: usize) -> ErroSintaxe {
    ErroSintaxe {
        posicao: start_,
        comprimento: (end - start_).max(1),
        mensagem: "Unterminated string literal.".into(),
    }
}
