extension E on int {
  static m<T>() {}
}
f(int a) {
  a.m<int>;
//  ^
// [diag.undefinedGetter] The getter 'm' isn't defined for the type 'int'.
}
