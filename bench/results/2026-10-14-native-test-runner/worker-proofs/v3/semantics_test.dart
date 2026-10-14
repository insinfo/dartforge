import 'dart:async';
import 'dart:io';
import 'package:test/test.dart';
Future<void> main() async {
  await Future<void>.delayed(Duration.zero);
  var value = 0;
  var bodies = 0;
  group('outer', () {
    setUpAll(() { value = 10; });
    tearDownAll(() { expect(value, 10); print('ALL_CLEAN'); });
    setUp(() async { await Future<void>.delayed(Duration.zero); value++; if (Platform.environment['TEST_FAILURE'] == 'setup') throw StateError('SETUP_ORACLE'); });
    tearDown(() async { await Future<void>.delayed(Duration.zero); value--; if (Platform.environment['TEST_FAILURE'] == 'teardown') throw StateError('TEARDOWN_ORACLE'); });
    group('inner', () {
      test('sync', () { expect(value, 11); bodies++; });
      test('async', () async {
        await Future<void>.delayed(const Duration(milliseconds: 1));
        expect(value, 11); bodies++;
      });
      test('callback', () {
        final callback = expectAsync1((int n) { expect(n, 7); bodies++; });
        Timer.run(() { callback(7); });
        addTearDown(() { print('DYNAMIC_CLEAN'); });
      });
      test('skip', () { fail('must not run'); }, skip: 'fixture skip');
      test('timeout', () async { if (Platform.environment['TEST_FAILURE'] == 'timeout') await Completer<void>().future; }, timeout: const Timeout(Duration(milliseconds: 20)));
      test('failure', () {
        if (Platform.environment['TEST_FAILURE'] == 'assertion') expect(1, 2);
        if (Platform.environment['TEST_FAILURE'] == 'async') {
          final callback = expectAsync0(() {});
          Timer.run(() { callback(); throw StateError('ASYNC_ORACLE'); });
        }
      });
    });
  });
}
