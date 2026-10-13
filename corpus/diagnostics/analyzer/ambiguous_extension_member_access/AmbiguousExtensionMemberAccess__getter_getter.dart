extension E1 on int {
  void get a => 1;
}

extension E2 on int {
  void get a => 2;
}

f() {
  0.a;
//  ^
// [diag.ambiguousExtensionMemberAccessTwo] A member named 'a' is defined in 'extension E1 on int' and 'extension E2 on int', and neither is more specific.
}
