// Exercise actual JSON private buffer replacement, error and callback paths.
import 'dart:collection';
import 'dart:convert';

final document = <String, Object?>{
  'ascii': 'abcdefghijklmnopqrstuvwxyz',
  'unicode': 'é Ω 漢字 🙂 \ud800 end',
  'escape': 'quote" back\\ controls\n\u0001',
  'values': [null, true, false, -1, 0, 255, 65536, 1.25],
};

String outcome(void Function() operation) {
  try { operation(); return 'ok'; } catch (error) { return error.runtimeType.toString(); }
}

class ProbeSink extends ByteConversionSink {
  final List<int> result = [];
  final List<List<int>> retained = [];
  int calls = 0;
  bool closed = false;
  final bool throwChunk;
  ProbeSink({this.throwChunk = false});
  void add(List<int> bytes) => addSlice(bytes, 0, bytes.length, false);
  void addSlice(List<int> bytes, int start, int end, bool isLast) {
    calls++;
    if (throwChunk && calls == 2) throw StateError('chunk callback');
    result.addAll(bytes.sublist(start, end));
    final nested = JsonUtf8Encoder(null, null, 2).convert({'nested': calls});
    if (utf8.decode(nested) != '{"nested":$calls}') throw StateError('nested');
    // User callbacks may retain and mutate old buffers. Subsequent outer
    // stores must use the fresh replacement allocated after this returns.
    for (final old in retained) { old.fillRange(0, old.length, 99); }
    retained.add(bytes);
    bytes.fillRange(0, bytes.length, 0);
    if (isLast) close();
  }
  void close() { closed = true; }
}

class ObservedList extends ListBase<int> {
  int reads = 0;
  int get length => 3;
  set length(int value) { throw UnsupportedError('fixed'); }
  int operator [](int index) {
    reads++;
    JsonUtf8Encoder(null, null, 1).convert({'index': index});
    return index + 1;
  }
  void operator []=(int index, int value) { throw UnsupportedError('read only'); }
}

void main() {
  for (final size in [1, 2, 3, 7, 256]) {
    for (final indent in <String?>[null, '  ', 'é']) {
      final encoder = JsonUtf8Encoder(indent, null, size);
      final bytes = encoder.convert(document);
      print('$size:$indent:${bytes.join(',')}');
      final sink = ProbeSink();
      final input = encoder.startChunkedConversion(sink);
      input.add(document);
      input.close();
      print('chunks:${sink.calls}:${sink.closed}:${sink.result.join(',')}');
    }
  }
  for (final size in [-1, 0]) {
    print('size:$size:${outcome(() { JsonUtf8Encoder(null, null, size).convert(document); })}');
  }
  final cycle = <Object?>[]; cycle.add(cycle);
  for (final value in <Object?>[double.nan, double.infinity, cycle, {1: 'bad'}, Object()]) {
    print('error:${outcome(() { JsonUtf8Encoder().convert(value); })}');
  }
  print('toEncodable:${outcome(() { JsonUtf8Encoder(null, (_) => throw StateError('convert'), 1).convert(Object()); })}');
  final failing = ProbeSink(throwChunk: true);
  print('callback:${outcome(() { JsonUtf8Encoder(null, null, 1).startChunkedConversion(failing).add(document); })}:${failing.calls}');
  final observed = ObservedList();
  print('custom:${JsonUtf8Encoder(null, null, 1).convert(observed).join(',')}:${observed.reads}');
}
