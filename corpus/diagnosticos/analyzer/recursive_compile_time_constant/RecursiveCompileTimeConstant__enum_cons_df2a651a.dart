enum E {
  v(values);
//^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
  const E(Object a);
}
