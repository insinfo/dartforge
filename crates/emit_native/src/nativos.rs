//! A tabela de natives do SDK da fonte (P5b, docs/NATIVO-PLANO.md §7).
//!
//! No SDK da VM, um membro `external` sem patch é implementado de um de dois
//! jeitos: **native** — `@pragma("vm:external-name", "Nome")`, uma função C++
//! do runtime da VM (`runtime/lib/*.cc`) — ou **intrínseco** —
//! `@pragma("vm:recognized", ...)` sem nome de native, que o compilador da
//! VM gera no lugar da chamada (aritmética de `double`, `[]` de `_List`,
//! campos invisíveis de `_GrowableList`/`_compact_hash`/`Record`…). O nativo
//! faz o mesmo: todo native das [`BIBLIOTECAS_DA_FONTE`] tem uma entrada
//! aqui, com o que o backend faz com ele.
//!
//! * [`Estado::Runtime`]: uma função do runtime Rust,
//!   `dartforge_nativo_<Nome>` (o fragmento `nativos` de `crates/runtime`),
//!   com a assinatura na representação da HIR (R1) dos tipos Dart
//!   declarados: `int` → `i64`, `double` → `double`, `bool` → `i8`, o resto
//!   (receptor inclusive) → `Ref`. Erros saem pela exceção pendente.
//! * [`Estado::Pendente`]: ainda sem implementação; o lowering de P5d
//!   transforma a chamada em diagnóstico (N1), nunca em chamada a símbolo
//!   inexistente.
//!
//! Os efeitos seguem G8 (NATIVO-PLANO §6.5): marcados de forma conservadora
//! (aloca, lança) até um teste sob `DARTFORGE_GC_STRESS=1` provar o
//! contrário.
//!
//! O teste `inventario::toda_native_da_fonte_tem_entrada` carrega o SDK com a
//! sobreposição, percorre os `external` sem patch das bibliotecas da fonte e
//! recusa native sem entrada (e entrada que não é mais native): é o que
//! impede um patch novo do SDK — ou da sobreposição — de entrar em silêncio.
//!
//! [`BIBLIOTECAS_DA_FONTE`]: crate::sdk_modulo::BIBLIOTECAS_DA_FONTE

/// O que o backend faz com um native.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Função do runtime `dartforge_nativo_<Nome>`.
    Runtime,
    /// Sem implementação ainda: chamar é diagnóstico.
    Pendente,
    /// O lowering gera o código no lugar da chamada (`Internal_unsafeCast`
    /// é o próprio valor).
    Embutido,
}

/// Efeitos de uma chamada (G8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Efeitos {
    pub aloca: bool,
    pub lanca: bool,
    pub chama_dart: bool,
}

/// Conservador: aloca e lança, não chama Dart.
pub const CONSERVADOR: Efeitos = Efeitos { aloca: true, lanca: true, chama_dart: false };

/// Um native de `@pragma("vm:external-name", nome)`.
#[derive(Debug, Clone, Copy)]
pub struct Nativo {
    pub nome: &'static str,
    pub estado: Estado,
    pub efeitos: Efeitos,
}

const fn runtime(nome: &'static str) -> Nativo {
    Nativo { nome, estado: Estado::Runtime, efeitos: CONSERVADOR }
}

const fn embutido(nome: &'static str) -> Nativo {
    Nativo { nome, estado: Estado::Embutido, efeitos: CONSERVADOR }
}

/// Intrínsecos da VM (`vm:recognized` sem native) que o nativo implementa
/// com uma função do runtime, pelo `Classe.membro`: (membro, native).
pub const INTRINSECOS: &[(&str, &str)] = &[
    ("_Array.[]", "DartForge_lista_get"),
    ("_Double._modulo", "DartForge_double_modulo"),
    ("_Double._remainder", "DartForge_double_remainder"),
    ("_Double.ceilToDouble", "DartForge_double_ceil"),
    ("_Double.floorToDouble", "DartForge_double_floor"),
    ("_Double.hashCode", "DartForge_double_hashCode"),
    ("_Double.roundToDouble", "DartForge_double_round"),
    ("_Double.toInt", "DartForge_double_toInt"),
    ("_Double.truncateToDouble", "DartForge_double_truncate"),
    ("_Float32ArrayView.[]", "DartForge_typed_indexar_double"),
    ("_Float32List.[]", "DartForge_typed_indexar_double"),
    ("_Float64ArrayView.[]", "DartForge_typed_indexar_double"),
    ("_Float64List.[]", "DartForge_typed_indexar_double"),
    ("_GrowableList.[]", "DartForge_lista_get"),
    ("_ImmutableList.[]", "DartForge_lista_get"),
    ("_Int16ArrayView.[]", "DartForge_typed_indexar_int"),
    ("_Int16List.[]", "DartForge_typed_indexar_int"),
    ("_Int32ArrayView.[]", "DartForge_typed_indexar_int"),
    ("_Int32List.[]", "DartForge_typed_indexar_int"),
    ("_Int64ArrayView.[]", "DartForge_typed_indexar_int"),
    ("_Int64List.[]", "DartForge_typed_indexar_int"),
    ("_Int8ArrayView.[]", "DartForge_typed_indexar_int"),
    ("_Int8List.[]", "DartForge_typed_indexar_int"),
    ("_List.[]", "DartForge_lista_get"),
    ("_Mint.hashCode", "DartForge_int_hashCode"),
    ("_Record._fieldAt", "DartForge_record_fieldAt"),
    ("_Record._fieldNames", "DartForge_record_fieldNames"),
    ("_Record._numFields", "DartForge_record_numFields"),
    ("_Record._shape", "DartForge_record_shape"),
    ("_Smi.hashCode", "DartForge_int_hashCode"),
    ("_StringBase.codeUnitAt", "DartForge_string_codeUnitAt"),
    ("_TypedList._getFloat32", "TypedData_GetFloat32"),
    ("_TypedList._getFloat64", "TypedData_GetFloat64"),
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
    ("_Uint16ArrayView.[]", "DartForge_typed_indexar_int"),
    ("_Uint16List.[]", "DartForge_typed_indexar_int"),
    ("_Uint32ArrayView.[]", "DartForge_typed_indexar_int"),
    ("_Uint32List.[]", "DartForge_typed_indexar_int"),
    ("_Uint64ArrayView.[]", "DartForge_typed_indexar_int"),
    ("_Uint64List.[]", "DartForge_typed_indexar_int"),
    ("_Uint8ArrayView.[]", "DartForge_typed_indexar_int"),
    ("_Uint8ClampedArrayView.[]", "DartForge_typed_indexar_int"),
    ("_Uint8ClampedList.[]", "DartForge_typed_indexar_int"),
    ("_Uint8List.[]", "DartForge_typed_indexar_int"),
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
    ("has63BitSmis", "DartForge_verdadeiro"),
];

/// A função do runtime de um intrínseco da VM, se o nativo a tem.
pub fn intrinseco(membro: &str) -> Option<&'static str> {
    INTRINSECOS.iter().find(|(m, _)| *m == membro).map(|(_, n)| *n)
}

const fn pendente(nome: &'static str) -> Nativo {
    Nativo { nome, estado: Estado::Pendente, efeitos: CONSERVADOR }
}

/// Todo native das bibliotecas da fonte com a sobreposição `sdk_nativo/`,
/// em ordem de bytes (a de `str`, que a busca binária usa). Os `DartForge_*` são os da sobreposição.
pub const NATIVOS: &[Nativo] = &[
    pendente("AbstractType_equality"),
    pendente("AbstractType_getHashCode"),
    pendente("AbstractType_toString"),
    pendente("AssertionError_throwNew"),
    pendente("AssertionError_throwNewSource"),
    pendente("Bool_fromEnvironment"),
    pendente("Bool_hasEnvironment"),
    runtime("ClassID_getID"),
    pendente("Closure_computeHash"),
    runtime("Closure_equals"),
    runtime("Crypto_GetRandomBytes"),
    runtime("DartForge_Timer_cancelar"),
    runtime("DartForge_Timer_novo"),
    runtime("DartForge_capacidade_nova"),
    runtime("DartForge_classe_nao_enviavel"),
    runtime("DartForge_classe_transferivel"),
    runtime("DartForge_imprimir"),
    runtime("DartForge_mensagem_atual"),
    runtime("DartForge_porta_abrir"),
    runtime("DartForge_porta_atual"),
    runtime("DartForge_porta_enviar"),
    runtime("DartForge_porta_fechar"),
    runtime("DartForge_porta_mantem_vivo"),
    runtime("DartForge_porta_manter_vivo"),
    runtime("DartForge_porta_recusa"),
    runtime("DartForge_portas_despachante"),
    runtime("DartForge_regexp_captura"),
    runtime("DartForge_regexp_compilar"),
    runtime("DartForge_regexp_erro"),
    runtime("DartForge_regexp_executar"),
    runtime("DartForge_regexp_grupos"),
    runtime("DartForge_regexp_indice_do_nome"),
    runtime("DartForge_regexp_n_nomes"),
    runtime("DartForge_regexp_nome"),
    runtime("DartForge_scheduleImmediate"),
    runtime("DateTime_currentTimeMicros"),
    runtime("DateTime_timeZoneName"),
    runtime("DateTime_timeZoneOffsetInSeconds"),
    runtime("Developer_NativeRuntime_buildId"),
    pendente("Developer_NativeRuntime_writeHeapSnapshotToFile"),
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
    runtime("Double_add"),
    runtime("Double_div"),
    runtime("Double_doubleFromInteger"),
    runtime("Double_equal"),
    runtime("Double_equalToInteger"),
    runtime("Double_flipSignBit"),
    runtime("Double_getIsInfinite"),
    runtime("Double_getIsNaN"),
    runtime("Double_getIsNegative"),
    runtime("Double_greaterThan"),
    runtime("Double_greaterThanFromInteger"),
    runtime("Double_mul"),
    runtime("Double_parse"),
    runtime("Double_sub"),
    runtime("Double_toString"),
    runtime("Double_toStringAsExponential"),
    runtime("Double_toStringAsFixed"),
    runtime("Double_toStringAsPrecision"),
    runtime("Error_throwWithStackTrace"),
    runtime("Error_trySetStackTrace"),
    runtime("EventHandler_SendData"),
    runtime("EventHandler_TimerMillisecondClock"),
    pendente("FileSystemWatcher_CloseWatcher"),
    pendente("FileSystemWatcher_GetSocketId"),
    pendente("FileSystemWatcher_InitWatcher"),
    pendente("FileSystemWatcher_IsSupported"),
    pendente("FileSystemWatcher_ReadEvents"),
    pendente("FileSystemWatcher_UnwatchPath"),
    pendente("FileSystemWatcher_WatchPath"),
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
    pendente("Filter_CreateZLibDeflate"),
    pendente("Filter_CreateZLibInflate"),
    pendente("Filter_Process"),
    pendente("Filter_Processed"),
    pendente("FinalizerEntry_allocate"),
    pendente("Float32x4_abs"),
    pendente("Float32x4_add"),
    pendente("Float32x4_clamp"),
    pendente("Float32x4_cmpequal"),
    pendente("Float32x4_cmpgt"),
    pendente("Float32x4_cmpgte"),
    pendente("Float32x4_cmplt"),
    pendente("Float32x4_cmplte"),
    pendente("Float32x4_cmpnequal"),
    pendente("Float32x4_div"),
    pendente("Float32x4_fromDoubles"),
    pendente("Float32x4_fromFloat64x2"),
    pendente("Float32x4_fromInt32x4Bits"),
    pendente("Float32x4_getSignMask"),
    pendente("Float32x4_getW"),
    pendente("Float32x4_getX"),
    pendente("Float32x4_getY"),
    pendente("Float32x4_getZ"),
    pendente("Float32x4_max"),
    pendente("Float32x4_min"),
    pendente("Float32x4_mul"),
    pendente("Float32x4_negate"),
    pendente("Float32x4_reciprocal"),
    pendente("Float32x4_reciprocalSqrt"),
    pendente("Float32x4_scale"),
    pendente("Float32x4_setW"),
    pendente("Float32x4_setX"),
    pendente("Float32x4_setY"),
    pendente("Float32x4_setZ"),
    pendente("Float32x4_shuffle"),
    pendente("Float32x4_shuffleMix"),
    pendente("Float32x4_splat"),
    pendente("Float32x4_sqrt"),
    pendente("Float32x4_sub"),
    pendente("Float32x4_zero"),
    pendente("Float64x2_abs"),
    pendente("Float64x2_add"),
    pendente("Float64x2_clamp"),
    pendente("Float64x2_div"),
    pendente("Float64x2_fromDoubles"),
    pendente("Float64x2_fromFloat32x4"),
    pendente("Float64x2_getSignMask"),
    pendente("Float64x2_getX"),
    pendente("Float64x2_getY"),
    pendente("Float64x2_max"),
    pendente("Float64x2_min"),
    pendente("Float64x2_mul"),
    pendente("Float64x2_negate"),
    pendente("Float64x2_scale"),
    pendente("Float64x2_setX"),
    pendente("Float64x2_setY"),
    pendente("Float64x2_splat"),
    pendente("Float64x2_sqrt"),
    pendente("Float64x2_sub"),
    pendente("Float64x2_zero"),
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
    pendente("Int32x4_add"),
    pendente("Int32x4_and"),
    pendente("Int32x4_fromBools"),
    pendente("Int32x4_fromFloat32x4Bits"),
    pendente("Int32x4_fromInts"),
    pendente("Int32x4_getFlagW"),
    pendente("Int32x4_getFlagX"),
    pendente("Int32x4_getFlagY"),
    pendente("Int32x4_getFlagZ"),
    pendente("Int32x4_getSignMask"),
    pendente("Int32x4_getW"),
    pendente("Int32x4_getX"),
    pendente("Int32x4_getY"),
    pendente("Int32x4_getZ"),
    pendente("Int32x4_or"),
    pendente("Int32x4_select"),
    pendente("Int32x4_setFlagW"),
    pendente("Int32x4_setFlagX"),
    pendente("Int32x4_setFlagY"),
    pendente("Int32x4_setFlagZ"),
    pendente("Int32x4_setW"),
    pendente("Int32x4_setX"),
    pendente("Int32x4_setY"),
    pendente("Int32x4_setZ"),
    pendente("Int32x4_shuffle"),
    pendente("Int32x4_shuffleMix"),
    pendente("Int32x4_sub"),
    pendente("Int32x4_xor"),
    runtime("Integer_addFromInteger"),
    runtime("Integer_bitAndFromInteger"),
    runtime("Integer_bitOrFromInteger"),
    runtime("Integer_bitXorFromInteger"),
    runtime("Integer_equalToInteger"),
    pendente("Integer_fromEnvironment"),
    runtime("Integer_greaterThanFromInteger"),
    runtime("Integer_moduloFromInteger"),
    runtime("Integer_mulFromInteger"),
    runtime("Integer_shlFromInteger"),
    runtime("Integer_shrFromInteger"),
    runtime("Integer_subFromInteger"),
    runtime("Integer_truncDivFromInteger"),
    runtime("Integer_ushrFromInteger"),
    pendente("Internal_allocateObjectInstructionsEnd"),
    pendente("Internal_allocateObjectInstructionsStart"),
    runtime("Internal_allocateOneByteString"),
    runtime("Internal_allocateTwoByteString"),
    pendente("Internal_boundsCheckForPartialInstantiation"),
    pendente("Internal_collectAllGarbage"),
    pendente("Internal_deoptimizeFunctionsOnStack"),
    pendente("Internal_extractTypeArguments"),
    pendente("Internal_loadDynamicModule"),
    runtime("Internal_makeFixedListUnmodifiable"),
    runtime("Internal_makeListFixedLength"),
    pendente("Internal_nativeEffect"),
    pendente("Internal_prependTypeArguments"),
    embutido("Internal_unsafeCast"),
    runtime("Internal_writeIntoOneByteString"),
    runtime("Internal_writeIntoTwoByteString"),
    runtime("InternetAddress_Parse"),
    runtime("InternetAddress_ParseScopedLinkLocalAddress"),
    runtime("InternetAddress_RawAddrToString"),
    pendente("InvocationMirror_unpackTypeArguments"),
    runtime("Isolate_exit_"),
    runtime("Isolate_getCurrentRootUriStr"),
    runtime("Isolate_getDebugName"),
    runtime("Isolate_getPortAndCapabilitiesOfCurrentIsolate"),
    pendente("Isolate_registerKernelBlob"),
    runtime("Isolate_sendOOB"),
    runtime("Isolate_spawnFunction"),
    runtime("Isolate_spawnUri"),
    pendente("Isolate_unregisterKernelBlob"),
    pendente("LibraryPrefix_isLoaded"),
    pendente("LibraryPrefix_issueLoad"),
    pendente("LibraryPrefix_loadingUnit"),
    pendente("LibraryPrefix_setLoaded"),
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
    pendente("Object_instanceOf"),
    runtime("Object_runtimeType"),
    pendente("Object_simpleInstanceOf"),
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
    pendente("Profiler_getCurrentTag"),
    runtime("Random_initialSeed"),
    runtime("RawSocketOption_GetOptionValue"),
    pendente("ResourceHandleImpl_toFile"),
    pendente("ResourceHandleImpl_toRawDatagramSocket"),
    pendente("ResourceHandleImpl_toRawSocket"),
    pendente("ResourceHandleImpl_toSocket"),
    runtime("SecureRandom_getBytes"),
    pendente("SecureSocket_Connect"),
    pendente("SecureSocket_Destroy"),
    pendente("SecureSocket_FilterPointer"),
    pendente("SecureSocket_GetSelectedProtocol"),
    pendente("SecureSocket_Handshake"),
    pendente("SecureSocket_Init"),
    pendente("SecureSocket_MarkAsTrusted"),
    pendente("SecureSocket_NewX509CertificateWrapper"),
    pendente("SecureSocket_PeerCertificate"),
    pendente("SecureSocket_RegisterBadCertificateCallback"),
    pendente("SecureSocket_RegisterHandshakeCompleteCallback"),
    pendente("SecureSocket_RegisterKeyLogPort"),
    pendente("SecurityContext_Allocate"),
    pendente("SecurityContext_GetMinimumProtocolVersion"),
    pendente("SecurityContext_SetAllowTlsRenegotiation"),
    pendente("SecurityContext_SetAlpnProtocols"),
    pendente("SecurityContext_SetClientAuthoritiesBytes"),
    pendente("SecurityContext_SetMinimumProtocolVersion"),
    pendente("SecurityContext_SetTrustedCertificatesBytes"),
    pendente("SecurityContext_TrustBuiltinRoots"),
    pendente("SecurityContext_UseCertificateChainBytes"),
    pendente("SecurityContext_UsePrivateKeyBytes"),
    runtime("ServerSocket_Accept"),
    runtime("ServerSocket_CreateBindListen"),
    runtime("ServerSocket_CreateUnixDomainBindListen"),
    runtime("Smi_bitLength"),
    runtime("Smi_bitNegate"),
    runtime("SocketBase_IsBindError"),
    pendente("SocketControlMessageImpl_extractHandles"),
    pendente("SocketControlMessage_fromHandles"),
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
    pendente("Socket_JoinMulticast"),
    pendente("Socket_LeaveMulticast"),
    runtime("Socket_Read"),
    pendente("Socket_ReceiveMessage"),
    runtime("Socket_RecvFrom"),
    pendente("Socket_SendMessage"),
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
    pendente("StringBase_intern"),
    runtime("StringBase_joinReplaceAllResult"),
    runtime("StringBase_substringUnchecked"),
    runtime("StringToSystemEncoding"),
    runtime("String_charAt"),
    runtime("String_concat"),
    runtime("String_concatRange"),
    pendente("String_fromEnvironment"),
    runtime("String_getHashCode"),
    runtime("String_getLength"),
    runtime("String_toLowerCase"),
    runtime("String_toUpperCase"),
    pendente("SynchronousSocket_Available"),
    pendente("SynchronousSocket_CloseSync"),
    pendente("SynchronousSocket_CreateConnectSync"),
    pendente("SynchronousSocket_GetPort"),
    pendente("SynchronousSocket_GetRemotePeer"),
    pendente("SynchronousSocket_LookupRequest"),
    pendente("SynchronousSocket_Read"),
    pendente("SynchronousSocket_ReadList"),
    pendente("SynchronousSocket_ShutdownRead"),
    pendente("SynchronousSocket_ShutdownWrite"),
    pendente("SynchronousSocket_WriteList"),
    runtime("SystemEncodingToString"),
    runtime("Timeline_getNextTaskId"),
    runtime("Timeline_getTraceClock"),
    runtime("Timeline_isDartStreamEnabled"),
    runtime("Timeline_reportTaskEvent"),
    runtime("TwoByteString_allocateFromTwoByteList"),
    pendente("TypeError_throwNew"),
    pendente("Type_equality"),
    runtime("TypedDataBase_length"),
    runtime("TypedDataBase_setClampedRange"),
    runtime("TypedDataView_offsetInBytes"),
    runtime("TypedDataView_typedData"),
    runtime("TypedData_GetFloat32"),
    pendente("TypedData_GetFloat32x4"),
    runtime("TypedData_GetFloat64"),
    pendente("TypedData_GetFloat64x2"),
    pendente("TypedData_GetInt32x4"),
    runtime("TypedData_SetFloat32"),
    pendente("TypedData_SetFloat32x4"),
    runtime("TypedData_SetFloat64"),
    pendente("TypedData_SetFloat64x2"),
    pendente("TypedData_SetInt32x4"),
    runtime("Uri_isWindowsPlatform"),
    pendente("UserTag_defaultTag"),
    pendente("UserTag_label"),
    pendente("UserTag_makeCurrent"),
    pendente("UserTag_new"),
    pendente("WeakProperty_getKey"),
    pendente("WeakProperty_getValue"),
    pendente("WeakProperty_setKey"),
    pendente("WeakProperty_setValue"),
    pendente("WeakReference_getTarget"),
    pendente("WeakReference_setTarget"),
    pendente("X509_Der"),
    pendente("X509_EndValidity"),
    pendente("X509_Issuer"),
    pendente("X509_Pem"),
    pendente("X509_Sha1"),
    pendente("X509_StartValidity"),
    pendente("X509_Subject"),
];

/// A entrada de um native, se existe.
pub fn nativo(nome: &str) -> Option<&'static Nativo> {
    NATIVOS.binary_search_by(|n| n.nome.cmp(nome)).ok().map(|i| &NATIVOS[i])
}

/// O símbolo do runtime que implementa um native.
pub fn simbolo(nome: &str) -> String {
    format!("dartforge_nativo_{nome}")
}

/// Um `external` sem patch das bibliotecas da fonte: o que o implementa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDoSdk {
    /// `dart:core`…
    pub biblioteca: String,
    /// `Classe.membro` ou `membro` (de topo).
    pub membro: String,
    /// O nome do native, se é um.
    pub native: Option<String>,
    /// Tem `@pragma("vm:recognized", ...)` (intrínseco da VM).
    pub reconhecido: bool,
}

/// O que os `@pragma` de uma função dizem: o nome do native
/// (`vm:external-name`) e se ela é `vm:recognized` (intrínseco).
pub fn pragmas(
    program: &dartforge_elements::Program,
    interner: &dartforge_intern::Interner,
    f: &dartforge_elements::model::FunctionElement,
) -> (Option<String>, bool) {
    use dartforge_elements::model::FunctionRef;
    use dartforge_frontend::ast;
    // O pragma mora no `Member`/`Decl` que declara a função.
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
        FunctionRef::Constructor { unit, member } => &program.unit(unit).ast.members[member.0 as usize].metadata,
        FunctionRef::None => &[],
    };
    let unit = match f.node {
        FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => Some(unit),
        FunctionRef::None => None,
    };
    let mut native = None;
    let mut reconhecido = false;
    for an in metadata {
        let Some(u) = unit else { break };
        if an.name.len() != 1 || interner.resolve(an.name[0].sym) != "pragma" {
            continue;
        }
        let Some(args) = &an.arguments else { continue };
        let textos: Vec<Option<String>> = args
            .args
            .iter()
            .map(|a| match &program.unit(u).ast.exprs[a.value.0 as usize].kind {
                ast::ExprKind::String(lit) => {
                    lit.constant_value().map(|t| String::from_utf8_lossy(t.as_bytes()).into_owned())
                }
                _ => None,
            })
            .collect();
        match textos.first().and_then(|t| t.as_deref()) {
            Some("vm:external-name") => native = textos.get(1).cloned().flatten(),
            Some("vm:recognized") => reconhecido = true,
            _ => {}
        }
    }
    (native, reconhecido)
}

/// Os `external` sem patch das bibliotecas dadas, com o pragma que os
/// implementa. Um `external` que tem patch não entra: quem o implementa é o
/// membro do patch (que entra, se for `external` também).
pub fn inventario(
    program: &dartforge_elements::Program,
    interner: &dartforge_intern::Interner,
    bibliotecas: &[&str],
) -> Vec<ExternalDoSdk> {
    let mut saida = Vec::new();
    for f in &program.functions {
        if !f.external || f.patched_by.is_some() {
            continue;
        }
        let lib = program.library(f.library);
        let Some(nome_lib) = lib.uri.strip_prefix("dart:") else { continue };
        if !bibliotecas.contains(&nome_lib) {
            continue;
        }
        let (native, reconhecido) = pragmas(program, interner, f);
        let dono = f.class.map(|c| interner.resolve(program.classes[c.0 as usize].name).to_string());
        let nome = interner.resolve(f.name).to_string();
        saida.push(ExternalDoSdk {
            biblioteca: lib.uri.clone(),
            membro: match dono {
                Some(c) => format!("{c}.{nome}"),
                None => nome,
            },
            native,
            reconhecido,
        });
    }
    saida.sort_by(|a, b| (&a.biblioteca, &a.membro).cmp(&(&b.biblioteca, &b.membro)));
    saida
}

#[cfg(test)]
mod inventario {
    use super::*;
    use std::path::Path;

    /// O SDK do teste: `DARTFORGE_TEST_SDK_LIB`, senão o descoberto, senão o

    /// caminho da máquina de desenvolvimento.

    static SDK_DIR: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {

        std::env::var("DARTFORGE_TEST_SDK_LIB")

            .ok()

            .or_else(|| dartforge_elements::sdk::SdkLayout::discover().map(|p| p.to_string_lossy().into_owned()))

            .unwrap_or_else(|| "C:/tools/dartsdk-3.6.2/lib".to_string())

    });

    #[test]
    fn a_tabela_esta_em_ordem_e_sem_repeticao() {
        for par in NATIVOS.windows(2) {
            assert!(par[0].nome < par[1].nome, "{} antes de {}", par[0].nome, par[1].nome);
        }
        assert!(nativo("String_getLength").is_some());
        assert!(nativo("Inexistente").is_none());
    }

    /// Todo native da fonte (com a sobreposição) tem entrada, e toda entrada
    /// ainda é um native da fonte.
    #[test]
    fn toda_native_da_fonte_tem_entrada() {
        if !Path::new(SDK_DIR.as_str()).join("libraries.json").is_file() {
            eprintln!("SDK ausente em {}; teste pulado", SDK_DIR.as_str());
            return;
        }
        let externos = std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(|| {
                let sdk = crate::sdk_modulo::carregar_sdk_nativo(Path::new(SDK_DIR.as_str())).unwrap();
                let tmp = tempfile::tempdir().unwrap();
                let entrada = tmp.path().join("main.dart");
                let mut fonte = String::new();
                for b in crate::sdk_modulo::BIBLIOTECAS_DA_FONTE {
                    fonte.push_str(&format!("import 'dart:{b}';\n"));
                }
                fonte.push_str("void main() {}\n");
                std::fs::write(&entrada, fonte).unwrap();
                let mut interner = dartforge_intern::Interner::new();
                let (program, diags) = dartforge_elements::load::load_lenient(&entrada, &sdk, None, &mut interner);
                assert!(diags.is_empty(), "{diags:?}");
                inventario(&program, &interner, crate::sdk_modulo::BIBLIOTECAS_DA_FONTE)
            })
            .unwrap()
            .join()
            .unwrap();
        let nomes: std::collections::BTreeSet<&str> = externos.iter().filter_map(|e| e.native.as_deref()).collect();
        let sem_entrada: Vec<&&str> = nomes.iter().filter(|n| nativo(n).is_none()).collect();
        assert!(sem_entrada.is_empty(), "natives da fonte sem entrada em NATIVOS: {sem_entrada:?}");
        let sobrando: Vec<&str> = NATIVOS.iter().map(|n| n.nome).filter(|n| !nomes.contains(n)).collect();
        assert!(sobrando.is_empty(), "entradas de NATIVOS que não são mais natives da fonte: {sobrando:?}");
        // Um `external` sem patch sem native nem `vm:recognized` não tem
        // implementação em lugar nenhum — nem na VM.
        let orfaos: Vec<String> = externos
            .iter()
            .filter(|e| e.native.is_none() && !e.reconhecido)
            .map(|e| format!("{}::{}", e.biblioteca, e.membro))
            .collect();
        println!("externals: {} natives distintos, {} intrínsecos, {} sem pragma", nomes.len(), externos.iter().filter(|e| e.native.is_none() && e.reconhecido).count(), orfaos.len());
        for o in &orfaos {
            println!("  sem pragma: {o}");
        }
    }
}
