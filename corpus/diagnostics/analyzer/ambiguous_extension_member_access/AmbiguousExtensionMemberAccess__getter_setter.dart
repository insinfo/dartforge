extension E on int {
  int get a => 1;
}

extension E2 on int {
  set a(int v) { }
}

f() {
  0.a;
//  ^
// [diag.ambiguousExtensionMemberAccessTwo] A member named 'a' is defined in 'extension E on int' and 'extension E2 on int', and neither is more specific.
}
