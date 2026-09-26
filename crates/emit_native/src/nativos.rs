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
    ("_GrowableList.[]", "DartForge_lista_get"),
    ("_ImmutableList.[]", "DartForge_lista_get"),
    ("_List.[]", "DartForge_lista_get"),
    ("_Mint.hashCode", "DartForge_int_hashCode"),
    ("_Record._fieldAt", "DartForge_record_fieldAt"),
    ("_Record._fieldNames", "DartForge_record_fieldNames"),
    ("_Record._numFields", "DartForge_record_numFields"),
    ("_Record._shape", "DartForge_record_shape"),
    ("_Smi.hashCode", "DartForge_int_hashCode"),
    ("_StringBase.codeUnitAt", "DartForge_string_codeUnitAt"),
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
    pendente("Crypto_GetRandomBytes"),
    runtime("DartForge_Timer_cancelar"),
    runtime("DartForge_Timer_novo"),
    runtime("DartForge_capacidade_nova"),
    runtime("DartForge_classe_nao_enviavel"),
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
    pendente("Developer_NativeRuntime_buildId"),
    pendente("Developer_NativeRuntime_writeHeapSnapshotToFile"),
    pendente("Developer_debugger"),
    pendente("Developer_getIsolateIdFromSendPort"),
    pendente("Developer_getObjectId"),
    pendente("Developer_getServerInfo"),
    pendente("Developer_getServiceMajorVersion"),
    pendente("Developer_getServiceMinorVersion"),
    pendente("Developer_inspect"),
    pendente("Developer_log"),
    pendente("Developer_lookupExtension"),
    pendente("Developer_postEvent"),
    pendente("Developer_reachability_barrier"),
    pendente("Developer_registerExtension"),
    pendente("Developer_webServerControl"),
    pendente("Directory_Create"),
    pendente("Directory_CreateTemp"),
    pendente("Directory_Current"),
    pendente("Directory_Delete"),
    pendente("Directory_Exists"),
    pendente("Directory_FillWithDirectoryListing"),
    pendente("Directory_GetAsyncDirectoryListerPointer"),
    pendente("Directory_Rename"),
    pendente("Directory_SetAsyncDirectoryListerPointer"),
    pendente("Directory_SetCurrent"),
    pendente("Directory_SystemTemp"),
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
    pendente("EventHandler_SendData"),
    pendente("EventHandler_TimerMillisecondClock"),
    pendente("FileSystemWatcher_CloseWatcher"),
    pendente("FileSystemWatcher_GetSocketId"),
    pendente("FileSystemWatcher_InitWatcher"),
    pendente("FileSystemWatcher_IsSupported"),
    pendente("FileSystemWatcher_ReadEvents"),
    pendente("FileSystemWatcher_UnwatchPath"),
    pendente("FileSystemWatcher_WatchPath"),
    pendente("File_AreIdentical"),
    pendente("File_Close"),
    pendente("File_Copy"),
    pendente("File_Create"),
    pendente("File_CreateLink"),
    pendente("File_CreatePipe"),
    pendente("File_Delete"),
    pendente("File_DeleteLink"),
    pendente("File_Exists"),
    pendente("File_Flush"),
    pendente("File_GetFD"),
    pendente("File_GetPointer"),
    pendente("File_GetStdioHandleType"),
    pendente("File_GetType"),
    pendente("File_LastAccessed"),
    pendente("File_LastModified"),
    pendente("File_Length"),
    pendente("File_LengthFromPath"),
    pendente("File_LinkTarget"),
    pendente("File_Lock"),
    pendente("File_Open"),
    pendente("File_OpenStdio"),
    pendente("File_Position"),
    pendente("File_Read"),
    pendente("File_ReadByte"),
    pendente("File_ReadInto"),
    pendente("File_Rename"),
    pendente("File_RenameLink"),
    pendente("File_ResolveSymbolicLinks"),
    pendente("File_SetLastAccessed"),
    pendente("File_SetLastModified"),
    pendente("File_SetPointer"),
    pendente("File_SetPosition"),
    pendente("File_Stat"),
    pendente("File_Truncate"),
    pendente("File_WriteByte"),
    pendente("File_WriteFrom"),
    pendente("Filter_CreateZLibDeflate"),
    pendente("Filter_CreateZLibInflate"),
    pendente("Filter_Process"),
    pendente("Filter_Processed"),
    pendente("FinalizerEntry_allocate"),
    pendente("FullyRecognizedMethod_NoNative"),
    runtime("Function_apply"),
    runtime("GrowableList_allocate"),
    runtime("GrowableList_getCapacity"),
    runtime("GrowableList_getLength"),
    runtime("GrowableList_setData"),
    runtime("GrowableList_setIndexed"),
    runtime("GrowableList_setLength"),
    pendente("IOService_NewServicePort"),
    runtime("Identical_comparison"),
    runtime("ImmutableList_from"),
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
    pendente("InternetAddress_Parse"),
    pendente("InternetAddress_ParseScopedLinkLocalAddress"),
    pendente("InternetAddress_RawAddrToString"),
    pendente("InvocationMirror_unpackTypeArguments"),
    pendente("Isolate_exit_"),
    pendente("Isolate_getCurrentRootUriStr"),
    pendente("Isolate_getDebugName"),
    pendente("Isolate_getPortAndCapabilitiesOfCurrentIsolate"),
    pendente("Isolate_registerKernelBlob"),
    pendente("Isolate_sendOOB"),
    pendente("Isolate_spawnFunction"),
    pendente("Isolate_spawnUri"),
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
    pendente("Namespace_Create"),
    pendente("Namespace_GetDefault"),
    pendente("Namespace_GetPointer"),
    runtime("NoSuchMethodError_existingMethodSignature"),
    pendente("OSError_inProgressErrorCode"),
    runtime("Object_equals"),
    runtime("Object_getHash"),
    runtime("Object_haveSameRuntimeType"),
    pendente("Object_instanceOf"),
    runtime("Object_runtimeType"),
    pendente("Object_simpleInstanceOf"),
    runtime("Object_toString"),
    runtime("OneByteString_allocateFromOneByteList"),
    runtime("OneByteString_substringUnchecked"),
    pendente("Platform_Environment"),
    pendente("Platform_ExecutableArguments"),
    pendente("Platform_ExecutableName"),
    pendente("Platform_GetVersion"),
    pendente("Platform_LocalHostname"),
    pendente("Platform_LocaleName"),
    pendente("Platform_NumberOfProcessors"),
    pendente("Platform_OperatingSystem"),
    pendente("Platform_OperatingSystemVersion"),
    pendente("Platform_PathSeparator"),
    pendente("Platform_ResolvedExecutableName"),
    pendente("ProcessInfo_CurrentRSS"),
    pendente("ProcessInfo_MaxRSS"),
    pendente("Process_ClearSignalHandler"),
    pendente("Process_Exit"),
    pendente("Process_GetExitCode"),
    pendente("Process_KillPid"),
    pendente("Process_Pid"),
    pendente("Process_SetExitCode"),
    pendente("Process_SetSignalHandler"),
    pendente("Process_Sleep"),
    pendente("Process_Start"),
    pendente("Process_Wait"),
    pendente("Profiler_getCurrentTag"),
    runtime("Random_initialSeed"),
    pendente("RawSocketOption_GetOptionValue"),
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
    pendente("ServerSocket_Accept"),
    pendente("ServerSocket_CreateBindListen"),
    pendente("ServerSocket_CreateUnixDomainBindListen"),
    runtime("Smi_bitLength"),
    runtime("Smi_bitNegate"),
    pendente("SocketBase_IsBindError"),
    pendente("SocketControlMessageImpl_extractHandles"),
    pendente("SocketControlMessage_fromHandles"),
    pendente("Socket_Available"),
    pendente("Socket_AvailableDatagram"),
    pendente("Socket_CreateBindConnect"),
    pendente("Socket_CreateBindDatagram"),
    pendente("Socket_CreateConnect"),
    pendente("Socket_CreateUnixDomainBindConnect"),
    pendente("Socket_CreateUnixDomainConnect"),
    pendente("Socket_Fatal"),
    pendente("Socket_GetError"),
    pendente("Socket_GetFD"),
    pendente("Socket_GetOption"),
    pendente("Socket_GetPort"),
    pendente("Socket_GetRawOption"),
    pendente("Socket_GetRemotePeer"),
    pendente("Socket_GetSocketId"),
    pendente("Socket_GetStdioHandle"),
    pendente("Socket_GetType"),
    pendente("Socket_HasPendingWrite"),
    pendente("Socket_JoinMulticast"),
    pendente("Socket_LeaveMulticast"),
    pendente("Socket_Read"),
    pendente("Socket_ReceiveMessage"),
    pendente("Socket_RecvFrom"),
    pendente("Socket_SendMessage"),
    pendente("Socket_SendTo"),
    pendente("Socket_SetOption"),
    pendente("Socket_SetRawOption"),
    pendente("Socket_SetSocketId"),
    pendente("Socket_WriteList"),
    runtime("StackTrace_current"),
    pendente("Stdin_AnsiSupported"),
    pendente("Stdin_GetEchoMode"),
    pendente("Stdin_GetEchoNewlineMode"),
    pendente("Stdin_GetLineMode"),
    pendente("Stdin_ReadByte"),
    pendente("Stdin_SetEchoMode"),
    pendente("Stdin_SetEchoNewlineMode"),
    pendente("Stdin_SetLineMode"),
    pendente("Stdout_AnsiSupported"),
    pendente("Stdout_GetTerminalSize"),
    runtime("Stopwatch_frequency"),
    runtime("Stopwatch_now"),
    runtime("StringBase_createFromCodePoints"),
    pendente("StringBase_intern"),
    runtime("StringBase_joinReplaceAllResult"),
    runtime("StringBase_substringUnchecked"),
    pendente("StringToSystemEncoding"),
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
    pendente("SystemEncodingToString"),
    pendente("Timeline_getNextTaskId"),
    pendente("Timeline_getTraceClock"),
    pendente("Timeline_isDartStreamEnabled"),
    pendente("Timeline_reportTaskEvent"),
    pendente("TransferableTypedData_factory"),
    pendente("TransferableTypedData_materialize"),
    runtime("TwoByteString_allocateFromTwoByteList"),
    pendente("TypeError_throwNew"),
    pendente("Type_equality"),
    pendente("Uri_isWindowsPlatform"),
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
