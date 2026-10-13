enum E {
  v;
  static const x = y + 1;
//             ^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
  static const y = x + 1;
//             ^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
}
