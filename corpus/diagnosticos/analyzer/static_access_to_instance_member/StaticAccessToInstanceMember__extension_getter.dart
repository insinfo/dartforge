extension E on int {
  int get g => 0;
}
f() {
  E.g;
//  ^
// [diag.staticAccessToInstanceMember] Instance member 'g' can't be accessed using static access.
}
