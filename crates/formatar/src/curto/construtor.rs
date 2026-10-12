//! Porte de `short/chunk_builder.dart` e `short/nesting_builder.dart`: recebe
//! a saída incremental do visitante (texto, espaços, quebras, comentários) e
//! monta a árvore de chunks.
//!
//! O original troca o `builder` do visitante pelo de um bloco filho em
//! `startBlock` e volta ao pai em `endBlock`. Aqui os construtores vivem numa
//! pilha no visitante; [`Construtor::bloco`] lembra o chunk de bloco cujos
//! filhos este construtor escreve, e `endBlock` move a lista para lá.

use super::arena::{Arenas, ChunkId, NinhoId, RegraId, custo, recuo};

/// Tipo de comentário (`CommentType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoComentario {
    Doc,
    Linha,
    Bloco,
    BlocoEmLinha,
}

/// Comentário pronto para escrita (`SourceComment`).
#[derive(Debug, Clone)]
pub struct ComentarioFonte {
    pub texto: String,
    pub tipo: TipoComentario,
    pub linhas_antes: i32,
    pub rente: bool,
}

/// `NestingBuilder`.
#[derive(Debug, Clone)]
struct Aninhamento {
    pilha: Vec<i32>,
    pendente: Option<NinhoId>,
    atual: NinhoId,
}

impl Aninhamento {
    fn recuo(&self) -> i32 {
        *self.pilha.last().expect("pilha de recuo")
    }

    fn corrente(&self) -> NinhoId {
        self.pendente.unwrap_or(self.atual)
    }

    fn aninhar(&mut self, a: &mut Arenas, recuo: i32) {
        let base = self.pendente.unwrap_or(self.atual);
        self.pendente = Some(a.aninhar(base, recuo));
    }

    fn desaninhar(&mut self, a: &Arenas) {
        let base = self.pendente.unwrap_or(self.atual);
        self.pendente = a.ninhos[base as usize].pai;
        debug_assert!(self.pendente.is_some(), "unnest sem nest");
    }

    fn confirmar(&mut self) {
        if let Some(p) = self.pendente.take() {
            self.atual = p;
        }
    }
}

/// `ChunkBuilder`.
#[derive(Debug)]
pub struct Construtor {
    /// O chunk de bloco cujos filhos este construtor escreve; `None` no
    /// construtor de topo.
    pub bloco: Option<ChunkId>,
    pub chunks: Vec<ChunkId>,
    novas_linhas_pendentes: u8,
    espaco_pendente: bool,
    aninhado_pendente: bool,
    rente_pendente: bool,
    apos_comentario: bool,
    regras: Vec<RegraId>,
    regras_duras: Vec<RegraId>,
    regras_preguicosas: Vec<RegraId>,
    spans_abertos: Vec<(usize, i32)>,
    aninhamento: Aninhamento,
    aninhamento_de_argumento: Vec<NinhoId>,
    impedir_quebra: i32,
}

fn termina_com_ident(texto: &str) -> bool {
    texto.as_bytes().last().is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

fn comentario_de_metodo_generico(c: &ComentarioFonte) -> bool {
    c.texto.starts_with("/*<") || c.texto.starts_with("/*=")
}

impl Construtor {
    fn base(a: &mut Arenas, bloco: Option<ChunkId>) -> Construtor {
        let raiz = a.novo_ninho_raiz();
        let mut c = Construtor {
            bloco,
            chunks: Vec::new(),
            novas_linhas_pendentes: 0,
            espaco_pendente: false,
            aninhado_pendente: false,
            rente_pendente: false,
            apos_comentario: false,
            regras: Vec::new(),
            regras_duras: Vec::new(),
            regras_preguicosas: Vec::new(),
            spans_abertos: Vec::new(),
            aninhamento: Aninhamento {
                pilha: vec![0],
                pendente: None,
                atual: raiz,
            },
            aninhamento_de_argumento: Vec::new(),
            impedir_quebra: 0,
        };
        c.iniciar_aninhamento_de_argumento();
        c
    }

    /// `ChunkBuilder(formatter, source)`: o construtor de topo.
    pub fn topo(a: &mut Arenas, recuo_inicial: i32) -> Construtor {
        let mut c = Construtor::base(a, None);
        // `indent(_formatter.indent)` antes de `startBlockArgumentNesting`.
        c.aninhamento.pilha.push(recuo_inicial);
        c
    }

    /// A regra mais interna (`rule`).
    pub fn regra(&self) -> RegraId {
        *self.regras.last().expect("regra ativa")
    }

    /// `write(string, mergeEmptySplits:)`.
    pub fn escrever(&mut self, a: &mut Arenas, texto: &str, juntar_vazias: bool) {
        self.emitir_branco_pendente(a, false, juntar_vazias);
        self.escrever_texto(a, texto, None);
        let preguicosas = std::mem::take(&mut self.regras_preguicosas);
        for r in preguicosas {
            self.ativar_regra(a, r);
        }
        self.aninhamento.confirmar();
        self.apos_comentario = false;
    }

    /// `writeNewline(isDouble:, flushLeft:, nest:)`.
    pub fn nova_linha(&mut self, duplo: bool, rente: bool, aninhar: bool) {
        self.novas_linhas_pendentes = if duplo { 2 } else { 1 };
        self.rente_pendente = rente;
        self.aninhado_pendente = aninhar;
    }

    /// `writeSpace()`.
    pub fn espaco(&mut self) {
        self.espaco_pendente = true;
    }

    /// `split(nest:, space:)`.
    pub fn quebra(&mut self, a: &mut Arenas, aninhar: bool, espaco: bool) -> ChunkId {
        if self.impedir_quebra > 0 {
            self.novas_linhas_pendentes = 0;
            self.aninhado_pendente = false;
            if espaco {
                self.espaco_pendente = true;
            }
            return a.chunk_falso();
        }
        if self.novas_linhas_pendentes > 0 {
            return a.chunk_falso();
        }
        self.escrever_quebra(a, false, false, aninhar, espaco, true)
    }

    /// `writeComments(comments, linesBeforeToken, token)`.
    pub fn escrever_comentarios(&mut self, a: &mut Arenas, comentarios: &mut [ComentarioFonte], linhas_antes_do_token: i32, token: &str) {
        if self.novas_linhas_pendentes == 2 && comentarios[0].linhas_antes < 2 {
            if linhas_antes_do_token > 1 {
                self.nova_linha(false, false, false);
            } else {
                for c in comentarios.iter().skip(1) {
                    if c.linhas_antes > 1 {
                        self.nova_linha(false, false, false);
                        break;
                    }
                }
            }
        }

        if self.apos_comentario && self.novas_linhas_pendentes > 0 {
            comentarios[0].linhas_antes = 1;
            self.novas_linhas_pendentes = 0;
        }

        if linhas_antes_do_token == 0
            && i32::from(self.novas_linhas_pendentes) > comentarios[0].linhas_antes
            && comentarios.iter().all(|c| c.tipo == TipoComentario::BlocoEmLinha)
        {
            comentarios[0].linhas_antes = i32::from(self.novas_linhas_pendentes);
        }

        for i in 0..comentarios.len() {
            let comentario = comentarios[i].clone();
            let chunk = self.chunk_para_comentario(a, &comentario, token);
            if let Some(chunk) = chunk {
                self.espaco_pendente = self.precisa_espaco_antes(a, &comentario, chunk);
                let ultimo = *self.chunks.last().expect("chunk");
                if self.espaco_pendente && chunk != ultimo {
                    a.atualizar_quebra(ultimo, None, false, Some(true));
                }
            } else {
                if self.novas_linhas_pendentes == 0 {
                    if comentario.linhas_antes > 0 && (self.apos_comentario || comentario.tipo != TipoComentario::BlocoEmLinha) {
                        let duplo = self.precisa_linha_em_branco(a, &comentario);
                        self.nova_linha(duplo, comentario.rente, true);
                    } else if let Some(&ultimo) = self.chunks.last() {
                        self.espaco_pendente = self.precisa_espaco_antes(a, &comentario, ultimo);
                    }
                } else {
                    self.rente_pendente = comentario.rente;
                }
                let duplo = self.precisa_linha_em_branco(a, &comentario);
                self.emitir_branco_pendente(a, duplo, true);
            }

            self.escrever_texto(a, &comentario.texto, chunk);

            let mut linhas_depois;
            if i < comentarios.len() - 1 {
                linhas_depois = comentarios[i + 1].linhas_antes;
            } else {
                linhas_depois = linhas_antes_do_token;
                if linhas_depois == 0 && comentarios.last().expect("comentário").texto.contains('\n') {
                    linhas_depois = 1;
                }
            }
            if linhas_depois > 0 {
                let duplo = self.novas_linhas_pendentes == 2 || linhas_depois > 1;
                self.nova_linha(duplo, false, true);
            }
        }

        let ultimo = comentarios.last().expect("comentário").clone();
        self.espaco_pendente = self.precisa_espaco_depois(a, &ultimo, token);
        self.apos_comentario = true;
    }

    /// `indent([spaces])`.
    pub fn recuar(&mut self, espacos: Option<i32>) {
        let e = espacos.unwrap_or(recuo::BLOCK);
        debug_assert!(self.aninhamento.pendente.is_none());
        let topo = self.aninhamento.recuo();
        self.aninhamento.pilha.push(topo + e);
    }

    /// `unindent()`.
    pub fn desrecuar(&mut self) {
        debug_assert!(self.aninhamento.pendente.is_none());
        self.aninhamento.pilha.pop();
        debug_assert!(!self.aninhamento.pilha.is_empty());
    }

    /// `startSpan([cost])`.
    pub fn iniciar_span(&mut self, custo: i32) {
        self.spans_abertos.push((self.chunks.len(), custo));
    }

    /// `endSpan()`.
    pub fn terminar_span(&mut self, a: &mut Arenas) {
        let (inicio, custo) = self.spans_abertos.pop().expect("span aberto");
        let fim = self.chunks.len();
        if inicio == fim {
            return;
        }
        let span = a.novo_span(custo);
        for i in inicio..fim {
            let c = self.chunks[i];
            let r = a.chunks[c as usize].regra;
            if !a.endurecida(r) {
                a.chunks[c as usize].spans.push(span);
            }
        }
    }

    /// `startRule([rule])`.
    pub fn iniciar_regra(&mut self, a: &mut Arenas, regra: Option<RegraId>) {
        let r = regra.unwrap_or_else(|| a.regra(custo::NORMAL));
        let preguicosas = std::mem::take(&mut self.regras_preguicosas);
        for p in preguicosas {
            self.ativar_regra(a, p);
        }
        self.ativar_regra(a, r);
    }

    fn ativar_regra(&mut self, a: &mut Arenas, r: RegraId) {
        for &externa in &self.regras {
            if !a.quebra_com_internas(externa) {
                continue;
            }
            a.restringir_quando_quebra(r, externa);
        }
        self.regras.push(r);
    }

    /// `startLazyRule([rule])`.
    pub fn iniciar_regra_preguicosa(&mut self, a: &mut Arenas, regra: Option<RegraId>) {
        let r = regra.unwrap_or_else(|| a.regra(custo::NORMAL));
        self.regras_preguicosas.push(r);
    }

    /// `endRule()`.
    pub fn terminar_regra(&mut self) {
        if !self.regras_preguicosas.is_empty() {
            self.regras_preguicosas.pop();
        } else {
            self.regras.pop();
        }
    }

    /// `forceRules()`.
    pub fn forcar_regras(&mut self, a: &Arenas) {
        self.tratar_quebra_dura(a);
    }

    /// `nestExpression(indent:, now:)`.
    pub fn aninhar_expressao(&mut self, a: &mut Arenas, recuo: Option<i32>, agora: bool) {
        self.aninhamento.aninhar(a, recuo.unwrap_or(recuo::EXPRESSION));
        if agora {
            self.aninhamento.confirmar();
        }
    }

    /// `unnest(now:)`.
    pub fn desaninhar(&mut self, a: &Arenas, agora: bool) {
        self.aninhamento.desaninhar(a);
        if agora {
            self.aninhamento.confirmar();
        }
    }

    /// `startBlockArgumentNesting()`.
    pub fn iniciar_aninhamento_de_argumento(&mut self) {
        self.aninhamento_de_argumento.push(self.aninhamento.corrente());
    }

    /// `endBlockArgumentNesting()`.
    pub fn terminar_aninhamento_de_argumento(&mut self) {
        self.aninhamento_de_argumento.pop();
    }

    /// `startBlock(argumentChunk:, indent:, space:)`: cria o chunk de bloco
    /// aqui e devolve o construtor dos filhos.
    pub fn iniciar_bloco(&mut self, a: &mut Arenas, argumento: Option<ChunkId>, recuar: bool, espaco: bool) -> Construtor {
        let regra = *self.regras.last().expect("regra do bloco");
        let ninho = *self.aninhamento_de_argumento.last().expect("aninhamento de argumento");
        let chunk = a.novo_bloco(argumento, regra, self.aninhamento.recuo(), ninho, espaco, self.rente_pendente);
        self.chunks.push(chunk);
        self.rente_pendente = false;

        let mut filho = Construtor::base(a, Some(chunk));
        if recuar {
            filho.recuar(None);
        }
        let dura = a.regra_dura();
        filho.iniciar_regra(a, Some(dura));
        filho.quebra(a, false, espaco);
        filho
    }

    /// `endBlock(forceSplit:)`, primeira metade: fecha os filhos, move-os
    /// para o chunk de bloco e diz se o pai deve forçar as regras.
    pub fn terminar_bloco(mut self, a: &mut Arenas, forcar: bool) -> bool {
        self.dividir_chunks(a);
        let mut forcar = forcar || self.aninhado_pendente;
        if !forcar {
            let mut comprimento = 0usize;
            let primeira = self.regras.first().copied();
            for &c in &self.chunks {
                comprimento += a.chunks[c as usize].comprimento() + a.comprimento_bloco(c);
                if comprimento > a.largura {
                    forcar = true;
                    break;
                }
                let r = a.chunks[c as usize].regra;
                if a.endurecida(r) && Some(r) != primeira {
                    forcar = true;
                    break;
                }
            }
        }
        let bloco = self.bloco.expect("endBlock fora de bloco");
        if let Some(b) = &mut a.chunks[bloco as usize].bloco {
            b.filhos = std::mem::take(&mut self.chunks);
        }
        forcar
    }

    /// `end()`: divide os chunks de topo e devolve a lista.
    pub fn terminar(mut self, a: &mut Arenas) -> Vec<ChunkId> {
        debug_assert!(self.regras.is_empty());
        self.dividir_chunks(a);
        self.chunks
    }

    /// `preventSplit()`.
    pub fn impedir_quebras(&mut self) {
        self.impedir_quebra += 1;
    }

    /// `endPreventSplit()`.
    pub fn liberar_quebras(&mut self) {
        self.impedir_quebra -= 1;
        debug_assert!(self.impedir_quebra >= 0);
    }

    fn emitir_branco_pendente(&mut self, a: &mut Arenas, duplo: bool, juntar_vazias: bool) {
        if self.novas_linhas_pendentes == 0 {
            return;
        }
        let duplo = duplo || self.novas_linhas_pendentes == 2;
        let aninhar = self.aninhado_pendente;
        self.escrever_quebra(a, true, duplo, aninhar, false, juntar_vazias);
    }

    fn chunk_para_comentario(&self, a: &Arenas, c: &ComentarioFonte, token: &str) -> Option<ChunkId> {
        if self.chunks.is_empty() {
            return None;
        }
        if c.linhas_antes != 0 {
            return None;
        }
        if c.tipo == TipoComentario::Doc || c.tipo == TipoComentario::Bloco {
            return None;
        }
        let mut chunk = *self.chunks.last().expect("chunk");
        if a.chunks[chunk as usize].texto.is_empty() && self.chunks.len() > 1 && (!comentario_de_metodo_generico(c) || token != "(") {
            chunk = self.chunks[self.chunks.len() - 2];
        }
        let texto = &a.chunks[chunk as usize].texto;
        if texto.ends_with(',') && c.tipo == TipoComentario::BlocoEmLinha {
            return None;
        }
        if texto.ends_with('(') || texto.ends_with('[') || (texto.ends_with('{') && !texto.ends_with("${")) {
            return None;
        }
        Some(chunk)
    }

    fn precisa_espaco_antes(&self, a: &Arenas, c: &ComentarioFonte, chunk: ChunkId) -> bool {
        let texto = &a.chunks[chunk as usize].texto;
        if texto.is_empty() {
            return false;
        }
        if c.tipo == TipoComentario::Linha {
            return true;
        }
        if comentario_de_metodo_generico(c) && termina_com_ident(texto) {
            return false;
        }
        !texto.ends_with('(') && !texto.ends_with('[') && !texto.ends_with('{')
    }

    fn precisa_espaco_depois(&self, a: &Arenas, c: &ComentarioFonte, token: &str) -> bool {
        let ultimo = *self.chunks.last().expect("chunk");
        if a.chunks[ultimo as usize].texto.is_empty() {
            return false;
        }
        if self.novas_linhas_pendentes > 0 {
            return false;
        }
        if comentario_de_metodo_generico(c) && token == "(" {
            return false;
        }
        !matches!(token, ")" | "]" | "}" | "," | ";" | "")
    }

    fn precisa_linha_em_branco(&self, a: &Arenas, c: &ComentarioFonte) -> bool {
        if c.linhas_antes < 2 {
            return false;
        }
        let Some(&ultimo) = self.chunks.last() else {
            return false;
        };
        let texto = &a.chunks[ultimo as usize].texto;
        if texto.ends_with('{') || texto.ends_with('[') {
            return false;
        }
        true
    }

    fn escrever_quebra(&mut self, a: &mut Arenas, dura: bool, duplo: bool, aninhar: bool, espaco: bool, juntar_vazias: bool) -> ChunkId {
        let chunk;
        let ultimo_vazio = self.chunks.last().is_some_and(|&c| a.chunks[c as usize].texto.is_empty());
        if juntar_vazias && ultimo_vazio {
            chunk = *self.chunks.last().expect("chunk");
            let mut duplo = duplo;
            if duplo && self.chunks.len() > 1 && a.chunks[self.chunks[self.chunks.len() - 2] as usize].texto.ends_with('{') {
                duplo = false;
            }
            let r = a.chunks[chunk as usize].regra;
            a.endurecer(r);
            a.atualizar_quebra(chunk, Some(self.rente_pendente), duplo, None);
        } else {
            let ninho = if aninhar { self.aninhamento.atual } else { a.novo_ninho_raiz() };
            chunk = self.iniciar_chunk(a, ninho, dura, duplo, espaco);
        }
        let r = a.chunks[chunk as usize].regra;
        if a.endurecida(r) {
            self.tratar_quebra_dura(a);
        }
        self.novas_linhas_pendentes = 0;
        self.aninhado_pendente = false;
        chunk
    }

    fn escrever_texto(&mut self, a: &mut Arenas, texto: &str, chunk: Option<ChunkId>) {
        let chunk = match chunk {
            Some(c) => c,
            None => {
                if self.chunks.is_empty() {
                    let n = a.novo_ninho_raiz();
                    self.iniciar_chunk(a, n, true, false, false);
                }
                *self.chunks.last().expect("chunk")
            }
        };
        if self.espaco_pendente && !a.chunks[chunk as usize].texto.is_empty() {
            a.acrescentar(chunk, " ");
        }
        self.espaco_pendente = false;
        a.acrescentar(chunk, texto);
    }

    fn iniciar_chunk(&mut self, a: &mut Arenas, ninho: NinhoId, dura: bool, duplo: bool, espaco: bool) -> ChunkId {
        let regra = if dura { a.regra_dura() } else { *self.regras.last().expect("regra ativa") };
        let c = a.novo_chunk(regra, self.aninhamento.recuo(), ninho, espaco, self.rente_pendente, duplo);
        self.chunks.push(c);
        self.rente_pendente = false;
        c
    }

    fn dividir_chunks(&mut self, a: &mut Arenas) {
        self.endurecer_regras(a);
        for (i, &c) in self.chunks.iter().enumerate() {
            if !self.pode_dividir(a, i, c) {
                a.chunks[c as usize].divisivel = false;
            }
        }
    }

    fn pode_dividir(&self, a: &Arenas, i: usize, c: ChunkId) -> bool {
        if i == 0 {
            return false;
        }
        let ch = &a.chunks[c as usize];
        if !a.endurecida(ch.regra) {
            return false;
        }
        if a.aninhado(ch.ninho) {
            return false;
        }
        if ch.bloco.is_some() {
            return false;
        }
        true
    }

    fn tratar_quebra_dura(&mut self, a: &Arenas) {
        let Some(&ultima) = self.regras.last() else {
            return;
        };
        if !a.quebra_com_internas(ultima) {
            return;
        }
        if !self.regras_duras.contains(&ultima) {
            self.regras_duras.push(ultima);
        }
    }

    fn endurecer_regras(&mut self, a: &mut Arenas) {
        if self.regras_duras.is_empty() {
            return;
        }
        fn percorrer(a: &mut Arenas, r: RegraId) {
            a.endurecer(r);
            let mut i = 0;
            while let Some(outra) = a.restrita(r, i) {
                i += 1;
                if outra == r {
                    continue;
                }
                let total = a.valor_total(r);
                if !a.endurecida(outra) && a.restringe(r, total, outra) == Some(a.valor_total(outra)) {
                    percorrer(a, outra);
                }
            }
        }
        for &r in &self.regras_duras.clone() {
            percorrer(a, r);
        }
        for &c in &self.chunks {
            let r = a.chunks[c as usize].regra;
            if a.endurecida(r) {
                a.chunks[c as usize].spans.clear();
            }
        }
    }
}
