class B<E> {}
f(B<A> b) {}
//  ^
// [diag.nonTypeAsTypeArgument] The name 'A' isn't a type, so it can't be used as a type argument.
