void f(Never? x) {
  x.toString(1 + 2);
//           ^^^^^
// [diag.extraPositionalArguments] Too many positional arguments: 0 expected, but 1 found.
}
