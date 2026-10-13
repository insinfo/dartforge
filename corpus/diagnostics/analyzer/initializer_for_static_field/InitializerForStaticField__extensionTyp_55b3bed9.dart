extension type E(int it) {
  static int x = 0;
  E.named(this.x) : this.it = 0;
//        ^^^^
// [diag.initializerForStaticField] 'x' is a static field in the enclosing class. Fields initialized in a constructor can't be static.
}
