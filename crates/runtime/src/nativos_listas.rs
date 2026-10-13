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
// (`native_sdk/internal/print_patch.dart`) tem funções `_dartforge*` que
// constroem o erro da fonte (`IndexError`, `RangeError`…); o registro dela
// entrega os endereços ao runtime (`dartforge_registrar_ajudante`). Sem o
// SDK da fonte, o erro é o do runtime (`excecoes.rs`).

thread_local! {
    /// Funções Dart que o runtime chama, pelo nome (`_dartforgeErroDeIndice`…).
    static HELPERS: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::default());
}

/// Registra uma função Dart que o runtime chama pelo nome.
///
/// # Safety
/// `name` aponta para `len` bytes UTF-8 legíveis; `f` é a função.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_ajudante(name: *const u8, len: i64, f: usize) {
    // SAFETY: constante do módulo com `len` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(name, len as usize) };
    let name = String::from_utf8_lossy(bytes).into_owned();
    HELPERS.with(|a| a.borrow_mut().insert(name, f));
}

fn ajudante(name: &str) -> Option<usize> {
    HELPERS.with(|a| a.borrow().get(name).copied())
}

/// `IndexError` de `ix` em `target` de tamanho `size` (o `RangeError
/// (index)` da VM).
fn throw_index(ix: i64, target: i64, size: i64) {
    if let Some(f) = ajudante("_dartforgeErroDeIndice") {
        // SAFETY: registrado pela biblioteca `dart:_internal` com esta
        // assinatura (`int`, `Object?`, `int`) → `Object`.
        let g = { let dart_target: usize = f; move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(dart_target, a0, a1, a2) } };
        let error = with_roots(&[target], || g(ix, target, size));
        if dartforge_exception_pending() != 0 {
            return;
        }
        with_roots(&[error], || dartforge_exception_throw(error, 3));
        return;
    }
    let n = allocate_str("index");
    let err = with_roots(&[n], || dartforge_range_error_index(ix, size, n, 0));
    dartforge_exception_throw(err, 3);
}

/// A conferência de limites de `[]` (`name` "length") e do `_setIndexed`
/// das listas ("index") na VM: `RangeError.range(indice, 0, tamanho - 1,
/// nome)`, não `IndexError` (ex.: "RangeError (length): Invalid value: Not
/// in inclusive range 0..2: 10").
fn throw_range(ix: i64, size: i64, name: &str) {
    let n = allocate_str(name);
    let error = with_roots(&[n], || dartforge_range_error_range(ix, 0, size - 1, n, 0));
    if dartforge_exception_pending() != 0 {
        return;
    }
    with_roots(&[error], || dartforge_exception_throw(error, 3));
}

/// O comprimento da lista do núcleo `this` (pânico N4 se não é uma).
fn list_len(this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().list_len(this) as i64)
}

/// O `RangeError` de `ix` fora de `[0, n)` com o nome da VM (`"index"` na
/// gravação, `"length"` na leitura); `true` se lançou.
fn outside_list(ix: i64, n: i64, name: &str) -> bool {
    if ix < 0 || ix >= n {
        throw_range(ix, n, name);
        return true;
    }
    false
}

/// Grava o tipo reificado `L<E>` (`class`: o cid concreto) de `h` a partir da
/// tupla de argumentos de tipo do native, com `h` enraizado (a forma que o `E`
/// pede pode descompactar, e descompactar aloca).
fn type_tuple_list(h: i64, tuple: i64, class: i32) {
    if let Some(tipo) = tuple_list_type(tuple, Some(i64::from(class))) {
        with_roots(&[h], || set_list_type(h, tipo + 1));
    }
}

/// Grava em `h` (`class`) o tipo reificado de `origin`, com o `E` dela.
fn type_copied_list(h: i64, origin: i64, class: i32) {
    if let Some(tipo) = with_roots(&[h], || copied_list_type(origin, Some(i64::from(class)))) {
        with_roots(&[h], || set_list_type(h, tipo + 1));
    }
}

/// A pilha de valores do `_JsonListener` da sobreposição de `convert_patch.dart`
/// (docs/NATIVO-PLANO.md §13.2, item 4): uma `_GrowableList` de `Object?` que só
/// o listener vê, na forma geral. Os natives abaixo a leem e gravam pelos
/// deslocamentos do contrato (`Data` de `nativos_hash.rs`), sem o `Heap`, e
/// devolvem -1 sem mudar nada quando ela não está nessa forma — o Dart faz
/// então o `add`, o laço e o `length =` de sempre.
///
/// `_JsonListener._dfEmpilhar(stack, v)`: `v` no fim da pilha, sem o despacho
/// do `add` nem a conferência do `E` (`Object?`). Devolve o novo comprimento.
/// Pode coletar (o crescimento do armazenamento).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_json_empilhar(stack: i64, v: i64) -> i64 {
    if let Some(n) = push_raw(stack, v) {
        return n;
    }
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let serve = heap.e_lista(stack)
            && heap.class(stack) == crate::layout::cid::GROWABLE_LIST
            && heap.list_shape(stack) == crate::listas::Element::General;
        if !serve {
            return -1;
        }
        heap.list_push(stack, crate::heap::Value::Ref(v));
        heap.list_len(stack) as i64
    })
}

/// O `empilhar` com capacidade sobrando: grava no armazenamento (com a barreira
/// de elemento) e o comprimento no campo 0, sem o `Heap`. `None` se precisa
/// crescer ou a forma não é a simples.
fn push_raw(stack: i64, v: i64) -> Option<i64> {
    use crate::layout::{cid, desl, is_object};
    if !is_object(stack) {
        return None;
    }
    // SAFETY: objeto vivo (o campo `_pilha` do listener).
    if unsafe { raw_header(stack) }.class_id != cid::GROWABLE_LIST {
        return None;
    }
    // SAFETY: `_GrowableList`: comprimento no campo 0, armazenamento no 1.
    let (len, a) = unsafe { (raw_word(stack, desl::COMPRIMENTO), raw_word(stack, desl::EXPANSIVEL_DADOS)) };
    let d = Data::de(a, true)?;
    let n = usize::try_from(len).ok()?;
    if n >= d.len {
        return None;
    }
    d.write_value(n, v);
    // SAFETY: o campo 0 (comprimento bruto, sem barreira: não é referência).
    unsafe { *in_block(stack, desl::COMPRIMENTO).cast::<i64>() = len + 1 };
    Some(len + 1)
}

/// A marca em `start - 1` e a pilha (cortada no comprimento), se `start` é
/// uma posição de contêiner aberto (`1 ≤ start ≤ length`) e a marca um
/// `Smi`.
fn stack_mark(stack: i64, start: i64) -> Option<(Data, usize, i64)> {
    let p = Data::from_growable(stack)?;
    let start = usize::try_from(start).ok().filter(|&i| i >= 1 && i <= p.len)?;
    let m = p.do_read(start - 1);
    crate::layout::smi::is_smi(m).then(|| (p, start, crate::layout::smi::value(m)))
}

/// Corta a pilha em `until` (anulando as posições tiradas, para não reter os
/// valores; null não pede barreira).
fn cut_stack(stack: i64, p: Data, until: usize) {
    for i in until..p.len {
        // SAFETY: `i < len` do armazenamento geral gravável da pilha.
        let old = unsafe { std::mem::replace(&mut *p.p.add(i), 0) };
        if arc_enabled() {
            arc_raw_write(p.h, old, 0);
        }
    }
    // SAFETY: o campo 0 da `_GrowableList` (comprimento bruto).
    unsafe { *in_block(stack, crate::layout::desl::COMPRIMENTO).cast::<i64>() = until as i64 };
}

/// `_JsonListener._dfTruncar(stack, start)`: tira os valores `stack[start..]`
/// e a marca em `start - 1`; devolve a marca, ou -1. Não aloca.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_json_truncar(stack: i64, start: i64) -> i64 {
    let Some((p, start, mark)) = stack_mark(stack, start) else { return -1 };
    // A pilha é gravável (a do listener): a mesma conferência de `push_raw`.
    // SAFETY: o armazenamento da `_GrowableList` (campo 1).
    if Data::de(unsafe { raw_word(stack, crate::layout::desl::EXPANSIVEL_DADOS) }, true).is_none() {
        return -1;
    }
    cut_stack(stack, p, start - 1);
    mark
}

/// `_JsonListener._dfMarca(stack, start)`: a marca em `start - 1` (o contêiner
/// aberto), sem mudar nada; -1 fora da forma simples. Não aloca.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_json_marca(stack: i64, start: i64) -> i64 {
    stack_mark(stack, start).map_or(-1, |(_, _, m)| m)
}

/// `_JsonListener._dfFecharLista(stack, start, model)`: a lista do JSON com os
/// valores `stack[start..]`, na ordem — uma `_GrowableList` nova do tamanho
/// exato, com o tipo reificado de `model` (o `[]` de `List<dynamic>` do
/// listener, o mesmo tipo do `[]` que o listener da VM cria por lista) —, e a
/// pilha sem esses valores nem a marca em `start - 1`. null, sem mudar nada,
/// fora da forma simples. Pode coletar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_json_fechar_lista(stack: i64, start: i64, model: i64) -> i64 {
    use crate::layout::{cid, desl, is_object};
    let Some((p, i0, _)) = stack_mark(stack, start) else { return 0 };
    // SAFETY: o armazenamento da `_GrowableList` (campo 1).
    if Data::de(unsafe { raw_word(stack, desl::EXPANSIVEL_DADOS) }, true).is_none() {
        return 0;
    }
    // SAFETY: objeto vivo.
    if !is_object(model) || unsafe { raw_header(model) }.class_id != cid::GROWABLE_LIST {
        return 0;
    }
    let n = p.len - i0;
    let list = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // A pilha e o modelo enraizados: os valores são alcançados pela pilha.
        let list = heap.list_with_roots(&[stack, model], |heap| heap.new_growable(n, n, crate::listas::Element::General));
        let tipo = heap.metadata(model);
        heap.set_metadata(list, tipo);
        if n > 0 {
            // O coletor não move: `p` continua valendo. O armazenamento é novo e
            // ninguém mais o vê: cópia crua, sem barreira (como `list_reserve`).
            let a = heap.list_data(list);
            let copy: Vec<i64> = (i0..p.len).map(|i| p.do_read(i)).collect();
            heap.words_mut(a)[1..1 + n].copy_from_slice(&copy);
        }
        list
    });
    cut_stack(stack, p, i0 - 1);
    list
}

/// `_List(length)`: `length` nulls, tamanho fixo. O parâmetro não tem tipo
/// na declaração (`external factory _List(length)`): chega como `Ref`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_List_allocate(length: i64, tuple: i64) -> i64 {
    let n = HEAP.with(|heap| heap.borrow().int_of(length)).unwrap_or(0).max(0) as usize;
    let h = HEAP.with(|heap| heap.borrow_mut().new_list(crate::layout::cid::LIST, n, crate::listas::Element::General));
    type_tuple_list(h, tuple, crate::layout::cid::LIST);
    h
}

/// `_List.length` / `_ImmutableList.length`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_List_getLength(this: i64) -> i64 {
    list_len(this)
}

/// `_List._setIndexed(i, v)`, com a conferência de índice.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_List_setIndexed(this: i64, ix: i64, value: i64) {
    write_index(this, ix, value);
}

/// A gravação de `_setIndexed` (`_List` e `_GrowableList`): `RangeError
/// (index)` fora do comprimento; o valor entra sem caixa numa lista compacta
/// que o guarda, ou a descompacta (`Heap::list_set`).
fn write_index(this: i64, ix: i64, value: i64) {
    if outside_list(ix, list_len(this), "index") {
        return;
    }
    HEAP.with(|heap| heap.borrow_mut().list_set(this, ix as usize, crate::heap::Value::Ref(value)));
}

/// `list[i]` de `_List`, `_ImmutableList` e `_GrowableList` (intrínseco
/// `[]` da VM), com a conferência de índice. O elemento compacto sai numa
/// caixa (`Smi`, `_Mint`, `_Double`, `bool`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_lista_get(this: i64, ix: i64) -> i64 {
    if outside_list(ix, list_len(this), "length") {
        return 0;
    }
    HEAP.with(|heap| heap.borrow_mut().list_get_ref(this, ix as usize))
}

/// Os `how_many` elementos de `de` a partir de `start` numa lista fixa nova
/// (`cid`) da mesma forma.
fn list_slice(de: i64, start: i64, how_many: i64, cid: i32) -> i64 {
    let (start, how_many) = (start.max(0) as usize, how_many.max(0) as usize);
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let shape = heap.list_shape(de);
        // `de` é argumento do native: o código gerado o mantém enraizado.
        let h = heap.new_list(cid, how_many, shape);
        heap.list_copy(de, start, h, 0, how_many);
        h
    })
}

/// `_List._sliceInternal(start, count, needsTypeArgument)`: `_List` novo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_List_slice(this: i64, start: i64, how_many: i64, _type: u8) -> i64 {
    let h = list_slice(this, start, how_many, crate::layout::cid::LIST);
    type_copied_list(h, this, crate::layout::cid::LIST);
    h
}

/// `_ImmutableList._from(from, offset, length)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ImmutableList_from(de: i64, start: i64, how_many: i64, tuple: i64) -> i64 {
    let h = list_slice(de, start, how_many, crate::layout::cid::IMMUTABLE_LIST);
    type_tuple_list(h, tuple, crate::layout::cid::IMMUTABLE_LIST);
    h
}

/// `_GrowableList._withData(data)`: comprimento 0 sobre o armazenamento
/// `data` (compartilhado, como na VM: o `_setLength` seguinte expõe os
/// elementos dele).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_allocate(data: i64, tuple: i64) -> i64 {
    let h = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // `data` é argumento do native: o código gerado o mantém enraizado.
        let h = heap.allocate_instance(crate::layout::cid::GROWABLE_LIST, 2);
        heap.list_set_data(h, data);
        h
    });
    type_tuple_list(h, tuple, crate::layout::cid::GROWABLE_LIST);
    h
}

/// `_GrowableList._capacity`: o comprimento do armazenamento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_getCapacity(this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().list_capacity(this) as i64)
}

/// `_GrowableList.length`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_getLength(this: i64) -> i64 {
    list_len(this)
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
        if n > heap.list_capacity(this) {
            heap.list_reserve(this, n);
        }
        heap.list_set_len(this, n);
    });
}

/// `_GrowableList._setData(data)`: o armazenamento passa a ser `data` (com
/// barreira). O `_shrink` da VM monta `data` sem tipo (geral); a forma
/// compacta que a lista tinha volta para o novo armazenamento, sem alocar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_setData(this: i64, data: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let shape = heap.list_shape(this);
        heap.list_set_data(this, data);
        if shape != crate::listas::Element::General && heap.list_capacity(this) > 0 {
            heap.list_adjust_shape(this, shape);
        }
    });
}

/// `_preencherLista(list, value)` da sobreposição (`array.dart`):
/// os elementos de `_List.filled`/`_GrowableList.filled`, de uma vez.
/// `value` já é do `E` da lista (o parâmetro da fábrica); entra sem caixa na
/// lista compacta, e o mesmo objeto em todas as posições da geral (o
/// `List.filled` da VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_List_preencher(list: i64, value: i64) {
    HEAP.with(|heap| heap.borrow_mut().list_fill(list, crate::heap::Value::Ref(value)));
}

/// `_copiarElementos(dest, origin, n)` da sobreposição (`array.dart`):
/// os `n` primeiros elementos de `origin` nos `n` primeiros de `dest`,
/// duas listas do núcleo (`List.of`, `toList` e as cópias de
/// `_List`/`_GrowableList`). Entre listas da mesma forma, as palavras (com a
/// barreira em bloco na geral); entre formas diferentes, elemento a elemento
/// (`Heap::list_copy`). `dest` acabou de ser criada com o `E` dela, e
/// `origin` é um `Iterable<E>`: todo elemento cabe no destino; um que não
/// coubesse o descompactaria, como pelo `[]=`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_List_copiar(dest: i64, origin: i64, n: i64) {
    let n = usize::try_from(n).unwrap_or(0);
    if n == 0 {
        return;
    }
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if !heap.e_lista(origin) || !heap.e_lista(dest) {
            return;
        }
        let n = n.min(heap.list_len(origin)).min(heap.list_len(dest));
        heap.list_copy(origin, 0, dest, 0, n);
    });
}

/// `_GrowableList._grow(capacidade)` da sobreposição
/// (`native_sdk/core/growable_array.dart`): um armazenamento de
/// `capacidade`, da mesma forma, com os elementos até o comprimento (o
/// `_grow` da VM, que copiava pelo `[]=` e entregava ao `_setData`, numa
/// cópia só).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_GrowableList_reservar(this: i64, capacidade: i64) {
    let cap = usize::try_from(capacidade).unwrap_or(0);
    HEAP.with(|heap| heap.borrow_mut().list_reserve(this, cap));
}

/// `_GrowableList._setIndexed(i, v)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_setIndexed(this: i64, ix: i64, value: i64) {
    write_index(this, ix, value);
}

/// `makeListFixedLength(list)`: uma `_List` nova com os elementos de `list`
/// (a classe não muda depois de publicada, §2.16; os chamadores do SDK usam o
/// valor devolvido).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_makeListFixedLength(list: i64) -> i64 {
    // `list` é argumento do native: o código gerado a mantém enraizada.
    let h = HEAP.with(|heap| heap.borrow_mut().fixed_list_of(list));
    type_copied_list(h, list, crate::layout::cid::LIST);
    h
}

/// `makeFixedListUnmodifiable(list)`: uma `_ImmutableList` nova com os
/// elementos de `list` (a VM troca o cid no lugar, `Array::MakeImmutable`;
/// aqui não, porque `df.class` lê o cid com `!invariant.load`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_makeFixedListUnmodifiable(list: i64) -> i64 {
    let h = HEAP.with(|heap| heap.borrow_mut().unmodifiable_list_of(list));
    type_copied_list(h, list, crate::layout::cid::IMMUTABLE_LIST);
    h
}

#[cfg(test)]
mod tests_runtime_type {
    use super::*;

    #[test]
    fn native_returns_canonical_value_type() {
        dartforge_rti_classe_do_runtime(0, 101); // int
        dartforge_rti_classe_do_runtime(3, 102); // String
        let number = dartforge_box_int(7);
        let text = allocate_str("sete");
        let number_type = dartforge_nativo_Object_runtimeType(number);
        assert_eq!(number_type, dartforge_nativo_Object_runtimeType(dartforge_box_int(9)));
        assert_ne!(number_type, dartforge_nativo_Object_runtimeType(text));
        assert_eq!(number_type, dartforge_rti_objeto_tipo(dartforge_rti_do_valor(number)));
    }
}

/// Uma `_List` com os elementos da tabela `data` (`lower/literais.rs`): os
/// literais de coleção grandes de escalares constantes chegam como dados, e
/// não como uma instrução por elemento. Cada elemento: `n` (null), `t`/`f`
/// (bool), `i` + 8 bytes (int, little-endian), `s` + 4 bytes de tamanho +
/// os bytes WTF-8 (string, canônica como a de um literal). O apoio Dart
/// (`_dartforgePreencherLista`…) a percorre e a descarta.
///
/// # Safety
/// `data` aponta para `len` bytes legíveis; `getters`, para um vetor com
/// o endereço de cada getter que a tabela cita (`g` + 4 bytes de índice:
/// uma constante do programa, `dfc.<hash>.get`, sem argumentos).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_lista_de_tabela_g(data: *const u8, len: i64, getters: *const extern "C" fn() -> i64) -> i64 {
    // SAFETY: garantido por quem chama (constantes do módulo).
    let b = unsafe { std::slice::from_raw_parts(data, usize::try_from(len).unwrap_or(0)) };
    // Os elementos, sem o heap emprestado: o getter roda código Dart.
    enum Item<'a> {
        Value(crate::heap::Value),
        Texto(&'a [u8]),
        Getter(usize),
    }
    let word = |b: &[u8], i: usize, n: usize| {
        let mut w = [0u8; 8];
        w[..n].copy_from_slice(&b[i..i + n]);
        u64::from_le_bytes(w)
    };
    let mut items: Vec<Item<'_>> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let tag = b[i];
        i += 1;
        items.push(match tag {
            b'n' => Item::Value(crate::heap::Value::Ref(0)),
            b't' => Item::Value(crate::heap::Value::Bool(true)),
            b'f' => Item::Value(crate::heap::Value::Bool(false)),
            b'i' => {
                i += 8;
                Item::Value(crate::heap::Value::Int(word(b, i - 8, 8) as i64))
            }
            b'd' => {
                i += 8;
                Item::Value(crate::heap::Value::Double(f64::from_bits(word(b, i - 8, 8))))
            }
            b's' => {
                let n = word(b, i, 4) as usize;
                i += 4 + n;
                Item::Texto(&b[i - n..i])
            }
            b'g' => {
                i += 4;
                Item::Getter(word(b, i - 4, 4) as usize)
            }
            _ => panic!("bug do compilador: tabela de coleção com o marcador {tag}"),
        });
    }
    let h = HEAP.with(|heap| heap.borrow_mut().new_list(crate::layout::cid::LIST, items.len(), crate::listas::Element::General));
    // A lista fica enraizada enquanto os textos e os getters alocam.
    with_roots(&[h], || {
        for (k, item) in items.into_iter().enumerate() {
            let v = match item {
                Item::Value(v) => v,
                // Strings literais são canônicas e permanentes (a tabela
                // `literals` é raiz).
                Item::Texto(t) => {
                    let t = Texto::de_wtf8(t);
                    crate::heap::Value::Ref(HEAP.with(|heap| heap.borrow_mut().string_literal(t.vista())))
                }
                Item::Getter(g) => {
                    // SAFETY: o índice é de uma entrada do vetor (o compilador
                    // escreve os dois juntos); o getter é uma função do módulo.
                    let f = unsafe { *getters.add(g) };
                    let v = dart_r0(f as usize);
                    if dartforge_exception_pending() != 0 {
                        return;
                    }
                    crate::heap::Value::Ref(v)
                }
            };
            // As caixas de `int` fora do `Smi` alocam: `list_set` enraíza `h`.
            HEAP.with(|heap| heap.borrow_mut().list_set(h, k, v));
        }
    });
    h
}

/// # Safety
/// `data` aponta para `len` bytes legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_lista_de_tabela(data: *const u8, len: i64) -> i64 {
    // SAFETY: garantido por quem chama (uma constante do módulo).
    let b = unsafe { std::slice::from_raw_parts(data, usize::try_from(len).unwrap_or(0)) };
    let mut items: Vec<crate::heap::Value> = Vec::new();
    let mut i = 0;
    let word = |b: &[u8], i: usize, n: usize| {
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
                b'n' => crate::heap::Value::Ref(0),
                b't' => crate::heap::Value::Bool(true),
                b'f' => crate::heap::Value::Bool(false),
                b'i' => {
                    let x = word(b, i, 8) as i64;
                    i += 8;
                    crate::heap::Value::Int(x)
                }
                b'd' => {
                    let x = f64::from_bits(word(b, i, 8));
                    i += 8;
                    crate::heap::Value::Double(x)
                }
                b's' => {
                    let n = word(b, i, 4) as usize;
                    i += 4;
                    // Strings literais são canônicas e permanentes (a tabela
                    // `literals` é raiz): nenhuma coleta no meio as perde.
                    let t = Texto::de_wtf8(&b[i..i + n]);
                    let h = heap.string_literal(t.vista());
                    i += n;
                    crate::heap::Value::Ref(h)
                }
                _ => panic!("bug do compilador: tabela de coleção com o marcador {tag}"),
            };
            items.push(v);
        }
        let h = heap.new_list(crate::layout::cid::LIST, items.len(), crate::listas::Element::General);
        // As caixas de `int` fora do `Smi` alocam: `list_set` enraíza `h`.
        for (k, v) in items.into_iter().enumerate() {
            heap.list_set(h, k, v);
        }
        h
    })
}

// ─── Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.6) ─────────────────

/// A lista literal (`AllocList`, `llvm/listas_ir.rs`): `n` palavras em
/// `words`, da classe `cid` (`_List`, `_ImmutableList` ou `_GrowableList`
/// de capacidade `n`). `shape`:
///
/// * `0`–`3` (`Element::code`): todas as palavras já na representação da
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
/// `words` aponta `n` palavras legíveis; um `shape` fora de `0..=3` aponta
/// `n` bytes legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_lista_nova(words: *const i64, n: i64, cid: i64, shape: i64) -> i64 {
    // SAFETY: o contrato desta entrada é o mesmo da implementação comum.
    unsafe { list_literal(words, n, cid, shape, false) }
}

/// Cria uma lista literal e entrega um owner independente do código.
/// Os elementos Ref são copiados, sem consumir owners do chamador, e ficam
/// enraizados durante todas as alocações. Não executa Dart nem lança pending.
///
/// # Safety
/// `n` deve ser não negativo e `words` conter `n` palavras legíveis.
/// Fora de `0..=3`, `shape` aponta `n` tags legíveis (1 int, 2 bool,
/// 3 Ref, 4 double). Referências devem estar vivas; bool usa somente 0/1.
///
/// # Panics
/// Comprimento, classe, tag ou bool inválido; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_arc_lista_nova_owned_v1([42].as_ptr(), 1, 10, 1) };
/// assert_eq!(dartforge_arc_lista_len_v1(list), 1);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_lista_nova_owned_v1(words: *const i64, n: i64, cid: i64, shape: i64) -> i64 {
    assert!(n >= 0, "comprimento de literal negativo");
    assert!(matches!(cid, 8..=10), "classe de literal inválida");
    // SAFETY: buffers e referências válidos pelo contrato da entrada.
    unsafe { list_literal(words, n, cid, shape, true) }
}

unsafe fn list_literal(words: *const i64, n: i64, cid: i64, shape: i64, owned: bool) -> i64 {
    use crate::listas::Element;
    let n = usize::try_from(n).unwrap_or(0);
    // SAFETY: garantido por quem chama (o `alloca` do código gerado).
    let p: &[i64] = if n == 0 { &[] } else { unsafe { std::slice::from_raw_parts(words, n) } };
    let cid = i32::try_from(cid).expect("bug do compilador: cid de lista inválido");
    let (e, tags): (Element, Option<&[u8]>) = match Element::from_code(shape) {
        Some(e) => (e, None),
        // SAFETY: garantido por quem chama (o vetor de tags do literal).
        None => (Element::General, Some(unsafe { std::slice::from_raw_parts(shape as usize as *const u8, n) })),
    };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // Tracing homogêneo conserva o preenchimento direto, sem vetor extra.
        let refs: Vec<i64> = if !owned && tags.is_none() { Vec::new() } else { p.iter().enumerate().map(|(k, &w)| {
            let tag = tags.map_or(if e == Element::General { 3 } else { 0 }, |t| t[k]);
            if owned {
                assert!(tags.is_none() || matches!(tag, 1..=4), "tag de literal inválida");
                assert!(!(tag == 2 || e == Element::Bool) || matches!(w, 0 | 1), "bool de literal inválido");
            }
            if tag == 3 { w } else { 0 }
        }).collect() };
        let frame = owned.then(|| {
            let frame = heap.push_owning_frame(refs.len());
            for (k, &r) in refs.iter().enumerate() { heap.set_root(frame, k, r); }
            frame
        });
        let h = if cid == crate::layout::cid::GROWABLE_LIST {
            heap.new_growable(n, n, e)
        } else {
            heap.new_list(cid, n, e)
        };
        let a = heap.list_data(h);
        match tags {
            None => {
                // Recém-alocada, jovem: as palavras entram sem barreira.
                if owned && e == Element::General { heap.write_refs(a, 1, p); }
                else { heap.words_mut(a)[1..1 + n].copy_from_slice(p); }
            }
            Some(tags) => {
                // Primeiro os `Ref` (sem alocação), depois as caixas (que
                // podem coletar; `list_set` enraíza `h`).
                if owned { heap.write_refs(a, 1, &refs); }
                else { heap.words_mut(a)[1..1 + n].copy_from_slice(&refs); }
                for (k, (&w, &t)) in p.iter().zip(tags).enumerate() {
                    let v = match t {
                        1 => crate::heap::Value::Int(w),
                        2 => crate::heap::Value::Bool(w != 0),
                        4 => crate::heap::Value::Double(f64::from_bits(w as u64)),
                        _ => continue,
                    };
                    heap.list_set(h, k, v);
                }
            }
        }
        if let Some(frame) = frame {
            heap.retain_code_owner(h);
            heap.pop_frame(frame);
        }
        h
    })
}

/// O `add` de uma `_GrowableList` com o `Ref` `v` (o caminho do código gerado
/// quando o armazenamento está cheio ou não guarda `v` sem caixa;
/// `lower/intrinsecos.rs`, `lower/literais.rs`): cresce para
/// `(capacidade * 2) | 3` se preciso e grava (descompactando, se for o caso).
/// Devolve `list`. Não lança; pode coletar (`list` e `v` são operandos,
/// enraizados pelo código gerado).
/// O retorno é emprestado do primeiro argumento; não cria owner independente.
/// O valor é armazenado sem consumir o owner do chamador; na forma geral,
/// copia a aresta Ref. Crescimento e descompactação invalidam empréstimos
/// anteriores do armazenamento.
///
/// # Panics
/// Receiver não expansível, referências mortas ou layout inválido; aborta na
/// fronteira C. Quem chama deve manter ambos os argumentos vivos.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_lista_nova(std::ptr::null(), 0, 10, 0) };
/// dartforge_arc_retain(list);
/// let value = dartforge_arc_box_int_owned_v1(i64::MAX);
/// assert_eq!(dartforge_lista_acrescentar(list, value), list);
/// dartforge_arc_release(value);
/// assert_eq!(dartforge_arc_lista_len_v1(list), 1);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_lista_acrescentar(list: i64, v: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().list_push(list, crate::heap::Value::Ref(v)));
    list
}

#[cfg(test)]
mod testes_literal_owned {
    use super::*;

    #[test]
    fn mixed_literal_roots_refs_counts_edges_and_preserves_boxes() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            for cid in [8, 9, 10] {
                let child = HEAP.with(|h| h.borrow_mut().allocate_str("elemento sem owner do código"));
                let bits = 0xfff8_1234_5678_9abc_u64;
                let words = [child, i64::MAX, bits as i64, 1, child, 0];
                let tags = [3u8, 1, 4, 2, 3, 3];
                let list = unsafe { dartforge_arc_lista_nova_owned_v1(words.as_ptr(), 6, cid, tags.as_ptr() as i64) };
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(list), 3);
                assert_eq!(dartforge_arc_observar_heap_v1(child), 1);
                HEAP.with(|h| {
                    let heap = h.borrow();
                    assert_eq!(heap.list_len(list), 6);
                    let a = heap.list_data(list);
                    let w = heap.words(a);
                    assert_eq!(w[1], child);
                    assert_eq!(w[5], child);
                    assert_eq!(w[6], 0);
                    assert_eq!(heap.value(w[2]), crate::heap::Value::Int(i64::MAX));
                    let crate::heap::Value::Double(d) = heap.value(w[3]) else { panic!("caixa double esperada") };
                    assert_eq!(d.to_bits(), bits);
                    assert_eq!(heap.value(w[4]), crate::heap::Value::Bool(true));
                });
                dartforge_arc_release(list);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(list), 0);
                assert_eq!(dartforge_arc_observar_heap_v1(child), 0);
            }
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn empty_and_compact_literal_preserves_bits_until_owner_release() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            for cid in [8, 9, 10] {
                for (shape, words) in [(0, vec![]), (1, vec![i64::MIN, i64::MAX]),
                    (2, vec![i64::MIN, 0x7ff8_1234_5678_9abc]), (3, vec![0, 1])] {
                    let list = unsafe { dartforge_arc_lista_nova_owned_v1(words.as_ptr(), words.len() as i64, cid, shape) };
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observar_heap_v1(list), 3);
                    HEAP.with(|h| {
                        let heap = h.borrow();
                        let data = heap.list_data(list);
                        assert_eq!(&heap.words(data)[1..1 + words.len()], &words);
                    });
                    dartforge_arc_release(list);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observar_heap_v1(list), 0);
                }
            }
            HEAP.with(|h| { h.replace(previous); });
        }
    }
}
