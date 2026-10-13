class A {}
class C<E> {}
f(p) {
  return p is C<A, A>;
//            ^^^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'C' is declared with 1 type parameters, but 2 type arguments were given.
}
