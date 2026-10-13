class A {
  A.named();
}

typedef B<T> = T;

void f() {
  new B<A>.named();
//    ^
// [diag.instantiateTypeAliasExpandsToTypeParameter] Type aliases that expand to a type parameter can't be instantiated.
}
