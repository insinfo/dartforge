void f(Object? x) {
  if (x case _ && 0) {}
//           ^
// [diag.unnecessaryWildcardPattern] Unnecessary wildcard pattern.
}
