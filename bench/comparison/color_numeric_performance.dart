// Run with the dart_ui package configuration used by color_numeric.dart.
import 'package:dart_ui/src/graphics/image/codecs/color/format.dart';

@pragma('vm:never-inline')
int convertPixels(List<num> pixels, int iterations) {
  var checksum = 0;
  for (var i = 0; i < iterations; i++) {
    checksum += convertFormatValue(
      pixels[i & 255], Format.float32, Format.uint8,
    ).toInt();
  }
  return checksum;
}

void main(List<String> arguments) {
  final iterations = arguments.isEmpty ? 1000000 : int.parse(arguments[0]);
  final pixels = List<num>.generate(256, (i) => (i - 32) / 192.0);
  for (var sample = 0; sample < 5; sample++) {
    final clock = Stopwatch()..start();
    final checksum = convertPixels(pixels, iterations);
    clock.stop();
    print('$checksum:${clock.elapsedMicroseconds}');
  }
}
