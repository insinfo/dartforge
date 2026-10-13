class A<T> {
  int i = 1;
}

var x = A<int>.i;
//      ^^^^^^^^
// [diag.classInstantiationAccessToInstanceMember] The instance member 'i' can't be accessed on a class instantiation.
