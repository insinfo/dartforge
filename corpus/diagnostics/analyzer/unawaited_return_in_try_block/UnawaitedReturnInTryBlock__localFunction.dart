void f() {
  Future<int> foo() async {
    try {
      return Future.value(0);
//    ^^^^^^
// [diag.unawaitedReturnInTryBlock] Returning a 'Future' without 'await' inside a try block.
    } catch (_) {}
    return 0;
  }
  foo();
}
