class A<T extends num> {
  const A();
}

@A<String>()
// ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
void f() {}
