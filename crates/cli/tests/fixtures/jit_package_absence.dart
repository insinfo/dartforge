import 'dart:isolate';
void check(String name, Object? Function() action) {
  try { print('$name:${action()}'); }
  catch (error) { print('$name:error:${error.runtimeType}'); }
}
Future<void> main() async {
  check('config', () => Isolate.packageConfigSync);
  for (final text in ['package:missing/value.dart', 'package:invalid', 'file:///tmp/item', 'https://example.com/item', 'relative/item']) {
    check(text, () => Isolate.resolvePackageUriSync(Uri.parse(text)));
  }
  print('asyncConfig:${await Isolate.packageConfig}');
  print('asyncUnknown:${await Isolate.resolvePackageUri(Uri.parse('package:missing/value.dart'))}');
}
