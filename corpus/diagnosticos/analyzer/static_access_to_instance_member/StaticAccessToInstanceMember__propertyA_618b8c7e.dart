class A {
  set f(x) {}
}
main() {
  A.f = 42;
//  ^
// [diag.staticAccessToInstanceMember] Instance member 'f' can't be accessed using static access.
}
