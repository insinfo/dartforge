typedef void F(List<G> l);
//           ^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
typedef void G(List<F> l);
//           ^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
main() {
  F? foo(G? g) => g;
  foo(null);
}
