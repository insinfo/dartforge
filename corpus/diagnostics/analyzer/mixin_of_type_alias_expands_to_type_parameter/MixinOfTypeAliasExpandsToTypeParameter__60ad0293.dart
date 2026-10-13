mixin A {}
typedef T<X extends A> = X;
class B with T<A> {}
//           ^
// [diag.mixinOfTypeAliasExpandsToTypeParameter] A type alias that expands to a type parameter can't be mixed in.
