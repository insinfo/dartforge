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
//!   `>>=` e `>>>=` é o parser (ver [`Tokens::combinar_maior`]), só em
//!   contexto de expressão, como o parser do fasta faz ao dividir os tokens
//!   compostos do scanner em contexto de tipo.
//!
//! Os comentários são lidos aqui mesmo, no intervalo entre dois tokens (que
//! só contém brancos e comentários), com a mesma regra do scanner do fasta:
//! `//` vai até antes do `\n` ou `\r`, `/*` aninha.

use dartforge_frontend::token::{Keyword, Kind, Op};

/// Índice de um token em [`Tokens::lista`].
pub type Tk = u32;

/// Classe do token, no grão de que o formatador precisa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoToken {
    /// Identificador, inclusive os embutidos e contextuais (`abstract`,
    /// `async`, `show`…).
    Identificador,
    /// Palavra reservada.
    Reservada(Keyword),
    /// Literal inteiro.
    Inteiro,
    /// Literal `double`.
    Duplo,
    /// Pedaço de string (`STRING` do analyzer): a string inteira quando não
    /// há interpolação, ou um trecho entre interpolações.
    Texto,
    /// `${` que abre uma interpolação.
    InterpolacaoExpressao,
    /// `$` antes de um identificador interpolado.
    InterpolacaoIdentificador,
    /// Pontuação ou operador simples.
    Op(Op),
    /// `>` composto pelo parser com os seguintes (`>>`, `>=`, `>>>`, `>>=`,
    /// `>>>=`); o texto vem do intervalo.
    MaiorComposto,
    /// `#!...` na primeira linha.
    ScriptTag,
    /// Fim do arquivo.
    Eof,
}

/// Um comentário entre dois tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comentario {
    pub inicio: usize,
    pub fim: usize,
    /// `//` (o `SINGLE_LINE_COMMENT` do analyzer).
    pub de_linha: bool,
}

/// Token com posição em bytes e os comentários que o precedem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub tipo: TipoToken,
    pub inicio: usize,
    pub fim: usize,
    /// O próximo token começa em `fim` (sem branco nem comentário).
    pub colado: bool,
    /// Faixa em [`Tokens::comentarios`] com os comentários antes deste token.
    pub comentarios: (u32, u32),
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
    pub lista: Vec<Token>,
    pub comentarios: Vec<Comentario>,
}

impl Tokens {
    /// Lê `fonte` em tokens.
    ///
    /// ```
    /// use dartforge_formatter::sintaxe::token::{Tokens, TipoToken};
    /// let t = Tokens::ler("a = 'x${b}y'; // fim\n").unwrap();
    /// let textos: Vec<_> = t.lista.iter().map(|k| &"a = 'x${b}y'; // fim\n"[k.inicio..k.fim]).collect();
    /// assert_eq!(textos, ["a", "=", "'x", "${", "b", "}", "y'", ";", ""]);
    /// assert_eq!(t.lista.last().unwrap().tipo, TipoToken::Eof);
    /// assert_eq!(t.comentarios.len(), 1);
    /// ```
    ///
    /// # Erros
    /// Caractere inesperado, string ou comentário de bloco sem fecho, e os
    /// operadores `===`/`!==`, que o scanner do fasta aceita só para relatar
    /// erro.
    pub fn ler(fonte: &str) -> Result<Tokens, ErroSintaxe> {
        let brutos = dartforge_frontend::lexer::lex(fonte).map_err(|d| ErroSintaxe {
            posicao: d.span.start,
            comprimento: (d.span.end - d.span.start).max(1),
            mensagem: d.message.clone(),
        })?;
        let mut lista: Vec<Token> = Vec::with_capacity(brutos.len() + 8);
        // Pilha das interpolações abertas, por literal: `true` para `${…}`.
        // O trecho que segue uma interpolação `${…}` começa pela `}`.
        let mut abertas: Vec<bool> = Vec::new();
        let novo = |tipo, inicio, fim| Token {
            tipo,
            inicio,
            fim,
            colado: false,
            comentarios: (0, 0),
        };
        for b in &brutos {
            let (ini, fim) = (b.span.start, b.span.end);
            let texto = &fonte[ini..fim];
            match b.kind {
                Kind::Ident => {
                    if !texto.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'$') {
                        return Err(ErroSintaxe {
                            posicao: ini,
                            comprimento: fim - ini,
                            mensagem: format!("Unexpected character in '{texto}'."),
                        });
                    }
                    lista.push(novo(TipoToken::Identificador, ini, fim));
                }
                Kind::Keyword(k) => lista.push(novo(TipoToken::Reservada(k), ini, fim)),
                Kind::Int => lista.push(novo(TipoToken::Inteiro, ini, fim)),
                Kind::Double => lista.push(novo(TipoToken::Duplo, ini, fim)),
                Kind::Str(f) => {
                    if f.aberta {
                        return Err(erro_string(ini, fim));
                    }
                    lista.push(novo(TipoToken::Texto, ini, fim));
                }
                Kind::StrBegin(_, interp) | Kind::StrMid(_, interp) => {
                    let mut i = ini;
                    let chave = matches!(b.kind, Kind::StrMid(..)) && abertas.pop().unwrap_or(false);
                    abertas.push(interp == dartforge_frontend::token::Interp::Brace);
                    if chave {
                        lista.push(novo(TipoToken::Op(Op::RBrace), i, i + 1));
                        i += 1;
                    }
                    let delim = match interp {
                        dartforge_frontend::token::Interp::Brace => 2,
                        dartforge_frontend::token::Interp::Ident => 1,
                    };
                    lista.push(novo(TipoToken::Texto, i, fim - delim));
                    let tipo = if delim == 2 {
                        TipoToken::InterpolacaoExpressao
                    } else {
                        TipoToken::InterpolacaoIdentificador
                    };
                    lista.push(novo(tipo, fim - delim, fim));
                }
                Kind::StrEnd(f) => {
                    if f.aberta {
                        return Err(erro_string(ini, fim));
                    }
                    let mut i = ini;
                    if abertas.pop().unwrap_or(false) {
                        lista.push(novo(TipoToken::Op(Op::RBrace), i, i + 1));
                        i += 1;
                    }
                    lista.push(novo(TipoToken::Texto, i, fim));
                }
                Kind::Op(op) => {
                    if (op == Op::EqEq || op == Op::BangEq) && fim - ini == 3 {
                        return Err(ErroSintaxe {
                            posicao: ini,
                            comprimento: 3,
                            mensagem: format!("The '{texto}' operator is not supported."),
                        });
                    }
                    lista.push(novo(TipoToken::Op(op), ini, fim));
                }
                Kind::ScriptTag => lista.push(novo(TipoToken::ScriptTag, ini, fim)),
                Kind::Eof => lista.push(novo(TipoToken::Eof, ini, fim)),
            }
        }
        // Colagem e comentários a partir das posições finais.
        let mut comentarios = Vec::new();
        let bytes = fonte.as_bytes();
        let mut anterior_fim = 0usize;
        for i in 0..lista.len() {
            let ini = lista[i].inicio;
            let c0 = comentarios.len() as u32;
            let mut p = anterior_fim;
            while p < ini {
                let c = bytes[p];
                if c == b'/' && bytes.get(p + 1) == Some(&b'/') {
                    let s = p;
                    while p < ini && bytes[p] != b'\n' && bytes[p] != b'\r' {
                        p += 1;
                    }
                    comentarios.push(Comentario {
                        inicio: s,
                        fim: p,
                        de_linha: true,
                    });
                } else if c == b'/' && bytes.get(p + 1) == Some(&b'*') {
                    let s = p;
                    p += 2;
                    let mut nivel = 1usize;
                    while p < bytes.len() && nivel > 0 {
                        if bytes[p] == b'/' && bytes.get(p + 1) == Some(&b'*') {
                            nivel += 1;
                            p += 2;
                        } else if bytes[p] == b'*' && bytes.get(p + 1) == Some(&b'/') {
                            nivel -= 1;
                            p += 2;
                        } else {
                            p += 1;
                        }
                    }
                    if nivel > 0 {
                        return Err(ErroSintaxe {
                            posicao: s,
                            comprimento: 2,
                            mensagem: "Unterminated multi-line comment.".into(),
                        });
                    }
                    comentarios.push(Comentario {
                        inicio: s,
                        fim: p,
                        de_linha: false,
                    });
                } else {
                    p += 1;
                }
            }
            lista[i].comentarios = (c0, comentarios.len() as u32);
            if i > 0 {
                lista[i - 1].colado = lista[i - 1].fim == ini;
            }
            anterior_fim = lista[i].fim.max(anterior_fim);
        }
        Ok(Tokens { lista, comentarios })
    }

    /// Texto do token `t`.
    pub fn texto<'a>(&self, fonte: &'a str, t: Tk) -> &'a str {
        let k = &self.lista[t as usize];
        &fonte[k.inicio..k.fim]
    }

    /// Os comentários que precedem o token `t`.
    pub fn comentarios_de(&self, t: Tk) -> &[Comentario] {
        let (a, b) = self.lista[t as usize].comentarios;
        &self.comentarios[a as usize..b as usize]
    }

    /// Junta o `>` em `t` com os `>`/`=` colados que o seguem, até formar o
    /// operador `alvo` (`">>"`, `">="`, `">>>"`, `">>="` ou `">>>="`). Só é
    /// chamado pelo parser em contexto de expressão, logo depois de conferir
    /// com [`Tokens::maior_composto`]. Os tokens absorvidos saem da lista;
    /// nenhum nó aponta para eles ainda, porque o parser está exatamente em `t`.
    pub fn combinar_maior(&mut self, t: Tk, alvo: &str) {
        let i = t as usize;
        let n = alvo.len();
        let fim = self.lista[i].inicio + n;
        let colado_ultimo = self.lista[i + n - 1].colado;
        self.lista.drain(i + 1..i + n);
        self.lista[i].fim = fim;
        self.lista[i].tipo = TipoToken::MaiorComposto;
        self.lista[i].colado = colado_ultimo;
    }

    /// Se o `>` em `t` começa um operador composto por tokens colados,
    /// devolve o maior deles (`>>>=`, `>>>`, `>>=`, `>>`, `>=`).
    pub fn maior_composto(&self, t: Tk) -> Option<&'static str> {
        let i = t as usize;
        let e = |k: usize, op: Op| self.lista.get(i + k).is_some_and(|x| x.tipo == TipoToken::Op(op));
        let col = |k: usize| self.lista.get(i + k).is_some_and(|x| x.colado);
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
        if self.lista.get(i + 1).is_some_and(|x| x.tipo == TipoToken::Op(Op::EqEq)) {
            return None;
        }
        None
    }
}

fn erro_string(ini: usize, fim: usize) -> ErroSintaxe {
    ErroSintaxe {
        posicao: ini,
        comprimento: (fim - ini).max(1),
        mensagem: "Unterminated string literal.".into(),
    }
}
