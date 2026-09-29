//! A ligação no Windows pelo `lld-link` direto, sem o Visual Studio (MSVC)
//! nem o Windows SDK na máquina de quem usa o dartforge (N15).
//!
//! O que o driver do Clang, o `link.exe` e as bibliotecas do MSVC davam à
//! ligação vem daqui, escrito por nós (nada do MSVC nem do Windows SDK é
//! copiado nem redistribuído — as licenças deles não permitem):
//!
//! * **bibliotecas de importação** ([`IMPORTACOES`]): arquivos `ar` com
//!   membros COFF *short import* (`IMPORT_OBJECT_HEADER`, especificação PE/COFF
//!   §8), gerados de listas com só os nomes que o runtime, o SDK e o código
//!   gerado usam — `kernel32`, `ntdll`, `ws2_32`, `advapi32`, `userenv`,
//!   `iphlpapi`, `crypt32`, `bcrypt` e a CRT universal (`ucrtbase.dll`, parte
//!   do Windows 10 e posteriores). Um nome é um fato de interface; não há
//!   código de ninguém nelas. O `lld-link` monta a tabela de importação
//!   sozinho a partir desses membros;
//! * **a CRT mínima** ([`CRT_COMUM_IR`], [`CRT_EXE_IR`], [`CRT_DLL_IR`]): o
//!   que a CRT estática (`msvcrt.lib`/`vcruntime.lib`) punha no executável e
//!   o `ucrtbase.dll` não exporta — o diretório de TLS (`_tls_used`,
//!   `_tls_index`, os marcadores `.tls` e `.CRT$XL*` entre os quais o Rust
//!   registra as suas *callbacks*), os inicializadores `.CRT$XI*`/`.CRT$XC*`,
//!   as entradas `mainCRTStartup`/`_DllMainCRTStartup`, o cookie do `/GS`
//!   (`__security_cookie`, `__security_check_cookie`, `__GSHandlerCheck`,
//!   que o `cl.exe` usa no C do `ring` e do zlib), `__chkstk`, `_fltused`,
//!   `atexit` (sobre o `_crt_atexit` do `ucrtbase`) e a vtable de
//!   `type_info` que o descritor de tipo do pânico do Rust referencia. É LLVM
//!   IR compilado pelo mesmo gerador do programa.
//!
//! O tratamento de exceção estruturado (SEH) e o C++ EH do pânico do Rust
//! continuam os do sistema: `__C_specific_handler`, `__CxxFrameHandler3` e
//! `_CxxThrowException` vêm do `ucrtbase.dll`. O executável deixa de depender
//! do `vcruntime140.dll` (que não faz parte do Windows).
//!
//! Os arquivos gerados moram num diretório ([`SysrootWindows`]): o da
//! distribuição (`lib/sysroot/x86_64-pc-windows-msvc/`, gerado pelo
//! `dartforge empacotar`) ou, numa árvore de desenvolvimento, o cache nativo,
//! gerado uma vez por conteúdo.

use std::path::{Path, PathBuf};
use std::process::Command;

/// O triple dos objetos (o mesmo cabeçalho do IR do programa, `alvo.rs`).
pub const TRIPLE: &str = "x86_64-pc-windows-msvc";

/// As funções importadas, por DLL. Só nomes exportados pelo sistema desde o
/// Windows 10 (o teste `importacoes_existem_no_sistema` confere cada um nas
/// DLLs de `System32` do runner do Windows).
pub const IMPORTACOES: &[(&str, &[&str])] = &[
    (
        "kernel32.dll",
        &[
            "AddVectoredExceptionHandler",
            "CancelIo",
            "CancelIoEx",
            "CloseHandle",
            "CompareStringOrdinal",
            "CopyFileExW",
            "CreateDirectoryW",
            "CreateEventW",
            "CreateFileW",
            "CreateHardLinkW",
            "CreateIoCompletionPort",
            "CreateMutexA",
            "CreateNamedPipeW",
            "CreatePipe",
            "CreateProcessW",
            "CreateSymbolicLinkW",
            "CreateThread",
            "CreateWaitableTimerExW",
            "DeleteCriticalSection",
            "DeleteFileW",
            "DeleteProcThreadAttributeList",
            "DeviceIoControl",
            "DuplicateHandle",
            "EnterCriticalSection",
            "ExitProcess",
            "FindClose",
            "FindFirstFileExW",
            "FindNextFileW",
            "FlsAlloc",
            "FlsFree",
            "FlsGetValue",
            "FlsSetValue",
            "FlushFileBuffers",
            "FlushInstructionCache",
            "FormatMessageW",
            "FreeEnvironmentStringsW",
            "FreeLibrary",
            "GetCommandLineW",
            "GetConsoleCP",
            "GetConsoleMode",
            "GetConsoleOutputCP",
            "GetConsoleScreenBufferInfo",
            "GetCurrentDirectoryW",
            "GetCurrentProcess",
            "GetCurrentProcessId",
            "GetCurrentThread",
            "GetCurrentThreadId",
            "GetEnvironmentStringsW",
            "GetEnvironmentVariableW",
            "GetExitCodeProcess",
            "GetFileAttributesW",
            "GetFileInformationByHandle",
            "GetFileInformationByHandleEx",
            "GetFileSizeEx",
            "GetFileType",
            "GetFinalPathNameByHandleW",
            "GetFullPathNameW",
            "GetLastError",
            "GetModuleFileNameW",
            "GetModuleHandleA",
            "GetModuleHandleExW",
            "GetModuleHandleW",
            "GetOverlappedResult",
            "GetProcAddress",
            "GetProcessHeap",
            "GetProcessId",
            "GetQueuedCompletionStatus",
            "GetStdHandle",
            "GetSystemDirectoryW",
            "GetSystemInfo",
            "GetSystemTimeAsFileTime",
            "GetSystemTimePreciseAsFileTime",
            "GetTempPathW",
            "GetTimeZoneInformation",
            "GetUserDefaultLocaleName",
            "GetWindowsDirectoryW",
            "HeapAlloc",
            "HeapFree",
            "HeapReAlloc",
            "InitializeCriticalSection",
            "InitializeProcThreadAttributeList",
            "IsThreadAFiber",
            "K32EnumProcessModules",
            "K32GetProcessMemoryInfo",
            "LeaveCriticalSection",
            "LoadLibraryA",
            "LoadLibraryExW",
            "LoadLibraryW",
            "LockFileEx",
            "MoveFileExW",
            "MultiByteToWideChar",
            "OpenProcess",
            "PostQueuedCompletionStatus",
            "QueryPerformanceCounter",
            "QueryPerformanceFrequency",
            "ReadConsoleW",
            "ReadDirectoryChangesW",
            "ReadFile",
            "ReadFileEx",
            "RegisterWaitForSingleObject",
            "ReleaseMutex",
            "RemoveDirectoryW",
            "RtlCaptureContext",
            "RtlLookupFunctionEntry",
            "RtlVirtualUnwind",
            "SetConsoleCtrlHandler",
            "SetConsoleMode",
            "SetCurrentDirectoryW",
            "SetEnvironmentVariableW",
            "SetFileAttributesW",
            "SetFileInformationByHandle",
            "SetFilePointerEx",
            "SetFileTime",
            "SetHandleInformation",
            "SetLastError",
            "SetThreadStackGuarantee",
            "SetUnhandledExceptionFilter",
            "SetWaitableTimer",
            "Sleep",
            "SleepEx",
            "SwitchToThread",
            "TerminateProcess",
            "TlsAlloc",
            "TlsFree",
            "TlsGetValue",
            "TlsSetValue",
            "UnlockFile",
            "UnlockFileEx",
            "UnregisterWait",
            "UpdateProcThreadAttribute",
            "VirtualAlloc",
            "VirtualFree",
            "VirtualProtect",
            "VirtualQuery",
            "WaitForMultipleObjects",
            "WaitForSingleObject",
            "WaitForSingleObjectEx",
            "WideCharToMultiByte",
            "WriteConsoleW",
            "WriteFile",
            "WriteFileEx",
            "lstrlenW",
        ],
    ),
    (
        "ntdll.dll",
        &[
            "NtCreateFile",
            "NtCreateNamedPipeFile",
            "NtOpenFile",
            "NtQueryInformationFile",
            "NtReadFile",
            "NtWriteFile",
            "RtlNtStatusToDosError",
        ],
    ),
    (
        "ws2_32.dll",
        &[
            "FreeAddrInfoW",
            "GetAddrInfoW",
            "GetHostNameW",
            "GetNameInfoW",
            "InetPtonW",
            "WSACleanup",
            "WSADuplicateSocketW",
            "WSAGetLastError",
            "WSAIoctl",
            "WSAPoll",
            "WSARecv",
            "WSARecvFrom",
            "WSASend",
            "WSASendTo",
            "WSASetLastError",
            "WSASocketW",
            "WSAStartup",
            "accept",
            "bind",
            "closesocket",
            "connect",
            "freeaddrinfo",
            "getaddrinfo",
            "gethostname",
            "getpeername",
            "getsockname",
            "getsockopt",
            "ioctlsocket",
            "listen",
            "recv",
            "recvfrom",
            "select",
            "send",
            "sendto",
            "setsockopt",
            "shutdown",
        ],
    ),
    ("iphlpapi.dll", &["GetAdaptersAddresses", "if_indextoname", "if_nametoindex"]),
    (
        "advapi32.dll",
        &["GetTokenInformation", "OpenProcessToken", "RegCloseKey", "RegGetValueW", "RegOpenKeyExW", "RegQueryValueExW", "SystemFunction036"],
    ),
    ("userenv.dll", &["GetUserProfileDirectoryW"]),
    ("bcrypt.dll", &["BCryptGenRandom"]),
    (
        "crypt32.dll",
        &[
            "CertCloseStore",
            "CertDuplicateCertificateContext",
            "CertEnumCertificatesInStore",
            "CertFreeCertificateContext",
            "CertOpenStore",
            "CertOpenSystemStoreW",
        ],
    ),
    (
        "ucrtbase.dll",
        &[
            // O tratamento de exceção (SEH do C, C++ EH do pânico do Rust).
            "_CxxThrowException",
            "__CxxFrameHandler3",
            "__C_specific_handler",
            // A partida e a saída do processo (a CRT mínima).
            "_set_app_type",
            "_crt_atexit",
            "_initialize_onexit_table",
            "_register_onexit_function",
            "_execute_onexit_table",
            "_cexit",
            "_exit",
            "exit",
            "abort",
            // Memória e texto.
            "calloc",
            "free",
            "malloc",
            "realloc",
            "memchr",
            "memcmp",
            "memcpy",
            "memmove",
            "memset",
            "strchr",
            "strcmp",
            "strerror",
            "strlen",
            "strncmp",
            "wcslen",
            "wcstombs",
            "qsort",
            "getenv",
            "_errno",
            // O zlib (`gz*`) e o `ring`, compilados com o `cl.exe`.
            "__stdio_common_vsprintf",
            "_close",
            "_lseeki64",
            "_open",
            "_read",
            "_wopen",
            "_write",
            // A `libm`: o que o código gerado, o SDK e o runtime chamam.
            "abs",
            "labs",
            "llabs",
            "acos",
            "acosf",
            "asin",
            "asinf",
            "atan",
            "atan2",
            "atan2f",
            "atanf",
            "cbrt",
            "ceil",
            "ceilf",
            "copysign",
            "cos",
            "cosf",
            "cosh",
            "exp",
            "exp2",
            "expf",
            "expm1",
            "fabs",
            "floor",
            "floorf",
            "fma",
            "fmax",
            "fmin",
            "fmod",
            "fmodf",
            "frexp",
            "hypot",
            "ldexp",
            "log",
            "log10",
            "log1p",
            "log2",
            "logf",
            "modf",
            "nearbyint",
            "pow",
            "powf",
            "remainder",
            "rint",
            "round",
            "roundf",
            "sin",
            "sinf",
            "sinh",
            "sqrt",
            "sqrtf",
            "strtod",
            "tan",
            "tanf",
            "tanh",
            "trunc",
            "truncf",
        ],
    ),
];

/// O cabeçalho dos três módulos da CRT mínima.
const CABECALHO: &str = "target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\ntarget triple = \"x86_64-pc-windows-msvc\"\n\n";

/// A parte da CRT mínima comum ao executável e à DLL.
///
/// Os marcadores de seção são `constant` (só leitura) como os da CRT da
/// Microsoft: o `lld-link` agrupa as contribuições pelo nome **e** pelas
/// características, e um marcador gravável cairia noutra seção de saída,
/// longe das entradas do Rust (`.CRT$XLB`, `.CRT$XCU`). A leitura das tabelas
/// é `volatile` e o módulo é gerado sem otimização: o laço anda de um
/// marcador ao outro, fora dos limites de qualquer objeto do IR.
pub const CRT_COMUM_IR: &str = r#"
@_tls_index = global i32 0, align 4
@_tls_start = global i8 0, section ".tls", align 8
@_tls_end = global i8 0, section ".tls$ZZZ", align 8
@__xl_a = constant ptr null, section ".CRT$XLA", align 8
@__xl_z = constant ptr null, section ".CRT$XLZ", align 8
@__xi_a = constant ptr null, section ".CRT$XIA", align 8
@__xi_z = constant ptr null, section ".CRT$XIZ", align 8
@__xc_a = constant ptr null, section ".CRT$XCA", align 8
@__xc_z = constant ptr null, section ".CRT$XCZ", align 8

; IMAGE_TLS_DIRECTORY64: início e fim do modelo, o índice, as callbacks
; (depois do marcador `__xl_a`), o preenchimento com zeros e as
; características (o lld-link acerta o alinhamento).
@_tls_used = constant { ptr, ptr, ptr, ptr, i32, i32 } { ptr @_tls_start, ptr @_tls_end, ptr @_tls_index, ptr getelementptr (i8, ptr @__xl_a, i64 8), i32 0, i32 0 }, section ".rdata$T", align 8

; Referenciado por todo objeto que usa ponto flutuante.
@_fltused = global i32 39029, align 4

; A vtable de `type_info`: o descritor de tipo do pânico do Rust aponta para
; ela; o `__CxxFrameHandler3` casa os tipos pelo nome, nunca a chama.
@"??_7type_info@@6B@" = constant [2 x ptr] zeroinitializer, align 8

; O cookie do /GS (valor inicial o da Microsoft, sorteado na partida).
@__security_cookie = global i64 47936899621426, align 8
@__security_cookie_complement = global i64 -47936899621427, align 8

declare dllimport i32 @QueryPerformanceCounter(ptr)
declare dllimport i32 @GetCurrentProcessId()
declare dllimport i32 @GetCurrentThreadId()
declare dllimport void @GetSystemTimeAsFileTime(ptr)

define void @__security_init_cookie() {
entrada:
  %contador = alloca i64, align 8
  %relogio = alloca i64, align 8
  store i64 0, ptr %contador, align 8
  store i64 0, ptr %relogio, align 8
  %0 = call i32 @QueryPerformanceCounter(ptr %contador)
  call void @GetSystemTimeAsFileTime(ptr %relogio)
  %c = load i64, ptr %contador, align 8
  %r = load i64, ptr %relogio, align 8
  %pid = call i32 @GetCurrentProcessId()
  %tid = call i32 @GetCurrentThreadId()
  %pid64 = zext i32 %pid to i64
  %tid64 = zext i32 %tid to i64
  %pidd = shl i64 %pid64, 32
  %end = ptrtoint ptr %contador to i64
  %a = xor i64 %c, %r
  %b = xor i64 %a, %pidd
  %d = xor i64 %b, %tid64
  %e = xor i64 %d, %end
  %f = and i64 %e, 281474976710655
  %padrao = icmp eq i64 %f, 47936899621426
  %zero = icmp eq i64 %f, 0
  %ruim = or i1 %padrao, %zero
  %g = select i1 %ruim, i64 47936899621427, i64 %f
  store i64 %g, ptr @__security_cookie, align 8
  %h = xor i64 %g, -1
  store i64 %h, ptr @__security_cookie_complement, align 8
  ret void
}

define internal void @df_initterm(ptr %a, ptr %z) {
entrada:
  %p0 = getelementptr i8, ptr %a, i64 8
  br label %teste
teste:
  %p = phi ptr [ %p0, %entrada ], [ %prox, %seguir ]
  %fim = icmp uge ptr %p, %z
  br i1 %fim, label %sair, label %corpo
corpo:
  %f = load volatile ptr, ptr %p, align 8
  %nulo = icmp eq ptr %f, null
  br i1 %nulo, label %seguir, label %chamar
chamar:
  call void %f()
  br label %seguir
seguir:
  %prox = getelementptr i8, ptr %p, i64 8
  br label %teste
sair:
  ret void
}

; O cookie, os inicializadores C (`.CRT$XI*`) e C++ (`.CRT$XC*`, onde o Rust
; põe os dele).
define void @df_crt_iniciar() {
  call void @__security_init_cookie()
  call void @df_initterm(ptr @__xi_a, ptr @__xi_z)
  call void @df_initterm(ptr @__xc_a, ptr @__xc_z)
  ret void
}

; Folhas sem mexer no `rsp` (sem tabela de desenrolamento).
;
; `__chkstk`: toca cada página de 4 KiB entre o `rsp` de quem chamou e
; `rsp - rax`, de cima para baixo (a página de guarda cresce uma por vez).
; Preserva todos os registradores menos as bandeiras, como o `chkstk.asm` da
; Microsoft: o código do `cl.exe` (e o do LLVM em `alloca` dinâmico) pode
; ter valores vivos em r10 e r11 através da chamada.
module asm ".text"
module asm ".globl __chkstk"
module asm ".def __chkstk; .scl 2; .type 32; .endef"
module asm "__chkstk:"
module asm "  subq $16, %rsp"
module asm "  movq %r10, (%rsp)"
module asm "  movq %r11, 8(%rsp)"
module asm "  leaq 24(%rsp), %r11"
module asm "  movq %r11, %r10"
module asm "  subq %rax, %r10"
module asm "  jb .Ldf_chkstk_fim"
module asm ".Ldf_chkstk_laco:"
module asm "  subq $4096, %r11"
module asm "  cmpq %r10, %r11"
module asm "  jb .Ldf_chkstk_fim"
module asm "  testq %rax, (%r11)"
module asm "  jmp .Ldf_chkstk_laco"
module asm ".Ldf_chkstk_fim:"
module asm "  movq (%rsp), %r10"
module asm "  movq 8(%rsp), %r11"
module asm "  addq $16, %rsp"
module asm "  retq"
; `__security_check_cookie(rcx)`: preserva tudo menos rcx (o valor de
; retorno de quem chama está em rax); cookie errado encerra o processo
; (`__fastfail(FAST_FAIL_STACK_COOKIE_CHECK_FAILURE)`).
module asm ".globl __security_check_cookie"
module asm ".def __security_check_cookie; .scl 2; .type 32; .endef"
module asm "__security_check_cookie:"
module asm "  cmpq __security_cookie(%rip), %rcx"
module asm "  jne .Ldf_cookie_falhou"
module asm "  retq"
module asm ".Ldf_cookie_falhou:"
module asm "  movl $2, %ecx"
module asm "  int $0x29"
; `__report_rangecheckfailure`: `__fastfail(FAST_FAIL_RANGE_CHECK_FAILURE)`.
module asm ".globl __report_rangecheckfailure"
module asm ".def __report_rangecheckfailure; .scl 2; .type 32; .endef"
module asm "__report_rangecheckfailure:"
module asm "  movl $8, %ecx"
module asm "  int $0x29"
; `__GSHandlerCheck`: o tratador de linguagem das funções com cookie no
; desenrolamento; aqui só segue a busca (ExceptionContinueSearch).
module asm ".globl __GSHandlerCheck"
module asm ".def __GSHandlerCheck; .scl 2; .type 32; .endef"
module asm "__GSHandlerCheck:"
module asm "  movl $1, %eax"
module asm "  retq"
; `__GSHandlerCheck_SEH`: função com cookie e `__try`: o tratador do SEH.
module asm ".globl __GSHandlerCheck_SEH"
module asm ".def __GSHandlerCheck_SEH; .scl 2; .type 32; .endef"
module asm "__GSHandlerCheck_SEH:"
module asm "  jmpq *__imp___C_specific_handler(%rip)"
"#;

/// A entrada do executável: prepara a CRT, chama o `main` (o do runtime, ou o
/// que o emissor escreve com o SDK da fonte) e sai pelo `exit` do
/// `ucrtbase`, que roda os `atexit` e esvazia os fluxos do C.
pub const CRT_EXE_IR: &str = r#"
declare void @df_crt_iniciar()
declare i32 @main()
declare dllimport void @_set_app_type(i32)
declare dllimport void @exit(i32) noreturn
declare dllimport i32 @_crt_atexit(ptr)

; `atexit` do C no executável: a tabela global do ucrtbase, que o `exit`
; percorre.
define i32 @atexit(ptr %f) {
  %r = call i32 @_crt_atexit(ptr %f)
  ret i32 %r
}

define i32 @mainCRTStartup() {
  call void @_set_app_type(i32 1)
  call void @df_crt_iniciar()
  %r = call i32 @main()
  call void @exit(i32 %r)
  unreachable
}
"#;

/// A entrada da DLL (a do SDK da fonte): na carga, o cookie e os
/// inicializadores; na saída do processo (ou na descarga), os `atexit` dela.
pub const CRT_DLL_IR: &str = r#"
declare void @df_crt_iniciar()
declare dllimport i32 @_initialize_onexit_table(ptr)
declare dllimport i32 @_register_onexit_function(ptr, ptr)
declare dllimport i32 @_execute_onexit_table(ptr)

; A tabela de `atexit` desta DLL (`_onexit_table_t`: início, fim, limite),
; percorrida quando ela sai do processo — como a CRT da Microsoft faz numa
; DLL, e não no `exit` (a DLL pode ser descarregada antes).
@df_tabela_de_saida = internal global [3 x ptr] zeroinitializer, align 8

define i32 @atexit(ptr %f) {
  %r = call i32 @_register_onexit_function(ptr @df_tabela_de_saida, ptr %f)
  ret i32 %r
}

define i32 @_DllMainCRTStartup(ptr %h, i32 %motivo, ptr %reservado) {
entrada:
  %anexar = icmp eq i32 %motivo, 1
  br i1 %anexar, label %iniciar, label %talvez_sair
iniciar:
  %0 = call i32 @_initialize_onexit_table(ptr @df_tabela_de_saida)
  call void @df_crt_iniciar()
  br label %fim
talvez_sair:
  %sair = icmp eq i32 %motivo, 0
  br i1 %sair, label %desanexar, label %fim
desanexar:
  %1 = call i32 @_execute_onexit_table(ptr @df_tabela_de_saida)
  br label %fim
fim:
  ret i32 1
}
"#;

/// Os módulos da CRT mínima: (arquivo do objeto, IR sem cabeçalho).
const MODULOS_DA_CRT: &[(&str, &str)] =
    &[("dfcrt_comum.obj", CRT_COMUM_IR), ("dfcrt_exe.obj", CRT_EXE_IR), ("dfcrt_dll.obj", CRT_DLL_IR)];

/// O IR completo (com o cabeçalho do alvo) de um módulo da CRT mínima.
pub fn ir_da_crt(corpo: &str) -> String {
    format!("{CABECALHO}{corpo}")
}

/// O nome do arquivo da biblioteca de importação de `dll` (`kernel32.lib`).
pub fn nome_da_importacao(dll: &str) -> String {
    format!("{}.lib", dll.trim_end_matches(".dll"))
}

/// Um membro *short import* (`IMPORT_OBJECT_HEADER` e os dois nomes) para a
/// função `nome` de `dll`, importada pelo nome, em x86-64.
fn membro_de_importacao(dll: &str, nome: &str) -> Vec<u8> {
    let mut m = Vec::with_capacity(20 + nome.len() + dll.len() + 2);
    m.extend_from_slice(&0u16.to_le_bytes()); // Sig1: IMAGE_FILE_MACHINE_UNKNOWN
    m.extend_from_slice(&0xFFFFu16.to_le_bytes()); // Sig2
    m.extend_from_slice(&0u16.to_le_bytes()); // Version
    m.extend_from_slice(&0x8664u16.to_le_bytes()); // Machine: AMD64
    m.extend_from_slice(&0u32.to_le_bytes()); // TimeDateStamp (determinístico)
    m.extend_from_slice(&((nome.len() + 1 + dll.len() + 1) as u32).to_le_bytes()); // SizeOfData
    m.extend_from_slice(&0u16.to_le_bytes()); // Hint
    // Type = IMPORT_OBJECT_CODE (0), NameType = IMPORT_OBJECT_NAME (1) << 2.
    m.extend_from_slice(&(1u16 << 2).to_le_bytes());
    m.extend_from_slice(nome.as_bytes());
    m.push(0);
    m.extend_from_slice(dll.as_bytes());
    m.push(0);
    m
}

/// O cabeçalho `ar` de um membro (60 bytes).
fn cabecalho_ar(nome: &str, tamanho: usize) -> Vec<u8> {
    let h = format!("{nome:<16}{:<12}{:<6}{:<6}{:<8}{tamanho:<10}`\n", 0, 0, 0, 644);
    debug_assert_eq!(h.len(), 60);
    h.into_bytes()
}

/// A biblioteca de importação de `dll` com as funções `nomes`: um arquivo
/// `ar` (formato GNU, o do `llvm-dlltool`) com a tabela de símbolos
/// (`nome` e `__imp_nome` de cada função) e um membro *short import* por
/// função.
///
/// ```
/// use dartforge_emit_native::ligador_windows::biblioteca_de_importacao;
/// let lib = biblioteca_de_importacao("kernel32.dll", &["ExitProcess"]);
/// assert!(lib.starts_with(b"!<arch>\n/"));
/// assert!(lib.windows(18).any(|j| j == b"__imp_ExitProcess\0"));
/// ```
pub fn biblioteca_de_importacao(dll: &str, nomes: &[&str]) -> Vec<u8> {
    let membros: Vec<Vec<u8>> = nomes.iter().map(|n| membro_de_importacao(dll, n)).collect();
    let mut simbolos: Vec<(String, usize)> = Vec::new();
    for (i, n) in nomes.iter().enumerate() {
        simbolos.push((format!("__imp_{n}"), i));
        simbolos.push(((*n).to_string(), i));
    }
    // A tabela de símbolos: contagem, deslocamentos (big-endian) e nomes.
    let nomes_tabela: usize = simbolos.iter().map(|(s, _)| s.len() + 1).sum();
    let tamanho_tabela = 4 + 4 * simbolos.len() + nomes_tabela;
    let preenchido = |t: usize| t + (t & 1);
    // O nome do membro: o da DLL (até 15 bytes, o campo tem 16 com a `/`).
    let nome_membro: String = format!("{}/", &dll[..dll.len().min(15)]);
    let mut deslocamentos = Vec::with_capacity(membros.len());
    let mut pos = 8 + 60 + preenchido(tamanho_tabela);
    for m in &membros {
        deslocamentos.push(pos);
        pos += 60 + preenchido(m.len());
    }
    let mut saida = Vec::with_capacity(pos);
    saida.extend_from_slice(b"!<arch>\n");
    saida.extend(cabecalho_ar("/", tamanho_tabela));
    saida.extend_from_slice(&(simbolos.len() as u32).to_be_bytes());
    for (_, i) in &simbolos {
        saida.extend_from_slice(&(deslocamentos[*i] as u32).to_be_bytes());
    }
    for (s, _) in &simbolos {
        saida.extend_from_slice(s.as_bytes());
        saida.push(0);
    }
    if tamanho_tabela & 1 == 1 {
        saida.push(b'\n');
    }
    for m in &membros {
        saida.extend(cabecalho_ar(&nome_membro, m.len()));
        saida.extend_from_slice(m);
        if m.len() & 1 == 1 {
            saida.push(b'\n');
        }
    }
    debug_assert_eq!(saida.len(), pos);
    saida
}

/// Quem compila a CRT mínima: o gerador do programa, ou um Clang com
/// `--target` (o `dartforge empacotar` e os testes fora do Windows).
pub enum CompiladorDaCrt<'a> {
    Gerador(&'a crate::gerador::Gerador),
    ClangComAlvo(&'a Path),
}

/// Gera em `dir` as bibliotecas de importação e os objetos da CRT mínima.
///
/// # Erros
///
/// Falha de escrita no diretório ou do compilador do IR.
pub fn gerar(dir: &Path, compilador: &CompiladorDaCrt<'_>) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (dll, nomes) in IMPORTACOES {
        let caminho = dir.join(nome_da_importacao(dll));
        std::fs::write(&caminho, biblioteca_de_importacao(dll, nomes)).map_err(|e| format!("{}: {e}", caminho.display()))?;
    }
    for (arquivo, corpo) in MODULOS_DA_CRT {
        let ir = ir_da_crt(corpo);
        let obj = dir.join(arquivo);
        match compilador {
            CompiladorDaCrt::Gerador(g) => g.gerar(&ir, crate::gerador::Geracao::do_programa(false, false), &obj)?,
            CompiladorDaCrt::ClangComAlvo(clang) => {
                let ll = obj.with_extension("ll");
                std::fs::write(&ll, &ir).map_err(|e| format!("{}: {e}", ll.display()))?;
                let saida = Command::new(clang)
                    .args(["-x", "ir", "-c", "-O0", "-mno-incremental-linker-compatible"])
                    .arg(format!("--target={TRIPLE}"))
                    .arg(&ll)
                    .arg("-o")
                    .arg(&obj)
                    .output()
                    .map_err(|e| format!("falha ao executar {}: {e}", clang.display()))?;
                let _ = std::fs::remove_file(&ll);
                if !saida.status.success() {
                    return Err(format!("a CRT mínima não compilou: {}", String::from_utf8_lossy(&saida.stderr)));
                }
            }
        }
    }
    Ok(())
}

/// A impressão digital do conteúdo gerado (listas e IR), para o diretório no
/// cache.
fn impressao() -> String {
    let mut h = blake3::Hasher::new();
    for (dll, nomes) in IMPORTACOES {
        h.update(dll.as_bytes());
        for n in *nomes {
            h.update(n.as_bytes());
            h.update(&[0]);
        }
    }
    for (arquivo, corpo) in MODULOS_DA_CRT {
        h.update(arquivo.as_bytes());
        h.update(CABECALHO.as_bytes());
        h.update(corpo.as_bytes());
    }
    h.finalize().to_hex()[..16].to_string()
}

/// O diretório com as bibliotecas de importação e a CRT mínima.
#[derive(Debug, Clone)]
pub struct SysrootWindows {
    dir: PathBuf,
}

impl SysrootWindows {
    /// O da distribuição (`lib/sysroot/x86_64-pc-windows-msvc/`), senão o do
    /// cache nativo, gerado agora se faltar.
    ///
    /// # Erros
    ///
    /// A geração falhou (ver [`gerar`]).
    pub fn localizar(clang: &Path) -> Result<&'static SysrootWindows, String> {
        static S: std::sync::OnceLock<Result<SysrootWindows, String>> = std::sync::OnceLock::new();
        S.get_or_init(|| {
            if let Some(dir) = dartforge_elements::distribuicao::em_lib(&format!("sysroot/{TRIPLE}"))
                && dir.join("dfcrt_comum.obj").is_file()
            {
                return Ok(SysrootWindows { dir });
            }
            let gerador = crate::gerador::Gerador::escolher(clang);
            let chave = format!("{}-{}", impressao(), &blake3::hash(gerador.identidade()?.as_bytes()).to_hex()[..8]);
            let raiz = crate::cache::dir_cache_nativo().join("ligacao-windows");
            let dir = raiz.join(&chave);
            if dir.join("pronto").is_file() {
                return Ok(SysrootWindows { dir });
            }
            let tmp = raiz.join(format!("{chave}.tmp.{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&tmp);
            gerar(&tmp, &CompiladorDaCrt::Gerador(&gerador))?;
            std::fs::write(tmp.join("pronto"), b"").map_err(|e| e.to_string())?;
            if std::fs::rename(&tmp, &dir).is_err() {
                let _ = std::fs::remove_dir_all(&tmp);
                if !dir.join("pronto").is_file() {
                    return Err(format!("não foi possível instalar {}", dir.display()));
                }
            }
            Ok(SysrootWindows { dir })
        })
        .as_ref()
        .map_err(Clone::clone)
    }

    /// Um diretório já gerado (testes).
    pub fn do_diretorio(dir: PathBuf) -> SysrootWindows {
        SysrootWindows { dir }
    }

    fn bibliotecas(&self) -> Vec<PathBuf> {
        IMPORTACOES.iter().map(|(dll, _)| self.dir.join(nome_da_importacao(dll))).collect()
    }
}

/// O que se liga.
pub enum Produto<'a> {
    /// Um executável de console.
    Executavel,
    /// Uma DLL com as exportações do `.def` dado; a biblioteca de importação
    /// sai ao lado (`<saída>.lib`).
    Dll { def: &'a Path },
}

/// Uma ligação pelo `lld-link`.
pub struct Ligacao<'a> {
    pub produto: Produto<'a>,
    /// Objetos e bibliotecas do programa, na ordem.
    pub entradas: Vec<PathBuf>,
    /// LTO dos bitcodes de entrada (produção), em partições.
    pub lto: bool,
    /// Tirar as seções que nada alcança (`/OPT:REF`, produção).
    pub podar: bool,
    /// O PDB com as tabelas CodeView dos objetos (J05, `/DEBUG`).
    pub depuracao: bool,
    pub saida: &'a Path,
}

/// O `lld-link`: o da distribuição, o ao lado do Clang, ou o do `PATH`.
pub fn lld_link(clang: &Path) -> PathBuf {
    if let Some(l) = dartforge_elements::distribuicao::ferramenta_llvm("lld-link") {
        return l;
    }
    let ao_lado = clang.with_file_name(if cfg!(windows) { "lld-link.exe" } else { "lld-link" });
    if ao_lado.is_file() { ao_lado } else { PathBuf::from("lld-link") }
}

/// Os argumentos do `lld-link` para a ligação `l`.
pub fn argumentos(sysroot: &SysrootWindows, l: &Ligacao<'_>) -> Vec<std::ffi::OsString> {
    let mut a: Vec<std::ffi::OsString> = Vec::new();
    // Nada do ambiente (`LIB`, o Visual Studio detectado) nem das diretivas
    // `/DEFAULTLIB` dos objetos (`msvcrt.lib`, `oldnames.lib`, as do Rust):
    // tudo o que entra está na linha de comando.
    for s in ["/nologo", "/lldignoreenv", "/nodefaultlib", "/machine:x64", "/subsystem:console", "/include:_tls_used"] {
        a.push(s.into());
    }
    // Sem ICF em nenhum perfil: o tear-off de função de topo se compara pelo
    // endereço (`left == right`, corpus/js 147 e 184).
    a.push("/opt:noicf".into());
    let crt_comum = sysroot.dir.join("dfcrt_comum.obj");
    match &l.produto {
        Produto::Executavel => {
            a.push("/entry:mainCRTStartup".into());
            a.push(crt_comum.into_os_string());
            a.push(sysroot.dir.join("dfcrt_exe.obj").into_os_string());
        }
        Produto::Dll { def } => {
            a.push("/dll".into());
            a.push("/entry:_DllMainCRTStartup".into());
            let mut d = std::ffi::OsString::from("/def:");
            d.push(def.as_os_str());
            a.push(d);
            a.push(crt_comum.into_os_string());
            a.push(sysroot.dir.join("dfcrt_dll.obj").into_os_string());
        }
    }
    if l.podar {
        a.push("/opt:ref".into());
    }
    if l.lto {
        let particoes = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 16);
        a.push("/opt:lldlto=2".into());
        a.push(format!("/opt:lldltopartitions={particoes}").into());
    }
    if l.depuracao {
        a.push("/debug".into());
    }
    let mut o = std::ffi::OsString::from("/out:");
    o.push(l.saida.as_os_str());
    a.push(o);
    a.extend(l.entradas.iter().map(|e| e.clone().into_os_string()));
    a.extend(sysroot.bibliotecas().into_iter().map(PathBuf::into_os_string));
    a
}

/// Liga com o `lld-link`, as bibliotecas de importação e a CRT mínima.
///
/// # Erros
///
/// O `lld-link` não executou ou recusou a ligação (a mensagem leva o que ele
/// disse).
pub fn ligar(lld: &Path, sysroot: &SysrootWindows, l: &Ligacao<'_>) -> Result<(), String> {
    let saida = Command::new(lld)
        .args(argumentos(sysroot, l))
        .output()
        .map_err(|e| format!("falha ao executar {}: {e}", lld.display()))?;
    if !saida.status.success() {
        let mut texto = String::from_utf8_lossy(&saida.stderr).into_owned();
        texto.push_str(&String::from_utf8_lossy(&saida.stdout));
        let linhas: Vec<&str> = texto.lines().filter(|l| !l.trim().is_empty()).take(40).collect();
        return Err(format!("o lld-link falhou na ligação ({}):\n{}", saida.status, linhas.join("\n")));
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn membro_short_import() {
        let m = membro_de_importacao("k.dll", "F");
        assert_eq!(&m[..4], &[0, 0, 0xFF, 0xFF]);
        assert_eq!(u16::from_le_bytes([m[6], m[7]]), 0x8664);
        assert_eq!(u32::from_le_bytes([m[12], m[13], m[14], m[15]]), 8);
        assert_eq!(&m[20..], b"F\0k.dll\0");
    }

    #[test]
    fn arquivo_com_tabela_de_simbolos() {
        let lib = biblioteca_de_importacao("abc.dll", &["Um", "Dois"]);
        assert_eq!(&lib[..8], b"!<arch>\n");
        let tam: usize = std::str::from_utf8(&lib[8 + 48..8 + 58]).unwrap().trim().parse().unwrap();
        let tabela = &lib[68..68 + tam];
        assert_eq!(u32::from_be_bytes(tabela[..4].try_into().unwrap()), 4);
        // Cada deslocamento aponta um cabeçalho de membro com o nome da DLL.
        for i in 0..4 {
            let d = u32::from_be_bytes(tabela[4 + 4 * i..8 + 4 * i].try_into().unwrap()) as usize;
            assert_eq!(&lib[d..d + 8], b"abc.dll/");
            assert_eq!(&lib[d + 60..d + 64], &[0, 0, 0xFF, 0xFF]);
        }
    }

    #[test]
    fn nenhuma_importacao_repetida() {
        let mut todos: Vec<&str> = IMPORTACOES.iter().flat_map(|(_, n)| n.iter().copied()).collect();
        let total = todos.len();
        todos.sort_unstable();
        todos.dedup();
        assert_eq!(todos.len(), total);
    }

    /// Os nomes exportados por uma DLL (a tabela de exportação do PE).
    #[cfg(windows)]
    fn exportados(dll: &Path) -> Vec<String> {
        let b = std::fs::read(dll).unwrap();
        let u16_ = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]) as usize;
        let u32_ = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
        let pe = u32_(0x3c);
        let n_secoes = u16_(pe + 6);
        let opcional = pe + 24;
        let secoes = opcional + u16_(pe + 20);
        // PE32+: o diretório de exportação é o primeiro do opcional (+112).
        let rva_exp = u32_(opcional + 112);
        let para_arquivo = |rva: usize| -> usize {
            for i in 0..n_secoes {
                let s = secoes + 40 * i;
                let (va, tam, bruto) = (u32_(s + 12), u32_(s + 8).max(u32_(s + 16)), u32_(s + 20));
                if rva >= va && rva < va + tam {
                    return rva - va + bruto;
                }
            }
            panic!("RVA fora das seções")
        };
        let exp = para_arquivo(rva_exp);
        let n_nomes = u32_(exp + 24);
        let nomes = para_arquivo(u32_(exp + 32));
        (0..n_nomes)
            .map(|i| {
                let o = para_arquivo(u32_(nomes + 4 * i));
                let fim = b[o..].iter().position(|&c| c == 0).unwrap();
                String::from_utf8_lossy(&b[o..o + fim]).into_owned()
            })
            .collect()
    }

    /// Cada nome importado existe na DLL do sistema (o runner do Windows).
    #[cfg(windows)]
    #[test]
    fn importacoes_existem_no_sistema() {
        let sistema = PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into())).join("System32");
        let mut faltam = Vec::new();
        for (dll, nomes) in IMPORTACOES {
            let exp = exportados(&sistema.join(dll));
            for n in *nomes {
                if !exp.iter().any(|e| e == n) {
                    faltam.push(format!("{dll}!{n}"));
                }
            }
        }
        assert!(faltam.is_empty(), "não exportados pelo sistema: {faltam:?}");
    }

    /// Liga programas C para Windows (compilados pelo Clang com `--target`)
    /// só com o `lld-link`, as bibliotecas de importação e a CRT mínima — em
    /// qualquer sistema com o Clang e o `lld-link`: um executável (TLS,
    /// inicializador `.CRT$XCU`, `__chkstk`, a `libm`, `atexit`, código de
    /// saída) e uma DLL com `.def` (TLS e `atexit` na descarga) usada por
    /// outro executável. No Windows, ou com o Wine no `PATH`, também executa.
    #[test]
    #[ignore = "exige o Clang e o lld-link (DARTFORGE_CLANG ou PATH)"]
    fn liga_e_executa_programa_c() {
        let clang = crate::driver::NativeDriverOptions::default().clang;
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let sys = d.join("sysroot");
        gerar(&sys, &CompiladorDaCrt::ClangComAlvo(&clang)).unwrap();
        let sysroot = SysrootWindows::do_diretorio(sys);
        let comum = r#"
__declspec(dllimport) void* __stdcall GetStdHandle(unsigned);
__declspec(dllimport) int __stdcall WriteFile(void*, const void*, unsigned, unsigned*, void*);
double pow(double, double);
int atexit(void (*)(void));
static void escrever(const char* s) { unsigned n = 0, t = 0; while (s[t]) t++; WriteFile(GetStdHandle((unsigned)-11), s, t, &n, 0); }
"#;
        let compilar = |nome: &str, fonte: &str| -> PathBuf {
            let c = d.join(format!("{nome}.c"));
            std::fs::write(&c, format!("{comum}{fonte}")).unwrap();
            let obj = d.join(format!("{nome}.obj"));
            let st = Command::new(&clang)
                .args([&format!("--target={TRIPLE}"), "-c", "-O1", "-fms-extensions"])
                .arg(&c)
                .arg("-o")
                .arg(&obj)
                .status()
                .unwrap();
            assert!(st.success(), "o Clang não compilou {nome}.c");
            obj
        };
        let p = compilar(
            "p",
            r#"
static __declspec(thread) int por_thread = 41;
static int iniciado = 0;
static void iniciar(void) { iniciado = 1; }
#pragma section(".CRT$XCU", read)
__declspec(allocate(".CRT$XCU")) void (*entrada_xcu)(void) = iniciar;
static void no_fim(void) { escrever("fim\n"); }
/* O `__chkstk` preserva r10 e r11 (o `cl.exe` conta com isso). */
static int chkstk_preserva(void) {
  long long r10, r11;
  __asm__ volatile("movq $0x1234, %%r10\n\tmovq $0x5678, %%r11\n\tmovq $20000, %%rax\n\tcallq __chkstk\n\tmovq %%r10, %0\n\tmovq %%r11, %1"
                   : "=r"(r10), "=r"(r11) : : "rax", "r10", "r11", "memory", "cc");
  return r10 == 0x1234 && r11 == 0x5678;
}
int main(void) {
  volatile char grande[20000]; grande[0] = 1; grande[19999] = 2;
  por_thread++;
  atexit(no_fim);
  if (iniciado && por_thread == 42 && pow(2.0, 10.0) == 1024.0 && grande[0] + grande[19999] == 3 && chkstk_preserva()) escrever("ok\n");
  return 3;
}
"#,
        );
        let exe = d.join("p.exe");
        let ligar_exe = |entradas: Vec<PathBuf>, saida: &Path| {
            ligar(
                &lld_link(&clang),
                &sysroot,
                &Ligacao { produto: Produto::Executavel, entradas, lto: false, podar: true, depuracao: true, saida },
            )
            .unwrap();
        };
        ligar_exe(vec![p], &exe);
        let dll_obj = compilar(
            "b",
            r#"
static __declspec(thread) int t = 5;
static void no_fim(void) { escrever("dll fim\n"); }
int df_valor(void) { static int registrado; if (!registrado) { registrado = 1; atexit(no_fim); } return ++t; }
"#,
        );
        let def = d.join("b.def");
        std::fs::write(&def, "LIBRARY b.dll\nEXPORTS\n  df_valor\n").unwrap();
        ligar(
            &lld_link(&clang),
            &sysroot,
            &Ligacao { produto: Produto::Dll { def: &def }, entradas: vec![dll_obj], lto: false, podar: false, depuracao: false, saida: &d.join("b.dll") },
        )
        .unwrap();
        assert!(d.join("b.lib").is_file(), "a biblioteca de importação da DLL");
        let q = compilar("q", "int df_valor(void);\nint main(void) { if (df_valor() == 6) escrever(\"dll ok\\n\"); return 0; }\n");
        let exe_dll = d.join("q.exe");
        ligar_exe(vec![q, d.join("b.lib")], &exe_dll);
        let wine = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).map(|p| p.join("wine")).find(|w| w.is_file());
        if !cfg!(windows) && wine.is_none() {
            return;
        }
        let executar = |exe: &Path| {
            let mut cmd = match &wine {
                Some(w) if !cfg!(windows) => {
                    let mut c = Command::new(w);
                    c.arg(exe);
                    c
                }
                _ => Command::new(exe),
            };
            let s = cmd.current_dir(d).env("WINEDEBUG", "-all").output().unwrap();
            (String::from_utf8_lossy(&s.stdout).replace('\r', ""), s.status.code())
        };
        assert_eq!(executar(&exe), ("ok\nfim\n".to_string(), Some(3)));
        assert_eq!(executar(&exe_dll), ("dll ok\ndll fim\n".to_string(), Some(0)));
    }
}
