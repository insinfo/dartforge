extension type A<T extends A<T>>(int it) {}

void f(A a) {}
//     ^
// [context 1] The raw type was instantiated as 'A<A<dynamic>>', and is not regular-bounded.
// [diag.typeArgumentNotMatchingBounds][context 1] 'A<dynamic>' doesn't conform to the bound 'A<A<dynamic>>' of the type parameter 'T'.
