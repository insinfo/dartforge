extension E<T extends num> on int {
  void call() {}
}

void f() {
  E<String>(0)();
//  ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
}
