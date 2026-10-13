//! The partial table must tell references from scalars and the absence of a contract.

// Includes the generator's rejections in the Cargo suite, without running its main.
#[allow(dead_code)]
#[path = "../build.rs"]
mod gerador;

use dartforge_runtime::{
    efeitos::EFEITOS,
    ownership::{CONTRATOS, ModoParametro, ModoResultado},
    simbolos::NOMES,
};

#[test]
fn owned_closure_keeps_context_cell_and_repeated_edges_until_drop() {
    use dartforge_runtime::{abi::*, layout::cid};
    extern "C" fn input(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
    extern "C" fn body(_: i64) -> i64 { 7 }
    std::thread::spawn(|| {
        dartforge_memory_arc_v1();
        for cycle in [false, true] {
            let value = dartforge_arc_box_int_owned_v1(i64::MAX);
            let cell = dartforge_arc_object_owned_v1(i64::from(cid::CELULA), 1);
            dartforge_arc_write_field_ref_v1(cell, 0, value);
            dartforge_arc_release(value);
            let env = dartforge_arc_object_owned_v1(i64::from(cid::CONTEXTO), 40);
            for i in [0, 39] { dartforge_arc_write_field_ref_v1(env, i, cell); }
            dartforge_arc_release(cell);
            let code = input as *const () as usize as i64;
            let typed_one = body as *const () as usize as i64;
            let closure = dartforge_arc_closure_owned_v1(code, env, typed_one, 7);
            assert_eq!(dartforge_arc_observe_heap_v1(env), 3);
            dartforge_arc_release(env);
            if cycle { dartforge_arc_write_field_ref_v1(env, 38, closure); }
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(closure), 3);
            for h in [env, cell, value] { assert_eq!(dartforge_arc_observe_heap_v1(h), 1); }
            assert_eq!(dartforge_closure_code(closure), code);
            assert_eq!(dartforge_closure_typed(closure, 7), typed_one);
            assert_eq!(dartforge_arc_closure_context_v1(closure), env);
            assert_eq!(dartforge_arc_env_ref_v1(env, 39), cell);
            assert_eq!(dartforge_unbox_int(dartforge_arc_read_field_ref_v1(cell, 0)), i64::MAX);
            dartforge_arc_release(closure);
            dartforge_arc_collect();
            for h in [closure, env, cell, value] { assert_eq!(dartforge_arc_observe_heap_v1(h), 0); }
        }
    }).join().unwrap();
}

#[test]
fn borrowed_context_needs_owner_after_closure_release() {
    use dartforge_runtime::abi::*;
    extern "C" fn input(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
    std::thread::spawn(|| {
        dartforge_memory_arc_v1();
        let child = dartforge_arc_box_int_owned_v1(i64::MAX);
        let closure = dartforge_arc_closure_owned_v1(input as *const () as usize as i64, child, 0, 0);
        dartforge_arc_release(child);
        let borrowed = dartforge_arc_closure_context_v1(closure);
        assert_eq!(borrowed, child);
        assert_eq!(dartforge_arc_observe_heap_v1(borrowed), 1);
        dartforge_arc_retain(borrowed);
        dartforge_arc_release(closure);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observe_heap_v1(closure), 0);
        assert_eq!(dartforge_unbox_int(borrowed), i64::MAX);
        assert_eq!(dartforge_arc_observe_heap_v1(borrowed), 3);
        dartforge_arc_release(borrowed);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observe_heap_v1(borrowed), 0);
        let canonical_one = dartforge_tearoff(123);
        assert_eq!(dartforge_arc_closure_context_v1(canonical_one), 0);
    }).join().unwrap();
}

#[test]
fn nsm_publishes_error_until_clear_without_owner_in_caller() {
    use dartforge_runtime::abi::*;
    std::thread::spawn(|| {
        dartforge_memory_arc_v1();
        dartforge_nsm_call();
        assert_ne!(dartforge_exception_pending(), 0);
        let error = dartforge_exception_peek_ref();
        assert_ne!(error, 0);
        dartforge_arc_collect();
        assert_eq!(dartforge_exception_peek_ref(), error);
        assert_eq!(dartforge_arc_observe_heap_v1(error), 1);
        dartforge_exception_clear();
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observe_heap_v1(error), 0);
    }).join().unwrap();
}

/// The indexed and byte accesses of typed lists and `codeUnitAt` (the runtime
/// paths under ARC): they borrow the receiver and only throw `RangeError`.
const TYPED_ACCESSES: &[&str] = &[
    "dartforge_native_DartForge_typed_getInt8",
    "dartforge_native_DartForge_typed_getUint8",
    "dartforge_native_DartForge_typed_getInt16",
    "dartforge_native_DartForge_typed_getUint16",
    "dartforge_native_DartForge_typed_getInt32",
    "dartforge_native_DartForge_typed_getUint32",
    "dartforge_native_DartForge_typed_getInt64",
    "dartforge_native_DartForge_typed_getUint64",
    "dartforge_native_DartForge_typed_setInt8",
    "dartforge_native_DartForge_typed_setUint8",
    "dartforge_native_DartForge_typed_setInt16",
    "dartforge_native_DartForge_typed_setUint16",
    "dartforge_native_DartForge_typed_setInt32",
    "dartforge_native_DartForge_typed_setUint32",
    "dartforge_native_DartForge_typed_setInt64",
    "dartforge_native_DartForge_typed_setUint64",
    "dartforge_native_DartForge_typed_index_int",
    "dartforge_native_DartForge_typed_index_double",
    "dartforge_native_TypedData_GetFloat32",
    "dartforge_native_TypedData_GetFloat64",
    "dartforge_native_TypedData_SetFloat32",
    "dartforge_native_TypedData_SetFloat64",
    "dartforge_native_DartForge_string_codeUnitAt",
];

/// The natives with a scalar result and a contract: borrowed Ref, exit through
/// the effects (`effects.tsv`).
const SCALAR_NATIVES: &[&str] = &[
    "dartforge_native_Closure_computeHash",
    "dartforge_native_Closure_equals",
    "dartforge_native_DartApiDLInitializeData",
    "dartforge_native_DartApiDLMajorVersion",
    "dartforge_native_DartApiDLMinorVersion",
    "dartforge_native_DartForge_Timer_new",
    "dartforge_native_DartForge_capability_new",
    "dartforge_native_DartForge_double_ceil",
    "dartforge_native_DartForge_double_floor",
    "dartforge_native_DartForge_double_hashCode",
    "dartforge_native_DartForge_double_modulo",
    "dartforge_native_DartForge_double_remainder",
    "dartforge_native_DartForge_double_round",
    "dartforge_native_DartForge_double_toInt",
    "dartforge_native_DartForge_double_truncate",
    "dartforge_native_DartForge_ffi_abi",
    "dartforge_native_DartForge_ffi_callback_new",
    "dartforge_native_DartForge_ffi_load_abi",
    "dartforge_native_DartForge_ffi_load_f32",
    "dartforge_native_DartForge_ffi_load_f64",
    "dartforge_native_DartForge_ffi_load_i16",
    "dartforge_native_DartForge_ffi_load_i32",
    "dartforge_native_DartForge_ffi_load_i64",
    "dartforge_native_DartForge_ffi_load_i8",
    "dartforge_native_DartForge_ffi_load_u16",
    "dartforge_native_DartForge_ffi_load_u32",
    "dartforge_native_DartForge_ffi_load_u64",
    "dartforge_native_DartForge_ffi_load_u8",
    "dartforge_native_DartForge_ffi_size_of",
    "dartforge_native_DartForge_hash_set_add",
    "dartforge_native_DartForge_hash_set_fill",
    "dartforge_native_DartForge_hash_map_store",
    "dartforge_native_DartForge_hash_map_fill",
    "dartforge_native_DartForge_int_hashCode",
    "dartforge_native_DartForge_json_push",
    "dartforge_native_DartForge_json_mark",
    "dartforge_native_DartForge_json_truncate",
    "dartforge_native_DartForge_math_acos",
    "dartforge_native_DartForge_math_asin",
    "dartforge_native_DartForge_math_atan",
    "dartforge_native_DartForge_math_atan2",
    "dartforge_native_DartForge_math_cos",
    "dartforge_native_DartForge_math_exp",
    "dartforge_native_DartForge_math_log",
    "dartforge_native_DartForge_math_pow",
    "dartforge_native_DartForge_math_sin",
    "dartforge_native_DartForge_math_sqrt",
    "dartforge_native_DartForge_math_tan",
    "dartforge_native_DartForge_port_open",
    "dartforge_native_DartForge_port_current",
    "dartforge_native_DartForge_port_send",
    "dartforge_native_DartForge_port_keeps_alive",
    "dartforge_native_DartForge_regexp_capture",
    "dartforge_native_DartForge_regexp_exec",
    "dartforge_native_DartForge_regexp_groups",
    "dartforge_native_DartForge_regexp_name_index",
    "dartforge_native_DartForge_regexp_n_names",
    "dartforge_native_DartForge_sb_write_if_string",
    "dartforge_native_DartForge_string_equals_object",
    "dartforge_native_DartForge_string_index_of",
    "dartforge_native_DartForge_string_region_equals",
    "dartforge_native_DartForge_string_last_index_of",
    "dartforge_native_DartForge_tls_context_key",
    "dartforge_native_DartForge_tls_context_min_version",
    "dartforge_native_DartForge_tls_filter_peer_certificate",
    "dartforge_native_DartForge_tls_filter_pending_certificate",
    "dartforge_native_DartForge_tls_filter_handshake",
    "dartforge_native_DartForge_tls_filter_process",
    "dartforge_native_DartForge_tls_x509_end",
    "dartforge_native_DartForge_tls_x509_start",
    "dartforge_native_DartForge_true",
    "dartforge_native_DartNativeApiFunctionPointer",
    "dartforge_native_DateTime_currentTimeMicros",
    "dartforge_native_DateTime_timeZoneOffsetInSeconds",
    "dartforge_native_Developer_getServiceMajorVersion",
    "dartforge_native_Developer_getServiceMinorVersion",
    "dartforge_native_Directory_GetAsyncDirectoryListerPointer",
    "dartforge_native_Double_add",
    "dartforge_native_Double_div",
    "dartforge_native_Double_doubleFromInteger",
    "dartforge_native_Double_equal",
    "dartforge_native_Double_equalToInteger",
    "dartforge_native_Double_flipSignBit",
    "dartforge_native_Double_getIsInfinite",
    "dartforge_native_Double_getIsNaN",
    "dartforge_native_Double_getIsNegative",
    "dartforge_native_Double_greaterThan",
    "dartforge_native_Double_greaterThanFromInteger",
    "dartforge_native_Double_mul",
    "dartforge_native_Double_sub",
    "dartforge_native_EventHandler_TimerMillisecondClock",
    "dartforge_native_Ffi_dl_executableLibrary",
    "dartforge_native_Ffi_dl_lookup",
    "dartforge_native_Ffi_dl_open",
    "dartforge_native_Ffi_dl_processLibrary",
    "dartforge_native_Ffi_dl_providesSymbol",
    "dartforge_native_FileSystemWatcher_GetSocketId",
    "dartforge_native_FileSystemWatcher_InitWatcher",
    "dartforge_native_FileSystemWatcher_IsSupported",
    "dartforge_native_FileSystemWatcher_WatchPath",
    "dartforge_native_File_Close",
    "dartforge_native_File_GetFD",
    "dartforge_native_File_GetPointer",
    "dartforge_native_File_OpenStdio",
    "dartforge_native_Float32x4_getSignMask",
    "dartforge_native_Float32x4_getW",
    "dartforge_native_Float32x4_getX",
    "dartforge_native_Float32x4_getY",
    "dartforge_native_Float32x4_getZ",
    "dartforge_native_Float64x2_getSignMask",
    "dartforge_native_Float64x2_getX",
    "dartforge_native_Float64x2_getY",
    "dartforge_native_Int32x4_getFlagW",
    "dartforge_native_Int32x4_getFlagX",
    "dartforge_native_Int32x4_getFlagY",
    "dartforge_native_Int32x4_getFlagZ",
    "dartforge_native_Int32x4_getSignMask",
    "dartforge_native_Int32x4_getW",
    "dartforge_native_Int32x4_getX",
    "dartforge_native_Int32x4_getY",
    "dartforge_native_Int32x4_getZ",
    "dartforge_native_Integer_addFromInteger",
    "dartforge_native_Integer_bitAndFromInteger",
    "dartforge_native_Integer_bitOrFromInteger",
    "dartforge_native_Integer_bitXorFromInteger",
    "dartforge_native_Integer_equalToInteger",
    "dartforge_native_Integer_greaterThanFromInteger",
    "dartforge_native_Integer_moduloFromInteger",
    "dartforge_native_Integer_mulFromInteger",
    "dartforge_native_Integer_shlFromInteger",
    "dartforge_native_Integer_shrFromInteger",
    "dartforge_native_Integer_subFromInteger",
    "dartforge_native_Integer_truncDivFromInteger",
    "dartforge_native_Integer_ushrFromInteger",
    "dartforge_native_Mint_bitLength",
    "dartforge_native_Mint_bitNegate",
    "dartforge_native_Namespace_GetDefault",
    "dartforge_native_Namespace_GetPointer",
    "dartforge_native_OSError_inProgressErrorCode",
    "dartforge_native_Object_equals",
    "dartforge_native_Object_getHash",
    "dartforge_native_Platform_NumberOfProcessors",
    "dartforge_native_Process_GetExitCode",
    "dartforge_native_Process_KillPid",
    "dartforge_native_Process_Pid",
    "dartforge_native_Process_Start",
    "dartforge_native_Random_initialSeed",
    "dartforge_native_RawSocketOption_GetOptionValue",
    "dartforge_native_SecureRandom_getBytes",
    "dartforge_native_SendPort_get_id",
    "dartforge_native_ServerSocket_Accept",
    "dartforge_native_Smi_bitLength",
    "dartforge_native_Smi_bitNegate",
    "dartforge_native_SocketBase_IsBindError",
    "dartforge_native_Socket_Available",
    "dartforge_native_Socket_AvailableDatagram",
    "dartforge_native_Socket_GetFD",
    "dartforge_native_Socket_GetSocketId",
    "dartforge_native_Socket_GetStdioHandle",
    "dartforge_native_Socket_HasPendingWrite",
    "dartforge_native_Socket_SendTo",
    "dartforge_native_Socket_WriteList",
    "dartforge_native_Stopwatch_frequency",
    "dartforge_native_Stopwatch_now",
    "dartforge_native_Timeline_getNextTaskId",
    "dartforge_native_Timeline_getTraceClock",
    "dartforge_native_Timeline_isDartStreamEnabled",
    "dartforge_native_Uri_isWindowsPlatform",
];

/// The natives without a result that have a contract, generated the same way.
const NATIVES_WITHOUT_RESULT: &[&str] = &[
    "dartforge_native_DartForge_GrowableList_reserve",
    "dartforge_native_DartForge_List_copy",
    "dartforge_native_DartForge_List_fill",
    "dartforge_native_DartForge_Timer_cancel",
    "dartforge_native_DartForge_class_not_sendable",
    "dartforge_native_DartForge_class_transferable",
    "dartforge_native_DartForge_ffi_callback_delete",
    "dartforge_native_DartForge_ffi_callback_keep",
    "dartforge_native_DartForge_ffi_copy",
    "dartforge_native_DartForge_ffi_store_abi",
    "dartforge_native_DartForge_ffi_store_f32",
    "dartforge_native_DartForge_ffi_store_f64",
    "dartforge_native_DartForge_ffi_store_i16",
    "dartforge_native_DartForge_ffi_store_i32",
    "dartforge_native_DartForge_ffi_store_i64",
    "dartforge_native_DartForge_ffi_store_i8",
    "dartforge_native_DartForge_ffi_store_u16",
    "dartforge_native_DartForge_ffi_store_u32",
    "dartforge_native_DartForge_ffi_store_u64",
    "dartforge_native_DartForge_ffi_store_u8",
    "dartforge_native_DartForge_finalizer_attach",
    "dartforge_native_DartForge_finalizer_attach_native",
    "dartforge_native_DartForge_finalizer_detach",
    "dartforge_native_DartForge_print",
    "dartforge_native_DartForge_port_close",
    "dartforge_native_DartForge_port_set_keep_alive",
    "dartforge_native_DartForge_ports_dispatcher",
    "dartforge_native_DartForge_sb_write",
    "dartforge_native_DartForge_sb_write_char_code",
    "dartforge_native_DartForge_scheduleImmediate",
    "dartforge_native_DartForge_string_split_by_code",
    "dartforge_native_DartForge_tls_context_alpn",
    "dartforge_native_DartForge_tls_context_set_min_version",
    "dartforge_native_DartForge_tls_context_new",
    "dartforge_native_DartForge_tls_context_builtin_roots",
    "dartforge_native_DartForge_tls_filter_decide",
    "dartforge_native_DartForge_tls_filter_destroy",
    "dartforge_native_DartForge_tls_filter_new",
    "dartforge_native_DartForge_tls_filter_register_keys",
    "dartforge_native_DartForge_tls_x509_adopt",
    "dartforge_native_DartForge_typed_memMove1",
    "dartforge_native_DartForge_typed_memMove16",
    "dartforge_native_DartForge_typed_memMove2",
    "dartforge_native_DartForge_typed_memMove4",
    "dartforge_native_DartForge_typed_memMove8",
    "dartforge_native_Developer_getServerInfo",
    "dartforge_native_Developer_webServerControl",
    "dartforge_native_Directory_FillWithDirectoryListing",
    "dartforge_native_Directory_SetAsyncDirectoryListerPointer",
    "dartforge_native_Error_trySetStackTrace",
    "dartforge_native_EventHandler_SendData",
    "dartforge_native_Ffi_dl_close",
    "dartforge_native_FileSystemWatcher_CloseWatcher",
    "dartforge_native_FileSystemWatcher_UnwatchPath",
    "dartforge_native_File_SetPointer",
    "dartforge_native_Filter_CreateZLibDeflate",
    "dartforge_native_Filter_CreateZLibInflate",
    "dartforge_native_Filter_Process",
    "dartforge_native_GrowableList_setData",
    "dartforge_native_GrowableList_setIndexed",
    "dartforge_native_Isolate_sendOOB",
    "dartforge_native_Isolate_spawnFunction",
    "dartforge_native_Isolate_spawnUri",
    "dartforge_native_List_setIndexed",
    "dartforge_native_Process_ClearSignalHandler",
    "dartforge_native_Process_SetExitCode",
    "dartforge_native_Process_Sleep",
    "dartforge_native_Socket_GetRawOption",
    "dartforge_native_Socket_JoinMulticast",
    "dartforge_native_Socket_LeaveMulticast",
    "dartforge_native_Socket_SetOption",
    "dartforge_native_Socket_SetRawOption",
    "dartforge_native_Socket_SetSocketId",
    "dartforge_native_Timeline_reportTaskEvent",
    "dartforge_native_TypedDataBase_setClampedRange",
    "dartforge_native_TypedData_SetFloat32x4",
    "dartforge_native_TypedData_SetFloat64x2",
    "dartforge_native_TypedData_SetInt32x4",
    "dartforge_native_WeakProperty_setKey",
    "dartforge_native_WeakProperty_setValue",
    "dartforge_native_WeakReference_setTarget",
];

#[test]
fn arc_contracts_cover_exports_without_inventing_contracts_for_other_externs() {
    let audited: Vec<_> = NOMES
        .iter()
        .filter(|n| {
            n.starts_with("dartforge_arc_")
                || TYPED_ACCESSES.contains(n)
                || SCALAR_NATIVES.contains(n)
                || NATIVES_WITHOUT_RESULT.contains(n)
                || matches!(
                    **n,
                    "dartforge_gc_global_root"
                        | "dartforge_mark_constant"
                        | "dartforge_gc_collect"
                        | "dartforge_mark_permanent"
                        | "dartforge_native_DartForge_record_fieldAt"
                        | "dartforge_native_DartForge_record_numFields"
                        | "dartforge_native_DartForge_record_shape"
                        | "dartforge_print_handle"
                        | "dartforge_exception_pending"
                        | "dartforge_pending_interrupt"
                        | "dartforge_safepoint"
                        | "dartforge_exception_clear"
                        | "dartforge_exception_catchable"
                        | "dartforge_member_rejected"
                        | "dartforge_args_match"
                        | "dartforge_late_field_initialized"
                        | "dartforge_late_field_initializing"
                        | "dartforge_late_field_mark_initialized"
                        | "dartforge_late_field_set_initializing"
                        | "dartforge_arg_index"
                        | "dartforge_rti_set"
                        | "dartforge_rti_named_record"
                        | "dartforge_equal"
                        | "dartforge_rti_recipe"
                        | "dartforge_rti_eval"
                        | "dartforge_rti_e"
                        | "dartforge_rti_as"
                        | "dartforge_rti_as_with_context"
                        | "dartforge_is_subclass"
                        | "dartforge_nsm_call"
                        | "dartforge_unbox_int"
                        | "dartforge_unbox_double"
                        | "dartforge_unbox_bool"
                        | "dartforge_value_class"
                        | "dartforge_list_append"
                        | "dartforge_rti_init_table"
                        | "dartforge_ffi_closure_address"
                        | "dartforge_ffi_pointer_address"
                        | "dartforge_ffi_compound_address"
                        | "dartforge_ffi_native_symbol"
                        | "dartforge_rti_tuple_join"
                        | "dartforge_ffi_handles_open"
                        | "dartforge_ffi_handle_new"
                        | "dartforge_ffi_handles_close"
                        | "dartforge_stack_trace_enter"
                        | "dartforge_stack_trace_exit"
                        | "dartforge_typed_fill_int"
                        | "dartforge_typed_fill_double"
                )
        })
        .collect();
    assert_eq!(audited.len(), CONTRATOS.len());
    for nome in audited {
        let c = CONTRATOS.iter().find(|c| c.nome == *nome).unwrap();
        let effect = EFEITOS.iter().find(|e| e.0 == c.nome).unwrap();
        assert_eq!(effect.2, c.pode_falhar);
        assert_eq!(effect.3, c.chama_dart);
        assert!(!c.chama_dart || c.invalida_borrows);
    }
    let receber = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_global_receive_v1")
        .unwrap();
    assert_eq!(
        receber.parametros,
        &[ModoParametro::Native, ModoParametro::Consume]
    );
    assert!(receber.retencao_persistente && receber.invalida_borrows);
    let load = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_frame_load_v1")
        .unwrap();
    assert_eq!(load.resultado, ModoResultado::Owned);
    let boxed = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_box_int_owned_v1")
        .unwrap();
    assert_eq!(boxed.parametros, &[ModoParametro::Scalar]);
    assert_eq!(boxed.resultado, ModoResultado::Owned);
    assert!(!boxed.pode_falhar && !boxed.retencao_persistente);
    assert!(boxed.invalida_borrows);
    assert!(EFEITOS.iter().find(|e| e.0 == boxed.nome).unwrap().1);
    let instance = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_object_owned_v1")
        .unwrap();
    assert_eq!(
        instance.parametros,
        &[ModoParametro::Native, ModoParametro::Scalar]
    );
    assert_eq!(instance.resultado, ModoResultado::Owned);
    assert!(instance.invalida_borrows);
    assert!(!instance.pode_falhar && !instance.chama_dart && !instance.retencao_persistente);
    assert!(EFEITOS.iter().find(|e| e.0 == instance.nome).unwrap().1);
    let scalar = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_read_field_scalar_v1")
        .unwrap();
    assert_eq!(
        scalar.parametros,
        &[ModoParametro::Borrow, ModoParametro::Scalar]
    );
    assert_eq!(scalar.resultado, ModoResultado::ScalarI64);
    assert!(
        !scalar.pode_falhar
            && !scalar.chama_dart
            && !scalar.retencao_persistente
            && !scalar.invalida_borrows
    );
    for (nome, value, retention) in [
        (
            "dartforge_arc_write_field_scalar_v1",
            ModoParametro::Scalar,
            false,
        ),
        (
            "dartforge_arc_write_field_ref_v1",
            ModoParametro::Borrow,
            true,
        ),
    ] {
        let c = CONTRATOS.iter().find(|c| c.nome == nome).unwrap();
        assert_eq!(
            c.parametros,
            &[ModoParametro::Borrow, ModoParametro::Scalar, value]
        );
        assert_eq!(c.resultado, ModoResultado::Void);
        assert_eq!(c.retencao_persistente, retention);
        assert!(c.invalida_borrows && !c.pode_falhar && !c.chama_dart);
        assert!(!EFEITOS.iter().find(|e| e.0 == nome).unwrap().1);
    }
    let throw = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_throw_ref_v1")
        .unwrap();
    let ref_read = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_arc_read_field_ref_v1")
        .unwrap();
    assert_eq!(
        ref_read.parametros,
        &[ModoParametro::Borrow, ModoParametro::Scalar]
    );
    assert_eq!(ref_read.resultado, ModoResultado::BorrowArg(0));
    assert!(
        !ref_read.pode_falhar
            && !ref_read.chama_dart
            && !ref_read.retencao_persistente
            && !ref_read.invalida_borrows
    );
    assert!(!EFEITOS.iter().find(|e| e.0 == ref_read.nome).unwrap().1);
    assert_eq!(throw.parametros, &[ModoParametro::Borrow]);
    assert_eq!(throw.resultado, ModoResultado::Void);
    assert!(throw.pode_falhar && throw.retencao_persistente && throw.invalida_borrows);
    let clear = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_exception_clear")
        .unwrap();
    assert!(clear.parametros.is_empty());
    assert!(!clear.pode_falhar && !clear.retencao_persistente);
    assert!(clear.invalida_borrows);
    let field = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_native_DartForge_record_fieldAt")
        .unwrap();
    assert_eq!(field.resultado, ModoResultado::BorrowArg(0));
    // Record reads: they neither allocate nor throw (lowering does not check).
    assert!(!field.pode_falhar);
    for nome in [
        "dartforge_native_DartForge_record_numFields",
        "dartforge_native_DartForge_record_shape",
    ] {
        let c = CONTRATOS.iter().find(|c| c.nome == nome).unwrap();
        assert_eq!(c.parametros, &[ModoParametro::Borrow]);
        assert_eq!(c.resultado, ModoResultado::ScalarI64);
        assert!(!c.pode_falhar);
        assert!(!c.retencao_persistente && !c.invalida_borrows);
    }
    let print = CONTRATOS
        .iter()
        .find(|c| c.nome == "dartforge_print_handle")
        .unwrap();
    assert_eq!(print.parametros, &[ModoParametro::Borrow]);
    assert_eq!(print.resultado, ModoResultado::Void);
    assert!(print.pode_falhar);
    assert!(!print.retencao_persistente && !print.invalida_borrows);
    assert!(!CONTRATOS.iter().any(|c| c.nome == "dartforge_allocate"));
    for nome in ["dartforge_gc_global_root", "dartforge_mark_constant"] {
        let c = CONTRATOS.iter().find(|c| c.nome == nome).unwrap();
        assert!(c.retencao_persistente);
        assert!(!c.parametros.contains(&ModoParametro::Consume));
    }
}

#[test]
fn copied_borrowed_capture_survives_env_and_preserves_pending() {
    use dartforge_runtime::{abi::*, layout::cid};
    std::thread::spawn(|| {
        dartforge_memory_arc_v1();
        let child = dartforge_arc_box_int_owned_v1(i64::MAX);
        let env = dartforge_arc_object_owned_v1(i64::from(cid::CONTEXTO), 40);
        dartforge_arc_write_field_ref_v1(env, 39, child);
        dartforge_arc_release(child);
        dartforge_exception_throw(42, 1);
        let pending = dartforge_exception_peek_ref();
        let capture = dartforge_arc_env_ref_v1(env, 39);
        assert_eq!(capture, child);
        assert_eq!(dartforge_arc_observe_heap_v1(capture), 1);
        assert_ne!(dartforge_exception_pending(), 0);
        assert_eq!(dartforge_exception_peek_ref(), pending);
        dartforge_arc_retain(capture);
        dartforge_arc_release(env);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observe_heap_v1(env), 0);
        assert_eq!(dartforge_arc_observe_heap_v1(capture), 3);
        assert_eq!(dartforge_unbox_int(capture), i64::MAX);
        dartforge_arc_release(capture);
        dartforge_exception_clear();
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observe_heap_v1(capture), 0);
    }).join().unwrap();
}
