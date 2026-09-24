enum E {
  v(0);
  const E(super.x);
//              ^
// [diag.superFormalParameterWithoutAssociatedPositional] No associated positional super constructor parameter.
}
