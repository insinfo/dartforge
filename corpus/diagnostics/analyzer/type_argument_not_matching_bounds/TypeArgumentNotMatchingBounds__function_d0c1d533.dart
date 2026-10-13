class C {
  void call<T extends num>() {}
}

void f(C c) {
  c<String>();
//  ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
}
