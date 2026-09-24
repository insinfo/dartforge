Future<void> foo() async {
  try {
    {
      return Future<Null>.value(null);
//    ^^^^^^
// [diag.unawaitedReturnInTryBlock] Returning a 'Future' without 'await' inside a try block.
    }
  } catch (_) {}
}
