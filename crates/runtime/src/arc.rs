//! O núcleo do ARC com coleta de ciclos (docs/ARC-CICLOS-ESPECIFICACAO.md §19,
//! §22): os metadados laterais por objeto, o `retain_ref`/`release` de uma
//! ocorrência forte, a fila de zeros com o descarte em cascata e a coleta de
//! ciclos por *trial deletion* lateral, sem alterar o RC real (§22.1).
//!
//! O módulo não conhece o layout dos blocos: as arestas de cada objeto vêm de
//! um [`crate::arc::ArcGraph`] (no heap, a enumeração pela forma do corpo; nos testes, um
//! grafo de mentira). Nenhuma operação daqui aloca objeto Dart, lança exceção
//! Dart ou chama código Dart (§19.3).
//!
//! Contratos:
//! * o contador é por **ocorrência** forte (§18.1, decisão 2): duas posições
//!   de uma lista apontando para `x` contam duas vezes;
//! * `null` (0) e `Smi` (ímpar) não são gerenciados; um handle sem metadados
//!   (objeto da imagem, permanente) é tratado como imortal;
//! * a geração é monotônica por registro; as filas guardam [`crate::arc::ArcId`] e
//!   descartam a entrada cuja geração não bate (§19.1);
//! * a coleta de ciclos roda com o mutador parado e sem reentrada (§18.1,
//!   decisão 5): o chamador garante isso.

use crate::hash::{HashMap, HashSet};

/// Um valor em posição de referência (o `Ref` de `layout`).
pub type Ref = i64;

/// O ciclo de vida lógico de um objeto (§19.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcState {
    /// Alocado, campos ainda sendo inicializados: não morre por RC zero.
    Building,
    Live,
    /// Dentro da transação de descarte de um conjunto.
    Collecting,
    /// Logicamente morto: aguarda o reclamador físico.
    Dead,
}

/// Os metadados laterais de um objeto gerenciado (§19.1).
#[derive(Clone, Copy, Debug)]
pub struct MetaArc {
    pub generation: u64,
    /// Owners mais ocorrências fortes; não inclui weak.
    pub rc: u64,
    pub estado: ArcState,
    /// Está (ou esteve) na fila de candidatos a ciclo desde a última rodada.
    pub candidate: bool,
    /// Valor de ephemeron (ou clausura dele) protegido entre rodadas (§22.2):
    /// RC zero não inicia o descarte.
    pub conditionally_protected: bool,
    pub immortal: bool,
    /// Está na lista dos adiados (RC zero com raiz), uma vez só.
    pub deferred: bool,
    /// O corpo não basta para provar ausência de arestas nesta geração.
    pub side_edges: bool,
}

/// Identidade de um objeto nas filas: o handle e a geração do registro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArcId {
    pub handle: Ref,
    pub generation: u64,
}

/// As arestas fortes dos objetos e o rompimento delas na morte (§19.2).
pub trait ArcGraph {
    /// Cada ocorrência forte do objeto `h`, com multiplicidade.
    fn edges(&self, h: Ref, f: &mut dyn FnMut(Ref));
    /// Zera as ocorrências fortes de `h` (o descarte já debitou os destinos).
    fn break_off(&mut self, h: Ref);
    /// [`Self::edges`] e [`Self::break_off`] num percurso só: cada ocorrência
    /// forte de `h`, que sai zerada (o descarte de um objeto só, a cascata).
    fn take_edges(&mut self, h: Ref, f: &mut dyn FnMut(Ref)) {
        self.edges(h, f);
        self.break_off(h);
    }
}

/// Violação interna do contador (não é erro Dart): diagnóstico fatal (§19.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArcError {
    /// `release` com RC já zero.
    RcZero(Ref),
    /// Incremento além de `u64::MAX`.
    Overflow(Ref),
    /// Operação num objeto `Collecting`/`Dead`.
    InvalidState(Ref, ArcState),
    /// Registro de um handle ainda vivo.
    AlreadyRegistered(Ref),
    /// O trial deletion achou mais arestas internas que o RC (inventário ou
    /// contagem errados): a coleta é abortada sem mudar nada (§22.1).
    NegativeTrial(Ref),
    /// A auditoria recontou um RC diferente.
    Audit { handle: Ref, rc: u64, recounted: u64 },
}

/// Contadores de trabalho (§22.5, §26.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArcStats {
    pub registered_set: u64,
    pub retains: u64,
    pub releases: u64,
    /// Mortos pelo RC zero (cascata acíclica).
    pub dead_by_rc: u64,
    /// Mortos pela coleta de ciclos.
    pub dead_by_cycle: u64,
    pub cycle_rounds: u64,
    /// Objetos na região `R` de todas as rodadas.
    pub examined: u64,
    /// Entradas tiradas da fila de zeros e, delas, as adiadas (RC zero com
    /// raiz): o diagnóstico do custo da cascata.
    pub seen_zeros: u64,
    pub deferred_zeros: u64,
}

/// `null` e `Smi` não são referências gerenciadas.
#[inline]
pub fn e_handle(h: Ref) -> bool {
    h != 0 && h & 1 == 0
}

/// A geometria da página de um bloco do espaço, que o heap informa no
/// registro: o endereço do primeiro bloco, os bytes de cada bloco e quantos
/// cabem na página. O tamanho é positivo e `size * blocks` cabe em u64:
/// a extensão descreve memória contígua, sem volta no espaço de endereços.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub first: u64,
    pub size: u64,
    pub blocks: u32,
}

/// Os metadados dos blocos de uma página, pelo índice do bloco.
struct ArcPage {
    geometry: Geometry,
    /// Inverso da parte ímpar do tamanho, módulo 2⁶⁴, e a potência de dois
    /// retirada: a divisão exata vira multiplicação e rotação.
    inverse: u64,
    offset: u32,
    metas: Vec<Option<MetaArc>>,
    occupied: u32,
}

impl ArcPage {
    /// Prepara uma vez a divisão exata pelo tamanho dos blocos. A extensão
    /// precisa caber em u64: com isso, um índice menor que `blocks` não pode
    /// representar um produto que deu a volta no espaço de endereços.
    fn factors(g: Geometry) -> (u64, u32) {
        assert!(g.size != 0 && g.size.checked_mul(u64::from(g.blocks)).is_some(), "bug do ARC: geometria inválida");
        let offset = g.size.trailing_zeros();
        let odd = g.size >> offset;
        let mut inverse = odd;
        // Newton: cada passo dobra os bits corretos do inverso ímpar.
        for _ in 0..6 {
            inverse = inverse.wrapping_mul(2u64.wrapping_sub(odd.wrapping_mul(inverse)));
        }
        (inverse, offset)
    }

    fn new_area(g: Geometry) -> Self {
        let (inverse, offset) = Self::factors(g);
        Self { geometry: g, inverse, offset, metas: (0..g.blocks).map(|_| None).collect(), occupied: 0 }
    }

    /// Índice de um início de bloco; a rotação leva os bits que não são
    /// divisíveis pela potência de dois para o alto, fora de `blocks`.
    /// O produto pelo inverso recusa igualmente os múltiplos inexatos da
    /// parte ímpar. Não há divisão nem resto no caminho de consulta.
    #[inline]
    fn ix(&self, h: Ref) -> Option<usize> {
        let d = (h.wrapping_sub(crate::layout::DESLOCAMENTO_DO_HANDLE) as u64).checked_sub(self.geometry.first)?;
        let i = d.wrapping_mul(self.inverse).rotate_right(self.offset);
        (i < u64::from(self.geometry.blocks)).then_some(i as usize)
    }
}

/// A tabela dos metadados por handle (§19.1: "medir depois uma tabela por
/// página/índice de bloco, evitando um hash por retain"). O handle de um bloco
/// do espaço acha a página pelo número dela (`h >> 16`), num mapa radix de
/// 2¹⁶ posições por região de 4 GiB — como o mapa de páginas do espaço —, e o
/// metadado pelo índice do bloco: nenhum hash. O handle registrado sem
/// geometria (os testes do núcleo, os objetos da imagem) fica numa tabela de
/// reserva por hash.
#[derive(Default)]
struct Table {
    regions: Vec<(u64, Box<[u32]>)>,
    pages: Vec<ArcPage>,
    loose: HashMap<Ref, MetaArc>,
    n: usize,
}

/// `PAGINA` = 64 KiB; uma região do mapa radix cobre 2¹⁶ páginas.
const BITS_DA_PAGINA: u32 = 16;
const REGION_BITS: u32 = 16;

// Localização privada, válida apenas enquanto não registrar/reclamar objetos.
// A drenagem de um zero altera contadores/filas, não a geometria das páginas
// ou a identidade dos slots. Não conservar este valor entre drenagens.
#[derive(Clone, Copy)]
enum MetadataLocation {
    Page { page: usize, ix: usize },
    Loose(Ref),
}

impl Table {
    #[inline]
    fn key(h: Ref) -> (u64, usize) {
        let k = (h as u64) >> BITS_DA_PAGINA;
        (k >> REGION_BITS, (k & ((1 << REGION_BITS) - 1)) as usize)
    }

    /// A página registrada de `h`, se há.
    #[inline]
    fn page(&self, h: Ref) -> Option<usize> {
        let (r, i) = Self::key(h);
        let (_, t) = self.regions.iter().find(|(x, _)| *x == r)?;
        let v = t[i];
        (v != 0).then(|| v as usize - 1)
    }

    /// (página, índice) do metadado de `h` numa página registrada.
    #[inline]
    fn place(&self, h: Ref) -> Option<(usize, usize)> {
        let p = self.page(h)?;
        Some((p, self.pages[p].ix(h)?))
    }

    fn locate(&self, h: Ref) -> MetadataLocation {
        match self.place(h) {
            Some((page, ix)) => MetadataLocation::Page { page, ix },
            None => MetadataLocation::Loose(h),
        }
    }

    fn in_local(&self, local: MetadataLocation) -> Option<&MetaArc> {
        match local {
            MetadataLocation::Page { page, ix } => self.pages[page].metas[ix].as_ref(),
            MetadataLocation::Loose(h) => self.loose.get(&h),
        }
    }

    fn in_local_mut(&mut self, local: MetadataLocation) -> Option<&mut MetaArc> {
        match local {
            MetadataLocation::Page { page, ix } => self.pages[page].metas[ix].as_mut(),
            MetadataLocation::Loose(h) => self.loose.get_mut(&h),
        }
    }

    fn get(&self, h: &Ref) -> Option<&MetaArc> {
        match self.place(*h) {
            Some((p, i)) => self.pages[p].metas[i].as_ref(),
            None if self.loose.is_empty() => None,
            None => self.loose.get(h),
        }
    }

    fn get_mut(&mut self, h: &Ref) -> Option<&mut MetaArc> {
        match self.place(*h) {
            Some((p, i)) => self.pages[p].metas[i].as_mut(),
            None if self.loose.is_empty() => None,
            None => self.loose.get_mut(h),
        }
    }

    fn contains_key(&self, h: &Ref) -> bool {
        self.get(h).is_some()
    }

    fn len(&self) -> usize {
        self.n
    }

    /// Põe o metadado de `h`: pela página, com a geometria dela, ou na
    /// reserva, sem. A página que o espaço reformatou para outra classe (a
    /// página vazia reaproveitada) chega com outra geometria e não pode ter
    /// metadado ocupado: todo morto já saiu da tabela.
    fn insert(&mut self, h: Ref, m: MetaArc, geometry: Option<Geometry>) {
        let Some(g) = geometry else {
            if self.loose.insert(h, m).is_none() {
                self.n += 1;
            }
            return;
        };
        let p = match self.page(h) {
            Some(p) => p,
            None => self.new_page(h, g),
        };
        let pg = &mut self.pages[p];
        if pg.geometry != g {
            assert!(pg.occupied == 0, "bug do ARC: página reformatada com {} metadados vivos", pg.occupied);
            pg.geometry = g;
            (pg.inverse, pg.offset) = ArcPage::factors(g);
            pg.metas.clear();
            pg.metas.resize_with(g.blocks as usize, || None);
        }
        let i = pg.ix(h).expect("bug do ARC: handle fora da geometria da página");
        if pg.metas[i].replace(m).is_none() {
            pg.occupied += 1;
            self.n += 1;
        }
    }

    fn new_page(&mut self, h: Ref, g: Geometry) -> usize {
        let (r, i) = Self::key(h);
        let k = match self.regions.iter().position(|(x, _)| *x == r) {
            Some(k) => k,
            None => {
                self.regions.push((r, vec![0u32; 1 << REGION_BITS].into_boxed_slice()));
                self.regions.len() - 1
            }
        };
        self.pages.push(ArcPage::new_area(g));
        let p = self.pages.len() - 1;
        self.regions[k].1[i] = u32::try_from(p + 1).expect("ARC: páginas demais");
        p
    }

    fn remove(&mut self, h: &Ref) -> Option<MetaArc> {
        let m = match self.place(*h) {
            Some((p, i)) => {
                let m = self.pages[p].metas[i].take();
                if m.is_some() {
                    self.pages[p].occupied -= 1;
                }
                m
            }
            None => self.loose.remove(h),
        };
        if m.is_some() {
            self.n -= 1;
        }
        m
    }

    /// Cada handle registrado com o metadado.
    fn iter(&self) -> impl Iterator<Item = (Ref, &MetaArc)> + '_ {
        self.pages
            .iter()
            .flat_map(|pg| {
                let g = pg.geometry;
                pg.metas.iter().enumerate().filter_map(move |(i, m)| {
                    m.as_ref().map(|m| ((g.first + i as u64 * g.size) as Ref + crate::layout::DESLOCAMENTO_DO_HANDLE, m))
                })
            })
            .chain(self.loose.iter().map(|(&h, m)| (h, m)))
    }

    fn values_mut(&mut self) -> impl Iterator<Item = &mut MetaArc> + '_ {
        self.pages.iter_mut().flat_map(|pg| pg.metas.iter_mut().flatten()).chain(self.loose.values_mut())
    }
}

impl std::ops::Index<&Ref> for Table {
    type Output = MetaArc;
    fn index(&self, h: &Ref) -> &MetaArc {
        self.get(h).expect("bug do ARC: handle sem metadado")
    }
}

/// O estado do ARC de um isolate (§19.1).
#[derive(Default)]
pub struct EstadoDoArc {
    objects: Table,
    candidates: Vec<ArcId>,
    zeros: Vec<ArcId>,
    /// Os de RC zero que uma raiz segurava na última drenagem dos zeros, uma
    /// entrada por objeto ([`MetaArc::deferred`]); voltam à fila uma vez por
    /// coleta ([`Self::resume_deferred`]), quando as raízes podem ter mudado.
    deferred_set: Vec<ArcId>,
    dead_set: Vec<Ref>,
    next_generation: u64,
    /// Uma transação de descarte está em curso (não reentrar).
    transaction: bool,
    /// As saídas do conjunto em descarte, reaproveitado ([`Self::discard`]).
    exits: Vec<Ref>,
    pub statistics: ArcStats,
    /// Uma linha no stderr por evento (registro, retain, release, morte): o
    /// diagnóstico de `DARTFORGE_ARC_TRACO=1`.
    pub trace_log: bool,
    /// O objeto não tem posição de referência nenhuma (no heap, o corpo
    /// `BRUTO`: string, caixa numérica, dados tipados)? Sem aresta de saída
    /// ele nunca está num ciclo, e não entra entre os candidatos (o filtro
    /// mínimo do §32). `None` (os testes do núcleo): todo objeto pode.
    pub without_edges: Option<fn(Ref) -> bool>,
}

impl EstadoDoArc {
    pub fn new() -> Self {
        EstadoDoArc { next_generation: 1, ..Default::default() }
    }

    /// Os metadados de `h`, se gerenciado.
    pub fn meta(&self, h: Ref) -> Option<&MetaArc> {
        self.objects.get(&h)
    }

    /// Quantos objetos têm metadados (vivos, em construção ou mortos ainda
    /// não reclamados).
    pub fn registered_set(&self) -> usize {
        self.objects.len()
    }

    /// Registra o bloco recém-reservado `h` (§19.3): `rc=1` (o owner de
    /// construção), estado `Building`, geração nova. Falha antes de o
    /// handle ser publicado.
    pub fn register(&mut self, h: Ref) -> Result<ArcId, ArcError> {
        debug_assert!(e_handle(h));
        if let Some(m) = self.objects.get(&h)
            && m.estado != ArcState::Dead
        {
            return Err(ArcError::AlreadyRegistered(h));
        }
        let generation = self.next_generation;
        // Geração monotônica; overflow é falha interna, nunca reuso (§19.1).
        self.next_generation = self.next_generation.checked_add(1).expect("ARC: geração esgotada");
        self.objects.insert(
            h,
            MetaArc { generation, rc: 1, estado: ArcState::Building, candidate: false, conditionally_protected: false, immortal: false, deferred: false, side_edges: false },
            None,
        );
        self.statistics.registered_set += 1;
        Ok(ArcId { handle: h, generation })
    }

    /// Registra um objeto da imagem comprovadamente vivo pela imagem inteira
    /// (§19.1): não morre nem entra em filas; suas arestas continuam contadas.
    ///
    /// # Panics
    /// A geração monotônica se esgotou; nenhum registro é publicado nesse caso.
    ///
    /// ```
    /// use dartforge_runtime::arc::EstadoDoArc;
    /// let mut arc = EstadoDoArc::new();
    /// arc.register_immortal(2);
    /// assert!(arc.meta(2).unwrap().immortal);
    /// ```
    pub fn register_immortal(&mut self, h: Ref) {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.checked_add(1).expect("ARC: geração esgotada");
        self.objects.insert(h, MetaArc { generation, rc: 0, estado: ArcState::Live, candidate: false, conditionally_protected: false, immortal: true, deferred: false, side_edges: false }, None);
    }

    /// Registra um objeto que já existe e passa a ser contado (o jovem
    /// promovido pela coleta menor, os vivos na ativação): `rc=0`, `Live`; o
    /// chamador conta as ocorrências que chegam a ele e depois chama
    /// [`EstadoDoArc::review`].
    pub fn register_live(&mut self, h: Ref) -> ArcId {
        self.register_live_at(h, None)
    }

    /// [`Self::register_live`] de um bloco do espaço, com a geometria da
    /// página dele: o metadado vai para a tabela por página, sem hash.
    ///
    /// ```
    /// use dartforge_runtime::arc::{EstadoDoArc, Geometry};
    /// let mut arc = EstadoDoArc::new();
    /// let first = 0x1_0000_0400;
    /// let h = first as i64 + dartforge_runtime::layout::DESLOCAMENTO_DO_HANDLE;
    /// arc.register_live_at(h, Some(Geometry { first, size: 24, blocks: 2 }));
    /// assert_eq!(arc.meta(h).unwrap().rc, 0);
    /// ```
    ///
    /// # Panics
    /// A geometria tem tamanho zero ou extensão que transborda u64; `h` não
    /// começa um bloco dela; a página mudou de geometria com objetos vivos;
    /// ou a geração monotônica se esgotou.
    pub fn register_live_at(&mut self, h: Ref, geometry: Option<Geometry>) -> ArcId {
        debug_assert!(e_handle(h));
        let generation = self.next_generation;
        self.next_generation = self.next_generation.checked_add(1).expect("ARC: geração esgotada");
        if self.trace_log {
            eprintln!("[arc-traco] registrar {h} ja={}", self.objects.contains_key(&h));
        }
        self.objects.insert(h, MetaArc { generation, rc: 0, estado: ArcState::Live, candidate: false, conditionally_protected: false, immortal: false, deferred: false, side_edges: false }, geometry);
        self.statistics.registered_set += 1;
        ArcId { handle: h, generation }
    }

    /// Põe `h` na fila de zeros se o RC dele é zero (o registrado sem
    /// ocorrências: só raízes o veem, ou ninguém).
    pub fn review(&mut self, h: Ref) {
        if let Some(m) = self.objects.get(&h)
            && m.rc == 0
            && m.estado == ArcState::Live
            && !m.immortal
        {
            self.zeros.push(ArcId { handle: h, generation: m.generation });
        }
    }

    /// O objeto `h` é contado e está vivo (nem morto nem em descarte).
    pub fn alive(&self, h: Ref) -> bool {
        self.objects.get(&h).is_some_and(|m| matches!(m.estado, ArcState::Live | ArcState::Building))
    }

    /// O objeto `h` morreu (logicamente) e espera o reclamador.
    pub fn dead(&self, h: Ref) -> bool {
        self.objects.get(&h).is_some_and(|m| m.estado == ArcState::Dead)
    }

    /// Os handles contados ainda vivos.
    pub fn live_set(&self) -> impl Iterator<Item = Ref> + '_ {
        self.objects.iter().filter(|(_, m)| matches!(m.estado, ArcState::Live | ArcState::Building)).map(|(h, _)| h)
    }

    /// Põe o vivo `h` entre os candidatos da próxima rodada de ciclos: o
    /// objeto que uma raiz observacional vê pode perder essa raiz sem
    /// decremento (as raízes não contam), e só a rodada o examina de novo.
    pub fn add_candidate(&mut self, h: Ref) {
        let without_edges = self.without_edges;
        if let Some(m) = self.objects.get_mut(&h)
            && !m.candidate
            && !m.immortal
            && m.estado == ArcState::Live
            && (m.side_edges || !without_edges.is_some_and(|f| f(h)))
        {
            m.candidate = true;
            self.candidates.push(ArcId { handle: h, generation: m.generation });
        }
    }

    /// Impede o filtro do corpo de excluir um objeto com arestas laterais.
    /// A indicação é conservadora até o próximo registro da identidade.
    pub(crate) fn mark_side_edges(&mut self, h: Ref) {
        if let Some(m) = self.objects.get_mut(&h) { m.side_edges = true; }
        self.add_candidate(h);
    }

    /// O jovem `h` sobreviveu à drenagem (tem metadados): o RC zero vai para
    /// a fila de zeros ([`Self::review`]) e, sem ser imortal, ele vira
    /// candidato a ciclo ([`Self::add_candidate`]) — numa consulta só. Devolve
    /// se `h` tinha metadados.
    pub fn decide_young(&mut self, h: Ref) -> bool {
        let without_edges = self.without_edges;
        let Some(m) = self.objects.get_mut(&h) else { return false };
        if m.immortal || m.estado != ArcState::Live {
            return true;
        }
        let id = ArcId { handle: h, generation: m.generation };
        if m.rc == 0 {
            self.zeros.push(id);
        }
        if !m.candidate && (m.side_edges || !without_edges.is_some_and(|f| f(h))) {
            m.candidate = true;
            self.candidates.push(id);
        }
        true
    }

    /// Quantos candidatos a ciclo esperam a próxima rodada.
    pub fn candidates(&self) -> usize {
        self.candidates.len()
    }

    /// A construção terminou: o owner de construção passa a ser o resultado.
    pub fn finish_construction(&mut self, h: Ref) {
        if let Some(m) = self.objects.get_mut(&h)
            && m.estado == ArcState::Building
        {
            m.estado = ArcState::Live;
        }
    }

    /// Uma ocorrência forte a mais de `h` (§19.3).
    pub fn retain_ref(&mut self, h: Ref) -> Result<(), ArcError> {
        self.retain_if_registered(h).map(|_| ())
    }

    // Retém numa única consulta, informando se o handle já foi resolvido.
    // false é só ausência de metadados, nunca um estado inválido/imortal;
    // o heap pode registrar o destino ausente antes de contar a ocorrência.
    pub(crate) fn retain_if_registered(&mut self, h: Ref) -> Result<bool, ArcError> {
        if !e_handle(h) {
            return Ok(true);
        }
        let Some(m) = self.objects.get_mut(&h) else { return Ok(false) };
        if m.immortal {
            return Ok(true);
        }
        if matches!(m.estado, ArcState::Collecting | ArcState::Dead) {
            return Err(ArcError::InvalidState(h, m.estado));
        }
        m.rc = m.rc.checked_add(1).ok_or(ArcError::Overflow(h))?;
        self.statistics.retains += 1;
        if self.trace_log {
            eprintln!("[arc-traco] reter {h} rc={}", m.rc);
        }
        Ok(true)
    }

    /// Uma ocorrência forte a menos de `h` (§19.3): zero vai para a fila de
    /// zeros; positivo vira candidato a ciclo.
    pub fn release(&mut self, h: Ref) -> Result<(), ArcError> {
        if !e_handle(h) {
            return Ok(());
        }
        let Some(m) = self.objects.get_mut(&h) else { return Ok(()) };
        if m.immortal {
            return Ok(());
        }
        if matches!(m.estado, ArcState::Collecting | ArcState::Dead) {
            return Err(ArcError::InvalidState(h, m.estado));
        }
        if m.rc == 0 {
            return Err(ArcError::RcZero(h));
        }
        m.rc -= 1;
        self.statistics.releases += 1;
        if self.trace_log {
            eprintln!("[arc-traco] soltar {h} rc={}", m.rc);
        }
        let id = ArcId { handle: h, generation: m.generation };
        if m.rc == 0 {
            self.zeros.push(id);
        } else if !m.candidate && (m.side_edges || !self.without_edges.is_some_and(|f| f(h))) {
            m.candidate = true;
            self.candidates.push(id);
        }
        Ok(())
    }

    /// Marca a clausura forte de `h` como protegida condicionalmente (§22.2):
    /// RC zero de um protegido não inicia o descarte.
    pub fn protect(&mut self, graph: &dyn ArcGraph, h: Ref) {
        let mut stack = vec![h];
        while let Some(x) = stack.pop() {
            let Some(m) = self.objects.get_mut(&x) else { continue };
            if m.conditionally_protected || m.immortal {
                continue;
            }
            m.conditionally_protected = true;
            graph.edges(x, &mut |v| {
                if e_handle(v) {
                    stack.push(v);
                }
            });
        }
    }

    /// Retira toda proteção condicional (o ponto fixo da rodada a refaz).
    pub fn clear_protections(&mut self) {
        for m in self.objects.values_mut() {
            m.conditionally_protected = false;
        }
    }

    /// Processa a fila de zeros (§19.3): cada objeto vivo, da geração da
    /// entrada, com RC ainda zero e sem proteção morre; os destinos das suas
    /// arestas perdem uma ocorrência, o que pode encadear mais zeros (fila
    /// iterativa, sem recursão). `limit` conta objetos descartados.
    ///
    /// `protected` são as raízes observacionais (§21.3): as ocorrências que o
    /// RC não conta (a pilha, os quadros do runtime, os globais). Um zero que
    /// elas veem não morre e fica na fila para a próxima drenagem (a tabela
    /// de contagem zero do RC adiado).
    pub fn drain_zeros(&mut self, graph: &mut dyn ArcGraph, limit: usize, protected: &dyn Fn(Ref) -> bool) -> Result<usize, ArcError> {
        let mut done = 0;
        while done < limit {
            let Some(id) = self.zeros.pop() else { break };
            self.statistics.seen_zeros += 1;
            let local = self.objects.locate(id.handle);
            let Some(m) = self.objects.in_local(local).filter(|m| m.generation == id.generation) else { continue };
            if m.estado == ArcState::Live && m.rc > 0 {
                // Desceu a zero e voltou a subir antes da drenagem: a
                // ocorrência nova pode fechar um ciclo sem raiz (o
                // autociclo `x[i] = x` depois de soltar a última de fora),
                // então ele é candidato como quem desce a um RC positivo.
                self.add_candidate(id.handle);
                continue;
            }
            if m.estado != ArcState::Live || m.rc != 0 || m.conditionally_protected || m.immortal {
                continue;
            }
            if protected(id.handle) {
                // Fica fora da fila até a próxima coleta: dentro desta as
                // raízes não mudam, e reexaminá-lo a cada passada (e cada
                // cópia dele, a cada vez que o RC voltou a zero) era o custo
                // maior da cascata.
                self.statistics.deferred_zeros += 1;
                if let Some(m) = self.objects.in_local_mut(local)
                    && !m.deferred
                {
                    m.deferred = true;
                    self.deferred_set.push(id);
                }
                continue;
            }
            self.discard_located(graph, &[id.handle], Some(local))?;
            self.statistics.dead_by_rc += 1;
            done += 1;
        }
        Ok(done)
    }

    /// Os adiados voltam à fila de zeros, uma vez por coleta (no começo dela:
    /// a raiz que os segurava pode ter sumido). O que não é mais válido, ou
    /// já não tem RC zero, sai.
    pub fn resume_deferred(&mut self) {
        let deferred_set = std::mem::take(&mut self.deferred_set);
        for id in deferred_set {
            if let Some(m) = self.objects.get_mut(&id.handle)
                && m.generation == id.generation
            {
                m.deferred = false;
                self.zeros.push(id);
            }
        }
    }

    /// Há zeros esperando.
    pub fn ha_zeros(&self) -> bool {
        !self.zeros.is_empty()
    }

    /// A transação de descarte do conjunto `d` (§19.4): marca todos
    /// `Collecting`; para cada ocorrência forte `u→v` com `u` em `d`, solta
    /// `v` se `v` não pertence a `d`; rompe as arestas; marca `Dead` e
    /// publica os mortos para o reclamador.
    pub fn discard(&mut self, graph: &mut dyn ArcGraph, d: &[Ref]) -> Result<(), ArcError> {
        self.discard_located(graph, d, None)
    }

    // O slot já validado pela geração vale durante esta transação: soltar
    // saídas não registra nem remove metadados. O descarte coletivo continua
    // resolvendo cada handle, sem reutilizar localização de outro objeto.
    fn discard_located(&mut self, graph: &mut dyn ArcGraph, d: &[Ref], local: Option<MetadataLocation>) -> Result<(), ArcError> {
        debug_assert!(local.is_none() || d.len() == 1);
        assert!(!self.transaction, "ARC: descarte reentrante");
        self.transaction = true;
        // A cascata dos zeros descarta um objeto por vez: aí a única
        // aresta interna é a do próprio objeto, o conjunto (uma alocação
        // por morte) não é montado, e as arestas são lidas e zeradas num
        // percurso só.
        let single = if d.len() == 1 { Some(d[0]) } else { None };
        let item_set: HashSet<Ref> = if single.is_some() { HashSet::default() } else { d.iter().copied().collect() };
        let internal = |v: Ref| match single {
            Some(u) => u == v,
            None => item_set.contains(&v),
        };
        for &u in d {
            let m = match local {
                Some(local) => self.objects.in_local_mut(local),
                None => self.objects.get_mut(&u),
            };
            if let Some(m) = m {
                m.estado = ArcState::Collecting;
            }
        }
        // As saídas do conjunto, uma vez cada ocorrência (o vetor é
        // reaproveitado entre descartes).
        let mut exits = std::mem::take(&mut self.exits);
        if let Some(u) = single {
            graph.take_edges(u, &mut |v| {
                if e_handle(v) && v != u {
                    exits.push(v);
                }
            });
        } else {
            for &u in d {
                graph.edges(u, &mut |v| {
                    if e_handle(v) && !internal(v) {
                        exits.push(v);
                    }
                });
            }
        }
        let mut error = None;
        for v in exits.drain(..) {
            if let Err(e) = self.release(v) {
                error.get_or_insert(e);
            }
        }
        self.exits = exits;
        for &u in d {
            if self.trace_log {
                eprintln!("[arc-traco] morrer {u}");
            }
            if single.is_none() {
                graph.break_off(u);
            }
            let m = match local {
                Some(local) => self.objects.in_local_mut(local),
                None => self.objects.get_mut(&u),
            };
            if let Some(m) = m {
                m.estado = ArcState::Dead;
                m.rc = 0;
                m.candidate = false;
            }
            self.dead_set.push(u);
        }
        self.transaction = false;
        match error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// A coleta de ciclos por *trial deletion* lateral (§22.1): a região `R`
    /// é a clausura forte dos candidatos válidos; `trial[v] = rc[v]` menos as
    /// ocorrências internas a `R`; os de trial positivo (e os imortais, em
    /// construção ou protegidos) sobrevivem e propagam sobrevivência; o resto
    /// é um conjunto morto, descartado em lote. Nenhum RC real muda antes do
    /// commit; um trial negativo aborta com diagnóstico. `protected`: as
    /// raízes observacionais, que fazem sobreviver como uma referência externa.
    pub fn collect_cycles(&mut self, graph: &mut dyn ArcGraph, protected: &dyn Fn(Ref) -> bool) -> Result<usize, ArcError> {
        let candidates = std::mem::take(&mut self.candidates);
        let mut seeds: Vec<Ref> = Vec::new();
        for id in candidates {
            let Some(m) = self.objects.get_mut(&id.handle).filter(|m| m.generation == id.generation) else { continue };
            m.candidate = false;
            if m.estado == ArcState::Live && m.rc > 0 && !m.immortal {
                if protected(id.handle) {
                    self.add_candidate(id.handle);
                } else {
                    seeds.push(id.handle);
                }
            }
        }
        if seeds.is_empty() {
            return Ok(0);
        }
        self.statistics.cycle_rounds += 1;
        // A raiz protegida fica fora de R: suas arestas continuam entradas
        // externas no trial. Reenfileirá-la permite coletar quando a raiz sumir.
        let mut r: Vec<Ref> = Vec::new();
        // Um índice denso por handle; contadores e marcas ficam em vetores,
        // sem três tabelas de hash distintas para a mesma região.
        let mut in_r: HashMap<Ref, usize> = HashMap::default();
        let mut stack = seeds;
        while let Some(x) = stack.pop() {
            let std::collections::hash_map::Entry::Vacant(input) = in_r.entry(x) else { continue };
            let Some(m) = self.objects.get(&x) else { continue };
            if m.immortal || m.estado == ArcState::Dead {
                continue;
            }
            if protected(x) {
                self.add_candidate(x);
                continue;
            }
            input.insert(r.len());
            r.push(x);
            graph.edges(x, &mut |v| {
                if e_handle(v) {
                    stack.push(v);
                }
            });
        }
        self.statistics.examined += r.len() as u64;
        // trial = rc − ocorrências internas.
        let mut trial: Vec<i128> = r.iter().map(|x| self.objects[x].rc as i128).collect();
        for &u in &r {
            graph.edges(u, &mut |v| {
                if let Some(&ix) = in_r.get(&v) {
                    trial[ix] -= 1;
                }
            });
        }
        if let Some(ix) = trial.iter().position(|&t| t < 0) {
            return Err(ArcError::NegativeTrial(r[ix]));
        }
        // Sobreviventes: trial positivo, em construção ou protegido; propagam.
        let mut live_set = vec![false; r.len()];
        let mut stack: Vec<usize> = Vec::new();
        for (ix, &x) in r.iter().enumerate() {
            let m = &self.objects[&x];
            if trial[ix] > 0 || m.estado == ArcState::Building || m.conditionally_protected || protected(x) {
                live_set[ix] = true;
                stack.push(ix);
            }
        }
        // Marca ao enfileirar: convergência de caminhos e posições repetidas
        // não multiplicam entradas na pilha. O trial acima continua contando
        // cada ocorrência. A propagação usa índices, sem buscar o handle de
        // novo ao retirar da pilha; destinos fora de R não são percorridos.
        while let Some(ix) = stack.pop() {
            graph.edges(r[ix], &mut |v| {
                if e_handle(v)
                    && let Some(&dest) = in_r.get(&v)
                    && !live_set[dest]
                {
                    live_set[dest] = true;
                    stack.push(dest);
                }
            });
        }
        // O sobrevivente que só a raiz observacional segura volta a ser
        // candidato: a raiz pode sumir sem decremento.
        for (ix, &x) in r.iter().enumerate() {
            if live_set[ix] && protected(x) {
                self.add_candidate(x);
            }
        }
        let dead_set: Vec<Ref> = r.into_iter().enumerate().filter_map(|(i, x)| (!live_set[i]).then_some(x)).collect();
        if dead_set.is_empty() {
            return Ok(0);
        }
        let n = dead_set.len();
        self.discard(graph, &dead_set)?;
        self.statistics.dead_by_cycle += n as u64;
        Ok(n)
    }

    /// Os mortos lógicos publicados desde a última chamada, para o reclamador
    /// físico; os metadados deles saem aqui (§19.4, passo 2).
    pub fn take_dead(&mut self) -> Vec<Ref> {
        let mut dead_set = Vec::new();
        self.transfer_dead(&mut dead_set);
        dead_set
    }

    /// Remove metadados e transfere mortos para o reclamador físico.
    /// Destino vazio recebe o buffer por troca, sem copiar cada handle;
    /// rodadas posteriores acrescentam seus mortos, preservando os anteriores.
    pub(crate) fn transfer_dead(&mut self, dest: &mut Vec<Ref>) -> usize {
        let amount = self.dead_set.len();
        for h in &self.dead_set {
            if self.objects.get(h).is_some_and(|m| m.estado == ArcState::Dead) {
                self.objects.remove(h);
            }
        }
        if dest.is_empty() {
            std::mem::swap(dest, &mut self.dead_set);
        } else {
            dest.append(&mut self.dead_set);
        }
        amount
    }

    /// Reconta, com o heap estabilizado, `owners de root + edges fortes` de
    /// cada objeto vivo e compara com o RC (§20.3, modo auditor). `roots`
    /// são as ocorrências proprietárias (não as observacionais, §21.3).
    pub fn audit(&self, graph: &dyn ArcGraph, roots: &[Ref]) -> Result<(), ArcError> {
        let mut count: HashMap<Ref, u64> = HashMap::default();
        for &h in roots {
            if e_handle(h) {
                *count.entry(h).or_insert(0) += 1;
            }
        }
        for (h, m) in self.objects.iter() {
            if m.estado == ArcState::Dead {
                continue;
            }
            graph.edges(h, &mut |v| {
                if e_handle(v) {
                    *count.entry(v).or_insert(0) += 1;
                }
            });
        }
        for (h, m) in self.objects.iter() {
            if m.immortal || m.estado == ArcState::Dead {
                continue;
            }
            let recounted = count.get(&h).copied().unwrap_or(0);
            if recounted != m.rc {
                return Err(ArcError::Audit { handle: h, rc: m.rc, recounted });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests_mod {
    use super::*;

    #[test]
    fn immortal_does_not_reuse_generation_after_exhaustion() {
        let mut arc = EstadoDoArc::new();
        arc.next_generation = u64::MAX - 1;
        arc.register_immortal(2);
        assert_eq!(arc.meta(2).unwrap().generation, u64::MAX - 1);
        let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            arc.register_immortal(4);
        }));
        assert!(error.is_err());
        assert_eq!(arc.next_generation, u64::MAX);
        assert!(arc.meta(4).is_none());
        assert_eq!(arc.registered_set(), 1);
    }

    #[test]
    fn index_by_inverse_checks_every_class_byte() {
        use crate::layout::{PAGE_HEAD, DESLOCAMENTO_DO_HANDLE, N_CLASSES, PAGINA, bytes_do_bloco, class_words};
        let first = (1u64 << 32) + PAGE_HEAD as u64;
        for class in 1..N_CLASSES {
            let size = bytes_do_bloco(class_words(class)) as u64;
            let blocks = ((PAGINA - PAGE_HEAD) as u64 / size) as u32;
            let page = ArcPage::new_area(Geometry { first, size, blocks });
            assert_eq!(page.ix(first as Ref + DESLOCAMENTO_DO_HANDLE - 1), None);
            for d in 0..=PAGINA as u64 {
                // Oráculo deliberadamente pela divisão original: todos os
                // inícios, interiores e bytes além do último bloco.
                let expected = (d % size == 0 && d / size < u64::from(blocks)).then_some((d / size) as usize);
                let h = (first + d) as Ref + DESLOCAMENTO_DO_HANDLE;
                assert_eq!(page.ix(h), expected, "classe {class}, deslocamento {d}");
            }
        }
    }

    #[test]
    fn index_by_inverse_checks_large_sizes_and_u64_bounds() {
        for size in [1, 2, 3, 24, (1u64 << 32) - 1, 1u64 << 32, (1u64 << 63) - 1, 1u64 << 63, u64::MAX] {
            let blocks = (u64::MAX / size).min(7) as u32;
            let page = ArcPage::new_area(Geometry { first: 0, size, blocks });
            let query = |d: u64| page.ix(d.wrapping_add(crate::layout::DESLOCAMENTO_DO_HANDLE as u64) as Ref);
            for i in 0..u64::from(blocks) {
                let d = i * size;
                assert_eq!(query(d), Some(i as usize), "tamanho {size}, índice {i}");
                if size > 1 {
                    assert_eq!(query(d + 1), None);
                }
            }
            let end = size * u64::from(blocks);
            for d in [end, end.saturating_add(1), u64::MAX] {
                assert_eq!(query(d), None, "tamanho {size}, deslocamento {d}");
            }
        }
    }

    #[test]
    fn reformatted_page_updates_inverse_and_preserves_metadata() {
        let first = (1u64 << 32) + 1024;
        let h = (first + 48) as Ref + crate::layout::DESLOCAMENTO_DO_HANDLE;
        let mut a = EstadoDoArc::new();
        a.register_live_at(h, Some(Geometry { first, size: 24, blocks: 4 }));
        a.review(h);
        a.drain_zeros(&mut Graph::default(), usize::MAX, &|_| false).unwrap();
        a.take_dead();
        let new = a.register_live_at(h, Some(Geometry { first, size: 48, blocks: 2 }));
        a.retain_ref(h).unwrap();
        assert_eq!(a.meta(h).unwrap().generation, new.generation);
        assert_eq!(a.meta(h).unwrap().rc, 1);
        assert!(a.meta(h + 24).is_none());
    }

    /// Um grafo de mentira: handle → lista de destinos (com multiplicidade).
    #[derive(Default)]
    struct Graph(HashMap<Ref, Vec<Ref>>);

    impl ArcGraph for Graph {
        fn edges(&self, h: Ref, f: &mut dyn FnMut(Ref)) {
            for &v in self.0.get(&h).map(|v| v.as_slice()).unwrap_or(&[]) {
                f(v);
            }
        }
        fn break_off(&mut self, h: Ref) {
            self.0.remove(&h);
        }
    }

    /// Aloca `h` (rc=1 do owner de construção) e o conclui.
    fn new(a: &mut EstadoDoArc, h: Ref) {
        a.register(h).unwrap();
        a.finish_construction(h);
    }

    /// `u.campo = v`: o destino ganha a ocorrência.
    fn enable(a: &mut EstadoDoArc, g: &mut Graph, u: Ref, v: Ref) {
        a.retain_ref(v).unwrap();
        g.0.entry(u).or_default().push(v);
    }

    fn dead(a: &EstadoDoArc, h: Ref) -> bool {
        a.meta(h).is_none_or(|m| m.estado == ArcState::Dead)
    }

    #[test]
    fn null_and_smi_are_not_counted() {
        let mut a = EstadoDoArc::new();
        a.retain_ref(0).unwrap();
        a.retain_ref(7).unwrap();
        a.release(7).unwrap();
        assert_eq!(a.statistics.retains, 0);
    }

    #[test]
    fn transferred_dead_preserve_cascade_and_rounds_without_copying_first_buffer() {
        let mut a = EstadoDoArc::new();
        let mut g = Graph::default();
        new(&mut a, 2);
        new(&mut a, 4);
        enable(&mut a, &mut g, 2, 4);
        a.release(4).unwrap();
        a.release(2).unwrap();
        assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap(), 2);
        let buffer = a.dead_set.as_ptr();
        let mut dest = Vec::new();
        assert_eq!(a.transfer_dead(&mut dest), 2);
        assert_eq!(dest, [2, 4]);
        assert_eq!(dest.as_ptr(), buffer);
        assert!(a.meta(2).is_none() && a.meta(4).is_none());
        new(&mut a, 6);
        a.release(6).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert_eq!(a.transfer_dead(&mut dest), 1);
        assert_eq!(dest, [2, 4, 6]);
        assert!(a.meta(6).is_none());
        assert_eq!(a.transfer_dead(&mut dest), 0);
        assert_eq!(dest, [2, 4, 6]);
        new(&mut a, 8);
        a.release(8).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert_eq!(a.take_dead(), [8]);
        assert!(a.meta(8).is_none());
    }

    #[test]
    fn side_edge_prevents_refless_body_filter() {
        let mut a = EstadoDoArc::new();
        a.without_edges = Some(|_| true);
        let mut g = Graph::default();
        a.register_live(2);
        a.register_live(4);
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 4, 2);
        a.retain_ref(2).unwrap();
        a.release(2).unwrap();
        assert_eq!(a.candidates(), 0);
        a.mark_side_edges(2);
        assert_eq!(a.candidates(), 1);
        a.collect_cycles(&mut g, &|_| false).unwrap();
        assert!(dead(&a, 2) && dead(&a, 4));
        a.register_live(2);
        assert!(!a.meta(2).unwrap().side_edges);
        assert_eq!(std::mem::size_of::<MetaArc>(), 24);
    }

    #[test]
    fn acyclic_chain_dies_in_cascade_when_root_is_released() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        for h in [2, 4, 6] {
            new(&mut a, h);
        }
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 4, 6);
        // Os owners de construção de 4 e 6 saem: só as arestas os mantêm.
        a.release(4).unwrap();
        a.release(6).unwrap();
        a.audit(&g, &[2]).unwrap();
        a.release(2).unwrap();
        assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap(), 3);
        assert!(dead(&a, 2) && dead(&a, 4) && dead(&a, 6));
        assert_eq!(a.take_dead().len(), 3);
        assert_eq!(a.registered_set(), 0);
    }

    #[test]
    fn occurrence_multiplicity() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        new(&mut a, 2);
        new(&mut a, 4);
        // Duas posições da lista 2 apontam para 4: contam duas vezes.
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 2, 4);
        a.release(4).unwrap();
        assert_eq!(a.meta(4).unwrap().rc, 2);
        a.audit(&g, &[2]).unwrap();
        a.release(2).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(dead(&a, 4));
    }

    #[test]
    fn isolated_cycle_dies_only_by_cycle_collection() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        new(&mut a, 2);
        new(&mut a, 4);
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 4, 2);
        a.release(4).unwrap();
        a.release(2).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(!dead(&a, 2) && !dead(&a, 4), "o RC sozinho não fecha o ciclo");
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 2);
        assert!(dead(&a, 2) && dead(&a, 4));
        assert_eq!(a.statistics.dead_by_cycle, 2);
    }

    #[test]
    fn retain_if_registered_tells_absence_without_hiding_errors() {
        let mut a = EstadoDoArc::new();
        for h in [0, 1, 3] { assert_eq!(a.retain_if_registered(h), Ok(true)); }
        assert_eq!(a.retain_if_registered(2), Ok(false));
        assert_eq!(a.retain_ref(2), Ok(()));
        assert_eq!(a.registered_set(), 0);
        assert_eq!(a.statistics.retains, 0);
        let id = a.register(2).unwrap();
        assert_eq!(a.retain_if_registered(2), Ok(true));
        assert_eq!(a.meta(2).unwrap().rc, 2);
        assert_eq!(a.meta(2).unwrap().generation, id.generation);
        a.finish_construction(2);
        a.objects.get_mut(&2).unwrap().rc = u64::MAX;
        assert_eq!(a.retain_if_registered(2), Err(ArcError::Overflow(2)));
        assert_eq!(a.meta(2).unwrap().rc, u64::MAX);
        assert_eq!(a.statistics.retains, 1);
        for estado in [ArcState::Collecting, ArcState::Dead] {
            a.objects.get_mut(&2).unwrap().estado = estado;
            assert_eq!(a.retain_if_registered(2), Err(ArcError::InvalidState(2, estado)));
            assert_eq!(a.statistics.retains, 1);
        }
        a.register_immortal(4);
        assert_eq!(a.retain_if_registered(4), Ok(true));
        assert_eq!(a.meta(4).unwrap().rc, 0);
        assert_eq!(a.statistics.retains, 1);
    }

    #[test]
    fn dense_survival_preserves_converging_paths_and_repeated_edges() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        let (x, y, z, w, root) = (2, 0x1_0002, 0x2_0002, 0x3_0002, 0x4_0002);
        for h in [x, y, z, w, root] { new(&mut a, h); }
        enable(&mut a, &mut g, root, x);
        enable(&mut a, &mut g, w, x);
        for _ in 0..96 {
            enable(&mut a, &mut g, x, y);
            enable(&mut a, &mut g, x, z);
            enable(&mut a, &mut g, y, w);
            enable(&mut a, &mut g, z, w);
        }
        for h in [x, y, z, w] { a.release(h).unwrap(); }
        let before = g.0.clone();
        let counts: Vec<_> = [x, y, z, w, root].map(|h| a.meta(h).unwrap().rc).into();
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 0);
        assert_eq!(g.0, before);
        assert_eq!([x, y, z, w, root].map(|h| a.meta(h).unwrap().rc).as_slice(), counts);
        a.audit(&g, &[root]).unwrap();
        // A mesma região, agora sem entrada externa, morre de uma vez;
        // todas as ocorrências internas são abatidas, sem soltar RC interno.
        a.release(root).unwrap();
        assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap(), 1);
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 4);
        for h in [x, y, z, w, root] { assert!(dead(&a, h)); }
        let dead_set = a.take_dead();
        assert_eq!(dead_set.len(), 5);
        assert_eq!(dead_set.into_iter().collect::<HashSet<_>>(), [x, y, z, w, root].into_iter().collect::<HashSet<_>>());
    }

    #[test]
    fn dense_trial_preserves_multiplicity_and_external_entry_in_sparse_handles() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        let (x, y, root) = (2, 0x1_0002, 0x4_0002);
        for h in [x, y, root] { new(&mut a, h); }
        enable(&mut a, &mut g, x, y);
        enable(&mut a, &mut g, x, y);
        enable(&mut a, &mut g, y, x);
        enable(&mut a, &mut g, root, y);
        a.release(x).unwrap();
        a.release(y).unwrap();
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 0);
        assert_eq!((a.meta(x).unwrap().rc, a.meta(y).unwrap().rc), (1, 3));
        a.release(root).unwrap();
        assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap(), 1);
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 2);
        let dead_set = a.take_dead();
        assert_eq!(dead_set.len(), 3);
        assert!([x, y, root].iter().all(|h| dead_set.contains(h)));
    }

    #[test]
    fn dense_negative_trial_identifies_handle_and_aborts_before_discard() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        let (x, y) = (2, 0x1_0002);
        new(&mut a, x);
        new(&mut a, y);
        // Inventário adulterado: duas arestas internas, mas RC de y é um.
        g.0.insert(x, vec![y, y]);
        g.0.insert(y, vec![x]);
        a.add_candidate(x);
        let before = g.0.clone();
        assert_eq!(a.collect_cycles(&mut g, &|_| false), Err(ArcError::NegativeTrial(y)));
        assert_eq!(g.0, before);
        for h in [x, y] {
            assert_eq!(a.meta(h).unwrap().rc, 1);
            assert_eq!(a.meta(h).unwrap().estado, ArcState::Live);
        }
        assert!(a.take_dead().is_empty());
        // Corrigir o inventário e reenfileirar não pode herdar índices/trial
        // do diagnóstico anterior nem aplicar um descarte parcial dele.
        g.0.insert(x, vec![y]);
        a.add_candidate(x);
        assert_eq!(a.collect_cycles(&mut g, &|_| false), Ok(2));
        let dead_set = a.take_dead();
        assert_eq!(dead_set.len(), 2);
        assert!([x, y].iter().all(|h| dead_set.contains(h)));
    }

    #[test]
    fn successive_cycles_inherit_neither_survivors_nor_reclaimed_generations() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        let (x, y) = (2, 0x1_0002);
        for h in [x, y] { new(&mut a, h); }
        enable(&mut a, &mut g, x, y);
        enable(&mut a, &mut g, y, x);
        a.release(y).unwrap();
        a.add_candidate(x);
        let generation = a.meta(x).unwrap().generation;
        // O owner externo de x preserva os dois nós; na rodada seguinte
        // nenhum deles pode herdar a marca de sobrevivente.
        assert_eq!(a.collect_cycles(&mut g, &|_| false), Ok(0));
        assert_eq!((a.meta(x).unwrap().rc, a.meta(y).unwrap().rc), (2, 1));
        a.release(x).unwrap();
        assert_eq!(a.collect_cycles(&mut g, &|_| false), Ok(2));
        assert_eq!(a.take_dead().len(), 2);
        assert_eq!(a.collect_cycles(&mut g, &|_| false), Ok(0));
        // Mesmo endereço, região menor e geração nova: o mapa e os vetores
        // antigos não podem impedir a descoberta ou proteger o autociclo.
        new(&mut a, x);
        assert!(a.meta(x).unwrap().generation > generation);
        enable(&mut a, &mut g, x, x);
        a.release(x).unwrap();
        assert_eq!(a.collect_cycles(&mut g, &|_| false), Ok(1));
        assert_eq!(a.take_dead(), vec![x]);
        assert_eq!(a.registered_set(), 0);
    }

    #[test]
    fn cycle_with_external_reference_survives_and_collection_keeps_rc() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        for h in [2, 4, 6] {
            new(&mut a, h);
        }
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 4, 2);
        // 6 (raiz) aponta para 4.
        enable(&mut a, &mut g, 6, 4);
        a.release(4).unwrap();
        a.release(2).unwrap();
        let before = (a.meta(2).unwrap().rc, a.meta(4).unwrap().rc);
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 0);
        assert_eq!((a.meta(2).unwrap().rc, a.meta(4).unwrap().rc), before);
        a.audit(&g, &[6]).unwrap();
        // Sem a raiz, o ciclo morre na próxima rodada; 6 morre pelo RC.
        a.release(6).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(dead(&a, 6));
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 2);
    }

    #[test]
    fn cycle_holding_an_acyclic_releases_both() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        for h in [2, 4, 8] {
            new(&mut a, h);
        }
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 4, 2);
        enable(&mut a, &mut g, 4, 8);
        for h in [2, 4, 8] {
            a.release(h).unwrap();
        }
        a.collect_cycles(&mut g, &|_| false).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(dead(&a, 2) && dead(&a, 4) && dead(&a, 8));
    }

    #[test]
    fn under_construction_and_protected_do_not_die() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        a.register(2).unwrap();
        // Em construção: RC zero não descarta.
        a.release(2).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(!dead(&a, 2));
        new(&mut a, 4);
        a.protect(&g, 4);
        a.release(4).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(!dead(&a, 4), "protegido condicionalmente espera o ponto fixo");
    }

    #[test]
    fn protected_root_does_not_expand_trial_and_keeps_candidate() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        for h in [2, 4, 6, 8] { new(&mut a, h); }
        for (de, to) in [(2, 4), (4, 2), (4, 6), (6, 8), (8, 6)] {
            enable(&mut a, &mut g, de, to);
        }
        for h in [2, 4, 6, 8] { a.release(h).unwrap(); }
        let root = |h| h == 2;
        assert_eq!(a.collect_cycles(&mut g, &root).unwrap(), 0);
        a.audit(&g, &[]).unwrap();
        let examined = a.statistics.examined;
        assert_eq!(a.collect_cycles(&mut g, &root).unwrap(), 0);
        assert_eq!(a.statistics.examined, examined);
        assert_eq!(a.collect_cycles(&mut g, &|_| false).unwrap(), 4);
        for h in [2, 4, 6, 8] { assert!(dead(&a, h)); }
    }

    #[test]
    fn observational_root_holds_zero_and_cycle() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        new(&mut a, 2);
        new(&mut a, 4);
        enable(&mut a, &mut g, 2, 4);
        enable(&mut a, &mut g, 4, 2);
        new(&mut a, 6);
        a.release(2).unwrap();
        a.release(4).unwrap();
        a.release(6).unwrap();
        // 6 só é visto pela pilha; o ciclo 2-4 também.
        let root = |h: Ref| h == 6 || h == 2;
        a.drain_zeros(&mut g, usize::MAX, &root).unwrap();
        assert!(!dead(&a, 6));
        assert_eq!(a.collect_cycles(&mut g, &root).unwrap(), 0);
        // A pilha soltou: morrem na próxima drenagem (que começa retomando
        // os adiados) e rodada.
        a.resume_deferred();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(dead(&a, 6));
        let n = {
            a.candidates.push(ArcId { handle: 2, generation: a.meta(2).unwrap().generation });
            a.collect_cycles(&mut g, &|_| false).unwrap()
        };
        assert_eq!(n, 2);
    }

    #[test]
    fn release_with_zero_rc_is_an_error_and_overflow_too() {
        let mut a = EstadoDoArc::new();
        new(&mut a, 2);
        a.release(2).unwrap();
        assert_eq!(a.release(2), Err(ArcError::RcZero(2)));
    }

    #[test]
    fn stale_queue_entry_is_ignored_by_generation() {
        let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
        new(&mut a, 2);
        a.release(2).unwrap();
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        a.take_dead();
        // O endereço é reaproveitado: geração nova, a entrada velha não vale.
        new(&mut a, 2);
        a.zeros.push(ArcId { handle: 2, generation: 1 });
        a.drain_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(!dead(&a, 2));
    }

    #[test]
    fn audit_finds_wrong_rc() {
        let (mut a, g) = (EstadoDoArc::new(), Graph::default());
        new(&mut a, 2);
        a.retain_ref(2).unwrap();
        assert!(matches!(a.audit(&g, &[2]), Err(ArcError::Audit { handle: 2, rc: 2, recounted: 1 })));
    }

    #[test]
    fn bounded_cascade_preserves_root_multiplicity_and_generation_in_reformatted_page() {
        use crate::layout::DESLOCAMENTO_DO_HANDLE;
        for paged in [false, true] {
            let first = (1u64 << 32) + 1024;
            let h1 = first as Ref + DESLOCAMENTO_DO_HANDLE;
            let h2 = h1 + 24;
            let h3 = 2; // Reserva por hash, mesmo quando os outros usam página.
            let geometry = paged.then_some(Geometry { first, size: 24, blocks: 4 });
            let (mut a, mut g) = (EstadoDoArc::new(), Graph::default());
            let id1 = a.register_live_at(h1, geometry);
            let id2 = a.register_live_at(h2, geometry);
            a.register_live(h3);
            for h in [h1, h2, h3] { a.retain_ref(h).unwrap(); }
            for (de, to) in [(h1, h2), (h1, h2), (h1, h3), (h2, h3)] {
                enable(&mut a, &mut g, de, to);
            }
            for h in [h2, h3, h1] { a.release(h).unwrap(); }
            let root = |h| h == h2;
            assert_eq!(a.drain_zeros(&mut g, 1, &root), Ok(1));
            assert_eq!(a.take_dead(), vec![h1]);
            assert_eq!(a.meta(h2).unwrap().rc, 0);
            assert_eq!(a.meta(h3).unwrap().rc, 1);
            assert_eq!(a.drain_zeros(&mut g, usize::MAX, &root), Ok(0));
            assert!(a.alive(h2) && a.alive(h3));
            a.audit(&g, &[]).unwrap();
            a.resume_deferred();
            assert_eq!(a.drain_zeros(&mut g, 1, &|_| false), Ok(1));
            assert!(a.alive(h3));
            assert_eq!(a.meta(h3).unwrap().rc, 0);
            assert_eq!(a.drain_zeros(&mut g, 1, &|_| false), Ok(1));
            assert_eq!(a.take_dead(), vec![h2, h3]);
            assert_eq!(a.registered_set(), 0);
            // Todos os slots da página morreram; outro tamanho pode ocupar
            // o mesmo endereço. Nenhuma localização atravessa esta fronteira.
            let new_area = paged.then_some(Geometry { first, size: 48, blocks: 2 });
            let current = a.register_live_at(h1, new_area);
            a.retain_ref(h1).unwrap();
            a.zeros.extend([id1, id2]);
            assert_eq!(a.drain_zeros(&mut g, usize::MAX, &|_| false), Ok(0));
            let m = a.meta(h1).unwrap();
            assert_eq!((m.generation, m.rc, m.candidate), (current.generation, 1, false));
            a.release(h1).unwrap();
            assert_eq!(a.drain_zeros(&mut g, 1, &|_| false), Ok(1));
            assert_eq!(a.take_dead(), vec![h1]);
            a.audit(&g, &[]).unwrap();
        }
    }
}
