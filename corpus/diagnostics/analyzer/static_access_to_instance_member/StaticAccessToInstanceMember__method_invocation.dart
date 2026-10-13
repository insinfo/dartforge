class A {
  m() {}
}
main() {
  A.m();
//  ^
// [diag.staticAccessToInstanceMember] Instance member 'm' can't be accessed using static access.
}
