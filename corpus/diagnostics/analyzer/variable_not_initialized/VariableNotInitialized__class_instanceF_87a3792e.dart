class A {
  int v;
//    ^
// [diag.notInitializedNonNullableInstanceField] Non-nullable instance field 'v' must be initialized.

  factory A() => throw 0;
}
