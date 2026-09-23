class A {}
class B {}
typedef F = void Function<T extends A>();
void f<T extends void Function<U extends B>()>() {}
void g() {
  f<F>();
//  ^
// [diag.typeArgumentNotMatchingBounds] 'F' doesn't conform to the bound 'void Function<U extends B>()' of the type parameter 'T'.
}
