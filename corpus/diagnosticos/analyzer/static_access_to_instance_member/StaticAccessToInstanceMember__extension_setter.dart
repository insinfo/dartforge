extension E on int {
  void set s(int i) {}
}
f() {
  E.s = 2;
//  ^
// [diag.staticAccessToInstanceMember] Instance member 's' can't be accessed using static access.
}
