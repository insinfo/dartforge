abstract class A {
  int get v;
}

class B extends A {
  var v;
//    ^
// [diag.notInitializedNonNullableInstanceField] Non-nullable instance field 'v' must be initialized.
}
