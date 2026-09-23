dynamic f(bool c, int a, int b) {
  return c ? a as int : b as int;
//           ^^^^^^^^
// [diag.unnecessaryCast] Unnecessary cast.
//                      ^^^^^^^^
// [diag.unnecessaryCast] Unnecessary cast.
}
