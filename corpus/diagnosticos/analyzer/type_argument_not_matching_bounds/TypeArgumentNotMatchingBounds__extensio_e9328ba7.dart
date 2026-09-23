extension type A<T extends num>(int it) {}

void f(A<String> a) {}
//       ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
