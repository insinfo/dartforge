class A {}
class B {}
class C<E> {}
class D<E extends A> {}
C<D<B>> c = (throw 0);
//^^^^
// [context 1] The inverted type 'D<B>' is also not regular-bounded, so the type is not well-bounded.
//  ^
// [diag.typeArgumentNotMatchingBounds][context 1] 'B' doesn't conform to the bound 'A' of the type parameter 'E'.
