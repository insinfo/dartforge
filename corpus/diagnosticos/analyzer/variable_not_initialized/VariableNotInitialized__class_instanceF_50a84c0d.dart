class A {
  int v1, v2, v3;

  A() : v1 = 0, v3 = 0;
//^
// [diag.notInitializedNonNullableInstanceFieldConstructor] Non-nullable instance field 'v2' must be initialized.
}
