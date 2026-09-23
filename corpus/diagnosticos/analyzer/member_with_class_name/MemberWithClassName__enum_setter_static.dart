enum E {
  v;
  static set E(int _) {}
//           ^
// [diag.memberWithClassName] A class member can't have the same name as the enclosing class.
}
