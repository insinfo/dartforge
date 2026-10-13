void f(x) {
  if (x case <int>{0: _}) {}
//           ^^^^^
// [diag.expectedTwoMapPatternTypeArguments] Map patterns require two type arguments or none, but 1 found.
}
