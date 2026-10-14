// Incoming typed buffers stay valid across later reads and allocations.
// Packet boundaries are deliberately ignored: only content and lifetime matter.
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

Future<void> main() async {
  const size = 65573;
  final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
  server.listen((connection) {
    final chunks = <List<int>>[];
    var total = 0;
    connection.listen((chunk) async {
      chunks.add(chunk);
      total += chunk.length;
      if (total == size) {
        // Trigger allocations before rereading all previously delivered chunks.
        final scratch = [for (var i = 0; i < 50; i++) List<int>.filled(100, i)];
        var index = 0;
        var sum = 0;
        var correct = true;
        for (final retained in chunks) {
          for (final byte in retained) {
            correct = correct && byte == (index & 255);
            sum += byte;
            index++;
          }
        }
        connection.write('$total $sum $correct ${scratch.last.first}');
        await connection.close();
      }
    });
  });
  final client =
      await Socket.connect(InternetAddress.loopbackIPv4, server.port);
  final payload = Uint8List(size);
  for (var i = 0; i < size; i++) {
    payload[i] = i & 255;
  }
  client.add(Uint8List.sublistView(payload, 0, 17));
  await client.flush();
  client.add(Uint8List.sublistView(payload, 17));
  await client.flush();
  print(await utf8.decoder.bind(client).join());
  await client.close();
  await server.close();
}
