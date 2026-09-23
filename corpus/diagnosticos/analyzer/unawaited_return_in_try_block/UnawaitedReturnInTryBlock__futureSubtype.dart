Future<int> foo() async {
  try {
    return MyFuture();
//  ^^^^^^
// [diag.unawaitedReturnInTryBlock] Returning a 'Future' without 'await' inside a try block.
  } catch (_) {}
  return MyFuture();
}

class MyFuture implements Future<int> {
  @override
  noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
