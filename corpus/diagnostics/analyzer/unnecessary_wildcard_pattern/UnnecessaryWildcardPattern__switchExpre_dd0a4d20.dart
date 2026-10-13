void f(Object? x) {
  (switch (x) {
    0 && _ => 0,
//       ^
// [diag.unnecessaryWildcardPattern] Unnecessary wildcard pattern.
    _ => 1,
  });
}
