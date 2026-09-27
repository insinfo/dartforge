// Contrato de `SecurityContext.allowLegacyUnsafeRenegotiation` no nativo
// (rustls, que não renegocia): um servidor `openssl s_server` TLS 1.2 pede
// renegociação (`R`) depois do aperto de mão.
//
// * Sem a opção (o padrão), como a VM: o pedido é recusado e a conexão
//   termina com `TlsException`.
// * Com a opção: a VM renegocia; aqui o pedido é recusado do mesmo jeito (a
//   opção só é guardada) — o comportamento documentado em NATIVO-PLANO.
// * Controle: sem pedido de renegociação, os dados chegam.
//
// Argumentos: o diretório dos certificados (tls/).
import 'dart:async';
import 'dart:convert';
import 'dart:io';

/// Uma rodada: o servidor manda `R` (renegociar) ou só uma linha; o que o
/// cliente viu.
Future<String> rodada(String dir, {required bool permitir, required bool renegociar}) async {
  final livre = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
  final porta = livre.port;
  await livre.close();
  final servidor = await Process.start('openssl', [
    's_server', '-accept', '$porta', '-cert', '$dir/srv.pem', '-key', '$dir/srv.key', //
    '-tls1_2', '-legacy_renegotiation',
  ]);
  final saida = StringBuffer();
  servidor.stdout.transform(utf8.decoder).listen(saida.write);
  servidor.stderr.transform(utf8.decoder).listen((_) {});
  // O `s_server` não avisa quando escuta: tenta até conectar.
  SecureSocket? s;
  final c = SecurityContext()..setTrustedCertificates('$dir/ca.pem');
  c.allowLegacyUnsafeRenegotiation = permitir;
  for (var i = 0; i < 100 && s == null; i++) {
    try {
      s = await SecureSocket.connect('localhost', porta, context: c);
    } on SocketException {
      await Future.delayed(const Duration(milliseconds: 50));
    }
  }
  if (s == null) {
    servidor.kill();
    return 'sem conexão';
  }
  final recebido = StringBuffer();
  String? erro;
  final fim = Completer<void>();
  s.cast<List<int>>().transform(utf8.decoder).listen(recebido.write, onError: (e) {
    erro = e.runtimeType.toString();
    if (!fim.isCompleted) fim.complete();
  }, onDone: () {
    if (!fim.isCompleted) fim.complete();
  });
  // O aperto de mão terminou (o `connect` voltou): o pedido ou a linha.
  servidor.stdin.write(renegociar ? 'R\n' : 'linha do servidor\n');
  await servidor.stdin.flush();
  await Future.any([fim.future, Future.delayed(const Duration(seconds: 3))]);
  s.destroy();
  servidor.kill();
  await servidor.exitCode;
  return erro != null ? 'erro: $erro' : 'recebido: ${recebido.toString().trim()}';
}

Future<void> main(List<String> a) async {
  try {
    await Process.run('openssl', ['version']);
  } on ProcessException {
    print('sem openssl');
    return;
  }
  final dir = a[0];
  print('controle: ${await rodada(dir, permitir: false, renegociar: false)}');
  print('padrão: ${await rodada(dir, permitir: false, renegociar: true)}');
  print('com a opção: ${await rodada(dir, permitir: true, renegociar: true)}');
}
