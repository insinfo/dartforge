class A<T> {}

extension E on A {
  int get i => 1;
}

var x = A<int>.i;
//      ^^^^^^^^
// [diag.classInstantiationAccessToUnknownMember] The class 'A' doesn't have a constructor named 'i'.
