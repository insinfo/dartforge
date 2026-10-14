import 'dart:typed_data';
import 'package:dart_ui/src/graphics/image/codecs/color/format.dart';
String encode(num value) {
  if (value is int) return 'i:$value';
  if (value.isNaN) return 'nan';
  if (value.isInfinite) return value.isNegative ? '-inf' : 'inf';
  final data = ByteData(8)..setFloat64(0, value.toDouble(), Endian.little);
  return 'd:${data.buffer.asUint8List().map((b) => b.toRadixString(16).padLeft(2, '0')).join()}';
}
String errorTag(Object error) {
  if (error is TypeError) return 'TypeError';
  if (error is UnsupportedError) return 'UnsupportedError';
  if (error is ArgumentError) return 'ArgumentError';
  if (error is StateError) return 'StateError';
  return 'unexpected:${error.runtimeType}';
}
void main() {
  final values = <num>[0, 1, 3, 15, 128, 255, 65535, 2147483647,
    -1, -128, double.nan, double.infinity, double.negativeInfinity, 0.5, -0.0];
  for (final from in Format.values) {
    for (final to in Format.values) {
      final output = <String>[];
      for (final value in values) {
        try { output.add(encode(convertFormatValue(value, from, to))); }
        catch (error) { output.add(errorTag(error)); }
      }
      print('${from.index}:${to.index}:${output.join('|')}');
    }
  }
}
