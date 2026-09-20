// Oracle Dart 3.6.2: somente o primeiro ramo satisfeito deve ser carregado.
import 'fallback.dart'
  if (dart.library.io) 'native.dart'
  if (dart.library.io) 'missing_native.dart'
  if (dart.library.js) 'javascript.dart'
  if (dart.library.js_interop) 'wasm.dart'
  if (dart.library.js_interop) 'missing_javascript.dart' show label;
import 'absent.dart' if (unconfigured.flag) 'dart:unsupported_inactive' show absentLabel;
import 'exports.dart' show exportedLabel;
import 'package:fixture/choice.dart'
  if (dart.library.io == 'true') 'package:fixture/native_choice.dart'
  if (dart.library.js_interop == 'true') 'package:fixture/javascript_choice.dart' show packageLabel;
import 'flavor_default.dart' if (build.flavor == 'blue') 'flavor_blue.dart' show flavorLabel;
import 'cycle_a.dart' show cycleValue;
void main() {
  print(label());
  print(absentLabel());
  print(exportedLabel());
  print(packageLabel());
  print(flavorLabel());
  print(cycleValue());
}
