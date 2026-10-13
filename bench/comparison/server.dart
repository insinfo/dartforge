// HTTP server of the benchmark (`scripts/bench-http.py`): real `dart:io`,
// keep-alive on (the `HttpServer` default).
//
//   GET /      -> "hello" (text/plain)
//   GET /json  -> jsonEncode of a small object (application/json)
//   GET /quit  -> "bye", and the server stops
//
// The port comes from the first argument (0 = any) and is printed as
// `port <n>` when the server is listening.
import 'dart:convert';
import 'dart:io';

Future<void> main(List<String> args) async {
  final port = args.isEmpty ? 0 : int.parse(args[0]);
  final server = await HttpServer.bind(InternetAddress.loopbackIPv4, port);
  server.autoCompress = false;
  print('port ${server.port}');
  var n = 0;
  await for (final req in server) {
    final res = req.response;
    switch (req.uri.path) {
      case '/json':
        n++;
        res.headers.contentType = ContentType.json;
        res.write(jsonEncode({
          'message': 'Hello, World!',
          'id': n,
          'active': true,
          'items': [1, 2, 3],
        }));
      case '/quit':
        res.write('bye');
        await res.close();
        await server.close(force: true);
        return;
      default:
        res.headers.contentType = ContentType.text;
        res.write('hello');
    }
    res.close();
  }
}
