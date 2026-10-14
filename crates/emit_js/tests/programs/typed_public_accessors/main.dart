import 'dart:typed_data';
import 'hook_stub.dart' if (dart.library.js_interop) 'hook_web.dart';

int sum(Uint8List values) {
  var result = 0;
  for (var i = 0; i < values.length; i++) {
    result += values[i];
  }
  return result;
}

void main() {
  final values = Uint8List.fromList([1, 2, 3]);
  installBufferProbe();
  print(sum(values));
  print(bufferReads());
}
