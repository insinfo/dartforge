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

/// `_StringBase.codeUnitAt(i)` (intrínseco da VM), com a conferência de
/// índice.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_codeUnitAt(this: i64, indice: i64) -> i64 {
    let t = texto_de(this);
    if indice < 0 || indice as usize >= t.len() {
        lancar_indice(indice, this, t.len() as i64);
        return 0;
    }
    i64::from(t.unidade(indice as usize))
}

/// `_StringBase._concatRangeNative(strings, start, end)`: a concatenação
/// de `strings[start..end]` (`String_concatRange`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_concatRange(lista: i64, inicio: i64, fim: i64) -> i64 {
    let mut saida = TextoMut::new();
    let n = lista_len(lista);
    for i in inicio.max(0)..fim.min(n) {
        let v = HEAP.with(|heap| heap.borrow().list_get(lista, i as usize));
        if v.is_ref && v.bits != 0 {
            let t = texto_de(v.bits);
            saida.push_texto(&t);
        }
    }
    alocar_texto(saida.fim())
}

/// `has63BitSmis()`: o `Smi` daqui tem 63 bits (R10).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_verdadeiro() -> u8 {
    1
}

/// `_Smi.hashCode`/`_Mint.hashCode`: o próprio valor (a VM devolve o `Smi`;
/// o `_Mint` cabe no `hashCode` de 64 bits daqui).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_int_hashCode(this: i64) -> i64 {
    this
}

/// `Object._getHash`/`hashCode` de identidade: o handle do objeto, estável
/// enquanto ele vive e único entre os vivos (a VM sorteia; o valor não é
/// observável pelo programa além de igualdade).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_getHash(o: i64) -> i64 {
    if crate::heap::smi::e_smi(o) {
        return crate::heap::smi::valor(o);
    }
    (o >> 1) & 0x3fff_ffff
}

/// `Object.toString()` (`Object_toString`): `Instance of 'Classe'`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_toString(this: i64) -> i64 {
    let cid = dartforge_value_class(this);
    let nome = CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned()).unwrap_or_default();
    alocar_str(&format!("Instance of '{nome}'"))
}

/// `Object._haveSameRuntimeType(a, b)`: a mesma classe (os argumentos de
/// tipo ficam para a RTI).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_haveSameRuntimeType(a: i64, b: i64) -> u8 {
    u8::from(dartforge_value_class(a) == dartforge_value_class(b))
}

/// `_trySetStackTrace(error, stackTrace)`: o rastro dos erros da fonte fica
/// para depois (o `Error.stackTrace` devolve null).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Error_trySetStackTrace(_erro: i64, _rastro: i64) {}

/// `makeListFixedLength(list)`: uma `_List` com os mesmos elementos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_makeListFixedLength(lista: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let Value::List(itens) = heap.get(lista) else { return 0 };
        let itens = itens.clone();
        let h = heap.allocate(Value::List(itens));
        heap.fixas.insert(h);
        h
    })
}

/// `makeFixedListUnmodifiable(list)`: uma `_ImmutableList` com os mesmos
/// elementos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_makeFixedListUnmodifiable(lista: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let Value::List(itens) = heap.get(lista) else { return 0 };
        let itens = itens.clone();
        let h = heap.allocate(Value::List(itens));
        heap.imutaveis.insert(h);
        h
    })
}

/// `_Double.toInt()`: truncado; NaN e infinito lançam `UnsupportedError`
/// na VM (aqui: o valor saturado, até os erros da fonte chegarem aqui).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_toInt(this: f64) -> i64 {
    this as i64
}

/// `_Double.floorToDouble()` e afins.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_floor(this: f64) -> f64 {
    this.floor()
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_ceil(this: f64) -> f64 {
    this.ceil()
}

/// `roundToDouble`: metade para longe de zero (`round` da VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_round(this: f64) -> f64 {
    this.round()
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_truncate(this: f64) -> f64 {
    this.trunc()
}

/// `_Double._modulo(other)`: `%` euclidiano de `double` (resultado nunca
/// negativo), como `DoubleModulo` da VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_modulo(this: f64, outro: f64) -> f64 {
    let r = this % outro;
    if r < 0.0 { r + outro.abs() } else { r }
}

/// `_Double._remainder(other)`: o resto com o sinal do dividendo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_remainder(this: f64, outro: f64) -> f64 {
    this % outro
}

/// `_Double.hashCode`: o do `int` quando o valor é inteiro (`1.0.hashCode ==
/// 1.hashCode`, exigido pela igualdade de `num`), senão os bits.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_hashCode(this: f64) -> i64 {
    if this.is_finite() && this == this.trunc() && this.abs() < 9.0e18 {
        return this as i64;
    }
    let b = this.to_bits();
    ((b ^ (b >> 32)) & 0x3fff_ffff) as i64
}
