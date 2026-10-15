import 'dart:async';
import 'dart:io';
import 'package:test/test.dart';

void main() {
  test('ordinary sync', () {
    expect(7, Platform.environment['INTEGRATION_FAILURE'] == 'assertion' ? 9 : 7,
        reason: 'ASSERTION_INTEGRATION');
  });
  test('awaited future', () async {
    await Future<void>.delayed(Duration.zero);
    expect(1 + 2, 3);
  });
  test('unawaited timer error', () {
    final callback = expectAsync0(() {});
    Timer.run(() {
      callback();
      if (Platform.environment['INTEGRATION_FAILURE'] == 'async') {
        throw StateError('ASYNC_INTEGRATION');
      }
    });
  });
}
