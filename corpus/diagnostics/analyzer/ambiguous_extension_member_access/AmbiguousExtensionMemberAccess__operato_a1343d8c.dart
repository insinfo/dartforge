extension E1 on int {
  int operator[](int index) => 0;
}

extension E2 on int {
  void operator[]=(int index, int value) {}
}

f() {
  0[1] += 2;
//^
// [diag.ambiguousExtensionMemberAccessTwo] A member named '[]' is defined in 'extension E1 on int' and 'extension E2 on int', and neither is more specific.
}
