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
    let n = HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(Texto::de_str("index"))));
    let err = com_raizes(&[n], || dartforge_range_error_index(indice, tamanho, n, 0));
    dartforge_exception_throw(err, 3);
}

/// A conferência de limites de `[]` (`nome` "length") e do `_setIndexed`
/// das listas ("index") na VM: `RangeError.range(indice, 0, tamanho - 1,
/// nome)`, não `IndexError` (ex.: "RangeError (length): Invalid value: Not
/// in inclusive range 0..2: 10").
fn lancar_faixa(indice: i64, tamanho: i64, nome: &str) {
    let n = HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(Texto::de_str(nome))));
    let erro = com_raizes(&[n], || dartforge_range_error_range(indice, 0, tamanho - 1, n, 0));
    if dartforge_exception_pending() != 0 {
        return;
    }
    com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
}

fn lista_len(this: i64) -> i64 {
    HEAP.with(|heap| match heap.borrow().get(this) {
        Value::List(e) => e.len_logico() as i64,
        _ => panic!("lista esperada"),
    })
}

/// `lista[i] = v` direto (`lower/tipados.rs`) com `v` `int` (`codigo` 1),
/// `double` (2) ou `bool` (3): o comprimento de `h` se ela é uma lista do
/// runtime modificável e compacta dessa forma (N14: o `E` reificado é o
/// escalar, e o código gerado grava os 8 bytes dos bits); senão 0 (o `[]=`
/// do SDK, com a conferência de covariância e os erros da VM). Uma
/// `List<Never>` vista como `List<int>` cai aqui. Não muda a forma nem os
/// dados (o laço de N13 guarda o endereço deles): só lê o heap e marca o
/// bit no cabeçalho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_lista_len_gravavel(h: i64, codigo: i64) -> i64 {
    heap_sem_emprestimo(|heap| {
        let imutavel = !heap.imutaveis.is_empty() && heap.imutaveis.contains(&h);
        match heap.try_get(h) {
            Some(Value::List(e)) if !imutavel && Some(e.forma()) == crate::heap::FormaDeLista::do_codigo(codigo) => {}
            _ => return 0,
        }
        let Value::List(itens) = heap.get_mut(h) else { unreachable!() };
        // Conferido: o código gerado passa a gravar direto, pelo cabeçalho,
        // até o `E` ou a imutabilidade mudarem (`Heap::set_metadado`,
        // `Heap::marcar_imutavel`).
        itens.conferir_gravacao(codigo);
        itens.len_logico() as i64
    })
}

/// `lista.add(v)` direto (`lower/intrinsecos.rs`) com `v` `int` (`codigo`
/// 1), `double` (2) ou `bool` (3) sem caixa, nos bits: 1 se acrescentou,
/// 0 se o caminho não serve e o código gerado chama o `add` do SDK (uma
/// classe do usuário, lista de tamanho fixo ou não modificável, `E` que não
/// aceita o valor, a reserva pendente de `_withData`). Conferido uma vez por
/// lista (o bit `1 << (codigo + 4)` do cabeçalho). O mesmo efeito do
/// `_GrowableList.add` da VM: o elemento no fim, o comprimento mais um; a
/// capacidade cresce pelo vetor. Pode coletar (como `list_push`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_lista_add_escalar(h: i64, bits: i64, codigo: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let Some(Value::List(itens)) = heap.try_get(h) else { return 0 };
        if itens.logico().is_some() {
            return 0;
        }
        let bit = codigo + 4;
        if !itens.gravacao_conferida(bit) {
            if heap.fixas.contains(&h) || heap.imutaveis.contains(&h) {
                return 0;
            }
            let meta = heap.metadado(h);
            if meta != 0 && !lista_aceita_escalar(meta - 1, codigo) {
                return 0;
            }
            let Value::List(itens) = heap.get_mut(h) else { unreachable!() };
            itens.conferir_gravacao(bit);
        }
        let v = match codigo {
            1 => TaggedValue::scalar(bits),
            2 => TaggedValue::double(f64::from_bits(bits as u64)),
            _ => TaggedValue::boolean(bits != 0),
        };
        heap.list_push(h, v);
        1
    })
}

/// O cabeçalho de `h` (`heap::CabecalhoDeLista`: endereço dos elementos,
/// comprimento lógico e gravações conferidas) se ela é uma lista do
/// runtime, senão o `CABECALHO_VAZIO` (comprimento 0). O endereço não muda
/// enquanto a lista vive: para o código gerado é uma função pura do handle
/// (`memory(none)`), e sai dos laços. Não entra em pânico com null, `Smi`
/// ou escalar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_lista_cabecalho(h: i64) -> i64 {
    heap_sem_emprestimo(|heap| match heap.try_get(h) {
        Some(Value::List(itens)) => itens.cabecalho() as i64,
        _ => std::ptr::addr_of!(crate::heap::CABECALHO_VAZIO) as i64,
    })
}

/// O comprimento de `h` se ela é uma lista do runtime (`_List`,
/// `_GrowableList`, `_ImmutableList`), senão -1: o `for-in` direto
/// (`lower/sdk_fonte.rs`) distingue a lista vazia de uma classe do usuário
/// que implementa `List` (que fica com o `Iterator` dela). Só lê o heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_lista_len_ou_menos1(h: i64) -> i64 {
    heap_sem_emprestimo(|heap| match heap.try_get(h) {
        Some(Value::List(itens)) => {
            itens.len_logico() as i64
        }
        _ => -1,
    })
}

/// `lista[i]` (já conferido) numa posição `Ref` (a caixa de um escalar, se
/// for o caso): o caminho do código gerado quando a tag do elemento não é
/// a da representação esperada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_lista_ref(h: i64, i: i64) -> i64 {
    let v = HEAP.with(|heap| match heap.borrow().try_get(h) {
        Some(Value::List(itens)) => usize::try_from(i).ok().and_then(|i| itens.get(i)),
        _ => None,
    });
    v.map_or(0, valor_como_ref)
}

/// `_List(length)`: `length` nulls, tamanho fixo. O parâmetro não tem tipo
/// na declaração (`external factory _List(length)`): chega como `Ref`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_List_allocate(length: i64, tupla: i64) -> i64 {
    let n = HEAP.with(|heap| heap.borrow().int_de_ref(length)).unwrap_or(0).max(0) as usize;
    let h = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.allocate(Value::List(vec![TaggedValue::reference(0); n].into()));
        heap.fixas.insert(h);
        h
    });
    if let Some(tipo) = tipo_lista_da_tupla(tupla, cid_do_runtime(h)) {
        definir_tipo_da_lista(h, tipo + 1);
    }
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

fn gravar_indice(this: i64, indice: i64, valor: i64) {
    let n = lista_len(this);
    if indice < 0 || indice >= n {
        lancar_faixa(indice, n, "index");
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
        lancar_faixa(indice, n, "length");
        return 0;
    }
    let v = HEAP.with(|heap| heap.borrow().list_get(this, indice as usize));
    valor_como_ref(v)
}

/// `_List._sliceInternal(start, count, needsTypeArgument)`: `_List` novo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_List_slice(this: i64, inicio: i64, quantos: i64, _tipo: u8) -> i64 {
    let h = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let Value::List(itens) = heap.get(this) else { return 0 };
        let fatia = itens.fatia(inicio as usize, (inicio + quantos) as usize);
        let h = heap.allocate(Value::List(fatia));
        heap.fixas.insert(h);
        h
    });
    if h != 0 && let Some(tipo) = tipo_lista_copiada(this, cid_do_runtime(h)) {
        definir_tipo_da_lista(h, tipo + 1);
    }
    h
}

/// `_ImmutableList._from(from, offset, length)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ImmutableList_from(de: i64, inicio: i64, quantos: i64, tupla: i64) -> i64 {
    let h = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let Value::List(itens) = heap.get(de) else { return 0 };
        let fatia = itens.fatia(inicio as usize, (inicio + quantos) as usize);
        let h = heap.allocate(Value::List(fatia));
        heap.marcar_imutavel(h);
        h
    });
    if h != 0 && let Some(tipo) = tipo_lista_da_tupla(tupla, cid_do_runtime(h)) {
        definir_tipo_da_lista(h, tipo + 1);
    }
    h
}

/// `_GrowableList._withData(data)`: tamanho 0; os elementos de `data` ficam
/// como reserva (o `_setLength` seguinte os expõe, como na VM, onde a lista
/// aponta para o `_List`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_allocate(dados: i64, tupla: i64) -> i64 {
    let h = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let Value::List(itens) = heap.get(dados) else { return 0 };
        let itens = itens.clone();
        let mut itens = itens;
        itens.definir_logico(Some(0));
        heap.allocate(Value::List(itens))
    });
    if h != 0 && let Some(tipo) = tipo_lista_da_tupla(tupla, cid_do_runtime(h)) {
        definir_tipo_da_lista(h, tipo + 1);
    }
    h
}

/// `_GrowableList._capacity`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_getCapacity(this: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(this) {
            Value::List(itens) if itens.logico().is_some() => itens.len() as i64,
            Value::List(itens) => itens.capacity() as i64,
            _ => 0,
        }
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
        let mut heap = heap.borrow_mut();
        // A reserva de `_withData` vira os elementos até `n`.
        if let Value::List(itens) = heap.get_mut(this) {
            itens.definir_logico(None);
            itens.redimensionar(n.max(0) as usize, TaggedValue::reference(0));
        }
    });
}

/// `_GrowableList._setData(data)`: o conteúdo (até o tamanho corrente) e a
/// capacidade passam a ser os do `_List`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_GrowableList_setData(this: i64, dados: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let Value::List(itens) = heap.get(this) else { return };
        let n_atual = itens.len_logico();
        // Uma cópia só: os `n` primeiros de `dados`, com a capacidade dele
        // (antes, um clone de `dados` inteiro e depois a cópia).
        let Value::List(novos) = heap.get(dados) else { return };
        let n = n_atual.min(novos.len());
        let mut v = Vec::with_capacity(novos.len());
        v.extend(novos.iter().take(n));
        if let Value::List(itens) = heap.get_mut(this) {
            // No mesmo `Elementos`: o cabeçalho não muda de endereço, e a
            // lista compacta continua compacta (N14).
            itens.definir_logico(None);
            itens.substituir(v);
        }
    });
}

/// `_preencherLista(lista, valor)` da sobreposição (`array.dart`):
/// os elementos de `_List.filled`/`_GrowableList.filled`, de uma vez.
/// `valor` já é do `E` da lista (o parâmetro da fábrica); entra normalizado
/// (um `int`, `double` ou `bool` sem caixa), como pelo `[]=`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_List_preencher(lista: i64, valor: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let v = heap.normalizar(TaggedValue::reference(valor));
        if let Value::List(itens) = heap.get_mut(lista) {
            let n = itens.len_logico();
            itens.preencher(n, v);
        }
    });
}

/// A lista `h` mudou de classe concreta depois de ganhar o tipo — a
/// constante `const <int>[…]` nasce como o literal (`_GrowableList<int>`,
/// `dartforge_rti_definir`) e só depois é marcada não modificável, quando
/// passa a `_ImmutableList`: o metadado segue para a classe nova, com os
/// mesmos argumentos. Sem isto, os métodos de `_Array` (o `toList` da
/// constante) avaliavam o `E` do receptor como `dynamic`.
fn reclassificar_lista(h: i64) {
    if !smi::e_handle(h) {
        return;
    }
    let meta = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.try_get(h) {
            Some(Value::List(_)) => heap.metadado(h),
            _ => 0,
        }
    });
    if meta == 0 {
        return;
    }
    let Some(classe) = cid_do_runtime(h) else { return };
    let novo = RTI.with(|u| {
        let mut u = u.borrow_mut();
        let lista = u.rt.list;
        match u.tipo(meta - 1).clone() {
            Tipo::Interface(c, args)
                if c != classe && args.len() == 1 && u.como_supertipo(c, &args, lista).is_some() =>
            {
                Some(u.internar(Tipo::Interface(classe, args)))
            }
            _ => None,
        }
    });
    if let Some(t) = novo {
        HEAP.with(|heap| heap.borrow_mut().set_metadado(h, t + 1));
    }
}

/// `_copiarElementos(destino, origem, n)` da sobreposição (`array.dart`):
/// os `n` primeiros elementos de `origem` nos `n` primeiros de `destino`,
/// duas listas do runtime (`List.of`, `toList` e as cópias de
/// `_List`/`_GrowableList`). Os elementos saem com a tag, sem caixa: um
/// `int` fora do `Smi` não vira `_Mint`, e a cópia entre listas compactas
/// da mesma forma (N14) grava os bits direto. `destino` acabou de ser
/// criada com o `E` dela, e `origem` é um `Iterable<E>` (o parâmetro da
/// fábrica), então todo elemento cabe na forma do destino; um que não
/// coubesse devolveria o destino à forma geral, como pelo `[]=`. Não
/// aloca no heap Dart.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_List_copiar(destino: i64, origem: i64, n: i64) {
    let n = usize::try_from(n).unwrap_or(0);
    if n == 0 {
        return;
    }
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let fonte: Vec<TaggedValue> = match heap.get(origem) {
            Value::List(itens) => itens.iter().take(n.min(itens.len_logico())).collect(),
            _ => return,
        };
        if let Value::List(itens) = heap.get_mut(destino) {
            let limite = itens.len_logico().min(itens.len());
            for (i, v) in fonte.into_iter().enumerate().take(limite) {
                itens.definir(i, v);
            }
        }
    });
}

/// `_GrowableList._grow(capacidade)` da sobreposição
/// (`sdk_nativo/core/growable_array.dart`): a capacidade reservada no
/// vetor da lista, onde os elementos já estão. Uma lista de `_withData`
/// ainda com o tamanho lógico pendente fica com os `n` primeiros.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_GrowableList_reservar(this: i64, capacidade: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if let Value::List(itens) = heap.get_mut(this) {
            let pendente = itens.logico();
            itens.definir_logico(None);
            if let Some(n) = pendente {
                itens.truncar(n);
            }
            let cap = usize::try_from(capacidade).unwrap_or(0);
            let len = itens.len();
            if cap > len {
                itens.reservar_exato(cap - len);
            }
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

/// `_StringBase._iguais(outro)` da sobreposição (`string_patch.dart`): as
/// mesmas unidades UTF-16 (o laço do `==` da VM, feito aqui).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_iguais(this: i64, outro: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().string_equal(this, outro)))
}

/// `_StringBase.codeUnitAt(i)` (intrínseco da VM), com a conferência de
/// índice.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_codeUnitAt(this: i64, indice: i64) -> i64 {
    let r = com_texto(this, |t| usize::try_from(indice).ok().filter(|&i| i < t.len()).map(|i| t.unidade(i)).ok_or(t.len()));
    match r {
        Ok(u) => i64::from(u),
        Err(n) => {
            lancar_faixa(indice, n as i64, "length");
            0
        }
    }
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

/// `Object._getHash`/`hashCode` de identidade: o de um objeto do usuário
/// fica no cabeçalho dele (`Heap::hash_de_identidade`, como o da VM); o de
/// um valor do runtime é o handle, estável enquanto ele vive e único entre
/// os vivos (a VM sorteia; o valor não é observável pelo programa além de
/// igualdade).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_getHash(o: i64) -> i64 {
    if crate::heap::smi::e_smi(o) {
        return crate::heap::smi::valor(o);
    }
    if let Some(h) = HEAP.with(|h| h.borrow_mut().hash_de_identidade(o)) {
        return h;
    }
    (o >> 2) & 0x3fff_ffff
}

/// `Object.toString()` (`Object_toString`): `Instance of 'Classe'`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_toString(this: i64) -> i64 {
    // O `Object_toString` da VM também escreve os números (o `_Mint` e o
    // `_Double` não têm `toString` próprio na fonte).
    if let Some(i) = HEAP.with(|h| h.borrow().int_de_ref(this)) {
        return dartforge_to_string_i64(i);
    }
    if let Some(Value::BoxedDouble(d)) = HEAP.with(|h| h.borrow().try_get(this).map(|v| match v {
        Value::BoxedDouble(d) => Value::BoxedDouble(*d),
        _ => Value::BoxedInt(0),
    })) {
        return dartforge_nativo_Double_toString(d);
    }
    if let Some(t) = texto_simd(this) {
        return alocar_str(&t);
    }
    let cid = dartforge_value_class(this);
    // Genérica com argumentos reificados: o nome inclui os argumentos
    // (`Instance of 'Caixa<int>'`), como a VM; sem argumentos, o nome
    // registrado da classe, como antes.
    let nome = texto_com_argumentos(this)
        .unwrap_or_else(|| CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned()).unwrap_or_default());
    alocar_str(&format!("Instance of '{nome}'"))
}

/// `Object._haveSameRuntimeType(a, b)`: a mesma classe (os argumentos de
/// tipo ficam para a RTI).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_haveSameRuntimeType(a: i64, b: i64) -> u8 {
    u8::from(dartforge_value_class(a) == dartforge_value_class(b))
}

/// `Object.runtimeType`: usa o universo RTI, inclusive argumentos de tipo
/// reificados, e devolve o objeto `Type` canônico do isolate.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_runtimeType(this: i64) -> i64 {
    dartforge_rti_objeto_tipo(dartforge_rti_do_valor(this))
}

/// `makeListFixedLength(list)`: uma `_List` com os mesmos elementos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_makeListFixedLength(lista: i64) -> i64 {
    let h = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let Value::List(itens) = heap.get(lista) else { return 0 };
        let itens = itens.clone();
        let h = heap.allocate(Value::List(itens));
        heap.fixas.insert(h);
        h
    });
    if h != 0 && let Some(tipo) = tipo_lista_copiada(lista, cid_do_runtime(h)) {
        definir_tipo_da_lista(h, tipo + 1);
    }
    h
}

/// `makeFixedListUnmodifiable(list)`: uma `_ImmutableList` com os mesmos
/// elementos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_makeFixedListUnmodifiable(lista: i64) -> i64 {
    let h = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let Value::List(itens) = heap.get(lista) else { return 0 };
        let itens = itens.clone();
        let h = heap.allocate(Value::List(itens));
        heap.marcar_imutavel(h);
        h
    });
    if h != 0 && let Some(tipo) = tipo_lista_copiada(lista, cid_do_runtime(h)) {
        definir_tipo_da_lista(h, tipo + 1);
    }
    h
}

/// `_Double.toInt()`: truncado, saturado fora da faixa de 64 bits (como a
/// VM); NaN e infinito lançam `UnsupportedError("Infinity or NaN toInt")`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_toInt(this: f64) -> i64 {
    if !this.is_finite() {
        let m = alocar_str("Infinity or NaN toInt");
        let e = com_raizes(&[m], || dartforge_unsupported_error_new(m));
        com_raizes(&[e], || dartforge_exception_throw(e, 3));
        return 0;
    }
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
    if r < 0.0 {
        r + outro.abs()
    } else if r == 0.0 {
        // A VM normaliza -0.0 para +0.0 no módulo; remainder preserva o sinal.
        0.0
    } else {
        r
    }
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

/// Os elementos `[inicio, fim)` de uma lista de inteiros (códigos).
fn codigos_da_lista(lista: i64, inicio: i64, fim: i64) -> Vec<i64> {
    let n = lista_len(lista);
    (inicio.max(0)..fim.min(n))
        .map(|i| {
            let v = HEAP.with(|heap| heap.borrow().list_get(lista, i as usize));
            HEAP.with(|heap| heap.borrow().normalizar(v)).bits
        })
        .collect()
}

/// `_OneByteString._allocateFromOneByteList(list, start, end)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_OneByteString_allocateFromOneByteList(lista: i64, inicio: i64, fim: i64) -> i64 {
    let u: Vec<u16> = codigos_da_lista(lista, inicio, fim).into_iter().map(|c| (c & 0xFF) as u16).collect();
    alocar_texto(Texto::de_unidades(u))
}

/// `_TwoByteString._allocateFromTwoByteList(list, start, end)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TwoByteString_allocateFromTwoByteList(lista: i64, inicio: i64, fim: i64) -> i64 {
    let u: Vec<u16> = codigos_da_lista(lista, inicio, fim).into_iter().map(|c| (c & 0xFFFF) as u16).collect();
    alocar_texto(Texto::de_unidades(u))
}

/// `_StringBase._createFromCodePoints(list, start, end)`: pontos de código
/// (acima de U+FFFF viram o par substituto).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_StringBase_createFromCodePoints(lista: i64, inicio: i64, fim: i64) -> i64 {
    let mut u = Vec::new();
    for c in codigos_da_lista(lista, inicio, fim) {
        crate::heap::empurrar_ponto(&mut u, c as u32);
    }
    alocar_texto(Texto::de_unidades(u))
}

/// `_StringBase._joinReplaceAllResult(base, matches, length, oneByte)`: as
/// fatias de `base` (um `Smi` negativo `-(início << 11 | tamanho)`, ou o par
/// início, fim) e as substituições, na ordem (`string_patch.dart`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_StringBase_joinReplaceAllResult(base: i64, partes: i64, _tamanho: i64, _um_byte: u8) -> i64 {
    let b = texto_de(base);
    let mut saida = TextoMut::new();
    let n = lista_len(partes);
    let mut i = 0;
    while i < n {
        let v = HEAP.with(|heap| heap.borrow().list_get(partes, i as usize));
        let v = HEAP.with(|heap| heap.borrow().normalizar(v));
        if v.is_ref {
            saida.push_texto(&texto_de(v.bits));
        } else {
            let (ini, fim) = if v.bits < 0 {
                let bits = -v.bits;
                let ini = bits >> 11;
                (ini, ini + (bits & ((1 << 11) - 1)))
            } else {
                i += 1;
                let f = HEAP.with(|heap| heap.borrow().list_get(partes, i as usize));
                let f = HEAP.with(|heap| heap.borrow().normalizar(f));
                (v.bits, f.bits)
            };
            saida.push_texto(&b.fatia(ini as usize, fim as usize));
        }
        i += 1;
    }
    alocar_texto(saida.fim())
}

/// `_Closure.==` (`Closure_equals`): a mesma função e o mesmo receptor (o
/// tear-off de um método sobre o mesmo objeto); closures comuns só são
/// iguais a si mesmas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Closure_equals(this: i64, outro: i64) -> u8 {
    if this == outro {
        return 1;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match (heap.try_get(this), heap.try_get(outro)) {
            (Some(Value::Closure(ca)), Some(Value::Closure(cb))) if ca.code_id == cb.code_id => {
                match (heap.try_get(ca.environment), heap.try_get(cb.environment)) {
                    (Some(Value::Environment(x)), Some(Value::Environment(y))) if x.len() == 1 && y.len() == 1 => {
                        u8::from(x[0].is_ref && y[0].is_ref && x[0].bits == y[0].bits)
                    }
                    _ => 0,
                }
            }
            _ => 0,
        }
    })
}

/// `double._nativeParse(str, start, end)` (`Double_parse`): o número do
/// texto `[start, end)` ou null — decimal com expoente opcional, `NaN`,
/// `Infinity`, com sinal (o `CStringToDouble` da VM; `inf`/`nan` em
/// minúsculas não são aceitos, como na VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_parse(texto: i64, inicio: i64, fim: i64) -> i64 {
    let t = texto_de(texto).fatia(inicio.max(0) as usize, fim.max(0) as usize).para_string();
    let corpo = t.strip_prefix(['+', '-']).unwrap_or(&t);
    let valido = corpo == "NaN"
        || corpo == "Infinity"
        || (!corpo.is_empty()
            && corpo.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
            && corpo.chars().any(|c| c.is_ascii_digit()));
    if !valido {
        return 0;
    }
    let v = if corpo == "NaN" {
        f64::NAN
    } else if corpo == "Infinity" {
        if t.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY }
    } else {
        match t.parse::<f64>() {
            Ok(v) => v,
            Err(_) => return 0,
        }
    };
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::BoxedDouble(v)))
}

/// Intrínsecos de `dart:math` (`_sqrt`, `_sin`…): a função da libm do Rust.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_sqrt(x: f64) -> f64 {
    x.sqrt()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_sin(x: f64) -> f64 {
    x.sin()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_cos(x: f64) -> f64 {
    x.cos()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_tan(x: f64) -> f64 {
    x.tan()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_acos(x: f64) -> f64 {
    x.acos()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_asin(x: f64) -> f64 {
    x.asin()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_atan(x: f64) -> f64 {
    x.atan()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_atan2(a: f64, b: f64) -> f64 {
    a.atan2(b)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_exp(x: f64) -> f64 {
    x.exp()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_log(x: f64) -> f64 {
    x.ln()
}
/// `_doublePow(base, exponent)`: `pow` da C (a VM chama `pow`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_pow(base: f64, expoente: f64) -> f64 {
    base.powf(expoente)
}

/// `_Record._numFields`: quantos campos (o record posicional do runtime).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_record_numFields(this: i64) -> i64 {
    HEAP.with(|heap| match heap.borrow().try_get(this) {
        Some(Value::Record(v)) => v.len() as i64,
        _ => 0,
    })
}

/// `_Record._shape`: a forma — para o record posicional, o número de campos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_record_shape(this: i64) -> i64 {
    dartforge_nativo_DartForge_record_numFields(this)
}

/// `_Record._fieldNames`: os nomes (nenhum no record posicional).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_record_fieldNames(_this: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.allocate(Value::List(Vec::new().into()));
        heap.marcar_imutavel(h);
        h
    })
}

/// `_Record._fieldAt(i)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_record_fieldAt(this: i64, i: i64) -> i64 {
    let v = HEAP.with(|heap| match heap.borrow().try_get(this) {
        Some(Value::Record(v)) => v.get(i as usize).copied(),
        _ => None,
    });
    v.map_or(0, valor_como_ref)
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

#[cfg(test)]
mod testes_modulo_double {
    use super::dartforge_nativo_DartForge_double_modulo as modulo;

    #[test]
    fn segue_sinais_e_zero_da_vm() {
        assert_eq!(modulo(-7.5, 2.0), 0.5);
        assert_eq!(modulo(-7.5, -2.0), 0.5);
        assert_eq!(modulo(7.5, -2.0), 1.5);
        assert_eq!(modulo(-7.5, f64::INFINITY), f64::INFINITY);
        assert_eq!(modulo(-0.0, 2.0).to_bits(), 0.0f64.to_bits());
        assert!(modulo(7.5, 0.0).is_nan());
    }
}

// ---------------------------------------------------------------------------
// Referências fracas e efêmeros (`WeakReference`, `Expando`): o estado mora
// nas tabelas `fracas`/`efemeros` do heap, que a coleta trata (`heap.rs`).

/// `WeakReference.target` (`WeakReference_getTarget`): o alvo, ou null se
/// foi coletado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakReference_getTarget(this: i64) -> i64 {
    HEAP.with(|h| h.borrow().fracas.get(&this).copied().unwrap_or(0))
}

/// `_WeakReference._target =` (`WeakReference_setTarget`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakReference_setTarget(this: i64, alvo: i64) {
    HEAP.with(|h| h.borrow_mut().fracas.insert(this, alvo));
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakProperty_getKey(this: i64) -> i64 {
    HEAP.with(|h| h.borrow().efemeros.get(&this).map_or(0, |p| p.0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakProperty_setKey(this: i64, chave: i64) {
    HEAP.with(|h| h.borrow_mut().efemeros.entry(this).or_insert((0, 0)).0 = chave);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakProperty_getValue(this: i64) -> i64 {
    HEAP.with(|h| h.borrow().efemeros.get(&this).map_or(0, |p| p.1))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakProperty_setValue(this: i64, valor: i64) {
    HEAP.with(|h| h.borrow_mut().efemeros.entry(this).or_insert((0, 0)).1 = valor);
}

/// `_Closure._computeHash` (`Closure_computeHash`): coerente com o
/// `Closure_equals` — a função e, no tear-off de um método, o receptor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Closure_computeHash(this: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Some(Value::Closure(c)) = heap.try_get(this) else { return 0 };
        let mut h = (c.code_id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        if let Some(Value::Environment(x)) = heap.try_get(c.environment)
            && x.len() == 1
            && x[0].is_ref
        {
            h ^= (x[0].bits as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
        }
        // Um `Smi` positivo de 30 bits, como o hash da VM.
        ((h >> 34) & 0x3FFF_FFFF) as i64
    })
}

/// Uma lista com os elementos da tabela `dados` (`lower/literais.rs`): os
/// literais de coleção grandes de escalares constantes chegam como dados, e
/// não como uma instrução por elemento. Cada elemento: `n` (null), `t`/`f`
/// (bool), `i` + 8 bytes (int, little-endian), `s` + 4 bytes de tamanho +
/// os bytes WTF-8 (string, canônica como a de um literal).
///
/// # Safety
/// `dados` aponta para `len` bytes legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_lista_de_tabela(dados: *const u8, len: i64) -> i64 {
    // SAFETY: garantido por quem chama (uma constante do módulo).
    let b = unsafe { std::slice::from_raw_parts(dados, usize::try_from(len).unwrap_or(0)) };
    let mut itens = Vec::new();
    let mut i = 0;
    let palavra = |b: &[u8], i: usize, n: usize| {
        let mut w = [0u8; 8];
        w[..n].copy_from_slice(&b[i..i + n]);
        u64::from_le_bytes(w)
    };
    while i < b.len() {
        let tag = b[i];
        i += 1;
        let v = match tag {
            b'n' => TaggedValue::reference(0),
            b't' => TaggedValue::boolean(true),
            b'f' => TaggedValue::boolean(false),
            b'i' => {
                let x = palavra(b, i, 8) as i64;
                i += 8;
                TaggedValue::scalar(x)
            }
            b's' => {
                let n = palavra(b, i, 4) as usize;
                i += 4;
                // Strings literais são canônicas e permanentes: nenhuma
                // coleta no meio as perde.
                let h = HEAP.with(|heap| heap.borrow_mut().string_literal(Texto::de_wtf8(&b[i..i + n])));
                i += n;
                TaggedValue::reference(h)
            }
            _ => panic!("bug do compilador: tabela de coleção com o marcador {tag}"),
        };
        itens.push(v);
    }
    HEAP.with(|heap| heap.borrow_mut().create_list(itens))
}
