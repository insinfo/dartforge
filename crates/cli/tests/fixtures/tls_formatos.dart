// Regressão: os formatos de chave e certificado do `SecurityContext`
// (crates/runtime/src/tls_formatos.rs), como a VM: PEM, PKCS#8 cifrado, PEM
// cifrado legado, PKCS#12 (PBES2, legado RC2/3DES, sem senha), senha errada,
// lixo e DER solto (a VM não aceita); e um aperto de mão com o servidor
// carregado do PKCS#12 e da chave cifrada. Os arquivos estão em tls/.
import 'dart:async';
import 'dart:convert';
import 'dart:io';

void tentar(String nome, void Function(SecurityContext) f) {
  try {
    f(SecurityContext());
    print('$nome: ok');
  } on TlsException catch (e) {
    print('$nome: ${e.runtimeType} ${e.message} | ${e.osError?.message}');
  } catch (e) {
    print('$nome: ${e.runtimeType} $e');
  }
}

/// Um servidor com `contexto`, e um cliente que confia em `confianca`:
/// o eco de uma linha.
Future<void> aperto(String nome, SecurityContext contexto, SecurityContext confianca) async {
  final server = await SecureServerSocket.bind('127.0.0.1', 0, contexto);
  server.listen((s) {
    s.listen((dados) async {
      s.add(dados);
      await s.flush();
      await s.close();
    }, onError: (_) {});
  }, onError: (_) {});
  final s = await SecureSocket.connect('localhost', server.port, context: confianca);
  s.write('ola $nome');
  final resposta = await s.cast<List<int>>().transform(utf8.decoder).join();
  print('$nome: ${s.peerCertificate!.subject} -> $resposta');
  await s.close();
  await server.close();
}

Future<void> main(List<String> a) async {
  final d = a[0];
  tentar('pkcs8 cifrada', (c) => c.usePrivateKey('$d/srv_pkcs8_cifrada.key', password: 'segredo'));
  tentar('pkcs8 senha errada', (c) => c.usePrivateKey('$d/srv_pkcs8_cifrada.key', password: 'x'));
  tentar('pkcs8 sem senha', (c) => c.usePrivateKey('$d/srv_pkcs8_cifrada.key'));
  tentar('rsa cifrada', (c) => c.usePrivateKey('$d/srv_rsa_cifrada.key', password: 'segredo'));
  tentar('rsa senha errada', (c) => c.usePrivateKey('$d/srv_rsa_cifrada.key', password: 'x'));
  tentar('p12 chave', (c) => c.usePrivateKey('$d/srv.p12', password: 'segredo'));
  tentar('p12 cadeia', (c) => c.useCertificateChain('$d/srv.p12', password: 'segredo'));
  tentar('p12 confiaveis', (c) => c.setTrustedCertificates('$d/srv.p12', password: 'segredo'));
  tentar('p12 autoridades', (c) => c.setClientAuthorities('$d/srv.p12', password: 'segredo'));
  tentar('p12 senha errada', (c) => c.usePrivateKey('$d/srv.p12', password: 'x'));
  tentar('p12 sem senha', (c) => c.usePrivateKey('$d/srv.p12'));
  tentar('p12 legado', (c) => c.usePrivateKey('$d/srv_legado.p12', password: 'segredo'));
  tentar('p12 cadeia senha errada', (c) => c.useCertificateChain('$d/srv.p12', password: 'x'));
  tentar('p12 confiaveis senha errada', (c) => c.setTrustedCertificates('$d/srv.p12', password: 'x'));
  tentar('cadeia lixo', (c) => c.useCertificateChainBytes([1, 2, 3]));
  tentar('confiaveis lixo', (c) => c.setTrustedCertificatesBytes([1, 2, 3], password: 'x'));
  tentar('lixo', (c) => c.usePrivateKeyBytes([1, 2, 3], password: 'segredo'));
  final der = base64.decode(File('$d/srv.pem').readAsLinesSync().where((l) => !l.startsWith('-')).join());
  tentar('autoridades lixo', (c) => c.setClientAuthoritiesBytes([1, 2, 3]));
  tentar('autoridades p12 senha errada', (c) => c.setClientAuthorities('$d/srv.p12', password: 'x'));
  tentar('cadeia der', (c) => c.useCertificateChainBytes(der));
  tentar('confiaveis der', (c) => c.setTrustedCertificatesBytes(der));
  tentar('pem quebrado', (c) => c.setTrustedCertificatesBytes(utf8.encode('-----BEGIN CERTIFICATE-----\n@@@@\n-----END CERTIFICATE-----\n')));
  tentar('chave der', (c) => c.usePrivateKeyBytes(base64.decode(File('$d/srv.key').readAsLinesSync().where((l) => !l.startsWith('-')).join())));
  tentar('chave em pem de certificado', (c) => c.usePrivateKey('$d/srv.pem'));
  tentar('p12 vazio sem senha', (c) => c.usePrivateKey('$d/srv_sem_senha.p12'));
  tentar('p12 vazio cadeia', (c) => c.useCertificateChain('$d/srv_sem_senha.p12'));

  await aperto(
      'p12',
      SecurityContext()
        ..useCertificateChain('$d/srv.p12', password: 'segredo')
        ..usePrivateKey('$d/srv.p12', password: 'segredo'),
      SecurityContext()..setTrustedCertificates('$d/srv.p12', password: 'segredo'));
  await aperto(
      'pkcs8 cifrada',
      SecurityContext()
        ..useCertificateChain('$d/srv.pem')
        ..usePrivateKey('$d/srv_pkcs8_cifrada.key', password: 'segredo'),
      SecurityContext()..setTrustedCertificates('$d/ca.pem'));
  await aperto(
      'rsa cifrada',
      SecurityContext()
        ..useCertificateChain('$d/srv.pem')
        ..usePrivateKey('$d/srv_rsa_cifrada.key', password: 'segredo'),
      SecurityContext()..setTrustedCertificates('$d/ca.pem'));
}
