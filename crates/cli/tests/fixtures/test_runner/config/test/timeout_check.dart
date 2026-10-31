@t.Timeout(Duration(milliseconds: 250))
@t.TestOn('vm && !windows')
library;

import 'dart:async';
import 'package:test/test.dart' as t;

void main() {
  t.test('exceeds the library timeout', () async {
    await Future<void>.delayed(const Duration(seconds: 2));
  }, retry: 0);
  t.test('within the library timeout', () {});
  t.test('test timeout overrides', () async {
    await Future<void>.delayed(const Duration(milliseconds: 400));
  }, timeout: const t.Timeout.factor(4));
}
