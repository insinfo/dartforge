class C<T> {
  const factory C() = C<C<T>>;
//                    ^^^^^^^
// [diag.recursiveFactoryRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
main() {
  const C<int>();
}
