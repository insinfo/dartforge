import 'dart:async';
import 'package:dartforge/resources.dart';

void require(bool value, String message) {
  if (!value) throw StateError(message);
}

final class Counter implements ScopedResource {
  int attempts = 0;
  Object? disposalError;
  @override
  void dispose() {
    attempts++;
    final error = disposalError;
    if (error != null) throw error;
  }
}

Future<void> main() async {
  final sync = Counter();
  final value = using(sync, (resource) {
    require(resource.attempts == 0, 'sync lifetime');
    return 7;
  });
  require(value == 7 && sync.attempts == 1, 'sync success disposal');
  print('sync success=7 dispose=1');

  final bodyError = StateError('body');
  final cleanupError = StateError('cleanup');
  for (var failure = 0; failure < 3; failure++) {
    final resource = Counter();
    if (failure != 0) resource.disposalError = cleanupError;
    Object? observed;
    try {
      using<Counter, int>(resource, (_) {
        if (failure != 1) throw bodyError;
        return 8;
      });
    } catch (error) {
      observed = error;
    }
    require(identical(observed, failure == 0 ? bodyError : cleanupError),
        'sync exception precedence');
    require(resource.attempts == 1, 'sync disposal attempts');
    print(
        'sync failure=$failure dispose=1 primary=${failure == 0 ? 'body' : 'cleanup'}');
  }

  final asynchronous = Counter();
  final gate = Completer<int>();
  final pending = usingAsync(asynchronous, (resource) async {
    final answer = await gate.future;
    require(resource.attempts == 0, 'async lifetime after await');
    return answer;
  });
  require(
      asynchronous.attempts == 0, 'async must not dispose before completion');
  gate.complete(11);
  require(await pending == 11 && asynchronous.attempts == 1,
      'async success disposal');
  print('async success=11 dispose=1 after-await');

  for (var failure = 0; failure < 5; failure++) {
    final resource = Counter();
    if (failure >= 2) resource.disposalError = cleanupError;
    Object? observed;
    try {
      await usingAsync<Counter, int>(resource, (_) {
        if (failure == 0 || failure == 3) throw bodyError;
        if (failure == 1 || failure == 4) return Future<int>.error(bodyError);
        return 12;
      });
    } catch (error) {
      observed = error;
    }
    require(identical(observed, failure < 2 ? bodyError : cleanupError),
        'async exception precedence');
    require(resource.attempts == 1, 'async disposal attempts');
    print(
        'async failure=$failure dispose=1 primary=${failure < 2 ? 'body' : 'cleanup'}');
  }

  var releases = 0;
  final handle = ResourceHandle(19, (int value) {
    require(value == 19, 'adapter value');
    releases++;
  });
  require(await usingAsync(handle, (resource) => resource.value) == 19,
      'async synchronous body');
  handle.dispose();
  require(handle.isDisposed && releases == 1, 'adapter idempotent disposal');
  print('adapter value=19 dispose=1');
  print('resource scopes verified');
}
