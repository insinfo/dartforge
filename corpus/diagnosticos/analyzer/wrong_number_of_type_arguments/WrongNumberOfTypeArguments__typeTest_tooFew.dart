class A {}
class C<K, V> {}
f(p) {
  return p is C<A>;
//            ^^^^
// [diag.wrongNumberOfTypeArguments] The type 'C' is declared with 2 type parameters, but 1 type arguments were given.
}
