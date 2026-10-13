// Runtime nativo: a TLS do `dart:io` — `SecureSocket`, `RawSecureSocket`,
// `SecureServerSocket`, `SecurityContext` e `X509Certificate` —, sobre o
// rustls (provedor de criptografia `ring`).
//
// A VM implementa o filtro em C++ sobre o BoringSSL
// (`runtime/bin/secure_socket_filter.cc`): quatro anéis de bytes que o Dart
// de `secure_socket.dart` enche e esvazia (texto a ler, texto a escrever,
// cifrado lido, cifrado a escrever), e o `ProcessAllBuffers`, que move os
// bytes entre os anéis e a máquina de TLS. Aqui é o mesmo protocolo, com os
// anéis nas `Uint8List` do heap e a máquina de TLS do rustls. O Dart do
// `secure_socket.dart` do SDK fica intacto; a sobreposição troca só o patch
// da VM (`native_sdk/io/secure_socket_patch.dart`) e o pedido
// `sslProcessFilter` do IOService, atendido na thread do isolado
// (`native_sdk/io/io_service_patch.dart`): o filtro só faz criptografia em
// memória, nunca E/S — quem lê e escreve no soquete continua sendo o
// `RawSocket`, pelo laço de eventos.
//
// A verificação do certificado do servidor segue o avaliador da VM: um
// certificado que as raízes do contexto não validam não derruba o handshake
// na hora; o handshake para (`SSL_ERROR_WANT_CERTIFICATE_VERIFY`, 16), o
// Dart chama o `onBadCertificate` e responde (`_decidir`). Enquanto a
// decisão não vem, nada do que o rustls produziu depois do certificado sai
// para o soquete.
//
// Os erros seguem o texto da VM: `HandshakeException: Handshake error in
// client (OS Error: \n\tCERTIFICATE_VERIFY_FAILED: unable to get local issuer
// certificate(handshake.cc:392))`.

/// A versão mínima padrão do `SecurityContext` (TLS 1.2, como a VM).
const TLS_DEFAULT_MIN_VERSION: i64 = 0x0303;

/// `SSL_ERROR_WANT_CERTIFICATE_VERIFY`: o handshake espera a decisão do Dart.
const TLS_AWAIT_VERIFICATION: i64 = 16;

/// Um `SecurityContext`: as raízes, a cadeia e a chave, o ALPN e a versão
/// mínima. Fica no campo nativo do objeto como `Arc<Mutex<_>>`.
struct TlsContext {
    trusted: Vec<rustls::pki_types::CertificateDer<'static>>,
    builtin_roots: bool,
    chain: Vec<rustls::pki_types::CertificateDer<'static>>,
    key: Option<rustls::pki_types::PrivateKeyDer<'static>>,
    authorities: Vec<rustls::pki_types::CertificateDer<'static>>,
    client_alpn: Vec<Vec<u8>>,
    server_alpn: Vec<Vec<u8>>,
    min_version: i64,
    /// As raízes montadas (as confiáveis e, se pedido, as do sistema); refeitas
    /// quando o contexto muda.
    roots: Option<std::sync::Arc<rustls::RootCertStore>>,
}

type SharedContext = std::sync::Arc<std::sync::Mutex<TlsContext>>;

fn tls_provider() -> std::sync::Arc<rustls::crypto::CryptoProvider> {
    static P: std::sync::OnceLock<std::sync::Arc<rustls::crypto::CryptoProvider>> = std::sync::OnceLock::new();
    P.get_or_init(|| std::sync::Arc::new(rustls::crypto::ring::default_provider())).clone()
}

/// As raízes do sistema (`SecurityContext.defaultContext`,
/// `withTrustedRoots: true`): o repositório de certificados do sistema
/// (arquivos do Linux, chaveiro do macOS, repositório do Windows); sem
/// nenhuma, as da Mozilla embutidas.
fn system_roots() -> &'static [rustls::pki_types::CertificateDer<'static>] {
    static R: std::sync::OnceLock<Vec<rustls::pki_types::CertificateDer<'static>>> = std::sync::OnceLock::new();
    R.get_or_init(|| rustls_native_certs::load_native_certs().certs)
}

impl TlsContext {
    fn new() -> Self {
        TlsContext {
            trusted: Vec::new(),
            builtin_roots: false,
            chain: Vec::new(),
            key: None,
            authorities: Vec::new(),
            client_alpn: Vec::new(),
            server_alpn: Vec::new(),
            min_version: TLS_DEFAULT_MIN_VERSION,
            roots: None,
        }
    }

    fn roots(&mut self) -> std::sync::Arc<rustls::RootCertStore> {
        if let Some(r) = &self.roots {
            return r.clone();
        }
        let mut cert_store = rustls::RootCertStore::empty();
        cert_store.add_parsable_certificates(self.trusted.iter().cloned());
        if self.builtin_roots {
            let system = system_roots();
            if system.is_empty() {
                cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            } else {
                cert_store.add_parsable_certificates(system.iter().cloned());
            }
        }
        let r = std::sync::Arc::new(cert_store);
        self.roots = Some(r.clone());
        r
    }

    fn versions(&self) -> &'static [&'static rustls::SupportedProtocolVersion] {
        static SO_TLS13: [&rustls::SupportedProtocolVersion; 1] = [&rustls::version::TLS13];
        if self.min_version >= 0x0304 { &SO_TLS13 } else { rustls::ALL_VERSIONS }
    }
}

/// Os protocolos do ALPN na codificação por comprimento
/// (`SecurityContext._protocolsToLengthEncoding`).
fn protocols_of(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut v = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let n = bytes[i] as usize;
        let end = (i + 1 + n).min(bytes.len());
        v.push(bytes[i + 1..end].to_vec());
        i = end;
    }
    v
}

fn object_context(obj: i64) -> Option<SharedContext> {
    let p = native_field(obj);
    if p == 0 {
        return None;
    }
    // SAFETY: o campo guarda um `Arc` de `DartForge_tls_contexto_novo`, vivo
    // até a coleta do objeto; aqui se toma mais uma referência.
    unsafe {
        let a = p as *const std::sync::Mutex<TlsContext>;
        std::sync::Arc::increment_strong_count(a);
        Some(std::sync::Arc::from_raw(a))
    }
}

fn free_tls_context(p: usize) {
    // SAFETY: a referência do objeto, solta uma vez pelo finalizador.
    unsafe { drop(std::sync::Arc::from_raw(p as *const std::sync::Mutex<TlsContext>)) };
}

fn with_context<T>(obj: i64, f: impl FnOnce(&mut TlsContext) -> T) -> Option<T> {
    let c = object_context(obj)?;
    let mut g = c.lock().unwrap_or_else(|e| e.into_inner());
    Some(f(&mut g))
}

fn string_or_null(r: Result<(), String>) -> i64 {
    match r {
        Ok(()) => 0,
        Err(e) => allocate_str(&format!("\n\t{e}")),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_novo(this: i64) {
    let c: SharedContext = std::sync::Arc::new(std::sync::Mutex::new(TlsContext::new()));
    let p = std::sync::Arc::into_raw(c) as i64;
    attach_finalizer(this, free_tls_context, p as usize);
    write_native_field(this, p);
}

/// `usePrivateKeyBytes`: se havia chave (formatos em `tls_formatos.rs`); sem
/// ela, o Dart lança a `ArgumentError` da VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_chave(this: i64, bytes: i64, password: i64) -> u8 {
    let b = typed_list_bytes(bytes).unwrap_or_default();
    let password = String::from_utf8_lossy(&utf8_of_string(password)).into_owned();
    match key_of(&b, &password) {
        Ok(k) => {
            with_context(this, |c| c.key = Some(k));
            1
        }
        Err(()) => 0,
    }
}

/// A senha de um native de certificados (PKCS#12).
fn password_of(password: i64) -> String {
    String::from_utf8_lossy(&utf8_of_string(password)).into_owned()
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_confiaveis(this: i64, bytes: i64, password: i64) -> i64 {
    let b = typed_list_bytes(bytes).unwrap_or_default();
    string_or_null(certificates_of(&b, &password_of(password)).map(|certs| {
        with_context(this, |c| {
            c.trusted.extend(certs);
            c.roots = None;
        });
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_cadeia(this: i64, bytes: i64, password: i64) -> i64 {
    let b = typed_list_bytes(bytes).unwrap_or_default();
    string_or_null(certificates_of(&b, &password_of(password)).map(|certs| {
        with_context(this, |c| c.chain = certs);
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_autoridades(this: i64, bytes: i64, password: i64) -> i64 {
    let b = typed_list_bytes(bytes).unwrap_or_default();
    string_or_null(certificates_of(&b, &password_of(password)).map(|certs| {
        with_context(this, |c| c.authorities.extend(certs));
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_alpn(this: i64, bytes: i64, tls_server: u8) {
    let protocols = protocols_of(&typed_list_bytes(bytes).unwrap_or_default());
    with_context(this, |c| {
        if tls_server != 0 {
            c.server_alpn = protocols;
        } else {
            c.client_alpn = protocols;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_raizes_embutidas(this: i64) {
    with_context(this, |c| {
        c.builtin_roots = true;
        c.roots = None;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_definir_versao_minima(this: i64, version: i64) {
    with_context(this, |c| c.min_version = version);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_versao_minima(this: i64) -> i64 {
    with_context(this, |c| c.min_version).unwrap_or(TLS_DEFAULT_MIN_VERSION)
}

// ---------------------------------------------------------------------------
// O filtro.

/// A decisão sobre o certificado do servidor que as raízes não validaram.
#[derive(Default)]
struct PendingVerification {
    /// O certificado e o motivo (o texto do BoringSSL), enquanto o Dart não
    /// decide.
    pending: Option<(rustls::pki_types::CertificateDer<'static>, String)>,
    /// O Dart aceitou o certificado (`onBadCertificate` devolveu `true`).
    accepted: bool,
}

type Verification = std::sync::Arc<std::sync::Mutex<PendingVerification>>;

/// O verificador do cliente: as raízes do contexto pelo webpki; o que elas
/// não validam fica para o Dart (ver o topo do arquivo). As assinaturas do
/// handshake são sempre conferidas.
#[derive(Debug)]
struct DartVerifier {
    webpki: Option<std::sync::Arc<rustls::client::WebPkiServerVerifier>>,
    provider: std::sync::Arc<rustls::crypto::CryptoProvider>,
    estado: Verification,
}

impl std::fmt::Debug for PendingVerification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerificacaoPendente")
    }
}

/// O texto do BoringSSL (`X509_verify_cert_error_string`) de um erro de
/// certificado do rustls.
fn certificate_reason(e: &rustls::Error, self_signed: bool) -> String {
    use rustls::CertificateError as C;
    match e {
        rustls::Error::InvalidCertificate(c) => match c {
            C::UnknownIssuer if self_signed => "self signed certificate".into(),
            C::UnknownIssuer => "unable to get local issuer certificate".into(),
            C::Expired | C::ExpiredContext { .. } => "certificate has expired".into(),
            C::NotValidYet | C::NotValidYetContext { .. } => "certificate is not yet valid".into(),
            C::NotValidForName | C::NotValidForNameContext { .. } => "Hostname mismatch".into(),
            C::BadSignature => "certificate signature failure".into(),
            C::Revoked => "certificate revoked".into(),
            other_one => format!("{other_one:?}"),
        },
        other_one => other_one.to_string(),
    }
}

impl rustls::client::danger::ServerCertVerifier for DartVerifier {
    fn verify_server_cert(
        &self,
        leaf: &rustls::pki_types::CertificateDer<'_>,
        intermediates: &[rustls::pki_types::CertificateDer<'_>],
        name: &rustls::pki_types::ServerName<'_>,
        ocsp: &[u8],
        current_time: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let result = match &self.webpki {
            Some(v) => v.verify_server_cert(leaf, intermediates, name, ocsp, current_time),
            None => Err(rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer)),
        };
        if let Err(e) = result {
            let mut estado = self.estado.lock().unwrap_or_else(|e| e.into_inner());
            if !estado.accepted {
                let self_signed = intermediates.is_empty() && x509_self_signed(leaf);
                estado.pending = Some((leaf.clone().into_owned(), certificate_reason(&e, self_signed)));
            }
        }
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// O `keyLog` do `SecureSocket`: cada linha no formato NSS fica guardada
/// até o Dart levá-la à porta dele (`DartForge_tls_filtro_chaves_pendentes`).
#[derive(Debug, Default)]
struct KeyRegistry(std::sync::Mutex<Vec<String>>);

impl rustls::KeyLog for KeyRegistry {
    fn log(&self, label_name: &str, random: &[u8], secret: &[u8]) {
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        let line = format!("{label_name} {} {}", hex(random), hex(secret));
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(line);
    }
}

/// O estado nativo de um `_SecureFilterImpl`.
struct TlsFilter {
    connection: Option<rustls::Connection>,
    tls_server: bool,
    verification: Verification,
    /// O erro do handshake (texto do `OS Error`), relatado pelo próximo
    /// `handshake()`; ou o erro do último processamento.
    error: Option<String>,
    /// O fim do handshake já foi relatado ao Dart.
    reported: bool,
    keys: Option<std::sync::Arc<KeyRegistry>>,
}

fn object_filter<'a>(obj: i64) -> Option<&'a mut TlsFilter> {
    let p = native_field(obj);
    if p == 0 {
        return None;
    }
    // SAFETY: o campo guarda a `Box` de `DartForge_tls_filtro_novo`, só usada
    // pela thread do isolado dono, e viva até `destroy` ou a coleta.
    Some(unsafe { &mut *(p as *mut TlsFilter) })
}

fn free_tls_filter(p: usize) {
    // SAFETY: a `Box` do filtro, solta uma vez.
    unsafe { drop(Box::from_raw(p as *mut TlsFilter)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_novo(this: i64) {
    let f = Box::new(TlsFilter {
        connection: None,
        tls_server: false,
        verification: Verification::default(),
        error: None,
        reported: false,
        keys: None,
    });
    let p = Box::into_raw(f) as i64;
    attach_finalizer(this, free_tls_filter, p as usize);
    write_native_field(this, p);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_destruir(this: i64) {
    let p = native_field(this);
    if p != 0 {
        remove_finalizer(this);
        write_native_field(this, 0);
        free_tls_filter(p as usize);
    }
}

/// `registerKeyLogPort`: as chaves da sessão passam a ser registradas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_registrar_chaves(this: i64) {
    if let Some(f) = object_filter(this) {
        f.keys = Some(std::sync::Arc::default());
    }
}

/// As linhas do `keyLog` desde a última chamada, separadas por `\n`, ou
/// `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_chaves_pendentes(this: i64) -> i64 {
    let line_list = object_filter(this)
        .and_then(|f| f.keys.as_ref().map(|c| std::mem::take(&mut *c.0.lock().unwrap_or_else(|e| e.into_inner()))))
        .unwrap_or_default();
    if line_list.is_empty() { 0 } else { allocate_str(&line_list.join("\n")) }
}

/// `SecureSocket_Connect`: monta a conexão do rustls. `null`, ou o texto do
/// erro (o Dart lança a `TlsException`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_conectar(
    this: i64,
    host: i64,
    contexto: i64,
    tls_server: u8,
    request_certificate: u8,
    require_certificate: u8,
    protocols: i64,
) -> i64 {
    let Some(f) = object_filter(this) else { return allocate_str("SecureSocket destruído") };
    let host = String::from_utf8_lossy(&utf8_of_string(host)).into_owned();
    let protocols = protocols_of(&typed_list_bytes(protocols).unwrap_or_default());
    let Some(contexto) = object_context(contexto) else { return allocate_str("SecurityContext sem estado nativo") };
    let mut c = contexto.lock().unwrap_or_else(|e| e.into_inner());
    let r = if tls_server != 0 {
        connect_server(f, &mut c, request_certificate != 0, require_certificate != 0, protocols)
    } else {
        connect_client(f, &mut c, &host, protocols)
    };
    match r {
        Ok(()) => 0,
        Err(e) => allocate_str(&e),
    }
}

fn connect_client(f: &mut TlsFilter, c: &mut TlsContext, host: &str, protocols: Vec<Vec<u8>>) -> Result<(), String> {
    let provider = tls_provider();
    let roots = c.roots();
    let webpki = if roots.is_empty() {
        None
    } else {
        Some(
            rustls::client::WebPkiServerVerifier::builder_with_provider(roots, provider.clone())
                .build()
                .map_err(|e| e.to_string())?,
        )
    };
    let verifier = DartVerifier { webpki, provider: provider.clone(), estado: f.verification.clone() };
    let base = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(c.versions())
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(std::sync::Arc::new(verifier));
    let mut config = match (&c.key, c.chain.is_empty()) {
        (Some(k), false) => base.with_client_auth_cert(c.chain.clone(), k.clone_key()).map_err(|e| e.to_string())?,
        _ => base.with_no_client_auth(),
    };
    config.alpn_protocols = if protocols.is_empty() { c.client_alpn.clone() } else { protocols };
    if let Some(c) = &f.keys {
        config.key_log = c.clone();
    }
    let name = rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|e| format!("{e}: {host}"))?;
    let connection = rustls::ClientConnection::new(std::sync::Arc::new(config), name).map_err(|e| e.to_string())?;
    f.connection = Some(rustls::Connection::Client(connection));
    f.tls_server = false;
    Ok(())
}

fn connect_server(
    f: &mut TlsFilter,
    c: &mut TlsContext,
    request_fn: bool,
    require: bool,
    protocols: Vec<Vec<u8>>,
) -> Result<(), String> {
    let provider = tls_provider();
    let Some(key) = c.key.as_ref().map(|k| k.clone_key()) else {
        return Err("\n\tNO_PRIVATE_KEY_ASSIGNED(ssl_lib.cc)".to_string());
    };
    let base = rustls::ServerConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(c.versions())
        .map_err(|e| e.to_string())?;
    // O certificado do cliente: validado pelas autoridades do contexto (ou
    // pelas raízes confiáveis), opcional se só foi pedido.
    let mut authorities = rustls::RootCertStore::empty();
    authorities.add_parsable_certificates(c.authorities.iter().cloned());
    if authorities.is_empty() {
        authorities = (*c.roots()).clone();
    }
    let base = if (request_fn || require) && !authorities.is_empty() {
        let b = rustls::server::WebPkiClientVerifier::builder_with_provider(std::sync::Arc::new(authorities), provider);
        let b = if require { b } else { b.allow_unauthenticated() };
        base.with_client_cert_verifier(b.build().map_err(|e| e.to_string())?)
    } else {
        base.with_no_client_auth()
    };
    let mut config = base.with_single_cert(c.chain.clone(), key).map_err(|e| e.to_string())?;
    config.alpn_protocols = if protocols.is_empty() { c.server_alpn.clone() } else { protocols };
    if let Some(c) = &f.keys {
        config.key_log = c.clone();
    }
    let connection = rustls::ServerConnection::new(std::sync::Arc::new(config)).map_err(|e| e.to_string())?;
    f.connection = Some(rustls::Connection::Server(connection));
    f.tls_server = true;
    Ok(())
}

/// O `SSLFilter::Handshake` da VM: 0 em andamento, 1 terminou agora (o Dart
/// chama o `handshakeCompleteHandler`), 16 o certificado espera a decisão
/// do Dart, -1 erro (`DartForge_tls_filtro_erro`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_handshake(this: i64) -> i64 {
    let Some(f) = object_filter(this) else { return -1 };
    if f.error.is_some() {
        return -1;
    }
    if f.verification.lock().unwrap_or_else(|e| e.into_inner()).pending.is_some() {
        return TLS_AWAIT_VERIFICATION;
    }
    match &f.connection {
        Some(c) if !c.is_handshaking() && !f.reported => {
            f.reported = true;
            1
        }
        Some(_) => 0,
        None => {
            f.error = Some("\n\tSecureSocket sem conexão".to_string());
            -1
        }
    }
}

/// O texto do erro (o `OS Error`), ou `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_erro(this: i64) -> i64 {
    match object_filter(this).and_then(|f| f.error.clone()) {
        Some(e) => allocate_str(&e),
        None => 0,
    }
}

/// O certificado que espera a decisão, para o `onBadCertificate`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_certificado_pendente(this: i64) -> i64 {
    let Some(f) = object_filter(this) else { return 0 };
    let v = f.verification.lock().unwrap_or_else(|e| e.into_inner());
    v.pending.as_ref().map_or(0, |(c, _)| x509_new(c.clone()))
}

/// `_markAsTrusted`: a decisão do Dart sobre o certificado pendente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_decidir(this: i64, trusted_one: u8) {
    let Some(f) = object_filter(this) else { return };
    let mut v = f.verification.lock().unwrap_or_else(|e| e.into_inner());
    let Some((_, reason)) = v.pending.take() else { return };
    if trusted_one != 0 {
        v.accepted = true;
    } else {
        drop(v);
        f.error = Some(format!("\n\tCERTIFICATE_VERIFY_FAILED: {reason}(handshake.cc:392)"));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_protocolo(this: i64) -> i64 {
    match object_filter(this).and_then(|f| f.connection.as_ref()?.alpn_protocol().map(<[u8]>::to_vec)) {
        Some(p) => dart_string_from_bytes(&p),
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_certificado_do_par(this: i64) -> i64 {
    let Some(f) = object_filter(this) else { return 0 };
    match f.connection.as_ref().and_then(|c| c.peer_certificates()).and_then(|c| c.first()) {
        Some(c) => x509_new(c.clone().into_owned()),
        None => 0,
    }
}

/// Os anéis: texto lido, texto a escrever, cifrado lido, cifrado a escrever
/// (`_RawSecureSocket.readPlaintextId`…).
const RING_PLAIN_READ: usize = 0;
const RING_PLAIN_TO_WRITE: usize = 1;
const RING_ENCRYPTED_READ: usize = 2;
const RING_ENCRYPTED_TO_WRITE: usize = 3;

/// Consome os dados do anel `[start, end)` (a leitura do
/// `ProcessAllBuffers`): `f` recebe um trecho e devolve quanto consumiu.
fn ring_consume(buf: &[u8], start: &mut usize, end: usize, f: &mut dyn FnMut(&[u8]) -> usize) {
    let size = buf.len();
    let mut chunk = |de: usize, until: usize| {
        let mut done = 0;
        while de + done < until {
            let n = f(&buf[de + done..until]);
            if n == 0 {
                break;
            }
            done += n;
        }
        done
    };
    if end < *start {
        *start += chunk(*start, size);
        if *start == size {
            *start = 0;
        }
    }
    if *start < end {
        *start += chunk(*start, end);
    }
}

/// Enche o espaço livre do anel depois de `end` (a escrita do
/// `ProcessAllBuffers`: um byte fica sempre livre, para cheio ≠ vazio).
fn ring_fill(buf: &mut [u8], start: usize, end: &mut usize, f: &mut dyn FnMut(&mut [u8]) -> usize) {
    let size = buf.len();
    let mut chunk = |buf: &mut [u8], de: usize, until: usize| {
        let mut done = 0;
        while de + done < until {
            let n = f(&mut buf[de + done..until]);
            if n == 0 {
                break;
            }
            done += n;
        }
        done
    };
    if start <= *end {
        let limit = if start == 0 { size - 1 } else { size };
        *end += chunk(buf, *end, limit);
        if *end == size {
            *end = 0;
        }
    }
    if start > *end + 1 {
        *end += chunk(buf, *end, start - 1);
    }
}

/// O `ProcessAllBuffers` da VM sobre os quatro anéis (`b0`…`b3`, as
/// `Uint8List` dos `_ExternalBuffer`) e as posições (`pos`, uma `Int64List`
/// com `[início, end]` de cada anel, atualizada no lugar). 0, ou o código do
/// erro fatal depois do handshake (o texto em `DartForge_tls_filtro_erro`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_processar(
    this: i64,
    in_handshake: u8,
    b0: i64,
    b1: i64,
    b2: i64,
    b3: i64,
    pos: i64,
) -> i64 {
    let Some(f) = object_filter(this) else { return 0 };
    let listas = [b0, b1, b2, b3];
    // O processamento trabalha direto sobre os bytes dos anéis e das
    // posições (§2.14: o coletor não move, e nada aqui chama o Dart nem aloca
    // no heap), sem cópia.
    // SAFETY: as cinco listas são distintas (os `_ExternalBuffer` e a lista
    // das posições do `_SecureFilterImpl`) e vivas (argumentos enraizados pelo
    // código gerado) durante toda a chamada.
    let mut rings: [&mut [u8]; 4] = std::array::from_fn(|i| unsafe { typed_bytes_mut_of(listas[i]) });
    // SAFETY: como acima.
    let bytes_pos = unsafe { typed_bytes_mut_of(pos) };
    if bytes_pos.len() < 64 {
        return 0;
    }
    let mut p = [0usize; 8];
    for (i, v) in p.iter_mut().enumerate() {
        let mut b = [0u8; 8];
        b.copy_from_slice(&bytes_pos[i * 8..i * 8 + 8]);
        *v = i64::from_ne_bytes(b).max(0) as usize;
    }
    let r = process_rings(f, in_handshake != 0, &mut rings, &mut p);
    for (i, v) in p.iter().enumerate() {
        bytes_pos[i * 8..i * 8 + 8].copy_from_slice(&(*v as i64).to_ne_bytes());
    }
    match r {
        Ok(()) => 0,
        Err((code, text)) => {
            f.error = Some(text);
            code
        }
    }
}

fn process_rings(f: &mut TlsFilter, in_handshake: bool, rings: &mut [&mut [u8]; 4], p: &mut [usize; 8]) -> Result<(), (i64, String)> {
    use std::io::{Read, Write};
    let Some(connection) = f.connection.as_mut() else { return Ok(()) };
    for (i, a) in rings.iter().enumerate() {
        let size = a.len();
        if size == 0 || p[2 * i] >= size || p[2 * i + 1] >= size {
            return Err((1, "Out-of-bounds internal buffer access in dart:io SecureSocket".to_string()));
        }
    }
    // Cifrado lido → TLS.
    let (mut start, end) = (p[2 * RING_ENCRYPTED_READ], p[2 * RING_ENCRYPTED_READ + 1]);
    ring_consume(&rings[RING_ENCRYPTED_READ], &mut start, end, &mut |chunk| {
        let mut tls_reader = chunk;
        connection.read_tls(&mut tls_reader).unwrap_or(0)
    });
    let had_read = start != p[2 * RING_ENCRYPTED_READ];
    p[2 * RING_ENCRYPTED_READ] = start;
    if (had_read || in_handshake)
        && f.error.is_none()
        && let Err(e) = connection.process_new_packets()
    {
        let text = format!("\n\t{e}");
        if in_handshake {
            // Relatado pelo próximo `handshake()`, como o erro do
            // `SSL_do_handshake`; o alerta ainda sai para o par.
            f.error = Some(text);
        } else {
            return Err((1, text));
        }
    }
    let blocked = f.verification.lock().unwrap_or_else(|e| e.into_inner()).pending.is_some();
    if !in_handshake && !blocked && f.error.is_none() {
        // TLS → texto lido.
        let (start, mut end) = (p[2 * RING_PLAIN_READ], p[2 * RING_PLAIN_READ + 1]);
        let mut error = None;
        ring_fill(&mut rings[RING_PLAIN_READ], start, &mut end, &mut |chunk| match connection.reader().read(chunk) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => 0,
            Err(e) => {
                error = Some(e);
                0
            }
        });
        p[2 * RING_PLAIN_READ + 1] = end;
        if let Some(e) = error
            && e.kind() != std::io::ErrorKind::UnexpectedEof
        {
            return Err((1, format!("\n\t{e}")));
        }
        // Texto a escrever → TLS.
        let (mut start, end) = (p[2 * RING_PLAIN_TO_WRITE], p[2 * RING_PLAIN_TO_WRITE + 1]);
        ring_consume(&rings[RING_PLAIN_TO_WRITE], &mut start, end, &mut |chunk| connection.writer().write(chunk).unwrap_or(0));
        p[2 * RING_PLAIN_TO_WRITE] = start;
    }
    if !blocked {
        // TLS → cifrado a escrever.
        let (start, mut end) = (p[2 * RING_ENCRYPTED_TO_WRITE], p[2 * RING_ENCRYPTED_TO_WRITE + 1]);
        ring_fill(&mut rings[RING_ENCRYPTED_TO_WRITE], start, &mut end, &mut |chunk| {
            let mut tls_writer = chunk;
            connection.write_tls(&mut tls_writer).unwrap_or(0)
        });
        p[2 * RING_ENCRYPTED_TO_WRITE + 1] = end;
    }
    Ok(())
}

/// Os bytes graváveis da lista tipada `h` (vazio se não é lista tipada
/// modificável).
///
/// # Safety
/// A lista vive enquanto a fatia é usada, nada aloca no heap nesse tempo, e
/// não há outra fatia viva dos mesmos bytes.
unsafe fn typed_bytes_mut_of<'a>(h: i64) -> &'a mut [u8] {
    match HEAP.with(|heap| heap.borrow().tipada(h)) {
        // SAFETY: pelo contrato desta função.
        Some(t) if !t.unmodifiable => unsafe { t.slice_mut() },
        _ => &mut [],
    }
}

// ---------------------------------------------------------------------------
// X509Certificate.

/// Um certificado (`_X509CertificateImpl`): o DER, com contagem de
/// referências (o objeto Dart e quem o entrega).
type X509Certificate = std::sync::Arc<rustls::pki_types::CertificateDer<'static>>;

/// Um ponteiro com uma referência do certificado, para o Dart adotar
/// (`DartForge_tls_x509_adotar`).
fn x509_new(c: rustls::pki_types::CertificateDer<'static>) -> i64 {
    std::sync::Arc::into_raw(X509Certificate::new(c)) as i64
}

fn free_x509(p: usize) {
    // SAFETY: a referência do objeto, solta uma vez.
    unsafe { drop(std::sync::Arc::from_raw(p as *const rustls::pki_types::CertificateDer<'static>)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_adotar(this: i64, p: i64) {
    if p != 0 {
        attach_finalizer(this, free_x509, p as usize);
        write_native_field(this, p);
    }
}

fn x509_der(this: i64) -> Vec<u8> {
    let p = native_field(this);
    if p == 0 {
        return Vec::new();
    }
    // SAFETY: a referência do objeto está viva até a coleta dele.
    unsafe { (*(p as *const rustls::pki_types::CertificateDer<'static>)).to_vec() }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_der(this: i64) -> i64 {
    dart_bytes(x509_der(this))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_pem(this: i64) -> i64 {
    let b64 = base64_standard(&x509_der(this));
    let mut pem = String::from("-----BEGIN CERTIFICATE-----\n");
    for line in b64.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(line).unwrap_or_default());
        pem.push('\n');
    }
    pem.push_str("-----END CERTIFICATE-----\n");
    allocate_str(&pem)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_sha1(this: i64) -> i64 {
    let d = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &x509_der(this));
    dart_bytes(d.as_ref().to_vec())
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_sujeito(this: i64) -> i64 {
    let der = x509_der(this);
    allocate_str(&x509_fields(&der).map(|c| x509_name(c.subject)).unwrap_or_default())
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_emissor(this: i64) -> i64 {
    let der = x509_der(this);
    allocate_str(&x509_fields(&der).map(|c| x509_name(c.emitter)).unwrap_or_default())
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_inicio(this: i64) -> i64 {
    let der = x509_der(this);
    x509_fields(&der).and_then(|c| x509_instant(c.start)).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_fim(this: i64) -> i64 {
    let der = x509_der(this);
    x509_fields(&der).and_then(|c| x509_instant(c.end)).unwrap_or(0)
}

fn base64_standard(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        s.push(A[(n >> 18) as usize & 63] as char);
        s.push(A[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    s
}

/// Um elemento DER: a etiqueta, o conteúdo e o resto depois dele.
fn der_element(b: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let tag = *b.first()?;
    let first = *b.get(1)?;
    let (size, header) = if first < 0x80 {
        (first as usize, 2)
    } else {
        let n = (first & 0x7f) as usize;
        if n == 0 || n > 4 {
            return None;
        }
        let mut t = 0usize;
        for i in 0..n {
            t = (t << 8) | *b.get(2 + i)? as usize;
        }
        (t, 2 + n)
    };
    let end = header.checked_add(size)?;
    Some((tag, b.get(header..end)?, &b[end..]))
}

/// Os campos do `tbsCertificate` que o `X509Certificate` mostra.
struct X509Fields<'a> {
    emitter: &'a [u8],
    start: (u8, &'a [u8]),
    end: (u8, &'a [u8]),
    subject: &'a [u8],
}

fn x509_fields(der: &[u8]) -> Option<X509Fields<'_>> {
    let (_, cert, _) = der_element(der)?;
    let (_, tbs, _) = der_element(cert)?;
    let (mut tag, _, mut rest) = der_element(tbs)?;
    // `[0] version` é opcional.
    if tag == 0xa0 {
        (tag, _, rest) = der_element(rest)?;
    }
    let _ = tag; // serialNumber
    let (_, _, rest) = der_element(rest)?; // signature
    let (_, emitter, rest) = der_element(rest)?;
    let (_, validity, rest) = der_element(rest)?;
    let (_, subject, _) = der_element(rest)?;
    let (e1, start, r) = der_element(validity)?;
    let (e2, end, _) = der_element(r)?;
    Some(X509Fields { emitter, start: (e1, start), end: (e2, end), subject })
}

fn x509_self_signed(c: &rustls::pki_types::CertificateDer<'_>) -> bool {
    x509_fields(c).is_some_and(|c| c.emitter == c.subject)
}

/// O nome no formato de `X509_NAME_oneline` do BoringSSL:
/// `/C=US/O=Let's Encrypt/CN=R3`.
fn x509_name(name: &[u8]) -> String {
    let mut s = String::new();
    let mut rdns = name;
    while let Some((_, rdn, rest)) = der_element(rdns) {
        rdns = rest;
        let mut attributes = rdn;
        while let Some((_, attribute, rest)) = der_element(attributes) {
            attributes = rest;
            let Some((_, oid, r)) = der_element(attribute) else { continue };
            let Some((tag, value, _)) = der_element(r) else { continue };
            s.push('/');
            s.push_str(&oid_name(oid));
            s.push('=');
            let text = if tag == 0x1e {
                // BMPString (UCS-2 big endian).
                String::from_utf16_lossy(&value.chunks(2).map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)])).collect::<Vec<_>>())
            } else {
                String::from_utf8_lossy(value).into_owned()
            };
            for ch in text.chars() {
                if ch.is_ascii() && !ch.is_ascii_control() {
                    s.push(ch);
                } else {
                    let mut b = [0u8; 4];
                    for byte in ch.encode_utf8(&mut b).bytes() {
                        s.push_str(&format!("\\x{byte:02X}"));
                    }
                }
            }
        }
    }
    s
}

fn oid_name(oid: &[u8]) -> String {
    let known = match oid {
        [0x55, 0x04, 0x03] => "CN",
        [0x55, 0x04, 0x06] => "C",
        [0x55, 0x04, 0x07] => "L",
        [0x55, 0x04, 0x08] => "ST",
        [0x55, 0x04, 0x09] => "street",
        [0x55, 0x04, 0x0a] => "O",
        [0x55, 0x04, 0x0b] => "OU",
        [0x55, 0x04, 0x0c] => "title",
        [0x55, 0x04, 0x04] => "SN",
        [0x55, 0x04, 0x05] => "serialNumber",
        [0x55, 0x04, 0x11] => "postalCode",
        [0x55, 0x04, 0x2a] => "GN",
        [0x55, 0x04, 0x2e] => "dnQualifier",
        [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x01] => "emailAddress",
        [0x09, 0x92, 0x26, 0x89, 0x93, 0xf2, 0x2c, 0x64, 0x01, 0x19] => "DC",
        [0x09, 0x92, 0x26, 0x89, 0x93, 0xf2, 0x2c, 0x64, 0x01, 0x01] => "UID",
        _ => "",
    };
    if !known.is_empty() {
        return known.to_string();
    }
    // O OID em pontos.
    let mut part_list = Vec::new();
    if let Some(&p) = oid.first() {
        part_list.push((p / 40).min(2) as u64);
        part_list.push(u64::from(p) - 40 * (p / 40).min(2) as u64);
    }
    let mut v = 0u64;
    for &b in oid.iter().skip(1) {
        v = (v << 7) | u64::from(b & 0x7f);
        if b & 0x80 == 0 {
            part_list.push(v);
            v = 0;
        }
    }
    part_list.iter().map(u64::to_string).collect::<Vec<_>>().join(".")
}

/// Milissegundos desde a época de um `UTCTime` (0x17) ou `GeneralizedTime`
/// (0x18) em UTC.
fn x509_instant((tag, t): (u8, &[u8])) -> Option<i64> {
    let s = std::str::from_utf8(t).ok()?;
    let digits = |de: usize, n: usize| -> Option<i64> { s.get(de..de + n)?.parse().ok() };
    let (year, rest) = if tag == 0x17 {
        let a = digits(0, 2)?;
        (if a < 50 { 2000 + a } else { 1900 + a }, 2)
    } else {
        (digits(0, 4)?, 4)
    };
    let month = digits(rest, 2)?;
    let day = digits(rest + 2, 2)?;
    let hour = digits(rest + 4, 2)?;
    let minute = digits(rest + 6, 2)?;
    let second = digits(rest + 8, 2).unwrap_or(0);
    // Dias desde 1970-01-01 (algoritmo de Howard Hinnant).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(((days * 24 + hour) * 60 + minute) * 60_000 + second * 1000)
}
