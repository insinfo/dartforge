class A {
  A({required int? a, required int? b});
}
class B extends A {
  B({required super.a});
//^
// [diag.implicitSuperInitializerMissingArguments] The implicitly invoked unnamed constructor from 'A' has required parameters.
}
