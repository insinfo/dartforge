class C<T> {
  late T<int> f;
//     ^^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'T' is declared with 0 type parameters, but 1 type arguments were given.
}
