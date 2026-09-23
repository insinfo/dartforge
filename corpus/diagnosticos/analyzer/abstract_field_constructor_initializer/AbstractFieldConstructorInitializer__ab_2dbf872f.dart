abstract class A {
  abstract final int x;
  A(this.x);
//       ^
// [diag.abstractFieldConstructorInitializer] Abstract fields can't have initializers.
}
