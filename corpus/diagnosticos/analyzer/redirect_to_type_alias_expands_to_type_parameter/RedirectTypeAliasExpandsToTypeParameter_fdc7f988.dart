class A implements C {}

typedef B<T> = T;

class C {
  factory C() = B<A>;
//              ^
// [diag.redirectToTypeAliasExpandsToTypeParameter] A redirecting constructor can't redirect to a type alias that expands to a type parameter.
}
