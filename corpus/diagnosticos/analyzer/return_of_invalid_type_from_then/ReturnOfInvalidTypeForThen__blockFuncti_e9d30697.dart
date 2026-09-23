void f(Future<int> future) {
  future.then<int>((_) => 0, onError: (e, st) async {
    return;
//  ^^^^^^
// [diag.returnWithoutValue] The return value is missing after 'return'.
  });
}
