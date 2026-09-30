// Runtime nativo: o caminho rápido do `_Map` e do `_Set` padrão
// (`sdk_nativo/collection/compact_hash.dart`) para chaves `int` e `String`.
//
// A tabela é a do SDK, sem mudança de forma: o `_index` (`Uint32List`, as
// sondas lineares com o padrão de hash nos bits altos) e o `_data` (`_List`
// com chave e valor lado a lado no mapa, só a chave no conjunto), com a
// ordem de inserção e os marcadores de remoção do `compact_hash.dart`. O
// que muda é quem percorre: na VM, o compilador especializa `_hashCode` e
// `_equals` do `_OperatorEqualsAndHashCode` para `int` e `String` e embute
// tudo; aqui cada passo seria uma chamada pelo seletor, com a conferência
// de covariância do `[]=` da lista. Estas funções fazem a sonda, a
// comparação e a gravação de uma vez, com as mesmas contas do Dart
// (`_hashPattern`, `_firstProbe`, `_nextProbe`), então a tabela que deixam
// é bit a bit a que o código Dart deixaria — e o código Dart continua
// valendo para ela (remoção, iteração, `_rehash`).
//
// Só a chave cujo `==` e `hashCode` o runtime conhece sem chamar Dart
// passa: `int` (`Smi` ou `_Mint`: o `hashCode` do `HashIntegerOp` da VM) e
// `String` (o `StringHasher` da VM, guardado no cabeçalho da string; a
// igualdade por unidades). Como `int` e `String` não têm subclasses,
// `chave == entrada` é conhecido para toda entrada, com uma exceção:
// `int == double` compara números (`1 == 1.0`); esse encontro devolve "não
// sei". "Não sei" (outra chave, tabela que precisa crescer, forma
// inesperada) não muda nada, e o Dart refaz a operação pelo caminho do SDK.
//
// Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §4.5, item 4): o `_data`
// é uma `_List` geral (`REFS`: cada entrada um `Ref` — `Smi`, `_Mint`,
// string…) e o `_index` uma `_Uint32List` interna (cid 28). Uma lista de dados
// compacta, imutável na gravação ou de outra classe devolve "não sei".
//
// Sem o `Heap` (docs/NATIVO-PLANO.md §13.2, item 1): os blocos são lidos pelos
// deslocamentos do contrato de `layout`, como o código gerado os lê
// (`@df.lista_*`, `@df.texto_*`), sem `HEAP.with`, `borrow` nem as conferências
// de `bloco_vivo`; o cabeçalho de cada objeto diz a classe e a forma antes de
// qualquer leitura do corpo. A gravação no `_data` aplica a barreira de
// elemento do código gerado (`@df.barreira_elemento`). Nada aqui aloca, lança
// ou chama Dart.

use crate::layout::{Cabecalho, DESLOCAMENTO_DO_HANDLE};

/// O cabeçalho do objeto `h` (`e_objeto(h)`; vale também para os estáticos).
///
/// # Safety
/// `h` é o handle de um objeto vivo que o código gerado passou.
#[inline(always)]
unsafe fn cabecalho_cru<'a>(h: i64) -> &'a Cabecalho {
    // SAFETY: o contrato da função (o bloco começa em `h - 2`).
    unsafe { &*((h - DESLOCAMENTO_DO_HANDLE) as *const Cabecalho) }
}

/// O endereço do deslocamento `d` (de `layout::desl`, relativo ao bloco) de `h`.
#[inline(always)]
fn no_bloco(h: i64, d: usize) -> *mut u8 {
    (h - DESLOCAMENTO_DO_HANDLE + d as i64) as *mut u8
}

/// A palavra de 8 bytes no deslocamento `d` do bloco de `h`.
///
/// # Safety
/// `d` está dentro do bloco de um objeto vivo.
#[inline(always)]
unsafe fn palavra_crua(h: i64, d: usize) -> i64 {
    // SAFETY: o contrato da função; os corpos são alinhados a 8.
    unsafe { *(no_bloco(h, d) as *const i64) }
}

/// O `_index` visto pelos bytes: as posições `u32`.
#[derive(Clone, Copy)]
struct Indice {
    p: *mut u32,
    tamanho: usize,
}

impl Indice {
    /// O `_index`, se `h` é uma `_Uint32List` interna (cid 28, sem `EXTERNO`).
    #[inline(always)]
    fn de(h: i64) -> Option<Indice> {
        use crate::layout::{cid, desl, e_objeto, flags};
        if !e_objeto(h) {
            return None;
        }
        // SAFETY: objeto vivo (o código gerado passou o campo `_index`).
        let c = unsafe { cabecalho_cru(h) };
        if c.class_id != cid::tipada(crate::tipadas::tipo::UINT32) || c.flags & flags::EXTERNO != 0 {
            return None;
        }
        // SAFETY: toda lista tipada tem o comprimento em `b+16` e o ponteiro
        // dos dados em `b+24` (§2.5 da especificação).
        let (tamanho, p) = unsafe { (palavra_crua(h, desl::COMPRIMENTO) as usize, palavra_crua(h, desl::DADOS) as *mut u32) };
        Some(Indice { p, tamanho })
    }

    #[inline(always)]
    fn ler(self, i: usize) -> u32 {
        debug_assert!(i < self.tamanho);
        // SAFETY: `i < tamanho` (a sonda mascara pelo tamanho).
        unsafe { *self.p.add(i) }
    }

    #[inline(always)]
    fn gravar(self, i: usize, par: u32) {
        debug_assert!(i < self.tamanho);
        // SAFETY: como em `ler`; bytes sem referência, sem barreira.
        unsafe { *self.p.add(i) = par };
    }
}

/// Os elementos de uma `_List`/`_ImmutableList` geral (`REFS`).
#[derive(Clone, Copy)]
struct Dados {
    h: i64,
    p: *mut i64,
    len: usize,
}

impl Dados {
    /// As entradas de `h`, se é `_List` geral (`gravar`) ou `_List`/`_ImmutableList`
    /// geral (leitura).
    #[inline(always)]
    fn de(h: i64, gravar: bool) -> Option<Dados> {
        use crate::layout::{cid, desl, e_objeto, estado, flags};
        if !e_objeto(h) {
            return None;
        }
        // SAFETY: objeto vivo.
        let c = unsafe { cabecalho_cru(h) };
        let serve = if gravar { c.class_id == cid::LIST && c.estado != estado::PERMANENTE } else { cid::e_lista_fixa(c.class_id) };
        if !serve || c.flags & (flags::FORMA | flags::ELEMENTO) != flags::REFS {
            return None;
        }
        // SAFETY: `_List` `REFS`: o comprimento na palavra 0 do corpo, os
        // elementos a partir da palavra 1.
        let len = unsafe { palavra_crua(h, desl::COMPRIMENTO) } as usize;
        Some(Dados { h, p: no_bloco(h, desl::ELEMENTOS).cast(), len })
    }

    /// Os dados de uma `_GrowableList` geral (a pilha do listener do JSON):
    /// o armazenamento cortado no comprimento.
    #[inline(always)]
    fn da_expansivel(h: i64) -> Option<Dados> {
        use crate::layout::{cid, desl, e_objeto};
        if !e_objeto(h) {
            return None;
        }
        // SAFETY: objeto vivo.
        if unsafe { cabecalho_cru(h) }.class_id != cid::GROWABLE_LIST {
            return None;
        }
        // SAFETY: `_GrowableList`: comprimento no campo 0, armazenamento no 1.
        let (len, a) = unsafe { (palavra_crua(h, desl::COMPRIMENTO) as usize, palavra_crua(h, desl::EXPANSIVEL_DADOS)) };
        let mut d = Dados::de(a, false)?;
        if len > d.len {
            return None;
        }
        d.len = len;
        Some(d)
    }

    #[inline(always)]
    fn ler(self, i: usize) -> i64 {
        debug_assert!(i < self.len);
        // SAFETY: `i < len`.
        unsafe { *self.p.add(i) }
    }

    /// Grava o `Ref` `v` na entrada `i`, com a barreira de elemento do código
    /// gerado: um velho (ou lembrado) que recebe um filho jovem tem o cartão da
    /// entrada sujo, se tem cartões, e o velho é lembrado.
    #[inline(always)]
    fn gravar(self, i: usize, v: i64) {
        use crate::layout::{estado, flags};
        debug_assert!(i < self.len);
        // SAFETY: `i < len`; `_List` gravável (conferida em `de`).
        unsafe { *self.p.add(i) = v };
        // SAFETY: objeto vivo.
        let c = unsafe { cabecalho_cru(self.h) };
        let e = c.estado;
        if (e == estado::VELHO || e == estado::LEMBRADO) && filho_jovem(v) {
            if c.flags & flags::CARTOES != 0 {
                // SAFETY: bloco `REFS` com `CARTOES` de `len` elementos, `i < len`.
                unsafe { crate::espaco::sujar_cartao((self.h - DESLOCAMENTO_DO_HANDLE) as *mut Cabecalho, self.len, i) };
            }
            if e == estado::VELHO {
                dartforge_lembrar(self.h);
            }
        }
    }
}

/// O `Ref` gravado pede a barreira (`@df.filho_jovem`)? Só um objeto jovem.
#[inline(always)]
fn filho_jovem(v: i64) -> bool {
    // SAFETY: objeto vivo.
    crate::layout::e_objeto(v) && unsafe { cabecalho_cru(v) }.estado == crate::layout::estado::JOVEM
}

/// Uma chave que o runtime compara e espalha sozinho.
#[derive(Clone, Copy)]
enum ChaveDeHash {
    Int(i64),
    /// O handle da string.
    Texto(i64),
}

/// O que o `==` da chave diz de uma entrada.
enum Comparacao {
    Igual,
    Diferente,
    NaoSei,
}

/// O resultado da sonda.
enum Sonda {
    /// A entrada `d` de `_data` (a da chave).
    Achou(usize),
    /// A posição do `_index` onde a chave entraria e o padrão de hash.
    Falta { ponto: usize, padrao: u32 },
}

/// O `int` de um `Smi` ou `_Mint`.
#[inline(always)]
fn inteiro(k: i64) -> Option<i64> {
    use crate::layout::{cid, desl, e_objeto, smi};
    if smi::e_smi(k) {
        return Some(smi::valor(k));
    }
    // SAFETY: objeto vivo; a caixa `_Mint` tem o valor em `b+16`.
    if e_objeto(k) && unsafe { cabecalho_cru(k) }.class_id == cid::MINT {
        return Some(unsafe { palavra_crua(k, desl::VALOR) });
    }
    None
}

/// As unidades de uma string: de um byte (`_OneByteString`) ou de dois.
#[derive(Clone, Copy)]
enum Unidades {
    Um(*const u8, usize),
    Dois(*const u16, usize),
}

impl Unidades {
    /// As unidades de `h`, se é string.
    #[inline(always)]
    fn de(h: i64) -> Option<Unidades> {
        use crate::layout::{cid, desl, e_objeto};
        if !e_objeto(h) {
            return None;
        }
        // SAFETY: objeto vivo.
        let c = unsafe { cabecalho_cru(h) }.class_id;
        if !cid::e_texto(c) {
            return None;
        }
        // SAFETY: string: o comprimento em `b+16`, as unidades em `b+24`.
        let len = unsafe { palavra_crua(h, desl::COMPRIMENTO) } as usize;
        let p = no_bloco(h, desl::UNIDADES);
        Some(if c == cid::ONE_BYTE_STRING { Unidades::Um(p, len) } else { Unidades::Dois(p.cast(), len) })
    }

    fn len(self) -> usize {
        match self {
            Unidades::Um(_, n) | Unidades::Dois(_, n) => n,
        }
    }

    /// A unidade `i` (`i < len`).
    #[inline(always)]
    fn em(self, i: usize) -> u16 {
        // SAFETY: `i < len` (quem chama confere).
        unsafe {
            match self {
                Unidades::Um(p, _) => u16::from(*p.add(i)),
                Unidades::Dois(p, _) => *p.add(i),
            }
        }
    }

    /// As mesmas unidades?
    fn iguais(self, outra: Unidades) -> bool {
        if self.len() != outra.len() {
            return false;
        }
        // SAFETY: as duas fatias têm `len` unidades do tipo delas.
        unsafe {
            match (self, outra) {
                (Unidades::Um(a, n), Unidades::Um(b, _)) => std::slice::from_raw_parts(a, n) == std::slice::from_raw_parts(b, n),
                (Unidades::Dois(a, n), Unidades::Dois(b, _)) => std::slice::from_raw_parts(a, n) == std::slice::from_raw_parts(b, n),
                _ => (0..self.len()).all(|i| self.em(i) == outra.em(i)),
            }
        }
    }
}

/// O `hashCode` da string `h` (o de `Heap::hash_de_texto`): o do cabeçalho, ou
/// calculado e gravado nele na primeira consulta (menos no estático, que já o
/// traz e mora em memória só de leitura).
#[inline(always)]
fn hash_de_texto_cru(h: i64, u: Unidades) -> u32 {
    use crate::layout::{desl, estado};
    let p = no_bloco(h, desl::HASH_DO_TEXTO).cast::<u32>();
    // SAFETY: `mapa` do cabeçalho de uma string viva; o heap é desta thread.
    let atual = unsafe { *p };
    if atual != 0 {
        return atual;
    }
    let x = match u {
        // SAFETY: as unidades da string.
        Unidades::Um(q, n) => crate::layout::hash_de_texto(unsafe { std::slice::from_raw_parts(q, n) }.iter().map(|&b| u16::from(b))),
        Unidades::Dois(q, n) => crate::layout::hash_de_texto(unsafe { std::slice::from_raw_parts(q, n) }.iter().copied()),
    };
    // SAFETY: como acima; o estado é o byte 0 do bloco.
    if unsafe { cabecalho_cru(h) }.estado != estado::PERMANENTE {
        unsafe { *p = x };
    }
    x
}

/// A chave e o `hashCode` dela, se o runtime os conhece: `int` (`Smi` ou
/// `_Mint`, o `hashCode` da VM, o mesmo de `_Smi.hashCode` no código gerado)
/// ou string (o hash do cabeçalho).
#[inline(always)]
fn chave_de_hash(k: i64) -> Option<(ChaveDeHash, i64)> {
    if let Some(v) = inteiro(k) {
        return Some((ChaveDeHash::Int(v), dartforge_nativo_DartForge_int_hashCode(v)));
    }
    let u = Unidades::de(k)?;
    Some((ChaveDeHash::Texto(k), i64::from(hash_de_texto_cru(k, u))))
}

/// `chave == entrada` (o `==` de `int` ou de `String`), com a entrada `Ref`.
#[inline(always)]
fn comparar_chave(chave: ChaveDeHash, entrada: i64) -> Comparacao {
    use crate::layout::{cid, e_objeto, smi};
    match chave {
        ChaveDeHash::Int(v) => {
            if smi::e_smi(entrada) {
                return igual_se(smi::valor(entrada) == v);
            }
            if !e_objeto(entrada) {
                return Comparacao::Diferente;
            }
            // SAFETY: objeto vivo (uma entrada do `_data`).
            match unsafe { cabecalho_cru(entrada) }.class_id {
                cid::MINT => igual_se(inteiro(entrada) == Some(v)),
                // `1 == 1.0`: o `==` de `num`, que o Dart decide.
                cid::DOUBLE => Comparacao::NaoSei,
                _ => Comparacao::Diferente,
            }
        }
        ChaveDeHash::Texto(h) => {
            if entrada == h {
                return Comparacao::Igual;
            }
            let (Some(a), Some(b)) = (Unidades::de(h), Unidades::de(entrada)) else {
                return Comparacao::Diferente;
            };
            // Os hashes já calculados diferem: unidades diferentes.
            // SAFETY: `mapa` de duas strings vivas, só lido.
            let (ha, hb) = unsafe {
                (
                    *no_bloco(h, crate::layout::desl::HASH_DO_TEXTO).cast::<u32>(),
                    *no_bloco(entrada, crate::layout::desl::HASH_DO_TEXTO).cast::<u32>(),
                )
            };
            if ha != 0 && hb != 0 && ha != hb {
                return Comparacao::Diferente;
            }
            igual_se(a.iguais(b))
        }
    }
}

#[inline(always)]
fn igual_se(b: bool) -> Comparacao {
    if b { Comparacao::Igual } else { Comparacao::Diferente }
}

/// A sonda do `_findValueOrInsertPoint` (mapa, `passo` 2) e do `_add`
/// (conjunto, `passo` 1), com as contas de `_HashBase`. `None`: a
/// igualdade não é conhecida sem o Dart, ou a tabela tem forma inesperada.
#[inline(always)]
fn sondar(indice: Indice, dados: Dados, mascara: i64, chave: ChaveDeHash, hash: i64, passo: usize) -> Option<Sonda> {
    let tamanho = indice.tamanho;
    if tamanho == 0 || !tamanho.is_power_of_two() {
        return None;
    }
    let metade = (tamanho >> 1) as i64;
    let mascarado = hash & mascara;
    // `_hashPattern`: cabe em 32 bits pela escolha de `_hashMask`.
    let padrao = if mascarado == 0 { metade } else { mascarado.wrapping_mul(metade) };
    let padrao = u32::try_from(padrao).ok()?;
    let mascara_de_tamanho = tamanho - 1;
    let maximo = tamanho >> 1;
    // `_firstProbe`.
    let i0 = (hash as usize) & mascara_de_tamanho;
    let mut i = ((i0 << 1) + i0) & mascara_de_tamanho;
    let mut primeira_removida: Option<usize> = None;
    loop {
        let par = indice.ler(i);
        if par == 0 {
            break;
        }
        if par == 1 {
            primeira_removida.get_or_insert(i);
        } else {
            let entrada = (padrao ^ par) as usize;
            if entrada < maximo {
                let d = entrada * passo;
                if d >= dados.len {
                    return None;
                }
                match comparar_chave(chave, dados.ler(d)) {
                    Comparacao::Igual => return Some(Sonda::Achou(d)),
                    Comparacao::Diferente => {}
                    Comparacao::NaoSei => return None,
                }
            }
        }
        i = (i + 1) & mascara_de_tamanho;
    }
    Some(Sonda::Falta { ponto: primeira_removida.unwrap_or(i), padrao })
}

/// `mapa[chave] = valor` sobre a tabela já resolvida: o novo `_usedData` (o
/// mesmo, se a chave já estava), ou -1 quando o Dart tem de fazer a operação
/// (chave de outro tipo, `int` diante de `double`, tabela cheia: o `_rehash`
/// é do Dart).
#[inline(always)]
fn mapa_gravar(indice: Indice, dados: Dados, mascara: i64, usados: i64, chave: i64, valor: i64) -> i64 {
    let Some((k, hash)) = chave_de_hash(chave) else { return -1 };
    match sondar(indice, dados, mascara, k, hash, 2) {
        Some(Sonda::Achou(d)) => {
            if d + 1 >= dados.len {
                return -1;
            }
            dados.gravar(d + 1, valor);
            usados
        }
        Some(Sonda::Falta { ponto, padrao }) => {
            if usados < 0 || usados as usize + 1 >= dados.len {
                return -1;
            }
            let u = usados as usize;
            indice.gravar(ponto, padrao | (u >> 1) as u32);
            dados.gravar(u, chave);
            dados.gravar(u + 1, valor);
            usados + 2
        }
        None => -1,
    }
}

/// `_Set.add(chave)` sobre a tabela já resolvida: o novo `_usedData` (o mesmo,
/// se a chave já estava), ou -1.
#[inline(always)]
fn conjunto_adicionar(indice: Indice, dados: Dados, mascara: i64, usados: i64, chave: i64) -> i64 {
    let Some((k, hash)) = chave_de_hash(chave) else { return -1 };
    match sondar(indice, dados, mascara, k, hash, 1) {
        // O conjunto guarda a chave que já estava.
        Some(Sonda::Achou(_)) => usados,
        Some(Sonda::Falta { ponto, padrao }) => {
            if usados < 0 || usados as usize >= dados.len {
                return -1;
            }
            let u = usados as usize;
            indice.gravar(ponto, padrao | u as u32);
            dados.gravar(u, chave);
            usados + 1
        }
        None => -1,
    }
}

/// `_Map[chave] = valor` para chave `int`/`String`: devolve o novo
/// `_usedData` (o mesmo, se a chave já estava), ou -1 quando o Dart tem de
/// fazer a operação (chave de outro tipo, `int` diante de `double`, tabela
/// cheia: o `_rehash` é do Dart).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_mapa_gravar(indice: i64, dados: i64, mascara: i64, usados: i64, chave: i64, valor: i64) -> i64 {
    match (Indice::de(indice), Dados::de(dados, true)) {
        (Some(i), Some(d)) => mapa_gravar(i, d, mascara, usados, chave, valor),
        _ => -1,
    }
}

/// `_Map._getValueOrData(chave)` para chave `int`/`String`: o valor, ou
/// `dados` (o `_data`) quando a chave falta — o protocolo do SDK —, ou
/// `indice` (nunca um valor do mapa) quando o Dart tem de procurar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_mapa_buscar(indice: i64, dados: i64, mascara: i64, chave: i64) -> i64 {
    let (Some(ind), Some(el), Some((k, hash))) = (Indice::de(indice), Dados::de(dados, false), chave_de_hash(chave)) else {
        return indice;
    };
    match sondar(ind, el, mascara, k, hash, 2) {
        Some(Sonda::Achou(d)) if d + 1 < el.len => el.ler(d + 1),
        Some(Sonda::Falta { .. }) => dados,
        _ => indice,
    }
}

/// `_Set.add(chave)` para chave `int`/`String`: o novo `_usedData` (o
/// mesmo, se a chave já estava), ou -1 quando o Dart tem de fazer a
/// operação.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_conjunto_adicionar(indice: i64, dados: i64, mascara: i64, usados: i64, chave: i64) -> i64 {
    match (Indice::de(indice), Dados::de(dados, true)) {
        (Some(i), Some(d)) => conjunto_adicionar(i, d, mascara, usados, chave),
        _ => -1,
    }
}

/// `_Set._getKeyOrData(chave)` para chave `int`/`String`: a chave que está
/// no conjunto, ou `dados` quando falta, ou `indice` quando o Dart tem de
/// procurar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_conjunto_buscar(indice: i64, dados: i64, mascara: i64, chave: i64) -> i64 {
    let (Some(ind), Some(el), Some((k, hash))) = (Indice::de(indice), Dados::de(dados, false), chave_de_hash(chave)) else {
        return indice;
    };
    match sondar(ind, el, mascara, k, hash, 1) {
        Some(Sonda::Achou(d)) => el.ler(d),
        Some(Sonda::Falta { .. }) => dados,
        None => indice,
    }
}

/// O resultado de um lote: a posição seguinte em `pares` (onde o lote parou) nos
/// 32 bits altos e o novo `_usedData` nos baixos.
#[inline(always)]
fn lote(posicao: usize, usados: i64) -> i64 {
    ((posicao as i64) << 32) | (usados & 0xFFFF_FFFF)
}

/// As entradas de `pares`: uma `_List`/`_ImmutableList` geral (o `_data` velho
/// do `_rehash`) ou uma `_GrowableList` geral (a pilha do listener do JSON).
#[inline(always)]
fn entradas(pares: i64) -> Option<Dados> {
    Dados::de(pares, false).or_else(|| Dados::da_expansivel(pares))
}

/// O lote do `_Map` (docs/NATIVO-PLANO.md §13.2, itens 2 e 3): `mapa[k] = v`
/// para cada par `(pares[i], pares[i + 1])`, `i` de `de` até `ate` de 2 em 2,
/// na ordem, como o `_set` faria. Com `pular_removidas`, o par cuja chave é a
/// própria lista (`_HashBase._isDeleted` do `_data` velho) é pulado. Para no
/// primeiro par que o runtime não sabe gravar (chave de outro tipo, `int`
/// diante de `double`, tabela cheia) e devolve [`lote`]: o Dart grava esse par
/// e chama de novo a partir do seguinte.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_mapa_preencher(indice: i64, dados: i64, mascara: i64, usados: i64, pares: i64, de: i64, ate: i64, pular_removidas: u8) -> i64 {
    let (Some(ind), Some(d), Some(p)) = (Indice::de(indice), Dados::de(dados, true), entradas(pares)) else {
        return lote(de.max(0) as usize, usados);
    };
    let (mut i, ate) = (de.max(0) as usize, (ate.max(0) as usize).min(p.len));
    let mut u = usados;
    while i + 1 < ate {
        let k = p.ler(i);
        if !(pular_removidas != 0 && k == pares) {
            let r = mapa_gravar(ind, d, mascara, u, k, p.ler(i + 1));
            if r < 0 {
                break;
            }
            u = r;
        }
        i += 2;
    }
    lote(i, u)
}

/// O lote do `_Set` (o `_init` do `_rehash`): `add(velhos[i])` para `i` de `de`
/// até `ate`, pulando as removidas (a chave que é a própria lista). Para no
/// primeiro elemento que o runtime não sabe inserir e devolve [`lote`].
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_conjunto_preencher(indice: i64, dados: i64, mascara: i64, usados: i64, velhos: i64, de: i64, ate: i64) -> i64 {
    let (Some(ind), Some(d), Some(p)) = (Indice::de(indice), Dados::de(dados, true), entradas(velhos)) else {
        return lote(de.max(0) as usize, usados);
    };
    let (mut i, ate) = (de.max(0) as usize, (ate.max(0) as usize).min(p.len));
    let mut u = usados;
    while i < ate {
        let k = p.ler(i);
        if k != velhos {
            let r = conjunto_adicionar(ind, d, mascara, u, k);
            if r < 0 {
                break;
            }
            u = r;
        }
        i += 1;
    }
    lote(i, u)
}

/// `_dfNovosDados(n)` (`compact_hash.dart`): o `_data` novo de uma tabela, uma
/// `_List` geral de `n` nulls sem tipo reificado (o `new List.filled(n, null)`
/// do SDK, sem avaliar o `List<Object?>`: a lista nunca sai da tabela). Pode
/// coletar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_novos_dados(n: i64) -> i64 {
    let n = usize::try_from(n).unwrap_or(0);
    HEAP.with(|heap| heap.borrow_mut().nova_lista(crate::layout::cid::LIST, n, crate::listas::Elemento::Geral))
}
