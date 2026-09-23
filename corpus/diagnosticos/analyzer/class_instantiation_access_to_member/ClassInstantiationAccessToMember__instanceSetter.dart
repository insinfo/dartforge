class A<T> {
  set i(int value) {}
}

void foo() {
  A<int>.i = 7;
//^^^^^^^^
// [diag.classInstantiationAccessToInstanceMember] The instance member 'i' can't be accessed on a class instantiation.
}
