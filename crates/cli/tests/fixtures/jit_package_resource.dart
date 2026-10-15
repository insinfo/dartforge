import 'dart:io';
import 'dart:isolate';

void main() {
  final uri = Isolate.resolvePackageUriSync(
      Uri.parse('package:resource_package/resource.txt'));
  if (uri == null) throw StateError('The selected package resource is missing');
  print(File.fromUri(uri).readAsStringSync().trim());
}
