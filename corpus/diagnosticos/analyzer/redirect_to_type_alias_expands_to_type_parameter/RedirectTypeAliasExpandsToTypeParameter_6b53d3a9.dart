class A implements C {
  A.named();
}

typedef B<T> = T;

class C {
  factory C() = B<A>.named;
//              ^
// [diag.redirectToTypeAliasExpandsToTypeParameter] A redirecting constructor can't redirect to a type alias that expands to a type parameter.
}
