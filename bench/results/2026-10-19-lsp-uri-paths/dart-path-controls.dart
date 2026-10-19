// Run with Dart 3.6.2 on the validation host.
void main() {
  for (final value in [
    'file:///tmp/folder%20space/',
    'file:///tmp/folder%2Fspace/',
    'file:///tmp/hash#tag/',
    'file:///tmp/what?name/',
    'file://localhost/tmp/hello/',
    'file:///tmp/%FF/',
    'file:///tmp/%2520/',
    'file:///tmp/%zz/',
  ]) {
    try {
      print('$value -> ${Uri.parse(value).toFilePath()}');
    } catch (error) {
      print('$value -> $error');
    }
  }
}
