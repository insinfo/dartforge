//! A ligação no macOS pelo `ld64.lld` direto, sem o driver do Clang, o `ld`
//! da Apple nem as Command Line Tools (ou o Xcode) na máquina de quem usa o
//! dartforge (N16).
//!
//! O driver do Clang só dava à ligação a raiz do SDK do macOS (achada pelo
//! `xcrun`), de onde vêm os *stubs* `.tbd` das bibliotecas do sistema
//! (`libSystem` e os frameworks `CoreFoundation` e `Security`), e as versões
//! de plataforma. Aqui os `.tbd` são **gerados pelo dartforge**
//! ([`BIBLIOTECAS_DO_SISTEMA`], [`gerar`]), como as bibliotecas de importação
//! do Windows (`ligador_windows.rs`): nada do SDK da Apple é copiado nem
//! redistribuído — a licença dele (Xcode and Apple SDKs Agreement) não
//! permite. Um `.tbd` é texto (YAML, formato TAPI v4): o caminho de instalação
//! da biblioteca e os nomes que ela exporta; um nome é um fato de interface.
//! O programa carrega as bibliotecas do sistema ao rodar.
//!
//! Os arquivos gerados moram num **sysroot de ligação** ([`SysrootMacos`]): o
//! da distribuição (`lib/sysroot/<arch>-apple-darwin/`, gerado pelo
//! `dartforge empacotar`) ou, numa árvore de desenvolvimento, o cache nativo,
//! gerado uma vez por conteúdo.

use std::path::{Path, PathBuf};
use std::process::Command;

/// As arquiteturas do macOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arquitetura {
    Arm64,
    X86_64,
}

impl Arquitetura {
    /// A deste hospedeiro.
    pub const fn do_hospedeiro() -> Arquitetura {
        if cfg!(target_arch = "aarch64") { Arquitetura::Arm64 } else { Arquitetura::X86_64 }
    }

    /// O triple do sysroot (`lib/sysroot/<triple>`).
    pub const fn triple_do_sysroot(self) -> &'static str {
        match self {
            Arquitetura::Arm64 => "arm64-apple-darwin",
            Arquitetura::X86_64 => "x86_64-apple-darwin",
        }
    }

    /// O nome no `-arch` do `ld64.lld`.
    const fn nome(self) -> &'static str {
        match self {
            Arquitetura::Arm64 => "arm64",
            Arquitetura::X86_64 => "x86_64",
        }
    }

    /// A versão mínima do macOS: a do `rustc` para o alvo.
    const fn minimo(self) -> &'static str {
        match self {
            Arquitetura::Arm64 => "11.0",
            Arquitetura::X86_64 => "10.12",
        }
    }
}

/// O triple do sysroot deste hospedeiro.
pub fn triple_do_sysroot() -> &'static str {
    Arquitetura::do_hospedeiro().triple_do_sysroot()
}

/// Uma biblioteca do sistema e os nomes que a ligação pede a ela.
pub struct BibliotecaDoSistema {
    /// O `.tbd`, relativo ao sysroot: o nome com que o `ld64.lld` o procura
    /// (`-lSystem` → `usr/lib/libSystem.tbd`; `-framework X` →
    /// `System/Library/Frameworks/X.framework/X.tbd`).
    pub arquivo: &'static str,
    /// O caminho de instalação, gravado no executável (`LC_LOAD_DYLIB`).
    pub install_name: &'static str,
    /// Os nomes (com o `_` do Mach-O) nas duas arquiteturas.
    pub simbolos: &'static [&'static str],
    /// Os que só o arm64 usa.
    pub so_arm64: &'static [&'static str],
    /// Os que só o x86-64 usa: as variantes com sufixo que os cabeçalhos do
    /// SDK escolhem nele (`$INODE64`, `$NOCANCEL`).
    pub so_x86_64: &'static [&'static str],
}

/// As bibliotecas do sistema e os nomes que o runtime (as duas `staticlib`),
/// o SDK e o código gerado usam. A `libSystem` reexporta as de
/// `/usr/lib/system/` (`libsystem_c`, `libsystem_kernel`, `libdyld`,
/// `libunwind`, `libdispatch`, `libcommonCrypto`, `libsystem_m`…): o
/// executável liga só com ela, e o `dyld` acha cada nome pelas reexportações.
///
/// A lista do runtime saiu do `nm` das `staticlib` (`arm64` e `x86_64`, `aot`
/// e `dll`) e do que o `ld64.lld` exigiu ao ligar, no macOS, tudo o que um
/// programa pode puxar delas: o Rust compila um crate em poucos objetos, e
/// puxar um nome traz o objeto inteiro com tudo o que ele cita (daí o
/// `security_framework` quase inteiro). Os testes
/// `runtime_so_usa_simbolos_da_lista` e `simbolos_existem_no_sdk` conferem,
/// no runner do macOS (arm64), que nada falta e que cada nome existe no SDK;
/// as variantes do x86-64 (`so_x86_64`) não têm runner que as confira.
pub const BIBLIOTECAS_DO_SISTEMA: &[BibliotecaDoSistema] = &[
    BibliotecaDoSistema {
        arquivo: "usr/lib/libSystem.tbd",
        install_name: "/usr/lib/libSystem.B.dylib",
        simbolos: &[
            // A partida, o `dyld` e a TLS.
            "__NSGetArgc",
            "__NSGetArgv",
            "__NSGetEnviron",
            "__NSGetExecutablePath",
            "__dyld_get_image_header",
            "__dyld_get_image_name",
            "__dyld_get_image_vmaddr_slide",
            "__dyld_image_count",
            "__tlv_atexit",
            "__tlv_bootstrap",
            "dyld_stub_binder",
            // O desenrolar (pânico do Rust).
            "__Unwind_Backtrace",
            "__Unwind_DeleteException",
            "__Unwind_GetDataRelBase",
            "__Unwind_GetIP",
            "__Unwind_GetIPInfo",
            "__Unwind_GetLanguageSpecificData",
            "__Unwind_GetRegionStart",
            "__Unwind_GetTextRelBase",
            "__Unwind_RaiseException",
            "__Unwind_Resume",
            "__Unwind_SetGR",
            "__Unwind_SetIP",
            // O que o compilador de C e o LLVM chamam sozinhos.
            "___assert_rtn",
            "___chkstk_darwin",
            "___error",
            "___exp10",
            "___exp10f",
            "___sincos_stret",
            "___sincosf_stret",
            "___stack_chk_fail",
            // As variantes do `_FORTIFY_SOURCE` dos cabeçalhos do SDK (o C do
            // zlib e do `ring` compilado no macOS).
            "___memcpy_chk",
            "___memmove_chk",
            "___memset_chk",
            "___snprintf_chk",
            "___sprintf_chk",
            "___strcat_chk",
            "___strcpy_chk",
            "___strncat_chk",
            "___strncpy_chk",
            "___vsnprintf_chk",
            "___vsprintf_chk",
            "___stack_chk_guard",
            "_memset_pattern16",
            // Processo, memória e texto.
            "__exit",
            "_abort",
            "_calloc",
            "_exit",
            "_free",
            "_getenv",
            "_malloc",
            "_memchr",
            "_memcmp",
            "_memcpy",
            "_memmove",
            "_memset",
            "_posix_memalign",
            "_qsort",
            "_realloc",
            "_snprintf",
            "_strchr",
            "_strcmp",
            "_strerror",
            "_strerror_r",
            "_strlen",
            "_strncmp",
            "_strtod",
            "_vsnprintf",
            // Arquivos, processos e terminal.
            "_chdir",
            "_close",
            "_closedir",
            "_dirfd",
            "_dup2",
            "_execvp",
            "_fcntl",
            "_fork",
            "_fsetattrlist",
            "_fsync",
            "_ftruncate",
            "_getcwd",
            "_getpid",
            "_getrusage",
            "_ioctl",
            "_isatty",
            "_kill",
            "_lseek",
            "_mkdir",
            "_open",
            "_pause",
            "_pipe",
            "_poll",
            "_read",
            "_readlink",
            "_readv",
            "_realpath$DARWIN_EXTSN",
            "_rename",
            "_rmdir",
            "_setsid",
            "_signal",
            "_symlink",
            "_tcgetattr",
            "_tcsetattr",
            "_unlink",
            "_wait",
            "_write",
            "_writev",
            // Eventos, soquetes e rede.
            "_accept",
            "_bind",
            "_connect",
            "_freeaddrinfo",
            "_freeifaddrs",
            "_gai_strerror",
            "_getaddrinfo",
            "_gethostname",
            "_getifaddrs",
            "_getnameinfo",
            "_getpeername",
            "_getsockname",
            "_getsockopt",
            "_if_nametoindex",
            "_inet_pton",
            "_kevent",
            "_kqueue",
            "_listen",
            "_recv",
            "_recvfrom",
            "_recvmsg",
            "_send",
            "_sendmsg",
            "_sendto",
            "_setsockopt",
            "_shutdown",
            "_socket",
            // Threads, tempo e o sistema.
            "_CCRandomGenerateBytes",
            "_clock_gettime",
            "_dispatch_release",
            "_dispatch_semaphore_create",
            "_dispatch_semaphore_signal",
            "_dispatch_semaphore_wait",
            "_dispatch_time",
            "_dlclose",
            "_dlerror",
            "_dlopen",
            "_dlsym",
            "_getentropy",
            "_localtime_r",
            "_mach_task_self_",
            "_mmap",
            "_munmap",
            "_nanosleep",
            "_pthread_attr_destroy",
            "_pthread_attr_init",
            "_pthread_attr_setstacksize",
            "_pthread_cond_broadcast",
            "_pthread_cond_destroy",
            "_pthread_cond_signal",
            "_pthread_cond_timedwait_relative_np",
            "_pthread_cond_wait",
            "_pthread_create",
            "_pthread_detach",
            "_pthread_join",
            "_pthread_mutex_destroy",
            "_pthread_mutex_init",
            "_pthread_mutex_lock",
            "_pthread_mutex_trylock",
            "_pthread_mutex_unlock",
            "_pthread_mutexattr_destroy",
            "_pthread_mutexattr_init",
            "_pthread_mutexattr_settype",
            "_pthread_setname_np",
            "_pthread_threadid_np",
            "_sched_yield",
            "_sysconf",
            "_sysctlbyname",
            "_task_info",
            "_tzset",
            // A `libm`: o que o código gerado, o SDK e o runtime chamam (a
            // mesma lista do `ucrtbase` do Windows).
            "_abs",
            "_acos",
            "_acosf",
            "_asin",
            "_asinf",
            "_atan",
            "_atan2",
            "_atan2f",
            "_atanf",
            "_cbrt",
            "_ceil",
            "_ceilf",
            "_copysign",
            "_cos",
            "_cosf",
            "_cosh",
            "_exp",
            "_exp2",
            "_expf",
            "_expm1",
            "_fabs",
            "_floor",
            "_floorf",
            "_fma",
            "_fmax",
            "_fmin",
            "_fmod",
            "_fmodf",
            "_frexp",
            "_hypot",
            "_labs",
            "_ldexp",
            "_llabs",
            "_log",
            "_log10",
            "_log1p",
            "_log2",
            "_logf",
            "_modf",
            "_nearbyint",
            "_pow",
            "_powf",
            "_remainder",
            "_rint",
            "_round",
            "_roundf",
            "_sin",
            "_sinf",
            "_sinh",
            "_sqrt",
            "_sqrtf",
            "_tan",
            "_tanf",
            "_tanh",
            "_trunc",
            "_truncf",
            // O resto do que os objetos puxados da `std` e das dependências
            // pedem (o Rust compila um crate em poucos objetos: puxar um nome
            // traz o objeto inteiro, e o ligador exige tudo o que ele cita).
            "__Unwind_GetCFA",
            "_chmod",
            "_chown",
            "_chroot",
            "_confstr",
            "_copyfile_state_alloc",
            "_copyfile_state_free",
            "_copyfile_state_get",
            "_dup",
            "_fchmod",
            "_fchown",
            "_fclonefileat",
            "_fcopyfile",
            "_fdopendir",
            "_fileno",
            "_flock",
            "_getpeereid",
            "_getppid",
            "_getpwuid_r",
            "_getuid",
            "_killpg",
            "_lchown",
            "_linkat",
            "_mach_error_string",
            "_mach_timebase_info",
            "_mach_wait_until",
            "_mkfifo",
            "_mprotect",
            "_openat",
            "_posix_spawn_file_actions_adddup2",
            "_posix_spawn_file_actions_destroy",
            "_posix_spawn_file_actions_init",
            "_posix_spawnattr_destroy",
            "_posix_spawnattr_init",
            "_posix_spawnattr_setflags",
            "_posix_spawnattr_setpgroup",
            "_posix_spawnattr_setsigdefault",
            "_posix_spawnp",
            "_pread",
            "_pthread_get_stackaddr_np",
            "_pthread_get_stacksize_np",
            "_pthread_self",
            "_pwrite",
            "_setattrlist",
            "_setenv",
            "_setgid",
            "_setgroups",
            "_setpgid",
            "_setuid",
            "_sigaction",
            "_sigaddset",
            "_sigaltstack",
            "_sigemptyset",
            "_socketpair",
            "_strnlen",
            "_unlinkat",
            "_unsetenv",
            "_waitpid",
        ],
        so_arm64: &[
            "_bzero",
            "_fstat",
            "_fstatat",
            "_lstat",
            "_opendir",
            "_pthread_jit_write_protect_np",
            "_readdir_r",
            "_stat",
            "_sys_icache_invalidate",
        ],
        so_x86_64: &[
            "___bzero",
            "_close$NOCANCEL",
            "_fstat$INODE64",
            "_fstatat$INODE64",
            "_lstat$INODE64",
            "_opendir$INODE64",
            "_readdir_r$INODE64",
            "_stat$INODE64",
        ],
    },
    // O `Platform.localeName` (io_plataforma.rs) e o `CFRunLoop` do FSEvents
    // (io_observador.rs; as funções do FSEvents vêm do `CoreServices` pelo
    // `dlsym`).
    BibliotecaDoSistema {
        arquivo: "System/Library/Frameworks/CoreFoundation.framework/CoreFoundation.tbd",
        install_name: "/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation",
        simbolos: &[
            "_CFAbsoluteTimeGetCurrent",
            "_CFArrayCreate",
            "_CFArrayGetCount",
            "_CFArrayGetValueAtIndex",
            "_CFDataGetBytePtr",
            "_CFDataGetLength",
            "_CFDictionaryGetValueIfPresent",
            "_CFEqual",
            "_CFLocaleCopyCurrent",
            "_CFLocaleCopyPreferredLanguages",
            "_CFLocaleGetIdentifier",
            "_CFMachPortCreateRunLoopSource",
            "_CFNumberGetValue",
            "_CFRelease",
            "_CFRetain",
            "_CFRunLoopAddTimer",
            "_CFRunLoopGetCurrent",
            "_CFRunLoopRun",
            "_CFRunLoopStop",
            "_CFRunLoopTimerCreate",
            "_CFRunLoopWakeUp",
            "_CFStringCreateWithBytesNoCopy",
            "_CFStringCreateWithCString",
            "_CFStringGetBytes",
            "_CFStringGetCString",
            "_CFStringGetCStringPtr",
            "_CFStringGetLength",
            "_CFStringGetMaximumSizeForEncoding",
            "_kCFAllocatorDefault",
            "_kCFAllocatorNull",
            "_kCFRunLoopCommonModes",
            "_kCFRunLoopDefaultMode",
            "_kCFTypeArrayCallBacks",
            // O que o `security_framework` e o `core_foundation` citam.
            "_CFCopyDescription",
            "_CFDataCreate",
            "_CFDataGetTypeID",
            "_CFDictionaryAddValue",
            "_CFDictionaryCreate",
            "_CFDictionaryCreateMutable",
            "_CFDictionaryGetCount",
            "_CFDictionaryGetKeysAndValues",
            "_CFDictionarySetValue",
            "_CFGetTypeID",
            "_CFNumberCreate",
            "_CFStringCreateWithBytes",
            "_kCFBooleanFalse",
            "_kCFBooleanTrue",
            "_kCFTypeDictionaryKeyCallBacks",
            "_kCFTypeDictionaryValueCallBacks",
        ],
        so_arm64: &[],
        so_x86_64: &[],
    },
    // O chaveiro do sistema: as raízes da TLS (`tls.rs`, `rustls-native-certs`).
    BibliotecaDoSistema {
        arquivo: "System/Library/Frameworks/Security.framework/Security.tbd",
        install_name: "/System/Library/Frameworks/Security.framework/Versions/A/Security",
        simbolos: &[
            "_SecCertificateCopyData",
            "_SecCopyErrorMessageString",
            "_SecTrustSettingsCopyCertificates",
            "_SecTrustSettingsCopyTrustSettings",
            // O resto do `security_framework` (o ligador exige o objeto inteiro
            // do crate, inclusive o SecureTransport, CMS e Authorization).
            "_AuthorizationCreate",
            "_AuthorizationCreateFromExternalForm",
            "_AuthorizationExecuteWithPrivileges",
            "_AuthorizationMakeExternalForm",
            "_CMSDecoderCopyAllCerts",
            "_CMSDecoderCopyContent",
            "_CMSDecoderCopyDetachedContent",
            "_CMSDecoderCopySignerEmailAddress",
            "_CMSDecoderCopySignerSigningTime",
            "_CMSDecoderCopySignerStatus",
            "_CMSDecoderCopySignerTimestamp",
            "_CMSDecoderCopySignerTimestampCertificates",
            "_CMSDecoderCopySignerTimestampWithPolicy",
            "_CMSDecoderCreate",
            "_CMSDecoderFinalizeMessage",
            "_CMSDecoderGetNumSigners",
            "_CMSDecoderIsContentEncrypted",
            "_CMSDecoderSetDetachedContent",
            "_CMSDecoderUpdateMessage",
            "_CMSEncodeContent",
            "_CMSEncoderAddRecipients",
            "_CMSEncoderAddSignedAttributes",
            "_CMSEncoderAddSigners",
            "_CMSEncoderAddSupportingCerts",
            "_CMSEncoderCopyEncapsulatedContentType",
            "_CMSEncoderCopyEncodedContent",
            "_CMSEncoderCopyRecipients",
            "_CMSEncoderCopySignerTimestamp",
            "_CMSEncoderCopySignerTimestampWithPolicy",
            "_CMSEncoderCopySigners",
            "_CMSEncoderCopySupportingCerts",
            "_CMSEncoderCreate",
            "_CMSEncoderGetCertificateChainMode",
            "_CMSEncoderGetHasDetachedContent",
            "_CMSEncoderSetCertificateChainMode",
            "_CMSEncoderSetEncapsulatedContentTypeOID",
            "_CMSEncoderSetHasDetachedContent",
            "_CMSEncoderSetSignerAlgorithm",
            "_CMSEncoderUpdateContent",
            "_SecAccessControlCreateWithFlags",
            "_SecCertificateAddToKeychain",
            "_SecCertificateCopyCommonName",
            "_SecCertificateCopyEmailAddresses",
            "_SecCertificateCopyKey",
            "_SecCertificateCopyNormalizedIssuerSequence",
            "_SecCertificateCopyNormalizedSubjectSequence",
            "_SecCertificateCopySerialNumberData",
            "_SecCertificateCopySubjectSummary",
            "_SecCertificateCopyValues",
            "_SecCertificateCreateWithData",
            "_SecCertificateGetTypeID",
            "_SecCodeCheckValidity",
            "_SecCodeCopyGuestWithAttributes",
            "_SecCodeCopyPath",
            "_SecCodeCopySelf",
            "_SecDecryptTransformCreate",
            "_SecDigestTransformCreate",
            "_SecEncryptTransformCreate",
            "_SecIdentityCopyCertificate",
            "_SecIdentityCopyPrivateKey",
            "_SecIdentityCreateWithCertificate",
            "_SecIdentityGetTypeID",
            "_SecItemAdd",
            "_SecItemCopyMatching",
            "_SecItemDelete",
            "_SecItemImport",
            "_SecItemUpdate",
            "_SecKeyCopyAttributes",
            "_SecKeyCopyExternalRepresentation",
            "_SecKeyCopyKeyExchangeResult",
            "_SecKeyCopyPublicKey",
            "_SecKeyCreateDecryptedData",
            "_SecKeyCreateEncryptedData",
            "_SecKeyCreateFromData",
            "_SecKeyCreateRandomKey",
            "_SecKeyCreateSignature",
            "_SecKeyGetTypeID",
            "_SecKeyVerifySignature",
            "_SecKeychainAddGenericPassword",
            "_SecKeychainAddInternetPassword",
            "_SecKeychainCopyDefault",
            "_SecKeychainCopyDomainDefault",
            "_SecKeychainFindGenericPassword",
            "_SecKeychainFindInternetPassword",
            "_SecKeychainGetUserInteractionAllowed",
            "_SecKeychainItemFreeContent",
            "_SecKeychainItemModifyAttributesAndData",
            "_SecKeychainSetUserInteractionAllowed",
            "_SecKeychainUnlock",
            "_SecPKCS12Import",
            "_SecPolicyCreateBasicX509",
            "_SecPolicyCreateRevocation",
            "_SecPolicyCreateSSL",
            "_SecRandomCopyBytes",
            "_SecRequirementCreateWithString",
            "_SecStaticCodeCheckValidity",
            "_SecStaticCodeCreateWithPath",
            "_SecTransformExecute",
            "_SecTransformSetAttribute",
            "_SecTrustCopyAnchorCertificates",
            "_SecTrustCopyPublicKey",
            "_SecTrustCreateWithCertificates",
            "_SecTrustEvaluate",
            "_SecTrustEvaluateWithError",
            "_SecTrustGetCertificateAtIndex",
            "_SecTrustGetCertificateCount",
            "_SecTrustGetNetworkFetchAllowed",
            "_SecTrustSetAnchorCertificates",
            "_SecTrustSettingsSetTrustSettings",
            "_kSecAttrAccessControl",
            "_kSecAttrAccessGroup",
            "_kSecAttrAccessibleAfterFirstUnlock",
            "_kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly",
            "_kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly",
            "_kSecAttrAccessibleWhenUnlocked",
            "_kSecAttrAccessibleWhenUnlockedThisDeviceOnly",
            "_kSecAttrAccount",
            "_kSecAttrApplicationLabel",
            "_kSecAttrAuthenticationType",
            "_kSecAttrComment",
            "_kSecAttrDescription",
            "_kSecAttrIsPermanent",
            "_kSecAttrKeySizeInBits",
            "_kSecAttrKeyType",
            "_kSecAttrKeyTypeAES",
            "_kSecAttrKeyTypeEC",
            "_kSecAttrKeyTypeECSECPrimeRandom",
            "_kSecAttrKeyTypeRSA",
            "_kSecAttrLabel",
            "_kSecAttrPath",
            "_kSecAttrPort",
            "_kSecAttrProtocol",
            "_kSecAttrSecurityDomain",
            "_kSecAttrServer",
            "_kSecAttrService",
            "_kSecAttrSynchronizable",
            "_kSecAttrSynchronizableAny",
            "_kSecAttrTokenID",
            "_kSecAttrTokenIDSecureEnclave",
            "_kSecClass",
            "_kSecClassGenericPassword",
            "_kSecClassInternetPassword",
            "_kSecDigestHMACKeyAttribute",
            "_kSecDigestSHA2",
            "_kSecEncryptionMode",
            "_kSecGuestAttributeAudit",
            "_kSecGuestAttributePid",
            "_kSecIVKey",
            "_kSecImportExportAccess",
            "_kSecImportExportKeychain",
            "_kSecImportExportPassphrase",
            "_kSecImportItemCertChain",
            "_kSecImportItemIdentity",
            "_kSecImportItemKeyID",
            "_kSecImportItemLabel",
            "_kSecImportItemTrust",
            "_kSecKeyAlgorithmECDHKeyExchangeCofactor",
            "_kSecKeyAlgorithmECDHKeyExchangeCofactorX963SHA1",
            "_kSecKeyAlgorithmECDHKeyExchangeCofactorX963SHA224",
            "_kSecKeyAlgorithmECDHKeyExchangeCofactorX963SHA256",
            "_kSecKeyAlgorithmECDHKeyExchangeCofactorX963SHA384",
            "_kSecKeyAlgorithmECDHKeyExchangeCofactorX963SHA512",
            "_kSecKeyAlgorithmECDHKeyExchangeStandard",
            "_kSecKeyAlgorithmECDHKeyExchangeStandardX963SHA1",
            "_kSecKeyAlgorithmECDHKeyExchangeStandardX963SHA224",
            "_kSecKeyAlgorithmECDHKeyExchangeStandardX963SHA256",
            "_kSecKeyAlgorithmECDHKeyExchangeStandardX963SHA384",
            "_kSecKeyAlgorithmECDHKeyExchangeStandardX963SHA512",
            "_kSecKeyAlgorithmECDSASignatureDigestX962",
            "_kSecKeyAlgorithmECDSASignatureDigestX962SHA1",
            "_kSecKeyAlgorithmECDSASignatureDigestX962SHA224",
            "_kSecKeyAlgorithmECDSASignatureDigestX962SHA256",
            "_kSecKeyAlgorithmECDSASignatureDigestX962SHA384",
            "_kSecKeyAlgorithmECDSASignatureDigestX962SHA512",
            "_kSecKeyAlgorithmECDSASignatureMessageX962SHA1",
            "_kSecKeyAlgorithmECDSASignatureMessageX962SHA224",
            "_kSecKeyAlgorithmECDSASignatureMessageX962SHA256",
            "_kSecKeyAlgorithmECDSASignatureMessageX962SHA384",
            "_kSecKeyAlgorithmECDSASignatureMessageX962SHA512",
            "_kSecKeyAlgorithmECDSASignatureRFC4754",
            "_kSecKeyAlgorithmECIESEncryptionCofactorVariableIVX963SHA224AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionCofactorVariableIVX963SHA256AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionCofactorVariableIVX963SHA384AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionCofactorVariableIVX963SHA512AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionCofactorX963SHA1AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionCofactorX963SHA224AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionCofactorX963SHA256AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionCofactorX963SHA384AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionCofactorX963SHA512AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionStandardVariableIVX963SHA224AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionStandardVariableIVX963SHA256AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionStandardVariableIVX963SHA384AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionStandardVariableIVX963SHA512AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionStandardX963SHA1AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionStandardX963SHA224AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionStandardX963SHA256AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionStandardX963SHA384AESGCM",
            "_kSecKeyAlgorithmECIESEncryptionStandardX963SHA512AESGCM",
            "_kSecKeyAlgorithmRSAEncryptionOAEPSHA1",
            "_kSecKeyAlgorithmRSAEncryptionOAEPSHA1AESGCM",
            "_kSecKeyAlgorithmRSAEncryptionOAEPSHA224",
            "_kSecKeyAlgorithmRSAEncryptionOAEPSHA224AESGCM",
            "_kSecKeyAlgorithmRSAEncryptionOAEPSHA256",
            "_kSecKeyAlgorithmRSAEncryptionOAEPSHA256AESGCM",
            "_kSecKeyAlgorithmRSAEncryptionOAEPSHA384",
            "_kSecKeyAlgorithmRSAEncryptionOAEPSHA384AESGCM",
            "_kSecKeyAlgorithmRSAEncryptionOAEPSHA512",
            "_kSecKeyAlgorithmRSAEncryptionOAEPSHA512AESGCM",
            "_kSecKeyAlgorithmRSAEncryptionPKCS1",
            "_kSecKeyAlgorithmRSAEncryptionRaw",
            "_kSecKeyAlgorithmRSASignatureDigestPKCS1v15Raw",
            "_kSecKeyAlgorithmRSASignatureDigestPKCS1v15SHA1",
            "_kSecKeyAlgorithmRSASignatureDigestPKCS1v15SHA224",
            "_kSecKeyAlgorithmRSASignatureDigestPKCS1v15SHA256",
            "_kSecKeyAlgorithmRSASignatureDigestPKCS1v15SHA384",
            "_kSecKeyAlgorithmRSASignatureDigestPKCS1v15SHA512",
            "_kSecKeyAlgorithmRSASignatureDigestPSSSHA1",
            "_kSecKeyAlgorithmRSASignatureDigestPSSSHA224",
            "_kSecKeyAlgorithmRSASignatureDigestPSSSHA256",
            "_kSecKeyAlgorithmRSASignatureDigestPSSSHA384",
            "_kSecKeyAlgorithmRSASignatureDigestPSSSHA512",
            "_kSecKeyAlgorithmRSASignatureMessagePKCS1v15SHA1",
            "_kSecKeyAlgorithmRSASignatureMessagePKCS1v15SHA224",
            "_kSecKeyAlgorithmRSASignatureMessagePKCS1v15SHA256",
            "_kSecKeyAlgorithmRSASignatureMessagePKCS1v15SHA384",
            "_kSecKeyAlgorithmRSASignatureMessagePKCS1v15SHA512",
            "_kSecKeyAlgorithmRSASignatureMessagePSSSHA1",
            "_kSecKeyAlgorithmRSASignatureMessagePSSSHA224",
            "_kSecKeyAlgorithmRSASignatureMessagePSSSHA256",
            "_kSecKeyAlgorithmRSASignatureMessagePSSSHA384",
            "_kSecKeyAlgorithmRSASignatureMessagePSSSHA512",
            "_kSecKeyAlgorithmRSASignatureRaw",
            "_kSecKeyKeyExchangeParameterRequestedSize",
            "_kSecKeyKeyExchangeParameterSharedInfo",
            "_kSecPaddingKey",
            "_kSecPrivateKeyAttrs",
            "_kSecPropertyKeyLabel",
            "_kSecPropertyKeyType",
            "_kSecPropertyKeyValue",
            "_kSecPropertyTypeSection",
            "_kSecPropertyTypeString",
            "_kSecPublicKeyAttrs",
            "_kSecReturnData",
            "_kSecTransformInputAttributeName",
            "_kSecUseKeychain",
            "_kSecValueData",
            "_kSecValueRef",
        ],
        so_arm64: &[],
        so_x86_64: &[],
    },
];

/// As bibliotecas do sistema, no formato do `ld64.lld`.
const BIBLIOTECAS: &[&str] = &["-lSystem", "-framework", "CoreFoundation", "-framework", "Security"];

/// O arquivo com a versão do SDK (o `-platform_version`) dos sysroots que
/// copiavam os `.tbd` do SDK; nos gerados ele não existe, e a versão é a
/// mínima.
const ARQUIVO_VERSAO: &str = "versao-do-sdk";

/// O `.tbd` (TAPI v4) de `b` para `arch`.
///
/// ```
/// use dartforge_emit_native::ligador_macos::{Arquitetura, BIBLIOTECAS_DO_SISTEMA, tbd};
/// let texto = tbd(&BIBLIOTECAS_DO_SISTEMA[0], Arquitetura::Arm64);
/// assert!(texto.starts_with("--- !tapi-tbd\n"));
/// assert!(texto.contains("install-name:    '/usr/lib/libSystem.B.dylib'"));
/// assert!(texto.contains("'_malloc'"));
/// ```
pub fn tbd(b: &BibliotecaDoSistema, arch: Arquitetura) -> String {
    let alvo = format!("{}-macos", arch.nome());
    let proprios = match arch {
        Arquitetura::Arm64 => b.so_arm64,
        Arquitetura::X86_64 => b.so_x86_64,
    };
    let mut nomes: Vec<&str> = b.simbolos.iter().chain(proprios).copied().collect();
    nomes.sort_unstable();
    let lista: Vec<String> = nomes.iter().map(|n| format!("'{n}'")).collect();
    format!(
        "--- !tapi-tbd\ntbd-version:     4\ntargets:         [ {alvo} ]\ninstall-name:    '{}'\nexports:\n  - targets:     [ {alvo} ]\n    symbols:     [ {} ]\n...\n",
        b.install_name,
        lista.join(",\n                   ")
    )
}

/// Gera em `dir` os `.tbd` de [`BIBLIOTECAS_DO_SISTEMA`] para `arch`.
///
/// # Erros
///
/// Falha de escrita no diretório.
pub fn gerar(dir: &Path, arch: Arquitetura) -> Result<(), String> {
    for b in BIBLIOTECAS_DO_SISTEMA {
        let caminho = dir.join(b.arquivo);
        if let Some(p) = caminho.parent() {
            std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        std::fs::write(&caminho, tbd(b, arch)).map_err(|e| format!("{}: {e}", caminho.display()))?;
    }
    Ok(())
}

/// A impressão digital do conteúdo gerado, para o diretório no cache.
fn impressao(arch: Arquitetura) -> String {
    let mut h = blake3::Hasher::new();
    for b in BIBLIOTECAS_DO_SISTEMA {
        h.update(b.arquivo.as_bytes());
        h.update(tbd(b, arch).as_bytes());
    }
    h.finalize().to_hex()[..16].to_string()
}

/// O sysroot de ligação com os `.tbd`, e a versão do SDK (vazia: a mínima).
#[derive(Debug, Clone)]
pub struct SysrootMacos {
    raiz: PathBuf,
    versao: String,
}

impl SysrootMacos {
    /// O da distribuição, senão o do cache nativo, gerado agora se faltar.
    ///
    /// # Erros
    ///
    /// A geração falhou (ver [`gerar`]).
    pub fn localizar() -> Result<&'static SysrootMacos, String> {
        static S: std::sync::OnceLock<Result<SysrootMacos, String>> = std::sync::OnceLock::new();
        S.get_or_init(|| {
            let arch = Arquitetura::do_hospedeiro();
            if let Some(dir) = dartforge_elements::distribuicao::em_lib(&format!("sysroot/{}", arch.triple_do_sysroot()))
                && dir.join(BIBLIOTECAS_DO_SISTEMA[0].arquivo).is_file()
            {
                let versao = std::fs::read_to_string(dir.join(ARQUIVO_VERSAO)).unwrap_or_default().trim().to_string();
                return Ok(SysrootMacos { raiz: dir, versao });
            }
            let raiz = crate::cache::dir_cache_nativo().join("ligacao-macos");
            let chave = format!("{}-{}", arch.nome(), impressao(arch));
            let dir = raiz.join(&chave);
            if dir.join("pronto").is_file() {
                return Ok(SysrootMacos { raiz: dir, versao: String::new() });
            }
            let tmp = raiz.join(format!("{chave}.tmp.{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&tmp);
            gerar(&tmp, arch)?;
            std::fs::write(tmp.join("pronto"), b"").map_err(|e| e.to_string())?;
            if std::fs::rename(&tmp, &dir).is_err() {
                // O destino existe. Com `pronto`, outro processo venceu a
                // corrida: vale o dele. Sem `pronto`, é resto de uma geração
                // interrompida ou de uma versão anterior sem a marca (o CI
                // restaura o `target/` de um cache): sai, e a nossa entra.
                if !dir.join("pronto").is_file() {
                    let _ = std::fs::remove_dir_all(&dir);
                    if std::fs::rename(&tmp, &dir).is_err() && !dir.join("pronto").is_file() {
                        let _ = std::fs::remove_dir_all(&tmp);
                        return Err(format!("não foi possível instalar {}", dir.display()));
                    }
                }
                let _ = std::fs::remove_dir_all(&tmp);
            }
            Ok(SysrootMacos { raiz: dir, versao: String::new() })
        })
        .as_ref()
        .map_err(Clone::clone)
    }
}

/// O que se liga.
pub enum Produto<'a> {
    /// Um executável.
    Executavel,
    /// Uma biblioteca dinâmica com o `install_name` dado, exportando (e
    /// trazendo das `staticlib`) os símbolos dados.
    Dinamica { install_name: &'a str, exportados: &'a [String] },
}

/// Uma ligação pelo `ld64.lld`.
pub struct Ligacao<'a> {
    pub produto: Produto<'a>,
    /// Objetos e bibliotecas do programa, na ordem.
    pub entradas: Vec<PathBuf>,
    /// Procurar as bibliotecas dinâmicas ao lado do executável.
    pub rpath_executavel: bool,
    /// LTO dos bitcodes de entrada (produção).
    pub lto: bool,
    /// A CPU-alvo da geração de código da LTO (`--cpu`, o nome do LLVM);
    /// `None`, a base.
    pub cpu: Option<&'static str>,
    /// Tirar o que nada alcança (`-dead_strip`) e os símbolos locais (produção).
    pub podar: bool,
    /// Manter o mapa de depuração mesmo podando (J05).
    pub manter_depuracao: bool,
    pub saida: &'a Path,
}

/// O `ld64.lld`: o da distribuição, o ao lado do Clang, ou o do `PATH`.
pub fn ld64_lld(clang: &Path) -> PathBuf {
    if let Some(l) = dartforge_elements::distribuicao::ferramenta_llvm("ld64.lld") {
        return l;
    }
    let ao_lado = clang.with_file_name("ld64.lld");
    if ao_lado.is_file() { ao_lado } else { PathBuf::from("ld64.lld") }
}

/// Liga com o `ld64.lld` e os `.tbd` do sysroot.
///
/// # Erros
///
/// O `ld64.lld` não executou ou recusou a ligação (a mensagem leva o que ele
/// disse).
pub fn ligar(ld: &Path, sysroot: &SysrootMacos, l: &Ligacao<'_>) -> Result<(), String> {
    let arch = Arquitetura::do_hospedeiro();
    let minimo =
        std::env::var("MACOSX_DEPLOYMENT_TARGET").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| arch.minimo().to_string());
    let versao_sdk = if sysroot.versao.is_empty() { minimo.clone() } else { sysroot.versao.clone() };
    let mut cmd = Command::new(ld);
    cmd.args(["-arch", arch.nome(), "-platform_version", "macos", &minimo, &versao_sdk]);
    cmd.arg("-syslibroot").arg(&sysroot.raiz);
    match &l.produto {
        Produto::Executavel => {
            cmd.args(["-execute", "-dynamic"]);
        }
        Produto::Dinamica { install_name, exportados } => {
            cmd.args(["-dylib", "-install_name", install_name]);
            let mut rsp = String::new();
            for n in exportados.iter() {
                rsp.push_str(&format!("-u _{n}\n"));
            }
            let arquivo = l.saida.with_extension("rsp");
            std::fs::write(&arquivo, rsp).map_err(|e| format!("{}: {e}", arquivo.display()))?;
            cmd.arg(format!("@{}", arquivo.display()));
        }
    }
    cmd.arg("-o").arg(l.saida);
    if l.rpath_executavel {
        cmd.args(["-rpath", "@executable_path"]);
    }
    if l.lto {
        cmd.arg(format!("--lto-O{}", crate::driver::nivel_da_lto()));
        cmd.arg(format!("--thinlto-jobs={}", crate::driver::tarefas_de_geracao()));
        if let Some(d) = crate::driver::cache_do_thinlto() {
            cmd.arg("-cache_path_lto").arg(d);
        }
        if let Some(c) = l.cpu {
            cmd.arg("-mllvm").arg(format!("-mcpu={c}"));
        }
    }
    if l.podar {
        cmd.arg("-dead_strip");
        // Sem `--icf=safe` (que o `ld64.lld` tem, como o `ld.lld` e o
        // `lld-link`): não foi verificado num Mac que ele trate como tomado
        // todo endereço de um objeto sem `__llvm_addrsig` (os do Rust), e
        // o `==` de tear-offs depende disso (`ligador_windows.rs`).
        // `-S` tira o mapa de depuração (J05); `-x`, os símbolos locais.
        if !crate::ligador::manter_simbolos() && !l.manter_depuracao {
            cmd.args(["-S", "-x"]);
        }
    }
    if l.podar
        && let Some(m) = crate::ligador::mapa_da_ligacao(l.saida)
    {
        cmd.arg("-map").arg(m);
    }
    cmd.args(&l.entradas);
    cmd.args(BIBLIOTECAS);
    let saida = cmd.output().map_err(|e| format!("falha ao executar {}: {e}", ld.display()))?;
    if !saida.status.success() {
        let texto = String::from_utf8_lossy(&saida.stderr);
        // Os erros (e as linhas `>>>` que os explicam) antes dos avisos: com
        // muitos objetos, os avisos de versão mínima escondiam o erro.
        let (erros, avisos): (Vec<&str>, Vec<&str>) =
            texto.lines().filter(|l| !l.trim().is_empty()).partition(|l| !l.contains("warning:"));
        let n_avisos = 40usize.saturating_sub(erros.len());
        let linhas: Vec<&str> = erros.into_iter().chain(avisos.into_iter().take(n_avisos)).collect();
        return Err(format!("o ld64.lld falhou na ligação ({}):\n{}", saida.status, linhas.join("\n")));
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nenhum_simbolo_repetido() {
        for arch in [Arquitetura::Arm64, Arquitetura::X86_64] {
            let mut todos: Vec<&str> = Vec::new();
            for b in BIBLIOTECAS_DO_SISTEMA {
                todos.extend(b.simbolos);
                todos.extend(if arch == Arquitetura::Arm64 { b.so_arm64 } else { b.so_x86_64 });
            }
            let total = todos.len();
            todos.sort_unstable();
            todos.dedup();
            assert_eq!(todos.len(), total, "{arch:?}");
        }
    }

    #[test]
    fn tbd_por_arquitetura() {
        let arm = tbd(&BIBLIOTECAS_DO_SISTEMA[0], Arquitetura::Arm64);
        let x86 = tbd(&BIBLIOTECAS_DO_SISTEMA[0], Arquitetura::X86_64);
        assert!(arm.contains("targets:         [ arm64-macos ]") && arm.contains("'_stat'") && !arm.contains("INODE64"));
        assert!(x86.contains("targets:         [ x86_64-macos ]") && x86.contains("'_stat$INODE64'") && !x86.contains("'_stat'"));
        assert!(arm.ends_with("...\n"));
    }

    #[test]
    fn gerar_escreve_cada_biblioteca() {
        let dir = tempfile::tempdir().unwrap();
        gerar(dir.path(), Arquitetura::Arm64).unwrap();
        for b in BIBLIOTECAS_DO_SISTEMA {
            let texto = std::fs::read_to_string(dir.path().join(b.arquivo)).unwrap();
            assert!(texto.contains(b.install_name));
        }
    }

    /// Os `.tbd` (relativos à raiz do SDK) das bibliotecas citadas num `.tbd`
    /// que não são documentos dele mesmo (o `ld64.lld` procura uma
    /// reexportação pelo caminho de instalação dentro do `-syslibroot`).
    #[cfg(target_os = "macos")]
    fn reexportacoes_externas(texto: &str) -> Vec<String> {
        let proprios: Vec<&str> = texto
            .lines()
            .filter_map(|l| l.trim().strip_prefix("install-name:"))
            .map(|v| v.trim().trim_matches(|c| c == '\'' || c == '"'))
            .collect();
        let mut saida = Vec::new();
        for pedaco in texto.split(['\'', '"', ' ', ',', '[', ']', '\n']) {
            let caminho = pedaco.trim();
            if !(caminho.starts_with("/usr/lib/") || caminho.starts_with("/System/Library/")) || proprios.contains(&caminho) {
                continue;
            }
            let rel = caminho.trim_start_matches('/');
            let tbd = match rel.strip_suffix(".dylib") {
                Some(base) => format!("{base}.tbd"),
                None => format!("{rel}.tbd"),
            };
            if !saida.contains(&tbd) {
                saida.push(tbd);
            }
        }
        saida
    }

    /// Os nomes que um `.tbd` do SDK e os que ele reexporta de outros
    /// arquivos citam (todas as palavras: basta para conferir existência).
    #[cfg(target_os = "macos")]
    fn nomes_no_sdk(raiz: &Path, arquivo: &str) -> std::collections::HashSet<String> {
        let mut nomes = std::collections::HashSet::new();
        let mut pendentes = vec![arquivo.to_string()];
        let mut vistos = std::collections::HashSet::new();
        while let Some(rel) = pendentes.pop() {
            if !vistos.insert(rel.clone()) {
                continue;
            }
            let Ok(texto) = std::fs::read_to_string(raiz.join(&rel)) else { continue };
            nomes.extend(texto.split(['\'', '"', ' ', ',', '[', ']', '\n']).map(|p| p.trim().to_string()));
            pendentes.extend(reexportacoes_externas(&texto));
        }
        nomes
    }

    /// Cada nome gerado existe no `.tbd` do SDK da mesma biblioteca (o
    /// runner do macOS, com o Xcode ou as Command Line Tools).
    #[cfg(target_os = "macos")]
    #[test]
    fn simbolos_existem_no_sdk() {
        let raiz = crate::alvo::raiz_do_sdk_macos().expect("SDK do macOS (xcrun)");
        let mut faltam = Vec::new();
        for b in BIBLIOTECAS_DO_SISTEMA {
            let nomes = nomes_no_sdk(raiz, b.arquivo);
            assert!(nomes.contains(b.install_name), "{}: install-name {} fora do SDK", b.arquivo, b.install_name);
            let proprios = if Arquitetura::do_hospedeiro() == Arquitetura::Arm64 { b.so_arm64 } else { b.so_x86_64 };
            for n in b.simbolos.iter().chain(proprios) {
                if !nomes.contains(*n) {
                    faltam.push(format!("{}: {n}", b.arquivo));
                }
            }
        }
        assert!(faltam.is_empty(), "não exportados pelo SDK: {faltam:#?}");
    }

    /// As duas `staticlib` do runtime ligam só com os `.tbd` gerados, com
    /// tudo o que um programa pode puxar delas: cada nome que o código gerado
    /// chama (`dartforge_runtime::simbolos::NOMES`, como a biblioteca do SDK
    /// da fonte em `sdk_modulo.rs`) é exigido com `-u`. A `aot` (com o `main`)
    /// vira executável e a `dll`, biblioteca dinâmica; o `dartforge_entry` do
    /// código gerado vem de um objeto C. Um nome fora de
    /// [`BIBLIOTECAS_DO_SISTEMA`] faz o `ld64.lld` recusar a ligação, e a
    /// mensagem dele lista todos (`--error-limit=0`). (O `nm` da Apple não lê
    /// o bitcode do LLVM do dartforge que vai nas `staticlib`; e o
    /// `-all_load` exigiria até o que nenhum programa liga, como o
    /// SecureTransport do `security_framework`.)
    #[cfg(target_os = "macos")]
    #[test]
    fn runtime_so_usa_simbolos_da_lista() {
        let clang = crate::driver::NativeDriverOptions::default().clang;
        let dir = tempfile::tempdir().unwrap();
        gerar(&dir.path().join("sysroot"), Arquitetura::do_hospedeiro()).unwrap();
        let sysroot = SysrootMacos { raiz: dir.path().join("sysroot"), versao: String::new() };
        let c = dir.path().join("entrada.c");
        std::fs::write(&c, "void dartforge_entry(void) {}\n").unwrap();
        let obj = dir.path().join("entrada.o");
        let status = Command::new(&clang).arg("-c").arg(&c).arg("-o").arg(&obj).status().expect("executar o Clang");
        assert!(status.success(), "o Clang não compilou {}", c.display());
        let exe = dir.path().join("programa");
        let dylib = dir.path().join("libteste.dylib");
        let casos = [
            (crate::cache::RuntimeCache::get_or_compile(), Produto::Executavel, &exe),
            (
                crate::cache::RuntimeCache::para_dll(),
                Produto::Dinamica { install_name: "@rpath/libteste.dylib", exportados: &[] },
                &dylib,
            ),
        ];
        for (lib, produto, saida) in casos {
            let lib = lib.expect("runtime pré-compilado").lib_path;
            let mut entradas = vec![PathBuf::from("--error-limit=0"), obj.clone()];
            for n in dartforge_runtime::simbolos::NOMES.iter().filter(|n| **n != "main") {
                entradas.push(PathBuf::from("-u"));
                entradas.push(PathBuf::from(format!("_{n}")));
            }
            entradas.push(lib);
            let l = Ligacao { produto, entradas, rpath_executavel: false, lto: false, cpu: None, podar: false, manter_depuracao: false, saida };
            if let Err(e) = ligar(&ld64_lld(&clang), &sysroot, &l) {
                panic!("o runtime usa nomes fora de BIBLIOTECAS_DO_SISTEMA:\n{e}");
            }
        }
        // O executável roda (só o `dartforge_entry` vazio).
        let saida = Command::new(&exe).output().expect("executar o programa");
        assert!(saida.status.code().is_some(), "o programa terminou por sinal: {saida:?}");
    }
}
