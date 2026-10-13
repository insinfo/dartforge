extension type E(int it) {
  static int x = 1;
  E.named() : x = 0, this.it = 0;
//            ^^^^^
// [diag.initializerForStaticField] 'x' is a static field in the enclosing class. Fields initialized in a constructor can't be static.
}
