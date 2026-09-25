class A {
  const A(String p);
}
class B extends A {
  const B() : super(42);
//                  ^^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'String'.
}
