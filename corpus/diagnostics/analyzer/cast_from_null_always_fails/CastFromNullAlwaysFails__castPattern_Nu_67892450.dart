void f(Null n, num? m) {
  (m as int?) = n;
//   ^^
// [diag.unnecessaryCastPattern] Unnecessary cast pattern.
}
