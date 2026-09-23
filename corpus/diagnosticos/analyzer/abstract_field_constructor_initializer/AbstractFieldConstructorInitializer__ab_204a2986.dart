abstract class A {
  abstract int x;
  A() : x = 0;
//      ^
// [diag.abstractFieldConstructorInitializer] Abstract fields can't have initializers.
}
