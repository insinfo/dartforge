class A {}
class B {}
typedef F<T extends A>();
F<B> fff = (throw 42);
// [context 1][column 1][length 4] The inverted type 'F<B>' is also not regular-bounded, so the type is not well-bounded.
//^
// [diag.typeArgumentNotMatchingBounds][context 1] 'B' doesn't conform to the bound 'A' of the type parameter 'T'.
