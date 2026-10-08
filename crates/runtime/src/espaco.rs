//! O espaço de objetos (docs/NATIVO-ESPACO-UNIFICADO.md §2.6–§2.8 e §3.2).
//!
//! Todo bloco do heap mora aqui: um [`Cabecalho`] de 16 bytes seguido do corpo,
//! em palavras de 8 bytes. As páginas de [`PAGINA`] bytes guardam blocos de uma
//! só **classe de tamanho** (`layout::classe_de_tamanho`: as exatas de 1 a 64
//! palavras e 24 médias até 2014 palavras, cada uma enchendo a página); um corpo
//! maior tem uma **região grande** própria, arredondada a 4 KiB e alinhada a
//! [`PAGINA`] (`VirtualAlloc`/`mmap`), solta já na coleta menor que a acha
//! morta, para um cache por tamanho com teto. As páginas saem de pedaços
//! reservados do sistema e só ocupam memória onde se escreve; as soltas voltam
//! ao sistema salvo um cache quente pequeno (docs/NATIVO-PLANO.md §14).
//!
//! O começo de cada página (e de cada região) é o mapa de marcas: um bit por
//! palavra. A marcação acende o bit do objeto alcançado; as varreduras leem só os
//! bits, sem tocar os blocos, vivos ou mortos (a VM também marca fora do objeto
//! no *old space*, `marking.cc`; aqui o mapa fica junto, por página, como o do
//! Immix). O coletor não move: o endereço de um objeto é fixo enquanto ele vive.
//!
//! O coletor percorre o corpo pelo **formato** em `flags` (§2.3), sem olhar a
//! classe: `INSTANCIA` pelo mapa de referências, `BRUTO` nada, `REFS` as palavras
//! `1..=palavra 0`. As `REFS` grandes têm **cartões** (1 bit por 32 elementos,
//! depois do último elemento): a coleta menor percorre de um lembrado só os
//! elementos dos cartões sujos. Um bloco com **anexo** (`ANEXO`) guarda em `b+16`
//! um ponteiro nativo, solto quando o dono morre.
//!
//! É o par, sem mover objetos, do *new space* da VM
//! (`runtime/vm/heap/scavenger.cc` e a alocação em linha do
//! `stub_code_compiler.cc`): alocar é tirar o próximo bloco da região da classe —
//! pelo código gerado, da TLAB (`Contexto::tlab`) — sem `malloc`.
#![allow(unsafe_code)]

use crate::layout::{
    CABECA_DA_PAGINA, Cabecalho, DESLOCAMENTO_DO_HANDLE, ELEMENTOS_POR_CARTAO, ELEMENTOS_POR_PALAVRA_DE_CARTAO,
    MAIOR_CLASSE, N_CLASSES, PAGINA, bytes_do_bloco, capacidade, classe_de_tamanho, e_objeto, palavras_da_classe,
    palavras_do_mapa,
};

const LIVRE: u8 = crate::layout::estado::LIVRE;
const JOVEM: u8 = crate::layout::estado::JOVEM;
const VELHO: u8 = crate::layout::estado::VELHO;
const LEMBRADO: u8 = crate::layout::estado::LEMBRADO;
const FORA: u8 = crate::layout::flags::FORA;
const FORMA: u8 = crate::layout::flags::FORMA;
const INSTANCIA: u8 = crate::layout::flags::INSTANCIA;
const REFS: u8 = crate::layout::flags::REFS;
const CARTOES: u8 = crate::layout::flags::CARTOES;

/// A classe das regiões grandes nos vetores por classe (o índice 0, que nenhuma
/// classe de tamanho usa).
pub(crate) const GRANDE: usize = 0;

/// Granularidade das regiões grandes (a página do sistema).
const GRANULARIDADE_DA_REGIAO: usize = 4096;

/// Até quantos blocos cada reabastecimento da TLAB entrega (uma faixa contígua da
/// lista livre: menos, se a contiguidade acaba antes).
pub(crate) const TLAB_BLOCOS: usize = 256;

// ─── Veneno e quarentena (`DARTFORGE_GC_VENENO`) ───────────────────────────

/// O estado de um bloco envenenado: o primeiro byte do padrão de veneno.
/// Nenhum estado válido o usa; a validação de handle o recusa.
pub(crate) const ESTADO_DE_VENENO: u8 = 0xDE;
/// A palavra com que o bloco morto é preenchido: par, e com o primeiro byte
/// (o estado) em [`ESTADO_DE_VENENO`].
const PALAVRA_DE_VENENO: u64 = 0xDFDF_DFDF_DFDF_DFDE;

/// A quarentena dos blocos mortos (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
/// §7.5): com `DARTFORGE_GC_VENENO=1` (ou `=<n>` blocos), cada bloco que
/// morre é preenchido com o padrão de veneno e só volta à lista livre depois
/// que outros `n` (1 000 por padrão) morreram depois dele. Um uso depois de
/// liberar lê então o veneno e falha na validação de handle, em vez de ler um
/// objeto novo posto no mesmo lugar. O coletor não move objetos: é o
/// equivalente da quarentena do *from-space* de um coletor de cópia.
struct Quarentena {
    /// (classe, bloco), na ordem em que entraram.
    anel: std::collections::VecDeque<(usize, *mut u8)>,
    /// Os blocos do anel, para a varredura completa não os devolver.
    presos: crate::hash::HashSet<usize>,
    capacidade: usize,
}

impl Quarentena {
    fn do_ambiente() -> Option<Self> {
        let v = std::env::var("DARTFORGE_GC_VENENO").ok()?;
        let capacidade = match v.trim() {
            "" | "0" => return None,
            "1" => 1000,
            n => n.parse::<usize>().ok().filter(|&n| n > 0)?,
        };
        Some(Quarentena { anel: std::collections::VecDeque::new(), presos: crate::hash::HashSet::default(), capacidade })
    }

    /// Envenena o bloco `b` de `tamanho` bytes e o põe no anel.
    ///
    /// # Safety
    /// `b` é bloco sem objeto vivo de uma página do espaço.
    unsafe fn prender(&mut self, classe: usize, b: *mut u8, tamanho: usize) {
        let palavras = b.cast::<u64>();
        for i in 0..tamanho / 8 {
            // SAFETY: o contrato da função; o bloco é alinhado a 8.
            unsafe { palavras.add(i).write(PALAVRA_DE_VENENO) };
        }
        self.anel.push_back((classe, b));
        self.presos.insert(b as usize);
    }

    /// Na varredura completa: o bloco `b` não marcado fica (ou entra) na
    /// quarentena? Já no anel, sim; com o estado de um objeto que acabou de
    /// morrer, entra; livre (zerado ou já envenenado e solto), não.
    ///
    /// # Safety
    /// `b` é bloco sem marca de uma página do espaço.
    unsafe fn reter(&mut self, classe: usize, b: *mut u8, tamanho: usize) -> bool {
        if self.presos.contains(&(b as usize)) {
            return true;
        }
        // SAFETY: o contrato da função.
        let estado = unsafe { *b };
        if estado == LIVRE || estado == ESTADO_DE_VENENO {
            return false;
        }
        // SAFETY: o contrato da função.
        unsafe { self.prender(classe, b, tamanho) };
        true
    }

    /// Os blocos que passaram da capacidade, os mais antigos primeiro: saem
    /// do anel (continuam envenenados até a entrega, que os zera).
    fn soltos(&mut self) -> Vec<(usize, *mut u8)> {
        let mut v = Vec::new();
        while self.anel.len() > self.capacidade {
            let (classe, b) = self.anel.pop_front().expect("anel além da capacidade");
            self.presos.remove(&(b as usize));
            v.push((classe, b));
        }
        v
    }
}

// ─── Mapa de marcas ────────────────────────────────────────────────────────

/// A palavra e o bit do mapa de marcas do bloco `b`.
#[inline]
pub(crate) fn bit_de_marca(b: *const Cabecalho) -> (*mut u64, u64) {
    let a = b as usize;
    let base = a & !(PAGINA - 1);
    let desl = a - base;
    ((base + (desl >> 9) * 8) as *mut u64, 1u64 << ((desl >> 3) & 63))
}

/// O bloco `b` foi marcado (a coleta completa em curso, ou a menor que o
/// promoveu)?
///
/// # Safety
/// `b` é bloco de uma página do espaço (nunca um estático: o mapa dele cairia
/// fora de uma página).
#[inline]
pub(crate) unsafe fn marcado(b: *const Cabecalho) -> bool {
    let (w, m) = bit_de_marca(b);
    // SAFETY: o contrato da função; o mapa está no começo da página.
    unsafe { *w & m != 0 }
}

/// Acende o bit de marca do bloco `b`.
///
/// # Safety
/// `b` é bloco de uma página do espaço.
#[inline]
pub(crate) unsafe fn marcar_bloco(b: *const Cabecalho) {
    let (w, m) = bit_de_marca(b);
    // SAFETY: o contrato da função.
    unsafe { *w |= m };
}

/// Apaga o bit de marca do bloco `b` (o bloco que o ARC puro solta: ao ser
/// entregue de novo como jovem, a marca velha o daria por vivo).
///
/// # Safety
/// `b` é bloco de uma página do espaço.
#[inline]
pub(crate) unsafe fn desmarcar_bloco(b: *const Cabecalho) {
    let (w, m) = bit_de_marca(b);
    // SAFETY: o contrato da função.
    unsafe { *w &= !m };
}

// ─── O corpo de um bloco ───────────────────────────────────────────────────

/// Bytes de um bloco (ou corpo de fora) `INSTANCIA` de `n` campos.
#[inline]
pub const fn tamanho_do_bloco(n: usize) -> usize {
    bytes_do_bloco(capacidade(n) + palavras_do_mapa(n))
}

/// O formato do corpo do bloco `b` (`flags & FORMA`).
///
/// # Safety
/// `b` aponta um cabeçalho legível.
#[inline]
pub(crate) unsafe fn forma(b: *const Cabecalho) -> u8 {
    // SAFETY: o contrato da função.
    unsafe { (*b).flags & FORMA }
}

/// O corpo de um objeto `INSTANCIA` (o próprio bloco, ou o corpo de fora).
///
/// # Safety
/// `b` é o cabeçalho de um bloco vivo.
#[inline]
pub(crate) unsafe fn corpo(b: *mut Cabecalho) -> *mut Cabecalho {
    // SAFETY: o contrato da função; com FORA, o primeiro campo é o endereço.
    unsafe {
        if (*b).flags & FORA != 0 { *campos_de(b).cast::<*mut Cabecalho>() } else { b }
    }
}

/// As palavras depois de um cabeçalho (bloco ou corpo).
#[inline]
pub(crate) fn campos_de(c: *mut Cabecalho) -> *mut i64 {
    c.wrapping_add(1).cast()
}

/// O campo `i` de um corpo `INSTANCIA` é referência?
///
/// # Safety
/// `c` é um corpo válido e `i < n`.
#[inline]
pub(crate) unsafe fn e_referencia(c: *const Cabecalho, i: usize) -> bool {
    // SAFETY: o contrato da função.
    unsafe {
        if i < 32 {
            (*c).mapa >> i & 1 != 0
        } else {
            let ext = campos_de(c.cast_mut()).add(capacidade(usize::from((*c).n))).cast::<u64>();
            *ext.add((i - 32) / 64) >> ((i - 32) % 64) & 1 != 0
        }
    }
}

/// Acende ou apaga o bit de referência do campo `i` de um corpo `INSTANCIA`.
///
/// # Safety
/// `c` é um corpo válido e `i < n`.
#[inline]
pub(crate) unsafe fn marcar_referencia(c: *mut Cabecalho, i: usize, e_ref: bool) {
    // SAFETY: o contrato da função.
    unsafe {
        if i < 32 {
            if e_ref { (*c).mapa |= 1 << i } else { (*c).mapa &= !(1 << i) }
        } else {
            let w = campos_de(c).add(capacidade(usize::from((*c).n))).cast::<u64>().add((i - 32) / 64);
            let bit = 1u64 << ((i - 32) % 64);
            if e_ref { *w |= bit } else { *w &= !bit }
        }
    }
}

/// Empilha os campos-referência de um corpo `INSTANCIA`; devolve quantos campos
/// tem. O `mapa` só é mapa de referências em `INSTANCIA` (§2.3).
///
/// # Safety
/// `c` é um corpo `INSTANCIA` válido.
#[inline]
pub(crate) unsafe fn empilhar_referencias(c: *const Cabecalho, pilha: &mut Vec<i64>) -> usize {
    // SAFETY: o contrato da função.
    unsafe {
        debug_assert!(forma(c) == INSTANCIA, "mapa lido como mapa de referências num corpo que não é INSTANCIA");
        let n = usize::from((*c).n);
        let campos = campos_de(c.cast_mut());
        let mut m = (*c).mapa;
        while m != 0 {
            let i = m.trailing_zeros() as usize;
            pilha.push(*campos.add(i));
            m &= m - 1;
        }
        if n > 32 {
            let ext = campos.add(capacidade(n)).cast::<u64>();
            for w in 0..palavras_do_mapa(n) {
                let mut m = *ext.add(w);
                while m != 0 {
                    let i = 32 + w * 64 + m.trailing_zeros() as usize;
                    pilha.push(*campos.add(i));
                    m &= m - 1;
                }
            }
        }
        n
    }
}

/// Quantos `Ref` um corpo `REFS` de `palavras` palavras guarda: a palavra 0 (o
/// comprimento da `_List`, o número de campos do `_Record`), limitada ao corpo.
///
/// # Safety
/// `b` é um bloco `REFS` válido de `palavras` palavras de corpo.
#[inline]
pub(crate) unsafe fn refs_do_corpo(b: *const Cabecalho, palavras: usize) -> usize {
    // SAFETY: o contrato da função; a palavra 0 existe (`palavras ≥ 1`).
    let n = unsafe { *campos_de(b.cast_mut()) };
    usize::try_from(n).unwrap_or(0).min(palavras.saturating_sub(1))
}

/// As palavras de cartões de um corpo `REFS` com `CARTOES` de `len` elementos
/// (depois do último elemento: `b + 24 + 8·len`).
#[inline]
pub(crate) fn cartoes_de(b: *mut Cabecalho, len: usize) -> *mut u64 {
    campos_de(b).wrapping_add(1 + len).cast()
}

/// Suja o cartão do elemento `i` (índice do elemento, a partir de 0) de uma lista
/// com `CARTOES` de `len` elementos.
///
/// # Safety
/// `b` é um bloco `REFS` com `CARTOES` e `i < len`.
#[inline]
pub(crate) unsafe fn sujar_cartao(b: *mut Cabecalho, len: usize, i: usize) {
    let p = cartoes_de(b, len).wrapping_add(i / ELEMENTOS_POR_PALAVRA_DE_CARTAO);
    // SAFETY: o contrato da função.
    unsafe { *p |= 1u64 << ((i / ELEMENTOS_POR_CARTAO) & 63) };
}

/// Suja todos os cartões (a barreira explícita de quem gravou por fora).
///
/// # Safety
/// `b` é um bloco `REFS` com `CARTOES` de `len` elementos.
pub(crate) unsafe fn sujar_todos_os_cartoes(b: *mut Cabecalho, len: usize) {
    let c = cartoes_de(b, len);
    for w in 0..len.div_ceil(ELEMENTOS_POR_PALAVRA_DE_CARTAO) {
        // SAFETY: o contrato da função.
        unsafe { *c.add(w) = u64::MAX };
    }
}

/// Empilha os `Ref` das palavras `1..=n` de um corpo `REFS`, pulando null e `Smi`
/// (o chamador filtra o resto); devolve `n`.
///
/// # Safety
/// `b` é um bloco `REFS` válido com pelo menos `n + 1` palavras de corpo.
#[inline]
pub(crate) unsafe fn empilhar_refs(b: *const Cabecalho, n: usize, pilha: &mut Vec<i64>) -> usize {
    let p = campos_de(b.cast_mut());
    for i in 1..=n {
        // SAFETY: o contrato da função.
        let r = unsafe { *p.add(i) };
        if r != 0 && r & 1 == 0 {
            pilha.push(r);
        }
    }
    n
}

/// Empilha os `Ref` dos elementos dos cartões sujos de uma lista `REFS` com
/// `CARTOES` de `len` elementos e zera os cartões; devolve quantos elementos
/// percorreu.
///
/// # Safety
/// `b` é um bloco `REFS` com `CARTOES` de `len` elementos.
pub(crate) unsafe fn empilhar_cartoes_sujos(b: *mut Cabecalho, len: usize, pilha: &mut Vec<i64>) -> usize {
    let c = cartoes_de(b, len);
    let elementos = campos_de(b).wrapping_add(1);
    let mut trabalho = 0;
    for w in 0..len.div_ceil(ELEMENTOS_POR_PALAVRA_DE_CARTAO) {
        // SAFETY: o contrato da função.
        let mut m = unsafe { std::ptr::replace(c.add(w), 0) };
        while m != 0 {
            let k = w * 64 + m.trailing_zeros() as usize;
            let (de, ate) = (k * ELEMENTOS_POR_CARTAO, ((k + 1) * ELEMENTOS_POR_CARTAO).min(len));
            for i in de..ate {
                // SAFETY: `i < len`.
                let r = unsafe { *elementos.add(i) };
                if r != 0 && r & 1 == 0 {
                    pilha.push(r);
                }
            }
            trabalho += ate.saturating_sub(de);
            m &= m - 1;
        }
    }
    trabalho
}

/// Zera os cartões de uma lista `REFS` com `CARTOES` de `len` elementos.
///
/// # Safety
/// `b` é um bloco `REFS` com `CARTOES` de `len` elementos.
pub(crate) unsafe fn zerar_cartoes(b: *mut Cabecalho, len: usize) {
    // SAFETY: o contrato da função.
    unsafe { std::ptr::write_bytes(cartoes_de(b, len), 0, len.div_ceil(ELEMENTOS_POR_PALAVRA_DE_CARTAO)) };
}

/// Um corpo de fora de `n` campos zerados (o alocador do sistema).
pub(crate) fn novo_corpo_de_fora(n: usize) -> *mut Cabecalho {
    let layout = std::alloc::Layout::from_size_align(tamanho_do_bloco(n), 8).expect("layout do corpo");
    // SAFETY: layout de tamanho não nulo.
    let p = unsafe { std::alloc::alloc_zeroed(layout) }.cast::<Cabecalho>();
    if p.is_null() {
        std::alloc::handle_alloc_error(layout);
    }
    // SAFETY: memória nova, zerada, do tamanho do corpo.
    unsafe {
        (*p).n = u16::try_from(n).expect("objeto com campos demais");
    }
    p
}

/// Solta um corpo de fora.
///
/// # Safety
/// `c` veio de [`novo_corpo_de_fora`] e ninguém mais o usa.
pub(crate) unsafe fn soltar_corpo_de_fora(c: *mut Cabecalho) {
    // SAFETY: o contrato da função.
    unsafe {
        let n = usize::from((*c).n);
        std::alloc::dealloc(c.cast(), std::alloc::Layout::from_size_align(tamanho_do_bloco(n), 8).expect("layout do corpo"));
    }
}

/// Bytes de uma região grande de corpo de `palavras` palavras: o mapa de marcas
/// e o bloco, arredondados a [`GRANULARIDADE_DA_REGIAO`].
#[inline]
pub(crate) const fn bytes_da_regiao(palavras: usize) -> usize {
    (CABECA_DA_PAGINA + bytes_do_bloco(palavras)).next_multiple_of(GRANULARIDADE_DA_REGIAO)
}

/// Bytes que uma alocação de corpo de `palavras` palavras ocupa: o bloco da
/// classe, ou a região grande.
#[inline]
pub(crate) const fn bytes_de_alocacao(palavras: usize) -> usize {
    match classe_de_tamanho(palavras) {
        Some(c) => bytes_do_bloco(palavras_da_classe(c)),
        None => bytes_da_regiao(palavras),
    }
}

// ─── Páginas ───────────────────────────────────────────────────────────────

/// Uma página do espaço de objetos (blocos de uma só classe de tamanho) ou uma
/// região grande (um bloco só).
#[derive(Debug)]
pub(crate) struct Pagina {
    base: *mut u8,
    /// Bytes alocados (`PAGINA`, ou os da região grande).
    bytes: usize,
    /// A classe de tamanho, ou [`GRANDE`].
    classe: usize,
    /// Palavras do corpo de cada bloco (a da classe; na região grande, a real).
    palavras: usize,
    blocos: usize,
    /// O inverso de `bytes_do_bloco(palavras) / 8` (ímpar, ou potência de 2
    /// vezes ímpar: ver `indice`) módulo 2³²: o índice de um bloco sai de uma
    /// multiplicação, e um deslocamento que não é múltiplo do tamanho dá um
    /// índice além de `blocos` (o teste de divisibilidade exata, *Hacker's
    /// Delight* 10-17).
    inverso: u32,
    /// `bytes_do_bloco(palavras) / 8` = `impar << deslocamento`.
    deslocamento: u32,
    /// Página sem objeto vivo guardada para reuso (em
    /// [`EspacoDeObjetos::vazias`]): o conteúdo é lixo dos mortos até ela ser
    /// zerada e formatada de novo, para qualquer classe.
    vazia: bool,
}

impl Pagina {
    /// A página de blocos da classe `classe` (corpo de `palavras` palavras) em
    /// `base` (`bytes` bytes).
    fn formatada(base: *mut u8, bytes: usize, classe: usize, palavras: usize) -> Self {
        let tamanho = bytes_do_bloco(palavras);
        let blocos = if classe == GRANDE { 1 } else { (PAGINA - CABECA_DA_PAGINA) / tamanho };
        let em_palavras = u32::try_from(tamanho / 8).unwrap_or(1);
        let deslocamento = em_palavras.trailing_zeros();
        let inverso = inverso_impar(em_palavras >> deslocamento);
        Pagina { base, bytes, classe, palavras, blocos, inverso, deslocamento, vazia: false }
    }
    /// O primeiro bloco (depois do mapa de marcas).
    fn inicio(&self) -> *mut u8 {
        self.base.wrapping_add(CABECA_DA_PAGINA)
    }
    fn tamanho(&self) -> usize {
        bytes_do_bloco(self.palavras)
    }
    fn bloco(&self, i: usize) -> *mut Cabecalho {
        self.inicio().wrapping_add(i * self.tamanho()).cast()
    }
    /// O índice do bloco que começa `desl` bytes depois da base, se algum
    /// começa ali.
    #[inline]
    fn indice(&self, desl: usize) -> Option<usize> {
        if desl & 7 != 0 || desl >= self.bytes || desl < CABECA_DA_PAGINA || self.vazia {
            return None;
        }
        let desl = desl - CABECA_DA_PAGINA;
        let palavras = (desl >> 3) as u32;
        let mascara = (1u32 << self.deslocamento) - 1;
        if palavras & mascara != 0 {
            return None;
        }
        let i = (palavras >> self.deslocamento).wrapping_mul(self.inverso) as usize;
        (i < self.blocos).then_some(i)
    }
}

/// O inverso de `a` (ímpar) módulo 2³² (Newton: cada passo dobra os bits certos).
const fn inverso_impar(a: u32) -> u32 {
    let mut x = a;
    let mut i = 0;
    while i < 5 {
        x = x.wrapping_mul(2u32.wrapping_sub(a.wrapping_mul(x)));
        i += 1;
    }
    x
}

/// Base da página (`endereço >> 16`) → índice + 1 em `paginas`: tabelas de 2¹⁶
/// posições por região de 4 GiB (quase sempre uma só), como o *pagemap* de um
/// alocador — a validação de um handle sem hash.
#[derive(Default)]
struct MapaDePaginas {
    regioes: Vec<(usize, Box<[u32]>)>,
}

impl MapaDePaginas {
    const BITS: usize = 16;
    #[inline]
    fn chave(base: usize) -> (usize, usize) {
        let k = base >> PAGINA.trailing_zeros();
        (k >> Self::BITS, k & ((1 << Self::BITS) - 1))
    }
    #[inline]
    fn get(&self, base: usize) -> Option<usize> {
        let (r, i) = Self::chave(base);
        let (_, t) = self.regioes.iter().find(|(x, _)| *x == r)?;
        let v = t[i];
        (v != 0).then(|| v as usize - 1)
    }
    fn definir(&mut self, base: usize, valor: u32) {
        let (r, i) = Self::chave(base);
        let pos = match self.regioes.iter().position(|(x, _)| *x == r) {
            Some(p) => p,
            None => {
                self.regioes.push((r, vec![0u32; 1 << Self::BITS].into_boxed_slice()));
                self.regioes.len() - 1
            }
        };
        self.regioes[pos].1[i] = valor;
    }
    fn inserir(&mut self, base: usize, indice: usize) {
        self.definir(base, u32::try_from(indice + 1).expect("páginas demais"));
    }
    fn remover(&mut self, base: usize) {
        self.definir(base, 0);
    }
}

// ─── Memória do sistema ────────────────────────────────────────────────────

/// A reserva das páginas de [`PAGINA`] bytes do espaço de objetos
/// (docs/NATIVO-PLANO.md §14): pedaços de [`PAGINAS_POR_PEDACO`] páginas
/// reservados do sistema de uma vez, em todas as plataformas, e as páginas
/// soltas pela coleta. A página nova vem zerada do sistema e só entra na
/// memória residente onde o programa escreve (antes, fora do Linux, cada
/// página era um `alloc_zeroed` alinhado a 64 KiB, que no Windows o `HeapAlloc`
/// atende com o dobro dos bytes, todos zerados: 128 KiB residentes por página).
/// A página solta fica suja num cache quente pequeno e o resto volta ao sistema
/// ([`sistema::descartar`]) sem desfazer a reserva — o `gc_free_pages` da Julia.
#[derive(Default)]
struct ReservaDePaginas {
    /// Os pedaços reservados `(início, bytes)`.
    pedacos: Vec<(*mut u8, usize)>,
    /// Páginas zeradas por usar (nunca usadas, ou devolvidas ao sistema): no
    /// Windows, confirmadas só quando saem ([`sistema::confirmar`]).
    livres: Vec<*mut u8>,
    /// Páginas soltas pela coleta, ainda com o lixo dos mortos (e na memória
    /// residente): a próxima página sai delas, zerada à mão. A coleta completa
    /// guarda no máximo [`PAGINAS_QUENTES`] (as soltas mais recentes) e devolve
    /// ao sistema as demais e as que ficaram sem uso desde a completa anterior
    /// ([`ReservaDePaginas::aparar`]).
    sujas: Vec<*mut u8>,
    /// Quantas das `sujas` (as do fundo) ficaram sem uso desde o último
    /// [`ReservaDePaginas::aparar`].
    paradas: usize,
}

/// Páginas de cada pedaço que a [`ReservaDePaginas`] reserva (2 MiB).
const PAGINAS_POR_PEDACO: usize = 32;

/// Quantas páginas sujas a reserva guarda de uma coleta completa à outra: as
/// que cabem no semiespaço jovem (`LIMITE_JOVEM`, 2 MiB), o que a próxima
/// coleta menor pode pedir. A Julia guarda o intervalo de coleta
/// (`gc_free_pages`, `default_collect_interval`).
const PAGINAS_QUENTES: usize = 32;

/// Teto dos bytes do cache das regiões grandes soltas ([`RegioesGrandes`]).
const TETO_DO_CACHE_DE_GRANDES: usize = 4 * 1024 * 1024;

#[cfg(unix)]
mod mapeamento {
    unsafe extern "C" {
        fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
        fn munmap(addr: *mut u8, len: usize) -> i32;
        #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
        fn madvise(addr: *mut u8, len: usize, conselho: i32) -> i32;
    }
    /// `MAP_ANONYMOUS`: 0x20 no Linux, 0x1000 no macOS e nos BSD.
    #[cfg(target_os = "linux")]
    const MAP_ANON: i32 = 0x20;
    #[cfg(not(target_os = "linux"))]
    const MAP_ANON: i32 = 0x1000;
    const PROT_RW: i32 = 1 | 2;
    const MAP_PRIVATE: i32 = 2;
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    const MAP_FIXED: i32 = 0x10;
    /// `bytes` novos, zerados, alinhados a `alinhamento` (potência de 2).
    pub fn mapear(bytes: usize, alinhamento: usize) -> *mut u8 {
        let total = bytes + alinhamento;
        // SAFETY: mapeamento anônimo novo.
        let p = unsafe { mmap(std::ptr::null_mut(), total, PROT_RW, MAP_PRIVATE | MAP_ANON, -1, 0) };
        if p as isize == -1 || p.is_null() {
            std::alloc::handle_alloc_error(std::alloc::Layout::from_size_align(bytes, alinhamento).expect("layout"));
        }
        let inicio = (p as usize).next_multiple_of(alinhamento);
        let antes = inicio - p as usize;
        let depois = total - antes - bytes;
        // SAFETY: as sobras do próprio mapeamento, fora de `[inicio, inicio + bytes)`.
        unsafe {
            if antes > 0 {
                munmap(p, antes);
            }
            if depois > 0 {
                munmap((inicio + bytes) as *mut u8, depois);
            }
        }
        inicio as *mut u8
    }
    /// Desfaz o mapeamento de `[p, p + bytes)`.
    ///
    /// # Safety
    /// `[p, p + bytes)` veio de [`mapear`] e ninguém mais o usa.
    pub unsafe fn desmapear(p: *mut u8, bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { munmap(p, bytes) };
    }
    /// Devolve ao sistema a memória de `[p, p + bytes)`, que volta zerada: no
    /// Linux, `madvise(MADV_DONTNEED)`; nos demais Unix (onde o
    /// `MADV_DONTNEED` não garante zero), um mapeamento anônimo novo por cima.
    ///
    /// # Safety
    /// `[p, p + bytes)` é mapeamento anônimo privado sem nada vivo.
    pub unsafe fn descartar(p: *mut u8, bytes: usize) {
        #[cfg(target_os = "linux")]
        {
            const MADV_DONTNEED: i32 = 4;
            // SAFETY: o contrato da função.
            if unsafe { madvise(p, bytes, MADV_DONTNEED) } == 0 {
                return;
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            // SAFETY: o contrato da função; o mapeamento novo substitui o
            // trecho no mesmo endereço.
            let q = unsafe { mmap(p, bytes, PROT_RW, MAP_PRIVATE | MAP_ANON | MAP_FIXED, -1, 0) };
            if q == p {
                return;
            }
        }
        // Sem o descarte a memória não volta zerada: zera à mão.
        // SAFETY: o contrato da função.
        unsafe { std::ptr::write_bytes(p, 0, bytes) };
    }
}

/// A memória do sistema: as regiões grandes (reservadas e confirmadas de uma
/// vez, zeradas, alinhadas a [`PAGINA`], com a granularidade de página do
/// sistema — um objeto de 20 KiB ocupa 24 KiB; o resto do endereço alinhado
/// não é confirmado) e os pedaços de páginas da [`ReservaDePaginas`]
/// (reservados, confirmados página a página no Windows).
mod sistema {
    use super::PAGINA;

    #[cfg(windows)]
    unsafe extern "system" {
        fn VirtualAlloc(endereco: *mut u8, bytes: usize, tipo: u32, protecao: u32) -> *mut u8;
        fn VirtualFree(endereco: *mut u8, bytes: usize, tipo: u32) -> i32;
    }
    #[cfg(windows)]
    const MEM_COMMIT: u32 = 0x1000;
    #[cfg(windows)]
    const MEM_RESERVE: u32 = 0x2000;
    #[cfg(windows)]
    const MEM_DECOMMIT: u32 = 0x4000;
    #[cfg(windows)]
    const MEM_RELEASE: u32 = 0x8000;
    #[cfg(windows)]
    const PAGE_READWRITE: u32 = 0x04;

    /// `VirtualAlloc` de `bytes` com `tipo`; aborta sem memória.
    #[cfg(windows)]
    fn virtual_alloc(endereco: *mut u8, bytes: usize, tipo: u32) -> *mut u8 {
        // SAFETY: reserva nova (`endereco` nulo) ou confirmação dentro de uma
        // reserva deste módulo.
        let p = unsafe { VirtualAlloc(endereco, bytes, tipo, PAGE_READWRITE) };
        if p.is_null() {
            std::alloc::handle_alloc_error(std::alloc::Layout::from_size_align(bytes, PAGINA).expect("layout"));
        }
        p
    }

    /// `bytes` zerados alinhados a [`PAGINA`], confirmados.
    #[cfg(windows)]
    pub fn reservar(bytes: usize) -> *mut u8 {
        // A granularidade de alocação do Windows (64 KiB) dá o alinhamento
        // de uma página do espaço.
        let p = virtual_alloc(std::ptr::null_mut(), bytes, MEM_RESERVE | MEM_COMMIT);
        assert!(p as usize % PAGINA == 0, "VirtualAlloc sem o alinhamento de 64 KiB");
        p
    }
    /// Devolve `[p, p + bytes)` ao sistema.
    ///
    /// # Safety
    /// Veio de [`reservar`] ou de [`reservar_pedaco`] e ninguém mais o usa.
    #[cfg(windows)]
    pub unsafe fn liberar(p: *mut u8, _bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { VirtualFree(p, 0, MEM_RELEASE) };
    }
    /// Um pedaço de `bytes` alinhado a [`PAGINA`] para páginas: no Windows só
    /// reservado ([`confirmar`] cada página antes do uso).
    #[cfg(windows)]
    pub fn reservar_pedaco(bytes: usize) -> *mut u8 {
        let p = virtual_alloc(std::ptr::null_mut(), bytes, MEM_RESERVE);
        assert!(p as usize % PAGINA == 0, "VirtualAlloc sem o alinhamento de 64 KiB");
        p
    }
    /// Confirma `[p, p + bytes)` de um pedaço (zerado se não estava
    /// confirmado; confirmar o já confirmado não muda nada).
    #[cfg(windows)]
    pub fn confirmar(p: *mut u8, bytes: usize) {
        virtual_alloc(p, bytes, MEM_COMMIT);
    }
    /// Devolve ao sistema a memória de `[p, p + bytes)` de um pedaço (sai da
    /// residente e da confirmada; volta zerada no próximo [`confirmar`]).
    ///
    /// # Safety
    /// `[p, p + bytes)` é de um pedaço, sem nada vivo.
    #[cfg(windows)]
    pub unsafe fn descartar(p: *mut u8, bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { VirtualFree(p, bytes, MEM_DECOMMIT) };
    }

    /// `bytes` zerados alinhados a [`PAGINA`].
    #[cfg(unix)]
    pub fn reservar(bytes: usize) -> *mut u8 {
        super::mapeamento::mapear(bytes, PAGINA)
    }
    /// Devolve `[p, p + bytes)` ao sistema.
    ///
    /// # Safety
    /// Veio de [`reservar`] ou de [`reservar_pedaco`] e ninguém mais o usa.
    #[cfg(unix)]
    pub unsafe fn liberar(p: *mut u8, bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { super::mapeamento::desmapear(p, bytes) };
    }
    /// Um pedaço de `bytes` alinhado a [`PAGINA`] para páginas (o `mmap` só
    /// ocupa memória onde se escreve).
    #[cfg(unix)]
    pub fn reservar_pedaco(bytes: usize) -> *mut u8 {
        super::mapeamento::mapear(bytes, PAGINA)
    }
    /// Nada a confirmar no Unix.
    #[cfg(unix)]
    pub fn confirmar(_p: *mut u8, _bytes: usize) {}
    /// Devolve ao sistema a memória de `[p, p + bytes)` de um pedaço (volta
    /// zerada).
    ///
    /// # Safety
    /// `[p, p + bytes)` é de um pedaço, sem nada vivo.
    #[cfg(unix)]
    pub unsafe fn descartar(p: *mut u8, bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { super::mapeamento::descartar(p, bytes) };
    }

    /// `bytes` zerados alinhados a [`PAGINA`].
    #[cfg(not(any(windows, unix)))]
    pub fn reservar(bytes: usize) -> *mut u8 {
        let layout = std::alloc::Layout::from_size_align(bytes, PAGINA).expect("layout da região");
        // SAFETY: layout de tamanho não nulo.
        let p = unsafe { std::alloc::alloc_zeroed(layout) };
        if p.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        p
    }
    /// Devolve `[p, p + bytes)` ao sistema.
    ///
    /// # Safety
    /// Veio de [`reservar`] ou de [`reservar_pedaco`] e ninguém mais o usa.
    #[cfg(not(any(windows, unix)))]
    pub unsafe fn liberar(p: *mut u8, bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { std::alloc::dealloc(p, std::alloc::Layout::from_size_align(bytes, PAGINA).expect("layout da região")) };
    }
    /// Um pedaço de `bytes` zerados alinhado a [`PAGINA`].
    #[cfg(not(any(windows, unix)))]
    pub fn reservar_pedaco(bytes: usize) -> *mut u8 {
        reservar(bytes)
    }
    /// Nada a confirmar.
    #[cfg(not(any(windows, unix)))]
    pub fn confirmar(_p: *mut u8, _bytes: usize) {}
    /// Zera `[p, p + bytes)` (sem como devolver ao sistema).
    ///
    /// # Safety
    /// `[p, p + bytes)` é de um pedaço, sem nada vivo.
    #[cfg(not(any(windows, unix)))]
    pub unsafe fn descartar(p: *mut u8, bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { std::ptr::write_bytes(p, 0, bytes) };
    }
}

impl ReservaDePaginas {
    /// Uma página zerada de [`PAGINA`] bytes, alinhada a [`PAGINA`].
    fn pedir(&mut self) -> *mut u8 {
        if let Some(p) = self.sujas.pop() {
            self.paradas = self.paradas.min(self.sujas.len());
            // SAFETY: página da reserva, sem uso.
            unsafe { std::ptr::write_bytes(p, 0, PAGINA) };
            return p;
        }
        if self.livres.is_empty() {
            let bytes = PAGINA * PAGINAS_POR_PEDACO;
            let inicio = sistema::reservar_pedaco(bytes);
            self.pedacos.push((inicio, bytes));
            // As de endereço baixo saem primeiro (`pop`).
            for i in (0..PAGINAS_POR_PEDACO).rev() {
                self.livres.push(inicio.wrapping_add(i * PAGINA));
            }
        }
        let p = self.livres.pop().expect("pedaço novo sem páginas");
        sistema::confirmar(p, PAGINA);
        p
    }
    /// Devolve a página `p` (sem objeto vivo) à reserva, suja.
    fn devolver(&mut self, p: *mut u8) {
        self.sujas.push(p);
    }

    /// A cada coleta completa: as sujas paradas desde a chamada anterior, e as
    /// que passam de [`PAGINAS_QUENTES`] (as soltas há mais tempo), voltam ao
    /// sistema ([`sistema::descartar`]: saem da memória residente e voltam
    /// zeradas).
    fn aparar(&mut self) {
        let k = self.paradas.max(self.sujas.len().saturating_sub(PAGINAS_QUENTES)).min(self.sujas.len());
        for p in self.sujas.drain(..k) {
            // SAFETY: página de um pedaço, sem uso.
            unsafe { sistema::descartar(p, PAGINA) };
            self.livres.push(p);
        }
        self.paradas = self.sujas.len();
    }
}

impl Drop for ReservaDePaginas {
    fn drop(&mut self) {
        for &(p, bytes) in &self.pedacos {
            // SAFETY: o pedaço veio de `reservar_pedaco`; o espaço que o usava
            // acabou.
            unsafe { sistema::liberar(p, bytes) };
        }
    }
}

/// O cache das regiões grandes soltas, por tamanho: um programa que refaz um
/// objeto grande a cada rodada (o buffer, a lista) reusa a região em vez de
/// pedi-la e devolvê-la ao sistema a cada vez. Tem teto
/// ([`TETO_DO_CACHE_DE_GRANDES`]; as mais antigas saem primeiro), e as que
/// ficam sem uso de uma coleta completa à outra voltam ao sistema: a VM não
/// guarda página grande nenhuma (`CanUseCache`, `page.cc`), e a região morta
/// na coleta menor vem para cá na hora (docs/NATIVO-PLANO.md §14).
#[derive(Default)]
struct RegioesGrandes {
    /// `(bytes, início)`, as mais antigas primeiro.
    soltas: Vec<(usize, *mut u8)>,
    /// Quantas das `soltas` (as do começo) ficaram sem uso desde o último
    /// [`RegioesGrandes::aparar`].
    paradas: usize,
    /// Bytes das `soltas`.
    bytes: usize,
}

impl RegioesGrandes {
    /// Uma região zerada de `bytes` bytes, alinhada a [`PAGINA`].
    fn pedir(&mut self, bytes: usize) -> *mut u8 {
        if let Some(i) = self.soltas.iter().rposition(|&(b, _)| b == bytes) {
            let (_, p) = self.soltas.remove(i);
            if i < self.paradas {
                self.paradas -= 1;
            }
            self.bytes -= bytes;
            // SAFETY: região do cache, sem uso.
            unsafe { std::ptr::write_bytes(p, 0, bytes) };
            return p;
        }
        sistema::reservar(bytes)
    }
    /// Guarda a região `p` de `bytes` bytes (sem objeto vivo); acima do teto,
    /// as mais antigas voltam ao sistema (a que sozinha passa dele, direto).
    fn devolver(&mut self, p: *mut u8, bytes: usize) {
        if bytes > TETO_DO_CACHE_DE_GRANDES {
            // SAFETY: região sem objeto vivo.
            unsafe { sistema::liberar(p, bytes) };
            return;
        }
        self.soltas.push((bytes, p));
        self.bytes += bytes;
        let mut k = 0;
        while self.bytes > TETO_DO_CACHE_DE_GRANDES {
            let (b, q) = self.soltas[k];
            self.bytes -= b;
            // SAFETY: região do cache, sem uso.
            unsafe { sistema::liberar(q, b) };
            k += 1;
        }
        if k > 0 {
            self.soltas.drain(..k);
            self.paradas = self.paradas.saturating_sub(k);
        }
    }
    /// As soltas sem uso desde a chamada anterior voltam ao sistema.
    fn aparar(&mut self) {
        let k = self.paradas.min(self.soltas.len());
        for (bytes, p) in self.soltas.drain(..k) {
            self.bytes -= bytes;
            // SAFETY: região do cache, sem uso.
            unsafe { sistema::liberar(p, bytes) };
        }
        self.paradas = self.soltas.len();
    }
}

impl Drop for RegioesGrandes {
    fn drop(&mut self) {
        for &(bytes, p) in &self.soltas {
            // SAFETY: região do cache, sem uso; o espaço acabou.
            unsafe { sistema::liberar(p, bytes) };
        }
    }
}

/// Um anexo nativo (`ANEXO`, §2.8): o dono, a função que solta o ponteiro, o
/// ponteiro e os bytes que ele ocupa fora do heap.
#[derive(Clone, Copy)]
pub(crate) struct AnexoNativo {
    pub bloco: *mut Cabecalho,
    pub soltar: unsafe fn(*mut u8),
    pub ptr: *mut u8,
    pub bytes: usize,
}

// ─── O espaço ──────────────────────────────────────────────────────────────

/// O espaço de objetos: páginas de blocos por classe de tamanho, regiões grandes
/// e as faixas livres (trechos contíguos de blocos zerados) por classe, refeitas
/// a cada coleta completa (em ordem de endereço).
pub struct EspacoDeObjetos {
    pub(crate) paginas: Vec<Pagina>,
    /// A página de cada base (a validação de um handle).
    mapa: MapaDePaginas,
    /// As faixas `[início, fim)` de blocos livres de cada classe, zeradas
    /// (cabeçalho inclusive) ou com o lixo dos mortos (zeradas ao sair).
    livres: Vec<Vec<(*mut u8, *mut u8)>>,
    /// Blocos vivos (entregues e não devolvidos).
    pub vivos: usize,
    /// Blocos em todas as páginas.
    pub blocos: usize,
    /// Os blocos entregues um a um pelo runtime desde a última coleta, com a
    /// classe ([`GRANDE`] nas regiões grandes): com
    /// [`EspacoDeObjetos::faixas`], o que a coleta menor varre.
    jovens: Vec<(*mut Cabecalho, usize)>,
    /// As faixas `(início, fim, classe)` entregues às TLABs desde a última
    /// coleta.
    faixas: Vec<(*mut u8, *mut u8, usize)>,
    /// A região ainda não usada `[cursor, fim)` da página em uso de cada classe
    /// (o *bump pointer*).
    regiao: Vec<(*mut u8, *mut u8)>,
    /// As páginas sem objeto vivo guardadas pela coleta completa (índices em
    /// `paginas`), por zerar.
    vazias: Vec<usize>,
    /// Os velhos que a barreira de escrita marcou ([`LEMBRADO`]).
    pub(crate) lembrados: Vec<*mut Cabecalho>,
    /// Blocos entregues desde a última coleta, por classe.
    demanda: Vec<usize>,
    /// O pico recente de blocos ocupados de cada classe, que decai a cada coleta
    /// completa.
    pico: Vec<usize>,
    /// Os objetos com corpo de fora ([`FORA`]).
    pub(crate) com_fora: Vec<*mut Cabecalho>,
    /// A região de cada classe veio de uma faixa livre (lixo dos mortos) ou de
    /// uma página zerada.
    regiao_suja: Vec<bool>,
    /// Blocos entregues e não soltos de cada classe.
    em_uso: Vec<usize>,
    /// Zerar os mortos já na varredura (`--gc-stress` e a verificação).
    pub(crate) zerar_mortos: bool,
    /// Blocos entregues desde a última coleta (menos os devolvidos).
    entregues: usize,
    /// Objetos do espaço marcados pela coleta em curso.
    pub(crate) marcados: usize,
    /// A quarentena dos mortos (`DARTFORGE_GC_VENENO`); `None` sem ela.
    quarentena: Option<Quarentena>,
    /// De onde vêm as páginas de [`PAGINA`] bytes.
    reserva: ReservaDePaginas,
    /// O cache das regiões grandes soltas.
    grandes: RegioesGrandes,
    /// Os anexos nativos dos blocos vivos (ou mortos desde a última coleta),
    /// pelo endereço do bloco.
    anexos: crate::hash::HashMap<usize, AnexoNativo>,
    /// Bytes dos anexos.
    pub(crate) bytes_de_anexos: usize,
}

impl std::fmt::Debug for EspacoDeObjetos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EspacoDeObjetos").field("paginas", &self.paginas.len()).field("vivos", &self.vivos).finish()
    }
}

impl EspacoDeObjetos {
    pub(crate) fn new(zerar_mortos: bool) -> Self {
        Self {
            paginas: Vec::new(),
            mapa: MapaDePaginas::default(),
            livres: vec![Vec::new(); N_CLASSES],
            vivos: 0,
            blocos: 0,
            jovens: Vec::new(),
            faixas: Vec::new(),
            regiao: vec![(std::ptr::null_mut(), std::ptr::null_mut()); N_CLASSES],
            vazias: Vec::new(),
            lembrados: Vec::new(),
            demanda: vec![0; N_CLASSES],
            pico: vec![0; N_CLASSES],
            com_fora: Vec::new(),
            regiao_suja: vec![false; N_CLASSES],
            em_uso: vec![0; N_CLASSES],
            zerar_mortos,
            entregues: 0,
            marcados: 0,
            quarentena: Quarentena::do_ambiente(),
            reserva: ReservaDePaginas::default(),
            grandes: RegioesGrandes::default(),
            anexos: crate::hash::HashMap::default(),
            bytes_de_anexos: 0,
        }
    }

    /// Uma página zerada para blocos da classe `classe` (de uma página): uma
    /// vazia reusada (zerada e formatada agora) ou uma nova. Devolve o índice.
    fn pagina_zerada(&mut self, classe: usize) -> usize {
        debug_assert!(classe != GRANDE);
        let palavras = palavras_da_classe(classe);
        if let Some(i) = self.vazias.pop() {
            let p = &mut self.paginas[i];
            // SAFETY: a página inteira é deste espaço e não tem objeto vivo.
            unsafe { std::ptr::write_bytes(p.base, 0, p.bytes) };
            self.blocos -= p.blocos;
            *p = Pagina::formatada(p.base, p.bytes, classe, palavras);
            self.blocos += p.blocos;
            return i;
        }
        let base = self.reserva.pedir();
        self.nova_pagina(Pagina::formatada(base, PAGINA, classe, palavras))
    }

    /// Uma região grande zerada para um corpo de `palavras` palavras. Devolve o
    /// índice.
    fn pagina_grande(&mut self, palavras: usize) -> usize {
        let bytes = bytes_da_regiao(palavras);
        let base = self.grandes.pedir(bytes);
        self.nova_pagina(Pagina::formatada(base, bytes, GRANDE, palavras))
    }

    fn nova_pagina(&mut self, pagina: Pagina) -> usize {
        self.blocos += pagina.blocos;
        self.mapa.inserir(pagina.base as usize, self.paginas.len());
        self.paginas.push(pagina);
        self.paginas.len() - 1
    }

    /// Tira a página `i` de `paginas` (a última toma o lugar dela) e do mapa de
    /// páginas, sem soltá-la: devolve-a.
    fn remover_pagina(&mut self, i: usize) -> Pagina {
        let p = self.paginas.swap_remove(i);
        self.mapa.remover(p.base as usize);
        self.blocos -= p.blocos;
        let antiga = self.paginas.len();
        if i < antiga {
            // A que estava em `antiga` agora está em `i`.
            self.mapa.inserir(self.paginas[i].base as usize, i);
            if let Some(v) = self.vazias.iter_mut().find(|v| **v == antiga) {
                *v = i;
            }
        }
        p
    }

    /// Devolve a página `p` (sem objeto vivo): à reserva, ou ao cache das
    /// regiões grandes.
    fn soltar_pagina(&mut self, p: &Pagina) {
        if p.classe == GRANDE {
            self.grandes.devolver(p.base, p.bytes);
        } else {
            self.reserva.devolver(p.base);
        }
    }

    /// Até `k` blocos zerados e contíguos da classe `classe` tirados da região
    /// dela; quando ela acaba, a região passa a ser uma faixa livre da classe ou
    /// uma página zerada. Devolve o início e quantos. O cabeçalho sai zerado.
    ///
    /// As faixas livres guardam o lixo dos mortos (a varredura não os toca): o
    /// trecho entregue é zerado aqui, logo antes de o programa o usar.
    fn tirar_da_regiao(&mut self, classe: usize, k: usize) -> (*mut u8, usize) {
        let tamanho = bytes_do_bloco(palavras_da_classe(classe));
        let (mut cursor, mut fim) = self.regiao[classe];
        if (fim as usize).saturating_sub(cursor as usize) < tamanho {
            (cursor, fim) = match self.livres[classe].pop() {
                Some(faixa) => {
                    self.regiao_suja[classe] = true;
                    faixa
                }
                None => {
                    let i = self.pagina_zerada(classe);
                    self.regiao_suja[classe] = false;
                    let p = &self.paginas[i];
                    (p.inicio(), p.inicio().wrapping_add(p.blocos * tamanho))
                }
            };
        }
        let j = k.min((fim as usize - cursor as usize) / tamanho);
        self.regiao[classe] = (cursor.wrapping_add(j * tamanho), fim);
        if self.regiao_suja[classe] {
            // SAFETY: blocos livres da classe, sem objeto vivo.
            unsafe { std::ptr::write_bytes(cursor, 0, j * tamanho) };
        }
        (cursor, j)
    }

    /// Uma faixa de até `k` blocos livres, zerados e contíguos da classe
    /// `classe` para a TLAB: o início e quantos.
    pub(crate) fn tirar_faixa(&mut self, classe: usize, k: usize) -> (*mut u8, usize) {
        debug_assert!(classe != GRANDE && classe <= MAIOR_CLASSE && k > 0);
        let (inicio, j) = self.tirar_da_regiao(classe, k);
        self.faixas.push((inicio, inicio.wrapping_add(j * bytes_do_bloco(palavras_da_classe(classe))), classe));
        self.demanda[classe] += j;
        self.entregues += j;
        self.em_uso[classe] += j;
        (inicio, j)
    }

    /// Devolve os blocos zerados `[inicio, fim)` da classe (o resto não usado da
    /// última faixa da TLAB): de volta à região, se ela continua dali; senão, às
    /// faixas livres. A faixa entregue encolhe junto.
    pub(crate) fn devolver_faixa(&mut self, classe: usize, inicio: *mut u8, fim: *mut u8) {
        let k = (fim as usize - inicio as usize) / bytes_do_bloco(palavras_da_classe(classe));
        self.entregues -= k;
        self.em_uso[classe] -= k;
        if let Some(f) = self.faixas.iter_mut().rev().find(|f| f.2 == classe && f.1 == fim) {
            f.1 = inicio;
        }
        if self.regiao[classe].0 == fim {
            self.regiao[classe].0 = inicio;
        } else {
            self.livres[classe].push((inicio, fim));
        }
    }

    /// Devolve às faixas livres os blocos mortos `[inicio, fim)` da classe, sem
    /// tocá-los — no `--gc-stress` e com a verificação, zerados já (o estado
    /// [`LIVRE`] deixa o handle velho ser apanhado, N4).
    ///
    /// # Safety
    /// `[inicio, fim)` são blocos da classe de uma página deste espaço, sem
    /// objeto vivo.
    unsafe fn soltar_faixa(&mut self, classe: usize, inicio: *mut u8, fim: *mut u8) {
        // Com o veneno, cada bloco entra na quarentena; volta à lista livre
        // o que passou da capacidade dela.
        if let Some(q) = self.quarentena.as_mut() {
            let tamanho = bytes_do_bloco(palavras_da_classe(classe));
            let mut b = inicio;
            while b < fim {
                // SAFETY: o contrato da função.
                unsafe { q.prender(classe, b, tamanho) };
                b = b.wrapping_add(tamanho);
            }
            for (c, b) in q.soltos() {
                let t = bytes_do_bloco(palavras_da_classe(c));
                self.livres[c].push((b, b.wrapping_add(t)));
            }
            return;
        }
        if self.zerar_mortos {
            // SAFETY: o contrato da função.
            unsafe { std::ptr::write_bytes(inicio, 0, fim as usize - inicio as usize) };
        }
        self.livres[classe].push((inicio, fim));
    }

    /// Marca o velho `b` como [`LEMBRADO`] (a barreira de escrita).
    #[inline]
    pub(crate) fn lembrar(&mut self, b: *mut Cabecalho) {
        // SAFETY: bloco do espaço.
        unsafe {
            if (*b).estado == VELHO {
                (*b).estado = LEMBRADO;
                self.lembrados.push(b);
            }
        }
    }

    /// Solta os corpos de fora e os anexos dos objetos que a coleta não marcou
    /// (o bit no mapa de marcas da página): devolve os bytes soltos.
    fn soltar_mortos_de_fora(&mut self) -> usize {
        let mut soltos = 0;
        self.com_fora.retain(|&b| {
            // SAFETY: bloco com corpo de fora, de uma página viva.
            unsafe {
                if marcado(b) {
                    return true;
                }
                soltos += tamanho_do_bloco(usize::from((*corpo(b)).n));
                Self::soltar_corpo(b);
            }
            false
        });
        let mut de_anexos = 0;
        self.anexos.retain(|_, a| {
            // SAFETY: bloco com anexo, de uma página viva.
            if unsafe { marcado(a.bloco) } {
                return true;
            }
            // SAFETY: o contrato de `Heap::anexar`: `soltar(ptr)` é a única
            // liberação de `ptr`, e o dono morreu.
            unsafe { (a.soltar)(a.ptr) };
            de_anexos += a.bytes;
            false
        });
        self.bytes_de_anexos -= de_anexos;
        soltos + de_anexos
    }

    /// Bytes dos corpos de fora dos objetos vivos.
    fn bytes_de_fora(&self) -> usize {
        // SAFETY: blocos com corpo de fora, vivos.
        self.com_fora.iter().map(|&b| unsafe { tamanho_do_bloco(usize::from((*corpo(b)).n)) }).sum()
    }

    /// Anexa `ptr` ao bloco `b` (ver `Heap::anexar`).
    pub(crate) fn anexar(&mut self, b: *mut Cabecalho, soltar: unsafe fn(*mut u8), ptr: *mut u8, bytes: usize) {
        self.anexos.insert(b as usize, AnexoNativo { bloco: b, soltar, ptr, bytes });
        self.bytes_de_anexos += bytes;
    }

    /// Os bytes do anexo de `b` passam a ser `bytes` (o anexo cresceu ou
    /// encolheu); devolve a diferença.
    pub(crate) fn ajustar_anexo(&mut self, b: *mut Cabecalho, bytes: usize) -> isize {
        let a = self.anexos.get_mut(&(b as usize)).expect("bug do runtime: ajustar_anexo sem anexo");
        let delta = bytes as isize - a.bytes as isize;
        a.bytes = bytes;
        self.bytes_de_anexos = (self.bytes_de_anexos as isize + delta) as usize;
        delta
    }

    /// A varredura da coleta menor, só pelo mapa de marcas (a marcação acendeu o
    /// bit dos jovens alcançados, já velhos): cada trecho de jovens sem bit numa
    /// faixa entregue volta às faixas livres sem ser tocado; a região grande de
    /// um jovem morto volta ao cache delas. Os lembrados voltam a velhos.
    /// Devolve (mortos, bytes soltos).
    /// Os jovens que a marcação menor em curso alcançou (os promovidos): os
    /// entregues um a um e os blocos marcados das faixas das TLABs. Antes de
    /// [`EspacoDeObjetos::varrer_jovens`] (o ARC os registra,
    /// docs/ARC-IMPLEMENTACAO.md).
    pub(crate) fn jovens_marcados(&self, f: &mut dyn FnMut(*mut Cabecalho)) {
        for &(b, _) in &self.jovens {
            // SAFETY: bloco entregue desde a última coleta, numa página viva.
            if unsafe { marcado(b) } {
                f(b);
            }
        }
        for &(inicio, fim, classe) in &self.faixas {
            let tamanho = bytes_do_bloco(palavras_da_classe(classe));
            let mut p = inicio;
            while p < fim {
                // SAFETY: bloco da faixa, numa página viva.
                if unsafe { marcado(p.cast()) } {
                    f(p.cast());
                }
                p = p.wrapping_add(tamanho);
            }
        }
    }

    /// Visita cada bloco entregue desde a última coleta e ainda ocupado
    /// (os um a um e os das faixas das TLABs; um bloco da faixa que a TLAB
    /// não chegou a entregar está zerado, [`LIVRE`]).
    pub(crate) fn jovens_entregues(&self, f: &mut dyn FnMut(*mut Cabecalho)) {
        for &(b, _) in &self.jovens {
            // SAFETY: bloco entregue desde a última coleta, numa página viva.
            if unsafe { (*b).estado } == JOVEM {
                f(b);
            }
        }
        for &(inicio, fim, classe) in &self.faixas {
            let tamanho = bytes_do_bloco(palavras_da_classe(classe));
            let mut p = inicio;
            while p < fim {
                let b: *mut Cabecalho = p.cast();
                // SAFETY: bloco da faixa, numa página viva.
                if unsafe { (*b).estado } == JOVEM {
                    f(b);
                }
                p = p.wrapping_add(tamanho);
            }
        }
    }

    /// Solta o bloco `b`, morto pelo RC (o ARC puro, fora de qualquer
    /// varredura): o corpo de fora, o anexo, o bit de marca e o bloco, que
    /// volta às faixas livres da classe (o objeto grande devolve a página).
    /// Devolve os bytes soltos.
    ///
    /// # Safety
    /// `b` é bloco ocupado de uma página deste espaço, fora da lista dos
    /// jovens, cujo objeto morreu e que ninguém mais lê.
    pub(crate) unsafe fn soltar_morto(&mut self, b: *mut Cabecalho) -> usize {
        let mut soltos = 0;
        // SAFETY: o contrato da função.
        unsafe {
            if (*b).flags & FORA != 0 {
                soltos += tamanho_do_bloco(usize::from((*corpo(b)).n));
                self.com_fora.retain(|&x| x != b);
                Self::soltar_corpo(b);
            }
            if let Some(a) = self.anexos.remove(&(b as usize)) {
                (a.soltar)(a.ptr);
                self.bytes_de_anexos -= a.bytes;
                soltos += a.bytes;
            }
            desmarcar_bloco(b);
        }
        let Some(i) = self.mapa.get(b as usize & !(PAGINA - 1)) else { return soltos };
        let classe = self.paginas[i].classe;
        self.vivos -= 1;
        if classe == GRANDE {
            let p = self.remover_pagina(i);
            soltos += p.bytes;
            self.grandes.devolver(p.base, p.bytes);
        } else {
            let tamanho = bytes_do_bloco(palavras_da_classe(classe));
            soltos += tamanho;
            self.em_uso[classe] -= 1;
            let inicio = b.cast::<u8>();
            // SAFETY: o bloco morto da classe, sem objeto vivo.
            unsafe { self.soltar_faixa(classe, inicio, inicio.wrapping_add(tamanho)) };
        }
        soltos
    }

    /// A geometria da página do bloco do handle `h`: o endereço do primeiro
    /// bloco, os bytes de cada um e quantos cabem (a tabela de metadados do
    /// ARC por página, `arc::Geometria`). `None` fora do espaço.
    pub(crate) fn geometria(&self, h: i64) -> Option<(u64, u64, u32)> {
        let a = (h - DESLOCAMENTO_DO_HANDLE) as usize;
        let i = self.mapa.get(a & !(PAGINA - 1))?;
        let p = &self.paginas[i];
        if p.vazia {
            return None;
        }
        Some((p.inicio() as u64, p.tamanho() as u64, u32::try_from(p.blocos).ok()?))
    }

    pub(crate) fn varrer_jovens(&mut self) -> (usize, usize) {
        let (mut mortos, mut soltos) = (0, 0);
        soltos += self.soltar_mortos_de_fora();
        let jovens = std::mem::take(&mut self.jovens);
        for &(b, classe) in &jovens {
            // SAFETY: bloco entregue desde a última coleta, de uma página
            // ainda viva (as páginas só saem na coleta completa).
            unsafe {
                if marcado(b) {
                    continue;
                }
                mortos += 1;
                if classe == GRANDE {
                    // A região do objeto grande morto volta ao cache delas já
                    // (a varredura dos grandes da Julia, `sweep_big`): até a
                    // coleta completa, ela seria memória que nenhum gatilho
                    // conta (docs/NATIVO-PLANO.md §14.2).
                    if let Some(i) = self.mapa.get(b as usize & !(PAGINA - 1)) {
                        let p = self.remover_pagina(i);
                        soltos += p.bytes;
                        self.grandes.devolver(p.base, p.bytes);
                    }
                } else {
                    let tamanho = bytes_do_bloco(palavras_da_classe(classe));
                    soltos += tamanho;
                    self.em_uso[classe] -= 1;
                    let inicio = b.cast::<u8>();
                    self.soltar_faixa(classe, inicio, inicio.wrapping_add(tamanho));
                }
            }
        }
        self.jovens = jovens;
        self.jovens.clear();
        let faixas = std::mem::take(&mut self.faixas);
        // Todos os entregues marcados: nenhum morto nas faixas.
        let todos_vivos = self.marcados >= self.entregues;
        for &(inicio, fim, classe) in if todos_vivos { &faixas[..0] } else { &faixas[..] } {
            let tamanho = bytes_do_bloco(palavras_da_classe(classe));
            let mut trecho: *mut u8 = std::ptr::null_mut();
            let mut mortos_na_faixa = 0;
            let mut p = inicio;
            while p < fim {
                // SAFETY: bloco da faixa, numa página viva.
                if unsafe { marcado(p.cast()) } {
                    if !trecho.is_null() {
                        // SAFETY: blocos mortos da faixa.
                        unsafe { self.soltar_faixa(classe, trecho, p) };
                        trecho = std::ptr::null_mut();
                    }
                } else {
                    mortos_na_faixa += 1;
                    if trecho.is_null() {
                        trecho = p;
                    }
                }
                p = p.wrapping_add(tamanho);
            }
            if !trecho.is_null() {
                // SAFETY: blocos mortos da faixa.
                unsafe { self.soltar_faixa(classe, trecho, fim) };
            }
            mortos += mortos_na_faixa;
            soltos += mortos_na_faixa * tamanho;
            self.em_uso[classe] -= mortos_na_faixa;
        }
        self.faixas = faixas;
        self.faixas.clear();
        for b in std::mem::take(&mut self.lembrados) {
            // SAFETY: bloco lembrado, velho.
            unsafe { (*b).estado = VELHO };
        }
        self.vivos -= mortos;
        self.entregues = 0;
        self.marcados = 0;
        for d in self.demanda.iter_mut() {
            *d = 0;
        }
        (mortos, soltos)
    }

    /// Solta o corpo de fora do objeto morto do bloco `b`, se ele tem um.
    ///
    /// # Safety
    /// `b` é bloco deste espaço cujo objeto morreu.
    #[inline]
    unsafe fn soltar_corpo(b: *mut Cabecalho) {
        // SAFETY: o contrato da função.
        unsafe {
            if (*b).flags & FORA != 0 {
                soltar_corpo_de_fora(corpo(b));
                (*b).flags &= !FORA;
            }
        }
    }

    /// Um bloco novo de corpo de `palavras` palavras zeradas, classe `cid`,
    /// `flags` e `n` no cabeçalho (saturado em `u16::MAX`), estado `JOVEM`: o
    /// handle.
    #[inline]
    pub(crate) fn alocar(&mut self, cid: i32, palavras: usize, flags: u8, n: usize) -> i64 {
        let (b, classe) = match classe_de_tamanho(palavras) {
            Some(c) => {
                self.em_uso[c] += 1;
                self.demanda[c] += 1;
                (self.tirar_da_regiao(c, 1).0.cast::<Cabecalho>(), c)
            }
            None => {
                let i = self.pagina_grande(palavras);
                (self.paginas[i].inicio().cast::<Cabecalho>(), GRANDE)
            }
        };
        self.jovens.push((b, classe));
        self.entregues += 1;
        // SAFETY: bloco livre tirado agora, zerado.
        unsafe {
            (*b).estado = JOVEM;
            (*b).flags = flags;
            (*b).n = u16::try_from(n).unwrap_or(u16::MAX);
            (*b).class_id = cid;
            // Objeto grande que não é string nem `INSTANCIA` (`mapa` sem uso,
            // §2.3): o `mapa` guarda as palavras do corpo, para
            // `palavras_do_corpo` não ir ao mapa de páginas a cada leitura.
            if n > usize::from(u16::MAX) && flags & crate::layout::flags::FORMA != INSTANCIA && !crate::layout::cid::e_texto(cid) {
                (*b).mapa = u32::try_from(palavras).unwrap_or(0);
            }
        }
        self.vivos += 1;
        b as i64 + DESLOCAMENTO_DO_HANDLE
    }

    /// O bloco do handle `h`, se `h` é o de um bloco de alguma página deste
    /// espaço (vivo ou não: o estado fica com quem pergunta).
    #[inline]
    pub fn bloco_de(&self, h: i64) -> Option<*mut Cabecalho> {
        if !e_objeto(h) {
            return None;
        }
        let b = (h - DESLOCAMENTO_DO_HANDLE) as usize;
        let base = b & !(PAGINA - 1);
        let p = &self.paginas[self.mapa.get(base)?];
        p.indice(b - base).map(|i| p.bloco(i))
    }

    /// O endereço do handle `h` cai numa página deste espaço (viva ou vazia)?
    /// Distingue o handle de um objeto coletado do escalar usado como handle.
    pub(crate) fn na_pagina(&self, h: i64) -> bool {
        let b = (h - DESLOCAMENTO_DO_HANDLE) as usize;
        self.mapa.get(b & !(PAGINA - 1)).is_some()
    }

    /// As palavras do corpo do bloco `b` (vivo, desta página): em `INSTANCIA`, as
    /// dos campos e do mapa; nos demais, as do cabeçalho, ou as da região grande
    /// quando o `n` saturou.
    ///
    /// # Safety
    /// `b` é bloco vivo de uma página deste espaço.
    #[inline]
    pub(crate) unsafe fn palavras_do_corpo(&self, b: *const Cabecalho) -> usize {
        // SAFETY: o contrato da função.
        unsafe {
            let n = usize::from((*b).n);
            if forma(b) == INSTANCIA {
                return crate::layout::palavras_de_instancia(n);
            }
            if n != usize::from(u16::MAX) {
                return n;
            }
            // O objeto grande que não é string guarda as palavras em `mapa` (`alocar`).
            let m = (*b).mapa;
            if m != 0 && !crate::layout::cid::e_texto((*b).class_id) {
                return m as usize;
            }
        }
        self.palavras_da_regiao(b)
    }

    /// As palavras do bloco da região grande de `b` (pelo mapa de páginas).
    #[cold]
    #[inline(never)]
    fn palavras_da_regiao(&self, b: *const Cabecalho) -> usize {
        self.mapa.get(b as usize & !(PAGINA - 1)).map_or(0, |i| self.paginas[i].palavras)
    }

    /// Empilha as referências do corpo de `b` pelo formato, sem mexer em cartões
    /// (a verificação); devolve as posições percorridas.
    ///
    /// # Safety
    /// `b` é bloco vivo de uma página deste espaço.
    pub(crate) unsafe fn empilhar_corpo(&self, b: *mut Cabecalho, pilha: &mut Vec<i64>) -> usize {
        // SAFETY: o contrato da função.
        unsafe {
            match forma(b) {
                INSTANCIA => empilhar_referencias(corpo(b), pilha),
                REFS => {
                    let n = refs_do_corpo(b, self.palavras_do_corpo(b));
                    empilhar_refs(b, n, pilha)
                }
                _ => 0,
            }
        }
    }

    /// Empilha as referências de um lembrado na coleta menor: `INSTANCIA` pelo
    /// mapa; `REFS` com `CARTOES`, só os elementos dos cartões sujos (e os
    /// zera); `REFS` sem cartões, todas. Devolve as posições percorridas.
    ///
    /// # Safety
    /// `b` é bloco lembrado de uma página deste espaço.
    pub(crate) unsafe fn empilhar_lembrado(&self, b: *mut Cabecalho, pilha: &mut Vec<i64>) -> usize {
        // SAFETY: o contrato da função.
        unsafe {
            if forma(b) == REFS && (*b).flags & CARTOES != 0 {
                let n = refs_do_corpo(b, self.palavras_do_corpo(b));
                return empilhar_cartoes_sujos(b, n, pilha);
            }
            self.empilhar_corpo(b, pilha)
        }
    }

    /// Apaga o mapa de marcas de cada página (o começo da coleta completa).
    pub(crate) fn limpar_marcas(&mut self) {
        for p in self.paginas.iter().filter(|p| !p.vazia) {
            // SAFETY: o mapa de marcas da página.
            unsafe { std::ptr::write_bytes(p.base, 0, CABECA_DA_PAGINA) };
        }
    }

    /// A varredura da coleta completa, só pelo mapa de marcas: o bit aceso é
    /// vivo (a marcação já o deixou velho); cada trecho sem bit volta às faixas
    /// livres, refeitas em ordem de endereço, sem que os blocos sejam lidos. A
    /// página sem vivo fica inteira como vazia enquanto os blocos livres da classe
    /// dela não passam dos vivos, da demanda do ciclo e do pico recente (e de duas
    /// páginas); as demais voltam à reserva. A região grande sem vivo volta ao
    /// cache delas. Devolve (mortos, vivos, bytes vivos).
    pub(crate) fn varrer(&mut self) -> (usize, usize, usize) {
        let (mut vivos, mut bytes_vivos) = (0, 0);
        for r in self.regiao.iter_mut() {
            *r = (std::ptr::null_mut(), std::ptr::null_mut());
        }
        self.soltar_mortos_de_fora();
        // Por página: o trecho `[de, ate)` de `faixas_livres` com as faixas
        // livres dela e os vivos.
        let mut trechos: Vec<(usize, usize, usize)> = Vec::with_capacity(self.paginas.len());
        let mut faixas_livres: Vec<(*mut u8, *mut u8)> = Vec::new();
        let mut vivos_da_classe = vec![0usize; N_CLASSES];
        let mut livres_da_classe = vec![0usize; N_CLASSES];
        for p in &self.paginas {
            let de = faixas_livres.len();
            if p.vazia {
                trechos.push((de, de, 0));
                continue;
            }
            let tamanho = p.tamanho();
            let mut vivos_na_pagina = 0;
            // Os mortos em quarentena (`DARTFORGE_GC_VENENO`): não são vivos,
            // mas também não voltam à lista livre, e seguram a página.
            let mut presos_na_pagina = 0;
            let mut livre: *mut u8 = std::ptr::null_mut();
            let mut q = p.inicio();
            for _ in 0..p.blocos {
                // SAFETY: bloco da página.
                let vivo = unsafe { marcado(q.cast()) };
                // SAFETY: bloco sem marca da página.
                let preso = !vivo && p.classe != GRANDE && self.quarentena.as_mut().is_some_and(|qt| unsafe { qt.reter(p.classe, q, tamanho) });
                if vivo || preso {
                    if vivo {
                        vivos_na_pagina += 1;
                    } else {
                        presos_na_pagina += 1;
                    }
                    if !livre.is_null() {
                        faixas_livres.push((livre, q));
                        livre = std::ptr::null_mut();
                    }
                } else if livre.is_null() {
                    livre = q;
                }
                q = q.wrapping_add(tamanho);
            }
            if !livre.is_null() {
                faixas_livres.push((livre, q));
            }
            if vivos_na_pagina + presos_na_pagina == 0 || p.classe == GRANDE {
                faixas_livres.truncate(de);
            } else if self.zerar_mortos {
                for &(inicio, fim) in &faixas_livres[de..] {
                    // SAFETY: blocos sem objeto vivo da página.
                    unsafe { std::ptr::write_bytes(inicio, 0, fim as usize - inicio as usize) };
                }
            }
            vivos += vivos_na_pagina;
            if p.classe == GRANDE {
                bytes_vivos += vivos_na_pagina * p.bytes;
            } else {
                bytes_vivos += vivos_na_pagina * tamanho;
                vivos_da_classe[p.classe] += vivos_na_pagina;
                livres_da_classe[p.classe] += p.blocos - vivos_na_pagina - presos_na_pagina;
            }
            // O terceiro campo decide se a página fica: os presos a seguram.
            trechos.push((de, faixas_livres.len(), vivos_na_pagina + presos_na_pagina));
        }
        bytes_vivos += self.bytes_de_fora() + self.bytes_de_anexos;
        let mortos = self.vivos.saturating_sub(vivos);
        for (c, p) in self.pico.iter_mut().enumerate() {
            // Os blocos ocupados (vivos e mortos desta coleta): o tamanho que o
            // heap teve neste ciclo.
            *p = (*p - *p / 8).max(self.em_uso[c]);
        }
        self.em_uso.copy_from_slice(&vivos_da_classe);
        // As páginas vazias que passam da folga voltam à reserva (as últimas
        // primeiro: as faixas preferem os endereços baixos); as já vazias de
        // antes contam como folga de todas as classes.
        let mut solta = vec![false; self.paginas.len()];
        let mut guardadas = 0usize;
        for i in (0..self.paginas.len()).rev() {
            let p = &self.paginas[i];
            if trechos[i].2 != 0 {
                continue;
            }
            if p.classe == GRANDE {
                solta[i] = true;
                continue;
            }
            if p.vazia {
                // Vazia desde a coleta anterior e não reusada: sobra.
                if guardadas >= 2 {
                    solta[i] = true;
                } else {
                    guardadas += 1;
                }
                continue;
            }
            // A folga: o que sobreviveu, o que se alocou desde a última coleta
            // (a demanda do próximo ciclo), o pico recente e duas páginas.
            let folga = vivos_da_classe[p.classe].max(self.demanda[p.classe]).max(self.pico[p.classe]).max(2 * p.blocos);
            if livres_da_classe[p.classe] >= folga + p.blocos {
                livres_da_classe[p.classe] -= p.blocos;
                solta[i] = true;
            }
        }
        for l in self.livres.iter_mut() {
            l.clear();
        }
        self.vazias.clear();
        let paginas = std::mem::take(&mut self.paginas);
        for (i, mut p) in paginas.into_iter().enumerate() {
            if solta[i] {
                self.mapa.remover(p.base as usize);
                self.blocos -= p.blocos;
                self.soltar_pagina(&p);
                continue;
            }
            let (de, ate, vivos_na_pagina) = trechos[i];
            if vivos_na_pagina == 0 {
                p.vazia = true;
                self.vazias.push(self.paginas.len());
            } else if p.classe != GRANDE {
                self.livres[p.classe].extend_from_slice(&faixas_livres[de..ate]);
            }
            self.mapa.inserir(p.base as usize, self.paginas.len());
            self.paginas.push(p);
        }
        // As faixas de endereço baixo saem primeiro (`pop`).
        for l in self.livres.iter_mut() {
            l.reverse();
        }
        // A quarentena: o que passou da capacidade volta à lista livre.
        if let Some(q) = self.quarentena.as_mut() {
            for (c, b) in q.soltos() {
                let t = bytes_do_bloco(palavras_da_classe(c));
                self.livres[c].push((b, b.wrapping_add(t)));
            }
        }
        // As vazias de endereço baixo saem primeiro (`pop`).
        self.vazias.reverse();
        self.reserva.aparar();
        self.grandes.aparar();
        self.vivos = vivos;
        self.entregues = 0;
        self.marcados = 0;
        self.jovens.clear();
        self.faixas.clear();
        self.lembrados.clear();
        for d in self.demanda.iter_mut() {
            *d = 0;
        }
        (mortos, vivos, bytes_vivos)
    }

    /// Zera as faixas livres e o resto sujo das regiões: depois disso, todo bloco
    /// sem objeto tem o estado [`LIVRE`].
    fn limpar_livres(&mut self) {
        for faixas in &self.livres {
            for &(inicio, fim) in faixas {
                // SAFETY: blocos livres de uma página do espaço.
                unsafe { std::ptr::write_bytes(inicio, 0, fim as usize - inicio as usize) };
            }
        }
        for (c, &(cursor, fim)) in self.regiao.iter().enumerate() {
            if self.regiao_suja[c] && !cursor.is_null() {
                // SAFETY: blocos livres da região da classe.
                unsafe { std::ptr::write_bytes(cursor, 0, fim as usize - cursor as usize) };
            }
        }
    }

    /// Visita o bloco de cada objeto vivo (ou morto desde a última coleta).
    pub(crate) fn para_cada_vivo(&mut self, mut f: impl FnMut(*mut Cabecalho)) {
        self.limpar_livres();
        for p in self.paginas.iter().filter(|p| !p.vazia) {
            for j in 0..p.blocos {
                let b = p.bloco(j);
                // SAFETY: bloco da página.
                let estado = unsafe { (*b).estado };
                if estado != LIVRE && estado != ESTADO_DE_VENENO {
                    f(b);
                }
            }
        }
    }

    /// Quantas páginas e regiões (para o rastro da coleta).
    pub(crate) fn n_paginas(&self) -> usize {
        self.paginas.len()
    }

    /// Bytes das regiões grandes no cache (fora do heap vivo).
    #[allow(dead_code)]
    pub(crate) fn bytes_em_cache(&self) -> usize {
        self.grandes.bytes
    }

    /// A quebra da memória do espaço (`DARTFORGE_GC_MEMORIA=1`): por classe de
    /// tamanho, as páginas, os blocos e os ocupados (os vivos da última coleta
    /// completa mais os entregues desde então); as páginas vazias, as da
    /// reserva, as regiões grandes vivas e em cache, os corpos de fora e os
    /// anexos. Uma linha por item, em KiB.
    pub(crate) fn relatorio_de_memoria(&self) -> String {
        use std::fmt::Write;
        let kib = |b: usize| b / 1024;
        let mut paginas_da_classe = vec![0usize; N_CLASSES];
        let (mut grandes, mut bytes_grandes, mut vazias) = (0, 0, 0);
        let (mut grandes_mortas, mut bytes_grandes_mortas) = (0, 0);
        for p in &self.paginas {
            if p.vazia {
                vazias += 1;
            } else if p.classe == GRANDE {
                grandes += 1;
                bytes_grandes += p.bytes;
                // SAFETY: o bloco da região, legível.
                if unsafe { (*p.bloco(0)).estado } == LIVRE {
                    grandes_mortas += 1;
                    bytes_grandes_mortas += p.bytes;
                }
            } else {
                paginas_da_classe[p.classe] += 1;
            }
        }
        let mut s = String::new();
        let (mut total_paginas, mut total_ocupado) = (0, 0);
        let mut linhas = Vec::new();
        for c in 1..N_CLASSES {
            let k = paginas_da_classe[c];
            if k == 0 {
                continue;
            }
            let tamanho = bytes_do_bloco(palavras_da_classe(c));
            let por_pagina = (PAGINA - CABECA_DA_PAGINA) / tamanho;
            let ocupados = self.em_uso[c];
            total_paginas += k;
            total_ocupado += ocupados * tamanho;
            linhas.push(format!(
                "  classe {c:>2} ({tamanho:>5} B): {k:>3} pág., {ocupados:>6}/{:>6} blocos ({}%)",
                k * por_pagina,
                ocupados * 100 / (k * por_pagina).max(1)
            ));
        }
        let _ = writeln!(
            s,
            "[memória] páginas de classe: {total_paginas} ({} KiB), ocupado {} KiB ({}%), classes com página: {}",
            kib(total_paginas * PAGINA),
            kib(total_ocupado),
            total_ocupado * 100 / (total_paginas * PAGINA).max(1),
            linhas.len()
        );
        for l in linhas {
            let _ = writeln!(s, "{l}");
        }
        let _ = writeln!(
            s,
            "[memória] vazias guardadas: {vazias} ({} KiB); reserva: {} livres, {} sujas ({} KiB sujas)",
            kib(vazias * PAGINA),
            self.reserva.livres.len(),
            self.reserva.sujas.len(),
            kib(self.reserva.sujas.len() * PAGINA)
        );
        let _ = writeln!(
            s,
            "[memória] regiões grandes: {grandes} no espaço ({} KiB; {grandes_mortas} mortas, {} KiB), {} em cache ({} KiB); corpos de fora: {} ({} KiB); anexos: {} ({} KiB)",
            kib(bytes_grandes),
            kib(bytes_grandes_mortas),
            self.grandes.soltas.len(),
            kib(self.grandes.bytes),
            self.com_fora.len(),
            kib(self.bytes_de_fora()),
            self.anexos.len(),
            kib(self.bytes_de_anexos)
        );
        let _ = writeln!(
            s,
            "[memória] vetores do espaço: páginas {} KiB, livres {} faixas, jovens {} KiB, faixas {} KiB, lembrados {} KiB",
            kib(self.paginas.capacity() * std::mem::size_of::<Pagina>() + self.mapa.regioes.len() * 4 * (1 << MapaDePaginas::BITS)),
            self.livres.iter().map(Vec::len).sum::<usize>(),
            kib(self.jovens.capacity() * 16),
            kib(self.faixas.capacity() * 24),
            kib(self.lembrados.capacity() * 8)
        );
        s
    }
}

/// A memória residente e o pico do processo, e os bytes confirmados do heap do
/// processo (o do alocador do Rust), em bytes (`DARTFORGE_GC_MEMORIA=1`).
pub(crate) fn memoria_do_processo() -> (usize, usize, usize) {
    #[cfg(windows)]
    {
        #[repr(C)]
        #[derive(Default)]
        struct Contadores {
            cb: u32,
            faltas: u32,
            pico: usize,
            atual: usize,
            resto: [usize; 6],
        }
        #[repr(C)]
        #[derive(Default)]
        struct ResumoDoHeap {
            cb: u32,
            alocado: usize,
            confirmado: usize,
            reservado: usize,
            maximo: usize,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetProcessHeap() -> *mut std::ffi::c_void;
            fn K32GetProcessMemoryInfo(p: *mut std::ffi::c_void, c: *mut Contadores, n: u32) -> i32;
            fn HeapSummary(h: *mut std::ffi::c_void, flags: u32, r: *mut ResumoDoHeap) -> i32;
        }
        let mut c = Contadores { cb: std::mem::size_of::<Contadores>() as u32, ..Default::default() };
        let mut r = ResumoDoHeap { cb: std::mem::size_of::<ResumoDoHeap>() as u32, ..Default::default() };
        // SAFETY: estruturas do tamanho informado.
        unsafe {
            K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb);
            HeapSummary(GetProcessHeap(), 0, &mut r);
        }
        (c.atual, c.pico, r.confirmado)
    }
    #[cfg(not(windows))]
    {
        let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        let campo = |nome: &str| {
            status
                .lines()
                .find(|l| l.starts_with(nome))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<usize>().ok())
                .map_or(0, |k| k * 1024)
        };
        (campo("VmRSS:"), campo("VmHWM:"), 0)
    }
}

impl Drop for EspacoDeObjetos {
    fn drop(&mut self) {
        for &b in &self.com_fora {
            // SAFETY: bloco com corpo de fora (do alocador do sistema).
            unsafe { Self::soltar_corpo(b) };
        }
        for (_, a) in std::mem::take(&mut self.anexos) {
            // SAFETY: o contrato de `Heap::anexar`; o espaço acabou.
            unsafe { (a.soltar)(a.ptr) };
        }
        for p in std::mem::take(&mut self.paginas) {
            // As de `PAGINA` bytes saem com a reserva (o `Drop` dela); as
            // regiões grandes, direto ao sistema.
            if p.classe == GRANDE {
                // SAFETY: região de `sistema::reservar`, sem uso.
                unsafe { sistema::liberar(p.base, p.bytes) };
            } else {
                self.reserva.livres.push(p.base);
            }
        }
    }
}
