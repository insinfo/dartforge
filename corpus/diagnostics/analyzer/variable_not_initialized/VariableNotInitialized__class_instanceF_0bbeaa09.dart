abstract class A {
  abstract int v;
  augment external int v = 0;
//                     ^
// [diag.externalFieldInitializer] External fields can't have initializers.
}
