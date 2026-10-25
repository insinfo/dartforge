//! The native table of the source SDK (P5b, docs/NATIVO-PLANO.md §7).
//!
//! In the VM SDK, an `external` member without a patch is implemented in one of two
//! ways: **native** — `@pragma("vm:external-name", "Name")`, a C++ function
//! of the VM runtime (`runtime/lib/*.cc`) — or **intrinsic** —
//! `@pragma("vm:recognized", ...)` without a native name, which the VM
//! compiler generates in place of the call (`double` arithmetic, `[]` of `_List`,
//! invisible fields of `_GrowableList`/`_compact_hash`/`Record`…). The native backend
//! does the same: every native of the [`SOURCE_LIBRARIES`] has an entry
//! here, with what the backend does with it.
//!
//! * [`State::Runtime`]: a function of the Rust runtime,
//!   `dartforge_native_<Name>` (the `natives` fragment of `crates/runtime`),
//!   with the signature in the HIR representation (R1) of the declared Dart
//!   types: `int` → `i64`, `double` → `double`, `bool` → `i8`, the rest
//!   (receiver included) → `Ref`. Errors leave through the pending exception.
//! * [`State::Pending`]: not implemented yet; the P5d lowering
//!   turns the call into a diagnostic (N1), never into a call to a nonexistent
//!   symbol.
//!
//! The effects follow G8 (NATIVO-PLANO §6.5): marked conservatively
//! (allocates, throws) until a test under `DARTFORGE_GC_STRESS=1` proves the
//! opposite.
//!
//! The `inventory::every_source_native_has_entry` test loads the SDK with the
//! overlay, walks the unpatched `external`s of the source libraries and
//! rejects a native without an entry (and an entry that is no longer a native): that is what
//! prevents a new patch of the SDK — or of the overlay — from coming in silently.
//!
//! [`SOURCE_LIBRARIES`]: crate::sdk_module::SOURCE_LIBRARIES

/// What the backend does with a native.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Runtime function `dartforge_native_<Name>`.
    Runtime,
    /// Not implemented yet: calling it is a diagnostic.
    Pending,
    /// The lowering generates the code in place of the call (`Internal_unsafeCast`
    /// is the value itself).
    Builtin,
}

/// Effects of a call (G8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effects {
    pub allocates: bool,
    pub throws: bool,
    pub calls_dart: bool,
}

/// Conservative: allocates and throws, does not call Dart.
pub const CONSERVATIVE: Effects = Effects {
    allocates: true,
    throws: true,
    calls_dart: false,
};

/// A native from `@pragma("vm:external-name", name)`.
#[derive(Debug, Clone, Copy)]
pub struct NativeEntry {
    pub name: &'static str,
    pub state: State,
    pub effects: Effects,
}

const fn runtime(name: &'static str) -> NativeEntry {
    NativeEntry {
        name,
        state: State::Runtime,
        effects: CONSERVATIVE,
    }
}

const fn embedded(name: &'static str) -> NativeEntry {
    NativeEntry {
        name,
        state: State::Builtin,
        effects: CONSERVATIVE,
    }
}

/// Natives that the SDK declares with different returns depending on the use site
/// and that the runtime implements with the widest one, `Ref`: `Double_parse` is
/// `double?` in `double._nativeParse` and `double` in `_parseDouble` of
/// `convert_patch.dart` (the JSON number). The `external` calls with `Ref` and
/// converts to the declared type — without that the handle of the box was read
/// as the bits of the `double`.
pub const RETURN_REF: &[&str] = &["Double_parse"];

/// VM intrinsics (`vm:recognized` without a native) that the native backend implements
/// with a runtime function, by `Class.member`: (member, native).
pub const INTRINSICS: &[(&str, &str)] = &[
    ("_Array.[]", "DartForge_list_get"),
    ("_Double._modulo", "DartForge_double_modulo"),
    ("_Double._remainder", "DartForge_double_remainder"),
    ("_Double.ceilToDouble", "DartForge_double_ceil"),
    ("_Double.floorToDouble", "DartForge_double_floor"),
    ("_Double.hashCode", "DartForge_double_hashCode"),
    ("_Double.roundToDouble", "DartForge_double_round"),
    ("_Double.toInt", "DartForge_double_toInt"),
    ("_Double.truncateToDouble", "DartForge_double_truncate"),
    ("_Float32ArrayView.[]", "DartForge_typed_index_double"),
    ("_Float32List.[]", "DartForge_typed_index_double"),
    ("_Float64ArrayView.[]", "DartForge_typed_index_double"),
    ("_Float64List.[]", "DartForge_typed_index_double"),
    ("_Float32x4List.[]", "DartForge_typed_index_simd"),
    ("_Int32x4List.[]", "DartForge_typed_index_simd"),
    ("_Float64x2List.[]", "DartForge_typed_index_simd"),
    ("_Float32x4ArrayView.[]", "DartForge_typed_index_simd"),
    ("_Int32x4ArrayView.[]", "DartForge_typed_index_simd"),
    ("_Float64x2ArrayView.[]", "DartForge_typed_index_simd"),
    ("_GrowableList.[]", "DartForge_list_get"),
    ("_ImmutableList.[]", "DartForge_list_get"),
    ("_Int16ArrayView.[]", "DartForge_typed_index_int"),
    ("_Int16List.[]", "DartForge_typed_index_int"),
    ("_Int32ArrayView.[]", "DartForge_typed_index_int"),
    ("_Int32List.[]", "DartForge_typed_index_int"),
    ("_Int64ArrayView.[]", "DartForge_typed_index_int"),
    ("_Int64List.[]", "DartForge_typed_index_int"),
    ("_Int8ArrayView.[]", "DartForge_typed_index_int"),
    ("_Int8List.[]", "DartForge_typed_index_int"),
    ("_List.[]", "DartForge_list_get"),
    ("_Mint.hashCode", "DartForge_int_hashCode"),
    ("_Record._fieldAt", "DartForge_record_fieldAt"),
    ("_Record._fieldNames", "DartForge_record_fieldNames"),
    ("_Record._numFields", "DartForge_record_numFields"),
    ("_Record._shape", "DartForge_record_shape"),
    ("_Smi.hashCode", "DartForge_int_hashCode"),
    ("_StringBase.codeUnitAt", "DartForge_string_codeUnitAt"),
    ("_TypedList._getFloat32", "TypedData_GetFloat32"),
    ("_TypedList._getFloat64", "TypedData_GetFloat64"),
    ("_TypedList._getFloat32x4", "TypedData_GetFloat32x4"),
    ("_TypedList._getInt32x4", "TypedData_GetInt32x4"),
    ("_TypedList._getFloat64x2", "TypedData_GetFloat64x2"),
    ("_TypedList._getInt16", "DartForge_typed_getInt16"),
    ("_TypedList._getInt32", "DartForge_typed_getInt32"),
    ("_TypedList._getInt64", "DartForge_typed_getInt64"),
    ("_TypedList._getInt8", "DartForge_typed_getInt8"),
    ("_TypedList._getUint16", "DartForge_typed_getUint16"),
    ("_TypedList._getUint32", "DartForge_typed_getUint32"),
    ("_TypedList._getUint64", "DartForge_typed_getUint64"),
    ("_TypedList._getUint8", "DartForge_typed_getUint8"),
    ("_TypedList._setFloat32", "TypedData_SetFloat32"),
    ("_TypedList._setFloat64", "TypedData_SetFloat64"),
    ("_TypedList._setFloat32x4", "TypedData_SetFloat32x4"),
    ("_TypedList._setInt32x4", "TypedData_SetInt32x4"),
    ("_TypedList._setFloat64x2", "TypedData_SetFloat64x2"),
    ("_TypedList._setInt16", "DartForge_typed_setInt16"),
    ("_TypedList._setInt32", "DartForge_typed_setInt32"),
    ("_TypedList._setInt64", "DartForge_typed_setInt64"),
    ("_TypedList._setInt8", "DartForge_typed_setInt8"),
    ("_TypedList._setUint16", "DartForge_typed_setUint16"),
    ("_TypedList._setUint32", "DartForge_typed_setUint32"),
    ("_TypedList._setUint64", "DartForge_typed_setUint64"),
    ("_TypedList._setUint8", "DartForge_typed_setUint8"),
    ("_TypedListBase._memMove1", "DartForge_typed_memMove1"),
    ("_TypedListBase._memMove16", "DartForge_typed_memMove16"),
    ("_TypedListBase._memMove2", "DartForge_typed_memMove2"),
    ("_TypedListBase._memMove4", "DartForge_typed_memMove4"),
    ("_TypedListBase._memMove8", "DartForge_typed_memMove8"),
    ("_Uint16ArrayView.[]", "DartForge_typed_index_int"),
    ("_Uint16List.[]", "DartForge_typed_index_int"),
    ("_Uint32ArrayView.[]", "DartForge_typed_index_int"),
    ("_Uint32List.[]", "DartForge_typed_index_int"),
    ("_Uint64ArrayView.[]", "DartForge_typed_index_int"),
    ("_Uint64List.[]", "DartForge_typed_index_int"),
    ("_Uint8ArrayView.[]", "DartForge_typed_index_int"),
    ("_Uint8ClampedArrayView.[]", "DartForge_typed_index_int"),
    ("_Uint8ClampedList.[]", "DartForge_typed_index_int"),
    ("_Uint8List.[]", "DartForge_typed_index_int"),
    ("_acos", "DartForge_math_acos"),
    ("_asin", "DartForge_math_asin"),
    ("_atan", "DartForge_math_atan"),
    ("_atan2", "DartForge_math_atan2"),
    ("_cos", "DartForge_math_cos"),
    ("_doublePow", "DartForge_math_pow"),
    ("_exp", "DartForge_math_exp"),
    ("_log", "DartForge_math_log"),
    ("_sin", "DartForge_math_sin"),
    ("_sqrt", "DartForge_math_sqrt"),
    ("_tan", "DartForge_math_tan"),
    ("has63BitSmis", "DartForge_true"),
];

/// The runtime function of a VM intrinsic, if the native backend has it.
pub fn intrinsic(member: &str) -> Option<&'static str> {
    INTRINSICS
        .iter()
        .find(|(m, _)| *m == member)
        .map(|(_, n)| *n)
}

const fn pending(name: &'static str) -> NativeEntry {
    NativeEntry {
        name,
        state: State::Pending,
        effects: CONSERVATIVE,
    }
}

/// Every native of the source libraries with the `native_sdk/` overlay,
/// in byte order (that of `str`, which the binary search uses). The `DartForge_*` ones are from the overlay.
pub const NATIVES: &[NativeEntry] = &[
    pending("AbstractType_equality"),
    pending("AbstractType_getHashCode"),
    pending("AbstractType_toString"),
    pending("AssertionError_throwNew"),
    pending("AssertionError_throwNewSource"),
    pending("Bool_fromEnvironment"),
    pending("Bool_hasEnvironment"),
    // The class of the value inline (`@df.class`, docs/NATIVO-ESPACO-UNIFICADO.md §3.8).
    embedded("ClassID_getID"),
    runtime("Closure_computeHash"),
    runtime("Closure_equals"),
    runtime("Crypto_GetRandomBytes"),
    runtime("DartApiDLInitializeData"),
    runtime("DartApiDLMajorVersion"),
    runtime("DartApiDLMinorVersion"),
    runtime("DartForge_GrowableList_reserve"),
    runtime("DartForge_List_copy"),
    runtime("DartForge_List_fill"),
    runtime("DartForge_Timer_cancel"),
    runtime("DartForge_Timer_new"),
    runtime("DartForge_Timer_tick"),
    runtime("DartForge_capability_new"),
    runtime("DartForge_class_not_sendable"),
    runtime("DartForge_class_transferable"),
    runtime("DartForge_current_message"),
    runtime("DartForge_double_bits"),
    runtime("DartForge_ffi_abi"),
    runtime("DartForge_ffi_callback_args"),
    runtime("DartForge_ffi_callback_delete"),
    runtime("DartForge_ffi_callback_keep"),
    runtime("DartForge_ffi_callback_new"),
    runtime("DartForge_ffi_callback_sync_claim"),
    runtime("DartForge_ffi_callback_sync_complete"),
    runtime("DartForge_ffi_callback_sync_new"),
    runtime("DartForge_ffi_compound_of_type"),
    runtime("DartForge_ffi_copy"),
    runtime("DartForge_ffi_function"),
    runtime("DartForge_ffi_load_abi"),
    runtime("DartForge_ffi_load_f32"),
    runtime("DartForge_ffi_load_f64"),
    runtime("DartForge_ffi_load_i16"),
    runtime("DartForge_ffi_load_i32"),
    runtime("DartForge_ffi_load_i64"),
    runtime("DartForge_ffi_load_i8"),
    runtime("DartForge_ffi_load_u16"),
    runtime("DartForge_ffi_load_u32"),
    runtime("DartForge_ffi_load_u64"),
    runtime("DartForge_ffi_load_u8"),
    runtime("DartForge_ffi_size_of"),
    runtime("DartForge_ffi_store_abi"),
    runtime("DartForge_ffi_store_f32"),
    runtime("DartForge_ffi_store_f64"),
    runtime("DartForge_ffi_store_i16"),
    runtime("DartForge_ffi_store_i32"),
    runtime("DartForge_ffi_store_i64"),
    runtime("DartForge_ffi_store_i8"),
    runtime("DartForge_ffi_store_u16"),
    runtime("DartForge_ffi_store_u32"),
    runtime("DartForge_ffi_store_u64"),
    runtime("DartForge_ffi_store_u8"),
    runtime("DartForge_finalizer_attach"),
    runtime("DartForge_finalizer_attach_native"),
    runtime("DartForge_finalizer_detach"),
    runtime("DartForge_hash_map_fill"),
    runtime("DartForge_hash_map_lookup"),
    runtime("DartForge_hash_map_store"),
    runtime("DartForge_hash_new_data"),
    runtime("DartForge_hash_set_add"),
    runtime("DartForge_hash_set_fill"),
    runtime("DartForge_hash_set_lookup"),
    runtime("DartForge_int_toString"),
    // The JavaScript interop of WebAssembly modules (`dart:_js_helper`).
    runtime("DartForge_js_attach"),
    runtime("DartForge_js_dart_object"),
    runtime("DartForge_js_dart_ref"),
    runtime("DartForge_js_typed_data_address"),
    runtime("DartForge_json_close_list"),
    runtime("DartForge_json_mark"),
    runtime("DartForge_json_push"),
    runtime("DartForge_json_truncate"),
    runtime("DartForge_mirrors_name"),
    runtime("DartForge_mirrors_uri"),
    runtime("DartForge_native_pump_clear_dispatcher"),
    runtime("DartForge_native_pump_disable"),
    runtime("DartForge_native_pump_dispatcher_supported"),
    runtime("DartForge_native_pump_enable"),
    runtime("DartForge_native_pump_set_dispatcher"),
    runtime("DartForge_native_pump_supported"),
    runtime("DartForge_package_config"),
    runtime("DartForge_port_close"),
    runtime("DartForge_port_current"),
    runtime("DartForge_port_keeps_alive"),
    runtime("DartForge_port_open"),
    runtime("DartForge_port_refusal"),
    runtime("DartForge_port_send"),
    runtime("DartForge_port_set_keep_alive"),
    runtime("DartForge_ports_dispatcher"),
    runtime("DartForge_print"),
    runtime("DartForge_read_text"),
    runtime("DartForge_regexp_capture"),
    runtime("DartForge_regexp_compile"),
    runtime("DartForge_regexp_error"),
    runtime("DartForge_regexp_exec"),
    runtime("DartForge_regexp_groups"),
    runtime("DartForge_regexp_n_names"),
    runtime("DartForge_regexp_name"),
    runtime("DartForge_regexp_name_index"),
    runtime("DartForge_sb_new"),
    runtime("DartForge_sb_to_string"),
    runtime("DartForge_sb_write"),
    runtime("DartForge_sb_write_char_code"),
    runtime("DartForge_sb_write_if_string"),
    runtime("DartForge_scheduleImmediate"),
    runtime("DartForge_string_equals"),
    runtime("DartForge_string_equals_object"),
    runtime("DartForge_string_from_char_codes"),
    runtime("DartForge_string_index_of"),
    runtime("DartForge_string_last_index_of"),
    runtime("DartForge_string_region_equals"),
    runtime("DartForge_string_split_by_code"),
    runtime("DartForge_tls_context_alpn"),
    runtime("DartForge_tls_context_authorities"),
    runtime("DartForge_tls_context_builtin_roots"),
    runtime("DartForge_tls_context_chain"),
    runtime("DartForge_tls_context_key"),
    runtime("DartForge_tls_context_min_version"),
    runtime("DartForge_tls_context_new"),
    runtime("DartForge_tls_context_set_min_version"),
    runtime("DartForge_tls_context_trusted"),
    runtime("DartForge_tls_filter_connect"),
    runtime("DartForge_tls_filter_decide"),
    runtime("DartForge_tls_filter_destroy"),
    runtime("DartForge_tls_filter_error"),
    runtime("DartForge_tls_filter_handshake"),
    runtime("DartForge_tls_filter_new"),
    runtime("DartForge_tls_filter_peer_certificate"),
    runtime("DartForge_tls_filter_pending_certificate"),
    runtime("DartForge_tls_filter_pending_keys"),
    runtime("DartForge_tls_filter_process"),
    runtime("DartForge_tls_filter_protocol"),
    runtime("DartForge_tls_filter_register_keys"),
    runtime("DartForge_tls_x509_adopt"),
    runtime("DartForge_tls_x509_der"),
    runtime("DartForge_tls_x509_end"),
    runtime("DartForge_tls_x509_issuer"),
    runtime("DartForge_tls_x509_pem"),
    runtime("DartForge_tls_x509_sha1"),
    runtime("DartForge_tls_x509_start"),
    runtime("DartForge_tls_x509_subject"),
    runtime("DartForge_uri_base"),
    runtime("DartNativeApiFunctionPointer"),
    runtime("DateTime_currentTimeMicros"),
    runtime("DateTime_timeZoneName"),
    runtime("DateTime_timeZoneOffsetInSeconds"),
    runtime("Developer_NativeRuntime_buildId"),
    pending("Developer_NativeRuntime_writeHeapSnapshotToFile"),
    runtime("Developer_debugger"),
    runtime("Developer_getIsolateIdFromSendPort"),
    runtime("Developer_getObjectId"),
    runtime("Developer_getServerInfo"),
    runtime("Developer_getServiceMajorVersion"),
    runtime("Developer_getServiceMinorVersion"),
    runtime("Developer_inspect"),
    runtime("Developer_log"),
    runtime("Developer_lookupExtension"),
    runtime("Developer_postEvent"),
    runtime("Developer_reachability_barrier"),
    runtime("Developer_registerExtension"),
    runtime("Developer_webServerControl"),
    runtime("Directory_Create"),
    runtime("Directory_CreateTemp"),
    runtime("Directory_Current"),
    runtime("Directory_Delete"),
    runtime("Directory_Exists"),
    runtime("Directory_FillWithDirectoryListing"),
    runtime("Directory_GetAsyncDirectoryListerPointer"),
    runtime("Directory_Rename"),
    runtime("Directory_SetAsyncDirectoryListerPointer"),
    runtime("Directory_SetCurrent"),
    runtime("Directory_SystemTemp"),
    // The pure arithmetic and comparisons of `_Double`/`_IntegerImplementation`
    // are HIR operations in place of the call (`sdk_source::embedded_native`):
    // the runtime functions stay for whoever links them, but generated code no
    // longer pays a call, an exception check and the roots per operator.
    embedded("Double_add"),
    embedded("Double_div"),
    embedded("Double_doubleFromInteger"),
    embedded("Double_equal"),
    embedded("Double_equalToInteger"),
    embedded("Double_flipSignBit"),
    embedded("Double_getIsInfinite"),
    embedded("Double_getIsNaN"),
    embedded("Double_getIsNegative"),
    embedded("Double_greaterThan"),
    embedded("Double_greaterThanFromInteger"),
    embedded("Double_mul"),
    runtime("Double_parse"),
    embedded("Double_sub"),
    runtime("Double_toString"),
    runtime("Double_toStringAsExponential"),
    runtime("Double_toStringAsFixed"),
    runtime("Double_toStringAsPrecision"),
    runtime("Error_throwWithStackTrace"),
    runtime("Error_trySetStackTrace"),
    runtime("EventHandler_SendData"),
    runtime("EventHandler_TimerMillisecondClock"),
    pending("Ffi_GetFfiNativeResolver"),
    pending("Ffi_createNativeCallableIsolateLocal"),
    pending("Ffi_createNativeCallableListener"),
    runtime("Ffi_dl_close"),
    runtime("Ffi_dl_executableLibrary"),
    runtime("Ffi_dl_lookup"),
    runtime("Ffi_dl_open"),
    runtime("Ffi_dl_processLibrary"),
    runtime("Ffi_dl_providesSymbol"),
    pending("Ffi_nativeAsyncCallbackFunction"),
    pending("Ffi_nativeCallbackFunction"),
    pending("Ffi_nativeIsolateLocalCallbackFunction"),
    runtime("FileSystemWatcher_CloseWatcher"),
    runtime("FileSystemWatcher_GetSocketId"),
    runtime("FileSystemWatcher_InitWatcher"),
    runtime("FileSystemWatcher_IsSupported"),
    runtime("FileSystemWatcher_ReadEvents"),
    runtime("FileSystemWatcher_UnwatchPath"),
    runtime("FileSystemWatcher_WatchPath"),
    runtime("File_AreIdentical"),
    runtime("File_Close"),
    runtime("File_Copy"),
    runtime("File_Create"),
    runtime("File_CreateLink"),
    runtime("File_CreatePipe"),
    runtime("File_Delete"),
    runtime("File_DeleteLink"),
    runtime("File_Exists"),
    runtime("File_Flush"),
    runtime("File_GetFD"),
    runtime("File_GetPointer"),
    runtime("File_GetStdioHandleType"),
    runtime("File_GetType"),
    runtime("File_LastAccessed"),
    runtime("File_LastModified"),
    runtime("File_Length"),
    runtime("File_LengthFromPath"),
    runtime("File_LinkTarget"),
    runtime("File_Lock"),
    runtime("File_Open"),
    runtime("File_OpenStdio"),
    runtime("File_Position"),
    runtime("File_Read"),
    runtime("File_ReadByte"),
    runtime("File_ReadInto"),
    runtime("File_Rename"),
    runtime("File_RenameLink"),
    runtime("File_ResolveSymbolicLinks"),
    runtime("File_SetLastAccessed"),
    runtime("File_SetLastModified"),
    runtime("File_SetPointer"),
    runtime("File_SetPosition"),
    runtime("File_Stat"),
    runtime("File_Truncate"),
    runtime("File_WriteByte"),
    runtime("File_WriteFrom"),
    runtime("Filter_CreateZLibDeflate"),
    runtime("Filter_CreateZLibInflate"),
    runtime("Filter_Process"),
    runtime("Filter_Processed"),
    pending("FinalizerEntry_allocate"),
    runtime("Float32x4_abs"),
    runtime("Float32x4_add"),
    runtime("Float32x4_clamp"),
    runtime("Float32x4_cmpequal"),
    runtime("Float32x4_cmpgt"),
    runtime("Float32x4_cmpgte"),
    runtime("Float32x4_cmplt"),
    runtime("Float32x4_cmplte"),
    runtime("Float32x4_cmpnequal"),
    runtime("Float32x4_div"),
    runtime("Float32x4_fromDoubles"),
    runtime("Float32x4_fromFloat64x2"),
    runtime("Float32x4_fromInt32x4Bits"),
    runtime("Float32x4_getSignMask"),
    runtime("Float32x4_getW"),
    runtime("Float32x4_getX"),
    runtime("Float32x4_getY"),
    runtime("Float32x4_getZ"),
    runtime("Float32x4_max"),
    runtime("Float32x4_min"),
    runtime("Float32x4_mul"),
    runtime("Float32x4_negate"),
    runtime("Float32x4_reciprocal"),
    runtime("Float32x4_reciprocalSqrt"),
    runtime("Float32x4_scale"),
    runtime("Float32x4_setW"),
    runtime("Float32x4_setX"),
    runtime("Float32x4_setY"),
    runtime("Float32x4_setZ"),
    runtime("Float32x4_shuffle"),
    runtime("Float32x4_shuffleMix"),
    runtime("Float32x4_splat"),
    runtime("Float32x4_sqrt"),
    runtime("Float32x4_sub"),
    runtime("Float32x4_zero"),
    runtime("Float64x2_abs"),
    runtime("Float64x2_add"),
    runtime("Float64x2_clamp"),
    runtime("Float64x2_div"),
    runtime("Float64x2_fromDoubles"),
    runtime("Float64x2_fromFloat32x4"),
    runtime("Float64x2_getSignMask"),
    runtime("Float64x2_getX"),
    runtime("Float64x2_getY"),
    runtime("Float64x2_max"),
    runtime("Float64x2_min"),
    runtime("Float64x2_mul"),
    runtime("Float64x2_negate"),
    runtime("Float64x2_scale"),
    runtime("Float64x2_setX"),
    runtime("Float64x2_setY"),
    runtime("Float64x2_splat"),
    runtime("Float64x2_sqrt"),
    runtime("Float64x2_sub"),
    runtime("Float64x2_zero"),
    runtime("Function_apply"),
    runtime("GrowableList_allocate"),
    runtime("GrowableList_getCapacity"),
    runtime("GrowableList_getLength"),
    runtime("GrowableList_setData"),
    runtime("GrowableList_setIndexed"),
    runtime("GrowableList_setLength"),
    runtime("IOService_NewServicePort"),
    runtime("Identical_comparison"),
    runtime("ImmutableList_from"),
    runtime("Int32x4_add"),
    runtime("Int32x4_and"),
    runtime("Int32x4_fromBools"),
    runtime("Int32x4_fromFloat32x4Bits"),
    runtime("Int32x4_fromInts"),
    runtime("Int32x4_getFlagW"),
    runtime("Int32x4_getFlagX"),
    runtime("Int32x4_getFlagY"),
    runtime("Int32x4_getFlagZ"),
    runtime("Int32x4_getSignMask"),
    runtime("Int32x4_getW"),
    runtime("Int32x4_getX"),
    runtime("Int32x4_getY"),
    runtime("Int32x4_getZ"),
    runtime("Int32x4_or"),
    runtime("Int32x4_select"),
    runtime("Int32x4_setFlagW"),
    runtime("Int32x4_setFlagX"),
    runtime("Int32x4_setFlagY"),
    runtime("Int32x4_setFlagZ"),
    runtime("Int32x4_setW"),
    runtime("Int32x4_setX"),
    runtime("Int32x4_setY"),
    runtime("Int32x4_setZ"),
    runtime("Int32x4_shuffle"),
    runtime("Int32x4_shuffleMix"),
    runtime("Int32x4_sub"),
    runtime("Int32x4_xor"),
    embedded("Integer_addFromInteger"),
    embedded("Integer_bitAndFromInteger"),
    embedded("Integer_bitOrFromInteger"),
    embedded("Integer_bitXorFromInteger"),
    embedded("Integer_equalToInteger"),
    pending("Integer_fromEnvironment"),
    embedded("Integer_greaterThanFromInteger"),
    runtime("Integer_moduloFromInteger"),
    embedded("Integer_mulFromInteger"),
    runtime("Integer_shlFromInteger"),
    runtime("Integer_shrFromInteger"),
    embedded("Integer_subFromInteger"),
    runtime("Integer_truncDivFromInteger"),
    runtime("Integer_ushrFromInteger"),
    pending("Internal_allocateObjectInstructionsEnd"),
    pending("Internal_allocateObjectInstructionsStart"),
    runtime("Internal_allocateOneByteString"),
    runtime("Internal_allocateTwoByteString"),
    pending("Internal_boundsCheckForPartialInstantiation"),
    pending("Internal_collectAllGarbage"),
    pending("Internal_deoptimizeFunctionsOnStack"),
    pending("Internal_extractTypeArguments"),
    pending("Internal_loadDynamicModule"),
    runtime("Internal_makeFixedListUnmodifiable"),
    runtime("Internal_makeListFixedLength"),
    pending("Internal_nativeEffect"),
    pending("Internal_prependTypeArguments"),
    embedded("Internal_unsafeCast"),
    runtime("Internal_writeIntoOneByteString"),
    runtime("Internal_writeIntoTwoByteString"),
    runtime("InternetAddress_Parse"),
    runtime("InternetAddress_ParseScopedLinkLocalAddress"),
    runtime("InternetAddress_RawAddrToString"),
    runtime("InvocationMirror_unpackTypeArguments"),
    runtime("Isolate_exit_"),
    runtime("Isolate_getCurrentRootUriStr"),
    runtime("Isolate_getDebugName"),
    runtime("Isolate_getPortAndCapabilitiesOfCurrentIsolate"),
    pending("Isolate_registerKernelBlob"),
    runtime("Isolate_sendOOB"),
    runtime("Isolate_spawnFunction"),
    runtime("Isolate_spawnUri"),
    pending("Isolate_unregisterKernelBlob"),
    pending("LibraryPrefix_isLoaded"),
    pending("LibraryPrefix_issueLoad"),
    pending("LibraryPrefix_loadingUnit"),
    pending("LibraryPrefix_setLoaded"),
    runtime("List_allocate"),
    runtime("List_getLength"),
    runtime("List_setIndexed"),
    runtime("List_slice"),
    runtime("Mint_bitLength"),
    runtime("Mint_bitNegate"),
    runtime("Namespace_Create"),
    runtime("Namespace_GetDefault"),
    runtime("Namespace_GetPointer"),
    runtime("NoSuchMethodError_existingMethodSignature"),
    runtime("OSError_inProgressErrorCode"),
    runtime("Object_equals"),
    runtime("Object_getHash"),
    runtime("Object_haveSameRuntimeType"),
    pending("Object_instanceOf"),
    runtime("Object_runtimeType"),
    pending("Object_simpleInstanceOf"),
    runtime("Object_toString"),
    runtime("OneByteString_allocateFromOneByteList"),
    runtime("OneByteString_substringUnchecked"),
    runtime("Platform_Environment"),
    runtime("Platform_ExecutableArguments"),
    runtime("Platform_ExecutableName"),
    runtime("Platform_GetVersion"),
    runtime("Platform_LocalHostname"),
    runtime("Platform_LocaleName"),
    runtime("Platform_NumberOfProcessors"),
    runtime("Platform_OperatingSystem"),
    runtime("Platform_OperatingSystemVersion"),
    runtime("Platform_PathSeparator"),
    runtime("Platform_ResolvedExecutableName"),
    runtime("ProcessInfo_CurrentRSS"),
    runtime("ProcessInfo_MaxRSS"),
    runtime("Process_ClearSignalHandler"),
    runtime("Process_Exit"),
    runtime("Process_GetExitCode"),
    runtime("Process_KillPid"),
    runtime("Process_Pid"),
    runtime("Process_SetExitCode"),
    runtime("Process_SetSignalHandler"),
    runtime("Process_Sleep"),
    runtime("Process_Start"),
    runtime("Process_Wait"),
    runtime("Random_initialSeed"),
    runtime("RawSocketOption_GetOptionValue"),
    runtime("ResourceHandleImpl_toFile"),
    runtime("ResourceHandleImpl_toRawDatagramSocket"),
    runtime("ResourceHandleImpl_toRawSocket"),
    runtime("ResourceHandleImpl_toSocket"),
    runtime("SecureRandom_getBytes"),
    runtime("SendPort_get_id"),
    runtime("ServerSocket_Accept"),
    runtime("ServerSocket_CreateBindListen"),
    runtime("ServerSocket_CreateUnixDomainBindListen"),
    runtime("Smi_bitLength"),
    runtime("Smi_bitNegate"),
    runtime("SocketBase_IsBindError"),
    runtime("SocketControlMessageImpl_extractHandles"),
    runtime("SocketControlMessage_fromHandles"),
    runtime("Socket_Available"),
    runtime("Socket_AvailableDatagram"),
    runtime("Socket_CreateBindConnect"),
    runtime("Socket_CreateBindDatagram"),
    runtime("Socket_CreateConnect"),
    runtime("Socket_CreateUnixDomainBindConnect"),
    runtime("Socket_CreateUnixDomainConnect"),
    runtime("Socket_Fatal"),
    runtime("Socket_GetError"),
    runtime("Socket_GetFD"),
    runtime("Socket_GetOption"),
    runtime("Socket_GetPort"),
    runtime("Socket_GetRawOption"),
    runtime("Socket_GetRemotePeer"),
    runtime("Socket_GetSocketId"),
    runtime("Socket_GetStdioHandle"),
    runtime("Socket_GetType"),
    runtime("Socket_HasPendingWrite"),
    runtime("Socket_JoinMulticast"),
    runtime("Socket_LeaveMulticast"),
    runtime("Socket_Read"),
    runtime("Socket_ReceiveMessage"),
    runtime("Socket_RecvFrom"),
    runtime("Socket_SendMessage"),
    runtime("Socket_SendTo"),
    runtime("Socket_SetOption"),
    runtime("Socket_SetRawOption"),
    runtime("Socket_SetSocketId"),
    runtime("Socket_WriteList"),
    runtime("StackTrace_current"),
    runtime("Stdin_AnsiSupported"),
    runtime("Stdin_GetEchoMode"),
    runtime("Stdin_GetEchoNewlineMode"),
    runtime("Stdin_GetLineMode"),
    runtime("Stdin_ReadByte"),
    runtime("Stdin_SetEchoMode"),
    runtime("Stdin_SetEchoNewlineMode"),
    runtime("Stdin_SetLineMode"),
    runtime("Stdout_AnsiSupported"),
    runtime("Stdout_GetTerminalSize"),
    runtime("Stopwatch_frequency"),
    runtime("Stopwatch_now"),
    runtime("StringBase_createFromCodePoints"),
    pending("StringBase_intern"),
    runtime("StringBase_joinReplaceAllResult"),
    runtime("StringBase_substringUnchecked"),
    runtime("StringToSystemEncoding"),
    runtime("String_charAt"),
    runtime("String_concat"),
    runtime("String_concatRange"),
    pending("String_fromEnvironment"),
    runtime("String_getHashCode"),
    runtime("String_getLength"),
    runtime("String_toLowerCase"),
    runtime("String_toUpperCase"),
    runtime("SynchronousSocket_Available"),
    runtime("SynchronousSocket_CloseSync"),
    runtime("SynchronousSocket_CreateConnectSync"),
    runtime("SynchronousSocket_GetPort"),
    runtime("SynchronousSocket_GetRemotePeer"),
    runtime("SynchronousSocket_LookupRequest"),
    runtime("SynchronousSocket_Read"),
    runtime("SynchronousSocket_ReadList"),
    runtime("SynchronousSocket_ShutdownRead"),
    runtime("SynchronousSocket_ShutdownWrite"),
    runtime("SynchronousSocket_WriteList"),
    runtime("SystemEncodingToString"),
    runtime("Timeline_getNextTaskId"),
    runtime("Timeline_getTraceClock"),
    runtime("Timeline_isDartStreamEnabled"),
    runtime("Timeline_reportTaskEvent"),
    runtime("TwoByteString_allocateFromTwoByteList"),
    pending("TypeError_throwNew"),
    pending("Type_equality"),
    runtime("TypedDataBase_length"),
    runtime("TypedDataBase_setClampedRange"),
    runtime("TypedDataView_offsetInBytes"),
    runtime("TypedDataView_typedData"),
    runtime("TypedData_GetFloat32"),
    runtime("TypedData_GetFloat32x4"),
    runtime("TypedData_GetFloat64"),
    runtime("TypedData_GetFloat64x2"),
    runtime("TypedData_GetInt32x4"),
    runtime("TypedData_SetFloat32"),
    runtime("TypedData_SetFloat32x4"),
    runtime("TypedData_SetFloat64"),
    runtime("TypedData_SetFloat64x2"),
    runtime("TypedData_SetInt32x4"),
    runtime("Uri_isWindowsPlatform"),
    runtime("WeakProperty_getKey"),
    runtime("WeakProperty_getValue"),
    runtime("WeakProperty_setKey"),
    runtime("WeakProperty_setValue"),
    runtime("WeakReference_getTarget"),
    runtime("WeakReference_setTarget"),
];

/// The entry of a native, if it exists.
pub fn native(name: &str) -> Option<&'static NativeEntry> {
    NATIVES
        .binary_search_by(|n| n.name.cmp(name))
        .ok()
        .map(|i| &NATIVES[i])
}

/// The packages of the unified space specification
/// (docs/NATIVO-ESPACO-UNIFICADO.md §4): which one changes the implementation of a native.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Package {
    /// Foundation: layout, space, collector, emitter contract.
    P0,
    /// Strings.
    P1,
    /// Boxes, cells, contexts, closures, records.
    P2,
    /// Listas, mapas, conjuntos.
    P3,
    /// Typed lists, SIMD, FFI, I/O.
    P4,
    /// Cross-cutting (isolates, RTI, errors, output).
    P5,
}

/// What the unified space changes in a native (table §3.8 of the specification):
/// the package that replaces the implementation and whether it becomes **inline** (through the
/// `@df.*` helpers and the `*_em_linha` hooks of the lowering; the runtime
/// native, when it stays, is the slow path). `None`: the native does not change its
/// representation (only its heap API, when it has one).
pub fn in_unified_space(name: &str) -> Option<(Package, bool)> {
    use Package::*;
    let r = match name {
        // P1: strings.
        "String_getLength"
        | "Internal_allocateOneByteString"
        | "Internal_allocateTwoByteString"
        | "Internal_writeIntoOneByteString"
        | "Internal_writeIntoTwoByteString"
        | "String_getHashCode"
        | "DartForge_string_codeUnitAt" => (P1, true),
        "String_charAt"
        | "String_concat"
        | "StringBase_substringUnchecked"
        | "OneByteString_substringUnchecked"
        | "OneByteString_allocateFromOneByteList"
        | "TwoByteString_allocateFromTwoByteList"
        | "StringBase_createFromCodePoints"
        | "StringBase_joinReplaceAllResult"
        | "String_concatRange"
        | "String_toUpperCase"
        | "String_toLowerCase"
        | "DartForge_int_toString"
        | "Double_toString"
        | "DartForge_double_bits"
        | "DartForge_print" => (P1, false),
        n if n.starts_with("DartForge_string_")
            || n.starts_with("DartForge_sb_")
            || n.starts_with("DartForge_regexp_") =>
        {
            (P1, false)
        }
        // P0: the class of the value.
        "ClassID_getID" => (P0, true),
        // P3: core lists (the inline code for `List<E>` in `lower/typed_items.rs` belongs to P4).
        "List_getLength"
        | "GrowableList_getLength"
        | "GrowableList_getCapacity"
        | "GrowableList_setLength"
        | "GrowableList_setData"
        | "GrowableList_setIndexed"
        | "List_setIndexed"
        | "DartForge_list_get" => (P3, true),
        "List_allocate"
        | "List_slice"
        | "ImmutableList_from"
        | "GrowableList_allocate"
        | "DartForge_List_fill"
        | "DartForge_List_copy"
        | "DartForge_GrowableList_reserve"
        | "Internal_makeListFixedLength"
        | "Internal_makeFixedListUnmodifiable"
        | "DartForge_json_push"
        | "DartForge_json_close_list"
        | "DartForge_json_mark"
        | "DartForge_json_truncate" => (P3, false),
        n if n.starts_with("DartForge_hash_") => (P3, false),
        // P2: caixas, closures, records.
        n if n.starts_with("DartForge_record_") => (P2, true),
        "Closure_equals"
        | "Closure_computeHash"
        | "Function_apply"
        | "Identical_comparison"
        | "Double_parse"
        | "DartForge_int_hashCode"
        | "Object_getHash" => (P2, false),
        n if n.starts_with("Double_")
            || n.starts_with("Integer_")
            || n.starts_with("Mint_")
            || n.starts_with("Smi_")
            || n.starts_with("DartForge_double_") =>
        {
            (P2, false)
        }
        // P4: typed lists and SIMD.
        "TypedDataBase_length" | "TypedDataView_typedData" | "TypedDataView_offsetInBytes" => {
            (P4, true)
        }
        n if n.starts_with("TypedData_Get")
            || n.starts_with("TypedData_Set")
            || n.starts_with("DartForge_typed_") =>
        {
            (P4, true)
        }
        "TypedDataBase_setClampedRange" => (P4, false),
        n if n.starts_with("Float32x4_")
            || n.starts_with("Int32x4_")
            || n.starts_with("Float64x2_") =>
        {
            (P4, false)
        }
        // P5: transversais.
        n if n.starts_with("WeakReference_")
            || n.starts_with("WeakProperty_")
            || n.starts_with("Object_")
            || n.starts_with("Isolate_")
            || n.starts_with("DartForge_port_") =>
        {
            (P5, false)
        }
        _ => return None,
    };
    Some(r)
}

/// The runtime symbol that implements a native.
pub fn symbol(name: &str) -> String {
    format!("dartforge_native_{name}")
}

/// An unpatched `external` of the source libraries: what implements it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkExternal {
    /// `dart:core`…
    pub library: String,
    /// `Class.member` or `member` (top level).
    pub member: String,
    /// The name of the native, if it is one.
    pub native: Option<String>,
    /// Has `@pragma("vm:recognized", ...)` (a VM intrinsic).
    pub recognized: bool,
}

/// The kind of `@pragma("vm:recognized", kind)` of a function
/// (`asm-intrinsic`, `graph-intrinsic`, `other`), if it has the pragma.
pub fn recognized_kind(
    program: &dartforge_elements::Program,
    interner: &dartforge_intern::Interner,
    f: &dartforge_elements::model::FunctionElement,
) -> Option<String> {
    use dartforge_elements::model::FunctionRef;
    use dartforge_frontend::ast;
    let (unit, metadata): (_, &[ast::Annotation]) = match f.node {
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            let m = a
                .members
                .iter()
                .find(|m| matches!(m.kind, ast::MemberKind::Method(id) if id == function))
                .map(|m| &*m.metadata)
                .or_else(|| {
                    a.decls
                        .iter()
                        .find(|d| matches!(d.kind, ast::DeclKind::Function(id) if id == function))
                        .map(|d| &*d.metadata)
                })
                .unwrap_or(&[]);
            (unit, m)
        }
        FunctionRef::Constructor { unit, member } => (
            unit,
            &program.unit(unit).ast.members[member.0 as usize].metadata,
        ),
        FunctionRef::None => return None,
    };
    let text = |e: ast::ExprId| match &program.unit(unit).ast.exprs[e.0 as usize].kind {
        ast::ExprKind::String(lit) => lit
            .constant_value()
            .map(|t| String::from_utf8_lossy(t.as_bytes()).into_owned()),
        _ => None,
    };
    metadata.iter().find_map(|an| {
        if an.name.len() != 1 || interner.resolve(an.name[0].sym) != "pragma" {
            return None;
        }
        let args = an.arguments.as_ref()?;
        if text(args.args.first()?.value).as_deref() != Some("vm:recognized") {
            return None;
        }
        text(args.args.get(1)?.value)
    })
}

/// What the `@pragma`s of a function say: the name of the native
/// (`vm:external-name`) and whether it is `vm:recognized` (intrinsic).
pub fn pragmas(
    program: &dartforge_elements::Program,
    interner: &dartforge_intern::Interner,
    f: &dartforge_elements::model::FunctionElement,
) -> (Option<String>, bool) {
    let mut native = None;
    let mut recognized = false;
    for strings in pragma_strings(program, interner, f) {
        match strings.first().and_then(|t| t.as_deref()) {
            Some("vm:external-name") => native = strings.get(1).cloned().flatten(),
            Some("vm:recognized") => recognized = true,
            _ => {}
        }
    }
    (native, recognized)
}

/// The JavaScript of an `external` function of the SDK that is a snippet of the
/// WebAssembly target (`@pragma('dartforge:js', '<code>')`, `crate::js_interop`).
/// Only the SDK's libraries declare snippets: a user library's pragma is not one.
pub fn js_snippet(
    program: &dartforge_elements::Program,
    interner: &dartforge_intern::Interner,
    f: &dartforge_elements::model::FunctionElement,
) -> Option<String> {
    if !f.external || !program.library(f.library).uri.starts_with("dart:") {
        return None;
    }
    pragma_strings(program, interner, f)
        .into_iter()
        .find(|strings| strings.first().and_then(|t| t.as_deref()) == Some("dartforge:js"))
        .and_then(|strings| strings.get(1).cloned().flatten())
}

/// The function an `external` of the SDK returns the address of
/// (`@pragma('dartforge:address-of', '<name>')`: a top-level function of the same
/// library), for the runtime to call it back (the WebAssembly target's
/// `Function.toJS`, `dart:_js_helper`).
pub fn address_of(
    program: &dartforge_elements::Program,
    interner: &dartforge_intern::Interner,
    f: &dartforge_elements::model::FunctionElement,
) -> Option<String> {
    if !f.external || !program.library(f.library).uri.starts_with("dart:") {
        return None;
    }
    pragma_strings(program, interner, f)
        .into_iter()
        .find(|strings| strings.first().and_then(|t| t.as_deref()) == Some("dartforge:address-of"))
        .and_then(|strings| strings.get(1).cloned().flatten())
}

/// The `int` parameters (by name, and `return` for the result) of a snippet
/// that cross as JavaScript `BigInt`s (`@pragma('dartforge:js-int64',
/// 'value,return')`).
pub fn js_int64(
    program: &dartforge_elements::Program,
    interner: &dartforge_intern::Interner,
    f: &dartforge_elements::model::FunctionElement,
) -> Vec<String> {
    pragma_strings(program, interner, f)
        .into_iter()
        .filter(|strings| strings.first().and_then(|t| t.as_deref()) == Some("dartforge:js-int64"))
        .filter_map(|strings| strings.get(1).cloned().flatten())
        .flat_map(|names| {
            names
                .split(',')
                .map(|n| n.trim().to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The arguments of each `@pragma` of the function: the text of each string
/// literal, `None` for another expression.
fn pragma_strings(
    program: &dartforge_elements::Program,
    interner: &dartforge_intern::Interner,
    f: &dartforge_elements::model::FunctionElement,
) -> Vec<Vec<Option<String>>> {
    use dartforge_elements::model::FunctionRef;
    use dartforge_frontend::ast;
    // The pragma lives on the `Member`/`Decl` that declares the function.
    let metadata: &[ast::Annotation] = match f.node {
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            a.members
                .iter()
                .find(|m| matches!(m.kind, ast::MemberKind::Method(id) if id == function))
                .map(|m| &*m.metadata)
                .or_else(|| {
                    a.decls
                        .iter()
                        .find(|d| matches!(d.kind, ast::DeclKind::Function(id) if id == function))
                        .map(|d| &*d.metadata)
                })
                .unwrap_or(&[])
        }
        FunctionRef::Constructor { unit, member } => {
            &program.unit(unit).ast.members[member.0 as usize].metadata
        }
        FunctionRef::None => &[],
    };
    let unit = match f.node {
        FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => unit,
        FunctionRef::None => return Vec::new(),
    };
    metadata
        .iter()
        .filter(|an| an.name.len() == 1 && interner.resolve(an.name[0].sym) == "pragma")
        .filter_map(|an| an.arguments.as_ref())
        .map(|args| {
            args.args
                .iter()
                .map(
                    |a| match &program.unit(unit).ast.exprs[a.value.0 as usize].kind {
                        ast::ExprKind::String(lit) => lit
                            .constant_value()
                            .map(|t| String::from_utf8_lossy(t.as_bytes()).into_owned()),
                        _ => None,
                    },
                )
                .collect()
        })
        .collect()
}

/// The unpatched `external`s of the given libraries, with the pragma that
/// implements them. An `external` that has a patch does not come in: what implements it is the
/// patch member (which comes in, if it is `external` too).
pub fn inventory(
    program: &dartforge_elements::Program,
    interner: &dartforge_intern::Interner,
    libraries: &[&str],
) -> Vec<SdkExternal> {
    let mut exit = Vec::new();
    for f in &program.functions {
        if !f.external || f.patched_by.is_some() {
            continue;
        }
        let lib = program.library(f.library);
        let Some(lib_name) = lib.uri.strip_prefix("dart:") else {
            continue;
        };
        if !libraries.contains(&lib_name) {
            continue;
        }
        let (native, recognized) = pragmas(program, interner, f);
        let owner = f.class.map(|c| {
            interner
                .resolve(program.classes[c.0 as usize].name)
                .to_string()
        });
        let name = interner.resolve(f.name).to_string();
        exit.push(SdkExternal {
            library: lib.uri.clone(),
            member: match owner {
                Some(c) => format!("{c}.{name}"),
                None => name,
            },
            native,
            recognized,
        });
    }
    exit.sort_by(|a, b| (&a.library, &a.member).cmp(&(&b.library, &b.member)));
    exit
}

#[cfg(test)]
mod inventory {
    use super::*;

    #[test]
    fn table_is_sorted_and_without_duplicates() {
        for par in NATIVES.windows(2) {
            assert!(
                par[0].name < par[1].name,
                "{} before {}",
                par[0].name,
                par[1].name
            );
        }
        assert!(native("String_getLength").is_some());
        assert!(native("Inexistente").is_none());
    }

    /// Every source native (with the overlay of each destination) has an
    /// entry, and every entry is still a source native of some destination or a
    /// versioned package runtime contract.
    #[test]
    fn every_source_native_has_entry() {
        crate::sdk_tests();
        let externals = std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(|| {
                let mut all = Vec::new();
                for (target, libraries) in [
                    (
                        crate::sdk_module::OVERLAY_TARGET,
                        crate::sdk_module::SOURCE_LIBRARIES,
                    ),
                    (
                        crate::sdk_module::WASM_OVERLAY_TARGET,
                        crate::sdk_module::WASM_SOURCE_LIBRARIES,
                    ),
                ] {
                    let sdk = crate::sdk_module::sdk_sources_layout(
                        dartforge_elements::sdk::SdkLayout::load_with_overlay(
                            crate::sdk_tests(),
                            &crate::sdk_module::overlay_dir(),
                            target,
                        )
                        .unwrap(),
                    );
                    let tmp = tempfile::tempdir().unwrap();
                    let entry = tmp.path().join("main.dart");
                    let mut source = String::new();
                    for b in libraries {
                        source.push_str(&format!("import 'dart:{b}';\n"));
                    }
                    source.push_str("void main() {}\n");
                    std::fs::write(&entry, source).unwrap();
                    let mut interner = dartforge_intern::Interner::new();
                    let (program, diags) =
                        dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
                    assert!(diags.is_empty(), "{target}: {diags:?}");
                    all.extend(inventory(&program, &interner, libraries));
                }
                all
            })
            .unwrap()
            .join()
            .unwrap();
        let names: std::collections::BTreeSet<&str> = externals
            .iter()
            .filter_map(|e| e.native.as_deref())
            .collect();
        let without_entry: Vec<&&str> = names.iter().filter(|n| native(n).is_none()).collect();
        assert!(
            without_entry.is_empty(),
            "source natives without an entry in NATIVES: {without_entry:?}"
        );
        // These portable package bodies are replaced only after declaration
        // identity and contract version are checked by the lowering pass.
        // native_pump_contract_tests checks the actual package declarations.
        let package_contracts = [
            "DartForge_native_pump_clear_dispatcher",
            "DartForge_native_pump_disable",
            "DartForge_native_pump_dispatcher_supported",
            "DartForge_native_pump_enable",
            "DartForge_native_pump_set_dispatcher",
            "DartForge_native_pump_supported",
        ];
        let leftover: Vec<&str> = NATIVES
            .iter()
            .map(|n| n.name)
            .filter(|n| !names.contains(n) && !package_contracts.contains(n))
            .collect();
        assert!(
            leftover.is_empty(),
            "NATIVES entries that are no longer source natives: {leftover:?}"
        );
        // An unpatched `external` without a native nor `vm:recognized` has no
        // implementation anywhere — not even in the VM.
        let orphans: Vec<String> = externals
            .iter()
            .filter(|e| e.native.is_none() && !e.recognized)
            .map(|e| format!("{}::{}", e.library, e.member))
            .collect();
        println!(
            "externals: {} distinct natives, {} intrinsics, {} without a pragma",
            names.len(),
            externals
                .iter()
                .filter(|e| e.native.is_none() && e.recognized)
                .count(),
            orphans.len()
        );
        for o in &orphans {
            println!("  without a pragma: {o}");
        }
    }
}
