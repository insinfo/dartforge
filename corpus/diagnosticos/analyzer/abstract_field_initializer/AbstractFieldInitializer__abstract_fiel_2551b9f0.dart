abstract class A {
  abstract final int x = 0;
//                   ^
// [diag.abstractFieldInitializer] Abstract fields can't have initializers.
}
