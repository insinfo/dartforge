class A {
  A({required int? a, required int? b});
}
class B({required super.a}) extends A {
  this;
//^^^^
// [diag.implicitSuperInitializerMissingArguments] The implicitly invoked unnamed constructor from 'A' has required parameters.
}
