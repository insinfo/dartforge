@Tags(['fast'])
library;

import 'dart:async';
import 'package:test/test.dart';

var attempts = 0;

void main() {
  test('flaky passes on retry', () {
    attempts++;
    expect(attempts, 2);
  });
  test('slow by tag', () {}, tags: 'slow');
  test('quick tag timeout', () async {
    await Future<void>.delayed(const Duration(milliseconds: 900));
  }, tags: ['quick'], retry: 0);
  test('plain', () {});
}
