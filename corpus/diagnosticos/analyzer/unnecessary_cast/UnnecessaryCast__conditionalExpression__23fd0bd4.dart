dynamic f(bool c, int a, int b) {
  return c ? a as int : b;
//           ^^^^^^^^
// [diag.unnecessaryCast] Unnecessary cast.
}
