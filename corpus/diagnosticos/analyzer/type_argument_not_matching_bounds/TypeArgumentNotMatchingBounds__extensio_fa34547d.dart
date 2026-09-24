extension E<T extends num> on int {
  void foo() {}
}

void f() {
  E<String>(0).foo();
//  ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
}
