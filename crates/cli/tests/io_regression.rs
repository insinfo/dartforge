//! Regressions of the native `dart:io` that the differential corpus does not
//! cover (the VM does not serve as a reference, or the defect only shows up
//! under load): each program is compiled by AOT and has to finish, within
//! the deadline, with the expected output.
#![cfg(feature = "native")]

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Compiles `fixtures/<name>.dart` with the source-built SDK, runs it (with
/// `args`) and compares the output; fails if the program does not finish
/// within `deadline`.
fn compile_and_run(name: &str, args: &[&std::ffi::OsStr], expected: &str, deadline: Duration) {
    compile_and_run_with_env(name, args, &[], expected, deadline);
}

/// [`compile_and_run`] with environment variables for the program.
fn compile_and_run_with_env(
    name: &str,
    args: &[&std::ffi::OsStr],
    env: &[(&str, &str)],
    expected: &str,
    deadline: Duration,
) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join(format!(
        "../../target/tmp-io-regressao-{}-{name}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("test directory");
    let exe = dir.join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    });
    let compilation = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("compile-native")
        .arg(root.join("tests/fixtures").join(format!("{name}.dart")))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("CLI");
    assert!(
        compilation.status.success(),
        "compilation of {name}: {}",
        String::from_utf8_lossy(&compilation.stderr)
    );

    let mut child = Command::new(&exe)
        .args(args)
        .envs(env.iter().copied())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("program");
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().expect("program status") {
            break Some(s);
        }
        if start.elapsed() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    child
        .stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut stdout)
        .expect("stdout");
    child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut stderr)
        .expect("stderr");
    let _ = std::fs::remove_dir_all(&dir);
    let status = status.unwrap_or_else(|| {
        panic!(
            "{name} did not finish within {deadline:?} (hung)\nstdout: {stdout}\nstderr: {stderr}"
        )
    });
    assert!(
        status.success(),
        "{name} exited with {status}\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        expected,
        "{name}\nstderr: {stderr}"
    );
}

#[test]
#[ignore = "compiles with the source-built SDK and links with Clang; runs in the CI of each system"]
fn shared_listening_socket_only_closes_with_the_last_one() {
    compile_and_run(
        "shared_listener",
        &[],
        "o compartilhado sobrevive: ok\no ultimo fecha: ok\n",
        Duration::from_secs(60),
    );
}

#[test]
#[ignore = "compiles with the source-built SDK and links with Clang; runs in the CI of each system"]
fn signal_burst_does_not_hang_the_handler() {
    compile_and_run(
        "signal_burst",
        &[],
        "rajada: ok\nreinscricao: ok\n",
        Duration::from_secs(60),
    );
}

#[test]
#[ignore = "compiles with the source-built SDK and links with Clang; runs in the CI of each system"]
fn tls_and_https_like_the_vm() {
    let certificates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    compile_and_run(
        "tls_https",
        &[certificates.as_os_str()],
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

/// The key and certificate formats (PKCS#12, encrypted keys) and the
/// rejections with the VM's texts; the output is that of `dart run` 3.6.2.
#[test]
#[ignore = "compiles with the source-built SDK and links with Clang; runs in the CI of each system"]
fn key_and_certificate_formats_like_the_vm() {
    let certificates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    compile_and_run(
        "tls_formatos",
        &[certificates.as_os_str()],
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

/// The contract of `allowLegacyUnsafeRenegotiation` (rustls does not
/// renegotiate): without the option, the rejection is the VM's; with it, the
/// VM would renegotiate and here the request is rejected all the same
/// (documented in NATIVO-PLANO). Needs OpenSSL's `openssl` (`s_server -www`
/// requests renegotiation on receiving `GET /reneg`; its standard input is of
/// no use, because on Windows `s_server` blocks on it before the handshake);
/// without it, the test does not run.
#[test]
#[ignore = "compiles with the source-built SDK and links with Clang; runs in the CI of each system"]
fn legacy_renegotiation_is_rejected() {
    // OpenSSL (macOS's LibreSSL has no `-legacy_renegotiation`).
    let version = Command::new("openssl")
        .arg("version")
        .output()
        .map(|s| String::from_utf8_lossy(&s.stdout).into_owned())
        .unwrap_or_default();
    if !version.starts_with("OpenSSL") {
        eprintln!(
            "no OpenSSL openssl ({}): renegotiation test does not run",
            version.trim()
        );
        return;
    }
    let certificates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    compile_and_run(
        "tls_renegociacao",
        &[certificates.as_os_str()],
        "controle: recebido: HTTP/1.0 200 ok\npadrão: erro: TlsException\ncom a opção: erro: TlsException\n",
        // Each step of the program has its own deadline (the worst-case sum of
        // the three rounds stays below this): hanging here is the event loop.
        Duration::from_secs(300),
    );
}

#[test]
#[ignore = "compiles with the source-built SDK and links with Clang; runs in the CI of each system"]
fn main_receives_the_arguments() {
    let args = [
        std::ffi::OsStr::new("one"),
        std::ffi::OsStr::new("two three"),
    ];
    compile_and_run(
        "main_arguments",
        &args,
        "[one, two three]\ntrue\nfixa\n",
        Duration::from_secs(60),
    );
}

#[test]
#[ignore = "compiles with the source-built SDK and links with Clang; runs in the CI of each system"]
fn heap_near_the_ceiling_does_not_collect_on_every_allocation() {
    compile_and_run_with_env(
        "heap_pressure",
        &[],
        &[("DARTFORGE_HEAP_MAX_MB", "64")],
        "9000 45000150000\n",
        Duration::from_secs(60),
    );
}
