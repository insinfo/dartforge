extension E on int {
  void m() {}
}
f() {
  E.m();
//  ^
// [diag.staticAccessToInstanceMember] Instance member 'm' can't be accessed using static access.
}
