class A {}
class B extends A {}
class D<T> {}
typedef Alias<T extends B> = D<T>;
main() {
  D d = Alias<A>();
//  ^
// [diag.unusedLocalVariable] The value of the local variable 'd' isn't used.
//            ^
// [diag.typeArgumentNotMatchingBounds] 'A' doesn't conform to the bound 'B' of the type parameter 'T'.
}
