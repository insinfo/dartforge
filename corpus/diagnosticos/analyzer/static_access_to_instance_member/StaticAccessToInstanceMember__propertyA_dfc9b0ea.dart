class A {
  get f => 42;
}
main() {
  A.f;
//  ^
// [diag.staticAccessToInstanceMember] Instance member 'f' can't be accessed using static access.
}
