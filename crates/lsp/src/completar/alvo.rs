//! O `CompletionTarget.forOffset` do analysis server 3.6.2
//! (`AS:src/provisional/completion/dart/completion_target.dart:141-260`), o
//! intervalo de substituição (`computeReplacementRange`, `:470-526`) e os
//! dois prefixos: o do casador (`TokenData.fromSelection`,
//! `completion_manager.dart:484-539`) e o do pedido (`targetPrefix`,
//! `:388-445`); docs/LSP-ESPECIFICACAO.md §14.13.1-3.
//!
//! As entidades de um nó são os filhos dele e os tokens do texto dentro do
//! nó que nenhum filho cobre. A árvore do DartForge não tem tokens
//! sintéticos (o parser recupera sem criá-los): um token que o fasta
//! criaria sintético simplesmente não está; as regras de token sintético
//! (comprimento 0) valem para os nós vazios da árvore, que ficam no
//! próximo token real como o fasta os poria.
//! Escrito sem compilar nem executar (2026-10-05).

use super::isp::{Inicial, Isp};
use dartforge_frontend::token::{Kind, Op};

/// Uma entidade filha (`childEntities`): um nó da árvore ou um token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Entidade {
    No(usize),
    Tok(usize),
    /// Um comentário (o `CommentToken` do Dart): o intervalo.
    Comentario(usize, usize),
}

/// O alvo do pedido.
#[derive(Debug, Clone, Copy)]
pub(super) struct Alvo {
    pub no: usize,
    pub entidade: Option<Entidade>,
    /// `isCommentText`: o cursor está no texto de um comentário comum.
    pub texto_de_comentario: bool,
}

impl<'a, 'c> Isp<'a, 'c> {
    /// As entidades de um nó, na ordem do fonte.
    pub(super) fn entidades(&self, n: usize) -> Vec<Entidade> {
        let filhos: Vec<usize> = self.filhos(n).to_vec();
        let (ini, fim) = (self.ini(n), self.fim(n));
        let mut v: Vec<(usize, Entidade)> = filhos.iter().map(|&f| (self.ini(f), Entidade::No(f))).collect();
        let mut i = self.tok_desde(ini);
        while let Some(t) = self.toks.get(i) {
            if t.span.start >= fim || t.kind == Kind::Eof {
                break;
            }
            let dentro_de_filho = filhos.iter().any(|&f| self.ini(f) <= t.span.start && t.span.end <= self.fim(f) && self.ini(f) < self.fim(f));
            if !dentro_de_filho {
                v.push((t.span.start, Entidade::Tok(i)));
            }
            i += 1;
        }
        v.sort_by_key(|(p, e)| (*p, matches!(e, Entidade::No(_))));
        v.into_iter().map(|(_, e)| e).collect()
    }

    /// O último token de um nó (`endToken`); `None` num nó vazio.
    pub(super) fn ultimo_token(&self, n: usize) -> Option<usize> {
        self.tok_antes(self.fim(n)).filter(|&i| self.span_tok(i).start >= self.ini(n) && self.ini(n) < self.fim(n))
    }

    /// `_isCandidateToken` para um token real.
    fn candidato_tok(&self, i: usize) -> bool {
        let s = self.span_tok(i);
        if self.offset < s.end {
            return true;
        }
        if self.offset == s.end {
            return self.palavra(i);
        }
        false
    }

    /// `_isCandidateToken(node, entity.endToken)` de um nó: o vazio é um
    /// token sintético de comprimento 0 no próximo token real.
    fn candidato_fim(&self, n: usize) -> bool {
        match self.ultimo_token(n) {
            // O nó acaba num token sintético (o `)`/`;` que o fasta insere,
            // de comprimento 0 no fim do nó), depois do último real.
            Some(i) if self.span_tok(i).end < self.fim(n) => self.offset < self.fim(n),
            Some(i) => self.candidato_tok(i),
            None => self.offset <= self.sintetico(self.ini(n)).start,
        }
    }

    /// `_isCandidateNode`.
    fn candidato_no(&self, n: usize) -> bool {
        match self.inicial(n) {
            Inicial::Tok(i) if self.span_tok(i).start < self.fim(n).max(self.ini(n) + 1) && self.palavra(i) => self.candidato_tok(i),
            _ if self.ini(n) >= self.fim(n) => {
                // Vazio: o identificador sintético (palavra) ou outro token
                // sintético, ambos em `offset <= posição`.
                self.offset <= self.sintetico(self.ini(n)).start
            }
            _ => self.offset <= self.ini(n),
        }
    }

    /// `_getContainingCommentToken`: entre os comentários que precedem o
    /// token que começa em `antes_de` (o EOF com `eof`), o que contém o
    /// cursor (o de linha com o fim inclusivo, o de bloco sem).
    fn comentario_que_contem(&self, antes_de: usize, eof: bool) -> Option<(usize, usize)> {
        if !eof && self.offset >= antes_de {
            return None;
        }
        let de = self.tok_antes(antes_de).map_or(0, |i| self.span_tok(i).end);
        for c in dartforge_frontend::comentarios::Comentarios::de(self.fonte).todos() {
            if c.start < de || c.end > antes_de {
                continue;
            }
            if self.offset <= c.start {
                return None;
            }
            if self.offset <= c.end {
                let linha = self.fonte[c.start..].starts_with("//");
                if linha || self.offset < c.end {
                    return Some((c.start, c.end));
                }
            }
        }
        None
    }

    /// `_getContainingDocComment`: o nó tem o comentário de documentação
    /// que é o `c`.
    fn comentario_de_documentacao(&self, n: usize, c: (usize, usize)) -> Option<usize> {
        self.filhos_de(n, &["Comment"]).into_iter().find(|&k| self.ini(k) <= c.0 && c.1 <= self.fim(k))
    }

    /// `CompletionTarget.forOffset(unit, offset)`.
    pub(super) fn alvo(&self) -> Alvo {
        let mut no = 0usize;
        'externo: loop {
            if self.especie(no) == "Comment" {
                for r in self.filhos_de(no, &["CommentReference"]) {
                    if self.ini(r) <= self.offset && self.offset <= self.fim(r) {
                        no = r;
                        continue 'externo;
                    }
                }
            }
            for e in self.entidades(no) {
                match e {
                    Entidade::Tok(i) => {
                        if !self.candidato_tok(i) {
                            continue;
                        }
                        if let Some(c) = self.comentario_que_contem(self.span_tok(i).start, false) {
                            return match self.comentario_de_documentacao(no, c) {
                                Some(d) => Alvo { no: d, entidade: Some(Entidade::Comentario(c.0, c.1)), texto_de_comentario: false },
                                None => Alvo { no: 0, entidade: Some(Entidade::Comentario(c.0, c.1)), texto_de_comentario: true },
                            };
                        }
                        return Alvo { no, entidade: Some(e), texto_de_comentario: false };
                    }
                    Entidade::No(f) => {
                        if !self.candidato_fim(f) {
                            continue;
                        }
                        if self.candidato_no(f) {
                            let inicio = match self.inicial(f) {
                                Inicial::Tok(i) => self.span_tok(i).start,
                                _ => self.ini(f),
                            };
                            if let Some(c) = self.comentario_que_contem(inicio, false) {
                                return match self.comentario_de_documentacao(no, c) {
                                    Some(d) => Alvo { no: d, entidade: Some(Entidade::Comentario(c.0, c.1)), texto_de_comentario: false },
                                    None => Alvo { no: 0, entidade: Some(Entidade::Comentario(c.0, c.1)), texto_de_comentario: true },
                                };
                            }
                            return Alvo { no, entidade: Some(e), texto_de_comentario: false };
                        }
                        no = f;
                        continue 'externo;
                    }
                    Entidade::Comentario(..) => {}
                }
            }
            break;
        }
        if no == 0
            && let Some(c) = self.comentario_que_contem(self.fonte.len(), true)
        {
            return Alvo { no: 0, entidade: Some(Entidade::Comentario(c.0, c.1)), texto_de_comentario: true };
        }
        Alvo { no, entidade: None, texto_de_comentario: false }
    }

    /// O token de início de uma entidade.
    fn token_da_entidade(&self, e: Entidade) -> Option<usize> {
        match e {
            Entidade::Tok(i) => Some(i),
            Entidade::No(n) => match self.inicial(n) {
                Inicial::Tok(i) if self.span_tok(i).start < self.fim(n).max(self.ini(n) + 1) => Some(i),
                _ => None,
            },
            Entidade::Comentario(..) => None,
        }
    }

    /// `containingNode.findPrevious(token)`: o token anterior, se ainda é
    /// do nó.
    fn anterior_no_no(&self, no: usize, i: usize) -> Option<usize> {
        let j = i.checked_sub(1)?;
        (self.span_tok(j).start >= self.ini(no)).then_some(j)
    }

    /// `_computeDroppedToken`: um token de palavra fora da árvore entre o
    /// filho anterior à entidade e a entidade, que contém o cursor.
    pub(super) fn token_largado(&self, alvo: &Alvo) -> Option<usize> {
        let e = alvo.entidade?;
        let inicio_e = match e {
            Entidade::No(n) => self.ini(n),
            Entidade::Tok(i) => self.span_tok(i).start,
            Entidade::Comentario(a, _) => a,
        };
        let anteriores: Vec<usize> = self.filhos(alvo.no).iter().copied().filter(|&f| self.fim(f) <= inicio_e && self.especie(f) != "Comment").collect();
        let ultimo = *anteriores.last()?;
        let mut i = self.ultimo_token(ultimo)? + 1;
        while let Some(t) = self.toks.get(i) {
            if t.span.start >= inicio_e || t.kind == Kind::Eof {
                break;
            }
            let dentro = self.filhos(alvo.no).iter().any(|&f| self.ini(f) <= t.span.start && t.span.end <= self.fim(f));
            if !dentro && self.palavra(i) && t.span.start <= self.offset && self.offset <= t.span.end {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// `computeReplacementRange`: `(início, comprimento)`.
    pub(super) fn intervalo_de_substituicao(&self, alvo: &Alvo) -> (usize, usize) {
        let mut token = self.token_largado(alvo).or_else(|| alvo.entidade.and_then(|e| self.token_da_entidade(e)));
        if let Some(t) = token
            && self.offset < self.span_tok(t).start
        {
            token = self.anterior_no_no(alvo.no, t);
        }
        if let Some(t) = token {
            let mut t = Some(t);
            if let Some(x) = t
                && self.offset == self.span_tok(x).start
                && !self.palavra(x)
            {
                t = self.anterior_no_no(alvo.no, x);
            }
            if let Some(x) = t {
                let s = self.span_tok(x);
                if self.palavra(x) && s.start <= self.offset && self.offset <= s.end {
                    return (s.start, s.end - s.start);
                }
                if matches!(self.toks[x].kind, Kind::Str(_)) {
                    // A URI de uma diretiva (ou da configuração dela).
                    let uri = match self.especie(alvo.no) {
                        "ImportDirective" | "ExportDirective" => self
                            .filhos(alvo.no)
                            .iter()
                            .copied()
                            .find(|&k| self.especie(k) == "SimpleStringLiteral" && self.ini(k) <= self.offset && self.offset <= self.fim(k))
                            .or_else(|| {
                                self.filhos_de(alvo.no, &["Configuration"])
                                    .into_iter()
                                    .flat_map(|c| self.filhos_de(c, &["SimpleStringLiteral"]))
                                    .find(|&k| self.ini(k) <= self.offset && self.offset <= self.fim(k))
                            }),
                        "SimpleStringLiteral" => {
                            let pai = self.pai(alvo.no);
                            let diretiva = pai.filter(|&p| self.e_diretiva(p) || (self.especie(p) == "Configuration" && self.pai(p).is_some_and(|d| self.e_diretiva(d))));
                            diretiva.map(|_| alvo.no)
                        }
                        _ => None,
                    };
                    if let Some(u) = uri {
                        let (ci, cf) = (self.inicio_do_conteudo(u), self.fim_do_conteudo(u));
                        if ci <= self.offset && self.offset <= cf {
                            return (ci, cf - ci);
                        }
                    }
                }
            }
        }
        (self.offset, 0)
    }

    /// `contentsEnd`: antes da aspa de fechamento (se ela está).
    pub(super) fn fim_do_conteudo(&self, n: usize) -> usize {
        let ini_c = self.inicio_do_conteudo(n);
        let t = &self.fonte[self.ini(n)..self.fim(n)];
        let abre = &self.fonte[self.ini(n)..ini_c];
        let aspa = abre.trim_start_matches('r');
        if t.len() > abre.len() && t.ends_with(aspa) && !aspa.is_empty() {
            self.fim(n) - aspa.len()
        } else {
            self.fim(n)
        }
    }

    /// `TokenData.fromSelection`: o prefixo do casador.
    pub(super) fn prefixo_do_casador(&self) -> String {
        let Some(c) = self.no_coberto() else { return String::new() };
        let Some(mut t) = self.tok_antes(self.fim(c)).or_else(|| self.tok_depois(self.fim(c))) else { return String::new() };
        loop {
            let s = self.span_tok(t);
            if s.start > self.offset || (s.start == self.offset && !self.palavra(t)) {
                match t.checked_sub(1) {
                    Some(j) => t = j,
                    None => return String::new(),
                }
                continue;
            }
            break;
        }
        let s = self.span_tok(t);
        if self.toks[t].kind == Kind::Eof {
            return String::new();
        }
        if self.offset > s.end {
            return String::new();
        }
        if self.palavra(t) {
            return self.fonte[s.start..self.offset].to_string();
        }
        if let Kind::Str(_) | Kind::StrBegin(..) = self.toks[t].kind {
            let texto = &self.fonte[s.start..s.end];
            let pular = if texto.starts_with("r'''") || texto.starts_with("r\"\"\"") {
                4
            } else if texto.starts_with("r'") || texto.starts_with("r\"") {
                2
            } else if texto.starts_with("'''") || texto.starts_with("\"\"\"") {
                3
            } else {
                1
            };
            let ini = s.start + pular;
            if self.offset < ini {
                return String::new();
            }
            return self.fonte[ini..self.offset].to_string();
        }
        String::new()
    }

    /// `targetPrefix`: o prefixo do pedido (o filtro do handler).
    pub(super) fn prefixo_do_pedido(&self, alvo: &Alvo) -> String {
        let Some(e) = alvo.entidade else { return String::new() };
        let do_token = |i: usize| -> String {
            let s = self.span_tok(i);
            if s.start <= self.offset && self.offset < s.end {
                self.fonte[s.start..self.offset].to_string()
            } else if self.offset == s.end {
                self.fonte[s.start..s.end].to_string()
            } else {
                String::new()
            }
        };
        if let Entidade::Tok(i) = e {
            if let Some(j) = i.checked_sub(1)
                && self.span_tok(j).end == self.offset
                && self.palavra(j)
            {
                return self.texto_tok(j).to_string();
            }
            if let Kind::Str(_) = self.toks[i].kind {
                let s = self.span_tok(i);
                let uri = (alvo.no != 0 && self.especie(alvo.no) == "SimpleStringLiteral" && self.pai(alvo.no).is_some_and(|p| self.e_diretiva(p))).then_some(alvo.no);
                if s.start < self.offset
                    && self.offset < s.end
                    && let Some(u) = uri
                {
                    let ci = self.inicio_do_conteudo(u);
                    if self.offset >= ci {
                        return self.fonte[ci..self.offset].to_string();
                    }
                }
            }
            if self.palavra(i) && self.span_tok(i).end == self.offset {
                return self.texto_tok(i).to_string();
            }
            return String::new();
        }
        let Entidade::No(mut n) = e else { return String::new() };
        if self.especie(n) == "DeclaredVariablePattern"
            && let Some(nome) = self.nome_do_padrao_de_variavel(n)
            && self.span_tok(nome).start <= self.offset
        {
            return do_token(nome);
        }
        loop {
            if self.especie(n) == "SimpleIdentifier" {
                return match self.inicial(n) {
                    Inicial::Tok(i) => do_token(i),
                    _ => String::new(),
                };
            }
            // O primeiro `childEntity`: um nó desce; um token encerra.
            match self.entidades(n).first().copied() {
                Some(Entidade::No(f)) => n = f,
                Some(Entidade::Tok(i)) => return do_token(i),
                _ => return String::new(),
            }
        }
    }

    /// `inConstantContext` do pedido: a entidade é expressão em contexto
    /// constante.
    pub(super) fn pedido_em_contexto_constante(&self, alvo: &Alvo) -> bool {
        match alvo.entidade {
            Some(Entidade::No(n)) => super::isp::ESPECIES_DE_EXPRESSAO.contains(&self.especie(n)) && self.contexto_constante(n),
            _ => false,
        }
    }

    /// O nó `Op` de uma entidade token (para os testes de vírgula e `)`).
    pub(super) fn entidade_e_op(&self, e: Option<Entidade>, op: Op) -> bool {
        matches!(e, Some(Entidade::Tok(i)) if self.e_op(i, op))
    }
}
