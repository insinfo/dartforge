class A {}
class B {}
mixin C {}
class G<E extends A> {}
class D = G<B> with C;
//          ^
// [diag.typeArgumentNotMatchingBounds] 'B' doesn't conform to the bound 'A' of the type parameter 'E'.
