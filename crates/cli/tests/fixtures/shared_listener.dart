// Regression: two shared ServerSockets (`shared: true`) share the same
// system socket; closing one must not close the other's socket.
import 'dart:async';
import 'dart:io';

Future<int> conectarELer(int porta) async {
  final s = await Socket.connect(InternetAddress.loopbackIPv4, porta);
  final dados = await s.fold<List<int>>(<int>[], (a, b) => a..addAll(b));
  s.destroy();
  return dados.length;
}

void responder(Socket c) {
  c.add([1, 2, 3]);
  c.close();
  // Listening to the input until the end frees the socket when the client leaves.
  c.drain<void>();
}

Future<void> main() async {
  for (var rodada = 0; rodada < 20; rodada++) {
    final a = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0, shared: true);
    final b = await ServerSocket.bind(InternetAddress.loopbackIPv4, a.port, shared: true);
    a.listen(responder);
    b.listen(responder);
    await a.close();
    for (var i = 0; i < 5; i++) {
      final n = await conectarELer(b.port).timeout(const Duration(seconds: 5));
      if (n != 3) throw StateError('rodada $rodada: $n bytes');
    }
    await b.close();
  }
  print('o compartilhado sobrevive: ok');

  // When the last one leaves, the system socket closes: the port becomes
  // free again for an exclusive bind.
  for (var rodada = 0; rodada < 20; rodada++) {
    final a = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0, shared: true);
    final b = await ServerSocket.bind(InternetAddress.loopbackIPv4, a.port, shared: true);
    final porta = a.port;
    await a.close();
    await b.close();
    final c = await ServerSocket.bind(InternetAddress.loopbackIPv4, porta);
    await c.close();
  }
  print('o ultimo fecha: ok');
}
