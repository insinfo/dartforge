extension E1 on int {
  set a(x) {}
}

extension E2 on int {
  set a(x) {}
}

f() {
  0.a = 3;
//  ^
// [diag.ambiguousExtensionMemberAccessTwo] A member named 'a' is defined in 'extension E1 on int' and 'extension E2 on int', and neither is more specific.
}
