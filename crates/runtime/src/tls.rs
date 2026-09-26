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
// da VM (`sdk_nativo/io/secure_socket_patch.dart`) e o pedido
// `sslProcessFilter` do IOService, atendido na thread do isolado
// (`sdk_nativo/io/io_service_patch.dart`): o filtro só faz criptografia em
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
const TLS_VERSAO_MINIMA_PADRAO: i64 = 0x0303;

/// `SSL_ERROR_WANT_CERTIFICATE_VERIFY`: o handshake espera a decisão do Dart.
const TLS_ESPERA_VERIFICACAO: i64 = 16;

/// Um `SecurityContext`: as raízes, a cadeia e a chave, o ALPN e a versão
/// mínima. Fica no campo nativo do objeto como `Arc<Mutex<_>>`.
struct ContextoTls {
    confiaveis: Vec<rustls::pki_types::CertificateDer<'static>>,
    raizes_embutidas: bool,
    cadeia: Vec<rustls::pki_types::CertificateDer<'static>>,
    chave: Option<rustls::pki_types::PrivateKeyDer<'static>>,
    autoridades: Vec<rustls::pki_types::CertificateDer<'static>>,
    alpn_cliente: Vec<Vec<u8>>,
    alpn_servidor: Vec<Vec<u8>>,
    versao_minima: i64,
    /// As raízes montadas (as confiáveis e, se pedido, as do sistema); refeitas
    /// quando o contexto muda.
    raizes: Option<std::sync::Arc<rustls::RootCertStore>>,
}

type ContextoCompartilhado = std::sync::Arc<std::sync::Mutex<ContextoTls>>;

fn provedor_tls() -> std::sync::Arc<rustls::crypto::CryptoProvider> {
    static P: std::sync::OnceLock<std::sync::Arc<rustls::crypto::CryptoProvider>> = std::sync::OnceLock::new();
    P.get_or_init(|| std::sync::Arc::new(rustls::crypto::ring::default_provider())).clone()
}

/// As raízes do sistema (`SecurityContext.defaultContext`,
/// `withTrustedRoots: true`): o repositório de certificados do sistema
/// (arquivos do Linux, chaveiro do macOS, repositório do Windows); sem
/// nenhuma, as da Mozilla embutidas.
fn raizes_do_sistema() -> &'static [rustls::pki_types::CertificateDer<'static>] {
    static R: std::sync::OnceLock<Vec<rustls::pki_types::CertificateDer<'static>>> = std::sync::OnceLock::new();
    R.get_or_init(|| rustls_native_certs::load_native_certs().certs)
}

impl ContextoTls {
    fn novo() -> Self {
        ContextoTls {
            confiaveis: Vec::new(),
            raizes_embutidas: false,
            cadeia: Vec::new(),
            chave: None,
            autoridades: Vec::new(),
            alpn_cliente: Vec::new(),
            alpn_servidor: Vec::new(),
            versao_minima: TLS_VERSAO_MINIMA_PADRAO,
            raizes: None,
        }
    }

    fn raizes(&mut self) -> std::sync::Arc<rustls::RootCertStore> {
        if let Some(r) = &self.raizes {
            return r.clone();
        }
        let mut loja = rustls::RootCertStore::empty();
        loja.add_parsable_certificates(self.confiaveis.iter().cloned());
        if self.raizes_embutidas {
            let sistema = raizes_do_sistema();
            if sistema.is_empty() {
                loja.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            } else {
                loja.add_parsable_certificates(sistema.iter().cloned());
            }
        }
        let r = std::sync::Arc::new(loja);
        self.raizes = Some(r.clone());
        r
    }

    fn versoes(&self) -> &'static [&'static rustls::SupportedProtocolVersion] {
        static SO_TLS13: [&rustls::SupportedProtocolVersion; 1] = [&rustls::version::TLS13];
        if self.versao_minima >= 0x0304 { &SO_TLS13 } else { rustls::ALL_VERSIONS }
    }
}

/// Os certificados de `bytes`: PEM (um ou mais), senão um DER. Senha: a VM
/// aceita PKCS#12 protegido; aqui é recusado com a mensagem.
fn certificados_de(bytes: &[u8]) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>, String> {
    use rustls::pki_types::pem::PemObject;
    let pem: Vec<_> = rustls::pki_types::CertificateDer::pem_slice_iter(bytes).collect();
    if pem.is_empty() {
        if bytes.first() == Some(&0x30) {
            return Ok(vec![rustls::pki_types::CertificateDer::from(bytes.to_vec())]);
        }
        return Err("NO_START_LINE(pem_lib.c:631)".to_string());
    }
    pem.into_iter().collect::<Result<Vec<_>, _>>().map_err(|e| format!("BAD_BASE64_DECODE({e})"))
}

/// A chave privada de `bytes`: PEM (PKCS#8, PKCS#1 ou SEC1) ou DER.
fn chave_de(bytes: &[u8], senha: &str) -> Result<rustls::pki_types::PrivateKeyDer<'static>, String> {
    use rustls::pki_types::pem::PemObject;
    if bytes.windows(9).any(|j| j == b"ENCRYPTED") {
        return Err(if senha.is_empty() {
            "BAD_DECRYPT(pem_lib.c:420)".to_string()
        } else {
            "UNSUPPORTED_ENCRYPTION: chave privada cifrada ainda não suportada".to_string()
        });
    }
    match rustls::pki_types::PrivateKeyDer::from_pem_slice(bytes) {
        Ok(k) => Ok(k),
        Err(_) => rustls::pki_types::PrivateKeyDer::try_from(bytes.to_vec()).map_err(|_| "NO_START_LINE(pem_lib.c:631)".to_string()),
    }
}

/// Os protocolos do ALPN na codificação por comprimento
/// (`SecurityContext._protocolsToLengthEncoding`).
fn protocolos_de(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut v = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let n = bytes[i] as usize;
        let fim = (i + 1 + n).min(bytes.len());
        v.push(bytes[i + 1..fim].to_vec());
        i = fim;
    }
    v
}

fn contexto_do_objeto(obj: i64) -> Option<ContextoCompartilhado> {
    let p = campo_nativo(obj);
    if p == 0 {
        return None;
    }
    // SAFETY: o campo guarda um `Arc` de `DartForge_tls_contexto_novo`, vivo
    // até a coleta do objeto; aqui se toma mais uma referência.
    unsafe {
        let a = p as *const std::sync::Mutex<ContextoTls>;
        std::sync::Arc::increment_strong_count(a);
        Some(std::sync::Arc::from_raw(a))
    }
}

fn liberar_contexto_tls(p: usize) {
    // SAFETY: a referência do objeto, solta uma vez pelo finalizador.
    unsafe { drop(std::sync::Arc::from_raw(p as *const std::sync::Mutex<ContextoTls>)) };
}

fn com_contexto<T>(obj: i64, f: impl FnOnce(&mut ContextoTls) -> T) -> Option<T> {
    let c = contexto_do_objeto(obj)?;
    let mut g = c.lock().unwrap_or_else(|e| e.into_inner());
    Some(f(&mut g))
}

fn texto_ou_nulo(r: Result<(), String>) -> i64 {
    match r {
        Ok(()) => 0,
        Err(e) => alocar_str(&format!("\n\t{e}")),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_novo(this: i64) {
    let c: ContextoCompartilhado = std::sync::Arc::new(std::sync::Mutex::new(ContextoTls::novo()));
    let p = std::sync::Arc::into_raw(c) as i64;
    anexar_finalizador(this, liberar_contexto_tls, p as usize);
    gravar_campo_nativo(this, p);
}

/// `usePrivateKeyBytes`: `null`, ou o texto do erro (o Dart lança a
/// `TlsException`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_chave(this: i64, bytes: i64, senha: i64) -> i64 {
    let b = bytes_da_lista_tipada(bytes).unwrap_or_default();
    let senha = String::from_utf8_lossy(&utf8_de_texto(senha)).into_owned();
    let r = chave_de(&b, &senha).map(|k| {
        com_contexto(this, |c| c.chave = Some(k));
    });
    texto_ou_nulo(r)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_confiaveis(this: i64, bytes: i64, _senha: i64) -> i64 {
    let b = bytes_da_lista_tipada(bytes).unwrap_or_default();
    texto_ou_nulo(certificados_de(&b).map(|certs| {
        com_contexto(this, |c| {
            c.confiaveis.extend(certs);
            c.raizes = None;
        });
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_cadeia(this: i64, bytes: i64, _senha: i64) -> i64 {
    let b = bytes_da_lista_tipada(bytes).unwrap_or_default();
    texto_ou_nulo(certificados_de(&b).map(|certs| {
        com_contexto(this, |c| c.cadeia = certs);
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_autoridades(this: i64, bytes: i64, _senha: i64) -> i64 {
    let b = bytes_da_lista_tipada(bytes).unwrap_or_default();
    texto_ou_nulo(certificados_de(&b).map(|certs| {
        com_contexto(this, |c| c.autoridades.extend(certs));
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_alpn(this: i64, bytes: i64, servidor: u8) {
    let protocolos = protocolos_de(&bytes_da_lista_tipada(bytes).unwrap_or_default());
    com_contexto(this, |c| {
        if servidor != 0 {
            c.alpn_servidor = protocolos;
        } else {
            c.alpn_cliente = protocolos;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_raizes_embutidas(this: i64) {
    com_contexto(this, |c| {
        c.raizes_embutidas = true;
        c.raizes = None;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_definir_versao_minima(this: i64, versao: i64) {
    com_contexto(this, |c| c.versao_minima = versao);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_contexto_versao_minima(this: i64) -> i64 {
    com_contexto(this, |c| c.versao_minima).unwrap_or(TLS_VERSAO_MINIMA_PADRAO)
}

// ---------------------------------------------------------------------------
// O filtro.

/// A decisão sobre o certificado do servidor que as raízes não validaram.
#[derive(Default)]
struct VerificacaoPendente {
    /// O certificado e o motivo (o texto do BoringSSL), enquanto o Dart não
    /// decide.
    pendente: Option<(rustls::pki_types::CertificateDer<'static>, String)>,
    /// O Dart aceitou o certificado (`onBadCertificate` devolveu `true`).
    aceito: bool,
}

type Verificacao = std::sync::Arc<std::sync::Mutex<VerificacaoPendente>>;

/// O verificador do cliente: as raízes do contexto pelo webpki; o que elas
/// não validam fica para o Dart (ver o topo do arquivo). As assinaturas do
/// handshake são sempre conferidas.
#[derive(Debug)]
struct VerificadorDoDart {
    webpki: Option<std::sync::Arc<rustls::client::WebPkiServerVerifier>>,
    provedor: std::sync::Arc<rustls::crypto::CryptoProvider>,
    estado: Verificacao,
}

impl std::fmt::Debug for VerificacaoPendente {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerificacaoPendente")
    }
}

/// O texto do BoringSSL (`X509_verify_cert_error_string`) de um erro de
/// certificado do rustls.
fn motivo_do_certificado(e: &rustls::Error, autoassinado: bool) -> String {
    use rustls::CertificateError as C;
    match e {
        rustls::Error::InvalidCertificate(c) => match c {
            C::UnknownIssuer if autoassinado => "self signed certificate".into(),
            C::UnknownIssuer => "unable to get local issuer certificate".into(),
            C::Expired | C::ExpiredContext { .. } => "certificate has expired".into(),
            C::NotValidYet | C::NotValidYetContext { .. } => "certificate is not yet valid".into(),
            C::NotValidForName | C::NotValidForNameContext { .. } => "Hostname mismatch".into(),
            C::BadSignature => "certificate signature failure".into(),
            C::Revoked => "certificate revoked".into(),
            outro => format!("{outro:?}"),
        },
        outro => outro.to_string(),
    }
}

impl rustls::client::danger::ServerCertVerifier for VerificadorDoDart {
    fn verify_server_cert(
        &self,
        folha: &rustls::pki_types::CertificateDer<'_>,
        intermediarios: &[rustls::pki_types::CertificateDer<'_>],
        nome: &rustls::pki_types::ServerName<'_>,
        ocsp: &[u8],
        agora: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let resultado = match &self.webpki {
            Some(v) => v.verify_server_cert(folha, intermediarios, nome, ocsp, agora),
            None => Err(rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer)),
        };
        if let Err(e) = resultado {
            let mut estado = self.estado.lock().unwrap_or_else(|e| e.into_inner());
            if !estado.aceito {
                let autoassinado = intermediarios.is_empty() && x509_autoassinado(folha);
                estado.pendente = Some((folha.clone().into_owned(), motivo_do_certificado(&e, autoassinado)));
            }
        }
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        mensagem: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(mensagem, cert, dss, &self.provedor.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        mensagem: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(mensagem, cert, dss, &self.provedor.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provedor.signature_verification_algorithms.supported_schemes()
    }
}

/// O `keyLog` do `SecureSocket`: cada linha no formato NSS fica guardada
/// até o Dart levá-la à porta dele (`DartForge_tls_filtro_chaves_pendentes`).
#[derive(Debug, Default)]
struct RegistroDeChaves(std::sync::Mutex<Vec<String>>);

impl rustls::KeyLog for RegistroDeChaves {
    fn log(&self, rotulo: &str, aleatorio: &[u8], segredo: &[u8]) {
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        let linha = format!("{rotulo} {} {}", hex(aleatorio), hex(segredo));
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(linha);
    }
}

/// O estado nativo de um `_SecureFilterImpl`.
struct FiltroTls {
    conexao: Option<rustls::Connection>,
    servidor: bool,
    verificacao: Verificacao,
    /// O erro do handshake (texto do `OS Error`), relatado pelo próximo
    /// `handshake()`; ou o erro do último processamento.
    erro: Option<String>,
    /// O fim do handshake já foi relatado ao Dart.
    relatado: bool,
    chaves: Option<std::sync::Arc<RegistroDeChaves>>,
}

fn filtro_do_objeto<'a>(obj: i64) -> Option<&'a mut FiltroTls> {
    let p = campo_nativo(obj);
    if p == 0 {
        return None;
    }
    // SAFETY: o campo guarda a `Box` de `DartForge_tls_filtro_novo`, só usada
    // pela thread do isolado dono, e viva até `destroy` ou a coleta.
    Some(unsafe { &mut *(p as *mut FiltroTls) })
}

fn liberar_filtro_tls(p: usize) {
    // SAFETY: a `Box` do filtro, solta uma vez.
    unsafe { drop(Box::from_raw(p as *mut FiltroTls)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_novo(this: i64) {
    let f = Box::new(FiltroTls {
        conexao: None,
        servidor: false,
        verificacao: Verificacao::default(),
        erro: None,
        relatado: false,
        chaves: None,
    });
    let p = Box::into_raw(f) as i64;
    anexar_finalizador(this, liberar_filtro_tls, p as usize);
    gravar_campo_nativo(this, p);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_destruir(this: i64) {
    let p = campo_nativo(this);
    if p != 0 {
        remover_finalizador(this);
        gravar_campo_nativo(this, 0);
        liberar_filtro_tls(p as usize);
    }
}

/// `registerKeyLogPort`: as chaves da sessão passam a ser registradas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_registrar_chaves(this: i64) {
    if let Some(f) = filtro_do_objeto(this) {
        f.chaves = Some(std::sync::Arc::default());
    }
}

/// As linhas do `keyLog` desde a última chamada, separadas por `\n`, ou
/// `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_chaves_pendentes(this: i64) -> i64 {
    let linhas = filtro_do_objeto(this)
        .and_then(|f| f.chaves.as_ref().map(|c| std::mem::take(&mut *c.0.lock().unwrap_or_else(|e| e.into_inner()))))
        .unwrap_or_default();
    if linhas.is_empty() { 0 } else { alocar_str(&linhas.join("\n")) }
}

/// `SecureSocket_Connect`: monta a conexão do rustls. `null`, ou o texto do
/// erro (o Dart lança a `TlsException`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_conectar(
    this: i64,
    host: i64,
    contexto: i64,
    servidor: u8,
    pedir_certificado: u8,
    exigir_certificado: u8,
    protocolos: i64,
) -> i64 {
    let Some(f) = filtro_do_objeto(this) else { return alocar_str("SecureSocket destruído") };
    let host = String::from_utf8_lossy(&utf8_de_texto(host)).into_owned();
    let protocolos = protocolos_de(&bytes_da_lista_tipada(protocolos).unwrap_or_default());
    let Some(contexto) = contexto_do_objeto(contexto) else { return alocar_str("SecurityContext sem estado nativo") };
    let mut c = contexto.lock().unwrap_or_else(|e| e.into_inner());
    let r = if servidor != 0 {
        conectar_servidor(f, &mut c, pedir_certificado != 0, exigir_certificado != 0, protocolos)
    } else {
        conectar_cliente(f, &mut c, &host, protocolos)
    };
    match r {
        Ok(()) => 0,
        Err(e) => alocar_str(&e),
    }
}

fn conectar_cliente(f: &mut FiltroTls, c: &mut ContextoTls, host: &str, protocolos: Vec<Vec<u8>>) -> Result<(), String> {
    let provedor = provedor_tls();
    let raizes = c.raizes();
    let webpki = if raizes.is_empty() {
        None
    } else {
        Some(
            rustls::client::WebPkiServerVerifier::builder_with_provider(raizes, provedor.clone())
                .build()
                .map_err(|e| e.to_string())?,
        )
    };
    let verificador = VerificadorDoDart { webpki, provedor: provedor.clone(), estado: f.verificacao.clone() };
    let base = rustls::ClientConfig::builder_with_provider(provedor)
        .with_protocol_versions(c.versoes())
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(std::sync::Arc::new(verificador));
    let mut config = match (&c.chave, c.cadeia.is_empty()) {
        (Some(k), false) => base.with_client_auth_cert(c.cadeia.clone(), k.clone_key()).map_err(|e| e.to_string())?,
        _ => base.with_no_client_auth(),
    };
    config.alpn_protocols = if protocolos.is_empty() { c.alpn_cliente.clone() } else { protocolos };
    if let Some(c) = &f.chaves {
        config.key_log = c.clone();
    }
    let nome = rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|e| format!("{e}: {host}"))?;
    let conexao = rustls::ClientConnection::new(std::sync::Arc::new(config), nome).map_err(|e| e.to_string())?;
    f.conexao = Some(rustls::Connection::Client(conexao));
    f.servidor = false;
    Ok(())
}

fn conectar_servidor(
    f: &mut FiltroTls,
    c: &mut ContextoTls,
    pedir: bool,
    exigir: bool,
    protocolos: Vec<Vec<u8>>,
) -> Result<(), String> {
    let provedor = provedor_tls();
    let Some(chave) = c.chave.as_ref().map(|k| k.clone_key()) else {
        return Err("\n\tNO_PRIVATE_KEY_ASSIGNED(ssl_lib.cc)".to_string());
    };
    let base = rustls::ServerConfig::builder_with_provider(provedor.clone())
        .with_protocol_versions(c.versoes())
        .map_err(|e| e.to_string())?;
    // O certificado do cliente: validado pelas autoridades do contexto (ou
    // pelas raízes confiáveis), opcional se só foi pedido.
    let mut autoridades = rustls::RootCertStore::empty();
    autoridades.add_parsable_certificates(c.autoridades.iter().cloned());
    if autoridades.is_empty() {
        autoridades = (*c.raizes()).clone();
    }
    let base = if (pedir || exigir) && !autoridades.is_empty() {
        let b = rustls::server::WebPkiClientVerifier::builder_with_provider(std::sync::Arc::new(autoridades), provedor);
        let b = if exigir { b } else { b.allow_unauthenticated() };
        base.with_client_cert_verifier(b.build().map_err(|e| e.to_string())?)
    } else {
        base.with_no_client_auth()
    };
    let mut config = base.with_single_cert(c.cadeia.clone(), chave).map_err(|e| e.to_string())?;
    config.alpn_protocols = if protocolos.is_empty() { c.alpn_servidor.clone() } else { protocolos };
    if let Some(c) = &f.chaves {
        config.key_log = c.clone();
    }
    let conexao = rustls::ServerConnection::new(std::sync::Arc::new(config)).map_err(|e| e.to_string())?;
    f.conexao = Some(rustls::Connection::Server(conexao));
    f.servidor = true;
    Ok(())
}

/// O `SSLFilter::Handshake` da VM: 0 em andamento, 1 terminou agora (o Dart
/// chama o `handshakeCompleteHandler`), 16 o certificado espera a decisão
/// do Dart, -1 erro (`DartForge_tls_filtro_erro`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_handshake(this: i64) -> i64 {
    let Some(f) = filtro_do_objeto(this) else { return -1 };
    if f.erro.is_some() {
        return -1;
    }
    if f.verificacao.lock().unwrap_or_else(|e| e.into_inner()).pendente.is_some() {
        return TLS_ESPERA_VERIFICACAO;
    }
    match &f.conexao {
        Some(c) if !c.is_handshaking() && !f.relatado => {
            f.relatado = true;
            1
        }
        Some(_) => 0,
        None => {
            f.erro = Some("\n\tSecureSocket sem conexão".to_string());
            -1
        }
    }
}

/// O texto do erro (o `OS Error`), ou `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_erro(this: i64) -> i64 {
    match filtro_do_objeto(this).and_then(|f| f.erro.clone()) {
        Some(e) => alocar_str(&e),
        None => 0,
    }
}

/// O certificado que espera a decisão, para o `onBadCertificate`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_certificado_pendente(this: i64) -> i64 {
    let Some(f) = filtro_do_objeto(this) else { return 0 };
    let v = f.verificacao.lock().unwrap_or_else(|e| e.into_inner());
    v.pendente.as_ref().map_or(0, |(c, _)| x509_novo(c.clone()))
}

/// `_markAsTrusted`: a decisão do Dart sobre o certificado pendente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_decidir(this: i64, confiavel: u8) {
    let Some(f) = filtro_do_objeto(this) else { return };
    let mut v = f.verificacao.lock().unwrap_or_else(|e| e.into_inner());
    let Some((_, motivo)) = v.pendente.take() else { return };
    if confiavel != 0 {
        v.aceito = true;
    } else {
        drop(v);
        f.erro = Some(format!("\n\tCERTIFICATE_VERIFY_FAILED: {motivo}(handshake.cc:392)"));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_protocolo(this: i64) -> i64 {
    match filtro_do_objeto(this).and_then(|f| f.conexao.as_ref()?.alpn_protocol().map(<[u8]>::to_vec)) {
        Some(p) => dart_texto_de_bytes(&p),
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_certificado_do_par(this: i64) -> i64 {
    let Some(f) = filtro_do_objeto(this) else { return 0 };
    match f.conexao.as_ref().and_then(|c| c.peer_certificates()).and_then(|c| c.first()) {
        Some(c) => x509_novo(c.clone().into_owned()),
        None => 0,
    }
}

/// Os anéis: texto lido, texto a escrever, cifrado lido, cifrado a escrever
/// (`_RawSecureSocket.readPlaintextId`…).
const ANEL_TEXTO_LIDO: usize = 0;
const ANEL_TEXTO_A_ESCREVER: usize = 1;
const ANEL_CIFRADO_LIDO: usize = 2;
const ANEL_CIFRADO_A_ESCREVER: usize = 3;

/// Consome os dados do anel `[inicio, fim)` (a leitura do
/// `ProcessAllBuffers`): `f` recebe um trecho e devolve quanto consumiu.
fn anel_consumir(buf: &[u8], inicio: &mut usize, fim: usize, f: &mut dyn FnMut(&[u8]) -> usize) {
    let tamanho = buf.len();
    let mut trecho = |de: usize, ate: usize| {
        let mut feito = 0;
        while de + feito < ate {
            let n = f(&buf[de + feito..ate]);
            if n == 0 {
                break;
            }
            feito += n;
        }
        feito
    };
    if fim < *inicio {
        *inicio += trecho(*inicio, tamanho);
        if *inicio == tamanho {
            *inicio = 0;
        }
    }
    if *inicio < fim {
        *inicio += trecho(*inicio, fim);
    }
}

/// Enche o espaço livre do anel depois de `fim` (a escrita do
/// `ProcessAllBuffers`: um byte fica sempre livre, para cheio ≠ vazio).
fn anel_encher(buf: &mut [u8], inicio: usize, fim: &mut usize, f: &mut dyn FnMut(&mut [u8]) -> usize) {
    let tamanho = buf.len();
    let mut trecho = |buf: &mut [u8], de: usize, ate: usize| {
        let mut feito = 0;
        while de + feito < ate {
            let n = f(&mut buf[de + feito..ate]);
            if n == 0 {
                break;
            }
            feito += n;
        }
        feito
    };
    if inicio <= *fim {
        let limite = if inicio == 0 { tamanho - 1 } else { tamanho };
        *fim += trecho(buf, *fim, limite);
        if *fim == tamanho {
            *fim = 0;
        }
    }
    if inicio > *fim + 1 {
        *fim += trecho(buf, *fim, inicio - 1);
    }
}

/// O `ProcessAllBuffers` da VM sobre os quatro anéis (`b0`…`b3`, as
/// `Uint8List` dos `_ExternalBuffer`) e as posições (`pos`, uma `Int64List`
/// com `[início, fim]` de cada anel, atualizada no lugar). 0, ou o código do
/// erro fatal depois do handshake (o texto em `DartForge_tls_filtro_erro`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_filtro_processar(
    this: i64,
    em_handshake: u8,
    b0: i64,
    b1: i64,
    b2: i64,
    b3: i64,
    pos: i64,
) -> i64 {
    let Some(f) = filtro_do_objeto(this) else { return 0 };
    let listas = [b0, b1, b2, b3];
    // Os bytes dos anéis e das posições saem do heap durante o
    // processamento (nada aqui chama o Dart nem aloca no heap) e voltam
    // depois, sem cópia.
    let mut aneis: [Vec<u8>; 4] = std::array::from_fn(|i| tomar_bytes_tipados(listas[i]));
    let mut bytes_pos = tomar_bytes_tipados(pos);
    let mut p = [0usize; 8];
    for (i, v) in p.iter_mut().enumerate() {
        let mut b = [0u8; 8];
        b.copy_from_slice(&bytes_pos[i * 8..i * 8 + 8]);
        *v = i64::from_ne_bytes(b).max(0) as usize;
    }
    let r = processar_aneis(f, em_handshake != 0, &mut aneis, &mut p);
    for (i, v) in p.iter().enumerate() {
        bytes_pos[i * 8..i * 8 + 8].copy_from_slice(&(*v as i64).to_ne_bytes());
    }
    for (i, a) in aneis.into_iter().enumerate() {
        devolver_bytes_tipados(listas[i], a);
    }
    devolver_bytes_tipados(pos, bytes_pos);
    match r {
        Ok(()) => 0,
        Err((codigo, texto)) => {
            f.erro = Some(texto);
            codigo
        }
    }
}

fn processar_aneis(f: &mut FiltroTls, em_handshake: bool, aneis: &mut [Vec<u8>; 4], p: &mut [usize; 8]) -> Result<(), (i64, String)> {
    use std::io::{Read, Write};
    let Some(conexao) = f.conexao.as_mut() else { return Ok(()) };
    for (i, a) in aneis.iter().enumerate() {
        let tamanho = a.len();
        if tamanho == 0 || p[2 * i] >= tamanho || p[2 * i + 1] >= tamanho {
            return Err((1, "Out-of-bounds internal buffer access in dart:io SecureSocket".to_string()));
        }
    }
    // Cifrado lido → TLS.
    let (mut inicio, fim) = (p[2 * ANEL_CIFRADO_LIDO], p[2 * ANEL_CIFRADO_LIDO + 1]);
    anel_consumir(&aneis[ANEL_CIFRADO_LIDO], &mut inicio, fim, &mut |trecho| {
        let mut leitor = trecho;
        conexao.read_tls(&mut leitor).unwrap_or(0)
    });
    let houve_leitura = inicio != p[2 * ANEL_CIFRADO_LIDO];
    p[2 * ANEL_CIFRADO_LIDO] = inicio;
    if (houve_leitura || em_handshake)
        && f.erro.is_none()
        && let Err(e) = conexao.process_new_packets()
    {
        let texto = format!("\n\t{e}");
        if em_handshake {
            // Relatado pelo próximo `handshake()`, como o erro do
            // `SSL_do_handshake`; o alerta ainda sai para o par.
            f.erro = Some(texto);
        } else {
            return Err((1, texto));
        }
    }
    let bloqueado = f.verificacao.lock().unwrap_or_else(|e| e.into_inner()).pendente.is_some();
    if !em_handshake && !bloqueado && f.erro.is_none() {
        // TLS → texto lido.
        let (inicio, mut fim) = (p[2 * ANEL_TEXTO_LIDO], p[2 * ANEL_TEXTO_LIDO + 1]);
        let mut erro = None;
        anel_encher(&mut aneis[ANEL_TEXTO_LIDO], inicio, &mut fim, &mut |trecho| match conexao.reader().read(trecho) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => 0,
            Err(e) => {
                erro = Some(e);
                0
            }
        });
        p[2 * ANEL_TEXTO_LIDO + 1] = fim;
        if let Some(e) = erro
            && e.kind() != std::io::ErrorKind::UnexpectedEof
        {
            return Err((1, format!("\n\t{e}")));
        }
        // Texto a escrever → TLS.
        let (mut inicio, fim) = (p[2 * ANEL_TEXTO_A_ESCREVER], p[2 * ANEL_TEXTO_A_ESCREVER + 1]);
        anel_consumir(&aneis[ANEL_TEXTO_A_ESCREVER], &mut inicio, fim, &mut |trecho| conexao.writer().write(trecho).unwrap_or(0));
        p[2 * ANEL_TEXTO_A_ESCREVER] = inicio;
    }
    if !bloqueado {
        // TLS → cifrado a escrever.
        let (inicio, mut fim) = (p[2 * ANEL_CIFRADO_A_ESCREVER], p[2 * ANEL_CIFRADO_A_ESCREVER + 1]);
        anel_encher(&mut aneis[ANEL_CIFRADO_A_ESCREVER], inicio, &mut fim, &mut |trecho| {
            let mut escritor = trecho;
            conexao.write_tls(&mut escritor).unwrap_or(0)
        });
        p[2 * ANEL_CIFRADO_A_ESCREVER + 1] = fim;
    }
    Ok(())
}

/// Tira os bytes de uma lista tipada interna do heap (a lista fica vazia
/// até [`devolver_bytes_tipados`]).
fn tomar_bytes_tipados(h: i64) -> Vec<u8> {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if !matches!(heap.try_get(h), Some(Value::TypedData { .. })) {
            return Vec::new();
        }
        std::mem::take(bytes_de_mut(&mut heap, h))
    })
}

fn devolver_bytes_tipados(h: i64, b: Vec<u8>) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if matches!(heap.try_get(h), Some(Value::TypedData { .. })) {
            *bytes_de_mut(&mut heap, h) = b;
        }
    });
}

// ---------------------------------------------------------------------------
// X509Certificate.

/// Um certificado (`_X509CertificateImpl`): o DER, com contagem de
/// referências (o objeto Dart e quem o entrega).
type CertificadoX509 = std::sync::Arc<rustls::pki_types::CertificateDer<'static>>;

/// Um ponteiro com uma referência do certificado, para o Dart adotar
/// (`DartForge_tls_x509_adotar`).
fn x509_novo(c: rustls::pki_types::CertificateDer<'static>) -> i64 {
    std::sync::Arc::into_raw(CertificadoX509::new(c)) as i64
}

fn liberar_x509(p: usize) {
    // SAFETY: a referência do objeto, solta uma vez.
    unsafe { drop(std::sync::Arc::from_raw(p as *const rustls::pki_types::CertificateDer<'static>)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_adotar(this: i64, p: i64) {
    if p != 0 {
        anexar_finalizador(this, liberar_x509, p as usize);
        gravar_campo_nativo(this, p);
    }
}

fn der_do_x509(this: i64) -> Vec<u8> {
    let p = campo_nativo(this);
    if p == 0 {
        return Vec::new();
    }
    // SAFETY: a referência do objeto está viva até a coleta dele.
    unsafe { (*(p as *const rustls::pki_types::CertificateDer<'static>)).to_vec() }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_der(this: i64) -> i64 {
    dart_bytes(der_do_x509(this))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_pem(this: i64) -> i64 {
    let b64 = base64_padrao(&der_do_x509(this));
    let mut pem = String::from("-----BEGIN CERTIFICATE-----\n");
    for linha in b64.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(linha).unwrap_or_default());
        pem.push('\n');
    }
    pem.push_str("-----END CERTIFICATE-----\n");
    alocar_str(&pem)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_sha1(this: i64) -> i64 {
    let d = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &der_do_x509(this));
    dart_bytes(d.as_ref().to_vec())
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_sujeito(this: i64) -> i64 {
    let der = der_do_x509(this);
    alocar_str(&x509_campos(&der).map(|c| nome_x509(c.sujeito)).unwrap_or_default())
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_emissor(this: i64) -> i64 {
    let der = der_do_x509(this);
    alocar_str(&x509_campos(&der).map(|c| nome_x509(c.emissor)).unwrap_or_default())
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_inicio(this: i64) -> i64 {
    let der = der_do_x509(this);
    x509_campos(&der).and_then(|c| instante_x509(c.inicio)).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_tls_x509_fim(this: i64) -> i64 {
    let der = der_do_x509(this);
    x509_campos(&der).and_then(|c| instante_x509(c.fim)).unwrap_or(0)
}

fn base64_padrao(b: &[u8]) -> String {
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
fn der_elemento(b: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let etiqueta = *b.first()?;
    let primeiro = *b.get(1)?;
    let (tamanho, cabecalho) = if primeiro < 0x80 {
        (primeiro as usize, 2)
    } else {
        let n = (primeiro & 0x7f) as usize;
        if n == 0 || n > 4 {
            return None;
        }
        let mut t = 0usize;
        for i in 0..n {
            t = (t << 8) | *b.get(2 + i)? as usize;
        }
        (t, 2 + n)
    };
    let fim = cabecalho.checked_add(tamanho)?;
    Some((etiqueta, b.get(cabecalho..fim)?, &b[fim..]))
}

/// Os campos do `tbsCertificate` que o `X509Certificate` mostra.
struct CamposX509<'a> {
    emissor: &'a [u8],
    inicio: (u8, &'a [u8]),
    fim: (u8, &'a [u8]),
    sujeito: &'a [u8],
}

fn x509_campos(der: &[u8]) -> Option<CamposX509<'_>> {
    let (_, cert, _) = der_elemento(der)?;
    let (_, tbs, _) = der_elemento(cert)?;
    let (mut etiqueta, _, mut resto) = der_elemento(tbs)?;
    // `[0] version` é opcional.
    if etiqueta == 0xa0 {
        (etiqueta, _, resto) = der_elemento(resto)?;
    }
    let _ = etiqueta; // serialNumber
    let (_, _, resto) = der_elemento(resto)?; // signature
    let (_, emissor, resto) = der_elemento(resto)?;
    let (_, validade, resto) = der_elemento(resto)?;
    let (_, sujeito, _) = der_elemento(resto)?;
    let (e1, inicio, r) = der_elemento(validade)?;
    let (e2, fim, _) = der_elemento(r)?;
    Some(CamposX509 { emissor, inicio: (e1, inicio), fim: (e2, fim), sujeito })
}

fn x509_autoassinado(c: &rustls::pki_types::CertificateDer<'_>) -> bool {
    x509_campos(c).is_some_and(|c| c.emissor == c.sujeito)
}

/// O nome no formato de `X509_NAME_oneline` do BoringSSL:
/// `/C=US/O=Let's Encrypt/CN=R3`.
fn nome_x509(nome: &[u8]) -> String {
    let mut s = String::new();
    let mut rdns = nome;
    while let Some((_, rdn, resto)) = der_elemento(rdns) {
        rdns = resto;
        let mut atributos = rdn;
        while let Some((_, atributo, resto)) = der_elemento(atributos) {
            atributos = resto;
            let Some((_, oid, r)) = der_elemento(atributo) else { continue };
            let Some((etiqueta, valor, _)) = der_elemento(r) else { continue };
            s.push('/');
            s.push_str(&nome_do_oid(oid));
            s.push('=');
            let texto = if etiqueta == 0x1e {
                // BMPString (UCS-2 big endian).
                String::from_utf16_lossy(&valor.chunks(2).map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)])).collect::<Vec<_>>())
            } else {
                String::from_utf8_lossy(valor).into_owned()
            };
            for ch in texto.chars() {
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

fn nome_do_oid(oid: &[u8]) -> String {
    let conhecido = match oid {
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
    if !conhecido.is_empty() {
        return conhecido.to_string();
    }
    // O OID em pontos.
    let mut partes = Vec::new();
    if let Some(&p) = oid.first() {
        partes.push((p / 40).min(2) as u64);
        partes.push(u64::from(p) - 40 * (p / 40).min(2) as u64);
    }
    let mut v = 0u64;
    for &b in oid.iter().skip(1) {
        v = (v << 7) | u64::from(b & 0x7f);
        if b & 0x80 == 0 {
            partes.push(v);
            v = 0;
        }
    }
    partes.iter().map(u64::to_string).collect::<Vec<_>>().join(".")
}

/// Milissegundos desde a época de um `UTCTime` (0x17) ou `GeneralizedTime`
/// (0x18) em UTC.
fn instante_x509((etiqueta, t): (u8, &[u8])) -> Option<i64> {
    let s = std::str::from_utf8(t).ok()?;
    let digitos = |de: usize, n: usize| -> Option<i64> { s.get(de..de + n)?.parse().ok() };
    let (ano, resto) = if etiqueta == 0x17 {
        let a = digitos(0, 2)?;
        (if a < 50 { 2000 + a } else { 1900 + a }, 2)
    } else {
        (digitos(0, 4)?, 4)
    };
    let mes = digitos(resto, 2)?;
    let dia = digitos(resto + 2, 2)?;
    let hora = digitos(resto + 4, 2)?;
    let minuto = digitos(resto + 6, 2)?;
    let segundo = digitos(resto + 8, 2).unwrap_or(0);
    // Dias desde 1970-01-01 (algoritmo de Howard Hinnant).
    let y = if mes <= 2 { ano - 1 } else { ano };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if mes > 2 { mes - 3 } else { mes + 9 }) + 2) / 5 + dia - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let dias = era * 146097 + doe - 719468;
    Some(((dias * 24 + hora) * 60 + minuto) * 60_000 + segundo * 1000)
}
