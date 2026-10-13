class C {
  const C();
}

f() {
  return const C<int>();
//             ^^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'C' is declared with 0 type parameters, but 1 type arguments were given.
}
