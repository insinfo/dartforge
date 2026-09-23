class A {}
class B {}
class X<T extends A> {
  X(int x, int y) {}
  factory X.name(int x, int y) = X<B>;
//                               ^^^^
// [diag.redirectToInvalidReturnType] The return type 'X<B>' of the redirected constructor isn't a subtype of 'X<T>'.
//                                 ^
// [diag.typeArgumentNotMatchingBounds] 'B' doesn't conform to the bound 'A' of the type parameter 'T'.
}
