void f(Object? x) {
  if (x case 0 && _) {}
//                ^
// [diag.unnecessaryWildcardPattern] Unnecessary wildcard pattern.
}
