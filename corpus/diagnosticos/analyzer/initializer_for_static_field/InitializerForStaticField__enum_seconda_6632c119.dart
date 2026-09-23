enum E {
  v(0);
  static int x = 0;
  const E(this.x);
//        ^^^^
// [diag.initializerForStaticField] 'x' is a static field in the enclosing class. Fields initialized in a constructor can't be static.
}
