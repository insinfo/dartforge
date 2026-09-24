class A {}
class B {}
class G<E extends A> {}
class C extends G<B>{}
//                ^
// [diag.typeArgumentNotMatchingBounds] 'B' doesn't conform to the bound 'A' of the type parameter 'E'.
