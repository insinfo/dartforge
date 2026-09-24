class A<T> {
  int i = 1;
}

typedef TA<T> = A<T>;

var x = TA<int>.i;
//      ^^^^^^^^^
// [diag.classInstantiationAccessToInstanceMember] The instance member 'i' can't be accessed on a class instantiation.
