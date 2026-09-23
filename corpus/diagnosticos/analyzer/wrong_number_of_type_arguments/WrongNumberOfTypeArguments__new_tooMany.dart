class C<E> {}

f() {
  return new C<int, int>();
//           ^^^^^^^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'C' is declared with 1 type parameters, but 2 type arguments were given.
}
