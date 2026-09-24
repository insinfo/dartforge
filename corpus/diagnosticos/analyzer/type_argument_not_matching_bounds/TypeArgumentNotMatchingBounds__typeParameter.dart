class A {}
class B {}
class C {}
class G<E extends A> {}
class D<F extends G<B>> {}
//                ^^^^
// [context 1] The inverted type 'G<B>' is also not regular-bounded, so the type is not well-bounded.
//                  ^
// [diag.typeArgumentNotMatchingBounds][context 1] 'B' doesn't conform to the bound 'A' of the type parameter 'E'.
