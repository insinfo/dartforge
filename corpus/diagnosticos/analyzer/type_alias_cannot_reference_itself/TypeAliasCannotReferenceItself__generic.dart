typedef F = void Function(List<G> l);
//      ^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
typedef G = void Function(List<F> l);
//      ^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
main() {
  F? foo(G? g) => g;
  foo(null);
}
