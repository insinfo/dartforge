class A {
  A({required int? a});
}
class B extends A {
  B.named();
//^^^^^^^
// [diag.implicitSuperInitializerMissingArguments] The implicitly invoked unnamed constructor from 'A' has required parameters.
}
