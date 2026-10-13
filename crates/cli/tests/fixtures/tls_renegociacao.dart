// Contract of `SecurityContext.allowLegacyUnsafeRenegotiation` on native
// (rustls, which does not renegotiate): an `openssl s_server -www` TLS 1.2
// server requests renegotiation (HelloRequest) on receiving `GET /reneg`,
// after the handshake.
//
// * Without the option (the default), like the VM: the request is refused and
//   the connection ends with `TlsException`.
// * With the option: the VM renegotiates and receives the page; here the
//   request is refused the same way (the option is only stored) — the
//   behavior documented in NATIVO-PLANO.
// * Control: `GET /`, without a renegotiation request; the page arrives.
//
// Only the request line is sent, without the blank line: `s_server -www`
// answers the `GET` line and, after the HelloRequest, reads the connection
// once to receive the renegotiation ClientHello; a blank line already in its
// buffer would be read instead, and the page would go out in the middle of
// the renegotiation.
//
// The request goes through the TLS connection itself, not through the
// standard input of `s_server`: on Windows `s_server` does not wait on the
// input and the socket together (Winsock's `select` only works for sockets).
// With the input redirected, OpenSSL's `has_stdin_waiting`
// (`apps/lib/apps.c`) answers "there is input" until end of file and
// `sv_body` (`apps/s_server.c`) blocks on the input `read` before serving the
// socket: the handshake would only proceed after the client wrote to the
// input, which it only does after the handshake. The `-www` mode
// (`www_body`) does not read the standard input on any system.
//
// Arguments: the certificates directory (tls/).
//
// Every wait has a deadline and the program always ends: a step that does not
// complete becomes the round's line (`etapa X: sem resposta em Ns`), instead
// of a process killed by the test with the output lost in the pipe.
import 'dart:async';
import 'dart:convert';
import 'dart:io';

/// [f] with a deadline; once exceeded, [EtapaSemResposta] with the step's name.
Future<T> etapa<T>(String nome, Duration prazo, Future<T> f) =>
    f.timeout(prazo, onTimeout: () => throw EtapaSemResposta(nome, prazo));

class EtapaSemResposta implements Exception {
  final String etapa;
  final Duration prazo;
  /// What helps understand the stalled step (the output of `openssl`).
  final String detalhe;
  EtapaSemResposta(this.etapa, this.prazo, [this.detalhe = '']);
  @override
  String toString() => 'etapa $etapa: sem resposta em ${prazo.inSeconds}s${detalhe.isEmpty ? '' : ' ($detalhe)'}';
}

/// One round: the client requests `/reneg` (the server requests
/// renegotiation) or `/` (just the page); what the client saw.
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
  // The output of `openssl` for the report of a stalled step.
  String relato() => 'openssl: ${erros.toString().trim()} ${saida.toString().trim()}';
  Future<void> encerrar() async {
    servidor.kill();
    await etapa('exitCode do openssl', const Duration(seconds: 10), servidor.exitCode);
  }

  // `s_server` does not announce when it is listening: retry until connected,
  // with a deadline (on Windows each refused connection takes ~2 s) and
  // stopping if it already exited. The TCP connection and the TLS handshake
  // are separate steps: the `timeout` of `connect` only applies to the first.
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
  // When a step stalls, the connection and the server go away and the report
  // carries the output of `openssl`. [s] is the TLS connection, after `secure`.
  SecureSocket? s;
  final recebido = StringBuffer();
  String? erro;
  try {
    final conexao = await etapa('SecureSocket.secure', const Duration(seconds: 15),
        SecureSocket.secure(tcp, host: 'localhost', context: c));
    s = conexao;
    // The round ends at the response's status line, at the error or at the end
    // of the connection (`s_server` does not close right after answering).
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
    // The handshake finished (`secure` returned): the request. `s_server`
    // answers with the page; with `/reneg`, it first sends the HelloRequest,
    // and the client's refusal (the `no_renegotiation` alert) drops the
    // connection.
    conexao.write(renegociar ? 'GET /reneg HTTP/1.0\r\n' : 'GET / HTTP/1.0\r\n');
    try {
      await etapa('flush do pedido', const Duration(seconds: 5), conexao.flush());
    } on TlsException {
      // The refusal may arrive before the send finishes; the report comes
      // from `listen`.
    } on SocketException {
      // Likewise, with the connection already dropped.
    }
    await etapa('resposta do servidor', const Duration(seconds: 10), fim.future);
  } on EtapaSemResposta catch (e) {
    (s ?? tcp).destroy();
    await encerrar();
    throw EtapaSemResposta(e.etapa, e.prazo, relato());
  }
  s.destroy();
  await encerrar();
  // From the control round, the status line (the page body carries the
  // arguments and the ciphers, which vary with the system).
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
  // The operation that did not respond still holds the event loop: exit with
  // the report already printed (the output comparison points at the step).
  if (semResposta) exit(0);
}
