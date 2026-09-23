class A {}

class B {
  A.new();
//^
// [diag.invalidConstructorName] The name of a constructor must match the name of the enclosing class.
  B();
}
