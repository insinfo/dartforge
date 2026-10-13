// Runtime nativo: células, contextos e closures (espaço unificado, P2:
// docs/NATIVO-ESPACO-UNIFICADO.md §2.5 e `boxes.rs`).
//
// O código gerado cria e lê células, contextos e closures em linha
// (`llvm/caixas_ir.rs`). As funções abaixo ficam para a ABI: as de criação
// como caminho lento, e as de leitura para o runtime e para o que ainda as
// declara (`llvm/externs.rs`). A ABI de pares `(bits, tag)` (tags 1 int, 2
// bool, 3 referência, 4 double) das externs antigas vira um `Field`: a
// palavra na representação gravada e se ela é referência.

/// O `Field` de um par `(bits, tag)` da ABI antiga: só a tag 3 é referência;
/// `bool` é 0/1 e `double`, os bits.
fn campo_de_par(bits: i64, tag: u8) -> crate::heap::Field {
    match tag {
        1 | 4 => (bits, false),
        2 => (i64::from(bits != 0), false),
        3 => (bits, true),
        _ => panic!("tag de valor inválida"),
    }
}

/// O corpo tipado de `h` se ela é uma closure com a ABI `abi` (a que quem
/// chama espera), senão 0 — e quem chama segue pela entrada uniforme. Só
/// lê o heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_tipada(h: i64, abi: i64) -> i64 {
    heap_without_borrow(|heap| match heap.closure(h) {
        Some(c) if c.abi == abi && abi != 0 => c.typed_one,
        _ => 0,
    })
}

/// Devolve o tear-off canônico de uma função top-level (mesmo handle sempre).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_tearoff(code_id: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().tearoff(code_id))
}

/// [`dartforge_tearoff`] sob ARC: o tear-off canônico com um owner do código,
/// independente do registro que o mantém. Pode alocar na primeira vez; não
/// lança nem executa Dart.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let a = dartforge_arc_tearoff_owned_v1(77);
/// assert_eq!(a, dartforge_arc_tearoff_owned_v1(77));
/// dartforge_arc_release(a);
/// dartforge_arc_release(a);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_tearoff_owned_v1(code_id: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.tearoff(code_id);
        heap.retain_code_owner(h);
        h
    })
}

/// Consulta o código simbólico de uma closure para despacho indireto.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_code(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().closure(handle).expect("closure esperada").code)
}

/// Os quatro campos de ninguém (`abi` 0): o que [`dartforge_closure_cabecalho`]
/// devolve para quem não é closure.
static CLOSURE_NONE: [i64; 4] = [0; 4];

/// O endereço dos quatro campos da closure `h` (código, contexto, corpo
/// tipado, ABI — a ordem do antigo `CabecalhoDeClosure`), ou os de
/// [`CLOSURE_NONE`] para quem não é closure. O código gerado lê o bloco
/// direto (`lower/closures.rs`); fica para a ABI.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_cabecalho(h: i64) -> i64 {
    heap_without_borrow(|heap| match heap.closure(h) {
        Some(_) => h + (crate::layout::desl::CORPO as i64 - crate::layout::DESLOCAMENTO_DO_HANDLE),
        None => CLOSURE_NONE.as_ptr() as i64,
    })
}

// --- P1: convenção uniforme das closures (docs/NATIVO-PLANO.md §7.4) --------

/// Lê uma captura mutável como referência. A palavra de um escalar não diz o
/// tipo: sai como `int` (o código gerado lê a célula na representação do
/// local e nunca chama esta).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_cell_get_ref(handle: i64) -> i64 {
    let (bits, is_ref_kind) = HEAP.with(|heap| heap.borrow().cell(handle));
    if is_ref_kind { bits } else { HEAP.with(|heap| heap.borrow_mut().box_int(bits)) }
}

/// O endereço da entrada uniforme de uma closure (o código gravado nela). Um
/// valor que não é closure (null, ou outro objeto chamado como função) deixa
/// `NoSuchMethodError` pendente e devolve 0, que o ponto de chamada troca pela
/// entrada `@df_clo_invalido`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_entry(handle: i64) -> i64 {
    let code = HEAP.with(|heap| heap.borrow().closure(handle).map(|c| c.code));
    // Um objeto de classe com `call` (classe chamável): a entrada do método
    // na tabela da classe tem a mesma convenção (receptor, args, desc).
    code.or_else(|| class_method(dartforge_value_class(handle), name_hash("c:call")).map(|f| f as i64))
        .unwrap_or_else(|| {
            dartforge_nsm_chamada();
            0
        })
}

/// Lança o `NoSuchMethodError` de uma chamada de valor função que não casa
/// (valor que não é função, ou aridade/nomes errados).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nsm_chamada() {
    let name = HEAP.with(|heap| heap.borrow_mut().allocate_str("call"));
    let error = with_roots(&[name], || dartforge_no_such_method_error_new(name));
    with_roots(&[error], || dartforge_exception_throw(error, 3));
}

/// A tupla RTI do corpo de uma closure genérica: os `n_outside` argumentos de
/// tipo de quem a criou (`from_outside`, `0` sem nenhum; o que faltar é
/// `dynamic`) seguidos dos dela — os da chamada (`own`, a tupla do slot
/// depois dos argumentos) ou, quando a chamada não passou nenhum (`0`), os
/// padrões (`pattern`, a instanciação pelos limites).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_tupla_juntar(from_outside: i64, n_outside: i64, own: i64, pattern: i64) -> i64 {
    let mut v = tuple_arguments(from_outside);
    v.resize(usize::try_from(n_outside).unwrap_or(0), 0);
    v.extend(tuple_arguments(if own > 0 { own } else { pattern }));
    RTI.with(|u| u.borrow_mut().intern(Type::Tuple(v)))
}

/// Confere um descritor de chamada `[n_pos, n_nom, hash…]` contra a
/// assinatura `[n_required, n_pos, n_nom, hash…, obrigatório…]` da função
/// chamada: posicionais entre os obrigatórios e o total, todo nomeado
/// passado existe, todo nomeado `required` foi passado.
///
/// # Safety
/// Os dois ponteiros são vetores constantes emitidos pelo compilador, com o
/// tamanho que os seus próprios cabeçalhos dizem.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_args_casam(desc: *const i64, sig: *const i64) -> u8 {
    // SAFETY: cabeçalhos de vetores constantes do emissor.
    let (npos, nnom) = unsafe { (*desc, *desc.add(1)) };
    let (required_count, total, snom) = unsafe { (*sig, *sig.add(1), *sig.add(2)) };
    if npos < required_count || npos > total {
        return 0;
    }
    let nnom = usize::try_from(nnom).unwrap_or(0);
    let snom = usize::try_from(snom).unwrap_or(0);
    // SAFETY: os tamanhos vêm dos cabeçalhos.
    let passed = unsafe { std::slice::from_raw_parts(desc.add(2), nnom) };
    let name_list = unsafe { std::slice::from_raw_parts(sig.add(3), snom) };
    let required = unsafe { std::slice::from_raw_parts(sig.add(3 + snom), snom) };
    if passed.iter().any(|h| !name_list.contains(h)) {
        return 0;
    }
    for (h, req) in name_list.iter().zip(required) {
        if *req != 0 && !passed.contains(h) {
            return 0;
        }
    }
    1
}

/// A posição (entre os nomeados) do argumento de nome `hash` no descritor,
/// ou -1 se ele não foi passado.
///
/// # Safety
/// `desc` é um descritor constante emitido pelo compilador.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arg_indice(desc: *const i64, hash: i64) -> i64 {
    // SAFETY: cabeçalho e nomes do descritor constante.
    let nnom = usize::try_from(unsafe { *desc.add(1) }).unwrap_or(0);
    let passed = unsafe { std::slice::from_raw_parts(desc.add(2), nnom) };
    passed
        .iter()
        .position(|h| *h == hash)
        .map_or(-1, |p| p as i64)
}

/// O comprimento de uma lista do núcleo, ou 0 para o que não é lista.
fn list_length(h: i64) -> usize {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if heap.e_lista(h) { heap.list_len(h) } else { 0 }
    })
}

/// `Function._apply` da VM recebe `[função, positional…, named…]` e os
/// nomes em uma segunda lista, já produzidas pelo patch Dart de `Function.apply`.
/// Recompõe a ABI uniforme das closures, inclusive o descritor de nomes.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Function_apply(arguments: i64, names: i64) -> i64 {
    with_roots(&[arguments, names], || {
        let count = list_length(arguments);
        let named = list_length(names);
        if count == 0 || named >= count {
            dartforge_nsm_chamada();
            return 0;
        }
        // Os elementos em posição `Ref` (as listas são as gerais do patch; a
        // caixa, se houver, fica viva na própria lista).
        let (function, args, name_list) = HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            let function = heap.list_get_ref(arguments, 0);
            // O slot depois dos argumentos é o da tupla de tipos (nenhuma).
            let args: Vec<i64> = (1..count).map(|i| heap.list_get_ref(arguments, i)).chain(std::iter::once(0)).collect();
            let name_list: Vec<Option<String>> = (0..named)
                .map(|i| {
                    let v = heap.list_get_ref(names, i);
                    heap.text(v).map(|t| t.para_string())
                })
                .collect();
            (function, args, name_list)
        });
        let mut desc = vec![(count - 1 - named) as i64, named as i64];
        for name in name_list {
            let Some(name) = name else {
                dartforge_nsm_chamada();
                return 0;
            };
            // Mesmo FNV-1a 64 do descritor emitido em `lower/closures.rs`.
            let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
            desc.push(hash as i64);
        }
        with_roots(&args, || {
            let code = dartforge_closure_entry(function);
            if code == 0 { return 0; }
            // SAFETY: `dartforge_closure_entry` devolve o endereço de uma entrada
            // uniforme gerada com assinatura (closure, argumentos, descritor).
            // Pela porta (`excecoes_tabelas.rs`): os dois ponteiros passam como
            // palavras, a mesma passagem de argumento.
            dart_r3(code as usize, function, args.as_ptr() as i64, desc.as_ptr() as i64)
        })
    })
}

#[cfg(test)]
mod function_apply_tests {
    use super::*;

    extern "C" fn entry(_closure: i64, args: *const i64, desc: *const i64) -> i64 {
        // O patch Dart fornece 1 posicional e dois nomeados na ordem c,b.
        dartforge_gc_collect();
        let matches = unsafe {
            *desc == 1 && *desc.add(1) == 2
                && *desc.add(2) == hash("c") && *desc.add(3) == hash("b")
                && *args.add(1) == 0 && *args.add(2) == 0
        };
        let value_alive = HEAP.with(|heap| heap.borrow().text(unsafe { *args }).is_some_and(|t| t.para_string() == "valor"));
        i64::from(matches && value_alive)
    }

    fn hash(name: &str) -> i64 {
        name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        }) as i64
    }

    #[test]
    fn function_apply_forwards_named_descriptor_to_uniform_entry() {
        use crate::heap::Value;
        use crate::lists::Element;
        let function = dartforge_tearoff(entry as usize as i64);
        with_roots(&[function], || {
            let c = HEAP.with(|heap| heap.borrow_mut().allocate_str("c"));
            with_roots(&[c], || {
                let b = HEAP.with(|heap| heap.borrow_mut().allocate_str("b"));
                with_roots(&[b], || {
                    let value = HEAP.with(|heap| heap.borrow_mut().allocate_str("valor"));
                    with_roots(&[value], || {
                        let arguments = HEAP.with(|heap| {
                            let mut heap = heap.borrow_mut();
                            let l = heap.new_list(crate::layout::cid::LIST, 4, Element::General);
                            heap.list_set(l, 0, Value::Ref(function));
                            heap.list_set(l, 1, Value::Ref(value));
                            l
                        });
                        with_roots(&[arguments], || {
                            let names = HEAP.with(|heap| {
                                let mut heap = heap.borrow_mut();
                                let l = heap.new_list(crate::layout::cid::LIST, 2, Element::General);
                                heap.list_set(l, 0, Value::Ref(c));
                                heap.list_set(l, 1, Value::Ref(b));
                                l
                            });
                            assert_eq!(dartforge_nativo_Function_apply(arguments, names), 1);
                        });
                    });
                });
            });
        });
    }
}
