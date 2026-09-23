enum E {
  v;
  static int x = 1;
  const E() : x = 0;
//            ^^^^^
// [diag.initializerForStaticField] 'x' is a static field in the enclosing class. Fields initialized in a constructor can't be static.
}
