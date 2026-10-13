import 'dart:async';

FutureOr<Iterable<int>?> f() sync* {
  yield 3.14;
//      ^^^^
// [diag.yieldOfInvalidType] A yielded value of type 'double' must be assignable to 'int'.
  yield '2';
//      ^^^
// [diag.yieldOfInvalidType] A yielded value of type 'String' must be assignable to 'int'.
  yield Future<int>.value(0);
//      ^^^^^^^^^^^^^^^^^^^^
// [diag.yieldOfInvalidType] A yielded value of type 'Future<int>' must be assignable to 'int'.
}
