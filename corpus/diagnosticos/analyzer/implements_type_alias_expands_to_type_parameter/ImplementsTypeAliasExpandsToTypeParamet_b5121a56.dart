class A {}
typedef T<X extends A> = X;
class B implements T {}
//                 ^
// [diag.implementsTypeAliasExpandsToTypeParameter] A type alias that expands to a type parameter can't be implemented.
