// Regression: dart:io TLS (crates/runtime/src/tls.rs and
// native_sdk/io/secure_socket_patch.dart). Client and server in the same
// isolate: ALPN, the peer's X509Certificate, the rejection of the certificate
// without the CA (the VM's message), onBadCertificate, a volume larger than
// the filter's rings and dart:_http's HTTPS. The test certificates are in tls/.
import 'dart:async';
import 'dart:convert';
import 'dart:io';

Future<void> soquetes(String dir) async {
  final servidor = SecurityContext()
    ..useCertificateChain('$dir/srv.pem')
    ..usePrivateKey('$dir/srv.key')
    ..setAlpnProtocols(['h2', 'http/1.1'], true);
  final server = await SecureServerSocket.bind('127.0.0.1', 0, servidor);
  server.listen((s) {
    final recebido = <int>[];
    late StreamSubscription<List<int>> sub;
    sub = s.listen((dados) async {
      recebido.addAll(dados);
      if (recebido.isNotEmpty && recebido.last == 10) {
        final linha = utf8.decode(recebido);
        s.write(linha.length > 1000 ? 'eco: ${linha.length}' : 'eco: $linha');
        await s.flush();
        await s.close();
      }
    }, onError: (e) => print('servidor (leitura): ${e.runtimeType}'));
  }, onError: (e) => print('servidor: ${e.runtimeType}'));

  // 1. Trusting the CA.
  final cliente = SecurityContext()..setTrustedCertificates('$dir/ca.pem');
  final s = await SecureSocket.connect('localhost', server.port,
      context: cliente, supportedProtocols: ['http/1.1']);
  print('protocolo: ${s.selectedProtocol}');
  final c = s.peerCertificate!;
  print('sujeito: ${c.subject}');
  print('emissor: ${c.issuer}');
  print('validade: ${c.endValidity.isAfter(c.startValidity)}');
  print('der: ${c.der.length > 100} sha1: ${c.sha1.length}');
  print('pem: ${c.pem.startsWith('-----BEGIN CERTIFICATE-----')}');
  s.write('olá TLS\n');
  await s.flush();
  print(await utf8.decoder.bind(s).join());
  await s.close();

  // 2. Without trusting: HandshakeException.
  try {
    await SecureSocket.connect('localhost', server.port,
        context: SecurityContext());
    print('conectou sem confiar?!');
  } on HandshakeException catch (e) {
    print('recusado: ${e.runtimeType}');
    print(e.osError?.message.contains('CERTIFICATE_VERIFY_FAILED'));
  }

  // 3. onBadCertificate accepts.
  String? visto;
  final s3 = await SecureSocket.connect('localhost', server.port,
      context: SecurityContext(), onBadCertificate: (cert) {
    visto = cert.subject;
    return true;
  });
  print('aceito pelo callback: $visto');
  s3.write('de novo\n');
  await s3.flush();
  print(await utf8.decoder.bind(s3).join());
  await s3.close();

  // 4. Large volume in both directions.
  final grande = 'x' * 200000;
  final s4 = await SecureSocket.connect('localhost', server.port, context: cliente);
  s4.write('$grande\n');
  await s4.flush();
  final resposta = await utf8.decoder.bind(s4).join();
  print('grande: ${resposta.length}');
  await s4.close();

  await server.close();
}

Future<void> http(String dir) async {
  final ctx = SecurityContext()
    ..useCertificateChain('$dir/srv.pem')
    ..usePrivateKey('$dir/srv.key');
  final server = await HttpServer.bindSecure('127.0.0.1', 0, ctx);
  server.listen((req) async {
    final corpo = await utf8.decoder.bind(req).join();
    req.response.headers.contentType = ContentType.json;
    req.response.write(jsonEncode({
      'metodo': req.method,
      'caminho': req.uri.path,
      'corpo': corpo.length,
      'https': req.requestedUri.scheme,
    }));
    await req.response.close();
  });

  final cliente = HttpClient(
      context: SecurityContext()..setTrustedCertificates('$dir/ca.pem'));
  for (final caminho in ['/a', '/b/c']) {
    final req = await cliente.postUrl(Uri.parse('https://localhost:${server.port}$caminho'));
    req.write('x' * 50000);
    final resp = await req.close();
    print('${resp.statusCode} ${resp.headers.contentType} '
        '${await utf8.decoder.bind(resp).join()}');
  }
  // Bad certificate in HttpClient: badCertificateCallback.
  final inseguro = HttpClient()..badCertificateCallback = (c, h, p) {
    print('callback: $h ${c.issuer}');
    return true;
  };
  final r = await (await inseguro.getUrl(Uri.parse('https://localhost:${server.port}/i'))).close();
  print('inseguro: ${r.statusCode} ${await utf8.decoder.bind(r).join()}');
  try {
    await (await HttpClient().getUrl(Uri.parse('https://localhost:${server.port}/'))).close();
  } on HandshakeException catch (e) {
    print('recusado: ${e.osError?.message.trim()}');
  }
  cliente.close();
  inseguro.close();
  await server.close();
}

Future<void> main(List<String> args) async {
  final dir = args[0];
  await soquetes(dir);
  await http(dir);
}
