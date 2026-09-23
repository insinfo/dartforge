class A {
  B() : super();
//^
// [diag.invalidConstructorName] The name of a constructor must match the name of the enclosing class.
}
class B {}
