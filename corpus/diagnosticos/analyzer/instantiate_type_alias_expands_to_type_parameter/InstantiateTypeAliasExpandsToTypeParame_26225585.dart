class A {}

typedef B<T> = T;

void f() {
  new B<A>();
//    ^
// [diag.instantiateTypeAliasExpandsToTypeParameter] Type aliases that expand to a type parameter can't be instantiated.
}
