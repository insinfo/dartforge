void f(Object? x) {
  switch (x) {
    case 0 && _:
//            ^
// [diag.unnecessaryWildcardPattern] Unnecessary wildcard pattern.
      break;
  }
}
