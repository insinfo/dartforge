void f(x) {
  if (x case <int, int>[0]) {}
//           ^^^^^^^^^^
// [diag.expectedOneListPatternTypeArguments] List patterns require one type argument or none, but 2 found.
}
