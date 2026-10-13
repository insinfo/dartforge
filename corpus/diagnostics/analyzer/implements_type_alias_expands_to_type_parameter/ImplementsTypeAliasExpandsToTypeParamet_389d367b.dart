class A {}
typedef T<X extends A> = X;
mixin M implements T {}
//                 ^
// [diag.implementsTypeAliasExpandsToTypeParameter] A type alias that expands to a type parameter can't be implemented.
