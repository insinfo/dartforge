// Run with Dart 3.6.2 and the sass 1.102.0 package selected by --packages.
// This records public Dart Sass conversions, independently of the Rust port.
import 'package:sass/sass.dart';

void main() {
  final inputs = <String, List<double?>>{
    'extreme-saturation': [20, 999999, 50, 1],
    'negative-saturation': [20, -999999, 50, 0.5],
    'negative-lightness': [20, 100, -100, 1],
    'excess-lightness': [-340, 100, 200, 1],
    'missing-hue': [null, 75, 50, 1],
    'missing-saturation': [20, null, 50, null],
    'missing-lightness': [20, 75, null, -0.0],
    'black': [0, 0, 0, 0],
  };
  print('case,space,channel0,channel1,channel2,alpha');
  for (final entry in inputs.entries) {
    final input = entry.value;
    final color = SassColor.hsl(input[0], input[1], input[2], input[3]);
    final lab = color.toSpace(ColorSpace.lab);
    final outputs = {
      'lab': lab,
      'srgb': color.toSpace(ColorSpace.srgb),
      'xyz-d50': color.toSpace(ColorSpace.xyzD50),
      'lab-to-xyz-d65': lab.toSpace(ColorSpace.xyzD65),
    };
    for (final output in outputs.entries) {
      final channels = [
        ...output.value.channelsOrNull,
        output.value.alphaOrNull
      ];
      print(
          '${entry.key},${output.key},${channels.map((v) => v?.toString() ?? 'none').join(',')}');
    }
  }
}
