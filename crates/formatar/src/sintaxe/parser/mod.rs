//! Parser de Dart 3.6 que monta a [`Arvore`] com as
//! mesmas formas do `parseString` do analyzer 3.6 (o parser do fasta mais o
//! `AstBuilder`, sem resolução nem `AstRewriter`).
//!
//! É um descendente recursivo que porta as decisões do fasta
//! (`_fe_analyzer_shared` 76, `parser_impl.dart` e `type_info*.dart`) ponto a
//! ponto, porque o formatador distingue os nós que o analyzer produz: o
//! mesmo texto `a.b()` vira `MethodInvocation`, `a.b()()` vira
//! `FunctionExpressionInvocation`, `A<T>.b()` vira
//! `InstanceCreationExpression` e assim por diante. Os módulos seguem a
//! divisão da gramática:
//!
//! * `olhar` — *lookahead* sem efeitos: `computeType`,
//!   `computeTypeParamOrArg`, `skipOuterPattern` e afins;
//! * `tipos` — anotações de tipo, parâmetros e argumentos de tipo,
//!   parâmetros formais e metadata;
//! * `expressoes` — expressões, literais, cascatas e coleções;
//! * `instrucoes` — comandos e declarações locais;
//! * `padroes` — padrões (Dart 3);
//! * `declaracoes` — unidade, diretivas e declarações de topo e de membro;
//! * [`verificar`] — conferências da árvore (cobertura de tokens, limites) e
//!   o despejo textual usado para comparar com o analyzer.
//!
//! O parser não altera a lista de tokens enquanto trabalha: os `>` que
//! formam `>>`, `>=`, `>>>`, `>>=` e `>>>=` em contexto de expressão, e os
//! `[` `]` (`=`) dos nomes de operador `[]`/`[]=`, ficam anotados em
//! `Parser::fusoes` e são fundidos só no fim, com o remapeamento dos
//! índices de token dos nós. Assim o *lookahead* e a especulação
//! (`canParseAsConditional`) trabalham sobre índices estáveis.
//!
//! Erros: qualquer erro que o analyzer relataria e que dá para detectar sem
//! resolução vira [`ErroSintaxe`]; o formatador recusa o arquivo, como o
//! `dart format`. Não há recuperação de erros.

mod declaracoes;
mod expressoes;
mod instrucoes;
mod olhar;
mod padroes;
mod tipos;
pub mod verificar;

use dartforge_frontend::token::{Keyword, Op};

use super::arvore::{Arvore, No, NoId};
use super::token::{ErroSintaxe, TipoToken, Tk, Token, Tokens};

/// Resultado das funções do parser.
type R<T> = Result<T, ErroSintaxe>;

/// Profundidade máxima de aninhamento de expressões e comandos, como os 500
/// do fasta (`expressionDepth`/`statementDepth`), que relata
/// `StackOverflow` além disso.
const PROFUNDIDADE_MAXIMA: u32 = 500;

/// Precedências do fasta (`token.dart`).
mod prec {
    pub const ATRIBUICAO: u8 = 1;
    pub const CASCATA: u8 = 2;
    pub const CONDICIONAL: u8 = 3;
    pub const SE_NULO: u8 = 4;
    pub const OU: u8 = 5;
    pub const E: u8 = 6;
    pub const IGUALDADE: u8 = 7;
    pub const RELACIONAL: u8 = 8;
    pub const BIT_OU: u8 = 9;
    pub const BIT_XOU: u8 = 10;
    pub const BIT_E: u8 = 11;
    pub const DESLOCAMENTO: u8 = 12;
    pub const ADITIVA: u8 = 13;
    pub const MULTIPLICATIVA: u8 = 14;
    pub const PREFIXO: u8 = 15;
    pub const POSFIXO: u8 = 16;
    pub const SELETOR: u8 = 17;
}

/// Estado `async` do corpo em análise (`AsyncModifier` do fasta).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Assinc {
    Sinc,
    Comum,
    SincEstrela,
    Estrela,
}

/// Onde estamos em relação a laços (`LoopState` do fasta), para `break` e
/// `continue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Laco {
    Fora,
    Switch,
    Repeticao,
}

/// Uma fusão de tokens adiada para o fim da análise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fusao {
    /// `>` composto ([`Tokens::combinar_maior`]).
    Maior(&'static str),
    /// `[` `]` ou `[` `]` `=` colados: o token `INDEX`/`INDEX_EQ` do scanner
    /// do analyzer, com `n` tokens nossos.
    Colchetes(usize),
}

/// Opções de análise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcoesAnalise {
    /// Versão da linguagem (maior, menor). Antes de 3.0 não há padrões: os
    /// `case` de `switch` são expressões ([`No::SwitchCase`]). Um comentário
    /// `// @dart=X.Y` antes da primeira declaração tem precedência, como no
    /// scanner do analyzer.
    pub versao: (u32, u32),
}

impl Default for OpcoesAnalise {
    fn default() -> Self {
        OpcoesAnalise { versao: (3, 6) }
    }
}

/// Lê `fonte` inteira como unidade de compilação, na versão 3.6 da linguagem.
///
/// ```
/// use dartforge_formatar::sintaxe::arvore::No;
/// use dartforge_formatar::sintaxe::parser::analisar;
/// let (_tokens, arvore) = analisar("main() => a.b();").unwrap();
/// let raiz = arvore.raiz.unwrap();
/// assert!(matches!(arvore.no(raiz), No::CompilationUnit { .. }));
/// ```
///
/// # Erros
/// O primeiro erro de sintaxe encontrado.
pub fn analisar(fonte: &str) -> Result<(Tokens, Arvore), ErroSintaxe> {
    analisar_com(fonte, OpcoesAnalise::default())
}

/// Como [`analisar`], com a versão da linguagem explícita.
///
/// ```
/// use dartforge_formatar::sintaxe::parser::{analisar_com, OpcoesAnalise};
/// let fonte = "f(x) { switch (x) { case 1: break; } }";
/// assert!(analisar_com(fonte, OpcoesAnalise { versao: (2, 19) }).is_ok());
/// ```
///
/// # Erros
/// O primeiro erro de sintaxe encontrado.
pub fn analisar_com(fonte: &str, opcoes: OpcoesAnalise) -> Result<(Tokens, Arvore), ErroSintaxe> {
    let tokens = Tokens::ler(fonte)?;
    let versao = versao_do_comentario(fonte, &tokens).unwrap_or(opcoes.versao);
    let mut p = Parser::novo(fonte, tokens)?;
    p.padroes = versao >= (3, 0);
    p.versao = versao;
    let raiz = p.unidade()?;
    if let Some(e) = p.erro_de_versao.take() {
        return Err(e);
    }
    p.arv.raiz = Some(raiz);
    let Parser {
        mut toks,
        mut arv,
        fusoes,
        ..
    } = p;
    aplicar_fusoes(&mut toks, &mut arv, &fusoes);
    arv.ligar_pais();
    Ok((toks, arv))
}

/// O `// @dart = X.Y` antes do primeiro token (`LanguageVersionToken` do
/// scanner).
fn versao_do_comentario(fonte: &str, tokens: &Tokens) -> Option<(u32, u32)> {
    let primeiro = tokens.lista.first()?;
    let alvo = if primeiro.tipo == TipoToken::ScriptTag {
        tokens.lista.get(1)?
    } else {
        primeiro
    };
    let (a, b) = alvo.comentarios;
    for c in &tokens.comentarios[a as usize..b as usize] {
        if !c.de_linha {
            continue;
        }
        let texto = &fonte[c.inicio + 2..c.fim];
        let t = texto.trim_start();
        let Some(resto) = t.strip_prefix("@dart") else {
            continue;
        };
        let resto = resto.trim_start();
        let Some(resto) = resto.strip_prefix('=') else {
            continue;
        };
        let resto = resto.trim();
        let mut partes = resto.splitn(2, '.');
        let maior = partes.next()?.parse().ok()?;
        let menor = partes.next()?.parse().ok()?;
        return Some((maior, menor));
    }
    None
}

/// Aplica as fusões adiadas e remapeia os índices de token da árvore.
fn aplicar_fusoes(toks: &mut Tokens, arv: &mut Arvore, fusoes: &[(usize, Fusao)]) {
    if fusoes.is_empty() {
        return;
    }
    let n = toks.lista.len();
    // `removidos[i]` = quantos tokens antes de `i` (exclusive) somem.
    // `dono[i]`: para um token absorvido, o primeiro token da fusão.
    let mut dono: Vec<Option<usize>> = vec![None; n];
    for &(i, f) in fusoes {
        let k = match f {
            Fusao::Maior(a) => a.len(),
            Fusao::Colchetes(k) => k,
        };
        for d in dono.iter_mut().skip(i + 1).take(k - 1) {
            *d = Some(i);
        }
    }
    let mut novo = vec![0u32; n];
    let mut removidos = 0u32;
    for i in 0..n {
        match dono[i] {
            // Um nó que termina num token absorvido termina no token fundido.
            Some(d) => {
                novo[i] = novo[d];
                removidos += 1;
            }
            None => novo[i] = i as u32 - removidos,
        }
    }
    let mut ordenadas: Vec<(usize, Fusao)> = fusoes.to_vec();
    ordenadas.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (i, f) in ordenadas {
        match f {
            Fusao::Maior(a) => toks.combinar_maior(i as Tk, a),
            Fusao::Colchetes(k) => {
                let fim = toks.lista[i + k - 1].fim;
                let colado = toks.lista[i + k - 1].colado;
                toks.lista.drain(i + 1..i + k);
                toks.lista[i].fim = fim;
                toks.lista[i].colado = colado;
            }
        }
    }
    let mapa = |t: &mut Tk| *t = novo[*t as usize];
    for d in &mut arv.nos {
        d.inicio = novo[d.inicio as usize];
        d.fim = novo[d.fim as usize];
        verificar::tokens_mut(&mut d.no, &mut |t| mapa(t));
    }
}

/// Classe de palavra do fasta para identificadores embutidos e
/// pseudo-palavras (`KeywordStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Estilo {
    Embutida,
    Pseudo,
    Nenhum,
}

fn estilo(texto: &str) -> Estilo {
    match texto {
        "abstract" | "as" | "covariant" | "deferred" | "dynamic" | "export" | "extension"
        | "external" | "factory" | "Function" | "get" | "implements" | "import" | "interface"
        | "late" | "library" | "mixin" | "operator" | "part" | "required" | "set" | "static"
        | "typedef" => Estilo::Embutida,
        "async" | "await" | "base" | "hide" | "inout" | "native" | "of" | "on" | "out"
        | "patch" | "sealed" | "show" | "source" | "sync" | "when" | "yield" => Estilo::Pseudo,
        _ => Estilo::Nenhum,
    }
}

/// O cursor e o estado do parser.
pub(crate) struct Parser<'f> {
    fonte: &'f str,
    toks: Tokens,
    /// Para cada `(`, `[`, `{` e `${`, o índice do fecho correspondente.
    pares: Vec<u32>,
    pos: usize,
    arv: Arvore,
    fusoes: Vec<(usize, Fusao)>,
    assinc: Assinc,
    laco: Laco,
    /// `mayParseFunctionExpressions` do fasta.
    funcoes: bool,
    /// `allowPatterns` do fasta (versão >= 3.0).
    padroes: bool,
    /// Versão da linguagem em vigor.
    versao: (u32, u32),
    /// Primeiro uso de recurso acima da versão em vigor.
    erro_de_versao: Option<ErroSintaxe>,
    profundidade: u32,
    /// `isLastPatternAllowedInsideUnaryPattern` do fasta.
    padrao_unario_ok: bool,
}

impl<'f> Parser<'f> {
    fn novo(fonte: &'f str, toks: Tokens) -> R<Parser<'f>> {
        let n = toks.lista.len();
        let mut pares = vec![u32::MAX; n];
        let mut pilha: Vec<usize> = Vec::new();
        for (i, t) in toks.lista.iter().enumerate() {
            match t.tipo {
                TipoToken::Op(Op::LParen | Op::LBracket | Op::LBrace)
                | TipoToken::InterpolacaoExpressao => pilha.push(i),
                TipoToken::Op(o @ (Op::RParen | Op::RBracket | Op::RBrace)) => {
                    let Some(a) = pilha.pop() else {
                        return Err(erro_token(fonte, t, &format!("Unexpected '{}'.", o.text())));
                    };
                    let esperado = match toks.lista[a].tipo {
                        TipoToken::Op(Op::LParen) => Op::RParen,
                        TipoToken::Op(Op::LBracket) => Op::RBracket,
                        _ => Op::RBrace,
                    };
                    if esperado != o {
                        return Err(erro_token(
                            fonte,
                            &toks.lista[a],
                            &format!("Expected '{}' to match.", esperado.text()),
                        ));
                    }
                    pares[a] = i as u32;
                    pares[i] = a as u32;
                }
                _ => {}
            }
        }
        if let Some(&a) = pilha.last() {
            return Err(erro_token(fonte, &toks.lista[a], "Unbalanced bracket."));
        }
        conferir_lacunas(fonte, &toks)?;
        Ok(Parser {
            fonte,
            toks,
            pares,
            pos: 0,
            arv: Arvore::default(),
            fusoes: Vec::new(),
            assinc: Assinc::Sinc,
            laco: Laco::Fora,
            funcoes: true,
            padroes: true,
            versao: (3, 6),
            erro_de_versao: None,
            profundidade: 0,
            padrao_unario_ok: false,
        })
    }

    // ------------------------------------------------------------ tokens

    fn tk(&self, i: usize) -> &Token {
        let n = self.toks.lista.len();
        &self.toks.lista[i.min(n - 1)]
    }

    fn tipo(&self, i: usize) -> TipoToken {
        self.tk(i).tipo
    }

    fn txt(&self, i: usize) -> &'f str {
        let t = self.tk(i);
        &self.fonte[t.inicio..t.fim]
    }

    fn op(&self, i: usize, o: Op) -> bool {
        self.tipo(i) == TipoToken::Op(o)
    }

    fn kw(&self, i: usize, k: Keyword) -> bool {
        self.tipo(i) == TipoToken::Reservada(k)
    }

    /// `isIdentifier` do fasta: identificador comum, embutido ou pseudo.
    fn ident(&self, i: usize) -> bool {
        self.tipo(i) == TipoToken::Identificador
    }

    /// `kind == IDENTIFIER_TOKEN`: identificador que não é embutido nem
    /// pseudo-palavra.
    fn ident_puro(&self, i: usize) -> bool {
        self.ident(i) && estilo(self.txt(i)) == Estilo::Nenhum
    }

    fn embutida(&self, i: usize) -> bool {
        self.ident(i) && estilo(self.txt(i)) == Estilo::Embutida
    }

    fn pseudo(&self, i: usize) -> bool {
        self.ident(i) && estilo(self.txt(i)) == Estilo::Pseudo
    }

    /// Identificador com o texto `s` (palavras contextuais).
    fn pal(&self, i: usize, s: &str) -> bool {
        self.ident(i) && self.txt(i) == s
    }

    /// `isKeywordOrIdentifier`.
    fn palavra(&self, i: usize) -> bool {
        matches!(
            self.tipo(i),
            TipoToken::Identificador | TipoToken::Reservada(_)
        )
    }

    fn eof(&self, i: usize) -> bool {
        self.tipo(i) == TipoToken::Eof
    }

    fn texto(&self, i: usize) -> bool {
        self.tipo(i) == TipoToken::Texto
    }

    /// Fecho do grupo aberto em `i`.
    fn par(&self, i: usize) -> usize {
        let p = self.pares.get(i).copied().unwrap_or(u32::MAX);
        if p == u32::MAX {
            self.toks.lista.len() - 1
        } else {
            p as usize
        }
    }

    /// `isModifier` do fasta (`modifier_context.dart`): os embutidos só são
    /// modificadores antes de outra palavra (ou de um tipo record seguido de
    /// nome).
    fn modificador(&self, i: usize) -> bool {
        let t = self.txt(i);
        match self.tipo(i) {
            TipoToken::Reservada(Keyword::Const | Keyword::Final | Keyword::Var) => true,
            TipoToken::Identificador
                if matches!(
                    t,
                    "abstract" | "covariant" | "external" | "late" | "required" | "static"
                ) =>
            {
                let n = i + 1;
                if !self.palavra(n) || self.kw(n, Keyword::In) {
                    if self.op(n, Op::LParen) {
                        let depois = self.par(n) + 1;
                        let this_super = |p: &Self, k: usize| {
                            (p.kw(k, Keyword::This) || p.kw(k, Keyword::Super))
                                && p.op(k + 1, Op::Dot)
                        };
                        if self.ident(depois) || this_super(self, depois) {
                            return true;
                        }
                        if self.op(depois, Op::Question)
                            && (self.ident(depois + 1) || this_super(self, depois + 1))
                        {
                            return true;
                        }
                    }
                    return false;
                }
                true
            }
            _ => false,
        }
    }

    /// Operador definível pelo usuário começando em `i` e quantos tokens
    /// nossos ele ocupa (`isUserDefinableOperator`).
    fn operador_definivel(&self, i: usize) -> Option<usize> {
        match self.tipo(i) {
            TipoToken::Op(
                Op::EqEq
                | Op::Lt
                | Op::LtEq
                | Op::Minus
                | Op::Plus
                | Op::Slash
                | Op::TildeSlash
                | Op::Star
                | Op::Percent
                | Op::Pipe
                | Op::Caret
                | Op::Amp
                | Op::LtLt
                | Op::Tilde,
            ) => Some(1),
            TipoToken::Op(Op::Gt) => match self.toks.maior_composto(i as Tk) {
                None => Some(1),
                Some(">>") => Some(2),
                Some(">=") => Some(2),
                Some(">>>") => Some(3),
                Some(_) => None,
            },
            TipoToken::Op(Op::LBracket) if self.op(i + 1, Op::RBracket) && self.tk(i).colado => {
                if self.op(i + 2, Op::Assign) && self.tk(i + 1).colado {
                    Some(3)
                } else {
                    Some(2)
                }
            }
            _ => None,
        }
    }

    // ------------------------------------------------------------ erros

    fn erro_em(&self, i: usize, msg: &str) -> ErroSintaxe {
        erro_token(self.fonte, self.tk(i), msg)
    }

    fn erro(&self, msg: &str) -> ErroSintaxe {
        self.erro_em(self.pos, msg)
    }

    fn esperado(&self, o: &str) -> ErroSintaxe {
        let achado = self.txt(self.pos);
        self.erro(&format!("Expected '{o}' before '{achado}'."))
    }

    // ------------------------------------------------------------ cursor

    fn avancar(&mut self) -> usize {
        let i = self.pos;
        self.pos += 1;
        i
    }

    fn espera_op(&mut self, o: Op) -> R<usize> {
        if self.op(self.pos, o) {
            Ok(self.avancar())
        } else {
            Err(self.esperado(o.text()))
        }
    }

    fn espera_kw(&mut self, k: Keyword) -> R<usize> {
        if self.kw(self.pos, k) {
            Ok(self.avancar())
        } else {
            Err(self.esperado(k.text()))
        }
    }

    fn espera_pal(&mut self, s: &str) -> R<usize> {
        if self.pal(self.pos, s) {
            Ok(self.avancar())
        } else {
            Err(self.esperado(s))
        }
    }

    fn opcional_op(&mut self, o: Op) -> Option<usize> {
        if self.op(self.pos, o) {
            Some(self.avancar())
        } else {
            None
        }
    }

    /// `ensureIdentifier` para nomes: um identificador (qualquer estilo).
    fn espera_ident(&mut self) -> R<usize> {
        if self.ident(self.pos) {
            Ok(self.avancar())
        } else {
            Err(self.erro(&format!(
                "Expected an identifier, but got '{}'.",
                self.txt(self.pos)
            )))
        }
    }

    /// Identificador ou `new` (nas continuações em que o fasta reescreve
    /// `new` como identificador: `A.new`).
    fn espera_ident_ou_new(&mut self) -> R<usize> {
        if self.kw(self.pos, Keyword::New) {
            return Ok(self.avancar());
        }
        self.espera_ident()
    }

    /// Nome declarado (variável, parâmetro, função local): além de
    /// identificador, `await`/`yield` não valem fora de corpo síncrono comum
    /// (`checkAsyncAwaitYieldAsIdentifier`).
    fn espera_nome_declarado(&mut self) -> R<usize> {
        let i = self.espera_ident()?;
        self.conferir_await_yield(i)?;
        Ok(i)
    }

    fn conferir_await_yield(&self, i: usize) -> R<()> {
        if self.assinc != Assinc::Sinc && matches!(self.txt(i), "await" | "yield") {
            return Err(self.erro_em(i, "'await' and 'yield' can't be used as identifiers here."));
        }
        Ok(())
    }

    /// Anota a fusão do `>` em `i` com os seguintes, formando `alvo`.
    fn fundir_maior(&mut self, i: usize, alvo: &'static str) {
        if alvo.starts_with(">>>") {
            self.recurso(i, (2, 14), "triple-shift");
        }
        self.fusoes.push((i, Fusao::Maior(alvo)));
    }

    /// Consome um operador definível pelo usuário (nome de `operator` ou de
    /// símbolo `#`), anotando as fusões necessárias; devolve o token.
    fn consumir_operador_definivel(&mut self) -> R<usize> {
        let i = self.pos;
        let Some(n) = self.operador_definivel(i) else {
            return Err(self.erro("Expected an operator."));
        };
        if self.op(i, Op::Gt) && n > 1 {
            let alvo = self.toks.maior_composto(i as Tk).unwrap_or(">");
            self.fundir_maior(i, alvo);
        } else if self.op(i, Op::LBracket) {
            self.fusoes.push((i, Fusao::Colchetes(n)));
        }
        self.pos += n;
        Ok(i)
    }

    /// Recurso da linguagem que exige a versão `minima`: antes dela o
    /// analyzer relata `EXPERIMENT_NOT_ENABLED`, que é erro sintático. O
    /// primeiro erro fica guardado e é devolvido no fim da análise.
    fn recurso(&mut self, i: usize, minima: (u32, u32), nome: &str) {
        if self.versao < minima && self.erro_de_versao.is_none() {
            self.erro_de_versao = Some(self.erro_em(
                i,
                &format!("This requires the '{nome}' language feature to be enabled."),
            ));
        }
    }

    // ------------------------------------------------------------ nós

    /// Cria o nó `no` de `ini` até o último token consumido.
    fn no(&mut self, no: No, ini: usize) -> NoId {
        let fim = self.pos.saturating_sub(1).max(ini);
        self.arv.novo(no, ini as Tk, fim as Tk)
    }

    fn no_entre(&mut self, no: No, ini: usize, fim: usize) -> NoId {
        self.arv.novo(no, ini as Tk, fim as Tk)
    }

    fn ini(&self, id: NoId) -> usize {
        self.arv.inicio(id) as usize
    }

    fn fim(&self, id: NoId) -> usize {
        self.arv.fim(id) as usize
    }

    fn simples(&mut self, i: usize) -> NoId {
        self.no_entre(No::SimpleIdentifier { token: i as Tk }, i, i)
    }

    fn e_simples(&self, id: NoId) -> bool {
        matches!(self.arv.no(id), No::SimpleIdentifier { .. })
    }

    /// Roda `f` como sonda: devolve a posição alcançada se `f` teve sucesso
    /// e desfaz todo efeito (nós, fusões, cursor e estado).
    fn sondar<T>(&mut self, f: impl FnOnce(&mut Self) -> R<T>) -> Option<usize> {
        let pos = self.pos;
        let nos = self.arv.nos.len();
        let fusoes = self.fusoes.len();
        let (assinc, laco, funcoes, prof, unario) = (
            self.assinc,
            self.laco,
            self.funcoes,
            self.profundidade,
            self.padrao_unario_ok,
        );
        let r = f(self);
        let alcancado = self.pos;
        self.pos = pos;
        self.arv.nos.truncate(nos);
        self.fusoes.truncate(fusoes);
        self.assinc = assinc;
        self.laco = laco;
        self.funcoes = funcoes;
        self.profundidade = prof;
        self.padrao_unario_ok = unario;
        r.ok().map(|_| alcancado)
    }

    fn entrar(&mut self) -> R<()> {
        self.profundidade += 1;
        if self.profundidade > PROFUNDIDADE_MAXIMA {
            return Err(self.erro("The file has too many nested expressions or statements."));
        }
        Ok(())
    }

    fn sair(&mut self) {
        self.profundidade -= 1;
    }
}

/// Entre dois tokens só pode haver brancos e comentários; o scanner do
/// analyzer relata qualquer outro caractere (`Illegal character`), como o
/// espaço não separável.
fn conferir_lacunas(fonte: &str, toks: &Tokens) -> R<()> {
    let b = fonte.as_bytes();
    let mut ant = 0usize;
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        ant = 3;
    }
    for t in &toks.lista {
        let (c0, c1) = t.comentarios;
        let mut p = ant;
        let mut cs = toks.comentarios[c0 as usize..c1 as usize].iter().peekable();
        while p < t.inicio {
            if let Some(c) = cs.peek()
                && c.inicio == p
            {
                p = c.fim;
                cs.next();
                continue;
            }
            if !matches!(b[p], b' ' | b'\t' | b'\n' | b'\r' | 0x0C) {
                let ch = fonte[p..].chars().next().unwrap_or(' ');
                return Err(ErroSintaxe {
                    posicao: p,
                    comprimento: ch.len_utf8(),
                    mensagem: format!("Illegal character '{}'.", ch as u32),
                });
            }
            p += 1;
        }
        if t.tipo == TipoToken::ScriptTag {
            ant = t.fim;
        } else {
            ant = ant.max(t.fim);
        }
    }
    Ok(())
}

fn erro_token(_fonte: &str, t: &Token, msg: &str) -> ErroSintaxe {
    ErroSintaxe {
        posicao: t.inicio,
        comprimento: (t.fim - t.inicio).max(1),
        mensagem: msg.to_string(),
    }
}
