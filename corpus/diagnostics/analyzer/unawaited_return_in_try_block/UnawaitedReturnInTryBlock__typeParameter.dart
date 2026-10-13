Future<int> foo<T extends Future<int>>(T v) async {
  try {
    return v;
//  ^^^^^^
// [diag.unawaitedReturnInTryBlock] Returning a 'Future' without 'await' inside a try block.
  } catch (_) {}
  return v;
}
