class A {}
class B {}
class G<E extends A> {
  const G();
}
f() { return const G<B>(); }
//                   ^
// [diag.typeArgumentNotMatchingBounds] 'B' doesn't conform to the bound 'A' of the type parameter 'E'.
