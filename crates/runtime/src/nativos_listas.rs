// Runtime nativo: natives de lista do SDK da fonte (P5c/P5d, δ) e os erros
// que o runtime lança como objetos Dart da fonte.
//
// `_List`, `_ImmutableList` e `_GrowableList` são o mesmo `Value::List` do
// heap: a classe sai das marcas `fixas`/`imutaveis` (`cid_do_runtime`). A
// VM guarda a `_GrowableList` como (tamanho, `_List`) e a lê pelos natives
// `GrowableList_*`; aqui o vetor é um só, e os natives são as mesmas
// operações sobre ele (`_setData` copia o conteúdo do `_List` novo — que
// `_grow`/`_shrink` acabaram de preencher com os mesmos elementos — e fica
// com a capacidade dele). Nada disso é observável pelo programa.
//
// Os erros: a biblioteca `dart:_internal` da sobreposição
// (`sdk_nativo/internal/print_patch.dart`) tem funções `_dartforge*` que
// constroem o erro da fonte (`IndexError`, `RangeError`…); o registro dela
// entrega os endereços ao runtime (`dartforge_registrar_ajudante`). Sem o
// SDK da fonte, o erro é o do runtime (`excecoes.rs`).

thread_local! {
    /// Funções Dart que o runtime chama, pelo nome (`_dartforgeErroDeIndice`…).
    static AJUDANTES: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
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
    let n = HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(Texto::de_str("index"))));
    let err = com_raizes(&[n], || dartforge_range_error_index(indice, tamanho, n, 0));
    dartforge_exception_throw(err, 3);
}

fn lista_len(this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().list_len(this) as i64)
}

/// `_List(length)`: `length` nulls, tamanho fixo. O parâmetro não tem tipo
/// na declaração (`external factory _List(length)`): chega como `Ref`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_List_allocate(length: i64) -> i64 {
    let n = HEAP.with(|heap| heap.borrow().int_de_ref(length)).unwrap_or(0).max(0) as usize;
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.allocate(Value::List(vec![TaggedValue::reference(0); n]));
        heap.fixas.insert(h);
        h
    })
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

fn gravar_indice(this: i64, indice: i64, valor: i64) {
    let n = lista_len(this);
    if indice < 0 || indice >= n {
        lancar_indice(indice, this, n);
        return;
    }
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let v = heap.normalizar(TaggedValue::reference(valor));
        heap.list_set(this, indice as usize, v);
    });
}

/// `lista[i]` de `_List`, `_ImmutableList` e `_GrowableList` (intrínseco
/// `[]` da VM), com a conferência de índice.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_lista_get(this: i64, indice: i64) -> i64 {
    let n = lista_len(this);
    if indice < 0 || indice >= n {
        lancar_indice(indice, this, n);
        return 0;
    }
    let v = HEAP.with(|heap| heap.borrow().list_get(this, indice as usize));
    valor_como_ref(v)
}

/// `_List._sliceInternal(start, count, needsTypeArgument)`: `_List` novo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_List_slice(this: i64, inicio: i64, quantos: i64, _tipo: u8) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let Value::List(itens) = heap.get(this) else { return 0 };
        let fatia: Vec<TaggedValue> = itens[inicio as usize..(inicio + quantos) as usize].to_vec();
        let h = heap.allocate(Value::List(fatia));
        heap.fixas.insert(h);
        h
    })
}

/// `_ImmutableList._from(from, offset, length)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ImmutableList_from(de: i64, inicio: i64, quantos: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let Value::List(itens) = heap.get(de) else { return 0 };
        let fatia: Vec<TaggedValue> = itens[inicio as usize..(inicio + quantos) as usize].to_vec();
        let h = heap.allocate(Value::List(fatia));
        heap.imutaveis.insert(h);
        h
    })
}

/// `_GrowableList._withData(data)`: tamanho 0, capacidade a do `_List`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_allocate(dados: i64) -> i64 {
    let cap = lista_len(dados) as usize;
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(Vec::with_capacity(cap))))
}

/// `_GrowableList._capacity`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_getCapacity(this: i64) -> i64 {
    HEAP.with(|heap| match heap.borrow().get(this) {
        Value::List(itens) => itens.capacity() as i64,
        _ => 0,
    })
}

/// `_GrowableList.length`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_getLength(this: i64) -> i64 {
    lista_len(this)
}

/// `_GrowableList._setLength(n)`: corta, ou cresce com null.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_setLength(this: i64, n: i64) {
    HEAP.with(|heap| {
        if let Value::List(itens) = heap.borrow_mut().get_mut(this) {
            itens.resize(n.max(0) as usize, TaggedValue::reference(0));
        }
    });
}

/// `_GrowableList._setData(data)`: o conteúdo (até o tamanho corrente) e a
/// capacidade passam a ser os do `_List`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_setData(this: i64, dados: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let Value::List(novos) = heap.get(dados) else { return };
        let novos = novos.clone();
        if let Value::List(itens) = heap.get_mut(this) {
            let n = itens.len().min(novos.len());
            let mut v = Vec::with_capacity(novos.len());
            v.extend_from_slice(&novos[..n]);
            *itens = v;
        }
    });
}

/// `_GrowableList._setIndexed(i, v)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_setIndexed(this: i64, indice: i64, valor: i64) {
    gravar_indice(this, indice, valor);
}

/// `ClassID.getID(o)`: o id de classe do valor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ClassID_getID(o: i64) -> i64 {
    dartforge_value_class(o)
}

/// `identical(a, b)` (`Identical_comparison`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Identical_comparison(a: i64, b: i64) -> u8 {
    dartforge_identical(a, b)
}

/// `Object.==` (`Object_equals`): identidade.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_equals(this: i64, outro: i64) -> u8 {
    dartforge_identical(this, outro)
}

/// `printToConsole(line)` da sobreposição (`DartForge_imprimir`): a linha e
/// o fim de linha, como o `print` da VM (o substituto solto vira U+FFFD).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_imprimir(linha: i64) {
    dartforge_print_string(linha);
}
