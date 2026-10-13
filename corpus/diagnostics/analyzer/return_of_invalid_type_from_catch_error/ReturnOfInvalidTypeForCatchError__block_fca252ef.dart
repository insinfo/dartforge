void f(Future<int> future) {
  future.catchError((e, st) async {
    return;
//  ^^^^^^
// [diag.returnWithoutValue] The return value is missing after 'return'.
  });
}
