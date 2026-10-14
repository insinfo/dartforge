import 'dart:typed_data';
import 'package:dart_ui/src/graphics/image/codecs/color/format.dart';

void main(List<String> arguments) {
  final rounds = arguments.isEmpty ? 1000 : int.parse(arguments.single);
  if (rounds < 1) throw ArgumentError.value(rounds, 'rounds');
  final formats = Format.values;
  final values = <num>[0, 1, 128, 255];
  var checksum = 0.0;
  var calls = 0;
  for (var round = 0; round < rounds; round++) {
    for (final from in formats) {
      for (final to in formats) {
        for (final value in values) {
          checksum += convertFormatValue(value, from, to).toDouble();
          calls++;
        }
      }
    }
  }
  final bytes = ByteData(8)..setFloat64(0, checksum, Endian.little);
  print('$calls:${bytes.buffer.asUint8List().join(',')}');
}
