// Runtime nativo: natives de lista do SDK da fonte (P5c/P5d, δ) e os erros
// que o runtime lança como objetos Dart da fonte.
//
// `_List` (cid 8), `_ImmutableList` (9) e `_GrowableList` (10) são blocos do
// espaço de objetos com cid fixo (docs/NATIVO-ESPACO-UNIFICADO.md §2.5): a
// `_GrowableList` guarda (comprimento, `_List`) como a VM, e os natives
// `GrowableList_*` são as operações da VM sobre esses dois campos. A vista
// está em `listas.rs` (`Heap::lista_*`); aqui ficam só a conferência de
// índice, os erros da VM e o tipo reificado (`tipos.rs`). A classe nunca muda
// depois de criada: fixar ou tornar imutável é copiar (§2.16).
//
// Os erros: a biblioteca `dart:_internal` da sobreposição
// (`sdk_nativo/internal/print_patch.dart`) tem funções `_dartforge*` que
// constroem o erro da fonte (`IndexError`, `RangeError`…); o registro dela
// entrega os endereços ao runtime (`dartforge_registrar_ajudante`). Sem o
// SDK da fonte, o erro é o do runtime (`excecoes.rs`).

thread_local! {
    /// Funções Dart que o runtime chama, pelo nome (`_dartforgeErroDeIndice`…).
    static AJUDANTES: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::default());
}

/// Registra uma função Dart que o runtime chama pelo nome.
///
/// # Safety
/// `nome` aponta para `len` bytes UTF-8 legíveis; `f` é a função.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_ajudante(nome: *const u8, len: i64, f: usize) {
    // SAFETY: constante do módulo com `len` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(nome, len as usize) };
    let nome = String::from_utf8_lossy(bytes).into_owned();
    AJUDANTES.with(|a| a.borrow_mut().insert(nome, f));
}

fn ajudante(nome: &str) -> Option<usize> {
    AJUDANTES.with(|a| a.borrow().get(nome).copied())
}

/// `IndexError` de `indice` em `alvo` de tamanho `tamanho` (o `RangeError
/// (index)` da VM).
fn lancar_indice(indice: i64, alvo: i64, tamanho: i64) {
    if let Some(f) = ajudante("_dartforgeErroDeIndice") {
        // SAFETY: registrado pela biblioteca `dart:_internal` com esta
        // assinatura (`int`, `Object?`, `int`) → `Object`.
        let g: extern "C" fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
        let erro = com_raizes(&[alvo], || g(indice, alvo, tamanho));
        if dartforge_exception_pending() != 0 {
            return;
        }
        com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
        return;
    }
    let n = alocar_str("index");
    let err = com_raizes(&[n], || dartforge_range_error_index(indice, tamanho, n, 0));
    dartforge_exception_throw(err, 3);
}

/// A conferência de limites de `[]` (`nome` "length") e do `_setIndexed`
/// das listas ("index") na VM: `RangeError.range(indice, 0, tamanho - 1,
/// nome)`, não `IndexError` (ex.: "RangeError (length): Invalid value: Not
/// in inclusive range 0..2: 10").
fn lancar_faixa(indice: i64, tamanho: i64, nome: &str) {
    let n = alocar_str(nome);
    let erro = com_raizes(&[n], || dartforge_range_error_range(indice, 0, tamanho - 1, n, 0));
    if dartforge_exception_pending() != 0 {
        return;
    }
    com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
}

/// O comprimento da lista do núcleo `this` (pânico N4 se não é uma).
fn lista_len(this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().lista_len(this) as i64)
}

/// O `RangeError` de `indice` fora de `[0, n)` com o nome da VM (`"index"` na
/// gravação, `"length"` na leitura); `true` se lançou.
fn fora_da_lista(indice: i64, n: i64, nome: &str) -> bool {
    if indice < 0 || indice >= n {
        lancar_faixa(indice, n, nome);
        return true;
    }
    false
}

/// Grava o tipo reificado `L<E>` (`classe`: o cid concreto) de `h` a partir da
/// tupla de argumentos de tipo do native, com `h` enraizado (a forma que o `E`
/// pede pode descompactar, e descompactar aloca).
fn tipar_lista_da_tupla(h: i64, tupla: i64, classe: i32) {
    if let Some(tipo) = tipo_lista_da_tupla(tupla, Some(i64::from(classe))) {
        com_raizes(&[h], || definir_tipo_da_lista(h, tipo + 1));
    }
}

/// Grava em `h` (`classe`) o tipo reificado de `origem`, com o `E` dela.
fn tipar_lista_copiada(h: i64, origem: i64, classe: i32) {
    if let Some(tipo) = com_raizes(&[h], || tipo_lista_copiada(origem, Some(i64::from(classe)))) {
        com_raizes(&[h], || definir_tipo_da_lista(h, tipo + 1));
    }
}

/// `_JsonListener._acrescentar(lista, v)` da sobreposição de
/// `convert_patch.dart`: `v` no fim de uma lista que o listener criou
/// (`[]`, de `E` `dynamic`/`Object?`, que aceita qualquer valor), sem o
/// despacho do `add` nem a conferência de covariância. 0, sem mudar nada, se
/// `lista` não é uma `_GrowableList` na forma geral (o Dart faz então o
/// `add`). Pode coletar (o crescimento).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_json_acrescentar(lista: i64, v: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let serve = heap.e_lista(lista)
            && heap.classe(lista) == crate::layout::cid::GROWABLE_LIST
            && heap.lista_forma(lista) == crate::listas::Elemento::Geral;
        if !serve {
            return 0;
        }
        heap.lista_push(lista, crate::heap::Valor::Ref(v));
        1
    })
}

/// `_List(length)`: `length` nulls, tamanho fixo. O parâmetro não tem tipo
/// na declaração (`external factory _List(length)`): chega como `Ref`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_List_allocate(length: i64, tupla: i64) -> i64 {
    let n = HEAP.with(|heap| heap.borrow().int_de(length)).unwrap_or(0).max(0) as usize;
    let h = HEAP.with(|heap| heap.borrow_mut().nova_lista(crate::layout::cid::LIST, n, crate::listas::Elemento::Geral));
    tipar_lista_da_tupla(h, tupla, crate::layout::cid::LIST);
    h
}

/// `_List.length` / `_ImmutableList.length`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_List_getLength(this: i64) -> i64 {
    lista_len(this)
}

/// `_List._setIndexed(i, v)`, com a conferência de índice.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_List_setIndexed(this: i64, indice: i64, valor: i64) {
    gravar_indice(this, indice, valor);
}

/// A gravação de `_setIndexed` (`_List` e `_GrowableList`): `RangeError
/// (index)` fora do comprimento; o valor entra sem caixa numa lista compacta
/// que o guarda, ou a descompacta (`Heap::lista_set`).
fn gravar_indice(this: i64, indice: i64, valor: i64) {
    if fora_da_lista(indice, lista_len(this), "index") {
        return;
    }
    HEAP.with(|heap| heap.borrow_mut().lista_set(this, indice as usize, crate::heap::Valor::Ref(valor)));
}

/// `lista[i]` de `_List`, `_ImmutableList` e `_GrowableList` (intrínseco
/// `[]` da VM), com a conferência de índice. O elemento compacto sai numa
/// caixa (`Smi`, `_Mint`, `_Double`, `bool`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_lista_get(this: i64, indice: i64) -> i64 {
    if fora_da_lista(indice, lista_len(this), "length") {
        return 0;
    }
    HEAP.with(|heap| heap.borrow_mut().lista_get_ref(this, indice as usize))
}

/// Os `quantos` elementos de `de` a partir de `inicio` numa lista fixa nova
/// (`cid`) da mesma forma.
fn fatia_de_lista(de: i64, inicio: i64, quantos: i64, cid: i32) -> i64 {
    let (inicio, quantos) = (inicio.max(0) as usize, quantos.max(0) as usize);
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let forma = heap.lista_forma(de);
        // `de` é argumento do native: o código gerado o mantém enraizado.
        let h = heap.nova_lista(cid, quantos, forma);
        heap.lista_copiar(de, inicio, h, 0, quantos);
        h
    })
}

/// `_List._sliceInternal(start, count, needsTypeArgument)`: `_List` novo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_List_slice(this: i64, inicio: i64, quantos: i64, _tipo: u8) -> i64 {
    let h = fatia_de_lista(this, inicio, quantos, crate::layout::cid::LIST);
    tipar_lista_copiada(h, this, crate::layout::cid::LIST);
    h
}

/// `_ImmutableList._from(from, offset, length)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ImmutableList_from(de: i64, inicio: i64, quantos: i64, tupla: i64) -> i64 {
    let h = fatia_de_lista(de, inicio, quantos, crate::layout::cid::IMMUTABLE_LIST);
    tipar_lista_da_tupla(h, tupla, crate::layout::cid::IMMUTABLE_LIST);
    h
}

/// `_GrowableList._withData(data)`: comprimento 0 sobre o armazenamento
/// `data` (compartilhado, como na VM: o `_setLength` seguinte expõe os
/// elementos dele).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_allocate(dados: i64, tupla: i64) -> i64 {
    let h = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // `dados` é argumento do native: o código gerado o mantém enraizado.
        let h = heap.alocar_instancia(crate::layout::cid::GROWABLE_LIST, 2);
        heap.lista_definir_dados(h, dados);
        h
    });
    tipar_lista_da_tupla(h, tupla, crate::layout::cid::GROWABLE_LIST);
    h
}

/// `_GrowableList._capacity`: o comprimento do armazenamento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_getCapacity(this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().lista_capacidade(this) as i64)
}

/// `_GrowableList.length`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_getLength(this: i64) -> i64 {
    lista_len(this)
}

/// `_GrowableList._setLength(n)`: só o campo (as posições entre o
/// comprimento velho e `n` já são null — o armazenamento nasce zerado e o
/// `length =` que encolhe as limpa). O SDK cresce antes (`_grow`); um `n`
/// além da capacidade reserva, por robustez.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_setLength(this: i64, n: i64) {
    let n = n.max(0) as usize;
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if n > heap.lista_capacidade(this) {
            heap.lista_reservar(this, n);
        }
        heap.lista_definir_len(this, n);
    });
}

/// `_GrowableList._setData(data)`: o armazenamento passa a ser `data` (com
/// barreira). O `_shrink` da VM monta `data` sem tipo (geral); a forma
/// compacta que a lista tinha volta para o novo armazenamento, sem alocar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_setData(this: i64, dados: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let forma = heap.lista_forma(this);
        heap.lista_definir_dados(this, dados);
        if forma != crate::listas::Elemento::Geral && heap.lista_capacidade(this) > 0 {
            heap.lista_ajustar_forma(this, forma);
        }
    });
}

/// `_preencherLista(lista, valor)` da sobreposição (`array.dart`):
/// os elementos de `_List.filled`/`_GrowableList.filled`, de uma vez.
/// `valor` já é do `E` da lista (o parâmetro da fábrica); entra sem caixa na
/// lista compacta, e o mesmo objeto em todas as posições da geral (o
/// `List.filled` da VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_List_preencher(lista: i64, valor: i64) {
    HEAP.with(|heap| heap.borrow_mut().lista_preencher(lista, crate::heap::Valor::Ref(valor)));
}

/// `_copiarElementos(destino, origem, n)` da sobreposição (`array.dart`):
/// os `n` primeiros elementos de `origem` nos `n` primeiros de `destino`,
/// duas listas do núcleo (`List.of`, `toList` e as cópias de
/// `_List`/`_GrowableList`). Entre listas da mesma forma, as palavras (com a
/// barreira em bloco na geral); entre formas diferentes, elemento a elemento
/// (`Heap::lista_copiar`). `destino` acabou de ser criada com o `E` dela, e
/// `origem` é um `Iterable<E>`: todo elemento cabe no destino; um que não
/// coubesse o descompactaria, como pelo `[]=`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_List_copiar(destino: i64, origem: i64, n: i64) {
    let n = usize::try_from(n).unwrap_or(0);
    if n == 0 {
        return;
    }
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if !heap.e_lista(origem) || !heap.e_lista(destino) {
            return;
        }
        let n = n.min(heap.lista_len(origem)).min(heap.lista_len(destino));
        heap.lista_copiar(origem, 0, destino, 0, n);
    });
}

/// `_GrowableList._grow(capacidade)` da sobreposição
/// (`sdk_nativo/core/growable_array.dart`): um armazenamento de
/// `capacidade`, da mesma forma, com os elementos até o comprimento (o
/// `_grow` da VM, que copiava pelo `[]=` e entregava ao `_setData`, numa
/// cópia só).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_GrowableList_reservar(this: i64, capacidade: i64) {
    let cap = usize::try_from(capacidade).unwrap_or(0);
    HEAP.with(|heap| heap.borrow_mut().lista_reservar(this, cap));
}

/// `_GrowableList._setIndexed(i, v)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_setIndexed(this: i64, indice: i64, valor: i64) {
    gravar_indice(this, indice, valor);
}

/// `makeListFixedLength(list)`: uma `_List` nova com os elementos de `list`
/// (a classe não muda depois de publicada, §2.16; os chamadores do SDK usam o
/// valor devolvido).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_makeListFixedLength(lista: i64) -> i64 {
    // `lista` é argumento do native: o código gerado a mantém enraizada.
    let h = HEAP.with(|heap| heap.borrow_mut().lista_fixa_de(lista));
    tipar_lista_copiada(h, lista, crate::layout::cid::LIST);
    h
}

/// `makeFixedListUnmodifiable(list)`: uma `_ImmutableList` nova com os
/// elementos de `list` (a VM troca o cid no lugar, `Array::MakeImmutable`;
/// aqui não, porque `df.classe` lê o cid com `!invariant.load`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_makeFixedListUnmodifiable(lista: i64) -> i64 {
    let h = HEAP.with(|heap| heap.borrow_mut().lista_imutavel_de(lista));
    tipar_lista_copiada(h, lista, crate::layout::cid::IMMUTABLE_LIST);
    h
}

#[cfg(test)]
mod testes_runtime_type {
    use super::*;

    #[test]
    fn native_retorna_tipo_canonico_do_valor() {
        dartforge_rti_classe_do_runtime(0, 101); // int
        dartforge_rti_classe_do_runtime(3, 102); // String
        let numero = dartforge_box_int(7);
        let texto = alocar_str("sete");
        let tipo_numero = dartforge_nativo_Object_runtimeType(numero);
        assert_eq!(tipo_numero, dartforge_nativo_Object_runtimeType(dartforge_box_int(9)));
        assert_ne!(tipo_numero, dartforge_nativo_Object_runtimeType(texto));
        assert_eq!(tipo_numero, dartforge_rti_objeto_tipo(dartforge_rti_do_valor(numero)));
    }
}

/// Uma `_List` com os elementos da tabela `dados` (`lower/literais.rs`): os
/// literais de coleção grandes de escalares constantes chegam como dados, e
/// não como uma instrução por elemento. Cada elemento: `n` (null), `t`/`f`
/// (bool), `i` + 8 bytes (int, little-endian), `s` + 4 bytes de tamanho +
/// os bytes WTF-8 (string, canônica como a de um literal). O apoio Dart
/// (`_dartforgePreencherLista`…) a percorre e a descarta.
///
/// # Safety
/// `dados` aponta para `len` bytes legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_lista_de_tabela(dados: *const u8, len: i64) -> i64 {
    // SAFETY: garantido por quem chama (uma constante do módulo).
    let b = unsafe { std::slice::from_raw_parts(dados, usize::try_from(len).unwrap_or(0)) };
    let mut itens: Vec<crate::heap::Valor> = Vec::new();
    let mut i = 0;
    let palavra = |b: &[u8], i: usize, n: usize| {
        let mut w = [0u8; 8];
        w[..n].copy_from_slice(&b[i..i + n]);
        u64::from_le_bytes(w)
    };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        while i < b.len() {
            let tag = b[i];
            i += 1;
            let v = match tag {
                b'n' => crate::heap::Valor::Ref(0),
                b't' => crate::heap::Valor::Bool(true),
                b'f' => crate::heap::Valor::Bool(false),
                b'i' => {
                    let x = palavra(b, i, 8) as i64;
                    i += 8;
                    crate::heap::Valor::Int(x)
                }
                b's' => {
                    let n = palavra(b, i, 4) as usize;
                    i += 4;
                    // Strings literais são canônicas e permanentes (a tabela
                    // `literais` é raiz): nenhuma coleta no meio as perde.
                    let t = Texto::de_wtf8(&b[i..i + n]);
                    let h = heap.string_literal(t.vista());
                    i += n;
                    crate::heap::Valor::Ref(h)
                }
                _ => panic!("bug do compilador: tabela de coleção com o marcador {tag}"),
            };
            itens.push(v);
        }
        let h = heap.nova_lista(crate::layout::cid::LIST, itens.len(), crate::listas::Elemento::Geral);
        // As caixas de `int` fora do `Smi` alocam: `lista_set` enraíza `h`.
        for (k, v) in itens.into_iter().enumerate() {
            heap.lista_set(h, k, v);
        }
        h
    })
}

// ─── Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.6) ─────────────────

/// A lista literal (`AllocList`, `llvm/listas_ir.rs`): `n` palavras em
/// `palavras`, da classe `cid` (`_List`, `_ImmutableList` ou `_GrowableList`
/// de capacidade `n`). `forma`:
///
/// * `0`–`3` (`Elemento::codigo`): todas as palavras já na representação da
///   forma (`Ref` na geral; `i64`, bits de `f64` ou 0/1 na compacta);
/// * outro valor: o endereço de `n` bytes com a representação de cada palavra
///   (a tag da ABI de pares: 1 `int`, 2 `bool`, 3 `Ref`, 4 bits de `double`),
///   para o literal misto; a lista sai geral, com as caixas feitas aqui (o
///   código gerado não encaixota, e uma caixa feita antes da lista ficaria sem
///   raiz na alocação seguinte).
///
/// Os `Ref` das palavras são operandos da instrução, enraizados pelo código
/// gerado durante a chamada. Não lança.
///
/// # Safety
/// `palavras` aponta `n` palavras legíveis; um `forma` fora de `0..=3` aponta
/// `n` bytes legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_lista_nova(palavras: *const i64, n: i64, cid: i64, forma: i64) -> i64 {
    use crate::listas::Elemento;
    let n = usize::try_from(n).unwrap_or(0);
    // SAFETY: garantido por quem chama (o `alloca` do código gerado).
    let p: &[i64] = if n == 0 { &[] } else { unsafe { std::slice::from_raw_parts(palavras, n) } };
    let cid = i32::try_from(cid).expect("bug do compilador: cid de lista inválido");
    let (e, tags): (Elemento, Option<&[u8]>) = match Elemento::do_codigo(forma) {
        Some(e) => (e, None),
        // SAFETY: garantido por quem chama (o vetor de tags do literal).
        None => (Elemento::Geral, Some(unsafe { std::slice::from_raw_parts(forma as usize as *const u8, n) })),
    };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = if cid == crate::layout::cid::GROWABLE_LIST {
            heap.nova_expansivel(n, n, e)
        } else {
            heap.nova_lista(cid, n, e)
        };
        let a = heap.lista_dados(h);
        match tags {
            None => {
                // Recém-alocada, jovem: as palavras entram sem barreira.
                heap.palavras_mut(a)[1..1 + n].copy_from_slice(p);
            }
            Some(tags) => {
                // Primeiro os `Ref` (sem alocação), depois as caixas (que
                // podem coletar; `lista_set` enraíza `h`).
                let refs: Vec<i64> = p.iter().zip(tags).map(|(&w, &t)| if t == 3 { w } else { 0 }).collect();
                heap.palavras_mut(a)[1..1 + n].copy_from_slice(&refs);
                for (k, (&w, &t)) in p.iter().zip(tags).enumerate() {
                    let v = match t {
                        1 => crate::heap::Valor::Int(w),
                        2 => crate::heap::Valor::Bool(w != 0),
                        4 => crate::heap::Valor::Double(f64::from_bits(w as u64)),
                        _ => continue,
                    };
                    heap.lista_set(h, k, v);
                }
            }
        }
        h
    })
}

/// O `add` de uma `_GrowableList` com o `Ref` `v` (o caminho do código gerado
/// quando o armazenamento está cheio ou não guarda `v` sem caixa;
/// `lower/intrinsecos.rs`, `lower/literais.rs`): cresce para
/// `(capacidade * 2) | 3` se preciso e grava (descompactando, se for o caso).
/// Devolve `lista`. Não lança; pode coletar (`lista` e `v` são operandos,
/// enraizados pelo código gerado).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_lista_acrescentar(lista: i64, v: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().lista_push(lista, crate::heap::Valor::Ref(v)));
    lista
}
