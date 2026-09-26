// Regressão: dois ServerSocket compartilhados (`shared: true`) dividem o
// mesmo soquete do sistema; fechar um não pode fechar o soquete do outro.
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
  // Escutar a entrada até o fim libera o soquete quando o cliente sai.
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

  // Quando o último sai, o soquete do sistema fecha: a porta volta a ficar
  // livre para um bind exclusivo.
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
