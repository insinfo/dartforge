class C {
  int get value => 0;
  set value(int newValue) {}
}
extension E on C {
  set sign(int sign) {
    value = super.value * sign;
//          ^^^^^
// [diag.superInExtension] The 'super' keyword can't be used in an extension because an extension doesn't have a superclass.
  }
}
