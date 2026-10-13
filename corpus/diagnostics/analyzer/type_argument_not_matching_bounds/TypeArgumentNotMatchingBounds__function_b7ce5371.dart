class A {}
class B {}
class G<E extends A> {}
G<B> f() => throw 0;
// [context 1][column 1][length 4] The inverted type 'G<B>' is also not regular-bounded, so the type is not well-bounded.
//^
// [diag.typeArgumentNotMatchingBounds][context 1] 'B' doesn't conform to the bound 'A' of the type parameter 'E'.
