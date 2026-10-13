class C<T, U> {}
var t = C<int>;
//       ^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'C' is declared with 2 type parameters, but 1 type arguments were given.
