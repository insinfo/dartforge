extension E1 on int {
  void a() {}
}

extension E2 on int {
  void a() {}
}

f() {
  0.a();
//  ^
// [diag.ambiguousExtensionMemberAccessTwo] A member named 'a' is defined in 'extension E1 on int' and 'extension E2 on int', and neither is more specific.
}
