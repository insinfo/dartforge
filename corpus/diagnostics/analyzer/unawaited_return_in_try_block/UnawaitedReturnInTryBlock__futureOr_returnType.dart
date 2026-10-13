import 'dart:async';

FutureOr<int> foo() async {
  try {
    return Future.value(42);
//  ^^^^^^
// [diag.unawaitedReturnInTryBlock] Returning a 'Future' without 'await' inside a try block.
  } catch (_) {}
  return Future.value(42);
}
