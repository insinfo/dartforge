//! As listas do núcleo no espaço unificado (P3, docs/NATIVO-ESPACO-UNIFICADO.md §3.3).
//!
//! `_List` (cid 8) e `_ImmutableList` (9) são `REFS` (o comprimento na palavra 0,
//! os elementos `Ref` depois; cartões nas grandes) ou `BRUTO` com a forma compacta
//! em `flags` (`ELEMENTO_INT`/`_DOUBLE`/`_BOOL`); `_GrowableList` (10) é
//! `INSTANCIA` de dois campos: o comprimento (bruto) e o armazenamento (`_List`,
//! `Ref`), como o `GrowableObjectArray` da VM (`raw_object.h:3561-3572`).
//!
//! Regras desta vista:
//!
//! * **A classe diz** fixa (`_List`), imutável (`_ImmutableList`) ou expansível
//!   (`_GrowableList`); o cid nunca muda depois de publicado (§2.16). Fixar ou
//!   tornar imutável é copiar para um objeto novo ([`Heap::lista_fixa_de`],
//!   [`Heap::lista_imutavel_de`]).
//! * **A forma é do armazenamento** (a `_List` de uma `_GrowableList`): só
//!   representação, nunca semântica. Uma forma compacta que não guarda o valor
//!   gravado descompacta (§2.16): monta um armazenamento geral enraizado com as
//!   caixas e só então troca, sem alocação entre a mudança dos dados e a do
//!   `flags`.
//! * **Toda gravação de `Ref`** num armazenamento publicado passa por
//!   [`Heap::gravar_ref`]/[`Heap::gravar_refs`] (barreira e cartão); o armazenamento
//!   recém-alocado, ainda jovem e sem outra referência, é preenchido por
//!   [`Heap::palavras_mut`].
//! * **Coleta no meio:** os métodos que alocam (`nova_*`, `lista_get_ref`,
//!   `lista_set`, `lista_push`, `lista_reservar`, `lista_ajustar_forma`,
//!   `lista_copiar`, `lista_*_de`) enraízam o que seguram entre duas alocações; os
//!   handles que o chamador segura depois da chamada são dele.
//! * **A compactação aceita null.** [`Heap::lista_ajustar_forma`] é chamado depois
//!   de o `E` reificado (`int`, `double` ou `bool`, não anuláveis) ser gravado; um
//!   null dentro do comprimento só existe antes de o SDK preencher a lista
//!   (`List<int>.filled`, `_GrowableList<int>(n)`) e vira 0/0.0/false. Pelo mesmo
//!   motivo, gravar null numa lista compacta **de tipo gravado** (metadado ≠ 0: o
//!   `length =` que encolhe limpa as posições cortadas com null) grava zero em vez
//!   de descompactar; sem tipo gravado, descompacta.

use crate::heap::{Heap, Valor};
use crate::layout::{self, cid, flags, Ref};

/// A forma dos elementos de uma lista.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elemento {
    /// `Ref` (`REFS`).
    Geral,
    /// `i64` sem caixa.
    Int,
    /// Bits de `f64`.
    Double,
    /// 0/1.
    Bool,
}

impl Elemento {
    /// Os bits de `flags` (`FORMA` e `ELEMENTO`) de um armazenamento nesta forma.
    pub const fn flags(self) -> u8 {
        match self {
            Elemento::Geral => flags::REFS,
            Elemento::Int => flags::BRUTO | flags::ELEMENTO_INT,
            Elemento::Double => flags::BRUTO | flags::ELEMENTO_DOUBLE,
            Elemento::Bool => flags::BRUTO | flags::ELEMENTO_BOOL,
        }
    }

    /// A forma de um armazenamento pelos bits de `flags`.
    pub const fn de_flags(f: u8) -> Elemento {
        if f & flags::FORMA != flags::BRUTO {
            return Elemento::Geral;
        }
        match f & flags::ELEMENTO {
            flags::ELEMENTO_INT => Elemento::Int,
            flags::ELEMENTO_DOUBLE => Elemento::Double,
            flags::ELEMENTO_BOOL => Elemento::Bool,
            _ => Elemento::Geral,
        }
    }

    /// O código da ABI de `dartforge_lista_nova` (0 geral, 1 int, 2 double, 3
    /// bool: a ordem das variantes).
    pub const fn codigo(self) -> i64 {
        match self {
            Elemento::Geral => 0,
            Elemento::Int => 1,
            Elemento::Double => 2,
            Elemento::Bool => 3,
        }
    }

    /// A forma do código da ABI; `None` para outro valor.
    pub const fn do_codigo(c: i64) -> Option<Elemento> {
        match c {
            0 => Some(Elemento::Geral),
            1 => Some(Elemento::Int),
            2 => Some(Elemento::Double),
            3 => Some(Elemento::Bool),
            _ => None,
        }
    }

    /// Os bits de `v` nesta forma compacta, se ela o guarda sem caixa.
    fn bits_de(self, v: Valor) -> Option<i64> {
        match (self, v) {
            (Elemento::Int, Valor::Int(x)) => Some(x),
            (Elemento::Double, Valor::Double(d)) => Some(d.to_bits() as i64),
            (Elemento::Bool, Valor::Bool(b)) => Some(i64::from(b)),
            _ => None,
        }
    }

    /// O valor dos bits `w` de um elemento nesta forma.
    fn valor_de(self, w: i64) -> Valor {
        match self {
            Elemento::Geral => Valor::Ref(w),
            Elemento::Int => Valor::Int(w),
            Elemento::Double => Valor::Double(f64::from_bits(w as u64)),
            Elemento::Bool => Valor::Bool(w != 0),
        }
    }
}

/// Os elementos de uma lista, cortados no comprimento, na forma do armazenamento.
#[derive(Clone, Copy)]
pub enum ElementosRef<'a> {
    Geral(&'a [i64]),
    Int(&'a [i64]),
    Double(&'a [f64]),
    Bool(&'a [i64]),
}

impl ElementosRef<'_> {
    /// Quantos elementos.
    pub fn len(&self) -> usize {
        match self {
            ElementosRef::Geral(p) | ElementosRef::Int(p) | ElementosRef::Bool(p) => p.len(),
            ElementosRef::Double(p) => p.len(),
        }
    }

    /// Sem elementos?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// O elemento `i`, sem caixa.
    pub fn get(&self, i: usize) -> Valor {
        match self {
            ElementosRef::Geral(p) => Valor::Ref(p[i]),
            ElementosRef::Int(p) => Valor::Int(p[i]),
            ElementosRef::Double(p) => Valor::Double(p[i]),
            ElementosRef::Bool(p) => Valor::Bool(p[i] != 0),
        }
    }
}

impl Heap {
    /// Uma `_List` ou `_ImmutableList` (`cid`) de `len` elementos zerados (null,
    /// 0, 0.0 ou false, pela forma).
    ///
    /// O corpo tem sempre as palavras de cartões de uma lista grande
    /// (`layout::palavras_de_lista`), também na forma compacta: descompactar no
    /// lugar (§2.16) precisa delas.
    pub fn nova_lista(&mut self, cid: i32, len: usize, e: Elemento) -> Ref {
        assert!(cid::e_lista_fixa(cid), "bug do runtime: nova_lista com o cid {cid}");
        let mut f = e.flags();
        if layout::tem_cartoes(len) {
            f |= flags::CARTOES;
        }
        let h = self.alocar(cid, layout::palavras_de_lista(len), f);
        self.palavras_mut(h)[0] = len as i64;
        h
    }

    /// Uma `_GrowableList` de `len` elementos zerados e armazenamento de
    /// `capacidade` (no mínimo `len`).
    pub fn nova_expansivel(&mut self, len: usize, capacidade: usize, e: Elemento) -> Ref {
        let dados = self.nova_lista(cid::LIST, capacidade.max(len), e);
        let h = self.lista_com_raizes(&[dados], |heap| heap.alocar_instancia(cid::GROWABLE_LIST, 2));
        self.definir_campo(h, 0, len as i64, false);
        self.definir_campo(h, 1, dados, true);
        h
    }

    /// `h` é lista do núcleo (cid 8, 9 ou 10)? Sem pânico para null, `Smi` ou
    /// objeto morto.
    pub fn e_lista(&self, h: Ref) -> bool {
        layout::e_objeto(h) && self.e_objeto_vivo(h) && cid::e_lista(self.cabecalho(h).class_id)
    }

    /// O comprimento.
    pub fn lista_len(&self, h: Ref) -> usize {
        match self.lista_cid(h) {
            cid::GROWABLE_LIST => self.lista_campo_expansivel(h, 0) as usize,
            _ => self.palavras(h)[0] as usize,
        }
    }

    /// A capacidade (o comprimento do armazenamento).
    pub fn lista_capacidade(&self, h: Ref) -> usize {
        let a = self.lista_dados(h);
        self.palavras(a)[0] as usize
    }

    /// A forma dos elementos (do armazenamento, numa `_GrowableList`).
    pub fn lista_forma(&self, h: Ref) -> Elemento {
        let a = self.lista_dados(h);
        Elemento::de_flags(self.cabecalho(a).flags)
    }

    /// Os elementos, cortados no comprimento.
    #[allow(unsafe_code)]
    pub fn lista_elementos(&self, h: Ref) -> ElementosRef<'_> {
        let n = self.lista_len(h);
        let a = self.lista_dados(h);
        let forma = Elemento::de_flags(self.cabecalho(a).flags);
        let p = &self.palavras(a)[1..1 + n];
        match forma {
            Elemento::Geral => ElementosRef::Geral(p),
            Elemento::Int => ElementosRef::Int(p),
            Elemento::Bool => ElementosRef::Bool(p),
            // SAFETY: `i64` e `f64` têm o mesmo tamanho e alinhamento, e todo
            // padrão de bits é um `f64`; a fatia é a mesma memória, só lida.
            Elemento::Double => ElementosRef::Double(unsafe { std::slice::from_raw_parts(p.as_ptr().cast::<f64>(), n) }),
        }
    }

    /// O elemento `i`, sem caixa. Pânico se `i` está fora do comprimento.
    pub fn lista_get(&self, h: Ref, i: usize) -> Valor {
        let n = self.lista_len(h);
        assert!(i < n, "bug do runtime: elemento {i} de uma lista de {n}");
        let a = self.lista_dados(h);
        let forma = Elemento::de_flags(self.cabecalho(a).flags);
        forma.valor_de(self.palavras(a)[1 + i])
    }

    /// O elemento `i` em posição `Ref` (encaixota o compacto; pode coletar).
    pub fn lista_get_ref(&mut self, h: Ref, i: usize) -> Ref {
        match self.lista_get(h, i) {
            Valor::Ref(r) => r,
            v => self.como_ref(v),
        }
    }

    /// Grava o elemento `i` (descompacta se a forma não guarda `v`; com barreira).
    /// Pânico se `i` está fora do comprimento. Pode coletar (caixa, descompactação).
    pub fn lista_set(&mut self, h: Ref, i: usize, v: Valor) {
        let n = self.lista_len(h);
        assert!(i < n, "bug do runtime: gravação no elemento {i} de uma lista de {n}");
        self.lista_gravar_no_armazenamento(h, i, v);
    }

    /// Acrescenta ao fim de uma `_GrowableList` (cresce se preciso: a capacidade
    /// seguinte é `(capacidade * 2) | 3`, o `_nextCapacity` da VM,
    /// `growable_array.dart:385`). Pode coletar.
    pub fn lista_push(&mut self, h: Ref, v: Valor) {
        assert_eq!(self.lista_cid(h), cid::GROWABLE_LIST, "bug do runtime: lista_push numa lista fixa");
        let len = self.lista_len(h);
        let cap = self.lista_capacidade(h);
        if len == cap {
            let r = match v {
                Valor::Ref(r) => r,
                _ => 0,
            };
            self.lista_com_raizes(&[h, r], |heap| heap.lista_reservar(h, (cap * 2) | 3));
        }
        self.definir_campo(h, 0, (len + 1) as i64, false);
        self.lista_gravar_no_armazenamento(h, len, v);
    }

    /// Muda o comprimento de uma `_GrowableList` (sem barreira; o armazenamento
    /// precisa comportar `len`).
    pub fn lista_definir_len(&mut self, h: Ref, len: usize) {
        assert_eq!(self.lista_cid(h), cid::GROWABLE_LIST, "bug do runtime: comprimento de uma lista fixa");
        debug_assert!(len <= self.lista_capacidade(h), "comprimento {len} além da capacidade");
        self.definir_campo(h, 0, len as i64, false);
    }

    /// O armazenamento de uma `_GrowableList`; a própria lista, se é fixa ou
    /// imutável (o `@df.lista_armazenamento` do código gerado).
    pub fn lista_dados(&self, h: Ref) -> Ref {
        match self.lista_cid(h) {
            cid::GROWABLE_LIST => self.lista_campo_expansivel(h, 1),
            _ => h,
        }
    }

    /// Troca o armazenamento de uma `_GrowableList` (com barreira). `dados` é uma
    /// `_List`.
    pub fn lista_definir_dados(&mut self, h: Ref, dados: Ref) {
        assert_eq!(self.lista_cid(h), cid::GROWABLE_LIST, "bug do runtime: armazenamento de uma lista fixa");
        debug_assert!(cid::e_lista_fixa(self.classe(dados)), "o armazenamento é uma _List");
        self.definir_campo(h, 1, dados, true);
    }

    /// Garante armazenamento para `capacidade` elementos numa `_GrowableList`:
    /// um armazenamento novo, da mesma forma, com os elementos até o comprimento.
    /// Nada se a capacidade já basta. Pode coletar.
    pub fn lista_reservar(&mut self, h: Ref, capacidade: usize) {
        assert_eq!(self.lista_cid(h), cid::GROWABLE_LIST, "bug do runtime: reserva numa lista fixa");
        if capacidade <= self.lista_capacidade(h) {
            return;
        }
        let forma = self.lista_forma(h);
        let novo = self.lista_com_raizes(&[h], |heap| heap.nova_lista(cid::LIST, capacidade, forma));
        let n = self.lista_len(h);
        let velho = self.lista_dados(h);
        // O novo é jovem e ninguém mais o vê: cópia crua, sem barreira.
        let copia: Vec<i64> = self.palavras(velho)[1..1 + n].to_vec();
        self.palavras_mut(novo)[1..1 + n].copy_from_slice(&copia);
        self.lista_definir_dados(h, novo);
    }

    /// Troca a forma do armazenamento, convertendo os elementos no lugar (§2.15,
    /// §2.16). Para a forma compacta, só se todo elemento até o comprimento cabe
    /// nela (null conta como 0/0.0/false: ver o cabeçalho do módulo); para a
    /// geral, descompacta (aloca as caixas). A `_GrowableList` de armazenamento
    /// vazio ganha um armazenamento vazio próprio da forma (o vazio é
    /// compartilhado, o `_emptyList` do SDK). Pode coletar.
    pub fn lista_ajustar_forma(&mut self, h: Ref, e: Elemento) {
        let atual = self.lista_forma(h);
        if atual == e {
            return;
        }
        let a = self.lista_dados(h);
        let cap = self.palavras(a)[0] as usize;
        if self.lista_cid(h) == cid::GROWABLE_LIST && cap == 0 {
            let novo = self.lista_com_raizes(&[h], |heap| heap.nova_lista(cid::LIST, 0, e));
            self.lista_definir_dados(h, novo);
            return;
        }
        if atual != Elemento::Geral {
            self.lista_descompactar(h);
            if e == Elemento::Geral {
                return;
            }
        }
        self.lista_compactar(h, e);
    }

    /// Copia `n` elementos de `de[inicio..]` para `para[destino..]` (com barreira;
    /// `para` descompacta se um elemento não cabe na forma dela). As faixas estão
    /// dentro dos comprimentos; `de` e `para` podem ser a mesma lista (a cópia é a
    /// de `memmove`). Pode coletar.
    pub fn lista_copiar(&mut self, de: Ref, inicio: usize, para: Ref, destino: usize, n: usize) {
        if n == 0 {
            return;
        }
        let (nd, np) = (self.lista_len(de), self.lista_len(para));
        assert!(inicio + n <= nd && destino + n <= np, "bug do runtime: cópia fora das listas");
        let (fd, fp) = (self.lista_forma(de), self.lista_forma(para));
        let (ad, ap) = (self.lista_dados(de), self.lista_dados(para));
        if fd == fp {
            let fonte: Vec<i64> = self.palavras(ad)[1 + inicio..1 + inicio + n].to_vec();
            if fp == Elemento::Geral {
                self.gravar_refs(ap, 1 + destino, &fonte);
            } else {
                self.palavras_mut(ap)[1 + destino..1 + destino + n].copy_from_slice(&fonte);
            }
            return;
        }
        // Formas diferentes: elemento a elemento, com as caixas e a
        // descompactação do destino que forem precisas. Os valores saem antes
        // (a fonte pode ser o próprio destino, que a descompactação troca).
        let valores: Vec<Valor> = (0..n).map(|k| self.lista_get(de, inicio + k)).collect();
        self.lista_com_raizes(&[de, para], |heap| {
            for (k, v) in valores.into_iter().enumerate() {
                heap.lista_gravar_no_armazenamento(para, destino + k, v);
            }
        });
    }

    /// Uma `_List` nova com os elementos de `h` (na mesma forma). Pode coletar.
    pub fn lista_fixa_de(&mut self, h: Ref) -> Ref {
        self.lista_copia(h, cid::LIST)
    }

    /// Uma `_ImmutableList` nova com os elementos de `h` (na mesma forma). Pode
    /// coletar.
    pub fn lista_imutavel_de(&mut self, h: Ref) -> Ref {
        self.lista_copia(h, cid::IMMUTABLE_LIST)
    }

    /// Uma `_List`/`_ImmutableList` (`cid`) geral com os `Ref` de `valores` (as
    /// listas que o runtime monta: argumentos, resultados, literais). Os `Ref`
    /// precisam estar vivos até a alocação (quem chama os enraíza, se preciso).
    pub fn lista_de_refs(&mut self, cid: i32, valores: &[Ref]) -> Ref {
        let h = self.nova_lista(cid, valores.len(), Elemento::Geral);
        // Recém-alocada, jovem: sem barreira.
        self.palavras_mut(h)[1..1 + valores.len()].copy_from_slice(valores);
        h
    }

    /// Grava `v` em todos os elementos de `h` (o `List.filled`): uma gravação
    /// normal no primeiro (a caixa, se a forma geral pedir, ou a descompactação,
    /// se a compacta não guardar `v`), e as mesmas palavras nos demais — o mesmo
    /// objeto em toda posição, como na VM. Pode coletar.
    pub fn lista_preencher(&mut self, h: Ref, v: Valor) {
        let n = self.lista_len(h);
        if n == 0 {
            return;
        }
        self.lista_set(h, 0, v);
        let a = self.lista_dados(h);
        let w = self.palavras(a)[1];
        if self.lista_forma(h) == Elemento::Geral {
            self.gravar_refs(a, 1, &vec![w; n]);
        } else {
            self.palavras_mut(a)[1..1 + n].fill(w);
        }
    }

    // ── Auxiliares ───────────────────────────────────────────────────────────

    /// O cid de uma lista do núcleo; pânico (N4) se `h` não é uma.
    fn lista_cid(&self, h: Ref) -> i32 {
        let c = self.classe(h);
        assert!(cid::e_lista(c), "bug do runtime: lista do núcleo esperada (cid {c}, handle {h})");
        c
    }

    /// O campo `i` (0 comprimento, 1 armazenamento) de uma `_GrowableList`.
    fn lista_campo_expansivel(&self, h: Ref, i: usize) -> i64 {
        self.objeto(h).expect("_GrowableList é INSTANCIA").campo(i).0
    }

    /// `f` com `hs` enraizados num quadro do runtime (null e `Smi` não ocupam
    /// raiz útil, mas não atrapalham).
    pub(crate) fn lista_com_raizes<R>(&mut self, hs: &[Ref], f: impl FnOnce(&mut Heap) -> R) -> R {
        let quadro = self.push_frame_with_slots(hs.len());
        for (i, &x) in hs.iter().enumerate() {
            self.set_root(quadro, i, x);
        }
        let r = f(self);
        self.pop_frame(quadro);
        r
    }

    /// Grava `v` no elemento `i` do armazenamento de `h` (o comprimento já foi
    /// conferido por quem chama).
    fn lista_gravar_no_armazenamento(&mut self, h: Ref, i: usize, v: Valor) {
        let a = self.lista_dados(h);
        let forma = Elemento::de_flags(self.cabecalho(a).flags);
        if forma == Elemento::Geral {
            let r = match v {
                Valor::Ref(r) => r,
                // A caixa pode coletar: `h` fica enraizado (o armazenamento é
                // relido depois).
                outro => self.lista_com_raizes(&[h], |heap| heap.como_ref(outro)),
            };
            let a = self.lista_dados(h);
            self.gravar_ref(a, 1 + i, r);
            return;
        }
        // Forma compacta: o valor sem caixa, se couber.
        let v = match v {
            Valor::Ref(r) => self.valor(r),
            outro => outro,
        };
        if let Some(bits) = forma.bits_de(v) {
            self.palavras_mut(a)[1 + i] = bits;
            return;
        }
        if v == Valor::Ref(0) && self.metadado(h) != 0 {
            // A limpeza de uma posição cortada de uma lista de `E` escalar não
            // anulável (o `length =` que encolhe): zero, sem descompactar.
            self.palavras_mut(a)[1 + i] = 0;
            return;
        }
        let r = match v {
            Valor::Ref(r) => r,
            outro => self.lista_com_raizes(&[h], |heap| heap.como_ref(outro)),
        };
        self.lista_com_raizes(&[h, r], |heap| heap.lista_descompactar(h));
        let a = self.lista_dados(h);
        self.gravar_ref(a, 1 + i, r);
    }

    /// Passa o armazenamento de `h` à forma geral, com as caixas dos elementos
    /// até o comprimento (§2.16). Numa `_GrowableList`, um armazenamento geral
    /// novo (o velho pode ser compartilhado); numa `_List`/`_ImmutableList`, uma
    /// lista geral temporária com as caixas, copiada para o corpo de `h` depois
    /// de trocar o `flags` — sem alocação entre as duas coisas. Pode coletar.
    fn lista_descompactar(&mut self, h: Ref) {
        let forma = self.lista_forma(h);
        if forma == Elemento::Geral {
            return;
        }
        let n = self.lista_len(h);
        let expansivel = self.lista_cid(h) == cid::GROWABLE_LIST;
        let tamanho = if expansivel { self.lista_capacidade(h) } else { n };
        let geral = self.lista_com_raizes(&[h], |heap| heap.nova_lista(cid::LIST, tamanho, Elemento::Geral));
        self.lista_com_raizes(&[h, geral], |heap| {
            for i in 0..n {
                let bits = heap.palavras(heap.lista_dados(h))[1 + i];
                let r = heap.como_ref(forma.valor_de(bits));
                // `geral` pode ter envelhecido numa coleta da caixa seguinte.
                heap.gravar_ref(geral, 1 + i, r);
            }
        });
        if expansivel {
            self.lista_definir_dados(h, geral);
            return;
        }
        // Daqui ao fim, nenhuma alocação: o corpo de `h` troca de formato.
        let refs: Vec<i64> = self.palavras(geral)[1..1 + n].to_vec();
        self.definir_flags(h, flags::FORMA | flags::ELEMENTO, Elemento::Geral.flags());
        self.gravar_refs(h, 1, &refs);
    }

    /// Passa o armazenamento geral de `h` à forma compacta `e`, no lugar, se todo
    /// elemento até o comprimento cabe nela (null conta como zero). Não aloca.
    fn lista_compactar(&mut self, h: Ref, e: Elemento) {
        debug_assert!(e != Elemento::Geral && self.lista_forma(h) == Elemento::Geral);
        let n = self.lista_len(h);
        let a = self.lista_dados(h);
        let cap = self.palavras(a)[0] as usize;
        let mut bits = Vec::with_capacity(cap);
        for &r in &self.palavras(a)[1..1 + cap] {
            if r == 0 {
                bits.push(0);
                continue;
            }
            match e.bits_de(self.valor(r)) {
                Some(b) => bits.push(b),
                // Um elemento que não cabe dentro do comprimento: fica geral.
                // Além dele (o resto da capacidade), o valor não é
                // observável e vira zero.
                None if bits.len() < n => return,
                None => bits.push(0),
            }
        }
        self.definir_flags(a, flags::FORMA | flags::ELEMENTO, e.flags());
        self.palavras_mut(a)[1..1 + cap].copy_from_slice(&bits);
    }

    /// A cópia de `h` numa lista fixa (`cid`) nova, na mesma forma. Pode coletar.
    fn lista_copia(&mut self, h: Ref, cid: i32) -> Ref {
        let n = self.lista_len(h);
        let forma = self.lista_forma(h);
        let novo = self.lista_com_raizes(&[h], |heap| heap.nova_lista(cid, n, forma));
        let a = self.lista_dados(h);
        let fonte: Vec<i64> = self.palavras(a)[1..1 + n].to_vec();
        // Recém-alocada, jovem: sem barreira.
        self.palavras_mut(novo)[1..1 + n].copy_from_slice(&fonte);
        novo
    }
}

#[cfg(test)]
mod testes_listas {
    use super::*;

    #[test]
    fn listas_flags_e_codigos_ida_e_volta() {
        for e in [Elemento::Geral, Elemento::Int, Elemento::Double, Elemento::Bool] {
            assert_eq!(Elemento::de_flags(e.flags()), e);
            assert_eq!(Elemento::de_flags(e.flags() | flags::CARTOES), e);
            assert_eq!(Elemento::do_codigo(e.codigo()), Some(e));
        }
        assert_eq!(Elemento::do_codigo(4), None);
        // O `@df.lista_forma` compara `flags & 0x66` com estes valores.
        assert_eq!(Elemento::Int.flags(), 0x22);
        assert_eq!(Elemento::Double.flags(), 0x42);
        assert_eq!(Elemento::Bool.flags(), 0x62);
        assert_eq!(Elemento::Geral.flags(), 0x04);
    }

    #[test]
    fn listas_bits_de_cada_forma() {
        assert_eq!(Elemento::Int.bits_de(Valor::Int(-3)), Some(-3));
        assert_eq!(Elemento::Double.bits_de(Valor::Double(1.5)), Some(1.5f64.to_bits() as i64));
        assert_eq!(Elemento::Bool.bits_de(Valor::Bool(true)), Some(1));
        assert_eq!(Elemento::Int.bits_de(Valor::Double(1.0)), None);
        assert_eq!(Elemento::Int.bits_de(Valor::Ref(0)), None);
        assert_eq!(Elemento::Double.valor_de(2.5f64.to_bits() as i64), Valor::Double(2.5));
        assert_eq!(Elemento::Bool.valor_de(0), Valor::Bool(false));
    }
}
