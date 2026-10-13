class A {}
class B {}
mixin G<E extends A> {}
class C extends Object with G<B>{}
//                            ^
// [diag.typeArgumentNotMatchingBounds] 'B' doesn't conform to the bound 'A' of the type parameter 'E'.
