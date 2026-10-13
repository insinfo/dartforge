class A {}
class B {}
class G<T extends A> {}
typedef X = G<B>;
//            ^
// [diag.typeArgumentNotMatchingBounds] 'B' doesn't conform to the bound 'A' of the type parameter 'T'.
