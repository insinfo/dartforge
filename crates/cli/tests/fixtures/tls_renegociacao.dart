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
//
// Cada espera tem prazo e o programa sempre termina: uma etapa que não
// conclui vira a linha da rodada (`etapa X: sem resposta em Ns`), em vez de
// um processo morto pelo teste com a saída perdida no pipe.
import 'dart:async';
import 'dart:convert';
import 'dart:io';

/// [f] com prazo; estourado, [EtapaSemResposta] com o nome da etapa.
Future<T> etapa<T>(String nome, Duration prazo, Future<T> f) =>
    f.timeout(prazo, onTimeout: () => throw EtapaSemResposta(nome, prazo));

class EtapaSemResposta implements Exception {
  final String etapa;
  final Duration prazo;
  EtapaSemResposta(this.etapa, this.prazo);
  @override
  String toString() => 'etapa $etapa: sem resposta em ${prazo.inSeconds}s';
}

/// Uma rodada: o servidor manda `R` (renegociar) ou só uma linha; o que o
/// cliente viu.
Future<String> rodada(String dir, {required bool permitir, required bool renegociar}) async {
  final livre = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
  final porta = livre.port;
  await livre.close();
  final servidor = await etapa('Process.start', const Duration(seconds: 20), Process.start('openssl', [
    's_server', '-accept', '$porta', '-cert', '$dir/srv.pem', '-key', '$dir/srv.key', //
    '-tls1_2', '-legacy_renegotiation',
  ]));
  final saida = StringBuffer();
  final erros = StringBuffer();
  servidor.stdout.transform(utf8.decoder).listen(saida.write);
  servidor.stderr.transform(utf8.decoder).listen(erros.write);
  int? saiu;
  servidor.exitCode.then((c) => saiu = c);
  // O `s_server` não avisa quando escuta: tenta até conectar, com prazo (no
  // Windows cada conexão recusada leva ~2 s) e parando se ele já saiu.
  SecureSocket? s;
  final c = SecurityContext()..setTrustedCertificates('$dir/ca.pem');
  c.allowLegacyUnsafeRenegotiation = permitir;
  final prazo = DateTime.now().add(const Duration(seconds: 20));
  while (s == null && saiu == null && DateTime.now().isBefore(prazo)) {
    try {
      s = await etapa('SecureSocket.connect', const Duration(seconds: 15),
          SecureSocket.connect('localhost', porta, context: c, timeout: const Duration(seconds: 5)));
    } on SocketException {
      await Future.delayed(const Duration(milliseconds: 50));
    }
  }
  if (s == null) {
    servidor.kill();
    await etapa('exitCode do openssl', const Duration(seconds: 10), servidor.exitCode);
    return 'sem conexão (openssl ${saiu == null ? 'não escutou' : 'saiu com $saiu'}): ${erros.toString().trim()}';
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
  await etapa('stdin.flush', const Duration(seconds: 10), servidor.stdin.flush());
  await Future.any([fim.future, Future.delayed(const Duration(seconds: 3))]);
  s.destroy();
  servidor.kill();
  await etapa('exitCode do openssl', const Duration(seconds: 10), servidor.exitCode);
  return erro != null ? 'erro: $erro' : 'recebido: ${recebido.toString().trim()}';
}

Future<void> main(List<String> a) async {
  try {
    await etapa('openssl version', const Duration(seconds: 20), Process.run('openssl', ['version']));
  } on ProcessException {
    print('sem openssl');
    return;
  }
  final dir = a[0];
  var semResposta = false;
  Future<String> medir(bool permitir, bool renegociar) =>
      rodada(dir, permitir: permitir, renegociar: renegociar).catchError((Object e) {
        semResposta = true;
        return '$e';
      }, test: (e) => e is EtapaSemResposta);
  print('controle: ${await medir(false, false)}');
  print('padrão: ${await medir(false, true)}');
  print('com a opção: ${await medir(true, true)}');
  // A operação que não respondeu ainda prende o laço de eventos: sai com o
  // relato já impresso (a comparação da saída acusa a etapa).
  if (semResposta) exit(0);
}
