/// The builder executor's channel: big-endian `u32` frames + UTF-8 JSON over
/// stdin/stdout (docs/BUILD-PROTOCOLO.md §1). stderr stays free for human
/// logs; nothing but frames may go to stdout.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

/// Already framed JSON messages. Tests use an in-memory channel.
abstract interface class Channel {
  Stream<Map<String, Object?>> get input;
  void send(Map<String, Object?> message);
  Future<void> close();
}

/// The executor process's stdio channel.
final class StdioChannel implements Channel {
  final _output = StreamController<Map<String, Object?>>();

  StdioChannel() {
    var pending = Uint8List(0);
    stdin.listen((bytes) {
      final data = Uint8List(pending.length + bytes.length)
        ..setAll(0, pending)
        ..setAll(pending.length, bytes);
      var i = 0;
      while (data.length - i >= 4) {
        final n = ByteData.sublistView(data, i, i + 4).getUint32(0, Endian.big);
        if (data.length - i - 4 < n) break;
        final text = utf8.decode(Uint8List.sublistView(data, i + 4, i + 4 + n));
        _output.add(jsonDecode(text) as Map<String, Object?>);
        i += 4 + n;
      }
      pending = Uint8List.fromList(Uint8List.sublistView(data, i));
    }, onDone: _output.close);
  }

  @override
  Stream<Map<String, Object?>> get input => _output.stream;

  @override
  void send(Map<String, Object?> message) {
    final body = utf8.encode(jsonEncode(message));
    final header = ByteData(4)..setUint32(0, body.length, Endian.big);
    stdout.add(header.buffer.asUint8List());
    stdout.add(body);
  }

  @override
  Future<void> close() => stdout.flush();
}
