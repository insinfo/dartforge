//! O universo de tipos (RTI, `src/tipos.rs`) e o laço de eventos
//! (`src/eventos.rs`), pela ABI que o código gerado usa.
//!
//! Cada teste roda numa thread própria: o estado do runtime é por thread (um
//! isolado por thread).

use dartforge_runtime::abi::*;

/// Lê uma receita (o texto vira `String` do heap, como no código gerado).
#[allow(unsafe_code)]
fn recipe(r: &str) -> i64 {
    // SAFETY: `r` é UTF-8 válido com `r.len()` bytes, vivo durante a chamada.
    let h = unsafe { dartforge_string_new(r.as_ptr(), r.len() as i64) };
    dartforge_rti_receita(h)
}

fn sub(s: &str, t: &str) -> bool {
    dartforge_rti_subtipo(recipe(s), recipe(t)) != 0
}

fn in_a_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(16 << 20).spawn(f).unwrap().join().unwrap();
}

#[test]
#[allow(unsafe_code)]
fn internal_recipe_ids_without_collecting_retaining_string_or_changing_pending() {
    in_a_thread(|| {
        dartforge_memoria_arc_v1();
        for recipe in ["C350<C201>", "F<0;C201;1;C201;nome:C350!>", "R<C201;nome:C350>", "L<C201,C350>"] {
            let mut previous = None;
            for pending in [false, true] {
                let mid = recipe.len() / 2;
                // SAFETY: receitas ASCII; as duas partes são regiões válidas.
                let (a, b) = unsafe {
                    (dartforge_string_new(recipe.as_ptr(), mid as i64),
                     dartforge_string_new(recipe.as_ptr().add(mid), (recipe.len() - mid) as i64))
                };
                // Os literais são raízes canônicas; a concatenação é dinâmica.
                let text = dartforge_string_concat(a, b);
                dartforge_arc_retain(text);
                let trace = if pending {
                    let r = dartforge_stack_trace_get();
                    dartforge_throw_with_stack_trace(0, 3, r);
                    Some(r)
                } else { None };
                let name = b"dartforge_rti_receita";
                // SAFETY: nome constante. Bit 0 proíbe coleta, bit 1 nova pendência.
                unsafe { dartforge_efeitos_antes(name.as_ptr(), name.len() as i64, 3); }
                let tipo = dartforge_rti_receita(text);
                dartforge_efeitos_depois();
                assert_eq!(dartforge_efeitos_nivel(), 0);
                assert_eq!(dartforge_exception_pending(), u8::from(pending));
                if let Some(trace) = trace {
                    assert_eq!(dartforge_exception_peek_ref(), 0);
                    assert_eq!(dartforge_stack_trace_get(), trace);
                    dartforge_exception_clear();
                }
                if let Some(id) = previous { assert_eq!(tipo, id); }
                previous = Some(tipo);
                assert_eq!(dartforge_arc_observar_heap_v1(text), 3);
                dartforge_arc_release(text);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(text), 0, "receita {recipe}; pendente {pending}");
                // O ID canônico permanece válido sem reter a string da receita.
                assert_eq!(dartforge_rti_subtipo(tipo, tipo), 1);
            }
        }
    });
}

#[test]
#[allow(unsafe_code)]
fn set_rti_preserves_owner_pending_and_elements_on_decompact() {
    in_a_thread(|| {
        dartforge_memoria_arc_v1();
        dartforge_rti_classe_do_runtime(0, 201);
        dartforge_rti_classe_do_runtime(4, 200);
        dartforge_rti_regra(8, recipe("C200<P0>"));
        let int_type = recipe("C200<C201>");
        let general_type = recipe("C200<D>");
        let value_list = [i64::MAX, i64::MIN];
        // SAFETY: vetor de palavras vivo e completo para a lista compacta.
        let list = unsafe { dartforge_lista_nova(value_list.as_ptr(), 2, 8, 1) };
        dartforge_arc_retain(list);
        let define = |tipo| {
            let name = b"dartforge_rti_definir";
            // SAFETY: nome constante vivo, com seu comprimento exato.
            unsafe { dartforge_efeitos_antes(name.as_ptr(), name.len() as i64, 2); }
            dartforge_rti_definir(list, tipo);
            dartforge_efeitos_depois();
            assert_eq!(dartforge_efeitos_nivel(), 0);
        };
        define(int_type);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observar_heap_v1(list), 3);
        define(general_type);
        assert_eq!(dartforge_exception_pending(), 0);
        define(int_type);
        // A chamada não limpa nem substitui uma pendência anterior.
        let trace = dartforge_stack_trace_get();
        dartforge_throw_with_stack_trace(0, 3, trace);
        define(general_type);
        assert_eq!(dartforge_exception_pending(), 1);
        assert_eq!(dartforge_exception_peek_ref(), 0);
        assert_eq!(dartforge_stack_trace_get(), trace);
        dartforge_exception_clear();
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observar_heap_v1(list), 3);
        let first = dartforge_nativo_DartForge_lista_get(list, 0);
        let second = dartforge_nativo_DartForge_lista_get(list, 1);
        assert_eq!(dartforge_unbox_int(first), value_list[0]);
        assert_eq!(dartforge_unbox_int(second), value_list[1]);
        assert_ne!(dartforge_rti_e(list, general_type), 0);
        dartforge_arc_release(list);
        dartforge_arc_collect();
        for h in [list, first, second] {
            assert_eq!(dartforge_arc_observar_heap_v1(h), 0);
        }
        assert_eq!(dartforge_exception_pending(), 0);
    });
}

#[test]
fn late_field_tells_zero_value_from_missing_write() {
    in_a_thread(|| {
        let first = dartforge_object_new(1, 1);
        let second = dartforge_object_new(1, 1);
        assert_eq!(dartforge_late_field_initialized(first, 0), 0);
        dartforge_object_set(first, 0, 0, 0);
        dartforge_late_field_mark_initialized(first, 0);
        assert_eq!(dartforge_late_field_initialized(first, 0), 1);
        assert_eq!(dartforge_object_get(first, 0), 0);
        assert_eq!(dartforge_late_field_initialized(second, 0), 0);
    });
}

/// Subtipagem (especificação, "Subtypes"): classes com argumentos pelas
/// regras de supertipo, anuláveis, `FutureOr`, funções (parâmetros
/// contravariantes), e a avaliação de uma receita com `P<i>` no tipo de
/// `this`.
#[test]
fn subtyping_and_evaluation() {
    in_a_thread(|| {
        // 0 = Object; 201 e 202 classes sem parâmetros; 210 = List<E>, 211 =
        // Iterable<E>, 220 = Future<T> (classes do programa: cid ≥ 128, fora
        // dos cids fixos do runtime, docs/NATIVO-ESPACO-UNIFICADO.md §2.4).
        dartforge_rti_classe_do_runtime(10, 0);
        dartforge_rti_classe_do_runtime(9, 220);
        dartforge_rti_regra(210, recipe("C211<P0>"));
        dartforge_rti_regra(201, recipe("C202"));
        assert!(sub("C210<C201>", "C211<C201>"));
        assert!(sub("C210<C201>", "C211<C202>"), "covariante pela regra 201 <: 202");
        assert!(!sub("C210<C202>", "C211<C201>"));
        assert!(!sub("C211<C201>", "C210<C201>"));
        assert!(sub("C201", "C0"), "tudo é Object");
        assert!(!sub("U", "C0"), "Null não é Object");
        assert!(sub("U", "C201?"));
        assert!(!sub("C201?", "C201"));
        assert!(sub("C201", "C201?"));
        assert!(sub("C220<C201>", "O<C201>"));
        assert!(sub("C201", "O<C201>"));
        assert!(!sub("C202", "O<C201>"));
        assert!(sub("N", "C201"), "Never é o fundo");
        assert!(sub("C201", "D"), "dynamic é o topo");
        // R Function(P): retorno covariante, parâmetro contravariante.
        assert!(sub("F<0;C201;1;C0;>", "F<0;C0;1;C201;>"));
        assert!(!sub("F<0;C0;1;C201;>", "F<0;C201;1;C0;>"));
        assert!(!sub("F<0;D;1;D;>", "F<0;D;2;C0,C0;>"), "aridade");

        // `P0` avaliado no tipo de `this` (List<C201>) visto como Iterable.
        let obj = dartforge_object_new(210, 0);
        dartforge_rti_definir(obj, recipe("C210<C201>"));
        let t = dartforge_rti_avaliar(recipe("C211<P0>"), obj, 211, 0);
        assert_eq!(t, recipe("C211<C201>"));
        assert!(dartforge_rti_e(obj, recipe("C211<C202>")) != 0);
        assert!(dartforge_rti_e(obj, recipe("C211<U>")) == 0);
        // `M0` da tupla.
        let t = dartforge_rti_avaliar(recipe("C210<M0>"), 0, 0, recipe("L<C202>"));
        assert_eq!(t, recipe("C210<C202>"));
    });
}

/// Os allocators de listas do SDK recebem `L<E>` pela ABI. A fatia de uma
/// `_List<E>` e a cópia `_ImmutableList<E>` conservam E nos metadados, mesmo
/// quando a classe concreta do resultado é diferente da origem.
#[test]
#[allow(unsafe_code)]
fn list_allocators_preserve_type_argument() {
    in_a_thread(|| {
        // 200 = List<E> (uma classe do programa, cid ≥ 128); as concretas têm
        // os cids fixos do contrato (docs/NATIVO-ESPACO-UNIFICADO.md §2.4):
        // 8 = _List<E>, 9 = _ImmutableList<E>, 10 = _GrowableList<E>.
        dartforge_rti_classe_do_runtime(4, 200);
        for class in [8, 9, 10] {
            dartforge_rti_regra(class, recipe("C200<P0>"));
        }

        let fixed_one = dartforge_nativo_List_allocate(dartforge_box_int(2), recipe("L<C201>"));
        assert!(dartforge_rti_e(fixed_one, recipe("C200<C201>")) != 0);
        let piece = dartforge_nativo_List_slice(fixed_one, 0, 2, 1);
        assert!(dartforge_rti_e(piece, recipe("C200<C201>")) != 0);

        let unmodifiable = dartforge_nativo_ImmutableList_from(fixed_one, 0, 2, recipe("L<C201>"));
        assert!(dartforge_rti_e(unmodifiable, recipe("C200<C201>")) != 0);
        assert!(dartforge_rti_e(unmodifiable, recipe("C200<C202>")) == 0);

        let unmodifiable_from_copy = dartforge_nativo_Internal_makeFixedListUnmodifiable(fixed_one);
        assert!(dartforge_rti_e(unmodifiable_from_copy, recipe("C200<C201>")) != 0);
        let fixed_from_copy = dartforge_nativo_Internal_makeListFixedLength(unmodifiable_from_copy);
        assert!(dartforge_rti_e(fixed_from_copy, recipe("C200<C201>")) != 0);

        let mutable = dartforge_nativo_GrowableList_allocate(fixed_one, recipe("L<C201>"));
        assert!(dartforge_rti_e(mutable, recipe("C200<C201>")) != 0);
    });
}

thread_local! {
    static ORDER: std::cell::RefCell<Vec<i64>> = const { std::cell::RefCell::new(Vec::new()) };
}

extern "C" fn call(closure: i64) -> i64 {
    let code = dartforge_closure_code(closure);
    ORDER.with(|o| o.borrow_mut().push(code));
    0
}

/// Microtarefas antes de qualquer timer; timers por (prazo, sequência); o
/// timer cancelado não roda.
#[test]
fn event_loop_in_vm_order() {
    in_a_thread(|| {
        let c = |k: i64| dartforge_tearoff(k);
        dartforge_nativo_DartForge_Timer_novo(20, c(1), 0);
        dartforge_nativo_DartForge_Timer_novo(0, c(2), 0);
        let cancelled = dartforge_nativo_DartForge_Timer_novo(0, c(3), 0);
        dartforge_nativo_DartForge_Timer_novo(0, c(5), 0);
        dartforge_nativo_DartForge_scheduleImmediate(c(4));
        dartforge_nativo_DartForge_Timer_cancelar(cancelled);
        dartforge_laco_de_eventos(call);
        let order = ORDER.with(|o| o.borrow().clone());
        assert_eq!(order, vec![4, 2, 5, 1]);
    });
}
