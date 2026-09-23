dynamic f(bool c, int a, dynamic b) {
  return c ? a as int : b;
//           ^^^^^^^^
// [diag.unnecessaryCast] Unnecessary cast.
}
