class A<T> {
  const A();
}

typedef B<T extends num> = A<T>;

@B<String>()
// ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
void f() {}
