class A {}
class B extends A {}
class C extends B {}
class G<E extends B> {}
f() { return new G<A>(); }
//                 ^
// [diag.typeArgumentNotMatchingBounds] 'A' doesn't conform to the bound 'B' of the type parameter 'E'.
