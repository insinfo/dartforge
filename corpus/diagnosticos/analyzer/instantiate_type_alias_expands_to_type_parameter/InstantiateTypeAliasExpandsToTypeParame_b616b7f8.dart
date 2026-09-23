typedef A<T> = T;

void f() {
  const A();
//      ^
// [diag.instantiateTypeAliasExpandsToTypeParameter] Type aliases that expand to a type parameter can't be instantiated.
}
