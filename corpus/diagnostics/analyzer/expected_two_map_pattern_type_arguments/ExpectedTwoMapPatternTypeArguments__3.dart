void f(x) {
  if (x case <bool, int, String>{0: _}) {}
//           ^^^^^^^^^^^^^^^^^^^
// [diag.expectedTwoMapPatternTypeArguments] Map patterns require two type arguments or none, but 3 found.
}
