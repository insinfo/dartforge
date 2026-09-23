dynamic f(bool c, int a, int b) {
  return c ? a : b as int;
//               ^^^^^^^^
// [diag.unnecessaryCast] Unnecessary cast.
}
