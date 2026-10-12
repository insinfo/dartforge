//! A tabela parcial deve distinguir referências de escalares e ausência de contrato.

// Inclui as rejeições do gerador na suíte Cargo, sem executar seu main.
#[allow(dead_code)]
#[path = "../build.rs"]
mod gerador;

use dartforge_runtime::{
    efeitos::EFEITOS,
    ownership::{CONTRATOS, ModoParametro, ModoResultado},
    simbolos::NOMES,
};

#[test]
fn closure_owned_mantem_contexto_celula_e_arestas_repetidas_ate_o_drop() {
    use dartforge_runtime::{abi::*, layout::cid};
    extern "C" fn entrada(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
    extern "C" fn corpo(_: i64) -> i64 { 7 }
    std::thread::spawn(|| {
        dartforge_memoria_arc_v1();
        for ciclo in [false, true] {
            let valor = dartforge_arc_box_int_owned_v1(i64::MAX);
            let celula = dartforge_arc_objeto_owned_v1(i64::from(cid::CELULA), 1);
            dartforge_arc_gravar_campo_ref_v1(celula, 0, valor);
            dartforge_arc_release(valor);
            let env = dartforge_arc_objeto_owned_v1(i64::from(cid::CONTEXTO), 40);
            for i in [0, 39] { dartforge_arc_gravar_campo_ref_v1(env, i, celula); }
            dartforge_arc_release(celula);
            let codigo = entrada as *const () as usize as i64;
            let tipado = corpo as *const () as usize as i64;
            let closure = dartforge_arc_closure_owned_v1(codigo, env, tipado, 7);
            assert_eq!(dartforge_arc_observar_heap_v1(env), 3);
            dartforge_arc_release(env);
            if ciclo { dartforge_arc_gravar_campo_ref_v1(env, 38, closure); }
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(closure), 3);
            for h in [env, celula, valor] { assert_eq!(dartforge_arc_observar_heap_v1(h), 1); }
            assert_eq!(dartforge_closure_code(closure), codigo);
            assert_eq!(dartforge_closure_tipada(closure, 7), tipado);
            assert_eq!(dartforge_arc_contexto_closure_v1(closure), env);
            assert_eq!(dartforge_arc_env_ref_v1(env, 39), celula);
            assert_eq!(dartforge_unbox_int(dartforge_arc_ler_campo_ref_v1(celula, 0)), i64::MAX);
            dartforge_arc_release(closure);
            dartforge_arc_collect();
            for h in [closure, env, celula, valor] { assert_eq!(dartforge_arc_observar_heap_v1(h), 0); }
        }
    }).join().unwrap();
}

#[test]
fn contexto_emprestado_precisa_de_owner_apos_liberar_closure() {
    use dartforge_runtime::abi::*;
    extern "C" fn entrada(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
    std::thread::spawn(|| {
        dartforge_memoria_arc_v1();
        let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
        let closure = dartforge_arc_closure_owned_v1(entrada as *const () as usize as i64, filho, 0, 0);
        dartforge_arc_release(filho);
        let emprestado = dartforge_arc_contexto_closure_v1(closure);
        assert_eq!(emprestado, filho);
        assert_eq!(dartforge_arc_observar_heap_v1(emprestado), 1);
        dartforge_arc_retain(emprestado);
        dartforge_arc_release(closure);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observar_heap_v1(closure), 0);
        assert_eq!(dartforge_unbox_int(emprestado), i64::MAX);
        assert_eq!(dartforge_arc_observar_heap_v1(emprestado), 3);
        dartforge_arc_release(emprestado);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observar_heap_v1(emprestado), 0);
        let canonica = dartforge_tearoff(123);
        assert_eq!(dartforge_arc_contexto_closure_v1(canonica), 0);
    }).join().unwrap();
}

#[test]
fn nsm_publica_erro_ate_clear_sem_owner_no_chamador() {
    use dartforge_runtime::abi::*;
    std::thread::spawn(|| {
        dartforge_memoria_arc_v1();
        dartforge_nsm_chamada();
        assert_ne!(dartforge_exception_pending(), 0);
        let erro = dartforge_exception_peek_ref();
        assert_ne!(erro, 0);
        dartforge_arc_collect();
        assert_eq!(dartforge_exception_peek_ref(), erro);
        assert_eq!(dartforge_arc_observar_heap_v1(erro), 1);
        dartforge_exception_clear();
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observar_heap_v1(erro), 0);
    }).join().unwrap();
}

/// Os acessos por índice e por bytes das listas tipadas e o `codeUnitAt`
/// (os caminhos do runtime sob ARC): emprestam o receptor e só lançam
/// `RangeError`.
const ACESSOS_TIPADOS: &[&str] = &[
    "dartforge_nativo_DartForge_typed_getInt8",
    "dartforge_nativo_DartForge_typed_getUint8",
    "dartforge_nativo_DartForge_typed_getInt16",
    "dartforge_nativo_DartForge_typed_getUint16",
    "dartforge_nativo_DartForge_typed_getInt32",
    "dartforge_nativo_DartForge_typed_getUint32",
    "dartforge_nativo_DartForge_typed_getInt64",
    "dartforge_nativo_DartForge_typed_getUint64",
    "dartforge_nativo_DartForge_typed_setInt8",
    "dartforge_nativo_DartForge_typed_setUint8",
    "dartforge_nativo_DartForge_typed_setInt16",
    "dartforge_nativo_DartForge_typed_setUint16",
    "dartforge_nativo_DartForge_typed_setInt32",
    "dartforge_nativo_DartForge_typed_setUint32",
    "dartforge_nativo_DartForge_typed_setInt64",
    "dartforge_nativo_DartForge_typed_setUint64",
    "dartforge_nativo_DartForge_typed_indexar_int",
    "dartforge_nativo_DartForge_typed_indexar_double",
    "dartforge_nativo_TypedData_GetFloat32",
    "dartforge_nativo_TypedData_GetFloat64",
    "dartforge_nativo_TypedData_SetFloat32",
    "dartforge_nativo_TypedData_SetFloat64",
    "dartforge_nativo_DartForge_string_codeUnitAt",
];

/// Os natives de resultado escalar com contrato: Ref emprestado, saída pelos
/// efeitos (`efeitos.tsv`).
const NATIVOS_ESCALARES: &[&str] = &[
    "dartforge_nativo_Closure_computeHash",
    "dartforge_nativo_Closure_equals",
    "dartforge_nativo_DartApiDLInitializeData",
    "dartforge_nativo_DartApiDLMajorVersion",
    "dartforge_nativo_DartApiDLMinorVersion",
    "dartforge_nativo_DartForge_Timer_novo",
    "dartforge_nativo_DartForge_capacidade_nova",
    "dartforge_nativo_DartForge_double_ceil",
    "dartforge_nativo_DartForge_double_floor",
    "dartforge_nativo_DartForge_double_hashCode",
    "dartforge_nativo_DartForge_double_modulo",
    "dartforge_nativo_DartForge_double_remainder",
    "dartforge_nativo_DartForge_double_round",
    "dartforge_nativo_DartForge_double_toInt",
    "dartforge_nativo_DartForge_double_truncate",
    "dartforge_nativo_DartForge_ffi_abi",
    "dartforge_nativo_DartForge_ffi_callback_novo",
    "dartforge_nativo_DartForge_ffi_carregar_abi",
    "dartforge_nativo_DartForge_ffi_carregar_f32",
    "dartforge_nativo_DartForge_ffi_carregar_f64",
    "dartforge_nativo_DartForge_ffi_carregar_i16",
    "dartforge_nativo_DartForge_ffi_carregar_i32",
    "dartforge_nativo_DartForge_ffi_carregar_i64",
    "dartforge_nativo_DartForge_ffi_carregar_i8",
    "dartforge_nativo_DartForge_ffi_carregar_u16",
    "dartforge_nativo_DartForge_ffi_carregar_u32",
    "dartforge_nativo_DartForge_ffi_carregar_u64",
    "dartforge_nativo_DartForge_ffi_carregar_u8",
    "dartforge_nativo_DartForge_ffi_tamanho",
    "dartforge_nativo_DartForge_hash_conjunto_adicionar",
    "dartforge_nativo_DartForge_hash_conjunto_preencher",
    "dartforge_nativo_DartForge_hash_mapa_gravar",
    "dartforge_nativo_DartForge_hash_mapa_preencher",
    "dartforge_nativo_DartForge_int_hashCode",
    "dartforge_nativo_DartForge_json_empilhar",
    "dartforge_nativo_DartForge_json_marca",
    "dartforge_nativo_DartForge_json_truncar",
    "dartforge_nativo_DartForge_math_acos",
    "dartforge_nativo_DartForge_math_asin",
    "dartforge_nativo_DartForge_math_atan",
    "dartforge_nativo_DartForge_math_atan2",
    "dartforge_nativo_DartForge_math_cos",
    "dartforge_nativo_DartForge_math_exp",
    "dartforge_nativo_DartForge_math_log",
    "dartforge_nativo_DartForge_math_pow",
    "dartforge_nativo_DartForge_math_sin",
    "dartforge_nativo_DartForge_math_sqrt",
    "dartforge_nativo_DartForge_math_tan",
    "dartforge_nativo_DartForge_porta_abrir",
    "dartforge_nativo_DartForge_porta_atual",
    "dartforge_nativo_DartForge_porta_enviar",
    "dartforge_nativo_DartForge_porta_mantem_vivo",
    "dartforge_nativo_DartForge_regexp_captura",
    "dartforge_nativo_DartForge_regexp_executar",
    "dartforge_nativo_DartForge_regexp_grupos",
    "dartforge_nativo_DartForge_regexp_indice_do_nome",
    "dartforge_nativo_DartForge_regexp_n_nomes",
    "dartforge_nativo_DartForge_sb_escrever_se_texto",
    "dartforge_nativo_DartForge_string_igual_a",
    "dartforge_nativo_DartForge_string_indice_de",
    "dartforge_nativo_DartForge_string_regiao_igual",
    "dartforge_nativo_DartForge_string_ultimo_indice_de",
    "dartforge_nativo_DartForge_tls_contexto_chave",
    "dartforge_nativo_DartForge_tls_contexto_versao_minima",
    "dartforge_nativo_DartForge_tls_filtro_certificado_do_par",
    "dartforge_nativo_DartForge_tls_filtro_certificado_pendente",
    "dartforge_nativo_DartForge_tls_filtro_handshake",
    "dartforge_nativo_DartForge_tls_filtro_processar",
    "dartforge_nativo_DartForge_tls_x509_fim",
    "dartforge_nativo_DartForge_tls_x509_inicio",
    "dartforge_nativo_DartForge_verdadeiro",
    "dartforge_nativo_DartNativeApiFunctionPointer",
    "dartforge_nativo_DateTime_currentTimeMicros",
    "dartforge_nativo_DateTime_timeZoneOffsetInSeconds",
    "dartforge_nativo_Developer_getServiceMajorVersion",
    "dartforge_nativo_Developer_getServiceMinorVersion",
    "dartforge_nativo_Directory_GetAsyncDirectoryListerPointer",
    "dartforge_nativo_Double_add",
    "dartforge_nativo_Double_div",
    "dartforge_nativo_Double_doubleFromInteger",
    "dartforge_nativo_Double_equal",
    "dartforge_nativo_Double_equalToInteger",
    "dartforge_nativo_Double_flipSignBit",
    "dartforge_nativo_Double_getIsInfinite",
    "dartforge_nativo_Double_getIsNaN",
    "dartforge_nativo_Double_getIsNegative",
    "dartforge_nativo_Double_greaterThan",
    "dartforge_nativo_Double_greaterThanFromInteger",
    "dartforge_nativo_Double_mul",
    "dartforge_nativo_Double_sub",
    "dartforge_nativo_EventHandler_TimerMillisecondClock",
    "dartforge_nativo_Ffi_dl_executableLibrary",
    "dartforge_nativo_Ffi_dl_lookup",
    "dartforge_nativo_Ffi_dl_open",
    "dartforge_nativo_Ffi_dl_processLibrary",
    "dartforge_nativo_Ffi_dl_providesSymbol",
    "dartforge_nativo_FileSystemWatcher_GetSocketId",
    "dartforge_nativo_FileSystemWatcher_InitWatcher",
    "dartforge_nativo_FileSystemWatcher_IsSupported",
    "dartforge_nativo_FileSystemWatcher_WatchPath",
    "dartforge_nativo_File_Close",
    "dartforge_nativo_File_GetFD",
    "dartforge_nativo_File_GetPointer",
    "dartforge_nativo_File_OpenStdio",
    "dartforge_nativo_Float32x4_getSignMask",
    "dartforge_nativo_Float32x4_getW",
    "dartforge_nativo_Float32x4_getX",
    "dartforge_nativo_Float32x4_getY",
    "dartforge_nativo_Float32x4_getZ",
    "dartforge_nativo_Float64x2_getSignMask",
    "dartforge_nativo_Float64x2_getX",
    "dartforge_nativo_Float64x2_getY",
    "dartforge_nativo_Int32x4_getFlagW",
    "dartforge_nativo_Int32x4_getFlagX",
    "dartforge_nativo_Int32x4_getFlagY",
    "dartforge_nativo_Int32x4_getFlagZ",
    "dartforge_nativo_Int32x4_getSignMask",
    "dartforge_nativo_Int32x4_getW",
    "dartforge_nativo_Int32x4_getX",
    "dartforge_nativo_Int32x4_getY",
    "dartforge_nativo_Int32x4_getZ",
    "dartforge_nativo_Integer_addFromInteger",
    "dartforge_nativo_Integer_bitAndFromInteger",
    "dartforge_nativo_Integer_bitOrFromInteger",
    "dartforge_nativo_Integer_bitXorFromInteger",
    "dartforge_nativo_Integer_equalToInteger",
    "dartforge_nativo_Integer_greaterThanFromInteger",
    "dartforge_nativo_Integer_moduloFromInteger",
    "dartforge_nativo_Integer_mulFromInteger",
    "dartforge_nativo_Integer_shlFromInteger",
    "dartforge_nativo_Integer_shrFromInteger",
    "dartforge_nativo_Integer_subFromInteger",
    "dartforge_nativo_Integer_truncDivFromInteger",
    "dartforge_nativo_Integer_ushrFromInteger",
    "dartforge_nativo_Mint_bitLength",
    "dartforge_nativo_Mint_bitNegate",
    "dartforge_nativo_Namespace_GetDefault",
    "dartforge_nativo_Namespace_GetPointer",
    "dartforge_nativo_OSError_inProgressErrorCode",
    "dartforge_nativo_Object_equals",
    "dartforge_nativo_Object_getHash",
    "dartforge_nativo_Platform_NumberOfProcessors",
    "dartforge_nativo_Process_GetExitCode",
    "dartforge_nativo_Process_KillPid",
    "dartforge_nativo_Process_Pid",
    "dartforge_nativo_Process_Start",
    "dartforge_nativo_Random_initialSeed",
    "dartforge_nativo_RawSocketOption_GetOptionValue",
    "dartforge_nativo_SecureRandom_getBytes",
    "dartforge_nativo_SendPort_get_id",
    "dartforge_nativo_ServerSocket_Accept",
    "dartforge_nativo_Smi_bitLength",
    "dartforge_nativo_Smi_bitNegate",
    "dartforge_nativo_SocketBase_IsBindError",
    "dartforge_nativo_Socket_Available",
    "dartforge_nativo_Socket_AvailableDatagram",
    "dartforge_nativo_Socket_GetFD",
    "dartforge_nativo_Socket_GetSocketId",
    "dartforge_nativo_Socket_GetStdioHandle",
    "dartforge_nativo_Socket_HasPendingWrite",
    "dartforge_nativo_Socket_SendTo",
    "dartforge_nativo_Socket_WriteList",
    "dartforge_nativo_Stopwatch_frequency",
    "dartforge_nativo_Stopwatch_now",
    "dartforge_nativo_Timeline_getNextTaskId",
    "dartforge_nativo_Timeline_getTraceClock",
    "dartforge_nativo_Timeline_isDartStreamEnabled",
    "dartforge_nativo_Uri_isWindowsPlatform",
];

/// Os natives sem resultado com contrato, gerados do mesmo jeito.
const NATIVOS_SEM_RESULTADO: &[&str] = &[
    "dartforge_nativo_DartForge_GrowableList_reservar",
    "dartforge_nativo_DartForge_List_copiar",
    "dartforge_nativo_DartForge_List_preencher",
    "dartforge_nativo_DartForge_Timer_cancelar",
    "dartforge_nativo_DartForge_classe_nao_enviavel",
    "dartforge_nativo_DartForge_classe_transferivel",
    "dartforge_nativo_DartForge_ffi_callback_apagar",
    "dartforge_nativo_DartForge_ffi_callback_manter",
    "dartforge_nativo_DartForge_ffi_copiar",
    "dartforge_nativo_DartForge_ffi_gravar_abi",
    "dartforge_nativo_DartForge_ffi_gravar_f32",
    "dartforge_nativo_DartForge_ffi_gravar_f64",
    "dartforge_nativo_DartForge_ffi_gravar_i16",
    "dartforge_nativo_DartForge_ffi_gravar_i32",
    "dartforge_nativo_DartForge_ffi_gravar_i64",
    "dartforge_nativo_DartForge_ffi_gravar_i8",
    "dartforge_nativo_DartForge_ffi_gravar_u16",
    "dartforge_nativo_DartForge_ffi_gravar_u32",
    "dartforge_nativo_DartForge_ffi_gravar_u64",
    "dartforge_nativo_DartForge_ffi_gravar_u8",
    "dartforge_nativo_DartForge_finalizador_anexar",
    "dartforge_nativo_DartForge_finalizador_anexar_nativo",
    "dartforge_nativo_DartForge_finalizador_desanexar",
    "dartforge_nativo_DartForge_imprimir",
    "dartforge_nativo_DartForge_porta_fechar",
    "dartforge_nativo_DartForge_porta_manter_vivo",
    "dartforge_nativo_DartForge_portas_despachante",
    "dartforge_nativo_DartForge_sb_escrever",
    "dartforge_nativo_DartForge_sb_escrever_codigo",
    "dartforge_nativo_DartForge_scheduleImmediate",
    "dartforge_nativo_DartForge_string_dividir_por_codigo",
    "dartforge_nativo_DartForge_tls_contexto_alpn",
    "dartforge_nativo_DartForge_tls_contexto_definir_versao_minima",
    "dartforge_nativo_DartForge_tls_contexto_novo",
    "dartforge_nativo_DartForge_tls_contexto_raizes_embutidas",
    "dartforge_nativo_DartForge_tls_filtro_decidir",
    "dartforge_nativo_DartForge_tls_filtro_destruir",
    "dartforge_nativo_DartForge_tls_filtro_novo",
    "dartforge_nativo_DartForge_tls_filtro_registrar_chaves",
    "dartforge_nativo_DartForge_tls_x509_adotar",
    "dartforge_nativo_DartForge_typed_memMove1",
    "dartforge_nativo_DartForge_typed_memMove16",
    "dartforge_nativo_DartForge_typed_memMove2",
    "dartforge_nativo_DartForge_typed_memMove4",
    "dartforge_nativo_DartForge_typed_memMove8",
    "dartforge_nativo_Developer_getServerInfo",
    "dartforge_nativo_Developer_webServerControl",
    "dartforge_nativo_Directory_FillWithDirectoryListing",
    "dartforge_nativo_Directory_SetAsyncDirectoryListerPointer",
    "dartforge_nativo_Error_trySetStackTrace",
    "dartforge_nativo_EventHandler_SendData",
    "dartforge_nativo_Ffi_dl_close",
    "dartforge_nativo_FileSystemWatcher_CloseWatcher",
    "dartforge_nativo_FileSystemWatcher_UnwatchPath",
    "dartforge_nativo_File_SetPointer",
    "dartforge_nativo_Filter_CreateZLibDeflate",
    "dartforge_nativo_Filter_CreateZLibInflate",
    "dartforge_nativo_Filter_Process",
    "dartforge_nativo_GrowableList_setData",
    "dartforge_nativo_GrowableList_setIndexed",
    "dartforge_nativo_Isolate_sendOOB",
    "dartforge_nativo_Isolate_spawnFunction",
    "dartforge_nativo_Isolate_spawnUri",
    "dartforge_nativo_List_setIndexed",
    "dartforge_nativo_Process_ClearSignalHandler",
    "dartforge_nativo_Process_SetExitCode",
    "dartforge_nativo_Process_Sleep",
    "dartforge_nativo_Socket_GetRawOption",
    "dartforge_nativo_Socket_JoinMulticast",
    "dartforge_nativo_Socket_LeaveMulticast",
    "dartforge_nativo_Socket_SetOption",
    "dartforge_nativo_Socket_SetRawOption",
    "dartforge_nativo_Socket_SetSocketId",
    "dartforge_nativo_Timeline_reportTaskEvent",
    "dartforge_nativo_TypedDataBase_setClampedRange",
    "dartforge_nativo_TypedData_SetFloat32x4",
    "dartforge_nativo_TypedData_SetFloat64x2",
    "dartforge_nativo_TypedData_SetInt32x4",
    "dartforge_nativo_WeakProperty_setKey",
    "dartforge_nativo_WeakProperty_setValue",
    "dartforge_nativo_WeakReference_setTarget",
];

#[test]
fn contratos_arc_cobrem_exportacoes_sem_inventar_contratos_para_outras_externs() {
    let auditadas: Vec<_> = NOMES
        .iter()
        .filter(|n| {
            n.starts_with("dartforge_arc_")
                || ACESSOS_TIPADOS.contains(n)
                || NATIVOS_ESCALARES.contains(n)
                || NATIVOS_SEM_RESULTADO.contains(n)
                || matches!(
                    **n,
                    "dartforge_gc_global_root"
                        | "dartforge_marcar_constante"
                        | "dartforge_gc_collect"
                        | "dartforge_marcar_permanente"
                        | "dartforge_nativo_DartForge_record_fieldAt"
                        | "dartforge_nativo_DartForge_record_numFields"
                        | "dartforge_nativo_DartForge_record_shape"
                        | "dartforge_print_handle"
                        | "dartforge_exception_pending"
                        | "dartforge_interrupcao_pendente"
                        | "dartforge_ponto_seguro"
                        | "dartforge_exception_clear"
                        | "dartforge_exception_capturavel"
                        | "dartforge_membro_recusado"
                        | "dartforge_args_casam"
                        | "dartforge_late_field_initialized"
                        | "dartforge_late_field_initializing"
                        | "dartforge_late_field_mark_initialized"
                        | "dartforge_late_field_set_initializing"
                        | "dartforge_arg_indice"
                        | "dartforge_rti_definir"
                        | "dartforge_rti_registro_nomeado"
                        | "dartforge_equal"
                        | "dartforge_rti_receita"
                        | "dartforge_rti_avaliar"
                        | "dartforge_rti_e"
                        | "dartforge_rti_como"
                        | "dartforge_rti_como_em"
                        | "dartforge_is_subclass"
                        | "dartforge_nsm_chamada"
                        | "dartforge_unbox_int"
                        | "dartforge_unbox_double"
                        | "dartforge_unbox_bool"
                        | "dartforge_value_class"
                        | "dartforge_lista_acrescentar"
                        | "dartforge_rti_iniciar_tabela"
                        | "dartforge_ffi_endereco_da_closure"
                        | "dartforge_ffi_endereco_do_ponteiro"
                        | "dartforge_ffi_endereco_do_composto"
                        | "dartforge_ffi_simbolo_nativo"
                        | "dartforge_rti_tupla_juntar"
                        | "dartforge_ffi_handles_abrir"
                        | "dartforge_ffi_handle_novo"
                        | "dartforge_ffi_handles_fechar"
                        | "dartforge_rastro_entrar"
                        | "dartforge_rastro_sair"
                )
        })
        .collect();
    assert_eq!(auditadas.len(), CONTRATOS.len());
    for nome in auditadas {
        let c = CONTRATOS.iter().find(|c| c.nome == *nome).unwrap();
        let efeito = EFEITOS.iter().find(|e| e.0 == c.nome).unwrap();
        assert_eq!(efeito.2, c.pode_falhar);
        assert_eq!(efeito.3, c.chama_dart);
        assert!(!c.chama_dart || c.invalida_borrows);
    }
    let receber = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_global_receber_v1")
        .unwrap();
    assert_eq!(
        receber.parametros,
        &[ModoParametro::Native, ModoParametro::Consume]
    );
    assert!(receber.retencao_persistente && receber.invalida_borrows);
    let carregar = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_quadro_carregar_v1")
        .unwrap();
    assert_eq!(carregar.resultado, ModoResultado::Owned);
    let caixa = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_box_int_owned_v1")
        .unwrap();
    assert_eq!(caixa.parametros, &[ModoParametro::Scalar]);
    assert_eq!(caixa.resultado, ModoResultado::Owned);
    assert!(!caixa.pode_falhar && !caixa.retencao_persistente);
    assert!(caixa.invalida_borrows);
    assert!(EFEITOS.iter().find(|e| e.0 == caixa.nome).unwrap().1);
    let instancia = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_objeto_owned_v1")
        .unwrap();
    assert_eq!(
        instancia.parametros,
        &[ModoParametro::Native, ModoParametro::Scalar]
    );
    assert_eq!(instancia.resultado, ModoResultado::Owned);
    assert!(instancia.invalida_borrows);
    assert!(!instancia.pode_falhar && !instancia.chama_dart && !instancia.retencao_persistente);
    assert!(EFEITOS.iter().find(|e| e.0 == instancia.nome).unwrap().1);
    let escalar = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_ler_campo_escalar_v1")
        .unwrap();
    assert_eq!(
        escalar.parametros,
        &[ModoParametro::Borrow, ModoParametro::Scalar]
    );
    assert_eq!(escalar.resultado, ModoResultado::ScalarI64);
    assert!(
        !escalar.pode_falhar
            && !escalar.chama_dart
            && !escalar.retencao_persistente
            && !escalar.invalida_borrows
    );
    for (nome, valor, retencao) in [
        (
            "dartforge_arc_gravar_campo_escalar_v1",
            ModoParametro::Scalar,
            false,
        ),
        (
            "dartforge_arc_gravar_campo_ref_v1",
            ModoParametro::Borrow,
            true,
        ),
    ] {
        let c = CONTRATOS.iter().find(|c| c.nome == nome).unwrap();
        assert_eq!(
            c.parametros,
            &[ModoParametro::Borrow, ModoParametro::Scalar, valor]
        );
        assert_eq!(c.resultado, ModoResultado::Void);
        assert_eq!(c.retencao_persistente, retencao);
        assert!(c.invalida_borrows && !c.pode_falhar && !c.chama_dart);
        assert!(!EFEITOS.iter().find(|e| e.0 == nome).unwrap().1);
    }
    let lancar = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_lancar_ref_v1")
        .unwrap();
    let leitura_ref = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_ler_campo_ref_v1")
        .unwrap();
    assert_eq!(
        leitura_ref.parametros,
        &[ModoParametro::Borrow, ModoParametro::Scalar]
    );
    assert_eq!(leitura_ref.resultado, ModoResultado::BorrowArg(0));
    assert!(
        !leitura_ref.pode_falhar
            && !leitura_ref.chama_dart
            && !leitura_ref.retencao_persistente
            && !leitura_ref.invalida_borrows
    );
    assert!(!EFEITOS.iter().find(|e| e.0 == leitura_ref.nome).unwrap().1);
    assert_eq!(lancar.parametros, &[ModoParametro::Borrow]);
    assert_eq!(lancar.resultado, ModoResultado::Void);
    assert!(lancar.pode_falhar && lancar.retencao_persistente && lancar.invalida_borrows);
    let limpar = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_exception_clear")
        .unwrap();
    assert!(limpar.parametros.is_empty());
    assert!(!limpar.pode_falhar && !limpar.retencao_persistente);
    assert!(limpar.invalida_borrows);
    let campo = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_nativo_DartForge_record_fieldAt")
        .unwrap();
    assert_eq!(campo.resultado, ModoResultado::BorrowArg(0));
    // Leituras do record: não alocam nem lançam (o lowering não confere).
    assert!(!campo.pode_falhar);
    for nome in [
        "dartforge_nativo_DartForge_record_numFields",
        "dartforge_nativo_DartForge_record_shape",
    ] {
        let c = CONTRATOS.iter().find(|c| c.nome == nome).unwrap();
        assert_eq!(c.parametros, &[ModoParametro::Borrow]);
        assert_eq!(c.resultado, ModoResultado::ScalarI64);
        assert!(!c.pode_falhar);
        assert!(!c.retencao_persistente && !c.invalida_borrows);
    }
    let imprimir = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_print_handle")
        .unwrap();
    assert_eq!(imprimir.parametros, &[ModoParametro::Borrow]);
    assert_eq!(imprimir.resultado, ModoResultado::Void);
    assert!(imprimir.pode_falhar);
    assert!(!imprimir.retencao_persistente && !imprimir.invalida_borrows);
    assert!(!CONTRATOS.iter().any(|c| c.nome == "dartforge_alocar"));
    for nome in ["dartforge_gc_global_root", "dartforge_marcar_constante"] {
        let c = CONTRATOS.iter().find(|c| c.nome == nome).unwrap();
        assert!(c.retencao_persistente);
        assert!(!c.parametros.contains(&ModoParametro::Consume));
    }
}

#[test]
fn captura_emprestada_copiada_sobrevive_ao_ambiente_e_preserva_pendencia() {
    use dartforge_runtime::{abi::*, layout::cid};
    std::thread::spawn(|| {
        dartforge_memoria_arc_v1();
        let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
        let env = dartforge_arc_objeto_owned_v1(i64::from(cid::CONTEXTO), 40);
        dartforge_arc_gravar_campo_ref_v1(env, 39, filho);
        dartforge_arc_release(filho);
        dartforge_exception_throw(42, 1);
        let pendente = dartforge_exception_peek_ref();
        let captura = dartforge_arc_env_ref_v1(env, 39);
        assert_eq!(captura, filho);
        assert_eq!(dartforge_arc_observar_heap_v1(captura), 1);
        assert_ne!(dartforge_exception_pending(), 0);
        assert_eq!(dartforge_exception_peek_ref(), pendente);
        dartforge_arc_retain(captura);
        dartforge_arc_release(env);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observar_heap_v1(env), 0);
        assert_eq!(dartforge_arc_observar_heap_v1(captura), 3);
        assert_eq!(dartforge_unbox_int(captura), i64::MAX);
        dartforge_arc_release(captura);
        dartforge_exception_clear();
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observar_heap_v1(captura), 0);
    }).join().unwrap();
}
