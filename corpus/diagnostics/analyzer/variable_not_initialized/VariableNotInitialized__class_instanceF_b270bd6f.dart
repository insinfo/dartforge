class A {
  int v;

  A.foo(this.v);

  A.bar();
//^^^^^
// [diag.notInitializedNonNullableInstanceFieldConstructor] Non-nullable instance field 'v' must be initialized.
}
