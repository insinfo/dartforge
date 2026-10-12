// Servidor HTTP do benchmark (`scripts/bench-http.py`): `dart:io` de verdade,
// keep-alive ligado (o padrão do `HttpServer`).
//
//   GET /      -> "hello" (text/plain)
//   GET /json  -> jsonEncode de um objeto pequeno (application/json)
//
// A porta vem do primeiro argumento (0 = qualquer uma) e é impressa como
// `porta <n>` quando o servidor está ouvindo.
import 'dart:convert';
import 'dart:io';

Future<void> main(List<String> args) async {
  final porta = args.isEmpty ? 0 : int.parse(args[0]);
  final servidor = await HttpServer.bind(InternetAddress.loopbackIPv4, porta);
  servidor.autoCompress = false;
  print('porta ${servidor.port}');
  var n = 0;
  await for (final req in servidor) {
    final res = req.response;
    switch (req.uri.path) {
      case '/json':
        n++;
        res.headers.contentType = ContentType.json;
        res.write(jsonEncode({
          'mensagem': 'Hello, World!',
          'id': n,
          'ativo': true,
          'itens': [1, 2, 3],
        }));
      case '/sair':
        res.write('tchau');
        await res.close();
        await servidor.close(force: true);
        return;
      default:
        res.headers.contentType = ContentType.text;
        res.write('hello');
    }
    res.close();
  }
}
