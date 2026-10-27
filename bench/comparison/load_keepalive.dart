// Keep-alive HTTP/1.1 load generator for bench/comparison/server.dart
// (RELATORIO-PRONTIDAO-PRODUCAO.md), compiled with `dart compile exe`: I isolates x C
// connections, one GET /json in flight per connection, for S seconds; prints the
// total and the requests per second. Usage: load_keepalive <port> [I=4] [C=8] [S=5].
import 'dart:async';
import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

final _req = 'GET /json HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n'.codeUnits;

Future<int> worker(List args) async {
  final port = args[0] as int, conns = args[1] as int, ms = args[2] as int;
  final end = DateTime.now().millisecondsSinceEpoch + ms;
  var count = 0;
  Future<void> one() async {
    final s = await Socket.connect('127.0.0.1', port);
    s.setOption(SocketOption.tcpNoDelay, true);
    final done = Completer<void>();
    var tail = <int>[];
    s.listen((Uint8List data) {
      // A response ends with the last chunk "0\r\n\r\n".
      tail.addAll(data.length > 8 ? data.sublist(data.length - 8) : data);
      if (tail.length > 8) tail = tail.sublist(tail.length - 8);
      final n = tail.length;
      if (n >= 5 && tail[n - 5] == 48 && tail[n - 4] == 13 && tail[n - 3] == 10 && tail[n - 2] == 13 && tail[n - 1] == 10) {
        tail = <int>[];
        count++;
        if (DateTime.now().millisecondsSinceEpoch >= end) {
          s.destroy();
          if (!done.isCompleted) done.complete();
        } else {
          s.add(_req);
        }
      }
    }, onDone: () { if (!done.isCompleted) done.complete(); });
    s.add(_req);
    await done.future;
  }
  await Future.wait([for (var i = 0; i < conns; i++) one()]);
  return count;
}

Future<void> main(List<String> a) async {
  final port = int.parse(a[0]);
  final iso = a.length > 1 ? int.parse(a[1]) : 4;
  final conns = a.length > 2 ? int.parse(a[2]) : 8;
  final secs = a.length > 3 ? double.parse(a[3]) : 5.0;
  final ms = (secs * 1000).round();
  final sw = Stopwatch()..start();
  final counts = await Future.wait([for (var i = 0; i < iso; i++) Isolate.run(() => worker([port, conns, ms]))]);
  final total = counts.fold<int>(0, (x, y) => x + y);
  print('requests $total req/s ${(total / sw.elapsedMicroseconds * 1e6).round()}');
}
