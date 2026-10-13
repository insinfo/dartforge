// ABI de tokens e slots proprietários usados pelo código gerado.
// A pilha observacional não substitui estes owners. Operações de ownership
// não coletam nem executam Dart; coleta tem um safepoint explícito separado.
// A fábrica owned de caixas é uma operação de alocação, com safepoint próprio.
// O lançamento Ref publica uma raiz runtime e pode alocar o rastro.

fn audited_list_map(heap: &Heap, list: i64) -> Result<(i64, usize, usize), &'static str> {
    use crate::layout::cid;
    if !heap.e_lista(list) { return Err("receiver não é lista viva"); }
    let (data, length) = if heap.header(list).class_id == cid::GROWABLE_LIST {
        let object = heap.object(list).ok_or("lista expansível sem mapa de instância")?;
        if object.len() != 2 { return Err("layout da lista expansível inválido"); }
        let (n, mark_n) = object.get(0).ok_or("comprimento ausente")?;
        let (data, mark_data) = object.get(1).ok_or("armazenamento ausente")?;
        if mark_n || !mark_data { return Err("mapa da lista expansível inválido"); }
        (data, usize::try_from(n).map_err(|_| "comprimento negativo")?)
    } else {
        let n = *heap.words(list).first().ok_or("comprimento ausente")?;
        (list, usize::try_from(n).map_err(|_| "comprimento negativo")?)
    };
    if !heap.e_lista(data) || !cid::is_fixed_list(heap.header(data).class_id) {
        return Err("armazenamento não é lista fixa viva");
    }
    let words = heap.words(data);
    let capacidade = usize::try_from(*words.first().ok_or("capacidade ausente")?)
        .map_err(|_| "capacidade negativa")?;
    if capacidade > words.len().saturating_sub(1) {
        return Err("capacidade fora do armazenamento");
    }
    Ok((data, length, capacidade))
}

fn audited_list_storage(heap: &Heap, list: i64) -> Result<(i64, usize), &'static str> {
    let (data, length, capacidade) = audited_list_map(heap, list)?;
    if length > capacidade { return Err("comprimento fora do armazenamento"); }
    Ok((data, length))
}

fn read_audited_ref_element(heap: &Heap, list: i64, ix: i64) -> Result<i64, &'static str> {
    use crate::layout::flags;
    let (data, length) = audited_list_storage(heap, list)?;
    if heap.header(data).flags & flags::FORMA != flags::REFS {
        return Err("armazenamento não contém referências");
    }
    let words = heap.words(data);
    let ix = usize::try_from(ix).map_err(|_| "índice negativo")?;
    if ix >= length { return Err("índice fora da lista"); }
    Ok(words[1 + ix])
}

/// Lê o comprimento escalar de uma lista viva sem alterar seu ownership.
///
/// Aceita listas fixas, imutáveis e expansíveis, gerais ou compactas. Confere
/// mapa e limites do armazenamento sem alocar, coletar ou executar Dart.
/// A referência recebida permanece emprestada durante a chamada.
///
/// # Panics
/// Receiver morto, classe, mapa ou comprimento inválido; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// // SAFETY: elements Ref váread_list durante a criação da list.
/// let list = unsafe { dartforge_lista_nova([0, 0].as_ptr(), 2, 8, 0) };
/// dartforge_arc_retain(list);
/// assert_eq!(dartforge_arc_lista_len_v1(list), 2);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_len_v1(list: i64) -> i64 {
    HEAP.with(|h| {
        let heap = h.borrow();
        let (_, length) = audited_list_storage(&heap, list)
            .expect("comprimento ARC de lista inválido");
        i64::try_from(length).expect("comprimento fora de i64")
    })
}

/// Empresta o armazenamento Ref, mantido vivo pelo owner da lista recebida.
///
/// Confere classe, mapa e limites; não aloca, coleta, retém ou executa Dart.
/// O empréstimo perde validade após uma operação que possa trocar arestas.
///
/// # Panics
/// Lista morta ou layout inválido; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// // SAFETY: one element Ref váread_so_far durante a criação.
/// let list = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(list);
/// assert_eq!(dartforge_arc_lista_armazenamento_ref_v1(list), list);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_armazenamento_ref_v1(list: i64) -> i64 {
    HEAP.with(|h| audited_list_storage(&h.borrow(), list)
        .expect("armazenamento ARC inválido").0)
}

/// Lê a forma escalar do armazenamento de uma lista viva.
///
/// Não aloca, coleta, retém ou executa Dart; a referência fica emprestada.
///
/// # Panics
/// Lista morta ou layout inválido; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(list);
/// assert_eq!(dartforge_arc_lista_forma_v1(list), 4);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_forma_v1(list: i64) -> i64 {
    HEAP.with(|h| {
        let heap = h.borrow();
        let (data, _) = audited_list_storage(&heap, list).expect("forma ARC inválida");
        i64::from(heap.header(data).flags & (crate::layout::flags::FORMA | crate::layout::flags::ELEMENTO))
    })
}

/// Lê a capacidade escalar do armazenamento de uma lista viva.
///
/// Não aloca, coleta, retém ou executa Dart; a referência fica emprestada.
///
/// # Panics
/// Lista morta ou layout inválido; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_lista_nova([0, 0].as_ptr(), 2, 8, 0) };
/// dartforge_arc_retain(list);
/// assert_eq!(dartforge_arc_lista_capacidade_v1(list), 2);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_capacidade_v1(list: i64) -> i64 {
    HEAP.with(|h| {
        let heap = h.borrow();
        let (data, _) = audited_list_storage(&heap, list).expect("capacidade ARC inválida");
        heap.list_len(data) as i64
    })
}

fn check_arc_list_len(heap: &Heap, list: i64, new: i64) -> Result<usize, &'static str> {
    let (_, _, capacidade) = audited_list_map(heap, list)?;
    if heap.header(list).class_id != crate::layout::cid::GROWABLE_LIST {
        return Err("comprimento mutável exige lista expansível");
    }
    let new = usize::try_from(new).map_err(|_| "comprimento negativo")?;
    if new > capacidade { return Err("comprimento fora do armazenamento"); }
    Ok(new)
}

/// Grava o comprimento escalar de uma lista expansível dentro da capacidade.
///
/// Preserva a sequência `_setData` seguida de `_setLength` do SDK, mesmo
/// quando o comprimento antigo excede temporariamente a capacidade nova.
/// Não aloca, coleta ou executa Dart; invalida empréstimos anteriores.
///
/// # Panics
/// Classe, mapa ou novo comprimento inválido; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 10, 0) };
/// dartforge_arc_retain(list);
/// dartforge_arc_lista_gravar_len_v1(list, 0);
/// assert_eq!(dartforge_arc_lista_len_v1(list), 0);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_gravar_len_v1(list: i64, new: i64) {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let new = check_arc_list_len(&heap, list, new).expect("gravação ARC de comprimento inválida");
        heap.list_set_len(list, new);
    })
}

fn check_arc_list_bits(heap: &Heap, data: i64, ix: i64, bits: i64, shape: i64)
    -> Result<usize, &'static str> {
    use crate::layout::{cid, flags};
    let (storage, length) = audited_list_storage(heap, data)?;
    if storage != data || heap.header(data).class_id != cid::LIST {
        return Err("gravação exige armazenamento fixo mutável");
    }
    let f = heap.header(data).flags & (flags::FORMA | flags::ELEMENTO);
    if shape != i64::from(f) || ![flags::BRUTO | flags::ELEMENTO_INT,
        flags::BRUTO | flags::ELEMENTO_DOUBLE, flags::BRUTO | flags::ELEMENTO_BOOL].contains(&f) {
        return Err("forma escalar incompatível");
    }
    if f == flags::BRUTO | flags::ELEMENTO_BOOL && ![0, 1].contains(&bits) {
        return Err("booleano não canônico");
    }
    let i = usize::try_from(ix).map_err(|_| "índice negativo")?;
    if i >= length { return Err("índice fora do armazenamento"); }
    Ok(i)
}

/// Grava bits de int, double ou bool num armazenamento compacto mutável.
///
/// Confere forma e índice; `shape` usa os bits FORMA/ELEMENTO do layout.
/// Não encaixota, aloca, coleta ou executa Dart; invalida empréstimos anteriores.
///
/// # Panics
/// Lista, forma, índice ou booleano inválido; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 8, 1) };
/// dartforge_arc_retain(list);
/// dartforge_arc_lista_gravar_bits_v1(list, 0, 42, 0x22);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_gravar_bits_v1(data: i64, ix: i64, bits: i64, shape: i64) {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let i = check_arc_list_bits(&heap, data, ix, bits, shape).expect("gravação ARC escalar inválida");
        heap.words_mut(data)[1 + i] = bits;
    })
}

/// Copia uma referência emprestada para um elemento geral, com barreira ARC/GC.
///
/// Confere armazenamento mutável, forma Ref e índice. Não aloca, coleta ou
/// executa Dart; não consome o owner do valor e invalida empréstimos anteriores.
///
/// # Panics
/// Lista, índice ou referência morta inválida; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(list);
/// dartforge_arc_lista_gravar_ref_v1(list, 0, 0);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_gravar_ref_v1(data: i64, ix: i64, value: i64) {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        assert!(heap.e_lista(data) && heap.header(data).class_id == crate::layout::cid::LIST,
            "gravação ARC Ref exige armazenamento mutável");
        read_audited_ref_element(&heap, data, ix).expect("gravação ARC Ref inválida");
        assert!(!crate::layout::is_object(value) || heap.is_live_object(value), "referência ARC morta");
        heap.write_refs(data, 1 + ix as usize, &[value]);
    })
}

/// Troca o armazenamento de uma expansível se a forma do novo for a mesma.
///
/// Retorna 1 após copiar a aresta Ref com barreira, ou 0 sem alterar a lista
/// quando as formas diferem. Não aloca, coleta ou executa Dart. A troca pode
/// anteceder `_setLength` ao encolher e invalida empréstimos anteriores.
///
/// # Panics
/// Receiver, mapa ou armazenamento inválido; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 10, 0) };
/// dartforge_arc_retain(list);
/// let data = dartforge_arc_lista_armazenamento_ref_v1(list);
/// assert_eq!(dartforge_arc_lista_gravar_dados_v1(list, data), 1);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_gravar_dados_v1(list: i64, data: i64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let (old_gen, _, _) = audited_list_map(&heap, list).expect("mapa ARC de lista inválido");
        assert_eq!(heap.header(list).class_id, crate::layout::cid::GROWABLE_LIST);
        let (fixed, _) = audited_list_storage(&heap, data).expect("novo armazenamento ARC inválido");
        assert_eq!(fixed, data, "armazenamento ARC deve ser fixo");
        let mask = crate::layout::flags::FORMA | crate::layout::flags::ELEMENTO;
        if heap.header(old_gen).flags & mask != heap.header(data).flags & mask { return 0; }
        heap.list_set_data(list, data);
        1
    })
}

/// Lê os bits de um elemento int, double ou bool de uma lista compacta viva.
///
/// Confere lista fixa/imutável/expansível, mapa, forma compacta e limites
/// lógicos; bool sai 0 ou 1. Não aloca, coleta, retém ou executa Dart; a
/// referência fica emprestada. As guardas Dart de índice já devem ter sido
/// executadas.
///
/// # Panics
/// Lista/layout/forma/índice inválido; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// // SAFETY: elements escalares durante a criação da list compacta.
/// let list = unsafe { dartforge_lista_nova([7].as_ptr(), 1, 8, 1) };
/// dartforge_arc_retain(list);
/// dartforge_arc_lista_gravar_bits_v1(list, 0, 42, 0x22);
/// assert_eq!(dartforge_arc_lista_ler_bits_v1(list, 0), 42);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_ler_bits_v1(list: i64, ix: i64) -> i64 {
    HEAP.with(|h| {
        let heap = h.borrow();
        read_arc_list_bits(&heap, list, ix).expect("leitura ARC escalar inválida")
    })
}

fn read_arc_list_bits(heap: &Heap, list: i64, ix: i64) -> Result<i64, &'static str> {
    use crate::layout::flags;
    let (data, length) = audited_list_storage(heap, list)?;
    let f = heap.header(data).flags & (flags::FORMA | flags::ELEMENTO);
    if ![flags::BRUTO | flags::ELEMENTO_INT, flags::BRUTO | flags::ELEMENTO_DOUBLE,
        flags::BRUTO | flags::ELEMENTO_BOOL].contains(&f) {
        return Err("armazenamento não é compacto");
    }
    let i = usize::try_from(ix).map_err(|_| "índice negativo")?;
    if i >= length { return Err("índice fora da lista"); }
    let bits = heap.words(data)[1 + i];
    if f == flags::BRUTO | flags::ELEMENTO_BOOL && ![0, 1].contains(&bits) {
        return Err("booleano não canônico");
    }
    Ok(bits)
}

/// Lê um elemento da lista geral e devolve um owner independente.
///
/// Confere lista fixa/imutável/expansível viva, mapa, representação Ref e
/// limites físicos e lógicos. Retém sem alocar, coletar ou executar Dart.
/// O chamador deve consumir o token; as guardas Dart de índice já devem ter
/// sido executadas. Não converte elementos compactos em referências.
///
/// # Panics
/// Lista/layout/índice inválido ou falha de retenção; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// // SAFETY: one element Ref alive e alinhado durante a cópia.
/// let list = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(list);
/// assert_eq!(dartforge_arc_lista_ref_owned_v1(list, 0), 0);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_ref_owned_v1(list: i64, ix: i64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let value = read_audited_ref_element(&heap, list, ix).expect("leitura ARC de lista inválida");
        heap.retain_code_owner(value);
        value
    })
}

/// Lê um elemento da lista geral emprestado da lista, sem token do código.
///
/// As mesmas conferências de [`dartforge_arc_lista_ref_owned_v1`]. É a
/// leitura do ARC sem preparação (o regime legado), em que ninguém consome
/// tokens: o elemento fica vivo pela lista e pelas raízes do quadro.
///
/// # Panics
/// Lista/layout/índice inválido; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// // SAFETY: one element Ref alive e alinhado durante a cópia.
/// let list = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(list);
/// assert_eq!(dartforge_arc_lista_ref_v1(list, 0), 0);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_ref_v1(list: i64, ix: i64) -> i64 {
    HEAP.with(|h| read_audited_ref_element(&h.borrow(), list, ix).expect("leitura ARC de lista inválida"))
}

#[cfg(test)]
mod tests_owned_ref_list {
    use super::*;
    use crate::{layout::{cid, smi}, lists::Element, heap::Value};

    #[test]
    fn append_borrows_receiver_and_preserves_roots_across_growth_and_decompaction() {
        for arc in [false, true] {
            for full_list in [false, true] {
                for shape in [Element::General, Element::Int, Element::Double, Element::Bool] {
                    for compatible in [false, true] {
                        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
                        let (list, value) = HEAP.with(|h| {
                            let mut heap = h.borrow_mut();
                            if arc { heap.enable_arc(); }
                            let list = heap.new_growable(usize::from(full_list), if full_list { 1 } else { 2 }, shape);
                            heap.retain_code_owner(list);
                            if full_list {
                                let data = heap.list_data(list);
                                heap.words_mut(data)[1] = match shape {
                                    Element::General => 0, Element::Int => i64::MIN,
                                    Element::Double => (-0.0_f64).to_bits() as i64, Element::Bool => 1,
                                };
                            }
                            let v = if !compatible || shape == Element::General {
                                Value::Ref(heap.allocate_instance(123, 1))
                            } else { match shape {
                                Element::Int => Value::Int(i64::MAX),
                                Element::Double => Value::Double(f64::from_bits(0xfff8_1234_5678_9abc)),
                                Element::Bool => Value::Bool(true), Element::General => unreachable!(),
                            }};
                            let value = heap.to_ref(v);
                            heap.retain_code_owner(value);
                            (list, value)
                        });
                        let stats = HEAP.with(|h| h.borrow().stats());
                        let r = dartforge_lista_acrescentar(list, value);
                        assert_eq!(r, list);
                        HEAP.with(|h| {
                            let heap = h.borrow();
                            assert_eq!(heap.list_len(list), 1 + usize::from(full_list));
                            assert_eq!(heap.list_capacity(list), if full_list { 3 } else { 2 });
                            let general = !compatible || shape == Element::General;
                            assert_eq!(heap.list_shape(list), if general { Element::General } else { shape });
                            let last_one = heap.list_get(list, usize::from(full_list));
                            match shape {
                                _ if general => assert_eq!(last_one, Value::Ref(value)),
                                Element::Int => assert_eq!(last_one, Value::Int(i64::MAX)),
                                Element::Double => {
                                    let Value::Double(d) = last_one else { panic!("elemento double esperado") };
                                    assert_eq!(d.to_bits(), 0xfff8_1234_5678_9abc);
                                },
                                Element::Bool => assert_eq!(last_one, Value::Bool(true)),
                                Element::General => unreachable!(),
                            }
                            if full_list || (!compatible && shape != Element::General) {
                                assert!(heap.stats().allocations > stats.allocations);
                                assert!(heap.stats().collections > stats.collections);
                            } else {
                                assert_eq!(heap.stats().allocations, stats.allocations);
                                assert_eq!(heap.stats().collections, stats.collections);
                            }
                        });
                        dartforge_arc_release(value);
                        dartforge_arc_collect();
                        if !compatible || shape == Element::General {
                            assert!(HEAP.with(|h| h.borrow().is_live_object(value)));
                        }
                        // O retorno borrowed não pode deixar um owner adicional da lista.
                        dartforge_arc_release(list);
                        dartforge_arc_collect();
                        assert!(!HEAP.with(|h| h.borrow().is_live_object(r)));
                        if !compatible || shape == Element::General {
                            assert!(!HEAP.with(|h| h.borrow().is_live_object(value)));
                        }
                        HEAP.with(|h| h.replace(previous));
                    }
                }
            }
        }
    }

    #[test]
    fn compact_storage_preserves_bits_and_shrink_sequence() {
        for arc in [false, true] {
            for (shape, bits) in [(Element::Int, i64::MIN),
                (Element::Double, 0xfff8_1234_5678_9abc_u64 as i64), (Element::Bool, 1)] {
                let previous = HEAP.with(|h| h.replace(Heap::new(true)));
                let list = HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    if arc { heap.enable_arc(); }
                    let list = heap.new_growable(0, 3, shape);
                    heap.retain_code_owner(list);
                    list
                });
                let stats = HEAP.with(|h| h.borrow().stats());
                let data = dartforge_arc_lista_armazenamento_ref_v1(list);
                assert_ne!(data, list);
                assert_eq!(dartforge_arc_lista_capacidade_v1(list), 3);
                assert_eq!(dartforge_arc_lista_len_v1(data), 3);
                assert_eq!(dartforge_arc_lista_forma_v1(data), i64::from(shape.flags()));
                dartforge_arc_lista_gravar_bits_v1(data, 0, bits, i64::from(shape.flags()));
                dartforge_arc_lista_gravar_len_v1(list, 1);
                HEAP.with(|h| {
                    let heap = h.borrow();
                    assert_eq!(heap.words(data)[1], bits);
                    assert_eq!(heap.stats().allocations, stats.allocations);
                    assert_eq!(heap.stats().collections, stats.collections);
                });
                // Cópia explícita antes de retirar a aresta que sustenta o borrow.
                dartforge_arc_retain(data);
                let new = HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    let new = heap.new_list(cid::LIST, 0, shape);
                    heap.retain_code_owner(new);
                    new
                });
                assert_eq!(dartforge_arc_lista_gravar_dados_v1(list, new), 1);
                assert!(HEAP.with(|h| audited_list_storage(&h.borrow(), list).is_err()));
                dartforge_arc_lista_gravar_len_v1(list, 0);
                assert_eq!(dartforge_arc_lista_len_v1(list), 0);
                dartforge_arc_release(new);
                dartforge_arc_collect();
                assert!(HEAP.with(|h| h.borrow().is_live_object(data)));
                dartforge_arc_release(data);
                dartforge_arc_collect();
                assert!(!HEAP.with(|h| h.borrow().is_live_object(data)));
                assert!(HEAP.with(|h| h.borrow().is_live_object(new)));
                dartforge_arc_release(list);
                dartforge_arc_collect();
                assert!(!HEAP.with(|h| h.borrow().is_live_object(new)));
                HEAP.with(|h| h.replace(previous));
            }
        }
    }

    #[test]
    fn ref_storage_copies_edges_and_preserves_element_owner() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            let list = HEAP.with(|h| {
                let mut heap = h.borrow_mut();
                if arc { heap.enable_arc(); }
                let list = heap.new_growable(1, 3, Element::General);
                heap.retain_code_owner(list);
                list
            });
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            let data = dartforge_arc_lista_armazenamento_ref_v1(list);
            let stats = HEAP.with(|h| h.borrow().stats());
            dartforge_arc_lista_gravar_ref_v1(data, 0, child);
            assert_eq!(HEAP.with(|h| h.borrow().stats().allocations), stats.allocations);
            assert_eq!(HEAP.with(|h| h.borrow().stats().collections), stats.collections);
            dartforge_arc_release(child);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(child)));
            let received = dartforge_arc_lista_ref_owned_v1(data, 0);
            dartforge_arc_lista_gravar_ref_v1(data, 0, 0);
            dartforge_arc_release(list);
            dartforge_arc_collect();
            assert_eq!(dartforge_unbox_int(received), i64::MAX);
            dartforge_arc_release(received);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(child)));
            HEAP.with(|h| h.replace(previous));
        }
    }

    #[test]
    fn writes_reject_invalid_shape_index_and_len_and_keep_shape() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let (list, new) = HEAP.with(|h| {
            let mut heap = h.borrow_mut();
            heap.enable_arc();
            let list = heap.new_growable(1, 2, Element::Bool);
            heap.retain_code_owner(list);
            let new = heap.new_list(cid::LIST, 1, Element::Int);
            heap.retain_code_owner(new);
            (list, new)
        });
        let data = dartforge_arc_lista_armazenamento_ref_v1(list);
        assert_eq!(dartforge_arc_lista_gravar_dados_v1(list, new), 0);
        assert_eq!(dartforge_arc_lista_armazenamento_ref_v1(list), data);
        HEAP.with(|h| {
            let heap = h.borrow();
            for (i, bits, shape) in [(-1, 1, 0x62), (2, 1, 0x62), (0, 2, 0x62),
                (0, 1, 0x22), (0, 1, 0x04)] {
                assert!(check_arc_list_bits(&heap, data, i, bits, shape).is_err());
            }
            assert!(check_arc_list_bits(&heap, list, 0, 1, 0x62).is_err());
            assert!(check_arc_list_len(&heap, data, 0).is_err());
            for n in [-1, 3] { assert!(check_arc_list_len(&heap, list, n).is_err()); }
            assert_eq!(heap.words(data)[1], 0);
        });
        dartforge_arc_release(new);
        dartforge_arc_release(list);
        dartforge_arc_collect();
        HEAP.with(|h| h.replace(previous));
    }

    #[test]
    fn general_and_compact_length_does_not_allocate_collect_or_retain() {
        for arc in [false, true] {
            for class in [cid::LIST, cid::IMMUTABLE_LIST, cid::GROWABLE_LIST] {
                for shape in [Element::General, Element::Int, Element::Double, Element::Bool] {
                    let previous = HEAP.with(|h| h.replace(Heap::new(true)));
                    let list = HEAP.with(|h| {
                        let mut heap = h.borrow_mut();
                        if arc { heap.enable_arc(); }
                        let list = if class == cid::GROWABLE_LIST {
                            heap.new_growable(2, 5, shape)
                        } else { heap.new_list(class, 2, shape) };
                        heap.retain_code_owner(list);
                        list
                    });
                    let stats = HEAP.with(|h| h.borrow().stats().clone());
                    assert_eq!(dartforge_arc_lista_len_v1(list), 2);
                    HEAP.with(|h| {
                        let heap = h.borrow();
                        assert_eq!(heap.stats().allocations, stats.allocations);
                        assert_eq!(heap.stats().collections, stats.collections);
                    });
                    dartforge_arc_release(list);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observar_heap_v1(list), 0);
                    assert!(HEAP.with(|h| audited_list_storage(&h.borrow(), list).is_err()));
                    HEAP.with(|h| h.replace(previous));
                }
            }
        }
    }

    #[test]
    fn length_rejects_invalid_map_class_and_bounds() {
        let mut heap = Heap::new(true);
        let other = heap.allocate_instance(123, 2);
        heap.retain_code_owner(other);
        for receiver in [0, smi::de(3).unwrap(), i64::MAX, other] {
            assert!(audited_list_storage(&heap, receiver).is_err());
        }
        let fixed = heap.new_list(cid::LIST, 2, Element::Int);
        heap.retain_code_owner(fixed);
        for n in [-1, 3, i64::MAX] {
            heap.words_mut(fixed)[0] = n;
            assert!(audited_list_storage(&heap, fixed).is_err());
        }
        heap.words_mut(fixed)[0] = 2;
        let list = heap.new_growable(0, 3, Element::Double);
        heap.retain_code_owner(list);
        assert_eq!(audited_list_storage(&heap, list).unwrap().1, 0);
        for n in [-1, 4] {
            heap.set_field(list, 0, n, false);
            assert!(audited_list_storage(&heap, list).is_err());
        }
        heap.set_field(list, 0, 0, true);
        assert!(audited_list_storage(&heap, list).is_err());
        heap.set_field(list, 0, 0, false);
        for (data, mark) in [(fixed, false), (other, true), (list, true), (0, true)] {
            heap.set_field(list, 1, data, mark);
            assert!(audited_list_storage(&heap, list).is_err());
        }
        let fake = heap.allocate_instance(cid::GROWABLE_LIST, 1);
        assert!(audited_list_storage(&heap, fake).is_err());
    }

    #[test]
    fn owned_element_survives_cleared_slot_and_released_list() {
        for arc in [false, true] {
            for class in [cid::LIST, cid::IMMUTABLE_LIST, cid::GROWABLE_LIST] {
                let previous = HEAP.with(|h| h.replace(Heap::new(true)));
                if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
                let child = dartforge_arc_box_int_owned_v1(i64::MAX);
                let list = HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    let list = if class == cid::GROWABLE_LIST { heap.new_growable(3, 5, Element::General) }
                        else { heap.new_list(class, 3, Element::General) };
                    heap.retain_code_owner(list);
                    heap.list_set(list, 1, Value::Ref(child));
                    heap.list_set(list, 2, Value::Ref(smi::de(7).unwrap()));
                    list
                });
                dartforge_arc_release(child);
                let stats = HEAP.with(|h| h.borrow().stats());
                let received = dartforge_arc_lista_ref_owned_v1(list, 1);
                assert_eq!(received, child);
                assert_eq!(dartforge_arc_lista_ref_owned_v1(list, 0), 0);
                assert_eq!(dartforge_arc_lista_ref_owned_v1(list, 2), smi::de(7).unwrap());
                HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    assert_eq!(heap.stats().allocations, stats.allocations);
                    assert_eq!(heap.stats().collections, stats.collections);
                    heap.list_set(list, 1, Value::Ref(0));
                });
                dartforge_arc_release(list);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(list), 0);
                assert_eq!(dartforge_arc_observar_heap_v1(received), 3);
                assert_eq!(dartforge_unbox_int(received), i64::MAX);
                dartforge_arc_release(received);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(received), 0);
                HEAP.with(|h| h.replace(previous));
            }
        }
    }

    #[test]
    fn read_rejects_fake_compacts_indices_and_layouts() {
        let mut heap = Heap::new(true);
        for shape in [Element::General, Element::Int, Element::Double, Element::Bool] {
            let list = heap.new_list(cid::LIST, 2, shape);
            for ix in [-1, 2, i64::MAX] { assert!(read_audited_ref_element(&heap, list, ix).is_err()); }
            assert_eq!(read_audited_ref_element(&heap, list, 0).is_ok(), shape == Element::General);
        }
        for receiver in [0, smi::de(3).unwrap(), i64::MAX] {
            assert!(read_audited_ref_element(&heap, receiver, 0).is_err());
        }
        let fake = heap.allocate_instance(cid::GROWABLE_LIST, 2);
        heap.set_field(fake, 0, 1, false);
        heap.set_field(fake, 1, 0, false);
        assert!(read_audited_ref_element(&heap, fake, 0).is_err());
        let list = heap.new_growable(1, 3, Element::General);
        assert!(read_audited_ref_element(&heap, list, 1).is_err());
        heap.set_field(list, 0, 4, false);
        assert!(read_audited_ref_element(&heap, list, 0).is_err());
    }
}

fn arc_argument_index(positional: i64, named: i64, ix: i64) -> Result<usize, &'static str> {
    let total = positional.checked_add(named).filter(|_| positional >= 0 && named >= 0)
        .and_then(|n| usize::try_from(n).ok()).filter(|n| *n <= isize::MAX as usize / std::mem::size_of::<i64>())
        .ok_or("contagens inválidas do descritor")?;
    usize::try_from(ix).ok().filter(|i| *i < total).ok_or("índice fora dos argumentos Ref")
}

/// Lê um argumento Ref da ABI uniforme e entrega um owner independente.
///
/// Confere o índice contra a soma dos posicionais e nomeados: o slot RTI
/// posterior não é uma Ref. Retém antes de devolver, sem alocar, coletar,
/// executar Dart ou alterar os vetores. O chamador deve consumir o token.
///
/// # Safety
/// `descriptor` deve apontar para ao menos dois i64 alinhados e imutáveis.
/// `arguments` deve conter ao menos a quantidade declarada de Ref válidos,
/// alinhados e sustentados pelo chamador, sem mutação durante esta chamada.
/// A conferência do índice não prova a extensão real de ponteiros externos.
///
/// # Panics
/// Ponteiro nulo, contagem/índice inválido ou falha interna de retenção;
/// aborta na fronteira C, sem exceção Dart.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_objeto_owned_v1(123, 0);
/// let arguments = [object];
/// let descriptor = [1, 0];
/// // SAFETY: vetores live_set e imutáveis; object sustentado pelo caller.
/// let received = unsafe { dartforge_arc_argumento_owned_v1(arguments.as_ptr(), descriptor.as_ptr(), 0) };
/// dartforge_arc_release(object);
/// assert_eq!(dartforge_arc_observar_heap_v1(received), 3);
/// dartforge_arc_release(received);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_argumento_owned_v1(arguments: *const i64, descriptor: *const i64, ix: i64) -> i64 {
    assert!(!descriptor.is_null() && !arguments.is_null(), "vetores ABI ARC nulos");
    // SAFETY: dois elementos válidos conforme o contrato da ABI.
    let ix = arc_argument_index(unsafe { *descriptor }, unsafe { *descriptor.add(1) }, ix)
        .expect("argumento ARC fora do descritor");
    // SAFETY: índice conferido e extensão garantida pelo chamador.
    let value = unsafe { *arguments.add(ix) };
    HEAP.with(|heap| heap.borrow_mut().retain_code_owner(value));
    value
}

#[cfg(test)]
mod tests_owned_argument {
    use super::*;

    #[test]
    #[allow(unsafe_code)]
    fn ref_arguments_check_bounds_and_survive_caller_owner() {
        for (pos, nom, i) in [(-1, 0, 0), (0, -1, 0), (1, 0, -1), (1, 0, 1), (0, 0, 0), (i64::MAX, 1, 0)] {
            assert!(arc_argument_index(pos, nom, i).is_err());
        }
        assert_eq!(arc_argument_index(1, 2, 2), Ok(2));
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(crate::heap::Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let object = dartforge_arc_objeto_owned_v1(32001, 0);
            let arguments = [0, object, crate::layout::smi::de(7).unwrap(), i64::MAX];
            let descriptor = [1, 2, 101, 102];
            let stats = HEAP.with(|h| h.borrow().stats());
            // SAFETY: três argumentos válidos seguidos de RTI que não será lido.
            let received = unsafe { dartforge_arc_argumento_owned_v1(arguments.as_ptr(), descriptor.as_ptr(), 1) };
            assert_eq!(received, object);
            dartforge_arc_release(object);
            assert_eq!(dartforge_arc_observar_heap_v1(received), 3);
            HEAP.with(|h| { assert_eq!(h.borrow().stats().allocations, stats.allocations);
                assert_eq!(h.borrow().stats().collections, stats.collections); });
            dartforge_arc_release(received);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(received), 0);
            HEAP.with(|h| h.replace(previous));
        }
    }
}

/// Aloca uma instância zerada e entrega exatamente um token Owned ao chamador.
///
/// Classe deve caber em i32 e quantidade de campos em u16, como o cabeçalho
/// físico. Todos os campos começam com bits zero e sem marca de referência,
/// inclusive no mapa estendido. A alocação pode coletar; a retenção ocorre
/// no mesmo empréstimo do heap, antes de devolver o handle ou reabastecer TLAB.
/// Não executa construtor Dart, registra métodos ou certifica layout/recarga.
/// O chamador deve transferir o token ou liberá-lo exatamente uma vez.
///
/// # Panics
/// Classe/quantidade inválida ou falha interna de alocação/contagem; aborta
/// no limite C. Não representa uma exceção Dart.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_objeto_owned_v1(123, 40);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(object), 3);
/// assert_eq!(dartforge_arc_ler_campo_escalar_v1(object, 39), 0);
/// dartforge_arc_release(object);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(object), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_objeto_owned_v1(class: i64, fields: i64) -> i64 {
    let (class, fields) = owned_instance_parameters(class, fields)
        .expect("contrato de alocação de instância ARC violado");
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let object = heap.allocate_instance(class, fields);
        heap.retain_code_owner(object);
        heap.refill_tlab(crate::layout::palavras_de_instancia(fields));
        object
    })
}

/// Cria uma closure com owner independente e contexto emprestado.
///
/// Código e corpo tipado são endereços nativos da imagem; sua ABI e vida
/// continuam sob responsabilidade do emissor. Não chama esses endereços.
/// O contexto ganha a aresta da closure sem consumir o token do chamador.
/// Inicialização e retenção ocorrem no mesmo empréstimo do heap. Pode
/// coletar, mas não executa Dart nem deixa exceção Dart pendente.
///
/// # Panics
/// Falha interna de alocação/contagem; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn input(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
/// let closure = dartforge_arc_closure_owned_v1(input as usize as i64, 0, 0, 0);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(closure), 3);
/// dartforge_arc_release(closure);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_closure_owned_v1(code: i64, contexto: i64, typed_one: i64, abi: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let closure = heap.new_closure(code, (contexto, true), typed_one, abi);
        heap.retain_code_owner(closure);
        closure
    })
}

/// Empresta o contexto Ref de uma closure sem reter ou coletar.
///
/// Confere a classe e o mapa físico. Contexto escalar só é aceito quando
/// vale zero, usado pelas closures canônicas sem capturas. O empréstimo
/// depende da closure e de sua aresta; deve ser copiado antes de invalidá-la.
/// Não executa Dart nem deixa exceção pendente.
///
/// # Panics
/// Receiver não closure ou contexto escalar não nulo; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn input(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
/// let closure = dartforge_arc_closure_owned_v1(input as usize as i64, 0, 0, 0);
/// assert_eq!(dartforge_arc_contexto_closure_v1(closure), 0);
/// dartforge_arc_release(closure);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_contexto_closure_v1(closure: i64) -> i64 {
    HEAP.with(|heap| audited_closure_context(&heap.borrow(), closure)
        .expect("contrato de contexto da closure ARC violado"))
}

/// Empresta uma captura Ref de um ambiente, conferindo seu mapa físico.
///
/// Exige ambiente vivo da classe CONTEXTO, índice válido e campo marcado Ref.
/// O resultado depende da aresta do ambiente; deve ser copiado antes de
/// substituir a captura ou liberar o ambiente. Não retém, aloca, coleta,
/// executa Dart ou altera a exceção pendente.
///
/// # Panics
/// Classe, índice ou representação inválida; aborta no limite C.
///
/// ```
/// use dartforge_runtime::{abi::*, layout::cid};
/// let env = dartforge_arc_objeto_owned_v1(i64::from(cid::CONTEXTO), 1);
/// dartforge_arc_gravar_campo_ref_v1(env, 0, 0);
/// assert_eq!(dartforge_arc_env_ref_v1(env, 0), 0);
/// dartforge_arc_release(env);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_env_ref_v1(env: i64, ix: i64) -> i64 {
    HEAP.with(|heap| audited_env_ref(&heap.borrow(), env, ix)
        .expect("contrato de captura Ref ARC violado"))
}

/// Copia os bits de uma captura escalar de um ambiente, conferindo seu mapa.
///
/// Exige ambiente vivo da classe CONTEXTO, índice válido e campo não marcado
/// Ref. Os bits seguem a representação com que a captura foi gravada; a
/// conversão (`double`, `bool`) fica com o chamador. Não retém, aloca,
/// coleta, executa Dart ou altera a exceção pendente.
///
/// # Panics
/// Classe, índice ou representação inválida; aborta no limite C.
///
/// ```
/// use dartforge_runtime::{abi::*, layout::cid};
/// let env = dartforge_arc_objeto_owned_v1(i64::from(cid::CONTEXTO), 1);
/// dartforge_arc_gravar_campo_escalar_v1(env, 0, 42);
/// assert_eq!(dartforge_arc_env_escalar_v1(env, 0), 42);
/// dartforge_arc_release(env);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_env_escalar_v1(env: i64, ix: i64) -> i64 {
    HEAP.with(|heap| audited_env_scalar(&heap.borrow(), env, ix)
        .expect("contrato de captura escalar ARC violado"))
}

fn audited_env_scalar(heap: &Heap, env: i64, ix: i64) -> Result<i64, &'static str> {
    let object = heap.object(env).ok_or("receiver não é ambiente vivo")?;
    if object.class_id != i64::from(crate::layout::cid::CONTEXTO) {
        return Err("receiver não é ambiente");
    }
    let ix = usize::try_from(ix).map_err(|_| "índice negativo")?;
    let (bits, reference) = object.get(ix).ok_or("índice fora do ambiente")?;
    if reference { return Err("captura é referência gerenciada"); }
    Ok(bits)
}

fn audited_env_ref(heap: &Heap, env: i64, ix: i64) -> Result<i64, &'static str> {
    let object = heap.object(env).ok_or("receiver não é ambiente vivo")?;
    if object.class_id != i64::from(crate::layout::cid::CONTEXTO) {
        return Err("receiver não é ambiente");
    }
    let ix = usize::try_from(ix).map_err(|_| "índice negativo")?;
    let (bits, reference) = object.get(ix).ok_or("índice fora do ambiente")?;
    if !reference { return Err("captura é escalar"); }
    Ok(bits)
}

fn audited_closure_context(heap: &Heap, closure: i64) -> Result<i64, &'static str> {
    let contexto = heap.closure(closure).ok_or("receiver não é closure")?.contexto;
    if !contexto.1 && contexto.0 != 0 { return Err("contexto é escalar não nulo"); }
    Ok(contexto.0)
}

#[cfg(test)]
mod tests_closure_context {
    use super::*;

    #[test]
    fn env_checks_class_bounds_and_extended_map_without_collecting() {
        let mut heap = Heap::new(false);
        let child = heap.new_object(32000, &[]);
        let mut fields = vec![(0, false); 40];
        fields[0] = (0, true);
        fields[1] = (smi::de(42).unwrap(), true);
        fields[38] = (child, false);
        fields[39] = (child, true);
        let env = heap.new_object(i64::from(crate::layout::cid::CONTEXTO), &fields);
        let false_value = heap.new_object(i64::from(crate::layout::cid::CELULA), &fields);
        let stats = heap.stats();
        for (ix, expected) in [(0, 0), (1, fields[1].0), (39, child)] {
            assert_eq!(audited_env_ref(&heap, env, ix), Ok(expected));
        }
        for (receiver, ix) in [(env, -1), (env, 40), (env, 38), (env, 37),
            (0, 0), (false_value, 39), (child, 0)] {
            assert!(audited_env_ref(&heap, receiver, ix).is_err());
        }
        assert_eq!(heap.stats().allocations, stats.allocations);
        assert_eq!(heap.stats().collections, stats.collections);
    }

    #[test]
    fn context_checks_class_and_map_without_mistaking_bits_for_ref() {
        let mut heap = Heap::new(false);
        for field in [(0, false), (0, true), (42, false)] {
            let closure = heap.new_closure(123, field, 0, 0);
            assert_eq!(audited_closure_context(&heap, closure).is_ok(), field.0 == 0);
        }
        let child = heap.new_object(32000, &[]);
        let closure = heap.new_closure(123, (child, true), 0, 0);
        assert_eq!(audited_closure_context(&heap, closure), Ok(child));
        let fake = heap.new_object(32001, &[(123, false), (child, true), (0, false), (0, false)]);
        for invalid in [0, fake] { assert!(audited_closure_context(&heap, invalid).is_err()); }
    }
}

fn owned_instance_parameters(class: i64, fields: i64) -> Result<(i32, usize), &'static str> {
    let class = i32::try_from(class).map_err(|_| "classe fora de i32")?;
    let fields = u16::try_from(fields).map_err(|_| "quantidade de campos fora de u16")?;
    Ok((class, usize::from(fields)))
}

/// Registra a tabela estática de métodos e aloca uma instância zerada Owned.
///
/// A função `tabela` é o thunk estático do emissor: devolve uma tabela válida
/// `{cid, n, [hash, input]…}`, sem executar Dart, coletar ou lançar. A memória
/// e as entradas precisam permanecer válidas segundo o protocolo de gerações.
/// O registro ocorre antes da alocação; o token tem o mesmo contrato de
/// [`dartforge_arc_objeto_owned_v1`]. Não executa o construtor Dart.
///
/// # Safety
/// `tabela` deve cumprir o protocolo acima: ponteiro legível, quantidade e
/// entradas válidas, memória durável e ausência de Dart/GC/exceções. A função
/// de registro lê a memória devolvida; um ponteiro inválido não é verificável.
///
/// # Panics
/// Classe/quantidade inválida ou falha interna; aborta no limite C. Tabela
/// inválida viola o protocolo nativo, como em `dartforge_object_new_t`.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn tabela() -> *const i64 {
///     static TABLE: [i64; 2] = [32000, 0];
///     TABLE.as_ptr()
/// }
/// // SAFETY: thunk pure, tabela estática válida e without entries.
/// let object = unsafe { dartforge_arc_objeto_owned_t_v1(32000, 0, tabela) };
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(object), 3);
/// dartforge_arc_release(object);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(object), 0);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_objeto_owned_t_v1(class: i64, fields: i64, tabela: extern "C" fn() -> *const i64) -> i64 {
    owned_instance_parameters(class, fields).expect("contrato de alocação de instância ARC violado");
    dartforge_registrar_tabela(class, tabela);
    dartforge_arc_objeto_owned_v1(class, fields)
}

/// Grava bits escalares num campo de uma instância emprestada.
///
/// Exige receiver vivo e mutável, índice válido e layout escalar certificado
/// pelo lowering. Não interpreta os bits como handle: remove a marca Ref e
/// a aresta forte anterior, mesmo quando os bits novos parecem um endereço.
/// Aplica a barreira do heap. Não aloca, coleta ou executa Dart; invalida
/// empréstimos porque a substituição pode retirar a última aresta do filho.
/// Não faz guardas Dart de tipo/late nem certifica versão ou pin de layout.
///
/// # Panics
/// Violação das precondições ou falha interna de contagem; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_objeto_owned_v1(123, 40);
/// dartforge_arc_gravar_campo_escalar_v1(object, 39, i64::MAX);
/// assert_eq!(dartforge_arc_ler_campo_escalar_v1(object, 39), i64::MAX);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_gravar_campo_escalar_v1(object: i64, ix: i64, bits: i64) {
    HEAP.with(|heap| write_audited_field(&mut heap.borrow_mut(), object, ix, bits, false)
        .expect("contrato de gravação escalar ARC violado"));
}

/// Publica uma referência emprestada como aresta forte de uma instância.
///
/// Não consome token do receiver nem do valor. Exige receiver vivo e mutável,
/// índice válido, valor Ref vivo (ou null/Smi/permanente) e layout Ref
/// certificado. Atualiza bits, mapa e barreira, retendo a nova aresta antes
/// de soltar a anterior. Autoatribuição não muda a multiplicidade.
/// Não aloca, coleta ou executa Dart. Invalida empréstimos pela substituição
/// da aresta; guardas Dart de tipo/late e versão/pins são obrigações do lowering.
///
/// # Panics
/// Violação das precondições ou falha interna de contagem; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_objeto_owned_v1(123, 40);
/// let child = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_gravar_campo_ref_v1(object, 39, child);
/// dartforge_arc_release(child);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(child), 1);
/// dartforge_arc_gravar_campo_ref_v1(object, 39, 0);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(child), 0);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_gravar_campo_ref_v1(object: i64, ix: i64, value: i64) {
    HEAP.with(|heap| write_audited_field(&mut heap.borrow_mut(), object, ix, value, true)
        .expect("contrato de gravação Ref ARC violado"));
}

/// Grava uma aresta Ref somente após conferir o layout físico do receiver.
///
/// `layouts` contém pares ordenados `(cid, quantidade_de_campos)` de classes
/// cujo campo `ix` foi declarado Ref pelo compilador. A conferência usa
/// a classe e o tamanho reais, inclusive na primeira escrita do campo zerado.
/// Não consome owners, aloca, coleta ou executa Dart; invalida empréstimos
/// da aresta anterior. Guardas Dart de tipo/late continuam no lowering.
///
/// # Safety
/// O vetor deve estar alinhado e conter `pair_list * 2` palavras legíveis e
/// imutáveis durante a chamada. Cada par deve corresponder ao layout atual
/// da imagem, com `ix` Ref. A função não guarda nem devolve o endereço.
/// Vida/versões da imagem são obrigação do chamador.
///
/// # Panics
/// Receiver, índice, classe ou tamanho fora dos layouts; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let layouts = [123, 40];
/// let object = dartforge_arc_objeto_owned_v1(123, 40);
/// let child = dartforge_arc_box_int_owned_v1(i64::MAX);
/// // SAFETY: vetor alive e imutável, field 39 declarado Ref nesta imagem.
/// unsafe { dartforge_arc_gravar_campo_ref_conferido_v1(object, 39, child, layouts.as_ptr(), 1); }
/// dartforge_arc_release(child);
/// assert_eq!(dartforge_arc_ler_campo_ref_v1(object, 39), child);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_arc_gravar_campo_ref_conferido_v1(
    object: i64, ix: i64, value: i64, layouts: *const i64, pair_list: i64,
) {
    let words = usize::try_from(pair_list).ok().and_then(|n| n.checked_mul(2))
        .filter(|n| *n > 0 && *n <= isize::MAX as usize / std::mem::size_of::<i64>())
        .expect("vetor de layouts ARC inválido");
    assert!(!layouts.is_null() && layouts.is_aligned(), "endereço dos layouts ARC inválido");
    // SAFETY: o produtor garante a extensão, imutabilidade e vida do vetor.
    let layouts = unsafe { std::slice::from_raw_parts(layouts, words) };
    HEAP.with(|heap| write_checked_ref_field(&mut heap.borrow_mut(), object, ix, value, layouts)
        .expect("contrato de gravação Ref conferida ARC violado"));
}

fn write_checked_ref_field(
    heap: &mut Heap, object: i64, ix: i64, value: i64, layouts: &[i64],
) -> Result<(), &'static str> {
    check_physical_layout(heap, object, ix, layouts)?;
    write_audited_field(heap, object, ix, value, true)
}

fn check_physical_layout(heap: &Heap, object: i64, ix: i64, layouts: &[i64]) -> Result<(), &'static str> {
    let vista = heap.object(object).ok_or("receiver não é objeto vivo")?;
    let i = usize::try_from(ix).map_err(|_| "índice negativo")?;
    if i >= vista.len() { return Err("índice fora do objeto"); }
    let fits = |fields: i64| usize::try_from(fields).ok() == Some(vista.len());
    if layouts.len() % 2 != 0 || !(layout_fields(layouts, vista.class_id).is_some_and(fits)
        || layouts.chunks_exact(2).any(|p| p[0] == vista.class_id && fits(p[1]))) {
        return Err("classe/tamanho fora dos layouts Ref");
    }
    Ok(())
}

/// Os campos declarados de `cid` nos pares `(cid, fields)` em ordem de `cid`
/// (a do produtor, `layouts_do_campo`), por busca binária. Um vetor fora de
/// ordem só perde o atalho: quem chama confere de novo em sequência.
fn layout_fields(layouts: &[i64], cid: i64) -> Option<i64> {
    let (mut low, mut high) = (0, layouts.len() / 2);
    while low < high {
        let mid = (low + high) / 2;
        match layouts[2 * mid].cmp(&cid) {
            std::cmp::Ordering::Less => low = mid + 1,
            std::cmp::Ordering::Greater => high = mid,
            std::cmp::Ordering::Equal => return Some(layouts[2 * mid + 1]),
        }
    }
    None
}

/// Grava bits escalares após conferir classe e tamanho contra layouts declarados.
///
/// `representation` é 1=int, 2=double, 3=bool, 4=byte. Os bits já devem estar
/// convertidos pelo lowering; não faz coerções, aloca, coleta ou executa Dart.
/// Mantém a barreira e invalida borrows anteriores ao substituir o campo.
///
/// # Safety
/// `layouts` deve apontar para `pair_list * 2` palavras alinhadas, imutáveis e
/// legíveis durante a chamada. Os pares `(cid, fields)` devem declarar o
/// índice com a representação indicada na versão atual da imagem.
/// Não guarda nem devolve esse endereço; versões/pins são dever do chamador.
///
/// # Panics
/// Representação, receiver, índice, classe ou tamanho inválido; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let layouts = [123, 1];
/// let object = dartforge_arc_objeto_owned_v1(123, 1);
/// // SAFETY: vetor alive; o field 0 é int no layout current da imagem.
/// unsafe { dartforge_arc_gravar_campo_escalar_conferido_v1(object, 0, 42, layouts.as_ptr(), 1, 1); }
/// assert_eq!(dartforge_arc_ler_campo_escalar_v1(object, 0), 42);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_arc_gravar_campo_escalar_conferido_v1(
    object: i64, ix: i64, bits: i64, layouts: *const i64, pair_list: i64, representation: i64,
) {
    assert!((1..=4).contains(&representation), "representação escalar ARC inválida");
    let words = usize::try_from(pair_list).ok().and_then(|n| n.checked_mul(2))
        .filter(|n| *n > 0 && *n <= isize::MAX as usize / std::mem::size_of::<i64>())
        .expect("vetor de layouts ARC inválido");
    assert!(!layouts.is_null() && layouts.is_aligned(), "endereço dos layouts ARC inválido");
    // SAFETY: vetor e extensão garantidos pelo produtor.
    let layouts = unsafe { std::slice::from_raw_parts(layouts, words) };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        check_physical_layout(&heap, object, ix, layouts).expect("layout escalar ARC incompatível");
        write_audited_field(&mut heap, object, ix, bits, false)
            .expect("contrato de gravação escalar conferida ARC violado");
    });
}

/// Lê os bits escalares após conferir o layout físico e o mapa real do campo.
///
/// A representação (1=int, 2=double, 3=bool, 4=byte) e as conversões são
/// obrigações do lowering. Não cria owners, aloca, coleta ou executa Dart.
///
/// # Safety
/// O vetor `(cid, fields)` deve conter `pair_list * 2` palavras alinhadas,
/// imutáveis e legíveis. Cada par deve declarar o índice com a representação
/// indicada no layout atual da imagem. Não guarda esse endereço.
/// Vida/versões e guardas Dart de tipo/late são responsabilidade do chamador.
///
/// # Panics
/// Representação, receiver, índice, classe, tamanho ou mapa incompatível;
/// aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let layouts = [123, 1];
/// let object = dartforge_arc_objeto_owned_v1(123, 1);
/// dartforge_arc_gravar_campo_escalar_v1(object, 0, 42);
/// // SAFETY: vetor alive; o field 0 é int no layout current da imagem.
/// let bits = unsafe { dartforge_arc_ler_campo_escalar_conferido_v1(object, 0, layouts.as_ptr(), 1, 1) };
/// assert_eq!(bits, 42);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_arc_ler_campo_escalar_conferido_v1(
    object: i64, ix: i64, layouts: *const i64, pair_list: i64, representation: i64,
) -> i64 {
    assert!((1..=4).contains(&representation), "representação escalar ARC inválida");
    let words = usize::try_from(pair_list).ok().and_then(|n| n.checked_mul(2))
        .filter(|n| *n > 0 && *n <= isize::MAX as usize / std::mem::size_of::<i64>())
        .expect("vetor de layouts ARC inválido");
    assert!(!layouts.is_null() && layouts.is_aligned(), "endereço dos layouts ARC inválido");
    // SAFETY: vetor e extensão garantidos pelo produtor.
    let layouts = unsafe { std::slice::from_raw_parts(layouts, words) };
    HEAP.with(|heap| {
        let heap = heap.borrow();
        check_physical_layout(&heap, object, ix, layouts).expect("layout escalar ARC incompatível");
        read_audited_scalar_field(&heap, object, ix)
            .expect("contrato de leitura escalar conferida ARC violado")
    })
}

fn write_audited_field(heap: &mut Heap, object: i64, ix: i64, bits: i64, reference: bool) -> Result<(), &'static str> {
    let vista = heap.object(object).ok_or("receiver não é objeto vivo")?;
    let ix = usize::try_from(ix).map_err(|_| "índice negativo")?;
    if ix >= vista.len() { return Err("índice fora do objeto"); }
    heap.set_field(object, ix, bits, reference);
    Ok(())
}

/// Copia os bits de um campo escalar de um objeto emprestado.
///
/// Exige objeto vivo, índice válido e campo não marcado como referência no
/// mapa do heap. O lowering deve provar o layout correspondente antes de
/// escolher esta ABI. Não aloca, coleta, retém o objeto, invalida empréstimos
/// nem converte os bits. O catálogo pode declarar resultado escalar porque
/// a implementação confere o mapa, não porque a representação é i64.
///
/// # Panics
/// Violação das precondições de layout/objeto/índice; aborta no limite C.
/// Não substitui as verificações Dart de receiver, tipo ou inicialização late.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_object_new(123, 1);
/// dartforge_arc_retain(object);
/// dartforge_object_set(object, 0, i64::MAX, 0);
/// assert_eq!(dartforge_arc_ler_campo_escalar_v1(object, 0), i64::MAX);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ler_campo_escalar_v1(object: i64, ix: i64) -> i64 {
    HEAP.with(|heap| read_audited_scalar_field(&heap.borrow(), object, ix)
        .expect("contrato de leitura escalar ARC violado"))
}

fn read_audited_scalar_field(heap: &Heap, object: i64, ix: i64) -> Result<i64, &'static str> {
    // O record posicional do runtime: a palavra 0 é o número de campos.
    if let Some(fields) = heap.record(object) {
        return if ix == 0 { Ok(fields.len() as i64) } else { Err("campo do record é referência") };
    }
    let object = heap.readable_object(object).ok_or("receiver não é objeto vivo")?;
    let ix = usize::try_from(ix).map_err(|_| "índice negativo")?;
    let (bits, reference) = object.get(ix).ok_or("índice fora do objeto")?;
    if reference { return Err("campo é referência gerenciada"); }
    Ok(bits)
}

/// Empresta uma referência de campo, sem criar token Owned para o resultado.
///
/// Exige instância viva, índice válido e marca Ref no mapa real do heap.
/// Null/Smi são referências válidas. O resultado depende da aresta do receiver:
/// manter só o receiver vivo não protege o filho após substituir o campo.
/// O mapa real confere a representação Ref mesmo sem classe nominal estática.
/// O lowering deve preservar guardas semânticas e limites de empréstimo; antes
/// de uma invalidação, deve copiar para Owned ou encerrar seu uso.
/// Não aloca, coleta, retém, invalida outros borrows ou executa Dart.
/// Não substitui guardas Dart de tipo/late nem certifica versões/pins.
///
/// # Panics
/// Receiver/índice/mapa incompatível; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_objeto_owned_v1(123, 40);
/// let child = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_gravar_campo_ref_v1(object, 39, child);
/// dartforge_arc_release(child);
/// let borrowed = dartforge_arc_ler_campo_ref_v1(object, 39);
/// assert_eq!(borrowed, child);
/// dartforge_arc_retain(borrowed); // Copia o empréstimo before da substituição.
/// dartforge_arc_gravar_campo_ref_v1(object, 39, 0);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(borrowed), 3);
/// dartforge_arc_release(borrowed);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ler_campo_ref_v1(object: i64, ix: i64) -> i64 {
    HEAP.with(|heap| read_audited_ref_field(&heap.borrow(), object, ix)
        .expect("contrato de leitura Ref ARC violado"))
}

fn read_audited_ref_field(heap: &Heap, object: i64, ix: i64) -> Result<i64, &'static str> {
    // O record posicional do runtime: os campos `1..=n` são referências.
    if let Some(fields) = heap.record(object) {
        return usize::try_from(ix).ok().filter(|&i| i >= 1)
            .and_then(|i| fields.get(i - 1).copied())
            .ok_or("índice fora do record");
    }
    let object = heap.readable_object(object).ok_or("receiver não é objeto vivo")?;
    let ix = usize::try_from(ix).map_err(|_| "índice negativo")?;
    let (bits, reference) = object.get(ix).ok_or("índice fora do objeto")?;
    // O campo ainda não gravado nasce zerado e marcado escalar: é o null do
    // campo Ref sem inicializador (`Object? x;`), sem objeto nem owner.
    if !reference && bits != 0 { return Err("campo é escalar"); }
    Ok(bits)
}

/// Lê um empréstimo Ref de campo late ou o null anterior à inicialização.
///
/// Exige receiver vivo e índice válido. Um campo marcado Ref segue o mesmo
/// contrato de aresta do leitor Ref. Sem essa marca, apenas zero de um campo
/// ainda não inicializado pode sair como null; bits escalares não viram Ref.
/// Não marca inicialização, retém, aloca, coleta ou executa Dart. Guardas late,
/// inicializadores e cópia antes de invalidar a aresta continuam no chamador.
///
/// # Panics
/// Receiver, índice ou representação incompatível; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_objeto_owned_v1(123, 1);
/// assert_eq!(dartforge_arc_ler_campo_late_ref_v1(object, 0), 0);
/// dartforge_arc_gravar_campo_ref_v1(object, 0, 0);
/// dartforge_late_field_mark_initialized(object, 0);
/// assert_eq!(dartforge_arc_ler_campo_late_ref_v1(object, 0), 0);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ler_campo_late_ref_v1(object: i64, ix: i64) -> i64 {
    HEAP.with(|heap| read_audited_late_ref_field(&heap.borrow(), object, ix)
        .expect("contrato de leitura late Ref ARC violado"))
}

fn read_audited_late_ref_field(heap: &Heap, object: i64, ix: i64) -> Result<i64, &'static str> {
    let vista = heap.object(object).ok_or("receiver não é objeto vivo")?;
    let index = usize::try_from(ix).map_err(|_| "índice negativo")?;
    let (bits, reference) = vista.get(index).ok_or("índice fora do objeto")?;
    if reference || (bits == 0 && !heap.late_initialized((object, ix))) {
        Ok(bits)
    } else {
        Err("campo late não contém Ref nem zero anterior à inicialização")
    }
}

/// Lança um valor Ref emprestado e o mantém como raiz da exceção pendente.
///
/// Não consome o token do chamador. O protocolo de exceções mantém uma raiz
/// independente até clear/consumo; a preparação do rastro pode alocar/coletar.
/// O lowering deve fornecer um Ref vivo e conferir a pendência imediatamente.
/// Não converte bits escalares nem aplica a semântica Dart de `throw null`.
///
/// # Panics
/// Falha interna do protocolo de exceções/heap; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_box_int_owned_v1, dartforge_arc_lancar_ref_v1,
///     dartforge_arc_release, dartforge_exception_pending, dartforge_exception_clear};
/// let value = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_lancar_ref_v1(value);
/// assert_ne!(dartforge_exception_pending(), 0);
/// dartforge_arc_release(value);
/// dartforge_exception_clear();
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lancar_ref_v1(value: i64) {
    dartforge_exception_throw(value, 3);
}

/// Copia a exceção pendente para um token Owned independente do clear.
///
/// Preserva a identidade de valores Ref. Escalares são encaixotados dentro
/// do mesmo empréstimo do heap que publica o owner, antes de devolver o handle.
/// Sem pendência devolve null; release de null não altera contadores.
/// Não consome a pendência nem modifica seu rastro. Pode alocar/coletar.
///
/// # Panics
/// Falha interna de alocação/ownership; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let original = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_lancar_ref_v1(original);
/// let captured = dartforge_arc_excecao_owned_v1();
/// assert_eq!(captured, original);
/// dartforge_arc_release(original);
/// dartforge_exception_clear();
/// dartforge_arc_collect();
/// dartforge_arc_release(captured);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_excecao_owned_v1() -> i64 {
    let Some(value) = EXCEPTION.with(|slot| *slot.borrow()) else { return 0; };
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let reference = heap.to_ref(value);
        heap.retain_code_owner(reference);
        reference
    })
}

/// Copia o rastro corrente para um token Owned independente do clear.
///
/// Sem rastro corrente cria um StackTrace não vazio, sem instalá-lo como
/// rastro de uma exceção futura. Pode alocar/coletar; publica o owner antes
/// de devolver o handle, sem safepoint entre a leitura e a retenção.
///
/// # Panics
/// Falha interna de alocação/ownership; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let trace = dartforge_arc_rastro_owned_v1();
/// dartforge_arc_collect();
/// dartforge_arc_release(trace);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_rastro_owned_v1() -> i64 {
    let trace = dartforge_stack_trace_get();
    HEAP.with(|h| h.borrow_mut().retain_code_owner(trace));
    trace
}

/// Lança um Ref emprestado com um StackTrace explícito emprestado.
///
/// Não consome os dois tokens do chamador. Publica raízes independentes
/// para valor e rastro até clear/consumo. Exige ambos vivos e um StackTrace
/// válido; não aplica a semântica Dart de `throw null`.
/// O lowering deve conferir a pendência imediatamente após a chamada.
///
/// # Panics
/// Falha interna do protocolo de exceções/heap; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let value = dartforge_arc_box_int_owned_v1(i64::MAX);
/// let trace = dartforge_arc_rastro_owned_v1();
/// dartforge_arc_lancar_com_rastro_ref_v1(value, trace);
/// let copy = dartforge_arc_rastro_owned_v1();
/// assert_eq!(copy, trace);
/// dartforge_arc_release(value);
/// dartforge_arc_release(trace);
/// dartforge_exception_clear();
/// dartforge_arc_collect();
/// dartforge_arc_release(copy);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lancar_com_rastro_ref_v1(value: i64, trace: i64) {
    dartforge_throw_with_stack_trace(value, 3, trace);
}

/// Entrega um int em representação Ref com um token Owned para o chamador.
///
/// Smi não tem contador; valores fora de sua faixa criam um Mint mortal.
/// A alocação pode coletar, mas o token é publicado antes de devolver o handle.
/// O chamador deve transferi-lo ou chamar release exatamente uma vez.
///
/// # Panics
/// Falha interna de alocação/contagem; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_box_int_owned_v1, dartforge_arc_release, dartforge_arc_collect};
/// let value = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_release(value);
/// dartforge_arc_collect();
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_box_int_owned_v1(value: i64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let boxed = heap.box_int(value);
        heap.retain_code_owner(boxed);
        boxed
    })
}

/// Encaixota um double e entrega um token Owned antes de outra coleta.
///
/// Preserva todos os bits, inclusive zero negativo e payload de NaN. A caixa
/// pertence ao chamador até release ou transferência para slot/retorno.
///
/// # Panics
/// Empréstimo reentrante do heap ou falha de alocação; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let value = dartforge_arc_box_double_owned_v1(-0.0);
/// assert_eq!(dartforge_arc_observar_heap_v1(value), 3);
/// dartforge_arc_release(value);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(value), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_box_double_owned_v1(value: f64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let boxed = heap.box_double(value);
        heap.retain_code_owner(boxed);
        boxed
    })
}

/// Encaixota os 128 bits de um SIMD e entrega um owner do código.
///
/// As palavras preservam os bits nativos das pistas, inclusive payload de NaN.
/// `cid` deve identificar Float32x4, Int32x4 ou Float64x2. Nenhuma pista é Ref.
///
/// # Panics
/// Classe incompatível, empréstimo reentrante ou falha de alocação; aborta no C.
///
/// ```
/// use dartforge_runtime::{abi::*, layout::cid};
/// let value = dartforge_arc_box_simd_owned_v1(cid::FLOAT32X4 as i64, 0, 0);
/// assert_eq!(dartforge_arc_observar_heap_v1(value), 3);
/// dartforge_arc_release(value);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(value), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_box_simd_owned_v1(cid: i64, low: i64, high: i64) -> i64 {
    let cid = i32::try_from(cid).expect("classe SIMD fora da representação");
    assert!(matches!(cid, crate::layout::cid::FLOAT32X4 | crate::layout::cid::INT32X4 | crate::layout::cid::FLOAT64X2), "classe SIMD incompatível");
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&low.to_ne_bytes());
    bytes[8..].copy_from_slice(&high.to_ne_bytes());
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let boxed = heap.new_simd(cid, bytes);
        heap.retain_code_owner(boxed);
        boxed
    })
}

#[cfg(test)]
mod tests_owned_simd_boxes {
    use super::*;

    #[test]
    fn hints_keep_bits_and_owner_protects_until_release_in_both_modes() {
        for arc in [false, true] {
            for cid in [crate::layout::cid::FLOAT32X4, crate::layout::cid::INT32X4,
                crate::layout::cid::FLOAT64X2] {
                HEAP.with(|h| h.replace(Heap::new(true)));
                if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
                for (low, high) in [(0i64, 0i64), (i64::MIN, 0x7ff8_0000_0000_0123),
                    (-1, i64::MAX), (0x7fc0_0123_8000_0000, 0x0000_0001_7f80_0000)] {
                    let h = dartforge_arc_box_simd_owned_v1(cid as i64, low, high);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observar_heap_v1(h), 3);
                    HEAP.with(|heap| {
                        let heap = heap.borrow();
                        assert_eq!(heap.header(h).class_id, cid);
                        let b = heap.simd(h).unwrap();
                        assert_eq!(&b[..8], &low.to_ne_bytes());
                        assert_eq!(&b[8..], &high.to_ne_bytes());
                    });
                    dartforge_arc_release(h);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observar_heap_v1(h), 0);
                }
            }
        }
    }
}

/// Observa bloco vivo e owner do código sem adquirir referência ao objeto.
///
/// O argumento é um endereço nativo de diagnóstico, não um Ref emprestado:
/// pode designar um handle já liberado. Bit 0 indica bloco vivo deste heap;
/// bit 1 indica ao menos um token do código. Null, Smi, objetos estáticos e
/// endereços fora das páginas registradas não contam como blocos do heap.
/// Não coleta nem executa Dart. O snapshot não garante validade futura;
/// reutilização do endereço pode fazer uma observação posterior ver outro objeto.
///
/// # Panics
/// Empréstimo reentrante do heap; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let address = dartforge_arc_box_int_owned_v1(i64::MAX);
/// assert_eq!(dartforge_arc_observar_heap_v1(address), 3);
/// dartforge_arc_release(address);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(address), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_observar_heap_v1(address: i64) -> u8 {
    HEAP.with(|h| h.borrow().observe_code_owner(address))
}

/// Cria um token proprietário do código gerado a partir de um empréstimo.
///
/// Null e Smi não têm contador. Não coleta nem executa Dart. O token fica
/// no inventário do heap até release; slots proprietários são independentes.
///
/// # Panics
/// Handle morto ou overflow; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_retain, dartforge_arc_release};
/// dartforge_arc_retain(0);
/// dartforge_arc_release(0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_retain(value: i64) {
    HEAP.with(|h| h.borrow_mut().retain_code_owner(value));
}

/// Consome um token do código gerado, sem coletar ou executar Dart.
///
/// Não consome owners de quadros, campos ou mensagens. Null/Smi são no-op.
/// Zero de RC apenas agenda trabalho para um safepoint posterior.
///
/// # Panics
/// Handle sem token correspondente; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_retain, dartforge_arc_release};
/// dartforge_arc_retain(0);
/// dartforge_arc_release(0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_release(value: i64) {
    HEAP.with(|h| h.borrow_mut().release_code_owner(value));
}

/// Executa coleta explícita num safepoint com as raízes já publicadas.
///
/// Pode executar callbacks nativos de finalização; callbacks Dart são
/// enfileirados, sem execução nesta chamada. Em tracing faz coleta completa.
///
/// # Panics
/// Falha interna de coleta; aborta no limite C.
///
/// ```
/// dartforge_runtime::abi::dartforge_arc_collect();
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_collect() {
    HEAP.with(|h| h.borrow_mut().collect());
}

/// Confere a versão de tokens desta ABI e o modo ARC do heap corrente.
///
/// Retorna 1 somente para versão 1 com ARC ativo, 0 para incompatibilidade.
/// O chamador deve recusar a execução ARC quando o resultado for zero.
/// Não certifica contratos de um módulo nem a ABI de retornos das externs.
///
/// ```
/// assert_eq!(dartforge_runtime::abi::dartforge_arc_verificar_abi(-1), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_verificar_abi(version: i64) -> i8 {
    i8::from(version == 1 && HEAP.with(|h| h.borrow().arc_active()))
}

/// Abre um quadro proprietário com slots inicialmente nulos.
///
/// # Panics
/// Quantidade negativa/incompatível com usize ou falha interna; o limite C
/// aborta, sem desenrolar para o código gerado.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_quadro_abrir_v1, dartforge_arc_quadro_fechar_v1};
/// let frame = dartforge_arc_quadro_abrir_v1(0);
/// dartforge_arc_quadro_fechar_v1(frame);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_abrir_v1(slots: i64) -> i64 {
    let slots = usize::try_from(slots).expect("quantidade inválida de slots ARC");
    HEAP.with(|h| h.borrow_mut().push_owning_frame(slots))
}

/// Copia o valor para um slot, retendo antes de soltar o conteúdo antigo.
///
/// O quadro e índice são escalares; valor é Ref gerenciado, inclusive null/Smi.
/// Zero consome o conteúdo anterior. Não atende dívida de coleta.
///
/// # Panics
/// Quadro/slot inválido ou referência morta; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let frame = dartforge_arc_quadro_abrir_v1(1);
/// dartforge_arc_quadro_copiar_v1(frame, 0, 0);
/// dartforge_arc_quadro_fechar_v1(frame);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_copiar_v1(frame: i64, slot: i64, value: i64) {
    let slot = usize::try_from(slot).expect("índice inválido de slot ARC");
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.check_owning_frame(frame);
        h.set_root(frame, slot, value);
    });
}

/// Carrega um slot proprietário e cria um token owned do código.
///
/// O slot mantém sua ocorrência. O resultado precisa de release ou transferência,
/// mesmo se o quadro for fechado antes. Não coleta nem executa Dart.
///
/// # Panics
/// Quadro observacional/inexistente, slot inválido ou falha interna; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let frame = dartforge_arc_quadro_abrir_v1(1);
/// let value = dartforge_arc_quadro_carregar_v1(frame, 0);
/// dartforge_arc_quadro_fechar_v1(frame);
/// dartforge_arc_release(value);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_carregar_v1(frame: i64, slot: i64) -> i64 {
    let slot = usize::try_from(slot).expect("índice inválido de slot ARC");
    HEAP.with(|h| h.borrow_mut().copy_root_to_code(frame, slot))
}

/// Transfere um token do código para um slot proprietário.
///
/// Não retém a origem. Publica o destino antes de soltar seu owner anterior.
/// Null/Smi não exigem token físico. Não coleta nem executa Dart.
///
/// # Panics
/// Quadro/slot inválido ou handle sem token do código; valida antes da mutação.
/// Falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let frame = dartforge_arc_quadro_abrir_v1(1);
/// dartforge_arc_quadro_receber_v1(frame, 0, 0);
/// dartforge_arc_quadro_fechar_v1(frame);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_receber_v1(frame: i64, slot: i64, value: i64) {
    let slot = usize::try_from(slot).expect("índice inválido de slot ARC");
    HEAP.with(|h| h.borrow_mut().move_code_to_root(frame, slot, value));
}

/// Transfere token do código ao registro de owner de um global.
///
/// `id` identifica o armazenamento, não é desreferenciado. O código gerado
/// deve publicar o valor real antes, sem safepoint até esta chamada. O registro
/// guarda apenas owners de handles; não fornece o valor de uma carga global.
/// Não retém a origem, coleta ou executa Dart. Null/Smi retiram owner anterior.
///
/// # Panics
/// Handle morto ou sem token do código; falha interna aborta no limite C.
///
/// ```
/// dartforge_runtime::abi::dartforge_arc_global_receber_v1(17, 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_global_receber_v1(id: i64, value: i64) {
    HEAP.with(|h| h.borrow_mut().move_code_to_global(id, value));
}

/// Move um owner entre slots, consumindo origem e conteúdo antigo do destino.
///
/// Mover para o mesmo slot preserva a ocorrência. Não há coleta nem Dart
/// entre publicação do destino e limpeza da origem.
///
/// # Panics
/// Quadros não proprietários, inexistentes ou índices inválidos; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let frame = dartforge_arc_quadro_abrir_v1(2);
/// dartforge_arc_quadro_mover_v1(frame, 0, frame, 1);
/// dartforge_arc_quadro_fechar_v1(frame);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_mover_v1(
    origin: i64,
    source_slot: i64,
    dest: i64,
    dest_slot: i64,
) {
    let de = usize::try_from(source_slot).expect("índice inválido de origem ARC");
    let to = usize::try_from(dest_slot).expect("índice inválido de destino ARC");
    HEAP.with(|h| h.borrow_mut().move_root(origin, de, dest, to));
}

/// Consome os owners do quadro do topo sem coletar ou executar Dart.
///
/// # Panics
/// Quadro não está no topo; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let frame = dartforge_arc_quadro_abrir_v1(1);
/// dartforge_arc_quadro_fechar_v1(frame);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_fechar_v1(frame: i64) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.check_owning_frame(frame);
        h.pop_frame(frame);
    });
}

#[cfg(test)]
mod tests_arc_abi_frames {
    use super::*;

    #[test]
    fn exception_ref_preserves_mint_after_release_and_clear_removes_root() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let value = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_lancar_ref_v1(value);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_arc_release(value);
            dartforge_arc_collect();
            HEAP.with(|h| assert!(h.borrow().is_live_object(value)));
            assert_eq!(dartforge_exception_peek_ref(), value);
            dartforge_exception_clear();
            assert_eq!(dartforge_exception_pending(), 0);
            dartforge_arc_collect();
            HEAP.with(|h| assert!(!h.borrow().is_live_object(value)));
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn owned_capture_preserves_identity_and_survives_clear_in_both_modes() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            dartforge_exception_clear();
            assert_eq!(dartforge_arc_excecao_owned_v1(), 0);
            let original = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_lancar_ref_v1(original);
            let captured = dartforge_arc_excecao_owned_v1();
            assert_eq!(captured, original);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_arc_release(original);
            dartforge_exception_clear();
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(captured)));
            assert_eq!(HEAP.with(|h| h.borrow().int_of(captured)), Some(i64::MAX));
            dartforge_arc_release(captured);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(captured)));
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn owned_trace_preserves_identity_and_dies_after_last_release() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            dartforge_exception_clear();
            let original = dartforge_arc_box_int_owned_v1(i64::MAX);
            let trace = dartforge_arc_rastro_owned_v1();
            assert!(CURRENT_STACK_TRACE.with(|slot| slot.borrow().is_none()));
            dartforge_arc_lancar_com_rastro_ref_v1(original, trace);
            let captured_value = dartforge_arc_rastro_owned_v1();
            let value = dartforge_arc_excecao_owned_v1();
            assert_eq!(captured_value, trace);
            assert_eq!(value, original);
            assert_eq!(CURRENT_STACK_TRACE.with(|slot| *slot.borrow()), Some(trace));
            dartforge_arc_release(original);
            dartforge_arc_release(trace);
            dartforge_exception_clear();
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(captured_value)));
            assert!(!trace_text(captured_value).unwrap().is_empty());
            assert_eq!(HEAP.with(|h| h.borrow().int_of(value)), Some(i64::MAX));
            dartforge_arc_release(captured_value);
            dartforge_arc_release(value);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(captured_value)));
            assert!(!HEAP.with(|h| h.borrow().is_live_object(value)));
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn owned_capture_boxes_scalar_without_consuming_pending() {
        use crate::heap::Value;
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            for (bits, tag, expected) in [
                (i64::MAX, 1, Value::Int(i64::MAX)),
                (42, 1, Value::Int(42)),
                (3.5_f64.to_bits() as i64, 4, Value::Double(3.5)),
                (1, 2, Value::Bool(true)),
                (0, 2, Value::Bool(false)),
                (0, 3, Value::Ref(0)),
            ] {
                dartforge_exception_throw(bits, tag);
                let captured = dartforge_arc_excecao_owned_v1();
                assert_eq!(EXCEPTION.with(|slot| *slot.borrow()), Some(expected));
                dartforge_exception_clear();
                dartforge_arc_collect();
                assert_eq!(HEAP.with(|h| h.borrow().value(captured)), expected);
                dartforge_arc_release(captured);
                dartforge_arc_collect();
                if matches!(expected, Value::Int(i64::MAX) | Value::Double(_)) {
                    assert!(!HEAP.with(|h| h.borrow().is_live_object(captured)));
                }
            }
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn native_observation_creates_no_owner_and_rejects_addresses_outside_heap() {
        for validate in [false, true] {
            for arc in [false, true] {
                let previous = HEAP.with(|h| h.replace(Heap::new(validate)));
                if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
                for address in [0, 2, i64::MIN, i64::MAX, smi::de(42).unwrap()] {
                    assert_eq!(dartforge_arc_observar_heap_v1(address), 0);
                }
                let address = dartforge_arc_box_int_owned_v1(i64::MAX);
                assert_eq!(dartforge_arc_observar_heap_v1(address), 3);
                dartforge_arc_retain(address);
                dartforge_arc_release(address);
                assert_eq!(dartforge_arc_observar_heap_v1(address), 3);
                dartforge_arc_release(address);
                assert_eq!(dartforge_arc_observar_heap_v1(address), 1);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(address), 0);
                HEAP.with(|h| { h.replace(previous); });
            }
        }
    }

    #[test]
    fn owned_factory_hands_out_token_and_mint_dies_after_last_release() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let value = dartforge_arc_box_int_owned_v1(i64::MAX);
            assert!(HEAP.with(|h| h.borrow().is_live_object(value)));
            assert_eq!(HEAP.with(|h| h.borrow().int_of(value)), Some(i64::MAX));
            // A cópia representa um retorno retido, independente do token inicial.
            dartforge_arc_retain(value);
            dartforge_arc_release(value);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(value)));
            dartforge_arc_release(value);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(value)));
            let immediate = dartforge_arc_box_int_owned_v1(42);
            assert_eq!(immediate, smi::de(42).unwrap());
            dartforge_arc_release(immediate);
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn abi_global_receives_mortal_object_and_releases_last_owner() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc {
                HEAP.with(|h| h.borrow_mut().enable_arc());
            }
            // O endereço representa armazenamento do chamador, sem uma raiz
            // observacional que possa esconder a perda do owner proprietário.
            let mut global = 0_i64;
            let id = (&mut global as *mut i64) as i64;
            let frame = dartforge_arc_quadro_abrir_v1(1);
            let value = HEAP.with(|h| h.borrow_mut().allocate_str("global mortal"));
            dartforge_arc_quadro_copiar_v1(frame, 0, value);
            global = dartforge_arc_quadro_carregar_v1(frame, 0);
            dartforge_arc_global_receber_v1(id, global);
            dartforge_arc_quadro_fechar_v1(frame);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(global)));

            // Reatribuir o mesmo objeto consome o token novo e o owner velho;
            // a identidade não pode pular a transferência ou vazar um retain.
            dartforge_arc_retain(global);
            dartforge_arc_global_receber_v1(id, global);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(value)));

            dartforge_arc_retain(global);
            global = smi::de(42).unwrap();
            dartforge_arc_global_receber_v1(id, global);
            dartforge_arc_collect();
            assert_eq!(global, smi::de(42).unwrap());
            assert!(HEAP.with(|h| h.borrow().is_live_object(value)));
            dartforge_arc_release(value);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(value)));
            global = 0;
            dartforge_arc_global_receber_v1(id, global);
            HEAP.with(|h| {
                h.replace(previous);
            });
        }
    }

    #[test]
    fn abi_loads_independent_owner_and_receives_token_in_slot() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(false)));
            if arc {
                HEAP.with(|h| h.borrow_mut().enable_arc());
            }
            let frame = dartforge_arc_quadro_abrir_v1(1);
            let value = HEAP.with(|h| h.borrow_mut().allocate_str("slot e SSA"));
            dartforge_arc_quadro_copiar_v1(frame, 0, value);
            let token = dartforge_arc_quadro_carregar_v1(frame, 0);
            assert_eq!(token, value);
            dartforge_arc_quadro_receber_v1(frame, 0, token);
            let token = dartforge_arc_quadro_carregar_v1(frame, 0);
            dartforge_arc_quadro_fechar_v1(frame);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(token)));
            dartforge_arc_release(token);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(token)));
            let frame = dartforge_arc_quadro_abrir_v1(1);
            for scalar in [0, smi::de(42).unwrap()] {
                dartforge_arc_quadro_receber_v1(frame, 0, scalar);
                assert_eq!(dartforge_arc_quadro_carregar_v1(frame, 0), scalar);
                dartforge_arc_release(scalar);
            }
            dartforge_arc_quadro_fechar_v1(frame);
            HEAP.with(|h| {
                h.replace(previous);
            });
        }
    }

    #[test]
    fn minimal_abi_checks_mode_and_keeps_tokens_until_release() {
        let previous = HEAP.with(|h| h.replace(Heap::new(false)));
        assert_eq!(dartforge_arc_verificar_abi(1), 0);
        HEAP.with(|h| h.borrow_mut().enable_arc());
        assert_eq!(dartforge_arc_verificar_abi(1), 1);
        assert_eq!(dartforge_arc_verificar_abi(2), 0);
        let value = HEAP.with(|h| h.borrow_mut().allocate_str("token ABI"));
        dartforge_arc_retain(value);
        dartforge_arc_retain(value);
        dartforge_arc_collect();
        assert!(HEAP.with(|h| h.borrow().is_live_object(value)));
        dartforge_arc_release(value);
        dartforge_arc_collect();
        assert!(HEAP.with(|h| h.borrow().is_live_object(value)));
        dartforge_arc_release(value);
        dartforge_arc_collect();
        assert!(!HEAP.with(|h| h.borrow().is_live_object(value)));
        HEAP.with(|h| {
            h.replace(previous);
        });
    }

    #[test]
    #[should_panic(expected = "quadro ARC deve ser proprietário")]
    fn abi_rejects_observational_frame_before_changing_slots() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        heap.check_owning_frame(frame);
    }

    #[test]
    fn copy_move_and_closure_preserve_occurrences_until_last_owner() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(false)));
            if arc {
                HEAP.with(|h| h.borrow_mut().enable_arc());
            }
            let origin = dartforge_arc_quadro_abrir_v1(2);
            let dest = dartforge_arc_quadro_abrir_v1(1);
            let value = HEAP.with(|h| h.borrow_mut().allocate_str("owner ABI"));
            dartforge_arc_quadro_copiar_v1(origin, 0, value);
            dartforge_arc_quadro_copiar_v1(origin, 1, value);
            dartforge_arc_quadro_mover_v1(origin, 0, dest, 0);
            dartforge_arc_quadro_mover_v1(dest, 0, dest, 0);
            dartforge_arc_quadro_copiar_v1(origin, 1, 0);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert!(h.is_live_object(value));
            });
            dartforge_arc_quadro_fechar_v1(dest);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert!(!h.is_live_object(value));
            });
            dartforge_arc_quadro_fechar_v1(origin);
            HEAP.with(|h| {
                h.replace(previous);
            });
        }
    }
}

#[cfg(test)]
mod tests_owned_double_box {
    use super::*;
    #[test]
    fn owned_double_preserves_bits_and_dies_after_last_token() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            for bits in [(-0.0_f64).to_bits(), 3.25_f64.to_bits(), f64::INFINITY.to_bits(), 0x7ff80000deadbeef] {
                let value = dartforge_arc_box_double_owned_v1(f64::from_bits(bits));
                dartforge_arc_retain(value);
                dartforge_arc_release(value);
                dartforge_arc_collect();
                assert_eq!(HEAP.with(|h| h.borrow().double_of(value).unwrap().to_bits()), bits);
                assert_eq!(dartforge_arc_observar_heap_v1(value), 3);
                dartforge_arc_release(value);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(value), 0);
            }
            HEAP.with(|h| { h.replace(previous); });
        }
    }
}

#[cfg(test)]
mod testes_campo_ref {
    use super::*;

    #[test]
    fn layouts_in_order_for_binary_search() {
        let layouts = [3, 2, 7, 4, 40, 1, 900, 6];
        for (cid, fields) in [(3, 2), (7, 4), (40, 1), (900, 6)] {
            assert_eq!(layout_fields(&layouts, cid), Some(fields));
        }
        assert_eq!(layout_fields(&layouts, 8), None);
        assert_eq!(layout_fields(&[], 8), None);
        // Fora de ordem pode não achar; a conferência refaz em sequência.
        assert_eq!(layout_fields(&[900, 6, 3, 2], 900), None);
    }

    #[test]
    fn positional_record_reads_scalar_count_and_ref_fields() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let child = dartforge_arc_box_int_owned_v1(i64::MAX);
        let record = HEAP.with(|h| h.borrow_mut().new_counted_record(&[child, 0]));
        HEAP.with(|h| {
            let h = h.borrow();
            assert_eq!(read_audited_scalar_field(&h, record, 0), Ok(2));
            assert!(read_audited_scalar_field(&h, record, 1).is_err());
            assert_eq!(read_audited_ref_field(&h, record, 1), Ok(child));
            assert_eq!(read_audited_ref_field(&h, record, 2), Ok(0));
            for i in [0, 3, -1] {
                assert!(read_audited_ref_field(&h, record, i).is_err());
            }
        });
        HEAP.with(|h| h.replace(previous));
    }

    #[test]
    fn checked_write_initializes_ref_and_rejects_layout_without_changing_edge() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let object = dartforge_arc_objeto_owned_v1(123, 40);
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            HEAP.with(|h| {
                let mut heap = h.borrow_mut();
                assert!(write_checked_ref_field(&mut heap, object, 39, child, &[123, 40]).is_ok());
                for (receiver, ix, layouts) in [
                    (object, 39, vec![124, 40]), (object, 39, vec![123, 39]),
                    (object, -1, vec![123, 40]), (object, 40, vec![123, 40]),
                    (0, 39, vec![123, 40]), (child, 0, vec![123, 40]),
                    (object, 39, vec![123]),
                ] {
                    assert!(write_checked_ref_field(&mut heap, receiver, ix, 0, &layouts).is_err());
                    assert_eq!(read_audited_ref_field(&heap, object, 39), Ok(child));
                }
            });
            dartforge_arc_release(child);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(child), 1);
            HEAP.with(|h| assert!(write_checked_ref_field(&mut h.borrow_mut(), object, 39, 0, &[123, 40]).is_ok()));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(child), 0);
            dartforge_arc_release(object);
            HEAP.with(|h| h.replace(previous));
        }
    }

    #[test]
    fn read_borrows_edge_and_owned_copy_survives_replacement() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let object = dartforge_arc_objeto_owned_v1(123, 40);
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_gravar_campo_ref_v1(object, 0, 0);
            dartforge_arc_gravar_campo_ref_v1(object, 1, smi::de(42).unwrap());
            dartforge_arc_gravar_campo_ref_v1(object, 39, child);
            dartforge_arc_gravar_campo_escalar_v1(object, 37, child);
            dartforge_arc_release(child);
            dartforge_arc_collect();
            let stats = HEAP.with(|h| h.borrow().stats());
            assert_eq!(dartforge_arc_ler_campo_ref_v1(object, 0), 0);
            assert_eq!(dartforge_arc_ler_campo_ref_v1(object, 1), smi::de(42).unwrap());
            assert_eq!(dartforge_arc_ler_campo_ref_v1(object, 39), child);
            assert_eq!(dartforge_arc_observar_heap_v1(child), 1);
            HEAP.with(|h| {
                let h = h.borrow();
                for (receiver, ix) in [(object, 37), (object, -1), (object, 40), (0, 0), (child, 0)] {
                    assert!(read_audited_ref_field(&h, receiver, ix).is_err());
                }
                // O campo nunca gravado (zerado, marcado escalar) é null.
                assert_eq!(read_audited_ref_field(&h, object, 38), Ok(0));
                assert_eq!(h.stats().allocations, stats.allocations);
                assert_eq!(h.stats().collections, stats.collections);
            });
            dartforge_arc_gravar_campo_ref_v1(object, 39, 0);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(child), 0);
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_gravar_campo_ref_v1(object, 39, child);
            dartforge_arc_release(child);
            let copy = dartforge_arc_ler_campo_ref_v1(object, 39);
            assert_eq!(copy, child);
            dartforge_arc_retain(copy);
            dartforge_arc_gravar_campo_ref_v1(object, 39, 0);
            dartforge_arc_release(object);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(object), 0);
            assert_eq!(dartforge_arc_observar_heap_v1(copy), 3);
            dartforge_arc_release(copy);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(copy), 0);
            HEAP.with(|h| { h.replace(previous); });
        }
    }
}

#[cfg(test)]
mod tests_late_ref_read {
    use super::*;

    #[test]
    fn late_ref_tells_empty_zero_written_null_and_scalar_bits() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let object = dartforge_arc_objeto_owned_v1(123, 40);
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            let before = HEAP.with(|h| h.borrow().stats());
            assert_eq!(dartforge_arc_ler_campo_late_ref_v1(object, 39), 0);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.mark_late((object, 39));
                assert!(read_audited_late_ref_field(&h, object, 39).is_err());
                h.unmark_late((object, 39));
                for (bits, marked) in [(child, false), (42, false), (0, true), (child, true)] {
                    h.set_field(object, 38, bits, false);
                    if marked { h.mark_late((object, 38)); } else { h.unmark_late((object, 38)); }
                    assert!(read_audited_late_ref_field(&h, object, 38).is_err());
                }
                for (receiver, ix) in [(0, 0), (object, -1), (object, 40)] {
                    assert!(read_audited_late_ref_field(&h, receiver, ix).is_err());
                }
                assert_eq!(h.stats().allocations, before.allocations);
                assert_eq!(h.stats().collections, before.collections);
            });
            for value in [0, smi::de(42).unwrap(), child] {
                dartforge_arc_gravar_campo_ref_v1(object, 39, value);
                HEAP.with(|h| h.borrow_mut().mark_late((object, 39)));
                assert_eq!(dartforge_arc_ler_campo_late_ref_v1(object, 39), value);
            }
            dartforge_arc_release(child);
            let copy = dartforge_arc_ler_campo_late_ref_v1(object, 39);
            dartforge_arc_retain(copy);
            dartforge_arc_gravar_campo_ref_v1(object, 39, 0);
            dartforge_arc_release(object);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(copy), 3);
            assert_eq!(dartforge_arc_observar_heap_v1(object), 0);
            HEAP.with(|h| assert!(read_audited_late_ref_field(&h.borrow(), object, 39).is_err()));
            dartforge_arc_release(copy);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(copy), 0);
            HEAP.with(|h| { h.replace(previous); });
        }
    }
}

#[cfg(test)]
mod tests_field_write {
    use super::*;

    #[test]
    fn write_tells_bits_from_edges_and_replaces_without_consuming_tokens() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let object = dartforge_arc_objeto_owned_v1(123, 40);
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            let before = HEAP.with(|h| h.borrow().stats());
            for ix in [0, 37, 38] {
                dartforge_arc_gravar_campo_ref_v1(object, ix, child);
                dartforge_arc_gravar_campo_ref_v1(object, ix, child);
            }
            for (ix, bits) in [(1, i64::MAX), (32, (-0.0_f64).to_bits() as i64), (39, 0x7ff80000deadbeef)] {
                dartforge_arc_gravar_campo_escalar_v1(object, ix, bits);
                assert_eq!(dartforge_arc_ler_campo_escalar_v1(object, ix), bits);
            }
            HEAP.with(|h| {
                let h = h.borrow();
                assert_eq!(h.stats().allocations, before.allocations);
                assert_eq!(h.stats().collections, before.collections);
                assert_eq!(h.observe_code_owner(object), 3);
                assert_eq!(h.observe_code_owner(child), 3);
            });
            dartforge_arc_release(child);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(child), 1);
            // Os mesmos bits deixam de constituir aresta nos dois mapas.
            for ix in [0, 37] { dartforge_arc_gravar_campo_escalar_v1(object, ix, child); }
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(child), 1);
            dartforge_arc_gravar_campo_ref_v1(object, 38, 0);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(child), 0);
            for ix in [0, 37] { assert_eq!(dartforge_arc_ler_campo_escalar_v1(object, ix), child); }
            dartforge_arc_gravar_campo_ref_v1(object, 33, object);
            dartforge_arc_collect();
            dartforge_arc_gravar_campo_escalar_v1(object, 33, 0);
            dartforge_arc_release(object);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(object), 0);
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn invalid_receiver_and_index_neither_write_nor_collect() {
        let mut heap = Heap::new(true);
        let object = heap.allocate_instance(123, 40);
        heap.retain_code_owner(object);
        let boxed = heap.box_int(i64::MAX);
        heap.retain_code_owner(boxed);
        let before = heap.object(object).unwrap().to_vec();
        let stats = heap.stats();
        for reference in [false, true] {
            for (receiver, ix) in [(object, -1), (object, 40), (0, 0), (boxed, 0)] {
                assert!(write_audited_field(&mut heap, receiver, ix, 0, reference).is_err());
                assert_eq!(heap.object(object).unwrap().to_vec(), before);
                assert_eq!(heap.stats().allocations, stats.allocations);
                assert_eq!(heap.stats().collections, stats.collections);
            }
        }
    }
}

#[cfg(test)]
mod tests_owned_instance {
    use super::*;

    #[test]
    fn owned_instance_registers_methods_before_allocating_and_preserves_token() {
        use std::sync::{OnceLock, atomic::{AtomicUsize, Ordering}};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        extern "C" fn input(_: i64, _: *const i64, _: *const i64) -> i64 { 85 }
        extern "C" fn tabela() -> *const i64 {
            static TABLE: OnceLock<[i64; 4]> = OnceLock::new();
            CALLS.fetch_add(1, Ordering::Relaxed);
            TABLE.get_or_init(|| [32001, 1, 42, input as *const () as usize as i64]).as_ptr()
        }
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        HEAP.with(|h| h.borrow_mut().enable_arc());
        for _ in 0..2 {
            // SAFETY: tabela estática válida com uma entrada nativa durável.
            let object = unsafe { dartforge_arc_objeto_owned_t_v1(32001, 1, tabela) };
            assert_eq!(class_method(32001, 42), Some(input as *const () as usize));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(object), 3);
            assert_eq!(dartforge_arc_ler_campo_escalar_v1(object, 0), 0);
            dartforge_arc_release(object);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(object), 0);
        }
        assert_eq!(CALLS.load(Ordering::Relaxed), 1);
        HEAP.with(|h| { h.replace(previous); });
    }

    #[test]
    fn zeroed_instance_and_token_survive_stress_until_a_release() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            for fields in [0, 1, 32, 33, 40] {
                let object = dartforge_arc_objeto_owned_v1(123, fields);
                assert_eq!(dartforge_arc_observar_heap_v1(object), 3);
                // A próxima fábrica pode coletar antes de publicar seu token.
                let other_one = dartforge_arc_objeto_owned_v1(124, 1);
                dartforge_arc_collect();
                HEAP.with(|h| {
                    let h = h.borrow();
                    let vista = h.object(object).unwrap();
                    assert_eq!(vista.class_id, 123);
                    assert_eq!(vista.len(), fields as usize);
                    assert!(vista.iter().all(|field| field == (0, false)));
                });
                let child = if fields > 0 {
                    let child = dartforge_arc_box_int_owned_v1(i64::MAX);
                    dartforge_object_set(object, fields - 1, child, 1);
                    dartforge_arc_release(child);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observar_heap_v1(child), 1);
                    assert_eq!(HEAP.with(|h| h.borrow().object(object).unwrap().field(fields as usize - 1)), (child, true));
                    Some(child)
                } else { None };
                dartforge_arc_release(other_one);
                dartforge_arc_release(object);
                assert_eq!(dartforge_arc_observar_heap_v1(object) & 2, 0);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(object), 0);
                assert_eq!(dartforge_arc_observar_heap_v1(other_one), 0);
                if let Some(child) = child { assert_eq!(dartforge_arc_observar_heap_v1(child), 0); }
            }
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn parameters_must_fit_in_header_without_truncation() {
        for class in [i32::MIN as i64, 0, i32::MAX as i64] {
            for fields in [0, 40, u16::MAX as i64] {
                assert_eq!(owned_instance_parameters(class, fields), Ok((class as i32, fields as usize)));
            }
        }
        for class in [i32::MIN as i64 - 1, i32::MAX as i64 + 1] {
            assert!(owned_instance_parameters(class, 1).is_err());
        }
        for fields in [-1, u16::MAX as i64 + 1, i64::MAX] {
            assert!(owned_instance_parameters(123, fields).is_err());
        }
    }
}

#[cfg(test)]
mod tests_scalar_field {
    use super::*;

    #[test]
    fn read_checks_map_without_mistaking_bits_for_handle_or_collecting() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let object = HEAP.with(|h| {
                let mut h = h.borrow_mut();
                let child = h.allocate_str("campo de referência");
                h.retain_code_owner(child);
                let mut fields = vec![(0, false); 40];
                fields[0] = (child, false);
                fields[1] = (child, true);
                fields[2] = ((-0.0_f64).to_bits() as i64, false);
                fields[3] = (0x7ff80000deadbeef, false);
                fields[32] = (i64::MAX, false);
                fields[37] = (child, true);
                let object = h.new_object(123, &fields);
                h.retain_code_owner(object);
                h.collect();
                object
            });
            let before = HEAP.with(|h| h.borrow().stats());
            for i in [0, 2, 3, 32, 39] {
                let expected = HEAP.with(|h| h.borrow().object(object).unwrap().field(i).0);
                assert_eq!(dartforge_arc_ler_campo_escalar_v1(object, i as i64), expected);
            }
            HEAP.with(|h| {
                let h = h.borrow();
                for i in [1, 37] {
                    assert_eq!(read_audited_scalar_field(&h, object, i), Err("campo é referência gerenciada"));
                }
                for i in [-1, 40] { assert!(read_audited_scalar_field(&h, object, i).is_err()); }
                assert!(read_audited_scalar_field(&h, 0, 0).is_err());
                assert_eq!(h.stats().allocations, before.allocations);
                assert_eq!(h.stats().collections, before.collections);
            });
            HEAP.with(|h| { h.replace(previous); });
        }
    }
}
