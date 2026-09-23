//! O universo de tipos (RTI, `src/tipos.rs`) e o laço de eventos
//! (`src/eventos.rs`), pela ABI que o código gerado usa.
//!
//! Cada teste roda numa thread própria: o estado do runtime é por thread (um
//! isolado por thread).

use dartforge_runtime::abi::*;

/// Lê uma receita (o texto vira `String` do heap, como no código gerado).
#[allow(unsafe_code)]
fn receita(r: &str) -> i64 {
    // SAFETY: `r` é UTF-8 válido com `r.len()` bytes, vivo durante a chamada.
    let h = unsafe { dartforge_string_new(r.as_ptr(), r.len() as i64) };
    dartforge_rti_receita(h)
}

fn sub(s: &str, t: &str) -> bool {
    dartforge_rti_subtipo(receita(s), receita(t)) != 0
}

fn numa_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(16 << 20).spawn(f).unwrap().join().unwrap();
}

/// Subtipagem (especificação, "Subtypes"): classes com argumentos pelas
/// regras de supertipo, anuláveis, `FutureOr`, funções (parâmetros
/// contravariantes), e a avaliação de uma receita com `P<i>` no tipo de
/// `this`.
#[test]
fn subtipagem_e_avaliacao() {
    numa_thread(|| {
        // 0 = Object; 1 e 2 classes sem parâmetros; 10 = List<E>, 11 =
        // Iterable<E>, 20 = Future<T>.
        dartforge_rti_classe_do_runtime(10, 0);
        dartforge_rti_classe_do_runtime(9, 20);
        dartforge_rti_regra(10, receita("C11<P0>"));
        dartforge_rti_regra(1, receita("C2"));
        assert!(sub("C10<C1>", "C11<C1>"));
        assert!(sub("C10<C1>", "C11<C2>"), "covariante pela regra 1 <: 2");
        assert!(!sub("C10<C2>", "C11<C1>"));
        assert!(!sub("C11<C1>", "C10<C1>"));
        assert!(sub("C1", "C0"), "tudo é Object");
        assert!(!sub("U", "C0"), "Null não é Object");
        assert!(sub("U", "C1?"));
        assert!(!sub("C1?", "C1"));
        assert!(sub("C1", "C1?"));
        assert!(sub("C20<C1>", "O<C1>"));
        assert!(sub("C1", "O<C1>"));
        assert!(!sub("C2", "O<C1>"));
        assert!(sub("N", "C1"), "Never é o fundo");
        assert!(sub("C1", "D"), "dynamic é o topo");
        // R Function(P): retorno covariante, parâmetro contravariante.
        assert!(sub("F<0;C1;1;C0;>", "F<0;C0;1;C1;>"));
        assert!(!sub("F<0;C0;1;C1;>", "F<0;C1;1;C0;>"));
        assert!(!sub("F<0;D;1;D;>", "F<0;D;2;C0,C0;>"), "aridade");

        // `P0` avaliado no tipo de `this` (List<C1>) visto como Iterable.
        let obj = dartforge_object_new(10, 0);
        dartforge_rti_definir(obj, receita("C10<C1>"));
        let t = dartforge_rti_avaliar(receita("C11<P0>"), obj, 11, 0);
        assert_eq!(t, receita("C11<C1>"));
        assert!(dartforge_rti_e(obj, receita("C11<C2>")) != 0);
        assert!(dartforge_rti_e(obj, receita("C11<U>")) == 0);
        // `M0` da tupla.
        let t = dartforge_rti_avaliar(receita("C10<M0>"), 0, 0, receita("L<C2>"));
        assert_eq!(t, receita("C10<C2>"));
    });
}

thread_local! {
    static ORDEM: std::cell::RefCell<Vec<i64>> = const { std::cell::RefCell::new(Vec::new()) };
}

extern "C" fn chamar(closure: i64) -> i64 {
    let codigo = dartforge_closure_code(closure);
    ORDEM.with(|o| o.borrow_mut().push(codigo));
    0
}

/// Microtarefas antes de qualquer timer; timers por (prazo, sequência); o
/// timer cancelado não roda.
#[test]
fn laco_de_eventos_na_ordem_da_vm() {
    numa_thread(|| {
        let c = |k: i64| dartforge_tearoff(k);
        dartforge_nativo_DartForge_Timer_novo(20, c(1), 0);
        dartforge_nativo_DartForge_Timer_novo(0, c(2), 0);
        let cancelado = dartforge_nativo_DartForge_Timer_novo(0, c(3), 0);
        dartforge_nativo_DartForge_Timer_novo(0, c(5), 0);
        dartforge_nativo_DartForge_scheduleImmediate(c(4));
        dartforge_nativo_DartForge_Timer_cancelar(cancelado);
        dartforge_laco_de_eventos(chamar);
        let ordem = ORDEM.with(|o| o.borrow().clone());
        assert_eq!(ordem, vec![4, 2, 5, 1]);
    });
}
