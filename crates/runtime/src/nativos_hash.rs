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
// passa: `int` (`Smi` ou `_Mint`: o `hashCode` do `HashIntegerOp` da VM) e `String` (o
// `StringHasher` da VM, guardado no cabeçalho da string; a igualdade por
// unidades). Como
// `int` e `String` não têm subclasses, `chave == entrada` é conhecido para
// toda entrada, com uma exceção: `int == double` compara números
// (`1 == 1.0`); esse encontro devolve "não sei". "Não sei" (outra chave,
// tabela que precisa crescer, forma inesperada) não muda nada, e o Dart
// refaz a operação pelo caminho do SDK.
//
// Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §4.5, item 4): o `_data`
// é uma `_List` geral (`REFS`: cada entrada um `Ref` — `Smi`, `_Mint`,
// string…) e o `_index` uma `_Uint32List` interna (cid 28), lida pelos bytes
// da lista tipada. Uma lista de dados compacta, imutável na gravação ou de
// outra classe devolve "não sei".

/// Uma chave que o runtime compara e espalha sozinho.
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

/// A chave e o `hashCode` dela, se o runtime os conhece: `int` (`Smi` ou
/// `_Mint`, o `hashCode` da VM, o mesmo de `_Smi.hashCode` no código gerado)
/// ou string (o hash do cabeçalho).
fn chave_de_hash(heap: &Heap, k: i64) -> Option<(ChaveDeHash, i64)> {
    use crate::layout::cid;
    if crate::layout::smi::e_smi(k) {
        let v = crate::layout::smi::valor(k);
        return Some((ChaveDeHash::Int(v), dartforge_nativo_DartForge_int_hashCode(v)));
    }
    if !crate::layout::e_objeto(k) {
        return None;
    }
    let c = heap.classe(k);
    if c == cid::MINT {
        let v = heap.int_de(k)?;
        return Some((ChaveDeHash::Int(v), dartforge_nativo_DartForge_int_hashCode(v)));
    }
    if cid::e_texto(c) {
        return Some((ChaveDeHash::Texto(k), i64::from(heap.hash_de_texto(k)?)));
    }
    None
}

/// `chave == entrada` (o `==` de `int` ou de `String`), com a entrada `Ref`.
fn comparar_chave(heap: &Heap, chave: &ChaveDeHash, entrada: i64) -> Comparacao {
    use crate::layout::cid;
    match *chave {
        ChaveDeHash::Int(v) => {
            if crate::layout::smi::e_smi(entrada) {
                return igual_se(crate::layout::smi::valor(entrada) == v);
            }
            if !crate::layout::e_objeto(entrada) {
                return Comparacao::Diferente;
            }
            match heap.classe(entrada) {
                cid::MINT => igual_se(heap.int_de(entrada) == Some(v)),
                // `1 == 1.0`: o `==` de `num`, que o Dart decide.
                cid::DOUBLE => Comparacao::NaoSei,
                _ => Comparacao::Diferente,
            }
        }
        ChaveDeHash::Texto(h) => {
            if entrada == h {
                return Comparacao::Igual;
            }
            if !crate::layout::e_objeto(entrada) || !cid::e_texto(heap.classe(entrada)) {
                return Comparacao::Diferente;
            }
            igual_se(heap.textos_iguais(h, entrada))
        }
    }
}

fn igual_se(b: bool) -> Comparacao {
    if b { Comparacao::Igual } else { Comparacao::Diferente }
}

/// Os bytes de uma `_Uint32List` interna (o `_index`), se `h` é uma.
#[allow(unsafe_code)]
fn bytes_do_indice(heap: &Heap, h: i64) -> Option<&[u8]> {
    let t = heap.tipada(h)?;
    if t.cid != crate::layout::cid::tipada(crate::tipadas::tipo::UINT32) || t.externa {
        return None;
    }
    // SAFETY: a lista vive enquanto dura o empréstimo do heap (nenhuma coleta
    // com `&Heap`); uma resolução só (`bytes_da_tipada` resolveria de novo).
    Some(unsafe { t.fatia() })
}

/// Os elementos de `_data` (até o comprimento), se é uma `_List` geral;
/// `gravar` exige a modificável (`_List`, não `_ImmutableList`).
fn elementos_dos_dados(heap: &Heap, h: i64, gravar: bool) -> Option<&[i64]> {
    if !heap.e_lista(h) {
        return None;
    }
    let c = heap.classe(h);
    let serve = if gravar { c == crate::layout::cid::LIST } else { crate::layout::cid::e_lista_fixa(c) };
    if !serve {
        return None;
    }
    match heap.lista_elementos(h) {
        crate::listas::ElementosRef::Geral(p) => Some(p),
        _ => None,
    }
}

/// O elemento `i` do `_index`.
fn par_do_indice(indice: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(indice[4 * i..4 * i + 4].try_into().expect("4 bytes"))
}

/// A sonda do `_findValueOrInsertPoint` (mapa, `passo` 2) e do `_add`
/// (conjunto, `passo` 1), com as contas de `_HashBase`. `None`: a
/// igualdade não é conhecida sem o Dart.
fn sondar(heap: &Heap, indice: &[u8], dados: &[i64], mascara: i64, chave: &ChaveDeHash, hash: i64, passo: usize) -> Option<Sonda> {
    let tamanho = indice.len() / 4;
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
        let par = par_do_indice(indice, i);
        if par == 0 {
            break;
        }
        if par == 1 {
            primeira_removida.get_or_insert(i);
        } else {
            let entrada = (padrao ^ par) as usize;
            if entrada < maximo {
                let d = entrada * passo;
                match comparar_chave(heap, chave, *dados.get(d)?) {
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

/// Grava `par` na posição `i` do `_index` (bytes da lista tipada: não é
/// referência, sem barreira).
fn gravar_no_indice(heap: &mut Heap, indice: i64, i: usize, par: u32) {
    if let Some(bytes) = heap.bytes_da_tipada_mut(indice) {
        bytes[4 * i..4 * i + 4].copy_from_slice(&par.to_le_bytes());
    }
}

/// Grava o `Ref` `v` na entrada `d` do `_data` (`_List` geral: palavra `1 + d`
/// do corpo, com barreira e cartão).
fn gravar_nos_dados(heap: &mut Heap, dados: i64, d: usize, v: i64) {
    heap.gravar_ref(dados, 1 + d, v);
}

/// O que a gravação decidiu na leitura.
enum Gravacao {
    NaoSei,
    Atualizar(usize),
    Inserir { ponto: usize, padrao: u32 },
}

/// `_Map[chave] = valor` para chave `int`/`String`: devolve o novo
/// `_usedData` (o mesmo, se a chave já estava), ou -1 quando o Dart tem de
/// fazer a operação (chave de outro tipo, `int` diante de `double`, tabela
/// cheia: o `_rehash` é do Dart).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_mapa_gravar(indice: i64, dados: i64, mascara: i64, usados: i64, chave: i64, valor: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let decisao = {
            let h: &Heap = &heap;
            match (chave_de_hash(h, chave), bytes_do_indice(h, indice), elementos_dos_dados(h, dados, true)) {
                (Some((k, hash)), Some(ind), Some(el)) => match sondar(h, ind, el, mascara, &k, hash, 2) {
                    Some(Sonda::Achou(d)) => Gravacao::Atualizar(d + 1),
                    Some(Sonda::Falta { ponto, padrao }) => {
                        if usados < 0 || usados as usize + 1 >= el.len() {
                            Gravacao::NaoSei
                        } else {
                            Gravacao::Inserir { ponto, padrao }
                        }
                    }
                    None => Gravacao::NaoSei,
                },
                _ => Gravacao::NaoSei,
            }
        };
        match decisao {
            Gravacao::NaoSei => -1,
            Gravacao::Atualizar(d) => {
                gravar_nos_dados(&mut heap, dados, d, valor);
                usados
            }
            Gravacao::Inserir { ponto, padrao } => {
                let u = usados as usize;
                gravar_no_indice(&mut heap, indice, ponto, padrao | (u >> 1) as u32);
                gravar_nos_dados(&mut heap, dados, u, chave);
                gravar_nos_dados(&mut heap, dados, u + 1, valor);
                usados + 2
            }
        }
    })
}

/// `_Map._getValueOrData(chave)` para chave `int`/`String`: o valor, ou
/// `dados` (o `_data`) quando a chave falta — o protocolo do SDK —, ou
/// `indice` (nunca um valor do mapa) quando o Dart tem de procurar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_mapa_buscar(indice: i64, dados: i64, mascara: i64, chave: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let h: &Heap = &heap;
        match (chave_de_hash(h, chave), bytes_do_indice(h, indice), elementos_dos_dados(h, dados, false)) {
            (Some((k, hash)), Some(ind), Some(el)) => match sondar(h, ind, el, mascara, &k, hash, 2) {
                Some(Sonda::Achou(d)) => el.get(d + 1).copied().unwrap_or(indice),
                Some(Sonda::Falta { .. }) => dados,
                None => indice,
            },
            _ => indice,
        }
    })
}

/// `_Set.add(chave)` para chave `int`/`String`: o novo `_usedData` (o
/// mesmo, se a chave já estava), ou -1 quando o Dart tem de fazer a
/// operação.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_conjunto_adicionar(indice: i64, dados: i64, mascara: i64, usados: i64, chave: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let decisao = {
            let h: &Heap = &heap;
            match (chave_de_hash(h, chave), bytes_do_indice(h, indice), elementos_dos_dados(h, dados, true)) {
                (Some((k, hash)), Some(ind), Some(el)) => match sondar(h, ind, el, mascara, &k, hash, 1) {
                    Some(Sonda::Achou(d)) => Gravacao::Atualizar(d),
                    Some(Sonda::Falta { ponto, padrao }) => {
                        if usados < 0 || usados as usize >= el.len() {
                            Gravacao::NaoSei
                        } else {
                            Gravacao::Inserir { ponto, padrao }
                        }
                    }
                    None => Gravacao::NaoSei,
                },
                _ => Gravacao::NaoSei,
            }
        };
        match decisao {
            Gravacao::NaoSei => -1,
            // O conjunto guarda a chave que já estava.
            Gravacao::Atualizar(_) => usados,
            Gravacao::Inserir { ponto, padrao } => {
                let u = usados as usize;
                gravar_no_indice(&mut heap, indice, ponto, padrao | u as u32);
                gravar_nos_dados(&mut heap, dados, u, chave);
                usados + 1
            }
        }
    })
}

/// `_Set._getKeyOrData(chave)` para chave `int`/`String`: a chave que está
/// no conjunto, ou `dados` quando falta, ou `indice` quando o Dart tem de
/// procurar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_conjunto_buscar(indice: i64, dados: i64, mascara: i64, chave: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let h: &Heap = &heap;
        match (chave_de_hash(h, chave), bytes_do_indice(h, indice), elementos_dos_dados(h, dados, false)) {
            (Some((k, hash)), Some(ind), Some(el)) => match sondar(h, ind, el, mascara, &k, hash, 1) {
                Some(Sonda::Achou(d)) => el.get(d).copied().unwrap_or(indice),
                Some(Sonda::Falta { .. }) => dados,
                None => indice,
            },
            _ => indice,
        }
    })
}
