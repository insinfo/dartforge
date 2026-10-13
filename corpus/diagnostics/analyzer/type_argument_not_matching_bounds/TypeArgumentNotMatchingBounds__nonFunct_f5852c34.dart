class A<T extends A<T>> {}
typedef X<T> = A;
//             ^
// [context 1] The raw type was instantiated as 'A<A<dynamic>>', and is not regular-bounded.
// [diag.typeArgumentNotMatchingBounds][context 1] 'A<dynamic>' doesn't conform to the bound 'A<A<dynamic>>' of the type parameter 'T'.
