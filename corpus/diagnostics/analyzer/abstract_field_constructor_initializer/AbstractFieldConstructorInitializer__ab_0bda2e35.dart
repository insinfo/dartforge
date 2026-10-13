abstract class A {
  abstract final int x;
  A() : x = 0;
//      ^
// [diag.abstractFieldConstructorInitializer] Abstract fields can't have initializers.
}
