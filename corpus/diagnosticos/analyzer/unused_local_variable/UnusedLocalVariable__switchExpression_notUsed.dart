Object? f(Object? x) {
  return switch (x) {
    (int a,) => 0,
//       ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
    _ => 0,
  };
}
