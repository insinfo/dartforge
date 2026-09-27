// dart:io nativo: RawSynchronousSocket (N05) — conexão, porta e par remoto,
// readSync/readIntoSync/writeFromSync, fim do fluxo (null), available,
// shutdown, closeSync e os erros de conexão recusada e de resolução.
import 'dart:io';
import 'dart:isolate';

// Servidor de eco num isolado: responde o que recebe, com um prefixo, e
// fecha a conexão depois de "fim".
Future<void> servidor(SendPort pronto) async {
  final s = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
  pronto.send(s.port);
  await for (final c in s) {
    c.listen((d) {
      final t = String.fromCharCodes(d);
      c.add('eco:$t'.codeUnits);
      if (t.contains('fim')) c.close();
    });
  }
}

Future<void> main() async {
  final porta = ReceivePort();
  await Isolate.spawn(servidor, porta.sendPort);
  final p = await porta.first as int;
  final s = RawSynchronousSocket.connectSync(InternetAddress.loopbackIPv4, p);
  print('porta local > 0: ${s.port > 0}');
  print('remota: ${s.remoteAddress.address} ${s.remotePort == p}');
  s.writeFromSync('ola'.codeUnits);
  final r = s.readSync(64)!;
  print(String.fromCharCodes(r));
  final buf = List<int>.filled(16, 0);
  s.writeFromSync('abc'.codeUnits);
  final n = s.readIntoSync(buf, 2);
  print('$n ${String.fromCharCodes(buf.sublist(2, 2 + n))}');
  s.writeFromSync('xyz-fim'.codeUnits, 4);
  print(String.fromCharCodes(s.readSync(64)!));
  print('eof: ${s.readSync(8)}');
  print('available: ${s.available()}');
  s.shutdown(SocketDirection.send);
  try {
    s.writeFromSync([1]);
  } on SocketException catch (e) {
    print('escrita depois do shutdown: ${e.message}');
  }
  s.closeSync();
  try {
    s.readSync(1);
  } on SocketException catch (e) {
    print('leitura depois de fechar: ${e.message}');
  }
  try {
    RawSynchronousSocket.connectSync('localhost', 1);
  } on SocketException catch (e) {
    print('recusada: ${e.osError != null}');
  }
  try {
    RawSynchronousSocket.connectSync('nao.existe.invalido', 80);
  } on SocketException catch (e) {
    print('lookup: ${e.message.startsWith('Failed host lookup')}');
  }
  porta.close();
  exit(0);
}
