typedef A<T> = T;
typedef B<T> = A<T>;

void f() {
  new B();
//    ^
// [diag.instantiateTypeAliasExpandsToTypeParameter] Type aliases that expand to a type parameter can't be instantiated.
}
