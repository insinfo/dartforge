import 'dart:async';

Future<int> foo(FutureOr<int> v) async {
  try {
    return v;
//  ^^^^^^
// [diag.unawaitedReturnInTryBlock] Returning a 'Future' without 'await' inside a try block.
  } catch (_) {}
  return v;
}
