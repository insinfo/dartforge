//! As estruturas do back end do estilo curto, em arenas: regras de quebra
//! (`rule/*.dart`), *chunks* (`chunk.dart`), *spans* e níveis de
//! aninhamento de expressão (`nesting_level.dart`).
//!
//! O original usa objetos com identidade (comparações `==` entre chunks e
//! regras, conjuntos e mapas com ordem de inserção). Aqui cada objeto é um
//! índice na sua arena e a identidade é o índice; as coleções ordenadas são
//! vetores, preservando a ordem de iteração do Dart.

/// Índice de uma regra em [`Arenas::regras`].
pub type RegraId = u32;
/// Índice de um chunk em [`Arenas::chunks`].
pub type ChunkId = u32;
/// Índice de um span em [`Arenas::spans`].
pub type SpanId = u32;
/// Índice de um nível de aninhamento em [`Arenas::ninhos`].
pub type NinhoId = u32;

/// `Rule.unsplit`.
pub const NAO_QUEBRA: i32 = 0;
/// `Rule.mustSplit`: qualquer valor, desde que quebre.
pub const DEVE_QUEBRAR: i32 = -1;
/// `Rule._fullSplitConstraint`.
const QUEBRA_TOTAL: i32 = -2;

/// Custos (`Cost` de `constants.dart`).
pub mod custo {
    pub const ARROW: i32 = 0;
    pub const NORMAL: i32 = 1;
    pub const ASSIGN: i32 = 1;
    pub const ASSIGN_BLOCK: i32 = 2;
    pub const POSITIONAL_ARGUMENTS: i32 = 2;
    pub const SINGLE_ELEMENT_LIST: i32 = 2;
    pub const SPLIT_BLOCKS: i32 = 2;
    pub const CONSTRUCTOR_NAME: i32 = 4;
    pub const INDEX: i32 = 4;
    pub const TYPE_ARGUMENT: i32 = 4;
    pub const PARAMETER_TYPE: i32 = 4;
}

/// Recuos (`Indent` de `constants.dart`).
pub mod recuo {
    pub const BLOCK: i32 = 2;
    pub const CASCADE: i32 = 2;
    pub const EXPRESSION: i32 = 4;
    pub const CONSTRUCTOR_INITIALIZER: i32 = 4;
}

/// Restrição de uma regra sobre outra (`_Constraint`).
#[derive(Debug, Clone, Copy)]
struct Restricao {
    min: i32,
    max: i32,
    outro_valor: i32,
}

/// As subclasses de `Rule` e seus dados.
#[derive(Debug, Clone)]
pub enum TipoRegra {
    /// `Rule`.
    Simples,
    /// `SplitContainingRule` puro (não usado diretamente pelo visitante, mas
    /// a base de [`TipoRegra::Posicional`] e [`TipoRegra::Nomeada`]).
    Posicional {
        argumentos: Vec<Option<ChunkId>>,
        coleções_iniciais: usize,
        coleções_finais: usize,
    },
    /// `NamedRule`.
    Nomeada { argumentos: Vec<Option<ChunkId>> },
    /// `CombinatorRule`.
    Combinador {
        combinadores: Vec<ChunkId>,
        nomes: Vec<Vec<ChunkId>>,
    },
    /// `TypeArgumentRule`.
    ArgumentoDeTipo { argumentos: Vec<ChunkId> },
}

/// Uma regra de quebra (`Rule`).
#[derive(Debug, Clone)]
pub struct Regra {
    pub tipo: TipoRegra,
    custo: i32,
    /// `index`: posição na lista de regras do último divisor que a usou.
    /// Como no original, nunca volta a `None`.
    pub indice: Option<usize>,
    endurecida: bool,
    /// `SplitContainingRule._trackInnerRules`.
    rastrear_internas: bool,
    restricoes: Vec<(RegraId, Vec<Restricao>)>,
    todas_restritas: Option<Vec<RegraId>>,
}

/// Dados de um chunk de bloco (`BlockChunk`).
#[derive(Debug, Clone)]
pub struct Bloco {
    /// `argument`: o chunk antes do argumento que contém este bloco.
    pub argumento: Option<ChunkId>,
    pub filhos: Vec<ChunkId>,
    /// Memória de `unsplitBlockLength`, calculada depois que os filhos ficam
    /// prontos.
    comprimento: Option<usize>,
}

/// Um chunk (`Chunk`/`BlockChunk`).
#[derive(Debug, Clone)]
pub struct Chunk {
    pub texto: String,
    /// Comprimento de `texto` em unidades UTF-16 (o `length` do Dart).
    pub comp_texto: usize,
    pub recuo: i32,
    pub ninho: NinhoId,
    pub regra: RegraId,
    pub duplo: bool,
    pub rente: bool,
    pub espaco: bool,
    pub divisivel: bool,
    pub spans: Vec<SpanId>,
    pub bloco: Option<Bloco>,
}

impl Chunk {
    /// `length`.
    pub fn comprimento(&self) -> usize {
        usize::from(self.espaco) + self.comp_texto
    }
}

/// Um nível de aninhamento de expressão (`NestingLevel`).
#[derive(Debug, Clone)]
pub struct Ninho {
    pub pai: Option<NinhoId>,
    pub recuo: i32,
    marcado: bool,
    total: Option<i32>,
}

/// Um span (`Span`): custo pago uma vez se alguma quebra cair dentro dele.
#[derive(Debug, Clone)]
pub struct SpanC {
    pub custo: i32,
    marcado: bool,
}

/// As arenas de uma formatação e os parâmetros do formatador.
#[derive(Debug)]
pub struct Arenas {
    pub regras: Vec<Regra>,
    pub chunks: Vec<Chunk>,
    pub spans: Vec<SpanC>,
    pub ninhos: Vec<Ninho>,
    /// `Rule.dummy`.
    pub regra_falsa: RegraId,
    pub largura: usize,
    pub fim_de_linha: String,
}

/// Comprimento de um texto em unidades UTF-16.
pub fn comp16(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.encode_utf16().count()
    }
}

impl Arenas {
    /// Arenas vazias com a regra falsa já criada.
    pub fn nova(largura: usize, fim_de_linha: String) -> Arenas {
        let mut a = Arenas {
            regras: Vec::new(),
            chunks: Vec::new(),
            spans: Vec::new(),
            ninhos: Vec::new(),
            regra_falsa: 0,
            largura,
            fim_de_linha,
        };
        a.regra_falsa = a.regra_dura();
        a
    }

    // ------------------------------------------------------------- regras

    fn nova_regra(&mut self, tipo: TipoRegra, custo: i32) -> RegraId {
        self.regras.push(Regra {
            tipo,
            custo,
            indice: None,
            endurecida: false,
            rastrear_internas: true,
            restricoes: Vec::new(),
            todas_restritas: None,
        });
        self.regras.len() as RegraId - 1
    }

    /// `Rule([cost])`.
    pub fn regra(&mut self, custo: i32) -> RegraId {
        self.nova_regra(TipoRegra::Simples, custo)
    }

    /// `Rule.hard()`.
    pub fn regra_dura(&mut self) -> RegraId {
        let r = self.nova_regra(TipoRegra::Simples, 0);
        self.regras[r as usize].endurecida = true;
        r
    }

    /// `TypeArgumentRule()`.
    pub fn regra_argumento_de_tipo(&mut self) -> RegraId {
        self.nova_regra(TipoRegra::ArgumentoDeTipo { argumentos: Vec::new() }, custo::TYPE_ARGUMENT)
    }

    /// `CombinatorRule()`.
    pub fn regra_combinador(&mut self) -> RegraId {
        self.nova_regra(
            TipoRegra::Combinador {
                combinadores: Vec::new(),
                nomes: Vec::new(),
            },
            custo::NORMAL,
        )
    }

    /// `PositionalRule(collectionRule, argumentCount:, leadingCollections:,
    /// trailingCollections:)`.
    pub fn regra_posicional(
        &mut self,
        regra_colecao: Option<RegraId>,
        quantidade: usize,
        iniciais: usize,
        finais: usize,
    ) -> RegraId {
        let r = self.nova_regra(
            TipoRegra::Posicional {
                argumentos: Vec::new(),
                coleções_iniciais: iniciais,
                coleções_finais: finais,
            },
            custo::NORMAL,
        );
        let n = quantidade as i32;
        if iniciais > 0 {
            self.adicionar_restricao(r, 1, regra_colecao.expect("regra de coleção"), NAO_QUEBRA);
        }
        if iniciais > 0 || finais > 0 {
            self.adicionar_restricao(r, n + 1, regra_colecao.expect("regra de coleção"), 1);
        }
        for arg in 0..iniciais as i32 {
            let valor = n - arg + 1;
            self.adicionar_restricao(r, valor, regra_colecao.expect("regra de coleção"), NAO_QUEBRA);
        }
        let mut arg = n - finais as i32;
        while arg < n {
            let valor = n - arg + 1;
            self.adicionar_restricao(r, valor, regra_colecao.expect("regra de coleção"), NAO_QUEBRA);
            arg += 1;
        }
        r
    }

    /// `NamedRule(collectionRule, leadingCollections, trailingCollections)`.
    pub fn regra_nomeada(&mut self, regra_colecao: Option<RegraId>, iniciais: usize, finais: usize) -> RegraId {
        let r = self.nova_regra(TipoRegra::Nomeada { argumentos: Vec::new() }, custo::NORMAL);
        if iniciais > 0 || finais > 0 {
            self.adicionar_restricao(r, 1, regra_colecao.expect("regra de coleção"), NAO_QUEBRA);
        }
        r
    }

    /// `ArgumentRule.beforeArgument` e `TypeArgumentRule.beforeArgument`.
    pub fn antes_do_argumento(&mut self, r: RegraId, chunk: Option<ChunkId>) {
        match &mut self.regras[r as usize].tipo {
            TipoRegra::Posicional { argumentos, .. } | TipoRegra::Nomeada { argumentos } => argumentos.push(chunk),
            TipoRegra::ArgumentoDeTipo { argumentos } => argumentos.push(chunk.expect("chunk do argumento de tipo")),
            _ => panic!("beforeArgument numa regra que não é de argumentos"),
        }
    }

    /// `CombinatorRule.addCombinator`.
    pub fn adicionar_combinador(&mut self, r: RegraId, chunk: ChunkId) {
        if let TipoRegra::Combinador { combinadores, nomes } = &mut self.regras[r as usize].tipo {
            if !combinadores.contains(&chunk) {
                combinadores.push(chunk);
            }
            nomes.push(Vec::new());
        } else {
            panic!("addCombinator fora de CombinatorRule");
        }
    }

    /// `CombinatorRule.addName`.
    pub fn adicionar_nome(&mut self, r: RegraId, chunk: ChunkId) {
        if let TipoRegra::Combinador { nomes, .. } = &mut self.regras[r as usize].tipo {
            let ultimo = nomes.last_mut().expect("combinador antes dos nomes");
            if !ultimo.contains(&chunk) {
                ultimo.push(chunk);
            }
        } else {
            panic!("addName fora de CombinatorRule");
        }
    }

    /// `rule is CombinatorRule`.
    pub fn e_combinador(&self, r: RegraId) -> bool {
        matches!(self.regras[r as usize].tipo, TipoRegra::Combinador { .. })
    }

    /// `numValues`.
    pub fn num_valores(&self, r: RegraId) -> i32 {
        match &self.regras[r as usize].tipo {
            TipoRegra::Simples => 2,
            TipoRegra::Posicional {
                argumentos,
                coleções_iniciais,
                coleções_finais,
            } => {
                let mut n = argumentos.len() as i32 + 1;
                if argumentos.len() > 1 {
                    n += 1;
                }
                if *coleções_iniciais > 0 || *coleções_finais > 0 {
                    n += 1;
                }
                n
            }
            TipoRegra::Nomeada { .. } => 3,
            TipoRegra::Combinador { nomes, .. } => {
                if nomes.len() == 2 {
                    5
                } else {
                    3
                }
            }
            TipoRegra::ArgumentoDeTipo { argumentos } => {
                if argumentos.len() == 1 {
                    2
                } else {
                    argumentos.len() as i32 + 2
                }
            }
        }
    }

    /// `fullySplitValue`.
    pub fn valor_total(&self, r: RegraId) -> i32 {
        self.num_valores(r) - 1
    }

    /// `cost`.
    pub fn custo(&self, r: RegraId) -> i32 {
        self.regras[r as usize].custo
    }

    /// `isHardened`.
    pub fn endurecida(&self, r: RegraId) -> bool {
        self.regras[r as usize].endurecida
    }

    /// `harden()`.
    pub fn endurecer(&mut self, r: RegraId) {
        self.regras[r as usize].endurecida = true;
    }

    /// `splitsOnInnerRules`.
    pub fn quebra_com_internas(&self, r: RegraId) -> bool {
        match self.regras[r as usize].tipo {
            TipoRegra::Posicional { .. } | TipoRegra::Nomeada { .. } => self.regras[r as usize].rastrear_internas,
            _ => true,
        }
    }

    /// `disableSplitOnInnerRules()`.
    pub fn desligar_internas(&mut self, r: RegraId) {
        debug_assert!(self.regras[r as usize].rastrear_internas);
        self.regras[r as usize].rastrear_internas = false;
    }

    /// `enableSplitOnInnerRules()`.
    pub fn ligar_internas(&mut self, r: RegraId) {
        debug_assert!(!self.regras[r as usize].rastrear_internas);
        self.regras[r as usize].rastrear_internas = true;
    }

    /// `isSplit(value, chunk)`.
    pub fn quebra(&self, r: RegraId, valor: i32, chunk: ChunkId) -> bool {
        let regra = &self.regras[r as usize];
        if let TipoRegra::ArgumentoDeTipo { argumentos } = &regra.tipo {
            // `TypeArgumentRule` sobrescreve o `isSplit` inteiro.
            if valor == NAO_QUEBRA {
                return false;
            }
            if valor == self.num_valores(r) - 1 {
                return true;
            }
            let i = argumentos.len() as i32 - valor;
            return argumentos.get(i as usize).is_some_and(|c| *c == chunk) && i >= 0;
        }
        if regra.endurecida {
            return true;
        }
        if valor == NAO_QUEBRA {
            return false;
        }
        match &regra.tipo {
            TipoRegra::Simples => true,
            TipoRegra::Posicional {
                argumentos,
                coleções_iniciais,
                coleções_finais,
            } => {
                let n = argumentos.len() as i32;
                if valor == 1 {
                    return argumentos.first().copied().flatten() == Some(chunk);
                }
                if valor <= n {
                    let arg = (n - valor + 1) as usize;
                    return argumentos[arg] == Some(chunk);
                }
                if valor == n + 1 {
                    for a in argumentos.iter().take(*coleções_iniciais) {
                        if *a == Some(chunk) {
                            return false;
                        }
                    }
                    for a in argumentos.iter().skip(argumentos.len() - coleções_finais) {
                        if *a == Some(chunk) {
                            return false;
                        }
                    }
                    return true;
                }
                true
            }
            TipoRegra::Nomeada { argumentos } => {
                if valor == 1 {
                    return argumentos.first().copied().flatten() == Some(chunk);
                }
                true
            }
            TipoRegra::Combinador { combinadores, nomes } => {
                let no_combinador = |i: usize| combinadores.contains(&chunk) || nomes[i].contains(&chunk);
                match valor {
                    1 => combinadores.contains(&chunk),
                    2 => no_combinador(0),
                    3 => {
                        if nomes.len() == 2 {
                            no_combinador(1)
                        } else {
                            true
                        }
                    }
                    _ => true,
                }
            }
            TipoRegra::ArgumentoDeTipo { .. } => unreachable!(),
        }
    }

    /// `constrain(value, other)`.
    pub fn restringe(&self, r: RegraId, valor: i32, outra: RegraId) -> Option<i32> {
        if valor == NAO_QUEBRA {
            return None;
        }
        let regra = &self.regras[r as usize];
        let (_, lista) = regra.restricoes.iter().find(|(o, _)| *o == outra)?;
        for c in lista {
            if valor >= c.min && valor <= c.max {
                if c.outro_valor == QUEBRA_TOTAL {
                    return Some(self.valor_total(outra));
                }
                return Some(c.outro_valor);
            }
        }
        None
    }

    /// `addConstraint(value, other, otherValue)`.
    pub fn adicionar_restricao(&mut self, r: RegraId, valor: i32, outra: RegraId, outro_valor: i32) {
        self.adicionar_restricao_faixa(r, valor, valor, outra, outro_valor);
    }

    /// `addRangeConstraint(min, max, other, otherValue)`.
    pub fn adicionar_restricao_faixa(&mut self, r: RegraId, min: i32, max: i32, outra: RegraId, outro_valor: i32) {
        let regra = &mut self.regras[r as usize];
        let c = Restricao { min, max, outro_valor };
        if let Some((_, lista)) = regra.restricoes.iter_mut().find(|(o, _)| *o == outra) {
            lista.push(c);
        } else {
            regra.restricoes.push((outra, vec![c]));
        }
    }

    /// `constrainWhenSplit(other)`.
    pub fn restringir_quando_quebra(&mut self, r: RegraId, outra: RegraId) {
        self.adicionar_restricao_faixa(r, 1, 100000, outra, QUEBRA_TOTAL);
    }

    /// `constrainWhenFullySplit(other)`.
    pub fn restringir_quando_quebra_total(&mut self, r: RegraId, outra: RegraId) {
        let v = self.valor_total(r);
        self.adicionar_restricao(r, v, outra, QUEBRA_TOTAL);
    }

    /// `PositionalRule.addNamedArgsConstraints(rule)`.
    pub fn restricoes_de_nomeados(&mut self, posicional: RegraId, outra: RegraId) {
        self.restringir_quando_quebra_total(posicional, outra);
        let total = self.valor_total(posicional);
        self.adicionar_restricao_faixa(posicional, 1, total, outra, DEVE_QUEBRAR);
    }

    /// `forgetUnusedRules()`.
    pub fn esquecer_nao_usadas(&mut self, r: RegraId) {
        let indices: Vec<bool> = self.regras[r as usize]
            .restricoes
            .iter()
            .map(|(o, _)| self.regras[*o as usize].indice.is_some())
            .collect();
        let regra = &mut self.regras[r as usize];
        let mut i = 0;
        regra.restricoes.retain(|_| {
            let manter = indices[i];
            i += 1;
            manter
        });
        regra.todas_restritas = None;
    }

    /// `constrainedRules`.
    pub fn restritas(&self, r: RegraId) -> Vec<RegraId> {
        self.regras[r as usize].restricoes.iter().map(|(o, _)| *o).collect()
    }

    /// Quantidade de regras restringidas (`constrainedRules.length`).
    pub fn tem_restritas(&self, r: RegraId) -> bool {
        !self.regras[r as usize].restricoes.is_empty()
    }

    /// A regra restringida de posição `i`.
    pub fn restrita(&self, r: RegraId, i: usize) -> Option<RegraId> {
        self.regras[r as usize].restricoes.get(i).map(|(o, _)| *o)
    }

    /// `allConstrainedRules`: o fecho transitivo, incluindo a própria regra.
    pub fn todas_restritas(&mut self, r: RegraId) -> Vec<RegraId> {
        if let Some(v) = &self.regras[r as usize].todas_restritas {
            return v.clone();
        }
        let mut v: Vec<RegraId> = Vec::new();
        let mut vistas = std::collections::HashSet::new();
        self.percorrer(&mut v, &mut vistas, r);
        self.regras[r as usize].todas_restritas = Some(v.clone());
        v
    }

    fn percorrer(&self, v: &mut Vec<RegraId>, vistas: &mut std::collections::HashSet<RegraId>, r: RegraId) {
        if !vistas.insert(r) {
            return;
        }
        v.push(r);
        for (o, _) in &self.regras[r as usize].restricoes {
            self.percorrer(v, vistas, *o);
        }
    }

    // ------------------------------------------------------------- chunks

    /// `Chunk(rule, indent, nesting, space:, flushLeft:, isDouble:)`.
    pub fn novo_chunk(&mut self, regra: RegraId, recuo: i32, ninho: NinhoId, espaco: bool, rente: bool, duplo: bool) -> ChunkId {
        self.chunks.push(Chunk {
            texto: String::new(),
            comp_texto: 0,
            recuo,
            ninho,
            regra,
            duplo,
            rente,
            espaco,
            divisivel: true,
            spans: Vec::new(),
            bloco: None,
        });
        self.chunks.len() as ChunkId - 1
    }

    /// `BlockChunk(argument, rule, indent, nesting, space:, flushLeft:)`.
    pub fn novo_bloco(&mut self, argumento: Option<ChunkId>, regra: RegraId, recuo: i32, ninho: NinhoId, espaco: bool, rente: bool) -> ChunkId {
        let c = self.novo_chunk(regra, recuo, ninho, espaco, rente, false);
        self.chunks[c as usize].bloco = Some(Bloco {
            argumento,
            filhos: Vec::new(),
            comprimento: None,
        });
        c
    }

    /// `Chunk.dummy()`.
    pub fn chunk_falso(&mut self) -> ChunkId {
        let n = self.novo_ninho_raiz();
        let r = self.regra_falsa;
        let c = self.novo_chunk(r, 0, n, false, false, false);
        self.chunks[c as usize].texto = "(dummy)".into();
        self.chunks[c as usize].comp_texto = 7;
        c
    }

    /// `appendText`.
    pub fn acrescentar(&mut self, c: ChunkId, texto: &str) {
        let ch = &mut self.chunks[c as usize];
        ch.texto.push_str(texto);
        ch.comp_texto += comp16(texto);
    }

    /// `updateSplit(flushLeft:, isDouble:, space:)`.
    pub fn atualizar_quebra(&mut self, c: ChunkId, rente: Option<bool>, duplo: bool, espaco: Option<bool>) {
        let ch = &mut self.chunks[c as usize];
        if let Some(r) = rente {
            ch.rente = r;
        }
        if duplo {
            ch.duplo = true;
        }
        if let Some(e) = espaco {
            ch.espaco = e;
        }
    }

    /// `unsplitBlockLength`.
    pub fn comprimento_bloco(&mut self, c: ChunkId) -> usize {
        let Some(b) = &self.chunks[c as usize].bloco else {
            return 0;
        };
        if let Some(n) = b.comprimento {
            return n;
        }
        let filhos = b.filhos.clone();
        let mut n = 0;
        for f in filhos {
            n += self.chunks[f as usize].comprimento() + self.comprimento_bloco(f);
        }
        if let Some(b) = &mut self.chunks[c as usize].bloco {
            b.comprimento = Some(n);
        }
        n
    }

    /// `indentBlock(getValue)`, com o valor da regra do argumento já dado.
    pub fn recua_bloco(&self, c: ChunkId, valor_de: &dyn Fn(RegraId) -> i32) -> bool {
        let Some(b) = &self.chunks[c as usize].bloco else {
            return false;
        };
        let Some(arg) = b.argumento else {
            return false;
        };
        let regra = self.chunks[arg as usize].regra;
        if regra == self.regra_falsa {
            return true;
        }
        self.quebra(regra, valor_de(regra), arg)
    }

    // ------------------------------------------------------------ aninhamento

    /// `NestingLevel()`.
    pub fn novo_ninho_raiz(&mut self) -> NinhoId {
        self.ninhos.push(Ninho {
            pai: None,
            recuo: 0,
            marcado: false,
            total: None,
        });
        self.ninhos.len() as NinhoId - 1
    }

    /// `nest(spaces)`.
    pub fn aninhar(&mut self, pai: NinhoId, recuo: i32) -> NinhoId {
        self.ninhos.push(Ninho {
            pai: Some(pai),
            recuo,
            marcado: false,
            total: None,
        });
        self.ninhos.len() as NinhoId - 1
    }

    /// `isNested`.
    pub fn aninhado(&self, n: NinhoId) -> bool {
        self.ninhos[n as usize].pai.is_some()
    }

    /// `mark()`.
    pub fn marcar_ninho(&mut self, n: NinhoId) -> bool {
        let x = &mut self.ninhos[n as usize];
        if x.marcado {
            return false;
        }
        x.marcado = true;
        true
    }

    /// `unmark()`.
    pub fn desmarcar_ninho(&mut self, n: NinhoId) {
        self.ninhos[n as usize].marcado = false;
    }

    /// `clearTotalUsedIndent()`.
    pub fn limpar_total(&mut self, n: NinhoId) {
        let mut atual = Some(n);
        while let Some(i) = atual {
            self.ninhos[i as usize].total = None;
            atual = self.ninhos[i as usize].pai;
        }
    }

    /// `refreshTotalUsedIndent()`.
    pub fn atualizar_total(&mut self, n: NinhoId) {
        if self.ninhos[n as usize].total.is_some() {
            return;
        }
        let mut total = 0;
        if let Some(p) = self.ninhos[n as usize].pai {
            self.atualizar_total(p);
            total += self.ninhos[p as usize].total.expect("total do pai");
        }
        if self.ninhos[n as usize].marcado {
            total += self.ninhos[n as usize].recuo;
        }
        self.ninhos[n as usize].total = Some(total);
    }

    /// `totalUsedIndent`.
    pub fn total_ninho(&self, n: NinhoId) -> i32 {
        self.ninhos[n as usize].total.expect("totalUsedIndent calculado")
    }

    // ------------------------------------------------------------- spans

    /// `Span(cost)`.
    pub fn novo_span(&mut self, custo: i32) -> SpanId {
        self.spans.push(SpanC { custo, marcado: false });
        self.spans.len() as SpanId - 1
    }

    /// `mark()` de span.
    pub fn marcar_span(&mut self, s: SpanId) -> bool {
        let x = &mut self.spans[s as usize];
        if x.marcado {
            return false;
        }
        x.marcado = true;
        true
    }

    /// `unmark()` de span.
    pub fn desmarcar_span(&mut self, s: SpanId) {
        self.spans[s as usize].marcado = false;
    }
}
