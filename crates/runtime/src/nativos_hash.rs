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
// passa: `int` (`Smi` ou `_Mint`: o `hashCode` é o valor) e `String` (o
// `StringHasher` da VM, `Texto::hash_vm`; a igualdade por unidades). Como
// `int` e `String` não têm subclasses, `chave == entrada` é conhecido para
// toda entrada, com uma exceção: `int == double` compara números
// (`1 == 1.0`); esse encontro devolve "não sei". "Não sei" (outra chave,
// tabela que precisa crescer, forma inesperada) não muda nada, e o Dart
// refaz a operação pelo caminho do SDK.

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

/// A chave e o `hashCode` dela, se o runtime os conhece.
fn chave_de_hash(heap: &Heap, k: i64) -> Option<(ChaveDeHash, i64)> {
    if crate::heap::smi::e_smi(k) {
        let v = crate::heap::smi::valor(k);
        return Some((ChaveDeHash::Int(v), v));
    }
    match heap.try_get(k)? {
        Value::BoxedInt(v) => Some((ChaveDeHash::Int(*v), *v)),
        Value::String(_) => Some((ChaveDeHash::Texto(k), heap.hash_de_texto(k)?)),
        _ => None,
    }
}

/// `chave == entrada` (o `==` de `int` ou de `String`).
fn comparar_chave(heap: &Heap, chave: &ChaveDeHash, entrada: TaggedValue) -> Comparacao {
    match *chave {
        ChaveDeHash::Int(v) => match entrada.tag {
            crate::heap::ValueTag::Int => igual_se(entrada.bits == v),
            crate::heap::ValueTag::Double => Comparacao::NaoSei,
            crate::heap::ValueTag::Bool => Comparacao::Diferente,
            crate::heap::ValueTag::Ref => {
                if crate::heap::smi::e_smi(entrada.bits) {
                    return igual_se(crate::heap::smi::valor(entrada.bits) == v);
                }
                match heap.try_get(entrada.bits) {
                    Some(Value::BoxedInt(x)) => igual_se(*x == v),
                    Some(Value::BoxedDouble(_)) => Comparacao::NaoSei,
                    _ => Comparacao::Diferente,
                }
            }
        },
        ChaveDeHash::Texto(h) => {
            if entrada.tag != crate::heap::ValueTag::Ref || entrada.bits == 0 {
                return Comparacao::Diferente;
            }
            if entrada.bits == h {
                return Comparacao::Igual;
            }
            match (heap.try_get(h), heap.try_get(entrada.bits)) {
                (Some(Value::String(a)), Some(Value::String(b))) => igual_se(a == b),
                _ => Comparacao::Diferente,
            }
        }
    }
}

fn igual_se(b: bool) -> Comparacao {
    if b { Comparacao::Igual } else { Comparacao::Diferente }
}

/// Os bytes de um `Uint32List` interno (o `_index`), se `h` é um.
fn bytes_do_indice(heap: &Heap, h: i64) -> Option<&[u8]> {
    match heap.try_get(h)? {
        Value::TypedData { tipo: 6, bytes, .. } if !bytes.e_externo() => Some(bytes),
        _ => None,
    }
}

/// Os elementos de `_data`, se é uma lista do runtime na forma geral.
fn elementos_dos_dados(heap: &Heap, h: i64) -> Option<&crate::heap::Elementos> {
    match heap.try_get(h)? {
        Value::List(e) if e.forma() == crate::heap::FormaDeLista::Geral => Some(e),
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
fn sondar(heap: &Heap, indice: &[u8], dados: &crate::heap::Elementos, mascara: i64, chave: &ChaveDeHash, hash: i64, passo: usize) -> Option<Sonda> {
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
                match comparar_chave(heap, chave, dados.get(d)?) {
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

/// Grava `par` na posição `i` do `_index`.
fn gravar_no_indice(heap: &mut Heap, indice: i64, i: usize, par: u32) {
    if let Value::TypedData { bytes, .. } = heap.get_mut(indice)
        && !bytes.e_externo()
    {
        bytes[4 * i..4 * i + 4].copy_from_slice(&par.to_le_bytes());
    }
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
            match (chave_de_hash(h, chave), bytes_do_indice(h, indice), elementos_dos_dados(h, dados)) {
                (Some((k, hash)), Some(ind), Some(el)) => match sondar(h, ind, el, mascara, &k, hash, 2) {
                    Some(Sonda::Achou(d)) => Gravacao::Atualizar(d + 1),
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
            Gravacao::Atualizar(d) => {
                heap.list_set(dados, d, TaggedValue::reference(valor));
                usados
            }
            Gravacao::Inserir { ponto, padrao } => {
                let u = usados as usize;
                gravar_no_indice(&mut heap, indice, ponto, padrao | (u >> 1) as u32);
                heap.list_set(dados, u, TaggedValue::reference(chave));
                heap.list_set(dados, u + 1, TaggedValue::reference(valor));
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
        let mut heap = heap.borrow_mut();
        let achado = {
            let h: &Heap = &heap;
            match (chave_de_hash(h, chave), bytes_do_indice(h, indice), elementos_dos_dados(h, dados)) {
                (Some((k, hash)), Some(ind), Some(el)) => match sondar(h, ind, el, mascara, &k, hash, 2) {
                    Some(Sonda::Achou(d)) => el.get(d + 1).map(Ok),
                    Some(Sonda::Falta { .. }) => Some(Err(dados)),
                    None => None,
                },
                _ => None,
            }
        };
        match achado {
            Some(Ok(v)) => heap.como_ref(v),
            Some(Err(d)) => d,
            None => indice,
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
            match (chave_de_hash(h, chave), bytes_do_indice(h, indice), elementos_dos_dados(h, dados)) {
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
                heap.list_set(dados, u, TaggedValue::reference(chave));
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
        let mut heap = heap.borrow_mut();
        let achado = {
            let h: &Heap = &heap;
            match (chave_de_hash(h, chave), bytes_do_indice(h, indice), elementos_dos_dados(h, dados)) {
                (Some((k, hash)), Some(ind), Some(el)) => match sondar(h, ind, el, mascara, &k, hash, 1) {
                    Some(Sonda::Achou(d)) => el.get(d).map(Ok),
                    Some(Sonda::Falta { .. }) => Some(Err(dados)),
                    None => None,
                },
                _ => None,
            }
        };
        match achado {
            Some(Ok(v)) => heap.como_ref(v),
            Some(Err(d)) => d,
            None => indice,
        }
    })
}
