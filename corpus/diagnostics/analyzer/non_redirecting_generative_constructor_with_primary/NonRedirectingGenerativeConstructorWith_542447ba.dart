enum E(int x) {
  v(0);
  const E.named(int x) : this(x);
//        ^^^^^
// [diag.unusedElement] The declaration 'E.named' isn't referenced.
}
