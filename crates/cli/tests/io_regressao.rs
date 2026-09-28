//! Regressões do `dart:io` nativo que o corpus diferencial não cobre (a VM
//! não serve de referência ou o defeito só aparece sob carga): cada
//! programa é compilado pelo AOT e tem de terminar, no prazo, com a saída
//! esperada.
#![cfg(feature = "nativo")]

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Compila `fixtures/<nome>.dart` com o SDK da fonte, roda (com `args`) e
/// compara a saída; falha se o programa não termina em `prazo`.
fn compilar_e_rodar(nome: &str, args: &[&std::ffi::OsStr], esperado: &str, prazo: Duration) {
    compilar_e_rodar_com_ambiente(nome, args, &[], esperado, prazo);
}

/// [`compilar_e_rodar`] com variáveis de ambiente para o programa.
fn compilar_e_rodar_com_ambiente(nome: &str, args: &[&std::ffi::OsStr], ambiente: &[(&str, &str)], esperado: &str, prazo: Duration) {
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = raiz.join(format!("../../target/tmp-io-regressao-{}-{nome}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("diretório do teste");
    let exe = dir.join(if cfg!(windows) { format!("{nome}.exe") } else { nome.to_string() });
    let compilacao = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("compile-native")
        .arg(raiz.join("tests/fixtures").join(format!("{nome}.dart")))
        .arg("-o")
        .arg(&exe)
        .env("DARTFORGE_SDK_DA_FONTE", "1")
        .output()
        .expect("CLI");
    assert!(compilacao.status.success(), "compilação de {nome}: {}", String::from_utf8_lossy(&compilacao.stderr));

    let mut filho = Command::new(&exe).args(args).envs(ambiente.iter().copied()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("programa");
    let inicio = Instant::now();
    let status = loop {
        if let Some(s) = filho.try_wait().expect("estado do programa") {
            break Some(s);
        }
        if inicio.elapsed() > prazo {
            let _ = filho.kill();
            let _ = filho.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut saida = String::new();
    let mut erro = String::new();
    filho.stdout.take().expect("stdout").read_to_string(&mut saida).expect("saída");
    filho.stderr.take().expect("stderr").read_to_string(&mut erro).expect("erro");
    let _ = std::fs::remove_dir_all(&dir);
    let status = status.unwrap_or_else(|| panic!("{nome} não terminou em {prazo:?} (travou)\nsaída: {saida}\nerro: {erro}"));
    assert!(status.success(), "{nome} saiu com {status}\nsaída: {saida}\nerro: {erro}");
    assert_eq!(saida.replace("\r\n", "\n"), esperado, "{nome}\nerro: {erro}");
}

#[test]
#[ignore = "compila com o SDK da fonte e liga com o Clang; roda no CI de cada sistema"]
fn soquete_de_escuta_compartilhado_so_fecha_com_o_ultimo() {
    compilar_e_rodar("escuta_compartilhada", &[], "o compartilhado sobrevive: ok\no ultimo fecha: ok\n", Duration::from_secs(60));
}

#[test]
#[ignore = "compila com o SDK da fonte e liga com o Clang; roda no CI de cada sistema"]
fn rajada_de_sinais_nao_trava_o_tratador() {
    compilar_e_rodar("rajada_de_sinais", &[], "rajada: ok\nreinscricao: ok\n", Duration::from_secs(60));
}

#[test]
#[ignore = "compila com o SDK da fonte e liga com o Clang; roda no CI de cada sistema"]
fn tls_e_https_como_a_vm() {
    let certificados = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    compilar_e_rodar(
        "tls_https",
        &[certificados.as_os_str()],
        "protocolo: http/1.1
sujeito: /C=BR/O=DartForge/CN=localhost
emissor: /C=BR/O=DartForge Teste/CN=CA de Teste
validade: true
der: true sha1: 20
pem: true
eco: olá TLS

recusado: HandshakeException
true
servidor: HandshakeException
aceito pelo callback: /C=BR/O=DartForge/CN=localhost
eco: de novo

grande: 11
200 application/json; charset=utf-8 {\"metodo\":\"POST\",\"caminho\":\"/a\",\"corpo\":50000,\"https\":\"https\"}
200 application/json; charset=utf-8 {\"metodo\":\"POST\",\"caminho\":\"/b/c\",\"corpo\":50000,\"https\":\"https\"}
callback: localhost /C=BR/O=DartForge Teste/CN=CA de Teste
inseguro: 200 {\"metodo\":\"GET\",\"caminho\":\"/i\",\"corpo\":0,\"https\":\"https\"}
recusado: CERTIFICATE_VERIFY_FAILED: unable to get local issuer certificate(handshake.cc:392)
",
        Duration::from_secs(120),
    );
}

/// Os formatos de chave e certificado (PKCS#12, chaves cifradas) e as recusas
/// com os textos da VM; a saída é a do `dart run` 3.6.2.
#[test]
#[ignore = "compila com o SDK da fonte e liga com o Clang; roda no CI de cada sistema"]
fn formatos_de_chave_e_certificado_como_a_vm() {
    let certificados = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    compilar_e_rodar(
        "tls_formatos",
        &[certificados.as_os_str()],
        "pkcs8 cifrada: ok
pkcs8 senha errada: ArgumentError Invalid argument(s): Expected private key, but none was found
pkcs8 sem senha: ArgumentError Invalid argument(s): Expected private key, but none was found
rsa cifrada: ok
rsa senha errada: ArgumentError Invalid argument(s): Expected private key, but none was found
p12 chave: ok
p12 cadeia: ok
p12 confiaveis: ok
p12 autoridades: ok
p12 senha errada: ArgumentError Invalid argument(s): Expected private key, but none was found
p12 sem senha: ArgumentError Invalid argument(s): Expected private key, but none was found
p12 legado: ok
p12 cadeia senha errada: TlsException Failure in useCertificateChainBytes | 
\tINCORRECT_PASSWORD(pkcs8_x509.c:710)
p12 confiaveis senha errada: TlsException Failure trusting builtin roots | 
\tINCORRECT_PASSWORD(pkcs8_x509.c:710)
cadeia lixo: TlsException Failure in useCertificateChainBytes | 
\tBAD_PKCS12_DATA(pkcs8_x509.c:599)
confiaveis lixo: TlsException Failure trusting builtin roots | 
\tBAD_PKCS12_DATA(pkcs8_x509.c:599)
lixo: ArgumentError Invalid argument(s): Expected private key, but none was found
autoridades lixo: TlsException Failure in setClientAuthoritiesBytes | 
\tBAD_PKCS12_DATA(pkcs8_x509.c:599)
autoridades p12 senha errada: TlsException Failure in setClientAuthoritiesBytes | 
\tINCORRECT_PASSWORD(pkcs8_x509.c:710)
cadeia der: TlsException Failure in useCertificateChainBytes | 
\tBAD_PKCS12_DATA(pkcs8_x509.c:611)
confiaveis der: TlsException Failure trusting builtin roots | 
\tBAD_PKCS12_DATA(pkcs8_x509.c:611)
pem quebrado: TlsException Failure trusting builtin roots | 
\tBAD_BASE64_DECODE(pem_lib.c:752)
chave der: ArgumentError Invalid argument(s): Expected private key, but none was found
chave em pem de certificado: ArgumentError Invalid argument(s): Expected private key, but none was found
p12 vazio sem senha: ok
p12 vazio cadeia: ok
p12: /C=BR/O=DartForge/CN=localhost -> ola p12
pkcs8 cifrada: /C=BR/O=DartForge/CN=localhost -> ola pkcs8 cifrada
rsa cifrada: /C=BR/O=DartForge/CN=localhost -> ola rsa cifrada
",
        Duration::from_secs(120),
    );
}

/// O contrato de `allowLegacyUnsafeRenegotiation` (o rustls não renegocia):
/// sem a opção, a recusa é a da VM; com ela, a VM renegociaria e aqui o
/// pedido é recusado igual (documentado em NATIVO-PLANO). Precisa do
/// `openssl` do OpenSSL (o `s_server -www` pede a renegociação ao receber
/// `GET /reneg`; a entrada padrão dele não serve, porque no Windows o
/// `s_server` bloqueia nela antes do aperto de mão); sem ele, o teste não
/// roda.
#[test]
#[ignore = "compila com o SDK da fonte e liga com o Clang; roda no CI de cada sistema"]
fn renegociacao_legada_e_recusada() {
    // O OpenSSL (o LibreSSL do macOS não tem `-legacy_renegotiation`).
    let versao = Command::new("openssl").arg("version").output().map(|s| String::from_utf8_lossy(&s.stdout).into_owned()).unwrap_or_default();
    if !versao.starts_with("OpenSSL") {
        eprintln!("sem o openssl do OpenSSL ({}): teste de renegociação não roda", versao.trim());
        return;
    }
    let certificados = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    compilar_e_rodar(
        "tls_renegociacao",
        &[certificados.as_os_str()],
        "controle: recebido: HTTP/1.0 200 ok\npadrão: erro: TlsException\ncom a opção: erro: TlsException\n",
        // Cada etapa do programa tem prazo próprio (a soma no pior caso das
        // três rodadas fica abaixo disto): travar aqui é o laço de eventos.
        Duration::from_secs(300),
    );
}

#[test]
#[ignore = "compila com o SDK da fonte e liga com o Clang; roda no CI de cada sistema"]
fn main_recebe_os_argumentos() {
    let args = [std::ffi::OsStr::new("um"), std::ffi::OsStr::new("dois três")];
    compilar_e_rodar("argumentos_do_main", &args, "[um, dois três]\ntrue\nfixa\n", Duration::from_secs(60));
}

#[test]
#[ignore = "compila com o SDK da fonte e liga com o Clang; roda no CI de cada sistema"]
fn heap_perto_do_teto_nao_coleta_a_cada_alocacao() {
    compilar_e_rodar_com_ambiente(
        "pressao_do_heap",
        &[],
        &[("DARTFORGE_HEAP_MAX_MB", "64")],
        "9000 45000150000\n",
        Duration::from_secs(60),
    );
}
