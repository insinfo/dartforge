// Contrato de `SecurityContext.allowLegacyUnsafeRenegotiation` no nativo
// (rustls, que não renegocia): um servidor `openssl s_server -www` TLS 1.2
// pede renegociação (HelloRequest) ao receber `GET /reneg`, depois do aperto
// de mão.
//
// * Sem a opção (o padrão), como a VM: o pedido é recusado e a conexão
//   termina com `TlsException`.
// * Com a opção: a VM renegocia e recebe a página; aqui o pedido é recusado
//   do mesmo jeito (a opção só é guardada) — o comportamento documentado em
//   NATIVO-PLANO.
// * Controle: `GET /`, sem pedido de renegociação; a página chega.
//
// Vai só a linha do pedido, sem a linha em branco: o `s_server -www` responde
// à linha `GET` e, depois do HelloRequest, lê a conexão uma vez para receber
// o ClientHello da renegociação; uma linha em branco já no buffer dele seria
// lida no lugar, e a página iria no meio da renegociação.
//
// O pedido vai pela própria conexão TLS, não pela entrada padrão do
// `s_server`: no Windows o `s_server` não espera pela entrada e pelo soquete
// juntos (o `select` do Winsock só vale para soquetes). Com a entrada
// redirecionada, o `has_stdin_waiting` do OpenSSL (`apps/lib/apps.c`)
// responde "há entrada" até o fim do arquivo e o `sv_body`
// (`apps/s_server.c`) bloqueia no `read` da entrada antes de atender o
// soquete: o aperto de mão só andaria depois de o cliente escrever na
// entrada, o que ele só faz depois do aperto de mão. O modo `-www`
// (`www_body`) não lê a entrada padrão em sistema nenhum.
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
  /// O que ajuda a entender a etapa parada (a saída do `openssl`).
  final String detalhe;
  EtapaSemResposta(this.etapa, this.prazo, [this.detalhe = '']);
  @override
  String toString() => 'etapa $etapa: sem resposta em ${prazo.inSeconds}s${detalhe.isEmpty ? '' : ' ($detalhe)'}';
}

/// Uma rodada: o cliente pede `/reneg` (o servidor pede a renegociação) ou
/// `/` (só a página); o que o cliente viu.
Future<String> rodada(String dir, {required bool permitir, required bool renegociar}) async {
  final livre = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
  final porta = livre.port;
  await livre.close();
  final servidor = await etapa('Process.start', const Duration(seconds: 20), Process.start('openssl', [
    's_server', '-accept', '$porta', '-cert', '$dir/srv.pem', '-key', '$dir/srv.key', //
    '-tls1_2', '-legacy_renegotiation', '-www',
  ]));
  final saida = StringBuffer();
  final erros = StringBuffer();
  servidor.stdout.transform(utf8.decoder).listen(saida.write);
  servidor.stderr.transform(utf8.decoder).listen(erros.write);
  int? saiu;
  servidor.exitCode.then((c) => saiu = c);
  // A saída do `openssl` para o relato de uma etapa parada.
  String relato() => 'openssl: ${erros.toString().trim()} ${saida.toString().trim()}';
  Future<void> encerrar() async {
    servidor.kill();
    await etapa('exitCode do openssl', const Duration(seconds: 10), servidor.exitCode);
  }

  // O `s_server` não avisa quando escuta: tenta até conectar, com prazo (no
  // Windows cada conexão recusada leva ~2 s) e parando se ele já saiu. A
  // conexão TCP e o aperto de mão TLS são etapas separadas: o `timeout` do
  // `connect` só vale para a primeira.
  final c = SecurityContext()..setTrustedCertificates('$dir/ca.pem');
  c.allowLegacyUnsafeRenegotiation = permitir;
  Socket? tcp;
  final prazo = DateTime.now().add(const Duration(seconds: 20));
  while (tcp == null && saiu == null && DateTime.now().isBefore(prazo)) {
    try {
      tcp = await etapa('Socket.connect', const Duration(seconds: 15),
          Socket.connect('localhost', porta, timeout: const Duration(seconds: 5)));
    } on SocketException {
      await Future.delayed(const Duration(milliseconds: 50));
    }
  }
  if (tcp == null) {
    await encerrar();
    return 'sem conexão (openssl ${saiu == null ? 'não escutou' : 'saiu com $saiu'}): ${erros.toString().trim()}';
  }
  // Parada uma etapa, a conexão e o servidor vão embora e o relato leva a
  // saída do `openssl`. [s] é a conexão TLS, depois do `secure`.
  SecureSocket? s;
  final recebido = StringBuffer();
  String? erro;
  try {
    final conexao = await etapa('SecureSocket.secure', const Duration(seconds: 15),
        SecureSocket.secure(tcp, host: 'localhost', context: c));
    s = conexao;
    // A rodada acaba na linha de estado da resposta, no erro ou no fim da
    // conexão (o `s_server` não fecha logo depois de responder).
    final fim = Completer<void>();
    conexao.cast<List<int>>().transform(utf8.decoder).listen((texto) {
      recebido.write(texto);
      if (recebido.toString().contains('\n') && !fim.isCompleted) fim.complete();
    }, onError: (e) {
      erro = e.runtimeType.toString();
      if (!fim.isCompleted) fim.complete();
    }, onDone: () {
      if (!fim.isCompleted) fim.complete();
    });
    // O aperto de mão terminou (o `secure` voltou): o pedido. O `s_server`
    // responde com a página; com `/reneg`, antes manda o HelloRequest, e a
    // recusa do cliente (o alerta `no_renegotiation`) derruba a conexão.
    conexao.write(renegociar ? 'GET /reneg HTTP/1.0\r\n' : 'GET / HTTP/1.0\r\n');
    try {
      await etapa('flush do pedido', const Duration(seconds: 5), conexao.flush());
    } on TlsException {
      // A recusa pode chegar antes de o envio terminar; o relato vem do
      // `listen`.
    } on SocketException {
      // Idem, com a conexão já derrubada.
    }
    await etapa('resposta do servidor', const Duration(seconds: 10), fim.future);
  } on EtapaSemResposta catch (e) {
    (s ?? tcp).destroy();
    await encerrar();
    throw EtapaSemResposta(e.etapa, e.prazo, relato());
  }
  s.destroy();
  await encerrar();
  // Do controle, a linha de estado (o corpo da página traz os argumentos e
  // as cifras, que variam com o sistema).
  return erro != null ? 'erro: $erro' : 'recebido: ${const LineSplitter().convert(recebido.toString()).firstOrNull ?? ''}';
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
