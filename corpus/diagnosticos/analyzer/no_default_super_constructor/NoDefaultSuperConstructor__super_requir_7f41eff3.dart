class A {
  A(int p);
}
class B extends A {
  new named();
//^^^^^^^^^
// [diag.implicitSuperInitializerMissingArguments] The implicitly invoked unnamed constructor from 'A' has required parameters.
}
