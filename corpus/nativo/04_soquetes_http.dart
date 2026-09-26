// dart:io, rede: ServerSocket/Socket (TCP), InternetAddress.lookup, HttpServer e HttpClient.
import 'dart:io';
import 'dart:convert';

Future<void> main() async {
  final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
  print('porta > 0: ${server.port > 0}');
  server.listen((c) async {
    final linha = await c.cast<List<int>>().transform(utf8.decoder).first;
    c.write('eco: $linha');
    await c.close();
  });
  final s = await Socket.connect(InternetAddress.loopbackIPv4, server.port);
  s.write('ola');
  await s.flush();
  final resp = await utf8.decoder.bind(s).join();
  print(resp);
  print(s.remoteAddress.address);
  await s.close();
  await server.close();
  final addrs = await InternetAddress.lookup('localhost');
  print(addrs.any((a) => a.isLoopback));
  final h = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
  h.listen((req) {
    req.response.write('ola http ${req.uri.path}');
    req.response.close();
  });
  final cli = HttpClient();
  final req = await cli.get('127.0.0.1', h.port, '/x');
  final res = await req.close();
  print('${res.statusCode} ${await utf8.decoder.bind(res).join()}');
  cli.close();
  await h.close();
}
