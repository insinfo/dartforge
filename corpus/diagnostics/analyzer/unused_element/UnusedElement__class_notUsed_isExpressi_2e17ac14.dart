class _A {}
//    ^^
// [diag.unusedElement] The declaration '_A' isn't referenced.
void f(Object p) {
  if (p is void Function<T extends _A>()) {
  }
}
