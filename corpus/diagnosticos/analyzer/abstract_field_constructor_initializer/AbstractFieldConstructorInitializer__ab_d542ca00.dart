abstract class A {
  abstract int x;
  A(this.x);
//       ^
// [diag.abstractFieldConstructorInitializer] Abstract fields can't have initializers.
}
