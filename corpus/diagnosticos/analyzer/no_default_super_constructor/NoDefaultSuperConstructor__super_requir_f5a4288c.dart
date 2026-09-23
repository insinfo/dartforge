class A {
  A(int p);
}
class B() extends A;
//    ^
// [diag.implicitSuperInitializerMissingArguments] The implicitly invoked unnamed constructor from 'A' has required parameters.
