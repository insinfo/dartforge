// Differential coverage for the ordinary dart_ui color conversion kernel.
import 'dart:typed_data';
import 'package:dart_ui/src/graphics/image/codecs/color/format.dart';

String fingerprint(num value) {
  if (value is int) return 'int:${value.toRadixString(16)}';
  final bytes = ByteData(8)..setFloat64(0, value.toDouble());
  // Compare exact IEEE representation instead of decimal formatting.
  return 'double:${bytes.getUint32(0)}:${bytes.getUint32(4)}';
}

void main() {
  final values = <num>[
    0, 1, -1, 3, 127, 255, 65535, 4294967295,
    9007199254740993, 9223372036854775807,
    0.0, -0.0, 0.5, -0.5, 1.0, 255.0,
    double.nan, double.infinity, double.negativeInfinity,
  ];
  for (var i = 0; i < values.length; i++) {
    for (final from in Format.values) {
      for (final to in Format.values) {
        final prefix = '$i:${from.index}:${to.index}';
        try {
          print('$prefix:${fingerprint(convertFormatValue(values[i], from, to))}');
        } catch (error) {
          print('$prefix:error:${error.runtimeType}');
        }
      }
    }
  }
}
