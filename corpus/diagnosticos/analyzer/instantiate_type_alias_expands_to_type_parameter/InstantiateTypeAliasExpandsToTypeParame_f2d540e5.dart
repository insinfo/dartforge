typedef A<T> = T;

void f() {
  new A();
//    ^
// [diag.instantiateTypeAliasExpandsToTypeParameter] Type aliases that expand to a type parameter can't be instantiated.
}
