class A {
  var f;
}
main() {
  A.f;
//  ^
// [diag.staticAccessToInstanceMember] Instance member 'f' can't be accessed using static access.
}