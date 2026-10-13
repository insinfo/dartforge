/// The executor process's channel: big-endian `u32` frames + UTF-8 JSON over
/// stdin/stdout (docs/BUILD-PROTOCOLO.md §1, the framing of the official
/// prototype's `message_grouper.dart`). stderr stays free for logs.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'service.dart';

final class StdioChannel implements Channel {
  final _output = StreamController<Map<String, Object?>>();

  StdioChannel() {
    final buffer = BytesBuilder(copy: false);
    stdin.listen((bytes) {
      buffer.add(bytes);
      var data = buffer.takeBytes();
      var i = 0;
      while (data.length - i >= 4) {
        final n = ByteData.sublistView(data, i, i + 4).getUint32(0, Endian.big);
        if (data.length - i - 4 < n) break;
        final json = utf8.decode(Uint8List.sublistView(data, i + 4, i + 4 + n));
        _output.add(jsonDecode(json) as Map<String, Object?>);
        i += 4 + n;
      }
      if (i < data.length) buffer.add(Uint8List.sublistView(data, i));
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
