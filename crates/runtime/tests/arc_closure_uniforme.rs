//! Regressões da fronteira uniforme de closures, com heap e exceção reais.
use dartforge_runtime::abi::*;

extern "C" fn mint(_: i64, _: *const i64, _: *const i64) -> i64 {
    dartforge_arc_box_int_owned_v1(i64::MAX)
}
extern "C" fn pendente(_: i64, _: *const i64, _: *const i64) -> i64 {
    dartforge_exception_throw(42, 1);
    0
}
extern "C" fn erro(_: i64) -> i64 {
    dartforge_arc_box_int_owned_v1(i64::MAX)
}

#[test]
#[allow(unsafe_code)] // Fixtures de ABI estática; cada chamada confere seu contrato.
fn closure_preserva_um_owned_e_restabelece_escopos_inclusive_no_pending() {
    std::thread::spawn(|| {
        dartforge_memoria_arc_v1();
        let args = [0_i64];
        let desc = [0_i64, 0];
        for (f, falha) in [
            (mint as *const () as usize, false),
            (pendente as *const () as usize, true),
        ] {
            // SAFETY: fixtures estáticas de ABI uniforme readonly e retorno Owned/null pending.
            let v = unsafe { dartforge_arc_registrar_uniforme_owned_v1(f as i64) };
            let c = dartforge_arc_closure_owned_v1(f as i64, 0, 0, 0);
            let escopo = dartforge_arc_escopo_nativo_abrir_v1();
            // SAFETY: closure, vetores e código registrados permanecem vivos.
            let x = unsafe {
                dartforge_arc_invocar_closure_uniforme_v1(
                    c,
                    args.as_ptr() as i64,
                    desc.as_ptr() as i64,
                )
            };
            dartforge_arc_escopo_nativo_fechar_v1(escopo);
            dartforge_arc_release(c);
            dartforge_arc_collect();
            if falha {
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
#[allow(unsafe_code)] // Fixtures de ABI estática; cada chamada confere seu contrato.
fn busca_invalida_prende_helper_owned_ate_publicar_a_excecao() {
    std::thread::spawn(|| {
        dartforge_memoria_arc_v1();
        let pc = erro as *const () as usize;
        let nome = b"_dartforgeErroDeChamada";
        // SAFETY: fixture estática de uma palavra com retorno Mint Owned.
        let v = unsafe { dartforge_arc_registrar_retorno_owned_v1(pc as i64, 1) };
        // SAFETY: nome estático e callback de ABI compatível.
        unsafe {
            dartforge_registrar_ajudante(nome.as_ptr(), nome.len() as i64, pc);
        }
        let args = [0_i64];
        let desc = [0_i64, 0];
        let escopo = dartforge_arc_escopo_nativo_abrir_v1();
        // SAFETY: null é receptor Ref válido; vetores vivos. Busca inválida devolve pending.
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
        dartforge_arc_escopo_nativo_fechar_v1(escopo);
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
