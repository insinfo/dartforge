class A {}
typedef T<X extends A> = X;
class B extends T<A> {}
//              ^
// [diag.extendsTypeAliasExpandsToTypeParameter] A type alias that expands to a type parameter can't be used as a superclass.
