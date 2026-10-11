// ABI de tokens e slots proprietários usados pelo código gerado.
// A pilha observacional não substitui estes owners. Operações de ownership
// não coletam nem executam Dart; coleta tem um safepoint explícito separado.
// A fábrica owned de caixas é uma operação de alocação, com safepoint próprio.
// O lançamento Ref publica uma raiz runtime e pode alocar o rastro.

fn mapa_lista_auditado(heap: &Heap, lista: i64) -> Result<(i64, usize, usize), &'static str> {
    use crate::layout::cid;
    if !heap.e_lista(lista) { return Err("receiver não é lista viva"); }
    let (dados, comprimento) = if heap.cabecalho(lista).class_id == cid::GROWABLE_LIST {
        let objeto = heap.objeto(lista).ok_or("lista expansível sem mapa de instância")?;
        if objeto.len() != 2 { return Err("layout da lista expansível inválido"); }
        let (n, marca_n) = objeto.get(0).ok_or("comprimento ausente")?;
        let (dados, marca_dados) = objeto.get(1).ok_or("armazenamento ausente")?;
        if marca_n || !marca_dados { return Err("mapa da lista expansível inválido"); }
        (dados, usize::try_from(n).map_err(|_| "comprimento negativo")?)
    } else {
        let n = *heap.palavras(lista).first().ok_or("comprimento ausente")?;
        (lista, usize::try_from(n).map_err(|_| "comprimento negativo")?)
    };
    if !heap.e_lista(dados) || !cid::e_lista_fixa(heap.cabecalho(dados).class_id) {
        return Err("armazenamento não é lista fixa viva");
    }
    let palavras = heap.palavras(dados);
    let capacidade = usize::try_from(*palavras.first().ok_or("capacidade ausente")?)
        .map_err(|_| "capacidade negativa")?;
    if capacidade > palavras.len().saturating_sub(1) {
        return Err("capacidade fora do armazenamento");
    }
    Ok((dados, comprimento, capacidade))
}

fn armazenamento_lista_auditado(heap: &Heap, lista: i64) -> Result<(i64, usize), &'static str> {
    let (dados, comprimento, capacidade) = mapa_lista_auditado(heap, lista)?;
    if comprimento > capacidade { return Err("comprimento fora do armazenamento"); }
    Ok((dados, comprimento))
}

fn ler_elemento_ref_auditado(heap: &Heap, lista: i64, indice: i64) -> Result<i64, &'static str> {
    use crate::layout::flags;
    let (dados, comprimento) = armazenamento_lista_auditado(heap, lista)?;
    if heap.cabecalho(dados).flags & flags::FORMA != flags::REFS {
        return Err("armazenamento não contém referências");
    }
    let palavras = heap.palavras(dados);
    let indice = usize::try_from(indice).map_err(|_| "índice negativo")?;
    if indice >= comprimento { return Err("índice fora da lista"); }
    Ok(palavras[1 + indice])
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
/// // SAFETY: elementos Ref válidos durante a criação da lista.
/// let lista = unsafe { dartforge_lista_nova([0, 0].as_ptr(), 2, 8, 0) };
/// dartforge_arc_retain(lista);
/// assert_eq!(dartforge_arc_lista_len_v1(lista), 2);
/// dartforge_arc_release(lista);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_len_v1(lista: i64) -> i64 {
    HEAP.with(|h| {
        let heap = h.borrow();
        let (_, comprimento) = armazenamento_lista_auditado(&heap, lista)
            .expect("comprimento ARC de lista inválido");
        i64::try_from(comprimento).expect("comprimento fora de i64")
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
/// // SAFETY: um elemento Ref válido durante a criação.
/// let lista = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(lista);
/// assert_eq!(dartforge_arc_lista_armazenamento_ref_v1(lista), lista);
/// dartforge_arc_release(lista);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_armazenamento_ref_v1(lista: i64) -> i64 {
    HEAP.with(|h| armazenamento_lista_auditado(&h.borrow(), lista)
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
/// let lista = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(lista);
/// assert_eq!(dartforge_arc_lista_forma_v1(lista), 4);
/// dartforge_arc_release(lista);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_forma_v1(lista: i64) -> i64 {
    HEAP.with(|h| {
        let heap = h.borrow();
        let (dados, _) = armazenamento_lista_auditado(&heap, lista).expect("forma ARC inválida");
        i64::from(heap.cabecalho(dados).flags & (crate::layout::flags::FORMA | crate::layout::flags::ELEMENTO))
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
/// let lista = unsafe { dartforge_lista_nova([0, 0].as_ptr(), 2, 8, 0) };
/// dartforge_arc_retain(lista);
/// assert_eq!(dartforge_arc_lista_capacidade_v1(lista), 2);
/// dartforge_arc_release(lista);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_capacidade_v1(lista: i64) -> i64 {
    HEAP.with(|h| {
        let heap = h.borrow();
        let (dados, _) = armazenamento_lista_auditado(&heap, lista).expect("capacidade ARC inválida");
        heap.lista_len(dados) as i64
    })
}

fn conferir_len_lista_arc(heap: &Heap, lista: i64, novo: i64) -> Result<usize, &'static str> {
    let (_, _, capacidade) = mapa_lista_auditado(heap, lista)?;
    if heap.cabecalho(lista).class_id != crate::layout::cid::GROWABLE_LIST {
        return Err("comprimento mutável exige lista expansível");
    }
    let novo = usize::try_from(novo).map_err(|_| "comprimento negativo")?;
    if novo > capacidade { return Err("comprimento fora do armazenamento"); }
    Ok(novo)
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
/// let lista = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 10, 0) };
/// dartforge_arc_retain(lista);
/// dartforge_arc_lista_gravar_len_v1(lista, 0);
/// assert_eq!(dartforge_arc_lista_len_v1(lista), 0);
/// dartforge_arc_release(lista);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_gravar_len_v1(lista: i64, novo: i64) {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let novo = conferir_len_lista_arc(&heap, lista, novo).expect("gravação ARC de comprimento inválida");
        heap.lista_definir_len(lista, novo);
    })
}

fn conferir_bits_lista_arc(heap: &Heap, dados: i64, indice: i64, bits: i64, forma: i64)
    -> Result<usize, &'static str> {
    use crate::layout::{cid, flags};
    let (armazenamento, comprimento) = armazenamento_lista_auditado(heap, dados)?;
    if armazenamento != dados || heap.cabecalho(dados).class_id != cid::LIST {
        return Err("gravação exige armazenamento fixo mutável");
    }
    let f = heap.cabecalho(dados).flags & (flags::FORMA | flags::ELEMENTO);
    if forma != i64::from(f) || ![flags::BRUTO | flags::ELEMENTO_INT,
        flags::BRUTO | flags::ELEMENTO_DOUBLE, flags::BRUTO | flags::ELEMENTO_BOOL].contains(&f) {
        return Err("forma escalar incompatível");
    }
    if f == flags::BRUTO | flags::ELEMENTO_BOOL && ![0, 1].contains(&bits) {
        return Err("booleano não canônico");
    }
    let i = usize::try_from(indice).map_err(|_| "índice negativo")?;
    if i >= comprimento { return Err("índice fora do armazenamento"); }
    Ok(i)
}

/// Grava bits de int, double ou bool num armazenamento compacto mutável.
///
/// Confere forma e índice; `forma` usa os bits FORMA/ELEMENTO do layout.
/// Não encaixota, aloca, coleta ou executa Dart; invalida empréstimos anteriores.
///
/// # Panics
/// Lista, forma, índice ou booleano inválido; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let lista = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 8, 1) };
/// dartforge_arc_retain(lista);
/// dartforge_arc_lista_gravar_bits_v1(lista, 0, 42, 0x22);
/// dartforge_arc_release(lista);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_gravar_bits_v1(dados: i64, indice: i64, bits: i64, forma: i64) {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let i = conferir_bits_lista_arc(&heap, dados, indice, bits, forma).expect("gravação ARC escalar inválida");
        heap.palavras_mut(dados)[1 + i] = bits;
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
/// let lista = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(lista);
/// dartforge_arc_lista_gravar_ref_v1(lista, 0, 0);
/// dartforge_arc_release(lista);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_gravar_ref_v1(dados: i64, indice: i64, valor: i64) {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        assert!(heap.e_lista(dados) && heap.cabecalho(dados).class_id == crate::layout::cid::LIST,
            "gravação ARC Ref exige armazenamento mutável");
        ler_elemento_ref_auditado(&heap, dados, indice).expect("gravação ARC Ref inválida");
        assert!(!crate::layout::e_objeto(valor) || heap.e_objeto_vivo(valor), "referência ARC morta");
        heap.gravar_refs(dados, 1 + indice as usize, &[valor]);
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
/// let lista = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 10, 0) };
/// dartforge_arc_retain(lista);
/// let dados = dartforge_arc_lista_armazenamento_ref_v1(lista);
/// assert_eq!(dartforge_arc_lista_gravar_dados_v1(lista, dados), 1);
/// dartforge_arc_release(lista);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_gravar_dados_v1(lista: i64, dados: i64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let (velho, _, _) = mapa_lista_auditado(&heap, lista).expect("mapa ARC de lista inválido");
        assert_eq!(heap.cabecalho(lista).class_id, crate::layout::cid::GROWABLE_LIST);
        let (fixa, _) = armazenamento_lista_auditado(&heap, dados).expect("novo armazenamento ARC inválido");
        assert_eq!(fixa, dados, "armazenamento ARC deve ser fixo");
        let mascara = crate::layout::flags::FORMA | crate::layout::flags::ELEMENTO;
        if heap.cabecalho(velho).flags & mascara != heap.cabecalho(dados).flags & mascara { return 0; }
        heap.lista_definir_dados(lista, dados);
        1
    })
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
/// // SAFETY: um elemento Ref vivo e alinhado durante a cópia.
/// let lista = unsafe { dartforge_lista_nova([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(lista);
/// assert_eq!(dartforge_arc_lista_ref_owned_v1(lista, 0), 0);
/// dartforge_arc_release(lista);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lista_ref_owned_v1(lista: i64, indice: i64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let valor = ler_elemento_ref_auditado(&heap, lista, indice).expect("leitura ARC de lista inválida");
        heap.reter_owner_codigo(valor);
        valor
    })
}

#[cfg(test)]
mod testes_lista_ref_owned {
    use super::*;
    use crate::{layout::{cid, smi}, listas::Elemento, heap::Valor};

    #[test]
    fn acrescentar_empresta_receiver_e_preserva_raizes_em_crescimento_e_descompactacao() {
        for arc in [false, true] {
            for cheia in [false, true] {
                for forma in [Elemento::Geral, Elemento::Int, Elemento::Double, Elemento::Bool] {
                    for compativel in [false, true] {
                        let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
                        let (lista, valor) = HEAP.with(|h| {
                            let mut heap = h.borrow_mut();
                            if arc { heap.ativar_arc(); }
                            let lista = heap.nova_expansivel(usize::from(cheia), if cheia { 1 } else { 2 }, forma);
                            heap.reter_owner_codigo(lista);
                            if cheia {
                                let dados = heap.lista_dados(lista);
                                heap.palavras_mut(dados)[1] = match forma {
                                    Elemento::Geral => 0, Elemento::Int => i64::MIN,
                                    Elemento::Double => (-0.0_f64).to_bits() as i64, Elemento::Bool => 1,
                                };
                            }
                            let v = if !compativel || forma == Elemento::Geral {
                                Valor::Ref(heap.alocar_instancia(123, 1))
                            } else { match forma {
                                Elemento::Int => Valor::Int(i64::MAX),
                                Elemento::Double => Valor::Double(f64::from_bits(0xfff8_1234_5678_9abc)),
                                Elemento::Bool => Valor::Bool(true), Elemento::Geral => unreachable!(),
                            }};
                            let valor = heap.como_ref(v);
                            heap.reter_owner_codigo(valor);
                            (lista, valor)
                        });
                        let stats = HEAP.with(|h| h.borrow().stats());
                        let r = dartforge_lista_acrescentar(lista, valor);
                        assert_eq!(r, lista);
                        HEAP.with(|h| {
                            let heap = h.borrow();
                            assert_eq!(heap.lista_len(lista), 1 + usize::from(cheia));
                            assert_eq!(heap.lista_capacidade(lista), if cheia { 3 } else { 2 });
                            let general = !compativel || forma == Elemento::Geral;
                            assert_eq!(heap.lista_forma(lista), if general { Elemento::Geral } else { forma });
                            let ultimo = heap.lista_get(lista, usize::from(cheia));
                            match forma {
                                _ if general => assert_eq!(ultimo, Valor::Ref(valor)),
                                Elemento::Int => assert_eq!(ultimo, Valor::Int(i64::MAX)),
                                Elemento::Double => {
                                    let Valor::Double(d) = ultimo else { panic!("elemento double esperado") };
                                    assert_eq!(d.to_bits(), 0xfff8_1234_5678_9abc);
                                },
                                Elemento::Bool => assert_eq!(ultimo, Valor::Bool(true)),
                                Elemento::Geral => unreachable!(),
                            }
                            if cheia || (!compativel && forma != Elemento::Geral) {
                                assert!(heap.stats().allocations > stats.allocations);
                                assert!(heap.stats().collections > stats.collections);
                            } else {
                                assert_eq!(heap.stats().allocations, stats.allocations);
                                assert_eq!(heap.stats().collections, stats.collections);
                            }
                        });
                        dartforge_arc_release(valor);
                        dartforge_arc_collect();
                        if !compativel || forma == Elemento::Geral {
                            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
                        }
                        // O retorno borrowed não pode deixar um owner adicional da lista.
                        dartforge_arc_release(lista);
                        dartforge_arc_collect();
                        assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(r)));
                        if !compativel || forma == Elemento::Geral {
                            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
                        }
                        HEAP.with(|h| h.replace(anterior));
                    }
                }
            }
        }
    }

    #[test]
    fn armazenamento_compacto_preserva_bits_e_sequencia_de_encolhimento() {
        for arc in [false, true] {
            for (forma, bits) in [(Elemento::Int, i64::MIN),
                (Elemento::Double, 0xfff8_1234_5678_9abc_u64 as i64), (Elemento::Bool, 1)] {
                let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
                let lista = HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    if arc { heap.ativar_arc(); }
                    let lista = heap.nova_expansivel(0, 3, forma);
                    heap.reter_owner_codigo(lista);
                    lista
                });
                let stats = HEAP.with(|h| h.borrow().stats());
                let dados = dartforge_arc_lista_armazenamento_ref_v1(lista);
                assert_ne!(dados, lista);
                assert_eq!(dartforge_arc_lista_capacidade_v1(lista), 3);
                assert_eq!(dartforge_arc_lista_len_v1(dados), 3);
                assert_eq!(dartforge_arc_lista_forma_v1(dados), i64::from(forma.flags()));
                dartforge_arc_lista_gravar_bits_v1(dados, 0, bits, i64::from(forma.flags()));
                dartforge_arc_lista_gravar_len_v1(lista, 1);
                HEAP.with(|h| {
                    let heap = h.borrow();
                    assert_eq!(heap.palavras(dados)[1], bits);
                    assert_eq!(heap.stats().allocations, stats.allocations);
                    assert_eq!(heap.stats().collections, stats.collections);
                });
                // Cópia explícita antes de retirar a aresta que sustenta o borrow.
                dartforge_arc_retain(dados);
                let novo = HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    let novo = heap.nova_lista(cid::LIST, 0, forma);
                    heap.reter_owner_codigo(novo);
                    novo
                });
                assert_eq!(dartforge_arc_lista_gravar_dados_v1(lista, novo), 1);
                assert!(HEAP.with(|h| armazenamento_lista_auditado(&h.borrow(), lista).is_err()));
                dartforge_arc_lista_gravar_len_v1(lista, 0);
                assert_eq!(dartforge_arc_lista_len_v1(lista), 0);
                dartforge_arc_release(novo);
                dartforge_arc_collect();
                assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(dados)));
                dartforge_arc_release(dados);
                dartforge_arc_collect();
                assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(dados)));
                assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(novo)));
                dartforge_arc_release(lista);
                dartforge_arc_collect();
                assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(novo)));
                HEAP.with(|h| h.replace(anterior));
            }
        }
    }

    #[test]
    fn armazenamento_ref_copia_arestas_e_preserva_owner_do_elemento() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            let lista = HEAP.with(|h| {
                let mut heap = h.borrow_mut();
                if arc { heap.ativar_arc(); }
                let lista = heap.nova_expansivel(1, 3, Elemento::Geral);
                heap.reter_owner_codigo(lista);
                lista
            });
            let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
            let dados = dartforge_arc_lista_armazenamento_ref_v1(lista);
            let stats = HEAP.with(|h| h.borrow().stats());
            dartforge_arc_lista_gravar_ref_v1(dados, 0, filho);
            assert_eq!(HEAP.with(|h| h.borrow().stats().allocations), stats.allocations);
            assert_eq!(HEAP.with(|h| h.borrow().stats().collections), stats.collections);
            dartforge_arc_release(filho);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(filho)));
            let recebido = dartforge_arc_lista_ref_owned_v1(dados, 0);
            dartforge_arc_lista_gravar_ref_v1(dados, 0, 0);
            dartforge_arc_release(lista);
            dartforge_arc_collect();
            assert_eq!(dartforge_unbox_int(recebido), i64::MAX);
            dartforge_arc_release(recebido);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(filho)));
            HEAP.with(|h| h.replace(anterior));
        }
    }

    #[test]
    fn escritas_recusam_forma_indice_e_len_invalidos_e_nao_trocam_forma() {
        let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
        let (lista, novo) = HEAP.with(|h| {
            let mut heap = h.borrow_mut();
            heap.ativar_arc();
            let lista = heap.nova_expansivel(1, 2, Elemento::Bool);
            heap.reter_owner_codigo(lista);
            let novo = heap.nova_lista(cid::LIST, 1, Elemento::Int);
            heap.reter_owner_codigo(novo);
            (lista, novo)
        });
        let dados = dartforge_arc_lista_armazenamento_ref_v1(lista);
        assert_eq!(dartforge_arc_lista_gravar_dados_v1(lista, novo), 0);
        assert_eq!(dartforge_arc_lista_armazenamento_ref_v1(lista), dados);
        HEAP.with(|h| {
            let heap = h.borrow();
            for (i, bits, forma) in [(-1, 1, 0x62), (2, 1, 0x62), (0, 2, 0x62),
                (0, 1, 0x22), (0, 1, 0x04)] {
                assert!(conferir_bits_lista_arc(&heap, dados, i, bits, forma).is_err());
            }
            assert!(conferir_bits_lista_arc(&heap, lista, 0, 1, 0x62).is_err());
            assert!(conferir_len_lista_arc(&heap, dados, 0).is_err());
            for n in [-1, 3] { assert!(conferir_len_lista_arc(&heap, lista, n).is_err()); }
            assert_eq!(heap.palavras(dados)[1], 0);
        });
        dartforge_arc_release(novo);
        dartforge_arc_release(lista);
        dartforge_arc_collect();
        HEAP.with(|h| h.replace(anterior));
    }

    #[test]
    fn comprimento_geral_e_compacto_nao_aloca_coleta_ou_retem() {
        for arc in [false, true] {
            for classe in [cid::LIST, cid::IMMUTABLE_LIST, cid::GROWABLE_LIST] {
                for forma in [Elemento::Geral, Elemento::Int, Elemento::Double, Elemento::Bool] {
                    let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
                    let lista = HEAP.with(|h| {
                        let mut heap = h.borrow_mut();
                        if arc { heap.ativar_arc(); }
                        let lista = if classe == cid::GROWABLE_LIST {
                            heap.nova_expansivel(2, 5, forma)
                        } else { heap.nova_lista(classe, 2, forma) };
                        heap.reter_owner_codigo(lista);
                        lista
                    });
                    let stats = HEAP.with(|h| h.borrow().stats().clone());
                    assert_eq!(dartforge_arc_lista_len_v1(lista), 2);
                    HEAP.with(|h| {
                        let heap = h.borrow();
                        assert_eq!(heap.stats().allocations, stats.allocations);
                        assert_eq!(heap.stats().collections, stats.collections);
                    });
                    dartforge_arc_release(lista);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observar_heap_v1(lista), 0);
                    assert!(HEAP.with(|h| armazenamento_lista_auditado(&h.borrow(), lista).is_err()));
                    HEAP.with(|h| h.replace(anterior));
                }
            }
        }
    }

    #[test]
    fn comprimento_recusa_mapa_classe_e_limites_invalidos() {
        let mut heap = Heap::new(true);
        let outra = heap.alocar_instancia(123, 2);
        heap.reter_owner_codigo(outra);
        for receiver in [0, smi::de(3).unwrap(), i64::MAX, outra] {
            assert!(armazenamento_lista_auditado(&heap, receiver).is_err());
        }
        let fixa = heap.nova_lista(cid::LIST, 2, Elemento::Int);
        heap.reter_owner_codigo(fixa);
        for n in [-1, 3, i64::MAX] {
            heap.palavras_mut(fixa)[0] = n;
            assert!(armazenamento_lista_auditado(&heap, fixa).is_err());
        }
        heap.palavras_mut(fixa)[0] = 2;
        let lista = heap.nova_expansivel(0, 3, Elemento::Double);
        heap.reter_owner_codigo(lista);
        assert_eq!(armazenamento_lista_auditado(&heap, lista).unwrap().1, 0);
        for n in [-1, 4] {
            heap.definir_campo(lista, 0, n, false);
            assert!(armazenamento_lista_auditado(&heap, lista).is_err());
        }
        heap.definir_campo(lista, 0, 0, true);
        assert!(armazenamento_lista_auditado(&heap, lista).is_err());
        heap.definir_campo(lista, 0, 0, false);
        for (dados, marca) in [(fixa, false), (outra, true), (lista, true), (0, true)] {
            heap.definir_campo(lista, 1, dados, marca);
            assert!(armazenamento_lista_auditado(&heap, lista).is_err());
        }
        let falsa = heap.alocar_instancia(cid::GROWABLE_LIST, 1);
        assert!(armazenamento_lista_auditado(&heap, falsa).is_err());
    }

    #[test]
    fn elemento_owned_sobrevive_a_slot_limpo_e_lista_liberada() {
        for arc in [false, true] {
            for classe in [cid::LIST, cid::IMMUTABLE_LIST, cid::GROWABLE_LIST] {
                let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
                if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
                let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
                let lista = HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    let lista = if classe == cid::GROWABLE_LIST { heap.nova_expansivel(3, 5, Elemento::Geral) }
                        else { heap.nova_lista(classe, 3, Elemento::Geral) };
                    heap.reter_owner_codigo(lista);
                    heap.lista_set(lista, 1, Valor::Ref(filho));
                    heap.lista_set(lista, 2, Valor::Ref(smi::de(7).unwrap()));
                    lista
                });
                dartforge_arc_release(filho);
                let stats = HEAP.with(|h| h.borrow().stats());
                let recebido = dartforge_arc_lista_ref_owned_v1(lista, 1);
                assert_eq!(recebido, filho);
                assert_eq!(dartforge_arc_lista_ref_owned_v1(lista, 0), 0);
                assert_eq!(dartforge_arc_lista_ref_owned_v1(lista, 2), smi::de(7).unwrap());
                HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    assert_eq!(heap.stats().allocations, stats.allocations);
                    assert_eq!(heap.stats().collections, stats.collections);
                    heap.lista_set(lista, 1, Valor::Ref(0));
                });
                dartforge_arc_release(lista);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(lista), 0);
                assert_eq!(dartforge_arc_observar_heap_v1(recebido), 3);
                assert_eq!(dartforge_unbox_int(recebido), i64::MAX);
                dartforge_arc_release(recebido);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(recebido), 0);
                HEAP.with(|h| h.replace(anterior));
            }
        }
    }

    #[test]
    fn leitura_recusa_compactos_indices_e_layouts_falsos() {
        let mut heap = Heap::new(true);
        for forma in [Elemento::Geral, Elemento::Int, Elemento::Double, Elemento::Bool] {
            let lista = heap.nova_lista(cid::LIST, 2, forma);
            for indice in [-1, 2, i64::MAX] { assert!(ler_elemento_ref_auditado(&heap, lista, indice).is_err()); }
            assert_eq!(ler_elemento_ref_auditado(&heap, lista, 0).is_ok(), forma == Elemento::Geral);
        }
        for receiver in [0, smi::de(3).unwrap(), i64::MAX] {
            assert!(ler_elemento_ref_auditado(&heap, receiver, 0).is_err());
        }
        let falsa = heap.alocar_instancia(cid::GROWABLE_LIST, 2);
        heap.definir_campo(falsa, 0, 1, false);
        heap.definir_campo(falsa, 1, 0, false);
        assert!(ler_elemento_ref_auditado(&heap, falsa, 0).is_err());
        let lista = heap.nova_expansivel(1, 3, Elemento::Geral);
        assert!(ler_elemento_ref_auditado(&heap, lista, 1).is_err());
        heap.definir_campo(lista, 0, 4, false);
        assert!(ler_elemento_ref_auditado(&heap, lista, 0).is_err());
    }
}

fn indice_argumento_arc(posicionais: i64, nomeados: i64, indice: i64) -> Result<usize, &'static str> {
    let total = posicionais.checked_add(nomeados).filter(|_| posicionais >= 0 && nomeados >= 0)
        .and_then(|n| usize::try_from(n).ok()).filter(|n| *n <= isize::MAX as usize / std::mem::size_of::<i64>())
        .ok_or("contagens inválidas do descritor")?;
    usize::try_from(indice).ok().filter(|i| *i < total).ok_or("índice fora dos argumentos Ref")
}

/// Lê um argumento Ref da ABI uniforme e entrega um owner independente.
///
/// Confere o índice contra a soma dos posicionais e nomeados: o slot RTI
/// posterior não é uma Ref. Retém antes de devolver, sem alocar, coletar,
/// executar Dart ou alterar os vetores. O chamador deve consumir o token.
///
/// # Safety
/// `descritor` deve apontar para ao menos dois i64 alinhados e imutáveis.
/// `argumentos` deve conter ao menos a quantidade declarada de Ref válidos,
/// alinhados e sustentados pelo chamador, sem mutação durante esta chamada.
/// A conferência do índice não prova a extensão real de ponteiros externos.
///
/// # Panics
/// Ponteiro nulo, contagem/índice inválido ou falha interna de retenção;
/// aborta na fronteira C, sem exceção Dart.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let objeto = dartforge_arc_objeto_owned_v1(123, 0);
/// let argumentos = [objeto];
/// let descritor = [1, 0];
/// // SAFETY: vetores vivos e imutáveis; objeto sustentado pelo chamador.
/// let recebido = unsafe { dartforge_arc_argumento_owned_v1(argumentos.as_ptr(), descritor.as_ptr(), 0) };
/// dartforge_arc_release(objeto);
/// assert_eq!(dartforge_arc_observar_heap_v1(recebido), 3);
/// dartforge_arc_release(recebido);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_argumento_owned_v1(argumentos: *const i64, descritor: *const i64, indice: i64) -> i64 {
    assert!(!descritor.is_null() && !argumentos.is_null(), "vetores ABI ARC nulos");
    // SAFETY: dois elementos válidos conforme o contrato da ABI.
    let indice = indice_argumento_arc(unsafe { *descritor }, unsafe { *descritor.add(1) }, indice)
        .expect("argumento ARC fora do descritor");
    // SAFETY: índice conferido e extensão garantida pelo chamador.
    let valor = unsafe { *argumentos.add(indice) };
    HEAP.with(|heap| heap.borrow_mut().reter_owner_codigo(valor));
    valor
}

#[cfg(test)]
mod testes_argumento_owned {
    use super::*;

    #[test]
    #[allow(unsafe_code)]
    fn argumentos_ref_conferem_limites_e_sobrevivem_ao_owner_do_chamador() {
        for (pos, nom, i) in [(-1, 0, 0), (0, -1, 0), (1, 0, -1), (1, 0, 1), (0, 0, 0), (i64::MAX, 1, 0)] {
            assert!(indice_argumento_arc(pos, nom, i).is_err());
        }
        assert_eq!(indice_argumento_arc(1, 2, 2), Ok(2));
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(crate::heap::Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let objeto = dartforge_arc_objeto_owned_v1(32001, 0);
            let argumentos = [0, objeto, crate::layout::smi::de(7).unwrap(), i64::MAX];
            let descritor = [1, 2, 101, 102];
            let stats = HEAP.with(|h| h.borrow().stats());
            // SAFETY: três argumentos válidos seguidos de RTI que não será lido.
            let recebido = unsafe { dartforge_arc_argumento_owned_v1(argumentos.as_ptr(), descritor.as_ptr(), 1) };
            assert_eq!(recebido, objeto);
            dartforge_arc_release(objeto);
            assert_eq!(dartforge_arc_observar_heap_v1(recebido), 3);
            HEAP.with(|h| { assert_eq!(h.borrow().stats().allocations, stats.allocations);
                assert_eq!(h.borrow().stats().collections, stats.collections); });
            dartforge_arc_release(recebido);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(recebido), 0);
            HEAP.with(|h| h.replace(anterior));
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
/// let objeto = dartforge_arc_objeto_owned_v1(123, 40);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(objeto), 3);
/// assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, 39), 0);
/// dartforge_arc_release(objeto);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_objeto_owned_v1(classe: i64, campos: i64) -> i64 {
    let (classe, campos) = parametros_instancia_owned(classe, campos)
        .expect("contrato de alocação de instância ARC violado");
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let objeto = heap.alocar_instancia(classe, campos);
        heap.reter_owner_codigo(objeto);
        heap.reabastecer_tlab(crate::layout::palavras_de_instancia(campos));
        objeto
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
/// extern "C" fn entrada(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
/// let closure = dartforge_arc_closure_owned_v1(entrada as usize as i64, 0, 0, 0);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(closure), 3);
/// dartforge_arc_release(closure);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_closure_owned_v1(codigo: i64, contexto: i64, tipado: i64, abi: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let closure = heap.nova_closure(codigo, (contexto, true), tipado, abi);
        heap.reter_owner_codigo(closure);
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
/// extern "C" fn entrada(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
/// let closure = dartforge_arc_closure_owned_v1(entrada as usize as i64, 0, 0, 0);
/// assert_eq!(dartforge_arc_contexto_closure_v1(closure), 0);
/// dartforge_arc_release(closure);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_contexto_closure_v1(closure: i64) -> i64 {
    HEAP.with(|heap| contexto_closure_auditado(&heap.borrow(), closure)
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
pub extern "C" fn dartforge_arc_env_ref_v1(env: i64, indice: i64) -> i64 {
    HEAP.with(|heap| env_ref_auditado(&heap.borrow(), env, indice)
        .expect("contrato de captura Ref ARC violado"))
}

fn env_ref_auditado(heap: &Heap, env: i64, indice: i64) -> Result<i64, &'static str> {
    let objeto = heap.objeto(env).ok_or("receiver não é ambiente vivo")?;
    if objeto.class_id != i64::from(crate::layout::cid::CONTEXTO) {
        return Err("receiver não é ambiente");
    }
    let indice = usize::try_from(indice).map_err(|_| "índice negativo")?;
    let (bits, referencia) = objeto.get(indice).ok_or("índice fora do ambiente")?;
    if !referencia { return Err("captura é escalar"); }
    Ok(bits)
}

fn contexto_closure_auditado(heap: &Heap, closure: i64) -> Result<i64, &'static str> {
    let contexto = heap.closure(closure).ok_or("receiver não é closure")?.contexto;
    if !contexto.1 && contexto.0 != 0 { return Err("contexto é escalar não nulo"); }
    Ok(contexto.0)
}

#[cfg(test)]
mod testes_contexto_closure {
    use super::*;

    #[test]
    fn ambiente_confere_classe_limites_e_mapa_estendido_sem_coletar() {
        let mut heap = Heap::new(false);
        let filho = heap.novo_objeto(32000, &[]);
        let mut campos = vec![(0, false); 40];
        campos[0] = (0, true);
        campos[1] = (smi::de(42).unwrap(), true);
        campos[38] = (filho, false);
        campos[39] = (filho, true);
        let env = heap.novo_objeto(i64::from(crate::layout::cid::CONTEXTO), &campos);
        let falso = heap.novo_objeto(i64::from(crate::layout::cid::CELULA), &campos);
        let stats = heap.stats();
        for (indice, esperado) in [(0, 0), (1, campos[1].0), (39, filho)] {
            assert_eq!(env_ref_auditado(&heap, env, indice), Ok(esperado));
        }
        for (receiver, indice) in [(env, -1), (env, 40), (env, 38), (env, 37),
            (0, 0), (falso, 39), (filho, 0)] {
            assert!(env_ref_auditado(&heap, receiver, indice).is_err());
        }
        assert_eq!(heap.stats().allocations, stats.allocations);
        assert_eq!(heap.stats().collections, stats.collections);
    }

    #[test]
    fn contexto_confere_classe_e_mapa_sem_confundir_bits_com_ref() {
        let mut heap = Heap::new(false);
        for campo in [(0, false), (0, true), (42, false)] {
            let closure = heap.nova_closure(123, campo, 0, 0);
            assert_eq!(contexto_closure_auditado(&heap, closure).is_ok(), campo.0 == 0);
        }
        let filho = heap.novo_objeto(32000, &[]);
        let closure = heap.nova_closure(123, (filho, true), 0, 0);
        assert_eq!(contexto_closure_auditado(&heap, closure), Ok(filho));
        let falsa = heap.novo_objeto(32001, &[(123, false), (filho, true), (0, false), (0, false)]);
        for invalida in [0, falsa] { assert!(contexto_closure_auditado(&heap, invalida).is_err()); }
    }
}

fn parametros_instancia_owned(classe: i64, campos: i64) -> Result<(i32, usize), &'static str> {
    let classe = i32::try_from(classe).map_err(|_| "classe fora de i32")?;
    let campos = u16::try_from(campos).map_err(|_| "quantidade de campos fora de u16")?;
    Ok((classe, usize::from(campos)))
}

/// Registra a tabela estática de métodos e aloca uma instância zerada Owned.
///
/// A função `tabela` é o thunk estático do emissor: devolve uma tabela válida
/// `{cid, n, [hash, entrada]…}`, sem executar Dart, coletar ou lançar. A memória
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
///     static TABELA: [i64; 2] = [32000, 0];
///     TABELA.as_ptr()
/// }
/// // SAFETY: thunk puro, tabela estática válida e sem entradas.
/// let objeto = unsafe { dartforge_arc_objeto_owned_t_v1(32000, 0, tabela) };
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(objeto), 3);
/// dartforge_arc_release(objeto);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_objeto_owned_t_v1(classe: i64, campos: i64, tabela: extern "C" fn() -> *const i64) -> i64 {
    parametros_instancia_owned(classe, campos).expect("contrato de alocação de instância ARC violado");
    dartforge_registrar_tabela(classe, tabela);
    dartforge_arc_objeto_owned_v1(classe, campos)
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
/// let objeto = dartforge_arc_objeto_owned_v1(123, 40);
/// dartforge_arc_gravar_campo_escalar_v1(objeto, 39, i64::MAX);
/// assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, 39), i64::MAX);
/// dartforge_arc_release(objeto);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_gravar_campo_escalar_v1(objeto: i64, indice: i64, bits: i64) {
    HEAP.with(|heap| gravar_campo_auditado(&mut heap.borrow_mut(), objeto, indice, bits, false)
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
/// let objeto = dartforge_arc_objeto_owned_v1(123, 40);
/// let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_gravar_campo_ref_v1(objeto, 39, filho);
/// dartforge_arc_release(filho);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(filho), 1);
/// dartforge_arc_gravar_campo_ref_v1(objeto, 39, 0);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(filho), 0);
/// dartforge_arc_release(objeto);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_gravar_campo_ref_v1(objeto: i64, indice: i64, valor: i64) {
    HEAP.with(|heap| gravar_campo_auditado(&mut heap.borrow_mut(), objeto, indice, valor, true)
        .expect("contrato de gravação Ref ARC violado"));
}

/// Grava uma aresta Ref somente após conferir o layout físico do receiver.
///
/// `layouts` contém pares ordenados `(cid, quantidade_de_campos)` de classes
/// cujo campo `indice` foi declarado Ref pelo compilador. A conferência usa
/// a classe e o tamanho reais, inclusive na primeira escrita do campo zerado.
/// Não consome owners, aloca, coleta ou executa Dart; invalida empréstimos
/// da aresta anterior. Guardas Dart de tipo/late continuam no lowering.
///
/// # Safety
/// O vetor deve estar alinhado e conter `pares * 2` palavras legíveis e
/// imutáveis durante a chamada. Cada par deve corresponder ao layout atual
/// da imagem, com `indice` Ref. A função não guarda nem devolve o endereço.
/// Vida/versões da imagem são obrigação do chamador.
///
/// # Panics
/// Receiver, índice, classe ou tamanho fora dos layouts; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let layouts = [123, 40];
/// let objeto = dartforge_arc_objeto_owned_v1(123, 40);
/// let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
/// // SAFETY: vetor vivo e imutável, campo 39 declarado Ref nesta imagem.
/// unsafe { dartforge_arc_gravar_campo_ref_conferido_v1(objeto, 39, filho, layouts.as_ptr(), 1); }
/// dartforge_arc_release(filho);
/// assert_eq!(dartforge_arc_ler_campo_ref_v1(objeto, 39), filho);
/// dartforge_arc_release(objeto);
/// ```
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_arc_gravar_campo_ref_conferido_v1(
    objeto: i64, indice: i64, valor: i64, layouts: *const i64, pares: i64,
) {
    let palavras = usize::try_from(pares).ok().and_then(|n| n.checked_mul(2))
        .filter(|n| *n > 0 && *n <= isize::MAX as usize / std::mem::size_of::<i64>())
        .expect("vetor de layouts ARC inválido");
    assert!(!layouts.is_null() && layouts.is_aligned(), "endereço dos layouts ARC inválido");
    // SAFETY: o produtor garante a extensão, imutabilidade e vida do vetor.
    let layouts = unsafe { std::slice::from_raw_parts(layouts, palavras) };
    HEAP.with(|heap| gravar_campo_ref_conferido(&mut heap.borrow_mut(), objeto, indice, valor, layouts)
        .expect("contrato de gravação Ref conferida ARC violado"));
}

fn gravar_campo_ref_conferido(
    heap: &mut Heap, objeto: i64, indice: i64, valor: i64, layouts: &[i64],
) -> Result<(), &'static str> {
    conferir_layout_fisico(heap, objeto, indice, layouts)?;
    gravar_campo_auditado(heap, objeto, indice, valor, true)
}

fn conferir_layout_fisico(heap: &Heap, objeto: i64, indice: i64, layouts: &[i64]) -> Result<(), &'static str> {
    let vista = heap.objeto(objeto).ok_or("receiver não é objeto vivo")?;
    let i = usize::try_from(indice).map_err(|_| "índice negativo")?;
    if i >= vista.len() { return Err("índice fora do objeto"); }
    if layouts.len() % 2 != 0 || !layouts.chunks_exact(2).any(|p|
        p[0] == vista.class_id && usize::try_from(p[1]).ok() == Some(vista.len())) {
        return Err("classe/tamanho fora dos layouts Ref");
    }
    Ok(())
}

/// Grava bits escalares após conferir classe e tamanho contra layouts declarados.
///
/// `representacao` é 1=int, 2=double, 3=bool, 4=byte. Os bits já devem estar
/// convertidos pelo lowering; não faz coerções, aloca, coleta ou executa Dart.
/// Mantém a barreira e invalida borrows anteriores ao substituir o campo.
///
/// # Safety
/// `layouts` deve apontar para `pares * 2` palavras alinhadas, imutáveis e
/// legíveis durante a chamada. Os pares `(cid, campos)` devem declarar o
/// índice com a representação indicada na versão atual da imagem.
/// Não guarda nem devolve esse endereço; versões/pins são dever do chamador.
///
/// # Panics
/// Representação, receiver, índice, classe ou tamanho inválido; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let layouts = [123, 1];
/// let objeto = dartforge_arc_objeto_owned_v1(123, 1);
/// // SAFETY: vetor vivo; o campo 0 é int no layout atual da imagem.
/// unsafe { dartforge_arc_gravar_campo_escalar_conferido_v1(objeto, 0, 42, layouts.as_ptr(), 1, 1); }
/// assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, 0), 42);
/// dartforge_arc_release(objeto);
/// ```
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_arc_gravar_campo_escalar_conferido_v1(
    objeto: i64, indice: i64, bits: i64, layouts: *const i64, pares: i64, representacao: i64,
) {
    assert!((1..=4).contains(&representacao), "representação escalar ARC inválida");
    let palavras = usize::try_from(pares).ok().and_then(|n| n.checked_mul(2))
        .filter(|n| *n > 0 && *n <= isize::MAX as usize / std::mem::size_of::<i64>())
        .expect("vetor de layouts ARC inválido");
    assert!(!layouts.is_null() && layouts.is_aligned(), "endereço dos layouts ARC inválido");
    // SAFETY: vetor e extensão garantidos pelo produtor.
    let layouts = unsafe { std::slice::from_raw_parts(layouts, palavras) };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        conferir_layout_fisico(&heap, objeto, indice, layouts).expect("layout escalar ARC incompatível");
        gravar_campo_auditado(&mut heap, objeto, indice, bits, false)
            .expect("contrato de gravação escalar conferida ARC violado");
    });
}

/// Lê os bits escalares após conferir o layout físico e o mapa real do campo.
///
/// A representação (1=int, 2=double, 3=bool, 4=byte) e as conversões são
/// obrigações do lowering. Não cria owners, aloca, coleta ou executa Dart.
///
/// # Safety
/// O vetor `(cid, campos)` deve conter `pares * 2` palavras alinhadas,
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
/// let objeto = dartforge_arc_objeto_owned_v1(123, 1);
/// dartforge_arc_gravar_campo_escalar_v1(objeto, 0, 42);
/// // SAFETY: vetor vivo; o campo 0 é int no layout atual da imagem.
/// let bits = unsafe { dartforge_arc_ler_campo_escalar_conferido_v1(objeto, 0, layouts.as_ptr(), 1, 1) };
/// assert_eq!(bits, 42);
/// dartforge_arc_release(objeto);
/// ```
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_arc_ler_campo_escalar_conferido_v1(
    objeto: i64, indice: i64, layouts: *const i64, pares: i64, representacao: i64,
) -> i64 {
    assert!((1..=4).contains(&representacao), "representação escalar ARC inválida");
    let palavras = usize::try_from(pares).ok().and_then(|n| n.checked_mul(2))
        .filter(|n| *n > 0 && *n <= isize::MAX as usize / std::mem::size_of::<i64>())
        .expect("vetor de layouts ARC inválido");
    assert!(!layouts.is_null() && layouts.is_aligned(), "endereço dos layouts ARC inválido");
    // SAFETY: vetor e extensão garantidos pelo produtor.
    let layouts = unsafe { std::slice::from_raw_parts(layouts, palavras) };
    HEAP.with(|heap| {
        let heap = heap.borrow();
        conferir_layout_fisico(&heap, objeto, indice, layouts).expect("layout escalar ARC incompatível");
        ler_campo_escalar_auditado(&heap, objeto, indice)
            .expect("contrato de leitura escalar conferida ARC violado")
    })
}

fn gravar_campo_auditado(heap: &mut Heap, objeto: i64, indice: i64, bits: i64, referencia: bool) -> Result<(), &'static str> {
    let vista = heap.objeto(objeto).ok_or("receiver não é objeto vivo")?;
    let indice = usize::try_from(indice).map_err(|_| "índice negativo")?;
    if indice >= vista.len() { return Err("índice fora do objeto"); }
    heap.definir_campo(objeto, indice, bits, referencia);
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
/// let objeto = dartforge_object_new(123, 1);
/// dartforge_arc_retain(objeto);
/// dartforge_object_set(objeto, 0, i64::MAX, 0);
/// assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, 0), i64::MAX);
/// dartforge_arc_release(objeto);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ler_campo_escalar_v1(objeto: i64, indice: i64) -> i64 {
    HEAP.with(|heap| ler_campo_escalar_auditado(&heap.borrow(), objeto, indice)
        .expect("contrato de leitura escalar ARC violado"))
}

fn ler_campo_escalar_auditado(heap: &Heap, objeto: i64, indice: i64) -> Result<i64, &'static str> {
    let objeto = heap.objeto(objeto).ok_or("receiver não é objeto vivo")?;
    let indice = usize::try_from(indice).map_err(|_| "índice negativo")?;
    let (bits, referencia) = objeto.get(indice).ok_or("índice fora do objeto")?;
    if referencia { return Err("campo é referência gerenciada"); }
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
/// let objeto = dartforge_arc_objeto_owned_v1(123, 40);
/// let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_gravar_campo_ref_v1(objeto, 39, filho);
/// dartforge_arc_release(filho);
/// let emprestado = dartforge_arc_ler_campo_ref_v1(objeto, 39);
/// assert_eq!(emprestado, filho);
/// dartforge_arc_retain(emprestado); // Copia o empréstimo antes da substituição.
/// dartforge_arc_gravar_campo_ref_v1(objeto, 39, 0);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(emprestado), 3);
/// dartforge_arc_release(emprestado);
/// dartforge_arc_release(objeto);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ler_campo_ref_v1(objeto: i64, indice: i64) -> i64 {
    HEAP.with(|heap| ler_campo_ref_auditado(&heap.borrow(), objeto, indice)
        .expect("contrato de leitura Ref ARC violado"))
}

fn ler_campo_ref_auditado(heap: &Heap, objeto: i64, indice: i64) -> Result<i64, &'static str> {
    let objeto = heap.objeto(objeto).ok_or("receiver não é objeto vivo")?;
    let indice = usize::try_from(indice).map_err(|_| "índice negativo")?;
    let (bits, referencia) = objeto.get(indice).ok_or("índice fora do objeto")?;
    if !referencia { return Err("campo é escalar"); }
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
/// let objeto = dartforge_arc_objeto_owned_v1(123, 1);
/// assert_eq!(dartforge_arc_ler_campo_late_ref_v1(objeto, 0), 0);
/// dartforge_arc_gravar_campo_ref_v1(objeto, 0, 0);
/// dartforge_late_field_mark_initialized(objeto, 0);
/// assert_eq!(dartforge_arc_ler_campo_late_ref_v1(objeto, 0), 0);
/// dartforge_arc_release(objeto);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ler_campo_late_ref_v1(objeto: i64, indice: i64) -> i64 {
    HEAP.with(|heap| ler_campo_late_ref_auditado(&heap.borrow(), objeto, indice)
        .expect("contrato de leitura late Ref ARC violado"))
}

fn ler_campo_late_ref_auditado(heap: &Heap, objeto: i64, indice: i64) -> Result<i64, &'static str> {
    let vista = heap.objeto(objeto).ok_or("receiver não é objeto vivo")?;
    let index = usize::try_from(indice).map_err(|_| "índice negativo")?;
    let (bits, referencia) = vista.get(index).ok_or("índice fora do objeto")?;
    if referencia || (bits == 0 && !heap.late_inicializado((objeto, indice))) {
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
/// let valor = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_lancar_ref_v1(valor);
/// assert_ne!(dartforge_exception_pending(), 0);
/// dartforge_arc_release(valor);
/// dartforge_exception_clear();
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lancar_ref_v1(valor: i64) {
    dartforge_exception_throw(valor, 3);
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
/// let capturada = dartforge_arc_excecao_owned_v1();
/// assert_eq!(capturada, original);
/// dartforge_arc_release(original);
/// dartforge_exception_clear();
/// dartforge_arc_collect();
/// dartforge_arc_release(capturada);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_excecao_owned_v1() -> i64 {
    let Some(valor) = EXCEPTION.with(|slot| *slot.borrow()) else { return 0; };
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let referencia = heap.como_ref(valor);
        heap.reter_owner_codigo(referencia);
        referencia
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
/// let rastro = dartforge_arc_rastro_owned_v1();
/// dartforge_arc_collect();
/// dartforge_arc_release(rastro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_rastro_owned_v1() -> i64 {
    let rastro = dartforge_stack_trace_get();
    HEAP.with(|h| h.borrow_mut().reter_owner_codigo(rastro));
    rastro
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
/// let valor = dartforge_arc_box_int_owned_v1(i64::MAX);
/// let rastro = dartforge_arc_rastro_owned_v1();
/// dartforge_arc_lancar_com_rastro_ref_v1(valor, rastro);
/// let copia = dartforge_arc_rastro_owned_v1();
/// assert_eq!(copia, rastro);
/// dartforge_arc_release(valor);
/// dartforge_arc_release(rastro);
/// dartforge_exception_clear();
/// dartforge_arc_collect();
/// dartforge_arc_release(copia);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lancar_com_rastro_ref_v1(valor: i64, rastro: i64) {
    dartforge_throw_with_stack_trace(valor, 3, rastro);
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
/// let valor = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_release(valor);
/// dartforge_arc_collect();
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_box_int_owned_v1(valor: i64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let caixa = heap.caixa_int(valor);
        heap.reter_owner_codigo(caixa);
        caixa
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
/// let valor = dartforge_arc_box_double_owned_v1(-0.0);
/// assert_eq!(dartforge_arc_observar_heap_v1(valor), 3);
/// dartforge_arc_release(valor);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(valor), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_box_double_owned_v1(valor: f64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let caixa = heap.caixa_double(valor);
        heap.reter_owner_codigo(caixa);
        caixa
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
/// let valor = dartforge_arc_box_simd_owned_v1(cid::FLOAT32X4 as i64, 0, 0);
/// assert_eq!(dartforge_arc_observar_heap_v1(valor), 3);
/// dartforge_arc_release(valor);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(valor), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_box_simd_owned_v1(cid: i64, baixo: i64, alto: i64) -> i64 {
    let cid = i32::try_from(cid).expect("classe SIMD fora da representação");
    assert!(matches!(cid, crate::layout::cid::FLOAT32X4 | crate::layout::cid::INT32X4 | crate::layout::cid::FLOAT64X2), "classe SIMD incompatível");
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&baixo.to_ne_bytes());
    bytes[8..].copy_from_slice(&alto.to_ne_bytes());
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let caixa = heap.novo_simd(cid, bytes);
        heap.reter_owner_codigo(caixa);
        caixa
    })
}

#[cfg(test)]
mod testes_caixas_simd_owned {
    use super::*;

    #[test]
    fn pistas_mantem_bits_e_owner_protege_ate_release_nos_dois_modos() {
        for arc in [false, true] {
            for cid in [crate::layout::cid::FLOAT32X4, crate::layout::cid::INT32X4,
                crate::layout::cid::FLOAT64X2] {
                HEAP.with(|h| h.replace(Heap::new(true)));
                if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
                for (baixo, alto) in [(0i64, 0i64), (i64::MIN, 0x7ff8_0000_0000_0123),
                    (-1, i64::MAX), (0x7fc0_0123_8000_0000, 0x0000_0001_7f80_0000)] {
                    let h = dartforge_arc_box_simd_owned_v1(cid as i64, baixo, alto);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observar_heap_v1(h), 3);
                    HEAP.with(|heap| {
                        let heap = heap.borrow();
                        assert_eq!(heap.cabecalho(h).class_id, cid);
                        let b = heap.simd(h).unwrap();
                        assert_eq!(&b[..8], &baixo.to_ne_bytes());
                        assert_eq!(&b[8..], &alto.to_ne_bytes());
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
/// let endereco = dartforge_arc_box_int_owned_v1(i64::MAX);
/// assert_eq!(dartforge_arc_observar_heap_v1(endereco), 3);
/// dartforge_arc_release(endereco);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(endereco), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_observar_heap_v1(endereco: i64) -> u8 {
    HEAP.with(|h| h.borrow().observar_owner_codigo(endereco))
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
pub extern "C" fn dartforge_arc_retain(valor: i64) {
    HEAP.with(|h| h.borrow_mut().reter_owner_codigo(valor));
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
pub extern "C" fn dartforge_arc_release(valor: i64) {
    HEAP.with(|h| h.borrow_mut().soltar_owner_codigo(valor));
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
pub extern "C" fn dartforge_arc_verificar_abi(versao: i64) -> i8 {
    i8::from(versao == 1 && HEAP.with(|h| h.borrow().arc_ativo()))
}

/// Abre um quadro proprietário com slots inicialmente nulos.
///
/// # Panics
/// Quantidade negativa/incompatível com usize ou falha interna; o limite C
/// aborta, sem desenrolar para o código gerado.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_quadro_abrir_v1, dartforge_arc_quadro_fechar_v1};
/// let quadro = dartforge_arc_quadro_abrir_v1(0);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_abrir_v1(slots: i64) -> i64 {
    let slots = usize::try_from(slots).expect("quantidade inválida de slots ARC");
    HEAP.with(|h| h.borrow_mut().push_frame_proprietario(slots))
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
/// let quadro = dartforge_arc_quadro_abrir_v1(1);
/// dartforge_arc_quadro_copiar_v1(quadro, 0, 0);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_copiar_v1(quadro: i64, slot: i64, valor: i64) {
    let slot = usize::try_from(slot).expect("índice inválido de slot ARC");
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.conferir_quadro_proprietario(quadro);
        h.set_root(quadro, slot, valor);
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
/// let quadro = dartforge_arc_quadro_abrir_v1(1);
/// let valor = dartforge_arc_quadro_carregar_v1(quadro, 0);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// dartforge_arc_release(valor);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_carregar_v1(quadro: i64, slot: i64) -> i64 {
    let slot = usize::try_from(slot).expect("índice inválido de slot ARC");
    HEAP.with(|h| h.borrow_mut().copiar_raiz_para_codigo(quadro, slot))
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
/// let quadro = dartforge_arc_quadro_abrir_v1(1);
/// dartforge_arc_quadro_receber_v1(quadro, 0, 0);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_receber_v1(quadro: i64, slot: i64, valor: i64) {
    let slot = usize::try_from(slot).expect("índice inválido de slot ARC");
    HEAP.with(|h| h.borrow_mut().mover_codigo_para_raiz(quadro, slot, valor));
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
pub extern "C" fn dartforge_arc_global_receber_v1(id: i64, valor: i64) {
    HEAP.with(|h| h.borrow_mut().mover_codigo_para_global(id, valor));
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
/// let quadro = dartforge_arc_quadro_abrir_v1(2);
/// dartforge_arc_quadro_mover_v1(quadro, 0, quadro, 1);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_mover_v1(
    origem: i64,
    slot_origem: i64,
    destino: i64,
    slot_destino: i64,
) {
    let de = usize::try_from(slot_origem).expect("índice inválido de origem ARC");
    let para = usize::try_from(slot_destino).expect("índice inválido de destino ARC");
    HEAP.with(|h| h.borrow_mut().mover_raiz(origem, de, destino, para));
}

/// Consome os owners do quadro do topo sem coletar ou executar Dart.
///
/// # Panics
/// Quadro não está no topo; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let quadro = dartforge_arc_quadro_abrir_v1(1);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_fechar_v1(quadro: i64) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.conferir_quadro_proprietario(quadro);
        h.pop_frame(quadro);
    });
}

#[cfg(test)]
mod testes_arc_abi_quadros {
    use super::*;

    #[test]
    fn excecao_ref_preserva_mint_apos_release_e_clear_remove_raiz() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let valor = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_lancar_ref_v1(valor);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_arc_release(valor);
            dartforge_arc_collect();
            HEAP.with(|h| assert!(h.borrow().e_objeto_vivo(valor)));
            assert_eq!(dartforge_exception_peek_ref(), valor);
            dartforge_exception_clear();
            assert_eq!(dartforge_exception_pending(), 0);
            dartforge_arc_collect();
            HEAP.with(|h| assert!(!h.borrow().e_objeto_vivo(valor)));
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn captura_owned_preserva_identidade_e_sobrevive_clear_nos_dois_modos() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            dartforge_exception_clear();
            assert_eq!(dartforge_arc_excecao_owned_v1(), 0);
            let original = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_lancar_ref_v1(original);
            let capturada = dartforge_arc_excecao_owned_v1();
            assert_eq!(capturada, original);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_arc_release(original);
            dartforge_exception_clear();
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(capturada)));
            assert_eq!(HEAP.with(|h| h.borrow().int_de(capturada)), Some(i64::MAX));
            dartforge_arc_release(capturada);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(capturada)));
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn rastro_owned_preserva_identidade_e_morre_apos_ultimo_release() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            dartforge_exception_clear();
            let original = dartforge_arc_box_int_owned_v1(i64::MAX);
            let rastro = dartforge_arc_rastro_owned_v1();
            assert!(CURRENT_STACK_TRACE.with(|slot| slot.borrow().is_none()));
            dartforge_arc_lancar_com_rastro_ref_v1(original, rastro);
            let capturado = dartforge_arc_rastro_owned_v1();
            let valor = dartforge_arc_excecao_owned_v1();
            assert_eq!(capturado, rastro);
            assert_eq!(valor, original);
            assert_eq!(CURRENT_STACK_TRACE.with(|slot| *slot.borrow()), Some(rastro));
            dartforge_arc_release(original);
            dartforge_arc_release(rastro);
            dartforge_exception_clear();
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(capturado)));
            assert!(!texto_do_rastro(capturado).unwrap().is_empty());
            assert_eq!(HEAP.with(|h| h.borrow().int_de(valor)), Some(i64::MAX));
            dartforge_arc_release(capturado);
            dartforge_arc_release(valor);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(capturado)));
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn captura_owned_encaixota_escalar_sem_consumir_pendencia() {
        use crate::heap::Valor;
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            for (bits, tag, esperado) in [
                (i64::MAX, 1, Valor::Int(i64::MAX)),
                (42, 1, Valor::Int(42)),
                (3.5_f64.to_bits() as i64, 4, Valor::Double(3.5)),
                (1, 2, Valor::Bool(true)),
                (0, 2, Valor::Bool(false)),
                (0, 3, Valor::Ref(0)),
            ] {
                dartforge_exception_throw(bits, tag);
                let capturada = dartforge_arc_excecao_owned_v1();
                assert_eq!(EXCEPTION.with(|slot| *slot.borrow()), Some(esperado));
                dartforge_exception_clear();
                dartforge_arc_collect();
                assert_eq!(HEAP.with(|h| h.borrow().valor(capturada)), esperado);
                dartforge_arc_release(capturada);
                dartforge_arc_collect();
                if matches!(esperado, Valor::Int(i64::MAX) | Valor::Double(_)) {
                    assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(capturada)));
                }
            }
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn observacao_nativa_nao_cria_owner_e_rejeita_enderecos_fora_do_heap() {
        for validar in [false, true] {
            for arc in [false, true] {
                let anterior = HEAP.with(|h| h.replace(Heap::new(validar)));
                if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
                for endereco in [0, 2, i64::MIN, i64::MAX, smi::de(42).unwrap()] {
                    assert_eq!(dartforge_arc_observar_heap_v1(endereco), 0);
                }
                let endereco = dartforge_arc_box_int_owned_v1(i64::MAX);
                assert_eq!(dartforge_arc_observar_heap_v1(endereco), 3);
                dartforge_arc_retain(endereco);
                dartforge_arc_release(endereco);
                assert_eq!(dartforge_arc_observar_heap_v1(endereco), 3);
                dartforge_arc_release(endereco);
                assert_eq!(dartforge_arc_observar_heap_v1(endereco), 1);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(endereco), 0);
                HEAP.with(|h| { h.replace(anterior); });
            }
        }
    }

    #[test]
    fn fabrica_owned_entrega_token_e_mint_morre_apos_ultimo_release() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let valor = dartforge_arc_box_int_owned_v1(i64::MAX);
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
            assert_eq!(HEAP.with(|h| h.borrow().int_de(valor)), Some(i64::MAX));
            // A cópia representa um retorno retido, independente do token inicial.
            dartforge_arc_retain(valor);
            dartforge_arc_release(valor);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
            dartforge_arc_release(valor);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
            let imediato = dartforge_arc_box_int_owned_v1(42);
            assert_eq!(imediato, smi::de(42).unwrap());
            dartforge_arc_release(imediato);
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn abi_global_recebe_objeto_mortal_e_libera_ultimo_owner() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc {
                HEAP.with(|h| h.borrow_mut().ativar_arc());
            }
            // O endereço representa armazenamento do chamador, sem uma raiz
            // observacional que possa esconder a perda do owner proprietário.
            let mut global = 0_i64;
            let id = (&mut global as *mut i64) as i64;
            let quadro = dartforge_arc_quadro_abrir_v1(1);
            let valor = HEAP.with(|h| h.borrow_mut().alocar_str("global mortal"));
            dartforge_arc_quadro_copiar_v1(quadro, 0, valor);
            global = dartforge_arc_quadro_carregar_v1(quadro, 0);
            dartforge_arc_global_receber_v1(id, global);
            dartforge_arc_quadro_fechar_v1(quadro);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(global)));

            // Reatribuir o mesmo objeto consome o token novo e o owner velho;
            // a identidade não pode pular a transferência ou vazar um retain.
            dartforge_arc_retain(global);
            dartforge_arc_global_receber_v1(id, global);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));

            dartforge_arc_retain(global);
            global = smi::de(42).unwrap();
            dartforge_arc_global_receber_v1(id, global);
            dartforge_arc_collect();
            assert_eq!(global, smi::de(42).unwrap());
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
            dartforge_arc_release(valor);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
            global = 0;
            dartforge_arc_global_receber_v1(id, global);
            HEAP.with(|h| {
                h.replace(anterior);
            });
        }
    }

    #[test]
    fn abi_carrega_owner_independente_e_recebe_token_no_slot() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(false)));
            if arc {
                HEAP.with(|h| h.borrow_mut().ativar_arc());
            }
            let quadro = dartforge_arc_quadro_abrir_v1(1);
            let valor = HEAP.with(|h| h.borrow_mut().alocar_str("slot e SSA"));
            dartforge_arc_quadro_copiar_v1(quadro, 0, valor);
            let token = dartforge_arc_quadro_carregar_v1(quadro, 0);
            assert_eq!(token, valor);
            dartforge_arc_quadro_receber_v1(quadro, 0, token);
            let token = dartforge_arc_quadro_carregar_v1(quadro, 0);
            dartforge_arc_quadro_fechar_v1(quadro);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(token)));
            dartforge_arc_release(token);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(token)));
            let quadro = dartforge_arc_quadro_abrir_v1(1);
            for escalar in [0, smi::de(42).unwrap()] {
                dartforge_arc_quadro_receber_v1(quadro, 0, escalar);
                assert_eq!(dartforge_arc_quadro_carregar_v1(quadro, 0), escalar);
                dartforge_arc_release(escalar);
            }
            dartforge_arc_quadro_fechar_v1(quadro);
            HEAP.with(|h| {
                h.replace(anterior);
            });
        }
    }

    #[test]
    fn abi_minima_confere_modo_e_sustenta_tokens_ate_release() {
        let anterior = HEAP.with(|h| h.replace(Heap::new(false)));
        assert_eq!(dartforge_arc_verificar_abi(1), 0);
        HEAP.with(|h| h.borrow_mut().ativar_arc());
        assert_eq!(dartforge_arc_verificar_abi(1), 1);
        assert_eq!(dartforge_arc_verificar_abi(2), 0);
        let valor = HEAP.with(|h| h.borrow_mut().alocar_str("token ABI"));
        dartforge_arc_retain(valor);
        dartforge_arc_retain(valor);
        dartforge_arc_collect();
        assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
        dartforge_arc_release(valor);
        dartforge_arc_collect();
        assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
        dartforge_arc_release(valor);
        dartforge_arc_collect();
        assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
        HEAP.with(|h| {
            h.replace(anterior);
        });
    }

    #[test]
    #[should_panic(expected = "quadro ARC deve ser proprietário")]
    fn abi_recusa_quadro_observacional_antes_de_alterar_slots() {
        let mut heap = Heap::new(false);
        let quadro = heap.push_frame_with_slots(1);
        heap.conferir_quadro_proprietario(quadro);
    }

    #[test]
    fn copia_movimento_e_fecho_preservam_occorrencias_ate_o_ultimo_owner() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(false)));
            if arc {
                HEAP.with(|h| h.borrow_mut().ativar_arc());
            }
            let origem = dartforge_arc_quadro_abrir_v1(2);
            let destino = dartforge_arc_quadro_abrir_v1(1);
            let valor = HEAP.with(|h| h.borrow_mut().alocar_str("owner ABI"));
            dartforge_arc_quadro_copiar_v1(origem, 0, valor);
            dartforge_arc_quadro_copiar_v1(origem, 1, valor);
            dartforge_arc_quadro_mover_v1(origem, 0, destino, 0);
            dartforge_arc_quadro_mover_v1(destino, 0, destino, 0);
            dartforge_arc_quadro_copiar_v1(origem, 1, 0);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert!(h.e_objeto_vivo(valor));
            });
            dartforge_arc_quadro_fechar_v1(destino);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert!(!h.e_objeto_vivo(valor));
            });
            dartforge_arc_quadro_fechar_v1(origem);
            HEAP.with(|h| {
                h.replace(anterior);
            });
        }
    }
}

#[cfg(test)]
mod testes_caixa_double_owned {
    use super::*;
    #[test]
    fn double_owned_preserva_bits_e_morre_apos_ultimo_token() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            for bits in [(-0.0_f64).to_bits(), 3.25_f64.to_bits(), f64::INFINITY.to_bits(), 0x7ff80000deadbeef] {
                let valor = dartforge_arc_box_double_owned_v1(f64::from_bits(bits));
                dartforge_arc_retain(valor);
                dartforge_arc_release(valor);
                dartforge_arc_collect();
                assert_eq!(HEAP.with(|h| h.borrow().double_de(valor).unwrap().to_bits()), bits);
                assert_eq!(dartforge_arc_observar_heap_v1(valor), 3);
                dartforge_arc_release(valor);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(valor), 0);
            }
            HEAP.with(|h| { h.replace(anterior); });
        }
    }
}

#[cfg(test)]
mod testes_campo_ref {
    use super::*;

    #[test]
    fn escrita_conferida_inicializa_ref_e_recusa_layout_sem_alterar_aresta() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let objeto = dartforge_arc_objeto_owned_v1(123, 40);
            let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
            HEAP.with(|h| {
                let mut heap = h.borrow_mut();
                assert!(gravar_campo_ref_conferido(&mut heap, objeto, 39, filho, &[123, 40]).is_ok());
                for (receiver, indice, layouts) in [
                    (objeto, 39, vec![124, 40]), (objeto, 39, vec![123, 39]),
                    (objeto, -1, vec![123, 40]), (objeto, 40, vec![123, 40]),
                    (0, 39, vec![123, 40]), (filho, 0, vec![123, 40]),
                    (objeto, 39, vec![123]),
                ] {
                    assert!(gravar_campo_ref_conferido(&mut heap, receiver, indice, 0, &layouts).is_err());
                    assert_eq!(ler_campo_ref_auditado(&heap, objeto, 39), Ok(filho));
                }
            });
            dartforge_arc_release(filho);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(filho), 1);
            HEAP.with(|h| assert!(gravar_campo_ref_conferido(&mut h.borrow_mut(), objeto, 39, 0, &[123, 40]).is_ok()));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(filho), 0);
            dartforge_arc_release(objeto);
            HEAP.with(|h| h.replace(anterior));
        }
    }

    #[test]
    fn leitura_empresta_aresta_e_copia_owned_sobrevive_a_substituicao() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let objeto = dartforge_arc_objeto_owned_v1(123, 40);
            let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_gravar_campo_ref_v1(objeto, 0, 0);
            dartforge_arc_gravar_campo_ref_v1(objeto, 1, smi::de(42).unwrap());
            dartforge_arc_gravar_campo_ref_v1(objeto, 39, filho);
            dartforge_arc_gravar_campo_escalar_v1(objeto, 37, filho);
            dartforge_arc_release(filho);
            dartforge_arc_collect();
            let stats = HEAP.with(|h| h.borrow().stats());
            assert_eq!(dartforge_arc_ler_campo_ref_v1(objeto, 0), 0);
            assert_eq!(dartforge_arc_ler_campo_ref_v1(objeto, 1), smi::de(42).unwrap());
            assert_eq!(dartforge_arc_ler_campo_ref_v1(objeto, 39), filho);
            assert_eq!(dartforge_arc_observar_heap_v1(filho), 1);
            HEAP.with(|h| {
                let h = h.borrow();
                for (receiver, indice) in [(objeto, 37), (objeto, 38), (objeto, -1), (objeto, 40), (0, 0), (filho, 0)] {
                    assert!(ler_campo_ref_auditado(&h, receiver, indice).is_err());
                }
                assert_eq!(h.stats().allocations, stats.allocations);
                assert_eq!(h.stats().collections, stats.collections);
            });
            dartforge_arc_gravar_campo_ref_v1(objeto, 39, 0);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(filho), 0);
            let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_gravar_campo_ref_v1(objeto, 39, filho);
            dartforge_arc_release(filho);
            let copia = dartforge_arc_ler_campo_ref_v1(objeto, 39);
            assert_eq!(copia, filho);
            dartforge_arc_retain(copia);
            dartforge_arc_gravar_campo_ref_v1(objeto, 39, 0);
            dartforge_arc_release(objeto);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
            assert_eq!(dartforge_arc_observar_heap_v1(copia), 3);
            dartforge_arc_release(copia);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(copia), 0);
            HEAP.with(|h| { h.replace(anterior); });
        }
    }
}

#[cfg(test)]
mod testes_leitura_late_ref {
    use super::*;

    #[test]
    fn late_ref_distingue_zero_vazio_null_escrito_e_bits_escalares() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let objeto = dartforge_arc_objeto_owned_v1(123, 40);
            let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
            let antes = HEAP.with(|h| h.borrow().stats());
            assert_eq!(dartforge_arc_ler_campo_late_ref_v1(objeto, 39), 0);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.marcar_late((objeto, 39));
                assert!(ler_campo_late_ref_auditado(&h, objeto, 39).is_err());
                h.desmarcar_late((objeto, 39));
                for (bits, marcado) in [(filho, false), (42, false), (0, true), (filho, true)] {
                    h.definir_campo(objeto, 38, bits, false);
                    if marcado { h.marcar_late((objeto, 38)); } else { h.desmarcar_late((objeto, 38)); }
                    assert!(ler_campo_late_ref_auditado(&h, objeto, 38).is_err());
                }
                for (receiver, indice) in [(0, 0), (objeto, -1), (objeto, 40)] {
                    assert!(ler_campo_late_ref_auditado(&h, receiver, indice).is_err());
                }
                assert_eq!(h.stats().allocations, antes.allocations);
                assert_eq!(h.stats().collections, antes.collections);
            });
            for valor in [0, smi::de(42).unwrap(), filho] {
                dartforge_arc_gravar_campo_ref_v1(objeto, 39, valor);
                HEAP.with(|h| h.borrow_mut().marcar_late((objeto, 39)));
                assert_eq!(dartforge_arc_ler_campo_late_ref_v1(objeto, 39), valor);
            }
            dartforge_arc_release(filho);
            let copia = dartforge_arc_ler_campo_late_ref_v1(objeto, 39);
            dartforge_arc_retain(copia);
            dartforge_arc_gravar_campo_ref_v1(objeto, 39, 0);
            dartforge_arc_release(objeto);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(copia), 3);
            assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
            HEAP.with(|h| assert!(ler_campo_late_ref_auditado(&h.borrow(), objeto, 39).is_err()));
            dartforge_arc_release(copia);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(copia), 0);
            HEAP.with(|h| { h.replace(anterior); });
        }
    }
}

#[cfg(test)]
mod testes_gravacao_campo {
    use super::*;

    #[test]
    fn gravacao_distingue_bits_de_arestas_e_substitui_sem_consumir_tokens() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let objeto = dartforge_arc_objeto_owned_v1(123, 40);
            let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
            let antes = HEAP.with(|h| h.borrow().stats());
            for indice in [0, 37, 38] {
                dartforge_arc_gravar_campo_ref_v1(objeto, indice, filho);
                dartforge_arc_gravar_campo_ref_v1(objeto, indice, filho);
            }
            for (indice, bits) in [(1, i64::MAX), (32, (-0.0_f64).to_bits() as i64), (39, 0x7ff80000deadbeef)] {
                dartforge_arc_gravar_campo_escalar_v1(objeto, indice, bits);
                assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, indice), bits);
            }
            HEAP.with(|h| {
                let h = h.borrow();
                assert_eq!(h.stats().allocations, antes.allocations);
                assert_eq!(h.stats().collections, antes.collections);
                assert_eq!(h.observar_owner_codigo(objeto), 3);
                assert_eq!(h.observar_owner_codigo(filho), 3);
            });
            dartforge_arc_release(filho);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(filho), 1);
            // Os mesmos bits deixam de constituir aresta nos dois mapas.
            for indice in [0, 37] { dartforge_arc_gravar_campo_escalar_v1(objeto, indice, filho); }
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(filho), 1);
            dartforge_arc_gravar_campo_ref_v1(objeto, 38, 0);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(filho), 0);
            for indice in [0, 37] { assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, indice), filho); }
            dartforge_arc_gravar_campo_ref_v1(objeto, 33, objeto);
            dartforge_arc_collect();
            dartforge_arc_gravar_campo_escalar_v1(objeto, 33, 0);
            dartforge_arc_release(objeto);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn receiver_e_indice_invalidos_nao_gravam_nem_coletam() {
        let mut heap = Heap::new(true);
        let objeto = heap.alocar_instancia(123, 40);
        heap.reter_owner_codigo(objeto);
        let caixa = heap.caixa_int(i64::MAX);
        heap.reter_owner_codigo(caixa);
        let antes = heap.objeto(objeto).unwrap().to_vec();
        let stats = heap.stats();
        for referencia in [false, true] {
            for (receiver, indice) in [(objeto, -1), (objeto, 40), (0, 0), (caixa, 0)] {
                assert!(gravar_campo_auditado(&mut heap, receiver, indice, 0, referencia).is_err());
                assert_eq!(heap.objeto(objeto).unwrap().to_vec(), antes);
                assert_eq!(heap.stats().allocations, stats.allocations);
                assert_eq!(heap.stats().collections, stats.collections);
            }
        }
    }
}

#[cfg(test)]
mod testes_instancia_owned {
    use super::*;

    #[test]
    fn instancia_owned_registra_metodos_antes_de_alocar_e_preserva_token() {
        use std::sync::{OnceLock, atomic::{AtomicUsize, Ordering}};
        static CHAMADAS: AtomicUsize = AtomicUsize::new(0);
        extern "C" fn entrada(_: i64, _: *const i64, _: *const i64) -> i64 { 85 }
        extern "C" fn tabela() -> *const i64 {
            static TABELA: OnceLock<[i64; 4]> = OnceLock::new();
            CHAMADAS.fetch_add(1, Ordering::Relaxed);
            TABELA.get_or_init(|| [32001, 1, 42, entrada as *const () as usize as i64]).as_ptr()
        }
        let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
        HEAP.with(|h| h.borrow_mut().ativar_arc());
        for _ in 0..2 {
            // SAFETY: tabela estática válida com uma entrada nativa durável.
            let objeto = unsafe { dartforge_arc_objeto_owned_t_v1(32001, 1, tabela) };
            assert_eq!(metodo_da_classe(32001, 42), Some(entrada as *const () as usize));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(objeto), 3);
            assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, 0), 0);
            dartforge_arc_release(objeto);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
        }
        assert_eq!(CHAMADAS.load(Ordering::Relaxed), 1);
        HEAP.with(|h| { h.replace(anterior); });
    }

    #[test]
    fn instancia_zerada_e_token_sobrevivem_a_stress_ate_uma_liberacao() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            for campos in [0, 1, 32, 33, 40] {
                let objeto = dartforge_arc_objeto_owned_v1(123, campos);
                assert_eq!(dartforge_arc_observar_heap_v1(objeto), 3);
                // A próxima fábrica pode coletar antes de publicar seu token.
                let outro = dartforge_arc_objeto_owned_v1(124, 1);
                dartforge_arc_collect();
                HEAP.with(|h| {
                    let h = h.borrow();
                    let vista = h.objeto(objeto).unwrap();
                    assert_eq!(vista.class_id, 123);
                    assert_eq!(vista.len(), campos as usize);
                    assert!(vista.iter().all(|campo| campo == (0, false)));
                });
                let filho = if campos > 0 {
                    let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
                    dartforge_object_set(objeto, campos - 1, filho, 1);
                    dartforge_arc_release(filho);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observar_heap_v1(filho), 1);
                    assert_eq!(HEAP.with(|h| h.borrow().objeto(objeto).unwrap().campo(campos as usize - 1)), (filho, true));
                    Some(filho)
                } else { None };
                dartforge_arc_release(outro);
                dartforge_arc_release(objeto);
                assert_eq!(dartforge_arc_observar_heap_v1(objeto) & 2, 0);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
                assert_eq!(dartforge_arc_observar_heap_v1(outro), 0);
                if let Some(filho) = filho { assert_eq!(dartforge_arc_observar_heap_v1(filho), 0); }
            }
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn parametros_devem_caber_no_cabecalho_sem_truncar() {
        for classe in [i32::MIN as i64, 0, i32::MAX as i64] {
            for campos in [0, 40, u16::MAX as i64] {
                assert_eq!(parametros_instancia_owned(classe, campos), Ok((classe as i32, campos as usize)));
            }
        }
        for classe in [i32::MIN as i64 - 1, i32::MAX as i64 + 1] {
            assert!(parametros_instancia_owned(classe, 1).is_err());
        }
        for campos in [-1, u16::MAX as i64 + 1, i64::MAX] {
            assert!(parametros_instancia_owned(123, campos).is_err());
        }
    }
}

#[cfg(test)]
mod testes_campo_escalar {
    use super::*;

    #[test]
    fn leitura_confere_mapa_sem_confundir_bits_com_handle_e_sem_coletar() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let objeto = HEAP.with(|h| {
                let mut h = h.borrow_mut();
                let filho = h.alocar_str("campo de referência");
                h.reter_owner_codigo(filho);
                let mut campos = vec![(0, false); 40];
                campos[0] = (filho, false);
                campos[1] = (filho, true);
                campos[2] = ((-0.0_f64).to_bits() as i64, false);
                campos[3] = (0x7ff80000deadbeef, false);
                campos[32] = (i64::MAX, false);
                campos[37] = (filho, true);
                let objeto = h.novo_objeto(123, &campos);
                h.reter_owner_codigo(objeto);
                h.collect();
                objeto
            });
            let antes = HEAP.with(|h| h.borrow().stats());
            for i in [0, 2, 3, 32, 39] {
                let esperado = HEAP.with(|h| h.borrow().objeto(objeto).unwrap().campo(i).0);
                assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, i as i64), esperado);
            }
            HEAP.with(|h| {
                let h = h.borrow();
                for i in [1, 37] {
                    assert_eq!(ler_campo_escalar_auditado(&h, objeto, i), Err("campo é referência gerenciada"));
                }
                for i in [-1, 40] { assert!(ler_campo_escalar_auditado(&h, objeto, i).is_err()); }
                assert!(ler_campo_escalar_auditado(&h, 0, 0).is_err());
                assert_eq!(h.stats().allocations, antes.allocations);
                assert_eq!(h.stats().collections, antes.collections);
            });
            HEAP.with(|h| { h.replace(anterior); });
        }
    }
}
