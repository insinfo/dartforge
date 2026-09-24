Future<int> foo() async {
  try {
    try {} catch (_) {
      return Future.value(42);
//    ^^^^^^
// [diag.unawaitedReturnInTryBlock] Returning a 'Future' without 'await' inside a try block.
    }
  } catch (_) {}
  return -1;
}
