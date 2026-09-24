class A<T> {
  static int i = 1;
}

var x = A<int>.i;
//      ^^^^^^^^
// [diag.classInstantiationAccessToStaticMember] The static member 'i' can't be accessed on a class instantiation.
