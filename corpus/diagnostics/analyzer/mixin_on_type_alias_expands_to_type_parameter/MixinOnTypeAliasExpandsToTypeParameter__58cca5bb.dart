class A {}
typedef T<X extends A> = X;
mixin M on T<A> {}
//         ^
// [diag.mixinOnTypeAliasExpandsToTypeParameter] A type alias that expands to a type parameter can't be used as a superclass constraint.
