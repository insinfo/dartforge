class A {
  int get x => 0;
  A() : x = 0;
//      ^^^^^
// [diag.initializerForNonExistentField] 'x' isn't a field in the enclosing class.
}
