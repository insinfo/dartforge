class A<T> {
  static set i(int value) {}
}

void bar() {
  A<int>.i = 7;
//^^^^^^^^
// [diag.classInstantiationAccessToStaticMember] The static member 'i' can't be accessed on a class instantiation.
}
