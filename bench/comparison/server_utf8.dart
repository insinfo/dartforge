// The server.dart workload with JSON encoded directly to bytes. Keep this a
// separate program so adding a second codec does not change the closed world
// or introduce branches and locals into the String-encoder baseline.
import 'dart:convert';
import 'dart:io';

Future<void> main(List<String> args) async {
  final port = args.isEmpty ? 0 : int.parse(args[0]);
  final server = await HttpServer.bind(InternetAddress.loopbackIPv4, port);
  server.autoCompress = false;
  final encoder = JsonUtf8Encoder();
  print('port ${server.port}');
  var n = 0;
  await for (final req in server) {
    final res = req.response;
    switch (req.uri.path) {
      case '/json':
        n++;
        res.headers.contentType = ContentType.json;
        res.add(encoder.convert({
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
