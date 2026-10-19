import 'dart:typed_data';
import 'package:dart_ui/src/graphics/image/codecs/color/format.dart';

int evaluations = 0;
@pragma('vm:never-inline')
num evaluated(num value) { evaluations++; return value; }
@pragma('vm:never-inline')
num toByte(num value) => convertFormatValue(value, Format.float32, Format.uint8);
@pragma('vm:never-inline')
num sameFormat(num value) => convertFormatValue(value, Format.float32, Format.float32);

String fingerprint(num value) {
  if (value is int) return 'int:${value.toRadixString(16)}';
  final bytes = ByteData(8)..setFloat64(0, value.toDouble());
  return 'double:${bytes.getUint32(0)}:${bytes.getUint32(4)}';
}
double fromBits(int bits) {
  final bytes = ByteData(8)..setInt64(0, bits);
  return bytes.getFloat64(0);
}
void main() {
  final values = <num>[
    -0x8000000000000000, -9007199254740993, -1, 0, 1, 2,
    9007199254740993, 0x7fffffffffffffff,
    double.negativeInfinity, -1.0, -0.5, -0.0, 0.0, 0.5, 1.0,
    double.infinity, double.nan, fromBits(1),
    fromBits(0x3fefffffffffffff), fromBits(0x3ff0000000000001),
    fromBits(0x7ff8000000000042), fromBits(-0x7ffffffffffbe),
  ];
  for (var pass = 0; pass < 2; pass++) {
    for (final value in values) {
      try { print('byte:${fingerprint(toByte(evaluated(value)))}'); }
      catch (error) { print('error:${error.runtimeType}'); }
      final same = sameFormat(evaluated(value));
      print('same:${fingerprint(same)}:${identical(value, same)}');
    }
  }
  print('evaluations:$evaluations');
}
