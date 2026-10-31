@TestOn('browser')
library;

import 'package:test/test.dart';

void main() {
  test('browser only', () => fail('browser suite ran on the VM'));
}
