class _A {}
//    ^^
// [diag.unusedElement] The declaration '_A' isn't referenced.
main(p) {
  if (p is _A) {
  }
}
