class A {}
typedef X<T extends A> = Map<int, T>;
void f(X<String> a) {}
//     ^^^^^^^^^
// [context 1] The inverted type 'X<String>' is also not regular-bounded, so the type is not well-bounded.
//       ^^^^^^
// [diag.typeArgumentNotMatchingBounds][context 1] 'String' doesn't conform to the bound 'A' of the type parameter 'T'.
