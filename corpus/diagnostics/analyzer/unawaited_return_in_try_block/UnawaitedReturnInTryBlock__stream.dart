import 'dart:async';

Stream<int> foo() async* {
  try {
    yield 42;
    return Future.value(42);
//  ^^^^^^
// [diag.returnInGenerator] Can't return a value from a generator function that uses the 'async*' or 'sync*' modifier.
  } catch (_) {}
}
