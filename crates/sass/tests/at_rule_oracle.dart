// Run with Dart 3.6.2 and sass 1.102.0 selected by --packages.
// Arguments: input.scss|input.sass|input.css expanded|compressed.
// Writes only the official CSS.
import 'dart:io';
import 'package:sass/sass.dart' as sass;

void main(List<String> arguments) {
  final source = File(arguments[0]).readAsStringSync();
  final style = switch (arguments[1]) {
    'expanded' => sass.OutputStyle.expanded,
    'compressed' => sass.OutputStyle.compressed,
    _ => throw ArgumentError.value(arguments[1], 'style'),
  };
  final syntax = arguments[0].endsWith('.sass')
      ? sass.Syntax.sass
      : arguments[0].endsWith('.css')
          ? sass.Syntax.css
          : sass.Syntax.scss;
  stdout.write(
      sass.compileStringToResult(source, style: style, syntax: syntax).css);
}
