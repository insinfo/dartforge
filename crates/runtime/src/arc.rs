//! O núcleo do ARC com coleta de ciclos (docs/ARC-CICLOS-ESPECIFICACAO.md §19,
//! §22): os metadados laterais por objeto, o `reter`/`soltar` de uma
//! ocorrência forte, a fila de zeros com o descarte em cascata e a coleta de
//! ciclos por *trial deletion* lateral, sem alterar o RC real (§22.1).
//!
//! O módulo não conhece o layout dos blocos: as arestas de cada objeto vêm de
//! um [`GrafoArc`] (no heap, a enumeração pela forma do corpo; nos testes, um
//! grafo de mentira). Nenhuma operação daqui aloca objeto Dart, lança exceção
//! Dart ou chama código Dart (§19.3).
//!
//! Contratos:
//! * o contador é por **ocorrência** forte (§18.1, decisão 2): duas posições
//!   de uma lista apontando para `x` contam duas vezes;
//! * `null` (0) e `Smi` (ímpar) não são gerenciados; um handle sem metadados
//!   (objeto da imagem, permanente) é tratado como imortal;
//! * a geração é monotônica por registro; as filas guardam [`IdArc`] e
//!   descartam a entrada cuja geração não bate (§19.1);
//! * a coleta de ciclos roda com o mutador parado e sem reentrada (§18.1,
//!   decisão 5): o chamador garante isso.

use crate::hash::{HashMap, HashSet};

/// Um valor em posição de referência (o `Ref` de `layout`).
pub type Ref = i64;

/// O ciclo de vida lógico de um objeto (§19.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstadoArc {
    /// Alocado, campos ainda sendo inicializados: não morre por RC zero.
    Construindo,
    Vivo,
    /// Dentro da transação de descarte de um conjunto.
    Coletando,
    /// Logicamente morto: aguarda o reclamador físico.
    Morto,
}

/// Os metadados laterais de um objeto gerenciado (§19.1).
#[derive(Clone, Copy, Debug)]
pub struct MetaArc {
    pub geracao: u64,
    /// Owners mais ocorrências fortes; não inclui weak.
    pub rc: u64,
    pub estado: EstadoArc,
    /// Está (ou esteve) na fila de candidatos a ciclo desde a última rodada.
    pub candidato: bool,
    /// Valor de ephemeron (ou clausura dele) protegido entre rodadas (§22.2):
    /// RC zero não inicia o descarte.
    pub protegido_condicional: bool,
    pub imortal: bool,
    /// Está na lista dos adiados (RC zero com raiz), uma vez só.
    pub adiado: bool,
}

/// Identidade de um objeto nas filas: o handle e a geração do registro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdArc {
    pub handle: Ref,
    pub geracao: u64,
}

/// As arestas fortes dos objetos e o rompimento delas na morte (§19.2).
pub trait GrafoArc {
    /// Cada ocorrência forte do objeto `h`, com multiplicidade.
    fn arestas(&self, h: Ref, f: &mut dyn FnMut(Ref));
    /// Zera as ocorrências fortes de `h` (o descarte já debitou os destinos).
    fn romper(&mut self, h: Ref);
    /// [`Self::arestas`] e [`Self::romper`] num percurso só: cada ocorrência
    /// forte de `h`, que sai zerada (o descarte de um objeto só, a cascata).
    fn tirar_arestas(&mut self, h: Ref, f: &mut dyn FnMut(Ref)) {
        self.arestas(h, f);
        self.romper(h);
    }
}

/// Violação interna do contador (não é erro Dart): diagnóstico fatal (§19.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroArc {
    /// `soltar` com RC já zero.
    RcZero(Ref),
    /// Incremento além de `u64::MAX`.
    Overflow(Ref),
    /// Operação num objeto `Coletando`/`Morto`.
    EstadoInvalido(Ref, EstadoArc),
    /// Registro de um handle ainda vivo.
    JaRegistrado(Ref),
    /// O trial deletion achou mais arestas internas que o RC (inventário ou
    /// contagem errados): a coleta é abortada sem mudar nada (§22.1).
    TrialNegativo(Ref),
    /// A auditoria recontou um RC diferente.
    Auditoria { handle: Ref, rc: u64, recontado: u64 },
}

/// Contadores de trabalho (§22.5, §26.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EstatisticasArc {
    pub registrados: u64,
    pub retains: u64,
    pub releases: u64,
    /// Mortos pelo RC zero (cascata acíclica).
    pub mortos_por_rc: u64,
    /// Mortos pela coleta de ciclos.
    pub mortos_por_ciclo: u64,
    pub rodadas_de_ciclo: u64,
    /// Objetos na região `R` de todas as rodadas.
    pub examinados: u64,
    /// Entradas tiradas da fila de zeros e, delas, as adiadas (RC zero com
    /// raiz): o diagnóstico do custo da cascata.
    pub zeros_vistos: u64,
    pub zeros_adiados: u64,
}

/// `null` e `Smi` não são referências gerenciadas.
#[inline]
pub fn e_handle(h: Ref) -> bool {
    h != 0 && h & 1 == 0
}

/// A geometria da página de um bloco do espaço, que o heap informa no
/// registro: o endereço do primeiro bloco, os bytes de cada bloco e quantos
/// cabem na página. O tamanho é positivo e `tamanho * blocos` cabe em u64:
/// a extensão descreve memória contígua, sem volta no espaço de endereços.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometria {
    pub primeiro: u64,
    pub tamanho: u64,
    pub blocos: u32,
}

/// Os metadados dos blocos de uma página, pelo índice do bloco.
struct PaginaArc {
    geometria: Geometria,
    /// Inverso da parte ímpar do tamanho, módulo 2⁶⁴, e a potência de dois
    /// retirada: a divisão exata vira multiplicação e rotação.
    inverso: u64,
    deslocamento: u32,
    metas: Vec<Option<MetaArc>>,
    ocupados: u32,
}

impl PaginaArc {
    /// Prepara uma vez a divisão exata pelo tamanho dos blocos. A extensão
    /// precisa caber em u64: com isso, um índice menor que `blocos` não pode
    /// representar um produto que deu a volta no espaço de endereços.
    fn fatores(g: Geometria) -> (u64, u32) {
        assert!(g.tamanho != 0 && g.tamanho.checked_mul(u64::from(g.blocos)).is_some(), "bug do ARC: geometria inválida");
        let deslocamento = g.tamanho.trailing_zeros();
        let impar = g.tamanho >> deslocamento;
        let mut inverso = impar;
        // Newton: cada passo dobra os bits corretos do inverso ímpar.
        for _ in 0..6 {
            inverso = inverso.wrapping_mul(2u64.wrapping_sub(impar.wrapping_mul(inverso)));
        }
        (inverso, deslocamento)
    }

    fn nova(g: Geometria) -> Self {
        let (inverso, deslocamento) = Self::fatores(g);
        Self { geometria: g, inverso, deslocamento, metas: (0..g.blocos).map(|_| None).collect(), ocupados: 0 }
    }

    /// Índice de um início de bloco; a rotação leva os bits que não são
    /// divisíveis pela potência de dois para o alto, fora de `blocos`.
    /// O produto pelo inverso recusa igualmente os múltiplos inexatos da
    /// parte ímpar. Não há divisão nem resto no caminho de consulta.
    #[inline]
    fn indice(&self, h: Ref) -> Option<usize> {
        let d = (h.wrapping_sub(crate::layout::DESLOCAMENTO_DO_HANDLE) as u64).checked_sub(self.geometria.primeiro)?;
        let i = d.wrapping_mul(self.inverso).rotate_right(self.deslocamento);
        (i < u64::from(self.geometria.blocos)).then_some(i as usize)
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
struct Tabela {
    regioes: Vec<(u64, Box<[u32]>)>,
    paginas: Vec<PaginaArc>,
    avulsos: HashMap<Ref, MetaArc>,
    n: usize,
}

/// `PAGINA` = 64 KiB; uma região do mapa radix cobre 2¹⁶ páginas.
const BITS_DA_PAGINA: u32 = 16;
const BITS_DA_REGIAO: u32 = 16;

impl Tabela {
    #[inline]
    fn chave(h: Ref) -> (u64, usize) {
        let k = (h as u64) >> BITS_DA_PAGINA;
        (k >> BITS_DA_REGIAO, (k & ((1 << BITS_DA_REGIAO) - 1)) as usize)
    }

    /// A página registrada de `h`, se há.
    #[inline]
    fn pagina(&self, h: Ref) -> Option<usize> {
        let (r, i) = Self::chave(h);
        let (_, t) = self.regioes.iter().find(|(x, _)| *x == r)?;
        let v = t[i];
        (v != 0).then(|| v as usize - 1)
    }

    /// (página, índice) do metadado de `h` numa página registrada.
    #[inline]
    fn lugar(&self, h: Ref) -> Option<(usize, usize)> {
        let p = self.pagina(h)?;
        Some((p, self.paginas[p].indice(h)?))
    }

    fn get(&self, h: &Ref) -> Option<&MetaArc> {
        match self.lugar(*h) {
            Some((p, i)) => self.paginas[p].metas[i].as_ref(),
            None if self.avulsos.is_empty() => None,
            None => self.avulsos.get(h),
        }
    }

    fn get_mut(&mut self, h: &Ref) -> Option<&mut MetaArc> {
        match self.lugar(*h) {
            Some((p, i)) => self.paginas[p].metas[i].as_mut(),
            None if self.avulsos.is_empty() => None,
            None => self.avulsos.get_mut(h),
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
    fn insert(&mut self, h: Ref, m: MetaArc, geometria: Option<Geometria>) {
        let Some(g) = geometria else {
            if self.avulsos.insert(h, m).is_none() {
                self.n += 1;
            }
            return;
        };
        let p = match self.pagina(h) {
            Some(p) => p,
            None => self.nova_pagina(h, g),
        };
        let pg = &mut self.paginas[p];
        if pg.geometria != g {
            assert!(pg.ocupados == 0, "bug do ARC: página reformatada com {} metadados vivos", pg.ocupados);
            pg.geometria = g;
            (pg.inverso, pg.deslocamento) = PaginaArc::fatores(g);
            pg.metas.clear();
            pg.metas.resize_with(g.blocos as usize, || None);
        }
        let i = pg.indice(h).expect("bug do ARC: handle fora da geometria da página");
        if pg.metas[i].replace(m).is_none() {
            pg.ocupados += 1;
            self.n += 1;
        }
    }

    fn nova_pagina(&mut self, h: Ref, g: Geometria) -> usize {
        let (r, i) = Self::chave(h);
        let k = match self.regioes.iter().position(|(x, _)| *x == r) {
            Some(k) => k,
            None => {
                self.regioes.push((r, vec![0u32; 1 << BITS_DA_REGIAO].into_boxed_slice()));
                self.regioes.len() - 1
            }
        };
        self.paginas.push(PaginaArc::nova(g));
        let p = self.paginas.len() - 1;
        self.regioes[k].1[i] = u32::try_from(p + 1).expect("ARC: páginas demais");
        p
    }

    fn remove(&mut self, h: &Ref) -> Option<MetaArc> {
        let m = match self.lugar(*h) {
            Some((p, i)) => {
                let m = self.paginas[p].metas[i].take();
                if m.is_some() {
                    self.paginas[p].ocupados -= 1;
                }
                m
            }
            None => self.avulsos.remove(h),
        };
        if m.is_some() {
            self.n -= 1;
        }
        m
    }

    /// Cada handle registrado com o metadado.
    fn iter(&self) -> impl Iterator<Item = (Ref, &MetaArc)> + '_ {
        self.paginas
            .iter()
            .flat_map(|pg| {
                let g = pg.geometria;
                pg.metas.iter().enumerate().filter_map(move |(i, m)| {
                    m.as_ref().map(|m| ((g.primeiro + i as u64 * g.tamanho) as Ref + crate::layout::DESLOCAMENTO_DO_HANDLE, m))
                })
            })
            .chain(self.avulsos.iter().map(|(&h, m)| (h, m)))
    }

    fn values_mut(&mut self) -> impl Iterator<Item = &mut MetaArc> + '_ {
        self.paginas.iter_mut().flat_map(|pg| pg.metas.iter_mut().flatten()).chain(self.avulsos.values_mut())
    }
}

impl std::ops::Index<&Ref> for Tabela {
    type Output = MetaArc;
    fn index(&self, h: &Ref) -> &MetaArc {
        self.get(h).expect("bug do ARC: handle sem metadado")
    }
}

/// O estado do ARC de um isolate (§19.1).
#[derive(Default)]
pub struct EstadoDoArc {
    objetos: Tabela,
    candidatos: Vec<IdArc>,
    zeros: Vec<IdArc>,
    /// Os de RC zero que uma raiz segurava na última drenagem dos zeros, uma
    /// entrada por objeto ([`MetaArc::adiado`]); voltam à fila uma vez por
    /// coleta ([`Self::retomar_adiados`]), quando as raízes podem ter mudado.
    adiados: Vec<IdArc>,
    mortos: Vec<Ref>,
    proxima_geracao: u64,
    /// Uma transação de descarte está em curso (não reentrar).
    transacao: bool,
    /// As saídas do conjunto em descarte, reaproveitado ([`Self::descartar`]).
    saidas: Vec<Ref>,
    pub estatisticas: EstatisticasArc,
    /// Uma linha no stderr por evento (registro, retain, release, morte): o
    /// diagnóstico de `DARTFORGE_ARC_TRACO=1`.
    pub traco: bool,
    /// O objeto não tem posição de referência nenhuma (no heap, o corpo
    /// `BRUTO`: string, caixa numérica, dados tipados)? Sem aresta de saída
    /// ele nunca está num ciclo, e não entra entre os candidatos (o filtro
    /// mínimo do §32). `None` (os testes do núcleo): todo objeto pode.
    pub sem_arestas: Option<fn(Ref) -> bool>,
}

impl EstadoDoArc {
    pub fn novo() -> Self {
        EstadoDoArc { proxima_geracao: 1, ..Default::default() }
    }

    /// Os metadados de `h`, se gerenciado.
    pub fn meta(&self, h: Ref) -> Option<&MetaArc> {
        self.objetos.get(&h)
    }

    /// Quantos objetos têm metadados (vivos, em construção ou mortos ainda
    /// não reclamados).
    pub fn registrados(&self) -> usize {
        self.objetos.len()
    }

    /// Registra o bloco recém-reservado `h` (§19.3): `rc=1` (o owner de
    /// construção), estado `Construindo`, geração nova. Falha antes de o
    /// handle ser publicado.
    pub fn registrar(&mut self, h: Ref) -> Result<IdArc, ErroArc> {
        debug_assert!(e_handle(h));
        if let Some(m) = self.objetos.get(&h)
            && m.estado != EstadoArc::Morto
        {
            return Err(ErroArc::JaRegistrado(h));
        }
        let geracao = self.proxima_geracao;
        // Geração monotônica; overflow é falha interna, nunca reuso (§19.1).
        self.proxima_geracao = self.proxima_geracao.checked_add(1).expect("ARC: geração esgotada");
        self.objetos.insert(
            h,
            MetaArc { geracao, rc: 1, estado: EstadoArc::Construindo, candidato: false, protegido_condicional: false, imortal: false, adiado: false },
            None,
        );
        self.estatisticas.registrados += 1;
        Ok(IdArc { handle: h, geracao })
    }

    /// Registra um objeto da imagem comprovadamente vivo pela imagem inteira
    /// (§19.1): não morre nem entra em filas; suas arestas continuam contadas.
    ///
    /// # Panics
    /// A geração monotônica se esgotou; nenhum registro é publicado nesse caso.
    ///
    /// ```
    /// use dartforge_runtime::arc::EstadoDoArc;
    /// let mut arc = EstadoDoArc::novo();
    /// arc.registrar_imortal(2);
    /// assert!(arc.meta(2).unwrap().imortal);
    /// ```
    pub fn registrar_imortal(&mut self, h: Ref) {
        let geracao = self.proxima_geracao;
        self.proxima_geracao = self.proxima_geracao.checked_add(1).expect("ARC: geração esgotada");
        self.objetos.insert(h, MetaArc { geracao, rc: 0, estado: EstadoArc::Vivo, candidato: false, protegido_condicional: false, imortal: true, adiado: false }, None);
    }

    /// Registra um objeto que já existe e passa a ser contado (o jovem
    /// promovido pela coleta menor, os vivos na ativação): `rc=0`, `Vivo`; o
    /// chamador conta as ocorrências que chegam a ele e depois chama
    /// [`EstadoDoArc::revisar`].
    pub fn registrar_vivo(&mut self, h: Ref) -> IdArc {
        self.registrar_vivo_em(h, None)
    }

    /// [`Self::registrar_vivo`] de um bloco do espaço, com a geometria da
    /// página dele: o metadado vai para a tabela por página, sem hash.
    ///
    /// ```
    /// use dartforge_runtime::arc::{EstadoDoArc, Geometria};
    /// let mut arc = EstadoDoArc::novo();
    /// let primeiro = 0x1_0000_0400;
    /// let h = primeiro as i64 + dartforge_runtime::layout::DESLOCAMENTO_DO_HANDLE;
    /// arc.registrar_vivo_em(h, Some(Geometria { primeiro, tamanho: 24, blocos: 2 }));
    /// assert_eq!(arc.meta(h).unwrap().rc, 0);
    /// ```
    ///
    /// # Panics
    /// A geometria tem tamanho zero ou extensão que transborda u64; `h` não
    /// começa um bloco dela; a página mudou de geometria com objetos vivos;
    /// ou a geração monotônica se esgotou.
    pub fn registrar_vivo_em(&mut self, h: Ref, geometria: Option<Geometria>) -> IdArc {
        debug_assert!(e_handle(h));
        let geracao = self.proxima_geracao;
        self.proxima_geracao = self.proxima_geracao.checked_add(1).expect("ARC: geração esgotada");
        if self.traco {
            eprintln!("[arc-traco] registrar {h} ja={}", self.objetos.contains_key(&h));
        }
        self.objetos.insert(h, MetaArc { geracao, rc: 0, estado: EstadoArc::Vivo, candidato: false, protegido_condicional: false, imortal: false, adiado: false }, geometria);
        self.estatisticas.registrados += 1;
        IdArc { handle: h, geracao }
    }

    /// Põe `h` na fila de zeros se o RC dele é zero (o registrado sem
    /// ocorrências: só raízes o veem, ou ninguém).
    pub fn revisar(&mut self, h: Ref) {
        if let Some(m) = self.objetos.get(&h)
            && m.rc == 0
            && m.estado == EstadoArc::Vivo
            && !m.imortal
        {
            self.zeros.push(IdArc { handle: h, geracao: m.geracao });
        }
    }

    /// O objeto `h` é contado e está vivo (nem morto nem em descarte).
    pub fn vivo(&self, h: Ref) -> bool {
        self.objetos.get(&h).is_some_and(|m| matches!(m.estado, EstadoArc::Vivo | EstadoArc::Construindo))
    }

    /// O objeto `h` morreu (logicamente) e espera o reclamador.
    pub fn morto(&self, h: Ref) -> bool {
        self.objetos.get(&h).is_some_and(|m| m.estado == EstadoArc::Morto)
    }

    /// Os handles contados ainda vivos.
    pub fn vivos(&self) -> impl Iterator<Item = Ref> + '_ {
        self.objetos.iter().filter(|(_, m)| matches!(m.estado, EstadoArc::Vivo | EstadoArc::Construindo)).map(|(h, _)| h)
    }

    /// Põe o vivo `h` entre os candidatos da próxima rodada de ciclos: o
    /// objeto que uma raiz observacional vê pode perder essa raiz sem
    /// decremento (as raízes não contam), e só a rodada o examina de novo.
    pub fn candidatar(&mut self, h: Ref) {
        let sem_arestas = self.sem_arestas;
        if let Some(m) = self.objetos.get_mut(&h)
            && !m.candidato
            && !m.imortal
            && m.estado == EstadoArc::Vivo
            && !sem_arestas.is_some_and(|f| f(h))
        {
            m.candidato = true;
            self.candidatos.push(IdArc { handle: h, geracao: m.geracao });
        }
    }

    /// O jovem `h` sobreviveu à drenagem (tem metadados): o RC zero vai para
    /// a fila de zeros ([`Self::revisar`]) e, sem ser imortal, ele vira
    /// candidato a ciclo ([`Self::candidatar`]) — numa consulta só. Devolve
    /// se `h` tinha metadados.
    pub fn decidir_jovem(&mut self, h: Ref) -> bool {
        let sem_arestas = self.sem_arestas;
        let Some(m) = self.objetos.get_mut(&h) else { return false };
        if m.imortal || m.estado != EstadoArc::Vivo {
            return true;
        }
        let id = IdArc { handle: h, geracao: m.geracao };
        if m.rc == 0 {
            self.zeros.push(id);
        }
        if !m.candidato && !sem_arestas.is_some_and(|f| f(h)) {
            m.candidato = true;
            self.candidatos.push(id);
        }
        true
    }

    /// Quantos candidatos a ciclo esperam a próxima rodada.
    pub fn candidatos(&self) -> usize {
        self.candidatos.len()
    }

    /// A construção terminou: o owner de construção passa a ser o resultado.
    pub fn concluir_construcao(&mut self, h: Ref) {
        if let Some(m) = self.objetos.get_mut(&h)
            && m.estado == EstadoArc::Construindo
        {
            m.estado = EstadoArc::Vivo;
        }
    }

    /// Uma ocorrência forte a mais de `h` (§19.3).
    pub fn reter(&mut self, h: Ref) -> Result<(), ErroArc> {
        if !e_handle(h) {
            return Ok(());
        }
        let Some(m) = self.objetos.get_mut(&h) else { return Ok(()) };
        if m.imortal {
            return Ok(());
        }
        if matches!(m.estado, EstadoArc::Coletando | EstadoArc::Morto) {
            return Err(ErroArc::EstadoInvalido(h, m.estado));
        }
        m.rc = m.rc.checked_add(1).ok_or(ErroArc::Overflow(h))?;
        self.estatisticas.retains += 1;
        if self.traco {
            eprintln!("[arc-traco] reter {h} rc={}", m.rc);
        }
        Ok(())
    }

    /// Uma ocorrência forte a menos de `h` (§19.3): zero vai para a fila de
    /// zeros; positivo vira candidato a ciclo.
    pub fn soltar(&mut self, h: Ref) -> Result<(), ErroArc> {
        if !e_handle(h) {
            return Ok(());
        }
        let Some(m) = self.objetos.get_mut(&h) else { return Ok(()) };
        if m.imortal {
            return Ok(());
        }
        if matches!(m.estado, EstadoArc::Coletando | EstadoArc::Morto) {
            return Err(ErroArc::EstadoInvalido(h, m.estado));
        }
        if m.rc == 0 {
            return Err(ErroArc::RcZero(h));
        }
        m.rc -= 1;
        self.estatisticas.releases += 1;
        if self.traco {
            eprintln!("[arc-traco] soltar {h} rc={}", m.rc);
        }
        let id = IdArc { handle: h, geracao: m.geracao };
        if m.rc == 0 {
            self.zeros.push(id);
        } else if !m.candidato && !self.sem_arestas.is_some_and(|f| f(h)) {
            m.candidato = true;
            self.candidatos.push(id);
        }
        Ok(())
    }

    /// Marca a clausura forte de `h` como protegida condicionalmente (§22.2):
    /// RC zero de um protegido não inicia o descarte.
    pub fn proteger(&mut self, grafo: &dyn GrafoArc, h: Ref) {
        let mut pilha = vec![h];
        while let Some(x) = pilha.pop() {
            let Some(m) = self.objetos.get_mut(&x) else { continue };
            if m.protegido_condicional || m.imortal {
                continue;
            }
            m.protegido_condicional = true;
            grafo.arestas(x, &mut |v| {
                if e_handle(v) {
                    pilha.push(v);
                }
            });
        }
    }

    /// Retira toda proteção condicional (o ponto fixo da rodada a refaz).
    pub fn limpar_protecoes(&mut self) {
        for m in self.objetos.values_mut() {
            m.protegido_condicional = false;
        }
    }

    fn valido(&self, id: IdArc) -> Option<&MetaArc> {
        self.objetos.get(&id.handle).filter(|m| m.geracao == id.geracao)
    }

    /// Processa a fila de zeros (§19.3): cada objeto vivo, da geração da
    /// entrada, com RC ainda zero e sem proteção morre; os destinos das suas
    /// arestas perdem uma ocorrência, o que pode encadear mais zeros (fila
    /// iterativa, sem recursão). `limite` conta objetos descartados.
    ///
    /// `protegido` são as raízes observacionais (§21.3): as ocorrências que o
    /// RC não conta (a pilha, os quadros do runtime, os globais). Um zero que
    /// elas veem não morre e fica na fila para a próxima drenagem (a tabela
    /// de contagem zero do RC adiado).
    pub fn drenar_zeros(&mut self, grafo: &mut dyn GrafoArc, limite: usize, protegido: &dyn Fn(Ref) -> bool) -> Result<usize, ErroArc> {
        let mut feitos = 0;
        while feitos < limite {
            let Some(id) = self.zeros.pop() else { break };
            self.estatisticas.zeros_vistos += 1;
            let Some(m) = self.valido(id) else { continue };
            if m.estado == EstadoArc::Vivo && m.rc > 0 {
                // Desceu a zero e voltou a subir antes da drenagem: a
                // ocorrência nova pode fechar um ciclo sem raiz (o
                // autociclo `x[i] = x` depois de soltar a última de fora),
                // então ele é candidato como quem desce a um RC positivo.
                self.candidatar(id.handle);
                continue;
            }
            if m.estado != EstadoArc::Vivo || m.rc != 0 || m.protegido_condicional || m.imortal {
                continue;
            }
            if protegido(id.handle) {
                // Fica fora da fila até a próxima coleta: dentro desta as
                // raízes não mudam, e reexaminá-lo a cada passada (e cada
                // cópia dele, a cada vez que o RC voltou a zero) era o custo
                // maior da cascata.
                self.estatisticas.zeros_adiados += 1;
                if let Some(m) = self.objetos.get_mut(&id.handle)
                    && !m.adiado
                {
                    m.adiado = true;
                    self.adiados.push(id);
                }
                continue;
            }
            self.descartar(grafo, &[id.handle])?;
            self.estatisticas.mortos_por_rc += 1;
            feitos += 1;
        }
        Ok(feitos)
    }

    /// Os adiados voltam à fila de zeros, uma vez por coleta (no começo dela:
    /// a raiz que os segurava pode ter sumido). O que não é mais válido, ou
    /// já não tem RC zero, sai.
    pub fn retomar_adiados(&mut self) {
        let adiados = std::mem::take(&mut self.adiados);
        for id in adiados {
            if let Some(m) = self.objetos.get_mut(&id.handle)
                && m.geracao == id.geracao
            {
                m.adiado = false;
                self.zeros.push(id);
            }
        }
    }

    /// Há zeros esperando.
    pub fn ha_zeros(&self) -> bool {
        !self.zeros.is_empty()
    }

    /// A transação de descarte do conjunto `d` (§19.4): marca todos
    /// `Coletando`; para cada ocorrência forte `u→v` com `u` em `d`, solta
    /// `v` se `v` não pertence a `d`; rompe as arestas; marca `Morto` e
    /// publica os mortos para o reclamador.
    pub fn descartar(&mut self, grafo: &mut dyn GrafoArc, d: &[Ref]) -> Result<(), ErroArc> {
        assert!(!self.transacao, "ARC: descarte reentrante");
        self.transacao = true;
        // A cascata dos zeros descarta um objeto por vez: aí a única
        // aresta interna é a do próprio objeto, o conjunto (uma alocação
        // por morte) não é montado, e as arestas são lidas e zeradas num
        // percurso só.
        let unico = if d.len() == 1 { Some(d[0]) } else { None };
        let conjunto: HashSet<Ref> = if unico.is_some() { HashSet::default() } else { d.iter().copied().collect() };
        let interno = |v: Ref| match unico {
            Some(u) => u == v,
            None => conjunto.contains(&v),
        };
        for &u in d {
            if let Some(m) = self.objetos.get_mut(&u) {
                m.estado = EstadoArc::Coletando;
            }
        }
        // As saídas do conjunto, uma vez cada ocorrência (o vetor é
        // reaproveitado entre descartes).
        let mut saidas = std::mem::take(&mut self.saidas);
        if let Some(u) = unico {
            grafo.tirar_arestas(u, &mut |v| {
                if e_handle(v) && v != u {
                    saidas.push(v);
                }
            });
        } else {
            for &u in d {
                grafo.arestas(u, &mut |v| {
                    if e_handle(v) && !interno(v) {
                        saidas.push(v);
                    }
                });
            }
        }
        let mut erro = None;
        for v in saidas.drain(..) {
            if let Err(e) = self.soltar(v) {
                erro.get_or_insert(e);
            }
        }
        self.saidas = saidas;
        for &u in d {
            if self.traco {
                eprintln!("[arc-traco] morrer {u}");
            }
            if unico.is_none() {
                grafo.romper(u);
            }
            if let Some(m) = self.objetos.get_mut(&u) {
                m.estado = EstadoArc::Morto;
                m.rc = 0;
                m.candidato = false;
            }
            self.mortos.push(u);
        }
        self.transacao = false;
        match erro {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// A coleta de ciclos por *trial deletion* lateral (§22.1): a região `R`
    /// é a clausura forte dos candidatos válidos; `trial[v] = rc[v]` menos as
    /// ocorrências internas a `R`; os de trial positivo (e os imortais, em
    /// construção ou protegidos) sobrevivem e propagam sobrevivência; o resto
    /// é um conjunto morto, descartado em lote. Nenhum RC real muda antes do
    /// commit; um trial negativo aborta com diagnóstico. `protegido`: as
    /// raízes observacionais, que fazem sobreviver como uma referência externa.
    pub fn coletar_ciclos(&mut self, grafo: &mut dyn GrafoArc, protegido: &dyn Fn(Ref) -> bool) -> Result<usize, ErroArc> {
        let candidatos = std::mem::take(&mut self.candidatos);
        let mut sementes: Vec<Ref> = Vec::new();
        for id in candidatos {
            let Some(m) = self.objetos.get_mut(&id.handle).filter(|m| m.geracao == id.geracao) else { continue };
            m.candidato = false;
            if m.estado == EstadoArc::Vivo && m.rc > 0 && !m.imortal {
                sementes.push(id.handle);
            }
        }
        if sementes.is_empty() {
            return Ok(0);
        }
        self.estatisticas.rodadas_de_ciclo += 1;
        // R: a clausura forte das sementes entre os gerenciados não imortais.
        let mut r: Vec<Ref> = Vec::new();
        let mut em_r: HashSet<Ref> = HashSet::default();
        let mut pilha = sementes;
        while let Some(x) = pilha.pop() {
            if em_r.contains(&x) {
                continue;
            }
            let Some(m) = self.objetos.get(&x) else { continue };
            if m.imortal || m.estado == EstadoArc::Morto {
                continue;
            }
            em_r.insert(x);
            r.push(x);
            grafo.arestas(x, &mut |v| {
                if e_handle(v) {
                    pilha.push(v);
                }
            });
        }
        self.estatisticas.examinados += r.len() as u64;
        // trial = rc − ocorrências internas.
        let mut trial: HashMap<Ref, i128> = HashMap::default();
        for &x in &r {
            trial.insert(x, self.objetos[&x].rc as i128);
        }
        for &u in &r {
            grafo.arestas(u, &mut |v| {
                if let Some(t) = trial.get_mut(&v) {
                    *t -= 1;
                }
            });
        }
        if let Some((&h, _)) = trial.iter().find(|&(_, &t)| t < 0) {
            return Err(ErroArc::TrialNegativo(h));
        }
        // Sobreviventes: trial positivo, em construção ou protegido; propagam.
        let mut vivos: HashSet<Ref> = HashSet::default();
        let mut pilha: Vec<Ref> = r
            .iter()
            .copied()
            .filter(|x| {
                let m = &self.objetos[x];
                trial[x] > 0 || m.estado == EstadoArc::Construindo || m.protegido_condicional || protegido(*x)
            })
            .collect();
        while let Some(x) = pilha.pop() {
            if !em_r.contains(&x) || !vivos.insert(x) {
                continue;
            }
            grafo.arestas(x, &mut |v| {
                if e_handle(v) {
                    pilha.push(v);
                }
            });
        }
        // O sobrevivente que só a raiz observacional segura volta a ser
        // candidato: a raiz pode sumir sem decremento.
        for &x in &r {
            if vivos.contains(&x) && protegido(x) {
                self.candidatar(x);
            }
        }
        let mortos: Vec<Ref> = r.into_iter().filter(|x| !vivos.contains(x)).collect();
        if mortos.is_empty() {
            return Ok(0);
        }
        let n = mortos.len();
        self.descartar(grafo, &mortos)?;
        self.estatisticas.mortos_por_ciclo += n as u64;
        Ok(n)
    }

    /// Os mortos lógicos publicados desde a última chamada, para o reclamador
    /// físico; os metadados deles saem aqui (§19.4, passo 2).
    pub fn tomar_mortos(&mut self) -> Vec<Ref> {
        let mortos = std::mem::take(&mut self.mortos);
        for h in &mortos {
            if self.objetos.get(h).is_some_and(|m| m.estado == EstadoArc::Morto) {
                self.objetos.remove(h);
            }
        }
        mortos
    }

    /// Reconta, com o heap estabilizado, `owners de raiz + arestas fortes` de
    /// cada objeto vivo e compara com o RC (§20.3, modo auditor). `raizes`
    /// são as ocorrências proprietárias (não as observacionais, §21.3).
    pub fn auditar(&self, grafo: &dyn GrafoArc, raizes: &[Ref]) -> Result<(), ErroArc> {
        let mut conta: HashMap<Ref, u64> = HashMap::default();
        for &h in raizes {
            if e_handle(h) {
                *conta.entry(h).or_insert(0) += 1;
            }
        }
        for (h, m) in self.objetos.iter() {
            if m.estado == EstadoArc::Morto {
                continue;
            }
            grafo.arestas(h, &mut |v| {
                if e_handle(v) {
                    *conta.entry(v).or_insert(0) += 1;
                }
            });
        }
        for (h, m) in self.objetos.iter() {
            if m.imortal || m.estado == EstadoArc::Morto {
                continue;
            }
            let recontado = conta.get(&h).copied().unwrap_or(0);
            if recontado != m.rc {
                return Err(ErroArc::Auditoria { handle: h, rc: m.rc, recontado });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn imortal_nao_reutiliza_geracao_apos_esgotamento() {
        let mut arc = EstadoDoArc::novo();
        arc.proxima_geracao = u64::MAX - 1;
        arc.registrar_imortal(2);
        assert_eq!(arc.meta(2).unwrap().geracao, u64::MAX - 1);
        let erro = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            arc.registrar_imortal(4);
        }));
        assert!(erro.is_err());
        assert_eq!(arc.proxima_geracao, u64::MAX);
        assert!(arc.meta(4).is_none());
        assert_eq!(arc.registrados(), 1);
    }

    #[test]
    fn indice_por_inverso_confere_todos_os_bytes_das_classes() {
        use crate::layout::{CABECA_DA_PAGINA, DESLOCAMENTO_DO_HANDLE, N_CLASSES, PAGINA, bytes_do_bloco, palavras_da_classe};
        let primeiro = (1u64 << 32) + CABECA_DA_PAGINA as u64;
        for classe in 1..N_CLASSES {
            let tamanho = bytes_do_bloco(palavras_da_classe(classe)) as u64;
            let blocos = ((PAGINA - CABECA_DA_PAGINA) as u64 / tamanho) as u32;
            let pagina = PaginaArc::nova(Geometria { primeiro, tamanho, blocos });
            assert_eq!(pagina.indice(primeiro as Ref + DESLOCAMENTO_DO_HANDLE - 1), None);
            for d in 0..=PAGINA as u64 {
                // Oráculo deliberadamente pela divisão original: todos os
                // inícios, interiores e bytes além do último bloco.
                let esperado = (d % tamanho == 0 && d / tamanho < u64::from(blocos)).then_some((d / tamanho) as usize);
                let h = (primeiro + d) as Ref + DESLOCAMENTO_DO_HANDLE;
                assert_eq!(pagina.indice(h), esperado, "classe {classe}, deslocamento {d}");
            }
        }
    }

    #[test]
    fn indice_por_inverso_confere_tamanhos_grandes_e_limites_de_u64() {
        for tamanho in [1, 2, 3, 24, (1u64 << 32) - 1, 1u64 << 32, (1u64 << 63) - 1, 1u64 << 63, u64::MAX] {
            let blocos = (u64::MAX / tamanho).min(7) as u32;
            let pagina = PaginaArc::nova(Geometria { primeiro: 0, tamanho, blocos });
            let consultar = |d: u64| pagina.indice(d.wrapping_add(crate::layout::DESLOCAMENTO_DO_HANDLE as u64) as Ref);
            for i in 0..u64::from(blocos) {
                let d = i * tamanho;
                assert_eq!(consultar(d), Some(i as usize), "tamanho {tamanho}, índice {i}");
                if tamanho > 1 {
                    assert_eq!(consultar(d + 1), None);
                }
            }
            let fim = tamanho * u64::from(blocos);
            for d in [fim, fim.saturating_add(1), u64::MAX] {
                assert_eq!(consultar(d), None, "tamanho {tamanho}, deslocamento {d}");
            }
        }
    }

    #[test]
    fn pagina_reformatada_atualiza_o_inverso_e_preserva_o_metadado() {
        let primeiro = (1u64 << 32) + 1024;
        let h = (primeiro + 48) as Ref + crate::layout::DESLOCAMENTO_DO_HANDLE;
        let mut a = EstadoDoArc::novo();
        a.registrar_vivo_em(h, Some(Geometria { primeiro, tamanho: 24, blocos: 4 }));
        a.revisar(h);
        a.drenar_zeros(&mut Grafo::default(), usize::MAX, &|_| false).unwrap();
        a.tomar_mortos();
        let novo = a.registrar_vivo_em(h, Some(Geometria { primeiro, tamanho: 48, blocos: 2 }));
        a.reter(h).unwrap();
        assert_eq!(a.meta(h).unwrap().geracao, novo.geracao);
        assert_eq!(a.meta(h).unwrap().rc, 1);
        assert!(a.meta(h + 24).is_none());
    }

    /// Um grafo de mentira: handle → lista de destinos (com multiplicidade).
    #[derive(Default)]
    struct Grafo(HashMap<Ref, Vec<Ref>>);

    impl GrafoArc for Grafo {
        fn arestas(&self, h: Ref, f: &mut dyn FnMut(Ref)) {
            for &v in self.0.get(&h).map(|v| v.as_slice()).unwrap_or(&[]) {
                f(v);
            }
        }
        fn romper(&mut self, h: Ref) {
            self.0.remove(&h);
        }
    }

    /// Aloca `h` (rc=1 do owner de construção) e o conclui.
    fn novo(a: &mut EstadoDoArc, h: Ref) {
        a.registrar(h).unwrap();
        a.concluir_construcao(h);
    }

    /// `u.campo = v`: o destino ganha a ocorrência.
    fn ligar(a: &mut EstadoDoArc, g: &mut Grafo, u: Ref, v: Ref) {
        a.reter(v).unwrap();
        g.0.entry(u).or_default().push(v);
    }

    fn morto(a: &EstadoDoArc, h: Ref) -> bool {
        a.meta(h).is_none_or(|m| m.estado == EstadoArc::Morto)
    }

    #[test]
    fn null_e_smi_nao_sao_contados() {
        let mut a = EstadoDoArc::novo();
        a.reter(0).unwrap();
        a.reter(7).unwrap();
        a.soltar(7).unwrap();
        assert_eq!(a.estatisticas.retains, 0);
    }

    #[test]
    fn cadeia_aciclica_morre_em_cascata_ao_soltar_a_raiz() {
        let (mut a, mut g) = (EstadoDoArc::novo(), Grafo::default());
        for h in [2, 4, 6] {
            novo(&mut a, h);
        }
        ligar(&mut a, &mut g, 2, 4);
        ligar(&mut a, &mut g, 4, 6);
        // Os owners de construção de 4 e 6 saem: só as arestas os mantêm.
        a.soltar(4).unwrap();
        a.soltar(6).unwrap();
        a.auditar(&g, &[2]).unwrap();
        a.soltar(2).unwrap();
        assert_eq!(a.drenar_zeros(&mut g, usize::MAX, &|_| false).unwrap(), 3);
        assert!(morto(&a, 2) && morto(&a, 4) && morto(&a, 6));
        assert_eq!(a.tomar_mortos().len(), 3);
        assert_eq!(a.registrados(), 0);
    }

    #[test]
    fn multiplicidade_de_ocorrencias() {
        let (mut a, mut g) = (EstadoDoArc::novo(), Grafo::default());
        novo(&mut a, 2);
        novo(&mut a, 4);
        // Duas posições da lista 2 apontam para 4: contam duas vezes.
        ligar(&mut a, &mut g, 2, 4);
        ligar(&mut a, &mut g, 2, 4);
        a.soltar(4).unwrap();
        assert_eq!(a.meta(4).unwrap().rc, 2);
        a.auditar(&g, &[2]).unwrap();
        a.soltar(2).unwrap();
        a.drenar_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(morto(&a, 4));
    }

    #[test]
    fn ciclo_isolado_so_morre_pela_coleta_de_ciclos() {
        let (mut a, mut g) = (EstadoDoArc::novo(), Grafo::default());
        novo(&mut a, 2);
        novo(&mut a, 4);
        ligar(&mut a, &mut g, 2, 4);
        ligar(&mut a, &mut g, 4, 2);
        a.soltar(4).unwrap();
        a.soltar(2).unwrap();
        a.drenar_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(!morto(&a, 2) && !morto(&a, 4), "o RC sozinho não fecha o ciclo");
        assert_eq!(a.coletar_ciclos(&mut g, &|_| false).unwrap(), 2);
        assert!(morto(&a, 2) && morto(&a, 4));
        assert_eq!(a.estatisticas.mortos_por_ciclo, 2);
    }

    #[test]
    fn ciclo_com_referencia_externa_sobrevive_e_a_coleta_nao_muda_rc() {
        let (mut a, mut g) = (EstadoDoArc::novo(), Grafo::default());
        for h in [2, 4, 6] {
            novo(&mut a, h);
        }
        ligar(&mut a, &mut g, 2, 4);
        ligar(&mut a, &mut g, 4, 2);
        // 6 (raiz) aponta para 4.
        ligar(&mut a, &mut g, 6, 4);
        a.soltar(4).unwrap();
        a.soltar(2).unwrap();
        let antes = (a.meta(2).unwrap().rc, a.meta(4).unwrap().rc);
        assert_eq!(a.coletar_ciclos(&mut g, &|_| false).unwrap(), 0);
        assert_eq!((a.meta(2).unwrap().rc, a.meta(4).unwrap().rc), antes);
        a.auditar(&g, &[6]).unwrap();
        // Sem a raiz, o ciclo morre na próxima rodada; 6 morre pelo RC.
        a.soltar(6).unwrap();
        a.drenar_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(morto(&a, 6));
        assert_eq!(a.coletar_ciclos(&mut g, &|_| false).unwrap(), 2);
    }

    #[test]
    fn ciclo_que_segura_um_acíclico_libera_os_dois() {
        let (mut a, mut g) = (EstadoDoArc::novo(), Grafo::default());
        for h in [2, 4, 8] {
            novo(&mut a, h);
        }
        ligar(&mut a, &mut g, 2, 4);
        ligar(&mut a, &mut g, 4, 2);
        ligar(&mut a, &mut g, 4, 8);
        for h in [2, 4, 8] {
            a.soltar(h).unwrap();
        }
        a.coletar_ciclos(&mut g, &|_| false).unwrap();
        a.drenar_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(morto(&a, 2) && morto(&a, 4) && morto(&a, 8));
    }

    #[test]
    fn em_construcao_e_protegido_nao_morrem() {
        let (mut a, mut g) = (EstadoDoArc::novo(), Grafo::default());
        a.registrar(2).unwrap();
        // Em construção: RC zero não descarta.
        a.soltar(2).unwrap();
        a.drenar_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(!morto(&a, 2));
        novo(&mut a, 4);
        a.proteger(&g, 4);
        a.soltar(4).unwrap();
        a.drenar_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(!morto(&a, 4), "protegido condicionalmente espera o ponto fixo");
    }

    #[test]
    fn raiz_observacional_segura_zero_e_ciclo() {
        let (mut a, mut g) = (EstadoDoArc::novo(), Grafo::default());
        novo(&mut a, 2);
        novo(&mut a, 4);
        ligar(&mut a, &mut g, 2, 4);
        ligar(&mut a, &mut g, 4, 2);
        novo(&mut a, 6);
        a.soltar(2).unwrap();
        a.soltar(4).unwrap();
        a.soltar(6).unwrap();
        // 6 só é visto pela pilha; o ciclo 2-4 também.
        let raiz = |h: Ref| h == 6 || h == 2;
        a.drenar_zeros(&mut g, usize::MAX, &raiz).unwrap();
        assert!(!morto(&a, 6));
        assert_eq!(a.coletar_ciclos(&mut g, &raiz).unwrap(), 0);
        // A pilha soltou: morrem na próxima drenagem (que começa retomando
        // os adiados) e rodada.
        a.retomar_adiados();
        a.drenar_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(morto(&a, 6));
        let n = {
            a.candidatos.push(IdArc { handle: 2, geracao: a.meta(2).unwrap().geracao });
            a.coletar_ciclos(&mut g, &|_| false).unwrap()
        };
        assert_eq!(n, 2);
    }

    #[test]
    fn soltar_com_rc_zero_e_erro_e_overflow_tambem() {
        let mut a = EstadoDoArc::novo();
        novo(&mut a, 2);
        a.soltar(2).unwrap();
        assert_eq!(a.soltar(2), Err(ErroArc::RcZero(2)));
    }

    #[test]
    fn entrada_obsoleta_de_fila_e_ignorada_pela_geracao() {
        let (mut a, mut g) = (EstadoDoArc::novo(), Grafo::default());
        novo(&mut a, 2);
        a.soltar(2).unwrap();
        a.drenar_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        a.tomar_mortos();
        // O endereço é reaproveitado: geração nova, a entrada velha não vale.
        novo(&mut a, 2);
        a.zeros.push(IdArc { handle: 2, geracao: 1 });
        a.drenar_zeros(&mut g, usize::MAX, &|_| false).unwrap();
        assert!(!morto(&a, 2));
    }

    #[test]
    fn auditoria_acha_rc_errado() {
        let (mut a, g) = (EstadoDoArc::novo(), Grafo::default());
        novo(&mut a, 2);
        a.reter(2).unwrap();
        assert!(matches!(a.auditar(&g, &[2]), Err(ErroArc::Auditoria { handle: 2, rc: 2, recontado: 1 })));
    }
}
