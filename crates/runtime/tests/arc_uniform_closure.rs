//! Regressions of the uniform closure boundary, with a real heap and real exceptions.
use dartforge_runtime::abi::*;

extern "C" fn mint(_: i64, _: *const i64, _: *const i64) -> i64 {
    dartforge_arc_box_int_owned_v1(i64::MAX)
}
extern "C" fn pending(_: i64, _: *const i64, _: *const i64) -> i64 {
    dartforge_exception_throw(42, 1);
    0
}
extern "C" fn error(_: i64) -> i64 {
    dartforge_arc_box_int_owned_v1(i64::MAX)
}

#[test]
#[allow(unsafe_code)] // Static ABI fixtures; each call checks its contract.
fn closure_preserves_one_owned_and_restores_scopes_even_when_pending() {
    std::thread::spawn(|| {
        dartforge_memoria_arc_v1();
        let args = [0_i64];
        let desc = [0_i64, 0];
        for (f, failure) in [
            (mint as *const () as usize, false),
            (pending as *const () as usize, true),
        ] {
            // SAFETY: static fixtures of the readonly uniform ABI with an Owned return/null pending.
            let v = unsafe { dartforge_arc_registrar_uniforme_owned_v1(f as i64) };
            let c = dartforge_arc_closure_owned_v1(f as i64, 0, 0, 0);
            let scope = dartforge_arc_escopo_nativo_abrir_v1();
            // SAFETY: the registered closure, vectors and code stay alive.
            let x = unsafe {
                dartforge_arc_invocar_closure_uniforme_v1(
                    c,
                    args.as_ptr() as i64,
                    desc.as_ptr() as i64,
                )
            };
            dartforge_arc_escopo_nativo_fechar_v1(scope);
            dartforge_arc_release(c);
            dartforge_arc_collect();
            if failure {
                assert_eq!(x, 0);
                assert_ne!(dartforge_exception_pending(), 0);
                dartforge_exception_clear();
            } else {
                assert_eq!(dartforge_arc_observar_heap_v1(x), 3);
                assert_eq!(dartforge_unbox_int(x), i64::MAX);
                dartforge_arc_release(x);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(x), 0);
            }
            assert_eq!(dartforge_arc_observar_heap_v1(c), 0);
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
        }
        assert_eq!(args, [0]);
        assert_eq!(desc, [0, 0]);
    })
    .join()
    .unwrap();
}

#[test]
#[allow(unsafe_code)] // Static ABI fixtures; each call checks its contract.
fn invalid_lookup_holds_owned_helper_until_exception_is_published() {
    std::thread::spawn(|| {
        dartforge_memoria_arc_v1();
        let pc = error as *const () as usize;
        let name = b"_dartforgeErroDeChamada";
        // SAFETY: static one-word fixture with an Owned Mint return.
        let v = unsafe { dartforge_arc_registrar_retorno_owned_v1(pc as i64, 1) };
        // SAFETY: static name and a callback with a compatible ABI.
        unsafe {
            dartforge_registrar_ajudante(name.as_ptr(), name.len() as i64, pc);
        }
        let args = [0_i64];
        let desc = [0_i64, 0];
        let scope = dartforge_arc_escopo_nativo_abrir_v1();
        // SAFETY: null is a valid Ref receiver; vectors are alive. An invalid lookup returns pending.
        assert_eq!(
            unsafe {
                dartforge_arc_invocar_closure_uniforme_v1(
                    0,
                    args.as_ptr() as i64,
                    desc.as_ptr() as i64,
                )
            },
            0
        );
        dartforge_arc_escopo_nativo_fechar_v1(scope);
        assert_ne!(dartforge_exception_pending(), 0);
        let payload = dartforge_arc_excecao_owned_v1();
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observar_heap_v1(payload), 3);
        assert_eq!(dartforge_unbox_int(payload), i64::MAX);
        dartforge_exception_clear();
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observar_heap_v1(payload), 3);
        dartforge_arc_release(payload);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observar_heap_v1(payload), 0);
        dartforge_arc_retirar_retorno_owned_v1(pc as i64, v);
    })
    .join()
    .unwrap();
}
