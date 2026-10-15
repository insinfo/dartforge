// Diagnostic public error classifications; runtimeType spelling is separate.
import 'dart:convert';
import 'dart:typed_data';
void report(String name, void Function() operation) {
  try { operation(); print('$name:ok'); }
  catch (error) {
    print('$name:${error is RangeError}:${error is ArgumentError}:${error is Error}:${error is IndexError}');
  }
}
void main() {
  report('uint8-negative', () { Uint8List(-1); });
  report('json-negative', () { JsonUtf8Encoder(null, null, -1).convert({'x': 1}); });
  report('json-zero', () { JsonUtf8Encoder(null, null, 0).convert({'x': 1}); });
  print(JsonUtf8Encoder(null, null, 1).convert({'x': 'é🙂'}).join(','));
}
