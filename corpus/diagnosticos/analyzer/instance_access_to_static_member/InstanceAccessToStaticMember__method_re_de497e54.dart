mixin A {
  static m<T>() {}
}
f(A a) {
  a.m<int>;
//  ^
// [diag.instanceAccessToStaticMember] The static method 'm' can't be accessed through an instance.
}
